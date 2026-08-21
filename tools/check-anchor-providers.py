#!/usr/bin/env python3
"""One authority answers *what anchors does this campaign have* — nobody else asks.

WHY THIS EXISTS

`AnchorRegistry::anchors_for(prefab)` answers a narrow question: which anchor
names does THIS PREFAB declare. Turning that into the question every check
actually has — *is this name one some area of this campaign provides* — takes a
walk over `world.areas`, and eleven checks in `dsl::validate` each wrote their
own. They were identical, they were correct, and they were correct about a world
that no longer exists.

When a site plan became a second placement authority (spec-0049 §5.2) a campaign
stopped needing prefabs at all: the derivation synthesizes the whole spatial
vocabulary — `anchor/node-<place>` per place, `anchor/seam-<edge>` over every
barred way, `anchor/unlock-<edge>` on the openable side of a one-sided one — and
`siteplan::synthesized_anchors` is the authority for the names. **One** of the
eleven walks was taught to ask it. The other ten went on enumerating prefabs a
derived world does not have, so every stage-5 verb but the ones that happen to
walk was unauthorable on a derived map: a `shortcut` naming the very
`anchor/unlock-<edge>` the derivation places for it was refused as an invented
name, and so were a trap, a shop, a loot chest, a lethal volume, a lane, a timed
gate, an actor and a trigger.

**Nothing was red, and nothing could have been.** A check that resolves against a
smaller world than the campaign has refuses CONTENT; its own binding counts stay
truthful about what it was handed. It surfaced only when somebody tried to author
the second verb on a derived world — five rounds after the first.

This is `CLAUDE.md`'s recorded shape (*a hand-rolled walk enumerating three of
five effect roots: a defect of expressibility, not of care*), and it has the same
repair: one authority, and a gate that stops the next copy being written. The
repair without the gate is the instance fix without the general form.

WHAT IS CHECKED

Every `anchors_for(` call site in `crates/dsl/src/` is inside one of the files
that may legitimately hold one:

- `registry.rs` — the trait's own declaration and its implementations, which is
  where the narrow question is DEFINED;
- `validate.rs` — permitted at most once, inside `AnchorProviders::build`, which
  is the one place the broad question is answered.

A second call in `validate.rs`, or a call anywhere else, is the eleventh walk
arriving, and it fails here instead of shipping a check blind to whatever
placement authority lands next.

WHAT THIS GATE DOES *NOT* PROVE

That `AnchorProviders` is CORRECT, or that every consumer uses the right
accessor: a check calling `resolvable` where it needed `for_area` is a different
defect and this gate cannot see it. What it guarantees is that there is exactly
one place to fix when the answer changes — which is the property the ten copies
destroyed.

BINDING COUNT

Every run prints the call sites it found and the population it looked at. **Zero
call sites is a FAILURE, not a pass**: the trait would have been renamed and this
gate would be silently guarding nothing.

Deterministic, offline, no dependencies (Python 3 stdlib). Run from anywhere:

    python3 tools/check-anchor-providers.py

Exit 0 = one authority, 1 = a finding (see stderr), 2 = IO error.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
DSL_SRC = REPO / "crates" / "dsl" / "src"

CALL_RE = re.compile(r"\banchors_for\s*\(")

# The file that DEFINES the narrow question, and the file that answers the broad
# one. Every other call site is a copy of a walk that has already been written.
DEFINITION = "registry.rs"
AUTHORITY = "validate.rs"
AUTHORITY_FN = "impl AnchorProviders"


def call_sites(text: str) -> list[int]:
    """1-based line numbers of every `anchors_for(` call in `text`."""
    return [i for i, line in enumerate(text.splitlines(), 1) if CALL_RE.search(line)]


def main() -> int:
    if not DSL_SRC.is_dir():
        print(f"check-anchor-providers: FAIL — {DSL_SRC} is missing", file=sys.stderr)
        return 2

    files = sorted(DSL_SRC.rglob("*.rs"))
    findings: list[str] = []
    total = 0
    per_file: dict[str, list[int]] = {}

    for f in files:
        text = f.read_text(encoding="utf-8")
        lines = call_sites(text)
        if not lines:
            continue
        rel = str(f.relative_to(REPO))
        per_file[rel] = lines
        total += len(lines)

        if f.name == DEFINITION:
            continue
        if f.name == AUTHORITY:
            # The one broad answer, and only one: `AnchorProviders::build`.
            if AUTHORITY_FN not in text:
                findings.append(
                    f"{rel} calls `anchors_for` and has no `{AUTHORITY_FN}` — the "
                    "one authority this gate exists to protect is gone, so the "
                    "call is a hand-rolled walk by definition"
                )
            elif len(lines) > 1:
                findings.append(
                    f"{rel} calls `anchors_for` {len(lines)} times (lines "
                    f"{', '.join(map(str, lines))}). Exactly one is permitted, "
                    "inside `AnchorProviders::build`.\n"
                    "    A second call is a second answer to *what anchors does "
                    "this campaign have*, and the last time this file held eleven "
                    "of them, ten were blind to the site plan and every stage-5 "
                    "verb but two was unauthorable on a derived map — green, "
                    "because a check resolving against a truncated world refuses "
                    "CONTENT.\n"
                    "    Use `AnchorProviders::build(c, anchors)` and its "
                    "accessors (`resolvable`, `for_area`, `union`, "
                    "`all_areas_known`). If it cannot answer your question, WIDEN "
                    "IT — that is the whole point of there being one."
                )
            continue

        findings.append(
            f"{rel} calls `anchors_for` at line(s) {', '.join(map(str, lines))}.\n"
            "    Only `registry.rs` (which declares it) and "
            "`validate.rs`'s `AnchorProviders::build` (which answers the broad "
            "question with it) may. Anywhere else is a walk over `world.areas` "
            "that will be correct until the next placement authority lands and "
            "will then be quietly wrong about a whole class of campaign."
        )

    binding = (
        f"{total} `anchors_for` call site(s) across {len(per_file)} of "
        f"{len(files)} file(s) under {DSL_SRC.relative_to(REPO)}"
    )

    if total == 0:
        print(
            "check-anchor-providers: FAIL — found 0 `anchors_for` call sites. The "
            "trait method this gate keys off has been renamed or removed, so the "
            "gate is guarding nothing and a green here means nothing (CLAUDE.md: "
            "a green gate that binds to nothing is vacuous). Fix the pattern, do "
            "not drop the gate.",
            file=sys.stderr,
        )
        return 1

    if findings:
        print(
            f"check-anchor-providers: {len(findings)} finding(s) — bound to {binding}\n",
            file=sys.stderr,
        )
        for finding in findings:
            print(f"  - {finding}", file=sys.stderr)
        return 1

    print(f"check-anchor-providers: OK — one authority. Bound to {binding}.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
