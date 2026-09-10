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
  * it said *all four of these carry it* and listed four. It was already a set
    of seven, and the missing one is the root `Cargo.toml`
    `[workspace.dependencies]` pin —
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
number the AUTHORITY row declares. A renamed constant, a moved table or a site
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

## And why the rows are not enough

Rows say where the number is SUPPOSED to be. For as long as that was all they
said, the number was also in 446 other files: a `dsl_version` bump touched 816
occurrences and **319 of the 489 files in that pull request changed nothing but
that one string**. So `verify` also SWEEPS the tree — see the block above
`GENERATED_JSON_ROOTS` for the rule it applies and the three shapes a version
literal may take — and `blast-radius` answers *what does a bump edit* with a
number a person can check.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import tomllib
from pathlib import Path

# --- how a row says where it is ---------------------------------------------
#
# Four kinds, because four shapes of file hold a version and a single regex
# over all of them would be the "one shared parse rule" written as a wildcard.
#
#   toml   — a key in a named table, compared as a VALUE (`tomllib`, so the
#            table is resolved rather than guessed at from indentation);
#   regex  — a Rust source line, matched with the version interpolated, or (with
#            `literal: False`) a DERIVATION with no version in it to interpolate;
#   lock   — a `Cargo.lock` `[[package]]` entry, which is two lines and cannot
#            be matched by one, and which cargo owns: it is listed so the reader
#            knows it moves, and named as cargo's so nobody hand-edits it;
#   present— a document a tool writes, where all this row asks is that the number
#            is in it. WHERE it may sit, and that no other version literal sits
#            beside it, belongs to the gate that writes it.

ROWS: dict[str, list[dict[str, object]]] = {
    # The DSL crate's version IS the `dsl_version` (ADR-0024), so every one of
    # these is a statement of the format's number. `shape` says what a bump does
    # to the site, and it is the vocabulary the tree sweep below judges by:
    #
    #   authority     the one place a person types the number
    #   derived       the site reads the authority; it holds no literal at all
    #   hand-edited   a person retypes it and a gate holds it equal
    #   cargo-written cargo owns the file; a bump is `cargo update -p <crate>`
    #   tool-written  a tool writes it; a bump is that command
    #
    # `sites` is how many times the file states the number. `None` means the
    # file's whole content is a tool's, so the count is not this gate's business.
    "dsl": [
        {
            "path": "crates/dsl/Cargo.toml",
            "label": "[package] version — THE AUTHORITY: the one place this number is typed",
            "kind": "toml",
            "table": ["package"],
            "key": "version",
            "shape": "authority",
            "sites": 1,
        },
        {
            "path": "crates/dsl/src/envelope.rs",
            "label": 'DSL_VERSION = env!("CARGO_PKG_VERSION") — derived, holds no literal',
            "kind": "regex",
            "pattern": r'pub\s+const\s+DSL_VERSION\s*:\s*&str\s*=\s*env!\("CARGO_PKG_VERSION"\)\s*;',
            "literal": False,
            "shape": "derived",
            "sites": 0,
        },
        {
            "path": "versions.toml",
            "label": "[engine] dsl_crate_version",
            "kind": "toml",
            "table": ["engine"],
            "key": "dsl_crate_version",
            "shape": "hand-edited",
            "sites": 2,  # with dsl_crate_req below, both in this one file
        },
        {
            "path": "versions.toml",
            "label": "[engine] dsl_crate_req (=<version>)",
            "kind": "toml",
            "table": ["engine"],
            "key": "dsl_crate_req",
            "prefix": "=",
            "shape": "hand-edited",
            "sites": 0,  # counted by the row above: one file, one census
        },
        {
            "path": "Cargo.toml",
            "label": "[workspace.dependencies] delvewright-dsl version (=<version>)",
            "kind": "toml",
            "table": ["workspace", "dependencies", "delvewright-dsl"],
            "key": "version",
            "prefix": "=",
            "shape": "hand-edited",
            "sites": 1,
        },
        {
            "path": "Cargo.lock",
            "label": "delvewright-dsl — cargo's, never edited: cargo update -p delvewright-dsl",
            "kind": "lock",
            "package": "delvewright-dsl",
            "shape": "cargo-written",
            "sites": None,
        },
        {
            "path": "prefabs/Cargo.lock",
            "label": "delvewright-dsl — cargo's, never edited: "
            "cargo update -p delvewright-dsl --manifest-path prefabs/Cargo.toml",
            "kind": "lock",
            "package": "delvewright-dsl",
            "shape": "cargo-written",
            "sites": None,
        },
        {
            "path": "docs/reference/compiler.md",
            "label": "the version header and the DW0102 row — written by "
            "`python3 tools/check-reference-versions.py --write`",
            "kind": "present",
            "shape": "tool-written",
            "sites": 2,
        },
        {
            "path": "crates/dsl/README.md",
            "label": "the crates.io front page's `Campaign format` claim — written by "
            "`python3 tools/check-reference-versions.py --write`",
            "kind": "present",
            "shape": "tool-written",
            "sites": 1,
        },
        {
            "path": "crates/delvec/README.md",
            "label": "the crates.io front page's `Campaign format` claim — written by "
            "`python3 tools/check-reference-versions.py --write`",
            "kind": "present",
            "shape": "tool-written",
            "sites": 1,
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
            "shape": "hand-edited",
            "sites": 1,
        },
        {
            "path": "Cargo.toml",
            "label": "[workspace.package] version (every member inherits it: version.workspace = true)",
            "kind": "toml",
            "table": ["workspace", "package"],
            "key": "version",
            "shape": "authority",
            "sites": 1,
        },
        {
            "path": "Cargo.lock",
            "label": "delvec — cargo's, never edited: cargo update -p delvec",
            "kind": "lock",
            "package": "delvec",
            "shape": "cargo-written",
            "sites": None,
        },
    ],
}

# --- the fourth shape, and the tree sweep that refuses it -------------------
#
# The rows above say where the number is SUPPOSED to be. They said nothing about
# anywhere else, and anywhere else is where it actually was: a `dsl_version`
# bump moved 446 files and 816 occurrences, and 319 of them changed nothing but
# that one string — Rust tests passing `"0.23.0"` to a helper that already took
# it as a parameter, and fixtures for a *different* diagnostic carrying an
# envelope only to get as far as their own subject.
#
# A version literal is legitimate in exactly three shapes, and a hand-typed
# literal that is none of them is the defect:
#
#   1. DERIVED       the site takes the value from the authority at compile or
#                    run time, so it holds no literal and a bump cannot reach it;
#   2. TOOL-WRITTEN  a tool writes the document (`delvec fmt` writes an
#                    envelope's `dsl_version`), so a bump is a regeneration, and
#                    a gate binds every committed document's value to the number;
#   3. A COUNTER-EXAMPLE on this allowlist, each entry carrying its reason.
#
# The sweep below reads every text file git tracks and asks which shape each
# carrier is. A file that is none of them is a finding, by name, with the
# question it has to answer. **A data document stating its own version is not a
# duplicated constant**: a campaign document declares the surface it was written
# against and the engine refuses a mismatch (ADR-0024), which is why the JSON
# rule below is structural — every occurrence must be the value of a
# `dsl_version` key, i.e. the document describing itself — and not a path list.

# Generated artifacts whose `dsl_version` values are NESTED (an emitted
# `manifest.json` recorded inside a baseline), so `delvec fmt` does not stamp
# them and a bump is the generator's own command. Prefix -> the command.
GENERATED_JSON_ROOTS: dict[str, str] = {
    "gallery/baseline/": "python3 tools/gallery-baseline.py --delvec <bin> --prefabs <dir> --write",
}

# Deliberate counter-examples: a file that states this exact number where the
# number IS the subject, so a bump must NOT move it. EMPTY ON PURPOSE, and the
# empty state is the design — the type specimen,
# `crates/dsl/fixtures/invalid/DW0102-bad-dsl-version.json`, states `9.9.9`
# precisely so that it is not this number and never has to move. An entry here
# is a claim that needs a reason written beside it; a stale one (naming a file
# that no longer states the number) is reported rather than left to rot into a
# licence to hardcode.
COUNTEREXAMPLES: dict[str, str] = {}

def _declared(root: Path) -> dict[str, str]:
    """The two numbers, each read from its own AUTHORITY row.

    Not from `versions.toml`: that file RESTATES the DSL number and is one of
    the sites this module checks. Reading the value being checked out of a site
    under check is how a gate agrees with itself. The authority is the row that
    says it is one, so this cannot drift from the table above.
    """
    out: dict[str, str] = {}
    for kind, rows in ROWS.items():
        auth = [r for r in rows if r.get("shape") == "authority"]
        if len(auth) != 1:
            raise SystemExit(
                f"version-sites: `{kind}` names {len(auth)} authority row(s); exactly one "
                "site may be the place a person types the number."
            )
        row = auth[0]
        doc: object = tomllib.loads((root / str(row["path"])).read_text(encoding="utf-8"))
        for key in list(row["table"]) + [row["key"]]:  # type: ignore[arg-type]
            doc = doc[key]  # type: ignore[index]
        out[kind] = str(doc)
    return out


def _resolve(root: Path, row: dict[str, object], version: str) -> str | None:
    """`None` when the row resolves; otherwise why it does not."""
    p = root / str(row["path"])
    if not p.is_file():
        return f"no such file: {row['path']}"
    text = p.read_text(encoding="utf-8")
    want = str(row.get("prefix", "")) + version

    if row["kind"] == "regex":
        # A row whose pattern is a DERIVATION carries no version to interpolate;
        # `literal: False` says so, and `.format` would then be reading `{`s that
        # belong to the regex.
        pat = str(row["pattern"])
        if row.get("literal", True):
            pat = pat.format(v=re.escape(version))
        if re.search(pat, text) is None:
            return f"nothing in {row['path']} matches /{pat}/ — the symbol the message names is not there"
        return None

    if row["kind"] == "present":
        # A document a tool writes: what is checked here is that the number is
        # actually in it. WHERE it may sit, and that no other version literal
        # sits beside it, is `tools/check-reference-versions.py`'s, which binds
        # each claim by equality in both directions and writes them on --write.
        if want not in text:
            return (
                f"{row['path']} does not state `{want}` at all — it is written by "
                "`python3 tools/check-reference-versions.py --write`; run it"
            )
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



# ---------------------------------------------------------------- the sweep --


def _tracked_text(root: Path) -> list[str]:
    """Every file git tracks, in git's order. The population is DERIVED.

    Using git rather than a filesystem walk is what makes the exclusions
    properties instead of names: `target/`, `node_modules/` and every build
    product are absent because they are not tracked, and `campaigns/` contributes
    one symlink entry rather than a second repository's contents.
    """
    r = subprocess.run(
        ["git", "-C", str(root), "ls-files", "-z"],
        capture_output=True,
        text=True,
        check=False,
    )
    if r.returncode != 0:
        raise SystemExit(f"version-sites: `git ls-files` exited {r.returncode}: {r.stderr.strip()}")
    return sorted(p for p in r.stdout.split("\0") if p)


def _dsl_version_values(node: object, version: str) -> int:
    """How many times `version` appears as the value of a `dsl_version` key.

    Structural, not textual: this is the document DESCRIBING ITSELF, which
    ADR-0024 requires and which is the whole reason a campaign document may
    state a number. Anything else in the same file — the number inside some
    other key, or inside prose — is not counted here and so is not explained.
    """
    n = 0
    if isinstance(node, dict):
        for k, v in node.items():
            if k == "dsl_version" and v == version:
                n += 1
            else:
                n += _dsl_version_values(v, version)
    elif isinstance(node, list):
        for v in node:
            n += _dsl_version_values(v, version)
    return n


def _classify(root: Path, rel: str, text: str, version: str, by_path: dict[str, list[dict]]):
    """`(shape, how a bump reaches it)` for a carrier, or `None` if it is a finding."""
    occurrences = text.count(version)

    rows = by_path.get(rel)
    if rows:
        shapes = {str(r["shape"]) for r in rows}
        expect = [r["sites"] for r in rows]
        if None in expect:
            return sorted(shapes)[0], "cargo writes this file"
        total = sum(int(s) for s in expect)  # type: ignore[arg-type]
        if occurrences != total:
            return None, (
                f"states the number {occurrences} time(s); its row(s) account for "
                f"{total}. A site nobody wrote down is a site a bump has to find by "
                f"grep — add a row in {__file__} saying which shape it is, or remove it"
            )
        return sorted(shapes)[0], ", ".join(str(r["label"]) for r in rows)

    if rel.endswith(".json"):
        try:
            doc = json.loads(text)
        except json.JSONDecodeError as exc:
            return None, f"states the number but is not valid JSON ({exc})"
        declared = _dsl_version_values(doc, version)
        if declared == occurrences:
            for prefix, cmd in GENERATED_JSON_ROOTS.items():
                if rel.startswith(prefix):
                    return "tool-written", cmd
            if isinstance(doc, dict) and doc.get("dsl_version") == version and occurrences == 1:
                return "tool-written", "delvec fmt <path>"
            return None, (
                f"states the number {occurrences} time(s), all as `dsl_version` values, "
                "but not as the ONE top-level envelope key `delvec fmt` writes and not "
                f"under a generated root ({', '.join(GENERATED_JSON_ROOTS) or 'none'}). "
                "Name the command that writes it"
            )
        return None, (
            f"states the number {occurrences} time(s) and only {declared} of them is a "
            "`dsl_version` value. A document may declare the surface it was written "
            "against; a number sitting anywhere else in it was typed by a person"
        )

    if rel in COUNTEREXAMPLES:
        return "counter-example", COUNTEREXAMPLES[rel]

    return None, (
        "states the number and is none of the three legitimate shapes: it does not "
        "derive it, no tool writes it, and it is not an allowlisted counter-example. "
        "Take the value from the authority, or say here which shape it is and why"
    )


def sweep(root: Path, version: str) -> tuple[int, int, dict[str, list[str]], list[str]]:
    """Every tracked file that states `version`, sorted into shapes.

    Returns (population, carriers, shape -> files, findings).
    """
    population = _tracked_text(root)
    if not population:
        raise SystemExit(
            "version-sites: git tracks NO files at all. This sweep examined nothing, "
            "which is a vacuous pass rather than a pass."
        )
    by_path: dict[str, list[dict]] = {}
    for row in ROWS["dsl"]:
        by_path.setdefault(str(row["path"]), []).append(row)

    shapes: dict[str, list[str]] = {}
    findings: list[str] = []
    carriers = 0
    for rel in population:
        p = root / rel
        if p.is_symlink() or not p.is_file():
            continue
        try:
            text = p.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue  # a binary blob cannot state a version a person typed
        if version not in text:
            continue
        carriers += 1
        shape, why = _classify(root, rel, text, version, by_path)
        if shape is None:
            findings.append(f"  {rel}: {why}")
        else:
            shapes.setdefault(f"{shape} — {why}" if shape == "counter-example" else shape, []).append(rel)

    for rel in sorted(COUNTEREXAMPLES):
        if rel not in population:
            findings.append(
                f"  COUNTEREXAMPLES names {rel!r}, which git does not track — remove it "
                "rather than letting it rot into a licence to hardcode"
            )
        elif version not in (root / rel).read_text(encoding="utf-8"):
            findings.append(
                f"  COUNTEREXAMPLES names {rel!r}, which no longer states `{version}` — "
                "the entry measures nothing"
            )

    if carriers == 0:
        raise SystemExit(
            f"version-sites: no tracked file states `{version}` at all — not even the "
            "authority. The sweep examined the whole tree and matched nothing, which is "
            "the measurement failing, not a clean tree."
        )
    return len(population), carriers, shapes, findings


HAND = ("authority", "hand-edited")


def blast_radius(root: Path) -> tuple[list[str], dict[str, list[str]]]:
    """The files a person edits to move the number, and everything a tool rewrites."""
    version = _declared(root)["dsl"]
    _, _, shapes, _ = sweep(root, version)
    hand = sorted({f for s in HAND for f in shapes.get(s, [])})
    return hand, shapes


def advise(kind: str, version: str) -> list[str]:
    """The message's own lines. The count comes from the list, never from prose."""
    rows = ROWS[kind]
    width = max(len(str(r["path"])) for r in rows)
    out = [f"    {str(r['path']):<{width}}  {r['label']}" for r in rows]
    return out


def verify(root: Path) -> int:
    """Every row resolved against the tree, then the whole tree swept. Exit status."""
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

    population, carriers, shapes, sweep_findings = sweep(root, declared["dsl"])
    if sweep_findings:
        print(
            f"version-sites: a file states the `dsl_version` `{declared['dsl']}` and is "
            "none of the three shapes a version literal may take.",
            file=sys.stderr,
        )
        print(
            "  A version's authoritative value lives in exactly one place; every other "
            "site DERIVES it, is WRITTEN by a tool a bump re-runs, or is an allowlisted "
            "counter-example. One bump edits one file.",
            file=sys.stderr,
        )
        for f in sweep_findings:
            print(f, file=sys.stderr)
        print(
            f"  The authority is {ROWS['dsl'][0]['path']}. Rows and the allowlist live "
            f"in {__file__}.",
            file=sys.stderr,
        )
        return 1

    counts = ", ".join(f"{k} {len(v)}" for k, v in ROWS.items())
    print(
        f"version-sites: {checked} row(s) resolved against the tree ({counts}); "
        f"dsl {declared['dsl']}, engine {declared['engine']}"
    )
    hand = sorted({f for s in HAND for f in shapes.get(s, [])})
    print(
        f"version-sites sweep: {carriers} of {population} tracked file(s) state "
        f"`{declared['dsl']}`; every one is accounted:"
    )
    for shape in sorted(shapes):
        files = sorted(shapes[shape])
        shown = ", ".join(files[:4]) + (f", … (+{len(files) - 4})" if len(files) > 4 else "")
        print(f"    {len(files):>4}  {shape:<14}  {shown}")
    print(
        f"  a bump edits {len(hand)} file(s) BY HAND — {', '.join(hand)} — and "
        f"{carriers - len(hand)} more are rewritten by a tool named above."
    )
    return 0


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("command", choices=["verify", "advise", "blast-radius"])
    ap.add_argument("--root", default=".", help="repository root")
    ap.add_argument("--kind", choices=sorted(ROWS), help="advise: which number")
    ap.add_argument("--version", help="advise: the number being moved away from")
    ap.add_argument("--count", action="store_true", help="advise: print the row count only")
    a = ap.parse_args(argv)
    root = Path(a.root).resolve()
    if a.command == "verify":
        return verify(root)
    if a.command == "blast-radius":
        hand, shapes = blast_radius(root)
        if a.count:
            print(len(hand))
            return 0
        for f in hand:
            print(f)
        return 0
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
