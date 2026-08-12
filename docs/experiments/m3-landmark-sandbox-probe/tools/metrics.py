#!/usr/bin/env python3
"""Objective A/B metrics over the six builds — machine gates, no human, no model.

Implements, from the dossier's Part 6/7:
  * Silhouette test        — black-mask fill ratio + perimeter complexity
  * Squint test            — low-pass (8x downsample) luminance spread
  * Palette 60/30/10       — block-type histogram over the expanded build
  * Accent budget <10%     — share of the accent-role block
  * Surface articulation   — exposed faces per placed block (matchbox proxy)
"""
import json
import math
import os
import sys
from collections import Counter

import numpy as np
from PIL import Image

ROOT = os.path.dirname(os.path.abspath(__file__))
BG = np.array([26, 26, 31])          # delve-render's fixed dark-slate background

BUILDS = [
    ("A1-tiantan-baseline", "gold_block"),
    ("B1-tiantan-methodology", "gold_block"),
    ("A2-greek-baseline", "gold_block"),
    ("B2-greek-methodology", "gold_block"),
    ("A3-bridge-baseline", "gold_block"),
    ("B3-bridge-methodology", "gold_block"),
]


def silhouette(png):
    im = np.asarray(Image.open(png).convert("RGB")).astype(int)
    mask = (np.abs(im - BG).sum(axis=2) > 24)
    if mask.sum() == 0:
        return None
    ys, xs = np.nonzero(mask)
    bh, bw = ys.ptp() + 1, xs.ptp() + 1
    area = mask.sum()
    # perimeter = mask pixels with at least one 4-neighbour outside the mask
    p = np.zeros_like(mask)
    p[1:-1, 1:-1] = mask[1:-1, 1:-1] & ~(
        mask[:-2, 1:-1] & mask[2:, 1:-1] & mask[1:-1, :-2] & mask[1:-1, 2:])
    per = p.sum()
    # squint test: 8x box downsample of luminance INSIDE the mask
    lum = im.mean(axis=2)
    lum = np.where(mask, lum, np.nan)
    h, w = lum.shape
    lo = lum[: h // 8 * 8, : w // 8 * 8].reshape(h // 8, 8, w // 8, 8)
    with np.errstate(all="ignore"):
        small = np.nanmean(lo, axis=(1, 3))
    vals = small[~np.isnan(small)]
    return {
        "fill_ratio": round(float(area) / (bh * bw), 4),
        "perimeter_complexity": round(float(per) / math.sqrt(area), 3),
        "squint_value_spread": round(float(vals.std()), 2) if vals.size else 0.0,
    }


def palette_stats(expanded, accent):
    build = json.load(open(expanded))
    c = Counter(b["type"] for b in build["blocks"])
    tot = sum(c.values())
    ranked = c.most_common()
    return {
        "distinct_blocks": len(c),
        "top1_share": round(ranked[0][1] / tot, 3),
        "top1": ranked[0][0],
        "top3_share": round(sum(n for _, n in ranked[:3]) / tot, 3),
        "accent_share": round(c.get(accent, 0) / tot, 4),
        "accent_ok": c.get(accent, 0) / tot < 0.10,
        "histogram": [(t, round(n / tot, 3)) for t, n in ranked],
    }


def articulation(expanded):
    """Exposed faces per placed block. A solid box -> ~0; a modelled, layered
    surface -> higher. The machine-checkable 'matchbox' proxy."""
    build = json.load(open(expanded))
    occ = set()
    for b in build["blocks"]:
        occ.add((b["x"], b["y"], b["z"]))
    n = len(occ)
    faces = 0
    for (x, y, z) in occ:
        for d in ((1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)):
            if (x + d[0], y + d[1], z + d[2]) not in occ:
                faces += 1
    return {"blocks": n, "exposed_faces": faces,
            "faces_per_block": round(faces / n, 3)}


if __name__ == "__main__":
    out = {}
    for name, accent in BUILDS:
        rd = os.path.join(ROOT, "renders", name)
        exp = os.path.join(ROOT, "builds", f"{name}.expanded.json")
        rec = {"palette": palette_stats(exp, accent),
               "geometry": articulation(exp), "shots": {}}
        for shot in ("front", "hero", "ext-ne"):
            p = os.path.join(rd, f"{name}-{shot}.png")
            if os.path.exists(p):
                rec["shots"][shot] = silhouette(p)
        out[name] = rec
        print(f"== {name}")
        print(json.dumps(rec, indent=1))
    json.dump(out, open(os.path.join(ROOT, "metrics.json"), "w"), indent=1)
