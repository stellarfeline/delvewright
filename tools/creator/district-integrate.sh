#!/usr/bin/env bash
# district-integrate.sh — the planner's check before a worker's branch is merged
# (spec-0103 §8.2 gate 1): every path the branch changed since its baseline is
# inside the district's write set, by the same subcommand the worker's
# pre-commit hook calls.
#
#   tools/creator/district-integrate.sh <repo> <campaign-dir> <district> <baseline> <head> <prefabs-dir> [delvec]
#
# Exit 0 when every changed path is inside; 1 with each refused path named
# (`DW1008`) and its owner; 2 on a usage error.
set -euo pipefail

if [[ $# -lt 6 || $# -gt 7 ]]; then
  echo "usage: $0 <repo> <campaign-dir> <district> <baseline> <head> <prefabs-dir> [delvec]" >&2
  exit 2
fi
repo="$1"
campaign="$2"
district="$3"
baseline="$4"
head="$5"
prefabs="$6"
delvec="${7:-delvec}"

changed="$(git -C "$repo" diff --name-only "$baseline..$head")"
printf '%s\n' "$changed" | "$delvec" --prefabs "$prefabs" district writes "$campaign" "$district" --paths -
