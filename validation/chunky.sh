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
# Usage: validation/chunky.sh <Chunky arguments>
#   e.g. validation/chunky.sh -scene-dir <dir> -render <scene> -f -threads <n>
#        validation/chunky.sh -scene-dir <dir> -snapshot <scene> <out>.png
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/.." && pwd)"

case "${1:-}" in
  --help-delvewright) sed -n '2,20p' "$0"; exit 0;;
esac

# shellcheck source=tools/lib/chunky-home.sh
. "$repo/tools/lib/chunky-home.sh"
dw_resolve_chunky_home

status=0
classpath="$(python3 "$repo/tools/lib/chunky_core.py" classpath --home "$DW_CHUNKY_HOME")" || status=$?
if [ "$status" -ne 0 ]; then
  echo "chunky: refusing to render — chunky home $DW_CHUNKY_HOME (from $DW_CHUNKY_HOME_SOURCE) does not hold the pinned core." >&2
  exit 2
fi
main_class="$(python3 "$repo/tools/lib/chunky_core.py" main-class --home "$DW_CHUNKY_HOME")"
echo "chunky: rendering with $(python3 "$repo/tools/lib/versions.py" render.chunky_core), content digest verified, from $DW_CHUNKY_HOME" >&2
exec java "-Dchunky.home=$DW_CHUNKY_HOME" -cp "$classpath" "$main_class" "$@"
