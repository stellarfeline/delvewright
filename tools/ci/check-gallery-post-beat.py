#!/usr/bin/env python3
"""The gallery's showcase cameras stand where its record says (spec-0089 §9).

    python3 tools/ci/check-gallery-post-beat.py --delvec <delvec> --prefabs <dir> \
        --build-out <gallery build> --work <dir>

Runs `delvec cameras` over the gallery's own build — a cargo test cannot build
the gallery, whose pieces the gallery job generates — and asserts, from the
binding lines the command prints and the worlds it writes:

1. exactly two worlds are written, `at-load` and `after-obj-clear-the-muster`,
   and the camera `east-bay-after-the-muster` stands in the second with a
   positive `cells moved from load`;
2. removing that camera's `after` moves its scene's `world.path` to `at-load`,
   the after-world's hash out of the run, and its count to 0;
3. moving it to `obj/take-the-bone` moves the world's hash and the count.

Each perturbation is made on a scratch copy of the gallery, never on the tree.
Every count is printed with its denominator; a perturbation whose run moves
nothing reds.
"""

from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import gallery_domain  # noqa: E402

REPO = Path(__file__).resolve().parents[2]
GALLERY = REPO / "gallery"
CAMERA = "east-bay-after-the-muster"
STEP = "obj/clear-the-muster"
KEY = "after-obj-clear-the-muster"

WORLD = re.compile(r"^world: (\S+) (\d+) chunk\(s\), (\d+) cell\(s\), sha256 ([0-9a-f]{64}) -> (.+)$", re.M)
AFTER = re.compile(rf"^after: {re.escape(CAMERA)} (?:at load|after (\S+) .*?: (\d+) cells moved from load)", re.M)


def fail(msg: str) -> None:
    print(f"check-gallery-post-beat: FAIL — {msg}", file=sys.stderr)
    raise SystemExit(1)


def run(delvec: Path, prefabs: Path, build: Path, campaign: Path, out: Path) -> dict:
    if out.exists():
        shutil.rmtree(out)
    r = subprocess.run(
        [str(delvec), "--prefabs", str(prefabs), "cameras", str(build), "--campaign", str(campaign), "-o", str(out)],
        capture_output=True,
        text=True,
    )
    if r.returncode != 0:
        fail(f"`delvec cameras` exited {r.returncode} over {campaign}:\n{r.stdout}{r.stderr}")
    worlds = {m.group(1): {"chunks": int(m.group(2)), "cells": int(m.group(3)), "sha256": m.group(4)} for m in WORLD.finditer(r.stderr)}
    m = AFTER.search(r.stderr)
    if not m:
        fail(f"`delvec cameras` printed no `after:` line for `{CAMERA}`:\n{r.stderr}")
    scene = json.loads((out / f"gallery_camera_{CAMERA}.json").read_text())
    return {
        "worlds": worlds,
        "step": m.group(1),
        "moved": int(m.group(2) or 0),
        "world_path": scene["world"]["path"],
        "summary": next((l for l in r.stderr.splitlines() if l.startswith("configurations:")), ""),
    }


def perturbed(work: Path, tag: str, edit) -> Path:
    # What a build point IS is `gallery_domain`'s answer: the primary,
    # materialised, then the one edit this perturbation makes.
    dest = work / f"gallery-{tag}"
    if gallery_domain.materialise(dest) == 0:
        fail(f"the primary materialised zero files into {dest}")
    record = dest / "design" / "cameras.json"
    doc = json.loads(record.read_text())
    cams = [c for c in doc["cameras"] if c["name"] == CAMERA]
    if len(cams) != 1:
        fail(f"gallery/design/cameras.json carries {len(cams)} camera(s) named `{CAMERA}`, not 1")
    edit(cams[0])
    record.write_text(json.dumps(doc, indent=2) + "\n")
    return dest


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--delvec", type=Path, required=True)
    ap.add_argument("--prefabs", type=Path, required=True)
    ap.add_argument("--build-out", type=Path, required=True)
    ap.add_argument("--work", type=Path, required=True)
    a = ap.parse_args(argv[1:])
    a.work.mkdir(parents=True, exist_ok=True)

    primary = run(a.delvec, a.prefabs, a.build_out, GALLERY, a.work / "primary")
    print(f"primary: {primary['summary']}")
    for key, w in sorted(primary["worlds"].items()):
        print(f"  world {key}: {w['chunks']} chunk(s), {w['cells']} cell(s), sha256 {w['sha256']}")
    if sorted(primary["worlds"]) != sorted(["at-load", KEY]):
        fail(f"the gallery writes worlds {sorted(primary['worlds'])}, not ['at-load', '{KEY}']")
    if primary["step"] != STEP or primary["moved"] <= 0:
        fail(f"`{CAMERA}` stands after {primary['step']} with {primary['moved']} cell(s) moved, not after {STEP} with a positive count")
    if not primary["world_path"].endswith(f"worlds/{KEY}"):
        fail(f"`{CAMERA}`'s scene loads {primary['world_path']}, not worlds/{KEY}")
    print(f"  `{CAMERA}` after {STEP}: {primary['moved']} cell(s) moved from load")

    gone = run(a.delvec, a.prefabs, a.build_out, perturbed(a.work, "no-after", lambda c: c.pop("after")), a.work / "no-after")
    print(f"no `after`: {gone['summary']}; `{CAMERA}` loads {Path(gone['world_path']).name}, {gone['moved']} cell(s) moved")
    if not gone["world_path"].endswith("worlds/at-load") or gone["moved"] != 0 or KEY in gone["worlds"]:
        fail("removing the camera's `after` did not move its scene to `at-load` and its count to 0")

    bone = run(
        a.delvec,
        a.prefabs,
        a.build_out,
        perturbed(a.work, "take-the-bone", lambda c: c.__setitem__("after", {"step": "obj/take-the-bone"})),
        a.work / "take-the-bone",
    )
    moved_key = "after-obj-take-the-bone"
    print(
        f"after `obj/take-the-bone`: {bone['summary']}; {bone['moved']} cell(s) moved, sha256 "
        f"{bone['worlds'].get(moved_key, {}).get('sha256')}"
    )
    if moved_key not in bone["worlds"]:
        fail(f"moving the step wrote no `{moved_key}` world")
    if bone["worlds"][moved_key]["sha256"] == primary["worlds"][KEY]["sha256"] or bone["moved"] == primary["moved"]:
        fail("moving the camera's step moved neither the world's hash nor its count")
    print("gallery post-beat: 3 run(s), 2 perturbation(s), each moved the scene's world and its count")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
