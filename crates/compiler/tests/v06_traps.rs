//! DSL v0.6 traps (spec-0011) end-to-end. Built on hello-world with the `world`
//! and `quests` stages bumped to 0.6.0 and a `traps[]` section added. The prefab
//! library is copied and `hello-room.json` patched to expose an `anchor/trap`
//! (with a `dispenser` socket) on the walked critical path plus an `anchor/lever`
//! disarm affordance beside it — no island prefab carries trap anchors yet, so the
//! fixture builds the hardware contract locally.
//!
//! Covers: the dispenser payload fill + disarm emission, the `tnt_explodes` seal
//! (v0.6-gated), the trap PackTest, and the DW0342 completability proof (a lethal
//! forced-path trap with no discharge fails the build).

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildFailure, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{parse_campaign, validate_campaign_with};

const NS: &str = "hello-world";

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A private prefab copy whose `hello-room.json` gains an `anchor/trap` (trigger
/// cell [5,1,6] on the spawn→exit path, dispenser socket at [4,1,6]) and an
/// `anchor/lever` disarm affordance at [3,1,6].
fn patched_prefabs(name: &str, trigger_block: Option<&str>) -> PathBuf {
    let dir = tmp(name);
    common::copy_dir_all(&common::prefabs_dir(), &dir);
    let path = dir.join("hello-room.json");
    let mut meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let anchors = meta
        .get_mut("anchors")
        .and_then(|a| a.as_object_mut())
        .unwrap();
    let mut trap_anchor = serde_json::json!({ "pos": [5, 1, 6], "dispenser": [4, 1, 6] });
    // The trigger hardware the prefab wired onto the trap cell, declared with its
    // blockstate exactly as a gate anchor declares its fill `block`. Only a
    // flag-gated trap needs it (`DW0363`), so it stays optional here.
    if let Some(tb) = trigger_block {
        trap_anchor["trigger_block"] = tb.into();
    }
    anchors.insert("anchor/trap".to_string(), trap_anchor);
    anchors.insert(
        "anchor/lever".to_string(),
        serde_json::json!({ "pos": [3, 1, 6] }),
    );
    std::fs::write(&path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();
    dir
}

fn world_v06() -> serde_json::Value {
    serde_json::json!({
        "dsl_version": "0.6.0",
        "campaign_id": "hello-world",
        "stage": "world",
        "content": {
            "title": "The Keeper's Door",
            "theme": "A lonely keep at the edge of the moor.",
            "premise": "One locked door stands between you and the road home.",
            "seed": 20260729,
            "target_minutes": 5,
            "areas": [ { "id": "area/keep", "name": "The Keep", "prefab": "prefab/hello-room" } ]
        }
    })
}

fn quests_v06(trap: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "dsl_version": "0.6.0",
        "campaign_id": "hello-world",
        "stage": "quests",
        "content": {
            "quests": [ {
                "id": "quest/open-the-door",
                "trigger": { "type": "campaign-start" },
                "objectives": [
                    { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" },
                    { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit", "radius": 2, "after": ["obj/talk"] }
                ],
                "on_objective_complete": { "obj/talk": [ { "type": "open-gate", "anchor": "anchor/door" } ] },
                "on_complete": [ { "type": "campaign-complete" } ]
            } ],
            "traps": [ trap ]
        }
    })
}

/// Build a hello-world variant with one trap against the patched prefabs.
fn build_with_trap(name: &str, trap: serde_json::Value) -> Result<BuildOutput, BuildFailure> {
    build_with_trap_hw(
        name,
        trap,
        Some("minecraft:oak_pressure_plate[powered=false]"),
    )
}

/// As [`build_with_trap`], with explicit control over the `trigger_block` the
/// prefab metadata declares (`None` = the anchor declares none).
fn build_with_trap_hw(
    name: &str,
    trap: serde_json::Value,
    trigger_block: Option<&str>,
) -> Result<BuildOutput, BuildFailure> {
    let camp_dir = tmp(&format!("{name}-camp"));
    let patch = serde_json::json!({
        "documents": { "world": world_v06(), "quests": quests_v06(trap) }
    });
    common::materialize_from(&common::hello_world_dir(), &patch, &camp_dir);
    let prefabs_dir = patched_prefabs(&format!("{name}-prefabs"), trigger_block);

    let loaded = load_campaign_dir(&camp_dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("campaign parses");
    let prefabs = PrefabRegistry::load_dir(&prefabs_dir).unwrap();
    let items = FullItemRegistry::v1_21_11();
    let entities = FullEntityRegistry::v1_21_11();
    let diags = validate_campaign_with(&campaign, &items, &prefabs, &entities);
    assert!(diags.is_empty(), "must validate clean: {diags:#?}");

    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let bytes = std::fs::read(prefabs_dir.join(&piece.structure_file)).unwrap();
            structures.insert(piece.structure_file.clone(), bytes);
        }
    }
    let tree = CommandTree::v1_21_11();
    emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &tree,
        &prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
}

fn text(out: &BuildOutput, key: &str) -> String {
    String::from_utf8(
        out.get(key)
            .unwrap_or_else(|| panic!("missing {key}"))
            .clone(),
    )
    .unwrap()
}

/// A harmful dispense trap with a disarm builds clean and emits the payload fill,
/// the disarm affordance + function, the v0.6 `tnt_explodes` seal, and a PackTest.
#[test]
fn trap_dispense_and_disarm_emit_end_to_end() {
    let trap = serde_json::json!({
        "id": "trap/dart-hall",
        "at": "anchor/trap",
        "trigger": "trapped-chest",
        "effect": { "dispense": { "item": "minecraft:arrow", "count": 8 } },
        "lethality": "harmful",
        "disarm": { "via": "anchor/lever", "sets_flag": "flag/darts-off" }
    });
    let out = build_with_trap("trap-clean", trap).expect("harmful trap builds");

    // Dispenser payload fill lands in setup_finish (dispenser socket = [4,65,6]).
    let setup_finish = text(
        &out,
        &format!("datapack/data/{NS}/function/setup_finish.mcfunction"),
    );
    assert!(
        setup_finish.contains("item replace block 4 65 6 container.0 with minecraft:arrow 8"),
        "dispenser payload fill missing:\n{setup_finish}"
    );
    // Disarm interaction affordance is summoned.
    assert!(
        setup_finish.contains("dw_trapdis_dart_hall"),
        "disarm affordance summon missing:\n{setup_finish}"
    );

    // v0.6-gated TNT seal appears in setup.
    let setup = text(
        &out,
        &format!("datapack/data/{NS}/function/setup.mcfunction"),
    );
    assert!(
        setup.contains("gamerule tnt_explodes false"),
        "v0.6 tnt_explodes seal missing:\n{setup}"
    );

    // The disarm function sets the flag and empties the dispenser.
    let disarm = text(
        &out,
        &format!("datapack/data/{NS}/function/trap_disarm_dart_hall.mcfunction"),
    );
    assert!(
        disarm.contains("data modify block 4 65 6 Items set value []"),
        "disarm must empty the dispenser:\n{disarm}"
    );
    assert!(
        disarm.contains("scoreboard players set @a dw.f_darts_off 1"),
        "disarm must set the flag party-wide:\n{disarm}"
    );

    // The trap PackTest is emitted.
    assert!(
        out.contains_key(&format!(
            "packtest-datapack/data/{NS}/test/v06_trap.mcfunction"
        )),
        "trap PackTest missing"
    );

    // Every emitted trap command validates against the vendored 1.21.11 Brigadier
    // tree (the ADR-0011 cross-check CI also runs via mecha).
    let errors = emit::validate_emitted(&out, &CommandTree::v1_21_11());
    assert!(
        errors.is_empty(),
        "emitted trap commands must validate: {errors:#?}"
    );
}

/// A lethal, `rearm` trap on the forced critical path with no disarm fails the
/// build with DW0342 (soft-loop; not avoidable/survivable/disarmable).
#[test]
fn lethal_forced_trap_without_discharge_is_dw0342() {
    let trap = serde_json::json!({
        "id": "trap/dart-hall",
        "at": "anchor/trap",
        "trigger": "pressure-plate",
        "effect": { "dispense": { "item": "minecraft:arrow", "count": 8 } },
        "lethality": "lethal",
        "reset": "rearm"
    });
    match build_with_trap("trap-lethal", trap) {
        Err(BuildFailure::Diagnostic { code, .. }) => {
            assert_eq!(
                code, "DW0342",
                "expected the trap completability proof to fire"
            )
        }
        other => panic!("expected DW0342, got {other:?}"),
    }
}

/// The same lethal forced trap set to `once` is survivable → builds clean.
#[test]
fn lethal_forced_once_trap_builds() {
    let trap = serde_json::json!({
        "id": "trap/dart-hall",
        "at": "anchor/trap",
        "trigger": "pressure-plate",
        "effect": { "dispense": { "item": "minecraft:arrow", "count": 8 } },
        "lethality": "lethal",
        "reset": "once"
    });
    assert!(
        build_with_trap("trap-once", trap).is_ok(),
        "a once-shot lethal trap is survivable and must build"
    );
}

// --- flag gating: the trap is physically off while the gate is shut -----------

/// A trap gated by `forbids_flags`. The disarm produces the flag, so the flag has
/// a declared producer (`DW0172`) and the gate is drivable end-to-end.
fn gated_trap() -> serde_json::Value {
    serde_json::json!({
        "id": "trap/dart-hall",
        "at": "anchor/trap",
        "trigger": "pressure-plate",
        "effect": { "dispense": { "item": "minecraft:arrow", "count": 8 } },
        "lethality": "harmful",
        "disarm": { "via": "anchor/lever", "sets_flag": "flag/darts-off" },
        "forbids_flags": ["flag/darts-off"]
    })
}

/// `requires_flags`/`forbids_flags` on a trap were populated in the plan and
/// checked by `DW0172`, and then read by **no emission site at all** — the
/// documented "inactive while the flag is set" behaviour did not exist. It is now
/// a physical gate: the trigger block leaves the world and comes back.
#[test]
fn a_flag_gated_trap_removes_and_restores_its_trigger() {
    let out = build_with_trap("trap-gated", gated_trap()).expect("a gated trap builds");

    // The trigger cell is [5,65,6] (local [5,1,6] at base y=64).
    let on = text(
        &out,
        &format!("datapack/data/{NS}/function/trap_gate_on_dart_hall.mcfunction"),
    );
    assert!(
        on.contains("setblock 5 65 6 minecraft:oak_pressure_plate[powered=false]"),
        "opening the gate must restore the AUTHORED trigger, blockstate and all:\n{on}"
    );
    assert!(
        on.contains("scoreboard players set #trapgate_dart_hall dw.sys 1"),
        "opening must flip the hardware sentinel:\n{on}"
    );
    let off = text(
        &out,
        &format!("datapack/data/{NS}/function/trap_gate_off_dart_hall.mcfunction"),
    );
    assert!(
        off.contains("setblock 5 65 6 minecraft:air"),
        "shutting the gate must take the trigger out of the world:\n{off}"
    );

    // The tick drives both directions, keyed on the sentinel so the `setblock`
    // fires on a transition rather than every tick.
    let tick = text(
        &out,
        &format!("datapack/data/{NS}/function/tick.mcfunction"),
    );
    let shut = "execute if score #trapgate_dart_hall dw.sys matches 1 \
                if entity @a[scores={dw.f_darts_off=1..}] \
                run function hello-world:trap_gate_off_dart_hall";
    let open = "execute unless score #trapgate_dart_hall dw.sys matches 1 \
                unless entity @a[scores={dw.f_darts_off=1..}] \
                run function hello-world:trap_gate_on_dart_hall";
    for expected in [shut, open] {
        assert!(
            tick.lines().any(|l| l.trim() == expected),
            "the tick must carry `{expected}`:\n{tick}"
        );
    }

    // A `forbids_flags`-only gate starts OPEN (no flag is set at world start), so
    // setup arms the sentinel and leaves the prefab's own block alone.
    let setup_finish = text(
        &out,
        &format!("datapack/data/{NS}/function/setup_finish.mcfunction"),
    );
    assert!(
        setup_finish.contains("scoreboard players set #trapgate_dart_hall dw.sys 1"),
        "setup must seed the gate sentinel to the world it starts in:\n{setup_finish}"
    );

    // And the behaviour is asserted in-game, not just in the emitted text.
    assert!(
        out.contains_key(&format!(
            "packtest-datapack/data/{NS}/test/v06_trap_gate.mcfunction"
        )),
        "the trap flag-gate PackTest must be emitted"
    );

    let errors = emit::validate_emitted(&out, &CommandTree::v1_21_11());
    assert!(
        errors.is_empty(),
        "emitted trap-gate commands must validate: {errors:#?}"
    );
}

/// A `requires_flags` gate starts SHUT — no flag is set at world start — so setup
/// takes the trigger straight back out rather than leaving a live trap for the
/// tick to notice.
#[test]
fn a_requires_flags_gate_starts_shut() {
    let mut trap = gated_trap();
    trap["forbids_flags"] = serde_json::json!([]);
    trap["requires_flags"] = serde_json::json!(["flag/darts-off"]);
    let out = build_with_trap("trap-gated-req", trap).expect("a gated trap builds");
    let setup_finish = text(
        &out,
        &format!("datapack/data/{NS}/function/setup_finish.mcfunction"),
    );
    assert!(
        setup_finish.contains("scoreboard players set #trapgate_dart_hall dw.sys 0"),
        "a requires-gate starts shut:\n{setup_finish}"
    );
    assert!(
        setup_finish.contains("setblock 5 65 6 minecraft:air"),
        "a requires-gate must clear the trigger at setup:\n{setup_finish}"
    );
}

/// An **ungated** trap emits none of the gate machinery, so every campaign that
/// existed before this feature stays byte-identical (ADR-0006).
#[test]
fn an_ungated_trap_emits_no_gate_machinery() {
    let mut trap = gated_trap();
    trap["forbids_flags"] = serde_json::json!([]);
    let out = build_with_trap("trap-ungated", trap).expect("an ungated trap builds");
    let tick = text(
        &out,
        &format!("datapack/data/{NS}/function/tick.mcfunction"),
    );
    assert!(
        !tick.contains("trapgate_"),
        "an ungated trap must not gain a gate clause:\n{tick}"
    );
    assert!(
        !out.keys().any(|k| k.contains("trap_gate_")),
        "an ungated trap must emit no gate functions"
    );
}

/// A flag gate on a trap whose prefab declares no `trigger_block` is `DW0363`:
/// the compiler will not pretend to gate hardware it cannot name.
#[test]
fn a_gated_trap_without_declared_hardware_is_dw0363() {
    match build_with_trap_hw("trap-gated-nohw", gated_trap(), None) {
        Err(BuildFailure::Diagnostic { code, message }) => {
            assert_eq!(code, "DW0363", "message was: {message}");
            assert!(
                message.contains("trigger_block"),
                "the diagnostic must name the missing declaration: {message}"
            );
        }
        other => panic!("expected DW0363, got {other:?}"),
    }
}

/// A flag gate on a **trapped-chest** trigger is `DW0363` rather than folklore:
/// the chest is a block entity with an inventory, so removing it destroys state
/// the compiler never authored and cannot put back.
#[test]
fn a_gated_trapped_chest_is_dw0363() {
    let mut trap = gated_trap();
    trap["trigger"] = "trapped-chest".into();
    match build_with_trap_hw(
        "trap-gated-chest",
        trap,
        Some("minecraft:trapped_chest[facing=north]"),
    ) {
        Err(BuildFailure::Diagnostic { code, message }) => {
            assert_eq!(code, "DW0363", "message was: {message}");
            assert!(
                message.contains("block entity"),
                "the diagnostic must explain why the chest cannot be gated: {message}"
            );
        }
        other => panic!("expected DW0363, got {other:?}"),
    }
}
