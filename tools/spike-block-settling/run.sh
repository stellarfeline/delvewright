#!/usr/bin/env bash
# SPIKE TOOLING (block settling) — NOT part of the shipped pipeline. Same shape
# as `tools/spike-fluid-plane/` and `tools/spike-death-teleport/`.
#
# Measures, on the exact pinned vanilla 1.21.11 image, the two facts the
# `stair-shape` and `water-contained` gates encode — so that neither gate rests
# on a recalled reading of vanilla's source:
#
#   - STAIR SHAPE. A stair's `shape` is not stored, it is DERIVED from the
#     stair's neighbours on every block update. A random field of stairs (two
#     different stair blocks, both halves, all four facings, air holes) is
#     placed, settled, and every cell's resulting `shape` read back. The result
#     is `observations.json`, which `crates/schem/tests/stairs.rs` replays
#     against `delvewright_schem::stairs::derive_shape` — so the implementation
#     is pinned to the GAME's answer, cell for cell, in CI, with no server.
#
#   - WATER. Three placements that decide what "a body of water stays where it
#     was authored" has to mean: a source with an open neighbour, a source
#     beside a waterloggable block written `waterlogged=false`, and a source in
#     a sealed stone box.
#
# Usage:  EULA=TRUE tools/spike-block-settling/run.sh [--out <path>]
#
# EULA: acceptance is the owner's action (ADR-0010) — read from the
# environment, never hardcoded here.
#
# PORTS. Ephemeral loopback port only, never 25565 (the owner's client address;
# see validation/README.md and tools/check-compose-isolation.py).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=tools/lib/rcon.sh
. "${REPO_ROOT}/tools/lib/rcon.sh"
HERE="${REPO_ROOT}/tools/spike-block-settling"
CONTAINER="${SPIKE_CONTAINER:-dw-spike-block-settling}"
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

cleanup() { docker rm -f "${CONTAINER}" >/dev/null 2>&1 || true; }
trap cleanup EXIT

docker rm -f "${CONTAINER}" >/dev/null 2>&1 || true
echo "[spike] starting ${CONTAINER} (vanilla 1.21.11, dry superflat) ..."
docker run -d --name "${CONTAINER}" \
  -e EULA="${EULA}" \
  -e VERSION=1.21.11 -e TYPE=VANILLA \
  -e ONLINE_MODE=FALSE \
  -e MODE=survival -e DIFFICULTY=normal \
  -e LEVEL_TYPE=minecraft:flat -e GENERATE_STRUCTURES=false \
  -e GENERATOR_SETTINGS='{"biome":"minecraft:plains","layers":[{"block":"minecraft:bedrock","height":1},{"block":"minecraft:stone","height":3}]}' \
  -e SPAWN_PROTECTION=0 -e VIEW_DISTANCE=8 -e SIMULATION_DISTANCE=8 \
  -p 127.0.0.1::25565 \
  "${IMAGE}" >/dev/null

echo "[spike] waiting for RCON ..."
READY=0
for _ in $(seq 1 120); do
  if [ -n "$(dw_rcon_probe "${CONTAINER}" list)" ]; then READY=1; break; fi
  sleep 5
done
[ "${READY}" = 1 ] || { echo "[spike] server did not become ready in 10m" >&2; docker logs --tail 50 "${CONTAINER}" >&2; exit 1; }

SPIKE_CONTAINER="${CONTAINER}" SPIKE_OUT="${OUT}" node "${HERE}/measure.mjs"

echo "[spike] observations -> ${OUT}"
