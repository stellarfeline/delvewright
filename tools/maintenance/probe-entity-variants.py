#!/usr/bin/env python3
"""Measure, on the PINNED server, that every body variant the engine writes
reads back as written.

`crates/dsl/data/entity-variants-1.21.11.json` (written by
`extract-entity-variants.py` from the jar's bytecode) states, for every entity
type, each variant property's values and the NBT a `/summon` writes it under.
This is the second method: it shares nothing with the bytecode reading but the
table under test. For every entity type, every property and every value it
summons a body with that value written as the table says, then asks the
running game for the body's saved data at that key (`data get entity`). The
game saves from the body's state, so a key it did not read, a form it did not
accept or an id it mapped elsewhere does not come back as written. Then one
body per type carrying every property at once (the packed keys combine), and
one carrying none (the defaults, printed).

    tools/maintenance/probe-entity-variants.py --jar SERVER_JAR --work DIR [--java JAVA]

The jar is refused unless its sha256 is the `versions.toml` pin. Requires a JDK
>= 21. Maintenance tool: never run by CI; run it when the MC pin moves or the
table changes. Exits 1 when the game disagrees with the table, or when nothing
was measured; the record is `docs/reference/body-variants.md`.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import socket
import struct
import subprocess
import sys
import time

ROOT = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools" / "lib"))
from versions import pin as versions_pin  # noqa: E402

TABLE = ROOT / "crates" / "dsl" / "data" / "entity-variants-1.21.11.json"
RCON_PORT, RCON_PASS = 25987, "probe"
AT = "0.5 -60 3.5"
# A painting hangs on a wall: the stone face at z = 0, the body one block south.
WALL = "fill -8 -61 0 8 -50 0 minecraft:stone"
PAINTING_AT = "0 -57 1"


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
        if struct.unpack("<i", data[:4])[0] == -1:
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
        reply = self._send(2, c)
        if not reply.startswith(ok):
            sys.exit(f"probe: `{c}` answered {reply!r}, expected one of {ok!r}")
        return reply


def written(table: dict, entity: str, chosen: dict[str, str]) -> dict[str, object]:
    """Key path -> the value written, before SNBT: what the table says a summon
    carries for `chosen` (property -> value name). Packed numeric keys OR their
    parts; an undeclared part contributes nothing."""
    props = table["entities"][entity]
    out: dict[str, object] = {}
    for prop, value in chosen.items():
        e = props[prop]
        entry = next(v for v in table["domains"][e["domain"]] if v["name"] == value)
        for n in e["nbt"]:
            if n["form"] == "string":
                out[n["key"]] = value
            else:
                out[n["key"]] = int(out.get(n["key"], 0)) | (entry["id"] << n["shift"])
    return out


def snbt(entity: str, table: dict, chosen: dict[str, str]) -> tuple[str, dict[str, str]]:
    """The summon compound and, per key path, the SNBT the game must read back."""
    props = table["entities"][entity]
    forms = {n["key"]: n["form"] for e in props.values() for n in e["nbt"]}
    vals = written(table, entity, chosen)
    expect: dict[str, str] = {}
    nested: dict[str, list[str]] = {}
    flat: list[str] = []
    for key, v in sorted(vals.items()):
        form = forms[key]
        s = f'"{v}"' if form == "string" else (f"{v}b" if form == "byte" else str(v))
        expect[key] = s
        outer, _, inner = key.partition(".")
        if inner:
            nested.setdefault(outer, []).append(f"{inner}:{s}")
        else:
            flat.append(f"{key}:{s}")
    for outer, parts in sorted(nested.items()):
        flat.append(f"{outer}:{{{','.join(parts)}}}")
    return ",".join(flat), expect


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
    table = json.loads(TABLE.read_text(encoding="utf8"))
    a.work.mkdir(parents=True, exist_ok=False)
    (a.work / "eula.txt").write_text("eula=true\n")
    (a.work / "server.properties").write_text(
        "level-type=minecraft\\:flat\nenable-rcon=true\n"
        f"rcon.port={RCON_PORT}\nrcon.password={RCON_PASS}\nserver-port=25988\n"
        "online-mode=false\ngenerate-structures=false\ndifficulty=easy\n"
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
        r.cmd("forceload add -16 -16 16 16", "Marked ")
        r.cmd(WALL, "Successfully filled")

        # The forced chunks load over the next ticks: a body summoned before its
        # chunk ticks is not found. Wait until a sentinel body is.
        for _ in range(100):
            r.cmd("kill @e[tag=dw_vp]", "Killed ", "No entity was found")
            r.cmd(f"summon minecraft:pig {AT} {{Tags:[\"dw_vp\"],NoAI:1b}}", "Summoned new ")
            if r.cmd("data get entity @e[tag=dw_vp,limit=1] NoAI", "").endswith("entity data: 1b"):
                break
            time.sleep(0.2)
        else:
            sys.exit("probe: a summoned body is never found: nothing can be measured")
        r.cmd("kill @e[tag=dw_vp]", "Killed ", "No entity was found")

        reads = disagree = 0
        serial = 0

        def measure(entity: str, chosen: dict[str, str], label: str) -> None:
            nonlocal reads, disagree, serial
            nbt, expect = snbt(entity, table, chosen)
            pos = PAINTING_AT if entity == "minecraft:painting" else AT
            extra = ",facing:3b" if entity == "minecraft:painting" else ",NoAI:1b,Silent:1b"
            # Each body its own tag, so a read can only ever find this one.
            serial += 1
            tag = f"dw_vp{serial}"
            r.cmd("kill @e[tag=dw_vp]", "Killed ", "No entity was found")
            r.cmd(f"summon {entity} {pos} {{Tags:[\"dw_vp\",\"{tag}\"]{extra},{nbt}}}", "Summoned new ")
            # A body is read once it is found: the summon's reply is sent before
            # the body is visible to a selector on some ticks (measured: one
            # body in a few hundred). Up to 2 s; a body never found disagrees.
            for wait in range(10):
                if not r.cmd(f"execute if entity @e[tag={tag}]", "Test passed", "Test failed").startswith("Test failed"):
                    break
                time.sleep(0.2)
            if wait:
                print(f"note: {entity} {label}: found after {wait} wait(s)")
            for key, s in expect.items():
                reply = r.cmd(f"data get entity @e[tag={tag},limit=1] {key}", "")
                reads += 1
                got = reply.split("entity data: ", 1)[1] if "entity data: " in reply else reply
                if got != s:
                    disagree += 1
                    print(f"DISAGREES {entity} {label}: {key} wrote {s}, read {got}")

        for entity, props in sorted(table["entities"].items()):
            n = 0
            for prop, e in sorted(props.items()):
                for v in table["domains"][e["domain"]]:
                    measure(entity, {prop: v["name"]}, f"{prop}={v['name']}")
                    n += 1
            every = {p: table["domains"][e["domain"]][-1]["name"] for p, e in props.items()}
            measure(entity, every, "every property")
            # Defaults: a body summoned with NBT and no variant.
            r.cmd("kill @e[tag=dw_vp]", "Killed ", "No entity was found")
            pos = PAINTING_AT if entity == "minecraft:painting" else AT
            extra = ",facing:3b" if entity == "minecraft:painting" else ""
            r.cmd(f"summon {entity} {pos} {{Tags:[\"dw_vp\"]{extra}}}", "Summoned new ")
            keys = sorted({nn["key"] for e in props.values() for nn in e["nbt"]})
            defaults = []
            for key in keys:
                reply = r.cmd(f"data get entity @e[tag=dw_vp,limit=1] {key}", "")
                defaults.append(f"{key}={reply.split('entity data: ', 1)[-1]}")
            print(f"{entity}: {n} value(s) + 1 combined; defaults {', '.join(defaults)}")
        r.cmd("kill @e[tag=dw_vp]", "Killed ", "No entity was found")
        types = len(table["entities"])
        print(f"{types} entity types, {reads} reads, {disagree} disagreeing")
        if reads == 0:
            print("nothing was measured")
            return 1
        return 1 if disagree else 0
    finally:
        proc.communicate(b"stop\n", timeout=60)


if __name__ == "__main__":
    sys.exit(main())
