"""Anvil worlds read as the pinned Chunky core reads them — the one parse rule.

Every tool that reads a world save or a world `delvec cameras` writes reads it
through here (`tools/ci/check-written-world.py`, the spec-0089 rig, the
writer's own read-back test in `crates/delvec/tests/view_cli.rs`), so two gates
never hold two copies of the region and chunk rules.

The layout follows minecraft.wiki's *Region file format* and *Chunk format*: an
8 KiB header of 1024 location entries (3 bytes sector offset, 1 byte sector
count) and 1024 timestamps, 4 KiB sectors, a chunk payload of a 4-byte length,
a 1-byte compression scheme (1 GZip, 2 Zlib, 3 none) and the bytes; a section's
`block_states.data` packs palette indices at max(4, ceil(log2 n)) bits, never
across a long, absent when the palette has one entry; `biomes` the same at
max(1, ceil(log2 n)) over 64 cells. Standard library only.
"""

from __future__ import annotations

import io
import pathlib
import struct
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


def biome_bits(n: int) -> int:
    """Bits per biome index for an `n`-entry palette (no 4-bit floor)."""
    return max(1, (n - 1).bit_length())


def section_biomes(sec: Tag) -> tuple[list[str], list[int]]:
    bio = sec.value.get("biomes")
    if bio is None:
        return [], []
    palette = [p.value for p in bio.value["palette"].value]
    data = bio.value.get("data")
    if data is None or len(palette) == 1:
        return palette, [0] * 64
    return palette, unpack_indices(data.value, 64, biome_bits(len(palette)))


def load_world(world: pathlib.Path):
    """`{(cx, cz): (timestamp, compression, root)}` over `world/region/*.mca`."""
    chunks = {}
    for f in sorted((world / "region").glob("r.*.mca")):
        for cx, cz, ts, comp, root in region_chunks(f):
            chunks[(cx, cz)] = (ts, comp, root)
    return chunks


def cells(world: pathlib.Path, box=None) -> dict[tuple[int, int, int], str]:
    """Every non-air cell of `world` as `{(x, y, z): state text}`, inside the
    inclusive `box` (`((x0, y0, z0), (x1, y1, z1))`) when one is given."""
    out: dict[tuple[int, int, int], str] = {}
    for (cx, cz), (_ts, _c, root) in load_world(world).items():
        if box is not None and (
            cx * 16 + 15 < box[0][0] or cx * 16 > box[1][0] or cz * 16 + 15 < box[0][2] or cz * 16 > box[1][2]
        ):
            continue
        for sec in root.value.get("sections", Tag(T_LIST, [])).value:
            palette, idx = section_blocks(sec)
            if not palette:
                continue
            y0 = sec.value["Y"].value * 16
            if box is not None and (y0 + 15 < box[0][1] or y0 > box[1][1]):
                continue
            for i, pi in enumerate(idx):
                st = palette[pi]
                if st in AIR:
                    continue
                x, y, z = cx * 16 + (i % 16), y0 + i // 256, cz * 16 + ((i // 16) % 16)
                if box is not None and not (
                    box[0][0] <= x <= box[1][0] and box[0][1] <= y <= box[1][1] and box[0][2] <= z <= box[1][2]
                ):
                    continue
                out[(x, y, z)] = st
    return out


AIR = frozenset({"minecraft:air", "minecraft:cave_air", "minecraft:void_air"})
