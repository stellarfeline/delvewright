r"""Guards for `tools/check-numbered-doc-uniqueness.py` (task #111).

The red this gate exists to prevent: two branches each pick "the next spec
number" against the SAME `docs/specs/` they can each see, and neither sees the
other's file, because two new files never produce a git conflict. PR #361
(`spec-0033-declared-body-traversal.md`) and a later PR
(`spec-0033-grammar-corpus.md`) held number 0033 for three days; a per-branch
uniqueness check would have been green on both, every day, because the
collision exists only in the UNION of the two trees.

These tests build tiny real git repos — an `origin/main` commit constructed
directly with `hash-object`/`write-tree`/`commit-tree` (no clone, no network),
plus a plain on-disk `docs/specs/`/`docs/adr/` standing in for "this branch's
checkout" (the gate reads the checkout with `Path.iterdir()`, never through
git, so it never needs to be committed) — and assert the gate catches the
`#361`-shaped union collision, a same-branch self-collision, a pre-existing
`origin/main` self-collision, and refuses to run at all against an unfetched
base rather than silently comparing against nothing.
"""

from __future__ import annotations

import importlib.util
import os
import subprocess
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[2]
CHECKER = REPO / "tools" / "check-numbered-doc-uniqueness.py"


@pytest.fixture
def checker():
    """The gate, loaded fresh so `ROOT` can be pointed at a fixture repo."""
    spec = importlib.util.spec_from_file_location("cndu", CHECKER)
    mod = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(mod)
    return mod


def run(args: list[str], cwd: Path, input: str = "") -> str:
    result = subprocess.run(
        args, cwd=cwd, capture_output=True, text=True, input=input
    )
    assert result.returncode == 0, f"{args}: {result.stderr}"
    return result.stdout


def init_repo(root: Path) -> None:
    root.mkdir(parents=True, exist_ok=True)
    run(["git", "init", "-q", "-b", "main"], cwd=root)
    run(["git", "config", "user.email", "test@example.com"], cwd=root)
    run(["git", "config", "user.name", "Test"], cwd=root)


def commit_base(root: Path, files: dict[str, str]) -> str:
    """Build a commit purely via plumbing (no checkout) and return its sha.

    Standing in for `origin/main`: everything here goes through a SCRATCH
    index file (`GIT_INDEX_FILE`), so it never touches the real index or the
    working tree and can never collide with the on-disk "this branch" files
    the gate reads separately via `write_local`.
    """
    idx = root / ".git" / "tmp-index"
    env = {**os.environ, "GIT_INDEX_FILE": str(idx)}
    subprocess.run(["git", "read-tree", "--empty"], cwd=root, env=env, check=True)
    for relpath, content in files.items():
        blob = run(["git", "hash-object", "-w", "--stdin"], cwd=root, input=content).strip()
        subprocess.run(
            ["git", "update-index", "--add", "--cacheinfo", f"100644,{blob},{relpath}"],
            cwd=root,
            env=env,
            check=True,
            capture_output=True,
        )
    tree = subprocess.run(
        ["git", "write-tree"], cwd=root, env=env, check=True, capture_output=True, text=True
    ).stdout.strip()
    idx.unlink(missing_ok=True)
    sha = run(["git", "commit-tree", tree, "-m", "origin/main fixture commit"], cwd=root).strip()
    return sha


def make_shallow(root: Path) -> None:
    """Give the fixture a real shallow boundary — the same `.git/shallow` file a
    `git fetch --depth=1` writes, carrying a real commit rather than a placeholder."""
    tree = run(["git", "hash-object", "-w", "-t", "tree", "--stdin"], cwd=root, input="").strip()
    boundary = run(["git", "commit-tree", tree, "-m", "shallow boundary"], cwd=root).strip()
    (root / ".git" / "shallow").write_text(boundary + "\n", encoding="utf-8")
    assert (
        run(["git", "rev-parse", "--is-shallow-repository"], cwd=root).strip() == "true"
    ), "fixture did not actually become shallow"


def set_origin_main(root: Path, sha: str) -> None:
    run(["git", "update-ref", "refs/remotes/origin/main", sha], cwd=root)


def write_local(root: Path, files: dict[str, str]) -> None:
    """Files as they sit in the checkout — never committed, never need to be:
    `worktree_numbers()` reads the filesystem directly."""
    for relpath, content in files.items():
        path = root / relpath
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")


SEED_SPECS = {"docs/specs/spec-0001-seed.md": "seed"}
SEED_ADRS = {"docs/adr/0001-seed.md": "seed"}


def test_clean_checkout_matches_base(checker, tmp_path, capsys, monkeypatch):
    init_repo(tmp_path)
    base_sha = commit_base(tmp_path, {**SEED_SPECS, **SEED_ADRS})
    set_origin_main(tmp_path, base_sha)
    write_local(tmp_path, {**SEED_SPECS, **SEED_ADRS})

    checker.ROOT = tmp_path
    monkeypatch.setattr("sys.argv", ["check-numbered-doc-uniqueness.py"])
    assert checker.main() == 0
    out = capsys.readouterr().out
    assert "OK" in out
    # Binding count, always printed (CLAUDE.md: a green gate that binds to
    # nothing is VACUOUS).
    assert "spec: 1 here, 1 at origin/main" in out
    assert "adr: 1 here, 1 at origin/main" in out


def test_361_shaped_cross_branch_collision(checker, tmp_path, capsys, monkeypatch):
    """The actual historical incident: this branch's own spec-0033 vs a
    DIFFERENT spec-0033 that reached origin/main from elsewhere."""
    init_repo(tmp_path)
    base_sha = commit_base(
        tmp_path,
        {
            **SEED_SPECS,
            **SEED_ADRS,
            "docs/specs/spec-0033-declared-body-traversal.md": "the #361 spec",
        },
    )
    set_origin_main(tmp_path, base_sha)
    write_local(
        tmp_path,
        {
            **SEED_SPECS,
            **SEED_ADRS,
            "docs/specs/spec-0033-grammar-corpus.md": "the other spec",
        },
    )

    checker.ROOT = tmp_path
    monkeypatch.setattr("sys.argv", ["check-numbered-doc-uniqueness.py"])
    assert checker.main() == 1
    err = capsys.readouterr().err
    assert "spec-0033 is claimed by 2 different files" in err
    assert "docs/specs/spec-0033-declared-body-traversal.md  (origin/main)" in err
    assert "docs/specs/spec-0033-grammar-corpus.md  (this branch)" in err
    # Actionable: names the fix, not just the symptom.
    assert "rename ONE of these files" in err


def test_renumbering_the_collision_away_is_green(checker, tmp_path, capsys, monkeypatch):
    """Same fixture as above, but this branch's file already moved to the next
    free number — the fix CI is supposed to accept."""
    init_repo(tmp_path)
    base_sha = commit_base(
        tmp_path,
        {
            **SEED_SPECS,
            **SEED_ADRS,
            "docs/specs/spec-0033-declared-body-traversal.md": "the #361 spec",
        },
    )
    set_origin_main(tmp_path, base_sha)
    write_local(
        tmp_path,
        {
            **SEED_SPECS,
            **SEED_ADRS,
            "docs/specs/spec-0034-grammar-corpus.md": "the other spec, renumbered",
        },
    )

    checker.ROOT = tmp_path
    monkeypatch.setattr("sys.argv", ["check-numbered-doc-uniqueness.py"])
    assert checker.main() == 0
    assert "OK" in capsys.readouterr().out


def test_local_self_collision(checker, tmp_path, capsys, monkeypatch):
    """Two differently-named files under one number, both only on this
    branch — origin/main is not even involved."""
    init_repo(tmp_path)
    base_sha = commit_base(tmp_path, {**SEED_SPECS, **SEED_ADRS})
    set_origin_main(tmp_path, base_sha)
    write_local(
        tmp_path,
        {
            **SEED_SPECS,
            **SEED_ADRS,
            "docs/specs/spec-0002-first-take.md": "a",
            "docs/specs/spec-0002-second-take.md": "b",
        },
    )

    checker.ROOT = tmp_path
    monkeypatch.setattr("sys.argv", ["check-numbered-doc-uniqueness.py"])
    assert checker.main() == 1
    err = capsys.readouterr().err
    assert "spec-0002 is claimed by 2 different files" in err
    assert "spec-0002-first-take.md  (this branch)" in err
    assert "spec-0002-second-take.md  (this branch)" in err


def test_base_already_carries_a_duplicate(checker, tmp_path, capsys, monkeypatch):
    """origin/main itself already holds two files under one ADR number —
    "the case where main itself is the thing that moved." This branch's
    checkout does not even touch the ADR series."""
    init_repo(tmp_path)
    base_sha = commit_base(
        tmp_path,
        {
            **SEED_SPECS,
            "docs/adr/0002-first.md": "a",
            "docs/adr/0002-second.md": "b",
        },
    )
    set_origin_main(tmp_path, base_sha)
    write_local(tmp_path, SEED_SPECS)  # no docs/adr/ at all in this checkout

    checker.ROOT = tmp_path
    monkeypatch.setattr("sys.argv", ["check-numbered-doc-uniqueness.py"])
    assert checker.main() == 1
    err = capsys.readouterr().err
    assert "ADR-0002 is claimed by 2 different files" in err
    assert "0002-first.md  (origin/main)" in err
    assert "0002-second.md  (origin/main)" in err


def test_unfetched_base_refuses_to_run(checker, tmp_path, capsys, monkeypatch):
    """No refs/remotes/origin/main at all — the gate must refuse, loudly and
    actionably, never silently compare against nothing.

    And the remedy must not damage the repository it is printed into: `--depth=1`
    in a full clone shallows it, and a shallow clone answers ancestry questions
    with confident wrong numbers instead of erroring."""
    init_repo(tmp_path)
    write_local(tmp_path, {**SEED_SPECS, **SEED_ADRS})

    checker.ROOT = tmp_path
    monkeypatch.setattr("sys.argv", ["check-numbered-doc-uniqueness.py"])
    assert checker.main() == 1
    err = capsys.readouterr().err
    assert "does not resolve to a commit" in err
    assert "git fetch --no-tags origin main:refs/remotes/origin/main" in err
    assert "--depth" not in err.split("Do NOT add")[0]


def test_unfetched_base_in_a_shallow_checkout_is_told_to_fetch_shallowly(
    checker, tmp_path, capsys, monkeypatch
):
    """CI's case, decided by looking at the repository rather than assuming it:
    there is no full history left to truncate, so the one-commit fetch is right."""
    init_repo(tmp_path)
    write_local(tmp_path, {**SEED_SPECS, **SEED_ADRS})
    make_shallow(tmp_path)

    checker.ROOT = tmp_path
    monkeypatch.setattr("sys.argv", ["check-numbered-doc-uniqueness.py"])
    assert checker.main() == 1
    err = capsys.readouterr().err
    assert "ALREADY\n    SHALLOW" in err
    assert "git fetch --no-tags --depth=1 origin main:refs/remotes/origin/main" in err


def test_vacuous_both_sides_empty_is_a_fail(checker, tmp_path, capsys, monkeypatch):
    """Neither this checkout nor origin/main has any spec files at all — a
    check that binds to nothing is not a pass (CLAUDE.md)."""
    init_repo(tmp_path)
    base_sha = commit_base(tmp_path, {"README.md": "x"})
    set_origin_main(tmp_path, base_sha)
    # No docs/specs, no docs/adr anywhere.

    checker.ROOT = tmp_path
    monkeypatch.setattr("sys.argv", ["check-numbered-doc-uniqueness.py"])
    assert checker.main() == 1
    err = capsys.readouterr().err
    assert "examined 0 files" in err
    assert "docs/specs" in err
    assert "docs/adr" in err
