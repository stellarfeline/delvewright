#!/usr/bin/env bash
# Refuse to take an irreversible action unless this run carries a RECORDED
# environment approval.
#
# WHY THIS EXISTS
#
# `publish-crates` declared `environment: crates-io`, its job name said
# "(owner approval)", the workflow comment said "an environment with required
# reviewers", and ADR-0017 §4 named that reviewer as THE control on the one-way
# door. The environment existed and held CARGO_REGISTRY_TOKEN. It had
# `protection_rules: []` — no required reviewer had ever been configured. A tag
# push walked straight through and published `delvewright-dsl 0.1.0` and
# `delvec 1.1.0` to crates.io with nobody reviewing anything. crates.io versions
# cannot be deleted, only yanked.
#
# Every part of that gate was declared in the repository. The part that makes it
# bind lives in GitHub's settings, outside the repository, where nothing in the
# repository could see it — the same out-of-band shape as branch protection,
# which `tools/check-required-contexts.py` exists for. This is the executable
# form of the project's vacuity doctrine ("a green gate that binds to nothing is
# VACUOUS, not a pass"): the irreversible step asserts its own binding count,
# and a zero binding is a red instead of a publish.
#
# HOW: GitHub records environment approvals against the RUN. An approved run
# has at least one entry in .../actions/runs/<id>/approvals with state
# `approved`; a run that was never held has an empty list. That is exactly the
# distinction the repository could not otherwise make, and it needs only
# `actions: read` on the run's own repository — no privileged token, so this
# does not become a gate that quietly stops running.
#
# WHAT THIS DOES NOT PROVE: that the approver is a different human from whoever
# pushed the tag. GitHub's `prevent_self_review` covers that, and it is
# configured in the same out-of-band settings this script cannot read. What is
# asserted here is that a human passed through a review UI at all — which is
# precisely the step that did not happen on 2026-08-08.
#
# Usage: assert-run-approved.sh <environment-name>
# Env:   GITHUB_REPOSITORY, GITHUB_RUN_ID, GH_TOKEN (all supplied by Actions)

set -euo pipefail

env_name="${1:?usage: assert-run-approved.sh <environment-name>}"
repo="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is not set — this runs inside Actions}"
run_id="${GITHUB_RUN_ID:?GITHUB_RUN_ID is not set — this runs inside Actions}"

echo "asserting run ${run_id} carries a recorded approval for environment '${env_name}'"

# Never `curl | jq` here: a pipe hides the producer's exit status, and a
# readiness probe that SIGPIPEs its producer is how a false negative read as
# flakiness for months in this repo. Materialise, then parse.
approvals_json="$(mktemp)"
trap 'rm -f "$approvals_json"' EXIT

if ! gh api "repos/${repo}/actions/runs/${run_id}/approvals" > "$approvals_json"; then
  echo "FAIL: could not read the run's approval history."
  echo "      The job needs 'permissions: actions: read'. Refusing to proceed:"
  echo "      an unreadable gate is not a passed gate."
  exit 1
fi

count="$(jq --arg e "$env_name" \
  '[.[] | select(.state == "approved")
        | select([.environments[]?.name] | index($e))] | length' \
  "$approvals_json")"

# State the binding count, always — a proof that does not say what it bound to
# is how a zero binding survives review (playtest-methodology.md).
echo "binding count: ${count} recorded approval(s) for '${env_name}' on this run"

if [ "$count" -eq 0 ]; then
  cat <<EOF

FAIL: this run was never approved, so the gate did not bind.

  The job declares 'environment: ${env_name}', but GitHub recorded no approval
  for it on run ${run_id}. That means the environment has no required reviewer
  configured — the run was never held, and nobody reviewed anything.

  This is not something to retry. Fix the configuration:

    Settings -> Environments -> ${env_name} -> Required reviewers

  then re-run this workflow. Refusing to perform the irreversible step.

EOF
  exit 1
fi

echo "OK: the gate bound — proceeding."
