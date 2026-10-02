#!/usr/bin/env bash
# Spike runner: fetch the CC0 mesh, voxelise at each scale, convert through the
# engine (`delvec schem convert`), audit, try the light probe, draw four views.
#
# usage: run.sh <delvec-binary> <scratch-dir> <length>[:<palette>]...
#   <delvec-binary>  a release build of origin/main (the instrument; named on stdout)
#   <scratch-dir>    working directory outside the repository (never /tmp)
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
delvec="$1"; scratch="$2"; shift 2
"$delvec" --version
py=(uv run -q --with numpy==2.3.3 --with scipy==1.16.2 python)

"$here/fetch-mesh.sh" "$scratch/mesh"
stl="$scratch/mesh/recon.stl"

for arg in "$@"; do
  L="${arg%%:*}"; pal="weathered"; [ "$arg" != "$L" ] && pal="${arg#*:}"
  base="whale-$L"; [ "$pal" != "weathered" ] && base="whale-$L-$pal"
  pre="$scratch/prefab-$base"
  mkdir -p "$pre" "$scratch/out" "$scratch/renders"
  "${py[@]}" "$here/voxelize.py" --stl "$stl" --length "$L" --palette "$pal" \
      --out "$scratch/out/$base.schem" --report "$scratch/out/$base.report.json" \
      --grid-out "$scratch/out/$base.npy"
  "$delvec" schem convert "$scratch/out/$base.schem" -o "$pre/$base.nbt"
  mv "$pre/$base.split.json" "$scratch/out/"
  python3 "$here/make_manifest.py" "$scratch/out/$base.split.json" "$scratch/out/$base.report.json" - "$pre/$base.json"
  "$delvec" prefab audit "$pre/$base.json" -o "$scratch/out/$base.audit.json" >/dev/null
  python3 -c "import json,sys; d=json.load(open(sys.argv[1])); print('audit', d['verdict'], 'stairs_examined', d['stairs_examined'], 'underspecified', d['underspecified'])" "$scratch/out/$base.audit.json"
  # The light probe is expected to refuse a floating piece (DW0752); its words are the reading.
  "$delvec" prefab lighting "$pre/$base.json" 2>&1 | grep -E "^DW|^light" || true
  camp="$scratch/campaign-$base"
  rm -rf "$camp"; cp -R "$here/campaign" "$camp"
  sed -i.bak "s#prefab/whale-LEN#prefab/$base#" "$camp/world.json" && rm "$camp/world.json.bak"
  "${py[@]}" "$here/views.py" "$delvec" "$pre" "$camp" "$scratch/out/$base.npy" "$scratch/renders" "$base"
done
