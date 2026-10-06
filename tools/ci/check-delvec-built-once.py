#!/usr/bin/env python3
"""One `delvec` per CI run: the `delvec-binary` job builds it, every job that
only RUNS it takes that build, and nothing else compiles the root workspace.

WHY THIS EXISTS

Five `ci.yml` jobs each compiled the same debug `delvec` from the same tree only
to run it, one to three minutes apiece. They now take the binary the
`delvec-binary` job uploads, checked against the sha256 that job reports
(`.github/actions/delvec-binary`). The shape that would undo it is quiet: a new
step that writes `cargo run -p delvec -- …` instead of `target/debug/delvec …`
works, is green, and compiles the engine again — and in a job that holds the
installed binary at `target/debug/delvec`, it silently replaces the instrument
the run was handed with one of its own. This refuses that shape.

WHAT IT ASSERTS

1. The build job exists, runs exactly `BUILD`, uploads the artifact `ARTIFACT`,
   and declares the `sha256` output its consumers verify against.
2. A job outside `COMPILES` invokes cargo only against another workspace (a
   `--manifest-path` naming `prefabs/`), in its own `run:` text and in every
   repository script that text invokes, followed through the shell libraries
   those scripts source.
3. A job outside `COMPILES` that runs `target/debug/delvec` needs the build job
   and takes the binary through `ACTION`, with the sha256 read from
   `needs.delvec-binary.outputs.sha256`, before the first step that runs it.
4. `ACTION` is the only place the artifact is downloaded.

`COMPILES` is the declared list of jobs that compile the root workspace for a
reason of their own, each with that reason; a job enters it by an edit here.

WHAT IT DOES NOT FOLLOW, said on every run: a script a Python tool starts
through `subprocess`. Those are counted as unread. Every Python tool the
consumer jobs run today resolves its engine through `tools/lib/delvec_bin.py`,
which has no build path, and receives it through `--delvec`.

Reads the workflow through `tools/lib/workflow_yaml.py`, the one shared parse
rule. States its binding counts; a run that judges zero consumers exits 2.
Deterministic, offline, stdlib-only python3.

Usage:
  python3 tools/ci/check-delvec-built-once.py [--workflow PATH] [--repo DIR]

Exit 0 clean, 1 with findings, 2 when the inputs cannot be read or bind nothing.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys
from typing import Any

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "lib"))

from workflow_yaml import WorkflowYamlError, load  # noqa: E402

REPO = pathlib.Path(__file__).resolve().parents[2]
WORKFLOW = REPO / ".github" / "workflows" / "ci.yml"

PRODUCER = "delvec-binary"
BUILD = "cargo build --locked -p delvec --bin delvec"
ARTIFACT = "delvec-binary"
ACTION = "./.github/actions/delvec-binary"
SHA_INPUT = "${{ needs.delvec-binary.outputs.sha256 }}"
BINARY = "target/debug/delvec"

# The jobs that compile the root workspace for a reason of their own.
COMPILES: dict[str, str] = {
    PRODUCER: "it is the one build every other job takes",
    "rust": "clippy and `cargo test --workspace` compile every target of every member, which no binary stands in for",
    "determinism-cross-os": "the macOS half of the comparison is a binary built by macOS's toolchain; a Linux artifact cannot be it",
    "engine-shelf": "`cargo check` of `delvec` for every release target is the whole of what it proves",
    "publishable": "it builds the packaged crates the way `cargo install` does, outside the workspace",
    "dsl-crate-version": "`cargo package` of `crates/dsl` is how it reads the bytes crates.io would receive",
}

CARGO_COMPILE = re.compile(r"\bcargo\s+(?:\+\S+\s+)?(?P<verb>build|run|test|bench|clippy|check|install|package|publish|doc)\b")
OTHER_WORKSPACE = re.compile(r"--manifest-path[\s=]+[\"']?(?:\$\S*/)?prefabs/")
PY_CARGO = re.compile(
    r"[\"']cargo[\"']\s*,\s*[\"'](?P<verb>build|run|test|bench|clippy|check|install|package|publish|doc)[\"'](?P<rest>[^\]]*)\]",
    re.S,
)
PY_OTHER_WORKSPACE = re.compile(r"[\"']--manifest-path[\"']")
SCRIPT = re.compile(r"(?<![\w./-])(?:\./)?(?P<path>(?:tools|validation|\.github/scripts)/[\w./-]+\.(?:sh|py))\b")
SOURCED = re.compile(r"^\s*(?:\.|source)\s+[\"']?\$[{(]?\w+[})]?/(?P<path>tools/lib/[\w.-]+\.sh)", re.M)


def joined(run: str) -> str:
    """A `run:` body with its shell line continuations joined."""
    return re.sub(r"\\\n\s*", " ", run)


def shell_compiles(text: str) -> list[str]:
    """Each line of shell that compiles the root workspace."""
    out = []
    for line in joined(text).splitlines():
        code = line.split("#", 1)[0] if not line.lstrip().startswith("#") else ""
        if CARGO_COMPILE.search(code) and not OTHER_WORKSPACE.search(code):
            out.append(line.strip())
    return out


def python_compiles(text: str) -> list[str]:
    """Each argv list in Python source that compiles the root workspace."""
    return [
        " ".join(m.group(0).split())[:120]
        for m in PY_CARGO.finditer(text)
        if not PY_OTHER_WORKSPACE.search(m.group("rest"))
    ]


def steps_of(job: dict[str, Any]) -> list[dict[str, Any]]:
    return [s for s in (job.get("steps") or []) if isinstance(s, dict)]


def needs_of(job: dict[str, Any]) -> list[str]:
    n = job.get("needs")
    return [n] if isinstance(n, str) else [x for x in (n or []) if isinstance(x, str)]


class Reader:
    """Repository scripts reached from `run:` text, each read once."""

    def __init__(self, repo: pathlib.Path) -> None:
        self.repo = repo
        self.read: set[str] = set()
        self.python: set[str] = set()

    def findings_for(self, rel: str, via: str, seen: set[str] | None = None) -> list[str]:
        seen = set() if seen is None else seen
        if rel in seen:
            return []
        seen.add(rel)
        path = self.repo / rel
        if not path.is_file():
            return []
        self.read.add(rel)
        text = path.read_text(encoding="utf-8", errors="replace")
        if rel.endswith(".py"):
            self.python.add(rel)
            return [f"{via} → `{rel}` compiles the root workspace: {hit}" for hit in python_compiles(text)]
        found = [f"{via} → `{rel}` compiles the root workspace: {hit}" for hit in shell_compiles(text)]
        for m in SOURCED.finditer(text):
            found += self.findings_for(m.group("path"), f"{via} → `{rel}`", seen)
        return found


def check(workflow: dict[str, Any], repo: pathlib.Path) -> tuple[list[str], dict[str, int]]:
    findings: list[str] = []
    jobs = workflow.get("jobs")
    if not isinstance(jobs, dict):
        raise SystemExit("check-delvec-built-once: the workflow has no `jobs` mapping")
    reader = Reader(repo)

    # 1. the build job
    producer = jobs.get(PRODUCER)
    if not isinstance(producer, dict):
        findings.append(f"no `{PRODUCER}` job: nothing builds the run's one `delvec`")
    else:
        runs = [str(s.get("run") or "") for s in steps_of(producer)]
        if not any(r.strip() == BUILD for r in runs):
            findings.append(f"`{PRODUCER}` has no step running exactly `{BUILD}`")
        uploads = [
            s for s in steps_of(producer)
            if str(s.get("uses") or "").startswith("actions/upload-artifact@")
            and isinstance(s.get("with"), dict)
            and s["with"].get("name") == ARTIFACT
        ]
        if len(uploads) != 1:
            findings.append(f"`{PRODUCER}` uploads the artifact `{ARTIFACT}` {len(uploads)} time(s), not once")
        elif str(uploads[0]["with"].get("path") or "").strip() != BINARY:
            findings.append(f"`{PRODUCER}` uploads `{uploads[0]['with'].get('path')}` as `{ARTIFACT}`, not `{BINARY}`")
        outputs = producer.get("outputs") if isinstance(producer.get("outputs"), dict) else {}
        if "sha256" not in outputs:
            findings.append(f"`{PRODUCER}` declares no `sha256` output, so a consumer has nothing to verify the bytes against")

    for jid in COMPILES:
        if jid not in jobs:
            findings.append(f"`COMPILES` names `{jid}`, which is not a job in the workflow; drop the entry")

    # 2 + 3 + 4. every other job
    consumers = 0
    judged = 0
    for jid, job in jobs.items():
        if not isinstance(job, dict):
            continue
        steps = steps_of(job)
        for i, s in enumerate(steps):
            uses = str(s.get("uses") or "")
            if uses.startswith("actions/download-artifact@") and isinstance(s.get("with"), dict) and s["with"].get("name") == ARTIFACT:
                findings.append(f"job `{jid}` step {i + 1} downloads `{ARTIFACT}` itself; take it through `{ACTION}`, which checks its sha256")
        if jid in COMPILES:
            continue
        judged += 1
        first_run = None
        for i, s in enumerate(steps):
            run = str(s.get("run") or "")
            where = f"job `{jid}` step {i + 1}"
            for hit in shell_compiles(run):
                findings.append(f"{where} compiles the root workspace: {hit}")
            for m in SCRIPT.finditer(joined(run)):
                findings += reader.findings_for(m.group("path"), where)
            if first_run is None and BINARY in run:
                first_run = i
        if first_run is None:
            continue
        consumers += 1
        if PRODUCER not in needs_of(job):
            findings.append(f"job `{jid}` runs `{BINARY}` and does not need `{PRODUCER}`")
        takes = [
            i for i, s in enumerate(steps)
            if str(s.get("uses") or "") == ACTION
            and isinstance(s.get("with"), dict)
            and str(s["with"].get("sha256") or "").strip() == SHA_INPUT
        ]
        if not takes:
            findings.append(f"job `{jid}` runs `{BINARY}` without taking it through `{ACTION}` with `sha256: {SHA_INPUT}`")
        elif takes[0] > first_run:
            findings.append(f"job `{jid}` runs `{BINARY}` at step {first_run + 1}, before `{ACTION}` installs it at step {takes[0] + 1}")

    action = repo / ACTION.removeprefix("./") / "action.yml"
    if not action.is_file():
        findings.append(f"`{ACTION}/action.yml` does not exist")
    else:
        text = action.read_text(encoding="utf-8")
        if f"name: {ARTIFACT}" not in text or "sha256sum" not in text:
            findings.append(f"`{ACTION}/action.yml` does not download `{ARTIFACT}` and check its sha256")

    counts = {
        "jobs": len(jobs),
        "compiles": len(COMPILES),
        "judged": judged,
        "consumers": consumers,
        "scripts": len(reader.read),
        "python": len(reader.python),
    }
    return findings, counts


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--workflow", type=pathlib.Path, default=WORKFLOW)
    ap.add_argument("--repo", type=pathlib.Path, default=REPO)
    args = ap.parse_args(argv)
    try:
        workflow = load(args.workflow.read_text(encoding="utf-8"))
    except (OSError, WorkflowYamlError) as exc:
        print(f"check-delvec-built-once: FATAL — {exc}", file=sys.stderr)
        return 2
    if not isinstance(workflow, dict):
        print("check-delvec-built-once: FATAL — the workflow is not a mapping", file=sys.stderr)
        return 2
    findings, c = check(workflow, args.repo)
    binding = (
        f"{c['jobs']} job(s): {c['compiles']} declared to compile, {c['judged']} judged, "
        f"{c['consumers']} of them running `{BINARY}`; {c['scripts']} invoked script(s) read, "
        f"of which {c['python']} Python — a script a Python tool starts is not followed"
    )
    if c["judged"] == 0 or c["consumers"] == 0:
        print(f"check-delvec-built-once: FAIL — a binding of zero ({binding}); this examined nothing", file=sys.stderr)
        return 2
    if findings:
        print(f"check-delvec-built-once: FAIL — {len(findings)} finding(s); {binding}", file=sys.stderr)
        for line in findings:
            print(f"  - {line}", file=sys.stderr)
        return 1
    print(f"check-delvec-built-once: OK — {binding}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
