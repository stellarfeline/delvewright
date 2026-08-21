#!/usr/bin/env python3
"""Every JSON document this repository holds is in `delvec fmt` canonical form.

## The defect this replaces

`delvec fmt --check` reached CI over **two roots named by hand** in `ci.yml`:

    delvec fmt --check crates/dsl/fixtures crates/compiler/tests/fixtures

That is the enumeration-somebody-remembered shape this repository refuses
elsewhere by name. It examined 240 of the 335 JSON files git tracks, and the
95 it never looked at held **50** that were not canonical — `gallery/`
(whose own spec, spec-0039 criterion 16, asserts in writing that its JSON *is*
inside the sweep), `docs/specs/spec-003{7,8}-probes/`, `docs/playtest-findings.json`,
`.github/`, `harness/`, `tools/spike-*/`. Nothing was red, because nothing looked.
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

## Authored and generated, and why there is exactly ONE exemption

"It's generated" is precisely what someone would say about a file nobody
canonicalised, so an exemption an author can simply assert is a hatch the defect
supplies (CLAUDE.md, the sixth vacuity mode). The resolution taken here is to
remove the need for the hatch rather than to secure it:

* **What this repository's own tools write is made canonical at the writer.**
  `tools/gallery-baseline.py` wrote `gallery/baseline/warnings.json` with
  python's default `ensure_ascii=True`, so an em-dash inside a warning pointer
  came out escaped. That is the only reason any of `gallery/baseline/` was
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

That leaves one exemption, and it is not a judgement this file makes. It is a
POINTER to a machine fact that already exists:

    crates/compiler/tests/golden/

Those files are RECORDINGS of emitter output, asserted byte-for-byte by
`golden_scene_matches`; one of them is another program's schema in that
program's key order, so canonical form would break the byte equality the test
exists to make. And the directory's membership is CLOSED by
`every_golden_is_emitter_output`, which refuses any file appearing there that no
test pins to live emitter output.

**Could the defect produce that proof?** No — and that is the whole argument for
this being the only entry. The defect is *nobody canonicalised this document*.
To hide such a document here you would have to make a Rust test demand that it
equal live emitter output byte for byte, which requires the emitter to actually
emit it. Nobody reaches that state by forgetting to run a formatter. Contrast
the exemption this design refused — a marker file, or a name in a list — either
of which a forgetful author can produce in one line.

## Binding count, with its denominator

Every run states files swept **against the tracked-JSON population**, and the
exempt count beside it. Three ways to be vacuous are refused rather than
reported: a population of zero, a swept set of zero (`delvec fmt` itself would
also say `DW0774`), and an exemption that matches zero tracked files — that last
one is a stale entry measuring nothing, which is how an exclusion rots into a
green no-op after its directory is renamed. The pin that admits the exemption is
checked to still exist, so deleting `every_golden_is_emitter_output` reds here
instead of silently converting a bound exemption into an unbound one.

Deterministic, offline, stdlib-only python3.
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

# The one exemption. Not a name this file judges to be generated — a pointer to
# the test that pins these bytes to emitter output, and to the test that closes
# the directory's membership. Both are asserted to still exist below: an
# exemption whose proof has been deleted is an unbound exemption wearing a bound
# one's clothes.
GOLDEN_DIR = "crates/compiler/tests/golden/"
GOLDEN_BINDING_FILE = "crates/compiler/src/view/scene.rs"
GOLDEN_BINDINGS = ("golden_scene_matches", "every_golden_is_emitter_output")

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

    exempt = [p for p in population if p.startswith(GOLDEN_DIR)]
    swept = [p for p in population if p not in set(exempt)]

    if not exempt:
        return die(
            f"the one exemption, `{GOLDEN_DIR}`, matches ZERO tracked files. A "
            "stale exclusion measures nothing and hides the day it stops being "
            "true — delete it, or point it at where the goldens moved."
        )
    binding_src = (repo / GOLDEN_BINDING_FILE).read_text(encoding="utf-8")
    missing = [b for b in GOLDEN_BINDINGS if b not in binding_src]
    if missing:
        return die(
            f"`{GOLDEN_DIR}` is exempt from canonical form ONLY because "
            f"{' and '.join(GOLDEN_BINDINGS)} pin its bytes to emitter output and "
            f"close its membership. {', '.join(missing)} is no longer in "
            f"{GOLDEN_BINDING_FILE}, so the exemption is now bound to nothing."
        )
    if not swept:
        return die(
            "every tracked .json is exempt, so this check formats nothing. That "
            "is vacuous, not a pass."
        )

    print(
        f"canonical-form sweep: {len(swept)} of {len(population)} tracked JSON "
        f"document(s) swept; {len(exempt)} exempt "
        f"(`{GOLDEN_DIR}`, pinned by {' + '.join(GOLDEN_BINDINGS)})."
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
