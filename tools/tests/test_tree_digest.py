"""Guards for `tools/tree-digest.py` — the instrument of the cross-OS ADR-0006 gate.

The gate this feeds compares two hosts. What makes such a comparison worthless
is not a wrong hash: it is a comparison that CANNOT disagree — because the
absolute checkout path was hashed into it (so it always differs), because paths
were dropped entirely (so a rename compares equal), or because one side digested
nothing (so two empties agree). Each of those is asserted here by construction.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent.parent
TOOL = REPO / "tools" / "tree-digest.py"


def run(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, str(TOOL), *args], capture_output=True, text=True
    )


def tree(root: Path, files: dict[str, str]) -> Path:
    for rel, body in files.items():
        p = root / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(body)
    return root


SAMPLE = {
    "datapack/pack.mcmeta": '{"pack":{}}\n',
    "datapack/data/x/function/a.mcfunction": "say a\n",
    "manifest.json": '{"campaign_id":"x"}\n',
}


def test_the_same_bytes_at_different_absolute_paths_agree(tmp_path):
    """The recorded trap: hashing `shasum` output hashes the PATHS too, and two
    checkouts never live at the same absolute path — a comparison built that way
    can only ever disagree."""
    a = tree(tmp_path / "some/deep/place/out", dict(SAMPLE))
    b = tree(tmp_path / "elsewhere/out", dict(SAMPLE))
    assert run("--root", str(a), "--out", str(tmp_path / "a.txt")).returncode == 0
    assert run("--root", str(b), "--out", str(tmp_path / "b.txt")).returncode == 0
    r = run("--compare", str(tmp_path / "a.txt"), str(tmp_path / "b.txt"))
    assert r.returncode == 0, r.stderr
    assert "IDENTICAL" in r.stdout


def test_a_rename_is_a_difference(tmp_path):
    """The other direction: dropping paths entirely would pass this."""
    a = tree(tmp_path / "a", dict(SAMPLE))
    moved = dict(SAMPLE)
    moved["datapack/data/x/function/b.mcfunction"] = moved.pop(
        "datapack/data/x/function/a.mcfunction"
    )
    b = tree(tmp_path / "b", moved)
    run("--root", str(a), "--out", str(tmp_path / "a.txt"))
    run("--root", str(b), "--out", str(tmp_path / "b.txt"))
    r = run("--compare", str(tmp_path / "a.txt"), str(tmp_path / "b.txt"))
    assert r.returncode == 1
    assert "a.mcfunction" in r.stderr and "b.mcfunction" in r.stderr


def test_a_changed_byte_names_the_file_that_moved(tmp_path):
    a = tree(tmp_path / "a", dict(SAMPLE))
    changed = dict(SAMPLE)
    changed["manifest.json"] = '{"campaign_id":"y"}\n'
    b = tree(tmp_path / "b", changed)
    run("--root", str(a), "--out", str(tmp_path / "a.txt"))
    run("--root", str(b), "--out", str(tmp_path / "b.txt"))
    r = run("--compare", str(tmp_path / "a.txt"), str(tmp_path / "b.txt"))
    assert r.returncode == 1
    assert "differs: manifest.json" in r.stderr


def test_an_empty_directory_that_stopped_being_written_is_a_difference(tmp_path):
    """A file walk cannot see an empty directory, and invisible is the direction
    that reads as a pass."""
    a = tree(tmp_path / "a", dict(SAMPLE))
    (a / "server" / "world").mkdir(parents=True)
    b = tree(tmp_path / "b", dict(SAMPLE))
    run("--root", str(a), "--out", str(tmp_path / "a.txt"))
    run("--root", str(b), "--out", str(tmp_path / "b.txt"))
    r = run("--compare", str(tmp_path / "a.txt"), str(tmp_path / "b.txt"))
    assert r.returncode == 1
    assert "server/world/" in r.stderr


def test_digesting_an_empty_tree_is_refused(tmp_path):
    empty = tmp_path / "empty"
    empty.mkdir()
    r = run("--root", str(empty), "--out", str(tmp_path / "e.txt"))
    assert r.returncode == 1
    assert "ZERO files" in r.stderr


def test_a_symlink_is_refused_rather_than_skipped(tmp_path):
    a = tree(tmp_path / "a", dict(SAMPLE))
    (a / "link").symlink_to(a / "manifest.json")
    r = run("--root", str(a), "--out", str(tmp_path / "a.txt"))
    assert r.returncode == 1
    assert "neither a regular file nor a directory" in r.stderr


def test_a_manifest_this_tool_did_not_write_is_refused(tmp_path):
    bogus = tmp_path / "bogus.txt"
    bogus.write_text("deadbeef  manifest.json\n")
    a = tree(tmp_path / "a", dict(SAMPLE))
    run("--root", str(a), "--out", str(tmp_path / "a.txt"))
    r = run("--compare", str(tmp_path / "a.txt"), str(bogus))
    assert r.returncode == 1
    assert "tree-digest v1" in r.stderr


def test_the_subject_script_and_the_job_name_the_same_thing():
    """The comparison is only about determinism if both hosts ran one subject.

    Binding: the two `tools/determinism-subject.sh` invocations in `ci.yml` — one
    in `rust (fmt, clippy, test)`, one in the macOS job — and the required
    context that names the second.
    """
    ci = (REPO / ".github" / "workflows" / "ci.yml").read_text(encoding="utf-8")
    # INVOCATIONS, not mentions: a comment naming the script is not a caller,
    # and counting mentions would make a prose edit move a binding count.
    invocations = ci.count("bash tools/determinism-subject.sh")
    assert invocations == 2, (
        f"the two hosts must build the SAME subject and {invocations} site(s) "
        "invoke it; a second statement of the subject in the workflow is how "
        "they drift apart"
    )
    context = "cross-OS determinism (macOS vs Linux bytes)"
    assert f"name: {context}" in ci
    manifest = (REPO / ".github" / "required-status-checks.txt").read_text(
        encoding="utf-8"
    )
    assert context in manifest.splitlines()
