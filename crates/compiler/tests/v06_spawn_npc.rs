//! DSL v0.6 `deferred` NPCs + `spawn-npc` emission — the dual of `despawn-npc`.
//!
//! Owner-observed defect (island QA): a stage-2 NPC stands at its anchor from world
//! init, so a character with a scripted entrance is visible as a statue long before
//! the beat that introduces him. `deferred: true` keeps him out of `setup_finish`;
//! `spawn-npc` runs the very same summon commands when the beat fires.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

/// hello-world's `npcs` doc raised to 0.6, optionally with the keeper deferred.
fn npcs_doc(deferred: bool) -> String {
    let src = read_hw("npcs.json").replacen("\"0.2.0\"", "\"0.6.0\"", 1);
    if deferred {
        src.replace(
            "\"base_entity\": \"minecraft:villager\",",
            "\"base_entity\": \"minecraft:villager\",\n        \"deferred\": true,",
        )
    } else {
        src
    }
}

/// hello-world's `quests` doc raised to 0.6 with an `approach` trigger that fires
/// the keeper's entrance — the natural staging shape (walk in, the NPC appears).
fn quests_doc() -> String {
    r#"{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {
    "quests": [
      {
        "id": "quest/open-the-door",
        "trigger": { "type": "campaign-start" },
        "objectives": [
          { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" },
          { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit",
            "radius": 2, "after": ["obj/talk"] }
        ],
        "on_objective_complete": {
          "obj/talk": [ { "type": "open-gate", "anchor": "anchor/door" } ]
        },
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ],
    "triggers": [
      {
        "id": "trigger/entrance",
        "at": "anchor/keeper-stand",
        "on": { "on": "approach", "range": 4 },
        "once": true,
        "effects": [ { "type": "spawn-npc", "npc": "npc/keeper" } ]
      }
    ]
  }
}"#
    .to_string()
}

/// The same quests doc, with the keeper also **despawned** when the quest ends —
/// the shape that makes the compiler emit the `v04_despawn` PackTest.
fn quests_doc_with_despawn() -> String {
    quests_doc().replace(
        r#""on_complete": [ { "type": "campaign-complete" } ]"#,
        r#""on_complete": [ { "type": "despawn-npc", "npc": "npc/keeper" }, { "type": "campaign-complete" } ]"#,
    )
}

fn parse_hw(deferred: bool) -> Campaign {
    parse_hw_with(deferred, quests_doc())
}

fn parse_hw_with(deferred: bool, quests: String) -> Campaign {
    let raw = RawCampaign {
        world: read_hw("world.json"),
        npcs: npcs_doc(deferred),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests,
        dialogue: read_hw("dialogue.json"),
    };
    parse_campaign(&raw).expect("campaign parses")
}

fn build(campaign: &Campaign, prefabs: &PrefabRegistry) -> BuildOutput {
    let plan = Plan::build(campaign, prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let bytes = std::fs::read(common::prefabs_dir().join(&piece.structure_file)).unwrap();
            structures.insert(piece.structure_file.clone(), bytes);
        }
    }
    let tree = CommandTree::v1_21_11();
    emit::build(
        &plan,
        &BTreeMap::new(),
        &structures,
        &tree,
        prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
    .expect("every emitted command validates")
}

fn file(out: &BuildOutput, path: &str) -> Option<String> {
    out.iter()
        .find(|(p, _)| p.as_str() == path)
        .map(|(_, b)| String::from_utf8(b.clone()).unwrap())
}

const SETUP_FINISH: &str = "datapack/data/hello-world/function/setup_finish.mcfunction";
const SPAWN_NPC: &str = "datapack/data/hello-world/function/spawn_npc_keeper.mcfunction";
const V04_DESPAWN: &str = "packtest-datapack/data/hello-world/test/v04_despawn.mcfunction";

/// A **deferred** NPC is absent from world init and enters only via `spawn-npc`:
/// no summon in `setup_finish`, a generated `spawn_npc_<id>` carrying the body +
/// hitbox summons, and the trigger calling it.
#[test]
fn deferred_npc_leaves_world_init_and_spawns_on_effect() {
    let c = parse_hw(true);
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let out = build(&c, &prefabs);

    let setup = file(&out, SETUP_FINISH).expect("setup_finish emitted");
    assert!(
        !setup.contains("Tags:[\"dw_npc\",\"dw_npc_keeper\"]"),
        "a deferred NPC must NOT be summoned at world init:\n{setup}"
    );
    assert!(
        !setup.contains("dw_npc_keeper"),
        "neither body nor hitbox of a deferred NPC belongs in setup_finish:\n{setup}"
    );

    let spawn = file(&out, SPAWN_NPC).expect("spawn_npc_keeper emitted for a deferred NPC");
    assert!(
        spawn.contains("summon minecraft:villager")
            && spawn.contains("Tags:[\"dw_npc\",\"dw_npc_keeper\"]"),
        "spawn-npc must summon the NPC body:\n{spawn}"
    );
    assert!(
        spawn.contains("summon minecraft:interaction")
            && spawn.contains("Tags:[\"dw_npc_keeper\"]"),
        "spawn-npc must summon the interaction hitbox too:\n{spawn}"
    );
    // Idempotent, and the two guards must discriminate body from hitbox: a single
    // `unless entity @e[tag=dw_npc_keeper]` on both lines would let the body's own
    // summon suppress the hitbox.
    assert!(
        spawn.contains("execute unless entity @e[tag=dw_npc,tag=dw_npc_keeper] run summon"),
        "the body summon must be guarded on the body-only tag pair:\n{spawn}"
    );
    assert!(
        spawn.contains("execute unless entity @e[tag=dw_npc_keeper,tag=!dw_npc] run summon"),
        "the hitbox summon must be guarded on its own (non-body) selector:\n{spawn}"
    );

    let all: String = out
        .iter()
        .filter(|(p, _)| p.starts_with("datapack/") && p.ends_with(".mcfunction"))
        .map(|(_, b)| String::from_utf8(b.clone()).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        all.contains("function hello-world:spawn_npc_keeper"),
        "the spawn-npc effect must call the generated entrance function"
    );
}

/// The refactor is behaviour-preserving: a NON-deferred NPC is summoned in
/// `setup_finish` exactly as before, and no `spawn_npc_<id>` function is emitted.
#[test]
fn non_deferred_npc_is_unchanged() {
    let c = parse_hw(false);
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let out = build(&c, &prefabs);

    let setup = file(&out, SETUP_FINISH).expect("setup_finish emitted");
    assert!(
        setup.contains("summon minecraft:villager")
            && setup.contains("Tags:[\"dw_npc\",\"dw_npc_keeper\"]"),
        "a normal NPC is still summoned at world init:\n{setup}"
    );
    assert!(
        file(&out, SPAWN_NPC).is_none(),
        "no spawn_npc_<id> function is emitted when no NPC is deferred"
    );
}

/// The `v04_despawn` PackTest must stay true when its target NPC is `deferred`.
/// A deferred NPC is absent after `setup_finish`, so the unmodified test asserted
/// `#before == 2` against an empty world and failed. The test now fires the NPC's
/// generated entrance first — the presence and removal assertions are unchanged,
/// so despawn semantics are still what is being proved.
#[test]
fn despawn_packtest_spawns_a_deferred_target_first() {
    let c = parse_hw_with(true, quests_doc_with_despawn());
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let out = build(&c, &prefabs);

    let t = file(&out, V04_DESPAWN).expect("v04_despawn emitted for a despawn-npc campaign");
    let entrance = t
        .find("function hello-world:spawn_npc_keeper")
        .unwrap_or_else(|| panic!("the deferred target's entrance must run in the test:\n{t}"));
    let before = t
        .find("assert score #before dw.sys matches 2")
        .unwrap_or_else(|| panic!("the presence assertion must survive:\n{t}"));
    assert!(
        entrance < before,
        "the entrance must run BEFORE the presence assertion:\n{t}"
    );
    assert!(
        t.contains("assert score #after dw.sys matches 0"),
        "the removal assertion must survive:\n{t}"
    );
}

/// A NON-deferred despawn target keeps the pre-0.6 test verbatim: the entrance
/// line is emitted for deferred targets only, so campaigns without a deferred NPC
/// stay byte-identical (ADR-0006 determinism gate).
#[test]
fn despawn_packtest_unchanged_for_a_non_deferred_target() {
    let c = parse_hw_with(false, quests_doc_with_despawn());
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let out = build(&c, &prefabs);

    let t = file(&out, V04_DESPAWN).expect("v04_despawn emitted for a despawn-npc campaign");
    assert!(
        !t.contains("spawn_npc_"),
        "no entrance call belongs in the test when the target is not deferred:\n{t}"
    );
    assert!(
        t.contains("assert score #before dw.sys matches 2")
            && t.contains("assert score #after dw.sys matches 0"),
        "both assertions stay:\n{t}"
    );
}

/// Determinism (ADR-0006): the deferred build is byte-identical across two runs.
#[test]
fn deferred_build_is_deterministic() {
    let c = parse_hw(true);
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let a = build(&c, &prefabs);
    let b = build(&c, &prefabs);
    assert_eq!(a, b, "two builds of the same deferred campaign must match");
}
