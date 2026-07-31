"""``python -m delve_skin`` -- compose skins, render previews, emit catalog cards.

Usage:
  python -m delve_skin build   CAST.json --out-dir DIR [--id ID]
  python -m delve_skin preview CAST.json --out-dir DIR [--id ID] [--scale N]
  python -m delve_skin catalog CAST.json --out-dir DIR [--id ID]
  python -m delve_skin all     CAST.json --skins-dir DIR --catalog-dir DIR \
                               --preview-dir DIR [--id ID]

A CAST file is ``{"campaign": "...", "skins": [ <entry>, ... ]}`` or a bare list
of entries. Each entry: texture_id, model (wide|slim, REQUIRED), palette, ...
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import List

from delve_skin.catalog import catalog_card, dumps
from delve_skin.compose import CastEntry, compose_png_bytes, compose_skin
from delve_skin.preview import DEFAULT_SCALE, render_previews


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
        print(f"composed {e.texture_id}.png ({e.model})")
    return 0


def cmd_preview(args) -> int:
    for e in _load_entries(Path(args.cast), args.id):
        img = compose_skin(e)
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
        img = compose_skin(e)
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


def main(argv: List[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="delve_skin", description=__doc__)
    sub = parser.add_subparsers(dest="cmd", required=True)

    def add_common(sp):
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

    args = parser.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
