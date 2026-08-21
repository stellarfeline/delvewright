#!/usr/bin/env python3
"""Regenerate `crates/compiler/data/item-combat-1.21.11.json` from the pinned MC
1.21.11 item-components summary — the vendored `item id -> combat stats` table the
spec-0023 winnability arithmetic reads (`DW0472`, `DW0473`).

Why it exists: the compiler has to answer "can this kit kill that hostile, and in
how many swings" at build time. Every number it needs is already Mojang's own
data — an item's `minecraft:attribute_modifiers` component carries its
`attack_damage`, `attack_speed`, `armor` and `armor_toughness` contributions —
so the table is EXTRACTED, never hand-maintained. A hand-typed weapon table is
exactly the "invented precision" this codebase refuses (see `DEFAULT_FOLLOW_RANGE`
in `nav.rs` and `MODEL_MARGIN` in `clearance.rs` for the same discipline).

Deterministic, offline once the source is fetched, no dependencies (Python 3
stdlib). Same shape and provenance discipline as `extract-item-stack-sizes.py`.

## Source

Mojang's generated **default item components** report, republished verbatim by
misode/mcmeta — the SAME file `extract-item-stack-sizes.py` reads, so the two
tables can never disagree about which MC pin they describe. Fetch it once:

    curl -sSL -o item_components.min.json \
      https://raw.githubusercontent.com/misode/mcmeta/1.21.11-summary/item_components/data.min.json

Expected SHA-256 of that source (pinned in PROVENANCE.md):
    51b191e13f86813ca02f1498942e5bc235947edb71eb8105a78401670b3665c4

## Transform

For every item, sum the `add_value` modifiers of the four combat attribute types
over the component list, take `minecraft:food`'s `nutrition` as the sustain term,
and emit an entry only when at least one number is non-zero:

    {"minecraft:<id>": {"attack_damage": f, "attack_speed": f,
                        "armor": f, "armor_toughness": f, "nutrition": f}}

`nutrition` is here rather than in a table of its own because sustain is a combat
stat: spec-0023 §2 requires the kit's healing to be non-zero, and "is this item
food" is `minecraft:food`'s presence — Mojang's data, not a curated list.

then `json.dumps(indent=2, sort_keys=True, ensure_ascii=False) + "\n"` — `delvec
fmt` canonical form, because the output is a tracked file inside the
canonical-form sweep (`tools/check-json-canonical.py`).

**Only `add_value` modifiers are summed.** 1.21.11 vanilla expresses every base
weapon/armour stat as `add_value`; a `add_multiplied_*` operation would need the
attribute's base value to resolve, which is not in this report, so it is refused
rather than silently mis-summed. Absence from the table therefore means exactly
one thing: Mojang's data gives that item no combat contribution (a bow's damage
is projectile code, not an attribute — which is why `minecraft:bow` is absent and
the compiler must never read it as "deals no damage"; see `combat.rs`).

    python3 tools/extract-item-combat-stats.py item_components.min.json \
      crates/compiler/data/item-combat-1.21.11.json
"""

import hashlib
import json
import pathlib
import sys

EXPECTED_SOURCE_SHA256 = (
    "51b191e13f86813ca02f1498942e5bc235947edb71eb8105a78401670b3665c4"
)

MODIFIERS = "minecraft:attribute_modifiers"
FOOD = "minecraft:food"

# The four attribute types the winnability arithmetic reads, mapped to the key
# they take in the emitted table.
WANTED = {
    "minecraft:attack_damage": "attack_damage",
    "minecraft:attack_speed": "attack_speed",
    "minecraft:armor": "armor",
    "minecraft:armor_toughness": "armor_toughness",
}


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        sys.stderr.write(
            "usage: extract-item-combat-stats.py <item_components/data.min.json> <out.json>\n"
        )
        return 2
    src_path, out_path = pathlib.Path(argv[1]), pathlib.Path(argv[2])
    raw = src_path.read_bytes()
    got = hashlib.sha256(raw).hexdigest()
    if got != EXPECTED_SOURCE_SHA256:
        sys.stderr.write(
            f"source SHA-256 mismatch\n  expected {EXPECTED_SOURCE_SHA256}\n  got      {got}\n"
            "This table is pinned to MC 1.21.11 (ADR-0009). Re-fetch the 1.21.11-summary\n"
            "file, or — if the MC pin genuinely moved — update the pin here AND in\n"
            "crates/compiler/data/PROVENANCE.md in the same commit.\n"
        )
        return 2

    src = json.loads(raw)
    out: dict[str, dict[str, float]] = {}
    for item_id, components in sorted(src.items()):
        mods = components.get(MODIFIERS) or []
        food = components.get(FOOD) or {}
        stats = {name: 0.0 for name in WANTED.values()}
        stats["nutrition"] = float(food.get("nutrition", 0))
        for mod in mods:
            key = WANTED.get(mod.get("type", ""))
            if key is None:
                continue
            operation = mod.get("operation")
            if operation != "add_value":
                sys.stderr.write(
                    f"{item_id}: {mod.get('type')} uses operation {operation!r}; this\n"
                    "extractor only resolves 'add_value'. Teach it the new operation —\n"
                    "do NOT drop the modifier, which would understate the item.\n"
                )
                return 1
            stats[key] += float(mod.get("amount", 0.0))
        if any(v != 0.0 for v in stats.values()):
            out[f"minecraft:{item_id}"] = stats

    out_path.write_text(json.dumps(out, indent=2, sort_keys=True, ensure_ascii=False) + "\n")
    sys.stderr.write(f"wrote {len(out)} combat/sustain items -> {out_path}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
