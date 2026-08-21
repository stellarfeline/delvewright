#!/usr/bin/env python3
"""Every PackTest project the gate will run, derived from `ci.yml` itself.

WHY THIS EXISTS. A round that changes PackTest emission has to verify it on a
live server, and until this existed the only way to know WHICH servers was to
read the tier-2 job and remember what you found. One round read it, found the
gallery, ran that suite exhaustively — three times, end to end, 97/97 — and
shipped a red. The gate ran TWELVE projects that day; the change broke the one whose
campaign has no cast ledger, which is a shape the gallery does not have. The
verification surface was a strict PREFIX of the gate's, and the red lived
exactly in the difference.

That is `CLAUDE.md`'s truncation-fakes-coverage shape arriving through a project
list rather than a test list, and it fakes coverage in the direction that reads
as a clean pass: every project you DID run was green, and nothing anywhere said
how many there were. It is also the UNRUN shape — "read `ci.yml` and run them
all" is a doc line, and a doc line is not an invocation.

WHAT THIS IS. One authority, not two. The matrix is READ OUT OF `ci.yml`, so
there is no second list to drift and no lockstep checker to own: a project added
to the tier-2 job is covered by the next local run without anyone remembering to
add a line. That is the same "both walks derive from one traversal" rule
`DW0497` states for emitters, applied to the gate.

BINDING, AND WHAT A SHORT COUNT MEANS. The parse states how many projects it
found, and refuses when it finds fewer than `--expect` (the count a caller
believes it is covering) — because the failure mode being guarded is precisely
"the list I ran was shorter than the list that gates me", and a parser that
silently returns three rows would reproduce it one layer down. A zero parse is a
refusal, never an empty pass.

Used by `validation/packtest-all.sh`, which runs the whole matrix. Printing the
matrix (`--list`) is also how a round reports what it covered.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys

REPO = pathlib.Path(__file__).resolve().parents[1]
CI = REPO / ".github/workflows/ci.yml"

# `packtest-run.sh --project <id> [\ --output <tree>]`. The continuation is
# optional because the default tree (`./delve-output`) is spelled by omission —
# which is exactly how the hello-world project hides from a careless reader.
RUN = re.compile(
    r"packtest-run\.sh\s+--project\s+(?P<project>[A-Za-z0-9_.-]+)"
    r"(?:\s*\\\s*\n\s*--output\s+(?P<tree>\S+))?"
)
# `delvec ... build <campaign> ... -o validation/<tree> --prefabs <dir>`, across
# the line continuations the job writes it with. Both spellings of the binary
# appear in the job — `target/debug/delvec` and `cargo run -p delvec --bin delvec
# --` — so the optional `--` is load-bearing: without it the lift-stake row is
# the one that goes missing, which is this file's own failure mode.
BUILD = re.compile(
    r"delvec\s*(?:--)?\s*\\?\s*\n?\s*build\s+(?P<campaign>\S+)\s*\\?\s*\n?\s*"
    r"-o\s+validation/(?P<tree>delve-output\S*)\s+--prefabs\s+(?P<prefabs>\S+)"
)
# A build step that generates its prefabs first (the gallery's own generator).
GEN = re.compile(
    r"cargo run[^\n]*--manifest-path\s+(?P<manifest>\S+)\s*\\?\s*\n?\s*"
    r"--\s+(?P<out>\S+)\s+--skins\s+(?P<skins>\S+)"
)


def matrix(text: str) -> list[dict]:
    """Every (project, tree, campaign, prefabs) the tier-2 job runs, in job order."""
    trees = {m.group("tree"): m for m in BUILD.finditer(text)}
    # A generator is attributed to the build step it shares a `run:` block with,
    # found by the prefab directory that step passes to `delvec`.
    gens = {m.group("out"): m for m in GEN.finditer(text)}

    rows = []
    for m in RUN.finditer(text):
        project = m.group("project")
        # The usage block in the script's own comment is prose, not a step.
        if not project.startswith("dw-"):
            continue
        tree = (m.group("tree") or "./delve-output").lstrip("./")
        b = trees.get(tree)
        if b is None:
            sys.exit(
                f"error: PackTest project `{project}` runs over `{tree}`, and no "
                f"`delvec build … -o validation/{tree}` step was found in ci.yml. "
                "Either the job changed shape or this parser has gone stale — a "
                "row this script cannot build is a row it must not silently drop."
            )
        prefabs = b.group("prefabs")
        g = gens.get(prefabs)
        rows.append(
            {
                "project": project,
                "tree": tree,
                "campaign": b.group("campaign"),
                "prefabs": prefabs,
                "generator": g.group("manifest") if g else None,
                "skins": g.group("skins") if g else None,
            }
        )
    return rows


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--list", action="store_true", help="print the matrix, one row per line")
    ap.add_argument(
        "--expect",
        type=int,
        default=None,
        help="refuse unless exactly this many projects were found",
    )
    args = ap.parse_args()

    rows = matrix(CI.read_text())
    if not rows:
        sys.exit(
            "error: parsed ZERO PackTest projects out of ci.yml. That is a "
            "refusal, not an empty pass — a runner that covers nothing while "
            "reporting success is the defect this file exists to prevent."
        )
    if args.expect is not None and len(rows) != args.expect:
        sys.exit(
            f"error: ci.yml runs {len(rows)} PackTest project(s), not the "
            f"{args.expect} this caller expected. If the job gained a project, "
            "cover it; if it lost one, say so — never lower the number to match."
        )

    if args.list:
        for r in rows:
            gen = f"\t{r['generator']}\t{r['skins']}" if r["generator"] else "\t-\t-"
            print(f"{r['project']}\t{r['tree']}\t{r['campaign']}\t{r['prefabs']}{gen}")
    else:
        print(f"packtest matrix: {len(rows)} project(s) in ci.yml tier 2.")
        for r in rows:
            print(f"  {r['project']:22s} {r['tree']:26s} {r['campaign']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
