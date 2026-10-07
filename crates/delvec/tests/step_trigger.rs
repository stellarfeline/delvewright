//! A `step` trigger: a player stepping onto a plate fires a beat, and with
//! `audience: presser` the beat runs as the player who stepped.
//!
//! The step is detected as a player whose hitbox is in the plate's cell — the
//! selector a plate trap fires on — so the act and the actor are one fact and
//! the actor is never inferred after the event. These tests pin the emission
//! (party edge latch, presser per-player latch, the gate on the presser's
//! dispatch, no advancement), the hardware proof (`DW0917` over a cell that
//! holds no plate), and the validation it unlocks (`audience: presser` on a
//! step, an `audience: actor` effect under it).

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildFailure, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{DSL_VERSION, parse_campaign, validate_campaign_with};

const NS: &str = "hello-world";

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The hello-room prefab with `anchor/plate` at `[5, 1, 7]`, one cell past the
/// doorway the compiler clears.
fn plate_prefabs(name: &str) -> PathBuf {
    let dir = tmp(name);
    common::copy_dir_all(&common::prefabs_dir(), &dir);
    let path = dir.join("hello-room.json");
    let mut meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    meta.get_mut("anchors")
        .and_then(|a| a.as_object_mut())
        .unwrap()
        .insert(
            "anchor/plate".to_string(),
            serde_json::json!({ "pos": [5, 1, 7] }),
        );
    std::fs::write(&path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();
    dir
}

fn quests(trigger: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "dsl_version": DSL_VERSION,
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
                "on_objective_complete": { "obj/talk": [
                    { "type": "open-gate", "anchor": "anchor/door" },
                    { "type": "set-flag", "flag": "flag/spoken" }
                ] },
                "on_complete": [ { "type": "campaign-complete" } ]
            } ],
            "triggers": [ trigger ]
        }
    })
}

fn world() -> serde_json::Value {
    serde_json::json!({
        "dsl_version": DSL_VERSION,
        "campaign_id": "hello-world",
        "stage": "world",
        "content": {
            "title": "The Keeper's Door",
            "theme": "A lonely keep at the edge of the moor.",
            "premise": "One locked door stands between you and the road home.",
            "seed": 20260729,
            "target_minutes": 5,
            "time": "noon",
            "weather": "clear",
            "areas": [ { "id": "area/keep", "name": "The Keep", "prefab": "prefab/hello-room" } ]
        }
    })
}

fn load(
    name: &str,
    trigger: serde_json::Value,
) -> (delvec::compiler::load::LoadedCampaign, PathBuf) {
    let camp_dir = tmp(&format!("{name}-camp"));
    let patch = serde_json::json!({
        "documents": { "world": world(), "quests": quests(trigger) }
    });
    common::materialize_from(&common::hello_world_dir(), &patch, &camp_dir);
    let prefabs_dir = plate_prefabs(&format!("{name}-prefabs"));
    (load_campaign_dir(&camp_dir).unwrap(), prefabs_dir)
}

fn diags_for(name: &str, trigger: serde_json::Value) -> Vec<delvewright_dsl::Diagnostic> {
    let (loaded, prefabs_dir) = load(name, trigger);
    let campaign = parse_campaign(&loaded.raw).expect("campaign parses");
    let prefabs = PrefabRegistry::load_dir(&prefabs_dir).unwrap();
    validate_campaign_with(
        &campaign,
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    )
}

/// Build, with `plate` (when given) written into the step trigger's cell.
fn build(
    name: &str,
    trigger: serde_json::Value,
    plate: Option<&str>,
) -> (Result<BuildOutput, BuildFailure>, [i32; 3]) {
    use delvec::admit::structure::{PaletteEntry, Structure};
    let (loaded, prefabs_dir) = load(name, trigger);
    let campaign = parse_campaign(&loaded.raw).expect("campaign parses");
    let prefabs = PrefabRegistry::load_dir(&prefabs_dir).unwrap();
    let diags = validate_campaign_with(
        &campaign,
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    );
    assert!(diags.is_empty(), "must validate clean: {diags:#?}");
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let cell = plan
        .point_any("anchor/plate")
        .expect("the plate anchor resolves");
    let mut structures = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let mut bytes = std::fs::read(prefabs_dir.join(&t.structure_file)).unwrap();
                if let Some(block) = plate {
                    let mut s = Structure::read(&bytes).unwrap();
                    let local = [cell[0] - t.pos[0], cell[1] - t.pos[1], cell[2] - t.pos[2]];
                    if s.in_bounds(local) {
                        s.set_cell(local, PaletteEntry::simple(block), None);
                        bytes = s.write();
                    }
                }
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    let out = emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    );
    (out, cell)
}

fn text(out: &BuildOutput, key: &str) -> String {
    String::from_utf8(
        out.get(key)
            .unwrap_or_else(|| panic!("missing {key}"))
            .clone(),
    )
    .unwrap()
}

fn fn_body(out: &BuildOutput, name: &str) -> String {
    text(
        out,
        &format!("datapack/data/{NS}/function/{name}.mcfunction"),
    )
}

fn step_trigger(extra: serde_json::Value) -> serde_json::Value {
    let mut t = serde_json::json!({
        "id": "trigger/doormat",
        "at": "anchor/plate",
        "on": { "on": "step" },
        "once": false,
        "effects": [ { "type": "narrate", "text": "The stone gives a little under your weight." } ]
    });
    for (k, v) in extra.as_object().unwrap() {
        t.as_object_mut().unwrap().insert(k.clone(), v.clone());
    }
    t
}

fn cell_box(c: [i32; 3]) -> String {
    format!("x={},dx=0,y={},dy=0,z={},dz=0", c[0], c[1], c[2])
}

/// A party step is polled on the plate's cell, edge-latched on `#stp_<id>`:
/// it dispatches `step_<id>` only while unlatched, and the latch clears once
/// nobody is in the cell. No hitbox is summoned and no advancement written.
#[test]
fn a_party_step_is_an_edge_latched_poll_of_the_cell() {
    let (out, c) = build(
        "step-party",
        step_trigger(serde_json::json!({})),
        Some("minecraft:stone_pressure_plate"),
    );
    let out = out.expect("builds");
    let tick = fn_body(&out, "tick");
    let sel = format!("@a[{},tag=!dw_cutscene]", cell_box(c));
    assert!(
        tick.contains(&format!(
            "execute unless score #stp_doormat dw.sys matches 1 if entity {sel} run function \
             {NS}:step_doormat"
        )),
        "{tick}"
    );
    assert!(
        tick.contains(&format!(
            "execute unless entity {sel} run scoreboard players set #stp_doormat dw.sys 0"
        )),
        "{tick}"
    );
    let step = fn_body(&out, "step_doormat");
    assert_eq!(
        step.lines().collect::<Vec<_>>(),
        vec![
            "scoreboard players set #stp_doormat dw.sys 1",
            &format!("function {NS}:trig_doormat"),
        ]
    );
    assert!(
        !out.keys().any(|k| k.contains("press_doormat")),
        "a step has no advancement: {:?}",
        out.keys()
            .filter(|k| k.contains("doormat"))
            .collect::<Vec<_>>()
    );
    let setup = fn_body(&out, "setup_finish");
    assert!(
        !setup.contains("dw_trig_doormat"),
        "a step summons no hitbox: {setup}"
    );
    // The party bundle addresses the party.
    let trig = fn_body(&out, "trig_doormat");
    assert!(trig.contains("tellraw @a"), "{trig}");
}

/// A presser step runs `as` each player in the cell who has not yet been
/// dispatched for this step, tags them, and untags them when they leave; the
/// bundle addresses `@s`, the player who stepped; the trigger's gate is stated
/// on the dispatch, spelled once.
#[test]
fn a_presser_step_runs_as_the_player_who_stepped() {
    let (out, c) = build(
        "step-presser",
        step_trigger(serde_json::json!({
            "audience": "presser",
            "requires_flags": ["flag/spoken"]
        })),
        Some("minecraft:oak_pressure_plate"),
    );
    let out = out.expect("builds");
    let tick = fn_body(&out, "tick");
    assert!(
        tick.contains(&format!(
            "execute as @a[{},tag=!dw_cutscene,tag=!dw_stp_doormat] run function \
             {NS}:step_doormat",
            cell_box(c)
        )),
        "{tick}"
    );
    assert!(
        tick.contains(&format!(
            "execute as @a[tag=dw_stp_doormat] unless entity @s[{}] run tag @s remove \
             dw_stp_doormat",
            cell_box(c)
        )),
        "{tick}"
    );
    let step = fn_body(&out, "step_doormat");
    assert_eq!(
        step.lines().collect::<Vec<_>>(),
        vec![
            "tag @s add dw_stp_doormat",
            &format!(
                "execute if score #party dw.f_spoken matches 1 run function {NS}:trig_doormat"
            ),
        ],
        "{step}"
    );
    let trig = fn_body(&out, "trig_doormat");
    assert!(trig.contains("tellraw @s"), "the stepper alone: {trig}");
    assert!(
        !out.keys().any(|k| k.contains("press_doormat")),
        "a presser step is polled, never an advancement"
    );
}

/// The plate is the piece's hardware: a step trigger over bare floor is
/// refused at build (`DW0917`), naming the trigger.
#[test]
fn a_step_over_bare_floor_is_dw0917() {
    let (out, _) = build("step-air", step_trigger(serde_json::json!({})), None);
    let e = out.expect_err("a step over air must not build");
    let s = format!("{e:?}");
    assert!(s.contains("DW0917"), "{s}");
    assert!(s.contains("trigger/doormat"), "{s}");
}

/// `audience: presser` is accepted on a step and refused on an `approach`
/// (`DW0427`); under a presser step an `audience: actor` effect has an actor,
/// under a party step it has none (`DW0503`).
#[test]
fn a_step_attributes_its_actor_and_an_approach_does_not() {
    let codes = |d: &[delvewright_dsl::Diagnostic]| -> Vec<String> {
        d.iter().map(|x| x.code.clone()).collect()
    };
    let presser = diags_for(
        "step-v-presser",
        step_trigger(serde_json::json!({
            "audience": "presser",
            "effects": [ { "type": "narrate", "text": "Only you feel it.", "audience": "actor" } ]
        })),
    );
    assert!(presser.is_empty(), "{:#?}", codes(&presser));

    let party = diags_for(
        "step-v-party",
        step_trigger(serde_json::json!({
            "effects": [ { "type": "narrate", "text": "Only you feel it.", "audience": "actor" } ]
        })),
    );
    assert!(
        codes(&party).iter().any(|c| c == "DW0503"),
        "{:#?}",
        codes(&party)
    );

    let approach = diags_for(
        "step-v-approach",
        step_trigger(serde_json::json!({
            "on": { "on": "approach", "range": 2 },
            "audience": "presser"
        })),
    );
    assert!(
        codes(&approach).iter().any(|c| c == "DW0427"),
        "{:#?}",
        codes(&approach)
    );

    let no_anchor = {
        let mut t = step_trigger(serde_json::json!({}));
        t.as_object_mut().unwrap().remove("at");
        diags_for("step-v-noat", t)
    };
    assert!(
        codes(&no_anchor).iter().any(|c| c == "DW0194"),
        "a step watches a place: {:#?}",
        codes(&no_anchor)
    );
}

/// spec-0093 §6.2 meets the `step` trigger: a checkpoint a plate sets is
/// rooted where a body can first stand on the plate, and the derivation reads
/// the plate's own firing cells — the cell and its eight horizontal neighbours
/// whose bodies lean into it (`nav::STEP_REACH`) — rather than refusing to
/// compile or falling back to the entry. Driven through the binary, because the
/// derivation is reported on the build's own `DW0315:` line.
#[test]
fn a_checkpoint_a_plate_sets_is_rooted_at_the_plates_firing_cells() {
    use delvec::admit::structure::{PaletteEntry, Structure};
    let name = "step-checkpoint";
    let trigger = step_trigger(serde_json::json!({
        "once": true,
        "effects": [ {
            "type": "set-checkpoint", "anchor": "anchor/plate",
            "happening": { "verb": "gains", "text": "The flagstone remembers them." }
        } ]
    }));
    // The hello-world campaign whole, as the binary's narrative gates read it,
    // with the plate's trigger added to its own triggers.
    let src = common::hello_world_dir();
    let mut quests: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(src.join("quests.json")).unwrap()).unwrap();
    let content = quests["content"].as_object_mut().unwrap();
    let triggers = content
        .entry("triggers")
        .or_insert_with(|| serde_json::json!([]));
    triggers.as_array_mut().unwrap().push(trigger);
    let camp_dir = tmp(&format!("{name}-camp"));
    common::materialize_from(
        &src,
        &serde_json::json!({ "documents": { "quests": quests } }),
        &camp_dir,
    );
    let prefabs_dir = plate_prefabs(&format!("{name}-prefabs"));
    let loaded = load_campaign_dir(&camp_dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("campaign parses");
    let prefabs = PrefabRegistry::load_dir(&prefabs_dir).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let cell = plan.point_any("anchor/plate").unwrap();
    let mut written = 0;
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let path = prefabs_dir.join(&t.structure_file);
                let mut s = Structure::read(&std::fs::read(&path).unwrap()).unwrap();
                let local = [cell[0] - t.pos[0], cell[1] - t.pos[1], cell[2] - t.pos[2]];
                if s.in_bounds(local) {
                    s.set_cell(
                        local,
                        PaletteEntry::simple("minecraft:stone_pressure_plate"),
                        None,
                    );
                    std::fs::write(&path, s.write()).unwrap();
                    written += 1;
                }
            }
        }
    }
    assert_eq!(written, 1, "the plate is written into exactly one template");
    let out_dir = tmp(&format!("{name}-out"));
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_delvec"))
        .arg("build")
        .arg(&camp_dir)
        .arg("--prefabs")
        .arg(&prefabs_dir)
        .arg("-o")
        .arg(&out_dir)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "the build is green: {stderr}");
    let line = stderr
        .lines()
        .find(|l| l.starts_with("DW0315: checkpoint `anchor/plate` is set by `trigger/doormat`"))
        .unwrap_or_else(|| panic!("the derivation is reported: {stderr}"));
    assert!(
        line.contains(&format!("within 1.5 of {cell:?}")),
        "rooted at the plate's own firing cells: {line}"
    );
}
