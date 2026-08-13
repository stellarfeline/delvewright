#!/usr/bin/env python3
"""Every judged verdict in a trial record declares what bounded it.

A trial's rubric answers are judgements, and a judgement is only as good as the
instrument that took the picture. Trial 0001 answered R1 `partial` for run 1 and
said, in the same section, that the renderer had no camera an author could aim
and that this "alone is the whole of R1's `partial`". The disclaimer was three
paragraphs away from the verdict; the verdict is what later rounds cite. When an
aimable camera arrived and the same delivered bytes were re-photographed
square-on, the answer was `yes` — the trial had understated its own result for
as long as the record existed, and nothing in the toolchain could say so.

So: a verdict is `artifact-bound` (the instrument could frame the thing being
judged, and the answer is about the artifact) or `instrument-bound` (it could
not, and the answer is partly about the tooling — in which case the blocker is
named, so it can be fixed and the verdict re-taken). The declaration lives beside
the verdict, in the same file, and this gate is what makes it exist.

Why a check and not a line in the methodology doc: a doc line is not an
invocation (CLAUDE.md). The event guarded here is "a trial record states a
judged verdict", and its entry points are enumerable — every `trial-*.md` under
docs/trials/, every `## Run N — result` section in it, every rubric row that
carries a bolded verdict. A record cannot gain a run, or a rubric answer,
without gaining the declaration.

Deterministic, offline, stdlib-only python3. States its binding counts; a trial
record that yields zero verdicts is a finding, not a pass, because the most
likely cause is that the rubric table was reformatted out from under this
parser.
"""

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
TRIALS = ROOT / "docs" / "trials"

RUN_HEADING = re.compile(r"^##\s+Run\s+(\d+)\s+—\s+result\s*$")
BOUNDS_HEADING = re.compile(r"^##\s+Instrument bounds\b")
VERDICT = re.compile(r"\*\*(yes|partial|no)\*\*", re.IGNORECASE)
RUBRIC_CELL = re.compile(r"^(R\d+)\b")
BOUNDS_CELL = re.compile(r"^(R\d+)\s+run\s+(\d+)$")
INSTRUMENT_BOUND = re.compile(r"^instrument-bound\s+—\s+\S.*$")


def cells(line):
    """The cells of a markdown table row, or None if the line is not one."""
    s = line.strip()
    if not s.startswith("|") or not s.endswith("|"):
        return None
    parts = [c.strip() for c in s[1:-1].split("|")]
    if all(set(c) <= set("-: ") for c in parts):
        return None  # the ---|--- separator
    return parts


def audit(path):
    """(problems, verdict_pairs, bounds_rows) for one trial record."""
    problems = []
    lines = path.read_text(encoding="utf-8").splitlines()

    runs = [m.group(1) for line in lines if (m := RUN_HEADING.match(line))]
    rubric_ids, bounds = [], {}
    in_bounds = False

    for line in lines:
        if line.startswith("## "):
            in_bounds = bool(BOUNDS_HEADING.match(line))
        row = cells(line)
        if row is None or not row:
            continue
        head = RUBRIC_CELL.match(row[0])
        if not head:
            continue
        if in_bounds:
            b = BOUNDS_CELL.match(row[0])
            if not b:
                problems.append(
                    f"{path.name}: instrument-bounds row `{row[0]}` is not of the "
                    f"form `R<n> run <k>`"
                )
                continue
            if len(row) < 3 or not row[2]:
                problems.append(
                    f"{path.name}: {row[0]} declares no instrument in its "
                    f"`Judged from` cell"
                )
            bound = row[1] if len(row) > 1 else ""
            if bound != "artifact-bound" and not INSTRUMENT_BOUND.match(bound):
                problems.append(
                    f"{path.name}: {row[0]} is bound `{bound}` — must be exactly "
                    f"`artifact-bound`, or `instrument-bound — <named blocker>`"
                )
            bounds[(b.group(1), b.group(2))] = bound
        elif any(VERDICT.search(c) for c in row[1:]):
            rubric_ids.append(head.group(1))

    rubric_ids = sorted(set(rubric_ids))
    if not runs:
        problems.append(f"{path.name}: no `## Run <n> — result` section")
    if not rubric_ids:
        problems.append(
            f"{path.name}: zero rubric verdicts found — a trial record with no "
            f"judged verdict is a finding, not a pass"
        )

    required = {(r, run) for r in rubric_ids for run in runs}
    for key in sorted(required - set(bounds)):
        problems.append(
            f"{path.name}: {key[0]} run {key[1]} is judged but declares no "
            f"instrument bound — add a row to `## Instrument bounds`"
        )
    for key in sorted(set(bounds) - required):
        problems.append(
            f"{path.name}: `{key[0]} run {key[1]}` declares an instrument bound "
            f"but no such verdict is judged in this record"
        )
    return problems, required, bounds


def main():
    if not TRIALS.is_dir():
        print("check-trial-verdicts: docs/trials/ absent — nothing to bind to")
        return 0

    files = sorted(TRIALS.glob("trial-*.md"))
    problems, verdicts, artifact, instrument = [], 0, 0, 0
    for path in files:
        p, required, bounds = audit(path)
        problems += p
        verdicts += len(required)
        # Split by kind. A single `declared` total labelled with ONE of the two
        # kinds is the defect this whole gate exists to catch, one layer out:
        # the count would be right and the sentence would not be about it.
        artifact += sum(1 for b in bounds.values() if b == "artifact-bound")
        instrument += sum(1 for b in bounds.values() if INSTRUMENT_BOUND.match(b))

    print(
        f"check-trial-verdicts: {len(files)} trial record(s), {verdicts} judged "
        f"verdict(s), {artifact} artifact-bound + {instrument} instrument-bound "
        f"declaration(s)"
    )
    if files and not verdicts:
        problems.append(
            "no judged verdict was found in any trial record — this gate is "
            "bound to nothing"
        )
    for line in problems:
        print(f"  FAIL {line}")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
