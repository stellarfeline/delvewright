#!/usr/bin/env bash
# SPIKE TOOLING (view distance) — NOT part of the shipped pipeline and NOT wired
# into CI. Same shape as `tools/spike-display-assembly/run.sh`.
#
# What a served view distance IS on the pinned server, and what it costs with
# the delve's whole party online (spec-0091 §2). One cell of the matrix boots a
# throwaway vanilla server from the exact pinned image digest (`versions.toml`
# `[images.base] mirror_of`, Minecraft Java 1.21.11) on a flat world at one
# `view-distance` / `simulation-distance` pair, then `measure.mjs` joins four
# mineflayer bots (the player cap), stands each in its own disc 2048 blocks from
# the next, and reads:
#
#   * the chunk set each client is SENT (its shape, its axis and diagonal
#     extents, and the radius a body is guaranteed to see in every direction
#     from any standing position in its chunk);
#   * the server's own tick time (`tick query`) and the container's CPU and RSS
#     (`docker stats`) over a settled window;
#   * the JVM's heap after each collection (`-Xlog:gc` into `/data/gc.log`),
#     the live set being the smallest "after" in the window;
#   * on the cell marked `track`, the distance at which the server stops
#     tracking a `block_display` and an `armor_stand` for a client.
#
# Every rcon reply is read (tools/lib/rcon.mjs). The heap ceiling is passed
# through `dw_server_heap_env` with an OVERRIDE (`--heap`, default 8G) so the pin
# cannot cap the measurement: a cell that runs out of heap is a reading about the
# pin, not about the view distance, and is reported as such.
#
# Usage:  EULA=TRUE tools/spike-view-distance/run.sh [--cells "<h>:<view>:<sim>[:track] ..."] [--heap 8G] [--out <path>]
#         default cells: ocean:10:10 ocean:16:10 ocean:24:10:track ocean:32:10 ocean:32:32 void:32:10 ocean:40:10
#
# EULA: acceptance is the owner's action (ADR-0010) — read from the environment.
# PORTS: an ephemeral loopback port, never 25565 (validation/README.md).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=tools/lib/rcon.sh
. "${REPO_ROOT}/tools/lib/rcon.sh"
# shellcheck source=tools/lib/server-heap.sh
. "${REPO_ROOT}/tools/lib/server-heap.sh"
HERE="${REPO_ROOT}/tools/spike-view-distance"
CONTAINER="${SPIKE_CONTAINER:-dw-spike-view-distance}"
OUT="${HERE}/observations.json"
HEAP="8G"
CELLS="ocean:10:10 ocean:16:10 ocean:24:10:track ocean:32:10 ocean:32:32 void:32:10 ocean:40:10"
IMAGE='itzg/minecraft-server@sha256:3e7db2562b492dbf442568a327d361547628c98c04a7cb68218c8dde6abdd1de'
# The two backdrops a delve ships (spec-0013 / spec-0026): the ocean superflat
# the compiler writes for `horizon: ocean`, and a void flat for `void`.
OCEAN='{"layers":[{"block":"minecraft:bedrock","height":1},{"block":"minecraft:stone","height":118},{"block":"minecraft:water","height":8}],"biome":"minecraft:ocean"}'
VOID='{"layers":[],"biome":"minecraft:the_void"}'

while [ $# -gt 0 ]; do
  case "$1" in
    --out) OUT="$2"; shift 2 ;;
    --heap) HEAP="$2"; shift 2 ;;
    --cells) CELLS="$2"; shift 2 ;;
    *) echo "usage: $0 [--cells \"h:view:sim[:track] ...\"] [--heap SIZE] [--out <path>]" >&2; exit 2 ;;
  esac
done

: "${EULA:?set EULA=TRUE in your environment to accept the Mojang EULA (https://aka.ms/MinecraftEULA)}"
[ -d "${REPO_ROOT}/harness/node_modules/mineflayer" ] || {
  echo "[spike] harness deps missing — run: (cd ${REPO_ROOT}/harness && npm ci)" >&2
  exit 1
}

HEAP_ENV="$(dw_server_heap_env "${HEAP}")"
cleanup() { docker rm -f "${CONTAINER}" >/dev/null 2>&1 || true; }
trap cleanup EXIT

wait_ready() {
  local ok=0
  for _ in $(seq 1 120); do
    if dw_rcon_ready "${CONTAINER}"; then ok=1; break; fi
    sleep 5
  done
  [ "${ok}" = 1 ] || { echo "[spike] server did not become ready in 10m" >&2; docker logs --tail 50 "${CONTAINER}" >&2; exit 1; }
}

[ -n "${APPEND:-}" ] || rm -f "${OUT}"
for cell in ${CELLS}; do
  IFS=: read -r horizon view sim track <<<"${cell}"
  track="${track:-}"
  case "${horizon}" in
    ocean) flat="${OCEAN}" ;;
    void) flat="${VOID}" ;;
    *) echo "[spike] unknown horizon ${horizon}" >&2; exit 2 ;;
  esac
  echo "[spike] cell ${cell}: view-distance=${view} simulation-distance=${sim} horizon=${horizon} heap=${HEAP}"
  cleanup
  # `-Xlog:gc` writes one line per collection with the heap before and after it;
  # the periodic G1 cycle every 10 s (load threshold 0: run it however busy the
  # host is) keeps those lines coming once the chunks have settled, so the
  # window holds collections to read a live set from. Neither flag changes what
  # the server serves.
  docker run -d --name "${CONTAINER}" \
    -e EULA="${EULA}" -e VERSION=1.21.11 -e TYPE=VANILLA -e ONLINE_MODE=FALSE \
    -e "${HEAP_ENV}" -e MODE=adventure -e DIFFICULTY=normal \
    -e LEVEL_TYPE=minecraft:flat -e "GENERATOR_SETTINGS=${flat}" -e GENERATE_STRUCTURES=false \
    -e SPAWN_PROTECTION=0 -e "VIEW_DISTANCE=${view}" -e "SIMULATION_DISTANCE=${sim}" \
    -e MAX_PLAYERS=8 \
    -e "JVM_OPTS=-Xlog:gc:file=/data/gc.log:time,uptime -XX:G1PeriodicGCInterval=10000 -XX:G1PeriodicGCSystemLoadThreshold=0" \
    -p 127.0.0.1::25565 "${IMAGE}" >/dev/null
  wait_ready
  PORT_MAP="$(docker port "${CONTAINER}" 25565)"
  PORT_LINE="${PORT_MAP%%$'\n'*}"
  PORT="${PORT_LINE##*:}"
  [ -n "${PORT}" ] || { echo "[spike] could not read the ephemeral host port" >&2; exit 1; }
  echo "[spike] ephemeral host port: 127.0.0.1:${PORT}"
  # The pair the server really runs — read back from its own file, never assumed.
  dw_rcon "${CONTAINER}" "list" >/dev/null
  PROPS="$(docker exec "${CONTAINER}" sh -c "grep -E '^(view-distance|simulation-distance|entity-broadcast-range-percentage)=' /data/server.properties" | tr '\n' ' ')"
  echo "[spike] server.properties: ${PROPS}"
  SPIKE_CONTAINER="${CONTAINER}" SPIKE_PORT="${PORT}" SPIKE_OUT="${OUT}" \
    SPIKE_HORIZON="${horizon}" SPIKE_VIEW="${view}" SPIKE_SIM="${sim}" SPIKE_TRACK="${track}" \
    SPIKE_HEAP="${HEAP}" SPIKE_PROPS="${PROPS}" node "${SPIKE_SCRIPT:-${HERE}/measure.mjs}"
  if dw_server_log_shows_oom "$(docker logs "${CONTAINER}" 2>&1)"; then
    echo "[spike] cell ${cell}: $(dw_server_oom_advice)" >&2
  fi
done
echo "[spike] done -> ${OUT}"
