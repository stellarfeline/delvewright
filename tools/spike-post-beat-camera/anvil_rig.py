#!/usr/bin/env python3
"""A rig for one question: can the renderer be handed a world the compiler writes?

A Chunky scene names a world save, and today the only producer of one is a
server boot (`validation/world-save.sh`). The spec this rig serves proposes that
the compiler write the world itself, from its per-configuration block map, as
Anvil region files holding block states and biomes and nothing else. Before that
is specced, two things are measured here, on a real save:

  census   what a server-written save holds that a block map does not — the
           chunk tags present, entities and block entities by id, fluid states
           with a non-zero level, light arrays, heightmaps — inside the layout
           box and over the whole save.
  rewrite  the same save written back as a MINIMAL world: per chunk only
           `DataVersion`, `xPos`, `zPos`, `yPos`, `Status`, and per section `Y`,
           `block_states` and `biomes`; every region timestamp zero; zlib. Two
           rewrites of one save must be byte-identical, and the pinned Chunky
           core must render the minimal world as it renders the server's — the
           frames are compared by `compare`.
  compare  two PNG frames: identical-pixel fraction and luminance rms.

Standard library only. The Anvil layout and the chunk NBT are read as
minecraft.wiki documents them (Region file format; Chunk format): an 8 KiB header
of 1024 location entries (3 bytes sector offset, 1 byte sector count) and 1024
timestamps, 4 KiB sectors, a chunk payload of 4-byte length + 1-byte compression
(2 = zlib) + bytes; a section's `block_states.data` packs indices at
max(4, ceil(log2(palette))) bits each, never across a long, and is absent when
the palette has one entry; `biomes` the same at max(1, …) bits over 64 cells.
"""

from __future__ import annotations

import argparse
import collections
import hashlib
import io
import json
import math
import os
import pathlib
import struct
import sys
import zlib

# ----------------------------------------------------------------------- NBT

T_END, T_BYTE, T_SHORT, T_INT, T_LONG, T_FLOAT, T_DOUBLE = range(7)
T_BYTE_ARRAY, T_STRING, T_LIST, T_COMPOUND, T_INT_ARRAY, T_LONG_ARRAY = range(7, 13)


class Tag:
    __slots__ = ("kind", "value", "elem")

    def __init__(self, kind, value, elem=None):
        self.kind = kind
        self.value = value
        self.elem = elem  # list element kind

    def __repr__(self):
        return f"Tag({self.kind}, {self.value!r})"


def _read_payload(b: io.BytesIO, kind: int) -> Tag:
    if kind == T_BYTE:
        return Tag(kind, struct.unpack(">b", b.read(1))[0])
    if kind == T_SHORT:
        return Tag(kind, struct.unpack(">h", b.read(2))[0])
    if kind == T_INT:
        return Tag(kind, struct.unpack(">i", b.read(4))[0])
    if kind == T_LONG:
        return Tag(kind, struct.unpack(">q", b.read(8))[0])
    if kind == T_FLOAT:
        return Tag(kind, struct.unpack(">f", b.read(4))[0])
    if kind == T_DOUBLE:
        return Tag(kind, struct.unpack(">d", b.read(8))[0])
    if kind == T_BYTE_ARRAY:
        n = struct.unpack(">i", b.read(4))[0]
        return Tag(kind, b.read(n))
    if kind == T_STRING:
        n = struct.unpack(">H", b.read(2))[0]
        return Tag(kind, b.read(n).decode("utf-8", "replace"))
    if kind == T_LIST:
        elem = b.read(1)[0]
        n = struct.unpack(">i", b.read(4))[0]
        return Tag(kind, [_read_payload(b, elem) for _ in range(n)], elem)
    if kind == T_COMPOUND:
        d = {}
        while True:
            k = b.read(1)[0]
            if k == T_END:
                break
            n = struct.unpack(">H", b.read(2))[0]
            name = b.read(n).decode("utf-8", "replace")
            d[name] = _read_payload(b, k)
        return Tag(kind, d)
    if kind == T_INT_ARRAY:
        n = struct.unpack(">i", b.read(4))[0]
        return Tag(kind, list(struct.unpack(f">{n}i", b.read(4 * n))))
    if kind == T_LONG_ARRAY:
        n = struct.unpack(">i", b.read(4))[0]
        return Tag(kind, list(struct.unpack(f">{n}q", b.read(8 * n))))
    raise ValueError(f"unknown tag kind {kind}")


def nbt_read(data: bytes) -> tuple[str, Tag]:
    b = io.BytesIO(data)
    kind = b.read(1)[0]
    n = struct.unpack(">H", b.read(2))[0]
    name = b.read(n).decode("utf-8", "replace")
    return name, _read_payload(b, kind)


def _write_payload(out: bytearray, t: Tag) -> None:
    k = t.kind
    if k == T_BYTE:
        out += struct.pack(">b", t.value)
    elif k == T_SHORT:
        out += struct.pack(">h", t.value)
    elif k == T_INT:
        out += struct.pack(">i", t.value)
    elif k == T_LONG:
        out += struct.pack(">q", t.value)
    elif k == T_FLOAT:
        out += struct.pack(">f", t.value)
    elif k == T_DOUBLE:
        out += struct.pack(">d", t.value)
    elif k == T_BYTE_ARRAY:
        out += struct.pack(">i", len(t.value)) + bytes(t.value)
    elif k == T_STRING:
        s = t.value.encode("utf-8")
        out += struct.pack(">H", len(s)) + s
    elif k == T_LIST:
        elem = t.elem if t.value or t.elem is not None else T_END
        out += bytes([elem]) + struct.pack(">i", len(t.value))
        for e in t.value:
            _write_payload(out, e)
    elif k == T_COMPOUND:
        for name, v in t.value.items():
            s = name.encode("utf-8")
            out += bytes([v.kind]) + struct.pack(">H", len(s)) + s
            _write_payload(out, v)
        out += bytes([T_END])
    elif k == T_INT_ARRAY:
        out += struct.pack(">i", len(t.value)) + struct.pack(f">{len(t.value)}i", *t.value)
    elif k == T_LONG_ARRAY:
        out += struct.pack(">i", len(t.value)) + struct.pack(f">{len(t.value)}q", *t.value)
    else:
        raise ValueError(f"unknown tag kind {k}")


def nbt_write(name: str, root: Tag) -> bytes:
    out = bytearray()
    s = name.encode("utf-8")
    out += bytes([root.kind]) + struct.pack(">H", len(s)) + s
    _write_payload(out, root)
    return bytes(out)


# -------------------------------------------------------------------- region

SECTOR = 4096


def region_chunks(path: pathlib.Path):
    """Yield `(cx, cz, timestamp, compression, nbt_root)` per stored chunk."""
    data = path.read_bytes()
    stem = path.stem.split(".")
    rx, rz = int(stem[1]), int(stem[2])
    if len(data) < 2 * SECTOR:
        # The server writes an empty region file for a region it touched and
        # stored nothing in; a header shorter than two sectors holds no chunk.
        return
    for i in range(1024):
        loc = struct.unpack(">I", data[4 * i : 4 * i + 4])[0]
        off, cnt = loc >> 8, loc & 0xFF
        if off == 0 and cnt == 0:
            continue
        ts = struct.unpack(">I", data[4096 + 4 * i : 4096 + 4 * i + 4])[0]
        start = off * SECTOR
        length = struct.unpack(">i", data[start : start + 4])[0]
        comp = data[start + 4]
        payload = data[start + 5 : start + 4 + length]
        if comp == 2:
            raw = zlib.decompress(payload)
        elif comp == 1:
            import gzip

            raw = gzip.decompress(payload)
        elif comp == 3:
            raw = payload
        else:
            raise ValueError(f"{path}: chunk {i} uses compression {comp}")
        _, root = nbt_read(raw)
        cx = rx * 32 + (i % 32)
        cz = rz * 32 + (i // 32)
        yield cx, cz, ts, comp, root


def region_write(path: pathlib.Path, chunks: dict[tuple[int, int], Tag]) -> None:
    """Write `chunks` ({(cx, cz): root}) as one region file: zlib, timestamps 0,
    chunk order by index, no padding beyond the sector rule."""
    header_loc = bytearray(4096)
    header_ts = bytes(4096)
    body = bytearray()
    sector = 2
    for (cx, cz), root in sorted(chunks.items(), key=lambda kv: ((kv[0][1] % 32) * 32 + kv[0][0] % 32)):
        i = (cz % 32) * 32 + (cx % 32)
        raw = nbt_write("", root)
        comp = zlib.compress(raw, 9)
        payload = struct.pack(">i", len(comp) + 1) + bytes([2]) + comp
        pad = (-len(payload)) % SECTOR
        payload += bytes(pad)
        cnt = len(payload) // SECTOR
        header_loc[4 * i : 4 * i + 4] = struct.pack(">I", (sector << 8) | cnt)
        body += payload
        sector += cnt
    path.write_bytes(bytes(header_loc) + header_ts + bytes(body))


# ------------------------------------------------------------------ sections

def _bits(n: int, floor: int) -> int:
    return max(floor, (n - 1).bit_length()) if n > 1 else floor


def unpack_indices(longs: list[int], count: int, bits: int) -> list[int]:
    per = 64 // bits
    mask = (1 << bits) - 1
    out = []
    for i in range(count):
        w = longs[i // per] & 0xFFFFFFFFFFFFFFFF
        out.append((w >> ((i % per) * bits)) & mask)
    return out


def pack_indices(indices: list[int], bits: int) -> list[int]:
    per = 64 // bits
    longs = []
    for base in range(0, len(indices), per):
        w = 0
        for j, v in enumerate(indices[base : base + per]):
            w |= (v & ((1 << bits) - 1)) << (j * bits)
        if w >= 1 << 63:
            w -= 1 << 64
        longs.append(w)
    return longs


def state_text(c: Tag) -> str:
    name = c.value["Name"].value
    props = c.value.get("Properties")
    if not props or not props.value:
        return name
    inner = ",".join(f"{k}={v.value}" for k, v in sorted(props.value.items()))
    return f"{name}[{inner}]"


def section_blocks(sec: Tag) -> tuple[list[str], list[int]]:
    bs = sec.value.get("block_states")
    if bs is None:
        return [], []
    palette = [state_text(p) for p in bs.value["palette"].value]
    data = bs.value.get("data")
    if data is None or len(palette) == 1:
        return palette, [0] * 4096
    return palette, unpack_indices(data.value, 4096, _bits(len(palette), 4))


# -------------------------------------------------------------------- census

def load_world(world: pathlib.Path):
    chunks = {}
    for f in sorted((world / "region").glob("r.*.mca")):
        for cx, cz, ts, comp, root in region_chunks(f):
            chunks[(cx, cz)] = (ts, comp, root)
    return chunks


def census(world: pathlib.Path, box) -> dict:
    chunks = load_world(world)
    out: dict = {"world": str(world), "chunks": len(chunks)}
    tags = collections.Counter()
    sec_tags = collections.Counter()
    status = collections.Counter()
    dv = collections.Counter()
    comp = collections.Counter()
    ts_nonzero = 0
    biomes = collections.Counter()
    block_entities = collections.Counter()
    cells = 0
    nonair = 0
    states = collections.Counter()
    fluid_levelled = collections.Counter()
    gravity = collections.Counter()
    in_box_nonair = 0
    for (cx, cz), (ts, c, root) in chunks.items():
        comp[c] += 1
        ts_nonzero += ts != 0
        d = root.value
        for k in d:
            tags[k] += 1
        status[d.get("Status", Tag(T_STRING, "?")).value] += 1
        dv[d.get("DataVersion", Tag(T_INT, -1)).value] += 1
        for be in d.get("block_entities", Tag(T_LIST, [])).value:
            block_entities[be.value["id"].value] += 1
        for sec in d.get("sections", Tag(T_LIST, [])).value:
            for k in sec.value:
                sec_tags[k] += 1
            y0 = sec.value["Y"].value * 16
            bio = sec.value.get("biomes")
            if bio is not None:
                for p in bio.value["palette"].value:
                    biomes[p.value] += 1
            palette, idx = section_blocks(sec)
            if not palette:
                continue
            for i, pi in enumerate(idx):
                st = palette[pi]
                cells += 1
                if st == "minecraft:air":
                    continue
                nonair += 1
                states[st] += 1
                x = cx * 16 + (i % 16)
                z = cz * 16 + ((i // 16) % 16)
                y = y0 + i // 256
                inside = box is None or (
                    box[0][0] <= x <= box[1][0] and box[0][1] <= y <= box[1][1] and box[0][2] <= z <= box[1][2]
                )
                if inside:
                    in_box_nonair += 1
                if ("water" in st or "lava" in st) and "level=" in st and "level=0" not in st:
                    fluid_levelled[st] += 1
                base = st.split("[")[0].split(":")[-1]
                if base in {"sand", "red_sand", "gravel", "anvil", "chipped_anvil", "damaged_anvil", "dragon_egg"} or base.endswith("concrete_powder"):
                    gravity[st] += 1
    entities = collections.Counter()
    ent_dir = world / "entities"
    if ent_dir.is_dir():
        for f in sorted(ent_dir.glob("r.*.mca")):
            for _cx, _cz, _ts, _c, root in region_chunks(f):
                for e in root.value.get("Entities", Tag(T_LIST, [])).value:
                    entities[e.value["id"].value] += 1
    out.update(
        {
            "chunk_tags": dict(tags),
            "section_tags": dict(sec_tags),
            "status": dict(status),
            "data_version": dict(dv),
            "compression": dict(comp),
            "timestamps_nonzero": ts_nonzero,
            "cells_read": cells,
            "cells_non_air": nonair,
            "cells_non_air_in_box": in_box_nonair,
            "distinct_states": len(states),
            "fluid_with_nonzero_level": dict(fluid_levelled),
            "gravity_blocks": dict(gravity),
            "biome_palette_entries": dict(biomes),
            "block_entities": dict(block_entities),
            "entities": dict(entities),
            "entities_dir": ent_dir.is_dir(),
            "level_dat_bytes": (world / "level.dat").stat().st_size if (world / "level.dat").is_file() else 0,
        }
    )
    return out


# ------------------------------------------------------------------- rewrite

KEEP_CHUNK = ("DataVersion", "xPos", "zPos", "yPos", "Status")
KEEP_SECTION = ("Y", "block_states", "biomes")


def minimal_chunk(root: Tag) -> Tag:
    d = root.value
    out = {k: d[k] for k in KEEP_CHUNK if k in d}
    out["Status"] = Tag(T_STRING, "minecraft:full")
    secs = []
    for sec in d.get("sections", Tag(T_LIST, [])).value:
        s = {k: sec.value[k] for k in KEEP_SECTION if k in sec.value}
        if "block_states" in s:
            # Re-pack the indices from the palette we read, so the writer proves
            # the packing rule rather than copying the server's longs.
            palette, idx = section_blocks(sec)
            bs = {"palette": sec.value["block_states"].value["palette"]}
            if len(palette) > 1:
                bs["data"] = Tag(T_LONG_ARRAY, pack_indices(idx, _bits(len(palette), 4)))
            s["block_states"] = Tag(T_COMPOUND, bs)
        secs.append(Tag(T_COMPOUND, s))
    out["sections"] = Tag(T_LIST, secs, T_COMPOUND)
    return Tag(T_COMPOUND, out)


def minimal_level_dat(data_version: int) -> bytes:
    """The smallest `level.dat` this rig can defend: the keys the pinned core's
    world loader names (`Data`, `SpawnX`/`SpawnY`/`SpawnZ`, `version`) plus the
    data version, gzip-framed as the game writes it."""
    import gzip

    data = Tag(
        T_COMPOUND,
        {
            "DataVersion": Tag(T_INT, data_version),
            "LevelName": Tag(T_STRING, "delve"),
            "SpawnX": Tag(T_INT, 0),
            "SpawnY": Tag(T_INT, 64),
            "SpawnZ": Tag(T_INT, 0),
            "version": Tag(T_INT, 19133),
        },
    )
    raw = nbt_write("", Tag(T_COMPOUND, {"Data": data}))
    return gzip.compress(raw, mtime=0)


def perturb_chunk(root: Tag, swap: tuple[str, str]) -> int:
    """Replace every palette entry equal to `swap[0]` with `swap[1]`; returns the
    number of palette entries moved. A perturbation only a block map could
    produce, used to show the frame comparison can see one block state move."""
    moved = 0
    for sec in root.value.get("sections", Tag(T_LIST, [])).value:
        bs = sec.value.get("block_states")
        if bs is None:
            continue
        for p in bs.value["palette"].value:
            if state_text(p) == swap[0]:
                p.value["Name"] = Tag(T_STRING, swap[1])
                p.value.pop("Properties", None)
                moved += 1
    return moved


def rewrite(world: pathlib.Path, out: pathlib.Path, minimal_level: bool = False, swap=None) -> dict:
    chunks = load_world(world)
    (out / "region").mkdir(parents=True, exist_ok=True)
    data_version = next(iter(chunks.values()))[2].value["DataVersion"].value
    if minimal_level:
        (out / "level.dat").write_bytes(minimal_level_dat(data_version))
    else:
        (out / "level.dat").write_bytes((world / "level.dat").read_bytes())
    moved = 0
    if swap:
        for (_ts, _c, root) in chunks.values():
            moved += perturb_chunk(root, swap)
    by_region: dict[tuple[int, int], dict] = collections.defaultdict(dict)
    kept = {}
    for (cx, cz), (_ts, _c, root) in chunks.items():
        # A compiler-written world holds the chunks its block map touches and no
        # proto-chunk the server's generator started: keep a chunk only if one
        # of its sections holds a block.
        holds = any(
            pal and any(p != "minecraft:air" for p in pal)
            for pal, _ in (section_blocks(s) for s in root.value.get("sections", Tag(T_LIST, [])).value)
        )
        if not holds:
            continue
        kept[(cx, cz)] = chunks[(cx, cz)]
        by_region[(cx >> 5, cz >> 5)][(cx, cz)] = minimal_chunk(root)
    chunks = kept
    hashes = {}
    for (rx, rz), cs in sorted(by_region.items()):
        p = out / "region" / f"r.{rx}.{rz}.mca"
        region_write(p, cs)
        hashes[p.name] = hashlib.sha256(p.read_bytes()).hexdigest()
    # Read back what was written and hold it to what was read: every cell equal.
    back = load_world(out)
    mismatched = 0
    compared = 0
    for key, (_ts, _c, root) in chunks.items():
        a = {s.value["Y"].value: section_blocks(s) for s in root.value["sections"].value}
        b = {s.value["Y"].value: section_blocks(s) for s in back[key][2].value["sections"].value}
        for y, (pal, idx) in a.items():
            pal2, idx2 = b[y]
            if not pal:
                continue
            for i in range(4096):
                compared += 1
                if pal[idx[i]] != pal2[idx2[i]]:
                    mismatched += 1
    return {
        "out": str(out),
        "chunks": len(chunks),
        "region_files": {k: v for k, v in hashes.items()},
        "cells_compared": compared,
        "cells_mismatched": mismatched,
        "bytes": sum((out / "region" / k).stat().st_size for k in hashes),
        "level_dat": "minimal" if minimal_level else "copied",
        "level_dat_bytes": (out / "level.dat").stat().st_size,
        "level_dat_sha256": hashlib.sha256((out / "level.dat").read_bytes()).hexdigest(),
        "palette_entries_perturbed": moved,
    }


# ------------------------------------------------------------------- compare

def read_png(path: pathlib.Path):
    data = path.read_bytes()
    assert data[:8] == b"\x89PNG\r\n\x1a\n", path
    pos = 8
    idat = bytearray()
    w = h = 0
    depth = ctype = 0
    while pos < len(data):
        n = struct.unpack(">I", data[pos : pos + 4])[0]
        kind = data[pos + 4 : pos + 8]
        body = data[pos + 8 : pos + 8 + n]
        pos += 12 + n
        if kind == b"IHDR":
            w, h, depth, ctype = struct.unpack(">IIBB", body[:10])
        elif kind == b"IDAT":
            idat += body
        elif kind == b"IEND":
            break
    assert depth == 8, (path, depth)
    bpp = {2: 3, 6: 4, 0: 1, 4: 2}[ctype]
    raw = zlib.decompress(bytes(idat))
    stride = w * bpp
    prev = bytearray(stride)
    rows = []
    p = 0
    for _ in range(h):
        f = raw[p]
        p += 1
        cur = bytearray(raw[p : p + stride])
        p += stride
        for i in range(stride):
            a = cur[i - bpp] if i >= bpp else 0
            b = prev[i]
            c = prev[i - bpp] if i >= bpp else 0
            if f == 1:
                cur[i] = (cur[i] + a) & 0xFF
            elif f == 2:
                cur[i] = (cur[i] + b) & 0xFF
            elif f == 3:
                cur[i] = (cur[i] + ((a + b) >> 1)) & 0xFF
            elif f == 4:
                pa, pb, pc = abs(b - c), abs(a - c), abs(a + b - 2 * c)
                pr = a if pa <= pb and pa <= pc else (b if pb <= pc else c)
                cur[i] = (cur[i] + pr) & 0xFF
        rows.append(bytes(cur))
        prev = cur
    return w, h, bpp, rows


def compare(a: pathlib.Path, b: pathlib.Path) -> dict:
    wa, ha, ba, ra = read_png(a)
    wb, hb, bb, rb = read_png(b)
    assert (wa, ha, ba) == (wb, hb, bb), ((wa, ha, ba), (wb, hb, bb))
    same = 0
    total = wa * ha
    acc = 0.0
    maxd = 0
    for y in range(ha):
        la, lb = ra[y], rb[y]
        for x in range(wa):
            o = x * ba
            pa = la[o : o + 3]
            pb = lb[o : o + 3]
            if pa == pb:
                same += 1
            ya = 0.299 * pa[0] + 0.587 * pa[1] + 0.114 * pa[2]
            yb = 0.299 * pb[0] + 0.587 * pb[1] + 0.114 * pb[2]
            d = ya - yb
            acc += d * d
            maxd = max(maxd, abs(d))
    return {
        "a": str(a),
        "b": str(b),
        "size": [wa, ha],
        "pixels": total,
        "identical_pixels": same,
        "identical_fraction": round(same / total, 4),
        "luminance_rms": round(math.sqrt(acc / total), 3),
        "luminance_max_abs": round(maxd, 1),
        "sha256_a": hashlib.sha256(a.read_bytes()).hexdigest(),
        "sha256_b": hashlib.sha256(b.read_bytes()).hexdigest(),
    }


# ---------------------------------------------------------------------- main

def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("census")
    c.add_argument("world", type=pathlib.Path)
    c.add_argument("--render-plan", type=pathlib.Path, help="read `layout_aabb` as the box")
    r = sub.add_parser("rewrite")
    r.add_argument("world", type=pathlib.Path)
    r.add_argument("out", type=pathlib.Path)
    r.add_argument("--minimal-level-dat", action="store_true", help="write a minimal level.dat instead of copying the server's")
    r.add_argument("--perturb", metavar="FROM=TO", help="swap one block state for another in every palette (a block-map perturbation)")
    k = sub.add_parser("compare")
    k.add_argument("a", type=pathlib.Path)
    k.add_argument("b", type=pathlib.Path)
    a = ap.parse_args(argv[1:])
    if a.cmd == "census":
        box = None
        if a.render_plan:
            lb = json.loads(a.render_plan.read_text())["layout_aabb"]
            box = (lb["min"], lb["max"])
        print(json.dumps(census(a.world, box), indent=1, sort_keys=True))
    elif a.cmd == "rewrite":
        swap = tuple(a.perturb.split("=", 1)) if a.perturb else None
        print(json.dumps(rewrite(a.world, a.out, a.minimal_level_dat, swap), indent=1, sort_keys=True))
    else:
        print(json.dumps(compare(a.a, a.b), indent=1, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
