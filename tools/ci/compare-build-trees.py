#!/usr/bin/env python3
"""Compare two `delvec build` output trees file by file (spec-0106 §6).

    python3 tools/ci/compare-build-trees.py A B [--allow PATH[#KEY]]... [--report-only DIR]...

Every file of the compared subtrees — `datapack/`, `creator-datapack/`,
`packtest-datapack/`, `server/`, and `resourcepack.zip` unpacked in memory as
`resourcepack.zip!/<entry>` — is hashed in both trees and listed with its
SHA-256 on each side. A file present on one side only is a difference.

`--allow PATH` admits a difference in that file; `--allow PATH#KEY` admits a
difference in a JSON file only where its top-level object differs by that key
(present on one side, absent or different on the other) and nowhere else, so
an allowlisted key cannot cover a second, unnamed change in the same file.
The script exits 1 on any difference no allowlist entry names, and on an
allowlist entry that matched no difference (a stale admission is a claim
about a tree that no longer holds).

`--report-only DIR` (repeatable, e.g. `validation`) lists the differences of
another subtree without judging them: records one form of a build writes and
the other does not are compared and named, never failed on.

The binding line states the files compared per side, the differing files and
the allowlisted ones; a tree with zero compared files is refused (a comparison
over nothing is not a pass).
"""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import sys
import zipfile
from pathlib import Path

JUDGED = ("datapack", "creator-datapack", "packtest-datapack", "server")
PACK = "resourcepack.zip"


def files_of(root: Path, subtrees: tuple[str, ...]) -> dict[str, bytes]:
    """Every file under the judged subtrees, by tree-relative path."""
    out: dict[str, bytes] = {}
    for sub in subtrees:
        base = root / sub
        if not base.is_dir():
            continue
        for p in sorted(base.rglob("*")):
            if p.is_file():
                out[p.relative_to(root).as_posix()] = p.read_bytes()
    return out


def pack_of(root: Path) -> dict[str, bytes]:
    """The resource pack's entries, unpacked in memory."""
    p = root / PACK
    if not p.is_file():
        return {}
    out: dict[str, bytes] = {}
    with zipfile.ZipFile(io.BytesIO(p.read_bytes())) as z:
        for name in sorted(z.namelist()):
            if not name.endswith("/"):
                out[f"{PACK}!/{name}"] = z.read(name)
    return out


def sha(b: bytes | None) -> str:
    return "-" if b is None else hashlib.sha256(b).hexdigest()


def json_key_diff(a: bytes | None, b: bytes | None) -> set[str] | None:
    """The top-level keys two JSON objects differ by, or None if either side
    is not a JSON object."""
    try:
        x = json.loads(a) if a is not None else None
        y = json.loads(b) if b is not None else None
    except (ValueError, UnicodeDecodeError):
        return None
    if not isinstance(x, dict) or not isinstance(y, dict):
        return None
    return {k for k in set(x) | set(y) if x.get(k, KeyError) != y.get(k, KeyError)}


def compare(a: dict[str, bytes], b: dict[str, bytes]) -> list[str]:
    return [k for k in sorted(set(a) | set(b)) if a.get(k) != b.get(k)]


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("a", type=Path)
    ap.add_argument("b", type=Path)
    ap.add_argument("--allow", action="append", default=[])
    ap.add_argument("--report-only", action="append", default=[])
    ap.add_argument("--table", action="store_true", help="print every file's hash on each side")
    args = ap.parse_args(argv)

    a = {**files_of(args.a, JUDGED), **pack_of(args.a)}
    b = {**files_of(args.b, JUDGED), **pack_of(args.b)}
    if not a or not b:
        print(
            f"compare-build-trees: refused — {len(a)} file(s) under {args.a} and {len(b)} under "
            f"{args.b}: a comparison over nothing is not a pass",
            file=sys.stderr,
        )
        return 1

    allow_files: set[str] = set()
    allow_keys: dict[str, set[str]] = {}
    for entry in args.allow:
        path, _, key = entry.partition("#")
        if key:
            allow_keys.setdefault(path, set()).add(key)
        else:
            allow_files.add(path)

    if args.table:
        for k in sorted(set(a) | set(b)):
            print(f"{sha(a.get(k))}  {sha(b.get(k))}  {k}")

    differing = compare(a, b)
    used: set[str] = set()
    failed: list[str] = []
    admitted: list[str] = []
    for k in differing:
        if k in allow_files:
            used.add(k)
            admitted.append(f"{k} (whole file admitted)")
            continue
        if k in allow_keys:
            keys = json_key_diff(a.get(k), b.get(k))
            if keys is not None and keys and keys <= allow_keys[k]:
                used.add(k)
                admitted.append(f"{k} (by key: {', '.join(sorted(keys))})")
                continue
            failed.append(
                f"{k}: differs beyond its admitted key(s) {sorted(allow_keys[k])} — "
                f"differing top-level key(s): {sorted(keys) if keys is not None else 'not a JSON object'}"
            )
            continue
        failed.append(f"{k}: {sha(a.get(k))[:16]} vs {sha(b.get(k))[:16]}")

    stale = sorted((allow_files | set(allow_keys)) - used)

    for d in args.report_only:
        ra, rb = files_of(args.a, (d,)), files_of(args.b, (d,))
        diff = compare(ra, rb)
        print(
            f"report-only {d}/: {len(ra)} file(s) in A, {len(rb)} in B, {len(diff)} differing"
            + "".join(
                f"\n  {k}: {'only in A' if k not in rb else 'only in B' if k not in ra else 'differs'}"
                for k in diff
            )
        )

    for line in admitted:
        print(f"admitted: {line}")
    for line in failed:
        print(f"DIFFERS: {line}")
    for s in stale:
        print(f"STALE: --allow {s} matched no difference")
    print(
        f"compare-build-trees binding: {len(a)} file(s) in A, {len(b)} in B, "
        f"{len(set(a) | set(b))} compared; {len(differing)} differing, {len(admitted)} admitted "
        f"by {len(args.allow)} allowlist entr(y/ies), {len(failed)} not admitted, {len(stale)} stale."
    )
    return 1 if failed or stale else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
