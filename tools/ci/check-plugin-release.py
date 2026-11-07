#!/usr/bin/env python3
"""The identity of a plugin release: what `plugin-release.yml` refuses before it writes anything (ADR-0038 §4).

WHY THIS EXISTS

The `delvewright` plugin is released on its own version line, independently of
`delvec`: `plugin.json` `version` at a `main` commit names the release, the tag
is `delvewright--v<version>` at that commit, and the marketplace entry in that
same tree names the tag — so the commit names its own release before the tag
exists, the way an engine release's commit does (ADR-0029 §3). The workflow is
dispatched with a commit, derives everything else from the tree there, and
refuses here, through one tool a test can perturb, rather than in shell a
workflow run is the only thing that ever exercises.

WHAT IT REFUSES, each with the perturbation that reds it

  1  the commit is on `main` (an ancestor of `--main`).
                                            RED: a commit on a side branch
  2  `plugin.json` `version` at the commit is a strict `major.minor.patch`,
     and `delvewright--v<version>` is unborn in this checkout (and, `--online`,
     on the remote).                        RED: a tag already at the commit;
                                                 a version the remote has tagged
  3  the version is above every `delvewright--v*` tag this checkout carries:
     a release moves the line forward, so a creator holding the newest one
     receives it as an update.              RED: the released version again;
                                                 a version below the newest
  4  the marketplace entry at the commit names that tag: the tree a creator
     receives from the tag is the tree whose catalog row points at it.
                                            RED: an entry still naming the
                                                 previous release
  5  the page's pin `[engine].ref` at the commit is a `delvec` release tag this
     checkout carries, and, `--online`, its Release is PUBLISHED — a plugin that
     tells its creator to install an engine nobody can download is refused
     here, before the tag.                  RED: an unborn engine tag; a
                                                 draft Release

What it does not judge: the page against the engine (`tools/ci/check-skill-page.py
--online`, run at the commit by the workflow after this), and the notes
(`tools/ci/release-notes.py delvewright`, which refuses a version with no
`RELEASE-NOTES.md` entry). Each question has one home.

VACUITY. Every run prints the tags of the line it read and the engine tag it
resolved; a checkout with no `delvewright--v*` tag is a refusal naming the
fetch, since the line has releases and a clone without them would call every
version unborn. Offline by default (`git` only); `--online` asks the remote
through `tools/lib/github_releases.py`. Exit 0 ok, 1 refused, 2 unusable.

Usage:
  python3 tools/ci/check-plugin-release.py --commit REV [--repo DIR] [--main REF]
      [--repository OWNER/NAME] [--online] [--output FILE]

`--output FILE` appends `tag=`, `sha=`, `version=` and `engine=` lines for the
workflow step that follows (`$GITHUB_OUTPUT`).
"""

from __future__ import annotations

import argparse
import json
import pathlib
import subprocess
import sys
import tomllib

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "lib"))

import github_releases  # noqa: E402
import release_tags  # noqa: E402

REPO = pathlib.Path(__file__).resolve().parents[2]
REPOSITORY = "stellarfeline/delvewright"
LINE = "delvewright"
ENGINE_LINE = "delvec"
PLUGIN_ROOT = ".claude/skills/delvewright"
MANIFEST = f"{PLUGIN_ROOT}/.claude-plugin/plugin.json"
PAGE_PIN = f"{PLUGIN_ROOT}/skills/new-delve/versions.toml"
MARKETPLACE = ".claude-plugin/marketplace.json"


class Unusable(Exception):
    """Something this tool reads cannot be read; nothing was judged."""


def git(repo: pathlib.Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True)


def show(repo: pathlib.Path, sha: str, path: str) -> str:
    result = git(repo, "show", f"{sha}:{path}")
    if result.returncode != 0:
        raise Unusable(f"`git show {sha[:12]}:{path}` failed: {result.stderr.strip()}")
    return result.stdout


def line_tags(repo: pathlib.Path, line: str) -> list[str]:
    listed = git(repo, "tag", "--list", f"{line}--v*")
    if listed.returncode != 0:
        raise Unusable(f"could not list tags: {listed.stderr.strip()}")
    return [t for t in listed.stdout.split() if release_tags.TAG_RE.match(t)]


def judge(
    repo: pathlib.Path,
    commit: str,
    main: str,
    repository: str,
    online: bool,
) -> tuple[list[str], dict[str, str], list[str]]:
    """`(refusals, facts, lines printed)` for the plugin release at `commit`."""
    refusals: list[str] = []
    said: list[str] = []
    resolved = git(repo, "rev-parse", "--verify", f"{commit}^{{commit}}")
    if resolved.returncode != 0:
        raise Unusable(f"{commit} is not a commit: {resolved.stderr.strip()}")
    sha = resolved.stdout.strip()

    # 1. on main
    ancestor = git(repo, "merge-base", "--is-ancestor", sha, main)
    if ancestor.returncode == 1:
        refusals.append(f"{sha[:12]} is not on {main}: a release is cut from a commit that went through review and CI.")
    elif ancestor.returncode != 0:
        raise Unusable(f"`git merge-base --is-ancestor {sha[:12]} {main}` failed: {ancestor.stderr.strip()}")
    else:
        said.append(f"{sha[:12]} is on {main}")

    # 2. the version and its tag
    try:
        version = json.loads(show(repo, sha, MANIFEST)).get("version")
    except json.JSONDecodeError as exc:
        raise Unusable(f"{MANIFEST} at {sha[:12]} does not parse: {exc}") from exc
    if not isinstance(version, str):
        raise Unusable(f"{MANIFEST} at {sha[:12]} states no `version`")
    try:
        tag = release_tags.tag_for(LINE, version)
    except release_tags.Refused as exc:
        raise Unusable(f"{MANIFEST} at {sha[:12]}: {exc}") from exc
    mine = release_tags.version_key(version)
    tags = line_tags(repo, LINE)
    if not tags:
        raise Unusable(
            f"this checkout carries no `{LINE}--v*` tag, so {version} cannot be held "
            f"above the newest release of its line; fetch them "
            f"(`git fetch --no-tags origin 'refs/tags/{LINE}--v*:refs/tags/{LINE}--v*'`)."
        )
    said.append(f"{len(tags)} {LINE}--v* tag(s) read")
    if tag in tags:
        at = git(repo, "rev-parse", "--verify", f"refs/tags/{tag}^{{commit}}").stdout.strip()
        refusals.append(
            f"{tag} already exists, at {at[:12]}. A release tag is written once; move "
            f"`plugin.json` `version` forward in a pull request and dispatch on its merge."
        )
    if online:
        remote = github_releases.tag_commit(repository, tag)
        if remote is not None:
            refusals.append(f"{repository} already carries {tag} at {remote[:12]}.")
        else:
            said.append(f"{repository} has no {tag}")

    # 3. above every release of the line
    released = sorted((release_tags.version_key(release_tags.parse(t)[1]), t) for t in tags)
    newest_key, newest = released[-1]
    if newest_key >= mine and tag not in tags:
        refusals.append(
            f"{version} is not above the newest release of the line, {newest}: a "
            f"release moves the line forward, or a creator holding {newest} receives "
            f"nothing."
        )
    elif tag not in tags:
        said.append(f"{version} is above the newest release, {newest}")

    # 4. the entry names the tag
    try:
        market = json.loads(show(repo, sha, MARKETPLACE))
        entries = market.get("plugins") or []
        entry_ref = entries[0]["source"].get("ref") if entries and isinstance(entries[0].get("source"), dict) else None
    except (json.JSONDecodeError, AttributeError, TypeError) as exc:
        raise Unusable(f"{MARKETPLACE} at {sha[:12]} is unreadable: {exc}") from exc
    if entry_ref != tag:
        refusals.append(
            f"the marketplace entry at {sha[:12]} names {entry_ref!r}, and the tag this "
            f"release writes is {tag}. The tree a creator receives is the one whose "
            f"catalog row points at it (ADR-0038 §3): the pull request that moved the "
            f"version moves the entry with it."
        )
    else:
        said.append(f"the marketplace entry names {tag}")

    # 5. the engine the page is proven on is released
    try:
        engine = tomllib.loads(show(repo, sha, PAGE_PIN))["engine"]["ref"]
    except (KeyError, tomllib.TOMLDecodeError) as exc:
        raise Unusable(f"{PAGE_PIN} at {sha[:12]} has no readable `[engine].ref`: {exc}") from exc
    try:
        engine_line, _engine_version = release_tags.parse(engine)
    except release_tags.Refused as exc:
        raise Unusable(f"{PAGE_PIN} at {sha[:12]}: {exc}") from exc
    if engine_line != ENGINE_LINE:
        raise Unusable(f"{PAGE_PIN} at {sha[:12]} pins {engine}, which is not a {ENGINE_LINE} release")
    if engine not in line_tags(repo, ENGINE_LINE):
        refusals.append(
            f"the page at {sha[:12]} pins {engine}, which this checkout has no tag for. "
            f"A plugin ships only at an engine release that exists: dispatch "
            f"`engine-release.yml` first, or pin a released engine."
        )
    elif online:
        state = github_releases.state(repository, engine)
        if state != "published":
            refusals.append(
                f"the page at {sha[:12]} pins {engine}, whose Release on {repository} is "
                f"{state}. A plugin that tells its creator to install an engine nobody "
                f"can download is refused before its tag is written."
            )
        else:
            said.append(f"{engine} is published on {repository}")
    else:
        said.append(f"{engine} is a tag this checkout carries")

    facts = {"tag": tag, "sha": sha, "version": version, "engine": engine}
    return refusals, facts, said


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description="the identity of a plugin release at a commit")
    ap.add_argument("--commit", required=True)
    ap.add_argument("--repo", type=pathlib.Path, default=REPO)
    ap.add_argument("--main", default="origin/main", help="the ref the commit must be an ancestor of")
    ap.add_argument("--repository", default=REPOSITORY, help="OWNER/NAME on GitHub, for --online")
    ap.add_argument("--online", action="store_true", help="ask the remote: the tag is unborn there, the engine Release is published")
    ap.add_argument("--output", type=pathlib.Path, help="append tag=, sha=, version= and engine= lines here")
    args = ap.parse_args(argv)
    sys.stdout.reconfigure(newline="\n")  # CRLF-proof: tools/ci/check-python-shell-newlines.py
    try:
        refusals, facts, said = judge(args.repo, args.commit, args.main, args.repository, args.online)
    except (Unusable, github_releases.Unreadable) as exc:
        print(f"check-plugin-release: cannot judge — {exc}", file=sys.stderr)
        return 2
    for line in said:
        print(f"  ok   {line}")
    for refusal in refusals:
        print(f"check-plugin-release: REFUSED — {refusal}", file=sys.stderr)
    if refusals:
        print(f"check-plugin-release: {len(refusals)} refusal(s); nothing is released")
        return 1
    if args.output is not None:
        with args.output.open("a", encoding="utf-8") as fh:
            for key, value in facts.items():
                fh.write(f"{key}={value}\n")
    print(
        f"check-plugin-release: ok — {LINE} {facts['version']} at {facts['sha'][:12]} writes "
        f"{facts['tag']}; the page is proven on {facts['engine']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
