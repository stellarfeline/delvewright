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

import json
import pathlib
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
ADVISORY_JOBS: dict[str, str] = {
    # spec-0039. The gallery gate is CORRECT and currently RED: it reports units
    # that are neither written anywhere in the gallery nor proven refused, and
    # the residue includes `Locomotion::*`, which may be unbindable by
    # construction — `DW0454` refuses a traversal declaration that restates the
    # derived class, so binding one needs a world with a climb rather than a
    # field line. That contradicts a premise of an ACCEPTED spec (§3: "any legal
    # combination is expressible by some overlay"), so it is a spec question and
    # not something an implementation may decide.
    #
    # This entry exists because the alternative is worse in a specific,
    # recorded way. Branch protection matches a context by NAME, and live
    # protection does not carry this one; adding the name to the manifest while
    # the job is red would block every pull request forever, INCLUDING the one
    # that would fix it — the deadlock this whole file is a tripwire for. An
    # honest missing gate beats a gate that bricks the repository.
    #
    # It is not an escape hatch and it is not open-ended: the job runs on every
    # PR and its red is visible. It stops being advisory in the change that
    # takes the unaccounted count to zero, which is the same change that adds
    # the name back to `.github/required-status-checks.txt` and to branch
    # protection. Nothing else may be added here on this precedent — an
    # advisory gate is one nobody has to obey, which is how `tier 2` sat
    # unenforced while it was believed to be blocking.
    "gallery (coverage + build + baseline)": (
        "spec-0039: the gate is red on units a spec question owns; making it "
        "required before it is green would deadlock branch protection"
    ),
}

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

# The gallery entry's expiry, as something this checker can EVALUATE rather than
# recite. `gallery/baseline/header.json` records the coverage counts (spec-0039
# §6), so "it becomes required in the change that takes the unaccounted count to
# zero" is a committed number, not a promise. When it reaches zero the entry has
# outlived its reason and this file says so.
GALLERY_JOB = "gallery (coverage + build + baseline)"
_REPO = pathlib.Path(__file__).resolve().parent.parent
# The job is THREE gates, so the expiry reads all of them. Keying it off the
# coverage count alone would have demanded the job be made required the moment
# coverage reached zero, while the render arm was still red — which is the
# deadlock this file exists to prevent, arriving through the mechanism built to
# prevent it. Both numbers are committed by the tools that measure them.
GALLERY_HEADER = _REPO / "gallery/baseline/header.json"
GALLERY_RENDER = _REPO / "gallery/render-plan.json"

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

    # The gallery entry's expiry, evaluated.
    if GALLERY_JOB in ADVISORY_JOBS and GALLERY_HEADER.is_file():
        try:
            counts = json.loads(GALLERY_HEADER.read_text(encoding="utf-8")).get(
                "coverage", {}
            )
        except json.JSONDecodeError:
            counts = {}
        left = counts.get("units_unaccounted")
        try:
            render_left = json.loads(GALLERY_RENDER.read_text(encoding="utf-8")).get(
                "findings"
            )
        except (OSError, json.JSONDecodeError):
            render_left = None
        if left == 0 and render_left == 0:
            findings.append(
                f"{GALLERY_JOB!r} is still advisory and every gate it runs is clean "
                f"— ZERO unaccounted units and ZERO render findings. The condition "
                f"the entry was granted under has been met.\n"
                f"    Make it required: add the name to {MANIFEST.name} and to "
                f"branch protection, and delete its ADVISORY_JOBS entry."
            )
        elif left is None or render_left is None:
            findings.append(
                f"{GALLERY_JOB!r} is advisory and its own artifacts record no "
                f"counts (unaccounted={left!r}, render findings={render_left!r}), so "
                f"nothing here can tell whether the entry has outlived its reason. "
                f"Regenerate them: `tools/gallery-baseline.py --write` and "
                f"`tools/check-gallery-render.py --write`."
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
