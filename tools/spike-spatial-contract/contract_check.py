#!/usr/bin/env python3
"""SPIKE TOOLING (ADR-PENDING-map-design-pipeline, spec-PENDING §4.1) — NOT part
of the shipped pipeline, and deliberately not in a crate.

The **cheap falsifier** the spec's order-of-work puts before any IR change: a
standalone checker for a hand-written spatial contract against blocks already on
disk. It changes no IR, adds no node, emits nothing, and imports nothing from
`crates/grammar` — the walk predicates are ported into `voxels.py` so that this
prototype is able to disagree with the engine.

  python3 contract_check.py <manifest.json> <contract.json> [--list N] [--json PATH]

Exit codes follow `delve-grammar`: 0 pass, 2 input, 4 a gate went red.

# The contract

A contract is one JSON object. Boxes are inclusive `[x0,y0,z0,x1,y1,z1]` in
zone-local coordinates — the coordinates the manifest's parts reassemble into.

    {
      "zone": "<what this is a contract for>",
      "entry": "<space name>",
      "spaces": [ {"name": "nave", "envelope": "enclosed", "box": [...]}, ... ],
      "no_body": [ {"name": "aisle-roof-north", "box": [...]}, ... ],
      "edges": [
        {"a": "parvis", "b": "west-portal-bay", "class": "walk"},
        {"a": "exterior", "b": "parvis", "class": "walk", "via": [...]},
        {"a": "gallery", "b": "nave", "class": "drop"},
        {"a": "ward", "b": "shortcut", "class": "barred", "bar": [...]},
        {"a": "nave", "b": "exterior", "class": "vision", "via": [...]}
      ]
    }

One box per space, because that is what the proposed `space` declaration node
gives an author: it claims the scope's box. A room that is not a box costs
several named spaces, and the point of this prototype is to find out how many.

# The obligations, and which of them are the spec's

Gates 1-8 are spec-PENDING §2, implemented as written and never weakened.
Gate 9 is **not in the spec**. It is printed as `[BEYOND SPEC]` because writing
the first honest contract by hand made it necessary: §2's coverage and
reachability obligations are both stated per *space*, so a space whose own
standable cells are severed from each other satisfies both. Every verdict states
its binding count, and a zero binding is reported as a finding rather than a
pass.
"""

import json
import sys
from collections import deque
from pathlib import Path

from voxels import Grid, fall_closure, walk_closure

EXIT_INPUT = 2
EXIT_GATE = 4

ENVELOPES = ("enclosed", "open_top", "open")
CLASSES = ("walk", "stair", "drop", "barred", "vision")
EXTERIOR = "exterior"


# --- boxes -------------------------------------------------------------------

def cells_of(box):
    x0, y0, z0, x1, y1, z1 = box
    for x in range(x0, x1 + 1):
        for y in range(y0, y1 + 1):
            for z in range(z0, z1 + 1):
                yield (x, y, z)


def in_box(box, c):
    x0, y0, z0, x1, y1, z1 = box
    return x0 <= c[0] <= x1 and y0 <= c[1] <= y1 and z0 <= c[2] <= z1


def volume(box):
    x0, y0, z0, x1, y1, z1 = box
    return (x1 - x0 + 1) * (y1 - y0 + 1) * (z1 - z0 + 1)


def overlap(a, b):
    """The intersecting box, or None."""
    lo = [max(a[i], b[i]) for i in range(3)]
    hi = [min(a[i + 3], b[i + 3]) for i in range(3)]
    if any(lo[i] > hi[i] for i in range(3)):
        return None
    return lo + hi


def shell(box, envelope):
    """The boundary of a space: the cell layer immediately outside each face a
    body must not pass through. `open_top` drops the top plane; `open` has none."""
    x0, y0, z0, x1, y1, z1 = box
    planes = [
        (x0 - 1, x0 - 1, y0, y1, z0, z1),
        (x1 + 1, x1 + 1, y0, y1, z0, z1),
        (x0, x1, y0 - 1, y0 - 1, z0, z1),
        (x0, x1, y0, y1, z0 - 1, z0 - 1),
        (x0, x1, y0, y1, z1 + 1, z1 + 1),
    ]
    if envelope == "enclosed":
        planes.append((x0, x1, y1 + 1, y1 + 1, z0, z1))
    for ax0, ax1, ay0, ay1, az0, az1 in planes:
        for x in range(ax0, ax1 + 1):
            for y in range(ay0, ay1 + 1):
                for z in range(az0, az1 + 1):
                    yield (x, y, z)


# --- report ------------------------------------------------------------------

class Report:
    def __init__(self, zone, prefab_id):
        self.zone = zone
        self.prefab_id = prefab_id
        self.gates = []
        self.findings = []

    def gate(self, gid, ok, bound, detail, cells=None, beyond_spec=False):
        self.gates.append({
            "id": gid, "pass": bool(ok), "bound": bound, "detail": detail,
            "cells": sorted(cells) if cells else [],
            "beyond_spec": beyond_spec,
        })
        if bound == 0:
            self.findings.append(f"{gid}: BINDING ZERO — this gate examined nothing")

    def finding(self, text):
        self.findings.append(text)

    @property
    def failed(self):
        return [g for g in self.gates if not g["pass"]]

    def render(self, list_n):
        out = []
        out.append(f"spatial contract — {self.zone}")
        out.append(f"  prefab {self.prefab_id}")
        out.append("")
        width = max(len(g["id"]) for g in self.gates)
        for g in self.gates:
            mark = "pass" if g["pass"] else "RED "
            tag = " [BEYOND SPEC]" if g["beyond_spec"] else ""
            out.append(f"  {g['id']:<{width}}  {mark}  bound {g['bound']:<7} {g['detail']}{tag}")
            for c in g["cells"][:list_n]:
                out.append(f"      {c[0]},{c[1]},{c[2]}")
            if len(g["cells"]) > list_n:
                out.append(f"      ... {len(g['cells']) - list_n} more")
        out.append("")
        if self.findings:
            out.append(f"  findings ({len(self.findings)}) — printed on pass as well as fail:")
            for f in self.findings:
                out.append(f"    - {f}")
        else:
            out.append("  findings (0)")
        out.append("")
        red = self.failed
        if red:
            out.append(f"VERDICT: RED — {len(red)} of {len(self.gates)} obligations: "
                       + ", ".join(g["id"] for g in red))
        else:
            out.append(f"VERDICT: pass — {len(self.gates)} obligations, "
                       f"{len(self.findings)} finding(s)")
        return "\n".join(out)


# --- the checker -------------------------------------------------------------

def check(grid, manifest, contract, report):
    spaces = {s["name"]: s for s in contract.get("spaces", [])}
    nobody = {n["name"]: n for n in contract.get("no_body", [])}
    edges = contract.get("edges", [])
    entry = contract.get("entry")
    standable = grid.standable_cells()

    # --- 1. well-formed ------------------------------------------------------
    problems = []
    examined = len(spaces) + len(nobody) + len(edges)
    for name, s in sorted(spaces.items()):
        if s.get("envelope") not in ENVELOPES:
            problems.append(f"space {name}: envelope {s.get('envelope')!r} is not one of {ENVELOPES}")
    if entry not in spaces:
        problems.append(f"entry {entry!r} is not a declared space")
    names = sorted(set(spaces) | set(nobody))
    if len(set(spaces) & set(nobody)) > 0:
        problems.append("a name is declared as both a space and a no_body region: "
                        + ", ".join(sorted(set(spaces) & set(nobody))))
    boxes = [(n, (spaces.get(n) or nobody[n])["box"]) for n in names]
    for i in range(len(boxes)):
        for j in range(i + 1, len(boxes)):
            (na, ba), (nb, bb) = boxes[i], boxes[j]
            ov = overlap(ba, bb)
            if ov is not None and ba != bb:
                problems.append(f"{na} and {nb} overlap in {volume(ov)} cell(s) without being "
                                f"identical — refused at this version")
    for e in edges:
        for side in ("a", "b"):
            if e[side] != EXTERIOR and e[side] not in spaces:
                problems.append(f"edge {e['a']}-{e['b']}: {e[side]!r} is neither a declared "
                                f"space nor {EXTERIOR!r}")
        if e.get("class") not in CLASSES:
            problems.append(f"edge {e['a']}-{e['b']}: class {e.get('class')!r} is not one of {CLASSES}")
        if e.get("class") == "barred" and "bar" not in e:
            problems.append(f"edge {e['a']}-{e['b']}: a barred edge must declare `bar`")
        if e.get("class") != "barred" and "bar" in e:
            problems.append(f"edge {e['a']}-{e['b']}: only a barred edge may declare `bar`")
        if e.get("class") == "barred" and "bar" in e:
            # the bar has to sit on the shared boundary of the two endpoints
            ba = spaces[e["a"]]["box"] if e["a"] != EXTERIOR else None
            bb = spaces[e["b"]]["box"] if e["b"] != EXTERIOR else None
            touching = False
            for b in (ba, bb):
                if b is None:
                    continue
                grown = [b[0] - 1, b[1] - 1, b[2] - 1, b[3] + 1, b[4] + 1, b[5] + 1]
                if overlap(grown, e["bar"]) is not None:
                    touching = True
            if not touching:
                problems.append(f"edge {e['a']}-{e['b']}: bar region touches neither endpoint")
    report.gate("well-formed", not problems, examined,
                f"{len(spaces)} space(s), {len(nobody)} no_body region(s), {len(edges)} edge(s)"
                + ("" if not problems else "; " + "; ".join(problems[:6])
                   + (f"; ... {len(problems)-6} more" if len(problems) > 6 else "")))

    # --- 2. coverage ---------------------------------------------------------
    covered_by_space, covered_by_nobody, uncovered = set(), set(), set()
    space_boxes = [(n, s["box"]) for n, s in sorted(spaces.items())]
    nobody_boxes = [(n, s["box"]) for n, s in sorted(nobody.items())]
    for c in sorted(standable):
        if any(in_box(b, c) for _, b in space_boxes):
            covered_by_space.add(c)
        elif any(in_box(b, c) for _, b in nobody_boxes):
            covered_by_nobody.add(c)
        else:
            uncovered.add(c)
    report.gate("coverage", not uncovered, len(standable),
                f"{len(standable)} standable cell(s): {len(covered_by_space)} in a declared "
                f"space, {len(covered_by_nobody)} in a no_body region, {len(uncovered)} "
                f"undeclared", cells=uncovered)
    if len(covered_by_nobody) > len(covered_by_space):
        report.finding(f"no_body majority: {len(covered_by_nobody)} of {len(standable)} standable "
                       f"cell(s) are declared standable-but-unowned, more than the "
                       f"{len(covered_by_space)} a body is claimed to occupy")

    # --- 3. closure ----------------------------------------------------------
    # exemptions: an edge's opening, and the face shared with an abutting space
    openings = []
    for e in edges:
        if "via" in e:
            openings.append(e["via"])
        if e.get("class") == "barred":
            openings.append(e["bar"])
    boundary_examined = 0
    breaches = set()
    closed_spaces = [(n, s) for n, s in sorted(spaces.items()) if s["envelope"] != "open"]
    for name, s in closed_spaces:
        others = [b for n2, b in space_boxes if n2 != name]
        for c in shell(s["box"], s["envelope"]):
            boundary_examined += 1
            if not grid.passable(c):
                continue
            if any(in_box(b, c) for b in others):
                continue          # a face shared with an abutting declared space
            if any(in_box(v, c) for v in openings):
                continue          # inside a declared edge's opening
            breaches.add(c)
    report.gate("closure", not breaches, boundary_examined,
                f"{len(closed_spaces)} space(s) with a closed envelope, {boundary_examined} "
                f"boundary cell(s) examined, {len(breaches)} unexplained passable",
                cells=breaches)

    # --- 4. edge proof -------------------------------------------------------
    def side_cells(name, e):
        if name != EXTERIOR:
            return set(c for c in standable if in_box(spaces[name]["box"], c))
        if "via" in e:
            return set(c for c in standable if in_box(e["via"], c))
        faces = set()
        for c in standable:
            if c[0] in (0, grid.w - 1) or c[2] in (0, grid.d - 1):
                faces.add(c)
        return faces

    def arena(e):
        cells = set()
        for name in (e["a"], e["b"]):
            if name != EXTERIOR:
                cells |= set(c for c in standable if in_box(spaces[name]["box"], c))
        for key in ("via", "bar"):
            if key in e:
                cells |= set(c for c in standable if in_box(e[key], c))
        if EXTERIOR in (e["a"], e["b"]):
            cells |= side_cells(EXTERIOR, e)
        return cells

    edge_failures, beyond_failures, proved = [], [], 0
    for e in edges:
        cls, a, b = e["class"], e["a"], e["b"]
        label = f"{a} -{cls}- {b}"
        A, B, cells = side_cells(a, e), side_cells(b, e), arena(e)
        if not A or not B:
            edge_failures.append(f"{label}: {'a' if not A else 'b'} side has no standable cell")
            continue
        proved += 1
        if cls in ("walk", "stair"):
            fwd = bool(walk_closure(cells, A) & B)
            bwd = bool(walk_closure(cells, B) & A)
            if not (fwd and bwd):
                edge_failures.append(f"{label}: the plain walk does not connect "
                                     f"{'a to b' if not fwd else 'b to a'}")
            elif cls == "stair":
                rise = abs(min(c[1] for c in B) - min(c[1] for c in A))
                if rise < 1:
                    edge_failures.append(f"{label}: measured rise {rise}, a stair must rise")
        # [BEYOND SPEC] a DECLARED level relation across the edge. spec-PENDING
        # §2.4 asks a stair only for `rise >= 1`, so a flight that arrives one
        # course off its landing walks fine under every class and is green. This
        # optional `rise` is the smallest surface that reds Z7's recorded drifts.
        if "rise" in e and A and B:
            got = min(c[1] for c in B) - min(c[1] for c in A)
            if got != e["rise"]:
                beyond_failures.append(f"{label}: declared rise {e['rise']}, measured {got}")
        elif cls == "drop":
            fwd = bool(fall_closure(grid, cells, A) & B)
            back = bool(walk_closure(cells, B) & A)
            if not fwd:
                edge_failures.append(f"{label}: b is not reachable from a under walk-and-fall")
            if back:
                edge_failures.append(f"{label}: a IS reachable back from b under the plain "
                                     f"step — a drop is one-way")
        elif cls == "barred":
            if walk_closure(cells, A) & B:
                edge_failures.append(f"{label}: connected while the bar stands")
            voided = Grid((grid.w, grid.h, grid.d),
                          {k: v for k, v in grid.blocks.items() if not in_box(e["bar"], k)})
            vstand = voided.standable_cells()
            varena = set(c for c in vstand
                         if any(in_box(spaces[n]["box"], c) for n in (a, b) if n != EXTERIOR)
                         or in_box(e["bar"], c)
                         or ("via" in e and in_box(e["via"], c)))
            vA = set(c for c in vstand if in_box(spaces[a]["box"], c)) if a != EXTERIOR else A
            vB = set(c for c in vstand if in_box(spaces[b]["box"], c)) if b != EXTERIOR else B
            if not (walk_closure(varena, vA) & vB):
                edge_failures.append(f"{label}: still not connected with the bar region voided")
        elif cls == "vision":
            pass  # no traversal claim; its opening exempts closure (applied above)
    report.gate("edge-proof", not edge_failures, proved,
                f"{proved} edge(s) proved of {len(edges)} declared"
                + ("" if not edge_failures else "; " + "; ".join(edge_failures[:8])
                   + (f"; ... {len(edge_failures)-8} more" if len(edge_failures) > 8 else "")))
    declared_rises = sum(1 for e in edges if "rise" in e)
    if declared_rises:
        report.gate("edge-level", not beyond_failures, declared_rises,
                    f"{declared_rises} edge(s) declaring a level relation"
                    + ("" if not beyond_failures else "; " + "; ".join(beyond_failures)),
                    beyond_spec=True)

    # --- 5. reachability -----------------------------------------------------
    def graph(open_barred):
        adj = {n: set() for n in list(spaces) + [EXTERIOR]}
        for e in edges:
            cls = e["class"]
            if cls == "vision":
                continue
            if cls == "barred" and not open_barred:
                continue
            adj[e["a"]].add(e["b"])
            if cls != "drop":
                adj[e["b"]].add(e["a"])
        return adj

    def bfs(adj, start):
        seen, q = {start}, deque([start])
        while q:
            n = q.popleft()
            for m in sorted(adj[n]):
                if m not in seen:
                    seen.add(m)
                    q.append(m)
        return seen

    walked_edges = sum(1 for e in edges if e["class"] != "vision")
    if entry in spaces:
        closed_reach = bfs(graph(False), entry)
        open_reach = bfs(graph(True), entry)
        never = sorted(set(spaces) - open_reach)
        gated = sorted(set(spaces) - closed_reach - set(never))
        detail = (f"{len(spaces)} space(s) over {walked_edges} traversable edge(s): "
                  f"{len(closed_reach & set(spaces))} reachable with every barred edge closed, "
                  f"{len(gated)} more once barred edges open, {len(never)} never")
        if gated:
            detail += "; behind a bar: " + ", ".join(gated)
        report.gate("reachability", not never, len(spaces) * max(walked_edges, 1),
                    detail + ("" if not never else "; UNREACHABLE: " + ", ".join(never)))
    else:
        report.gate("reachability", False, 0, "no declared entry to walk from")

    # --- 6. anchors ----------------------------------------------------------
    anchors = manifest.get("anchors", {})
    stray, in_nobody = [], []
    for name in sorted(anchors):
        p = tuple(anchors[name]["pos"])
        if any(in_box(b, p) for _, b in space_boxes):
            continue
        if any(in_box(b, p) for _, b in nobody_boxes):
            in_nobody.append(name)
        else:
            stray.append(name)
    report.gate("anchors", not stray, len(anchors),
                f"{len(anchors)} anchor(s), {len(stray)} in no declared space"
                + ("" if not stray else ": " + ", ".join(stray)))
    for name in in_nobody:
        report.finding(f"anchor {name} lies in a no_body region, not in a space a body occupies")

    # --- 7. exterior faces ---------------------------------------------------
    ext = [e for e in edges if EXTERIOR in (e["a"], e["b"])]
    ext_spaces = sorted({e["a"] if e["b"] == EXTERIOR else e["b"] for e in ext})
    report.gate("exterior-faces", len(ext_spaces) >= 2, len(ext),
                f"{len(ext)} exterior edge(s) touching {len(ext_spaces)} space(s)"
                + (f": {', '.join(ext_spaces)}" if ext_spaces else "")
                + ("" if len(ext_spaces) >= 2 else
                   " — a piece with fewer than two exterior faces makes no traversable claim"))

    # --- 8. vacuity findings (printed pass or fail) --------------------------
    if len(spaces) == 1 and not edges:
        report.finding("VACUOUS SHAPE: 1 space, 0 edges — the contract claims nothing about "
                       "how a body moves")
    open_envelopes = [n for n, s in sorted(spaces.items()) if s["envelope"] == "open"]
    if spaces and len(open_envelopes) == len(spaces):
        report.finding(f"VACUOUS ENVELOPE: all {len(spaces)} space(s) are `open`, so the closure "
                       f"obligation examined nothing")
    elif open_envelopes:
        report.finding(f"{len(open_envelopes)} of {len(spaces)} space(s) are `open` and exempt "
                       f"from closure: " + ", ".join(open_envelopes))
    if nobody_boxes:
        nb_vol = sum(volume(b) for _, b in nobody_boxes)
        sp_vol = sum(volume(b) for _, b in space_boxes)
        if nb_vol > sp_vol:
            report.finding(f"no_body VOLUME majority: {nb_vol} cell(s) declared unowned against "
                           f"{sp_vol} cell(s) of body space")

    # --- 9. [BEYOND SPEC] space integrity ------------------------------------
    # spec-PENDING states coverage and reachability per SPACE. A space whose own
    # standable cells are severed from each other satisfies both.
    severed, examined_spaces = [], 0
    for name, s in sorted(spaces.items()):
        cells = set(c for c in standable if in_box(s["box"], c))
        if len(cells) <= 1:
            continue
        examined_spaces += 1
        rest, islands, biggest = set(cells), 0, 0
        while rest:
            reached = walk_closure(rest, {min(rest)})
            islands += 1
            biggest = max(biggest, len(reached))
            rest -= reached
        if islands > 1:
            severed.append(f"{name}: {len(cells)} standable cell(s) in {islands} islands, "
                           f"the largest reaching {biggest}")
    report.gate("space-integrity", not severed, examined_spaces,
                f"{examined_spaces} space(s) with more than one standable cell"
                + ("" if not severed else "; " + "; ".join(severed[:8])
                   + (f"; ... {len(severed)-8} more" if len(severed) > 8 else "")),
                beyond_spec=True)


def main(argv):
    args = [a for a in argv[1:] if not a.startswith("--")]
    flags = [a for a in argv[1:] if a.startswith("--")]
    list_n = 12
    json_out = None
    perturb = []
    for f in flags:
        if f.startswith("--list="):
            list_n = int(f.split("=", 1)[1])
        elif f.startswith("--json="):
            json_out = f.split("=", 1)[1]
        elif f.startswith("--void=") or f.startswith("--fill="):
            # Perturb the DELIVERED BLOCKS before checking, so that a green can be
            # shown to be non-accidental: break the artifact, watch the obligation
            # that owns that defect go red. Nothing is written back to disk.
            kind, spec = f[2:].split("=", 1)
            perturb.append((kind, [int(v) for v in spec.split(",")]))
    if len(args) != 2:
        print(__doc__)
        return EXIT_INPUT
    manifest_path, contract_path = args
    grid, manifest = Grid.from_manifest(manifest_path)
    for kind, box in perturb:
        for c in cells_of(box):
            if not grid.inside(c):
                continue
            if kind == "void":
                grid.blocks.pop(c, None)
            else:
                grid.blocks[c] = "minecraft:stone"
        grid._standable = None
    contract = json.loads(Path(contract_path).read_text())
    report = Report(contract.get("zone", "unnamed"), manifest.get("prefab_id", "?"))
    for kind, box in perturb:
        report.finding(f"ARTIFACT PERTURBED: {kind} {','.join(str(v) for v in box)}")
    check(grid, manifest, contract, report)
    print(report.render(list_n))
    if json_out:
        Path(json_out).write_text(json.dumps(
            {"zone": report.zone, "gates": report.gates, "findings": report.findings},
            indent=1, sort_keys=True) + "\n")
    return EXIT_GATE if report.failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
