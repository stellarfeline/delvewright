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

* **Nothing stands proud of the body.** The base layer is a paint job on six
  boxes. A coat that hangs open, a hat with a brim, a hood, a cloak, hair with
  volume and a beard that juts all need the *overlay* layer (or model geometry)
  and cannot be painted here. `delve_skin` authors the base layer only.
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

#: Facial hair. ``moustache`` is the one row under the nose; ``beard`` is that
#: row plus the chin, the jaw and the chin underside.
FACIAL_HAIR = ("none", "moustache", "beard")

_AXES: Dict[str, Tuple[str, ...]] = {
    "sleeves": tuple(SLEEVES),
    "legs": tuple(LEGS),
    "footwear": tuple(FOOTWEAR),
    "facial_hair": FACIAL_HAIR,
}


@dataclass(frozen=True)
class Wardrobe:
    """How one cast entry is dressed. Defaults reproduce the former hardcode."""

    sleeves: str = "short"
    legs: str = "short"
    footwear: str = "sandal"
    facial_hair: str = "beard"

    @staticmethod
    def from_dict(raw: object, texture_id: str = "") -> "Wardrobe":
        """Parse a ``wardrobe`` block. Every mistake is refused where it is written.

        An unknown key or an unknown value is an error rather than a silent
        default: a sheet that means to dress a character and misspells the key
        would otherwise compose the old costume and say nothing.
        """
        who = f"cast entry {texture_id!r}: " if texture_id else ""
        if raw is None:
            return Wardrobe()
        if not isinstance(raw, dict):
            raise ValueError(f"{who}'wardrobe' must be an object, got {type(raw).__name__}")
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
            "facial_hair": self.facial_hair,
        }
