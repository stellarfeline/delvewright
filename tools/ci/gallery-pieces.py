#!/usr/bin/env python3
"""Pack what the gallery's piece generation wrote, so the jobs that read it take that one copy.

## What this is for

`tools/ci/gallery-prefabs.py` writes the gallery's prefab directory and, beside
it, the generated files it puts inside `gallery/` (skins, textures, the stand-in
reference images). `ci.yml` generates them once, in `gallery pieces`, and every
other gallery job takes that output instead of generating its own — the way
`delvec-binary` hands the engine down.

A hand-down that carries less than the generator wrote is a consumer judging a
different tree from the one a creator's machine has. So this packs NOTHING BY
NAME: it packs every file in the checkout that git does not track, which on a
fresh runner after generation is exactly what generation wrote, and a list of
roots typed here cannot go stale when the generator starts writing somewhere new.

## What it excludes, and why

Two classes of untracked file are build caches, never pieces, and are skipped
and COUNTED on every run:

- any path with a `target/` segment — cargo's output (the engine the run was
  handed sits at `target/debug/delvec`; the generator's build is under
  `prefabs/gallery-generator/target/`);
- any path with a `__pycache__/` segment — the bytecode of the tools that ran.

## What it refuses

- a tracked file the generation modified: the hand-down carries untracked files
  only, so a consumer would read the committed bytes instead;
- an archive of zero files: a hand-down of nothing agrees with everything.

The archive lists its members in sorted order. Its sha256 travels as the job's
output (computed by the workflow step, like `delvec-binary`'s), and
`.github/actions/gallery-pieces` refuses a download that does not hash to it.

Usage:
  python3 tools/ci/gallery-pieces.py --out gallery-pieces.tar [--repo DIR]

Exit 0 packed, 1 refused.
"""

from __future__ import annotations

import argparse
import subprocess
import sys
import tarfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
CACHE_SEGMENTS = ("target", "__pycache__")


def die(msg: str) -> int:
    print(f"gallery-pieces: FAIL — {msg}", file=sys.stderr)
    return 1


def git(repo: Path, *args: str) -> str:
    r = subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True)
    if r.returncode != 0:
        raise SystemExit(f"gallery-pieces: `git {' '.join(args)}` exited {r.returncode}: {r.stderr.strip()}")
    return r.stdout


def is_cache(rel: str) -> bool:
    return any(seg in CACHE_SEGMENTS for seg in rel.split("/")[:-1])


def classify(untracked: list[str]) -> tuple[list[str], list[str]]:
    """Split untracked paths into (pieces, build caches), each sorted."""
    pieces = sorted(p for p in untracked if not is_cache(p))
    caches = sorted(p for p in untracked if is_cache(p))
    return pieces, caches


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--out", required=True, type=Path, help="the archive to write")
    ap.add_argument("--repo", type=Path, default=REPO)
    args = ap.parse_args(argv)
    repo = args.repo.resolve()

    modified = [line for line in git(repo, "status", "--porcelain", "--untracked-files=no").splitlines() if line]
    if modified:
        return die(
            f"the generation modified {len(modified)} tracked file(s); the hand-down carries "
            "untracked files only, so a consumer would read the committed bytes instead:\n  "
            + "\n  ".join(modified)
        )

    out = args.out.resolve()
    untracked = [p for p in git(repo, "ls-files", "--others", "-z").split("\0") if p]
    untracked = [p for p in untracked if (repo / p).resolve() != out]
    pieces, caches = classify(untracked)
    if not pieces:
        return die(
            f"ZERO untracked files outside the build caches ({len(caches)} cache file(s) skipped); "
            "the generation wrote nothing to hand down"
        )

    by_top: dict[str, int] = {}
    for p in pieces:
        top = p.split("/", 2)
        key = "/".join(top[:2]) if top[0] == "gallery" and len(top) > 2 else top[0]
        by_top[key] = by_top.get(key, 0) + 1

    out.parent.mkdir(parents=True, exist_ok=True)
    with tarfile.open(out, "w", format=tarfile.PAX_FORMAT) as tar:
        for p in pieces:
            tar.add(repo / p, arcname=p, recursive=False)

    sys.stdout.reconfigure(newline="\n")
    print(
        f"gallery-pieces: packed {len(pieces)} file(s) into {out.name} — "
        + ", ".join(f"{k}: {v}" for k, v in sorted(by_top.items()))
        + f"; skipped {len(caches)} build-cache file(s) under `target/` or `__pycache__/`"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
