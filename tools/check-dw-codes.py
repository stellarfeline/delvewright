#!/usr/bin/env python3
"""Bidirectional DW-diagnostic-code consistency + test-coverage gate.

Keeps `docs/reference/compiler.md` honest against the Rust source (CLAUDE.md
Methodology): the diagnostics catalog in the reference must list exactly the DW
codes that exist in `crates/**/*.rs` — no more, no less. It also enforces the
CLAUDE.md Conventions rule (owner, 2026-07-31): every DW diagnostic must be
covered by at least one test asserting its code.

## Consistency (bidirectional)

- Any DW code in source but missing from the doc  -> FAIL (undocumented behavior).
- Any DW code in the doc but absent from source    -> FAIL (stale doc), unless it
  is declared in PENDING below (approved-but-not-yet-landed surface).
- A PENDING code that has landed in source          -> FAIL (graduate it: turn its
  catalog entry into a normal row and drop it from PENDING).
- A PENDING code not actually documented            -> FAIL (document it).

## Test-coverage gate

Every documented, landed (non-PENDING) DW code must be referenced by at least
one test: either the literal code string, or a symbolic diagnostic-code
constant (e.g. `pub const DW_STRIP: &str = "DW0700";`) that resolves to it,
appearing in `crates/<crate>/tests/**/*.rs` or inside a `#[cfg(test)]` module
in `crates/<crate>/src/**/*.rs`. Symbol resolution is scoped per-crate: two
crates may (and do) reuse the same constant name for different codes (e.g.
`DW_INPUT` names `DW0710` in `delve-schem` and `DW0732` in `delve-admit`), so a
name is only resolved against the crate it is defined in.

A code with no reachable test may be declared in ALLOWLIST below, but every
entry needs a one-line justification — keep this list minimal; prefer writing
the test (CLAUDE.md debug doctrine: a red check is information, not an
obstacle to route around).

Deterministic, offline, no dependencies (Python 3 stdlib). Run from the repo root:
    python3 tools/check-dw-codes.py
Exit 0 = consistent + covered, 1 = mismatch/gap (see stderr), 2 = usage/IO error.
"""

import pathlib
import re
import sys

CODE_RE = re.compile(r"DW[0-9]{4}")
CONST_RE = re.compile(
    r'const\s+([A-Za-z_][A-Za-z0-9_]*)\s*:\s*&(?:\'static\s+)?str\s*=\s*"(DW[0-9]{4})"'
)
REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
DOC_PATH = REPO_ROOT / "docs" / "reference" / "compiler.md"
CRATES_DIR = REPO_ROOT / "crates"

# Codes approved and documented in the reference as "approved, landing" but NOT
# yet present in the source. Remove a code from here the moment it lands in
# crates/**/*.rs (the check below forces this). spec-0010 (#35) has landed:
# DW0211 graduated to a normal catalog row.
PENDING: set[str] = set()

# Codes exempt from the test-coverage gate. Keep MINIMAL — every entry needs a
# one-line justification; prefer writing the test (see module docstring).
ALLOWLIST: dict[str, str] = {
    # The fidelity gate's missing-texture (magenta) hard-fail is only
    # constructed in `delve-render`'s `run_piece`/`run_fidelity_gate` (main.rs),
    # both of which require a real GPU adapter + the 1.21.11 client jar (never
    # committed — EULA) to actually render a frame first. The detection
    # *algorithm* it wraps (`detect::scan_default`) is unit-tested directly in
    # `crates/render/src/detect.rs`'s `#[cfg(test)]` module; the CLI wiring that
    # emits DW0720 from a real render is exercised by
    # `crates/render/tests/gpu.rs::detector_catches_heavy_core_when_included`,
    # `#[ignore]`d because no GPU/jar is available in CI or this dev sandbox.
    "DW0720": (
        "requires a GPU adapter + the never-committed 1.21.11 client jar "
        "(see crates/render/tests/gpu.rs, #[ignore]d); the detector algorithm "
        "it wraps is unit-tested in crates/render/src/detect.rs"
    ),
}


def codes_in(text: str) -> set[str]:
    return set(CODE_RE.findall(text))


def source_codes() -> set[str]:
    found: set[str] = set()
    for rs in sorted(CRATES_DIR.rglob("*.rs")):
        found |= codes_in(rs.read_text(encoding="utf-8"))
    return found


def cfg_test_module_bodies(text: str) -> list[str]:
    """Extract the `{ ... }` body of every `#[cfg(test)] mod ... { }` block via
    brace counting (good enough for a text-scan gate; not a Rust parser)."""
    bodies: list[str] = []
    marker = "#[cfg(test)]"
    idx = 0
    while True:
        pos = text.find(marker, idx)
        if pos == -1:
            break
        mod_pos = text.find("mod", pos)
        if mod_pos == -1:
            break
        brace_pos = text.find("{", mod_pos)
        if brace_pos == -1:
            break
        depth = 0
        i = brace_pos
        n = len(text)
        while i < n:
            if text[i] == "{":
                depth += 1
            elif text[i] == "}":
                depth -= 1
                if depth == 0:
                    break
            i += 1
        bodies.append(text[brace_pos : i + 1])
        idx = i + 1 if i < n else n
    return bodies


def crate_symbol_table(crate: str) -> dict[str, str]:
    """Map constant name -> DW code for every `const NAME: &str = "DWxxxx"` in
    crates/<crate>/src/**/*.rs (any visibility)."""
    table: dict[str, str] = {}
    src_dir = CRATES_DIR / crate / "src"
    if not src_dir.is_dir():
        return table
    for rs in sorted(src_dir.rglob("*.rs")):
        for name, code in CONST_RE.findall(rs.read_text(encoding="utf-8")):
            table[name] = code
    return table


def crate_test_scope_texts(crate: str) -> list[str]:
    """Every text blob that counts as 'test code' for a crate: whole files
    under crates/<crate>/tests/**/*.rs, plus #[cfg(test)] module bodies inside
    crates/<crate>/src/**/*.rs."""
    texts: list[str] = []
    tests_dir = CRATES_DIR / crate / "tests"
    if tests_dir.is_dir():
        for rs in sorted(tests_dir.rglob("*.rs")):
            texts.append(rs.read_text(encoding="utf-8"))
    src_dir = CRATES_DIR / crate / "src"
    if src_dir.is_dir():
        for rs in sorted(src_dir.rglob("*.rs")):
            texts.extend(cfg_test_module_bodies(rs.read_text(encoding="utf-8")))
    return texts


def tested_codes() -> set[str]:
    """Every DW code referenced by test code, resolving per-crate symbolic
    diagnostic-code constants as well as literal code strings."""
    found: set[str] = set()
    if not CRATES_DIR.is_dir():
        return found
    for crate_dir in sorted(p for p in CRATES_DIR.iterdir() if p.is_dir()):
        crate = crate_dir.name
        symbols = crate_symbol_table(crate)
        name_res = {name: re.compile(r"\b" + re.escape(name) + r"\b") for name in symbols}
        for text in crate_test_scope_texts(crate):
            found |= codes_in(text)
            for name, pattern in name_res.items():
                if pattern.search(text):
                    found.add(symbols[name])
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

    # --- test-coverage gate -------------------------------------------------
    stale_allowlist = sorted(set(ALLOWLIST) - (doc - PENDING))
    if stale_allowlist:
        errors.append(
            "ALLOWLIST entries that are not live documented codes (remove them): "
            f"{', '.join(stale_allowlist)}"
        )

    tested = tested_codes()
    requires_test = doc - PENDING
    untested = sorted(requires_test - tested - set(ALLOWLIST))
    if untested:
        errors.append(
            "DW codes with NO test coverage (write a test asserting the code, or "
            "add a justified ALLOWLIST entry in tools/check-dw-codes.py): "
            f"{', '.join(untested)}"
        )

    allowlisted_but_tested = sorted(set(ALLOWLIST) & tested)
    if allowlisted_but_tested:
        errors.append(
            "ALLOWLIST entries that now HAVE test coverage — drop them from the "
            f"allowlist: {', '.join(allowlisted_but_tested)}"
        )

    if errors:
        print("DW-code consistency check FAILED:", file=sys.stderr)
        for e in errors:
            print(f"  - {e}", file=sys.stderr)
        return 1

    print(
        f"DW-code consistency OK: {len(src)} source codes documented; "
        f"{len(PENDING)} approved-landing (pending); "
        f"{len(requires_test)} require tests, all covered "
        f"({len(ALLOWLIST)} allowlisted)."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
