#!/usr/bin/env python3
"""SPIKE TOOLING (ADR-PENDING-map-design-pipeline, spec-PENDING §4.1) — NOT part
of the shipped pipeline, and deliberately not in a crate.

The **cheap falsifier** the spec's order of work puts before any IR change: a
standalone checker for a hand-written spatial contract against blocks already on
disk. It changes no IR, adds no node, emits nothing, and imports nothing from
`crates/grammar` — the walk predicates are ported into `voxels.py` so that this
prototype is able to disagree with the engine.

  python3 contract_check.py <manifest.json> <contract.json> [--list=N] [--json=PATH]
                            [--void=x0,y0,z0,x1,y1,z1] [--fill=…] [--physical-reach]

Exit codes follow `delve-grammar`: 0 pass, 2 input, 4 a gate went red.

# The contract (spec-PENDING §1, amended twice)

§0 is the governing rule and every check below traces to it: **an opt-out must
be secured by a property the defect cannot supply.** `sealed` demands its own
closure, `open` demands sky, `posted` demands an anchor, `via` demands its
endpoints' shared boundary, a merge demands one floor.

Boxes are inclusive `[x0,y0,z0,x1,y1,z1]` in zone-local coordinates.

    {
      "zone": "…", "entry": "<space>",
      "no_body_majority_ack": "<why, if the unowned share is the larger>",
      "spaces":  [{"name": "nave", "envelope": "enclosed", "boxes": [[…], […]]}],
      "no_body": [{"name": "rafters", "kind": "posted", "reason": "…",
                   "boxes": [[…]]}],
      "edges":   [{"a": "hall", "b": "crossing", "class": "walk", "rise": 0},
                  {"a": "foot", "b": "head", "class": "stair", "rise": 8,
                   "via": [[…]]},
                  {"a": "landing", "b": "pit", "class": "drop", "rise": -8},
                  {"a": "ward", "b": "cut", "class": "barred", "bar": […]},
                  {"a": "nave", "b": "exterior", "class": "walk"}]
    }

A space is the **union of its boxes** and is **one floor**: its standable cells
span at most 2 consecutive y-levels.

# The two walks, and why there are two

§2.5 (per-cell reachability) is **graph-confined**: cell by cell under the
ordinary step-and-fall, but crossing between declared regions only where an edge
licenses it, with `barred` closed and `drop` forward only. The spec pins this
reading; `--physical-reach` runs the rejected one so the difference stays
demonstrable rather than remembered.

§2.6 `sealed` and `posted` are proved against the *physical* walk, because they
are claims about what a body can do, not about what the graph says.
"""

import json
import sys
from collections import deque
from pathlib import Path

from voxels import Grid, STEPS

EXIT_INPUT = 2
EXIT_GATE = 4

ENVELOPES = ("enclosed", "open_top", "open")
CLASSES = ("walk", "stair", "drop", "barred", "vision")
TRAVERSAL = ("walk", "stair", "drop", "barred")
TRANSIT = ("stair", "drop")          # classes whose `via` is a transit volume
BIDIRECTIONAL = ("walk", "stair")
KINDS = ("sealed", "open", "posted")
EXTERIOR = "exterior"
POSTED_REACH = 2                      # §2.6 posted: Chebyshev radius from an anchor


# --- boxes -------------------------------------------------------------------

def in_box(box, c):
    x0, y0, z0, x1, y1, z1 = box
    return x0 <= c[0] <= x1 and y0 <= c[1] <= y1 and z0 <= c[2] <= z1


def in_any(boxes, c):
    return any(in_box(b, c) for b in boxes)


def volume(box):
    x0, y0, z0, x1, y1, z1 = box
    return (x1 - x0 + 1) * (y1 - y0 + 1) * (z1 - z0 + 1)


def cells_of(box):
    x0, y0, z0, x1, y1, z1 = box
    for x in range(x0, x1 + 1):
        for y in range(y0, y1 + 1):
            for z in range(z0, z1 + 1):
                yield (x, y, z)


def cells_over(boxes):
    for b in boxes:
        yield from cells_of(b)


def overlap(a, b):
    lo = [max(a[i], b[i]) for i in range(3)]
    hi = [min(a[i + 3], b[i + 3]) for i in range(3)]
    if any(lo[i] > hi[i] for i in range(3)):
        return None
    return lo + hi


def contains(outer, inner):
    return all(outer[i] <= inner[i] for i in range(3)) and \
           all(outer[i + 3] >= inner[i + 3] for i in range(3))


def grow(box, n=1):
    return [box[0] - n, box[1] - n, box[2] - n, box[3] + n, box[4] + n, box[5] + n]


def as_boxes(v):
    """`via` may be one box or a list of them."""
    if not v:
        return []
    return v if isinstance(v[0], list) else [v]


def shell(boxes, envelope="enclosed"):
    """Every cell face-adjacent to the union that is not itself in the union.
    `open_top` drops the +Y direction; `open` has no boundary obligation."""
    dirs = [(-1, 0, 0), (1, 0, 0), (0, -1, 0), (0, 0, -1), (0, 0, 1)]
    if envelope == "enclosed":
        dirs.append((0, 1, 0))
    out = set()
    for box in boxes:
        for c in cells_of(box):
            for d in dirs:
                n = (c[0] + d[0], c[1] + d[1], c[2] + d[2])
                if not in_any(boxes, n):
                    out.add(n)
    return out


# --- report ------------------------------------------------------------------

class Report:
    def __init__(self, zone, prefab_id):
        self.zone = zone
        self.prefab_id = prefab_id
        self.gates = []
        self.findings = []
        self.optouts = []

    def gate(self, gid, ok, bound, detail, cells=None):
        self.gates.append({"id": gid, "pass": bool(ok), "bound": bound,
                           "detail": detail, "cells": sorted(cells) if cells else []})

    def finding(self, text):
        self.findings.append(text)

    def optout(self, text):
        self.optouts.append(text)

    @property
    def failed(self):
        return [g for g in self.gates if not g["pass"]]

    def render(self, list_n):
        out = [f"spatial contract — {self.zone}", f"  prefab {self.prefab_id}", ""]
        width = max(len(g["id"]) for g in self.gates)
        for g in self.gates:
            mark = "pass" if g["pass"] else "RED "
            out.append(f"  {g['id']:<{width}}  {mark}  bound {g['bound']:<7} {g['detail']}")
            for c in g["cells"][:list_n]:
                out.append(f"      {c[0]},{c[1]},{c[2]}")
            if len(g["cells"]) > list_n:
                out.append(f"      ... {len(g['cells']) - list_n} more")
        out.append("")
        # §2.9: the verdict block enumerates every opt-out INSTANCE, always.
        out.append(f"  opt-outs taken ({len(self.optouts)}) — enumerated per instance, "
                   f"pass or fail:")
        for o in self.optouts or ["    (none)"]:
            out.append(f"    - {o}" if self.optouts else o)
        out.append("")
        if self.findings:
            out.append(f"  findings ({len(self.findings)}):")
            out += [f"    - {f}" for f in self.findings]
        else:
            out.append("  findings (0)")
        out.append("")
        red = self.failed
        if red:
            out.append(f"VERDICT: RED — {len(red)} of {len(self.gates)} obligations: "
                       + ", ".join(g["id"] for g in red))
        else:
            out.append(f"VERDICT: pass — {len(self.gates)} obligations, "
                       f"{len(self.optouts)} opt-out(s), {len(self.findings)} finding(s)")
        return "\n".join(out)


# --- the walks ---------------------------------------------------------------

def _flood(grid, cells, seeds, gate=None):
    seen = set(c for c in seeds if c in cells)
    q = deque(sorted(seen))
    while q:
        x, y, z = q.popleft()
        for dx, dz in STEPS:
            for dy in (0, 1, -1):
                n = (x + dx, y + dy, z + dz)
                if n in cells and n not in seen and (gate is None or gate((x, y, z), n)):
                    seen.add(n)
                    q.append(n)
            fy = y
            while y - fy <= 64:
                fy -= 1
                below = (x + dx, fy, z + dz)
                if grid.get(below) is None:
                    break
                if grid.solid(below):
                    landing = (x + dx, fy + 1, z + dz)
                    if landing in cells and landing not in seen \
                            and (gate is None or gate((x, y, z), landing)):
                        seen.add(landing)
                        q.append(landing)
                    break
    return seen


def physical_reach(grid, seeds):
    """Every standable cell a body can get to from `seeds` over the delivered
    blocks. Nothing declared constrains it."""
    return _flood(grid, grid.standable_cells(), seeds)


def physical_reach_within(grid, cells, seeds):
    return _flood(grid, cells, seeds)


def step_only(cells, seeds):
    """`nav::connected`: the plain +/-1 step, no fall."""
    seen = set(c for c in seeds if c in cells)
    q = deque(sorted(seen))
    while q:
        x, y, z = q.popleft()
        for dx, dz in STEPS:
            for dy in (0, 1, -1):
                n = (x + dx, y + dy, z + dz)
                if n in cells and n not in seen:
                    seen.add(n)
                    q.append(n)
    return seen


def graph_reach(grid, walkable, owner, crossings, seeds):
    """§2.5: cell by cell, crossing between declared regions only where an edge
    licenses that ordered pair."""
    def gate(a, b):
        oa, ob = owner[a], owner[b]
        return oa == ob or (oa, ob) in crossings
    return _flood(grid, walkable, seeds, gate)


# --- the checker -------------------------------------------------------------

def check(grid, manifest, contract, report, physical_25=False, literal_targets=False):
    spaces = {s["name"]: s for s in contract.get("spaces", [])}
    nobody = {n["name"]: n for n in contract.get("no_body", [])}
    edges = contract.get("edges", [])
    entry = contract.get("entry")
    ack = contract.get("no_body_majority_ack")
    anchors = manifest.get("anchors", {})
    standable = grid.standable_cells()

    sboxes = {n: s["boxes"] for n, s in spaces.items()}
    nboxes = {n: r["boxes"] for n, r in nobody.items()}
    transit = {i: as_boxes(e.get("via")) for i, e in enumerate(edges)
               if e.get("class") in TRANSIT and e.get("via")}

    def space_cells(name):
        return set(c for c in standable if in_any(sboxes[name], c))

    def nested_cells():
        return set(c for c in standable
                   if any(in_any(bs, c) for bs in nboxes.values()))

    nested = nested_cells()

    # --- 1. well-formed ------------------------------------------------------
    problems = []
    examined = len(spaces) + len(nobody) + len(edges)
    for name, s in sorted(spaces.items()):
        if s.get("envelope") not in ENVELOPES:
            problems.append(f"space {name}: envelope {s.get('envelope')!r} not in {ENVELOPES}")
        if not s.get("boxes"):
            problems.append(f"space {name}: declares no box")
    for name, r in sorted(nobody.items()):
        if r.get("kind") not in KINDS:
            problems.append(f"no_body {name}: kind {r.get('kind')!r} not in {KINDS}")
        if not (r.get("reason") or "").strip():
            problems.append(f"no_body {name}: a non-empty `reason` is required")
    if entry not in spaces:
        problems.append(f"entry {entry!r} is not a declared space")
    else:
        if not any(EXTERIOR in (e["a"], e["b"]) and entry in (e["a"], e["b"])
                   and e.get("class") in TRAVERSAL for e in edges):
            problems.append(f"entry {entry!r} carries no exterior edge of a traversal class "
                            f"— the piece does not claim to be enterable where it says")
    if set(spaces) & set(nobody):
        problems.append("declared as both a space and a no_body region: "
                        + ", ".join(sorted(set(spaces) & set(nobody))))

    # one floor per space (§1a, the merge rule)
    for name in sorted(spaces):
        cells = space_cells(name) - nested
        ys = sorted({c[1] for c in cells})
        if ys and (max(ys) - min(ys) + 1) > 2:
            problems.append(f"space {name}: standable cells span y {min(ys)}..{max(ys)} "
                            f"({max(ys) - min(ys) + 1} levels) — a space is one floor, so this "
                            f"is two places and a transition, and a transition is an edge")

    names = sorted(spaces)
    for i in range(len(names)):
        for j in range(i + 1, len(names)):
            na, nb = names[i], names[j]
            for ba in sboxes[na]:
                for bb in sboxes[nb]:
                    ov = overlap(ba, bb)
                    if ov is not None:
                        problems.append(f"spaces {na} and {nb} overlap in {volume(ov)} cell(s) "
                                        f"— two spaces may abut, never overlap")
    for rn, rbs in sorted(nboxes.items()):
        for rb in rbs:
            hosts = [sn for sn, sbs in sorted(sboxes.items())
                     if any(contains(sb, rb) for sb in sbs)]
            touched = [sn for sn, sbs in sorted(sboxes.items())
                       if any(overlap(sb, rb) for sb in sbs)]
            if touched and not hosts:
                problems.append(f"no_body {rn}: overlaps {', '.join(touched)} without nesting "
                                f"wholly inside any one space")
            if len(hosts) > 1:
                problems.append(f"no_body {rn}: nests inside more than one space")
        for rn2 in sorted(nboxes):
            if rn2 <= rn:
                continue
            for ba in rbs:
                for bb in nboxes[rn2]:
                    if overlap(ba, bb) is not None:
                        problems.append(f"no_body {rn} and {rn2} overlap")

    for idx, e in enumerate(edges):
        cls, a, b = e.get("class"), e["a"], e["b"]
        lbl = f"edge {a}-{b}"
        for side in ("a", "b"):
            if e[side] != EXTERIOR and e[side] not in spaces:
                problems.append(f"{lbl}: {e[side]!r} is neither a declared space nor {EXTERIOR!r}")
        if cls not in CLASSES:
            problems.append(f"{lbl}: class {cls!r} is not one of {CLASSES}")
            continue
        if cls == "barred" and "bar" not in e:
            problems.append(f"{lbl}: a barred edge must declare `bar`")
        if cls != "barred" and "bar" in e:
            problems.append(f"{lbl}: only a barred edge may declare `bar`")
        vias = as_boxes(e.get("via"))
        # --- `rise` per class (§1a) ---
        ext = EXTERIOR in (a, b)
        if cls == "vision" and "rise" in e:
            problems.append(f"{lbl}: `rise` is meaningless on a vision edge and is refused")
        if ext and "rise" in e:
            problems.append(f"{lbl}: `rise` is refused on an exterior edge — exterior has no "
                            f"resolved box to measure against")
        if not ext:
            if cls == "stair":
                if "rise" not in e:
                    problems.append(f"{lbl}: a stair must declare its `rise`")
                elif e["rise"] < 1:
                    problems.append(f"{lbl}: a stair's declared rise {e['rise']} must be >= 1")
            if cls == "drop":
                if "rise" not in e:
                    problems.append(f"{lbl}: a drop must declare its `rise`")
                elif e["rise"] > -1:
                    problems.append(f"{lbl}: a drop's declared rise {e['rise']} must be <= -1")
        # --- `via` per class (§1a, §0) ---
        if cls == "stair" and not vias:
            problems.append(f"{lbl}: a stair must declare its `via` — the run's cells belong "
                            f"to the edge, and something must own them")
        if vias:
            if cls in TRANSIT:
                # a transit volume: disjoint from every space, abutting both endpoints
                for v in vias:
                    for sn, sbs in sorted(sboxes.items()):
                        if any(overlap(sb, v) for sb in sbs):
                            problems.append(f"{lbl}: transit volume overlaps space {sn} — a "
                                            f"transit volume is disjoint from every space")
                for side in (a, b):
                    if side == EXTERIOR:
                        continue
                    if not any(overlap(grow(v), sb) for v in vias for sb in sboxes[side]):
                        problems.append(f"{lbl}: transit volume does not abut {side}")
            else:
                # on the shared boundary of the two endpoints (§0's constraint on via)
                for v in vias:
                    for c in cells_of(v):
                        okay = True
                        for side in (a, b):
                            if side == EXTERIOR:
                                if not (c[0] in (0, grid.w - 1) or c[1] in (0, grid.h - 1)
                                        or c[2] in (0, grid.d - 1)):
                                    okay = False
                            elif not any(in_box(grow(sb), c) for sb in sboxes[side]):
                                okay = False
                        if not okay:
                            problems.append(f"{lbl}: via cell {c[0]},{c[1]},{c[2]} does not lie "
                                            f"on the shared boundary of its endpoints")
                            break
        if cls == "barred" and "bar" in e:
            if not any(overlap(grow(sb), e["bar"])
                       for side in (a, b) if side != EXTERIOR for sb in sboxes[side]):
                problems.append(f"{lbl}: bar region touches neither endpoint")
    report.gate("well-formed", not problems, examined,
                f"{len(spaces)} space(s) in {sum(len(v) for v in sboxes.values())} box(es), "
                f"{len(nobody)} no_body region(s), {len(edges)} edge(s), "
                f"{len(transit)} transit volume(s)"
                + ("" if not problems else "; " + "; ".join(problems[:6])
                   + (f"; ... {len(problems)-6} more" if len(problems) > 6 else "")))

    # --- 2. coverage ---------------------------------------------------------
    all_transit = [b for bs in transit.values() for b in bs]
    in_space, in_nobody, in_transit, uncovered = set(), set(), set(), set()
    for c in standable:
        if any(in_any(bs, c) for bs in nboxes.values()):
            in_nobody.add(c)
        elif any(in_any(bs, c) for bs in sboxes.values()):
            in_space.add(c)
        elif in_any(all_transit, c):
            in_transit.add(c)
        else:
            uncovered.add(c)
    owned = in_space | in_transit
    unowned = in_nobody
    report.gate("coverage", not uncovered, len(standable),
                f"{len(standable)} standable cell(s): {len(in_space)} in a space, "
                f"{len(in_transit)} in a transit volume, {len(unowned)} declared out of play, "
                f"{len(uncovered)} undeclared", cells=uncovered)

    # --- 3. closure (with §0's envelope-sky rule) ----------------------------
    openings = [b for e in edges for b in as_boxes(e.get("via"))
                if e.get("class") not in TRANSIT] + \
               [e["bar"] for e in edges if e.get("class") == "barred"]
    sky_problems = set()
    for name, s in sorted(spaces.items()):
        if s["envelope"] == "enclosed":
            continue
        roofed = set(c for c in space_cells(name) - nested if not grid.open_to_sky(c))
        if roofed:
            sky_problems.add((name, s["envelope"], len(roofed)))
        report.optout(f"envelope `{s['envelope']}` on space {name} "
                      f"({len(space_cells(name) - nested)} standable cell(s), "
                      f"{len(roofed)} of them roofed)")
    boundary_examined, breaches = 0, set()
    closed = [(n, s) for n, s in sorted(spaces.items()) if s["envelope"] != "open"]
    for name, s in closed:
        others = [bs for n2, bs in sorted(sboxes.items()) if n2 != name]
        for c in shell(s["boxes"], s["envelope"]):
            boundary_examined += 1
            if not grid.passable(c):
                continue
            if any(in_any(bs, c) for bs in others):
                continue
            if any(in_any(bs, c) for bs in nboxes.values()):
                continue
            if in_any(openings, c) or in_any(all_transit, c):
                continue
            breaches.add(c)
    detail = (f"{len(closed)} space(s) with a closed envelope, {boundary_examined} "
              f"boundary cell(s) examined, {len(breaches)} unexplained passable")
    if sky_problems:
        detail += "; ENVELOPE WITHOUT SKY: " + "; ".join(
            f"{n} is `{env}` but {k} of its standable cell(s) have artifact solid above"
            for n, env, k in sorted(sky_problems))
    report.gate("closure", not breaches and not sky_problems, boundary_examined,
                detail, cells=breaches)

    # --- 4. edge proof -------------------------------------------------------
    def side_cells(name, e):
        if name != EXTERIOR:
            return set(c for c in standable if in_any(sboxes[name], c))
        v = as_boxes(e.get("via"))
        if v:
            return set(c for c in standable if in_any(v, c))
        return set(c for c in standable
                   if c[0] in (0, grid.w - 1) or c[2] in (0, grid.d - 1))

    def arena(e):
        cells = side_cells(e["a"], e) | side_cells(e["b"], e)
        for key in ("via", "bar"):
            if key in e:
                cells |= set(c for c in standable if in_any(as_boxes(e[key]), c))
        return cells

    failures, proved, rises = [], 0, 0
    for e in edges:
        cls, a, b = e["class"], e["a"], e["b"]
        label = f"{a} -{cls}- {b}"
        A, B, cells = side_cells(a, e), side_cells(b, e), arena(e)
        if cls == "vision":
            for v in as_boxes(e.get("via")):
                report.optout(f"vision via on {label}: {volume(v)} cell(s)")
            continue
        if not A or not B:
            failures.append(f"{label}: {'a' if not A else 'b'} side has no standable cell")
            continue
        proved += 1
        measure_on = (A, B)
        if cls in BIDIRECTIONAL:
            fwd, bwd = bool(step_only(cells, A) & B), bool(step_only(cells, B) & A)
            if not (fwd and bwd):
                failures.append(f"{label}: the plain walk does not connect "
                                f"{'a to b' if not fwd else 'b to a'}")
        elif cls == "drop":
            if not (physical_reach_within(grid, cells, A) & B):
                failures.append(f"{label}: b is not reachable from a under walk-and-fall")
            if step_only(cells, B) & A:
                failures.append(f"{label}: a IS reachable back from b under the plain step "
                                f"— a drop is one-way")
        elif cls == "barred":
            if step_only(cells, A) & B:
                failures.append(f"{label}: connected while the bar stands")
            voided = Grid((grid.w, grid.h, grid.d),
                          {k: v for k, v in grid.blocks.items() if not in_box(e["bar"], k)})
            vs = voided.standable_cells()
            vA = set(c for c in vs if in_any(sboxes[a], c)) if a != EXTERIOR else A
            vB = set(c for c in vs if in_any(sboxes[b], c)) if b != EXTERIOR else B
            varena = vA | vB | set(c for c in vs if in_box(e["bar"], c))
            if not (step_only(varena, vA) & vB):
                failures.append(f"{label}: still not connected with the bar region voided")
            measure_on = (vA, vB)          # §1a: barred's rise is read on the voided copy
        if EXTERIOR not in (a, b):
            declared = e.get("rise", 0 if cls in ("walk", "barred") else None)
            if declared is not None:
                rises += 1
                mA, mB = measure_on
                if mA and mB:
                    measured = min(c[1] for c in mB) - min(c[1] for c in mA)
                    if measured != declared:
                        failures.append(f"{label}: declared rise {declared}, "
                                        f"measured {measured}")
    report.gate("edge-proof", not failures, proved,
                f"{proved} edge(s) proved of {len(edges)} declared, {rises} carrying a "
                f"level relation"
                + ("" if not failures else "; " + "; ".join(failures[:8])
                   + (f"; ... {len(failures)-8} more" if len(failures) > 8 else "")))

    # --- 5. reachability, per cell, graph-confined ---------------------------
    owner = {}
    for c in standable:
        for n, bs in sorted(sboxes.items()):
            if in_any(bs, c):
                owner[c] = n
                break
    for i, bs in sorted(transit.items()):
        for c in standable:
            if c not in owner and in_any(bs, c):
                owner[c] = f"@via{i}"
    walkable = set(owner) - nested
    # §2.5 says "every standable cell of every declared SPACE". Read literally,
    # a transit volume's cells are not targets — which makes a padded transit
    # volume a total exemption (see cheat2.py, attack F). This prototype makes
    # them targets by default, because a cell a body can stand in is a cell the
    # contract owes an account of; `--literal-targets` runs the spec's words.
    targets = set(c for c in walkable if not owner[c].startswith("@")) if literal_targets \
        else set(walkable)
    entry_cells = set(c for c in walkable if owner.get(c) == entry) if entry in spaces else set()

    def crossings_for(open_bars):
        out = set()
        for i, e in enumerate(edges):
            if EXTERIOR in (e["a"], e["b"]) or e["class"] == "vision":
                continue
            if e["class"] == "barred" and not open_bars:
                continue
            hops = [(e["a"], e["b"])]
            if i in transit:
                hops = [(e["a"], f"@via{i}"), (f"@via{i}", e["b"])]
            for x, y in hops:
                out.add((x, y))
                if e["class"] != "drop":
                    out.add((y, x))
        return out

    if entry_cells:
        if physical_25:
            reached = physical_reach(grid, entry_cells) & targets
        else:
            reached = graph_reach(grid, walkable, owner, crossings_for(False), entry_cells)
        bars = [e["bar"] for e in edges if e.get("class") == "barred"]
        if bars:
            opened = Grid((grid.w, grid.h, grid.d),
                          {k: v for k, v in grid.blocks.items()
                           if not any(in_box(bb, k) for bb in bars)})
            ostand = opened.standable_cells()
            oowner = {}
            for c in ostand:
                for n, bs in sorted(sboxes.items()):
                    if in_any(bs, c):
                        oowner[c] = n
                        break
                else:
                    if any(in_box(bb, c) for bb in bars) or in_any(all_transit, c):
                        oowner[c] = "@bar"
            owalk = set(oowner) - nested
            ocross = crossings_for(True) | {(n, "@bar") for n in list(spaces) + ["@bar"]} \
                                         | {("@bar", n) for n in list(spaces) + ["@bar"]}
            with_bars = graph_reach(opened, owalk, oowner, ocross, entry_cells)
            report.optout(f"bars opened for the stratified walk: "
                          + ", ".join(f"{e['a']}-{e['b']}" for e in edges
                                      if e.get("class") == "barred"))
        else:
            with_bars = reached
        gated = (targets - reached) & with_bars
        never = (targets - reached) - with_bars
        per_space = {}
        for c in never:
            per_space[owner[c]] = per_space.get(owner[c], 0) + 1
        detail = (f"{len(targets)} cell(s) after nested no_body, {len(reached)} reached from "
                  f"{entry!r} with bars standing, {len(gated)} more once bars open, "
                  f"{len(never)} unreached")
        if gated:
            detail += "; behind a bar: " + ", ".join(sorted({owner[c] for c in gated}))
        if per_space:
            detail += "; unreached per space: " + ", ".join(
                f"{k}={v}" for k, v in sorted(per_space.items()))
        report.gate("reachability", not never, len(targets), detail,
                    cells=(never if never else None))
    else:
        report.gate("reachability", False, 0, "no declared entry with a standable cell")
        reached = set()

    # --- 6. the no_body obligation, three kinds ------------------------------
    phys = physical_reach(grid, entry_cells) if entry_cells else set()
    nb_problems, nb_cells, bad = [], 0, set()
    counts = {k: 0 for k in KINDS}
    sealed_boxes = [b for n, r in sorted(nobody.items()) if r.get("kind") == "sealed"
                    for b in r["boxes"]]
    for name, r in sorted(nobody.items()):
        cells = set(c for c in standable if in_any(r["boxes"], c))
        nb_cells += len(cells)
        kind = r.get("kind")
        if kind in counts:
            counts[kind] += 1
        if kind == "sealed":
            # §0: the defect supplies "unreached"; it does NOT supply "walled off".
            leaks = set(c for c in shell(r["boxes"]) if grid.passable(c)
                        and not in_any(sealed_boxes, c))
            if leaks:
                nb_problems.append(f"{name}: {len(leaks)} boundary cell(s) are passable — "
                                   f"`sealed` claims walled off, not merely unreached")
                bad |= leaks
        elif kind == "open":
            roofed = set(c for c in cells if not grid.open_to_sky(c))
            if roofed:
                nb_problems.append(f"{name}: {len(roofed)} standable cell(s) with artifact "
                                   f"solid above them — not sky-open decoration")
                bad |= roofed
        elif kind == "posted":
            inside = {an: tuple(anchors[an]["pos"]) for an in sorted(anchors)
                      if in_any(r["boxes"], tuple(anchors[an]["pos"]))}
            if not inside:
                nb_problems.append(f"{name}: `posted` but contains no declared anchor")
            else:
                far = set(c for c in cells
                          if not any(max(abs(c[i] - p[i]) for i in range(3)) <= POSTED_REACH
                                     for p in inside.values()))
                if far:
                    nb_problems.append(f"{name}: {len(far)} standable cell(s) further than "
                                       f"Chebyshev {POSTED_REACH} from any anchor in the region")
                    bad |= far
            report.optout(f"posted region {name}: {len(cells)} standable cell(s) served by "
                          f"{len(inside)} anchor(s) [" + ", ".join(sorted(inside)) + "]")
    report.gate("no-body", not nb_problems, len(nobody),
                f"{len(nobody)} region(s) ("
                + ", ".join(f"{v} {k}" for k, v in sorted(counts.items()) if v)
                + f") over {nb_cells} standable cell(s)"
                + ("" if not nb_problems else "; " + "; ".join(nb_problems[:6])
                   + (f"; ... {len(nb_problems)-6} more" if len(nb_problems) > 6 else "")),
                cells=bad)

    # --- 7. anchors ----------------------------------------------------------
    stray, by_kind = [], {}
    for name in sorted(anchors):
        p = tuple(anchors[name]["pos"])
        kind = None
        for rn, bs in sorted(nboxes.items()):
            if in_any(bs, p):
                kind = f"no_body/{nobody[rn].get('kind')}"
                if nobody[rn].get("kind") != "posted":
                    report.finding(f"anchor {name} resolves to a `{nobody[rn].get('kind')}` "
                                   f"no_body region, not to play space")
                break
        if kind is None:
            for sn, bs in sorted(sboxes.items()):
                if any(in_box(grow(bb), p) for bb in bs):
                    kind = "space"
                    break
        if kind is None:
            for e in edges:
                for key in ("via", "bar"):
                    if key in e and in_any(as_boxes(e[key]), p):
                        kind = "edge-region"
        if kind is None:
            stray.append(name)
        else:
            by_kind[kind] = by_kind.get(kind, 0) + 1
    report.gate("anchors", not stray, len(anchors),
                f"{len(anchors)} anchor(s) ("
                + ", ".join(f"{v} {k}" for k, v in sorted(by_kind.items())) + ")"
                + ("" if not stray else f"; {len(stray)} resolving to nothing: "
                                        + ", ".join(stray)))

    # --- 8. exterior faces ---------------------------------------------------
    ext = [e for e in edges if EXTERIOR in (e["a"], e["b"])]
    ext_spaces = sorted({e["a"] if e["b"] == EXTERIOR else e["b"] for e in ext})
    report.gate("exterior-faces", len(ext_spaces) >= 2, len(ext),
                f"{len(ext)} exterior edge(s) touching {len(ext_spaces)} space(s)"
                + (f": {', '.join(ext_spaces)}" if ext_spaces else "")
                + ("" if len(ext_spaces) >= 2 else
                   " — a piece with fewer than two exterior faces makes no traversable claim"))

    # --- 9. vacuity reds -----------------------------------------------------
    zeros = [g["id"] for g in report.gates
             if g["id"] in ("closure", "edge-proof", "reachability") and g["bound"] == 0]
    majority = len(unowned) > len(owned)
    vac = []
    if zeros:
        vac.append("ZERO BINDING on " + ", ".join(zeros) + " — those gates examined nothing")
    if majority and not ack:
        vac.append(f"no_body MAJORITY: {len(unowned)} of {len(standable)} standable cell(s) are "
                   f"declared out of play against {len(owned)} owned, and the contract carries "
                   f"no `no_body_majority_ack`")
    report.gate("vacuity", not vac, len(report.gates) - 1,
                ("no vacuous binding" if not vac else "; ".join(vac)))
    if majority and ack:
        report.finding(f"no_body majority ({len(unowned)} of {len(standable)}) acknowledged: "
                       f"{ack} — the acknowledgement does not weaken §2.6, which still binds "
                       f"every region")
    if len(spaces) == 1 and not edges:
        report.finding("1 space, 0 edges — the contract claims nothing about how a body moves")


def main(argv):
    args = [a for a in argv[1:] if not a.startswith("--")]
    list_n, json_out, perturb, physical_25, literal_targets = 12, None, [], False, False
    for f in [a for a in argv[1:] if a.startswith("--")]:
        if f.startswith("--list="):
            list_n = int(f.split("=", 1)[1])
        elif f.startswith("--json="):
            json_out = f.split("=", 1)[1]
        elif f == "--physical-reach":
            physical_25 = True
        elif f == "--literal-targets":
            literal_targets = True
        elif f.startswith("--void=") or f.startswith("--fill="):
            kind, spec = f[2:].split("=", 1)
            perturb.append((kind, [int(v) for v in spec.split(",")]))
        else:
            print(f"unknown flag {f}")
            return EXIT_INPUT
    if len(args) != 2:
        print(__doc__)
        return EXIT_INPUT
    grid, manifest = Grid.from_manifest(args[0])
    for kind, box in perturb:
        for c in cells_of(box):
            if not grid.inside(c):
                continue
            if kind == "void":
                grid.blocks.pop(c, None)
            else:
                grid.blocks[c] = "minecraft:stone"
        grid._standable = None
    contract = json.loads(Path(args[1]).read_text())
    report = Report(contract.get("zone", "unnamed"), manifest.get("prefab_id", "?"))
    for kind, box in perturb:
        report.finding(f"ARTIFACT PERTURBED: {kind} {','.join(str(v) for v in box)}")
    if literal_targets:
        report.finding("reachability targets restricted to cells of declared SPACES — §2.5's "
                       "literal words, which exempt every transit volume")
    if physical_25:
        report.finding("reachability run under the PHYSICAL reading of §2.5, which the spec "
                       "rejects — for comparison only")
    check(grid, manifest, contract, report, physical_25, literal_targets)
    print(report.render(list_n))
    if json_out:
        Path(json_out).write_text(json.dumps(
            {"zone": report.zone, "gates": report.gates, "optouts": report.optouts,
             "findings": report.findings}, indent=1, sort_keys=True) + "\n")
    return EXIT_GATE if report.failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
