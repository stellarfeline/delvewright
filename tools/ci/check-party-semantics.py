#!/usr/bin/env python3
"""Every surface the DSL declares states what it means for a party (spec-0141 §2).

A delve is played by one to four players, and a creator never writes how many.
So each thing a creator can write must already mean something definite for a
party: who is credited, who is told, who is moved. This gate holds that
statement for every surface unit and refuses a unit that has none.

## The denominator

The units are the ones the gallery coverage gate enumerates, by the same walk
of the same document: `gallery_units.Enumerator` over `delvec schema --stage
all`. There is no second list of surfaces here.

## The record

`tools/ci/party-semantics.json` holds the classes and the rows. A unit
takes the first row that names it, most specific first:

1. `units[<unit id>]`            — one property or variant;
2. `units[<owner>]`              — a declaring type with its variant
                                   (`QuestEffect::give-item`);
3. `types[<base type>]`          — the type before any `::variant` / `.field`;
4. `stages`                      — when every stage the unit appears in is a
                                   stage that emits nothing a player meets.

## What is refused

* a unit no row reaches (a new surface arrived without its party meaning);
* a row that reaches no unit (the surface it described is gone);
* a row whose class the record does not define.

## Binding

Every run prints the unit count, the count per class, and how many rows of each
kind bound. `--index <file>` writes the per-class table the reference page
links to.
"""

from __future__ import annotations

import argparse
import collections
import json
import pathlib
import subprocess
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import gallery_units  # noqa: E402

ROOT = pathlib.Path(__file__).resolve().parents[2]
RECORD = ROOT / "tools" / "ci" / "party-semantics.json"


def base_type(owner: str) -> str:
    return owner.split("::", 1)[0].split(".", 1)[0]


def classify(units: dict, record: dict):
    """unit id -> (class, kind of row, row key); plus the rows that bound."""
    out: dict[str, tuple[str, str, str]] = {}
    unreached: list[str] = []
    used: dict[str, set[str]] = {"units": set(), "types": set(), "stages": set()}
    for uid, unit in sorted(units.items()):
        row = None
        for kind, key in (
            ("units", uid),
            ("units", unit.owner),
            ("types", base_type(unit.owner)),
        ):
            if key in record[kind]:
                row = (record[kind][key]["class"], kind, key)
                break
        if row is None and unit.stages and all(s in record["stages"] for s in unit.stages):
            row = ("no-runtime", "stages", ",".join(unit.stages))
            used["stages"].update(unit.stages)
        if row is None:
            unreached.append(uid)
            continue
        if row[1] != "stages":
            used[row[1]].add(row[2])
        out[uid] = row
    return out, unreached, used


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    src = ap.add_mutually_exclusive_group(required=True)
    src.add_argument("--delvec", help="the delvec binary whose schema export is enumerated")
    src.add_argument("--schema", help="a saved `delvec schema --stage all` export")
    ap.add_argument("--record", default=str(RECORD))
    ap.add_argument("--index", help="write the per-class table here (markdown)")
    args = ap.parse_args()

    if args.delvec:
        r = subprocess.run(
            [args.delvec, "schema", "--stage", "all"], capture_output=True, text=True
        )
        if r.returncode != 0:
            print(f"check-party-semantics: `delvec schema --stage all` failed:\n{r.stderr}")
            return 2
        export = json.loads(r.stdout)
    else:
        export = json.loads(pathlib.Path(args.schema).read_text())
    record = json.loads(pathlib.Path(args.record).read_text())
    units = gallery_units.Enumerator(export).run()

    problems: list[str] = []
    for kind in ("units", "types"):
        for key, row in record[kind].items():
            if row["class"] not in record["classes"]:
                problems.append(f"row `{kind}[{key}]` names class `{row['class']}`, which the record does not define")
            for field in ("semantics", "evidence"):
                if not row.get(field):
                    problems.append(f"row `{kind}[{key}]` states no `{field}`")
    rows, unreached, used = classify(units, record)
    for uid in unreached:
        problems.append(
            f"unit `{uid}` has no party meaning: add a row for it, its owner or its type to "
            f"{pathlib.Path(args.record).name} (who is credited, who is told, who is moved, for 1–4 players)"
        )
    for kind in ("units", "types", "stages"):
        for key in sorted(set(record[kind]) - used[kind]):
            problems.append(f"row `{kind}[{key}]` reaches no unit of the schema export")

    per_class = collections.Counter(c for c, _, _ in rows.values())
    per_kind = collections.Counter(k for _, k, _ in rows.values())
    runtime = sum(n for c, n in per_class.items() if c not in ("no-runtime", "parameter"))
    print(
        f"party semantics: {len(rows)} of {len(units)} surface unit(s) classified "
        f"({per_kind['units']} by a unit row, {per_kind['types']} by a type row, "
        f"{per_kind['stages']} by a stage row); {runtime} with a party meaning of their own, "
        f"{per_class['parameter']} parameter(s) judged with their holder, "
        f"{per_class['no-runtime']} with no runtime behaviour"
    )
    for cls in record["classes"]:
        print(f"  {cls}: {per_class[cls]}")

    if args.index:
        lines = [
            "| Class | Units | Meaning |",
            "|---|---|---|",
        ]
        for cls, meaning in record["classes"].items():
            lines.append(f"| `{cls}` | {per_class[cls]} | {meaning} |")
        lines.append(f"| total | {len(units)} | |")
        lines.append("")
        lines.append("| Surface | Units | Class | For 1–4 players | Evidence |")
        lines.append("|---|---|---|---|---|")
        counts = collections.Counter((k, key) for _, k, key in rows.values())
        for kind in ("types", "units"):
            for key in sorted(record[kind]):
                row = record[kind][key]
                if row["class"] in ("no-runtime", "parameter"):
                    continue
                lines.append(
                    f"| `{key}` | {counts[(kind, key)]} | `{row['class']}` | {row['semantics']} | {row['evidence']} |"
                )
        pathlib.Path(args.index).write_text("\n".join(lines) + "\n")

    if problems:
        print(f"check-party-semantics: {len(problems)} problem(s):")
        for p in problems:
            print(f"  - {p}")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
