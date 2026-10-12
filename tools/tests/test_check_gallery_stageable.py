"""The staging gate runs on every push, over every point the ladder builds.

Three failure modes are pinned here, and they are different in kind.

**UNRUN.** A gate nothing invokes is a documented command. That is the shape this
step exists to remove from `tools/creator/staging-gate.py`, so a step that stopped
invoking `check-gallery-stageable.py` would put it straight back.

**Unbound.** The domain is enumerated by `gallery_domain.points()` from the
gallery's own languages and overlay directories rather than typed, because a
typed list goes stale silently the first time an overlay is added and the job
then passes while judging less than the ladder builds. The count is asserted
against the denominator counted from the objects.

**A servable gallery.** A pass mints an admission token, and a token inside a
build tree is exactly what the compose staging path looks for. spec-0039 §2 says
the gallery is never staged; these assert that judging it cannot make it so.
"""

from __future__ import annotations

import importlib.util
import json
import pathlib

TOOLS = pathlib.Path(__file__).resolve().parents[1]
REPO = TOOLS.parent
GALLERY = REPO / "gallery"


def _load(name: str, filename: str):
    spec = importlib.util.spec_from_file_location(name, TOOLS / filename)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


cgs = _load("check_gallery_stageable", "ci/check-gallery-stageable.py")
staging_gate = _load("staging_gate", "creator/staging-gate.py")


# ------------------------------------------------------------------------ UNRUN


def test_ci_runs_the_check():
    """A step nothing invokes leaves the staging gate exactly as bound as before."""
    ci = (REPO / ".github" / "workflows" / "ci.yml").read_text()
    invocations = [
        line
        for line in ci.splitlines()
        if "tools/ci/check-gallery-stageable.py" in line and not line.strip().startswith("#")
    ]
    assert invocations, (
        "no CI step runs `tools/ci/check-gallery-stageable.py`. The staging gate is then bound to "
        "the staging event alone again, which is where it started"
    )


def test_the_step_lives_in_a_required_job():
    """A gate in a job nobody requires is advisory, and an advisory gate is unbound."""
    required = (REPO / ".github" / "required-status-checks.txt").read_text()
    ci = (REPO / ".github" / "workflows" / "ci.yml").read_text()
    assert "gallery baseline (emission, warnings, served points)" in required, (
        "the gallery baseline job is no longer a required status check, so the step this file guards "
        "reds nothing"
    )
    assert "name: gallery baseline (emission, warnings, served points)" in ci, (
        "the gallery baseline job's name has moved; the required context above now reports on nothing"
    )


# ---------------------------------------------------------------------- unbound


def test_the_domain_is_enumerated_from_the_objects():
    """Counted from the directory and `world.json` by a second method, not by `points()`."""
    rows = cgs.domain_rows()
    overlays = sorted(p.name for p in (GALLERY / "overlays").iterdir() if p.is_dir())
    langs = json.loads((GALLERY / "world.json").read_text())["content"].get("languages") or []
    assert overlays, "the gallery declares no overlay — the enumeration binds to nothing"
    expected = {("primary", "en")} | {("primary", lang) for lang in langs} | {(o, "en") for o in overlays}
    assert set(rows) == expected
    assert len(rows) == len(expected), "a point was enumerated twice"
    den = cgs.denominator()
    assert den["overlays"] == len(overlays)
    assert den["languages"] == len({"en", *langs})
    assert den["probes"] == len([p for p in (GALLERY / "probes").iterdir() if p.is_dir()])
    assert den["probes"] > 0, "no probe is counted — the exclusion the count states binds nothing"


def test_a_probe_is_never_a_point():
    probes = set(cgs.gallery_domain.probes())
    assert probes and not probes & {point for point, _ in cgs.domain_rows()}


def test_the_points_follow_the_directory(tmp_path):
    """Perturbed toward the vacuous shape: an overlay added is a point added."""
    gd = cgs.gallery_domain
    assert gd.points(["zh-cn"], ["easy"]) == [(None, "en"), (None, "zh-cn"), ("easy", "en")]
    assert gd.points(["zh-cn"], ["easy", "new"])[-1] == ("new", "en")
    assert gd.points(["en"], []) == [(None, "en")], "the source language is one point, not two"


def test_the_gate_hands_the_runs_manifests_to_every_build():
    """Each point is proved against the manifests this run's baseline measured."""
    src = (TOOLS / "ci" / "check-gallery-stageable.py").read_text()
    assert '"--manifests",\n            str(manifests),' in src
    ci = (REPO / ".github" / "workflows" / "ci.yml").read_text()
    step = ci[ci.index("python3 tools/ci/check-gallery-stageable.py"):]
    assert "--manifests .gallery-baseline/report/manifests.json" in step.split("\n\n", 1)[0]


def test_which_verdicts_refuse_is_the_gates_own_answer():
    """One authority. A verdict added to the gate cannot be counted as a pass here."""
    assert cgs.RED_VERDICTS == staging_gate.RED_VERDICTS
    assert "UNBOUND" in cgs.RED_VERDICTS and "NO-GENERAL-FORM" in cgs.RED_VERDICTS


def test_the_override_is_never_reachable_from_here():
    """`--stage-anyway` is the one flag that must never become how a gate is run."""
    text = (TOOLS / "ci" / "check-gallery-stageable.py").read_text()
    for flag in ("--stage-anyway", "--acknowledge-red"):
        assert flag not in text, (
            f"this step can pass `{flag}` to the staging gate. An override reachable from a "
            "CI step is an override that becomes the way the tool is run"
        )


# ----------------------------------------------------- the gallery stays unstaged


def test_the_gate_is_asked_coverage_only_and_mints_no_token():
    """A gallery point is judged, never admitted: the gate runs `--coverage-only`,
    which writes no token anywhere, so a green point is not a servable one
    (spec-0039 §2)."""
    text = (TOOLS / "ci" / "check-gallery-stageable.py").read_text()
    assert '"--coverage-only"' in text
    assert '"--admit"' not in text


def test_a_token_in_a_build_tree_is_seen(tmp_path):
    out = tmp_path / "delve-output-gallery-primary-en"
    out.mkdir()
    assert cgs.token_in_tree(out) is False
    (out / cgs.ADMISSION).write_text("{}\n")
    assert cgs.token_in_tree(out) is True
