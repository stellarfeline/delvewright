"""A re-based review delta is not an emission drift, and the write says which it is.

`tools/gallery-baseline.py --write` printed ONE sentence carrying three figures —
changed paths, warning rows, recorded manifest values — every one of them measured
between THIS TREE and THE REVIEW BASE, and none of them against what was on disk.
Nothing in the arm measured the write's own effect at all.

Measured on a green branch whose review base had advanced, that sentence read

    0 added, 0 removed, 3 changed path(s); 0 warning row(s) at a new count;
    3 recorded manifest value(s) moved.
      ~ primary.en.inputs[world.json]: baseline `d8cbe83a…` vs this tree `d516d1a9…`

over a write whose whole effect on disk was ONE JSON field: `delta.json`'s
`base_commit`. Its path lists were byte-identical before and after. The words
`moved`, `at a new count` and `baseline … vs this tree` are the vocabulary of a
drift, and the reader had nothing in the output to weigh them against.

That is the alarming mirror of a reassuring gloss, and it is the more expensive
one: a reassuring gloss invites no action, while a false drift gets acted on. A
round told only "run `--write`" commits the result as a regeneration, and the
commit carries a measurement of nothing.

The same divergence has a second, quiet direction and it is reproducible with no
construction at all: on a pristine checkout of `main` the artifact names the
merge base of the pull request that produced it, `merge-base(origin/main, HEAD)`
is `HEAD`, so `--write` does not refuse, writes an EMPTY delta, and reports
`0 added, 0 removed, 0 changed` while destroying the review record of the last
real emission change.

These tests drive `write_effect` and `write_report` rather than restating them.
The properties asserted are the two the report has to keep: what this invocation
changed is measured (in documents, before the write) and stated first, and the
difference between two commits never appears without both of its ends named.
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
    spec = importlib.util.spec_from_file_location("gallery_baseline_write_report", path)
    mod = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    sys.modules["gallery_baseline_write_report"] = mod
    spec.loader.exec_module(mod)
    return mod


GB = _load()

OLD_BASE = "a" * 40
NEW_BASE = "b" * 40
PATHS = ["primary.en:datapack/data/gallery/function/boundary_return.mcfunction"]


def _produced(base: str, changed: list[str]) -> dict:
    """The four documents a write produces, with a delta measured from `base`."""
    return {
        "header.json": {"delvec_version": "1.1.0"},
        "manifests.json": {"primary.en": {"outputs": {"x.mcfunction": "d" * 64}}},
        "warnings.json": {"primary.en": []},
        "delta.json": {
            "base_commit": base,
            "added": [],
            "removed": [],
            "changed": list(changed),
            "classes": {"datapack function": len(changed)} if changed else {},
        },
    }


def _delta_of(produced: dict) -> dict:
    return produced["delta.json"]


# --------------------------------------------------------------- the re-base --


def test_a_pure_rebase_is_measured_as_one_document_and_no_recorded_movement():
    """The live case: the tree did not move, and the effect says so in documents.

    `recorded_moved` empty is the whole finding — every document that RECORDS the
    tree already held what the run measured. Nothing about that is derivable from
    the path counts, which is why the arm printing only path counts could not say
    it.
    """
    on_disk = _produced(OLD_BASE, PATHS)
    produced = _produced(NEW_BASE, PATHS)

    effect = GB.write_effect(on_disk, produced, NEW_BASE)
    assert effect["changed"] == ["delta.json"]
    assert effect["recorded_moved"] == []
    assert effect["rebased"] is True
    assert effect["delta_lists_moved"] is False
    assert set(effect["unchanged"]) == set(GB.RECORDED)


def test_the_rebase_report_refuses_the_drift_reading_and_names_both_ends(capsys):
    on_disk = _produced(OLD_BASE, PATHS)
    produced = _produced(NEW_BASE, PATHS)
    effect = GB.write_effect(on_disk, produced, NEW_BASE)
    out = "\n".join(
        GB.write_report(effect, _delta_of(produced), [], ["  ~ primary.en.resource_pack_sha1: …"])
    )

    first, rest = out.split("\n", 1)
    assert "this invocation changed 1 of 4 document(s)" in first, (
        "the effect of the write is the first thing the reader meets, and it is "
        "counted in documents — a number that cannot be alarming"
    )
    assert "RE-BASED" in rest and "NOT AN EMISSION DRIFT" in rest
    assert OLD_BASE[:12] in rest and NEW_BASE[:12] in rest, (
        "a re-base that hides which two bases it moved between is half a finding"
    )
    # The drift-shaped figures survive, and they may: they are true of the
    # transition. What must not survive is their appearing with no second end.
    figures = [ln for ln in rest.splitlines() if "changed path(s)" in ln]
    assert len(figures) == 1
    heading = rest.splitlines()[rest.splitlines().index(figures[0]) - 1]
    assert "THIS TREE against" in heading and NEW_BASE[:12] in heading
    assert "not a list of what this invocation changed" in heading


def test_the_old_words_that_read_as_a_tree_movement_are_gone():
    """`moved` and `at a new count` are the tree's vocabulary; the base is not the tree.

    Keyed to the words rather than to the sentence, because the defect was the
    words: they are what a reader scanning the output takes the verdict from.
    """
    on_disk = _produced(OLD_BASE, PATHS)
    produced = _produced(NEW_BASE, PATHS)
    effect = GB.write_effect(on_disk, produced, NEW_BASE)
    out = "\n".join(GB.write_report(effect, _delta_of(produced), [], []))
    assert "at a new count" not in out
    assert "recorded manifest value(s) moved" not in out
    assert "recorded manifest value(s) differ from the base" in out


# ------------------------------------------------------- the discarded record --


def test_a_write_that_empties_the_delta_says_so_and_says_where_it_survives():
    """Reproducible on a pristine `main`, and it destroys a record while printing zeroes.

    The delta becomes empty because the new base already contains the transition
    the old one recorded. That is honest for the new base and a loss all the same,
    and `0 added, 0 removed, 0 changed` is the least alarming thing a reader could
    be shown at that moment.
    """
    on_disk = _produced(OLD_BASE, PATHS * 6)
    produced = _produced(NEW_BASE, [])
    effect = GB.write_effect(on_disk, produced, NEW_BASE)
    assert effect["discarded"] == 6
    out = "\n".join(GB.write_report(effect, _delta_of(produced), [], []))
    assert "DISCARDED A REVIEW RECORD" in out
    assert "6 path(s)" in out
    assert OLD_BASE[:12] in out, "the reader must be told where the record still is"


def test_an_ordinary_shrinking_delta_is_not_called_a_discard():
    """The claim is *emptied*, not *smaller* — a gate that fires on ordinary work gets ignored."""
    on_disk = _produced(OLD_BASE, PATHS * 6)
    produced = _produced(OLD_BASE, PATHS * 2)
    effect = GB.write_effect(on_disk, produced, OLD_BASE)
    assert effect["discarded"] == 0
    assert "DISCARDED" not in "\n".join(GB.write_report(effect, _delta_of(produced), [], []))


# ------------------------------------------------- the direction that must red --


def test_a_real_emission_change_still_reports_as_one():
    """The other direction, and the one a gate is usually blind to.

    A repair that only quietens the false alarm is not a repair. When the
    documents that RECORD the tree move, the report must say they moved and must
    NOT say the tree stood still.
    """
    on_disk = _produced(OLD_BASE, [])
    produced = _produced(OLD_BASE, PATHS)
    produced["manifests.json"]["primary.en"]["outputs"]["x.mcfunction"] = "9" * 64
    produced["warnings.json"]["primary.en"] = [{"code": "DW0188"}]

    effect = GB.write_effect(on_disk, produced, OLD_BASE)
    assert set(effect["recorded_moved"]) == {"manifests.json", "warnings.json"}
    assert effect["rebased"] is False

    out = "\n".join(GB.write_report(effect, _delta_of(produced), ["  + row"], ["  ~ field"]))
    assert "this invocation changed 3 of 4 document(s)" in out
    assert "manifests.json — rewritten" in out
    assert "warnings.json — rewritten" in out
    assert "NOT AN EMISSION DRIFT" not in out, (
        "the tree moved; saying it did not is the failure this repair must not buy"
    )
    assert "RE-BASED" not in out
    assert "1 changed path(s)" in out


def test_a_rebase_that_also_moves_emission_is_never_called_a_non_drift():
    """Both at once — the case a two-state verdict would get wrong.

    A sync merge brings `main`'s emission in AND advances the base. The base
    moved, so the delta is re-based; the tree moved too, so this is a drift and
    the reassuring sentence must not appear.
    """
    on_disk = _produced(OLD_BASE, [])
    produced = _produced(NEW_BASE, PATHS)
    produced["manifests.json"]["primary.en"]["outputs"]["x.mcfunction"] = "9" * 64

    effect = GB.write_effect(on_disk, produced, NEW_BASE)
    assert effect["rebased"] is True
    assert effect["recorded_moved"] == ["manifests.json"]
    out = "\n".join(GB.write_report(effect, _delta_of(produced), [], []))
    assert "RE-BASED" in out
    assert "NOT AN EMISSION DRIFT" not in out


def test_a_first_write_is_a_creation_and_claims_nothing_about_a_previous_base():
    on_disk = None
    produced = _produced(NEW_BASE, PATHS)
    effect = GB.write_effect(on_disk, produced, NEW_BASE)
    assert effect["created"] is True
    assert set(effect["changed"]) == set(GB.PRODUCED)
    out = "\n".join(GB.write_report(effect, _delta_of(produced), [], []))
    assert "this invocation changed 4 of 4 document(s)" in out
    assert "RE-BASED" not in out and "NOT AN EMISSION DRIFT" not in out
    assert "written against" in out


# --------------------------------------------------------- the pair, restated --


def test_the_implication_runs_one_way_and_the_pair_property_survives():
    """`--write` refuses ⇒ verify passes; the converse is what was false.

    Asserted as the pair's real shape rather than as the biconditional the module
    used to claim, and derived rather than remembered: a refusal means
    `on_disk == produced`, so the committed `delta.json` names `--write`'s own
    base, so verify resolves the SAME base and asks the identical question. The
    converse fails exactly when `--base` has advanced, and it fails toward an
    extra WRITABLE state — never toward a tree both arms refuse.
    """
    produced = _produced(NEW_BASE, PATHS)

    # `--write` refuses here, and the base verify would resolve is `--write`'s own.
    on_disk = copy.deepcopy(produced)
    assert GB.baseline_matches(on_disk, produced) is True
    assert GB.base_of(on_disk["delta.json"]) == NEW_BASE

    # Verify's produced document is built from the base the artifact names, so it
    # is the same document, so verify passes. No tree is refused by both arms.
    verify_produced = _produced(GB.base_of(on_disk["delta.json"]), PATHS)
    assert GB.baseline_matches(on_disk, verify_produced) is True

    # The converse: verify passes on an artifact naming OLD_BASE while `--write`
    # re-derives NEW_BASE, so `--write` lands. This is the reproduced divergence.
    on_disk_old = _produced(OLD_BASE, PATHS)
    assert GB.baseline_matches(on_disk_old, _produced(OLD_BASE, PATHS)) is True, "verify passes"
    assert GB.baseline_matches(on_disk_old, produced) is False, "`--write` lands anyway"


def test_the_divergence_is_confined_to_the_delta_s_base():
    """Why the extra state is benign, asserted rather than argued.

    The two arms differ in one input — the base — and that input reaches exactly
    one produced document. Were it to reach a RECORDED one, the arms would
    disagree about the tree itself and the pair property would be at risk.
    """
    a, b = _produced(OLD_BASE, PATHS), _produced(NEW_BASE, PATHS)
    differing = [n for n in GB.PRODUCED if a[n] != b[n]]
    assert differing == ["delta.json"]
    assert all(n in GB.RECORDED for n in GB.PRODUCED if n not in differing)


def test_the_field_delta_left_label_names_what_old_actually_is():
    """The write arm's `old` is a commit, not the committed baseline.

    Calling it "baseline" in the write arm asserted that the file on disk had
    moved — the same misreading one line further down the page.
    """
    old = {"primary.en": {"resource_pack_sha1": "1" * 40, "outputs": {}}}
    new = copy.deepcopy(old)
    new["primary.en"]["resource_pack_sha1"] = "0" * 40

    assert "baseline `" in GB.manifest_field_delta(old, new)[0]
    labelled = GB.manifest_field_delta(old, new, left="`abcdef123456`")[0]
    assert "baseline `" not in labelled
    assert "`abcdef123456` `" + "1" * 40 in labelled


@pytest.mark.parametrize(
    "name", ["write_effect", "write_report"]
)
def test_the_report_is_a_function_so_it_can_be_driven(name):
    """UNRUN can live in code: a report built inline in `main` is untestable by construction.

    The arm's message was a single `print` inside `main`, so nothing could assert
    what it said without building a gallery. That is why it went wrong invisibly.
    """
    assert callable(getattr(GB, name))
