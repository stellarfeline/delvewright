//! **What invokes the layout graph's reachability proofs** (spec-0049 §3.3).
//!
//! The rules themselves are `delvewright_dsl::layout::reachability` and are
//! tested against hand-drawn maps in `crates/dsl/tests/v13_layout_graph.rs`.
//! What that file cannot show is the half CLAUDE.md calls UNRUN: a check can be
//! correct in every reviewable way and still protect nothing, because the
//! obligation to run it lives in a doc line.
//!
//! So this file asserts the binding rather than the rule, and it asserts it in
//! BOTH directions, because the binding moved. The proofs used to be raised from
//! `analyze_campaign`; measured, that pass runs only on a campaign that already
//! validates, so a campaign at the graph step — carrying `DW0150` because the
//! plan is written and stage 5 is not — never reached them at all. They now run
//! from `dsl::validate::validate_campaign`, which every `delvec` subcommand goes
//! through. The two assertions are: validation raises the finding, and the
//! analysis pass does NOT raise it a second time. A rule in two batteries is a
//! rule that can be repaired in one of them.
//!
//! The demonstration is a campaign whose graph strands a body, put through each
//! entry point with nothing else touched.

use delvewright_compiler::analyze::analyze_campaign;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::validate::validate_campaign;
use delvewright_dsl::{RawCampaign, parse_campaign};

mod common;

/// A two-place map with a one-way drop into a room that has no way out. Drawn by
/// hand; the fault is stated in the comment above it, not computed.
const STRANDED: &str = r#"{
  "campaign_id": "hello-world",
  "dsl_version": "0.13.0",
  "stage": "layout-graph",
  "content": {
    "nodes": [
      { "id": "node/porch", "intent": "threshold", "size_class": "alcove" },
      { "id": "node/pit", "intent": "oubliette", "size_class": "alcove" }
    ],
    "edges": [
      { "id": "edge/pit-drop", "class": "drop", "a": "node/porch", "b": "node/pit",
        "falls": "a-to-b" }
    ],
    "entry": "node/porch",
    "goal": "node/porch",
    "critical_path": ["node/porch"],
    "beats": [
      { "quest": "quest/open-the-door", "objective": "obj/talk", "node": "node/porch" },
      { "quest": "quest/open-the-door", "objective": "obj/exit", "node": "node/porch" }
    ]
  }
}"#;

fn campaign_with_graph(graph: Option<&str>) -> delvewright_dsl::Campaign {
    let hw = common::hello_world_dir();
    let read = |n: &str| std::fs::read_to_string(hw.join(n)).expect("read stage document");
    let raw = RawCampaign {
        world: read("world.json"),
        npcs: read("npcs.json"),
        classes: read("classes.json"),
        quest_plan: read("quest-plan.json"),
        quests: read("quests.json"),
        dialogue: read("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: graph.map(str::to_string),
        site_plan: None,
        detail_plan: None,
    };
    parse_campaign(&raw).expect("the fixture parses")
}

/// The binding: the validation pass raises the graph's findings.
///
/// If someone deleted the one call in `dsl::layout::check`, every rule test in
/// the DSL crate would still pass — the rules would still be right — and no
/// campaign would ever be judged by them again. This is the test that would go
/// red.
#[test]
fn the_validation_pass_runs_the_graph_proofs() {
    let c = campaign_with_graph(Some(STRANDED));
    let codes: Vec<String> = validate_campaign(&c).into_iter().map(|d| d.code).collect();
    assert!(
        codes.contains(&"DW0819".to_string()),
        "the drop into `node/pit` strands and `validate_campaign` did not say so: {codes:?}"
    );
}

/// The other half, and the reason it is its own assertion: **the analysis pass
/// must not raise the same finding a second time.**
///
/// The proofs were moved out of `analyze_campaign`, not copied out of it. A rule
/// living in two batteries is one an author sees twice and a maintainer can
/// repair in one of them, and nothing else in this workspace would notice — the
/// rule tests pass either way. So this asserts the absence, over the same
/// stranding graph the assertion above uses.
#[test]
fn the_analysis_pass_does_not_raise_them_a_second_time() {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir())
        .expect("the hello-world prefab library loads");
    let c = campaign_with_graph(Some(STRANDED));
    let codes: Vec<String> = analyze_campaign(&c, &prefabs)
        .into_iter()
        .map(|d| d.code)
        .collect();
    for code in ["DW0816", "DW0817", "DW0819"] {
        assert!(
            !codes.contains(&code.to_string()),
            "{code} is the validation battery's and the analysis pass raised it too: {codes:?}"
        );
    }
}

/// The third side of the same binding: the pass must be silent on a campaign
/// that carries no graph. A check that fired anyway would be examining a
/// document that is not there.
///
/// Stated over the CODES the layout module owns rather than over a diagnostic
/// count. A count was the earlier form and it measured the wrong thing: a
/// campaign that carries a graph also draws the two advisories the graph step
/// always draws — the pacing projection and the uncalibrated-standards notice —
/// so "one more diagnostic than without" was a statement about how many
/// advisories the module happened to print, and it went red the moment the
/// battery this file is about moved into the same pass as them.
#[test]
fn a_campaign_with_no_graph_is_untouched_by_the_graph_proofs() {
    let layout_codes = |c: &delvewright_dsl::Campaign| -> Vec<String> {
        validate_campaign(c)
            .into_iter()
            .map(|d| d.code)
            .filter(|c| c.starts_with("DW081") || c.starts_with("DW082"))
            .collect()
    };
    let with = layout_codes(&campaign_with_graph(Some(STRANDED)));
    let without = layout_codes(&campaign_with_graph(None));
    assert!(
        without.is_empty(),
        "no graph is present and the layout module spoke anyway: {without:?}"
    );
    assert!(
        with.contains(&"DW0819".to_string()),
        "the graph is present and strands, and the module did not say so: {with:?}"
    );
}
