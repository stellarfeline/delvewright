"""An rcon reply that hit the packet ceiling is refused, in both halves of the rule.

## What this is about

`tools/lib/rcon.{sh,mjs}` is the repository's one definition of "the server did
not do what was asked", and it knew one way for a reply to be worthless: a
refusal. There is a second, and it is worse, because it does not look like a
failure at all. An rcon reply arrives in ONE packet and `rcon-cli` reads one, so
a longer answer is CUT — no error, no marker, no short line — and what comes back
is a shorter, entirely well-formed answer.

Measured on the pinned 1.21.11 server by asking the same multi-record reply at
increasing `limit=N` (``execute as @e[type=minecraft:interaction,limit=N] run
data get entity @s``, about 360 bytes a record):

    limit  records  reply_bytes
        1        1          356
        ...
       10       10         3684
       11       11         4077
       12       11         4097
       16       11         4097

So a selector that matched sixteen entities answered with eleven of them and
nothing anywhere said eleven was not the population. That is CLAUDE.md's "a count
equal to its own fetch limit is not a measurement — it is the limit", arriving
through the channel every live measurement in this repository uses; and it did
exactly that to a census taken while this test was being written.

## Why the two constants differ by one, and why that is asserted here

The shell half counts the reply AFTER `tr '\\n' ' '` has turned `rcon-cli`'s
trailing newline into a space, so a full packet measures 4097 there; the Node
half tests the raw stdout against the 4096-byte payload itself. Two numbers for
one fact is exactly the pair that goes stale on one side, so the relationship is
pinned here rather than left to whoever edits one of them next.
"""

import json
import shutil
import subprocess
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
RCON_SH = REPO / "tools" / "lib" / "rcon.sh"
RCON_MJS = REPO / "tools" / "lib" / "rcon.mjs"

# The payload ceiling measured above. Written here as a literal ON PURPOSE: this
# file is the second observer, so reading the number out of the thing under test
# would make the test agree with itself.
MEASURED_PAYLOAD_CAP = 4096


def _bash(script: str) -> str:
    """Run `script` under bash (never the interactive shell) with rcon.sh sourced."""
    out = subprocess.run(
        ["bash", "-c", f'. "{RCON_SH}"\n{script}'],
        capture_output=True,
        text=True,
        check=True,
    )
    return out.stdout.strip()


def _node(script: str) -> str:
    node = shutil.which("node")
    assert node is not None, "node is required to test the Node half of the rcon rule"
    out = subprocess.run(
        [node, "--input-type=module", "-e", f'import * as rcon from "{RCON_MJS}";\n{script}'],
        capture_output=True,
        text=True,
        check=True,
    )
    return out.stdout.strip()


def test_the_two_halves_agree_on_the_measured_ceiling():
    sh_cap = int(_bash('printf "%s" "$DW_RCON_REPLY_CAP"'))
    mjs_cap = int(_node("process.stdout.write(String(rcon.REPLY_CAP));"))
    # The Node half tests the raw payload; the shell half tests it after `tr` has
    # turned the trailing newline into a space, so it counts exactly one more.
    assert mjs_cap == MEASURED_PAYLOAD_CAP, mjs_cap
    assert sh_cap == mjs_cap + 1, (sh_cap, mjs_cap)


def test_a_reply_at_the_ceiling_is_truncated_and_one_below_it_is_not():
    # Shell half, at and around its own cap.
    verdicts = _bash(
        "cap=$DW_RCON_REPLY_CAP\n"
        'for n in $((cap - 1)) "$cap" $((cap + 1)); do\n'
        '  s="$(printf "%${n}s" "")"\n'
        '  if dw_rcon_truncated "$s"; then echo "$n cut"; else echo "$n whole"; fi\n'
        "done"
    )
    assert verdicts.splitlines() == [
        f"{MEASURED_PAYLOAD_CAP} whole",
        f"{MEASURED_PAYLOAD_CAP + 1} cut",
        f"{MEASURED_PAYLOAD_CAP + 2} cut",
    ], verdicts
    # Node half, same shape one byte down.
    js = _node(
        "const cap = rcon.REPLY_CAP;\n"
        "const at = (n) => rcon.isTruncated(' '.repeat(n));\n"
        "process.stdout.write(JSON.stringify([at(cap - 1), at(cap), at(cap + 1)]));"
    )
    assert json.loads(js) == [False, True, True], js


def test_a_truncated_reply_is_refused_by_name_rather_than_returned():
    # The shell half prints what came back — a caller that wants the fragment can
    # still read stdout — and returns non-zero, so the fragment cannot be mistaken
    # for the population. Both halves say the byte count and the ceiling.
    out = subprocess.run(
        [
            "bash",
            "-c",
            f'. "{RCON_SH}"\n'
            'reply="$(printf "%4097s" "")"\n'
            'if dw_rcon_truncated "$reply"; then echo REFUSED; else echo ACCEPTED; fi',
        ],
        capture_output=True,
        text=True,
        check=True,
    )
    assert out.stdout.strip() == "REFUSED", out.stdout

    err = _node(
        "let msg = '';\n"
        "try { rcon.assertAccepted('data get entity @s', ' '.repeat(rcon.REPLY_CAP)); }\n"
        "catch (e) { msg = e.message; }\n"
        "process.stdout.write(msg);"
    )
    assert "was CUT" in err, err
    assert str(MEASURED_PAYLOAD_CAP) in err, err
    # And a reply that is merely long is still accepted — the rule must not turn
    # every big answer into a failure.
    ok = _node(
        "process.stdout.write(String(\n"
        "  rcon.assertAccepted('x', 'a'.repeat(rcon.REPLY_CAP - 1)).length));"
    )
    assert int(ok) == MEASURED_PAYLOAD_CAP - 1, ok


def test_the_measured_evidence_stays_beside_the_constant():
    # A constant with no argument behind it is the shape that gets "tidied" to a
    # rounder number by the next reader. Both halves carry the measurement.
    for path in (RCON_SH, RCON_MJS):
        text = path.read_text(encoding="utf-8")
        assert "4077" in text, f"{path.name} must carry the measured saturation point"
        assert "limit=N" in text, f"{path.name} must say how the ceiling was measured"


if __name__ == "__main__":  # pragma: no cover — lets this run without pytest installed
    for name, fn in sorted(globals().items()):
        if name.startswith("test_") and callable(fn):
            fn()
            print(f"ok  {name}")
