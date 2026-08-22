#!/usr/bin/env python3
"""Derive the shape-carrying block-state properties from the 1.21.11 client jar.

Writes ``crates/dsl/data/blockstate-shape-props-1.21.11.json``: block id →
the properties named by ``multipart`` selectors in the block's own blockstate
definition (``assets/minecraft/blockstates/<block>.json``).

Why multipart, and not "any property the model varies by": a ``variants``
property picks one complete model — a rotated stair, a snowy grass top, a
pressed button — so a state that omits it still renders as a whole block, and
the default is what the author meant. A ``multipart`` property *assembles* the
model: wall arms, pane connections, vine faces exist only when the property says
so, so a state that omits it ships an isolated post where the author drew a
wall. That is the line between a benign omission and a shape defect (DW0735),
and it is a property of the block class, derived from Mojang's own blockstate
definitions — never a hand-kept id list.

The jar itself is EULA-bound and never committed (same rule as the font
metrics); what is committed is the derived table of property *names*, and this
script is the reproduction path. Deterministic: sorted keys, sorted property
lists, no timestamps.

Usage:
    python3 tools/extract-shape-properties.py <minecraft-1.21.11-client.jar> \
        crates/dsl/data/blockstate-shape-props-1.21.11.json
"""

import json
import sys
import zipfile
from pathlib import Path

PIN_ID = "1.21.11"
PIN_DATA_VERSION = 4671


def selector_props(when):
    """Property names a multipart `when` condition tests, through OR/AND."""
    props = set()

    def walk(node):
        for key, value in node.items():
            if key in ("OR", "AND"):
                for sub in value:
                    walk(sub)
            else:
                props.add(key)

    walk(when)
    return props


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    jar_path, out_path = sys.argv[1], sys.argv[2]

    jar = zipfile.ZipFile(jar_path)
    version = json.loads(jar.read("version.json"))
    if version.get("id") != PIN_ID or version.get("world_version") != PIN_DATA_VERSION:
        sys.exit(
            f"refusing: jar is {version.get('id')!r} "
            f"(DataVersion {version.get('world_version')!r}), "
            f"not the pinned {PIN_ID} / {PIN_DATA_VERSION} (ADR-0009)"
        )

    # The pinned registry is the cross-check: every derived property must be a
    # property the block actually defines, or the derivation is reading the
    # wrong data.
    registry_path = (
        Path(__file__).resolve().parent.parent
        / "crates/dsl/data/blocks-1.21.11.json"
    )
    registry = json.loads(registry_path.read_text())

    prefix = "assets/minecraft/blockstates/"
    table = {}
    scanned = 0
    for name in sorted(jar.namelist()):
        if not (name.startswith(prefix) and name.endswith(".json")):
            continue
        scanned += 1
        block = "minecraft:" + name[len(prefix) : -len(".json")]
        definition = json.loads(jar.read(name))
        if "multipart" not in definition:
            continue
        props = set()
        for part in definition["multipart"]:
            props |= selector_props(part.get("when", {}))
        if not props:
            continue
        if block not in registry:
            sys.exit(f"refusing: {block} has a blockstate file but no registry entry")
        undefined = props - set(registry[block])
        if undefined:
            sys.exit(
                f"refusing: {block} multipart selectors name {sorted(undefined)}, "
                f"which the registry does not define for it"
            )
        table[block] = sorted(props)

    if scanned == 0:
        sys.exit("refusing: the jar has no blockstate definitions — wrong file?")

    out = json.dumps(table, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    Path(out_path).write_text(out)
    print(
        f"{len(table)} blocks with multipart (shape-carrying) properties, "
        f"from {scanned} blockstate definitions -> {out_path}"
    )


if __name__ == "__main__":
    main()
