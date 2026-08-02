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

## Uniqueness (one code, one rule)

A DW code is a name, and a name that denotes two rules denotes neither:

- A code declared by two different diagnostic constants -> FAIL.
- A code with two diagnostics-catalog rows in the doc   -> FAIL.

This is the parallel-branch collision class: two branches each pick "the next
free code" against the main they branched from, and the merge silently ships one
number for two rules. Every OTHER gate here passes on a colliding pair — both
rules are in source, both are documented, both are tested — so consistency and
coverage cannot see it. Landed after PR #157 shipped `DW0352` for stealth-onset
survivability into a main that had just given `DW0352` to the map editor's
trap-hardware integrity check (#155).

## Test-coverage gate

Every documented, landed (non-PENDING) DW code must be **asserted** by at least
one test in `crates/<crate>/tests/**/*.rs` or inside a `#[cfg(test)]` module in
`crates/<crate>/src/**/*.rs`. Two shapes count:

- a **bare** `"DWxxxx"` string literal — the code standing alone as a value, so
  it is something a test compares, searches for, or tabulates
  (`assert_eq!(d.code, "DW0142")`, `.any(|d| d.code == "DW0311")`,
  `stderr.contains("DW0322")`, an array/tuple table a loop asserts over);
- a **symbolic** diagnostic-code constant (e.g. `pub const DW_STRIP: &str =
  "DW0700";`) referenced somewhere other than a `use` line. Symbol resolution is
  scoped per-crate: two crates may (and do) reuse a constant name for different
  codes (`DW_INPUT` names `DW0710` in `delve-schem` and `DW0732` in
  `delve-admit`), so a name resolves only against the crate that defines it.

What deliberately does **not** count, and why the matcher is shaped this way:

- **comments.** A code named in a `//` / `///` / `//!` / `/* */` comment is
  documentation, not proof. This was the loophole: a `///` doc-comment
  *mentioning* `DW0304` in a test that never touches it read as full coverage,
  and the gate reported green for a rule nothing exercised.
- **prose inside a longer string.** `.expect("must raise DW0313")` names the
  code in a failure *message* while the assertion itself looks at something
  else entirely — the test passes whatever code is raised.
- **`use` lines.** An import is not a use; every real consumer follows it with a
  comparison anyway.

Comment stripping is Rust-string-aware (normal, raw and `r#"…"#` literals are
preserved intact), because test fixtures are full of `//` inside JSON and path
strings.

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


# A DW code standing alone as a string literal — the assertion-shaped form.
BARE_CODE_LITERAL_RE = re.compile(r'"(DW[0-9]{4})"')
USE_LINE_RE = re.compile(r"^\s*use\s[^;]*;", re.MULTILINE)


def codes_in(text: str) -> set[str]:
    return set(CODE_RE.findall(text))


def strip_comments(text: str) -> str:
    """Rust source with every `//`, `///`, `//!` and `/* */` comment removed.

    String-aware: normal (`"…"`, with backslash escapes), raw (`r"…"`) and hashed
    raw (`r#"…"#`, any hash count) literals pass through untouched, so a `//` in a
    JSON fixture or a path string is never mistaken for a comment. Block comments
    nest, as they do in Rust. Comment bodies are replaced by nothing; everything
    else keeps its bytes, so the result is still greppable.
    """
    out: list[str] = []
    i, n = 0, len(text)
    while i < n:
        c = text[i]
        # raw string: r, r#, r##, … followed by a quote
        if c == "r":
            j = i + 1
            while j < n and text[j] == "#":
                j += 1
            if j < n and text[j] == '"':
                hashes = "#" * (j - i - 1)
                close = '"' + hashes
                end = text.find(close, j + 1)
                end = n if end == -1 else end + len(close)
                out.append(text[i:end])
                i = end
                continue
        if c == '"':
            j = i + 1
            while j < n:
                if text[j] == "\\":
                    j += 2
                    continue
                if text[j] == '"':
                    j += 1
                    break
                j += 1
            out.append(text[i:j])
            i = j
            continue
        if c == "'":
            # char literal or a lifetime; only a real char literal can hide a `//`,
            # and it is at most 4 chars — copy it verbatim when it closes.
            j = text.find("'", i + 1)
            if j != -1 and j - i <= 5:
                out.append(text[i : j + 1])
                i = j + 1
                continue
        if text.startswith("//", i):
            j = text.find("\n", i)
            i = n if j == -1 else j
            continue
        if text.startswith("/*", i):
            depth, j = 1, i + 2
            while j < n and depth:
                if text.startswith("/*", j):
                    depth += 1
                    j += 2
                elif text.startswith("*/", j):
                    depth -= 1
                    j += 2
                else:
                    j += 1
            i = j
            continue
        out.append(c)
        i += 1
    return "".join(out)


def assertable_text(text: str) -> str:
    """Test source reduced to what may count as an assertion: comments gone, and
    `use …;` lines dropped so a bare import cannot credit a symbol."""
    return USE_LINE_RE.sub("", strip_comments(text))


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


def declared_constants() -> dict[str, set[tuple[str, str]]]:
    """DW code -> {(crate, constant name)} over every `const NAME: &str = "DWxxxx"`
    in crates/**/*.rs. The source-of-truth view for the uniqueness gate: one code
    declared by two different diagnostic constants means two rules are wearing the
    same number."""
    table: dict[str, set[tuple[str, str]]] = {}
    for rs in sorted(CRATES_DIR.rglob("*.rs")):
        try:
            crate = rs.relative_to(CRATES_DIR).parts[0]
        except ValueError:  # pragma: no cover - rglob always yields children
            continue
        for name, code in CONST_RE.findall(rs.read_text(encoding="utf-8")):
            table.setdefault(code, set()).add((crate, name))
    return table


def catalog_row_counts() -> dict[str, int]:
    """DW code -> number of diagnostics-catalog table rows introducing it in the
    reference (`| `DWxxxx` | …`). A code with two rows documents two rules."""
    counts: dict[str, int] = {}
    row_re = re.compile(r"^\|\s*`(DW[0-9]{4})`\s*\|")
    for line in DOC_PATH.read_text(encoding="utf-8").splitlines():
        m = row_re.match(line)
        if m:
            counts[m.group(1)] = counts.get(m.group(1), 0) + 1
    return counts


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
    """Every DW code **asserted** by test code: a bare `"DWxxxx"` string literal,
    or a per-crate symbolic diagnostic-code constant, in comment-stripped test
    source with `use` lines removed. See the module docstring for what does not
    count and why."""
    found: set[str] = set()
    if not CRATES_DIR.is_dir():
        return found
    for crate_dir in sorted(p for p in CRATES_DIR.iterdir() if p.is_dir()):
        crate = crate_dir.name
        symbols = crate_symbol_table(crate)
        name_res = {name: re.compile(r"\b" + re.escape(name) + r"\b") for name in symbols}
        for raw in crate_test_scope_texts(crate):
            text = assertable_text(raw)
            found |= set(BARE_CODE_LITERAL_RE.findall(text))
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

    # --- uniqueness gate: one code, one rule --------------------------------
    # A DW code is a name, and a name that denotes two rules denotes neither. Two
    # branches developed in parallel will each pick "the next free code" against
    # the main they branched from and collide on merge — silently, because every
    # other gate here is satisfied by a colliding pair (both rules are in source,
    # both are documented, both are tested). This gate is the one that isn't.
    collisions = sorted(
        (code, sorted(owners))
        for code, owners in declared_constants().items()
        if len({name for _, name in owners}) > 1
    )
    for code, owners in collisions:
        where = ", ".join(f"{crate}::{name}" for crate, name in owners)
        errors.append(
            f"{code} is declared by MORE THAN ONE diagnostic constant ({where}) — two "
            "rules are wearing the same code. This is the parallel-branch merge "
            "collision: renumber the one that landed second to the next genuinely "
            "free code (check the merged catalog, not your branch point) across "
            "source, tests, the reference catalog and any content-repo mention"
        )

    dup_rows = sorted(code for code, n in catalog_row_counts().items() if n > 1)
    if dup_rows:
        errors.append(
            "DW codes with MORE THAN ONE diagnostics-catalog row in "
            "docs/reference/compiler.md — one code documents one rule "
            f"(renumber or merge the duplicate row): {', '.join(dup_rows)}"
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
