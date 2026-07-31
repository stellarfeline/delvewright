#!/usr/bin/env python3
"""Bidirectional DW-diagnostic-code consistency check.

Keeps `docs/reference/compiler.md` honest against the Rust source (CLAUDE.md
Methodology): the diagnostics catalog in the reference must list exactly the DW
codes that exist in `crates/**/*.rs` — no more, no less.

- Any DW code in source but missing from the doc  -> FAIL (undocumented behavior).
- Any DW code in the doc but absent from source    -> FAIL (stale doc), unless it
  is declared in PENDING below (approved-but-not-yet-landed surface).
- A PENDING code that has landed in source          -> FAIL (graduate it: turn its
  catalog entry into a normal row and drop it from PENDING).
- A PENDING code not actually documented            -> FAIL (document it).

Deterministic, offline, no dependencies (Python 3 stdlib). Run from the repo root:
    python3 tools/check-dw-codes.py
Exit 0 = consistent, 1 = mismatch (see stderr), 2 = usage/IO error.
"""

import pathlib
import re
import sys

CODE_RE = re.compile(r"DW[0-9]{4}")
REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
DOC_PATH = REPO_ROOT / "docs" / "reference" / "compiler.md"
CRATES_DIR = REPO_ROOT / "crates"

# Codes approved and documented in the reference as "approved, landing" but NOT
# yet present in the source. Remove a code from here the moment it lands in
# crates/**/*.rs (the check below forces this). spec-0010 (#35) has landed:
# DW0211 graduated to a normal catalog row.
PENDING: set[str] = set()


def codes_in(text: str) -> set[str]:
    return set(CODE_RE.findall(text))


def source_codes() -> set[str]:
    found: set[str] = set()
    for rs in sorted(CRATES_DIR.rglob("*.rs")):
        found |= codes_in(rs.read_text(encoding="utf-8"))
    return found


def main() -> int:
    if not DOC_PATH.is_file():
        print(f"error: reference doc not found: {DOC_PATH}", file=sys.stderr)
        return 2
    if not CRATES_DIR.is_dir():
        print(f"error: crates dir not found: {CRATES_DIR}", file=sys.stderr)
        return 2

    src = source_codes()
    doc = codes_in(DOC_PATH.read_text(encoding="utf-8"))

    errors: list[str] = []

    missing_from_doc = sorted(src - doc)
    if missing_from_doc:
        errors.append(
            "DW codes in crates/**/*.rs but MISSING from docs/reference/compiler.md "
            f"(document them): {', '.join(missing_from_doc)}"
        )

    extra_in_doc = sorted(doc - src - PENDING)
    if extra_in_doc:
        errors.append(
            "DW codes in docs/reference/compiler.md but NOT in source and NOT "
            f"declared PENDING (remove or fix): {', '.join(extra_in_doc)}"
        )

    landed_pending = sorted(PENDING & src)
    if landed_pending:
        errors.append(
            "PENDING DW codes have LANDED in source — graduate them (make a normal "
            "catalog row + drop from PENDING in this script): "
            f"{', '.join(landed_pending)}"
        )

    pending_undocumented = sorted(PENDING - doc)
    if pending_undocumented:
        errors.append(
            "PENDING DW codes not documented in the reference (add their "
            f"'approved, landing' entries): {', '.join(pending_undocumented)}"
        )

    if errors:
        print("DW-code consistency check FAILED:", file=sys.stderr)
        for e in errors:
            print(f"  - {e}", file=sys.stderr)
        return 1

    print(
        f"DW-code consistency OK: {len(src)} source codes documented; "
        f"{len(PENDING)} approved-landing (pending)."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
