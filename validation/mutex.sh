#!/usr/bin/env bash
# The owner's-port mutex: ONE resource, host port 25565.
#
#   source validation/mutex.sh
#   dw_mutex_acquire "owner-play-session"   # exits non-zero if someone else holds it
#   trap dw_mutex_release EXIT              # release is idempotent and never steals
#
# This lock means "someone is using the owner's client address, localhost:25565".
# Nothing else. The only things that may take it are the two sanctioned 25565
# binders — `docker compose … -f validation/owner-play.yaml` and
# `tools/playtest-server.sh` — and a human at the keyboard behind either of them.
#
# ## What this lock is NOT
#
# It used to guard the whole validation stack, because `compose.yaml` pinned
# `container_name: delvewright-server` and a fixed `127.0.0.1:25565` binding, so
# every caller aimed at the SAME container and the SAME port and had to be
# serialized. That made a lock out of something that should never have been
# shared: worker ladders queued on each other for no reason, and an island worker
# once waited 30+ minutes behind a holder whose session had ZERO containers
# running — the lock outlived the work, and nothing could tell.
#
# The fix is isolation by construction, not a better lock. `compose.yaml` now
# pins no container name and publishes no port (CI gate:
# `tools/check-compose-isolation.py`), so a worker ladder is fully described by
# its compose project: `validation/packtest-run.sh --project dw-<id>`,
# `validation/bot-run.sh --project dw-<id>`, teardown via
# `validation/fresh-volumes.sh --project dw-<id>`. Two of them on one host are
# independent, so **a worker ladder does not take this lock at all** — there is
# nothing left for it to contend over. If you find yourself waiting on this lock
# to run a ladder, the ladder is wrong, not the lock.
#
# ## Why it still exists, and the rules it enforces
#
# A hand-rolled waiter took the lock with `mkdir` and then guarded with
# `if [ ! -d "$LOCK" ]; then exit 1; fi`, meaning "bail if I did not get it".
# That guard tests **directory existence**, which is true precisely when SOMEONE
# ELSE holds the lock — the case it was written to catch. When the acquire loop
# timed out against a lock the owner held, the script sailed past the guard and
# ran a teardown that destroyed the OWNER'S live play session and its world
# volume mid-playtest.
#
# Three rules fall out of that, enforced here rather than left to each caller:
#
# 1. **Acquisition is a return value, never an inference from the filesystem.**
#    `mkdir` succeeding is the only proof of ownership.
# 2. **A holder must be named.** The lock directory carries a `HOLDER` file so
#    anyone can see whose session it is. `owner-play-session` is sacred: a human
#    is playing, and no automation may take 25565, however stale the lock looks.
# 3. **Release only what you took.** `dw_mutex_release` no-ops unless the HOLDER
#    is us, so a crashed script can never free someone else's lock — and no
#    teardown trap is ever installed before acquisition succeeds.
#
# The lock path never changes, on purpose: renaming it would make a new shell
# blind to a lock a live session had already taken.
set -uo pipefail

DW_MUTEX_DIR="${DW_MUTEX_DIR:-/private/tmp/delvewright-validation.lock.d}"
DW_MUTEX_ME=""

# The current holder's name, or empty if the lock is free.
dw_mutex_holder() {
  [ -d "$DW_MUTEX_DIR" ] || return 0
  cut -d' ' -f1 "$DW_MUTEX_DIR/HOLDER" 2>/dev/null || echo "unknown-holder"
}

# True while any container publishes host 25565 — i.e. an owner-facing session is
# actually up. Name-independent on purpose: `owner-play.yaml` and
# `tools/playtest-server.sh` use different container names, and what matters is
# the PORT, not who bound it.
# Capture, then test — a `| grep -q` here is a coin flip: grep exits at the first
# match, `docker ps` dies of SIGPIPE, and the caller's `pipefail` turns the match
# into a FALSE. This function decides whether the sacred 25565 mutex may be
# released, so a false negative frees the lock while a human is playing.
dw_mutex_port_bound() {
  command -v docker >/dev/null 2>&1 || return 1
  local ports
  ports="$(docker ps --format '{{.Ports}}' 2>/dev/null || true)"
  [[ $ports == *":25565->"* ]]
}

# Hard stop before binding 25565: refuse while a human is playing.
#
# A worker ladder does NOT need to call this — it publishes no host port and
# cannot reach the owner's session (isolation by construction). It is
# for the 25565 binders, and for a spike heavy enough that running it beside a
# live playtest would be rude (`warden-probe.sh`).
dw_mutex_assert_not_owner_session() {
  if [ "$(dw_mutex_holder)" = "owner-play-session" ]; then
    echo "REFUSING: the 25565 mutex is held by owner-play-session — a human is" >&2
    echo "playing on localhost:25565. Wait to be told it is free." >&2
    echo "(A project-scoped worker ladder needs no port; run it without this check.)" >&2
    return 1
  fi
  return 0
}

# dw_mutex_acquire <name> [wait-seconds]
# Returns 0 only when WE created the lock. Never proceeds on someone else's.
dw_mutex_acquire() {
  local me="${1:?dw_mutex_acquire needs a holder name}" wait_s="${2:-0}" waited=0
  while :; do
    if mkdir "$DW_MUTEX_DIR" 2>/dev/null; then
      printf '%s %s\n' "$me" "$(date +%s)" >"$DW_MUTEX_DIR/HOLDER"
      DW_MUTEX_ME="$me"
      echo "25565 mutex acquired by $me"
      return 0
    fi
    local holder; holder="$(dw_mutex_holder)"
    if [ "$holder" = "owner-play-session" ]; then
      echo "25565 mutex held by owner-play-session — refusing to wait or steal." >&2
      return 1
    fi
    if [ "$waited" -ge "$wait_s" ]; then
      echo "25565 mutex held by '${holder:-unknown}' after ${waited}s — NOT acquired, doing nothing." >&2
      return 1
    fi
    sleep 5
    waited=$((waited + 5))
  done
}

# Idempotent; releases only a lock this process actually took.
#
# NOTE the shell-state contract: this only works in the SAME shell session that
# ran dw_mutex_acquire ($DW_MUTEX_ME does not survive a new shell). A caller
# operating across shells — every agent tool invocation is a fresh shell — MUST
# use dw_mutex_release_named instead. The no-op below stays silent-successful
# because `trap dw_mutex_release EXIT` relies on it, but it says so on stderr:
# a shell that "releases" a lock it never took would otherwise echo its own
# success message and leave the lock in place for hours.
dw_mutex_release() {
  if [ -z "$DW_MUTEX_ME" ]; then
    echo "dw_mutex_release: nothing was acquired in THIS shell — no-op." >&2
    echo "  (cross-shell release: dw_mutex_release_named <holder-name>)" >&2
    return 0
  fi
  if [ "$(dw_mutex_holder)" = "$DW_MUTEX_ME" ]; then
    rm -f "$DW_MUTEX_DIR/HOLDER"
    rmdir "$DW_MUTEX_DIR" 2>/dev/null
    echo "25565 mutex released by $DW_MUTEX_ME"
  fi
  DW_MUTEX_ME=""
}

# dw_mutex_release_named <holder-name>
# Cross-shell release: frees the lock ONLY if HOLDER matches the given name
# exactly. This is how a coordinator releases a lock it took in an earlier
# shell (agent tool calls never share shell state). Releasing
# owner-play-session is additionally guarded: it is refused while ANY container
# still publishes host 25565 — the name is sacred because a HUMAN may be behind
# it, so the end of their session must be verifiable, not assumed. The PORT is
# the resource, never a container name — `tools/playtest-server.sh` binds it
# under a name of its own.
dw_mutex_release_named() {
  local name="${1:?dw_mutex_release_named needs the holder name to release}"
  local holder; holder="$(dw_mutex_holder)"
  if [ -z "$holder" ]; then
    echo "dw_mutex_release_named: lock is already free." >&2
    return 0
  fi
  if [ "$holder" != "$name" ]; then
    echo "REFUSING: lock is held by '$holder', not '$name' — not touching it." >&2
    return 1
  fi
  if [ "$name" = "owner-play-session" ] && dw_mutex_port_bound; then
    echo "REFUSING: a container still publishes host 25565 — the owner may still be playing." >&2
    docker ps --format '  {{.Names}}  {{.Ports}}' 2>/dev/null | grep -E ':25565->' >&2
    echo "Stop that session first; only then may owner-play-session be released." >&2
    return 1
  fi
  rm -f "$DW_MUTEX_DIR/HOLDER"
  rmdir "$DW_MUTEX_DIR" 2>/dev/null
  echo "25565 mutex (holder '$name') released by name"
}
