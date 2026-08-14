#!/usr/bin/env bash
# SPIKE TOOLING (fluid plane / dynamic water level) — NOT part of the shipped
# pipeline and NOT wired into CI. Same shape as `tools/spike-death-teleport/`.
#
# Measures, on the exact pinned vanilla 1.21.11 image, what the spec-0038 design
# needs to know rather than reason about:
#
#   - /fill of a whole water plane layer: throughput, MSPT cost, and whether the
#     result is STILL (all sources, no flowing states) in the interior;
#   - what happens at the edge of the filled plane — in ticking chunks, in
#     border chunks, and across a forceload-add/fill/forceload-remove window
#     (the emission strategy for a transition over a mostly-unloaded extent);
#   - lowering the plane with a ticking edge: how fast the ambient sea heals a
#     cleared layer (the "unpleasant to play around" number);
#   - saturated placement vs flow-and-settle vs an interior gap, in a closed
#     basin (the saturation ruling's evidence);
#   - waterloggable blocks under a rising and a falling level;
#   - a player standing in a column the level moves through (displacement,
#     drowning), and a solid runtime fill over a player (entombment);
#   - the /fill block-count ceiling, its exact refusal text, and the
#     `max_block_modifications` gamerule that moves it.
#
# The world is the delve ocean superflat, layer for layer what
# `crates/compiler/src/horizon.rs` emits: bedrock 1 + stone 118 + water 8,
# water top y=62 (SEA_LEVEL), sea floor top y=54.
#
# Usage:  EULA=TRUE tools/spike-fluid-plane/run.sh [--out <path>]
#
# EULA: acceptance is the owner's action (ADR-0010) — read from the environment,
# never hardcoded here.
#
# PORTS. Ephemeral loopback port only, never 25565 (the owner's client address;
# see validation/README.md and tools/check-compose-isolation.py).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=tools/lib/rcon.sh
. "${REPO_ROOT}/tools/lib/rcon.sh"
HERE="${REPO_ROOT}/tools/spike-fluid-plane"
CONTAINER="${SPIKE_CONTAINER:-dw-spike-fluid-plane}"
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
echo "[spike] starting ${CONTAINER} (vanilla 1.21.11, ocean superflat, survival) ..."
# The generator settings are BYTE-IDENTICAL to emit_server's ocean literal, so
# the water geometry measured here is the water geometry a delve ships.
docker run -d --name "${CONTAINER}" \
  -e EULA="${EULA}" \
  -e VERSION=1.21.11 -e TYPE=VANILLA \
  -e ONLINE_MODE=FALSE \
  -e MODE=survival -e DIFFICULTY=normal \
  -e LEVEL_TYPE=minecraft:flat -e GENERATE_STRUCTURES=false \
  -e GENERATOR_SETTINGS='{"biome":"minecraft:ocean","layers":[{"block":"minecraft:bedrock","height":1},{"block":"minecraft:stone","height":118},{"block":"minecraft:water","height":8}]}' \
  -e SPAWN_PROTECTION=0 -e VIEW_DISTANCE=8 -e SIMULATION_DISTANCE=8 \
  -p 127.0.0.1::25565 \
  "${IMAGE}" >/dev/null

PORT_MAP="$(docker port "${CONTAINER}" 25565)"
PORT_LINE="${PORT_MAP%%$'\n'*}"
PORT="${PORT_LINE##*:}"
[ -n "${PORT}" ] || { echo "[spike] could not read the ephemeral host port" >&2; exit 1; }
echo "[spike] ephemeral host port: 127.0.0.1:${PORT}"

echo "[spike] waiting for RCON ..."
READY=0
for _ in $(seq 1 120); do
  if [ -n "$(dw_rcon_probe "${CONTAINER}" list)" ]; then READY=1; break; fi
  sleep 5
done
[ "${READY}" = 1 ] || { echo "[spike] server did not become ready in 10m" >&2; docker logs --tail 50 "${CONTAINER}" >&2; exit 1; }

# --- measurements --------------------------------------------------------------
SPIKE_CONTAINER="${CONTAINER}" SPIKE_PORT="${PORT}" SPIKE_OUT="${OUT}" \
  node "${HERE}/measure.mjs"

echo "[spike] observations -> ${OUT}"
