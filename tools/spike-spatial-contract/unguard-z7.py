#!/usr/bin/env python3
"""SPIKE TOOLING (ADR-PENDING-map-design-pipeline, spec-PENDING §4.1) — NOT part
of the shipped pipeline.

Writes an *unguarded* copy of the Z7 bell-tower program's JSON IR: the same
program with the three seam-identity clauses of `tower_plan` deleted and every
frame clause kept.

Why this exists. `crates/grammar/tests/zones.rs::the_towers_plinth_arithmetic_is_
guarded_not_hoped` shows four knob drifts each being **refused by the program's
own guard** — the expansion errors, and no blocks are ever produced. A spatial
contract is checked against delivered blocks, so on the shipped program the
checker can never see those drifts at all. To answer the question the design
actually asks — "is the zone-gate discipline expressible as data?" — the drift
has to reach geometry. This removes the guard so it can, and nothing else: run
at the default parameters the unguarded copy must expand byte-identically to the
guarded one, which the driver asserts.

It edits a JSON file on disk. No library program is touched.

  python3 unguard-z7.py <z7-bell-tower.json> <out.json>
"""

import json
import sys
from pathlib import Path

# The clauses of `tower_plan`'s single alternative, in the order
# `crates/grammar/src/library/bell/bell_tower.rs` writes them. The three deleted
# here are the ones the module note calls "the one seam arithmetic this zone
# owes"; clauses 0-6 and 8 are frame constraints about the box and stay.
SEAM_CLAUSES = {
    7: "climb == treads()",
    9: "shaft/sill == climb",
    10: "dim(Y) - shaft/sill == shaft/storey",
}


def main(argv):
    if len(argv) != 3:
        print(__doc__)
        return 2
    program = json.loads(Path(argv[1]).read_text())
    alts = program["rules"]["tower_plan"]
    if len(alts) != 1:
        raise SystemExit("tower_plan is expected to have exactly one alternative")
    clauses = alts[0]["when"]["of"]
    if len(clauses) != 11:
        raise SystemExit(f"tower_plan's guard has {len(clauses)} clauses, expected 11")
    kept = [c for i, c in enumerate(clauses) if i not in SEAM_CLAUSES]
    alts[0]["when"]["of"] = kept
    Path(argv[2]).write_text(json.dumps(program, indent=1) + "\n")
    print(f"dropped {len(SEAM_CLAUSES)} seam clause(s), kept {len(kept)} frame clause(s):")
    for i, name in sorted(SEAM_CLAUSES.items()):
        print(f"  - clause {i}: {name}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
