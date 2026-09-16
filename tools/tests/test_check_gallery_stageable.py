"""The staging gate runs on every push, over every point the ladder builds.

Three failure modes are pinned here, and they are different in kind.

**UNRUN.** A gate nothing invokes is a documented command. That is the shape this
step exists to remove from `tools/staging-gate.py`, so a step that stopped
invoking `check-gallery-stageable.py` would put it straight back.

**Unbound.** The domain is enumerated from `gallery/baseline/manifests.json`
rather than typed, because a typed list goes stale silently the first time an
overlay is added and the job then passes while judging less than the ladder
builds. The enumeration is asserted to cover the gallery directory, which is the
second observer that shares no configuration with the committed ledger.

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
BASELINE = REPO / "gallery" / "baseline" / "manifests.json"


def _load(name: str, filename: str):
    spec = importlib.util.spec_from_file_location(name, TOOLS / filename)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


cgs = _load("check_gallery_stageable", "check-gallery-stageable.py")
staging_gate = _load("staging_gate", "staging-gate.py")


# ------------------------------------------------------------------------ UNRUN


def test_ci_runs_the_check():
    """A step nothing invokes leaves the staging gate exactly as bound as before."""
    ci = (REPO / ".github" / "workflows" / "ci.yml").read_text()
    invocations = [
        line
        for line in ci.splitlines()
        if "tools/check-gallery-stageable.py" in line and not line.strip().startswith("#")
    ]
    assert invocations, (
        "no CI step runs `tools/check-gallery-stageable.py`. The staging gate is then bound to "
        "the staging event alone again, which is where it started"
    )


def test_the_step_lives_in_a_required_job():
    """A gate in a job nobody requires is advisory, and an advisory gate is unbound."""
    required = (REPO / ".github" / "required-status-checks.txt").read_text()
    ci = (REPO / ".github" / "workflows" / "ci.yml").read_text()
    assert "gallery (coverage + build + baseline)" in required, (
        "the gallery job is no longer a required status check, so the step this file guards "
        "reds nothing"
    )
    assert "name: gallery (coverage + build + baseline)" in ci, (
        "the gallery job's name has moved; the required context above now reports on nothing"
    )


# ---------------------------------------------------------------------- unbound


def test_the_domain_is_enumerated_from_the_build_ledger():
    rows = cgs.ledger_rows()
    recorded = json.loads(BASELINE.read_text())
    assert rows, "the build ledger enumerated zero points — this gate would judge nothing"
    assert len(rows) == len(recorded), (
        f"enumerated {len(rows)} of the ledger's {len(recorded)} row(s): the gate would judge "
        "fewer points than the ladder builds"
    )
    keys = {
        cgs.build_id(None if point == cgs.PRIMARY else point, lang) for point, lang in rows
    }
    assert keys == set(recorded), "the pairs do not re-encode to the ledger's own keys"


def test_the_ledger_covers_the_gallery_directory():
    """The cross-check, run against the real tree — the two must not have drifted."""
    rows = cgs.ledger_rows()
    declared = cgs.domain_points()
    assert len(declared) > 1, "the gallery declares no overlay — the cross-check binds to nothing"
    assert cgs.uncovered_points(rows, declared) == [], (
        "the build ledger does not record every point the gallery directory has"
    )


def test_a_point_the_ledger_lost_is_refused():
    """Perturbed toward the vacuous shape: the gate must see a shrinking domain."""
    rows = [("primary", "en"), ("easy", "en")]
    assert cgs.uncovered_points(rows, ["primary", "easy"]) == []
    assert cgs.uncovered_points(rows, ["primary", "easy", "site-plan"]) == ["site-plan"]


def test_which_verdicts_refuse_is_the_gates_own_answer():
    """One authority. A verdict added to the gate cannot be counted as a pass here."""
    assert cgs.RED_VERDICTS == staging_gate.RED_VERDICTS
    assert "UNBOUND" in cgs.RED_VERDICTS and "NO-GENERAL-FORM" in cgs.RED_VERDICTS


def test_the_override_is_never_reachable_from_here():
    """`--stage-anyway` is the one flag that must never become how a gate is run."""
    text = (TOOLS / "check-gallery-stageable.py").read_text()
    for flag in ("--stage-anyway", "--acknowledge-red"):
        assert flag not in text, (
            f"this step can pass `{flag}` to the staging gate. An override reachable from a "
            "CI step is an override that becomes the way the tool is run"
        )


# ----------------------------------------------------- the gallery stays unstaged


def test_the_token_is_written_outside_every_build_tree(tmp_path):
    work = tmp_path / "work"
    out = work / "delve-output-gallery-primary-en"
    admit = cgs.admission_path(work, "primary.en")
    assert not admit.is_relative_to(out), (
        "the admission token lands inside the build tree, which is where the compose staging "
        "path looks for one — a green gallery point would then be servable (spec-0039 §2)"
    )
    assert admit.is_relative_to(work)


def test_a_token_in_a_build_tree_is_seen(tmp_path):
    out = tmp_path / "delve-output-gallery-primary-en"
    out.mkdir()
    assert cgs.token_in_tree(out) is False
    (out / cgs.ADMISSION).write_text("{}\n")
    assert cgs.token_in_tree(out) is True
