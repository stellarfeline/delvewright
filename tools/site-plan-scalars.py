#!/usr/bin/env python3
"""Count the hand-typed scalars of a site plan, classified creative / procedural.

The instrument behind spec-0059's count criterion. A **scalar** is one JSON leaf
an author types that is a number, or one of the plan's enum-valued strings
(`face`, `cmp`, `axis`, `of`, `role`, `fixture`, and `"open"` as a ceiling). An
id, a reference and a note are not scalars: they name things, they do not
measure them. The envelope is not counted.

Every scalar field of the document is classified by spec-0059 §4, and the
classification is a property of the FIELD, stated once in `FIELDS` below —
`creative` for a judgement the author makes, `procedural` for a value the
compiler derives. A field the table does not name is a refusal, not a zero: a
surface added to the plan is classified here in the same change, or this tool
cannot say what the plan costs to type.

Two forms are known, because the number this measures is a before/after pair:

- `--form old` — the absolute form (`boxes[].min` required, `seams[].at` a
  world coordinate `[along, sill]` or `[x, z]`). `min` and both components of
  `at` are procedural: determined by the graph and the extents once the packing
  rule is fixed.
- `--form new` — the relational form (spec-0059): `min` is an optional pin,
  `at` and `meets` are optional face offsets, the sill does not exist. A pin
  that IS typed is a creative choice and counts as one; a procedural scalar has
  no spelling, so the procedural count of a well-formed new-form plan is zero
  by construction, and the tool asserts it.

Usage:
    python3 tools/site-plan-scalars.py --form old|new <site-plan.json> [...]

Prints one line per field with its count and class, then the totals, then the
binding: how many leaves were examined and how many of them were scalars, so a
document that carries nothing cannot read as cheap.
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter
from pathlib import Path

ENUM_FIELDS = {"face", "cmp", "axis", "of", "role", "fixture", "ceiling"}

# field path (arrays elided as `[]`) -> "creative" | "procedural"
FIELDS_COMMON: dict[str, str] = {
    "region.min[]": "creative",
    "region.extent[]": "creative",
    "datums[].y": "creative",
    "boxes[].extent[]": "creative",
    "boxes[].floor.y": "creative",
    "boxes[].ceiling.clearance": "creative",
    "boxes[].ceiling": "creative",  # the string "open"
    "seams[].face": "creative",
    "seams[].contact.extent[]": "creative",
    "volumes[].region.min[]": "creative",
    "volumes[].region.extent[]": "creative",
    "volumes[].role": "creative",
    "identities[].cmp": "creative",
    "identities[].measure.of": "creative",
    "identities[].measure.axis": "creative",
    "sightlines[].from[]": "creative",
    "sightlines[].to[]": "creative",
    "views[].eye[]": "creative",
    "views[].look_at[]": "creative",
    "lighting.fixture": "creative",
    "lighting.min_light": "creative",
}

FIELDS_BY_FORM: dict[str, dict[str, str]] = {
    "old": {
        **FIELDS_COMMON,
        "boxes[].min[]": "procedural",
        "seams[].at[]": "procedural",
    },
    "new": {
        **FIELDS_COMMON,
        # A pin is a creative choice; an untyped one has no leaf to count.
        "boxes[].min[]": "creative",
        "seams[].at": "creative",
        "seams[].at[]": "creative",
        "seams[].meets": "creative",
        "seams[].meets[]": "creative",
    },
}


def leaves(node, path: str):
    """Every leaf of `node` with its elided field path."""
    if isinstance(node, dict):
        for k, v in node.items():
            yield from leaves(v, f"{path}.{k}" if path else k)
    elif isinstance(node, list):
        for v in node:
            yield from leaves(v, f"{path}[]")
    else:
        yield path, node


def is_scalar(path: str, value) -> bool:
    if isinstance(value, bool):
        return False
    if isinstance(value, (int, float)):
        return True
    field = path.rsplit(".", 1)[-1].rstrip("[]")
    return isinstance(value, str) and field in ENUM_FIELDS


def count(doc: dict, form: str) -> tuple[Counter, Counter, int, int]:
    table = FIELDS_BY_FORM[form]
    per_field: Counter = Counter()
    per_class: Counter = Counter()
    examined = 0
    scalars = 0
    for path, value in leaves(doc["content"], ""):
        examined += 1
        if not is_scalar(path, value):
            continue
        scalars += 1
        cls = table.get(path)
        if cls is None:
            sys.exit(
                f"refused: `{path}` carries a scalar ({value!r}) and spec-0059 §4 "
                f"classifies no such field for the {form} form. Classify it in "
                "tools/site-plan-scalars.py in the change that adds it."
            )
        per_field[path] += 1
        per_class[cls] += 1
    return per_field, per_class, examined, scalars


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    ap.add_argument("--form", choices=sorted(FIELDS_BY_FORM), required=True)
    ap.add_argument("plans", nargs="+", type=Path)
    args = ap.parse_args()
    total_examined = total_scalars = 0
    grand: Counter = Counter()
    for plan in args.plans:
        doc = json.loads(plan.read_text(encoding="utf-8"))
        if doc.get("stage") != "site-plan":
            sys.exit(f"refused: {plan} is not a site-plan document (stage={doc.get('stage')!r})")
        per_field, per_class, examined, scalars = count(doc, args.form)
        print(f"{plan} ({args.form} form)")
        for path, n in sorted(per_field.items()):
            print(f"  {path:<32} {n:>4}  {FIELDS_BY_FORM[args.form][path]}")
        creative = per_class["creative"]
        procedural = per_class["procedural"]
        print(f"  creative {creative}, procedural {procedural}, typed {creative + procedural}")
        if args.form == "new" and procedural:
            sys.exit("refused: a new-form plan typed a procedural scalar; the form has no such spelling")
        total_examined += examined
        total_scalars += scalars
        grand.update(per_class)
    print(
        f"binding: {len(args.plans)} plan(s), {total_examined} leaf(ves) examined, "
        f"{total_scalars} scalar(s) counted — creative {grand['creative']}, "
        f"procedural {grand['procedural']}."
    )
    if total_scalars == 0:
        sys.exit("refused: zero scalars counted — the plan carries nothing to classify")
    return 0


if __name__ == "__main__":
    sys.exit(main())
