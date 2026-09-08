#!/usr/bin/env bash
# Visual-tier shot-set producer (spec-0003; player-POV shots).
#
# Turns a `delvec build` output into the two artifacts a scene review consumes:
#   1. the Chunky scene set  — `delvec scene`  (one free-camera scene JSON per
#      shot; the first-person player-POV shots render here, since Nucleation is an
#      orbit/turntable renderer and cannot place a free camera at eye height inside a
#      room — see docs/reference/tools.md §4).
#   2. the shot index        — `delvec index`  (image ↔ expect pairs, so a
#      reviewing agent / vision model is handed pairs directly).
#
# This is the ladder step; it does NOT call a vision model — the review stays
# agent-driven (spec-0003 visual tier).
#
# ## THE WORLD MUST EXIST FIRST, and this refuses without it
#
# A scene names the world Chunky loads. `delvec build` emits no world — a delve's
# geometry is stamped by the datapack's `place_all` over the first ticks of a
# server boot — so a scene set produced straight off a build tree points at
# nothing. Chunky's answer to a missing world is an EMPTY SKY at exit 0, with the
# reason (`Could not load chunks (no world found for scene)`) buried in a Java
# stack trace: hundreds of plausible, beautiful, identical pictures of nothing,
# and every command in the recipe green. That is what this refusal exists for.
# `validation/world-save.sh <build-dir> --project <id>` boots the tree once and
# writes `<build-dir>/world/`; nothing else in the repository produces one, so
# there is no second way to satisfy this and no way to opt out of it.
#
# The emitted scenes name that world by ABSOLUTE path, because Chunky resolves a
# scene's world path against the RENDERING PROCESS'S working directory and not
# against the scene directory (measured on the pinned core: the same scene, same
# `-scene-dir`, loads from one CWD and reports "no world found" from another).
# A relative path would therefore be correct only for a reviewer who happened to
# stand in the right directory, and wrong SILENTLY — as an empty frame — for
# everyone else, which is the defect this script is refusing on.
#
# Determinism: the scene set is deterministic for a given build tree at a given
# path — same build output, same output directory, same scene and index bytes.
# The world path is the one machine-dependent field, and it is the price of a
# scene that resolves from any CWD. The world save itself is server-written and
# NOT byte-reproducible (see `world-save.sh`); nothing hashes either.
#
# ## THE RENDERER IS NAMED, AND SO IS THE ONE THAT IS ACTUALLY INSTALLED
#
# These scenes are written for ONE Chunky core — `versions.toml [render]
# chunky_core` — and the camera basis, the water-surface offset and the
# night-vision emulation were all reverse-engineered against that core's
# bytecode. The install line every page prints, `java -jar ChunkyLauncher.jar
# --update snapshot`, installs whatever the snapshot CHANNEL is serving today,
# which on the third end-to-end drill was four builds newer than the pin. Nothing
# compared the two, in either direction.
#
# The pinned core cannot be asked for by name, and that was established against
# the launcher rather than recalled (see `docs/reference/tools.md` §4a for the
# evidence): `--update` takes a release CHANNEL, every channel resolves to
# `snapshot.json` / `latest.json` which name only the newest build, and
# `<updateSite>/lib/<name>.jar` ignores the name it is given — asking it for the
# pinned `…474.g156e2bb.jar` returns the current jar, `content-disposition`
# naming `…478.g527cb4a.jar`, whose MD5 the launcher's own integrity check would
# then reject against a version record naming the pin.
#
# So this script REPORTS rather than refuses, and it reports at the end where it
# is read: it names the pin and every core actually installed, and says plainly
# when the frames will not come off the pinned renderer. A refusal here would be
# a wall with no remedy behind it — there is no command that installs the pin —
# and a gate that names an unreachable remedy is worse than a stated fact.
#
# The Chunky settings directory is `~/.chunky`; `DELVEWRIGHT_CHUNKY_HOME`
# overrides it for a machine that keeps it elsewhere.
#
# Usage: validation/render-shots.sh <build-dir> [out-dir] [--delvec BIN]
#   <build-dir>  a `delvec build` output directory (containing render-plan.json)
#   [out-dir]    where to write scenes/ + shot-index.json (default <build-dir>/shots)
#   --delvec     the engine to emit with (default: the creator's, see below)
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/.." && pwd)"

build_dir=""
out_dir=""
delvec_arg=""
while [ $# -gt 0 ]; do
  case "$1" in
    --delvec) delvec_arg="$2"; shift 2;;
    -h|--help) sed -n '2,73p' "$0"; exit 0;;
    -*) echo "render-shots: unknown argument \`$1\`" >&2; exit 2;;
    *)
      if [ -z "$build_dir" ]; then build_dir="$1"
      elif [ -z "$out_dir" ]; then out_dir="$1"
      else echo "render-shots: unexpected argument \`$1\`" >&2; exit 2
      fi
      shift;;
  esac
done
[ -n "$build_dir" ] || { echo "usage: render-shots.sh <build-dir> [out-dir] [--delvec BIN]" >&2; exit 2; }
[ -n "$out_dir" ] || out_dir="$build_dir/shots"

if [ ! -f "$build_dir/render-plan.json" ]; then
  echo "error: $build_dir/render-plan.json not found — pass a 'delvec build' output dir" >&2
  exit 2
fi

# The world gate. Before anything is written: a scene set over a world that does
# not exist is never produced, so nobody can read one as evidence.
build_abs="$(cd "$build_dir" && pwd)"
world_dir="$build_abs/world"
world_regions=0
if [ -d "$world_dir/region" ]; then
  world_regions="$(find "$world_dir/region" -type f -name '*.mca' | wc -l | tr -d ' ')"
fi
if [ ! -f "$world_dir/level.dat" ] || [ "$world_regions" -eq 0 ]; then
  echo "error: $world_dir has no world save (level.dat: $([ -f "$world_dir/level.dat" ] && echo present || echo MISSING), region files: $world_regions)." >&2
  echo "  A Chunky scene names the world it loads, and \`delvec build\` writes no world:" >&2
  echo "  the geometry is stamped by the datapack over the first ticks of a server boot." >&2
  echo "  Rendering these scenes now would produce an empty sky at exit 0 — the reason" >&2
  echo "  appears only inside a Java stack trace — so no scene set is written." >&2
  echo "" >&2
  echo "  Remedy — boot the tree once and copy its world out:" >&2
  echo "    EULA=TRUE validation/world-save.sh $build_abs --project dw-<id>" >&2
  exit 2
fi

# Every arm this script runs is a CPU arm, so it is `delvec` — one binary, and
# the one a creator already has (ADR-0021 §1). WHICH one is not left to whatever
# `PATH` happens to hold: a `delvec` on `PATH` is used when it IS this engine
# (`--version` == `versions.toml` `[engine].version`), and otherwise one is built
# from this tree — with the choice and its reason printed in one line. See
# `tools/lib/delvec-bin.sh`; `--delvec BIN` names one outright.
# shellcheck source=tools/lib/delvec-bin.sh
. "$repo/tools/lib/delvec-bin.sh"
delvec_bin="$(dw_resolve_delvec "$delvec_arg" "$repo" "render-shots")" || exit 1
render() { "$delvec_bin" "$@"; }

mkdir -p "$out_dir/scenes"
render scene "$build_dir" -o "$out_dir/scenes" --world "$world_dir"
# The whole-map release panorama lands in the same scene dir (every content
# release ships one; the reviewer gets it for free).
render panorama "$build_dir" -o "$out_dir/scenes" --world "$world_dir"
render index "$build_dir" -o "$out_dir/shot-index.json"

n_scenes=$(find "$out_dir/scenes" -name '*.json' | wc -l | tr -d ' ')
echo "shot set ready: $n_scenes Chunky scene(s) (incl. the whole-map panorama) + shot-index.json -> $out_dir"
echo "world binding: $world_regions region file(s) at $world_dir — every scene loads that save."
echo "review: hand each shot-index.json entry's (image, expect) pair to the vision reviewer."

# ---- which renderer will actually read these scenes --------------------------
# The pin has ONE home and is read from it (tools/lib/versions.py); this script
# restates no revision. What it adds is the second half of the comparison — the
# cores the machine actually holds — because until now nothing anywhere put the
# two numbers in the same sentence.
chunky_pin="$(python3 "$repo/tools/lib/versions.py" render.chunky_core)" || {
  echo "render-shots: cannot read \`[render].chunky_core\` from versions.toml — the renderer cannot be named" >&2
  exit 1
}
chunky_home="${DELVEWRIGHT_CHUNKY_HOME:-$HOME/.chunky}"
installed_cores=""
if [ -d "$chunky_home/lib" ]; then
  installed_cores="$(find "$chunky_home/lib" -maxdepth 1 -type f -name 'chunky-core-*.jar' \
    -exec basename {} .jar \; | LC_ALL=C sort | tr '\n' ' ')"
  installed_cores="${installed_cores% }"
fi

if [ -z "$installed_cores" ]; then
  echo "chunky core: NONE installed under $chunky_home/lib. These scenes were emitted for the pinned core $chunky_pin." >&2
  echo "  \`java -jar ChunkyLauncher.jar --update snapshot\` installs the CURRENT snapshot, which is not the pin and cannot be made to be:" >&2
  echo "  the launcher's --update takes a release channel, and the update site's lib/ path serves today's core whatever name it is asked for." >&2
  echo "  Render anyway if you must, and read every frame as coming off an unpinned renderer." >&2
elif case " $installed_cores " in *" $chunky_pin "*) true;; *) false;; esac; then
  echo "chunky core: pinned $chunky_pin is installed at $chunky_home/lib — render with it."
else
  echo "chunky core: MISMATCH — these scenes were emitted for $chunky_pin, and $chunky_home/lib holds only: $installed_cores" >&2
  echo "  Every frame you are about to judge will come off a core this project has not verified its scene format against." >&2
  echo "  There is no command that installs the pin: --update takes a release channel and the update site's lib/ path" >&2
  echo "  serves today's core whatever name it is asked for, so this is reported and not refused. Say so in the review." >&2
fi
