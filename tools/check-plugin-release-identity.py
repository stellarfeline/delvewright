#!/usr/bin/env python3
"""A plugin release records what `main` delivered, and proves the two are one thing.

WHY THIS EXISTS

A creator receives the `delvewright` plugin from the marketplace at `main`, and
what decides delivery is the merge that moves `plugin.json` `version` — a tag
decides nothing for a default-branch subscriber (ADR-0028, *Context*). So a
plugin Release can only RECORD a delivery, and it is worth having only if what it
records is what was delivered. `tools/check-skill-page.py`'s version-bump rule
holds that a change under the plugin root moves the version; the consequence this
checks is that **every first-parent commit on `main` carrying one plugin version
carries a byte-identical plugin root**, so an archive made from any of them is
the bytes a creator holding that version has (ADR-0028 §3).

WHAT IT REFUSES, in order, before anything is archived

1. the commit's `plugin.json` names a plugin other than `delvewright`, or a
   version outside strict semver;
2. with `--tag`: the tag is outside the grammar (`tools/lib/release_tags.py`) or
   names a version the commit does not carry — a `v1.4.3` tag and a
   `delvewright--v1.4.3` tag on a `1.4.4` commit are both refused here;
3. the commit is not an ancestor of `--main`;
4. the commit is not itself one of `--main`'s first-parent commits carrying that
   version (a commit reached only through a merge's second parent was never what
   `main` served);
5. the first-parent commits carrying the version do not agree on ONE plugin-root
   tree.

BINDING: the count of first-parent commits carrying the version and the count of
distinct trees among them — `N commits carry X, 1 distinct tree` — out of the
first-parent commits read. N is at least 1 whenever (4) passes, so a green can
never be a zero.

OUTPUT: with `--github-output FILE`, appends `tag`, `version`, `commit` and
`tree` lines for the workflow.

Offline, stdlib only, `git` for history. Exit 0 clean, 1 refused, 2 unreadable.

Usage:
  python3 tools/check-plugin-release-identity.py --commit REV [--tag TAG]
      [--main origin/main] [--repo DIR] [--github-output FILE]
"""

from __future__ import annotations

import argparse
import json
import pathlib
import subprocess
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent / "lib"))

import release_tags  # noqa: E402

TOOL = "check-plugin-release-identity"
REPO = pathlib.Path(__file__).resolve().parent.parent
PLUGIN_ROOT = ".claude/skills/delvewright"
MANIFEST = f"{PLUGIN_ROOT}/.claude-plugin/plugin.json"
NAME = "delvewright"


class Unreadable(Exception):
    pass


def git(repo: pathlib.Path, *args: str, stdin: str | None = None) -> str:
    result = subprocess.run(
        ["git", "-C", str(repo), *args], capture_output=True, text=True, input=stdin
    )
    if result.returncode != 0:
        raise Unreadable(f"`git {' '.join(args)}` failed: {result.stderr.strip()}")
    return result.stdout


def manifest_at(repo: pathlib.Path, commit: str) -> dict:
    raw = git(repo, "show", f"{commit}:{MANIFEST}")
    try:
        doc = json.loads(raw)
    except json.JSONDecodeError as exc:
        raise Unreadable(f"{commit[:8]}:{MANIFEST} is not JSON: {exc}")
    if not isinstance(doc, dict):
        raise Unreadable(f"{commit[:8]}:{MANIFEST} is not an object")
    return doc


def first_parent_versions(repo: pathlib.Path, main: str) -> list[tuple[str, str | None, str | None]]:
    """`(commit, plugin version or None, plugin-root tree or None)` for every
    first-parent commit of `main`, newest first — read in two batch calls."""
    commits = git(repo, "rev-list", "--first-parent", main).split()
    if not commits:
        raise Unreadable(f"{main} has no first-parent history")
    blobs = subprocess.run(
        ["git", "-C", str(repo), "cat-file", "--batch"],
        input="".join(f"{c}:{MANIFEST}\n" for c in commits).encode(),
        capture_output=True,
    )
    if blobs.returncode != 0:
        raise Unreadable(f"`git cat-file --batch` failed: {blobs.stderr.decode().strip()}")
    trees = git(
        repo, "cat-file", "--batch-check=%(objectname) %(objecttype)",
        stdin="".join(f"{c}:{PLUGIN_ROOT}\n" for c in commits),
    ).splitlines()
    if len(trees) != len(commits):
        raise Unreadable(f"read {len(trees)} tree answers for {len(commits)} commits")

    out: list[tuple[str, str | None, str | None]] = []
    data = blobs.stdout
    pos = 0
    for commit, tree_line in zip(commits, trees):
        end = data.index(b"\n", pos)
        header = data[pos:end].decode()
        pos = end + 1
        version: str | None = None
        if not header.endswith(" missing"):
            size = int(header.split()[2])
            body = data[pos : pos + size]
            pos += size + 1
            try:
                doc = json.loads(body)
                if isinstance(doc, dict) and isinstance(doc.get("version"), str):
                    version = doc["version"]
            except json.JSONDecodeError:
                version = None
        parts = tree_line.split()
        tree = parts[0] if len(parts) == 2 and parts[1] == "tree" else None
        out.append((commit, version, tree))
    return out


def judge(repo: pathlib.Path, commit_rev: str, tag: str | None, main: str) -> dict[str, str]:
    commit = git(repo, "rev-parse", "--verify", f"{commit_rev}^{{commit}}").strip()
    doc = manifest_at(repo, commit)
    if doc.get("name") != NAME:
        raise release_tags.Refused(
            f"{commit[:8]}:{MANIFEST} names plugin {doc.get('name')!r}; this workflow releases {NAME!r}"
        )
    version = doc.get("version")
    if not isinstance(version, str):
        raise release_tags.Refused(f"{commit[:8]}:{MANIFEST} carries no string `version`")
    release_tags.version_key(version)
    expected = release_tags.tag_for(NAME, version)
    if tag is not None:
        release_tags.identity(NAME, tag, version)
        print(f"  ok   tag {tag} names {NAME} {version}, the version {commit[:8]} carries")
    else:
        print(f"  ok   {commit[:8]} carries {NAME} {version}; the tag this release writes is {expected}")

    ancestor = subprocess.run(
        ["git", "-C", str(repo), "merge-base", "--is-ancestor", commit, main], capture_output=True
    )
    if ancestor.returncode == 1:
        raise release_tags.Refused(
            f"{commit[:8]} is not an ancestor of {main}. A plugin release records what "
            f"`main` delivered, and nothing off `main` was delivered."
        )
    if ancestor.returncode != 0:
        raise Unreadable(f"`git merge-base --is-ancestor` failed: {ancestor.stderr.decode().strip()}")
    print(f"  ok   {commit[:8]} is an ancestor of {main}")

    history = first_parent_versions(repo, main)
    carrying = [(c, t) for c, v, t in history if v == version]
    trees = sorted({t or "(no plugin root)" for _c, t in carrying})
    binding = (
        f"{len(carrying)} of {len(history)} first-parent commit(s) on {main} carry {version}, "
        f"{len(trees)} distinct tree(s)"
    )
    if commit not in {c for c, _t in carrying}:
        raise release_tags.Refused(
            f"{binding}; {commit[:8]} is not one of them. It is reachable from {main} only "
            f"through a merge's second parent, so no creator was ever served it — tag the "
            f"first-parent commit that carries {version} instead."
        )
    if len(trees) != 1:
        listing = "\n".join(
            f"      {c[:8]} {t or '(no plugin root)'}" for c, t in carrying
        )
        raise release_tags.Refused(
            f"{binding}. Every commit `main` served under one version must carry one plugin "
            f"root, or creators holding {version} hold different bytes and no single archive "
            f"is what they received:\n{listing}"
        )
    tree = trees[0]
    print(f"  ok   {binding} ({tree})")
    return {"tag": expected, "version": version, "commit": commit, "tree": tree}


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description="a plugin release records what main delivered")
    ap.add_argument("--commit", required=True)
    ap.add_argument("--tag", default=None)
    ap.add_argument("--main", default="origin/main")
    ap.add_argument("--repo", type=pathlib.Path, default=REPO)
    ap.add_argument("--github-output", type=pathlib.Path, default=None)
    args = ap.parse_args(argv)
    sys.stdout.reconfigure(newline="\n")  # CRLF-proof: tools/check-python-shell-newlines.py
    try:
        out = judge(args.repo, args.commit, args.tag, args.main)
    except release_tags.Refused as exc:
        print(f"{TOOL}: REFUSED — {exc}", file=sys.stderr)
        return 1
    except Unreadable as exc:
        print(f"{TOOL}: UNREADABLE — {exc}", file=sys.stderr)
        return 2
    if args.github_output is not None:
        with args.github_output.open("a", encoding="utf-8", newline="\n") as fh:
            for key, value in out.items():
                fh.write(f"{key}={value}\n")
    print(f"{TOOL}: OK — {out['tag']} at {out['commit'][:8]}, plugin root {out['tree']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
