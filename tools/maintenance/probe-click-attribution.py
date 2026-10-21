#!/usr/bin/env python3
"""Measure, on the PINNED server, whether vanilla names the player who attacks or
right-clicks a body that cannot be harmed — the puppet the compiler stages
(`NoAI`, `Invulnerable`) and the `minecraft:interaction` hitbox it summons.

The engine answers a press through an advancement criterion rewarded with a
function that runs AS the player (`trigger.rs`, `TriggerAudience::Presser`). The
tree holds that `minecraft:player_interacted_with_entity` is that criterion for
a right-click and that no criterion exists for a left-click (`DW0427`). This tool
asks the running game instead of the record, for each body shape the compiler
emits, in the game mode a delve ships (adventure):

  mob    a `minecraft:zombie` puppet, `NoAI`+`Invulnerable`, alone
  box    a `minecraft:interaction` `{width:1.0f,height:2.0f,response:1b,Invulnerable:1b}`, alone
  live   a `minecraft:zombie` puppet that is NOT invulnerable — the positive
         control: if `player_hurt_entity` does not fire here the bot's attack
         never reached the server and nothing below was measured (exit 2)
  pair   the puppet and the box on one cell, as `emit/npc.rs` co-locates them

Per body a datapack carries one advancement per criterion, each gated to that
body by its tag and rewarded with a function that counts on the acting player
(`@s`) and revokes itself. A mineflayer bot (`probe-click-attribution.mjs`)
attacks and right-clicks each body by entity; the driver reads the scores and —
the second method, sharing no configuration with the criterion — the
`attack` / `interaction` record vanilla writes on the interaction entity, whose
`player` must equal the bot's own UUID.

    tools/maintenance/probe-click-attribution.py --jar SERVER_JAR --work DIR \
        --node-modules DIR [--java JAVA] [--node NODE]

`--node-modules` is a `node_modules` directory carrying mineflayer at the
version `harness/package.json` pins. The jar is refused unless its sha256 is the
`versions.toml` pin. Requires a JDK >= 21 and free local ports 25995/25996;
`--work` must not exist. Maintenance tool: CI reads the committed table
(`EXPECT`) and never runs this. Exit 0 when the game agrees with the table, 1
when it disagrees, 2 when the control shows nothing was measured.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import select
import shutil
import socket
import struct
import subprocess
import sys
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "lib"))
from versions import pin as versions_pin  # noqa: E402

RCON_PORT, RCON_PASS, GAME_PORT = 25995, "probe", 25996
BOT = "Probe"
FLOOR_Y = -61  # the flat preset's surface; bodies stand one above it
STAND_Y = FLOOR_Y + 1

# Every body the rig stages: name -> (x, z, summon lines). Tags are what the
# advancements key on, so a criterion fires for exactly one body.
PUPPET = "NoAI:1b,Invulnerable:1b,Silent:1b,PersistenceRequired:1b"
BOX = "width:1.0f,height:2.0f,response:1b,Invulnerable:1b"
BODIES: dict[str, tuple[float, float, list[str]]] = {
    "mob": (2.5, 6.5, [f"summon minecraft:zombie {{x}} {STAND_Y} {{z}} {{{{{PUPPET},Tags:[\"probe_mob\"]}}}}"]),
    "box": (5.5, 6.5, [f"summon minecraft:interaction {{x}} {STAND_Y} {{z}} {{{{{BOX},Tags:[\"probe_box\"]}}}}"]),
    "live": (8.5, 6.5, [f"summon minecraft:zombie {{x}} {STAND_Y} {{z}} {{{{NoAI:1b,Silent:1b,PersistenceRequired:1b,Tags:[\"probe_live\"]}}}}"]),
    "pair": (11.5, 6.5, [
        f"summon minecraft:zombie {{x}} {STAND_Y} {{z}} {{{{{PUPPET},Tags:[\"probe_pair_mob\"]}}}}",
        f"summon minecraft:interaction {{x}} {STAND_Y} {{z}} {{{{{BOX},Tags:[\"probe_pair_box\"]}}}}",
    ]),
}

# Advancement name -> (criterion, the tag it is gated to).
ADVANCEMENTS: dict[str, tuple[str, str]] = {
    "attack_mob": ("minecraft:player_hurt_entity", "probe_mob"),
    "use_mob": ("minecraft:player_interacted_with_entity", "probe_mob"),
    "attack_box": ("minecraft:player_hurt_entity", "probe_box"),
    "use_box": ("minecraft:player_interacted_with_entity", "probe_box"),
    "attack_live": ("minecraft:player_hurt_entity", "probe_live"),
    "attack_pair_box": ("minecraft:player_hurt_entity", "probe_pair_box"),
    "attack_pair_mob": ("minecraft:player_hurt_entity", "probe_pair_mob"),
    "use_pair_box": ("minecraft:player_interacted_with_entity", "probe_pair_box"),
    "use_pair_mob": ("minecraft:player_interacted_with_entity", "probe_pair_mob"),
}

# The acts, in order: (act, body, mineflayer entity name the bot targets).
ACTS: list[tuple[str, str, str]] = [
    ("attack", "mob", "zombie"),
    ("use", "mob", "zombie"),
    ("attack", "box", "interaction"),
    ("use", "box", "interaction"),
    ("attack", "live", "zombie"),
    ("attack", "pair", "interaction"),
    ("use", "pair", "interaction"),
]

# The committed table: after each act, which advancements have fired so far
# (cumulative counts), and which interaction record names the bot. Written down
# after it was measured; the tool exits 1 when the game disagrees.
EXPECT: dict[str, dict[str, int]] = {
    "attack mob": {},
    "use mob": {},
    "attack box": {"attack_box": 1},
    "use box": {"attack_box": 1, "use_box": 1},
    "attack live": {"attack_box": 1, "use_box": 1, "attack_live": 1},
    "attack pair": {"attack_box": 1, "use_box": 1, "attack_live": 1, "attack_pair_box": 1},
    "use pair": {"attack_box": 1, "use_box": 1, "attack_live": 1, "attack_pair_box": 1, "use_pair_box": 1},
}
# Which `(record, tag)` must name the bot after which act.
EXPECT_RECORD: dict[str, tuple[str, str]] = {
    "attack box": ("attack", "probe_box"),
    "use box": ("interaction", "probe_box"),
    "attack pair": ("attack", "probe_pair_box"),
    "use pair": ("interaction", "probe_pair_box"),
}


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


def write_datapack(world: pathlib.Path) -> None:
    pack = world / "datapacks" / "probe"
    (pack / "data" / "probe" / "advancement").mkdir(parents=True)
    (pack / "data" / "probe" / "function").mkdir(parents=True)
    # 1.21.11's format (compiler::PACK_FORMAT): `min_format`/`max_format` are mandatory.
    (pack / "pack.mcmeta").write_text(json.dumps({"pack": {
        "description": "click attribution probe", "pack_format": 94,
        "min_format": [94, 1], "max_format": [94, 1]}}))
    for name, (criterion, tag) in ADVANCEMENTS.items():
        adv = {
            "criteria": {"act": {"trigger": criterion, "conditions": {"entity": [{
                "condition": "minecraft:entity_properties", "entity": "this",
                "predicate": {"nbt": f"{{Tags:[\"{tag}\"]}}"}}]}}},
            "rewards": {"function": f"probe:{name}"},
        }
        (pack / "data" / "probe" / "advancement" / f"{name}.json").write_text(json.dumps(adv))
        (pack / "data" / "probe" / "function" / f"{name}.mcfunction").write_text(
            f"scoreboard players add @s probe.{name} 1\n"
            f"advancement revoke @s only probe:{name}\n")


class Bot:
    def __init__(self, node: str, script: pathlib.Path, work: pathlib.Path,
                 node_modules: pathlib.Path) -> None:
        home = work / "bot"
        home.mkdir()
        shutil.copy(script, home / script.name)
        os.symlink(node_modules.resolve(), home / "node_modules")
        self.log = open(work / "bot.log", "w")
        self.proc = subprocess.Popen(
            [node, str(home / script.name), "127.0.0.1", str(GAME_PORT), BOT],
            cwd=home, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.log, text=True)

    def line(self, timeout: float) -> str:
        assert self.proc.stdout is not None
        ready, _, _ = select.select([self.proc.stdout], [], [], timeout)
        if not ready:
            sys.exit(f"probe: bot said nothing for {timeout}s")
        out = self.proc.stdout.readline().rstrip("\n")
        if not out:
            sys.exit("probe: bot exited")
        if out.startswith("error"):
            sys.exit(f"probe: bot: {out}")
        return out

    def act(self, verb: str, name: str, x: float, z: float) -> None:
        assert self.proc.stdin is not None
        self.proc.stdin.write(f"{verb} {name} {x} {STAND_Y} {z}\n")
        self.proc.stdin.flush()
        got = self.line(30)
        if got != f"done {verb} {name}":
            sys.exit(f"probe: bot answered {got!r} to `{verb} {name}`")

    def census(self) -> int:
        assert self.proc.stdin is not None
        self.proc.stdin.write("census\n")
        self.proc.stdin.flush()
        got = self.line(30)
        if not got.startswith("entities "):
            sys.exit(f"probe: bot answered {got!r} to `census`")
        return int(got.split()[1])

    def quit(self) -> None:
        if self.proc.poll() is None and self.proc.stdin is not None:
            try:
                self.proc.stdin.write("quit\n")
                self.proc.stdin.flush()
                self.proc.wait(timeout=10)
            except (BrokenPipeError, subprocess.TimeoutExpired):
                self.proc.kill()


def uuid_of(reply: str) -> list[int]:
    """`[I;a,b,c,d]` out of a `data get` reply."""
    inner = reply[reply.rindex("[I;") + 3:reply.rindex("]")]
    return [int(v.strip()) for v in inner.split(",")]


def scores(r: Rcon) -> dict[str, int]:
    out: dict[str, int] = {}
    for name in ADVANCEMENTS:
        reply = r.cmd(f"scoreboard players get {BOT} probe.{name}",
                      f"{BOT} has ", f"Can't get value of probe.{name} for {BOT}")
        if reply.startswith(f"{BOT} has "):
            out[name] = int(reply.split()[2])
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--jar", required=True, type=pathlib.Path)
    ap.add_argument("--work", required=True, type=pathlib.Path)
    ap.add_argument("--node-modules", required=True, type=pathlib.Path)
    ap.add_argument("--java", default="java")
    ap.add_argument("--node", default="node")
    a = ap.parse_args()

    want = versions_pin("minecraft", "server_jar_sha256")
    got = hashlib.sha256(a.jar.read_bytes()).hexdigest()
    if got != want:
        sys.exit(f"probe: {a.jar} is sha256 {got}, pin is {want}")
    if not (a.node_modules / "mineflayer" / "package.json").is_file():
        sys.exit(f"probe: {a.node_modules} carries no mineflayer")
    a.work.mkdir(parents=True, exist_ok=False)
    (a.work / "eula.txt").write_text("eula=true\n")
    (a.work / "server.properties").write_text(
        "level-type=minecraft\\:flat\nenable-rcon=true\n"
        f"rcon.port={RCON_PORT}\nrcon.password={RCON_PASS}\nserver-port={GAME_PORT}\n"
        "online-mode=false\ndifficulty=normal\ngenerate-structures=false\n"
        "spawn-protection=0\nmax-tick-time=-1\npause-when-empty-seconds=-1\n"
        "gamemode=adventure\nforce-gamemode=true\nenforce-secure-profile=false\n")
    write_datapack(a.work / "world")
    log = open(a.work / "server.log", "w")
    proc = subprocess.Popen([a.java, "-Xmx2G", "-jar", str(a.jar.resolve()), "nogui"],
                            cwd=a.work, stdout=log, stderr=subprocess.STDOUT, stdin=subprocess.PIPE)
    bot: Bot | None = None
    try:
        for _ in range(240):
            if "Done (" in (a.work / "server.log").read_text():
                break
            if proc.poll() is not None:
                sys.exit("probe: server exited before Done")
            time.sleep(1)
        else:
            sys.exit("probe: server never reported Done")
        server_log = (a.work / "server.log").read_text()
        bad = [l for l in server_log.splitlines() if "probe:" in l and ("Couldn't" in l or "Failed" in l)]
        if bad:
            sys.exit("probe: the datapack did not load whole:\n" + "\n".join(bad))
        r = Rcon(RCON_PORT, RCON_PASS)
        packs = r.cmd("datapack list enabled", "There are ")
        if "[file/probe" not in packs:
            sys.exit(f"probe: datapack not enabled: {packs!r}")
        for g in ("advance_time", "advance_weather", "spawn_mobs"):
            r.cmd(f"gamerule {g} false", "Gamerule ")
        r.cmd("time set midnight", "Set the time to ")  # a zombie puppet in daylight burns
        r.cmd("weather clear", "Set the weather to ")
        r.cmd("forceload add 0 0 15 15", "Marked chunk ", "Marked ")
        r.cmd("kill @e[type=!player]", "Killed ", "No entity was found")
        filled = "Successfully filled"
        r.cmd(f"fill 0 {FLOOR_Y} 0 15 {FLOOR_Y} 15 minecraft:stone", filled)
        r.cmd(f"fill 0 {STAND_Y} 0 15 {STAND_Y + 6} 15 minecraft:air", filled, "No blocks were filled")
        r.cmd(f"setworldspawn 7 {STAND_Y} 2", "Set the world spawn point to ")
        for name in ADVANCEMENTS:
            r.cmd(f"scoreboard objectives add probe.{name} dummy", "Created new objective ")
        for x, z, lines in BODIES.values():
            for line in lines:
                r.cmd(line.format(x=x, z=z), "Summoned new ")
        # A body summoned into a freshly forceloaded chunk joins the loaded set a
        # tick or two later; the census waits for it.
        time.sleep(2)
        count = r.cmd("execute if entity @e[type=!player]", "Test passed. Count: ")
        staged = sum(len(l) for _, _, l in BODIES.values())
        if count != f"Test passed. Count: {staged}":
            sys.exit(f"probe: staged {staged} bodies, the world holds {count!r}")

        bot = Bot(a.node, pathlib.Path(__file__).with_suffix(".mjs"), a.work, a.node_modules)
        joined = bot.line(90)
        if joined != f"joined {BOT}":
            sys.exit(f"probe: bot announced {joined!r}")
        # The server is `gamemode=adventure` + `force-gamemode=true`; the command on a
        # player already in that mode answers nothing (measured), so the mode is
        # asserted by selector instead of by the reply.
        r.cmd(f"gamemode adventure {BOT}", "Set ", "")
        r.cmd(f"execute if entity @a[name={BOT},gamemode=adventure]", "Test passed")
        bot_uuid = uuid_of(r.cmd(f"data get entity {BOT} UUID", f"{BOT} has the following entity data: "))
        # The bot must see the whole rig before it acts: the server streams the
        # bodies after the chunks, so readiness is the bot's own census, not a sleep.
        for _ in range(60):
            if bot.census() == staged:
                break
            time.sleep(0.5)
        else:
            sys.exit(f"probe: the bot sees {bot.census()} of {staged} bodies after 30s")

        disagree = 0
        print(f"bot {BOT} uuid {bot_uuid}, game mode adventure; {staged} bodies staged")
        print("act           fired-so-far                                             record                      table")
        for verb, body, ename in ACTS:
            x, z, _ = BODIES[body]
            # Two blocks south of the body, facing it: inside the 3-block entity reach.
            r.cmd(f"tp {BOT} {x} {STAND_Y} {z - 2.0}", "Teleported ")
            time.sleep(0.5)
            bot.act(verb, ename, x, z)
            time.sleep(1.0)
            key = f"{verb} {body}"
            fired = scores(r)
            record = "-"
            record_ok = True
            if key in EXPECT_RECORD:
                field, tag = EXPECT_RECORD[key]
                reply = r.cmd(f"data get entity @e[type=minecraft:interaction,tag={tag},limit=1] {field}",
                              "Interaction has the following entity data: ", "Found no elements")
                if reply.startswith("Found no elements"):
                    record = f"{field}: none"
                    record_ok = False
                else:
                    who = uuid_of(reply[:reply.index("timestamp")]) if "timestamp" in reply else uuid_of(reply)
                    record_ok = who == bot_uuid
                    record = f"{field}: {'the bot' if record_ok else who}"
            ok = fired == EXPECT[key] and record_ok
            disagree += not ok
            fired_s = ", ".join(f"{k}={v}" for k, v in fired.items()) or "nothing"
            print(f"{key:<13} {fired_s:<56} {record:<27} {'agrees' if ok else 'DISAGREES'}")
            if key == "attack live" and fired.get("attack_live", 0) == 0:
                print("probe: the control did not fire — the bot's attack never reached a body; "
                      "nothing above or below is a measurement")
                return 2
        print(f"{len(ACTS)} act(s) measured, {disagree} disagreement(s) with the committed table")
        return 1 if disagree else 0
    finally:
        if bot is not None:
            bot.quit()
        if proc.poll() is None:
            proc.stdin.write(b"stop\n")
            proc.stdin.flush()
        try:
            proc.wait(timeout=60)
        except subprocess.TimeoutExpired:
            proc.kill()


if __name__ == "__main__":
    sys.exit(main())
