"""What a cast entry is *wearing* — the declared surface the composer dresses.

The composer used to hold one costume as straight-line code: a short sleeve on
rows 7..11, a skirt on rows 10..11, a sandal on rows 0..1 and a beard nobody
could turn off. Every character it could make was a Bronze-Age Greek sailor, and
no palette key reached past that, so a present-day guide came out in a short
tunic with bare arms and bare legs.

This module is where the costume is *declared* instead. Each field is one axis
with named positions; the composer reads a span off the axis and paints it. A
new costume is a new value on an axis, never a new branch in the composer.

Every default is the costume the composer used to hardcode, so a cast sheet that
names no wardrobe at all composes exactly the bytes it composed before.

## What the vanilla player model cannot carry

The skin is 64x64 and the model's geometry is fixed, so some costume ideas have
nowhere to go and are refused rather than approximated:

* **Nothing stands proud of the body by more than the overlay shell.** Each
  part has a second box over it, the overlay, grown by half a pixel a side on
  the head and a quarter on the body and limbs (spec-0097). The composer paints
  a beard, hair, a hood and a high collar onto that shell, so they stand off the
  base by exactly that much. The shell can be left transparent where it is not
  wanted, but it cannot grow: a hat with a brim, a coat that hangs open, a
  cloak, a beard that juts, a bun, a braid or a ponytail have no geometry and
  cannot be drawn. Hair is a lip of paint half a pixel off the skull, not a
  silhouette.
* **A limb is 4 px around.** A lapel, a cuff, a buckle or a seam narrower than
  one pixel does not exist; a belt is 2 px tall on a 12 px torso and that is the
  finest horizontal band there is.
* **A garment cannot cross a body part.** Arms, legs and torso are separate
  boxes, so a sleeve length and a torso hem are independent numbers; there is no
  shoulder seam to align and no skirt that hangs past the hips.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Dict, Optional, Tuple

# An axis maps a name a creator writes to the row span it paints on a 12-px
# limb, bottom-up: y=0 is the foot/hand, y=11 the hip/shoulder. ``None`` is the
# bare position -- the garment is absent, not zero-height.
Span = Optional[Tuple[int, int]]

#: Sleeve length. ``long`` stops at y=2 so rows 0..1 stay as the hand: a sleeve
#: that reached y=0 would paint the fingers.
SLEEVES: Dict[str, Span] = {
    "bare": None,
    "short": (7, 11),
    "long": (2, 11),
}

#: Leg garment length. ``short`` is the exomis/kilt line over the upper thigh;
#: ``full`` is trousers to the ankle.
LEGS: Dict[str, Span] = {
    "bare": None,
    "short": (10, 11),
    "full": (0, 11),
}

#: Footwear height, in pixels up the 12-px leg from the sole.
FOOTWEAR: Dict[str, Span] = {
    "none": None,
    "sandal": (0, 1),
    "shoe": (0, 2),
    "boot": (0, 5),
    "tall_boot": (0, 8),
}

#: Facial hair. ``moustache`` is the lip row; ``beard`` is that row plus the
#: mouth and chin rows, the sides of the jaw and the chin underside. ``none`` is
#: a modelled face and not a blank one -- the composer paints a mouth, a chin
#: and a jaw that narrows toward it for everybody (``compose.FACE_*``).
FACIAL_HAIR = ("none", "moustache", "beard")

#: How far hair comes down the SIDES of the 8-px head, bottom-up: y=0 is the
#: jaw line, y=7 the crown. The crown, the back of the head and the brow fringe
#: come with every length; this axis is the only thing separating a crop from
#: hair to the shoulders. ``long`` also carries onto the shoulders, the one
#: place hair can go that is not the head -- see ``SHOULDER_HAIR``.
HAIR: Dict[str, Span] = {
    "bald": None,
    "crop": (6, 7),
    "short": (5, 7),
    "jaw": (2, 7),
    "long": (0, 7),
}

#: Reaching below this row makes the hair on the side of the head a large flat
#: field, so its lower edge is marked in ``hair_shadow``. A crop or a short back
#: and sides has no edge to read and gets none.
HAIR_CUT_LINE_BELOW = 4

#: Rows of the TORSO back that long hair falls across, bottom-up on a 12-row
#: torso; 11 is the shoulder line and 9 carries the lower edge.
SHOULDER_HAIR: Tuple[int, int] = (9, 11)

#: Whether the torso garment is open at the throat. ``open`` leaves the V of
#: bare skin at the collar that a tunic or an open shirt has; ``closed`` takes
#: it away, which is what a weatherproof jacket needs; ``high`` is ``closed``
#: plus a collar ring on the torso's overlay shell, standing off the neck.
COLLAR = ("open", "closed", "high")

#: A hood. ``up`` covers the head's overlay shell but for the face, and falls
#: onto the top and the upper back of the torso's shell; it replaces the hair's
#: shell, and the hair painted on the skull still shows round the face.
HOOD = ("none", "up")

#: What is going grey. ``features.greying`` is the older spelling of ``beard``;
#: a sheet carrying both is refused -- see ``Wardrobe.from_dict``.
GREYING = ("none", "hair", "beard", "both")

_AXES: Dict[str, Tuple[str, ...]] = {
    "sleeves": tuple(SLEEVES),
    "legs": tuple(LEGS),
    "footwear": tuple(FOOTWEAR),
    "hair": tuple(HAIR),
    "facial_hair": FACIAL_HAIR,
    "collar": COLLAR,
    "hood": HOOD,
    "greying": GREYING,
}


@dataclass(frozen=True)
class Wardrobe:
    """How one cast entry is dressed. Defaults reproduce the former hardcode."""

    sleeves: str = "short"
    legs: str = "short"
    footwear: str = "sandal"
    hair: str = "short"
    facial_hair: str = "beard"
    collar: str = "open"
    hood: str = "none"
    greying: str = "none"

    @staticmethod
    def from_dict(raw: object, texture_id: str = "",
                  features: Dict[str, object] | None = None) -> "Wardrobe":
        """Parse a ``wardrobe`` block. Every mistake is refused where it is written.

        An unknown key or an unknown value is an error rather than a silent
        default: a sheet that means to dress a character and misspells the key
        would otherwise compose the old costume and say nothing.

        ``features.greying`` is the older spelling of ``greying: "beard"`` and
        still means exactly that, so no sheet written before this axis existed
        changes. A sheet carrying BOTH is refused rather than resolved by a
        precedence rule nobody would remember: two ways to say one thing, and
        the one that errors on a mistake is the one to have.
        """
        who = f"cast entry {texture_id!r}: " if texture_id else ""
        legacy_grey = bool((features or {}).get("greying", False))
        if raw is None:
            return Wardrobe(greying="beard" if legacy_grey else "none")
        if not isinstance(raw, dict):
            raise ValueError(f"{who}'wardrobe' must be an object, got {type(raw).__name__}")
        if "greying" in raw and legacy_grey:
            raise ValueError(
                f"{who}'features.greying' and 'wardrobe.greying' both set what is "
                f"going grey. Keep wardrobe.greying ({raw['greying']!r}) and drop "
                "features.greying, which is the older spelling of 'beard'."
            )
        unknown = sorted(set(raw) - set(_AXES))
        if unknown:
            raise ValueError(
                f"{who}unknown wardrobe key(s) {', '.join(repr(k) for k in unknown)}; "
                f"known keys: {', '.join(sorted(_AXES))}"
            )
        values = {}
        for key, allowed in _AXES.items():
            if key not in raw:
                continue
            value = raw[key]
            if value not in allowed:
                raise ValueError(
                    f"{who}wardrobe.{key} must be one of "
                    f"{', '.join(repr(a) for a in allowed)}, got {value!r}"
                )
            values[key] = value
        return Wardrobe(**values)

    def sleeve_span(self) -> Span:
        return SLEEVES[self.sleeves]

    def leg_span(self) -> Span:
        return LEGS[self.legs]

    def footwear_span(self) -> Span:
        return FOOTWEAR[self.footwear]

    def hair_span(self) -> Span:
        return HAIR[self.hair]

    def hair_has_a_cut_line(self) -> bool:
        """Does the hair reach far enough down the side of the head to show one?"""
        span = self.hair_span()
        return span is not None and span[0] <= HAIR_CUT_LINE_BELOW

    def hair_reaches_the_shoulders(self) -> bool:
        return self.hair == "long"

    def hooded(self) -> bool:
        return self.hood == "up"

    def collar_ring(self) -> bool:
        return self.collar == "high"

    def needs_body_shell(self) -> list[str]:
        """The declared features that exist only on the torso's overlay shell."""
        return ["collar: high"] if self.collar_ring() else []

    def greys_hair(self) -> bool:
        return self.greying in ("hair", "both")

    def greys_beard(self) -> bool:
        return self.greying in ("beard", "both")

    def covers_leg_row(self, y: int) -> bool:
        """Is the leg garment (not the footwear) over row ``y``?

        The knee shadow asks this: over bare skin it is a skin shadow, over
        trousers it is a cloth shadow. Reading it off the span is what keeps the
        shading a property of the garment rather than a second field to set.
        """
        span = self.leg_span()
        return span is not None and span[0] <= y <= span[1]

    def as_tags(self) -> Dict[str, str]:
        """The wardrobe as catalog-card tags."""
        return {
            "sleeves": self.sleeves,
            "legs": self.legs,
            "footwear": self.footwear,
            "hair": self.hair,
            "facial_hair": self.facial_hair,
            "collar": self.collar,
            "hood": self.hood,
            "greying": self.greying,
        }
