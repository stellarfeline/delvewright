#!/usr/bin/env python3
"""Measure, on the PINNED server, under which declared hour and weather a drowned
engages a target standing on land.

`DW0920` refuses a drowned staged as a fight the party must win on land under a
bright hour. Its hour table (`compiler::engage::bright_outside`) is derived
by reading the pinned jar: `Drowned.okTarget` is `!level.isBrightOutside() ||
target.isInWater()`, `isBrightOutside` is `skyDarken < 4`, and `skyDarken` is
`15 - gameplay/sky_light_level` as the `minecraft:day` timeline and the
weather layers set it. This is the second method, sharing nothing with the
first: it asks the running game.

Per case it builds a roofed stone cell (so no body burns), sets the hour and
the weather, waits for the rain level to settle, summons a villager and a
bare-handed drowned in the cell (a `NoAI` villager is never targeted, measured), waits, and reads the villager's health. A
drowned that engages hits it; one whose AI refuses the target does not. The
last case repeats the tightest bright state with the cell's floor flooded, so
the villager stands in water — the footing clause.

    tools/maintenance/probe-drowned-engagement.py --jar SERVER_JAR --work DIR [--java JAVA]

The jar is refused unless its sha256 is the `versions.toml` pin. Requires a JDK
>= 21. Maintenance tool: CI reads the committed table and never runs this. Run it
when the MC pin moves; exit 1 when the game disagrees with the table.
"""

from __future__ import annotations

import argparse
import hashlib
import pathlib
import socket
import struct
import subprocess
import sys
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "lib"))
from versions import pin as versions_pin  # noqa: E402

# (keyword, /time set argument) — the table in crates/dsl/src/world.rs.
TIMES = [("day", "day"), ("noon", "noon"), ("dusk", "12000"), ("night", "night"),
         ("midnight", "midnight"), ("dawn", "23000")]
WEATHERS = ["clear", "rain", "thunder"]
# The committed table: the states in which a drowned refuses a target on land.
DISENGAGED = {(t, w) for t in ("day", "noon", "dusk") for w in ("clear", "rain")}

RCON_PORT, RCON_PASS = 25995, "probe"


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
        f"rcon.port={RCON_PORT}\nrcon.password={RCON_PASS}\nserver-port=25996\n"
        "online-mode=false\ndifficulty=normal\ngenerate-structures=false\n"
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
        for g in ("advance_time", "advance_weather", "spawn_mobs"):
            r.cmd(f"gamerule {g} false", "Gamerule ")
        r.cmd("forceload add 0 0 4 4", "Marked chunk ")
        r.cmd("kill @e[type=!player]", "Killed ", "No entity was found")
        cases = [(t, tok, w, "land") for t, tok in TIMES for w in WEATHERS]
        # The footing clause, at the tightest bright state: a flooded floor, and
        # a floor of waterlogged bottom slabs a body stands ON with its feet in
        # the slab's water.
        cases.append(("dusk", "12000", "rain", "water"))
        cases.append(("dusk", "12000", "rain", "waterlogged-slab"))
        disagree = 0
        engaged_cases = 0
        print("hour      weather  footing  villager-health  engaged  table")
        for i, (t, tok, w, wet) in enumerate(cases):
            # One cell, rebuilt per case, with a ticking witness: a server with
            # no player pauses after `pause-when-empty-seconds`, and a paused
            # world answers every read with the state it froze in.
            x = 0
            filled = "Successfully filled"
            r.cmd(f"fill {x} -60 0 {x + 4} -51 4 minecraft:air", filled, "No blocks were filled")
            r.cmd(f"fill {x} -60 0 {x + 4} -57 4 minecraft:stone hollow", filled)
            # Six blocks of cover: a thunder case's lightning strikes the top,
            # out of reach of the cell, and cannot turn the villager into a witch.
            r.cmd(f"fill {x} -56 0 {x + 4} -51 4 minecraft:stone", filled)
            if wet == "water":
                r.cmd(f"fill {x + 1} -59 1 {x + 3} -59 3 minecraft:water", filled)
            elif wet == "waterlogged-slab":
                r.cmd(f"fill {x + 1} -60 1 {x + 3} -60 3 "
                      "minecraft:stone_slab[type=bottom,waterlogged=true]", filled)
            r.cmd(f"time set {tok}", "Set the time to ")
            r.cmd(f"weather {w}", "Set the weather to ")
            time.sleep(7)  # the rain level moves 0.01 a tick: 100 ticks to settle
            day = r.cmd("time query daytime", "The time is ")
            tag = f"c{i}"
            r.cmd(f"summon minecraft:villager {x + 1.5} -59 1.5 {{PersistenceRequired:1b,Tags:[\"{tag}\"]}}",
                  "Summoned new ")
            r.cmd(f"summon minecraft:drowned {x + 3.5} -59 3.5 {{PersistenceRequired:1b,Tags:[\"{tag}\"]}}",
                  "Summoned new ")
            r.cmd(f"summon minecraft:item {x + 2.5} -59 2.5 {{Item:{{id:\"minecraft:stone\",count:1}},"
                  f"PickupDelay:32767s,Age:0s,Tags:[\"{tag}\"]}}", "Summoned new ")
            time.sleep(12)
            age = r.cmd(f"data get entity @e[type=minecraft:item,tag={tag},limit=1] Age",
                        "Stone has the following entity data: ")
            ticks = int(age.rsplit(":", 1)[1].strip().rstrip("s"))
            if ticks < 100:
                sys.exit(f"probe: case {i} cell did not tick (witness age {ticks}); nothing was measured")
            hp = r.cmd(f"data get entity @e[type=minecraft:villager,tag={tag},limit=1] Health",
                       "Villager has the following entity data: ", "No entity was found")
            if "No entity was found" in hp:
                health = 0.0
            else:
                health = float(hp.rsplit(":", 1)[1].strip().rstrip("f"))
            engaged = health < 20.0
            expect_engaged = wet != "land" or (t, w) not in DISENGAGED
            ok = engaged == expect_engaged
            disagree += not ok
            print(f"{t:<9} {w:<8} {wet:<8} {health:<16} "
                  f"{'yes' if engaged else 'no':<8} {'agrees' if ok else 'DISAGREES'}  ({day})")
            r.cmd(f"kill @e[tag={tag}]", "Killed ")
            engaged_cases += engaged
        # Cross-read: every villager counted as struck died to a drowned, by the
        # server's own death messages — not to lightning, cramming or the cleanup.
        slain = (a.work / "server.log").read_text().count("was slain by Drowned")
        if slain != engaged_cases:
            sys.exit(f"probe: {engaged_cases} case(s) read as engaged but the log records "
                     f"{slain} villager(s) slain by a drowned")
        print(f"{len(cases)} case(s) measured, {engaged_cases} engaged ({slain} slain by a "
              f"drowned in the log), {disagree} disagreement(s) with the committed table")
        return 1 if disagree else 0
    finally:
        if proc.poll() is None:
            proc.stdin.write(b"stop\n")
            proc.stdin.flush()
        try:
            proc.wait(timeout=60)
        except subprocess.TimeoutExpired:
            proc.kill()


if __name__ == "__main__":
    sys.exit(main())
