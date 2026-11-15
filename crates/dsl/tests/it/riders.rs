//! spec-0135 §13, document tier: a wave body on its mount, an actor seated
//! on a vehicle (`DW1114`), a
//! vehicle counted per player present (`DW1115`), a board of places
//! (`DW1116`), and a wave's spawn anchor resolved where it is written
//! (`DW0142`).

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

fn on_talk(content: &mut Value, effects: Value) {
    content["quests"][0]["on_objective_complete"]["obj/talk"] = effects;
}

fn racer() -> Value {
    // The document tier's registry is the vendored sixteen; the compiler's
    // is the pinned game's whole list.
    json!({"id": "actor/racer", "entity": "minecraft:zombie", "name": "Oskar",
           "anchor": "anchor/keeper-stand"})
}

/// A one-body pig on the racer's mark, seating it.
fn racer_pig() -> Value {
    json!({"id": "vehicle/racer-pig", "entity": "minecraft:pig",
           "at": {"anchor": "anchor/keeper-stand"},
           "bounds": {"anchor": "anchor/keeper-stand", "extent": [8, 3, 8]},
           "rider": "actor/racer"})
}

#[test]
fn a_wave_mob_takes_a_mount_and_an_actor_does_not() {
    let c = with(|q| {
        q["waves"] = json!([{"id": "wave/horsemen", "anchor": "anchor/keeper-stand",
            "mobs": [{"entity": "minecraft:skeleton", "count": 2,
                      "mount": {"entity": "minecraft:spider"}}]}]);
        q["actors"] = json!([racer()]);
    });
    let sites = delvewright_dsl::mount_sites(&c);
    assert_eq!(
        sites.iter().map(|s| s.path.as_str()).collect::<Vec<_>>(),
        ["/content/waves/0/mobs/0/mount"]
    );
    assert!(codes_at(&c, "DW0173").is_empty());
    // An actor rides a vehicle of `vehicles[]`: a `mount` on one is a field
    // the class does not have, refused as the document is read.
    let mut q: Value = serde_json::from_str(&hw("quests.json")).unwrap();
    q["dsl_version"] = json!(DSL_VERSION);
    let mut a = racer();
    a["mount"] = json!({"entity": "minecraft:pig"});
    q["content"]["actors"] = json!([a]);
    let err = delvewright_dsl::parse_campaign(&RawCampaign {
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
    .expect_err("an actor's `mount` is refused");
    assert!(format!("{err:?}").contains("mount"), "{err:?}");
}

#[test]
fn a_mount_of_no_known_kind_is_dw0173() {
    let c = with(|q| {
        q["waves"] = json!([{"id": "wave/horsemen", "anchor": "anchor/keeper-stand",
            "mobs": [{"entity": "minecraft:skeleton", "count": 1,
                      "mount": {"entity": "minecraft:unicorn"}}]}]);
    });
    assert_eq!(
        codes_at(&c, "DW0173"),
        ["/content/waves/0/mobs/0/mount/entity"]
    );
}

#[test]
fn a_vehicle_seats_a_declared_actor_on_its_own_mark() {
    let ok = with(|q| {
        q["actors"] = json!([racer()]);
        q["vehicles"] = json!([racer_pig()]);
    });
    assert!(
        codes_at(&ok, "DW1114").is_empty(),
        "{:#?}",
        validate_campaign(&ok)
    );
    assert_eq!(
        delvewright_dsl::vehicle_of_rider(&ok, "actor/racer").map(|v| v.id.as_str()),
        Some("vehicle/racer-pig")
    );
    let unknown = with(|q| q["vehicles"] = json!([racer_pig()]));
    assert_eq!(codes_at(&unknown, "DW0112"), ["/content/vehicles/0/rider"]);
    let apart = with(|q| {
        q["actors"] = json!([racer()]);
        let mut v = racer_pig();
        v["at"]["offset"] = json!([1, 0, 0]);
        q["vehicles"] = json!([v]);
    });
    assert_eq!(codes_at(&apart, "DW1114"), ["/content/vehicles/0/rider"]);
    let two = with(|q| {
        q["actors"] = json!([racer()]);
        let mut second = racer_pig();
        second["id"] = json!("vehicle/second-pig");
        q["vehicles"] = json!([racer_pig(), second]);
    });
    assert_eq!(codes_at(&two, "DW1114"), ["/content/vehicles/1/rider"]);
    let row = with(|q| {
        q["actors"] = json!([racer()]);
        let mut v = racer_pig();
        v["count"] = json!(2);
        v["spacing"] = json!([2, 0, 0]);
        q["vehicles"] = json!([v]);
    });
    assert!(codes_at(&row, "DW1114").contains(&"/content/vehicles/0/rider".to_string()));
    // A pig seats one: with its actor aboard, the party has no seat on it.
    let full = with(|q| {
        q["actors"] = json!([racer()]);
        q["vehicles"] = json!([racer_pig()]);
        q["quests"][0]["objectives"][0]["ride"] = json!("vehicle/racer-pig");
    });
    assert_eq!(
        codes_at(&full, "DW1114"),
        ["/content/quests/0/objectives/0/ride"]
    );
}

#[test]
fn a_hop_of_a_seated_actor_and_an_entrance_on_one_are_dw1114() {
    let hop = |q: &mut Value| {
        q["actors"] = json!([racer()]);
        q["vehicles"] = json!([racer_pig()]);
        on_talk(
            q,
            json!([{"type": "spawn-actor", "actor": "actor/racer"},
                   {"type": "hop", "height": 1, "audience": {"actor": "actor/racer"}}]),
        );
    };
    let c = with(hop);
    assert_eq!(
        codes_at(&c, "DW1114").len(),
        1,
        "{:#?}",
        validate_campaign(&c)
    );
    // Perturbation: the same hop of the actor on its own feet is no refusal.
    let c = with(|q| {
        hop(q);
        q["vehicles"][0].as_object_mut().unwrap().remove("rider");
    });
    assert!(codes_at(&c, "DW1114").is_empty());
    let c = with(|q| {
        let mut a = racer();
        a["entity"] = json!("minecraft:warden");
        a["entrance"] = json!("native");
        q["actors"] = json!([a]);
        q["vehicles"] = json!([racer_pig()]);
    });
    assert_eq!(codes_at(&c, "DW1114"), ["/content/actors/0/entrance"]);
}

#[test]
fn a_seated_actor_walks_a_mount_and_never_a_boat() {
    let walk = |entity: &str| {
        let entity = entity.to_string();
        with(move |q| {
            q["actors"] = json!([racer()]);
            let mut v = racer_pig();
            v["entity"] = json!(entity);
            q["vehicles"] = json!([v]);
            on_talk(
                q,
                json!([{"type": "spawn-actor", "actor": "actor/racer"},
                       {"type": "move-actor", "actor": "actor/racer",
                        "to": {"anchor": "anchor/keeper-stand"}, "speed": 0.1}]),
            );
        })
    };
    assert!(codes_at(&walk("minecraft:pig"), "DW1114").is_empty());
    assert_eq!(codes_at(&walk("minecraft:oak_boat"), "DW1114").len(), 1);
}

fn pig(count: Value) -> Value {
    json!({"id": "vehicle/pig", "entity": "minecraft:pig", "at": {"anchor": "anchor/keeper-stand"},
           "bounds": {"anchor": "anchor/keeper-stand", "extent": [8, 3, 8]},
           "count": count, "spacing": [2, 0, 0], "seated": "by-story"})
}

#[test]
fn a_count_per_player_is_seated_by_the_story_with_a_spacing() {
    let ok = with(|q| q["vehicles"] = json!([pig(json!({"base": 1, "per_player": 1}))]));
    assert!(
        codes_at(&ok, "DW1115").is_empty(),
        "{:#?}",
        validate_campaign(&ok)
    );
    let at_load = with(|q| {
        let mut v = pig(json!({"base": 1, "per_player": 1}));
        v.as_object_mut().unwrap().remove("seated");
        q["vehicles"] = json!([v]);
    });
    assert_eq!(codes_at(&at_load, "DW1115"), ["/content/vehicles/0/count"]);
    let no_step = with(|q| {
        let mut v = pig(json!(3));
        v.as_object_mut().unwrap().remove("spacing");
        q["vehicles"] = json!([v]);
    });
    assert_eq!(
        codes_at(&no_step, "DW1115"),
        ["/content/vehicles/0/spacing"]
    );
    let none = with(|q| q["vehicles"] = json!([pig(json!(0))]));
    assert_eq!(codes_at(&none, "DW1115"), ["/content/vehicles/0/count"]);
    // A vehicle of one body declares neither and is the vehicle it was.
    let one = with(|q| {
        let mut v = pig(json!(1));
        v.as_object_mut().unwrap().remove("spacing");
        v.as_object_mut().unwrap().remove("count");
        q["vehicles"] = json!([v]);
    });
    assert!(codes_at(&one, "DW1115").is_empty());
    assert_eq!(
        one.quests.content.vehicles[0].count,
        delvewright_dsl::MobCount::Fixed(1)
    );
}

#[test]
fn a_board_places_players_and_named_bodies_and_refuses_a_crowd() {
    let board = |q: &mut Value, place: Value| {
        q["boards"] = json!([{"id": "board/race", "title": "The Pig Race"}]);
        q["actors"] = json!([racer()]);
        q["waves"] = json!([{"id": "wave/pigs", "anchor": "anchor/keeper-stand",
            "mobs": [{"entity": "minecraft:pig", "count": 2}]}]);
        on_talk(
            q,
            json!([{"type": "show-board", "board": "board/race"}, place,
                   {"type": "hide-board", "board": "board/race"}]),
        );
    };
    let player = with(|q| {
        board(
            q,
            json!({"type": "place-on-board", "board": "board/race",
        "audience": "actor"}),
        )
    });
    assert!(
        codes_at(&player, "DW1116").is_empty(),
        "{:#?}",
        validate_campaign(&player)
    );
    let body = with(|q| {
        board(
            q,
            json!({"type": "place-on-board", "board": "board/race",
        "audience": {"actor": "actor/racer"}}),
        )
    });
    assert!(
        codes_at(&body, "DW1116").is_empty(),
        "{:#?}",
        validate_campaign(&body)
    );
    assert!(
        codes_at(&body, "DW0942").is_empty(),
        "{:#?}",
        validate_campaign(&body)
    );
    assert_eq!(delvewright_dsl::board_rows(&body, "board/race"), 1);
    assert_eq!(delvewright_dsl::board_rows(&player, "board/race"), 4);
    let npc = with(|q| {
        board(
            q,
            json!({"type": "place-on-board", "board": "board/race",
        "audience": {"npc": "npc/keeper"}}),
        )
    });
    assert!(codes_at(&npc, "DW1116").is_empty());
    assert!(
        codes_at(&npc, "DW0942").is_empty(),
        "{:#?}",
        validate_campaign(&npc)
    );
    let crowd = with(|q| {
        board(
            q,
            json!({"type": "place-on-board", "board": "board/race",
        "audience": {"wave": "wave/pigs"}}),
        )
    });
    assert_eq!(codes_at(&crowd, "DW1116").len(), 1);
    let unnamed = with(|q| {
        board(
            q,
            json!({"type": "place-on-board", "board": "board/race",
            "audience": {"actor": "actor/racer"}}),
        );
        q["actors"][0].as_object_mut().unwrap().remove("name");
    });
    assert_eq!(codes_at(&unnamed, "DW1116").len(), 1);
    let dangling = with(|q| {
        board(q, json!({"type": "show-board", "board": "board/nowhere"}));
    });
    assert_eq!(
        codes_at(&dangling, "DW0112").len(),
        1,
        "{:#?}",
        validate_campaign(&dangling)
    );
}

#[test]
fn a_wave_anchor_no_area_provides_is_dw0142_at_validate() {
    let wave = |anchor: &str| {
        let anchor = anchor.to_string();
        with(move |q| {
            q["waves"] = json!([{"id": "wave/w", "anchor": anchor,
                "mobs": [{"entity": "minecraft:zombie", "count": 1}]}]);
        })
    };
    assert_eq!(
        codes_at(&wave("anchor/nowhere"), "DW0142"),
        ["/content/waves/0/anchor"]
    );
    assert!(codes_at(&wave("anchor/keeper-stand"), "DW0142").is_empty());
}
