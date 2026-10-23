#!/usr/bin/env python3
"""Every campaign the demo queue names is a directory the content pin carries.

`docs/demo-levels.md` names a built level in its row's Status cell by its
address in the content repository, `campaigns/<id>`. A level declares
`test_level: true` in its `world.json` and carries no design record; nothing in
the creator's toolchain reads this queue, so whether a row's address is real is
this repository's own concern, and this is where it is asked.

The answer is read out of the content repository's object store at
`versions.toml` `[content].sha` — never a working tree, whose branch says
nothing about the pin — through the repository the `campaigns/` path names (a
dev symlink locally, `checkout-content` in CI). An absent repository, or one
that does not carry the pinned commit, is a failure with its repair, never a
skip: a check that could not look has not looked.

A row naming no address is not judged: a pending row, a covered row and a row
whose demo is not a level name none. A queue naming zero addresses passes with
its count stated.

Binding count, every run: rows read, Status cells naming an address, distinct
addresses, and the campaigns the pin carries.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))

from lib import demo_queue  # noqa: E402

# The whole path segment, as `demo_queue` reads an address: `campaigns/the-ferry`
# is not `campaigns/the-ferry-bell`.
ADDRESS = re.compile(r"(?<![\w/.-])campaigns/([a-z0-9][a-z0-9-]*)(?![\w-])")


def pinned_sha(root: Path = ROOT) -> str:
    """`versions.toml` `[content].sha`."""
    with open(root / "versions.toml", "rb") as f:
        return tomllib.load(f)["content"]["sha"]


def carried(content: Path, sha: str) -> set[str]:
    """The campaign directories the content repository holds at `sha`.

    Raises `SystemExit` naming the repair when the repository cannot be read.
    """
    if not (content / ".git").exists() and not (content / "HEAD").exists():
        raise SystemExit(
            f"check-demo-queue: {content} is not a git repository — the content "
            "repository is read out of its object store at the pin. Clone "
            "stellarfeline/delvewright-campaigns and point `campaigns` at it "
            "(docs/reference/worktree-bootstrap.md), or in CI run the "
            "`checkout-content` action first."
        )
    r = subprocess.run(
        ["git", "-C", str(content), "ls-tree", "--name-only", sha, "campaigns/"],
        capture_output=True,
        text=True,
    )
    if r.returncode != 0:
        raise SystemExit(
            f"check-demo-queue: the content repository at {content} cannot list "
            f"`campaigns/` at the pinned {sha}: {r.stderr.strip()}. Fetch the pin "
            "(`git -C campaigns fetch origin`) and run again."
        )
    out = set()
    for line in r.stdout.splitlines():
        name = line.strip()
        if name.startswith("campaigns/") and name != "campaigns/README.md":
            out.add(name[len("campaigns/") :])
    return out


def judge(queue_text: str, have: set[str]) -> tuple[list[str], dict[str, int]]:
    """`(findings, binding)` for a queue against the campaigns the pin carries."""
    rows, _ = demo_queue.read(queue_text)
    findings: list[str] = []
    cells = 0
    addresses: set[str] = set()
    for lineno, row in rows:
        if not row:
            continue
        named = ADDRESS.findall(row[-1])
        if not named:
            continue
        cells += 1
        for cid in named:
            addresses.add(cid)
            if cid not in have:
                findings.append(
                    f"{demo_queue.PATH}:{lineno}: the Status cell names "
                    f"`campaigns/{cid}`, which the content pin does not carry. A "
                    "row names a level by the directory it has on the content "
                    "repository's main line; a level on a branch has no address "
                    "yet — say so in the Status cell, or merge the level and move "
                    "the pin."
                )
    binding = {
        "rows": len(rows),
        "cells": cells,
        "addresses": len(addresses),
        "carried": len(have),
    }
    return findings, binding


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    p.add_argument(
        "--content",
        type=Path,
        default=ROOT / "campaigns",
        help="the content repository (default: the `campaigns/` path)",
    )
    p.add_argument(
        "--sha",
        default=None,
        help="the content revision (default: versions.toml [content].sha)",
    )
    args = p.parse_args(argv)
    sha = args.sha or pinned_sha()
    have = carried(args.content, sha)
    text = (ROOT / demo_queue.PATH).read_text(encoding="utf-8")
    findings, b = judge(text, have)
    print(
        f"check-demo-queue binding: {b['rows']} row(s) read, {b['cells']} Status "
        f"cell(s) naming {b['addresses']} address(es); the content pin "
        f"{sha[:12]} carries {b['carried']} campaign(s)"
    )
    for f in findings:
        print(f"error: {f}", file=sys.stderr)
    return 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main())
