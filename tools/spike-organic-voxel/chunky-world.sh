#!/usr/bin/env bash
# Spike: stamp a `delvec prefab gallery` browse world on the pinned base server
# image and copy the save out, so Chunky can path-trace the voxelised whale.
#
# Why not validation/world-save.sh: it needs a `delvec build` tree, and the
# stub campaign does not build — the analyzer refuses a floating skeleton for
# soft-lock pockets (DW0921) and the light probe has no grade to seed from
# (DW0752). Those refusals are the reading (README "Obstacles"); this script
# only gets pixels. It is not a validation step and proves nothing.
#
# usage: EULA=TRUE chunky-world.sh <gallery-dir> <out-world-dir> <name>
set -euo pipefail
gallery="$(cd "$1" && pwd)"; out="$2"; name="owv-$3"
: "${EULA:?set EULA=TRUE to accept the Mojang EULA (https://aka.ms/MinecraftEULA)}"
here="$(cd "$(dirname "$0")" && pwd)"
base=$(sed -n 's/^ARG DELVE_BASE_IMAGE=//p' "$here/../../validation/Dockerfile.delve")
docker rm -f "$name" >/dev/null 2>&1 || true
docker run -d --name "$name" -e EULA="$EULA" -e TYPE=VANILLA -e VERSION=1.21.11 \
  -e DATAPACKS=/delve/datapack -v "$gallery/datapack:/delve/datapack:ro" \
  -e LEVEL_TYPE=minecraft:flat -e 'GENERATOR_SETTINGS={"biome":"minecraft:the_void","layers":[]}' \
  -e GENERATE_STRUCTURES=false -e SPAWN_MONSTERS=false -e MODE=adventure -e DIFFICULTY=peaceful \
  -e ONLINE_MODE=FALSE -e MAX_MEMORY=4G "$base" >/dev/null
trap 'docker rm -f "$name" >/dev/null 2>&1 || true' EXIT
for _ in $(seq 1 120); do
  r=$(docker exec "$name" rcon-cli "scoreboard players get #t admit.sys" 2>/dev/null || true)
  case "$r" in *"has 7 "*) break ;; esac
  sleep 5
done
case "$r" in *"has 7 "*) echo "placed: $r" ;; *) echo "refusing: gallery never finished placing ($r)" >&2; docker logs "$name" | tail -20 >&2; exit 1 ;; esac
docker exec "$name" rcon-cli "save-all flush"
docker stop -t 60 "$name" >/dev/null
rm -rf "$out"; mkdir -p "$out"
docker cp "$name:/data/world/." "$out/"
n=$(ls "$out/region" | wc -l | tr -d ' ')
[ "$n" -gt 0 ] || { echo "refusing: no region files copied" >&2; exit 1; }
echo "world: $out ($n region files)"
