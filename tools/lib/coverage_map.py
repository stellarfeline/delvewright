"""The per-test coverage map's runner (spec-0126 §3.1), called by
`tools/ci/test-coverage-map.sh` after it has built the instrumented test
binaries and listed them with `cargo nextest list --message-format json`.

Each listed test that is not `#[ignore]` runs as its own process:
`LLVM_PROFILE_FILE=<dir>/<n>-%p-%m.profraw <binary> --exact <name>`, so the
test binary and every instrumented `delvec` it spawns each leave a profile.
The profiles are merged with `llvm-profdata merge`, exported with
`llvm-cov export --object <binary> --object <delvec>`, and reduced through the
shared production filter (`tools/lib/coverage_regions.py`) to the set of
executed region entries, keyed by `(path, line, column)` — a key that does not
depend on the binary, so sets from different binaries compare.

Writes `<out>/tests/<binary-id>/<n>.json` per test:
`{"binary", "test", "regions", "seconds", "exit"}`; a test whose process did
not exit 0 has `regions: null` — its coverage is unknown, not small. Writes
`<out>/summary.json` with the per-binary listed and written counts, and
prints the binding line. Exits 1 when any binary's written count differs from
its listed count.
"""

from __future__ import annotations

import argparse
import concurrent.futures
import json
import os
import pathlib
import re
import subprocess
import sys
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

from lib.coverage_regions import Filter, read_export  # noqa: E402


def llvm_tool(name: str) -> str:
    sysroot = subprocess.run(["rustc", "--print", "sysroot"], stdout=subprocess.PIPE, text=True, check=True).stdout.strip()
    host = re.search(
        r"^host: (\S+)$",
        subprocess.run(["rustc", "-vV"], stdout=subprocess.PIPE, text=True, check=True).stdout,
        re.M,
    ).group(1)
    path = pathlib.Path(sysroot) / "lib" / "rustlib" / host / "bin" / name
    if not path.is_file():
        sys.exit(f"coverage_map: {path} not found — `rustup component add llvm-tools`")
    return str(path)


def safe(name: str) -> str:
    return re.sub(r"[^A-Za-z0-9_.-]", "_", name)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--list", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--root", required=True)
    ap.add_argument("--jobs", type=int, default=os.cpu_count() or 1)
    ap.add_argument("--binary", default=None, help="only binaries whose id matches this regex")
    ap.add_argument("--timeout", type=int, default=3600)
    args = ap.parse_args()
    sys.stdout.reconfigure(newline="\n")

    root = pathlib.Path(args.root).resolve()
    out = pathlib.Path(args.out).resolve()
    listing = json.loads(pathlib.Path(args.list).read_text(encoding="utf-8"))
    target = pathlib.Path(listing["rust-build-meta"]["target-directory"])
    delvec = next(
        (str(p) for p in (target / "debug" / "delvec", target / "debug" / "delvec.exe") if p.is_file()),
        None,
    )
    profdata = llvm_tool("llvm-profdata")
    llvm_cov = llvm_tool("llvm-cov")
    filt = Filter(root)
    prefix = str(root) + "/"

    jobs: list[tuple[str, str, str, str, int]] = []
    listed: dict[str, int] = {}
    ignored: dict[str, int] = {}
    for bid, suite in sorted(listing["rust-suites"].items()):
        if args.binary and not re.search(args.binary, bid):
            continue
        names = sorted(suite["testcases"])
        run = [n for n in names if not suite["testcases"][n].get("ignored")]
        listed[bid] = len(run)
        ignored[bid] = len(names) - len(run)
        for i, name in enumerate(run):
            jobs.append((bid, suite["binary-path"], suite["cwd"], name, i))

    def one(job: tuple[str, str, str, str, int]) -> tuple[str, int]:
        bid, binary, cwd, name, i = job
        dest = out / "tests" / safe(bid) / f"{i:05d}.json"
        if dest.is_file():
            return bid, json.loads(dest.read_text(encoding="utf-8"))["exit"]
        prof = out / "prof" / safe(bid) / f"{i:05d}"
        prof.mkdir(parents=True, exist_ok=True)
        env = dict(os.environ)
        env["LLVM_PROFILE_FILE"] = str(prof / "%p-%m.profraw")
        env["CARGO_MANIFEST_DIR"] = cwd
        t0 = time.monotonic()
        try:
            r = subprocess.run(
                [binary, "--exact", name, "--test-threads", "1"],
                cwd=cwd,
                env=env,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                timeout=args.timeout,
            )
            code = r.returncode
            log = r.stdout[-4000:].decode("utf-8", "replace")
        except subprocess.TimeoutExpired:
            code, log = -9, "timeout"
        seconds = round(time.monotonic() - t0, 3)
        regions = None
        if code == 0:
            raws = sorted(str(p) for p in prof.glob("*.profraw"))
            merged = prof / "merged.profdata"
            subprocess.run([profdata, "merge", "-sparse", *raws, "-o", str(merged)], check=True)
            objs = [binary] + (["-object", delvec] if delvec else [])
            ex = subprocess.run(
                [llvm_cov, "export", "-format=text", "-skip-functions", f"-instr-profile={merged}", *objs, str(root / "crates")],
                stdout=subprocess.PIPE,
                check=True,
            )
            view = read_export(json.loads(ex.stdout)["data"][0], filt, prefix)
            regions = sorted([p, l, c] for (p, l, c), hit in view["regions"].items() if hit)
        for p in prof.glob("*"):
            p.unlink()
        prof.rmdir()
        dest.parent.mkdir(parents=True, exist_ok=True)
        row = {"binary": bid, "test": name, "regions": regions, "seconds": seconds, "exit": code}
        if code != 0:
            row["log"] = log
        dest.write_text(json.dumps(row, separators=(",", ":")) + "\n", encoding="utf-8", newline="\n")
        return bid, code

    written: dict[str, int] = {b: 0 for b in listed}
    failed: dict[str, int] = {b: 0 for b in listed}
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
        for bid, code in pool.map(one, jobs):
            written[bid] += 1
            failed[bid] += 1 if code != 0 else 0
    short = sorted(b for b in listed if written[b] != listed[b])
    summary = {
        "binaries": {b: {"listed": listed[b], "written": written[b], "exit_nonzero": failed[b], "ignored": ignored[b]} for b in listed},
        "listed": sum(listed.values()),
        "written": sum(written.values()),
        "exit_nonzero": sum(failed.values()),
        "ignored": sum(ignored.values()),
    }
    (out / "summary.json").write_text(json.dumps(summary, indent=1, sort_keys=True) + "\n", encoding="utf-8", newline="\n")
    print(
        f"coverage map: {summary['written']} tests written of {summary['listed']} listed "
        f"(--list, not #[ignore]) over {len(listed)} binaries; {summary['exit_nonzero']} exited "
        f"non-zero (no region set); {summary['ignored']} ignored; {len(short)} binaries short"
    )
    for b in short:
        print(f"SHORT: {b}: {written[b]} of {listed[b]}", file=sys.stderr)
    return 1 if short else 0


if __name__ == "__main__":
    sys.exit(main())
