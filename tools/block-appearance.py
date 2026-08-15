#!/usr/bin/env python3
"""What a block actually LOOKS like, measured from the pinned client jar.

A block's name is not its appearance and repeatedly is not close to it:
`packed_mud` is orange, `dried_kelp_block` is a woven olive-green, and
`lightning_rod` is signal orange. Choosing a palette from memory therefore
produces a piece whose colours nobody chose, and the mistake only surfaces at
render time — a whole authoring round late.

So palette selection is a **query**, not a recollection:

    tools/block-appearance.py --id minecraft:packed_mud          # what colour IS it
    tools/block-appearance.py --near '#6b6b6b' -n 12             # what is this colour
    tools/block-appearance.py --list --full-cube-only            # the whole shelf

and it ends in a screen, a measurement of the mix, and a LOOK:

    # "a pale, cool ashlar for a nave wall": 1146 blocks down to 14
    tools/block-appearance.py --screen --where full_cube --where 'L>=0.75' \\
        --where 'L<=0.95' --where 'C_mean<0.02' --where 'texture_range<=0.30'

    # what a weighted paint actually reads as — four numbers, never a mean
    tools/block-appearance.py --mix 'sandstone=3,smooth_sandstone=3,andesite=4'

    # the shortlist and the mixes as pixels, for whoever chooses to look at
    tools/block-appearance.py --screen --where full_cube --where 'L>=0.75' \\
        --sheet .sheets/palette/nave.png

## What is measured

For a block's **default state**: resolve `assets/minecraft/blockstates/<id>.json`
to a model, walk the model's `parent` chain merging `textures`, and collect the
textures its faces reference. Over every alpha-covered pixel of those textures,
converted to **Oklab**:

`L` · `L_p05` / `L_p95` / `L_sd` and `texture_range` (= `L_p95 - L_p05`, how loud
the pattern is) · `C_mean` · `C_p90` / `C_max` (the outlier tail) · `hue`
(chroma-weighted, degrees). Plus the mean `rgb`, `coverage` (mean alpha, 0-255 —
a fence is mostly empty space) and `full_cube` (whether the model's geometry
fills the cell).

**Why not just the mean.** Averaging is right when blocks are smaller than the
viewer's resolution — the mapart case, where the eye fuses them. A delve is
walked at player scale, so each block is a distinct visible patch and a palette
reads by its extremes and their area share. Measured: swapping half of a
sandstone mix for calcite and polished diorite moves the mean **13.5 RGB units**
— nothing — while the chromatic area falls from 60% to 30%. `--mix` reports the
statistic that moved.

Classification — form, material family, gravity, technical, biome-tinted — needs
no jar and comes from `crates/compiler/data/block-classification-1.21.11.json`
(`tools/extract-block-classification.py`).

## What it is not

Not a renderer, and not a chooser. Constraints **eliminate**; they never score.
What survives a screen is a shelf of equals, and the last step is a look — at the
swatch sheet here, then at the geometry in light via `delve-render piece`.
Measurement can prove a mix is not warm; only a look decides it is right.

The jar is EULA-gated and never committed (versions.toml [render]). It is
resolved from --jar, then $DELVEWRIGHT_CLIENT_JAR, then
~/.chunky/resources/minecraft.jar — the same order `delve-render` uses, so one
machine setup serves both. Without it this tool **refuses**; it never reports a
partial answer as a whole one.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import struct
import sys
import zipfile
import zlib
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
REGISTRY = REPO / "crates" / "compiler" / "data" / "blocks-1.21.11.json"
CLASSIFICATION = REPO / "crates" / "compiler" / "data" / "block-classification-1.21.11.json"

# Generation-time working material only — gitignored, never shipped, and unable
# to move a delve's bytes (ADR-0006) or carry a licence into one (ADR-0013).
WORKDIR = REPO / ".sheets" / "palette"

# Texture keys a model with no `elements` might still be described by, in the
# order vanilla's own block models tend to define them.
FALLBACK_TEXTURE_KEYS = ("all", "texture", "particle", "side", "top", "still", "cross")

# Technical blocks: real 1.21.11 blocks that are never building material. A
# colour query ranks by colour alone, so without this it cheerfully proposes
# `structure_block` for "dark grey stone" — a block `delve-admit` hard-forbids as
# a code-injection vector, and one no reviewer would have questioned in a palette
# list. Ranking by appearance and filtering by role are two different jobs.
TECHNICAL = {
    "minecraft:air",
    "minecraft:cave_air",
    "minecraft:void_air",
    "minecraft:barrier",
    "minecraft:light",
    "minecraft:structure_void",
    "minecraft:structure_block",
    "minecraft:jigsaw",
    "minecraft:command_block",
    "minecraft:chain_command_block",
    "minecraft:repeating_command_block",
    "minecraft:spawner",
    "minecraft:trial_spawner",
    "minecraft:vault",
    "minecraft:moving_piston",
    "minecraft:end_portal",
    "minecraft:end_gateway",
    "minecraft:nether_portal",
    "minecraft:bedrock",
}

# Blocks whose final colour is decided by the biome, not the texture.
TINTED_SUFFIXES = ("_leaves",)
TINTED_EXACT = {
    "minecraft:grass_block",
    "minecraft:short_grass",
    "minecraft:tall_grass",
    "minecraft:fern",
    "minecraft:large_fern",
    "minecraft:vine",
    "minecraft:sugar_cane",
    "minecraft:water",
    "minecraft:water_cauldron",
    "minecraft:bubble_column",
    "minecraft:lily_pad",
}

# Blocks that fall when unsupported (vanilla `FallingBlock`). NOT a second
# opinion: this is the set `crates/compiler/src/assembled.rs::is_falling_block`
# owns for `DW0313`, and `tools/tests/test_block_appearance.py` reads that
# function's own source and fails if the two ever disagree — so the palette layer
# cannot drift into a private gravity model.
GRAVITY_EXACT = {
    "minecraft:sand",
    "minecraft:red_sand",
    "minecraft:gravel",
    "minecraft:anvil",
    "minecraft:chipped_anvil",
    "minecraft:damaged_anvil",
    "minecraft:dragon_egg",
}
GRAVITY_SUFFIXES = ("_concrete_powder",)


def is_gravity(block_id: str) -> bool:
    return block_id in GRAVITY_EXACT or block_id.endswith(GRAVITY_SUFFIXES)


def is_tinted(block_id: str) -> bool:
    return block_id in TINTED_EXACT or block_id.endswith(TINTED_SUFFIXES)


# --------------------------------------------------------------------------
# Oklab
#
# Björn Ottosson's published transform (public domain reference formulation,
# 2020). Written from the published matrices; no implementation is reproduced.
# Chosen because its lightness axis is perceptually uniform and its chroma is a
# plain Euclidean radius in (a, b) — which is the whole point here: "how coloured
# is this block" has to be ONE comparable number across the shelf, and in sRGB or
# HSL it is not.
# --------------------------------------------------------------------------

_LINEAR = [
    (c / 255.0) / 12.92 if (c / 255.0) <= 0.04045 else (((c / 255.0) + 0.055) / 1.055) ** 2.4
    for c in range(256)
]
_OKLAB_CACHE: dict[tuple[int, int, int], tuple[float, float, float]] = {}


def oklab(r: int, g: int, b: int) -> tuple[float, float, float]:
    """sRGB 0-255 → Oklab (L, a, b)."""
    key = (r, g, b)
    hit = _OKLAB_CACHE.get(key)
    if hit is not None:
        return hit
    lr, lg, lb = _LINEAR[r], _LINEAR[g], _LINEAR[b]
    l = 0.4122214708 * lr + 0.5363325363 * lg + 0.0514459929 * lb
    m = 0.2119034982 * lr + 0.6806995451 * lg + 0.1073969566 * lb
    s = 0.0883024619 * lr + 0.2817188376 * lg + 0.6299787005 * lb
    l_, m_, s_ = l ** (1 / 3), m ** (1 / 3), s ** (1 / 3)
    out = (
        0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_,
        1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_,
        0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_,
    )
    _OKLAB_CACHE[key] = out
    return out


def percentile(sorted_values: list[float], p: float) -> float:
    """Linear-interpolated percentile of an already-sorted list."""
    n = len(sorted_values)
    if n == 1:
        return sorted_values[0]
    k = (n - 1) * p
    lo, hi = math.floor(k), math.ceil(k)
    if lo == hi:
        return sorted_values[lo]
    return sorted_values[lo] * (hi - k) + sorted_values[hi] * (k - lo)


# The chroma above which a block reads as *coloured* rather than as a neutral.
# Derived, not chosen: it is the 30th percentile of `C_mean` over the 409
# full-cube blocks of the pinned 1.21.11 shelf. If a later batch shows it
# mis-binds it is RE-DERIVED from the distribution — never loosened so that a
# mix passes.
CHROMATIC_THRESHOLD = 0.03


# --------------------------------------------------------------------------
# A dependency-free PNG reader.
#
# Deliberately stdlib-only: this tool has to run on any machine that has the jar,
# including a fresh checkout with no Python packages installed, and a palette
# step an agent cannot run is a palette step that gets skipped.
#
# Written from the PNG specification (W3C/ISO 15948) — the five filter types and
# the sub-byte sample packing are the format's own definition, not a port of any
# implementation. No third-party code is reproduced here.
# --------------------------------------------------------------------------


def decode_png(data: bytes) -> tuple[int, int, list[tuple[int, int, int, int]]]:
    """Return (width, height, RGBA pixels). Raises ValueError on anything unhandled."""
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError("not a PNG")
    pos = 8
    idat = bytearray()
    palette: list[tuple[int, int, int]] = []
    trns: list[int] = []
    width = height = depth = ctype = 0
    while pos < len(data):
        (length,) = struct.unpack(">I", data[pos : pos + 4])
        kind = data[pos + 4 : pos + 8]
        body = data[pos + 8 : pos + 8 + length]
        pos += 12 + length
        if kind == b"IHDR":
            width, height, depth, ctype, _, _, interlace = struct.unpack(">IIBBBBB", body)
            if interlace:
                raise ValueError("interlaced PNG")
            if depth not in (1, 2, 4, 8, 16):
                raise ValueError(f"bit depth {depth}")
            if depth < 8 and ctype not in (0, 3):
                raise ValueError(f"bit depth {depth} with colour type {ctype}")
        elif kind == b"PLTE":
            palette = [tuple(body[i : i + 3]) for i in range(0, len(body), 3)]
        elif kind == b"tRNS":
            trns = list(body)
        elif kind == b"IDAT":
            idat += body
        elif kind == b"IEND":
            break

    channels = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}[ctype]
    sample = max(1, depth // 8)
    # Sub-byte depths pack several samples per byte; vanilla block textures are
    # often 4-bit indexed, which is why this branch exists at all.
    stride = (width * channels * depth + 7) // 8
    raw = zlib.decompress(bytes(idat))

    out = bytearray(stride * height)
    bpp = max(1, channels * depth // 8)
    prev = bytearray(stride)
    src = 0
    for row in range(height):
        f = raw[src]
        src += 1
        line = bytearray(raw[src : src + stride])
        src += stride
        if f == 1:
            for i in range(bpp, stride):
                line[i] = (line[i] + line[i - bpp]) & 0xFF
        elif f == 2:
            for i in range(stride):
                line[i] = (line[i] + prev[i]) & 0xFF
        elif f == 3:
            for i in range(stride):
                left = line[i - bpp] if i >= bpp else 0
                line[i] = (line[i] + ((left + prev[i]) >> 1)) & 0xFF
        elif f == 4:
            for i in range(stride):
                a = line[i - bpp] if i >= bpp else 0
                b = prev[i]
                c = prev[i - bpp] if i >= bpp else 0
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                pred = a if (pa <= pb and pa <= pc) else (b if pb <= pc else c)
                line[i] = (line[i] + pred) & 0xFF
        elif f != 0:
            raise ValueError(f"filter {f}")
        out[row * stride : (row + 1) * stride] = line
        prev = line

    def sample_at(row: int, index: int) -> int:
        """One sample of one row, at any of the five legal bit depths."""
        if depth >= 8:
            return out[row * stride + index * sample]
        per_byte = 8 // depth
        byte = out[row * stride + index // per_byte]
        shift = 8 - depth * (index % per_byte + 1)
        return (byte >> shift) & ((1 << depth) - 1)

    pixels: list[tuple[int, int, int, int]] = []
    for i in range(width * height):
        row, col = divmod(i, width)
        vals = [sample_at(row, col * channels + c) for c in range(channels)]
        if ctype == 0:
            # Sub-byte greyscale is an N-bit level, not an 8-bit one.
            g = vals[0] * 255 // ((1 << depth) - 1) if depth < 8 else vals[0]
            pixels.append((g, g, g, 255))
        elif ctype == 4:
            g = vals[0]
            pixels.append((g, g, g, vals[1]))
        elif ctype == 2:
            pixels.append((vals[0], vals[1], vals[2], 255))
        elif ctype == 6:
            pixels.append((vals[0], vals[1], vals[2], vals[3]))
        else:  # indexed
            idx = vals[0]
            r, g, b = palette[idx] if idx < len(palette) else (0, 0, 0)
            a = trns[idx] if idx < len(trns) else 255
            pixels.append((r, g, b, a))
    return width, height, pixels


# --------------------------------------------------------------------------
# Asset resolution
# --------------------------------------------------------------------------


class Jar:
    def __init__(self, path: Path):
        self.zip = zipfile.ZipFile(path)
        self.names = set(self.zip.namelist())
        self._cache: dict[str, object] = {}

    def json_at(self, path: str):
        if path in self._cache:
            return self._cache[path]
        value = json.loads(self.zip.read(path)) if path in self.names else None
        self._cache[path] = value
        return value

    def png_at(self, path: str):
        if path in self._cache:
            return self._cache[path]
        value = None
        if path in self.names:
            try:
                value = decode_png(self.zip.read(path))
            except ValueError:
                value = None
        self._cache[path] = value
        return value


def split_id(block_id: str) -> tuple[str, str]:
    ns, _, name = block_id.partition(":")
    return (ns, name) if name else ("minecraft", ns)


def default_model(jar: Jar, block_id: str) -> str | None:
    """The model of a block's *default* state."""
    ns, name = split_id(block_id)
    doc = jar.json_at(f"assets/{ns}/blockstates/{name}.json")
    if doc is None:
        return None
    if "variants" in doc:
        # The default state is not recorded in the client assets, so take the
        # lexicographically first variant key. For the appearance of a MATERIAL
        # this is stable and adequate: variants of one block differ by facing and
        # waterlogging, not by material.
        for key in sorted(doc["variants"]):
            entry = doc["variants"][key]
            if isinstance(entry, list):
                entry = entry[0]
            if isinstance(entry, dict) and "model" in entry:
                return entry["model"]
        return None
    if "multipart" in doc:
        for case in doc["multipart"]:
            entry = case.get("apply")
            if isinstance(entry, list):
                entry = entry[0]
            if isinstance(entry, dict) and "model" in entry:
                return entry["model"]
    return None


def resolve_model(jar: Jar, model_ref: str) -> tuple[dict, list]:
    """Walk the `parent` chain: merged textures, and the nearest elements."""
    textures: dict[str, str] = {}
    elements: list = []
    seen: set[str] = set()
    ref = model_ref
    for _ in range(32):
        if ref in seen:
            break
        seen.add(ref)
        ns, name = split_id(ref)
        doc = jar.json_at(f"assets/{ns}/models/{name}.json")
        if doc is None:
            break
        for k, v in doc.get("textures", {}).items():
            textures.setdefault(k, v)
        if not elements and doc.get("elements"):
            elements = doc["elements"]
        parent = doc.get("parent")
        if not parent:
            break
        ref = parent
    return textures, elements


def texture_paths(textures: dict[str, str], elements: list) -> list[str]:
    """Every texture the model's faces reference, resolved through `#refs`."""

    def resolve(value: str) -> str | None:
        for _ in range(16):
            if not value.startswith("#"):
                return value
            value = textures.get(value[1:], "")
            if not value:
                return None
        return None

    refs: list[str] = []
    if elements:
        for element in elements:
            for face in element.get("faces", {}).values():
                t = face.get("texture")
                if t:
                    refs.append(t)
    if not refs:
        for key in FALLBACK_TEXTURE_KEYS:
            if key in textures:
                refs.append(f"#{key}")
    out: list[str] = []
    for ref in refs:
        path = resolve(ref)
        if path and path not in out:
            out.append(path)
    return sorted(out)


def is_full_cube(elements: list) -> bool:
    if not elements:
        return False
    for element in elements:
        frm, to = element.get("from"), element.get("to")
        if frm == [0, 0, 0] and to == [16, 16, 16]:
            return True
    return False


def block_pixels(jar: Jar, block_id: str) -> tuple[list[tuple[int, int, int, int]], list, list[str]] | None:
    """Every pixel of every texture a block's default-state model references."""
    model_ref = default_model(jar, block_id)
    if model_ref is None:
        return None
    textures, elements = resolve_model(jar, model_ref)
    paths = texture_paths(textures, elements)
    if not paths:
        return None
    pixels: list[tuple[int, int, int, int]] = []
    for path in paths:
        ns, name = split_id(path)
        png = jar.png_at(f"assets/{ns}/textures/{name}.png")
        if png is None:
            continue
        width, height, px = png
        # An animated texture is a vertical strip; only the first frame is the
        # block's resting appearance.
        if height > width and width > 0 and height % width == 0:
            px = px[: width * width]
        pixels += px
    return pixels, elements, paths


def appearance(jar: Jar, block_id: str, classification: dict | None = None) -> dict | None:
    got = block_pixels(jar, block_id)
    if got is None:
        return None
    pixels, elements, paths = got

    r = g = b = 0.0
    alpha_sum = 0.0
    count = len(pixels)
    lightness: list[float] = []
    chroma: list[float] = []
    a_sum = b_sum = 0.0
    for pr, pg, pb, pa in pixels:
        a = pa / 255.0
        r += pr * a
        g += pg * a
        b += pb * a
        alpha_sum += a
        if pa == 0:
            continue
        # The alpha-covered pixels ARE the block's surface; a fully transparent
        # pixel is a hole, and averaging a hole's colour in would report the
        # texture sheet's padding as material.
        ok_l, ok_a, ok_b = oklab(pr, pg, pb)
        lightness.append(ok_l)
        chroma.append(math.hypot(ok_a, ok_b))
        a_sum += ok_a
        b_sum += ok_b
    if count == 0 or alpha_sum == 0 or not lightness:
        return None
    lightness.sort()
    chroma.sort()
    n = len(lightness)
    mean_l = sum(lightness) / n
    l_p05, l_p95 = percentile(lightness, 0.05), percentile(lightness, 0.95)
    variance = sum((v - mean_l) ** 2 for v in lightness) / n
    row = {
        "id": block_id,
        "rgb": [round(r / alpha_sum), round(g / alpha_sum), round(b / alpha_sum)],
        "coverage": round(255 * alpha_sum / count),
        "full_cube": is_full_cube(elements),
        "textures": paths,
        "tinted": is_tinted(block_id),
        "gravity": is_gravity(block_id),
        "technical": block_id in TECHNICAL,
        "L": mean_l,
        "L_p05": l_p05,
        "L_p95": l_p95,
        "L_sd": math.sqrt(variance),
        "texture_range": l_p95 - l_p05,
        "C_mean": sum(chroma) / n,
        "C_p90": percentile(chroma, 0.90),
        "C_max": chroma[-1],
        "hue": math.degrees(math.atan2(b_sum, a_sum)) % 360.0,
        "pixels": n,
    }
    if classification is not None:
        entry = classification.get(block_id, {})
        row["family"] = entry.get("family")
        row["form"] = entry.get("form")
    return row


def load_classification() -> dict:
    """The no-jar half of the table. Absence is a REFUSAL, not a missing column.

    Reporting `family: null` for every block would let a family-scoped screen or
    a per-family budget run and come back green over nothing — the unbound
    vacuity, and the one shape this table exists to make impossible.
    """
    if not CLASSIFICATION.exists():
        raise SystemExit(
            f"refusing: {CLASSIFICATION} is absent, so form and material family "
            f"cannot be answered.\n"
            f"Regenerate it: python3 tools/extract-block-classification.py "
            f"<tag/block/data.min.json> <recipe/data.min.json> {CLASSIFICATION}"
        )
    return json.loads(CLASSIFICATION.read_text())["blocks"]


# --------------------------------------------------------------------------


# --------------------------------------------------------------------------
# The screen: constraints ELIMINATE, they never score.
#
# A ranked list answers a question the author did not ask. They do not know the
# target hex; they know a fiction ("cold northern limestone"), and ranking 1146
# blocks by distance to a guessed colour returns fifteen rows most of which are
# wrong for reasons that have nothing to do with colour. A facet screen states
# the fiction as constraints on measured axes and narrows to a shelf of equals.
#
# The residue is honest and is not a bug: a screen for "pale, cool, quiet,
# full-cube" still returns a light source, a gravity block, wool and a metal —
# four blocks right on every measured axis and wrong for a nave wall. Light
# emission is not in any vanilla data branch at all. That boundary is why the
# leaf is a LOOK.
# --------------------------------------------------------------------------

BOOLEAN_FIELDS = ("full_cube", "tinted", "gravity", "technical")
TEXT_FIELDS = ("id", "family", "form")
NUMERIC_FIELDS = (
    "L",
    "L_p05",
    "L_p95",
    "L_sd",
    "texture_range",
    "C_mean",
    "C_p90",
    "C_max",
    "hue",
    "coverage",
)
_OPS = ("<=", ">=", "!=", "<", ">", "=")


def parse_where(expr: str):
    """`full_cube` · `not tinted` · `L>=0.75` · `form=slab` · `family!=minecraft:sand`."""
    text = expr.strip()
    negate = False
    if text.startswith("not "):
        negate, text = True, text[4:].strip()
    if text in BOOLEAN_FIELDS:
        field = text
        return lambda row: bool(row.get(field)) != negate
    for op in _OPS:
        if op in text:
            field, _, raw = text.partition(op)
            field, raw = field.strip(), raw.strip()
            if field in NUMERIC_FIELDS:
                try:
                    value = float(raw)
                except ValueError:
                    raise SystemExit(f"--where {expr!r}: {raw!r} is not a number")
                cmps = {
                    "<": lambda a, b: a < b,
                    "<=": lambda a, b: a <= b,
                    ">": lambda a, b: a > b,
                    ">=": lambda a, b: a >= b,
                    "=": lambda a, b: a == b,
                    "!=": lambda a, b: a != b,
                }
                fn = cmps[op]
                return lambda row: row.get(field) is not None and fn(row[field], value)
            if field in TEXT_FIELDS:
                if op not in ("=", "!="):
                    raise SystemExit(f"--where {expr!r}: {field} takes only = or !=")
                want = raw if ":" in raw or field == "form" else f"minecraft:{raw}"
                if op == "=":
                    return lambda row: row.get(field) == want
                return lambda row: row.get(field) != want
            raise SystemExit(
                f"--where {expr!r}: unknown field {field!r}. Numeric: "
                f"{', '.join(NUMERIC_FIELDS)}; text: {', '.join(TEXT_FIELDS)}; "
                f"boolean: {', '.join(BOOLEAN_FIELDS)}"
            )
    raise SystemExit(f"--where {expr!r}: expected `field`, `not field`, or `field OP value`")


def run_screen(rows: list[dict], exprs: list[str]) -> tuple[list[dict], list[tuple[str, int]]]:
    """Apply each constraint in order; return the survivors and the cascade."""
    cascade = [("all candidate blocks", len(rows))]
    live = rows
    for expr in exprs:
        keep = parse_where(expr)
        live = [row for row in live if keep(row)]
        cascade.append((expr, len(live)))
    return live, cascade


# --------------------------------------------------------------------------
# The mix report: four numbers, and never a mean as the verdict.
#
# A weighted paint's mean colour is the statistic that cannot see the failure it
# is asked about. The measured case: a "pale ashlar" whose mean sat within 15 RGB
# units of its intended target read as an Egyptian desert temple, because 60% of
# its AREA was sandstone-family. Swap half of that for calcite and polished
# diorite and the mean moves 13.5 units — nothing — while the chromatic area
# halves. So the report names the loud member and its area share, which is the
# thing the 60/30/10 craft rule is actually about.
# --------------------------------------------------------------------------


def parse_mix(spec: str) -> list[tuple[str, float]]:
    """`sandstone=3,smooth_sandstone=3,andesite=4` → normalised area shares."""
    members: list[tuple[str, float]] = []
    for part in spec.split(","):
        part = part.strip()
        if not part:
            continue
        block, sep, raw = part.partition("=")
        if not sep:
            block, raw = part, "1"
        block = block.strip()
        block = block if ":" in block else f"minecraft:{block}"
        try:
            weight = float(raw)
        except ValueError:
            raise SystemExit(f"--mix {spec!r}: {raw!r} is not a weight")
        if weight <= 0:
            raise SystemExit(f"--mix {spec!r}: weight for {block} must be positive")
        members.append((block, weight))
    if not members:
        raise SystemExit(f"--mix {spec!r}: no members")
    total = sum(w for _, w in members)
    return [(b, w / total) for b, w in members]


# Air is a paint member like any other — it is what makes a mix "a material that
# is partly not there", and it is the whole of decay in the grammar. It has area
# and no colour, so it dilutes chroma rather than being excluded from the
# denominator: a wall that is 40% holes IS less coloured than the same wall
# solid. It is reported separately as `void_area` so an author never has to infer
# it from a number that quietly absorbed it.
VOID_BLOCKS = {"minecraft:air", "minecraft:cave_air", "minecraft:void_air"}


def mix_report(name: str, members: list[tuple[str, float]], by_id: dict[str, dict]) -> dict:
    missing = [b for b, _ in members if b not in by_id and b not in VOID_BLOCKS]
    if missing:
        raise SystemExit(
            f"refusing to report mix {name!r}: no measurement for {', '.join(missing)}. "
            f"A mix reported over the members that happened to resolve is a mean of a "
            f"different paint."
        )
    void_area = sum(share for b, share in members if b in VOID_BLOCKS)
    rows = [(by_id[b], share) for b, share in members if b not in VOID_BLOCKS]

    # Weighted over the WHOLE paint, void included at zero chroma.
    chroma_mass = sum(row["C_mean"] * share for row, share in rows)
    chromatic_area = sum(
        share for row, share in rows if row["C_mean"] >= CHROMATIC_THRESHOLD
    )
    loudest = (
        max(rows, key=lambda rs: (rs[0]["C_p90"], rs[0]["id"])) if rows else None
    )
    # The hue of the COLOURED part: weighting by chroma is what stops a large
    # neutral share from steering the answer towards its own numerical noise.
    a_sum = sum(
        math.cos(math.radians(row["hue"])) * row["C_mean"] * share for row, share in rows
    )
    b_sum = sum(
        math.sin(math.radians(row["hue"])) * row["C_mean"] * share for row, share in rows
    )
    # The mean colour is of the MATERIAL, so it is renormalised over the solid
    # share — averaging a hole's colour in would report the void as a dark grey.
    solid = 1.0 - void_area
    mean_rgb = (
        [sum(row["rgb"][i] * share for row, share in rows) / solid for i in range(3)]
        if solid > 0
        else [0, 0, 0]
    )
    return {
        "name": name,
        "members": [
            {
                "id": row["id"],
                "area_share": share,
                "C_mean": row["C_mean"],
                "C_p90": row["C_p90"],
                "family": row.get("family"),
            }
            for row, share in rows
        ],
        "void_area": void_area,
        # The count the BINDING is stated from. Air is a member, so a paint of
        # stone + air is a two-member mix; deriving the count from `members`
        # alone would under-report every eroded role in the corpus as a solid.
        "member_count": len(rows) + (1 if void_area > 0 else 0),
        "chroma_mass": chroma_mass,
        "chromatic_area": chromatic_area,
        "chromatic_threshold": CHROMATIC_THRESHOLD,
        "loudest_member": (
            {"id": loudest[0]["id"], "area_share": loudest[1]} if loudest else None
        ),
        "dominant_hue": (math.degrees(math.atan2(b_sum, a_sum)) % 360.0)
        if (a_sum or b_sum)
        else None,
        # Present, and deliberately never alone: this is the number that did not
        # move when the building moved continents.
        "mean_rgb_not_a_verdict": [round(v) for v in mean_rgb],
    }


def program_mixes(doc: dict) -> list[tuple[str, list[tuple[str, float]]]]:
    """Every weighted paint in a grammar program: palette roles and inline fills.

    A paint is a block-state string or a weighted list, in `palette` and equally
    on any `fill`. Reading only the named roles would leave every inline mix
    unmeasured — the same shape as a walk that enumerates three of five effect
    roots.

    A paint written in the scope's own axis frame is that same string or list
    wrapped as `{"local": ...}`. The frame decides which world direction a
    property names; it moves no block and changes no colour, so a local paint is
    unwrapped and measured exactly like a bare one. Skipping the wrapper instead
    is how a program whose whole palette is local reports NOTHING and prints the
    shelf.
    """
    out: list[tuple[str, list[tuple[str, float]]]] = []

    def unwrap(value):
        """A `{"local": ...}` paint is its inner states; anything else is itself."""
        if isinstance(value, dict) and "local" in value:
            return value["local"]
        return value

    def paint(label: str, value) -> None:
        value = unwrap(value)
        if isinstance(value, str):
            out.append((label, [(base_block(value), 1.0)]))
        elif isinstance(value, list):
            members = []
            for entry in value:
                if not isinstance(entry, dict) or "block" not in entry:
                    continue
                members.append((base_block(entry["block"]), float(entry.get("weight", 1))))
            total = sum(w for _, w in members)
            if total > 0:
                out.append((label, [(b, w / total) for b, w in members]))

    for role, value in sorted((doc.get("palette") or {}).items()):
        paint(f"palette.{role}", value)

    def is_paint(material) -> bool:
        """An inline paint, as against a `{"role": ...}` reference to a named one."""
        return isinstance(unwrap(material), (str, list))

    def walk(node, path: str) -> None:
        if isinstance(node, dict):
            if node.get("op") == "fill" and is_paint(node.get("material")):
                paint(f"fill@{path}", node["material"])
            for key in sorted(node):
                walk(node[key], f"{path}/{key}")
        elif isinstance(node, list):
            for i, item in enumerate(node):
                walk(item, f"{path}[{i}]")

    walk(doc.get("rules") or {}, "rules")
    return out


def base_block(state: str) -> str:
    """`minecraft:oak_stairs[facing=east]` → `minecraft:oak_stairs`."""
    name = state.split("[", 1)[0].strip()
    return name if ":" in name else f"minecraft:{name}"


# --------------------------------------------------------------------------
# The leaf: a swatch sheet. No GPU, no Chunky, no world.
#
# The screen hands over ten to twenty survivors, and a shortlist is not a choice.
# What decides it is seeing the material — its pattern, its scale, and how a mix
# reads as a field rather than as a row of numbers. So the handoff to whoever
# chooses is PIXELS: each survivor tiled, each candidate mix rendered as its
# seeded weighted tiling, which is literally the wall at distance zero. An agent
# has vision; a ranked list wastes it.
#
# Labels are drawn with the jar's own `ascii.png` glyph sheet rather than an
# invented font — the sheet already carries jar pixels, and inventing a second
# font is data nobody can check.
# --------------------------------------------------------------------------

FONT_SHEET = "assets/minecraft/textures/font/ascii.png"
GLYPH = 8
ADVANCE = GLYPH - 2  # the sheet is a fixed 8x8 grid; 6px reads without touching
TILE = 16  # a vanilla block texture is 16x16
SWATCH_TILES = 4  # four blocks square: enough for a pattern to repeat


class Canvas:
    def __init__(self, width: int, height: int, fill: tuple[int, int, int] = (24, 24, 27)):
        self.w, self.h = width, height
        self.px = bytearray(width * height * 3)
        for i in range(width * height):
            self.px[i * 3 : i * 3 + 3] = bytes(fill)

    def set(self, x: int, y: int, rgb: tuple[int, int, int]) -> None:
        if 0 <= x < self.w and 0 <= y < self.h:
            i = (y * self.w + x) * 3
            self.px[i : i + 3] = bytes(rgb)

    def to_png(self) -> bytes:
        raw = bytearray()
        stride = self.w * 3
        for y in range(self.h):
            raw.append(0)  # filter type 0: the sheet is deterministic, not small
            raw += self.px[y * stride : (y + 1) * stride]
        comp = zlib.compressobj(9, zlib.DEFLATED, 15, 9, zlib.Z_DEFAULT_STRATEGY)
        body = comp.compress(bytes(raw)) + comp.flush()

        def chunk(kind: bytes, data: bytes) -> bytes:
            return (
                struct.pack(">I", len(data))
                + kind
                + data
                + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)
            )

        return (
            b"\x89PNG\r\n\x1a\n"
            + chunk(b"IHDR", struct.pack(">IIBBBBB", self.w, self.h, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", body)
            + chunk(b"IEND", b"")
        )


def load_font(jar: Jar) -> dict[str, list[list[bool]]] | None:
    png = jar.png_at(FONT_SHEET)
    if png is None:
        return None
    width, _, pixels = png
    glyphs: dict[str, list[list[bool]]] = {}
    for code in range(32, 127):
        row, col = divmod(code, 16)
        glyphs[chr(code)] = [
            [
                pixels[(row * GLYPH + y) * width + col * GLYPH + x][3] > 0
                for x in range(GLYPH)
            ]
            for y in range(GLYPH)
        ]
    return glyphs


def draw_text(
    canvas: Canvas, glyphs: dict, x: int, y: int, text: str, rgb=(235, 235, 235)
) -> None:
    for i, ch in enumerate(text):
        glyph = glyphs.get(ch)
        if glyph is None:
            continue
        for gy in range(GLYPH):
            for gx in range(GLYPH):
                if glyph[gy][gx]:
                    canvas.set(x + i * ADVANCE + gx, y + gy, rgb)


def block_tile(jar: Jar, block_id: str) -> list[tuple[int, int, int]] | None:
    """One 16x16 tile of a block's first texture, over a neutral backing."""
    got = block_pixels(jar, block_id)
    if got is None:
        return None
    _, _, paths = got
    ns, name = split_id(paths[0])
    png = jar.png_at(f"assets/{ns}/textures/{name}.png")
    if png is None:
        return None
    width, height, pixels = png
    if height > width and width > 0 and height % width == 0:
        pixels = pixels[: width * width]
        height = width
    out: list[tuple[int, int, int]] = []
    for ty in range(TILE):
        for tx in range(TILE):
            sx, sy = tx * width // TILE, ty * height // TILE
            r, g, b, a = pixels[sy * width + sx]
            f = a / 255.0
            out.append(
                (
                    round(r * f + 24 * (1 - f)),
                    round(g * f + 24 * (1 - f)),
                    round(b * f + 24 * (1 - f)),
                )
            )
    return out


class Splitmix:
    """A tiny seeded stream. Deterministic and self-contained, so the sheet's
    weighted tiling depends on the seed and on nothing else (ADR-0006)."""

    def __init__(self, seed: int):
        self.state = seed & 0xFFFFFFFFFFFFFFFF

    def next(self) -> int:
        self.state = (self.state + 0x9E3779B97F4A7C15) & 0xFFFFFFFFFFFFFFFF
        z = self.state
        z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & 0xFFFFFFFFFFFFFFFF
        z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & 0xFFFFFFFFFFFFFFFF
        return z ^ (z >> 31)


def swatch_sheet(
    jar: Jar,
    shortlist: list[dict],
    mixes: list[tuple[str, list[tuple[str, float]]]],
    seed: int,
    columns: int = 6,
) -> bytes:
    glyphs = load_font(jar)
    if glyphs is None:
        raise SystemExit(f"refusing: the jar has no {FONT_SHEET}, so labels cannot be drawn")
    cell = TILE * SWATCH_TILES
    pad, label_h, header = 8, 12, 20
    labels = [row["id"].removeprefix("minecraft:") for row in shortlist]
    # A label that overlaps its neighbour is a sheet nobody can read the ids off,
    # so the column pitch is set by the longest NAME, not by the tile.
    pitch = max([cell] + [len(text) * ADVANCE for text in labels]) + pad
    mix_w = cell * 2
    rows_of_blocks = (len(shortlist) + columns - 1) // columns if shortlist else 0
    mix_rows = len(mixes)
    mix_label = max([0] + [len(name) * ADVANCE for name, _ in mixes])
    width = max(columns * pitch + pad, mix_w + 2 * pad, mix_label + 2 * pad, 220)
    height = (
        header
        + rows_of_blocks * (cell + label_h + pad)
        + (header + mix_rows * (cell + label_h + pad) if mixes else 0)
        + pad
    )
    canvas = Canvas(width, max(height, header + pad))

    draw_text(canvas, glyphs, pad, 6, f"shortlist {len(shortlist)}  seed {seed}")
    y = header
    for index, row in enumerate(shortlist):
        col, line = index % columns, index // columns
        tile = block_tile(jar, row["id"])
        x = pad + col * pitch
        top = y + line * (cell + label_h + pad)
        if tile is not None:
            for py in range(cell):
                for px in range(cell):
                    canvas.set(x + px, top + py, tile[(py % TILE) * TILE + (px % TILE)])
        draw_text(canvas, glyphs, x, top + cell + 2, labels[index])
    y += rows_of_blocks * (cell + label_h + pad)

    if mixes:
        draw_text(canvas, glyphs, pad, y + 4, f"mixes {len(mixes)}")
        y += header
        for line, (name, members) in enumerate(mixes):
            # A void member is DRAWN, as the flat backing — dropping it would
            # silently renormalise the other weights and paint a solid wall for a
            # mix that is 40% holes.
            tiles = []
            for block, share in members:
                if block in VOID_BLOCKS:
                    tiles.append(([(16, 16, 18)] * (TILE * TILE), share))
                    continue
                tile = block_tile(jar, block)
                if tile is None:
                    raise SystemExit(
                        f"refusing to draw mix {name!r}: no texture for {block}, and a "
                        f"tiling that skips a member is a picture of a different paint"
                    )
                tiles.append((tile, share))
            if not tiles:
                continue
            rng = Splitmix(seed ^ (zlib.crc32(name.encode()) & 0xFFFFFFFF))
            top = y + line * (cell + label_h + pad)
            for by in range(SWATCH_TILES):
                for bx in range(SWATCH_TILES * 2):
                    draw = (rng.next() >> 11) / float(1 << 53)
                    acc = 0.0
                    chosen = tiles[-1][0]
                    for tile, share in tiles:
                        acc += share
                        if draw < acc:
                            chosen = tile
                            break
                    for py in range(TILE):
                        for px in range(TILE):
                            canvas.set(
                                pad + bx * TILE + px,
                                top + by * TILE + py,
                                chosen[py * TILE + px],
                            )
            draw_text(canvas, glyphs, pad, top + cell + 2, name)
    return canvas.to_png()


def hexcolor(rgb: list[int]) -> str:
    return "#{:02x}{:02x}{:02x}".format(*rgb)


def parse_hex(s: str) -> tuple[int, int, int]:
    s = s.lstrip("#")
    if len(s) != 6:
        raise ValueError(f"{s!r} is not a #rrggbb colour")
    return int(s[0:2], 16), int(s[2:4], 16), int(s[4:6], 16)


def distance(a: tuple[int, int, int], b: list[int]) -> float:
    """Weighted RGB distance — a cheap stand-in for perceptual distance that is
    good enough to RANK candidates, which is all this tool claims to do."""
    rmean = (a[0] + b[0]) / 2
    dr, dg, db = a[0] - b[0], a[1] - b[1], a[2] - b[2]
    return math.sqrt(
        (2 + rmean / 256) * dr * dr + 4 * dg * dg + (2 + (255 - rmean) / 256) * db * db
    )


def load_registry() -> str:
    """The pinned 1.21.11 block list, or a refusal that says what to do instead.

    This tool is a mandatory step of `docs/reference/prefab-procedure.md` §2, and
    it is run from checkouts that do not carry `crates/` — a `FileNotFoundError`
    traceback there tells an author the step is broken rather than which of its
    two inputs is missing and what the fallback is.
    """
    try:
        return REGISTRY.read_text()
    except OSError:
        raise SystemExit(
            f"no block registry at {REGISTRY} — this tool needs the pinned "
            "1.21.11 block list from crates/compiler/data/, and this checkout "
            "does not have it.\n"
            "The palette step is not optional: take role names from the corpus "
            "instead (`delve-grammar list`, then `delve-grammar show --program "
            "<nearest>`), which is a palette that was measured already. Never "
            "name a block from memory — an id that does not exist is refused at "
            "export, and an id that exists but looks nothing like its name is "
            "caught only by eye."
        ) from None


def resolve_jar(explicit: str | None) -> Path:
    candidates = [
        explicit,
        os.environ.get("DELVEWRIGHT_CLIENT_JAR"),
        str(Path.home() / ".chunky" / "resources" / "minecraft.jar"),
    ]
    for c in candidates:
        if c and Path(c).exists():
            return Path(c)
    raise SystemExit(
        "refusing: no client jar, so nothing here can be measured — pass "
        "--jar <1.21.11 client jar>, set $DELVEWRIGHT_CLIENT_JAR, or place it at "
        "~/.chunky/resources/minecraft.jar (the same order delve-render uses). "
        "A palette answer given without the textures is a recollection, which is "
        "the failure this tool exists to remove."
    )


def sheet_path_ok(target: Path) -> bool:
    """A sheet outside the repo is the operator's own business; a sheet INSIDE it
    must land in the gitignored working dir, or the next `git add -A` commits
    Mojang texture pixels."""
    resolved = (target if target.is_absolute() else Path.cwd() / target).resolve()
    try:
        resolved.relative_to(REPO)
    except ValueError:
        return True
    try:
        resolved.relative_to(WORKDIR)
    except ValueError:
        return False
    return True


def print_mix_report(reports: list[dict], roles_examined: int) -> None:
    """A mix report ALWAYS states what it bound to. A zero binding is a finding."""
    multi = [r for r in reports if r["member_count"] >= 2]
    print(f"binding: {roles_examined} paint(s) examined, {len(multi)} mix(es) with >= 2 members")
    if not multi:
        print(
            "FINDING: zero binding — no paint here has two or more members, so this "
            "report proves nothing about any palette. A green report over nothing is "
            "not a pass."
        )
    for report in reports:
        print(f"\n{report['name']}")
        for member in report["members"]:
            print(
                f"  {member['area_share']:6.1%}  {member['id']:<44} "
                f"C_mean {member['C_mean']:.4f}  C_p90 {member['C_p90']:.4f}"
            )
        if report["void_area"]:
            print(f"  {report['void_area']:6.1%}  (air — this much of the paint is not there)")
        hue = report["dominant_hue"]
        loudest = report["loudest_member"]
        print(
            f"  chroma_mass    {report['chroma_mass']:.4f}\n"
            f"  chromatic_area {report['chromatic_area']:.2f}"
            f"   (share of area with C_mean >= {report['chromatic_threshold']})\n"
            f"  loudest_member "
            + (
                f"{loudest['id']} at {loudest['area_share']:.0%} of area"
                if loudest
                else "none — this paint is entirely air"
            )
            + f"\n  dominant_hue   {'n/a (no chroma)' if hue is None else f'{hue:.1f} deg'}\n"
            f"  mean colour    {hexcolor(report['mean_rgb_not_a_verdict'])} "
            f"— of the SOLID share, NOT a verdict; it is the statistic that does not "
            f"move when a mix's loud member does"
        )


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--jar", help="1.21.11 client jar (EULA-gated, never committed)")
    ap.add_argument(
        "--id",
        action="append",
        default=[],
        help="measure this block id; repeatable, and every id given is measured",
    )
    ap.add_argument("--near", help="rank blocks by closeness to a #rrggbb colour")
    ap.add_argument("--list", action="store_true", help="measure every block")
    ap.add_argument("-n", type=int, default=15, help="how many rows to print (--near)")
    ap.add_argument(
        "--technical",
        action="store_true",
        help="include technical blocks (barrier, structure_block, spawner...); excluded by default",
    )
    ap.add_argument(
        "--full-cube-only",
        action="store_true",
        help="only blocks whose model fills the cell — what a wall or a floor is made of",
    )
    ap.add_argument("--json", action="store_true", help="emit JSON instead of a table")
    ap.add_argument(
        "--screen",
        action="store_true",
        help="narrow the whole shelf by --where constraints and print the cascade",
    )
    ap.add_argument(
        "--where",
        action="append",
        default=[],
        metavar="EXPR",
        help="a constraint, applied in the order given: `full_cube`, `not tinted`, "
        "`L>=0.75`, `C_mean<0.02`, `texture_range<=0.30`, `form=slab`, "
        "`family=minecraft:sandstone`. Repeatable; constraints eliminate, never score",
    )
    ap.add_argument(
        "--exclude-tinted",
        action="store_true",
        help="drop biome-tinted blocks (their measured colour is the untinted texture, "
        "i.e. a fiction for grass and leaves)",
    )
    ap.add_argument(
        "--mix",
        action="append",
        default=[],
        metavar="SPEC",
        help="report a weighted paint, e.g. 'sandstone=3,smooth_sandstone=3,andesite=4'. "
        "Repeatable",
    )
    ap.add_argument(
        "--program",
        type=Path,
        help="a grammar program JSON: report every palette role and every inline fill mix",
    )
    ap.add_argument(
        "--sheet",
        type=Path,
        nargs="?",
        const=WORKDIR / "swatches.png",
        help=f"write a swatch sheet PNG of the survivors and mixes "
        f"(default {WORKDIR.relative_to(REPO)}/swatches.png; must stay under a "
        f"gitignored working dir)",
    )
    ap.add_argument("--seed", type=int, default=0, help="seed for --sheet's mix tilings")
    args = ap.parse_args(argv[1:])

    if not (args.id or args.near or args.list or args.screen or args.mix or args.program):
        ap.error("give --id, --near, --list, --screen, --mix or --program")
    if args.where and not args.screen:
        ap.error("--where is part of --screen; add --screen")
    # Checked BEFORE any measurement: a refusal that arrives after the whole
    # screen has printed reads as an afterthought, and the operator has already
    # scrolled past it.
    if args.sheet is not None and not sheet_path_ok(args.sheet):
        print(
            f"refusing to write {args.sheet}: a swatch sheet is generation-time working "
            f"material and belongs under {WORKDIR.relative_to(REPO)} (gitignored). "
            f"Nothing on a sheet may reach a delve's bytes (ADR-0006) or carry a "
            f"licence into one (ADR-0013).",
            file=sys.stderr,
        )
        return 2

    registry = json.loads(load_registry())
    classification = load_classification()
    jar = Jar(resolve_jar(args.jar))

    if args.screen or args.program or args.sheet:
        # A screen, a program's palette, and a sheet all read the WHOLE shelf: a
        # cascade counted over a subset is a different cascade.
        args.list = args.list or not args.id

    if args.id:
        wanted = []
        for raw in args.id:
            block_id = raw if ":" in raw else f"minecraft:{raw}"
            if block_id not in registry:
                print(
                    f"error: {block_id} is not a block in Minecraft 1.21.11 "
                    f"(checked against {REGISTRY.relative_to(REPO)})",
                    file=sys.stderr,
                )
                return 2
            wanted.append(block_id)
    else:
        wanted = sorted(registry)

    rows = []
    unresolved = 0
    for block_id in wanted:
        a = appearance(jar, block_id, classification)
        if a is None:
            unresolved += 1
            continue
        if args.full_cube_only and not a["full_cube"]:
            continue
        if not args.technical and block_id in TECHNICAL and not args.id:
            continue
        if args.exclude_tinted and a["tinted"]:
            continue
        rows.append(a)
    rows.sort(key=lambda a: a["id"])
    by_id = {a["id"]: a for a in rows}

    # ---- the mix report: a measurement, and it always states its binding ----
    mixes: list[tuple[str, list[tuple[str, float]]]] = []
    for index, spec in enumerate(args.mix):
        mixes.append((f"mix{index + 1}", parse_mix(spec)))
    if args.program:
        doc = json.loads(args.program.read_text())
        mixes += program_mixes(doc)
    # Asking for a mix report and getting none back is the finding, so the report
    # is printed on the REQUEST, never on there being something to say. Gating it
    # on a non-empty list left `print_mix_report`'s zero-binding finding — already
    # written, already correct — with nothing that could ever invoke it, and a
    # program whose palette this reader did not understand fell through to the
    # whole-shelf listing and exited 0.
    if mixes or args.mix or args.program:
        reports = [mix_report(name, members, by_id) for name, members in mixes]
        if args.json:
            multi = sum(1 for r in reports if r["member_count"] >= 2)
            print(
                json.dumps(
                    {
                        "mixes": reports,
                        "binding": {
                            "paints_examined": len(reports),
                            "mixes_with_two_or_more_members": multi,
                            "zero_binding_finding": multi == 0,
                        },
                    },
                    indent=2,
                    sort_keys=True,
                )
            )
        else:
            print_mix_report(reports, len(reports))
        if not args.screen and not args.sheet:
            return 0

    # ---- the screen: constraints eliminate, they never score ----
    # Without a screen there is no shortlist, so a sheet shows the blocks the
    # mixes are actually made of. Tiling all 1146 would be a sheet nobody reads.
    seen: dict[str, dict] = {}
    for _, members in mixes:
        for block, _share in members:
            if block in by_id and block not in seen:
                seen[block] = by_id[block]
    survivors = list(seen.values()) if mixes else rows
    if args.screen:
        survivors, cascade = run_screen(rows, args.where)
        if args.json:
            print(
                json.dumps(
                    {
                        "cascade": [{"step": s, "survivors": n} for s, n in cascade],
                        "survivors": survivors,
                    },
                    indent=2,
                    sort_keys=True,
                )
            )
        else:
            print(f"{'step':<44} survivors")
            for step, count in cascade:
                print(f"{step:<44} {count:>9}")
            print()
            print(f"{'block':<44} {'hex':<9} {'L':<6} {'C_mean':<7} {'range':<6} form")
            for a in survivors:
                print(
                    f"{a['id']:<44} {hexcolor(a['rgb']):<9} {a['L']:<6.3f} "
                    f"{a['C_mean']:<7.4f} {a['texture_range']:<6.3f} {a.get('form')}"
                )
            print(
                f"\n{len(survivors)} survivor(s). What survives is a shelf of EQUALS, not a "
                f"ranking — measurement cannot tell you which of these is your material.\n"
                f"Some survivors will be right on every measured axis and wrong for the job "
                f"(a light source, a gravity block, wool): light emission is in no vanilla\n"
                f"data branch at all. LOOK at them: re-run with --sheet, then put the winner "
                f"on real geometry with `delve-render piece <prefab.nbt> -o <dir>`."
            )
        if not args.sheet:
            return 0

    # ---- the leaf: pixels ----
    if args.sheet:
        out = args.sheet if args.sheet.is_absolute() else Path.cwd() / args.sheet
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_bytes(swatch_sheet(jar, survivors, mixes, args.seed))
        print(f"wrote {out} — {len(survivors)} swatch(es), {len(mixes)} mix(es), seed {args.seed}")
        print("Now LOOK at it. That is the step no statistic replaces.")
        return 0

    if args.near:
        target = parse_hex(args.near)
        rows.sort(key=lambda a: (distance(target, a["rgb"]), a["id"]))
        rows = rows[: args.n]

    if args.json:
        print(json.dumps({"blocks": rows, "unresolved": unresolved}, indent=2, sort_keys=True))
        return 0

    print(f"{'block':<44} {'hex':<9} {'rgb':<16} cov  cube  note")
    for a in rows:
        note = "BIOME-TINTED (this number is the untinted texture)" if a["tinted"] else ""
        print(
            f"{a['id']:<44} {hexcolor(a['rgb']):<9} "
            f"{str(tuple(a['rgb'])):<16} {a['coverage']:>3}  "
            f"{'yes' if a['full_cube'] else ' no':<4}  {note}"
        )
    print(f"\n{len(rows)} row(s); {unresolved} block(s) had no resolvable model or texture.")
    print(
        "A mean colour ranks candidates; it cannot see pattern or scale. SEE the shortlist "
        "before binding it: `delve-render piece <prefab.nbt> -o <dir>`."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
