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
#   0. the operating-practice page — the half of the constitution that is not
#      checked in (see below)
#   1. both checkouts: current commit, dirtiness
#   2. shallow checkouts — the one corruption that reports numbers, not errors
#   3. worktrees beyond the main checkout (each is owned by an open dispatch,
#      or it is garbage)
#   4. commits that exist on NO remote (the only unrecoverable git state)
#   5. open PRs, both repos (the in-flight set the planner must be able to hold)
#   6. decision ledger: open + unenforced rows
#
# WHY SHALLOWNESS IS ON THIS PAGE
#
# Every other failure a planner meets announces itself. A shallow repository does
# not: it answers ancestry questions with plausible wrong integers. Measured on
# throwaway clones — a branch 1 commit ahead of `origin/main` and 5 behind reads
# as 401 ahead and 1 behind once the repo is shallow, and `git merge origin/main`
# answers "refusing to merge unrelated histories". The refusal costs minutes; the
# counts are what someone resets or force-pushes on. It is also sticky and shared:
# the boundary lives in the object store, so one `--depth` fetch in any linked
# worktree shallows the main checkout too, and nothing ever says so.
#
# So it is reported here rather than left to be inferred from a strange git
# answer hours later and a directory away — on the same two events as everything
# else on this page, because a doc line telling someone to check is the UNRUN
# vacuity mode (CLAUDE.md).
#
# THE LOCAL HALF OF THE CONSTITUTION
#
# CLAUDE.md holds what anyone building Delvewright must obey. How this project
# is RUN — dispatch, review, merge gates, staging, decision sessions — is about
# one owner and one deployment, and a repository holds finished results rather
# than a person's decisions, so that half lives in CLAUDE.local.md, which is
# gitignored.
#
# It is NOT printed here, and that is the point. CLAUDE.local.md is loaded by
# the same memory loader as CLAUDE.md, so the local half carries the same force
# as the checked-in half: it is instructions, not text an agent is shown and may
# skim. An earlier design emitted the page from this script, which made it a
# tool result — the same standing as a doc line, and this project's own doctrine
# says a doc line is not an invocation.
#
# What remains here is the part a loader cannot do: SAY SO WHEN IT IS ABSENT. A
# gitignored file is missing on exactly the machine that has never had it — a
# fresh clone — and a missing memory file loads silently, which is the UNRUN
# vacuity mode wearing the fix's clothes. Missing or EMPTY => a refusal that
# names the file and says the session is running on half a constitution. Present
# => one line stating its size, so "loaded" is an observation rather than an
# assumption. That refusal is the ONE thing on this page that is not merely
# informational.
#
# The refusal is CONTENT, not an exit code, and deliberately so: this script's
# output is injected into the session, and a `UserPromptSubmit` hook that exits
# non-zero blocks the prompt instead of informing the agent. What has to reach
# the agent is the sentence, and the sentence is what it reads.
#
# Read-only, and it never fails the session: a section that cannot be computed
# says so and the page continues — an absent answer is itself state worth seeing.
#
# INVOCATION (one long-lived session, so a session-start-only binding would
# almost never fire):
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
  # In a linked worktree `.git` is a file; resolve the real directory. If it
  # cannot be resolved the throttle fails OPEN (emit, don't stamp): a missing
  # stamp home must never silence the page.
  gitdir="$(git -C "$ROOT" rev-parse --git-dir 2>/dev/null || true)"
  case "$gitdir" in /*) ;; ?*) gitdir="$ROOT/$gitdir" ;; esac
  if [ -n "$gitdir" ] && [ -d "$gitdir" ]; then
    mkdir -p "$gitdir"
    stamp="$gitdir/planner-state-stamp"
    now="$(date +%s)"
    if [ -f "$stamp" ]; then
      last="$(cat "$stamp" 2>/dev/null || echo 0)"
      age=$(( now - last ))
      [ "$age" -lt $(( window_h * 3600 )) ] && exit 0
    fi
    printf '%s\n' "$now" > "$stamp"
  fi
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

# Shallowness is a property of the OBJECT STORE, so one call answers for a
# checkout and every worktree linked to it. Prints nothing when the repo is
# whole; the caller counts what was examined either way, so "no finding" and
# "looked at nothing" stay distinguishable (CLAUDE.md: state the binding count).
shallow_line() { # <label> <path>
  local label="$1" path="$2"
  [ -n "$path" ] && [ -d "$path" ] || return 1
  local answer
  answer="$(git -C "$path" rev-parse --is-shallow-repository 2>/dev/null || echo '?')"
  case "$answer" in
    true)
      cat <<EOF
  SHALLOW — $label ($path)

  This repository has a truncated history, and so does every worktree sharing
  its object store. It does not fail; it ANSWERS WRONG. \`git merge origin/main\`
  says "refusing to merge unrelated histories", \`merge-base\` returns nothing,
  and ahead/behind counts come back as confident integers computed from the two
  commits that survived. Do not reset, rebase or force-push on any number this
  checkout produced until it is repaired.

  Repair it, then re-read anything you concluded from git here:
      git -C $path fetch --unshallow --no-tags

  Cause: a \`git fetch --depth=…\` or a \`git clone --depth=…\` run against a
  working checkout. That flag belongs to CI, whose checkout is disposable.
EOF
      return 0
      ;;
    false) return 1 ;;
    *)
      printf '  %s: could not be computed (git said %s)\n' "$label" "$answer"
      return 0
      ;;
  esac
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

LOCAL="$ROOT/docs/notes/private"
PRACTICE="$ROOT/CLAUDE.local.md"

section "operating practice — the half of the constitution that is not checked in"
if [ -s "$PRACTICE" ]; then
  echo "  present ($(wc -c < "$PRACTICE" | tr -d ' ') bytes) — loaded as instructions, not printed here."
else
  cat <<EOF
  REFUSED — $PRACTICE is missing.

  CLAUDE.md is HALF of this project's constitution. The other half — how work is
  dispatched, how a change is reviewed and merged, how a playtest round is
  staged, how a decision is put to the owner — is not checked in, and this
  session has not received it.

  Do not improvise any of it. Say plainly that you are running on half a
  constitution, and ask for the page before dispatching a worker, merging
  anything, staging a build, or asking the owner to decide something.

  This is expected on a fresh clone and on any machine that has never held the
  local half; recover it from the owner's machine or from local agent memory.
EOF
fi

section "checkouts"
repo_line "engine " "$ROOT"
repo_line "content" "$CONTENT"

section "history depth (a shallow repo answers with wrong numbers, never an error)"
examined=0; found=0
for pair in "engine:$ROOT" "content:$CONTENT"; do
  label="${pair%%:*}"; path="${pair#*:}"
  [ -n "$path" ] && [ -d "$path" ] || continue
  examined=$((examined + 1))
  if shallow_line "$label" "$path"; then found=$((found + 1)); fi
done
printf '  examined %s checkout(s); %s shallow\n' "$examined" "$found"
[ "$examined" = 0 ] && printf '  BINDING ZERO — no checkout could be examined, so this section proves nothing.\n'

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

section "ideas not yet graduated (docs/ideas.md — an idea leaves only by graduating or an owner 'declined')"
if [ -f "$ROOT/docs/ideas.md" ]; then
  awk -F'|' '/^\| IDEA-/ {
    status=$5; gsub(/^ +| +$/, "", status)
    if (status == "captured" || status == "elaborating") {
      id=$2; date=$3; idea=$4
      gsub(/^ +| +$/, "", id); gsub(/^ +| +$/, "", date); gsub(/^ +| +$/, "", idea)
      printf "  %s  %s  [%s]  %s\n", id, date, status, idea
      n++
    }
  } END { if (!n) print "  none pending" }' "$ROOT/docs/ideas.md"
else
  echo '  (no docs/ideas.md — could not compute)'
fi

# The ledger and its checker are gitignored for the same reason the practice
# page is, so this hook is the only thing that runs them. CI cannot.
section "decision ledger (docs/notes/private/check-decisions.py)"
if [ -s "$LOCAL/check-decisions.py" ]; then
  python3 "$LOCAL/check-decisions.py" 2>&1 | sed 's/^/  /' ||
    echo '  (checker itself red — findings above are real, fix before anything else)'
else
  echo '  REFUSED — docs/notes/private/check-decisions.py is missing, so no'
  echo '  recorded decision is bound to anything this session. Same recovery as'
  echo '  the operating-practice page above.'
fi

exit 0
