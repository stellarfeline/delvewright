"""Headless multi-angle player-model previews (spec-0009 workflow step 3).

Uses skinpy-extended's isometric renderer -- a pure-Python orthographic
projection with no GPU/WebGL dependency, so previews are produced deterministically
(no driver variance). See README for why this supersedes a headless skinview3d
(WebGL) renderer for the verify loop.

Four 3/4 views cover the front, back and both sides of the model.
"""

from __future__ import annotations

from pathlib import Path
from typing import Dict, List, Tuple

from PIL import Image
from skinpy import Perspective, Skin

# name -> (x, y, z) perspective faces. Together these show front, back, and
# both sides of the figure.
PREVIEW_ANGLES: Dict[str, Tuple[str, str, str]] = {
    "front": ("right", "front", "up"),
    "left": ("left", "front", "up"),
    "right": ("right", "back", "up"),
    "back": ("left", "back", "up"),
}

DEFAULT_SCALE = 12


def render_previews(
    skin_png: Image.Image,
    out_dir: Path,
    stem: str,
    scale: int = DEFAULT_SCALE,
) -> List[Path]:
    """Render the 4-angle preview set for a skin image; returns written paths."""
    if skin_png.mode != "RGBA":
        skin_png = skin_png.convert("RGBA")
    skin = Skin.from_image(skin_png)
    out_dir.mkdir(parents=True, exist_ok=True)
    written: List[Path] = []
    for name, (x, y, z) in PREVIEW_ANGLES.items():
        persp = Perspective.new(x=x, y=y, z=z, scaling_factor=scale)
        img = skin.to_isometric_image(persp, background_color=(0, 0, 0, 0))
        path = out_dir / f"{stem}-{name}.png"
        img.save(path, format="PNG", optimize=False, compress_level=9)
        written.append(path)
    return written
