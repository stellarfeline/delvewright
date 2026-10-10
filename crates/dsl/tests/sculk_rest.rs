//! **The sculk rest rule refuses where a state is typed** (spec-0100 §4.1,
//! acceptance criterion 4): a `set-block` and a world-edit `fill` recipe naming
//! `sculk_shrieker[can_summon=true]` are `DW0998` at validation, and a
//! `fill-region` of `sculk_catalyst[bloom=true]` is `DW0999` — each naming the
//! document path, before any world is assembled. The build judges the assembled
//! world again with the same function; that call names a cell, never a path, so
//! a path here is what proves the entry point judged.

use delvewright_dsl::{DSL_VERSION, Diagnostic, RawCampaign, check_campaign};

fn hw(name: &str) -> String {
    std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/valid/hello-world")
            .join(name),
    )
    .unwrap()
}

fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .unwrap()
        .to_path_buf()
}

/// A hello-world `quests` doc whose `obj/talk` bundle carries `effects`.
fn quests_doc(effects: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "{DSL_VERSION}",
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
          "obj/talk": [ {{ "type": "open-gate", "anchor": "anchor/door" }}, {effects} ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ]
  }}
}}"#
    )
}

fn hello(quests: String) -> RawCampaign {
    RawCampaign {
        world: hw("world.json"),
        npcs: hw("npcs.json"),
        classes: hw("classes.json"),
        quest_plan: hw("quest-plan.json"),
        quests,
        dialogue: hw("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    }
}

/// The gallery's site-plan overlay carrying `world_edits` — a real campaign a
/// stage-7 script reaches.
fn site_plan_campaign(world_edits: &str) -> RawCampaign {
    let gallery = repo_root().join("gallery");
    let overlay = gallery.join("overlays/site-plan");
    let read = |name: &str| -> String {
        let from = if overlay.join(name).is_file() {
            overlay.join(name)
        } else {
            gallery.join(name)
        };
        std::fs::read_to_string(from).unwrap()
    };
    RawCampaign {
        world: read("world.json"),
        npcs: read("npcs.json"),
        classes: read("classes.json"),
        quest_plan: read("quest-plan.json"),
        quests: read("quests.json"),
        dialogue: read("dialogue.json"),
        world_edits: Some(world_edits.to_string()),
        geometry_brief: Some(read("geometry-brief.json")),
        layout_graph: Some(read("layout-graph.json")),
        site_plan: Some(read("site-plan.json")),
        detail_plan: Some(read("detail-plan.json")),
        design: Some(read("design.json")),
    }
}

fn with_code<'d>(d: &'d [Diagnostic], code: &str) -> Vec<&'d Diagnostic> {
    d.iter().filter(|x| x.code == code).collect()
}

#[test]
fn a_set_block_of_a_summoning_shrieker_is_refused_where_it_is_typed() {
    let d = check_campaign(&hello(quests_doc(
        r#"{ "type": "set-block", "anchor": "anchor/exit",
             "block": "minecraft:sculk_shrieker[can_summon=true]" }"#,
    )));
    let hits = with_code(&d, "DW0998");
    assert_eq!(hits.len(), 1, "{d:#?}");
    assert!(hits[0].path.ends_with("/block"), "{:?}", hits[0].path);
    assert!(hits[0].path.contains("on_objective_complete"), "{:?}", hits[0].path);
    // The rest state passes the same entry point.
    let d = check_campaign(&hello(quests_doc(
        r#"{ "type": "set-block", "anchor": "anchor/exit",
             "block": "minecraft:sculk_shrieker[can_summon=false]" }"#,
    )));
    assert!(with_code(&d, "DW0998").is_empty(), "{d:#?}");
    assert!(with_code(&d, "DW0999").is_empty(), "{d:#?}");
}

#[test]
fn a_fill_region_of_a_blooming_catalyst_is_not_at_rest() {
    let d = check_campaign(&hello(quests_doc(
        r#"{ "type": "fill-region",
             "region": { "anchor": "anchor/exit", "extent": [0, 0, 0] },
             "block": "minecraft:sculk_catalyst[bloom=true]" }"#,
    )));
    let hits = with_code(&d, "DW0999");
    assert_eq!(hits.len(), 1, "{d:#?}");
    assert!(hits[0].message.contains("`bloom`"), "{}", hits[0].message);
    assert!(hits[0].path.ends_with("/block"), "{:?}", hits[0].path);
}

#[test]
fn a_world_edit_fill_recipe_of_a_summoning_shrieker_is_refused_where_it_is_typed() {
    let script = format!(
        r#"{{
  "dsl_version": "{DSL_VERSION}",
  "campaign_id": "gallery",
  "stage": "world-edits",
  "content": {{
    "batches": [
      {{ "id": "batch/listen", "area": "area/site", "edits": [
        {{ "verb": "select", "name": "region/patch",
           "shape": {{ "kind": "box", "min": [1, 0, 1], "max": [1, 0, 1],
                      "frame": {{ "kind": "anchor-relative",
                                 "anchor": "anchor/node-annex" }} }} }},
        {{ "verb": "fill", "region": "region/patch",
           "recipe": {{ "blocks": [ {{ "block": "minecraft:sculk_shrieker[can_summon=true]",
                                     "weight": 1.0 }} ] }} }}
      ] }}
    ]
  }}
}}"#
    );
    let d = check_campaign(&site_plan_campaign(&script));
    let hits = with_code(&d, "DW0998");
    assert_eq!(hits.len(), 1, "{d:#?}");
    assert_eq!(hits[0].stage, "world-edits");
    assert!(
        hits[0].path.contains("/recipe/blocks/0/block"),
        "{:?}",
        hits[0].path
    );
}
