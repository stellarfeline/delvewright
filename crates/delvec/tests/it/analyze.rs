//! `analyze_campaign` over the shipped hello-world campaign.
//!
//! Light is judged on the assembled world (`DW0210`, `compiler::light`), never
//! on a prefab's profile: the registry the analyzer is handed has no lighting
//! accessor at all. That gate is exercised in `light.rs` (assembled-light unit
//! tests) and `relight.rs` (spec-0010 acceptance criteria).

mod common;

use delvec::compiler::analyze::analyze_campaign;
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::{RawCampaign, parse_campaign};

fn hello_world_raw() -> RawCampaign {
    load_campaign_dir(&common::hello_world_dir()).unwrap().raw
}

/// Control: the shipped hello-world campaign analyzes clean (reachability only).
#[test]
fn hello_world_analyzes_clean() {
    let campaign = parse_campaign(&hello_world_raw()).expect("valid campaign parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let diags = analyze_campaign(&campaign, &prefabs);
    assert!(
        diags.is_empty(),
        "hello-world must analyze clean: {diags:#?}"
    );
}
