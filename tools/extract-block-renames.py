#!/usr/bin/env python3
"""Derive the block-id renames Minecraft's DataFixerUpper applies, from Mojang's
own published data.

Writes ``crates/dsl/data/block-renames-1.21.11.json``: an id the pin does not
have -> the id it becomes, plus the greatest ``DataVersion`` at which the old id
still existed.

Why this exists
---------------

The game datafixes every structure ``.nbt`` it loads against the ``DataVersion``
the file declares, so a pre-pin template naming ``minecraft:chain`` loads
``minecraft:iron_chain`` and is correct. Two checks in ``delve-admit audit`` then
disagreed about it: the spelling rule (``DW0734``) passed it as a warning because
the fixer handles it, and the palette allowlist (``DW0730``) refused it in the
next breath, because the allowlist is a list of names AT THE PIN and was being
applied to a name written in an older vocabulary. The allowlist has to judge the
id the game will actually load, which is what this table supplies.

What is derived, and what is not
--------------------------------

Two halves, and only one of them is a judgement call:

* **Which ids disappeared, and when.** Fully derived. The ``block`` array of
  each released version's ``registries/data.min.json`` is Mojang's own complete
  block registry at that version, so "present at version A, absent at version B"
  is a fact, and ``valid_through`` is A's ``DataVersion``. That is deliberately a
  LOWER BOUND on the schema that performs the rename: the fix landed somewhere in
  the development cycle between the two releases, and the repo cannot read the
  fixer schedule (it lives in the game jar). A file at or below ``valid_through``
  certainly pre-dates the fix; a file above it is left unresolved, which fails
  CLOSED.

* **What each disappeared id became.** Derived where the game's own recipe data
  determines it: a crafting recipe whose ingredient side is byte-identical
  across the two versions and whose result changed from the removed id to an id
  the same step added. ``chain`` (nugget/ingot/nugget of iron) and ``iron_chain``
  are that recipe, and the pairing is forced rather than guessed — which matters,
  because the same step added ten chain blocks and the registry's own
  nearest-name suggestion for ``chain`` is ``copper_chain``.

A removal the recipe graph cannot pair is **reported and left out**. That is the
honest state: the audit then still refuses the id under ``DW0730``, a reviewer
sees a name the pin does not have, and nothing is invented. Incompleteness here
costs a false red, never a false pass.

Determinism: sorted keys, no timestamps, no network state in the output.

Usage::

    python3 tools/extract-block-renames.py crates/dsl/data/block-renames-1.21.11.json
    python3 tools/extract-block-renames.py --cache-dir /tmp/mcmeta out.json

The summaries come from the community mirror of Mojang's generated reports
(``misode/mcmeta``, one ``<version>-summary`` branch per release), the same
source ``crates/compiler/data/PROVENANCE.md`` records for every other vendored
table here.
"""

import argparse
import json
import os
import sys
import urllib.request

MCMETA = "https://raw.githubusercontent.com/misode/mcmeta"
PIN_ID = "1.21.11"
PIN_DATA_VERSION = 4671

# The three summary branches this derivation reads, per version.
WANT = {
    "version": "version.json",
    "registries": "registries/data.min.json",
    "recipes": "data/recipe/data.min.json",
}


def fetch(url, dest):
    """Download `url` to `dest` once; return its bytes, or None on 404."""
    if os.path.exists(dest) and os.path.getsize(dest) > 0:
        with open(dest, "rb") as f:
            return f.read()
    try:
        with urllib.request.urlopen(url) as r:
            body = r.read()
    except Exception:
        return None
    os.makedirs(os.path.dirname(dest), exist_ok=True)
    with open(dest, "wb") as f:
        f.write(body)
    return body


def summary(cache, version, what):
    body = fetch(
        f"{MCMETA}/{version}-summary/{WANT[what]}",
        os.path.join(cache, version, what + ".json"),
    )
    return json.loads(body) if body else None


def qualify(name):
    """The registry writes bare ids; recipes and every consumer of this table
    write namespaced ones. One spelling, chosen here, or the two sides never
    meet and every rename reads as undetermined."""
    return name if ":" in name else f"minecraft:{name}"


def ingredient_side(recipe):
    """Everything a recipe says EXCEPT what it produces."""
    r = dict(recipe)
    r.pop("result", None)
    return json.dumps(r, sort_keys=True)


def produced(recipe):
    result = recipe.get("result")
    if isinstance(result, dict):
        return result.get("id") or result.get("item")
    return result if isinstance(result, str) else None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("out", help="where to write the vendored rename table")
    ap.add_argument(
        "--cache-dir",
        default=os.path.join(os.path.dirname(os.path.abspath(__file__)), ".mcmeta-cache"),
        help="where the downloaded summaries are kept between runs",
    )
    args = ap.parse_args()

    manifest = fetch(f"{MCMETA}/summary/versions/data.min.json",
                     os.path.join(args.cache_dir, "versions.json"))
    if manifest is None:
        sys.exit("could not read mcmeta's version manifest")
    # Up to and including the pin. Versions past it describe a game this repo
    # does not run (ADR-0009), and a rename made after the pin would be a rename
    # the pinned server never performs.
    releases = sorted(
        (
            v
            for v in json.loads(manifest)
            if v.get("type") == "release" and v["data_version"] <= PIN_DATA_VERSION
        ),
        key=lambda v: v["data_version"],
    )
    if not releases:
        sys.exit("version manifest lists no releases — binding count would be zero")
    if releases[-1]["id"] != PIN_ID or releases[-1]["data_version"] != PIN_DATA_VERSION:
        sys.exit(
            f"newest release is {releases[-1]['id']} (DataVersion "
            f"{releases[-1]['data_version']}), not the pin {PIN_ID} "
            f"({PIN_DATA_VERSION}) — this table describes the pin (ADR-0009)"
        )

    blocks = {}
    recipes = {}
    for v in releases:
        reg = summary(args.cache_dir, v["id"], "registries")
        blocks[v["id"]] = (
            {qualify(b) for b in reg["block"]} if reg and "block" in reg else None
        )
        rec = summary(args.cache_dir, v["id"], "recipes")
        recipes[v["id"]] = rec

    at_pin = blocks[PIN_ID]
    if not at_pin:
        sys.exit(f"no block registry for the pin {PIN_ID}")

    table = {}
    steps = 0
    removals = 0
    undetermined = []
    for older, newer in zip(releases, releases[1:]):
        ob, nb = blocks[older["id"]], blocks[newer["id"]]
        if ob is None or nb is None:
            continue
        steps += 1
        gone = ob - nb
        arrived = nb - ob
        if not gone:
            continue
        orc, nrc = recipes[older["id"]], recipes[newer["id"]]
        for name in sorted(gone):
            removals += 1
            targets = set()
            if orc and nrc:
                index = {}
                for r in nrc.values():
                    index.setdefault(ingredient_side(r), set()).add(produced(r))
                for r in orc.values():
                    if produced(r) != name:
                        continue
                    targets |= {t for t in index.get(ingredient_side(r), ()) if t in arrived}
            if len(targets) != 1:
                undetermined.append(
                    f"{name} ({older['id']} -> {newer['id']}): "
                    + (f"{len(targets)} candidate(s) {sorted(targets)}" if targets
                       else "no recipe pairs it")
                )
                continue
            table[name] = {
                "to": targets.pop(),
                "valid_through": older["data_version"],
            }

    # A rename chain: an id renamed twice lands on whatever the pin has.
    for name, row in list(table.items()):
        seen = {name}
        while row["to"] in table and row["to"] not in seen:
            seen.add(row["to"])
            row["to"] = table[row["to"]]["to"]

    # Invariants. Each one is a refusal rather than a filter: a row that fails
    # any of them means the derivation is wrong, not that the row is optional.
    for name, row in sorted(table.items()):
        if name in at_pin:
            sys.exit(f"{name} is still a block at {PIN_ID}: it was never renamed away")
        if row["to"] not in at_pin:
            sys.exit(f"{name} -> {row['to']}, which {PIN_ID} does not have")
        if row["valid_through"] >= PIN_DATA_VERSION:
            sys.exit(
                f"{name}: valid_through {row['valid_through']} is not below the pin's "
                f"{PIN_DATA_VERSION}"
            )

    with open(args.out, "w", newline="\n") as f:
        json.dump(table, f, indent=2, sort_keys=True, ensure_ascii=False)
        f.write("\n")

    print(
        f"{len(releases)} released version(s), {steps} consecutive step(s) examined; "
        f"{removals} block id(s) left the registry; {len(table)} rename(s) derived, "
        f"{len(undetermined)} undetermined"
    )
    for name, row in sorted(table.items()):
        print(f"  {name} -> {row['to']} (valid_through {row['valid_through']})")
    for u in undetermined:
        print(f"  UNDETERMINED {u} — left out; the audit refuses the id (DW0730)")
    if steps == 0:
        sys.exit("binding count is zero: no version step was examined")


if __name__ == "__main__":
    main()
