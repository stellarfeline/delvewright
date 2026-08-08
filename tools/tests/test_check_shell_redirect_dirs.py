r"""Guards for the release-preflight bug that reported the wrong failure.

The v1.0.0 red (`crates.io preflight (no credential)`):

    tools/check-publishable.sh: line 79: .../target/package-log.txt: No such file
      FAIL cargo package failed:
    sed: can't read .../target/package-log.txt

`cargo package` had not failed — it had never run. The runner had no build cache,
so `target/` did not exist, and the shell could not open the redirect that was
supposed to capture the command's output. The general form:

    AN ERROR PATH MUST NOT DEPEND ON AN ARTIFACT THE ERROR MAY HAVE PREVENTED
    FROM EXISTING.

Two layers, both needed:

  * the functional tests below run the REAL `tools/check-publishable.sh` against a
    `cargo` that fails, and assert the report describes what actually happened.
    Syntax cannot check a message; only running it can.
  * the gate tests exercise `tools/check-shell-redirect-dirs.py`, which removes
    the root cause repo-wide by requiring every redirect's directory to be
    guaranteed before the redirect is opened.
"""

from __future__ import annotations

import os
import shutil
import stat
import subprocess
import sys
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[2]
CHECKER = REPO / "tools" / "check-shell-redirect-dirs.py"
PREFLIGHT = REPO / "tools" / "check-publishable.sh"

FIXTURE_DRIVER = """
import importlib.util, pathlib, sys
spec = importlib.util.spec_from_file_location("chk", sys.argv[1])
mod = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mod)
mod.REPO = pathlib.Path(sys.argv[2])
repo_dirs = mod.tracked_dirs()
n = 0
for rel in mod.shell_files():
    findings, _ = mod.check_file(rel, (mod.REPO / rel).read_text(encoding="utf-8"), repo_dirs)
    for f in findings:
        print(f)
        n += 1
sys.exit(1 if n else 0)
"""


@pytest.fixture
def preflight_tree(tmp_path: Path) -> tuple[Path, dict[str, str]]:
    """A minimal tree `check-publishable.sh` will run in, with `cargo` shimmed.

    Deliberately has NO `target/` directory — that is the runner state the
    release hit, and the state the script must survive.
    """
    (tmp_path / "tools").mkdir()
    shutil.copy(PREFLIGHT, tmp_path / "tools" / "check-publishable.sh")
    shutil.copy(REPO / "versions.toml", tmp_path / "versions.toml")

    binpath = tmp_path / "bin"
    binpath.mkdir()
    cargo = binpath / "cargo"
    # Fails, and writes nothing at all — so the log it was supposed to fill is
    # empty, and the report must say that rather than print nothing and imply it
    # read something.
    cargo.write_text("#!/bin/sh\nexit 101\n", encoding="utf-8")
    cargo.chmod(0o755)

    env = {**os.environ, "PATH": f"{binpath}:{os.environ['PATH']}"}
    return tmp_path, env


def run_preflight(tree: Path, env: dict[str, str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["bash", str(tree / "tools" / "check-publishable.sh"), "--allow-dirty"],
        cwd=tree,
        capture_output=True,
        text=True,
        env=env,
    )


# --------------------------------------------------------------- the behaviour


def test_no_target_dir_does_not_make_the_report_lie(preflight_tree):
    """The motivating red: no `target/`, and the script blamed `cargo package`.

    Before the fix this run printed `line 79: .../package-log.txt: No such file`,
    then `FAIL cargo package failed:`, then `sed: can't read ...` — three lines,
    none of them true. The directory is now created before the redirect, so
    `cargo` actually runs and the exit status reported is `cargo`'s own.
    """
    tree, env = preflight_tree
    assert not (tree / "target").exists(), "fixture must start with no build cache"

    proc = run_preflight(tree, env)

    assert proc.returncode == 1
    combined = proc.stdout + proc.stderr
    assert "No such file or directory" not in combined, combined
    assert "sed:" not in combined, combined
    # The shim exits 101; an honest report names that, not a bare "failed".
    assert "cargo package exited 101" in combined, combined
    assert (tree / "target" / "package-log.txt").exists()


def test_an_empty_log_is_named_as_empty(preflight_tree):
    """A command that ran and printed nothing must not look like a printed log."""
    tree, env = preflight_tree
    proc = run_preflight(tree, env)
    combined = proc.stdout + proc.stderr
    assert "is empty" in combined, combined
    assert "ran and wrote nothing" in combined, combined


@pytest.mark.skipif(os.geteuid() == 0, reason="root ignores directory permissions")
def test_a_log_that_could_not_be_opened_says_the_command_did_not_run(preflight_tree):
    """The other half of the general form, exercised rather than asserted.

    If the redirect still cannot be opened for some other reason, the report must
    say the command DID NOT RUN — never quote a log it does not have.
    """
    tree, env = preflight_tree
    target = tree / "target"
    target.mkdir()
    target.chmod(stat.S_IRUSR | stat.S_IXUSR)  # r-x: mkdir -p is a no-op, `>` fails
    try:
        proc = run_preflight(tree, env)
        combined = proc.stdout + proc.stderr
    finally:
        target.chmod(0o755)
    assert "DID NOT RUN" in combined, combined
    assert "sed:" not in combined, combined


# ------------------------------------------------------------------- the gate


def test_repo_is_clean():
    proc = subprocess.run(
        [sys.executable, str(CHECKER)], cwd=REPO, capture_output=True, text=True
    )
    assert proc.returncode == 0, proc.stderr
    n = int(proc.stdout.split("OK — ", 1)[1].split(" ", 1)[0])
    # Binding count (CLAUDE.md): a gate that examined no redirects is not a pass.
    assert n >= 5, f"only {n} directory-writing redirect(s) examined"


def _fixture_repo(tmp_path: Path, body: str) -> Path:
    subprocess.run(["git", "init", "-q"], cwd=tmp_path, check=True)
    (tmp_path / "s.sh").write_text(body, encoding="utf-8")
    subprocess.run(["git", "add", "-A"], cwd=tmp_path, check=True)
    return tmp_path


@pytest.mark.parametrize(
    "label,body,expect_finding",
    [
        # The release bug, reduced.
        ("bare redirect into an uncreated dir", 'cargo x >"$ROOT/target/log.txt"\n', True),
        (
            "directory created first",
            'mkdir -p "$ROOT/target"\ncargo x >"$ROOT/target/log.txt"\n',
            False,
        ),
        # Hoisting the path into a variable must not hide it.
        (
            "path hoisted into a variable",
            'LOG="$ROOT/target/log.txt"\ncargo x >"$LOG"\n',
            True,
        ),
        (
            "path hoisted, directory created",
            'LOG="$ROOT/target/log.txt"\nmkdir -p "$ROOT/target"\ncargo x >"$LOG"\n',
            False,
        ),
        # `mkdir -p a/b` creates `a` too; plain `mkdir a` does not create `a/b`.
        ("mkdir -p of a descendant", 'mkdir -p "$S/w/d"\nx > "$S/f"\n', False),
        ("mkdir without -p of an ancestor", 'mkdir "$S"\nx > "$S/w/f"\n', True),
        ("mkdir without -p of the exact dir", 'mkdir "$S"\nx > "$S/f"\n', False),
        ("mktemp -d", 'D="$(mktemp -d)"\nx > "$D/f"\n', False),
        # A `>` inside a quoted string is text, not a redirection.
        ("arrow inside a double-quoted argument", 'want "a -> b/c.json" "$V"\n', False),
        ("arrow inside a single-quoted awk program", "awk '/cat > \\/d\\/e/{f=1}' x\n", False),
        # Streams and always-present directories.
        ("stderr dup", "echo hi >&2\n", False),
        ("dev null", "cmd >/dev/null 2>&1\n", False),
        ("tmp", "cmd > /tmp/have.txt\n", False),
        ("no directory component", "cmd > out.txt\n", False),
    ],
)
def test_gate_verdicts(tmp_path, label, body, expect_finding):
    repo = _fixture_repo(tmp_path, body)
    proc = subprocess.run(
        [sys.executable, "-c", FIXTURE_DRIVER, str(CHECKER), str(repo)],
        capture_output=True,
        text=True,
    )
    assert (proc.returncode == 1) == expect_finding, (
        f"{label}: expected finding={expect_finding}\n"
        f"stdout={proc.stdout}\nstderr={proc.stderr}"
    )
