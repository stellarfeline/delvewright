#!/usr/bin/env bash
# Render with the pinned Chunky core, or refuse by name.
#
# Every emitted scene is written for ONE core, `versions.toml [render]
# chunky_core`. The launcher runs whichever core it chooses and has no flag that
# names one (measured: with the pin and a newer core installed together it chose
# the newer), so a frame rendered through it comes off a core nobody named. This
# runs Chunky's own main class on the classpath the pinned core's version record
# names, after `tools/lib/chunky_core.py` has held the core jar to the pinned
# content digest and every library to its recorded md5 — and when the Chunky home
# does not hold the pin it renders nothing, exits 2, and names the pin, the home
# and `validation/chunky-install.sh`.
#
# The home is resolved by `tools/lib/chunky-home.sh` and handed to Chunky as
# `-Dchunky.home`, so the directory that was verified is the directory Chunky
# reads — its textures, its settings, its cores.
#
# A delve's look (spec-0084 §5.1): `--pack <resourcepack.zip>` as the FIRST
# argument layers that pack above the pinned client jar, through the core's own
# layered option, `-textures <pack>:<jar>`. The order is the core's: it reads
# an entry from the first pack listed that holds it, and appends its default
# jar after every pack named (read off the pinned core's `LayeredResourcePacks`
# and `ResourcePackLoader` bytecode), so the delve's pack is listed first. The
# jar is resolved as `delvec render` resolves it ($DELVEWRIGHT_CLIENT_JAR, then
# `resources/minecraft.jar` in the Chunky home), and the layering is printed.
#
# Usage: validation/chunky.sh [--pack <resourcepack.zip>] <Chunky arguments>
#   e.g. validation/chunky.sh -scene-dir <dir> -render <scene> -f -threads <n>
#        validation/chunky.sh -scene-dir <dir> -snapshot <scene> <out>.png
#        validation/chunky.sh --pack <build>/resourcepack.zip -scene-dir <dir> -render <scene>
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/.." && pwd)"

case "${1:-}" in
  --help-delvewright) sed -n '2,30p' "$0"; exit 0;;
esac
pack=""
if [ "${1:-}" = "--pack" ]; then
  [ $# -ge 2 ] || { echo "chunky: --pack needs a resourcepack.zip" >&2; exit 2; }
  pack="$2"; shift 2
  [ -f "$pack" ] || { echo "chunky: --pack $pack is not a file" >&2; exit 2; }
fi

# shellcheck source=tools/lib/chunky-home.sh
. "$repo/tools/lib/chunky-home.sh"
dw_resolve_chunky_home

# A review render reaches the scene's declared budget (tools/lib/chunky_budget.py):
# a draft's `-target` is saved into the scene by Chunky, so a pass run without one
# is handed the budget the emitter wrote. A `-target` the caller names is theirs.
scene_dir=""; scene=""; explicit=""; prev=""
for a in "$@"; do
  case "$prev" in
    -scene-dir) scene_dir="$a";;
    -render) scene="$a";;
  esac
  [ "$a" = "-target" ] && explicit="--explicit"
  prev="$a"
done
budget_args=()
if [ -n "$scene" ]; then
  budget="$(python3 "$repo/tools/lib/chunky_budget.py" resolve "${scene_dir:-.}" "$scene" $explicit)"
  if [ -n "$budget" ]; then
    budget_args=(-target "$budget")
    echo "chunky: $scene renders to its declared budget, -target $budget" >&2
  fi
fi

status=0
classpath="$(python3 "$repo/tools/lib/chunky_core.py" classpath --home "$DW_CHUNKY_HOME")" || status=$?
if [ "$status" -ne 0 ]; then
  echo "chunky: refusing to render — chunky home $DW_CHUNKY_HOME (from $DW_CHUNKY_HOME_SOURCE) does not hold the pinned core." >&2
  exit 2
fi
main_class="$(python3 "$repo/tools/lib/chunky_core.py" main-class --home "$DW_CHUNKY_HOME")"
echo "chunky: rendering with $(python3 "$repo/tools/lib/versions.py" render.chunky_core), content digest verified, from $DW_CHUNKY_HOME" >&2
if [ -n "$pack" ]; then
  jar="${DELVEWRIGHT_CLIENT_JAR:-$DW_CHUNKY_HOME/resources/minecraft.jar}"
  [ -f "$jar" ] || { echo "chunky: --pack needs the client jar beneath it, and $jar is not a file" >&2; exit 2; }
  echo "chunky: textures $pack layered above $jar (-textures, first listed wins)" >&2
  exec java "-Dchunky.home=$DW_CHUNKY_HOME" -cp "$classpath" "$main_class" -textures "$pack:$jar" "${budget_args[@]+"${budget_args[@]}"}" "$@"
fi
exec java "-Dchunky.home=$DW_CHUNKY_HOME" -cp "$classpath" "$main_class" "${budget_args[@]+"${budget_args[@]}"}" "$@"
