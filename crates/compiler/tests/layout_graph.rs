//! **What invokes the layout graph's analysis-tier proofs** (spec-0049 §3.3).
//!
//! The rules themselves are `delvewright_dsl::layout::analyze` and are tested
//! against hand-drawn maps in `crates/dsl/tests/v13_layout_graph.rs`. What that
//! file cannot show is the half CLAUDE.md calls UNRUN: a check can be correct in
//! every reviewable way and still protect nothing, because the obligation to run
//! it lives in a doc line.
//!
//! So this file asserts the binding rather than the rule. `analyze_campaign` is
//! the one pass `delvec analyze` and `delvec build` both go through, and there is
//! no way to reach a built world around it — so a graph fault cannot be shipped
//! by anybody who simply did not run the graph checker. The demonstration is a
//! campaign whose graph strands a body, put through the compiler's own analysis
//! entry point with nothing else touched.

use delvewright_compiler::analyze::analyze_campaign;
use delvewright_compiler::registry::PrefabRegistry;
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
    };
    parse_campaign(&raw).expect("the fixture parses")
}

/// The binding: the compiler's own analysis pass raises the graph's findings.
///
/// If someone deleted the one call in `analyze_campaign`, every test in the DSL
/// crate would still pass — the rules would still be right — and no campaign
/// would ever be judged by them again. This is the test that would go red.
#[test]
fn the_compilers_analysis_pass_runs_the_graph_proofs() {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir())
        .expect("the hello-world prefab library loads");
    let c = campaign_with_graph(Some(STRANDED));
    let codes: Vec<String> = analyze_campaign(&c, &prefabs)
        .into_iter()
        .map(|d| d.code)
        .collect();
    assert!(
        codes.contains(&"DW0819".to_string()),
        "the drop into `node/pit` strands and `analyze_campaign` did not say so: {codes:?}"
    );
}

/// The other side of the same binding, and the reason it is a separate
/// assertion: the pass must be silent on a campaign that carries no graph. A
/// check that fired anyway would be examining a document that is not there.
#[test]
fn a_campaign_with_no_graph_is_untouched_by_the_graph_proofs() {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir())
        .expect("the hello-world prefab library loads");
    let with = campaign_with_graph(Some(STRANDED));
    let without = campaign_with_graph(None);
    let n_with = analyze_campaign(&with, &prefabs).len();
    let n_without = analyze_campaign(&without, &prefabs).len();
    assert_eq!(
        n_with,
        n_without + 1,
        "the graph contributed exactly its one finding and nothing else moved"
    );
}
