//! Environment, step and press triggers.

use super::*;

/// Command storage holding the UUID of the player who most recently struck (or
/// used) a click trigger, for the duration of that trigger's own effect bundle.
///
/// Vanilla writes the clicking player's UUID into the `minecraft:interaction`
/// entity's `attack` / `interaction` record; `data modify … set from entity` is the
/// intended primitive for moving it, and command storage is the intended place to
/// park it. Written at the top of `trig_<id>` and removed at the bottom, so it is
/// live exactly while the trigger's synchronous effects run and can never go stale.
pub(super) const STRIKER_STORAGE: &str = "dw:strike";

/// The storage path under [`STRIKER_STORAGE`] holding the striking player's UUID.
pub(super) const STRIKER_PATH: &str = "player";

/// Whether `t` is a click trigger (`strike` / `strike-npc` / `use`) — the forms
/// whose interaction entity records *which player* acted.
pub(super) fn trigger_is_click(t: &delvewright_dsl::EnvTrigger) -> bool {
    t.on.is_click()
}

/// The tag of the `minecraft:interaction` a click trigger's record is read off:
/// the trigger's own `dw_trig_<id>` (worn by the box it summons, or added to the
/// NPC hitbox or press body it rides), or — for a `strike-assembly`
/// (spec-0082) — the assembly's own hitbox, which the trigger rides without
/// tagging it, since the assembly summons and kills that box itself.
pub(super) fn trigger_carrier_tag(t: &delvewright_dsl::EnvTrigger) -> String {
    match t.on.assembly_target() {
        Some(a) => crate::compiler::assembly::hit_tag(&plan::safe_local(a.as_str())),
        None => format!("dw_trig_{}", plan::safe_local(t.id.as_str())),
    }
}

/// The NBT record a click trigger reads off its interaction entity: a left-click
/// writes `attack`, a right-click writes `interaction`.
pub(super) fn trigger_record(t: &delvewright_dsl::EnvTrigger) -> &'static str {
    match t.on {
        delvewright_dsl::TriggerOn::Use => "interaction",
        _ => "attack",
    }
}

/// Whether this trigger's effect tree reaches an `unleash-actor` — the only reason
/// to capture the striker at all. Campaigns that never unleash from a click stay
/// byte-identical.
pub(super) fn trigger_unleashes(t: &delvewright_dsl::EnvTrigger) -> bool {
    let mut all = Vec::new();
    for e in &t.effects {
        push_effect_deep(e, &mut all);
    }
    all.iter()
        .any(|e| matches!(&e.verb, Verb::UnleashActor { .. }))
}

/// Whether any click trigger in the campaign captures a striker — i.e. whether
/// [`STRIKER_STORAGE`] can ever hold a value. Gates the aggro-lock lines in
/// `unleash_<id>` so an unrelated campaign's unleash functions are unchanged.
pub(super) fn campaign_captures_striker(c: &delvewright_dsl::Campaign) -> bool {
    c.quests
        .content
        .triggers
        .iter()
        .any(|t| trigger_is_click(t) && trigger_unleashes(t))
}

/// Environment-trigger interaction-entity summons (strike/use) for
/// `setup_finish`. Approach triggers need no entity. Empty for a campaign with no
/// triggers (byte-identical v0.2/v0.3).
///
/// A left-click trigger that rides an NPC — `strike-npc` (DSL v0.6), or the
/// pre-0.6 `strike` on the NPC's own stand anchor — gets **no entity of its
/// own**: the NPC's interaction hitbox is the trigger's sole carrier
/// ([`npc_hitbox_trigger_tags`]). Emitting a second, exactly co-located hitbox
/// here made the vanilla client's entity ray-pick ambiguous — an exact tie
/// resolves to whichever entity the pick iterates first, in practice this
/// world-init summon — so every right-click landed on an entity without the
/// `dw_npc_<n>` tag and the `player_interacted_with_entity` dialogue
/// advancement never fired (round-6 island QA: after the boulder seal,
/// Polyphemus could not be talked to at all). One cell, one hitbox. The
/// trigger's lifecycle therefore follows the NPC's presence — which is also
/// its meaning: the thing being struck is the NPC.
pub(super) fn env_trigger_setup(plan: &Plan, chrome: &delvewright_dsl::Chrome) -> Vec<String> {
    use delvewright_dsl::TriggerOn;
    let mut out = Vec::new();
    let props: BTreeMap<String, crate::compiler::light::Placement> =
        crate::compiler::pressable::trigger_props(plan)
            .into_iter()
            .collect();
    for t in &plan.emitted_triggers(chrome) {
        // An `approach` and a `step` read a body's position; nothing is summoned.
        if !t.on.is_click() {
            continue;
        }
        // `strike-npc` never has a cell of its own; a `strike` on an NPC's stand
        // anchor gives its cell up to that NPC's hitbox. Either way, no summon.
        let Some(at) = t.at_anchor() else {
            continue;
        };
        if matches!(t.on, TriggerOn::Strike) && crate::compiler::pressable::npc_stands_at(plan, at)
        {
            continue;
        }
        // spec-0093 §6.5: the trigger's `prop` is placed at its cell — a lever
        // or a button is then the whole body (vanilla reports its press, so
        // nothing is summoned); any other block stands under the hitbox.
        // The one list of prop cells ([`crate::compiler::pressable::trigger_props`]),
        // which the written world lays too.
        if let Some(p) = props.get(t.id.as_str()) {
            out.push(format!(
                "setblock {} {} {} {}",
                p.pos[0], p.pos[1], p.pos[2], p.block
            ));
        }
        // Same rule, one layer out: a click trigger anchored on a gate
        // the campaign SEALS rides that seal's own hitboxes — `seal_arm_<safe>`
        // summons them wearing this trigger's tag. A second entity here would be
        // exactly co-located with them, and the ray-pick tie is what killed the
        // island's boulder hint (`DESIGN.md` round 13). One cell, one hitbox.
        let tag = format!("dw_trig_{}", plan::safe_local(t.id.as_str()));
        match crate::compiler::pressable::trigger_body(plan, t) {
            // The block is the body; vanilla reports its press.
            crate::compiler::pressable::Body::Block { .. } => {}
            // An existing set covers this anchor; `seal_fns` / `ws_arm_fns` put
            // this trigger's tag on those entities. One cell, one hitbox.
            crate::compiler::pressable::Body::Rides { .. } => {}
            // The anchor is a REGION. A point body here is buried in the solid
            // block and reachable from nowhere (see `crate::compiler::pressable`), so the
            // object gets the clickable shape it actually has: one protruding box
            // per shell cell, exactly as a `close-gate` seal has always done.
            crate::compiler::pressable::Body::Region(cells) => {
                for c in cells {
                    let x = fmt_centi(c[0] as i64 * 100 + 50);
                    let y = fmt_centi(c[1] as i64 * 100 - 1);
                    let z = fmt_centi(c[2] as i64 * 100 + 50);
                    out.push(format!(
                        "summon minecraft:interaction {x} {y} {z} \
                         {{width:{SEAL_BOX_SIZE},height:{SEAL_BOX_SIZE},response:1b,Invulnerable:1b,Tags:[{FIXTURE_NBT}\"{tag}\"]}}"
                    ));
                }
            }
            // A point in open space: the ordinary body, byte-identical.
            crate::compiler::pressable::Body::Point(p) => {
                let q = ent_xyz(p);
                out.push(format!(
                    "summon minecraft:interaction {} {} {} {{width:1.0f,height:2.0f,response:1b,Invulnerable:1b,Tags:[{FIXTURE_NBT}\"{tag}\"]}}",
                    q[0], q[1], q[2]
                ));
            }
            // `DW0426` has already failed the build.
            crate::compiler::pressable::Body::Nothing => {}
        }
    }
    out
}

/// `DW0426`: every click trigger must be anchored somewhere a player can click.
///
/// Build tier (exit 3), raised before any function is emitted. The trigger
/// declares an anchor, a click and a full effect bundle, and the press lands on
/// nothing — the beat never happens and every board stays green, which is the
/// unbound-vacuity class this whole task came out of.
pub(super) fn check_trigger_bodies(
    plan: &Plan,
) -> Result<crate::compiler::pressable::PressLedger, BuildFailure> {
    use delvewright_dsl::TriggerOn;
    let mut ledger = crate::compiler::pressable::PressLedger::default();
    // Every trigger this build EMITS, not only the campaign's own: a press answer
    // the compiler synthesizes lands on a body exactly as an authored click does,
    // and a proof that walked the authored list alone would leave the compiler's
    // own presses unexamined and the ledger's count short of what shipped.
    for t in &plan.emitted_triggers_unlocalized() {
        // A position trigger has no body to press; a `step`'s cell is judged by
        // `DW0917` against the assembled world instead.
        if !t.on.is_click() {
            continue;
        }
        // A `strike-assembly` (spec-0082) lands on the assembly's own hitbox.
        // An assembly with none is `DW0936`, and one out of reach `DW0937`;
        // what is recorded here is the body the press resolved to.
        if let Some(a) = t.on.assembly_target() {
            let placed = crate::compiler::assembly::placed(plan);
            if let Some(p) = placed.iter().find(|p| p.decl.id == *a)
                && let Some(h) = &p.decl.hitbox
            {
                ledger.push(
                    t.id.as_str(),
                    t.on.kind(),
                    &p.decl.at.display(),
                    &format!(
                        "rides assembly `{a}`'s hitbox, {} x {} standing on {:?}",
                        h.width,
                        h.height,
                        p.hitbox_cell()
                    ),
                );
            }
            continue;
        }
        let Some(at) = t.at_anchor() else {
            continue;
        };
        if matches!(t.on, TriggerOn::Strike) && crate::compiler::pressable::npc_stands_at(plan, at)
        {
            ledger.push(
                t.id.as_str(),
                t.on.kind(),
                at,
                "rides the NPC's dialogue hitbox",
            );
            continue;
        }
        let body = crate::compiler::pressable::trigger_body(plan, t);
        if body != crate::compiler::pressable::Body::Nothing {
            ledger.push(
                t.id.as_str(),
                t.on.kind(),
                at,
                &crate::compiler::pressable::describe(&body),
            );
            continue;
        }
        return Err(BuildFailure::Diagnostic {
            code: crate::compiler::pressable::DW_TRIGGER_UNPRESSABLE,
            message: format!(
                "trigger `{}` watches a `{}` on anchor `{}`, but nothing at that anchor is \
                 clickable: it resolves to no placed piece, so the compiler has no cell to give \
                 the trigger a body at and the press can never land. The trigger's effects would \
                 simply never run, with every check green. Prescription: {}, or drop the \
                 trigger.",
                t.id,
                t.on.kind(),
                at,
                delvewright_dsl::Placement::of(plan.campaign).anchor_remedy(
                    "anchor it on a place a prefab provides (anchor names come from prefab \
                     metadata; do NOT invent one)"
                ),
            ),
        });
    }
    Ok(ledger)
}

/// The three gate fragments a trigger's dispatch clause carries: its
/// at-most-once guard, its forbidden flags and its required flags and state.
/// One authority for [`env_trigger_tick`], the assembly PackTest that runs the
/// very clause a blow on a hitbox meets (spec-0082), and [`press_dispatch_fn`],
/// which re-states the gate for a presser trigger.
///
/// **Every fragment is space-TERMINATED** (`unless score … matches 1 `) or
/// empty, so a caller concatenates them straight in front of the next clause
/// or `run`. The required half is assembled from [`party_flag_gate`] and
/// [`state_cond`], which are space-PREFIXED, and re-spaced here once: a caller
/// that spliced the prefixed form in front of `run` shipped `… matches 1run`
/// (and `execute  if …`), which 1.21.11 refuses.
pub(super) fn trigger_poll_guards(
    plan: &Plan,
    t: &delvewright_dsl::EnvTrigger,
) -> (String, String, String) {
    let id = plan::safe_local(t.id.as_str());
    let once_guard = if t.once {
        format!("unless score #trig_{id} dw.sys matches 1 ")
    } else {
        String::new()
    };
    // Flags are party state (spec-0018): the gate is a single `#party` read,
    // positive and negative alike. `unless … matches 1` is unset-safe (an
    // uninitialized flag score counts as "not set").
    let required = format!(
        "{}{}",
        party_flag_gate(&t.requires_flags),
        // DSL v0.10 (spec-0031). A trigger's arming gate is a party predicate
        // (`DW0503` keeps `player`-scoped data out of it).
        state_cond(plan, &t.requires_state, false)
    );
    let flag_guard = if required.is_empty() {
        required
    } else {
        format!("{} ", required.trim_start())
    };
    let forbid_guard: String = t
        .forbids_flags
        .iter()
        .map(|f| {
            format!(
                "unless score {} {} matches 1 ",
                plan::PARTY,
                plan::flag_score(f.as_str())
            )
        })
        .collect();
    (once_guard, forbid_guard, flag_guard)
}

/// A click trigger's tick clause and the clear that consumes its record:
/// `(poll, clear)`.
///
/// The two click streams are separate NBT fields on ONE
/// `minecraft:interaction`: a left-click writes `attack`, a right-click writes
/// `interaction`. That is what lets a `strike-npc` trigger share the hitbox
/// with the NPC's dialogue — the dialogue advancement reads the right-click,
/// this reads the left-click, and neither consumes the other's record. The
/// poll fires when the interaction entity has recorded the event and (if
/// gated) the party holds the flags; the clear removes the record. The
/// carrier is the trigger's own tag, or — for a `strike-assembly` — the
/// assembly's hitbox (spec-0082 §4.3).
/// Whether a trigger's body is a block a hand presses (spec-0093 §6.5) — read
/// from the one authority, [`crate::compiler::pressable::trigger_body`].
pub(super) fn trigger_is_block_bound(plan: &Plan, t: &delvewright_dsl::EnvTrigger) -> bool {
    matches!(
        crate::compiler::pressable::trigger_body(plan, t),
        crate::compiler::pressable::Body::Block { .. }
    )
}

/// The `minecraft:default_block_use` criterion for a press of `block` at `cell`
/// (spec-0093 §6.5): vanilla's own report that a player used a block with its
/// default interaction — a lever flipped, a button pressed — held to the block's
/// id and its exact position, so a lever elsewhere fires nothing here. The
/// `location` conditions are loot-table predicates, as the pinned format spells
/// them.
pub(super) fn block_use_criterion(block: &str, cell: [i32; 3]) -> serde_json::Value {
    let id = crate::compiler::pressable::block_id(block);
    // The criterion hands its predicates the block's CENTRE (`x + 0.5`), so the
    // range is the whole cell `[x, x + 1]`, which the centre is inside and the
    // neighbouring cells' centres are not.
    let cell_span = |v: i32| json!({ "min": v, "max": v + 1 });
    json!({
        "trigger": "minecraft:default_block_use",
        "conditions": {
            "location": [{
                "condition": "minecraft:location_check",
                "predicate": {
                    "block": { "blocks": [id] },
                    "position": {
                        "x": cell_span(cell[0]),
                        "y": cell_span(cell[1]),
                        "z": cell_span(cell[2])
                    }
                }
            }]
        }
    })
}

pub(super) fn click_trigger_poll(plan: &Plan, t: &delvewright_dsl::EnvTrigger) -> (String, String) {
    let ns = &plan.namespace;
    let id = plan::safe_local(t.id.as_str());
    let (once_guard, forbid_guard, flag_guard) = trigger_poll_guards(plan, t);
    let rec = trigger_record(t);
    let carrier = trigger_carrier_tag(t);
    (
        format!(
            "execute {once_guard}{forbid_guard}if entity @e[tag={carrier},nbt={{{rec}:{{}}}}] {flag_guard}run function {ns}:trig_{id}"
        ),
        format!("execute as @e[tag={carrier}] run data remove entity @s {rec}"),
    )
}

/// Environment-trigger per-tick checks for the `tick` function. Empty for a
/// campaign with no triggers.
///
/// **Two phases, not one, for the click triggers** (round-8 island QA). A click
/// trigger is `if <record present> run <effects>` followed by `data remove` of the
/// record — the removal is what makes a held-down click fire exactly once. Emitting
/// that pair *per trigger*, inline, is only sound while at most one trigger reads a
/// given interaction entity. Several `strike-npc` triggers legitimately ride ONE
/// NPC hitbox (see [`npc_hitbox_trigger_tags`]) — the island's giant carries
/// `wake-the-giant` (requires `flag/asleep`) and `his-house` (forbids it), one
/// hitbox, mutually exclusive gates. Inline removal made the FIRST-DECLARED trigger
/// consume the record even when its own gate was shut, so `his-house` could never
/// see a click and never fired: a suppressed trigger starved its siblings, and which
/// one starved depended on declaration order.
///
/// So the record is read by every trigger first and cleared afterwards: all fire
/// clauses in declaration order, then all clear clauses. The semantics become
/// order-independent — every trigger sharing a hitbox sees the same click, and each
/// fires exactly when its own gate says so. Consumption is unchanged (the record is
/// gone by the end of the same `tick` pass, so a held click still fires once).
///
/// Byte impact: a campaign whose click triggers are its last-declared triggers is
/// unchanged; any other ordering moves the clear clauses to the end of the block.
pub(super) fn env_trigger_tick(plan: &Plan, chrome: &delvewright_dsl::Chrome) -> Vec<String> {
    use delvewright_dsl::TriggerOn;
    let ns = &plan.namespace;
    let mut out = Vec::new();
    // Phase 2, accumulated while phase 1 is emitted: `(tag, record)` for every
    // click trigger, in declaration order (deterministic).
    let mut clears: Vec<String> = Vec::new();
    for t in &plan.emitted_triggers(chrome) {
        // A `presser` trigger is not polled at all (DSL v0.11): its dispatch is a
        // `player_interacted_with_entity` advancement, which is the only vanilla
        // primitive that knows WHO pressed. It therefore also emits no `data
        // remove` — an advancement observes the click without consuming it, which
        // is what lets a press answer share one hitbox with a polled trigger and
        // neither eat the other's record (round-8: adjudicate conditionally,
        // consume unconditionally).
        // A `step` is polled whoever it addresses: a body in the cell is the
        // event, and with `presser` the same selector names the actor.
        if matches!(t.on, TriggerOn::Step) {
            out.extend(step_trigger_poll(plan, t));
            continue;
        }
        if t.addresses_presser() {
            continue;
        }
        // spec-0093 §6.5: a block a hand presses is dispatched by its
        // `default_block_use` advancement, not read off a record on the tick.
        if trigger_is_block_bound(plan, t) {
            continue;
        }
        let id = plan::safe_local(t.id.as_str());
        let (once_guard, forbid_guard, flag_guard) = trigger_poll_guards(plan, t);
        match &t.on {
            TriggerOn::Strike
            | TriggerOn::Use
            | TriggerOn::StrikeNpc { .. }
            | TriggerOn::StrikeAssembly { .. } => {
                let (poll, clear) = click_trigger_poll(plan, t);
                out.push(poll);
                // Several triggers may ride one hitbox (an NPC's, an
                // assembly's); it is cleared once, after every one of them has
                // been offered it.
                if !clears.contains(&clear) {
                    clears.push(clear);
                }
            }
            TriggerOn::Approach { range } => {
                if let Some(p) = t.at_anchor().and_then(|at| anchor_point_any(plan, at)) {
                    // The proximity test stays per-player (`@a[distance=…]` — SOME
                    // party member walked in); the flag gate is a party read
                    // alongside it, no longer merged into the selector.
                    out.push(format!(
                        "execute {once_guard}{forbid_guard}positioned {} {} {} if entity @a[distance=..{range}{}] {flag_guard}run function {ns}:trig_{id}",
                        p[0], p[1], p[2], observer_guard(plan)
                    ));
                }
            }
            // Polled above.
            TriggerOn::Step => {}
        }
    }
    out.extend(clears);
    out
}

/// The selector terms for a body standing in a step cell: a player whose
/// hitbox is in the cell, and who is not only watching. One authority for a
/// plate or tripwire trap's detection ([`trap_fire_tick`]) and a `step`
/// trigger's ([`step_trigger_poll`]), so the two fire on the same body.
pub(super) fn step_cell_terms(c: [i32; 3]) -> String {
    format!("{},tag=!{CUTSCENE_TAG}", step_cell_box(c))
}

/// The volume half of [`step_cell_terms`]: the one block at `c`.
pub(super) fn step_cell_box(c: [i32; 3]) -> String {
    format!("x={},dx=0,y={},dy=0,z={},dz=0", c[0], c[1], c[2])
}

/// The tick clauses of a `step` trigger.
///
/// **Party.** Edge-latched on `#stp_<id>`: `step_<id>` sets it when it
/// dispatches, and it is cleared once no player is in the cell, so a plate
/// stood on fires once and fires again only after it is stepped off and on —
/// the shape a plate or tripwire trap's `rearm` has.
///
/// **Presser.** The same selector, run `as` each player in the cell who does
/// not carry `dw_stp_<id>`; `step_<id>` tags that player, and the tag comes off
/// when they leave the cell. So every player who steps on is dispatched once
/// per step, as `@s` — the act and the actor are one fact, a body in the cell,
/// and nothing about who acted is inferred after the event.
pub(super) fn step_trigger_poll(plan: &Plan, t: &delvewright_dsl::EnvTrigger) -> Vec<String> {
    let ns = &plan.namespace;
    let id = plan::safe_local(t.id.as_str());
    let Some(c) = t.at_anchor().and_then(|at| anchor_point_any(plan, at)) else {
        return Vec::new();
    };
    let sel = step_cell_terms(c);
    if t.addresses_presser() {
        return vec![
            format!("execute as @a[{sel},tag=!{STEP_TAG}{id}] run function {ns}:step_{id}"),
            format!(
                "execute as @a[tag={STEP_TAG}{id}] unless entity @s[{}] run tag @s remove \
                 {STEP_TAG}{id}",
                step_cell_box(c)
            ),
        ];
    }
    let (once_guard, forbid_guard, flag_guard) = trigger_poll_guards(plan, t);
    vec![
        format!(
            "execute {once_guard}{forbid_guard}unless score #stp_{id} dw.sys matches 1 if entity \
             @a[{sel}] {flag_guard}run function {ns}:step_{id}"
        ),
        format!("execute unless entity @a[{sel}] run scoreboard players set #stp_{id} dw.sys 0"),
    ]
}

/// The tag a `presser` `step` trigger puts on a player standing in its cell,
/// suffixed with the trigger's safe id.
pub(super) const STEP_TAG: &str = "dw_stp_";

/// The dispatch function of a `step` trigger: `step_<id>`, which latches the
/// step ([`step_trigger_poll`]) and runs `trig_<id>`.
///
/// A party step reaches it from a tick clause that already carries the
/// trigger's gate. A presser step reaches it once per player who steps on, so
/// the gate — `once`, the forbidden flags, the required flags and state — is
/// stated here, spelled by [`trigger_poll_guards`] as the tick spells it.
pub(super) fn step_dispatch_fn(
    plan: &Plan,
    t: &delvewright_dsl::EnvTrigger,
    id: &str,
) -> (String, String) {
    let ns = &plan.namespace;
    if !t.addresses_presser() {
        return (
            format!("step_{id}"),
            lines(&[
                format!("scoreboard players set #stp_{id} dw.sys 1"),
                format!("function {ns}:trig_{id}"),
            ]),
        );
    }
    let (once_guard, forbid_guard, flag_guard) = trigger_poll_guards(plan, t);
    let conds = format!("{once_guard}{forbid_guard}{}", flag_guard.trim_start());
    let conds = conds.trim_end();
    let dispatch = if conds.is_empty() {
        format!("function {ns}:trig_{id}")
    } else {
        format!("execute {conds} run function {ns}:trig_{id}")
    };
    (
        format!("step_{id}"),
        lines(&[format!("tag @s add {STEP_TAG}{id}"), dispatch]),
    )
}

/// Environment-trigger effect functions (`trig_<id>`). A trigger is a **party
/// event** (spec-0018): the beat fires once and its player-facing effects address
/// `@a` on their own (see [`Audience::Party`]), so the bundle is emitted plainly
/// — no outer `as @a`, which under party state would fire every `fill`, driver
/// start and party-holder write once per player. `once` sets a global sentinel so
/// the trigger fires at most once.
///
/// The function is dispatched from `env_trigger_tick` without an executor for an
/// `approach`/`strike`/`use` trigger, so nothing here may rely on `@s` — the
/// party model means nothing needs to.
///
/// **`#trig_<id>` is written by every trigger, not only `once` ones** (round-8).
/// `once` *reads* it as its at-most-once guard, but the write is what makes trigger
/// dispatch observable at all: without it a repeatable trigger firing leaves no
/// machine-readable trace, and the shared-hitbox starvation bug — where a trigger
/// simply never fired — was invisible to every automated check the repo had. One
/// scoreboard write on a rare event buys a PackTest that can assert *which* of two
/// triggers on one hitbox actually ran (`v06_shared_hitbox`). Byte impact: one added
/// line per non-`once` trigger function.
///
/// **An `audience: presser` trigger reverses both of those** (DSL v0.11). Its
/// bundle is emitted under [`Audience::Solo`] — `@s` is the player who pressed —
/// and it is reached from a second, tiny function [`press_dispatch_fn`] that the
/// interaction advancement rewards, rather than from the tick. Everything else is
/// identical, which is the point: a press answer is not a mechanism, it is this
/// mechanism with a different addressee.
pub(super) fn env_trigger_fns(
    plan: &Plan,
    chrome: &delvewright_dsl::Chrome,
) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for t in &plan.emitted_triggers(chrome) {
        let id = plan::safe_local(t.id.as_str());
        if matches!(t.on, delvewright_dsl::TriggerOn::Step) {
            out.push(step_dispatch_fn(plan, t, &id));
        } else if t.addresses_presser() || trigger_is_block_bound(plan, t) {
            out.push(press_dispatch_fn(plan, t, &id));
        }
        let mut body: Vec<String> = Vec::new();
        body.push(format!("scoreboard players set #trig_{id} dw.sys 1"));
        // The fired marker a critical-path `trigger` step passes on — the same
        // anchored channel an objective's completion uses, with the trigger's own
        // id as the token, broadcast before any effect can teleport or end the
        // delve. Only a trigger a path could perform carries one
        // (`plan::trigger_may_be_performed`), so no other bundle moves a byte.
        if plan::trigger_may_be_performed(t) {
            body.push(format!(
                "tellraw @a {}",
                json!({
                    "text": plan::marker_line(&plan.namespace, t.id.as_str()),
                    "color": "dark_gray"
                })
            ));
        }
        // Striker capture. The click record is still on
        // the hitbox here — `env_trigger_tick` clears every record only after every
        // trigger has been offered it — so this is the one place the acting player's
        // UUID is knowable. Parked in storage rather than passed as an argument
        // because `unleash-actor` may sit behind any amount of nesting inside this
        // bundle, and removed again below so it can never leak into a later beat.
        let capture = trigger_is_click(t) && trigger_unleashes(t);
        if capture {
            let rec = trigger_record(t);
            body.push(format!(
                "data modify storage {STRIKER_STORAGE} {STRIKER_PATH} set from entity @e[tag={},limit=1] {rec}.player",
                trigger_carrier_tag(t)
            ));
        }
        // The trigger's own flag gate is already proven by `env_trigger_tick`
        // (or by `press_<id>`) before it dispatches here; each effect still
        // carries its own gate.
        for e in &t.effects {
            emit_gated_effect(plan, e, trigger_audience(t), &mut body);
        }
        if capture {
            body.push(format!(
                "data remove storage {STRIKER_STORAGE} {STRIKER_PATH}"
            ));
        }
        out.push((format!("trig_{id}"), lines(&body)));
    }
    out
}

/// The audience one trigger's bundle is emitted under.
///
/// [`root_audience`] answers for the root *class*, and remains the authority the
/// DSL's `EffectRootKind::runs_with_acting_player` is bound to. A trigger is the
/// one root whose audience is a per-declaration fact (DSL v0.11), and this is
/// where that is resolved — bound to the DSL by
/// `EffectRootOwner::runs_with_acting_player`, which `DW0503` reads, so the
/// validator and the emitter cannot disagree about whether `@s` exists.
pub(super) fn trigger_audience(t: &delvewright_dsl::EnvTrigger) -> Audience {
    if t.addresses_presser() {
        Audience::Solo
    } else {
        root_audience(delvewright_dsl::EffectRootKind::Trigger)
    }
}

/// The advancement reward function of an `audience: presser` trigger (DSL v0.11):
/// `press_<id>`, which revokes its own grant and then runs the trigger's bundle
/// **as the player who right-clicked**.
///
/// This is `seal_hint_<safe>` generalized off the verb it was built onto. The
/// revoke is what makes the object answer *every* press rather than only the
/// first — a wall is not consumed by being asked — and `once`, the flag gate and
/// the state gate are re-stated here because for a presser trigger this function
/// takes the place of the tick clause that would otherwise have carried them.
/// They are the trigger's own, read from [`trigger_poll_guards`] — the one
/// authority `env_trigger_tick` reads — so the two dispatch routes gate
/// identically.
pub(super) fn press_dispatch_fn(
    plan: &Plan,
    t: &delvewright_dsl::EnvTrigger,
    id: &str,
) -> (String, String) {
    let ns = &plan.namespace;
    let (once_guard, forbid_guard, flag_guard) = trigger_poll_guards(plan, t);
    // An ungated press answer — which is every one the compiler synthesizes —
    // calls its bundle outright. `execute run function …` is legal and would
    // work, but a conditionless `execute` in shipped output reads as a guard
    // somebody deleted.
    let dispatch = if once_guard.is_empty() && forbid_guard.is_empty() && flag_guard.is_empty() {
        format!("function {ns}:trig_{id}")
    } else {
        format!("execute {once_guard}{forbid_guard}{flag_guard}run function {ns}:trig_{id}")
    };
    (
        format!("press_{id}"),
        lines(&[
            format!("advancement revoke @s only {ns}:press_{id}"),
            dispatch,
        ]),
    )
}
