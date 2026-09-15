#!/usr/bin/env python3
"""Regenerate `crates/delvec/data/item-equippable-1.21.11.json` — every pinned
1.21.11 item that carries the `minecraft:equippable` component, with the slot it
declares, the equipment asset it draws through, the entities it admits, and the
**kind** of piece it is (spec-0067 §4).

The table is the item half of the fit rule `DW0898` applies: a piece is refused
where the item names another slot (shape 2) or excludes the body (shape 3), and
the kind decides which of a body's shown slots can draw it (shape 1, against the
body table `crates/dsl/data/entity-slots-1.21.11.json`).

Deterministic, offline once the sources are fetched, Python 3 stdlib only.

## Sources

Two records of the pinned game, both republished verbatim by `misode/mcmeta`:

1. Mojang's default item components report, tag `1.21.11-summary` at commit
   `c976eb3b2cfcb9f205171527dec46b266afa3ac9`:

       curl -sSL -o item_components.min.json \\
         https://raw.githubusercontent.com/misode/mcmeta/c976eb3b2cfcb9f205171527dec46b266afa3ac9/item_components/data.min.json

   SHA-256 `51b191e13f86813ca02f1498942e5bc235947edb71eb8105a78401670b3665c4`.

2. The pinned client's equipment assets, tag `1.21.11-assets` at commit
   `c5b876288e0df01b5cd5798434b066ab97eff88c`, the 44 files under
   `assets/minecraft/equipment/`, fetched into one directory. Their digest is
   the SHA-256 of the sorted `sha256sum` listing (`<hex>  <name>` per line),
   `150b585538e2295db9daee16e3fce4c23ac5bcdddc85358208b8e7adadbf78bb`.

## Transform

For every item whose components carry `minecraft:equippable`:
`{slot, asset_id?, allowed_entities? (always a list), kind}`, keyed
`minecraft:<id>`. The kind:

- `wings`  — the asset has a `wings` layer (the elytra);
- `armour` — an asset and a `head` / `chest` / `legs` / `feet` slot
  (`HumanoidArmorLayer.shouldRender`);
- `animal` — an asset and a `body` / `saddle` slot, drawn through the asset's
  `*_body` / `*_saddle` layer;
- `item`   — no asset (the heads, the carved pumpkin, the shield).

`asset_layers` records every asset's layer types, so the body table's layer
types are checked against the assets themselves. Output is `delvec fmt`
canonical form.

Every source digest and every count below is pinned, and a mismatch is refused
by exit status 1: a regenerated table is never quietly different.

    python3 tools/extract-item-equippable.py item_components.min.json \\
      <equipment-dir> crates/delvec/data/item-equippable-1.21.11.json
"""

from __future__ import annotations

import collections
import hashlib
import json
import pathlib
import sys

EXPECTED_ITEMS_SHA256 = "51b191e13f86813ca02f1498942e5bc235947edb71eb8105a78401670b3665c4"
EXPECTED_ASSETS_LIST_SHA256 = "150b585538e2295db9daee16e3fce4c23ac5bcdddc85358208b8e7adadbf78bb"

EQUIPPABLE = "minecraft:equippable"
ARMOUR_SLOTS = {"head", "chest", "legs", "feet"}
ANIMAL_SLOTS = {"body", "saddle"}

# spec-0067 §1 and §4.1, computed from the sources above.
EXPECTED_COUNTS = {
    "items": 84,
    "slots": {"body": 44, "head": 16, "chest": 8, "feet": 7, "legs": 7, "saddle": 1, "offhand": 1},
    "allowed": 45,
    "kinds": {"armour": 29, "wings": 1, "animal": 45, "item": 9},
    "assets": 44,
    "layer_types": 18,
    "animal_layer_types": 15,
}


def assets_list_digest(directory: pathlib.Path) -> tuple[str, dict[str, bytes]]:
    files = {p.name: p.read_bytes() for p in sorted(directory.glob("*.json"))}
    listing = sorted(f"{hashlib.sha256(b).hexdigest()}  {name}\n" for name, b in files.items())
    return hashlib.sha256("".join(listing).encode()).hexdigest(), files


def derive(items_raw: bytes, asset_files: dict[str, bytes]) -> tuple[dict, list[str]]:
    errors: list[str] = []
    asset_layers = {
        "minecraft:" + name.removesuffix(".json"): sorted(json.loads(b)["layers"].keys())
        for name, b in asset_files.items()
    }
    data = json.loads(items_raw)
    items: dict[str, dict] = {}
    for bare, components in sorted(data.items()):
        eq = components.get(EQUIPPABLE)
        if eq is None:
            continue
        item = f"minecraft:{bare}"
        slot = eq["slot"]
        row: dict[str, object] = {"slot": slot}
        asset = eq.get("asset_id")
        allowed = eq.get("allowed_entities")
        if allowed is not None:
            row["allowed_entities"] = [allowed] if isinstance(allowed, str) else list(allowed)
        layers = []
        if asset is not None:
            row["asset_id"] = asset
            if asset not in asset_layers:
                errors.append(f"{item} draws through asset `{asset}`, which the equipment assets do not hold")
            layers = asset_layers.get(asset, [])
        if asset is None:
            kind = "item"
        elif "wings" in layers:
            kind = "wings"
        elif slot in ARMOUR_SLOTS:
            kind = "armour"
        elif slot in ANIMAL_SLOTS:
            kind = "animal"
            suffix = "_body" if slot == "body" else "_saddle"
            if not any(layer.endswith(suffix) for layer in layers):
                errors.append(f"{item} is a `{slot}` piece whose asset `{asset}` has no `*{suffix}` layer")
            if allowed is None:
                errors.append(f"{item} is a `{slot}` piece that names no allowed entities")
        else:
            errors.append(f"{item} has an asset and the slot `{slot}`, which no layer draws")
            kind = "item"
        row["kind"] = kind
        items[item] = row
    return {"asset_layers": asset_layers, "items": items}, errors


def counts(table: dict) -> dict:
    items = table["items"].values()
    layer_types = {t for ls in table["asset_layers"].values() for t in ls}
    return {
        "items": len(table["items"]),
        "slots": dict(collections.Counter(r["slot"] for r in items)),
        "allowed": sum(1 for r in items if "allowed_entities" in r),
        "kinds": dict(collections.Counter(r["kind"] for r in items)),
        "assets": len(table["asset_layers"]),
        "layer_types": len(layer_types),
        "animal_layer_types": sum(1 for t in layer_types if t.endswith(("_body", "_saddle"))),
    }


def refuse(refusals: list[str]) -> int:
    sys.stderr.write("refused — the sources are not the pinned 1.21.11 records:\n")
    for r in refusals:
        sys.stderr.write(f"  {r}\n")
    return 1


def main(argv: list[str]) -> int:
    if len(argv) != 4:
        sys.stderr.write(
            "usage: extract-item-equippable.py <item_components/data.min.json> "
            "<assets/minecraft/equipment dir> <out.json>\n"
        )
        return 2
    src, assets_dir, out_path = pathlib.Path(argv[1]), pathlib.Path(argv[2]), pathlib.Path(argv[3])
    items_raw = src.read_bytes()
    refusals: list[str] = []
    got = hashlib.sha256(items_raw).hexdigest()
    if got != EXPECTED_ITEMS_SHA256:
        refusals.append(f"item components SHA-256 {got}, pinned {EXPECTED_ITEMS_SHA256}")
    list_digest, asset_files = assets_list_digest(assets_dir)
    if list_digest != EXPECTED_ASSETS_LIST_SHA256:
        refusals.append(
            f"equipment-asset list SHA-256 {list_digest} over {len(asset_files)} file(s), "
            f"pinned {EXPECTED_ASSETS_LIST_SHA256}"
        )
    if refusals:
        return refuse(refusals)
    table, errors = derive(items_raw, asset_files)
    refusals.extend(errors)
    measured = counts(table)
    for key, want in EXPECTED_COUNTS.items():
        if measured[key] != want:
            refusals.append(f"count `{key}` is {measured[key]}, pinned {want}")
    if refusals:
        return refuse(refusals)
    out =json.dumps(table, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    out_path.write_text(out, encoding="utf-8")
    sys.stderr.write(
        f"wrote {measured['items']} equippable item(s) over {len(measured['slots'])} slot value(s) "
        f"({measured['allowed']} with an allowed-entity list; kinds {measured['kinds']}) and "
        f"{measured['assets']} asset(s) to {out_path}\n"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
