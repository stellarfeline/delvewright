#!/usr/bin/env python3
"""SPIKE TOOLING (ADR-PENDING-map-design-pipeline, spec-PENDING §4.1) — NOT part
of the shipped pipeline.

**The third adversary**, written against the §0 rules. `cheat.py` used the two
opt-outs §0 secured (`sealed`, `open` envelopes) and is now dead. This one uses
the two surfaces §0 did not reach.

**Attack F — pad the transit volume.** `via` on `stair`/`drop` is a transit
volume. Its only stated constraints are "disjoint from every space" and
"abutting both endpoints", `via` accepts a *list* of boxes, and §2.5 asks for
"every standable cell of every declared **space**". So: delete every space whose
cells are unreached, and hang those cells on an existing stair edge as extra
`via` boxes. Coverage accepts them (§2.2 counts transit volumes), closure never
looks at them (it examines spaces), the one-floor rule never looks at them, and
under §2.5's literal words neither does reachability. Nothing about the blocks
changes.

**Attack G — shop for a kind.** §2.6 says "a region satisfying several may
declare any one". The demands are defect-independent one at a time, but the
author picks, so the effective obligation is their disjunction — as strong as
its weakest term. This tries all three kinds per region and keeps whichever
passes, with no understanding of what the region is.

  python3 cheat2.py <manifest.json> <honest.json> <out.json> [--attack=F|G]

Run the output through `contract_check.py`. Attack F is scored under
`--literal-targets`, which is §2.5 as written.
"""

import json
import sys
from pathlib import Path

from voxels import Grid
import contract_check as cc


def bbox(cells):
    return [min(c[i] for c in cells) for i in range(3)] + \
           [max(c[i] for c in cells) for i in range(3)]


def probe(grid, manifest, contract, **kw):
    r = cc.Report("probe", "probe")
    cc.check(grid, manifest, contract, r, **kw)
    return {g["id"]: g for g in r.gates}


def attack_f(grid, manifest, honest):
    gates = probe(grid, manifest, honest)
    unreached = [tuple(c) for c in gates["reachability"]["cells"]]
    out = json.loads(json.dumps(honest))
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

    # a space entirely unreached is deleted outright; a partly-unreached one
    # keeps its box and donates the cells (which then overlap — so only whole
    # spaces are taken, which is enough).
    doomed = [n for n, cells in per_space.items()
              if len(cells) == len([c for c in grid.standable_cells()
                                    if cc.in_any(sboxes[n], c)])]
    # one 1x1x1 box per cell: a transit volume must be disjoint from every space,
    # and a bounding box is not. Ugly, legal, and a script does not mind.
    pad = [[c[0], c[1], c[2], c[0], c[1], c[2]]
           for n in sorted(doomed) for c in sorted(per_space[n])]
    out["spaces"] = [s for s in out["spaces"] if s["name"] not in doomed]
    out["edges"] = [e for e in out["edges"]
                    if e["a"] not in doomed and e["b"] not in doomed]

    stair = next((e for e in out["edges"] if e["class"] == "stair"), None)
    if stair is None:
        # no stair to hang it on? make one between two spaces that already walk.
        walkedge = next(e for e in out["edges"]
                        if e["class"] == "walk" and cc.EXTERIOR not in (e["a"], e["b"]))
        sb = {s["name"]: s["boxes"] for s in out["spaces"]}
        S = grid.standable_cells()
        ya = min(c[1] for c in S if cc.in_any(sb[walkedge["a"]], c))
        yb = min(c[1] for c in S if cc.in_any(sb[walkedge["b"]], c))
        stair = {"a": walkedge["a"], "b": walkedge["b"], "class": "stair",
                 "rise": max(1, yb - ya), "via": []}
        out["edges"].append(stair)
    stair["via"] = cc.as_boxes(stair.get("via")) + pad
    out["zone"] += (" — ATTACK F: every wholly-unreached space deleted and its cells "
                    "hung on a stair edge as extra transit-volume boxes")
    return out, f"deleted {len(doomed)} space(s), padded one stair via with {len(pad)} box(es)"


def attack_g(grid, manifest, honest):
    out = json.loads(json.dumps(honest))
    picked = []
    for r in out["no_body"]:
        best = None
        for kind in ("open", "posted", "sealed"):
            trial = json.loads(json.dumps(out))
            for rr in trial["no_body"]:
                if rr["name"] == r["name"]:
                    rr["kind"] = kind
            if probe(grid, manifest, trial)["no-body"]["pass"]:
                best = kind
                break
        if best and best != r["kind"]:
            picked.append(f"{r['name']}: {r['kind']} -> {best}")
            r["kind"] = best
    out["zone"] += " — ATTACK G: every no_body kind re-shopped for whichever one passes"
    return out, ("; ".join(picked) if picked else "no kind needed changing")


def main(argv):
    args = [a for a in argv[1:] if not a.startswith("--")]
    which = "F"
    for f in argv[1:]:
        if f.startswith("--attack="):
            which = f.split("=", 1)[1].upper()
    if len(args) != 3:
        print(__doc__)
        return 2
    grid, manifest = Grid.from_manifest(args[0])
    honest = json.loads(Path(args[1]).read_text())
    out, note = (attack_f if which == "F" else attack_g)(grid, manifest, honest)
    Path(args[2]).write_text(json.dumps(out, indent=1) + "\n")
    print(f"attack {which}: {note}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
