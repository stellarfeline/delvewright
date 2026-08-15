#!/usr/bin/env python3
"""Extract the pinned 1.21.11 block **default state** table.

`blocks-1.21.11.json` answers "is this property legal on this block". It cannot
answer "what does the game read when the property is not written", and that is a
different question with a different consumer.

A structure template's palette may leave properties out. Vanilla fills them from
the block's default state on load, so the file is legal and the running server
draws the right thing — but every reader that is not a running server has to
work it out, and the only honest way to work it out is to have the table. A
`minecraft:cobblestone_wall` with nothing written is a wall POST (`up=true`,
every side `none`); a reader that guesses "the first legal value" gets `up=false`
and `east=low`, which is a different block.

Source: the SAME `blocks/data.min.json` the property registry comes from, whose
entries are `[every property's legal values, the default state]`. This script
keeps the SECOND element, namespaced and sorted; its sibling keeps the first.

Reproduce:

    curl -sSL -o /tmp/blocks.min.json \\
      https://raw.githubusercontent.com/misode/mcmeta/1.21.11-summary/blocks/data.min.json
    python3 tools/extract-block-defaults.py /tmp/blocks.min.json \\
      crates/dsl/data/block-defaults-1.21.11.json

The source SHA-256 is pinned below and checked, and every extracted default is
checked to be one of that property's own legal values — a table that disagreed
with its sibling would make every completion silently wrong.
"""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path

# `blocks/data.min.json` @ misode/mcmeta tag `1.21.11-summary`
# (commit c976eb3b2cfcb9f205171527dec46b266afa3ac9), retrieved 2026-08-11.
SOURCE_SHA256 = "178a12096f59f863758a6c685e5eb6de38721b376a30a30383e171d0799f3ee7"

# 1.21.11 has exactly this many blocks, and every one of them has a default
# state — a block with no properties has an empty one.
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

    defaults: dict[str, dict[str, str]] = {}
    for block_id, entry in data.items():
        if not isinstance(entry, list) or len(entry) != 2:
            print(f"error: {block_id!r} is not [properties, default]", file=sys.stderr)
            return 2
        properties, default = entry
        if not isinstance(properties, dict) or not isinstance(default, dict):
            print(f"error: {block_id!r} has no [properties, default] pair", file=sys.stderr)
            return 2
        if sorted(properties) != sorted(default):
            print(
                f"error: {block_id!r} names {sorted(properties)} but defaults "
                f"{sorted(default)} — the two halves of the source disagree",
                file=sys.stderr,
            )
            return 2
        for name, value in default.items():
            legal = [str(v) for v in properties[name]]
            if str(value) not in legal:
                print(
                    f"error: {block_id!r} defaults {name}={value!r}, which is not "
                    f"one of {legal}",
                    file=sys.stderr,
                )
                return 2
        defaults[f"minecraft:{block_id}"] = {
            name: str(value) for name, value in sorted(default.items())
        }

    if len(defaults) != EXPECTED_BLOCKS:
        print(
            f"error: extracted {len(defaults)} blocks, expected {EXPECTED_BLOCKS}",
            file=sys.stderr,
        )
        return 2

    out.write_text(json.dumps(defaults, indent=2, sort_keys=True) + "\n")
    with_properties = sum(1 for d in defaults.values() if d)
    print(f"{out}: {len(defaults)} blocks, {with_properties} of them with properties")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
