#!/usr/bin/env python3
"""Regenerate the block-entity table from the PINNED Minecraft server jar.

`crates/delvec/data/block-entities-<version>.tsv` is what the world writer
(`delvec::compiler::registry::block_entity_type`) reads when it asks whether a
block it writes carries a block entity, and of which type. The pinned server
makes a block entity for every `EntityBlock` it places — a structure template's
barrel, a `setblock` chest, a sign — whether or not the write carries a
compound, and saves it with the chunk; a world written without it is not the
world the server builds (`DW1040`). It is a measurement, not a table somebody
typed: every row is the game's own
`EntityBlock.newBlockEntity(BlockPos.ZERO, state).getType()`, named by
`BuiltInRegistries.BLOCK_ENTITY_TYPE.getKey`, over every blockstate, and a
block whose states disagree on the type is refused.

    tools/maintenance/dump-block-entities.py            # rewrite the table
    tools/maintenance/dump-block-entities.py --check    # regenerate and diff

The pin, fetch and mapping steps are `dump-collision-tops.py`'s own, imported,
not copied. No obfuscated name is written down here or in the Java dumper
(`tools/maintenance/blockentity/BlockEntityDump.java`).

Requires a JDK (>= the pin's `javaVersion.majorVersion`) on PATH and network
access. A REGENERATION tool: CI reads the committed table and never runs this.
Run it when the MC pin moves.
"""

from __future__ import annotations

import argparse
import collections
import hashlib
import importlib.util
import pathlib
import re
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[2]
JAVA_SRC = ROOT / "tools" / "maintenance" / "blockentity" / "BlockEntityDump.java"
DATA_DIR = ROOT / "crates" / "delvec" / "data"

_spec = importlib.util.spec_from_file_location(
    "dump_collision_tops", ROOT / "tools" / "maintenance" / "dump-collision-tops.py"
)
tops = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(tops)
light = tops.light

CLASS_ENTITY_BLOCK = "net.minecraft.world.level.block.EntityBlock"
CLASS_BLOCK_ENTITY = "net.minecraft.world.level.block.entity.BlockEntity"
CLASS_BUILTIN = "net.minecraft.core.registries.BuiltInRegistries"
CLASS_REGISTRY = "net.minecraft.core.Registry"


def generate(work: pathlib.Path) -> tuple[str, str]:
    version, jar, mappings, classpath, maps = tops.prepare(work)
    args = []
    args += light.resolve(maps, tops.CLASS_SHARED, methods=["tryDetectVersion"])
    args += light.resolve(maps, tops.CLASS_BOOTSTRAP, methods=["bootStrap"])
    args += light.resolve(maps, tops.CLASS_BLOCK, fields=["BLOCK_STATE_REGISTRY"])
    args += light.resolve(maps, tops.CLASS_STATE_BASE, methods=["getBlock"])
    args += light.resolve(maps, tops.CLASS_BLOCKPOS, fields=["ZERO"])
    args += light.resolve(maps, CLASS_ENTITY_BLOCK, methods=["newBlockEntity"])
    args += light.resolve(maps, CLASS_BLOCK_ENTITY, methods=["getType"])
    args += light.resolve(maps, CLASS_BUILTIN, fields=["BLOCK_ENTITY_TYPE"])
    args += light.resolve(maps, CLASS_REGISTRY, methods=["getKey"])
    sys.stderr.write(f"  resolved from the pinned mappings: {' '.join(args)}\n")

    classes = work / "classes-block-entities"
    classes.mkdir(exist_ok=True)
    subprocess.run(["javac", "-nowarn", "-cp", classpath, "-d", str(classes), str(JAVA_SRC)], check=True)
    states_tsv = work / "block-entity-states.tsv"
    proc = subprocess.run(
        ["java", "-Xmx3g", "-cp", f"{classpath}:{classes}", "dw.BlockEntityDump", str(states_tsv), *args],
        check=True,
        capture_output=True,
        text=True,
        cwd=work,
    )
    counted = re.search(r"DUMPED states=(\d+) entity-states=(\d+) entity-states-made-none=(\d+)", proc.stdout)
    if not counted or int(counted.group(1)) == 0 or int(counted.group(2)) == 0:
        tops.fail(f"the dumper reported nothing to count: {proc.stdout.strip()!r}")
    sys.stderr.write(f"  dumper: {counted.group(0)}\n")

    per: dict[str, set[str]] = collections.defaultdict(set)
    covered = 0
    for ln in states_tsv.read_text(encoding="utf8").splitlines():
        state, kind = ln.rsplit("\t", 1)
        m = tops.STATE_RE.match(state)
        if not m:
            tops.fail(f"unparsable blockstate from the dumper: {state!r}")
        per[m.group(1)].add(kind)
        covered += 1
    if covered != int(counted.group(1)):
        tops.fail(f"read {covered} states, the dumper produced {counted.group(1)}")
    rows: list[tuple[str, str]] = []
    made_none: list[str] = []
    for block in sorted(per):
        kinds = per[block]
        if len(kinds) != 1:
            tops.fail(f"{block}: its states make block entities of {sorted(kinds)} — one type per block is assumed")
        (kind,) = kinds
        if kind == "!":
            made_none.append(f"minecraft:{block}")
        elif kind != "-":
            rows.append((f"minecraft:{block}", kind))
    types = sorted({t for _, t in rows})
    jar_sha = hashlib.sha256(jar.read_bytes()).hexdigest()
    map_sha = hashlib.sha1(mappings.read_bytes()).hexdigest()
    head = [
        f"# The block entity every block of Minecraft Java {version} carries, where it carries one.",
        "#",
        "# MEASURED, not written: one row per block id whose blocks are an EntityBlock, its type",
        "# EntityBlock.newBlockEntity(BlockPos.ZERO, state).getType() named by",
        "# BuiltInRegistries.BLOCK_ENTITY_TYPE.getKey, the same on every state (asserted by the",
        "# driver). A block with no row carries no block entity.",
        f"#   server jar sha256 {jar_sha}",
        f"#   server mappings sha1 {map_sha}",
        "# Regenerate with tools/maintenance/dump-block-entities.py; --check diffs against this file.",
        "#",
        f"# {len(rows)} of {len(per)} blocks over {covered} blockstates carry one of {len(types)} block entity types.",
        "# EntityBlocks whose newBlockEntity makes none (a block entity some other act makes), so no",
        f"# row: {', '.join(made_none) or 'none'}.",
        "#",
        "# block<TAB>block_entity_type",
    ]
    body = "\n".join(f"{b}\t{t}" for b, t in rows)
    return version, "\n".join(head) + "\n" + body + "\n"


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--check", action="store_true", help="diff against the committed table")
    ap.add_argument("--work", help="cache directory for the jar and mappings")
    args = ap.parse_args()
    work = (
        pathlib.Path(args.work).resolve()
        if args.work
        else pathlib.Path(tempfile.mkdtemp(prefix="dwblockentities-"))
    )
    version, text = generate(work)
    target = DATA_DIR / f"block-entities-{version}.tsv"
    if args.check:
        if not target.exists():
            tops.fail(f"{target} does not exist")
        if target.read_text(encoding="utf8") == text:
            print(f"dump-block-entities: {target.relative_to(ROOT)} matches the pinned jar")
            return
        sys.exit(f"dump-block-entities: {target.relative_to(ROOT)} DISAGREES with the pinned jar")
    target.write_text(text, encoding="utf8")
    print(f"dump-block-entities: wrote {target.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
