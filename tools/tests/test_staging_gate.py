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
import sys

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
# The blockout: a pre-detail site-plan subject (spec-0049)
#
# The conflict this answers: spec-0049 stages a whole-map walk BEFORE any
# content exists, and the gate's zero-binding verdicts redded precisely
# because no content exists — no green state, and the remedy each red named
# was the one thing the spec forbids doing first. The repair is a verdict the
# OBJECT determines: a site-plan campaign whose build the compiler records as
# derived massing, with no detail-plan document anywhere, may carry
# OUT-OF-STAGE for a row whose class measures zero at both binding and
# precondition. Nothing an operator types reaches it, and every test below
# drives it in both directions.
# ---------------------------------------------------------------------------


def make_blockout_campaign(tmp_path, *, objectives=None, detail_plan=False):
    d = make_campaign(
        tmp_path,
        objectives=objectives if objectives is not None else [{"type": "talk-to"}],
        dsl_version="0.14.0",
    )
    (d / "site-plan.json").write_text(
        json.dumps({"dsl_version": "0.14.0", "stage": "site-plan", "content": {"boxes": []}})
    )
    if detail_plan:
        (d / "detail-plan.json").write_text(
            json.dumps({"dsl_version": "0.15.0", "stage": "detail-plan", "content": {}})
        )
    return d


def make_blockout_build(tmp_path, *, validation=None, inputs=("quests.json", "site-plan.json")):
    b = make_build(tmp_path, validation=validation)
    (b / "manifest.json").write_text(
        json.dumps({"inputs": {k: "0" * 8 for k in inputs}, "outputs": {}})
    )
    return b


# An identity-shaped probe over a class the subject declares zero of.
IDENTITY_ZERO_ROW = {
    "id": "z",
    "finding": "f",
    "carrier": {"kind": "dw", "code": LIVE_CODE},
    "binding": {
        "kind": "dsl",
        "files": ["quests.json"],
        "match": {"eq": {"type": "volley"}},
    },
}


def adjudicate_on(gate, camp, build, row):
    return gate.adjudicate(row, gate.Engine(), gate.Subject(camp, build))


def test_an_identity_zero_on_a_blockout_is_out_of_stage_not_red(gate, tmp_path):
    """The probe selects the class by identity, so its zero counts the class
    itself: a measured double zero on a build that does not claim to be
    finished. Non-red, but never silent — the id lands in the token."""
    camp = make_blockout_campaign(tmp_path)
    build = make_blockout_build(tmp_path)
    r = adjudicate_on(gate, camp, build, IDENTITY_ZERO_ROW)
    assert r["verdict"] == "OUT-OF-STAGE"
    assert r["verdict"] not in gate.RED_VERDICTS
    assert r["binding"] == 0 and r["precondition"] == 0


def test_the_same_zero_on_an_assembled_campaign_stays_red(gate, tmp_path):
    """Absence on a build that claims to be finished is the news, exactly as
    before this verdict existed. Assembled adjudication is byte-for-byte the
    old behaviour."""
    r = run(gate, tmp_path, IDENTITY_ZERO_ROW, objectives=[{"type": "talk-to"}])
    assert r["verdict"] == "UNBOUND"


def test_a_detail_plan_document_ends_the_blockout_stage(gate, tmp_path):
    """The verdict is about a stage, never about a campaign: the day the
    campaign details, every OUT-OF-STAGE row reverts to red."""
    camp = make_blockout_campaign(tmp_path, detail_plan=True)
    build = make_blockout_build(tmp_path)
    r = adjudicate_on(gate, camp, build, IDENTITY_ZERO_ROW)
    assert r["verdict"] == "UNBOUND"


def test_a_manifest_not_compiled_from_the_site_plan_fails_closed(gate, tmp_path):
    """The stage claim needs the compiler's own record. A build whose manifest
    does not list the site plan among its inputs gets no blockout verdict,
    whatever the campaign directory says."""
    camp = make_blockout_campaign(tmp_path)
    build = make_blockout_build(tmp_path, inputs=("quests.json",))
    r = adjudicate_on(gate, camp, build, IDENTITY_ZERO_ROW)
    assert r["verdict"] == "UNBOUND"


def test_a_nonzero_precondition_stays_red_on_a_blockout(gate, tmp_path):
    """The quietly-lost direction: objects that could carry the defect exist
    (the precondition measures non-zero), and the check is inert over them.
    A blockout buys no forgiveness for that."""
    row = dict(
        IDENTITY_ZERO_ROW,
        id="lost",
        applies_when={
            "kind": "dsl",
            "files": ["quests.json"],
            "match": {"eq": {"type": "talk-to"}},
        },
    )
    camp = make_blockout_campaign(tmp_path)
    build = make_blockout_build(tmp_path)
    r = adjudicate_on(gate, camp, build, row)
    assert r["verdict"] == "UNBOUND"
    assert r["precondition"] == 1


def test_a_measured_double_zero_is_out_of_stage_only_at_pre_detail(gate, tmp_path):
    row = dict(
        IDENTITY_ZERO_ROW,
        id="dz",
        applies_when={
            "kind": "dsl",
            "files": ["quests.json"],
            "match": {"eq": {"type": "interact"}},
        },
    )
    blockout = tmp_path / "blockout"
    blockout.mkdir()
    camp = make_blockout_campaign(blockout)
    build = make_blockout_build(blockout)
    assert adjudicate_on(gate, camp, build, row)["verdict"] == "OUT-OF-STAGE"
    r = run(gate, tmp_path, row, objectives=[{"type": "talk-to"}])
    assert r["verdict"] == "INAPPLICABLE"
    assert r["verdict"] in gate.RED_VERDICTS


def test_a_declaration_shaped_zero_without_a_probe_stays_red_on_a_blockout(gate, tmp_path):
    """A `has` predicate can be narrower than its carriers (the island's floor
    gate counted `tier`, not actors), so its zero is ambiguous and the stage
    cannot answer it. The gate keeps refusing to guess, blockout or not."""
    row = dict(
        IDENTITY_ZERO_ROW,
        id="decl",
        binding={
            "kind": "dsl",
            "files": ["quests.json"],
            "match": {"has": ["container"]},
        },
    )
    camp = make_blockout_campaign(tmp_path)
    build = make_blockout_build(tmp_path)
    r = adjudicate_on(gate, camp, build, row)
    assert r["verdict"] == "UNBOUND"
    assert "never measured" in r["detail"]


def test_an_unemitted_artifact_with_declared_objects_is_missing_check(gate, tmp_path):
    """combat-plan.json absent while the campaign stages fights: the build
    lost its ledger. Red on every subject, blockout included."""
    row = dict(
        IDENTITY_ZERO_ROW,
        id="art",
        carrier={"kind": "dw", "code": LIVE_CODE},
        binding={"kind": "artifact", "file": "combat-plan.json", "path": "fights.total"},
        applies_when={
            "kind": "dsl",
            "files": ["quests.json"],
            "match": {"eq": {"type": "talk-to"}},
        },
    )
    camp = make_blockout_campaign(tmp_path)
    build = make_blockout_build(tmp_path)
    r = adjudicate_on(gate, camp, build, row)
    assert r["verdict"] == "MISSING-CHECK"
    assert "emitted no ledger" in r["detail"]


def test_an_unemitted_artifact_whose_class_measures_zero_is_explained(gate, tmp_path):
    """The compiler emits these ledgers only over objects that exist, so a
    measured-zero precondition explains the absence: INAPPLICABLE on an
    assembled subject (still red), OUT-OF-STAGE on a blockout."""
    row = dict(
        IDENTITY_ZERO_ROW,
        id="art0",
        binding={"kind": "artifact", "file": "combat-plan.json", "path": "fights.total"},
        applies_when={
            "kind": "dsl",
            "files": ["quests.json"],
            "match": {"eq": {"type": "volley"}},
        },
    )
    blockout = tmp_path / "blockout"
    blockout.mkdir()
    camp = make_blockout_campaign(blockout)
    build = make_blockout_build(blockout)
    assert adjudicate_on(gate, camp, build, row)["verdict"] == "OUT-OF-STAGE"
    r = run(gate, tmp_path, row, objectives=[{"type": "talk-to"}])
    assert r["verdict"] == "INAPPLICABLE"


def test_an_unemitted_artifact_without_a_probe_is_still_missing_check(gate, tmp_path):
    row = dict(
        IDENTITY_ZERO_ROW,
        id="artn",
        binding={"kind": "artifact", "file": "combat-plan.json", "path": "fights.total"},
    )
    camp = make_blockout_campaign(tmp_path)
    build = make_blockout_build(tmp_path)
    assert adjudicate_on(gate, camp, build, row)["verdict"] == "MISSING-CHECK"


def test_a_blockout_buys_nothing_for_the_other_reds(gate, tmp_path):
    """NO-GENERAL-FORM and UNFENCED are about the ledger and the engine, not
    about this build's content; the stage cannot touch them."""
    camp = make_blockout_campaign(tmp_path)
    build = make_blockout_build(tmp_path)
    ngf = {"id": "ngf", "finding": "f", "carrier": None}
    assert adjudicate_on(gate, camp, build, ngf)["verdict"] == "NO-GENERAL-FORM"
    fenced = dict(
        IDENTITY_ZERO_ROW,
        id="fence",
        requires={"file": "quests.json", "min_dsl_version": "99.0.0"},
    )
    assert adjudicate_on(gate, camp, build, fenced)["verdict"] == "UNFENCED"


def test_a_row_may_not_declare_its_own_binding_as_its_precondition(gate, tmp_path):
    """The self-grant guard: an exemption a row can grant itself is not a
    gate."""
    bad = tmp_path / "l.json"
    probe = {
        "kind": "dsl",
        "files": ["quests.json"],
        "match": {"eq": {"type": "volley"}},
    }
    bad.write_text(
        json.dumps(
            {
                "findings": [
                    {
                        "id": "x",
                        "finding": "f",
                        "carrier": {"kind": "dw", "code": LIVE_CODE},
                        "binding": probe,
                        "applies_when": probe,
                    }
                ]
            }
        )
    )
    with pytest.raises(ValueError, match="grant itself"):
        gate.load_ledger(bad)


def test_a_green_blockout_mints_a_token_the_verifier_announces(gate, tmp_path):
    """End to end: the gate passes an honestly-empty blockout, the token names
    the out-of-stage classes, and the boot banner reads them aloud — the
    owner is told the session's scope where the session starts."""
    camp = make_blockout_campaign(tmp_path, objectives=[{"type": "interact"}])
    tree = make_blockout_build(tmp_path)
    ledger = tmp_path / "led.json"
    ledger.write_text(
        json.dumps({"findings": [dict(BOUND_ROW, id="ok"), dict(IDENTITY_ZERO_ROW, id="oos")]})
    )
    proc = subprocess.run(
        [sys.executable, str(SCRIPT), "--campaign", str(camp), "--build", str(tree),
         "--ledger", str(ledger)],
        capture_output=True, text=True,
    )
    assert proc.returncode == 0, proc.stderr
    assert "OUT-OF-STAGE" in proc.stderr
    token = json.loads((tree / "staging-admission.json").read_text())
    assert token["pre_detail"] is True
    assert token["out_of_stage"] == ["oos"]
    assert token["out_of_stage_count"] == 1
    r = verify(tree)
    assert r.returncode == 0
    assert "BLOCKOUT WALK" in r.stderr
    assert "cannot exercise: 1" in r.stderr


def test_an_assembled_token_gets_no_blockout_banner(gate, tmp_path):
    r = verify(admitted_tree(tmp_path, gate))
    assert r.returncode == 0
    assert "BLOCKOUT WALK" not in r.stderr


def test_strict_fails_on_out_of_stage_rows(gate, tmp_path):
    """The absolute floor treats OUT-OF-STAGE like DECLARED-UNCOVERABLE."""
    camp = make_blockout_campaign(tmp_path, objectives=[{"type": "interact"}])
    tree = make_blockout_build(tmp_path)
    ledger = tmp_path / "led.json"
    ledger.write_text(json.dumps({"findings": [dict(IDENTITY_ZERO_ROW, id="oos")]}))
    proc = subprocess.run(
        [sys.executable, str(SCRIPT), "--campaign", str(camp), "--build", str(tree),
         "--ledger", str(ledger), "--strict"],
        capture_output=True, text=True,
    )
    assert proc.returncode == 1


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


# ---------------------------------------------------------------------------
# Admission: the part that makes the gate impossible to skip
#
# The gate shipped invoked by nothing but these tests — the UNRUN shape. What
# closes it is not a doc line but an artifact the owner-facing paths REQUIRE:
# `tools/playtest-server.sh` runs the gate itself, and
# `validation/owner-play.yaml` runs `validation/staging-admission.sh` as a
# service both 25565 binders `depends_on`. These drive the real verifier
# script, so a change to the token format reds here instead of silently
# admitting an unadmitted build.
# ---------------------------------------------------------------------------

import subprocess

VERIFIER = pathlib.Path(__file__).resolve().parents[2] / "validation" / "staging-admission.sh"


def verify(tree: pathlib.Path):
    return subprocess.run(
        ["bash", str(VERIFIER), str(tree)], capture_output=True, text=True
    )


def admitted_tree(tmp_path, gate, *, overridden=False):
    """A build tree carrying a real token, minted by the real gate."""
    tree = tmp_path / "out"
    (tree / "validation").mkdir(parents=True, exist_ok=True)
    (tree / "manifest.json").write_text('{"outputs": {"a": "b"}}')
    camp = make_campaign(tmp_path, objectives=[{"type": "interact"}])
    ledger = tmp_path / "led.json"
    rows = [dict(BOUND_ROW, id="ok")]
    if overridden:
        rows.append({"id": "red", "finding": "an uncovered class", "carrier": None})
    ledger.write_text(json.dumps({"findings": rows}))
    cmd = [
        sys.executable, str(SCRIPT),
        "--campaign", str(camp), "--build", str(tree), "--ledger", str(ledger),
    ]
    if overridden:
        cmd += [
            "--stage-anyway", "a deliberate look at one beat, not a QC round",
            "--acknowledge-red", "1",
        ]
    proc = subprocess.run(cmd, capture_output=True, text=True)
    assert proc.returncode == 0, proc.stderr
    return tree


def test_an_unadmitted_build_is_refused_by_the_verifier(gate, tmp_path):
    tree = tmp_path / "out"
    tree.mkdir()
    (tree / "manifest.json").write_text("{}")
    r = verify(tree)
    assert r.returncode == 1
    assert "no admission token" in r.stderr


def test_an_admitted_build_is_accepted(gate, tmp_path):
    r = verify(admitted_tree(tmp_path, gate))
    assert r.returncode == 0
    assert "admitted" in r.stderr


def test_a_refusal_revokes_an_existing_token(gate, tmp_path):
    """A tree green once and red now must not still carry its old token —
    that stale token is the bypass."""
    tree = admitted_tree(tmp_path, gate)
    assert (tree / "staging-admission.json").is_file()
    camp = tmp_path / "camp"
    red = tmp_path / "red.json"
    red.write_text(json.dumps({"findings": [{"id": "r", "finding": "uncovered", "carrier": None}]}))
    proc = subprocess.run(
        [sys.executable, str(SCRIPT), "--campaign", str(camp), "--build", str(tree),
         "--ledger", str(red)],
        capture_output=True, text=True,
    )
    assert proc.returncode == 1
    assert not (tree / "staging-admission.json").exists()
    assert verify(tree).returncode == 1


def test_a_token_minted_for_another_build_is_rejected(gate, tmp_path):
    """Run the gate green on one tree, serve another — the obvious bypass."""
    tree = admitted_tree(tmp_path, gate)
    (tree / "manifest.json").write_text('{"outputs": {"a": "CHANGED"}}')
    r = verify(tree)
    assert r.returncode == 1
    assert "DIFFERENT build tree" in r.stderr


def test_a_build_with_no_manifest_cannot_be_admitted(gate, tmp_path):
    tree = tmp_path / "out"
    tree.mkdir()
    camp = make_campaign(tmp_path, objectives=[{"type": "interact"}])
    ledger = tmp_path / "led.json"
    ledger.write_text(json.dumps({"findings": [dict(BOUND_ROW, id="ok")]}))
    proc = subprocess.run(
        [sys.executable, str(SCRIPT), "--campaign", str(camp), "--build", str(tree),
         "--ledger", str(ledger)],
        capture_output=True, text=True,
    )
    assert proc.returncode == 2
    assert "no manifest.json" in proc.stderr


def test_an_overridden_token_is_announced_not_waved_through(gate, tmp_path):
    """The banner failing open is the defect this pins: the first cut used
    BASIC sed with a `true\\|false` alternation, which matches nothing, so an
    overridden build reported as a clean one."""
    r = verify(admitted_tree(tmp_path, gate, overridden=True))
    assert r.returncode == 0
    assert "UNDER OVERRIDE" in r.stderr
    assert "not a new finding" in r.stderr
    assert "a deliberate look at one beat" in r.stderr


def test_the_override_needs_a_substantive_reason(gate, tmp_path):
    tree, camp, ledger = _red_setup(tmp_path)
    proc = _gate(tree, camp, ledger, "too busy", "1")
    assert proc.returncode == 2
    assert "needs a real reason" in proc.stderr


def test_the_override_needs_the_exact_current_red_count(gate, tmp_path):
    """A bare flag becomes habit; a number that moves cannot."""
    tree, camp, ledger = _red_setup(tmp_path)
    proc = _gate(tree, camp, ledger, "one framing check, deliberately not a QC round", "7")
    assert proc.returncode == 2
    assert "does not match" in proc.stderr
    assert not (tree / "staging-admission.json").exists()


def test_the_override_refuses_when_nothing_is_red(gate, tmp_path):
    """An override that overrode nothing would normalise the flag."""
    tree = tmp_path / "out"
    tree.mkdir()
    (tree / "manifest.json").write_text("{}")
    camp = make_campaign(tmp_path, objectives=[{"type": "interact"}])
    ledger = tmp_path / "led.json"
    ledger.write_text(json.dumps({"findings": [dict(BOUND_ROW, id="ok")]}))
    proc = _gate(tree, camp, ledger, "a reason long enough to pass the floor", "0")
    assert proc.returncode == 2
    assert "overrode nothing" in proc.stderr


def _red_setup(tmp_path):
    tree = tmp_path / "out"
    tree.mkdir()
    (tree / "manifest.json").write_text("{}")
    camp = make_campaign(tmp_path, objectives=[{"type": "interact"}])
    ledger = tmp_path / "led.json"
    ledger.write_text(json.dumps({"findings": [{"id": "r", "finding": "uncovered", "carrier": None}]}))
    return tree, camp, ledger


def _gate(tree, camp, ledger, reason, ack):
    return subprocess.run(
        [sys.executable, str(SCRIPT), "--campaign", str(camp), "--build", str(tree),
         "--ledger", str(ledger), "--stage-anyway", reason, "--acknowledge-red", ack],
        capture_output=True, text=True,
    )


def test_the_owner_facing_paths_actually_invoke_the_gate(gate):
    """The UNRUN tripwire. This gate was correct and called by nothing; a doc
    line is not an invocation. If either owner-facing path stops requiring
    admission, that is this test, not a review someone has to remember."""
    root = pathlib.Path(__file__).resolve().parents[2]
    server = (root / "tools" / "playtest-server.sh").read_text()
    assert "staging-gate.py" in server, "playtest-server.sh no longer runs the gate"
    # and it must run BEFORE the container exists, or a refusal costs a session
    assert server.index("staging-gate.py") < server.index("docker run"), \
        "the gate must run before the container is created"

    owner_play = (root / "validation" / "owner-play.yaml").read_text()
    assert "staging-admission" in owner_play
    assert owner_play.count("service_completed_successfully") >= 2, \
        "both 25565-publishing services must depend on the admission check"
