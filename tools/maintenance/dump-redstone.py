#!/usr/bin/env python3
"""Regenerate the redstone table from the PINNED Minecraft server jar.

`crates/dsl/data/redstone-<version>.tsv` is what `delvewright_dsl::blockshape`
reads when the compiler asks what a sculk sensor's power can reach (spec-0100
§4.2): which blocks conduct a strong signal, which are signal sources, and which
read a signal at all. It is a measurement, not a table somebody typed:

- `conductor` — the game's own `BlockState.isRedstoneConductor(EmptyBlockGetter.INSTANCE,
  BlockPos.ZERO)` over every state, set against `isCollisionShapeFullBlock` (the
  game's default): `shape` when the two agree on every state, else `always` or
  `never`; a block that is none of the three is refused.
- `signal_source` — `BlockState.isSignalSource()` over every state: `true` when
  any state is one (the over-approximation the containment rule wants).
- `reads_signal` — whether any method of the block's class or an ancestor below
  `BlockBehaviour`, of its block-entity class and ancestors below `BlockEntity`
  when it is an `EntityBlock`, or of a class nested in any of them, invokes one
  of `SignalGetter`'s seven methods on a receiver assignable to `SignalGetter`;
  read from `javap -c -p` of the bundled server jar, the seven methods'
  obfuscated names and descriptors resolved from the mappings.

    tools/maintenance/dump-redstone.py            # rewrite the table
    tools/maintenance/dump-redstone.py --check    # regenerate and diff

The pin, fetch, mapping steps are `dump-collision-tops.py`'s own, imported, not
copied. No obfuscated name is written down here or in the Java dumper
(`tools/maintenance/collision/RedstoneDump.java`).

Requires a JDK (>= the pin's `javaVersion.majorVersion`, `javap` included) on PATH
and network access. A REGENERATION tool: CI reads the committed table and never
runs this. Run it when the MC pin moves.
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
import zipfile

ROOT = pathlib.Path(__file__).resolve().parents[2]
JAVA_SRC = ROOT / "tools" / "maintenance" / "collision" / "RedstoneDump.java"
DATA_DIR = ROOT / "crates" / "dsl" / "data"

_spec = importlib.util.spec_from_file_location(
    "dump_collision_tops", ROOT / "tools" / "maintenance" / "dump-collision-tops.py"
)
tops = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(tops)
light = tops.light

CLASS_DIRECTION = "net.minecraft.core.Direction"
CLASS_BEHAVIOUR = "net.minecraft.world.level.block.state.BlockBehaviour"
CLASS_ENTITY_BLOCK = "net.minecraft.world.level.block.EntityBlock"
CLASS_BLOCK_ENTITY = "net.minecraft.world.level.block.entity.BlockEntity"
CLASS_SIGNAL_GETTER = "net.minecraft.world.level.SignalGetter"
SIGNAL_METHODS = (
    "getSignal",
    "getDirectSignal",
    "getDirectSignalTo",
    "getControlInputSignal",
    "hasSignal",
    "hasNeighborSignal",
    "getBestNeighborSignal",
)
PRIMITIVE = {
    "int": "I",
    "boolean": "Z",
    "void": "V",
    "long": "J",
    "byte": "B",
    "short": "S",
    "char": "C",
    "float": "F",
    "double": "D",
}

INVOKE_RE = re.compile(r"//\s+(?:Interface)?Method ([\w$/.]+)\.([\w$<>]+):(\(\S*)")
CLASS_HEAD_RE = re.compile(
    r"^(?:(?:public|protected|private|abstract|final|static|sealed|non-sealed|strictfp)\s+)*"
    r"(?:class|interface|enum|record)\s+([\w$.]+)"
)


def obf_type(java: str, classes: dict[str, str]) -> str:
    """A mapped Java type name as a JVM descriptor in the obfuscated jar."""
    dims = java.count("[]")
    base = java.replace("[]", "")
    if base in PRIMITIVE:
        d = PRIMITIVE[base]
    elif base in classes:
        d = "L" + classes[base].replace(".", "/") + ";"
    else:
        d = "L" + base.replace(".", "/") + ";"
    return "[" * dims + d


def signal_methods(mappings: pathlib.Path, maps) -> set[tuple[str, str]]:
    """The seven `SignalGetter` methods as (obfuscated name, obfuscated
    descriptor), every overload, read from the mappings by mapped name."""
    classes = {k: v[0] for k, v in maps.items()}
    out: set[tuple[str, str]] = set()
    seen: set[str] = set()
    inside = False
    for raw in mappings.read_text(encoding="utf8").splitlines():
        if raw.startswith("#"):
            continue
        if raw[:1] not in (" ", "\t"):
            inside = raw.startswith(CLASS_SIGNAL_GETTER + " -> ")
            continue
        if not inside:
            continue
        m = re.match(r"^(?:\d+:\d+:)?(\S+) (\w+)\(([^)]*)\) -> (\S+)$", raw.strip())
        if not m or m.group(2) not in SIGNAL_METHODS:
            continue
        ret, name, params, obf = m.groups()
        desc = (
            "("
            + "".join(obf_type(p, classes) for p in params.split(",") if p)
            + ")"
            + obf_type(ret, classes)
        )
        out.add((obf, desc))
        seen.add(name)
    missing = set(SIGNAL_METHODS) - seen
    if missing:
        tops.fail(f"mappings have no SignalGetter method {sorted(missing)}")
    return out


def javap_classes(inner_jar: str, names: list[str], work: pathlib.Path) -> dict[str, list[tuple[str, str, str]]]:
    """Every invocation in each named class: (receiver, name, descriptor), from
    `javap -c -p` of the bundled server jar."""
    out: dict[str, list[tuple[str, str, str]]] = {}
    batch = 200
    for i in range(0, len(names), batch):
        chunk = names[i : i + batch]
        proc = subprocess.run(
            ["javap", "-c", "-p", "-cp", inner_jar, *chunk],
            check=True,
            capture_output=True,
            text=True,
            cwd=work,
        )
        current: str | None = None
        for ln in proc.stdout.splitlines():
            if ln and not ln[0].isspace() and not ln.startswith("Compiled from") and not ln.startswith("}"):
                h = CLASS_HEAD_RE.match(ln)
                if h:
                    current = h.group(1)
                    out.setdefault(current, [])
                continue
            m = INVOKE_RE.search(ln)
            if m and current is not None:
                recv = m.group(1).replace("/", ".")
                out[current].append((recv, m.group(2), m.group(3)))
    missing = [n for n in names if n not in out]
    if missing:
        tops.fail(f"javap printed no class for {len(missing)} name(s): {missing[:5]}")
    return out


def generate(work: pathlib.Path) -> tuple[str, str]:
    version, jar, mappings, classpath, maps = tops.prepare(work)
    inner_jar = classpath.split(":")[0]
    args = []
    args += light.resolve(maps, tops.CLASS_SHARED, methods=["tryDetectVersion"])
    args += light.resolve(maps, tops.CLASS_BOOTSTRAP, methods=["bootStrap"])
    args += light.resolve(maps, tops.CLASS_BLOCK, fields=["BLOCK_STATE_REGISTRY"], methods=["isFaceFull"])
    args += light.resolve(
        maps,
        tops.CLASS_STATE_BASE,
        methods=[
            "getCollisionShape",
            "isRedstoneConductor",
            "isCollisionShapeFullBlock",
            "isSignalSource",
            "getBlock",
        ],
    )
    args += light.resolve(maps, tops.CLASS_EMPTY_GETTER, fields=["INSTANCE"])
    args += light.resolve(maps, tops.CLASS_BLOCKPOS, fields=["ZERO"])
    args += light.resolve(maps, CLASS_DIRECTION, fields=["DOWN", "UP", "NORTH", "SOUTH", "WEST", "EAST"])
    args += light.resolve(maps, CLASS_BEHAVIOUR)
    args += light.resolve(maps, CLASS_ENTITY_BLOCK, methods=["newBlockEntity"])
    args += light.resolve(maps, CLASS_BLOCK_ENTITY)
    signal_getter = light.resolve(maps, CLASS_SIGNAL_GETTER)[0]
    wanted = signal_methods(mappings, maps)
    sys.stderr.write(f"  resolved from the pinned mappings: {' '.join(args)}\n")
    sys.stderr.write(f"  SignalGetter {signal_getter}: {sorted(wanted)}\n")

    classes = work / "classes-redstone"
    classes.mkdir(exist_ok=True)
    subprocess.run(["javac", "-nowarn", "-cp", classpath, "-d", str(classes), str(JAVA_SRC)], check=True)
    run_cp = f"{classpath}:{classes}"
    states_tsv = work / "redstone-states.tsv"
    blocks_tsv = work / "redstone-blocks.tsv"
    proc = subprocess.run(
        ["java", "-Xmx3g", "-cp", run_cp, "dw.RedstoneDump", "states", str(states_tsv), str(blocks_tsv), *args],
        check=True,
        capture_output=True,
        text=True,
        cwd=work,
    )
    counted = re.search(
        r"DUMPED states=(\d+) blocks=(\d+) conductors=(\d+) sources=(\d+) entity-blocks=(\d+)"
        r" entity-blocks-made-none=(\d+)",
        proc.stdout,
    )
    if not counted or any(int(counted.group(i)) == 0 for i in range(1, 6)):
        tops.fail(f"the dumper reported nothing to count: {proc.stdout.strip()!r}")
    sys.stderr.write(f"  dumper: {counted.group(0)}\n")

    # Per block: the conductor class and the source flag, over every state.
    per = tops.read_states(states_tsv, 4)
    covered = sum(len(v) for v in per.values())
    if covered != int(counted.group(1)):
        tops.fail(f"read {covered} states, the dumper produced {counted.group(1)}")
    conductor: dict[str, str] = {}
    source: dict[str, bool] = {}
    for block, states in per.items():
        vals = [tuple(x == "true" for x in v) for _, v in states]
        for c, f, faces, _ in vals:
            if f != faces:
                tops.fail(
                    f"{block}: isCollisionShapeFullBlock ({f}) disagrees with six full faces ({faces}) —"
                    " the engine's reading of `shape` would not be the game's"
                )
        if all(c == f for c, f, _, _ in vals):
            conductor[block] = "shape"
        elif all(c for c, _, _, _ in vals):
            conductor[block] = "always"
        elif not any(c for c, _, _, _ in vals):
            conductor[block] = "never"
        else:
            tops.fail(f"{block}: conducts on some states and neither always, never nor by its shape")
        source[block] = any(s for _, _, _, s in vals)

    # Per block: the classes whose bytecode is read.
    chains: dict[str, list[str]] = {}
    made_none: list[str] = []
    for ln in blocks_tsv.read_text(encoding="utf8").splitlines():
        state, bchain, bechain = ln.split("\t")
        m = tops.STATE_RE.match(state)
        if not m:
            tops.fail(f"unparsable blockstate from the dumper: {state!r}")
        if bechain == "!":
            made_none.append(m.group(1))
        names = bchain.split(",") + ([] if bechain in ("-", "!") else bechain.split(","))
        chains[m.group(1)] = names
    if set(chains) != set(per):
        tops.fail("the per-block and per-state dumps name different blocks")
    with zipfile.ZipFile(inner_jar) as z:
        entries = [n[: -len(".class")].replace("/", ".") for n in z.namelist() if n.endswith(".class")]
    nested = collections.defaultdict(list)
    for e in entries:
        if "$" in e:
            nested[e.split("$", 1)[0]].append(e)
    closure: dict[str, list[str]] = {}
    for block, names in chains.items():
        full: list[str] = []
        for n in names:
            full.append(n)
            full.extend(sorted(nested.get(n, [])))
        closure[block] = full
    all_classes = sorted({c for v in closure.values() for c in v})
    invokes = javap_classes(inner_jar, all_classes, work)
    names_hit = {(n, d) for v in invokes.values() for (_, n, d) in v} & wanted
    if not names_hit:
        tops.fail("no class invokes any SignalGetter method — the bytecode read found nothing")
    candidates = sorted({r for v in invokes.values() for (r, n, d) in v if (n, d) in wanted})
    cand_in = work / "redstone-receivers.txt"
    cand_out = work / "redstone-receivers-assignable.tsv"
    cand_in.write_text("\n".join(candidates) + "\n", encoding="utf8")
    proc = subprocess.run(
        ["java", "-cp", run_cp, "dw.RedstoneDump", "assignable", signal_getter, str(cand_in), str(cand_out)],
        check=True,
        capture_output=True,
        text=True,
        cwd=work,
    )
    assigned = re.search(r"ASSIGNABLE names=(\d+) assignable=(\d+)", proc.stdout)
    if not assigned or int(assigned.group(2)) == 0:
        tops.fail(f"no receiver is assignable to SignalGetter: {proc.stdout.strip()!r}")
    sys.stderr.write(f"  receivers: {assigned.group(0)}\n")
    is_getter = {
        n: v == "true" for n, v in (ln.split("\t") for ln in cand_out.read_text(encoding="utf8").splitlines())
    }
    class_reads = {
        c: any((n, d) in wanted and is_getter.get(r, False) for (r, n, d) in invokes[c]) for c in all_classes
    }
    reads = {b: any(class_reads[c] for c in cs) for b, cs in closure.items()}

    rows = [
        (f"minecraft:{b}", conductor[b], source[b], reads[b])
        for b in sorted(per)
    ]
    n_reads = sum(1 for r in rows if r[3])
    n_src = sum(1 for r in rows if r[2])
    jar_sha = hashlib.sha256(jar.read_bytes()).hexdigest()
    map_sha = hashlib.sha1(mappings.read_bytes()).hexdigest()
    head = [
        f"# Redstone facts of every block of Minecraft Java {version}.",
        "#",
        "# MEASURED, not written (spec-0100 §4.2), one row per block id:",
        "#   conductor     BlockState.isRedstoneConductor(EmptyBlockGetter.INSTANCE, BlockPos.ZERO)",
        "#                 over every state, set against isCollisionShapeFullBlock (the game's",
        "#                 default): `shape` when they agree on every state, else `always`/`never`.",
        "#                 isCollisionShapeFullBlock equals \"all six collision faces full\" on every",
        "#                 state (asserted by the dumper), so `shape` is read from the face table.",
        "#   signal_source BlockState.isSignalSource(), true when any state is one.",
        "#   reads_signal  a method of the block class or an ancestor below BlockBehaviour, of its",
        "#                 block-entity class or an ancestor below BlockEntity, or of a class nested",
        "#                 in any of them, invokes a SignalGetter method (getSignal, getDirectSignal,",
        "#                 getDirectSignalTo, getControlInputSignal, hasSignal, hasNeighborSignal,",
        "#                 getBestNeighborSignal) on a receiver assignable to SignalGetter (javap -c -p).",
        f"#   server jar sha256 {jar_sha}",
        f"#   server mappings sha1 {map_sha}",
        "# Regenerate with tools/maintenance/dump-redstone.py; --check diffs against this file.",
        "#",
        f"# {len(rows)} blocks over {covered} blockstates; {n_src} signal sources; {n_reads} read a signal;",
        f"# {len(all_classes)} classes read by javap. EntityBlocks whose newBlockEntity makes none,",
        f"# so only their block classes are read: {', '.join(sorted(made_none)) or 'none'}.",
        "#",
        "# block<TAB>conductor<TAB>signal_source<TAB>reads_signal",
    ]
    body = "\n".join(f"{n}\t{c}\t{str(s).lower()}\t{str(r).lower()}" for n, c, s, r in rows)
    return version, "\n".join(head) + "\n" + body + "\n"


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--check", action="store_true", help="diff against the committed table")
    ap.add_argument("--work", help="cache directory for the jar and mappings")
    args = ap.parse_args()
    work = (
        pathlib.Path(args.work).resolve()
        if args.work
        else pathlib.Path(tempfile.mkdtemp(prefix="dwredstone-"))
    )
    version, text = generate(work)
    target = DATA_DIR / f"redstone-{version}.tsv"
    if args.check:
        if not target.exists():
            tops.fail(f"{target} does not exist")
        if target.read_text(encoding="utf8") == text:
            print(f"dump-redstone: {target.relative_to(ROOT)} matches the pinned jar")
            return
        sys.exit(f"dump-redstone: {target.relative_to(ROOT)} DISAGREES with the pinned jar")
    target.write_text(text, encoding="utf8")
    print(f"dump-redstone: wrote {target.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
