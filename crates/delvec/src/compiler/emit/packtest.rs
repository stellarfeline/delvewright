//! The compiler-generated PackTest suite (spec-0003): the batch model every
//! template is written against — the header, the pinned dummy, the gate drive,
//! the guards, the preamble — the ordered list of object emitters the suite is
//! assembled from, and the ordered list of per-object watch claims (`DW0811`).
//! Each object's own templates live in the module named for it.

use super::*;

mod actor;
mod assembly;
mod atmosphere;
mod boundary;
mod cast;
mod checkpoint;
mod class;
mod creator;
mod dialogue;
mod economy;
mod healthbar;
mod lane;
mod lethal;
mod lightning;
mod r#loop;
mod loot;
mod npc;
mod objective;
mod onkill;
mod quest;
mod seal;
mod sequence;
mod shortcut;
mod standin;
mod teleport;
mod pulse;
mod timed_gate;
mod trap;
mod trigger;
mod wave;
mod world;

/// Emit the compiler-generated PackTest suite (spec-0003). PackTest (misode,
/// 2.4.0 for MC 1.21.11) auto-discovers `*.mcfunction` files under
/// `data/<ns>/test/`; each is one game test driven by `# @…` directive comments,
/// with `assert`/`await`/`succeed`/`fail` commands the mod adds. Run headlessly
/// with `-Dpacktest.auto` (exit code = failed tests). These functions use
/// PackTest-only commands and run on the modded validation server, so they are
/// exempt from the vanilla command-tree validator (see `is_vanilla_function`).
pub(super) fn emit_packtest(
    plan: &Plan,
    out: &mut BuildOutput,
    moves: &[crate::compiler::nav::MovePlan],
    actor_moves: &[crate::compiler::nav::ActorMovePlan],
    waves: &WaveGeometry<'_>,
    payloads: &PayloadPlans,
    asm_locks: &crate::compiler::assembly::Locks,
) {
    let ns = &plan.namespace;
    put_json(
        out,
        "packtest-datapack/pack.mcmeta",
        &json!({
            "pack": {
                "description": format!("Delvewright PackTest suite: {ns}"),
                "min_format": PACK_FORMAT,
                "max_format": PACK_FORMAT,
            }
        }),
    );

    quest::emit_campaign_packtest(plan, out, moves, actor_moves);
    world::emit_sealed_state_packtest(plan, out);
    world::emit_declared_difficulty_packtest(plan, out);

    // spec-0082: per assembly, the body it spawns, every hit counter that rides
    // its hitbox, and its landing. Emits nothing for a campaign with none.
    assembly::emit_assembly_packtests(plan, asm_locks, out);

    // v0.3: one focused mechanism test per gameplay verb present in the campaign,
    // plus a flag-gate test. Each drives the compiler-generated mechanic functions
    // on a dummy player (no real combat / advancement events needed) and asserts
    // the objective scoreboard. Emits nothing for a v0.2 campaign.
    objective::emit_verb_packtests(plan, out);
    // spec-0080 §5.2: the server holds the biome each carried place declares,
    // and every repaint moves it — inside the volume and not outside.
    atmosphere::emit_atmosphere_packtests(plan, out);

    creator::emit_creator_packtests(plan, out);

    // The dialogue trigger must survive a second use with NO tick in between —
    // the singleplayer pause-freeze contract. Emits nothing for a campaign with no
    // terminal dialogue option.
    dialogue::emit_dialogue_trigger_packtest(plan, out);
    cast::emit_cast_packtests(plan, out);

    // v0.4: prop-on-activation, despawn removes body+hitbox, move arrives at
    // target. Emits nothing when the campaign uses none of them.
    npc::emit_v04_packtests(plan, out, moves);

    // round-8: two flag-gated click triggers on one NPC hitbox must both be
    // reachable. Emits nothing without such a pair.
    trigger::emit_shared_hitbox_packtest(plan, out);

    // The class trigger is one-shot per player. Emitted for every campaign
    // that declares a class, i.e. every campaign.
    class::emit_class_seal_packtest(plan, out);

    // A sealed gate carries the hitboxes its right-click answer rides.
    // Emits nothing for a campaign that seals no gate.
    seal::emit_seal_packtest(plan, out);

    // v0.6: boundary return / never-move-inside (spec-0013). Emits nothing without
    // a boundary.
    boundary::emit_boundary_packtest(plan, out);
    boundary::emit_night_vision_packtest(plan, out);
    lightning::emit_lightning_packtests(plan, out);
    standin::emit_standin_packtest(plan, out);

    // v0.6: checkpoint respawn contract + stealth kill/spare judge (spec-0012 /
    // spec-0014). Emits nothing when the campaign uses neither.
    checkpoint::emit_v06_packtests(plan, out);

    // spec-0031 lethal volumes: the runtime half, one template per volume.
    lethal::emit_lethal_packtests(plan, out);
    r#loop::emit_loop_packtests(plan, out);
    // spec-0102: a pulse's open edge, its cut and its re-arm. Emits nothing for
    // a campaign that declares no pulse.
    pulse::emit_pulse_packtests(plan, out);
    economy::emit_economy_packtests(plan, out);

    // spec-0031 teleport: the runtime half of TOTALITY, one template per teleport.
    teleport::emit_teleport_packtests(plan, out);
    // …and the runtime half of the fixture class (`DW0545`), one template per
    // (teleport × stake) pair — the one defect in this family that has no
    // compile-time form at all.
    teleport::emit_fixture_packtests(plan, out);

    // v0.6 (spec-0014): actor spawn/despawn (kill vs vanish), move-actor arrival,
    // unleash swap. Emits nothing for a campaign with no actors.
    actor::emit_v06_actor_packtests(plan, out, actor_moves);
    // v0.6: trap payload loads into the dispenser; a disarm empties it (spec-0011).
    // Emits nothing when the campaign declares no traps.
    trap::emit_trap_packtests(plan, out);
    // The gate's own template, independent of the dispenser one: a campaign
    // whose gated trap carries a command payload and no dispenser still owes it.
    trap::emit_trap_gate_packtest(plan, out);
    trap::emit_payload_packtests(plan, out, payloads);
    // spec-0016 §1: resting at a bonfire moves the party respawn point and
    // re-seats its `respawns_on_rest` waves. Emits nothing without a bonfire.
    // The tag census really counts the wave, and only the wave.
    wave::emit_wave_census_packtest(plan, out, waves.placements);
    checkpoint::emit_bonfire_packtests(plan, out);
    // spec-0016 §1: a re-seated wave comes back
    // STATIONED — at its lane start / anchor, in its routed state, with no trace
    // of the previous life's feral release. Emits nothing without a bonfire and
    // a `respawns_on_rest` wave.
    checkpoint::emit_reseat_stationed_packtest(plan, out, waves.placements, waves.lanes);
    // spec-0016 §1: the UNDEFEATED re-seat — an elite
    // the party is still fighting is deleted and stood up fresh on its origin;
    // one they finished stays finished. Emits nothing without a bonfire and a
    // hostile actor / billed wave.
    checkpoint::emit_reseat_undefeated_packtests(plan, out);
    // A removal the compiler performs yields nothing: the unleash and every
    // re-seat, on a body that declares a drop. Emits nothing without a bonfire
    // and a re-seated body that declares one.
    checkpoint::emit_reseat_yields_nothing_packtest(plan, out, waves.placements);
    // spec-0073: every declared health bar, driven through its real refresh.
    // Emits nothing for a campaign that declares no bar.
    healthbar::emit_health_bar_packtests(plan, out, waves.placements);
    // spec-0016 §1: rest and save-only really differ.
    checkpoint::emit_bonfire_option_packtest(plan, out);
    checkpoint::emit_bonfire_mend_packtest(plan, out);
    // spec-0016 §2: the shortcut really opens, and opens exactly once.
    shortcut::emit_shortcut_packtest(plan, out);
    // spec-0016 §4: the clock really alternates the gate region.
    timed_gate::emit_timed_gate_packtest(plan, out);
    // Per-object bodies whose family the suite claims whole (DW0811): every
    // declared wave's own kill reward, every declared objective's own activation,
    // every declared class's own apply. Each emits nothing for a campaign that
    // declares none of its mechanic.
    wave::emit_kill_reward_packtests(plan, out, waves.placements);
    // spec-0074: every fight that declares an `on_kill` — a credited kill pays,
    // an uncredited death and a compiler removal do not, and a rest tells the
    // two `fires` values apart. Emits nothing for a campaign with no bundle.
    onkill::emit_kill_pays_packtests(plan, out, waves.placements);
    objective::emit_objective_activation_packtests(plan, out);
    class::emit_class_apply_packtests(plan, out);
    npc::emit_npc_talk_packtests(plan, out);
    // Every environment trigger's own bundle, plus a presser's dispatch and
    // re-arm. Before this the presser bodies were the only per-object bodies in
    // the gallery the suite never executed at ANY depth — not driven, not even
    // reached transitively. Emits nothing for a campaign with no trigger.
    trigger::emit_env_trigger_packtests(plan, out);
    loot::emit_loot_packtest(plan, out);
    actor::emit_actor_equipment_packtest(plan, out);
    // spec-0016 §6: the patrol NBT survives 1.21.11's strict codec, the lane
    // advances in march order, the squad is released to native AI at aggro range,
    // and an aggro-edge wave really materializes on its perception ring. Emits
    // nothing for a campaign with no lane and no aggro-edge wave.
    lane::emit_td_lane_packtests(plan, out, waves.lanes, waves.rings);

    // The scheduled-executor contract (AUDIT-P0): a function reached through
    // `schedule` still lands per-player state on real players.
    sequence::emit_scheduled_executor_packtests(plan, out, moves);

    // spec-0018: one n-dummy division-of-labour test per AND-join.
    objective::emit_party_join_packtests(plan, out);
}

// ------------------------------------------- per-object watch claims (DW0811) --
//
// Every emitter that registers a claim walks a DECLARED list and writes one template per member,
// and each registers a `crate::compiler::watch::Claim` over that same authored list. The
// two halves of a claim are read from different places on purpose (see
// `crate::compiler::watch`): `declared` comes from the plan's own authored list, so a walk
// that stops at `first()` still declares every member; the driven set is read
// off the shipped suite bytes, so an emitter cannot report coverage it did not
// write. Neither half is forgeable by the defect the claim catches.
//
// What decided WHICH families get one. A family is claimable when its members'
// bodies can differ in a way a sibling's proof cannot cover — a different entity
// type, a different kit, a different dispatch arity, a different fixture at a
// different cell. Every family claimed is of that kind, established by reading the
// emitted bodies rather than by asserting it: `spawn_actor_hall_moth` summons a
// bat and `spawn_actor_rafter_spider` a spider; `class_apply_wanderer` carries a
// party-unique latch `class_apply_warder` does not; `talk_warden` dispatches one
// cast clause and `talk_marshal` four. The rule is deliberately NOT "the bodies
// differ today" — a family whose members happen to be identical modulo their id
// is one authored field away from not being, and a claim narrowed to what the
// emitter currently does is the defect this machinery exists to prevent arriving
// from inside.

/// Every per-object watch claim the suite registers, in the order the build
/// judges them.
pub(super) fn watch_claims(plan: &Plan) -> Vec<crate::compiler::watch::Claim> {
    let mut watch_claims = vec![
        timed_gate::timed_gate_watch_claim(plan),
        actor::actor_watch_claim(plan),
        wave::wave_census_watch_claim(plan),
        wave::kill_reward_watch_claim(plan),
        onkill::kill_pays_watch_claim(plan),
        objective::objective_activation_watch_claim(plan),
        class::class_apply_watch_claim(plan),
        npc::npc_talk_watch_claim(plan),
        cast::cast_ladder_watch_claim(plan),
        pulse::pulse_watch_claim(plan),
    ];
    watch_claims.extend(trigger::env_trigger_watch_claims(plan));
    watch_claims.extend(dialogue::dialogue_mask_watch_claims(plan));
    watch_claims.extend(cast::cast_bark_watch_claims(plan));
    watch_claims
}

/// One real `tick` pass with every player on the batch server shielded from harm.
///
/// A trigger template that asserts *which* trigger fired has to run the real `tick`,
/// which runs the trigger's real effects — and a delve's effects include
/// `damage-players` (the island's `his-house` deals 40, twice a dummy's health).
/// PackTest runs every generated template as one batch against one shared server, so
/// an unshielded pass would kill sibling templates' dummies for reasons that have
/// nothing to do with what they test.
///
/// Resistance V is total immunity to the `minecraft:generic` damage the effect
/// emits, and it is scaffolding around the pass, not part of any assertion: the
/// claims are all reads of `#trig_<id>`. The damage effect itself is pinned by its
/// own test.
fn shielded_tick(ns: &str) -> Vec<String> {
    vec![
        "effect give @a minecraft:resistance 1 4 true".to_string(),
        format!("function {ns}:tick"),
        "effect clear @a minecraft:resistance".to_string(),
    ]
}

/// The header lines shared by every generated PackTest (`# @dummy` + timeout).
fn packtest_header(title: &str) -> Vec<String> {
    vec![
        format!("#> {title}"),
        "# @dummy".to_string(),
        "# @timeout 100".to_string(),
        String::new(),
    ]
}

/// Pin a PackTest template's own dummy player: the pin line (`tag @p add …`)
/// plus the selector that addresses that dummy — and only it — thereafter.
///
/// PackTest runs the whole generated suite as ONE batch on one shared server:
/// each `# @dummy` test spawns its OWN dummy, all dummies coexist, and every
/// test function executes over the same server tick(s), in an order the
/// compiler does not control. Consequences for template authorship — the hard
/// rule is **every generated test is interleaving-independent: own dummy, own
/// scores, own init**:
///
/// 1. `@p` re-resolves from the test structure origin on every command — the
///    moment a template teleports its dummy to absolute campaign coordinates,
///    `@p` retargets to a NEIGHBOR test's dummy and all later writes/asserts
///    land on the wrong player (round-5 island red: `v06_stealth` read a
///    foreign dummy's grace). A template that drives per-player state must tag
///    its dummy on the first post-setup line — while its own dummy, inside its
///    own structure, is still the nearest player — and address it exclusively
///    through the tag (which, unlike `@p`, also keeps matching a dummy that
///    content effects have killed). A template PackTest executes AS its dummy
///    may use `@s` instead — the binding survives teleports.
/// 2. An `@a` write hits every test's dummy, so a sibling template can pre-set
///    state this test believes it controls (round-5 island red:
///    `verb_flag_gate`'s "withheld" flag arrived via `verb_interact`'s `@a`).
///    Templates never write `@a`-wide, and every score a template asserts on
///    is actively initialized by that template ("never set" is not 0 here).
/// 3. Fake-player scratch holders on `dw.sys` are batch-global: every template
///    suffixes its own (`#n_sidm`, `#bx_bret`, …) so no two templates share a
///    holder. Real runtime scores (`#stealth`, `#placed`, `#trig_<id>`, move
///    drivers) are deliberately shared — tests drive them and must initialize
///    them explicitly.
/// 4. Entity state is batch-global too: a sibling's residue can defeat a
///    guarded summon (round-6 island red: `v06_unleash`'s leftover twin
///    carried `dw_actor_<id>` with no puppet marker, so
///    `v06_spawn_idempotent`'s guarded spawns no-op'd and it counted 0
///    puppets), and re-running the unguarded `setup_finish` over live NPCs
///    duplicates them. A template clears every entity tag it counts on at
///    entry and leaves none of its own residue behind; each template is a
///    single atomic function, so within it nothing can be interleaved.
fn pin_dummy(tag: &str) -> (String, String) {
    (
        format!("tag @p add {tag}"),
        format!("@a[tag={tag},limit=1]"),
    )
}

/// **The one way a generated PackTest establishes a gate.** Takes a whole
/// [`Gate`] and drives every term it reads to the value that opens (`satisfy`)
/// or shuts it, across all three axes at once.
///
/// It takes the gate as ONE value on purpose. `Gate` exists because a proof that
/// reasons about gating must be written against the gate rather than against two
/// of its three fields (`crates/dsl/src/gate.rs`), and three templates had each
/// hand-rolled their own partial copy of this — `v04_strike_npc` drove none of
/// the three axes, `collect_preheld` one, `v06_shared_hitbox` two. Each worked
/// perfectly on the campaign it was written for and was a coin toss on any
/// campaign that used an axis its author had not needed.
///
/// **Why a template must do this at all**: the suite runs as ONE batch on one
/// shared server, and a gate's terms are `#party` state — batch-global. Siblings
/// write them, and the campaign-playthrough template holds them ACROSS ticks, so
/// a template that pins some terms and leaves the rest to whatever ran last is a
/// coin toss dressed as a proof. That is not hypothetical: `trigger/skip-the-label`
/// forbids `flag/hall-sealed`, the campaign template's phase-0 run completes
/// `q_far_hall` (which SETS that flag) and never clears it, and `v04_strike_npc`
/// then failed or passed purely on whether it ran before or after that — the same
/// bytes producing both verdicts. `DW0807` is the standing check.
fn packtest_gate_drive(plan: &Plan, gate: Gate<'_>, satisfy: bool) -> Vec<String> {
    let party = plan::PARTY;
    let mut p: Vec<String> = Vec::new();
    for f in gate.requires_flags {
        p.push(format!(
            "scoreboard players set {party} {} {}",
            plan::flag_score(f.as_str()),
            if satisfy { 1 } else { 0 }
        ));
    }
    // The v0.6 negative axis: actively CLEAR every forbidden flag rather than
    // trusting it to be unset. `unless … matches 1` is unset-safe at read time,
    // but a sibling that set it is not, and on a shared batch server one did.
    //
    // Cleared in BOTH directions, deliberately. With `satisfy: false` the caller
    // wants the gate shut, and shutting it through the positive axis alone keeps
    // the template varying exactly one thing — a red then names its own cause
    // instead of leaving two candidate reasons the gate was closed.
    for f in gate.forbids_flags {
        p.push(format!(
            "scoreboard players set {party} {} 0",
            plan::flag_score(f.as_str())
        ));
    }
    // The v0.10 numeric axis (spec-0031): the datum is DRIVEN to a value that
    // opens or shuts the gate, for the same reason the flags are.
    p.extend(state_drive_lines(plan, gate.requires_state, satisfy));
    p
}

/// The guard half of [`packtest_preamble`]: every progression term an
/// objective's activation gate READS, pinned to the value that opens (or, with
/// `with_flags: false`, withholds) it — quest active, `after` prerequisites, and
/// the objective's whole [`Gate`] via [`packtest_gate_drive`].
///
/// Split out because a template that must prove something about **how the item
/// reaches the player** (the v0.8 named-stack collect) cannot use the preamble's
/// own `give`: handing the plain item over first completes the objective and
/// makes the named stack's assertion vacuous.
///
/// The gate half is delegated rather than written here, because an objective is
/// not the only thing a template opens a gate on — a trigger's dispatch gate is
/// the same question about a different object class, and a copy of this loop
/// living beside each caller is what shipped the `v04_strike_npc` flake.
fn packtest_guards(plan: &Plan, quest_id: &str, o: &Objective, with_flags: bool) -> Vec<String> {
    let party = plan::PARTY;
    let mut p = vec![format!(
        "scoreboard players set {party} {} 1",
        quest_active_score(quest_id)
    )];
    for a in o.after() {
        p.push(format!(
            "scoreboard players set {party} {} 1",
            obj_score(a.as_str())
        ));
    }
    p.extend(packtest_gate_drive(plan, o.gate(), with_flags));
    p
}

/// Lines that satisfy an objective's activation guard (quest active, all `after`
/// prerequisites set, all `requires_flags` set, and any required item given to
/// `sel`). With `with_flags: false` the flags are not merely omitted but actively
/// cleared: PackTest runs the whole suite as one batch on one shared server, so
/// "never set" does not mean 0.
///
/// spec-0018: every progression term is written on the **party holder**, which is
/// the state the generated guards actually read. The holder is batch-global —
/// but each template is a single atomic mcfunction, so its baseline, its drive
/// and its assert all land inside one tick with no sibling in between (the one
/// place that stops being true is a template that `await`s, which
/// `tests/packtest_batch.rs` polices separately). Only the ITEM still goes to the
/// test's own pinned dummy.
fn packtest_preamble(
    plan: &Plan,
    quest_id: &str,
    o: &Objective,
    with_flags: bool,
    sel: &str,
) -> Vec<String> {
    let mut p = packtest_guards(plan, quest_id, o, with_flags);
    match o {
        Objective::Collect { item, count, .. } => {
            p.push(format!("give {sel} {item} {count}"));
        }
        Objective::Interact {
            requires_item: Some(it),
            ..
        } => {
            // HELD, not merely carried: the gate reads
            // `weapon.mainhand`, so the preamble must put the item THERE. `give`
            // only happened to satisfy the old inventory-wide gate because a fresh
            // dummy's first free slot is also its selected one — an accident, not a
            // guarantee, and exactly the kind of coincidence a test must not rest on.
            p.push(format!(
                "item replace entity {sel} weapon.mainhand with {it}"
            ));
        }
        _ => {}
    }
    p
}
