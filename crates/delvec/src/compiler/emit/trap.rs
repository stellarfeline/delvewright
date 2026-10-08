//! Traps: gate hardware, payload verbs (`volley`, `collapse`), fire guards (spec-0011, spec-0022).

use super::*;

/// The content key naming a spec-0022 trap-payload verb's generated function.
/// FNV-1a over the **verb's** `Debug` rendering, so two identical `volley`s share
/// one function body. The guard is not part of it: emission wraps the call, never
/// the body, so a gated and an ungated `volley` of the same shape are one body
/// (ADR-0006 — no wall-clock / hash-order input).
pub(super) fn payload_verb_key(eff: &QuestEffect) -> String {
    let s = format!("{:?}", eff.verb);
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// The generated function name for a `volley` effect.
pub(super) fn volley_fn(eff: &QuestEffect) -> String {
    format!("volley_{}", payload_verb_key(eff))
}

/// The generated function name for a `collapse` effect.
pub(super) fn collapse_fn(eff: &QuestEffect) -> String {
    format!("collapse_{}", payload_verb_key(eff))
}

/// Trap flag-gating hardware: for every trap that declares a flag gate, the
/// trigger block its `anchor/trap` prefab metadata declares — the thing the gate
/// physically removes and restores.
///
/// The compiler owns world mutation, so "the trap is inactive while a flag is set"
/// is implemented as exactly that: the plate or tripwire is taken out of the world
/// and put back on the flag transition. The block comes from prefab metadata for
/// the same reason a `close-gate`'s fill block does (`DW0343`): the hardware is
/// baked into the `.nbt` and only the prefab author knows what it is. It is
/// restored **verbatim, blockstate and all** — stamping a bare id over an authored
/// state silently changes the block (the `DW0354` lesson).
///
/// Only a trigger whose entire state is the block itself can be gated this way. A
/// `trapped-chest` trigger carries a **block entity with an inventory** that removal
/// would destroy and the compiler could not restore, so a flag gate on one is
/// rejected (`DW0363`) rather than shipped as folklore. So is a gated trap whose
/// prefab declares no `trigger_block` at all: be loud, do not guess.
pub(super) fn trap_gate_hardware(
    plan: &Plan,
    prefabs: &crate::compiler::registry::PrefabRegistry,
) -> Result<BTreeMap<String, String>, BuildFailure> {
    let mut out = BTreeMap::new();
    for t in plan.traps.iter().filter(|t| trap_is_gated(t)) {
        let declared = prefabs.trap_trigger_block(&t.at_anchor);
        let gatable = declared
            .map(|b| b.split('[').next().unwrap_or(b))
            .is_some_and(crate::compiler::assembled::is_passable_trap_trigger);
        if !gatable {
            let what = match declared {
                Some(b) => format!("declares `trigger_block` `{b}`"),
                None => "declares no `trigger_block`".to_string(),
            };
            return Err(BuildFailure::Diagnostic {
                code: DW_TRAP_GATE_UNSUPPORTED,
                message: format!(
                    "trap `{}` declares a gate (`requires_flags`/`forbids_flags`/ \
                     `requires_state`), but its `anchor/trap` marker `{}` {what}. A gate \
                     physically removes the trigger from the world while it is shut and puts \
                     it back after, so \
                     it is only sound for a trigger whose whole state is the block: a pressure \
                     plate or a tripwire. A `trapped-chest` trigger carries a block entity with \
                     an inventory that removal would destroy. Declare the plate/tripwire as \
                     `trigger_block` on the anchor's prefab metadata (with its blockstate, as a \
                     gate anchor declares its fill `block`), switch the trap to a \
                     `pressure-plate`/`tripwire` trigger, or drop the gate and gate the \
                     story beat that arms the trap instead.",
                    t.id, t.at_anchor,
                ),
            });
        }
        out.insert(t.safe.clone(), declared.unwrap_or_default().to_string());
    }
    Ok(out)
}

/// Whether `t` declares a gate at all — flags, negative flags, or (DSL v0.10) a
/// numeric comparison. An ungated trap emits nothing new, so every existing
/// campaign stays byte-identical.
///
/// All three axes, not two: the arming machinery a gated trap gets is the same
/// machinery whichever axis shut it, and a trap gated only by a number that
/// quietly skipped it would be armed forever.
pub(super) fn trap_is_gated(t: &plan::TrapPlan) -> bool {
    !t.requires_flags.is_empty() || !t.forbids_flags.is_empty() || !t.requires_state.is_empty()
}

/// The authored gate of a planned trap, read off its DSL declaration so every
/// emission site reads the gate through [`Plan::gate_terms`] — the one rule that
/// says which holder a term lives on.
pub(super) fn trap_gate_of<'a>(plan: &Plan<'a>, t: &plan::TrapPlan) -> Gate<'a> {
    plan.campaign
        .quests
        .content
        .traps
        .iter()
        .find(|d| d.id.as_str() == t.id)
        .map_or(Gate::OPEN, delvewright_dsl::Trap::gate)
}

/// The clauses of `trap_gate_tick`, which the tick calls: open and shut every
/// gated trap's hardware.
///
/// Edge-triggered on a per-trap sentinel `#trapgate_<safe>` (1 = armed, i.e. the
/// trigger block is in the world) so the `setblock` fires only on a transition —
/// a per-tick unconditional write would be both wasteful and wrong (it would also
/// fight the disarm path).
///
/// The gate is **campaign state**, so every term is read where the campaign
/// writes it: [`Plan::gate_terms`] — a flag on the party holder, a datum on its
/// declared holder (party-scoped here by `DW0503`). A trap reads its gate through
/// the same rule every other gate consumer uses; a selector asking whether some
/// player carries a flag score matches nobody, because no flag is written on a
/// player.
///
/// Shutting is one clause per term because "not (every term holds)" is a
/// disjunction: any single term failing shuts the gate on its own. Each is
/// idempotent behind the sentinel. Opening is one clause carrying the whole
/// conjunction.
pub(super) fn trap_gate_tick(plan: &Plan) -> Vec<String> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for t in plan.traps.iter().filter(|t| trap_is_gated(t)) {
        let id = &t.safe;
        let gate = trap_gate_of(plan, t);
        out.extend(trap_gate_shut_clauses(plan, t));
        out.push(format!(
            "execute unless score #trapgate_{id} dw.sys matches 1{} run function {ns}:trap_gate_on_{id}",
            gate_cond(plan, gate)
        ));
    }
    out
}

/// One shutting clause per term of `t`'s gate, each guarded by the armed
/// sentinel: "not (every term holds)" is a disjunction, so any single term
/// failing disarms the trap on its own. The tick and the world-start seed read
/// the gate through these same clauses.
pub(super) fn trap_gate_shut_clauses(plan: &Plan, t: &plan::TrapPlan) -> Vec<String> {
    let ns = &plan.namespace;
    let id = &t.safe;
    plan.gate_terms(trap_gate_of(plan, t))
        .iter()
        .map(|term| {
            format!(
                "execute if score #trapgate_{id} dw.sys matches 1 {} run function {ns}:trap_gate_off_{id}",
                term.clause(true)
            )
        })
        .collect()
}

/// The body of `trap_gate_init`, which `setup_finish` calls once, before the
/// first tick: put every gated trap's hardware in the state its gate says.
///
/// Arm the trap (`trap_gate_on`), then run the gate's shutting clauses — the
/// same ones the tick runs — so any term that fails in the world the campaign
/// starts in (a required flag nothing has set, a datum whose declared initial
/// fails its comparison) takes the trigger straight back out. Reading only
/// `requires_flags` here left a trap gated by a failing `requires_state` term
/// armed for the first tick.
pub(super) fn trap_gate_init(plan: &Plan, hardware: &BTreeMap<String, String>) -> Vec<String> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for t in plan.traps.iter().filter(|t| trap_is_gated(t)) {
        if !hardware.contains_key(&t.safe) {
            continue;
        }
        out.push(format!("function {ns}:trap_gate_on_{}", t.safe));
        out.extend(trap_gate_shut_clauses(plan, t));
    }
    out
}

/// The `trap_gate_on_<safe>` / `trap_gate_off_<safe>` pair per gated trap: flip the
/// sentinel and write the trigger cell. `on` restores the authored block verbatim
/// (state and all); `off` clears it to air, which is what the plate/tripwire cell
/// is when the piece does not carry one.
pub(super) fn trap_gate_fns(
    plan: &Plan,
    hardware: &BTreeMap<String, String>,
) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for t in plan.traps.iter().filter(|t| trap_is_gated(t)) {
        let id = &t.safe;
        let c = t.trigger_cell;
        let Some(block) = hardware.get(id) else {
            continue;
        };
        out.push((
            format!("trap_gate_on_{id}"),
            lines(&[
                format!("scoreboard players set #trapgate_{id} dw.sys 1"),
                format!("setblock {} {} {} {block}", c[0], c[1], c[2]),
            ]),
        ));
        out.push((
            format!("trap_gate_off_{id}"),
            lines(&[
                format!("scoreboard players set #trapgate_{id} dw.sys 0"),
                format!("setblock {} {} {} minecraft:air", c[0], c[1], c[2]),
            ]),
        ));
    }
    out
}

/// `setup_finish` commands for traps (spec-0011): fill each `dispense` trap's
/// prefab dispenser with its static payload (`item replace block … container.0`,
/// the same deterministic mechanism as a `collect` chest — no raw NBT), and summon
/// the disarm affordance's interaction entity. The trap's *harm* needs no command:
/// the plate/tripwire/trapped-chest → dispenser redstone is already in the prefab.
/// Empty for a campaign with no traps → byte-identical.
pub(super) fn trap_setup(plan: &Plan, gate_hardware: &BTreeMap<String, String>) -> Vec<String> {
    let mut out = Vec::new();
    for t in &plan.traps {
        // Fill the pre-wired dispenser with the declared payload.
        if let (Some(disp), Some((item, count))) = (t.dispenser, &t.payload) {
            out.push(format!(
                "item replace block {} {} {} container.0 with {item} {count}",
                disp[0], disp[1], disp[2]
            ));
        }
        // spec-0022: a `trapped-chest` trigger with a command payload needs a
        // detection surface, and the only player-distinct one vanilla offers is
        // the v0.4 interaction entity (`use`) — the SAME primitive the disarm
        // affordance already uses. Reading the chest's redstone output would be
        // block-power polling, which spec-0011 excluded as folklore.
        //
        // No `affordance_hardware` accompanies this one (cf. `DW0420` below):
        // the trapped chest IS the visible hardware, authored in the prefab as
        // the trap's trigger block. The invisible-lever failure that rule exists
        // to catch is an interaction with nothing to look at — not the case here.
        if !t.payload_effects.is_empty() && t.trigger == delvewright_dsl::TrapTrigger::TrappedChest
        {
            let v = ent_xyz(t.trigger_cell);
            out.push(format!(
                "summon minecraft:interaction {} {} {} {{width:1.0f,height:2.0f,response:1b,Invulnerable:1b,Tags:[{FIXTURE_NBT}\"dw_trapfire_{}\"]}}",
                v[0], v[1], v[2], t.safe
            ));
        }
        // Summon the disarm interaction affordance (a right-click target) and
        // the visible hardware that makes it findable. The prefab may ALSO dress
        // the cell, but the compiler no longer depends on that: an invisible
        // `minecraft:interaction` on its own is a lever the player cannot see
        // (the drowned-bell class, `DW0420`).
        if let Some(dis) = &t.disarm {
            let v = ent_xyz(dis.via_cell);
            out.push(format!(
                "summon minecraft:interaction {} {} {} {{width:1.0f,height:2.0f,response:1b,Invulnerable:1b,Tags:[{FIXTURE_NBT}\"dw_trapdis_{}\"]}}",
                v[0], v[1], v[2], t.safe
            ));
            out.push(affordance_hardware(
                v,
                &format!("dw_trapdis_{}", t.safe),
                "minecraft:lever",
            ));
        }
    }
    // A gated trap's hardware is seeded once, here, so there is never a tick in
    // which the trap is live before its gate has been read.
    if plan
        .traps
        .iter()
        .any(|t| trap_is_gated(t) && gate_hardware.contains_key(&t.safe))
    {
        out.push(format!("function {}:trap_gate_init", plan.namespace));
    }
    out
}

/// Per-tick disarm detection for disarmable traps (spec-0011), reusing the v0.4
/// interaction-entity `use` primitive: when a player right-clicks the disarm
/// affordance, fire the disarm once. Empty for a campaign with no disarmable traps.
pub(super) fn trap_tick(plan: &Plan) -> Vec<String> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    // spec-0022: fire the command payload when the trigger is sprung. Redstone
    // keeps exactly one job — being the visible, learnable trigger — and the
    // consequence is commands, so the compiler owns the detection tick.
    //
    // Detection reuses the two primitives already in the compiler and adds none:
    // a plate/tripwire is a POSITION test on the trigger cell (the `reach-anchor`
    // idiom), a trapped chest is the v0.4 interaction `use`. Neither reads block
    // power — that would be the polling hack spec-0011 excluded.
    out.extend(trap_fire_tick(plan));
    for t in &plan.traps {
        if t.disarm.is_none() {
            continue;
        }
        let id = &t.safe;
        out.push(format!(
            "execute unless score #trapdis_{id} dw.sys matches 1 if entity @e[tag=dw_trapdis_{id},nbt={{interaction:{{}}}}] run function {ns}:trap_disarm_{id}"
        ));
        out.push(format!(
            "execute as @e[tag=dw_trapdis_{id}] run data remove entity @s interaction"
        ));
    }
    // The gate clauses live in their own function so the generated PackTest can
    // run exactly the clauses the tick runs, and nothing else the tick does.
    if plan.traps.iter().any(trap_is_gated) {
        out.push(format!("function {ns}:trap_gate_tick"));
    }
    out
}

/// Disarm functions (`trap_disarm_<id>`) for disarmable traps (spec-0011). Firing
/// once (`#trapdis_<id>` sentinel): set the disarm flag party-wide (so
/// `requires_flags` reads elsewhere see it) and **empty the dispenser** — the
/// modeled, global disarm that actually stops a redstone-native dispense trap for
/// everyone. Empty for a campaign with no disarmable traps.
pub(super) fn trap_fns(
    plan: &Plan,
    gate_hardware: &BTreeMap<String, String>,
) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for t in &plan.traps {
        let Some(dis) = &t.disarm else {
            continue;
        };
        let id = &t.safe;
        let mut body: Vec<String> = Vec::new();
        body.push(format!("scoreboard players set #trapdis_{id} dw.sys 1"));
        body.push(format!(
            "scoreboard players set {} {} 1",
            plan::PARTY,
            plan::flag_score(&dis.sets_flag)
        ));
        if let Some(disp) = t.dispenser {
            // Empty the dispenser to an empty stack list — the modeled, global disarm
            // that actually stops a redstone-native dispense trap (no ammo → no fire).
            body.push(format!(
                "data modify block {} {} {} Items set value []",
                disp[0], disp[1], disp[2]
            ));
        }
        // The lever has been thrown: the disarm affordance is spent, so its
        // visible hardware retires with it. The ONE function allowed to do this
        // — `DW0421` fails the build if any other machinery reaches it.
        body.push(format!(
            "kill @e[tag={}]",
            crate::compiler::affordance::hardware_tag(&format!("dw_trapdis_{id}"))
        ));
        out.push((format!("trap_disarm_{id}"), lines(&body)));
    }
    out.extend(trap_payload_fns(plan));
    out.extend(trap_gate_fns(plan, gate_hardware));
    let gate_tick = trap_gate_tick(plan);
    if !gate_tick.is_empty() {
        out.push(("trap_gate_tick".to_string(), lines(&gate_tick)));
    }
    let gate_init = trap_gate_init(plan, gate_hardware);
    if !gate_init.is_empty() {
        out.push(("trap_gate_init".to_string(), lines(&gate_init)));
    }
    out
}

/// A planned `volley`: the proven per-cell geometry plus its authored cadence.
pub(super) struct VolleyEmit {
    pub(super) key: String,
    pub(super) geom: crate::compiler::nav::VolleyGeometry,
    projectile: String,
    pub(super) salvos: u32,
    interval: u32,
}

/// A planned `collapse`: the settled debris plus its authored materials.
pub(super) struct CollapseEmit {
    pub(super) key: String,
    pub(super) geom: crate::compiler::nav::CollapseGeometry,
    falling_block: String,
    then_floor: Option<String>,
}

/// Every spec-0022 payload verb in the campaign, resolved and proven. Empty for
/// a campaign that uses none, so its output stays byte-identical.
#[derive(Default)]
pub(super) struct PayloadPlans {
    pub(super) volleys: Vec<VolleyEmit>,
    pub(super) collapses: Vec<CollapseEmit>,
}

/// Resolve and PROVE every `volley` / `collapse` in the campaign against the
/// assembled world (spec-0022).
///
/// Coverage is proven by construction: [`crate::compiler::nav::plan_volley`] returns one
/// shot per standable kill-zone cell or an error naming the cell it cannot
/// reach, and the emitter writes exactly those shots. There is no path by which
/// a volley ships covering less than its declared zone.
pub(super) fn plan_payload_verbs(
    plan: &Plan,
    world: &crate::compiler::nav::World,
    blocks: &crate::compiler::blockstate::BlockMap,
) -> Result<PayloadPlans, BuildFailure> {
    let mut out = PayloadPlans::default();
    let placement = delvewright_dsl::Placement::of(plan.campaign);
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for eff in all_campaign_effects(plan.campaign) {
        let key = payload_verb_key(eff);
        if let Some((projectile, from_anchor, kill_zone, salvos, interval)) = eff.volley() {
            if !seen.insert(format!("v{key}")) {
                continue;
            }
            let label = format!("volley from `{from_anchor}` into `{}`", kill_zone.anchor);
            let from = plan
                .point_any(from_anchor.as_str())
                .ok_or_else(|| payload_anchor_failure(placement, &label, from_anchor.as_str()))?;
            let region = plan.zone_box(kill_zone).ok_or_else(|| {
                payload_anchor_failure(placement, &label, kill_zone.anchor.as_str())
            })?;
            let geom = crate::compiler::nav::plan_volley(world, from, region, &label)?;
            // The cadence is a timing read (DW0918): a body a salvo lands on can
            // walk out of the zone before the next one.
            crate::compiler::nav::check_volley_cadence(world, region, salvos, interval, &label)?;
            out.volleys.push(VolleyEmit {
                key,
                geom,
                projectile: projectile.to_string(),
                salvos,
                interval,
            });
        } else if let Some((region_anchor, falling_block, then_floor)) = eff.collapse() {
            if !seen.insert(format!("c{key}")) {
                continue;
            }
            let label = format!("collapse of `{}`", region_anchor.anchor);
            let region = plan.zone_box(region_anchor).ok_or_else(|| {
                payload_anchor_failure(placement, &label, region_anchor.anchor.as_str())
            })?;
            let geom = crate::compiler::nav::plan_collapse(world, blocks, region, &label)?;
            out.collapses.push(CollapseEmit {
                key,
                geom,
                falling_block: falling_block.to_string(),
                then_floor: then_floor.map(str::to_string),
            });
        }
    }
    // A trap is proven in its SPRUNG state (see `check_collapses`).
    let labelled: Vec<(String, crate::compiler::nav::CollapseGeometry)> = out
        .collapses
        .iter()
        .map(|c| (format!("collapse `{}`", c.key), c.geom.clone()))
        .collect();
    crate::compiler::nav::check_collapses(plan, world, &labelled)?;
    Ok(out)
}

/// The `DW0441` failure for a payload-verb anchor that no placed piece provides.
pub(super) fn payload_anchor_failure(
    placement: delvewright_dsl::Placement,
    label: &str,
    anchor: &str,
) -> BuildFailure {
    BuildFailure::Diagnostic {
        code: DW_PAYLOAD_ANCHOR_UNRESOLVED,
        message: format!(
            "{label}: anchor `{anchor}` is not provided by any placed prefab piece, so the \
             volume it centres cannot be resolved — {}",
            placement.anchor_remedy(
                "use an anchor name the prefab metadata actually exposes (anchor names come \
                 from prefab metadata; do NOT invent one)"
            ),
        ),
    }
}

/// Format a `Motion` component deterministically. Fixed precision, so the same
/// DSL and seed produce byte-identical NBT on every platform (ADR-0006).
pub(super) fn motion_component(v: f64) -> String {
    format!("{v:.6}")
}

/// The generated functions for every `volley` (spec-0022).
///
/// One start function fans out into one function per salvo — the `sequence`
/// scheduling shape, so a volley costs **nothing per tick**: no polling, no
/// clock, just `schedule` hops the server owns. Each salvo function is:
///
/// 1. the **saturation** — one projectile per standable kill-zone cell,
///    unconditional, with the compile-time velocity that reaches that cell. This
///    is the contract: the zone is blanketed, so a player inside it is hit no
///    matter which cell they are standing in, and moving between salvos does not
///    help.
/// 2. the **aimed extra** — a second projectile toward whichever cells actually
///    hold a player this tick, selected by a plain vanilla block-volume selector
///    (`@a[x=…,dx=0,…]`). Standing still therefore costs double fire, exactly as
///    spec-0022 asks, using only compile-time velocities: no runtime vector
///    arithmetic, no scoreboard math, no folklore.
///
/// Projectiles are summoned `NoGravity` so the flown path is the straight
/// segment the coverage proof checked, and `crit:0b` so damage is deterministic
/// (a random crit bonus would make the PackTest flaky).
pub(super) fn volley_fns(plan: &Plan, payloads: &PayloadPlans) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for v in &payloads.volleys {
        let base = format!("volley_{}", v.key);
        let mut start: Vec<String> = Vec::new();
        for i in 0..v.salvos {
            let at = i * v.interval;
            if at == 0 {
                start.push(format!("function {ns}:{base}_s{i}"));
            } else {
                start.push(format!("schedule function {ns}:{base}_s{i} {at}t"));
            }
        }
        out.push((base.clone(), lines(&start)));

        let src = crate::compiler::nav::volley_source(v.geom.from);
        let pos = format!(
            "{} {} {}",
            motion_component(src[0]),
            motion_component(src[1]),
            motion_component(src[2])
        );
        let mut body: Vec<String> = Vec::new();
        for shot in &v.geom.shots {
            body.push(format!(
                "summon {} {pos} {{Motion:[{}d,{}d,{}d],NoGravity:1b,crit:0b,pickup:0b}}",
                v.projectile,
                motion_component(shot.motion[0]),
                motion_component(shot.motion[1]),
                motion_component(shot.motion[2])
            ));
        }
        for shot in &v.geom.shots {
            let c = shot.cell;
            body.push(format!(
                "execute if entity @a[x={},dx=0,y={},dy=0,z={},dz=0,tag=!{CUTSCENE_TAG}] run \
                 summon {} {pos} {{Motion:[{}d,{}d,{}d],NoGravity:1b,crit:0b,pickup:0b}}",
                c[0],
                c[1],
                c[2],
                v.projectile,
                motion_component(shot.motion[0]),
                motion_component(shot.motion[1]),
                motion_component(shot.motion[2])
            ));
        }
        let salvo_body = lines(&body);
        for i in 0..v.salvos {
            out.push((format!("{base}_s{i}"), salvo_body.clone()));
        }
    }
    out
}

/// Ticks of slack allowed for debris to finish falling before `then_floor`
/// paves the landing surface. A falling block accelerates at 0.04 b/t², so this
/// is generous for any box-garden room height.
pub(super) const COLLAPSE_SETTLE_SLACK: i32 = 20;

/// The generated functions for every `collapse` (spec-0022).
///
/// Summon one `falling_block` per region cell that holds a block, then delete
/// the region. `HurtEntities` gives the impact damage; the debris pile then
/// suffocates whoever it lands on — the buried-alive beat redstone cannot
/// express at all. When `then_floor` is authored, a scheduled second function
/// paves the settled surface once the rubble has landed.
pub(super) fn collapse_fns(plan: &Plan, payloads: &PayloadPlans) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for c in &payloads.collapses {
        let base = format!("collapse_{}", c.key);
        let mut body: Vec<String> = Vec::new();
        for cell in &c.geom.drops {
            body.push(format!(
                "summon minecraft:falling_block {} {} {} \
                 {{BlockState:{{Name:\"{}\"}},Time:1,DropItem:0b,HurtEntities:1b,\
                 FallHurtMax:40,FallHurtAmount:2.0f}}",
                f64::from(cell[0]) + 0.5,
                cell[1],
                f64::from(cell[2]) + 0.5,
                c.falling_block
            ));
        }
        let (lo, hi) = c.geom.region;
        body.push(format!(
            "fill {} {} {} {} {} {} minecraft:air",
            lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]
        ));
        if c.then_floor.is_some() {
            let delay = c.geom.max_fall * 4 + COLLAPSE_SETTLE_SLACK;
            body.push(format!("schedule function {ns}:{base}_floor {delay}t"));
        }
        out.push((base.clone(), lines(&body)));
        if let Some(floor) = &c.then_floor {
            // Pave only the TOP cell of each debris column: the surface the
            // party walks on afterwards, which is what the completability proof
            // reasoned about.
            let mut tops: BTreeMap<[i32; 2], i32> = BTreeMap::new();
            for d in &c.geom.debris {
                let e = tops.entry([d[0], d[2]]).or_insert(d[1]);
                *e = (*e).max(d[1]);
            }
            let floor_body: Vec<String> = tops
                .into_iter()
                .map(|(col, y)| format!("setblock {} {y} {} {floor}", col[0], col[1]))
                .collect();
            out.push((format!("{base}_floor"), lines(&floor_body)));
        }
    }
    out
}

/// The guard clauses that must hold for a trap's command payload to fire: the
/// flag gate (when the trap declares one) and the disarm latch (when it has a
/// disarm affordance).
///
/// The disarm latch is load-bearing in a way it was not for a redstone trap:
/// emptying a dispenser stopped a `dispense` trap for everyone, but a command
/// payload has no ammunition to remove, so "disarmed" has to be read at fire
/// time or the affordance would be decorative.
pub(super) fn trap_fire_guard(t: &plan::TrapPlan) -> String {
    let mut g = String::new();
    if trap_is_gated(t) {
        g.push_str(&format!("if score #trapgate_{} dw.sys matches 1 ", t.safe));
    }
    if t.disarm.is_some() {
        g.push_str(&format!(
            "unless score #trapdis_{} dw.sys matches 1 ",
            t.safe
        ));
    }
    g
}

/// Per-tick trigger detection for traps carrying a spec-0022 command payload.
///
/// Edge-triggered on a per-trap sentinel (`#trapfire_<safe>`), so stepping onto
/// a plate fires the payload ONCE rather than every tick the player stands
/// there. A `rearm` trap clears the sentinel when the trigger cell is vacated
/// (the plate pops back up); a `once` trap never clears it, which is exactly the
/// survivability discharge `DW0342` reasons about.
///
/// A trap with no command payload emits nothing here, so every spec-0011
/// campaign stays byte-identical.
pub(super) fn trap_fire_tick(plan: &Plan) -> Vec<String> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for t in &plan.traps {
        if t.payload_effects.is_empty() {
            continue;
        }
        let id = &t.safe;
        let guard = trap_fire_guard(t);
        match t.trigger {
            delvewright_dsl::TrapTrigger::TrappedChest => {
                out.push(format!(
                    "execute unless score #trapfire_{id} dw.sys matches 1 {guard}if entity \
                     @e[tag=dw_trapfire_{id},nbt={{interaction:{{}}}}] run function \
                     {ns}:trap_fire_{id}"
                ));
                out.push(format!(
                    "execute as @e[tag=dw_trapfire_{id}] run data remove entity @s interaction"
                ));
                if matches!(t.reset, delvewright_dsl::TrapReset::Rearm) {
                    out.push(format!(
                        "execute unless entity @e[tag=dw_trapfire_{id},nbt={{interaction:{{}}}}] \
                         run scoreboard players set #trapfire_{id} dw.sys 0"
                    ));
                }
            }
            delvewright_dsl::TrapTrigger::PressurePlate
            | delvewright_dsl::TrapTrigger::Tripwire => {
                let at = step_cell_terms(t.trigger_cell);
                out.push(format!(
                    "execute unless score #trapfire_{id} dw.sys matches 1 {guard}if entity \
                     @a[{at}] run function {ns}:trap_fire_{id}"
                ));
                if matches!(t.reset, delvewright_dsl::TrapReset::Rearm) {
                    out.push(format!(
                        "execute unless entity @a[{at}] run scoreboard players set \
                         #trapfire_{id} dw.sys 0"
                    ));
                }
            }
        }
    }
    out
}

/// The `trap_fire_<id>` function per command-payload trap (spec-0022): latch the
/// sentinel, then run the authored payload bundle.
///
/// The bundle is emitted under [`Audience::Scheduled`] — there is no acting
/// player. That is the honest audience for a trap: the dungeon fires at the
/// party, not at whoever happened to touch the plate, and a `volley` salvo chain
/// re-enters under the server command source anyway (where `@s` resolves to
/// nothing). Player-facing effects therefore address `@a`, and a `carrier: "one"`
/// hand-off has no answer here — the same structural guarantee scheduled
/// sequences have.
pub(super) fn trap_payload_fns(plan: &Plan) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for t in &plan.traps {
        if t.payload_effects.is_empty() {
            continue;
        }
        let mut body = vec![format!(
            "scoreboard players set #trapfire_{} dw.sys 1",
            t.safe
        )];
        body.extend(emit_effect_bundle(
            plan,
            &t.payload_effects,
            root_audience(delvewright_dsl::EffectRootKind::TrapPayload),
        ));
        out.push((format!("trap_fire_{}", t.safe), lines(&body)));
    }
    out
}
