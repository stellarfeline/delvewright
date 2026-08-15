#!/usr/bin/env python3
"""Keep `.github/required-status-checks.txt` and `ci.yml`'s job names in lockstep.

WHY THIS EXISTS

Every CI job is a required status check. An advisory job is one whose red never
blocks a merge: only `gh pr merge`'s own refusal on an UNSTABLE state stops
anything, and `--admin` goes straight through. That is no gate at all for
`tier 2` (datapack load on the pinned server plus the entire generated PackTest
suite), the storybook engine-version marker, or the determinism gate on prefab
generators.

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

import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
CI = REPO / ".github" / "workflows" / "ci.yml"
MANIFEST = REPO / ".github" / "required-status-checks.txt"

# Jobs that are DELIBERATELY advisory, each with the reason it may not block a
# merge. Empty on purpose — see the module docstring. Adding an entry here is a
# decision to let something fail without consequence, so it needs a reason a
# future reader can weigh, not just a name.
ADVISORY_JOBS: dict[str, str] = {}

# A job's display name: `    name: <value>` nested under a job key. Quotes are
# optional in YAML and both forms appear in the wild, so strip them.
JOB_NAME = re.compile(r"^    name:\s*(.+?)\s*$")


def ci_job_names() -> list[str]:
    names: list[str] = []
    for line in CI.read_text(encoding="utf-8").splitlines():
        m = JOB_NAME.match(line)
        if m:
            names.append(m.group(1).strip("'\""))
    return names


def manifest_contexts() -> list[str]:
    out: list[str] = []
    for line in MANIFEST.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if line and not line.startswith("#"):
            out.append(line)
    return out


def main() -> int:
    if not CI.is_file():
        print(f"check-required-contexts: FAIL — {CI} is missing", file=sys.stderr)
        return 1
    if not MANIFEST.is_file():
        print(f"check-required-contexts: FAIL — {MANIFEST} is missing", file=sys.stderr)
        return 1

    jobs = ci_job_names()
    required = manifest_contexts()

    # Vacuity guard: parsing nothing is not a pass (CLAUDE.md — a green gate that
    # binds to nothing is VACUOUS). Both sides must be non-empty and state counts.
    if not jobs:
        print(
            "check-required-contexts: FAIL — parsed 0 job names from ci.yml; the "
            "`    name:` indentation this checker keys off has changed",
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

    findings: list[str] = []

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
