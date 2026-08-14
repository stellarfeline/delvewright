#!/usr/bin/env python3
"""One version number names exactly one engine surface, once this branch merges.

## The defect this exists to end

A version ledger is picked exactly the way a spec number is: an author LISTS the
versions that exist and writes the next one. Two branches that each add "the next
version" for a DIFFERENT surface therefore both write the same number, and — like
two new spec files — the collision exists only in the UNION of the two trees.

It happened. One branch introduced `crates/grammar/src/version.rs` with
`MIRROR_SINCE = "1.1.0"` (a frame's direction); another independently introduced
the same file with `CONTRACT_SINCE = "1.1.0"` (the spatial contract). Both were
green. The two files conflict textually, so a merge WOULD have raised a marker —
and the tempting resolution is to union the two `*_SINCE` constants under one
`1.1.0`, which is the actual hazard. Ship that and an engine calling itself
`1.1.0` accepts a document declaring `1.1.0` that uses the other construct and
silently drops it: the exact silent-wrongness the version fence was built to
prevent (ADR-0018 §7), reintroduced by the fence's own numbering.

A textual conflict is not the guard. It is raised, resolved by hand, and the
resolution is where the defect enters.

## What "uniqueness" means here, precisely

Not "no duplicate inside the working tree" — every colliding branch is internally
consistent, which is exactly the vacuous shape this repo keeps finding
(CLAUDE.md). The question this gate answers is **"will every version number still
name exactly one surface after this branch merges into `--base`"**
(`origin/main` by default).

A version's **surface** has to be named by something a machine can compare across
two trees. The name used here is the ledger's own **fence anchor** — the
identifier that resolves to that version — because it is code, not prose, and
therefore does not drift with wording:

- `grammar-program`: the `*_SINCE` constants in `crates/grammar/src/version.rs`,
  plus each row of `RESERVED_VERSIONS`, which names the anchor a sibling change
  will define. A reservation is the forward declaration made checkable: reserve
  `1.1.0` for `MIRROR_SINCE`, and the day that change merges the union for
  `1.1.0` is `{MIRROR_SINCE}` and stays green — while a reservation for a
  DIFFERENT anchor
  than the one that landed is a union of two and reds.
- `dsl-campaign`: the `is_vNN` predicates in `crates/dsl/src/envelope.rs`,
  resolved through `ordinal()`'s match arms.

Five rules, run over the union of the checkout and `--base`:

1. **One number, one surface.** More than one distinct anchor claiming a version,
   across the two trees, is the collision above.
2. **One surface, one number.** An anchor at one version in the checkout and a
   different version at `--base` moved a construct's fence after it shipped,
   which changes what an already-written document means.
3. **Every number is claimed.** A version in the ledger past the founding one
   with no anchor claiming it names nothing — and a number that names nothing is
   a number a second change can take. This is the rule that makes rule 1 hard to
   route around: skipping a number does not free you from declaring it.
4. **The ledger is append-only.** `--base`'s version list must be a prefix of the
   checkout's: versions are added at the end, never renumbered or inserted.
5. **A reservation is deleted by the change that lands its surface.** A reserved
   version whose anchor is also DEFINED in the same tree is refusing a version
   that engine can now honour.

## Ledgers covered, and why

One row per ledger in `LEDGERS`. The object class is *a version ledger*, not one
crate: `dsl_version` has per-stage fences and the identical exposure,
so a gate that only knew about the grammar crate would be the bespoke-field
defect one layer out. Adding a third ledger is one row here, not a new script.

## What this CANNOT catch — read this before trusting a green run

Two limits, and the second is specific to one ledger.

- **Two open PRs colliding with each other but not with `--base`.** This gate
  diffs the checkout against `--base` AS OF THE MOMENT IT RUNS, and has no
  visibility into any branch that has not merged into `--base`; nothing short of
  the GitHub API would, and this repo's CI token deliberately stays
  `contents: read` (the stance `check-required-contexts.py` and
  `check-numbered-doc-uniqueness.py` both take, for the same reason — a gate that
  needs a privileged token is a gate that quietly stops running). So a
  collision like the one above is invisible to both CI runs while both are open.
  **What this gate guarantees is the half that is catchable: once the first of
  them merges, the second goes RED against `origin/main`** — the resolution that
  unions two surfaces under one number cannot reach `main` past a re-run. That
  narrows the window from "forever, until a human reads two diffs side by side"
  to "until the second PR's next required re-run"; it does not close it to zero,
  because a stale green can still merge unless branch protection requires a
  branch be up to date first (a GitHub setting this script cannot read for the
  same reason it cannot call the API).
- **`dsl-campaign`'s anchors are self-naming, so rule 1 is structurally blind
  there.** `0.11.0` forces the anchor name `is_v11` in any branch that adds it,
  so two branches adding `0.11.0` for different surfaces produce the SAME anchor
  and rule 1 sees one claim, correctly, and says nothing. Rules 2, 3, 4 and 5 all
  bind on that ledger and are real; rule 1 is not. Closing it needs the ledger to
  carry a per-version surface LABEL that is not derivable from the number — a
  change to `crates/dsl/src/envelope.rs`, deliberately not made here, and the
  reason this limitation is written down rather than left to be rediscovered.
  `crates/grammar/src/version.rs` does not have the problem: `MIRROR_SINCE` and
  `CONTRACT_SINCE` are different names for the same number, which is what rule 1
  reads.

## Binding count

Every run prints, per ledger: versions in the checkout and at `--base`, distinct
anchors on each side, reservations, and findings. A ledger with **zero anchors on
BOTH sides** is a FAIL — the file moved, was renamed, or the extraction pattern no
longer matches (CLAUDE.md: a green gate that binds to nothing is VACUOUS). A
ledger file that parses to zero versions, or to zero anchors while naming two or
more versions, exits **2** with the pattern to fix named: fix the pattern, never
loosen the check.

## Fetching `--base`

This script performs no network I/O. When the ref is missing it says so and prints
how to get it, computed from THIS repository by `tools/lib/gitbase.py` — a plain
fetch in a full clone, a `--depth=1` fetch in an already-shallow one. The two are
not interchangeable, which is the whole reason that decision is not made here:
`--depth=1` in a full clone shallows it, and a shallow clone answers ancestry
questions with confident wrong numbers rather than with an error.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent / "lib"))

from gitbase import BaseUnresolved, resolve_base  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent

# `pub const <NAME>: &[&str] = &[ "a", "b" ];` — the ordered version list every
# ledger exposes under its own constant name.
LIST_RE_TEMPLATE = r"pub const {const}: &\[&str\] = &\[(.*?)\];"
# `pub const RESERVED_VERSIONS: &[(&str, &str)] = &[("1.1.0", "MIRROR_SINCE")];`
RESERVED_LIST_RE = re.compile(
    r"pub const RESERVED_VERSIONS: &\[\(&str, &str\)\] = &\[(.*?)\];", re.DOTALL
)
RESERVED_ROW_RE = re.compile(r'\(\s*"([^"]+)"\s*,\s*"([^"]+)"\s*\)')
QUOTED_RE = re.compile(r'"([^"]+)"')

# grammar: `pub const CONTRACT_SINCE: &str = "1.2.0";`
SINCE_CONST_RE = re.compile(r'pub const (\w+_SINCE): &str = "([^"]+)";')
# dsl: `"0.10.0" => 10,` inside `ordinal`
DSL_ORDINAL_ARM_RE = re.compile(r'"(\d+\.\d+\.\d+)"\s*=>\s*(\d+)\s*,')
# dsl: `pub fn is_v10(version: &str) -> bool { ordinal(version) >= 10 }`
DSL_PREDICATE_RE = re.compile(
    r"pub fn (is_v\d+)\(version: &str\) -> bool \{\s*ordinal\(version\) >= (\d+)\s*\}"
)


class ShapeDrift(Exception):
    """A ledger file no longer matches the pattern this gate reads it with."""


def reservations(source: str) -> dict[str, str]:
    """version -> the anchor name a sibling change will define for it."""
    block = RESERVED_LIST_RE.search(source)
    if not block:
        return {}
    return dict(RESERVED_ROW_RE.findall(block.group(1)))


def grammar_claims(source: str) -> dict[str, set[str]]:
    """version -> anchors claiming it, for `crates/grammar/src/version.rs`.

    A reservation claims its version under the name of the constant that will
    carry it, so a forward declaration and the change that fulfils it agree
    rather than collide.
    """
    claims: dict[str, set[str]] = defaultdict(set)
    for name, version in SINCE_CONST_RE.findall(source):
        claims[version].add(name)
    for version, anchor in reservations(source).items():
        claims[version].add(anchor)
    return claims


def dsl_claims(source: str) -> dict[str, set[str]]:
    """version -> anchors claiming it, for `crates/dsl/src/envelope.rs`.

    Two hops, because the predicate names an ordinal rather than a version:
    `ordinal()`'s arms give version -> N, the predicates give N -> `is_vNN`.
    """
    ordinals = {v: int(n) for v, n in DSL_ORDINAL_ARM_RE.findall(source)}
    by_ordinal = {int(n): name for name, n in DSL_PREDICATE_RE.findall(source)}
    claims: dict[str, set[str]] = defaultdict(set)
    for version, n in ordinals.items():
        if n in by_ordinal:
            claims[version].add(by_ordinal[n])
    return claims


# One row per version ledger in the repo. `claims` names, per version, the
# identifier(s) that resolve to it — the machine-comparable stand-in for "the
# surface this number means".
LEDGERS = [
    {
        "name": "grammar-program",
        "path": "crates/grammar/src/version.rs",
        "list_const": "SUPPORTED_PROGRAM_VERSIONS",
        "claims": grammar_claims,
        "claim_pattern": SINCE_CONST_RE.pattern,
        "reservations": reservations,
    },
    {
        "name": "dsl-campaign",
        "path": "crates/dsl/src/envelope.rs",
        "list_const": "SUPPORTED_DSL_VERSIONS",
        "claims": dsl_claims,
        "claim_pattern": DSL_PREDICATE_RE.pattern,
        "reservations": lambda _source: {},
    },
]


def versions_of(ledger: dict, source: str) -> list[str]:
    pattern = LIST_RE_TEMPLATE.format(const=ledger["list_const"])
    block = re.search(pattern, source, re.DOTALL)
    if not block:
        raise ShapeDrift(
            f"{ledger['name']}: {ledger['path']} no longer matches {pattern!r}, so "
            f"the version list could not be read at all. Fix the pattern in "
            f"tools/check-version-ledger-uniqueness.py — never loosen the check."
        )
    return QUOTED_RE.findall(block.group(1))


def read_ledger(ledger: dict, source: str, where: str) -> tuple[list[str], dict[str, set[str]]]:
    versions = versions_of(ledger, source)
    if not versions:
        raise ShapeDrift(
            f"{ledger['name']}: {ledger['list_const']} in {ledger['path']} parsed to "
            f"ZERO versions at {where}. A ledger with no versions is a parse failure, "
            f"not a ledger."
        )
    claims = ledger["claims"](source)
    if len(versions) >= 2 and not claims:
        raise ShapeDrift(
            f"{ledger['name']}: {ledger['path']} at {where} names {len(versions)} "
            f"versions and ZERO of them could be traced to a fence anchor via "
            f"{ledger['claim_pattern']!r}. The extraction has drifted; every version "
            f"would then read as unclaimed. Fix the pattern — never loosen the check."
        )
    return versions, claims


def base_source(base: str, path: str) -> str | None:
    """The file's content at `base`, or None if it does not exist there."""
    result = subprocess.run(
        ["git", "show", f"{base}:{path}"], cwd=ROOT, capture_output=True, text=True
    )
    return result.stdout if result.returncode == 0 else None


def check_ledger(ledger: dict, base: str) -> tuple[list[str], str]:
    """Findings for one ledger, plus its binding-count summary line."""
    name = ledger["name"]
    path = ledger["path"]
    findings: list[str] = []

    local_path = ROOT / path
    local_source = local_path.read_text(encoding="utf-8") if local_path.is_file() else None
    base_src = base_source(base, path)

    if local_source is None and base_src is None:
        return (
            [
                f"{name}: {path} exists neither in this checkout nor at {base} — the "
                f"ledger moved or was renamed, and this gate examined nothing. A check "
                f"that binds to nothing is not a pass."
            ],
            f"{name}: absent on both sides (VACUOUS)",
        )
    if local_source is None:
        return (
            [
                f"{name}: {path} exists at {base} but not in this checkout. A version "
                f"ledger cannot be deleted — every number in it names a surface some "
                f"already-written document declares."
            ],
            f"{name}: deleted in this checkout",
        )

    local_versions, local_claims = read_ledger(ledger, local_source, "this checkout")
    if base_src is None:
        base_versions, base_claims = [], {}
    else:
        base_versions, base_claims = read_ledger(ledger, base_src, base)

    local_reserved = ledger["reservations"](local_source)

    # Rule 1 — one number, one surface, across the union of the two trees.
    collisions = 0
    for version in sorted(set(local_claims) | set(base_claims)):
        here = set(local_claims.get(version, set()))
        there = set(base_claims.get(version, set()))
        both = here | there
        if len(both) <= 1:
            continue
        collisions += 1
        lines = [
            f"{name}: version {version} is claimed by {len(both)} different surfaces — "
            f"it will NOT name exactly one thing once this branch merges into {base}:"
        ]
        for anchor in sorted(both):
            where = []
            if anchor in here:
                where.append("this branch")
            if anchor in there:
                where.append(base)
            lines.append(f"    {anchor}  ({' + '.join(where)})")
        lines.append(
            f"    Fix: move ONE of these surfaces to the next version free on BOTH "
            f"sides, and leave the number it vacates named by the surface that keeps "
            f"it. Unioning them under one number is the defect: an engine at that "
            f"version would accept a document declaring it and silently drop the "
            f"surface it does not implement."
        )
        findings.append("\n".join(lines))

    # Rule 2 — one surface, one number.
    moved = 0
    for anchor in sorted({a for s in local_claims.values() for a in s}):
        here_versions = {v for v, s in local_claims.items() if anchor in s}
        there_versions = {v for v, s in base_claims.items() if anchor in s}
        if there_versions and here_versions != there_versions:
            moved += 1
            findings.append(
                f"{name}: {anchor} is at {sorted(here_versions)} on this branch and "
                f"{sorted(there_versions)} at {base}. A construct's fence version may "
                f"not move once it is on {base} — every document already written "
                f"against it would change meaning."
            )

    # Rule 3 — every number past the founding one is claimed by something.
    unclaimed = 0
    for version in local_versions[1:]:
        if not local_claims.get(version):
            unclaimed += 1
            findings.append(
                f"{name}: version {version} is in the ledger and no fence anchor "
                f"claims it, so it names nothing — and a number that names nothing is "
                f"a number a second change can take. Give it the anchor that "
                f"introduces its surface, or reserve it for the anchor that will."
            )

    # Rule 4 — append-only.
    if base_versions and local_versions[: len(base_versions)] != base_versions:
        findings.append(
            f"{name}: the ledger at {base} is {base_versions} and this branch has "
            f"{local_versions} — a version was renumbered, reordered or inserted "
            f"rather than appended. Every number is a promise to documents already "
            f"written against it; only the end of the list is free."
        )

    # Rule 5 — a reservation is deleted by the change that lands its surface.
    defined_here = {n for n, _ in SINCE_CONST_RE.findall(local_source)}
    for version, anchor in sorted(local_reserved.items()):
        if anchor in defined_here:
            findings.append(
                f"{name}: version {version} is reserved for {anchor}, and {anchor} is "
                f"defined in this same tree. The surface has landed, so the "
                f"reservation now refuses a version this engine can honour — delete "
                f"the RESERVED_VERSIONS row."
            )

    n_local_anchors = len({a for s in local_claims.values() for a in s})
    n_base_anchors = len({a for s in base_claims.values() for a in s})
    if n_local_anchors == 0 and n_base_anchors == 0:
        findings.append(
            f"{name}: zero fence anchors on BOTH this checkout and {base} — this "
            f"ledger's uniqueness rules examined no surface at all. A check that binds "
            f"to nothing is not a pass."
        )
        summary = f"{name}: 0 anchors here, 0 at {base} (VACUOUS)"
    else:
        summary = (
            f"{name}: {len(local_versions)} versions here ({len(base_versions)} at "
            f"{base}), {n_local_anchors} anchors here ({n_base_anchors} at {base}), "
            f"{len(local_reserved)} reserved, {collisions} collision(s)"
        )
    return findings, summary


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Every version number names exactly one surface after this branch merges."
    )
    parser.add_argument(
        "--base",
        default="origin/main",
        help="git ref to diff the checkout against (default: origin/main). "
        "Must already be fetched — this script performs no network I/O itself.",
    )
    args = parser.parse_args()
    base = args.base

    try:
        base_sha = resolve_base(ROOT, base, "check-version-ledger-uniqueness")
    except BaseUnresolved as unresolved:
        print(unresolved.message, file=sys.stderr)
        return 1

    findings: list[str] = []
    summaries: list[str] = []
    for ledger in LEDGERS:
        try:
            ledger_findings, summary = check_ledger(ledger, base)
        except ShapeDrift as drift:
            print(f"check-version-ledger-uniqueness: {drift}", file=sys.stderr)
            return 2
        findings.extend(ledger_findings)
        summaries.append(summary)

    if findings:
        print(
            f"check-version-ledger-uniqueness: {len(findings)} finding(s) against "
            f"{base} @ {base_sha[:12]} — {'; '.join(summaries)}\n",
            file=sys.stderr,
        )
        for finding in findings:
            print(f"  {finding}\n", file=sys.stderr)
        return 1

    print(
        f"check-version-ledger-uniqueness: OK — {'; '.join(summaries)}, diffed against "
        f"{base} @ {base_sha[:12]}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
