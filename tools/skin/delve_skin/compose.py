"""Deterministic, original 64x64 skin composition (spec-0009).

Pixels are addressed through ``skinpy-extended`` at the part/face level -- no raw
UV-atlas arithmetic (the no-hack layering principle: use the intended primitive).

Face coordinate convention (verified against skinpy-extended 1.0.1):
  * ``y = 0`` is the *bottom* of a face; ``y`` increases upward.
  * ``x = 0`` is the observer's *left*; ``x`` increases rightward.
So a torso front face (8 wide x 12 tall) has the waist at ``y=0`` and the
shoulders at ``y=11``; a head front face (8x8) has the chin at ``y=0``.

What a character *wears* is declared, not hardcoded: a cast entry carries a
``wardrobe`` block (see :mod:`delve_skin.wardrobe`) and every garment here paints
a span read off one of its axes. The defaults are the one costume this composer
used to be able to make, so a sheet that declares no wardrobe composes the same
bytes it always did.

Composition targets the classic **wide** player model. ``slim`` is recorded and
emitted (it is mandatory metadata -- an omitted model renders slim and distorts a
wide texture) but slim *geometry* is not yet supported by the wide-only
skinpy-extended layout; a slim entry raises rather than emit a distorted texture.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Dict, List

import numpy as np
from PIL import Image
from skinpy import Skin

from delve_skin.palette import RGBA, jitter, parse_hex, rng_for, seed_from_id, shade
from delve_skin.wardrobe import SHOULDER_HAIR, Span, Wardrobe

FACE_IDS = ("front", "back", "left", "right", "up", "down")

#: The four faces a garment band wraps around. ``up``/``down`` are the caps and
#: are painted on their own, because a hem is a band and a cap is not.
SIDE_FACES = ("front", "back", "left", "right")

# Palette keys a cast entry may provide. Missing keys fall back to a derived
# shade so a sparse palette still yields a complete, coherent skin. A key that
# is NOT here is refused at the entry: a misspelled colour would otherwise be
# dropped in silence and the character dressed in a default nobody asked for.
PALETTE_KEYS = (
    "skin", "skin_shadow", "hair", "hair_shadow", "hair_grey",
    "beard", "beard_grey", "tunic", "tunic_shadow", "belt",
    "legwear", "legwear_shadow", "sandal", "eye",
)

#: Fields a cast-sheet entry may carry, for the same reason: a misspelled
#: ``wardrobe`` would compose the default costume and say nothing.
ENTRY_KEYS = (
    "texture_id", "model", "palette", "wardrobe", "style_brief", "role",
    "hidden_layers", "features", "seed",
)


@dataclass(frozen=True)
class CastEntry:
    """One row of a skin cast sheet (spec-0009 workflow step 1)."""

    texture_id: str
    model: str  # "wide" | "slim" -- MANDATORY (spec-0009)
    palette: Dict[str, str]
    wardrobe: Wardrobe = field(default_factory=Wardrobe)
    style_brief: str = ""
    role: str = ""
    hidden_layers: List[str] = field(default_factory=list)
    features: Dict[str, object] = field(default_factory=dict)
    seed: int | None = None

    @staticmethod
    def from_dict(d: dict) -> "CastEntry":
        if "texture_id" not in d:
            raise ValueError("cast entry missing required field 'texture_id'")
        texture_id = d["texture_id"]
        unknown = sorted(set(d) - set(ENTRY_KEYS))
        if unknown:
            raise ValueError(
                f"cast entry {texture_id!r}: unknown field(s) "
                f"{', '.join(repr(k) for k in unknown)}; known fields: "
                f"{', '.join(ENTRY_KEYS)}"
            )
        # spec-0009: model is mandatory; omission silently renders slim.
        if "model" not in d or d["model"] in (None, ""):
            raise ValueError(
                f"cast entry {texture_id!r} missing required field "
                "'model' (wide|slim) -- an omitted model renders slim and "
                "distorts a wide skin (spec-0009)"
            )
        if d["model"] not in ("wide", "slim"):
            raise ValueError(f"model must be 'wide' or 'slim', got {d['model']!r}")
        palette = dict(d.get("palette", {}))
        unknown_colours = sorted(set(palette) - set(PALETTE_KEYS))
        if unknown_colours:
            raise ValueError(
                f"cast entry {texture_id!r}: unknown palette key(s) "
                f"{', '.join(repr(k) for k in unknown_colours)}; known keys: "
                f"{', '.join(PALETTE_KEYS)}"
            )
        return CastEntry(
            texture_id=texture_id,
            model=d["model"],
            palette=palette,
            wardrobe=Wardrobe.from_dict(
                d.get("wardrobe"), texture_id, d.get("features")
            ),
            style_brief=d.get("style_brief", ""),
            role=d.get("role", ""),
            hidden_layers=list(d.get("hidden_layers", [])),
            features=dict(d.get("features", {})),
            seed=d.get("seed"),
        )

    def resolved_seed(self) -> int:
        return self.seed if self.seed is not None else seed_from_id(self.texture_id)


def _resolve_palette(raw: Dict[str, str]) -> Dict[str, RGBA]:
    """Parse provided colours and derive sensible defaults for missing keys."""
    p: Dict[str, RGBA] = {}
    for k, v in raw.items():
        p[k] = parse_hex(v)
    p.setdefault("skin", (179, 118, 63, 255))
    p.setdefault("skin_shadow", shade(p["skin"], -34))
    p.setdefault("hair", (58, 47, 42, 255))
    # The cut line down the side of a long head of hair, and the grey coming
    # into it -- the two roles ``tunic_shadow`` and ``beard_grey`` already have.
    p.setdefault("hair_shadow", shade(p["hair"], -30))
    p.setdefault("hair_grey", shade(p["hair"], 70))
    p.setdefault("beard", p["hair"])
    p.setdefault("beard_grey", shade(p["beard"], 70))
    p.setdefault("tunic", (150, 90, 60, 255))
    p.setdefault("tunic_shadow", shade(p["tunic"], -40))
    p.setdefault("belt", shade(p["tunic"], -70))
    # A leg garment that names no colour of its own is cut from the same cloth
    # as the torso -- which is what an exomis skirt is, and what every sheet
    # written before the leg had a colour of its own meant.
    p.setdefault("legwear", p["tunic"])
    p.setdefault("legwear_shadow", shade(p["legwear"], -40))
    p.setdefault("sandal", (74, 55, 40, 255))
    p.setdefault("eye", (40, 34, 30, 255))
    return p


class _Canvas:
    """Thin part/face addressing helper over a skinpy Skin."""

    def __init__(self) -> None:
        self.skin = Skin.new()

    def face(self, part: str, face: str):
        return self.skin.get_body_part_for_id(part).get_face_for_id(face)

    def fill(self, part: str, face: str, color: RGBA) -> None:
        f = self.face(part, face)
        w, h = f.shape
        for x in range(w):
            for y in range(h):
                f.set_color(x, y, color)

    def fill_part(self, part: str, color: RGBA) -> None:
        for fid in FACE_IDS:
            self.fill(part, fid, color)

    def rows(self, part: str, face: str, y0: int, y1: int, color: RGBA) -> None:
        """Fill rows [y0, y1] across the full width of a face."""
        f = self.face(part, face)
        w, h = f.shape
        for x in range(w):
            for y in range(max(0, y0), min(h - 1, y1) + 1):
                f.set_color(x, y, color)

    def px(self, part: str, face: str, x: int, y: int, color: RGBA) -> None:
        f = self.face(part, face)
        w, h = f.shape
        if 0 <= x < w and 0 <= y < h:
            f.set_color(x, y, color)

    def noise(self, part: str, face: str, base: RGBA, amount: int,
              rng: np.random.Generator, only_color: RGBA | None = None) -> None:
        """Re-jitter a face's pixels for cloth/skin texture (deterministic)."""
        f = self.face(part, face)
        w, h = f.shape
        for x in range(w):
            for y in range(h):
                if only_color is not None:
                    cur = tuple(int(c) for c in f.get_color(x, y))
                    if cur != tuple(only_color):
                        continue
                f.set_color(x, y, jitter(rng, base, amount))

    def band(self, part: str, span: Span, color: RGBA) -> None:
        """Wrap a garment band around the four side faces of a part."""
        if span is None:
            return
        y0, y1 = span
        for face in SIDE_FACES:
            self.rows(part, face, y0, y1, color)

    def band_noise(self, part: str, span: Span, color: RGBA, amount: int,
                   rng: np.random.Generator) -> None:
        """Paint a garment band and texture it, face by face.

        Band-then-texture per face -- rather than all four bands, then all four
        noise passes -- is the order a seeded stream was consumed in when this
        was straight-line code, and the order of consumption is part of the
        output (ADR-0006).
        """
        if span is None:
            return
        y0, y1 = span
        for face in SIDE_FACES:
            self.rows(part, face, y0, y1, color)
            self.noise(part, face, color, amount, rng, only_color=color)

    def columns(self, part: str, face: str, x0: int, x1: int, y0: int, y1: int,
                color: RGBA) -> None:
        """Fill a rectangle. Hair framing a face is a column, not a band."""
        f = self.face(part, face)
        w, h = f.shape
        for x in range(max(0, x0), min(w - 1, x1) + 1):
            for y in range(max(0, y0), min(h - 1, y1) + 1):
                f.set_color(x, y, color)

    def streak(self, part: str, face: str, span: Span, color: RGBA,
               rng: np.random.Generator, one_in: int = 7,
               xs: tuple[int, int] | None = None, amount: int = 4) -> None:
        """Scatter ``color`` through a region -- going grey, at any length.

        Addressed by REGION rather than by colour, because the hair it streaks
        has already been jittered and no longer equals any palette entry. The
        odds match the beard's, so a head and a beard going grey together look
        like one person.
        """
        f = self.face(part, face)
        w, h = f.shape
        y0, y1 = (0, h - 1) if span is None else span
        x0, x1 = (0, w - 1) if xs is None else xs
        for x in range(max(0, x0), min(w - 1, x1) + 1):
            for y in range(max(0, y0), min(h - 1, y1) + 1):
                if rng.integers(0, one_in) == 0:
                    f.set_color(x, y, jitter(rng, color, amount))


def _build_head(c: _Canvas, p: Dict[str, RGBA], w: Wardrobe, feat: dict,
                rng: np.random.Generator) -> None:
    skin, sh = p["skin"], p["skin_shadow"]
    c.fill_part("head", skin)
    c.noise("head", "front", skin, 6, rng)
    c.noise("head", "left", skin, 6, rng)
    c.noise("head", "right", skin, 6, rng)

    hair, beard, grey = p["hair"], p["beard"], p["beard_grey"]
    hair_sh, hair_grey = p["hair_shadow"], p["hair_grey"]
    hair_span = w.hair_span()
    framed = w.hair_has_a_cut_line()

    # Hair: the crown, the back of the head and a fringe across the brow come
    # with any length; how far it comes down the SIDES is the axis. It is paint
    # on the skull -- there is no volume, and no silhouette but the cube.
    if hair_span is None:
        # A bald head keeps its skin, textured like the rest of the face.
        c.noise("head", "up", skin, 6, rng)
        c.noise("head", "back", skin, 6, rng)
    else:
        hy0, hy1 = hair_span
        c.fill("head", "up", hair)
        c.fill("head", "back", hair)
        c.rows("head", "left", hy0, hy1, hair)
        c.rows("head", "right", hy0, hy1, hair)
        c.rows("head", "front", 7, 7, hair)  # fringe row across the brow-top
        c.noise("head", "up", hair, 8, rng)
        c.noise("head", "back", hair, 8, rng)
        if framed:
            # Hair this long is a flat field on the side of the head; its lower
            # edge is what makes it read as a cut rather than as a helmet. And
            # it comes round the FRONT, down the outer column either side: that
            # frame is most of what separates a face with hair round it from the
            # short-back-and-sides every clean-shaven head used to be.
            c.rows("head", "left", hy0, hy0, hair_sh)
            c.rows("head", "right", hy0, hy0, hair_sh)
            c.columns("head", "front", 0, 0, hy0, 7, hair)
            c.columns("head", "front", 7, 7, hy0, 7, hair)
        if w.hair_reaches_the_shoulders():
            sy0, sy1 = SHOULDER_HAIR
            c.rows("torso", "back", sy0, sy1, hair)
            c.rows("torso", "back", sy0, sy0, hair_sh)
        if w.greys_hair():
            c.streak("head", "up", None, hair_grey, rng)
            c.streak("head", "back", None, hair_grey, rng)
            c.streak("head", "left", (hy0, hy1), hair_grey, rng)
            c.streak("head", "right", (hy0, hy1), hair_grey, rng)
            c.streak("head", "front", (7, 7), hair_grey, rng)
            if framed:
                c.streak("head", "front", (hy0, 6), hair_grey, rng, xs=(0, 0))
                c.streak("head", "front", (hy0, 6), hair_grey, rng, xs=(7, 7))
            if w.hair_reaches_the_shoulders():
                c.streak("torso", "back", SHOULDER_HAIR, hair_grey, rng)

    # Brow shadow just under the fringe, spanning whatever the hair leaves --
    # a face with hair down its outer columns has the shadow between them.
    bx0, bx1 = (1, 6) if framed else (0, 7)
    for bx in range(bx0, bx1 + 1):
        c.px("head", "front", bx, 6, sh)

    # Eyes at y=5: sockets + a faint highlight pixel to the outer side.
    eye = p["eye"]
    for ex in (2, 5):
        c.px("head", "front", ex, 5, eye)
    c.px("head", "front", 1, 5, shade(skin, 18))
    c.px("head", "front", 6, 5, shade(skin, 18))
    # Nose ridge shadow at mid-face.
    c.px("head", "front", 3, 4, sh)
    c.px("head", "front", 4, 4, sh)

    greys_beard = w.greys_beard()

    def beardcol(x: int, y: int) -> RGBA:
        if greys_beard and (rng.integers(0, 5) == 0 or y == 0):
            return grey
        return beard

    # Facial hair is a declared feature, not a region that is always there. A
    # full beard is the chin and jaw (front y0..2), a centred moustache row
    # (y3), the chin underside and the lower front of the side faces; a
    # moustache is that one row and nothing else; clean-shaven paints nothing
    # and the head keeps the skin it was filled with.
    if w.facial_hair == "beard":
        for x in range(1, 7):
            for y in range(0, 3):
                c.px("head", "front", x, y, jitter(rng, beardcol(x, y), 8))
    if w.facial_hair in ("beard", "moustache"):
        for x in range(2, 6):
            c.px("head", "front", x, 3, jitter(rng, beard, 8))
    if w.facial_hair == "beard":
        c.fill("head", "down", beard)  # chin underside
        c.rows("head", "left", 0, 2, beard)
        c.rows("head", "right", 0, 2, beard)
        c.noise("head", "down", beard, 8, rng)


def _build_arm(c: _Canvas, part: str, p: Dict[str, RGBA], w: Wardrobe,
               rng: np.random.Generator) -> None:
    """Arm: skin, with the declared sleeve painted over it down to its hem."""
    skin = p["skin"]
    c.fill_part(part, skin)
    sleeve = w.sleeve_span()
    if sleeve is not None:
        tunic, tsh = p["tunic"], p["tunic_shadow"]
        c.band_noise(part, sleeve, tunic, 7, rng)
        c.fill(part, "up", tunic)  # shoulder cap
        hem = sleeve[0]
        c.band(part, (hem, hem), tsh)  # the sleeve's hem shadow, wherever it ends
    # Hand shading at the very bottom, below any sleeve.
    c.band(part, (0, 0), p["skin_shadow"])
    c.fill(part, "down", p["skin_shadow"])
    c.noise(part, "front", skin, 5, rng, only_color=skin)


def _build_leg(c: _Canvas, part: str, p: Dict[str, RGBA], w: Wardrobe,
               rng: np.random.Generator) -> None:
    """Leg: skin, then the declared leg garment, then the declared footwear."""
    skin = p["skin"]
    c.fill_part(part, skin)
    # A leg garment is cloth in its own colour: an exomis skirt over the upper
    # thigh, or trousers to the ankle.
    garment_span = w.leg_span()
    if garment_span is not None:
        garment = p["legwear"]
        c.band_noise(part, garment_span, garment, 7, rng)
        c.fill(part, "up", garment)  # hip cap
    # Knee shadow, in whatever the knee is wearing. Reading the colour off the
    # garment's own span is what stops a trouser leg getting a bare-skin shadow;
    # it is a property of the garment, not a second field to set.
    knee = p["legwear_shadow"] if w.covers_leg_row(5) else p["skin_shadow"]
    c.band(part, (5, 5), knee)
    # Footwear last, so a boot that reaches past the knee covers that shadow.
    boots = w.footwear_span()
    if boots is not None:
        c.band(part, boots, p["sandal"])
        c.fill(part, "down", p["sandal"])  # sole


def _build_torso(c: _Canvas, p: Dict[str, RGBA], w: Wardrobe,
                 rng: np.random.Generator) -> None:
    tunic, tsh, belt = p["tunic"], p["tunic_shadow"], p["belt"]
    c.fill_part("torso", tunic)
    # form shading: sides a touch darker than the front/back
    c.fill("torso", "left", tsh)
    c.fill("torso", "right", tsh)
    for face in SIDE_FACES:
        c.noise("torso", face, tunic if face in ("front", "back") else tsh, 8,
                rng)
    # belt band low on the waist
    c.band("torso", (1, 2), belt)
    # hem shadow at the very bottom
    c.band("torso", (0, 0), tsh)
    # An OPEN collar is the V of bare skin a tunic or an unbuttoned shirt has
    # at the throat. A CLOSED one paints nothing and the garment reaches the
    # neck, which is what a weatherproof jacket or a habit needs -- and what no
    # palette key could ever have given, the V being painted from ``skin``.
    if w.collar == "open":
        skin = p["skin"]
        c.px("torso", "front", 3, 11, skin)
        c.px("torso", "front", 4, 11, skin)
        c.px("torso", "front", 3, 10, skin)
        c.px("torso", "front", 4, 10, skin)
        c.px("torso", "front", 4, 9, skin)


def compose_skin(entry: CastEntry) -> Image.Image:
    """Compose an original 64x64 RGBA skin for a cast entry (deterministic)."""
    if entry.model == "slim":
        raise NotImplementedError(
            "slim geometry is not supported by the wide-only skinpy-extended "
            "layout yet; author the cast entry as 'wide' or extend the composer. "
            "The model field is still validated and emitted (spec-0009)."
        )
    p = _resolve_palette(entry.palette)
    w = entry.wardrobe
    rng = rng_for(entry.resolved_seed())
    c = _Canvas()
    # Order matters for deterministic rng consumption; keep it stable.
    _build_torso(c, p, w, rng)
    _build_arm(c, "left_arm", p, w, rng)
    _build_arm(c, "right_arm", p, w, rng)
    _build_leg(c, "left_leg", p, w, rng)
    _build_leg(c, "right_leg", p, w, rng)
    _build_head(c, p, w, entry.features, rng)
    return c.skin.to_image()


def compose_png_bytes(entry: CastEntry) -> bytes:
    """Compose and serialise. The IMAGE is portable; the FILE is not.

    ``compose_skin`` is deterministic everywhere -- the same cast entry yields
    the same 64x64 pixels on any machine. Serialisation adds a dependency this
    tool does not control: Pillow hands the scanlines to whatever zlib it is
    linked against, and deflate output differs between zlib builds. Measured
    with the same Pillow 12.3.0 and numpy 2.5.3 on both sides, varying only
    zlib -- macOS (1.2.12) and Linux (1.3.1) compose identical pixels and write
    different files at compress_level 1, 6 and 9 alike. Only compress_level 0
    agreed, at 16516 bytes against 1902: a portable serialisation exists and
    costs 8.7x the file.

    So a skin composed on two machines is one picture in two files. That does
    not move a delve's bytes -- the compiler bakes the PNG a creator COMMITTED
    rather than recomposing it -- but a regeneration cannot be compared across
    machines byte for byte, and pixels are what to compare.
    """
    import io

    img = compose_skin(entry)
    buf = io.BytesIO()
    # Fixed encoder options -> byte-stable output within one zlib build.
    img.save(buf, format="PNG", optimize=False, compress_level=9)
    return buf.getvalue()
