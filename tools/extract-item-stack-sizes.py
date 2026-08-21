#!/usr/bin/env python3
"""Regenerate `crates/compiler/data/item-stack-sizes-1.21.11.json` from the pinned
MC 1.21.11 item-components summary — the vendored `item id -> max stack size`
table `delvec` validates single-slot fills against (`DW0436`).

Why it exists: `item replace block … container.<n> with <item> <count>` fails
**silently** when `count` exceeds the item's `minecraft:max_stack_size` (rabbit
stew caps at 1, so `count: 2` puts nothing in the chest). That is the same
silent-failure class `DW0431` exists for, so the cap has to be known at build
time — and known from Mojang's own data, never a hand-maintained table that
rots at the next MC pin.

Deterministic, offline once the source is fetched, no dependencies (Python 3
stdlib). Same shape and provenance discipline as `extract-sound-registry.py`.

## Source

Mojang's generated **default item components** report, republished verbatim by
misode/mcmeta (same provenance as the item/entity/sound registries — see
`crates/compiler/data/PROVENANCE.md`). Fetch the pinned source once:

    curl -sSL -o item_components.min.json \
      https://raw.githubusercontent.com/misode/mcmeta/1.21.11-summary/item_components/data.min.json

Expected SHA-256 of that source (pinned in PROVENANCE.md):
    51b191e13f86813ca02f1498942e5bc235947edb71eb8105a78401670b3665c4

## Transform

`{"minecraft:" + id: components["minecraft:max_stack_size"]}` over every item,
then `json.dumps(indent=2, sort_keys=True, ensure_ascii=False) + "\n"` — `delvec
fmt` canonical form, because the output is a tracked file inside the
canonical-form sweep (`tools/check-json-canonical.py`). Every 1.21.11 item declares
the component explicitly, so nothing is defaulted or inferred here — a missing
one is an error, not a silent 64.

    python3 tools/extract-item-stack-sizes.py item_components.min.json \
      crates/compiler/data/item-stack-sizes-1.21.11.json
"""

import hashlib
import json
import pathlib
import sys

EXPECTED_SOURCE_SHA256 = (
    "51b191e13f86813ca02f1498942e5bc235947edb71eb8105a78401670b3665c4"
)

MAX_STACK_SIZE = "minecraft:max_stack_size"


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        sys.stderr.write(
            "usage: extract-item-stack-sizes.py <item_components/data.min.json> <out.json>\n"
        )
        return 2
    src_path, out_path = pathlib.Path(argv[1]), pathlib.Path(argv[2])
    raw = src_path.read_bytes()
    got = hashlib.sha256(raw).hexdigest()
    if got != EXPECTED_SOURCE_SHA256:
        sys.stderr.write(
            "warning: source SHA-256 differs from the pinned 1.21.11 item-components "
            f"summary\n  expected {EXPECTED_SOURCE_SHA256}\n  got      {got}\n"
            "  (proceeding — verify the version if this is intentional)\n"
        )
    data = json.loads(raw)
    sizes: dict[str, int] = {}
    missing: list[str] = []
    for item, components in sorted(data.items()):
        size = components.get(MAX_STACK_SIZE)
        if not isinstance(size, int):
            missing.append(item)
            continue
        sizes[f"minecraft:{item}"] = size
    if missing:
        sys.stderr.write(
            f"error: {len(missing)} item(s) declare no integer `{MAX_STACK_SIZE}`: "
            f"{', '.join(missing[:10])}\n"
            "  Refusing to default them — the cap must come from Mojang's data.\n"
        )
        return 1
    out = json.dumps(sizes, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    out_path.write_text(out, encoding="utf-8")
    sys.stderr.write(f"wrote {len(sizes)} item stack sizes to {out_path}\n")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
