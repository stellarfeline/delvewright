//! Walker→NPC handoff + deferred-NPC strike triggers (round-6 island QA).
//!
//! The island's sealed beat: a puppet walker crosses the hall, vanishes on
//! arrival, and the real (dialogue-bearing) NPC spawns in its place — with the
//! gate boulder already down. Two engine invariants own that beat:
//!
//! 1. A `strike` trigger anchored on a **deferred** NPC's stand cell summons no
//!    interaction entity of its own at world init: the NPC's entrance hitbox is
//!    the trigger's sole carrier. (The round-6 soft-lock: the world-init
//!    standalone sat exactly on the entrance hitbox and won the client's
//!    right-click tie-break, so the dialogue advancement never fired.)
//! 2. The generated `v06_arrive_handoff` PackTest drives the arrival tick with
//!    every campaign gate sealed and asserts puppet-out / NPC-in (body + exactly
//!    one hitbox).

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{RawCampaign, parse_campaign};

const NS: &str = "hello-world";

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

/// hello-world's `npcs` doc raised to 0.6 with the keeper deferred — the island
/// shape: the NPC exists only after its scripted entrance.
fn npcs_deferred() -> String {
    read_hw("npcs.json")
        .replacen("\"0.2.0\"", "\"0.6.0\"", 1)
        .replace(
            "\"base_entity\": \"minecraft:villager\",",
            "\"base_entity\": \"minecraft:villager\",\n        \"deferred\": true,",
        )
}

/// A 0.6 quests doc staging the island beat in miniature: an approach trigger
/// spawns a puppet at `anchor/exit` and walks it to the keeper's stand; **on
/// arrival** the door gate seals behind it and the puppet hands off to the
/// (deferred) keeper. A `strike` trigger sits on the keeper's stand cell.
///
/// The seal fires on arrival, not ahead of the walk, because that is the order
/// the beat physically has: the walker crosses the threshold and *then* the
/// boulder comes down behind it ("point of no return by geometry"). Sealing
/// first and walking across afterwards is the round-8 island defect — the
/// puppet's tp chain drives it straight through solid blocks — and is now a
/// build error (`DW0410`, `compiler::timeline`). The PackTest below still drives
/// the arrival with every campaign gate filled: what has to be immune to sealed
/// terrain is the **arrival machinery**, which is a tp chain and not
/// pathfinding. What may not be routed across a seal is the compiler's *plan*.
fn quests_doc(on_arrive: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {{
    "quests": [
      {{
        "id": "quest/open-the-door",
        "trigger": {{ "type": "campaign-start" }},
        "objectives": [
          {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }},
          {{ "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit",
             "radius": 2, "after": ["obj/talk"] }}
        ],
        "on_objective_complete": {{
          "obj/talk": [ {{ "type": "open-gate", "anchor": "anchor/door" }} ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ],
    "triggers": [
      {{
        "id": "trigger/entrance",
        "at": "anchor/exit",
        "on": {{ "on": "approach", "range": 4 }},
        "once": true,
        "effects": [
          {{ "type": "spawn-actor", "actor": "actor/sleeper" }},
          {{ "type": "move-actor", "actor": "actor/sleeper",
             "to_anchor": "anchor/keeper-stand",
             "on_arrive": [
               {{ "type": "close-gate", "anchor": "anchor/door" }},
               {on_arrive} ] }}
        ]
      }},
      {{
        "id": "trigger/wake",
        "at": "anchor/keeper-stand",
        "on": {{ "on": "strike" }},
        "once": true,
        "effects": [ {{ "type": "narrate", "style": "chat", "text": "He stirs." }} ]
      }}
    ],
    "actors": [
      {{ "id": "actor/sleeper", "entity": "minecraft:zombie", "name": "The Sleeper",
         "anchor": "anchor/exit", "facing": "west" }}
    ]
  }}
}}"#
    )
}

const HANDOFF_ARRIVE: &str = r#"{ "type": "despawn-actor", "actor": "actor/sleeper", "style": "vanish" },
             { "type": "spawn-npc", "npc": "npc/keeper" }"#;

fn build(on_arrive: &str) -> BuildOutput {
    let raw = RawCampaign {
        world: read_hw("world.json"),
        npcs: npcs_deferred(),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: quests_doc(on_arrive),
        dialogue: read_hw("dialogue.json"),
        world_edits: None,
    };
    let campaign = parse_campaign(&raw).expect("campaign parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
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
        &prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
    .expect("every emitted command validates")
}

fn file(out: &BuildOutput, path: &str) -> String {
    String::from_utf8(
        out.get(path)
            .unwrap_or_else(|| panic!("missing {path}"))
            .clone(),
    )
    .unwrap()
}

/// Invariant 1 — the island soft-lock shape exactly: deferred NPC, strike
/// trigger on its stand. World init must summon NO interaction entity for the
/// trigger; the NPC's entrance summons the one hitbox, wearing both tags.
#[test]
fn deferred_npc_strike_trigger_has_no_world_init_hitbox() {
    let out = build(HANDOFF_ARRIVE);
    let setup = file(
        &out,
        &format!("datapack/data/{NS}/function/setup_finish.mcfunction"),
    );
    assert!(
        !setup.contains("dw_trig_wake"),
        "no standalone trigger hitbox at world init — the deferred NPC's entrance \
         is the sole carrier:\n{setup}"
    );
    let entrance = file(
        &out,
        &format!("datapack/data/{NS}/function/spawn_npc_keeper.mcfunction"),
    );
    assert!(
        entrance.contains(r#"Tags:["dw_borne","dw_npc_keeper","dw_trig_wake"]"#),
        "the entrance hitbox carries the strike trigger's tag:\n{entrance}"
    );
}

/// Invariant 2 — the generated handoff PackTest: gates sealed, arrival driven,
/// puppet out, NPC in (body + exactly one hitbox), then zero residue.
#[test]
fn arrive_handoff_packtest_drives_the_sealed_handoff() {
    let out = build(HANDOFF_ARRIVE);
    let body = file(
        &out,
        &format!("packtest-datapack/data/{NS}/test/v06_arrive_handoff.mcfunction"),
    );
    // Own init: both entity families cleared before the drive.
    let clear_actor = body
        .find("kill @e[tag=dw_actor_sleeper]")
        .expect("clears actor");
    let clear_npc = body.find("kill @e[tag=dw_npc_keeper]").expect("clears npc");
    let spawn = body.find(":spawn_actor_sleeper").expect("spawns puppet");
    assert!(
        clear_actor < spawn && clear_npc < spawn,
        "entry clears precede the spawn:\n{body}"
    );
    // The gate is sealed before arrival and restored afterwards (no block residue).
    let seal = body.find("fill ").expect("seals the campaign gate");
    let drive = body
        .find(":ma_tick_sleeper_keeper_stand")
        .expect("drives the arrival tick");
    let restore = body
        .rfind("minecraft:air replace")
        .expect("re-opens the gate");
    assert!(
        seal < drive && restore > drive,
        "seal precedes the drive, restore follows it:\n{body}"
    );
    assert!(
        body.contains("scoreboard players set #at_sleeper_keeper_stand dw.sys"),
        "fast-forwards the driver to the arrival tick:\n{body}"
    );
    // The handoff itself.
    assert!(
        body.contains("assert score #pup_ahof dw.sys matches 0"),
        "puppet gone on arrival:\n{body}"
    );
    assert!(
        body.contains("if entity @e[tag=dw_npc,tag=dw_npc_keeper]")
            && body.contains("assert score #npc_ahof dw.sys matches 1"),
        "NPC body present on arrival:\n{body}"
    );
    assert!(
        body.contains("if entity @e[type=minecraft:interaction,tag=dw_npc_keeper]")
            && body.contains("assert score #box_ahof dw.sys matches 1"),
        "exactly one NPC hitbox on arrival:\n{body}"
    );
    // No entity residue: final kills follow the asserts.
    let last_assert = body.rfind("assert score").unwrap();
    assert!(
        body.rfind("kill @e[tag=dw_npc_keeper]").unwrap() > last_assert
            && body.rfind("kill @e[tag=dw_actor_sleeper]").unwrap() > last_assert,
        "exit kills follow the asserts (no residue for a sibling):\n{body}"
    );
}

/// No spawn-npc on any on_arrive → no handoff to prove → no template.
#[test]
fn no_handoff_packtest_without_a_spawn_npc_arrival() {
    let out = build(r#"{ "type": "despawn-actor", "actor": "actor/sleeper", "style": "vanish" }"#);
    assert!(
        !out.contains_key(&format!(
            "packtest-datapack/data/{NS}/test/v06_arrive_handoff.mcfunction"
        )),
        "the handoff packtest is emitted only when an arrival spawns an NPC"
    );
}
