//! A body watches (spec-0101): the declaration's refusals, the drawability
//! proof and its binding, and the bytes the emitter writes — `watch_tick`, the
//! summon's live-watch tag, and each walk driver's yield and resume — read back
//! out of real builds of the hello-world fixture.
//!
//! The run-time turn itself is proven by the generated PackTests on a server
//! (`watch_<class>_<id>`, `watch_yield_<class>_<id>`) and by the bot; the
//! supersession half of the yield by the interpreter in `move_supersede.rs`.

mod common;

use std::collections::BTreeMap;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildFailure, BuildOutput};
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};
use serde_json::{Value, json};

const FN_DIR: &str = "datapack/data/hello-world/function";

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

/// The keeper with `watch`, and a puppet that is spawned and walked once with
/// `actor_watch`. `None` leaves the field off.
fn raw(keeper_watch: Option<Value>, actor_watch: Option<Value>) -> RawCampaign {
    let npcs = common::patch_doc(&read_hw("npcs.json"), |d| {
        if let Some(w) = keeper_watch {
            d["content"]["npcs"][0]["watch"] = w;
        }
    });
    let quests = common::patch_doc(&read_hw("quests.json"), |doc| {
        let mut actor = json!({
            "id": "actor/walker",
            "entity": "minecraft:villager",
            "anchor": "spawn",
            "facing": "east"
        });
        if let Some(w) = actor_watch {
            actor["watch"] = w;
        }
        doc["content"]["actors"] = json!([actor]);
        let effects = common::objective_effects(doc, 0, "obj/talk");
        effects.push(json!({
            "type": "spawn-actor",
            "actor": "actor/walker",
            "happening": { "verb": "arrives", "text": "the walker steps out" }
        }));
        effects.push(json!({
            "type": "move-actor",
            "actor": "actor/walker",
            "to": { "anchor": "anchor/exit" },
            "happening": { "verb": "arrives", "text": "the walker crosses to anchor/exit" }
        }));
        effects.push(json!({
            "type": "move-npc",
            "npc": "npc/keeper",
            "to": { "anchor": "anchor/exit" },
            "happening": { "verb": "arrives", "text": "the keeper crosses to anchor/exit" }
        }));
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

/// Every code the two validation layers raise for `raw`: the stage parse
/// (`DW0100` and its kin) and, when that passes, the campaign checks.
fn validation_codes(raw: &RawCampaign) -> Vec<(String, String)> {
    match parse_campaign(raw) {
        Err(diags) => diags
            .iter()
            .map(|d| (d.code.to_string(), d.message.clone()))
            .collect(),
        Ok(c) => {
            let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
            common::validation_diagnostics(
                &c,
                &FullItemRegistry::v1_21_11(),
                &prefabs,
                &FullEntityRegistry::v1_21_11(),
            )
            .iter()
            .filter(|d| d.severity == delvewright_dsl::Severity::Error)
            .map(|d| (d.code.to_string(), d.message.clone()))
            .collect()
        }
    }
}

fn build(campaign: &Campaign) -> Result<BuildOutput, BuildFailure> {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    emit::build(
        &plan,
        &BTreeMap::new(),
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
}

fn built(keeper_watch: Option<Value>, actor_watch: Option<Value>) -> BuildOutput {
    let c = parse_campaign(&raw(keeper_watch, actor_watch)).expect("campaign parses");
    build(&c).unwrap_or_else(|e| panic!("build fails: {e:?}"))
}

fn function(out: &BuildOutput, name: &str) -> Option<String> {
    out.get(&format!("{FN_DIR}/{name}.mcfunction"))
        .map(|b| String::from_utf8(b.clone()).unwrap())
}

fn functions_named(out: &BuildOutput, prefix: &str) -> BTreeMap<String, String> {
    out.iter()
        .filter_map(|(p, b)| {
            let name = p
                .strip_prefix(&format!("{FN_DIR}/"))?
                .strip_suffix(".mcfunction")?;
            name.starts_with(prefix)
                .then(|| (name.to_string(), String::from_utf8(b.clone()).unwrap()))
        })
        .collect()
}

fn watchers_json(out: &BuildOutput) -> Option<Value> {
    out.get("validation/watchers.json")
        .map(|b| serde_json::from_slice(b).unwrap())
}

fn nearest(within: u32) -> Value {
    json!({ "who": "nearest", "within": within })
}

// ---------------------------------------------------------------------------
// The declaration (criterion 2)
// ---------------------------------------------------------------------------

#[test]
fn a_malformed_watch_is_the_shape_refusal() {
    for (label, w) in [
        ("within 0", json!({ "who": "nearest", "within": 0 })),
        ("within 2.5", json!({ "who": "nearest", "within": 2.5 })),
        ("no who", json!({ "within": 4 })),
        ("no within", json!({ "who": "nearest" })),
        ("an unknown who", json!({ "who": "farthest", "within": 4 })),
    ] {
        for (class, r) in [
            ("npc", raw(Some(w.clone()), None)),
            ("actor", raw(None, Some(w.clone()))),
        ] {
            let codes = validation_codes(&r);
            assert!(
                codes.iter().any(|(c, _)| c == "DW0100"),
                "{label} on the {class} is refused as a shape (DW0100); got {codes:?}"
            );
        }
    }
}

#[test]
fn a_watch_for_a_class_nobody_plays_is_dw0996_naming_the_body_and_the_class() {
    let codes = validation_codes(&raw(
        Some(json!({ "who": { "class": "class/pilgrim" }, "within": 4 })),
        None,
    ));
    let hit = codes
        .iter()
        .find(|(c, _)| c == "DW0996")
        .unwrap_or_else(|| panic!("DW0996 raised; got {codes:?}"));
    assert!(
        hit.1.contains("npc/keeper") && hit.1.contains("class/pilgrim"),
        "{}",
        hit.1
    );
    // The declared class passes.
    let ok = validation_codes(&raw(
        Some(json!({ "who": { "class": "class/wanderer" }, "within": 4 })),
        None,
    ));
    assert!(
        !ok.iter().any(|(c, _)| c == "DW0996" || c == "DW0100"),
        "{ok:?}"
    );
}

// ---------------------------------------------------------------------------
// The drawability proof (criterion 3)
// ---------------------------------------------------------------------------

/// A puppet stood two blocks over its anchor, clear of the ceiling — inside its
/// piece, out of reach of a one-block watch.
fn floating(within: u32) -> Campaign {
    let mut r = raw(None, Some(nearest(within)));
    r.quests = common::patch_doc(&r.quests, |d| {
        d["content"]["actors"][0]["offset"] = json!([0, 2, 0]);
        // A body in the air walks nowhere: the puppet keeps its stand.
        common::objective_effects(d, 0, "obj/talk").retain(|e| e["type"] != "move-actor");
    });
    parse_campaign(&r).expect("campaign parses")
}

#[test]
fn a_watch_nobody_can_draw_is_dw0997_and_its_ceiling_passes() {
    let refused = match build(&floating(1)) {
        Err(BuildFailure::Diagnostic { code, message }) => (code.to_string(), message),
        Err(other) => panic!("refused by something else: {other:?}"),
        Ok(_) => panic!("a watch no walked cell can draw built"),
    };
    assert_eq!(refused.0, "DW0997", "{}", refused.1);
    for needle in [
        "actor/walker",
        "within 1 block",
        "the nearest walked cell is [",
    ] {
        assert!(
            refused.1.contains(needle),
            "the refusal names `{needle}`: {}",
            refused.1
        );
    }
    // The prescription's number: the ceiling of the nearest distance passes.
    let ceiling: u32 = refused
        .1
        .split("`within: ")
        .nth(1)
        .and_then(|s| s.split('`').next())
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("the refusal states the reaching `within`: {}", refused.1));
    assert!(ceiling > 1);
    let out = build(&floating(ceiling)).unwrap_or_else(|e| panic!("within {ceiling}: {e:?}"));
    let w = watchers_json(&out).expect("watchers.json written");
    assert_eq!(w["binding"]["declared"], 1);
    assert_eq!(w["binding"]["refused"], 0);
}

#[test]
fn the_binding_counts_every_watcher_over_a_non_empty_population() {
    let out = built(
        Some(nearest(8)),
        Some(json!({ "who": { "class": "class/wanderer" }, "within": 6 })),
    );
    let w = watchers_json(&out).expect("watchers.json written");
    let b = &w["binding"];
    // Vacuity: a population of zero would make every watch undrawable and the
    // proof meaningless.
    assert!(
        b["population"].as_u64().unwrap() > 0,
        "P is non-empty on the fixture: {b}"
    );
    assert_eq!(b["declared"], 2);
    assert_eq!(b["drawable"], 2);
    assert_eq!(b["refused"], 0);
    let rows = w["watchers"].as_array().unwrap();
    assert_eq!(rows[0]["id"], "npc/keeper");
    assert_eq!(rows[0]["selector"], "tag=dw_npc_keeper,tag=dw_npc");
    assert_eq!(rows[0]["class_tag"], Value::Null);
    assert_eq!(rows[1]["id"], "actor/walker");
    assert_eq!(rows[1]["selector"], "tag=dw_pup_walker");
    assert_eq!(rows[1]["class_tag"], "dw_class_wanderer");
    for r in rows {
        // The PackTest cell turns the body observably from its home facing.
        let home = r["home_yaw"].as_f64().unwrap();
        let yaw = r["test_yaw"].as_f64().expect("an observable cell");
        let turn = delvec::compiler::watching::yaw_difference(yaw, home);
        assert!(turn >= 10.0, "{r}: turn {turn}");
        // Every walk's end is a stand, after the summon point.
        assert_eq!(r["stands"][0], r["feet"]);
        assert_eq!(r["stands"].as_array().unwrap().len(), 2, "{r}");
    }
    // The critical path carries the same rows.
    let cp: Value = serde_json::from_slice(out.get("critical-path.json").unwrap()).unwrap();
    assert_eq!(cp["watchers"], w["watchers"]);
}

#[test]
fn a_campaign_with_no_watch_emits_none_of_it() {
    let out = built(None, None);
    assert!(function(&out, "watch_tick").is_none());
    assert!(watchers_json(&out).is_none());
    let cp: Value = serde_json::from_slice(out.get("critical-path.json").unwrap()).unwrap();
    assert!(cp.get("watchers").is_none());
    for (path, bytes) in out.iter() {
        let text = String::from_utf8_lossy(bytes);
        assert!(
            !text.contains("dw_watch") && !text.contains("watch_tick"),
            "`{path}` carries watch bytes in a campaign that declares no watch"
        );
    }
}

// ---------------------------------------------------------------------------
// The bytes (criterion 4)
// ---------------------------------------------------------------------------

#[test]
fn the_emitted_watch_is_exactly_the_specified_bytes() {
    let out = built(
        Some(nearest(8)),
        Some(json!({ "who": { "class": "class/wanderer" }, "within": 6 })),
    );
    assert_eq!(
        function(&out, "watch_tick").expect("watch_tick emitted"),
        "execute as @e[tag=dw_npc_keeper,tag=dw_npc,tag=dw_watch,tag=!dw_unseen,limit=1] at @s \
         run rotate @s facing entity @p[distance=..8,tag=!dw_cutscene] eyes\n\
         execute as @e[tag=dw_pup_walker,tag=dw_watch,tag=!dw_unseen,limit=1] at @s run rotate \
         @s facing entity @p[distance=..6,tag=!dw_cutscene,tag=dw_class_wanderer] eyes\n"
    );
    let tick = function(&out, "tick").unwrap();
    assert_eq!(
        tick.lines()
            .filter(|l| l.contains("watch_tick"))
            .collect::<Vec<_>>(),
        vec!["function hello-world:watch_tick"]
    );
    // The summons carry the tag.
    let setup = function(&out, "setup_finish").unwrap();
    let keeper = setup
        .lines()
        .find(|l| l.contains("\"dw_npc_keeper\"") && l.starts_with("summon minecraft:villager"))
        .unwrap();
    assert!(
        keeper.contains("Tags:[\"dw_npc\",\"dw_npc_keeper\",\"dw_watch\"]"),
        "{keeper}"
    );
    let spawn = function(&out, "spawn_actor_walker").unwrap();
    assert!(
        spawn.contains("Tags:[\"dw_actor\",\"dw_actor_walker\",\"dw_pup_walker\",\"dw_watch\"]"),
        "{spawn}"
    );
    // The class watch makes the class apply wear the tag it filters on.
    let apply = function(&out, "class_apply_wanderer").unwrap();
    assert!(apply.contains("tag @s add dw_class_wanderer"), "{apply}");
    // Each walk: the start yields, the arrival tick resumes.
    for (start, tick, sel, counter) in [
        (
            "mv_keeper_exit",
            "mv_tick_keeper_exit",
            "tag=dw_npc_keeper,tag=dw_npc",
            "#mt_keeper_exit",
        ),
        (
            "ma_walker_exit",
            "ma_tick_walker_exit",
            "tag=dw_pup_walker",
            "#at_walker_exit",
        ),
    ] {
        let s = function(&out, start).unwrap_or_else(|| panic!("{start}"));
        assert_eq!(
            s.lines()
                .filter(|l| l.contains("dw_watch"))
                .collect::<Vec<_>>(),
            vec![format!("tag @e[{sel}] remove dw_watch")],
            "{start}"
        );
        // After the re-entry guards: a refused re-fire must not yield.
        let pos_guard = s.lines().position(|l| l.contains("return fail")).unwrap();
        let pos_yield = s.lines().position(|l| l.contains("dw_watch")).unwrap();
        assert!(pos_yield > pos_guard, "{start}:\n{s}");
        let t = function(&out, tick).unwrap_or_else(|| panic!("{tick}"));
        let total = t
            .lines()
            .filter(|l| l.contains(" run tp "))
            .count()
            .saturating_sub(1);
        assert_eq!(
            t.lines()
                .filter(|l| l.contains("dw_watch"))
                .collect::<Vec<_>>(),
            vec![format!(
                "execute if score {counter} dw.sys matches {total} run tag @e[{sel}] add dw_watch"
            )],
            "{tick}"
        );
    }
}

#[test]
fn a_body_without_watch_carries_none_of_the_yield() {
    // Only the keeper watches: the puppet's summon and walk are untouched.
    let out = built(Some(nearest(8)), None);
    let watch = function(&out, "watch_tick").unwrap();
    assert_eq!(watch.lines().count(), 1);
    for (name, body) in functions_named(&out, "ma_")
        .into_iter()
        .chain(functions_named(&out, "spawn_actor_"))
    {
        assert!(
            !body.contains("dw_watch"),
            "`{name}` carries the watch of a body that declares none"
        );
    }
    // And without a class watch the class apply is the flask-less one.
    let apply = function(&out, "class_apply_wanderer").unwrap();
    assert!(!apply.contains("dw_class_"), "{apply}");
}

#[test]
fn the_packtests_stand_on_the_recorded_cell_and_yield_on_every_walk() {
    let out = built(
        Some(nearest(8)),
        Some(json!({ "who": { "class": "class/wanderer" }, "within": 6 })),
    );
    let test = |name: &str| -> String {
        String::from_utf8(
            out.get(&format!(
                "packtest-datapack/data/hello-world/test/{name}.mcfunction"
            ))
            .unwrap_or_else(|| panic!("{name} emitted"))
            .clone(),
        )
        .unwrap()
    };
    let w = watchers_json(&out).unwrap();
    for (row, turn, yield_) in [
        (
            &w["watchers"][0],
            "watch_npc_keeper",
            "watch_yield_npc_keeper",
        ),
        (
            &w["watchers"][1],
            "watch_actor_walker",
            "watch_yield_actor_walker",
        ),
    ] {
        let t = test(turn);
        let cell = row["test_cell"].as_array().unwrap();
        let x = cell[0].as_i64().unwrap() as f64 + 0.5;
        let z = cell[2].as_i64().unwrap() as f64 + 0.5;
        assert!(
            t.lines()
                .any(|l| l.starts_with(&format!("tp @s {x:?} ")) && l.ends_with(&format!(" {z:?}"))),
            "{turn} stands its dummy on the recorded cell:\n{t}"
        );
        // The turn at the recorded cell is the watch's own; the wiring is read
        // by one real root tick with the body and dummy lifted out of the
        // content — dropping the tick line reds `#wr_`.
        let lines: Vec<&str> = t.lines().collect();
        let turn_at = lines
            .iter()
            .position(|l| l.contains("#wt_"))
            .expect("the turn is read");
        assert!(
            lines[..turn_at]
                .iter()
                .rev()
                .find(|l| l.starts_with("function "))
                .is_some_and(|l| l.ends_with(":watch_tick")),
            "{turn} turns the body by `watch_tick`:\n{t}"
        );
        let wired_at = lines
            .iter()
            .position(|l| l.contains("#wr_"))
            .expect("the wiring is read");
        let tick_at = lines
            .iter()
            .position(|l| *l == "function hello-world:tick")
            .expect("the real tick runs");
        assert!(tick_at < wired_at && turn_at < tick_at, "{turn}:\n{t}");
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("tp @e[") && l.contains(" 400.0 ")),
            "{turn} lifts the body above the build limit for the wiring check:\n{t}"
        );
        for assert_ in ["#wh_", "#wt_", "#wk_", "#wr_"] {
            assert!(
                t.lines()
                    .any(|l| l.starts_with("assert score") && l.contains(assert_)),
                "{turn} asserts {assert_}"
            );
        }
        let y = test(yield_);
        assert_eq!(
            y.lines().filter(|l| l.starts_with("assert score")).count(),
            2,
            "{yield_}: one walk, absent then present"
        );
    }
    // The class watch asserts the dummy draws nothing until it wears the class.
    assert!(test("watch_actor_walker").contains("#wc_"));
    assert!(!test("watch_npc_keeper").contains("#wc_"));
}
