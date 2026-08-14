#!/usr/bin/env python3
"""Nothing in this repo may write a prefab `.nbt` without judging its block states.

The rule itself lives elsewhere (`prefabs/invariants.rs::assert_blocks_are_real`
for the generator workspaces, `BlockRegistry::validate` inside the workspace).
This file answers a different question: **which sites is it obliged at?**

That set was enumerated by hand once, and the hand missed one. Five tileset
generators got the guard; `crates/compiler/examples/gen_hello_room.rs` — a
sixth emitter, hand-building its own palette in a directory reserved for
non-production code — did not, and so nothing judged the states it wrote. A
seventh would have arrived the same way, because nothing anywhere asked what
the set was.

So the set is discovered, not listed. Two checks:

1. **Every site that turns a value into NBT bytes is accounted for.** The
   discriminator is the ingredient, not a name: `fastnbt::to_bytes` is the only
   way anything here produces NBT, so every file that names it is a candidate.
   A candidate passes by naming the block-state rule — judging the palette it
   authored against the pinned registry is what an emitter owes. A candidate
   that is not an emitter must be listed in [`NOT_EMITTERS`] below with a
   reason, which is PRINTED on every run. A NEW file that serialises NBT and
   does neither is red, and the message says which of the two it needs.

   The exemptions are enumerated file by file, deliberately. A class exemption
   ("anything under `tests/` or `examples/` is a fixture") is exactly the
   assumption that hid the sixth emitter for the entire life of the file: it was
   production tooling living in `examples/`. The polarity is the point — a
   hand-written list of *inclusions* fails silently when it misses a site, a
   hand-written list of *exclusions* fails loudly.

2. **Every prefab generator workspace is a workspace CI runs.** The other way a
   new emitter arrives is a new `prefabs/<name>-generator/`. The
   `prefab-generators` job's lists are held equal to what is on disk, in both
   directions, so adding a generator without wiring it up is an ordinary red
   rather than a tileset nothing ever runs twice.

Exit 0 clean, 1 with findings. Both checks print their binding count: a check
that matched nothing is a finding, not a pass (CLAUDE.md).
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# The one way this repo turns a value into NBT bytes. Everything that writes a
# structure template goes through it, in every workspace.
INGREDIENT = "fastnbt::to_bytes"

# How a file shows it applied the block-state rule to the palette it authored:
# every id and every property value is judged against the pinned registry before
# the bytes are written. Two spellings of one rule — the source-included one the
# generator workspaces share, and the registry method used inside the workspace.
BLOCK_RULE_MARKERS = (
    "invariants::assert_blocks_are_real(",
    "registry.validate(",
    "BlockRegistry::validate",
)

# Sites that serialise NBT and are NOT prefab emitters. Each is named
# individually, with the reason it is not obliged to judge a palette — and
# every one is printed on every run, because an exemption nobody sees is how a
# convenient exemption becomes a habit.
NOT_EMITTERS = {
    "crates/admit/src/structure.rs": (
        "rewriter, not an emitter: it re-serialises the palette it read out of an existing "
        "template rather than authoring one. Judging here would hold a PRE-PIN piece to a "
        "registry that describes 1.21.11 alone and has no authority over it — the same "
        "scoping DW0734 already states."
    ),
    "crates/schem/src/convert.rs": (
        "converter, not an emitter: `build_region` re-serialises the states it read out of an "
        "input `.schem`, so the palette is the input's and not this file's. The id verdict "
        "belongs to whoever admits the result — `delve-admit audit` (DW0733/DW0734) for an "
        "imported piece, `refuse_unknown_states` for a grammar expansion — and both judge "
        "against the DataVersion the piece declares, which this function does not know."
    ),
    "crates/schem/src/fixtures.rs": (
        "input fixtures: builds Sponge `.schem` bytes for the round-trip tests, deliberately "
        "including under-specified states, which are the red half of the emitter's own proof."
    ),
    "crates/compiler/src/assembled.rs": (
        "`#[cfg(test)]` fixture: hand-frames a template to feed `structure_cells`. Emits "
        "nothing; reads its own bytes back in the same function."
    ),
    "crates/compiler/tests/boundary_assembled.rs": "test fixture for the assembled-world model.",
    "crates/compiler/tests/emit.rs": "test fixture for the emitted datapack.",
    "crates/compiler/tests/lava_floor.rs": (
        "test fixture for the fluid-occupancy proof: hand-frames a synthetic room whose floor "
        "course is the variable. Its palette is deliberately minimal and is never admitted."
    ),
    "crates/compiler/tests/relight.rs": "test fixture for the spec-0010 relight.",
    "docs/experiments/m2-jigsaw-seed-stability/generator/src/main.rs": (
        "frozen experiment record — the script exactly as it was run, beside the evidence it "
        "produced. Rewriting it would make the recorded result irreproducible from the "
        "recorded method (same rule as `tools/check-live-commands.py`)."
    ),
}


def tracked(suffix: str) -> list[str]:
    out = subprocess.run(
        ["git", "-C", str(ROOT), "ls-files"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.split("\n")
    return [f for f in out if f.endswith(suffix)]


def check_emitters() -> tuple[list[str], int]:
    """Every NBT-serialising file judges its palette, or says why it need not."""
    findings: list[str] = []
    candidates = 0
    emitters: list[str] = []
    honoured: list[str] = []

    for rel in tracked(".rs"):
        text = (ROOT / rel).read_text(errors="replace")
        if INGREDIENT not in text:
            continue
        candidates += 1
        scope = emitter_scope(rel, text)
        if any(m in s for m in BLOCK_RULE_MARKERS for s in scope):
            emitters.append(rel)
        elif rel in NOT_EMITTERS:
            honoured.append(rel)
        else:
            findings.append(
                f"{rel} serialises NBT ({INGREDIENT}) and never judges a block state.\n"
                f"    If it writes a prefab `.nbt`, route its palette through the block-state "
                f"rule — `invariants::assert_blocks_are_real` in a `prefabs/*` generator, "
                f"`BlockRegistry::validate` inside the workspace — or an id it writes "
                f"that the pinned version does not have loads as AIR, costing the piece "
                f"those cells with no error anywhere.\n"
                f"    If it is not an emitter, add it to NOT_EMITTERS in "
                f"tools/check-structure-emitters.py with the reason."
            )

    print(f"NBT-serialising files examined: {candidates}")
    print(f"  emitters (judge their palette): {len(emitters)}")
    for f in sorted(emitters):
        print(f"    {f}")
    print(f"  declared non-emitters: {len(honoured)}")
    for f in sorted(honoured):
        print(f"    {f} — {NOT_EMITTERS[f]}")

    stale = sorted(set(NOT_EMITTERS) - set(honoured) - set(findings))
    for f in stale:
        findings.append(
            f"{f} is listed in NOT_EMITTERS but no longer serialises NBT (or now judges). "
            f"Drop the entry — an exemption that binds to nothing hides the next one."
        )
    if candidates == 0:
        findings.append(
            f"binding count is zero: no tracked .rs file names {INGREDIENT}. Either the repo "
            f"stopped writing NBT or the ingredient changed name; this check is inert either way."
        )
    return findings, candidates


MOD_DECL = re.compile(r'^\s*(?:#\[path\s*=\s*"([^"]+)"\]\s*)?(?:pub\s+)?mod\s+(\w+)\s*;', re.M)


def emitter_scope(rel: str, text: str) -> list[str]:
    """The file, plus the module files IT declares.

    A generator may put its palette in one file and its emission in another
    (`prefabs/tidal-keep-generator` splits `main.rs` / `common.rs`), so the
    marker is looked for through the file's own `mod` declarations. Deliberately
    not "everything in the directory" and not "everything in the crate": a
    neighbour's judgement is not this file's, and widening the scope that far
    would let `crates/schem/src/fixtures.rs` inherit `convert.rs`'s guard.
    """
    scope = [text]
    here = (ROOT / rel).parent
    for path_attr, name in MOD_DECL.findall(text):
        for cand in ([here / path_attr] if path_attr else [here / f"{name}.rs", here / name / "mod.rs"]):
            if cand.is_file():
                scope.append(cand.read_text(errors="replace"))
                break
    return scope


def check_generators_are_wired() -> tuple[list[str], int]:
    """Every `prefabs/*` generator workspace is one the `prefab-generators` job runs."""
    findings: list[str] = []
    on_disk = {p.parent.name for p in sorted((ROOT / "prefabs").glob("*/Cargo.toml"))}

    ci = (ROOT / ".github/workflows/ci.yml").read_text()
    job = ci.split("prefab-generators:", 1)
    if len(job) != 2:
        return ["the `prefab-generators` job is gone from ci.yml — this check binds to nothing"], 0
    body = job[1].split("\n  harness:", 1)[0]

    # The cache list (`workspaces: | prefabs/<g>`) and every shell `for g in ...`
    # list in the job. All three must name exactly the workspaces on disk.
    cached = set(re.findall(r"^\s+prefabs/([\w.-]+)\s*$", body, re.M))
    lists = {"cache list": cached}
    for i, m in enumerate(re.finditer(r"for g in ([^;]+?); do", body, re.S)):
        names = set(re.findall(r"[\w.-]+", m.group(1).replace("\\\n", " ")))
        lists[f"`for g in` list #{i + 1}"] = names

    for label, names in lists.items():
        missing = sorted(on_disk - names)
        extra = sorted(names - on_disk)
        if missing:
            findings.append(
                f"the prefab-generators job's {label} does not name {', '.join(missing)}, "
                f"which is a generator workspace on disk. A generator CI does not run is a "
                f"tileset with no invariant gate and no ADR-0006 double-run."
            )
        if extra:
            findings.append(
                f"the prefab-generators job's {label} names {', '.join(extra)}, which is not a "
                f"generator workspace — the job will fail on a path that does not exist."
            )

    print(f"prefab generator workspaces on disk: {len(on_disk)} ({', '.join(sorted(on_disk))})")
    for label, names in lists.items():
        print(f"  {label}: {len(names)}")
    if not on_disk:
        findings.append("binding count is zero: no prefabs/*/Cargo.toml found")
    if len(lists) < 3:
        findings.append(
            f"expected the cache list and two `for g in` loops in the prefab-generators job, "
            f"found {len(lists)} list(s) — the job was restructured and this check no longer "
            f"reads it."
        )
    return findings, len(on_disk)


def main() -> int:
    findings: list[str] = []
    print("== every site that serialises NBT judges its block states ==")
    f, _ = check_emitters()
    findings += f
    print()
    print("== every prefab generator workspace is wired into CI ==")
    f, _ = check_generators_are_wired()
    findings += f

    if findings:
        print()
        print(f"FAIL: {len(findings)} finding(s)")
        for x in findings:
            print(f"  - {x}")
        return 1
    print()
    print("OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
