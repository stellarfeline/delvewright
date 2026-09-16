"""Colour helpers for deterministic skin composition.

All randomness flows through a single seeded ``numpy`` generator so that the same
cast entry always yields byte-identical output (ADR-0006). Python's builtin
``hash`` is never used (it is salted per-process); seeds derive from a stable
SHA-256 instead.
"""

from __future__ import annotations

import hashlib
from typing import Tuple

import numpy as np

RGBA = Tuple[int, int, int, int]


def parse_hex(value: str) -> RGBA:
    """Parse ``#rrggbb`` or ``#rrggbbaa`` into an RGBA tuple."""
    s = value.strip().lstrip("#")
    if len(s) == 6:
        s += "ff"
    if len(s) != 8:
        raise ValueError(f"expected #rrggbb or #rrggbbaa, got {value!r}")
    return (int(s[0:2], 16), int(s[2:4], 16), int(s[4:6], 16), int(s[6:8], 16))


def clamp8(v: int) -> int:
    return 0 if v < 0 else 255 if v > 255 else int(v)


def shade(color: RGBA, delta: int) -> RGBA:
    """Lighten (delta>0) or darken (delta<0) an opaque-ish colour."""
    r, g, b, a = color
    return (clamp8(r + delta), clamp8(g + delta), clamp8(b + delta), a)


def seed_from_id(texture_id: str) -> int:
    """Stable 32-bit seed from a texture id (process-independent)."""
    digest = hashlib.sha256(texture_id.encode("utf-8")).digest()
    return int.from_bytes(digest[:4], "big")


def rng_for(seed: int) -> np.random.Generator:
    """A deterministic generator. PCG64 with an explicit integer seed."""
    return np.random.Generator(np.random.PCG64(int(seed) & 0xFFFFFFFF))


def jitter(rng: np.random.Generator, color: RGBA, amount: int) -> RGBA:
    """Nudge brightness by +/-amount for cloth/skin/beard texture."""
    if amount <= 0:
        return color
    d = int(rng.integers(-amount, amount + 1))
    return shade(color, d)
