#!/usr/bin/env bash
# PackTest ladder entry (spec-0003 PackTest tier). One build tree, one compose
# project, one exit code = the number of failed tests.
#
#   EULA=TRUE validation/packtest-run.sh --project dw-<id> [--output ./delve-output]
#
# ## Why the project id is REQUIRED (task #185)
#
# The compose project is now the ONLY name a ladder has: `compose.yaml` pins no
# container name and publishes no port, so `-p <id>` isolates the whole stack.
# That is what lets two ladders run side by side on one host instead of queueing
# on a mutex. It only holds if every caller actually passes a distinct id — so an
# invocation without one FAILS here rather than silently landing in compose's
# default project (`validation`), which is a shared name by another route and
# exactly the collision this replaced.
#
# `--output` is the build tree to boot (default `./delve-output`, relative to
# `validation/`). The generated suite is per-campaign — a template class is only
# proven live by a campaign that emits it — so CI runs this script once per
# fixture, each with its own project and tree.
#
# Teardown is `fresh-volumes.sh --project <id>`: it removes only this project and
# PROVES it. Never a bare `docker compose down`, never `docker rm` of a container
# this script did not create.
#
# ## Why it seeds the world volume before booting (task #41)
#
# The Mojang server jar is never baked into an image (ADR-0010, EULA), so a server
# on a FRESH volume bootstraps it live. Isolation gives every ladder its own fresh
# volume — which is right — but it also meant every ladder ran its own live
# bootstrap, and `tier 2` runs six of them. One Mojang blip in any one reddened a
# required status check with nothing to do with the datapack under test (PR #312).
#
# So the bootstrap is fetched and sha256-verified ONCE (`server-bootstrap-cache.sh`,
# idempotent — a warm cache costs nothing) and COPIED into this project's world
# volume before the server starts. Measured: with the seed in place the whole
# PackTest suite runs green under `--network none`. Isolation is unchanged: the copy
# lands in THIS project's volume and `fresh-volumes.sh` still removes it.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"

usage() {
  cat >&2 <<'USAGE'
usage: EULA=TRUE validation/packtest-run.sh --project <compose-project>
                                            [--output <build-tree>]

  --project  REQUIRED. The compose project this ladder owns (e.g. dw-worker-7).
             Distinct per concurrent ladder; there is no default, because a
             shared default is what made ladders queue on each other.
  --output   Build tree to boot, relative to validation/ (default ./delve-output).
USAGE
  exit 2
}

project=""
output="${DELVE_OUTPUT:-./delve-output}"
while [ $# -gt 0 ]; do
  case "$1" in
    --project|-p) [ $# -ge 2 ] || usage; project="$2"; shift 2 ;;
    --output|-o)  [ $# -ge 2 ] || usage; output="$2";  shift 2 ;;
    -h|--help) usage ;;
    *) echo "packtest-run: unknown argument '$1'" >&2; usage ;;
  esac
done

if [ -z "$project" ]; then
  echo "packtest-run: --project <compose-project> is REQUIRED — two ladders sharing" >&2
  echo "  compose's default project would collide on volumes and tear each other down." >&2
  usage
fi
: "${EULA:?set EULA=TRUE to accept the Mojang EULA (https://aka.ms/MinecraftEULA)}"

export DELVE_OUTPUT="$output"
cache="${DW_SERVER_CACHE:-$here/server-cache}"
export DW_SERVER_CACHE="$cache"
COMPOSE=(docker compose -p "$project" -f "$here/compose.yaml" --profile packtest)

cleanup() { "$here/fresh-volumes.sh" --project "$project" >/dev/null 2>&1 || true; }
trap cleanup EXIT

echo "==> packtest: project '$project', build tree '$output'"

# The one live bootstrap fetch, shared by every ladder in this job. Idempotent: a
# warm cache performs no fetch at all. If Mojang really is unreachable this exits
# here, before a server boots and before a datapack is evaluated, with an error
# naming the network — never mistakable for a PackTest failure.
"$here/server-bootstrap-cache.sh" --cache "$cache"

"$here/fresh-volumes.sh" --project "$project"

# Copy the bootstrap overlay into THIS project's world volume. `compose run` is what
# creates the volume, so it is compose-labelled exactly like the one `up` would make
# and `fresh-volumes.sh` still tears it down. The image's own /data (the pre-baked
# Fabric launcher) is populated into the empty volume first, then the overlay lands
# on top; the copy is writable, so itzg's and Fabric's rename-into-place paths stay
# available.
echo "==> packtest: seeding the server bootstrap into '$project' world volume"
"${COMPOSE[@]}" run --rm --no-deps --entrypoint /bin/bash packtest -c '
  set -euo pipefail
  n="$(find /seed -type f | wc -l | tr -d " ")"
  if [ "$n" -eq 0 ]; then
    echo "::error::the server-bootstrap cache is EMPTY — this boot would fetch live" >&2
    exit 1
  fi
  cp -a /seed/. /data/
  # itzg drops to uid/gid 1000 (its default; compose sets no UID/GID override), and
  # this copy runs as root, so hand the files over or the server cannot read them.
  chown -R 1000:1000 /data
  echo "seeded $n bootstrap files into /data"
'

runlog="$(mktemp)"
trap 'rm -f "$runlog"; cleanup' EXIT
set +e
"${COMPOSE[@]}" up --abort-on-container-exit --exit-code-from packtest 2>&1 | tee "$runlog"
rc=${PIPESTATUS[0]}
set -e

# BINDING (CLAUDE.md: a green gate that binds to nothing is VACUOUS). The seed is
# only worth anything if the boot actually used it, so assert the server took the
# cache-hit path and performed NO live bootstrap fetch. A seed that silently missed
# would leave this ladder exactly as fragile as before while reporting success.
boot_log="$(cat "$runlog")"
# Positive binding: itzg says so out loud when the pinned launcher is already
# provisioned locally. Its ABSENCE means the boot went to meta.fabricmc.net.
case "$boot_log" in
  *"is already available"*) : ;;
  *)
    if [ "$rc" -eq 0 ]; then
      echo "::error::the seeded PackTest boot in '$project' never reported a locally" >&2
      echo "  provisioned Fabric launcher — FABRIC_LAUNCHER_VERSION did not match the" >&2
      echo "  pinned image, so this boot resolved it over the network." >&2
      rc=1
    fi
    ;;
esac
fetched=""
for marker in "Downloading Minecraft server" "Downloading library " "Downloading required files"; do
  case "$boot_log" in *"$marker"*) fetched="$fetched  - $marker"$'\n' ;; esac
done
if [ -n "$fetched" ]; then
  echo "::error::the seeded PackTest boot in '$project' STILL performed a live bootstrap fetch:" >&2
  printf '%s' "$fetched" >&2
  echo "  The bootstrap cache did not bind. Re-run validation/server-bootstrap-cache.sh --force;" >&2
  echo "  if it persists, the pinned toolserver image's baked Fabric launcher has moved." >&2
  rc=1
fi

echo "==> packtest: tearing down project '$project'"
"$here/fresh-volumes.sh" --project "$project"
rm -f "$runlog"
trap - EXIT

if [ "$rc" -ne 0 ]; then
  echo "::error:: PackTest FAILED in project '$project' over '$output' (exit $rc = failed tests)" >&2
else
  echo "==> packtest PASSED (project '$project', tree '$output'; 0 live bootstrap fetches)"
fi
exit "$rc"
