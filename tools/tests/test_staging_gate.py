"""The staging gate (`tools/staging-gate.py`).

A gate nobody has falsified is decoration, and this one is specifically the kind
that could be: it is easy to write a coverage checker that only ever says yes.
So every verdict it can reach is driven here over synthetic campaigns and build
trees, in BOTH directions — the red is produced, then the exact condition is
undone and the same row goes green.

The direction that actually drifts on this project is the FIRST test below: a
new owner finding lands in the ledger with no general form yet. That is the
event this gate exists for, and it must red immediately and without argument.

The live ledger against the real island build is checked by the CI step itself;
these fixtures keep the detector failing for the right reasons as that ledger
grows.
"""

import importlib.util
import json
import pathlib

import pytest

SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "staging-gate.py"


@pytest.fixture(scope="module")
def gate():
    spec = importlib.util.spec_from_file_location("staging_gate", SCRIPT)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


# A DW code that certainly exists, is documented and is asserted by a test.
# DW0100 is the stage-schema conformance error — the oldest rule in the
# compiler; if it ever stops existing the whole DSL has been replaced.
LIVE_CODE = "DW0100"


def make_campaign(tmp_path, *, objectives, dsl_version="0.10.0"):
    """A minimal campaign source tree: one stage file with the given nodes."""
    d = tmp_path / "camp"
    d.mkdir(exist_ok=True)
    (d / "quests.json").write_text(
        json.dumps(
            {
                "dsl_version": dsl_version,
                "stage": 5,
                "content": {"quests": [{"id": "quest/a", "objectives": objectives}]},
            }
        )
    )
    return d


def make_build(tmp_path, *, validation=None):
    b = tmp_path / "out"
    (b / "validation").mkdir(parents=True, exist_ok=True)
    for name, doc in (validation or {}).items():
        (b / "validation" / name).write_text(json.dumps(doc))
    return b


def run(gate, tmp_path, row, *, objectives=None, dsl_version="0.10.0", validation=None):
    camp = make_campaign(
        tmp_path,
        objectives=objectives if objectives is not None else [{"type": "interact"}],
        dsl_version=dsl_version,
    )
    build = make_build(tmp_path, validation=validation)
    subj = gate.Subject(camp, build)
    return gate.adjudicate(row, gate.Engine(), subj)


BOUND_ROW = {
    "id": "t",
    "finding": "f",
    "carrier": {"kind": "dw", "code": LIVE_CODE},
    "binding": {
        "kind": "dsl",
        "files": ["quests.json"],
        "match": {"eq": {"type": "interact"}},
    },
}


def test_a_bound_row_is_the_baseline(gate, tmp_path):
    """Everything below is a departure from this, so it has to hold first."""
    r = run(gate, tmp_path, BOUND_ROW)
    assert r["verdict"] == "BOUND"
    assert r["binding"] == 1


# ---------------------------------------------------------------------------
# The direction that actually drifts
# ---------------------------------------------------------------------------


def test_a_new_finding_with_no_general_form_is_red(gate, tmp_path):
    """The event this gate exists for: a playtest produces a finding, the
    instance gets fixed, and nobody builds the class. Red, immediately."""
    row = {"id": "new", "finding": "the owner hit something new", "carrier": None}
    assert run(gate, tmp_path, row)["verdict"] == "NO-GENERAL-FORM"


def test_and_it_goes_green_when_the_general_form_is_built(gate, tmp_path):
    """The same row, once someone writes the check and a binding probe."""
    assert run(gate, tmp_path, dict(BOUND_ROW, id="new"))["verdict"] == "BOUND"


def test_a_bare_disposition_label_does_not_buy_an_exemption(gate, tmp_path):
    """rule 2's escape is a JUSTIFIED reason, not a keyword. A row that types
    the magic word and says nothing is still a red."""
    row = {
        "id": "lazy",
        "finding": "f",
        "carrier": None,
        "disposition": "no-machine-form",
        "justification": "too hard",
    }
    r = run(gate, tmp_path, row)
    assert r["verdict"] == "NO-GENERAL-FORM"
    assert "not a reason" in r["detail"]


def test_a_substantive_justification_is_the_one_permitted_exemption(gate, tmp_path):
    row = {
        "id": "prose",
        "finding": "f",
        "carrier": None,
        "disposition": "no-machine-form",
        "justification": (
            "Translation register is an aesthetic judgement; the measured "
            "detector for it ran at chance and was kept out of every gate."
        ),
    }
    assert run(gate, tmp_path, row)["verdict"] == "DECLARED-UNCOVERABLE"


# ---------------------------------------------------------------------------
# The direction that drifts slowly: a check rots
# ---------------------------------------------------------------------------


def test_a_carrier_the_engine_does_not_have_is_red(gate, tmp_path):
    row = dict(BOUND_ROW, id="gone", carrier={"kind": "dw", "code": "DW9998"})
    r = run(gate, tmp_path, row)
    assert r["verdict"] == "MISSING-CHECK"
    assert "DW9998" in r["detail"]


def test_a_named_invariant_that_no_longer_exists_is_red(gate, tmp_path):
    row = dict(
        BOUND_ROW,
        id="inv",
        carrier={"kind": "invariant", "test": "a_test_nobody_ever_wrote"},
    )
    assert run(gate, tmp_path, row)["verdict"] == "MISSING-CHECK"


def test_a_real_invariant_restores_it(gate, tmp_path):
    """`assert_distress_never_stacks` is the generator-side proof that scatter
    is embedded rather than stacked — the island's round-13 debris finding."""
    row = dict(
        BOUND_ROW,
        id="inv",
        carrier={"kind": "invariant", "test": "assert_distress_never_stacks"},
    )
    assert run(gate, tmp_path, row)["verdict"] == "BOUND"


def test_an_artifact_this_build_never_emitted_is_red(gate, tmp_path):
    row = dict(
        BOUND_ROW,
        id="art",
        carrier={"kind": "artifact", "file": "lethal-gate.json"},
        binding={"kind": "artifact", "file": "lethal-gate.json", "path": "cells"},
    )
    assert run(gate, tmp_path, row)["verdict"] == "MISSING-CHECK"


def test_the_same_artifact_binds_once_the_build_emits_it(gate, tmp_path):
    row = dict(
        BOUND_ROW,
        id="art",
        carrier={"kind": "artifact", "file": "lethal-gate.json"},
        binding={"kind": "artifact", "file": "lethal-gate.json", "path": "cells"},
    )
    r = run(gate, tmp_path, row, validation={"lethal-gate.json": {"cells": 42}})
    assert r["verdict"] == "BOUND"
    assert r["binding"] == 42


# ---------------------------------------------------------------------------
# Vacuity: the three ways this project's greens have lied
# ---------------------------------------------------------------------------


def test_a_check_that_matches_nothing_is_red(gate, tmp_path):
    """The island's floor gate examined zero enemies for nineteen rounds and
    was green every time."""
    r = run(gate, tmp_path, BOUND_ROW, objectives=[{"type": "narrate"}])
    assert r["verdict"] == "UNBOUND"
    assert r["binding"] == 0


def test_an_unbound_row_names_which_kind_of_zero_it_is(gate, tmp_path):
    """Objects that COULD carry the defect exist; the declaration the check
    keys off does not. That is the floor-gate shape, and it stays red."""
    row = dict(
        BOUND_ROW,
        id="z",
        applies_when={
            "kind": "dsl",
            "files": ["quests.json"],
            "match": {"eq": {"type": "narrate"}},
        },
    )
    r = run(gate, tmp_path, row, objectives=[{"type": "narrate"}])
    assert r["verdict"] == "UNBOUND"
    assert r["precondition"] == 1


def test_a_zero_precondition_is_labelled_inapplicable_and_still_red(gate, tmp_path):
    """A campaign declaring none of the objects the class needs cannot exercise
    it. That is a fact for the round summary — never a pass."""
    row = dict(
        BOUND_ROW,
        id="z",
        applies_when={
            "kind": "dsl",
            "files": ["quests.json"],
            "match": {"eq": {"type": "volley"}},
        },
    )
    r = run(gate, tmp_path, row, objectives=[{"type": "narrate"}])
    assert r["verdict"] == "INAPPLICABLE"
    assert r["precondition"] == 0
    assert r["verdict"] in gate.RED_VERDICTS


def test_a_campaign_below_the_checks_dsl_version_is_red(gate, tmp_path):
    """The island's four branch proofs were physically impossible before round
    19 declared `branch_points`, and read green throughout."""
    row = dict(
        BOUND_ROW, id="fence", requires={"file": "quests.json", "min_dsl_version": "0.10.0"}
    )
    r = run(gate, tmp_path, row, dsl_version="0.8.0")
    assert r["verdict"] == "UNFENCED"
    assert "0.8.0" in r["detail"]


def test_and_it_binds_once_the_campaign_adopts_that_version(gate, tmp_path):
    row = dict(
        BOUND_ROW, id="fence", requires={"file": "quests.json", "min_dsl_version": "0.10.0"}
    )
    assert run(gate, tmp_path, row, dsl_version="0.10.0")["verdict"] == "BOUND"


def test_the_fence_is_checked_before_the_binding_count(gate, tmp_path):
    """An unfenced campaign's zero is EXPLAINED by the fence. Reporting it as
    UNBOUND would send a reader hunting for objects that could not have been
    declared at that version."""
    row = dict(
        BOUND_ROW, id="fence", requires={"file": "quests.json", "min_dsl_version": "0.10.0"}
    )
    r = run(gate, tmp_path, row, objectives=[{"type": "narrate"}], dsl_version="0.8.0")
    assert r["verdict"] == "UNFENCED"


# ---------------------------------------------------------------------------
# A campaign that cannot be measured at all
# ---------------------------------------------------------------------------


def test_a_campaign_with_no_dsl_source_never_passes(gate, tmp_path):
    """The drowned-bell remake is in exactly this state: a design document and
    no stage JSON. A gate that shrugged at it would green-light the build this
    directive was written for."""
    empty = tmp_path / "nosrc"
    empty.mkdir()
    subj = gate.Subject(empty, make_build(tmp_path))
    r = gate.adjudicate(BOUND_ROW, gate.Engine(), subj)
    assert r["verdict"] == "NO-SOURCE"


# ---------------------------------------------------------------------------
# The ledger's own shape
# ---------------------------------------------------------------------------


def test_a_carrier_with_no_binding_probe_is_refused_outright(gate, tmp_path):
    """A carrier nobody counts is the exact vacuity this gate exists to expose,
    so it may not even be written down."""
    bad = tmp_path / "l.json"
    bad.write_text(
        json.dumps(
            {
                "findings": [
                    {"id": "x", "finding": "f", "carrier": {"kind": "dw", "code": LIVE_CODE}}
                ]
            }
        )
    )
    with pytest.raises(ValueError, match="no binding probe"):
        gate.load_ledger(bad)


def test_duplicate_finding_ids_are_refused(gate, tmp_path):
    bad = tmp_path / "l.json"
    bad.write_text(
        json.dumps({"findings": [{"id": "x", "finding": "a"}, {"id": "x", "finding": "b"}]})
    )
    with pytest.raises(ValueError, match="duplicate"):
        gate.load_ledger(bad)


def test_the_live_ledger_parses_and_every_row_is_well_formed(gate):
    """The shipped ledger is itself under test: a row that names a carrier
    without a binding probe, or reuses an id, breaks CI here rather than
    silently producing a green row."""
    doc = gate.load_ledger(gate.DEFAULT_LEDGER)
    assert len(doc["findings"]) > 50
    for r in doc["findings"]:
        if r.get("carrier") is None and r.get("disposition"):
            assert len(r.get("justification", "")) >= gate.MIN_JUSTIFICATION, r["id"]
        assert r["finding"][:1].islower() or r["finding"][:1].isupper(), r["id"]
