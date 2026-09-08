"""The documentation merge-artifact gate (`tools/check-doc-dupes.py`).

The defect this pins, from the field: the stage-5 DSL
table of `docs/reference/compiler.md` carried `shortcuts[]` twice and
`waves[].respawns_on_rest` twice, added by branches merged in a catch-up burst
that each appended their rows at the same anchor. Git merged both hunks cleanly,
every existing check stayed green, and one of the two `respawns_on_rest` copies
named the WRONG diagnostic — so the reference documented one field twice, in two
contradicting ways, with nothing able to see it.

A second defect pins rule 4: `docs/reference/tools.md`'s `validate` row carried
a long passage twice, back to back, inside one cell of one physical line. Rule
1 never saw it — it only compares the FIRST cell across DIFFERENT rows, and this
duplicate touched neither the key column nor a line boundary. Before rule 4
existed, `test_duplicate_passage_within_one_row_fails` below was RED against
this file's own gate (asserting exit 1 where the gate actually returned 0) —
that red was the gate's blindness made visible, exactly as the finding
requires; rule 4 turns it green without touching rules 1-3.

These tests drive the detectors over synthetic markdown rather than the live doc
tree, so they keep failing for the right reason as the real docs grow.
"""

import importlib.util
import pathlib

import pytest

SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "check-doc-dupes.py"


@pytest.fixture
def gate(tmp_path, monkeypatch):
    """The script loaded as a module, re-rooted at an empty synthetic doc tree."""
    spec = importlib.util.spec_from_file_location("check_doc_dupes", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    monkeypatch.setattr(module, "REPO_ROOT", tmp_path)
    monkeypatch.setattr(module, "ALLOWLIST", {})
    (tmp_path / "docs").mkdir()
    return module


def write(gate, name: str, text: str) -> pathlib.Path:
    path = gate.REPO_ROOT / "docs" / name
    path.write_text(text, encoding="utf-8")
    return path


def run(gate) -> int:
    return gate.main(["check-doc-dupes.py"])


# --------------------------------------------------------------- rule 1: rows


def test_duplicate_table_key_fails(gate, capsys):
    write(
        gate,
        "ref.md",
        "| Element | Behavior |\n"
        "|---------|----------|\n"
        "| `alpha` | first |\n"
        "| `beta` | second |\n"
        "| `alpha` | a duplicated row |\n",
    )
    assert run(gate) == 1
    err = capsys.readouterr().err
    assert "docs/ref.md:5" in err
    assert "`alpha`" in err


def test_duplicate_key_detected_through_differing_inline_markup(gate, capsys):
    """A copy that picked up different emphasis on the way through a merge is
    still the same key — the key column is compared after markup is stripped."""
    write(
        gate,
        "ref.md",
        "| Element | Behavior |\n"
        "|---------|----------|\n"
        "| `shortcuts[]` | the souls loop-back |\n"
        "| **shortcuts[]** | the souls loop-back |\n",
    )
    assert run(gate) == 1
    assert "docs/ref.md:4" in capsys.readouterr().err


def test_same_key_in_two_different_tables_is_fine(gate):
    """`set-flag` legitimately appears in the DSL-surface table AND the emission
    table AND the diagnostics catalog. Uniqueness is per table, never per file."""
    write(
        gate,
        "ref.md",
        "| Element | Behavior |\n"
        "|---------|----------|\n"
        "| `set-flag` | DSL surface |\n"
        "\n"
        "## Emission\n"
        "\n"
        "| Verb | Commands |\n"
        "|------|----------|\n"
        "| `set-flag` | scoreboard players set |\n",
    )
    assert run(gate) == 0


def test_continuation_and_filler_first_cells_are_not_keys(gate):
    """Emission tables use an empty or em-dash first cell for rows that continue
    the row above; those are not keys and must not collide with each other."""
    write(
        gate,
        "ref.md",
        "| Verb | Commands |\n"
        "|------|----------|\n"
        "| `bootstrap` | first line |\n"
        "| | continued |\n"
        "| — | also continued |\n"
        "| | continued again |\n"
        "| — | and again |\n",
    )
    assert run(gate) == 0


def test_pipe_table_inside_a_code_fence_is_ignored(gate):
    write(
        gate,
        "ref.md",
        "```\n"
        "| Element | Behavior |\n"
        "|---------|----------|\n"
        "| `alpha` | one |\n"
        "| `alpha` | two |\n"
        "```\n",
    )
    assert run(gate) == 0


def test_escaped_pipe_inside_a_cell_does_not_split_the_key(gate, capsys):
    """`campaign-start \\| quest-complete` is ONE key cell, not two — a naive
    split would compare the wrong text and both miss and invent duplicates."""
    write(
        gate,
        "ref.md",
        "| Element | Behavior |\n"
        "|---------|----------|\n"
        "| `a` \\| `b` | one |\n"
        "| `a` \\| `b` | two |\n"
        "| `a` | different key |\n",
    )
    assert run(gate) == 1
    err = capsys.readouterr().err
    assert "docs/ref.md:4" in err
    assert "docs/ref.md:5" not in err


def test_run_of_pipe_lines_without_a_separator_is_not_a_table(gate):
    write(gate, "ref.md", "| `alpha` | prose |\n| `alpha` | more prose |\n")
    assert run(gate) == 0


# ------------------------------------------------------------ rule 2: headings


def test_duplicate_heading_fails(gate, capsys):
    write(gate, "ref.md", "## Determinism\n\ntext\n\n## Determinism\n\nmore\n")
    assert run(gate) == 1
    assert "docs/ref.md:5" in capsys.readouterr().err


def test_same_text_at_different_levels_is_allowed(gate):
    """A `### Overview` under one `## Section` and a `## Overview` elsewhere are
    different headings; only an exact repeat is a doubled section."""
    write(gate, "ref.md", "## Overview\n\ntext\n\n### Overview\n\nmore\n")
    assert run(gate) == 0


def test_hash_comment_inside_a_code_fence_is_not_a_heading(gate):
    write(gate, "ref.md", "```bash\n# build\n```\n\n```bash\n# build\n```\n")
    assert run(gate) == 0


# ----------------------------------------------------- rule 3: conflict markers


def test_conflict_markers_fail(gate, capsys):
    write(gate, "ref.md", "intro\n<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> other\n")
    assert run(gate) == 1
    err = capsys.readouterr().err
    assert "docs/ref.md:2" in err
    assert "docs/ref.md:4" in err
    assert "docs/ref.md:6" in err


def test_setext_underline_is_not_a_conflict_marker(gate):
    """`=======` under a line is a setext H1 in markdown; only a `=======` inside
    an OPEN conflict counts, so a legitimate underline never trips the gate."""
    write(gate, "ref.md", "Title\n=======\n\nbody\n")
    assert run(gate) == 0


def test_conflict_marker_inside_a_code_fence_still_fails(gate):
    """Fences hide tables and headings, never a conflict marker — a marker in a
    fenced block is an artifact just the same."""
    write(gate, "ref.md", "```\n<<<<<<< HEAD\n```\n")
    assert run(gate) == 1


# -------------------------------------------------------- rule 4: within-row


def test_duplicate_passage_within_one_row_fails(gate, capsys):
    """The `tools.md` shape: one row, one physical line, the same multi-sentence
    passage twice back to back inside one cell. This is the case rule 1 cannot
    see (same key, no line boundary crossed) — before rule 4 existed this
    assertion was the red half of the round's red→green."""
    passage = (
        "The run writes the report to disk. It records every assist window "
        "with its own id. Fencing is telemetry, never the gate."
    )
    write(
        gate,
        "ref.md",
        "| Verb | Behavior |\n"
        "|------|----------|\n"
        f"| `validate` | {passage} {passage} |\n",
    )
    assert run(gate) == 1
    err = capsys.readouterr().err
    assert "docs/ref.md:3" in err
    assert "`validate`" in err
    assert "The run writes the report to disk" in err


def test_duplicate_short_cell_values_across_one_row_is_fine(gate):
    """A comparison-matrix row legitimately repeats a bare value (`yes`, a
    number) across several columns — that is an idiom, not a merge artifact,
    and rule 4 must not fire on it. This is the false-positive class measured
    on the live tree before `is_terminated_sentence` was added: 103 hits, every
    one a bare cell value, none a sentence."""
    write(
        gate,
        "ref.md",
        "| Hazard | DS1 | DS3 | Elden Ring |\n"
        "|--------|-----|-----|------------|\n"
        "| fall | yes | yes | yes |\n",
    )
    assert run(gate) == 0


def test_duplicate_passage_split_across_two_cells_of_one_row_fails(gate, capsys):
    """The same complete sentence, once in one cell and again in a later cell
    of the SAME row, is still a within-row duplicate — rule 4 scans every cell
    of a row, not only the one the field defect happened to land in."""
    write(
        gate,
        "ref.md",
        "| Verb | Note A | Note B |\n"
        "|------|--------|--------|\n"
        "| `x` | Fencing is telemetry, never the gate. | Fencing is telemetry, never the gate. |\n",
    )
    assert run(gate) == 1
    assert "docs/ref.md:3" in capsys.readouterr().err


def test_an_unterminated_fragment_is_never_a_candidate(gate):
    """A cell with no sentence-ending punctuation at all (the common case for a
    short table value) contributes no rule-4 candidate, however many times its
    bare text recurs across the row."""
    write(
        gate,
        "ref.md",
        "| K | A | B | C |\n"
        "|---|---|---|---|\n"
        "| `k` | pending | pending | pending |\n",
    )
    assert run(gate) == 0


def test_passage_duplicated_across_different_rows_is_not_a_rule_4_finding(gate):
    """Rule 4 is scoped to ONE row — a sentence legitimately repeated across
    DIFFERENT rows (a shared caveat, a shared citation) is a different question
    rule 4 does not answer."""
    write(
        gate,
        "ref.md",
        "| Verb | Behavior |\n"
        "|------|----------|\n"
        "| `a` | Fencing is telemetry, never the gate. |\n"
        "| `b` | Fencing is telemetry, never the gate. |\n",
    )
    assert run(gate) == 0


def test_rule_4_binding_is_printed(gate, capsys):
    write(
        gate,
        "ref.md",
        "| K | V |\n|---|---|\n| `a` | one. |\n",
    )
    assert run(gate) == 0
    out = capsys.readouterr().out
    assert "rule 4 binding" in out
    assert "1 table row(s) examined" in out


# ------------------------------------------------------------------ plumbing


def test_clean_tree_passes(gate, capsys):
    write(gate, "ref.md", "# Title\n\n| K | V |\n|---|---|\n| `a` | one |\n")
    assert run(gate) == 0
    assert "OK" in capsys.readouterr().out


def test_explicit_missing_path_is_a_usage_error(gate):
    """A typo'd path must be loud, or the check silently scans nothing."""
    assert gate.main(["check-doc-dupes.py", "nope"]) == 2


def test_absent_default_target_is_simply_not_scanned(gate):
    """`README.md` is a default target; a tree without one has nothing to scan
    there, which is not an error (unlike an explicitly named missing path)."""
    write(gate, "ref.md", "# Title\n")
    assert not (gate.REPO_ROOT / "README.md").exists()
    assert run(gate) == 0


def test_allowlist_suppresses_exactly_its_own_key(gate, monkeypatch, capsys):
    write(
        gate,
        "ref.md",
        "| K | V |\n|---|---|\n| `a` | one |\n| `a` | two |\n| `b` | x |\n| `b` | y |\n",
    )
    monkeypatch.setattr(gate, "ALLOWLIST", {("docs/ref.md", "a"): "justified in-script"})
    assert run(gate) == 1
    err = capsys.readouterr().err
    assert "`b`" in err
    assert "`a`" not in err


def test_stale_allowlist_entry_is_reported(gate, monkeypatch, capsys):
    """An allowlist entry that no longer suppresses anything must not rot into a
    standing licence to duplicate."""
    write(gate, "ref.md", "# Title\n")
    monkeypatch.setattr(gate, "ALLOWLIST", {("docs/gone.md", "a"): "file gone"})
    assert run(gate) == 1
    assert "docs/gone.md" in capsys.readouterr().err


def test_gitignored_private_notes_are_skipped(gate):
    """`docs/notes/private/` exists on a workstation and never in CI; scanning it
    would make the local run and the CI run disagree."""
    private = gate.REPO_ROOT / "docs" / "notes" / "private"
    private.mkdir(parents=True)
    (private / "n.md").write_text("## X\n\n## X\n", encoding="utf-8")
    write(gate, "ref.md", "# Title\n")
    assert run(gate) == 0


def test_the_live_docs_tree_is_clean(gate):
    """The gate must be green on the real repo — it ships wired into CI."""
    repo_root = SCRIPT.resolve().parents[1]
    module_spec = importlib.util.spec_from_file_location("check_doc_dupes_live", SCRIPT)
    live = importlib.util.module_from_spec(module_spec)
    assert module_spec.loader is not None
    module_spec.loader.exec_module(live)
    assert live.REPO_ROOT == repo_root
    assert live.main(["check-doc-dupes.py"]) == 0
