#!/usr/bin/env bash
# world-save.sh — make a build tree's WORLD exist, once, so the visual tier has
# something to render (spec-0003).
#
#   EULA=TRUE validation/world-save.sh <build-dir> --project dw-<id>
#
# `delvec build` emits a datapack, a server config and a render plan. It does not
# emit a world: a delve's geometry is stamped by the datapack's own `place_all`
# over the first ticks of a server boot (compiler.md §"placement"), which is the
# right design — the shipped image carries bytes we authored, never a world save.
# The consequence is that every Chunky scene the build produces names a world
# that does not exist until some server has booted the tree, and Chunky renders a
# missing world as an empty sky at exit 0.
#
# So this boots the delve ONCE, waits for the datapack to say it has finished
# placing, stops it, and copies the world save into `<build-dir>/world/`. That is
# the path `validation/render-shots.sh` points every scene at, and the path it
# REFUSES to emit a scene set without.
#
# It boots the SAME image the bot ladder and the owner's play session boot
# (`compose.yaml`'s `server` service, `Dockerfile.delve`), in its own compose
# project, with no host port — so it runs beside another ladder and beside a play
# session. Docker is required, as for every other rung of the ladder.
#
# ## The world save is NOT deterministic, and nothing may hash it
#
# ADR-0006 is a claim about the compiler: same DSL + same seed → byte-identical
# DATAPACK. A world save is written by a Minecraft server, and it carries wall
# clock (`level.dat`'s `LastPlayed`, every region file's per-chunk timestamp
# table) plus whatever the server's own tick scheduling touched. Two runs of this
# script over one build tree produce different bytes and that is correct.
#
# Nothing hashes it, by construction rather than by exclusion: the build
# manifest's `outputs` is an index over the compiler's in-memory emission map
# (`emit.rs`), not a walk of the output directory, so a directory that appeared
# after the build cannot enter it; and `tools/gallery-baseline.py` hashes source
# trees (`gallery/`, `crates/`) and reads each build's `manifest.json`, never the
# build directory itself. Keep it that way: a world save inside a hashed walk
# would turn every render into a determinism finding.
#
# ## Readiness is the datapack's own signal, over rcon
#
# `setup` seals the world and forceloads every piece; the tick function then
# retries `place_all`/`place_verify` until every sentinel reports and
# `setup_finish` sets `#placed dw.sys` to 1. That score is the only honest
# "the geometry is in the world now" signal there is — a log line saying `Done (`
# means the server accepted connections, which happens ticks earlier. So this
# polls the score over rcon, through the repo's one rejection rule
# (`tools/lib/rcon.sh`); a world copied out before `#placed` is a world with
# holes in it.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/.." && pwd)"

usage() {
  cat >&2 <<'USAGE'
usage: EULA=TRUE validation/world-save.sh <build-dir> --project <compose-project>
                                          [--timeout <seconds>]

  <build-dir>  a `delvec build` output directory (containing datapack/ + server/)
  --project    REQUIRED. The compose project this boot owns (e.g. dw-round-m).
               Distinct per concurrent run; there is no default, because a shared
               default is what makes two ladders tear each other down.
  --timeout    how long to wait for the datapack to finish placing (default 600).

Writes <build-dir>/world/ (level.dat + region/), replacing whatever was there.
USAGE
  exit 2
}

build_dir=""
project=""
timeout_s=600
while [ $# -gt 0 ]; do
  case "$1" in
    --project|-p) [ $# -ge 2 ] || usage; project="$2"; shift 2 ;;
    --timeout)    [ $# -ge 2 ] || usage; timeout_s="$2"; shift 2 ;;
    -h|--help)    usage ;;
    -*)           echo "world-save: unknown argument '$1'" >&2; usage ;;
    *)            [ -z "$build_dir" ] || { echo "world-save: unexpected argument '$1'" >&2; usage; }
                  build_dir="$1"; shift ;;
  esac
done

[ -n "$build_dir" ] || { echo "world-save: a <build-dir> is required" >&2; usage; }
if [ -z "$project" ]; then
  echo "world-save: --project <compose-project> is REQUIRED — two boots sharing" >&2
  echo "  compose's default project would collide on volumes and tear each other down." >&2
  usage
fi
case "$project" in
  *[!A-Za-z0-9_.-]*|[!A-Za-z0-9]*)
    echo "world-save: '$project' is not a compose project name ([A-Za-z0-9][A-Za-z0-9_.-]*)." >&2
    exit 2
    ;;
esac
case "$timeout_s" in
  ''|*[!0-9]*) echo "world-save: --timeout takes seconds, got '$timeout_s'" >&2; exit 2 ;;
esac

[ -d "$build_dir" ] || { echo "world-save: no such build dir: $build_dir" >&2; exit 2; }
build_abs="$(cd "$build_dir" && pwd)"
for required in datapack server/server.properties manifest.json critical-path.json; do
  [ -e "$build_abs/$required" ] || {
    echo "world-save: $build_abs/$required is missing — pass a 'delvec build' output dir" >&2
    exit 2
  }
done
: "${EULA:?set EULA=TRUE to accept the Mojang EULA (https://aka.ms/MinecraftEULA)}"

# The repo's ONE definition of "the server refused that command"
# (tools/check-live-commands.py binds this). `dw_rcon` asserts the reply;
# `dw_rcon_probe` is the deliberately unjudged form, used here only for the
# liveness poll, which EXPECTS failure until rcon is listening.
# shellcheck source=tools/lib/rcon.sh
. "$repo/tools/lib/rcon.sh"

export DELVE_OUTPUT="$build_abs"
# The Dockerfile lives beside this script, never beside the build tree; left
# relative to the context, a tree outside `validation/` cannot resolve it.
export DELVE_DOCKERFILE="$here/Dockerfile.delve"
# An image TAG is global to the daemon exactly as a container name is: two runs
# building different trees into one tag race, and the loser boots the other's delve.
# shellcheck source=validation/lib/delve-image.sh
. "$here/lib/delve-image.sh"
dw_export_delve_image "$project"
COMPOSE=(docker compose -p "$project" -f "$here/compose.yaml" --profile play)

cleanup() { "$here/fresh-volumes.sh" --project "$project" >/dev/null 2>&1 || true; }
trap cleanup EXIT

echo "==> world save: project '$project', build tree '$build_abs'"
# A stale world volume carries a world already placed by a PREVIOUS tree — which
# is exactly the picture nobody would question.
"$here/fresh-volumes.sh" --project "$project"

echo "==> booting the delve image"
"${COMPOSE[@]}" up -d --build server

cid="$("${COMPOSE[@]}" ps -q server)"
[ -n "$cid" ] || { echo "world-save: the server container did not start" >&2; exit 1; }

echo "==> waiting for the datapack to finish placing (#placed dw.sys = 1, over rcon)"
placed=0
waited=0
while [ "$waited" -lt "$timeout_s" ]; do
  state="$(docker inspect -f '{{.State.Status}}' "$cid" 2>/dev/null || true)"
  if [ "$state" = "exited" ] || [ "$state" = "dead" ]; then
    echo "world-save: the server container exited before the world was placed" >&2
    docker logs "$cid" 2>&1 | tail -n 40 >&2
    exit 1
  fi
  # Unjudged on purpose: until rcon is listening and the datapack has run its
  # `load` function, every reply here is a refusal, and that is the state being
  # polled for. The judged read happens once, below, after this converges.
  reply="$(dw_rcon_probe "$cid" "scoreboard players get #placed dw.sys" || true)"
  case "$reply" in
    *"has 1 [dw.sys]"*) placed=1; break ;;
  esac
  sleep 5
  waited=$((waited + 5))
done
if [ "$placed" != 1 ]; then
  echo "world-save: the datapack never reported #placed dw.sys = 1 within ${timeout_s}s —" >&2
  echo "  the world would be copied out half-stamped, so nothing is copied." >&2
  docker logs "$cid" 2>&1 | tail -n 40 >&2
  exit 1
fi
# Judged, now that a reply is expected to succeed: a refusal here means the
# poll above matched something other than a live scoreboard read.
placed_reply="$(dw_rcon "$cid" "scoreboard players get #placed dw.sys")"
echo "    $placed_reply"

echo "==> flushing the world to disk"
dw_rcon "$cid" "save-all flush" >/dev/null

echo "==> stopping the server"
"${COMPOSE[@]}" stop -t 120 server >/dev/null

# WHICH directory under /data holds the world: `level-name`, read from the
# server's own properties rather than assumed. itzg writes that file at boot from
# the environment the entrypoint derived, so this is the server's answer, not ours.
props="$(mktemp "${TMPDIR:-/tmp}/dw-world-save-props.XXXXXX")"
trap 'rm -f "$props"; cleanup' EXIT
docker cp "$cid:/data/server.properties" "$props" >/dev/null
[ -s "$props" ] || { echo "world-save: could not read the server's /data/server.properties" >&2; exit 1; }
# First match wins and `sed` stops by itself — never `| head -1`, which SIGPIPEs
# its producer (CLAUDE.md's readiness-probe lesson).
level="$(sed -n '/^level-name=/{s///;p;q;}' "$props")"
[ -n "$level" ] || level="world"

echo "==> copying /data/$level -> $build_abs/world"
rm -rf "$build_abs/world"
docker cp "$cid:/data/$level" "$build_abs/world"

[ -f "$build_abs/world/level.dat" ] || {
  echo "world-save: $build_abs/world/level.dat is missing after the copy — no world was saved" >&2
  exit 1
}
regions="$(find "$build_abs/world/region" -type f -name '*.mca' 2>/dev/null | wc -l | tr -d ' ')"
if [ "$regions" -eq 0 ]; then
  echo "world-save: the copied world has NO region files — a scene set over it would" >&2
  echo "  render exactly the empty sky this tool exists to prevent." >&2
  exit 1
fi
# `cat | wc -c` rather than `stat`: BSD and GNU `stat` disagree on every flag,
# and `wc` consumes all of its input so the producer never takes a SIGPIPE.
region_bytes="$(find "$build_abs/world/region" -type f -name '*.mca' -exec cat {} + | wc -c | tr -d ' ')"

echo "world save binding: $regions region file(s), $region_bytes byte(s) -> $build_abs/world"
echo "note: a world save is server-written and carries wall clock — it is NOT"
echo "      byte-reproducible, and nothing in this repository hashes it."
echo "next: validation/render-shots.sh $build_abs"
