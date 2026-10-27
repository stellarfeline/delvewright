#!/usr/bin/env python3
"""Render the run's **Technical decisions and engine limits** section (spec-0122 §4.3).

Reads the rows `debug-note.py` appended to `<campaign>/.out/technical.jsonl`
and, when `DELVEWRIGHT_DEBUG` is on, prints one Markdown section for the engine
developer: a subsection per kind (defaults, refusals, limits, workarounds,
departures), refusals as a table `code · count · where · what was said ·
repair`, and a closing line with the count per kind and the total, computed from
the rows. `--append <GENERATION.md>` appends the same section to that file, so
the record outlives the chat. With the flag unset or `0` — the released default
— it prints nothing, appends nothing and exits 0.

    "$DELVEWRIGHT_PYTHON" "$DELVEWRIGHT_SKILL/scripts/debug-report.py" <campaign-dir> \\
        [--append <campaign-dir>/GENERATION.md]

A row this tool cannot read is refused (exit 2) rather than skipped: a report
that leaves an event out is the defect it exists to prevent. Standard library
only.
"""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import sys

HEADING = "## Technical decisions and engine limits"
KINDS = ("default", "refusal", "limit", "workaround", "departure")
TITLES = {
    "default": "Defaults taken",
    "refusal": "Refusals met",
    "limit": "Engine limits met",
    "workaround": "Workarounds",
    "departure": "Departures the player will see",
}
LOG = pathlib.Path(".out") / "technical.jsonl"


def debug_on(env: dict[str, str] | None = None) -> bool:
    """The flag: unset or `0` is off, anything else is on (spec-0122 §4.1)."""
    value = (os.environ if env is None else env).get("DELVEWRIGHT_DEBUG")
    return value not in (None, "", "0")


def read_rows(path: pathlib.Path) -> list[dict]:
    if not path.is_file():
        return []
    rows: list[dict] = []
    for n, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip():
            continue
        try:
            row = json.loads(line)
        except ValueError as e:
            raise ValueError(f"{path}:{n}: not a JSON row: {e}") from e
        if not isinstance(row, dict) or row.get("kind") not in KINDS:
            raise ValueError(f"{path}:{n}: no `kind` of {', '.join(KINDS)}")
        rows.append(row)
    return rows


def cell(s: object) -> str:
    return str(s).replace("|", "\\|").replace("\n", " ")


def render(rows: list[dict]) -> str:
    """The section, for any list of rows (zero included: a declared zero states its count)."""
    out = [HEADING, ""]
    for kind in KINDS:
        mine = [r for r in rows if r["kind"] == kind]
        out.append(f"### {TITLES[kind]} ({len(mine)})")
        out.append("")
        if not mine:
            out.append("None.")
        elif kind == "refusal":
            out.append("| code | count | where | what was said | repair |")
            out.append("|---|---|---|---|---|")
            for r in mine:
                out.append(
                    f"| `{cell(r.get('code'))}` | {cell(r.get('count'))} | {cell(r.get('step'))} "
                    f"| {cell(r.get('said', ''))} | {cell(r.get('what', ''))} |"
                )
        else:
            for r in mine:
                out.append(f"- step {r.get('step')}: {r.get('what', '')}")
        out.append("")
    per = " · ".join(f"{kind} {sum(1 for r in rows if r['kind'] == kind)}" for kind in KINDS)
    out.append(f"Events: {per} — total {len(rows)}.")
    return "\n".join(out) + "\n"


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("campaign_dir", type=pathlib.Path)
    ap.add_argument("--append", type=pathlib.Path, help="also append the section to this file (GENERATION.md)")
    args = ap.parse_args(argv)
    if not debug_on():
        return 0
    try:
        rows = read_rows(args.campaign_dir / LOG)
    except ValueError as e:
        print(f"debug-report: {e}", file=sys.stderr)
        return 2
    section = render(rows)
    sys.stdout.write(section)
    if args.append is not None:
        prior = args.append.read_text(encoding="utf-8") if args.append.is_file() else ""
        sep = "" if not prior or prior.endswith("\n\n") else ("\n" if prior.endswith("\n") else "\n\n")
        args.append.write_text(prior + sep + section, encoding="utf-8")
    print(f"debug-report: {len(rows)} event(s) rendered", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
