#!/usr/bin/env python3
"""Regenerate the outline-shape table from the PINNED Minecraft server jar.

`crates/dsl/data/outline-shapes-<version>.tsv` is what
`delvewright_dsl::blockshape::outline` reads when the compiler fits a click box
over a block (spec-0117 §3a) and when `DW1013` proves the box encloses the
block's outline. It is a measurement, not a table somebody typed: every row is
the list of boxes of the shape the game's own `BlockState.getShape(BlockGetter,
BlockPos)` returns inside the pinned server jar — the outline the crosshair's
pick hits — read box by box (`VoxelShape.toAabbs()`), in sixteenths.

    tools/maintenance/dump-outline-shapes.py            # rewrite the table
    tools/maintenance/dump-outline-shapes.py --check    # regenerate and diff

The pin, fetch, mapping and collapse steps are `dump-collision-tops.py`'s own,
imported, not copied. No obfuscated name is written down here or in the Java
dumper (`tools/maintenance/outline/OutlineShapeDump.java`).

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
JAVA_SRC = ROOT / "tools" / "maintenance" / "outline" / "OutlineShapeDump.java"
DATA_DIR = ROOT / "crates" / "dsl" / "data"

_spec = importlib.util.spec_from_file_location(
    "dump_collision_tops", ROOT / "tools" / "maintenance" / "dump-collision-tops.py"
)
tops = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(tops)
light = tops.light

CLASS_AABB = "net.minecraft.world.phys.AABB"


def boxes_in_sixteenths(raw: str) -> str:
    """The dumper's boxes, each bound in sixteenths exactly (`tops.sixteenths`:
    an integer, or a reduced fraction — never a rounded float), sorted so the
    row is the same whatever order the game decomposed the shape in."""
    if raw == "-":
        return "-"
    out = []
    for box in raw.split(";"):
        out.append(",".join(tops.sixteenths(v) for v in box.split(",")))
    return ";".join(sorted(out))


def generate(work: pathlib.Path) -> tuple[str, str]:
    version, jar, mappings, classpath, maps = tops.prepare(work)
    args = []
    args += light.resolve(maps, tops.CLASS_SHARED, methods=["tryDetectVersion"])
    args += light.resolve(maps, tops.CLASS_BOOTSTRAP, methods=["bootStrap"])
    args += light.resolve(maps, tops.CLASS_BLOCK, fields=["BLOCK_STATE_REGISTRY"])
    # `getShape` is overloaded; the dumper reflects the two-argument form by
    # arity under the mapped name and refuses when that is ambiguous.
    args += light.resolve(maps, tops.CLASS_STATE_BASE, methods=["getShape"])
    args += light.resolve(maps, tops.CLASS_EMPTY_GETTER, fields=["INSTANCE"])
    args += light.resolve(maps, tops.CLASS_BLOCKPOS, fields=["ZERO"])
    args += light.resolve(maps, tops.CLASS_VOXEL, methods=["toAabbs", "isEmpty"])
    args += light.resolve(maps, CLASS_AABB, fields=["minX", "minY", "minZ", "maxX", "maxY", "maxZ"])
    sys.stderr.write(f"  resolved from the pinned mappings: {' '.join(args)}\n")

    classes = work / "classes-outline"
    classes.mkdir(exist_ok=True)
    subprocess.run(
        ["javac", "-nowarn", "-cp", classpath, "-d", str(classes), str(JAVA_SRC)], check=True
    )
    states_tsv = work / "outline-states.tsv"
    proc = subprocess.run(
        ["java", "-Xmx3g", "-cp", f"{classpath}:{classes}", "dw.OutlineShapeDump", str(states_tsv), *args],
        check=True,
        capture_output=True,
        text=True,
        cwd=work,
    )
    counted = re.search(r"DUMPED states=(\d+) with-outline=(\d+)", proc.stdout)
    if not counted or any(int(counted.group(i)) == 0 for i in (1, 2)):
        tops.fail(f"the dumper reported nothing to count: {proc.stdout.strip()!r}")
    sys.stderr.write(f"  dumper: {counted.group(0)}\n")

    per = tops.read_states(states_tsv, 1)
    for block in per:
        per[block] = [
            (props, (b, "0" if b == "-" else str(b.count(";") + 1)))
            for props, (b,) in ((p, (boxes_in_sixteenths(r),)) for p, (r,) in per[block])
        ]
    rows, n = tops.collapse_values(per)
    if n != int(counted.group(1)):
        tops.fail(f"collapse covered {n} states, the dumper produced {counted.group(1)}")

    jar_sha = hashlib.sha256(jar.read_bytes()).hexdigest()
    map_sha = hashlib.sha1(mappings.read_bytes()).hexdigest()
    head = [
        f"# Outline shape of every blockstate of Minecraft Java {version}.",
        "#",
        "# MEASURED, not written: each row is the boxes of the shape the game's own",
        "# BlockState.getShape(EmptyBlockGetter.INSTANCE, BlockPos.ZERO) returns inside the",
        "# pinned server jar (VoxelShape.toAabbs()), each box `x0,y0,z0,x1,y1,z1` in",
        "# sixteenths of a block from the cell's minimum corner (an integer, or a reduced",
        "# fraction), boxes `;`-joined and sorted; `-` for an empty shape.",
        f"#   server jar sha256 {jar_sha}",
        f"#   server mappings sha1 {map_sha}",
        "# Regenerate with tools/maintenance/dump-outline-shapes.py; --check diffs against this file.",
        "#",
        f"# {len(rows)} rows collapse the game's {n} blockstates onto the properties",
        "# that move the outline; a row names only those properties.",
        "#",
        "# block[relevant-properties]<TAB>boxes<TAB>box-count<TAB>states-this-row-stands-for",
    ]
    body = "\n".join(f"{name}\t{b}\t{k}\t{c}" for name, (b, k), c in rows)
    return version, "\n".join(head) + "\n" + body + "\n"


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--check", action="store_true", help="diff against the committed table")
    ap.add_argument("--work", help="cache directory for the jar and mappings")
    args = ap.parse_args()
    work = (
        pathlib.Path(args.work).resolve()
        if args.work
        else pathlib.Path(tempfile.mkdtemp(prefix="dwoutline-"))
    )
    version, text = generate(work)
    target = DATA_DIR / f"outline-shapes-{version}.tsv"
    if args.check:
        if not target.exists():
            tops.fail(f"{target} does not exist")
        if target.read_text(encoding="utf8") == text:
            print(f"dump-outline-shapes: {target.relative_to(ROOT)} matches the pinned jar")
            return
        sys.exit(f"dump-outline-shapes: {target.relative_to(ROOT)} DISAGREES with the pinned jar")
    target.write_text(text, encoding="utf8")
    print(f"dump-outline-shapes: wrote {target.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
