#!/usr/bin/env python3
"""No source file may enumerate the campaign's effect roots by hand.

## The defect this exists to end

An *effect root* is a `Vec<QuestEffect>` that emission can lower. There are seven,
six hang off the quests stage and one hangs off dialogue, and nothing about the
shape of the DSL makes them findable by inspection. So every walk that needed
"every effect" was written by someone enumerating the roots they happened to know
about, and each copy missed a different subset.

Six were found and fixed independently, by six unrelated investigations
(`plan::collect_gate_events`, `l10n::each_string`, `timeline::walk_campaign`,
`flow::read_flags`, `emit::check_effect_anchors`, `emit::declared_flags`). A sweep
afterwards found **thirteen more**. Not one of them was ever red: a walk that
visits four of five roots produces correct-looking output over any campaign that
happens not to use the fifth, and stays green until content routes an effect
through the root it cannot see.

`delvewright_dsl::effects` is now the single enumeration and every walk inherits
it, so the *next* root is one edit. That closes the thirteen. It does not stop a
fourteenth hand-rolled walk being written tomorrow — nothing in the type system
can, because the root fields are ordinary public fields on ordinary public
structs. This gate is that half.

**This gate has one blind spot by construction, and it has been hit** (spec-0031).
It greps for the roots it knows, so it cannot see a root that is *not in the
enumeration at all*: `shortcuts[].on_unlock` was a `Vec<QuestEffect>` emission
lowered for two versions while every walk, and this file, went green. The gate for
THAT shape is `tools/check-capability-ownership.py` check E, which enumerates the
effect-bundle FIELDS out of `stages.rs` and fails on any it cannot account for.
The two are complementary and neither replaces the other: this one catches a walk
that forgets a known root, that one catches a root nobody knows.

## What it flags

A window of `WINDOW` source lines that mentions `THRESHOLD` or more *distinct*
root markers. A walk that enumerates three of the seven roots names three root
fields within a few lines of each other; that is what this catches, and it is why
the threshold is on DISTINCT roots rather than on occurrences. Single-root access
— `dsl::validate` checking one quest's own `on_complete`, emission lowering one
trigger — is legitimate and never trips it.

Comment lines are skipped: this file's own prose, and the doc comments on the
walks that were fixed, name every root on purpose.

## Known non-proof, stated rather than implied

This is a proximity heuristic over text. A hand-rolled walk whose roots are spread
across a hundred lines, or which reaches them through a helper that takes the list
as an argument, is invisible here. It is a tripwire for the shape the thirteen
actually had, not a proof of absence — the proof-shaped half is the single
enumeration, and this backs it up.

## Binding count

Every run prints how many files it examined and how many root markers it found in
total. **Examining zero files, or finding zero markers, is a red**: a gate that
matched nothing is vacuous, not a pass (CLAUDE.md). If the marker patterns stop
matching because a field was renamed, this gate would otherwise go quietly green
forever.
"""

import re
import sys
import pathlib

WINDOW = 40
THRESHOLD = 3

# One pattern per effect root. Each must be specific enough that a match really is
# that root: `on_objective_complete` and `content.triggers` exist nowhere else,
# and the dialogue root is named by the accessor pair the walk uses.
ROOT_MARKERS = {
    "R1 on_objective_complete": re.compile(r"\bon_objective_complete\b"),
    "R2 quest on_complete": re.compile(r"\.on_complete\b"),
    "R3 triggers[].effects": re.compile(r"content\.triggers\b"),
    "R4 traps[].payload": re.compile(r"content\.traps\b"),
    "R5 dialogue on_respawn": re.compile(
        r"set_checkpoint_on_respawn\b|DialogueEffect::SetCheckpoint"
    ),
    "R6 shortcuts[].on_unlock": re.compile(r"content\.shortcuts\b"),
    "R7 campaign on_death": re.compile(r"content\.on_death\b"),
}

# Files allowed to name several roots close together, each with the reason. A new
# entry here is a claim someone has to defend in review — that is the point of it
# being a list of reasons rather than a list of paths.
ALLOWED = {
    "crates/dsl/src/effects.rs": (
        "THE enumeration. This is the one file that is supposed to name every "
        "root; every other walk inherits from it."
    ),
    "crates/compiler/src/plan.rs": (
        "`required_anchors_for_area` — OPEN FINDING, not a false positive. It "
        "collects the anchors an area's assembly must provide from R1+R2 (and R3 "
        "only when the campaign has a single area), so an anchor named only in a "
        "`traps[].payload` or a dialogue `on_respawn` bundle is never registered "
        "as required. Left unfixed deliberately: unlike every other walker in the "
        "sweep this is not a mechanical widening, because a trap payload has no "
        "area attribution — a trap carries an `at` anchor, not an area — and "
        "registering its anchors in EVERY area is the over-provisioning the "
        "function's own comment warns against. `DW0360`/`DW0447` still catch the "
        "resulting unresolved anchor at build time, so this is a worse message "
        "rather than a silent drop. Needs its own round with a layout diff."
    ),
}


def repo_root() -> pathlib.Path:
    return pathlib.Path(__file__).resolve().parent.parent


def scan(path: pathlib.Path):
    """Windows in `path` that name THRESHOLD+ distinct roots -> (line, roots)."""
    marks = {}
    for i, line in enumerate(path.read_text(encoding="utf-8").split("\n")):
        stripped = line.lstrip()
        if stripped.startswith("//") or stripped.startswith("#"):
            continue
        found = {name for name, pat in ROOT_MARKERS.items() if pat.search(line)}
        if found:
            marks[i] = found
    if not marks:
        return [], 0
    total = sum(len(v) for v in marks.values())
    order = sorted(marks)
    hits = []
    for a in order:
        seen = set()
        for b in order:
            if a <= b < a + WINDOW:
                seen |= marks[b]
        if len(seen) >= THRESHOLD:
            hits.append((a + 1, sorted(seen)))
            break
    return hits, total


def main() -> int:
    root = repo_root()
    files = sorted(
        p
        for p in (root / "crates").rglob("*.rs")
        if "/tests/" not in str(p) and "/target/" not in str(p)
    )
    examined = 0
    markers = 0
    offenders = []
    allowed_seen = set()

    for path in files:
        rel = str(path.relative_to(root))
        examined += 1
        hits, total = scan(path)
        markers += total
        if not hits:
            continue
        if rel in ALLOWED:
            allowed_seen.add(rel)
            continue
        offenders.append((rel, hits))

    # A gate that matched nothing is vacuous, not a pass.
    if examined == 0:
        print("FAIL: examined 0 source files — the crates tree moved or is empty")
        return 1
    if markers == 0:
        print(
            "FAIL: found 0 effect-root markers in {} files. Every marker pattern "
            "stopped matching, which means a root field was renamed and this gate "
            "is now blind. Update ROOT_MARKERS.".format(examined)
        )
        return 1

    stale = sorted(set(ALLOWED) - allowed_seen)
    for rel in stale:
        print(
            f"FAIL: {rel} is allowlisted but no longer names {THRESHOLD}+ roots "
            f"close together. Drop its ALLOWED entry — a stale exemption hides the "
            f"next real one."
        )

    for rel, hits in offenders:
        for line, roots in hits:
            print(
                f"FAIL: {rel}:{line} enumerates effect roots by hand "
                f"({', '.join(roots)})."
            )
            print(
                "      Walk `delvewright_dsl::for_each_effect_root` (or "
                "`for_each_campaign_effect`, which is defined in terms of it and "
                "also descends nesting) instead. A walk that lists roots itself is "
                "correct only until the next root is added, and it will not be red "
                "when it stops being correct."
            )

    if offenders or stale:
        return 1

    print(
        f"OK: {examined} source files examined, {markers} effect-root markers "
        f"found, 0 hand-rolled enumerations. Allowlisted by name with a reason: "
        f"{len(ALLOWED)} ({', '.join(sorted(ALLOWED))})."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
