#!/usr/bin/env python3
"""Regenerate `crates/dsl/data/item-use-duration-1.21.11.json` from the PINNED
Minecraft server jar: every item's use duration, in ticks.

A `use-item` trigger (spec-0134 §3) fires while a player holds an item in use,
through the `minecraft:using_item` criterion; the game reports an item in use
only while its use duration is above zero. Which items those are is a fact of
the pinned game, read from it here and never written down by hand; `DW1092`
refuses a `use-item` naming an item whose duration is zero.

## Two methods

1. **Runtime**: the dumper boots the registries and asks every item for
   `Item#getUseDuration(ItemStack, LivingEntity)` on its default stack
   (`Item#getDefaultInstance()`), with no entity.
2. **Report**: the same jar's data generator writes `reports/items.json`, every
   item's default components. Every item that carries `minecraft:consumable`
   must take `consume_seconds × 20` ticks at runtime (1.6 s when unstated); every
   item the runtime gives a duration with no consumable component is printed by
   name, so the non-food set can be read.

An item whose answer needs the holding entity (the goat horn reads its
instrument through the holder's registries) is resolved from the jar's own data:
its default `minecraft:instrument` and that instrument's `use_duration` seconds.

    tools/maintenance/extract-item-use-duration.py            # regenerate the table
    tools/maintenance/extract-item-use-duration.py --check    # non-zero if it would change

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
import zipfile

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "lib"))
from versions import PinError, pin as versions_pin  # noqa: E402

ROOT = pathlib.Path(__file__).resolve().parents[2]
JAVA_SRC = ROOT / "tools" / "maintenance" / "usedur" / "UseDurationDump.java"
OUT_JSON = ROOT / "crates" / "dsl" / "data" / "item-use-duration-1.21.11.json"
MANIFEST = "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json"

CLASS_SHARED = "net.minecraft.SharedConstants"
CLASS_BOOTSTRAP = "net.minecraft.server.Bootstrap"
CLASS_BUILTIN = "net.minecraft.core.registries.BuiltInRegistries"
CLASS_REGISTRY = "net.minecraft.core.Registry"
CLASS_ITEM = "net.minecraft.world.item.Item"
CLASS_STACK = "net.minecraft.world.item.ItemStack"
CLASS_LIVING = "net.minecraft.world.entity.LivingEntity"
CONSUMABLE = "minecraft:consumable"
INSTRUMENT = "minecraft:instrument"
DEFAULT_CONSUME_SECONDS = 1.6


def fail(msg: str) -> None:
    sys.exit(f"extract-item-use-duration: {msg}")


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


def report_items(jar: pathlib.Path, work: pathlib.Path) -> dict[str, dict]:
    """Method 2: the data generator's `reports/items.json` over the same jar."""
    gen = work / "generated"
    report = gen / "reports" / "items.json"
    if not report.exists():
        subprocess.run(
            [jdk_tool("java"), "-DbundlerMainClass=net.minecraft.data.Main", "-jar", str(jar),
             "--reports", "--output", str(gen)],
            check=True, capture_output=True, text=True, cwd=work,
        )
    if not report.exists():
        fail("the data generator wrote no reports/items.json")
    return json.loads(report.read_text(encoding="utf8"))


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--check", action="store_true", help="non-zero if the table would change")
    ap.add_argument("--work", help="cache directory for the jar and mappings")
    args = ap.parse_args()

    work = (
        pathlib.Path(args.work).resolve()
        if args.work
        else pathlib.Path(tempfile.mkdtemp(prefix="dwusedur-"))
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
    use_sig = f"getUseDuration({CLASS_STACK},{CLASS_LIVING})"
    dump_args = []
    dump_args += resolve(maps, CLASS_SHARED, methods=["tryDetectVersion()"])
    dump_args += resolve(maps, CLASS_BOOTSTRAP, methods=["bootStrap()"])
    dump_args += resolve(maps, CLASS_BUILTIN, fields=["ITEM"])
    dump_args += resolve(maps, CLASS_REGISTRY, methods=["getKey"])
    dump_args += resolve(maps, CLASS_ITEM, methods=["getDefaultInstance()", use_sig])
    dump_args += resolve(maps, CLASS_STACK)
    dump_args += resolve(maps, CLASS_LIVING)

    classes = work / "usedur-classes"
    classes.mkdir(exist_ok=True)
    subprocess.run(
        [jdk_tool("javac"), "-nowarn", "-cp", classpath, "-d", str(classes), str(JAVA_SRC)],
        check=True,
    )
    table = work / "usedur.tsv"
    proc = subprocess.run(
        [jdk_tool("java"), "-Xmx3g", "-cp", f"{classpath}:{classes}", "dw.UseDurationDump",
         str(table), *dump_args],
        check=True, capture_output=True, text=True, cwd=work,
    )
    counted = re.search(r"DUMPED items=(\d+)", proc.stdout)
    if not counted or int(counted.group(1)) == 0:
        fail(f"the dumper reported nothing to count: {proc.stdout.strip()!r}")
    rows = [ln.split("\t") for ln in table.read_text(encoding="utf8").splitlines()]
    if len(rows) != int(counted.group(1)):
        fail(f"dumper counted {counted.group(1)} items but wrote {len(rows)} rows")
    report = report_items(jar, work)
    runtime: dict[str, int] = {}
    through_holder: list[str] = []
    for ident, ticks in rows:
        if not ticks.startswith("holder:"):
            runtime[ident] = int(ticks)
            continue
        # The item reads its duration through the holder's registries: the one
        # such read the pinned game has is the instrument's, resolved from the
        # jar's own data for the item's default instrument.
        comps = report.get(ident, {}).get("components", {})
        if INSTRUMENT not in comps:
            fail(f"{ident}: needs a holder to answer ({ticks}) and carries no {INSTRUMENT}")
        inst = comps[INSTRUMENT].removeprefix("minecraft:")
        with zipfile.ZipFile(inner[0]) as zf:
            data = json.loads(zf.read(f"data/minecraft/instrument/{inst}.json"))
        runtime[ident] = int(data["use_duration"] * 20)
        through_holder.append(f"{ident} ({inst})")

    if set(report) != set(runtime):
        fail(
            f"runtime-only {sorted(set(runtime) - set(report))} / report-only "
            f"{sorted(set(report) - set(runtime))} — the two methods must see one registry"
        )
    consumables = 0
    others: list[str] = []
    for ident in sorted(runtime):
        comps = report[ident].get("components", {})
        if CONSUMABLE in comps:
            consumables += 1
            secs = comps[CONSUMABLE].get("consume_seconds", DEFAULT_CONSUME_SECONDS)
            want = int(secs * 20)
            if runtime[ident] != want:
                fail(f"{ident}: report consumes in {secs}s ({want} t), runtime says {runtime[ident]} t")
        elif runtime[ident] > 0:
            others.append(f"{ident}={runtime[ident]}")
    used = {k: v for k, v in runtime.items() if v > 0}
    sys.stderr.write(
        f"  {len(runtime)} items, {len(used)} with a use duration: {consumables} consumables agree "
        f"with the report; the {len(others)} others: {', '.join(others)}; read through the "
        f"instrument data: {', '.join(through_holder) or 'none'}\n"
    )

    text = json.dumps(used, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    if args.check:
        if OUT_JSON.read_text(encoding="utf8") != text:
            fail(f"{OUT_JSON.relative_to(ROOT)} is stale — regenerate it")
        print(f"extract-item-use-duration: up to date ({len(used)} items)")
        return
    OUT_JSON.write_text(text, encoding="utf8")
    print(
        f"extract-item-use-duration: wrote {len(used)} items with a use duration to "
        f"{OUT_JSON.relative_to(ROOT)} (Minecraft {pin['version']})"
    )


main()
