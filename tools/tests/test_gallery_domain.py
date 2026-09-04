"""One authority on what a gallery build point is, and one act that rebuilds it.

Two failures are pinned here, and they are different in kind.

**The drift.** Materialising a domain point existed twice: the version in
`gallery-baseline.py` strips the three non-campaign directories because it is
about to compile the result, and the version in `check-gallery-coverage.py` did
not because it only had to survive a schema walk. Both were correct for their own
caller and they were not the same function, so the two tools judged two different
trees — and a round that needed to BUILD a point read the validate-only copy
first and had to recover the difference from the repository by hand. Nothing was
red and nothing could have been.

**The UNRUN shape.** The materialise-and-build step is only a rebuild guarantee
while something runs it. `test_ci_runs_the_rebuild` is what stops it becoming a
documented command nobody invokes, which is the mode this project has shipped
five times.
"""

from __future__ import annotations

import importlib.util
import json
import pathlib
import re

import pytest

TOOLS = pathlib.Path(__file__).resolve().parents[1]
REPO = TOOLS.parent
GALLERY = REPO / "gallery"

spec = importlib.util.spec_from_file_location("gallery_domain", TOOLS / "gallery_domain.py")
gallery_domain = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gallery_domain)


# ------------------------------------------------------------------ one authority


def test_only_one_tool_defines_materialise():
    """A third copy is the defect this module exists to make unnecessary.

    Stated with its DENOMINATOR: an exclusion that quietly shrinks the population
    keeps every count truthful while the gate stops covering the tree.
    """
    scripts = sorted(p for p in TOOLS.glob("*.py"))
    assert scripts, "no python tools found — this gate examined nothing"
    # Keyed to the OBJECT — a tool that copies the gallery tree — rather than to
    # the word `materialise`. `gallery-baseline.py` and `check-gallery-coverage.py`
    # each keep a one-line wrapper under the familiar name, which is a call site
    # and not a second answer; what may never come back is a second BODY. Scoping
    # it to the gallery is also what stops this reddening an unrelated future tool
    # that copies a tree for its own reasons.
    copiers = [
        p.name
        for p in scripts
        if "copytree" in (t := p.read_text()) and re.search(r"\bGALLERY\b|gallery/", t)
    ]
    assert copiers == ["gallery_domain.py"], (
        f"{len(scripts)} python tool(s) examined; these copy the gallery tree themselves: "
        f"{copiers}. What a build point IS is decided in `gallery_domain.py` and nowhere "
        "else — two answers is how one round took the validate-only one and had to work "
        "the difference out of the repository by hand."
    )
    assert re.search(r"^def materialise\(", (TOOLS / "gallery_domain.py").read_text(), re.M), (
        "the one authority no longer defines `materialise`"
    )


@pytest.mark.parametrize("caller", ["gallery-baseline.py", "check-gallery-coverage.py"])
def test_both_callers_reach_the_authority(caller):
    text = (TOOLS / caller).read_text()
    assert "gallery_domain" in text, (
        f"`{caller}` no longer imports `gallery_domain`, so it has an opinion of its own "
        "about what a build point is"
    )


# ------------------------------------------------------- what a point actually is


def _write_point(tmp_path: pathlib.Path) -> pathlib.Path:
    p = tmp_path / "an-overlay"
    p.mkdir(exist_ok=True)
    (p / "world.json").write_text('{"stage": "world"}')
    (p / "overlay.json").write_text('{"binds": []}')
    return p


def test_a_point_is_a_campaign_and_not_the_gallery(tmp_path):
    dest = tmp_path / "camp"
    n = gallery_domain.materialise(dest, _write_point(tmp_path))
    assert n > 0, "materialising wrote zero files"
    for junk in gallery_domain.NOT_CAMPAIGN:
        assert not (dest / junk).exists(), (
            f"`{junk}/` reached the campaign directory. It is domain source, not a stage "
            "document, and a walk of the campaign would find a whole second gallery inside it"
        )
    for m in gallery_domain.POINT_MANIFESTS:
        assert not (dest / m).exists(), f"`{m}` is tooling metadata and is never a stage document"
    assert json.loads((dest / "world.json").read_text())["stage"] == "world", (
        "the point's own files did not land on top of the primary"
    )


def test_materialise_replaces_rather_than_merges(tmp_path):
    """A file from a previous point is a campaign nobody authored.

    Both original copies passed `dirs_exist_ok=True` onto whatever was already
    there, which is fine for a fresh temp dir and silently wrong for the
    persistent tree `gallery-build.py` writes on every run.
    """
    dest = tmp_path / "camp"
    gallery_domain.materialise(dest, _write_point(tmp_path))
    (dest / "left-over.json").write_text("{}")
    gallery_domain.materialise(dest, _write_point(tmp_path))
    assert not (dest / "left-over.json").exists(), (
        "a file from the previous materialisation survived into the next campaign"
    )


def test_materialise_refuses_to_delete_the_gallery():
    """This function removes its destination, so the one wrong default is fatal."""
    for dangerous in (GALLERY, GALLERY / "overlays", REPO):
        with pytest.raises(SystemExit):
            gallery_domain.materialise(dangerous, None)
    assert (GALLERY / "world.json").is_file(), "the gallery survived — as it must"


def test_the_domain_enumeration_is_derived():
    names = gallery_domain.overlays()
    assert names, "the gallery declares no overlay — this gate bound to nothing"
    assert "site-plan" in names, (
        "the site-plan overlay is gone; it is the map pipeline's walkable whole and the "
        "point the CI rebuild step names"
    )
    assert gallery_domain.build_id(None, "en") == "primary.en"
    assert gallery_domain.build_id("site-plan", "en") == "site-plan.en"


# ------------------------------------------------------------------ not UNRUN


def test_ci_runs_the_rebuild():
    """A committed act nothing invokes is a documented command, not a guarantee."""
    ci = (REPO / ".github" / "workflows" / "ci.yml").read_text()
    invocations = [
        line for line in ci.splitlines() if "tools/gallery-build.py" in line and not line.strip().startswith("#")
    ]
    assert invocations, (
        "no CI step runs `tools/gallery-build.py`. The rebuild it promises is then exactly "
        "as good as whoever remembers to type it, which is the UNRUN shape CLAUDE.md names"
    )
    assert any("--point site-plan" in line for line in ci.splitlines()), (
        "CI rebuilds some point but not `site-plan` — the one the bot walked and the one "
        "whose absence from any committed act started this"
    )


def test_the_rebuild_is_measured_against_the_baseline():
    """Exit 0 is not reproduction. The manifest comparison is what makes it one."""
    text = (TOOLS / "gallery-build.py").read_text()
    assert "manifests.json" in text, (
        "`gallery-build.py` no longer compares its build to the committed baseline, so it "
        "asserts that a tree compiles and not that it is the tree the baseline measured"
    )
    assert (GALLERY / "baseline" / "manifests.json").is_file(), (
        "the baseline it compares against is gone"
    )


# --------------------------------------------------- the refusal carries evidence

gb_spec = importlib.util.spec_from_file_location("gallery_build", TOOLS / "gallery-build.py")
gallery_build = importlib.util.module_from_spec(gb_spec)
gb_spec.loader.exec_module(gallery_build)


def test_the_delta_reads_the_whole_manifest_not_just_outputs():
    """The first perturbation this was tested against moved `inputs` and nothing else.

    A stage document whose change emits identical bytes still moves the compiler's
    index over its inputs. An outputs-only delta printed a refusal with an empty
    evidence list under it — a red that sends the reader to rebuild the gallery to
    find out what it was about, which is the failure this project already paid for
    once in `gallery-baseline.py`'s warning ledger.
    """
    base = {
        "campaign_id": "aaa",
        "inputs": {"world.json": "1", "quests.json": "2"},
        "outputs": {"data/x.mcfunction": "h1", "data/gone.json": "h2"},
    }
    got = {
        "campaign_id": "bbb",
        "inputs": {"world.json": "9", "quests.json": "2"},
        "outputs": {"data/x.mcfunction": "h1", "data/new.json": "h3"},
    }
    lines = gallery_build.manifest_delta(base, got)
    joined = "\n".join(lines)
    assert "inputs: world.json — same path, different content" in joined
    assert "outputs: data/gone.json — in the baseline, absent here" in joined
    assert "outputs: data/new.json — emitted here, absent from the baseline" in joined
    assert "campaign_id: baseline `aaa` vs this build `bbb`" in joined
    assert "quests.json" not in joined, "an unchanged entry was reported as differing"
    assert "data/x.mcfunction" not in joined, "an unchanged output was reported as differing"


def test_the_delta_is_empty_when_the_manifests_agree():
    """The direction that must never fire, or every green run prints a refusal."""
    m = {"outputs": {"a": "1"}, "inputs": {"b": "2"}, "dsl_version": "0.14.0"}
    assert gallery_build.manifest_delta(m, json.loads(json.dumps(m))) == []


# ------------------------------------------- a probe is the primary plus an edit


def _write_probe(tmp_path: pathlib.Path, patch: list, extra: dict | None = None) -> pathlib.Path:
    p = tmp_path / "a-probe"
    p.mkdir(exist_ok=True)
    (p / "probe.json").write_text(
        json.dumps({"code": "DW0001", "units": [], "why": "because", "patch": patch})
    )
    for name, text in (extra or {}).items():
        (p / name).write_text(text)
    return p


def test_a_probe_patch_edits_the_primary_in_place(tmp_path):
    """The whole point: the probe names one edit and inherits everything else."""
    before = json.loads((GALLERY / "world.json").read_text())
    assert before["content"]["difficulty"] != "peaceful", (
        "this test perturbs `difficulty`; the primary already holding the perturbed value "
        "would make it green for the wrong reason"
    )
    dest = tmp_path / "camp"
    gallery_domain.materialise(
        dest,
        _write_probe(
            tmp_path,
            [{"doc": "world.json", "op": "replace", "path": "/content/difficulty", "value": "peaceful"}],
        ),
    )
    after = json.loads((dest / "world.json").read_text())
    assert after["content"]["difficulty"] == "peaceful", "the declared edit did not land"
    before["content"]["difficulty"] = "peaceful"
    assert after == before, (
        "the probe changed something it did not declare — everything but the edit must be "
        "the primary, which is the property that makes drift impossible rather than gated"
    )


def test_a_patch_that_no_longer_applies_is_refused_by_pointer(tmp_path):
    """Drift, as it actually arrives: the primary moves and the probe does not.

    This is the one the copy shape could not report. A stale COPY goes on
    validating — against a document that has quietly become someone else's — and
    the first thing anybody hears is a diagnostic the probe never named.
    """
    for pointer in ("/content/difficulty_level", "/content/areas/99/prefab", "/nope/deeper"):
        with pytest.raises(gallery_domain.PatchError) as e:
            gallery_domain.materialise(
                tmp_path / "camp",
                _write_probe(
                    tmp_path,
                    [{"doc": "world.json", "op": "replace", "path": pointer, "value": 1}],
                ),
            )
        assert pointer in str(e.value), f"the refusal must name the pointer: {e.value}"


def test_add_and_replace_are_different_verbs(tmp_path):
    """A forgiving `set` would absorb exactly the drift this mechanism exists to see."""
    with pytest.raises(gallery_domain.PatchError) as e:
        gallery_domain.materialise(
            tmp_path / "camp",
            _write_probe(
                tmp_path,
                [{"doc": "world.json", "op": "add", "path": "/content/difficulty", "value": "peaceful"}],
            ),
        )
    assert "already has that key" in str(e.value)

    with pytest.raises(gallery_domain.PatchError):
        gallery_domain.materialise(
            tmp_path / "camp",
            _write_probe(
                tmp_path,
                [{"doc": "world.json", "op": "replace", "path": "/content/no-such-key", "value": 1}],
            ),
        )


def test_a_probe_may_not_ship_a_copy_of_a_primary_document(tmp_path):
    """The copy shape, refused at the point where a point is made.

    Bound HERE rather than in the coverage gate because a probe is materialised
    by anything that wants one, and a rule that only the gate applies is a rule
    the next caller does not have.
    """
    with pytest.raises(gallery_domain.PatchError) as e:
        gallery_domain.materialise(
            tmp_path / "camp",
            _write_probe(tmp_path, [], {"world.json": (GALLERY / "world.json").read_text()}),
        )
    assert "world.json" in str(e.value) and "primary also holds" in str(e.value)


def test_a_document_the_primary_does_not_hold_is_left_alone(tmp_path):
    """The one shape that must still ship a file: there is nothing to drift from."""
    assert not (GALLERY / "site-plan.json").is_file(), (
        "the primary has gained a site plan, which is what this test assumes it cannot have "
        "(DW0839 refuses `areas[]` beside one)"
    )
    dest = tmp_path / "camp"
    gallery_domain.materialise(dest, _write_probe(tmp_path, [], {"site-plan.json": '{"stage": "site-plan"}'}))
    assert json.loads((dest / "site-plan.json").read_text())["stage"] == "site-plan"


def test_json_pointer_escapes_are_honoured(tmp_path):
    """`obj/press-the-case` is a KEY, not two levels — the gallery is full of them."""
    doc = {"a/b": {"c~d": 1}}
    gallery_domain.apply_patch(doc, [{"op": "replace", "path": "/a~1b/c~0d", "value": 2}])
    assert doc == {"a/b": {"c~d": 2}}


def test_patch_ops_are_read_from_the_manifest_a_reader_opens(tmp_path):
    """One object is both the explanation and the instruction, or they drift."""
    p = _write_probe(tmp_path, [{"doc": "world.json", "op": "remove", "path": "/content/difficulty"}])
    assert gallery_domain.patch_ops(p) == json.loads((p / "probe.json").read_text())["patch"]
    assert gallery_domain.patch_ops(None) == []
    assert gallery_domain.patch_ops(_write_point(tmp_path)) == [], "an overlay declares no edit"


def test_array_edits_are_positional_and_bounded(tmp_path):
    doc = {"xs": [1, 2, 3]}
    gallery_domain.apply_patch(doc, [{"op": "add", "path": "/xs/1", "value": 9}])
    assert doc["xs"] == [1, 9, 2, 3]
    gallery_domain.apply_patch(doc, [{"op": "add", "path": "/xs/-", "value": 7}])
    assert doc["xs"] == [1, 9, 2, 3, 7]
    gallery_domain.apply_patch(doc, [{"op": "remove", "path": "/xs/0"}])
    assert doc["xs"] == [9, 2, 3, 7]
    for bad in ("/xs/99", "/xs/x"):
        with pytest.raises(gallery_domain.PatchError):
            gallery_domain.apply_patch({"xs": [1]}, [{"op": "remove", "path": bad}])


def test_every_committed_probe_is_the_primary_plus_its_declared_edit():
    """The repository's own probes, materialised, with the count stated.

    A binding count computed from the objects rather than written beside them —
    a constant here would be green on a gate that binds to nothing.
    """
    probes = sorted(p for p in (GALLERY / "probes").iterdir() if p.is_dir())
    assert probes, "no probes examined — this assertion bound to nothing"
    edits = own = 0
    for pd in probes:
        assert gallery_domain.shadowed_documents(pd) == [], (
            f"probe `{pd.name}` ships a copy of a primary document"
        )
        ops = gallery_domain.patch_ops(pd)
        edits += len(ops)
        own += sum(
            1 for f in pd.rglob("*") if f.is_file() and f.name not in gallery_domain.POINT_MANIFESTS
        )
        for op in ops:
            assert (GALLERY / op["doc"]).is_file(), (
                f"probe `{pd.name}` edits `{op['doc']}`, which is not a primary document"
            )
    assert edits or own, (
        f"{len(probes)} probe(s) examined and not one perturbs anything — every probe would "
        "then be the primary, and the refusal half of the coverage gate binds to nothing"
    )
