r"""Guards for the CRLF bug that broke the v1.0.0 shelf on the msvc runner alone.

Two layers, and both are needed:

  * `test_release_shelf_*` runs the REAL `tools/build-release-binaries.sh` under an
    interpreter whose stdout translates `\n` to `\r\n` — i.e. a Windows python,
    faithfully simulated — and asserts the target list comes back clean. That is
    the behaviour, and it would have caught the release failure from a mac.
  * the rest exercise `tools/check-python-shell-newlines.py` itself, so the gate
    that keeps the fix in place cannot silently stop matching.

The simulation is a `sitecustomize.py` on `PYTHONPATH` doing
`sys.stdout.reconfigure(newline="\r\n")`. That is exactly what a Windows
interpreter's text-mode stdout does, and — importantly — the fix under test
overrides it, because the script's own `reconfigure(newline="\n")` runs after
site initialisation. So the shim reproduces the PLATFORM, not the bug: a script
without the fix reds under it, a script with the fix greens.
"""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[2]
CHECKER = REPO / "tools" / "check-python-shell-newlines.py"
SHELF = REPO / "tools" / "build-release-binaries.sh"

# Import the checker with its repo root rebound, and report only whether it
# MATCHED — deliberately bypassing main()'s vacuity guard, which is about the
# real repo and would drown out a fixture that legitimately contains no python.
FIXTURE_DRIVER = """
import importlib.util, pathlib, sys
spec = importlib.util.spec_from_file_location("chk", sys.argv[1])
mod = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mod)
mod.REPO = pathlib.Path(sys.argv[2])
n = 0
for rel in mod.scan_files():
    lines = (mod.REPO / rel).read_text(encoding="utf-8").splitlines()
    for lineno, body in mod.programs(rel, lines):
        if mod.WRITES_STDOUT.search(body) and not mod.GUARD.search(body):
            print(f"{rel}:{lineno}")
            n += 1
sys.exit(1 if n else 0)
"""


@pytest.fixture(scope="module")
def win_env(tmp_path_factory) -> dict[str, str]:
    """A process env whose python3 writes `\\r\\n` for `\\n`, as Windows does."""
    d = tmp_path_factory.mktemp("winsim")
    (d / "sitecustomize.py").write_text(
        'import sys\nsys.stdout.reconfigure(newline="\\r\\n")\n', encoding="utf-8"
    )
    return {**os.environ, "PYTHONPATH": str(d)}


# --------------------------------------------------------------- the behaviour


def test_the_windows_simulation_is_faithful(win_env):
    """If the shim does not actually emit CRLF, every test below is vacuous."""
    out = subprocess.run(
        [sys.executable, "-c", "print('x')"], capture_output=True, env=win_env
    ).stdout
    assert out == b"x\r\n", out


def test_shelf_target_list_survives_a_windows_interpreter(win_env):
    """The v1.0.0 red: every target arrived carrying a trailing `\\r` on msvc.

    Without the fix this is `b'...-msvc\\r\\n'`, and `build_one()`'s membership
    test then rejects a triple that IS in versions.toml — which is the exact
    message the release run printed.
    """
    lf = subprocess.run(
        ["bash", str(SHELF), "--list-targets"], cwd=REPO, capture_output=True, check=True
    ).stdout
    crlf = subprocess.run(
        ["bash", str(SHELF), "--list-targets"],
        cwd=REPO,
        capture_output=True,
        env=win_env,
        check=True,
    ).stdout
    assert lf, "vacuous: the shelf listed no targets at all"
    assert b"\r" not in crlf, f"CRLF leaked into the shelf target list: {crlf!r}"
    assert crlf == lf, "the shelf target list depends on the interpreter's platform"
    assert b"windows" in lf, "vacuous: no windows target in the shelf to protect"


def test_shelf_membership_check_is_still_live():
    """The guarded behaviour only means something while the check exists."""
    proc = subprocess.run(
        ["bash", str(SHELF), "--target", "not-a-real-triple"],
        cwd=REPO,
        capture_output=True,
        text=True,
    )
    assert proc.returncode == 1
    assert "is not in versions.toml" in proc.stderr, proc.stderr


# ------------------------------------------------------------------- the gate


def test_repo_is_clean():
    proc = subprocess.run(
        [sys.executable, str(CHECKER)], cwd=REPO, capture_output=True, text=True
    )
    assert proc.returncode == 0, proc.stderr
    n = int(proc.stdout.split("OK — ", 1)[1].split(" ", 1)[0])
    # Binding count, so the gate cannot go green by scanning nothing (CLAUDE.md).
    assert n >= 10, f"only {n} inline python program(s) examined — did the scan break?"


def _fixture_repo(tmp_path: Path, body: str) -> Path:
    subprocess.run(["git", "init", "-q"], cwd=tmp_path, check=True)
    (tmp_path / "s.sh").write_text(body, encoding="utf-8")
    subprocess.run(["git", "add", "-A"], cwd=tmp_path, check=True)
    return tmp_path


@pytest.mark.parametrize(
    "label,body,expect_finding",
    [
        # The exact shape that broke the release: a heredoc inside a shell
        # FUNCTION, captured only at the call sites — no redirect, no pipe and no
        # `$(` anywhere near the invocation. A checker that reasoned about the
        # invocation site would pass this.
        (
            "heredoc in a function",
            "read_it() {\n  python3 - <<'PY'\nimport tomllib\nprint(1)\nPY\n}\n",
            True,
        ),
        (
            "heredoc in a function, pinned",
            "read_it() {\n  python3 - <<'PY'\nimport sys\n"
            'sys.stdout.reconfigure(newline="\\n")\nprint(1)\nPY\n}\n',
            False,
        ),
        (
            "-c captured into a variable",
            "V=\"$(python3 -c 'import tomllib;print(2)')\"\n",
            True,
        ),
        (
            "-c captured into a variable, pinned",
            'V="$(python3 -c \'import sys;sys.stdout.reconfigure(newline="\\n");print(2)\')"\n',
            False,
        ),
        # Communicates by exit status: no stdout, so no newline to get wrong.
        ("exit-status only", "python3 -c 'import sys;sys.exit(0)'\n", False),
        # A committed .py file is python all the way down, not a shell boundary.
        ("a committed script", "python3 tools/thing.py\n", False),
        # Inside a container the interpreter is a pinned Linux one by construction
        # — and the continuation must be joined for the rule to see the `docker`.
        (
            "inside a container, across a continuation",
            'x="$(docker run --rm img -c \\\n  \'python3 -c "print(1)"\')"\n',
            False,
        ),
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
