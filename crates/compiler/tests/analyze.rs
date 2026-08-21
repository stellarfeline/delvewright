//! Lighting-mitigation gate relocation (spec-0010).
//!
//! The dark-prefab lighting gate (`DW0210`) used to live in `analyze_campaign`,
//! judging per-piece admission profiles. spec-0010 moved it to the compiler's
//! assembled-world light model (`crate::light`), which measures real light over
//! the placed geometry. This file pins the relocation: `analyze_campaign` no
//! longer emits `DW0210`, even for a campaign whose bound prefab is profiled
//! `dark`. The new measured gate is exercised in `light.rs` (assembled-light unit
//! tests) and `relight.rs` (spec-0010 acceptance criteria).

mod common;

use delvewright_compiler::analyze::analyze_campaign;
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{
    AnchorRegistry, Lighting, LightingProfile, PoolId, PrefabId, RawCampaign, parse_campaign,
};
use std::collections::BTreeSet;

/// Wraps the real prefab registry but forces every prefab's lighting profile to
/// `dark` — the input that used to trip the old `analyze_campaign` gate.
struct ForceDark<'a>(&'a PrefabRegistry);

impl AnchorRegistry for ForceDark<'_> {
    fn anchors_for(&self, prefab: &PrefabId) -> Option<&BTreeSet<String>> {
        self.0.anchors_for(prefab)
    }
    fn has_pool(&self, pool: &PoolId) -> bool {
        self.0.has_pool(pool)
    }
    fn lighting_for(&self, _prefab: &PrefabId) -> Option<Lighting> {
        Some(Lighting {
            profile: LightingProfile::Dark,
            measured_min_light: Some(0),
            measured: Some("2026-07-30".to_string()),
            rationale: Some("a deliberately unlit crypt".to_string()),
            method: None,
        })
    }
}

fn hello_world_raw() -> RawCampaign {
    load_campaign_dir(&common::hello_world_dir()).unwrap().raw
}

/// spec-0010: `analyze_campaign` no longer emits `DW0210`, even when every bound
/// prefab is profiled `dark` (the admission profile is no longer a gating input).
#[test]
fn analyze_no_longer_emits_dw0210_for_dark_profile() {
    let campaign = parse_campaign(&hello_world_raw()).expect("valid campaign parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let diags = analyze_campaign(&campaign, &ForceDark(&prefabs));
    assert!(
        !diags.iter().any(|d| d.code == "DW0210"),
        "DW0210 moved to the assembled-light model; analyze must not emit it: {diags:#?}"
    );
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
