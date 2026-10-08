"""``python -m delve_skin`` -- compose skins, render previews, emit catalog cards.

Usage:
  python -m delve_skin build   CAST.json --out-dir DIR [--id ID]
  python -m delve_skin preview CAST.json --out-dir DIR [--id ID] [--scale N]
  python -m delve_skin catalog CAST.json --out-dir DIR [--id ID]
  python -m delve_skin all     CAST.json --skins-dir DIR --catalog-dir DIR \
                               --preview-dir DIR [--id ID]
  python -m delve_skin parts   MODEL

A CAST file is ``{"campaign": "...", "skins": [ <entry>, ... ]}`` or a bare list
of entries. ``--help`` on any subcommand prints the whole entry surface: which
fields an entry may carry, which colours a palette may name, and what a wardrobe
may say about how the character is dressed.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import List

from delve_skin.catalog import catalog_card, dumps
from delve_skin import models
from delve_skin.compose import (
    ENTRY_KEYS,
    PALETTE_KEYS,
    CastEntry,
    compose_png_bytes,
    compose_preview_skin,
    entities,
)
from delve_skin.preview import DEFAULT_SCALE, render_previews
from delve_skin.wardrobe import (
    COLLAR,
    FACIAL_HAIR,
    FOOTWEAR,
    GREYING,
    HAIR,
    HOOD,
    LEGS,
    SLEEVES,
    Wardrobe,
)


def _span_help(span) -> str:
    """Describe an axis position by the rows it paints on a 12-px limb."""
    if span is None:
        return "nothing"
    y0, y1 = span
    return f"rows {y0}-{y1} ({y1 - y0 + 1} px)"


def _hair_help(span) -> str:
    """How far down the 8-px side of the head a hair length comes."""
    if span is None:
        return "no hair"
    y0, y1 = span
    return f"side rows {y0}-{y1}"


def _entry_surface_help() -> str:
    """The cast-entry surface, derived from the code that enforces it.

    Enumerated from the same constants ``CastEntry.from_dict`` and ``Wardrobe``
    validate against, so ``--help`` cannot drift from what the tool accepts.
    """
    d = Wardrobe()
    lines = [
        "cast-entry fields:",
        "  " + ", ".join(ENTRY_KEYS),
        "",
        "entity -- the body the sheet is drawn to (default 'mannequin', which needs",
        "'model' wide|slim; a mob has one model and refuses 'model'):",
        "  " + ", ".join(entities()),
        "",
        "hidden_layers -- a mannequin's overlay layers it does not draw:",
        "  " + ", ".join(models.mannequin_layers()),
        "",
        "palette colours (#rrggbb; a missing one is derived from its neighbours):",
        "  " + ", ".join(PALETTE_KEYS),
        "",
        "wardrobe -- how the character is dressed. Every key is optional; the",
        "defaults below are a short-sleeved belted tunic open at the throat over",
        "bare legs, sandals, a short back and sides and a full beard:",
        f"  sleeves     (default {d.sleeves!r}) -- "
        + "; ".join(f"{k}: {_span_help(v)}" for k, v in SLEEVES.items())
        + ". Painted in 'tunic'.",
        f"  legs        (default {d.legs!r}) -- "
        + "; ".join(f"{k}: {_span_help(v)}" for k, v in LEGS.items())
        + ". Painted in 'legwear', which defaults to 'tunic'.",
        f"  footwear    (default {d.footwear!r}) -- "
        + "; ".join(f"{k}: {_span_help(v)}" for k, v in FOOTWEAR.items())
        + ". Painted in 'sandal'.",
        f"  hair        (default {d.hair!r}) -- "
        + "; ".join(f"{k}: {_hair_help(v)}" for k, v in HAIR.items())
        + ". The crown, the back of the head and the brow fringe come with every"
        + " length; hair past the ear also frames the face and falls to a cut"
        + " line in 'hair_shadow'. Painted in 'hair'.",
        f"  facial_hair (default {d.facial_hair!r}) -- "
        + ", ".join(FACIAL_HAIR)
        + ". Painted in 'beard'.",
        f"  collar      (default {d.collar!r}) -- "
        + ", ".join(COLLAR)
        + ". 'open' leaves the V of bare skin a tunic has at the throat;"
        + " 'closed' is a jacket that fastens; 'high' is 'closed' with a collar"
        + " ring on the torso's overlay shell.",
        f"  hood        (default {d.hood!r}) -- "
        + ", ".join(HOOD)
        + ". 'up' covers the head's overlay shell but for the face and falls"
        + " onto the shoulders. Painted in 'hood', its rim in 'hood_shadow'.",
        f"  greying     (default {d.greying!r}) -- "
        + ", ".join(GREYING)
        + ". Streaks 'hair_grey' / 'beard_grey' through whichever it names."
        + " 'features.greying' is the older spelling of 'beard', and a sheet"
        + " carrying both is refused.",
        "",
        "An unknown field, palette colour, wardrobe key or wardrobe value is",
        "refused by name: a misspelling would otherwise compose the default",
        "costume and say nothing.",
        "",
        "Beards and hair are painted on the overlay shell as well as the skull, so",
        "they stand half a pixel off the head. The shell is the only volume there",
        "is: a brim, a crest, a bun, a cloak or a coat that hangs open have no",
        "geometry and are not drawn.",
    ]
    return "\n".join(lines)


def _load_entries(path: Path, only_id: str | None) -> List[CastEntry]:
    data = json.loads(path.read_text(encoding="utf-8"))
    rows = data["skins"] if isinstance(data, dict) else data
    entries = [CastEntry.from_dict(r) for r in rows]
    if only_id:
        entries = [e for e in entries if e.texture_id == only_id]
        if not entries:
            raise SystemExit(f"no cast entry with texture_id {only_id!r} in {path}")
    return entries


def _write_png(entry: CastEntry, out_dir: Path) -> bytes:
    out_dir.mkdir(parents=True, exist_ok=True)
    png = compose_png_bytes(entry)
    (out_dir / f"{entry.texture_id}.png").write_bytes(png)
    return png


def cmd_build(args) -> int:
    for e in _load_entries(Path(args.cast), args.id):
        _write_png(e, Path(args.out_dir))
        print(f"composed {e.texture_id}.png ({e.model or e.entity})")
    return 0


def cmd_preview(args) -> int:
    for e in _load_entries(Path(args.cast), args.id):
        img = compose_preview_skin(e)
        paths = render_previews(img, Path(args.out_dir), e.texture_id, args.scale)
        print(f"rendered {len(paths)} previews for {e.texture_id}")
    return 0


def cmd_catalog(args) -> int:
    for e in _load_entries(Path(args.cast), args.id):
        png = compose_png_bytes(e)
        names = [f"{e.texture_id}-{n}.png" for n in ("front", "left", "right", "back")]
        card = catalog_card(e, png, names)
        out = Path(args.out_dir)
        out.mkdir(parents=True, exist_ok=True)
        (out / f"skin-{e.texture_id}.json").write_text(dumps(card), encoding="utf-8")
        print(f"wrote catalog/skin-{e.texture_id}.json")
    return 0


def cmd_all(args) -> int:
    for e in _load_entries(Path(args.cast), args.id):
        png = _write_png(e, Path(args.skins_dir))
        img = compose_preview_skin(e)
        preview_paths = render_previews(
            img, Path(args.preview_dir), e.texture_id, args.scale
        )
        names = [p.name for p in preview_paths]
        card = catalog_card(e, png, names)
        cat = Path(args.catalog_dir)
        cat.mkdir(parents=True, exist_ok=True)
        (cat / f"skin-{e.texture_id}.json").write_text(dumps(card), encoding="utf-8")
        print(
            f"{e.texture_id}: skin + {len(preview_paths)} previews + catalog card"
        )
    return 0


def parts_table(key: str) -> str:
    """A model's boxes, read from the model-part table (spec-0097 §6.4)."""
    w, h = models.texture_size(key)
    lines = [f"{key}: a {w}x{h} sheet, from {models.TABLE_PATH.name}"]
    bs = models.boxes(key)
    outer = not any(b.grow == 0 for b in bs)
    for b in bs:
        layer = "shell" if (b.grow > 0 and not outer) else ("outer" if outer else "base")
        notes = []
        if b.mirror:
            notes.append("mirrored")
        if b.posed:
            notes.append("posed")
        tail = f" ({', '.join(notes)})" if notes else ""
        lines.append(
            f"  {b.part:28} {layer:5} texOffs {b.u},{b.v}  box {b.w}x{b.h}x{b.d}  "
            f"grow {b.grow}{tail}"
        )
        for face, (x, y, fw, fh) in b.face_rects().items():
            if fw and fh:
                lines.append(f"      {face:5} x {x}..{x + fw - 1}, y {y}..{y + fh - 1}")
    return "\n".join(lines)


def cmd_parts(args) -> int:
    print(parts_table(args.model))
    return 0


def main(argv: List[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="delve_skin",
        description=__doc__,
        epilog=_entry_surface_help(),
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    sub = parser.add_subparsers(dest="cmd", required=True)

    def add_common(sp):
        # The entry surface is what a creator is about to write, so it is on
        # every subcommand's --help, not only on the bare one.
        sp.epilog = _entry_surface_help()
        sp.formatter_class = argparse.RawDescriptionHelpFormatter
        sp.add_argument("cast", help="path to a cast-sheet JSON file")
        sp.add_argument("--id", default=None, help="only this texture_id")

    b = sub.add_parser("build", help="compose skin PNG(s)")
    add_common(b)
    b.add_argument("--out-dir", required=True)
    b.set_defaults(func=cmd_build)

    p = sub.add_parser("preview", help="render preview PNGs")
    add_common(p)
    p.add_argument("--out-dir", required=True)
    p.add_argument("--scale", type=int, default=DEFAULT_SCALE)
    p.set_defaults(func=cmd_preview)

    c = sub.add_parser("catalog", help="emit catalog card(s)")
    add_common(c)
    c.add_argument("--out-dir", required=True)
    c.set_defaults(func=cmd_catalog)

    a = sub.add_parser("all", help="skin + previews + catalog card")
    add_common(a)
    a.add_argument("--skins-dir", required=True)
    a.add_argument("--catalog-dir", required=True)
    a.add_argument("--preview-dir", required=True)
    a.add_argument("--scale", type=int, default=DEFAULT_SCALE)
    a.set_defaults(func=cmd_all)

    m = sub.add_parser("parts", help="print a model's boxes from the model-part table")
    m.add_argument("model", choices=models.model_keys())
    m.set_defaults(func=cmd_parts)

    args = parser.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
