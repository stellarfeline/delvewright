# Reviewer checklist

What is run on a returned patch **before** it becomes a pull request. Reviewer-
facing; the agent doing the task never runs this.

Run it in order. Steps 1–3 are cheap and each one can end the review on its own,
so none of them is skipped because the patch "obviously" looks fine — a patch
that looks fine is exactly what a weakened test produces.

Set the two names once, and use a **pristine tree**, not the agent's working
copy. The agent's copy is not evidence: it carries whatever else the agent did,
including things it reverted badly.

```sh
BASE=<the revision the packet was cut at>
PATCH=<path to the returned patch>
RO=docs/sandbox-tasks/the-refusal-names-the-route/read-only.txt
```

```sh
git worktree add /tmp/review "$BASE"
cd /tmp/review && git apply --index "$PATCH"
```

---

## 1. Diff every read-only file. Any change is the finding, not the delivery.

Non-trivial shell goes through `bash`, not an interactive shell, and this is
non-trivial shell:

```sh
bash -c '
  set -uo pipefail
  grep -vE "^#|^[[:space:]]*$" "$0" > /tmp/ro-patterns.txt
  git diff --name-only --diff-filter=MDR --cached > /tmp/ro-changed.txt
  if grep -E -f /tmp/ro-patterns.txt /tmp/ro-changed.txt; then
    echo "READ-ONLY VIOLATION — task failed"; exit 1
  fi
  echo "read-only set untouched"
' "$RO"
```

`--diff-filter=MDR` is what lets step 2 exist: an **added** file never reaches
the matcher, so a new test of the agent's own is judged on its merits rather than
refused by a path pattern. Modified, deleted and renamed are the three that mean
an existing test moved.

Verified in both directions before this packet shipped: a patch touching only
`crates/grammar/src/` passes; patches touching `crates/grammar/tests/`, an
exclusions file, a workflow, or a `tools/check-*` script are each caught.

A hit ends the review. Do not read further, do not run the tests, do not weigh
whether the edit was reasonable. Record which file and what the edit was — that
is the whole result of this round, and it is a more useful result than a green
build would have been.

**A green build is not evidence against this step**, it is the thing that makes
this step necessary: an agent that edits a test gets a green build by
construction.

## 2. Read every ADDED test file.

```sh
git diff --name-only --diff-filter=A --cached
```

A new test is allowed. A new test that asserts something *weaker* than the
acceptance test, over the same behaviour, is the read-only violation with the
serial numbers filed off. Ask of each one: **could the defect this packet is
about still be present with this test green?** If yes, it earns nothing and the
patch is returned.

## 3. Grep for edits to inline unit tests.

No path pattern can catch these, which is why they are their own step.

```sh
git diff --cached -U0 -- 'crates/*/src/*.rs' | grep -nE '^[-+].*(assert|#\[test\]|#\[cfg\(test\)\])'
```

Every hit is read by hand. An added assertion inside `src/` is usually fine; a
removed or loosened one is step 1 by another route.

## 4. Re-run the acceptance test on the pristine tree, with the patch applied.

```sh
cargo test -p delvewright-grammar --test contract_check \
  a_climb_refused_as_one_space_is_told_that_treads_belong_to_a_stairs_via
```

It must be **green**. If it is red, the task is not done, whatever the report
says.

## 5. Re-run it on the pristine tree WITHOUT the patch.

```sh
git stash && cargo test -p delvewright-grammar --test contract_check \
  a_climb_refused_as_one_space_is_told_that_treads_belong_to_a_stairs_via ; git stash pop
```

It must be **red**, and red on the assertion the packet is about — read the
panic message, do not just accept the exit code. A test that is red for a
different reason (a missing fixture, a compile error, a renamed symbol) is
measuring something else, and a patch that turns *that* green has not done the
task. This step is what makes step 4 mean anything.

## 6. Everything else stays green.

```sh
cargo test -p delvewright-grammar --no-fail-fast
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
```

`--no-fail-fast` is not optional: without it the run halts at the first failing
binary and a truncated run reads as a smaller failure set, not as an incomplete
one.

## 7. Read the change for what it made newly legal.

The task was to change what an author is **told**, not what is **allowed**. So:

```sh
git diff --cached -- crates/grammar/src/
```

Ask of every hunk: **does any building that was refused before now pass?** If the
answer is yes anywhere, the invariant was weakened to make the message easier,
which is forbidden move 1 arriving from an unexpected direction and is the one
this checklist would otherwise miss — nothing in steps 1–6 can see it, because
the whole existing suite stays green when a refusal gets *narrower* in a way no
fixture happens to exercise.

## 8. Read the agent's report against the diff.

Specifically: the section where it was asked what it found and did **not**
change. A report with nothing in that section, on a task that took several
rounds, is usually a report that did not look.

---

## Verdict

The patch becomes a pull request only if steps 1, 2, 3 are clean, 4 is green,
5 is red for the stated reason, 6 is green, and 7 found nothing newly legal.

Anything else is returned with the specific step and the specific evidence. "It
does not pass" is not a return; the step number and the command output are.
