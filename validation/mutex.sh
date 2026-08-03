#!/usr/bin/env bash
# The validation-stack mutex (owner incident, 2026-08-02).
#
# One Docker host, one port 25565, one `delvewright-server` container name — so
# exactly one agent or human may drive the validation stack at a time. This is
# the ONLY sanctioned way to take it.
#
#   source validation/mutex.sh
#   dw_mutex_acquire "my-name"   # exits non-zero if someone else holds it
#   trap dw_mutex_release EXIT   # release is idempotent and never steals
#
# ## Why this exists
#
# A hand-rolled waiter took the lock with `mkdir` and then guarded with
# `if [ ! -d "$LOCK" ]; then exit 1; fi`, meaning "bail if I did not get it".
# That guard tests **directory existence**, which is true precisely when SOMEONE
# ELSE holds the lock — the case it was written to catch. When the acquire loop
# timed out against a lock the owner held, the script sailed past the guard,
# ran `docker compose --profile play up`, and — because `compose.yaml` pins
# `container_name: delvewright-server` — its cleanup `docker compose down -v`
# destroyed the OWNER'S live play session and its world volume mid-playtest.
#
# Three rules fall out of that, and they are enforced here rather than left to
# each caller to remember:
#
# 1. **Acquisition is a return value, never an inference from the filesystem.**
#    `mkdir` succeeding is the only proof of ownership.
# 2. **A holder must be named.** The lock directory carries a `HOLDER` file so
#    anyone can see whose session it is. `owner-play-session` is sacred: a human
#    is playing, and no automation may touch Docker at all, however stale the
#    lock looks.
# 3. **Release only what you took.** `dw_mutex_release` no-ops unless the HOLDER
#    is us, so a crashed script can never free someone else's lock — and no
#    teardown trap is ever installed before acquisition succeeds.
#
# ## Worker isolation (the other half of the rule)
#
# The mutex serialises access; **isolation** is what makes a mistake survivable.
# The default compose project plus `compose.yaml`'s pinned
# `container_name: delvewright-server` and its `127.0.0.1:25565` binding mean
# every caller aims at the SAME container and the SAME port — which is why one
# stray `down -v` could reach across into a human's session at all.
#
# Any agent/worker live-server work MUST therefore:
#
# 1. run in its **own compose project** — `docker compose -p dw-worker-<unique>`
#    (or a plain `docker run` with a unique `--name`, as the warden spike does);
# 2. publish **no host binding on 25565** — reach the server over the compose
#    network, via `docker exec … rcon-cli`, or on a distinct high port. 25565 is
#    reserved for the owner's client;
# 3. tear down **only its own project** — `docker compose -p dw-worker-<unique>
#    down -v`, never a bare `docker compose down`, and never `docker rm` a
#    container it did not create.
#
# Held together: the mutex means only one of you runs at a time, and isolation
# means that even when someone gets it wrong, what they tear down is provably
# their own.
set -uo pipefail

DW_MUTEX_DIR="${DW_MUTEX_DIR:-/private/tmp/delvewright-validation.lock.d}"
DW_MUTEX_ME=""

# The current holder's name, or empty if the lock is free.
dw_mutex_holder() {
  [ -d "$DW_MUTEX_DIR" ] || return 0
  cut -d' ' -f1 "$DW_MUTEX_DIR/HOLDER" 2>/dev/null || echo "unknown-holder"
}

# Hard stop for any Docker work: refuse while a human is playing.
dw_mutex_assert_not_owner_session() {
  if [ "$(dw_mutex_holder)" = "owner-play-session" ]; then
    echo "REFUSING: the validation mutex is held by owner-play-session — a human is" >&2
    echo "playing on delvewright-server. Do not touch Docker. Wait to be told it is free." >&2
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
      echo "mutex acquired by $me"
      return 0
    fi
    local holder; holder="$(dw_mutex_holder)"
    if [ "$holder" = "owner-play-session" ]; then
      echo "mutex held by owner-play-session — refusing to wait or steal." >&2
      return 1
    fi
    if [ "$waited" -ge "$wait_s" ]; then
      echo "mutex held by '${holder:-unknown}' after ${waited}s — NOT acquired, doing nothing." >&2
      return 1
    fi
    sleep 5
    waited=$((waited + 5))
  done
}

# Idempotent; releases only a lock this process actually took.
dw_mutex_release() {
  [ -n "$DW_MUTEX_ME" ] || return 0
  if [ "$(dw_mutex_holder)" = "$DW_MUTEX_ME" ]; then
    rm -f "$DW_MUTEX_DIR/HOLDER"
    rmdir "$DW_MUTEX_DIR" 2>/dev/null
    echo "mutex released by $DW_MUTEX_ME"
  fi
  DW_MUTEX_ME=""
}
