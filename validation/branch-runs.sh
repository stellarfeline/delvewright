#!/usr/bin/env bash
# Branch runs (spec-0025 §3): walk every branch the tier selects, each in its own
# FRESH world, and merge the per-branch run reports into one branch report.
#
#   EULA=TRUE validation/branch-runs.sh --project dw-<id>        # release tier: all branches
#   EULA=TRUE DELVEWRIGHT_BRANCHES=branch/bolt validation/branch-runs.sh --project dw-<id>
#
# ## Why one world per branch
#
# A delve's progress is party state that only ever moves forward: an objective
# completed stays completed, a flag set stays set. There is no "replay the
# campaign" — so a second branch needs a second WORLD, not a second pass over the
# same server. This script is that loop: tear the stack down to verified-clean
# volumes (fresh-volumes.sh), bring it up, drive ONE branch, keep its report,
# repeat. `run.ts` therefore drives exactly one branch per invocation, and its
# report names every OTHER branch with the reason it did not run there.
#
# ## What it proves
#
# Green = every branch this tier selected was walked to its ending, and the merged
# report (`run-out/branch-runs.json`) says so branch by branch. A branch the tier
# skipped is in that report too, with its reason — spec-0025's rule that a skipped
# branch is NAMED, never silent, is what makes the artifact readable as coverage.
#
# The selection itself comes from `harness/src/branch-select.ts`, i.e. from the
# same code the run uses, so a tier can never select a branch the run then refuses.
#
# ## Isolation
#
# `--project <id>` (or `DW_COMPOSE_PROJECT`) is REQUIRED. `compose.yaml` pins no
# container name and publishes no host port, so the compose project is the ONLY
# name this ladder has — and it is a complete one: two branch-run loops on one
# host are independent, with no mutex and no queueing. An invocation without an
# id fails here rather than landing in compose's default project (`validation`),
# a shared name by another route.
set -euo pipefail

cd "$(dirname "$0")/.."
here="validation"

usage() {
  cat >&2 <<'USAGE'
usage: EULA=TRUE validation/branch-runs.sh --project <compose-project>
                                           [--out <dir>]

  --project  REQUIRED (or DW_COMPOSE_PROJECT). The compose project this ladder
             owns; distinct per concurrent ladder, no default.
  --out      Where merged + per-branch reports are filed (default
             validation/run-out/<project>; DW_RUN_OUT does the same).
USAGE
  exit 2
}

PROJECT="${DW_COMPOSE_PROJECT:-}"
RUN_OUT="${DW_RUN_OUT:-}"
while [ $# -gt 0 ]; do
  case "$1" in
    --project|-p) [ $# -ge 2 ] || usage; PROJECT="$2"; shift 2 ;;
    --out)        [ $# -ge 2 ] || usage; RUN_OUT="$2"; shift 2 ;;
    -h|--help) usage ;;
    *) echo "branch-runs: unknown argument '$1'" >&2; usage ;;
  esac
done
if [ -z "$PROJECT" ]; then
  echo "branch-runs: --project <compose-project> is REQUIRED — two ladders sharing" >&2
  echo "  compose's default project would collide on volumes and tear each other down." >&2
  usage
fi
: "${EULA:?set EULA=TRUE to accept the Mojang EULA (https://aka.ms/MinecraftEULA)}"

OUT="${DELVE_OUTPUT:-validation/delve-output}"
PLAN="$OUT/validation/branch-plan.json"
[ -n "$RUN_OUT" ] || RUN_OUT="validation/run-out/$PROJECT"
# Where the compose bot actually writes its report. The mount follows DW_BOT_OUT
# (a path relative to the compose file), and it is scoped to this project so two
# concurrent loops from one checkout cannot overwrite each other's reports.
# Reports are read from here and FILED under $RUN_OUT — before the split, a
# custom DW_RUN_OUT silently lost every per-branch report.
BOT_OUT="$here/run-out/$PROJECT"
export DW_BOT_OUT="./run-out/$PROJECT"
# The Dockerfile lives beside compose.yaml, never beside the build tree. Left to
# compose's default it is resolved relative to the CONTEXT, so any tree outside
# validation/ fails with `failed to read dockerfile`. bot-run.sh exports this;
# this script runs the same `validate` profile with `up --build` and was missed
# when that fix landed -- the fix enumerated the scripts that call compose
# rather than the directory that holds them.
export DELVE_DOCKERFILE="$here/Dockerfile.delve"
TIER="${DELVEWRIGHT_BRANCHES:-all}"

COMPOSE=(docker compose -p "$PROJECT" -f "$here/compose.yaml" --profile validate)

if [ ! -f "$PLAN" ]; then
  echo "::error:: $PLAN not found — this build declares no narrative branches." >&2
  echo "  Build a campaign with stage-4 branch_points first (delvec build … -o $OUT)." >&2
  exit 2
fi

echo "==> tier: DELVEWRIGHT_BRANCHES=$TIER"
selected="$(DELVEWRIGHT_BRANCHES="$TIER" node harness/src/branch-select.ts "$OUT/critical-path.json")"
if [ -z "$selected" ]; then
  echo "::error:: the tier selected no branch to run" >&2
  exit 1
fi
echo "==> selected: $(echo "$selected" | tr '\n' ' ')"

mkdir -p "$RUN_OUT" "$BOT_OUT"
MERGED="$RUN_OUT/branch-runs.json"
rm -f "$MERGED" "$BOT_OUT/run-report.json"

cleanup() {
  "$here/fresh-volumes.sh" --project "$PROJECT" >/dev/null 2>&1 || true
}
trap cleanup EXIT

status=0
reports=()
# Branches whose compose run exited without writing ANY run report — an INFRA
# failure (server never booted, bot never connected, mount broken), which is a
# different fact from a red run and from a tier skip, and must render as one.
# Entries are `branch=exit-code`.
infra=()
while IFS= read -r branch; do
  [ -n "$branch" ] || continue
  slug="$(printf '%s' "$branch" | tr '/' '-')"
  report="$RUN_OUT/run-report-$slug.json"
  rm -f "$report" "$BOT_OUT/run-report.json"

  echo
  echo "==> ${branch}: fresh world"
  "$here/fresh-volumes.sh" --project "$PROJECT"

  echo "==> ${branch}: run"
  set +e
  DELVEWRIGHT_BRANCH="$branch" DELVEWRIGHT_BRANCHES="$TIER" \
    "${COMPOSE[@]}" up --build --abort-on-container-exit --exit-code-from bot
  rc=$?
  set -e

  # The bot writes into the compose-mounted $BOT_OUT; file this branch's copy
  # under its own name in $RUN_OUT so the merge can read them all back.
  if [ -f "$BOT_OUT/run-report.json" ]; then
    mv "$BOT_OUT/run-report.json" "$report"
    reports+=("$report")
  else
    infra+=("$branch=$rc")
  fi

  if [ "$rc" -ne 0 ]; then
    echo "::error:: branch $branch FAILED (exit $rc)"
    status=1
  else
    echo "==> ${branch}: PASSED"
  fi
done <<EOF
$selected
EOF

echo
echo "==> merging per-branch reports into $MERGED"
# `${reports[@]+…}`: bash 3.2 (the dev environment's /bin/bash) treats an empty
# array's `[@]` expansion as an unbound variable under `set -u`. A run where every
# branch failed before writing a report must still reach the merge and SAY so —
# crashing here would hide the very thing this artifact exists to show.
# `DW_INFRA` carries the attempted-but-reportless branches (`branch=exit-code`
# lines): those rows must render as INFRA failures — distinct from a red run
# (the bot walked and failed a step) and from a tier skip (never attempted) —
# and no other session's "skipped" row may paper over them.
DW_INFRA="$(printf '%s\n' ${infra[@]+"${infra[@]}"})" \
python3 - "$MERGED" "$PLAN" ${reports[@]+"${reports[@]}"} <<'PY'
import json, os, sys
sys.stdout.reconfigure(newline="\n")  # CRLF-proof: tools/check-python-shell-newlines.py

merged_path, plan_path, *report_paths = sys.argv[1:]
plan = json.load(open(plan_path))
infra = dict(
    line.split("=", 1)
    for line in os.environ.get("DW_INFRA", "").splitlines()
    if "=" in line
)
# Start from the PLAN, not from the reports: a branch whose run produced no report
# at all must still appear. An absent branch is the exact failure mode spec-0025
# exists to end — coverage you cannot see is coverage you do not have.
rows = {
    b["id"]: {
        "branch": b["id"],
        "ran": False,
        "passed": False,
        "reason": "no run report was produced for this branch",
        "chronicle": b["chronicle"],
        "endings": b["endings"],
        "entry_commands": [],
    }
    for b in plan["branches"]
}
for branch, rc in infra.items():
    if branch in rows:
        rows[branch]["infra_failed"] = True
        rows[branch]["reason"] = (
            f"INFRA FAILURE: the compose run exited {rc} without writing a run "
            f"report — the server or bot never produced a result, so this is a "
            f"validation-infrastructure fault, not a branch verdict"
        )
tier = "all"
for p in report_paths:
    report = json.load(open(p))
    section = report.get("branches")
    if section is None:
        continue
    tier = section.get("tier", tier)
    for row in section["outcomes"]:
        # Only the session that DROVE a branch may claim it; every other session
        # lists it as unrun, and those rows must not overwrite the real result —
        # nor an infra-failure row, whose absence of a report is the finding.
        current = rows[row["branch"]]
        if row["ran"] or (not current["ran"] and not current.get("infra_failed")):
            rows[row["branch"]] = row

merged = {"version": 1, "campaign_id": plan["campaign_id"], "tier": tier,
          "branches": [rows[b["id"]] for b in plan["branches"]]}
with open(merged_path, "w") as fh:
    json.dump(merged, fh, indent=2)
    fh.write("\n")

for row in merged["branches"]:
    if row["passed"]:
        state = "RAN/passed"
    elif row["ran"]:
        state = "RAN/FAILED"
    elif row.get("infra_failed"):
        state = "INFRA-FAILED"
    else:
        state = "skipped"
    print(f"  {row['branch']}: {state}" + ("" if row["ran"] else f" — {row['reason']}"))
PY

exit "$status"
