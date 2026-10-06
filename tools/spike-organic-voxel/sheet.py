"""Contact sheets for the spike: the four views of one tag in a 2x2 grid, and
each reference view beside its counterpart.

usage: sheet.py <renders-dir> <reference-dir> [--suffix -chunky] <tag>...
  writes <renders-dir>/<tag><suffix>-sheet.png and <tag><suffix>-vs-reference.png
"""

import sys

from PIL import Image, ImageDraw

rd, ref, *tags = sys.argv[1:]
suf = ""
if tags and tags[0] == "--suffix":
    suf, tags = tags[1], tags[2:]
VIEWS = ["far", "spine", "ribcage", "skull"]
REF = {"far": "ref-view1-establishing.jpg", "spine": "ref-view2-spine.jpg", "ribcage": "ref-view3-ribcage.jpg"}
W, H = 600, 334


def label(im, text):
    d = ImageDraw.Draw(im)
    d.rectangle((0, 0, 8 * len(text) + 10, 18), fill=(0, 0, 0))
    d.text((5, 3), text, fill=(255, 255, 255))
    return im


for tag in tags:
    ims = [label(Image.open(f"{rd}/{tag}-{v}{suf}.png").convert("RGB").resize((W, H)), f"{tag}{suf} {v}") for v in VIEWS]
    s = Image.new("RGB", (2 * W, 2 * H))
    for i, im in enumerate(ims):
        s.paste(im, ((i % 2) * W, (i // 2) * H))
    s.save(f"{rd}/{tag}{suf}-sheet.png")
    rows = [v for v in VIEWS if v in REF]
    s = Image.new("RGB", (2 * W, len(rows) * H))
    for r, v in enumerate(rows):
        s.paste(label(Image.open(f"{ref}/{REF[v]}").convert("RGB").resize((W, H)), f"reference {v}"), (0, r * H))
        s.paste(label(Image.open(f"{rd}/{tag}-{v}{suf}.png").convert("RGB").resize((W, H)), f"{tag}{suf} {v}"), (W, r * H))
    s.save(f"{rd}/{tag}{suf}-vs-reference.png")
    print(f"{rd}/{tag}{suf}-sheet.png {rd}/{tag}{suf}-vs-reference.png")
