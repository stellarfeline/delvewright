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
# **Every `-render` is denoised** (`tools/lib/chunky_denoise.py`, `compiler.md`
# §4): the engine's lantern scale was fitted on denoised frames. Before rendering,
# the home must hold the pinned denoise kit (the chunky-denoiser plugin and Open
# Image Denoise for this platform), or this refuses by name, exit 2, and renders
# nothing. After the core finishes it takes the raw buffer (a PFM snapshot of a
# scene copy), renders the plugin's albedo and normal passes from a derived
# Chunky home, ends that JVM once its job is done (the plugin's update thread
# keeps it alive; bounded by the "Render job finished." line and both maps),
# runs `oidnDenoise` (HDR, RT filter, albedo + normal), keeps the raw dump as
# `<scene>.dump.raw` and writes the denoised buffer as `<scene>.dump`. The
# `-snapshot` that follows applies the scene's own post-process to it — the core
# stays the one implementation of the tone curve. A later `-render` of the same
# scene first puts the raw dump back, so samples accumulate on the raw buffer.
# The step's work files are in `<scene-dir>/<scene>.denoise/`.
#
# Usage: validation/chunky.sh [--pack <resourcepack.zip>] <Chunky arguments>
#   e.g. validation/chunky.sh -scene-dir <dir> -render <scene> -f -threads <n>
#        validation/chunky.sh -scene-dir <dir> -snapshot <scene> <out>.png
#        validation/chunky.sh --pack <build>/resourcepack.zip -scene-dir <dir> -render <scene>
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/.." && pwd)"

case "${1:-}" in
  --help-delvewright) sed -n '2,46p' "$0"; exit 0;;
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
scene_dir=""; scene=""; explicit=""; threads=""; prev=""
for a in "$@"; do
  case "$prev" in
    -scene-dir) scene_dir="$a";;
    -render) scene="$a";;
    -threads) threads="$a";;
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
# Every java launch below carries these. The render never opens a window, and the
# pinned core still reaches the platform toolkit: its texture loader
# (`se.llbit.resources.ImageLoader`) copies each image through
# `BufferedImage.getGraphics`, which asks for the local graphics environment, and
# on macOS that starts the Cocoa application — a foreground app that takes the
# creator's focus and puts an icon in the Dock (measured on a `-render`:
# `sun.awt.CGraphicsDevice` loads, and Chunky becomes the frontmost app until it
# exits). Headless hands it the headless environment; UIElement keeps any
# toolkit that still starts out of the Dock and the app switcher.
quiet_jvm=(-Djava.awt.headless=true -Dapple.awt.UIElement=true)
tex_args=()
if [ -n "$pack" ]; then
  jar="${DELVEWRIGHT_CLIENT_JAR:-$DW_CHUNKY_HOME/resources/minecraft.jar}"
  [ -f "$jar" ] || { echo "chunky: --pack needs the client jar beneath it, and $jar is not a file" >&2; exit 2; }
  echo "chunky: textures $pack layered above $jar (-textures, first listed wins)" >&2
  tex_args=(-textures "$pack:$jar")
fi
if [ -z "$scene" ]; then
  exec java "${quiet_jvm[@]}" "-Dchunky.home=$DW_CHUNKY_HOME" -cp "$classpath" "$main_class" "${tex_args[@]+"${tex_args[@]}"}" "$@"
fi

# ---- -render: the denoise kit is owed before anything renders ----------------
denoise="$repo/tools/lib/chunky_denoise.py"
status=0
python3 "$denoise" verify --home "$DW_CHUNKY_HOME" >&2 || status=$?
if [ "$status" -ne 0 ]; then
  echo "chunky: refusing to render — a rendered frame is denoised, and chunky home $DW_CHUNKY_HOME does not hold the pinned denoise kit (above)." >&2
  exit 2
fi
oidn="$(python3 "$denoise" oidn --home "$DW_CHUNKY_HOME")"
denoise_home="$(python3 "$denoise" denoise-home --home "$DW_CHUNKY_HOME")"
dir="${scene_dir:-.}"
python3 "$denoise" restore --scene-dir "$dir" --scene "$scene" >&2

status=0
java "${quiet_jvm[@]}" "-Dchunky.home=$DW_CHUNKY_HOME" -cp "$classpath" "$main_class" "${tex_args[@]+"${tex_args[@]}"}" "${budget_args[@]+"${budget_args[@]}"}" "$@" || status=$?
[ "$status" -eq 0 ] || { echo "chunky: the render exited $status; nothing was denoised." >&2; exit "$status"; }

# ---- the denoise step --------------------------------------------------------
work="$dir/$scene.denoise"
thread_args=()
[ -n "$threads" ] && thread_args=(-threads "$threads")
python3 "$denoise" prepare --scene-dir "$dir" --scene "$scene" --work "$work"
mkdir -p "$work/raw" "$work/aux"
status=0
java "${quiet_jvm[@]}" "-Dchunky.home=$DW_CHUNKY_HOME" -cp "$classpath" "$main_class" "${tex_args[@]+"${tex_args[@]}"}" \
  -scene-dir "$work/raw" -snapshot "$scene" "$work/raw/$scene.pfm" > "$work/raw.log" 2>&1 || status=$?
[ "$status" -eq 0 ] || { echo "chunky: the raw-buffer snapshot exited $status (see $work/raw.log); nothing was denoised." >&2; exit 1; }
java "${quiet_jvm[@]}" "-Dchunky.home=$denoise_home" -cp "$classpath" "$main_class" "${tex_args[@]+"${tex_args[@]}"}" \
  -scene-dir "$work/aux" -render "$scene" -f "${thread_args[@]+"${thread_args[@]}"}" -target 16 > "$work/aux.log" 2>&1 &
passes=$!
status=0
python3 "$denoise" await-maps --dir "$work/aux" --scene "$scene" --log "$work/aux.log" --pid "$passes" || status=$?
wait "$passes" 2>/dev/null || true
[ "$status" -eq 0 ] || { echo "chunky: the albedo and normal passes failed; nothing was denoised." >&2; exit 1; }
oidn_threads=()
[ -n "$threads" ] && oidn_threads=(--threads "$threads")
status=0
"$oidn" --hdr "$work/raw/$scene.pfm" --alb "$work/aux/$scene.albedo.pfm" --nrm "$work/aux/$scene.normal.pfm" \
  -o "$work/$scene.denoised.pfm" "${oidn_threads[@]+"${oidn_threads[@]}"}" > "$work/oidn.log" 2>&1 || status=$?
[ "$status" -eq 0 ] || { echo "chunky: oidnDenoise exited $status (see $work/oidn.log); nothing was denoised." >&2; exit 1; }
python3 "$denoise" writeback --pfm "$work/$scene.denoised.pfm" --scene-dir "$dir" --scene "$scene"
echo "chunky: $scene denoised (Open Image Denoise $(python3 "$repo/tools/lib/versions.py" render.oidn_version), albedo + normal passes); the raw dump is kept as $scene.dump.raw" >&2
