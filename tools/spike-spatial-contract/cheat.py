#!/usr/bin/env python3
"""SPIKE TOOLING (ADR-PENDING-map-design-pipeline, spec-PENDING §4.1) — NOT part
of the shipped pipeline.

**The adversary.** Takes an honest contract that reds and mechanically rewrites
it into one that passes, without touching a single block of the artifact.

It applies exactly the two moves the amended obligations leave open, and nothing
clever:

1. **Seal what is not reached.** Every cell §2.5 could not reach becomes a
   `sealed` `no_body` region. §2.6's proof for `sealed` is "these cells are not
   reachable" — which is the *same property* that made them a reachability red.
   The proof is guaranteed to pass on exactly the cells that failed.
2. **Open what is not closed.** Every space whose envelope §2.3 found broken is
   downgraded to `envelope: "open"`, which §2.3 exempts entirely. As long as one
   sound space stays `enclosed`, §2.9's zero-binding red never fires.

Then it adds `no_body_majority_ack`, which §2.9 accepts as a string.

  python3 cheat.py <manifest.json> <honest.json> <out.json>

If the output passes, the amended obligations do not distinguish a broken
artifact from an intended one — the author's honesty is the only thing between
them, and that is what this script measures.
"""

import json
import sys
from pathlib import Path

from voxels import Grid
import contract_check as cc


def bbox(cells):
    xs = [c[0] for c in cells]
    ys = [c[1] for c in cells]
    zs = [c[2] for c in cells]
    return [min(xs), min(ys), min(zs), max(xs), max(ys), max(zs)]


def main(argv):
    if len(argv) != 4:
        print(__doc__)
        return 2
    grid, manifest = Grid.from_manifest(argv[1])
    honest = json.loads(Path(argv[2]).read_text())

    probe = cc.Report("probe", "probe")
    cc.check(grid, manifest, honest, probe)
    gates = {g["id"]: g for g in probe.gates}

    unreached = [tuple(c) for c in gates["reachability"]["cells"]]
    breaches = [tuple(c) for c in gates["closure"]["cells"]]

    out = json.loads(json.dumps(honest))
    out["zone"] = (honest.get("zone", "") +
                   " — MECHANICALLY CHEATED by cheat.py: unreached cells sealed, "
                   "broken envelopes opened")
    out["no_body_majority_ack"] = ("the piece is mostly fabric; the reasons above say which "
                                   "part and why")

    # 1. seal what is not reached, one region per owning space
    sboxes = {s["name"]: s["boxes"] for s in out["spaces"]}
    owner = {}
    for c in unreached:
        for n, bs in sboxes.items():
            if cc.in_any(bs, c):
                owner.setdefault(c, n)
                break
    per_space = {}
    for c, n in owner.items():
        per_space.setdefault(n, []).append(c)
    for n, cells in sorted(per_space.items()):
        out["no_body"].append({
            "name": f"sealed-{n}",
            "kind": "sealed",
            "reason": f"{len(cells)} cell(s) of {n} that no route was ever meant to reach",
            "boxes": [bbox(cells)],
        })

    # 2. open what is not closed
    broken = set()
    for c in breaches:
        for s in out["spaces"]:
            if s["envelope"] == "open":
                continue
            for b in s["boxes"]:
                if cc.overlap(cc.grow(b), [c[0], c[1], c[2], c[0], c[1], c[2]]):
                    broken.add(s["name"])
    for s in out["spaces"]:
        if s["name"] in broken:
            s["envelope"] = "open"

    Path(argv[3]).write_text(json.dumps(out, indent=1) + "\n")
    print(f"sealed {len(unreached)} unreached cell(s) into {len(per_space)} region(s); "
          f"opened {len(broken)} broken envelope(s): {', '.join(sorted(broken))}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
