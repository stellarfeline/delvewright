#!/usr/bin/env python3
"""Every `RELEASE-NOTES.md` entry is a summary of the release being prepared, judged where it is written (ADR-0036).

WHY THIS EXISTS

A Release's notes are a summary of what the release changes for its consumer —
`### Highlights`, then grouped changes — never the titles of the merged pull
requests, which the compare link already gives. The summary is an entry of
`RELEASE-NOTES.md`, written before the release by the agent that prepares it, and
the release workflows refuse a version with none (`tools/ci/release-notes.py`).
That refusal fires at the dispatch; this fires on the pull request that writes
the entry, which is where a defect in it is cheapest to fix.

WHAT IT JUDGES, through `tools/lib/release_notes.py`, the one reader of the file:

- the file parses: every `##` heading is a Release title (`<name> <version>` in
  the tag grammar), no title twice, one entry per line;
- every entry is the release being prepared: its version is the one the tree
  states for that line (`versions.toml [engine].version`,
  `[engine].dsl_crate_version`, `plugin.json` `version`). An entry for a version
  the tree does not state names no release; an entry whose tag is already
  written while the tree states a later version is stale — the published
  Release keeps that text, and the file is replaced for the next release. An
  entry whose tag is written and whose version the tree still states is the
  release being re-run, and passes;
- every entry has the shape of a summary (`release_notes.shape`): a Highlights
  section within its ceiling, at least one group of changes with a bullet, the
  changes within their ceiling, every line inside a section, and no line that
  is a merged change's own subject — compared over `git log previous..end`,
  where `previous` is the previous release of the same line and `end` is the
  entry's tag where it exists, else `HEAD`.

VACUITY. Every run prints the entries judged, the bullets judged and the merged
subjects compared. A file with no entry is a legitimate zero (no release is
being prepared) and passes with its count stated; a tree that cannot be read is
a refusal (exit 2).

Deterministic, offline, stdlib-only python3; `git` for the tags, the merged
subjects and nothing else. Exit 0 clean, 1 with findings, 2 unreadable.

Usage:
  python3 tools/ci/check-release-notes.py [--repo DIR]
"""

from __future__ import annotations

import argparse
import json
import pathlib
import subprocess
import sys
import tomllib

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "lib"))

import release_notes  # noqa: E402
import release_tags  # noqa: E402

REPO = pathlib.Path(__file__).resolve().parents[2]
MANIFEST = ".claude/skills/delvewright/.claude-plugin/plugin.json"


def git(repo: pathlib.Path, *args: str) -> str:
    result = subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True)
    if result.returncode != 0:
        raise OSError(f"`git {' '.join(args)}` failed: {result.stderr.strip()}")
    return result.stdout


def stated_versions(repo: pathlib.Path) -> dict[str, str]:
    """The version the working tree states for each released line."""
    engine = tomllib.loads((repo / "versions.toml").read_text(encoding="utf-8"))["engine"]
    plugin = json.loads((repo / MANIFEST).read_text(encoding="utf-8"))["version"]
    return {"delvec": engine["version"], "delvewright-dsl": engine["dsl_crate_version"], "delvewright": plugin}


def judge(repo: pathlib.Path) -> tuple[list[str], str]:
    """`(findings, binding summary)`."""
    path = repo / release_notes.FILE
    text = path.read_text(encoding="utf-8")
    entries, findings = release_notes.parse(text)
    tags = [t for t in git(repo, "tag", "-l").split("\n") if t.strip()]
    stated = stated_versions(repo)
    bullets = 0
    compared = 0
    for entry in entries:
        released = entry.tag in tags
        if stated[entry.name] != entry.version:
            if released:
                findings.append(
                    f"{release_notes.FILE}:{entry.line}: `## {entry.title}` is stale: {entry.tag} is released and the "
                    f"tree is at {stated[entry.name]}; the file holds the release being prepared, and the published "
                    f"Release keeps this text"
                )
            else:
                findings.append(
                    f"{release_notes.FILE}:{entry.line}: `## {entry.title}` names a version the tree does not state "
                    f"({entry.name} is {stated[entry.name]} here) and no tag names"
                )
        previous = release_tags.previous(entry.tag, tags)
        end = entry.tag if released else "HEAD"
        subjects = None
        if previous:
            subjects = [s for s in git(repo, "log", "--format=%s", f"{previous}..{end}").split("\n") if s.strip()]
            compared += len(subjects)
        bullets += len(entry.highlights) + entry.change_count
        findings.extend(release_notes.shape(entry, subjects))
        print(
            f"  {entry.title}: {'released' if released else 'unreleased'}; {len(entry.highlights)} highlight(s), "
            f"{entry.change_count} change(s); compared with {len(subjects) if subjects is not None else 0} "
            f"merged subject(s) since {previous or '(none)'}"
        )
    summary = (
        f"check-release-notes: {len(entries)} entr{'y' if len(entries) == 1 else 'ies'} judged, {bullets} bullet(s), "
        f"{compared} merged subject(s) compared, {len(findings)} finding(s)"
    )
    return findings, summary


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description="every release-notes entry is a summary, not the merged list")
    ap.add_argument("--repo", type=pathlib.Path, default=REPO)
    args = ap.parse_args(argv)
    sys.stdout.reconfigure(newline="\n")  # CRLF-proof: tools/ci/check-python-shell-newlines.py
    try:
        findings, summary = judge(args.repo)
    except (OSError, KeyError, tomllib.TOMLDecodeError, json.JSONDecodeError) as exc:
        print(f"check-release-notes: cannot read the tree — {exc}", file=sys.stderr)
        return 2
    for finding in findings:
        print(f"check-release-notes: FAIL — {finding}", file=sys.stderr)
    print(summary)
    return 1 if findings else 0


if __name__ == "__main__":
    raise SystemExit(main())
