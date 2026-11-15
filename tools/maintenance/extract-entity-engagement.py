#!/usr/bin/env python3
"""Regenerate `crates/delvec/data/entity-engagement-1.21.11.json` from the
PINNED Minecraft server jar: every entity type's mob category and the base
value of its `minecraft:follow_range`.

A wave's mob keeps a target only as far as its `follow_range`, and a lane mob
released to its own AI has its base reset to its species' value (spec-0016 §6,
`docs/reference/lane-engagement.md`): the radius a fight reaches is a fact about
the entity TYPE, so it is read from the game, never written down by hand. The
category (`monster`, `creature`, ...) is the game's own answer to which waves
are hostile, which the engagement PackTest binds to.

## Two methods

1. **Runtime**: the dumper boots the registries and asks every type for
   `EntityType#getCategory()` and, where `DefaultAttributes#hasSupplier` holds
   and the supplier carries the attribute, `AttributeSupplier#getBaseValue` of
   `Attributes.FOLLOW_RANGE` — the value `attribute ... base reset` restores.
2. **Static**: `DefaultAttributes.<clinit>` is disassembled; per supplier put,
   the entity type's field (named to its id through `EntityType.<clinit>`) and
   the static builder method it calls are read, and that method's chain is
   followed until a `FOLLOW_RANGE` add states the value — the outermost add
   wins, as the builder's later `add` replaces the earlier one. An add with no
   value takes the attribute's own default, which must be one value.

Every type with a supplier must be seen by both methods with one value, or the
tool refuses.

    tools/maintenance/extract-entity-engagement.py            # regenerate the table
    tools/maintenance/extract-entity-engagement.py --check    # non-zero if it would change

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
JAVA_SRC = ROOT / "tools" / "maintenance" / "engagement" / "EngagementDump.java"
OUT_JSON = ROOT / "crates" / "delvec" / "data" / "entity-engagement-1.21.11.json"
MANIFEST = "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json"

CLASS_SHARED = "net.minecraft.SharedConstants"
CLASS_BOOTSTRAP = "net.minecraft.server.Bootstrap"
CLASS_BUILTIN = "net.minecraft.core.registries.BuiltInRegistries"
CLASS_REGISTRY = "net.minecraft.core.Registry"
CLASS_ENTITY_TYPE = "net.minecraft.world.entity.EntityType"
CLASS_BUILDER = "net.minecraft.world.entity.EntityType$Builder"
CLASS_CATEGORY = "net.minecraft.world.entity.MobCategory"
CLASS_DEFAULTS = "net.minecraft.world.entity.ai.attributes.DefaultAttributes"
CLASS_SUPPLIER = "net.minecraft.world.entity.ai.attributes.AttributeSupplier"
CLASS_SUPPLIER_BUILDER = "net.minecraft.world.entity.ai.attributes.AttributeSupplier$Builder"
CLASS_ATTRIBUTES = "net.minecraft.world.entity.ai.attributes.Attributes"
CLASS_HOLDER = "net.minecraft.core.Holder"


def fail(msg: str) -> None:
    sys.exit(f"extract-entity-engagement: {msg}")


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



FLOAT_PUSH = re.compile(r"\b(?:fconst_(\d)|ldc(?:_w)?\s+#\d+\s+// float (\S+?)f?)$")
INT_PUSH = re.compile(r"\b(?:iconst_(m1|\d)|bipush\s+(-?\d+)|sipush\s+(-?\d+))$")


def float_of(line: str) -> float | None:
    m = FLOAT_PUSH.search(line.strip())
    if not m:
        return None
    return float(m.group(1)) if m.group(1) is not None else float(m.group(2))


def int_of(line: str) -> int | None:
    m = INT_PUSH.search(line.strip())
    if not m:
        return None
    if m.group(1) is not None:
        return -1 if m.group(1) == "m1" else int(m.group(1))
    return int(m.group(2) or m.group(3))



DOUBLE_PUSH = re.compile(r"\b(?:dconst_(\d)|ldc2_w\s+#\d+\s+// double (\S+?)d?)$")


def double_of(line: str) -> float | None:
    m = DOUBLE_PUSH.search(line.strip())
    if not m:
        return None
    return float(m.group(1)) if m.group(1) is not None else float(m.group(2))


def code_of(dump: str, header: str) -> list[str]:
    """The bytecode lines of the method whose javap header line is `header`."""
    lines = dump.splitlines()
    for k, ln in enumerate(lines):
        if ln.strip() == header:
            out = []
            for body in lines[k + 2:]:
                if not body.strip() or not re.match(r"^\s+\d+:", body):
                    break
                out.append(body)
            return out
    fail(f"javap shows no method `{header}`")
    return []


def type_ids(maps, classpath: str) -> dict[str, str]:
    """EntityType's obf static field -> registry id, from `EntityType.<clinit>`:
    each registration's string id followed by the `putstatic` it fills."""
    obf_et = resolve(maps, CLASS_ENTITY_TYPE)[0]
    code = code_of(javap(classpath, obf_et), "static {};")
    out: dict[str, str] = {}
    pending: str | None = None
    for ln in code:
        m = re.search(r"// String (\S+)$", ln)
        if m and pending is None:
            pending = m.group(1)
        m = re.search(rf"putstatic\s+#\d+\s+// Field (\w+):L{re.escape(obf_et)};", ln)
        if m:
            if pending is None:
                fail(f"EntityType field {m.group(1)} is filled with no string id before it")
            out[m.group(1)] = f"minecraft:{pending}"
            pending = None
    if not out:
        fail("found zero registrations in EntityType.<clinit>")
    return out


def static_scan(maps, classpath: str) -> dict[str, float | None]:
    """Method 2: per supplier in `DefaultAttributes.<clinit>`, the follow range
    its builder chain states (`None` when the chain adds none)."""
    obf_et = resolve(maps, CLASS_ENTITY_TYPE)[0]
    obf_defaults = resolve(maps, CLASS_DEFAULTS)[0]
    obf_sb = resolve(maps, CLASS_SUPPLIER_BUILDER)[0]
    obf_attrs, f_follow = resolve(maps, CLASS_ATTRIBUTES, fields=["FOLLOW_RANGE"])
    ids = type_ids(maps, classpath)
    code = code_of(javap(classpath, obf_defaults), "static {};")
    follow_get = f"Field {obf_attrs}.{f_follow}:"
    builder_ret = re.compile(
        rf"invokestatic\s+#\d+\s+// (?:Interface)?Method (?:([\w$/]+)\.)?(\w+):\(\)L{re.escape(obf_sb)};"
    )
    memo: dict[tuple[str, str], tuple[bool, float | None]] = {}

    def chain(cls: str, meth: str, depth: int = 0) -> tuple[bool, float | None]:
        """(stated, value) for `cls.meth()`'s chain; value None = the default."""
        if depth > 16:
            fail(f"builder chain through {cls}.{meth} is deeper than 16")
        key = (cls, meth)
        if key in memo:
            return memo[key]
        # A static method is resolved up the class chain, as the JVM resolves
        # an `invokestatic` naming a subclass.
        owner = cls.replace("/", ".")
        header = None
        while header is None:
            dump = javap(classpath, owner)
            for ln in dump.splitlines():
                s = ln.strip()
                if re.match(rf"^(?:public |protected |private )?static [\w.$]+ {re.escape(meth)}\(\);$", s):
                    header = s
                    break
            if header is None:
                sup = re.search(r"^(?:public |abstract |final |\s)*class \S+ extends (\S+)", dump, re.M)
                if not sup:
                    fail(f"javap shows no static {cls}.{meth}() on its class chain")
                owner = sup.group(1)
        body = code_of(dump, header)
        stated: tuple[bool, float | None] | None = None
        for k, ln in enumerate(body):
            if follow_get in ln and "getstatic" in ln:
                nxt = body[k + 1] if k + 1 < len(body) else ""
                v = double_of(nxt)
                stated = (True, v)
        if stated is None:
            inner = next((builder_ret.search(ln) for ln in body if builder_ret.search(ln)), None)
            # javap names no class for a method of the class it is reading.
            stated = (
                chain(inner.group(1) or owner, inner.group(2), depth + 1) if inner else (False, None)
            )
        memo[key] = stated
        return stated

    out: dict[str, float | None] = {}
    pending_type: str | None = None
    for ln in code:
        m = re.search(rf"getstatic\s+#\d+\s+// Field {re.escape(obf_et)}\.(\w+):L{re.escape(obf_et)};", ln)
        if m:
            pending_type = m.group(1)
            continue
        m = builder_ret.search(ln)
        if m and pending_type is not None:
            if pending_type not in ids:
                fail(f"DefaultAttributes names EntityType field {pending_type}, which registers no id")
            if m.group(1) is None:
                fail(f"a DefaultAttributes supplier for {pending_type} calls a builder of its own class")
            stated, v = chain(m.group(1), m.group(2))
            out[ids[pending_type]] = v if stated else "absent"  # type: ignore[assignment]
            pending_type = None
    if not out:
        fail("found zero suppliers in DefaultAttributes.<clinit>")
    return out


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--check", action="store_true", help="non-zero if the table would change")
    ap.add_argument("--work", help="cache directory for the jar and mappings")
    args = ap.parse_args()

    work = (
        pathlib.Path(args.work).resolve()
        if args.work
        else pathlib.Path(tempfile.mkdtemp(prefix="dwengage-"))
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
    dump_args += resolve(maps, CLASS_BUILTIN, fields=["ENTITY_TYPE"])
    dump_args += resolve(maps, CLASS_REGISTRY, methods=["getKey"])
    dump_args += resolve(maps, CLASS_ENTITY_TYPE, methods=["getCategory()"])
    dump_args += resolve(maps, CLASS_CATEGORY, methods=["getName()"])
    dump_args += resolve(
        maps, CLASS_DEFAULTS,
        methods=[f"hasSupplier({CLASS_ENTITY_TYPE})", f"getSupplier({CLASS_ENTITY_TYPE})"],
    )
    dump_args += resolve(
        maps, CLASS_SUPPLIER,
        methods=[f"hasAttribute({CLASS_HOLDER})", f"getBaseValue({CLASS_HOLDER})"],
    )
    dump_args += resolve(maps, CLASS_ATTRIBUTES, fields=["FOLLOW_RANGE"])

    classes = work / "classes"
    classes.mkdir(exist_ok=True)
    subprocess.run(
        [jdk_tool("javac"), "-nowarn", "-cp", classpath, "-d", str(classes), str(JAVA_SRC)],
        check=True,
    )
    table = work / "engagement.tsv"
    proc = subprocess.run(
        [jdk_tool("java"), "-Xmx3g", "-cp", f"{classpath}:{classes}", "dw.EngagementDump",
         str(table), *dump_args],
        check=True, capture_output=True, text=True, cwd=work,
    )
    counted = re.search(r"DUMPED types=(\d+) ranged=(\d+)", proc.stdout)
    if not counted or int(counted.group(1)) == 0 or int(counted.group(2)) == 0:
        fail(f"the dumper reported nothing to count: {proc.stdout.strip()!r}")
    rows = [ln.split("\t") for ln in table.read_text(encoding="utf8").splitlines()]
    if len(rows) != int(counted.group(1)):
        fail(f"dumper counted {counted.group(1)} types but wrote {len(rows)} rows")
    runtime: dict[str, dict[str, object]] = {}
    for ident, category, rng in rows:
        row: dict[str, object] = {"category": category}
        if rng != "-":
            row["follow_range"] = float(rng)
        runtime[ident] = row
    ranged = sum(1 for r in runtime.values() if "follow_range" in r)
    if ranged != int(counted.group(2)):
        fail(f"dumper counted {counted.group(2)} ranged types but wrote {ranged}")

    static = static_scan(maps, str(inner[0]))
    defaults: set[float] = set()
    stated = 0
    for ident, v in static.items():
        if ident not in runtime:
            fail(f"{ident}: a supplier the runtime registry does not know")
        rt = runtime[ident].get("follow_range")
        if v == "absent":
            if rt is not None:
                fail(f"{ident}: no FOLLOW_RANGE in its chain, but the runtime reads {rt}")
            continue
        if v is None:
            if rt is None:
                fail(f"{ident}: a valueless FOLLOW_RANGE add, but the runtime carries none")
            defaults.add(rt)  # type: ignore[arg-type]
            continue
        stated += 1
        if rt != v:
            fail(f"{ident}: static follow_range {v} / runtime {rt} disagree")
    runtime_ranged = {i for i, r in runtime.items() if "follow_range" in r}
    static_ranged = {i for i, v in static.items() if v != "absent"}
    if runtime_ranged != static_ranged:
        fail(
            f"runtime-only {sorted(runtime_ranged - static_ranged)} / static-only "
            f"{sorted(static_ranged - runtime_ranged)} — the two methods must see one set"
        )
    if len(defaults) > 1:
        fail(f"valueless FOLLOW_RANGE adds take {sorted(defaults)} — an attribute default is ONE value")
    sys.stderr.write(
        f"  runtime and static agree over {len(static_ranged)} ranged types of {len(rows)}: "
        f"{stated} state a value, the rest take the attribute default {sorted(defaults)}\n"
    )

    text = json.dumps(runtime, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    if args.check:
        if OUT_JSON.read_text(encoding="utf8") != text:
            fail(f"{OUT_JSON.relative_to(ROOT)} is stale — regenerate it")
        print(f"extract-entity-engagement: up to date ({len(rows)} types)")
        return
    OUT_JSON.write_text(text, encoding="utf8")
    print(
        f"extract-entity-engagement: wrote {len(rows)} entity types ({ranged} with a follow "
        f"range) to {OUT_JSON.relative_to(ROOT)} (Minecraft {pin['version']})"
    )


main()
