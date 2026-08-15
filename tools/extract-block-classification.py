#!/usr/bin/env python3
"""Derive every 1.21.11 block's FORM and material FAMILY from pinned vanilla data.

`tools/block-appearance.py` measures what a block looks like; it needs the jar.
This derives what a block *is* — its shape class and the material group it is
derived from — and needs no jar at all, so the result is vendored beside the
other pinned game data and is available in CI.

Two axes, both from Mojang's own data rather than from name morphology
(`packed_mud`/`mud_bricks` and `end_stone`/`stone` both mis-merge under
stem-matching, in opposite directions):

**form** — the vanilla block tag that names the shape: `#slabs`, `#stairs`,
`#walls`, `#fences`, `#doors`, `#trapdoors`, `#buttons`, `#pressure_plates`,
`#all_signs`. Tags are resolved transitively (`#logs` is a tag of tags). A block
in none of them is `block`. `pane` has no vanilla tag and is instead read off the
blockstate property signature `{east,north,south,waterlogged,west}`, which is
exactly the connection-model panes and bars share and nothing else has.

**family** — connected components of the derivation graph, one edge per recipe
that turns **a single block stock** into another block:

  - `stonecutting`: ingredient → result,
  - `smelting` / `blasting` / `smoking` / `campfire_cooking`: ingredient → result,
  - crafting (`shaped`, `shapeless`, `transmute`): ingredient → result when the
    recipe has **exactly one distinct ingredient and that ingredient is a block**.

A **tag** ingredient is never a block: `crafting_table` is four `#planks`, and
treating that as a block edge welds every wood species in the game into one
component. The single-stock rule is what keeps a *compound* from reading as a
derivation — `granite` is `diorite` + `quartz` and `mossy_stone_bricks` is
`stone_bricks` + `vine`, so neither joins its second ingredient's family.
Loosening it to "exactly one block-valued ingredient among any others" does merge
the 16 stained glasses and pulls the whole stone group into one 41-member
component, which is a different (and much coarser) answer; the strict rule is the
one shipped and `--loose` measures the other.

`concrete` has no recipe at all (powder meets water in the world), so it is a
family of one whatever the rule. Such gaps are reported by `--report`, never
closed by name morphology.

A family's name is the lexicographically smallest member, so the naming is a
property of the component and not of the order the edges were walked.

**On the family-shaped tags.** spec-0035 §3.4 recommends unioning the graph with
`#planks`, `#logs`, `#wool`, `#terracotta`, `#stone_bricks`, `#sand`, `#dirt`,
`#leaves` and `#copper`. Measured, that union takes the largest family from 20 to
**87** — `#planks` alone reaches 87 and `#logs` 55, because a species' planks
already reach its stairs, slabs, doors and buttons through the recipe graph, so
welding the twelve species together welds everything downstream of them. The
purely-derived table is what ships; `--family-tags` reproduces the measurement.

Reproduce (the two sources are the same misode/mcmeta 1.21.11 summary the rest of
`crates/compiler/data/` comes from — see PROVENANCE.md):

    python3 tools/extract-block-classification.py \\
        <data/tag/block/data.min.json> <data/recipe/data.min.json> \\
        crates/compiler/data/block-classification-1.21.11.json

The source SHA-256s are pinned below and checked before anything is derived.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
REGISTRY = REPO / "crates" / "dsl" / "data" / "blocks-1.21.11.json"

# misode/mcmeta @ 1.21.11-summary (PROVENANCE.md's ref/commit), retrieved 2026-08-13.
SOURCE_SHA256 = {
    "tags": "ff73a0c7f08cb8276a48daa39c104a34d79f0aebd872b37de9c9dc137d49082f",
    "recipes": "811e914cf45fc801146103442811285342327a6bb2f46641a58120a131e31918",
}
EXPECTED_BLOCKS = 1166

# Shape tags, most specific first — a block is reported under the first that
# claims it. Every one is a real 1.21.11 block tag; a missing one is an error,
# not a silently empty form.
FORM_TAGS: tuple[tuple[str, str], ...] = (
    ("slab", "slabs"),
    ("stair", "stairs"),
    ("wall", "walls"),
    ("fence", "fences"),
    ("door", "doors"),
    ("trapdoor", "trapdoors"),
    ("button", "buttons"),
    ("pressure_plate", "pressure_plates"),
    ("sign", "all_signs"),
)

# Vanilla has no `#panes` tag. A pane's blockstate is its connection model, and
# this exact property set is shared by glass panes and iron bars and by nothing
# else in 1.21.11 (asserted by the tests).
PANE_PROPERTIES = frozenset({"east", "north", "south", "waterlogged", "west"})

# The tags spec-0035 §3.4 recommends unioning in. NOT applied by default — see
# the module docstring for the measurement that decided it.
FAMILY_TAGS: tuple[str, ...] = (
    "planks",
    "logs",
    "wool",
    "terracotta",
    "stone_bricks",
    "sand",
    "dirt",
    "leaves",
    "copper",
)

COOKING_TYPES = frozenset(
    {
        "minecraft:smelting",
        "minecraft:blasting",
        "minecraft:smoking",
        "minecraft:campfire_cooking",
    }
)
CRAFTING_TYPES = frozenset(
    {
        "minecraft:crafting_shaped",
        "minecraft:crafting_shapeless",
        "minecraft:crafting_transmute",
    }
)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def namespaced(value: str) -> str:
    return value if ":" in value else f"minecraft:{value}"


def resolve_tags(raw: dict) -> dict[str, set[str]]:
    """Every block tag flattened to its transitive block ids.

    `#logs` is three tag references and no blocks at all, so a form or family
    read off the unresolved values sees an empty group and reports a green
    nothing.
    """
    resolved: dict[str, set[str]] = {}
    visiting: set[str] = set()

    def walk(name: str) -> set[str]:
        if name in resolved:
            return resolved[name]
        if name in visiting:  # vanilla has no tag cycles; refuse rather than hang
            raise SystemExit(f"tag cycle at #{name}")
        visiting.add(name)
        out: set[str] = set()
        for value in raw.get(name, {}).get("values", []):
            if isinstance(value, dict):
                value = value.get("id", "")
            if not value:
                continue
            if value.startswith("#"):
                out |= walk(value[1:].removeprefix("minecraft:"))
            else:
                out.add(namespaced(value))
        visiting.discard(name)
        resolved[name] = out
        return out

    for name in raw:
        walk(name)
    return resolved


def ingredient_ids(value) -> list[str]:
    """Every concrete id an ingredient slot names; a tag reference names none."""
    if isinstance(value, str):
        return [] if value.startswith("#") else [namespaced(value)]
    if isinstance(value, list):
        out: list[str] = []
        for item in value:
            out += ingredient_ids(item)
        return out
    if isinstance(value, dict):
        for key in ("item", "id", "tag"):
            if key in value:
                return ingredient_ids(value[key])
    return []


def recipe_ingredients(recipe: dict) -> list[str]:
    kind = recipe.get("type", "")
    if kind in COOKING_TYPES:
        return ingredient_ids(recipe.get("ingredient"))
    if kind == "minecraft:stonecutting":
        return ingredient_ids(recipe.get("ingredient"))
    if kind == "minecraft:crafting_transmute":
        # BOTH halves: `input` is the thing being recoloured, `material` the dye.
        # Reporting only the input would make every transmute look like a
        # single-stock derivation, which is the one thing the rule below tests.
        return ingredient_ids(recipe.get("input")) + ingredient_ids(
            recipe.get("material")
        )
    if kind == "minecraft:crafting_shapeless":
        out: list[str] = []
        for item in recipe.get("ingredients", []):
            out += ingredient_ids(item)
        return out
    if kind == "minecraft:crafting_shaped":
        out = []
        for item in recipe.get("key", {}).values():
            out += ingredient_ids(item)
        return out
    return []


def recipe_result(recipe: dict) -> str | None:
    result = recipe.get("result")
    if isinstance(result, dict) and "id" in result:
        return namespaced(result["id"])
    if isinstance(result, str):
        return namespaced(result)
    return None


class Union:
    def __init__(self, items):
        self.parent = {i: i for i in items}

    def find(self, a: str) -> str:
        root = a
        while self.parent[root] != root:
            root = self.parent[root]
        while self.parent[a] != root:
            self.parent[a], a = root, self.parent[a]
        return root

    def join(self, a: str, b: str) -> None:
        ra, rb = self.find(a), self.find(b)
        if ra != rb:
            # Point at the lexicographically smaller root so the structure never
            # depends on edge order (ADR-0006).
            lo, hi = (ra, rb) if ra < rb else (rb, ra)
            self.parent[hi] = lo


def derive(
    registry: dict,
    tags_raw: dict,
    recipes: dict,
    *,
    family_tags: tuple[str, ...] = (),
    loose: bool = False,
) -> dict:
    blocks = sorted(registry)
    block_set = set(blocks)
    tags = resolve_tags(tags_raw)

    for _, tag in FORM_TAGS:
        if tag not in tags:
            raise SystemExit(f"form tag #{tag} is absent from the pinned tag data")
    for tag in family_tags:
        if tag not in tags:
            raise SystemExit(f"family tag #{tag} is absent from the pinned tag data")

    forms: dict[str, str] = {}
    for block in blocks:
        form = "block"
        for name, tag in FORM_TAGS:
            if block in tags[tag]:
                form = name
                break
        else:
            props = registry[block]
            if isinstance(props, dict) and PANE_PROPERTIES == frozenset(props):
                form = "pane"
        forms[block] = form

    union = Union(blocks)
    edges = 0
    for _, recipe in sorted(recipes.items()):
        result = recipe_result(recipe)
        if result is None or result not in block_set:
            continue
        kind = recipe.get("type", "")
        if kind not in CRAFTING_TYPES and kind not in COOKING_TYPES and kind != "minecraft:stonecutting":
            continue
        every = list(dict.fromkeys(recipe_ingredients(recipe)))
        ins = [i for i in every if i in block_set]
        if kind in CRAFTING_TYPES:
            # A single block stock, and nothing else in the recipe. `granite` is
            # `diorite` + `quartz`; under `--loose` the lone block-valued
            # ingredient still counts and granite joins diorite's family.
            if len(ins) != 1 or (not loose and len(every) != 1):
                continue
        if not ins:
            continue
        for source in ins:
            if source != result:
                union.join(source, result)
                edges += 1

    tag_edges = 0
    for tag in family_tags:
        members = sorted(m for m in tags[tag] if m in block_set)
        for member in members[1:]:
            union.join(members[0], member)
            tag_edges += 1

    groups: dict[str, list[str]] = {}
    for block in blocks:
        groups.setdefault(union.find(block), []).append(block)

    families = {}
    for members in groups.values():
        name = min(members)
        for member in members:
            families[member] = name

    return {
        "version": "1.21.11",
        "blocks": {
            block: {"family": families[block], "form": forms[block]} for block in blocks
        },
        "stats": {
            "blocks": len(blocks),
            "families": len(groups),
            "multi_member_families": sum(1 for g in groups.values() if len(g) > 1),
            "blocks_in_multi_member_families": sum(
                len(g) for g in groups.values() if len(g) > 1
            ),
            "largest_family": max(len(g) for g in groups.values()),
            "recipe_edges": edges,
            "family_tag_edges": tag_edges,
        },
    }


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("tags", type=Path, help="mcmeta data/tag/block/data.min.json")
    ap.add_argument("recipes", type=Path, help="mcmeta data/recipe/data.min.json")
    ap.add_argument("out", type=Path, help="where to write the derived table")
    ap.add_argument(
        "--report", action="store_true", help="print the largest families and exit"
    )
    ap.add_argument(
        "--family-tags",
        action="store_true",
        help="also union spec-0035 §3.4's family-shaped tags (a MEASUREMENT switch: "
        "it takes the largest family from 20 to 87 — see the module docstring)",
    )
    ap.add_argument(
        "--loose",
        action="store_true",
        help="count a crafting recipe with one block-valued ingredient among others "
        "(a MEASUREMENT switch: granite joins diorite, largest family becomes 41)",
    )
    args = ap.parse_args(argv[1:])

    for label, path in (("tags", args.tags), ("recipes", args.recipes)):
        got = sha256(path)
        want = SOURCE_SHA256[label]
        if got != want:
            print(
                f"error: {path} sha256 {got}\n"
                f"       expected {want} (misode/mcmeta 1.21.11-summary).\n"
                f"       A different source is a different game version — update the pin "
                f"deliberately, do not derive from an unpinned file.",
                file=sys.stderr,
            )
            return 2

    registry = json.loads(REGISTRY.read_text())
    if len(registry) != EXPECTED_BLOCKS:
        print(
            f"error: {REGISTRY.name} has {len(registry)} blocks, expected {EXPECTED_BLOCKS}",
            file=sys.stderr,
        )
        return 2

    table = derive(
        registry,
        json.loads(args.tags.read_text()),
        json.loads(args.recipes.read_text()),
        family_tags=FAMILY_TAGS if args.family_tags else (),
        loose=args.loose,
    )

    if args.report:
        by_family: dict[str, list[str]] = {}
        for block, row in table["blocks"].items():
            by_family.setdefault(row["family"], []).append(block)
        biggest = sorted(by_family.items(), key=lambda kv: (-len(kv[1]), kv[0]))[:15]
        for name, members in biggest:
            print(f"{len(members):4}  {name}")
        print(json.dumps(table["stats"], indent=2, sort_keys=True))
        return 0

    args.out.write_text(json.dumps(table, indent=2, sort_keys=True) + "\n")
    print(f"wrote {args.out} — {json.dumps(table['stats'], sort_keys=True)}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
