#!/usr/bin/env python3
"""Build the owner's contact sheets: sweep -> render -> page, one command.

This is the driver for step 3 of the grammar authoring loop (spec-0027 §3) and
of a campaign's build sequence ("zone contact sheets -> owner curates massing").
It chains three tools that already exist and adds no judgement of its own:

    delve-grammar sweep         expand one program many ways -> snapshots
                                + a semantics sidecar per candidate
    delve-render  batch         image every snapshot          -> PNGs + plan keys
    delve-render  contact-sheet composite one page per program

Every program in `delve-grammar list` gets a sheet, or name the ones you want.

## Two pages per program, and why

**`<program>.png` is the massing page**: one three-quarter render per candidate.
It answers "what shape is it" and nothing else — a grey solid cannot say where
the party enters, which cells can be walked on, or where an anchor sits.

**`<program>-key.png` is the plan-key page**: the same candidates in the same
order, each drawn as a plan with its walkable floor shaded by level, its
boundary openings marked, and every declared anchor numbered and named. Those
are the facts the program already computed; the sweep now carries them through
to the picture instead of discarding them at the `.nbt`.

Read them together. The summary's `ANCHORS` column is the key page's binding
count: `0/0` means the programs declare nothing and the key page annotates
nothing, which is a finding about the programs and is said out loud rather than
left to be noticed.

## The number to read first

Each sheet's row in the summary states **distinct massings**. That is how many
genuinely different buildings are on the page, computed from the models and not
from the pictures. If it is 1, the page is one building drawn N times and there
is nothing on it to choose between — the driver says so, loudly, and exits
non-zero under --require-choice so a curation gate cannot be passed by a page
that offers none.

"Different building" means different **up to placement**: translation, the four
yaw rotations and a horizontal mirror do not make a second building, so a
candidate and its transposed region count once. The count comes from
`delve-grammar` and this driver adds nothing to it — see `docs/reference/
grammar.md` §6c for the equivalence and for why a pose-sensitive count let a
sheet read 4 while holding 3.

A sweep varies whatever its manifest varies: seed, region, parameters. Seeds
alone are usually inert (a box-split grammar chooses by guards on the box, not
by the RNG), so `--seeds` is the quick look and a manifest is the real sweep.

## Ranking

`--scores` orders a page by `tools/refscore.py` output. The score RANKS and
never GATES (spec-0028 §3): every candidate reaches the page, low scorers last.
With no scores the page is in id order and says so. Scoring needs a reference
image per program, so it is off unless you pass `--scores-dir`.

Nothing here ships or is committed: sweeps, renders and sheets are
generation-time working material (ADR-0013) and cannot move a delve's bytes
(ADR-0006).

Usage:
    tools/zone-sheets.py --out .sheets                    # every program, seeds 1..6
    tools/zone-sheets.py --out .sheets --program bell:gate-ward --seeds 1,2,3
    tools/zone-sheets.py --out .sheets --manifest-dir sweeps/   # <program>.json each
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_SEEDS = "1,2,3,4,5,6"


def find_tool(name: str, explicit: str | None) -> str:
    """Locate a delvewright binary: an explicit path, the release build, or PATH."""
    if explicit:
        return explicit
    built = REPO_ROOT / "target" / "release" / name
    if built.is_file():
        return str(built)
    found = shutil.which(name)
    if found:
        return found
    sys.exit(
        f"error: cannot find {name}. Build it with `cargo build --release` or pass "
        f"--{name.replace('delve-', '')}-bin"
    )


def run(cmd: list[str], *, quiet: bool) -> None:
    """Run a tool, and fail loudly with its own message when it refuses."""
    proc = subprocess.run(cmd, capture_output=True, text=True)
    if proc.returncode != 0:
        sys.stderr.write(proc.stderr)
        sys.exit(f"error: {cmd[0]} exited {proc.returncode}")
    if not quiet and proc.stderr:
        sys.stderr.write(proc.stderr)


def list_programs(grammar_bin: str) -> list[str]:
    proc = subprocess.run([grammar_bin, "list"], capture_output=True, text=True)
    if proc.returncode != 0:
        sys.stderr.write(proc.stderr)
        sys.exit(f"error: {grammar_bin} list exited {proc.returncode}")
    out = []
    for line in proc.stdout.splitlines():
        line = line.strip()
        if line and not line.endswith(":"):
            out.append(line.split()[0])
    return out


# `sweep.json` spells a comparison with the IR's own operator names, because
# that field is the same `CmpOp` a grammar program is written with.
OPS = {"lt": "<", "le": "<=", "gt": ">", "ge": ">=", "eq": "==", "ne": "!="}


def clause_holds(trace: dict) -> bool:
    """Whether one LEAF clause held, by the same rule `CondTrace::holds` uses."""
    cond = trace["cond"]
    if cond == "cmp":
        return trace["holds"]
    if cond == "orientation":
        return trace["want"] == trace["got"]
    if cond == "always":
        return True
    # `otherwise` never holds of its own accord, and a clause that could not be
    # measured counts as not holding — the weight the interpreter gave it.
    return False


def render_clause(trace: dict) -> str:
    """One leaf clause as its author wrote it, with what it came to here."""
    cond = trace["cond"]
    if cond == "cmp":
        short = trace.get("shortfall", {})
        return (
            f"{trace['lhs']['source']} {OPS.get(trace['op'], trace['op'])} "
            f"{trace['rhs']['source']} "
            f"({trace['lhs']['value']} vs {trace['rhs']['value']}), "
            f"{short.get('blocks', '?')} short"
        )
    if cond == "orientation":
        want = ",".join(trace["want"])
        got = ",".join(trace["got"])
        return f"orientation is {want}, the scope's is {got}"
    if cond == "unreadable":
        return f"{trace['source']} cannot be measured here: {trace['reason']}"
    return cond


def decisive_clauses(trace: dict, want: bool = True):
    """Every leaf clause that did NOT do what its guard needed, in reading order.

    `want` is what the enclosing node needed of this one — hold, or, under a
    `none_of`, not hold — which is the same bookkeeping the Rust reader does, so
    a clause inside a `none_of` is named for holding rather than skipped for it.

    A guard can decline on a comparison, on an `orientation` that did not match,
    or on a clause that could not be measured at all. All three are reported: a
    refusal whose digest line is silently absent would be the exact defect this
    digest exists to remove.
    """
    cond = trace["cond"]
    if cond in ("all", "any"):
        for clause in trace["of"]:
            yield from decisive_clauses(clause, want)
    elif cond == "none_of":
        for clause in trace["of"]:
            yield from decisive_clauses(clause, not want)
    elif clause_holds(trace) != want:
        yield trace


def refusal_digest(report: dict) -> list[str]:
    """One line per guard clause that decided a refusal, over every refused row.

    `delve-grammar sweep` prints the full reading on stderr; this is the same
    thing read back out of `sweep.json` as data, so the summary a driver
    assembles says WHICH clause refused rather than only how many did. A rule
    that declines on several clauses, or over several alternatives, contributes
    a line each — the digest names what refused, and does not pick among them.
    A candidate refused for a reason that is not a guard (an export refusal, a
    write error) has no structured refusal and keeps its prose `error`.
    """
    lines = []
    for row in report["rows"]:
        refusal = row.get("refusal")
        if not refusal:
            continue
        for alt in refusal["alternatives"]:
            for clause in decisive_clauses(alt["guard"]):
                lines.append(
                    f"{row['id']}: rule {refusal['symbol']} — {render_clause(clause)}"
                )
    return lines


def build_one(
    program: str,
    out_root: Path,
    *,
    grammar_bin: str,
    render_bin: str,
    seeds: str | None,
    manifest: Path | None,
    scores: Path | None,
    thumb: int,
    size: int,
    shot: str,
    quiet: bool,
) -> dict:
    """Sweep, render and composite one program. Returns its sweep report."""
    slug = program.replace(":", "-")
    work = out_root / slug
    nbt_dir = work / "snapshots"
    render_dir = work / "renders"
    for d in (nbt_dir, render_dir):
        if d.exists():
            shutil.rmtree(d)
        d.mkdir(parents=True, exist_ok=True)

    sweep_cmd = [grammar_bin, "sweep", "-o", str(nbt_dir), "--save-manifest"]
    if manifest:
        sweep_cmd += ["--manifest", str(manifest)]
    else:
        sweep_cmd += ["--program", program, "--seeds", seeds or DEFAULT_SEEDS]
    run(sweep_cmd, quiet=quiet)

    report = json.loads((nbt_dir / "sweep.json").read_text())
    for line in refusal_digest(report):
        print(f"  {line}", file=sys.stderr)
    if report["built"] == 0:
        print(
            f"  {program}: 0 of {report['candidates']} candidates built — nothing to render",
            file=sys.stderr,
        )
        report["_sheet"] = None
        report["_key_sheet"] = None
        return report

    run(
        [render_bin, "--size", str(size), "batch", str(nbt_dir), "-o", str(render_dir)],
        quiet=quiet,
    )

    massings = report["distinct_massings"]
    built, total = report["built"], report["candidates"]
    anchors, with_anchors = report["anchors_declared"], report["rows_with_anchors"]

    def compose(stem: str, which: str, extra_title: str, with_scores: bool) -> str:
        out = work / f"{stem}.png"
        cmd = [
            render_bin,
            "contact-sheet",
            str(render_dir),
            "-o",
            str(out),
            "--thumb",
            str(thumb),
            "--shot",
            which,
            "--title",
            f"{program} - {built}/{total} built, {massings} distinct massing(s){extra_title}",
        ]
        if with_scores and scores:
            cmd += ["--scores", str(scores)]
        run(cmd, quiet=quiet)
        return str(out)

    # The massing page: what shape is it. Ranked when scores were supplied.
    report["_sheet"] = compose(slug, shot, "", True)
    # The key page: what the program KNOWS about it. Never score-ranked — a
    # similarity score measures a render against concept art and has nothing to
    # say about a plan diagram, so ordering it by one would be a number pretending
    # to be a measurement.
    report["_key_sheet"] = compose(
        f"{slug}-key",
        "key",
        f", {anchors} anchor(s) on {with_anchors}/{built}",
        False,
    )
    return report


def scale_notes(reports: list[dict]) -> list[str]:
    """Say, per sheet, that the page does not show absolute size.

    Every candidate is rendered with the camera fitted to its own bounding
    sphere and then scaled into its own square cell, so a 60-long zone and a
    96-long one fill the same square: PROPORTION reads across the page and
    LENGTH does not. When a sweep's candidates are all the same size that costs
    nothing, and when they are not — which is the usual case, since region is
    the liveliest axis a manifest has — the page is quietly normalising away one
    of the things it is asking the owner to compare.

    This is a note and never a gate: it changes no exit code and removes no
    candidate. Making the page itself honest (a shared scale across a sweep's
    renders, or the box on each caption) is the renderer's job and its own
    finding; until then the number is stated where the reader is, rather than
    left to be inferred from a picture that cannot show it.
    """
    out = []
    for r in reports:
        sizes = {tuple(row["region"]) for row in r["rows"] if not row.get("error")}
        if len(sizes) < 2:
            continue
        spans = ["x".join(str(v) for v in s) for s in sorted(sizes)]
        out.append(
            f"NOTE: {r['program']}'s candidates span {spans[0]} .. {spans[-1]}, but every "
            f"cell is scaled to fill its own thumbnail — the page shows proportion, not size."
        )
    return out


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--out", required=True, type=Path, help="output root for sheets and working files")
    ap.add_argument("--program", action="append", help="program id (repeatable); default: all of `delve-grammar list`")
    ap.add_argument("--seeds", help=f"comma-separated seeds for the quick look (default {DEFAULT_SEEDS})")
    ap.add_argument("--manifest-dir", type=Path, help="directory of <program-slug>.json sweep manifests")
    ap.add_argument("--scores-dir", type=Path, help="directory of <program-slug>-scores.json from tools/refscore.py")
    ap.add_argument("--thumb", type=int, default=320, help="thumbnail side on the sheet")
    ap.add_argument("--size", type=int, default=1024, help="render size")
    ap.add_argument("--shot", default="ext-se", help="representative shot per candidate")
    ap.add_argument("--grammar-bin", help="path to delve-grammar")
    ap.add_argument("--render-bin", help="path to delve-render")
    ap.add_argument("--quiet", action="store_true")
    ap.add_argument(
        "--require-choice",
        action="store_true",
        help="exit non-zero if any sheet has only one distinct massing (a page with no choice on it)",
    )
    args = ap.parse_args()

    grammar_bin = find_tool("delve-grammar", args.grammar_bin)
    render_bin = find_tool("delve-render", args.render_bin)
    programs = args.program or list_programs(grammar_bin)
    if not programs:
        sys.exit("error: no programs to sweep")

    args.out.mkdir(parents=True, exist_ok=True)
    reports = []
    for program in programs:
        slug = program.replace(":", "-")
        manifest = None
        if args.manifest_dir:
            cand = args.manifest_dir / f"{slug}.json"
            if cand.is_file():
                manifest = cand
        scores = None
        if args.scores_dir:
            cand = args.scores_dir / f"{slug}-scores.json"
            if cand.is_file():
                scores = cand
        print(f"== {program}", file=sys.stderr)
        reports.append(
            build_one(
                program,
                args.out,
                grammar_bin=grammar_bin,
                render_bin=render_bin,
                seeds=args.seeds,
                manifest=manifest,
                scores=scores,
                thumb=args.thumb,
                size=args.size,
                shot=args.shot,
                quiet=args.quiet,
            )
        )

    index = args.out / "sheets.json"
    index.write_text(json.dumps(reports, indent=2) + "\n")

    print(
        f"\n{'program':<22} {'cand':>5} {'built':>6} {'models':>7} {'MASSINGS':>9} "
        f"{'ANCHORS':>9} {'WAYS':>5}  sheets"
    )
    uniform = []
    unannotated = []
    no_ways = []
    for r in reports:
        flag = ""
        if r["built"] > 1 and r["distinct_massings"] <= 1:
            flag = "  <-- ONE BUILDING, NO CHOICE"
            uniform.append(r["program"])
        if r["built"] and not r["anchors_declared"]:
            flag += "  <-- NOTHING ANNOTATED"
            unannotated.append(r["program"])
        ways = r["rows_with_entry"] + r["rows_with_exit"]
        if r["built"] and not ways:
            no_ways.append(r["program"])
        sheets = "-"
        if r["_sheet"]:
            sheets = f"{Path(r['_sheet']).name} + {Path(r['_key_sheet']).name}"
        print(
            f"{r['program']:<22} {r['candidates']:>5} {r['built']:>6} "
            f"{r['distinct_models']:>7} {r['distinct_massings']:>9} "
            f"{r['anchors_declared']:>9} {ways:>5}  {sheets}{flag}"
        )
    print(f"\nindex: {index}")

    for line in scale_notes(reports):
        print(line, file=sys.stderr)

    if uniform:
        print(
            f"\nFINDING: {len(uniform)} of {len(reports)} sheet(s) show ONE building repeated: "
            + ", ".join(uniform)
            + "\nThose pages are evidence, not choices. Vary region or parameters, not only seeds.",
            file=sys.stderr,
        )
    # A key page that annotated nothing is the vacuity this driver exists to stop
    # being silent about: it looks exactly like a key page of a piece with nothing
    # to annotate. Say which it is, by name, every run.
    if unannotated:
        print(
            f"\nFINDING: {len(unannotated)} of {len(reports)} program(s) declare NO ANCHOR: "
            + ", ".join(unannotated)
            + "\nTheir key pages draw a shape and name nothing on it. An anchor is declared by a "
            "rule (`mark`); rendering cannot recover one that was never declared.",
            file=sys.stderr,
        )
    if no_ways:
        print(
            f"\nFINDING: {len(no_ways)} of {len(reports)} program(s) declare NO ENTRY OR EXIT: "
            + ", ".join(no_ways)
            + "\nThe key pages mark every boundary cell a body could cross, in blue, and say so. "
            "Which of them is the door is authored — no tool here guesses at it.",
            file=sys.stderr,
        )
    if uniform and args.require_choice:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
