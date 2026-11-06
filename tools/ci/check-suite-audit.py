#!/usr/bin/env python3
"""The suite audit table holds: every test file has a row, and every removal its proof.

spec-0126 §6. `crates/delvec/tests/measured/suite-audit.json` has one row per
test file of the tree the audit was taken at (`tree`), enumerated from that
tree's own listing. Each row states the file's measured `tests`, `seconds` and
`unique_regions`, its `shared_state` (`disk`, `cheap` or `none`), a `verdict`
from a closed set, and the `proof` the verdict rests on (spec-0126 §3.3).

WHAT IT ASSERTS (exit 1 on any)

- the rows are exactly the files `git ls-tree -r --name-only <tree> -- crates`
  lists under `crates/*/tests/` with a `.rs` name, outside `common/`,
  `support/`, `fixtures/` and `measured/`, and the table's `files` equals
  that count;
- every verdict is `keep`, `merge into <file>`, `remove: held by
  <file>::<test>` or `remove: held by kill set <path>`;
- a `keep` row's file stands in the tree being judged (at its own path or as
  a module of the package's one test crate, `tests/it/`); a `merge into`
  target stands there; a `held by` test is a function of its file there; a
  `kill set` path holds `with/` and `without/` outcomes with equal caught sets
  and a `command.txt` that names every binary the row's `proof` lists;
- every `shared_state = disk` row's file calls `common::built`.

  --merges     also prints, per `merge into` row, the assertion-macro count and
               the DW-code multiset over the touched files at `tree` and now,
               and refuses a row where either differs.
  --measure MAP --junit X --tree SHA [--second-map MAP2]
               writes the measured columns (tests, seconds, unique_regions,
               subsumed_tests, unknown_coverage_tests, shared_state) from a
               coverage map and a JUnit report, keeping each row's verdict.

Stdlib only.
"""

from __future__ import annotations

import argparse
import collections
import json
import pathlib
import re
import subprocess
import sys

TABLE = "crates/delvec/tests/measured/suite-audit.json"
EXCLUDED_DIRS = ("common", "support", "fixtures", "measured")
VERDICT = re.compile(
    r"^(keep|merge into (?P<into>\S+)|remove: held by kill set (?P<kill>\S+)|remove: held by (?P<held>\S+::\S+))$"
)
ASSERTS = re.compile(r"\b(assert|assert_eq|assert_ne|debug_assert|debug_assert_eq|debug_assert_ne)!\s*\(")
DW = re.compile(r"\bDW\d{4}\b")


def git(root: pathlib.Path, *args: str) -> str:
    r = subprocess.run(["git", "-C", str(root), *args], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    if r.returncode != 0:
        sys.exit(f"check-suite-audit: git {' '.join(args)} failed: {r.stderr.strip()}")
    return r.stdout


def listing(root: pathlib.Path, tree: str) -> list[str]:
    """The test files of `tree`, as the table's denominator defines them."""
    out = []
    for path in git(root, "ls-tree", "-r", "--name-only", tree, "--", "crates").splitlines():
        parts = path.split("/")
        if len(parts) < 4 or parts[0] != "crates" or parts[2] != "tests" or not path.endswith(".rs"):
            continue
        if parts[3] in EXCLUDED_DIRS:
            continue
        out.append(path)
    return sorted(out)


def now_path(root: pathlib.Path, file: str) -> pathlib.Path | None:
    """Where a pre-audit test file stands now: its own path, or `tests/it/`."""
    p = root / file
    if p.is_file():
        return p
    parts = file.split("/")
    moved = root / "/".join(parts[:3] + ["it"] + parts[3:])
    return moved if moved.is_file() else None


def show(root: pathlib.Path, tree: str, file: str) -> str:
    r = subprocess.run(["git", "-C", str(root), "show", f"{tree}:{file}"], stdout=subprocess.PIPE, text=True)
    return r.stdout if r.returncode == 0 else ""


def census(texts: list[str]) -> tuple[int, collections.Counter]:
    n = sum(len(ASSERTS.findall(t)) for t in texts)
    codes = collections.Counter(c for t in texts for c in DW.findall(t))
    return n, codes


def caught(path: pathlib.Path) -> set[str]:
    o = json.loads(path.read_text(encoding="utf-8"))
    return {json.dumps(x["scenario"], sort_keys=True) for x in o.get("outcomes", []) if x.get("summary") == "CaughtMutant"}


SHARED_STATE = re.compile(r"OnceLock|LazyLock|lazy_static|static [A-Z_0-9]+: *(std::sync::)?Mutex")


def suite_of(file: str) -> str:
    """The runner's binary id for a pre-audit test file: `<package>::<stem>`."""
    parts = file.split("/")
    pkg = {"delvec": "delvec", "dsl": "delvewright-dsl"}.get(parts[1], parts[1])
    return f"{pkg}::{pathlib.Path(parts[-1]).stem}"


def load_map(mapdir: pathlib.Path, index: dict) -> dict:
    """{(binary, test): bitmap or None} from a coverage map's per-test files."""
    out = {}
    for p in sorted(mapdir.joinpath("tests").rglob("*.json")):
        r = json.loads(p.read_text(encoding="utf-8"))
        if r["regions"] is None:
            out[(r["binary"], r["test"])] = None
            continue
        ids = []
        for reg in r["regions"]:
            key = (reg[0], reg[1], reg[2])
            i = index.get(key)
            if i is None:
                i = index[key] = len(index)
            ids.append(i)
        out[(r["binary"], r["test"])] = ids
    return out


def to_bitmap(ids: list[int], size: int) -> bytearray:
    bm = bytearray((size + 7) // 8)
    for i in ids:
        bm[i >> 3] |= 1 << (i & 7)
    return bm


def measure(args, root: pathlib.Path) -> int:
    """Write the table's measured columns from a coverage map and a JUnit report.

    `unique_regions`: production regions executed by a test of the file and by
    no test of any other file or binary. `subsumed_tests`: the file's tests
    whose region set is a subset of another test's set (in any binary) — the
    HGS candidates of spec-0126 §3.2, never a verdict. A row's verdict and
    proof are carried over from the existing table when it has one, else
    `keep`."""
    import xml.etree.ElementTree as ET

    index: dict = {}
    first = load_map(pathlib.Path(args.measure), index)
    second = load_map(pathlib.Path(args.second_map), index) if args.second_map else None
    size = len(index)
    bitmaps = {k: (to_bitmap(v, size) if v is not None else None) for k, v in first.items()}
    unstable_tests = unstable_regions = 0
    if second is not None:
        for k, v in second.items():
            a, b = bitmaps.get(k), (to_bitmap(v, size) if v is not None else None)
            if a is None or b is None:
                continue
            x = int.from_bytes(a, "little") ^ int.from_bytes(b, "little")
            if x:
                unstable_tests += 1
                unstable_regions += bin(x).count("1")
    ints = {k: int.from_bytes(b, "little") for k, b in bitmaps.items() if b is not None}
    # Per region: how many tests execute it, and which binaries.
    count = [0] * size
    owner: list = [None] * size
    for (binary, _), ids in first.items():
        if ids is None:
            continue
        for i in ids:
            count[i] += 1
            if owner[i] is None:
                owner[i] = binary
            elif owner[i] != binary:
                owner[i] = ""
    keys = sorted(ints)

    def subsumed(k) -> bool:
        ids = first[k]
        if not ids:
            return False
        rare = min(ids, key=lambda i: count[i])
        byte, bit = rare >> 3, 1 << (rare & 7)
        mine = ints[k]
        for u in keys:
            if u == k:
                continue
            if bitmaps[u][byte] & bit and mine & ~ints[u] == 0:
                return True
        return False

    times: dict = collections.defaultdict(float)
    tests: collections.Counter = collections.Counter()
    for tc in ET.parse(args.junit).getroot().iter("testcase"):
        times[tc.get("classname")] += float(tc.get("time"))
        tests[tc.get("classname")] += 1
    old = {}
    table_path = root / args.table
    if table_path.is_file():
        old = {r["file"]: r for r in json.loads(table_path.read_text(encoding="utf-8"))["rows"]}
    rows = []
    for f in listing(root, args.tree):
        b = suite_of(f)
        mine = [k for k in keys if k[0] == b]
        unknown = sum(1 for k, v in first.items() if k[0] == b and v is None)
        uniq = len({i for k in mine for i in first[k] if owner[i] == b})
        here = now_path(root, f)
        text = here.read_text(encoding="utf-8") if here else ""
        if "common::built" in text or "built::built(" in text:
            state = "disk"
        elif SHARED_STATE.search(show(root, args.tree, f)):
            state = "cheap"
        else:
            state = "none"
        prev = old.get(f, {})
        rows.append({
            "file": f,
            "tests": tests.get(b, 0),
            "seconds": round(times.get(b, 0.0), 3),
            "unique_regions": uniq,
            "subsumed_tests": sum(1 for k in mine if subsumed(k)),
            "unknown_coverage_tests": unknown,
            "shared_state": state,
            "verdict": prev.get("verdict", "keep"),
            "proof": prev.get("proof"),
        })
    doc = {
        "tree": args.tree,
        "files": len(rows),
        "measured": {
            "map_tests": len(first),
            "map_regions": size,
            "map_exit_nonzero": sum(1 for v in first.values() if v is None),
            "map_runs_compared": 2 if second is not None else 1,
            "map_unstable_tests": unstable_tests,
            "map_unstable_regions": unstable_regions,
            "junit": pathlib.Path(args.junit).name,
        },
        "rows": rows,
    }
    table_path.parent.mkdir(parents=True, exist_ok=True)
    table_path.write_text(json.dumps(doc, indent=2, sort_keys=True, ensure_ascii=False) + "\n", encoding="utf-8", newline="\n")
    states = collections.Counter(r["shared_state"] for r in rows)
    print(
        f"measured: {len(rows)} rows; map {len(first)} tests over {size} regions, "
        f"{doc['measured']['map_exit_nonzero']} without a set; second map: {unstable_tests} tests "
        f"differ by {unstable_regions} regions; subsumed candidates "
        f"{sum(r['subsumed_tests'] for r in rows)}; files with no unique region "
        f"{sum(1 for r in rows if r['unique_regions'] == 0)}; shared_state "
        + ", ".join(f"{k} {states[k]}" for k in sorted(states))
    )
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--root", default=None)
    ap.add_argument("--table", default=TABLE)
    ap.add_argument("--merges", action="store_true")
    ap.add_argument("--measure", help="a coverage map directory: write the table's measured columns")
    ap.add_argument("--second-map", help="a second map of the same tree, compared for stability")
    ap.add_argument("--junit", help="the JUnit report the seconds and test counts are read from")
    ap.add_argument("--tree", help="the tree the audit is taken at (with --measure)")
    args = ap.parse_args()
    sys.stdout.reconfigure(newline="\n")
    root = pathlib.Path(args.root) if args.root else pathlib.Path(
        subprocess.run(["git", "rev-parse", "--show-toplevel"], stdout=subprocess.PIPE, text=True, check=True).stdout.strip()
    )
    if args.measure:
        if not (args.junit and args.tree):
            ap.error("--measure needs --junit and --tree")
        return measure(args, root)
    table = json.loads((root / args.table).read_text(encoding="utf-8"))
    tree = table["tree"]
    rows = table["rows"]
    errors: list[str] = []

    want = listing(root, tree)
    have = [r["file"] for r in rows]
    if len(have) != len(set(have)):
        dup = sorted(f for f, n in collections.Counter(have).items() if n > 1)
        errors.append(f"files with more than one row: {dup}")
    missing = sorted(set(want) - set(have))
    extra = sorted(set(have) - set(want))
    if missing:
        errors.append(f"{len(missing)} test file(s) of {tree[:12]} have no row: {missing[:10]}")
    if extra:
        errors.append(f"{len(extra)} row(s) name a file {tree[:12]} does not list: {extra[:10]}")
    if table.get("files") != len(want):
        errors.append(f"the table states files = {table.get('files')}, the listing at {tree[:12]} has {len(want)}")

    verdicts: collections.Counter = collections.Counter()
    states: collections.Counter = collections.Counter()
    merge_rows = []
    for r in rows:
        f = r["file"]
        m = VERDICT.match(r.get("verdict", ""))
        state = r.get("shared_state")
        states[state] += 1
        if state not in ("disk", "cheap", "none"):
            errors.append(f"{f}: shared_state {state!r} is not disk, cheap or none")
        if not m:
            errors.append(f"{f}: verdict {r.get('verdict')!r} is not in the closed set")
            continue
        kind = m.group(1).split(" ")[0].rstrip(":")
        verdicts["keep" if kind == "keep" else "merge" if kind == "merge" else
                 "remove (kill set)" if m.group("kill") else "remove (held by)"] += 1
        here = now_path(root, f)
        if kind == "keep" and here is None:
            errors.append(f"{f}: kept, but it stands nowhere in the tree being judged")
        if state == "disk":
            text = here.read_text(encoding="utf-8") if here else ""
            if "common::built" not in text and "built::built(" not in text:
                errors.append(f"{f}: shared_state disk, but the file never calls common::built")
        if m.group("into"):
            if now_path(root, m.group("into")) is None:
                errors.append(f"{f}: merged into {m.group('into')}, which stands nowhere")
            merge_rows.append((f, m.group("into")))
        if m.group("held"):
            hf, ht = m.group("held").rsplit("::", 1)
            hp = now_path(root, hf)
            if hp is None or not re.search(rf"\bfn {re.escape(ht)}\s*\(", hp.read_text(encoding="utf-8")):
                errors.append(f"{f}: held by {m.group('held')}, which is not a test function in the tree")
        if m.group("kill"):
            kd = root / m.group("kill")
            w, wo, cmd = kd / "with/mutants.out/outcomes.json", kd / "without/mutants.out/outcomes.json", kd / "command.txt"
            if not (w.is_file() and wo.is_file() and cmd.is_file()):
                errors.append(f"{f}: kill set {m.group('kill')} lacks with/, without/ outcomes or command.txt")
            else:
                if caught(w) != caught(wo):
                    errors.append(f"{f}: kill set {m.group('kill')}: caught sets differ")
                text = cmd.read_text(encoding="utf-8")
                for b in r.get("proof", {}).get("binaries", []) if isinstance(r.get("proof"), dict) else []:
                    if f"binary_id({b})" not in text:
                        errors.append(f"{f}: kill set command never names covering binary {b}")
                if not (isinstance(r.get("proof"), dict) and r["proof"].get("binaries")):
                    errors.append(f"{f}: a kill-set row's proof names no covering binary")

    if args.merges:
        for f, into in merge_rows:
            before_texts = [show(root, tree, f), show(root, tree, into)]
            after_paths = {p for p in (now_path(root, f), now_path(root, into)) if p}
            after_texts = [p.read_text(encoding="utf-8") for p in sorted(after_paths)]
            nb, cb = census(before_texts)
            na, ca = census(after_texts)
            print(f"merge {f} -> {into}: assertions {nb} -> {na}; DW codes {sum(cb.values())} -> {sum(ca.values())}")
            if na < nb or any(ca[c] < n for c, n in cb.items()):
                errors.append(f"{f} -> {into}: an assertion or a DW code was dropped by the merge")

    print(
        f"suite audit: {len(rows)} rows for {len(want)} test files of {tree[:12]}; verdicts "
        + ", ".join(f"{k} {verdicts[k]}" for k in sorted(verdicts))
        + "; shared_state "
        + ", ".join(f"{k} {states[k]}" for k in sorted(states, key=str))
        + f"; tests {sum(r.get('tests', 0) for r in rows)}, seconds {round(sum(r.get('seconds', 0) for r in rows))}"
    )
    for e in errors:
        print(f"REFUSED: {e}", file=sys.stderr)
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
