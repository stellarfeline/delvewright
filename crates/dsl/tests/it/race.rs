//! spec-0139, document tier: a race's shape (`DW1117`), and every reference
//! to a race, a racer and a board resolving (`DW0112`).

use delvewright_dsl::{DSL_VERSION, RawCampaign, parse_campaign, validate_campaign};
use serde_json::{Value, json};

fn hw(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/valid/hello-world")
            .join(name),
    )
    .unwrap()
}

/// hello-world with `edit` applied to its stage-5 `content`.
fn with(edit: impl FnOnce(&mut Value)) -> delvewright_dsl::Campaign {
    let mut q: Value = serde_json::from_str(&hw("quests.json")).unwrap();
    q["dsl_version"] = json!(DSL_VERSION);
    edit(&mut q["content"]);
    parse_campaign(&RawCampaign {
        world: hw("world.json"),
        npcs: hw("npcs.json"),
        classes: hw("classes.json"),
        quest_plan: hw("quest-plan.json"),
        quests: q.to_string(),
        dialogue: hw("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    })
    .expect("parses")
}

fn codes_at(c: &delvewright_dsl::Campaign, code: &str) -> Vec<String> {
    validate_campaign(c)
        .into_iter()
        .filter(|d| d.code == code)
        .map(|d| d.path)
        .collect()
}

fn cp() -> Value {
    json!({"at": {"anchor": "anchor/keeper-stand"}, "extent": [1, 1, 1]})
}

fn race() -> Value {
    json!({"id": "race/dash", "checkpoints": [cp(), cp()], "missed": "Go back."})
}

fn racer(name: bool) -> Value {
    let mut a = json!({"id": "actor/racer", "entity": "minecraft:zombie",
                       "anchor": "anchor/keeper-stand"});
    if name {
        a["name"] = json!("Oskar");
    }
    a
}

fn on_talk(content: &mut Value, effects: Value) {
    content["quests"][0]["on_objective_complete"]["obj/talk"] = effects;
}

#[test]
fn a_race_as_written_is_accepted_and_round_trips() {
    let c = with(|q| {
        let mut r = race();
        r["laps"] = json!(3);
        r["grace_ticks"] = json!(600);
        r["racers"] = json!(["actor/racer"]);
        r["board"] = json!("board/dash");
        r["passed"] = json!("Checkpoint");
        q["races"] = json!([r]);
        q["actors"] = json!([racer(true)]);
        q["boards"] = json!([{"id": "board/dash", "title": "The Dash"}]);
        on_talk(
            q,
            json!([{"type": "start-race", "race": "race/dash"},
                   {"type": "end-race", "race": "race/dash"}]),
        );
    });
    assert!(codes_at(&c, "DW1117").is_empty());
    assert!(codes_at(&c, "DW0112").is_empty());
    let r = &c.quests.content.races[0];
    assert_eq!((r.laps, r.total(), r.grace_ticks), (3, 6, Some(600)));
    // A place for every player and for the listed actor.
    assert_eq!(delvewright_dsl::board_rows(&c, "board/dash"), 5);
    // One lap is the default and is not written.
    let one = with(|q| q["races"] = json!([race()]));
    let wire = serde_json::to_value(&one.quests.content.races[0]).unwrap();
    assert!(wire.get("laps").is_none(), "{wire}");
}

#[test]
fn a_race_that_cannot_be_run_is_dw1117() {
    let shape = |edit: fn(&mut Value)| {
        let c = with(|q| {
            let mut r = race();
            edit(&mut r);
            q["races"] = json!([r]);
        });
        codes_at(&c, "DW1117")
    };
    assert_eq!(
        shape(|r| r["checkpoints"] = json!([])),
        ["/content/races/0/checkpoints"]
    );
    assert_eq!(shape(|r| r["laps"] = json!(0)), ["/content/races/0/laps"]);
    assert_eq!(
        shape(|r| r["grace_ticks"] = json!(0)),
        ["/content/races/0/grace_ticks"]
    );
    assert_eq!(
        shape(|r| {
            r["laps"] = json!(2);
            r["checkpoints"] = json!([cp()]);
        }),
        ["/content/races/0/laps"]
    );
    assert!(shape(|_| {}).is_empty());
}

#[test]
fn a_racer_is_listed_once_named_for_a_board_and_in_one_race() {
    let twice = with(|q| {
        let mut r = race();
        r["racers"] = json!(["actor/racer", "actor/racer"]);
        q["races"] = json!([r]);
        q["actors"] = json!([racer(true)]);
    });
    assert_eq!(codes_at(&twice, "DW1117"), ["/content/races/0/racers/1"]);
    let unnamed = |name: bool| {
        let c = with(|q| {
            let mut r = race();
            r["racers"] = json!(["actor/racer"]);
            r["board"] = json!("board/dash");
            q["races"] = json!([r]);
            q["actors"] = json!([racer(name)]);
            q["boards"] = json!([{"id": "board/dash", "title": "The Dash"}]);
        });
        codes_at(&c, "DW1117")
    };
    assert_eq!(unnamed(false), ["/content/races/0/racers/0"]);
    assert!(unnamed(true).is_empty());
    let two = with(|q| {
        let mut a = race();
        a["racers"] = json!(["actor/racer"]);
        let mut b = a.clone();
        b["id"] = json!("race/second");
        q["races"] = json!([a, b]);
        q["actors"] = json!([racer(true)]);
    });
    assert_eq!(codes_at(&two, "DW1117"), ["/content/races/1/racers/0"]);
}

#[test]
fn a_walk_along_a_race_is_a_racers_and_a_course_is_ridden() {
    let walk = |listed: bool| {
        let c = with(|q| {
            let mut r = race();
            if listed {
                r["racers"] = json!(["actor/racer"]);
            }
            q["races"] = json!([r]);
            q["actors"] = json!([racer(true)]);
            on_talk(
                q,
                json!([{"type": "move-actor", "actor": "actor/racer",
                        "to": {"anchor": "anchor/keeper-stand"}, "along": "race/dash"}]),
            );
        });
        codes_at(&c, "DW1117")
    };
    assert_eq!(
        walk(false),
        ["/content/quests/0/on_objective_complete/obj~1talk/0"]
    );
    assert!(walk(true).is_empty());
    // A course with no ride beside it.
    let c = with(|q| {
        q["races"] = json!([race()]);
        q["quests"][0]["objectives"][0]["course"] = json!("race/dash");
    });
    assert_eq!(
        codes_at(&c, "DW1117"),
        ["/content/quests/0/objectives/0/course"]
    );
}

#[test]
fn every_reference_to_a_race_resolves() {
    let c = with(|q| {
        let mut r = race();
        r["racers"] = json!(["actor/nobody"]);
        r["board"] = json!("board/nowhere");
        q["races"] = json!([r]);
        q["quests"][0]["objectives"][0]["course"] = json!("race/nowhere");
        on_talk(
            q,
            json!([{"type": "start-race", "race": "race/nowhere"},
                   {"type": "end-race", "race": "race/nowhere"}]),
        );
    });
    let mut at = codes_at(&c, "DW0112");
    at.sort();
    assert_eq!(
        at,
        [
            "/content/quests/0/objectives/0/course",
            "/content/quests/0/on_objective_complete/obj~1talk/0",
            "/content/quests/0/on_objective_complete/obj~1talk/1",
            "/content/races/0/board",
            "/content/races/0/racers/0",
        ]
    );
    let dup = with(|q| q["races"] = json!([race(), race()]));
    assert!(!codes_at(&dup, "DW0111").is_empty());
}
