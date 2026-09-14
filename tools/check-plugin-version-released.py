#!/usr/bin/env python3
"""A plugin version is not left delivered and unreleased: the version this change
moves away from has its published `delvewright--v<version>` Release.

WHY THIS EXISTS

A creator receives the `delvewright` plugin when `plugin.json` `version` moves on
`main`; the Release is the record of that delivery, and it lags behind it — a
human pushes the tag, or runs `.github/workflows/plugin-release.yml`'s manual arm,
after the merge (ADR-0028 §3–§5). So a version can be delivered and never
recorded, and nothing else would ever say so.

WHAT THIS CHECKS, AND WHY THE SUBJECT IS THE OUTGOING NUMBER

The subject is the version a pull request moves AWAY from, for the reason
`tools/check-dsl-version-published.py` gives for its own: demanding that the
CURRENT version be released would red the correct state of the world between
the merge and the release, while the outgoing version has had its whole life on
`main` to be released and this is the last moment anyone asks. **A delivered
version with no Release reds the next bump.**

A Release counts when all of these hold, each read from the remote:

- a PUBLISHED Release exists at `delvewright--v<X>` (a draft is not one);
- it carries `delvewright-plugin-<X>.zip` and `SHA256SUMS`;
- the tag resolves to a commit whose `plugin.json` states `<X>` — a Release
  hung on a tag that names another version records nothing about this one.

BINDING, AND WHAT A ZERO MEANS HERE

The population is the set of plugin versions this change retires: at most one.
A change that does not move the version, or a branch whose version is BEHIND
the base's, retires nothing, and the run still reads and prints both numbers.
The Release lookup is bind-tested (`tools/lib/github_releases.py`) before any
"not released" is believed. On a push to `main` the base is the tree, so this
judges nothing; the bump was judged on its pull request, and branch protection
requires a pull request to be up to date with `main` before it merges, so no
bump reaches `main` unjudged by the tree it merges into.

Usage:
  python3 tools/check-plugin-version-released.py [--base origin/main] [--repo DIR]
      [--github-repo OWNER/NAME]

Exit 0 clean, 1 with the finding, 2 when the comparison could not be made.
"""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import subprocess
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent / "lib"))

import github_releases  # noqa: E402
import release_tags  # noqa: E402
from gitbase import BaseUnresolved, resolve_base  # noqa: E402

TOOL = "check-plugin-version-released"
REPO = pathlib.Path(__file__).resolve().parent.parent
NAME = "delvewright"
MANIFEST = ".claude/skills/delvewright/.claude-plugin/plugin.json"
WORKFLOW = ".github/workflows/plugin-release.yml"
DEFAULT_GITHUB_REPO = "stellarfeline/delvewright"


class Unjudgeable(Exception):
    """The comparison could not be made. Exit 2 — never a pass."""


def assets_for(version: str) -> list[str]:
    return [f"delvewright-plugin-{version}.zip", "SHA256SUMS"]


def _version(raw: str, where: str) -> str:
    try:
        doc = json.loads(raw)
    except json.JSONDecodeError as exc:
        raise Unjudgeable(f"{TOOL}: FAIL — {where} is not JSON: {exc}")
    value = doc.get("version") if isinstance(doc, dict) else None
    if not isinstance(value, str):
        raise Unjudgeable(f"{TOOL}: FAIL — {where} carries no string `version`")
    try:
        release_tags.version_key(value)
    except release_tags.Refused as exc:
        raise Unjudgeable(f"{TOOL}: FAIL — {where}: {exc}")
    return value


def _show(repo: pathlib.Path, rev: str, path: str) -> str:
    result = subprocess.run(["git", "-C", str(repo), "show", f"{rev}:{path}"], capture_output=True, text=True)
    if result.returncode != 0:
        raise Unjudgeable(f"{TOOL}: FAIL — `git show {rev}:{path}` failed: {result.stderr.strip()}")
    return result.stdout


def released_problems(github_repo: str, version: str) -> list[str]:
    """Why `delvewright--v<version>` is not a Release of that version; empty when it is."""
    tag = release_tags.tag_for(NAME, version)
    rel = github_releases.published(github_repo, tag)
    if rel is None:
        what = github_releases.state(github_repo, tag)
        return [f"{github_repo} has no published Release at {tag} (it is {what})"]
    problems: list[str] = []
    have = set(github_releases.asset_names(rel))
    missing = [a for a in assets_for(version) if a not in have]
    if missing:
        problems.append(f"Release {tag} lacks {len(missing)} of {len(assets_for(version))} asset(s): {', '.join(missing)}")
    commit = github_releases.tag_commit(github_repo, tag)
    if commit is None:
        problems.append(f"tag {tag} does not resolve to a commit on {github_repo}")
    else:
        raw = github_releases.file_at(github_repo, MANIFEST, commit)
        if raw is None:
            problems.append(f"tag {tag} is {commit[:8]}, which carries no {MANIFEST}")
        else:
            at = _version(raw.decode("utf-8"), f"{github_repo}@{commit[:8]}:{MANIFEST}")
            if at != version:
                problems.append(f"tag {tag} is {commit[:8]}, whose {MANIFEST} states {at}, not {version}")
    return problems


def main(argv: list[str] | None = None) -> int:
    try:
        return _judge(argv)
    except Unjudgeable as exc:
        print(str(exc), file=sys.stderr)
        return 2
    except github_releases.Unreadable as exc:
        print(f"{TOOL}: FAIL — the Release lookup could not be read: {exc}", file=sys.stderr)
        return 2


def _judge(argv: list[str] | None) -> int:
    ap = argparse.ArgumentParser(description="the plugin version this change moves away from is released")
    ap.add_argument("--base", default="origin/main")
    ap.add_argument("--repo", type=pathlib.Path, default=REPO)
    ap.add_argument("--github-repo", default=os.environ.get("GITHUB_REPOSITORY") or DEFAULT_GITHUB_REPO)
    args = ap.parse_args(argv)

    try:
        base_sha = resolve_base(args.repo, args.base, TOOL)
    except BaseUnresolved as exc:
        print(exc.message, file=sys.stderr)
        return 2

    tree_path = args.repo / MANIFEST
    try:
        tree_raw = tree_path.read_text(encoding="utf-8")
    except OSError as exc:
        raise Unjudgeable(f"{TOOL}: FAIL — {MANIFEST} cannot be read: {exc}")
    tree_version = _version(tree_raw, MANIFEST)
    base_version = _version(_show(args.repo, base_sha, MANIFEST), f"{args.base}:{MANIFEST}")
    read = f"base {args.base} ({base_sha[:8]}) states {NAME} {base_version}; the tree states {tree_version}"

    if base_version == tree_version:
        print(f"{TOOL}: OK — {read}; 0 plugin version(s) retired by this change, nothing to judge")
        return 0
    if release_tags.version_key(tree_version) < release_tags.version_key(base_version):
        print(
            f"{TOOL}: OK — {read}; the tree's version is BEHIND the base's, so this change retires "
            f"nothing — it is a branch that has not caught up."
        )
        return 0

    ok, message = github_releases.bind_test()
    if not ok:
        print(f"{TOOL}: FAIL — {message}", file=sys.stderr)
        return 2
    print(f"  bind test: {message}", flush=True)

    tag = release_tags.tag_for(NAME, base_version)
    problems = released_problems(args.github_repo, base_version)
    if not problems:
        print(f"{TOOL}: OK — {read}; 1 of 1 retired plugin version(s) released ({tag})")
        return 0

    print(
        f"{TOOL}: 1 finding — {read}; 0 of 1 retired plugin version(s) released\n\n"
        + "".join(f"  - {p}\n" for p in problems)
        + f"\n  This change moves the plugin from {base_version} to {tree_version}. {base_version} was delivered "
        f"to every creator on the marketplace the moment it reached `main`, and its Release is the record of "
        f"that delivery; after this merge nothing asks about {base_version} again.\n"
        f"  Release {base_version} before moving past it, by either of the two entry points of `{WORKFLOW}`:\n"
        f"    - push the tag `{tag}` at the first-parent `main` commit carrying {base_version}, or\n"
        f"    - run the workflow's manual arm on that commit (it writes the tag),\n"
        f"  then approve the `plugin-release` environment. Re-run this check once the Release is published.",
        file=sys.stderr,
    )
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
