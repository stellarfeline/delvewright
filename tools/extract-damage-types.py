#!/usr/bin/env python3
"""Regenerate `crates/compiler/data/damage-types-1.21.11.json` — the vendored
`damage type -> {bypasses_armor, scaling}` table the spec-0023 incoming-damage
arithmetic reads (`DW0473`).

Why it exists: "does this scripted hit kill a full-health player" has exactly two
data-dependent terms, and both are vanilla DATA rather than folklore:

- **armour.** Whether armour points reduce the hit at all is membership of the
  vanilla `#minecraft:bypasses_armor` damage-type tag. The compiler only
  adjudicates lethality for types that bypass armour, because for the rest the
  survived amount depends on what the player happens to be wearing at that beat —
  a number the compiler does not have (kits carry no slots).
- **difficulty.** A damage type's `scaling` field decides whether the Easy/Hard
  multipliers apply at all. The trap this closes: eight of the nine damage types
  the DSL exposes are `when_caused_by_living_non_player`, and the compiler emits
  `damage-players` as a bare `/damage <target> <amount> <type>` with NO attacker —
  so those hits are NOT halved on Easy. Only `minecraft:explosion` scales
  (`always`). Reading the doc-comment formula without reading this field would
  have made every Easy campaign's arithmetic wrong by a factor of two, in the
  lenient direction.

Deterministic, offline once the sources are fetched, no dependencies (Python 3
stdlib). Same provenance discipline as `extract-item-stack-sizes.py`.

## Sources

Both republished verbatim by misode/mcmeta from Mojang's generated data
(`crates/compiler/data/PROVENANCE.md`). Fetch them once:

    curl -sSL -o damage_type.min.json \
      https://raw.githubusercontent.com/misode/mcmeta/1.21.11-summary/data/damage_type/data.min.json
    curl -sSL -o damage_type_tags.min.json \
      https://raw.githubusercontent.com/misode/mcmeta/1.21.11-summary/data/tag/damage_type/data.min.json

Expected SHA-256 (pinned in PROVENANCE.md):
    damage_type       0ce7edc377446ecddfd1c3b74b32e2dc3b248edc4035275134fb821e98a6c7ad
    damage_type tags  794ce6343293660b5f32d6a78f7a374623bb785d18dfc5ce3cbdeb3093b0161d

## Transform

    {"minecraft:<id>": {"bypasses_armor": bool, "scaling": "<scaling>"}}

over every damage type in the registry, then
`json.dumps(indent=2, sort_keys=True, ensure_ascii=False) + "\n"` — `delvec fmt`
canonical form, because the output is a tracked file inside the canonical-form
sweep (`tools/check-json-canonical.py`) and a generator that wrote a form the
sweep rejects would leave no green state. `scaling` is copied verbatim from
Mojang's field; a type missing it is an error, never a defaulted guess.

    python3 tools/extract-damage-types.py damage_type.min.json \
      damage_type_tags.min.json crates/compiler/data/damage-types-1.21.11.json
"""

import hashlib
import json
import pathlib
import sys

# Pinned by content, printed on mismatch so a genuine MC-pin move is a one-line
# update here + in PROVENANCE.md rather than a silent re-vendor.
EXPECTED_TYPES_SHA256 = "0ce7edc377446ecddfd1c3b74b32e2dc3b248edc4035275134fb821e98a6c7ad"
EXPECTED_TAGS_SHA256 = "794ce6343293660b5f32d6a78f7a374623bb785d18dfc5ce3cbdeb3093b0161d"

BYPASSES_ARMOR = "bypasses_armor"
SCALINGS = {"never", "when_caused_by_living_non_player", "always"}


def checked(path: pathlib.Path, expected: str, label: str) -> bytes:
    raw = path.read_bytes()
    got = hashlib.sha256(raw).hexdigest()
    if got != expected:
        sys.stderr.write(
            f"{label} SHA-256 mismatch\n  expected {expected}\n  got      {got}\n"
            "This table is pinned to MC 1.21.11 (ADR-0009). Re-fetch the 1.21.11-summary\n"
            "file, or — if the MC pin genuinely moved — update the pin here AND in\n"
            "crates/compiler/data/PROVENANCE.md in the same commit.\n"
        )
        raise SystemExit(2)
    return raw


def main(argv: list[str]) -> int:
    if len(argv) != 4:
        sys.stderr.write(
            "usage: extract-damage-types.py <damage_type/data.min.json> "
            "<tag/damage_type/data.min.json> <out.json>\n"
        )
        return 2
    types_path, tags_path, out_path = (pathlib.Path(a) for a in argv[1:4])
    types = json.loads(checked(types_path, EXPECTED_TYPES_SHA256, "damage_type"))
    tags = json.loads(checked(tags_path, EXPECTED_TAGS_SHA256, "damage_type tag"))

    bypass = set(tags.get(BYPASSES_ARMOR, {}).get("values", []))
    if not bypass:
        sys.stderr.write(
            "the #minecraft:bypasses_armor tag resolved empty — the source layout "
            "changed; fix the extractor rather than shipping an empty tag.\n"
        )
        return 1

    out: dict[str, dict[str, object]] = {}
    for type_id, body in sorted(types.items()):
        scaling = body.get("scaling")
        if scaling not in SCALINGS:
            sys.stderr.write(
                f"minecraft:{type_id}: scaling {scaling!r} is missing or unknown. "
                "Mojang's data always states it; do NOT default it.\n"
            )
            return 1
        out[f"minecraft:{type_id}"] = {
            "bypasses_armor": f"minecraft:{type_id}" in bypass,
            "scaling": scaling,
        }

    out_path.write_text(json.dumps(out, indent=2, sort_keys=True, ensure_ascii=False) + "\n")
    sys.stderr.write(f"wrote {len(out)} damage types -> {out_path}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
