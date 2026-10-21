#!/usr/bin/env python3
"""Nothing in this repo may write a prefab `.nbt` without judging its block states.

The rule itself lives elsewhere
(`gallery/prefab-generator/src/prefab_invariants/invariants.rs::assert_blocks_are_real`
for the gallery's generator, `BlockRegistry::validate` inside the engine's workspace).
This file answers a different question: **which sites is it obliged at?**

That set was enumerated by hand once, and the hand missed one. Five tileset
generators got the guard; `crates/compiler/examples/gen_hello_room.rs` — a
sixth emitter, hand-building its own palette in a directory reserved for
non-production code — did not, and so nothing judged the states it wrote. A
seventh would have arrived the same way, because nothing anywhere asked what
the set was.

So the set is discovered, not listed. Three checks:

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

2. **Every emitter derives its connection states.** Judging the palette proves
   the ids and values exist; it says nothing about whether a `multipart`
   property was written at all, or written at the block's default. Both spellings
   validate, and the default of every connection property is *disconnected* — so
   completing a state from `BlockRegistry::default_state` ships the isolated post
   the author never meant AND empties the `DW0735` predicate, which is the check
   going green by ceasing to bind. The obligation is therefore the derivation
   (`gallery/prefab-generator/src/prefab_invariants/connections.rs`, computing each property from the blocks beside the
   cell), and an emitter whose palette carries no connection class says so in
   [`NOT_CONNECTION_EMITTERS`] with its reason.

3. **Every generator workspace is built, linted, tested and formatted.** The
   other way a new emitter arrives is a new generator outside the root
   workspace, which `cargo …--workspace` never reaches. The population is read
   off the disk (a tracked manifest with its own `[workspace]` table and a
   `src/main.rs` that serialises NBT), and each wiring it owes — clippy, test,
   and the derived `cargo fmt` sweep — is looked for by name in what `ci.yml`
   runs. Adding a generator without wiring it up is an ordinary red.

Exit 0 clean, 1 with findings. All three checks print their binding count: a
check that matched nothing is a finding, not a pass (CLAUDE.md).
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from lib.generator_workspaces import fmt_excluded_prefix, generator_workspaces  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]

# The one way this repo turns a value into NBT bytes. Everything that writes a
# structure template goes through it, in every workspace.
INGREDIENT = "fastnbt::to_bytes"

# How a file shows it applied the block-state rule to the palette it authored:
# every id and every property value is judged against the pinned registry before
# the bytes are written. Two spellings of one rule — the one the generators share
# through `prefab_invariants`, and the registry method used inside the workspace.
BLOCK_RULE_MARKERS = (
    "invariants::assert_blocks_are_real(",
    "registry.validate(",
    "BlockRegistry::validate",
    # The grammar export's own spelling gate, which judges every palette state
    # of an expansion against the pinned registry before a byte is written.
    "refuse_unknown_states(",
)

# How a file shows it judged the ENTITIES it writes (spec-0111 §3.1): a piece
# carries item holders saying what they look like, and nothing else. Two
# spellings of one rule — the engine's entity judge, and the generators' own
# invariant over the same holder table (`delvewright_dsl::holders`).
ENTITY_RULE_MARKERS = (
    "entities::judge(",
    "invariants::assert_entities_are_holders(",
)

# An `entities` field initialised to anything but an empty list: the emitter
# writes entities. A type (`entities: Vec<…>`, `&[…]`) is not an initialiser.
# The same, spelled as a dynamic NBT key: `"entities"` inserted with anything
# but an empty list.
WRITES_ENTITIES_KEY = re.compile(r'"entities"(?!\.to_string\(\),\s*(?:\w+::)*List\(vec!\[\]\))')
WRITES_ENTITIES = re.compile(r"\bentities:\s*(?!Vec::new\(\)|vec!\[\]|Vec<|&|Option<)\S")

# How a file shows it derived its connection states rather than leaving them to
# the default that means disconnected: the piece goes through the one authority
# that computes each `multipart` property from the blocks beside the cell.
CONNECTION_RULE_MARKERS = ("connections::resolve(",)

# Emitters whose palette carries no connection class, and so owe no derivation.
# Named individually and printed on every run, for the same reason NOT_EMITTERS
# is: a class exemption is what hid the sixth emitter.
NOT_CONNECTION_EMITTERS = {
    "crates/delvec/src/grammar/export.rs": (
        "the grammar export. It derives no connection because it accepts none it would have to: "
        "`refuse_unknown_states` refuses, before a byte is written, every palette state that "
        "omits a shape-carrying property (`ExportError::ShapeOmissions`, the `DW0735` rule at the "
        "writer), so each connection a program's walls, fences and panes carry is one the program "
        "wrote. It entered this check by writing a template's `entities` (spec-0111 §3.2) through "
        "`fastnbt::to_bytes`; its blocks still go through `convert::build_region`. The derivation "
        "lives in the gallery's generator (`gallery/prefab-generator`, its own workspace), which `delvec` does not depend "
        "on."
    ),
    "crates/delvec/src/compiler/surround.rs": (
        "the horizon surround generator, and the only NON-fixture entry here. Its vocabulary is "
        "rock, ground, logs, leaves and ground cover, and not one of those carries a "
        "shape-carrying property — so there is nothing beside a cell for a connection to be "
        "derived FROM. What makes that an exemption rather than a claim is that the emitter "
        "asserts it: `assert_palette_is_real` runs `BlockRegistry::omitted_shape_carrying` over "
        "the real palette of every tile it writes, and a block that DID carry a connection class "
        "could not pass that line. The assertion is not decorative — it fired on first run, on "
        "`minecraft:pink_petals`, which is multipart in `flower_amount`/`facing` and was being "
        "written bare, so vanilla would have filled a cherry valley floor with one north-facing "
        "petal per cell. Those two are an authored decision rather than something the neighbours "
        "imply (the same reason `gallery/prefab-generator/src/prefab_invariants/connections.rs` derives a fence but not a fence gate), "
        "so they are now authored, and the surround owes `connections::resolve` nothing. This "
        "crate could not call it in any case: the derivation lives in the gallery's generator "
        "(`gallery/prefab-generator`, its own workspace), and `delvec` does not depend on it."
    ),
    "crates/delvec/tests/staged_lethal.rs": (
        "test fixture for spec-0088's staged lethal volume: the lid room. Its palette is four "
        "plain ids — stone, magma block, glowstone and the room's air — judged against the "
        "pinned registry before the bytes are written, none of them a fence, wall, pane or "
        "multiface block, and the room is written into the test target's scratch directory and "
        "never admitted into the library."
    ),
    "crates/delvec/tests/edit.rs": (
        "test fixture for the edit-stage determinism gate. Its palette is four states — air, "
        "stone, a lantern and an oak log — none of them a fence, wall, pane or multiface block, "
        "and the box it frames is never admitted into the library."
    ),
}

# Sites that serialise NBT and are NOT prefab emitters. Each is named
# individually, with the reason it is not obliged to judge a palette — and
# every one is printed on every run, because an exemption nobody sees is how a
# convenient exemption becomes a habit.
NOT_EMITTERS = {
    "crates/delvec/src/admit/structure.rs": (
        "rewriter, not an emitter: it re-serialises the palette it read out of an existing "
        "template rather than authoring one. Judging here would hold a PRE-PIN piece to a "
        "registry that describes 1.21.11 alone and has no authority over it — the same "
        "scoping DW0734 already states."
    ),
    "crates/delvec/src/schem/convert.rs": (
        "converter, not an emitter: `build_region` re-serialises the states it read out of an "
        "input `.schem`, so the palette is the input's and not this file's. The id verdict "
        "belongs to whoever admits the result — `delvec prefab audit` (DW0733/DW0734) for an "
        "imported piece, `refuse_unknown_states` for a grammar expansion — and both judge "
        "against the DataVersion the piece declares, which this function does not know."
    ),
    "crates/delvec/src/schem/fixtures.rs": (
        "input fixtures: builds Sponge `.schem` bytes for the round-trip tests, deliberately "
        "including under-specified states, which are the red half of the emitter's own proof."
    ),
    "crates/delvec/src/compiler/assembled.rs": (
        "`#[cfg(test)]` fixture: hand-frames a template to feed `structure_cells`. Emits "
        "nothing; reads its own bytes back in the same function."
    ),
    "crates/delvec/tests/boundary_assembled.rs": (
        "test fixture for the assembled-world model. The plain stone box it used to frame here "
        "moved to `common::box_nbt` when a second file needed it; what remains is this file's "
        "own — a room with a doorway in one wall and an optional walled porch outside it, the "
        "shape the door-onto-the-void case turns on. Its palette is the same three literal ids "
        "in the same function, and the bytes go straight to `emit::build` in memory."
    ),
    "crates/delvec/tests/common/mod.rs": (
        "the shared test fixture module, and the only place these tests frame a template. Two "
        "synthesisers live here: the tiled-placement proof's sealed corridor past the "
        "48-per-axis cap, written as two tiles plus a manifest into a per-test temp dir, and "
        "`box_nbt`, the stone box the boundary and piece-exposure proofs both build their world "
        "out of. Between them the palette is three literal ids named in the same functions "
        "(`minecraft:air`, `minecraft:stone`, `minecraft:glowstone`), and nothing writes either "
        "into a prefab library — the bytes exist to be read back by the same test that asked for "
        "them. `box_nbt` arrived here because it was COPIED into a second test file, and this "
        "check caught the copy as a new unjudged site: the repair was to share the one function "
        "rather than to write a second exemption, so this list got shorter by one entry "
        "(`crates/delvec/tests/boundary_assembled.rs`) instead of longer by one."
    ),
    "crates/delvec/tests/emit.rs": "test fixture for the emitted datapack.",
    "crates/delvec/tests/lava_floor.rs": (
        "test fixture for the fluid-occupancy proof: hand-frames a synthetic room whose floor "
        "course is the variable. Its palette is deliberately minimal and is never admitted."
    ),
    "crates/delvec/tests/relight.rs": "test fixture for the spec-0010 relight.",
    "docs/experiments/m2-jigsaw-seed-stability/generator/src/main.rs": (
        "frozen experiment record — the script exactly as it was run, beside the evidence it "
        "produced. Rewriting it would make the recorded result irreproducible from the "
        "recorded method (same rule as `tools/ci/check-live-commands.py`)."
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


def check_entities(emitters: list[str]) -> tuple[list[str], int]:
    """Every emitter that writes entities judges them (spec-0111 §3.1).

    A template's `entities` list is placed by `/place template` as written, so
    what an emitter puts there is what the server summons. The engine strips
    and re-summons every holder it knows, but the obligation is at the writer:
    an emitter that writes an entity names the rule that judged it. Which
    emitters write entities is DERIVED from the text — an `entities` field
    initialised to anything but an empty list — and printed, never listed.
    """
    findings: list[str] = []
    writers: list[str] = []
    judged: list[str] = []
    for rel in emitters:
        scope = emitter_scope(rel, (ROOT / rel).read_text(errors="replace"))
        writes = any(WRITES_ENTITIES.search(s) for s in scope) or any(
            WRITES_ENTITIES_KEY.search(s) for s in scope
        )
        if not writes:
            continue
        writers.append(rel)
        if any(m in s for m in ENTITY_RULE_MARKERS for s in scope):
            judged.append(rel)
        else:
            findings.append(
                f"{rel} writes a template's `entities` and never judges them.\n"
                f"    Route every entity through the one entity rule — "
                f"`admit::entities::judge` inside the workspace, "
                f"`invariants::assert_entities_are_holders` in the gallery's generator — so "
                f"a piece carries item holders saying what they look like and nothing else "
                f"(DW0730 / DW1036)."
            )
    print(f"emitters examined for entities: {len(emitters)}")
    print(f"  write entities: {len(writers)}")
    print(f"  judge the entities they write: {len(judged)}")
    for f in sorted(judged):
        print(f"    {f}")
    if not emitters:
        findings.append("binding count is zero: no emitter reached the entity check.")
    return findings, len(writers)


def check_connections(emitters: list[str]) -> tuple[list[str], int]:
    """Every emitter derives its connection states, or says why it need not.

    Judging the palette is not enough. A `multipart` property is legal to omit
    and legal to write at the block's default, and both spellings pass
    `BlockRegistry::validate` — but the default of every connection property is
    *disconnected*, so an emitter that fills one from
    `BlockRegistry::default_state` ships the isolated post the author never meant
    and silences `DW0735` at the same time. The rule and its defeater sit in one
    impl block, seventy lines apart, and the defeater is the shorter call.

    So the obligation is the derivation, not the completion: a connection comes
    from the blocks beside the cell (`gallery/prefab-generator/src/prefab_invariants/connections.rs`, vanilla's own
    `connectsTo` / `attachsTo` / `canAttachTo`). This binds it to the event.
    Same polarity as the check above — an emitter is presumed to owe it, and an
    exception is named individually and printed on every run.
    """
    findings: list[str] = []
    derived: list[str] = []
    honoured: list[str] = []

    for rel in emitters:
        scope = emitter_scope(rel, (ROOT / rel).read_text(errors="replace"))
        if any(m in s for m in CONNECTION_RULE_MARKERS for s in scope):
            derived.append(rel)
        elif rel in NOT_CONNECTION_EMITTERS:
            honoured.append(rel)
        else:
            findings.append(
                f"{rel} authors a prefab palette and never derives a connection state.\n"
                f"    Route the piece through `connections::resolve` before it is serialised, "
                f"so every `multipart` property comes from the blocks beside the cell. Do NOT "
                f"fill them from `BlockRegistry::default_state` / `unwritten`: that writes the "
                f"disconnection nobody meant AND makes DW0735 bind to nothing, so the sweep "
                f"goes green over a library whose walls are isolated posts.\n"
                f"    If the palette genuinely has no connection class, add the file to "
                f"NOT_CONNECTION_EMITTERS in tools/ci/check-structure-emitters.py with the reason."
            )

    print(f"prefab emitters examined: {len(emitters)}")
    print(f"  derive their connection states: {len(derived)}")
    for f in sorted(derived):
        print(f"    {f}")
    print(f"  declared exceptions: {len(honoured)}")
    for f in sorted(honoured):
        print(f"    {f} — {NOT_CONNECTION_EMITTERS[f]}")

    for f in sorted(set(NOT_CONNECTION_EMITTERS) - set(honoured)):
        findings.append(
            f"{f} is listed in NOT_CONNECTION_EMITTERS but is no longer an emitter that skips "
            f"the connection rule. Drop the entry — an exemption that binds to nothing hides "
            f"the next one."
        )
    if not emitters:
        findings.append(
            "binding count is zero: no emitter reached this check, so nothing was asked to "
            "derive a connection. The check above found the emitters; if it found none, that "
            "is the finding."
        )
    return findings, len(emitters)


def check_emitters() -> tuple[list[str], int, list[str]]:
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
                f"rule — `invariants::assert_blocks_are_real` in the gallery's generator, "
                f"`BlockRegistry::validate` inside the workspace — or an id it writes "
                f"that the pinned version does not have loads as AIR, costing the piece "
                f"those cells with no error anywhere.\n"
                f"    If it is not an emitter, add it to NOT_EMITTERS in "
                f"tools/ci/check-structure-emitters.py with the reason."
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
    return findings, candidates, emitters


MOD_DECL = re.compile(r'^\s*(?:#\[path\s*=\s*"([^"]+)"\]\s*)?(?:pub\s+)?mod\s+(\w+)\s*;', re.M)


def emitter_scope(rel: str, text: str) -> list[str]:
    """The file, plus the module files IT declares.

    A generator may put its palette in one file and its emission in another
    (`main.rs` beside a `common.rs` it declares with `mod`), so the
    marker is looked for through the file's own `mod` declarations. Deliberately
    not "everything in the directory" and not "everything in the crate": a
    neighbour's judgement is not this file's, and widening the scope that far
    would let `crates/delvec/src/schem/fixtures.rs` inherit `convert.rs`'s guard.
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
    """Every generator workspace is one CI builds, lints, tests and formats.

    A generator outside the root workspace is reached by no `cargo …--workspace`
    the `rust` job runs, so each wiring it owes is looked for BY NAME, in what
    `ci.yml` RUNS (comment lines stripped — a comment that quotes a command
    would otherwise stand in for it):

      * `cargo clippy … --manifest-path <ws>/Cargo.toml … -D warnings`
      * `cargo test … --manifest-path <ws>/Cargo.toml`
      * `cargo fmt` — enumerated nowhere: `tools/ci/fmt-workspaces.sh` derives
        its population from `git ls-files`, so what is asked of it is whether
        the sweep is invoked, whether the manifest is inside the population
        (tracked), and whether the sweep's own exclusion prefix swallows it.

    Running the generator is the gallery build's own act
    (`tools/ci/gallery-prefabs.py`), and a generator that panics on an
    invariant fails that build; it is not asked here.
    """
    findings: list[str] = []
    workspaces = set(generator_workspaces(ROOT))
    ci = (ROOT / ".github/workflows/ci.yml").read_text()
    runnable = "\n".join(ln for ln in ci.splitlines() if not ln.lstrip().startswith("#"))
    joined = runnable.replace("\\\n", " ")

    def invoked(verb: str, manifest: str, extra: str = "") -> bool:
        return any(
            f"--manifest-path {manifest}" in line and (not extra or extra in line)
            for line in joined.splitlines()
            if f"cargo {verb}" in line
        )

    sweep_invoked = re.search(r"tools/ci/fmt-workspaces\.sh[^\n]*--check", runnable) is not None
    prefix = fmt_excluded_prefix(ROOT)
    tracked_manifests = set(tracked("Cargo.toml"))

    print(f"generator workspaces on disk: {len(workspaces)} ({', '.join(sorted(workspaces))})")
    for ws in sorted(workspaces):
        manifest = f"{ws}/Cargo.toml"
        wired = {
            "`cargo clippy … -D warnings`": invoked("clippy", manifest, "-D warnings"),
            "`cargo test`": invoked("test", manifest),
        }
        for what, ok in wired.items():
            if not ok:
                findings.append(
                    f"no ci.yml step runs {what} with `--manifest-path {manifest}`. The root "
                    f"workspace does not reach this generator, so nothing else does either."
                )
        if manifest not in tracked_manifests:
            findings.append(
                f"{manifest} is not tracked by git, so the derived fmt sweep — whose population IS "
                f"`git ls-files` — never sees it."
            )
        if prefix and manifest.startswith(prefix):
            findings.append(
                f"tools/ci/fmt-workspaces.sh's exclusion prefix '{prefix}' swallows {manifest}."
            )
        print(
            f"  {ws}: clippy {'yes' if wired['`cargo clippy … -D warnings`'] else 'NO'}, "
            f"test {'yes' if wired['`cargo test`'] else 'NO'}"
        )
    if not sweep_invoked:
        findings.append(
            "nothing in ci.yml invokes tools/ci/fmt-workspaces.sh --check, so no generator "
            "workspace is held to `cargo fmt`."
        )
    if prefix is None:
        findings.append(
            "tools/ci/fmt-workspaces.sh's EXCLUDED_PREFIX could not be read, so this check cannot "
            "say whether the derived fmt sweep still covers the generator workspaces. An "
            "unreadable exclusion is a finding, not an assumption."
        )
    print(f"  derived fmt sweep invoked by ci.yml: {'yes' if sweep_invoked else 'NO'}")
    if not workspaces:
        findings.append(
            "binding count is zero: no generator workspace found — the gallery's generator "
            "(`gallery/prefab-generator`) is one, so the discovery has gone stale"
        )
    return findings, len(workspaces)


def main() -> int:
    findings: list[str] = []
    print("== every site that serialises NBT judges its block states ==")
    f, _, emitters = check_emitters()
    findings += f
    print()
    print("== every emitter derives its connection states ==")
    f, _ = check_connections(emitters)
    findings += f
    print()
    print("== every emitter that writes entities judges them ==")
    f, _ = check_entities(emitters)
    findings += f
    print()
    print("== every generator workspace is wired into CI ==")
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
