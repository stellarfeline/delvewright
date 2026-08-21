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
therefore does not drift with wording. Anchors come in two kinds, and the
difference between them is the whole of rule 6:

- **Named** anchors are written by hand and are not derivable from the number:
  the `*_SINCE` constants in `crates/grammar/src/version.rs` and in
  `crates/dsl/src/envelope.rs`, plus each row of a ledger's reservation list
  (`RESERVED_VERSIONS`, `RESERVED_DSL_VERSIONS`), which names the anchor a
  sibling change will define. A reservation is the forward declaration made
  checkable: reserve `1.1.0` for `MIRROR_SINCE`, and the day that change merges
  the union for `1.1.0` is `{MIRROR_SINCE}` and stays green — while a
  reservation for a DIFFERENT anchor than the one that landed is a union of two
  and reds.
- **Derived** anchors are computed from the number itself: `dsl-campaign`'s
  `is_vNN` predicates in `crates/dsl/src/envelope.rs`, resolved through
  `ordinal()`'s match arms. `0.11.0` forces the name `is_v11` in every branch
  that adds it, so a derived anchor can never distinguish two branches — it is a
  claim that the number is *used*, never a name for what it means.

A version's claim is its **named** anchors when it has any, and its derived ones
otherwise. Precedence rather than union, because the two coexist for one version
the moment a reserved surface lands (`OPEN_WAY_SINCE` and `is_v12` at `0.12.0`),
and that is one surface with two spellings, not two surfaces.

## A withdrawn hold is not a competing claim

Rule 1 used to take the plain UNION of the two trees' claims while its own
message described the state *after* the merge, and those are not the same set:
a union cannot subtract, so a base-side row this branch REMOVES reads exactly
like one it never saw. The ordinary reservation lifecycle hid it, because the
change that lands a reserved surface defines the constant the row already named
and the two dedup — a **cancellation** defines a different one, and the branch
that cancelled `("0.13.0", "HORIZON_LIBRARY_SINCE")` and took the number for
`LAYOUT_GRAPH_SINCE` was refused for a collision that does not exist after it
merges.

The state is a three-way question, so it needs the third tree: the **merge base**
of this checkout's `HEAD` and `--base`, which is local (`git merge-base`, no
network) and is the only thing that can separate *removed* from *never seen*. A
claim at `--base` that this branch does not carry was withdrawn only if the
branch INHERITED it; one that appeared at `--base` after the fork point survives
the merge and is a competitor as before.

Two properties keep this from being a weakening, and the second is the one that
matters:

- **Only a HOLD can be cancelled.** The subtraction is offered to reservation
  rows alone, and only while neither tree DEFINES the anchor. A landed `*_SINCE`
  and a derived predicate are not withdrawable at any distance — deleting one is
  a deletion of shipped surface, and it stays rule 1's finding. This is the
  property a real collision cannot supply: a competitor's base-side claim is
  either implemented (so unwithdrawable) or arrived after the fork (so not
  inherited).
- **A withdrawal re-allocates the number, so rule 6 follows it.** Cancelling a
  hold frees the number and this branch is taking it, which is the allocation
  rule 6 exists for — so a withdrawn version must carry a hand-written name in
  this checkout exactly as an added one must.

When no merge base exists — an unborn `HEAD`, or a shallow checkout, where git
answers ancestry with nothing rather than with an error — nothing is withdrawn
and rule 1 is the union it always was. That is the fail-closed direction: the
gate refuses more, never less, and says in its binding line that it could not
compute the difference.

Six rules, run over the claims that stand once this branch merges — the union of
the checkout and `--base`, less what either side withdrew:

1. **One number, one surface.** More than one distinct anchor claiming a version,
   once the merge has been taken into account, is the collision above.
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
6. **A number this branch ADDS carries a hand-written name.** A version present
   in the checkout and absent at `--base` whose only claim is a derived anchor
   has not been named at all: a second branch adding the same number computes
   the same anchor, and rule 1 reads one claim where there are two. Satisfied by
   a `*_SINCE` constant when the surface lands in this same change, or by a
   reservation row when a sibling change will land it. This is the rule that
   makes rule 1 bind on a ledger whose implemented anchors are self-naming; it
   examines added versions and versions whose hold this branch withdrew, because
   those are the two ways a number gets allocated — a number already at `--base`
   and still held there is not being allocated, and re-naming one is the rename
   rule 2 refuses.

## Ledgers covered, and why

One row per ledger in `LEDGERS`. The object class is *a version ledger*, not one
crate: `dsl_version` has per-stage fences and the identical exposure,
so a gate that only knew about the grammar crate would be the bespoke-field
defect one layer out. Adding a third ledger is one row here, not a new script.

## What this CANNOT catch — read this before trusting a green run

Two limits. Neither is ledger-specific: rule 6 closed the one that was.

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
- **A branch that hangs a NEW surface off an EXISTING version's fence.** Nothing
  in a version ledger says what a surface *is*, so a change that adds a field
  behind `is_v09` — or behind `CONTRACT_SINCE` — allocates no number and this
  gate sees nothing. That is the same on both ledgers and is out of reach from
  the ledger file alone; what catches it is the version-adoption discipline that
  says a new surface takes a new number. Rule 6 makes the number it takes
  nameable; it cannot make an author take one.

  The blind spot this replaces was narrower and real: `dsl-campaign`'s
  implemented anchors are self-naming, so two branches adding `0.12.0` for
  different surfaces produced the SAME anchor `is_v12` and rule 1 read one claim.
  It was measured, not theorised — an `open-way` `0.12.0` in the checkout against
  a horizon-library `0.12.0` at base ran green with `0 collision(s)`. Rule 6 is
  what closes it: both branches must now hand-name the number they add, and two
  hand-written names for one number are what rule 1 has always been able to see.

## Binding count

Every run prints, per ledger: versions in the checkout and at `--base`, distinct
anchors on each side, reservations, collisions, how many ADDED versions rule
6 examined — zero added is the ordinary state of a branch that touches no ledger,
and it is printed rather than left to be assumed — and finally the withdrawals,
each named with its version, its anchor and the side that cancelled it, so a hold
cannot be dropped without appearing in the output. Zero withdrawals is stated
against the number of holds INHERITED, so it reads as a measurement rather than
as nothing to measure; where no merge base was available the line says the
difference could not be computed at all. A ledger with **zero anchors on
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
#
# `=\s*&\[` rather than `= &\[`, because rustfmt breaks the line after the `=`
# once the list outgrows one line, and it did the first time a sixth version was
# added. This accepts the SAME construct across a line break; it does not accept
# any other shape, and a genuine drift still raises `ShapeDrift`.
LIST_RE_TEMPLATE = r"pub const {const}: &\[&str\] =\s*&\[(.*?)\];"
# `pub const RESERVED_VERSIONS: &[(&str, &str)] = &[("1.1.0", "MIRROR_SINCE")];`
# The constant's NAME is per-ledger (`RESERVED_VERSIONS`, `RESERVED_DSL_VERSIONS`)
# and the shape is not, which is why the name is the parameter and the row
# grammar is shared.
RESERVED_LIST_RE_TEMPLATE = r"pub const {const}: &\[\(&str, &str\)\] = &\[(.*?)\];"
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


def reservations_of(const: str, source: str) -> dict[str, str]:
    """version -> the anchor name a sibling change will define for it."""
    block = re.search(RESERVED_LIST_RE_TEMPLATE.format(const=const), source, re.DOTALL)
    if not block:
        return {}
    return dict(RESERVED_ROW_RE.findall(block.group(1)))


def since_constants(source: str) -> dict[str, set[str]]:
    """version -> the hand-written `*_SINCE` constants defined at it.

    The same extraction on both ledgers: a fence constant is a name an author
    chose, so two authors choosing one number disagree visibly. `dsl-campaign`
    has none today and gains one each time a reserved surface lands.
    """
    named: dict[str, set[str]] = defaultdict(set)
    for name, version in SINCE_CONST_RE.findall(source):
        named[version].add(name)
    return named


def no_derived_anchors(_source: str) -> dict[str, set[str]]:
    """`grammar-program` has no anchor computable from a version number."""
    return {}


def dsl_predicates(source: str) -> dict[str, set[str]]:
    """version -> the `is_vNN` predicate open at it, for the campaign ledger.

    Two hops, because the predicate names an ordinal rather than a version:
    `ordinal()`'s arms give version -> N, the predicates give N -> `is_vNN`.

    These are DERIVED anchors: the name follows from the number, so they prove a
    number is in use and can never say what it means. Rule 6 is what keeps a
    newly allocated number from resting on one.
    """
    ordinals = {v: int(n) for v, n in DSL_ORDINAL_ARM_RE.findall(source)}
    by_ordinal = {int(n): name for name, n in DSL_PREDICATE_RE.findall(source)}
    derived: dict[str, set[str]] = defaultdict(set)
    for version, n in ordinals.items():
        if n in by_ordinal:
            derived[version].add(by_ordinal[n])
    return derived


def claims_of(
    named: dict[str, set[str]],
    derived: dict[str, set[str]],
    reserved: dict[str, str],
) -> dict[str, set[str]]:
    """version -> the anchors that stand for its surface.

    Named anchors win outright where a version has any: a landed surface spells
    its version twice (`OPEN_WAY_SINCE` and `is_v12`) and that is one surface,
    not two. Where a version has no name, its derived anchor is all there is —
    enough for rule 3 to see the number is used, never enough for rule 1.
    """
    claims: dict[str, set[str]] = defaultdict(set)
    for version in set(named) | set(derived) | set(reserved):
        here = set(named.get(version, set()))
        if version in reserved:
            here.add(reserved[version])
        claims[version] = here or set(derived.get(version, set()))
    return {v: s for v, s in claims.items() if s}


# One row per version ledger in the repo. `named` and `derived` name, per
# version, the identifier(s) that resolve to it — the machine-comparable
# stand-in for "the surface this number means" — split by whether the identifier
# was chosen by an author or computed from the number.
LEDGERS = [
    {
        "name": "grammar-program",
        "path": "crates/grammar/src/version.rs",
        "list_const": "SUPPORTED_PROGRAM_VERSIONS",
        "reserved_const": "RESERVED_VERSIONS",
        "named": since_constants,
        "derived": no_derived_anchors,
        "claim_pattern": SINCE_CONST_RE.pattern,
    },
    {
        "name": "dsl-campaign",
        "path": "crates/dsl/src/envelope.rs",
        "list_const": "SUPPORTED_DSL_VERSIONS",
        "reserved_const": "RESERVED_DSL_VERSIONS",
        "named": since_constants,
        "derived": dsl_predicates,
        "claim_pattern": f"{SINCE_CONST_RE.pattern} / {DSL_PREDICATE_RE.pattern}",
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


class LedgerSide:
    """One tree's reading of one ledger: its list, and its anchors by kind."""

    def __init__(
        self,
        versions: list[str],
        named: dict[str, set[str]],
        derived: dict[str, set[str]],
        reserved: dict[str, str],
    ) -> None:
        self.versions = versions
        self.named = named
        self.derived = derived
        self.reserved = reserved
        self.claims = claims_of(named, derived, reserved)

    @property
    def anchors(self) -> set[str]:
        return {a for s in self.claims.values() for a in s}

    @property
    def present(self) -> dict[str, set[str]]:
        """version -> every anchor this tree CARRIES for it, of any kind.

        Deliberately not [`claims`]: claims apply precedence, so a version that
        gains a hand-written name reads as having lost its derived one, and a
        withdrawal computed from claims would call that loss a cancellation. A
        withdrawal is a fact about what a tree contains, so it is read off what
        the tree contains.
        """
        carried: dict[str, set[str]] = defaultdict(set)
        for version, names in self.named.items():
            carried[version] |= names
        for version, names in self.derived.items():
            carried[version] |= names
        for version, anchor in self.reserved.items():
            carried[version].add(anchor)
        return carried

    @property
    def defined_anchor_names(self) -> set[str]:
        """Hand-written anchors whose surface is IMPLEMENTED in this tree —
        reservations excluded, since a reservation is the promise, not the
        surface. Rule 5 reads exactly this."""
        return {a for s in self.named.values() for a in s}


EMPTY_SIDE = LedgerSide([], {}, {}, {})


def read_ledger(ledger: dict, source: str, where: str) -> LedgerSide:
    versions = versions_of(ledger, source)
    if not versions:
        raise ShapeDrift(
            f"{ledger['name']}: {ledger['list_const']} in {ledger['path']} parsed to "
            f"ZERO versions at {where}. A ledger with no versions is a parse failure, "
            f"not a ledger."
        )
    side = LedgerSide(
        versions,
        ledger["named"](source),
        ledger["derived"](source),
        reservations_of(ledger["reserved_const"], source),
    )
    if len(versions) >= 2 and not side.claims:
        raise ShapeDrift(
            f"{ledger['name']}: {ledger['path']} at {where} names {len(versions)} "
            f"versions and ZERO of them could be traced to a fence anchor via "
            f"{ledger['claim_pattern']!r}. The extraction has drifted; every version "
            f"would then read as unclaimed. Fix the pattern — never loosen the check."
        )
    return side


def base_source(base: str, path: str) -> str | None:
    """The file's content at `base`, or None if it does not exist there."""
    result = subprocess.run(
        ["git", "show", f"{base}:{path}"], cwd=ROOT, capture_output=True, text=True
    )
    return result.stdout if result.returncode == 0 else None


def merge_base_with(base: str) -> str | None:
    """The fork point of this checkout's `HEAD` and `base`, or None.

    Local only — `git merge-base` reads the object store and touches no network.
    None is the honest answer in two ordinary states and is never an error: an
    unborn `HEAD` (nothing has been committed here), and a shallow checkout,
    where the graft hides every parent so git reports no common ancestor rather
    than refusing. Both fall back to the union, which refuses more rather than
    less.
    """
    result = subprocess.run(
        ["git", "merge-base", "HEAD", base], cwd=ROOT, capture_output=True, text=True
    )
    sha = result.stdout.strip()
    return sha if result.returncode == 0 and sha else None


def withdrawals_of(
    inherited_reserved: dict[str, str],
    local: LedgerSide,
    base_side: LedgerSide,
    base: str,
) -> list[tuple[str, str, str]]:
    """`(version, anchor, who cancelled it)` for every hold the merge drops.

    A row inherited from the merge base and gone from a side was deleted by that
    side, so it is not in the tree the merge produces. Two restrictions, and they
    are what stop this from being an amnesty:

    * only a **reservation** at the merge base is offered the subtraction — a
      landed `*_SINCE` or a derived predicate that vanishes is a deletion of
      shipped surface and stays rule 1's finding;
    * and only while **neither tree defines the anchor**, because a hold whose
      surface has landed is no longer a hold. That is the case a competitor
      cannot escape: its base-side claim is either implemented, or it arrived
      after the fork and was never inherited at all.
    """
    dropped: list[tuple[str, str, str]] = []
    local_present, base_present = local.present, base_side.present
    for version, anchor in sorted(inherited_reserved.items()):
        if anchor in local.defined_anchor_names or anchor in base_side.defined_anchor_names:
            continue
        gone_here = anchor not in local_present.get(version, set())
        gone_there = anchor not in base_present.get(version, set())
        if gone_here and gone_there:
            dropped.append((version, anchor, f"this branch and {base}"))
        elif gone_here:
            dropped.append((version, anchor, "this branch"))
        elif gone_there:
            dropped.append((version, anchor, base))
    return dropped


def check_ledger(ledger: dict, base: str, merge_base: str | None) -> tuple[list[str], str]:
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

    local = read_ledger(ledger, local_source, "this checkout")
    base_side = EMPTY_SIDE if base_src is None else read_ledger(ledger, base_src, base)

    local_versions, local_claims = local.versions, local.claims
    base_versions, base_claims = base_side.versions, base_side.claims
    local_reserved = local.reserved

    # The third tree. Only its reservation rows are read — a hold is the one
    # kind of claim a merge is allowed to drop — so an old fork point whose
    # ledger no longer matches this gate's other patterns cannot make the run
    # exit 2 for a file nobody is proposing to change.
    inherited_reserved = (
        {}
        if merge_base is None
        else reservations_of(ledger["reserved_const"], base_source(merge_base, path) or "")
    )
    withdrawn = withdrawals_of(inherited_reserved, local, base_side, base)
    withdrawn_anchors: dict[str, set[str]] = defaultdict(set)
    for version, anchor, _ in withdrawn:
        withdrawn_anchors[version].add(anchor)

    # Rule 1 — one number, one surface, over the claims that stand once this
    # branch merges: the union of the two trees, less what either side withdrew.
    collisions = 0
    for version in sorted(set(local_claims) | set(base_claims)):
        here = set(local_claims.get(version, set()))
        there = set(base_claims.get(version, set()))
        both = (here | there) - withdrawn_anchors.get(version, set())
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
        if merge_base is None:
            lines.append(
                f"    NOTE: no merge base between HEAD and {base} was available (unborn "
                f"HEAD, or a shallow checkout), so nothing could be read as WITHDRAWN. "
                f"If one of the claims above is a hold this branch cancels, this is "
                f"that hold reading as a competitor — fetch enough history for `git "
                f"merge-base HEAD {base}` to answer and run again."
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
    defined_here = local.defined_anchor_names
    for version, anchor in sorted(local_reserved.items()):
        if anchor in defined_here:
            findings.append(
                f"{name}: version {version} is reserved for {anchor}, and {anchor} is "
                f"defined in this same tree. The surface has landed, so the "
                f"reservation now refuses a version this engine can honour — delete "
                f"the {ledger['reserved_const']} row."
            )

    # Rule 6 — a number this branch ALLOCATES carries a hand-written name.
    #
    # A number is allocated two ways, and both are examined. Added: present in
    # the checkout and absent at `--base` — and only when there is a base ledger
    # to have added it against. Re-allocated: a hold this branch withdrew, which
    # frees the number and hands it to whatever this change puts there. What is
    # NOT examined is a number already at `--base` and still held there: it is
    # not being allocated, and re-naming one is the rename rule 2 refuses.
    #
    # A version whose sole claim is a derived anchor (`is_v12`) is unnamed — the
    # next branch to take that number computes the same anchor and rule 1 reads
    # one claim where there are two.
    added = [] if base_src is None else [v for v in local_versions if v not in set(base_versions)]
    reallocated = [
        version
        for version, _, who in withdrawn
        if who != base and version not in set(added) and version in set(local_versions)
    ]
    for version in added + reallocated:
        if local.named.get(version) or version in local_reserved:
            continue
        derived_here = sorted(local.derived.get(version, set()))
        how = (
            "is added by this branch"
            if version in set(added)
            else "had its hold withdrawn by this branch, which frees the number,"
        )
        findings.append(
            f"{name}: version {version} {how} and nothing NAMES it. "
            f"Its only claim is {derived_here or ['(nothing)']}, which is computed from "
            f"the number itself — a second branch adding {version} for a different "
            f"surface computes the same anchor, and rule 1 reads one claim where there "
            f"are two. That is not hypothetical: it is how one number came to be "
            f"allocated twice.\n"
            f"    Fix: name it. A `<SURFACE>_SINCE = \"{version}\"` constant if this "
            f"change lands the surface, or a {ledger['reserved_const']} row naming the "
            f"constant a sibling change will define if it does not."
        )

    if merge_base is None:
        withdrawal_note = (
            f"withdrawals not computable (no merge base with {base}) — a hold this "
            f"branch cancels reads as a competing claim"
        )
    elif withdrawn:
        # ` | ` rather than `; `, which joins one ledger's summary to the next.
        withdrawal_note = f"{len(withdrawn)} withdrawal(s): " + " | ".join(
            f"{version} {anchor} (withdrawn by {who})" for version, anchor, who in withdrawn
        )
    else:
        withdrawal_note = f"0 withdrawal(s) from {len(inherited_reserved)} inherited hold(s)"

    n_local_anchors = len(local.anchors)
    n_base_anchors = len(base_side.anchors)
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
            f"{len(local_reserved)} reserved, {collisions} collision(s), "
            f"{len(added)} added version(s) examined for a name, {withdrawal_note}"
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

    # The fork point, named by its revision rather than by "the base", because
    # it is the instrument that decides whether a missing row was withdrawn or
    # never seen. Absent, everything reads as never seen (see `merge_base_with`).
    merge_base = merge_base_with(base)
    against = (
        f"{base} @ {base_sha[:12]}, merge base {merge_base[:12]}"
        if merge_base
        else f"{base} @ {base_sha[:12]}, no merge base"
    )

    findings: list[str] = []
    summaries: list[str] = []
    for ledger in LEDGERS:
        try:
            ledger_findings, summary = check_ledger(ledger, base, merge_base)
        except ShapeDrift as drift:
            print(f"check-version-ledger-uniqueness: {drift}", file=sys.stderr)
            return 2
        findings.extend(ledger_findings)
        summaries.append(summary)

    if findings:
        print(
            f"check-version-ledger-uniqueness: {len(findings)} finding(s) against "
            f"{against} — {'; '.join(summaries)}\n",
            file=sys.stderr,
        )
        for finding in findings:
            print(f"  {finding}\n", file=sys.stderr)
        return 1

    print(
        f"check-version-ledger-uniqueness: OK — {'; '.join(summaries)}, diffed against "
        f"{against}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
