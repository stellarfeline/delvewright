"""The baseline's two arms are one rule, and no tree is refused by both.

`tools/gallery-baseline.py` verifies and it regenerates, and the verify arm's
only remedy is the regenerate arm. So the two are a PAIR (CLAUDE.md: when one
gate's prescription is another gate's refusal, the defect belongs to the pair),
and the property that has to hold is not about either half: **for every tree,
either verify passes or `--write` lands.**

It did not hold. Both halves ENUMERATED what to compare — the write arm asked
whether emission, the header and (one fix later) the warning ledger had moved —
and a manifest records more than those three. Every recorded manifest value is a
SIBLING of `outputs`, so a change moving only one of them — a campaign that
gains a skinned NPC moves `resource_pack_sha1` — moved something the baseline
records while all three qualifiers held: verify refused the tree and said
regenerate, `--write` refused the regeneration as a noise commit, and the verify
message named the one thing it was not, a determinism finding, over a list of
zero differing paths.

These tests bind the repaired shape rather than the instance, because the
instance is the second of its kind in this file and the first fix was a fourth
qualifier. `baseline_matches` is the single question; the tests below drive it
and `report_mismatch` directly, so a future manifest field or a fourth recorded
document cannot re-open the hole without reddening one of them.

The same defect then turned up one level up, and the later tests bind that too.
The shared question compared the documents the baseline RECORDS — which is
itself an enumeration, and it excluded `delta.json`. So a tree whose review
artifact was stale or hand-edited was refused by nothing; and the moment
anything did refuse it, `--write` would have called the repair a noise commit,
leaving the pair with no green state again. The question now weighs every
document a write PRODUCES, derived rather than listed.
"""

from __future__ import annotations

import copy
import importlib.util
import pathlib
import sys

import pytest

REPO = pathlib.Path(__file__).resolve().parents[2]


def _load():
    path = REPO / "tools" / "gallery-baseline.py"
    spec = importlib.util.spec_from_file_location("gallery_baseline_pair", path)
    mod = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    sys.modules["gallery_baseline_pair"] = mod
    spec.loader.exec_module(mod)
    return mod


GB = _load()


def _measured() -> dict:
    """One committed-baseline-shaped triple, with the manifest fields that exist."""
    return {
        "header": {
            "coverage": {"units_total": 3, "units_bound": 3},
            "delvec_version": "1.1.0",
            "dsl_version": "0.12.0",
            "gallery_source_sha256": "a" * 64,
            "generator_input_sha256": "b" * 64,
        },
        "manifests": {
            "primary.en": {
                "campaign_id": "gallery",
                "delvec_version": "1.1.0",
                "dsl_version": "0.12.0",
                "inputs": {"world.json": "c" * 64},
                "mc_version": "1.21.11",
                "outputs": {"datapack/data/gallery/function/tick.mcfunction": "d" * 64},
                "resource_pack_sha1": "e" * 40,
            }
        },
        "warnings": {"primary.en": [{"code": "DW0188", "stage": "l10n", "pointer": "x"}]},
    }


# Each perturbation is a real thing a change can do, and every one of them must
# be BOTH a verify red and a permitted `--write`. The recorded-value move is the
# one the enumerated guard could not see; the other four are its siblings, listed
# so the property is tested as a property and not as one bug.
PERTURBATIONS = {
    "a recorded manifest value (in no header, no warning row, no output)":
        lambda t: t["manifests"]["primary.en"].__setitem__("resource_pack_sha1", "0" * 40),
    "an input index entry (identical emission, different source)":
        lambda t: t["manifests"]["primary.en"]["inputs"].__setitem__("world.json", "9" * 64),
    "an emitted path (a real emission drift)":
        lambda t: t["manifests"]["primary.en"]["outputs"].__setitem__(
            "datapack/data/gallery/function/tick.mcfunction", "9" * 64
        ),
    "the header":
        lambda t: t["header"].__setitem__("gallery_source_sha256", "9" * 64),
    "the warning ledger":
        lambda t: t["warnings"].__setitem__("primary.en", []),
}


@pytest.mark.parametrize("what", sorted(PERTURBATIONS))
def test_no_tree_is_refused_by_both_arms(what):
    """The pair's whole property, and the one that was false.

    `--write` refuses exactly when `baseline_matches` holds, and verify passes
    exactly when it holds — so a tree verify reds on is a tree `--write` lands
    on, whatever the mismatch is in.
    """
    committed = _measured()
    measured = copy.deepcopy(committed)
    PERTURBATIONS[what](measured)
    assert measured != committed, "the perturbation did nothing; the test would be vacuous"

    matches = GB.baseline_matches(committed, measured)
    assert matches is False, "verify must red on this tree"
    # `--write` refuses iff `baseline_matches`; false here, so it lands.
    assert not matches, "and `--write` must therefore be permitted on the same tree"

    with pytest.raises(SystemExit) as e:
        GB.report_mismatch(committed, measured)
    assert e.value.code == 1


def test_the_enumerating_guard_is_the_defect_and_cannot_come_back():
    """The shape that made the pair unsatisfiable, written down as an assertion.

    The old write arm asked three questions — has emission moved, has the header
    moved, has the warning ledger moved — and a re-pin answers **no to all
    three** while the baseline plainly records something different. Any guard
    built by enumerating what to compare has this hole for whatever it did not
    enumerate, which is why the first fix (adding the warning ledger, a fourth
    qualifier) did not end it.

    The assertions below are the two halves of that fact side by side: the old
    predicate says nothing moved, and the repaired one says the baseline does not
    record this tree. If anyone narrows `baseline_matches` back toward the
    enumeration, the second assertion goes red here.
    """
    committed = _measured()
    measured = copy.deepcopy(committed)
    measured["manifests"]["primary.en"]["resource_pack_sha1"] = "0" * 40

    delta = GB.compute_delta(committed["manifests"], measured["manifests"])
    emission_moved = bool(delta["added"] + delta["removed"] + delta["changed"])
    header_moved = committed["header"] != measured["header"]
    warnings_moved = committed["warnings"] != measured["warnings"]
    assert not (emission_moved or header_moved or warnings_moved), (
        "the enumerated guard sees nothing here — that is the defect, not a pass"
    )
    assert GB.baseline_matches(committed, measured) is False


def test_a_matching_tree_passes_verify_and_refuses_a_rewrite():
    """The other direction, which is the noise-commit guard and must stay hard."""
    committed = _measured()
    assert GB.baseline_matches(committed, copy.deepcopy(committed)) is True


def test_a_missing_baseline_is_not_a_match():
    """Nothing recorded anything, so verify reds and `--write` lands."""
    assert GB.baseline_matches(None, _measured()) is False


def test_a_moved_recorded_value_is_not_reported_as_a_determinism_finding():
    """The message, not only the exit code.

    A determinism finding says *emission moved for no reason*; this says *a
    declared input moved*. Conflating them prints the gravest verdict this file
    has over a list of zero differing paths, and the reader has no way to tell
    what happened.
    """
    committed = _measured()
    measured = copy.deepcopy(committed)
    measured["manifests"]["primary.en"]["resource_pack_sha1"] = "0" * 40

    with pytest.raises(SystemExit):
        GB.report_mismatch(committed, measured)


def test_that_message_names_the_field_that_moved_and_never_determinism(capsys):
    committed = _measured()
    measured = copy.deepcopy(committed)
    measured["manifests"]["primary.en"]["resource_pack_sha1"] = "0" * 40
    with pytest.raises(SystemExit):
        GB.report_mismatch(committed, measured)
    err = capsys.readouterr().err
    assert "DECLARED INPUT MOVED" in err
    assert "DETERMINISM FINDING" not in err
    assert "EMISSION CHANGE" not in err
    assert "resource_pack_sha1" in err, "a finding nobody can act on is half a finding"


def test_the_emitted_path_delta_is_blind_to_the_siblings_by_design():
    """Why two deltas exist, asserted rather than remembered.

    `compute_delta` walks `outputs`. That is right — it is about emitted bytes —
    and it is exactly why an empty output delta beside a real mismatch must never
    be read as *nothing moved*.
    """
    committed = _measured()["manifests"]
    measured = copy.deepcopy(committed)
    measured["primary.en"]["resource_pack_sha1"] = "0" * 40

    delta = GB.compute_delta(committed, measured)
    assert not (delta["added"] + delta["removed"] + delta["changed"])

    fields = GB.manifest_field_delta(committed, measured)
    assert len(fields) == 1 and "resource_pack_sha1" in fields[0]


def test_the_field_delta_descends_inputs_and_names_the_file():
    committed = _measured()["manifests"]
    measured = copy.deepcopy(committed)
    measured["primary.en"]["inputs"]["world.json"] = "9" * 64
    fields = GB.manifest_field_delta(committed, measured)
    assert len(fields) == 1 and "inputs[world.json]" in fields[0]


def test_the_field_delta_names_a_build_that_appeared_or_vanished():
    committed = _measured()["manifests"]
    measured = copy.deepcopy(committed)
    measured["overlay.en"] = copy.deepcopy(measured["primary.en"])
    appeared = GB.manifest_field_delta(committed, measured)
    assert appeared == ["  + build `overlay.en`: built by this run, not in the baseline"]
    vanished = GB.manifest_field_delta(measured, committed)
    assert vanished == ["  - build `overlay.en`: in the baseline, not built by this run"]


def test_the_binding_count_counts_every_recorded_value_at_the_leaf():
    """Zero is a red elsewhere; here the number just has to be the truth."""
    manifests = _measured()["manifests"]
    # 5 scalars + 1 `inputs` entry, and `outputs` is not one of them.
    assert GB.field_count(manifests) == 6


def test_versions_toml_is_not_an_emission_input_and_a_lookalike_is_not_under():
    """The classifier decides what the reader is TOLD, never whether it refuses.

    The compiler reads nothing from `versions.toml`: a build is a function of the
    campaign directory, the prefab directory and the flags beside them. So a
    gallery mismatch beside a re-pin is exactly as unexplained as one beside no
    change at all, and calling it an emission change would excuse it. Membership
    here is decided by "can this reach a byte the compiler emits or records",
    which is also why a file that merely starts with a member's text is a
    different file.
    """
    assert "versions.toml" not in GB.EMISSION_INPUTS
    assert GB.under("crates/compiler/src/emit.rs", "crates/")
    assert not GB.under("crates.bak/x.rs", "crates/")
    assert GB.under("gallery/world.json", "gallery/")
    assert not GB.under("gallery-prefabs/x.nbt", "gallery/")


def test_delta_json_is_not_one_of_the_recorded_documents():
    """It is not a record of the tree — it is a record of the tree RELATIVE TO A COMMIT.

    So it cannot be compared against a measurement of the tree alone, which is
    why `RECORDED` — the set a mismatch is CLASSIFIED against — excludes it, and
    why nothing about that changes when the pair's shared question widens below.
    """
    assert "delta.json" not in GB.RECORDED
    assert set(GB.RECORDED) == {"header.json", "manifests.json", "warnings.json"}


def test_the_shared_question_weighs_every_produced_document_including_the_delta():
    """`PRODUCED`, not `RECORDED`, is what both arms compare — and it is DERIVED.

    Comparing `RECORDED` at the guard was the same enumeration defect one level
    up from the one this file's first repair retired: it left `delta.json`
    guarded by nothing at all, and made a delta-only defect unrepairable once
    anything did guard it (see the test below). A fifth produced document joins
    both arms by being produced, with nothing to remember.
    """
    assert set(GB.PRODUCED) == set(GB.RECORDED) | set(GB.DERIVED)
    assert "delta.json" in GB.PRODUCED
    assert GB.PRODUCED == GB.RECORDED + GB.DERIVED


def _produced(base: str = "b" * 40) -> dict:
    t = _measured()
    return {
        "header.json": t["header"],
        "manifests.json": t["manifests"],
        "warnings.json": t["warnings"],
        "delta.json": GB.review_delta(base, {}, t["manifests"]),
    }


def test_a_delta_only_defect_is_a_verify_red_and_a_permitted_write():
    """The state the pair had no answer for, asserted as the pair property.

    Every recorded document matches, so the old guard saw nothing move and
    `--write` would have refused the repair as a noise commit — while a check
    that recomputes the delta reds. No green state existed for a tree whose
    review artifact was stale or hand-edited, which is exactly the shape
    CLAUDE.md names when one gate's prescription is another gate's refusal.
    """
    on_disk = _produced()
    produced = copy.deepcopy(on_disk)
    on_disk["delta.json"]["changed"] = ["primary.en:datapack/data/gallery/function/tick.mcfunction"]

    # The recorded documents are untouched, so the old question says "nothing moved".
    assert GB.baseline_matches(
        GB.recorded_triple(on_disk), GB.recorded_triple(produced)
    ) is True
    # The shared question sees it, so verify reds and `--write` lands.
    assert GB.baseline_matches(on_disk, produced) is False

    with pytest.raises(SystemExit) as e:
        GB.report_delta_mismatch(on_disk["delta.json"], produced["delta.json"])
    assert e.value.code == 1


def test_the_delta_mismatch_verdict_names_the_base_and_the_path(capsys):
    on_disk = _produced()
    produced = copy.deepcopy(on_disk)
    on_disk["delta.json"]["changed"] = ["primary.en:gone.mcfunction"]
    with pytest.raises(SystemExit):
        GB.report_delta_mismatch(on_disk["delta.json"], produced["delta.json"])
    err = capsys.readouterr().err
    assert "REVIEW DELTA" in err
    assert "b" * 40 in err, "a finding that hides which two states it compared is half a finding"
    assert "gone.mcfunction" in err
    assert "DETERMINISM FINDING" not in err and "EMISSION CHANGE" not in err


def test_a_repeated_write_against_the_same_base_is_idempotent():
    """Why including the delta in the guard cannot make an ordinary rerun look dirty.

    The basis used to be whatever was on disk, so a second `--write` re-based the
    artifact onto its own first output: running it twice before committing
    silently changed what the file claimed, and a write that moved only the
    header rewrote it to empty and destroyed the record of the last real emission
    change. Measured from a commit, the same tree produces the same document.
    """
    manifests = _measured()["manifests"]
    once = GB.review_delta("a" * 40, {}, manifests)
    twice = GB.review_delta("a" * 40, {}, manifests)
    assert once == twice
    assert once["base_commit"] == "a" * 40


def test_the_delta_denominator_is_the_union_and_not_the_delta_itself():
    """A zero binding must mean *nothing was weighed*, never *nothing moved*.

    An honest delta is empty whenever a change moves a recorded input without
    moving an emitted byte — the commonest legitimate baseline update, and the
    one the live tree is in. Using the delta's own length as the binding count
    would call that vacuous and red every such change.
    """
    manifests = _measured()["manifests"]
    unchanged = GB.review_delta("a" * 40, manifests, manifests)
    assert not (unchanged["added"] + unchanged["removed"] + unchanged["changed"])
    assert GB.delta_binding(manifests, manifests) == 1
    assert GB.delta_binding({}, {}) == 0


def test_a_delta_that_does_not_name_its_base_is_unfalsifiable_and_says_so():
    """The migration case, and the reason the field is asserted rather than defaulted.

    A file with no base names one state and leaves the other nowhere, so nothing
    can recompute it — which is what `gallery/baseline/delta.json` was for as long
    as it existed. Guessing a base would make the check green over a claim it had
    invented, which is worse than the state it replaces.
    """
    assert GB.base_of({"added": [], "removed": [], "changed": []}) is None
    assert GB.base_of({"base_commit": "HEAD~1"}) is None
    assert GB.base_of({"base_commit": "A" * 40}) is None, "a sha is lowercase hex or it is not one"
    assert GB.base_of(None) is None
    assert GB.base_of({"base_commit": "c" * 40}) == "c" * 40
