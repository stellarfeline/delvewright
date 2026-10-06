#!/usr/bin/env python3
"""Derive the pinned client's texture census from its jar (spec-0084 §3.1).

`crates/delvec/data/textures-<version>.json` is what a `world.textures[]` row's
`replaces` is resolved against (`DW0939`) and what its image is measured
against (`DW0940`): one entry per `assets/minecraft/textures/**.png` the pinned
client ships, keyed by the resource location a campaign writes
(`minecraft:<path>`, without `textures/` and `.png`), carrying

    w, h       the PNG's pixel size, read off its header;
    sha256     the sha256 of vanilla's bytes (an override equal to it replaces
               nothing);
    mcmeta     whether vanilla ships a `.png.mcmeta` sidecar beside it;
    animated   whether that sidecar carries an `animation` object;
    fw, fh     for an animated texture, one frame's size by vanilla's own rule
               (`animation.width`/`height` where stated, else the square of the
               shorter side), which is the unit an override is scaled from.

The table is vendored because a build never reaches for a jar and never
reaches the network (ADR-0006): the jar is EULA-bound and is never committed,
and the census carries facts about it — sizes and digests — not its pixels.

The jar is REFUSED unless its sha256 is `versions.toml` `[render]
textures_sha256`: a census of some other jar is not a census of the pin. The
table states that sha256 in its header, so the file says which jar it
describes.

    python3 tools/maintenance/derive-client-textures.py <client jar>          # write the table
    python3 tools/maintenance/derive-client-textures.py <client jar> --check  # compare, write nothing

Run when ADR-0009's Minecraft pin moves. Human-in-the-loop: it is never run by
CI or by a build. Output is `delvec fmt` canonical (sorted keys, two-space
indent, raw UTF-8, one trailing newline).
"""

from __future__ import annotations

import argparse
import hashlib
import json
import struct
import sys
import zipfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "tools" / "lib"))
import versions  # noqa: E402

PREFIX = "assets/minecraft/textures/"
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


def die(msg: str) -> "None":
    print(f"derive-client-textures: FAIL — {msg}", file=sys.stderr)
    raise SystemExit(1)


def png_size(name: str, raw: bytes) -> tuple[int, int]:
    if raw[:8] != PNG_SIGNATURE or raw[12:16] != b"IHDR":
        die(f"{name} is not a PNG with a leading IHDR")
    return struct.unpack(">II", raw[16:24])


def frame_size(anim: dict, w: int, h: int) -> tuple[int, int]:
    """Vanilla's frame-size rule for an animation sidecar."""
    fw, fh = anim.get("width"), anim.get("height")
    if fw is not None:
        return (fw, fh if fh is not None else h)
    if fh is not None:
        return (w, fh)
    s = min(w, h)
    return (s, s)


def census(jar: zipfile.ZipFile) -> dict:
    names = set(jar.namelist())
    rows: dict[str, dict] = {}
    for name in sorted(names):
        if not name.startswith(PREFIX) or not name.endswith(".png"):
            continue
        raw = jar.read(name)
        w, h = png_size(name, raw)
        path = name[len(PREFIX) : -len(".png")]
        row: dict = {
            "h": h,
            "mcmeta": False,
            "sha256": hashlib.sha256(raw).hexdigest(),
            "w": w,
        }
        side = name + ".mcmeta"
        if side in names:
            row["mcmeta"] = True
            meta = json.loads(jar.read(side).decode("utf-8"))
            anim = meta.get("animation")
            if isinstance(anim, dict):
                row["animated"] = True
                fw, fh = frame_size(anim, w, h)
                row["fw"], row["fh"] = fw, fh
        rows[f"minecraft:{path}"] = row
    return rows


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("jar", help="the pinned Minecraft client jar")
    ap.add_argument("--check", action="store_true", help="compare against the committed table, write nothing")
    args = ap.parse_args()

    pin = versions.load()
    mc = pin["minecraft"]["version"]
    want = pin["render"]["textures_sha256"]
    raw = Path(args.jar).read_bytes()
    got = hashlib.sha256(raw).hexdigest()
    if got != want:
        die(f"{args.jar} has sha256 {got}; versions.toml [render] textures_sha256 is {want}")

    with zipfile.ZipFile(args.jar) as jar:
        rows = census(jar)
    if not rows:
        die(f"{args.jar} holds no {PREFIX}**.png — the census would bind nothing")
    doc = {"client_jar_sha256": got, "minecraft": mc, "textures": rows}
    text = json.dumps(doc, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    out = REPO / "crates" / "delvec" / "data" / f"textures-{mc}.json"
    animated = sum(1 for r in rows.values() if r.get("animated"))
    sidecars = sum(1 for r in rows.values() if r["mcmeta"])
    print(
        f"derive-client-textures: {len(rows)} texture(s), {sidecars} with a sidecar, "
        f"{animated} animated, from {args.jar} (sha256 {got})"
    )
    if args.check:
        have = out.read_text(encoding="utf-8") if out.exists() else None
        if have != text:
            die(f"{out.relative_to(REPO)} differs from what this jar derives — regenerate and commit it")
        print(f"derive-client-textures: {out.relative_to(REPO)} is what this jar derives, byte for byte")
        return 0
    out.write_text(text, encoding="utf-8")
    print(f"derive-client-textures: wrote {out.relative_to(REPO)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
