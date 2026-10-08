#!/usr/bin/env python3
"""Bidirectional DW-diagnostic-code consistency + test-coverage gate.

Keeps the diagnostics catalog honest against the Rust source (CLAUDE.md
Methodology): the catalog must list exactly the DW codes that exist in
`crates/**/*.rs` — no more, no less — and each row must be on the page of the
module that declares its code. It also enforces the CLAUDE.md Conventions rule:
every DW diagnostic must be covered by at least one test asserting its code.

The catalog is read through `tools/lib/dwcatalog.py`, the one reader every gate
that asks about it shares: `docs/reference/compiler.md` §5 holds its shared
rules, and each row sits on `docs/reference/<crate>/<module path>.md`.

## Consistency (bidirectional)

- Any DW code in source with no diagnostics-catalog row -> FAIL (undocumented
  behavior; `documented_codes()` is the one rule, which the staging gate reads).
- Any DW code in the doc but absent from source    -> FAIL (stale doc), unless it
  is declared in PENDING below (approved-but-not-yet-landed surface).
- A PENDING code that has landed in source          -> FAIL (graduate it: turn its
  catalog entry into a normal row and drop it from PENDING).
- A PENDING code not actually documented            -> FAIL (document it).

## Row in page (one record per code, where its declaration is)

A code's row is on the page of the module that declares it —
`dwcatalog.page_for(crate, module_of(file))`, the declaring module computed from
the source file the constant is written in:

- A row on any other page                            -> FAIL (move the row to
  its declaring module's page; a code whose declaration moved and whose row did
  not is this shape).
- A module page under `docs/reference/{delvec,dsl}/` that mirrors no source
  file                                               -> FAIL (the module moved
  or was deleted; its page goes with it).
- With `--delvec`, the registry's `module` field (`module_path!()` at the
  declaration) must name the same module the source reading computed, or an
  inline module declared inside that file (`diagnostic::codes`) -> otherwise
  FAIL: the source reading and the binary disagree about where the code lives.

## Uniqueness (one code, one rule)

A DW code is a name, and a name that denotes two rules denotes neither:

- A code declared by two different diagnostic constants -> FAIL.
- A code with two diagnostics-catalog rows in the doc   -> FAIL.

This is the parallel-branch collision class: two branches each pick "the next
free code" against the main they branched from, and the merge silently ships one
number for two rules. Every OTHER gate here passes on a colliding pair — both
rules are in source, both are documented, both are tested — so consistency and
coverage cannot see it. It has happened: `DW0352` shipped for stealth-onset
survivability into a main that had just given `DW0352` to the map editor's
trap-hardware integrity check.

## Exit tier (bidirectional)

Every `DwCode` declares which tier a hard failure carrying it exits at
(`dsl::diagnostic::ExitTier`), and §1 of the reference carries the table of the
analysis-tier ones. The two are held in lockstep in both directions: a code
declared `ExitTier::Analysis` and missing from the table, or listed in the table
and not declared `Analysis`, is a FAIL. Without this the tier would be a fact
stated twice with nothing comparing them, and the doc half would go stale the
first time a code changed tier — the reader's copy is the one nothing compiles.

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
  scoped per MODULE, the way `rustc` resolves it: modules of the one engine
  crate reuse a constant name for different codes (`DW_INPUT` names `DW0710` in
  `delvec::schem::diag`, `DW0721` in `delvec::compiler::view::diag` and
  `DW0732` in `delvec::admit::diag`), so a name resolves only through the path
  that brings it into scope — the `use delvec::<module>::…::NAME` (or
  `use delvewright_dsl::…`) line in an integration test, a `use super::*` or
  `use crate::…` in a `#[cfg(test)]` module, or a qualified path in the body —
  and `pub use` re-exports are followed. A name nothing imports is not in
  scope and credits nothing.

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

## The binary's registry (`--delvec <path>`)

A code is declared by writing it inside `dw_code!`, which also registers it in
the binary's registry; `delvec codes` prints that registry. Given a built
`delvec`, this gate holds the registry equal to the declarations `CONST_RE`
reads, in both directions, by `(code, constant name)` and by exit tier — so the
source reading here, the doc catalog and the binary all agree. A declaration
written outside `dw_code!` is in the source and absent from the registry, and
reds. `crates/delvec/tests/codes.rs` runs this with the binary cargo built, so
`cargo test --workspace` binds it; without `--delvec` the run prints that the
registry was not compared.

Deterministic, offline, no dependencies (Python 3 stdlib). Run from the repo root:
    python3 tools/ci/check-dw-codes.py
Exit 0 = consistent + covered, 1 = mismatch/gap (see stderr), 2 = usage/IO error.
"""

import json
import pathlib
import re
import subprocess
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

from lib import dwcatalog, mdtable  # noqa: E402
from lib.rust_source import VISIBILITY  # noqa: E402

CODE_RE = dwcatalog.CODE_RE
# A diagnostic-code constant, in either shape the workspace uses:
#
#   pub const L10N_MISSING: DwCode = DwCode::new("DW0180", ExitTier::Build);
#   pub const DW_STRIP: &str = "DW0700";
#
# The `DwCode` form is the campaign-facing one: it carries the exit tier and the
# subject of the rule. The bare `&str` form remains in `delvec schem` /
# `delvec prefab` / `delvec render`, whose diagnostics are about prefabs,
# schematics and renders.
#
# Matching BOTH is load-bearing, not tidiness: this regex is how a symbol name is
# resolved to its code, so a form it does not know silently drops every code
# declared that way out of coverage accounting (the `DwCode` rollout produced
# exactly that — 20 codes reported uncovered that were covered all along).
CONST_RE = re.compile(
    r'const\s+([A-Za-z_][A-Za-z0-9_]*)\s*:\s*(?:&(?:\'static\s+)?str|DwCode)\s*=\s*'
    r'(?:DwCode::new\(\s*)?"(DW[0-9]{4})"'
)
REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
# The page holding the exit-tier table of §1. The catalog itself is every page
# `dwcatalog.catalog_pages` lists.
DOC_PATH = REPO_ROOT / "docs" / "reference" / "compiler.md"
CRATES_DIR = REPO_ROOT / "crates"

# Codes approved and documented in the reference as "approved, landing" but NOT
# yet present in the source. Remove a code from here the moment it lands in
# crates/**/*.rs (the check below forces this). spec-0010 has landed:
# DW0211 graduated to a normal catalog row.
PENDING: set[str] = set()

# Codes exempt from the test-coverage gate. Keep MINIMAL — every entry needs a
# one-line justification; prefer writing the test (see module docstring).
ALLOWLIST: dict[str, str] = {
    # The fidelity gate's missing-texture (magenta) hard-fail is only
    # constructed in `delvec render`'s `run_piece`/`run_fidelity_gate`
    # (crates/delvec/src/render/cli.rs),
    # both of which require a real GPU adapter + the 1.21.11 client jar (never
    # committed — EULA) to actually render a frame first. The detection
    # *algorithm* it wraps (`detect::scan_default`) is unit-tested directly in
    # `crates/delvec/src/compiler/view/detect.rs`'s `#[cfg(test)]` module; the CLI wiring that
    # emits DW0720 from a real render is exercised by
    # `crates/delvec/tests/render_gpu.rs::detector_catches_heavy_core_when_included`,
    # `#[ignore]`d because no GPU/jar is available in CI or this dev sandbox.
    "DW0720": (
        "requires a GPU adapter + the never-committed 1.21.11 client jar "
        "(see crates/delvec/tests/render_gpu.rs, #[ignore]d); the detector algorithm "
        "it wraps is unit-tested in crates/delvec/src/compiler/view/detect.rs"
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


CATALOG_ROW_RE = dwcatalog.CATALOG_ROW_RE
EXIT_TIER_HEADER = dwcatalog.EXIT_TIER_HEADER

# A `DwCode` constant together with the tier it declares. Deliberately separate
# from CONST_RE, which also matches the bare `&str` codes in the tooling
# binaries: those carry no tier, so demanding one of them
# would be a gate asking a question its subject cannot answer.
TIERED_CONST_RE = re.compile(
    r'const\s+([A-Za-z_][A-Za-z0-9_]*)\s*:\s*(?:\w+::)*DwCode\s*=\s*'
    r'(?:\w+::)*DwCode::new\(\s*"(DW[0-9]{4})"'
    r'([^;]*?)\)\s*;'
)
TIER_RE = re.compile(r"ExitTier::(Analysis|Build)")


def declared_tiers() -> tuple[dict[str, str], list[str]]:
    """`(DW code -> declared tier, constants whose tier could not be read)`.

    The second half is the anti-vacuity half: a declaration this parser cannot
    read is reported, never treated as absent. A tier silently dropped here would
    take its code out of the comparison below and the gate would pass by having
    looked at less.
    """
    tiers: dict[str, str] = {}
    unreadable: list[str] = []
    for rs in sorted(CRATES_DIR.rglob("*.rs")):
        for name, code, rest in TIERED_CONST_RE.findall(rs.read_text(encoding="utf-8")):
            m = TIER_RE.search(rest)
            if not m:
                unreadable.append(f"{rs.relative_to(REPO_ROOT)}: {name} ({code})")
                continue
            tiers[code] = m.group(1)
    return tiers, unreadable


def documented_analysis_codes() -> tuple[set[str], int]:
    """`(codes the §1 exit-tier table lists, rows that table holds)`.

    The row count is returned so the caller can refuse a table that has gone
    empty: a comparison against an empty set agrees with a source that declares
    no analysis-tier code at all, which is the one answer this gate must never
    give quietly.
    """
    text = DOC_PATH.read_text(encoding="utf-8")
    rows = [r for r in mdtable.body_rows(text) if r.header == EXIT_TIER_HEADER]
    codes = set()
    for r in rows:
        m = CATALOG_ROW_RE.match(r.line.strip())
        if m:
            codes.add(m.group(1))
    return codes, len(rows)


def catalog_rows() -> tuple[dict[str, int], list[tuple[pathlib.PurePosixPath, int, str]]]:
    """`(DW code -> catalog rows introducing it, catalog rows no table holds)`,
    over every catalog page, read by `dwcatalog`.

    A code with two rows documents two rules, which is a finding. A row that no
    table holds documents nothing at all: a blank line ends a pipe table, so
    such a row renders as a paragraph of literal pipe characters. Twenty-one of
    them were live at once — four detached blocks covering DW0370 through
    DW0499 — when this gate matched a regex against lines; `dwcatalog` reads
    each page through `tools/lib/mdtable.py`, the way its reader does.
    """
    rows, detached = dwcatalog.catalog_rows(REPO_ROOT)
    counts: dict[str, int] = {}
    for row in rows:
        counts[row.code] = counts.get(row.code, 0) + 1
    return counts, detached


def catalog_row_counts() -> dict[str, int]:
    return catalog_rows()[0]


def documented_codes() -> set[str]:
    """The DW codes the catalog documents: those with a catalog row. The one
    rule for "documented" is `dwcatalog.documented_codes`; this gate and the
    staging gate both read it. A code named only in a heading or prose is
    undocumented."""
    return dwcatalog.documented_codes(REPO_ROOT)


def undocumented_source_codes(src: set[str]) -> list[str]:
    """Source codes with no diagnostics-catalog row, sorted."""
    return sorted(src - documented_codes())


def module_of(rs: pathlib.Path, crate: str) -> str:
    """The module path a source file declares — `dwcatalog.module_of`, the
    one rule, over this gate's crates directory."""
    return dwcatalog.module_of(rs, crate, CRATES_DIR)


def declaring_modules() -> dict[str, set[tuple[str, str, pathlib.Path]]]:
    """DW code -> {(crate dir, module, source file)} for every constant
    declaring it in `crates/<crate>/src/**/*.rs`."""
    table: dict[str, set[tuple[str, str, pathlib.Path]]] = {}
    for crate_dir in sorted(p for p in CRATES_DIR.iterdir() if p.is_dir()):
        src = crate_dir / "src"
        if not src.is_dir():
            continue
        for rs in sorted(src.rglob("*.rs")):
            for _name, code in CONST_RE.findall(rs.read_text(encoding="utf-8")):
                table.setdefault(code, set()).add((crate_dir.name, module_of(rs, crate_dir.name), rs))
    return table


def row_in_page_errors() -> tuple[list[str], int, int]:
    """`(findings, rows on their declaring module's page, module pages read)`.

    Every catalog row must be on `page_for` of the module declaring its code,
    and every module page must mirror a source file that exists.
    """
    errors: list[str] = []
    declared = declaring_modules()
    rows, _detached = dwcatalog.catalog_rows(REPO_ROOT)
    placed = 0
    for row in rows:
        owners = declared.get(row.code)
        if not owners:
            continue  # absent from source: the consistency half reports it
        expected = sorted({dwcatalog.page_for(c, m) for c, m, _rs in owners})
        if row.page in expected:
            placed += 1
            continue
        where = ", ".join(
            f"`{m or '(crate root)'}` in {rs.relative_to(REPO_ROOT)}" for _c, m, rs in sorted(owners)
        )
        errors.append(
            f"{row.page}:{row.lineno}: the catalog row for {row.code} is on the wrong page. "
            f"{row.code} is declared by {where}, so its row belongs on "
            f"{' or '.join(str(p) for p in expected)}. Move the row there — a code whose "
            "declaration moved carries its row with it in the same change."
        )
    pages = [p for p in dwcatalog.catalog_pages(REPO_ROOT) if dwcatalog.module_for_page(p)]
    for page in pages:
        crate, module = dwcatalog.module_for_page(page)
        src = CRATES_DIR / crate / "src"
        parts = module.split("::") if module else []
        candidates = (
            [src / pathlib.Path(*parts).with_suffix(".rs"), src / pathlib.Path(*parts) / "mod.rs"]
            if parts
            else [src / "lib.rs", src / "main.rs"]
        )
        if not any(c.is_file() for c in candidates):
            errors.append(
                f"{page} is the page of `{crate}::{module or '(crate root)'}`, and no source file "
                f"declares that module ({', '.join(str(c.relative_to(REPO_ROOT)) for c in candidates)} "
                "are all absent). A page lives at its module's path: move it with the module, "
                "or delete it with the module."
            )
    return errors, placed, len(pages)


def join_module(module: str, *more: str) -> str:
    return "::".join(p for p in (module, *more) if p)


# `use a::b::c;`, `pub use a::b::{c, d as e, self, *};` — the path and, when
# braced, the leaf list. Nested braces are not read (the tree writes none).
USE_RE = re.compile(
    rf"^\s*({VISIBILITY})use\s+([\w:]+?)(?:::(?:\{{([^}}]*)\}}|(\*)))?\s*;",
    re.MULTILINE,
)


def use_leaves(path: str, braces: str, star: str) -> tuple[list[str], list[str]]:
    """`(module prefix, leaves)` of one `use` line: `a::b::c` is prefix `a::b`
    and leaf `c`; `a::b::{c, d}` and `a::b::*` keep `a::b` whole."""
    segs = path.split("::")
    if star:
        return segs, ["*"]
    if braces:
        return segs, [l.strip() for l in braces.split(",") if l.strip()]
    return segs[:-1], [segs[-1]]
# `path::NAME` in a body: a constant named through a module — an imported
# module, or a path from a crate root, `self` or `super`.
QUALIFIED_RE = re.compile(r"\b((?:\w+::)+)([A-Z][A-Z0-9_]*)\b")
# The crate roots a `use` line may open with, mapped to the directory under
# `crates/` that holds the crate.
CRATE_ROOTS = {"delvewright_dsl": "dsl", "delvec": "delvec"}


class Resolver:
    """Constant lookup the way `rustc` resolves it: per MODULE, through re-exports.

    One crate holds every diagnostic module, and two modules of it reuse a
    constant name for different codes (`DW_INPUT` is `DW0710` in `schem::diag`,
    `DW0721` in `compiler::view::diag`, `DW0732` in `admit::diag`), so a table
    keyed by name alone would let one shadow the others and credit a test with
    a code it never asserted. Every constant is therefore tabled under the
    module that declares it, and a name in a test resolves only through the
    path that brings it into scope: the `use` line that imports it, a glob
    (`use super::*`) of the module that declares it, or a qualified path in the
    body. A `pub use` re-export is followed, so `render::diag::DW_INPUT` reaches
    the constant `compiler::view::diag` declares and `schem::blocks::…` reaches
    the format crate's.
    """

    def __init__(self, crates: list[str]) -> None:
        # (crate, module) -> {NAME: code}
        self.decls: dict[tuple[str, str], dict[str, str]] = {}
        # (crate, module-path-as-written) -> (crate, module) it re-exports
        self.module_alias: dict[tuple[str, str], tuple[str, str]] = {}
        # (crate, module, NAME) -> (crate, module, NAME) it re-exports
        self.item_alias: dict[tuple[str, str, str], tuple[str, str, str]] = {}
        # (crate, module) -> modules it glob-re-exports
        self.glob_alias: dict[tuple[str, str], list[tuple[str, str]]] = {}
        for crate in crates:
            src = CRATES_DIR / crate / "src"
            if not src.is_dir():
                continue
            for rs in sorted(src.rglob("*.rs")):
                text = rs.read_text(encoding="utf-8")
                module = module_of(rs, crate)
                table = self.decls.setdefault((crate, module), {})
                for name, code in CONST_RE.findall(text):
                    table[name] = code
                for pub, path, braces, star in USE_RE.findall(text):
                    if not pub:
                        continue
                    self._record_reexport(crate, module, path, braces, star)

    # -- paths -------------------------------------------------------------
    def resolve_path(self, crate: str, module: str | None, segments: list[str]):
        """`(crate, module)` a path names from inside `module` of `crate`, or
        None when it opens with a crate this repo does not table (`std`,
        `serde`, …) or with `crate`/`super` from an integration test, which has
        no module of its own to stand in."""
        if not segments:
            return (crate, module or "")
        root = segments[0]
        if root in CRATE_ROOTS:
            return (CRATE_ROOTS[root], "::".join(segments[1:]))
        if root == "crate":
            return None if module is None else (crate, "::".join(segments[1:]))
        if root in ("self", "super"):
            if module is None:
                return None
            parts = module.split("::") if module else []
            i = 0
            while i < len(segments) and segments[i] in ("self", "super"):
                if segments[i] == "super":
                    parts = parts[:-1]
                i += 1
            return (crate, "::".join(parts + segments[i:]))
        if module is not None and root[:1].islower():
            # a relative path to a sibling module (`pub use block::BlockState;`)
            return (crate, join_module(module, *segments))
        return None

    def _record_reexport(self, crate: str, module: str, path: str, braces: str, star: str) -> None:
        prefix, leaves = use_leaves(path, braces, star)
        base = self.resolve_path(crate, module, prefix)
        if base is None:
            return
        for leaf in leaves:
            leaf, _, alias = leaf.partition(" as ")
            leaf, alias = leaf.strip(), (alias.strip() or leaf.strip())
            if leaf == "*":
                self.glob_alias.setdefault((crate, module), []).append(base)
            elif leaf == "self":
                self.module_alias[(crate, join_module(module, prefix[-1] if not alias or alias == "self" else alias))] = base
            elif leaf[:1].isupper():
                self.item_alias[(crate, module, alias)] = (base[0], base[1], leaf)
            else:
                self.module_alias[(crate, join_module(module, alias))] = (base[0], join_module(base[1], leaf))

    def canon(self, crate: str, module: str, depth: int = 0) -> tuple[str, str]:
        """The declaring module behind a path, following module re-exports."""
        if depth > 8:
            return (crate, module)
        parts = module.split("::") if module else []
        for i in range(len(parts), 0, -1):
            key = (crate, "::".join(parts[:i]))
            if key in self.module_alias:
                c2, m2 = self.module_alias[key]
                return self.canon(c2, join_module(m2, *parts[i:]), depth + 1)
        return (crate, module)

    def lookup(self, crate: str, module: str, name: str, depth: int = 0) -> str | None:
        """The DW code `name` denotes at `crate::module`, or None."""
        if depth > 8:
            return None
        crate, module = self.canon(crate, module)
        code = self.decls.get((crate, module), {}).get(name)
        if code:
            return code
        alias = self.item_alias.get((crate, module, name))
        if alias:
            return self.lookup(*alias, depth + 1)
        for c2, m2 in self.glob_alias.get((crate, module), []):
            code = self.lookup(c2, m2, name, depth + 1)
            if code:
                return code
        return None

    def visible(self, crate: str, module: str, depth: int = 0) -> dict[str, str]:
        """Every constant a glob import of `crate::module` brings into scope."""
        if depth > 8:
            return {}
        crate, module = self.canon(crate, module)
        out: dict[str, str] = {}
        for c2, m2 in self.glob_alias.get((crate, module), []):
            out.update(self.visible(c2, m2, depth + 1))
        for (c, m, name), target in self.item_alias.items():
            if (c, m) == (crate, module):
                code = self.lookup(*target, depth + 1)
                if code:
                    out[name] = code
        out.update(self.decls.get((crate, module), {}))
        return out

    # -- a test's scope ----------------------------------------------------
    def scope_of(self, text: str, crate: str, module: str | None):
        """What the `use` lines of `text` bring into scope, resolved from
        `module` of `crate` (None for an integration test): `(name -> code,
        imported module name -> (crate, module))`."""
        names: dict[str, str] = {}
        modules: dict[str, tuple[str, str]] = {}
        for _pub, path, braces, star in USE_RE.findall(text):
            prefix, leaves = use_leaves(path, braces, star)
            base = self.resolve_path(crate, module, prefix)
            if base is None:
                continue
            for leaf in leaves:
                leaf, _, alias = leaf.partition(" as ")
                leaf, alias = leaf.strip(), (alias.strip() or leaf.strip())
                if leaf == "*":
                    names.update(self.visible(*base))
                elif leaf == "self":
                    modules[prefix[-1] if alias == "self" else alias] = base
                elif leaf[:1].isupper():
                    code = self.lookup(base[0], base[1], leaf)
                    if code:
                        names[alias] = code
                else:
                    modules[alias] = (base[0], join_module(base[1], leaf))
        return names, modules

    def codes_asserted(self, body: str, crate: str, module: str | None, names, modules) -> set[str]:
        """Every code `body` (comment-stripped, `use` lines removed) asserts:
        a bare literal, a name in scope, or a qualified `path::NAME`."""
        found = set(BARE_CODE_LITERAL_RE.findall(body))
        for name, code in names.items():
            if re.search(r"\b" + re.escape(name) + r"\b", body):
                found.add(code)
        for path, name in QUALIFIED_RE.findall(body):
            segs = path.rstrip(":").split("::")
            if segs[0] in modules:
                c, m = modules[segs[0]]
                base = (c, join_module(m, *segs[1:]))
            else:
                base = self.resolve_path(crate, module, segs)
            if base is None:
                continue
            code = self.lookup(base[0], base[1], name)
            if code:
                found.add(code)
        return found


def strip_cfg_test_bodies(text: str) -> str:
    out = text
    for body in cfg_test_module_bodies(text):
        out = out.replace(body, "{}", 1)
    return out


# `#[cfg(test)] mod tests;` at column zero: a test module written as its own file.
OUT_OF_LINE_TEST_MOD_RE = re.compile(
    rf"^#\[cfg\(test\)\]\n{VISIBILITY}mod\s+(\w+)\s*;", re.MULTILINE
)


def out_of_line_test_files(src_dir: pathlib.Path) -> set[pathlib.Path]:
    """Every file under `src_dir` that is an out-of-line `#[cfg(test)]` module,
    or lies under one's directory — test code exactly as an inline module is."""
    roots: list[pathlib.Path] = []
    for rs in sorted(src_dir.rglob("*.rs")):
        here = rs.parent if rs.stem in ("mod", "lib", "main") else rs.with_suffix("")
        for name in OUT_OF_LINE_TEST_MOD_RE.findall(rs.read_text(encoding="utf-8")):
            roots += [here / f"{name}.rs", here / name]
    return {
        rs
        for rs in src_dir.rglob("*.rs")
        if any(rs == r or r in rs.parents for r in roots)
    }


def crate_test_scopes(crate: str) -> list[tuple[str | None, str, str | None]]:
    """Every text that counts as test code for a crate: `(module, body,
    enclosing file without its test bodies)` — module None and file "" for a
    whole file under crates/<crate>/tests/**/*.rs, the declaring module and the
    surrounding file for a `#[cfg(test)]` module body inside
    crates/<crate>/src/**/*.rs, and the file's own module and None for an
    out-of-line `#[cfg(test)]` module's whole file."""
    scopes: list[tuple[str | None, str, str | None]] = []
    tests_dir = CRATES_DIR / crate / "tests"
    if tests_dir.is_dir():
        for rs in sorted(tests_dir.rglob("*.rs")):
            scopes.append((None, rs.read_text(encoding="utf-8"), ""))
    src_dir = CRATES_DIR / crate / "src"
    if src_dir.is_dir():
        whole = out_of_line_test_files(src_dir)
        for rs in sorted(src_dir.rglob("*.rs")):
            text = rs.read_text(encoding="utf-8")
            if rs in whole:
                scopes.append((module_of(rs, crate), text, None))
                continue
            bodies = cfg_test_module_bodies(text)
            if bodies:
                outer = strip_cfg_test_bodies(text)
                for body in bodies:
                    scopes.append((module_of(rs, crate), body, outer))
    return scopes


def tested_codes() -> set[str]:
    """Every DW code **asserted** by test code: a bare `"DWxxxx"` string literal,
    or a diagnostic-code constant resolved through the module that declares it
    — by the `use` line that imports it, by a glob of that module, or by a
    qualified path — in comment-stripped test source with `use` lines removed.
    See the module docstring for what does not count and why."""
    found: set[str] = set()
    if not CRATES_DIR.is_dir():
        return found
    crates = sorted(p.name for p in CRATES_DIR.iterdir() if p.is_dir())
    resolver = Resolver(crates)
    for crate in crates:
        for module, body, outer in crate_test_scopes(crate):
            names: dict[str, str] = {}
            modules: dict[str, tuple[str, str]] = {}
            context = module
            if module is not None and outer is not None:
                # A `#[cfg(test)] mod tests` sits inside the file's module: its
                # own `use` lines resolve from there (`super` is the file's
                # module), and the file's top-level imports are visible to it
                # through the `use super::*` nearly every one carries.
                n, m = resolver.scope_of(outer, crate, module)
                names.update(n)
                modules.update(m)
                context = join_module(module, "tests")
            n, m = resolver.scope_of(body, crate, context)
            names.update(n)
            modules.update(m)
            found |= resolver.codes_asserted(assertable_text(body), crate, context, names, modules)
    return found


# ---------------------------------------------------------------------------
# A remedy a message names owes a check that it is reachable (spec-0060 §10.3)
# ---------------------------------------------------------------------------

# The test that takes each move and asserts it reaches a different verdict.
REMEDY_TEST = REPO_ROOT / "crates" / "delvec" / "tests" / "remedy_reachability.rs"

# **What makes a sentence a MOVE rather than an explanation.**
#
# The rule is spec-0060 §10.3's own quantifier: a message that names *a base or a
# document* as a move. So the marker is the pair — an imperative the message
# addresses to the author, standing near a base name or a file the author edits.
# A message that merely mentions `void` while describing what a horizon is does
# not match, and a move that names neither is not the kind of remedy this gate is
# about (it prescribes nothing an author has to go and find).
# **What bounds the set is this list, and that is a recorded gap.** The verbs are
# enumerated one at a time, so a rule whose imperative is not among them prescribes
# a move this cross-check cannot see. Measured on this tree: adding the four
# imperatives spec-0062's own diagnostics use (`set <field>:`, `lower the volume`,
# `delete the declaration`, `move the anchor`) newly binds **7** codes — DW0317,
# DW0342, DW0450, DW0502, DW0522, DW0850, DW0866 — each of which would then owe a
# row in `remedy_reachability.rs`. That widening is a ledger row, not an
# allowlist entry, and it is not taken here.
MOVE_VERB_RE = re.compile(
    # The numbered form every multi-move message uses...
    r"\((?:1|2|3)\)\s+(?:BURY|PLACE|DECLARE|CHOOSE|RAISE|CORRECT|DELETE|AUTHOR|SEAL)\b"
    # ...and the plain imperative a single-move message uses instead. Both are
    # here because the quantifier is what the message SAYS to do, not how it
    # numbers it: DW0855 names two moves in one sentence and would otherwise
    # have escaped this obligation while being exactly the code that motivated
    # it.
    r"|(?:Give the campaign|[Ss]et `horizon` to|[Rr]e-?author|[Ss]plit the pool)"
)
# **The subject of a move: a backticked field, or a named object** (spec-0062
# §10.8). It began as bases and documents, which is the quantifier spec-0060 §10.3
# wrote — and a quantifier is part of what a check says, so it was worth widening
# once a rule started prescribing a FIELD (`set radius: 3`) rather than a base.
#
# The number this widening newly binds is **0**, measured rather than assumed, and
# the measurement is the finding: the subject was never the limiting term. Every
# message in this repository that names a base or a document also names it in a
# backtick, so widening the subject binds the same 7 codes. What bounds the set is
# `MOVE_VERB_RE`, whose imperatives are enumerated one at a time — see the note on
# it. `codes_that_prescribe_a_move` prints its count so that a later widening on
# either side is a number a reader can compare against this one.
MOVE_SUBJECT_RE = re.compile(
    r"`[A-Za-z_][A-Za-z0-9_.:\- ]*`|site plan|\{base\}\.json|"
    r"`\{base_file\}\.json`|`\{file\}\.json`"
)

# How far after a constant's name a message is still that constant's message.
# Generous on purpose: over-reaching pulls a code into the obligation, which is a
# red somebody reads, while under-reaching drops one silently.
MOVE_WINDOW = 6000


# A `pub const NAME:` whose value is NOT a DW code. Its only use is to find the
# names a bare-name scan cannot resolve.
NON_CODE_CONST_RE = re.compile(r"(?m)^\s*pub const ([A-Z][A-Z0-9_]*)\s*:(?![^\n]*\"DW[0-9]{4}\")")


def ambiguous_code_names(code_names: set[str]) -> set[str]:
    """Diagnostic-constant names the tree also uses for something else.

    `DSL_VERSION` names both `DW0102` and the campaign format's number. A scan
    for the bare name cannot tell them apart, so those names — and only those —
    are matched through `codes::<NAME>`.
    """
    out: set[str] = set()
    for rs in sorted(CRATES_DIR.rglob("*.rs")):
        for name in NON_CODE_CONST_RE.findall(rs.read_text(encoding="utf-8")):
            if name in code_names:
                out.add(name)
    return out


def codes_that_prescribe_a_move() -> dict[str, set[str]]:
    """`DW code -> {file:line}` for every message that names a base or a document
    as a move.

    Resolved through the code's own constant rather than by looking for a
    `DWxxxx` literal near the text: a diagnostic's message never repeats its own
    number, so a literal-based reading would bind to nothing and report a clean
    zero.

    **A name that is not unique in the tree is matched through its qualified
    path** (CLAUDE.md: a resolve-by-name over a scope where names are not unique
    yields a candidate, not a match). `codes::DSL_VERSION` is `DW0102`;
    `delvewright_dsl::DSL_VERSION` is the campaign format's number, and a
    bare-name scan reads every mention of the second as a mention of the first,
    then blames `DW0102` for whatever move happens to be written within 6000
    characters of it — which is exactly what it did the day the tests stopped
    typing that number out and started deriving it. `SCHEMA` is the other such
    pair today. The qualifier is applied ONLY to the ambiguous names, computed
    from the tree by `ambiguous_code_names()`: applying it to all of them drops
    the binding from 7 codes to 2, because several diagnostics cite a bare
    `&str` constant rather than a `codes::` path, and that would be a loosening
    wearing a precision's clothes.
    """
    consts = declared_constants()
    by_name: dict[str, str] = {}
    for code, pairs in consts.items():
        for _crate, name in pairs:
            by_name[name] = code
    ambiguous = ambiguous_code_names(set(by_name))
    found: dict[str, set[str]] = {}
    for rs in sorted(CRATES_DIR.rglob("*.rs")):
        raw = rs.read_text(encoding="utf-8")
        text = strip_comments(raw)
        for name, code in by_name.items():
            pat = (r"\bcodes::" if name in ambiguous else r"\b") + re.escape(name) + r"\b"
            for m in re.finditer(pat, text):
                window = text[m.end() : m.end() + MOVE_WINDOW]
                # Stop at the next code constant: a window that runs into the
                # next diagnostic would attribute its moves to this one.
                # The window is ONE diagnostic's message. It ends at whatever
                # comes first: the next code constant, or the construction of
                # the next diagnostic — because a window that runs past either
                # attributes another rule's moves to this one, which is how
                # DW0320 first appeared in this set.
                cut = len(window)
                for other in by_name:
                    if other == name:
                        continue
                    hit = window.find(f"codes::{other}" if other in ambiguous else other)
                    if hit != -1:
                        cut = min(cut, hit)
                for boundary in (
                    "Diagnostic::error(",
                    "Diagnostic::warning(",
                    "PlanError::new(",
                    "Failure {",
                ):
                    hit = window.find(boundary)
                    if hit != -1:
                        cut = min(cut, hit)
                window = window[:cut]
                if MOVE_VERB_RE.search(window) and MOVE_SUBJECT_RE.search(window):
                    # The constant's NAME, not a line number: the offsets above
                    # are into comment-stripped source, so a line taken from
                    # them names a line in a file nobody has. A name resolves.
                    found.setdefault(code, set()).add(
                        f"{rs.relative_to(REPO_ROOT)} ({name})"
                    )
    return found


def registry_rows(delvec: str) -> list[dict]:
    """`delvec codes`, one JSON object per line. A run that does not answer is a
    refusal, never an empty registry."""
    proc = subprocess.run([delvec, "codes"], capture_output=True, text=True)
    if proc.returncode != 0:
        raise SystemExit(f"error: `{delvec} codes` exited {proc.returncode}: {proc.stderr.strip()}")
    rows = [json.loads(line) for line in proc.stdout.splitlines() if line.strip()]
    if not rows:
        raise SystemExit(f"error: `{delvec} codes` listed no code")
    return rows


def registry_errors(
    rows: list[dict],
    constants: dict[str, set[tuple[str, str]]],
    tiers: dict[str, str],
) -> list[str]:
    """The binary's registry against the declarations, in both directions."""
    errors: list[str] = []
    registered = {(r["code"], r["name"]) for r in rows}
    declared = {(code, name) for code, owners in constants.items() for _crate, name in owners}
    unregistered = sorted(declared - registered)
    if unregistered:
        errors.append(
            "DW codes declared in source and ABSENT from `delvec codes` — declare "
            "them inside `dw_code!`, which is what registers them: "
            + ", ".join(f"{c} ({n})" for c, n in unregistered)
        )
    undeclared = sorted(registered - declared)
    if undeclared:
        errors.append(
            "DW codes `delvec codes` lists that no source declaration spells the way "
            "CONST_RE reads it: " + ", ".join(f"{c} ({n})" for c, n in undeclared)
        )
    for r in rows:
        if r["code"] in tiers and r["tier"] != tiers[r["code"]]:
            errors.append(
                f"{r['code']} ({r['name']}) is tier {r['tier']} in `delvec codes` "
                f"and {tiers[r['code']]} in source"
            )
    return errors


def registry_module_errors(
    rows: list[dict], declared: dict[str, set[tuple[str, str, pathlib.Path]]]
) -> tuple[list[str], int]:
    """`(findings, codes whose module agrees)`: the registry's `module` field
    against the module the source reading computed for the same code.

    `module` is `module_path!()` where `dw_code!` expands, so it names the
    crate (`delvewright_dsl`, `delvec`), raw identifiers spelled `r#loop`, and
    any inline module the constant sits in (`diagnostic::codes`). It agrees
    when it is the file's module, or that module followed by inline modules
    the file itself declares.
    """
    errors: list[str] = []
    agreed = 0
    for r in rows:
        owners = declared.get(r["code"])
        if not owners:
            continue  # a code no source declares: registry_errors reports it
        segs = [x.removeprefix("r#") for x in str(r.get("module") or "").split("::")]
        crate = CRATE_ROOTS.get(segs[0])
        ok = False
        for c, m, rs in owners:
            mparts = m.split("::") if m else []
            if crate != c or segs[1 : 1 + len(mparts)] != mparts:
                continue
            rest = segs[1 + len(mparts) :]
            text = rs.read_text(encoding="utf-8")
            if all(re.search(r"\bmod\s+(?:r#)?" + re.escape(x) + r"\s*\{", text) for x in rest):
                ok = True
                break
        if ok:
            agreed += 1
            continue
        where = ", ".join(f"{c}::{m or '(crate root)'} ({rs.relative_to(REPO_ROOT)})" for c, m, rs in sorted(owners))
        errors.append(
            f"{r['code']} ({r.get('name')}): `delvec codes` says it is declared in "
            f"`{r.get('module')}`, and the source reading says {where}. The page its row "
            "is held to is computed from the source reading, so the two must agree."
        )
    return errors, agreed


def main(argv: list[str] | None = None) -> int:
    argv = sys.argv[1:] if argv is None else argv
    delvec = None
    if argv[:1] == ["--delvec"] and len(argv) == 2:
        delvec = argv[1]
    elif argv:
        print("usage: check-dw-codes.py [--delvec <path to a built delvec>]", file=sys.stderr)
        return 2
    if not DOC_PATH.is_file():
        print(f"error: reference doc not found: {DOC_PATH}", file=sys.stderr)
        return 2
    pages = dwcatalog.catalog_pages(REPO_ROOT)
    if not CRATES_DIR.is_dir():
        print(f"error: crates dir not found: {CRATES_DIR}", file=sys.stderr)
        return 2

    src = source_codes()
    doc = dwcatalog.mentioned_codes(REPO_ROOT)

    errors: list[str] = []

    missing_from_doc = undocumented_source_codes(src)
    if missing_from_doc:
        errors.append(
            "DW codes in crates/**/*.rs with no diagnostics-catalog row on any "
            "catalog page (a heading or prose mention is not a row; add one on the "
            f"declaring module's page, docs/reference/<crate>/<module path>.md): {', '.join(missing_from_doc)}"
        )

    extra_in_doc = sorted(doc - src - PENDING)
    if extra_in_doc:
        errors.append(
            "DW codes named on a catalog page but NOT in source and NOT "
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

    row_counts, detached_rows = catalog_rows()
    for page, lineno, line in detached_rows:
        errors.append(
            f"{page}:{lineno} is a diagnostics-catalog row "
            f"that no table contains:\n    {line[:100]}\n    A blank line ends a "
            "pipe table, so this row renders as a paragraph of literal pipe "
            "characters and documents nothing to anyone reading the page. Delete "
            "the blank line above it so it rejoins the catalog table."
        )

    dup_rows = sorted(code for code, n in row_counts.items() if n > 1)
    if dup_rows:
        errors.append(
            "DW codes with MORE THAN ONE diagnostics-catalog row across the "
            "catalog pages — one code documents one rule "
            f"(renumber or merge the duplicate row): {', '.join(dup_rows)}"
        )

    # --- row in page: one record per code, at its declaring module's path -----
    page_errors, placed, module_pages = row_in_page_errors()
    errors += page_errors
    if placed == 0:
        errors.append(
            "row-in-page bound ZERO catalog rows to their declaring module's page. The "
            "catalog is never empty, so this is the reader having stopped finding rows "
            "(tools/lib/dwcatalog.py) — not a pass"
        )

    # --- exit tier: source and reference in lockstep -------------------------
    tiers, unreadable_tiers = declared_tiers()
    for where in unreadable_tiers:
        errors.append(
            f"a DwCode constant declares no readable ExitTier ({where}) — the "
            "tier is what decides the process exit status, so a declaration this "
            "gate cannot read is a code it cannot judge"
        )
    documented_analysis, tier_rows = documented_analysis_codes()
    if tier_rows == 0:
        errors.append(
            "the exit-tier table in docs/reference/compiler.md §1 holds no rows "
            f"(expected a table headed {' | '.join(EXIT_TIER_HEADER)}) — this gate "
            "would otherwise compare every declared tier against an empty set and "
            "pass by binding to nothing"
        )
    else:
        declared_analysis = {c for c, tier in tiers.items() if tier == "Analysis"}
        undocumented_tier = sorted(declared_analysis - documented_analysis)
        if undocumented_tier:
            errors.append(
                "DW codes declared ExitTier::Analysis in source but MISSING from the "
                "exit-tier table in docs/reference/compiler.md §1 (add a row saying "
                f"what the author changes): {', '.join(undocumented_tier)}"
            )
        overdocumented_tier = sorted(documented_analysis - declared_analysis)
        if overdocumented_tier:
            errors.append(
                "DW codes listed as analysis tier in docs/reference/compiler.md §1 "
                "that source does NOT declare ExitTier::Analysis (the table is stale "
                f"— they exit 3): {', '.join(overdocumented_tier)}"
            )

    # --- test-coverage gate -------------------------------------------------
    stale_allowlist = sorted(set(ALLOWLIST) - (doc - PENDING))
    if stale_allowlist:
        errors.append(
            "ALLOWLIST entries that are not live documented codes (remove them): "
            f"{', '.join(stale_allowlist)}"
        )

    # --- a named remedy owes a check that it is reachable -------------------
    #
    # `CLAUDE.md`: *a gate that names a remedy owes a check that the remedy is
    # reachable*. Nothing held that check, and the cost was the cycle spec-0060
    # §1 walks — three gates each naming as its remedy a base the next refuses.
    # So a message that names a base or a document as a move owes a row in
    # `remedy_reachability.rs`, which builds the campaign that takes the move and
    # asserts it reaches a different verdict.
    prescribing = codes_that_prescribe_a_move()
    ambiguous = ambiguous_code_names({n for pairs in declared_constants().values() for _c, n in pairs})
    print(
        f"remedy cross-check binding: {len(prescribing)} code(s) whose message names a "
        f"backticked field or a named object as a MOVE, out of {len(src)} in source "
        f"({', '.join(sorted(prescribing)) or 'none'}); "
        f"{len(ambiguous)} constant name(s) the tree also uses for something else "
        f"({', '.join(sorted(ambiguous)) or 'none'}) matched through `codes::`"
    )
    if not prescribing:
        errors.append(
            "the remedy cross-check matched ZERO diagnostics that name a base or a document "
            "as a move. That is not a pass: this repository has several, so a zero here means "
            "the reader stopped matching them (tools/ci/check-dw-codes.py, MOVE_VERB_RE)"
        )
    remedy_text = REMEDY_TEST.read_text(encoding="utf-8") if REMEDY_TEST.is_file() else ""
    if not remedy_text:
        errors.append(
            f"the remedy-reachability test is missing: {REMEDY_TEST.relative_to(REPO_ROOT)}"
        )
    unproven = sorted(c for c in prescribing if c not in codes_in(remedy_text))
    if unproven:
        errors.append(
            "DW codes whose message names a base or a document as a MOVE, with no row in "
            f"{REMEDY_TEST.relative_to(REPO_ROOT)} taking that move and asserting a different "
            "verdict — a remedy nobody has ever taken is a remedy nobody knows is reachable: "
            + ", ".join(f"{c} ({', '.join(sorted(prescribing[c]))})" for c in unproven)
        )

    tested = tested_codes()
    requires_test = doc - PENDING
    untested = sorted(requires_test - tested - set(ALLOWLIST))
    if untested:
        errors.append(
            "DW codes with NO test coverage (write a test asserting the code, or "
            "add a justified ALLOWLIST entry in tools/ci/check-dw-codes.py): "
            f"{', '.join(untested)}"
        )

    allowlisted_but_tested = sorted(set(ALLOWLIST) & tested)
    if allowlisted_but_tested:
        errors.append(
            "ALLOWLIST entries that now HAVE test coverage — drop them from the "
            f"allowlist: {', '.join(allowlisted_but_tested)}"
        )

    registry_note = (
        "registry NOT compared (no --delvec; `cargo test` compares it through "
        "crates/delvec/tests/codes.rs)"
    )
    if delvec is not None:
        rows = registry_rows(delvec)
        errors += registry_errors(rows, declared_constants(), tiers)
        module_errors, agreed = registry_module_errors(rows, declaring_modules())
        errors += module_errors
        registry_note = (
            f"registry: `delvec codes` lists {len(rows)} code(s), {agreed} of them in the "
            "module the source reading computed"
        )

    if errors:
        print("DW-code consistency check FAILED:", file=sys.stderr)
        for e in errors:
            print(f"  - {e}", file=sys.stderr)
        return 1

    print(
        f"DW-code consistency OK: {len(src)} source codes documented; "
        f"{placed} catalog row(s) on their declaring module's page, over "
        f"{len(pages)} catalog page(s) ({module_pages} module pages); "
        f"{len(PENDING)} approved-landing (pending); "
        f"{len(requires_test)} require tests, all covered "
        f"({len(ALLOWLIST)} allowlisted); "
        f"{len(prescribing)} code(s) name a base or a document as a move, all with a row in "
        f"{REMEDY_TEST.name}; "
        f"{len(tiers)} exit tiers declared, "
        f"{len([c for c, x in tiers.items() if x == 'Analysis'])} of them analysis "
        f"tier, matching {tier_rows} documented row(s); {registry_note}."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
