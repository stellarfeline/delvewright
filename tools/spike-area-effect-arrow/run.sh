#!/usr/bin/env bash
# SPIKE TOOLING (area-effect / non-block-breaking arrow) — NOT part of the
# shipped pipeline and NOT wired into CI. Same shape as
# `tools/spike-death-teleport/` and `tools/spike-jump-arc/`.
#
# Boots a throwaway vanilla server from the exact pinned image digest
# (`versions.toml [images.base] mirror_of`, Minecraft Java 1.21.11), dumps the
# server jar's OWN registry report (so the "does this primitive exist" half of
# the ruling is read off the pinned build, not off a wiki), installs the
# measurement-only datapack in `spikepack/`, and runs `measure.mjs` in two
# phases with a full server restart between them — the reload axis is measured,
# not assumed.
#
# Usage:  EULA=TRUE tools/spike-area-effect-arrow/run.sh [--out <path>]
#
# EULA: acceptance is the owner's action (ADR-0010) — read from the environment,
# never hardcoded here.
#
# PORTS. This rig publishes an EPHEMERAL loopback port (Docker picks the
# number), never 25565. 25565 is the owner's client address and the only
# genuinely shared resource on the host (validation/README.md,
# tools/check-compose-isolation.py); `validation/mutex.sh` guards exactly that
# port and a worker rig must not take it.
#
# ENCHANTMENTS ARE A DYNAMIC REGISTRY. `/reload` does NOT pick up a new or
# changed enchantment definition on 1.21.11 — it is loaded with the world, so
# the pack must be in place before the server starts and a change needs a
# restart. A bad `supported_items` entry does not warn: it aborts server startup
# with `Failed to get element <id>` / `Failed to load registries due to errors`.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=tools/lib/rcon.sh
. "${REPO_ROOT}/tools/lib/rcon.sh"
HERE="${REPO_ROOT}/tools/spike-area-effect-arrow"
CONTAINER="${SPIKE_CONTAINER:-dw-spike-arrow}"
OUT="${HERE}/observations.json"
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

start_server() {
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
}

wait_ready() {
  echo "[spike] waiting for RCON ..."
  for _ in $(seq 1 120); do
    # Liveness poll: the deliberately unjudged channel (tools/lib/rcon.sh) — a
    # refusal here means "not up yet". Readiness is the REPLY, never the exit
    # status of a discarded pipe: a `>/dev/null` probe cannot tell a server
    # that answered from one that answered an error, and the sibling idiom
    # (`| grep -q` under pipefail) read as flakiness for months, cost two owner
    # playtest stagings, and is task #173.
    if [ -n "$(dw_rcon_probe "${CONTAINER}" list)" ]; then return 0; fi
    sleep 5
  done
  echo "[spike] server did not become ready in 10m" >&2
  docker logs --tail 80 "${CONTAINER}" >&2
  return 1
}

read_port() {
  # Capture, then split in the shell. `docker port … | head -1` would put an
  # early-exit consumer on the right of a pipe under `set -o pipefail`
  # (tools/check-shell-pipe-shortcircuit.py).
  local map line
  map="$(docker port "${CONTAINER}" 25565)"
  line="${map%%$'\n'*}"
  PORT="${line##*:}"
  [ -n "${PORT}" ] || { echo "[spike] could not read the ephemeral host port" >&2; return 1; }
  echo "[spike] ephemeral host port: 127.0.0.1:${PORT}"
}

docker rm -f "${CONTAINER}" >/dev/null 2>&1 || true
start_server
wait_ready

# --- the pinned build's OWN registries -----------------------------------------
# `--reports` runs the vanilla data generator out of the same jar the server is
# running, so "no advancement trigger exists for a landed projectile" and "the
# `explode` / `run_function` enchantment entity effects DO exist" are read off
# the shipped build rather than asserted.
echo "[spike] dumping registry report from the pinned jar ..."
docker exec "${CONTAINER}" sh -c '
  set -e
  cd /tmp && rm -rf rep && mkdir rep && cd rep
  java -DbundlerMainClass=net.minecraft.data.Main -jar /data/minecraft_server.*.jar --reports >/dev/null 2>&1
  cd generated/reports && python3 -c "
import json
r = json.load(open(\"registries.json\"))
keys = [\"minecraft:trigger_type\",
        \"minecraft:enchantment_effect_component_type\",
        \"minecraft:enchantment_entity_effect_type\",
        \"minecraft:data_component_type\"]
print(json.dumps({k: sorted(r[k][\"entries\"]) for k in keys}, indent=1))
"' > "${HERE}/registries-1.21.11.json"
[ -s "${HERE}/registries-1.21.11.json" ] || { echo "[spike] registry dump was empty" >&2; exit 1; }
echo "[spike] registries -> ${HERE}/registries-1.21.11.json"

# --- the measurement-only datapack ---------------------------------------------
# Enchantments are a dynamic registry: installing the pack and calling `/reload`
# is NOT enough, so the pack goes in and the server is restarted before the
# first measurement.
docker exec "${CONTAINER}" mkdir -p /data/world/datapacks
docker cp "${HERE}/spikepack" "${CONTAINER}:/data/world/datapacks/dw-spike"
docker stop "${CONTAINER}" >/dev/null
docker start "${CONTAINER}" >/dev/null
wait_ready
PACKS="$(docker exec "${CONTAINER}" rcon-cli 'datapack list enabled')"
echo "[spike] ${PACKS}"
case "${PACKS}" in
  *dw-spike*) ;;
  *) echo "[spike] spike datapack did not enable: ${PACKS}" >&2; exit 1 ;;
esac
read_port

# --- phase 1 -------------------------------------------------------------------
SPIKE_CONTAINER="${CONTAINER}" SPIKE_PORT="${PORT}" SPIKE_OUT="${OUT}" SPIKE_PHASE=1 \
  node "${HERE}/measure.mjs"

# --- restart, then phase 2 (the reload axis) -----------------------------------
echo "[spike] restarting the server for the reload axis ..."
docker exec "${CONTAINER}" rcon-cli save-all flush >/dev/null
docker stop "${CONTAINER}" >/dev/null
docker start "${CONTAINER}" >/dev/null
wait_ready
read_port
SPIKE_CONTAINER="${CONTAINER}" SPIKE_PORT="${PORT}" SPIKE_OUT="${OUT}" SPIKE_PHASE=2 \
  node "${HERE}/measure.mjs"

echo "[spike] done -> ${OUT}"
