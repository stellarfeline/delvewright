#!/usr/bin/env python3
"""A resource pack this project owns, so a review page can be PUBLISHED.

## The problem this exists for

`delvec viewer` draws real block geometry by reading a Minecraft asset source —
`blockstates/<id>.json`, the model `parent` chain, and every `.png` those models
name — and it inlines all of it into the page. That is exactly right for the
artifact it was built for: a reviewer opens the page from `file://` on the
machine that made it, and the source is the creator's own client jar.

It is not publishable. The jar is EULA-gated and this project's standing rule is
that it is "never committed, cached or published" (`docs/ACKNOWLEDGEMENTS.md`,
the Chunky and deepslate rows; ADR-0010; ADR-0021 §40). A page built from it
carries Mojang's own texture bytes, base64'd, and serving that page from GitHub
Pages is distributing them. Measured, not assumed: `delvec viewer` with no
`--textures`, no `$DELVEWRIGHT_CLIENT_JAR` and an empty `$HOME` refuses with
`DW0723` and writes nothing, so there is no third state where the page simply
comes out untextured.

What `viewer` does offer is the seam: `--textures` takes an unpacked resource
DIRECTORY as well as a jar — "the seam a creator uses to point the page at their
own resource pack" (`docs/reference/tools.md` §4). This writes such a directory,
containing nothing but boxes and flat colours this repository authored, so the
page it produces can be served to strangers.

    tools/blockout-pack.py --out <dir>

## What is cited and what is authored, per rule

**Cited — the shape vocabulary.** Every box below is the geometry of the pinned
1.21.11 model of that form, read off the client jar (an instrument, not an
input): `stairs` is `[0,0,0]-[16,8,16]` plus `[8,8,0]-[16,16,16]`, `slab` is the
bottom half, `template_wall_post` is `[4,0,4]-[12,16,12]`, and so on. So are the
variant conventions: a stair's base orientation is `facing=east` with north at
`y=270`, a wall/fence/pane is `multipart` with north at `y=0` and each further
side at 90° more. Reading a fact off the jar redistributes nothing; the numbers
are recorded here so nothing downstream needs the jar to reproduce the pack.

**Cited — which blocks take which shape.** `form` comes from
`crates/delvec/data/block-classification-1.21.11.json`, derived from vanilla's
own block tags (`tools/extract-block-classification.py`, spec-0035 §3.1), and the
property lists come from `crates/dsl/data/blocks-1.21.11.json`. Neither needs a
jar and both are committed with provenance.

**Authored — the residue.** Two things here are this project's own and are named
as such rather than presented as facts about the game:

* `SHAPE_BY_ID`, rules over block ids for shapes vanilla's form tags do
  not distinguish — a torch, a lantern, a chain and a carpet are all `form:
  block`, and drawing a hanging lantern as a full cube puts a solid block where
  the room has a light. Everything not matched by a rule is a full cube.
* `COLOURS`, a legend of material rules over block ids. These are
  Delvewright's colours, not Minecraft's. The measured ones exist —
  `tools/block-appearance.py` computes a per-block mean from the jar's own pixels
  — but whether that derived table may be committed is spec-0035 §7 question 1,
  **an open question for the owner**, and this tool does not answer it on her
  behalf. A block matching no rule gets a low-chroma tint derived from its id,
  which is deliberately too dull to read as a claim about the block's real
  colour; the run states how many blocks landed there.

A page built from this pack is therefore an honest blockout: the building's
massing, its openings, its floors and its walkable shape are the engine's own
emitted bytes, and the surface is a legend. It is not a picture of the game, and
whatever displays it has to say so.

## Binding counts

Every run states the blocks written, how many took each cited form, how many
matched an authored shape rule, how many matched a colour rule, how many fell
back to the derived tint, and how many block-entity texture ids it covered. A
zero on the special-texture line means the table below stopped matching the
emitter's, which is the failure it exists to catch.

## The special-texture table is READ, never restated

Some textures the renderer reaches for are named by its own code and by no model
file — a chest's `entity/chest/normal`, water's `block/water_still`. The
emitter keeps that table at `SPECIAL_TEXTURES` / `SPECIAL_SUFFIXES` in
`crates/delvec/src/compiler/view/viewer/resources.rs`, and a pack that guessed
at a second copy of it would fail the way that table already fails: silently, as
magenta. So this parses the emitter's own source and asserts a non-zero match.

Deterministic: same inputs, byte-identical pack (ADR-0006). No clock, no RNG,
sorted iteration, no ancillary PNG chunks.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import struct
import sys
import zlib
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
REGISTRY = REPO / "crates" / "dsl" / "data" / "blocks-1.21.11.json"
CLASSIFICATION = REPO / "crates" / "delvec" / "data" / "block-classification-1.21.11.json"
RESOURCES_RS = REPO / "crates" / "delvec" / "src" / "compiler" / "view" / "viewer" / "resources.rs"

# --- the cited half: boxes read off the pinned 1.21.11 models ---------------
#
# Each entry is a list of `(from, to)` in sixteenths, exactly as the vanilla
# model of that form states them. `cull` marks the one shape that fills its cell,
# where face culling is correct and worth an order of magnitude in triangles.
SHAPES: dict[str, dict] = {
    # block/<form>.json in 1.21.11         from            to
    "cube": {"boxes": [((0, 0, 0), (16, 16, 16))], "cull": True},
    "slab_bottom": {"boxes": [((0, 0, 0), (16, 8, 16))]},
    "slab_top": {"boxes": [((0, 8, 0), (16, 16, 16))]},
    "stair": {"boxes": [((0, 0, 0), (16, 8, 16)), ((8, 8, 0), (16, 16, 16))]},
    "fence_post": {"boxes": [((6, 0, 6), (10, 16, 10))]},
    "fence_side": {"boxes": [((7, 12, 0), (9, 15, 9)), ((7, 6, 0), (9, 9, 9))]},
    "wall_post": {"boxes": [((4, 0, 4), (12, 16, 12))]},
    "wall_side": {"boxes": [((5, 0, 0), (11, 14, 8))]},
    "wall_side_tall": {"boxes": [((5, 0, 0), (11, 16, 8))]},
    "pane_post": {"boxes": [((7, 0, 7), (9, 16, 9))]},
    "pane_side": {"boxes": [((7, 0, 0), (9, 16, 7))]},
    "door": {"boxes": [((0, 0, 0), (3, 16, 16))]},
    "trapdoor_bottom": {"boxes": [((0, 0, 0), (16, 3, 16))]},
    "trapdoor_top": {"boxes": [((0, 13, 0), (16, 16, 16))]},
    "trapdoor_open": {"boxes": [((0, 0, 13), (16, 16, 16))]},
    "button": {"boxes": [((5, 0, 6), (11, 2, 10))]},
    "plate": {"boxes": [((1, 0, 1), (15, 1, 15))]},
    "sign": {"boxes": [((0, 4, 7), (16, 16, 9))]},
    # authored shapes (SHAPE_BY_ID), all traced from the vanilla model they stand in for
    "torch": {"boxes": [((7, 0, 7), (9, 10, 9))]},
    "lantern": {"boxes": [((5, 0, 5), (11, 7, 11))]},
    "lantern_hanging": {"boxes": [((5, 1, 5), (11, 8, 11))]},
    "candle": {"boxes": [((7, 0, 7), (9, 6, 9))]},
    "chain": {"boxes": [((7, 0, 7), (9, 16, 9))]},
    "rod": {"boxes": [((6, 0, 6), (10, 1, 10)), ((7, 1, 7), (9, 16, 9))]},
    "carpet": {"boxes": [((0, 0, 0), (16, 1, 16))]},
    "ladder": {"boxes": [((0, 0, 15), (16, 16, 16))]},
}

# --- the authored half: which ids take a shape vanilla's form tags do not ---
#
# Ordered; first match wins. Matched on the block id with the `minecraft:` prefix
# stripped. `None` in the second slot means "leave it a full cube" and exists so
# a broad rule can carve out the block it would otherwise capture.
SHAPE_BY_ID: list[tuple[str, str | None]] = [
    (r"^(soul_)?(wall_)?torch$", "torch"),
    (r"^redstone_(wall_)?torch$", "torch"),
    (r"^(soul_)?lantern$", "lantern"),
    (r"^.*_?candle$", "candle"),
    (r"^(iron_|copper_|gold_)?chain$", "chain"),
    (r"^(end_rod|lightning_rod)$", "rod"),
    (r"^(moss_carpet|pale_moss_carpet)$", "carpet"),
    (r"^[a-z_]+_carpet$", "carpet"),
    (r"^(rail|powered_rail|detector_rail|activator_rail)$", "carpet"),
    (r"^(tripwire|tripwire_hook)$", "carpet"),
    (r"^snow$", "carpet"),
    (r"^(ladder|vine)$", "ladder"),
]

# --- the authored half: the colour legend ----------------------------------
#
# Ordered; first match wins, so a specific rule precedes the family it lives in.
# These are Delvewright's colours; see the module docstring on why the measured
# ones are not here.
COLOURS: list[tuple[str, tuple[int, int, int]]] = [
    (r"water", (60, 100, 175)),
    (r"lava|magma", (200, 92, 30)),
    (r"^(fire|soul_fire|campfire|soul_campfire)$", (214, 128, 46)),
    (r"torch|lantern|glowstone|sea_lantern|shroomlight|candle|end_rod|light$", (238, 214, 140)),
    (r"deepslate|blackstone|basalt|obsidian|tuff", (60, 60, 66)),
    (r"cobble|andesite|^stone|stone_brick|smooth_stone|gravel", (128, 128, 128)),
    (r"granite", (150, 103, 86)),
    (r"diorite|calcite|quartz|white_concrete", (206, 204, 200)),
    (r"sandstone|^sand$|smooth_sandstone", (216, 202, 158)),
    (r"red_sand|terracotta|brick", (150, 92, 74)),
    (r"prismarine", (98, 154, 140)),
    (r"copper", (172, 110, 82)),
    (r"iron|anvil|chain", (150, 152, 155)),
    (r"gold", (206, 172, 84)),
    (r"glass|ice", (176, 206, 220)),
    (r"dark_oak|spruce|mangrove", (94, 68, 44)),
    (r"oak|birch|bamboo|crimson|warped|acacia|cherry|jungle|plank|log$|wood$", (154, 122, 78)),
    (r"grass|leaves|moss|azalea|vine|fern", (94, 138, 66)),
    (r"dirt|podzol|mud|rooted", (110, 84, 60)),
    (r"snow|powder_snow", (238, 244, 250)),
    (r"wool|carpet|bed|banner", (188, 182, 176)),
    (r"netherrack|nether_brick|crimson_nylium", (110, 58, 58)),
    (r"end_stone|purpur", (218, 220, 166)),
    (r"soul_sand|soul_soil", (86, 68, 56)),
    (r"^(chest|barrel|trapped_chest|lectern|bookshelf)$", (150, 114, 62)),
]

# How see-through a shape is, by id. Authored, and only where opacity would hide
# the room: an opaque water cube fills a cistern and an opaque pane fills a
# window, and either reads as a wall.
ALPHA: list[tuple[str, int]] = [
    (r"^(water|bubble_column)$", 140),
    (r"^lava$", 220),
    (r"glass|ice$", 110),
]


def die(msg: str) -> None:
    print(f"blockout-pack: {msg}", file=sys.stderr)
    raise SystemExit(2)


def rel(path: Path) -> str:
    """A path as a reader of this repository names it, and never a crash.

    A diagnostic that raises while formatting the diagnostic is how a refusal
    turns into a stack trace nobody can read; `relative_to` does exactly that
    for any path outside the tree, which is every path a test hands it.
    """
    try:
        return str(path.relative_to(REPO))
    except ValueError:
        return str(path)


def png(rgba: tuple[int, int, int, int]) -> bytes:
    """A 16x16 solid RGBA PNG, written by hand so the pack needs no image library.

    Deterministic: fixed filter byte, fixed zlib level, no ancillary chunks, so
    two runs produce identical bytes.
    """
    width = height = 16
    row = bytes([0]) + bytes(rgba) * width
    raw = row * height

    def chunk(kind: bytes, body: bytes) -> bytes:
        return (
            struct.pack(">I", len(body))
            + kind
            + body
            + struct.pack(">I", zlib.crc32(kind + body) & 0xFFFFFFFF)
        )

    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )


def model(shape: str, texture: str) -> dict:
    spec = SHAPES[shape]
    cull = spec.get("cull", False)
    elements = []
    for lo, hi in spec["boxes"]:
        faces = {}
        for face in ("down", "up", "north", "south", "west", "east"):
            entry: dict = {"texture": "#all"}
            if cull:
                entry["cullface"] = face
            faces[face] = entry
        elements.append({"from": list(lo), "to": list(hi), "faces": faces})
    return {"textures": {"all": texture}, "elements": elements}


def colour_for(stem: str) -> tuple[tuple[int, int, int], bool]:
    """The legend's colour for a block id, and whether a named rule matched."""
    for pattern, rgb in COLOURS:
        if re.search(pattern, stem):
            return rgb, True
    # No rule: a low-chroma tint from the id, stated as such by the run's counts.
    # Deliberately dull — it must not read as a claim about the block's colour.
    h = hashlib.sha256(stem.encode()).digest()
    base = 116
    return (base + h[0] % 24, base + h[1] % 24, base + h[2] % 24), False


def alpha_for(stem: str) -> int:
    for pattern, a in ALPHA:
        if re.search(pattern, stem):
            return a
    return 255


def shape_for(stem: str, form: str, props: dict) -> str:
    """The shape family this block draws with. Authored rules first, then form."""
    for pattern, shape in SHAPE_BY_ID:
        if re.match(pattern, stem):
            if shape is None:
                break
            if shape == "lantern" and "hanging" in props:
                return "lantern"  # the hanging variant is chosen per-state below
            return shape
    return {
        "slab": "slab_bottom",
        "stair": "stair",
        "wall": "wall_post",
        "fence": "fence_post",
        "pane": "pane_post",
        "door": "door",
        "trapdoor": "trapdoor_bottom",
        "button": "button",
        "pressure_plate": "plate",
        "sign": "sign",
    }.get(form, "cube")


# Vanilla's own rotation for each side of a `multipart` connection block, read off
# `cobblestone_wall.json` / `oak_fence.json` / `glass_pane.json`: north is the
# unrotated model and each further side is 90 degrees more.
SIDE_Y = {"north": 0, "east": 90, "south": 180, "west": 270}
# A stair's base orientation is `facing=east`; north is `y=270` (oak_stairs.json).
STAIR_Y = {"east": 0, "south": 90, "west": 180, "north": 270}


def apply(model_id: str, y: int = 0, x: int = 0) -> dict:
    out: dict = {"model": model_id}
    if x:
        out["x"] = x
    if y:
        out["y"] = y
    return out


def definition(stem: str, form: str, props: dict, models: set[str]) -> dict:
    """The blockstate definition, and the per-block models it needs, by side effect.

    Variant keys are PARTIAL on purpose — vanilla matches a key when every
    property it lists agrees, so `facing=east,half=bottom` covers a stair
    whatever its `shape` and `waterlogged` are, and the definition stays small.
    """

    def m(shape: str) -> str:
        models.add(shape)
        return f"minecraft:block/dw_{stem}__{shape}"

    if form == "stair" and "facing" in props and "half" in props:
        variants = {}
        for facing in sorted(props["facing"]):
            for half in sorted(props["half"]):
                variants[f"facing={facing},half={half}"] = apply(
                    m("stair"), y=STAIR_Y.get(facing, 0), x=180 if half == "top" else 0
                )
        return {"variants": variants}

    if form == "slab" and "type" in props:
        return {
            "variants": {
                "type=bottom": apply(m("slab_bottom")),
                "type=top": apply(m("slab_top")),
                "type=double": apply(m("cube")),
            }
        }

    if form in ("wall", "fence", "pane") and "north" in props:
        post, side = {
            "wall": ("wall_post", "wall_side"),
            "fence": ("fence_post", "fence_side"),
            "pane": ("pane_post", "pane_side"),
        }[form]
        parts: list[dict] = []
        if form == "wall":
            # A wall's post is conditional; a fence's and a pane's are not.
            parts.append({"when": {"up": "true"}, "apply": apply(m(post))})
        else:
            parts.append({"apply": apply(m(post))})
        for direction, y in SIDE_Y.items():
            if form == "wall":
                parts.append(
                    {"when": {direction: "low"}, "apply": apply(m(side), y=y)}
                )
                parts.append(
                    {"when": {direction: "tall"}, "apply": apply(m("wall_side_tall"), y=y)}
                )
            else:
                parts.append({"when": {direction: "true"}, "apply": apply(m(side), y=y)})
        return {"multipart": parts}

    if form == "door" and "facing" in props:
        return {
            "variants": {
                f"facing={facing}": apply(m("door"), y=SIDE_Y.get(facing, 0))
                for facing in sorted(props["facing"])
            }
        }

    if form == "trapdoor" and "facing" in props and "half" in props:
        variants = {}
        for facing in sorted(props["facing"]):
            for half in sorted(props["half"]):
                variants[f"facing={facing},half={half},open=false"] = apply(
                    m("trapdoor_top" if half == "top" else "trapdoor_bottom")
                )
                variants[f"facing={facing},half={half},open=true"] = apply(
                    m("trapdoor_open"), y=SIDE_Y.get(facing, 0)
                )
        return {"variants": variants}

    if re.match(r"^(soul_)?lantern$", stem) and "hanging" in props:
        return {
            "variants": {
                "hanging=false": apply(m("lantern")),
                "hanging=true": apply(m("lantern_hanging")),
            }
        }

    shape = shape_for(stem, form, props)
    return {"variants": {"": apply(m(shape))}}


def special_texture_ids(block_ids: list[str]) -> tuple[set[str], int, int]:
    """Block-entity texture ids, read from the emitter's own table.

    Returns the ids, and the two match counts that prove the parse bound to
    something. `resources.rs` is the one authority for this table; restating it
    here is exactly the failure it was written to catch.
    """
    src = RESOURCES_RS.read_text()

    def const_block(name: str) -> str:
        start = src.index(f"const {name}")
        end = src.index("\n];", start)
        return src[start:end]

    try:
        textures_src = const_block("SPECIAL_TEXTURES")
        suffixes_src = const_block("SPECIAL_SUFFIXES")
    except ValueError:
        die(
            f"neither SPECIAL_TEXTURES nor SPECIAL_SUFFIXES is where it was in "
            f"{rel(RESOURCES_RS)} — the emitter moved its table and this "
            f"pack would silently stop covering block-entity textures"
        )

    explicit: dict[str, list[str]] = {}
    for entry in re.finditer(r'\(\s*"([a-z_]+)"\s*,\s*&\[(.*?)\]\s*\)', textures_src, re.S):
        block = entry.group(1)
        ids = re.findall(r'"([^"]+)"', entry.group(2))
        explicit.setdefault(block, []).extend(ids)

    suffixes = [
        (m.group(1), m.group(2))
        for m in re.finditer(r'\(\s*"(_[a-z_]+)"\s*,\s*"([^"]+)"\s*,\s*"[^"]*"\s*\)', suffixes_src)
    ]

    if not explicit or not suffixes:
        die(
            f"parsed {len(explicit)} explicit block-entity texture entries and "
            f"{len(suffixes)} suffix rules out of {rel(RESOURCES_RS)} — a zero "
            f"means the table's shape moved and nothing here is bound to it"
        )

    wanted: set[str] = set()
    for block_id in block_ids:
        stem = block_id.split(":", 1)[1]
        for texture in explicit.get(stem, []):
            wanted.add(texture)
        for suffix, prefix in suffixes:
            if stem.endswith(suffix):
                wanted.add(prefix + stem[: -len(suffix)])
        if stem.endswith("_banner"):
            wanted.add("entity/banner_base")
    return wanted, len(explicit), len(suffixes)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--out", required=True, help="the resource directory to write")
    args = ap.parse_args()

    for path in (REGISTRY, CLASSIFICATION, RESOURCES_RS):
        if not path.exists():
            die(f"{rel(path)} is missing")

    registry: dict[str, dict] = json.loads(REGISTRY.read_text())
    classification: dict[str, dict] = json.loads(CLASSIFICATION.read_text())["blocks"]

    root = Path(args.out)
    states_dir = root / "assets/minecraft/blockstates"
    models_dir = root / "assets/minecraft/models/block"
    textures_dir = root / "assets/minecraft/textures/block"
    for directory in (states_dir, models_dir, textures_dir):
        directory.mkdir(parents=True, exist_ok=True)

    forms: dict[str, int] = {}
    authored_shape = 0
    named_colour = 0
    written = 0

    for block_id in sorted(registry):
        stem = block_id.split(":", 1)[1]
        props = registry[block_id]
        form = classification.get(block_id, {}).get("form", "block")
        forms[form] = forms.get(form, 0) + 1
        if any(re.match(pattern, stem) for pattern, _ in SHAPE_BY_ID):
            authored_shape += 1

        needed: set[str] = set()
        states_dir.joinpath(f"{stem}.json").write_text(
            json.dumps(definition(stem, form, props, needed), sort_keys=True, indent=1) + "\n"
        )

        texture_id = f"minecraft:block/{stem}"
        rgb, matched = colour_for(stem)
        named_colour += 1 if matched else 0
        for shape in sorted(needed):
            models_dir.joinpath(f"dw_{stem}__{shape}.json").write_text(
                json.dumps(model(shape, texture_id), sort_keys=True, indent=1) + "\n"
            )
        textures_dir.joinpath(f"{stem}.png").write_bytes(png((*rgb, alpha_for(stem))))
        written += 1

    special, explicit_rules, suffix_rules = special_texture_ids(sorted(registry))
    for texture in sorted(special):
        path = root / "assets/minecraft/textures" / f"{texture}.png"
        path.parent.mkdir(parents=True, exist_ok=True)
        stem = texture.rsplit("/", 1)[-1]
        rgb, _ = colour_for(stem)
        path.write_bytes(png((*rgb, 255)))

    print(f"blockout-pack: wrote {root}")
    print(f"  blocks          {written} of {len(registry)} in the pinned registry")
    print(
        "  forms (cited)   "
        + ", ".join(f"{form} {count}" for form, count in sorted(forms.items()))
    )
    print(
        f"  shapes          {authored_shape} block(s) matched one of "
        f"{len(SHAPE_BY_ID)} authored id rule(s); the rest take their cited form"
    )
    print(
        f"  colours         {named_colour} matched one of {len(COLOURS)} authored "
        f"material rule(s); {written - named_colour} fell back to the derived tint"
    )
    print(
        f"  block-entity    {len(special)} texture id(s) covered, read from "
        f"resources.rs ({explicit_rules} explicit entries, {suffix_rules} suffix rules)"
    )
    if not written or not special:
        die("a pack that covers no block, or no block-entity texture, is vacuous")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
