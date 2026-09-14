r"""Guard: one tag grammar, and each release workflow's trigger and identity bound to it (ADR-0028 §1, §7).

Two halves. The grammar itself (`tools/lib/release_tags.py`): strict semver, the
three names, the `v<semver>` shape refused. And the WORKFLOWS: each release
workflow's `on.push.tags` filter is read through the shared workflow parser and
its GitHub filter pattern is evaluated, so "a `v1.6.0` tag starts no release
run" is a measured property of the checked-in trigger, not a comment beside it —
and the identity step that stands behind the filter is the grammar's CLI with
the workflow's own name.

GitHub's filter-pattern language (Actions docs, "Filter pattern cheat sheet"):
`*` matches zero or more characters except `/`, `?` zero or one of the
preceding character, `+` one or more of the preceding character, `[]` a
character class; every other character is literal. `_glob` implements exactly
that subset, and refuses a pattern that uses anything else rather than guessing.
"""

from __future__ import annotations

import pathlib
import re
import subprocess
import sys

import pytest

REPO = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "tools" / "lib"))

import release_tags  # noqa: E402
from workflow_yaml import load  # noqa: E402

WORKFLOWS = REPO / ".github" / "workflows"
LINES = {
    "delvec": WORKFLOWS / "engine-release.yml",
    "delvewright": WORKFLOWS / "plugin-release.yml",
}


def _glob(pattern: str) -> re.Pattern[str]:
    out = []
    i = 0
    while i < len(pattern):
        c = pattern[i]
        if c == "[":
            j = pattern.index("]", i)
            out.append(pattern[i : j + 1])
            i = j + 1
            continue
        if c == "*":
            out.append("[^/]*")
        elif c in "+?":
            out.append(c)
        elif c in "!\\":
            raise AssertionError(f"filter construct {c!r} in {pattern!r} is outside what this test evaluates")
        else:
            out.append(re.escape(c))
        i += 1
    return re.compile("^" + "".join(out) + "$")


def _tag_filters(path: pathlib.Path) -> list[str]:
    doc = load(path.read_text(encoding="utf-8"))
    tags = doc["on"]["push"]["tags"]
    assert isinstance(tags, list) and tags, f"{path.name} has no push tag filter"
    return tags


def _starts(path: pathlib.Path, tag: str) -> bool:
    return any(_glob(p).match(tag) for p in _tag_filters(path))


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
        "delvewright-dsl--v0.25.0", "delvewright-dsl--v0.26.0",
    ]
    assert release_tags.previous("delvec--v1.6.0", tags) == "v1.5.0"
    assert release_tags.previous("delvec--v1.11.0", tags) == "delvec--v1.10.0"
    assert release_tags.previous("delvec--v1.7.0", tags) == "delvec--v1.6.0"
    assert release_tags.previous("delvewright--v1.4.5", tags) == "delvewright--v1.4.3"
    assert release_tags.previous("delvewright--v1.4.3", tags) is None
    assert release_tags.previous("delvewright-dsl--v0.26.0", tags) == "delvewright-dsl--v0.25.0"
    assert release_tags.previous("delvewright-dsl--v0.25.0", tags) is None


def test_the_cli_refuses_a_bare_v_tag_with_exit_1():
    result = subprocess.run(
        ["python3", str(REPO / "tools" / "lib" / "release_tags.py"), "identity", "delvec", "v1.6.0", "1.6.0"],
        capture_output=True, text=True,
    )
    assert result.returncode == 1, result.stdout + result.stderr
    assert "REFUSED" in result.stderr


# ------------------------------------------------------------- the triggers --
@pytest.mark.parametrize("line,path", sorted(LINES.items()))
def test_each_trigger_starts_its_own_line_only(line, path):
    own = release_tags.tag_for(line, "1.6.0")
    assert _starts(path, own), f"{path.name} does not start on {own}"
    for other in release_tags.NAMES:
        if other != line:
            tag = release_tags.tag_for(other, "1.6.0")
            assert not _starts(path, tag), f"{path.name} starts on another line's tag {tag}"
    # §7: no run can ever be started against the pre-grammar shape again.
    for legacy in ("v1.6.0", "v1.5.0"):
        assert not _starts(path, legacy), f"{path.name} starts on the legacy tag {legacy}"


@pytest.mark.parametrize("line,path", sorted(LINES.items()))
def test_each_identity_step_runs_the_grammar_for_its_own_line(line, path):
    doc = load(path.read_text(encoding="utf-8"))
    runs = [
        step["run"]
        for job in doc["jobs"].values()
        for step in job.get("steps", [])
        if isinstance(step, dict) and isinstance(step.get("run"), str)
    ]
    needle = re.compile(r"(release_tags\.py identity " + re.escape(line) + r" |--tag )")
    joined = "\n".join(runs)
    if line == "delvec":
        assert f"tools/lib/release_tags.py identity {line} " in joined
    else:
        # The plugin's identity is the §3 checker, which runs the same grammar.
        assert "tools/check-plugin-release-identity.py" in joined and needle.search(joined)


def test_no_release_workflow_filter_matches_a_bare_v_tag():
    """Every workflow in the directory, not only the two named above: a
    `v<semver>` tag starts nothing anywhere (§7)."""
    judged = 0
    for path in sorted(WORKFLOWS.glob("*.yml")):
        doc = load(path.read_text(encoding="utf-8"))
        on = doc.get("on") if isinstance(doc, dict) else None
        push = on.get("push") if isinstance(on, dict) else None
        tags = push.get("tags") if isinstance(push, dict) else None
        if not tags:
            continue
        judged += 1
        assert not any(_glob(p).match("v1.6.0") for p in tags), path.name
    assert judged >= 2, f"only {judged} workflow(s) carry a push tag filter; expected the two release lines"
