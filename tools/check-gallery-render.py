#!/usr/bin/env python3
"""Every declared view of the gallery is produced, and shows something (spec-0039 §7).

## What this is for

Some surfaces are only confirmable by a picture, and until this ran, CI rendered
nothing. The machine half is what belongs in CI — a picture nobody looks at
still has facts about it that a machine can check, and they are exactly the ones
a reviewer cannot check by eye across thirty-five frames:

- **the set is never silently short** (`DW0721` semantics). The gallery commits
  the view set it declares; the build emits one; a view that vanishes between
  them is a red naming it. Without this, a shot dropping out of the plan looks
  like a shorter directory listing and nothing else.
- **every declared view is produced or refused loudly.** A view the renderer
  cannot draw must say so; it may not simply not appear.
- **every produced frame shows something** — `detect::is_featureless`, the
  engine's own answer, computed by the arm that drew the frame and read back
  from its manifest. A render that succeeds, writes a file and is a rectangle of
  flat background looks like one more shot taken to a directory listing, to a
  contact sheet, and to a reviewer skimming.
- **every view manifest states a non-zero binding** (`DW0726` discipline): a
  shot that framed no target is a camera aimed at nothing, and the count is the
  only thing that separates it from a good one.

**No pixel is committed or compared.** Renderer output is not covered by
ADR-0006's byte guarantee across drivers, so the manifests are the machine truth
and the frames are for eyes. The frames are uploaded as a CI artifact for the
human half of the review and are never diffed.

## The arms this uses

The CPU arms only (`delvec snapshot`), which run anywhere — no GPU, no client
jar, no `nucleation`. Whether CI runners can drive the GPU arms through a
software adapter is a separate question and a separate job; either answer moves
where frames come from, never what the engine can do.

## Binding count

Every run states views declared, produced, refused and judged. Declaring zero
views is a red, and so is judging zero frames: a gate that looked at no pictures
is vacuous, not a pass.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
GALLERY = REPO / "gallery"
PLAN = GALLERY / "render-plan.json"


def die(msg: str) -> None:
    print(f"error: {msg}", file=sys.stderr)
    raise SystemExit(1)


def safe(shot_id: str) -> str:
    return shot_id.replace("/", "_")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--delvec", default=str(REPO / "target/release/delvec"))
    ap.add_argument("--prefabs", required=True)
    ap.add_argument("--build-out", required=True, help="a `delvec build` output tree")
    ap.add_argument("--frames", required=True, help="where to write the frames (never committed)")
    ap.add_argument("--write", action="store_true", help="regenerate the committed view set")
    args = ap.parse_args()

    delvec, out = Path(args.delvec), Path(args.build_out)
    frames = Path(args.frames)
    if not delvec.is_file():
        die(f"no delvec at `{delvec}`")

    emitted_path = out / "render-plan.json"
    if not emitted_path.is_file():
        die(f"`{emitted_path}` is missing — the build emitted no render plan")
    emitted = json.loads(emitted_path.read_text())
    declared = [
        {"id": s["id"], "kind": s.get("kind", ""), "area": s.get("area", "")}
        for s in emitted.get("shots", [])
    ]
    declared.sort(key=lambda s: s["id"])

    if args.write:
        PLAN.write_text(
            json.dumps(
                {
                    "note": (
                        "The view set the gallery declares. Committed so that a shot "
                        "VANISHING is a red naming it rather than a shorter directory "
                        "listing. Cameras are not committed: they are derived from the "
                        "campaign and belong to the build, and duplicating them here "
                        "would be a second authority on where a camera stands."
                    ),
                    "views": declared,
                },
                indent=2,
                sort_keys=True,
            )
            + "\n"
        )
        print(f"wrote {PLAN}: {len(declared)} declared view(s)")
        return 0

    if not PLAN.is_file():
        die(f"`{PLAN}` is missing — run this with `--write`")
    committed = json.loads(PLAN.read_text())["views"]
    if not committed:
        die(
            "the committed view set is EMPTY. Every assertion below is universally "
            "quantified over it, so a green here would mean nothing."
        )

    c_ids = {v["id"] for v in committed}
    e_ids = {v["id"] for v in declared}
    if c_ids - e_ids:
        die(
            "the build no longer emits view(s) the gallery declares — a set is never "
            "silently short (DW0721):\n"
            + "\n".join(f"  {i}" for i in sorted(c_ids - e_ids))
            + "\nIf the view really is gone, regenerate the set with `--write` in the "
            "change that removed it."
        )
    if e_ids - c_ids:
        die(
            "the build emits view(s) the gallery does not declare:\n"
            + "\n".join(f"  {i}" for i in sorted(e_ids - c_ids))
            + "\nRegenerate the committed set with `--write` in the change that added "
            "them, so the addition is reviewed rather than absorbed."
        )

    # ------------------------------------------------------------ produce ----
    frames.mkdir(parents=True, exist_ok=True)
    produced, refused, findings = 0, [], []
    for v in sorted(committed, key=lambda v: v["id"]):
        png = frames / f"{safe(v['id'])}.png"
        r = subprocess.run(
            [
                str(delvec), "--prefabs", args.prefabs, "snapshot", str(GALLERY),
                "--shot", v["id"], "-o", str(png),
            ],
            capture_output=True,
            text=True,
        )
        if r.returncode != 0:
            # A refusal is legitimate — it is the loud half of "produced or
            # refused". What is not legitimate is silence, so it is recorded by
            # name with the renderer's own reason.
            refused.append((v["id"], (r.stderr or r.stdout).strip().splitlines()[-1:] or [""]))
            continue
        produced += 1
        man = png.with_suffix("").with_suffix(".manifest.json")
        if not man.is_file():
            man = Path(str(png)[: -len(".png")] + ".manifest.json")
        if not man.is_file():
            findings.append(f"{v['id']}: produced a frame and no manifest — nothing states what it shows")
            continue
        doc = json.loads(man.read_text())
        frame = doc.get("frame")
        if frame is None:
            die(
                f"{man} carries no `frame` block. The producer states the verdict; a "
                "consumer computing its own would be a second authority on it."
            )
        if frame.get("featureless") is not None:
            findings.append(
                f"{v['id']}: the frame is FEATURELESS "
                f"({frame['featureless']['distinct_colors']} distinct colours) — it "
                "shows no scene at all, and a blank rectangle must not count as a shot "
                "of a room"
            )
        if frame.get("targets_in_frame", 0) == 0:
            findings.append(
                f"{v['id']}: the view manifest binds ZERO targets — the camera framed "
                "nothing the campaign declares, so nothing about this shot is checked "
                "by having taken it (DW0726 discipline)"
            )

    print(
        f"gallery render: {len(committed)} view(s) declared, {produced} produced, "
        f"{len(refused)} refused, {len(findings)} finding(s)."
    )
    for rid, why in refused:
        print(f"  refused: {rid} — {why[0] if why else 'no reason given'}")

    if produced == 0:
        die(
            "ZERO frames were produced, so every per-frame assertion above examined "
            "nothing. A gate that judged no pictures is vacuous, not a pass."
        )
    # Everything that is wrong, before anything exits. A gate that stops at its
    # first finding reports one finding and hides the rest — which does not fake
    # a pass, it fakes coverage of the failures, and this tool had that defect
    # while its own CI job was being fixed for it.
    if findings:
        print("\nfindings:", file=sys.stderr)
        for f in findings:
            print(f"  {f}", file=sys.stderr)
    if refused:
        die(
            f"{len(refused)} declared view(s) could not be produced. A view is produced "
            "or refused LOUDLY, and a refusal in CI is the loud part arriving — fix the "
            "shot or remove it from the declared set in the same change."
        )
    return 1 if findings else 0


if __name__ == "__main__":
    raise SystemExit(main())
