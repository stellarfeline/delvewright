#!/usr/bin/env bash
# Visual-tier shot-set producer (spec-0003, #18: player-POV shots).
#
# Turns a `delvec build` output into the two artifacts a scene review consumes:
#   1. the Chunky scene set  — `delve-render scene`  (one free-camera scene JSON per
#      shot; the first-person player-POV shots render here, since Nucleation is an
#      orbit/turntable renderer and cannot place a free camera at eye height inside a
#      room — see crates/render/README.md).
#   2. the shot index        — `delve-render index`  (image ↔ expect pairs, so a
#      reviewing agent / vision model is handed pairs directly).
#
# This is the ladder step; it does NOT call a vision model — the review stays
# agent-driven (spec-0003 visual tier). Deterministic: same build output → same
# scenes + index. Actually path-tracing the scenes with Chunky (out-of-process,
# xvfb) remains the open CI step (spec-0003) and needs a built world save.
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

# Prefer a prebuilt binary; fall back to `cargo run` from the repo.
render() {
  if command -v delve-render >/dev/null 2>&1; then
    delve-render "$@"
  elif [ -x "$repo/target/debug/delve-render" ]; then
    "$repo/target/debug/delve-render" "$@"
  else
    # `--manifest-path`, not `-p`: this crate is its own workspace (/Cargo.toml).
    ( cd "$repo" && cargo run -q --manifest-path crates/render/Cargo.toml \
        --bin delve-render -- "$@" )
  fi
}

mkdir -p "$out_dir/scenes"
render scene "$build_dir" -o "$out_dir/scenes" --world world
# The whole-map release panorama lands in the same scene dir (every content
# release ships one; the reviewer gets it for free).
render panorama "$build_dir" -o "$out_dir/scenes" --world world
render index "$build_dir" -o "$out_dir/shot-index.json"

n_scenes=$(find "$out_dir/scenes" -name '*.json' | wc -l | tr -d ' ')
echo "shot set ready: $n_scenes Chunky scene(s) (incl. the whole-map panorama) + shot-index.json -> $out_dir"
echo "review: hand each shot-index.json entry's (image, expect) pair to the vision reviewer."
