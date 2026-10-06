//! The spec-0082 fixture, shared by every test that needs an assembly: the
//! hello-world keep with one assembly standing on its exit — a three-part rig
//! whose `strike` clip lays a 3 × 3 slab over the exit and whose hitbox is
//! counted by a `strike-assembly` trigger. Three strikes set `flag/struck`,
//! which the exit's `reach-anchor` waits on, so the critical path performs the
//! trigger three times.
#![allow(dead_code)]

use std::collections::BTreeMap;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildFailure, BuildOutput};
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::rig::{Clip, PartKind, Rig, RigPart, RigProvenance, Transform};
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};
use serde_json::{Value, json};

pub fn read_hw(name: &str) -> String {
    std::fs::read_to_string(super::hello_world_dir().join(name)).unwrap()
}

fn t(translation: [f64; 3], scale: [f64; 3]) -> Transform {
    Transform {
        translation,
        left_rotation: [0.0, 0.0, 0.0, 1.0],
        scale,
        right_rotation: [0.0, 0.0, 0.0, 1.0],
    }
}

/// The fixture rig: three parts. `idle` stands a column on the mark cell;
/// `windup` raises it; `strike` lays a 3 × 3 slab centred on the mark;
/// `retract` sinks it below the floor.
pub fn rig() -> Rig {
    let column = |dy: f64| {
        vec![
            t([-0.5, dy, -0.5], [1.0, 1.0, 1.0]),
            t([-0.5, dy + 1.0, -0.5], [1.0, 1.0, 1.0]),
            t([-0.5, dy + 2.0, -0.5], [1.0, 0.5, 1.0]),
        ]
    };
    let slab = vec![
        t([-1.5, 0.0, -1.5], [3.0, 0.5, 3.0]),
        t([-0.5, 0.5, -0.5], [1.0, 0.5, 1.0]),
        t([-0.5, 1.0, -0.5], [1.0, 0.5, 1.0]),
    ];
    let mut clips = BTreeMap::new();
    clips.insert(
        "idle".to_string(),
        Clip {
            ticks_per_frame: 5,
            looping: true,
            frames: vec![column(0.0), column(0.1)],
        },
    );
    clips.insert(
        "windup".to_string(),
        Clip {
            ticks_per_frame: 2,
            looping: false,
            frames: vec![column(0.5), column(1.0)],
        },
    );
    clips.insert(
        "strike".to_string(),
        Clip {
            ticks_per_frame: 1,
            looping: false,
            frames: vec![column(0.5), slab],
        },
    );
    clips.insert(
        "retract".to_string(),
        Clip {
            ticks_per_frame: 5,
            looping: false,
            frames: vec![column(-3.0)],
        },
    );
    Rig {
        rig_version: 1,
        parts: (0..3)
            .map(|i| RigPart {
                id: format!("seg-{i}"),
                kind: PartKind::Block,
                block: if i == 2 {
                    "minecraft:crying_obsidian".into()
                } else {
                    "minecraft:sculk".into()
                },
                rest: None,
            })
            .collect(),
        clips,
        provenance: RigProvenance {
            generator: "crates/delvec/tests/assembly.rs".into(),
            source: "original".into(),
            spdx: "GPL-3.0-or-later".into(),
        },
    }
}

/// The fixture's quests document as a JSON value, edited by the caller.
pub fn quests() -> Value {
    let mut q: Value = serde_json::from_str(&read_hw("quests.json")).unwrap();
    let content = q["content"].as_object_mut().unwrap();
    content.insert(
        "state".into(),
        json!([{ "id": "state/hits", "scope": "party", "initial": 0,
                 "note": "how many times the thing on the exit has been struck" }]),
    );
    content.insert(
        "assemblies".into(),
        json!([{
            "id": "assembly/limb",
            "rig": "rig/limb",
            "at": { "anchor": "anchor/exit" },
            "initial": "idle",
            "hitbox": { "width": 1.0, "height": 2.0 },
            "strikes": {
                "while_in": { "anchor": "anchor/exit", "extent": [2, 1, 1] },
                "pattern": [{
                    "windup": "windup", "hold": 10, "strike": "strike",
                    "on_land": [
                        { "type": "damage-players", "amount": 4,
                          "in": { "anchor": "anchor/exit", "extent": [0, 0, 0] } }
                    ]
                }]
            }
        }]),
    );
    content.insert(
        "triggers".into(),
        json!([{
            "id": "trigger/limb-struck",
            "on": { "on": "strike-assembly", "assembly": "assembly/limb" },
            "once": false,
            "forbids_flags": ["flag/struck"],
            "effects": [
                { "type": "add-state", "state": "state/hits", "amount": 1 },
                { "type": "play-clip", "assembly": "assembly/limb", "clip": "retract",
                  "when": { "requires_state": [ { "state": "state/hits", "op": "at-least", "value": 3 } ] } },
                { "type": "set-flag", "flag": "flag/struck",
                  "when": { "requires_state": [ { "state": "state/hits", "op": "at-least", "value": 3 } ] } }
            ]
        }]),
    );
    // The thing appears when the keeper is talked to, and the exit waits on it.
    let quest = &mut content["quests"][0];
    quest["on_objective_complete"]["obj/talk"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "type": "spawn-assembly", "assembly": "assembly/limb" }));
    quest["objectives"][1]["requires_flags"] = json!(["flag/struck"]);
    q
}

/// Parse the fixture with `quests` as its quests document.
pub fn campaign(quests: &Value) -> Campaign {
    let raw = RawCampaign {
        world: read_hw("world.json"),
        npcs: read_hw("npcs.json"),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: quests.to_string(),
        dialogue: read_hw("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    };
    parse_campaign(&raw).expect("the fixture campaign parses")
}

/// The pinned library with the fixture rig added as `rig/limb`.
pub fn prefabs_with(r: Rig) -> PrefabRegistry {
    let mut p = PrefabRegistry::load_dir(&super::prefabs_dir()).unwrap();
    p.insert_rig("rig/limb", Ok(r));
    p
}

/// Build the campaign, keeping the failure and the advisory findings.
pub fn try_build_warned(
    c: &Campaign,
    prefabs: &PrefabRegistry,
) -> Result<(BuildOutput, Vec<delvewright_dsl::Diagnostic>), BuildFailure> {
    let plan = Plan::build(c, prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for tpl in &piece.templates {
                let bytes = std::fs::read(super::prefabs_dir().join(&tpl.structure_file)).unwrap();
                structures.insert(tpl.structure_file.clone(), bytes);
            }
        }
    }
    emit::build_with_warnings(
        &plan,
        &BTreeMap::new(),
        &structures,
        &CommandTree::v1_21_11(),
        prefabs,
        None,
        &BTreeMap::new(),
    )
}

/// Build the campaign, keeping the failure.
pub fn try_build(c: &Campaign, prefabs: &PrefabRegistry) -> Result<BuildOutput, BuildFailure> {
    let plan = Plan::build(c, prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for tpl in &piece.templates {
                let bytes = std::fs::read(super::prefabs_dir().join(&tpl.structure_file)).unwrap();
                structures.insert(tpl.structure_file.clone(), bytes);
            }
        }
    }
    emit::build(
        &plan,
        &BTreeMap::new(),
        &structures,
        &CommandTree::v1_21_11(),
        prefabs,
        None,
        &BTreeMap::new(),
    )
}

/// Build the fixture as written.
pub fn build_fixture() -> BuildOutput {
    let c = campaign(&quests());
    match try_build(&c, &prefabs_with(rig())) {
        Ok(o) => o,
        Err(BuildFailure::Diagnostic { code, message }) => panic!("{code}: {message}"),
        Err(BuildFailure::Validation(e)) => panic!("{} invalid command(s): {:?}", e.len(), e),
    }
}

/// The fixture with `edit` applied to its quests document.
pub fn quests_with(edit: impl FnOnce(&mut Value)) -> Value {
    let mut q = quests();
    edit(&mut q);
    q
}

/// The one assembly's JSON.
pub fn assembly(q: &mut Value) -> &mut Value {
    &mut q["content"]["assemblies"][0]
}

/// The one trigger's JSON.
pub fn trigger(q: &mut Value) -> &mut Value {
    &mut q["content"]["triggers"][0]
}

/// The code a build refuses with, or `None` when it builds.
pub fn build_code(q: &Value, r: Rig) -> Option<String> {
    match try_build(&campaign(q), &prefabs_with(r)) {
        Ok(_) => None,
        Err(BuildFailure::Diagnostic { code, .. }) => Some(code.id().to_string()),
        Err(BuildFailure::Validation(e)) => panic!("{} invalid command(s): {:?}", e.len(), e),
    }
}

/// The refusal message of a build that must fail with `code`.
pub fn refusal(q: &Value, r: Rig, code: &str) -> String {
    match try_build(&campaign(q), &prefabs_with(r)) {
        Err(BuildFailure::Diagnostic { code: c, message }) => {
            assert_eq!(c.id(), code, "refused with the wrong code: {message}");
            message
        }
        Err(BuildFailure::Validation(e)) => panic!("{} invalid command(s): {:?}", e.len(), e),
        Ok(_) => panic!("the build was meant to be refused with {code} and was not"),
    }
}

/// The validation diagnostics' codes.
pub fn validation_codes(q: &Value, r: Rig) -> Vec<(String, String, String)> {
    super::validation_diagnostics(
        &campaign(q),
        &FullItemRegistry::v1_21_11(),
        &prefabs_with(r),
        &FullEntityRegistry::v1_21_11(),
    )
    .into_iter()
    .map(|d| (d.code.clone(), d.path.clone(), d.message.clone()))
    .collect()
}
