#!/usr/bin/env python3
"""A capability must belong to the object class it acts on, not to the verb that
first needed it.

## The defect this exists to end

`close-gate` owns a field `sealed_hint` and the compiler emits, for that one verb,
its own fleet of `minecraft:interaction` bodies plus its own actionbar reply and
its own baked English default. The capability it encodes — *answer a player who
presses this thing* — has nothing to do with closing a gate. The DSL already
exposes that capability in full and generally: `EnvTrigger {at, on: use, effects}`
runs the whole effect vocabulary on a right-click.

So the seal is not a capability keyed to the wrong parent. It is a **private
re-implementation, inside one verb, of a general construct** — which is worse,
because the special case works perfectly and therefore nothing ever looks at it,
while every proof, l10n pass and diagnostic written for the general path has to be
taught about the private one separately or silently not cover it.

Three shapes, all of them this class (`docs/notes/capability-ownership-audit.md`):

  1. keyed to the verb, not the object class — the second consumer has no surface,
     and the fix *looks like* a second bespoke field on the second parent;
  2. re-implemented privately inside a verb — two mechanisms for one behaviour
     drift apart, and the private one sits outside every total check;
  3. the general mechanism exists but binds too narrowly to reach the objects it
     should — this one looks exactly like a missing feature, and the "fix" adds a
     fourth mechanism. `EnvTrigger` binds a body to a POINT at a cell; a seal, a
     door and a boulder are VOLUMES. That, and nothing else, is why `close-gate`
     grew its own per-shell-cell fleet.

The repo has already paid for this class twice under other names: the hand-rolled
effect-root walks (PRs #301/#302/#321, `check-effect-roots.py`), and the legacy
`traps[].effect` that spec-0022 superseded with a general `traps[].payload`. The
second of those is the proof the fix is possible — a verb's private machinery was
replaced by the general effect vocabulary, and the verb got *more* expressive.

## What it flags

Four checks, each a LEDGER: a known site must be listed with the reason it is not
the general construct. A new site that nobody has justified is a red. None of them
is a proof of absence; each is a tripwire on the shape the known instances had.

  A. interaction bodies — every `summon minecraft:interaction` in the compiler.
     Nine today; exactly ONE is `EnvTrigger`. Every other is a verb that grew its
     own answer to "the player clicked a thing".
  B. baked player-facing English — every compiler-owned default string a player
     can read. Each belongs to one verb and is unreachable from any other.
  C. structural twins — two DSL structs declared separately with an identical
     field set. `TrapDisarm` and `TimedGateDisarm` are the same object class typed
     out twice; the second's own doc comment says so.
  D. cross-cutting modifiers with holes — a field carried by a large minority of a
     tagged enum's variants but not all of them. `requires_flags` is documented as
     a per-effect flag gate and is absent from ten of twenty-six effects, so those
     ten cannot be branch-gated at all.

Check D distinguishes a *modifier* from a *payload* by prevalence: `anchor` rides
5/26 effects because only five act on an anchor, and that is correct; a field on a
third or more of the variants is cross-cutting, and its absence from the rest is a
hole rather than a design. `MODIFIER_MIN_SHARE` is that line.

## Known non-proof, stated rather than implied

A/B are text scans of the compiler; a private interaction body built through a
helper that hides the `summon`, or a default string assembled at runtime from
fragments, is invisible here. C/D parse `stages.rs` structurally but only see what
`pub` fields and variant blocks look like textually. This gate makes the KNOWN
shapes un-addable-in-silence. It does not certify that none remain.

## Binding count

Every run prints, per check, how many objects it examined and how many it matched.
**A check that examined zero objects, or matched zero, is a RED** — a green that
bound to nothing is vacuous, not a pass (CLAUDE.md). If a rename makes a pattern
stop matching, this gate would otherwise go quietly green forever.
"""

import collections
import pathlib
import re
import sys

# ---------------------------------------------------------------------------
# Check A — interaction bodies
# ---------------------------------------------------------------------------

# Keyed by the ENCLOSING FUNCTION, which survives the line moving. The value is
# the DSL construct that owns the body and why it is not an `EnvTrigger`.
INTERACTION_SITES = {
    "env_trigger_setup": (
        "THE GENERAL CONSTRUCT. `EnvTrigger {at, on: strike|use, effects}` — the "
        "DSL's first-class 'player clicked a thing here, run effects'. Every other "
        "entry in this ledger is a verb that did not use it."
    ),
    "npc_summon_commands": (
        "An NPC's dialogue hitbox. Legitimately its own: the box must track a "
        "character's body, not a cell, and it carries the NPC's identity. It is "
        "also the reason `strike-npc` exists as a separate `TriggerOn` variant — a "
        "point-bound trigger could not reach a large NPC's body (`DW0359`), so the "
        "general mechanism was taught to RIDE this one (`npc_hitbox_trigger_tags`) "
        "rather than widened to bind to a body. Shape 3."
    ),
    "seal_fns": (
        "CLOSED (DSL v0.11) — this was the audit's live instance, and it is worth "
        "keeping the shape written down. `close-gate.sealed_hint` owned the whole "
        "reply: this body fleet, its own `seal_<anchor>` advancement, its own "
        "`seal_hint_<anchor>` actionbar function and its own baked English, none of "
        "which anything else could reach — so the second object that needed a reply "
        "(a sealed shortcut door) had no surface and answered silence. The verb now "
        "owns only the BODY; the reply is an ordinary "
        "`EnvTrigger{on: use, audience: presser}` with a `narrate{style: actionbar}`, "
        "synthesized by `plan::collect_press_answers` for any sealed body the "
        "campaign leaves silent and emitted by `env_trigger_fns` like every other "
        "click. Both private functions are gone. What kept the fix from being a "
        "second bespoke field was widening the general verb instead: v0.11 gave "
        "`narrate` the actionbar channel and a trigger a presser audience, which "
        "were the only two things it could not say."
    ),
    "ws_arm_fns": (
        "The shortcut gate's approach-side bodies, and the SHAPE-3 LIFT this PR "
        "exists for. A trigger's `at` binds a POINT AT A CELL, not the clickable "
        "shape of the object standing at that anchor — so authoring the island "
        "boulder's own pattern on a shortcut door compiled clean and shipped a box "
        "pressable only from the side the door opens from. The fix is not a fourth "
        "mechanism: the body is arrayed over the gate's approach cells, and a click "
        "trigger the author anchors there RIDES these hitboxes (`seal_rider_tags`) "
        "instead of summoning a co-located one, exactly as `strike-npc` rides "
        "`npc_summon_commands`'s body. The compiler supplies the SHAPE; the campaign "
        "supplies the ANSWER, in the general effect vocabulary, so the reply is "
        "l10n-inventoried and flag-gated by construction. Empty for a campaign with "
        "no shortcut — byte-identical output. Shape 3, closed rather than catalogued. "
        "v0.11 closed the other half, and the owner's ruling decided WHO closes it: "
        "the answer is the general verb, and from 0.11.0 the CAMPAIGN must write "
        "it — an unanswered SEALED BODY is `DW0429`, door and `close-gate` wall "
        "alike, not a line the engine invents. A baked default would be the "
        "compiler making a design statement about this thing's tone and never "
        "disclosing it. Uniform over the class on purpose: two objects of one "
        "class with two defaulting policies would be shape S1 committed while "
        "closing it. The door is now an ordinary consumer of the mechanism rather "
        "than the one pressable object in the engine with nothing to say."
    ),
    "shortcut_setup": (
        "OPEN FINDING. `shortcuts[].unlock` summons its own body at the far-side "
        "anchor. `on_unlock` already uses the general effect vocabulary, so only "
        "the INTERACTION half is private: this is a trigger whose effects happen to "
        "clear a gate. Shape 2."
    ),
    "trap_setup": (
        "OPEN FINDING, two bodies in one function. `dw_trapfire_` detects the trap "
        "firing; `dw_trapdis_` is `traps[].disarm.via`, a private "
        "interaction-plus-set-flag that `EnvTrigger {on: use, effects: [set-flag]}` "
        "expresses exactly. Note `traps[].payload` (spec-0022) already replaced this "
        "trap's private CONSEQUENCE machinery with the general effect vocabulary — "
        "the disarm affordance is the half that was not lifted. Shape 2."
    ),
    "timed_gate_setup": (
        "OPEN FINDING. `timed_gates[].disarm.via` — a SECOND copy of the private "
        "machinery `trap_setup` already had, for a struct (`TimedGateDisarm`) that "
        "is a field-for-field twin of `TrapDisarm` (check C). The doc comment on it "
        "states the intent — 'one affordance grammar for every mechanism the party "
        "can switch off' — which is the general construct, described but not used. "
        "Shapes 1 and 2."
    ),
    "emit_quest_effect": (
        "OPEN FINDING. The `bonfire` rest affordance. Its body is private AND the "
        "two-option dialog behind it is a private re-implementation of the stage-6 "
        "dialogue machinery, with its own `prompt`/`rest_label`/`save_label` strings "
        "(check B) instead of `DialogueNode`/`DialogueOption`. Shape 2, twice."
    ),
    "shop_setup": (
        "OPEN FINDING, and the SAME finding as `emit_quest_effect`'s bonfire — "
        "recorded as a new instance rather than folded in, because it is the second "
        "consumer of one missing capability. A shop's body must open a DIALOG, and a "
        "dialog has to be shown to a named player. `EnvTrigger` is polled off the "
        "entity's own `interaction` NBT record, which names no player at all "
        "(`seal_hint_fns` documents why the record is observed and not consumed), so "
        "the general construct cannot express 'the player who pressed this' — every "
        "site in the engine that needs the presser goes through a "
        "`player_interacted_with_entity` advancement instead. The missing capability "
        "is therefore not on the shop: it is that `EnvTrigger` has no acting-player "
        "binding. Shape 3, catalogued rather than closed, because closing it is a "
        "change to the trigger's own contract and not to this feature."
    ),
    "emit_stake_functions": (
        "ACCEPTED — the general construct genuinely cannot bind here. A recovery "
        "stake's marker is summoned AT RUNTIME, at a position chosen at runtime from "
        "the compile-time placement table (or at the death point itself), once per "
        "death, per player. `EnvTrigger.at` binds a prefab ANCHOR — one cell, known "
        "at compile time, existing from world load — so there is no `EnvTrigger` a "
        "campaign could have written that would answer a click on a body that does "
        "not exist until somebody dies. The collection ANSWER is not private: the "
        "amount restored is the declared datum, the line said is the stake's own "
        "l10n-inventoried `collected_message`, and retirement goes through the one "
        "`retired_by` function `DW0421` polices. What is private is only the "
        "summoning, which is the half no declaration could have supplied."
    ),
    "activation_commands": (
        "OPEN FINDING. `Objective::Interact` summons its own affordance, and carries "
        "its own `missing_item_hint` for the refusal reply — a private narrate. The "
        "objective genuinely needs an activation-gated body; the REPLY does not have "
        "to be private. Shape 2."
    ),
}

INTERACTION_SUMMON = re.compile(r'"(?:execute [^"]*run )?summon minecraft:interaction ')
FN_DEF = re.compile(r"^\s*(?:pub(?:\(crate\))?\s+)?fn\s+([a-z0-9_]+)")
TEST_MOD = re.compile(r"^\s*mod tests\b")

# ---------------------------------------------------------------------------
# Check B — baked player-facing English
# ---------------------------------------------------------------------------

BAKED_STRINGS = {
    "BONFIRE_PROMPT_EN": (
        "OPEN FINDING. Title of the bonfire's private rest dialog (see check A, "
        "`emit_quest_effect`)."
    ),
    "BONFIRE_REST_LABEL_EN": (
        "OPEN FINDING. Button caption of the bonfire's private rest dialog. A "
        "stage-6 `DialogueOption.label` is the general form and is inventoried."
    ),
    "BONFIRE_SAVE_LABEL_EN": (
        "OPEN FINDING. Second button caption of the same private dialog."
    ),
}

BAKED_CONST = re.compile(
    r'^\s*(?:pub\s+)?const\s+([A-Z][A-Z0-9_]*(?:_EN|_DEFAULT|_MESSAGE|_LABEL|_HINT|_PROMPT))\s*:\s*&(?:\'static\s+)?str\s*=\s*"([^"]*)"'
)

# ---------------------------------------------------------------------------
# Checks C/D — the DSL surface
# ---------------------------------------------------------------------------

STAGES = "crates/dsl/src/stages.rs"

STRUCTURAL_TWINS = {
    ("TimedGateDisarm", "TrapDisarm"): (
        "OPEN FINDING. One object class — 'an affordance that switches a mechanism "
        "off' — declared twice, field for field. `TimedGateDisarm`'s own doc says "
        "it is 'the exact shape a trap's `TrapDisarm` takes, and deliberately so: "
        "one affordance grammar for every mechanism the party can switch off'. The "
        "grammar is stated; the TYPE is copied. Each also emits its own interaction "
        "body (check A)."
    ),
    ("AnchorSubject", "CameraTarget", "CameraWaypoint"): (
        "ACCEPTED. Three camera-geometry types that coincidentally share "
        "`{anchor, offset}`: a dolly waypoint, an aim target and its subject are "
        "different roles in the same shot and are kept distinct on purpose. No "
        "capability is stranded — nothing wants a 'generic anchor+offset'."
    ),
}

# A field on this share or more of a tagged enum's variants is a cross-cutting
# MODIFIER; below it, a payload that belongs to the few variants that carry it.
MODIFIER_MIN_SHARE = 1 / 3

# …and only for an enum with at least this many variants. Below six, a partial
# field is overwhelmingly an OPERAND rather than a modifier — `Objective.anchor`
# rides 3 of 5 because `kill` acts on a wave and `talk-to` on an NPC, which is
# correct — and flagging those would bury the real holes in noise.
MODIFIER_MIN_VARIANTS = 6

MODIFIER_HOLES = {
    ("QuestEffect", "requires_flags"): (
        "OPEN FINDING, and the one blocking today. Documented as 'per-effect flag "
        "gate' — a property of AN EFFECT — but absent from ten of twenty-six, so "
        "`spawn-actor`, `move-actor`, `unleash-actor`, `despawn-actor`, "
        "`spawn-npc`, `set-checkpoint`, `bonfire`, `begin-stealth` and `sequence` "
        "cannot be branch-gated at all. Every one of those is staging or souls "
        "vocabulary, which is exactly what the branch work (spec-0025) needs to "
        "gate per branch. `campaign-complete`'s absence is deliberate and "
        "documented (gating a campaign's own completion is a deadlock footgun)."
    ),
    ("QuestEffect", "forbids_flags"): (
        "OPEN FINDING. The dual of `requires_flags`, missing from the same ten "
        "variants for the same reason. Lifts with it, not separately."
    ),
    ("QuestEffect", "requires_state"): (
        "OPEN FINDING, inherited. The numeric third field of the SAME gate "
        "(spec-0031, DSL v0.10), placed on exactly the variants `requires_flags` "
        "and `forbids_flags` already ride and absent from exactly the same ten. "
        "That is deliberate rather than an oversight: a gate is one object, and "
        "giving its comparison a different carrier set than its flags would make "
        "'which verbs are gatable' two different answers, which is the very shape "
        "this check exists to catch. The hole IS the flag pair's hole; all three "
        "fields lift together, in one `dsl_version`, or none do. Do not close "
        "this entry by widening the numeric axis on its own."
    ),
    ("WorldEdit", "region"): (
        "ACCEPTED — operand, not modifier. `WorldEdit` splits cleanly into "
        "CELL-level ops (which take a `region`) and PIECE-level ops (which take a "
        "`prefab`). The seven without a `region` are the piece ops; asking "
        "`insert-piece` for a region would be meaningless. No capability is "
        "stranded."
    ),
    ("WorldEdit", "prefab"): (
        "ACCEPTED — the dual of the above. The six with a `prefab` are the piece "
        "ops; the cell ops operate on a region and have no piece to name."
    ),
    ("RegionShape", "of"): (
        "ACCEPTED — operand. `of` is the sub-shape a COMPOSITE shape wraps; a "
        "primitive shape (`box`, `surface-band`) has nothing to wrap."
    ),
}


# ---------------------------------------------------------------------------
# Check E — every effect bundle is reachable by some enumeration
# ---------------------------------------------------------------------------

# A `Vec<QuestEffect>` field is an effect bundle emission can lower. Each must be
# reachable by SOME total enumeration, or every walk that inherits the roots
# silently skips it: l10n never translates a `narrate` inside it, the flag model
# never sees a `set-flag`, the timeline never schedules it. That is the exact
# defect PRs #301/#302/#321 fixed thirteen times.
#
# Keyed by the line's field name plus the declaring context, because several
# share a name. Value: which enumeration reaches it.
EFFECT_BUNDLES = {
    "on_objective_complete": "ROOT R1 (`EffectRootKind::ObjectiveComplete`).",
    "on_complete": "ROOT R2 (`EffectRootKind::QuestComplete`).",
    "effects": (
        "ROOT R3 (`EffectRootKind::Trigger`) for `EnvTrigger.effects`; ROOT R8 "
        "(`EffectRootKind::ShopOffer`) for `ShopOffer.effects`; "
        "`SequenceStep.effects` is reached as a nested list of "
        "`QuestEffect::Sequence`."
    ),
    "payload": "ROOT R4 (`EffectRootKind::TrapPayload`).",
    "on_respawn": (
        "ROOT R5 (`EffectRootKind::DialogueRespawn`) for the dialogue form; the "
        "`QuestEffect::SetCheckpoint` form is a nested list."
    ),
    "telegraph": (
        "DESUGARED. `Ambush::to_trigger()` folds it into a real `EnvTrigger.effects` "
        "before anything walks, and `QuestsContent::all_triggers` is the single "
        "expansion authority — so it is reached as R3. This is the CORRECT pattern "
        "for adding a bundle without adding a root."
    ),
    "on_arrive": "NESTED (`move-npc` / `move-actor`), via `nested_effect_lists`.",
    "on_rest": "NESTED (`bonfire`), via `nested_effect_lists`.",
    "on_caught": "NESTED (`begin-stealth`), via `nested_effect_lists`.",
    "on_unlock": (
        "ROOT R6 (`EffectRootKind::ShortcutUnlock`) — CLOSED by spec-0031. It was "
        "the finding this check was written to surface: `shortcuts[].on_unlock` was "
        "lowered at emit time (`emit_shortcut_functions` → `emit_effect_bundle`) "
        "and was NOT an `EffectRootKind` variant and NOT in `nested_effect_lists`, "
        "so a `narrate` inside it was never l10n-inventoried, a `set-flag` inside "
        "it was invisible to the flag model and to `emit::declared_flags`, and a "
        "`sequence` inside it would have emitted a `function` call to a function "
        "nothing generated. Zero live campaign usage was the only reason it never "
        "shipped as a bug. Deliberately made a root rather than desugared the way "
        "`telegraph` is: an unlock is not a trigger — it is polled behind a "
        "once-only `#sc_<id>` sentinel and it clears the gate region — so "
        "desugaring it would have introduced a second detector for one event."
    ),
    "on_death": (
        "ROOT R7 (`EffectRootKind::OnDeath`) — the campaign-wide death beat "
        "(spec-0031). Added as a root on the day the surface was added, which is "
        "the point: 'the purse is dropped on death' is then ordinary content in a "
        "general mechanism rather than an engine feature."
    ),
}

EFFECT_BUNDLE_FIELD = re.compile(r"^\s*(?:pub )?([a-z0-9_]+):\s*(?:Vec<QuestEffect>|BTreeMap<ObjectiveId, Vec<QuestEffect>>)")


def check_effect_bundles(root):
    found, fails = {}, []
    for i, line in enumerate((root / STAGES).read_text(encoding="utf-8").split("\n")):
        m = EFFECT_BUNDLE_FIELD.match(line)
        if m:
            found.setdefault(m.group(1), f"{STAGES}:{i + 1}")
    for name, site in sorted(found.items()):
        if name not in EFFECT_BUNDLES:
            fails.append(
                f"FAIL: {site} — `{name}: Vec<QuestEffect>` is an effect bundle no "
                f"enumeration is known to reach.\n"
                f"      Make it an `EffectRootKind`, a `nested_effect_lists` entry, "
                f"or desugar it into an existing root the way `Ambush::to_trigger` "
                f"does. A bundle outside every enumeration is lowered by emission "
                f"and skipped by l10n, the flag model and the timeline — and it "
                f"will not be red when it stops being correct."
            )
    for name in sorted(set(EFFECT_BUNDLES) - set(found)):
        fails.append(
            f"FAIL: `{name}` is in EFFECT_BUNDLES but is no longer a "
            f"`Vec<QuestEffect>` field. Drop its entry — a stale exemption hides "
            f"the next one."
        )
    return len(found), len(found), fails


def repo_root() -> pathlib.Path:
    return pathlib.Path(__file__).resolve().parent.parent


def rust_sources(root: pathlib.Path):
    return sorted(
        p
        for p in (root / "crates").rglob("*.rs")
        if "/tests/" not in str(p) and "/target/" not in str(p)
    )


def check_interactions(root):
    """Every `summon minecraft:interaction`, keyed by enclosing function."""
    found, fails = collections.defaultdict(list), []
    examined = 0
    for path in rust_sources(root):
        examined += 1
        cur = None
        for i, line in enumerate(path.read_text(encoding="utf-8").split("\n")):
            # In-file unit tests build fixture command strings; they emit nothing.
            # Tests live at the bottom of a module by convention.
            if TEST_MOD.match(line):
                break
            m = FN_DEF.match(line)
            if m:
                cur = m.group(1)
            # `starts_with("summon minecraft:interaction ")` inspects a command
            # someone else emitted — it is a reader, not a second body.
            if INTERACTION_SUMMON.search(line) and "starts_with" not in line:
                found[cur].append(f"{path.relative_to(root)}:{i + 1}")
    for fn, sites in sorted(found.items()):
        if fn not in INTERACTION_SITES:
            fails.append(
                f"FAIL: {sites[0]} — `fn {fn}` summons a `minecraft:interaction` "
                f"body that no ledger entry justifies.\n"
                f"      A verb that grows its own answer to 'the player clicked a "
                f"thing' is a private re-implementation of `EnvTrigger`. Either "
                f"route it through the general construct, or add an entry to "
                f"INTERACTION_SITES saying why it cannot be."
            )
    stale = sorted(set(INTERACTION_SITES) - set(found))
    for fn in stale:
        fails.append(
            f"FAIL: `fn {fn}` is in INTERACTION_SITES but summons no interaction "
            f"body any more. Drop its entry — a stale exemption hides the next one."
        )
    return examined, sum(len(v) for v in found.values()), fails


def check_baked(root):
    found, fails = {}, []
    examined = 0
    for path in rust_sources(root):
        examined += 1
        for i, line in enumerate(path.read_text(encoding="utf-8").split("\n")):
            m = BAKED_CONST.match(line)
            if not m:
                continue
            name, val = m.group(1), m.group(2)
            # A DW code or an id token is not a sentence a player reads.
            if re.fullmatch(r"DW\d{4}|[a-z0-9_.:/-]*", val):
                continue
            found[name] = f"{path.relative_to(root)}:{i + 1}"
    for name, site in sorted(found.items()):
        if name not in BAKED_STRINGS:
            fails.append(
                f"FAIL: {site} — `{name}` bakes player-facing English into one "
                f"code path.\n"
                f"      A second verb that wants the same sentence cannot reach it, "
                f"and a baked default is not l10n-inventoried, so it ships in "
                f"English whatever the delve's language. Prefer the general "
                f"`narrate` path, or justify it in BAKED_STRINGS."
            )
    for name in sorted(set(BAKED_STRINGS) - set(found)):
        fails.append(
            f"FAIL: `{name}` is in BAKED_STRINGS but no longer exists. Drop its "
            f"entry — a stale exemption hides the next one."
        )
    return examined, len(found), fails


def parse_stages(root):
    """`{struct: [fields]}, {enum: {variant: [fields]}}` from the DSL surface."""
    lines = (root / STAGES).read_text(encoding="utf-8").split("\n")
    structs, enums = {}, {}
    i = 0
    while i < len(lines):
        m = re.match(r"pub (struct|enum) ([A-Za-z0-9_]+)", lines[i])
        if not m:
            i += 1
            continue
        kind, name = m.groups()
        depth, body, j = 0, [], i
        while j < len(lines):
            depth += lines[j].count("{") - lines[j].count("}")
            body.append(lines[j])
            j += 1
            if depth == 0 and "{" in "\n".join(body):
                break
        txt = "\n".join(body)
        if kind == "struct":
            structs[name] = sorted(re.findall(r"^\s{4}pub ([a-z0-9_]+):", txt, re.M))
        else:
            variants = {}
            for v in re.findall(r"^\s{4}([A-Z][A-Za-z0-9]*)\s*\{", txt, re.M):
                vm = re.search(
                    r"^\s{4}" + v + r"\s*\{(.*?)^\s{4}\}", txt, re.M | re.S
                )
                if vm:
                    variants[v] = sorted(
                        set(re.findall(r"^\s{8}([a-z0-9_]+):", vm.group(1), re.M))
                    )
            if variants:
                enums[name] = variants
        i = j
    return structs, enums


def check_twins(structs):
    by_set, fails = collections.defaultdict(list), []
    for name, fields in structs.items():
        if fields:
            by_set[tuple(fields)].append(name)
    groups = {tuple(sorted(v)) for v in by_set.values() if len(v) > 1}
    for g in sorted(groups):
        if g not in STRUCTURAL_TWINS:
            fails.append(
                f"FAIL: {', '.join(g)} are declared separately with an identical "
                f"field set.\n"
                f"      That is one object class typed out N times; the capability "
                f"belongs to the shared class. Share the type, or justify the "
                f"split in STRUCTURAL_TWINS."
            )
    for g in sorted(set(STRUCTURAL_TWINS) - groups):
        fails.append(
            f"FAIL: {', '.join(g)} no longer share a field set. Drop the "
            f"STRUCTURAL_TWINS entry — a stale exemption hides the next one."
        )
    return len(structs), len(groups), fails


def check_modifier_holes(enums):
    fails, holes = [], set()
    pairs = 0
    for ename, variants in enums.items():
        n = len(variants)
        if n < MODIFIER_MIN_VARIANTS:
            continue
        counts = collections.Counter()
        for fields in variants.values():
            counts.update(fields)
        for field, c in counts.items():
            pairs += 1
            if c == n or c / n < MODIFIER_MIN_SHARE:
                continue
            holes.add((ename, field))
            if (ename, field) not in MODIFIER_HOLES:
                missing = sorted(v for v in variants if field not in variants[v])
                fails.append(
                    f"FAIL: `{ename}.{field}` is on {c}/{n} variants — "
                    f"cross-cutting, so it is a property of the enum, not of those "
                    f"variants. Missing from: {', '.join(missing)}.\n"
                    f"      Those variants cannot express the capability at all. "
                    f"Lift it to the enum, or justify each hole in MODIFIER_HOLES."
                )
    for k in sorted(set(MODIFIER_HOLES) - holes):
        fails.append(
            f"FAIL: `{k[0]}.{k[1]}` is in MODIFIER_HOLES but is no longer a "
            f"partial modifier. Drop its entry — a stale exemption hides the next."
        )
    return pairs, len(holes), fails


def main() -> int:
    root = repo_root()
    if not (root / STAGES).exists():
        print(f"FAIL: {STAGES} not found — the DSL surface moved.")
        return 1

    structs, enums = parse_stages(root)
    results = []
    results.append(("A interaction bodies", "files", *check_interactions(root)))
    results.append(("B baked player strings", "files", *check_baked(root)))
    results.append(("C structural twins", "DSL structs", *check_twins(structs)))
    results.append(
        ("D modifier holes", "(enum, field) pairs", *check_modifier_holes(enums))
    )
    results.append(
        ("E effect bundles", "Vec<QuestEffect> fields", *check_effect_bundles(root))
    )

    failed = False
    for name, unit, examined, matched, fails in results:
        # A check that bound to nothing is vacuous, not a pass.
        if examined == 0 or matched == 0:
            print(
                f"FAIL: check {name} examined {examined} {unit} and matched "
                f"{matched}. A gate that binds to nothing is vacuous, not a pass — "
                f"its patterns have stopped matching and it is now blind."
            )
            failed = True
            continue
        for f in fails:
            print(f)
        failed = failed or bool(fails)
        print(
            f"{'FAIL' if fails else 'OK'}: check {name} — {examined} {unit} "
            f"examined, {matched} matched, {len(fails)} unjustified."
        )

    if failed:
        print("\nThe audit and the three shapes: docs/notes/capability-ownership-audit.md")
        return 1
    print(
        "\nOK: every capability site is ledgered. The ledger is not a clean bill — "
        "most entries are OPEN FINDINGS with a named lift; it only means none of "
        "them can be added or removed in silence."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
