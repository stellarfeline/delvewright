//! spec-0031 acceptance criterion 9 — **the lift, authored entirely in campaign
//! JSON**, and the exact places the v0.10 surface does not reach.
//!
//! The lift is not a feature. It is the strongest available proof that the five
//! primitives spec-0031 landed are sufficient, because it needs every one of
//! them, so the useful output of this file is not "a lift works" — it is a
//! precise, executing statement of *what is expressible and what is not*.
//!
//! ## What the fixture proves
//!
//! `tests/fixtures/lift` authors the owner's design of record with
//! **no new engine surface at all**: two party datums, one call lever per floor,
//! and the seven-step timing table as the steps of one `sequence`. Nothing in
//! the DSL, the compiler or the emission names a lift, and
//! [`no_engine_surface_names_a_lift`] is what keeps that true.
//!
//! ## What it does NOT prove, stated rather than left to be discovered
//!
//! Three parts of the owner's design are **not authorable today**, and each has
//! a test below that fails the moment it becomes authorable — which is the only
//! honest way to record a gap that a future reader might otherwise mistake for
//! an oversight in the fixture.
//!
//! 1. [`the_in_car_lever_is_refused`] — the lever *inside* the car. Any
//!    interaction affordance inside the car volume is `DW0542`, and the refusal
//!    is CORRECT: `teleport` moves entities and not blocks, and its `to` is a
//!    point, so an affordance riding the car would be torn off its lever and
//!    stacked on the destination anchor. The owner's design needs exactly one
//!    such affordance ("go to the other floor"), so **a car cannot be commanded
//!    from inside it**. The fixture therefore carries one call lever per floor
//!    and no in-car control.
//! 2. [`a_runtime_region_cannot_name_the_cell_under_a_rider`] — the car's deck.
//!    Every runtime region in the DSL is `StealthZone { anchor, extent }`: a box
//!    **centred** on a prefab anchor, with unsigned half-extents. A car needs a
//!    deck one block below the cell its riders stand in, an arrival cell one
//!    block above that deck, and a lethal volume one block below the ground-floor
//!    deck — three boxes at fixed OFFSETS from a named cell, and an offset is the
//!    one thing this type cannot express. Stage 7 already carries the general
//!    region language (`box { min, max }` in a `piece-local` or `anchor-relative`
//!    frame, plus `union` / `intersect` / `subtract`); stage 5 cannot see it.
//! 3. [`the_car_always_exists_invariant_is_unenforced`] — "create before clear,
//!    never the reverse". The fixture obeys it; nothing checks it, and a campaign
//!    that clears its only car before filling the next one compiles green.
//!
//! ## A fourth finding, paid for on a real server
//!
//! The first draft of this fixture put BOTH critical-path objectives on `spawn`
//! — hello-room has four anchors, two are car stations whose cells a
//! `fill-region` makes solid (so `DW0314` forbids routing through them) and one
//! is the upper call lever, which left exactly one. `join_place` teleports every
//! joining player onto the spawn cell, and both objectives' completion boxes
//! contain it, so **the world tick completed the whole campaign with no player
//! doing anything** — for every dummy of every sibling PackTest test.
//!
//! The generated `campaign` template then reds, and the reason is worth keeping:
//! it re-drives `complete_o_<obj>` after resetting only `dw.campaign` and the
//! first quest's `dw.qa_<q>`, while the chain it drives is latched by
//! `dw.q_<q>` (`check_q_<q>` carries `unless score #party dw.q_<q> matches 1`).
//! Once the tick has set those, the replay is a no-op and the assert fails **on
//! tick 0** — which is what CI reported, and which a local run can equally well
//! *pass*, because whether `campaign` executes before any dummy has been placed
//! and ticked is batch order the compiler does not control. Byte-identical
//! packs, pass/fail decided by order: the `v06_spawn_idempotent` class named in
//! `docs/reference/compiler.md` §"PackTest batch model", arriving from a new
//! direction.
//!
//! Two things are wrong there and only one is fixed here.
//!
//! * **The fixture was wrong**, and is fixed: a delve that finishes itself while
//!   the party stands still is not a lift proof. The finale objective is now
//!   gated on `flag/rode`, which only a ride sets, so the tick cannot reach it.
//!   [`the_finale_cannot_be_completed_by_standing_still`] is what holds that,
//!   and it is a statement about the EMISSION rather than about one server run —
//!   a green suite cannot prove order-independence, it can only fail to disprove
//!   it.
//! * **The generated `campaign` template is also wrong**, generally, and is NOT
//!   fixed here: it does not meet the batch model's own "own init" rule, because
//!   the scores its drive depends on are not the scores it initializes. Any
//!   campaign whose quests can advance by any other route makes it a coin flip.
//!   The fix is one line in its preamble (clear `dw.o_*` / `dw.q_*` for the
//!   quests it re-drives) and it belongs to its own round: it is an `emit.rs`
//!   change that moves `packtest-datapack/` bytes for every campaign, and this
//!   PR's whole claim is that it changes no engine source at all.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{parse_campaign, validate_campaign_with};

/// The fixture's namespace — the emitted function prefix.
const NS: &str = "lift";

/// A private working directory per CALLER: the tests run in parallel threads of
/// one binary, and a shared scratch directory is a race whose symptom is a
/// missing file (an intermittent red, which is a finding, not a re-run).
fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Materialize the `lift` fixture at a private path, optionally rewriting
/// `quests.json` first. `Err` carries the build diagnostic.
fn try_build(
    who: &str,
    patch: impl FnOnce(&mut serde_json::Value),
) -> Result<BuildOutput, (String, String)> {
    let src = common::compiler_fixtures_dir().join("lift");
    let dir = tmp(&format!("v10-lift-{who}"));
    for f in common::STAGE_FILES {
        std::fs::copy(src.join(f), dir.join(f)).unwrap();
    }
    let mut quests: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("quests.json")).unwrap()).unwrap();
    patch(&mut quests);
    std::fs::write(
        dir.join("quests.json"),
        serde_json::to_string_pretty(&quests).unwrap(),
    )
    .unwrap();

    let prefab_dir = common::prefabs_dir();
    let loaded = load_campaign_dir(&dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("the lift fixture parses");
    let prefabs = PrefabRegistry::load_dir(&prefab_dir).unwrap();
    let items = FullItemRegistry::v1_21_11();
    let entities = FullEntityRegistry::v1_21_11();
    let diags = validate_campaign_with(&campaign, &items, &prefabs, &entities);
    if let Some(d) = diags
        .iter()
        .find(|d| d.severity == delvewright_dsl::Severity::Error)
    {
        return Err((d.code.to_string(), d.message.clone()));
    }

    let plan = Plan::build(&campaign, &prefabs).expect("the lift plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(prefab_dir.join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
    .map_err(|e| match e {
        emit::BuildFailure::Diagnostic { code, message } => (code.to_string(), message),
        other => panic!("expected a diagnostic, got {other:?}"),
    })
}

fn build(who: &str) -> BuildOutput {
    try_build(who, |_| {}).expect("the lift fixture builds clean")
}

/// One emitted function's body.
fn func(out: &BuildOutput, name: &str) -> String {
    let path = format!("datapack/data/{NS}/function/{name}.mcfunction");
    String::from_utf8_lossy(
        out.get(&path)
            .unwrap_or_else(|| panic!("no emitted function `{name}`")),
    )
    .to_string()
}

/// The two emitted `seq_<hash>` entry-point names, in emission order.
fn sequence_roots(out: &BuildOutput) -> Vec<String> {
    let mut v: Vec<String> = out
        .keys()
        .filter_map(|p| {
            let n = p
                .strip_prefix(&format!("datapack/data/{NS}/function/"))?
                .strip_suffix(".mcfunction")?;
            (n.starts_with("seq_") && !n.rsplit('_').next().is_some_and(|t| t.len() == 1))
                .then(|| n.to_string())
        })
        .collect();
    v.sort();
    v
}

// ---------------------------------------------------------------- criterion 9 --

/// **Criterion 9, first half: no verb names a lift.**
///
/// The claim is about the DSL **surface**, so the thing examined is the surface
/// itself: every name in the generated JSON Schema of all seven stages — type
/// names, property names, `required` entries, string `enum` members and the
/// `const` tags that spell a verb. Derived from the Rust types by `schemars`, so
/// the enumeration is complete by construction rather than by diligence: a
/// `QuestEffect::Lift`, a `lift` field or a `lift_car` id prefix cannot exist
/// without appearing here.
///
/// Prose is deliberately NOT searched. `schemars` copies doc comments into
/// `description`, and the word belongs there — spec-0031's own worked example is
/// a lift, `lethal_volumes[]` cites "the bottom of a lift shaft", and a rule that
/// forbade the word in prose would forbid explaining the decision. A description
/// cannot become a surface; a name can.
///
/// Binding is stated: the number of names examined, asserted non-zero, because a
/// walk that found no names would pass forever having proven nothing.
#[test]
fn no_engine_surface_names_a_lift() {
    use delvewright_dsl::envelope::Stage;
    const STAGES: [Stage; 7] = [
        Stage::World,
        Stage::Npcs,
        Stage::Classes,
        Stage::QuestPlan,
        Stage::Quests,
        Stage::Dialogue,
        Stage::WorldEdits,
    ];

    // The matcher itself is bound first: a gate whose predicate never matches is
    // green over any surface at all, which is the vacuity this project keeps
    // being bitten by.
    for yes in [
        "lift", "Lift", "lift_car", "LiftPlan", "car-lift", "lift/car",
    ] {
        assert!(word_lift(yes), "`{yes}` must be seen as naming a lift");
    }
    for no in ["lifted", "uplift", "shoplift", "cliff", "shortcut"] {
        assert!(!word_lift(no), "`{no}` is not the word");
    }

    let mut names: Vec<String> = Vec::new();
    for stage in STAGES {
        collect_schema_names(&delvewright_dsl::schema::stage_schema(stage), &mut names);
    }
    assert!(
        names.len() > 500,
        "this gate examined only {} schema names, which is far too few to be the whole DSL \
         surface — it has stopped binding to anything",
        names.len()
    );
    let hits: Vec<&String> = names.iter().filter(|n| word_lift(n)).collect();
    assert!(
        hits.is_empty(),
        "no DSL surface may be named after a lift — a lift is a `sequence` of general \
         primitives and nothing else (spec-0031 §\"The worked example, which is deliberately \
         NOT a verb\"). Examined {} names across all seven stage schemas and found: {hits:?}",
        names.len()
    );
}

/// Every NAME a schema declares: `$defs` keys, `properties` keys, `required`
/// entries, string `enum` members and `const` values (the tag that spells a
/// verb). Deliberately not `description`, `title` or `$comment` — see the caller.
fn collect_schema_names(v: &serde_json::Value, out: &mut Vec<String>) {
    match v {
        serde_json::Value::Object(map) => {
            for (k, child) in map {
                match k.as_str() {
                    "$defs" | "definitions" | "properties" | "patternProperties" => {
                        if let Some(o) = child.as_object() {
                            out.extend(o.keys().cloned());
                        }
                    }
                    "required" => {
                        if let Some(a) = child.as_array() {
                            out.extend(a.iter().filter_map(|x| x.as_str()).map(String::from));
                        }
                    }
                    "enum" => {
                        if let Some(a) = child.as_array() {
                            out.extend(a.iter().filter_map(|x| x.as_str()).map(String::from));
                        }
                    }
                    "const" => {
                        if let Some(s) = child.as_str() {
                            out.push(s.to_string());
                        }
                    }
                    // Prose: never a surface, and the one place the word belongs.
                    "description" | "title" | "$comment" | "examples" | "default" => continue,
                    _ => {}
                }
                collect_schema_names(child, out);
            }
        }
        serde_json::Value::Array(a) => a.iter().for_each(|c| collect_schema_names(c, out)),
        _ => {}
    }
}

/// The word `lift` as a WORD inside a NAME, in any of the three casings the
/// engine's names use: `snake_case`, `kebab-case`, `PascalCase`. A boundary is a
/// non-alphanumeric neighbour **or** a case transition, which is what makes
/// `LiftPlan` a hit while `lifted`, `uplift` and `shoplift` are not.
fn word_lift(s: &str) -> bool {
    let b = s.as_bytes();
    let lower = s.to_ascii_lowercase();
    let mut from = 0usize;
    while let Some(rel) = lower[from..].find("lift") {
        let i = from + rel;
        let starts_word = i == 0 || !b[i - 1].is_ascii_alphanumeric() || b[i].is_ascii_uppercase();
        let j = i + 4;
        let ends_word = j >= b.len() || !b[j].is_ascii_alphanumeric() || b[j].is_ascii_uppercase();
        if starts_word && ends_word {
            return true;
        }
        from = j;
    }
    false
}

/// **The owner's timing table, emitted step for step from ONE `sequence`.**
///
/// | step | tick | what happens |
/// |---|---|---|
/// | 1 | 0 | gate: the car is not already at the destination, and no ride is in progress |
/// | 2 | 0 | set `ride_in_progress` |
/// | 3 | 0 | grant blindness for the whole sequence plus slack |
/// | 4 | 1 | fill the destination car region |
/// | 5 | 2 | teleport every player and entity inside the old car volume to the new one |
/// | 6 | 3 | clear the old car region |
/// | 7 | 4 | set `car_at_floor`; clear `ride_in_progress` |
///
/// Steps 1–3 and 7 are read off the emission here; the gate (step 1) is
/// [`the_call_gate_is_the_shared_gate_on_the_tick_line`].
#[test]
fn the_owners_timing_table_is_one_sequence() {
    let out = build("timing");
    let roots = sequence_roots(&out);
    assert_eq!(
        roots.len(),
        2,
        "one sequence per direction and nothing else: {roots:?}"
    );

    for root in &roots {
        let entry = func(&out, root);
        // The whole ride is ONE bundle: step 0 inline, four scheduled steps.
        let scheduled: Vec<&str> = entry
            .lines()
            .filter(|l| l.starts_with("schedule function"))
            .collect();
        assert_eq!(
            scheduled.len(),
            4,
            "the timing table has five steps — one at tick 0 and four scheduled:\n{entry}"
        );
        for (i, tick) in [(1usize, "1t"), (2, "2t"), (3, "3t"), (4, "4t")] {
            assert!(
                scheduled[i - 1].ends_with(&format!("{root}_{i} {tick}")),
                "step {i} must run at {tick}: {}",
                scheduled[i - 1]
            );
        }

        // Step 2 + 3 (tick 0): the ride latch, then blindness with a DURATION —
        // never a later `clear-effect` (DW0540 is what makes that inexpressible).
        let s0 = func(&out, &format!("{root}_0"));
        assert!(
            s0.contains("scoreboard players set #party dw.s_ride_in_progress 1"),
            "tick 0 must set `ride_in_progress`:\n{s0}"
        );
        assert!(
            s0.contains("minecraft:blindness 1 0 true") && s0.contains("effect give @a[x="),
            "tick 0 must grant blindness over the car volume, with a duration:\n{s0}"
        );
        assert!(
            !s0.contains("effect clear"),
            "a grant whose removal is a later effect is the hazard `give-effect` was shaped \
             to make inexpressible:\n{s0}"
        );

        // Step 4 (tick 1) fills the destination; step 6 (tick 3) clears the
        // source. The ORDER is the car-always-exists invariant.
        let s1 = func(&out, &format!("{root}_1"));
        let s3 = func(&out, &format!("{root}_3"));
        assert!(
            s1.starts_with("fill ") && !s1.contains("minecraft:air"),
            "tick 1 must FILL the destination car region:\n{s1}"
        );
        assert!(
            s3.starts_with("fill ") && s3.contains("minecraft:air"),
            "tick 3 must CLEAR the source car region:\n{s3}"
        );

        // Step 5 (tick 2): the move, through the campaign's own named function.
        let s2 = func(&out, &format!("{root}_2"));
        assert!(
            s2.trim().starts_with(&format!("function {NS}:teleport_")),
            "tick 2 must be the region teleport:\n{s2}"
        );

        // Step 7 (tick 4): the position is recorded and the latch drops.
        let s4 = func(&out, &format!("{root}_4"));
        assert!(
            s4.contains("scoreboard players set #party dw.s_car_at_floor")
                && s4.contains("scoreboard players set #party dw.s_ride_in_progress 0"),
            "tick 4 must record the car's floor and clear the ride latch:\n{s4}"
        );
    }
}

/// **The invariant: the car always exists somewhere.** Create before clear.
///
/// Read off the emitted tick offsets rather than off the JSON, because the
/// property is about what the SERVER does: there must be no tick at which a save
/// could be loaded with no car.
#[test]
fn the_car_exists_at_every_tick_of_the_ride() {
    let out = build("invariant");
    for root in sequence_roots(&out) {
        let entry = func(&out, &root);
        let tick_of = |suffix: &str| -> usize {
            entry
                .lines()
                .find(|l| l.contains(&format!("{root}_{suffix} ")))
                .and_then(|l| l.rsplit(' ').next())
                .and_then(|t| t.trim_end_matches('t').parse().ok())
                .unwrap_or_else(|| panic!("no scheduled step {suffix} in\n{entry}"))
        };
        let fill = tick_of("1");
        let clear = tick_of("3");
        assert!(
            fill < clear,
            "the destination car must exist BEFORE the source car is cleared — otherwise \
             there is a tick at which a save could be loaded with no car at all. \
             fill@{fill}t clear@{clear}t"
        );
    }
}

/// **Both planner rulings hold on the emission, with no surface of their own.**
///
/// * *Pulling a call lever at the floor the car already occupies is a no-op* —
///   the `not-equals` term of the shared gate, spliced into the tick line the
///   trigger already had. Not a destroy-and-recreate, which would drop the
///   occupants for a tick.
/// * *A pull during a ride is ignored, not queued* — the `ride_in_progress`
///   term closes the gate, and the interaction record is removed on the SAME
///   tick regardless of whether the gate opened. That second line is what makes
///   it "ignored" rather than "deferred": nothing survives to be replayed.
#[test]
fn the_call_gate_is_the_shared_gate_on_the_tick_line() {
    let out = build("gate");
    let tick = func(&out, "tick");
    for (trig, floor) in [("call_lower", 1), ("call_upper", 2)] {
        let dispatch = tick
            .lines()
            .find(|l| l.contains(&format!("run function {NS}:trig_{trig}")))
            .unwrap_or_else(|| panic!("no dispatch line for `{trig}`:\n{tick}"));
        assert!(
            dispatch.contains(&format!(
                "unless score #party dw.s_car_at_floor matches {floor}"
            )),
            "a call at the floor the car already occupies must be a NO-OP: {dispatch}"
        );
        assert!(
            dispatch.contains("if score #party dw.s_ride_in_progress matches 0"),
            "a pull during a ride must not open the gate: {dispatch}"
        );
        assert!(
            tick.lines().any(|l| l
                == format!(
                    "execute as @e[tag=dw_trig_{trig}] run data remove entity @s interaction"
                )),
            "the interaction record must be discarded UNCONDITIONALLY — that is what makes a \
             pull during a ride *ignored* rather than queued:\n{tick}"
        );
    }
}

/// The teleport ledger binds: two rides, two resolved volumes, two runtime
/// templates. A zero anywhere here is a finding, not a pass.
#[test]
fn the_lift_binds_the_teleport_ledger() {
    let out = build("ledger");
    let gate: serde_json::Value =
        serde_json::from_slice(out.get("validation/teleport-gate.json").unwrap()).unwrap();
    assert_eq!(gate["teleports"]["declared"], 2);
    assert_eq!(gate["teleports"]["resolved"], 2);
    assert_eq!(gate["packtest_templates"], 2);
    assert_eq!(gate["unbound"], false);
    assert!(
        gate["affordances_examined"].as_u64().unwrap() >= 2,
        "the two call levers are affordances and must have been examined: {gate}"
    );
}

/// **The finale is not reachable by standing still** — the fix for the fourth
/// finding, asserted on the emission rather than on a server run.
///
/// The hazard is concrete: `join_place` teleports every joining player onto the
/// spawn cell, and a `reach-anchor` objective completes from a box around its
/// anchor on the ordinary world tick. With the whole critical path on `spawn`,
/// the campaign finished itself for every PackTest dummy in the batch, and the
/// generated `campaign` template — which re-drives the chain but never clears
/// the `dw.q_<q>` latch `check_q_<q>` reads — then asserted against a campaign
/// that had already run. It passed or failed by batch order.
///
/// So the finale carries a flag no idle world can set. Three things make that
/// real, and all three are read off the emitted functions:
/// 1. the tick's completion line for the finale objective is guarded by the flag;
/// 2. nothing in `tick` writes that flag; and
/// 3. the only writers are the ride sequences' last step — i.e. the flag costs a
///    right-click on a lever, which no dummy performs.
#[test]
fn the_finale_cannot_be_completed_by_standing_still() {
    let out = build("standing-still");
    let tick = func(&out, "tick");

    let line = tick
        .lines()
        .find(|l| l.contains(&format!("run function {NS}:complete_o_return")))
        .unwrap_or_else(|| panic!("no finale completion line on the tick:\n{tick}"));
    assert!(
        line.contains("if score #party dw.f_rode matches 1"),
        "the finale must cost a ride: `join_place` puts every joining player on the spawn \
         cell, and an unguarded `reach-anchor` there completes on the ordinary tick with \
         nobody doing anything — which finishes the delve, and makes the generated \
         `campaign` template a batch-order coin flip. Emitted: {line}"
    );
    assert!(
        !tick.contains("dw.f_rode 1"),
        "…and the tick itself must not be able to set it:\n{tick}"
    );

    // Bound, not assumed: the flag really is written, and only where a player's
    // right-click can reach.
    let writers: Vec<String> = out
        .keys()
        .filter(|p| p.starts_with(&format!("datapack/data/{NS}/function/")))
        .filter(|p| {
            String::from_utf8_lossy(&out[*p]).contains("scoreboard players set #party dw.f_rode 1")
        })
        .map(|p| {
            p.rsplit('/')
                .next()
                .unwrap()
                .trim_end_matches(".mcfunction")
                .to_string()
        })
        .collect();
    assert_eq!(
        writers.len(),
        2,
        "exactly one writer per ride — a flag nothing writes would make the finale \
         unreachable, which is the opposite failure: {writers:?}"
    );
    for w in &writers {
        assert!(
            w.starts_with("seq_") && w.ends_with("_4"),
            "the flag must be set by the ride's last step and nowhere else: {writers:?}"
        );
    }
}

// ------------------------------------------------- what is NOT expressible --

/// **FINDING 1 — the lever inside the car cannot be authored.**
///
/// The owner's design of record gives the car one control of its own: "one lever
/// inside the car, part of the car's own blocks, therefore created and destroyed
/// with it". Expressed the only way the DSL offers — a `use` trigger at an
/// anchor inside the car — it is `DW0542`, and **the refusal is correct**:
///
/// * `teleport` moves entities and leaves blocks, so the affordance would ride
///   away from its lever; and
/// * `teleport`'s `to` is a **point**, so even a lever whose block travelled with
///   the car would have its hitbox stacked on the destination anchor rather than
///   on the lever.
///
/// The consequence is not cosmetic: with a call lever per floor meaning "the car
/// comes to this floor", a rider standing in the car has no way to say "go to the
/// other floor", so **the car cannot be commanded from inside it**. What the
/// fixture ships instead is the call half only. Closing this needs an engine
/// decision about affordances that belong to runtime-written blocks — it is NOT
/// a field on a trigger, and it is not a type exemption in the teleport's
/// selector (that would tear an NPC's dialogue hitbox off its body).
///
/// This test fails the day the gap closes, which is the point: the finding is
/// recorded as something that executes, not as a comment.
#[test]
fn the_in_car_lever_is_refused() {
    let (code, message) = try_build("incar", |q| {
        let triggers = q["content"]["triggers"].as_array_mut().unwrap();
        triggers.push(serde_json::json!({
            "id": "trigger/car-lever",
            "at": "anchor/keeper-stand",
            "on": { "on": "use" },
            "once": false,
            "requires_state": [
                { "state": "state/car-at-floor", "op": "equals", "value": 1 },
                { "state": "state/ride-in-progress", "op": "equals", "value": 0 }
            ],
            "effects": [ { "type": "narrate", "text": "The cage lurches." } ]
        }));
    })
    .expect_err(
        "the owner's in-car lever must still be refused; if this now builds, FINDING 1 of \
         spec-0031 criterion 9 has been closed and this test is the thing to update",
    );
    assert_eq!(code, "DW0542", "{message}");
    assert!(
        message.contains("trigger/car-lever"),
        "the diagnostic must name the affordance it refused: {message}"
    );
}

/// **FINDING 2 — a runtime region cannot name the cell under (or over) a rider.**
///
/// Every runtime region in the DSL is one type: `StealthZone { anchor, extent }`
/// — a box **centred** on a prefab anchor with unsigned half-extents, shared by
/// `begin-stealth`, `damage-players`'s `in`, `give-effect`'s `in`, `collapse`,
/// `fill-region` / `clear-region`, `teleport`'s `from` and `lethal_volumes[]`.
/// Sharing one type is right; what it cannot say is an **offset**.
///
/// A lift car needs three boxes at fixed offsets from the cell a rider stands in:
/// the deck one below, the arrival cell (the deck's top), and the shaft-bottom
/// lethal volume below the ground-floor deck. None of them is nameable unless the
/// PREFAB happens to ship an anchor at each cell — so the geometry of a lift is
/// authored in NBT, not in campaign JSON, and no prefab in the library ships it.
///
/// The general region language already exists one stage away: stage 7's
/// `select` takes `box { min, max }` in a `piece-local` or `anchor-relative`
/// frame plus `union` / `intersect` / `subtract`. Stage 5 cannot see it. This is
/// CLAUDE.md's third defect shape — *the general mechanism exists but its binding
/// is too narrow to reach the objects it should* — and the fix is not a fourth
/// mechanism.
///
/// Asserted **from the generated schema**, so it is a property of the type rather
/// than of anyone's diligence, and it reds the moment a runtime region grows a
/// way to say where it is.
#[test]
fn a_runtime_region_cannot_name_the_cell_under_a_rider() {
    let schema = delvewright_dsl::schema::stage_schema(delvewright_dsl::envelope::Stage::Quests);
    let defs = schema["$defs"].as_object().expect("$defs");
    let zone = defs
        .get("StealthZone")
        .expect("the one runtime region type")
        .as_object()
        .unwrap();
    let props: Vec<&str> = zone["properties"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        props,
        ["anchor", "extent"],
        "FINDING 2 of spec-0031 criterion 9: a runtime region is an anchor-centred box and \
         nothing else. If this list has grown a way to OFFSET a box from its anchor — or to \
         give it explicit corners, the way stage 7's `select` already can — the lift's deck, \
         its arrival cell and its shaft-bottom lethal volume have become authorable and this \
         test is the thing to update."
    );
    // …and every runtime region really is that one type, so the finding is about
    // the engine and not about one verb.
    let region_carriers = [
        (
            "QuestEffect",
            "fill-region / clear-region / teleport / give-effect",
        ),
        ("LethalVolume", "lethal_volumes[]"),
    ];
    for (name, what) in region_carriers {
        let text = serde_json::to_string(defs.get(name).unwrap()).unwrap();
        assert!(
            text.contains("StealthZone"),
            "{what} must reach its box through the ONE region type, never a private twin"
        );
    }
}

/// **FINDING 3 — "the car always exists somewhere" is authored, not enforced.**
///
/// [`the_car_exists_at_every_tick_of_the_ride`] asserts the fixture obeys it.
/// Nothing in the compiler does: a sequence that clears its only car before it
/// fills the next one compiles green, and the delve ships with a tick at which a
/// save could be loaded with no car — and with whoever was standing on it in the
/// air over the shaft.
///
/// Recorded here rather than fixed, because the general form is not obvious: the
/// compiler cannot know which region writes are "the same object at two places",
/// and a rule that guessed would be a lift-shaped rule wearing a diagnostic's
/// clothes. What it CAN see is stated in the assertion's message, for whoever
/// takes it up.
#[test]
fn the_car_always_exists_invariant_is_unenforced() {
    let out = try_build("no-car", |q| {
        // Swap the two ticks: clear the source at 1, fill the destination at 3.
        for t in q["content"]["triggers"].as_array_mut().unwrap() {
            let steps = t["effects"][0]["steps"].as_array_mut().unwrap();
            let fill = steps[1]["effects"].clone();
            let clear = steps[3]["effects"].clone();
            steps[1]["effects"] = clear;
            steps[3]["effects"] = fill;
        }
    })
    .expect(
        "FINDING 3: nothing refuses a ride that destroys its only car before creating the \
         next one. If this now FAILS to build, the invariant has grown a proof and this \
         test is the thing to update — note the code it fires so the catalog can be read.",
    );
    // Bound, not assumed: the swapped build really did emit the broken order.
    for root in sequence_roots(&out) {
        let entry = func(&out, &root);
        let tick_of = |s: &str| -> usize {
            entry
                .lines()
                .find(|l| l.contains(&format!("{root}_{s} ")))
                .and_then(|l| l.rsplit(' ').next())
                .and_then(|t| t.trim_end_matches('t').parse().ok())
                .unwrap()
        };
        let clear_at = tick_of("1");
        let fill_at = tick_of("3");
        assert!(
            clear_at < fill_at,
            "this test only means something if the mutation really inverted the order"
        );
    }
}
