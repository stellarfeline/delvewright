r"""Guard: the refusal's site list names places this tree actually has.

`tools/crates-io-publish.sh` refuses a version that is on crates.io with
different bytes, and everything useful about that refusal is the list it prints
next — where to move the number to. That list was a literal beside the `echo`s
and nothing read it, so it rotted in three places at once, all three measured
against the tree rather than suspected:

  * it named `SUPPORTED_DSL_VERSION`; the constant is `DSL_VERSION`, and
    `validation/check-versions.sh` has matched `pub const DSL_VERSION` for as
    long as it has existed;
  * it said *all four of these carry it* over a set of seven, missing the root
    `Cargo.toml` `[workspace.dependencies]` pin — and a reader who follows the
    message exactly then gets `failed to select a version for the requirement
    delvewright-dsl = "=<old>"` from the next `--locked` build, which is what
    happened to the round that moved 0.22.0 to 0.22.1;
  * the non-DSL branch told a reader bumping the ENGINE version to move
    `[workspace.dependencies]` too, a table that holds one entry
    (`delvewright-dsl`) and no engine number at all.

`tools/lib/version_sites.py` now owns the rows, prints them, and resolves every
one against the tree on every run of the script. RED WITHOUT IT: each
perturbation below is one of the three defects put back, and each reds only
because the rows are resolved — the printed prose would be identical.
"""

from __future__ import annotations

import shutil
import subprocess
import sys
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[2]
SITES = REPO / "tools" / "lib" / "version_sites.py"

# The files any row can name. Derived from the rows themselves, so a row added
# later cannot quietly fall outside the scratch tree and pass by not being there.
sys.path.insert(0, str(SITES.parent))
import version_sites  # noqa: E402

TOUCHED = sorted({str(r["path"]) for rows in version_sites.ROWS.values() for r in rows})


def run(*args: str, root: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(SITES), *args, "--root", str(root)],
        capture_output=True,
        text=True,
    )


@pytest.fixture()
def scratch(tmp_path: Path) -> Path:
    """A tree holding exactly the files the rows name, copied from the real one."""
    for rel in TOUCHED:
        dst = tmp_path / rel
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(REPO / rel, dst)
    return tmp_path


def test_every_row_resolves_against_the_real_tree() -> None:
    r = run("verify", root=REPO)
    assert r.returncode == 0, r.stderr
    # The binding count, stated and non-zero: a verify that resolved nothing
    # would print the same "OK" shape.
    total = sum(len(v) for v in version_sites.ROWS.values())
    assert total >= 10
    assert f"{total} row(s) resolved" in r.stdout


def test_the_scratch_tree_is_a_faithful_subject() -> None:
    # Without this the perturbations below could pass by accident, on a tree the
    # checker was refusing for some unrelated reason.
    assert TOUCHED, "the rows name no files at all"


def test_a_renamed_constant_reds_instead_of_being_printed(scratch: Path) -> None:
    """Defect 1: the message named `SUPPORTED_DSL_VERSION`."""
    assert run("verify", root=scratch).returncode == 0, "the scratch tree starts green"
    env = scratch / "crates/dsl/src/envelope.rs"
    env.write_text(
        env.read_text(encoding="utf-8").replace("DSL_VERSION", "SUPPORTED_DSL_VERSION"),
        encoding="utf-8",
    )
    r = run("verify", root=scratch)
    assert r.returncode == 1
    assert "crates/dsl/src/envelope.rs" in r.stderr
    assert "the symbol the message names is not there" in r.stderr


def test_a_site_that_stopped_carrying_the_number_reds(scratch: Path) -> None:
    """Defect 2: the `[workspace.dependencies]` pin, whose omission fails `--locked`."""
    root_manifest = scratch / "Cargo.toml"
    text = root_manifest.read_text(encoding="utf-8")
    assert 'path = "crates/dsl"' in text
    # The number comes from the tree, never from a literal here. Spelling it out
    # made this perturbation silently stop perturbing the moment the number
    # moved: the replace matched nothing, the gate stayed green because nothing
    # was broken, and the row read as a red gate rather than as an inert test.
    # The match count is asserted for the same reason.
    pin = f'version = "={version_sites._declared(scratch)["dsl"]}" }}'
    assert text.count(pin) == 1, f"the workspace pin `{pin}` is the thing this row breaks"
    root_manifest.write_text(
        text.replace(pin, 'version = "=0.0.0" }'), encoding="utf-8"
    )
    r = run("verify", root=scratch)
    assert r.returncode == 1
    assert "[workspace.dependencies]" in r.stderr


def test_a_row_whose_file_is_gone_reds(scratch: Path) -> None:
    (scratch / "prefabs/Cargo.lock").unlink()
    r = run("verify", root=scratch)
    assert r.returncode == 1
    assert "no such file: prefabs/Cargo.lock" in r.stderr


@pytest.mark.parametrize("kind", sorted(version_sites.ROWS))
def test_the_count_in_the_sentence_is_the_list_s_length(kind: str) -> None:
    """The defect that made "all four" outlive a fifth site: a number typed beside a list."""
    listed = run("advise", "--kind", kind, "--version", "9.9.9", root=REPO)
    counted = run("advise", "--kind", kind, "--version", "9.9.9", "--count", root=REPO)
    assert listed.returncode == 0 and counted.returncode == 0
    lines = [ln for ln in listed.stdout.splitlines() if ln.strip()]
    assert int(counted.stdout.strip()) == len(lines) == len(version_sites.ROWS[kind])


@pytest.mark.parametrize("kind", sorted(version_sites.ROWS))
def test_every_path_the_advice_prints_exists(kind: str) -> None:
    """One object is printed and resolved; this is the bind between the two uses."""
    out = run("advise", "--kind", kind, "--version", "9.9.9", root=REPO).stdout
    paths = [str(r["path"]) for r in version_sites.ROWS[kind]]
    assert paths, f"{kind} names no sites"
    for p in paths:
        assert p in out
        assert (REPO / p).is_file(), p


def test_the_engine_advice_does_not_name_the_dependency_table() -> None:
    """Defect 3, pinned so it cannot come back.

    Nothing in the workspace depends on the engine crate, so
    `[workspace.dependencies]` carries no engine number — the old message sent a
    reader to a table that holds one entry and none of what they were looking for.
    """
    labels = " ".join(str(r["label"]) for r in version_sites.ROWS["engine"])
    assert "workspace.dependencies" not in labels
    # …and the check is not vacuous: the DSL number really does live there, which
    # is why the table is a legitimate site for one number and not for the other.
    dsl_labels = " ".join(str(r["label"]) for r in version_sites.ROWS["dsl"])
    assert "[workspace.dependencies]" in dsl_labels


def test_the_script_refuses_to_plan_behind_advice_it_cannot_resolve() -> None:
    """The check is bound to the SCRIPT, not only to the failure path it decorates."""
    src = (REPO / "tools/crates-io-publish.sh").read_text(encoding="utf-8")
    assert "version_sites verify" in src
    assert "refusing to plan behind advice" in src
    # It runs before the plan, beside the index bind test — not inside the `FAIL`
    # arm, which almost never executes.
    assert src.index("version_sites verify") < src.index("== what crates.io already holds ==")
