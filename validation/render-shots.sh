#!/usr/bin/env bash
# Visual-tier shot-set producer (spec-0003; player-POV shots).
#
# Turns a `delvec build` output into the two artifacts a scene review consumes:
#   1. the Chunky scene set  — `delvec scene`  (one free-camera scene JSON per
#      shot; the first-person player-POV shots render here, since Nucleation is an
#      orbit/turntable renderer and cannot place a free camera at eye height inside a
#      room — see crates/render/README.md).
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
# Usage: validation/render-shots.sh <build-dir> [out-dir]
#   <build-dir>  a `delvec build` output directory (containing render-plan.json)
#   [out-dir]    where to write scenes/ + shot-index.json (default <build-dir>/shots)
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/.." && pwd)"

build_dir="${1:?usage: render-shots.sh <build-dir> [out-dir]}"
out_dir="${2:-$build_dir/shots}"

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
# the one a creator already has (ADR-0021 §1). Prefer a prebuilt binary; fall
# back to `cargo run` from the repo.
render() {
  if command -v delvec >/dev/null 2>&1; then
    delvec "$@"
  elif [ -x "$repo/target/debug/delvec" ]; then
    "$repo/target/debug/delvec" "$@"
  else
    ( cd "$repo" && cargo run -q -p delvec --bin delvec -- "$@" )
  fi
}

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
