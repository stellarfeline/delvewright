//! **A gate pair with no satisfiable state: no site-plan campaign could carry a
//! stage-7 edit script.**
//!
//! Two rules, each right on its own terms, whose union had no green state:
//!
//! * a site-plan campaign **must** declare an empty `areas[]` (`DW0839`) — its
//!   one place is the site the plan lays out, `area/site`;
//! * the edit script's area set was built from `world.content.areas` **alone**,
//!   which for such a campaign can only be empty.
//!
//! So every batch of every stage-7 script on a site-plan campaign was `DW0112`,
//! and the repair prescribed by the message — *use one of the world stage's area
//! ids* — names a set the other rule guarantees is empty.
//!
//! The tell that it was an oversight rather than a decision: `validate.rs` has
//! exactly three area-set constructions, and the other two both insert
//! `SITE_AREA`, each with a comment saying why.
//!
//! What it cost is not one check but a route: every build-tier refusal a stage-7
//! script is the only way to reach was unreachable from a site-plan campaign's
//! documents. Such a check can be live, correct and unit-tested against a world
//! built by hand, and still have nothing able to get content to it.
//!
//! The campaign here is the gallery's own committed site-plan overlay, so the
//! fixture is a real site-plan campaign rather than a hand-built stand-in.

use delvewright_dsl::{RawCampaign, check_campaign};

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
    }
}

/// A stage-7 document with one batch on `area`, carrying `edits` verbatim.
fn script(area: &str, edits: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "0.12.0",
  "campaign_id": "gallery",
  "stage": "world-edits",
  "content": {{
    "batches": [
      {{ "id": "batch/a-way-out", "area": "{area}", "edits": [{edits}] }}
    ]
  }}
}}"#
    )
}

fn has_dangling_area(raw: &RawCampaign) -> bool {
    check_campaign(raw)
        .iter()
        .any(|d| d.code == "DW0112" && d.message.contains("batch/a-way-out"))
}

/// The whole finding, isolated to the area set: an **empty** batch, so nothing
/// but the `area` field can be the cause.
#[test]
fn an_empty_batch_on_the_site_is_not_a_dangling_area() {
    assert!(
        !has_dangling_area(&site_plan_campaign(&script("area/site", ""))),
        "a site-plan campaign's one place is `area/site`, and a batch may name it"
    );
}

/// And the route is genuinely open, not merely un-refused at the area field: a
/// real detailing batch — a `select` in the only frame a site-plan campaign can
/// use, plus the `fill` it feeds — validates. `piece-local` is not available to
/// such a campaign, because it has no bound piece to be local to.
#[test]
fn a_real_edit_script_reaches_a_site_plan_campaign() {
    let raw = site_plan_campaign(&script(
        "area/site",
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

/// The check is corrected, never weakened: an area id nothing declares is still
/// refused, on the same campaign, through the same field.
#[test]
fn an_area_the_campaign_does_not_declare_is_still_dw0112() {
    assert!(
        has_dangling_area(&site_plan_campaign(&script("area/nowhere", ""))),
        "an undeclared area id must still be DW0112"
    );
}

/// And `area/site` is not a name that resolves anywhere: on a campaign with an
/// ordinary `areas[]` and no site plan, it is a dangling reference exactly as
/// before.
#[test]
fn the_site_area_does_not_resolve_without_a_site_plan() {
    let mut raw = site_plan_campaign(&script("area/site", ""));
    let gallery = repo_root().join("gallery");
    raw.world = std::fs::read_to_string(gallery.join("world.json")).unwrap();
    raw.site_plan = None;
    raw.geometry_brief = None;
    raw.layout_graph = None;
    raw.detail_plan = None;
    assert!(
        has_dangling_area(&raw),
        "without a site plan there is no `area/site`, and naming it is still DW0112"
    );
}
