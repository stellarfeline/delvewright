"""`playtest-server.sh up` sets a real MEMORY, and a probe failure leaves
nothing running.

Two things are bound here, both about a large campaign — many tiles plus many
horizon templates, more than a small build:

- The `docker run` carries a real `MEMORY`, well above the itzg image's own 1G
  default. Loading a large campaign at 1G can throw
  `java.lang.OutOfMemoryError: Java heap space` failing structure loads, so
  NPCs never spawn — and dying with "no dw_npc entities found" would be true
  but the wrong defect: a creator reading that message goes looking for a
  content bug that is not there. A probe failure checks the log for the OOM
  string and names it instead.
- A probe failure removes the half-booted container rather than leaving it
  running: the next `up` would otherwise find host 25565 still bound behind a
  mutex the failed session already released, or a creator would be staring at
  a "failed" session that is still quietly holding memory.

Both `up`'s expensive parts (a real `delvec` build, the staging gate, an
actual boot) are out of reach of a unit test — `up` publishes host 25565 with
a hardcoded `-p 25565:25565`, exactly as the existing suite already notes. So
this drives the extracted SEAMS instead, with `DW_PLAYTEST_SERVER_TEST_HOOK=1`
sourcing the real script and calling its real functions directly:

- `dw_playtest_docker_run_argv` — the exact `docker run` argv, provably
  carrying `-e MEMORY=<value>` for both the default and an explicit
  `--memory`, with no docker on PATH at all;
- `dw_playtest_log_shows_oom` — the OOM string match a probe failure consults
  before blaming content;
- `up_failed` — the real EXIT trap, called directly with a fake `docker` on
  PATH and the handful of variables the real `up` flow would have set
  (`NAME`, `STAGE`, `SESSION_FILE`, `UP_OK`), proving the container is removed
  and the mutex released without ever running a build.
"""

import os
import pathlib
import subprocess

import pytest

ROOT = pathlib.Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "tools" / "creator" / "playtest-server.sh"


def run_hook(code: str, path: str = "/usr/bin:/bin:/usr/sbin:/sbin", extra_env=None):
    env = {"PATH": path, **(extra_env or {})}
    return subprocess.run(
        ["bash", "-c", f'DW_PLAYTEST_SERVER_TEST_HOOK=1 . "{SCRIPT}"; {code}'],
        cwd=ROOT,
        env=env,
        capture_output=True,
        text=True,
    )


# ---------------------------------------------------------------------------
# The default is a real number, well above itzg's proven-insufficient 1G
# ---------------------------------------------------------------------------


def test_the_default_is_a_gigabyte_figure_above_itzgs_own_1g():
    result = run_hook('printf "%s" "$MEMORY_DEFAULT"')
    assert result.returncode == 0, result.stderr
    value = result.stdout.strip()
    assert value.endswith("G"), f"not a gigabyte figure: {value!r}"
    assert int(value[:-1]) >= 2, (
        f"MEMORY_DEFAULT={value!r} is not clearly above itzg's own 1G default, "
        "which is exactly what OOM'd on the reported campaign"
    )


# ---------------------------------------------------------------------------
# `dw_playtest_docker_run_argv`: MEMORY reaches the real docker-run argv
# ---------------------------------------------------------------------------


def argv_of(memory: str) -> list[str]:
    result = run_hook(
        f'dw_playtest_docker_run_argv myname 1.21.11 pw {memory} /stage/dir'
    )
    assert result.returncode == 0, result.stderr
    return result.stdout.splitlines()


@pytest.mark.parametrize("memory", ["4G", "8G", "512M"])
def test_the_argv_carries_exactly_the_memory_it_was_given(memory):
    lines = argv_of(memory)
    assert "-e" in lines, lines
    idx = lines.index(f"MEMORY={memory}")
    assert lines[idx - 1] == "-e", (
        f"MEMORY={memory} is present but not paired with its own -e flag: {lines}"
    )


def test_the_argv_still_carries_the_container_name_port_and_image():
    lines = argv_of("4G")
    assert lines[:2] == ["docker", "run"]
    assert "myname" in lines
    assert "25565:25565" in lines
    assert lines[-1] == "itzg/minecraft-server:latest"


# ---------------------------------------------------------------------------
# `dw_playtest_log_shows_oom`: the exact JVM string, nothing looser
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "log,expected",
    [
        ("...\njava.lang.OutOfMemoryError: Java heap space\n...", True),
        ("Done (12.345s)! For help, type \"help\"", False),
        ("", False),
    ],
)
def test_oom_detection_matches_the_literal_jvm_string_only(log, expected):
    result = run_hook(f'dw_playtest_log_shows_oom {log!r} && echo YES || echo NO')
    assert result.stdout.strip() == ("YES" if expected else "NO"), result.stderr


# ---------------------------------------------------------------------------
# `up_failed`: the real EXIT-trap function, called directly
# ---------------------------------------------------------------------------


def fake_docker(tmp_path, running_names):
    """A `docker` that knows about `running_names`, `rm -f`s them for real (by
    forgetting them), and records every `logs`/`rm` call it received."""
    bindir = tmp_path / "fakebin"
    bindir.mkdir(exist_ok=True)
    state = tmp_path / "docker-state.txt"
    state.write_text("\n".join(running_names) + ("\n" if running_names else ""))
    calls = tmp_path / "docker-calls.log"
    calls.write_text("")
    script = bindir / "docker"
    script.write_text(
        "#!/usr/bin/env python3\n"
        "import sys, pathlib\n"
        f"STATE = pathlib.Path({str(state)!r})\n"
        f"CALLS = pathlib.Path({str(calls)!r})\n"
        "args = sys.argv[1:]\n"
        "with CALLS.open('a') as f:\n"
        "    f.write(' '.join(args) + chr(10))\n"
        "known = [l for l in STATE.read_text().splitlines() if l]\n"
        "if args[:2] == ['container', 'inspect']:\n"
        "    sys.exit(0 if args[2] in known else 1)\n"
        "if args[:1] == ['logs']:\n"
        "    print('fake boot log for ' + args[-1])\n"
        "    sys.exit(0 if args[-1] in known else 1)\n"
        "if args[:1] == ['rm']:\n"
        "    name = args[-1]\n"
        "    if name not in known:\n"
        "        sys.exit(1)\n"
        "    known = [k for k in known if k != name]\n"
        "    STATE.write_text(chr(10).join(known) + (chr(10) if known else ''))\n"
        "    sys.exit(0)\n"
        "sys.exit(0)\n",
        encoding="utf-8",
    )
    script.chmod(0o755)
    return bindir, state, calls


def call_up_failed(tmp_path, *, running_names, name="dw-test", stage=None,
                    session_file=None, mutex_held_by=None, up_ok="0"):
    bindir, state, calls = fake_docker(tmp_path, running_names)
    mutex_dir = tmp_path / "mutex.lock.d"
    if mutex_held_by is not None:
        mutex_dir.mkdir()
        (mutex_dir / "HOLDER").write_text(f"{mutex_held_by} 1\n", encoding="utf-8")
    env = {
        "PATH": f"{bindir}{os.pathsep}/usr/bin:/bin:/usr/sbin:/sbin",
        "HOME": str(tmp_path),
        "DW_MUTEX_DIR": str(mutex_dir),
    }
    setup = [f'NAME="{name}"', f'UP_OK="{up_ok}"']
    if stage is not None:
        setup.append(f'STAGE="{stage}"')
    if session_file is not None:
        setup.append(f'SESSION_FILE="{session_file}"')
    if mutex_held_by is not None:
        # `up_failed` calls `dw_mutex_release`, which only releases a lock
        # THIS shell believes it took (`$DW_MUTEX_ME`) — exactly the state a
        # real `up` would be in right after its own `dw_mutex_acquire`.
        setup.append(f'DW_MUTEX_ME="{mutex_held_by}"')
    code = "; ".join(setup) + "; up_failed"
    result = run_hook(code, path=env["PATH"], extra_env=env)
    return result, mutex_dir, state, calls


def test_a_container_the_session_started_is_removed_on_probe_failure(tmp_path):
    result, _, state, calls = call_up_failed(
        tmp_path, running_names=["dw-test"], mutex_held_by="owner-play-session",
    )
    assert result.returncode == 0, result.stderr
    assert state.read_text().strip() == "", (
        "the container the failed session started is still 'running' after "
        f"up_failed:\n{state.read_text()}"
    )
    assert "removed the container" in result.stderr, result.stderr
    assert "rm -f dw-test" in calls.read_text() or "rm -f" in calls.read_text()


def test_the_mutex_is_released_on_probe_failure(tmp_path):
    result, mutex_dir, _, _ = call_up_failed(
        tmp_path, running_names=["dw-test"], mutex_held_by="owner-play-session",
    )
    assert result.returncode == 0, result.stderr
    assert not mutex_dir.exists(), (
        "the 25565 mutex is still held after a failed up: the next `up` would "
        "find the port apparently free-but-blocked, or refuse behind a stale lock"
    )


def test_a_success_that_reached_up_ok_removes_nothing(tmp_path):
    """The trap is a no-op once `up` actually finished — `UP_OK=1` is the
    signal, and a successful session's container is the whole point of
    running it."""
    result, _, state, _ = call_up_failed(
        tmp_path, running_names=["dw-test"], mutex_held_by="owner-play-session",
        up_ok="1",
    )
    assert result.returncode == 0, result.stderr
    assert state.read_text().strip() == "dw-test", (
        "up_failed removed a container from a session that succeeded"
    )


def test_no_container_ever_started_is_a_quiet_no_op(tmp_path):
    """A failure before `docker run` (a build or staging-gate refusal) must
    not try to remove a container that was never created."""
    result, _, state, calls = call_up_failed(
        tmp_path, running_names=[], mutex_held_by="owner-play-session",
    )
    assert result.returncode == 0, result.stderr
    assert "rm -f" not in calls.read_text()
    assert "removed the container" not in result.stderr


def test_the_boot_log_is_captured_into_the_staged_world_before_removal(tmp_path):
    """The container's own log dies with it; the staged world does not (it is
    a bind mount). Cleanup must not throw away the one diagnostic the wrapper's
    stdout could offer that the mounted world does not already have."""
    stage = tmp_path / "staged-world"
    stage.mkdir()
    result, _, _, _ = call_up_failed(
        tmp_path, running_names=["dw-test"], mutex_held_by="owner-play-session",
        stage=str(stage),
    )
    assert result.returncode == 0, result.stderr
    captured = stage / "docker-boot.log"
    assert captured.exists(), "docker logs were not captured before the container was removed"
    assert "dw-test" in captured.read_text()
