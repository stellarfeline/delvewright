#!/usr/bin/env python3
"""The coverage floor: region coverage of production source may not fall.

spec-0126 §1.2, §2. The `rust` job runs the whole suite once, instrumented
(`cargo llvm-cov --no-report nextest …`), and exports the counters
(`cargo llvm-cov report --json`; its file names are absolute, and the reader
strips the workspace root the export names in `cargo_llvm_cov.manifest_path`).
This gate reads that export, keeps the regions of production source, and judges them against the
record at `.github/coverage-baseline.json`.

WHAT IS PRODUCTION SOURCE

A file's production text is the file with every item a column-zero
`#[cfg(test)]` applies to removed; an out-of-line `#[cfg(test)] mod name;`
takes its file and every file under its directory with it. That rule's home is
`crates/delvec/tests/common/source_scan.rs`; the copy every coverage reader
uses, `tools/lib/coverage_regions.py`, is held equal to it by the agreement test
`source_scan_agrees` (the `--production` and
`--production-sources` modes below are what that test asks). cargo-llvm-cov
already drops directories named `tests`; this reader drops every region whose
start lies inside a `#[cfg(test)]` item, from numerator and denominator alike,
and prints what it dropped.

WHAT REFUSES (exit 1)

- `P < P0 - tolerance`, where tolerance is the record's `unstable_regions`
  over its region count, in points;
- zero production regions judged — an unbound run;
- the record's `rows_sha256` is not the hash of its own `files` rows, or the
  record's `tree` is not an ancestor of the head being judged;
- any `#[mutants::skip]` or `coverage(off)` attribute in production source.

MODES

  coverage-floor.py --report R [--record F] [--root DIR] [--head REV]
      judge a report against the record.
  coverage-floor.py --write --report R1 --second-report R2 --tree SHA
                    --llvm-tools VERSION [--record F] [--root DIR]
      write the record from two runs of the suite at SHA.
  coverage-floor.py --production FILE
      print, as JSON, FILE's production text and its out-of-line test modules.
  coverage-floor.py --production-sources SRC
      print, as JSON, every production file under SRC (relative path, text).

Stdlib only; deterministic in what it prints.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import re
import subprocess
import sys
import tomllib
from fractions import Fraction

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

from lib.coverage_regions import Filter, production, production_sources, read_report  # noqa: E402

RECORD = ".github/coverage-baseline.json"
COMMANDS = [
    "cargo llvm-cov --no-report nextest --workspace --locked --no-fail-fast --profile ci",
    "cargo llvm-cov report --json --output-path target/coverage.json",
]
# The opt-outs an author can write (spec-0126 §1.3): each hides exactly the
# region a weak test fails to check, so the count in production source is zero.
OPT_OUTS = re.compile(r"#\[\s*mutants::skip\s*\]|coverage\s*\(\s*off\s*\)")


def totals(files: list[dict], kind: str = "regions") -> tuple[int, int]:
    return (
        sum(f[kind]["covered"] for f in files),
        sum(f[kind]["count"] for f in files),
    )


def canonical_rows(files: list[dict]) -> str:
    return json.dumps(files, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def region_set_sha(regions: dict[tuple[str, int, int], bool]) -> str:
    h = hashlib.sha256()
    for (path, line, col) in sorted(regions):
        h.update(f"{path}\t{line}\t{col}\t{1 if regions[(path, line, col)] else 0}\n".encode())
    return h.hexdigest()


def opt_outs(root: pathlib.Path) -> list[str]:
    """Every opt-out attribute standing in production source, as `path:line`."""
    found: list[str] = []
    crates = root / "crates"
    if not crates.is_dir():
        return found
    for src in sorted(p for p in crates.glob("*/src") if p.is_dir()):
        for rel, text in production_sources(src):
            for i, line in enumerate(text.splitlines(), 1):
                if OPT_OUTS.search(line.split("//", 1)[0]):
                    found.append(f"{(src / rel).relative_to(root).as_posix()}:{i}")
    return found


def pct(c: int, n: int) -> str:
    return f"{(100 * c / n):.2f}" if n else "0.00"


# --- the record -------------------------------------------------------------


def instrument(root: pathlib.Path, llvm_tools: str) -> dict:
    with open(root / "rust-toolchain.toml", "rb") as fh:
        rustc = tomllib.load(fh)["toolchain"]["channel"]
    with open(root / "versions.toml", "rb") as fh:
        ci = tomllib.load(fh)["ci"]
    return {
        "rustc": rustc,
        "cargo_llvm_cov": ci["llvm_cov_version"],
        "llvm_tools": llvm_tools,
        "nextest": ci["nextest_version"],
    }


def write(args, root: pathlib.Path, filt: Filter) -> int:
    first = read_report(pathlib.Path(args.report), filt)
    second = read_report(pathlib.Path(args.second_report), filt)
    cov1 = {k for k, v in first["regions"].items() if v}
    cov2 = {k for k, v in second["regions"].items() if v}
    unstable = len(cov1 ^ cov2)
    record_path = root / args.record
    previous = None
    if record_path.is_file():
        previous = json.loads(record_path.read_text(encoding="utf-8"))
    rec = {
        "tree": args.tree,
        "instrument": instrument(root, args.llvm_tools),
        "command": COMMANDS,
        "files": first["files"],
        "excluded": first["excluded"],
        "unstable_regions": unstable,
        "sha256": region_set_sha(first["regions"]),
        "rows_sha256": hashlib.sha256(canonical_rows(first["files"]).encode()).hexdigest(),
    }
    c, n = totals(first["files"])
    if previous is not None:
        pc, pn = totals(previous["files"])
        if Fraction(c, n or 1) < Fraction(pc, pn or 1):
            rec["lowered_from"] = {"regions": {"covered": pc, "count": pn}, "tree": previous["tree"]}
    record_path.write_text(json.dumps(rec, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    lc, ln = totals(first["files"], "lines")
    fc, fnn = totals(first["files"], "functions")
    print(
        f"record written: {args.record} at {args.tree}: regions {c}/{n} ({pct(c, n)} %), "
        f"lines {lc}/{ln} ({pct(lc, ln)} %), functions {fc}/{fnn} ({pct(fc, fnn)} %); "
        f"{len(first['files'])} files; excluded {first['excluded']['regions']} regions in "
        f"{first['excluded']['files']} files; unstable_regions {unstable} "
        f"(covered sets {len(cov1)} and {len(cov2)}); skipped {len(first['skipped'])} non-tree files"
    )
    if "lowered_from" in rec:
        print(f"FLOOR LOWERED from {pct(pc, pn)} to {pct(c, n)} by this record")
    return 0


def ancestor(root: pathlib.Path, tree: str, head: str) -> tuple[bool, str]:
    r = subprocess.run(
        ["git", "-C", str(root), "merge-base", "--is-ancestor", tree, head],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        text=True,
    )
    if r.returncode == 0:
        return True, ""
    if r.returncode == 1:
        return False, f"the record's tree {tree} is not an ancestor of {head} — a floor set on a branch"
    return False, f"cannot establish that {tree} is an ancestor of {head}: {r.stderr.strip()}"


def judge(args, root: pathlib.Path, filt: Filter) -> int:
    record_path = root / args.record
    if not record_path.is_file():
        print(f"coverage-floor: no record at {args.record}", file=sys.stderr)
        return 1
    rec = json.loads(record_path.read_text(encoding="utf-8"))
    rep = read_report(pathlib.Path(args.report), filt)
    c, n = totals(rep["files"])
    c0, n0 = totals(rec["files"])
    u = rec.get("unstable_regions", 0)
    tol = Fraction(u, n0) if n0 else Fraction(0)
    p = Fraction(c, n) if n else Fraction(0)
    p0 = Fraction(c0, n0) if n0 else Fraction(0)
    headroom = float((p - p0) * 100)
    print(
        f"regions judged: {n} production regions in {len(rep['files'])} files; "
        f"excluded: {rep['excluded']['regions']} regions in {rep['excluded']['files']} files "
        f"(tests directories, cfg(test) items); covered: {c} ({pct(c, n)} %); "
        f"floor: {c0}/{n0} ({pct(c0, n0)} %); headroom: {headroom:+.2f} points"
    )
    if rep["skipped"]:
        print(f"skipped: {len(rep['skipped'])} files outside the tree (not production source of it)")
    refusals: list[str] = []
    if n == 0:
        refusals.append("zero production regions judged — an unbound run")
    elif p < p0 - tol:
        refusals.append(
            f"region coverage {pct(c, n)} % is below the floor {pct(c0, n0)} % less its "
            f"tolerance ({u} unstable regions = {float(tol * 100):.4f} points)"
        )
    rows = hashlib.sha256(canonical_rows(rec.get("files", [])).encode()).hexdigest()
    if rows != rec.get("rows_sha256"):
        refusals.append(
            f"the record's rows_sha256 {rec.get('rows_sha256')} is not the hash of its own "
            f"files rows ({rows}) — a hand-edited row"
        )
    ok, why = ancestor(root, rec.get("tree", ""), args.head)
    if not ok:
        refusals.append(why)
    outs = opt_outs(root)
    print(f"opt-outs in production source (mutants::skip, coverage(off)): {len(outs)}")
    if outs:
        refusals.append("an opt-out attribute stands in production source: " + ", ".join(outs))
    if "lowered_from" in rec:
        lf = rec["lowered_from"]["regions"]
        print(f"FLOOR LOWERED from {pct(lf['covered'], lf['count'])} to {pct(c0, n0)} by this pull request")
    for r in refusals:
        print(f"REFUSED: {r}", file=sys.stderr)
    return 1 if refusals else 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--report")
    ap.add_argument("--second-report")
    ap.add_argument("--record", default=RECORD)
    ap.add_argument("--root", default=None)
    ap.add_argument("--head", default="HEAD")
    ap.add_argument("--write", action="store_true")
    ap.add_argument("--tree")
    ap.add_argument("--llvm-tools")
    ap.add_argument("--production")
    ap.add_argument("--production-sources")
    args = ap.parse_args()
    if args.production:
        text = pathlib.Path(args.production).read_text(encoding="utf-8")
        prod, modules, _ = production(text)
        print(json.dumps({"production": prod, "modules": modules}, ensure_ascii=False))
        return 0
    if args.production_sources:
        rows = production_sources(pathlib.Path(args.production_sources))
        print(json.dumps([[r, t] for r, t in rows], ensure_ascii=False))
        return 0
    root = pathlib.Path(args.root) if args.root else pathlib.Path(
        subprocess.run(["git", "rev-parse", "--show-toplevel"], stdout=subprocess.PIPE, text=True, check=True).stdout.strip()
    )
    filt = Filter(root)
    if not args.report:
        ap.error("--report is required")
    if args.write:
        if not (args.second_report and args.tree and args.llvm_tools):
            ap.error("--write needs --second-report, --tree and --llvm-tools")
        return write(args, root, filt)
    return judge(args, root, filt)


if __name__ == "__main__":
    sys.exit(main())
