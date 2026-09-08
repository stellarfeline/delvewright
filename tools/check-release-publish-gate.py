#!/usr/bin/env python3
"""One approval publishes both shelves, or neither: only a gated job may publish.

WHY THIS EXISTS

`engine-release.yml` fills two shelves from one tag — the GitHub Release's
assets and the crates.io versions — and it used to gate only one of them.
`publish-assets` needed `[identity, binaries]`, declared no `environment:`, and
created the release and uploaded the whole shelf with nobody asked; only
`publish-crates` waited for a reviewer. One tag push therefore made the release
public while the registry half sat at "waiting for review", and either half
could land alone: at `v1.2.0` both Linux shelf jobs failed and nothing reached
crates.io, and the mirror image — a public release beside a registry that never
got the crates — is the same defect pointing the other way. The rule the
workflow now implements is that **one approval authorises both publishes, and
neither happens without it**: the shelf is assembled into a DRAFT release, which
publishes nothing, and a single environment-gated job then uploads to crates.io
and undrafts the release.

That rule is a claim about which job holds which verb, and a comment in a
workflow file cannot hold a claim of that shape — the previous arrangement was
described accurately by its own comments for as long as it existed. This is the
executable form: a publishing act may appear only inside a job that declares
`environment:`, so a second ungated job that undrafts a release, or an ungated
`gh release create` without `--draft`, is an ordinary red on the pull request
that writes it rather than a discovery made during a release.

WHAT COUNTS AS A PUBLISHING ACT, and why each is one

  registry-token      `CARGO_REGISTRY_TOKEN`, in an env key or a `secrets.`
                      reference. Whoever can read it can publish, and the token
                      lives on the `crates-io` environment precisely so that
                      only a gated job can.
  cargo-publish       `cargo publish`, or `tools/crates-io-publish.sh` invoked
                      with `--publish`. `--plan` is not an act: it reads the
                      registry and uploads nothing, which is why the preflight
                      job may run it with no credential in the run at all.
  release-create      `gh release create` without `--draft` on the same command
                      (or a `gh api` POST to `/releases` without `draft=true`).
                      A release created without `--draft` is public the instant
                      it exists.
  release-undraft     `gh release edit … --draft=false`, or a `gh api` call
                      setting `draft=false` on a release. This is the act that
                      makes a draft public, and it is the second half of the one
                      approval.

  draft-readback      not an act but its counterpart: an ungated job that WRITES
                      to a release (`gh release upload|edit|delete`) must read
                      `isDraft` back in the same job. Uploading an asset to a
                      release that is already published is itself publishing, and
                      no static rule can tell which kind of release a tag names —
                      so the workflow refuses at run time and this refuses the
                      deletion of that refusal. What is checked is that the
                      readback is PRESENT, never that it is right; the shape it
                      has to have is in `engine-release.yml`'s `shelf` job.

TWO POPULATIONS, because one of them is the way around the other

  A. every job in every `.github/workflows/*.yml`. An act in a job that declares
     no `environment:` is a finding.
  B. every tracked shell script, plus every composite-action YAML under
     `.github/`. The release verbs (`release-create`, `release-undraft`) may not
     appear there AT ALL: a script is invoked by whichever job calls it, so a
     `gh release edit --draft=false` moved into `tools/something.sh` would be
     invisible to A while doing exactly what A forbids. The release is created
     and undrafted by the workflow alone, where the gate can see it.
     `cargo publish` is deliberately NOT an act in B — the registry upload
     legitimately lives inside `tools/crates-io-publish.sh`, and it is that
     script's INVOCATION SITE, judged by A, that decides whether it can run.

Population A is read through `tools/lib/workflow_yaml.py`, the one shared parse
rule, which refuses constructs it does not implement rather than reading past
them into a clean pass — and reading the parsed structure rather than the file
text is also what keeps a workflow COMMENT that names a verb (this workflow's
header names `gh release edit <tag> --draft=false` while explaining it) from
being read as an act. Population B is raw text, because a shell script has no
structure a checker of this kind can use.

VACUITY: this exits non-zero when it finds no workflow, no job, no gated job, or
no publishing act at all. A repository whose release workflow contains no
publishing act is one where the discovery rule broke — a renamed key, a glob
that matched nothing, a verb spelled some new way — and reporting that as a
clean tree would leave the one-way door unguarded and green.

Deterministic, offline, stdlib-only python3 (plus `git ls-files` for the tracked
file set, the same way `check-unsanctioned-identifiers.py` derives its own).

Usage:
  python3 tools/check-release-publish-gate.py [--workflows DIR] [--repo DIR]

Exit 0 clean, 1 with one finding per offending job or file, 2 when the scan
itself did not bind.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import subprocess
import sys
from typing import Any, Iterator

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent / "lib"))

from workflow_yaml import WorkflowYamlError, load  # noqa: E402

REPO = pathlib.Path(__file__).resolve().parent.parent

# --------------------------------------------------------------------------
# the acts
# --------------------------------------------------------------------------
# Each is matched against ONE logical line (backslash continuations joined), so
# `gh release create … --draft` is judged as the single command it is.

_GH_RELEASE = r"\bgh\s+(?:[^\n]*?\s)?release\s+"

RELEASE_CREATE = re.compile(_GH_RELEASE + r"create\b")
RELEASE_WRITE = re.compile(_GH_RELEASE + r"(?:create|edit|upload|delete)\b")
DRAFT_FLAG = re.compile(r"--draft(?:\s*=\s*|\s+)?(?:true)?(?:\s|$|\"|')")
DRAFT_FALSE = re.compile(r"--draft\s*(?:=|\s)\s*(?:false|0)\b")
RELEASE_EDIT = re.compile(_GH_RELEASE + r"edit\b")
GH_API = re.compile(r"\bgh\s+api\b")
API_RELEASES = re.compile(r"/releases\b|/releases/")
API_DRAFT_FALSE = re.compile(r"draft[\"']?\s*[:=]\s*[\"']?false\b")
API_DRAFT_TRUE = re.compile(r"draft[\"']?\s*[:=]\s*[\"']?true\b")
API_POST = re.compile(r"(?:-X|--method)\s+POST\b")

REGISTRY_TOKEN = re.compile(r"\bCARGO_REGISTRY_TOKEN\b")
CARGO_PUBLISH = re.compile(r"\bcargo\s+publish\b")
PUBLISH_SCRIPT = re.compile(r"crates-io-publish\.sh\b[^\n]*?(?:^|\s)--publish\b")

# The API's own field name for the answer, and the only spelling that reads the
# state back. `--draft=false` mentions a draft and reads nothing, so it must not
# satisfy this — a job that writes to a release and only ever SETS the flag has
# not looked at what it is writing to.
DRAFT_READBACK = re.compile(r"\bisDraft\b")


def acts_on_line(line: str, *, release_verbs_only: bool = False) -> list[tuple[str, str]]:
    """Every publishing act this one logical line performs, as (kind, why)."""
    found: list[tuple[str, str]] = []

    if RELEASE_CREATE.search(line) and not DRAFT_FLAG.search(line):
        found.append(
            ("release-create", "`gh release create` with no `--draft`: the release is public the instant it exists")
        )
    if RELEASE_EDIT.search(line) and DRAFT_FALSE.search(line):
        found.append(("release-undraft", "`gh release edit --draft=false`: this is what makes a draft public"))
    if GH_API.search(line) and API_RELEASES.search(line):
        if API_DRAFT_FALSE.search(line):
            found.append(("release-undraft", "a `gh api` call setting `draft=false` on a release"))
        elif API_POST.search(line) and not API_DRAFT_TRUE.search(line):
            found.append(("release-create", "a `gh api` POST creating a release without `draft=true`"))

    if release_verbs_only:
        return found

    if REGISTRY_TOKEN.search(line):
        found.append(("registry-token", "`CARGO_REGISTRY_TOKEN`: whoever can read it can publish to crates.io"))
    if CARGO_PUBLISH.search(line):
        found.append(("cargo-publish", "`cargo publish`: a one-way door — a version can never be reused or deleted"))
    if PUBLISH_SCRIPT.search(line):
        found.append(("cargo-publish", "`crates-io-publish.sh --publish`: the upload itself"))
    return found


def numbered_logical_lines(text: str) -> Iterator[tuple[int, str]]:
    """The text's lines with backslash continuations joined, each paired with the
    PHYSICAL line it starts on — so a finding names a line the file really has."""
    buffer = ""
    start = 1
    for number, raw in enumerate(text.replace("\r\n", "\n").split("\n"), start=1):
        if not buffer:
            start = number
        line = buffer + raw
        if line.rstrip().endswith("\\"):
            buffer = line.rstrip()[:-1] + " "
            continue
        buffer = ""
        yield start, line
    if buffer:
        yield start, buffer


def logical_lines(text: str) -> Iterator[str]:
    """The text's lines, with backslash continuations joined into one line."""
    for _, line in numbered_logical_lines(text):
        yield line


def scalars(node: Any) -> Iterator[str]:
    """Every string in a parsed subtree — mapping keys included.

    Keys matter: `CARGO_REGISTRY_TOKEN: ${{ secrets.… }}` puts the credential's
    name on the left of the colon.
    """
    if isinstance(node, str):
        yield node
    elif isinstance(node, dict):
        for key, value in node.items():
            if isinstance(key, str):
                yield key
            yield from scalars(value)
    elif isinstance(node, list):
        for value in node:
            yield from scalars(value)


# --------------------------------------------------------------------------
# population A — workflow jobs
# --------------------------------------------------------------------------
def check_workflows(directory: pathlib.Path) -> tuple[list[str], dict[str, int]]:
    findings: list[str] = []
    counts = {"files": 0, "jobs": 0, "gated": 0, "acts": 0, "acts_gated": 0, "readbacks": 0}

    files = sorted(p for p in directory.iterdir() if p.is_file() and p.suffix in (".yml", ".yaml"))
    counts["files"] = len(files)

    for path in files:
        rel = path.relative_to(REPO) if path.is_relative_to(REPO) else path
        try:
            doc = load(path.read_text(encoding="utf-8"))
        except WorkflowYamlError as exc:
            findings.append(
                f"{rel}: the shared workflow parser refused this file — {exc}\n"
                f"    Nothing here can say which of its jobs may publish, and an "
                f"unreadable gate is not a passed gate. Either simplify the construct "
                f"or teach tools/lib/workflow_yaml.py to read it."
            )
            continue

        jobs = (doc or {}).get("jobs")
        if not isinstance(jobs, dict):
            findings.append(
                f"{rel}: no `jobs:` mapping. A workflow file with no jobs is not "
                f"something this checker can bind to; if the file is not a workflow, "
                f"it does not belong in this directory."
            )
            continue

        for job_id, job in jobs.items():
            if not isinstance(job, dict):
                continue
            counts["jobs"] += 1
            gated = "environment" in job
            if gated:
                counts["gated"] += 1

            text = "\n".join(scalars(job))
            job_acts: list[tuple[str, str]] = []
            writes_release = False
            for line in logical_lines(text):
                job_acts.extend(acts_on_line(line))
                if RELEASE_WRITE.search(line):
                    writes_release = True

            counts["acts"] += len(job_acts)
            if gated:
                counts["acts_gated"] += len(job_acts)
                continue

            for kind, why in job_acts:
                findings.append(
                    f"{rel}: job {job_id!r} performs a publishing act ({kind}) and declares no "
                    f"`environment:`.\n"
                    f"    {why}.\n"
                    f"    One approval publishes both shelves or neither: every act that makes\n"
                    f"    something public — the registry upload and the release undraft alike —\n"
                    f"    belongs in the single job that declares `environment: crates-io`, so the\n"
                    f"    run pauses for a reviewer before ANY of it happens. Assemble the shelf\n"
                    f"    into a DRAFT release here (a draft publishes nothing and owes no\n"
                    f"    approval) and move this act into the gated job."
                )

            if writes_release:
                if DRAFT_READBACK.search(text):
                    counts["readbacks"] += 1
                else:
                    findings.append(
                        f"{rel}: job {job_id!r} writes to a GitHub Release, declares no "
                        f"`environment:`, and never reads the release's draft state back.\n"
                        f"    Uploading an asset to a release that is already published IS "
                        f"publishing, and no rule outside the run can tell which kind of release a\n"
                        f"    tag names. An ungated job may touch a release only after reading\n"
                        f"    `isDraft` back from the API and refusing anything but a draft."
                    )

    return findings, counts


# --------------------------------------------------------------------------
# population B — scripts and composite actions
# --------------------------------------------------------------------------
SHEBANG = re.compile(rb"^#!.*\b(?:ba|da|k|z)?sh\b")


def tracked_files(repo: pathlib.Path) -> list[pathlib.Path]:
    out = subprocess.run(
        ["git", "-C", str(repo), "ls-files", "-z"],
        check=True,
        capture_output=True,
    ).stdout
    return [repo / name.decode("utf-8") for name in out.split(b"\0") if name]


def population_b(repo: pathlib.Path) -> list[pathlib.Path]:
    """Shell scripts and composite actions: everything a workflow job can call
    into that is not itself a workflow job."""
    chosen: list[pathlib.Path] = []
    for path in tracked_files(repo):
        if not path.is_file():
            continue
        relative = path.relative_to(repo).as_posix()
        if relative.startswith(".github/workflows/"):
            continue
        if path.suffix in (".sh", ".bash"):
            chosen.append(path)
            continue
        if relative.startswith(".github/") and path.suffix in (".yml", ".yaml"):
            chosen.append(path)
            continue
        try:
            with path.open("rb") as handle:
                first = handle.readline()
        except OSError:
            continue
        if SHEBANG.match(first):
            chosen.append(path)
    return sorted(set(chosen))


def check_scripts(repo: pathlib.Path) -> tuple[list[str], dict[str, int]]:
    findings: list[str] = []
    counts = {"files": 0, "acts": 0}
    for path in population_b(repo):
        counts["files"] += 1
        try:
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        rel = path.relative_to(repo).as_posix()
        for number, line in numbered_logical_lines(text):
            for kind, why in acts_on_line(line, release_verbs_only=True):
                counts["acts"] += 1
                findings.append(
                    f"{rel}:{number}: {kind} — {why}.\n"
                    f"    A script is invoked by whichever job calls it, so a release verb here is\n"
                    f"    invisible to the rule that every publishing act sits in the one\n"
                    f"    environment-gated job. The release is created (as a draft) and undrafted\n"
                    f"    by `.github/workflows/engine-release.yml` alone, where that rule can see\n"
                    f"    it. Put the verb in the gated job, not behind a script."
                )
    return findings, counts


# --------------------------------------------------------------------------
def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="one approval publishes both shelves, or neither")
    parser.add_argument("--workflows", type=pathlib.Path, default=REPO / ".github" / "workflows")
    parser.add_argument("--repo", type=pathlib.Path, default=REPO)
    args = parser.parse_args(argv)

    directory: pathlib.Path = args.workflows
    if not directory.is_dir():
        print(
            f"check-release-publish-gate: FAIL — {directory} is not a directory, so the scan "
            f"bound to nothing",
            file=sys.stderr,
        )
        return 2

    findings, counts = check_workflows(directory)
    script_findings, script_counts = check_scripts(args.repo)
    findings += script_findings

    binding = (
        f"{counts['files']} workflow(s), {counts['jobs']} job(s), {counts['gated']} "
        f"environment-gated; {counts['acts']} publishing act(s), {counts['acts_gated']} of them "
        f"inside a gated job; {counts['readbacks']} ungated release-writer(s) proving the release "
        f"is a draft; {script_counts['files']} script(s)/composite action(s) scanned, "
        f"{script_counts['acts']} release verb(s) found there"
    )

    for zero, why in (
        (
            counts["files"] == 0,
            "a directory of workflows containing no workflow file is the enumeration breaking",
        ),
        (
            counts["jobs"] == 0,
            "no job was read out of any workflow, so nothing was judged",
        ),
        (
            counts["gated"] == 0,
            "no job declares `environment:` — the publish gate this checker exists to bind "
            "publishing acts to does not exist, and a tree with no gate is not a tree with "
            "nothing to gate",
        ),
        (
            counts["acts"] == 0,
            "no publishing act was found anywhere: this repository publishes to crates.io and "
            "to GitHub Releases, so finding none means the discovery rule broke (a renamed key, "
            "a verb spelled a new way) and reporting it as a pass would leave the one-way door "
            "unguarded and green",
        ),
        (
            script_counts["files"] == 0,
            "no script or composite action was scanned, so the way AROUND the workflow rule was "
            "never looked at",
        ),
    ):
        if zero:
            print(f"check-release-publish-gate: FAIL — {binding}. {why}.", file=sys.stderr)
            return 2

    if findings:
        print(f"check-release-publish-gate: {len(findings)} finding(s) — {binding}\n", file=sys.stderr)
        for finding in findings:
            print(f"  {finding}\n", file=sys.stderr)
        return 1

    print(
        f"check-release-publish-gate: OK — {binding}; every publishing act is inside a job that "
        f"declares an environment, so one approval publishes both shelves or neither"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
