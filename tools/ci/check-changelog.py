#!/usr/bin/env python3
"""Every `CHANGELOG.md` entry is a summary of a release, judged where it is written (ADR-<next>).

WHY THIS EXISTS

A Release's notes are a summary of what the release changes for its consumer —
`### Highlights`, then grouped changes — never the titles of the merged pull
requests, which the compare link already gives. The summary is an entry of
`CHANGELOG.md`, written before the release by the agent that prepares it, and
the release workflows refuse a version with none (`tools/ci/release-notes.py`).
That refusal fires at the dispatch; this fires on the pull request that writes
the entry, which is where a defect in it is cheapest to fix.

WHAT IT JUDGES, through `tools/lib/changelog.py`, the one reader of the file:

- the file parses: every `##` heading is a Release title (`<name> <version>` in
  the tag grammar), no title twice, each line's entries newest first;
- every entry names a real release: its tag exists, or its version is the one
  the tree states for that line (`versions.toml [engine].version`,
  `[engine].dsl_crate_version`, `plugin.json` `version`) — an entry for a
  version nothing names is a stale draft;
- every entry has the shape of a summary (`changelog.shape`): one to five
  highlights, at least one change group from the Keep a Changelog vocabulary,
  at most thirty changes, nothing but sections and bullets, and no bullet that
  is a merged change's own subject — compared over `git log previous..end`,
  where `previous` is the previous release of the same line and `end` is the
  entry's tag where it exists, else `HEAD`.

VACUITY. Every run prints the entries judged, the bullets judged and the merged
subjects compared. A changelog with no entry is a legitimate zero (a line's
entries begin at its first release under this rule) and passes with its count
stated; a changelog that cannot be read is a refusal (exit 2).

Deterministic, offline, stdlib-only python3; `git` for the tags, the merged
subjects and nothing else. Exit 0 clean, 1 with findings, 2 unreadable.

Usage:
  python3 tools/ci/check-changelog.py [--repo DIR]
"""

from __future__ import annotations

import argparse
import json
import pathlib
import subprocess
import sys
import tomllib

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "lib"))

import changelog  # noqa: E402
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
    path = repo / changelog.FILE
    text = path.read_text(encoding="utf-8")
    entries, findings = changelog.parse(text)
    tags = [t for t in git(repo, "tag", "-l").split("\n") if t.strip()]
    stated = stated_versions(repo)
    bullets = 0
    compared = 0
    for entry in entries:
        released = entry.tag in tags
        if not released and stated[entry.name] != entry.version:
            findings.append(
                f"{changelog.FILE}:{entry.line}: `## {entry.title}` names a version no tag names and the tree "
                f"does not state ({entry.name} is {stated[entry.name]} here)"
            )
        previous = release_tags.previous(entry.tag, tags)
        end = entry.tag if released else "HEAD"
        subjects = None
        if previous:
            subjects = [s for s in git(repo, "log", "--format=%s", f"{previous}..{end}").split("\n") if s.strip()]
            compared += len(subjects)
        bullets += len(entry.highlights) + entry.change_count
        findings.extend(changelog.shape(entry, subjects))
        print(
            f"  {entry.title}: {'released' if released else 'unreleased'}; {len(entry.highlights)} highlight(s), "
            f"{entry.change_count} change(s); compared with {len(subjects) if subjects is not None else 0} "
            f"merged subject(s) since {previous or '(none)'}"
        )
    summary = (
        f"check-changelog: {len(entries)} entr{'y' if len(entries) == 1 else 'ies'} judged, {bullets} bullet(s), "
        f"{compared} merged subject(s) compared, {len(findings)} finding(s)"
    )
    return findings, summary


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description="every changelog entry is a summary, not the merged list")
    ap.add_argument("--repo", type=pathlib.Path, default=REPO)
    args = ap.parse_args(argv)
    sys.stdout.reconfigure(newline="\n")  # CRLF-proof: tools/ci/check-python-shell-newlines.py
    try:
        findings, summary = judge(args.repo)
    except (OSError, KeyError, tomllib.TOMLDecodeError, json.JSONDecodeError) as exc:
        print(f"check-changelog: cannot read the tree — {exc}", file=sys.stderr)
        return 2
    for finding in findings:
        print(f"check-changelog: FAIL — {finding}", file=sys.stderr)
    print(summary)
    return 1 if findings else 0


if __name__ == "__main__":
    raise SystemExit(main())
