"""The DW diagnostics catalog, read once for every gate that asks about it.

## What the catalog is

Every DW code has one catalog row, a pipe-table row whose first cell is the
code in backticks, and the row lives on the reference page of the module that
declares the code. The page for a module is at the module's path: strip the
crate root, replace `::` with `/`, append `.md`, under `docs/reference/<crate>/`
where `<crate>` is the directory under `crates/` (`dsl`, `delvec`). A code
declared in `crates/delvec/src/compiler/nav.rs` has its row in
`docs/reference/delvec/compiler/nav.md`; one declared in a crate root
(`lib.rs`, `main.rs`) has it in `docs/reference/<crate>.md`, the way Rust puts
`foo.rs` beside `foo/`. `docs/reference/compiler.md` §5 holds the catalog's
shared rules and no rows.

## Why one reader

`check-dw-codes`, `staging-gate` and `check-reference-versions` each ask the
catalog a question. Each used to read `compiler.md` itself, and a gate that
reads a document privately is a gate that goes on reading the old document when
the catalog moves. Every question about rows goes through here; a row is read
by `mdtable`, the way its renderer reads it, so a row a blank line has detached
from its table is reported rather than counted.

What this module does NOT decide is where a row ought to be: that is
`check-dw-codes`' row-in-page rule, which compares each row's page to
`page_for` of the module that declares its code.
"""

from __future__ import annotations

import pathlib
import re
from dataclasses import dataclass

from lib import mdtable

__all__ = [
    "CatalogRow",
    "CATALOG_ROW_RE",
    "EXIT_TIER_HEADER",
    "REPO_ROOT",
    "catalog_pages",
    "catalog_rows",
    "documented_codes",
    "mentioned_codes",
    "module_of",
    "module_for_page",
    "page_for",
    "row_counts",
]

REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
REFERENCE = pathlib.PurePosixPath("docs/reference")
#: The page that holds the catalog's shared rules. It holds no row; it is read
#: so that a row left behind in it is a row the row-in-page rule can see.
COMPILER_MD = REFERENCE / "compiler.md"
#: The crates whose modules have pages: the directory under `crates/`.
CRATES = ("delvec", "dsl")

CODE_RE = re.compile(r"DW[0-9]{4}")
CATALOG_ROW_RE = re.compile(r"^\|\s*`(DW[0-9]{4})`\s*\|")

# The exit-tier table in compiler.md §1. Its rows are code-shaped, so the
# catalog reader has to know it is not the catalog — identified by its header,
# positively, and the identification is self-protecting: rename the header and
# its codes read as catalog rows on a page no module owns, which the row-in-page
# rule reds.
EXIT_TIER_HEADER = ("Code", "What the author changes")


@dataclass(frozen=True)
class CatalogRow:
    """One catalog row: its code, the page it is on (repo-relative), its line."""

    code: str
    page: pathlib.PurePosixPath
    lineno: int
    line: str
    header: tuple[str, ...]


def module_of(rs: pathlib.Path, crate: str, crates_dir: pathlib.Path | None = None) -> str:
    """The module path a source file declares: `src/a/b.rs` is `a::b`,
    `src/a/mod.rs` is `a`, `src/lib.rs` and `src/main.rs` are the root."""
    crates_dir = crates_dir or REPO_ROOT / "crates"
    rel = rs.relative_to(crates_dir / crate / "src").with_suffix("")
    parts = list(rel.parts)
    if parts and parts[-1] in ("mod", "lib", "main"):
        parts = parts[:-1]
    return "::".join(parts)


def page_for(crate: str, module: str) -> pathlib.PurePosixPath:
    """The page of `crate`'s `module`, repo-relative: the one mapping."""
    if not module:
        return REFERENCE / f"{crate}.md"
    return REFERENCE / crate / ("/".join(module.split("::")) + ".md")


def module_for_page(page: pathlib.PurePosixPath) -> tuple[str, str] | None:
    """`(crate, module)` a catalog page stands for, or None for a page outside
    the module tree (`compiler.md`)."""
    parts = page.relative_to(REFERENCE).with_suffix("").parts
    if len(parts) == 1 and parts[0] in CRATES:
        return parts[0], ""
    if len(parts) >= 2 and parts[0] in CRATES:
        return parts[0], "::".join(parts[1:])
    return None


def catalog_pages(root: pathlib.Path = REPO_ROOT) -> list[pathlib.PurePosixPath]:
    """`compiler.md`, then every module page, sorted, repo-relative."""
    pages = [COMPILER_MD] if (root / COMPILER_MD).is_file() else []
    for crate in CRATES:
        top = root / REFERENCE / f"{crate}.md"
        if top.is_file():
            pages.append(REFERENCE / f"{crate}.md")
        tree = root / REFERENCE / crate
        if tree.is_dir():
            pages += sorted(
                pathlib.PurePosixPath(p.relative_to(root).as_posix()) for p in tree.rglob("*.md")
            )
    return pages


def catalog_rows(
    root: pathlib.Path = REPO_ROOT,
) -> tuple[list[CatalogRow], list[tuple[pathlib.PurePosixPath, int, str]]]:
    """`(catalog rows on every page, catalog-shaped rows no table holds)`.

    A row no table holds documents nothing: a blank line ends a pipe table, so
    it renders as a paragraph of literal pipe characters.
    """
    rows: list[CatalogRow] = []
    detached: list[tuple[pathlib.PurePosixPath, int, str]] = []
    for page in catalog_pages(root):
        text = (root / page).read_text(encoding="utf-8")
        found, loose = mdtable.rows_matching(text, CATALOG_ROW_RE)
        for r in found:
            if r.header == EXIT_TIER_HEADER:
                continue
            code = CATALOG_ROW_RE.match(r.line.strip()).group(1)
            rows.append(CatalogRow(code, page, r.lineno, r.line, r.header))
        detached += [(page, n, line) for n, line in loose]
    return rows, detached


def row_counts(root: pathlib.Path = REPO_ROOT) -> dict[str, int]:
    """DW code -> the number of catalog rows introducing it, over every page."""
    counts: dict[str, int] = {}
    for row in catalog_rows(root)[0]:
        counts[row.code] = counts.get(row.code, 0) + 1
    return counts


def documented_codes(root: pathlib.Path = REPO_ROOT) -> set[str]:
    """The DW codes the catalog documents: those with a catalog row. The one
    rule for "documented"; a code named only in a heading or prose is not."""
    return set(row_counts(root))


def mentioned_codes(root: pathlib.Path = REPO_ROOT) -> set[str]:
    """Every DW code the catalog's pages name anywhere, row or prose."""
    found: set[str] = set()
    for page in catalog_pages(root):
        found |= set(CODE_RE.findall((root / page).read_text(encoding="utf-8")))
    return found
