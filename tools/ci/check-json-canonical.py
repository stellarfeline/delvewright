#!/usr/bin/env python3
"""Every JSON document this repository holds is in `delvec fmt` canonical form.

## The defect this replaces

`delvec fmt --check` reached CI over **two roots named by hand** in `ci.yml`:

    delvec fmt --check crates/dsl/fixtures crates/delvec/tests/fixtures

That is the enumeration-somebody-remembered shape this repository refuses
elsewhere by name. It examined 240 of the 335 JSON files git tracks, and the
95 it never looked at held **50** that were not canonical — `gallery/`
(whose own spec, spec-0039 criterion 16, asserts in writing that its JSON *is*
inside the sweep), the spec probe fixtures, `docs/playtest-findings.json`,
`.github/` and `harness/`. Nothing was red, because nothing looked.
A hand-authored document is non-canonical by default and only CI says so — but
only if CI looks, and a root list only ever covers the directories that existed
when someone last thought about it.

## The set is DERIVED, never listed

The population is `git ls-files -z -- '*.json'`: the repository's own answer to
what it holds. A directory of authored JSON added next month is swept the moment
it is committed, with nothing here to edit — which is the whole point, and is
demonstrated rather than asserted by
`tools/tests/test_check_json_canonical.py::a_new_directory_is_swept_with_no_edit_here`.

Using git rather than a filesystem walk is not a convenience. It is what makes
the *exclusions* properties instead of names: `target/`, `node_modules/`,
`gallery-prefabs/` and every other build product are absent because they are not
tracked, `campaigns/` contributes one symlink entry rather than a second
repository's contents, and `delvec build` output trees are absent for the same
reason `delvewright_dsl::fmt::BUILD_OUTPUT_MARKER` skips them. None of those is
a rule written here that could go stale.

## Authored and generated, and why there is no exemption

"It's generated" is precisely what someone would say about a file nobody
canonicalised, so an exemption an author can simply assert is a hatch the defect
supplies (CLAUDE.md, the sixth vacuity mode). The resolution taken here is to
remove the need for the hatch rather than to secure it:

* **What this repository's own tools write is made canonical at the writer.**
  `tools/ci/gallery-baseline.py` wrote `gallery/baseline/warnings.json` with
  python's default `ensure_ascii=True`, so an em-dash inside a warning pointer
  came out escaped. That is the only reason that ledger was
  non-canonical, and the fix belongs in the generator: it now writes
  `ensure_ascii=False` and its output is swept like anything else. A generated
  file that is *in* canonical form needs no exemption, and cannot be used as
  cover by a file that merely never was.
* **What a foreign program owns is formatted anyway.** `harness/package.json`,
  `harness/package-lock.json` and `.claude/settings.json` are rewritten by npm
  and by the agent harness, which will re-order their keys the next time they
  run. That costs a `delvec fmt harness` afterwards — exactly the relationship
  `cargo fmt` has with hand-written Rust, self-announcing and one command to
  discharge. It does not cost a class of file nobody checks.

Nothing else is exempt: every tracked `.json` is swept.

## Binding count, with its denominator

Every run states files swept **against the tracked-JSON population**. Two ways
to be vacuous are refused rather than reported: a population of zero, and a
swept set of zero (`delvec fmt` itself would also say `DW0774`).

Deterministic, offline, stdlib-only python3.
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]

# `delvec fmt --check` takes the paths as arguments. Chunked so the population
# can grow without meeting ARG_MAX, and the exit status of every chunk counts.
CHUNK = 400


def die(msg: str) -> int:
    print(f"error: {msg}", file=sys.stderr)
    return 1


def tracked_json(repo: Path) -> list[str]:
    """Every `*.json` git tracks, in git's order — the derived population."""
    r = subprocess.run(
        ["git", "-C", str(repo), "ls-files", "-z", "--", "*.json"],
        capture_output=True,
        text=True,
    )
    if r.returncode != 0:
        raise SystemExit(die(f"`git ls-files` exited {r.returncode}: {r.stderr.strip()}"))
    return sorted(p for p in r.stdout.split("\0") if p)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--delvec",
        help="a built delvec binary. Default: build and run one through cargo, "
        "which is what CI does and what a fresh clone needs.",
    )
    ap.add_argument(
        "--repo",
        default=str(REPO),
        help="the repository to sweep. Defaults to this script's own, which is "
        "what CI and a creator use. It exists so `tools/tests/` can DEMONSTRATE "
        "the derivation on a throwaway repository — that a directory nobody "
        "here has heard of is swept the moment it is committed — rather than "
        "asserting it in prose.",
    )
    args = ap.parse_args()
    repo = Path(args.repo).resolve()

    delvec = (
        [args.delvec]
        if args.delvec
        else ["cargo", "run", "--locked", "-q", "-p", "delvec", "--bin", "delvec", "--"]
    )

    population = tracked_json(repo)
    if not population:
        return die(
            "git tracks NO .json files at all. This check examined nothing, which "
            "is a vacuous pass rather than a pass."
        )

    swept = population
    print(
        f"canonical-form sweep: {len(swept)} of {len(population)} tracked JSON "
        "document(s) swept; 0 exempt."
    )

    failed = False
    for i in range(0, len(swept), CHUNK):
        chunk = swept[i : i + CHUNK]
        # Status captured directly, never through a pipe: a piped verdict is the
        # pipe's exit status (CLAUDE.md).
        r = subprocess.run(delvec + ["fmt", "--check"] + chunk, cwd=repo)
        if r.returncode != 0:
            failed = True

    if failed:
        print(
            "\nerror: a JSON document this repository holds is not in canonical "
            "form. Fix it by running the formatter, never by hand:\n"
            "  cargo run -q -p delvec --bin delvec -- fmt <path>",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
