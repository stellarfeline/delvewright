"""A Release's notes are a summary written before the release, never the merged list.

`tools/lib/release_notes.py` reads `RELEASE-NOTES.md`; `tools/ci/release-notes.py`
publishes an entry through it and `tools/ci/check-release-notes.py` judges every
entry through it. Each gate below is perturbed toward the shape the rule
forbids — the merged commit subjects copied in as bullets, an entry missing, a
Highlights section missing or over its ceiling, a heading outside the Release
title grammar — and must red by name; the clean shape must print the notes with
the opening, the entry and the compare link, and state what it compared.

Every repository here is built in the test's own directory: no test reads the
live changelog, the live workflows or the network.
"""

from __future__ import annotations

import json
import pathlib
import subprocess
import sys

import pytest

REPO = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "tools" / "lib"))

import release_notes  # noqa: E402

RELEASE_NOTES = REPO / "tools" / "ci" / "release-notes.py"
CHECK = REPO / "tools" / "ci" / "check-release-notes.py"

SUBJECTS = [
    "feat(view): the camera frames the whole scene (#7)",
    "fix(harness): no leg starts before the delve has placed its world (#8)",
    "docs: the walkthrough describes the one map route",
]

GOOD_ENTRY = """## delvec 1.15.0

### Highlights

- Review frames now show the whole scene, so a reviewer sees the room, not a corner of it.

### Added

- `delvec view` frames every review picture on the scene's bounds.

### Fixed

- The playtest bot waits for the world to be placed before its first leg, so a cold
  server no longer reads as a navigation failure.
"""

COPIED_ENTRY = """## delvec 1.15.0

### Highlights

- Review frames now show the whole scene.

### Added

- feat(view): the camera frames the whole scene (#7)

### Fixed

- No leg starts before the delve has placed its world.
"""


def _git(repo: pathlib.Path, *args: str) -> str:
    return subprocess.run(["git", "-C", str(repo), *args], check=True, capture_output=True, text=True).stdout


def _write(repo: pathlib.Path, rel: str, text: str) -> None:
    path = repo / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def _commit(repo: pathlib.Path, subject: str) -> None:
    _git(repo, "add", "-A")
    _git(repo, "commit", "-q", "-m", subject)


@pytest.fixture
def repo(tmp_path):
    """A repository with one released `delvec` (tagged), three merged changes since,
    and the tree stating `delvec` 1.15.0, `delvewright-dsl` 0.2.0, plugin 1.15.0."""
    root = tmp_path / "engine"
    root.mkdir()
    _git(root, "init", "-q", "-b", "main")
    _write(root, "versions.toml", '[engine]\nversion = "1.14.0"\ndsl_crate_version = "0.1.0"\n')
    _write(
        root,
        ".claude/skills/delvewright/.claude-plugin/plugin.json",
        json.dumps({"name": "delvewright", "version": "1.14.0"}) + "\n",
    )
    _write(
        root,
        ".claude/skills/delvewright/skills/new-delve/versions.toml",
        '[engine]\nref = "delvec--v1.14.0"\n',
    )
    _write(
        root,
        ".claude/skills/delvewright/skills/new-delve/SKILL.md",
        '---\nname: new-delve\nmetadata:\n  requires_delvec: ">=1.14.0, <2.0.0"\n---\n# page\n',
    )
    _write(root, "RELEASE-NOTES.md", "# Changelog\n\nHeader.\n")
    _commit(root, "chore: the released tree")
    _git(root, "tag", "-a", "delvec--v1.14.0", "-m", "delvec 1.14.0")
    for i, subject in enumerate(SUBJECTS):
        _write(root, f"src/change-{i}.txt", subject + "\n")
        _commit(root, subject)
    _write(root, "versions.toml", '[engine]\nversion = "1.15.0"\ndsl_crate_version = "0.2.0"\n')
    _write(
        root,
        ".claude/skills/delvewright/.claude-plugin/plugin.json",
        json.dumps({"name": "delvewright", "version": "1.15.0"}) + "\n",
    )
    _write(
        root,
        ".claude/skills/delvewright/skills/new-delve/versions.toml",
        '[engine]\nref = "delvec--v1.15.0"\n',
    )
    _commit(root, "release: delvec 1.15.0 names its tag")
    return root


def _with_changelog(repo: pathlib.Path, body: str) -> None:
    _write(repo, "RELEASE-NOTES.md", "# Changelog\n\nHeader.\n\n" + body)
    _commit(repo, "release: the notes for 1.15.0")


def _notes(repo: pathlib.Path, line: str = "delvec") -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, "-I", str(RELEASE_NOTES), line, "--commit", "HEAD", "--repo", str(repo), "--repository", "o/r"],
        capture_output=True,
        text=True,
    )


def _check(repo: pathlib.Path) -> subprocess.CompletedProcess:
    return subprocess.run([sys.executable, "-I", str(CHECK), "--repo", str(repo)], capture_output=True, text=True)


# ------------------------------------------------------------------ the reader --
def test_normalize_reads_a_bullet_and_a_subject_the_same_way():
    assert release_notes.normalize("feat(view): The camera **frames** the whole scene (#7)") == "the camera frames the whole scene"
    assert release_notes.normalize("The camera frames the whole scene.") == "the camera frames the whole scene"
    assert release_notes.normalize("fix!: a `thing`") == "a thing"


def test_parse_reads_entries_sections_and_continuations():
    entries, findings = release_notes.parse("# Changelog\n\nheader\n\n" + GOOD_ENTRY)
    assert findings == []
    assert [e.title for e in entries] == ["delvec 1.15.0"]
    e = entries[0]
    assert e.tag == "delvec--v1.15.0" and e.line == 5
    assert len(e.highlights) == 1
    assert list(e.groups) == ["Added", "Fixed"]
    assert e.groups["Fixed"][0].endswith("navigation failure.")  # the continuation joined
    assert e.body.startswith("### Highlights")
    assert release_notes.shape(e, SUBJECTS) == []


@pytest.mark.parametrize(
    "text,needle",
    [
        ("## delvec v1.15.0\n\n### Highlights\n\n- x\n\n### Fixed\n\n- y\n", "not a Release title"),
        ("## delvec 1.15.0\n\n- x\n", "stands before the first section"),
        ("## delvec 1.15.0\n\nA paragraph.\n\n### Highlights\n\n- x\n", "stands before the first section"),
        ("## delvec 1.15.0\n\n### Highlights\n\n- x\n\n#### Fixed\n\n- y\n", "an entry holds `###` sections only"),
        ("## delvec 1.15.0\n\n### Highlights\n\n- x\n\n### Highlights\n\n- y\n", "a second `### Highlights`"),
        ("## delvec 1.15.0\n\n### Fixed\n\n- y\n\n## delvec 1.14.0\n\n### Fixed\n\n- y\n", "one entry per line"),
        ("## delvec 1.15.0\n\n### Fixed\n\n- y\n\n## delvec 1.15.0\n\n### Fixed\n\n- y\n", "already an entry"),
    ],
)
def test_parse_names_each_structural_defect(text, needle):
    entries, findings = release_notes.parse(text)
    all_findings = findings + [f for e in entries for f in e.findings]
    assert any(needle in f for f in all_findings), all_findings


@pytest.mark.parametrize(
    "text,needle",
    [
        ("## delvec 1.15.0\n\n### Fixed\n\n- y\n", "no `### Highlights`"),
        ("## delvec 1.15.0\n\n### Highlights\n\n- x\n", "no group of changes"),
        ("## delvec 1.15.0\n\n### Highlights\n\n- x\n\nAbout thirty more.\n", "no group of changes"),
        ("## delvec 1.15.0\n\n### Highlights\n\n- x\n\n### Fixed\n", "`### Fixed` has no bullet"),
        (
            "## delvec 1.15.0\n\n### Highlights\n\n"
            + "".join(f"- h{i}\n" for i in range(release_notes.HIGHLIGHTS_CEILING + 1))
            + "\n### Fixed\n\n- y\n",
            f"at most {release_notes.HIGHLIGHTS_CEILING}",
        ),
        (
            "## delvec 1.15.0\n\n### Highlights\n\n- x\n\n### Fixed\n\n"
            + "".join(f"- c{i}\n" for i in range(release_notes.CHANGES_CEILING + 1)),
            f"at most {release_notes.CHANGES_CEILING}",
        ),
    ],
)
def test_shape_names_each_summary_defect(text, needle):
    entries, _ = release_notes.parse(text)
    findings = release_notes.shape(entries[0], SUBJECTS)
    assert any(needle in f for f in findings), findings


def test_shape_refuses_a_bullet_that_is_a_merged_subject_in_any_section():
    entries, _ = release_notes.parse(COPIED_ENTRY)
    findings = release_notes.shape(entries[0], SUBJECTS)
    assert len(findings) == 2, findings
    assert all("is the merged change's own title" in f for f in findings)
    assert any("`### Added`" in f for f in findings) and any("`### Fixed`" in f for f in findings)
    # With no previous release there is nothing to be a copy of.
    assert release_notes.shape(entries[0], None) == []


def test_groups_are_the_authors_and_prose_is_read_too():
    """A group may be a Keep a Changelog kind or an area of the engine; a closing
    paragraph is admitted, and it is compared with the merged subjects like any
    bullet, so the list cannot come back as paragraphs."""
    text = (
        "## delvec 1.15.0\n\n### Highlights\n\n- x\n\n### Rendering and review frames\n\n- Frames show the scene.\n\n"
        "About thirty further merged changes alter nothing a creator uses.\n"
    )
    entries, findings = release_notes.parse(text)
    assert findings == [] and entries[0].findings == []
    assert list(entries[0].groups) == ["Rendering and review frames"]
    assert entries[0].prose == [("Rendering and review frames", "About thirty further merged changes alter nothing a creator uses.")]
    assert release_notes.shape(entries[0], SUBJECTS) == []
    pasted = text + "\n" + SUBJECTS[2] + "\n"
    entries, _ = release_notes.parse(pasted)
    findings = release_notes.shape(entries[0], SUBJECTS)
    assert len(findings) == 1 and "is the merged change's own title" in findings[0], findings


# -------------------------------------------------------------- release-notes --
def test_the_notes_are_the_opening_the_entry_and_the_compare_link(repo):
    _with_changelog(repo, GOOD_ENTRY)
    result = _notes(repo)
    assert result.returncode == 0, result.stderr
    out = result.stdout
    assert out.startswith("**`delvec` 1.15.0**")
    assert "speaks `dsl_version` **0.2.0**" in out
    assert "### Highlights" in out and "### Added" in out and "### Fixed" in out
    assert "cold\nserver" not in out and "a cold\n  server" in out  # the entry's bytes, as written
    assert out.rstrip().endswith("https://github.com/o/r/compare/delvec--v1.14.0...delvec--v1.15.0")
    # three merged changes plus the two release-preparing commits since the tag
    assert "compared with 5 merged subject(s) since delvec--v1.14.0" in result.stderr
    assert "## delvec 1.15.0" not in out  # the Release's title is the heading; it is not repeated in the body


def test_a_release_with_no_entry_is_refused(repo):
    result = _notes(repo)
    assert result.returncode == 1
    assert "has no entry `## delvec 1.15.0`" in result.stderr and "refused" in result.stderr


def test_a_tree_with_no_changelog_is_refused(repo):
    _git(repo, "rm", "-q", "RELEASE-NOTES.md")
    _commit(repo, "chore: drop the changelog")
    result = _notes(repo)
    assert result.returncode == 1
    assert "has no RELEASE-NOTES.md" in result.stderr


def test_an_entry_that_is_the_merged_list_is_refused(repo):
    _with_changelog(repo, COPIED_ENTRY)
    result = _notes(repo)
    assert result.returncode == 1
    assert "is not a summary" in result.stderr
    assert "is the merged change's own title" in result.stderr
    assert "feat(view): the camera frames the whole scene (#7)" in result.stderr


def test_an_entry_without_highlights_is_refused(repo):
    _with_changelog(repo, "## delvec 1.15.0\n\n### Fixed\n\n- A thing is fixed.\n")
    result = _notes(repo)
    assert result.returncode == 1
    assert "no `### Highlights`" in result.stderr


def test_the_first_release_of_a_line_has_no_compare_link_and_no_copy_check(repo):
    body = "## delvewright-dsl 0.2.0\n\n### Highlights\n\n- A new format.\n\n### Added\n\n- " + SUBJECTS[2] + "\n"
    _with_changelog(repo, body)
    result = _notes(repo, "delvewright-dsl")
    assert result.returncode == 0, result.stderr
    assert "first release of its line" in result.stdout
    assert "compared with 0 merged subject(s) since (no previous release of this line)" in result.stderr


def test_the_plugin_notes_read_the_pin_and_the_entry(repo):
    _with_changelog(repo, GOOD_ENTRY.replace("## delvec 1.15.0", "## delvewright 1.15.0"))
    result = _notes(repo, "delvewright")
    assert result.returncode == 0, result.stderr
    assert "ships at engine release `delvec--v1.15.0`" in result.stdout
    assert "### Highlights" in result.stdout
    assert "first release of its line" in result.stdout  # no `delvewright--v*` tag in this repository


def test_the_plugin_notes_pin_the_marketplace_with_a_hash_ref(repo):
    """Claude Code pins a GitHub marketplace to a branch or tag as `owner/repo#ref`;
    `owner/repo@ref` is not a form it reads. A creator who already holds the
    marketplace is told to update it, since the same name from another source is refused."""
    _with_changelog(repo, GOOD_ENTRY.replace("## delvec 1.15.0", "## delvewright 1.15.0"))
    result = _notes(repo, "delvewright")
    assert result.returncode == 0, result.stderr
    assert "`/plugin marketplace add stellarfeline/delvewright#delvec--v1.15.0`" in result.stdout
    assert "marketplace add stellarfeline/delvewright@" not in result.stdout
    assert "`/plugin marketplace update delvewright`" in result.stdout


# ------------------------------------------------------------ check-release-notes --
def test_check_passes_a_clean_changelog_and_states_what_it_compared(repo):
    _with_changelog(repo, GOOD_ENTRY)
    result = _check(repo)
    assert result.returncode == 0, result.stderr
    assert "1 entry judged" in result.stdout and "compared with 5 merged subject(s) since delvec--v1.14.0" in result.stdout
    assert "delvec 1.15.0: unreleased" in result.stdout


def test_check_passes_an_empty_changelog_saying_so(repo):
    result = _check(repo)
    assert result.returncode == 0, result.stderr
    assert "0 entries judged, 0 bullet(s), 0 merged subject(s) compared, 0 finding(s)" in result.stdout


def test_check_reds_on_the_merged_list(repo):
    _with_changelog(repo, COPIED_ENTRY)
    result = _check(repo)
    assert result.returncode == 1
    assert "is the merged change's own title" in result.stderr


def test_check_reds_on_a_version_nothing_names(repo):
    _with_changelog(repo, GOOD_ENTRY.replace("## delvec 1.15.0", "## delvec 1.16.0"))
    result = _check(repo)
    assert result.returncode == 1
    assert "names a version the tree does not state (delvec is 1.15.0 here) and no tag names" in result.stderr


def test_check_reds_on_a_heading_outside_the_title_grammar(repo):
    _with_changelog(repo, GOOD_ENTRY.replace("## delvec 1.15.0", "## delvec--v1.15.0"))
    result = _check(repo)
    assert result.returncode == 1
    assert "is not a Release title" in result.stderr


def test_check_accepts_the_release_being_re_run_and_refuses_a_stale_entry(repo):
    """The file holds the release being prepared. An entry whose tag is already
    written is the re-run case while the tree still states that version — judged
    over the range up to its tag — and stale once the line has moved on."""
    _with_changelog(repo, GOOD_ENTRY)
    _git(repo, "tag", "-a", "delvec--v1.15.0", "-m", "delvec 1.15.0")
    result = _check(repo)
    assert result.returncode == 0, result.stderr
    assert "delvec 1.15.0: released" in result.stdout
    assert "compared with 5 merged subject(s) since delvec--v1.14.0" in result.stdout
    _write(repo, "versions.toml", '[engine]\nversion = "1.16.0"\ndsl_crate_version = "0.2.0"\n')
    _commit(repo, "release: delvec 1.16.0 names its tag")
    result = _check(repo)
    assert result.returncode == 1
    assert "is stale: delvec--v1.15.0 is released and the tree is at 1.16.0" in result.stderr


def test_check_refuses_two_entries_for_one_line(repo):
    _with_changelog(repo, GOOD_ENTRY + "\n" + GOOD_ENTRY.replace("## delvec 1.15.0", "## delvec 1.14.0"))
    result = _check(repo)
    assert result.returncode == 1
    assert "the file holds one entry per line, the release being prepared" in result.stderr


def test_check_refuses_an_unreadable_tree(tmp_path):
    result = _check(tmp_path)
    assert result.returncode == 2
    assert "cannot read the tree" in result.stderr
