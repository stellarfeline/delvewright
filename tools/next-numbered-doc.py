#!/usr/bin/env python3
"""Report the next free number for a numbered document class, across EVERY ref.

`check-numbered-doc-uniqueness.py` compares a branch against `origin/main`, and
that is the right question for the check it performs: it catches a document that
collides with something already landed. It cannot catch the collision that
actually happens here, because on `main` neither number is taken yet — two
rounds dispatched at the same time each pick "the next free number", and the
collision only exists once both branches exist.

That is not a defect in the checker. A checker cannot see into a branch nobody
has pushed. The allocation has to happen where the concurrent work is KNOWN
about, which is at dispatch, and this tool is what makes that cheap: it scans
every remote ref, not just the default branch, so a number claimed on an open
branch is claimed.

    tools/next-numbered-doc.py spec        # docs/specs/spec-NNNN-*.md
    tools/next-numbered-doc.py adr         # docs/adr/NNNN-*.md
    tools/next-numbered-doc.py spec --list # every claimed number and its holder

Exit 0 always when it can answer; exit 1 if it cannot enumerate refs, because a
silent fallback to "the next number after main" is the failure this exists to
remove.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys

CLASSES = {
    "spec": ("docs/specs", re.compile(r"spec-(\d{4})-")),
    "adr": ("docs/adr", re.compile(r"^(\d{4})-")),
}


def refs() -> list[str]:
    out = subprocess.run(
        ["git", "for-each-ref", "--format=%(refname)", "refs/remotes/"],
        capture_output=True,
        text=True,
        check=False,
    )
    if out.returncode != 0:
        return []
    return [r for r in out.stdout.split() if not r.endswith("/HEAD")]


def claimed(ref: str, directory: str, pattern: re.Pattern[str]) -> set[str]:
    out = subprocess.run(
        ["git", "ls-tree", "--name-only", ref, directory + "/"],
        capture_output=True,
        text=True,
        check=False,
    )
    if out.returncode != 0:
        return set()
    found = set()
    for line in out.stdout.splitlines():
        name = line.rsplit("/", 1)[-1]
        m = pattern.search(name)
        if m:
            found.add(m.group(1))
    return found


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("klass", choices=sorted(CLASSES))
    ap.add_argument("--list", action="store_true")
    args = ap.parse_args()

    directory, pattern = CLASSES[args.klass]
    all_refs = refs()
    if not all_refs:
        print(
            "next-numbered-doc: could not enumerate remote refs. Refusing to "
            "answer from the default branch alone — that is exactly the reading "
            "that produces a collision. Run `git fetch` and try again.",
            file=sys.stderr,
        )
        return 1

    holders: dict[str, set[str]] = {}
    for ref in all_refs:
        for number in claimed(ref, directory, pattern):
            holders.setdefault(number, set()).add(ref.split("refs/remotes/")[-1])

    if args.list:
        for number in sorted(holders):
            where = sorted(holders[number])
            head = where[0]
            extra = "" if len(where) == 1 else f"  (+{len(where) - 1} more ref(s))"
            print(f"  {number}  {head}{extra}")

    taken = {int(n) for n in holders}
    nxt = (max(taken) + 1) if taken else 1
    print(
        f"next-numbered-doc: {len(all_refs)} ref(s) examined, "
        f"{len(taken)} {args.klass} number(s) claimed, next free is {nxt:04d}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
