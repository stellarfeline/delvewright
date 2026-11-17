"""Guards for `validation/branch-runs.sh`: a world whose server stood still
because the host could not hold it is reported as the host's verdict, never as
a failure of its path.

The case is the town's branch worlds 30, 31, 32 and 34: the vanilla watchdog
halted each server during its first join's placement (`A single server tick
took 60.00 seconds`), the bot lost its connection (`keepAliveError` /
`socketClosed`), and each crash report read 8406 to 13059 MiB of the host's
swap in use while the ladder's two worlds held at most 16 GiB of 30.5.

Docker and node are stubs (as in `test_branch_runs_keeps_bot_report.py`): the
subject is how the script classifies what the server and the bot said, and a
real server would make this slower without making it stronger.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent.parent

COPIED = (
    "validation/branch-runs.sh",
    "validation/fresh-volumes.sh",
    "validation/compose.yaml",
    "validation/lib/delve-image.sh",
    "validation/lib/branch-jobs.sh",
    "validation/lib/ladder-images.sh",
    "tools/lib/server-heap.sh",
)

# `up` plays a bot that lost its server mid-placement: it writes a FAILED
# report, prints the disconnect, and exits 1. `logs` prints the server's
# log ($STUB_SERVER_LOG); `ps -q` names a server container and `exec` reads
# the host's /proc/meminfo with $STUB_SWAP_USED_KB of swap in use.
DOCKER_STUB = r"""#!/usr/bin/env bash
case " $* " in
  *" info "*)
    case " $* " in *NCPU*) echo 16 ;; *) echo 32749125632 ;; esac ;;
  *" exec "*)
    printf 'MemTotal: 31981568 kB\nMemAvailable: 1048576 kB\nSwapTotal: 16777216 kB\nSwapFree: %d kB\n' \
      $(( 16777216 - ${STUB_SWAP_USED_KB:-0} )) ;;
  *" ps -q "*) echo cid-server ;;
  *" logs "*) printf '%s\n' "$STUB_SERVER_LOG" ;;
  *" up "*)
    sleep "${STUB_UP_SECONDS:-0}"
    out="validation/${DW_BOT_OUT#./}"
    mkdir -p "$out"
    printf '{"build":{"manifest_sha256":"feed"},"branches":{"tier":"all","outcomes":[{"branch":"%s","ran":true,"passed":false,"reason":"step 0 (select-class) failed"}]}}\n' \
      "$DELVEWRIGHT_BRANCH" > "$out/run-report.json"
    echo "bot-1     | [connection] connection ended: ${STUB_DISCONNECT:-socketClosed}"
    echo "bot-1     | FAILED: the server disconnected the bot"
    exit 1 ;;
esac
exit 0
"""

NODE_STUB = "#!/usr/bin/env bash\necho world/30\n"

WATCHDOG_LOG = (
    "server-1  | [21:01:59] [Server Watchdog/ERROR]: A single server tick took 60.00 seconds (should be max 0.05)\n"
    "server-1  | \tVirtual memory used (MiB): 37501.91\n"
    "server-1  | \tSwap memory used (MiB): {swap}.54\n"
)


def _exe(path: Path, text: str) -> None:
    path.write_text(text)
    path.chmod(0o755)


def _run(tmp_path: Path, **stub_env: str) -> tuple[subprocess.CompletedProcess, dict]:
    engine = tmp_path / "engine"
    for rel in COPIED:
        dst = engine / rel
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(REPO / rel, dst)
    out = tmp_path / "delve-output"
    (out / "validation").mkdir(parents=True)
    (out / "server").mkdir()
    (out / "server" / "resources.properties").write_text("heap-max=7G\n")
    (out / "critical-path.json").write_text("{}\n")
    (out / "validation" / "branch-plan.json").write_text(
        json.dumps({"campaign_id": "c", "branches": [{"id": "world/30", "chronicle": None, "endings": []}]})
    )
    stubs = tmp_path / "bin"
    stubs.mkdir()
    _exe(stubs / "docker", DOCKER_STUB)
    _exe(stubs / "node", NODE_STUB)
    env = {
        **os.environ,
        "PATH": f"{stubs}{os.pathsep}{os.environ['PATH']}",
        "EULA": "TRUE",
        "DELVE_OUTPUT": str(out),
        **stub_env,
    }
    for k in ("DW_RUN_OUT", "DW_BOT_OUT", "DW_COMPOSE_PROJECT", "DELVEWRIGHT_BRANCHES", "DELVEWRIGHT_BRANCH_JOBS"):
        env.pop(k, None)
    r = subprocess.run(
        ["bash", str(engine / "validation" / "branch-runs.sh"), "--project", "dw-c-r1", "--jobs", "2"],
        cwd=tmp_path,
        env=env,
        capture_output=True,
        text=True,
    )
    merged = json.loads((engine / "validation" / "run-out" / "dw-c-r1" / "branch-runs.json").read_text())
    return r, merged["branches"][0]


def test_a_watchdog_halt_on_a_swapping_host_is_the_hosts_verdict_not_the_paths(tmp_path):
    r, row = _run(tmp_path, STUB_SERVER_LOG=WATCHDOG_LOG.format(swap=8406))
    out = r.stdout + r.stderr
    assert r.returncode == 1, out
    assert (row["ran"], row["passed"], row.get("host_overloaded")) == (False, False, True), row
    assert "HOST OVERLOADED, not a verdict on the path" in row["reason"], row
    assert "the vanilla watchdog halted it (A single server tick took 60.00 seconds)" in row["reason"], row
    assert "while the host held 8406 MiB of swap" in row["reason"], row
    # The ladder's own share is stated, so the reader sees who held the rest:
    # one world (the plan's only one) of 7G plus 1024 MiB under a 31232 MiB daemon.
    assert "this ladder's 1 world(s) hold at most 8192 MiB of the host's 31232 MiB" in row["reason"], row
    assert "world/30: HOST-OVERLOADED" in out, out
    assert "branch world/30 UNPROVEN (exit 1)" in out, out
    assert "branch world/30 FAILED" not in out, out


def test_a_watchdog_halt_on_a_host_with_room_stays_a_failure(tmp_path):
    """The perturbation toward the vacuous shape: a host verdict without the
    host's evidence would excuse a build whose own tick runs past 60 s."""
    r, row = _run(tmp_path, STUB_SERVER_LOG=WATCHDOG_LOG.format(swap=113))
    out = r.stdout + r.stderr
    assert r.returncode == 1, out
    assert (row["ran"], row["passed"]) == (True, False), row
    assert not row.get("host_overloaded"), row
    assert "branch world/30 FAILED (exit 1)" in out, out


def test_a_keepalive_loss_while_the_sampler_saw_the_host_swapping_is_the_hosts_verdict(tmp_path):
    """No crash report: the server stood still under 60 s, long enough for the
    bot's 30 s keepalive to lapse. The evidence is the sampler's reading."""
    r, row = _run(
        tmp_path,
        STUB_SERVER_LOG="server-1  | [21:05:08] [Server thread/WARN]: Can't keep up!",
        STUB_DISCONNECT="keepAliveError",
        STUB_SWAP_USED_KB=str(2048 * 1024),
        STUB_UP_SECONDS="2",
    )
    out = r.stdout + r.stderr
    assert r.returncode == 1, out
    assert row.get("host_overloaded") is True, row
    assert "the bot heard no keepalive from it for 30 s (keepAliveError)" in row["reason"], row
    assert "while the host held 2048 MiB of swap" in row["reason"], row
    mem = (tmp_path / "engine" / "validation" / "run-out" / "dw-c-r1" / "world-30.mem").read_text().split()
    assert mem[1:] == ["1024", "2048"], mem


def test_a_socket_close_with_no_stall_on_the_server_stays_a_failure(tmp_path):
    """A disconnect the server did not stall into (no watchdog, no keepalive
    lapse) is not the host's, whatever the swap."""
    r, row = _run(
        tmp_path,
        STUB_SERVER_LOG="server-1  | [21:05:08] [Server thread/INFO]: delve-bot lost connection",
        STUB_SWAP_USED_KB=str(9000 * 1024),
        STUB_UP_SECONDS="2",
    )
    assert (row["ran"], row["passed"]) == (True, False), row
    assert not row.get("host_overloaded"), row
