"""The source merge-artifact gate (`tools/check-source-dupes.py`).

The defect this pins, from the field: `crates/compiler/src/plan.rs` invoked the
face-mating pass twice, as two verbatim copies of one labelled block — header
comment, `let binding = crate::faces::check(...)`, and the advisory push. The
pass is idempotent, so the second run recomputed the same answer and the second
binding shadowed the first; the only trace in the world was a zero-binding
`DW0781` advisory printed to the operator twice, and no gate looks at that.

These tests drive the detector over synthetic sources rather than the live tree,
so they keep failing for the right reason as the compiler grows — plus one test
that reproduces the shape of the real defect exactly.
"""

import importlib.util
import pathlib

import pytest

SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "check-source-dupes.py"


@pytest.fixture
def gate(tmp_path, monkeypatch):
    """The script loaded as a module, re-rooted at an empty synthetic tree."""
    spec = importlib.util.spec_from_file_location("check_source_dupes", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    monkeypatch.setattr(module, "REPO_ROOT", tmp_path)
    monkeypatch.setattr(module, "ALLOWLIST", set())
    (tmp_path / "crates").mkdir()
    return module


def write(gate, name: str, text: str) -> pathlib.Path:
    path = gate.REPO_ROOT / "crates" / name
    path.write_text(text, encoding="utf-8")
    return path


def run(gate, *args: str) -> int:
    return gate.main(["check-source-dupes.py", *args])


# --------------------------------------------------------------------------
# The binding case
# --------------------------------------------------------------------------


def test_the_field_defect_is_refused(gate, capsys):
    """The real shape: one labelled pass, invoked twice, verbatim."""
    write(
        gate,
        "plan.rs",
        """
fn build() {
    // ---- the pieces fit together (DW0780/DW0781, ADR-0020) ----
    let binding = crate::faces::check(&areas, prefabs)?;
    warnings.extend(binding.finding());

    // ---- the pieces fit together (DW0780/DW0781, ADR-0020) ----
    let binding = crate::faces::check(&areas, prefabs)?;
    warnings.extend(binding.finding());
}
""".lstrip(),
    )
    assert run(gate) == 1
    err = capsys.readouterr().err
    assert "the pieces fit together" in err
    assert "line 2" in err and "line 6" in err


def test_a_file_naming_each_section_once_is_clean(gate, capsys):
    """The control that keeps the case above from being vacuously red."""
    write(
        gate,
        "plan.rs",
        """
fn build() {
    // ---- the pieces fit together ----
    a();
    // ---- classes ----
    b();
}
""".lstrip(),
    )
    assert run(gate) == 0
    assert "2 section headers" in capsys.readouterr().out


# --------------------------------------------------------------------------
# What must NOT red — the measured shapes the code-similarity rule would have
# caught and this one deliberately does not.
# --------------------------------------------------------------------------


def test_the_same_header_in_two_different_files_is_fine(gate):
    """The rule is per file. Two modules may each have a `classes` phase."""
    write(gate, "a.rs", "// ---- classes ----\nfn a() {}\n")
    write(gate, "b.rs", "// ---- classes ----\nfn b() {}\n")
    assert run(gate) == 0


def test_a_repeated_code_run_is_not_a_finding(gate):
    """3930 such runs exist across 283 real files; every one is correct."""
    write(
        gate,
        "axes.rs",
        """
fn f() {
    push(x, y, z, 1);
    push(x, y, z, 2);
    push(x, y, z, 3);
    push(x, y, z, 1);
    push(x, y, z, 2);
    push(x, y, z, 3);
}
""".lstrip(),
    )
    assert run(gate) == 0


def test_a_test_module_may_mirror_the_implementations_section_names(gate):
    """`nav.rs` names `DW0355: stealth onset survivability` twice, correctly: once
    over the implementation and once over its tests, so a reader finds the tests
    for a phase under the phase's own heading. This is the one exception the
    measurement found, and it is the rule being stated too coarsely — not a site
    needing forgiveness."""
    write(
        gate,
        "nav.rs",
        """
// --- DW0355: stealth onset survivability ---
fn verify_stealth() {}

#[cfg(test)]
mod tests {
    // --- DW0355: stealth onset survivability ---
    #[test]
    fn t() {}
}
""".lstrip(),
    )
    assert run(gate) == 0


def test_a_duplicate_inside_the_test_module_is_still_a_finding(gate, capsys):
    """The narrowing partitions the namespaces; it does not exempt one of them."""
    write(
        gate,
        "nav.rs",
        """
#[cfg(test)]
mod tests {
    // --- onset ---
    #[test]
    fn a() {}

    // --- onset ---
    #[test]
    fn b() {}
}
""".lstrip(),
    )
    assert run(gate) == 1
    assert "the test module" in capsys.readouterr().err


def test_a_non_test_module_does_not_open_a_new_namespace(gate):
    """Only `#[cfg(test)]` partitions. An ordinary inline module is one scope with
    the file around it, because a duplicated pass can land in either."""
    write(
        gate,
        "a.rs",
        """
// --- phase ---
fn a() {}

mod helpers {
    // --- phase ---
    fn b() {}
}
""".lstrip(),
    )
    assert run(gate) == 1


def test_a_bare_rule_comment_may_repeat(gate):
    """`// --------` separates; it names nothing and is meant to recur."""
    write(gate, "a.rs", "// --------\nfn a() {}\n// --------\nfn b() {}\n")
    assert run(gate) == 0


def test_build_output_is_not_scanned(gate):
    """A vendored or generated tree under `target/` is not this repo's source.

    A real source file sits alongside it so the sweep is BOUND: without one,
    skipping `target/` would leave nothing to scan, and the exit-2 "scanned
    nothing" verdict would be mistaken for the skip working.
    """
    write(gate, "real.rs", "// ---- phase ----\nfn a() {}\n")
    d = gate.REPO_ROOT / "crates" / "target" / "debug"
    d.mkdir(parents=True)
    (d / "gen.rs").write_text("// ---- x ----\n// ---- x ----\n", encoding="utf-8")
    assert run(gate) == 0


# --------------------------------------------------------------------------
# Operational shape
# --------------------------------------------------------------------------


def test_an_explicit_missing_path_is_an_error_not_a_pass(gate, capsys):
    """A typo'd path must be loud: a gate that silently scans nothing is vacuous."""
    assert run(gate, "crates/nope") == 2
    assert "no such path" in capsys.readouterr().err


def test_scanning_nothing_is_an_error(gate, capsys):
    """Zero files is an unbound sweep, which is a finding rather than an OK."""
    assert run(gate) == 2
    assert "no Rust files" in capsys.readouterr().err


def test_an_allowlist_entry_suppresses_exactly_one_file_and_title(gate):
    write(gate, "a.rs", "// ---- x ----\n// ---- x ----\n")
    write(gate, "b.rs", "// ---- x ----\n// ---- x ----\n")
    gate.ALLOWLIST.add(("crates/a.rs", "x"))
    assert run(gate) == 1  # b.rs is still a finding


def test_a_stale_allowlist_entry_is_reported(gate, capsys):
    """An entry that suppresses nothing must go, not rot into a licence."""
    write(gate, "a.rs", "// ---- x ----\nfn a() {}\n")
    gate.ALLOWLIST.add(("crates/gone.rs", "x"))
    assert run(gate) == 1
    assert "outside the scanned set" in capsys.readouterr().err


def test_the_live_tree_is_clean():
    """The gate binds the real repository, not only its fixtures."""
    spec = importlib.util.spec_from_file_location("check_source_dupes", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    assert module.main(["check-source-dupes.py"]) == 0
