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
keeps only the first element, namespaced and sorted:

    {"minecraft:oak_stairs": {"facing": ["north", ...], ...}}

The default state is deliberately dropped: a validator needs to know which
properties and values are legal, and vendoring a second copy of information
nothing reads is a second thing that can go stale.

Reproduce (the same shape as `tools/extract-sound-registry.py`):

    curl -sSL -o /tmp/blocks.min.json \\
      https://raw.githubusercontent.com/misode/mcmeta/1.21.11-summary/blocks/data.min.json
    python3 tools/extract-block-registry.py /tmp/blocks.min.json \\
      crates/dsl/data/blocks-1.21.11.json

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

    registry: dict[str, dict[str, list[str]]] = {}
    for block_id, entry in data.items():
        if not isinstance(entry, list) or not entry:
            print(f"error: {block_id!r} is not [properties, default]", file=sys.stderr)
            return 2
        properties = entry[0]
        if not isinstance(properties, dict):
            print(f"error: {block_id!r} has no property map", file=sys.stderr)
            return 2
        for name, values in properties.items():
            if not isinstance(values, list) or not values:
                print(
                    f"error: {block_id!r} property {name!r} has no values",
                    file=sys.stderr,
                )
                return 2
        registry[f"minecraft:{block_id}"] = {
            name: sorted(str(v) for v in values)
            for name, values in sorted(properties.items())
        }

    if len(registry) != EXPECTED_BLOCKS:
        print(
            f"error: extracted {len(registry)} blocks, expected {EXPECTED_BLOCKS}",
            file=sys.stderr,
        )
        return 2

    out.write_text(json.dumps(registry, indent=2, sort_keys=True, ensure_ascii=False) + "\n")
    print(f"{out}: {len(registry)} blocks")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
