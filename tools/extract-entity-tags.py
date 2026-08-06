#!/usr/bin/env python3
"""Regenerate `crates/compiler/data/entity-tags-1.21.11.json` from the pinned MC
1.21.11 `entity_type` tag summary — Mojang's own answer to questions of the form
"which entity types do X", which the compiler must never re-answer with a table
of its own (`DW0496` daylight burning; CLAUDE.md forbids invented vanilla data,
cf. `nav::DEFAULT_FOLLOW_RANGE`, `DW0475`).

Deterministic, offline once the source is fetched, no dependencies (Python 3
stdlib). Same shape and provenance as `tools/extract-sound-registry.py`.

## Source

Vanilla's built-in `entity_type` tags, republished verbatim by misode/mcmeta
(same mirror of Mojang's generated reports as the item/entity registries).
Fetch the pinned source once:

    curl -sSL -o entity-type-tags.min.json \\
      https://raw.githubusercontent.com/misode/mcmeta/1.21.11-summary/data/tag/entity_type/data.min.json

Expected SHA-256 of that source (pinned in PROVENANCE.md):
    5523f45b7ddb178cd9f8bbe998458cc070910a74bc6c551a37b9279f5d73f844

## Transform

`{tag: sorted(values)}` over every tag, then
`json.dumps(indent=2, sort_keys=True) + "\\n"`. Values are already namespaced in
the source (`minecraft:zombie`); tag KEYS are bare in the source and are
namespaced here (`minecraft:burn_in_daylight`) so a lookup reads exactly like
the `#minecraft:<tag>` a datapack would write. 46 tags for 1.21.11.

    python3 tools/extract-entity-tags.py entity-type-tags.min.json \\
      crates/compiler/data/entity-tags-1.21.11.json
"""

import hashlib
import json
import pathlib
import sys

EXPECTED_SOURCE_SHA256 = (
    "5523f45b7ddb178cd9f8bbe998458cc070910a74bc6c551a37b9279f5d73f844"
)


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        sys.stderr.write(
            "usage: extract-entity-tags.py "
            "<data/tag/entity_type/data.min.json> <out.json>\n"
        )
        return 2
    src_path, out_path = pathlib.Path(argv[1]), pathlib.Path(argv[2])
    raw = src_path.read_bytes()
    got = hashlib.sha256(raw).hexdigest()
    if got != EXPECTED_SOURCE_SHA256:
        sys.stderr.write(
            "warning: source SHA-256 differs from the pinned 1.21.11 entity_type "
            f"tag summary\n  expected {EXPECTED_SOURCE_SHA256}\n  got      {got}\n"
            "  (proceeding — verify the version if this is intentional)\n"
        )
    data = json.loads(raw)
    tags = {
        f"minecraft:{tag}": sorted(set(body["values"])) for tag, body in data.items()
    }
    out = json.dumps(tags, indent=2, sort_keys=True) + "\n"
    out_path.write_text(out, encoding="utf-8")
    sys.stderr.write(f"wrote {len(tags)} entity_type tags to {out_path}\n")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
