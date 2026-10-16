#!/usr/bin/env python3
"""The gallery's review frames show what its scene holds (spec-0112 §15).

    python3 tools/ci/check-gallery-review-frames.py --delvec <delvec> --prefabs <dir> \\
        --build-out <gallery build> --work <dir>

The gallery's element for spec-0112, bound by perturbation: each case is the
primary, materialised on a scratch copy (`tools/ci/gallery_domain.py`), with
one declared edit, built, and compared with the unperturbed build on the
facts the review record (`validation/review-frames.json`) and the plan state.
A perturbation whose build moves nothing it is declared to move reds.

1. **The muster's fill moves the world it lays.** The `fill-region` block in
   `obj/clear-the-muster`'s bundle, changed, moves the sha256 of the world the
   critical path arrives under after that step.
2. **The curator's mark moves what a frame shows.** Her `offset` moved one
   cell moves the cell of her body in every frame that shows her.
3. **A lantern lights the load world.** One lantern of the hall's template
   made a chain (taken out to air, the relight siting puts a lantern back in
   the dark cell it leaves) moves the load world's light sha256.
4. **The committed view set holds the cutscene frames.** The muster's
   cutscene with its ninth shot deleted builds a plan one `cutscene/…` view
   short, and `tools/ci/check-gallery-render.py` reds on the committed set
   naming it.
5. **A cut sets the sky of every later leg.** A `set-time` to sunrise added
   to the muster's bundle moves the sky of the POV shots that stand after it.

Every case prints what it compared with its denominator.
"""

from __future__ import annotations

import argparse
import gzip
import json
import shutil
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
import anvil  # noqa: E402
import gallery_domain  # noqa: E402
from delvec_bin import resolve as resolve_delvec  # noqa: E402

REPO = Path(__file__).resolve().parents[2]
STEP = "obj/clear-the-muster"
AFTER = "after-obj-clear-the-muster"
CURATOR = "npc/curator"
CURATOR_TAG = "dw_npc_curator"


def fail(msg: str) -> None:
    print(f"check-gallery-review-frames: FAIL — {msg}", file=sys.stderr)
    raise SystemExit(1)


def build(delvec: Path, campaign: Path, prefabs: Path, out: Path) -> dict:
    if out.exists():
        shutil.rmtree(out)
    r = subprocess.run(
        [str(delvec), "build", str(campaign), "-o", str(out), "--prefabs", str(prefabs)],
        capture_output=True,
        text=True,
    )
    if r.returncode != 0:
        fail(f"`delvec build` exited {r.returncode} over {campaign}:\n{r.stderr[-3000:]}")
    return {
        "out": out,
        "record": json.loads((out / "validation" / "review-frames.json").read_text()),
        "plan": json.loads((out / "render-plan.json").read_text()),
    }


def point(work: Path, tag: str) -> Path:
    dest = work / f"gallery-{tag}"
    if gallery_domain.materialise(dest) == 0:
        fail(f"the primary materialised zero files into {dest}")
    return dest


def edit(path: Path, fn) -> None:
    doc = json.loads(path.read_text())
    fn(doc)
    path.write_text(json.dumps(doc, indent=2) + "\n")


def muster_bundle(doc: dict) -> list:
    for q in doc["content"]["quests"]:
        b = (q.get("on_objective_complete") or {}).get(STEP)
        if b is not None:
            return b
    fail(f"no quest carries a bundle for `{STEP}`")
    return []


def worlds(rec: dict) -> dict:
    return {w["key"]: w for w in rec["worlds"]}


def curator_cells(rec: dict) -> set:
    cells = set()
    for s in rec["shots"]:
        for t in s.get("shows") or []:
            if t.get("kind") == "body" and str(t.get("id", "")).startswith(CURATOR_TAG + "@"):
                cells.add(tuple(t.get("pos") or ()))
    return cells


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--delvec", help="the `delvec` this tool runs (`tools/lib/delvec_bin.py` resolves the default)")
    ap.add_argument("--prefabs", type=Path, required=True)
    ap.add_argument("--build-out", type=Path, required=True, help="the unperturbed gallery build")
    ap.add_argument("--work", type=Path, required=True)
    a = ap.parse_args(argv[1:])
    delvec = resolve_delvec(a.delvec, repo=REPO, caller="check-gallery-review-frames")
    a.work.mkdir(parents=True, exist_ok=True)
    base = {
        "out": a.build_out,
        "record": json.loads((a.build_out / "validation" / "review-frames.json").read_text()),
        "plan": json.loads((a.build_out / "render-plan.json").read_text()),
    }
    bw = worlds(base["record"])
    if AFTER not in bw:
        fail(f"the gallery's review record writes no `{AFTER}` world: {sorted(bw)}")
    passed = 0

    # 1. The muster's fill block.
    p = point(a.work, "fill")
    fills = []

    def change_fill(doc):
        for e in muster_bundle(doc):
            if e.get("type") == "fill-region":
                fills.append(e["block"])
                e["block"] = "minecraft:mossy_cobblestone"

    edit(p / "quests.json", change_fill)
    if not fills:
        fail(f"`{STEP}`'s bundle carries no `fill-region`")
    got = build(delvec, p, a.prefabs, a.work / "out-fill")
    gw = worlds(got["record"])
    moved = AFTER in gw and gw[AFTER]["sha256"] != bw[AFTER]["sha256"]
    print(
        f"1. fill: {len(fills)} fill-region block(s) of `{STEP}` {fills} -> mossy_cobblestone; "
        f"world `{AFTER}` sha256 {bw[AFTER]['sha256'][:12]}… -> {gw.get(AFTER, {}).get('sha256', 'absent')[:12]}…"
    )
    if not moved:
        fail("changing the muster's fill block did not move the world it lays")
    passed += 1

    # 2. The curator's mark.
    p = point(a.work, "curator")

    def move_curator(doc):
        for n in doc["content"]["npcs"]:
            if n["id"] == CURATOR:
                off = list(n.get("offset") or [0, 0, 0])
                off[0] += 1
                n["offset"] = off
                return
        fail(f"npcs.json declares no `{CURATOR}`")

    edit(p / "npcs.json", move_curator)

    def move_cast(doc):
        # Every quest's cast row declares her where she stands.
        for q in doc["content"]["quests"]:
            row = (q.get("cast") or {}).get(CURATOR)
            if isinstance(row, dict) and row.get("at") == "anchor/lectern":
                row["at"] = {"anchor": "anchor/lectern", "offset": [1, 0, 0]}

    edit(p / "quests.json", move_cast)
    got = build(delvec, p, a.prefabs, a.work / "out-curator")
    before, after = curator_cells(base["record"]), curator_cells(got["record"])
    print(
        f"2. curator: her body shown at {sorted(before)} over {len(base['record']['shots'])} shot(s); "
        f"moved one cell east, shown at {sorted(after)}"
    )
    if not before or not after or before == after:
        fail("moving the curator's mark one cell did not move the cell her body is shown at")
    passed += 1

    # 3. A lantern of the hall's template.
    lib = a.work / "prefabs-lantern"
    if lib.exists():
        shutil.rmtree(lib)
    shutil.copytree(a.prefabs, lib)
    nbt = lib / "gallery-hall.nbt"
    name, root = anvil.nbt_read(gzip.decompress(nbt.read_bytes()))
    palette = root.value["palette"].value
    lantern = [i for i, p_ in enumerate(palette) if p_.value["Name"].value == "minecraft:lantern"]
    blocks = root.value["blocks"].value
    lit = [b for b in blocks if b.value["state"].value in lantern]
    if not lantern or not lit:
        fail("the hall's template holds no lantern to take out")
    # The lantern becomes a chain, which lights nothing: taken out to air, the
    # dark cell it leaves is where the relight siting puts a lantern back.
    palette.append(
        anvil.Tag(
            anvil.T_COMPOUND,
            {
                "Name": anvil.Tag(anvil.T_STRING, "minecraft:iron_chain"),
                "Properties": anvil.Tag(
                    anvil.T_COMPOUND,
                    {
                        "axis": anvil.Tag(anvil.T_STRING, "y"),
                        "waterlogged": anvil.Tag(anvil.T_STRING, "false"),
                    },
                ),
            },
        )
    )
    lit[0].value["state"].value = len(palette) - 1
    nbt.write_bytes(gzip.compress(anvil.nbt_write(name, root), mtime=0))
    got = build(delvec, gallery_domain.GALLERY, lib, a.work / "out-lantern")
    gw = worlds(got["record"])
    print(
        f"3. lantern: 1 of {len(lit)} lantern(s) of the hall's template made a chain; load world light "
        f"sha256 {bw['at-load']['light_sha256'][:12]}… -> {gw['at-load']['light_sha256'][:12]}…"
    )
    if gw["at-load"]["light_sha256"] == bw["at-load"]["light_sha256"]:
        fail("taking a lantern out of the hall did not move the load world's light")
    passed += 1

    # 4. The muster cutscene's ninth shot.
    p = point(a.work, "ninth-shot")
    cut = []

    def drop_ninth(doc):
        for e in muster_bundle(doc):
            if e.get("type") == "cutscene" and len(e.get("shots") or []) >= 9:
                del e["shots"][8]
                cut.append(True)
                return

    edit(p / "quests.json", drop_ninth)
    if not cut:
        fail(f"`{STEP}`'s bundle carries no cutscene of nine shots")
    got = build(delvec, p, a.prefabs, a.work / "out-ninth-shot")
    views = lambda plan: {s["id"] for s in plan["shots"] if s["kind"] == "cutscene"}  # noqa: E731
    gone = views(base["plan"]) - views(got["plan"])
    print(f"4. ninth shot: {len(views(base['plan']))} cutscene view(s) -> {len(views(got['plan']))}; gone: {sorted(gone)}")
    if not gone:
        fail("deleting the muster cutscene's ninth shot removed no cutscene view")
    r = subprocess.run(
        [sys.executable, str(REPO / "tools" / "ci" / "check-gallery-render.py"), "--delvec", str(delvec),
         "--prefabs", str(a.prefabs), "--build-out", str(got["out"]), "--frames", str(a.work / "frames-ninth")],
        capture_output=True,
        text=True,
    )
    named = all(v in r.stderr for v in gone)
    print(f"   check-gallery-render.py on that build: exit {r.returncode}, names the gone view(s): {named}")
    if r.returncode == 0 or not named:
        fail("the render gate did not red on the committed set naming the vanished cutscene view")
    passed += 1

    # 5. A set-time cut in the muster's bundle.
    p = point(a.work, "set-time")
    # An hour the approved design already shows (`DW0890` holds every hour the
    # world reaches to a picture of it).
    edit(p / "quests.json", lambda doc: muster_bundle(doc).append({"type": "set-time", "time": {"sun": "rising"}}))
    got = build(delvec, p, a.prefabs, a.work / "out-set-time")
    later = lambda plan: {  # noqa: E731
        s["id"]: s["sky"] for s in plan["shots"] if s["kind"] == "pov" and s["world"] != "at-load"
    }
    b_sky, g_sky = later(base["plan"]), later(got["plan"])
    moved = [i for i in b_sky if i in g_sky and b_sky[i] != g_sky[i]]
    print(f"5. set-time: {len(moved)} of {len(b_sky)} later POV shot(s) moved their sky")
    if not moved:
        fail("a set-time cut in the muster's bundle moved no later leg's sky")
    passed += 1

    print(f"gallery review frames: {passed} of 5 perturbation(s), each moved what it is declared to move")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
