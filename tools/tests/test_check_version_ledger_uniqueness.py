r"""Guards for `tools/check-version-ledger-uniqueness.py`.

The red this gate exists to prevent: two branches each take "the next version"
off the same ledger they can each see, for DIFFERENT surfaces, and both are
green. One branch wrote `MIRROR_SINCE = "1.1.0"` and another wrote
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

The last group is the second incident, one ledger over. `dsl_version`'s
IMPLEMENTED anchors are self-naming (`0.12.0` forces `is_v12`), so rule 1 could
not separate two branches that took `0.12.0` for different surfaces — and two
did: `open-way` (spec-0042) and the horizon library, each checked, each found the
number free. Rule 6 is the repair: a version a branch ADDS must carry a
hand-written name, so the two branches disagree visibly and rule 1 — which has
always been able to read two names — sees them. These tests drive both halves,
plus the reservation lifecycle that lets a spec hold a number before its surface
exists.
"""

from __future__ import annotations

import importlib.util
import os
import re
import subprocess
import warnings
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[2]
CHECKER = REPO / "tools" / "check-version-ledger-uniqueness.py"

GRAMMAR = "crates/grammar/src/version.rs"
DSL = "crates/dsl/src/envelope.rs"


def load_checker():
    """The gate, loaded fresh so `ROOT` can be pointed at a fixture repo."""
    spec = importlib.util.spec_from_file_location("cvlu", CHECKER)
    mod = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(mod)
    return mod


@pytest.fixture
def checker():
    return load_checker()


# `LEDGERS` is the gate's own row per version ledger — the single authority over
# what a ledger IS, and the reason adding a third is one row rather than a new
# script. The live groups at the foot of this file enumerate from it rather than
# naming a ledger, so a third one is driven the day its row lands.
LEDGER_NAMES = [row["name"] for row in load_checker().LEDGERS]


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


def dsl_ledger(
    versions: list[str],
    predicates: dict[str, int],
    sinces: dict[str, str] | None = None,
    reserved: dict[str, str] | None = None,
) -> str:
    """`versions` in order; `predicates` maps `is_vNN` -> its ordinal threshold.

    `sinces` and `reserved` are the ledger's HAND-WRITTEN names — the same two
    shapes the grammar ledger carries, because the repair was to give this ledger
    the sibling's mechanism rather than a second one of its own.
    """
    body = [
        "//! fixture ledger",
        "pub const SUPPORTED_DSL_VERSIONS: &[&str] = &[\n    "
        + ", ".join(f'"{v}"' for v in versions)
        + ",\n];",
    ]
    if reserved:
        rows = ", ".join(f'("{v}", "{anchor}")' for v, anchor in reserved.items())
        body.append(f"pub const RESERVED_DSL_VERSIONS: &[(&str, &str)] = &[{rows}];")
    for name, version in (sinces or {}).items():
        body.append(f'pub const {name}: &str = "{version}";')
    body += ["fn ordinal(version: &str) -> u32 {", "    match version {"]
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

# `origin/main` after the mirror branch merged: the ledger names 1.1.0, and
# 1.1.0 is the frame's direction.
GRAMMAR_AFTER_MIRROR = grammar_ledger(["1.0.0", "1.1.0"], {"MIRROR_SINCE": "1.1.0"}, {})


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


def test_collision_once_the_first_branch_merges(checker, tmp_path, capsys, monkeypatch):
    """The actual incident: this branch's `CONTRACT_SINCE = 1.1.0` against an
    `origin/main` that now carries `MIRROR_SINCE = 1.1.0`."""
    scenario(
        tmp_path,
        {GRAMMAR: GRAMMAR_AFTER_MIRROR, DSL: DSL_OK},
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
    RESERVED under the name of the constant the mirror branch defines — so the forward
    declaration and the change that fulfils it agree instead of colliding."""
    scenario(
        tmp_path,
        {GRAMMAR: GRAMMAR_AFTER_MIRROR, DSL: DSL_OK},
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
        {GRAMMAR: GRAMMAR_AFTER_MIRROR, DSL: DSL_OK},
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
        {GRAMMAR: GRAMMAR_AFTER_MIRROR, DSL: DSL_OK},
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
        {GRAMMAR: GRAMMAR_AFTER_MIRROR, DSL: DSL_OK},
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
    # Two findings on the one number, and they say different things: rule 3 says
    # NOTHING claims it, rule 6 says nothing NAMES it. Adding `is_v06` silences
    # the first and leaves the second standing, which is the whole point of
    # having both. The grammar half of the same tree is clean.
    assert "version 0.6.0 is added by this branch and nothing NAMES it" in err
    assert "2 finding(s)" in err


def test_a_ledger_that_is_not_append_only_is_red(checker, tmp_path, capsys, monkeypatch):
    """Rule 4: only the end of the list is free."""
    scenario(
        tmp_path,
        {GRAMMAR: GRAMMAR_AFTER_MIRROR, DSL: DSL_OK},
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
        {GRAMMAR: GRAMMAR_AFTER_MIRROR, DSL: DSL_OK},
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
    scenario(tmp_path, {GRAMMAR: GRAMMAR_AFTER_MIRROR, DSL: DSL_OK}, {DSL: DSL_OK})
    assert go(checker, tmp_path, monkeypatch) == 1
    err = capsys.readouterr().err
    assert "exists at origin/main but not in this checkout" in err


def test_a_ledger_whose_shape_drifted_exits_two(checker, tmp_path, capsys, monkeypatch):
    """A gate that cannot find the thing it reads must say so, not pass. Exit 2
    separates "the source moved" from "the source is wrong"."""
    scenario(
        tmp_path,
        {GRAMMAR: GRAMMAR_AFTER_MIRROR, DSL: DSL_OK},
        {GRAMMAR: GRAMMAR_AFTER_MIRROR.replace("SUPPORTED_PROGRAM_VERSIONS", "PROGRAM_VERSIONS"),
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
        {GRAMMAR: GRAMMAR_AFTER_MIRROR, DSL: DSL_OK},
        {GRAMMAR: GRAMMAR_AFTER_MIRROR.replace("MIRROR_SINCE", "MIRROR_FROM"), DSL: DSL_OK},
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
    write_local(tmp_path, {GRAMMAR: GRAMMAR_AFTER_MIRROR, DSL: DSL_OK})
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
    write_local(tmp_path, {GRAMMAR: GRAMMAR_AFTER_MIRROR, DSL: DSL_OK})
    make_shallow(tmp_path)
    assert go(checker, tmp_path, monkeypatch) == 1
    err = capsys.readouterr().err
    assert "ALREADY\n    SHALLOW" in err
    assert "git fetch --no-tags --depth=1 origin main:refs/remotes/origin/main" in err


def test_a_clean_tree_is_green_and_prints_both_bindings(checker, tmp_path, capsys, monkeypatch):
    scenario(
        tmp_path,
        {GRAMMAR: GRAMMAR_AFTER_MIRROR, DSL: DSL_OK},
        {GRAMMAR: GRAMMAR_AFTER_MIRROR, DSL: DSL_OK},
    )
    assert go(checker, tmp_path, monkeypatch) == 0
    out = capsys.readouterr().out
    assert "grammar-program: 2 versions here (2 at origin/main), 1 anchors here" in out
    assert "dsl-campaign: 4 versions here (4 at origin/main), 3 anchors here" in out


# --- the second incident: one dsl number, two allocations -------------------
#
# The blind spot that used to be pinned here. `0.6.0` forces the anchor name
# `is_v06` in every branch that adds it, so two branches taking `0.6.0` for
# different surfaces produced the SAME anchor and rule 1 read one claim. Rule 6
# closes it by refusing an added number that rests on that derived anchor; rule 1
# then does the catching, because both branches carry a name.


def test_a_dsl_number_added_without_a_name_is_red(checker, tmp_path, capsys, monkeypatch):
    """Rule 6, and the shape the real branch had: `0.6.0` appended with `is_v06`
    and nothing else. Green under the old gate; a finding now, because the anchor
    it rests on is computed from the number and cannot disagree with a second
    branch's."""
    scenario(
        tmp_path,
        {GRAMMAR: GRAMMAR_AFTER_MIRROR, DSL: DSL_OK},
        {
            GRAMMAR: GRAMMAR_AFTER_MIRROR,
            DSL: dsl_ledger(DSL_VERSIONS + ["0.6.0"], {**DSL_PREDICATES, "is_v06": 6}),
        },
    )
    assert go(checker, tmp_path, monkeypatch) == 1
    err = capsys.readouterr().err
    assert "dsl-campaign: version 0.6.0 is added by this branch and nothing NAMES it" in err
    assert "Its only claim is ['is_v06'], which is computed from the number itself" in err
    assert "RESERVED_DSL_VERSIONS row" in err
    # Rule 3 is satisfied — `is_v06` DOES claim it — so this is rule 6 alone.
    assert "1 finding(s)" in err


def test_two_dsl_surfaces_on_one_number_are_red(checker, tmp_path, capsys, monkeypatch):
    """The incident itself, once rule 6 has forced both sides to name what they
    took: `open-way` and the horizon library each allocated `0.6.0`."""
    scenario(
        tmp_path,
        {
            GRAMMAR: GRAMMAR_AFTER_MIRROR,
            DSL: dsl_ledger(
                DSL_VERSIONS + ["0.6.0"],
                {**DSL_PREDICATES, "is_v06": 6},
                sinces={"HORIZON_LIBRARY_SINCE": "0.6.0"},
            ),
        },
        {
            GRAMMAR: GRAMMAR_AFTER_MIRROR,
            DSL: dsl_ledger(
                DSL_VERSIONS + ["0.6.0"],
                {**DSL_PREDICATES, "is_v06": 6},
                sinces={"OPEN_WAY_SINCE": "0.6.0"},
            ),
        },
    )
    assert go(checker, tmp_path, monkeypatch) == 1
    err = capsys.readouterr().err
    assert "dsl-campaign: version 0.6.0 is claimed by 2 different surfaces" in err
    assert "OPEN_WAY_SINCE  (this branch)" in err
    assert "HORIZON_LIBRARY_SINCE  (origin/main)" in err


def test_a_dsl_reservation_holds_a_number_for_a_surface_not_yet_written(
    checker, tmp_path, capsys, monkeypatch
):
    """What the ledger could not say at all before: a spec has taken `0.6.0` and
    nothing implements it. Held, named, and green."""
    scenario(
        tmp_path,
        {GRAMMAR: GRAMMAR_AFTER_MIRROR, DSL: DSL_OK},
        {
            GRAMMAR: GRAMMAR_AFTER_MIRROR,
            DSL: dsl_ledger(
                DSL_VERSIONS + ["0.6.0"], DSL_PREDICATES, reserved={"0.6.0": "OPEN_WAY_SINCE"}
            ),
        },
    )
    assert go(checker, tmp_path, monkeypatch) == 0
    out = capsys.readouterr().out
    assert "OK" in out
    # Binding counts: five entries here against four at base, four anchors (the
    # three predicates plus the reserved name), one reservation, and the one
    # added version rule 6 examined.
    assert (
        "dsl-campaign: 5 versions here (4 at origin/main), 4 anchors here "
        "(3 at origin/main), 1 reserved, 0 collision(s), 1 added version(s) "
        "examined for a name" in out
    )


def test_the_landed_surface_supersedes_its_own_reservation(
    checker, tmp_path, capsys, monkeypatch
):
    """The other end of the lifecycle. `0.6.0` is reserved for `OPEN_WAY_SINCE`
    at base; this branch defines the constant, adds `is_v06` and deletes the row.
    The number is spelled twice in one tree and that is ONE surface — a union
    would red on the change the reservation existed to admit."""
    scenario(
        tmp_path,
        {
            GRAMMAR: GRAMMAR_AFTER_MIRROR,
            DSL: dsl_ledger(
                DSL_VERSIONS + ["0.6.0"], DSL_PREDICATES, reserved={"0.6.0": "OPEN_WAY_SINCE"}
            ),
        },
        {
            GRAMMAR: GRAMMAR_AFTER_MIRROR,
            DSL: dsl_ledger(
                DSL_VERSIONS + ["0.6.0"],
                {**DSL_PREDICATES, "is_v06": 6},
                sinces={"OPEN_WAY_SINCE": "0.6.0"},
            ),
        },
    )
    assert go(checker, tmp_path, monkeypatch) == 0
    out = capsys.readouterr().out
    assert "0 collision(s)" in out
    # `0.6.0` is at base too, so rule 6 has nothing to examine — and says so.
    assert "0 added version(s) examined for a name" in out


def test_a_dsl_reservation_whose_surface_landed_is_red(checker, tmp_path, capsys, monkeypatch):
    """Rule 5 on the second ledger, naming the second ledger's own constant. The
    same rule, not a copy of it: one implementation reads both."""
    scenario(
        tmp_path,
        {GRAMMAR: GRAMMAR_AFTER_MIRROR, DSL: DSL_OK},
        {
            GRAMMAR: GRAMMAR_AFTER_MIRROR,
            DSL: dsl_ledger(
                DSL_VERSIONS + ["0.6.0"],
                {**DSL_PREDICATES, "is_v06": 6},
                sinces={"OPEN_WAY_SINCE": "0.6.0"},
                reserved={"0.6.0": "OPEN_WAY_SINCE"},
            ),
        },
    )
    assert go(checker, tmp_path, monkeypatch) == 1
    err = capsys.readouterr().err
    assert "reserved for OPEN_WAY_SINCE, and OPEN_WAY_SINCE is defined in this same tree" in err
    assert "delete the RESERVED_DSL_VERSIONS row" in err


# --- the live ledgers: the rules, held against the real files ---------------
#
# Every group above drives a synthetic ledger, and a rule that only ever meets a
# fixture is the UNRUN shape wearing a fixture's clothes. This group reads the
# REAL files, and reads EVERY one of them: the population comes from
# `checker.LEDGERS`, the script's own single authority over what a ledger is, so
# a third ledger is covered the day its row lands rather than the day someone
# remembers this file. Keying the live check to one ledger by name is the
# bespoke-field shape one layer out — and the ledger it happened to name is the
# one whose reservation list is empty.
#
# They are read through the gate's OWN extractors rather than a second set of
# regexes. A private copy of the reader answers a different question: it agrees
# with the gate only while both are maintained, and on the day the source shape
# drifts it is the copy that decides the verdict — which is the whole failure the
# `ShapeDrift` guards exist to make loud.
#
# What may NOT be asserted here is that a ledger reserves something.
#
# A reservation is born when a sibling change takes a number and dies in the edit
# that defines the constant it names, so an EMPTY list is the ordinary end of a
# reservation's life — a fact about what is in flight, never an invariant of the
# ledger. Demanding one reds exactly the change that correctly closes the last
# reservation, and the fix an author then reaches for is to invent a reservation
# for a number nothing has taken: manufacturing the artifact the gate asked for,
# which is the vacuity a gate exists to catch rather than a repair.
#
# It would also buy rule 6 nothing while it held. Rule 6's population is the
# versions a BRANCH ADDS against `--base`, which no state of the tree can supply:
# on a tree carrying a reservation the gate reports `0 added version(s) examined
# for a name` exactly as it does without one. Rule 6 is a property of a diff, so
# it is driven below by a diff — a base derived from the real file — and rule 5,
# whose population IS a property of the tree, is held against the real
# reservation rows.
#
# So emptiness is REPORTED, never demanded, and what is asserted is the
# complement: every number in every ledger names a surface.


def live_source(ledger) -> str:
    """One real ledger file, as it stands in this checkout."""
    return (REPO / ledger["path"]).read_text(encoding="utf-8")


def live_side(checker, ledger):
    """The gate's own reading of one real ledger file in this checkout."""
    return checker.read_ledger(ledger, live_source(ledger), "this checkout")


def summary_for(out: str, name: str) -> str:
    """The one ledger's slice of the gate's binding-count line."""
    return out.split(f"{name}: ", 1)[1].split(";", 1)[0].split(", diffed against", 1)[0]


def test_every_live_ledger_names_a_surface_for_every_number(checker):
    """Rule 3 and rule 5, held against the real ledgers rather than a fixture.

    The binding is stated per ledger, and a ledger that reserves nothing is
    named in a warning rather than passing silently — a zero binding is a
    finding, and the one thing it must never be is invisible.
    """
    bindings = []
    unheld = []
    numbers = 0
    for ledger in checker.LEDGERS:
        name = ledger["name"]
        side = live_side(checker, ledger)

        assert len(side.versions) >= 2, (
            f"{name}: {ledger['path']} parsed to {side.versions} — a live ledger names the "
            f"founding version and at least one fenced surface, so this is the extraction "
            f"drifting, not a ledger with nothing in it"
        )
        assert side.anchors, (
            f"{name}: not one version in {ledger['path']} could be traced to a fence anchor "
            f"via {ledger['claim_pattern']!r} — the rules below would examine nothing"
        )

        # Rule 3, on the real file: a number past the founding one that nothing
        # claims names nothing, and a number that names nothing is a number a
        # second change can take.
        for version in side.versions[1:]:
            assert side.claims.get(version), (
                f"{name}: {version} is in the ledger and no fence anchor claims it — give it "
                f"the anchor that introduces its surface, or reserve it for the anchor that "
                f"will"
            )

        # Rule 5, on the real file, plus the containment that makes a
        # reservation worth holding at all.
        for version, anchor in sorted(side.reserved.items()):
            assert version in side.versions, (
                f"{name}: {version} is reserved for {anchor} and is not in the ledger — a "
                f"number outside the ledger is a number the next author finds free"
            )
            assert anchor not in side.defined_anchor_names, (
                f"{name}: {version} is reserved for {anchor}, and {anchor} is defined in the "
                f"same file — the surface has landed, so the reservation refuses a version "
                f"this engine can honour"
            )

        numbers += len(side.versions) - 1
        if not side.reserved:
            unheld.append(name)
        bindings.append(
            f"{name}: {len(side.versions)} versions, {len(side.versions) - 1} claimed, "
            f"{len(side.anchors)} anchors, {len(side.reserved)} reserved"
        )

    assert numbers > 0, f"every live ledger examined zero numbers: {bindings}"
    print("live ledgers examined — " + "; ".join(bindings))

    if unheld:
        warnings.warn(
            f"rule 5 examined zero reservation rows on {', '.join(unheld)}. That is the "
            f"ordinary state of a ledger whose every number has landed, and it is a FINDING "
            f"only if some surface in flight has taken a number this ledger does not hold — "
            f"which is a question about the roadmap and cannot be settled from the file. "
            f"Binding — " + "; ".join(bindings),
            stacklevel=2,
        )


def one_version_younger(checker, ledger, source: str) -> tuple[str, str]:
    """The newest version, and the same ledger as it stood before that version.

    A coherent prior state, cut with the gate's own patterns rather than a
    hand-written slice: the number leaves the list, and so does everything that
    claimed it — its hand-written name, and on the campaign ledger the `ordinal`
    arm and the `is_vNN` predicate that name is computed from. Leave any behind
    and the base still claims the number under a DIFFERENT anchor, which is rule
    1's finding rather than rule 6's.
    """
    versions = checker.versions_of(ledger, source)
    newest = versions[-1]
    block = re.search(
        checker.LIST_RE_TEMPLATE.format(const=ledger["list_const"]), source, re.DOTALL
    )
    kept = ", ".join(f'"{v}"' for v in versions[:-1])
    prior = (
        source[: block.start()]
        + f"pub const {ledger['list_const']}: &[&str] = &[{kept}];"
        + source[block.end() :]
    )
    prior = without_the_name_of(checker, newest, prior)
    arms = checker.DSL_ORDINAL_ARM_RE.findall(prior)
    by_ordinal = {int(n): pred for pred, n in checker.DSL_PREDICATE_RE.findall(prior)}
    for version, n in arms:
        if version != newest:
            continue
        prior = re.sub(rf'\s*"{re.escape(version)}"\s*=>\s*{n}\s*,', "", prior)
        if int(n) in by_ordinal:
            prior = re.sub(
                rf"pub fn {by_ordinal[int(n)]}\(version: &str\) -> bool "
                rf"\{{\s*ordinal\(version\) >= {n}\s*\}}",
                "",
                prior,
            )
    return newest, prior


def without_the_name_of(checker, version: str, source: str) -> str:
    """The same source with every `*_SINCE` constant defined at `version` gone."""
    for name, at in checker.SINCE_CONST_RE.findall(source):
        if at == version:
            source = re.sub(rf'pub const {name}: &str = "{re.escape(at)}";\n', "", source)
    return source


@pytest.mark.parametrize("name", LEDGER_NAMES)
def test_rule_six_reads_the_real_ledger_shape(checker, tmp_path, capsys, monkeypatch, name):
    """Rule 6, driven against the real files rather than a fixture of them.

    Rule 6's population is a property of a DIFF, so nothing about the state of a
    ledger can make it bind — a tree carrying a reservation reports `0 added
    version(s) examined for a name` exactly as one without. What can bind it is a
    diff, and the one available without a network or a deep clone is the real
    file against itself one version younger.

    That is worth more than a synthetic diff for a second reason. Every fixture
    above is written in the shape the real ledgers have, and that shape is an
    assertion nothing checks — while the shape is precisely what the gate reads,
    and it has moved before: `rustfmt` broke the version list after the `=` the
    first time it outgrew one line. Here the real file IS the checkout, so a
    drift in it is a red rather than a fixture agreeing with a copy of itself.
    """
    ledger = next(row for row in checker.LEDGERS if row["name"] == name)
    real = {row["path"]: live_source(row) for row in checker.LEDGERS}
    here = real[ledger["path"]]
    newest, prior = one_version_younger(checker, ledger, here)

    scenario(tmp_path, {**real, ledger["path"]: prior}, real)
    assert go(checker, tmp_path, monkeypatch) == 0
    out = capsys.readouterr().out
    versions = checker.versions_of(ledger, here)
    summary = summary_for(out, name)
    assert f"{len(versions)} versions here ({len(versions) - 1} at origin/main)" in summary
    assert "1 added version(s) examined for a name" in summary

    # A green half alone is one-directional: it cannot separate a rule that
    # fires from a rule that is absent. So the same pair runs again with the
    # number's NAME taken away. It is still in the ledger and still claimed — on
    # the campaign ledger by the `is_vNN` computed from it — which is exactly the
    # claim rule 6 refuses from a version a branch has just added.
    stripped = without_the_name_of(checker, newest, here)
    assert stripped != here, (
        f"{name}: nothing NAMES {newest} in the real ledger, so this half of the "
        f"demonstration would pass by accident"
    )
    scenario(tmp_path, {**real, ledger["path"]: prior}, {**real, ledger["path"]: stripped})
    assert go(checker, tmp_path, monkeypatch) == 1
    err = capsys.readouterr().err
    assert f"{name}: version {newest} is added by this branch and nothing NAMES it" in err
    left = checker.read_ledger(ledger, stripped, "the stripped checkout")
    claim = sorted(left.derived.get(newest, set()))
    assert f"Its only claim is {claim or ['(nothing)']}" in err
    assert f"{ledger['reserved_const']} row" in err
