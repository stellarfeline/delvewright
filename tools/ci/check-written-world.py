#!/usr/bin/env python3
"""The world `delvec cameras` writes is the world the pinned server builds
(spec-0089 §5.4, `DW0955`).

    python3 tools/ci/check-written-world.py <build-dir> <written-world> <server-save>

Two methods that share no configuration: the written world is the compiler's
model of the world at load (`delvec cameras … -o <out>` writes it to
`<out>/worlds/at-load`), and the server save is the pinned game executing the
compiler's datapack (`validation/world-save.sh <build-dir>` writes it to
`<build-dir>/world`). Both are read through the one reader, `tools/lib/anvil.py`,
and compared cell by cell inside the build's `layout_aabb`
(`render-plan.json`).

A differing cell is either one of the classes named below — each a measured
count, printed with its cells, never an allowlist of cells — or it reds as
`DW0955`:

- **gravity** — a gravity block (sand, gravel, concrete powder, an anvil, a
  dragon egg) on either side: the server settled it.
- **fluid** — a water or lava cell on either side: the server flowed it.
- **random-tick** — turf on one side and dirt on the other (grass block or
  mycelium against dirt): the server's random tick turned it. Vanilla's
  `SpreadingSnowyDirtBlock.randomTick` (the class of both) sets a block it
  cannot keep — under a cover light does not pass — to dirt, and spreads onto
  lit dirt beside it; which cells have turned by the instant of the save is
  the tick's draw (1.21.11 server, Mojang mappings).
- **re-derived** — the same block on both sides, differing only in a property
  the server re-derives on a block update (a fence, wall, bar or pane's
  `north`/`south`/`east`/`west`/`up`, a stair's `shape`, leaves' `distance`).
- **clock** — a cell inside a gate region a clock owns (`clocked` in
  `validation/gate-seal.json`): its blocks are the clock's phase at the instant
  the save was copied.

A non-zero class is a finding about the model, reported with its cells; it
does not red. Every count is printed with its denominator.

`--record <path>` also writes the verdict as JSON, named by the sha256 of the
build's `manifest.json` — the fingerprint the staging gate binds an admission
to — so `tools/creator/staging-gate.py --written-world <path>` can refuse a
build whose world was never compared, compared red, or compared for another
tree. The record is written on a red as on a green.
"""

from __future__ import annotations

import argparse
import collections
import hashlib
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
import anvil  # noqa: E402

CODE = "DW0955"

GRAVITY = {"sand", "red_sand", "gravel", "anvil", "chipped_anvil", "damaged_anvil", "dragon_egg"}
# The blocks `SpreadingSnowyDirtBlock.randomTick` turns into dirt, or spreads
# onto dirt as (grass block and mycelium are its two subclasses).
TURF = {"grass_block", "mycelium"}
CLASSES = ("gravity", "fluid", "random-tick", "re-derived", "clock", "model")
REDERIVED_KEYS = {"north", "south", "east", "west", "up", "shape", "distance"}
SHOWN = 12


def split(state: str | None) -> tuple[str, dict[str, str]]:
    if state is None:
        return "minecraft:air", {}
    if "[" not in state:
        return state, {}
    name, rest = state.split("[", 1)
    props = dict(p.split("=", 1) for p in rest.rstrip("]").split(",") if "=" in p)
    return name, props


def base(name: str) -> str:
    return name.split(":")[-1]


def is_gravity(name: str) -> bool:
    b = base(name)
    return b in GRAVITY or b.endswith("concrete_powder")


def is_fluid(name: str) -> bool:
    return base(name) in {"water", "lava"}


def classify(a: str | None, b: str | None, cell, clocked) -> str:
    if any(lo[0] <= cell[0] <= hi[0] and lo[1] <= cell[1] <= hi[1] and lo[2] <= cell[2] <= hi[2] for lo, hi in clocked):
        return "clock"
    (na, pa), (nb, pb) = split(a), split(b)
    if is_gravity(na) or is_gravity(nb):
        return "gravity"
    if is_fluid(na) or is_fluid(nb):
        return "fluid"
    if {base(na), base(nb)} in ({"dirt", t} for t in TURF):
        return "random-tick"
    if na == nb:
        differ = {k for k in set(pa) | set(pb) if pa.get(k) != pb.get(k)}
        if differ and differ <= REDERIVED_KEYS:
            return "re-derived"
    return "model"


def clocked_regions(build: Path):
    ledger = build / "validation" / "gate-seal.json"
    if not ledger.is_file():
        return []
    out = []
    for g in json.loads(ledger.read_text()).get("gates", []):
        if "clocked" not in g:
            raise SystemExit(
                f"check-written-world: {ledger} carries no `clocked` key on gate {g.get('anchor')}: "
                "the build is older than the reader, rebuild it"
            )
        if g["clocked"]:
            lo = [min(a, b) for a, b in zip(g["from"], g["to"])]
            hi = [max(a, b) for a, b in zip(g["from"], g["to"])]
            out.append((lo, hi))
    return out


def compare(build: Path, written: Path, server: Path) -> tuple[int, list[str], dict]:
    lines: list[str] = []
    summary: dict = {"compared": 0, "classes": {}, "model_sample": []}
    for w, what in ((written, "written world"), (server, "server save")):
        if not (w / "level.dat").is_file() or not list((w / "region").glob("r.*.mca")):
            lines.append(f"{CODE} {what} {w} holds no level.dat and region file: nothing to compare")
            return 1, lines, summary
    plan = json.loads((build / "render-plan.json").read_text())["layout_aabb"]
    box = (plan["min"], plan["max"])
    volume = 1
    for i in range(3):
        volume *= box[1][i] - box[0][i] + 1
    clocked = clocked_regions(build)
    a = anvil.cells(written, box)
    b = anvil.cells(server, box)
    union = set(a) | set(b)
    classes: dict[str, collections.Counter] = collections.defaultdict(collections.Counter)
    where: dict[tuple[str, str | None, str | None], list] = collections.defaultdict(list)
    for c in sorted(union):
        x, y = a.get(c), b.get(c)
        if x == y:
            continue
        k = classify(x, y, c, clocked)
        classes[k][(x, y)] += 1
        where[(k, x, y)].append(c)
    lines.append(
        f"written-world binding: box {box[0]}..{box[1]} ({volume} cell(s)); {len(a)} non-air cell(s) "
        f"written, {len(b)} in the server save, {len(union)} compared; {len(clocked)} clocked gate region(s)"
    )
    for k in CLASSES:
        n = sum(classes[k].values())
        lines.append(f"  {k}: {n} differing cell(s) of {len(union)}")
        for (x, y), cnt in classes[k].most_common():
            cells = where[(k, x, y)]
            more = f" (+{len(cells) - SHOWN} more)" if len(cells) > SHOWN else ""
            lines.append(f"    {cnt} x written {x or 'air'} / server {y or 'air'}: {cells[:SHOWN]}{more}")
    bad = sum(classes["model"].values())
    summary = {
        "box": [list(box[0]), list(box[1])],
        "compared": len(union),
        "classes": {k: sum(classes[k].values()) for k in CLASSES},
        "model_sample": [
            {"cell": list(c), "written": x or "minecraft:air", "server": y or "minecraft:air"}
            for (k, x, y), cells in sorted(where.items(), key=lambda kv: (kv[0][0], str(kv[0][1]), str(kv[0][2])))
            if k == "model"
            for c in cells[:SHOWN]
        ][:SHOWN],
    }
    if bad:
        lines.append(
            f"{CODE} the world the engine writes is not the world the pinned server builds: {bad} cell(s) "
            "inside the layout box differ in no class a server's physics or clock explains. A showcase "
            "frame is rendered from the written world, so each is a picture the game would not show — "
            "the model is wrong at those cells; fix the model, never this comparison"
        )
        return 1, lines, summary
    lines.append(f"written world: the written world is the server's at every compared cell outside the named classes")
    return 0, lines, summary


def manifest_sha256(build: Path) -> str | None:
    """The build's identity: the sha256 of its `manifest.json`, the same
    fingerprint `tools/creator/staging-gate.py` stamps into an admission."""
    m = build / "manifest.json"
    return hashlib.sha256(m.read_bytes()).hexdigest() if m.is_file() else None


def write_record(path: Path, build: Path, written: Path, server: Path, code: int, summary: dict) -> None:
    doc = {
        "check": "written-world",
        "code": CODE,
        "verdict": "pass" if code == 0 else "fail",
        "manifest_sha256": manifest_sha256(build),
        "build": str(build.resolve()),
        "written": str(written.resolve()),
        "server": str(server.resolve()),
        **summary,
    }
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(doc, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("build", type=Path, help="a `delvec build` output directory")
    ap.add_argument("written", type=Path, help="the world `delvec cameras` wrote (<out>/worlds/at-load)")
    ap.add_argument("server", type=Path, help="the save `validation/world-save.sh` wrote (<build>/world)")
    ap.add_argument(
        "--record",
        type=Path,
        help="also write the verdict as JSON here, named by the build's manifest sha256 "
        "(what `staging-gate.py --written-world` reads); outside the build tree",
    )
    a = ap.parse_args(argv[1:])
    if a.record is not None:
        try:
            a.record.resolve().relative_to(a.build.resolve())
        except ValueError:
            pass
        else:
            print(
                f"check-written-world: --record {a.record} is inside the build tree {a.build}; "
                "write it beside the tree",
                file=sys.stderr,
            )
            return 2
    code, lines, summary = compare(a.build, a.written, a.server)
    for line in lines:
        print(line)
    if a.record is not None:
        write_record(a.record, a.build, a.written, a.server, code, summary)
        print(f"written world: verdict recorded -> {a.record}")
    return code


if __name__ == "__main__":
    sys.exit(main(sys.argv))
