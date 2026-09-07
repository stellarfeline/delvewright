r"""Guard: a FAILING `tools/check-publishable.sh` run leaves no scratch tree.

`VERIFY_TARGET` (`package-verify`, the `CARGO_TARGET_DIR` check 1 points
`cargo package` at so it verifies siblings against a temporary registry rather
than `crates/*` on disk) sits BESIDE `target/`, not under it — deliberately,
per `tools/lib/package-verify.sh` — so `Swatinem/rust-cache`'s save/restore walk
for the calling job never sees it at all, regardless of what a run does to it.
A PASSING run keeps `package-verify/package/*.crate` (plus the sha256 written
beside each) for `tools/crates-io-publish.sh` to read — that half is
`test_publish_gate_order.py`'s job. A FAILING run — packaging itself failed, a
later check found something wrong, a mid-run crash — proved nothing, so it
leaves NOTHING: `$SUCCESS` stays 0, and `cleanup`'s `trap … EXIT`, registered
before either scratch variable is assigned, removes `VERIFY_TARGET` whole. The
same treatment `$SCRATCH` (check 3's standalone-build tree) already had on its
own, on every exit path alike.

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
LIB = REPO / "tools" / "lib"


@pytest.fixture
def preflight_tree_with_leftover_cargo(tmp_path: Path) -> tuple[Path, dict[str, str]]:
    """`check-publishable.sh` plus a `cargo` shim that writes real files into
    `$CARGO_TARGET_DIR/package/` before failing, so the tree cleanup is supposed
    to remove is never merely absent to begin with.
    """
    (tmp_path / "tools" / "lib").mkdir(parents=True)
    shutil.copy(PREFLIGHT, tmp_path / "tools" / "check-publishable.sh")
    shutil.copy(REPO / "versions.toml", tmp_path / "versions.toml")
    shutil.copy(LIB / "checksum.sh", tmp_path / "tools" / "lib" / "checksum.sh")
    shutil.copy(LIB / "package-verify.sh", tmp_path / "tools" / "lib" / "package-verify.sh")

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

    verify_target = tree / "package-verify"
    assert not verify_target.exists(), (
        f"{verify_target} survived a failing run with real evidence written "
        f"into it — the verify tree is scratch state the script owns end to "
        f"end, not build output a cache action should be walking"
    )
