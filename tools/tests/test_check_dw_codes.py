"""The DW-code gate's own uniqueness check (`tools/check-dw-codes.py`).

The collision this pins, from the field: two branches developed in parallel each
picked "the next free DW code" against the main they branched from, and the merge
shipped ONE number for TWO rules — `DW0352` was simultaneously the map editor's
trap-hardware integrity check and the stealth-onset survivability proof. Every
other gate in the script passed on that pair: both rules were in source, both were
documented, both were tested. Consistency and coverage are structurally blind to
it, which is why uniqueness had to become its own gate.

These tests drive the two detectors over synthetic trees rather than the live
repo, so they keep failing for the right reason as the real catalog grows.
"""

import importlib.util
import pathlib

import pytest

SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "check-dw-codes.py"


@pytest.fixture
def gate(tmp_path, monkeypatch):
    """The script loaded as a module, re-rooted at an empty synthetic repo."""
    spec = importlib.util.spec_from_file_location("check_dw_codes", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    crates = tmp_path / "crates"
    crates.mkdir()
    doc = tmp_path / "compiler.md"
    doc.write_text("", encoding="utf-8")
    monkeypatch.setattr(module, "REPO_ROOT", tmp_path)
    monkeypatch.setattr(module, "CRATES_DIR", crates)
    monkeypatch.setattr(module, "DOC_PATH", doc)
    return module


def _rs(gate, crate: str, name: str, body: str) -> None:
    src = gate.CRATES_DIR / crate / "src"
    src.mkdir(parents=True, exist_ok=True)
    (src / name).write_text(body, encoding="utf-8")


def test_one_code_declared_by_two_constants_is_detected(gate):
    """The exact shape of the #155/#157 collision: one code, two rules, one crate."""
    _rs(gate, "compiler", "edit.rs", 'pub const DW_EDIT_TRAP_HARDWARE: &str = "DW0352";')
    _rs(gate, "compiler", "nav.rs", 'pub const DW_STEALTH_ONSET: &str = "DW0352";')
    owners = gate.declared_constants()["DW0352"]
    assert {name for _, name in owners} == {"DW_EDIT_TRAP_HARDWARE", "DW_STEALTH_ONSET"}
    assert len({name for _, name in owners}) > 1, "the gate's collision predicate"


def test_a_collision_across_crates_is_detected_too(gate):
    """Splitting the rules across crates must not launder the collision."""
    _rs(gate, "compiler", "nav.rs", 'pub const DW_A: &str = "DW0355";')
    _rs(gate, "dsl", "validate.rs", 'pub const DW_B: &str = "DW0355";')
    owners = gate.declared_constants()["DW0355"]
    assert {crate for crate, _ in owners} == {"compiler", "dsl"}
    assert len({name for _, name in owners}) > 1


def test_distinct_codes_and_reused_constant_names_are_not_collisions(gate):
    """The legitimate pattern the gate must never flag: two crates reusing a
    constant NAME for DIFFERENT codes (`DW_INPUT` is DW0710 in delve-schem and
    DW0732 in delve-admit). Same name, different code = fine; the collision is
    the reverse."""
    _rs(gate, "schem", "diag.rs", 'pub const DW_INPUT: &str = "DW0710";')
    _rs(gate, "admit", "diag.rs", 'pub const DW_INPUT: &str = "DW0732";')
    table = gate.declared_constants()
    assert all(len({n for _, n in owners}) == 1 for owners in table.values())


def test_one_constant_referenced_from_many_files_is_not_a_collision(gate):
    """A single declaration cited all over the codebase is the normal case."""
    _rs(gate, "compiler", "nav.rs", 'pub const DW_STEALTH_ONSET: &str = "DW0355";')
    _rs(gate, "compiler", "emit.rs", "// see DW0355 (the onset proof)\n")
    _rs(gate, "compiler", "plan.rs", "/// Roots the DW0355 proof's start set.\n")
    owners = gate.declared_constants()["DW0355"]
    assert owners == {("compiler", "DW_STEALTH_ONSET")}


def test_two_catalog_rows_for_one_code_are_detected(gate):
    """The documentation half of the collision — two rules, two rows, one code."""
    gate.DOC_PATH.write_text(
        "| `DW0351` | A continuity lint. |\n"
        "| `DW0352` | A world-edits batch writes into trap hardware. |\n"
        "| `DW0352` | A punishing begin-stealth whose grace window cannot be beaten. |\n"
        "| `DW0353` | A gate-region collision. |\n",
        encoding="utf-8",
    )
    counts = gate.catalog_row_counts()
    assert counts["DW0352"] == 2
    assert [c for c, n in counts.items() if n > 1] == ["DW0352"]


def test_a_code_merely_mentioned_in_prose_does_not_count_as_a_row(gate):
    """Only a catalog ROW introduces a rule; cross-references in pipeline tables
    and prose cite codes constantly and must not read as duplicate definitions."""
    gate.DOC_PATH.write_text(
        "| 10 | Nav checks | `compiler::nav` | `DW0327`/`DW0355` (exit 3) |\n"
        "- **Trap-hardware integrity (`DW0352`).** No batch write may land on it.\n"
        "| `DW0355` | A punishing begin-stealth whose grace window cannot be beaten. |\n",
        encoding="utf-8",
    )
    counts = gate.catalog_row_counts()
    assert counts == {"DW0355": 1}


def test_the_live_repo_has_no_collisions(gate, monkeypatch):
    """Regression guard against the real tree — the check the merge needed."""
    spec = importlib.util.spec_from_file_location("check_dw_codes_live", SCRIPT)
    live = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(live)
    dupes = {
        code: sorted(owners)
        for code, owners in live.declared_constants().items()
        if len({name for _, name in owners}) > 1
    }
    assert dupes == {}, f"two rules share a DW code: {dupes}"
    rows = {code: n for code, n in live.catalog_row_counts().items() if n > 1}
    assert rows == {}, f"a DW code has two catalog rows: {rows}"
