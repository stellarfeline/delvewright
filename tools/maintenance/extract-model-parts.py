#!/usr/bin/env python3
"""Regenerate `crates/delvec/data/model-parts-1.21.11.json` from the pinned client.

The table is what a skin is drawn to and judged against (spec-0097): for every
humanoid entity model a delve can retexture, every box the model builds -- its
`texOffs`, its size, its grow, whether it is mirrored, and how high above the
ground it stands at rest -- and the vanilla textures that model draws. The
mannequin's `hidden_layers` field and the eye height of a standing player are
read from the same jar. `delvec` judges a skin or a `world.textures[]` row
against it (`DW0978`, `DW0979`); `delve_skin` composes on it.

## Inputs (never committed: Mojang EULA)

    --client-jar  the 1.21.11 client jar. REFUSED unless its sha256 is
                  `versions.toml` `[render] textures_sha256` and its sha1 is
                  piston-meta's `downloads.client.sha1`.
    --work        a scratch directory; the mappings and the client's libraries
                  are fetched into it from piston-meta and verified by sha1.

A JDK at or above the pin's `javaVersion.majorVersion` must be on PATH (or under
`$JAVA_HOME/bin`). Network access is needed for piston-meta.

## Method

1. **The game builds the boxes.** `tools/maintenance/modelparts/ModelPartDump.java`
   boots the registries, calls the client's own `LayerDefinitions.createRoots()`
   and writes every named layer's `LayerDefinition` by reflection: the part tree,
   each part's `PartPose`, each `CubeDefinition`. Every obfuscated name is
   resolved from the official mappings at run time; none is written down.
2. **This script flattens it.** Each cube becomes one row: its part path, `u`,
   `v`, `w`, `h`, `d`, `grow`, `mirror`, and the height of its top and bottom
   above the ground in model pixels at rest (the pose chain applied; ground is
   model y = 24, where every humanoid renderer stands the model). A cube on a
   chain with a rotation is marked `posed`, and its height is not used.
3. **Second method, sharing nothing with the dumper.** Every vanilla texture the
   table binds to a layer is read out of the jar as a PNG, and every opaque pixel
   must fall on a face some box of that layer samples: vanilla's own art passes
   the refusal the table enforces, or this script refuses. The per-texture count
   of opaque pixels inside and outside the footprint is printed.
4. **The binding is authored and checked.** Which texture a layer is drawn with
   is set by a renderer, not by the mesh, so `BINDINGS` names it, citing the
   renderer class; step 3 is what keeps it honest (a texture bound to the wrong
   model lands paint outside the footprint).

The table names the instrument by exact revision: the last commit that touched
this script or the dumper. The script refuses when either file is dirty, because
a table written by an uncommitted instrument names no instrument.

    python3 tools/maintenance/extract-model-parts.py --client-jar <jar> --work <dir>
    python3 tools/maintenance/extract-model-parts.py --client-jar <jar> --work <dir> --check

Human-in-the-loop: never run by CI or by a build. Output is `delvec fmt`
canonical (sorted keys, two-space indent, one trailing newline).
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import urllib.request
import zipfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "tools" / "lib"))
import versions  # noqa: E402

DUMPER = REPO / "tools" / "maintenance" / "modelparts" / "ModelPartDump.java"
MANIFEST = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"

#: The ground a humanoid renderer stands its model on, in model pixels: every
#: humanoid `LivingEntityRenderer` translates the pose by -1.501 blocks after
#: flipping y, so model y = 24 is the soles of the feet.
GROUND_Y = 24.0

#: model key -> (ModelLayers field, vanilla textures drawn with it, renderer that
#: binds them). The texture list may name a directory with a trailing `/`, which
#: binds every PNG under it.
BINDINGS: dict[str, tuple[str, list[str], str]] = {
    "player": ("PLAYER", ["entity/player/wide/"], "AvatarRenderer (wide)"),
    "player_slim": ("PLAYER_SLIM", ["entity/player/slim/"], "AvatarRenderer (slim)"),
    "zombie": ("ZOMBIE", ["entity/zombie/zombie"], "ZombieRenderer"),
    "husk": ("HUSK", ["entity/zombie/husk"], "HuskRenderer"),
    "drowned": ("DROWNED", ["entity/zombie/drowned"], "DrownedRenderer"),
    "drowned_outer_layer": (
        "DROWNED_OUTER_LAYER",
        ["entity/zombie/drowned_outer_layer"],
        "DrownedOuterLayer",
    ),
    "skeleton": ("SKELETON", ["entity/skeleton/skeleton"], "SkeletonRenderer"),
    "wither_skeleton": (
        "WITHER_SKELETON",
        ["entity/skeleton/wither_skeleton"],
        "WitherSkeletonRenderer",
    ),
    "stray": ("STRAY", ["entity/skeleton/stray"], "StrayRenderer"),
    "stray_outer_layer": (
        "STRAY_OUTER_LAYER",
        ["entity/skeleton/stray_overlay"],
        "SkeletonClothingLayer (stray)",
    ),
    "bogged": ("BOGGED", ["entity/skeleton/bogged"], "BoggedRenderer"),
    "bogged_outer_layer": (
        "BOGGED_OUTER_LAYER",
        ["entity/skeleton/bogged_overlay"],
        "SkeletonClothingLayer (bogged)",
    ),
    "parched": ("PARCHED", ["entity/skeleton/parched"], "ParchedRenderer"),
    "villager": ("VILLAGER", ["entity/villager/"], "VillagerRenderer, VillagerProfessionLayer"),
    "wandering_trader": ("WANDERING_TRADER", ["entity/wandering_trader"], "WanderingTraderRenderer"),
    "zombie_villager": (
        "ZOMBIE_VILLAGER",
        ["entity/zombie_villager/"],
        "ZombieVillagerRenderer, VillagerProfessionLayer",
    ),
    "piglin": ("PIGLIN", ["entity/piglin/piglin"], "PiglinRenderer"),
    "piglin_brute": ("PIGLIN_BRUTE", ["entity/piglin/piglin_brute"], "PiglinRenderer"),
    "zombified_piglin": (
        "ZOMBIFIED_PIGLIN",
        ["entity/piglin/zombified_piglin"],
        "ZombifiedPiglinRenderer",
    ),
}

#: The box unwrap (`ModelPart.Cube`): face -> (du, dv, width, height) in terms of
#: the box's w, h, d. Face names are the observer's, as `delve_skin` names them:
#: `left` is the side on the observer's left when facing the model's front.
FACES = ("up", "down", "left", "front", "right", "back")


def face_rects(u: int, v: int, w: int, h: int, d: int) -> dict[str, tuple[int, int, int, int]]:
    return {
        "up": (u + d, v, w, d),
        "down": (u + d + w, v, w, d),
        "left": (u, v + d, d, h),
        "front": (u + d, v + d, w, h),
        "right": (u + d + w, v + d, d, h),
        "back": (u + 2 * d + w, v + d, w, h),
    }


def die(msg: str) -> "None":
    print(f"extract-model-parts: FAIL — {msg}", file=sys.stderr)
    raise SystemExit(1)


def sha(path: Path, algo: str) -> str:
    return hashlib.new(algo, path.read_bytes()).hexdigest()


def fetch(url: str, dest: Path, sha1: str) -> None:
    if not dest.exists() or sha(dest, "sha1") != sha1:
        with urllib.request.urlopen(url) as r, open(dest, "wb") as f:
            shutil.copyfileobj(r, f)
    got = sha(dest, "sha1")
    if got != sha1:
        die(f"{dest.name}: sha1 {got}, piston-meta says {sha1}")


def instrument_revision() -> str:
    paths = [str(Path(__file__).resolve().relative_to(REPO)), str(DUMPER.relative_to(REPO))]
    dirty = subprocess.run(
        ["git", "-C", str(REPO), "status", "--porcelain", "--", *paths],
        capture_output=True, text=True, check=True,
    ).stdout.strip()
    if dirty:
        die(f"the instrument is uncommitted ({dirty!r}) — commit it, then measure")
    rev = subprocess.run(
        ["git", "-C", str(REPO), "log", "-1", "--format=%H", "--", *paths],
        capture_output=True, text=True, check=True,
    ).stdout.strip()
    if not rev:
        die("the instrument has no commit")
    return rev


def java(tool: str) -> str:
    home = os.environ.get("JAVA_HOME")
    return str(Path(home) / "bin" / tool) if home else tool


def run_dumper(jar: Path, work: Path, version: dict, mappings: Path) -> dict:
    libs = work / "libraries"
    libs.mkdir(exist_ok=True)
    cp = [str(jar)]
    for lib in version["libraries"]:
        art = lib.get("downloads", {}).get("artifact")
        if not art or "rules" in lib:
            continue
        dest = libs / art["path"].split("/")[-1]
        fetch(art["url"], dest, art["sha1"])
        cp.append(str(dest))
    classpath = os.pathsep.join(cp)
    classes = work / "classes"
    if classes.exists():
        shutil.rmtree(classes)
    classes.mkdir()
    # A build failure is a tool failure: never fall through to stale classes.
    subprocess.run(
        [java("javac"), "-nowarn", "-cp", classpath, "-d", str(classes), str(DUMPER)], check=True
    )
    out = work / "model-parts-dump.json"
    layers = sorted({b[0] for b in BINDINGS.values()})
    proc = subprocess.run(
        [java("java"), "-Xmx2g", "-cp", classpath + os.pathsep + str(classes),
         "dw.ModelPartDump", str(mappings), str(out), *layers],
        capture_output=True, text=True, cwd=work,
    )
    if proc.returncode != 0:
        die(f"the dumper failed:\n{proc.stderr[-4000:]}")
    m = re.search(r"DUMPED layers=(\d+) roots=(\d+)", proc.stdout)
    if not m or int(m.group(1)) != len(layers):
        die(f"the dumper reported {proc.stdout.strip()!r}, asked for {len(layers)} layers")
    print(f"extract-model-parts: dumper {m.group(0)}")
    return json.loads(out.read_text(encoding="utf-8"))


def num(x: float) -> float | int:
    """A model value, as an int where it is whole, rounded to 4 places otherwise."""
    r = round(float(x), 4)
    return int(r) if r == int(r) else r


def flatten(layer: dict) -> list[dict]:
    """Every cube of a layer, with its part path and resting height above ground."""
    rows: list[dict] = []

    def walk(part: dict, path: str, origin, scale, posed: bool) -> None:
        pose = part["partPose"]
        # PartPose applies translate, then rotate, then scale (ModelPart.translateAndRotate).
        o = tuple(origin[i] + scale[i] * pose[k] for i, k in enumerate(("x", "y", "z")))
        s = tuple(scale[i] * pose[k] for i, k in enumerate(("xScale", "yScale", "zScale")))
        p = posed or any(pose[k] != 0.0 for k in ("xRot", "yRot", "zRot"))
        for cube in part["cubes"]:
            dims = cube["dimensions"]
            w, h, d = dims["x"], dims["y"], dims["z"]
            for v in (w, h, d):
                if v != int(v):
                    die(f"{path}: a box of {w}x{h}x{d} is not whole texels")
            if cube["texScale"]["u"] != 1.0 or cube["texScale"]["v"] != 1.0:
                die(f"{path}: a scaled texture on a box")
            if len(cube["visibleFaces"]) != 6:
                die(f"{path}: a box that hides faces ({cube['visibleFaces']})")
            grow = cube["grow"]
            if not (grow["growX"] == grow["growY"] == grow["growZ"]):
                die(f"{path}: an uneven grow {grow}")
            g = grow["growY"]
            y_min = o[1] + s[1] * (cube["origin"]["y"] - g)
            y_max = o[1] + s[1] * (cube["origin"]["y"] + h + g)
            rows.append(
                {
                    "part": path,
                    "u": int(cube["texCoord"]["u"]),
                    "v": int(cube["texCoord"]["v"]),
                    "w": int(w),
                    "h": int(h),
                    "d": int(d),
                    "grow": num(g),
                    "mirror": bool(cube["mirror"]),
                    "posed": p,
                    # Model y grows downward; height grows upward from the ground.
                    "top": num(GROUND_Y - y_min),
                    "bottom": num(GROUND_Y - y_max),
                }
            )
        for name in sorted(part["children"]):
            walk(part["children"][name], f"{path}/{name}" if path else name, o, s, p)

    root = layer["mesh"]["root"]
    for name in sorted(root["children"]):
        rp = root["partPose"]
        walk(
            root["children"][name],
            name,
            (rp["x"], rp["y"], rp["z"]),
            (rp["xScale"], rp["yScale"], rp["zScale"]),
            any(rp[k] != 0.0 for k in ("xRot", "yRot", "zRot")),
        )
    if root["cubes"]:
        die("a layer whose root carries boxes of its own")
    return rows


def png_rgba(raw: bytes) -> tuple[int, int, bytes]:
    """Decode a PNG to RGBA with the standard library only (8-bit, any colour type
    vanilla's entity textures use), so the second method shares no decoder with
    the game or with `delve_skin`."""
    import struct
    import zlib

    if raw[:8] != b"\x89PNG\r\n\x1a\n":
        die("not a PNG")
    pos, idat, plte, trns = 8, b"", None, None
    w = h = depth = ctype = 0
    while pos < len(raw):
        (length,) = struct.unpack(">I", raw[pos : pos + 4])
        kind = raw[pos + 4 : pos + 8]
        data = raw[pos + 8 : pos + 8 + length]
        pos += 12 + length
        if kind == b"IHDR":
            w, h, depth, ctype, _, _, interlace = struct.unpack(">IIBBBBB", data)
            if depth != 8 or interlace != 0:
                die(f"a PNG of depth {depth}, interlace {interlace}")
        elif kind == b"PLTE":
            plte = data
        elif kind == b"tRNS":
            trns = data
        elif kind == b"IDAT":
            idat += data
    channels = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}[ctype]
    stride = w * channels
    flat = zlib.decompress(idat)
    rows: list[bytearray] = []
    prev = bytearray(stride)
    i = 0
    for _ in range(h):
        f = flat[i]
        line = bytearray(flat[i + 1 : i + 1 + stride])
        i += 1 + stride
        for x in range(stride):
            a = line[x - channels] if x >= channels else 0
            b = prev[x]
            c = prev[x - channels] if x >= channels else 0
            if f == 1:
                line[x] = (line[x] + a) & 0xFF
            elif f == 2:
                line[x] = (line[x] + b) & 0xFF
            elif f == 3:
                line[x] = (line[x] + (a + b) // 2) & 0xFF
            elif f == 4:
                pa, pb, pc = abs(b - c), abs(a - c), abs(a + b - 2 * c)
                pr = a if pa <= pb and pa <= pc else b if pb <= pc else c
                line[x] = (line[x] + pr) & 0xFF
        rows.append(line)
        prev = line
    out = bytearray()
    for line in rows:
        for x in range(w):
            px = line[x * channels : (x + 1) * channels]
            if ctype == 6:
                out += px
            elif ctype == 2:
                out += px + b"\xff"
            elif ctype == 0:
                out += bytes([px[0]] * 3) + b"\xff"
            elif ctype == 4:
                out += bytes([px[0]] * 3) + bytes([px[1]])
            else:
                idx = px[0]
                rgb = plte[idx * 3 : idx * 3 + 3]
                alpha = trns[idx] if trns is not None and idx < len(trns) else 255
                out += rgb + bytes([alpha])
    return w, h, bytes(out)


def footprint(cubes: list[dict], k: int) -> set[tuple[int, int]]:
    cells: set[tuple[int, int]] = set()
    for c in cubes:
        for x0, y0, fw, fh in face_rects(c["u"], c["v"], c["w"], c["h"], c["d"]).values():
            for y in range(y0 * k, (y0 + fh) * k):
                for x in range(x0 * k, (x0 + fw) * k):
                    cells.add((x, y))
    return cells


def cross_check(jar: zipfile.ZipFile, models: dict) -> list[str]:
    report: list[str] = []
    bound = 0
    for key, model in models.items():
        tw, th = model["texture_size"]
        for tex in model["textures"]:
            img = jar.read(f"assets/minecraft/textures/{tex.removeprefix('minecraft:')}.png")
            w, h, rgba = png_rgba(img)
            if w % tw or h * tw != w * th:
                die(f"{tex} is {w}x{h}; {model['layer']} samples a {tw}x{th} sheet")
            k = w // tw
            cells = footprint(model["cubes"], k)
            inside = outside = 0
            stray: list[tuple[int, int]] = []
            for y in range(h):
                for x in range(w):
                    if rgba[(y * w + x) * 4 + 3] == 0:
                        continue
                    if (x, y) in cells:
                        inside += 1
                    else:
                        outside += 1
                        stray.append((x, y))
            bound += 1
            report.append(f"{key:22} {tex:58} opaque inside {inside:5}, outside {outside}")
            if outside:
                die(
                    f"vanilla's {tex} paints {outside} opaque pixel(s) no box of "
                    f"{model['layer']} samples (first {stray[:6]}) — the binding or the "
                    "table is wrong"
                )
    if bound == 0:
        die("the cross-check bound no texture")
    report.append(f"cross-check: {bound} vanilla texture(s) bound, every opaque pixel on a sampled face")
    return report


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--client-jar", required=True, type=Path)
    ap.add_argument("--work", required=True, type=Path)
    ap.add_argument("--check", action="store_true", help="compare against the committed table, write nothing")
    a = ap.parse_args()

    pin = versions.load()
    mc = pin["minecraft"]["version"]
    jar_sha256 = sha(a.client_jar, "sha256")
    if jar_sha256 != pin["render"]["textures_sha256"]:
        die(f"{a.client_jar} has sha256 {jar_sha256}; versions.toml [render] textures_sha256 is "
            f"{pin['render']['textures_sha256']}")
    revision = instrument_revision()
    a.work.mkdir(parents=True, exist_ok=True)

    with urllib.request.urlopen(MANIFEST) as r:
        manifest = json.load(r)
    entry = next((v for v in manifest["versions"] if v["id"] == mc), None)
    if entry is None:
        die(f"piston-meta lists no {mc}")
    vjson = a.work / f"{mc}.json"
    fetch(entry["url"], vjson, entry["sha1"])
    version = json.loads(vjson.read_text(encoding="utf-8"))
    jar_sha1 = sha(a.client_jar, "sha1")
    if jar_sha1 != version["downloads"]["client"]["sha1"]:
        die(f"{a.client_jar} has sha1 {jar_sha1}; piston-meta's client is "
            f"{version['downloads']['client']['sha1']}")
    maps = version["downloads"]["client_mappings"]
    mappings = a.work / "client-mappings.txt"
    fetch(maps["url"], mappings, maps["sha1"])

    dump = run_dumper(a.client_jar, a.work, version, mappings)

    with zipfile.ZipFile(a.client_jar) as jar:
        textures = sorted(
            n[len("assets/minecraft/textures/") : -len(".png")]
            for n in jar.namelist()
            if n.startswith("assets/minecraft/textures/") and n.endswith(".png")
        )
        models: dict[str, dict] = {}
        for key, (field, texs, renderer) in BINDINGS.items():
            layer = dump["layers"][field]
            bound: list[str] = []
            for t in texs:
                hit = [p for p in textures if (p.startswith(t) if t.endswith("/") else p == t)]
                if not hit:
                    die(f"{key}: the jar ships no texture {t!r}")
                bound += [f"minecraft:{p}" for p in hit]
            models[key] = {
                "layer": field,
                "renderer": renderer,
                "texture_size": [layer["material"]["xTexSize"], layer["material"]["yTexSize"]],
                "textures": sorted(bound),
                "cubes": flatten(layer),
            }
        report = cross_check(jar, models)
    for line in report:
        print(f"  {line}")

    man = dump["mannequin"]
    if man["encode_all_layers"] != "[]":
        die(f"a mannequin hiding nothing encodes as {man['encode_all_layers']}, not []")
    layers = []
    for p in man["parts"]:
        if p["id"] != p["serialized"]:
            die(f"PlayerModelPart {p['constant']}: id {p['id']} and serialized {p['serialized']} differ")
        if p["decoded_alone"] != man["all_layers"] & ~p["mask"]:
            die(f"`{man['field']}: [\"{p['id']}\"]` decodes to {p['decoded_alone']}, not all-but-its-bit")
        layers.append(p["id"])

    doc = {
        "client_jar_sha256": jar_sha256,
        "client_jar_sha1": jar_sha1,
        "client_mappings_sha1": maps["sha1"],
        "instrument": {
            "extractor": "tools/maintenance/extract-model-parts.py",
            "dumper": "tools/maintenance/modelparts/ModelPartDump.java",
            "revision": revision,
        },
        "minecraft": mc,
        "ground_y": num(GROUND_Y),
        "player_eye_height_px": num(float(dump["player_eye_height"]) * 16),
        "mannequin": {"field": man["field"], "layers": layers},
        "models": models,
    }
    text = json.dumps(doc, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    out = REPO / "crates" / "delvec" / "data" / f"model-parts-{mc}.json"
    cubes = sum(len(m["cubes"]) for m in models.values())
    print(f"extract-model-parts: {len(models)} model(s), {cubes} box(es), "
          f"{len(layers)} mannequin layer(s), instrument {revision}")
    if a.check:
        have = out.read_text(encoding="utf-8") if out.exists() else None
        if have is None:
            die(f"{out.relative_to(REPO)} is absent")
        # The instrument revision moves with every commit that touches it; the
        # check compares what was measured.
        mine, theirs = json.loads(text), json.loads(have)
        mine["instrument"].pop("revision")
        theirs["instrument"].pop("revision")
        if mine != theirs:
            die(f"{out.relative_to(REPO)} differs from what this jar derives — regenerate and commit it")
        print(f"extract-model-parts: {out.relative_to(REPO)} is what this jar derives")
        return 0
    out.write_text(text, encoding="utf-8")
    print(f"extract-model-parts: wrote {out.relative_to(REPO)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
