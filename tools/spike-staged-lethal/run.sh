#!/usr/bin/env bash
# SPIKE TOOLING (a lethal volume that goes live on a party score) — NOT part of
# the shipped pipeline and NOT wired into CI. Same shape as
# `tools/spike-death-teleport/run.sh`.
#
# Boots a throwaway vanilla server from the exact pinned image digest
# (`versions.toml [images.base] mirror_of`, Minecraft Java 1.21.11) on a flat
# world (bedrock 1, stone 64: the stone top is y 0, a body stands at y 1),
# installs `spikepack/` (functions only, so a `/reload` is enough) and runs
# `measure.mjs`: one mineflayer bot stands still at a chosen point while rcon
# flips the gate the volume's tick line reads, and the server's own tick counter
# and the body's own `Health` say what happened and when. Writes
# `observations.json`.
#
# Usage:  EULA=TRUE tools/spike-staged-lethal/run.sh [--out <path>]
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
HERE="${REPO_ROOT}/tools/spike-staged-lethal"
CONTAINER="${SPIKE_CONTAINER:-dw-spike-staged-lethal}"
OUT="${HERE}/observations.json"
IMAGE='itzg/minecraft-server@sha256:3e7db2562b492dbf442568a327d361547628c98c04a7cb68218c8dde6abdd1de'
FLAT='{"layers":[{"block":"minecraft:bedrock","height":1},{"block":"minecraft:stone","height":64}],"biome":"minecraft:plains"}'

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

docker run -d --name "${CONTAINER}" \
  -e EULA="${EULA}" -e VERSION=1.21.11 -e TYPE=VANILLA -e ONLINE_MODE=FALSE \
  -e "${HEAP_ENV}" -e MODE=survival -e DIFFICULTY=hard \
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

PORT_MAP="$(docker port "${CONTAINER}" 25565)"
PORT_LINE="${PORT_MAP%%$'\n'*}"
PORT="${PORT_LINE##*:}"
[ -n "${PORT}" ] || { echo "[spike] could not read the ephemeral host port" >&2; exit 1; }
echo "[spike] ephemeral host port: 127.0.0.1:${PORT}"

docker exec "${CONTAINER}" mkdir -p /data/world/datapacks
docker cp "${HERE}/spikepack" "${CONTAINER}:/data/world/datapacks/dw-staged-lethal-spike"
dw_rcon "${CONTAINER}" "reload" >/dev/null
sleep 2
LOG="$(docker logs "${CONTAINER}" 2>&1)"
case "${LOG}" in
  *"Failed to load function"*|*"Failed to parse"*|*"Errors in currently selected data packs"*)
    echo "[spike] the pack did not load cleanly:" >&2
    printf '%s\n' "${LOG}" | grep -E "Failed|Errors" >&2
    exit 1 ;;
esac
PACKS="$(dw_rcon "${CONTAINER}" 'datapack list enabled')"
case "${PACKS}" in
  *dw-staged-lethal-spike*) ;;
  *) echo "[spike] spike datapack did not enable: ${PACKS}" >&2; exit 1 ;;
esac
# The pack's own `load` ran on the reload; the gate starts shut.
GATE="$(dw_rcon "${CONTAINER}" 'scoreboard players get #party dwsl.f')"
case "${GATE}" in
  *"has 0 "*) ;;
  *) echo "[spike] the gate did not initialise shut: ${GATE}" >&2; exit 1 ;;
esac

SPIKE_CONTAINER="${CONTAINER}" SPIKE_PORT="${PORT}" SPIKE_OUT="${OUT}" node "${HERE}/measure.mjs"
echo "[spike] wrote ${OUT}"
