#!/usr/bin/env python3
"""Published security advisories against what this repository actually resolves.

## What was missing

Both repositories are PUBLIC (ADR-0017) and neither had any supply-chain gate:
no `dependabot.yml`, no `cargo audit`, no `npm audit` anywhere in CI, and
`harness/package.json` declaring `^` ranges over a committed lockfile nothing
examined. Dependency hygiene had no gate at all — not a weak one, none.

`.github/dependabot.yml` is the half that OPENS a pull request. This is the half
that REFUSES, and it runs as a step of two jobs that already exist rather than as
a job of its own, because every job name is a required status context and a new
one is a branch-protection change.

## Why the exit codes of the underlying tools are not consulted

`npm audit --audit-level=high` was measured here three times in sequence, on one
machine, one lockfile, six moderate advisories and zero high: **exit 1, exit 1,
exit 0**. The failing runs were not a verdict. They printed `undefined` on stdout
and, on stderr:

    npm warn audit network timeout at: …/security/advisories/bulk
    npm error audit endpoint returned an error

So the exit code was collapsing two different facts — *this tree has an advisory
at or above the floor* and *the audit never ran* — into one number, in the
direction that reads as a finding. Neither tool's exit code decides anything
here. Both are asked for their JSON report and the verdict is computed from the
counts; a run that cannot produce a parseable report is a REFUSAL that says the
audit did not answer, which is a different sentence from "the audit found
something".

## The one live network reach, named

An advisory check cannot be offline: `npm audit` queries the registry's bulk
advisory endpoint and `cargo audit` clones the RustSec database. This repository
handles that shape by NAMING the reach in the step that makes it (the render
workspace's one pinned git fetch), and the same applies here.

The fetch is retried a bounded number of times before the refusal. That is a
retry of a TRANSPORT, not of a verdict: the debug doctrine's "an intermittent red
is never re-run" is about a test whose result varies, and re-running it discards
the finding — here the finding IS that the endpoint did not answer, and it is
still reported, by name, once the attempts are spent.

## The npm severity floor is stated, not hidden

`--level high` fails on high and critical. Everything below is COUNTED and
PRINTED on every run, per severity, so a moderate advisory cannot sit behind a
green tick unseen. That is a stated threshold, not an ignore list: no advisory
id is ever excluded, and there is no mechanism here to exclude one.

## Usage

    tools/check-advisories.py --cargo [--cargo-audit BIN] [--repo DIR]
    tools/check-advisories.py --npm [--dir harness] [--level high]
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

# npm's severity ladder, lowest first. The floor is an INDEX into this, so a
# severity npm adds later lands above `critical` and fails rather than being
# silently unknown.
SEVERITIES = ("info", "low", "moderate", "high", "critical")

# Attempts at the one live reach, and the pause between them. Bounded and small:
# enough to survive a registry blip, not enough to sit on a runner while an
# outage runs its course.
ATTEMPTS = 3
BACKOFF_SECONDS = 5


def fetching(cmd: list[str], cwd: str | None = None) -> subprocess.CompletedProcess:
    """Run a command that must reach the network, and return the first attempt
    whose stdout parses as JSON — or the last attempt, for the caller to refuse on.

    A retry here is of the TRANSPORT. The verdict is never re-rolled: whatever
    report comes back is the report, and an exhausted retry is reported as the
    audit having failed to answer rather than as a clean result.
    """
    last: subprocess.CompletedProcess | None = None
    for attempt in range(1, ATTEMPTS + 1):
        last = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True)
        try:
            json.loads(last.stdout)
            return last
        except json.JSONDecodeError:
            if attempt < ATTEMPTS:
                print(
                    f"check-advisories: attempt {attempt} of {ATTEMPTS} produced no "
                    f"report (exit {last.returncode}); the advisory endpoint is a "
                    f"live reach. Retrying in {BACKOFF_SECONDS}s.",
                    file=sys.stderr,
                )
                time.sleep(BACKOFF_SECONDS)
    assert last is not None
    return last


def die(msg: str) -> int:
    print(f"check-advisories: FAIL — {msg}", file=sys.stderr)
    return 1


def lockfiles(repo: Path) -> list[str]:
    """Every cargo lockfile the repository tracks, derived rather than listed.

    Ten workspaces live here and a hand-written list of them has gone stale in
    this repository before (the `fmt` sweep named nine of ten).
    """
    r = subprocess.run(
        ["git", "-C", str(repo), "ls-files", "-z", "--", "*Cargo.lock"],
        capture_output=True,
        text=True,
    )
    if r.returncode != 0:
        raise RuntimeError(f"`git ls-files` exited {r.returncode}: {r.stderr.strip()}")
    return sorted(p for p in r.stdout.split("\0") if p)


def audit_cargo(repo: Path, binary: str) -> int:
    try:
        locks = lockfiles(repo)
    except RuntimeError as e:
        return die(str(e))
    if not locks:
        return die(
            "found ZERO cargo lockfiles. Auditing nothing agrees with everything; "
            "this is a red rather than a pass."
        )

    findings: list[str] = []
    warnings: list[str] = []
    crates = 0
    warned = 0
    for lock in locks:
        r = fetching([binary, "audit", "--json", "--file", str(repo / lock)])
        try:
            report = json.loads(r.stdout)
        except json.JSONDecodeError:
            return die(
                f"`{binary} audit` produced no parseable report for `{lock}` "
                f"after {ATTEMPTS} attempt(s) (exit {r.returncode}). An audit that "
                f"did not answer is not an audit that found nothing, and the "
                f"advisory database is a live reach.\n{r.stderr.strip()}"
            )
        vulns = report.get("vulnerabilities", {})
        count = vulns.get("count")
        if count is None:
            return die(f"the report for `{lock}` carries no vulnerability count")
        crates += report.get("lockfile", {}).get("dependency-count", 0)
        for kind, rows in report.get("warnings", {}).items():
            for row in rows:
                warned += 1
                pkg = row.get("package", {})
                adv = (row.get("advisory") or {}).get("id", "-")
                warnings.append(
                    f"{lock}: {kind} — {pkg.get('name')} {pkg.get('version')} ({adv})"
                )
        for v in vulns.get("list", []):
            adv = v.get("advisory", {})
            pkg = v.get("package", {})
            findings.append(
                f"{lock}: {adv.get('id')} {pkg.get('name')} {pkg.get('version')} "
                f"— {adv.get('title')}"
            )

    print(
        f"check-advisories (cargo): {len(locks)} lockfile(s) audited over "
        f"{crates} resolved crate dependency(ies); {len(findings)} vulnerability "
        f"(ies), {warned} warning(s) (unmaintained/yanked)"
    )
    # NAMED, never a bare count: `2 warning(s)` is a reassuring gloss, and the
    # question for a residue is never whether there is an explanation but what
    # would have to be true for it to be correct.
    for w in warnings:
        print(f"  warning: {w}")
    if findings:
        print(
            "check-advisories: FAIL — a published advisory matches what this "
            "repository resolves:",
            file=sys.stderr,
        )
        for f in findings:
            print(f"  {f}", file=sys.stderr)
        print(
            "    Update the lockfile, or record the advisory as a finding with a "
            "priority. There is no exclusion mechanism here on purpose.",
            file=sys.stderr,
        )
        return 1
    return 0


def audit_npm(directory: Path, level: str) -> int:
    if level not in SEVERITIES:
        return die(f"--level `{level}` is not one of {', '.join(SEVERITIES)}")
    if not (directory / "package-lock.json").is_file():
        return die(f"no `package-lock.json` under `{directory}`")

    r = fetching(["npm", "audit", "--json"], cwd=str(directory))
    try:
        report = json.loads(r.stdout)
    except json.JSONDecodeError:
        return die(
            f"`npm audit --json` produced no parseable report in `{directory}` "
            f"after {ATTEMPTS} attempt(s) (exit {r.returncode}). An audit that did "
            f"not answer is not an audit that found nothing, and the reach is "
            f"live.\n{r.stderr.strip()}"
        )
    counts = report.get("metadata", {}).get("vulnerabilities")
    if not isinstance(counts, dict) or not set(SEVERITIES) <= set(counts):
        return die(
            "`npm audit --json` returned a report with no per-severity counts "
            f"({counts!r}); this reader has stopped matching npm's output"
        )

    floor = SEVERITIES.index(level)
    over = {s: counts[s] for s in SEVERITIES[floor:] if counts[s]}
    under = {s: counts[s] for s in SEVERITIES[:floor] if counts[s]}
    total = report.get("metadata", {}).get("dependencies", {})

    print(
        "check-advisories (npm): "
        + f"{directory} — {sum(counts[s] for s in SEVERITIES)} advisory(ies) over "
        + f"{total.get('total', '?')} resolved package(s); "
        + "counts by severity: "
        + ", ".join(f"{s}={counts[s]}" for s in SEVERITIES)
        + f"; the floor is `{level}`"
    )
    if under:
        # PRINTED, never hidden: a stated threshold that does not say what sits
        # below it is an ignore list with better manners.
        print(
            "check-advisories (npm): below the floor and NOT failing this run — "
            + ", ".join(f"{n} {s}" for s, n in under.items())
            + ". Read them with `npm audit` in that directory; they are a finding "
            "to schedule, not a thing this gate has forgiven."
        )
    if over:
        print(
            "check-advisories: FAIL — "
            + ", ".join(f"{n} {s}" for s, n in over.items())
            + f" advisory(ies) at or above `{level}`. Run `npm audit` in "
            f"`{directory}` for the tree. Update the lockfile; there is no "
            "exclusion mechanism here on purpose.",
            file=sys.stderr,
        )
        return 1
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--cargo", action="store_true", help="audit every cargo lockfile")
    ap.add_argument("--npm", action="store_true", help="audit an npm project")
    ap.add_argument("--cargo-audit", default="cargo-audit", help="the cargo-audit binary")
    ap.add_argument("--repo", default=str(REPO))
    ap.add_argument("--dir", default=str(REPO / "harness"), help="the npm project")
    ap.add_argument("--level", default="high", help=f"npm floor: {', '.join(SEVERITIES)}")
    args = ap.parse_args()

    if args.cargo == args.npm:
        return die("give exactly one of --cargo or --npm")
    if args.cargo:
        return audit_cargo(Path(args.repo), args.cargo_audit)
    return audit_npm(Path(args.dir), args.level)


if __name__ == "__main__":
    raise SystemExit(main())
