#!/usr/bin/env python3
"""No two numbered docs claim the same number once this branch merges (#111).

## The defect this exists to end

Specs are numbered `spec-NNNN-<slug>.md` in `docs/specs/`; an authoring agent
picks the next number by LISTING that directory and writing the next integer.
PR #361 created `docs/specs/spec-0033-declared-body-traversal.md` on
2026-08-09 and stayed open. On 2026-08-12 a different agent listed
`docs/specs/` on `main` — where #361's file did not exist yet, because #361
had not merged — got the SAME next number, and merged
`spec-0033-grammar-corpus.md` straight to `main`. Two specs held number 0033
for three days. A human reading PR titles during an unrelated queue audit
caught it; that is not a mechanism.

## What "uniqueness" means here, precisely

Not "no duplicate number in the working tree" — a check shaped that way would
have been green on BOTH branches, every single day of the three, because
**each branch's own `docs/specs/` was internally consistent** (CLAUDE.md: a
check that examines the working tree alone is exactly the vacuous shape this
repo keeps finding). The collision exists only in the UNION of the two trees.
So the question this gate answers is **"would this number still be unique
after this branch merges into `--base`"** (`origin/main` by default): for
every number, take the union of the filenames claiming it in the CURRENT
checkout and the filenames claiming it at `--base`, and fail if that union
names more than one file. One rule, three shapes, without needing to tell
them apart up front:

- **cross-branch** — this branch adds a file `--base` does not have, and
  `--base` independently carries a different file under the same number (the
  #361 / grammar-corpus incident, exactly).
- **local self-collision** — this branch alone adds two differently-named
  files under one number.
- **`--base` self-collision** — a number is already claimed twice at `--base`
  itself, whether or not this branch's checkout touches that series at all
  ("the case where main itself is the thing that moved").

A file present under the SAME name and number on both sides is not a
collision — that is just the file existing before this branch touched it.

## Series covered, and why

`docs/specs/*.md` and `docs/adr/*.md` share the exact exposure and the exact
picking procedure — list the directory, write the next integer — so both are
driven off one generalized SERIES table. Adding a third numbered-by-listing
series later is one entry here, not a new script.

## Series considered and NOT covered here, and why

- **`DEC-NNNN`** (the local decision ledger; one row per id in a single
  markdown table; its own checker already refuses two rows sharing an id
  WITHIN one tree). Deliberately not covered by this script: a `DEC-NNNN` is
  an INSERTION into one shared file, not a new file. Two branches racing the
  same next id at the same table location produce an ordinary textual merge
  conflict — git's own conflict detection is a real first line of defense
  there, which is exactly the defense a brand-new FILE never gets (two new
  files never conflict with each other, which is the entire reason this
  script exists for `spec`/`adr` in the first place). It is a weaker
  guarantee than a real union check — an id appended far from the existing
  rows would not conflict either — so this is a materially different,
  already-partially-mitigated risk, not the same one. Widening this script
  (or a sibling) to cover single-shared-file series is a deliberate follow-up,
  not something to fold in silently here.
- **`DW-NNNN`** diagnostic codes (`crates/**/*.rs`, picked the identical
  "list and pick next" way). Already has ITS OWN dedicated uniqueness gate —
  `tools/check-dw-codes.py`'s "Uniqueness (one code, one rule)" section,
  added after PR #157 shipped `DW0352` into a main that had just given the
  same code to an unrelated rule (#155). That gate operates over the
  CHECKED-OUT tree only; it does not do the explicit `--base` diff this
  script does, so on a PR run it relies on the checked-out tree already BEING
  GitHub's merge-preview commit (head merged onto the base at dispatch time).
  That is very likely true for an ordinary PR run but is not asserted
  anywhere, and if it is ever untrue, `check-dw-codes.py` inherits the exact
  residual gap this script documents below. Flagged here as a candidate
  hardening follow-up — not fixed in this change, which is scoped to
  `spec`/`adr` numbering.

## What this CANNOT catch — read this before trusting a green run

This script diffs the checkout against `--base` AS OF THE MOMENT IT RUNS. It
has no visibility into any OTHER branch or PR that has not merged into
`--base` yet — nothing short of the GitHub API would give it that (and this
repo's CI token deliberately stays `contents: read`, the same stance
`check-required-contexts.py` takes, for the same reason: a gate that needs a
privileged token is a gate that quietly stops running).

- **Two open PRs colliding with each other but not with `origin/main`**:
  invisible to both their CI runs, for as long as neither has merged. This is
  not a hypothetical corner case — it is the EXACT shape of the incident this
  gate exists for, at the moment either PR's CI last ran before the other one
  merged.
- **What closes the gap, and what does not**: once the FIRST of the two
  merges, `origin/main` carries its file. The second PR's CI catches the
  collision the next time it is RE-RUN against the updated base — which
  happens automatically only if branch protection requires a branch be up to
  date before merging (a GitHub setting this script cannot set, for the same
  reason it cannot call the GitHub API), or if its author pushes a new
  commit, rebases, or merges `main` in first. Absent one of those, a stale
  green status can still merge a collision — this narrows the window from
  "forever, until a human reads PR titles" down to "until the next required
  re-run of the second PR"; it does not close the window to zero on its own.
- **A PR whose true base is not `origin/main`** (a stacked branch): `--base`
  defaults to `origin/main` and is not auto-detected from an actual PR base
  ref. Every PR in this repo targets `main` (CLAUDE.md: PR-based flow), so
  this is not exercised today; a stacked-PR workflow would need `--base`
  passed explicitly.
- **A genuine slug-only rename** of an existing, unchanged spec/ADR (same
  number, new filename) reads identically to a real collision — old name only
  at `--base`, new name only locally — and is flagged. This is a deliberate
  false-positive-over-false-negative choice: a rename is rare and its review
  costs one line; a silently swallowed real collision is the three-day bug
  this script exists to end. Renumbering an existing spec/ADR is out of
  policy anyway (CLAUDE.md).

## Binding count

Every run prints, per series, how many numbered files it found in the
checkout and how many at `--base`, plus how many collisions. A series with
zero files on BOTH sides is a FAIL (CLAUDE.md: a green gate that binds to
nothing is VACUOUS) — the directory moved, was renamed, or the pattern no
longer matches; a real run of this repo always has files in both
`docs/specs/` and `docs/adr/`.

## Fetching `--base`

This script performs no network I/O. When the ref is missing it says so and
prints how to get it, computed from THIS repository by `tools/lib/gitbase.py` —
a plain fetch in a full clone, a `--depth=1` fetch in an already-shallow one.
The two are not interchangeable, which is the whole reason that decision is not
made here: `--depth=1` in a full clone shallows it, and a shallow clone answers
ancestry questions with confident wrong numbers rather than with an error.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent / "lib"))

from gitbase import BaseUnresolved, resolve_base  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent

# One entry per numbered-by-directory-listing series. `pattern` has exactly
# one capture group: the number, exactly as an authoring agent would type it
# into a "what's the next number" decision. spec/adr are independent
# namespaces and are never unioned against each other — each series is
# resolved on its own.
SERIES = [
    {
        "series": "spec",
        "dir": "docs/specs",
        "pattern": re.compile(r"^(spec-\d{4})-.+\.md$"),
        "label": lambda num: num,  # already reads as "spec-0033"
    },
    {
        "series": "adr",
        "dir": "docs/adr",
        "pattern": re.compile(r"^(\d{4})-.+\.md$"),
        "label": lambda num: f"ADR-{num}",
    },
]


def git(*args: str) -> str:
    result = subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True)
    if result.returncode != 0:
        raise RuntimeError(f"git {' '.join(args)}: {result.stderr.strip()}")
    return result.stdout


def worktree_numbers(dirname: str, pattern: re.Pattern) -> dict[str, list[str]]:
    """number -> sorted list of `<dirname>/<filename>` claiming it, in the checkout."""
    numbers: dict[str, list[str]] = defaultdict(list)
    d = ROOT / dirname
    if not d.is_dir():
        return numbers
    for p in sorted(d.iterdir()):
        if not p.is_file():
            continue
        m = pattern.match(p.name)
        if m:
            numbers[m.group(1)].append(f"{dirname}/{p.name}")
    return numbers


def base_numbers(base: str, dirname: str, pattern: re.Pattern) -> dict[str, list[str]]:
    """number -> list of `<dirname>/<filename>` claiming it, at git ref `base`."""
    numbers: dict[str, list[str]] = defaultdict(list)
    out = git("ls-tree", "-r", "--name-only", base, "--", dirname)
    for line in out.splitlines():
        line = line.strip()
        if not line:
            continue
        name = line.rsplit("/", 1)[-1]
        m = pattern.match(name)
        if m:
            numbers[m.group(1)].append(line)
    return numbers


def main() -> int:
    parser = argparse.ArgumentParser(
        description="No spec/ADR number collides with --base after this branch merges."
    )
    parser.add_argument(
        "--base",
        default="origin/main",
        help="git ref to diff the checkout against (default: origin/main). "
        "Must already be fetched — this script performs no network I/O itself.",
    )
    args = parser.parse_args()
    base = args.base

    try:
        base_sha = resolve_base(ROOT, base, "check-numbered-doc-uniqueness")
    except BaseUnresolved as unresolved:
        print(unresolved.message, file=sys.stderr)
        return 1

    findings: list[str] = []
    summaries: list[str] = []

    for spec in SERIES:
        local = worktree_numbers(spec["dir"], spec["pattern"])
        base_map = base_numbers(base, spec["dir"], spec["pattern"])
        n_local = sum(len(v) for v in local.values())
        n_base = sum(len(v) for v in base_map.values())

        if n_local == 0 and n_base == 0:
            findings.append(
                f"{spec['series']}: examined 0 files in {spec['dir']!r} on BOTH "
                f"this checkout and {base} — the directory moved, was renamed, "
                f"or the pattern {spec['pattern'].pattern!r} no longer matches "
                f"anything. A check that binds to nothing is not a pass."
            )
            summaries.append(f"{spec['series']}: 0 here, 0 at {base} (VACUOUS)")
            continue

        collisions = 0
        for num in sorted(set(local) | set(base_map)):
            local_names = set(local.get(num, []))
            base_names = set(base_map.get(num, []))
            all_names = local_names | base_names
            if len(all_names) <= 1:
                continue
            collisions += 1
            label = spec["label"](num)
            lines = [
                f"{spec['series']}: {label} is claimed by {len(all_names)} "
                f"different files — this number will NOT be unique once this "
                f"branch merges into {base}:"
            ]
            for name in sorted(all_names):
                where = []
                if name in local_names:
                    where.append("this branch")
                if name in base_names:
                    where.append(base)
                lines.append(f"    {name}  ({' + '.join(where)})")
            lines.append(
                "    Fix: rename ONE of these files to the next number that is "
                f"free on BOTH this branch and {base}, and update anything that "
                "cites its old number (cross-references, the decision ledger). "
                "Never renumber a file the other side is not touching."
            )
            findings.append("\n".join(lines))

        summaries.append(
            f"{spec['series']}: {n_local} here, {n_base} at {base} "
            f"({len(set(local) | set(base_map))} numbers, {collisions} collision(s))"
        )

    if findings:
        print(
            f"check-numbered-doc-uniqueness: {len(findings)} finding(s) against "
            f"{base} @ {base_sha[:12]} — {'; '.join(summaries)}\n",
            file=sys.stderr,
        )
        for f in findings:
            print(f"  {f}\n", file=sys.stderr)
        return 1

    print(
        f"check-numbered-doc-uniqueness: OK — {'; '.join(summaries)}, diffed "
        f"against {base} @ {base_sha[:12]}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
