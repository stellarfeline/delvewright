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

import json
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
    """A tree holding exactly the files the rows name, copied from the real one.

    It is a real git repository because the sweep's population is `git ls-files`
    — the derivation is the point, and a fixture that faked the population would
    be testing a different checker from the one CI runs.
    """
    for rel in TOUCHED:
        dst = tmp_path / rel
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(REPO / rel, dst)
    subprocess.run(["git", "-C", str(tmp_path), "init", "-q"], check=True)
    subprocess.run(["git", "-C", str(tmp_path), "add", "-A"], check=True)
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


# ---------------------------------------------------------------------------
# The fourth shape: a version literal nobody derived, regenerated or allowlisted
# ---------------------------------------------------------------------------
#
# The rows above say where the number is SUPPOSED to be, and for as long as that
# was all they said, the number was also in 446 other files — 319 of which
# changed nothing but that one string in a single bump. These tests perturb the
# tree TOWARD that shape and check the sweep reds, with a perturbation only this
# gate could catch: every other version gate in the tree is green on all of them.


def _dsl_version() -> str:
    return version_sites._declared(REPO)["dsl"]


def test_the_sweep_states_its_binding_with_a_denominator() -> None:
    r = run("verify", root=REPO)
    assert r.returncode == 0, r.stderr
    v = _dsl_version()
    assert f"tracked file(s) state `{v}`" in r.stdout
    population, carriers, shapes, findings = version_sites.sweep(REPO, v)
    assert not findings
    assert 0 < carriers < population, (carriers, population)
    # Not vacuous in the other direction either: the sweep must actually be
    # reaching the campaign documents, which are the population that grew.
    assert carriers > 100, carriers


def test_a_hand_typed_literal_in_a_rust_test_reds(scratch: Path) -> None:
    """The 45 `.rs` files of the bump: `quests_doc("0.23.0")` and its siblings."""
    assert run("verify", root=scratch).returncode == 0, "the scratch tree starts green"
    planted = scratch / "crates/delvec/tests/planted.rs"
    planted.parent.mkdir(parents=True, exist_ok=True)
    planted.write_text(f'fn doc() -> String {{ quests_doc("{_dsl_version()}") }}\n', encoding="utf-8")
    subprocess.run(["git", "-C", str(scratch), "add", "-A"], check=True)
    r = run("verify", root=scratch)
    assert r.returncode == 1, r.stdout
    assert "crates/delvec/tests/planted.rs" in r.stderr
    assert "none of the three legitimate shapes" in r.stderr


def test_a_number_in_a_json_document_that_is_not_its_own_envelope_reds(scratch: Path) -> None:
    """A JSON file may declare the surface it was written against — and nothing else.

    This is the distinction the naive reading of the rule would break: a campaign
    document's `dsl_version` is the document being self-describing (ADR-0024) and
    stays. A number sitting under any other key was typed by a person.
    """
    v = _dsl_version()
    good = scratch / "camp/world.json"
    good.parent.mkdir(parents=True, exist_ok=True)
    good.write_text(json.dumps({"dsl_version": v, "stage": "world"}), encoding="utf-8")
    subprocess.run(["git", "-C", str(scratch), "add", "-A"], check=True)
    assert run("verify", root=scratch).returncode == 0, "a self-describing document is legitimate"

    good.write_text(json.dumps({"dsl_version": v, "note": f"written against {v}"}), encoding="utf-8")
    subprocess.run(["git", "-C", str(scratch), "add", "-A"], check=True)
    r = run("verify", root=scratch)
    assert r.returncode == 1
    assert "camp/world.json" in r.stderr
    assert "typed by a person" in r.stderr


def test_an_extra_site_in_a_row_s_own_file_reds(scratch: Path) -> None:
    """A row is a census of its file, so a second mention in it is a finding too."""
    v = _dsl_version()
    manifest = scratch / "Cargo.toml"
    manifest.write_text(
        manifest.read_text(encoding="utf-8") + f"\n# see {v}\n", encoding="utf-8"
    )
    subprocess.run(["git", "-C", str(scratch), "add", "-A"], check=True)
    r = run("verify", root=scratch)
    assert r.returncode == 1
    assert "its row(s) account for" in r.stderr


def test_a_stale_allowlist_entry_is_reported(scratch: Path) -> None:
    """An allowlist that names nothing measures nothing — the sixth vacuity mode."""
    r = subprocess.run(
        [sys.executable, "-c",
         "import sys; sys.path.insert(0, %r); import version_sites as m;"
         "m.COUNTEREXAMPLES['docs/gone.md'] = 'a reason nobody can check';"
         "raise SystemExit(m.verify(__import__('pathlib').Path(%r)))"
         % (str(SITES.parent), str(scratch))],
        capture_output=True, text=True,
    )
    assert r.returncode == 1
    assert "docs/gone.md" in r.stderr


def test_the_blast_radius_is_the_hand_edited_set() -> None:
    """"What does a bump edit" is a number a person can check, not a claim."""
    hand, shapes = version_sites.blast_radius(REPO)
    assert hand, "no file is hand-edited, so the number came from nowhere"
    assert set(hand) == {f for s in version_sites.HAND for f in shapes.get(s, [])}
    r = run("blast-radius", "--count", root=REPO)
    assert r.returncode == 0 and int(r.stdout.strip()) == len(hand)
    # The authority is one of them, and it is the crate manifest (ADR-0024).
    assert "crates/dsl/Cargo.toml" in hand
