#!/usr/bin/env python3
"""The world the engine writes is the world the pinned server builds
(spec-0089 §5.4, `DW0955`; its placed objects, spec-0112 §8.2, `DW1040`).

    python3 tools/ci/check-written-world.py <build-dir> <written-world> <server-save>

Two methods that share no configuration: the written world is the compiler's
model of the world at load (`delvec written-world <campaign> -o <dir>` writes
it from the campaign and `--prefabs`, the build run in memory; `delvec build`
writes the same world as `validation/worlds/at-load`), and
the server save is the pinned game executing the
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
  The same class holds copper the server weathered: the same block family,
  unwaxed on the written side, one or more stages further along
  unaffected -> exposed -> weathered -> oxidized on the server side, with every
  property equal. Vanilla's `WeatheringCopper` (the interface, `WeatherState`
  and its next-stage map) is advanced by `ChangeOverTimeBlock.changeOverTime`
  on a random tick, and nothing else moves a block along it unprompted (1.21.11
  server, Mojang mappings). The families are read from the pinned block
  registry (`crates/dsl/data/blocks-1.21.11.json`): a name `exposed_<stem>`
  whose `weathered_<stem>` and `oxidized_<stem>` also exist, the unaffected
  stage being `<stem>` (`copper_block` for the stem `copper`). A waxed block
  (`waxed_` prefix: honeycomb, never advanced by the tick) that changed, and a
  stage going backwards, stay in no class.
- **re-derived** — the same block on both sides, differing only in a property
  the server re-derives on a block update (a fence, wall, bar or pane's
  `north`/`south`/`east`/`west`/`up`, a stair's `shape`, leaves' `distance`).
- **clock** — a cell inside a gate region a clock owns (`clocked` in
  `validation/gate-seal.json`): its blocks are the clock's phase at the instant
  the save was copied.

A non-zero class is a finding about the model, reported with its cells; it
does not red. Every count is printed with its denominator.

**Item entities.** The server save is also read for every item entity
(`minecraft:item`, a block or item lying on the ground; `anvil.item_entities`),
anywhere in the save. A block the load writes and the server does not keep — a
falling block a later write lands on, a torch whose wall a later write takes —
lies there as one, and leaves no differing cell behind. The load's own
functions (`setup`, the ground and world-build steps, `place_all`,
`setup_finish`) are the only declarations that can put one there, as a `summon
minecraft:item`; an item entity beyond what they summon reds as `DW0955`,
with its position and item.

**Placed objects** (spec-0112 §8.2, `DW1040`). Beside the cells, the block
entities of the two worlds (`anvil.block_entities`) are compared by cell and
compound, and their entities (`anvil.entities`) by kind, cell and tag set. A
difference is either one of these classes — each counted and printed with its
objects, never an allowlist — or it reds as `DW1040`, naming the object:

- **bookkeeping** — a key the server writes for itself: `UUID`, `Motion`,
  `Fire`, `Air`, `OnGround`, `PortalCooldown`, `Health`, `Attributes`,
  `Brain`, `Rotation`'s second component, and any key the server's save holds
  that the written compound does not state (the server's own default for that
  kind). A key the written compound states and the save lacks, or holds at
  another value, is in no class.
- **loot** — a container whose `LootTable` the server rolled: its `Items`,
  `LootTable` and `LootTableSeed` are not compared.
- **played** — a body present only once a player has played (a wave's member,
  a cutscene's stand-in), on the server side alone.

Both denominators are printed: the block entities and the entities each world
holds.

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
REGISTRY = Path(__file__).resolve().parents[2] / "crates" / "dsl" / "data" / "blocks-1.21.11.json"
STAGES = ("exposed", "weathered", "oxidized")


def weathering_stages(names) -> dict[str, tuple[int, str]]:
    """Block name (bare) -> (stage index 0..3, family stem), for every block of a
    complete unaffected/exposed/weathered/oxidized ladder among `names`."""
    have = {base(n) for n in names}
    out: dict[str, tuple[int, str]] = {}
    for n in sorted(have):
        if not n.startswith("exposed_"):
            continue
        stem = n[len("exposed_"):]
        if not all(f"{s}_{stem}" in have for s in STAGES):
            continue
        unaffected = "copper_block" if stem == "copper" else stem
        if unaffected not in have:
            continue
        out[unaffected] = (0, stem)
        for i, s in enumerate(STAGES, 1):
            out[f"{s}_{stem}"] = (i, stem)
    return out


_WEATHERING: dict[str, tuple[int, str]] | None = None


def weathering() -> dict[str, tuple[int, str]]:
    global _WEATHERING
    if _WEATHERING is None:
        _WEATHERING = weathering_stages(json.loads(REGISTRY.read_text()))
    return _WEATHERING


def is_weathered(na: str, pa: dict, nb: str, pb: dict) -> bool:
    """The server's own rule could have produced `nb` from `na`."""
    w = weathering()
    a, b = w.get(base(na)), w.get(base(nb))
    return a is not None and b is not None and a[1] == b[1] and b[0] > a[0] and pa == pb


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
    if is_weathered(na, pa, nb, pb):
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


LOAD_FUNCTIONS = ("setup", "place_all", "setup_finish")
LOAD_STEPS = ("world_build", "world_ground")


def declared_items(build: Path) -> int:
    """How many `summon minecraft:item` lines the load's own functions hold —
    the only declarations that put an item entity in a save taken at load."""
    n = 0
    for f in sorted(build.glob("datapack/data/*/function/*.mcfunction")):
        stem = f.stem
        if stem not in LOAD_FUNCTIONS and not any(
            stem == s or (stem.startswith(s + "_") and stem[len(s) + 1 :].isdigit()) for s in LOAD_STEPS
        ):
            continue
        for line in f.read_text(encoding="utf-8").splitlines():
            if "summon minecraft:item " in f" {line} " or line.rstrip().endswith("summon minecraft:item"):
                n += 1
    return n


def item_findings(build: Path, server: Path) -> tuple[int, list[str], dict]:
    items = anvil.item_entities(server)
    declared = declared_items(build)
    total = sum(c for _, _, c in items)
    by = collections.Counter()
    for _, i, c in items:
        by[i] += c
    lines = [
        f"item entities: {len(items)} in the server save holding {total} item(s) "
        f"({', '.join(f'{k} x{v}' for k, v in sorted(by.items())) or 'none'}); "
        f"{declared} declared by a `summon minecraft:item` in the load's functions"
    ]
    sample = [
        {"pos": [round(c, 2) for c in pos], "item": i, "count": c}
        for pos, i, c in sorted(items, key=lambda r: (r[0], r[1]))[:SHOWN]
    ]
    summary = {"entities": len(items), "items": total, "declared": declared, "by_item": dict(sorted(by.items())), "sample": sample}
    if len(items) > declared:
        lines.append(
            f"{CODE} the server save holds {len(items) - declared} item entit(y/ies) no declaration put there: "
            "a block the load wrote and the server did not keep lies on the ground as an item (DW1002 judges "
            f"the writes that make one). First {len(sample)}: "
            + "; ".join(f"{s['item']} x{s['count']} at {s['pos']}" for s in sample)
        )
        return 1, lines, summary
    return 0, lines, summary


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


OBJECTS_CODE = "DW1040"
# Keys the server writes for itself on an entity (spec-0112 §8.2).
BOOKKEEPING = {
    "UUID", "Motion", "Fire", "Air", "OnGround", "PortalCooldown", "Health", "Attributes", "Brain",
    "FallDistance", "fall_distance",
}
LOOT_KEYS = {"Items", "LootTable", "LootTableSeed"}
PLAYED_TAGS = ("dw_wave_", "dw_cs_", "dw_standin")


def _same(a, b) -> bool:
    if isinstance(a, float) or isinstance(b, float):
        try:
            return abs(float(a) - float(b)) < 1e-6
        except (TypeError, ValueError):
            return False
    if isinstance(a, list) and isinstance(b, list):
        return len(a) == len(b) and all(_same(x, y) for x, y in zip(a, b))
    if isinstance(a, dict) and isinstance(b, dict):
        return a.keys() == b.keys() and all(_same(a[k], b[k]) for k in a)
    return a == b


def _compound_diff(written: dict, server: dict, skip: set[str]) -> tuple[list[str], list[str]]:
    """`(server-only keys, keys the written compound states that the save
    lacks or holds at another value)`, `skip` aside."""
    server_only = sorted(k for k in server if k not in written and k not in skip)
    wrong = []
    for k, v in sorted(written.items()):
        if k in skip:
            continue
        if k == "Rotation" and isinstance(v, list) and isinstance(server.get(k), list):
            if v and server[k] and _same(v[0], server[k][0]):
                continue
        if k not in server or not _same(v, server[k]):
            wrong.append(k)
    return server_only, wrong


def objects(written: Path, server: Path) -> tuple[int, list[str], dict]:
    """The placed-object comparison (`DW1040`)."""
    lines: list[str] = []
    counts = collections.Counter()
    findings: list[str] = []
    bw, bs = anvil.block_entities(written), anvil.block_entities(server)
    for cell in sorted(set(bw) | set(bs), key=lambda c: tuple(-1 << 30 if v is None else v for v in c)):
        a, b = bw.get(cell), bs.get(cell)
        if a is None or b is None:
            findings.append(
                f"block entity at {list(cell)}: {'written' if b is None else 'server'} side only "
                f"({(a or b).get('id')})"
            )
            continue
        skip = {"x", "y", "z", "id"}
        if "LootTable" in b or "LootTable" in a:
            skip |= LOOT_KEYS
            counts["loot"] += 1
        server_only, wrong = _compound_diff(a, b, skip)
        if a.get("id") != b.get("id"):
            wrong.append("id")
        if server_only:
            counts["bookkeeping"] += 1
        if wrong:
            findings.append(f"block entity {a.get('id')} at {list(cell)}: {', '.join(wrong)} differ")
    ew, es = anvil.entities(written), anvil.entities(server)

    def key(e: dict):
        pos = e.get("Pos") or [0, 0, 0]
        return (e.get("id"), tuple(int(float(p) // 1) for p in pos), frozenset(e.get("Tags") or []))

    left = collections.defaultdict(list)
    for e in es:
        left[key(e)].append(e)
    for e in ew:
        k = key(e)
        if not left.get(k):
            findings.append(
                f"entity {k[0]} at {list(k[1])} tags {sorted(k[2])}: written, not in the server save"
            )
            continue
        b = left[k].pop(0)
        server_only, wrong = _compound_diff(e, b, BOOKKEEPING)
        if server_only or any(x in b for x in BOOKKEEPING):
            counts["bookkeeping"] += 1
        if wrong:
            findings.append(f"entity {k[0]} at {list(k[1])}: {', '.join(wrong)} differ")
    for k, rest in sorted(left.items(), key=lambda kv: str(kv[0])):
        for _ in rest:
            if any(t.startswith(PLAYED_TAGS) for t in k[2]):
                counts["played"] += 1
            else:
                findings.append(
                    f"entity {k[0]} at {list(k[1])} tags {sorted(k[2])}: in the server save, not written"
                )
    lines.append(
        f"objects binding: {len(bw)} block entit(ies) written, {len(bs)} in the server save; "
        f"{len(ew)} entit(ies) written, {len(es)} in the server save"
    )
    for c in ("bookkeeping", "loot", "played"):
        lines.append(f"  {c}: {counts[c]} object(s)")
    summary = {
        "block_entities": {"written": len(bw), "server": len(bs)},
        "entities": {"written": len(ew), "server": len(es)},
        "classes": {c: counts[c] for c in ("bookkeeping", "loot", "played")},
        "differing": len(findings),
        "sample": findings[:SHOWN],
        "verdict": "fail" if findings else "pass",
    }
    if findings:
        lines.append(
            f"{OBJECTS_CODE} the objects the engine writes are not the objects the pinned server places: "
            f"{len(findings)} in no class — " + "; ".join(findings[:SHOWN])
            + ". The ledger or the writer is wrong at the objects named; fix the engine, never this comparison"
        )
        return 1, lines, summary
    lines.append("written world: every placed object is the server's outside the named classes")
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
    ap.add_argument("written", type=Path, help="the load world `delvec written-world <campaign> -o <dir>` wrote")
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
    summary["cells_verdict"] = "pass" if code == 0 else "fail"
    if (a.written / "level.dat").is_file() and (a.server / "level.dat").is_file():
        ocode, olines, osummary = objects(a.written, a.server)
        lines += olines
        summary["objects"] = osummary
        code = code or ocode
    if (a.server / "level.dat").is_file():
        icode, ilines, isummary = item_findings(a.build, a.server)
        lines += ilines
        summary["item_entities"] = isummary
        code = code or icode
    for line in lines:
        print(line)
    if a.record is not None:
        write_record(a.record, a.build, a.written, a.server, code, summary)
        print(f"written world: verdict recorded -> {a.record}")
    return code


if __name__ == "__main__":
    sys.exit(main(sys.argv))
