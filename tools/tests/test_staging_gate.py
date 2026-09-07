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


def test_a_check_that_matches_nothing_is_named_by_which_zero_it_is(gate, tmp_path):
    """The island's floor gate examined zero enemies for nineteen rounds and
    was green every time.

    This row selects its class by identity, so the zero is MEASURED, and a
    measured zero of the class across the declared design is `INAPPLICABLE`:
    counted, never silent, and not a refusal — the surface is optional and
    this campaign does not use it. Which zero a zero is, is the two tests
    below; that it is never an unremarked zero is this one."""
    r = run(gate, tmp_path, BOUND_ROW, objectives=[{"type": "narrate"}])
    assert r["verdict"] == "INAPPLICABLE"
    assert r["verdict"] not in gate.RED_VERDICTS
    assert r["verdict"] in gate.NOT_A_GAP_VERDICTS
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


def test_a_zero_precondition_is_labelled_inapplicable_and_does_not_refuse(gate, tmp_path):
    """A campaign declaring none of the objects the class needs cannot exercise
    it. That is a fact for the round summary — and it is not a refusal: the
    class it is about is an OPTIONAL surface this campaign did not use, and a
    surface the DSL requires cannot be absent from a build that compiled."""
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
    assert r["verdict"] not in gate.RED_VERDICTS


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


def test_the_same_zero_on_an_assembled_campaign_is_counted_inapplicable(gate, tmp_path):
    """The stage is the only thing that moves: `OUT-OF-STAGE` needs the
    twice-measured blockout and this subject is not one, so the same measured
    double zero is `INAPPLICABLE` — and never `UNBOUND`, whose whole content
    is that nobody looked. Neither refuses; what separates them is that only
    OUT-OF-STAGE makes a claim about a stage, which is why `--strict` reaches
    it and not this."""
    r = run(gate, tmp_path, IDENTITY_ZERO_ROW, objectives=[{"type": "talk-to"}])
    assert r["verdict"] not in gate.RED_VERDICTS
    assert r["verdict"] == "INAPPLICABLE"
    assert (r["binding"], r["precondition"]) == (0, 0)
    assert "never measured" not in r["detail"]


def test_an_identity_zero_on_an_assembled_campaign_is_measured_not_guessed(gate, tmp_path):
    """The defect this branch closes, driven both ways.

    A deliberately small campaign contains none of a past finding's objects.
    The row's probe COUNTED that — an identity predicate over the declared
    design has nothing standing behind it for a precondition probe to find —
    so reporting `UNBOUND`, whose detail says *which kind of zero this is was
    never measured*, was the gate stating its own ignorance where it held the
    number. Declaring one object of the class turns the same row green, which
    is what makes the zero a fact about the campaign."""
    absent = run(gate, tmp_path, IDENTITY_ZERO_ROW, objectives=[{"type": "talk-to"}])
    assert absent["verdict"] == "INAPPLICABLE"
    assert absent["verdict"] not in gate.RED_VERDICTS
    assert (absent["binding"], absent["precondition"]) == (0, 0)
    assert "selects the object class by identity" in absent["detail"]

    present = run(gate, tmp_path, IDENTITY_ZERO_ROW, objectives=[{"type": "volley"}])
    assert present["verdict"] == "BOUND"
    assert present["binding"] == 1


def test_a_declaration_shaped_zero_on_an_assembled_campaign_still_says_nobody_looked(
    gate, tmp_path
):
    """The bound, on the subject where it matters most. A `has` predicate can
    be narrower than its carriers, so its zero is the island's floor gate and
    the gate must keep demanding a probe — on an assembled campaign exactly as
    on a blockout. Perturbed the only way this branch could answer wrongly:
    the same binding, the same campaign, one declared `applies_when`, and the
    zero becomes classifiable."""
    row = dict(
        IDENTITY_ZERO_ROW,
        id="decl-assembled",
        binding={"kind": "dsl", "files": ["quests.json"], "match": {"has": ["container"]}},
    )
    r = run(gate, tmp_path, row, objectives=[{"type": "collect"}])
    assert r["verdict"] == "UNBOUND"
    assert "never measured" in r["detail"]

    carriers_exist = dict(
        row,
        applies_when={
            "kind": "dsl", "files": ["quests.json"], "match": {"eq": {"type": "collect"}},
        },
    )
    r = run(gate, tmp_path, carriers_exist, objectives=[{"type": "collect"}])
    assert r["verdict"] == "UNBOUND"
    assert r["precondition"] == 1
    assert "never measured" not in r["detail"]

    class_absent = dict(
        row,
        applies_when={
            "kind": "dsl", "files": ["quests.json"], "match": {"eq": {"type": "volley"}},
        },
    )
    r = run(gate, tmp_path, class_absent, objectives=[{"type": "collect"}])
    assert r["verdict"] == "INAPPLICABLE"
    assert r["precondition"] == 0


def test_a_detail_plan_document_ends_the_blockout_stage(gate, tmp_path):
    """The verdict is about a stage, never about a campaign: the day the
    campaign details, no row carries the blockout allowance any more — the
    same measured double zero is adjudicated afresh as `INAPPLICABLE`, out of
    the boot banner and out of `--strict`'s reach."""
    camp = make_blockout_campaign(tmp_path, detail_plan=True)
    build = make_blockout_build(tmp_path)
    r = adjudicate_on(gate, camp, build, IDENTITY_ZERO_ROW)
    assert r["verdict"] == "INAPPLICABLE"


def test_a_manifest_not_compiled_from_the_site_plan_fails_closed(gate, tmp_path):
    """The stage claim needs the compiler's own record. A build whose manifest
    does not list the site plan among its inputs gets no blockout verdict,
    whatever the campaign directory says."""
    camp = make_blockout_campaign(tmp_path)
    build = make_blockout_build(tmp_path, inputs=("quests.json",))
    r = adjudicate_on(gate, camp, build, IDENTITY_ZERO_ROW)
    assert r["verdict"] != "OUT-OF-STAGE"
    assert r["verdict"] == "INAPPLICABLE"


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


# ---------------------------------------------------------------------------
# A campaign-source FILE class is self-measuring too
#
# The storybook is `campaigns/<id>/README.md` — a file, not a declaration. No
# stage document says a campaign has one, so the only probe that could answer
# "does this campaign publish release notes" is the binding itself, which
# `load_ledger` refuses outright. The property `probe_is_self_measuring` tests
# is about WHAT A PROBE COUNTS, and it was keyed to `kind == "dsl"`; a
# `campaign` glob with no `contains` counts the object class itself.
#
# That clause is a LOOSENING, so every test below drives one edge of its bound.
# ---------------------------------------------------------------------------


STORYBOOK_ROW = {
    "id": "book",
    "finding": "f",
    "carrier": {"kind": "tool", "script": "check-storybook-version.py"},
    "binding": {"kind": "campaign", "glob": "README*.md"},
}


def test_an_absent_campaign_file_class_on_a_blockout_is_out_of_stage(gate, tmp_path):
    """The blocker this clause exists for: the storybook is written at
    skill-workflow step 14 and the blockout is walked at step 9, so EVERY
    campaign at blockout carries zero of them."""
    camp = make_blockout_campaign(tmp_path)
    r = adjudicate_on(gate, camp, make_blockout_build(tmp_path), STORYBOOK_ROW)
    assert r["verdict"] == "OUT-OF-STAGE"
    assert (r["binding"], r["precondition"]) == (0, 0)


def test_the_same_missing_storybook_on_an_assembled_campaign_is_inapplicable(gate, tmp_path):
    """The file class is self-measuring on every subject, so the verdict is the
    measured `INAPPLICABLE`; only the blockout stage turns that same pair of
    zeros into `OUT-OF-STAGE`."""
    camp = make_campaign(tmp_path, objectives=[{"type": "talk-to"}])
    build = make_build(tmp_path)
    r = adjudicate_on(gate, camp, build, STORYBOOK_ROW)
    assert r["verdict"] == "INAPPLICABLE"


def test_the_storybook_is_readjudicated_once_the_campaign_details(gate, tmp_path):
    camp = make_blockout_campaign(tmp_path, detail_plan=True)
    r = adjudicate_on(gate, camp, make_blockout_build(tmp_path), STORYBOOK_ROW)
    assert r["verdict"] == "INAPPLICABLE"


def test_a_contains_glob_with_candidates_present_stays_red_on_a_blockout(gate, tmp_path):
    """The opposite shape, and the one that must NOT be widened: a `contains`
    glob counts a declaration INSIDE carriers that exist. Two storybooks, no
    marker in either, is the floor gate's zero wearing a file's clothes."""
    camp = make_blockout_campaign(tmp_path)
    (camp / "README.md").write_text("# a delve\n")
    (camp / "README.zh-CN.md").write_text("# a delve\n")
    row = dict(
        STORYBOOK_ROW,
        id="marker",
        binding={
            "kind": "campaign",
            "glob": "README*.md",
            "contains": "Requires delve engine",
        },
    )
    r = adjudicate_on(gate, camp, make_blockout_build(tmp_path), row)
    assert r["verdict"] == "UNBOUND"
    assert "of 2 candidates" in r["detail"]


def test_a_directory_standing_where_the_file_belongs_fails_closed(gate, tmp_path):
    """`is_file()` answers an honest False for a directory, so the count would
    be a wrong measurement rather than a measured zero. Any non-file match
    withdraws the self-measuring claim."""
    camp = make_blockout_campaign(tmp_path)
    (camp / "README.md").mkdir()
    r = adjudicate_on(gate, camp, make_blockout_build(tmp_path), STORYBOOK_ROW)
    assert r["verdict"] == "UNBOUND"


def test_a_storybook_that_is_there_binds(gate, tmp_path):
    camp = make_blockout_campaign(tmp_path)
    (camp / "README.md").write_text("# a delve\n")
    r = adjudicate_on(gate, camp, make_blockout_build(tmp_path), STORYBOOK_ROW)
    assert r["verdict"] == "BOUND"
    assert r["binding"] == 1


def test_an_out_glob_zero_is_not_self_measuring_on_a_blockout(gate, tmp_path):
    """The clause is about the campaign SOURCE. An `out` glob counts derived
    output one step from a declaration, and stays ambiguous — which is why
    tm-03 needed a real precondition rather than this."""
    row = dict(
        STORYBOOK_ROW,
        id="derived",
        binding={
            "kind": "out",
            "glob": "packtest-datapack/**/declared_difficulty.mcfunction",
        },
    )
    camp = make_blockout_campaign(tmp_path)
    r = adjudicate_on(gate, camp, make_blockout_build(tmp_path), row)
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
    """NO-GENERAL-FORM is about the ledger and the engine, not about this
    build's content; the stage cannot touch it."""
    camp = make_blockout_campaign(tmp_path)
    build = make_blockout_build(tmp_path)
    ngf = {"id": "ngf", "finding": "f", "carrier": None}
    assert adjudicate_on(gate, camp, build, ngf)["verdict"] == "NO-GENERAL-FORM"


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
# An ABSENT optional stage document: "this campaign declares none" vs
# "I could not read one"
#
# Five of the eleven stage documents are optional (`compiler::load`), so a
# campaign that ships no `world-edits.json` is not a campaign missing a
# document. The probe used to answer `None` for both readings, which reported
# MISSING-CHECK — a promise nobody keeps — against a campaign whose only
# offence was declaring no edit stage.
#
# The fact that separates them is the COMPILER's, never the filesystem's:
# `manifest.json` `inputs` is written from the bytes `load_campaign_dir`
# actually read. A directory or an unreadable file standing where a document
# belongs is exactly what a broken campaign looks like on disk, and
# `load::optional` treats ONLY `NotFound` as absent — so such a campaign does
# not build, has no manifest, and can never present this witness. Every test
# below drives that in both directions.
# ---------------------------------------------------------------------------


OPTIONAL_DOC_ROW = {
    "id": "w",
    "finding": "f",
    "carrier": {"kind": "dw", "code": LIVE_CODE},
    "binding": {
        "kind": "dsl",
        "files": ["world-edits.json"],
        "match": {"prefix": {"id": "batch/"}},
    },
}


def test_a_document_no_build_input_names_is_a_measured_zero(gate, tmp_path):
    """The compiler compiled this world and read no such document, so the
    campaign declares none — a count, not a shrug."""
    camp = make_blockout_campaign(tmp_path)
    build = make_blockout_build(tmp_path)
    count, detail = gate.probe(OPTIONAL_DOC_ROW["binding"], gate.Subject(camp, build))
    assert count == 0, detail
    assert "manifest.json inputs" in detail


def test_the_same_document_in_the_manifest_is_missing_check(gate, tmp_path):
    """ONE variable: the compiler DID read a world-edits document. The campaign
    source is byte-identical to the test above, and the exemption vanishes.

    This is the perturbation the widening is the only thing that could answer
    differently, and it fails in the refusing direction: a document the
    compiler read and this gate cannot parse is format rot, which the probe had
    no way to report at all before."""
    camp = make_blockout_campaign(tmp_path)
    build = make_blockout_build(
        tmp_path, inputs=("quests.json", "site-plan.json", "world-edits.json")
    )
    r = adjudicate_on(gate, camp, build, OPTIONAL_DOC_ROW)
    assert r["verdict"] == "MISSING-CHECK"
    assert r["verdict"] in gate.RED_VERDICTS


def test_a_build_that_cannot_say_what_it_read_fails_closed(gate, tmp_path):
    """No manifest = no witness. "I could not look" is restored, not guessed."""
    camp = make_blockout_campaign(tmp_path)
    build = make_build(tmp_path)  # deliberately no manifest.json
    r = adjudicate_on(gate, camp, build, OPTIONAL_DOC_ROW)
    assert r["verdict"] == "MISSING-CHECK"
    assert "cannot say which documents the compiler read" in r["detail"]


def test_a_manifest_with_no_inputs_object_fails_closed(gate, tmp_path):
    """A manifest whose `inputs` is not a mapping answers nothing, and
    answering nothing is not the same as answering zero."""
    camp = make_blockout_campaign(tmp_path)
    build = make_blockout_build(tmp_path)
    (build / "manifest.json").write_text(json.dumps({"inputs": "gone", "outputs": {}}))
    r = adjudicate_on(gate, camp, build, OPTIONAL_DOC_ROW)
    assert r["verdict"] == "MISSING-CHECK"


def test_the_measured_zero_is_adjudicated_unchanged_on_an_assembled_campaign(gate, tmp_path):
    """The widening adds NO verdict of its own. The zero is handed to the
    unchanged adjudication, and off a pre-detail blockout that is the counted
    `INAPPLICABLE` exactly as every other measured zero of a class is."""
    camp = make_campaign(tmp_path, objectives=[{"type": "talk-to"}])
    build = make_build(tmp_path)
    (build / "manifest.json").write_text(
        json.dumps({"inputs": {"quests.json": "0" * 8}, "outputs": {}})
    )
    r = adjudicate_on(gate, camp, build, OPTIONAL_DOC_ROW)
    assert r["verdict"] == "INAPPLICABLE"
    assert r["binding"] == 0


def test_the_measured_zero_reaches_out_of_stage_only_on_a_blockout(gate, tmp_path):
    """And on a blockout it reaches the verdict that already existed for a
    measured double zero — no new verdict is minted anywhere."""
    camp = make_blockout_campaign(tmp_path)
    build = make_blockout_build(tmp_path)
    r = adjudicate_on(gate, camp, build, OPTIONAL_DOC_ROW)
    assert r["verdict"] == "OUT-OF-STAGE"
    assert r["binding"] == 0 and r["precondition"] == 0


def test_a_present_document_never_reaches_the_branch(gate, tmp_path):
    """Nothing changes for a campaign that HAS the document: it is read and
    counted, and the manifest is not consulted."""
    camp = make_blockout_campaign(tmp_path)
    (camp / "world-edits.json").write_text(
        json.dumps(
            {
                "dsl_version": "0.14.0",
                "stage": "world-edits",
                "content": {"batches": [{"id": "batch/one"}]},
            }
        )
    )
    build = make_blockout_build(tmp_path)  # manifest deliberately does NOT list it
    r = adjudicate_on(gate, camp, build, OPTIONAL_DOC_ROW)
    assert r["verdict"] == "BOUND"
    assert r["binding"] == 1


def test_a_dsl_probe_may_not_name_a_document_this_gate_cannot_read(gate, tmp_path):
    """The guard that keeps the widening honest. A mistyped filename is in no
    build's inputs, so it would measure zero on every campaign forever — a
    check gone quiet, which is exactly what this tool exists to expose. It is a
    load-time refusal, so it reaches every campaign before any verdict."""
    good = {
        "findings": [
            dict(OPTIONAL_DOC_ROW, applies_when={
                "kind": "dsl", "files": ["quests.json"], "match": {"eq": {"type": "x"}},
            })
        ]
    }
    p = tmp_path / "ok.json"
    p.write_text(json.dumps(good))
    assert gate.load_ledger(p)["findings"], "the well-formed ledger must load"

    for key in ("binding", "applies_when"):
        bad = json.loads(json.dumps(good))
        bad["findings"][0][key] = {
            "kind": "dsl", "files": ["worldedits.json"], "match": {"prefix": {"id": "batch/"}},
        }
        q = tmp_path / f"bad-{key}.json"
        q.write_text(json.dumps(bad))
        with pytest.raises(ValueError, match="not a stage document this gate reads"):
            gate.load_ledger(q)


# ---------------------------------------------------------------------------
# The LIVE ledger's preconditions, driven over synthetic campaigns
#
# The tests above prove the mechanism. These prove the DATA: each
# `applies_when` added to the live ledger must go non-zero on a campaign that
# declares the carrier the engine says the check quantifies over, and zero on
# one that does not. A precondition that can only measure zero is an exemption
# a row grants itself with extra steps.
# ---------------------------------------------------------------------------


def live_rows(gate, ids):
    doc = gate.load_ledger(gate.DEFAULT_LEDGER)
    rows = {r["id"]: r for r in doc["findings"] if r["id"] in ids}
    assert set(rows) == set(ids), f"live ledger is missing {set(ids) - set(rows)}"
    return rows


def test_the_live_item_gate_precondition_binds_on_an_interact_objective(gate, tmp_path):
    """`DW0849`'s catalogue row states its own binding: *interact objectives
    declaring requires_item*. So the carriers are interact objectives, and
    `requires_item` exists on exactly one enum variant. Driven both ways."""
    row = live_rows(gate, {"isl-02"})["isl-02"]
    aw = row["applies_when"]

    (tmp_path / "a").mkdir()
    (tmp_path / "b").mkdir()
    carrying = make_campaign(tmp_path / "a", objectives=[{"type": "interact"}])
    without = make_campaign(tmp_path / "b", objectives=[{"type": "talk-to"}])
    build = make_build(tmp_path)
    n_yes = gate.probe(aw, gate.Subject(carrying, build))[0]
    n_no = gate.probe(aw, gate.Subject(without, build))[0]
    assert (n_yes, n_no) == (1, 0)

    # And the row's verdict follows: a campaign that HAS the carrier and has
    # not declared the field is refused, which is the whole point.
    assert adjudicate_on(gate, carrying, build, row)["verdict"] == "UNBOUND"


def test_the_live_flask_precondition_binds_on_a_potion_bearing_kit_item(gate, tmp_path):
    """`DW0487` fires on a POTION-BEARING kit item, and the carriers are the
    four ids of `dsl::stages::POTION_BEARING_ITEMS` — in both the namespaced
    and bare forms `is_potion_bearing_item` normalises. A leather boot cannot
    carry a placeholder-flask defect, which is why the precondition is those
    four items and not "kit items"."""
    rows = live_rows(gate, {"isl-05", "bell-16"})
    aw = rows["isl-05"]["applies_when"]
    assert aw == rows["bell-16"]["applies_when"], "one rule, one precondition"

    build = make_build(tmp_path)

    def classes(where, kit):
        d = tmp_path / where
        d.mkdir(parents=True, exist_ok=True)
        (d / "classes.json").write_text(
            json.dumps(
                {
                    "dsl_version": "0.14.0",
                    "stage": "classes",
                    "content": {"classes": [{"id": "class/a", "kit": kit}]},
                }
            )
        )
        return d

    bare = classes("bare", [{"item": "potion", "count": 1}])
    full = classes("full", [{"item": "minecraft:splash_potion", "count": 1}])
    none = classes("none", [{"item": "minecraft:bread", "count": 3}])
    counts = {
        w: gate.probe(aw, gate.Subject(d, build))[0]
        for w, d in (("bare", bare), ("full", full), ("none", none))
    }
    assert counts == {"bare": 1, "full": 1, "none": 0}, counts


def test_the_live_difficulty_precondition_binds_on_a_declaring_world(gate, tmp_path):
    """`emit.rs` emits the `declared_difficulty` PackTest only
    `if let Some(diff) = declared_difficulty(c)`, so the class that check
    quantifies over is campaigns declaring `world.difficulty` — absent, the
    compiler's historical derivation ships and there is no declaration for a
    server configuration to be emitted from. `WorldContent.difficulty` is the
    only `difficulty` field in the DSL, so the `has` predicate is unambiguous.
    Driven both ways, and then through the row's verdict."""
    row = live_rows(gate, {"tm-03"})["tm-03"]
    aw = row["applies_when"]

    def world(where, content):
        d = tmp_path / where
        d.mkdir(parents=True, exist_ok=True)
        (d / "world.json").write_text(
            json.dumps({"dsl_version": "0.14.0", "stage": 1, "content": content})
        )
        return d

    declaring = world("yes", {"title": "t", "difficulty": "hard"})
    deriving = world("no", {"title": "t"})
    build = make_build(tmp_path)
    n_yes = gate.probe(aw, gate.Subject(declaring, build))[0]
    n_no = gate.probe(aw, gate.Subject(deriving, build))[0]
    assert (n_yes, n_no) == (1, 0)

    # And the verdict follows: a campaign that DECLARES a difficulty whose
    # build emitted no PackTest for it is the check going quiet over an object
    # it should have something to say about.
    assert adjudicate_on(gate, declaring, build, row)["verdict"] == "UNBOUND"


def dialogue_campaign(tmp_path, where, nodes):
    d = tmp_path / where
    d.mkdir(parents=True, exist_ok=True)
    (d / "dialogue.json").write_text(
        json.dumps(
            {
                "dsl_version": "0.21.1",
                "stage": "dialogue",
                "content": {"dialogues": [{"npc": "npc/a", "root": "dlg/r", "nodes": nodes}]},
            }
        )
    )
    return d


def test_the_live_label_precondition_binds_on_a_dialogue_node(gate, tmp_path):
    """`DW0331` and `DW0205` both read an option `label`, which is a
    declaration INSIDE a dialogue node — so the zero is ambiguous and the rows
    owed a precondition. The carriers are the nodes themselves, whose id form
    `dlg/<kebab>` is what `DW0110` enforces. Driven both ways, then through
    the verdict that matters: a campaign with dialogue whose options carry no
    label is the check going quiet over objects it should have something to
    say about."""
    rows = live_rows(gate, {"hv-07", "isl-49", "isl-55"})
    aw = rows["hv-07"]["applies_when"]
    assert all(r["applies_when"] == aw for r in rows.values()), "one class, one precondition"

    labelled = dialogue_campaign(
        tmp_path, "yes", [{"id": "dlg/r", "text": "t", "options": [{"label": "go"}]}]
    )
    unlabelled = dialogue_campaign(tmp_path, "mute", [{"id": "dlg/r", "text": "t"}])
    silent = dialogue_campaign(tmp_path, "none", [])
    build = make_build(tmp_path)
    counts = {
        w: gate.probe(aw, gate.Subject(d, build))[0]
        for w, d in (("yes", labelled), ("mute", unlabelled), ("none", silent))
    }
    assert counts == {"yes": 1, "mute": 1, "none": 0}, counts

    for r in rows.values():
        assert adjudicate_on(gate, labelled, build, r)["verdict"] == "BOUND"
        assert adjudicate_on(gate, unlabelled, build, r)["verdict"] == "UNBOUND"
        assert adjudicate_on(gate, silent, build, r)["verdict"] == "INAPPLICABLE"


def test_the_live_cast_precondition_binds_on_a_declared_quest(gate, tmp_path):
    """`DW0460`/`DW0461` read the `cast` ledger, a declaration ON a quest. The
    precondition counts the quests — and its UNBOUND direction is the island
    before round 13 verbatim: three quests, no cast ledger, nothing counting
    them."""
    rows = live_rows(gate, {"isl-35", "isl-46"})
    aw = rows["isl-35"]["applies_when"]
    assert rows["isl-46"]["applies_when"] == aw, "one class, one precondition"

    build = make_build(tmp_path)

    def quests(where, quest_nodes):
        d = tmp_path / where
        d.mkdir(parents=True, exist_ok=True)
        (d / "quests.json").write_text(
            json.dumps(
                {"dsl_version": "0.21.1", "stage": 5, "content": {"quests": quest_nodes}}
            )
        )
        return d

    with_cast = quests("cast", [{"id": "quest/a", "cast": {"npc/a": {"at": "anchor/a"}}}])
    no_cast = quests("nocast", [{"id": "quest/a", "objectives": []}])
    no_quest = quests("noquest", [])
    counts = {
        w: gate.probe(aw, gate.Subject(d, build))[0]
        for w, d in (("cast", with_cast), ("nocast", no_cast), ("noquest", no_quest))
    }
    assert counts == {"cast": 1, "nocast": 1, "noquest": 0}, counts

    for r in rows.values():
        assert adjudicate_on(gate, with_cast, build, r)["verdict"] == "BOUND"
        assert adjudicate_on(gate, no_cast, build, r)["verdict"] == "UNBOUND"
        assert adjudicate_on(gate, no_quest, build, r)["verdict"] == "INAPPLICABLE"


def test_the_live_prompt_precondition_binds_on_a_completable_objective(gate, tmp_path):
    """`DW0862` refuses a `hint` on an objective with no `title`, so the class
    is the objectives a prompt can be authored on. The binding pairs those
    five types with `has: [id]` — which is what keeps it off a sub-node that
    happens to carry a `type`, and also what makes the probe
    declaration-shaped. The precondition counts the types alone: a campaign
    that declares none of them cannot mislead anybody about one."""
    row = live_rows(gate, {"bell-13"})["bell-13"]
    aw = row["applies_when"]
    build = make_build(tmp_path)

    for sub in ("a", "b", "c"):
        (tmp_path / sub).mkdir()
    completable = make_campaign(tmp_path / "a", objectives=[{"id": "obj/a", "type": "collect"}])
    only_beats = make_campaign(tmp_path / "b", objectives=[{"id": "obj/a", "type": "narrate"}])
    n_yes = gate.probe(aw, gate.Subject(completable, build))[0]
    n_no = gate.probe(aw, gate.Subject(only_beats, build))[0]
    assert (n_yes, n_no) == (1, 0)

    assert adjudicate_on(gate, completable, build, row)["verdict"] == "BOUND"
    assert adjudicate_on(gate, only_beats, build, row)["verdict"] == "INAPPLICABLE"

    # The UNBOUND direction the `has: [id]` guard exists for: a node of one of
    # those types that carries no id is an anomaly, not an absence.
    anomalous = make_campaign(tmp_path / "c", objectives=[{"type": "collect"}])
    r = adjudicate_on(gate, anomalous, build, row)
    assert r["verdict"] == "UNBOUND"
    assert r["precondition"] == 1


def test_every_live_precondition_can_measure_non_zero(gate):
    """A precondition that no campaign shape could ever satisfy is the sixth
    vacuity mode with a probe's face on. The count below is COMPUTED from the
    ledger, never written down beside it — a constant asserted against itself
    is the vacuity this whole file exists to refuse."""
    doc = gate.load_ledger(gate.DEFAULT_LEDGER)
    rows = [r for r in doc["findings"] if r.get("applies_when") is not None]
    assert len(rows) >= 17, f"the ledger has stopped declaring preconditions: {len(rows)}"
    for r in rows:
        aw = r["applies_when"]
        assert aw != r["binding"], r["id"]
        if aw.get("kind") == "dsl":
            assert aw.get("match"), f"{r['id']}: a precondition matching anything is not one"


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


# ---------------------------------------------------------------------------
# The refusal is on PRESENCE, not on absence
#
# `INAPPLICABLE` stopped being a refusal, so the boundary it moved has to be
# driven from both sides — and from fixtures, never from the live ledger, so
# that what is proved is the RULE and not today's data. The class below is a
# real one: an `interact` objective declaring `requires_item`, the island's
# item-gate finding. One variable moves between the two subjects, the presence
# of an object of the class, and it decides refuse/admit.
#
# A campaign that declares no gate, no wave, no mount has used an OPTIONAL
# surface not at all, which is a design choice; a surface the DSL requires
# cannot be absent from a build that compiled at all. What must still refuse is
# the other side: the class is here and no check binds to it.
# ---------------------------------------------------------------------------


PRESENT_CLASS_ROW = {
    "id": "gate",
    "finding": "an item gate refused a player who was carrying the item",
    "carrier": {"kind": "dw", "code": LIVE_CODE},
    "binding": {
        "kind": "dsl",
        "files": ["quests.json"],
        "match": {"has": ["requires_item"]},
    },
    "applies_when": {
        "kind": "dsl",
        "files": ["quests.json"],
        "match": {"eq": {"type": "interact"}},
    },
}


def gate_cli(tmp_path, where, objectives, rows, *extra):
    """Run the real gate end to end over a fixture campaign, build and ledger."""
    root = tmp_path / where
    root.mkdir(parents=True, exist_ok=True)
    camp = make_campaign(root, objectives=objectives)
    tree = make_build(root)
    (tree / "manifest.json").write_text(
        json.dumps({"inputs": {"quests.json": "0" * 8}, "outputs": {"a": where}})
    )
    ledger = root / "led.json"
    ledger.write_text(json.dumps({"findings": rows}))
    proc = subprocess.run(
        [sys.executable, str(SCRIPT), "--campaign", str(camp), "--build", str(tree),
         "--ledger", str(ledger), *extra],
        capture_output=True, text=True,
    )
    return proc, tree


def test_an_object_of_the_class_with_nothing_binding_still_refuses(gate, tmp_path):
    """The direction that must never soften. The campaign DECLARES an interact
    objective — an object that could carry the item-gate defect — and the
    check that would catch it counts zero of them, because the declaration it
    keys off is not there. That is the island's floor gate exactly: carriers
    present, check inert. The gate refuses, mints no token, and the verifier
    refuses the tree after it."""
    proc, tree = gate_cli(
        tmp_path, "present", [{"type": "interact"}], [dict(PRESENT_CLASS_ROW)]
    )
    assert proc.returncode == 1, proc.stdout + proc.stderr
    assert "UNBOUND" in proc.stderr
    assert not (tree / "staging-admission.json").exists()
    assert verify(tree).returncode == 1


def test_the_same_campaign_without_the_object_is_admitted(gate, tmp_path):
    """One variable moves: the interact objective is gone, so the class has no
    object in this build at all. Nothing was skipped — an optional surface was
    not used — and holding the build for it would make every small delve
    unstageable forever, for reasons about other people's campaigns."""
    proc, tree = gate_cli(
        tmp_path, "absent", [{"type": "narrate"}], [dict(PRESENT_CLASS_ROW)]
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    token = json.loads((tree / "staging-admission.json").read_text())
    assert token["red_count"] == 0
    # Admitted, and never silent about what it could not exercise.
    assert token["inapplicable"] == ["gate"]
    assert token["inapplicable_count"] == 1
    assert "INAPPLICABLE" in proc.stderr
    assert "cannot meet them" in proc.stderr


def test_declaring_the_field_the_check_reads_binds_the_row(gate, tmp_path):
    """The third subject, so the pair above is not two ways of measuring
    nothing: declare the field and the same row is BOUND."""
    proc, tree = gate_cli(
        tmp_path,
        "bound",
        [{"type": "interact", "requires_item": "key/vault"}],
        [dict(PRESENT_CLASS_ROW)],
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr
    token = json.loads((tree / "staging-admission.json").read_text())
    assert token["inapplicable_count"] == 0


def test_the_headline_counts_inapplicable_in_its_own_words(gate, tmp_path):
    """A pass that could not exercise a class must not read as coverage: the
    count is in the headline and the rows are listed under a heading that says
    what the zero means."""
    proc, _ = gate_cli(
        tmp_path, "headline", [{"type": "narrate"}], [dict(PRESENT_CLASS_ROW)]
    )
    assert proc.returncode == 0, proc.stderr
    assert "- `INAPPLICABLE`: 1 " in proc.stdout
    assert "an optional surface this campaign does not use" in proc.stdout
    assert "## Inapplicable — classes this campaign contains no object of" in proc.stdout


def test_the_verifier_announces_what_this_session_cannot_meet(gate, tmp_path):
    """The token is read where the session starts. A build admitted with
    classes it contains none of says so at boot, for the same reason the
    override banner does."""
    proc, tree = gate_cli(
        tmp_path, "banner", [{"type": "narrate"}], [dict(PRESENT_CLASS_ROW)]
    )
    assert proc.returncode == 0, proc.stderr
    r = verify(tree)
    assert r.returncode == 0
    assert "contains no object of 1 past finding class(es)" in r.stderr
    assert "never coverage" in r.stderr


def test_strict_does_not_restore_the_old_rule(gate, tmp_path):
    """`--strict` is the floor for rows a DECLARATION excused. Nothing declared
    an INAPPLICABLE row: the class measured zero across the design. A flag that
    failed on it would be "resemble the campaigns we happened to test" under
    another name."""
    proc, _ = gate_cli(
        tmp_path, "strict", [{"type": "narrate"}], [dict(PRESENT_CLASS_ROW)], "--strict"
    )
    assert proc.returncode == 0, proc.stdout + proc.stderr


# ---------------------------------------------------------------------------
# What the non-refusal is secured by: a count over the CAMPAIGN SOURCE
#
# `INAPPLICABLE` is now an admission, so the property it rests on has to be one
# the defect cannot supply. A zero counted in the BUILD tree is exactly what a
# defect manufactures — stop emitting the ledger and the class "disappears"
# from a build whose campaign still declares it. So at least one of the two
# counts must be taken over the campaign source, which only the author can
# move. Zero live ledger rows are of the refused shape today (all 94 measure
# one side over the source); it binds against the row nobody has written yet,
# and both directions are driven here.
# ---------------------------------------------------------------------------


BUILD_SIDE_PRECONDITION = {"kind": "out", "glob": "**/*.mcfunction", "contains": "wave"}
SOURCE_SIDE_PRECONDITION = {
    "kind": "dsl",
    "files": ["quests.json"],
    "match": {"eq": {"type": "volley"}},
}


def test_a_double_zero_counted_only_in_the_build_tree_refuses(gate, tmp_path):
    """An unemitted artifact whose precondition is also derived output. Both
    numbers are zero and both came from the same place the defect would act,
    so nothing here measured the campaign's declared design."""
    row = dict(
        PRESENT_CLASS_ROW,
        id="derived",
        binding={"kind": "artifact", "file": "combat-plan.json", "path": "fights.total"},
        applies_when=BUILD_SIDE_PRECONDITION,
    )
    r = run(gate, tmp_path, row, objectives=[{"type": "narrate"}])
    assert r["verdict"] == "UNBOUND"
    assert r["verdict"] in gate.RED_VERDICTS
    assert "counted in the BUILD tree" in r["detail"]


def test_moving_the_precondition_to_the_source_makes_the_absence_measured(gate, tmp_path):
    """One variable: the same binding, the same campaign, the same two zeros —
    and a precondition counted over the stage documents the author wrote."""
    row = dict(
        PRESENT_CLASS_ROW,
        id="derived",
        binding={"kind": "artifact", "file": "combat-plan.json", "path": "fights.total"},
        applies_when=SOURCE_SIDE_PRECONDITION,
    )
    r = run(gate, tmp_path, row, objectives=[{"type": "narrate"}])
    assert r["verdict"] == "INAPPLICABLE"
    assert (r["binding"], r["precondition"]) == (0, 0)


def test_the_same_demand_holds_on_the_emitted_zero_path(gate, tmp_path):
    """The other adjudication site: an `out` glob that ran and matched nothing,
    with a derived precondition. Same rule, driven separately because it is a
    different branch."""
    row = dict(
        PRESENT_CLASS_ROW,
        id="glob",
        binding={"kind": "out", "glob": "**/declared_difficulty.mcfunction"},
        applies_when=BUILD_SIDE_PRECONDITION,
    )
    r = run(gate, tmp_path, row, objectives=[{"type": "narrate"}])
    assert r["verdict"] == "UNBOUND"
    assert "counted in the BUILD tree" in r["detail"]

    sourced = dict(row, applies_when=SOURCE_SIDE_PRECONDITION)
    assert run(gate, tmp_path, sourced, objectives=[{"type": "narrate"}])["verdict"] == (
        "INAPPLICABLE"
    )


def test_a_blockout_buys_no_forgiveness_for_a_build_side_double_zero(gate, tmp_path):
    """The stage cannot answer a question nobody asked of the design, so the
    demand holds on a pre-detail blockout too."""
    row = dict(
        PRESENT_CLASS_ROW,
        id="derived",
        binding={"kind": "out", "glob": "**/declared_difficulty.mcfunction"},
        applies_when=BUILD_SIDE_PRECONDITION,
    )
    camp = make_blockout_campaign(tmp_path, objectives=[{"type": "narrate"}])
    build = make_blockout_build(tmp_path)
    assert adjudicate_on(gate, camp, build, row)["verdict"] == "UNBOUND"


def test_every_live_ledger_row_anchors_its_absence_in_the_source(gate):
    """The live data against the rule, with the count COMPUTED from the ledger.
    A row that measured both sides in the build tree could never reach
    INAPPLICABLE — it would refuse forever — so this states, rather than
    assumes, that the guard above binds to nothing today."""
    doc = gate.load_ledger(gate.DEFAULT_LEDGER)
    rows = [r for r in doc["findings"] if r.get("binding")]
    assert rows, "the ledger declares no binding probes at all"
    build_side_only = [
        r["id"]
        for r in rows
        if not gate._absence_is_declared(r["binding"], r.get("applies_when"))
    ]
    assert build_side_only == [], build_side_only
