//! Quiet guidance (spec-0093): an objective says whether it is marked and
//! whether it is announced, the campaign states the default, and the proofs
//! that stood on the marker and the announcement stand on the act instead.
//!
//! Every test here builds a small hello-world-shaped campaign end to end and
//! reads the emitted bytes, or builds its plan and asks the rule that needs
//! places. Each surface is proven in both directions: the bytes move when the
//! declaration moves, and the rule reds on the perturbation only it could catch.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::promise;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, parse_campaign};
use serde_json::{Value, json};

const NS: &str = "hello-world";

/// Materialise a hello-world variant whose quests and dialogue are given, with
/// the story layer (`happening`, cast) declared the way every fixture's is.
fn variant(name: &str, mut quests: Value, mut dialogue: Value) -> PathBuf {
    let base = common::hello_world_dir();
    let npcs: Value =
        serde_json::from_str(&std::fs::read_to_string(base.join("npcs.json")).unwrap()).unwrap();
    // The story helper also writes a `title` and a `hint` onto every `kill` —
    // the lines the old rule demanded of every fight. These tests are about a
    // fight that is quiet on purpose, so a kill that declared no line keeps none.
    let bare_kills: Vec<String> = quests["content"]["quests"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|q| q["objectives"].as_array().cloned().unwrap_or_default())
        .filter(|o| o["type"] == "kill" && o.get("title").is_none() && o.get("hint").is_none())
        .map(|o| o["id"].as_str().unwrap().to_string())
        .collect();
    common::declare_story(&mut quests, &npcs, &dialogue);
    for q in quests["content"]["quests"].as_array_mut().unwrap() {
        for o in q["objectives"].as_array_mut().unwrap() {
            if bare_kills.iter().any(|id| o["id"] == *id) {
                o.as_object_mut().unwrap().remove("title");
                o.as_object_mut().unwrap().remove("hint");
            }
        }
    }
    common::declare_dialogue_story(&mut dialogue);
    let dst = std::env::temp_dir().join(format!("dw-quiet-{name}"));
    let _ = std::fs::remove_dir_all(&dst);
    common::materialize_from(
        &base,
        &json!({ "documents": { "quests": quests, "dialogue": dialogue } }),
        &dst,
    );
    dst
}

fn load(dir: &Path) -> (delvec::compiler::load::LoadedCampaign, Campaign) {
    let loaded = load_campaign_dir(dir).expect("variant loads");
    let campaign = parse_campaign(&loaded.raw).expect("variant parses");
    (loaded, campaign)
}

/// Build a campaign directory end to end — the recipe every emission test uses.
fn build_dir(dir: &Path) -> BuildOutput {
    let (loaded, campaign) = load(dir);
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    let skins: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let tree = CommandTree::v1_21_11();
    emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &tree,
        &prefabs,
        None,
        &skins,
    )
    .unwrap_or_else(|e| panic!("emission succeeds: {e:?}"))
}

fn fn_body<'a>(out: &'a BuildOutput, name: &str) -> Option<&'a str> {
    let path = format!("datapack/data/{NS}/function/{name}.mcfunction");
    out.get(&path).map(|b| std::str::from_utf8(b).unwrap())
}

/// Every shipped-datapack function body concatenated.
fn all_functions(out: &BuildOutput) -> String {
    let mut s = String::new();
    for (path, bytes) in out {
        if path.starts_with("datapack/") && path.ends_with(".mcfunction") {
            s.push_str(std::str::from_utf8(bytes).unwrap());
            s.push('\n');
        }
    }
    s
}

/// The keeper's cast entry every quest of a variant carries.
fn cast() -> Value {
    json!({ "npc/keeper": { "at": "anchor/keeper-stand", "dialogue": "dlg/greeting",
                            "doing": "keeping the gate" } })
}

/// The hello-world dialogue, with the completing option naming `objective`.
fn dialogue_completing(objective: &str) -> Value {
    json!({
        "campaign_id": NS, "stage": "dialogue",
        "content": { "dialogues": [{
            "npc": "npc/keeper", "root": "dlg/greeting",
            "nodes": [{
                "id": "dlg/greeting",
                "text": "Halt, traveler. This keep is mine to guard, and the door stays shut.",
                "options": [{
                    "label": "Open the door, please.",
                    "effects": [{ "type": "complete-objective", "objective": objective }]
                }]
            }]
        }] }
    })
}

// ---------------------------------------------------------------------------
// Markers and announcements
// ---------------------------------------------------------------------------

/// One quest: talk to the keeper, press a thing at the exit (an `interact` with
/// no `prop`), then stand at the keeper's post (a `reach-anchor`). `guidance` is
/// the campaign block; `press` and `stand` are extra fields on the two objectives.
fn guided_quests(guidance: Option<Value>, press: Value, stand: Value) -> Value {
    let mut press_obj = json!({
        "type": "interact", "id": "obj/press", "anchor": "anchor/exit",
        "title": "Press the plate", "after": ["obj/talk"]
    });
    for (k, v) in press.as_object().unwrap() {
        press_obj[k] = v.clone();
    }
    let mut stand_obj = json!({
        "type": "reach-anchor", "id": "obj/stand", "anchor": "anchor/keeper-stand",
        "radius": 2, "title": "Stand with the Keeper", "after": ["obj/press"]
    });
    for (k, v) in stand.as_object().unwrap() {
        stand_obj[k] = v.clone();
    }
    let mut content = json!({
        "quests": [{
            "id": "quest/open-the-door",
            "trigger": { "type": "campaign-start" },
            "objectives": [
                { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" },
                press_obj,
                stand_obj
            ],
            "on_objective_complete": {
                "obj/talk": [{ "type": "open-gate", "anchor": "anchor/door" }]
            },
            "on_complete": [{ "type": "campaign-complete" }],
            "cast": cast()
        }]
    });
    if let Some(g) = guidance {
        content["guidance"] = g;
    }
    json!({ "campaign_id": NS, "stage": "quests", "content": content })
}

fn guided(name: &str, guidance: Option<Value>, press: Value, stand: Value) -> BuildOutput {
    build_dir(&variant(
        name,
        guided_quests(guidance, press, stand),
        dialogue_completing("obj/talk"),
    ))
}

/// A marker is the objective's to hide and the campaign's to default: the
/// interact's lantern and the reach's end rod each follow the objective's own
/// `marker`, else `guidance.markers`; the interact's hitbox stands either way.
#[test]
fn a_marker_follows_the_objective_and_then_the_campaign() {
    // Default: both markers.
    let out = guided("markers-default", None, json!({}), json!({}));
    let all = all_functions(&out);
    assert!(
        all.contains("minecraft:lantern"),
        "the interact's lantern: {all}"
    );
    assert!(
        all.contains("minecraft:end_rod"),
        "the reach's end rod: {all}"
    );
    assert!(
        all.contains("summon minecraft:interaction"),
        "the hitbox: {all}"
    );

    // The interact hides its own marker; the reach keeps the default.
    let out = guided(
        "markers-press-hidden",
        None,
        json!({ "marker": "hidden" }),
        json!({}),
    );
    let all = all_functions(&out);
    assert!(!all.contains("minecraft:lantern"), "no lantern: {all}");
    assert!(
        all.contains("summon minecraft:interaction"),
        "the hitbox stays: {all}"
    );
    assert!(
        all.contains("minecraft:end_rod"),
        "the reach keeps its rod: {all}"
    );

    // The campaign hides markers; the reach opts back in.
    let out = guided(
        "markers-campaign-hidden",
        Some(json!({ "markers": "hidden" })),
        json!({}),
        json!({ "marker": "shown" }),
    );
    let all = all_functions(&out);
    assert!(
        !all.contains("minecraft:lantern"),
        "no lantern under the default: {all}"
    );
    assert!(
        all.contains("minecraft:end_rod"),
        "`marker: shown` wins over the default: {all}"
    );
}

/// An announcement is the objective's to hide and the campaign's to default:
/// the announce function, its once-flag, its tick line and the completion line
/// exist for exactly the announced objectives, and the bot's completion marker
/// is broadcast either way.
#[test]
fn an_announcement_follows_the_objective_and_then_the_campaign() {
    let out = guided("announce-default", None, json!({}), json!({}));
    assert!(fn_body(&out, "announce_o_press").is_some());
    assert!(fn_body(&out, "announce_o_stand").is_some());
    let setup = fn_body(&out, "setup").unwrap();
    assert!(
        setup.contains("dw.ann_press") && setup.contains("dw.ann_stand"),
        "{setup}"
    );
    let complete = fn_body(&out, "complete_o_press").unwrap();
    assert!(complete.contains("ui.objective.complete"), "{complete}");

    let out = guided(
        "announce-press-hidden",
        None,
        json!({ "announcement": "hidden" }),
        json!({}),
    );
    assert!(
        fn_body(&out, "announce_o_press").is_none(),
        "no announce function"
    );
    assert!(
        fn_body(&out, "announce_o_stand").is_some(),
        "the other is unchanged"
    );
    let setup = fn_body(&out, "setup").unwrap();
    assert!(!setup.contains("dw.ann_press"), "no once-flag: {setup}");
    let tick = fn_body(&out, "tick").unwrap();
    assert!(!tick.contains("announce_o_press"), "no tick line: {tick}");
    let complete = fn_body(&out, "complete_o_press").unwrap();
    assert!(
        !complete.contains("ui.objective.complete"),
        "no completion line: {complete}"
    );
    assert!(
        complete.contains("[dw:complete"),
        "the bot's marker stays: {complete}"
    );
    // The title still names the marker.
    let all = all_functions(&out);
    assert!(
        all.contains("CustomName:\"Press the plate\""),
        "the nameplate stays: {all}"
    );

    let out = guided(
        "announce-campaign-hidden",
        Some(json!({ "announcements": "hidden" })),
        json!({}),
        json!({ "announcement": "shown" }),
    );
    assert!(
        fn_body(&out, "announce_o_press").is_none(),
        "quiet by default"
    );
    assert!(
        fn_body(&out, "announce_o_stand").is_some(),
        "`announcement: shown` wins"
    );
}

// ---------------------------------------------------------------------------
// The button obeys the objective's gate (spec-0093 §6.3)
// ---------------------------------------------------------------------------

/// The island's muster/surf structure: a `talk-to` whose completion spawns the
/// wave, the `kill` on that wave, and a `talk-to` that ends the delve, gated
/// `after` the kill and on the flag the kill sets. Both buttons sit in the
/// keeper's one node from the first tick.
fn beach_quests(wave: Value, extra_roots: Value) -> Value {
    let mut content = json!({
        "quests": [{
            "id": "quest/open-the-door",
            "trigger": { "type": "campaign-start" },
            "objectives": [
                { "type": "talk-to", "id": "obj/muster", "npc": "npc/keeper" },
                { "type": "kill", "id": "obj/surf", "wave": "wave/surf", "after": ["obj/muster"] },
                { "type": "talk-to", "id": "obj/climb-out", "npc": "npc/keeper",
                  "after": ["obj/surf"], "requires_flags": ["flag/ashore"] }
            ],
            "on_objective_complete": {
                "obj/muster": [
                    { "type": "open-gate", "anchor": "anchor/door" },
                    { "type": "spawn-wave", "wave": "wave/surf" }
                ],
                "obj/surf": [{ "type": "set-flag", "flag": "flag/ashore" }]
            },
            "on_complete": [{ "type": "campaign-complete" }],
            "cast": cast()
        }],
        "waves": [wave]
    });
    for (k, v) in extra_roots.as_object().unwrap() {
        content[k] = v.clone();
    }
    json!({ "campaign_id": NS, "stage": "quests", "content": content })
}

fn beach_dialogue() -> Value {
    json!({
        "campaign_id": NS, "stage": "dialogue",
        "content": { "dialogues": [{
            "npc": "npc/keeper", "root": "dlg/greeting",
            "nodes": [{
                "id": "dlg/greeting",
                "text": "Twelve of us on this beach, Captain, and I do not like that smoke.",
                "options": [
                    { "label": "We climb.",
                      "effects": [{ "type": "complete-objective", "objective": "obj/muster" }] },
                    { "label": "Lead on.",
                      "effects": [{ "type": "complete-objective", "objective": "obj/climb-out" }] }
                ]
            }]
        }] }
    })
}

fn surf(follow_range: Option<f64>) -> Value {
    let mut mob = json!({ "entity": "minecraft:zombie", "count": 3 });
    if let Some(r) = follow_range {
        mob["attributes"] = json!({ "follow_range": r });
    }
    json!({ "id": "wave/surf", "anchor": "anchor/exit", "mobs": [mob] })
}

/// **The emission invariant that retired the skip rule** (ledger row `isl-55`): the
/// availability bit and the click handler of the button that completes
/// `obj/climb-out` both carry the objective's whole pending guard — `obj/surf`
/// complete and `flag/ashore` set — so "Lead on." is not on screen beside "We
/// climb." from the first tick, and a press before its turn completes nothing.
/// The generated `dialogue_mask` PackTest breaks each of those terms on its own.
#[test]
fn the_completing_button_carries_its_objectives_pending_guard() {
    let out = build_dir(&variant(
        "beach-button",
        beach_quests(surf(None), json!({})),
        beach_dialogue(),
    ));
    let dmask = fn_body(&out, "dmask_keeper_greeting").expect("the node is display-gated");
    let lead_on = dmask
        .lines()
        .find(|l| l.ends_with("run scoreboard players add @s dw.dmask 2"))
        .unwrap_or_else(|| panic!("bit 1 is `Lead on.`: {dmask}"));
    assert!(
        lead_on.contains("if score #party dw.o_surf matches 1"),
        "the `after` term: {lead_on}"
    );
    assert!(
        lead_on.contains("if score #party dw.f_ashore matches 1"),
        "the flag term: {lead_on}"
    );
    assert!(
        lead_on.contains("if score #party dw.qa_open_the_door matches 1")
            && lead_on.contains("unless score #party dw.o_climb_out matches 1"),
        "the quest-active and not-yet-done terms stay: {lead_on}"
    );
    let we_climb = dmask
        .lines()
        .find(|l| l.ends_with("run scoreboard players add @s dw.dmask 1"))
        .unwrap();
    assert!(
        !we_climb.contains("dw.o_surf") && !we_climb.contains("dw.f_ashore"),
        "`We climb.` carries only its own objective's guard: {we_climb}"
    );
    // The click handler is guarded the same way.
    let click = out
        .iter()
        .filter(|(p, _)| p.starts_with(&format!("datapack/data/{NS}/function/dlg_keeper_")))
        .map(|(_, b)| std::str::from_utf8(b).unwrap())
        .find(|b| b.contains("complete_o_climb_out"))
        .expect("the click handler for `Lead on.`");
    let line = click
        .lines()
        .find(|l| l.contains("complete_o_climb_out"))
        .unwrap();
    assert!(
        line.contains("if score #party dw.o_surf matches 1")
            && line.contains("if score #party dw.f_ashore matches 1"),
        "the click completes under the pending guard: {line}"
    );
    // The generated PackTest drives each term and asserts the bit gone.
    let mask_test = out
        .iter()
        .filter(|(p, _)| p.starts_with("packtest-datapack/") && p.ends_with(".mcfunction"))
        .map(|(_, b)| std::str::from_utf8(b).unwrap())
        .find(|b| b.contains("dmask_keeper_greeting") && b.contains("dw.o_surf 0"))
        .expect("a mask template breaks the `after` term");
    assert!(
        mask_test.contains("scoreboard players set #party dw.f_ashore 0"),
        "and the flag term: {mask_test}"
    );
}

/// spec-0093's button guard beside the one trigger-guard authority, in one
/// build: the keeper's `Lead on.` completes under its objective's
/// `pending_guard`, and a presser `use` trigger gated on the same flag answers
/// through `trigger_poll_guards`. The two gates are different rules (an
/// objective's turn, a trigger's arming) with one authority each; both lines
/// are single-spaced and the build's command-tree check walks them.
#[test]
fn the_button_guard_and_a_gated_press_answer_each_read_their_one_authority() {
    let out = build_dir(&variant(
        "beach-button-and-press",
        beach_quests(
            surf(None),
            json!({ "triggers": [{
                "id": "trigger/bell",
                "at": "anchor/exit",
                "on": { "on": "use" },
                "audience": "presser",
                "requires_flags": ["flag/ashore"],
                "effects": [{ "type": "narrate", "text": "The bell answers you alone.", "audience": "actor" }]
            }] }),
        ),
        beach_dialogue(),
    ));
    let press =
        fn_body(&out, "press_bell").expect("a presser trigger answers through its press function");
    assert!(
        press.lines().any(|l| l
            == format!(
                "execute unless score #trig_bell dw.sys matches 1 if score #party dw.f_ashore \
                 matches 1 run function {NS}:trig_bell"
            )),
        "the press answer reads the trigger's gate from the one authority: {press}"
    );
    let click = out
        .iter()
        .filter(|(p, _)| p.starts_with(&format!("datapack/data/{NS}/function/dlg_keeper_")))
        .map(|(_, b)| std::str::from_utf8(b).unwrap())
        .find(|b| b.contains("complete_o_climb_out"))
        .expect("the click handler for `Lead on.`");
    let line = click
        .lines()
        .find(|l| l.contains("complete_o_climb_out"))
        .unwrap();
    assert!(
        line.contains("if score #party dw.o_surf matches 1")
            && line.contains("if score #party dw.f_ashore matches 1")
            && !line.contains("  "),
        "the click completes under the pending guard, single-spaced: {line}"
    );
}

// ---------------------------------------------------------------------------
// DW0863 — a fight the party cannot find (spec-0093 §5)
// ---------------------------------------------------------------------------

fn signposts(name: &str, quests: Value) -> (promise::FightBinding, Result<(), String>) {
    let dir = variant(name, quests, beach_dialogue());
    let (_, campaign) = load(&dir);
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let (b, v) = promise::check_fight_signposts(&plan);
    (b, v.map_err(|f| format!("{}: {}", f.code, f.message)))
}

/// An untitled kill whose wave arrives where the act is — the keeper's post is
/// four blocks from the exit, inside the default reach — is found by the party
/// and raises nothing; the same wave with a two-block reach is not, and the
/// refusal names the site, the distance and the reach it was held to.
#[test]
fn a_fight_is_found_by_the_party_when_it_arrives_within_reach_of_the_act() {
    let (b, v) = signposts("found", beach_quests(surf(None), json!({})));
    assert!(v.is_ok(), "{v:?}");
    assert_eq!(
        (b.kills, b.announced, b.found, b.sites, b.unplaced),
        (1, 0, 1, 1, 0)
    );

    let (b, v) = signposts("out-of-reach", beach_quests(surf(Some(2.0)), json!({})));
    let msg = v.expect_err("a wave out of reach of its act is DW0863");
    assert!(msg.starts_with("DW0863"), "{msg}");
    assert!(
        msg.contains("4.0 blocks from the anchor"),
        "the distance: {msg}"
    );
    assert!(msg.contains("2 blocks here"), "the reach: {msg}");
    assert!(msg.contains("obj/muster"), "the firing site: {msg}");
    assert_eq!((b.kills, b.announced, b.found), (1, 0, 0));
}

/// The announced arm: a title and a hint make the same out-of-reach fight
/// findable; hide the announcement — on the objective or by the campaign — and
/// the lines say nothing again.
#[test]
fn an_announced_fight_is_found_by_its_lines_unless_its_announcement_is_hidden() {
    let announced = |extra: Value, guidance: Option<Value>| {
        let mut q = beach_quests(surf(Some(2.0)), json!({}));
        let kill = &mut q["content"]["quests"][0]["objectives"][1];
        kill["title"] = json!("Hold the surf");
        kill["hint"] = json!("They come out of the water at the gate.");
        for (k, v) in extra.as_object().unwrap() {
            kill[k] = v.clone();
        }
        if let Some(g) = guidance {
            q["content"]["guidance"] = g;
        }
        q
    };
    let (b, v) = signposts("announced", announced(json!({}), None));
    assert!(v.is_ok(), "{v:?}");
    assert_eq!((b.kills, b.announced, b.found), (1, 1, 0));

    let (_, v) = signposts(
        "announced-hidden",
        announced(json!({ "announcement": "hidden" }), None),
    );
    let msg = v.expect_err("a hidden announcement says nothing");
    assert!(msg.contains("its announcement is hidden"), "{msg}");

    let (_, v) = signposts(
        "announced-campaign-hidden",
        announced(json!({}), Some(json!({ "announcements": "hidden" }))),
    );
    assert!(v.is_err(), "the campaign default hides it too");
}

/// A root that places the party nowhere — the campaign's `on_death` — cannot
/// find the party for the fight, and the refusal says so by name.
#[test]
fn a_wave_fired_from_a_placeless_root_is_not_found_by_the_party() {
    let (b, v) = signposts(
        "placeless",
        beach_quests(
            surf(None),
            json!({ "on_death": [{ "type": "spawn-wave", "wave": "wave/surf" }] }),
        ),
    );
    let msg = v.expect_err("a death-fired wave places the party nowhere");
    assert!(msg.contains("places the party nowhere"), "{msg}");
    assert!(msg.contains("/content/on_death"), "{msg}");
    assert_eq!((b.sites, b.unplaced), (2, 1));
}

/// spec-0093 meets the plate's `step` trigger: a wave a stepped plate fires is
/// placed where the plate is — the act is a body on the plate's cell, so the
/// party stands there when the wave arrives — and is found by the party
/// within reach; in a single-area campaign the trigger's wave is the area's
/// (`DW0310`), so the plan builds.
#[test]
fn a_wave_a_stepped_plate_fires_is_found_where_the_plate_is() {
    let mut q = beach_quests(
        surf(None),
        json!({ "triggers": [{
            "id": "trigger/doormat",
            "at": "anchor/exit",
            "on": { "on": "step" },
            "effects": [{ "type": "spawn-wave", "wave": "wave/surf" }]
        }] }),
    );
    // The plate is the wave's only root.
    let muster = &mut q["content"]["quests"][0]["on_objective_complete"]["obj/muster"];
    muster
        .as_array_mut()
        .unwrap()
        .retain(|e| e["type"] != "spawn-wave");
    let (b, v) = signposts("stepped", q);
    assert!(v.is_ok(), "{v:?}");
    assert_eq!(
        (b.kills, b.announced, b.found, b.sites, b.unplaced),
        (1, 0, 1, 1, 0),
        "one site, the plate, placed and within reach"
    );
}

// ---------------------------------------------------------------------------
// DW0315 — a trigger-set checkpoint is rooted where the trigger is reachable
// ---------------------------------------------------------------------------

/// The v0.6 checkpoints showcase with one more trigger: approaching the shrine
/// sets the checkpoint there. The checkpoint records its trigger on the plan,
/// and the build — with the no-stranding proof rooted at the earliest
/// configuration that reaches the shrine, not at the entry — is green.
#[test]
fn a_checkpoint_set_by_an_approach_trigger_builds_and_names_its_trigger() {
    let base = common::compiler_fixtures_dir().join("v06-checkpoints");
    let mut quests: Value =
        serde_json::from_str(&std::fs::read_to_string(base.join("quests.json")).unwrap()).unwrap();
    quests["content"]["triggers"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "id": "trigger/shrine-rest",
            "at": "anchor/objective",
            "on": { "on": "approach", "range": 3 },
            "effects": [{
                "type": "set-checkpoint", "anchor": "anchor/objective",
                "happening": { "verb": "gains", "text": "The shrine keeps them." }
            }]
        }));
    let dst = std::env::temp_dir().join("dw-quiet-trigger-checkpoint");
    let _ = std::fs::remove_dir_all(&dst);
    common::materialize_from(&base, &json!({ "documents": { "quests": quests } }), &dst);
    common::copy_dir_all(&base.join("skins"), &dst.join("skins"));
    let (loaded, campaign) = load(&dst);
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let cp = plan
        .checkpoints
        .iter()
        .find(|c| c.trigger.as_deref() == Some("trigger/shrine-rest"))
        .expect("the trigger's checkpoint is on the plan and names its trigger");
    assert_eq!(
        cp.fire_step, 0,
        "the plan keeps the entry; the proof re-roots it"
    );
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    let mut skins: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for npc in &campaign.npcs.content.npcs {
        if let Some(skin) = &npc.skin {
            let png = std::fs::read(dst.join("skins").join(format!("{}.png", skin.texture_id)))
                .expect("skin png present");
            skins.insert(skin.texture_id.clone(), png);
        }
    }
    emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &skins,
    )
    .unwrap_or_else(|e| panic!("a trigger-set checkpoint at the shrine builds: {e:?}"));
}
