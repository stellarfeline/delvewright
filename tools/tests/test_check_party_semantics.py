r"""Guards for `tools/ci/check-party-semantics.py`.

The red this gate exists to give: a surface reaches the DSL with nobody having
said what it means for two, three or four players. The tests run the gate over
a small schema export and a small record, and perturb each toward the shapes it
must refuse — a unit no row reaches, a row that reaches no unit, a row with no
evidence — plus the committed record's own well-formedness.
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
CHECKER = REPO / "tools" / "ci" / "check-party-semantics.py"
RECORD = REPO / "tools" / "ci" / "party-semantics.json"

EXPORT = {
    "quests": {
        "title": "Envelope",
        "type": "object",
        "properties": {"content": {"$ref": "#/$defs/Thing"}},
        "$defs": {
            "Thing": {
                "type": "object",
                "properties": {"count": {"type": "integer"}, "who": {"$ref": "#/$defs/Who"}},
            },
            "Who": {"oneOf": [{"type": "string", "const": "all"}, {"type": "string", "const": "one"}]},
        },
    },
    "site-plan": {
        "title": "Envelope",
        "type": "object",
        "properties": {"plan": {"$ref": "#/$defs/Box"}},
        "$defs": {"Box": {"type": "object", "properties": {"size": {"type": "integer"}}}},
    },
}


def row(cls: str) -> dict:
    return {"class": cls, "semantics": "who is credited, told and moved", "evidence": "where it is emitted"}


def record() -> dict:
    return {
        "classes": {"party": "one fact", "declared": "a field says", "no-runtime": "nothing at run time"},
        "stages": {"site-plan": "builds the world"},
        "types": {"Thing": row("party"), "Who": row("declared"), "Envelope": row("no-runtime")},
        "units": {},
    }


def run(tmp_path: Path, rec: dict, export: dict = EXPORT) -> subprocess.CompletedProcess:
    (tmp_path / "export.json").write_text(json.dumps(export))
    (tmp_path / "record.json").write_text(json.dumps(rec))
    return subprocess.run(
        [sys.executable, str(CHECKER), "--schema", str(tmp_path / "export.json"), "--record", str(tmp_path / "record.json")],
        capture_output=True,
        text=True,
    )


def test_a_record_that_reaches_every_unit_is_green_and_states_its_denominator(tmp_path):
    r = run(tmp_path, record())
    assert r.returncode == 0, r.stdout
    # Thing.count, Thing.who, Who::all, Who::one, Box.size, and the two envelopes' fields.
    assert "7 of 7 surface unit(s) classified" in r.stdout, r.stdout
    assert "1 by a stage row" in r.stdout, r.stdout


def test_a_unit_no_row_reaches_is_red(tmp_path):
    rec = record()
    del rec["types"]["Who"]
    r = run(tmp_path, rec)
    assert r.returncode == 1, r.stdout
    assert "unit `Who::all` has no party meaning" in r.stdout, r.stdout
    assert "unit `Who::one` has no party meaning" in r.stdout, r.stdout


def test_a_row_that_reaches_no_unit_is_red(tmp_path):
    rec = record()
    rec["units"]["Thing.gone"] = row("party")
    r = run(tmp_path, rec)
    assert r.returncode == 1, r.stdout
    assert "row `units[Thing.gone]` reaches no unit" in r.stdout, r.stdout


def test_a_unit_row_outranks_its_type_row(tmp_path):
    rec = record()
    rec["units"]["Thing.who"] = row("declared")
    r = run(tmp_path, rec)
    assert r.returncode == 0, r.stdout
    assert "declared: 3" in r.stdout, r.stdout


def test_a_row_with_no_evidence_or_an_undefined_class_is_red(tmp_path):
    rec = record()
    rec["types"]["Thing"]["evidence"] = ""
    rec["types"]["Who"]["class"] = "somebody"
    r = run(tmp_path, rec)
    assert r.returncode == 1, r.stdout
    assert "row `types[Thing]` states no `evidence`" in r.stdout, r.stdout
    assert "names class `somebody`" in r.stdout, r.stdout


def test_a_stage_that_emits_nothing_classifies_only_units_wholly_inside_it(tmp_path):
    export = json.loads(json.dumps(EXPORT))
    # The same type declared in a runtime stage too: the stage row no longer reaches it.
    export["quests"]["$defs"]["Box"] = export["site-plan"]["$defs"]["Box"]
    r = run(tmp_path, record(), export)
    assert r.returncode == 1, r.stdout
    assert "unit `Box.size` has no party meaning" in r.stdout, r.stdout


def test_the_committed_record_is_well_formed():
    rec = json.loads(RECORD.read_text())
    assert set(rec) == {"classes", "stages", "types", "units"}
    for kind in ("types", "units"):
        for key, r in rec[kind].items():
            assert r["class"] in rec["classes"], (kind, key)
            assert r["semantics"] and r["evidence"], (kind, key)
