"""Minimal Sponge schematic v2 writer (gzip-framed Java NBT), stdlib only.

Writes exactly the fields `delvec schem convert` reads (crates/delvec/src/schem/
schematic.rs): Version, DataVersion, Width/Height/Length, Offset, PaletteMax,
Palette, BlockData (varint-packed, index = x + z*W + y*W*L). Deterministic:
the palette is ordered by first appearance in index order, gzip mtime is 0.
"""

import gzip
import io
import struct

DATA_VERSION = 4671  # MC 1.21.11 (ADR-0009), the value delvec's own fixtures write


def _name(out, s):
    b = s.encode("utf-8")
    out.write(struct.pack(">H", len(b)))
    out.write(b)


def _tag(out, tag_id, name, payload):
    out.write(bytes([tag_id]))
    _name(out, name)
    out.write(payload)


def _varints(values):
    out = bytearray()
    for v in values:
        while True:
            b = v & 0x7F
            v >>= 7
            if v:
                out.append(b | 0x80)
            else:
                out.append(b)
                break
    return bytes(out)


def write_schem(path, size, states, index_order_ids):
    """size=(W,H,L); states=list of state strings; index_order_ids = flat ids
    in schem index order (x fastest, then z, then y)."""
    w, h, l = size
    body = io.BytesIO()
    _tag(body, 3, "Version", struct.pack(">i", 2))
    _tag(body, 3, "DataVersion", struct.pack(">i", DATA_VERSION))
    _tag(body, 2, "Width", struct.pack(">h", w))
    _tag(body, 2, "Height", struct.pack(">h", h))
    _tag(body, 2, "Length", struct.pack(">h", l))
    _tag(body, 11, "Offset", struct.pack(">i", 3) + struct.pack(">iii", 0, 0, 0))
    _tag(body, 3, "PaletteMax", struct.pack(">i", len(states)))
    pal = io.BytesIO()
    for i, s in enumerate(states):
        _tag(pal, 3, s, struct.pack(">i", i))
    pal.write(b"\x00")
    _tag(body, 10, "Palette", pal.getvalue())
    data = _varints(index_order_ids)
    _tag(body, 7, "BlockData", struct.pack(">i", len(data)) + data)
    body.write(b"\x00")
    root = io.BytesIO()
    _tag(root, 10, "Schematic", body.getvalue())
    with open(path, "wb") as f:
        with gzip.GzipFile(fileobj=f, mode="wb", mtime=0, filename="") as g:
            g.write(root.getvalue())
