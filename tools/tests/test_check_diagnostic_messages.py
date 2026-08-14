"""The diagnostic-message wholeness gate (`tools/check-diagnostic-messages.py`).

The defect this pins, from the field: `delve-grammar expand` refused an author's
export with

    ... so freezing it would put a  on disk whose metadata describes ...

— a dropped noun and the doubled space it left behind, sitting across a `\\`-newline
continuation in the source so that no line of the file contained the gap. Every
gate the project had was green on it, because the only thing anything compared was
the diagnostic's CODE.

Two directions are exercised here, and the second is the one that matters after
the instance is fixed forever: reintroducing the dropped word must red the gate,
and a NEW message with a hole in it must red it too.

The end-to-end tests drive the checker over a synthetic crate tree rather than the
live one, so they keep failing for the right reason as the real crates grow.
"""

import importlib.util
import pathlib

import pytest

SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "check-diagnostic-messages.py"


@pytest.fixture
def gate(tmp_path, monkeypatch):
    """The script loaded as a module, re-rooted at an empty synthetic crate tree."""
    spec = importlib.util.spec_from_file_location("check_diagnostic_messages", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    monkeypatch.setattr(module, "REPO_ROOT", tmp_path)
    monkeypatch.setattr(module, "CRATES_DIR", tmp_path / "crates")
    (tmp_path / "crates").mkdir()
    return module


def crate(gate, name: str, rust: str) -> pathlib.Path:
    src = gate.CRATES_DIR / name / "src"
    src.mkdir(parents=True, exist_ok=True)
    path = src / "lib.rs"
    path.write_text(rust, encoding="utf-8")
    return path


# ---------------------------------------------------------------- the rule


def test_whole_sentence_has_no_holes(gate):
    assert gate.holes("the region is 3x3x3 and needs one cell on every axis") == []


def test_doubled_space_between_words_is_a_hole(gate):
    kinds = [k for k, _ in gate.holes("freezing it would put a  on disk")]
    assert "doubled-space" in kinds


def test_gap_after_an_article_is_a_hole(gate):
    # The shape a dropped noun leaves in front of an interpolation, which the
    # doubled-space rule alone cannot tell from a report column.
    kinds = [k for k, _ in gate.holes("this would overwrite the  X you declared")]
    assert "gap-after-article" in kinds


def test_dangling_article_is_a_hole(gate):
    kinds = [k for k, _ in gate.holes("the anchor is not provided by the.")]
    assert "dangling-article" in kinds


def test_space_before_punctuation_is_a_hole(gate):
    kinds = [k for k, _ in gate.holes("the anchor is unresolved , so nothing is placed")]
    assert "space-before-punctuation" in kinds


def test_empty_quoted_span_is_a_hole(gate):
    assert [k for k, _ in gate.holes("no such prefab ``")] == ["empty-quoted-span"]
    assert [k for k, _ in gate.holes('no such prefab ""')] == ["empty-quoted-span"]


def test_empty_brackets_are_a_hole(gate):
    assert [k for k, _ in gate.holes("the region () is degenerate")] == ["empty-brackets"]


def test_array_field_notation_is_not_empty_brackets(gate):
    # `waves[]` / `world.areas[].lighting` is how this DSL names an array field.
    assert gate.holes("declares no `waves[]` and no `world.areas[].lighting`") == []


def test_a_character_set_in_backticks_is_not_prose(gate):
    # An author being shown which characters a font covers is not a sentence with
    # a gap in it; the delimiters stay, so an EMPTY span is still a finding.
    assert gate.holes("the font covers `! \" ' ( ) , - . / : ; ?` and nothing else") == []


def test_a_report_column_is_not_a_doubled_space(gate):
    table = "  declared effective HP  12\n  best single hit        4\n  budget      8"
    assert gate.holes(table) == []


def test_a_gap_in_one_row_of_a_report_is_still_a_hole(gate):
    table = "  declared effective HP  12\n  best single hit        4\nit outlasts a  kit"
    assert [k for k, _ in gate.holes(table)] == ["doubled-space", "gap-after-article"]


def test_alignment_is_not_earned_by_one_line(gate):
    # The opt-out test: a column is credited only when the message REPRODUCES it.
    # A single dropped word cannot produce a second row, so it cannot excuse itself.
    assert [k for k, _ in gate.holes("it would put a  on disk")] == [
        "doubled-space",
        "gap-after-article",
    ]


# ------------------------------------------------------------- rendering


def test_line_continuation_joins_without_inventing_a_space(gate):
    path = crate(
        gate,
        "a",
        'fn f() { eprintln!("one \\\n                    two"); }\n',
    )
    src = gate.Source(path, path.read_text())
    start, end, is_raw = src.literals[0]
    assert gate.literal_value(src, start, end, is_raw) == "one two"


def test_a_substitution_renders_as_a_non_empty_sample(gate):
    assert gate.render("the id {id:?} is not usable") == "the id X is not usable"
    assert gate.render("{{literal braces}}") == "{literal braces}"


# ------------------------------------------------------- end to end: red


MOTIVATING = '''
use std::fmt;

pub enum ExportError {
    Contract { gates: Vec<String> },
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExportError::Contract { gates } => write!(
                f,
                "the expanded model disagrees with the spatial contract, so freezing it \\
                 would put a %s on disk whose metadata describes a building it is not \\
                 true of: {}",
                gates.join("; ")
            ),
        }
    }
}

impl std::error::Error for ExportError {}
'''


def test_the_motivating_instance_reds_the_gate(gate, capsys):
    """Direction 1: put the dropped word back and the gate must fail."""
    crate(gate, "grammar", MOTIVATING.replace("%s", " "))
    assert gate.main() == 1
    err = capsys.readouterr().err
    assert "doubled-space" in err
    assert "Display for ExportError" in err


def test_the_repaired_instance_greens_the_gate(gate, capsys):
    crate(gate, "grammar", MOTIVATING.replace("%s", "prefab"))
    assert gate.main() == 0
    assert "rendered and whole" in capsys.readouterr().out


def test_a_new_message_with_a_hole_reds_the_gate(gate, capsys):
    """Direction 2, the one that matters once the instance is fixed forever."""
    crate(
        gate,
        "dsl",
        'fn f(d: &mut Vec<u8>) {\n'
        '    d.push(Diagnostic::error(\n'
        '        codes::ID_SYNTAX,\n'
        '        "quests",\n'
        '        format!("/content/quests/{i}/id"),\n'
        '        format!("the quest id `{}` is not a  that stage 4 declares", id),\n'
        '    ));\n'
        '}\n',
    )
    assert gate.main() == 1
    err = capsys.readouterr().err
    assert "doubled-space" in err


def test_a_hole_in_a_bare_stderr_refusal_reds_the_gate(gate):
    # `delve-orchestrator` refuses only this way; keying the gate to the
    # `Diagnostic` type would have left that binary at a binding count of zero.
    crate(gate, "orchestrator", 'fn main() { eprintln!("cannot read the  log"); }\n')
    assert gate.main() == 1


# ------------------------------------------------- end to end: accounting


def test_zero_binding_is_a_failure_not_a_pass(gate, capsys):
    crate(gate, "empty", "pub fn f() -> u8 { 1 }\n")
    assert gate.main() == 1
    assert "binds to nothing" in capsys.readouterr().err


def test_an_unrenderable_expression_is_named_not_skipped(gate, capsys):
    crate(
        gate,
        "dsl",
        'fn f(msg: String) {\n'
        '    d.push(Diagnostic::error(codes::X, "quests", path, msg));\n'
        '    eprintln!("a whole sentence with nothing wrong with it");\n'
        '}\n',
    )
    assert gate.main() == 0
    out = capsys.readouterr().out
    assert "not renderable from source" in out
    assert "Diagnostic::" in out and "msg" in out


def test_a_message_built_by_a_helper_is_reached(gate, capsys):
    crate(
        gate,
        "compiler",
        'fn clearance_error(cell: [i32; 3]) -> String {\n'
        '    format!("the body at {cell:?} is inside a  wall")\n'
        '}\n'
        'fn f() { Err(CombatError { message: clearance_error(cell) }) }\n',
    )
    # The `[i32; 3]` in the signature is the shape that used to make the body
    # unfindable, and with it the compiler's longest diagnostics unchecked.
    assert gate.main() == 1
    assert "via clearance_error()" in capsys.readouterr().err


def test_a_message_assembled_into_a_local_is_reached(gate, capsys):
    crate(
        gate,
        "dsl",
        'fn f() {\n'
        '    let msg = format!("the flag `{}` is set by no  effect", id);\n'
        '    d.push(Diagnostic::error(codes::X, "quests", path, msg));\n'
        '}\n',
    )
    assert gate.main() == 1
    assert "via let msg" in capsys.readouterr().err


def test_a_forwarded_message_is_counted_not_reported_as_unreachable(gate, capsys):
    crate(
        gate,
        "dsl",
        'fn f(e: E) {\n'
        '    d.push(Diagnostic::error(codes::X, "quests", path, e.message));\n'
        '    eprintln!("a whole sentence with nothing wrong with it");\n'
        '}\n',
    )
    assert gate.main() == 0
    out = capsys.readouterr().out
    assert "1 forwarded from a site checked where it was built" in out


def test_a_field_declaration_is_not_mistaken_for_a_message(gate, capsys):
    crate(
        gate,
        "dsl",
        "pub struct Diagnostic { pub message: String }\n"
        'fn f() { eprintln!("a whole sentence with nothing wrong with it"); }\n',
    )
    assert gate.main() == 0
    assert "not renderable" in capsys.readouterr().out


def test_a_display_impl_on_a_non_error_type_is_left_alone(gate, capsys):
    # `BlockState`'s `Display` renders a block state, not a message to a reader.
    crate(
        gate,
        "grammar",
        "impl fmt::Display for BlockState {\n"
        '    fn fmt(&self, f: &mut fmt::Formatter<\'_>) -> fmt::Result { write!(f, "a  b") }\n'
        "}\n"
        'fn g() { eprintln!("a whole sentence with nothing wrong with it"); }\n',
    )
    assert gate.main() == 0


def test_the_binding_count_is_stated_per_crate(gate, capsys):
    crate(gate, "dsl", 'fn f() { eprintln!("a whole sentence with nothing wrong"); }\n')
    crate(gate, "schem", 'fn f() { eprintln!("another whole sentence here"); }\n')
    assert gate.main() == 0
    out = capsys.readouterr().out
    assert "dsl 1" in out and "schem 1" in out
    assert "2 rendered and whole" in out
