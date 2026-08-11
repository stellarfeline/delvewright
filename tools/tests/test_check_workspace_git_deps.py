r"""Guards for `tools/check-workspace-git-deps.py`.

The red it exists to prevent (PR #388, a docs-only change, in the REQUIRED
`tier 2 (datapack load + PackTest)` job, in a step named for a datapack):

    build the hello-world delve output
      Updating git repository `https://github.com/Schem-at/Nucleation`
      error: failed to get `schematic-mesher` as a dependency of package
             `nucleation v0.9.1` ... which satisfies git dependency `nucleation`
             of package `delvewright-render v0.0.0`
      network failure seems to have happened
      the SSL certificate is invalid; class=Ssl (16)

`cargo run -p delvec` does not build `delvewright-render`. It cloned that crate's
git dependency anyway, because a git dependency is loaded during RESOLUTION of the
workspace that declares it and a workspace resolves all of its members. The
general form:

    A REQUIRED STATUS CHECK MUST NOT REACH FOR A HOST IT DOES NOT NEED.

The tests below assert the gate fails in the direction the defect actually
arrives from (a git source in a lock that is not quarantined) and that a stale
exemption is a finding too — a checker that only ever passes is the vacuity mode
this repo keeps shipping.
"""

from __future__ import annotations

import importlib.util
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[2]
CHECKER = REPO / "tools" / "check-workspace-git-deps.py"

REGISTRY_PKG = """\
[[package]]
name = "serde"
version = "1.0.229"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "0000000000000000000000000000000000000000000000000000000000000000"
"""

GIT_PKG = """\
[[package]]
name = "nucleation"
version = "0.9.1"
source = "git+https://github.com/Schem-at/Nucleation?rev=dbc8fe02#dbc8fe02"
dependencies = [
 "schematic-mesher",
]
"""

LOCAL_PKG = """\
[[package]]
name = "delvec"
version = "1.1.0"
dependencies = [
 "serde",
]
"""


@pytest.fixture
def checker():
    """The gate, loaded fresh so `ROOT`/`ALLOWED` can be pointed at a fixture."""
    spec = importlib.util.spec_from_file_location("cwgd", CHECKER)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def write_lock(root: Path, rel: str, *bodies: str) -> None:
    path = root / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text('version = 4\n\n' + "\n".join(bodies), encoding="utf-8")


def test_registry_only_workspace_passes(checker, tmp_path, capsys):
    write_lock(tmp_path, "Cargo.lock", LOCAL_PKG, REGISTRY_PKG)
    checker.ROOT = tmp_path
    checker.ALLOWED = {}
    assert checker.main() == 0
    out = capsys.readouterr().out
    assert "OK" in out
    # Binding count, always printed (CLAUDE.md: a green gate that binds to
    # nothing is VACUOUS).
    assert "1 Cargo.lock examined" in out


def test_git_dependency_in_the_root_lock_is_a_finding(checker, tmp_path, capsys):
    """The #388 shape: the root workspace resolves a git source."""
    write_lock(tmp_path, "Cargo.lock", LOCAL_PKG, REGISTRY_PKG, GIT_PKG)
    checker.ROOT = tmp_path
    checker.ALLOWED = {}
    assert checker.main() == 1
    err = capsys.readouterr().err
    assert "Cargo.lock resolves 1 git dependency" in err
    assert "nucleation 0.9.1" in err
    # The message has to say what to DO, or the next author re-derives it.
    assert "its own workspace" in err


def test_a_quarantined_lock_may_carry_one(checker, tmp_path, capsys):
    write_lock(tmp_path, "Cargo.lock", LOCAL_PKG, REGISTRY_PKG)
    write_lock(tmp_path, "crates/render/Cargo.lock", GIT_PKG)
    checker.ROOT = tmp_path
    checker.ALLOWED = {"crates/render/Cargo.lock": "the render layer pins it by rev"}
    assert checker.main() == 0
    assert "2 Cargo.lock examined, 1 allowlisted" in capsys.readouterr().out


def test_an_exemption_that_outlived_its_reason_is_a_finding(checker, tmp_path, capsys):
    """An allowlist entry nothing needs is how the next one gets waved through."""
    write_lock(tmp_path, "Cargo.lock", LOCAL_PKG)
    write_lock(tmp_path, "crates/render/Cargo.lock", REGISTRY_PKG)
    checker.ROOT = tmp_path
    checker.ALLOWED = {"crates/render/Cargo.lock": "the render layer pins it by rev"}
    assert checker.main() == 1
    assert "outlived its reason" in capsys.readouterr().err


def test_examining_no_locks_is_a_finding(checker, tmp_path, capsys):
    """A gate that matched zero objects is not a pass."""
    checker.ROOT = tmp_path
    checker.ALLOWED = {}
    assert checker.main() == 1
    assert "examined 0 Cargo.lock" in capsys.readouterr().err


def test_symlinked_trees_are_never_followed(checker, tmp_path, capsys):
    """`campaigns/` is a symlink to the CONTENT repo on a dev machine."""
    outside = tmp_path / "outside"
    outside.mkdir()
    write_lock(outside, "Cargo.lock", GIT_PKG)
    repo = tmp_path / "repo"
    repo.mkdir()
    write_lock(repo, "Cargo.lock", LOCAL_PKG)
    (repo / "campaigns").symlink_to(outside, target_is_directory=True)
    checker.ROOT = repo
    checker.ALLOWED = {}
    assert checker.main() == 0
    assert "1 Cargo.lock examined" in capsys.readouterr().out


def test_the_real_repo_is_clean(capsys):
    """The gate, unpatched, against this repo — the state the PR leaves behind."""
    spec = importlib.util.spec_from_file_location("cwgd_real", CHECKER)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    assert mod.main() == 0
    assert (REPO / "crates" / "render" / "Cargo.lock") in mod.locks()
