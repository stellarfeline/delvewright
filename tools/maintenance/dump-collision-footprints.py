#!/usr/bin/env python3
"""Regenerate the collision-footprint table from the PINNED Minecraft server jar.

`crates/dsl/data/collision-footprints-<version>.tsv` is what
`delvewright_dsl::blockshape` reads when it asks whether a block laid through a
floor holds a body standing over it, or lets the body fall between its boxes —
iron bars, a pane, a chain. It is a measurement, not a table somebody typed:
every value is the horizontal projection of the shape the game's own
`BlockState.getCollisionShape(BlockGetter, BlockPos)` returns inside the pinned
server jar, read box by box (`VoxelShape.toAabbs()`).

    tools/maintenance/dump-collision-footprints.py            # rewrite the table
    tools/maintenance/dump-collision-footprints.py --check    # regenerate and diff

The pin, fetch, mapping and collapse steps are `dump-collision-tops.py`'s own,
imported, not copied. No obfuscated name is written down here or in the Java
dumper (`tools/maintenance/collision/CollisionFootprintDump.java`).

Requires a JDK (>= the pin's `javaVersion.majorVersion`) on PATH and network
access. A REGENERATION tool: CI reads the committed table and never runs this.
Run it when the MC pin moves.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import pathlib
import re
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[2]
JAVA_SRC = ROOT / "tools" / "maintenance" / "collision" / "CollisionFootprintDump.java"
DATA_DIR = ROOT / "crates" / "dsl" / "data"

_spec = importlib.util.spec_from_file_location(
    "dump_collision_tops", ROOT / "tools" / "maintenance" / "dump-collision-tops.py"
)
tops = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(tops)
light = tops.light

CLASS_AABB = "net.minecraft.world.phys.AABB"


def generate(work: pathlib.Path) -> tuple[str, str]:
    version, jar, mappings, classpath, maps = tops.prepare(work)
    args = []
    args += light.resolve(maps, tops.CLASS_SHARED, methods=["tryDetectVersion"])
    args += light.resolve(maps, tops.CLASS_BOOTSTRAP, methods=["bootStrap"])
    args += light.resolve(maps, tops.CLASS_BLOCK, fields=["BLOCK_STATE_REGISTRY"])
    args += light.resolve(maps, tops.CLASS_STATE_BASE, methods=["getCollisionShape"])
    args += light.resolve(maps, tops.CLASS_EMPTY_GETTER, fields=["INSTANCE"])
    args += light.resolve(maps, tops.CLASS_BLOCKPOS, fields=["ZERO"])
    args += light.resolve(maps, tops.CLASS_VOXEL, methods=["toAabbs", "isEmpty"])
    args += light.resolve(maps, CLASS_AABB, fields=["minX", "minZ", "maxX", "maxZ"])
    sys.stderr.write(f"  resolved from the pinned mappings: {' '.join(args)}\n")

    classes = work / "classes-footprints"
    classes.mkdir(exist_ok=True)
    subprocess.run(
        ["javac", "-nowarn", "-cp", classpath, "-d", str(classes), str(JAVA_SRC)], check=True
    )
    states_tsv = work / "footprint-states.tsv"
    proc = subprocess.run(
        ["java", "-Xmx3g", "-cp", f"{classpath}:{classes}", "dw.CollisionFootprintDump", str(states_tsv), *args],
        check=True,
        capture_output=True,
        text=True,
        cwd=work,
    )
    counted = re.search(r"DUMPED states=(\d+) partial-footprint=(\d+)", proc.stdout)
    if not counted or any(int(counted.group(i)) == 0 for i in (1, 2)):
        tops.fail(f"the dumper reported nothing to count: {proc.stdout.strip()!r}")
    sys.stderr.write(f"  dumper: {counted.group(0)}\n")

    per = tops.read_states(states_tsv, 1)
    for block in per:
        per[block] = [(props, (fp, covered(fp))) for props, (fp,) in per[block]]
    rows, n = tops.collapse_values(per)
    if n != int(counted.group(1)):
        tops.fail(f"collapse covered {n} states, the dumper produced {counted.group(1)}")

    jar_sha = hashlib.sha256(jar.read_bytes()).hexdigest()
    map_sha = hashlib.sha1(mappings.read_bytes()).hexdigest()
    head = [
        f"# Collision footprint of every blockstate of Minecraft Java {version}.",
        "#",
        "# MEASURED, not written: each row is the horizontal (x, z) projection of the shape",
        "# the game's own BlockState.getCollisionShape(EmptyBlockGetter.INSTANCE, BlockPos.ZERO)",
        "# returns inside the pinned server jar, on a 16x16 grid of sixteenths. A sixteenth",
        "# is covered when it lies wholly inside one of the shape's boxes (VoxelShape.toAabbs()).",
        "# `full` = every sixteenth covered; `-` = an empty shape; otherwise sixteen",
        "# four-hex-digit rows, z = 0..15, bit x set when (x, z) is covered.",
        f"#   server jar sha256 {jar_sha}",
        f"#   server mappings sha1 {map_sha}",
        "# Regenerate with tools/maintenance/dump-collision-footprints.py; --check diffs against this file.",
        "#",
        f"# {len(rows)} rows collapse the game's {n} blockstates onto the properties",
        "# that move the footprint; a row names only those properties.",
        "#",
        "# block[relevant-properties]<TAB>footprint<TAB>covered-sixteenths<TAB>states-this-row-stands-for",
    ]
    body = "\n".join(f"{name}\t{fp}\t{cov}\t{c}" for name, (fp, cov), c in rows)
    return version, "\n".join(head) + "\n" + body + "\n"


def covered(fp: str) -> str:
    """How many of the 256 sixteenths a footprint covers, for a reader of the table."""
    if fp == "full":
        return "256"
    if fp == "-":
        return "0"
    return str(sum(bin(int(fp[i : i + 4], 16)).count("1") for i in range(0, 64, 4)))


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--check", action="store_true", help="diff against the committed table")
    ap.add_argument("--work", help="cache directory for the jar and mappings")
    args = ap.parse_args()
    work = (
        pathlib.Path(args.work).resolve()
        if args.work
        else pathlib.Path(tempfile.mkdtemp(prefix="dwfootprints-"))
    )
    version, text = generate(work)
    target = DATA_DIR / f"collision-footprints-{version}.tsv"
    if args.check:
        if not target.exists():
            tops.fail(f"{target} does not exist")
        if target.read_text(encoding="utf8") == text:
            print(f"dump-collision-footprints: {target.relative_to(ROOT)} matches the pinned jar")
            return
        sys.exit(f"dump-collision-footprints: {target.relative_to(ROOT)} DISAGREES with the pinned jar")
    target.write_text(text, encoding="utf8")
    print(f"dump-collision-footprints: wrote {target.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
