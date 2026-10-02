#!/usr/bin/env python3
"""SPIKE TOOLING (texture overrides, spec-0084) — the measurement half of
`run.sh`. Reads the pinned 1.21.11 client jar (and, when handed one, the pinned
Chunky core) and writes every fact the spec cites to `observations.json`.

Deterministic and offline: stdlib only, sorted keys, no timestamps. Refuses a jar
whose sha256 is not the pinned client's, so a reading here is a reading of the
pin and nothing else.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import struct
import sys
import zipfile
from collections import Counter

# The 1.21.11 client jar, by content. The same digest spec-0080 and spec-0081
# read their client facts from.
PINNED_CLIENT_SHA256 = "1473c9489ac50fda3c435049a76a70d61a10b8610db27f5ba9d8756b686cd3bd"

# The textures the two motivating overrides replace, plus the sun for scale.
WATCHED_TEXTURES = (
    "assets/minecraft/textures/environment/celestial/sun.png",
    "assets/minecraft/textures/environment/celestial/end_flash.png",
    "assets/minecraft/textures/entity/zombie/drowned.png",
    "assets/minecraft/textures/entity/zombie/drowned_outer_layer.png",
    "assets/minecraft/textures/entity/zombie/zombie.png",
)
MOON_DIR = "assets/minecraft/textures/environment/celestial/moon/"

# Strings the client's pack loader and pack directories are known by. Each is
# searched for in the constant pools of every class; a hit is recorded with its
# count, a miss as 0 — a zero here is a finding, not a pass.
CLIENT_STRINGS = (
    "min_format",
    "max_format",
    "pack_format",
    ", but is missing mandatory fields min_format and max_format",
    " missing field, must declare both min_format and max_format",
    "downloads",
    "resourcepacks",
    "environment/celestial",
    "textures/atlas/celestials.png",
    "server_resource_pack",
)

PRINTABLE = re.compile(rb"[\x20-\x7e]{4,}")


def png_size(data: bytes) -> list[int] | None:
    """Width and height off the IHDR chunk, or None when the bytes are not a PNG."""
    if data[:8] != b"\x89PNG\r\n\x1a\n" or data[12:16] != b"IHDR":
        return None
    w, h = struct.unpack(">II", data[16:24])
    return [w, h]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--jar", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--chunky-core", default=None)
    args = ap.parse_args()

    with open(args.jar, "rb") as f:
        raw = f.read()
    sha = hashlib.sha256(raw).hexdigest()
    if sha != PINNED_CLIENT_SHA256:
        print(
            f"[spike] refusing: {args.jar} has sha256 {sha}, the pinned client is {PINNED_CLIENT_SHA256}",
            file=sys.stderr,
        )
        return 1

    zf = zipfile.ZipFile(args.jar)
    names = zf.namelist()

    version = json.loads(zf.read("version.json"))

    # One count per `assets/minecraft/<class>/` directory — the whole surface the
    # client's pack format reads, enumerated rather than listed.
    classes: Counter[str] = Counter()
    for n in names:
        m = re.match(r"assets/minecraft/([a-z_]+)/", n)
        if m and not n.endswith("/"):
            classes[m.group(1)] += 1

    textures = [n for n in names if n.startswith("assets/minecraft/textures/")]
    png = [n for n in textures if n.endswith(".png")]
    mcmeta = [n for n in textures if n.endswith(".png.mcmeta")]
    other = [n for n in textures if not (n.endswith(".png") or n.endswith(".png.mcmeta"))]

    moon = sorted(n for n in names if n.startswith(MOON_DIR) and n.endswith(".png"))
    watched = {}
    for n in list(WATCHED_TEXTURES) + moon:
        if n in names:
            data = zf.read(n)
            watched[n] = {"bytes": len(data), "size": png_size(data)}
        else:
            watched[n] = None

    # The client's constant pools: every printable run of every class, counted.
    strings: Counter[bytes] = Counter()
    class_count = 0
    for n in names:
        if n.endswith(".class"):
            class_count += 1
            for s in PRINTABLE.findall(zf.read(n)):
                strings[s] += 1
    client_strings = {}
    for want in CLIENT_STRINGS:
        key = want.encode()
        # Exact-run hits, plus runs that contain the wanted text (a message is
        # one constant with the text inside it).
        exact = strings.get(key, 0)
        within = sum(c for s, c in strings.items() if key in s)
        client_strings[want] = {"exact": exact, "within": within}

    chunky = None
    if args.chunky_core:
        cz = zipfile.ZipFile(args.chunky_core)
        help_lines = []
        layered = [n for n in cz.namelist() if "LayeredResourcePacks" in n]
        for n in cz.namelist():
            if n.startswith("se/llbit/chunky/main/CommandLineOptions") and n.endswith(".class"):
                for s in PRINTABLE.findall(cz.read(n)):
                    t = s.decode()
                    if "-texture" in t:
                        # A constant's length prefix can be a printable byte and
                        # then rides in front of the text; the option starts at
                        # its dash.
                        help_lines.append(t[t.index("-texture") :].strip())
        chunky = {
            "core": args.chunky_core.rsplit("/", 1)[-1],
            "sha256": hashlib.sha256(open(args.chunky_core, "rb").read()).hexdigest(),
            "texture_options": sorted(set(help_lines)),
            "layered_resource_pack_classes": sorted(layered),
        }

    out = {
        "client_jar": {
            "sha256": sha,
            "bytes": len(raw),
            "entries": len(names),
            "classes": class_count,
            "version_json": version,
        },
        "asset_classes": dict(sorted(classes.items())),
        "texture_census": {
            "png": len(png),
            "png_mcmeta": len(mcmeta),
            "other": sorted(other),
            "total": len(textures),
        },
        "watched_textures": dict(sorted(watched.items())),
        "moon_phase_textures": moon,
        "client_strings": client_strings,
        "chunky": chunky,
        "binding": {
            "asset_classes_counted": sum(classes.values()),
            "watched_textures_present": sum(1 for v in watched.values() if v),
            "watched_textures_asked": len(watched),
            "client_strings_asked": len(CLIENT_STRINGS),
            "client_strings_found": sum(1 for v in client_strings.values() if v["within"]),
        },
    }
    for k, v in out["binding"].items():
        if v == 0:
            print(f"[spike] zero binding: {k}", file=sys.stderr)
            return 1
    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(out, f, indent=2, sort_keys=True)
        f.write("\n")
    b = out["binding"]
    print(
        f"[spike] {b['asset_classes_counted']} asset entries in {len(classes)} classes; "
        f"{len(png)} PNG textures, {len(mcmeta)} animation sidecars; "
        f"{b['watched_textures_present']}/{b['watched_textures_asked']} watched textures present; "
        f"{b['client_strings_found']}/{b['client_strings_asked']} client strings found; "
        f"chunky reading {'present' if chunky else 'absent'}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
