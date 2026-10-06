#!/usr/bin/env python3
"""Regenerate `crates/delvec/data/environment-attributes-1.21.11.json` from the
pinned 1.21.11 jars (spec-0080 §2.4, §3.1.2), and cross-check the one particle
table (`crates/dsl/data/particles-1.21.11.json`) against the jar's bytecode.

The environment-attribute registry is what `world.atmospheres[].attributes` is
held to (`DW0928`): which ids exist, which a campaign may set, the shape of each
value, and the range the pinned codec rejects outside of. The particle table is
what an `ambient_particles` / `default_dripstone_particle` value is held to; it
is the same table the `particle` verb is held to (`DW0941`), written by
`tools/maintenance/extract-particle-registry.py` from the booted registry, and
this script reads the bytecode as a second method and refuses on any
disagreement rather than writing a second copy.

## Inputs (never committed: Mojang EULA)

    --server-jar  the pinned server jar (versions.toml [minecraft]
                  server_jar_sha256); its bundled
                  META-INF/versions/1.21.11/server-1.21.11.jar is read
    --client-jar  the 1.21.11 client jar (piston-meta downloads.client)
    --mappings    Mojang's official 1.21.11 SERVER mappings
                  (piston-meta downloads.server_mappings, sha1 below)

`javap` (any JDK) must be on PATH: the types and ranges are read out of the
bytecode of `EnvironmentAttributes.<clinit>`, never typed.

## Method, and the refusal

1. **Two instruments sharing no file.** Every `.class` of the bundled server jar,
   and separately every `.class` of the client jar, is scanned for strings
   matching `(visual|audio|gameplay)/[a-z_]+`; a string that names a loot table
   in the same jar (`data/minecraft/loot_table/<id>.json` — `gameplay/fishing`
   and eight more) is a loot-table id and not an attribute, and is dropped. The
   two lists must be identical, or this script refuses (exit 1).
2. **The registry itself.** The ids `EnvironmentAttributes.<clinit>` registers in
   the server jar (read through the mappings) must equal that list, or it
   refuses.
3. **Type and range per id**, from the same `<clinit>`: the `AttributeTypes`
   field each builder is made from, and the `AttributeRange` passed to
   `valueRange` (`UNIT_FLOAT`, `NON_NEGATIVE_FLOAT`, or `ofFloat(lo, hi)`), the
   bounds of the two named ranges read from `AttributeRange.<clinit>`. The
   attribute's value codec is `type.valueCodec().validate(range::validate)`
   (`EnvironmentAttribute.valueCodec`), so a value outside the range fails to
   load. No range → `null`, and the engine bounds nothing.
4. **Scope.** `gameplay/` ids are `gameplay` (spec-0080 §2.4: out of scope). A
   `visual/` id an overworld timeline (`dimension_type/overworld.json`
   `timelines`, the tag expanded) keys with modifier `override` is
   `overridden`: the day cycle replaces whatever a biome writes. Every other id
   is `admitted`.
5. **Records.** The fields of each record codec a structured value is built
   from (`AmbientParticle`, `AmbientSounds`, `AmbientMoodSettings`,
   `AmbientAdditionsSettings`, `BackgroundMusic`, `Music`): required
   (`fieldOf`) or optional (`optionalFieldOf`), and the range its codec
   validates (`Codec.floatRange`/`intRange`, or `ExtraCodecs.NON_NEGATIVE_INT`
   / `POSITIVE_INT`), else null.
6. **Particles.** `ParticleTypes.<clinit>` names each particle type and whether
   it is registered through `register(String, boolean)` — a
   `SimpleParticleType`, written `{"type": id}` with no options — or through a
   factory that takes options. A simple type is one the table records as
   `options: false`; every id and every answer must agree with it.

Usage:

    python3 tools/maintenance/extract-environment-attributes.py \\
      --server-jar server.jar --client-jar client.jar --mappings server.txt \\
      --out-attributes crates/delvec/data/environment-attributes-1.21.11.json \\
      --check-particles crates/dsl/data/particles-1.21.11.json
"""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import pathlib
import re
import subprocess
import sys
import tempfile
import zipfile

SERVER_JAR_SHA256 = "f83b8e093865806f931c7e34aae41b177d4c076335263dd124c75d6d65dd1726"
CLIENT_JAR_SHA256 = "1473c9489ac50fda3c435049a76a70d61a10b8610db27f5ba9d8756b686cd3bd"
SERVER_MAPPINGS_SHA1 = "5621e9253f05fd57872bbe7f8ddf5f9a7d525955"
INNER = "META-INF/versions/1.21.11/server-1.21.11.jar"
ID_RE = re.compile(rb"(?:visual|audio|gameplay)/[a-z_]+")

# The value shape a creator writes, per pinned `AttributeTypes` field. Only the
# types an admitted id is built from need one; an unknown type on an admitted id
# is a refusal, never a guess.
SHAPES = {
    "RGB_COLOR": "rgb",
    "ARGB_COLOR": "argb",
    "FLOAT": "float",
    "BOOLEAN": "boolean",
    "PARTICLE": "particle",
    "AMBIENT_PARTICLES": "ambient_particles",
    "BACKGROUND_MUSIC": "background_music",
    "AMBIENT_SOUNDS": "ambient_sounds",
}


def die(msg: str) -> int:
    sys.stderr.write(f"extract-environment-attributes: {msg}\n")
    return 1


def sha(path: pathlib.Path, algo: str) -> str:
    h = hashlib.new(algo)
    h.update(path.read_bytes())
    return h.hexdigest()


def class_strings(z: zipfile.ZipFile) -> set[str]:
    # A loot table is a file, or a directory of them (`gameplay/fishing/junk`),
    # so every directory prefix of a table's path names one too.
    loot: set[str] = set()
    for n in z.namelist():
        if n.startswith("data/minecraft/loot_table/") and n.endswith(".json"):
            parts = n[len("data/minecraft/loot_table/") : -len(".json")].split("/")
            for i in range(1, len(parts) + 1):
                loot.add("/".join(parts[:i]))
    found: set[str] = set()
    for n in z.namelist():
        if n.endswith(".class"):
            for m in ID_RE.findall(z.read(n)):
                found.add(m.decode())
    return {s for s in found if s not in loot}


class Mappings:
    """Mojang's ProGuard-format mappings: deobfuscated class → obfuscated
    name, and per class the obfuscated name of each field."""

    def __init__(self, text: str) -> None:
        self.cls: dict[str, str] = {}
        self.fields: dict[str, dict[str, str]] = {}
        cur = None
        for line in text.splitlines():
            if line.startswith("#") or not line.strip():
                continue
            if not line.startswith(" "):
                name, obf = line.rstrip(":").split(" -> ")
                self.cls[name] = obf
                self.fields[obf] = {}
                cur = obf
            elif cur and "(" not in line:
                parts = line.strip().split(" ")
                # `<type> <name> -> <obf>`
                if len(parts) == 4 and parts[2] == "->":
                    self.fields[cur][parts[3]] = parts[1]

    def obf(self, name: str) -> str:
        return self.cls[name]

    def field(self, cls_obf: str, field_obf: str) -> str:
        return self.fields[cls_obf][field_obf]


def javap(jar: zipfile.ZipFile, cls: str, tmp: pathlib.Path) -> str:
    target = tmp / f"{cls}.class"
    target.write_bytes(jar.read(f"{cls}.class"))
    r = subprocess.run(
        ["javap", "-c", "-p", str(target)], capture_output=True, text=True
    )
    if r.returncode != 0:
        raise SystemExit(die(f"javap {cls} failed: {r.stderr.strip()}"))
    return r.stdout


def static_block(code: str) -> list[str]:
    lines = code.splitlines()
    start = next(i for i, l in enumerate(lines) if l.strip() == "static {};")
    return lines[start:]


FLOAT_RE = re.compile(r"(?:ldc(?:_w)?\s+#\d+\s+// float (\S+)f|fconst_(\d))")


def floats_in(lines: list[str]) -> list[float]:
    out = []
    for l in lines:
        m = FLOAT_RE.search(l)
        if m:
            raw = m.group(1) if m.group(1) is not None else m.group(2)
            out.append(float("inf") if raw == "Infinity" else float(raw))
    return out


def attributes(jar: zipfile.ZipFile, maps: Mappings, tmp: pathlib.Path) -> list[dict]:
    env = maps.obf("net.minecraft.world.attribute.EnvironmentAttributes")
    types = maps.obf("net.minecraft.world.attribute.AttributeTypes")
    rng = maps.obf("net.minecraft.world.attribute.AttributeRange")
    # The two named ranges, read from AttributeRange.<clinit>: each is
    # `ofFloat(lo, hi)` stored into its field.
    named: dict[str, tuple[float, float]] = {}
    block = static_block(javap(jar, rng, tmp))
    pending: list[str] = []
    for l in block:
        pending.append(l)
        m = re.search(r"putstatic\s+#\d+\s+// Field (\w+):", l)
        if m:
            fl = floats_in(pending)
            named[maps.field(rng, m.group(1))] = (fl[0], fl[1])
            pending = []
    out = []
    block = static_block(javap(jar, env, tmp))
    chunk: list[str] = []
    ident = None

    def flush() -> None:
        if ident is None:
            return
        text = "\n".join(chunk)
        t = re.search(rf"getstatic\s+#\d+\s+// Field {re.escape(types)}\.(\w+):", text)
        if not t:
            raise SystemExit(die(f"{ident}: no AttributeTypes field in its builder"))
        type_name = maps.field(types, t.group(1))
        lo_hi = None
        r = re.search(rf"getstatic\s+#\d+\s+// Field {re.escape(rng)}\.(\w+):", text)
        if r:
            lo_hi = named[maps.field(rng, r.group(1))]
        elif re.search(rf"Method {re.escape(rng)}\.a:\(FF\)", text) or re.search(
            rf"InterfaceMethod {re.escape(rng)}\.a:\(FF\)", text
        ):
            idx = next(i for i, l in enumerate(chunk) if f"{rng}.a:(FF)" in l)
            fl = floats_in(chunk[:idx])
            lo_hi = (fl[-2], fl[-1])
        out.append(
            {
                "id": ident,
                "type": type_name,
                "range": None
                if lo_hi is None
                else [lo_hi[0], None if lo_hi[1] == float("inf") else lo_hi[1]],
            }
        )

    for l in block:
        m = re.search(r"ldc(?:_w)?\s+#\d+\s+// String ((?:visual|audio|gameplay)/[a-z_]+)$", l)
        if m:
            flush()
            ident, chunk = m.group(1), []
            continue
        chunk.append(l)
    flush()
    return out


def particles(jar: zipfile.ZipFile, maps: Mappings, tmp: pathlib.Path) -> list[dict]:
    cls = maps.obf("net.minecraft.core.particles.ParticleTypes")
    simple = maps.obf("net.minecraft.core.particles.SimpleParticleType")
    block = static_block(javap(jar, cls, tmp))
    out = []
    name = None
    for l in block:
        m = re.search(r"ldc(?:_w)?\s+#\d+\s+// String ([a-z_]+)$", l)
        if m:
            name = m.group(1)
            continue
        m = re.search(r"invokestatic\s+#\d+\s+// Method (?:\w+\.)?\w+:\(([^)]*)\)L(\w+);", l)
        if m and name is not None:
            out.append({"id": f"minecraft:{name}", "simple": m.group(2) == simple})
            name = None
    return sorted(out, key=lambda p: p["id"])


# The record codecs an admitted value is built from, by Mojang name.
RECORDS = {
    "ambient_particle": "net.minecraft.world.attribute.AmbientParticle",
    "ambient_sounds": "net.minecraft.world.attribute.AmbientSounds",
    "ambient_mood": "net.minecraft.world.attribute.AmbientMoodSettings",
    "ambient_additions": "net.minecraft.world.attribute.AmbientAdditionsSettings",
    "background_music": "net.minecraft.world.attribute.BackgroundMusic",
    "music": "net.minecraft.sounds.Music",
}
# A named `ExtraCodecs` codec whose name states the range it validates.
NAMED_INT_RANGES = {"NON_NEGATIVE_INT": [0, None], "POSITIVE_INT": [1, None]}
CONST_RE = re.compile(
    r"(?:fconst_(\d)|iconst_(\d)|bipush\s+(-?\d+)|sipush\s+(-?\d+)|"
    r"ldc(?:_w|2_w)?\s+#\d+\s+// (?:float|int|double) (\S+?)[fd]?$)"
)


def record_fields(jar: zipfile.ZipFile, maps: Mappings, tmp: pathlib.Path) -> dict:
    """Each record codec's fields: required or optional, and the range its
    codec validates (`Codec.floatRange`/`intRange`, or a named
    `ExtraCodecs` int codec), else null."""
    extra = maps.obf("net.minecraft.util.ExtraCodecs")
    out: dict[str, dict] = {}
    for key, name in RECORDS.items():
        code = javap(jar, maps.obf(name), tmp).splitlines()
        fields: dict[str, dict] = {}
        consts: list[float] = []
        rng = None
        for i, l in enumerate(code):
            m = CONST_RE.search(l)
            if m:
                raw = next(g for g in m.groups() if g is not None)
                consts.append(float(raw))
                continue
            if "Codec.floatRange:" in l or "Codec.intRange:" in l:
                rng = [consts[-2], consts[-1]]
                continue
            g = re.search(rf"getstatic\s+#\d+\s+// Field {re.escape(extra)}\.(\w+):", l)
            if g:
                rng = NAMED_INT_RANGES.get(maps.field(extra, g.group(1)))
                continue
            f = re.search(r"ldc(?:_w)?\s+#\d+\s+// String ([a-z_]+)$", l)
            # The field call follows the name, after the default value an
            # `optionalFieldOf(name, default)` pushes first.
            call = next((c for c in code[i + 1 : i + 4] if "ieldOf:" in c), None) if f else None
            if f and call is not None:
                fields[f.group(1)] = {
                    "required": "optionalFieldOf" not in call,
                    "range": rng,
                }
                rng, consts = None, []
        out[key] = fields
    return out


def overworld_overrides(jar: zipfile.ZipFile) -> dict[str, str]:
    """Every attribute an overworld timeline keys, with its modifier."""
    dim = json.loads(jar.read("data/minecraft/dimension_type/overworld.json"))

    def expand(ref: str) -> list[str]:
        if ref.startswith("#"):
            tag = json.loads(
                jar.read(f"data/minecraft/tags/timeline/{ref[1:].split(':')[1]}.json")
            )
            return [t for v in tag["values"] for t in expand(v)]
        return [ref.split(":")[1]]

    refs = dim["timelines"]
    names = expand(refs) if isinstance(refs, str) else [t for r in refs for t in expand(r)]
    mods: dict[str, str] = {}
    for n in names:
        tl = json.loads(jar.read(f"data/minecraft/timeline/{n}.json"))
        for k, v in tl.get("tracks", {}).items():
            mods[k.split(":", 1)[1]] = v.get("modifier", "override")
    return mods


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--server-jar", required=True, type=pathlib.Path)
    ap.add_argument("--client-jar", required=True, type=pathlib.Path)
    ap.add_argument("--mappings", required=True, type=pathlib.Path)
    ap.add_argument("--out-attributes", required=True, type=pathlib.Path)
    ap.add_argument("--check-particles", required=True, type=pathlib.Path)
    a = ap.parse_args()
    if sha(a.server_jar, "sha256") != SERVER_JAR_SHA256:
        return die(f"{a.server_jar} is not the pinned server jar")
    if sha(a.client_jar, "sha256") != CLIENT_JAR_SHA256:
        return die(f"{a.client_jar} is not the pinned 1.21.11 client jar")
    if sha(a.mappings, "sha1") != SERVER_MAPPINGS_SHA1:
        return die(f"{a.mappings} is not the official 1.21.11 server mappings")
    outer = zipfile.ZipFile(a.server_jar)
    server = zipfile.ZipFile(io.BytesIO(outer.read(INNER)))
    client = zipfile.ZipFile(a.client_jar)
    s_ids, c_ids = class_strings(server), class_strings(client)
    if s_ids != c_ids:
        return die(
            "the two jars disagree: server-only "
            f"{sorted(s_ids - c_ids)}, client-only {sorted(c_ids - s_ids)}"
        )
    maps = Mappings(a.mappings.read_text())
    with tempfile.TemporaryDirectory(dir=a.out_attributes.parent) as t:
        tmp = pathlib.Path(t)
        attrs = attributes(server, maps, tmp)
        parts = particles(server, maps, tmp)
        records = record_fields(server, maps, tmp)
    registered = {x["id"] for x in attrs}
    if registered != s_ids:
        return die(
            "EnvironmentAttributes registers a different set than the strings read: "
            f"registered-only {sorted(registered - s_ids)}, strings-only {sorted(s_ids - registered)}"
        )
    mods = overworld_overrides(server)
    rows = []
    for x in sorted(attrs, key=lambda x: x["id"]):
        ident = x["id"]
        modifier = mods.get(ident)
        if ident.startswith("gameplay/"):
            scope = "gameplay"
        elif ident.startswith("visual/") and modifier == "override":
            scope = "overridden"
        else:
            scope = "admitted"
        shape = SHAPES.get(x["type"])
        if scope == "admitted" and shape is None:
            return die(f"admitted `{ident}` is built from `{x['type']}`, which has no shape")
        rows.append(
            {
                "id": ident,
                "scope": scope,
                "type": x["type"],
                "shape": shape if scope == "admitted" else None,
                "range": x["range"],
                "overworld_timeline": modifier,
            }
        )
    counts = {s: sum(r["scope"] == s for r in rows) for s in ("admitted", "overridden", "gameplay")}
    doc = {
        "minecraft": "1.21.11",
        "source": {
            "server_jar_sha256": SERVER_JAR_SHA256,
            "client_jar_sha256": CLIENT_JAR_SHA256,
            "server_mappings_sha1": SERVER_MAPPINGS_SHA1,
            "registry": "net.minecraft.world.attribute.EnvironmentAttributes.<clinit>",
            "ranges": "net.minecraft.world.attribute.AttributeRange (validated by EnvironmentAttribute.valueCodec)",
            "scope": "data/minecraft/dimension_type/overworld.json timelines, expanded",
        },
        "counts": counts,
        "attributes": rows,
        "records": records,
    }
    a.out_attributes.write_text(json.dumps(doc, indent=2, sort_keys=True) + "\n")
    table = json.loads(a.check_particles.read_text())
    read = {p["id"]: not p["simple"] for p in parts}
    held = {k: v["options"] for k, v in table.items()}
    if read != held:
        return die(
            f"the bytecode's particle types disagree with {a.check_particles}: "
            f"only in the jar {sorted(set(read) - set(held))}, only in the table "
            f"{sorted(set(held) - set(read))}, options differ "
            f"{sorted(k for k in set(read) & set(held) if read[k] != held[k])}"
        )
    print(
        f"{len(rows)} attribute(s): {counts['admitted']} admitted, {counts['overridden']} "
        f"overridden, {counts['gameplay']} gameplay; {len(parts)} particle type(s), "
        f"{sum(p['simple'] for p in parts)} simple, agreeing with {a.check_particles}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
