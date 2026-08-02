#!/usr/bin/env bash
# SPIKE TOOLING (task #67 phase 1) — NOT part of the shipped pipeline.
#
# Empirically measures Minecraft Java 1.21.11 player jump kinematics on a
# throwaway vanilla server (the pinned itzg image from versions.toml), driven by
# a mineflayer bot reusing the harness's pinned dependencies. Findings feed
# docs/notes/jump-arc-model.md; the compiler consumes the MODEL, never this rig.
#
# Usage:  EULA=TRUE tools/spike-jump-arc/run.sh
#
# EULA: acceptance is the owner's action (ADR-0010) — read from the environment,
# never hardcoded here.
#
# Serialises against every other local validation-server user via the shared
# mkdir mutex, and always removes its container + lock on exit.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LOCK=/private/tmp/delvewright-validation.lock.d
CONTAINER=dw-spike-jump-arc
PORT="${SPIKE_PORT:-25599}"
# The pinned vanilla base (versions.toml [images.base] mirror_of).
IMAGE='itzg/minecraft-server@sha256:3e7db2562b492dbf442568a327d361547628c98c04a7cb68218c8dde6abdd1de'

: "${EULA:?set EULA=TRUE in your environment to accept the Mojang EULA (https://aka.ms/MinecraftEULA)}"

# --- validation-resource mutex -------------------------------------------------
echo "[spike] acquiring validation lock ${LOCK} ..."
for _ in $(seq 1 360); do
  if mkdir "${LOCK}" 2>/dev/null; then
    LOCKED=1
    break
  fi
  sleep 5
done
[ "${LOCKED:-0}" = 1 ] || { echo "[spike] could not acquire ${LOCK} after 30m" >&2; exit 1; }

cleanup() {
  docker rm -f "${CONTAINER}" >/dev/null 2>&1 || true
  rmdir "${LOCK}" 2>/dev/null || true
}
trap cleanup EXIT

# --- throwaway server ----------------------------------------------------------
docker rm -f "${CONTAINER}" >/dev/null 2>&1 || true
echo "[spike] starting ${CONTAINER} (vanilla 1.21.11, offline, peaceful) ..."
docker run -d --name "${CONTAINER}" \
  -e EULA="${EULA}" \
  -e VERSION=1.21.11 -e TYPE=VANILLA \
  -e ONLINE_MODE=FALSE \
  -e MODE=survival -e DIFFICULTY=peaceful \
  -e LEVEL_TYPE=minecraft:flat -e GENERATE_STRUCTURES=false \
  -e SPAWN_PROTECTION=0 -e VIEW_DISTANCE=8 -e SNOOPER_ENABLED=FALSE \
  -p "127.0.0.1:${PORT}:25565" \
  "${IMAGE}" >/dev/null

echo "[spike] waiting for RCON ..."
for _ in $(seq 1 120); do
  if docker exec "${CONTAINER}" rcon-cli list >/dev/null 2>&1; then
    READY=1
    break
  fi
  sleep 5
done
[ "${READY:-0}" = 1 ] || { echo "[spike] server did not become ready in 10m" >&2; docker logs --tail 50 "${CONTAINER}" >&2; exit 1; }

# --- measurements --------------------------------------------------------------
SPIKE_CONTAINER="${CONTAINER}" SPIKE_PORT="${PORT}" \
  node "${REPO_ROOT}/tools/spike-jump-arc/measure.mjs"
