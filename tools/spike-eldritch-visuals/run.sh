#!/usr/bin/env bash
# SPIKE TOOLING (eldritch visuals lab) — NOT part of the shipped pipeline and NOT
# wired into CI. Same shape as `tools/spike-death-teleport/run.sh`.
#
# Boots a throwaway vanilla server from the exact pinned image digest
# (`versions.toml [images.base] mirror_of`, Minecraft Java 1.21.11) on a flat
# world with the SAME layers as the owner's `datapack_test` world (bedrock 1,
# stone 118, water 8, ocean), installs `dw-eldritch-spike/` BEFORE first start
# (worldgen registries — the two biomes — load only at world load, never on
# /reload), and runs `measure.mjs`: a mineflayer bot walks every station while
# rcon reads the server's own state.
#
# Usage:  EULA=TRUE tools/spike-eldritch-visuals/run.sh [--out <path>]
#
# EULA: acceptance is the owner's action (ADR-0010) — read from the environment.
# PORTS: an ephemeral loopback port, never 25565 (validation/README.md).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=tools/lib/rcon.sh
. "${REPO_ROOT}/tools/lib/rcon.sh"
# shellcheck source=tools/lib/server-heap.sh
. "${REPO_ROOT}/tools/lib/server-heap.sh"
HEAP_ENV="$(dw_server_heap_env)"
HERE="${REPO_ROOT}/tools/spike-eldritch-visuals"
CONTAINER="${SPIKE_CONTAINER:-dw-spike-eldritch}"
OUT="${HERE}/observations.json"
IMAGE='itzg/minecraft-server@sha256:3e7db2562b492dbf442568a327d361547628c98c04a7cb68218c8dde6abdd1de'
FLAT='{"layers":[{"block":"minecraft:bedrock","height":1},{"block":"minecraft:stone","height":118},{"block":"minecraft:water","height":8}],"biome":"minecraft:ocean"}'

while [ $# -gt 0 ]; do
  case "$1" in
    --out) OUT="$2"; shift 2 ;;
    *) echo "usage: $0 [--out <path>]" >&2; exit 2 ;;
  esac
done

: "${EULA:?set EULA=TRUE in your environment to accept the Mojang EULA (https://aka.ms/MinecraftEULA)}"
[ -d "${REPO_ROOT}/harness/node_modules/mineflayer" ] || {
  echo "[spike] harness deps missing — run: (cd ${REPO_ROOT}/harness && npm ci)" >&2
  exit 1
}

cleanup() { docker rm -f "${CONTAINER}" >/dev/null 2>&1 || true; }
trap cleanup EXIT
cleanup

# Boot once so the world exists (owned by the server's uid), stop, add the pack,
# start again: worldgen registries are read at server start, as they are when
# the owner opens her world with the pack already in its datapacks folder.
docker run -d --name "${CONTAINER}" \
  -e EULA="${EULA}" -e VERSION=1.21.11 -e TYPE=VANILLA -e ONLINE_MODE=FALSE \
  -e "${HEAP_ENV}" -e MODE=adventure \
  -e LEVEL_TYPE=minecraft:flat -e "GENERATOR_SETTINGS=${FLAT}" -e GENERATE_STRUCTURES=false \
  -e SPAWN_PROTECTION=0 -e VIEW_DISTANCE=8 \
  -p 127.0.0.1::25565 "${IMAGE}" >/dev/null

wait_ready() {
  local ok=0
  for _ in $(seq 1 120); do
    if dw_rcon_ready "${CONTAINER}"; then ok=1; break; fi
    sleep 5
  done
  [ "${ok}" = 1 ] || { echo "[spike] server did not become ready in 10m" >&2; docker logs --tail 50 "${CONTAINER}" >&2; exit 1; }
}
wait_ready
docker stop "${CONTAINER}" >/dev/null
docker cp "${HERE}/dw-eldritch-spike" "${CONTAINER}:/data/world/datapacks/dw-eldritch-spike"
docker start "${CONTAINER}" >/dev/null

PORT_MAP="$(docker port "${CONTAINER}" 25565)"
PORT_LINE="${PORT_MAP%%$'\n'*}"
PORT="${PORT_LINE##*:}"
[ -n "${PORT}" ] || { echo "[spike] could not read the ephemeral host port" >&2; exit 1; }
echo "[spike] ephemeral host port: 127.0.0.1:${PORT}"
sleep 10
wait_ready

# A function or biome that fails to parse is reported only in the log.
LOG="$(docker logs "${CONTAINER}" 2>&1)"
case "${LOG}" in
  *"Failed to load function"*|*"Failed to parse"*|*"Errors in currently selected data packs"*)
    echo "[spike] the pack did not load cleanly:" >&2
    printf '%s\n' "${LOG}" | grep -E "Failed|Errors" >&2
    exit 1 ;;
esac
dw_rcon "${CONTAINER}" "datapack list enabled"

SPIKE_CONTAINER="${CONTAINER}" SPIKE_PORT="${PORT}" SPIKE_OUT="${OUT}" node "${HERE}/measure.mjs"
