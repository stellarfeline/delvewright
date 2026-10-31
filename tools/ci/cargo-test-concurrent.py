#!/usr/bin/env python3
"""`cargo test --workspace`, with the test executables run several at a time.

WHY THIS EXISTS

`cargo test` runs a workspace's test executables one after another. Inside
one executable the tests share the cores, but an executable whose work is a
sequence of `delvec` runs — each with long single-threaded stretches — holds
the whole runner while using little of it. Running a fixed number of
executables at once fills those stretches. Nothing about any test changes:
every executable cargo builds for `cargo test --workspace` is run, whole, with
the working directory and `CARGO_MANIFEST_DIR` cargo gives it, and the doc
tests run after, through cargo.

WHAT IT DOES

1. `cargo test --workspace --no-run --message-format=json` (plus `--locked`
   when given) builds every test target; the executables are read from
   cargo's own `compiler-artifact` messages (`profile.test`, a non-null
   `executable`) — the container is cargo's listing, so nothing is chosen.
2. Each executable runs once, `--jobs` at a time, in the order cargo listed
   them; its output is printed whole, under cargo's own `Running` header,
   when it exits.
3. `cargo test --workspace --doc` runs the doc tests.
4. It prints `test executables: R run of L listed, F failed` and exits 1 when
   any executable or the doc tests failed, or when R differs from L, and 2
   when cargo listed none.

Every executable runs to its end (`--no-fail-fast`). Deterministic in what it
runs; stdlib-only python3.
"""

from __future__ import annotations

import argparse
import concurrent.futures
import json
import os
import subprocess
import sys
import time
from pathlib import Path


def listed(cargo_args: list[str]) -> list[dict]:
    """Build the test targets; every test executable cargo reports, in its order."""
    cmd = ["cargo", "test", "--workspace", "--no-run", "--message-format=json", *cargo_args]
    proc = subprocess.run(cmd, stdout=subprocess.PIPE, text=True)
    out: list[dict] = []
    seen: set[str] = set()
    for line in proc.stdout.splitlines():
        try:
            msg = json.loads(line)
        except json.JSONDecodeError:
            continue
        if msg.get("reason") != "compiler-artifact":
            continue
        exe = msg.get("executable")
        if not exe or not msg.get("profile", {}).get("test"):
            continue
        if exe in seen:
            continue
        seen.add(exe)
        target = msg["target"]
        out.append(
            {
                "exe": exe,
                "src": target["src_path"],
                "kind": target["kind"][0],
                "manifest": msg["manifest_path"],
            }
        )
    if proc.returncode != 0:
        print(f"cargo test --no-run exited {proc.returncode}", file=sys.stderr)
        sys.exit(1)
    return out


def header(t: dict) -> str:
    """Cargo's own line for an executable it is about to run."""
    pkg = Path(t["manifest"]).parent
    try:
        src = Path(t["src"]).relative_to(pkg)
    except ValueError:
        src = Path(t["src"])
    rel = os.path.relpath(t["exe"])
    what = f"unittests {src}" if t["kind"] in ("lib", "bin") else str(src)
    return f"     Running {what} ({rel})"


def run_one(t: dict) -> tuple[dict, int, str, float]:
    pkg = str(Path(t["manifest"]).parent)
    env = {**os.environ, "CARGO_MANIFEST_DIR": pkg}
    start = time.monotonic()
    proc = subprocess.run(
        [t["exe"]], cwd=pkg, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT
    )
    took = time.monotonic() - start
    return t, proc.returncode, proc.stdout.decode("utf-8", "replace"), took


def doc_tests(cargo_args: list[str]) -> int:
    """The doc tests, through cargo; their exit status."""
    return subprocess.run(["cargo", "test", "--workspace", "--doc", *cargo_args]).returncode


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--jobs", type=int, required=True, help="executables run at once")
    ap.add_argument("--locked", action="store_true", help="pass --locked to cargo")
    args = ap.parse_args(argv)
    if args.jobs < 1:
        ap.error("--jobs must be at least 1")
    cargo_args = ["--locked"] if args.locked else []

    targets = listed(cargo_args)
    if not targets:
        print("test executables: 0 listed; nothing was bound", file=sys.stderr)
        return 2
    failed: list[str] = []
    ran = 0
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
        for t, code, text, took in pool.map(run_one, targets):
            ran += 1
            print(header(t))
            sys.stdout.write(text)
            print(f"     (exit {code}, {took:.1f}s wall)", flush=True)
            if code != 0:
                failed.append(header(t).strip())

    doc = doc_tests(cargo_args)
    if doc != 0:
        failed.append(f"doc tests (exit {doc})")

    print(f"test executables: {ran} run of {len(targets)} listed, {len(failed)} failed")
    for f in failed:
        print(f"  failed: {f}")
    return 1 if failed or ran != len(targets) else 0


if __name__ == "__main__":
    sys.exit(main())
