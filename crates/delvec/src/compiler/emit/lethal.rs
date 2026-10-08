//! Lethal volumes (spec-0031).

use super::*;

/// Entity types the **engine itself** places as machinery, never as bodies a
/// player fights or talks to: the `minecraft:interaction` hitboxes behind every
/// affordance, the cutscene camera and its marks, the display entities behind an
/// art title. A lethal volume must not delete them — a volume drawn across a
/// cutscene dolly would otherwise erase the camera mid-shot, and one over a gate
/// seal would erase the thing the player presses.
///
/// The list is exactly the set the compiler `summon`s as machinery, and it is a
/// list of **types** rather than of tags on purpose: a vanilla selector cannot
/// match a tag prefix, and every alternative — one negated tag per feature — grows
/// with the engine and is silently wrong the day a feature is added. Content
/// bodies (a wave mob, an actor puppet, an NPC) are deliberately NOT here: a mob
/// that walks into the lava dies, which is the mechanism working. An NPC posted
/// inside a volume is a content defect the campaign's own placement proofs and
/// `DW0511` are there to surface, not something to hide by exempting NPCs.
///
/// **It is no longer the whole answer, and it is kept anyway.** The engine's own
/// places now declare a class ([`crate::compiler::affordance::FIXTURE_TAG`]) and the
/// volume's selector negates it like every other box-narrowed entity selector, so
/// the sentence above — "one negated tag per feature" — is answered: it is one
/// negated tag for the whole engine, forever, because the class is decided at the
/// object rather than per feature. This roster stays because it is not the same
/// claim. It says *do not aim `/damage` at a thing that cannot take it*, which is
/// still true of a `block_display` that no fixture happens to be today; the class
/// says *do not disturb a place*. Deleting the roster would trade a live
/// statement for a shorter line.
pub(super) const LETHAL_EXEMPT_TYPES: [&str; 5] = [
    "minecraft:interaction",
    "minecraft:marker",
    "minecraft:item_display",
    "minecraft:block_display",
    "minecraft:text_display",
];

/// Scratch score holding the struck player's health **after** a lethal volume's
/// blow — the world state the volume's wording asserts, read back rather than
/// predicted.
///
/// **Measured, after getting it wrong once.** Vanilla refuses damage far more
/// often than "the target is dead" suggests: a player is invulnerable for **59
/// ticks (~3 s) after respawning**, and `/damage` (like `/kill`) reports success
/// and does nothing (spec-0031 spike). The obvious guard — `execute store
/// success ... run damage` — is therefore **inert**, and this was measured on the
/// pinned 1.21.11 toolserver rather than reasoned about: a PackTest dummy in
/// `playerGameType: 0` (survival) with `Invulnerable: 0` and `Health: 20f` took
/// `damage @s 1000 minecraft:fall`, ended on `Health: 20f`, and the command
/// answered **success = 1**. Reading a response that does not carry the answer is
/// the same defect as reading no response at all — which is precisely what the
/// eight legacy camelCase gamerules did at two sites in that same spike.
///
/// So the guard reads the **outcome**: the wording is printed only when the
/// player it is about actually ended the tick dead. That covers every refusal in
/// one rule and needs no list of them — the respawn window, a totem (the volume
/// struck and did not take them; the next tick will), `resistance`, creative.
/// Reset to a sentinel before the read so a failed `data get` can never leave a
/// previous player's zero behind and claim a death that did not happen.
pub(super) const LETHAL_HP: &str = "#leth_hp dw.sys";

/// Damage dealt by a lethal volume, in half-hearts. Far above any reachable
/// max-health + absorption, so "lethal" is a property of the verb and not a number
/// an author has to get right. A held totem still fires — the curated
/// [`delvewright_dsl::DamageKind`] set deliberately excludes every totem-bypassing
/// vanilla type — and the next tick inside the volume kills anyway, which is the
/// totem doing its job rather than the volume failing to do its own.
pub(super) const LETHAL_DAMAGE: u32 = 1000;

/// The fixture class tag as the leading element of a summon's `Tags:` NBT list —
/// *this entity's position IS engine state* ([`crate::compiler::affordance::FIXTURE_TAG`]).
///
/// Written at the summon site rather than derived by a roster in some verb,
/// because the class belongs to the object: a bonfire's hitbox is a place whether
/// the thing quantifying over it is a teleport, a lethal volume or a verb nobody
/// has written yet.
pub(super) const FIXTURE_NBT: &str = "\"dw_fixture\",";

/// The borne class tag ([`crate::compiler::affordance::BORNE_TAG`]) — *this entity's
/// position belongs to a body that carries it*. Exactly one summon in the engine
/// wears it: an NPC's co-located dialogue hitbox, which must ride whatever its
/// speaker rides.
pub(super) const BORNE_NBT: &str = "\"dw_borne\",";

/// The box-selector argument shared by both of a volume's selectors.
pub(super) fn lethal_box(v: &crate::compiler::plan::LethalVolumePlan) -> String {
    let (lo, hi) = v.region;
    box_selector_args(lo, hi)
}

/// The per-tick driver lines for the campaign's lethal volumes (spec-0031), in
/// declaration order. Empty for a campaign that declares none, so the emitted
/// `tick` is byte-identical for everybody who has not opted in.
///
/// A volume live from a story stage (spec-0088) is driven through its own
/// `lethal_<id>_tick`, whose one line guards the volume on its gate; a volume
/// live from world-load is driven directly, as it always has been.
pub(super) fn lethal_tick(plan: &Plan) -> Vec<String> {
    let ns = &plan.namespace;
    plan.lethal_volumes
        .iter()
        .map(|v| match v.staged {
            Some(_) => format!("function {ns}:lethal_{}_tick", v.safe),
            None => format!("function {ns}:lethal_{}", v.safe),
        })
        .collect()
}

/// The one line of a staged volume's `lethal_<id>_tick` (spec-0088 §7):
/// `execute <terms> run function <ns>:lethal_<id>`, with `<terms>` the gate's
/// [`crate::compiler::plan::GateTerm`]s each rendered by `clause(false)` — the
/// formatter every effect guard and every trigger gate is written by, so the
/// volume cannot disagree with them about what a term means.
pub(super) fn lethal_stage_line(
    ns: &str,
    safe: &str,
    gate: &crate::compiler::plan::StagedGate,
) -> String {
    let terms: Vec<String> = gate.terms.iter().map(|t| t.clause(false)).collect();
    format!(
        "execute {} run function {ns}:lethal_{safe}",
        terms.join(" ")
    )
}

/// The scoreboard lines that put a staged volume's gate **open** (`open`) or
/// **shut by exactly one term** (spec-0088 §7), for a PackTest template.
///
/// Open: every required flag 1, every forbidden flag reset, every numeric datum
/// on the value [`delvewright_dsl::gate::DatumSet::pick`] chooses from the set
/// its terms admit. Shut: the open state with one term broken — the first
/// required flag reset, else the first forbidden flag set, else the first
/// numeric datum on a value its first term refuses.
pub(super) fn lethal_gate_lines(
    gate: &crate::compiler::plan::StagedGate,
    open: bool,
) -> Vec<String> {
    let party = plan::PARTY;
    let mut out: Vec<String> = Vec::new();
    for f in &gate.requires_flags {
        out.push(format!(
            "scoreboard players set {party} {} 1",
            plan::flag_score(f)
        ));
    }
    for f in &gate.forbids_flags {
        out.push(format!(
            "scoreboard players reset {party} {}",
            plan::flag_score(f)
        ));
    }
    let mut per: BTreeMap<&str, delvewright_dsl::gate::DatumSet> = BTreeMap::new();
    for c in &gate.requires_state {
        per.entry(c.state.as_str())
            .or_default()
            .require(c.op, c.value);
    }
    for (state, set) in &per {
        if let Some(v) = set.pick() {
            out.push(format!(
                "scoreboard players set {party} {} {v}",
                plan::state_score(state)
            ));
        }
    }
    if open {
        return out;
    }
    if let Some(f) = gate.requires_flags.first() {
        out.push(format!(
            "scoreboard players reset {party} {}",
            plan::flag_score(f)
        ));
    } else if let Some(f) = gate.forbids_flags.first() {
        out.push(format!(
            "scoreboard players set {party} {} 1",
            plan::flag_score(f)
        ));
    } else if let Some(c) = gate.requires_state.first() {
        let mut off = delvewright_dsl::gate::DatumSet::all();
        off.forbid(c.op, c.value);
        if let Some(v) = off.pick() {
            out.push(format!(
                "scoreboard players set {party} {} {v}",
                plan::state_score(c.state.as_str())
            ));
        }
    }
    out
}

/// The lines that put back every score [`lethal_gate_lines`] touched, so a
/// template leaves no campaign state for a sibling on the shared-batch server.
pub(super) fn lethal_gate_reset(gate: &crate::compiler::plan::StagedGate) -> Vec<String> {
    let party = plan::PARTY;
    let flags = gate.requires_flags.iter().chain(&gate.forbids_flags);
    let mut out: Vec<String> = flags
        .map(|f| format!("scoreboard players reset {party} {}", plan::flag_score(f)))
        .collect();
    let states: BTreeSet<&str> = gate
        .requires_state
        .iter()
        .map(|c| c.state.as_str())
        .collect();
    out.extend(
        states
            .into_iter()
            .map(|st| format!("scoreboard players reset {party} {}", plan::state_score(st))),
    );
    out
}

/// Generate one function per lethal volume (spec-0031).
///
/// Two lines, and the split is the whole design:
///
/// * **players** are re-bound one at a time (`execute as @a[…] run function`) so
///   the volume's own wording is `tellraw`n to the player it is about, in that
///   player's language — the `{"translate":…,"fallback":…}` component every
///   player-visible string in this engine goes through. It is guarded by
///   `tag=!dw_cutscene` like every other harmful piece of campaign machinery: a
///   player watching a cutscene is an observer, and the camera flies wherever the
///   shot needs it to. **The blow comes first and the wording is conditioned on
///   the player actually ending up dead** ([`LETHAL_HP`]): vanilla refuses damage
///   to a player for 59 ticks after they respawn, and a message printed ahead of
///   the swing would be a line the delve repeats for three seconds about a death
///   that is not happening.
/// * **everything else** takes the same `/damage` in one line, minus the engine's
///   own machinery ([`LETHAL_EXEMPT_TYPES`]).
///
/// The wording is delivered as a component and NOT as a custom `damage_type`
/// `message_id`. Vanilla builds a death message from `message_id` with no
/// `fallback` field, so that spelling would ship a raw translation key
/// (`death.attack.…`) to any player who declines the resource-pack prompt — which
/// spec-0029 §3 makes an invariant against, and which `DW0185` would not catch
/// because the key is not the authored string. Vanilla's own broadcast still
/// fires, worded by the declared `damage_type`: the party reads who died, the
/// victim reads what the place was.
///
/// The kill is an ordinary `/damage`, so the vanilla `deathCount` edge
/// (`dw.deaths` / `dw.death_ack`), the checkpoint re-seat (`cp_respawn_check`) and
/// `keep_inventory` all see exactly the death they already handle. There is no
/// second death detector here, and there is nothing for one to do.
pub(super) fn emit_lethal_functions(plan: &Plan) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut fns: Vec<(String, String)> = Vec::new();
    for v in &plan.lethal_volumes {
        let bx = lethal_box(v);
        let kind = v.damage_type.id();
        if let Some(gate) = &v.staged {
            fns.push((
                format!("lethal_{}_tick", v.safe),
                lines(&[lethal_stage_line(ns, &v.safe, gate)]),
            ));
        }
        let exempt: String = LETHAL_EXEMPT_TYPES
            .iter()
            .map(|t| format!(",type=!{t}"))
            .collect();
        fns.push((
            format!("lethal_{}", v.safe),
            lines(&[
                format!(
                    "execute as @a[{bx},tag=!{CUTSCENE_TAG}] run function {ns}:lethal_{}_kill",
                    v.safe
                ),
                format!(
                    "execute as @e[{bx},{},type=!minecraft:player{exempt}] run damage @s \
                     {LETHAL_DAMAGE} {kind}",
                    crate::compiler::affordance::FIXTURE_EXCLUDE
                ),
            ]),
        ));
        fns.push((
            format!("lethal_{}_kill", v.safe),
            lines(&[
                format!("damage @s {LETHAL_DAMAGE} {kind}"),
                format!("scoreboard players set {LETHAL_HP} 1"),
                format!("execute store result score {LETHAL_HP} run data get entity @s Health 100"),
                format!(
                    "execute if score {LETHAL_HP} matches ..0 run tellraw @s {}",
                    tr(&v.message)
                ),
            ]),
        ));
    }
    fns
}
