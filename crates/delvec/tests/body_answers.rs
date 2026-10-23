//! **A harmless body says what it is** (spec-0119): the surface's refusals
//! (`DW1055`, `DW1056`), the answer box an actor gains and the NPC hitbox's
//! enclosing rule, the dispatch of each answer as the player who pressed, the
//! reach the obligation is read off, and `DW0427` shrunk to `approach`.
//!
//! Every build here is of the hello-world fixture with bodies spliced in. The
//! run-time halves — the box standing and going, each answer dispatching and
//! re-arming — are the generated PackTests (`v119_answer_box_<id>`,
//! `env_trigger_<id>`), run on a server by the ladder.

mod common;

use std::collections::{BTreeMap, BTreeSet};

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildFailure, BuildOutput};
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};
use serde_json::{Value, json};

const FN_DIR: &str = "datapack/data/hello-world/function";
const ADV_DIR: &str = "datapack/data/hello-world/advancement";

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

/// The statue: an iron-helmed zombie two cells east of the entry, never
/// `vulnerable`, spawned when the keeper is talked to.
fn statue() -> Value {
    json!({
        "id": "actor/statue",
        "entity": "minecraft:zombie",
        "anchor": "spawn",
        "offset": [2, 0, 0],
        "facing": "south",
        "equipment": { "head": "minecraft:iron_helmet" }
    })
}

fn both_lines() -> Value {
    json!({ "struck": "Stone under the iron. It does not stir.",
            "touched": "Cold. Whoever carved this did not mean it to wake." })
}

/// The hello-world campaign with `actors` spliced into stage 5 (each spawned on
/// `obj/talk`), `extra` effects appended there, and `patch_npcs` / `patch_quests`
/// applied last.
fn raw_with(
    actors: Vec<Value>,
    extra: Vec<Value>,
    patch_npcs: impl FnOnce(&mut Value),
    patch_quests: impl FnOnce(&mut Value),
) -> RawCampaign {
    let npcs = common::patch_doc(&read_hw("npcs.json"), patch_npcs);
    let quests = common::patch_doc(&read_hw("quests.json"), |doc| {
        let ids: Vec<String> = actors
            .iter()
            .map(|a| a["id"].as_str().unwrap().to_string())
            .collect();
        doc["content"]["actors"] = Value::Array(actors);
        let effects = common::objective_effects(doc, 0, "obj/talk");
        for id in ids {
            effects.push(json!({
                "type": "spawn-actor",
                "actor": id,
                "happening": { "verb": "arrives", "text": format!("{id} is set out") }
            }));
        }
        effects.extend(extra);
        patch_quests(doc);
    });
    RawCampaign {
        world: read_hw("world.json"),
        npcs,
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests,
        dialogue: read_hw("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    }
}

fn raw(actors: Vec<Value>) -> RawCampaign {
    raw_with(actors, Vec::new(), |_| {}, |_| {})
}

fn campaign(r: &RawCampaign) -> Campaign {
    parse_campaign(r).unwrap_or_else(|d| panic!("campaign parses: {d:#?}"))
}

/// Every error code the validation layer raises, with its message.
fn validation_errors(c: &Campaign) -> Vec<(String, String)> {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    common::validation_diagnostics(
        c,
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    )
    .iter()
    .filter(|d| d.severity == delvewright_dsl::Severity::Error)
    .map(|d| (d.code.to_string(), d.message.clone()))
    .collect()
}

fn structures(plan: &Plan) -> BTreeMap<String, Vec<u8>> {
    let mut s = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                s.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    s
}

fn try_build(c: &Campaign) -> Result<BuildOutput, BuildFailure> {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(c, &prefabs).expect("plan builds");
    emit::build(
        &plan,
        &BTreeMap::new(),
        &structures(&plan),
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
}

fn build(c: &Campaign) -> BuildOutput {
    try_build(c).unwrap_or_else(|e| panic!("build fails: {e:?}"))
}

fn refusal(c: &Campaign) -> (String, String) {
    match try_build(c) {
        Err(BuildFailure::Diagnostic { code, message }) => (code.to_string(), message),
        Err(other) => panic!("refused by something else: {other:?}"),
        Ok(_) => panic!("the build was expected to be refused"),
    }
}

fn function(out: &BuildOutput, name: &str) -> String {
    out.get(&format!("{FN_DIR}/{name}.mcfunction"))
        .map(|b| String::from_utf8(b.clone()).unwrap())
        .unwrap_or_else(|| panic!("no function `{name}`"))
}

fn advancement(out: &BuildOutput, name: &str) -> Value {
    out.get(&format!("{ADV_DIR}/{name}.json"))
        .map(|b| serde_json::from_slice(b).unwrap())
        .unwrap_or_else(|| panic!("no advancement `{name}`"))
}

fn record(out: &BuildOutput) -> Value {
    out.get("validation/body-answers.json")
        .map(|b| serde_json::from_slice(b).unwrap())
        .expect("validation/body-answers.json written")
}

/// The `width` / `height` of the one `minecraft:interaction` summon in `line`.
fn dims(line: &str) -> (f64, f64) {
    let field = |k: &str| -> f64 {
        line.split(&format!("{k}:"))
            .nth(1)
            .and_then(|s| s.split('f').next())
            .and_then(|n| n.parse().ok())
            .unwrap_or_else(|| panic!("no `{k}` in {line}"))
    };
    (field("width"), field("height"))
}

// ---------------------------------------------------------------------------
// DW1055 — silence is refused (criterion 6)
// ---------------------------------------------------------------------------

#[test]
fn a_statue_in_reach_with_no_line_is_dw1055_naming_body_kind_station_stance_and_key() {
    let (code, msg) = refusal(&campaign(&raw(vec![statue()])));
    assert_eq!(code, "DW1055", "{msg}");
    for needle in [
        "actor/statue",
        "minecraft:zombie",
        "[7, 65, 2]",
        "home mark `spawn`",
        "a player standing on [",
        "actor.statue.answers.struck",
        "actor.statue.answers.touched",
        "discards a `minecraft:zombie` on peaceful",
    ] {
        assert!(msg.contains(needle), "the refusal names `{needle}`: {msg}");
    }
}

#[test]
fn an_actor_with_struck_and_no_touched_is_refused_naming_touched() {
    let mut s = statue();
    s["answers"] = json!({ "struck": "Stone." });
    let (code, msg) = refusal(&campaign(&raw(vec![s])));
    assert_eq!(code, "DW1055", "{msg}");
    assert!(msg.contains("actor.statue.answers.touched"), "{msg}");
    assert!(!msg.contains("actor.statue.answers.struck"), "{msg}");
}

#[test]
fn a_vulnerable_actor_a_villager_statue_and_a_ridden_npc_each_pass_in_their_column() {
    let mut vulnerable = statue();
    vulnerable["id"] = json!("actor/creep");
    vulnerable["vulnerable"] = json!(true);
    let mut villager = statue();
    villager["id"] = json!("actor/clerk");
    villager["entity"] = json!("minecraft:villager");
    villager["offset"] = json!([-2, 0, 0]);
    let r = raw_with(
        vec![vulnerable, villager],
        Vec::new(),
        |n| n["content"]["npcs"][0]["base_entity"] = json!("minecraft:husk"),
        |q| {
            q["content"]["triggers"] = json!([{
                "id": "trigger/wake-the-keeper",
                "on": { "on": "strike-npc", "npc": "npc/keeper" },
                "effects": [{ "type": "narrate", "text": "He stirs.",
                              "happening": { "verb": "learns", "text": "the keeper stirs" } }]
            }]);
        },
    );
    let out = build(&campaign(&r));
    let rec = record(&out);
    let b = &rec["binding"];
    assert_eq!(b["bodies"], 3, "{b}");
    // The husk keeper and the zombie creep are monster-kind; the clerk is not.
    assert_eq!(b["monster_kind"], 2, "{b}");
    // The creep can be harmed; the keeper is inert.
    assert_eq!(b["inert"], 1, "{b}");
    assert_eq!(b["carried_by_strike_trigger"], 1, "{b}");
    assert_eq!(b["in_reach"], 0, "{b}");
    assert_eq!(b["lines_owed"], 0, "{b}");
    assert_eq!(b["refused"], 0, "{b}");
    let keeper = &rec["bodies"][0];
    assert_eq!(keeper["carried_by"], "trigger/wake-the-keeper");
}

#[test]
fn the_binding_line_prints_every_column_with_the_body_count() {
    let mut s = statue();
    s["answers"] = both_lines();
    let c = campaign(&raw(vec![s]));
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&c, &prefabs).unwrap();
    let world = delvec::compiler::nav::World::from_plan(&plan, &structures(&plan));
    let (b, verdict) = delvec::compiler::bodies::check(&plan, &world);
    assert!(verdict.is_ok());
    let line = b.line();
    assert_eq!(
        line,
        "body answers (DW1055): 2 bodies — 1 monster-kind, 1 inert, 1 in reach, 2 lines owed, \
         2 declared, 0 volunteered on kinds not owed, 0 refused; 0 out of reach, 0 carried by a \
         strike trigger"
    );
}

// ---------------------------------------------------------------------------
// Reach (criterion 5)
// ---------------------------------------------------------------------------

/// Solid cells boxing in the column above `cell`: every neighbour of its two
/// cells, and the cell over it.
fn walls_round(cell: [i32; 3]) -> BTreeSet<[i32; 3]> {
    let mut s = BTreeSet::new();
    for dy in 0..=2 {
        for dx in -1..=1 {
            for dz in -1..=1 {
                let c = [cell[0] + dx, cell[1] + dy, cell[2] + dz];
                let column = dx == 0 && dz == 0 && dy < 2;
                if !column {
                    s.insert(c);
                }
            }
        }
    }
    s
}

fn binding_over(
    c: &Campaign,
    solid: &BTreeSet<[i32; 3]>,
) -> delvec::compiler::bodies::AnswerBinding {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(c, &prefabs).unwrap();
    let world =
        delvec::compiler::nav::World::from_plan(&plan, &structures(&plan)).with_extra_solid(solid);
    delvec::compiler::bodies::check(&plan, &world).0
}

#[test]
fn a_statue_walled_off_on_every_side_is_not_owed_and_is_counted_out_of_reach() {
    let c = campaign(&raw(vec![statue()]));
    let b = binding_over(&c, &walls_round([7, 65, 2]));
    assert_eq!(
        (b.in_reach, b.out_of_reach, b.owed, b.refused),
        (0, 1, 0, 0),
        "{b:?}"
    );
    // …and the same statue, unwalled, is owed both lines.
    let open = binding_over(&c, &BTreeSet::new());
    assert_eq!(
        (open.in_reach, open.out_of_reach, open.owed, open.refused),
        (1, 0, 2, 1),
        "{open:?}"
    );
}

#[test]
fn a_statue_reachable_only_at_a_move_destination_is_owed() {
    let r = raw_with(
        vec![statue()],
        vec![json!({
            "type": "move-actor",
            "actor": "actor/statue",
            "to": { "anchor": "anchor/keeper-stand", "offset": [2, 0, 0] },
            "happening": { "verb": "arrives", "text": "the statue is carried to the keeper" }
        })],
        |_| {},
        |_| {},
    );
    let c = campaign(&r);
    let b = binding_over(&c, &walls_round([7, 65, 2]));
    assert_eq!((b.in_reach, b.out_of_reach, b.owed), (1, 0, 2), "{b:?}");
}

/// **One question, one implementation** (criterion 5): over generated worlds,
/// the body-answer reach is the holder stance pass's call less the cells the body
/// fills, and the holder stance pass's call is `nav::interaction_stances`.
#[test]
fn the_body_reach_is_the_holder_stance_call_over_generated_worlds() {
    let mut seed: u64 = 0x0119_0119;
    let mut next = || {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (seed >> 33) as i32
    };
    let mut compared = 0usize;
    for _ in 0..64 {
        // A floor at y = 0 over a 9 x 9 square, with up to twelve pillars.
        let mut solid: BTreeSet<[i32; 3]> = BTreeSet::new();
        for x in 0..9 {
            for z in 0..9 {
                solid.insert([x, 0, z]);
            }
        }
        for _ in 0..(next().rem_euclid(12)) {
            let (x, z) = (next().rem_euclid(9), next().rem_euclid(9));
            for y in 1..=2 {
                solid.insert([x, y, z]);
            }
        }
        let world = delvec::compiler::nav::World::from_solid_cells(solid.clone());
        let reachable: BTreeSet<[i32; 3]> = (0..9)
            .flat_map(|x| (0..9).map(move |z| [x, 1, z]))
            .filter(|c| !solid.contains(c))
            .collect();
        let cell = [next().rem_euclid(9), 1, next().rem_euclid(9)];
        let bx = delvec::compiler::bodies::body_box("minecraft:zombie", cell);
        let holder_call = delvec::compiler::nav::interaction_stances(&world, &bx, &reachable);
        let body_call = delvec::compiler::bodies::reach_cells(&world, &bx, &reachable);
        let expected: Vec<[i32; 3]> = holder_call.iter().copied().filter(|c| *c != cell).collect();
        assert_eq!(body_call, expected, "world {solid:?}, body on {cell:?}");
        compared += 1;
    }
    assert_eq!(compared, 64);
}

// ---------------------------------------------------------------------------
// The answer body (criterion 3)
// ---------------------------------------------------------------------------

#[test]
fn an_answering_actor_wears_a_box_that_encloses_it_and_loses_it_on_unleash_and_despawn() {
    let mut s = statue();
    s["answers"] = both_lines();
    let r = raw_with(
        vec![s],
        vec![
            json!({ "type": "move-actor", "actor": "actor/statue", "to": { "anchor": "anchor/keeper-stand", "offset": [2, 0, 0] },
                    "happening": { "verb": "arrives", "text": "the statue is carried" } }),
        ],
        |_| {},
        |q| {
            q["content"]["quests"][0]["on_objective_complete"]["obj/exit"] = json!([
                { "type": "unleash-actor", "actor": "actor/statue",
                  "happening": { "verb": "arrives", "text": "the statue wakes" } },
                { "type": "despawn-actor", "actor": "actor/statue", "style": "vanish",
                  "happening": { "verb": "departs", "text": "the statue is gone" } }
            ]);
        },
    );
    let out = build(&campaign(&r));
    let spawn = function(&out, "spawn_actor_statue");
    let line = spawn
        .lines()
        .find(|l| l.contains("summon minecraft:interaction"))
        .unwrap_or_else(|| panic!("spawn_actor_statue stands no box: {spawn}"));
    for tag in ["\"dw_actor_statue\"", "\"dw_abody_statue\"", "\"dw_borne\""] {
        assert!(line.contains(tag), "the box wears {tag}: {line}");
    }
    let (w, h) = dims(line);
    let (zw, zh) = delvec::compiler::nav::entity_dims("minecraft:zombie");
    let m = delvec::compiler::clickbox::CLICK_MARGIN;
    assert!(
        w >= zw + m && h >= zh + m,
        "the box {w} x {h} encloses a zombie: {line}"
    );
    let unleash = function(&out, "unleash_statue");
    assert_eq!(
        unleash.lines().next(),
        Some("kill @e[tag=dw_abody_statue]"),
        "the box goes before the twin stands: {unleash}"
    );
    let all: String = out
        .iter()
        .filter(|(p, _)| p.starts_with(FN_DIR))
        .map(|(_, b)| String::from_utf8(b.clone()).unwrap())
        .collect();
    let despawn_at = all
        .find(
            "kill @e[tag=dw_abody_statue]\nexecute if entity @e[tag=dw_actor_statue] run schedule",
        )
        .is_some();
    assert!(
        despawn_at,
        "despawn-actor kills the box before it touches the puppet"
    );
    // The walk's own line is unchanged, and the box follows the puppet.
    let tick = out
        .iter()
        .find(|(p, _)| p.contains("/function/ma_tick_statue"))
        .map(|(_, b)| String::from_utf8(b.clone()).unwrap())
        .expect("the walk driver");
    assert!(tick.contains("run tp @e[tag=dw_pup_statue] "), "{tick}");
    assert!(
        tick.contains(
            "execute as @e[tag=dw_abody_statue] at @e[tag=dw_pup_statue,limit=1] run tp @s ~ ~ ~"
        ),
        "{tick}"
    );
}

#[test]
fn an_actor_that_answers_nothing_emits_exactly_its_puppet() {
    let mut clerk = statue();
    clerk["entity"] = json!("minecraft:villager");
    clerk.as_object_mut().unwrap().remove("equipment");
    let out = build(&campaign(&raw(vec![clerk])));
    let spawn = function(&out, "spawn_actor_statue");
    assert_eq!(spawn.lines().count(), 1, "one summon, no box: {spawn}");
    assert!(!spawn.contains("minecraft:interaction"), "{spawn}");
    assert!(!function(&out, "unleash_statue").contains("dw_abody"));
}

#[test]
fn an_npc_larger_than_its_box_wears_one_that_encloses_it_and_a_villager_keeps_its_own() {
    let villager = build(&campaign(&raw(Vec::new())));
    let setup = function(&villager, "setup_finish");
    let line = setup
        .lines()
        .find(|l| l.contains("summon minecraft:interaction") && l.contains("dw_npc_keeper"))
        .expect("the keeper's hitbox");
    assert!(line.contains("{width:1.0f,height:2.0f,"), "{line}");

    let r = raw_with(
        Vec::new(),
        Vec::new(),
        |n| {
            n["content"]["npcs"][0]["base_entity"] = json!("minecraft:warden");
            n["content"]["npcs"][0]["answers"] =
                json!({ "struck": "The stone giant does not wake." });
        },
        |_| {},
    );
    let out = build(&campaign(&r));
    let line = function(&out, "setup_finish")
        .lines()
        .find(|l| l.contains("summon minecraft:interaction") && l.contains("dw_npc_keeper"))
        .expect("the keeper's hitbox")
        .to_string();
    let (w, h) = dims(&line);
    let (ww, wh) = delvec::compiler::nav::entity_dims("minecraft:warden");
    let m = delvec::compiler::clickbox::CLICK_MARGIN;
    assert!(
        w >= ww + 2.0 * m - 1e-9 && h >= wh + m - 1e-9,
        "{w} x {h}: {line}"
    );
    assert!(line.contains("{width:1.0f,height:2.91f,"), "{line}");
    // The struck answer rides it.
    assert!(
        line.contains("\"dw_trig_dw_answer_npc_keeper_struck\""),
        "{line}"
    );
}

// ---------------------------------------------------------------------------
// The answers dispatch as the player (criterion 4)
// ---------------------------------------------------------------------------

#[test]
fn each_answer_is_dispatched_by_its_press_criterion_as_the_presser() {
    let mut s = statue();
    s["answers"] = both_lines();
    let out = build(&campaign(&raw(vec![s])));
    for (press, criterion) in [
        ("struck", "minecraft:player_hurt_entity"),
        ("touched", "minecraft:player_interacted_with_entity"),
    ] {
        let id = format!("dw_answer_actor_statue_{press}");
        let adv = advancement(&out, &format!("press_{id}"));
        let c = &adv["criteria"]["interact"];
        assert_eq!(c["trigger"], criterion, "{adv}");
        assert_eq!(c["conditions"]["entity"]["type"], "minecraft:interaction");
        assert_eq!(
            c["conditions"]["entity"]["nbt"],
            format!("{{Tags:[\"dw_trig_{id}\"]}}")
        );
        let dispatch = function(&out, &format!("press_{id}"));
        assert!(
            dispatch.starts_with(&format!(
                "advancement revoke @s only hello-world:press_{id}\n"
            )),
            "the body answers every press: {dispatch}"
        );
        let bundle = function(&out, &format!("trig_{id}"));
        assert!(
            bundle.contains("title @s actionbar"),
            "the line goes on the presser's action bar alone: {bundle}"
        );
        // The box carries the trigger's tag.
        assert!(function(&out, "spawn_actor_statue").contains(&format!("\"dw_trig_{id}\"")));
    }
    // The press ledger records both answers on the box.
    let ledger: Value =
        serde_json::from_slice(out.get("validation/press-bodies.json").unwrap()).unwrap();
    let rides = ledger["presses"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["body"].as_str().unwrap().contains("dw_abody_statue"))
        .count();
    assert_eq!(rides, 2, "{ledger}");
}

#[test]
fn the_synthesized_answers_are_counted_in_the_record() {
    let mut s = statue();
    s["answers"] = both_lines();
    let out = build(&campaign(&raw(vec![s])));
    let rec = record(&out);
    assert_eq!(rec["binding"]["lines_declared"], 2);
    let row = rec["bodies"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == "actor/statue")
        .unwrap();
    assert_eq!(row["owed"], json!(["struck", "touched"]));
    assert_eq!(row["declared"], json!(["struck", "touched"]));
    assert_eq!(row["station"], json!([7, 65, 2]));
}

// ---------------------------------------------------------------------------
// DW1056 — a line no press can reach (criterion 7)
// ---------------------------------------------------------------------------

fn codes(c: &Campaign) -> Vec<String> {
    validation_errors(c).into_iter().map(|(c, _)| c).collect()
}

#[test]
fn a_line_on_a_vulnerable_actor_is_dw1056_and_passes_with_vulnerable_dropped() {
    let mut s = statue();
    s["vulnerable"] = json!(true);
    s["answers"] = json!({ "struck": "It bleeds." });
    let errs = validation_errors(&campaign(&raw(vec![s.clone()])));
    let hit = errs.iter().find(|(c, _)| c == "DW1056").expect("DW1056");
    assert!(
        hit.1.contains("answers.struck") && hit.1.contains("vulnerable"),
        "{}",
        hit.1
    );
    s.as_object_mut().unwrap().remove("vulnerable");
    s["answers"] = both_lines();
    assert!(!codes(&campaign(&raw(vec![s]))).contains(&"DW1056".to_string()));
}

#[test]
fn a_touched_line_on_an_npc_is_dw1056_and_passes_with_the_line_dropped() {
    let touched = raw_with(
        Vec::new(),
        Vec::new(),
        |n| n["content"]["npcs"][0]["answers"] = json!({ "touched": "He looks at you." }),
        |_| {},
    );
    let errs = validation_errors(&campaign(&touched));
    let hit = errs.iter().find(|(c, _)| c == "DW1056").expect("DW1056");
    assert!(
        hit.1.contains("answers.touched") && hit.1.contains("dialogue"),
        "{}",
        hit.1
    );
    let struck = raw_with(
        Vec::new(),
        Vec::new(),
        |n| n["content"]["npcs"][0]["answers"] = json!({ "struck": "He does not flinch." }),
        |_| {},
    );
    assert!(!codes(&campaign(&struck)).contains(&"DW1056".to_string()));
}

#[test]
fn a_struck_line_on_a_body_a_strike_trigger_rides_is_dw1056_and_passes_with_the_trigger_moved() {
    let trigger = |on: Value, at: Option<&str>| {
        let mut t = json!({
            "id": "trigger/wake",
            "on": on,
            "effects": [{ "type": "narrate", "text": "It wakes.",
                          "happening": { "verb": "learns", "text": "it wakes" } }]
        });
        if let Some(a) = at {
            t["at"] = json!(a);
            t["prop"] = json!({ "block": "minecraft:stone_button[face=floor,facing=north]" });
        }
        t
    };
    // strike-npc on the keeper, which declares `struck`.
    let ridden = raw_with(
        Vec::new(),
        Vec::new(),
        |n| n["content"]["npcs"][0]["answers"] = json!({ "struck": "He does not flinch." }),
        |q| {
            q["content"]["triggers"] = json!([trigger(
                json!({ "on": "strike-npc", "npc": "npc/keeper" }),
                None
            )])
        },
    );
    let errs = validation_errors(&campaign(&ridden));
    let hit = errs.iter().find(|(c, _)| c == "DW1056").expect("DW1056");
    assert!(hit.1.contains("trigger/wake"), "{}", hit.1);
    // a `strike` on the keeper's own stand anchor rides it the old way.
    let at_stand = raw_with(
        Vec::new(),
        Vec::new(),
        |n| n["content"]["npcs"][0]["answers"] = json!({ "struck": "He does not flinch." }),
        |q| {
            q["content"]["triggers"] = json!([trigger(
                json!({ "on": "strike" }),
                Some("anchor/keeper-stand")
            )])
        },
    );
    assert!(codes(&campaign(&at_stand)).contains(&"DW1056".to_string()));
    // moved off the body: passes.
    let moved = raw_with(
        Vec::new(),
        Vec::new(),
        |n| n["content"]["npcs"][0]["answers"] = json!({ "struck": "He does not flinch." }),
        |q| {
            q["content"]["triggers"] =
                json!([trigger(json!({ "on": "strike" }), Some("anchor/exit"))])
        },
    );
    assert!(!codes(&campaign(&moved)).contains(&"DW1056".to_string()));
}

// ---------------------------------------------------------------------------
// DW0427, shrunk (criterion 8)
// ---------------------------------------------------------------------------

fn presser_trigger(on: Value, at: Option<&str>) -> Value {
    let mut t = json!({
        "id": "trigger/answer-me",
        "on": on,
        "once": false,
        "audience": "presser",
        "effects": [{ "type": "narrate", "style": "actionbar", "text": "You strike it.",
                      "happening": { "verb": "learns", "text": "a blow lands" } }]
    });
    if let Some(a) = at {
        t["at"] = json!(a);
        t["prop"] = json!({ "block": "minecraft:stone" });
    }
    t
}

#[test]
fn a_presser_strike_and_strike_npc_pass_and_dispatch_by_player_hurt_entity_with_no_poll() {
    for (on, at) in [
        (json!({ "on": "strike" }), Some("anchor/exit")),
        (json!({ "on": "strike-npc", "npc": "npc/keeper" }), None),
    ] {
        let r = raw_with(
            Vec::new(),
            Vec::new(),
            |_| {},
            |q| q["content"]["triggers"] = json!([presser_trigger(on.clone(), at)]),
        );
        let c = campaign(&r);
        assert!(
            !codes(&c).contains(&"DW0427".to_string()),
            "{on}: {:?}",
            codes(&c)
        );
        let out = build(&c);
        let adv = advancement(&out, "press_answer_me");
        assert_eq!(
            adv["criteria"]["interact"]["trigger"], "minecraft:player_hurt_entity",
            "{adv}"
        );
        assert_eq!(
            adv["criteria"]["interact"]["conditions"]["entity"]["nbt"],
            "{Tags:[\"dw_trig_answer_me\"]}"
        );
        let tick = function(&out, "tick");
        assert!(
            !tick.contains("dw_trig_answer_me"),
            "a presser blow is not polled and its record is not cleared: {tick}"
        );
    }
}

/// **A lit blow is still the striker's** (spec-0119 §5.3 beside spec-0104 §7):
/// a presser `strike` that also declares a proximity `highlight` is admitted by
/// both checks, dispatched by `player_hurt_entity` with no tick poll, and lit
/// by its own poll — the glow reads the trigger's guards, never the dispatch.
#[test]
fn a_presser_strike_with_a_highlight_is_lit_and_dispatched_as_the_striker() {
    let r = raw_with(
        Vec::new(),
        Vec::new(),
        |_| {},
        |q| {
            let mut t = presser_trigger(json!({ "on": "strike" }), Some("anchor/exit"));
            t["highlight"] = json!({ "within": 5 });
            q["content"]["triggers"] = json!([t]);
        },
    );
    let c = campaign(&r);
    let cs = codes(&c);
    assert!(
        !cs.contains(&"DW0427".to_string()) && !cs.contains(&"DW1016".to_string()),
        "{cs:?}"
    );
    let out = build(&c);
    let adv = advancement(&out, "press_answer_me");
    assert_eq!(
        adv["criteria"]["interact"]["trigger"], "minecraft:player_hurt_entity",
        "{adv}"
    );
    // `function` panics when the body is absent: the blow's block is lit.
    let poll = function(&out, "hl_poll_t_answer_me");
    assert!(!poll.is_empty(), "the highlight polls its own nearness");
    assert!(
        !function(&out, "tick").contains("dw_trig_answer_me"),
        "the highlight does not bring the blow's poll back"
    );
}

#[test]
fn a_presser_approach_is_dw0427() {
    let r = raw_with(
        Vec::new(),
        Vec::new(),
        |_| {},
        |q| {
            let mut t = presser_trigger(json!({ "on": "approach", "range": 3 }), None);
            t["at"] = json!("anchor/exit");
            q["content"]["triggers"] = json!([t]);
        },
    );
    let errs = validation_errors(&campaign(&r));
    let hit = errs.iter().find(|(c, _)| c == "DW0427").expect("DW0427");
    assert!(
        hit.1.contains("approach") && hit.1.contains("carrier"),
        "{}",
        hit.1
    );
}

/// The doc comments of `TriggerAudience` and `DW0427` and the press-answer
/// section of `compiler.md` carry no sentence saying vanilla offers no left-click
/// criterion (criterion 8). The count is stated.
#[test]
fn no_page_says_vanilla_offers_no_left_click_criterion() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let sources = [
        root.join("crates/dsl/src/trigger.rs"),
        root.join("docs/reference/compiler.md"),
        root.join("docs/reference/dsl/trigger.md"),
    ];
    let claims = [
        "offers no such criterion for a left-click",
        "no criterion for it",
        "a UUID no command can become",
        "vanilla names the player only for a RIGHT-click",
    ];
    let mut matched = 0usize;
    for path in &sources {
        let text = std::fs::read_to_string(path).unwrap();
        for c in claims {
            matched += text.matches(c).count();
        }
    }
    println!(
        "left-click claim: {matched} match(es) over {} file(s), {} phrasing(s)",
        sources.len(),
        claims.len()
    );
    assert_eq!(matched, 0);
}

// ---------------------------------------------------------------------------
// Byte identity, and the inventory (criteria 3 and 9)
// ---------------------------------------------------------------------------

/// **An actor that answers nothing emits what it emitted before the field**:
/// the same campaign built with a villager actor's volunteered `answers` and
/// with the field removed. Every function of the field-less build is in the
/// answering build byte for byte, except `spawn_actor_<id>` and `unleash_<id>`,
/// which each differ by exactly the one box line; what the answering build adds is the two answers'
/// `trig_` / `press_` bodies and nothing else.
#[test]
fn removing_the_answers_removes_exactly_the_box_and_the_two_answers() {
    let mut clerk = statue();
    clerk["entity"] = json!("minecraft:villager");
    clerk.as_object_mut().unwrap().remove("equipment");
    let without = build(&campaign(&raw(vec![clerk.clone()])));
    clerk["answers"] = both_lines();
    let with = build(&campaign(&raw(vec![clerk])));
    let fns = |out: &BuildOutput| -> BTreeMap<String, String> {
        out.iter()
            .filter_map(|(p, b)| {
                let n = p
                    .strip_prefix(&format!("{FN_DIR}/"))?
                    .strip_suffix(".mcfunction")?;
                Some((n.to_string(), String::from_utf8(b.clone()).unwrap()))
            })
            .collect()
    };
    let (a, b) = (fns(&without), fns(&with));
    let mut compared = 0;
    for (name, body) in &a {
        let other = b.get(name).unwrap_or_else(|| panic!("`{name}` is gone"));
        // The box stands with the puppet and goes before the twin stands.
        if name == "spawn_actor_statue" || name == "unleash_statue" {
            let extra: Vec<&str> = other
                .lines()
                .filter(|l| !body.lines().any(|x| x == *l))
                .collect();
            assert_eq!(extra.len(), 1, "{other}");
            assert!(extra[0].contains("dw_abody_statue"), "{other}");
            continue;
        }
        assert_eq!(body, other, "`{name}` moved");
        compared += 1;
    }
    let added: Vec<&String> = b.keys().filter(|k| !a.contains_key(*k)).collect();
    assert_eq!(
        added,
        vec![
            "press_dw_answer_actor_statue_struck",
            "press_dw_answer_actor_statue_touched",
            "trig_dw_answer_actor_statue_struck",
            "trig_dw_answer_actor_statue_touched",
        ],
        "the answering build adds the two answers' bodies and nothing else"
    );
    assert!(compared > 10, "binding: {compared} function(s) compared");
}

#[test]
fn the_answers_are_inventoried_as_refusals_with_a_situation() {
    let mut s = statue();
    s["answers"] = both_lines();
    let r = raw_with(
        vec![s],
        Vec::new(),
        |n| n["content"]["npcs"][0]["answers"] = json!({ "struck": "He does not flinch." }),
        |_| {},
    );
    let c = campaign(&r);
    let inv = delvewright_dsl::l10n::inventory(&c);
    let situations = delvewright_dsl::l10n::key_situations(&c);
    for key in [
        "npc.keeper.answers.struck",
        "actor.statue.answers.struck",
        "actor.statue.answers.touched",
    ] {
        assert!(inv.contains_key(key), "`{key}` is inventoried");
        assert_eq!(
            delvewright_dsl::l10n::key_kind(key),
            Some(delvewright_dsl::l10n::TextKind::Refusal),
            "`{key}` is a refusal"
        );
        let lines = situations.get(key).expect("a situation row");
        assert!(
            lines.iter().any(|l| l.contains("action bar")),
            "`{key}`'s situation says where it shows: {lines:?}"
        );
    }
}
