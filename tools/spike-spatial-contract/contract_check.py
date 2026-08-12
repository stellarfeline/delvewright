#!/usr/bin/env python3
"""SPIKE TOOLING (ADR-PENDING-map-design-pipeline, spec-PENDING §4.1) — NOT part
of the shipped pipeline, and deliberately not in a crate.

The **cheap falsifier** the spec's order of work puts before any IR change: a
standalone checker for a hand-written spatial contract against blocks already on
disk. It changes no IR, adds no node, emits nothing, and imports nothing from
`crates/grammar` — the walk predicates are ported into `voxels.py` so that this
prototype is able to disagree with the engine.

  python3 contract_check.py <manifest.json> <contract.json> [--list=N] [--json=PATH]
                            [--void=x0,y0,z0,x1,y1,z1] [--fill=…]

Exit codes follow `delve-grammar`: 0 pass, 2 input, 4 a gate went red.

# The contract (spec-PENDING §1, amended 2026-08-12)

Boxes are inclusive `[x0,y0,z0,x1,y1,z1]` in zone-local coordinates.

    {
      "zone": "<what this is a contract for>",
      "entry": "<space name>",
      "no_body_majority_ack": "<why, if the unowned share is the larger>",
      "spaces": [
        {"name": "nave", "envelope": "enclosed", "boxes": [[…], […]]}
      ],
      "no_body": [
        {"name": "loft-rafters", "kind": "sealed", "reason": "…", "boxes": [[…]]}
      ],
      "edges": [
        {"a": "hall", "b": "crossing", "class": "walk", "rise": 0},
        {"a": "foot", "b": "head", "class": "stair", "rise": 8},
        {"a": "landing", "b": "pit", "class": "drop", "rise": -8},
        {"a": "ward", "b": "shortcut", "class": "barred", "bar": […]},
        {"a": "nave", "b": "exterior", "class": "walk"}
      ]
    }

A space is the **union of its boxes** — the amended §1a, which is what lets a
stepped cross-section be one space. Two different spaces never overlap; a
`no_body` region may nest wholly inside one space.

# The two walks, and why there are two

§2.5 (per-cell reachability) asks whether the declared graph *delivers* what it
claims, so its walk is **graph-confined**: it moves cell by cell under the
ordinary step-and-fall, but crosses from one space to another only where a
declared edge licenses that crossing, with `barred` closed and `drop` forward
only. A purely physical walk would make §2.5 independent of the edges — under
it, deleting Z7's stair edge changes nothing and AC11 cannot hold.

§2.6 (`sealed`) asks whether a body can *physically* get somewhere the author
said it cannot, so its walk is **physical**: every standable cell in the
artifact, every bar opened. A graph-confined walk can never enter a `no_body`
region at all, so it would pass §2.6 vacuously.

The spec says only "the voxel walk" for §2.5. That choice is recorded here
because it is a real fork, not an implementation detail: `--physical-reach`
runs §2.5 under the other reading so the two can be compared on the same
artifact.
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
KINDS = ("sealed", "open")
EXTERIOR = "exterior"
BIDIRECTIONAL = ("walk", "stair")


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


def shell(boxes, envelope):
    """The boundary of a union-of-boxes space: every cell face-adjacent to the
    union that is not itself in the union. `open_top` drops the +Y direction;
    `open` has no boundary obligation."""
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

    def gate(self, gid, ok, bound, detail, cells=None):
        self.gates.append({"id": gid, "pass": bool(ok), "bound": bound,
                           "detail": detail, "cells": sorted(cells) if cells else []})

    def finding(self, text):
        self.findings.append(text)

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
                       f"{len(self.findings)} finding(s)")
        return "\n".join(out)


# --- the walks ---------------------------------------------------------------

def physical_reach(grid, seeds):
    """Every standable cell a body can get to from `seeds` over the delivered
    blocks: the +/-1 step plus the one-way fall. Nothing declared constrains it."""
    cells = grid.standable_cells()
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
            fy = y
            while y - fy <= 64:
                fy -= 1
                below = (x + dx, fy, z + dz)
                if grid.get(below) is None:
                    break
                if grid.solid(below):
                    landing = (x + dx, fy + 1, z + dz)
                    if landing in cells and landing not in seen:
                        seen.add(landing)
                        q.append(landing)
                    break
    return seen


def graph_reach(grid, walkable, owner, crossings, seeds):
    """§2.5's walk: cell by cell, but a step that leaves one space for another is
    taken only where `crossings` licenses that ordered pair."""
    seen = set(c for c in seeds if c in walkable)
    q = deque(sorted(seen))

    def allowed(a, b):
        sa, sb = owner[a], owner[b]
        return sa == sb or (sa, sb) in crossings

    while q:
        x, y, z = q.popleft()
        for dx, dz in STEPS:
            for dy in (0, 1, -1):
                n = (x + dx, y + dy, z + dz)
                if n in walkable and n not in seen and allowed((x, y, z), n):
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
                    if landing in walkable and landing not in seen \
                            and allowed((x, y, z), landing):
                        seen.add(landing)
                        q.append(landing)
                    break
    return seen


# --- the checker -------------------------------------------------------------

def check(grid, manifest, contract, report, physical_25=False):
    spaces = {s["name"]: s for s in contract.get("spaces", [])}
    nobody = {n["name"]: n for n in contract.get("no_body", [])}
    edges = contract.get("edges", [])
    entry = contract.get("entry")
    ack = contract.get("no_body_majority_ack")
    standable = grid.standable_cells()

    sboxes = {n: s["boxes"] for n, s in spaces.items()}
    nboxes = {n: r["boxes"] for n, r in nobody.items()}

    # --- 1. well-formed ------------------------------------------------------
    problems = []
    examined = len(spaces) + len(nobody) + len(edges)
    for name, s in sorted(spaces.items()):
        if s.get("envelope") not in ENVELOPES:
            problems.append(f"space {name}: envelope {s.get('envelope')!r} is not one of {ENVELOPES}")
        if not s.get("boxes"):
            problems.append(f"space {name}: declares no box")
    for name, r in sorted(nobody.items()):
        if r.get("kind") not in KINDS:
            problems.append(f"no_body {name}: kind {r.get('kind')!r} is not one of {KINDS}")
        if not (r.get("reason") or "").strip():
            problems.append(f"no_body {name}: a non-empty `reason` is required")
    if entry not in spaces:
        problems.append(f"entry {entry!r} is not a declared space")
    if set(spaces) & set(nobody):
        problems.append("declared as both a space and a no_body region: "
                        + ", ".join(sorted(set(spaces) & set(nobody))))
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
                problems.append(f"no_body {rn}: nests inside more than one space "
                                f"({', '.join(hosts)})")
        for rn2 in sorted(nboxes):
            if rn2 <= rn:
                continue
            for ba in rbs:
                for bb in nboxes[rn2]:
                    if overlap(ba, bb) is not None:
                        problems.append(f"no_body {rn} and {rn2} overlap")
    for e in edges:
        cls, a, b = e.get("class"), e["a"], e["b"]
        lbl = f"edge {a}-{b}"
        for side in ("a", "b"):
            if e[side] != EXTERIOR and e[side] not in spaces:
                problems.append(f"{lbl}: {e[side]!r} is neither a declared space nor {EXTERIOR!r}")
        if cls not in CLASSES:
            problems.append(f"{lbl}: class {cls!r} is not one of {CLASSES}")
        if cls == "barred" and "bar" not in e:
            problems.append(f"{lbl}: a barred edge must declare `bar`")
        if cls != "barred" and "bar" in e:
            problems.append(f"{lbl}: only a barred edge may declare `bar`")
        # §1a's rise obligations, per class
        if cls == "vision" and "rise" in e:
            problems.append(f"{lbl}: `rise` is meaningless on a vision edge and is refused")
        if EXTERIOR not in (a, b):
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
        if cls == "barred" and "bar" in e:
            near = False
            for side in (a, b):
                if side == EXTERIOR:
                    continue
                if any(overlap(grow(sb), e["bar"]) for sb in sboxes[side]):
                    near = True
            if not near:
                problems.append(f"{lbl}: bar region touches neither endpoint")
    report.gate("well-formed", not problems, examined,
                f"{len(spaces)} space(s) in {sum(len(v) for v in sboxes.values())} box(es), "
                f"{len(nobody)} no_body region(s), {len(edges)} edge(s)"
                + ("" if not problems else "; " + "; ".join(problems[:6])
                   + (f"; ... {len(problems)-6} more" if len(problems) > 6 else "")))

    # --- 2. coverage ---------------------------------------------------------
    in_space, in_nobody, uncovered = set(), set(), set()
    for c in standable:
        if any(in_any(bs, c) for bs in sboxes.values()):
            in_space.add(c)
        elif any(in_any(bs, c) for bs in nboxes.values()):
            in_nobody.add(c)
        else:
            uncovered.add(c)
    # a nested region's cells are inside a space too; count them as unowned
    nested = set(c for c in in_space if any(in_any(bs, c) for bs in nboxes.values()))
    owned = in_space - nested
    unowned = in_nobody | nested
    report.gate("coverage", not uncovered, len(standable),
                f"{len(standable)} standable cell(s): {len(owned)} owned by a space, "
                f"{len(unowned)} declared out of play, {len(uncovered)} undeclared",
                cells=uncovered)

    # --- 3. closure ----------------------------------------------------------
    openings = [e["via"] for e in edges if "via" in e] + \
               [e["bar"] for e in edges if e.get("class") == "barred"]
    boundary_examined, breaches = 0, set()
    closed = [(n, s) for n, s in sorted(spaces.items()) if s["envelope"] != "open"]
    for name, s in closed:
        others = [bs for n2, bs in sorted(sboxes.items()) if n2 != name]
        for c in shell(s["boxes"], s["envelope"]):
            boundary_examined += 1
            if not grid.passable(c):
                continue
            if any(in_any(bs, c) for bs in others):
                continue                                   # abutting declared space
            if any(in_any(bs, c) for bs in nboxes.values()):
                continue                                   # abutting no_body region
            if any(in_box(v, c) for v in openings):
                continue                                   # a declared opening
            breaches.add(c)
    report.gate("closure", not breaches, boundary_examined,
                f"{len(closed)} space(s) with a closed envelope, {boundary_examined} "
                f"boundary cell(s) examined, {len(breaches)} unexplained passable",
                cells=breaches)

    # --- 4. edge proof (with the declared level relation) --------------------
    def side_cells(name, e):
        if name != EXTERIOR:
            return set(c for c in standable if in_any(sboxes[name], c))
        if "via" in e:
            return set(c for c in standable if in_box(e["via"], c))
        return set(c for c in standable
                   if c[0] in (0, grid.w - 1) or c[2] in (0, grid.d - 1))

    def arena(e):
        cells = set()
        for name in (e["a"], e["b"]):
            cells |= side_cells(name, e)
        for key in ("via", "bar"):
            if key in e:
                cells |= set(c for c in standable if in_box(e[key], c))
        return cells

    def step_reach(cells, seeds):
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

    def fall_reach(cells, seeds):
        return physical_reach_within(grid, cells, seeds)

    failures, proved, rises = [], 0, 0
    for e in edges:
        cls, a, b = e["class"], e["a"], e["b"]
        label = f"{a} -{cls}- {b}"
        A, B, cells = side_cells(a, e), side_cells(b, e), arena(e)
        if not A or not B:
            failures.append(f"{label}: {'a' if not A else 'b'} side has no standable cell")
            continue
        proved += 1
        if cls in BIDIRECTIONAL:
            fwd, bwd = bool(step_reach(cells, A) & B), bool(step_reach(cells, B) & A)
            if not (fwd and bwd):
                failures.append(f"{label}: the plain walk does not connect "
                                f"{'a to b' if not fwd else 'b to a'}")
        elif cls == "drop":
            if not (fall_reach(cells, A) & B):
                failures.append(f"{label}: b is not reachable from a under walk-and-fall")
            if step_reach(cells, B) & A:
                failures.append(f"{label}: a IS reachable back from b under the plain step "
                                f"— a drop is one-way")
        elif cls == "barred":
            if step_reach(cells, A) & B:
                failures.append(f"{label}: connected while the bar stands")
            voided = Grid((grid.w, grid.h, grid.d),
                          {k: v for k, v in grid.blocks.items() if not in_box(e["bar"], k)})
            vs = voided.standable_cells()
            vA = set(c for c in vs if in_any(sboxes[a], c)) if a != EXTERIOR else A
            vB = set(c for c in vs if in_any(sboxes[b], c)) if b != EXTERIOR else B
            varena = vA | vB | set(c for c in vs if in_box(e["bar"], c))
            if not (step_reach(varena, vA) & vB):
                failures.append(f"{label}: still not connected with the bar region voided")
        # the declared level relation, in every class that carries one
        if EXTERIOR in (a, b):
            if "rise" in e:
                failures.append(f"{label}: an exterior endpoint has no resolved box, so a "
                                f"declared rise cannot be measured")
        elif cls != "vision":
            declared = e.get("rise", 0 if cls == "walk" else None)
            if declared is not None:
                rises += 1
                measured = min(c[1] for c in B) - min(c[1] for c in A)
                if measured != declared:
                    failures.append(f"{label}: declared rise {declared}, measured {measured}")
    report.gate("edge-proof", not failures, proved,
                f"{proved} edge(s) proved of {len(edges)} declared, {rises} carrying a "
                f"level relation"
                + ("" if not failures else "; " + "; ".join(failures[:8])
                   + (f"; ... {len(failures)-8} more" if len(failures) > 8 else "")))

    # --- 5. reachability, per cell -------------------------------------------
    owner = {}
    for n, bs in sboxes.items():
        for c in standable:
            if in_any(bs, c):
                owner[c] = n
    walkable = set(owner) - nested
    targets = set(walkable)
    entry_cells = set(c for c in walkable if owner[c] == entry) if entry in spaces else set()

    def crossings_for(open_bars):
        out = set()
        for e in edges:
            if EXTERIOR in (e["a"], e["b"]):
                continue                     # exterior is a face, never connectivity
            cls = e["class"]
            if cls == "vision":
                continue
            if cls == "barred" and not open_bars:
                continue
            out.add((e["a"], e["b"]))
            if cls != "drop":
                out.add((e["b"], e["a"]))
        return out

    if entry_cells:
        if physical_25:
            reached = physical_reach(grid, entry_cells) & targets
        else:
            reached = graph_reach(grid, walkable, owner, crossings_for(False), entry_cells)
        unreached = targets - reached
        # "re-walked with bars opened" means the bar's BLOCKS are voided, exactly as
        # §2.4's barred proof does it — licensing the graph crossing alone reaches
        # nothing, because the wall still stands between the two spaces' cells.
        bars = [e["bar"] for e in edges if e.get("class") == "barred"]
        if bars:
            opened = Grid((grid.w, grid.h, grid.d),
                          {k: v for k, v in grid.blocks.items()
                           if not any(in_box(b, k) for b in bars)})
            ostand = opened.standable_cells()
            owalk = set(c for c in ostand
                        if any(in_any(bs, c) for bs in sboxes.values())
                        or any(in_box(b, c) for b in bars))
            owalk -= nested
            oowner = dict(owner)
            for c in owalk:
                if c not in oowner:
                    oowner[c] = "@bar"
            ocross = crossings_for(True) | {(n, "@bar") for n in spaces} \
                                         | {("@bar", n) for n in spaces}
            with_bars = graph_reach(opened, owalk, oowner, ocross, entry_cells)
        else:
            with_bars = reached
        # §2.5: a cell reachable only once bars open is stratified, not red; only a
        # cell unreachable under every opening is red.
        gated = unreached & with_bars
        never = unreached - with_bars
        per_space = {}
        for c in never:
            per_space[owner[c]] = per_space.get(owner[c], 0) + 1
        detail = (f"{len(targets)} cell(s) of {len(spaces)} space(s) after nested no_body, "
                  f"{len(reached)} reached from {entry!r} with bars standing, "
                  f"{len(gated)} more once bars open, {len(never)} unreached")
        if gated:
            detail += "; behind a bar: " + ", ".join(
                sorted({owner[c] for c in gated}))
        if per_space:
            detail += "; unreached per space: " + ", ".join(
                f"{k}={v}" for k, v in sorted(per_space.items()))
        report.gate("reachability", not never, len(targets), detail,
                    cells=(never if never else None))
    else:
        report.gate("reachability", False, 0, "no declared entry with a standable cell")
        reached = set()

    # --- 6. the no_body obligation -------------------------------------------
    phys = physical_reach(grid, entry_cells) if entry_cells else set()
    nb_problems, nb_cells = [], 0
    sealed_n = open_n = 0
    bad = set()
    for name, r in sorted(nobody.items()):
        cells = set(c for c in standable if in_any(r["boxes"], c))
        nb_cells += len(cells)
        if r.get("kind") == "sealed":
            sealed_n += 1
            live = cells & phys
            if live:
                nb_problems.append(f"{name}: {len(live)} standable cell(s) a body reaches — "
                                   f"play space wearing a no_body label")
                bad |= live
        elif r.get("kind") == "open":
            open_n += 1
            # §2.6 says "every cell of the region is open to sky". Taken over the
            # declared VOLUME the obligation is unsatisfiable: a box that hugs a
            # sloped surface necessarily contains cells under other parts of the
            # same surface — the cathedral's own roof region fails on 3690 of
            # them. The only satisfiable reading is the one coverage is stated
            # in: every STANDABLE cell. Both counts are reported; the volume
            # count is a finding, never the verdict, because a checker that can
            # never go green measures nothing.
            roofed = set(c for c in cells if not grid.open_to_sky(c))
            vol_roofed = sum(1 for c in cells_over(r["boxes"])
                             if grid.inside(c) and not grid.open_to_sky(c))
            if vol_roofed:
                report.finding(f"no_body {name}: {vol_roofed} cell(s) of the declared VOLUME "
                               f"have solid above them; the verdict uses the standable reading "
                               f"of §2.6 (see contract_check.py)")
            if roofed:
                nb_problems.append(f"{name}: {len(roofed)} standable cell(s) with solid above "
                                   f"them — not exterior decoration")
                bad |= roofed
    report.gate("no-body", not nb_problems, len(nobody),
                f"{len(nobody)} region(s) ({sealed_n} sealed, {open_n} open) over {nb_cells} "
                f"standable cell(s)"
                + ("" if not nb_problems else "; " + "; ".join(nb_problems[:6])
                   + (f"; ... {len(nb_problems)-6} more" if len(nb_problems) > 6 else "")),
                cells=bad)

    # --- 7. anchors ----------------------------------------------------------
    anchors = manifest.get("anchors", {})
    stray, by_kind = [], {}
    for name in sorted(anchors):
        p = tuple(anchors[name]["pos"])
        kind = None
        # a nested region is inside a space too, so it is asked FIRST — AC9 wants
        # `rafter_hall`'s perches to resolve as no_body with a finding, not to be
        # swallowed by the loft they hang in.
        if any(in_any(bs, p) for bs in nboxes.values()):
            kind = "no_body"
            report.finding(f"anchor {name} resolves to a no_body region, not to play space")
        if kind is None:
            for sn, bs in sorted(sboxes.items()):
                if any(in_box(grow(b), p) for b in bs):  # closed extent: interior + boundary
                    kind = "space"
                    break
        if kind is None:
            for e in edges:
                for key in ("via", "bar"):
                    if key in e and in_box(e[key], p):
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
        report.finding(f"no_body majority ({len(unowned)} of {len(standable)}) acknowledged: {ack}")
    if len(spaces) == 1 and not edges:
        report.finding("1 space, 0 edges — the contract claims nothing about how a body moves")
    open_env = [n for n, s in sorted(spaces.items()) if s["envelope"] == "open"]
    if open_env:
        report.finding(f"{len(open_env)} of {len(spaces)} space(s) are `open` and exempt from "
                       f"closure: " + ", ".join(open_env))


def cells_over(boxes):
    for b in boxes:
        yield from cells_of(b)


def physical_reach_within(grid, cells, seeds):
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
            fy = y
            while y - fy <= 64:
                fy -= 1
                below = (x + dx, fy, z + dz)
                if grid.get(below) is None:
                    break
                if grid.solid(below):
                    landing = (x + dx, fy + 1, z + dz)
                    if landing in cells and landing not in seen:
                        seen.add(landing)
                        q.append(landing)
                    break
    return seen


def main(argv):
    args = [a for a in argv[1:] if not a.startswith("--")]
    list_n, json_out, perturb, physical_25 = 12, None, [], False
    for f in [a for a in argv[1:] if a.startswith("--")]:
        if f.startswith("--list="):
            list_n = int(f.split("=", 1)[1])
        elif f.startswith("--json="):
            json_out = f.split("=", 1)[1]
        elif f == "--physical-reach":
            physical_25 = True
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
    if physical_25:
        report.finding("reachability run under the PHYSICAL reading of §2.5, not the "
                       "graph-confined one")
    check(grid, manifest, contract, report, physical_25)
    print(report.render(list_n))
    if json_out:
        Path(json_out).write_text(json.dumps(
            {"zone": report.zone, "gates": report.gates, "findings": report.findings},
            indent=1, sort_keys=True) + "\n")
    return EXIT_GATE if report.failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
