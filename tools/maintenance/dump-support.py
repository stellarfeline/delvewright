#!/usr/bin/env python3
"""Regenerate the support table from the PINNED Minecraft server jar.

`crates/dsl/data/support-<version>.tsv` is what `delvewright_dsl::support` reads
when the build asks whether a block it writes is one the server keeps: a wall
torch on a glass pane, a lantern hung under air, a flower on stone — each a block
the pinned server drops the first time a shape update reaches it (`DW1002`). It
is a measurement, not a table somebody typed: every row comes from asking the
game's own `BlockState.canSurvive(LevelReader, BlockPos)` inside the pinned
server jar, over a level that holds the state and one neighbour, for every
neighbour cell and every blockstate of the registry
(`tools/maintenance/support/SupportDump.java` says how, and what it cannot read).

`crates/dsl/data/support-bases-<version>.tsv` is the second half: per state,
what the support table's base sets ask of a neighbour — the faces the game calls
CENTER- and RIGID-sturdy (`isFaceSturdy` with `SupportType.CENTER` / `RIGID`,
which a torch's floor and a rail's bed are asked), and whether it is air
(`isAir`), solid (`isSolid`) or holds still water (its fluid is `Fluids.WATER`).
The FULL faces are `faces-<version>.tsv`'s `sturdy` column.

    tools/maintenance/dump-support.py            # rewrite both tables
    tools/maintenance/dump-support.py --check    # regenerate and diff

The pin, fetch and collapse steps are `dump-collision-tops.py`'s own, imported,
not copied. Members are resolved from the official mappings by EXACT signature
(an overload can carry a different obfuscated name: `isFaceSturdy/3` is not
`isFaceSturdy/4`), and no obfuscated name is written down here or in the Java.

Requires a JDK (>= the pin's `javaVersion.majorVersion`) on PATH and network
access. A REGENERATION tool: CI reads the committed tables and never runs this.
Run it when the MC pin moves.
"""

from __future__ import annotations

import argparse
import collections
import hashlib
import importlib.util
import pathlib
import re
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[2]
JAVA_SRC = ROOT / "tools" / "maintenance" / "support" / "SupportDump.java"
DATA_DIR = ROOT / "crates" / "dsl" / "data"

_spec = importlib.util.spec_from_file_location(
    "dump_collision_tops", ROOT / "tools" / "maintenance" / "dump-collision-tops.py"
)
tops = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(tops)

# The cross-check's sample: fixed, so two runs agree byte for byte.
SEED = 1002
SAMPLES = 256

# The pinned overworld's build height (`dimension_type/overworld.json`: min_y
# -64, height 384), which the probe level states when a rule asks.
MIN_Y = -64
HEIGHT = 384

NS = "net.minecraft."
# key -> deobfuscated class
CLASSES = {
    "SharedConstants": "SharedConstants",
    "Bootstrap": "server.Bootstrap",
    "PackType": "server.packs.PackType",
    "ServerPacksSource": "server.packs.repository.ServerPacksSource",
    "ResourceManager": "server.packs.resources.ResourceManager",
    "MultiPackResourceManager": "server.packs.resources.MultiPackResourceManager",
    "Registry": "core.Registry",
    "TagLoader": "tags.TagLoader",
    "PendingTags": "core.Registry$PendingTags",
    "BuiltInRegistries": "core.registries.BuiltInRegistries",
    "Direction": "core.Direction",
    "BlockPos": "core.BlockPos",
    "BlockGetter": "world.level.BlockGetter",
    "LevelReader": "world.level.LevelReader",
    "LevelHeightAccessor": "world.level.LevelHeightAccessor",
    "SupportType": "world.level.block.SupportType",
    "BlockStateBase": "world.level.block.state.BlockBehaviour$BlockStateBase",
    "Block": "world.level.block.Block",
    "EmptyBlockGetter": "world.level.EmptyBlockGetter",
    "FluidState": "world.level.material.FluidState",
    "Fluids": "world.level.material.Fluids",
}
FIELDS = {
    "PackType": ["SERVER_DATA"],
    "BuiltInRegistries": ["BLOCK", "FLUID"],
    "Direction": ["DOWN", "UP", "NORTH", "SOUTH", "WEST", "EAST"],
    "BlockPos": ["ZERO"],
    "SupportType": ["FULL", "CENTER", "RIGID"],
    "Block": ["BLOCK_STATE_REGISTRY"],
    "EmptyBlockGetter": ["INSTANCE"],
    "Fluids": ["WATER"],
}
# key -> [(method name, exact parameter list)]
METHODS = {
    "SharedConstants": [("tryDetectVersion", "")],
    "Bootstrap": [("bootStrap", "")],
    "ServerPacksSource": [("createVanillaPackSource", "")],
    "TagLoader": [
        (
            "loadPendingTags",
            "net.minecraft.server.packs.resources.ResourceManager,net.minecraft.core.Registry",
        )
    ],
    "PendingTags": [("apply", "")],
    "BlockPos": [("relative", "net.minecraft.core.Direction")],
    "BlockGetter": [
        ("getBlockState", "net.minecraft.core.BlockPos"),
        ("getFluidState", "net.minecraft.core.BlockPos"),
        ("getBlockEntity", "net.minecraft.core.BlockPos"),
    ],
    "LevelReader": [("isClientSide", "")],
    "FluidState": [("getType", "")],
    "LevelHeightAccessor": [("getMinY", ""), ("getHeight", "")],
    "BlockStateBase": [
        ("canSurvive", "net.minecraft.world.level.LevelReader,net.minecraft.core.BlockPos"),
        ("isAir", ""),
        ("isSolid", ""),
        ("getFluidState", ""),
        (
            "isFaceSturdy",
            "net.minecraft.world.level.BlockGetter,net.minecraft.core.BlockPos,"
            "net.minecraft.core.Direction,net.minecraft.world.level.block.SupportType",
        ),
    ],
}

LETTERS = "dunswe"


def fail(msg: str) -> None:
    sys.exit(f"dump-support: {msg}")


def parse_signatures(path: pathlib.Path) -> dict[str, tuple[str, dict, dict]]:
    """ProGuard mappings -> {deobf class: (obf, {field: obf}, {(method, params): {(return type, obf)}})}.
    Methods are keyed by name AND exact parameter list, so an overload is never
    answered by its sibling's obfuscated name."""
    out: dict[str, tuple[str, dict, dict]] = {}
    cur = None
    for raw in path.read_text(encoding="utf8").splitlines():
        if raw.startswith("#"):
            continue
        if raw[:1] not in (" ", "\t"):
            m = re.match(r"^(\S+) -> (\S+):$", raw)
            cur = None
            if m:
                cur = (m.group(2), {}, {})
                out[m.group(1)] = cur
            continue
        if cur is None:
            continue
        m = re.match(r"^(?:\d+:\d+:)?(\S+) ([\w$<>]+)(\(([^)]*)\))? -> (\S+)$", raw.strip())
        if not m:
            continue
        if m.group(3) is None:
            cur[1].setdefault(m.group(2), m.group(5))
        else:
            # A covariant override is listed twice under one signature; the
            # ambiguity is refused only where a member is asked for.
            cur[2].setdefault((m.group(2), m.group(4)), set()).add((m.group(1), m.group(5)))
    return out


def resolve_args(maps) -> list[str]:
    args = []
    for key, deobf in CLASSES.items():
        full = NS + deobf
        if full not in maps:
            fail(f"mappings have no class {full} — the pin moved under this tool")
        obf, fields, methods = maps[full]
        args.append(f"{key}={obf}")
        for f in FIELDS.get(key, []):
            if f not in fields:
                fail(f"mappings have no field {full}.{f}")
            args.append(f"{key}.{f}={fields[f]}")
        for name, params in METHODS.get(key, []):
            # A covariant bridge shares the signature and returns a supertype
            # (`BlockPos.relative` beside `Vec3i.relative`); where one of them
            # returns the class itself, that one is the member asked for.
            found = methods.get((name, params), set())
            obfs = {o for r, o in found if r == full} or {o for _, o in found}
            if len(obfs) != 1:
                fail(f"mappings give {full}.{name}({params}) {len(obfs)} names: {sorted(obfs)}")
            args.append(f"{key}.{name}={next(iter(obfs))}")
    return args


def state_names(faces_tsv: pathlib.Path) -> list[str]:
    """The registry, in index order, as the dumper wrote it."""
    return [ln.split("\t", 1)[0] for ln in faces_tsv.read_text(encoding="utf8").splitlines()]


def parse_state(s: str) -> tuple[str, dict[str, str]]:
    m = tops.STATE_RE.match(s)
    if not m:
        fail(f"unparsable blockstate from the dumper: {s!r}")
    props = {}
    if m.group(2):
        for kv in m.group(2).split(","):
            k, v = kv.split("=", 1)
            props[k] = v
    return m.group(1), props


def collapse_default_last(per: dict) -> tuple[list[tuple[str, tuple, int]], int]:
    """[`collapse_values`], then each block's most common value (ties: the
    smallest) written once, as a bare row LAST, in place of every row that
    carries it. A reader takes the first row whose properties a state matches
    (`delvewright_dsl::blockshape`'s `pinned_row`); the rows `collapse_values`
    writes partition the block's states, so a state the earlier rows do not
    select is one the bare row stands for. A wall's centre faces move with four
    connections and `up` on one state of its 324, and this writes that state and
    the rest, not 162 rows."""
    rows, covered = tops.collapse_values(per)
    by_block: dict[str, list] = collections.defaultdict(list)
    for name, value, count in rows:
        by_block[name.split("[", 1)[0]].append((name, value, count))
    out = []
    for block in sorted(by_block):
        group = by_block[block]
        tally = collections.Counter()
        for _, value, count in group:
            tally[value] += count
        top = max(sorted(tally), key=lambda v: tally[v])
        out.extend(r for r in group if r[1] != top)
        out.append((block, top, tally[top]))
    return out, covered


def name_set(indices: list[int], names: list[str], per_block: dict) -> list[str]:
    """A set of blockstates as the fewest rows that name it: a block whose every
    state is in the set by its id alone, otherwise the properties that separate
    its members from its other states (the one collapse, over membership)."""
    chosen = collections.defaultdict(set)
    for i in indices:
        block, _ = parse_state(names[i])
        chosen[block].add(i)
    out = []
    for block in sorted(chosen):
        members = per_block[block]
        if len(chosen[block]) == len(members):
            out.append(f"minecraft:{block}")
            continue
        per = {block: [(parse_state(names[i])[1], (i in chosen[block],)) for i in members]}
        rows, _ = tops.collapse_values(per)
        out.extend(n for n, (inside,), _ in rows if inside)
    return out


def rule_text(rule: str, names: list[str], per_block: dict) -> str:
    """`base+i,j-k` (indices) -> `base+name|name-name` (collapsed names)."""
    m = re.match(r"^([a-z]+)(?:\+([\d,]+))?(?:-([\d,]+))?$", rule)
    if not m:
        fail(f"unparsable support rule from the dumper: {rule!r}")
    text = m.group(1)
    for sign, group in (("+", m.group(2)), ("-", m.group(3))):
        if group:
            text += sign + "|".join(name_set([int(x) for x in group.split(",")], names, per_block))
    return text


def generate(work: pathlib.Path) -> tuple[str, str, str]:
    version, jar, mappings, classpath, _ = tops.prepare(work)
    maps = parse_signatures(mappings)
    args = resolve_args(maps)
    args += [f"minY={MIN_Y}", f"height={HEIGHT}", f"seed={SEED}", f"samples={SAMPLES}"]
    sys.stderr.write(f"  resolved from the pinned mappings: {' '.join(args)}\n")

    classes = work / "classes-support"
    classes.mkdir(exist_ok=True)
    subprocess.run(
        ["javac", "-nowarn", "-cp", classpath, "-d", str(classes), str(JAVA_SRC)], check=True
    )
    states_tsv = work / "support-states.tsv"
    faces_tsv = work / "support-face-states.tsv"
    proc = subprocess.run(
        [
            "java",
            "-Xmx3g",
            "-cp",
            f"{classpath}:{classes}",
            "dw.SupportDump",
            str(states_tsv),
            str(faces_tsv),
            *args,
        ],
        check=True,
        stdout=subprocess.PIPE,
        text=True,
        cwd=work,
    )
    counted = re.search(
        r"DUMPED states=(\d+) free=(\d+) supported=(\d+) unjudged=(\d+) tag-registries=(\d+) "
        r"probes=(\d+) killed-with-support=(\d+)",
        proc.stdout,
    )
    if not counted or any(int(counted.group(i)) == 0 for i in (1, 2, 3, 5, 6)):
        fail(f"the dumper reported nothing to count: {proc.stdout.strip()!r}")
    if int(counted.group(5)) != 2:
        fail(f"the dumper bound {counted.group(5)} tag registries, not 2")
    unjudged_line = re.search(r"UNJUDGED (\{[^}]*\})", proc.stdout)
    if not unjudged_line:
        fail(f"the dumper stated no unjudged breakdown: {proc.stdout.strip()[-400:]!r}")
    sys.stderr.write(f"  dumper: {counted.group(0)}\n  dumper: {unjudged_line.group(0)}\n")
    total = int(counted.group(1))

    names = state_names(faces_tsv)
    if len(names) != total:
        fail(f"the face dump holds {len(names)} states, the dumper counted {total}")
    per_block = collections.defaultdict(list)
    for i, s in enumerate(names):
        per_block[parse_state(s)[0]].append(i)

    # The support table: per state, (kind, rule).
    per: dict[str, list] = collections.defaultdict(list)
    lines = states_tsv.read_text(encoding="utf8").splitlines()
    if len(lines) != total:
        fail(f"the support dump holds {len(lines)} states, the dumper counted {total}")
    memo: dict[str, str] = {}
    # Which face of N each face-reading base is asked about, over every rule
    # the table holds: the base table carries those faces and no others.
    asked: dict[str, set[str]] = collections.defaultdict(set)
    opposite = dict(zip(LETTERS, "udnsew"))
    for ln in lines:
        cols = ln.split("\t")
        state, kind = cols[0], cols[1]
        if kind == "free":
            value = ("free", "-")
        elif kind == "support":
            parts = []
            for c in cols[2:]:
                d, rule = c.split(":", 1)
                if rule == "none":
                    continue
                asked[re.match(r"^[a-z]+", rule).group(0)].add(opposite[d])
                if rule not in memo:
                    memo[rule] = rule_text(rule, names, per_block)
                parts.append(f"{d}:{memo[rule]}")
            value = ("support", " ".join(parts))
        elif kind == "unjudged":
            value = ("unjudged", cols[2].replace("\t", " "))
        else:
            fail(f"unknown verdict {kind!r} for {state}")
        block, props = parse_state(state)
        per[block].append((props, value))
    rows, covered = collapse_default_last(per)
    if covered != total:
        fail(f"collapse covered {covered} states, the dumper produced {total}")

    # Each base column is collapsed on its own: a wall's centre faces move with
    # its connections and its water with `waterlogged`, and one row per pair
    # of them would write the product.
    face_per = tops.read_states(faces_tsv, 3)
    face_rows = []
    carried = {
        "center": "".join(f for f in LETTERS if f in asked["center"]),
        "rigid": "".join(f for f in LETTERS if f in asked["rigid"]),
    }
    if not carried["center"] or not carried["rigid"]:
        fail(f"no rule asks a centre or rigid face: {carried}")

    def keep(label: str, value: str) -> str:
        if label not in carried:
            return value
        kept = "".join(f for f in value if f in carried[label])
        return kept or "-"

    for col, label in enumerate(("center", "rigid", "flags")):
        one = {b: [(props, (keep(label, v[col]),)) for props, v in st] for b, st in face_per.items()}
        rows_c, covered_c = collapse_default_last(one)
        if covered_c != total:
            fail(f"{label} collapse covered {covered_c} states, the dumper produced {total}")
        face_rows.extend((n, label, v, c) for n, (v,), c in rows_c)

    jar_sha = hashlib.sha256(jar.read_bytes()).hexdigest()
    map_sha = hashlib.sha1(mappings.read_bytes()).hexdigest()
    kinds = collections.Counter(v[0] for _, v, _ in rows)
    state_kinds = collections.Counter()
    for _, v, c in rows:
        state_kinds[v[0]] += c
    head = [
        f"# What every blockstate of Minecraft Java {version} needs beside it to survive.",
        "#",
        "# MEASURED, not written: each state S was set alone in a level of air and",
        "# asked the game's own BlockState.canSurvive(level, pos) — once in air, then",
        "# once for every neighbour cell and every blockstate N of the registry at it.",
        "#   free      S survives in air: it needs nothing beside it.",
        "#   support   S survives only beside one of the sets named, any one enough:",
        "#             <cell>:<base>[+<added>][-<removed>], cell one of d u n s w e",
        "#             (down, up, north, south, west, east of S); base one of",
        "#             full / center / rigid (N.isFaceSturdy(face toward S, SupportType",
        "#             FULL / CENTER / RIGID)), nonair (!N.isAir()), solid",
        "#             (N.isSolid()), water (N's fluid is Fluids.WATER) or none — the",
        "#             base of the fewest differences; added and removed are N rows",
        "#             separated by `|`. A cell not named holds nothing for S.",
        "#   unjudged  the single-neighbour reading cannot be S's whole rule; the",
        "#             reason: far (it read a cell beyond the six), none (no single",
        "#             neighbour keeps it), error (it asked the level something the",
        "#             probe does not model), combined (a seeded sample of whole",
        f"#             neighbourhoods, {SAMPLES} per state, seed {SEED}, found it kept by",
        "#             no single neighbour). A reader judges nothing for it.",
        f"#   server jar sha256 {jar_sha}",
        f"#   server mappings sha1 {map_sha}",
        "# Regenerate with tools/maintenance/dump-support.py; --check diffs against this file.",
        "#",
        f"# {len(rows)} rows collapse the game's {covered} blockstates onto the properties",
        "# that move the verdict; a row names only those properties, and a state takes",
        "# the FIRST row of its block it matches: each block's most common verdict is",
        "# its last row, bare.",
        f"# Rows: free {kinds['free']}, support {kinds['support']}, unjudged {kinds['unjudged']}.",
        f"# States: free {state_kinds['free']}, support {state_kinds['support']}, "
        f"unjudged {state_kinds['unjudged']} ({unjudged_line.group(1)}).",
        f"# The sample found {counted.group(7)} neighbourhood(s) where a support was present and",
        "# the game still dropped S (a second, killing condition, such as a cactus",
        "# beside a wall): the table answers what holds a block, not all that breaks it.",
        "#",
        "# block[relevant-properties]<TAB>kind<TAB>rule<TAB>states-this-row-stands-for",
    ]
    body = "\n".join(f"{n}\t{k}\t{r}\t{c}" for n, (k, r), c in rows)
    support = "\n".join(head) + "\n" + body + "\n"

    fhead = [
        f"# What the support table's base sets ask of every blockstate of Minecraft Java {version}.",
        "#",
        "# MEASURED, not written: for each of the six faces, in the order d u n s w e,",
        "# `center` names the faces for which the game's own",
        "# BlockState.isFaceSturdy(EmptyBlockGetter.INSTANCE, BlockPos.ZERO, face,",
        "# SupportType.CENTER) answers true, and `rigid` those for SupportType.RIGID;",
        "# `flags` holds a when BlockState.isAir(), s when BlockState.isSolid(), w when",
        "# getFluidState().getType() is Fluids.WATER; `-` when none. FULL is",
        "# faces-<version>.tsv's `sturdy` column.",
        "# A face column carries only the faces some support rule asks of it:",
        f"#   center faces carried: {carried['center']}",
        f"#   rigid faces carried: {carried['rigid']}",
        f"#   server jar sha256 {jar_sha}",
        f"#   server mappings sha1 {map_sha}",
        "# Regenerate with tools/maintenance/dump-support.py; --check diffs against this file.",
        "#",
        f"# Each column is its own collapse of the game's {total} blockstates onto the",
        "# properties that move it, written as rows of that column; a row names only",
        "# those properties, and a state takes the FIRST row of its block in that",
        "# column it matches: each block's most common value is its last row, bare.",
        f"# {len(face_rows)} rows.",
        "#",
        "# block[relevant-properties]<TAB>column<TAB>value<TAB>states-this-row-stands-for",
    ]
    fbody = "\n".join(f"{n}\t{label}\t{v}\t{k}" for n, label, v, k in face_rows)
    faces = "\n".join(fhead) + "\n" + fbody + "\n"
    return version, support, faces


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--check", action="store_true", help="diff against the committed tables")
    ap.add_argument("--work", help="cache directory for the jar and mappings")
    args = ap.parse_args()
    work = (
        pathlib.Path(args.work).resolve()
        if args.work
        else pathlib.Path(tempfile.mkdtemp(prefix="dwsupport-"))
    )
    version, support, faces = generate(work)
    targets = [
        (DATA_DIR / f"support-{version}.tsv", support),
        (DATA_DIR / f"support-bases-{version}.tsv", faces),
    ]
    if args.check:
        bad = []
        for target, text in targets:
            if not target.exists():
                fail(f"{target} does not exist")
            if target.read_text(encoding="utf8") != text:
                bad.append(str(target.relative_to(ROOT)))
        if bad:
            sys.exit(f"dump-support: {', '.join(bad)} DISAGREE(S) with the pinned jar")
        for target, _ in targets:
            print(f"dump-support: {target.relative_to(ROOT)} matches the pinned jar")
        return
    for target, text in targets:
        target.write_text(text, encoding="utf8")
        print(f"dump-support: wrote {target.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
