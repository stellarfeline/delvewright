"""`dw_rcon` and `dw_rcon_probe` do not consume their caller's stdin.

The defect: both ran `docker exec -i`, which forwards the caller's stdin to the
command and reads it to the end. A caller looping `while read …; do dw_rcon …;
done <file` lost every line after the first to the first rcon call — the
checkpoint watcher's quest loop is one such caller.

The fake `docker` below reads its stdin the way `docker exec -i` does, so the
loop reds the moment either call stops reading /dev/null.
"""

import os
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
RCON = ROOT / "tools" / "lib" / "rcon.sh"

FAKE = """#!/usr/bin/env bash
cat >/dev/null
echo "Test passed"
"""


def loop(tmp_path, fn):
    bindir = tmp_path / "bin"
    bindir.mkdir(exist_ok=True)
    docker = bindir / "docker"
    docker.write_text(FAKE, encoding="utf-8")
    docker.chmod(0o755)
    lines = tmp_path / "lines"
    lines.write_text("a\nb\nc\n", encoding="utf-8")
    script = f'. "{RCON}"; n=0; while read -r x; do {fn} srv "say $x" >/dev/null; n=$((n + 1)); done <"{lines}"; echo "$n"'
    env = {"PATH": f"{bindir}{os.pathsep}{pathlib.Path(sys.executable).parent}:/usr/bin:/bin"}
    return subprocess.run(["bash", "-c", script], env=env, capture_output=True, text=True)


def test_a_read_loop_around_dw_rcon_sees_every_line(tmp_path):
    r = loop(tmp_path, "dw_rcon")
    assert r.returncode == 0, r.stderr
    assert r.stdout.strip() == "3", f"the loop ran {r.stdout.strip()} time(s) over 3 lines"


def test_a_read_loop_around_dw_rcon_probe_sees_every_line(tmp_path):
    r = loop(tmp_path, "dw_rcon_probe")
    assert r.returncode == 0, r.stderr
    assert r.stdout.strip() == "3", f"the loop ran {r.stdout.strip()} time(s) over 3 lines"


def test_the_fake_does_consume_stdin_so_the_test_can_red(tmp_path):
    """The perturbation: a call that forwards stdin, as both did before."""
    bindir = tmp_path / "bin"
    bindir.mkdir()
    docker = bindir / "docker"
    docker.write_text(FAKE, encoding="utf-8")
    docker.chmod(0o755)
    lines = tmp_path / "lines"
    lines.write_text("a\nb\nc\n", encoding="utf-8")
    script = f'n=0; while read -r x; do docker exec -i srv rcon-cli "say $x" >/dev/null; n=$((n + 1)); done <"{lines}"; echo "$n"'
    env = {"PATH": f"{bindir}{os.pathsep}/usr/bin:/bin"}
    r = subprocess.run(["bash", "-c", script], env=env, capture_output=True, text=True)
    assert r.stdout.strip() == "1", r.stdout
