#!/usr/bin/env python3
"""The partition census: every listed test ran in exactly one partition.

spec-0126 §5. The `rust` job's test run is N partition jobs, each running
`cargo nextest … --partition hash:k/N` over the one instrumented archive. A
sharding that can drop a test is the unbound vacuity mode (CLAUDE.md
*Vacuity*), so this gate reads what the partitions did the way the suite's
reader does — one JUnit report per partition, one <testcase> per test — and
holds the union to the archive's own listing.

WHAT REFUSES (exit 1)

- the partition count is zero, or the number of JUnit reports is not N;
- a partition ran zero tests;
- a listed (non-ignored) test ran in no partition;
- a test ran in two partitions, or ran without being listed;
- a partition's report carries a failure or an error (the partition job is red
  on its own, and the census says so beside the counts).

USAGE

  check-partitions.py --list LIST.json --junit-dir DIR --partitions N

`LIST.json` is `cargo nextest list … --message-format json` over the archive
(`rust-suites` → binary id → `testcases` → test name → `ignored`); `DIR` holds
one `junit.xml` per partition, each under its own directory (the partition's
name), at any depth inside it. Stdlib only.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import sys
import xml.etree.ElementTree as ET


def listed(path: pathlib.Path) -> tuple[set[tuple[str, str]], int]:
    """Every non-ignored (binary id, test name) the archive lists, and the ignored count."""
    data = json.loads(path.read_text(encoding="utf-8"))
    tests: set[tuple[str, str]] = set()
    ignored = 0
    for binary, suite in data.get("rust-suites", {}).items():
        for name, case in suite.get("testcases", {}).items():
            if case.get("ignored"):
                ignored += 1
            else:
                tests.add((binary, name))
    return tests, ignored


def report(path: pathlib.Path) -> dict:
    """One partition's JUnit report: its tests, failures, the runner's wall and the per-test sum."""
    root = ET.parse(path).getroot()
    cases: list[tuple[str, str]] = []
    seconds = 0.0
    failures = 0
    for suite in root.iter("testsuite"):
        for case in suite.iter("testcase"):
            cases.append((case.get("classname", ""), case.get("name", "")))
            seconds += float(case.get("time", "0"))
            if case.find("failure") is not None or case.find("error") is not None:
                failures += 1
    return {
        "path": path,
        "cases": cases,
        "seconds": seconds,
        "wall": float(root.get("time", "0")),
        "failures": failures,
    }


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--list", required=True, help="cargo nextest list --message-format json")
    ap.add_argument("--junit-dir", required=True, help="one junit.xml per partition, each in its own directory")
    ap.add_argument("--partitions", required=True, type=int, help="N, the partition count the workflow ran")
    args = ap.parse_args()

    refusals: list[str] = []
    n = args.partitions
    if n <= 0:
        refusals.append(f"partition count {n}: a test run in no partition")
    want, ignored = listed(pathlib.Path(args.list))
    junit_dir = pathlib.Path(args.junit_dir)
    reports = [report(p) for p in sorted(junit_dir.glob("**/junit.xml"))]
    for r in reports:
        # The partition is the directory its artifact was downloaded into,
        # whatever the report's path inside the artifact.
        r["label"] = r["path"].relative_to(junit_dir).parts[0]
    if len(reports) != n:
        refusals.append(f"{len(reports)} partition report(s) found and {n} partition(s) were declared")

    seen: dict[tuple[str, str], list[str]] = {}
    for r in reports:
        label = r["label"]
        if not r["cases"]:
            refusals.append(f"partition {label} ran zero tests")
        if r["failures"]:
            refusals.append(f"partition {label} reports {r['failures']} failing test(s)")
        for case in r["cases"]:
            seen.setdefault(case, []).append(label)

    missing = sorted(want - seen.keys())
    unknown = sorted(seen.keys() - want)
    twice = sorted(c for c, labels in seen.items() if len(labels) > 1)
    if missing:
        refusals.append(
            f"{len(missing)} listed test(s) ran in no partition: "
            + ", ".join(f"{b}::{t}" for b, t in missing[:10])
            + (" …" if len(missing) > 10 else "")
        )
    if unknown:
        refusals.append(
            f"{len(unknown)} test(s) ran that the archive does not list: "
            + ", ".join(f"{b}::{t}" for b, t in unknown[:10])
            + (" …" if len(unknown) > 10 else "")
        )
    if twice:
        refusals.append(
            f"{len(twice)} test(s) ran in more than one partition: "
            + ", ".join(f"{b}::{t} ({', '.join(seen[(b, t)])})" for b, t in twice[:10])
            + (" …" if len(twice) > 10 else "")
        )

    counts = [len(r["cases"]) for r in reports]
    print(
        f"partitions: {len(reports)} of {n} declared; listed: {len(want)} tests ({ignored} ignored); "
        f"ran: {len(seen)} distinct in {sum(counts)} testcases; per partition: "
        f"min {min(counts) if counts else 0}, max {max(counts) if counts else 0}"
    )
    for r in reports:
        print(
            f"  {r['label']}: {len(r['cases'])} tests, JUnit sum {r['seconds']:.0f} s, "
            f"nextest wall {r['wall']:.0f} s, failures {r['failures']}"
        )
    if reports:
        print(
            f"whole suite: JUnit sum {sum(r['seconds'] for r in reports):.0f} s; "
            f"longest partition wall {max(r['wall'] for r in reports):.0f} s"
        )
    for why in refusals:
        print(f"REFUSED: {why}", file=sys.stderr)
    return 1 if refusals else 0


if __name__ == "__main__":
    sys.exit(main())
