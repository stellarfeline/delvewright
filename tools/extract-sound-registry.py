#!/usr/bin/env python3
"""Regenerate `crates/compiler/data/sounds-1.21.11.json` from the pinned MC
1.21.11 registries summary — the vendored sound-event id list `delvec` validates
`play-sound`/`narrate.sound` against (spec-0014, `DW0326`).

Deterministic, offline once the source is fetched, no dependencies (Python 3
stdlib). This mirrors the manual transform recorded in
`crates/compiler/data/PROVENANCE.md` for `items-`/`entities-1.21.11.json`, made
repeatable so a future MC bump (ADR-0009 revisit) is one command, not folklore.

## Source

The `sound_event` registry lives inside Mojang's generated registries summary,
republished verbatim by misode/mcmeta (same provenance as the item/entity
registries). Fetch the pinned source once:

    curl -sSL -o registries.min.json \
      https://raw.githubusercontent.com/misode/mcmeta/1.21.11-summary/registries/data.min.json

Expected SHA-256 of that source (pinned in PROVENANCE.md):
    7efb184902cfef62b431bc9826ebcbcde2c23746e5624326ffcf922e15cf28f9

## Transform

`sorted(set("minecraft:" + id for id in registries["sound_event"]))`, then
`json.dumps(indent=2, sort_keys=True, ensure_ascii=False) + "\n"` — identical shape to the item and
entity registries. 1838 ids for 1.21.11.

    python3 tools/extract-sound-registry.py registries.min.json \
      crates/compiler/data/sounds-1.21.11.json
"""

import hashlib
import json
import pathlib
import sys

EXPECTED_SOURCE_SHA256 = (
    "7efb184902cfef62b431bc9826ebcbcde2c23746e5624326ffcf922e15cf28f9"
)


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        sys.stderr.write(
            "usage: extract-sound-registry.py <registries/data.min.json> <out.json>\n"
        )
        return 2
    src_path, out_path = pathlib.Path(argv[1]), pathlib.Path(argv[2])
    raw = src_path.read_bytes()
    got = hashlib.sha256(raw).hexdigest()
    if got != EXPECTED_SOURCE_SHA256:
        sys.stderr.write(
            "warning: source SHA-256 differs from the pinned 1.21.11 registries "
            f"summary\n  expected {EXPECTED_SOURCE_SHA256}\n  got      {got}\n"
            "  (proceeding — verify the version if this is intentional)\n"
        )
    data = json.loads(raw)
    events = data["sound_event"]
    ids = sorted({f"minecraft:{e}" for e in events})
    out = json.dumps(ids, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    out_path.write_text(out, encoding="utf-8")
    sys.stderr.write(f"wrote {len(ids)} sound-event ids to {out_path}\n")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
