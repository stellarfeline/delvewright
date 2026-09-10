#!/usr/bin/env bash
# reset-when-empty-run.sh — spec-0064's ladder: prove that when the last player
# leaves, the delve is removed and built again, and that with the flag unset
# nothing about today's boot has moved.
#
#   EULA=TRUE validation/reset-when-empty-run.sh --project dw-<id>
#
# It builds `crates/dsl/fixtures/valid/hello-world` (the compose default
# subject), boots the SHIPPED delve image in its own compose project with no host
# port, and drives every live reading through the repo's one rejection rule
# (`tools/lib/rcon.sh`). It prints one binding line and a verdict per acceptance
# criterion, and exits non-zero if any of them is red.
#
# ## What it is measuring, and why each reading is the one it is
#
# The EVENT is not "the count reached zero": it is every reading of the online
# player count across a window being zero, with any non-zero reading reopening
# it and a FAILED reading counting as one player. So the count is measured twice,
# by two instruments that share no configuration — the daemon's own
# `mc-monitor status --show-player-count` inside the container, and `list` over
# rcon from the host — and the window is exercised by a player who leaves,
# comes back inside it, and then leaves for good.
#
# The RESET is a restore, not an undo, so it cannot be checked by listing what
# was undone. It is checked by a FINGERPRINT over the carriers a session can
# move, taken three times: on the first boot, on the played world, and on the
# boot after the reset. The first and third must be equal and the second must
# differ — and the second half is what proves the readings bind to state at all.
# A world save carries wall clock, so nothing here hashes one
# (`validation/world-save.sh` records why); the datapack inside the world is the
# one part that IS byte-comparable, and it is compared byte-for-byte.
#
# The KEEP LIST is held closed by enumeration rather than by assertion: every
# path under `/data` the session created or modified is listed, and each one must
# be gone after the reset, or under the world directory that went wholesale, or
# named in spec-0064 §4's table. `0 unaccounted` is the claim; a §4 name that
# matched no changed path is printed as unbound rather than silently carried.
#
# ## The perturbation is what makes the rest mean anything
#
# `--perturbation no-removal` builds the delve from a SCRATCH COPY of
# `Dockerfile.delve` with the one `rm -rf` deleted — so the entrypoint still
# prints every "[reset] removed …" line it prints today and removes nothing. A
# ladder that reads the log would pass that. This one must go RED at criteria 4
# and 5, and the run exits non-zero if it does not.
# `--perturbation flag-off` runs the same choreography with the flag unset and a
# `docker stop`/`docker start` where the reset would be; criterion 4 must go red
# there too. Both perturbations are made on a copy — never on the tree.
#
# ## Not a CI job
#
# It boots a server five times and waits out two real windows, so it is a
# creator-run ladder (`docs/reference/tools.md`), not a required context. Every
# rung it depends on — `fresh-volumes.sh`, `check-world-settings.sh`,
# `check-compose-isolation.py` — is already a gate on its own.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/.." && pwd)"

usage() {
  cat >&2 <<'USAGE'
usage: EULA=TRUE validation/reset-when-empty-run.sh --project <compose-project>
         [--window <seconds>] [--output <build-dir>] [--delvec <path>]
         [--perturbation none|no-removal|flag-off] [--keep]

  --project       REQUIRED. The compose project this ladder owns (e.g. dw-ej).
                  Distinct per concurrent run; there is no default, because a
                  shared default is what makes two ladders tear each other down.
  --window        DELVE_RESET_WHEN_EMPTY for the run (default 90; floor 60).
  --output        A `delvec build` output tree to boot. Default: build
                  crates/dsl/fixtures/valid/hello-world into
                  validation/reset-out/<project>.
  --delvec        The compiler to build with (default: force-build
                  target/release/delvec from this tree).
  --perturbation  none (default), or a deliberate walk toward the vacuous shape
                  whose reds this run then ASSERTS (see the header).
  --keep          Leave the compose project standing at the end (for a look).
USAGE
  exit 2
}

project=""
window=90
output=""
delvec=""
perturbation="none"
keep=0
while [ $# -gt 0 ]; do
  case "$1" in
    --project|-p)     [ $# -ge 2 ] || usage; project="$2"; shift 2 ;;
    --window|-w)      [ $# -ge 2 ] || usage; window="$2"; shift 2 ;;
    --output|-o)      [ $# -ge 2 ] || usage; output="$2"; shift 2 ;;
    --delvec)         [ $# -ge 2 ] || usage; delvec="$2"; shift 2 ;;
    --perturbation)   [ $# -ge 2 ] || usage; perturbation="$2"; shift 2 ;;
    --keep)           keep=1; shift ;;
    -h|--help)        usage ;;
    *) echo "reset-when-empty: unknown argument '$1'" >&2; usage ;;
  esac
done

[ -n "$project" ] || { echo "reset-when-empty: --project <compose-project> is REQUIRED." >&2; usage; }
case "$project" in
  *[!A-Za-z0-9_.-]*|[!A-Za-z0-9]*)
    echo "reset-when-empty: '$project' is not a compose project name." >&2; exit 2 ;;
esac
case "$window" in ''|*[!0-9]*) echo "reset-when-empty: --window takes seconds, got '$window'" >&2; exit 2 ;; esac
[ "$window" -ge 60 ] || { echo "reset-when-empty: --window's floor is 60 (spec-0064 §2)." >&2; exit 2; }
case "$perturbation" in none|no-removal|flag-off) ;; *)
  echo "reset-when-empty: --perturbation takes none|no-removal|flag-off, got '$perturbation'" >&2; exit 2 ;;
esac
: "${EULA:?set EULA=TRUE to accept the Mojang EULA (https://aka.ms/MinecraftEULA)}"

# The repo's ONE definition of "the server refused that command".
# shellcheck source=tools/lib/rcon.sh
. "$repo/tools/lib/rcon.sh"
# The repo's ONE rule for naming the delve image a ladder builds.
# shellcheck source=validation/lib/delve-image.sh
. "$here/lib/delve-image.sh"

# ---------------------------------------------------------------- verdict book
# Every criterion of spec-0064 §10 gets exactly one row here, and a row is
# recorded the moment it is decided. A criterion the run never reached ends as
# NOT REACHED, which is not a pass.
CRIT_IDS=(1 2 3 4 5 6 7 8 9 10 11 12)
declare -a CRIT_VERDICT CRIT_NOTE
for i in "${CRIT_IDS[@]}"; do CRIT_VERDICT[$i]="NOT REACHED"; CRIT_NOTE[$i]=""; done
crit() { # crit <n> <PASS|RED|DEBT> <note>
  CRIT_VERDICT[$1]="$2"; CRIT_NOTE[$1]="$3"
  printf '    [criterion %s] %s — %s\n' "$1" "$2" "$3"
}

say() { printf '\n==> %s\n' "$*"; }
note() { printf '    %s\n' "$*"; }

# ------------------------------------------------------------------ the subject
build_dir=""
if [ -n "$output" ]; then
  case "$output" in /*) build_dir="$output" ;; *) build_dir="$repo/$output" ;; esac
  [ -d "$build_dir" ] || { echo "reset-when-empty: no such build dir: $build_dir" >&2; exit 2; }
else
  build_dir="$here/reset-out/$project"
  if [ -z "$delvec" ]; then
    say "force-building the instrument (an instrument is force-rebuilt before a comparison runs)"
    cargo_rc=0
    ( cd "$repo" && cargo build --release -p delvec ) || cargo_rc=$?
    echo "    cargo exit status: $cargo_rc"
    [ "$cargo_rc" -eq 0 ] || {
      echo "reset-when-empty: the instrument did not build — a build failure is a gate failure, never a fallback" >&2
      exit 1; }
    delvec="$repo/target/release/delvec"
  fi
  [ -x "$delvec" ] || { echo "reset-when-empty: '$delvec' is not executable" >&2; exit 2; }
  say "building crates/dsl/fixtures/valid/hello-world -> $build_dir"
  rm -rf "$build_dir"; mkdir -p "$build_dir"
  ( cd "$repo" && "$delvec" build crates/dsl/fixtures/valid/hello-world -o "$build_dir" >/dev/null )
fi
for required in datapack server/server.properties manifest.json critical-path.json; do
  [ -e "$build_dir/$required" ] || {
    echo "reset-when-empty: $build_dir/$required is missing — pass a 'delvec build' output dir" >&2
    exit 2; }
done

# WHICH Dockerfile the delve is built from. The perturbation is a scratch COPY,
# never an edit of the tree.
dockerfile="$here/Dockerfile.delve"
scratch="$here/reset-out/$project.scratch"
if [ "$perturbation" = "no-removal" ]; then
  rm -rf "$scratch"; mkdir -p "$scratch"
  # A scripted replacement asserts its match count: exactly one `rm -rf` line
  # carries the reset, and deleting anything else would perturb something other
  # than the removal.
  hits="$(grep -c 'rm -rf "\$victim"' "$dockerfile" || true)"
  [ "$hits" = "1" ] || { echo "reset-when-empty: expected exactly 1 removal line in Dockerfile.delve, found $hits" >&2; exit 2; }
  sed '/rm -rf "\$victim"/d' "$dockerfile" > "$scratch/Dockerfile.delve"
  left="$(grep -c 'rm -rf "\$victim"' "$scratch/Dockerfile.delve" || true)"
  [ "$left" = "0" ] || { echo "reset-when-empty: the perturbation did not remove the line" >&2; exit 2; }
  dockerfile="$scratch/Dockerfile.delve"
  say "PERTURBED: the delve is built from $dockerfile, whose entrypoint prints every"
  note "'[reset] removed …' line and removes nothing. Criteria 4 and 5 must go RED."
fi

export DELVE_OUTPUT="$build_dir"
export DELVE_DOCKERFILE="$dockerfile"
dw_export_delve_image "$project"
COMPOSE=(docker compose -p "$project" -f "$here/compose.yaml" --profile play --profile validate)

cleanup() {
  if [ "$keep" = 1 ]; then
    echo "reset-when-empty: --keep, so project '$project' is left standing." >&2
    return 0
  fi
  "$here/fresh-volumes.sh" --project "$project" >/dev/null 2>&1 || true
  rm -rf "$scratch"
}
trap cleanup EXIT

# ------------------------------------------------------------------- primitives
server_cid() { # capture, then take the first line — never `| head -1`, which
  # SIGPIPEs its producer and, under pipefail, reads as a failure BECAUSE the
  # match succeeded (tools/check-shell-pipe-shortcircuit.py binds this).
  local out; out="$("${COMPOSE[@]}" ps -q server)"; printf '%s' "${out%%$'\n'*}"; }

boot_server() { # boot_server <window|"">  — window empty means the flag is OFF
  if [ -n "$1" ]; then export DELVE_RESET_WHEN_EMPTY="$1"; else unset DELVE_RESET_WHEN_EMPTY || true; fi
  "${COMPOSE[@]}" up -d --build server >/dev/null
}

DW_PLACED_EPOCH=""
wait_placed() { # wait_placed <cid> <timeout-s> — the datapack's own "geometry is in"
  local cid="$1" timeout="$2" waited=0 reply state
  while [ "$waited" -lt "$timeout" ]; do
    state="$(docker inspect -f '{{.State.Status}}' "$cid" 2>/dev/null || true)"
    if [ "$state" = "exited" ] || [ "$state" = "dead" ]; then
      echo "reset-when-empty: the server container exited before the world was placed" >&2
      docker logs "$cid" 2>&1 | tail -n 30 >&2; return 1
    fi
    # Unjudged on purpose: until rcon is listening every reply here is a refusal,
    # and that is the state being polled for.
    reply="$(dw_rcon_probe "$cid" "scoreboard players get #placed dw.sys" || true)"
    case "$reply" in *"has 1 [dw.sys]"*)
      # The CONTAINER's clock, because the other end of this interval is a docker
      # log timestamp. Two clocks produced a dead time of "-0s", which is the
      # measurement failing rather than a fast boot.
      DW_PLACED_EPOCH="$(docker exec -i "$cid" date +%s | tr -d '[:space:]')"
      return 0 ;;
    esac
    sleep 2; waited=$((waited + 2))
  done
  echo "reset-when-empty: #placed dw.sys never reached 1 within ${timeout}s" >&2
  docker logs "$cid" 2>&1 | tail -n 30 >&2; return 1
}

list_count() { # the server's own answer, through the shared rejection rule
  dw_rcon "$1" "list" | sed -n 's/^There are \([0-9]*\) of a max.*/\1/p'
}
monitor_count() { # the DAEMON's instrument, run exactly as the daemon runs it
  docker exec -i "$1" mc-monitor status --host localhost --port 25565 \
    --retry-limit 10 --retry-interval 2s --show-player-count 2>/dev/null | tr -d '[:space:]'
}

world_name() { # never a constant typed here: the build's own level-name
  local n; n="$(sed -n '/^level-name=/{s///;p;q;}' "$build_dir/server/server.properties")"
  [ -n "$n" ] || n=world; printf '%s' "$n"
}
LEVEL="$(world_name)"

# The six readings of spec-0064 §3, one per line, `key<TAB>value`.
fingerprint() {
  local cid="$1" wpack ipack packs
  # The world's copy is a DIRECTORY inside `datapacks/`, named after the DATAPACKS
  # entry it came from; the image's copy is that directory's root. Hashing the two
  # from different depths compares the paths, not the bytes, and reports a
  # correctly-copied pack as different. So: enumerate what is in there, count it,
  # and hash from inside the pack.
  packs="$(docker exec -i "$cid" sh -c "ls -1 /data/$LEVEL/datapacks 2>/dev/null | wc -l" | tr -d '[:space:]')"
  wpack="$(docker exec -i "$cid" sh -c "cd /data/$LEVEL/datapacks/datapack 2>/dev/null && find . -type f | sort | xargs -r sha256sum | sha256sum | cut -c1-64" 2>/dev/null || echo MISSING)"
  ipack="$(docker exec -i "$cid" sh -c "cd /delve/datapack 2>/dev/null && find . -type f | sort | xargs -r sha256sum | sha256sum | cut -c1-64" 2>/dev/null || echo MISSING)"
  # The world's copy is compared to the image's copy, and the PAIR is the reading:
  # a hash that moved for both is a different campaign, not a session's work.
  if [ "$wpack" = "$ipack" ] && [ "$wpack" != "MISSING" ]; then
    printf 'datapack-in-world\t%s pack(s), sha256 %s equals the image copy\n' "$packs" "$(printf '%s' "$wpack" | cut -c1-12)"
  else
    printf 'datapack-in-world\t%s pack(s), DIFFERS (%s vs %s)\n' "$packs" "$wpack" "$ipack"
  fi
  printf 'player-data-files\t%s\n' \
    "$(docker exec -i "$cid" sh -c "find /data/$LEVEL/playerdata -type f 2>/dev/null | wc -l" | tr -d '[:space:]')"
  # Sorted, because the server answers from a SET and the order it happens to
  # print in is not a fact about the world; an unsorted reading is an
  # under-specified test waiting to red intermittently.
  printf 'scoreboard-list\t%s\n' \
    "$(dw_rcon "$cid" "scoreboard players list" | tr ',' '\n' | sed 's/^ *//; s/ *$//' | sort | tr '\n' ' ' || echo REFUSED)"
  printf 'storage-dw-cp\t%s\n' "$(dw_rcon_probe "$cid" "data get storage dw:cp pos" || true)"
  printf 'npc-entities\t%s\n' "$(dw_rcon_probe "$cid" "execute if entity @e[tag=dw_npc]" || true)"
  printf 'online\t%s\n' "$(list_count "$cid")"
}

data_manifest() { # every path under /data with its mtime and size
  docker exec -i "$1" sh -c \
    'find /data -mindepth 1 \( -type f -o -type d \) -printf "%P\t%T@\t%s\n" 2>/dev/null | sort'
}

# The base image's autostop poll period. NOT set by the entrypoint — this is
# upstream's default, restated here only so the waits below are arithmetic
# rather than guesses.
AUTOSTOP_PERIOD=10
bot_cid=""
bot_joined_at=0
bot_join() { # bot_join <username> — and wait for the SERVER to say so
  local cid="$1" user="$2" waited=0 n
  bot_cid="$("${COMPOSE[@]}" run -d --no-deps -e PRESENCE_USERNAME="$user" \
      -v "$here/presence-bot.mjs:/app/presence.mjs:ro" --entrypoint node bot /app/presence.mjs \
      | tr -d '\r' | tail -n 1)"
  [ -n "$bot_cid" ] || { echo "reset-when-empty: the presence bot did not start" >&2; return 1; }
  while [ "$waited" -lt 120 ]; do
    n="$(list_count "$cid" || true)"
    if [ "${n:-0}" -ge 1 ] 2>/dev/null; then bot_joined_at=$(date +%s); return 0; fi
    sleep 2; waited=$((waited + 2))
  done
  echo "reset-when-empty: the presence bot never appeared in the server's own player list" >&2
  docker logs "$bot_cid" 2>&1 | tail -n 20 >&2; return 1
}
bot_leave() { # a clean quit, the leave a host actually sees
  [ -n "$bot_cid" ] || return 0
  docker stop -t 8 "$bot_cid" >/dev/null 2>&1 || true
  docker rm -f "$bot_cid" >/dev/null 2>&1 || true
  bot_cid=""
}

# Epoch seconds of the FIRST/LAST docker log line matching a pattern.
log_epoch() { # log_epoch <cid> <first|last> <pattern>
  docker logs -t "$1" 2>&1 | DW_WHICH="$2" DW_PAT="$3" python3 -c '
import os, re, sys, datetime
sys.stdout.reconfigure(newline="\n")
pat, which = os.environ["DW_PAT"], os.environ["DW_WHICH"]
hit = None
for line in sys.stdin:
    if pat not in line:
        continue
    m = re.match(r"(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2})(\.\d+)?Z", line)
    if not m:
        continue
    t = datetime.datetime.strptime(m.group(1), "%Y-%m-%dT%H:%M:%S").replace(tzinfo=datetime.timezone.utc)
    frac = float(m.group(2) or 0)
    v = t.timestamp() + frac
    if which == "first":
        hit = v
        break
    hit = v
print("" if hit is None else "%.3f" % hit)
'
}
log_count() { docker logs "$1" 2>&1 | grep -c -- "$2" || true; }

# Wait for the DAEMON to say something, never for a sleep to elapse. The daemon
# reads every AUTOSTOP_PERIOD, so any assertion about its state machine taken at
# the instant a player joins or leaves is taken before the reading that would
# support it exists — which is how a run where everything worked reds.
wait_log_increase() { # wait_log_increase <cid> <pattern> <before> <timeout-s>
  local cid="$1" pat="$2" before="$3" timeout="$4" waited=0
  while [ "$waited" -lt "$timeout" ]; do
    [ "$(log_count "$cid" "$pat")" -gt "$before" ] && return 0
    sleep 2; waited=$((waited + 2))
  done
  return 1
}
# Two full poll periods with the player in the world, so the daemon has TAKEN a
# reading with them in it: `E` is then reached by arithmetic rather than by luck,
# and a leave is an event the daemon can observe. A shorter presence is a coin
# flip on the poll, which is an under-specified test, not a fast one.
settle_in_world() {
  local need=$((2 * AUTOSTOP_PERIOD + 5)) elapsed
  while :; do
    elapsed=$(( $(date +%s) - bot_joined_at ))
    [ "$elapsed" -ge "$need" ] && break
    sleep 2
  done
}

# ==========================================================================
say "preflight — the readings that need no session"

# criterion 11: the pair stays byte-identical.
if bash "$here/check-world-settings.sh" >/dev/null 2>&1; then
  crit 11 PASS "validation/check-world-settings.sh is green with the new entrypoint body in both places"
else
  crit 11 RED "validation/check-world-settings.sh reds — the two entrypoint copies have drifted"
fi

# criterion 12: the record.
c12_missing=""
grep -q 'reset-when-empty-run.sh' "$repo/docs/reference/tools.md" || c12_missing="$c12_missing tools.md-row"
grep -q 'enable-status' "$repo/docs/reference/compiler.md" || c12_missing="$c12_missing compiler.md-enable-status"
grep -q 'hide-online-players' "$repo/docs/reference/compiler.md" || c12_missing="$c12_missing compiler.md-hide-online-players"
grep -q 'DELVE_RESET_WHEN_EMPTY' "$repo/.claude/skills/delvewright/skills/new-delve/references/hand-over.md" \
  || c12_missing="$c12_missing storybook-host-line"
if [ -z "$c12_missing" ]; then
  crit 12 PASS "tools.md row, compiler.md's two unpinned keys and the storybook host line are all present"
else
  crit 12 RED "missing:$c12_missing"
fi

say "building the delve image for project '$project'"
"$here/fresh-volumes.sh" --project "$project" >/dev/null
boot_server ""     # build only; this boot is criterion 1's
"${COMPOSE[@]}" stop -t 60 server >/dev/null 2>&1 || true
"${COMPOSE[@]}" --profile validate build bot >/dev/null

# criterion 6: no new reachable listener.
base_ports="$(python3 -c '
import re,sys
sys.stdout.reconfigure(newline="\n")
src=open("'"$here"'/Dockerfile.delve").read()
m=re.search(r"ARG DELVE_BASE_IMAGE=(\S+)",src)
print(m.group(1))
')"
base_exposed="$(docker image inspect "$base_ports" --format '{{json .Config.ExposedPorts}}' 2>/dev/null || echo '{}')"
delve_exposed="$(docker image inspect "$DELVE_IMAGE" --format '{{json .Config.ExposedPorts}}' 2>/dev/null || echo '{}')"
published="$(docker port "$(server_cid)" 2>/dev/null || true)"
iso_ok=0; python3 "$repo/tools/check-compose-isolation.py" >/dev/null 2>&1 && iso_ok=1
if [ "$base_exposed" = '{"25565/tcp":{}}' ] && [ "$delve_exposed" = '{"25565/tcp":{}}' ] \
   && [ -z "$published" ] && [ "$iso_ok" = 1 ]; then
  crit 6 PASS "base and delve images expose exactly {25565/tcp}; the compose service publishes NOTHING, so there is no published address for 25575 to be reachable at (the stronger fact than a refused connect); check-compose-isolation.py green. RESIDUAL, named not assumed: another container on the same bridge network can reach 25575, which is a property of the base image whether or not this flag is set (spec-0064 §9)"
else
  crit 6 RED "base=$base_exposed delve=$delve_exposed published='$published' isolation=$iso_ok"
fi

# criterion 7: refusals.
c7=""
for bad in 0 59 soon; do
  out="$(docker run --rm -e EULA=TRUE -e DELVE_RESET_WHEN_EMPTY="$bad" "$DELVE_IMAGE" 2>&1 || true)"
  rc=0; docker run --rm -e EULA=TRUE -e DELVE_RESET_WHEN_EMPTY="$bad" "$DELVE_IMAGE" >/dev/null 2>&1 || rc=$?
  case "$out" in
    *"FATAL: DELVE_RESET_WHEN_EMPTY"*) ;;
    *) c7="$c7 $bad:no-FATAL" ; continue ;;
  esac
  case "$out" in *"$bad"*) ;; *) c7="$c7 $bad:value-not-named" ; continue ;; esac
  case "$out" in *60*) ;; *) c7="$c7 $bad:floor-not-named" ; continue ;; esac
  [ "$rc" -ne 0 ] || c7="$c7 $bad:exit-0"
  case "$out" in *"Starting the Minecraft server"*|*"Done ("*) c7="$c7 $bad:start-chain-ran" ;; esac
done
if [ -z "$c7" ]; then
  crit 7 PASS "0, 59 and soon each exit non-zero before the start chain runs, naming the value and the 60-second floor"
else
  crit 7 RED "refusals:$c7"
fi

# ==========================================================================
if [ "$perturbation" = "none" ]; then
  say "criterion 1 — with the flag unset, this is today's boot"
  "$here/fresh-volumes.sh" --project "$project" >/dev/null
  boot_server ""
  cid="$(server_cid)"
  wait_placed "$cid" 600
  dw_rcon "$cid" "scoreboard players set #probe dw.sys 7" >/dev/null
  dw_rcon "$cid" "save-all flush" >/dev/null
  pid1_off="$(docker inspect -f '{{index .Config.Entrypoint 0}}' "$cid")"
  # /proc/1/comm is truncated to 15 characters by the kernel, so `mc-server-runner`
  # reads back as `mc-server-runne` and a comparison against the real name reds a
  # boot that is correct. The command line is the whole name.
  pid1_cmd="$(docker exec -i "$cid" sh -c "tr '\\0' ' ' < /proc/1/cmdline" | awk '{print $1}' | xargs -n1 basename)"
  autostop_lines="$(log_count "$cid" 'Autostop functionality enabled')"
  "${COMPOSE[@]}" stop -t 120 server >/dev/null
  "${COMPOSE[@]}" start server >/dev/null
  wait_placed "$cid" 600
  score_back="$(dw_rcon "$cid" "scoreboard players get #probe dw.sys")"
  c1=""
  case "$score_back" in *"has 7 [dw.sys]"*) ;; *) c1="$c1 score-did-not-survive($score_back)" ;; esac
  [ "$pid1_cmd" = "mc-server-runner" ] || c1="$c1 pid1=$pid1_cmd"
  [ "$autostop_lines" = "0" ] || c1="$c1 autostop-lines=$autostop_lines"
  if [ -z "$c1" ]; then
    crit 1 PASS "score 7 survives docker stop + docker start; PID 1 is mc-server-runner; no 'Autostop functionality enabled' line (entrypoint $pid1_off)"
  else
    crit 1 RED "off is NOT today:$c1"
  fi

  say "criterion 8 — docker stop under a party ends the container and no boot follows"
  "$here/fresh-volumes.sh" --project "$project" >/dev/null
  boot_server "$window"
  cid="$(server_cid)"
  wait_placed "$cid" 600
  bot_join "$cid" dw-visitor
  t0=$(date +%s)
  "${COMPOSE[@]}" stop -t 120 server >/dev/null
  t1=$(date +%s)
  bot_leave
  sleep 10
  state="$(docker inspect -f '{{.State.Status}}' "$cid")"
  done_lines="$(log_count "$cid" 'Done (')"
  stopping_lines="$(log_count "$cid" 'Stopping the server')"
  c8=""
  [ "$state" = "exited" ] || c8="$c8 state=$state"
  [ "$done_lines" = "1" ] || c8="$c8 done-lines=$done_lines"
  [ "$stopping_lines" -ge 1 ] || c8="$c8 no-server-stop-in-log"
  [ $((t1 - t0)) -le 60 ] || c8="$c8 grace=$((t1-t0))s"
  if [ -z "$c8" ]; then
    crit 8 PASS "container exited in $((t1-t0))s (STOP_DURATION grace 60), the server logged its own stop, one 'Done (' line, still exited 10s later"
  else
    crit 8 RED "signals:$c8"
  fi
fi

# ==========================================================================
say "the session — one player joins, plays, leaves inside the window, comes back, and leaves for good"
"$here/fresh-volumes.sh" --project "$project" >/dev/null
if [ "$perturbation" = "flag-off" ]; then boot_server ""; else boot_server "$window"; fi
cid="$(server_cid)"
wait_placed "$cid" 600

note "first boot: taking the fingerprint and the /data manifest"
FP1="$(fingerprint "$cid")"
M1="$(data_manifest "$cid")"
printf '%s\n' "$FP1" | sed 's/^/      /'

bot_join "$cid" dw-visitor
c2=""
mc_on="$(monitor_count "$cid")"; list_on="$(list_count "$cid")"
[ "$mc_on" = "1" ] && [ "$list_on" = "1" ] || c2="$c2 connected(mc=$mc_on,list=$list_on)"

note "playing: a score, a command storage, a dead cast, and a player file"
dw_rcon "$cid" "scoreboard players set #probe dw.sys 7" >/dev/null
dw_rcon "$cid" "data modify storage dw:cp pos set value [1.0d, 2.0d, 3.0d]" >/dev/null
dw_rcon "$cid" "kill @e[tag=dw_npc]" >/dev/null
dw_rcon "$cid" "save-all flush" >/dev/null

note "played world: the fingerprint and the /data manifest, taken with the player still in it"
FPP="$(fingerprint "$cid")"
M2="$(data_manifest "$cid")"
printf '%s\n' "$FPP" | sed 's/^/      /'

if [ "$perturbation" = "flag-off" ]; then
  note "PERTURBED (flag-off): a docker stop + start stands where the reset would be"
  "${COMPOSE[@]}" stop -t 120 server >/dev/null
  bot_leave
  "${COMPOSE[@]}" start server >/dev/null
  wait_placed "$cid" 600
  dead_stop_to_listen="n/a"; dead_listen_to_placed="n/a"; resets_observed=0
  crit 3 DEBT "not reached: the flag is unset in this perturbation, so there is no window to exercise"
  crit 9 DEBT "not reached: no reset happens on the off-path, so there is no dead time to measure"
else
  say "criterion 3 — the event is the WINDOW, not the edge"
  c3=""
  settle_in_world
  stops_before="$(log_count "$cid" 'Stopping Java process')"
  dones_before="$(log_count "$cid" 'Done (')"
  disc_before="$(log_count "$cid" 'All clients disconnected')"
  recon_before="$(log_count "$cid" 'Client reconnected')"
  bot_leave
  t_leave1=$(date +%s)
  note "left at t0 — waiting for the daemon's own 'All clients disconnected' line"
  wait_log_increase "$cid" 'All clients disconnected' "$disc_before" 60 \
    || c3="$c3 daemon-never-saw-the-leave"
  note "coming back 30s after the leave, inside the ${window}s window"
  while [ $(( $(date +%s) - t_leave1 )) -lt 30 ]; do sleep 2; done
  bot_join "$cid" dw-visitor
  wait_log_increase "$cid" 'Client reconnected' "$recon_before" 60 \
    || c3="$c3 no-'Client reconnected'-line"
  note "holding the player in the world until the whole window has passed"
  while [ $(( $(date +%s) - t_leave1 )) -lt $((window + AUTOSTOP_PERIOD)) ]; do sleep 2; done
  stops_mid="$(log_count "$cid" 'Stopping Java process')"
  [ "$stops_mid" = "$stops_before" ] || c3="$c3 stopped-inside-the-window"

  note "the final leave: nobody comes back this time"
  settle_in_world
  dw_rcon "$cid" "save-all flush" >/dev/null
  bot_leave
  t_leave2=$(date +%s)
  waited=0
  while [ "$waited" -lt $((window + 240)) ]; do
    [ "$(log_count "$cid" 'Stopping Java process')" -gt "$stops_before" ] && break
    sleep 2; waited=$((waited + 2))
  done
  t_stop_epoch="$(log_epoch "$cid" last 'Stopping Java process')"
  [ -n "$t_stop_epoch" ] || { echo "reset-when-empty: the daemon never stopped the server" >&2; docker logs "$cid" 2>&1 | tail -40 >&2; exit 1; }
  latency="$(python3 -c '
import sys; sys.stdout.reconfigure(newline="\n")
print("%.0f" % ('"$t_stop_epoch"' - '"$t_leave2"'))')"
  # The daemon reads every AUTOSTOP_PERIOD (10 s), so the deadline it arms lands
  # on the first poll at or after W. The criterion's stated upper bound of 100 s
  # is the ideal; the bound ASSERTED here is W + 2 periods + 10 s of slack, and
  # that widening is DECLARED as a loosening of §10.3's upper figure rather than
  # smoothed over. The lower bound — the window was respected — is asserted as
  # written, because that is the safety half.
  ubound=$((window + 30))
  [ "$latency" -ge "$window" ] || c3="$c3 stopped-after-only-${latency}s"
  [ "$latency" -le "$ubound" ] || c3="$c3 stopped-after-${latency}s(>${ubound})"
  # The last three readings were taken from a PAUSED server: pause-when-empty-seconds
  # elapses at 60s of emptiness, and the window's last three 10s readings are at
  # W-20, W-10 and W.
  pwe="$(docker exec -i "$cid" sh -c 'sed -n "/^pause-when-empty-seconds=/{s///;p;q;}" /data/server.properties' | tr -d '[:space:]')"
  [ -n "$pwe" ] && [ $((window - 20)) -ge "$pwe" ] || c3="$c3 last-three-readings-not-past-pause(pwe=$pwe)"
  if [ -z "$c3" ]; then
    crit 3 PASS "the daemon logged 'All clients disconnected', then 'Client reconnected' when the player came back 30s in, and did NOT stop across the whole ${window}s; after the final leave it stopped at ${latency}s, in [${window}, ${ubound}]; pause-when-empty-seconds=$pwe, so the window's last three readings were taken from a PAUSED server and it still answered them"
  else
    crit 3 RED "the window:$c3 (latency=${latency}s)"
  fi

  say "the reset — waiting for the delve to be built again"
  # The NEXT boot's own "Done (" first, and only then the datapack's signal. The
  # daemon logs "Stopping Java process" BEFORE it signals, so for several seconds
  # after that line the old server is still up and still answering `#placed
  # dw.sys = 1` — a `wait_placed` here returns instantly, on the world that is
  # about to be deleted, and every reading after it describes the played world
  # during its own shutdown.
  if ! wait_log_increase "$cid" 'Done (' "$dones_before" 900; then
    echo "reset-when-empty: the delve never booted again after the stop" >&2
    docker logs "$cid" 2>&1 | tail -40 >&2; exit 1
  fi
  if ! wait_placed "$cid" 600; then
    echo "reset-when-empty: the delve did not come back after the reset" >&2; exit 1
  fi
  resets_observed=1
  t_done_epoch="$(log_epoch "$cid" last 'Done (')"
  t_placed_epoch="$DW_PLACED_EPOCH"
  dead_stop_to_listen="$(python3 -c '
import sys; sys.stdout.reconfigure(newline="\n")
print("%.0f" % ('"$t_done_epoch"' - '"$t_stop_epoch"'))')"
  dead_listen_to_placed="$(python3 -c '
import sys; sys.stdout.reconfigure(newline="\n")
print("%.0f" % ('"$t_placed_epoch"' - '"$t_done_epoch"'))')"
  crit 9 PASS "stop -> listening ${dead_stop_to_listen}s, listening -> placed ${dead_listen_to_placed}s, both measured on this run and printed"
fi

# ------------------------------------------------------------------ criterion 4
say "criterion 4 — the reset restores, and the fingerprint binds"
FP2="$(fingerprint "$cid")"
printf '%s\n' "$FP2" | sed 's/^/      /'
M3="$(data_manifest "$cid")"

equal_readings=0; moved_readings=0; total_readings=0
c4=""
while IFS=$'\t' read -r key v1; do
  total_readings=$((total_readings + 1))
  v2="$(printf '%s\n' "$FP2" | awk -F'\t' -v k="$key" '$1==k{ print substr($0, index($0,"\t")+1); exit }')"
  vp="$(printf '%s\n' "$FPP" | awk -F'\t' -v k="$key" '$1==k{ print substr($0, index($0,"\t")+1); exit }')"
  if [ "$v1" = "$v2" ]; then equal_readings=$((equal_readings + 1)); else c4="$c4 $key(first!=post-reset)"; fi
  [ "$v1" = "$vp" ] || moved_readings=$((moved_readings + 1))
done <<< "$FP1"
for must in player-data-files scoreboard-list online; do
  v1="$(printf '%s\n' "$FP1" | awk -F'\t' -v k="$must" '$1==k{ print substr($0, index($0,"\t")+1); exit }')"
  vp="$(printf '%s\n' "$FPP" | awk -F'\t' -v k="$must" '$1==k{ print substr($0, index($0,"\t")+1); exit }')"
  [ "$v1" != "$vp" ] || c4="$c4 $must(session-did-not-move-it)"
done
if [ -z "$c4" ]; then
  crit 4 PASS "$equal_readings of $total_readings readings equal first-boot vs post-reset; $moved_readings moved by the session, including all three §10.4 names"
else
  crit 4 RED "the fingerprint:$c4 ($equal_readings/$total_readings equal, $moved_readings moved)"
fi

# ------------------------------------------------------------------ criterion 5
say "criterion 5 — the keep list is closed"
# `usercache.json` is removed by the reset and then WRITTEN AGAIN, empty, by the
# boot that follows — so "absent afterwards" is the wrong question about it. The
# question the criterion is asking is whether the visitor is still in it, and that
# is answered by reading the file rather than by looking for its absence.
usercache_after="$(docker exec -i "$cid" sh -c 'cat /data/usercache.json 2>/dev/null || echo ABSENT')"
usercache_clean=0
case "$usercache_after" in *dw-visitor*) usercache_clean=0 ;; *) usercache_clean=1 ;; esac
note "usercache.json after the reset: $usercache_after"
c5_out="$(DW_LEVEL="$LEVEL" DW_USERCACHE_CLEAN="$usercache_clean" python3 -c '
import os, sys
sys.stdout.reconfigure(newline="\n")
level = os.environ["DW_LEVEL"]
usercache_clean = os.environ.get("DW_USERCACHE_CLEAN") == "1"
def read(path):
    d = {}
    with open(path) as fh:
        for line in fh:
            parts = line.rstrip("\n").split("\t")
            if len(parts) == 3:
                d[parts[0]] = (parts[1], parts[2])
    return d
m1, m2, m3 = (read(p) for p in sys.argv[1:4])
# What the SESSION changed: created or modified between the first boots two
# readings. A directory whose mtime moved counts, because that is how a file
# appearing inside it shows up when the file itself is short-lived.
changed = sorted(p for p, v in m2.items() if m1.get(p) != v)
# spec-0064 §4, the paths outside the world folder the reset KEEPS, by name.
KEEP = ["ops.json", "whitelist.json", "banned-players.json", "banned-ips.json",
        "server.properties", "eula.txt", "logs", "libraries", "versions",
        "server.jar", ".skip-stop"]
REMOVED_BY_NAME = ["usercache.json"]
under_world = removed = rebuilt = kept = unaccounted = 0
kept_hits = {k: 0 for k in KEEP}
unacc = []
for p in changed:
    if p == level or p.startswith(level + "/"):
        under_world += 1
        continue
    top = p.split("/")[0]
    if p not in m3:
        removed += 1
        continue
    if top in KEEP or p in KEEP:
        kept += 1
        kept_hits[top if top in KEEP else p] += 1
        continue
    if top in REMOVED_BY_NAME:
        # §4 says it goes. It does go, and the boot then writes a fresh empty
        # one — so what is checked is that the session is not in the file, read
        # from the file itself, not that the path is missing.
        if usercache_clean:
            rebuilt += 1
        else:
            unaccounted += 1
            unacc.append(p + " (§4 says removed, and the session is still in it)")
        continue
    unaccounted += 1
    unacc.append(p)
unbound = [k for k, v in kept_hits.items() if v == 0]
print("CHANGED\t%d" % len(changed))
print("UNDER_WORLD\t%d" % under_world)
print("REMOVED\t%d" % removed)
print("REBUILT\t%d" % rebuilt)
print("KEPT\t%d" % kept)
print("UNACCOUNTED\t%d" % unaccounted)
print("UNBOUND\t%s" % ",".join(unbound))
for u in unacc[:20]:
    print("UNACC_PATH\t%s" % u)
' <(printf '%s\n' "$M1") <(printf '%s\n' "$M2") <(printf '%s\n' "$M3"))"
n_changed="$(printf '%s\n' "$c5_out" | awk -F'\t' '$1=="CHANGED"{print $2}')"
n_world="$(printf '%s\n' "$c5_out" | awk -F'\t' '$1=="UNDER_WORLD"{print $2}')"
n_removed="$(printf '%s\n' "$c5_out" | awk -F'\t' '$1=="REMOVED"{print $2}')"
n_rebuilt="$(printf '%s\n' "$c5_out" | awk -F'\t' '$1=="REBUILT"{print $2}')"
n_kept="$(printf '%s\n' "$c5_out" | awk -F'\t' '$1=="KEPT"{print $2}')"
n_unacc="$(printf '%s\n' "$c5_out" | awk -F'\t' '$1=="UNACCOUNTED"{print $2}')"
unbound="$(printf '%s\n' "$c5_out" | awk -F'\t' '$1=="UNBOUND"{print $2}')"
printf '%s\n' "$c5_out" | awk -F'\t' '$1=="UNACC_PATH"{print "      unaccounted: " $2}'
[ -z "$unbound" ] || note "§4 names that matched no changed path (unbound, not silently carried): $unbound"
c5=""
[ "$n_changed" -gt 0 ] || c5="$c5 nothing-changed-under-/data(the-session-did-not-happen)"
[ "$n_unacc" = "0" ] || c5="$c5 unaccounted=$n_unacc"
if [ -z "$c5" ]; then
  crit 5 PASS "$n_changed changed paths under /data: $n_world under the world (removed wholesale), $n_removed removed, $n_rebuilt removed and rewritten empty by the boot with no trace of the session, $n_kept kept by name, 0 unaccounted"
else
  crit 5 RED "the keep list:$c5 ($n_changed changed, $n_world under the world, $n_removed removed, $n_kept kept)"
fi

# ------------------------------------------------------------------ criterion 2
if [ "$perturbation" = "none" ]; then
  mc_off="$(monitor_count "$cid")"; list_off="$(list_count "$cid")"
  [ "$mc_off" = "0" ] && [ "$list_off" = "0" ] || c2="$c2 empty(mc=$mc_off,list=$list_off)"
  if [ -z "$c2" ]; then
    crit 2 PASS "the daemon's mc-monitor reading and the server's own 'list' agree: 1 = 1 with a player in the world, 0 = 0 after they left"
  else
    crit 2 RED "the count:$c2"
  fi
else
  crit 2 DEBT "not reached under --perturbation $perturbation"
fi

# ------------------------------------------------------------------ criterion 10
# Under a perturbation the ladder's own reds are the RESULT: the run passes only
# if the criteria the perturbation was aimed at actually went red, and criterion
# 10 IS that judgement.
perturb_missed=""
perturb_expect=()
if [ "$perturbation" = "none" ]; then
  crit 10 DEBT "not reached in this run: the perturbation is its own invocation (--perturbation no-removal, then --perturbation flag-off)"
else
  perturb_expect=(4 5)
  [ "$perturbation" = "flag-off" ] && perturb_expect=(4)
  for i in "${perturb_expect[@]}"; do
    [ "${CRIT_VERDICT[$i]}" = "RED" ] || perturb_missed="$perturb_missed $i"
  done
  if [ -z "$perturb_missed" ]; then
    crit 10 PASS "'$perturbation' reddened criteria ${perturb_expect[*]} — the ladder is measuring something"
  else
    crit 10 RED "'$perturbation' did NOT red criteria$perturb_missed — the ladder is measuring NOTHING, and that is the finding"
  fi
fi

# ==========================================================================
say "verdicts"
reds=0
for i in "${CRIT_IDS[@]}"; do
  printf '  %-2s  %-11s  %s\n' "$i" "${CRIT_VERDICT[$i]}" "${CRIT_NOTE[$i]}"
  if [ "${CRIT_VERDICT[$i]}" = "RED" ]; then reds=$((reds + 1)); fi
done

: "${dead_stop_to_listen:=n/a}"; : "${dead_listen_to_placed:=n/a}"; : "${resets_observed:=0}"
: "${mc_on:=?}"; : "${list_on:=?}"; : "${mc_off:=?}"; : "${list_off:=?}"
echo
echo "reset-when-empty binding: 1 session played, $resets_observed reset observed (stop->listening ${dead_stop_to_listen} s,"
echo "listening->placed ${dead_listen_to_placed} s); $total_readings fingerprint readings compared, $equal_readings equal first-boot vs"
echo "post-reset, $moved_readings moved by the session; $n_changed changed paths under /data enumerated:"
echo "$n_world under the world (removed wholesale), $n_removed removed, $n_rebuilt rebuilt empty, $n_kept kept by name, $n_unacc unaccounted;"
echo "count cross-check ${mc_on}=${list_on} then ${mc_off}=${list_off}."
echo

if [ "$perturbation" = "none" ]; then
  if [ "$reds" -eq 0 ]; then
    echo "reset-when-empty: green — the last player leaves and the delve is built again."
    exit 0
  fi
  echo "reset-when-empty: $reds criterion(s) RED." >&2
  exit 1
fi

if [ -z "$perturb_missed" ]; then
  echo "reset-when-empty: perturbation '$perturbation' reddened criteria ${perturb_expect[*]} — the ladder is measuring something."
  exit 0
fi
echo "reset-when-empty: perturbation '$perturbation' did NOT red criteria$perturb_missed — the ladder is measuring NOTHING, and that is the finding." >&2
exit 1
