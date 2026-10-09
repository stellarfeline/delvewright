#!/usr/bin/env python3
"""Regenerate the full-face table from the PINNED Minecraft server jar.

`crates/dsl/data/faces-<version>.tsv` is what `delvewright_dsl::blockshape`
reads when it asks whether a block that hangs on another — a ladder on a wall, a
vine on a trunk, a weeping vine under a ceiling — is held there (spec-0099). It is
a measurement, not a table somebody typed: every value comes from calling the
game's own `BlockState.isFaceSturdy(BlockGetter, BlockPos, Direction)` and
`Block.isFaceFull(getCollisionShape(...), Direction)` inside the pinned server jar.

    tools/maintenance/dump-faces.py            # rewrite the table
    tools/maintenance/dump-faces.py --check    # regenerate and diff

The pin, fetch, mapping and collapse steps are `dump-collision-tops.py`'s own,
imported, not copied. No obfuscated name is written down here or in the Java
dumper (`tools/maintenance/collision/FaceDump.java`).

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
JAVA_SRC = ROOT / "tools" / "maintenance" / "collision" / "FaceDump.java"
DATA_DIR = ROOT / "crates" / "dsl" / "data"

_spec = importlib.util.spec_from_file_location(
    "dump_collision_tops", ROOT / "tools" / "maintenance" / "dump-collision-tops.py"
)
tops = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(tops)
light = tops.light

CLASS_DIRECTION = "net.minecraft.core.Direction"


def generate(work: pathlib.Path) -> tuple[str, str]:
    version, jar, mappings, classpath, maps = tops.prepare(work)
    args = []
    args += light.resolve(maps, tops.CLASS_SHARED, methods=["tryDetectVersion"])
    args += light.resolve(maps, tops.CLASS_BOOTSTRAP, methods=["bootStrap"])
    args += light.resolve(maps, tops.CLASS_BLOCK, fields=["BLOCK_STATE_REGISTRY"], methods=["isFaceFull"])
    # `getCollisionShape` and `isFaceSturdy` are both overloaded; the dumper
    # reflects each by arity and its last parameter, and refuses an ambiguity.
    args += light.resolve(maps, tops.CLASS_STATE_BASE, methods=["getCollisionShape", "isFaceSturdy"])
    args += light.resolve(maps, tops.CLASS_EMPTY_GETTER, fields=["INSTANCE"])
    args += light.resolve(maps, tops.CLASS_BLOCKPOS, fields=["ZERO"])
    args += light.resolve(maps, CLASS_DIRECTION, fields=["DOWN", "UP", "NORTH", "SOUTH", "WEST", "EAST"])
    sys.stderr.write(f"  resolved from the pinned mappings: {' '.join(args)}\n")

    classes = work / "classes-faces"
    classes.mkdir(exist_ok=True)
    subprocess.run(
        ["javac", "-nowarn", "-cp", classpath, "-d", str(classes), str(JAVA_SRC)], check=True
    )
    states_tsv = work / "face-states.tsv"
    proc = subprocess.run(
        ["java", "-Xmx3g", "-cp", f"{classpath}:{classes}", "dw.FaceDump", str(states_tsv), *args],
        check=True,
        capture_output=True,
        text=True,
        cwd=work,
    )
    counted = re.search(r"DUMPED states=(\d+) any-sturdy=(\d+) any-full=(\d+)", proc.stdout)
    if not counted or any(int(counted.group(i)) == 0 for i in (1, 2, 3)):
        tops.fail(f"the dumper reported nothing to count: {proc.stdout.strip()!r}")
    sys.stderr.write(f"  dumper: {counted.group(0)}\n")

    rows, covered = tops.collapse_values(tops.read_states(states_tsv, 2))
    if covered != int(counted.group(1)):
        tops.fail(f"collapse covered {covered} states, the dumper produced {counted.group(1)}")

    jar_sha = hashlib.sha256(jar.read_bytes()).hexdigest()
    map_sha = hashlib.sha1(mappings.read_bytes()).hexdigest()
    head = [
        f"# Full faces of every blockstate of Minecraft Java {version}.",
        "#",
        "# MEASURED, not written: for each of the six faces, in the order d u n s w e",
        "# (down, up, north, south, west, east), `sturdy` names the faces for which the",
        "# game's own BlockState.isFaceSturdy(EmptyBlockGetter.INSTANCE, BlockPos.ZERO, face)",
        "# answers true (SupportType.FULL: the support shape's face is the whole square),",
        "# and `full` the faces for which Block.isFaceFull(getCollisionShape(...), face)",
        "# does; `-` when none does.",
        f"#   server jar sha256 {jar_sha}",
        f"#   server mappings sha1 {map_sha}",
        "# Regenerate with tools/maintenance/dump-faces.py; --check diffs against this file.",
        "#",
        f"# {len(rows)} rows collapse the game's {covered} blockstates onto the properties",
        "# that move the faces; a row names only those properties.",
        "#",
        "# block[relevant-properties]<TAB>sturdy<TAB>full<TAB>states-this-row-stands-for",
    ]
    body = "\n".join(f"{n}\t{s}\t{f}\t{c}" for n, (s, f), c in rows)
    return version, "\n".join(head) + "\n" + body + "\n"


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--check", action="store_true", help="diff against the committed table")
    ap.add_argument("--work", help="cache directory for the jar and mappings")
    args = ap.parse_args()
    work = (
        pathlib.Path(args.work).resolve()
        if args.work
        else pathlib.Path(tempfile.mkdtemp(prefix="dwfaces-"))
    )
    version, text = generate(work)
    target = DATA_DIR / f"faces-{version}.tsv"
    if args.check:
        if not target.exists():
            tops.fail(f"{target} does not exist")
        if target.read_text(encoding="utf8") == text:
            print(f"dump-faces: {target.relative_to(ROOT)} matches the pinned jar")
            return
        sys.exit(f"dump-faces: {target.relative_to(ROOT)} DISAGREES with the pinned jar")
    target.write_text(text, encoding="utf8")
    print(f"dump-faces: wrote {target.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
