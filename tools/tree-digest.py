#!/usr/bin/env python3
"""A path-independent digest of an output tree, and the comparison of two of them.

## What this is for

ADR-0006 promises that the same DSL and the same seed produce a byte-identical
datapack and world. Until now every gate holding that promise ran on ONE machine:
`tests/cli.rs::build_is_byte_identical_across_runs` builds twice on one runner,
and the prefab-generator job runs each generator twice on one runner. Both prove
determinism *within* a host — which is the property that catches an unseeded RNG
or a hash-ordered iteration, and not the one that catches a libm difference, a
float-formatting difference, or a filesystem-ordering difference between
platforms.

This tool is the instrument for the cross-OS half: each OS writes one digest of
its build, and the digests are compared.

## Why paths are RELATIVE and content is hashed separately

The recorded trap is that hashing the output of `shasum` hashes the file PATHS
along with the contents — and two checkouts live at different absolute paths, so
that comparison can only ever disagree. The naive repair, dropping paths
entirely, fails in the other direction: a build that wrote the same bytes under a
different NAME would compare equal, and a renamed file is exactly the kind of
divergence a cross-platform run produces.

So the manifest holds `sha256(content)` beside the path RELATIVE to the tree
root. The absolute location never enters, and a rename is a difference.

Directories are entries too, with no hash. A build that stopped writing an empty
directory is invisible to a file walk, and "invisible" is the direction that
reads as a pass.

Anything that is neither a regular file nor a directory — a symlink, a socket, a
device node — is a REFUSAL rather than a skip, because a tool that decides part
of its subject is not its business owes an account of what it did not read.

## Usage

    tools/tree-digest.py --root DIR --out FILE      # write one side's manifest
    tools/tree-digest.py --compare A B              # exit 1 on any difference
"""

from __future__ import annotations

import argparse
import hashlib
import sys
from pathlib import Path

FORMAT = "tree-digest v1"
DIR_MARK = "directory" + " " * 55  # same width as a sha256, so columns line up


def die(msg: str) -> int:
    print(f"tree-digest: FAIL — {msg}", file=sys.stderr)
    return 1


def file_hash(p: Path) -> str:
    h = hashlib.sha256()
    with p.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def manifest(root: Path) -> tuple[list[str], int, int]:
    """Sorted `<hash-or-marker>  <relpath>` lines, plus the file and directory counts."""
    entries: list[tuple[str, str]] = []
    files = dirs = 0
    for p in sorted(root.rglob("*"), key=lambda q: q.relative_to(root).as_posix()):
        rel = p.relative_to(root).as_posix()
        if p.is_symlink() or not (p.is_file() or p.is_dir()):
            raise ValueError(
                f"`{rel}` is neither a regular file nor a directory. This digest "
                "accounts for everything under the root or it accounts for "
                "nothing: a skipped entry is a difference nobody would see."
            )
        if p.is_dir():
            entries.append((DIR_MARK, rel + "/"))
            dirs += 1
        else:
            entries.append((file_hash(p), rel))
            files += 1
    return [f"{h}  {rel}" for h, rel in sorted(entries, key=lambda e: e[1])], files, dirs


def write(root: Path, out: Path) -> int:
    if not root.is_dir():
        return die(f"--root `{root}` is not a directory")
    try:
        lines, files, dirs = manifest(root)
    except ValueError as e:
        return die(str(e))
    # Vacuity: a digest of nothing compares equal to a digest of nothing.
    if files == 0:
        return die(
            f"`{root}` holds ZERO files. Two empty trees agree, so a digest taken "
            "here would be a green that binds to nothing."
        )
    body = "\n".join(lines) + "\n"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(f"{FORMAT}\n{body}", encoding="utf-8")
    print(
        f"tree-digest: {files} file(s), {dirs} director(y/ies) under {root} "
        f"-> {out}; digest sha256:{hashlib.sha256(body.encode()).hexdigest()}"
    )
    return 0


def read(p: Path) -> dict[str, str]:
    lines = p.read_text(encoding="utf-8").splitlines()
    if not lines or lines[0] != FORMAT:
        raise ValueError(f"`{p}` does not start with `{FORMAT}`")
    out: dict[str, str] = {}
    for line in lines[1:]:
        h, _, rel = line.partition("  ")
        out[rel] = h.strip()
    return out


def compare(a: Path, b: Path) -> int:
    try:
        left, right = read(a), read(b)
    except (OSError, ValueError) as e:
        return die(str(e))
    if not left or not right:
        return die(
            f"one side is empty ({len(left)} entries in {a}, {len(right)} in {b}); "
            "an empty manifest agrees with everything"
        )

    only_a = sorted(set(left) - set(right))
    only_b = sorted(set(right) - set(left))
    differ = sorted(k for k in set(left) & set(right) if left[k] != right[k])

    if not (only_a or only_b or differ):
        print(
            f"tree-digest: IDENTICAL — {len(left)} entries compared, "
            f"{sum(1 for v in left.values() if v != DIR_MARK.strip())} of them files"
        )
        return 0

    print(
        f"tree-digest: DIVERGED — {len(differ)} entry(ies) differ, "
        f"{len(only_a)} only in {a.name}, {len(only_b)} only in {b.name}. "
        "ADR-0006 promises the same inputs produce the same bytes; these two "
        "hosts disagree, so one of them is reading a platform difference into "
        "the output.",
        file=sys.stderr,
    )
    for rel in differ:
        print(f"  differs: {rel}\n    {a.name}: {left[rel]}\n    {b.name}: {right[rel]}",
              file=sys.stderr)
    for rel in only_a:
        print(f"  only in {a.name}: {rel}", file=sys.stderr)
    for rel in only_b:
        print(f"  only in {b.name}: {rel}", file=sys.stderr)
    return 1


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--root", help="the tree to digest")
    ap.add_argument("--out", help="where the manifest is written")
    ap.add_argument(
        "--compare",
        nargs=2,
        metavar=("A", "B"),
        help="two manifests written by --root/--out; exits 1 on any difference",
    )
    args = ap.parse_args()

    if args.compare:
        if args.root or args.out:
            return die("--compare takes no --root/--out")
        return compare(Path(args.compare[0]), Path(args.compare[1]))
    if not (args.root and args.out):
        return die("give --root DIR --out FILE, or --compare A B")
    return write(Path(args.root), Path(args.out))


if __name__ == "__main__":
    raise SystemExit(main())
