#!/usr/bin/env python3
"""SPIKE TOOLING (ADR-PENDING-map-design-pipeline, spec-PENDING §4.1) — NOT part
of the shipped pipeline.

Reads a delve-grammar tile-set manifest and its `.nbt` parts back into one voxel
grid, and answers the body questions `crates/grammar/src/nav.rs` answers in Rust:
`passable`, `solid`, `standable`, the plain +/-1-step walk, and the walk-with-fall.

The predicates are ported deliberately rather than shared: this prototype has to
be able to disagree with the engine, and a checker that imports the thing it is
auditing cannot. They are kept line-for-line faithful to `nav.rs` (including
"outside the region blocks" and the 64-block runaway-fall cap) so that a
disagreement is a finding rather than a translation bug.

One correction against the throwaway `reach.py` probe this replaces: a cell whose
support lies OUTSIDE the region is not standable. `reach.py` returned "not air"
for out-of-bounds and so stood bodies on the region floor plane; `nav.rs`'s
`solid` returns false for `None`, and this file follows `nav.rs`.
"""

import gzip
import json
import struct
from collections import deque
from pathlib import Path


# --- NBT ---------------------------------------------------------------------

class _R:
    def __init__(self, b):
        self.b, self.i = b, 0

    def u1(self):
        v = self.b[self.i]
        self.i += 1
        return v

    def _a(self, n):
        j = self.i
        self.i += n
        return j

    def s(self):
        n = struct.unpack_from(">H", self.b, self._a(2))[0]
        return self.b[self._a(n):self.i].decode("utf-8", "replace")


def _payload(r, t):
    if t == 1:
        return struct.unpack_from(">b", r.b, r._a(1))[0]
    if t == 2:
        return struct.unpack_from(">h", r.b, r._a(2))[0]
    if t == 3:
        return struct.unpack_from(">i", r.b, r._a(4))[0]
    if t == 4:
        return struct.unpack_from(">q", r.b, r._a(8))[0]
    if t == 5:
        return struct.unpack_from(">f", r.b, r._a(4))[0]
    if t == 6:
        return struct.unpack_from(">d", r.b, r._a(8))[0]
    if t == 7:
        n = struct.unpack_from(">i", r.b, r._a(4))[0]
        return list(r.b[r._a(n):r.i])
    if t == 8:
        return r.s()
    if t == 9:
        et = r.u1()
        n = struct.unpack_from(">i", r.b, r._a(4))[0]
        return [_payload(r, et) for _ in range(n)]
    if t == 10:
        o = {}
        while True:
            tt = r.u1()
            if tt == 0:
                return o
            # Two statements, deliberately: `o[r.s()] = _payload(r, tt)` reads
            # the value before the key, because Python evaluates an assignment's
            # right-hand side first. The stream is order-sensitive.
            key = r.s()
            o[key] = _payload(r, tt)
    if t == 11:
        n = struct.unpack_from(">i", r.b, r._a(4))[0]
        return [struct.unpack_from(">i", r.b, r._a(4))[0] for _ in range(n)]
    if t == 12:
        n = struct.unpack_from(">i", r.b, r._a(4))[0]
        return [struct.unpack_from(">q", r.b, r._a(8))[0] for _ in range(n)]
    raise ValueError(f"unknown NBT tag {t}")


def read_nbt(path):
    r = _R(gzip.open(path, "rb").read())
    t = r.u1()
    r.s()
    return _payload(r, t)


# --- the grid ----------------------------------------------------------------

class Grid:
    """A zone's delivered blocks, addressed in zone-local coordinates."""

    def __init__(self, size, blocks):
        self.w, self.h, self.d = size
        self.blocks = blocks  # (x, y, z) -> block name; absent means air
        self._standable = None

    @classmethod
    def from_manifest(cls, manifest_path):
        manifest_path = Path(manifest_path)
        man = json.loads(manifest_path.read_text())
        # A prefab past the 48-per-axis template cap ships as a tile SET with a
        # `structure_set`; one under it ships as a single `structure`. Both are
        # the same zone once reassembled, so the checker takes either.
        if "structure_set" in man:
            ss = man["structure_set"]
        else:
            one = man["structure"]
            ss = {"size": one["size"],
                  "parts": [{"file": one["file"], "offset": [0, 0, 0]}]}
        blocks = {}
        for part in ss["parts"]:
            nbt = read_nbt(manifest_path.parent / part["file"])
            pal = [b["Name"] for b in nbt["palette"]]
            ox, oy, oz = part["offset"]
            for blk in nbt["blocks"]:
                x, y, z = blk["pos"]
                name = pal[blk["state"]]
                if name != "minecraft:air":
                    blocks[(x + ox, y + oy, z + oz)] = name
        return cls(ss["size"], blocks), man

    def inside(self, c):
        x, y, z = c
        return 0 <= x < self.w and 0 <= y < self.h and 0 <= z < self.d

    def get(self, c):
        """None outside the region; "" for an air cell inside it."""
        if not self.inside(c):
            return None
        return self.blocks.get(c, "")

    def passable(self, c):
        b = self.get(c)
        if b is None:
            return False
        return b == "" or b.endswith("_skull")

    def solid(self, c):
        b = self.get(c)
        return b is not None and not self.passable(c)

    def standable(self, c):
        x, y, z = c
        return (self.passable(c)
                and self.passable((x, y + 1, z))
                and self.solid((x, y - 1, z)))

    def standable_cells(self):
        if self._standable is None:
            self._standable = frozenset(
                (x, y, z)
                for x in range(self.w)
                for y in range(self.h)
                for z in range(self.d)
                if self.standable((x, y, z))
            )
        return self._standable

    def open_to_sky(self, c):
        x, y, z = c
        return not any(self.solid((x, yy, z)) for yy in range(y + 1, self.h))


# --- the walks (ports of nav.rs) ---------------------------------------------

STEPS = ((1, 0), (-1, 0), (0, 1), (0, -1))


def walk_closure(cells, seeds):
    """nav::connected's frontier: one cell horizontally, at most one block up or
    down, restricted to `cells`."""
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


def fall_closure(grid, cells, seeds):
    """nav::reachable_with_fall's frontier: the plain step plus a one-way fall to
    the first solid floor below, landing only on a member of `cells`."""
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
