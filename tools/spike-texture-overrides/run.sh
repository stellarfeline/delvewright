#!/usr/bin/env bash
# SPIKE TOOLING (texture overrides, spec-0084) — NOT part of the shipped pipeline
# and NOT wired into CI. Same shape as the other `tools/spike-*/` rigs, minus the
# server: every fact this rig records is read from the pinned 1.21.11 CLIENT jar
# and the pinned Chunky core, so nothing boots and no EULA is read.
#
# What it measures (the facts spec-0084 rests on, each one re-runnable):
#   - the client's own `version.json` pack_version — the resource-pack format a
#     delve's `pack.mcmeta` must declare;
#   - the asset namespace the client ships: one count per `assets/minecraft/<class>/`
#     directory, and the texture census (PNGs, animation `.png.mcmeta` sidecars);
#   - the celestial textures (sun, the eight moon phases) and the drowned's two
#     layers, each with its pixel dimensions read off the PNG header;
#   - the client's pack-loading strings: the `min_format`/`max_format` refusal
#     text, and the directory names a server-sent pack and a client pack live in;
#   - whether the pinned Chunky core takes several texture packs (`-textures`).
#
# Usage:  tools/spike-texture-overrides/run.sh [--textures <client jar>] [--out <path>]
#
# The jar is resolved the way `delvec render` resolves it (`--textures`,
# $DELVEWRIGHT_CLIENT_JAR, ~/.chunky/resources/minecraft.jar) and REFUSED unless
# its sha256 is the pinned client's — a reading off some other jar is not a
# measurement of the pin.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HERE="${REPO_ROOT}/tools/spike-texture-overrides"
OUT="${HERE}/observations.json"
TEXTURES=""

while [ $# -gt 0 ]; do
  case "$1" in
    --out) OUT="$2"; shift 2 ;;
    --textures) TEXTURES="$2"; shift 2 ;;
    *) echo "usage: $0 [--textures <client jar>] [--out <path>]" >&2; exit 2 ;;
  esac
done

if [ -z "${TEXTURES}" ]; then
  for c in "${DELVEWRIGHT_CLIENT_JAR:-}" "${HOME}/.chunky/resources/minecraft.jar"; do
    if [ -n "$c" ] && [ -f "$c" ]; then TEXTURES="$c"; break; fi
  done
fi
[ -n "${TEXTURES}" ] && [ -f "${TEXTURES}" ] || {
  echo "[spike] no client jar — pass --textures <1.21.11 client jar>, set DELVEWRIGHT_CLIENT_JAR, or place it at ~/.chunky/resources/minecraft.jar" >&2
  exit 1
}

# The pinned Chunky core, if the Chunky home holds it (optional reading).
CHUNKY_CORE=""
if command -v python3 >/dev/null 2>&1 && [ -f "${REPO_ROOT}/tools/lib/chunky_core.py" ]; then
  # shellcheck source=tools/lib/chunky-home.sh
  . "${REPO_ROOT}/tools/lib/chunky-home.sh"
  if dw_resolve_chunky_home 2>/dev/null; then
    core_name="$(python3 "${REPO_ROOT}/tools/lib/versions.py" render.chunky_core)"
    if [ -f "${DW_CHUNKY_HOME}/lib/${core_name}.jar" ]; then
      CHUNKY_CORE="${DW_CHUNKY_HOME}/lib/${core_name}.jar"
    fi
  fi
fi

echo "[spike] client jar: ${TEXTURES}"
echo "[spike] chunky core: ${CHUNKY_CORE:-<not found — the Chunky reading is recorded as absent>}"
python3 "${HERE}/measure.py" --jar "${TEXTURES}" --out "${OUT}" ${CHUNKY_CORE:+--chunky-core "${CHUNKY_CORE}"}
echo "[spike] observations -> ${OUT}"
