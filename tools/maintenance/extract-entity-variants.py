#!/usr/bin/env python3
"""Regenerate `crates/dsl/data/entity-variants-1.21.11.json` from the PINNED
Minecraft server jar: for every entity type, the variant properties a body of
that type carries, the values each one takes, and how a `/summon` writes it.

A body's `variant` (the DSL's one shared field on every class that summons a
body) names properties and values; the engine validates them against this
table (`DW1131`, `DW1132`) and writes them into the summon through it. So a
fact about the game is read from the game, never written by hand.

## What a variant property is

- **Every entity data component whose id is `<entity>/<property>`** — the
  game's own enumeration of an entity's variants (`DataComponents`, e.g.
  `minecraft:cat/variant`, `minecraft:cat/collar`). The property is the part
  after the slash. Which entity types carry it is read from the bytecode: a
  type carries a component when a class on its chain names that component in
  its `applyImplicitComponent(DataComponentType, Object)` override.
- **Three saved appearance fields no component carries** (authored, below):
  a horse's `markings` (the high byte of its `Variant`), a villager's or
  zombie villager's `profession` (`VillagerData.profession`), and a panda's
  `gene` (written to `MainGene` and `HiddenGene` alike, so the gene shown is
  the one declared).

## How a summon writes it

`COMPONENTS` and `EXTRAS` state, per property, the class that saves it and the
NBT it is saved under — read off `addAdditionalSaveData` and written down here
with the form the codec uses (`string`: the value's id or serialized name;
`byte` / `int`: the value's numeric id, shifted into a packed key where
several properties share one). This tool proves each statement against the
jar before writing anything:

- every named class's `addAdditionalSaveData` loads the NBT key as a string
  constant (a `variant` key through `VariantUtils.writeVariant`, whose own
  constant is checked; `VillagerData.<field>` through the `VillagerData`
  key and that codec's field name);
- the entity types that read a component (`applyImplicitComponent`) are
  exactly the types whose chain holds the class said to save it;
- every `<entity>/<property>` component the jar registers is in the table,
  so a pin that adds one fails here instead of going unnoticed.

The values come from the game: the data-driven registries from the jar's own
`data/minecraft/<registry>/` files, the built-in villager registries and every
enum (with its numeric id) from the dumper, which boots the registries.

    tools/maintenance/extract-entity-variants.py            # regenerate the table
    tools/maintenance/extract-entity-variants.py --check    # non-zero if it would change

The jar is identified by `versions.toml` and refused unless its sha256 matches
the pin. The mappings come from piston-meta and are sha1-verified. No
obfuscated name is written down here or in the Java dumper.

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
JAVA_SRC = ROOT / "tools" / "maintenance" / "variants" / "VariantDump.java"
OUT_JSON = ROOT / "crates" / "dsl" / "data" / "entity-variants-1.21.11.json"
MANIFEST = "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json"

E = "net.minecraft.world.entity."
CLASS_SHARED = "net.minecraft.SharedConstants"
CLASS_BOOTSTRAP = "net.minecraft.server.Bootstrap"
CLASS_BUILTIN = "net.minecraft.core.registries.BuiltInRegistries"
CLASS_REGISTRY = "net.minecraft.core.Registry"
CLASS_ENTITY_TYPE = E + "EntityType"
CLASS_ENTITY = E + "Entity"
CLASS_COMPONENTS = "net.minecraft.core.component.DataComponents"
CLASS_STRING_REPR = "net.minecraft.util.StringRepresentable"
CLASS_VARIANT_UTILS = E + "variant.VariantUtils"
CLASS_VILLAGER_DATA = E + "npc.villager.VillagerData"
SIG_APPLY = "applyImplicitComponent(net.minecraft.core.component.DataComponentType,java.lang.Object)"
SIG_SAVE = "addAdditionalSaveData(net.minecraft.world.level.storage.ValueOutput)"

# Value domains. `reg:<dir>` — a data-driven registry, one value per
# `data/minecraft/<dir>/*.json`, written as its id. `builtin:<field>` — a
# built-in registry, every key. `enum:<class>:<id method>` — every constant,
# written as its serialized name (or its lower-cased `name()` where the enum has
# none) with the numeric id the named method returns ("-": none).
DOMAINS = {
    "cat_variant": "reg:cat_variant",
    "wolf_variant": "reg:wolf_variant",
    "wolf_sound_variant": "reg:wolf_sound_variant",
    "pig_variant": "reg:pig_variant",
    "cow_variant": "reg:cow_variant",
    "chicken_variant": "reg:chicken_variant",
    "frog_variant": "reg:frog_variant",
    "zombie_nautilus_variant": "reg:zombie_nautilus_variant",
    "painting_variant": "reg:painting_variant",
    "villager_type": "builtin:VILLAGER_TYPE",
    "villager_profession": "builtin:VILLAGER_PROFESSION",
    "dye_color": "enum:net.minecraft.world.item.DyeColor:getId()",
    "fox_variant": f"enum:{E}animal.fox.Fox$Variant:-",
    "salmon_size": f"enum:{E}animal.fish.Salmon$Variant:-",
    "parrot_variant": f"enum:{E}animal.parrot.Parrot$Variant:getId()",
    "tropical_fish_pattern": f"enum:{E}animal.fish.TropicalFish$Pattern:getPackedId()",
    "mooshroom_variant": f"enum:{E}animal.cow.MushroomCow$Variant:-",
    "rabbit_variant": f"enum:{E}animal.rabbit.Rabbit$Variant:id()",
    "horse_variant": f"enum:{E}animal.equine.Variant:getId()",
    "horse_markings": f"enum:{E}animal.equine.Markings:getId()",
    "llama_variant": f"enum:{E}animal.equine.Llama$Variant:getId()",
    "axolotl_variant": f"enum:{E}animal.axolotl.Axolotl$Variant:getId()",
    "panda_gene": f"enum:{E}animal.panda.Panda$Gene:-",
}

# NBT spec: a list of (key, form, shift). `key` may be `Outer.inner` (a field
# of a compound the class saves). Every key of a property is written.
V = [("variant", "string", 0)]

# DataComponents field -> (owner classes, property, domain, nbt).
COMPONENTS = {
    "CAT_VARIANT": ([E + "animal.feline.Cat"], "variant", "cat_variant", V),
    "CAT_COLLAR": ([E + "animal.feline.Cat"], "collar", "dye_color", [("CollarColor", "byte", 0)]),
    "WOLF_VARIANT": ([E + "animal.wolf.Wolf"], "variant", "wolf_variant", V),
    "WOLF_SOUND_VARIANT": ([E + "animal.wolf.Wolf"], "sound_variant", "wolf_sound_variant", [("sound_variant", "string", 0)]),
    "WOLF_COLLAR": ([E + "animal.wolf.Wolf"], "collar", "dye_color", [("CollarColor", "byte", 0)]),
    "FOX_VARIANT": ([E + "animal.fox.Fox"], "variant", "fox_variant", [("Type", "string", 0)]),
    "SALMON_SIZE": ([E + "animal.fish.Salmon"], "size", "salmon_size", [("type", "string", 0)]),
    "PARROT_VARIANT": ([E + "animal.parrot.Parrot"], "variant", "parrot_variant", [("Variant", "int", 0)]),
    "TROPICAL_FISH_PATTERN": ([E + "animal.fish.TropicalFish"], "pattern", "tropical_fish_pattern", [("Variant", "int", 0)]),
    "TROPICAL_FISH_BASE_COLOR": ([E + "animal.fish.TropicalFish"], "base_color", "dye_color", [("Variant", "int", 16)]),
    "TROPICAL_FISH_PATTERN_COLOR": ([E + "animal.fish.TropicalFish"], "pattern_color", "dye_color", [("Variant", "int", 24)]),
    "MOOSHROOM_VARIANT": ([E + "animal.cow.MushroomCow"], "variant", "mooshroom_variant", [("Type", "string", 0)]),
    "RABBIT_VARIANT": ([E + "animal.rabbit.Rabbit"], "variant", "rabbit_variant", [("RabbitType", "int", 0)]),
    "PIG_VARIANT": ([E + "animal.pig.Pig"], "variant", "pig_variant", V),
    "COW_VARIANT": ([E + "animal.cow.Cow"], "variant", "cow_variant", V),
    "CHICKEN_VARIANT": ([E + "animal.chicken.Chicken"], "variant", "chicken_variant", V),
    "ZOMBIE_NAUTILUS_VARIANT": ([E + "animal.nautilus.ZombieNautilus"], "variant", "zombie_nautilus_variant", V),
    "FROG_VARIANT": ([E + "animal.frog.Frog"], "variant", "frog_variant", V),
    "HORSE_VARIANT": ([E + "animal.equine.Horse"], "variant", "horse_variant", [("Variant", "int", 0)]),
    "PAINTING_VARIANT": ([E + "decoration.painting.Painting"], "variant", "painting_variant", V),
    "LLAMA_VARIANT": ([E + "animal.equine.Llama"], "variant", "llama_variant", [("Variant", "int", 0)]),
    "AXOLOTL_VARIANT": ([E + "animal.axolotl.Axolotl"], "variant", "axolotl_variant", [("Variant", "int", 0)]),
    "SHEEP_COLOR": ([E + "animal.sheep.Sheep"], "color", "dye_color", [("Color", "byte", 0)]),
    "SHULKER_COLOR": ([E + "monster.Shulker"], "color", "dye_color", [("Color", "byte", 0)]),
    "VILLAGER_VARIANT": (
        [E + "npc.villager.Villager", E + "monster.zombie.ZombieVillager"],
        "variant", "villager_type", [("VillagerData.type", "string", 0)],
    ),
}

# Saved appearance fields no component carries: (owner classes, property,
# domain, nbt). Authored: the brief's reading of what a body of the type shows.
EXTRAS = [
    ([E + "animal.equine.Horse"], "markings", "horse_markings", [("Variant", "int", 8)]),
    (
        [E + "npc.villager.Villager", E + "monster.zombie.ZombieVillager"],
        "profession", "villager_profession", [("VillagerData.profession", "string", 0)],
    ),
    ([E + "animal.panda.Panda"], "gene", "panda_gene", [("MainGene", "string", 0), ("HiddenGene", "string", 0)]),
]


def fail(msg: str) -> None:
    sys.exit(f"extract-entity-variants: {msg}")


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
    methods maps `name(descriptor-args)` to the obf name. Unindented `#` lines
    are skipped."""
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
        m = re.match(r"^(?:\d+:\d+:)?(\S+) (\S+?)(\([^)]*\))? -> (\S+)$", raw.strip())
        if m:
            if m.group(3) is None:
                fields.setdefault(m.group(2), m.group(4))
            else:
                methods.setdefault(m.group(2) + m.group(3), m.group(4))
    return out


def obf(maps, cls: str) -> str:
    if cls not in maps:
        fail(f"mappings have no class {cls} — the pin moved under this tool")
    return maps[cls][0]


def obf_method(maps, cls: str, sig: str) -> str:
    obf(maps, cls)
    if sig not in maps[cls][2]:
        fail(f"mappings have no method {cls}.{sig}")
    return maps[cls][2][sig]


def obf_field(maps, cls: str, name: str) -> str:
    obf(maps, cls)
    if name not in maps[cls][1]:
        fail(f"mappings have no field {cls}.{name}")
    return maps[cls][1][name]


def strip_generics(t: str) -> str:
    prev = None
    while prev != t:
        prev = t
        t = re.sub(r"<[^<>]*>", "", t)
    return t


class Javap:
    """`javap -c -constants` over many classes at once, split per class."""

    def __init__(self, classpath: str, maps):
        self.classpath = classpath
        self.maps = maps
        self.cache: dict[str, str] = {}

    def load(self, classes: list[str]) -> None:
        want = sorted({c for c in classes if c not in self.cache})
        for i in range(0, len(want), 200):
            chunk = want[i : i + 200]
            proc = subprocess.run(
                [jdk_tool("javap"), "-p", "-c", "-constants", "-classpath", self.classpath,
                 *[obf(self.maps, c) for c in chunk]],
                check=True, capture_output=True, text=True,
            )
            parts = re.split(r"^Compiled from .*$", proc.stdout, flags=re.M)
            parts = [p for p in parts if p.strip()]
            if len(parts) != len(chunk):
                fail(f"javap returned {len(parts)} classes for {len(chunk)}")
            for c, p in zip(chunk, parts):
                header = p.strip().splitlines()[0]
                if f" {obf(self.maps, c)}" not in header.replace(" extends ", " ").split("{")[0] + " ":
                    if not re.search(rf"\b{re.escape(obf(self.maps, c))}\b", header):
                        fail(f"javap output for {c} does not start with its class: {header}")
                self.cache[c] = p

    def body(self, cls: str, sig: str) -> str | None:
        """The bytecode of `cls`'s own declaration of `sig`, or None."""
        if sig not in self.maps[cls][2]:
            return None
        name = self.maps[cls][2][sig]
        want = [x for x in sig[sig.index("(") + 1 : -1].split(",") if x]
        self.load([cls])
        for m in re.finditer(
            r"^  (\S[^\n]*?) ([\w$]+)\(([^)]*)\)[^\n]*\n    Code:\n(.*?)(?:\n\n|\n\}})",
            self.cache[cls], re.S | re.M,
        ):
            if m.group(2) != name:
                continue
            args = [a.strip() for a in strip_generics(m.group(3)).split(",") if a.strip()]
            if len(args) != len(want):
                continue
            if all(
                a == (self.maps[w][0] if w in self.maps else w)
                or (re.fullmatch(r"[A-Z]", a) and w == "java.lang.Object")
                for a, w in zip(args, want)
            ):
                return m.group(4)
        fail(f"javap shows no body for {cls}.{sig}")
        return None


def strings_in(code: str) -> set[str]:
    return set(re.findall(r"// String (\S+)$", code, re.M))


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--check", action="store_true", help="non-zero if the table would change")
    ap.add_argument("--work", help="cache directory for the jar and mappings")
    args = ap.parse_args()

    work = (
        pathlib.Path(args.work).resolve()
        if args.work
        else pathlib.Path(tempfile.mkdtemp(prefix="dwvariants-"))
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
    deobf = {o: c for c, (o, _, _) in maps.items()}
    m_ser = obf_method(maps, CLASS_STRING_REPR, "getSerializedName()")

    dump_args = [
        obf(maps, CLASS_SHARED), obf_method(maps, CLASS_SHARED, "tryDetectVersion()"),
        obf(maps, CLASS_BOOTSTRAP), obf_method(maps, CLASS_BOOTSTRAP, "bootStrap()"),
        obf(maps, CLASS_BUILTIN), obf_field(maps, CLASS_BUILTIN, "ENTITY_TYPE"),
        obf_field(maps, CLASS_BUILTIN, "DATA_COMPONENT_TYPE"),
        obf(maps, CLASS_REGISTRY), obf_method(maps, CLASS_REGISTRY, "getKey(java.lang.Object)"),
        obf(maps, CLASS_ENTITY_TYPE), obf(maps, CLASS_COMPONENTS), m_ser,
    ]
    for label, spec in sorted(DOMAINS.items()):
        kind, _, rest = spec.partition(":")
        if kind == "enum":
            cls, _, mid = rest.rpartition(":")
            dump_args += ["ENUM", label, obf(maps, cls), "-" if mid == "-" else obf_method(maps, cls, mid)]
        elif kind == "builtin":
            dump_args += ["REG", label, obf_field(maps, CLASS_BUILTIN, rest)]

    classes = work / "variant-classes"
    classes.mkdir(exist_ok=True)
    subprocess.run(
        [jdk_tool("javac"), "-nowarn", "-cp", classpath, "-d", str(classes), str(JAVA_SRC)],
        check=True,
    )
    table = work / "variants.tsv"
    proc = subprocess.run(
        [jdk_tool("java"), "-Xmx3g", "-cp", f"{classpath}:{classes}", "dw.VariantDump",
         str(table), *dump_args],
        check=True, capture_output=True, text=True, cwd=work,
    )
    counted = re.search(r"DUMPED types=(\d+) components=(\d+) enums=(\d+) registries=(\d+)", proc.stdout)
    if not counted or int(counted.group(1)) == 0 or int(counted.group(2)) == 0:
        fail(f"the dumper reported nothing to count: {proc.stdout.strip()!r}")

    chains: dict[str, list[str]] = {}
    comp_ids: dict[str, str] = {}
    enum_rows: dict[str, list[tuple[str, str, str]]] = {}
    reg_rows: dict[str, list[str]] = {}
    for ln in table.read_text(encoding="utf8").splitlines():
        row = ln.split("\t")
        if row[0] == "TYPE":
            chains[row[1]] = [deobf.get(c, c) for c in row[2].split(",")]
        elif row[0] == "COMP":
            field = next((n for n, o in maps[CLASS_COMPONENTS][1].items() if o == row[1]), None)
            if field is None:
                fail(f"no DataComponents field maps to {row[1]}")
            comp_ids[field] = row[2]
        elif row[0] == "ENUM":
            enum_rows.setdefault(row[1], []).append((row[2], row[3], row[4]))
        elif row[0] == "REG":
            reg_rows.setdefault(row[1], []).append(row[2])
    if len(chains) != int(counted.group(1)) or len(comp_ids) != int(counted.group(2)):
        fail("the dumper's counts disagree with the rows it wrote")

    # Every `<x>/<y>` component is an entity variant and must be in the table.
    slashed = {f for f, cid in comp_ids.items() if "/" in cid}
    if slashed != set(COMPONENTS):
        fail(
            f"`<entity>/<property>` components and the table disagree: "
            f"jar-only {sorted(slashed - set(COMPONENTS))}, table-only {sorted(set(COMPONENTS) - slashed)}"
        )

    # Domains: each value's written name and numeric id.
    with zipfile.ZipFile(inner[0]) as z:
        names = z.namelist()
    domains: dict[str, list[dict]] = {}
    for label, spec in sorted(DOMAINS.items()):
        kind, _, rest = spec.partition(":")
        if kind == "reg":
            pre = f"data/minecraft/{rest}/"
            vals = sorted(
                "minecraft:" + n[len(pre):-len(".json")]
                for n in names if n.startswith(pre) and n.endswith(".json") and "/" not in n[len(pre):]
            )
            domains[label] = [{"name": v} for v in vals]
        elif kind == "builtin":
            domains[label] = [{"name": v} for v in sorted(reg_rows.get(label, []))]
        else:
            rows = enum_rows.get(label, [])
            out = []
            for name, ser, ident in rows:
                entry = {"name": ser if ser != "-" else name.lower()}
                if ident != "-":
                    entry["id"] = int(ident)
                out.append(entry)
            domains[label] = sorted(out, key=lambda e: (e.get("id", 0), e["name"]))
        if not domains[label]:
            fail(f"domain {label} ({spec}) has no values")
        seen = [e["name"] for e in domains[label]]
        if len(set(seen)) != len(seen):
            fail(f"domain {label} writes two values under one name")

    jp = Javap(str(inner[0]), maps)
    all_classes = sorted({c for ch in chains.values() for c in ch if c in maps})
    jp.load(all_classes)

    # Which components each type reads, from `applyImplicitComponent` on its chain.
    comp_obf = {o: f for f, o in maps[CLASS_COMPONENTS][1].items()}
    reads: dict[str, set[str]] = {}
    obf_comp = obf(maps, CLASS_COMPONENTS)
    for ident, chain in chains.items():
        got: set[str] = set()
        for cls in chain:
            if cls not in maps:
                continue
            code = jp.body(cls, SIG_APPLY) if SIG_APPLY in maps[cls][2] else None
            if code:
                for fo in re.findall(rf"Field {re.escape(obf_comp)}\.(\w+):", code):
                    if fo in comp_obf and comp_obf[fo] in COMPONENTS:
                        got.add(comp_obf[fo])
        reads[ident] = got

    # Each NBT key is a string constant of the owner's save method.
    vu_code = jp.body(CLASS_VARIANT_UTILS, "writeVariant(net.minecraft.world.level.storage.ValueOutput,net.minecraft.core.Holder)")
    vu_consts = strings_in(jp.cache[CLASS_VARIANT_UTILS])
    if "variant" not in vu_consts:
        fail("VariantUtils carries no `variant` constant")
    m_write = obf_method(maps, CLASS_VARIANT_UTILS, "writeVariant(net.minecraft.world.level.storage.ValueOutput,net.minecraft.core.Holder)")
    jp.load([CLASS_VILLAGER_DATA])
    vd_consts = strings_in(jp.cache[CLASS_VILLAGER_DATA])
    _ = vu_code

    def prove_key(owner: str, key: str) -> None:
        code = jp.body(owner, SIG_SAVE)
        if code is None:
            fail(f"{owner} declares no addAdditionalSaveData")
        outer, _, field = key.partition(".")
        if outer == "variant" and not field:
            if f"{obf(maps, CLASS_VARIANT_UTILS)}.{m_write}:" not in code and "variant" not in strings_in(code):
                fail(f"{owner}.addAdditionalSaveData writes no `variant`")
            return
        consts = strings_in(code)
        # A key stored inside the method's own lambda (`ifPresent(k -> store(…))`).
        if "invokedynamic" in code:
            for sig in maps[owner][2]:
                if sig.startswith("lambda$addAdditionalSaveData$"):
                    consts |= strings_in(jp.body(owner, sig) or "")
        if outer not in consts:
            fail(f"{owner}.addAdditionalSaveData loads no `{outer}`")
        if field and outer == "VillagerData" and field not in vd_consts:
            fail(f"VillagerData carries no `{field}` field name")

    entities: dict[str, dict] = {}

    def add(owners, prop, domain, nbt, component):
        if domain not in domains:
            fail(f"{prop}: no domain {domain}")
        for owner in owners:
            jp.load([owner])
            for key, _, _ in nbt:
                prove_key(owner, key)
        types = sorted(t for t, ch in chains.items() if any(o in ch for o in owners))
        if not types:
            fail(f"no entity type is a {owners}")
        for t in types:
            row = entities.setdefault(t, {})
            if prop in row:
                fail(f"{t} carries `{prop}` twice")
            entry = {"domain": domain, "nbt": [{"key": k, "form": f, "shift": s} for k, f, s in nbt]}
            if component:
                entry["component"] = component
            row[prop] = entry
        return set(types)

    for field, (owners, prop, domain, nbt) in sorted(COMPONENTS.items()):
        types = add(owners, prop, domain, nbt, comp_ids[field])
        readers = {t for t, got in reads.items() if field in got}
        if readers != types:
            fail(
                f"{field}: types that read it {sorted(readers)} and types of {owners} {sorted(types)} disagree"
            )
    for owners, prop, domain, nbt in EXTRAS:
        add(owners, prop, domain, nbt, None)

    # Packed keys: shifted ids never overlap within one key.
    for t, props in entities.items():
        spans: dict[str, list[tuple[int, int, str]]] = {}
        for p, e in props.items():
            for n in e["nbt"]:
                if n["form"] in ("int", "byte"):
                    top = max(v.get("id", 0) for v in domains[e["domain"]])
                    spans.setdefault(n["key"], []).append((n["shift"], n["shift"] + max(top.bit_length(), 1), p))
        for key, ss in spans.items():
            ss.sort()
            for (a0, a1, pa), (b0, _, pb) in zip(ss, ss[1:]):
                if b0 < a1:
                    fail(f"{t}: `{pa}` and `{pb}` overlap in `{key}`")
        for p, e in props.items():
            for n in e["nbt"]:
                if n["form"] in ("int", "byte") and any("id" not in v for v in domains[e["domain"]]):
                    fail(f"{t}.{p}: a numeric form over a domain with no ids")

    used = {e["domain"] for props in entities.values() for e in props.values()}
    out = {
        "domains": {k: v for k, v in sorted(domains.items()) if k in used},
        "entities": {k: dict(sorted(v.items())) for k, v in sorted(entities.items())},
    }
    nprops = sum(len(v) for v in entities.values())
    sys.stderr.write(
        f"  {len(chains)} types: {len(entities)} carry a variant, {nprops} propert(ies); "
        f"{len(COMPONENTS)} component(s), {len(EXTRAS)} extra(s), {len(out['domains'])} domain(s)\n"
    )
    text = json.dumps(out, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    if args.check:
        if OUT_JSON.read_text(encoding="utf8") != text:
            fail(f"{OUT_JSON.relative_to(ROOT)} is stale — regenerate it")
        print(f"extract-entity-variants: up to date ({len(entities)} of {len(chains)} types)")
        return
    OUT_JSON.write_text(text, encoding="utf8")
    print(
        f"extract-entity-variants: wrote {len(entities)} of {len(chains)} entity types, "
        f"{nprops} properties, to {OUT_JSON.relative_to(ROOT)} (Minecraft {pin['version']})"
    )


main()
