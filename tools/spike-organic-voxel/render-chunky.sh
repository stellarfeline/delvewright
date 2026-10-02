#!/usr/bin/env bash
# Spike: path-trace the four views of one voxelised whale with the pinned Chunky.
# Needs run.sh to have produced <scratch>/prefab-<base>/ and <scratch>/out/<base>.npy.
#
# usage: EULA=TRUE render-chunky.sh <delvec-binary> <scratch-dir> <base> <spp>
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
delvec="$1"; scratch="$(cd "$2" && pwd)"; base="$3"; spp="$4"
"$delvec" --version
rm -rf "$scratch/gallery-$base" "$scratch/scenes-$base"
"$delvec" prefab gallery "$scratch/prefab-$base/$base.json" -o "$scratch/gallery-$base" | tail -1
"$here/chunky-world.sh" "$scratch/gallery-$base" "$scratch/world-$base" "$base"
mkdir -p "$scratch/scenes-$base" "$scratch/renders"
uv run -q --with numpy==2.3.3 python "$here/views.py" --chunky "$scratch/world-$base" \
    "$scratch/out/$base.npy" "$scratch/scenes-$base" "$base" "$spp"
for v in far spine ribcage skull; do
  "$here/../../validation/chunky.sh" -scene-dir "$scratch/scenes-$base" -render "$base-$v" -threads 4 -f >"$scratch/scenes-$base/$v.log" 2>&1
  snaps=("$scratch/scenes-$base/snapshots/$base-$v-"*.png)
  png="${snaps[${#snaps[@]}-1]}"  # one render per run: the scene directory is fresh
  cp "$png" "$scratch/renders/$base-$v-chunky.png"
  echo "$v: $scratch/renders/$base-$v-chunky.png (from $(basename "$png"))"
done
