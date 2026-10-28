//! spec-0124: a device does what it reads as — the document arms.
//!
//! A `happening` that says opens or seals is held to its node's own effects
//! (`DW1070`), and a door-shut seam stands only where a door can and its key
//! writes no `prop` and no `release` (`DW1069` shapes c and d). Read over
//! hello-world with its quest's objectives and effects replaced, and over the
//! gallery's site-plan point with one edge declared door-shut.

mod common;

use delvec::compiler::happening::{self, DW_OPENS_NOTHING};
use delvec::compiler::nav::device::{self, DW_DOOR_NO_WAY};
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};
use serde_json::{Value, json};

fn read(dir: &std::path::Path, name: &str) -> String {
    std::fs::read_to_string(dir.join(name)).unwrap()
}

fn read_opt(dir: &std::path::Path, name: &str) -> Option<String> {
    std::fs::read_to_string(dir.join(name)).ok()
}

/// hello-world whose first objective carries `happening` and completes on
/// `effects`.
fn with_happening(happening: Value, effects: Value) -> Campaign {
    let dir = common::hello_world_dir();
    let mut q: Value = serde_json::from_str(&read(&dir, "quests.json")).unwrap();
    q["content"]["quests"][0]["objectives"][0]["happening"] = happening;
    q["content"]["quests"][0]["on_objective_complete"] = json!({ "obj/talk": effects });
    let raw = RawCampaign {
        world: read(&dir, "world.json"),
        npcs: read(&dir, "npcs.json"),
        classes: read(&dir, "classes.json"),
        quest_plan: read(&dir, "quest-plan.json"),
        quests: q.to_string(),
        dialogue: read(&dir, "dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    };
    parse_campaign(&raw).expect("campaign parses")
}

fn opens(subject: Option<&str>) -> Value {
    let mut h = json!({"verb": "opens", "text": "the door opens"});
    if let Some(s) = subject {
        h["subject"] = json!(s);
    }
    h
}

/// **spec-0124 criterion 7**: `opens` with no opening effect is refused; with
/// an `open-gate` on the named subject it is green; with an `open-gate` on
/// another anchor, refused; `seals` reads `close-gate` the same way; a
/// `gains` happening on the same node is not judged.
#[test]
fn a_step_that_says_it_opens_a_thing_opens_it_dw1070() {
    assert_eq!(DW_OPENS_NOTHING.id(), "DW1070");
    let refused = |c: &Campaign| {
        let (d, _) = happening::check(c);
        d.into_iter()
            .filter(|x| x.path == "/content/quests/0/objectives/0/happening")
            .collect::<Vec<_>>()
    };
    let none = with_happening(opens(Some("anchor/door")), json!([]));
    let d = refused(&none);
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].code, "DW1070");
    assert!(
        d[0].message.contains("it opens nothing"),
        "{}",
        d[0].message
    );
    let gate = |a: &str| json!([{"type": "open-gate", "anchor": a}]);
    assert!(
        refused(&with_happening(
            opens(Some("anchor/door")),
            gate("anchor/door")
        ))
        .is_empty()
    );
    assert!(refused(&with_happening(opens(None), gate("anchor/door"))).is_empty());
    let other = refused(&with_happening(
        opens(Some("anchor/door")),
        gate("anchor/exit"),
    ));
    assert_eq!(other.len(), 1, "{other:?}");
    assert!(
        other[0].message.contains("is not the thing it opens"),
        "{}",
        other[0].message
    );
    let seals = json!({"verb": "seals", "text": "the door shuts", "subject": "anchor/door"});
    assert_eq!(
        refused(&with_happening(seals.clone(), gate("anchor/door"))).len(),
        1
    );
    let close = json!([{"type": "close-gate", "anchor": "anchor/door"}]);
    assert!(refused(&with_happening(seals, close)).is_empty());
    let gains = json!({"verb": "gains", "text": "the party gains a key"});
    let c = with_happening(gains, json!([]));
    let (_, before) = happening::check(&with_happening(opens(None), json!([])));
    let (d, after) = happening::check(&c);
    assert!(refused(&c).is_empty(), "{d:?}");
    assert_eq!(
        after.judged + 1,
        before.judged,
        "a `gains` is counted, not judged"
    );
    assert_eq!(after.happenings, before.happenings);
    assert!(
        after.line().starts_with("happening binding (DW1070): "),
        "{}",
        after.line()
    );
}

/// The gallery's site-plan point as a campaign, with `edit` applied to its
/// layout graph and quests documents (as JSON values).
fn site_plan(edit: impl FnOnce(&mut Value, &mut Value)) -> Campaign {
    let root = common::repo_root().join("gallery");
    let over = root.join("overlays/site-plan");
    let pick = |name: &str| read_opt(&over, name).unwrap_or_else(|| read(&root, name));
    let mut graph: Value = serde_json::from_str(&pick("layout-graph.json")).unwrap();
    let mut quests: Value = serde_json::from_str(&pick("quests.json")).unwrap();
    edit(&mut graph, &mut quests);
    let raw = RawCampaign {
        world: pick("world.json"),
        npcs: pick("npcs.json"),
        classes: pick("classes.json"),
        quest_plan: pick("quest-plan.json"),
        quests: quests.to_string(),
        dialogue: pick("dialogue.json"),
        world_edits: read_opt(&over, "world-edits.json"),
        geometry_brief: read_opt(&over, "geometry-brief.json"),
        layout_graph: Some(graph.to_string()),
        site_plan: read_opt(&over, "site-plan.json"),
        detail_plan: read_opt(&over, "detail-plan.json"),
        design: read_opt(&over, "design.json"),
    };
    parse_campaign(&raw).expect("the site-plan point parses")
}

fn set_shut(graph: &mut Value, edge: &str) {
    let edges = graph["content"]["edges"].as_array_mut().unwrap();
    let e = edges
        .iter_mut()
        .find(|e| e["id"] == edge)
        .unwrap_or_else(|| panic!("{edge}"));
    e["gating"]["shut"] = json!("door");
}

/// **spec-0124 criterion 1, shape c**: `shut: door` on a stair, a climb, a
/// drop, or a walk whose seam opening is not one wide and two high, is
/// refused; on a walk through a `door` opening it is admitted.
#[test]
fn a_door_shut_seam_stands_only_where_a_door_can_dw1069_c() {
    assert_eq!(DW_DOOR_NO_WAY.id(), "DW1069");
    let shape_c = |edge: &str| {
        let c = site_plan(|g, _| set_shut(g, edge));
        device::check_document(&c)
            .into_iter()
            .filter(|d| d.message.starts_with("shape c"))
            .collect::<Vec<_>>()
    };
    for edge in [
        "edge/loft-stair",
        "edge/undercroft-crawl",
        "edge/loft-drop",
        "edge/undercroft-shaft",
    ] {
        let d = shape_c(edge);
        assert_eq!(d.len(), 1, "{edge}: {d:?}");
        assert_eq!(d[0].code, "DW1069");
    }
    // `east-door` is a walk through an arch (two wide): no door fits it.
    let arch = shape_c("edge/east-door");
    assert_eq!(arch.len(), 1, "{arch:?}");
    assert!(arch[0].message.contains("2 wide"), "{}", arch[0].message);
    // `centre-door` is a walk through a `door` opening.
    assert!(shape_c("edge/centre-door").is_empty());
}

/// **spec-0124 criterion 1, shape d**: the key anchored at a door-shut seam
/// with a `prop` or a `release` written is refused.
#[test]
fn the_key_at_a_door_shut_seam_writes_no_prop_dw1069_d() {
    let with_key = |extra: Value| {
        site_plan(|g, q| {
            set_shut(g, "edge/centre-door");
            let mut key = json!({
                "type": "interact", "id": "obj/unbar-the-centre",
                "anchor": "anchor/seam-centre-door",
                "requires_item": "minecraft:tripwire_hook",
                "title": "Unlock the centre door"
            });
            for (k, v) in extra.as_object().unwrap() {
                key[k] = v.clone();
            }
            q["content"]["quests"][0]["objectives"]
                .as_array_mut()
                .unwrap()
                .push(key);
        })
    };
    let shape_d = |c: &Campaign| {
        device::check_document(c)
            .into_iter()
            .filter(|d| d.message.starts_with("shape d"))
            .collect::<Vec<_>>()
    };
    assert!(shape_d(&with_key(json!({}))).is_empty());
    let prop = shape_d(&with_key(
        json!({"prop": {"block": "minecraft:oak_door[half=lower]"}}),
    ));
    assert_eq!(prop.len(), 1, "{prop:?}");
    assert!(prop[0].path.ends_with("/prop"), "{}", prop[0].path);
    let release = shape_d(&with_key(json!({"release": true})));
    assert_eq!(release.len(), 1, "{release:?}");
    assert!(release[0].path.ends_with("/release"));
}
