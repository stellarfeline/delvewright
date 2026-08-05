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
COMPOSE=(docker compose -p "$project" -f "$here/compose.yaml" --profile packtest)

cleanup() { "$here/fresh-volumes.sh" --project "$project" >/dev/null 2>&1 || true; }
trap cleanup EXIT

echo "==> packtest: project '$project', build tree '$output'"
"$here/fresh-volumes.sh" --project "$project"

set +e
"${COMPOSE[@]}" up --abort-on-container-exit --exit-code-from packtest
rc=$?
set -e

echo "==> packtest: tearing down project '$project'"
"$here/fresh-volumes.sh" --project "$project"
trap - EXIT

if [ "$rc" -ne 0 ]; then
  echo "::error:: PackTest FAILED in project '$project' over '$output' (exit $rc = failed tests)" >&2
else
  echo "==> packtest PASSED (project '$project', tree '$output')"
fi
exit "$rc"
