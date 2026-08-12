#!/usr/bin/env python3
"""Contact sheet: target (rows) x condition (columns), A/B pairs side by side."""
import os
from PIL import Image, ImageDraw

ROOT = os.path.dirname(os.path.abspath(__file__))
ROWS = [
    ("Temple of Heaven (Hall of Prayer)", "A1-tiantan-baseline", "B1-tiantan-methodology"),
    ("Ancient Greek Doric temple", "A2-greek-baseline", "B2-greek-methodology"),
    ("Colossal ruined stone bridge", "A3-bridge-baseline", "B3-bridge-methodology"),
]
CELL, PAD, HDR, LBL = 512, 12, 44, 28
BG, FG = (24, 24, 28), (232, 232, 238)


def sheet(shot, title, outname):
    W = PAD + 2 * (CELL + PAD)
    H = HDR + len(ROWS) * (LBL + CELL + PAD) + PAD
    im = Image.new("RGB", (W, H), BG)
    d = ImageDraw.Draw(im)
    d.text((PAD, 14), f"{title}   —   left: A (baseline)   right: B (methodology-primed)"
                      f"   —   seed 121111, grid 256, MC 1.21.11 textures", fill=FG)
    y = HDR
    for label, a, b in ROWS:
        d.text((PAD, y + 6), label, fill=FG)
        y += LBL
        for i, name in enumerate((a, b)):
            p = os.path.join(ROOT, "renders", name, f"{name}-{shot}.png")
            tile = Image.open(p).convert("RGB").resize((CELL, CELL), Image.LANCZOS)
            x = PAD + i * (CELL + PAD)
            im.paste(tile, (x, y))
            d.rectangle([x, y, x + CELL - 1, y + CELL - 1], outline=(70, 70, 80))
            d.text((x + 6, y + CELL - 16), name, fill=(200, 200, 210))
        y += CELL + PAD
    out = os.path.join(ROOT, "contact-sheets", outname)
    os.makedirs(os.path.dirname(out), exist_ok=True)
    im.save(out)
    print(out)


if __name__ == "__main__":
    os.makedirs(os.path.join(ROOT, "contact-sheets"), exist_ok=True)
    sheet("hero", "3/4 PERSPECTIVE (yaw 115, pitch 14)", "sheet-1-perspective.png")
    sheet("front", "FRONT ELEVATION (yaw 0, pitch 0)", "sheet-2-front-elevation.png")
    sheet("ext-ne", "CORNER ISOMETRIC (yaw 45, pitch 30)", "sheet-3-isometric.png")
