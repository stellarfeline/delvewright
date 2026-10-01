"""The shared rcon rule knows the refusals the pinned server actually produces.

`tools/lib/rcon.{sh,mjs}` is the repository's one definition of "the server did
not do what was asked", and its own doc says a shape enters the list on evidence.
This file holds the evidence for the shapes a measurement has been caught by, and
asserts BOTH halves recognise each one — the halves are two copies of one truth
and drift the same way, so a test that asked only one would pass on half a fix.

Measured on the pinned 1.21.11 server (`versions.toml` `[images.base]`
`mirror_of`), driving a mineflayer bot at 14 of 20 health:

    data merge entity <player> {Health:20.0f}      -> Unable to modify player data
    data modify entity <player> Health set value   -> Unable to modify player data

Vanilla refuses to write a PLAYER's entity data. `crates/delvec/src/compiler/
emit.rs` already knew that and emits around it; this rule did not, so a rig that
reset its bot's health between blows with that command reset nothing and was told
nothing. Eighteen rows of shield readings were then taken on a bot that had died
on its fourth blow and respawned without shield or armour, and every one of them
read as a clean zero.

The replies are written here as literals on purpose: this file is the second
observer, and reading them out of the thing under test would make the test agree
with itself.
"""

import json
import shutil
import subprocess
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
RCON_SH = REPO / "tools" / "lib" / "rcon.sh"
RCON_MJS = REPO / "tools" / "lib" / "rcon.mjs"

# Verbatim replies the pinned server gave. One per measured refusal.
MEASURED_REFUSALS = [
    "Unable to modify player data",
    "Target is invulnerable to the given damage type",
]

# Replies that are the server ANSWERING, not refusing. A rule that swallowed one
# of these would turn a working measurement into a crash, so the negative side is
# asserted beside the positive one.
MEASURED_ANSWERS = [
    "refuse_bot has the following entity data: 14.0f",
    "Applied 6.0 damage to refuse_bot",
    "Modified entity data of Armor Stand",
    "There are 0 of a max of 20 players online: ",
]


def _bash_verdicts(replies: list[str]) -> list[bool]:
    script = '. "%s"\n' % RCON_SH
    script += "\n".join(
        'if dw_rcon_rejected "$%d"; then echo true; else echo false; fi' % (i + 1)
        for i in range(len(replies))
    )
    out = subprocess.run(
        ["bash", "-c", script, "bash", *replies], capture_output=True, text=True, check=True
    )
    return [line == "true" for line in out.stdout.split()]


def _node_verdicts(replies: list[str]) -> list[bool]:
    node = shutil.which("node")
    assert node is not None, "node is required to test the Node half of the rcon rule"
    script = (
        'import * as rcon from "%s";\n' % RCON_MJS
        + "const replies = %s;\n" % json.dumps(replies)
        + "process.stdout.write(JSON.stringify(replies.map((r) => rcon.isRejection(r))));"
    )
    out = subprocess.run(
        [node, "--input-type=module", "-e", script], capture_output=True, text=True, check=True
    )
    return json.loads(out.stdout)


def test_every_measured_refusal_is_a_refusal_in_both_halves():
    assert MEASURED_REFUSALS, "a rule with nothing to recognise proves nothing"
    assert _bash_verdicts(MEASURED_REFUSALS) == [True] * len(MEASURED_REFUSALS)
    assert _node_verdicts(MEASURED_REFUSALS) == [True] * len(MEASURED_REFUSALS)


def test_a_real_answer_is_not_mistaken_for_a_refusal():
    assert MEASURED_ANSWERS, "a rule with nothing to pass proves nothing"
    assert _bash_verdicts(MEASURED_ANSWERS) == [False] * len(MEASURED_ANSWERS)
    assert _node_verdicts(MEASURED_ANSWERS) == [False] * len(MEASURED_ANSWERS)


# --- readiness: the poll waits on the server's own `list` answer ---------------
#
# `dw_rcon_probe` folds stderr into its reply, so a connection failure comes back
# as a NON-EMPTY string; a poll that tested "non-empty" read ready on its first
# attempt, always. The error texts below are what the probe hands back when the
# server is not there. Written as literals: this file is the second observer.

PROBE_ERROR_TEXT = [
    "",
    "Failed to connect to RCON server at localhost:25575: dial tcp [::1]:25575: connect: connection refused ",
    "Error response from daemon: No such container: dw-spike ",
    "Error response from daemon: container abc is not running ",
    "Error: Authentication failed ",
    "There are of a max of players online: ",
    "There are 0 of a max of  players online: ",
    "There are x of a max of 20 players online: ",
    "Incorrect argument for command ",
]

REAL_LIST_REPLIES = [
    "There are 0 of a max of 20 players online: ",
    "There are 2 of a max of 8 players online: dw-a, dw-b ",
    "There are 0 of a max of 20 players online:",
]


def _bash_ready(replies: list[str]) -> list[bool]:
    script = '. "%s"\n' % RCON_SH
    script += "\n".join(
        'if dw_rcon_is_list_answer "$%d"; then echo true; else echo false; fi' % (i + 1)
        for i in range(len(replies))
    )
    out = subprocess.run(
        ["bash", "-c", script, "bash", *replies], capture_output=True, text=True, check=True
    )
    return [line == "true" for line in out.stdout.split("\n") if line]


def _node_ready(replies: list[str]) -> list[bool]:
    node = shutil.which("node")
    assert node is not None, "node is required to test the Node half of the rcon rule"
    script = (
        'import * as rcon from "%s";\n' % RCON_MJS
        + "const replies = %s;\n" % json.dumps(replies)
        + "process.stdout.write(JSON.stringify(replies.map((r) => rcon.isListAnswer(r))));"
    )
    out = subprocess.run(
        [node, "--input-type=module", "-e", script], capture_output=True, text=True, check=True
    )
    return json.loads(out.stdout)


def test_the_probes_own_error_text_is_not_readiness_in_either_half():
    assert _bash_ready(PROBE_ERROR_TEXT) == [False] * len(PROBE_ERROR_TEXT)
    assert _node_ready(PROBE_ERROR_TEXT) == [False] * len(PROBE_ERROR_TEXT)


def test_the_servers_list_answer_is_readiness_in_either_half():
    assert _bash_ready(REAL_LIST_REPLIES) == [True] * len(REAL_LIST_REPLIES)
    assert _node_ready(REAL_LIST_REPLIES) == [True] * len(REAL_LIST_REPLIES)


def test_dw_rcon_ready_judges_the_reply_not_the_exit_status(tmp_path):
    """End to end through the real function, with `docker` stubbed.

    A `docker` that fails the way a dead container does (text on stderr, folded
    into the reply) must read not-ready; one that prints the vanilla answer must
    read ready. Without this the shape test above could pass while the function
    still tested non-emptiness.
    """
    import os

    def run(stub_body: str) -> int:
        d = tmp_path / ("bin" + str(abs(hash(stub_body))))
        d.mkdir()
        docker = d / "docker"
        docker.write_text("#!/bin/sh\n" + stub_body + "\n")
        docker.chmod(0o755)
        env = dict(os.environ, PATH=f"{d}:{os.environ['PATH']}")
        return subprocess.run(
            ["bash", "-c", '. "%s"; dw_rcon_ready some-container' % RCON_SH],
            env=env,
            capture_output=True,
        ).returncode

    assert run("echo 'Failed to connect to RCON server at localhost:25575' >&2; exit 1") != 0
    assert run("echo 'Failed to connect to RCON server at localhost:25575' >&2; exit 0") != 0
    assert run("echo 'Error response from daemon: No such container: x' >&2; exit 1") != 0
    assert run("echo 'There are 0 of a max of 20 players online: '") == 0
