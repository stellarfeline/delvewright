#!/usr/bin/env python3
"""Build the owner's contact sheets: sweep -> render -> page, one command.

This is the driver for step 3 of the grammar authoring loop (spec-0027 §3) and
of a campaign's build sequence ("zone contact sheets -> owner curates massing").
It chains three tools that already exist and adds no judgement of its own:

    delve-grammar sweep         expand one program many ways -> snapshots
    delve-render  batch         image every snapshot          -> PNGs
    delve-render  contact-sheet composite one page per program

Every program in `delve-grammar list` gets a sheet, or name the ones you want.

## The number to read first

Each sheet's row in the summary states **distinct massings**. That is how many
genuinely different buildings are on the page, computed from the models and not
from the pictures. If it is 1, the page is one building drawn N times and there
is nothing on it to choose between — the driver says so, loudly, and exits
non-zero under --require-choice so a curation gate cannot be passed by a page
that offers none.

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
    if report["built"] == 0:
        print(
            f"  {program}: 0 of {report['candidates']} candidates built — nothing to render",
            file=sys.stderr,
        )
        report["_sheet"] = None
        return report

    run(
        [render_bin, "--size", str(size), "batch", str(nbt_dir), "-o", str(render_dir)],
        quiet=quiet,
    )

    sheet = work / f"{slug}.png"
    title = (
        f"{program} - {report['built']}/{report['candidates']} built, "
        f"{report['distinct_massings']} distinct massing(s)"
    )
    sheet_cmd = [
        render_bin,
        "contact-sheet",
        str(render_dir),
        "-o",
        str(sheet),
        "--thumb",
        str(thumb),
        "--shot",
        shot,
        "--title",
        title,
    ]
    if scores:
        sheet_cmd += ["--scores", str(scores)]
    run(sheet_cmd, quiet=quiet)
    report["_sheet"] = str(sheet)
    return report


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

    print(f"\n{'program':<22} {'cand':>5} {'built':>6} {'models':>7} {'MASSINGS':>9}  sheet")
    uniform = []
    for r in reports:
        flag = ""
        if r["built"] > 1 and r["distinct_massings"] <= 1:
            flag = "  <-- ONE BUILDING, NO CHOICE"
            uniform.append(r["program"])
        print(
            f"{r['program']:<22} {r['candidates']:>5} {r['built']:>6} "
            f"{r['distinct_models']:>7} {r['distinct_massings']:>9}  "
            f"{Path(r['_sheet']).name if r['_sheet'] else '-'}{flag}"
        )
    print(f"\nindex: {index}")

    if uniform:
        print(
            f"\nFINDING: {len(uniform)} of {len(reports)} sheet(s) show ONE building repeated: "
            + ", ".join(uniform)
            + "\nThose pages are evidence, not choices. Vary region or parameters, not only seeds.",
            file=sys.stderr,
        )
        if args.require_choice:
            return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
