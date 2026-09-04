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

## "Did the tool answer" is a question about CONTENT, not about parseability

The first version of this reader asked whether stdout parsed as JSON, and
treated a parse as an answer. That is a syntactic carrier standing in for a
semantic property, and npm has a value that gets it backwards: on a transport
failure `npm audit --json` emits **valid JSON**, measured here as

    {"message": "request to …/security/advisories/bulk failed, reason: …",
     "error": {"summary": "", "detail": ""}}

top-level keys `['message', 'error']` and no `metadata` anywhere. So a timed-out
audit parsed cleanly, the retry loop never fired, and the reader refused with
`no per-severity counts (None)` — a message that reads like a shape change in
npm and was in fact the endpoint being down. Five minutes of runner time, npm's
default `fetch-timeout`, spent to produce a misleading sentence.

So the caller now says what an ANSWER looks like — `metadata.vulnerabilities`
carrying every severity for npm, a vulnerability count for cargo — and anything
else is the tool not having answered, whether or not it is JSON.

## The one live network reach, named

An advisory check cannot be offline: `npm audit` queries the registry's bulk
advisory endpoint and `cargo audit` clones the RustSec database. This repository
handles that shape by NAMING the reach in the step that makes it (the render
workspace's one pinned git fetch), and the same applies here.

The fetch is retried a bounded number of times before the refusal, and npm's own
per-request timeout is bounded too, because three attempts at its 300-second
default is fifteen minutes inside a required job. That is a retry of a
TRANSPORT, not of a verdict: the debug doctrine's "an intermittent red is never
re-run" is about a test whose result varies, and re-running it discards the
finding — here the finding IS that the endpoint did not answer, and it is still
reported, by name, once the attempts are spent.

## Refusals name the instrument and show what came back

Every refusal prints the tool's own version and the raw report's top-level keys
(and npm's `message` when it has one), so the next shape this reader does not
know says what it was rather than printing `None`.

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

# npm's own per-request timeout, in milliseconds. Its default is 300000, and
# three attempts at that is fifteen minutes of a required job spent discovering
# that a registry is down. One bulk POST that has not answered in two minutes is
# not going to.
NPM_FETCH_TIMEOUT_MS = 120000

# The npm majors whose `--json` audit report this reader was written against.
# `metadata.vulnerabilities` is a npm 7+ shape; npm 6 reported a different one,
# and a reader that silently mis-reads it would answer confidently about the
# wrong numbers. An npm outside this set is a refusal naming the version, not a
# guess. Node is pinned (`versions.toml [ci].node_version`) and npm ships with
# node, so this is an assertion about the shape rather than a second statement
# of a version the node pin already decides.
NPM_MAJORS_UNDERSTOOD = (7, 8, 9, 10, 11)


def top_keys(text: str) -> str:
    """What actually came back, for a refusal to print."""
    try:
        doc = json.loads(text)
    except json.JSONDecodeError:
        head = " ".join(text.split())[:120]
        return f"not JSON at all; it begins {head!r}" if head else "nothing at all"
    if isinstance(doc, dict):
        note = ""
        msg = doc.get("message")
        if isinstance(msg, str) and msg:
            note = f"; message: {msg}"
        return f"a JSON object with top-level keys {sorted(doc)}{note}"
    return f"JSON, but a {type(doc).__name__} rather than an object"


def fetching(
    cmd: list[str],
    answered,
    cwd: str | None = None,
) -> subprocess.CompletedProcess:
    """Run a command that must reach the network; return its first real ANSWER.

    `answered(stdout) -> bool` is the caller's statement of what an answer looks
    like, and it is a question about CONTENT. Parseability will not do: npm emits
    a valid JSON error object when the advisory endpoint fails, so a reader that
    accepts any JSON accepts an outage as a report.

    A retry here is of the TRANSPORT. The verdict is never re-rolled: whatever
    report comes back is the report, and an exhausted retry is reported as the
    audit having failed to answer rather than as a clean result.
    """
    last: subprocess.CompletedProcess | None = None
    for attempt in range(1, ATTEMPTS + 1):
        last = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True)
        if answered(last.stdout):
            return last
        if attempt < ATTEMPTS:
            print(
                f"check-advisories: attempt {attempt} of {ATTEMPTS} did not answer "
                f"(exit {last.returncode}); it returned {top_keys(last.stdout)}. "
                f"The advisory endpoint is a live reach. Retrying in "
                f"{BACKOFF_SECONDS}s.",
                file=sys.stderr,
            )
            time.sleep(BACKOFF_SECONDS)
    assert last is not None
    return last


def tool_version(cmd: list[str]) -> str:
    r = subprocess.run(cmd, capture_output=True, text=True)
    return " ".join((r.stdout + r.stderr).split()) or "(said nothing)"


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

    print(
        f"check-advisories (cargo): instrument — {tool_version([binary, '--version'])}",
        flush=True,
    )

    def answered(text: str) -> bool:
        """A cargo-audit ANSWER carries a vulnerability count. Anything else —
        an empty stdout, a panic, a database clone that failed — is the tool
        not having answered, whatever its exit code says."""
        try:
            doc = json.loads(text)
        except json.JSONDecodeError:
            return False
        return isinstance(doc, dict) and isinstance(
            doc.get("vulnerabilities", {}).get("count"), int
        )

    findings: list[str] = []
    warnings: list[str] = []
    crates = 0
    warned = 0
    for lock in locks:
        r = fetching([binary, "audit", "--json", "--file", str(repo / lock)], answered)
        if not answered(r.stdout):
            return die(
                f"`{binary} audit` did not answer for `{lock}` after {ATTEMPTS} "
                f"attempt(s) (exit {r.returncode}). It returned "
                f"{top_keys(r.stdout)}.\n"
                f"    An audit that did not answer is not an audit that found "
                f"nothing, and the advisory database is a live reach.\n"
                f"{r.stderr.strip()}"
            )
        report = json.loads(r.stdout)
        vulns = report["vulnerabilities"]
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

    version = tool_version(["npm", "--version"])
    print(f"check-advisories (npm): instrument — npm {version}", flush=True)
    major = version.split(".")[0]
    if not major.isdigit() or int(major) not in NPM_MAJORS_UNDERSTOOD:
        return die(
            f"npm {version} is outside the majors this reader was written "
            f"against ({', '.join(str(m) for m in NPM_MAJORS_UNDERSTOOD)}). "
            "`metadata.vulnerabilities` is a npm 7+ shape and npm 6 reported a "
            "different one, so reading this version's output would be answering "
            "confidently about numbers nobody checked. Teach this reader the "
            "shape, or hold the runner's node (versions.toml [ci].node_version) "
            "at a major whose npm it knows."
        )

    def answered(text: str) -> bool:
        """An npm ANSWER carries every severity under `metadata.vulnerabilities`.

        Parseability is NOT the test, and that is the whole lesson of this
        function: on a transport failure `npm audit --json` emits valid JSON —
        `{"message": …, "error": {…}}` — so a reader that accepts any JSON
        accepts an outage as a report and then refuses with `None`.
        """
        try:
            doc = json.loads(text)
        except json.JSONDecodeError:
            return False
        counts = doc.get("metadata", {}).get("vulnerabilities") if isinstance(doc, dict) else None
        return isinstance(counts, dict) and set(SEVERITIES) <= set(counts)

    r = fetching(
        # npm's own timeout is bounded here: three attempts at its 300-second
        # default is fifteen minutes of a required job, which is how the run
        # that produced this code spent five.
        ["npm", "audit", "--json", "--fetch-timeout", str(NPM_FETCH_TIMEOUT_MS)],
        answered,
        cwd=str(directory),
    )
    if not answered(r.stdout):
        return die(
            f"`npm audit --json` did not answer in `{directory}` after "
            f"{ATTEMPTS} attempt(s) (npm {version}, exit {r.returncode}). It "
            f"returned {top_keys(r.stdout)}.\n"
            "    An audit that did not answer is not an audit that found "
            "nothing. If those keys look like a REPORT rather than an error, "
            "npm has changed its shape and this reader needs teaching; if they "
            "are `message`/`error`, the advisory endpoint did not answer three "
            f"times.\n{r.stderr.strip()}"
        )
    report = json.loads(r.stdout)
    counts = report["metadata"]["vulnerabilities"]

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
