r"""Guard: `tools/check-publishable.sh` leaves no scratch tree behind.

`VERIFY_TARGET` (`target/package-verify`, the `CARGO_TARGET_DIR` check 1 points
`cargo package` at so it verifies siblings against a temporary registry rather
than `crates/*` on disk) sits inside the tree `Swatinem/rust-cache` walks when
saving and restoring its cache for the calling job (`rust (fmt, clippy, test)`,
`ci.yml`'s default `workspaces: .`). Left behind, its extracted package trees —
each carrying the packaged crate's own `tests/` fixtures — are the dangling
paths that action was observed printing `ENOENT opendir …/tests/target` for: a
build-VERIFICATION tree is not build output worth caching, and check 0 already
purges same-named leftovers from the cargo registry cache for the identical
reason. The fix tears `VERIFY_TARGET` down on every exit path — a full pass, an
early `fail`-and-exit, or a mid-run crash — via one `trap … EXIT` registered
before either scratch variable is assigned, the same treatment `$SCRATCH`
(check 3's standalone-build tree) already had on its own.

This is a REAL run of the script, with only `cargo` shimmed to leave real
evidence under `$CARGO_TARGET_DIR/package/` before failing — a green here must
not be "there was never anything to clean up". Companion to
`test_check_shell_redirect_dirs.py`, which shims `cargo` to fail with NO
evidence at all and guards a different bug in the same script.
"""

from __future__ import annotations

import os
import shutil
import subprocess
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[2]
PREFLIGHT = REPO / "tools" / "check-publishable.sh"


@pytest.fixture
def preflight_tree_with_leftover_cargo(tmp_path: Path) -> tuple[Path, dict[str, str]]:
    """`check-publishable.sh` plus a `cargo` shim that writes real files into
    `$CARGO_TARGET_DIR/package/` before failing, so the tree cleanup is supposed
    to remove is never merely absent to begin with.
    """
    (tmp_path / "tools").mkdir()
    shutil.copy(PREFLIGHT, tmp_path / "tools" / "check-publishable.sh")
    shutil.copy(REPO / "versions.toml", tmp_path / "versions.toml")

    binpath = tmp_path / "bin"
    binpath.mkdir()
    cargo = binpath / "cargo"
    cargo.write_text(
        "#!/bin/sh\n"
        'if [ "$1" = package ]; then\n'
        '  mkdir -p "$CARGO_TARGET_DIR/package/marker"\n'
        '  echo evidence > "$CARGO_TARGET_DIR/package/marker/f"\n'
        "fi\n"
        "exit 101\n",
        encoding="utf-8",
    )
    cargo.chmod(0o755)

    env = {**os.environ, "PATH": f"{binpath}:{os.environ['PATH']}"}
    return tmp_path, env


def test_the_package_verify_tree_does_not_survive_a_failing_run(
    preflight_tree_with_leftover_cargo: tuple[Path, dict[str, str]],
) -> None:
    tree, env = preflight_tree_with_leftover_cargo
    proc = subprocess.run(
        ["bash", str(tree / "tools" / "check-publishable.sh"), "--allow-dirty"],
        cwd=tree,
        capture_output=True,
        text=True,
        env=env,
    )
    assert proc.returncode == 1
    combined = proc.stdout + proc.stderr
    assert "cargo package exited 101" in combined, combined

    verify_target = tree / "target" / "package-verify"
    assert not verify_target.exists(), (
        f"{verify_target} survived a failing run with real evidence written "
        f"into it — the verify tree is scratch state the script owns end to "
        f"end, not build output a cache action should be walking"
    )
