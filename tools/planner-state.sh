#!/usr/bin/env bash
# One page of planner state, computed from the tree — never recalled.
#
# WHY THIS EXISTS
#
# The planner is stateful, and for the project's first two weeks that state
# lived in the planner's context: which branches were in flight, which worker
# outputs were waiting to be delivered, which decisions were still unbound.
# Every new session was a differently-informed reconstruction of the previous
# one, and nothing could detect the difference. Every expensive coordination
# failure in the record — an undelivered worker result, a forgotten inventory,
# 36 unreclaimed worktrees filling the disk — was this, not a failure of any
# single check.
#
# The fix is the same trade this repo already makes for code: don't trust the
# author's memory, make the artifact carry the truth. Everything below is
# COMPUTED from git, gh and the decisions ledger. If this page and the
# planner's narrative disagree, the page wins.
#
# Reviewable in ~60 seconds. Sections:
#   1. both checkouts: current commit, dirtiness
#   2. worktrees beyond the main checkout (each is owned by an open dispatch,
#      or it is garbage — CLAUDE.md, owner 2026-08-11)
#   3. commits that exist on NO remote (the only unrecoverable git state)
#   4. open PRs, both repos (the in-flight set the planner must be able to hold)
#   5. decision ledger: open + unenforced rows (tools/check-decisions.py)
#
# Read-only. Never fails the session: a section that cannot be computed says so
# and the page continues — an absent answer is itself state worth seeing.
#
# INVOCATION (owner workflow, 2026-08-11: one long-lived session, so a
# session-start-only binding would almost never fire):
#   - SessionStart hook, unconditional — fires on startup, resume, AND after
#     every context compaction, which is exactly the moment the planner is a
#     reconstruction of its former self and most likely to be missing state.
#   - UserPromptSubmit hook with `--if-stale <hours>` — inside one long session
#     the page refreshes with the next user message once the stamp is older
#     than the window, and stays silent otherwise.
# The stamp lives in .git/ (never committed, per-checkout, survives nothing it
# shouldn't).

set -u

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

if [ "${1:-}" = "--if-stale" ]; then
  window_h="${2:-12}"
  stamp="$ROOT/.git/planner-state-stamp"
  now="$(date +%s)"
  if [ -f "$stamp" ]; then
    last="$(cat "$stamp" 2>/dev/null || echo 0)"
    age=$(( now - last ))
    [ "$age" -lt $(( window_h * 3600 )) ] && exit 0
  fi
  printf '%s\n' "$now" > "$stamp"
fi
CONTENT="$(cd "$ROOT" && cd "$(readlink campaigns 2>/dev/null || echo campaigns)" 2>/dev/null && pwd || true)"

section() { printf '\n== %s\n' "$1"; }

repo_line() { # <label> <path>
  local label="$1" path="$2"
  if [ -z "$path" ] || [ ! -d "$path" ]; then
    printf '%s: NOT FOUND\n' "$label"
    return
  fi
  local head dirty
  head="$(git -C "$path" log -1 --format='%h %s' 2>/dev/null || echo '?')"
  dirty="$(git -C "$path" status --porcelain 2>/dev/null | wc -l | tr -d ' ')"
  printf '%s: %s — %s\n' "$label" "$head" \
    "$([ "$dirty" = 0 ] && echo clean || echo "DIRTY ($dirty files)")"
}

worktrees() { # <path>
  git -C "$1" worktree list --porcelain 2>/dev/null |
    awk '/^worktree /{print $2}' | tail -n +2 | while read -r wt; do
      local_dirty="$(git -C "$wt" status --porcelain 2>/dev/null | wc -l | tr -d ' ')"
      unpushed="$(git -C "$wt" log --oneline '@{u}..HEAD' 2>/dev/null | wc -l | tr -d ' ')"
      printf '  %s  dirty=%s unpushed=%s\n' "$wt" "$local_dirty" "$unpushed"
    done
}

# Commits reachable from a local head but from no remote ref: the one category
# of git state that a machine failure actually destroys.
unpushed_commits() { # <path>
  git -C "$1" for-each-ref refs/heads --format='%(refname:short)' 2>/dev/null |
    while read -r b; do
      n="$(git -C "$1" rev-list --count "$b" --not --remotes 2>/dev/null || echo '?')"
      [ "$n" != 0 ] && printf '  %s: %s commit(s) on no remote\n' "$b" "$n"
    done
}

open_prs() { # <repo-dir>
  gh pr list --state open --limit 100 \
    --json number,title --jq '.[] | "  #\(.number)  \(.title)"' 2>/dev/null ||
    echo '  (gh unavailable — could not compute)'
}

section "checkouts"
repo_line "engine " "$ROOT"
repo_line "content" "$CONTENT"

section "worktrees beyond main (each owned by an open dispatch, or garbage)"
ew="$(worktrees "$ROOT")"; cw="$([ -n "$CONTENT" ] && worktrees "$CONTENT")"
printf '%s\n' "${ew:-  engine: none}"
printf '%s\n' "${cw:-  content: none}"

section "commits on NO remote (unrecoverable if this machine dies)"
eu="$(unpushed_commits "$ROOT")"; cu="$([ -n "$CONTENT" ] && unpushed_commits "$CONTENT")"
printf '%s\n' "${eu:-  engine: none}"
printf '%s\n' "${cu:-  content: none}"

section "open PRs — engine"
(cd "$ROOT" && open_prs "$ROOT")
section "open PRs — content"
[ -n "$CONTENT" ] && (cd "$CONTENT" && open_prs "$CONTENT") || echo '  (no content checkout)'

section "decision ledger (tools/check-decisions.py)"
python3 "$ROOT/tools/check-decisions.py" 2>&1 | sed 's/^/  /' ||
  echo '  (checker itself red — findings above are real, fix before anything else)'

exit 0
