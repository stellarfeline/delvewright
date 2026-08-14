#!/usr/bin/env python3
"""What a block actually LOOKS like, measured from the pinned client jar.

A block's name is not its appearance and repeatedly is not close to it:
`packed_mud` is orange, `dried_kelp_block` is a woven olive-green, and
`lightning_rod` is signal orange. Choosing a palette from memory therefore
produces a piece whose colours nobody chose, and the mistake only surfaces at
render time — a whole authoring round late.

So palette selection is a **query**, not a recollection. This tool answers three
questions against the 1.21.11 client jar:

    tools/block-appearance.py --id minecraft:packed_mud          # what colour IS it
    tools/block-appearance.py --near '#6b6b6b' -n 12             # what is this colour
    tools/block-appearance.py --list --full-cube-only            # the whole shelf

## What is measured

For a block's **default state**: resolve `assets/minecraft/blockstates/<id>.json`
to a model, walk the model's `parent` chain merging `textures`, collect the
textures its faces reference, and take the alpha-weighted mean of their pixels.
Reported as `rgb`, plus `coverage` (mean alpha, 0-255 — a fence is mostly empty
space) and `full_cube` (whether the model's geometry fills the cell).

Biome tint is deliberately NOT applied: grass and leaves are tinted per biome, so
a single number for them would be a fiction. They are reported with
`tinted: true` and the untinted texture mean, and the tool says so.

## What it is not

Not a renderer. A single mean colour cannot express a pattern (`bricks` and
`smooth_stone` land close together and read nothing alike), so this ranks
candidates for a human or an agent to then SEE — via `delve-render piece` or the
prefab viewer. It narrows a 1166-block shelf to a handful; it does not choose.

The jar is EULA-gated and never committed (versions.toml [render]). It is
resolved from --jar, then $DELVEWRIGHT_CLIENT_JAR, then
~/.chunky/resources/minecraft.jar — the same order `delve-render` uses, so one
machine setup serves both.
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


def appearance(jar: Jar, block_id: str) -> dict | None:
    model_ref = default_model(jar, block_id)
    if model_ref is None:
        return None
    textures, elements = resolve_model(jar, model_ref)
    paths = texture_paths(textures, elements)
    if not paths:
        return None

    r = g = b = 0.0
    alpha_sum = 0.0
    count = 0
    for path in paths:
        ns, name = split_id(path)
        png = jar.png_at(f"assets/{ns}/textures/{name}.png")
        if png is None:
            continue
        width, height, pixels = png
        # An animated texture is a vertical strip; only the first frame is the
        # block's resting appearance.
        if height > width and width > 0 and height % width == 0:
            pixels = pixels[: width * width]
        for pr, pg, pb, pa in pixels:
            a = pa / 255.0
            r += pr * a
            g += pg * a
            b += pb * a
            alpha_sum += a
            count += 1
    if count == 0 or alpha_sum == 0:
        return None
    return {
        "id": block_id,
        "rgb": [round(r / alpha_sum), round(g / alpha_sum), round(b / alpha_sum)],
        "coverage": round(255 * alpha_sum / count),
        "full_cube": is_full_cube(elements),
        "textures": paths,
        "tinted": block_id in TINTED_EXACT
        or any(block_id.endswith(s) for s in TINTED_SUFFIXES),
    }


# --------------------------------------------------------------------------


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
        "no client jar — pass --jar <1.21.11 client jar>, set $DELVEWRIGHT_CLIENT_JAR, "
        "or place it at ~/.chunky/resources/minecraft.jar (the same order delve-render uses)"
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
    args = ap.parse_args(argv[1:])

    if not (args.id or args.near or args.list):
        ap.error("give --id, --near or --list")

    registry = json.loads(load_registry())
    jar = Jar(resolve_jar(args.jar))

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
        a = appearance(jar, block_id)
        if a is None:
            unresolved += 1
            continue
        if args.full_cube_only and not a["full_cube"]:
            continue
        if not args.technical and block_id in TECHNICAL and not args.id:
            continue
        rows.append(a)

    if args.near:
        target = parse_hex(args.near)
        rows.sort(key=lambda a: (distance(target, a["rgb"]), a["id"]))
        rows = rows[: args.n]
    else:
        rows.sort(key=lambda a: a["id"])
        if args.list and not args.json:
            rows = rows  # the whole shelf, sorted

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
