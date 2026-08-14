r"""Guards for `tools/check-version-ledger-uniqueness.py`.

The red this gate exists to prevent: two branches each take "the next version"
off the same ledger they can each see, for DIFFERENT surfaces, and both are
green. PR #413 wrote `MIRROR_SINCE = "1.1.0"` and PR #417 wrote
`CONTRACT_SINCE = "1.1.0"` into a `crates/grammar/src/version.rs` each of them
was creating. The two files conflict textually — and the tempting resolution,
unioning both constants under one `1.1.0`, is the actual defect: an engine at
`1.1.0` then accepts a `1.1.0` document written against the other construct and
silently drops it.

These tests build tiny real git repos — an `origin/main` commit constructed
directly with `hash-object`/`write-tree`/`commit-tree` (no clone, no network),
plus plain on-disk files standing in for "this branch's checkout" (the gate reads
the checkout with `Path.read_text()`, never through git, so nothing needs to be
committed). The fixture ledgers are synthetic but are written in the exact source
shape the real ones have, because the shape IS what the gate reads.

The last test is the odd one: it asserts a documented BLIND SPOT rather than a
catch. `dsl_version`'s anchors are self-naming (`0.11.0` forces `is_v11`), so
rule 1 cannot separate two branches that take `0.11.0` for different surfaces.
That limit is stated in the script's docstring; pinning it here makes it a fact
under test instead of a claim, so a future change that closes it fails loudly
rather than leaving the docstring lying.
"""

from __future__ import annotations

import importlib.util
import os
import subprocess
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[2]
CHECKER = REPO / "tools" / "check-version-ledger-uniqueness.py"

GRAMMAR = "crates/grammar/src/version.rs"
DSL = "crates/dsl/src/envelope.rs"


@pytest.fixture
def checker():
    """The gate, loaded fresh so `ROOT` can be pointed at a fixture repo."""
    spec = importlib.util.spec_from_file_location("cvlu", CHECKER)
    mod = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(mod)
    return mod


def run(args: list[str], cwd: Path, input: str = "") -> str:
    result = subprocess.run(args, cwd=cwd, capture_output=True, text=True, input=input)
    assert result.returncode == 0, f"{args}: {result.stderr}"
    return result.stdout


def init_repo(root: Path) -> None:
    root.mkdir(parents=True, exist_ok=True)
    run(["git", "init", "-q", "-b", "main"], cwd=root)
    run(["git", "config", "user.email", "test@example.com"], cwd=root)
    run(["git", "config", "user.name", "Test"], cwd=root)


def commit_base(root: Path, files: dict[str, str]) -> str:
    """Build a commit purely via plumbing (no checkout) and return its sha.

    Everything goes through a SCRATCH index (`GIT_INDEX_FILE`), so it never
    touches the real index or the working tree and cannot collide with the
    on-disk "this branch" files the gate reads separately.
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
    return run(["git", "commit-tree", tree, "-m", "origin/main fixture"], cwd=root).strip()


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
    for relpath, content in files.items():
        path = root / relpath
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")


# --- fixture ledgers, in the exact source shape the real ones have -----------


def grammar_ledger(versions: list[str], sinces: dict[str, str], reserved: dict[str, str]) -> str:
    body = [
        "//! fixture ledger",
        f'pub const LATEST_PROGRAM_VERSION: &str = "{versions[-1]}";',
        "pub const SUPPORTED_PROGRAM_VERSIONS: &[&str] = &["
        + ", ".join(f'"{v}"' for v in versions)
        + "];",
    ]
    if reserved:
        rows = ", ".join(f'("{v}", "{anchor}")' for v, anchor in reserved.items())
        body.append(f"pub const RESERVED_VERSIONS: &[(&str, &str)] = &[{rows}];")
    for name, version in sinces.items():
        body.append(f'pub const {name}: &str = "{version}";')
    return "\n".join(body) + "\n"


def dsl_ledger(versions: list[str], predicates: dict[str, int]) -> str:
    """`versions` in order; `predicates` maps `is_vNN` -> its ordinal threshold."""
    body = [
        "//! fixture ledger",
        "pub const SUPPORTED_DSL_VERSIONS: &[&str] = &[\n    "
        + ", ".join(f'"{v}"' for v in versions)
        + ",\n];",
        "fn ordinal(version: &str) -> u32 {",
        "    match version {",
    ]
    for i, v in enumerate(versions):
        body.append(f'        "{v}" => {i + 2},')
    body += ["        _ => 0,", "    }", "}"]
    for name, threshold in predicates.items():
        body.append(
            f"pub fn {name}(version: &str) -> bool {{\n    ordinal(version) >= {threshold}\n}}"
        )
    return "\n".join(body) + "\n"


# `0.2.0 .. 0.5.0`, claimed by `is_v03 .. is_v05`; `0.2.0` is the founding
# version and is claimed by nothing, exactly like the real ledger.
DSL_VERSIONS = ["0.2.0", "0.3.0", "0.4.0", "0.5.0"]
DSL_PREDICATES = {"is_v03": 3, "is_v04": 4, "is_v05": 5}
DSL_OK = dsl_ledger(DSL_VERSIONS, DSL_PREDICATES)

# `origin/main` after PR #413 merged: the ledger names 1.1.0, and 1.1.0 is the
# frame's direction.
GRAMMAR_AFTER_413 = grammar_ledger(["1.0.0", "1.1.0"], {"MIRROR_SINCE": "1.1.0"}, {})


def go(checker, root: Path, monkeypatch, base: str = "origin/main") -> int:
    checker.ROOT = root
    argv = ["check-version-ledger-uniqueness.py"]
    if base != "origin/main":
        argv += ["--base", base]
    monkeypatch.setattr("sys.argv", argv)
    return checker.main()


def scenario(root: Path, base_files: dict[str, str], local_files: dict[str, str]) -> None:
    init_repo(root)
    set_origin_main(root, commit_base(root, base_files))
    write_local(root, local_files)


# --- the incident, both directions ------------------------------------------


def test_413_shaped_collision_once_the_first_pr_merges(checker, tmp_path, capsys, monkeypatch):
    """The actual incident: this branch's `CONTRACT_SINCE = 1.1.0` against an
    `origin/main` that now carries #413's `MIRROR_SINCE = 1.1.0`."""
    scenario(
        tmp_path,
        {GRAMMAR: GRAMMAR_AFTER_413, DSL: DSL_OK},
        {
            GRAMMAR: grammar_ledger(["1.0.0", "1.1.0"], {"CONTRACT_SINCE": "1.1.0"}, {}),
            DSL: DSL_OK,
        },
    )
    assert go(checker, tmp_path, monkeypatch) == 1
    err = capsys.readouterr().err
    assert "version 1.1.0 is claimed by 2 different surfaces" in err
    assert "CONTRACT_SINCE  (this branch)" in err
    assert "MIRROR_SINCE  (origin/main)" in err
    # Actionable, and it names the resolution that is NOT acceptable.
    assert "move ONE of these surfaces to the next version free on BOTH sides" in err
    assert "Unioning them under one number is the defect" in err


def test_the_renumber_with_a_reservation_is_green(checker, tmp_path, capsys, monkeypatch):
    """The fix CI must accept: the contract moves to 1.2.0, and 1.1.0 is
    RESERVED under the name of the constant #413 defines — so the forward
    declaration and the change that fulfils it agree instead of colliding."""
    scenario(
        tmp_path,
        {GRAMMAR: GRAMMAR_AFTER_413, DSL: DSL_OK},
        {
            GRAMMAR: grammar_ledger(
                ["1.0.0", "1.1.0", "1.2.0"],
                {"CONTRACT_SINCE": "1.2.0"},
                {"1.1.0": "MIRROR_SINCE"},
            ),
            DSL: DSL_OK,
        },
    )
    assert go(checker, tmp_path, monkeypatch) == 0
    out = capsys.readouterr().out
    assert "OK" in out
    # Binding counts, always printed, and each is the quantity its words claim:
    # three ledger entries here against two at base, two anchors (the contract's
    # own and the reserved one) against base's one, one reservation.
    assert "grammar-program: 3 versions here (2 at origin/main), 2 anchors here " in out
    assert "(1 at origin/main), 1 reserved, 0 collision(s)" in out
    assert "dsl-campaign: 4 versions here (4 at origin/main), 3 anchors here " in out


def test_a_reservation_naming_the_wrong_anchor_is_red(checker, tmp_path, capsys, monkeypatch):
    """A forward declaration is only worth something if being WRONG is loud: the
    reservation guesses `REFLECT_SINCE`, `MIRROR_SINCE` is what landed."""
    scenario(
        tmp_path,
        {GRAMMAR: GRAMMAR_AFTER_413, DSL: DSL_OK},
        {
            GRAMMAR: grammar_ledger(
                ["1.0.0", "1.1.0", "1.2.0"],
                {"CONTRACT_SINCE": "1.2.0"},
                {"1.1.0": "REFLECT_SINCE"},
            ),
            DSL: DSL_OK,
        },
    )
    assert go(checker, tmp_path, monkeypatch) == 1
    err = capsys.readouterr().err
    assert "version 1.1.0 is claimed by 2 different surfaces" in err
    assert "REFLECT_SINCE  (this branch)" in err
    assert "MIRROR_SINCE  (origin/main)" in err


# --- rules 2 to 5 -----------------------------------------------------------


def test_a_fence_that_moves_after_it_shipped_is_red(checker, tmp_path, capsys, monkeypatch):
    """Rule 2. `MIRROR_SINCE` is at 1.1.0 on the base; moving it to 1.2.0
    changes what every already-written 1.1.0 document means."""
    scenario(
        tmp_path,
        {GRAMMAR: GRAMMAR_AFTER_413, DSL: DSL_OK},
        {
            GRAMMAR: grammar_ledger(
                ["1.0.0", "1.1.0", "1.2.0"], {"MIRROR_SINCE": "1.2.0"}, {}
            ),
            DSL: DSL_OK,
        },
    )
    assert go(checker, tmp_path, monkeypatch) == 1
    err = capsys.readouterr().err
    assert "MIRROR_SINCE is at ['1.2.0'] on this branch and ['1.1.0'] at origin/main" in err
    assert "may not move once it is on origin/main" in err


def test_a_version_nothing_claims_is_red(checker, tmp_path, capsys, monkeypatch):
    """Rule 3, and the rule that makes rule 1 hard to route around: skipping a
    number does not free you from declaring what it is."""
    scenario(
        tmp_path,
        {GRAMMAR: GRAMMAR_AFTER_413, DSL: DSL_OK},
        {
            GRAMMAR: grammar_ledger(
                ["1.0.0", "1.1.0", "1.2.0"],
                {"MIRROR_SINCE": "1.1.0", "CONTRACT_SINCE": "1.2.0"},
                {},
            ),
            # A dsl version appended with no predicate claiming it — proves the
            # dsl row is a live participant and not decoration.
            DSL: dsl_ledger(DSL_VERSIONS + ["0.6.0"], DSL_PREDICATES),
        },
    )
    assert go(checker, tmp_path, monkeypatch) == 1
    err = capsys.readouterr().err
    assert "dsl-campaign: version 0.6.0 is in the ledger and no fence anchor claims it" in err
    assert "a number a second change can take" in err
    # The grammar half of the same tree is clean, so exactly one finding.
    assert "1 finding(s)" in err


def test_a_ledger_that_is_not_append_only_is_red(checker, tmp_path, capsys, monkeypatch):
    """Rule 4: only the end of the list is free."""
    scenario(
        tmp_path,
        {GRAMMAR: GRAMMAR_AFTER_413, DSL: DSL_OK},
        {
            GRAMMAR: grammar_ledger(
                ["1.0.0", "1.0.5", "1.1.0"],
                {"MIRROR_SINCE": "1.1.0", "CONTRACT_SINCE": "1.0.5"},
                {},
            ),
            DSL: DSL_OK,
        },
    )
    assert go(checker, tmp_path, monkeypatch) == 1
    err = capsys.readouterr().err
    assert "renumbered, reordered or inserted rather than appended" in err


def test_a_reservation_whose_surface_landed_is_red(checker, tmp_path, capsys, monkeypatch):
    """Rule 5: the merge that implements the surface deletes the reservation.
    Left standing, it refuses a version the engine can now honour."""
    scenario(
        tmp_path,
        {GRAMMAR: GRAMMAR_AFTER_413, DSL: DSL_OK},
        {
            GRAMMAR: grammar_ledger(
                ["1.0.0", "1.1.0", "1.2.0"],
                {"MIRROR_SINCE": "1.1.0", "CONTRACT_SINCE": "1.2.0"},
                {"1.1.0": "MIRROR_SINCE"},
            ),
            DSL: DSL_OK,
        },
    )
    assert go(checker, tmp_path, monkeypatch) == 1
    err = capsys.readouterr().err
    assert "reserved for MIRROR_SINCE, and MIRROR_SINCE is defined in this same tree" in err
    assert "delete the RESERVED_VERSIONS row" in err


# --- the gate's own vacuity and shape guards --------------------------------


def test_a_ledger_absent_on_both_sides_is_red(checker, tmp_path, capsys, monkeypatch):
    """CLAUDE.md: a green gate that binds to nothing is VACUOUS. The file moving
    must not read as "nothing to check"."""
    scenario(tmp_path, {DSL: DSL_OK}, {DSL: DSL_OK})
    assert go(checker, tmp_path, monkeypatch) == 1
    err = capsys.readouterr().err
    assert "exists neither in this checkout nor at origin/main" in err
    assert "A check that binds to nothing is not a pass" in err
    assert "grammar-program: absent on both sides (VACUOUS)" in err


def test_deleting_a_ledger_is_red(checker, tmp_path, capsys, monkeypatch):
    scenario(tmp_path, {GRAMMAR: GRAMMAR_AFTER_413, DSL: DSL_OK}, {DSL: DSL_OK})
    assert go(checker, tmp_path, monkeypatch) == 1
    err = capsys.readouterr().err
    assert "exists at origin/main but not in this checkout" in err


def test_a_ledger_whose_shape_drifted_exits_two(checker, tmp_path, capsys, monkeypatch):
    """A gate that cannot find the thing it reads must say so, not pass. Exit 2
    separates "the source moved" from "the source is wrong"."""
    scenario(
        tmp_path,
        {GRAMMAR: GRAMMAR_AFTER_413, DSL: DSL_OK},
        {GRAMMAR: GRAMMAR_AFTER_413.replace("SUPPORTED_PROGRAM_VERSIONS", "PROGRAM_VERSIONS"),
         DSL: DSL_OK},
    )
    assert go(checker, tmp_path, monkeypatch) == 2
    err = capsys.readouterr().err
    assert "no longer matches" in err
    assert "never loosen the check" in err


def test_anchors_that_stop_matching_exit_two(checker, tmp_path, capsys, monkeypatch):
    """The extractor's own vacuity guard: a ledger naming two or more versions
    with zero traceable anchors is a drifted pattern, not a tree where every
    version happens to be unclaimed."""
    scenario(
        tmp_path,
        {GRAMMAR: GRAMMAR_AFTER_413, DSL: DSL_OK},
        {GRAMMAR: GRAMMAR_AFTER_413.replace("MIRROR_SINCE", "MIRROR_FROM"), DSL: DSL_OK},
    )
    assert go(checker, tmp_path, monkeypatch) == 2
    err = capsys.readouterr().err
    assert "ZERO of them could be traced to a fence anchor" in err


def test_an_unfetched_base_refuses_to_run(checker, tmp_path, capsys, monkeypatch):
    """Refusing, loudly, with a remedy that does not damage the repository it is
    printed into. A full clone must never be told to `--depth=1`: that converts
    it to a shallow one, after which ancestry answers are wrong numbers rather
    than errors."""
    init_repo(tmp_path)
    write_local(tmp_path, {GRAMMAR: GRAMMAR_AFTER_413, DSL: DSL_OK})
    assert go(checker, tmp_path, monkeypatch) == 1
    err = capsys.readouterr().err
    assert "does not resolve to a commit" in err
    assert "git fetch --no-tags origin main:refs/remotes/origin/main" in err
    assert "--depth" not in err.split("Do NOT add")[0]


def test_an_already_shallow_checkout_is_told_to_fetch_shallowly(
    checker, tmp_path, capsys, monkeypatch
):
    """The other half of the same rule. There is no full history left to
    truncate here, so the cheap fetch is the right one — this is CI's case, and
    it is decided by looking at the repository rather than by assuming it."""
    init_repo(tmp_path)
    write_local(tmp_path, {GRAMMAR: GRAMMAR_AFTER_413, DSL: DSL_OK})
    make_shallow(tmp_path)
    assert go(checker, tmp_path, monkeypatch) == 1
    err = capsys.readouterr().err
    assert "ALREADY\n    SHALLOW" in err
    assert "git fetch --no-tags --depth=1 origin main:refs/remotes/origin/main" in err


def test_a_clean_tree_is_green_and_prints_both_bindings(checker, tmp_path, capsys, monkeypatch):
    scenario(
        tmp_path,
        {GRAMMAR: GRAMMAR_AFTER_413, DSL: DSL_OK},
        {GRAMMAR: GRAMMAR_AFTER_413, DSL: DSL_OK},
    )
    assert go(checker, tmp_path, monkeypatch) == 0
    out = capsys.readouterr().out
    assert "grammar-program: 2 versions here (2 at origin/main), 1 anchors here" in out
    assert "dsl-campaign: 4 versions here (4 at origin/main), 3 anchors here" in out


# --- the documented blind spot, pinned ---------------------------------------


def test_dsl_same_number_collision_is_the_documented_blind_spot(
    checker, tmp_path, capsys, monkeypatch
):
    """Rule 1 cannot bind on `dsl_version`, and the docstring says so.

    `0.6.0` forces the anchor name `is_v06` in every branch that adds it, so two
    branches taking `0.6.0` for different surfaces produce the SAME anchor and
    rule 1 sees one claim. This test pins that limit: if a future change gives
    the ledger a surface label rule 1 can compare, this test reds and the
    docstring's limitation section is what needs rewriting.
    """
    both = dsl_ledger(DSL_VERSIONS + ["0.6.0"], {**DSL_PREDICATES, "is_v06": 6})
    scenario(
        tmp_path,
        {GRAMMAR: GRAMMAR_AFTER_413, DSL: both},
        {GRAMMAR: GRAMMAR_AFTER_413, DSL: both},
    )
    assert go(checker, tmp_path, monkeypatch) == 0
    assert "0 collision(s)" in capsys.readouterr().out
