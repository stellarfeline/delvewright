#!/usr/bin/env python3
"""Every environment-gated job proves its own approval, or the tree is red.

WHY THIS EXISTS

`CARGO_REGISTRY_TOKEN` lives on the `crates-io` GitHub Environment, and an
environment is supposed to hold a run until a reviewer approves it. The
environment existed, held the token, and carried `protection_rules: []` — no
reviewer had ever been configured. A tag push walked through a job named
"publish to crates.io (owner approval)" and published two crates to crates.io
with nobody reviewing anything (ADR-0017, *What happened in practice*). A
crates.io version can never be deleted or reused.

`tools/assert-run-approved.sh` is the repository-side answer: it reads the run's
own approval history through the Actions API and refuses when no approval names
the environment, so the workflow asserts the CONSEQUENCE of the setting instead
of describing the setting. What it could not do is get itself invoked: it sits as
a hand-written first step in each job that needs it, which makes its PLACEMENT a
convention. ADR-0017 says so in the same paragraph, with the trigger for ending
that:

    if a second environment-gated job is ever added, it inherits nothing,
    and the checker is the work to redo.

A second one was added (`dsl-crate-publish.yml`'s `publish`, beside
`engine-release.yml`'s `publish-crates`). This is that checker.

WHAT IT ASSERTS, per job that declares `environment:`

1. the job has steps, and at least one of them runs a command;
2. the FIRST `run:` step invokes `tools/assert-run-approved.sh`;
3. the environment name it passes is the job's own.

Any of the three failing names the job and its file. (2) is keyed to the first
`run:` step rather than the first step because both live jobs check out the
repository first — `actions/checkout@v4` is a `uses:` step and touches nothing
outside the runner, whereas a `run:` is the first thing that can. Inside that
step's script, blank lines, comments and `set -...` lines may precede the
invocation; nothing else may.

Being FIRST is not only about the irreversible act. The guard reads
`GITHUB_RUN_ID` and `GITHUB_REPOSITORY` out of its own environment to decide
which run's approval history to ask about, and any earlier `run:` step can write
to `$GITHUB_ENV` — so a step ahead of it could point it at a different, approved
run and the guard would report a binding count of 1 about something else
entirely. Nothing before it can run, so nothing before it can redirect it.

VACUITY: a run that scans workflows and finds no environment-gated job exits
non-zero. There are two of them in this repository, so a zero is the discovery
rule having broken — a glob that matched nothing, a parser that lost the `jobs`
key — and reporting that as a clean tree is the exact shape this project refuses
(CLAUDE.md, *Vacuity*). The population is enumerated from the directory, both
YAML extensions, with nothing excluded, and the counts are printed on every run.

The workflow files are read through `tools/lib/workflow_yaml.py`, the one shared
parse rule, which refuses constructs it does not understand rather than guessing
past them. Deterministic, offline, stdlib-only python3.

Usage:
  python3 tools/check-approval-guard.py [--workflows DIR]

Exit 0 clean, 1 with one finding per job, 2 when the scan itself did not bind.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys
from typing import Any

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent / "lib"))

from workflow_yaml import WorkflowYamlError, load  # noqa: E402

REPO = pathlib.Path(__file__).resolve().parent.parent
GUARD = "tools/assert-run-approved.sh"

# `bash tools/assert-run-approved.sh crates-io`, `sh tools/…`, `./tools/…` or the
# bare path. The argument is captured so it can be compared with the job's own
# environment; a quoted argument is unquoted before comparison.
GUARD_CALL = re.compile(
    r"^(?:(?:bash|sh)\s+)?(?:\./)?" + re.escape(GUARD) + r"\s+(?P<env>\S+)\s*$"
)
# Lines allowed to precede the invocation inside the guard step's own script.
PRELUDE = re.compile(r"^set\s+[-+]\S*")


def workflows(directory: pathlib.Path) -> list[pathlib.Path]:
    """The population: every workflow file in the directory. Nothing is excluded."""
    return sorted(
        p
        for p in directory.iterdir()
        if p.is_file() and p.suffix in (".yml", ".yaml")
    )


def environment_name(value: Any) -> str | None:
    """The environment a job declares, as a string, or None if it cannot be read."""
    if isinstance(value, str):
        return value
    if isinstance(value, dict):
        name = value.get("name")
        return name if isinstance(name, str) else None
    return None


def first_run_step(steps: list[Any]) -> tuple[int, dict[str, Any]] | None:
    for index, step in enumerate(steps):
        if isinstance(step, dict) and step.get("run") is not None:
            return index, step
    return None


def guard_argument(script: str) -> str | None:
    """The environment `assert-run-approved.sh` is called with, if it leads the script."""
    for raw in script.splitlines():
        line = raw.strip()
        if line == "" or line.startswith("#") or PRELUDE.match(line):
            continue
        m = GUARD_CALL.match(line)
        if not m:
            return None
        return m.group("env").strip("'\"")
    return None


def step_label(step: dict[str, Any], index: int) -> str:
    name = step.get("name")
    return f"{name!r}" if isinstance(name, str) else f"step #{index + 1}"


def check(directory: pathlib.Path) -> tuple[list[str], int, int]:
    findings: list[str] = []
    files = workflows(directory)
    gated = 0

    for path in files:
        rel = path.relative_to(REPO) if path.is_relative_to(REPO) else path
        try:
            doc = load(path.read_text(encoding="utf-8"))
        except WorkflowYamlError as exc:
            findings.append(
                f"{rel}: the shared workflow parser refused this file — {exc}\n"
                f"    Nothing here can say whether its jobs are guarded, and an "
                f"unreadable gate is not a passed gate. Either simplify the "
                f"construct or teach tools/lib/workflow_yaml.py to read it."
            )
            continue
        jobs = (doc or {}).get("jobs")
        if not isinstance(jobs, dict):
            findings.append(
                f"{rel}: no `jobs:` mapping. A workflow file with no jobs is not "
                f"something this checker can bind to; if the file is not a "
                f"workflow, it does not belong in this directory."
            )
            continue

        for job_id, job in jobs.items():
            if not isinstance(job, dict) or "environment" not in job:
                continue
            gated += 1
            where = f"{rel}: job {job_id!r}"

            env = environment_name(job.get("environment"))
            if env is None:
                findings.append(
                    f"{where} declares `environment:` in a shape this checker "
                    f"cannot read a name out of ({job.get('environment')!r}).\n"
                    f"    Write it as `environment: <name>` or as a mapping with a "
                    f"`name:` key, so the guard's argument can be compared with it."
                )
                continue

            steps = job.get("steps")
            if not isinstance(steps, list) or not steps:
                findings.append(
                    f"{where} is gated on environment {env!r} and has no steps this "
                    f"checker can read.\n"
                    f"    A job that can reach an environment's secrets must run "
                    f"`{GUARD} {env}` as its first `run:` step; a job with no steps "
                    f"of its own (a reusable-workflow call, say) cannot, so it must "
                    f"not hold the environment."
                )
                continue

            found = first_run_step(steps)
            if found is None:
                findings.append(
                    f"{where} is gated on environment {env!r} and runs no command at "
                    f"all — so it never asserts that the run was approved.\n"
                    f"    Add `run: bash {GUARD} {env}` as its first `run:` step."
                )
                continue

            index, step = found
            argument = guard_argument(str(step["run"]))
            if argument is None:
                findings.append(
                    f"{where} is gated on environment {env!r}, but its first `run:` "
                    f"step ({step_label(step, index)}) does not call {GUARD}.\n"
                    f"    The environment holds a credential and its reviewer "
                    f"requirement lives in GitHub's settings, where nothing in this "
                    f"repository can see it — so the run must read its own approval "
                    f"history BEFORE it does anything else. Move "
                    f"`run: bash {GUARD} {env}` ahead of every other `run:` step in "
                    f"this job (a `uses: actions/checkout` before it is fine)."
                )
                continue

            if argument != env:
                findings.append(
                    f"{where} is gated on environment {env!r} but its guard asserts "
                    f"an approval for {argument!r}.\n"
                    f"    An approval recorded for a different environment is not an "
                    f"approval for this one: the guard would pass on a run nobody "
                    f"held for {env!r}. Pass {env!r} to {GUARD}."
                )

    return findings, len(files), gated


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--workflows",
        type=pathlib.Path,
        default=REPO / ".github" / "workflows",
        help="directory of workflow files to scan (default: .github/workflows)",
    )
    args = parser.parse_args(argv)
    directory: pathlib.Path = args.workflows

    if not directory.is_dir():
        print(
            f"check-approval-guard: FAIL — {directory} is not a directory, so the "
            f"scan bound to nothing",
            file=sys.stderr,
        )
        return 2

    guard = REPO / GUARD
    if not guard.is_file():
        # A gate that names a remedy owes a check that the remedy is reachable.
        print(
            f"check-approval-guard: FAIL — {GUARD} does not exist, and it is the "
            f"remedy every finding below would prescribe",
            file=sys.stderr,
        )
        return 2

    findings, scanned, gated = check(directory)
    binding = f"{scanned} workflow(s) scanned, {gated} environment-gated job(s)"

    if scanned == 0:
        print(
            f"check-approval-guard: FAIL — {binding}. A directory of workflows that "
            f"contains no workflow file is the enumeration breaking, not a clean "
            f"repository.",
            file=sys.stderr,
        )
        return 2
    if gated == 0:
        print(
            f"check-approval-guard: FAIL — {binding}. This gate exists because two "
            f"jobs declare `environment:` and each can reach a credential that buys "
            f"an irreversible act; finding none means the discovery rule broke (a "
            f"renamed key, a file this parser skipped), and reporting that as a pass "
            f"would leave the one-way door unguarded and green.",
            file=sys.stderr,
        )
        return 2

    if findings:
        print(
            f"check-approval-guard: {len(findings)} finding(s) — {binding}\n",
            file=sys.stderr,
        )
        for finding in findings:
            print(f"  {finding}", file=sys.stderr)
        return 1

    print(
        f"check-approval-guard: OK — {binding}; all {gated} call {GUARD} with their "
        f"own environment as their first `run:` step"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
