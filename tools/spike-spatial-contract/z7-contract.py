#!/usr/bin/env python3
"""SPIKE TOOLING (ADR-PENDING-map-design-pipeline, spec-PENDING §4.1) — NOT part
of the shipped pipeline.

Emits Z7's spatial contract for a given parameter set.

Why a generator and not a static file. Z7's runs are program *parameters*; dial
`ring_run` and every piece after the ring slides down the zone. spec-PENDING §1a
declares a space as a node that wraps a body and claims **the scope's box**, so
inside the IR a contract moves with its parameters for free. §1b's prefab
metadata, and this prototype, carry literal boxes instead — which are only true
at one parameter set. This file is the smallest honest stand-in for the IR
behaviour: it recomputes the same boxes from the same parameters, so a drift
test measures the drift rather than measuring the fact that the contract was
written against different numbers.

The layout it encodes is `crates/grammar/src/library/bell/bell_tower.rs`'s own:
the strip is cut off the X-min side, the five upper pieces run low-to-high along
Z with the flight taking the remainder, and the storey stands on a plinth of
`climb - 1` courses.

  python3 z7-contract.py [ring_run=22] [climb=9] ... > contract.json
"""

import json
import sys

DEFAULTS = {
    "region_x": 41, "region_y": 14, "region_z": 125,
    "ring_run": 20, "door_run": 20, "tee_run": 21, "loft_run": 20, "hearth_run": 20,
    "strip_depth": 22, "climb": 9,
    "head_landing": 5,   # cells of the flight's slice standing at the storey's level
    "shaft_lane": 3,     # the hole the lift shaft cuts, across the tee's own centre
}


def contract(p):
    x0, xmax = p["strip_depth"], p["region_x"] - 1
    ymax = p["region_y"] - 1
    floor = p["climb"] - 1                       # the storey's own floor course
    zmax = p["region_z"] - 1

    ring_z0 = 0
    door_z0 = ring_z0 + p["ring_run"]
    tee_z0 = door_z0 + p["door_run"]
    loft_z0 = tee_z0 + p["tee_run"]
    hearth_z0 = loft_z0 + p["loft_run"]
    flight_z0 = hearth_z0 + p["hearth_run"]
    head_z1 = flight_z0 + p["head_landing"] - 1

    tee_mid = tee_z0 + p["tee_run"] // 2
    half = p["shaft_lane"] // 2
    shaft_x1 = x0 - 2                            # the doorway column sits at x0-1
    shaft_x0 = shaft_x1 - p["shaft_lane"] + 1

    def box(a, b, c, d, e, f):
        return [a, b, c, d, e, f]

    spaces = [
        ("stair-foot", "enclosed", [box(x0, 0, head_z1 + 1, xmax, ymax, zmax)]),
        ("stair-head", "enclosed", [box(x0, 0, flight_z0, xmax, ymax, head_z1)]),
        ("hearth", "enclosed", [box(x0, floor, hearth_z0, xmax, ymax, flight_z0 - 1)]),
        ("loft", "enclosed", [box(x0, floor, loft_z0, xmax, ymax, hearth_z0 - 1)]),
        ("landing", "enclosed", [box(x0 - 1, floor, tee_z0, xmax, ymax, loft_z0 - 1)]),
        ("threshold", "enclosed", [box(x0, floor, door_z0, xmax, ymax, tee_z0 - 1)]),
        ("ring", "enclosed", [box(x0, floor, ring_z0, xmax, ymax, door_z0 - 1)]),
        ("lift-shaft", "enclosed",
         [box(shaft_x0, 0, tee_mid - half, shaft_x1, ymax, tee_mid + half)]),
    ]
    # `rafter_hall` hangs its perches over the loft on purpose: standable, and
    # meant to stay out of reach. The amended §1a lets that nest inside the loft
    # instead of forcing it out of the room it belongs to.
    no_body = [
        ("loft-rafters", "sealed",
         "rafter_hall's perches: beams over the loft floor, standable by "
         "construction and deliberately not a landing",
         [box(x0 + 1, floor + 4, loft_z0, x0 + 2, ymax, hearth_z0 - 1),
          box(xmax - 2, floor + 4, loft_z0, xmax, ymax, hearth_z0 - 1)]),
    ]
    RISE = p["climb"] - 1
    edges = [
        ("exterior", "stair-foot", "walk", None),
        ("stair-foot", "stair-head", "stair", RISE),
        ("stair-head", "hearth", "walk", 0),
        ("hearth", "loft", "walk", 0),
        ("loft", "landing", "walk", 0),
        ("landing", "threshold", "walk", 0),
        ("threshold", "ring", "walk", 0),
        ("landing", "lift-shaft", "drop", -(p["climb"] - 1)),
        ("ring", "exterior", "walk", None),
    ]
    label = " ".join(f"{k}={p[k]}" for k in sorted(p) if p[k] != DEFAULTS[k]) or "defaults"
    return {
        "zone": f"bell Z7 — the Bell Tower ({p['region_x']}x{p['region_y']}x{p['region_z']}, "
                f"seed 1) — hand-written honest contract [{label}]",
        "entry": "stair-foot",
        "spaces": [{"name": n, "envelope": e, "boxes": b} for n, e, b in spaces],
        "no_body": [{"name": n, "kind": k, "reason": r, "boxes": b}
                    for n, k, r, b in no_body],
        "edges": [dict({"a": a, "b": b, "class": c}, **({} if r is None else {"rise": r}))
                  for a, b, c, r in edges],
    }


def main(argv):
    p = dict(DEFAULTS)
    for spec in argv[1:]:
        k, v = spec.split("=", 1)
        if k not in p:
            raise SystemExit(f"unknown parameter {k!r}; known: {', '.join(sorted(p))}")
        p[k] = int(v)
    print(json.dumps(contract(p), indent=1))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
