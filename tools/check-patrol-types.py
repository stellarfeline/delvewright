#!/usr/bin/env python3
"""Check `#minecraft:raiders` against the PINNED Minecraft server jar.

`DW0382` admits a lane species iff it is in vanilla's `#minecraft:raiders` tag,
read from the vendored tag table `crates/dsl/data/entity-tags-1.21.11.json`. That
choice of tag is a claim about the game — that the tag's members are exactly the
entity types whose `Patrolling` / `PatrolLeader` / `patrol_target` NBT the game
honours — and this is the instrument that can falsify it.

It asks the game three ways and requires all three to agree:

1. every entity type whose class is a `PatrollingMonster`;
2. every entity type whose class is a `Raider`;
3. the vendored `#minecraft:raiders` tag.

and it asserts that of every class in the whole server jar that carries those
three keys as string constants, exactly one is a class some entity type is
actually built from — `PatrollingMonster` — which is what makes (1) the complete
answer rather than a plausible one. The test is structural, never an allowlist of
names: a carrier that stands in no entity's superclass chain cannot be a body
that patrols, whatever it is called. (In 1.21.11 there is one such carrier, the
datafixer that renamed `PatrolTarget` to `patrol_target`.)

    tools/check-patrol-types.py                # check; non-zero on disagreement
    tools/check-patrol-types.py --print        # also print the full type table

The jar is identified by `versions.toml` and refused unless its sha256 matches
the pin, so the answer cannot silently be taken against a different game. The
mappings are fetched from piston-meta, whose URLs are sha1-content-addressed and
are verified too. No obfuscated name is written down here or in the Java dumper.

Requires a JDK (>= the pin's `javaVersion.majorVersion`) and network access. Like
`tools/dump-block-light.py` this is a maintenance tool: CI reads the committed tag
table and never runs this. Run it when the MC pin moves.
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

ROOT = pathlib.Path(__file__).resolve().parent.parent
VERSIONS_TOML = ROOT / "versions.toml"
JAVA_SRC = ROOT / "tools" / "patroltypes" / "PatrolTypeDump.java"
TAGS_JSON = ROOT / "crates" / "dsl" / "data" / "entity-tags-1.21.11.json"
MANIFEST = "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json"

# The tag `DW0382` reads, and the class that decides the same question in code.
RAIDER_TAG = "minecraft:raiders"
CLASS_PATROLLING = "net.minecraft.world.entity.monster.PatrollingMonster"
CLASS_RAIDER = "net.minecraft.world.entity.raid.Raider"

# The DEOBFUSCATED names the dumper needs, in the order PatrolTypeDump.java takes
# them. Each is looked up in the mappings for the pinned version.
CLASS_SHARED = "net.minecraft.SharedConstants"
CLASS_BOOTSTRAP = "net.minecraft.server.Bootstrap"
CLASS_BUILTIN = "net.minecraft.core.registries.BuiltInRegistries"
CLASS_REGISTRY = "net.minecraft.core.Registry"
CLASS_ENTITY_TYPE = "net.minecraft.world.entity.EntityType"

# The NBT keys the patrol contract is written in. Their being constants of one
# single class is the measurement that closes the question.
PATROL_NBT_KEYS = ("Patrolling", "PatrolLeader", "patrol_target")


def fail(msg: str) -> None:
    sys.exit(f"check-patrol-types: {msg}")


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
    string. The `# {"fileName":...}` lines are UNindented, so they must be
    skipped explicitly: treating one as a class header silently empties the
    class that follows it.
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
# The measurements
# --------------------------------------------------------------------------


def classes_carrying_patrol_keys(inner_jar: pathlib.Path, work: pathlib.Path) -> set[str]:
    """Which class files in the server jar hold the patrol NBT keys as constants.

    Answered over the jar's whole class set — never over the classes this tool
    happened to name — because the point of the question is what it might find
    that nobody expected.
    """
    exploded = work / "inner"
    if not exploded.exists():
        subprocess.run(["unzip", "-o", "-q", str(inner_jar), "-d", str(exploded)], check=True)
    carriers: dict[str, set[str]] = {k: set() for k in PATROL_NBT_KEYS}
    class_files = list(exploded.rglob("*.class"))
    if not class_files:
        fail("the server jar exploded to zero class files")
    for key in PATROL_NBT_KEYS:
        needle = key.encode("utf8")
        for cf in class_files:
            if needle in cf.read_bytes():
                carriers[key].add(cf.relative_to(exploded).as_posix()[: -len(".class")])
    sys.stderr.write(f"  scanned {len(class_files)} class files for the patrol NBT keys\n")
    union: set[str] = set()
    for key in PATROL_NBT_KEYS:
        if not carriers[key]:
            fail(f"no class in the server jar carries the NBT key {key!r}")
        union |= carriers[key]
    return union


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--print", action="store_true", dest="show", help="print the full type table")
    ap.add_argument("--work", help="cache directory for the jar and mappings")
    args = ap.parse_args()

    work = (
        pathlib.Path(args.work).resolve()
        if args.work
        else pathlib.Path(tempfile.mkdtemp(prefix="dwpatrol-"))
    )
    work.mkdir(parents=True, exist_ok=True)

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
    cp = [str(inner[0])] + [str(p) for p in (bundle / "META-INF" / "libraries").rglob("*.jar")]
    classpath = ":".join(cp)

    maps = parse_mappings(mappings)
    dump_args = []
    dump_args += resolve(maps, CLASS_SHARED, methods=["tryDetectVersion"])
    dump_args += resolve(maps, CLASS_BOOTSTRAP, methods=["bootStrap"])
    dump_args += resolve(maps, CLASS_BUILTIN, fields=["ENTITY_TYPE"])
    dump_args += resolve(maps, CLASS_REGISTRY, methods=["getKey"])
    dump_args += resolve(maps, CLASS_ENTITY_TYPE)
    obf_patrolling = resolve(maps, CLASS_PATROLLING)[0]
    obf_raider = resolve(maps, CLASS_RAIDER)[0]
    dump_args += [obf_patrolling, obf_raider]
    sys.stderr.write(f"  resolved from the pinned mappings: {' '.join(dump_args)}\n")

    classes = work / "classes"
    classes.mkdir(exist_ok=True)
    # A build failure is a TOOL failure: never fall through to whatever is
    # already sitting in `classes/`.
    subprocess.run(
        ["javac", "-nowarn", "-cp", classpath, "-d", str(classes), str(JAVA_SRC)], check=True
    )

    table = work / "patrol-types.tsv"
    # `cwd=work`, because booting the vanilla registries starts log4j and it
    # writes a `logs/` directory into the CURRENT directory.
    proc = subprocess.run(
        ["java", "-Xmx3g", "-cp", f"{classpath}:{classes}", "dw.PatrolTypeDump",
         str(table), *dump_args],
        check=True, capture_output=True, text=True, cwd=work,
    )
    counted = re.search(r"DUMPED types=(\d+)", proc.stdout)
    if not counted or int(counted.group(1)) == 0:
        fail(f"the dumper reported nothing to count: {proc.stdout.strip()!r}")
    sys.stderr.write(f"  dumper: {counted.group(0)}\n")

    rows = [ln.split("\t") for ln in table.read_text(encoding="utf8").splitlines()]
    by_class = {r[0] for r in rows if r[2] == "patrolling"}
    by_raider = {r[0] for r in rows if r[3] == "raider"}
    # Every class some entity type is actually built from, its supertypes
    # included — the population a carrier of the patrol keys has to be in
    # before it can be a body that patrols.
    entity_classes = {c for r in rows for c in r[4].split(" ")}

    tags = json.loads(TAGS_JSON.read_text(encoding="utf8"))
    by_tag = set(tags.get(RAIDER_TAG, []))

    if args.show:
        for r in rows:
            print("\t".join(r))

    # The measurement that makes the class test the COMPLETE answer.
    carriers = classes_carrying_patrol_keys(inner[0], work)
    embodied = carriers & entity_classes
    if embodied != {obf_patrolling}:
        fail(
            f"of the {len(carriers)} class(es) carrying the patrol NBT keys, the "
            f"ones some entity is built from are {sorted(embodied)} — expected "
            f"exactly {obf_patrolling} ({CLASS_PATROLLING}). Another body reads "
            "the patrol contract, so `DW0382`'s rule needs re-deriving."
        )
    sys.stderr.write(
        f"  patrol NBT keys carried by {len(carriers)} class(es), of which "
        f"{len(embodied)} stand(s) in an entity's superclass chain\n"
    )

    problems = []
    if not by_tag:
        problems.append(f"the vendored tag table has no `#{RAIDER_TAG}`")
    if by_class != by_raider:
        problems.append(
            f"PatrollingMonster {sorted(by_class)} != Raider {sorted(by_raider)}"
        )
    if by_class != by_tag:
        problems.append(
            f"PatrollingMonster {sorted(by_class)} != #{RAIDER_TAG} {sorted(by_tag)}"
        )
    if problems:
        for p in problems:
            sys.stderr.write(f"  DISAGREEMENT: {p}\n")
        sys.exit(
            "check-patrol-types: the pinned game and the vendored tag disagree about "
            "which bodies patrol — `DW0382` reads the tag, so the tag is now wrong "
            "for the question. Re-derive the rule before regenerating anything."
        )

    print(
        f"check-patrol-types: {len(by_tag)} of {len(rows)} entity types in Minecraft "
        f"{pin['version']} honour patrol NBT, and `#{RAIDER_TAG}`, "
        f"{CLASS_PATROLLING.rsplit('.', 1)[1]} and {CLASS_RAIDER.rsplit('.', 1)[1]} "
        f"name the same {len(by_tag)}: {', '.join(sorted(by_tag))}. "
        f"Of every class carrying {', '.join(PATROL_NBT_KEYS)}, exactly one is a "
        "class an entity is built from."
    )


main()
