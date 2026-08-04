#!/usr/bin/env bash
# Branch runs (spec-0025 §3): walk every branch the tier selects, each in its own
# FRESH world, and merge the per-branch run reports into one branch report.
#
#   EULA=TRUE validation/branch-runs.sh                          # release tier: all branches
#   EULA=TRUE DELVEWRIGHT_BRANCHES=branch/bolt validation/branch-runs.sh
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
# Isolation (CLAUDE.md / mutex.sh "Worker isolation"): set DW_COMPOSE_PROJECT to
# run in your own compose project with the worker override, which drops the pinned
# container names and the 25565 host binding.
set -euo pipefail

cd "$(dirname "$0")/.."
here="validation"
: "${EULA:?set EULA=TRUE to accept the Mojang EULA (https://aka.ms/MinecraftEULA)}"

OUT="${DELVE_OUTPUT:-validation/delve-output}"
PLAN="$OUT/validation/branch-plan.json"
RUN_OUT="${DW_RUN_OUT:-validation/run-out}"
PROJECT="${DW_COMPOSE_PROJECT:-}"
TIER="${DELVEWRIGHT_BRANCHES:-all}"

COMPOSE=(docker compose)
[ -n "$PROJECT" ] && COMPOSE+=(-p "$PROJECT")
COMPOSE+=(-f "$here/compose.yaml")
# Worker isolation: no pinned container names, no host port.
[ -n "$PROJECT" ] && COMPOSE+=(-f "$here/worker-override.yaml")
COMPOSE+=(--profile validate)

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

mkdir -p "$RUN_OUT"
MERGED="$RUN_OUT/branch-runs.json"
rm -f "$MERGED" "$RUN_OUT/run-report.json"

cleanup() {
  if [ -n "$PROJECT" ]; then
    "$here/fresh-volumes.sh" --project "$PROJECT" >/dev/null 2>&1 || true
  else
    "${COMPOSE[@]}" down -v --remove-orphans >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

status=0
reports=()
while IFS= read -r branch; do
  [ -n "$branch" ] || continue
  slug="$(printf '%s' "$branch" | tr '/' '-')"
  report="$RUN_OUT/run-report-$slug.json"
  rm -f "$report" "$RUN_OUT/run-report.json"

  echo
  echo "==> ${branch}: fresh world"
  if [ -n "$PROJECT" ]; then
    "$here/fresh-volumes.sh" --project "$PROJECT"
  else
    "${COMPOSE[@]}" down -v --remove-orphans >/dev/null 2>&1 || true
  fi

  echo "==> ${branch}: run"
  set +e
  DELVEWRIGHT_BRANCH="$branch" DELVEWRIGHT_BRANCHES="$TIER" \
    "${COMPOSE[@]}" up --build --abort-on-container-exit --exit-code-from bot
  rc=$?
  set -e

  # The bot writes into the mounted ./run-out; keep this branch's copy under its
  # own name so the merge can read them all back.
  if [ -f "$RUN_OUT/run-report.json" ]; then
    mv "$RUN_OUT/run-report.json" "$report"
    reports+=("$report")
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
python3 - "$MERGED" "$PLAN" ${reports[@]+"${reports[@]}"} <<'PY'
import json, sys

merged_path, plan_path, *report_paths = sys.argv[1:]
plan = json.load(open(plan_path))
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
tier = "all"
for p in report_paths:
    report = json.load(open(p))
    section = report.get("branches")
    if section is None:
        continue
    tier = section.get("tier", tier)
    for row in section["outcomes"]:
        # Only the session that DROVE a branch may claim it; every other session
        # lists it as unrun, and those rows must not overwrite the real result.
        if row["ran"] or not rows[row["branch"]]["ran"]:
            rows[row["branch"]] = row

merged = {"version": 1, "campaign_id": plan["campaign_id"], "tier": tier,
          "branches": [rows[b["id"]] for b in plan["branches"]]}
with open(merged_path, "w") as fh:
    json.dump(merged, fh, indent=2)
    fh.write("\n")

for row in merged["branches"]:
    state = "RAN/passed" if row["passed"] else ("RAN/FAILED" if row["ran"] else "skipped")
    print(f"  {row['branch']}: {state}" + ("" if row["ran"] else f" — {row['reason']}"))
PY

exit "$status"
