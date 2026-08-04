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
# version. It binds host 25565 — this is the ONE sanctioned 25565 binding
# (validation workers must never bind it). Never run two `up`s concurrently.
set -euo pipefail

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
  exit 0
fi

# ---- up ---------------------------------------------------------------------
[ -n "$CAMPAIGN" ] || die "up needs a campaign dir"
[ -d "$CAMPAIGN" ] || die "no such campaign dir: $CAMPAIGN"
docker ps --format '{{.Names}}' | grep -qx "$NAME" && die "$NAME already running — 'down' first"
docker ps --format '{{.Ports}}' | grep -q '25565' && die "host 25565 already bound by another container"

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
[ -n "$DELVEC" ] || DELVEC="$REPO_ROOT/target/release/delvec"
if [ ! -x "$DELVEC" ]; then
  echo "building delvec (release)…"
  (cd "$REPO_ROOT" && cargo build --release -p delvewright-compiler --bin delvec >/dev/null)
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
for _ in $(seq 1 60); do
  sleep 10
  if docker logs "$NAME" 2>&1 | grep -q 'Done ('; then break; fi
done
docker logs "$NAME" 2>&1 | grep -q 'Done (' || die "server did not come up — docker logs $NAME"

# rcon verification: campaign objectives present, at least one campaign NPC,
# sidebar cleared. Any failure here means the datapack did not actually load.
rcon "scoreboard objectives list" | grep -q 'dw\.' || die "no dw.* objectives — datapack not loaded"
rcon "execute if entity @e[tag=dw_npc]" | grep -q 'Test passed' || die "no dw_npc entities found"
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

echo
echo "READY — Multiplayer -> Direct Connect: localhost:25565"
echo "$PACK_NOTE"
echo "teardown: tools/playtest-server.sh down --name $NAME"
