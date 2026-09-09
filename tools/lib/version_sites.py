#!/usr/bin/env python3
"""**Where a release number lives, as one object that is both printed and checked.**

## What this exists to remove

`tools/crates-io-publish.sh` refuses a version that is on crates.io with
different bytes, and the refusal is only useful because of what it prints next:
the list of places the reader must move the number to. That list was a literal
typed beside the `echo`s, and both halves of it had rotted without anything
noticing, because **nothing anywhere read it**:

  * it named `SUPPORTED_DSL_VERSION`. The tree's constant is `DSL_VERSION`, and
    has been for as long as `validation/check-versions.sh` has asserted it. A
    diagnostic naming a symbol that does not exist sends its reader looking for
    nothing;
  * it said *all four of these carry it* and listed four. There are seven, and
    the missing one is the root `Cargo.toml` `[workspace.dependencies]` pin —
    omitting it is the worst kind of omission, because a reader who follows the
    message exactly then gets `failed to select a version for the requirement
    delvewright-dsl = "=<old>"` from the very next `--locked` build. Measured:
    that is what happened to the round that moved 0.22.0 to 0.22.1;
  * and the non-DSL branch told a reader bumping the ENGINE version to move
    `[workspace.dependencies]` too. That table holds exactly one entry,
    `delvewright-dsl`, and the engine's number appears nowhere in it.

The count in the sentence is now `len(rows)` and the rows are this module's, so
"four" cannot disagree with a list of five again.

## Why the rows are checked rather than merely tidied

A corrected literal is a literal, and it rots the same way the last one did. So
every row here carries how to FIND itself, and `verify` resolves all of them
against the tree: the file must exist, and the site must actually hold the
number `versions.toml` declares. A renamed constant, a moved table or a site
that stopped carrying the number reds `crates-io-publish.sh` on its next run —
which is every run, not only the rare one that prints the advice, because a gate
bound to its own failure path is a gate nobody exercises.

`validation/check-versions.sh` asserts the same numbers agree, and this is not a
second authority over that: it owns the MESSAGE's rows. What it makes
impossible is a message that names a place the reader cannot find, which is a
different failure from two numbers disagreeing and is the one that had happened.
The two are kept from drifting the cheap way — this module resolves each row
against the tree, so a site `check-versions.sh` moves and this one does not
follow is red here on the next run rather than silently wrong in prose.
"""

from __future__ import annotations

import argparse
import re
import sys
import tomllib
from pathlib import Path

# --- how a row says where it is ---------------------------------------------
#
# Three kinds, because three shapes of file hold a version and a single regex
# over all of them would be the "one shared parse rule" written as a wildcard.
#
#   toml   — a key in a named table, compared as a VALUE (`tomllib`, so the
#            table is resolved rather than guessed at from indentation);
#   regex  — a Rust source line, matched with the version interpolated;
#   lock   — a `Cargo.lock` `[[package]]` entry, which is two lines and cannot
#            be matched by one, and which cargo owns: it is listed so the reader
#            knows it moves, and named as cargo's so nobody hand-edits it.

ROWS: dict[str, list[dict[str, object]]] = {
    # The DSL crate's version IS the `dsl_version` (ADR-0024), so every one of
    # these is a statement of the format's number.
    "dsl": [
        {
            "path": "crates/dsl/Cargo.toml",
            "label": "[package] version",
            "kind": "toml",
            "table": ["package"],
            "key": "version",
        },
        {
            "path": "crates/dsl/src/envelope.rs",
            "label": "DSL_VERSION",
            "kind": "regex",
            "pattern": r'pub\s+const\s+DSL_VERSION\s*:\s*&str\s*=\s*"{v}"\s*;',
        },
        {
            "path": "versions.toml",
            "label": "[engine] dsl_crate_version",
            "kind": "toml",
            "table": ["engine"],
            "key": "dsl_crate_version",
        },
        {
            "path": "versions.toml",
            "label": "[engine] dsl_crate_req (=<version>)",
            "kind": "toml",
            "table": ["engine"],
            "key": "dsl_crate_req",
            "prefix": "=",
        },
        {
            "path": "Cargo.toml",
            "label": "[workspace.dependencies] delvewright-dsl version (=<version>)",
            "kind": "toml",
            "table": ["workspace", "dependencies", "delvewright-dsl"],
            "key": "version",
            "prefix": "=",
        },
        {
            "path": "Cargo.lock",
            "label": "delvewright-dsl — cargo's, never edited: cargo update -p delvewright-dsl",
            "kind": "lock",
            "package": "delvewright-dsl",
        },
        {
            "path": "prefabs/Cargo.lock",
            "label": "delvewright-dsl — cargo's, never edited: "
            "cargo update -p delvewright-dsl --manifest-path prefabs/Cargo.toml",
            "kind": "lock",
            "package": "delvewright-dsl",
        },
    ],
    # The engine release line. Members inherit it (`version.workspace = true`),
    # so no member manifest carries it, and nothing in the workspace depends on
    # the engine crate, so `[workspace.dependencies]` does not carry it either.
    "engine": [
        {
            "path": "versions.toml",
            "label": "[engine] version",
            "kind": "toml",
            "table": ["engine"],
            "key": "version",
        },
        {
            "path": "Cargo.toml",
            "label": "[workspace.package] version (every member inherits it: version.workspace = true)",
            "kind": "toml",
            "table": ["workspace", "package"],
            "key": "version",
        },
        {
            "path": "Cargo.lock",
            "label": "delvec — cargo's, never edited: cargo update -p delvec",
            "kind": "lock",
            "package": "delvec",
        },
    ],
}


def _declared(root: Path) -> dict[str, str]:
    """The two numbers, from the one file that declares them."""
    engine = tomllib.loads((root / "versions.toml").read_text(encoding="utf-8"))["engine"]
    return {"dsl": engine["dsl_crate_version"], "engine": engine["version"]}


def _resolve(root: Path, row: dict[str, object], version: str) -> str | None:
    """`None` when the row resolves; otherwise why it does not."""
    p = root / str(row["path"])
    if not p.is_file():
        return f"no such file: {row['path']}"
    text = p.read_text(encoding="utf-8")
    want = str(row.get("prefix", "")) + version

    if row["kind"] == "regex":
        pat = str(row["pattern"]).format(v=re.escape(version))
        if re.search(pat, text) is None:
            return f"nothing in {row['path']} matches /{pat}/ — the symbol the message names is not there"
        return None

    if row["kind"] == "toml":
        doc: object = tomllib.loads(text)
        trail: list[str] = []
        for key in row["table"]:  # type: ignore[union-attr]
            trail.append(str(key))
            if not isinstance(doc, dict) or key not in doc:
                return f"{row['path']} has no table [{'.'.join(trail)}]"
            doc = doc[key]
        if not isinstance(doc, dict) or row["key"] not in doc:
            return f"{row['path']} [{'.'.join(trail)}] has no key `{row['key']}`"
        got = doc[str(row["key"])]
        if got != want:
            return f"{row['path']} [{'.'.join(trail)}] {row['key']} is {got!r}, not {want!r}"
        return None

    if row["kind"] == "lock":
        # A `[[package]]` entry is `name` then `version`; tomllib reads the whole
        # lock, which is what a lock is FOR — a regex over two lines would be a
        # private re-implementation of the format for one caller.
        pkgs = tomllib.loads(text).get("package", [])
        hits = [q for q in pkgs if q.get("name") == row["package"]]
        if not hits:
            return f"{row['path']} holds no package `{row['package']}`"
        if len(hits) != 1:
            return f"{row['path']} holds {len(hits)} packages named `{row['package']}`"
        if hits[0].get("version") != version:
            return f"{row['path']} has {row['package']} {hits[0].get('version')!r}, not {version!r}"
        return None

    return f"unknown row kind {row['kind']!r}"


def advise(kind: str, version: str) -> list[str]:
    """The message's own lines. The count comes from the list, never from prose."""
    rows = ROWS[kind]
    width = max(len(str(r["path"])) for r in rows)
    out = [f"    {str(r['path']):<{width}}  {r['label']}" for r in rows]
    return out


def verify(root: Path) -> int:
    """Every row of every kind, resolved against the tree. Exit status."""
    declared = _declared(root)
    findings: list[str] = []
    checked = 0
    for kind, rows in ROWS.items():
        for row in rows:
            checked += 1
            why = _resolve(root, row, declared[kind])
            if why is not None:
                findings.append(f"  {kind}: {row['path']} ({row['label']}): {why}")
    total = sum(len(r) for r in ROWS.values())
    assert checked == total, f"resolved {checked} of {total} row(s)"
    if findings:
        print(
            "version-sites: the advice `tools/crates-io-publish.sh` prints names a place "
            "that is not in this tree.",
            file=sys.stderr,
        )
        print(
            "  This is a defect in the MESSAGE, not in your bump: a reader who follows it "
            "will not find what it names.",
            file=sys.stderr,
        )
        for f in findings:
            print(f, file=sys.stderr)
        print(f"  Fix the rows in {__file__}.", file=sys.stderr)
        return 1
    counts = ", ".join(f"{k} {len(v)}" for k, v in ROWS.items())
    print(
        f"version-sites: {checked} row(s) resolved against the tree ({counts}); "
        f"dsl {declared['dsl']}, engine {declared['engine']}"
    )
    return 0


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("command", choices=["verify", "advise"])
    ap.add_argument("--root", default=".", help="repository root")
    ap.add_argument("--kind", choices=sorted(ROWS), help="advise: which number")
    ap.add_argument("--version", help="advise: the number being moved away from")
    ap.add_argument("--count", action="store_true", help="advise: print the row count only")
    a = ap.parse_args(argv)
    root = Path(a.root).resolve()
    if a.command == "verify":
        return verify(root)
    if not a.kind or not a.version:
        ap.error("advise needs --kind and --version")
    if a.count:
        print(len(ROWS[a.kind]))
        return 0
    for line in advise(a.kind, a.version):
        print(line)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
