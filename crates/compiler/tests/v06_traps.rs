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
fn patched_prefabs(name: &str) -> PathBuf {
    let dir = tmp(name);
    common::copy_dir_all(&common::prefabs_dir(), &dir);
    let path = dir.join("hello-room.json");
    let mut meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let anchors = meta
        .get_mut("anchors")
        .and_then(|a| a.as_object_mut())
        .unwrap();
    anchors.insert(
        "anchor/trap".to_string(),
        serde_json::json!({ "pos": [5, 1, 6], "dispenser": [4, 1, 6] }),
    );
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
    let camp_dir = tmp(&format!("{name}-camp"));
    let patch = serde_json::json!({
        "documents": { "world": world_v06(), "quests": quests_v06(trap) }
    });
    common::materialize_from(&common::hello_world_dir(), &patch, &camp_dir);
    let prefabs_dir = patched_prefabs(&format!("{name}-prefabs"));

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
        delvewright_compiler::light::has_night_vision(&campaign),
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
