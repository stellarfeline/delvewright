#!/usr/bin/env python3
"""Regenerate the block-light fixture from the PINNED Minecraft server jar.

The fixture `crates/compiler/tests/fixtures/light/emission-<version>.tsv` is what
`crates/compiler/tests/emission_table.rs` measures `light::emission()` against.
It is a measurement, not a table somebody typed: every value comes from calling
the game's own `BlockState.getLightEmission()` inside the pinned server jar.

    tools/dump-block-light.py            # rewrite the fixture
    tools/dump-block-light.py --check    # regenerate into a temp dir and diff

The jar is identified by `versions.toml` and refused unless its sha256 matches
the pin, so the fixture cannot silently be taken against a different game. The
mappings are fetched from piston-meta, whose URLs are sha1-content-addressed and
are verified too. No obfuscated name is written down here or in the Java dumper:
every one is resolved from the mappings for the same pin, so a version bump is a
pin edit and nothing else.

Requires a JDK (>= the pin's `javaVersion.majorVersion`) and network access. It
is a REGENERATION tool: CI reads the committed fixture and never runs this.
"""

from __future__ import annotations

import argparse
import collections
import hashlib
import json
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parent.parent
VERSIONS_TOML = ROOT / "versions.toml"
JAVA_SRC = ROOT / "tools" / "blocklight" / "BlockLightDump.java"
FIXTURE_DIR = ROOT / "crates" / "compiler" / "tests" / "fixtures" / "light"
MANIFEST = "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json"

# The DEOBFUSCATED names the dumper needs, in the order BlockLightDump.java
# takes them. Each is looked up in the mappings for the pinned version.
CLASS_SHARED = "net.minecraft.SharedConstants"
CLASS_BOOTSTRAP = "net.minecraft.server.Bootstrap"
CLASS_BLOCK = "net.minecraft.world.level.block.Block"
CLASS_STATE_BASE = "net.minecraft.world.level.block.state.BlockBehaviour$BlockStateBase"
CLASS_BUILTIN = "net.minecraft.core.registries.BuiltInRegistries"
CLASS_REGISTRY = "net.minecraft.core.Registry"

STATE_RE = re.compile(r"^Block\{minecraft:([a-z0-9_]+)\}(?:\[(.*)\])?$")


def fail(msg: str) -> None:
    sys.exit(f"dump-block-light: {msg}")


# --------------------------------------------------------------------------
# The pin
# --------------------------------------------------------------------------


def read_pin() -> dict[str, str]:
    """The `[minecraft]` block of versions.toml — the single source of the pin."""
    text = VERSIONS_TOML.read_text(encoding="utf8")
    section = text.split("[minecraft]", 1)
    if len(section) != 2:
        fail(f"{VERSIONS_TOML} has no [minecraft] section")
    body = section[1].split("\n[", 1)[0]
    out = {}
    for key in ("version", "server_jar_url", "server_jar_sha256"):
        m = re.search(rf'^{key}\s*=\s*"([^"]+)"', body, re.M)
        if not m:
            fail(f"versions.toml [minecraft] has no {key}")
        out[key] = m.group(1)
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


# --------------------------------------------------------------------------
# The mappings
# --------------------------------------------------------------------------


def parse_mappings(path: pathlib.Path) -> dict[str, tuple[str, dict[str, str], dict[str, str]]]:
    """ProGuard mappings -> {deobf class: (obf class, fields, methods)}.

    Fields and methods are kept APART because a name can be both, and taking
    whichever came first answers the wrong question with an honest-looking
    string: `Block.defaultBlockState` is a field (`d`) and a method (`m`), and
    reflecting the field name as a method fails far from the mistake.

    The `# {"fileName":...}` lines are UNindented, so they must be skipped
    explicitly: treating one as a class header silently empties the class that
    follows it, and every later lookup then fails for the wrong reason.
    """
    out: dict[str, tuple[str, dict[str, str], dict[str, str]]] = {}
    fields: dict[str, str] | None = None
    methods: dict[str, str] | None = None
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
            # Keyed by NAME: every member this tool asks for is unique within
            # its class once fields and methods are separated.
            (methods if m.group(3) is not None else fields).setdefault(m.group(2), m.group(4))
    return out


def resolve(maps, cls: str, fields: list[str] = (), methods: list[str] = ()) -> list[str]:
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


# --------------------------------------------------------------------------
# The collapse
# --------------------------------------------------------------------------


def collapse(states_tsv: pathlib.Path) -> tuple[list[tuple[str, int, int]], int]:
    """Collapse every blockstate onto its light-RELEVANT properties.

    Lossless with respect to light: within a group the emission is asserted
    constant, so each of the game's states is represented by exactly one row and
    the row carries a real, full blockstate string a prefab palette could hold.
    """
    per: dict[str, list[tuple[dict[str, str], int]]] = collections.defaultdict(list)
    for ln in states_tsv.read_text(encoding="utf8").splitlines():
        s, l = ln.rsplit("\t", 1)
        m = STATE_RE.match(s)
        if not m:
            fail(f"unparsable blockstate from the dumper: {s!r}")
        props = {}
        if m.group(2):
            for kv in m.group(2).split(","):
                k, v = kv.split("=", 1)
                props[k] = v
        per[m.group(1)].append((props, int(l)))

    rows: list[tuple[str, int, int]] = []
    covered = 0
    for block in sorted(per):
        states = per[block]
        keys = sorted(states[0][0])
        rel = []
        for k in keys:
            groups: dict[tuple, set[int]] = collections.defaultdict(set)
            for props, l in states:
                other = tuple(sorted((kk, vv) for kk, vv in props.items() if kk != k))
                groups[other].add(l)
            if any(len(v) > 1 for v in groups.values()):
                rel.append(k)
        buckets: dict[tuple, list[tuple[dict[str, str], int]]] = collections.defaultdict(list)
        for props, l in states:
            buckets[tuple((k, props[k]) for k in rel)].append((props, l))
        for key in sorted(buckets):
            members = buckets[key]
            lights = {l for _, l in members}
            if len(lights) != 1:
                fail(f"{block}{key}: light is not constant within the group: {lights}")
            reps = sorted(
                ("[" + ",".join(f"{k}={v}" for k, v in sorted(p.items())) + "]") if p else ""
                for p, _ in members
            )
            rows.append((f"minecraft:{block}{reps[0]}", lights.pop(), len(members)))
            covered += len(members)
    return rows, covered


# --------------------------------------------------------------------------


def generate(work: pathlib.Path) -> tuple[str, str]:
    pin = read_pin()
    version = pin["version"]
    work.mkdir(parents=True, exist_ok=True)

    jar = work / "server.jar"
    fetch(pin["server_jar_url"], jar, pin["server_jar_sha256"])

    manifest = work / "manifest.json"
    fetch(MANIFEST, manifest, None)
    entries = [
        v for v in json.loads(manifest.read_text(encoding="utf8"))["versions"] if v["id"] == version
    ]
    if not entries:
        fail(f"piston manifest has no version {version}")
    vjson = work / "version.json"
    fetch(entries[0]["url"], vjson, entries[0]["sha1"], "sha1")
    downloads = json.loads(vjson.read_text(encoding="utf8"))["downloads"]
    if downloads["server"]["url"] != pin["server_jar_url"]:
        fail("piston's server jar url disagrees with versions.toml — refusing")
    mappings = work / "server.txt"
    fetch(downloads["server_mappings"]["url"], mappings, downloads["server_mappings"]["sha1"], "sha1")

    bundle = work / "bundle"
    subprocess.run(
        ["unzip", "-o", "-q", str(jar), "-d", str(bundle), "META-INF/versions/*", "META-INF/libraries/*"],
        check=True,
    )
    inner = list((bundle / "META-INF" / "versions").rglob("*.jar"))
    if len(inner) != 1:
        fail(f"expected one bundled server jar, found {len(inner)}")
    cp = [str(inner[0])] + [str(p) for p in (bundle / "META-INF" / "libraries").rglob("*.jar")]
    classpath = ":".join(cp)

    maps = parse_mappings(mappings)
    args = []
    args += resolve(maps, CLASS_SHARED, methods=["tryDetectVersion"])
    args += resolve(maps, CLASS_BOOTSTRAP, methods=["bootStrap"])
    args += resolve(maps, CLASS_BLOCK, fields=["BLOCK_STATE_REGISTRY"], methods=["defaultBlockState"])
    args += resolve(maps, CLASS_STATE_BASE, methods=["getLightEmission"])
    args += resolve(maps, CLASS_BUILTIN, fields=["BLOCK"])
    args += resolve(maps, CLASS_REGISTRY, methods=["getKey"])
    sys.stderr.write(f"  resolved from the pinned mappings: {' '.join(args)}\n")

    classes = work / "classes"
    classes.mkdir(exist_ok=True)
    # A build failure is a TOOL failure: never fall through to whatever is
    # already sitting in `classes/`.
    subprocess.run(
        ["javac", "-nowarn", "-cp", classpath, "-d", str(classes), str(JAVA_SRC)], check=True
    )

    states_tsv = work / "states.tsv"
    defaults_tsv = work / "defaults.tsv"
    # `cwd=work`, because booting the vanilla registries starts log4j and it
    # writes a `logs/` directory into the CURRENT directory — run from the repo
    # root, this tool would otherwise litter the checkout every time.
    proc = subprocess.run(
        ["java", "-Xmx3g", "-cp", f"{classpath}:{classes}", "dw.BlockLightDump",
         str(states_tsv), str(defaults_tsv), *args],
        check=True,
        capture_output=True,
        text=True,
        cwd=work,
    )
    counted = re.search(r"DUMPED states=(\d+) blocks=(\d+)", proc.stdout)
    if not counted or int(counted.group(1)) == 0 or int(counted.group(2)) == 0:
        fail(f"the dumper reported nothing to count: {proc.stdout.strip()!r}")
    sys.stderr.write(f"  dumper: {counted.group(0)}\n")

    rows, covered = collapse(states_tsv)
    if covered != int(counted.group(1)):
        fail(f"collapse covered {covered} states, the dumper produced {counted.group(1)}")

    jar_sha = hashlib.sha256(jar.read_bytes()).hexdigest()
    map_sha = hashlib.sha1(mappings.read_bytes()).hexdigest()
    head = [
        f"# Block-light emission of every blockstate of Minecraft Java {version}.",
        "#",
        "# MEASURED, not written: each value is what the game's own",
        "# BlockState.getLightEmission() returns inside the pinned server jar.",
        f"#   server jar sha256 {jar_sha}",
        f"#   server mappings sha1 {map_sha}",
        "# Regenerate with tools/dump-block-light.py; --check diffs against this file.",
        "#",
        f"# {len(rows)} rows collapse the game's {covered} blockstates onto the",
        "# properties that can change the light, so every state is represented by",
        "# exactly one row and each row is a full blockstate a palette could hold.",
        "#",
        "# blockstate<TAB>light<TAB>states-this-row-stands-for",
    ]
    body = "\n".join(f"{n}\t{l}\t{c}" for n, l, c in rows)
    return version, "\n".join(head) + "\n" + body + "\n"


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--check", action="store_true", help="diff against the committed fixture")
    ap.add_argument("--work", help="cache directory for the jar and mappings")
    args = ap.parse_args()

    # Absolute, because the dumper runs with `cwd=work`.
    work = (
        pathlib.Path(args.work).resolve()
        if args.work
        else pathlib.Path(tempfile.mkdtemp(prefix="dwlight-"))
    )
    version, text = generate(work)
    target = FIXTURE_DIR / f"emission-{version}.tsv"

    if args.check:
        if not target.exists():
            fail(f"{target} does not exist")
        if target.read_text(encoding="utf8") == text:
            print(f"dump-block-light: {target.relative_to(ROOT)} matches the pinned jar")
            return
        sys.exit(f"dump-block-light: {target.relative_to(ROOT)} DISAGREES with the pinned jar")

    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(text, encoding="utf8")
    print(f"dump-block-light: wrote {target.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
