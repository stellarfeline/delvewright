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
pixels it always did.

Composition is on the boxes of the model the sheet is for, read from the
model-part table (:mod:`delve_skin.models`): ``wide`` or ``slim`` for a
mannequin (``model`` is mandatory -- an omitted model renders slim and distorts a
wide texture), or a mob's own model through ``entity``.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Dict, List

import numpy as np
from PIL import Image

from delve_skin.palette import (
    RGBA,
    deepen,
    jitter,
    parse_hex,
    rng_for,
    seed_from_id,
    shade,
)
from delve_skin import models
from delve_skin.png import encode_rgba
from delve_skin.wardrobe import SHOULDER_HAIR, Span, Wardrobe

FACE_IDS = ("front", "back", "left", "right", "up", "down")

#: The four faces a garment band wraps around. ``up``/``down`` are the caps and
#: are painted on their own, because a hem is a band and a cap is not.
SIDE_FACES = ("front", "back", "left", "right")

# --- where a face sits on the 8x8 front of a head ---------------------------
#
# Measured, not chosen. The nine default player skins the pinned client ships
# (1.21.11) agree: the eye row is the third row up from the chin in 9 of 9, and
# a two-pixel mouth sits at x=3,4 on the first row up in 9 of 9. The bottom row
# is darker at its outer columns than at its centre in 7 of the 9 -- and the
# two that are not are the two whose bottom row is beard, so it is 7 of 7 among
# the clean-shaven. The instrument, the second source and the whole derivation
# are in `docs/reference/face-craft.md`.
#
# A face is therefore the LOWER FIVE rows, and what is above the brow is
# forehead for the hair to come down onto. A face painted on the upper rows
# instead leaves four rows of unmodelled fill under it -- half a head of flat
# skin, which reads as an enormous jaw, and which a beard hides by occupying
# exactly those rows.
FACE_HAIRLINE = 6
FACE_BROW = 4
FACE_EYES = 3
#: The upper lip: what a moustache is, and the top row of a beard.
FACE_LIP = 2
FACE_MOUTH = 1
FACE_CHIN = 0

# Palette keys a cast entry may provide. Missing keys fall back to a derived
# shade so a sparse palette still yields a complete, coherent skin. A key that
# is NOT here is refused at the entry: a misspelled colour would otherwise be
# dropped in silence and the character dressed in a default nobody asked for.
PALETTE_KEYS = (
    "skin", "skin_shadow", "hair", "hair_shadow", "hair_grey",
    "beard", "beard_grey", "tunic", "tunic_shadow", "belt",
    "legwear", "legwear_shadow", "sandal", "eye", "hood", "hood_shadow", "coat", "coat_shadow",
)

#: Fields a cast-sheet entry may carry, for the same reason: a misspelled
#: ``wardrobe`` would compose the default costume and say nothing.
ENTRY_KEYS = (
    "texture_id", "entity", "model", "palette", "wardrobe", "style_brief", "role",
    "hidden_layers", "features", "seed",
)

#: The body a cast entry dresses when it names none: a player-model mannequin.
MANNEQUIN = "mannequin"


def entities() -> tuple[str, ...]:
    """What a cast entry's ``entity`` may name: a mannequin, or a mob model the
    wardrobe fits (every part the player's size, :func:`models.dressable`)."""
    return (MANNEQUIN,) + tuple(
        k for k in models.dressable() if k not in ("player", "player_slim")
    )


@dataclass(frozen=True)
class CastEntry:
    """One row of a skin cast sheet (spec-0009 workflow step 1)."""

    texture_id: str
    model: str  # "wide" | "slim" -- MANDATORY for a mannequin (spec-0009); "" for a mob
    palette: Dict[str, str]
    wardrobe: Wardrobe = field(default_factory=Wardrobe)
    style_brief: str = ""
    role: str = ""
    hidden_layers: List[str] = field(default_factory=list)
    features: Dict[str, object] = field(default_factory=dict)
    seed: int | None = None
    entity: str = MANNEQUIN

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
        entity = d.get("entity", MANNEQUIN)
        if entity not in entities():
            known = ", ".join(repr(e) for e in entities())
            raise ValueError(
                f"cast entry {texture_id!r}: entity {entity!r} is not a model the "
                f"model-part table carries ({known}). The parched and the zombie "
                "villager are not in it: their vanilla sheets carry paint their own "
                "boxes do not reach, so the table could not be measured for them"
            )
        if entity != MANNEQUIN:
            if d.get("model") not in (None, ""):
                raise ValueError(
                    f"cast entry {texture_id!r}: entity {entity!r} has one model, so "
                    "'model' (wide|slim) says nothing -- remove it"
                )
        else:
            # spec-0009: model is mandatory; omission silently renders slim.
            if "model" not in d or d["model"] in (None, ""):
                raise ValueError(
                    f"cast entry {texture_id!r} missing required field "
                    "'model' (wide|slim) -- an omitted model renders slim and "
                    "distorts a wide skin (spec-0009)"
                )
            if d["model"] not in ("wide", "slim"):
                raise ValueError(f"model must be 'wide' or 'slim', got {d['model']!r}")
        hidden = list(d.get("hidden_layers", []))
        if hidden and entity != MANNEQUIN:
            raise ValueError(
                f"cast entry {texture_id!r}: 'hidden_layers' is a mannequin's, and "
                f"entity {entity!r} is not one"
            )
        layers = models.mannequin_layers()
        unknown_layers = [h for h in hidden if h not in layers]
        if unknown_layers:
            raise ValueError(
                f"cast entry {texture_id!r}: unknown hidden layer(s) "
                f"{', '.join(repr(h) for h in unknown_layers)}; a mannequin's layers "
                f"are {', '.join(layers)}"
            )
        if len(set(hidden)) != len(hidden):
            raise ValueError(
                f"cast entry {texture_id!r}: 'hidden_layers' names a layer twice"
            )
        palette = dict(d.get("palette", {}))
        unknown_colours = sorted(set(palette) - set(PALETTE_KEYS))
        if unknown_colours:
            raise ValueError(
                f"cast entry {texture_id!r}: unknown palette key(s) "
                f"{', '.join(repr(k) for k in unknown_colours)}; known keys: "
                f"{', '.join(PALETTE_KEYS)}"
            )
        wardrobe = Wardrobe.from_dict(d.get("wardrobe"), texture_id, d.get("features"))
        layout = models.layout(entity if entity != MANNEQUIN else
                               ("player" if d.get("model") == "wide" else "player_slim"))
        part_word = {"head": "head", "torso": "torso"}
        missing = [
            f"{feat} is drawn on the {part_word[pid]}'s overlay shell"
            for feat, pid in wardrobe.needs_shells()
            if layout.shell[pid] is None
        ]
        if missing:
            raise ValueError(
                f"cast entry {texture_id!r}: {'; '.join(missing)}, and the "
                f"{layout.key!r} model builds none"
            )
        return CastEntry(
            texture_id=texture_id,
            entity=entity,
            model=d.get("model") or "",
            palette=palette,
            wardrobe=wardrobe,
            style_brief=d.get("style_brief", ""),
            role=d.get("role", ""),
            hidden_layers=hidden,
            features=dict(d.get("features", {})),
            seed=d.get("seed"),
        )

    def resolved_seed(self) -> int:
        return self.seed if self.seed is not None else seed_from_id(self.texture_id)

    def model_key(self) -> str:
        """The model-part table key this entry's sheet is drawn to."""
        if self.entity != MANNEQUIN:
            return self.entity
        return "player" if self.model == "wide" else "player_slim"


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
    # A hood is cut from the torso's cloth unless it names its own, and its rim
    # is that cloth in shadow -- the role `tunic_shadow` has for the torso.
    p.setdefault("hood", p["tunic"])
    p.setdefault("hood_shadow", shade(p["hood"], -40))
    # A coat is the garment's cloth a step darker unless it names its own.
    p.setdefault("coat", shade(p["tunic"], -25))
    p.setdefault("coat_shadow", shade(p["coat"], -30))
    return p


class _Canvas:
    """Part/face addressing over one layer of a model's sheet.

    ``parts`` maps a composer part id to the skinpy ``BodyPart`` that layer
    paints (:class:`delve_skin.models.Canvas`): the base boxes, or the shell
    boxes over them. A part the model does not paint addresses a discarded
    buffer, so every call is made, and consumes the seeded stream, the same on
    every model.
    """

    def __init__(self, parts) -> None:
        self.parts = parts

    def face(self, part: str, face: str):
        return self.parts[part].get_face_for_id(face)

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
    # The face is measured on the player's 8x8 front. On a head of another
    # size its rows stay counted from the chin and the hair's from the crown
    # (`top`), and its columns are centred (`ox`); on an 8x8 head both are 0.
    W, H = c.face("head", "front").shape
    top, ox, R = H - 8, (W - 8) // 2, W - 1
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
        hy0, hy1 = hair_span[0] + top, hair_span[1] + top
        c.fill("head", "up", hair)
        c.fill("head", "back", hair)
        c.rows("head", "left", hy0, hy1, hair)
        c.rows("head", "right", hy0, hy1, hair)
        c.rows("head", "front", H - 1, H - 1, hair)  # fringe row across the brow-top
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
            c.columns("head", "front", 0, 0, hy0, H - 1, hair)
            c.columns("head", "front", R, R, hy0, H - 1, hair)
        if w.hair_reaches_the_shoulders():
            sy0, sy1 = SHOULDER_HAIR
            c.rows("torso", "back", sy0, sy1, hair)
            c.rows("torso", "back", sy0, sy0, hair_sh)
        if w.greys_hair():
            c.streak("head", "up", None, hair_grey, rng)
            c.streak("head", "back", None, hair_grey, rng)
            c.streak("head", "left", (hy0, hy1), hair_grey, rng)
            c.streak("head", "right", (hy0, hy1), hair_grey, rng)
            c.streak("head", "front", (H - 1, H - 1), hair_grey, rng)
            if framed:
                c.streak("head", "front", (hy0, H - 2), hair_grey, rng, xs=(0, 0))
                c.streak("head", "front", (hy0, H - 2), hair_grey, rng, xs=(R, R))
            if w.hair_reaches_the_shoulders():
                c.streak("torso", "back", SHOULDER_HAIR, hair_grey, rng)

    # --- the face -----------------------------------------------------------
    #
    # A fringe has a shadow under it; a bare skull does not, and a band across
    # a bald head is a headband.
    if hair_span is not None:
        bx0, bx1 = (1, R - 1) if framed else (0, R)
        for bx in range(bx0, bx1 + 1):
            c.px("head", "front", bx, FACE_HAIRLINE + top, sh)

    # Eyebrows, over each eye in the two columns that eye occupies.
    for bx in (1, 2, 5, 6):
        c.px("head", "front", bx + ox, FACE_BROW, sh)

    # Eyes: sockets + a faint highlight pixel to the outer side.
    eye = p["eye"]
    for ex in (2, 5):
        c.px("head", "front", ex + ox, FACE_EYES, eye)
    c.px("head", "front", 1 + ox, FACE_EYES, shade(skin, 18))
    c.px("head", "front", 6 + ox, FACE_EYES, shade(skin, 18))

    # The lower face, which is the half of a head a beard used to be hiding.
    # There is no nose: at this size a nose is two dark pixels immediately over
    # the mouth, and all five clean-shaven default skins the pinned client
    # ships leave that row bare -- the pair the bearded ones carry there is the
    # moustache. The lip row is therefore where facial hair goes and nothing
    # else. Painted before the facial hair, so a beard covers a mouth the way a
    # beard covers a mouth, and consuming no rng, so a beard's own texture does
    # not move for its being here.
    dark = deepen(skin, sh)
    for mx in (3, 4):
        c.px("head", "front", mx + ox, FACE_MOUTH, dark)
    # The jaw narrows toward the chin: the two outermost columns of face the
    # chin row still has step down, the outer one further than the inner. Asked
    # of where the hair actually is rather than of its name, so a length that
    # frames the face the whole way down keeps its frame and tapers inside it.
    chin_bare = hair_span is None or hair_span[0] + top > FACE_CHIN
    jx0, jx1 = (0, R) if chin_bare or not framed else (1, R - 1)
    for jx in (jx0, jx1):
        c.px("head", "front", jx, FACE_CHIN, dark)
    for jx in (jx0 + 1, jx1 - 1):
        c.px("head", "front", jx, FACE_CHIN, sh)
    # The same taper carried round the sides. A jaw that narrows only on the
    # face is a mask, and the sides are most of what a player walking past sees.
    if chin_bare:
        c.rows("head", "left", FACE_CHIN, FACE_CHIN, sh)
        c.rows("head", "right", FACE_CHIN, FACE_CHIN, sh)

    greys_beard = w.greys_beard()

    def beardcol(x: int, y: int) -> RGBA:
        if greys_beard and (rng.integers(0, 5) == 0 or y == 0):
            return grey
        return beard

    # Facial hair is a declared feature, not a region that is always there. A
    # full beard is the chin and the jaw (the mouth and chin rows), a centred
    # moustache on the lip row above them, the chin underside and the lower
    # front of the side faces; a moustache is that one row and nothing else.
    # Clean-shaven paints nothing here, and what shows through is the face the
    # lower-face block above has already modelled.
    if w.facial_hair == "beard":
        for x in range(1, R):
            for y in (FACE_CHIN, FACE_MOUTH):
                c.px("head", "front", x, y, jitter(rng, beardcol(x, y), 8))
    if w.facial_hair in ("beard", "moustache"):
        for x in range(2 + ox, 6 + ox):
            c.px("head", "front", x, FACE_LIP, jitter(rng, beard, 8))
    if w.facial_hair == "beard":
        c.fill("head", "down", beard)  # chin underside
        c.rows("head", "left", FACE_CHIN, FACE_LIP, beard)
        c.rows("head", "right", FACE_CHIN, FACE_LIP, beard)
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


def _build_shell(c: _Canvas, p: Dict[str, RGBA], w: Wardrobe, rng: np.random.Generator,
                 sheet: "models.Canvas") -> None:
    """The overlay shell (spec-0097 §6.2): beard, hair, hood, collar and coat,
    half a pixel off the head and a quarter off the torso and limbs.

    Painted after the whole base, so a sheet's base pixels are the ones it
    composed before the shell existed, and the stream the base consumed is the
    stream it always consumed. A shell the model does not build is addressed
    into a discarded buffer; a feature that exists ONLY on a shell the model
    lacks is refused where the cast entry is read.

    Rows are counted the way the base counts them: the face from the chin, the
    hair from the crown (`top`), the torso garment from the shoulder (`t`), so a
    taller head or a robe that hangs past the hips keeps every feature where it
    belongs; on the player's boxes every offset is 0.
    """
    head_shell = sheet.has_shell("head")
    torso_shell = sheet.has_shell("torso")
    W, H = c.face("head", "front").shape
    top, ox, R = H - 8, (W - 8) // 2, W - 1
    TH = c.face("torso", "front").shape[1]
    t = TH - 12
    hair, hair_sh, hair_grey = p["hair"], p["hair_shadow"], p["hair_grey"]
    beard, grey = p["beard"], p["beard_grey"]
    hair_span = w.hair_span()
    framed = w.hair_has_a_cut_line()

    if w.hooded():
        hood, rim = p["hood"], p["hood_shadow"]
        for face in ("up", "back", "left", "right"):
            c.fill("head", face, hood)
            c.noise("head", face, hood, 6, rng)
        # The front frames the face: the brow row and the outer columns, with
        # the rim in shadow just inside them. The face rows stay open.
        c.rows("head", "front", H - 1, H - 1, hood)
        c.columns("head", "front", 0, 0, 0, H - 1, hood)
        c.columns("head", "front", R, R, 0, H - 1, hood)
        c.columns("head", "front", 1, R - 1, FACE_HAIRLINE + top, FACE_HAIRLINE + top, rim)
        c.columns("head", "front", 1, 1, 0, FACE_HAIRLINE + top - 1, rim)
        c.columns("head", "front", R - 1, R - 1, 0, FACE_HAIRLINE + top - 1, rim)
        if torso_shell:
            # The fall: over the shoulders and down the upper back.
            c.fill("torso", "up", hood)
            c.rows("torso", "back", 9 + t, 11 + t, hood)
            c.rows("torso", "back", 9 + t, 9 + t, rim)
    elif hair_span is not None and head_shell:
        hy0, hy1 = hair_span[0] + top, hair_span[1] + top
        c.fill("head", "up", hair)
        c.noise("head", "up", hair, 8, rng)
        c.rows("head", "back", hy0, H - 1, hair)
        c.noise("head", "back", hair, 8, rng, only_color=hair)
        c.rows("head", "left", hy0, hy1, hair)
        c.rows("head", "right", hy0, hy1, hair)
        # The fringe's lip over the brow.
        c.rows("head", "front", H - 1, H - 1, hair)
        if framed:
            c.rows("head", "left", hy0, hy0, hair_sh)
            c.rows("head", "right", hy0, hy0, hair_sh)
            c.rows("head", "back", hy0, hy0, hair_sh)
            c.columns("head", "front", 0, 0, hy0, H - 1, hair)
            c.columns("head", "front", R, R, hy0, H - 1, hair)
        if w.greys_hair():
            c.streak("head", "up", None, hair_grey, rng)
            c.streak("head", "back", (hy0, H - 1), hair_grey, rng)

    coat = w.overcoat != "none" and torso_shell
    if coat:
        # A coat over the garment: the torso shell all round, open down the
        # front, its hem in shadow; its sleeves over the sleeve's own span.
        cc, csh = p["coat"], p["coat_shadow"]
        c.fill("torso", "up", cc)
        for face in SIDE_FACES:
            c.rows("torso", face, 0, TH - 1, cc)
            c.noise("torso", face, cc, 6, rng, only_color=cc)
        c.columns("torso", "front", 3, 4, 0, TH - 1, csh)
        c.band("torso", (0, 0), csh)
        sleeve = w.sleeve_span() or w.coat_sleeve_default()
        for arm in ("left_arm", "right_arm"):
            if sheet.has_shell(arm):
                c.band(arm, sleeve, cc)
                c.fill(arm, "up", cc)
                c.band(arm, (sleeve[0], sleeve[0]), csh)
        if w.overcoat == "long_coat":
            for leg in ("left_leg", "right_leg"):
                if sheet.has_shell(leg):
                    c.band(leg, (5, 11), cc)
                    c.band(leg, (5, 5), csh)

    if w.hair_reaches_the_shoulders() and torso_shell and not w.hooded():
        sy0, sy1 = SHOULDER_HAIR
        c.rows("torso", "back", sy0 + t, sy1 + t, hair)
        c.rows("torso", "back", sy0 + t, sy0 + t, hair_sh)

    greys_beard = w.greys_beard()
    if w.facial_hair == "beard":
        for x in range(1, R):
            for y in (FACE_CHIN, FACE_MOUTH):
                col = grey if greys_beard and (rng.integers(0, 5) == 0 or y == 0) else beard
                c.px("head", "front", x, y, jitter(rng, col, 8))
    if w.facial_hair in ("beard", "moustache"):
        for x in range(2 + ox, 6 + ox):
            c.px("head", "front", x, FACE_LIP, jitter(rng, beard, 8))
    if w.facial_hair == "beard":
        c.fill("head", "down", beard)
        c.noise("head", "down", beard, 8, rng)
        if not w.hooded():
            c.rows("head", "left", FACE_CHIN, FACE_LIP, beard)
            c.rows("head", "right", FACE_CHIN, FACE_LIP, beard)

    if w.collar_ring() and torso_shell:
        # A ring that stands off the neck: the top two rows of the torso shell
        # all the way round, its lower row in shadow.
        ring = p["coat"] if coat else p["tunic"]
        c.band("torso", (TH - 2, TH - 1), ring)
        c.band("torso", (TH - 2, TH - 2), p["tunic_shadow"])


def _build_extras(sheet: "models.Canvas", p: Dict[str, RGBA]) -> None:
    """The boxes a model builds beyond head, torso and limbs (spec-0097 §6.3).

    Authored, by the part the box belongs to: a nose, a snout, tusks and ears
    are skin with their underside in shadow; a villager's crossed arms are the
    garment's sleeves with the hands bar in skin. A hat's brim (`hat_rim`) and a
    bogged's mushrooms are left clear -- the wardrobe has no hat and no
    mushrooms -- and so is every other box. Flat fills, no stream: a model with
    no extra box composes exactly what it did.
    """
    for box, part in sheet.extras:
        name = box.part
        if name.endswith("/nose") or name == "head" or name.endswith("_ear"):
            colour, under = p["skin"], p["skin_shadow"]
        elif name == "arms":
            colour, under = ((p["skin"], p["skin_shadow"]) if box.w == 8
                             else (p["tunic"], p["tunic_shadow"]))
        else:
            continue
        for face in models.FACES:
            f = part.get_face_for_id(face)
            fw, fh = f.shape
            col = under if face == "down" else colour
            for x in range(fw):
                for y in range(fh):
                    f.set_color(x, y, col)


def _compose(entry: CastEntry) -> models.Canvas:
    p = _resolve_palette(entry.palette)
    w = entry.wardrobe
    rng = rng_for(entry.resolved_seed())
    sheet = models.Canvas(entry.model_key())
    c = _Canvas(sheet.base)
    # Order matters for deterministic rng consumption; keep it stable.
    _build_torso(c, p, w, rng)
    _build_arm(c, "left_arm", p, w, rng)
    _build_arm(c, "right_arm", p, w, rng)
    _build_leg(c, "left_leg", p, w, rng)
    _build_leg(c, "right_leg", p, w, rng)
    _build_head(c, p, w, entry.features, rng)
    _build_extras(sheet, p)
    _build_shell(_Canvas(sheet.shell), p, w, rng, sheet)
    return sheet


def compose_skin(entry: CastEntry) -> Image.Image:
    """Compose an original RGBA sheet for a cast entry (deterministic): 64x64 for
    a mannequin, the model's own texture size for a mob."""
    w, h, rgba = _compose(entry).to_rgba()
    return Image.frombytes("RGBA", (w, h), rgba)


#: A mirrored box reads its source's faces flipped left to right, with its two
#: side faces exchanged (the model's left limb is the right one, reflected).
_MIRROR_FACE = {"left": "right", "right": "left"}


def compose_preview_skin(entry: CastEntry) -> Image.Image:
    """The sheet as a 64x64 player-layout skin a preview can project.

    Each composer part's base faces are copied, and each shell's opaque pixels
    laid over the face beneath -- what the shell covers, seen from outside. A
    part the model builds as a mirror of another (a zombie's left arm and leg)
    is drawn from that part, reflected. A model with no arm boxes of its own (a
    villager, whose arms are one crossed block) shows that block's arm on both
    preview arms, top-aligned. The half-pixel stand-off is not drawn, and no
    other extra box (a nose, a snout, ears, a brim) is projected.
    """
    sheet = _compose(entry)
    L = sheet.layout
    flat = models.Canvas("player")
    crossed = next((part for box, part in sheet.extras if box.part == "arms" and box.w != 8), None)

    def layers(pid: str):
        out = []
        if L.base[pid] is not None:
            out.append((sheet.base[pid], False))
        if L.shell[pid] is not None:
            out.append((sheet.shell[pid], False))
        if not out and pid in L.mirror_of:
            src = L.mirror_of[pid]
            if L.base[src] is not None:
                out.append((sheet.base[src], True))
            if L.shell[src] is not None:
                out.append((sheet.shell[src], True))
        if not out and pid in ("left_arm", "right_arm") and crossed is not None:
            out.append((crossed, False))
        return out

    for pid in models.PART_IDS:
        for face in models.FACES:
            dst = flat.base[pid].get_face_for_id(face)
            fw, fh = dst.shape
            for src_part, mirrored in layers(pid):
                src = src_part.get_face_for_id(_MIRROR_FACE.get(face, face) if mirrored else face)
                sw, sh_ = src.shape
                for x in range(min(fw, sw)):
                    for y in range(min(fh, sh_)):
                        sx = (sw - 1 - x) if mirrored else x
                        # Top-aligned: a shorter source face fills the top rows.
                        sy = y - (fh - sh_) if sh_ < fh else y
                        if not 0 <= sy < sh_:
                            continue
                        col = src.get_color(sx, sy)
                        if int(col[3]) > 0:
                            dst.set_color(x, y, tuple(int(v) for v in col))
    w, h, rgba = flat.to_rgba()
    return Image.frombytes("RGBA", (w, h), rgba)


def compose_png_bytes(entry: CastEntry) -> bytes:
    """Compose and serialise: the same cast entry is the same FILE on any machine.

    The bytes are those :func:`delve_skin.png.encode_rgba` writes for the
    composed pixels -- stored deflate, no compressor -- so they depend on the
    pixels alone and not on which zlib the machine links. That is the
    comparison this tool promises: bytes, by sha256, across machines.
    """
    img = compose_skin(entry)
    if img.mode != "RGBA":
        raise ValueError(f"composed skin is {img.mode}, not RGBA")
    width, height = img.size
    return encode_rgba(width, height, img.tobytes())
