"""The instrument-bound verdict gate (`tools/check-trial-verdicts.py`).

The defect this pins: trial 0001 answered R1 `partial` for run 1 and, three
paragraphs below the verdict, said the renderer had no camera an author could aim
and that this "alone is the whole of R1's `partial`". Later rounds cite the
verdict, not the paragraph. When an aimable camera arrived and the same delivered
bytes were re-photographed square-on, the answer was `yes` — the record had
understated its own result for as long as it existed.

The gate was landed with its red paths demonstrated by hand, once, against the
live record. A hand-run demonstration is not a guard: it cannot fail again. These
tests drive the parser over synthetic records so each red path keeps failing for
its own reason as the real records grow — and so a reformatted rubric table reds
on its own zero binding instead of quietly binding to nothing.
"""

import importlib.util
import pathlib

import pytest

SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "check-trial-verdicts.py"


@pytest.fixture
def gate(tmp_path, monkeypatch):
    """The script loaded as a module, re-rooted at an empty synthetic trial tree."""
    spec = importlib.util.spec_from_file_location("check_trial_verdicts", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    trials = tmp_path / "docs" / "trials"
    trials.mkdir(parents=True)
    monkeypatch.setattr(module, "ROOT", tmp_path)
    monkeypatch.setattr(module, "TRIALS", trials)
    return module


def write(gate, name, text):
    path = gate.TRIALS / name
    path.write_text(text, encoding="utf-8")
    return path


RUBRIC = """\
## Run 0 — result

| # | Question | Answer |
|---|---|---|
| R1 | Does it read as the thing? | **yes** |
| R2 | Is every named thing reachable? | **no** |
"""


def bounds(rows):
    head = "## Instrument bounds\n\n| Verdict | Bound | Judged from |\n|---|---|---|\n"
    return head + "".join(f"| {a} | {b} | {c} |\n" for a, b, c in rows)


def test_a_complete_record_passes_and_the_counts_split_by_kind(gate, capsys):
    write(
        gate,
        "trial-0001-x.md",
        RUBRIC
        + bounds(
            [
                ("R1 run 0", "artifact-bound", "square-on elevation"),
                ("R2 run 0", "instrument-bound — lighting cannot read a manifest", "gate report"),
            ]
        ),
    )
    assert gate.main() == 0
    out = capsys.readouterr().out
    # The label must name the quantity the count actually is. A single total
    # printed under ONE of the two kinds is this gate's own defect, one layer
    # out: the number right, the sentence not about it.
    assert "2 judged verdict(s)" in out
    assert "1 artifact-bound + 1 instrument-bound" in out


def test_a_declaration_detached_from_its_table_does_not_declare(gate, capsys):
    """The escape this gate was open to. A blank line ends a pipe table, so a
    bounds row under one renders as a paragraph of literal pipes — the verdict
    is undeclared on the page and was declared to the parser. Both halves must
    now be true at once: the row is named as unreadable, AND the verdict it was
    standing in for is reported undeclared."""
    write(
        gate,
        "trial-0001-x.md",
        RUBRIC
        + bounds([("R1 run 0", "artifact-bound", "elevation")])
        + "\n| R2 run 0 | artifact-bound | gate report |\n",
    )
    assert gate.main() == 1
    captured = capsys.readouterr()
    out = captured.out + captured.err
    assert "is a table row that no table contains" in out
    assert "R2 run 0 is judged but declares no instrument bound" in out


def test_the_same_declaration_attached_is_a_declaration(gate):
    """The other direction, so the red above is about the blank line alone."""
    write(
        gate,
        "trial-0001-x.md",
        RUBRIC
        + bounds(
            [
                ("R1 run 0", "artifact-bound", "elevation"),
                ("R2 run 0", "artifact-bound", "gate report"),
            ]
        ),
    )
    assert gate.main() == 0


def test_a_judged_verdict_with_no_declaration_reds(gate):
    write(gate, "trial-0001-x.md", RUBRIC + bounds([("R1 run 0", "artifact-bound", "elevation")]))
    assert gate.main() == 1  # R2 run 0 is judged and undeclared


def test_instrument_bound_without_a_named_blocker_reds(gate):
    write(
        gate,
        "trial-0001-x.md",
        RUBRIC
        + bounds(
            [
                ("R1 run 0", "artifact-bound", "elevation"),
                ("R2 run 0", "instrument-bound", "gate report"),
            ]
        ),
    )
    assert gate.main() == 1  # the blocker is the whole point of the declaration


def test_a_declaration_with_no_instrument_named_reds(gate):
    write(
        gate,
        "trial-0001-x.md",
        RUBRIC
        + bounds(
            [
                ("R1 run 0", "artifact-bound", ""),
                ("R2 run 0", "artifact-bound", "gate report"),
            ]
        ),
    )
    assert gate.main() == 1


def test_a_declaration_for_a_verdict_nobody_judged_reds(gate):
    write(
        gate,
        "trial-0001-x.md",
        RUBRIC
        + bounds(
            [
                ("R1 run 0", "artifact-bound", "elevation"),
                ("R2 run 0", "artifact-bound", "gate report"),
                ("R9 run 0", "artifact-bound", "nothing"),
            ]
        ),
    )
    assert gate.main() == 1


def test_a_reformatted_rubric_reds_on_its_own_zero_binding(gate, capsys):
    """The vacuity path: the parser binds to nothing and must say so, not pass.

    This is the mode that makes a gate worthless silently — the record still has
    its verdicts, the table just no longer looks like one to the parser.
    """
    write(
        gate,
        "trial-0001-x.md",
        "## Run 0 — result\n\nR1: **yes**\nR2: **no**\n\n" + bounds([]),
    )
    assert gate.main() == 1
    assert "0 judged verdict(s)" in capsys.readouterr().out


def test_a_record_with_no_run_section_reds(gate):
    write(
        gate,
        "trial-0001-x.md",
        "| # | Question | Answer |\n|---|---|---|\n| R1 | Reads? | **yes** |\n",
    )
    assert gate.main() == 1


def test_every_run_of_a_multi_run_record_owes_its_own_declaration(gate):
    """A second run is the way a record grows, so it is the way coverage decays."""
    two_runs = RUBRIC + "\n## Run 1 — result\n\n" + RUBRIC.split("\n", 2)[2]
    write(
        gate,
        "trial-0001-x.md",
        two_runs
        + bounds(
            [
                ("R1 run 0", "artifact-bound", "elevation"),
                ("R2 run 0", "artifact-bound", "gate report"),
                ("R1 run 1", "artifact-bound", "elevation"),
            ]
        ),
    )
    assert gate.main() == 1  # R2 run 1 rode in undeclared


def test_an_empty_trials_directory_passes_rather_than_asserting_coverage(gate, capsys):
    assert gate.main() == 0
    assert "0 trial record(s)" in capsys.readouterr().out
