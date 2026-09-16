#!/usr/bin/env python3
"""Wait until every required status check has SUCCEEDED on one commit (ADR-0028 §5).

`plugin-release.yml` commits the plugin's version bump on a `release/plugin-*`
branch, dispatches `ci.yml` there, and may fast-forward the protected `main` to
that commit only once the commit carries every required check. This is the wait.

The required set is `.github/required-status-checks.txt` — the file
`tools/check-required-contexts.py` holds in lockstep with `ci.yml` — never a list
written here. For each name, the newest GitHub Actions check run of that name on
the commit (`check-runs?filter=latest&app_id=15368`) must be `completed` with
conclusion `success`. `skipped` is refused like `failure`: a required check that
passes by being skipped judged nothing. A completed non-success refuses at once;
a check not yet present or still running is waited for, until `--timeout-minutes`.

Prints the binding every poll: `k of N required check(s) succeeded`. A listing
equal to its page size is paged, never taken as the end.

Usage:
  python3 tools/wait-required-checks.py --sha SHA [--repo OWNER/NAME]
      [--timeout-minutes 240] [--interval-seconds 60]

Exit 0 all succeeded, 1 a check failed or the wait timed out, 2 unreadable.
"""

from __future__ import annotations

import argparse
import os
import pathlib
import sys
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent / "lib"))

import github_releases  # noqa: E402

REPO = pathlib.Path(__file__).resolve().parent.parent
REQUIRED = REPO / ".github" / "required-status-checks.txt"
ACTIONS_APP_ID = 15368
PER_PAGE = 100


def required_names(path: pathlib.Path = REQUIRED) -> list[str]:
    names = [
        line.strip()
        for line in path.read_text(encoding="utf-8").splitlines()
        if line.strip() and not line.lstrip().startswith("#")
    ]
    if not names:
        raise SystemExit(f"wait-required-checks: {path} names no required check — nothing to wait for is not a pass")
    return names


def latest_runs(repo: str, sha: str, fetch=github_releases.fetch) -> dict[str, dict]:
    out: dict[str, dict] = {}
    page = 1
    while True:
        got = fetch(
            f"repos/{repo}/commits/{sha}/check-runs?filter=latest&app_id={ACTIONS_APP_ID}&per_page={PER_PAGE}&page={page}"
        )
        runs = got.get("check_runs", []) if isinstance(got, dict) else []
        for run in runs:
            name = run.get("name")
            if name and (name not in out or run.get("id", 0) > out[name].get("id", 0)):
                out[name] = run
        if len(runs) < PER_PAGE:
            return out
        page += 1


def judge(names: list[str], runs: dict[str, dict]) -> tuple[list[str], list[str], list[str]]:
    """(succeeded, refused, waiting) by required name."""
    ok, bad, waiting = [], [], []
    for name in names:
        run = runs.get(name)
        if run is None or run.get("status") != "completed":
            waiting.append(name)
        elif run.get("conclusion") == "success":
            ok.append(name)
        else:
            bad.append(f"{name} ({run.get('conclusion')})")
    return ok, bad, waiting


def main(argv: list[str] | None = None, fetch=github_releases.fetch, sleep=time.sleep, clock=time.monotonic) -> int:
    ap = argparse.ArgumentParser(description="wait for every required check to succeed on a commit")
    ap.add_argument("--sha", required=True)
    ap.add_argument("--repo", default=os.environ.get("GITHUB_REPOSITORY") or "stellarfeline/delvewright")
    ap.add_argument("--timeout-minutes", type=float, default=240)
    ap.add_argument("--interval-seconds", type=float, default=60)
    ap.add_argument("--required", type=pathlib.Path, default=REQUIRED)
    args = ap.parse_args(argv)
    sys.stdout.reconfigure(newline="\n")  # CRLF-proof: tools/check-python-shell-newlines.py
    names = required_names(args.required)
    deadline = clock() + args.timeout_minutes * 60
    while True:
        try:
            runs = latest_runs(args.repo, args.sha, fetch)
        except (github_releases.Unreadable, github_releases.NotFound) as exc:
            print(f"wait-required-checks: UNREADABLE — {exc}", file=sys.stderr)
            return 2
        ok, bad, waiting = judge(names, runs)
        print(f"wait-required-checks: {len(ok)} of {len(names)} required check(s) succeeded on {args.sha[:12]}; "
              f"{len(bad)} refused, {len(waiting)} not finished", flush=True)
        if bad:
            print(f"wait-required-checks: REFUSED — {', '.join(bad)}. A required check that did not succeed "
                  f"(a skip included) blocks the fast-forward of main.", file=sys.stderr)
            return 1
        if not waiting:
            return 0
        if clock() >= deadline:
            print(f"wait-required-checks: TIMED OUT — still not finished: {', '.join(waiting)}", file=sys.stderr)
            return 1
        sleep(args.interval_seconds)


if __name__ == "__main__":
    raise SystemExit(main())
