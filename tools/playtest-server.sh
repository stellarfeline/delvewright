#!/usr/bin/env bash
# playtest-server.sh — build a campaign and serve it locally for the owner's
# playtest (Multiplayer -> Direct Connect localhost:25565), or tear it down.
#
#   tools/playtest-server.sh up <campaign-dir> [--lang LANG] [--prefabs DIR]
#                                [--delvec BIN] [--name NAME] [--out DIR]
#   tools/playtest-server.sh down [--name NAME]
#   tools/playtest-server.sh status
#
# Owner-facing contract: `up` ends by printing the connect address and, if the
# build ships a resource pack, the pack file name to enable. `down` removes the
# container (server lifecycle rule: tear down as soon as feedback arrives).
#
# Machine-local paths (resource-pack install dir) come from the environment,
# never from this script: set DELVEWRIGHT_RESOURCEPACKS_DIR to the Minecraft
# instance's resourcepacks directory; unset, the pack is left in the build
# output and its path printed instead.
#
# The container is a throwaway itzg/minecraft-server pinned to the project MC
# version. It binds host 25565 — with `validation/owner-play.yaml`, one of the two
# sanctioned 25565 bindings; nothing else in the repo may publish that port
# (`tools/check-compose-isolation.py`, and validation ladders are project-scoped
# with no host port at all). Because 25565 is the one genuinely shared resource
# left on this host, `up` TAKES the 25565 mutex as `owner-play-session` and `down`
# releases it: while it is held, no automation may bind the port, and the release
# is refused while any container still publishes it (validation/mutex.sh).
set -euo pipefail

# shellcheck source=validation/mutex.sh
. "$(cd "$(dirname "$0")/.." && pwd)/validation/mutex.sh"
set -euo pipefail  # mutex.sh sets its own options when sourced; take ours back

MC_VERSION="1.21.11"
NAME="dw-playtest"
LANG_ARG=""
PREFABS_ARG=""
DELVEC=""
OUT_DIR=""
RCON_PW="playtest"

die() { echo "playtest-server: $*" >&2; exit 1; }

cmd="${1:-}"; shift || true
case "$cmd" in up|down|status) ;; *) die "usage: up|down|status (see header)";; esac

CAMPAIGN=""
while [ $# -gt 0 ]; do
  case "$1" in
    --lang)    LANG_ARG="$2"; shift 2;;
    --prefabs) PREFABS_ARG="$2"; shift 2;;
    --delvec)  DELVEC="$2"; shift 2;;
    --name)    NAME="$2"; shift 2;;
    --out)     OUT_DIR="$2"; shift 2;;
    *) [ -z "$CAMPAIGN" ] && CAMPAIGN="$1" || die "unexpected arg: $1"; shift;;
  esac
done

rcon() { docker exec "$NAME" rcon-cli --password "$RCON_PW" "$@"; }

if [ "$cmd" = "status" ]; then
  docker ps --filter "name=$NAME" --format '{{.Names}}\t{{.Status}}\t{{.Ports}}'
  exit 0
fi

if [ "$cmd" = "down" ]; then
  docker rm -f "$NAME" >/dev/null 2>&1 && echo "$NAME removed" || echo "$NAME was not running"
  # Give 25565 back. Cross-shell by construction (`up` ran in another shell), so
  # release BY NAME — which refuses if anything still publishes the port, and
  # refuses outright if the holder is somebody else.
  dw_mutex_release_named "owner-play-session" || true
  exit 0
fi

# ---- up ---------------------------------------------------------------------
[ -n "$CAMPAIGN" ] || die "up needs a campaign dir"
[ -d "$CAMPAIGN" ] || die "no such campaign dir: $CAMPAIGN"
# Capture, then test — never `cmd | grep -q`. Under `set -o pipefail` grep exits at
# the first match, `docker ps` dies of SIGPIPE (141), and pipefail promotes that to
# the pipeline: the guard silently fails to fire *because* it matched. CI keeps this
# out of the tree (tools/check-shell-pipe-shortcircuit.py).
RUNNING="$(docker ps --format '{{.Names}}' || true)"
if [[ $'\n'"$RUNNING"$'\n' == *$'\n'"$NAME"$'\n'* ]]; then die "$NAME already running — 'down' first"; fi
BOUND_PORTS="$(docker ps --format '{{.Ports}}' || true)"
if [[ $BOUND_PORTS == *":25565->"* ]]; then die "host 25565 already bound by another container"; fi
# Claim the port before building anything: if a human is already playing, this
# session must not start at all. Nothing is torn down before the lock is ours.
dw_mutex_acquire "owner-play-session" || die "another 25565 session holds the mutex"
# Hold it only while a session actually exists: a build or boot failure must give
# the port back, or the next `up` waits on a lock nothing is behind (the failure
# mode task #185 removed everywhere else).
UP_OK=0
trap '[ "$UP_OK" = 1 ] || dw_mutex_release' EXIT

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
[ -n "$DELVEC" ] || DELVEC="$REPO_ROOT/target/release/delvec"
if [ ! -x "$DELVEC" ]; then
  echo "building delvec (release)…"
  (cd "$REPO_ROOT" && cargo build --release -p delvec --bin delvec >/dev/null)
fi
[ -n "$OUT_DIR" ] || OUT_DIR="$(mktemp -d "${TMPDIR:-/tmp}/dw-playtest-out.XXXXXX")"

BUILD_ARGS=(build "$CAMPAIGN" --out "$OUT_DIR")
[ -n "$LANG_ARG" ]    && BUILD_ARGS+=(--lang "$LANG_ARG")
[ -n "$PREFABS_ARG" ] && BUILD_ARGS+=(--prefabs "$PREFABS_ARG")
echo "delvec ${BUILD_ARGS[*]}"
"$DELVEC" "${BUILD_ARGS[@]}" || die "build failed — fix the campaign before serving it"

STAGE="$(mktemp -d "${TMPDIR:-/tmp}/dw-playtest-data.XXXXXX")"
mkdir -p "$STAGE/world/datapacks"
cp "$OUT_DIR/server/server.properties" "$STAGE/server.properties"
printf 'enable-rcon=true\nrcon.password=%s\nrcon.port=25575\n' "$RCON_PW" >> "$STAGE/server.properties"
CAMP_ID="$(basename "$CAMPAIGN")"
cp -R "$OUT_DIR/datapack" "$STAGE/world/datapacks/$CAMP_ID"

docker run -d --name "$NAME" -p 25565:25565 \
  -e EULA=TRUE -e TYPE=VANILLA -e VERSION="$MC_VERSION" \
  -e RCON_PASSWORD="$RCON_PW" -e OVERRIDE_SERVER_PROPERTIES=false \
  -v "$STAGE:/data" itzg/minecraft-server:latest >/dev/null

echo "waiting for world generation…"
READY=0
for _ in $(seq 1 60); do
  sleep 10
  BOOT_LOG="$(docker logs "$NAME" 2>&1 || true)"
  if [[ $BOOT_LOG == *"Done ("* ]]; then READY=1; break; fi
done
[ "$READY" = 1 ] || die "server did not come up — docker logs $NAME"

# rcon verification: campaign objectives present, at least one campaign NPC,
# sidebar cleared. Any failure here means the datapack did not actually load.
OBJECTIVES="$(rcon "scoreboard objectives list")"
[[ $OBJECTIVES == *"dw."* ]] || die "no dw.* objectives — datapack not loaded"
NPC_PROBE="$(rcon "execute if entity @e[tag=dw_npc]")"
[[ $NPC_PROBE == *"Test passed"* ]] || die "no dw_npc entities found"
rcon "scoreboard objectives setdisplay sidebar" >/dev/null || true

PACK_NOTE="no resource pack in this build"
if [ -f "$OUT_DIR/resourcepack.zip" ]; then
  if [ -n "${DELVEWRIGHT_RESOURCEPACKS_DIR:-}" ] && [ -d "$DELVEWRIGHT_RESOURCEPACKS_DIR" ]; then
    cp "$OUT_DIR/resourcepack.zip" "$DELVEWRIGHT_RESOURCEPACKS_DIR/$CAMP_ID.zip"
    PACK_NOTE="enable resource pack: $CAMP_ID.zip"
  else
    PACK_NOTE="resource pack at $OUT_DIR/resourcepack.zip (set DELVEWRIGHT_RESOURCEPACKS_DIR to auto-install)"
  fi
fi

UP_OK=1   # the session exists; the 25565 mutex stays held until `down`
echo
echo "READY — Multiplayer -> Direct Connect: localhost:25565"
echo "$PACK_NOTE"
echo "teardown: tools/playtest-server.sh down --name $NAME  (also frees the 25565 mutex)"
