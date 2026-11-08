#!/usr/bin/env python3
"""Keep `.github/required-status-checks.txt` and `ci.yml`'s job names in lockstep.

WHY THIS EXISTS

Every CI job is a required status check. An advisory job is one whose red never
blocks a merge: only `gh pr merge`'s own refusal on an UNSTABLE state stops
anything, and `--admin` goes straight through. That is no gate at all for
`tier 2` (datapack load on the pinned server plus the entire generated PackTest
suite).

Requiring them all creates a hazard, and this checker exists for it: branch
protection matches a required context **by its name string**. Rename a job and
its required context simply never reports again — every PR blocks forever, and
the fix is itself a PR that cannot merge. This turns that deadlock into an
ordinary red on the PR that would have caused it.

The check is BIDIRECTIONAL on purpose:

- a name in the manifest with no matching job → the context can never report
  (the deadlock);
- a job with no line in the manifest → a gate nobody has to obey, which is the
  exact situation this task was opened to end. Silence is how it happened last
  time: seven gates drifted into advisory without anyone deciding they should be.

It reads only the repo, never the GitHub API — CI's token has `contents: read`
and cannot see branch protection, and a gate that needs a privileged token is a
gate that quietly stops running.

Exit 0 clean, 1 with one finding per line.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "tools"))

from lib import workflow_yaml  # noqa: E402

CI = REPO / ".github" / "workflows" / "ci.yml"
MANIFEST = REPO / ".github" / "required-status-checks.txt"

# Jobs that are DELIBERATELY advisory, each with the reason it may not block a
# merge. Empty on purpose — see the module docstring. Adding an entry here is a
# decision to let something fail without consequence, so it needs a reason a
# future reader can weigh, not just a name.
ADVISORY_JOBS: dict[str, str] = {}

# The job every other job needs (`.github/ci-reach.toml`'s filter). When it
# fails, every job needing it is skipped, and branch protection accepts a skipped
# required check — so its own red is the only thing standing between that pull
# request and `main`. It may never be advisory, and it must be required.
FILTER_JOB = "changes (which jobs a pull request reaches)"

# **How many advisory jobs this repository is allowed to hold.**
#
# The previous version of this file said "nothing else may be added here on this
# precedent" in a comment, which is a doc line — and this project's own rule is
# that a doc line is not an invocation. A second entry would have slipped in
# beside the first and left the checker green, which is precisely how `tier 2`
# sat unenforced while everyone believed it was blocking: the exact failure the
# entry above cites.
#
# So the list is BUDGETED, the same shape as check F in
# `check-capability-ownership.py`. Exceeding it is a hard red naming the budget.
# Raising the number is still possible — but it is a one-line diff to a constant
# whose name says what it is, which a reviewer sees, instead of one more key in a
# dict of keys, which nobody counts.
MAX_ADVISORY_JOBS = 1

# A job's display name is its `name:` (its key when it has none), read through
# the one workflow parser this tree's gates share (`tools/lib/workflow_yaml.py`).
# A job `name:` may carry `${{ matrix.<var> }}`: GitHub reports one context per
# matrix value, named with the value substituted, and branch protection matches
# each of those. The manifest therefore lists the EXPANDED names, and this
# checker expands the same way — every `${{ matrix.<var> }}` in the name from
# that job's `strategy.matrix.<var>` list, in order, all combinations. Any other
# expression in a name, or a matrix value that is not a plain list, is a
# finding: a context nobody can spell cannot be required.
MATRIX_REF = re.compile(r"\$\{\{\s*matrix\.([A-Za-z0-9_-]+)\s*\}\}")
ANY_EXPR = re.compile(r"\$\{\{")


# A job-level `if` that is true on every pull request. Anything else can skip the job.
ALWAYS_RUNS = {"always()", "!cancelled()", "${{ always() }}", "${{ !cancelled() }}"}


def expand_job_name(job_id: str, job: dict, findings: list[str]) -> list[str]:
    name = job.get("name")
    name = job_id if name is None else str(name)
    refs = MATRIX_REF.findall(name)
    if not refs:
        if ANY_EXPR.search(name):
            findings.append(
                f"job {job_id!r} is named by an expression this checker does not expand: {name!r}. "
                f"Only `${{{{ matrix.<var> }}}}` over a plain matrix list is expanded."
            )
        return [name]
    cond = job.get("if")
    if cond is not None and str(cond).strip() not in ALWAYS_RUNS:
        findings.append(
            f"job {job_id!r} is a matrix job named {name!r} and carries a job-level `if` ({str(cond).strip()!r}) "
            f"that can skip it. GitHub never expands a skipped matrix job: it reports one context, literally "
            f"the unexpanded template, and none of the required per-value names, so the pull request blocks for "
            f"ever. Run the job always (`if: ${{{{ !cancelled() }}}}`) and put the condition on each step."
        )
    strategy = job.get("strategy") or {}
    matrix = strategy.get("matrix") if isinstance(strategy, dict) else None
    names = [name]
    for var in refs:
        values = matrix.get(var) if isinstance(matrix, dict) else None
        if not isinstance(values, list) or not values or not all(isinstance(v, str) for v in values):
            findings.append(
                f"job {job_id!r} names itself by `${{{{ matrix.{var} }}}}` but `strategy.matrix.{var}` "
                f"is not a plain, non-empty list of scalars, so its contexts cannot be enumerated."
            )
            return []
        names = [
            MATRIX_REF.sub(lambda m, v=v: v if m.group(1) == var else m.group(0), n)
            for n in names
            for v in values
        ]
    if ANY_EXPR.search("".join(names)):
        findings.append(f"job {job_id!r}: an expression survives matrix expansion in {names[0]!r}")
    return names


def ci_job_names(findings: list[str]) -> list[str]:
    """Every context ci.yml can report, matrix names expanded (one parse rule: `tools/lib/workflow_yaml.py`)."""
    try:
        doc = workflow_yaml.normalize(workflow_yaml.load(CI.read_text(encoding="utf-8")))
    except workflow_yaml.WorkflowYamlError as e:
        findings.append(f"{CI.name} is not in the workflow subset this tree's gates parse: {e}")
        return []
    jobs = doc.get("jobs") if isinstance(doc, dict) else None
    if not isinstance(jobs, dict):
        return []
    names: list[str] = []
    for job_id, job in jobs.items():
        names.extend(expand_job_name(job_id, job if isinstance(job, dict) else {}, findings))
    return names


def manifest_contexts() -> list[str]:
    out: list[str] = []
    for line in MANIFEST.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if line and not line.startswith("#"):
            out.append(line)
    return out


def main() -> int:
    global CI, MANIFEST
    ap = argparse.ArgumentParser(description="keep the required-contexts manifest and ci.yml's job names in lockstep")
    ap.add_argument("--ci", default=None, help="the workflow file (default: this tree's ci.yml)")
    ap.add_argument("--manifest", default=None, help="the manifest (default: this tree's required-status-checks.txt)")
    args = ap.parse_args()
    if args.ci:
        CI = Path(args.ci)
    if args.manifest:
        MANIFEST = Path(args.manifest)
    if not CI.is_file():
        print(f"check-required-contexts: FAIL — {CI} is missing", file=sys.stderr)
        return 1
    if not MANIFEST.is_file():
        print(f"check-required-contexts: FAIL — {MANIFEST} is missing", file=sys.stderr)
        return 1

    findings: list[str] = []
    jobs = ci_job_names(findings)
    required = manifest_contexts()

    # Vacuity guard: parsing nothing is not a pass (CLAUDE.md — a green gate that
    # binds to nothing is VACUOUS). Both sides must be non-empty and state counts.
    if not jobs:
        print(
            "check-required-contexts: FAIL — parsed 0 job names from ci.yml; the "
            "workflow's `jobs:` block is not what this checker reads"
            + "".join(f"\n  {f}" for f in findings),
            file=sys.stderr,
        )
        return 1
    if not required:
        print(
            "check-required-contexts: FAIL — the manifest lists 0 required "
            "contexts; every gate would be advisory",
            file=sys.stderr,
        )
        return 1

    for ctx in required:
        if ctx not in jobs:
            findings.append(
                f"required context {ctx!r} matches no job `name:` in ci.yml.\n"
                f"    A required context that never reports blocks EVERY pull "
                f"request, including the one that would fix it. If you renamed "
                f"the job, add the new context to branch protection FIRST, then "
                f"merge the rename, then drop the old context."
            )

    for job in jobs:
        if job not in required and job not in ADVISORY_JOBS:
            findings.append(
                f"job {job!r} is not a required status check and is not listed as "
                f"deliberately advisory.\n"
                f"    Add it to .github/required-status-checks.txt AND to branch "
                f"protection's required_status_checks.contexts, or record it in "
                f"this checker's ADVISORY_JOBS with the reason it may fail without "
                f"consequence."
            )

    if FILTER_JOB not in jobs:
        findings.append(
            f"the filter job {FILTER_JOB!r} is not a job in ci.yml. Every other job "
            f"reads its group from it; if it was renamed, rename it here as well."
        )
    elif FILTER_JOB in ADVISORY_JOBS or FILTER_JOB not in required:
        findings.append(
            f"the filter job {FILTER_JOB!r} is not required. When it fails every "
            f"job that needs it is skipped, and a skipped required check passes "
            f"branch protection, so its own red must block the merge."
        )

    # The budget. A gate nothing enforces is the shape this whole file exists to
    # prevent, and an unbounded exemption list is one.
    if len(ADVISORY_JOBS) > MAX_ADVISORY_JOBS:
        findings.append(
            f"{len(ADVISORY_JOBS)} advisory job(s) are declared and the budget is "
            f"{MAX_ADVISORY_JOBS}: {', '.join(sorted(ADVISORY_JOBS))}.\n"
            f"    An advisory gate is one nobody has to obey. Make the new job "
            f"required — add it to {MANIFEST.name} AND to branch protection — or, "
            f"if it genuinely cannot gate yet, raise MAX_ADVISORY_JOBS in this "
            f"file and say in the same diff why this repository now needs two."
        )
    # A stale exemption is a budget the next one spends without anyone deciding to.
    for job in sorted(ADVISORY_JOBS):
        if job not in jobs:
            findings.append(
                f"advisory job {job!r} is not a job in ci.yml. Drop the entry — it "
                f"is holding a budget slot for a gate that no longer exists."
            )

    dupes = {n for n in required if required.count(n) > 1}
    for d in sorted(dupes):
        findings.append(f"required context {d!r} is listed more than once in the manifest.")

    if findings:
        print(
            f"check-required-contexts: {len(findings)} finding(s) "
            f"— {len(jobs)} ci.yml jobs, {len(required)} required contexts\n",
            file=sys.stderr,
        )
        for f in findings:
            print(f"  {f}", file=sys.stderr)
        return 1

    advisory = f", {len(ADVISORY_JOBS)} deliberately advisory" if ADVISORY_JOBS else ""
    print(
        f"check-required-contexts: OK — {len(jobs)} ci.yml jobs, all "
        f"{len(required)} required contexts resolve{advisory}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
