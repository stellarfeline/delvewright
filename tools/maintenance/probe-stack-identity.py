#!/usr/bin/env python3
"""Measure, on the PINNED server, what a `custom_data` mark does to a stack.

A `collect` that adopts a holder fills it with its stack stamped
`custom_data={dw_collect:"<objective id>"}` and completes only on that mark
(spec-0111 §4.2, `compiler::holders::placement_mark`). Two vanilla behaviours
carry that rule, and this asks the running game for both:

1. **Merging.** A hopper pushes into a chest by the game's own merge rule
   (`ItemStack.isSameItemSameComponents`, the rule a player's inventory uses
   to stack a pick-up): a marked stack joins a stack with the same mark, and
   an unmarked stack or one with another mark takes its own slot.
2. **Matching.** The item predicate `<item>[custom_data~<mark>]` (what the
   per-tick held check reads) matches a stack carrying the mark — with a
   `custom_name` beside it, or another `custom_data` key beside it — and
   matches no unmarked stack and no stack with another mark; the bare id
   matches all of them.

    tools/maintenance/probe-stack-identity.py --jar SERVER_JAR --work DIR [--java JAVA]

The jar is refused unless its sha256 is the `versions.toml` pin. Requires a JDK
>= 21. Maintenance tool: never run by CI; run it when the MC pin moves. Exits 1
when the game disagrees with `EXPECT`; the record is
`docs/reference/stack-identity.md`.
"""

from __future__ import annotations

import argparse
import hashlib
import pathlib
import re
import socket
import struct
import subprocess
import sys
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "lib"))
from versions import pin as versions_pin  # noqa: E402

RCON_PORT, RCON_PASS = 25997, "probe"
CHEST = (0, -60, 0)
HOPPER = (0, -59, 0)
A = '{dw_collect:"obj/a"}'
B = '{dw_collect:"obj/b"}'
NAME = 'custom_name={"text":"Hall Seal","italic":false}'
# What each pushed stack is, and the chest slot the game must merge it into.
PUSHES = [
    ("marked a, onto marked a", f"minecraft:paper[custom_data={A}]", 0),
    ("unmarked", "minecraft:paper", 1),
    ("marked b", f"minecraft:paper[custom_data={B}]", 2),
    ("marked a again", f"minecraft:paper[custom_data={A}]", 0),
]
# Slot -> (stack placed by hand, or None for a slot only the hopper fills).
PLACED = {
    0: f"minecraft:paper[custom_data={A}]",
    3: f"minecraft:paper[{NAME},custom_data={A}]",
    4: 'minecraft:paper[custom_data={dw_collect:"obj/a",other:1b}]',
}
# (slot, predicate) -> whether it matches.
EXPECT = {
    (0, f"minecraft:paper[custom_data~{A}]"): True,
    (1, f"minecraft:paper[custom_data~{A}]"): False,
    (2, f"minecraft:paper[custom_data~{A}]"): False,
    (3, f"minecraft:paper[custom_data~{A}]"): True,
    (4, f"minecraft:paper[custom_data~{A}]"): True,
    (0, "minecraft:paper"): True,
    (1, "minecraft:paper"): True,
    (2, "minecraft:paper"): True,
    (2, f"minecraft:paper[custom_data~{B}]"): True,
}
# Slot -> count once every push has landed: slot 0 holds the placed stack and
# the two marked-a pushes.
COUNTS = {0: 3, 1: 1, 2: 1, 3: 1, 4: 1}


class Rcon:
    def __init__(self, port: int, password: str) -> None:
        self.s = socket.create_connection(("127.0.0.1", port), timeout=30)
        self.rid = 0
        self._send(3, password)

    def _send(self, kind: int, body: str) -> str:
        self.rid += 1
        payload = struct.pack("<ii", self.rid, kind) + body.encode() + b"\x00\x00"
        self.s.sendall(struct.pack("<i", len(payload)) + payload)
        n = struct.unpack("<i", self._read(4))[0]
        data = self._read(n)
        rid = struct.unpack("<i", data[:4])[0]
        if rid == -1:
            sys.exit("probe: rcon authentication refused")
        return data[8:-2].decode()

    def _read(self, n: int) -> bytes:
        buf = b""
        while len(buf) < n:
            chunk = self.s.recv(n - len(buf))
            if not chunk:
                sys.exit("probe: rcon connection closed")
            buf += chunk
        return buf

    def cmd(self, c: str, *ok: str) -> str:
        """Send `c` and require its reply to start with one of `ok`: every
        response is read for the success it names, so a refusal of any shape
        stops the run without a list of refusal shapes to keep in step."""
        reply = self._send(2, c)
        if not reply.startswith(ok):
            sys.exit(f"probe: `{c}` answered {reply!r}, expected one of {ok!r}")
        return reply


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--jar", required=True, type=pathlib.Path)
    ap.add_argument("--work", required=True, type=pathlib.Path)
    ap.add_argument("--java", default="java")
    a = ap.parse_args()

    want = versions_pin("minecraft", "server_jar_sha256")
    got = hashlib.sha256(a.jar.read_bytes()).hexdigest()
    if got != want:
        sys.exit(f"probe: {a.jar} is sha256 {got}, pin is {want}")
    a.work.mkdir(parents=True, exist_ok=False)
    (a.work / "eula.txt").write_text("eula=true\n")
    (a.work / "server.properties").write_text(
        "level-type=minecraft\\:flat\nenable-rcon=true\n"
        f"rcon.port={RCON_PORT}\nrcon.password={RCON_PASS}\nserver-port=25998\n"
        "online-mode=false\ngenerate-structures=false\n"
        "spawn-protection=0\nmax-tick-time=-1\npause-when-empty-seconds=-1\n")
    log = open(a.work / "server.log", "w")
    proc = subprocess.Popen([a.java, "-Xmx2G", "-jar", str(a.jar.resolve()), "nogui"],
                            cwd=a.work, stdout=log, stderr=subprocess.STDOUT, stdin=subprocess.PIPE)
    try:
        for _ in range(240):
            if "Done (" in (a.work / "server.log").read_text():
                break
            if proc.poll() is not None:
                sys.exit("probe: server exited before Done")
            time.sleep(1)
        else:
            sys.exit("probe: server never reported Done")
        r = Rcon(RCON_PORT, RCON_PASS)
        r.cmd("forceload add 0 0", "Marked chunk ")
        cx, cy, cz = CHEST
        hx, hy, hz = HOPPER
        r.cmd(f"setblock {cx} {cy} {cz} minecraft:chest", "Changed the block")
        r.cmd(f"setblock {hx} {hy} {hz} minecraft:hopper[facing=down]", "Changed the block")
        for slot, stack in PLACED.items():
            r.cmd(f"item replace block {cx} {cy} {cz} container.{slot} with {stack}", "Replaced ")
        disagree = 0
        print("push                       landed-in  expected")
        for what, stack, slot in PUSHES:
            before = slots(r, CHEST)
            r.cmd(f"item replace block {hx} {hy} {hz} container.0 with {stack}", "Replaced ")
            for _ in range(40):
                time.sleep(0.25)
                if slots(r, HOPPER) == {}:
                    break
            else:
                sys.exit(f"probe: the hopper never pushed `{stack}`: nothing was measured")
            after = slots(r, CHEST)
            moved = [s for s in after if after[s] != before.get(s, 0)]
            ok = moved == [slot]
            disagree += not ok
            print(f"{what:<26} {moved!s:<10} [{slot}]{'' if ok else '  DISAGREES'}")
        counts = slots(r, CHEST)
        print(f"final counts: {counts} (expected {COUNTS})")
        disagree += counts != COUNTS
        print("slot  predicate                                          matches  expected")
        for (slot, pred), want_match in EXPECT.items():
            reply = r.cmd(f"execute if items block {cx} {cy} {cz} container.{slot} {pred}",
                          "Test passed", "Test failed")
            got_match = reply.startswith("Test passed")
            disagree += got_match != want_match
            print(f"{slot:<5} {pred:<50} {got_match!s:<8} {want_match}"
                  f"{'' if got_match == want_match else '  DISAGREES'}")
        print(f"{len(PUSHES)} pushes, {len(EXPECT)} predicate reads, {disagree} disagreeing")
        return 1 if disagree else 0
    finally:
        proc.communicate(b"stop\n", timeout=60)


def slots(r: Rcon, pos: tuple[int, int, int]) -> dict[int, int]:
    """Slot -> count of the container at `pos`, read from its block data."""
    x, y, z = pos
    reply = r.cmd(f"data get block {x} {y} {z} Items", f"{x}, {y}, {z} has the following",
                  "Found no elements matching Items")
    if reply.startswith("Found no elements"):
        return {}
    body = reply[reply.index(":", reply.index("has the following")) + 1:]
    out: dict[int, int] = {}
    depth, start = 0, 0
    for i, ch in enumerate(body):
        if ch == "{":
            depth += 1
            if depth == 1:
                start = i
        elif ch == "}":
            depth -= 1
            if depth == 0:
                item = body[start:i]
                # The item's own keys sit at depth 1; a component's nested
                # braces are skipped by cutting them out first.
                flat = re.sub(r"\{[^{}]*\}", "", item[1:])
                while re.search(r"\{[^{}]*\}", flat):
                    flat = re.sub(r"\{[^{}]*\}", "", flat)
                slot = re.search(r"Slot: (\d+)b", flat)
                count = re.search(r"count: (\d+)", flat)
                if not (slot and count):
                    sys.exit(f"probe: unreadable item {item!r}")
                out[int(slot.group(1))] = int(count.group(1))
    return out


if __name__ == "__main__":
    sys.exit(main())
