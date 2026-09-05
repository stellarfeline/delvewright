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
    """The exact shape of the `DW0352` collision: one code, two rules, one crate."""
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
    constant NAME for DIFFERENT codes (`DW_INPUT` is DW0710 in delvec schem and
    DW0732 in delvec prefab). Same name, different code = fine; the collision is
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
        "| Code | Rule |\n"
        "| --- | --- |\n"
        "| `DW0351` | A continuity lint. |\n"
        "| `DW0352` | A world-edits batch writes into trap hardware. |\n"
        "| `DW0352` | A punishing begin-stealth whose grace window cannot be beaten. |\n"
        "| `DW0353` | A gate-region collision. |\n",
        encoding="utf-8",
    )
    counts, detached = gate.catalog_rows()
    assert detached == []
    assert counts["DW0352"] == 2
    assert [c for c, n in counts.items() if n > 1] == ["DW0352"]


def test_a_row_a_blank_line_detached_from_the_catalog_documents_nothing(gate):
    """The page is what the catalog IS. A blank line ends a pipe table, so a row
    below one renders as a paragraph of literal pipes — twenty-one were live in
    `compiler.md` at once, and this gate counted every one as a documented
    diagnostic. It is a finding, never a row and never a silent discard."""
    gate.DOC_PATH.write_text(
        "| Code | Rule |\n"
        "| --- | --- |\n"
        "| `DW0351` | A continuity lint. |\n"
        "\n"
        "| `DW0352` | A world-edits batch writes into trap hardware. |\n",
        encoding="utf-8",
    )
    counts, detached = gate.catalog_rows()
    assert counts == {"DW0351": 1}
    assert [lineno for lineno, _ in detached] == [5]
    assert "DW0352" in detached[0][1]


def test_a_code_merely_mentioned_in_prose_does_not_count_as_a_row(gate):
    """Only a catalog ROW introduces a rule; cross-references in pipeline tables
    and prose cite codes constantly and must not read as duplicate definitions."""
    gate.DOC_PATH.write_text(
        "| # | Stage | Module | Codes |\n"
        "| --- | --- | --- | --- |\n"
        "| 10 | Nav checks | `compiler::nav` | `DW0327`/`DW0355` (exit 3) |\n"
        "\n"
        "- **Trap-hardware integrity (`DW0352`).** No batch write may land on it.\n"
        "\n"
        "| Code | Rule |\n"
        "| --- | --- |\n"
        "| `DW0355` | A punishing begin-stealth whose grace window cannot be beaten. |\n",
        encoding="utf-8",
    )
    counts, detached = gate.catalog_rows()
    assert detached == []
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


# --- the test-coverage gate: coverage means an ASSERTION, not a mention --------
#
# The loophole this closes: `tested_codes` used to be a raw `DW[0-9]{4}` grep over
# whole test files, so a `///` doc comment *naming* a code in a test that never
# touched it read as full coverage. Two real rules were green that way (`DW0304`,
# `DW0309`), plus one whose only reference was the code named in an `.expect`
# failure message while the assertion looked elsewhere (`DW0313`).


def _test_rs(gate, crate: str, name: str, body: str) -> None:
    tests = gate.CRATES_DIR / crate / "tests"
    tests.mkdir(parents=True, exist_ok=True)
    (tests / name).write_text(body, encoding="utf-8")


def test_a_code_only_named_in_a_comment_is_not_covered(gate):
    """The exact false-green: a doc comment citing a rule the test never exercises."""
    _test_rs(
        gate,
        "compiler",
        "solver.rs",
        "/// Branching growth (lifts the old `DW0304` one-terminal limit).\n"
        "#[test]\n"
        "fn branching_two_terminals_both_placed() { assert!(true); }\n",
    )
    assert "DW0304" not in gate.tested_codes()


def test_a_code_only_named_in_a_failure_message_is_not_covered(gate):
    """`.expect(\"must raise DW0313\")` names the code in prose while the assertion
    looks at something else — the test passes whatever code is raised."""
    _test_rs(
        gate,
        "compiler",
        "emit.rs",
        '#[test]\nfn t() {\n'
        '    let msg = f().expect("unsupported sand floor must raise DW0313");\n'
        '    assert!(msg.contains("despawn"));\n'
        "}\n",
    )
    assert "DW0313" not in gate.tested_codes()


def test_a_bare_code_literal_counts_in_every_idiom_the_repo_uses(gate):
    """The matcher must accept the assertion shapes actually written here —
    including the table-driven ones, where the literal sits in an array or tuple
    that a nearby loop asserts over."""
    _test_rs(
        gate,
        "compiler",
        "shapes.rs",
        '#[test]\nfn a() { assert_eq!(d.code, "DW0351"); }\n'
        '#[test]\nfn b() { assert!(diags.iter().any(|d| d.code == "DW0740")); }\n'
        '#[test]\nfn c() { assert!(stderr.contains("DW0732"), "expected it: {stderr}"); }\n'
        '#[test]\nfn d() { for code in ["DW0101", "DW0102"] { assert!(covered(code)); } }\n'
        '#[test]\nfn e() { for (f, x) in [("a.json", "DW0302")] { assert_eq!(run(f), x); } }\n'
        '#[test]\nfn f() { assert!(codes().contains(&"DW0203".to_string())); }\n',
    )
    covered = gate.tested_codes()
    for code in ("DW0351", "DW0740", "DW0732", "DW0101", "DW0102", "DW0302", "DW0203"):
        assert code in covered, code


def test_a_symbolic_constant_counts_but_a_bare_import_does_not(gate):
    """A symbol comparison is a real assertion; an import alone is not a use."""
    _rs(gate, "schem", "diag.rs", 'pub const DW_STRIP: &str = "DW0700";')
    _test_rs(gate, "schem", "imports.rs", "use delvewright_schem::diag::DW_STRIP;\n")
    assert "DW0700" not in gate.tested_codes()
    _test_rs(
        gate,
        "schem",
        "imports.rs",
        "use delvewright_schem::diag::DW_STRIP;\n"
        "#[test]\nfn t() { assert_eq!(err.code, DW_STRIP); }\n",
    )
    assert "DW0700" in gate.tested_codes()


def test_comment_stripping_never_eats_a_rust_string(gate):
    """Test fixtures are full of `//` inside JSON and path strings; a naive
    comment stripper would swallow the rest of the line — and with it the
    assertion that follows."""
    _test_rs(
        gate,
        "compiler",
        "raws.rs",
        'const F: &str = r#"{ "url": "https://example.invalid/x" }"#;\n'
        '#[test]\nfn t() { assert_eq!(d.code, "DW0322"); }\n'
        '#[test]\nfn u() { let p = "a//b"; assert!(p.is_empty() || d.code == "DW0323"); }\n',
    )
    covered = gate.tested_codes()
    assert "DW0322" in covered
    assert "DW0323" in covered


def test_a_block_comment_hides_a_code_but_not_the_code_after_it(gate):
    _test_rs(
        gate,
        "compiler",
        "blocks.rs",
        "/* DW0304 is discussed here /* and here */ but never asserted */\n"
        '#[test]\nfn t() { assert_eq!(d.code, "DW0305"); }\n',
    )
    covered = gate.tested_codes()
    assert "DW0304" not in covered
    assert "DW0305" in covered


# ------------------------------------------------------------- exit tier --
#
# The tier decides the process exit status, and it is stated twice — once as
# `ExitTier::…` on the constant, once as a row of the reference's §1 table. Two
# statements of one fact with nothing comparing them is how the reader's copy
# goes stale, so the gate compares them; these drive its two readers.

CATALOG = "| Code | Meaning |\n| --- | --- |\n"
TIER_TABLE = "| Code | What the author changes |\n| --- | --- |\n"


def test_a_declared_tier_is_read_off_the_constant(gate):
    _rs(
        gate,
        "compiler",
        "emit.rs",
        'pub const DW_WAVE_NO_ROOM: DwCode = DwCode::every_version("DW0312", ExitTier::Analysis);\n'
        'pub const DW_BUILD: DwCode = DwCode::every_version("DW0300", ExitTier::Build);\n'
        'pub const DW_FENCED: DwCode = DwCode::since("DW0481", 8, ExitTier::Build);\n',
    )
    tiers, unreadable = gate.declared_tiers()
    assert tiers == {"DW0312": "Analysis", "DW0300": "Build", "DW0481": "Build"}
    assert unreadable == []


def test_a_constant_written_through_a_path_is_still_read(gate):
    """Two live constants spell the type or the constructor through a path, and a
    reader demanding the bare name would drop exactly those from the comparison —
    a shrinking denominator, which is this gate passing by looking at less."""
    _rs(
        gate,
        "dsl",
        "prefab.rs",
        'pub const DW_FOOTPRINT_CLASS: crate::DwCode = '
        'crate::DwCode::every_version("DW0848", crate::ExitTier::Build);\n',
    )
    tiers, unreadable = gate.declared_tiers()
    assert tiers == {"DW0848": "Build"}
    assert unreadable == []


def test_a_tierless_dwcode_constant_is_reported_not_skipped(gate):
    """A declaration the reader cannot parse is a code it cannot judge. Reporting
    it is the difference between a gate and a gate that got smaller."""
    _rs(
        gate,
        "compiler",
        "nav.rs",
        'pub const DW_ODD: DwCode = DwCode::every_version("DW0399");\n',
    )
    tiers, unreadable = gate.declared_tiers()
    assert tiers == {}
    assert len(unreadable) == 1 and "DW0399" in unreadable[0]


def test_the_tier_table_is_read_and_the_catalog_is_not(gate):
    gate.DOC_PATH.write_text(
        TIER_TABLE
        + "| `DW0210` | the area's lighting declaration |\n"
        + "| `DW0312` | the wave's size |\n"
        + "\n"
        + CATALOG
        + "| `DW0300` | generic build failure |\n"
        + "| `DW0210` | too dark to read |\n",
        encoding="utf-8",
    )
    codes, rows = gate.documented_analysis_codes()
    assert codes == {"DW0210", "DW0312"}
    assert rows == 2
    # …and the catalog reader must not count the tier table's rows, or every code
    # in it reads as a duplicated catalog entry.
    assert gate.catalog_row_counts() == {"DW0300": 1, "DW0210": 1}


def test_an_empty_tier_table_is_a_finding_not_an_empty_agreement(gate):
    """A comparison against nothing agrees with a source declaring nothing. The
    row count is returned so the caller can refuse that answer instead of
    printing it."""
    gate.DOC_PATH.write_text(CATALOG + "| `DW0300` | generic build failure |\n", encoding="utf-8")
    codes, rows = gate.documented_analysis_codes()
    assert codes == set()
    assert rows == 0
