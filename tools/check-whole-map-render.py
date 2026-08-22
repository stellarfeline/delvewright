#!/usr/bin/env python3
"""The whole-map illustration path runs, on every horizon the engine can build.

## The gap this closes

`delvec panorama` and `delvec scene` read the compiler's own `render-plan.json`
and turn it into Chunky scenes — the release illustration and the whole-scene
review renders. **Nothing in this repository invoked either of them.**
`tools/check-gallery-render.py` runs `snapshot`, and only on the primary
gallery.

What that cost, measured rather than supposed: the render layer's `Horizon` enum
carried ONE variant while the compiler had been emitting a second for as long as
the base existed, and `#[serde(tag = "kind")]` has no fallback — so one unknown
value failed the whole document and **both commands refused every valley
campaign outright** with

    DW0721 [error] parse render-plan.json: unknown variant `valley`, expected `ocean`

A producer and a consumer of one document, and nothing compared them. That is
the UNRUN vacuity mode in the release path: a gate whose obligation lived in
nobody's head at all.

## What it asserts, and why the enumeration is the point

A check that ran the arms on whatever build happened to be lying around would
have passed for years — the primary gallery declares the default horizon, and
the default horizon is exactly the one that worked. So the population is fixed
by the ENGINE rather than by this file: the horizon bases are read from
`delvec schema --stage all`, which is the same single authority the coverage
gate enumerates its units from, and every one of them must be exercised.

A base with no gallery point declaring it is a red naming the base. A base whose
point exists and whose arms refuse is a red naming the command and the reason.
There is no third state and no list here to go stale — adding a base to the
engine reds this file until something renders it.

Per point, for each of `scene` and `panorama`:

- the command exits 0, and its stderr is reported verbatim when it does not;
- it emits at least one scene file;
- **the frame contains the ground.** A scene's `chunkList` must cover the
  landform the horizon declares, not merely the layout. The panorama camera and
  the chunk list are both solved from the layout AABB, which is the union of the
  placed AREAS — so on a horizon that BUILDS terrain the whole-map illustration
  framed a box inside a landform several times its size and loaded none of it.
  That defect renders successfully and produces a picture, which is why it needs
  an assertion rather than an eye.

## Binding count

Every run states the bases the schema declares, the points found for them, the
builds exercised, and the scene files judged. A base unaccounted for is a red;
zero scenes judged is a red. A gate that rendered nothing is vacuous, not a
pass.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from gallery_domain import GALLERY, materialise, overlays  # noqa: E402

REPO = Path(__file__).resolve().parent.parent


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
    doc = json.loads(world_json.read_text())
    h = doc.get("content", doc).get("horizon")
    if h is None:
        return None
    return h if isinstance(h, str) else h.get("base")


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


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--delvec", default=str(REPO / "target/release/delvec"))
    ap.add_argument("--prefabs", required=True)
    ap.add_argument("--work", help="scratch dir (default: a temp dir, removed)")
    args = ap.parse_args()

    delvec = Path(args.delvec)
    if not delvec.is_file():
        die(f"no delvec at `{delvec}`")

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
    scenes_judged = 0
    builds = 0

    for base in bases:
        point = found[base]
        label = point or "primary"
        src, out = work / f"src-{label}", work / f"out-{label}"
        materialise(src, None if point is None else GALLERY / "overlays" / point)
        r = run([str(delvec), "build", str(src), "-o", str(out), "--prefabs", args.prefabs])
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

        for arm in ("scene", "panorama"):
            dest = work / f"{arm}-{label}"
            cmd = [str(delvec)]
            if arm == "scene":
                cmd += ["--prefabs", args.prefabs]
            cmd += [arm, str(out), "-o", str(dest)]
            r = run(cmd)
            if r.returncode != 0:
                findings.append(
                    f"{base} ({label}): `delvec {arm}` exited {r.returncode} — "
                    f"{(r.stderr or r.stdout).strip().splitlines()[-1] if (r.stderr or r.stdout).strip() else 'no reason given'}"
                )
                continue
            files = sorted(dest.glob("*.json"))
            if not files:
                findings.append(
                    f"{base} ({label}): `delvec {arm}` exited 0 and emitted no scene "
                    "file. Exit 0 is not a picture."
                )
                continue
            for f in files:
                scenes_judged += 1
                doc = json.loads(f.read_text())
                chunks = doc.get("chunkList") or []
                if not chunks:
                    findings.append(f"{base} ({label}) {f.name}: empty chunkList")
                    continue
                if extent:
                    gap = covers(chunks, extent["min"], extent["max"])
                    if gap:
                        findings.append(
                            f"{base} ({label}) {f.name}: the scene loads none of "
                            f"{len(gap)} chunk column(s) of the landform the horizon "
                            f"declares (first: {', '.join(gap[:4])}). The frame would "
                            "show the map standing in a void it is surrounded by "
                            "ground in."
                        )

    print(
        f"whole-map render: {len(bases)} horizon base(s) declared "
        f"({', '.join(bases)}), {len(found)} point(s) found, {builds} build(s) "
        f"exercised, {scenes_judged} scene file(s) judged, {len(findings)} finding(s)."
    )
    if scenes_judged == 0:
        die(
            "ZERO scene files were judged, so every assertion above examined "
            "nothing. A gate that rendered nothing is vacuous, not a pass."
        )
    if findings:
        print("\nfindings:", file=sys.stderr)
        for f in findings:
            print(f"  {f}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
