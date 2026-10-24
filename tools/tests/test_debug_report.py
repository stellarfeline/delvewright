"""`scripts/debug-note.py` and `scripts/debug-report.py`: debug mode records, and off records nothing (spec-0122 §4).

Every test runs the two scripts as the page runs them — a subprocess with the
flag set or unset in its environment — so what is asserted is the behaviour a
run sees, never a function called past the flag.
"""

from __future__ import annotations

import os
import pathlib
import subprocess
import sys

import pytest

REPO = pathlib.Path(__file__).resolve().parents[2]
SCRIPTS = REPO / ".claude" / "skills" / "delvewright" / "skills" / "new-delve" / "scripts"
NOTE = SCRIPTS / "debug-note.py"
REPORT = SCRIPTS / "debug-report.py"
HEADING = "## Technical decisions and engine limits"

# What `delvec` prints on a refused detail, in its own human spelling: two error
# lines of one code, one of another, and a warning that refuses nothing.
DELVEC_OUTPUT = (
    "DW0844 [error] detail-plan node/hall: the piece answers no seam on its west face\n"
    "DW0844 [error] detail-plan node/hall: the piece answers no seam on its north face\n"
    "DW0845 [error] detail-plan node/hall: the owed name `anchor/node-hall` is unbound\n"
    "DW0210 [warning] build: 3 dark cell(s)\n"
    "error: `node/hall`: the piece(s) `prefab/x-hall` would not bind; nothing was written.\n"
)


def env(debug: str | None) -> dict[str, str]:
    e = {k: v for k, v in os.environ.items() if k != "DELVEWRIGHT_DEBUG"}
    if debug is not None:
        e["DELVEWRIGHT_DEBUG"] = debug
    return e


def note(campaign: pathlib.Path, debug: str | None, *args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, "-I", str(NOTE), str(campaign), *args],
        capture_output=True, text=True, env=env(debug), check=False,
    )


def report(campaign: pathlib.Path, debug: str | None, *args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, "-I", str(REPORT), str(campaign), *args],
        capture_output=True, text=True, env=env(debug), check=False,
    )


def log(campaign: pathlib.Path) -> pathlib.Path:
    return campaign / ".out" / "technical.jsonl"


FIVE = [
    ("default", "init", "the client jar was downloaded; the prompt named no directory"),
    ("limit", "9/node/hall", "an open shop front is not authorable under one box per building"),
    ("workaround", "2", "the terrace contact was moved to the flight foot"),
    ("departure", "9/node/forge", "the forge burns campfires where the design drew lava"),
]


@pytest.mark.parametrize("debug", [None, "0", ""])
def test_off_a_note_writes_nothing_and_a_report_renders_nothing(tmp_path: pathlib.Path, debug):
    out = note(tmp_path, debug, "--kind", "default", "--step", "init", "--what", "x")
    assert out.returncode == 0, out.stderr
    assert not log(tmp_path).exists(), "a note written while the flag is off"
    assert not (tmp_path / ".out").exists()
    r = report(tmp_path, debug, "--append", str(tmp_path / "GENERATION.md"))
    assert r.returncode == 0
    assert r.stdout == "", "a report rendered while the flag is off"
    assert not (tmp_path / "GENERATION.md").exists()


def test_off_rows_left_by_an_earlier_debug_run_are_still_not_rendered(tmp_path: pathlib.Path):
    assert note(tmp_path, "1", "--kind", "limit", "--step", "3", "--what", "x").returncode == 0
    assert log(tmp_path).is_file()
    r = report(tmp_path, None)
    assert r.returncode == 0 and HEADING not in r.stdout and r.stdout == ""


def test_on_five_notes_of_five_kinds_render_one_section_with_five_subsections(tmp_path: pathlib.Path):
    delvec_out = tmp_path / "last.txt"
    delvec_out.write_text("DW0843 [error] detail-plan node/hall: the piece is 14x9x18; the frame is 14x10x18\n", encoding="utf-8")
    for kind, step, what in FIVE:
        assert note(tmp_path, "1", "--kind", kind, "--step", step, "--what", what).returncode == 0
    out = note(tmp_path, "1", "--kind", "refusal", "--step", "9/node/hall", "--what", "the program's height reads handed/datum-y", "--from", str(delvec_out))
    assert out.returncode == 0, out.stderr
    rows = log(tmp_path).read_text(encoding="utf-8").strip().split("\n")
    assert len(rows) == 5
    gen = tmp_path / "GENERATION.md"
    gen.write_text("# Generation\n\nThe toolchain.\n", encoding="utf-8")
    r = report(tmp_path, "1", "--append", str(gen))
    assert r.returncode == 0, r.stderr
    text = r.stdout
    assert text.count(HEADING) == 1
    assert text.count("\n### ") == 5, "one subsection per kind"
    for title in ("Defaults taken (1)", "Refusals met (1)", "Engine limits met (1)", "Workarounds (1)", "Departures the player will see (1)"):
        assert f"### {title}" in text
    for _, _, what in FIVE:
        assert what in text
    assert "| code | count | where | what was said | repair |" in text
    assert "| `DW0843` | 1 | 9/node/hall | detail-plan node/hall: the piece is 14x9x18; the frame is 14x10x18 | the program's height reads handed/datum-y |" in text
    closing = text.strip().split("\n")[-1]
    assert closing == f"Events: default 1 · refusal 1 · limit 1 · workaround 1 · departure 1 — total {len(rows)}."
    # The same section, appended to GENERATION.md under the same heading.
    g = gen.read_text(encoding="utf-8")
    assert g.startswith("# Generation\n\nThe toolchain.\n\n")
    assert g.endswith(text)


def test_on_a_refusal_carries_the_code_and_count_of_the_delvec_output_it_was_handed(tmp_path: pathlib.Path):
    f = tmp_path / "detail.txt"
    f.write_text(DELVEC_OUTPUT, encoding="utf-8")
    out = note(tmp_path, "1", "--kind", "refusal", "--step", "9/node/hall", "--what", "answered the west seam", "--from", str(f))
    assert out.returncode == 0, out.stderr
    import json

    rows = [json.loads(x) for x in log(tmp_path).read_text(encoding="utf-8").strip().split("\n")]
    assert [(r["code"], r["count"]) for r in rows] == [("DW0844", 2), ("DW0845", 1)], "codes and counts read from the file; the warning refuses nothing"
    assert rows[0]["said"] == "detail-plan node/hall: the piece answers no seam on its west face"
    r = report(tmp_path, "1")
    assert "| `DW0844` | 2 |" in r.stdout and "| `DW0845` | 1 |" in r.stdout
    assert r.stdout.strip().split("\n")[-1].endswith("— total 2.")


def test_on_a_refusal_read_from_json_diagnostics(tmp_path: pathlib.Path):
    f = tmp_path / "detail.jsonl"
    f.write_text('{"code":"DW0882","severity":"error","stage":"detail","path":"node/hall","message":"not handed"}\n', encoding="utf-8")
    assert note(tmp_path, "1", "--kind", "refusal", "--step", "9/node/hall", "--what", "renamed", "--from", str(f)).returncode == 0
    assert '"code": "DW0882"' in log(tmp_path).read_text(encoding="utf-8")


def test_on_a_refusal_with_no_code_is_refused_and_writes_nothing(tmp_path: pathlib.Path):
    f = tmp_path / "x.txt"
    f.write_text("DW0210 [warning] build: dark\nok\n", encoding="utf-8")
    out = note(tmp_path, "1", "--kind", "refusal", "--step", "8", "--what", "x", "--from", str(f))
    assert out.returncode == 2 and "is a `limit`" in out.stderr
    assert note(tmp_path, "1", "--kind", "refusal", "--step", "8", "--what", "x").returncode == 2
    assert not log(tmp_path).exists()


def test_on_with_no_rows_the_section_states_its_zero(tmp_path: pathlib.Path):
    r = report(tmp_path, "1")
    assert r.returncode == 0
    assert HEADING in r.stdout
    assert r.stdout.strip().endswith("— total 0.")


def test_a_section_absent_while_rows_exist_fails(tmp_path: pathlib.Path):
    """The renderer's other defect: rows on disk, flag on, and no section."""
    assert note(tmp_path, "yes", "--kind", "workaround", "--step", "5", "--what", "w").returncode == 0
    r = report(tmp_path, "yes")
    assert r.returncode == 0
    assert HEADING in r.stdout, "rows exist and the flag is on, so the section must be rendered"
    assert "- step 5: w" in r.stdout


def test_an_unreadable_row_is_refused_never_skipped(tmp_path: pathlib.Path):
    log(tmp_path).parent.mkdir(parents=True)
    log(tmp_path).write_text('{"kind": "default", "step": "1", "what": "a"}\nnot json\n', encoding="utf-8")
    r = report(tmp_path, "1")
    assert r.returncode == 2 and "technical.jsonl:2" in r.stderr
    assert r.stdout == ""
