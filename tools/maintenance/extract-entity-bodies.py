#!/usr/bin/env python3
"""Regenerate `crates/delvec/data/entity-bodies-1.21.11.json` from the PINNED
Minecraft server jar: every entity type's bounding box and the distance the
server tracks it at.

A cutscene shot proves that the body it frames is sent to the player watching
it and drawn by that player's client (spec-0110 §5.7, §5.8). Both are facts
about the entity TYPE — how far the server sends it (`clientTrackingRange`, in
chunks) and how big its box is (the client culls a body beyond
`size × 64 × viewScale` blocks, `size` the mean of the box's three extents) —
so they are read from the game, never written down by hand.

## Two methods

1. **Runtime**: the dumper boots the registries and asks every type for
   `EntityType#getWidth()`, `#getHeight()` and `#clientTrackingRange()`.
2. **Static**: `EntityType.<clinit>` is disassembled and, per registration,
   the arguments of the builder chain's `sized(FF)` and
   `clientTrackingRange(I)` calls are read.

Every registration whose chain states a value must agree with the runtime
answer, and every registration whose chain states none must take one single
runtime value (the builder's default), or the tool refuses.

    tools/maintenance/extract-entity-bodies.py            # regenerate the table
    tools/maintenance/extract-entity-bodies.py --check    # non-zero if it would change

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
JAVA_SRC = ROOT / "tools" / "maintenance" / "bodies" / "BodiesDump.java"
OUT_JSON = ROOT / "crates" / "delvec" / "data" / "entity-bodies-1.21.11.json"
MANIFEST = "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json"

CLASS_SHARED = "net.minecraft.SharedConstants"
CLASS_BOOTSTRAP = "net.minecraft.server.Bootstrap"
CLASS_BUILTIN = "net.minecraft.core.registries.BuiltInRegistries"
CLASS_REGISTRY = "net.minecraft.core.Registry"
CLASS_ENTITY_TYPE = "net.minecraft.world.entity.EntityType"
CLASS_BUILDER = "net.minecraft.world.entity.EntityType$Builder"


def fail(msg: str) -> None:
    sys.exit(f"extract-entity-bodies: {msg}")


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


def static_scan(maps, classpath: str) -> dict[str, dict[str, object]]:
    """Method 2: per registration in `EntityType.<clinit>`, the stated
    `sized(FF)` box and `clientTrackingRange(I)`, where the chain states them."""
    obf_et = resolve(maps, CLASS_ENTITY_TYPE)[0]
    obf_builder = resolve(maps, CLASS_BUILDER)[0]
    m_sized = maps[CLASS_BUILDER][2]["sized(float,float)"]
    m_range = maps[CLASS_BUILDER][2]["clientTrackingRange(int)"]
    m_reg = maps[CLASS_ENTITY_TYPE][2][f"register(java.lang.String,{CLASS_BUILDER})"]
    dump = javap(classpath, obf_et)
    m = re.search(r"^  static \{\};\n    Code:\n(.*?)(?:\n\n|\n\}|\Z)", dump, re.S | re.M)
    if not m:
        fail("javap shows no EntityType static initialiser")
    reg_call = re.compile(
        rf"invokestatic .*Method {re.escape(m_reg)}:\(Ljava/lang/String;L{re.escape(obf_builder)};\)L{re.escape(obf_et)};"
    )
    sized_call = f"Method {obf_builder}.{m_sized}:(FF)L{obf_builder};"
    range_call = f"Method {obf_builder}.{m_range}:(I)L{obf_builder};"
    out: dict[str, dict[str, object]] = {}
    segment: list[str] = []
    for line in m.group(1).splitlines():
        segment.append(line)
        if not reg_call.search(line):
            continue
        ids = [s for ln in segment for s in re.findall(r"// String (\S+)$", ln)]
        if not ids:
            fail("a registration in EntityType.<clinit> carries no string id")
        row: dict[str, object] = {}
        for k, ln in enumerate(segment):
            if sized_call in ln:
                w, h = float_of(segment[k - 2]), float_of(segment[k - 1])
                if w is None or h is None:
                    fail(f"{ids[0]}: sized() is not called on two pushed floats")
                row["box"] = (w, h)
            if range_call in ln:
                r = int_of(segment[k - 1])
                if r is None:
                    fail(f"{ids[0]}: clientTrackingRange() is not called on a pushed int")
                row["range"] = r
        out[f"minecraft:{ids[0]}"] = row
        segment = []
    if not out:
        fail("found zero registrations in EntityType.<clinit>")
    return out


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--check", action="store_true", help="non-zero if the table would change")
    ap.add_argument("--work", help="cache directory for the jar and mappings")
    args = ap.parse_args()

    work = (
        pathlib.Path(args.work).resolve()
        if args.work
        else pathlib.Path(tempfile.mkdtemp(prefix="dwbodies-"))
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
    dump_args += resolve(
        maps, CLASS_ENTITY_TYPE,
        methods=["getWidth()", "getHeight()", "clientTrackingRange()"],
    )

    classes = work / "classes"
    classes.mkdir(exist_ok=True)
    subprocess.run(
        [jdk_tool("javac"), "-nowarn", "-cp", classpath, "-d", str(classes), str(JAVA_SRC)],
        check=True,
    )
    table = work / "bodies.tsv"
    proc = subprocess.run(
        [jdk_tool("java"), "-Xmx3g", "-cp", f"{classpath}:{classes}", "dw.BodiesDump",
         str(table), *dump_args],
        check=True, capture_output=True, text=True, cwd=work,
    )
    counted = re.search(r"DUMPED types=(\d+)", proc.stdout)
    if not counted or int(counted.group(1)) == 0:
        fail(f"the dumper reported nothing to count: {proc.stdout.strip()!r}")
    rows = [ln.split("\t") for ln in table.read_text(encoding="utf8").splitlines()]
    if len(rows) != int(counted.group(1)):
        fail(f"dumper counted {counted.group(1)} types but wrote {len(rows)} rows")
    runtime = {
        ident: {"width": float(w), "height": float(h), "tracking_range": int(r)}
        for ident, w, h, r in rows
    }

    static = static_scan(maps, str(inner[0]))
    if set(static) != set(runtime):
        fail(
            f"runtime-only {sorted(set(runtime) - set(static))} / static-only "
            f"{sorted(set(static) - set(runtime))} — the two methods must see one registry"
        )
    unstated_box: set[tuple[float, float]] = set()
    unstated_range: set[int] = set()
    unstated_ids: list[str] = []
    stated = [0, 0]
    for ident, row in static.items():
        rt = runtime[ident]
        box = (rt["width"], rt["height"])
        if "box" in row:
            stated[0] += 1
            if row["box"] != box:
                fail(f"{ident}: static box {row['box']} / runtime {box} disagree")
        else:
            unstated_box.add(box)
        if "range" in row:
            stated[1] += 1
            if row["range"] != rt["tracking_range"]:
                fail(f"{ident}: static range {row['range']} / runtime {rt['tracking_range']} disagree")
        else:
            unstated_range.add(rt["tracking_range"])
            unstated_ids.append(ident)
    if len(unstated_box) > 1 or len(unstated_range) > 1:
        fail(
            f"types whose chain states no value take {sorted(unstated_box)} / "
            f"{sorted(unstated_range)} — a builder default is ONE value"
        )
    sys.stderr.write(
        f"  runtime and static agree over {len(rows)} types: {stated[0]} state a box, "
        f"{stated[1]} a tracking range; the rest ({', '.join(sorted(unstated_ids))}) take "
        f"{sorted(unstated_box)} / {sorted(unstated_range)}\n"
    )

    text = json.dumps(runtime, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    if args.check:
        if OUT_JSON.read_text(encoding="utf8") != text:
            fail(f"{OUT_JSON.relative_to(ROOT)} is stale — regenerate it")
        print(f"extract-entity-bodies: up to date ({len(rows)} types)")
        return
    OUT_JSON.write_text(text, encoding="utf8")
    print(
        f"extract-entity-bodies: wrote {len(rows)} entity types to "
        f"{OUT_JSON.relative_to(ROOT)} (Minecraft {pin['version']})"
    )


main()
