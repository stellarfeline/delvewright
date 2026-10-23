#!/usr/bin/env python3
"""The storybook illustration path runs, on every horizon the engine can build.

    python3 tools/ci/check-whole-map-render.py --prefabs <dir> [--delvec <bin>] [--work <dir>]

## What it asserts, and why the enumeration is the point

`delvec panorama` draws the release illustration from the load world a build
writes, and `delvec cameras` draws the showcase record's cameras, each from
the world its `after` stands in. A check that ran them on whatever build
happened to be lying around would pass on the default horizon alone, so the
population is fixed by the ENGINE rather than by this file: the horizon bases
are read from `delvec schema --stage all`, the authority the coverage gate
enumerates its units from, and every one of them must be exercised. A base
with no gallery point declaring it is a red naming the base; a point whose
arms refuse is a red naming the command and the reason.

Per point, every frame on the CPU backend, small and cheap — this gate judges
what each frame's manifest says, never its pixels:

- `delvec build` compiles the point, and its plan names the base;
- **the panorama frames the built place, and fills the frame with it.** The
  manifest's camera (Minecraft yaw and pitch, vertical `fov`, the frame's own
  aspect) is projected here by an implementation of its own, sharing no code
  with the engine's solve: every corner of the plan's `layout_aabb` must
  project inside the frame, and the rectangle they span must cover at least a
  third of it. The manifest's own `subject.span.fill` must agree with that
  projection within 0.02.
- **the frame loads the ground.** The manifest's chunk rectangle covers the
  landform the horizon declares, not merely the layout.
- **every frame shows something**: the manifest's `frame.featureless` is null.
- **every stated camera is the frame's camera** (`delvec cameras --preview`, on
  every point whose campaign carries a `design/cameras.json`): one manifest per
  camera whose position, yaw, pitch and `fov` are the record's, whose world is
  a world the run wrote, and whose sky's weather is the camera's own `sky`,
  else the `design.json` row's it answers — read here from the two documents.
  Over the domain the weathers drawn are `{clear, rain, thunder}`: the gallery
  owes every weather a frame.

What the sky looks like at each hour is the engine's, judged in
`crates/delvec/tests/review_frame_sky.rs`; this gate holds that each camera
asks for the sky its record states.

## Binding count

Every run states the bases the schema declares, the points found for them, the
builds exercised, the panoramas framed and the cameras judged. A base
unaccounted for is a red; zero panoramas or zero cameras judged is a red. A
gate that drew nothing is vacuous, not a pass.
"""

from __future__ import annotations

import argparse
import contextlib
import json
import math
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
import horizon  # noqa: E402
import parallel  # noqa: E402
from delvec_bin import resolve as resolve_delvec  # noqa: E402
from gallery_domain import GALLERY, materialise, overlays  # noqa: E402

REPO = Path(__file__).resolve().parents[2]

# The share of the frame a panorama's subject must cover.
MIN_FILL = 1.0 / 3.0
# How far the manifest's own fill may sit from this gate's projection.
FILL_SLACK = 0.02
# The panorama's frame here: small, one sample — the manifest is what is judged.
PANORAMA = ["--width", "160", "--height", "90", "--spp", "1", "--backend", "cpu"]
WEATHERS = {"clear", "rain", "thunder"}


def die(msg: str) -> None:
    print(f"error: {msg}", file=sys.stderr)
    raise SystemExit(1)


def declared_bases(delvec: Path) -> list[str]:
    """Every horizon base the ENGINE declares, from its own schema export."""
    r = subprocess.run(
        [str(delvec), "schema", "--stage", "all"], capture_output=True, text=True
    )
    if r.returncode != 0:
        die(f"`delvec schema --stage all` exited {r.returncode}: {r.stderr.strip()}")
    doc = json.loads(r.stdout)
    found: list[str] = []

    def walk(node: object) -> None:
        if isinstance(node, dict):
            one_of = node.get("oneOf")
            if isinstance(one_of, list):
                consts = [e.get("const") for e in one_of if isinstance(e, dict)]
                if consts and all(isinstance(c, str) for c in consts) and "void" in consts:
                    found.extend(consts)
            for v in node.values():
                walk(v)
        elif isinstance(node, list):
            for v in node:
                walk(v)

    walk(doc)
    if not found:
        die(
            "the schema export declares no horizon base enumeration. The population "
            "this gate quantifies over comes from the engine and from nowhere else, "
            "so an empty one is a broken reading, not an empty world."
        )
    return sorted(set(found))


def horizon_of(world_json: Path) -> str | None:
    """The base a stage-1 document declares, or `None` if it declares none."""
    if not world_json.is_file():
        return None
    return horizon.declared_base(json.loads(world_json.read_text()))


def points_by_base() -> dict[str, str | None]:
    """Which gallery point declares each base. Derived, never listed."""
    out: dict[str, str | None] = {}
    primary = horizon_of(GALLERY / "world.json")
    if primary:
        out.setdefault(primary, None)
    for name in overlays():
        base = horizon_of(GALLERY / "overlays" / name / "world.json")
        if base:
            out.setdefault(base, name)
    return out


def run(cmd: list[str]) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, capture_output=True, text=True)


def covers(chunks: list[list[int]], lo: list[int], hi: list[int]) -> list[str]:
    """Chunk columns of `[lo, hi]` that the list is missing."""
    have = {(c[0], c[1]) for c in chunks}
    missing = []
    for cx in range(lo[0] // 16, hi[0] // 16 + 1):
        for cz in range(lo[2] // 16, hi[2] // 16 + 1):
            if (cx, cz) not in have:
                missing.append(f"[{cx}, {cz}]")
    return missing


def camera_matches(man: dict, cam: dict) -> list[str]:
    """What differs between a stated camera and the frame drawn for it."""
    got = man.get("camera") or {}
    bad = []
    for k in ("pos", "yaw", "pitch", "fov"):
        a, b = got.get(k), cam.get(k)
        if isinstance(a, list) and isinstance(b, list):
            same = len(a) == len(b) and all(abs(float(x) - float(y)) < 1e-6 for x, y in zip(a, b))
        else:
            same = a is not None and b is not None and abs(float(a) - float(b)) < 1e-6
        if not same:
            bad.append(f"`{k}` is {a} in the frame and {b} in the record")
    return bad


def projection(man: dict, lo: list[int], hi: list[int]) -> tuple[bool, float, float]:
    """Project the box `lo..=hi` through the manifest's camera, in Minecraft's
    convention: yaw 0 looks south (+Z), 90 west (-X); pitch positive looks
    down; `fov` the vertical field of view; the horizontal one from the frame's
    aspect. Returns whether every corner is inside the frame, and the share of
    its width and height the corners span (clipped to the frame)."""
    cam = man["camera"]
    yaw, pitch = math.radians(cam["yaw"]), math.radians(cam["pitch"])
    f = (-math.sin(yaw) * math.cos(pitch), -math.sin(pitch), math.cos(yaw) * math.cos(pitch))
    r = (-math.cos(yaw), 0.0, -math.sin(yaw))
    u = (
        r[1] * f[2] - r[2] * f[1],
        r[2] * f[0] - r[0] * f[2],
        r[0] * f[1] - r[1] * f[0],
    )
    w, h = man["image"]["width"], man["image"]["height"]
    tv = math.tan(math.radians(cam["fov"]) / 2.0)
    th = tv * w / h
    eye = cam["pos"]
    xs, ys, inside = [], [], True
    for cx in (lo[0], hi[0] + 1):
        for cy in (lo[1], hi[1] + 1):
            for cz in (lo[2], hi[2] + 1):
                d = (cx - eye[0], cy - eye[1], cz - eye[2])
                z = sum(a * b for a, b in zip(d, f))
                if z <= 0:
                    inside = False
                    continue
                x = sum(a * b for a, b in zip(d, r)) / (z * th)
                y = sum(a * b for a, b in zip(d, u)) / (z * tv)
                if abs(x) > 1 or abs(y) > 1:
                    inside = False
                xs.append(max(-1.0, min(1.0, x)))
                ys.append(max(-1.0, min(1.0, y)))
    if not xs:
        return False, 0.0, 0.0
    return inside, (max(xs) - min(xs)) / 2.0, (max(ys) - min(ys)) / 2.0


def stated_sky(cam: dict, design: dict) -> dict | None:
    """The sky a camera is taken under: its own `sky`, else its row's."""
    if isinstance(cam.get("sky"), dict):
        return cam["sky"]
    for row in (design.get("content") or {}).get("references") or []:
        if row.get("name") == cam.get("answers"):
            return {"time": row.get("time"), "weather": row.get("weather")}
    return None


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--delvec", help="the `delvec` this tool runs. Default: `target/release/delvec` then `target/debug/delvec` in this tree — resolved, NAMED on stderr, and refused when it is older than the compiler sources it was built from (`tools/lib/delvec_bin.py`).")
    ap.add_argument("--prefabs", required=True)
    ap.add_argument("--work", help="scratch dir (default: a temp dir, removed)")
    args = ap.parse_args()

    delvec = resolve_delvec(args.delvec, repo=REPO, caller="check-whole-map-render")

    bases = declared_bases(delvec)
    found = points_by_base()
    unaccounted = [b for b in bases if b not in found]
    if unaccounted:
        die(
            "the engine declares horizon base(s) no gallery point uses, so the "
            "whole-map illustration path is unproven for them:\n"
            + "\n".join(f"  {b}" for b in unaccounted)
            + "\nGive each one a gallery point declaring it, in the change that "
            "added the base."
        )

    tmp = tempfile.TemporaryDirectory() if args.work is None else None
    work = Path(args.work) if args.work else Path(tmp.name)
    work.mkdir(parents=True, exist_ok=True)

    findings: list[str] = []
    panoramas_framed = 0
    builds = 0
    cameras_judged = 0
    records = 0
    weathers_seen: dict[str, int] = {}

    def engine_runs(base: str) -> dict:
        """Every engine run one base needs, in directories named for its point."""
        point = found[base]
        label = point or "primary"
        src, out = work / f"src-{label}", work / f"out-{label}"
        materialise(src, None if point is None else GALLERY / "overlays" / point)
        got: dict = {"src": src, "out": out, "label": label}
        got["build"] = run([str(delvec), "build", str(src), "-o", str(out), "--prefabs", args.prefabs])
        if got["build"].returncode != 0:
            return got
        if (src / "design" / "cameras.json").is_file():
            got["cameras"] = run(
                [str(delvec), "--prefabs", args.prefabs, "cameras", str(out),
                 "--campaign", str(src), "-o", str(work / f"cameras-{label}"),
                 "--preview", "--backend", "cpu"]
            )
        got["panorama"] = run(
            [str(delvec), "panorama", str(out), "-o", str(work / f"panorama-{label}"), *PANORAMA]
        )
        return got

    with contextlib.closing(parallel.ordered(engine_runs, bases)) as ran:
        for base, outcome in zip(bases, ran):
            got = outcome.get()
            src, out, label = got["src"], got["out"], got["label"]
            r = got["build"]
            if r.returncode != 0:
                findings.append(
                    f"{base} ({label}): `delvec build` exited {r.returncode} — the point "
                    f"this base is proven on does not compile: {r.stderr.strip().splitlines()[-1:]}"
                )
                continue
            builds += 1
            plan = json.loads((out / "render-plan.json").read_text())
            h = plan.get("horizon")
            kind = h.get("kind") if h else "void"
            if kind != base:
                findings.append(
                    f"{base} ({label}): the build emitted horizon kind `{kind}` — the "
                    "point does not exercise the base it was selected for"
                )
            extent = (h or {}).get("extent")

            record = src / "design" / "cameras.json"
            if record.is_file():
                records += 1
                sheet = json.loads(record.read_text())
                design = json.loads((src / "design.json").read_text()) if (src / "design.json").is_file() else {}
                dest = work / f"cameras-{label}"
                r = got["cameras"]
                if r.returncode != 0:
                    findings.append(
                        f"{base} ({label}): `delvec cameras` exited {r.returncode} — "
                        f"{(r.stderr or r.stdout).strip().splitlines()[-1] if (r.stderr or r.stdout).strip() else 'no reason given'}"
                    )
                elif not sheet.get("cameras"):
                    findings.append(f"{base} ({label}): {record} states no camera")
                else:
                    for cam in sheet["cameras"]:
                        man_file = dest / f"{sheet['campaign_id']}_camera_{cam['name']}_preview.manifest.json"
                        if not man_file.is_file():
                            findings.append(
                                f"{base} ({label}): camera `{cam['name']}` has no frame manifest {man_file.name}"
                            )
                            continue
                        cameras_judged += 1
                        man = json.loads(man_file.read_text())
                        for why in camera_matches(man, cam):
                            findings.append(f"{base} ({label}) camera `{cam['name']}`: {why}")
                        world = (man.get("world") or {}).get("path")
                        if not world or not (dest / world / "level.dat").is_file():
                            findings.append(
                                f"{base} ({label}) camera `{cam['name']}`: its world `{world}` is "
                                "no world this run wrote"
                            )
                        want = stated_sky(cam, design)
                        got_w = (man.get("sky") or {}).get("weather")
                        if want is None or want.get("weather") != got_w:
                            findings.append(
                                f"{base} ({label}) camera `{cam['name']}`: drawn under weather "
                                f"`{got_w}`, its record states {want}"
                            )
                        else:
                            weathers_seen[got_w] = weathers_seen.get(got_w, 0) + 1
                        if (man.get("frame") or {}).get("featureless") is not None:
                            findings.append(
                                f"{base} ({label}) camera `{cam['name']}`: the frame is featureless"
                            )

            dest = work / f"panorama-{label}"
            r = got["panorama"]
            if r.returncode != 0:
                findings.append(
                    f"{base} ({label}): `delvec panorama` exited {r.returncode} — "
                    f"{(r.stderr or r.stdout).strip().splitlines()[-1] if (r.stderr or r.stdout).strip() else 'no reason given'}"
                )
                continue
            mans = sorted(dest.glob("*.manifest.json"))
            if len(mans) != 1:
                findings.append(
                    f"{base} ({label}): `delvec panorama` exited 0 and wrote {len(mans)} manifest(s). "
                    "Exit 0 is not a picture."
                )
                continue
            man = json.loads(mans[0].read_text())
            panoramas_framed += 1
            box = plan["layout_aabb"]
            inside, w, hh = projection(man, box["min"], box["max"])
            if not inside or w * hh < MIN_FILL:
                findings.append(
                    f"{base} ({label}) {mans[0].name}: the placed areas "
                    f"{box['min']}..{box['max']} are {'inside' if inside else 'NOT inside'} the "
                    f"frame and cover {w * hh:.2f} of it ({w:.2f} x {hh:.2f}); the floor is "
                    f"{MIN_FILL:.2f}. A panorama frames the built place, not the ground around it."
                )
            stated = ((man.get("subject") or {}).get("span") or {}).get("fill")
            if stated is None or abs(stated - w * hh) > FILL_SLACK:
                findings.append(
                    f"{base} ({label}) {mans[0].name}: the manifest states a fill of {stated}, "
                    f"this gate projects {w * hh:.3f}: two readings of one camera disagree"
                )
            if (man.get("frame") or {}).get("featureless") is not None:
                findings.append(f"{base} ({label}) {mans[0].name}: the frame is featureless")
            if extent:
                ch = man.get("chunks") or {}
                lo_c, hi_c = ch.get("min"), ch.get("max")
                chunks = (
                    [[x, z] for x in range(lo_c[0], hi_c[0] + 1) for z in range(lo_c[1], hi_c[1] + 1)]
                    if lo_c and hi_c
                    else []
                )
                gap = covers(chunks, extent["min"], extent["max"])
                if gap:
                    findings.append(
                        f"{base} ({label}) {mans[0].name}: the frame loads none of {len(gap)} chunk "
                        f"column(s) of the landform the horizon declares (first: {', '.join(gap[:4])})"
                    )

    print(
        f"whole-map render: {len(bases)} horizon base(s) declared "
        f"({', '.join(bases)}), {len(found)} point(s) found, {builds} build(s) "
        f"exercised, {panoramas_framed} of {builds} panorama(s) framed, {cameras_judged} stated "
        f"camera(s) judged over {records} camera record(s), weathers drawn "
        f"{dict(sorted(weathers_seen.items()))}, {len(findings)} finding(s)."
    )
    if builds and cameras_judged == 0:
        die(
            "ZERO stated cameras were judged: no point of the domain carries a "
            "`design/cameras.json`, or none of its cameras reached a frame. The "
            "showcase camera path is proven on the gallery's record or not at all."
        )
    if cameras_judged and set(weathers_seen) != WEATHERS:
        findings.append(
            f"the domain's cameras are drawn under weathers {sorted(weathers_seen)}; the "
            "gallery owes every weather a frame (spec-0079 §7)"
        )
    if builds and panoramas_framed != builds:
        die(
            f"{panoramas_framed} panorama frame(s) judged over {builds} build(s): "
            "every build owes one, so the framing assertion did not bind to them all."
        )
    if panoramas_framed == 0:
        die(
            "ZERO panoramas were judged, so every assertion above examined nothing. A "
            "gate that drew nothing is vacuous, not a pass."
        )
    if findings:
        print("\nfindings:", file=sys.stderr)
        for f in findings:
            print(f"  {f}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
