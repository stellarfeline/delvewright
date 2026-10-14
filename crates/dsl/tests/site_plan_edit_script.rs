//! **A site-plan campaign carries a stage-7 edit script.**
//!
//! A batch names no place: its regions are framed by anchors the plan's
//! derivation places, so a real detailing batch — a `select` in an
//! anchor-relative frame plus the `fill` it feeds — validates on the gallery's
//! own site plan, the fixture a real campaign rather than a hand-built
//! stand-in.

use delvewright_dsl::{DSL_VERSION, RawCampaign, check_campaign};

fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .unwrap()
        .to_path_buf()
}

/// The gallery's site-plan overlay: the primary gallery with every document the
/// overlay replaces taken from the overlay instead.
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

/// A stage-7 document with one batch, carrying `edits` verbatim.
fn script(edits: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "{DSL_VERSION}",
  "campaign_id": "gallery",
  "stage": "world-edits",
  "content": {{
    "batches": [
      {{ "id": "batch/a-way-out", "edits": [{edits}] }}
    ]
  }}
}}"#
    )
}

/// A real detailing batch — a `select` in the only frame a site-plan campaign can
/// use, plus the `fill` it feeds — validates. `piece-local` is not available to
/// such a campaign, because it has no bound piece to be local to.
#[test]
fn a_real_edit_script_reaches_a_site_plan_campaign() {
    let raw = site_plan_campaign(&script(
        r#"{ "verb": "select", "name": "region/patch",
             "shape": { "kind": "box", "min": [1, 0, 1], "max": [3, 0, 3],
                        "frame": { "kind": "anchor-relative",
                                   "anchor": "anchor/node-annex" } } },
           { "verb": "fill", "region": "region/patch",
             "recipe": { "blocks": [ { "block": "minecraft:polished_blackstone",
                                       "weight": 1.0 } ] } }"#,
    ));
    // The claim is about the edit SCRIPT, so the assertion is over the
    // diagnostics that point into it. `check_campaign` is the pure-document pass
    // and carries no registries, so `DW0193` fires on every block id it is shown
    // — the CLI, which loads the 1.21.11 block registry, validates this same
    // campaign clean. Folding that in would make this a test of the registry.
    let refused: Vec<String> = check_campaign(&raw)
        .iter()
        .filter(|d| d.severity == delvewright_dsl::Severity::Error)
        .filter(|d| d.path.starts_with("/batches") || d.stage == "world-edits")
        .filter(|d| d.code != "DW0193")
        .map(|d| format!("{} at {}: {}", d.code, d.path, d.message))
        .collect();
    assert!(
        refused.is_empty(),
        "a site-plan campaign carrying a stage-7 edit script must validate: {refused:#?}"
    );
}
