#!/usr/bin/env python3
"""Merge-artifact gate for the Rust source tree — the twin of `check-doc-dupes`.

`check-doc-dupes` rule 2 refuses a repeated heading, because "same level, same
text, twice in one file" is a doubled section. This is that rule where the
sections are code.

The failure it exists to kill, from the field: `crates/compiler/src/plan.rs`
carried the block

    // ---- the pieces fit together (DW0780/DW0781, ADR-0020) ----
    ...
    let binding = crate::faces::check(&areas, prefabs).map_err(...)?;
    if let Some(finding) = binding.finding(...) { warnings.push(finding); }

**twice, verbatim**, header comment and all. Every gate stayed green and nothing
could have gone red, because the pass is idempotent: running it a second time
recomputes the same answer, the second `let binding` shadows the first, and the
only trace anywhere was a zero-binding advisory printed to the operator twice.
A duplicated *pass* is invisible precisely because it is correct.

**Why the header comment and not the code.** Two candidate rules were measured
over the 397 `.rs` files of this tree:

* a repeated `// ---- ... ----` section header inside one file — **0** hits once
  the defect above was removed, and it fires on that defect;
* any repeated 3-line run of code inside one file — **3930** hits across 283
  files, every one of them correct: builder chains, match arms, per-axis
  repetition and test fixtures repeat short line runs constantly.

So the code-shape rule is declined, with the measurement, rather than shipped as
a check that would be routed around by an allowlist the size of the tree. The
header rule is kept because a labelled section is a thing an author *named*, and
naming the same section twice in one file is never intentional.

It is deliberately not a similarity detector. A block copied and then edited is a
different question — this gate answers "was a named section duplicated", which is
the merge/copy-paste accident, and it answers it with no false positives.
"""

import pathlib
import re
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parents[1]

#: Directories the sweep never descends into.
SKIP_DIRS = {"target", ".git", "node_modules", ".venv"}

#: Roots scanned when no path is given on the command line. Every Rust source in
#: the repository — the workspace crates and the standalone prefab generators,
#: which are separate cargo projects and are exactly where a copy-paste of a
#: labelled block is most likely to land unnoticed.
DEFAULT_TARGETS = ("crates", "prefabs", "harness")

#: A section-header comment: a line comment whose body is fenced by runs of
#: dashes on both sides. `// ---- name ----`, the convention used throughout the
#: compiler to label a phase of a long function.
HEADER = re.compile(r"^\s*//+ *-{2,} *(?P<title>.*?) *-{2,} *$")

#: A top-level `#[cfg(test)]` attribute, and the `mod` it guards. Under
#: `cargo fmt` — which this workspace is required to be clean under — a top-level
#: item sits at column 0 and its closing brace is a lone `}` at column 0, so the
#: test module's extent is exact rather than inferred.
CFG_TEST = re.compile(r"^#\[cfg\(test\)\]\s*$")
TEST_MOD = re.compile(r"^(?:pub(?:\([^)]*\))? )?mod \w+ \{\s*$")

#: Entries are `(path, header title)` and suppress ONE repeated title in ONE
#: file. Empty, and a stale entry is an error rather than a licence: the rule
#: measured exactly one exception across the whole tree, and that exception was
#: the rule being stated too coarsely rather than a site needing forgiveness — so
#: it was fixed in the rule (see `namespace_of`), not bought off here.
ALLOWLIST: set[tuple[str, str]] = set()


def namespace_of(lines: list[str]) -> list[str]:
    """Per line: which section namespace it belongs to, `"impl"` or `"test"`.

    A `#[cfg(test)] mod tests` conventionally MIRRORS the implementation's
    section names, so that a reader can find the tests for a phase under the
    phase's own heading. `nav.rs` names `DW0355: stealth onset survivability`
    twice for exactly that reason, and it is correct both times.

    Partitioning by module is safe against the defect this gate exists for, and
    the reason matters: a duplicated pass lands **next to its twin, inside one
    function**. Nothing about copying a block can move one copy into a test
    module, so this narrowing cannot be supplied by the failure it excludes —
    and a duplicate *within* the test module is still a finding.
    """
    out = ["impl"] * len(lines)
    i = 0
    while i < len(lines):
        if CFG_TEST.match(lines[i]):
            j = i + 1
            while j < len(lines) and lines[j].startswith("#["):
                j += 1
            if j < len(lines) and TEST_MOD.match(lines[j]):
                end = j + 1
                while end < len(lines) and lines[end].rstrip() != "}":
                    end += 1
                for k in range(j, min(end + 1, len(lines))):
                    out[k] = "test"
                i = end + 1
                continue
        i += 1
    return out


def display_path(path: pathlib.Path) -> str:
    """The repo-relative path, for stable messages and allowlist keys."""
    try:
        return str(path.relative_to(REPO_ROOT))
    except ValueError:
        return str(path)


def rust_files(targets: list[pathlib.Path]) -> list[pathlib.Path]:
    """Every `.rs` file under `targets`, sorted, skipping build output."""
    out: list[pathlib.Path] = []
    for target in targets:
        if target.is_file():
            if target.suffix == ".rs":
                out.append(target)
            continue
        for path in target.rglob("*.rs"):
            if any(part in SKIP_DIRS for part in path.parts):
                continue
            out.append(path)
    return sorted(set(out))


def duplicate_headers(rel: str, lines: list[str]) -> list[str]:
    """Section-header comments repeated within one namespace of one file."""
    ns = namespace_of(lines)
    seen: dict[tuple[str, str], list[int]] = {}
    for n, line in enumerate(lines, start=1):
        m = HEADER.match(line)
        if not m:
            continue
        title = m.group("title").strip()
        if not title:
            # A bare `// --------` is a rule, not a name; it separates and is
            # meant to repeat.
            continue
        seen.setdefault((ns[n - 1], title), []).append(n)
    errors = []
    for (where_ns, title), at in sorted(seen.items()):
        if len(at) < 2 or (rel, title) in ALLOWLIST:
            continue
        where = ", ".join(f"line {n}" for n in at)
        scope = "the test module" if where_ns == "test" else "one file"
        errors.append(
            f"{rel}: section header {title!r} appears {len(at)} times in {scope} "
            f"({where}) — a named section duplicated in one scope is a copy-paste "
            "or merge artifact. If the pass really does run twice, the two must "
            "be named differently and the reason stated"
        )
    return errors


def check_file(path: pathlib.Path) -> list[str]:
    lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    return duplicate_headers(display_path(path), lines)


def main(argv: list[str]) -> int:
    explicit = argv[1:]
    raw = explicit or list(DEFAULT_TARGETS)
    targets = [
        (REPO_ROOT / t) if not pathlib.Path(t).is_absolute() else pathlib.Path(t)
        for t in raw
    ]
    # An explicitly named path that does not exist is a typo and must be loud; a
    # DEFAULT target that does not exist is simply nothing to scan.
    missing = [t for t in targets if not t.exists()]
    if missing and explicit:
        for t in missing:
            print(f"error: no such path: {t}", file=sys.stderr)
        return 2

    files = rust_files([t for t in targets if t.exists()])
    if not files:
        print("error: no Rust files under the given paths", file=sys.stderr)
        return 2

    errors: list[str] = []
    for path in files:
        errors.extend(check_file(path))

    scanned = {display_path(p) for p in files}
    stale = sorted({p for p, _ in ALLOWLIST} - scanned)
    for p in stale:
        errors.append(
            f"ALLOWLIST entry for {p!r} names a file outside the scanned set — remove it"
        )

    if errors:
        print("source duplicate/merge-artifact check FAILED:", file=sys.stderr)
        for e in errors:
            print(f"  - {e}", file=sys.stderr)
        return 1

    # The binding count, not just a verdict: a sweep that found no files to
    # examine is unbound, and saying "OK" for it would be the vacuity mode.
    headers = sum(
        1
        for path in files
        for line in path.read_text(encoding="utf-8", errors="replace").splitlines()
        if HEADER.match(line)
    )
    print(
        f"source duplicate check OK: {len(files)} Rust files, "
        f"{headers} section headers, none repeated within a file."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
