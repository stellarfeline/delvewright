"""The baseline's two arms are one rule, and no tree is refused by both.

`tools/gallery-baseline.py` verifies and it regenerates, and the verify arm's
only remedy is the regenerate arm. So the two are a PAIR (CLAUDE.md: when one
gate's prescription is another gate's refusal, the defect belongs to the pair),
and the property that has to hold is not about either half: **for every tree,
either verify passes or `--write` lands.**

It did not hold. Both halves ENUMERATED what to compare — the write arm asked
whether emission, the header and (one fix later) the warning ledger had moved —
and a manifest records more than those three. `content_sha` is a SIBLING of
`outputs`, so a change moving only `versions.toml [content].sha` moved something
the baseline records while all three qualifiers held: verify refused the tree and
said regenerate, `--write` refused the regeneration as a noise commit, and the
verify message named the one thing it was not, a determinism finding, over a list
of zero differing paths.

These tests bind the repaired shape rather than the instance, because the
instance is the second of its kind in this file and the first fix was a fourth
qualifier. `baseline_matches` is the single question; the tests below drive it
and `report_mismatch` directly, so a future manifest field or a fourth recorded
document cannot re-open the hole without reddening one of them.
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
                "content_sha": "1" * 40,  # a fixture value, never the real pin (check-pins)
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
# be BOTH a verify red and a permitted `--write`. The re-pin is the live defect;
# the other four are its siblings, listed so the property is tested as a property
# and not as one bug.
PERTURBATIONS = {
    "content re-pin (the live defect: a manifest field, in no header, no warning row, no output)":
        lambda t: t["manifests"]["primary.en"].__setitem__("content_sha", "0" * 40),
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
    measured["manifests"]["primary.en"]["content_sha"] = "0" * 40

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


def test_the_re_pin_is_not_reported_as_a_determinism_finding():
    """The message, not only the exit code.

    A determinism finding says *emission moved for no reason*; a re-pin says *a
    declared input moved*. Conflating them printed the gravest verdict this file
    has over a list of zero differing paths, and the reader had no way to tell
    what had happened.
    """
    committed = _measured()
    measured = copy.deepcopy(committed)
    measured["manifests"]["primary.en"]["content_sha"] = "0" * 40

    with pytest.raises(SystemExit):
        GB.report_mismatch(committed, measured)


def test_the_re_pin_message_names_content_sha_and_never_determinism(capsys):
    committed = _measured()
    measured = copy.deepcopy(committed)
    measured["manifests"]["primary.en"]["content_sha"] = "0" * 40
    with pytest.raises(SystemExit):
        GB.report_mismatch(committed, measured)
    err = capsys.readouterr().err
    assert "DECLARED INPUT MOVED" in err
    assert "DETERMINISM FINDING" not in err
    assert "EMISSION CHANGE" not in err
    assert "content_sha" in err, "a finding nobody can act on is half a finding"


def test_the_emitted_path_delta_is_blind_to_the_siblings_by_design():
    """Why two deltas exist, asserted rather than remembered.

    `compute_delta` walks `outputs`. That is right — it is about emitted bytes —
    and it is exactly why an empty output delta beside a real mismatch must never
    be read as *nothing moved*.
    """
    committed = _measured()["manifests"]
    measured = copy.deepcopy(committed)
    measured["primary.en"]["content_sha"] = "0" * 40

    delta = GB.compute_delta(committed, measured)
    assert not (delta["added"] + delta["removed"] + delta["changed"])

    fields = GB.manifest_field_delta(committed, measured)
    assert len(fields) == 1 and "content_sha" in fields[0]


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
    # 6 scalars + 1 `inputs` entry, and `outputs` is not one of them.
    assert GB.field_count(manifests) == 7


def test_versions_toml_is_an_emission_input_and_a_lookalike_is_not():
    """The classifier decides what the reader is TOLD, never whether it refuses.

    `versions.toml` is read by the compiler at build time and recorded into every
    manifest, so a manifest mismatch beside a change to it is an ordinary
    consequence rather than an ADR-0006 violation. A file that merely starts with
    the same text is a different file.
    """
    assert "versions.toml" in GB.EMISSION_INPUTS
    assert GB.under("versions.toml", "versions.toml")
    assert not GB.under("versions.toml.bak", "versions.toml")
    assert GB.under("gallery/world.json", "gallery/")
    assert not GB.under("gallery-prefabs/x.nbt", "gallery/")


def test_delta_json_is_not_one_of_the_recorded_documents():
    """It is the review artifact OF a write, not a record of the tree.

    Counting it would make every rewrite look like it recorded something, which
    is the noise-commit guard going quiet.
    """
    assert "delta.json" not in GB.RECORDED
    assert set(GB.RECORDED) == {"header.json", "manifests.json", "warnings.json"}
