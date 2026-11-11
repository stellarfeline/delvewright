#!/usr/bin/env python3
"""Regenerate `crates/dsl/data/entity-steering-1.21.11.json` from the PINNED
Minecraft server jar: every entity type that carries a player rider, the kind
of vehicle it is, how many riders it seats, the medium it moves in and the
item a rider must hold to steer it.

A campaign's `vehicles[]` entry names an entity; the engine derives its kind
(boat, minecart or mount), seats and steering item from this table (spec-0135
§2), so a fact about the game is read from the game, never written by hand.

## The rules (1.21.11)

- **boat**: the type's class extends `AbstractBoat` and not
  `AbstractChestBoat` (a chest boat carries cargo in a seat). Cross-checked
  against the entity tag `#minecraft:boat`, which must name the same set.
- **minecart**: the type's class is `Minecart` itself — the rider's cart; a
  chest, hopper, furnace, spawner, TNT or command-block cart carries no rider.
- **mount**: the type is in `#minecraft:can_equip_saddle` and its class chain
  overrides `LivingEntity#tickRidden(Player, Vec3)` — the game steers it under
  a rider. Cross-checked against `entity-slots-1.21.11.json`: every mount
  draws a `saddle` slot.
- **medium**: `water` for a boat, `rails` for the minecart, and for a mount
  `water` when the type is in `#minecraft:aquatic`, else `ground`.
- **seats**: the riders a party seats by clicking — the bound below, lowered
  to the measured click bound where the record holds one (`SEATS_BY_CLICK`).
  The bound is read from the bytecode of the first `Entity#canAddPassenger`
  on the class chain: `getPassengers().isEmpty()` seats 1; `size() <
  getMaxPassengers()` seats the constant that method returns; `size() <= k`
  seats k + 1; `!isVehicle()` seats 1; any other shape refuses.
- **tame**: a mount whose class extends `AbstractHorse` throws a rider who
  has not tamed it, so it is summoned `Tame`.
- **steering item**: every registered item that is a `FoodOnAStickItem`, by
  the entity type its `canInteractWith` field names. Cross-checked: each such
  type implements `ItemSteerable`, and every mount implementing it has one.

    tools/maintenance/extract-entity-steering.py            # regenerate the table
    tools/maintenance/extract-entity-steering.py --check    # non-zero if it would change

The jar is identified by `versions.toml` and refused unless its sha256 matches
the pin. The mappings come from piston-meta and are sha1-verified. No obfuscated
name is written down here or in the Java dumper.

Requires a JDK >= 21 (`JAVA_HOME`, else `java`/`javac`/`javap` on PATH) and
network access. A maintenance tool: CI reads the committed table and never runs
this. Run it when the MC pin moves.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile
import urllib.request

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "lib"))
from versions import PinError, pin as versions_pin  # noqa: E402

ROOT = pathlib.Path(__file__).resolve().parents[2]
JAVA_SRC = ROOT / "tools" / "maintenance" / "steering" / "SteeringDump.java"
OUT_JSON = ROOT / "crates" / "dsl" / "data" / "entity-steering-1.21.11.json"
TAGS = ROOT / "crates" / "dsl" / "data" / "entity-tags-1.21.11.json"
SLOTS = ROOT / "crates" / "dsl" / "data" / "entity-slots-1.21.11.json"
MANIFEST = "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json"

CLASS_SHARED = "net.minecraft.SharedConstants"
CLASS_BOOTSTRAP = "net.minecraft.server.Bootstrap"
CLASS_BUILTIN = "net.minecraft.core.registries.BuiltInRegistries"
CLASS_REGISTRY = "net.minecraft.core.Registry"
CLASS_ENTITY_TYPE = "net.minecraft.world.entity.EntityType"
CLASS_ENTITY = "net.minecraft.world.entity.Entity"
CLASS_LIVING = "net.minecraft.world.entity.LivingEntity"
CLASS_STEERABLE = "net.minecraft.world.entity.ItemSteerable"
CLASS_FOOD_ON_A_STICK = "net.minecraft.world.item.FoodOnAStickItem"
CLASS_BOAT = "net.minecraft.world.entity.vehicle.boat.AbstractBoat"
CLASS_CHEST_BOAT = "net.minecraft.world.entity.vehicle.boat.AbstractChestBoat"
CLASS_MINECART = "net.minecraft.world.entity.vehicle.minecart.Minecart"
CLASS_HORSE = "net.minecraft.world.entity.animal.equine.AbstractHorse"
SIG_TICK_RIDDEN = "tickRidden(net.minecraft.world.entity.player.Player,net.minecraft.world.phys.Vec3)"
SIG_CAN_ADD = "canAddPassenger(net.minecraft.world.entity.Entity)"
SIG_MAX = "getMaxPassengers()"
CLASS_CAMEL = "net.minecraft.world.entity.animal.camel.Camel"

# Seats a PLAYER takes by a click, where that is fewer than `canAddPassenger`
# admits. Measured on the pinned server, never read from the jar:
# `docs/reference/vehicles-animals-and-targets.md` §12 — a camel seats a second
# player by a click and refuses a third, whom `ride … mount` then seats. Keyed
# by the class that owns the rule, so every type of that class takes it.
SEATS_BY_CLICK = {CLASS_CAMEL: 2}


def fail(msg: str) -> None:
    sys.exit(f"extract-entity-steering: {msg}")


def jdk_tool(name: str) -> str:
    home = os.environ.get("JAVA_HOME")
    if home:
        return str(pathlib.Path(home) / "bin" / name)
    found = shutil.which(name)
    if not found:
        fail(f"no `{name}` on PATH and no JAVA_HOME")
    return found


def read_pin() -> dict[str, str]:
    out = {}
    for key in ("version", "server_jar_url", "server_jar_sha256"):
        try:
            out[key] = versions_pin("minecraft", key)
        except PinError as e:
            fail(str(e))
    return out


def fetch(url: str, dest: pathlib.Path, sha: str | None, algo: str = "sha256") -> None:
    if not dest.exists():
        sys.stderr.write(f"  fetching {url}\n")
        with urllib.request.urlopen(url) as r, open(dest, "wb") as f:
            shutil.copyfileobj(r, f)
    if sha is not None:
        got = hashlib.new(algo, dest.read_bytes()).hexdigest()
        if got != sha:
            fail(f"{dest.name}: {algo} is {got}, pin says {sha} — refusing")
        sys.stderr.write(f"  {dest.name}: {algo} matches the pin\n")


def parse_mappings(path: pathlib.Path):
    """ProGuard mappings -> {deobf class: (obf class, fields, methods)}, where
    methods maps `name(descriptor-args)` and bare `name` to the obf name.
    Fields and methods are kept apart, and unindented `#` lines are skipped."""
    out: dict[str, tuple[str, dict[str, str], dict[str, str]]] = {}
    fields = methods = None
    for raw in path.read_text(encoding="utf8").splitlines():
        if raw.startswith("#"):
            continue
        if raw[:1] not in (" ", "\t"):
            m = re.match(r"^(\S+) -> (\S+):$", raw)
            fields = methods = None
            if m:
                fields, methods = {}, {}
                out[m.group(1)] = (m.group(2), fields, methods)
            continue
        if fields is None or methods is None:
            continue
        m = re.match(r"^(?:\d+:\d+:)?(\S+) (\w+)(\([^)]*\))? -> (\S+)$", raw.strip())
        if m:
            if m.group(3) is None:
                fields.setdefault(m.group(2), m.group(4))
            else:
                methods.setdefault(m.group(2) + m.group(3), m.group(4))
                methods.setdefault(m.group(2), m.group(4))
    return out


def resolve(maps, cls: str, fields=(), methods=()) -> list[str]:
    if cls not in maps:
        fail(f"mappings have no class {cls} — the pin moved under this tool")
    obf, fs, ms = maps[cls]
    got = [obf]
    for kind, names, table in (("field", fields, fs), ("method", methods, ms)):
        for name in names:
            if name not in table:
                fail(f"mappings have no {kind} {cls}.{name}")
            got.append(table[name])
    return got


def javap(classpath: str, cls: str) -> str:
    proc = subprocess.run(
        [jdk_tool("javap"), "-p", "-c", "-classpath", classpath, cls],
        check=True, capture_output=True, text=True,
    )
    return proc.stdout




def first_declarer(maps, chain: list[str], sig: str) -> str | None:
    """The first class of `chain` (deobfuscated names, most derived first) whose
    mappings declare `sig`."""
    for cls in chain:
        if cls in maps and sig in maps[cls][2]:
            return cls
    return None


def code_of(dump: str, header: str) -> str:
    m = re.search(rf"^  \S.*{re.escape(header)};\n    Code:\n(.*?)(?:\n\n|\n\}})", dump, re.S | re.M)
    if not m:
        fail(f"javap shows no method `{header}`")
    return m.group(1)


def seats_of(maps, classpath: str, chain: list[str]) -> int:
    """Read the seat count off the first `canAddPassenger` on the chain."""
    declarer = first_declarer(maps, chain, SIG_CAN_ADD)
    if declarer is None:
        fail(f"no canAddPassenger on the chain {chain}")
    obf_entity = maps[CLASS_ENTITY][0]
    m_can = maps[CLASS_ENTITY][2][SIG_CAN_ADD]
    body = code_of(javap(classpath, maps[declarer][0]), f"boolean {m_can}({obf_entity})")
    ops = [ln.split(":", 1)[1].split()[0] for ln in body.splitlines() if ":" in ln]
    if declarer == CLASS_ENTITY:
        if "isEmpty:()Z" not in body:
            fail("Entity.canAddPassenger no longer reads getPassengers().isEmpty()")
        return 1
    m_max = maps[CLASS_ENTITY][2].get(SIG_MAX) or maps[CLASS_BOAT][2].get(SIG_MAX)
    if m_max and f"Method {m_max}:()I" in body:
        holder = first_declarer(maps, chain, SIG_MAX)
        mbody = code_of(javap(classpath, maps[holder][0]), f"int {m_max}()")
        k = re.match(r"\s*0: iconst_(\d)\n\s*1: ireturn\s*$", mbody)
        if not k or "if_icmpge" not in ops:
            fail(f"{holder}.getMaxPassengers / {declarer}.canAddPassenger is not the shape this tool reads")
        return int(k.group(1))
    m_vehicle = maps[CLASS_ENTITY][2]["isVehicle()"]
    if f"Method {m_vehicle}:()Z" in body and "size:()I" not in body and ops[2:3] == ["ifne"]:
        return 1
    consts = [o for o in ops if o.startswith("iconst_")]
    if consts and "size:()I" in body:
        k = int(consts[0].split("_")[1])
        if "if_icmpgt" in ops:
            return k + 1
        if "if_icmpge" in ops:
            return k
    fail(f"{declarer}.canAddPassenger is not a shape this tool reads:\n{body}")
    return 0


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--check", action="store_true", help="non-zero if the table would change")
    ap.add_argument("--work", help="cache directory for the jar and mappings")
    args = ap.parse_args()

    work = (
        pathlib.Path(args.work).resolve()
        if args.work
        else pathlib.Path(tempfile.mkdtemp(prefix="dwsteering-"))
    )
    work.mkdir(parents=True, exist_ok=True)

    pin = read_pin()
    jar = work / "server.jar"
    fetch(pin["server_jar_url"], jar, pin["server_jar_sha256"])
    manifest = work / "manifest.json"
    fetch(MANIFEST, manifest, None)
    entries = [
        v for v in json.loads(manifest.read_text(encoding="utf8"))["versions"]
        if v["id"] == pin["version"]
    ]
    if not entries:
        fail(f"piston manifest has no version {pin['version']}")
    vjson = work / "version.json"
    fetch(entries[0]["url"], vjson, entries[0]["sha1"], "sha1")
    downloads = json.loads(vjson.read_text(encoding="utf8"))["downloads"]
    if downloads["server"]["url"] != pin["server_jar_url"]:
        fail("piston's server jar url disagrees with versions.toml — refusing")
    mappings = work / "server.txt"
    fetch(downloads["server_mappings"]["url"], mappings,
          downloads["server_mappings"]["sha1"], "sha1")

    bundle = work / "bundle"
    if not bundle.exists():
        subprocess.run(
            ["unzip", "-o", "-q", str(jar), "-d", str(bundle),
             "META-INF/versions/*", "META-INF/libraries/*"],
            check=True,
        )
    inner = list((bundle / "META-INF" / "versions").rglob("*.jar"))
    if len(inner) != 1:
        fail(f"expected one bundled server jar, found {len(inner)}")
    cp = [str(inner[0])] + sorted(str(p) for p in (bundle / "META-INF" / "libraries").rglob("*.jar"))
    classpath = ":".join(cp)

    maps = parse_mappings(mappings)
    dump_args = []
    dump_args += resolve(maps, CLASS_SHARED, methods=["tryDetectVersion()"])
    dump_args += resolve(maps, CLASS_BOOTSTRAP, methods=["bootStrap()"])
    dump_args += resolve(maps, CLASS_BUILTIN, fields=["ENTITY_TYPE", "ITEM"])
    dump_args += resolve(maps, CLASS_REGISTRY, methods=["getKey"])
    dump_args += resolve(maps, CLASS_ENTITY_TYPE)
    dump_args += resolve(maps, CLASS_FOOD_ON_A_STICK, fields=["canInteractWith"])

    classes = work / "steering-classes"
    classes.mkdir(exist_ok=True)
    subprocess.run(
        [jdk_tool("javac"), "-nowarn", "-cp", classpath, "-d", str(classes), str(JAVA_SRC)],
        check=True,
    )
    table = work / "steering.tsv"
    proc = subprocess.run(
        [jdk_tool("java"), "-Xmx3g", "-cp", f"{classpath}:{classes}", "dw.SteeringDump",
         str(table), *dump_args],
        check=True, capture_output=True, text=True, cwd=work,
    )
    counted = re.search(r"DUMPED types=(\d+) steering=(\d+)", proc.stdout)
    if not counted or int(counted.group(1)) == 0:
        fail(f"the dumper reported nothing to count: {proc.stdout.strip()!r}")

    obf_to_deobf = {obf: cls for cls, (obf, _, _) in maps.items()}
    chains: dict[str, list[str]] = {}
    steering: dict[str, str] = {}
    for ln in table.read_text(encoding="utf8").splitlines():
        kind, ident, rest = ln.split("\t")
        if kind == "TYPE":
            chains[ident] = [obf_to_deobf.get(c, c) for c in rest.split(",")]
        else:
            if rest in steering:
                fail(f"two steering items name {rest}")
            steering[rest] = ident
    if len(chains) != int(counted.group(1)) or len(steering) != int(counted.group(2)):
        fail("the dumper's counts disagree with the rows it wrote")

    tags = json.loads(TAGS.read_text(encoding="utf8"))
    slots = json.loads(SLOTS.read_text(encoding="utf8"))
    saddled = set(tags["minecraft:can_equip_saddle"])
    aquatic = set(tags["minecraft:aquatic"])
    boat_tag = set(tags["minecraft:boat"])

    # `ItemSteerable` is an interface: read it off the mappings' class list by
    # asking javap for each chain's interfaces would cost a JVM per class; the
    # steering items name their types, and the cross-check below reads the
    # interface from the mapped class header instead.
    steerable_obf = maps[CLASS_STEERABLE][0]

    out: dict[str, dict] = {}
    for ident, chain in sorted(chains.items()):
        row = None
        if CLASS_BOAT in chain and CLASS_CHEST_BOAT not in chain:
            row = {"kind": "boat", "medium": "water"}
        elif chain[0] == CLASS_MINECART:
            row = {"kind": "minecart", "medium": "rails"}
        elif ident in saddled:
            declarer = first_declarer(maps, chain, SIG_TICK_RIDDEN)
            if declarer is not None and declarer != CLASS_LIVING:
                row = {"kind": "mount", "medium": "water" if ident in aquatic else "ground"}
                if CLASS_HORSE in chain:
                    row["tame"] = True
                if "saddle" not in slots.get(ident, {}).get("slots", {}):
                    fail(f"{ident} is steered and saddled by tag but draws no saddle slot")
        if row is None:
            if ident in steering:
                fail(f"{ident} has a steering item but is no vehicle")
            continue
        row["seats"] = seats_of(maps, str(inner[0]), chain)
        for owner, by_click in SEATS_BY_CLICK.items():
            if owner in chain:
                if by_click > row["seats"]:
                    fail(f"{ident}: {by_click} seats by a click, and the jar admits {row['seats']}")
                row["seats"] = by_click
        if ident in steering:
            header = javap(str(inner[0]), maps[chain[0]][0]).splitlines()
            if not any(steerable_obf in h for h in header[:3]):
                fail(f"{ident} has a steering item but its class does not implement ItemSteerable")
            row["steering_item"] = steering[ident]
        out[ident] = row

    boats = {k for k, v in out.items() if v["kind"] == "boat"}
    if boats != boat_tag:
        fail(f"boats by class {sorted(boats)} and #minecraft:boat {sorted(boat_tag)} disagree")
    missing = set(steering) - set(out)
    if missing:
        fail(f"steering items name types that are not vehicles: {sorted(missing)}")
    kinds = {k: sum(1 for v in out.values() if v["kind"] == k) for k in ("boat", "minecart", "mount")}
    sys.stderr.write(
        f"  {len(chains)} types: {kinds['boat']} boat(s), {kinds['minecart']} minecart(s), "
        f"{kinds['mount']} mount(s); {len(steering)} steering item(s)\n"
    )

    text = json.dumps(out, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    if args.check:
        if OUT_JSON.read_text(encoding="utf8") != text:
            fail(f"{OUT_JSON.relative_to(ROOT)} is stale — regenerate it")
        print(f"extract-entity-steering: up to date ({len(out)} of {len(chains)} types)")
        return
    OUT_JSON.write_text(text, encoding="utf8")
    print(
        f"extract-entity-steering: wrote {len(out)} of {len(chains)} entity types "
        f"to {OUT_JSON.relative_to(ROOT)} (Minecraft {pin['version']})"
    )


main()
