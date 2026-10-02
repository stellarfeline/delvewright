#!/usr/bin/env bash
# SPIKE TOOLING (celestial time) — NOT part of the shipped pipeline and NOT wired
# into CI. Same shape as `tools/spike-block-settling/`.
#
# Measures, on the exact pinned vanilla 1.21.11 image, what `/time set <ticks>`
# does to the world clock and to a connected client — the facts spec-0081 (a
# creator declares the sky's celestial state in words; the engine computes the
# ticks) rests on:
#
#   - `time set N` with N >= 24000 writes the level's `dayTime` ABSOLUTELY:
#     `time query day` reads N / 24000 and `time query daytime` N % 24000, and
#     `time query gametime` does not move with it;
#   - a keyword (`time set night`) is absolute too, so it RESETS the day count
#     (and with it the moon phase) to day 0;
#   - the client is told at once: the `update_time` packet carrying the new
#     `dayTime` arrives inside the same second, with no reconnect;
#   - the moon phase is readable server-side through the `time_check` predicate
#     over `period: 192000`, so a PackTest can assert it;
#   - under `advance_time false` (the engine's own seal) the set state holds.
#
# Usage:  EULA=TRUE tools/spike-celestial-time/run.sh [--out <path>]
#
# EULA: acceptance is the owner's action (ADR-0010) — read from the
# environment, never hardcoded here.
#
# PORTS. Ephemeral loopback port only, never 25565 (the owner's client address;
# see validation/README.md and tools/ci/check-compose-isolation.py).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=tools/lib/rcon.sh
. "${REPO_ROOT}/tools/lib/rcon.sh"
# The heap every server this engine starts gets (versions.toml [server].heap_max).
# shellcheck source=tools/lib/server-heap.sh
. "${REPO_ROOT}/tools/lib/server-heap.sh"
HEAP_ENV="$(dw_server_heap_env)"
HERE="${REPO_ROOT}/tools/spike-celestial-time"
CONTAINER="${SPIKE_CONTAINER:-dw-spike-celestial-time}"
OUT="${HERE}/observations.json"
# The pinned vanilla base (versions.toml [images.base] mirror_of).
IMAGE='itzg/minecraft-server@sha256:3e7db2562b492dbf442568a327d361547628c98c04a7cb68218c8dde6abdd1de'

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

docker rm -f "${CONTAINER}" >/dev/null 2>&1 || true
echo "[spike] starting ${CONTAINER} (vanilla 1.21.11, offline, superflat) ..."
docker run -d --name "${CONTAINER}" \
  -e EULA="${EULA}" \
  -e VERSION=1.21.11 -e TYPE=VANILLA \
  -e ONLINE_MODE=FALSE \
  -e "${HEAP_ENV}" \
  -e MODE=creative -e DIFFICULTY=peaceful \
  -e LEVEL_TYPE=minecraft:flat -e GENERATE_STRUCTURES=false \
  -e SPAWN_PROTECTION=0 -e VIEW_DISTANCE=4 \
  -p 127.0.0.1::25565 \
  "${IMAGE}" >/dev/null

# Capture, then split in the shell (tools/ci/check-shell-pipe-shortcircuit.py).
PORT_MAP="$(docker port "${CONTAINER}" 25565)"
PORT_LINE="${PORT_MAP%%$'\n'*}"
PORT="${PORT_LINE##*:}"
[ -n "${PORT}" ] || { echo "[spike] could not read the ephemeral host port" >&2; exit 1; }
echo "[spike] ephemeral host port: 127.0.0.1:${PORT}"

echo "[spike] waiting for RCON ..."
READY=0
for _ in $(seq 1 120); do
  if dw_rcon_ready "${CONTAINER}"; then READY=1; break; fi
  sleep 5
done
[ "${READY}" = 1 ] || { echo "[spike] server did not become ready in 10m" >&2; docker logs --tail 50 "${CONTAINER}" >&2; exit 1; }

# --- the measurement-only datapack (two predicates, no mcfunction) -----------
docker exec "${CONTAINER}" mkdir -p /data/world/datapacks
docker cp "${HERE}/spikepack" "${CONTAINER}:/data/world/datapacks/dw-spike"
dw_rcon "${CONTAINER}" "reload" >/dev/null
PACKS="$(dw_rcon "${CONTAINER}" 'datapack list enabled')"
echo "[spike] ${PACKS}"
case "${PACKS}" in
  *dw-spike*) ;;
  *) echo "[spike] spike datapack did not enable: ${PACKS}" >&2; exit 1 ;;
esac

# --- measurements --------------------------------------------------------------
SPIKE_CONTAINER="${CONTAINER}" SPIKE_PORT="${PORT}" SPIKE_OUT="${OUT}" \
  node "${HERE}/measure.mjs"
echo "[spike] observations -> ${OUT}"
