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
  boxes. A coat that hangs open, a hat with a brim, a hood, a cloak, a beard
  that juts and hair with any volume at all need the *overlay* layer (or model
  geometry) and cannot be painted here. `delve_skin` authors the base layer
  only. **Hair in particular is paint on the skull**: a bun, a braid, a
  ponytail, a fringe that falls and a silhouette that is not a cube do not
  exist, and long hair is hair-coloured paint down the sides of the head and
  across the top of the torso back -- which reads at playing distance and is not
  the same thing as hair.
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
#: it away, which is what a weatherproof jacket or a high collar needs.
COLLAR = ("open", "closed")

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
            "greying": self.greying,
        }
