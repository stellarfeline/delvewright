r"""Guard: one tag grammar, and release workflows that only a human starts (ADR-0028 §1, §4).

Two halves. The grammar itself (`tools/lib/release_tags.py`): strict semver, the
three names, the `v<semver>` shape refused. And the WORKFLOWS: each of the three
release workflows is read through the shared workflow parser and must start
ONLY by `workflow_dispatch` — merging and releasing are unrelated, so no push, no
tag and no merge may start one — and each derives its tag through the grammar
rather than receiving one.
"""

from __future__ import annotations

import pathlib
import subprocess
import sys

import pytest

REPO = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "tools" / "lib"))

import release_tags  # noqa: E402
from workflow_yaml import load  # noqa: E402

WORKFLOWS = REPO / ".github" / "workflows"
RELEASE_WORKFLOWS = {
    "delvec": WORKFLOWS / "engine-release.yml",
    "delvewright-dsl": WORKFLOWS / "dsl-crate-publish.yml",
    "delvewright": WORKFLOWS / "plugin-release.yml",
}


def _runs(path: pathlib.Path) -> str:
    doc = load(path.read_text(encoding="utf-8"))
    return "\n".join(
        step["run"]
        for job in doc["jobs"].values()
        for step in job.get("steps", [])
        if isinstance(step, dict) and isinstance(step.get("run"), str)
    )


# -------------------------------------------------------------- the grammar --
@pytest.mark.parametrize(
    "tag,name,version",
    [
        ("delvec--v1.6.0", "delvec", "1.6.0"),
        ("delvewright-dsl--v0.26.0", "delvewright-dsl", "0.26.0"),
        ("delvewright--v1.4.3", "delvewright", "1.4.3"),
        ("delvewright--v10.0.12", "delvewright", "10.0.12"),
    ],
)
def test_the_grammar_reads_each_line(tag, name, version):
    assert release_tags.parse(tag) == (name, version)
    assert release_tags.tag_for(name, version) == tag
    assert release_tags.title(tag) == f"{name} {version}"


@pytest.mark.parametrize(
    "tag",
    [
        "v1.6.0",  # the pre-grammar shape (§7)
        "delvec-v1.6.0",
        "delvec/v1.6.0",
        "delvec--1.6.0",
        "delvec--v01.6.0",  # leading zero
        "delvec--v1.6",
        "delvec--v1.6.0-rc.1",  # prerelease (revisit trigger)
        "delvec--v1.6.0+build",
        "delvewright-plugin--v1.4.3",
        "Delvec--v1.6.0",
    ],
)
def test_the_grammar_refuses_everything_else(tag):
    with pytest.raises(release_tags.Refused):
        release_tags.parse(tag)


def test_a_legacy_tag_is_named_as_such():
    with pytest.raises(release_tags.Refused, match="pre-grammar"):
        release_tags.parse("v1.6.0")


def test_identity_refuses_a_tag_naming_another_version_or_line():
    assert release_tags.identity("delvec", "delvec--v1.6.0", "1.6.0") == "delvec--v1.6.0"
    with pytest.raises(release_tags.Refused, match="must be 'delvec--v1.6.0'"):
        release_tags.identity("delvec", "delvec--v1.5.0", "1.6.0")
    with pytest.raises(release_tags.Refused):
        release_tags.identity("delvec", "delvewright--v1.6.0", "1.6.0")
    with pytest.raises(release_tags.Refused):
        release_tags.identity("delvec", "v1.6.0", "1.6.0")


def test_previous_stays_on_its_own_line():
    tags = [
        "v1.4.0", "v1.5.0", "archive/bell-engine-r1",
        "delvec--v1.6.0", "delvec--v1.10.0",
        "delvewright--v1.4.3", "delvewright--v1.4.5",
        "delvewright-dsl--v0.24.0", "delvewright-dsl--v0.26.0",
    ]
    assert release_tags.previous("delvec--v1.6.0", tags) == "v1.5.0"
    assert release_tags.previous("delvec--v1.11.0", tags) == "delvec--v1.10.0"
    assert release_tags.previous("delvec--v1.7.0", tags) == "delvec--v1.6.0"
    assert release_tags.previous("delvewright--v1.4.5", tags) == "delvewright--v1.4.3"
    assert release_tags.previous("delvewright--v1.4.3", tags) is None
    assert release_tags.previous("delvewright-dsl--v0.26.0", tags) == "delvewright-dsl--v0.24.0"
    assert release_tags.previous("delvewright-dsl--v0.24.0", tags) is None


def test_the_cli_refuses_a_bare_v_tag_with_exit_1():
    result = subprocess.run(
        ["python3", str(REPO / "tools" / "lib" / "release_tags.py"), "identity", "delvec", "v1.6.0", "1.6.0"],
        capture_output=True, text=True,
    )
    assert result.returncode == 1, result.stdout + result.stderr
    assert "REFUSED" in result.stderr


# ------------------------------------------------------------- the triggers --
@pytest.mark.parametrize("line,path", sorted(RELEASE_WORKFLOWS.items()))
def test_a_release_starts_only_by_dispatch(line, path):
    """No push, tag or merge starts a release: `on` is `workflow_dispatch` alone.
    The engine and the format crate take the `main` commit whose version they
    release; the plugin takes the version it moves `main` to (§5)."""
    doc = load(path.read_text(encoding="utf-8"))
    on = doc["on"]
    assert isinstance(on, dict) and sorted(on) == ["workflow_dispatch"], f"{path.name} starts on {sorted(on)}"
    inputs = on["workflow_dispatch"]["inputs"]
    if line == "delvewright":
        assert sorted(inputs) == ["version"], inputs
    else:
        assert sorted(inputs) == ["commit"] and inputs["commit"].get("default") == "main", inputs


@pytest.mark.parametrize("line,path", sorted(RELEASE_WORKFLOWS.items()))
def test_each_release_derives_its_tag_through_the_grammar(line, path):
    assert f"tools/lib/release_tags.py tag {line} " in _runs(path), f"{path.name} does not derive its tag"


def test_no_workflow_starts_on_a_tag_push():
    """Every workflow in the directory, not only the three: a tag push starts
    nothing, so creating a release tag can never be what publishes."""
    judged = 0
    for path in sorted(WORKFLOWS.glob("*.yml")):
        doc = load(path.read_text(encoding="utf-8"))
        on = doc.get("on") if isinstance(doc, dict) else None
        judged += 1
        push = on.get("push") if isinstance(on, dict) else None
        assert not (isinstance(push, dict) and push.get("tags")), f"{path.name} starts on a tag push"
    assert judged >= 3, f"only {judged} workflow file(s) read"
