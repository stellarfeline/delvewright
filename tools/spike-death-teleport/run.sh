#!/usr/bin/env bash
# SPIKE TOOLING (death edge + teleport fall settlement) — NOT part of the shipped
# pipeline and NOT wired into CI. Same shape as `tools/spike-jump-arc/`.
#
# Boots a throwaway vanilla server from the exact pinned image digest
# (`versions.toml [images.base] mirror_of`, Minecraft Java 1.21.11), installs the
# measurement-only datapack in `spikepack/` (two advancements, no functions),
# and runs `measure.mjs` against it. Findings feed
# `docs/notes/death-and-teleport-spike.md`; nothing in the compiler consumes this
# rig.
#
# Usage:  EULA=TRUE tools/spike-death-teleport/run.sh [--out <path>]
#
# EULA: acceptance is the owner's action (ADR-0010) — read from the environment,
# never hardcoded here.
#
# PORTS. This rig publishes an EPHEMERAL loopback port (Docker picks the number),
# never 25565. 25565 is the owner's client address and the only genuinely shared
# resource on the host (validation/README.md, tools/check-compose-isolation.py);
# `validation/mutex.sh` guards exactly that port and a worker rig must not take
# it. Two copies of this spike can therefore run side by side — pass a distinct
# SPIKE_CONTAINER if you want that.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HERE="${REPO_ROOT}/tools/spike-death-teleport"
CONTAINER="${SPIKE_CONTAINER:-dw-spike-death-tp}"
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
echo "[spike] starting ${CONTAINER} (vanilla 1.21.11, offline, hard, superflat) ..."
docker run -d --name "${CONTAINER}" \
  -e EULA="${EULA}" \
  -e VERSION=1.21.11 -e TYPE=VANILLA \
  -e ONLINE_MODE=FALSE \
  -e MODE=survival -e DIFFICULTY=hard \
  -e LEVEL_TYPE=minecraft:flat -e GENERATE_STRUCTURES=false \
  -e SPAWN_PROTECTION=0 -e VIEW_DISTANCE=8 \
  -p 127.0.0.1::25565 \
  "${IMAGE}" >/dev/null

PORT="$(docker port "${CONTAINER}" 25565 | head -1 | sed 's/.*://')"
[ -n "${PORT}" ] || { echo "[spike] could not read the ephemeral host port" >&2; exit 1; }
echo "[spike] ephemeral host port: 127.0.0.1:${PORT}"

echo "[spike] waiting for RCON ..."
READY=0
for _ in $(seq 1 120); do
  if docker exec "${CONTAINER}" rcon-cli list >/dev/null 2>&1; then READY=1; break; fi
  sleep 5
done
[ "${READY}" = 1 ] || { echo "[spike] server did not become ready in 10m" >&2; docker logs --tail 50 "${CONTAINER}" >&2; exit 1; }

# --- the measurement-only datapack (two advancements, no mcfunction) -----------
docker exec "${CONTAINER}" mkdir -p /data/world/datapacks
docker cp "${HERE}/spikepack" "${CONTAINER}:/data/world/datapacks/dw-spike"
docker exec "${CONTAINER}" rcon-cli "reload" >/dev/null
PACKS="$(docker exec "${CONTAINER}" rcon-cli 'datapack list enabled')"
echo "[spike] ${PACKS}"
case "${PACKS}" in
  *dw-spike*) ;;
  *) echo "[spike] spike datapack did not enable: ${PACKS}" >&2; exit 1 ;;
esac

# --- the gamerule registry, straight out of the pinned jar ---------------------
# 1.21.11 renamed every gamerule identifier to snake_case and reworded several,
# so a legacy camelCase `gamerule` line is silently REJECTED at run time. This
# dumps the authoritative identifier list from the server jar's own constant pool
# next to the observations, so the claim in the findings note is checkable.
GR_OUT="$(dirname "${OUT}")/gamerules-1.21.11.txt"
docker exec "${CONTAINER}" sh -c '
  set -e
  cd /tmp && rm -rf gr && mkdir gr && cd gr
  # /data/minecraft_server.<v>.jar is the BUNDLER since 1.18; the classes live in
  # the bundled /data/versions/<v>/server-<v>.jar that itzg unpacks beside it.
  unzip -o -q /data/versions/*/server-*.jar
  set +e
  cls="$(grep -rl advance_time . | grep "\.class$")"
  [ -n "$cls" ] || { echo "gamerule class not found" >&2; exit 1; }
  cat $cls
' | LC_ALL=C tr -c '[:print:]' '\n' | grep -E '^[a-z][a-z_]{3,40}$' | sort -u > "${GR_OUT}"
[ -s "${GR_OUT}" ] || { echo "[spike] gamerule candidate extraction produced nothing" >&2; exit 1; }
echo "[spike] gamerule identifier candidates -> ${GR_OUT} ($(wc -l < "${GR_OUT}" | tr -d ' ') strings)"

# --- measurements --------------------------------------------------------------
SPIKE_CONTAINER="${CONTAINER}" SPIKE_PORT="${PORT}" SPIKE_OUT="${OUT}" \
  SPIKE_GAMERULE_CANDIDATES="${GR_OUT}" \
  node "${HERE}/measure.mjs"
