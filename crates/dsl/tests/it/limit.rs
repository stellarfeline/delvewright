//! spec-0133, document tier: the `limit` surface the schema exports, the
//! `despawn-wave` verb, and the refusals that need only the documents —
//! `DW1085` (a limit the player is not told about), `DW1086` (a clock that
//! starts after the thing it closes), `DW0100` (the window's bounds and an
//! empty `on_expire`) and `DW0913`'s `despawn-wave` arm.

use delvewright_dsl::{
    Campaign, DSL_VERSION, Diagnostic, RawCampaign, Stage, parse_campaign, stage_schema,
    validate_campaign,
};
use serde_json::{Value, json};

fn hw(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/valid/hello-world")
            .join(name),
    )
    .unwrap_or_else(|e| panic!("hello-world/{name}: {e}"))
}

/// A hello-world campaign with three titled objectives in a chain
/// (`obj/talk` → `obj/look` → `obj/exit`), each patched by `patch`, plus a
/// second quest whose one objective is `obj/elsewhere`.
fn campaign(patch: impl FnOnce(&mut Value)) -> Campaign {
    let mut quests = json!({
        "dsl_version": DSL_VERSION,
        "campaign_id": "hello-world",
        "stage": "quests",
        "content": {
            "quests": [{
                "id": "quest/open-the-door",
                "trigger": { "type": "campaign-start" },
                "objectives": [
                    { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper",
                      "title": "Talk to the keeper" },
                    { "type": "reach-anchor", "id": "obj/look", "anchor": "anchor/keeper-stand",
                      "radius": 2, "after": ["obj/talk"], "title": "Reach the stand" },
                    { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit",
                      "radius": 2, "after": ["obj/look"], "title": "Reach the door" }
                ],
                "on_objective_complete": {
                    "obj/talk": [ { "type": "open-gate", "anchor": "anchor/door" } ]
                },
                "on_complete": [ { "type": "campaign-complete" } ]
            }, {
                "id": "quest/elsewhere",
                "trigger": { "type": "quest-complete", "quest": "quest/open-the-door" },
                "objectives": [
                    { "type": "reach-anchor", "id": "obj/elsewhere", "anchor": "anchor/exit",
                      "radius": 2, "title": "Reach the door again" }
                ],
                "on_complete": []
            }]
        }
    });
    patch(&mut quests);
    let raw = RawCampaign {
        world: hw("world.json"),
        npcs: hw("npcs.json"),
        classes: hw("classes.json"),
        quest_plan: hw("quest-plan.json"),
        quests: quests.to_string(),
        dialogue: hw("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    };
    parse_campaign(&raw).expect("campaign parses")
}

fn limit_on(i: usize, limit: Value) -> impl FnOnce(&mut Value) {
    move |q| q["content"]["quests"][0]["objectives"][i]["limit"] = limit
}

fn with(code: &str, d: &[Diagnostic]) -> Vec<Diagnostic> {
    d.iter().filter(|x| x.code == code).cloned().collect()
}

/// Criterion 1: the exported schema carries `limit` on all five kinds, its
/// bounds, its three starts and `on_expire`'s `minItems`, and `despawn-wave`.
#[test]
fn the_schema_exports_the_limit_and_despawn_wave() {
    let s = serde_json::to_string(&stage_schema(Stage::Quests)).unwrap();
    let v: Value = serde_json::from_str(&s).unwrap();
    let defs = v["$defs"].as_object().or(v["definitions"].as_object()).expect("defs");
    let limit = &defs["Limit"];
    assert_eq!(limit["properties"]["ticks"]["minimum"], 20, "{limit}");
    assert_eq!(limit["properties"]["ticks"]["maximum"], 720000, "{limit}");
    assert_eq!(limit["required"], json!(["ticks"]), "{limit}");
    let on_expire = limit["properties"]["on_expire"].to_string();
    assert!(on_expire.contains("\"minItems\":1"), "{on_expire}");
    let from = defs["LimitFrom"].to_string();
    for arm in ["\"activation\"", "\"quest-start\"", "\"objective\""] {
        assert!(from.contains(arm), "{from}");
    }
    let objective = defs["Objective"].to_string();
    assert_eq!(
        objective.matches("\"limit\"").count(),
        5,
        "every objective kind carries `limit`: {objective}"
    );
    assert!(s.contains("\"despawn-wave\""), "the verb is exported");
}

/// A limit on an untitled objective, and on one whose announcement is
/// hidden, is `DW1085` at the objective's `limit`.
#[test]
fn a_clock_on_a_silent_objective_is_dw1085() {
    let c = campaign(|q| {
        let o = &mut q["content"]["quests"][0]["objectives"][2];
        o.as_object_mut().unwrap().remove("title");
        o["limit"] = json!({"ticks": 600});
    });
    let d = with("DW1085", &validate_campaign(&c));
    assert_eq!(d.len(), 1, "{d:#?}");
    assert_eq!(d[0].path, "/content/quests/0/objectives/2/limit");
    let c = campaign(|q| {
        let o = &mut q["content"]["quests"][0]["objectives"][2];
        o["announcement"] = json!("hidden");
        o["limit"] = json!({"ticks": 600});
    });
    let d = with("DW1085", &validate_campaign(&c));
    assert_eq!(d.len(), 1, "{d:#?}");
    assert!(d[0].message.contains("hidden"), "{}", d[0].message);
    // Announced: green.
    let c = campaign(limit_on(2, json!({"ticks": 600})));
    assert!(with("DW1085", &validate_campaign(&c)).is_empty());
}

/// `from` naming the objective itself, one of another quest, or a
/// non-ancestor of the same quest is `DW1086`; an ancestor two `after` hops
/// up is green.
#[test]
fn a_clock_that_starts_after_its_objective_is_dw1086() {
    for (from, why) in [
        ("obj/exit", "itself"),
        ("obj/elsewhere", "is not an objective of quest"),
    ] {
        let c = campaign(limit_on(2, json!({"ticks": 600, "from": {"objective": from}})));
        let d = with("DW1086", &validate_campaign(&c));
        assert_eq!(d.len(), 1, "{from}: {d:#?}");
        assert_eq!(d[0].path, "/content/quests/0/objectives/2/limit/from");
        assert!(d[0].message.contains(why), "{}", d[0].message);
    }
    // `obj/exit` is not an ancestor of `obj/look`: refused.
    let c = campaign(limit_on(1, json!({"ticks": 600, "from": {"objective": "obj/exit"}})));
    let d = with("DW1086", &validate_campaign(&c));
    assert_eq!(d.len(), 1, "{d:#?}");
    assert!(d[0].message.contains("not an `after`-ancestor"), "{}", d[0].message);
    // Two hops up: green.
    let c = campaign(limit_on(2, json!({"ticks": 600, "from": {"objective": "obj/talk"}})));
    assert!(with("DW1086", &validate_campaign(&c)).is_empty());
}

/// The window's bounds and an empty `on_expire` are `DW0100`.
#[test]
fn a_window_out_of_bounds_or_an_empty_expiry_is_dw0100() {
    for ticks in [19, 720_001] {
        let c = campaign(limit_on(2, json!({"ticks": ticks})));
        let d = with("DW0100", &validate_campaign(&c));
        assert!(
            d.iter().any(|x| x.path.ends_with("/limit/ticks")),
            "{ticks}: {d:#?}"
        );
    }
    for ticks in [20, 720_000] {
        let c = campaign(limit_on(2, json!({"ticks": ticks})));
        assert!(
            !with("DW0100", &validate_campaign(&c))
                .iter()
                .any(|x| x.path.contains("/limit")),
            "{ticks}"
        );
    }
    let c = campaign(limit_on(2, json!({"ticks": 600, "on_expire": []})));
    let d = with("DW0100", &validate_campaign(&c));
    assert!(d.iter().any(|x| x.path.ends_with("/limit/on_expire")), "{d:#?}");
}

/// A `despawn-wave` of a wave no beat seats removes nothing: `DW0913` at the
/// verb's path.
#[test]
fn despawn_wave_of_an_unseated_wave_is_dw0913() {
    let c = campaign(|q| {
        q["content"]["waves"] = json!([{
            "id": "wave/idle", "anchor": "anchor/exit",
            "mobs": [ { "entity": "minecraft:zombie", "count": 2 } ]
        }]);
        q["content"]["quests"][0]["on_objective_complete"]["obj/talk"]
            .as_array_mut()
            .unwrap()
            .push(json!({"type": "despawn-wave", "wave": "wave/idle"}));
    });
    let d = with("DW0913", &validate_campaign(&c));
    assert_eq!(d.len(), 1, "{d:#?}");
    assert_eq!(d[0].path, "/content/quests/0/on_objective_complete/obj/talk/1");
}

/// `DW1087` is declared nowhere: the planner allocated it and this spec
/// returned it.
#[test]
fn dw1087_is_declared_nowhere() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    fn walk(dir: &std::path::Path, hits: &mut Vec<String>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, hits);
            } else if p.extension().is_some_and(|x| x == "rs")
                && std::fs::read_to_string(&p).unwrap().contains("\"DW1087\"")
            {
                hits.push(p.display().to_string());
            }
        }
    }
    let mut hits = Vec::new();
    walk(&root, &mut hits);
    assert!(hits.is_empty(), "{hits:?}");
}
