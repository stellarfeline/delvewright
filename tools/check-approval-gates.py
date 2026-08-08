#!/usr/bin/env python3
"""Every environment-gated job must assert that its gate actually bound.

WHY THIS EXISTS (incident 2026-08-08, engine v1.1.0)

`publish-crates` declared `environment: crates-io`; the environment held
CARGO_REGISTRY_TOKEN and had `protection_rules: []`. No reviewer had ever been
configured, so the run was never held: a tag push published two crates to
crates.io unreviewed, through a job literally named "(owner approval)".

Everything that made the gate look real was in the repository. The one setting
that made it BIND was in GitHub's settings, where nothing in the repository
could see it. `tools/assert-run-approved.sh` closes that at run time by reading
the run's own approval history and refusing on a zero binding.

This file is the other half: it stops the assertion from being quietly dropped,
and — more importantly — it makes the obligation attach to the OBJECT CLASS
rather than to the one job that got burned. `environment:` on a job means "a
human is supposed to stand between this run and what the job does". Every job
that says that must prove it, not just `publish-crates`. A second such job added
later inherits the obligation on the day it is written, instead of needing its
own incident first (CLAUDE.md: generality is decided at the FIRST site).

Reads only the repository — no GitHub API, no token. A gate that needs a
privileged token is a gate that quietly stops running (task #31).

Three obligations per environment-gated job:

  1. it runs `tools/assert-run-approved.sh <env>` with ITS OWN environment name
     — a job asserting some other environment's approval proves nothing;
  2. the assertion comes before every other `run:` step, so nothing happens
     before the binding is proven;
  3. the job grants `actions: read`, without which the assertion cannot read the
     approval history and fails closed (correct, but as a late red rather than
     a configuration error anyone can see).

Exit 0 clean, 1 with one finding per line.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

WORKFLOWS = Path(".github/workflows")
ASSERT_SCRIPT = "tools/assert-run-approved.sh"


def _indent(line: str) -> int:
    return len(line) - len(line.lstrip(" "))


def _jobs(lines: list[str]) -> list[tuple[str, int, int]]:
    """(job_key, start_index, end_index) for each job in a workflow file."""
    out: list[tuple[str, int, int]] = []
    top = None
    for i, line in enumerate(lines):
        if re.match(r"^jobs:\s*$", line):
            top = i
            break
    if top is None:
        return out

    job_indent = None
    current: tuple[str, int] | None = None
    for i in range(top + 1, len(lines) + 1):
        line = lines[i] if i < len(lines) else "x"  # sentinel at column 0
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        ind = _indent(line)
        if ind == 0:  # left the jobs: block
            if current:
                out.append((current[0], current[1], i))
                current = None
            break
        if job_indent is None:
            job_indent = ind
        if ind == job_indent:
            m = re.match(r"^\s*([A-Za-z0-9_-]+):\s*$", line)
            if m:
                if current:
                    out.append((current[0], current[1], i))
                current = (m.group(1), i)
    if current:
        out.append((current[0], current[1], len(lines)))
    return out


def check_file(path: Path) -> list[str]:
    findings: list[str] = []
    lines = path.read_text(encoding="utf-8").splitlines()

    for job, start, end in _jobs(lines):
        body = lines[start:end]

        env = None
        for line in body:
            m = re.match(r"^\s*environment:\s*(\S+)\s*$", line)
            if m:
                env = m.group(1).strip("\"'")
                break
            # `environment:` with a nested `name:` mapping
            if re.match(r"^\s*environment:\s*$", line):
                idx = body.index(line)
                for nxt in body[idx + 1 : idx + 4]:
                    n = re.match(r"^\s*name:\s*(\S+)\s*$", nxt)
                    if n:
                        env = n.group(1).strip("\"'")
                        break
                break
        if env is None:
            continue

        where = f"{path.as_posix()}: job `{job}` (environment: {env})"

        text = "\n".join(body)

        # (2) The assertion must be the FIRST `run:` step. Steps before it may
        # only be actions (`uses:`) — checkout and caches move no bytes anyone
        # can see. Locate the first `run:` and read its whole block, so a
        # multi-line `run: |` still counts as one step.
        first_run_block = ""
        for i, line in enumerate(body):
            if re.search(r"^\s*(-\s+)?run:", line):
                run_indent = _indent(line)
                block = [line]
                for nxt in body[i + 1 :]:
                    if not nxt.strip():
                        block.append(nxt)
                        continue
                    if _indent(nxt) <= run_indent and re.match(
                        r"^\s*-?\s*(name|uses|run|with|env|id):", nxt
                    ):
                        break
                    block.append(nxt)
                first_run_block = "\n".join(block)
                break
        if ASSERT_SCRIPT not in text:
            findings.append(
                f"{where}: an environment-gated job that never asserts its gate "
                f"bound. Add `bash {ASSERT_SCRIPT} {env}` as its first run step — "
                f"a declared environment with no required reviewer publishes "
                f"silently (incident 2026-08-08)."
            )
            continue

        if not re.search(
            rf"{re.escape(ASSERT_SCRIPT)}\s+{re.escape(env)}\b", text
        ):
            findings.append(
                f"{where}: asserts an approval, but not for `{env}`. A job may "
                f"only prove the binding of the environment it actually declares."
            )

        elif ASSERT_SCRIPT not in first_run_block:
            findings.append(
                f"{where}: the approval assertion is not the FIRST `run:` step. "
                f"Nothing may run before the binding is proven — the step that "
                f"runs first is:\n    "
                + first_run_block.strip().splitlines()[0].strip()
            )

        # (3) actions: read, so the assertion can see the approval history.
        perms = re.search(r"^\s*permissions:\s*$", text, re.M)
        if not (perms and re.search(r"^\s*actions:\s*read\s*$", text, re.M)):
            findings.append(
                f"{where}: does not grant `actions: read`, so "
                f"{ASSERT_SCRIPT} cannot read the run's approval history."
            )

    return findings


def main() -> int:
    if not WORKFLOWS.is_dir():
        print(f"{WORKFLOWS}: not found — run from the repository root", file=sys.stderr)
        return 1

    findings: list[str] = []
    gated = 0
    for path in sorted(WORKFLOWS.glob("*.yml")) + sorted(WORKFLOWS.glob("*.yaml")):
        text = path.read_text(encoding="utf-8")
        gated += len(re.findall(r"^\s*environment:", text, re.M))
        findings.extend(check_file(path))

    # State the binding count. A checker that examined zero objects is not a
    # pass, and this one would silently become one the day the release workflow
    # is renamed or split (CLAUDE.md: unbound gates).
    print(f"environment-gated jobs examined: {gated}")
    if gated == 0:
        print(
            "FAIL: no job in .github/workflows declares an `environment:`. This "
            "check has nothing to bind to — either the release workflow moved, "
            "or the one-way door lost its gate entirely.",
            file=sys.stderr,
        )
        return 1

    sys.stdout.flush()
    for f in findings:
        print(f, file=sys.stderr)
    return 1 if findings else 0


if __name__ == "__main__":
    raise SystemExit(main())
