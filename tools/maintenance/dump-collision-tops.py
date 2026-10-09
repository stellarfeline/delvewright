#!/usr/bin/env python3
"""Regenerate the collision-height table from the PINNED Minecraft server jar.

`crates/dsl/data/collision-tops-<version>.tsv` is what
`delvewright_dsl::blockshape` reads a partial block's standing height from, and
what its tests hold every hand-written height in that module against. It is a
measurement, not a table somebody typed: every value comes from calling the
game's own `BlockState.getCollisionShape(BlockGetter, BlockPos)` inside the
pinned server jar and reading the shape's vertical extent.

    tools/maintenance/dump-collision-tops.py            # rewrite the table
    tools/maintenance/dump-collision-tops.py --check    # regenerate and diff

The jar is identified by `versions.toml` and refused unless its sha256 matches
the pin; the mappings are fetched from piston-meta and verified by sha1. No
obfuscated name is written down here or in the Java dumper
(`tools/maintenance/collision/CollisionTopDump.java`). The pin, fetch and
mapping steps are `tools/maintenance/dump-block-light.py`'s own, imported, not
copied.

Requires a JDK (>= the pin's `javaVersion.majorVersion`) on PATH and network
access. A REGENERATION tool: CI reads the committed table and never runs this.
Run it when the MC pin moves.
"""

from __future__ import annotations

import argparse
import collections
import hashlib
import importlib.util
import json
import pathlib
import re
import subprocess
import sys
import tempfile
from fractions import Fraction

ROOT = pathlib.Path(__file__).resolve().parents[2]
JAVA_SRC = ROOT / "tools" / "maintenance" / "collision" / "CollisionTopDump.java"
DATA_DIR = ROOT / "crates" / "dsl" / "data"

_spec = importlib.util.spec_from_file_location(
    "dump_block_light", ROOT / "tools" / "maintenance" / "dump-block-light.py"
)
light = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(light)

CLASS_SHARED = "net.minecraft.SharedConstants"
CLASS_BOOTSTRAP = "net.minecraft.server.Bootstrap"
CLASS_BLOCK = "net.minecraft.world.level.block.Block"
CLASS_STATE_BASE = "net.minecraft.world.level.block.state.BlockBehaviour$BlockStateBase"
CLASS_EMPTY_GETTER = "net.minecraft.world.level.EmptyBlockGetter"
CLASS_BLOCKPOS = "net.minecraft.core.BlockPos"
CLASS_VOXEL = "net.minecraft.world.phys.shapes.VoxelShape"
CLASS_AXIS = "net.minecraft.core.Direction$Axis"

STATE_RE = re.compile(r"^Block\{minecraft:([a-z0-9_]+)\}(?:\[(.*)\])?$")


def fail(msg: str) -> None:
    sys.exit(f"dump-collision-tops: {msg}")


def sixteenths(v: str) -> str:
    """A shape bound in sixteenths of a block, exactly: `-` for an empty shape,
    an integer when the bound is a whole sixteenth (every vanilla box is built
    from `Block.box` in sixteenths), otherwise a reduced fraction — never a
    rounded float."""
    if v == "-":
        return "-"
    f = Fraction(v).limit_denominator(1 << 20) * 16
    return str(f.numerator) if f.denominator == 1 else f"{f.numerator}/{f.denominator}"


def read_states(states_tsv: pathlib.Path, width: int) -> dict[str, list[tuple[dict[str, str], tuple]]]:
    """The dumper's per-blockstate lines, grouped by block: each state's
    properties beside the `width` value columns that follow it on its line."""
    per: dict[str, list[tuple[dict[str, str], tuple]]] = collections.defaultdict(list)
    for ln in states_tsv.read_text(encoding="utf8").splitlines():
        parts = ln.rsplit("\t", width)
        s, values = parts[0], tuple(parts[1:])
        m = STATE_RE.match(s)
        if not m:
            fail(f"unparsable blockstate from the dumper: {s!r}")
        props = {}
        if m.group(2):
            for kv in m.group(2).split(","):
                k, v = kv.split("=", 1)
                props[k] = v
        per[m.group(1)].append((props, values))
    return per


def collapse_values(
    per: dict[str, list[tuple[dict[str, str], tuple]]],
) -> tuple[list[tuple[str, tuple, int]], int]:
    """Collapse every blockstate onto the properties that move its value. Each
    row names the block and those properties only; within a row the value is
    asserted constant, so the row stands for every state it matches. The one
    collapse every per-blockstate table this directory writes is made with."""
    rows: list[tuple[str, tuple, int]] = []
    covered = 0
    for block in sorted(per):
        states = per[block]
        keys = sorted(states[0][0])
        rel = []
        for k in keys:
            groups: dict[tuple, set] = collections.defaultdict(set)
            for props, ext in states:
                other = tuple(sorted((kk, vv) for kk, vv in props.items() if kk != k))
                groups[other].add(ext)
            if any(len(v) > 1 for v in groups.values()):
                rel.append(k)
        buckets: dict[tuple, list] = collections.defaultdict(list)
        for props, ext in states:
            buckets[tuple((k, props[k]) for k in rel)].append(ext)
        for key in sorted(buckets):
            exts = set(buckets[key])
            if len(exts) != 1:
                fail(f"{block}{key}: value is not constant within the group: {exts}")
            name = f"minecraft:{block}" + (
                "[" + ",".join(f"{k}={v}" for k, v in key) + "]" if key else ""
            )
            rows.append((name, exts.pop(), len(buckets[key])))
            covered += len(buckets[key])
    return rows, covered


def collapse(states_tsv: pathlib.Path) -> tuple[list[tuple[str, str, str, int]], int]:
    """Collapse every blockstate onto the properties that move its vertical
    extent: [`collapse_values`] over the two extent columns, in sixteenths."""
    per = read_states(states_tsv, 2)
    for block in per:
        per[block] = [(props, (sixteenths(lo), sixteenths(hi))) for props, (lo, hi) in per[block]]
    rows, covered = collapse_values(per)
    return [(n, lo, hi, c) for n, (lo, hi), c in rows], covered


def prepare(work: pathlib.Path) -> tuple[str, pathlib.Path, pathlib.Path, str, dict]:
    """Fetch and verify the pinned jar and its mappings into `work`, unpack the
    bundle, and return `(version, jar, mappings, classpath, parsed mappings)`:
    the setup every dumper in this directory runs before it reflects anything."""
    pin = light.read_pin()
    version = pin["version"]
    work.mkdir(parents=True, exist_ok=True)

    jar = work / "server.jar"
    light.fetch(pin["server_jar_url"], jar, pin["server_jar_sha256"])
    manifest = work / "manifest.json"
    light.fetch(light.MANIFEST, manifest, None)
    entries = [
        v for v in json.loads(manifest.read_text(encoding="utf8"))["versions"] if v["id"] == version
    ]
    if not entries:
        fail(f"piston manifest has no version {version}")
    vjson = work / "version.json"
    light.fetch(entries[0]["url"], vjson, entries[0]["sha1"], "sha1")
    downloads = json.loads(vjson.read_text(encoding="utf8"))["downloads"]
    if downloads["server"]["url"] != pin["server_jar_url"]:
        fail("piston's server jar url disagrees with versions.toml — refusing")
    mappings = work / "server.txt"
    light.fetch(
        downloads["server_mappings"]["url"], mappings, downloads["server_mappings"]["sha1"], "sha1"
    )

    bundle = work / "bundle"
    subprocess.run(
        ["unzip", "-o", "-q", str(jar), "-d", str(bundle), "META-INF/versions/*", "META-INF/libraries/*"],
        check=True,
    )
    inner = list((bundle / "META-INF" / "versions").rglob("*.jar"))
    if len(inner) != 1:
        fail(f"expected one bundled server jar, found {len(inner)}")
    classpath = ":".join(
        [str(inner[0])] + [str(p) for p in sorted((bundle / "META-INF" / "libraries").rglob("*.jar"))]
    )
    return version, jar, mappings, classpath, light.parse_mappings(mappings)


def generate(work: pathlib.Path) -> tuple[str, str]:
    version, jar, mappings, classpath, maps = prepare(work)
    args = []
    args += light.resolve(maps, CLASS_SHARED, methods=["tryDetectVersion"])
    args += light.resolve(maps, CLASS_BOOTSTRAP, methods=["bootStrap"])
    args += light.resolve(maps, CLASS_BLOCK, fields=["BLOCK_STATE_REGISTRY"])
    # `getCollisionShape` is overloaded; the mappings keep the first by name,
    # which is the two-argument form the dumper reflects by arity and refuses
    # when it is ambiguous.
    args += light.resolve(maps, CLASS_STATE_BASE, methods=["getCollisionShape"])
    args += light.resolve(maps, CLASS_EMPTY_GETTER, fields=["INSTANCE"])
    args += light.resolve(maps, CLASS_BLOCKPOS, fields=["ZERO"])
    args += light.resolve(maps, CLASS_VOXEL, methods=["min", "max", "isEmpty"])
    args += light.resolve(maps, CLASS_AXIS, fields=["Y"])
    sys.stderr.write(f"  resolved from the pinned mappings: {' '.join(args)}\n")

    classes = work / "classes"
    classes.mkdir(exist_ok=True)
    subprocess.run(
        ["javac", "-nowarn", "-cp", classpath, "-d", str(classes), str(JAVA_SRC)], check=True
    )
    states_tsv = work / "collision-states.tsv"
    proc = subprocess.run(
        ["java", "-Xmx3g", "-cp", f"{classpath}:{classes}", "dw.CollisionTopDump", str(states_tsv), *args],
        check=True,
        capture_output=True,
        text=True,
        cwd=work,
    )
    counted = re.search(r"DUMPED states=(\d+) with-collision=(\d+)", proc.stdout)
    if not counted or int(counted.group(1)) == 0 or int(counted.group(2)) == 0:
        fail(f"the dumper reported nothing to count: {proc.stdout.strip()!r}")
    sys.stderr.write(f"  dumper: {counted.group(0)}\n")

    rows, covered = collapse(states_tsv)
    if covered != int(counted.group(1)):
        fail(f"collapse covered {covered} states, the dumper produced {counted.group(1)}")

    jar_sha = hashlib.sha256(jar.read_bytes()).hexdigest()
    map_sha = hashlib.sha1(mappings.read_bytes()).hexdigest()
    head = [
        f"# Collision-box vertical extent of every blockstate of Minecraft Java {version}.",
        "#",
        "# MEASURED, not written: each row is the bottom and top of the shape the",
        "# game's own BlockState.getCollisionShape(EmptyBlockGetter.INSTANCE, BlockPos.ZERO)",
        "# returns inside the pinned server jar, in sixteenths of a block above the",
        "# cell floor; `-` for an empty shape.",
        f"#   server jar sha256 {jar_sha}",
        f"#   server mappings sha1 {map_sha}",
        "# Regenerate with tools/maintenance/dump-collision-tops.py; --check diffs against this file.",
        "#",
        f"# {len(rows)} rows collapse the game's {covered} blockstates onto the properties",
        "# that move the extent; a row names only those properties.",
        "#",
        "# block[relevant-properties]<TAB>bottom<TAB>top<TAB>states-this-row-stands-for",
    ]
    body = "\n".join(f"{n}\t{lo}\t{hi}\t{c}" for n, lo, hi, c in rows)
    return version, "\n".join(head) + "\n" + body + "\n"


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--check", action="store_true", help="diff against the committed table")
    ap.add_argument("--work", help="cache directory for the jar and mappings")
    args = ap.parse_args()
    work = (
        pathlib.Path(args.work).resolve()
        if args.work
        else pathlib.Path(tempfile.mkdtemp(prefix="dwcollision-"))
    )
    version, text = generate(work)
    target = DATA_DIR / f"collision-tops-{version}.tsv"
    if args.check:
        if not target.exists():
            fail(f"{target} does not exist")
        if target.read_text(encoding="utf8") == text:
            print(f"dump-collision-tops: {target.relative_to(ROOT)} matches the pinned jar")
            return
        sys.exit(f"dump-collision-tops: {target.relative_to(ROOT)} DISAGREES with the pinned jar")
    target.write_text(text, encoding="utf8")
    print(f"dump-collision-tops: wrote {target.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
