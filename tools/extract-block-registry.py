#!/usr/bin/env python3
"""Extract the pinned 1.21.11 BLOCK-state registry from a `misode/mcmeta` summary.

The repo already checks every emitted *command* against a pinned command tree
(`crates/compiler/data/commands-1.21.11.json`), and every item id against a
pinned item registry. Nothing checked an emitted **block id** — which is how
`minecraft:chain`, renamed to `minecraft:iron_chain` in 1.21.11, reached a
shipped `.nbt` and stayed there. This script vendors the registry that closes
that gap.

Source: `blocks/data.min.json` of the `1.21.11-summary` branch, which is
Mojang's own generated block-state report republished verbatim. Its shape is

    {"oak_stairs": [{"facing": ["north", ...], ...}, {"facing": "north", ...}]}

i.e. `[every property's legal values, the default state]`. The vendored form
keeps both, namespaced and sorted:

    {"minecraft:oak_stairs": {
       "properties": {"facing": ["north", ...], ...},
       "default":    {"facing": "north", ...}}}

**Both halves are load-bearing.** The legal values answer "is this a state the
game has"; the DEFAULT answers "what state did the author actually write". A
structure template's palette entry may omit any property, and vanilla's
`BlockState` codec then fills it from the block's default — so an entry that
says `{"Name": "minecraft:cobblestone_wall"}` and one that spells out all six
properties at their defaults denote the *same* BlockState. Every consumer that
is not a running server (`delve-render`, `delve-admit`, occupancy analysis) can
only read what is written, and without the defaults it has to guess. The guess
was measured: the viewer unioned every multipart case of an under-specified
state, which drew a cobblestone wall as a solid 1x1x1 cube and reported nothing
unresolved.

The two halves are checked against each other here (same key set; every default
value legal), so a source that carried one without the other is a red rather
than a half-registry.

Reproduce (the same shape as `tools/extract-sound-registry.py`):

    curl -sSL -o /tmp/blocks.min.json \\
      https://raw.githubusercontent.com/misode/mcmeta/1.21.11-summary/blocks/data.min.json
    python3 tools/extract-block-registry.py /tmp/blocks.min.json \\
      crates/compiler/data/blocks-1.21.11.json

The source SHA-256 is pinned below and checked: a regeneration that silently
picked up a different version of the game would otherwise be invisible.
"""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path

# `blocks/data.min.json` @ misode/mcmeta tag `1.21.11-summary`
# (commit c976eb3b2cfcb9f205171527dec46b266afa3ac9), retrieved 2026-08-11.
SOURCE_SHA256 = "178a12096f59f863758a6c685e5eb6de38721b376a30a30383e171d0799f3ee7"

# 1.21.11 has exactly this many blocks. Pinned so that a source that parsed but
# yielded a wildly different registry is a red, not a quiet rewrite.
EXPECTED_BLOCKS = 1166


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        print(f"usage: {argv[0]} <blocks/data.min.json> <out.json>", file=sys.stderr)
        return 2

    src = Path(argv[1])
    out = Path(argv[2])

    raw = src.read_bytes()
    digest = hashlib.sha256(raw).hexdigest()
    if digest != SOURCE_SHA256:
        print(
            f"error: {src} has SHA-256 {digest}, expected {SOURCE_SHA256}.\n"
            "That is a different version of the game, not a different formatting "
            "of the same one. Update ADR-0009 and the pin here together, or fetch "
            "the 1.21.11-summary source again.",
            file=sys.stderr,
        )
        return 2

    data = json.loads(raw)
    if not isinstance(data, dict):
        print("error: source is not a JSON object", file=sys.stderr)
        return 2

    registry: dict[str, dict[str, object]] = {}
    for block_id, entry in data.items():
        if not isinstance(entry, list) or len(entry) != 2:
            print(f"error: {block_id!r} is not [properties, default]", file=sys.stderr)
            return 2
        properties, default = entry
        if not isinstance(properties, dict) or not isinstance(default, dict):
            print(f"error: {block_id!r} has no property map", file=sys.stderr)
            return 2
        for name, values in properties.items():
            if not isinstance(values, list) or not values:
                print(
                    f"error: {block_id!r} property {name!r} has no values",
                    file=sys.stderr,
                )
                return 2
        # The two halves must describe the same block, or the registry would
        # answer "is this legal" and "what did the author mean" about different
        # things. Checked here rather than trusted: this is the one place the
        # source is read.
        if set(default) != set(properties):
            print(
                f"error: {block_id!r} default state has properties "
                f"{sorted(default)}, but the block has {sorted(properties)}",
                file=sys.stderr,
            )
            return 2
        for name, value in default.items():
            if str(value) not in [str(v) for v in properties[name]]:
                print(
                    f"error: {block_id!r} default {name}={value!r} is not one of "
                    f"{sorted(str(v) for v in properties[name])}",
                    file=sys.stderr,
                )
                return 2
        registry[f"minecraft:{block_id}"] = {
            "default": {
                name: str(value) for name, value in sorted(default.items())
            },
            "properties": {
                name: sorted(str(v) for v in values)
                for name, values in sorted(properties.items())
            },
        }

    if len(registry) != EXPECTED_BLOCKS:
        print(
            f"error: extracted {len(registry)} blocks, expected {EXPECTED_BLOCKS}",
            file=sys.stderr,
        )
        return 2

    out.write_text(json.dumps(registry, indent=2, sort_keys=True) + "\n")
    print(f"{out}: {len(registry)} blocks")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
