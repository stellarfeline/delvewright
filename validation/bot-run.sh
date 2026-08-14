#!/usr/bin/env bash
# Bot ladder entry (spec-0003 `validate` profile): server + mineflayer
# critical-path bot, one compose project, exit code = the bot's result.
#
#   EULA=TRUE validation/bot-run.sh --project dw-<id>
#   DELVEWRIGHT_RUN_TIMEOUT_MS=2400000 EULA=TRUE validation/bot-run.sh --project dw-<id>
#
# ## Why the project id is REQUIRED (task #185)
#
# The compose project is the ONLY name a ladder has now: `compose.yaml` pins no
# container name and publishes no host port, so `-p <id>` isolates containers,
# volumes and network completely and two ladders can run at the same time on one
# host. That only holds if every caller passes a distinct id, so an invocation
# without one FAILS here instead of landing in compose's default project
# (`validation`) — a shared name by another route, and the collision this design
# exists to remove.
#
# The run report follows the project too (`--run-out`, default
# `validation/run-out/<project>/run-report.json`): the compose mount is a HOST
# path, so two ladders from one checkout would otherwise overwrite each other's
# report — the same collision one layer up, and the one that would make a green
# ladder describe somebody else's run.
#
# Every environment variable `src/run.ts` reads is forwarded by the compose file
# and therefore settable on this command line (DELVEWRIGHT_RUN_TIMEOUT_MS,
# DELVEWRIGHT_DIE_RETRY, DELVEWRIGHT_BRANCHES, …) — see docs/reference/tools.md.
# For a campaign with branches use `branch-runs.sh`, which is this loop once per
# branch, each in its own fresh world.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"

usage() {
  cat >&2 <<'USAGE'
usage: EULA=TRUE validation/bot-run.sh --project <compose-project>
                                       [--output <build-tree>] [--run-out <dir>]

  --project  REQUIRED. The compose project this ladder owns (e.g. dw-worker-7).
             Distinct per concurrent ladder; there is no default, because a
             shared default is what made ladders queue on each other.
  --output   Build tree to boot, relative to validation/ (default ./delve-output).
  --run-out  Where the bot writes its run report, relative to validation/
             (default ./run-out/<project>).
USAGE
  exit 2
}

project=""
output="${DELVE_OUTPUT:-./delve-output}"
run_out=""
while [ $# -gt 0 ]; do
  case "$1" in
    --project|-p) [ $# -ge 2 ] || usage; project="$2"; shift 2 ;;
    --output|-o)  [ $# -ge 2 ] || usage; output="$2";  shift 2 ;;
    --run-out)    [ $# -ge 2 ] || usage; run_out="$2"; shift 2 ;;
    -h|--help) usage ;;
    *) echo "bot-run: unknown argument '$1'" >&2; usage ;;
  esac
done

if [ -z "$project" ]; then
  echo "bot-run: --project <compose-project> is REQUIRED — two ladders sharing" >&2
  echo "  compose's default project would collide on volumes and tear each other down." >&2
  usage
fi
: "${EULA:?set EULA=TRUE to accept the Mojang EULA (https://aka.ms/MinecraftEULA)}"

[ -n "$run_out" ] || run_out="./run-out/$project"
mkdir -p "$here/${run_out#./}"

export DELVE_OUTPUT="$output"
# The image tag follows the project, because an image TAG is global to the daemon
# in exactly the way a container name is: two ladders building different trees into
# `delvewright/delve:local` race, and the loser boots the other ladder's delve.
export DELVE_IMAGE="delvewright/delve:$project"
export DW_BOT_OUT="$run_out"
COMPOSE=(docker compose -p "$project" -f "$here/compose.yaml" --profile validate)

cleanup() { "$here/fresh-volumes.sh" --project "$project" >/dev/null 2>&1 || true; }
trap cleanup EXIT

echo "==> bot ladder: project '$project', build tree '$output'"
# A stale world carries the scoreboard — completed objectives stay completed —
# and the bot then fails a step for a reason that has nothing to do with the
# delve (three misattributed red runs, island round 13).
"$here/fresh-volumes.sh" --project "$project"

set +e
"${COMPOSE[@]}" up --build --abort-on-container-exit --exit-code-from bot
rc=$?
set -e

report="$here/${run_out#./}/run-report.json"
if [ -f "$report" ]; then
  echo "==> run report: $report"
else
  echo "==> NO run report at $report — the server or bot never produced a result," >&2
  echo "    which is a validation-infrastructure fault, not a verdict on the delve." >&2
fi

echo "==> bot ladder: tearing down project '$project'"
"$here/fresh-volumes.sh" --project "$project"
trap - EXIT

if [ "$rc" -ne 0 ]; then
  echo "::error:: bot ladder FAILED in project '$project' (exit $rc)" >&2
else
  echo "==> bot ladder PASSED (project '$project')"
fi
exit "$rc"
