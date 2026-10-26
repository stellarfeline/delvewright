#!/usr/bin/env python3
"""Record one technical event of a `/new-delve` run, when debug mode is on (spec-0122 §4).

A run meets defaults it takes, refusals it repairs, engine limits it works
around and departures from the design the player will see. None of them is a
question for the user. With `DELVEWRIGHT_DEBUG` set (any value but `0`), each
one is an event recorded the moment it happens, as one JSON row appended to
`<campaign>/.out/technical.jsonl` (build output, never committed), and
`debug-report.py` renders the rows into the run's final report. With the flag
unset or `0` — the released default — this tool writes nothing and exits 0, so
the page gives the same instruction either way and no step branches on it.

    "$DELVEWRIGHT_PYTHON" "$DELVEWRIGHT_SKILL/scripts/debug-note.py" <campaign-dir> \\
        --kind default|refusal|limit|workaround|departure --step <step> --what "<one sentence>" \\
        [--from <file holding delvec's output>]

`--step` is the page step: `init`, `1` … `13`, or `9/<place>` for a place.
`--what` is one sentence for the engine developer; for a `refusal` it says the
repair. A `refusal` is handed the file `delvec`'s output was captured into
(`--from`, `… > .out/last.txt 2>&1`), and its code, count and message are read
from that file, never retyped: one row per distinct code, its count the number
of error lines carrying it. A refusal with no `--from`, or whose file carries
no error-severity DW line, is refused (exit 2) — what has no code is a `limit`.

Prints what it recorded on stderr. Standard library only.
"""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import re
import sys

KINDS = ("default", "refusal", "limit", "workaround", "departure")
LOG = pathlib.Path(".out") / "technical.jsonl"

# `delvec`'s two diagnostic spellings: the human line
# (`DW0844 [error] stage path: message`, `DW0301 [error] build: message`) and
# `--json`'s one object per line. Only errors refuse.
HUMAN = re.compile(r"^(DW\d{4}) \[(error|warning)\] (.*)$")


def debug_on(env: dict[str, str] | None = None) -> bool:
    """The flag: unset or `0` is off, anything else is on (spec-0122 §4.1)."""
    value = (os.environ if env is None else env).get("DELVEWRIGHT_DEBUG")
    return value not in (None, "", "0")


def refusals_in(text: str) -> list[tuple[str, int, str]]:
    """Every distinct error code in `text`, in first-seen order, with its count and first message."""
    seen: dict[str, list] = {}
    for line in text.splitlines():
        line = line.strip()
        code = message = None
        if line.startswith("{"):
            try:
                obj = json.loads(line)
            except ValueError:
                obj = None
            if isinstance(obj, dict) and re.fullmatch(r"DW\d{4}", str(obj.get("code", ""))):
                if obj.get("severity", "error") == "error":
                    code, message = obj["code"], str(obj.get("message", ""))
        else:
            m = HUMAN.match(line)
            if m and m.group(2) == "error":
                code, message = m.group(1), m.group(3)
        if code is None:
            continue
        if code in seen:
            seen[code][0] += 1
        else:
            seen[code] = [1, message]
    return [(c, n, msg) for c, (n, msg) in seen.items()]


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("campaign_dir", type=pathlib.Path)
    ap.add_argument("--kind", required=True, choices=KINDS)
    ap.add_argument("--step", required=True)
    ap.add_argument("--what", required=True)
    ap.add_argument("--from", dest="source", type=pathlib.Path)
    args = ap.parse_args(argv)
    if not debug_on():
        return 0
    rows: list[dict] = []
    if args.kind == "refusal":
        if args.source is None:
            print("debug-note: a `refusal` is read from `--from <file>`, the output `delvec` gave", file=sys.stderr)
            return 2
        try:
            text = args.source.read_text(encoding="utf-8", errors="replace")
        except OSError as e:
            print(f"debug-note: cannot read `{args.source}`: {e}", file=sys.stderr)
            return 2
        found = refusals_in(text)
        if not found:
            print(
                f"debug-note: `{args.source}` carries no error-severity DW line; what has no code is a `limit`",
                file=sys.stderr,
            )
            return 2
        for code, count, message in found:
            rows.append(
                {"kind": "refusal", "step": args.step, "what": args.what, "code": code, "count": count, "said": message}
            )
    else:
        rows.append({"kind": args.kind, "step": args.step, "what": args.what})
    log = args.campaign_dir / LOG
    log.parent.mkdir(parents=True, exist_ok=True)
    with log.open("a", encoding="utf-8") as f:
        for row in rows:
            f.write(json.dumps(row, ensure_ascii=False, sort_keys=True) + "\n")
    print(f"debug-note: {len(rows)} row(s) appended to `{log}` ({args.kind}, step {args.step})", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
