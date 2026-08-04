#!/usr/bin/env python3
"""Flora-parity gate for horizon parameter rows (spec-0026 acceptance
criterion 6).

Cherry-valley must be a PARAMETER ROW of the valley base, never a fork: given
two emitted output trees built from the same seed — one `{base: valley}`
(oak / stone-grass) and one `{base: valley, flora: cherry, palette:
stone-petal}` — their diff may touch ONLY flora/palette block ids and the
flora's biome id. Anything else (a moved block, a different tile split, an
extra file, a changed command) is a fork and fails.

How each differing file pair is judged:

- Byte-identical files pass unseen.
- `*.nbt` (gzip-framed vanilla structure NBT): both are parsed with the
  minimal NBT reader below. `size` and the full `blocks` list (pos + palette
  index + nbt) must be IDENTICAL; the two palettes must be equal length and
  each positional entry pair must be either identical or one of the allowed
  flora id pairs (properties compared verbatim after id mapping).
- Text files (`.mcfunction`, `.json`, `.properties`, …): equal after mapping
  the allowed biome/block id strings valley→cherry. Because the decor tables
  reuse `short_grass` on both sides with different meanings, text mapping is
  applied valley→cherry as an ordered list of whole-token replacements and
  must produce the cherry bytes exactly.
- A file present in only one tree fails.

Exit 0 = parameter row proven; exit 1 = fork (every offending path is named).

Usage: check-flora-parity.py <valley_out_dir> <cherry_out_dir>
"""

from __future__ import annotations

import gzip
import io
import struct
import sys
from pathlib import Path

# Declaration mirrors: files that verbatim-embed the campaign DECLARATION
# (horizon token, content hash) rather than the world the player enters. The
# two declarations differ by definition — the parity law binds the emitted
# WORLD, so these are exempt. Everything else must map.
DECLARATION_MIRRORS = {"manifest.json"}

# The single source of truth for the allowed id surface (valley id, cherry id).
# Keep in sync with `crates/compiler/src/surround.rs` flora/decor tables.
ALLOWED_PAIRS = [
    ("minecraft:oak_log", "minecraft:cherry_log"),
    ("minecraft:oak_leaves", "minecraft:cherry_leaves"),
    ("minecraft:short_grass", "minecraft:pink_petals"),
    ("minecraft:fern", "minecraft:short_grass"),
    ("minecraft:windswept_forest", "minecraft:cherry_grove"),
]


# --- minimal NBT reader (structure files only; big-endian, gzip-framed) -----

TAG_END = 0
TAG_NAMES = {
    0: "end", 1: "byte", 2: "short", 3: "int", 4: "long", 5: "float",
    6: "double", 7: "byte_array", 8: "string", 9: "list", 10: "compound",
    11: "int_array", 12: "long_array",
}


def _read(fmt: str, buf: io.BytesIO):
    size = struct.calcsize(fmt)
    return struct.unpack(fmt, buf.read(size))[0]


def _read_string(buf: io.BytesIO) -> str:
    n = _read(">H", buf)
    return buf.read(n).decode("utf-8")


def _read_payload(tag: int, buf: io.BytesIO):
    if tag == 1:
        return _read(">b", buf)
    if tag == 2:
        return _read(">h", buf)
    if tag == 3:
        return _read(">i", buf)
    if tag == 4:
        return _read(">q", buf)
    if tag == 5:
        return _read(">f", buf)
    if tag == 6:
        return _read(">d", buf)
    if tag == 7:
        n = _read(">i", buf)
        return list(buf.read(n))
    if tag == 8:
        return _read_string(buf)
    if tag == 9:
        item = _read(">b", buf)
        n = _read(">i", buf)
        return [_read_payload(item, buf) for _ in range(n)]
    if tag == 10:
        out = {}
        while True:
            t = _read(">b", buf)
            if t == TAG_END:
                return out
            name = _read_string(buf)
            out[name] = _read_payload(t, buf)
    if tag == 11:
        n = _read(">i", buf)
        return [_read(">i", buf) for _ in range(n)]
    if tag == 12:
        n = _read(">i", buf)
        return [_read(">q", buf) for _ in range(n)]
    raise ValueError(f"unknown NBT tag {tag}")


def read_nbt(path: Path):
    raw = path.read_bytes()
    if raw[:2] == b"\x1f\x8b":
        raw = gzip.decompress(raw)
    buf = io.BytesIO(raw)
    tag = _read(">b", buf)
    if tag != 10:
        raise ValueError(f"{path}: root tag {TAG_NAMES.get(tag, tag)}, want compound")
    _read_string(buf)  # root name
    return _read_payload(10, buf)


# --- comparison -------------------------------------------------------------


def map_palette_name(name: str) -> str:
    for valley, cherry in ALLOWED_PAIRS:
        if name == valley:
            return cherry
    return name


def compare_structures(a: Path, b: Path) -> list[str]:
    errs: list[str] = []
    na, nb = read_nbt(a), read_nbt(b)
    for key in ("size", "DataVersion", "entities"):
        if na.get(key) != nb.get(key):
            errs.append(f"`{key}` differs")
    if na.get("blocks") != nb.get("blocks"):
        errs.append("block positions/states differ (geometry fork, not a palette row)")
    pa, pb = na.get("palette", []), nb.get("palette", [])
    if len(pa) != len(pb):
        errs.append(f"palette lengths differ ({len(pa)} vs {len(pb)})")
        return errs
    for i, (ea, eb) in enumerate(zip(pa, pb)):
        name_a, name_b = ea.get("Name"), eb.get("Name")
        if map_palette_name(name_a) != name_b and name_a != name_b:
            errs.append(f"palette[{i}] `{name_a}` vs `{name_b}` is not an allowed flora pair")
        if ea.get("Properties") != eb.get("Properties"):
            errs.append(f"palette[{i}] Properties differ")
    return errs


def compare_text(a: Path, b: Path) -> list[str]:
    ta = a.read_text(encoding="utf-8")
    tb = b.read_text(encoding="utf-8")
    for valley, cherry in ALLOWED_PAIRS:
        ta = ta.replace(valley, cherry)
    if ta != tb:
        return ["text differs beyond the allowed flora/biome ids"]
    return []


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__)
        return 2
    valley, cherry = Path(sys.argv[1]), Path(sys.argv[2])
    if not valley.is_dir() or not cherry.is_dir():
        print(f"not directories: {valley} / {cherry}")
        return 2

    files_a = {p.relative_to(valley) for p in valley.rglob("*") if p.is_file()}
    files_b = {p.relative_to(cherry) for p in cherry.rglob("*") if p.is_file()}
    failures: list[str] = []
    for only, side in ((files_a - files_b, "valley"), (files_b - files_a, "cherry")):
        for rel in sorted(only):
            failures.append(f"{rel}: present only in the {side} emission")

    checked = 0
    for rel in sorted(files_a & files_b):
        if str(rel) in DECLARATION_MIRRORS:
            continue
        pa, pb = valley / rel, cherry / rel
        if pa.read_bytes() == pb.read_bytes():
            continue
        checked += 1
        if rel.suffix == ".nbt":
            errs = compare_structures(pa, pb)
        else:
            try:
                errs = compare_text(pa, pb)
            except UnicodeDecodeError:
                errs = ["binary file differs and is not structure NBT"]
        failures.extend(f"{rel}: {e}" for e in errs)

    if failures:
        print("flora-parity FAIL (cherry-valley is forking, not parameterizing):")
        for f in failures:
            print(f"  - {f}")
        return 1
    print(
        f"flora-parity OK: {checked} differing file(s) confined to flora/palette "
        f"block ids + biome id; {len(files_a & files_b) - checked} byte-identical"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
