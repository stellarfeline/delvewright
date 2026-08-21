r"""Guards for `tools/lib/cargo-fingerprint-inputs.py`.

The red it exists to prevent is not a red at all, which is why it needed a guard:
`tools/worktree-new.sh` cloned a donor's `target/` into a new worktree, the
refusal compared only `rustc`, and a donor whose `target/` predated
`[profile.dev] debug = "line-tables-only"` passed it. Nothing errored. The clone
succeeded, every cloned unit was invalid, and the first build rebuilt all 140
packages — which is verbatim the symptom a previous round recorded and
MISDIAGNOSED, and it arrives wearing the costume of a regression in the clone
tool.

The tests below assert the comparison fails in the direction the defect actually
arrives from (a profile key moved on one side), that it does NOT fire on the
things it deliberately does not compare (a differing `Cargo.lock`), and that an
input it cannot ESTABLISH is a refusal rather than a pass — a checker that can
only ever pass is the vacuity mode this repo keeps shipping.

`rustc`/`cargo` are stubbed with a PATH shim rather than mocked, because the one
property that matters about them is that they are resolved with the TREE as the
working directory: rustup walks up from the CWD to find `rust-toolchain.toml` and
has no manifest flag, and that is the original defect in this area.
"""

from __future__ import annotations

import importlib.util
import json
import os
import subprocess
import sys
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[2]
TOOL = REPO / "tools" / "lib" / "cargo-fingerprint-inputs.py"

SHIM = """\
#!/bin/sh
# Answers with the CWD's own marker file, so a caller that resolves this tool
# from the wrong directory gets a different answer instead of the same one.
printf '%s 1.2.3\\n' "$(cat ./TOOLCHAIN 2>/dev/null || echo unpinned)"
"""


def _load():
    spec = importlib.util.spec_from_file_location("cargo_fingerprint_inputs", TOOL)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


mod = _load()


@pytest.fixture
def shimmed(tmp_path, monkeypatch):
    """Put `rustc` and `cargo` stubs on PATH that report the CWD's marker file."""
    binder = tmp_path / "bin"
    binder.mkdir()
    for tool in ("rustc", "cargo"):
        script = binder / tool
        script.write_text(SHIM)
        script.chmod(0o755)
    monkeypatch.setenv("PATH", f"{binder}{os.pathsep}{os.environ['PATH']}")
    return binder


def _tree(root: Path, *, profile: str, toolchain: str = "pinned", lock: str = "a") -> Path:
    root.mkdir(parents=True)
    (root / "Cargo.toml").write_text(
        '[package]\nname = "x"\nversion = "0.0.0"\n\n' + profile
    )
    (root / "Cargo.lock").write_text(lock)
    (root / "TOOLCHAIN").write_text(toolchain)
    return root


DEV_OLD = "[profile.dev]\nopt-level = 1\n"
DEV_NEW = '[profile.dev]\nopt-level = 1\ndebug = "line-tables-only"\n'


def test_identical_trees_agree(tmp_path, shimmed):
    a = _tree(tmp_path / "a", profile=DEV_NEW)
    b = _tree(tmp_path / "b", profile=DEV_NEW)
    code, lines = mod.diff(a, b)
    assert code == 0, lines
    # The count is stated, and it is stated against the whole input set.
    assert f"all {len(mod.INPUTS)} compared fingerprint inputs agree" in lines[0]


def test_a_moved_profile_key_is_refused(tmp_path, shimmed):
    """The live case: `debug = "line-tables-only"` landed after the donor built."""
    donor = _tree(tmp_path / "donor", profile=DEV_OLD)
    new = _tree(tmp_path / "new", profile=DEV_NEW)
    code, lines = mod.diff(donor, new)
    assert code == 1
    body = "\n".join(lines)
    assert "manifest_profiles" in body
    assert "line-tables-only" in body
    # It names both sides, so the reader does not have to go and look.
    assert "donor:" in body and "new  :" in body


def test_a_moved_toolchain_is_refused_and_resolved_from_the_tree(tmp_path, shimmed):
    """`rustc` is resolved with the TREE as CWD, not the caller's directory."""
    donor = _tree(tmp_path / "donor", profile=DEV_NEW, toolchain="1.97.1")
    new = _tree(tmp_path / "new", profile=DEV_NEW, toolchain="nightly")
    code, lines = mod.diff(donor, new)
    assert code == 1
    assert any(line.strip() == "rustc" for line in lines), lines


def test_ancestor_cargo_config_rustflags_is_refused(tmp_path, shimmed):
    """`build.rustflags` from an ANCESTOR directory is a fingerprint input."""
    donor = _tree(tmp_path / "plain" / "donor", profile=DEV_NEW)
    new = _tree(tmp_path / "flagged" / "new", profile=DEV_NEW)
    config = tmp_path / "flagged" / ".cargo"
    config.mkdir()
    (config / "config.toml").write_text('[build]\nrustflags = ["-C", "target-cpu=native"]\n')
    code, lines = mod.diff(donor, new)
    assert code == 1
    assert any(line.strip() == "config_build" for line in lines), lines


def test_the_same_ancestor_config_at_a_different_DEPTH_still_agrees(tmp_path, shimmed):
    """Two trees at different depths hold the same configuration.

    Depth is not a fingerprint input, and refusing on it would refuse on where
    the worktree happened to be put — which is every worktree.
    """
    shallow = _tree(tmp_path / "s" / "tree", profile=DEV_NEW)
    deep = _tree(tmp_path / "d" / "x" / "y" / "tree", profile=DEV_NEW)
    for base in (tmp_path / "s", tmp_path / "d"):
        cfg = base / ".cargo"
        cfg.mkdir()
        (cfg / "config.toml").write_text('[build]\nrustflags = ["-C", "target-cpu=native"]\n')
    code, lines = mod.diff(shallow, deep)
    assert code == 0, lines


def test_a_differing_lockfile_is_NOT_refused(tmp_path, shimmed):
    """Deliberately not compared: a branch that touched a dependency is the
    common cheap case, and refusing it would degrade the tool to `--no-clone`."""
    donor = _tree(tmp_path / "donor", profile=DEV_NEW, lock="one")
    new = _tree(tmp_path / "new", profile=DEV_NEW, lock="two")
    code, _ = mod.diff(donor, new)
    assert code == 0


def test_an_unestablished_input_refuses_rather_than_passes(tmp_path, shimmed):
    """No manifest at all: agreement cannot be established, so it fails CLOSED."""
    donor = _tree(tmp_path / "donor", profile=DEV_NEW)
    new = tmp_path / "new"
    new.mkdir()
    (new / "TOOLCHAIN").write_text("pinned")
    code, lines = mod.diff(donor, new)
    assert code == 2
    assert "UNESTABLISHED" in lines[0]


def test_a_missing_rustc_refuses_rather_than_comparing_two_unknowns(tmp_path, monkeypatch):
    """The shape the previous refusal had: `|| echo unknown` on BOTH sides made
    two failures compare equal, and the clone went ahead."""
    donor = _tree(tmp_path / "donor", profile=DEV_NEW)
    new = _tree(tmp_path / "new", profile=DEV_NEW)
    empty = tmp_path / "emptybin"
    empty.mkdir()
    monkeypatch.setenv("PATH", str(empty))
    code, lines = mod.diff(donor, new)
    assert code == 2, lines


def test_cli_json_and_diff_agree_with_the_library(tmp_path, shimmed):
    donor = _tree(tmp_path / "donor", profile=DEV_OLD)
    new = _tree(tmp_path / "new", profile=DEV_NEW)
    done = subprocess.run(
        [sys.executable, str(TOOL), "--json", str(donor)],
        capture_output=True,
        text=True,
        check=True,
    )
    assert json.loads(done.stdout)["manifest_profiles"] == {"dev": {"opt-level": 1}}
    done = subprocess.run(
        [sys.executable, str(TOOL), "--diff", str(donor), str(new)],
        capture_output=True,
        text=True,
        check=False,
    )
    assert done.returncode == 1
    assert "manifest_profiles" in done.stdout


def test_every_declared_input_is_actually_produced(tmp_path, shimmed):
    """INPUTS is what the count is stated against, so it may not drift from the
    collector: a key declared and never produced would inflate a stated number."""
    tree = _tree(tmp_path / "t", profile=DEV_NEW)
    produced = mod.inputs(tree)
    assert set(produced) == set(mod.INPUTS)
