#!/usr/bin/env bash
# SPIKE TOOLING (the shield, and one body against a five-mob wave) — NOT part of
# the shipped pipeline and NOT wired into CI. Same shape as
# `tools/spike-death-teleport/`.
#
# It answers two questions about Minecraft Java 1.21.11 BY MEASUREMENT, and every
# number it reports is the SERVER's: the player statistics
# `damage_blocked_by_shield`, `damage_taken`, `damage_resisted` and
# `damage_dealt`, read as scoreboard objectives, cross-checked against entity NBT
# (`Health`) — never the bot's client-side belief, which cannot see a blocked
# blow at all.
#
#   Q1  Does a shield the harness raises with mineflayer's `activateItem(true)`
#       block, how long must it be up first, and how wide is its arc?
#       (`shield.mjs`, on a throwaway superflat server.)
#
#   Q2  Can ONE body clear a five-mob melee wave under spec-0023's assist?
#       (`guard.ts`, the harness's own `MineflayerExecutor` fighting the wave
#       through its own `fightWave` under its own `withAssist`; `hold.mjs`, the
#       defensive ceiling — a body that never swings, holds the shield up the
#       whole fight and always faces the nearest attacker; `kit.mjs`, the
#       attributes both sides actually carry.)
#
# Q2 needs a BUILT delve, because the fight is measured in the campaign's own
# geometry with the campaign's own bytes:
#
#     delvec build <campaign> --prefabs <content>/prefabs -o <tree>
#     EULA=TRUE tools/spike-guard-fight/run.sh --build <tree>
#
# EULA: acceptance is the owner's action (ADR-0010) — read from the environment,
# never hardcoded here.
#
# PORTS. Both servers publish an EPHEMERAL loopback port (Docker picks the
# number), never 25565: that is the owner's client address and the one genuinely
# shared resource on the host (validation/README.md).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=tools/lib/rcon.sh
. "${REPO_ROOT}/tools/lib/rcon.sh"
# shellcheck source=tools/lib/server-heap.sh
. "${REPO_ROOT}/tools/lib/server-heap.sh"
HEAP_ENV="$(dw_server_heap_env)"
HERE="${REPO_ROOT}/tools/spike-guard-fight"
FLAT="${SPIKE_FLAT_CONTAINER:-dw-spike-shield}"
DELVE="${SPIKE_DELVE_CONTAINER:-dw-spike-guard}"
BUILD=""
REPS_SHIELD="${SPIKE_REPS_SHIELD:-6}"
REPS_GUARD="${SPIKE_REPS_GUARD:-12}"
REPS_HOLD="${SPIKE_REPS_HOLD:-4}"
# The pinned vanilla base (versions.toml [images.base] mirror_of).
IMAGE='itzg/minecraft-server@sha256:3e7db2562b492dbf442568a327d361547628c98c04a7cb68218c8dde6abdd1de'

while [ $# -gt 0 ]; do
  case "$1" in
    --build) BUILD="$2"; shift 2 ;;
    *) echo "usage: EULA=TRUE $0 --build <delvec build output tree>" >&2; exit 2 ;;
  esac
done

: "${EULA:?set EULA=TRUE in your environment to accept the Mojang EULA (https://aka.ms/MinecraftEULA)}"
[ -n "${BUILD}" ] || { echo "[spike] --build <tree> is required: the Guard fight is measured in the campaign's own geometry" >&2; exit 2; }
[ -f "${BUILD}/critical-path.json" ] || { echo "[spike] ${BUILD}/critical-path.json is missing — that is not a build tree" >&2; exit 2; }
[ -d "${REPO_ROOT}/harness/node_modules/mineflayer" ] || {
  echo "[spike] harness deps missing — run: (cd ${REPO_ROOT}/harness && npm ci)" >&2
  exit 1
}

cleanup() { docker rm -f "${FLAT}" "${DELVE}" >/dev/null 2>&1 || true; }
trap cleanup EXIT

# `docker port` captured and split in the shell: `| head -1` would put an
# early-exit consumer on the right of a pipe under `set -o pipefail`.
host_port() {
  local map line
  map="$(docker port "$1" 25565)"
  line="${map%%$'\n'*}"
  echo "${line##*:}"
}

# Readiness is the SERVER'S OWN ANSWER to `list`, never "the poll printed
# something": `dw_rcon_probe` folds stderr into its reply, so before the server is
# up it hands back `Failed to connect to RCON server...` — non-empty, and a
# non-empty test reads that as ready on the first poll and walks straight into a
# world that does not exist yet.
wait_ready() {
  local c="$1" reply
  for _ in $(seq 1 120); do
    reply="$(dw_rcon_probe "$c" list || true)"
    case "${reply}" in
      *"players online"*) return 0 ;;
    esac
    sleep 5
  done
  echo "[spike] $c did not become ready in 10m (last reply: ${reply})" >&2
  docker logs --tail 50 "$c" >&2
  return 1
}

# --- Q1: a throwaway superflat vanilla server --------------------------------
docker rm -f "${FLAT}" >/dev/null 2>&1 || true
echo "[spike] starting ${FLAT} (vanilla 1.21.11, offline, superflat) ..."
docker run -d --name "${FLAT}" \
  -e EULA="${EULA}" -e VERSION=1.21.11 -e TYPE=VANILLA -e ONLINE_MODE=FALSE \
  -e "${HEAP_ENV}" -e MODE=survival -e DIFFICULTY=normal \
  -e LEVEL_TYPE=minecraft:flat -e GENERATE_STRUCTURES=false \
  -e SPAWN_PROTECTION=0 -e VIEW_DISTANCE=8 \
  -p 127.0.0.1::25565 "${IMAGE}" >/dev/null
FLAT_PORT="$(host_port "${FLAT}")"
echo "[spike] ${FLAT} on 127.0.0.1:${FLAT_PORT}"
wait_ready "${FLAT}"

# --- Q2: the SHIPPED delve image for this build tree -------------------------
echo "[spike] building the delve image from ${BUILD} ..."
docker build -q -f "${REPO_ROOT}/validation/Dockerfile.delve" \
  --build-arg "DELVE_BASE_IMAGE=${IMAGE}" -t dw-spike-guard-image "${BUILD}" >/dev/null
docker rm -f "${DELVE}" >/dev/null 2>&1 || true
docker run -d --name "${DELVE}" -e EULA="${EULA}" -e "${HEAP_ENV}" \
  -p 127.0.0.1::25565 dw-spike-guard-image >/dev/null
DELVE_PORT="$(host_port "${DELVE}")"
echo "[spike] ${DELVE} on 127.0.0.1:${DELVE_PORT}"
wait_ready "${DELVE}"
# The world is STAMPED by the datapack over the first ticks of the boot; the
# datapack's own readiness signal says when, and a run that measured a fight in
# an unbuilt hall would measure nothing.
PLACED="$(dw_rcon "${DELVE}" "scoreboard players get #placed dw.sys")"
case "${PLACED}" in
  *"#placed has 1 "*) ;;
  *) echo "[spike] the delve world is not placed: ${PLACED}" >&2; exit 1 ;;
esac

echo "[spike] Q1 — the shield, server-side (N=${REPS_SHIELD} per row)"
SPIKE_CONTAINER="${FLAT}" SPIKE_PORT="${FLAT_PORT}" SPIKE_REPS="${REPS_SHIELD}" \
  node "${HERE}/shield.mjs"

echo "[spike] Q2a — the kit and the wave, as the server reads them"
SPIKE_CONTAINER="${DELVE}" SPIKE_PORT="${DELVE_PORT}" node "${HERE}/kit.mjs"

echo "[spike] Q2b — the harness fighting the Guard, assisted (N=${REPS_GUARD})"
SPIKE_CONTAINER="${DELVE}" DELVEWRIGHT_MC_PORT="${DELVE_PORT}" SPIKE_REPS="${REPS_GUARD}" \
  SPIKE_CP="${BUILD}/critical-path.json" node "${HERE}/guard.ts"

echo "[spike] Q2c — the defensive ceiling: a body that never swings (N=${REPS_HOLD})"
SPIKE_CONTAINER="${DELVE}" SPIKE_PORT="${DELVE_PORT}" SPIKE_REPS="${REPS_HOLD}" \
  node "${HERE}/hold.mjs"

echo "[spike] done — observations beside this script"
