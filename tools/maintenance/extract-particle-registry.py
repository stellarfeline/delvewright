#!/usr/bin/env python3
"""Regenerate, or check, `crates/dsl/data/particles-1.21.11.json` from the PINNED
Minecraft server jar.

The file is what the `particle` verb's id is validated against (spec-0085,
`DW0941`): every particle type the game registers, and for each whether it
**takes options**. An options-taking type (`dust`, `block`, `item`, …) cannot be
written as a bare id, so the verb refuses it; a simple type can. Both facts are
measurements taken inside the game, never a list somebody typed:

1. the particle registry is iterated after booting the game's own registries,
   and each type is asked twice whether it takes options — is it a
   `SimpleParticleType` (the concrete class), and is the type object itself a
   `ParticleOptions` (the interface the `particle` command sends). The two must
   agree on every type;
2. the id set is cross-checked against a second method that shares no code with
   the first: the vanilla data generator's own `registries.json` report, run
   from the same jar.

    tools/maintenance/extract-particle-registry.py            # rewrite the file
    tools/maintenance/extract-particle-registry.py --check    # derive and compare; non-zero on difference

The jar is identified by `versions.toml` and refused unless its sha256 matches
the pin. The mappings are fetched from piston-meta, whose URLs are
sha1-content-addressed and verified too. No obfuscated name is written down here
or in the Java dumper (`tools/maintenance/particles/ParticleTypeDump.java`).

Requires a JDK (>= the pin's `javaVersion.majorVersion`) and network access for
the piston manifest. A maintenance tool: CI reads the committed file and never
runs this. Run it when the MC pin moves.
"""

from __future__ import annotations

import argparse
import hashlib
import json
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
JAVA_SRC = ROOT / "tools" / "maintenance" / "particles" / "ParticleTypeDump.java"
OUT_JSON = ROOT / "crates" / "dsl" / "data" / "particles-1.21.11.json"
MANIFEST = "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json"

CLASS_SHARED = "net.minecraft.SharedConstants"
CLASS_BOOTSTRAP = "net.minecraft.server.Bootstrap"
CLASS_BUILTIN = "net.minecraft.core.registries.BuiltInRegistries"
CLASS_REGISTRY = "net.minecraft.core.Registry"
CLASS_SIMPLE = "net.minecraft.core.particles.SimpleParticleType"
CLASS_OPTIONS = "net.minecraft.core.particles.ParticleOptions"

REGISTRY_KEY = "minecraft:particle_type"


def fail(msg: str) -> None:
    sys.exit(f"extract-particle-registry: {msg}")


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


def parse_mappings(path: pathlib.Path) -> dict[str, tuple[str, dict[str, str], dict[str, str]]]:
    """ProGuard mappings -> {deobf class: (obf class, fields, methods)}; fields and
    methods kept apart, the unindented `#` lines skipped (see
    `check-patrol-types.py`, whose reader this is)."""
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


def render(table: dict[str, bool]) -> str:
    doc = {pid: {"options": takes} for pid, takes in table.items()}
    return json.dumps(doc, indent=2, sort_keys=True, ensure_ascii=False) + "\n"


def derive(work: pathlib.Path) -> dict[str, bool]:
    pin = read_pin()
    jar = work / "server.jar"
    fetch(pin["server_jar_url"], jar, pin["server_jar_sha256"])

    manifest = work / "manifest.json"
    fetch(MANIFEST, manifest, None)
    entries = [
        v
        for v in json.loads(manifest.read_text(encoding="utf8"))["versions"]
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
    fetch(
        downloads["server_mappings"]["url"],
        mappings,
        downloads["server_mappings"]["sha1"],
        "sha1",
    )

    bundle = work / "bundle"
    if not bundle.exists():
        subprocess.run(
            [
                "unzip", "-o", "-q", str(jar), "-d", str(bundle),
                "META-INF/versions/*", "META-INF/libraries/*",
            ],
            check=True,
        )
    inner = list((bundle / "META-INF" / "versions").rglob("*.jar"))
    if len(inner) != 1:
        fail(f"expected one bundled server jar, found {len(inner)}")
    cp = [str(inner[0])] + sorted(
        str(p) for p in (bundle / "META-INF" / "libraries").rglob("*.jar")
    )
    classpath = ":".join(cp)

    maps = parse_mappings(mappings)
    dump_args: list[str] = []
    dump_args += resolve(maps, CLASS_SHARED, methods=["tryDetectVersion"])
    dump_args += resolve(maps, CLASS_BOOTSTRAP, methods=["bootStrap"])
    dump_args += resolve(maps, CLASS_BUILTIN, fields=["PARTICLE_TYPE"])
    dump_args += resolve(maps, CLASS_REGISTRY, methods=["getKey"])
    dump_args += resolve(maps, CLASS_SIMPLE)
    dump_args += resolve(maps, CLASS_OPTIONS)
    sys.stderr.write(f"  resolved from the pinned mappings: {' '.join(dump_args)}\n")

    classes = work / "classes"
    classes.mkdir(exist_ok=True)
    # A build failure is a TOOL failure: never fall through to stale classes.
    subprocess.run(
        ["javac", "-nowarn", "-cp", classpath, "-d", str(classes), str(JAVA_SRC)], check=True
    )
    tsv = work / "particle-types.tsv"
    # `cwd=work`: booting the registries starts log4j, which writes `logs/` into
    # the current directory.
    proc = subprocess.run(
        ["java", "-Xmx3g", "-cp", f"{classpath}:{classes}", "dw.ParticleTypeDump",
         str(tsv), *dump_args],
        check=True, capture_output=True, text=True, cwd=work,
    )
    counted = re.search(r"DUMPED types=(\d+)", proc.stdout)
    if not counted or int(counted.group(1)) == 0:
        fail(f"the dumper reported nothing to count: {proc.stdout.strip()!r}")
    sys.stderr.write(f"  dumper: {counted.group(0)}\n")

    table: dict[str, bool] = {}
    disagree: list[str] = []
    for line in tsv.read_text(encoding="utf8").splitlines():
        pid, cls, iface = line.split("\t")
        simple = cls == "simple"
        if simple != (iface == "is-options"):
            disagree.append(f"{pid}: class says {cls}, interface says {iface}")
        table[pid] = not simple
    if disagree:
        fail("the two readings of 'takes options' disagree: " + "; ".join(disagree))
    if len(table) != int(counted.group(1)):
        fail(f"dumper counted {counted.group(1)} types and wrote {len(table)} rows")

    # The second method: the vanilla data generator's own registry report.
    reports = work / "generated"
    if not (reports / "reports" / "registries.json").exists():
        subprocess.run(
            ["java", "-DbundlerMainClass=net.minecraft.data.Main", "-jar", str(jar),
             "--reports", "--output", str(reports)],
            check=True, capture_output=True, text=True, cwd=work,
        )
    report = json.loads((reports / "reports" / "registries.json").read_text(encoding="utf8"))
    if REGISTRY_KEY not in report:
        fail(f"the data generator's registries report has no `{REGISTRY_KEY}`")
    generated = set(report[REGISTRY_KEY]["entries"])
    if generated != set(table):
        fail(
            "the dumper and the data generator name different particle types: "
            f"only dumper {sorted(set(table) - generated)}, "
            f"only generator {sorted(generated - set(table))}"
        )
    sys.stderr.write(
        f"  data generator report names the same {len(generated)} particle types\n"
    )
    return table


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--check", action="store_true", help="derive and compare; never write")
    ap.add_argument("--work", help="cache directory for the jar, mappings and reports")
    args = ap.parse_args()
    work = (
        pathlib.Path(args.work).resolve()
        if args.work
        else pathlib.Path(tempfile.mkdtemp(prefix="dwparticles-"))
    )
    work.mkdir(parents=True, exist_ok=True)

    table = derive(work)
    text = render(table)
    options = sum(1 for v in table.values() if v)
    summary = (
        f"{len(table)} particle types, {options} take options, "
        f"{len(table) - options} are simple"
    )
    if args.check:
        committed = OUT_JSON.read_text(encoding="utf8") if OUT_JSON.exists() else ""
        if committed != text:
            fail(
                f"{OUT_JSON.relative_to(ROOT)} differs from what the pinned jar says "
                f"({summary}); re-run without --check and read the diff"
            )
        print(f"extract-particle-registry: committed file matches the pinned jar — {summary}")
        return
    OUT_JSON.write_text(text, encoding="utf8")
    print(f"extract-particle-registry: wrote {OUT_JSON.relative_to(ROOT)} — {summary}")


main()
