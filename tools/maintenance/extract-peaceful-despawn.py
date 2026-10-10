#!/usr/bin/env python3
"""Regenerate `crates/dsl/data/peaceful-despawn-1.21.11.json` from the PINNED
Minecraft server jar: the entity types the game discards while the world's
difficulty is peaceful.

The engine derives the world difficulty when a campaign declares none, and the
derivation must never pick a difficulty that removes a body the campaign stages.
Which bodies peaceful removes is a fact about the game, so it is read from the
game, never written down by hand.

## What the game does (1.21.11, read from the bytecode)

Every ticked entity runs `Entity#checkDespawn()`. Five classes declare it:

- `Entity` — empty: the body is never discarded by it.
- `Mob` and `WitherBoss` — `if (level.getDifficulty() == PEACEFUL &&
  !getType().isAllowedInPeaceful()) discard();` — the body is discarded on
  peaceful unless its type is allowed (`EntityType.Builder#notInPeaceful()`
  clears the flag). `NoAI`, `PersistenceRequired` and a `/summon` origin are
  not consulted.
- `EnderDragon` — empty: never discarded.
- `ShulkerBullet` — `if (level.getDifficulty() == PEACEFUL) discard();` —
  always discarded on peaceful.

This tool asserts that list is still the whole list (from the mappings), checks
each implementation's shape structurally in its bytecode, and only then
classifies every registered type by the implementation it runs.

## Two methods

1. **Runtime**: the dumper boots the registries and asks every type for
   `isAllowedInPeaceful()` and for the class declaring its `checkDespawn()`.
2. **Static**: `EntityType.<clinit>` is disassembled and every registration
   whose builder chain calls `notInPeaceful()` is collected.

The not-allowed set from (1) must equal the set from (2), or the tool refuses.

    tools/maintenance/extract-peaceful-despawn.py            # regenerate the table
    tools/maintenance/extract-peaceful-despawn.py --check    # non-zero if it would change

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
JAVA_SRC = ROOT / "tools" / "maintenance" / "peaceful" / "PeacefulDump.java"
OUT_JSON = ROOT / "crates" / "dsl" / "data" / "peaceful-despawn-1.21.11.json"
MANIFEST = "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json"

CLASS_SHARED = "net.minecraft.SharedConstants"
CLASS_BOOTSTRAP = "net.minecraft.server.Bootstrap"
CLASS_BUILTIN = "net.minecraft.core.registries.BuiltInRegistries"
CLASS_REGISTRY = "net.minecraft.core.Registry"
CLASS_ENTITY_TYPE = "net.minecraft.world.entity.EntityType"
CLASS_BUILDER = "net.minecraft.world.entity.EntityType$Builder"
CLASS_ENTITY = "net.minecraft.world.entity.Entity"
CLASS_DIFFICULTY = "net.minecraft.world.Difficulty"

# Every class declaring `checkDespawn()`, and what its body does on peaceful.
# `never`: the body has no peaceful branch. `unless_allowed`: discarded on
# peaceful unless the type's `isAllowedInPeaceful()` holds. `always`: discarded
# on peaceful whatever the type says.
IMPLEMENTATIONS = {
    "net.minecraft.world.entity.Entity": "never",
    "net.minecraft.world.entity.Mob": "unless_allowed",
    "net.minecraft.world.entity.boss.wither.WitherBoss": "unless_allowed",
    "net.minecraft.world.entity.boss.enderdragon.EnderDragon": "never",
    "net.minecraft.world.entity.projectile.ShulkerBullet": "always",
}


def fail(msg: str) -> None:
    sys.exit(f"extract-peaceful-despawn: {msg}")


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


def declarers_of_check_despawn(maps) -> set[str]:
    return {cls for cls, (_, _, ms) in maps.items() if "checkDespawn()" in ms}


def javap(classpath: str, cls: str) -> str:
    proc = subprocess.run(
        [jdk_tool("javap"), "-p", "-c", "-classpath", classpath, cls],
        check=True, capture_output=True, text=True,
    )
    return proc.stdout


def method_body(dump: str, obf_method: str) -> str:
    """The `Code:` listing of the zero-argument method `obf_method` in a javap dump."""
    m = re.search(
        rf"^  \S.*\b{re.escape(obf_method)}\(\);\n    Code:\n(.*?)(?:\n\n|\n\}})",
        dump, re.S | re.M,
    )
    if not m:
        fail(f"javap shows no zero-argument method {obf_method}")
    return m.group(1)


def check_implementation_shapes(maps, classpath: str) -> int:
    """Each `checkDespawn()` body carries exactly the peaceful facts its
    classification claims: a read of `Difficulty.PEACEFUL`, a call of
    `EntityType#isAllowedInPeaceful()`, a call of `Entity#discard()`."""
    m_check = resolve(maps, CLASS_ENTITY, methods=["checkDespawn()"])[1]
    obf_diff, diff_fields, _ = maps[CLASS_DIFFICULTY]
    peaceful = f"Field {obf_diff}.{diff_fields['PEACEFUL']}:"
    obf_et = resolve(maps, CLASS_ENTITY_TYPE)[0]
    allowed = f"{obf_et}.{resolve(maps, CLASS_ENTITY_TYPE, methods=['isAllowedInPeaceful()'])[1]}:()Z"
    discard = f"Method {resolve(maps, CLASS_ENTITY, methods=['discard()'])[1]}:()V"
    want = {
        "never": (False, False, False),
        "unless_allowed": (True, True, True),
        "always": (True, False, True),
    }
    for cls, kind in IMPLEMENTATIONS.items():
        body = method_body(javap(classpath, maps[cls][0]), m_check)
        got = (peaceful in body, allowed in body, discard in body)
        if got != want[kind]:
            fail(
                f"{cls}.checkDespawn() reads (PEACEFUL, isAllowedInPeaceful, discard) = "
                f"{got}, but it is classified `{kind}` {want[kind]} — re-read it"
            )
    return len(IMPLEMENTATIONS)


def static_not_in_peaceful(maps, classpath: str) -> set[str]:
    """Method 2: every registration in `EntityType.<clinit>` whose builder chain
    calls `notInPeaceful()`."""
    obf_et = resolve(maps, CLASS_ENTITY_TYPE)[0]
    obf_builder = resolve(maps, CLASS_BUILDER)[0]
    m_not = resolve(maps, CLASS_BUILDER, methods=["notInPeaceful()"])[1]
    m_reg = maps[CLASS_ENTITY_TYPE][2][
        f"register(java.lang.String,{CLASS_BUILDER})"
    ]
    dump = javap(classpath, obf_et)
    m = re.search(r"^  static \{\};\n    Code:\n(.*?)(?:\n\n|\n\}|\Z)", dump, re.S | re.M)
    if not m:
        fail("javap shows no EntityType static initialiser")
    reg_call = re.compile(
        rf"invokestatic .*Method {re.escape(m_reg)}:\(Ljava/lang/String;L{re.escape(obf_builder)};\)L{re.escape(obf_et)};"
    )
    not_call = f"Method {obf_builder}.{m_not}:()L{obf_builder};"
    out: set[str] = set()
    segment: list[str] = []
    registrations = 0
    for line in m.group(1).splitlines():
        segment.append(line)
        if reg_call.search(line):
            registrations += 1
            ids = [
                s for ln in segment
                for s in re.findall(r"// String (\S+)$", ln)
            ]
            if not ids:
                fail("a registration in EntityType.<clinit> carries no string id")
            if any(not_call in ln for ln in segment):
                out.add(f"minecraft:{ids[0]}")
            segment = []
    if registrations == 0:
        fail("found zero registrations in EntityType.<clinit>")
    sys.stderr.write(f"  static scan: {registrations} registrations, {len(out)} call notInPeaceful()\n")
    return out


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--check", action="store_true", help="non-zero if the table would change")
    ap.add_argument("--work", help="cache directory for the jar and mappings")
    args = ap.parse_args()

    work = (
        pathlib.Path(args.work).resolve()
        if args.work
        else pathlib.Path(tempfile.mkdtemp(prefix="dwpeaceful-"))
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
    declarers = declarers_of_check_despawn(maps)
    if declarers != set(IMPLEMENTATIONS):
        fail(
            f"the classes declaring checkDespawn() are {sorted(declarers)}, this tool "
            f"classifies {sorted(IMPLEMENTATIONS)} — read the new one before regenerating"
        )
    shapes = check_implementation_shapes(maps, str(inner[0]))
    sys.stderr.write(f"  {shapes} checkDespawn() implementations match their classification\n")

    dump_args = []
    dump_args += resolve(maps, CLASS_SHARED, methods=["tryDetectVersion()"])
    dump_args += resolve(maps, CLASS_BOOTSTRAP, methods=["bootStrap()"])
    dump_args += resolve(maps, CLASS_BUILTIN, fields=["ENTITY_TYPE"])
    dump_args += resolve(maps, CLASS_REGISTRY, methods=["getKey"])
    dump_args += resolve(maps, CLASS_ENTITY_TYPE, methods=["isAllowedInPeaceful()"])
    dump_args += resolve(maps, CLASS_ENTITY, methods=["checkDespawn()"])[1:]

    classes = work / "classes"
    classes.mkdir(exist_ok=True)
    subprocess.run(
        [jdk_tool("javac"), "-nowarn", "-cp", classpath, "-d", str(classes), str(JAVA_SRC)],
        check=True,
    )
    table = work / "peaceful.tsv"
    proc = subprocess.run(
        [jdk_tool("java"), "-Xmx3g", "-cp", f"{classpath}:{classes}", "dw.PeacefulDump",
         str(table), *dump_args],
        check=True, capture_output=True, text=True, cwd=work,
    )
    counted = re.search(r"DUMPED types=(\d+)", proc.stdout)
    if not counted or int(counted.group(1)) == 0:
        fail(f"the dumper reported nothing to count: {proc.stdout.strip()!r}")

    obf_to_deobf = {obf: cls for cls, (obf, _, _) in maps.items()}
    rows = [ln.split("\t") for ln in table.read_text(encoding="utf8").splitlines()]
    if len(rows) != int(counted.group(1)):
        fail(f"dumper counted {counted.group(1)} types but wrote {len(rows)} rows")
    removed: set[str] = set()
    not_allowed: set[str] = set()
    for ident, flag, _cls, declarer in rows:
        kind = IMPLEMENTATIONS.get(obf_to_deobf.get(declarer, ""))
        if kind is None:
            fail(f"{ident} runs checkDespawn() from {declarer}, which is not classified")
        if flag == "not_allowed":
            not_allowed.add(ident)
        if kind == "always" or (kind == "unless_allowed" and flag == "not_allowed"):
            removed.add(ident)

    static = static_not_in_peaceful(maps, str(inner[0]))
    if static != not_allowed:
        fail(
            f"runtime not-allowed {sorted(not_allowed - static)} / static "
            f"{sorted(static - not_allowed)} disagree — the two methods must agree"
        )
    sys.stderr.write(
        f"  runtime and static agree: {len(not_allowed)} of {len(rows)} types are not allowed in peaceful\n"
    )

    text = json.dumps(sorted(removed), indent=2, ensure_ascii=False) + "\n"
    if args.check:
        if OUT_JSON.read_text(encoding="utf8") != text:
            fail(f"{OUT_JSON.relative_to(ROOT)} is stale — regenerate it")
        print(f"extract-peaceful-despawn: up to date ({len(removed)} of {len(rows)} types)")
        return
    OUT_JSON.write_text(text, encoding="utf8")
    print(
        f"extract-peaceful-despawn: wrote {len(removed)} of {len(rows)} entity types "
        f"to {OUT_JSON.relative_to(ROOT)} (Minecraft {pin['version']})"
    )


main()
