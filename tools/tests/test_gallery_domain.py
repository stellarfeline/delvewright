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
        "content_sha": "aaa",
        "inputs": {"world.json": "1", "quests.json": "2"},
        "outputs": {"data/x.mcfunction": "h1", "data/gone.json": "h2"},
    }
    got = {
        "content_sha": "bbb",
        "inputs": {"world.json": "9", "quests.json": "2"},
        "outputs": {"data/x.mcfunction": "h1", "data/new.json": "h3"},
    }
    lines = gallery_build.manifest_delta(base, got)
    joined = "\n".join(lines)
    assert "inputs: world.json — same path, different content" in joined
    assert "outputs: data/gone.json — in the baseline, absent here" in joined
    assert "outputs: data/new.json — emitted here, absent from the baseline" in joined
    assert "content_sha: baseline `aaa` vs this build `bbb`" in joined
    assert "quests.json" not in joined, "an unchanged entry was reported as differing"
    assert "data/x.mcfunction" not in joined, "an unchanged output was reported as differing"


def test_the_delta_is_empty_when_the_manifests_agree():
    """The direction that must never fire, or every green run prints a refusal."""
    m = {"outputs": {"a": "1"}, "inputs": {"b": "2"}, "dsl_version": "0.14.0"}
    assert gallery_build.manifest_delta(m, json.loads(json.dumps(m))) == []
