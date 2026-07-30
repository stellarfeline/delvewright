//! Dark-prefab lighting-mitigation analysis (spec-0001 "Lighting contract",
//! DW0210). Fixture pair: a reachable `dark` prefab with no night-vision item in
//! any class kit fails analysis; the same campaign with a night-vision potion in
//! the kit passes.

mod common;

use delvewright_compiler::analyze::analyze_campaign;
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{
    AnchorRegistry, Lighting, LightingProfile, PoolId, PrefabId, RawCampaign, parse_campaign,
};
use std::collections::BTreeSet;

/// Wraps the real prefab registry but forces every prefab's lighting to `dark`,
/// so the mitigation check has a dark prefab to react to without shipping a dark
/// `.nbt` fixture (analysis reads metadata only).
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
            measured_min_light: 0,
            measured: "2026-07-30".to_string(),
            rationale: Some("a deliberately unlit crypt".to_string()),
            method: None,
        })
    }
}

fn hello_world_raw() -> RawCampaign {
    load_campaign_dir(&common::hello_world_dir()).unwrap().raw
}

/// A classes.json whose kit carries a night-vision potion (the v0.2 mitigation).
const NIGHT_VISION_CLASSES: &str = r#"{
  "dsl_version": "0.2.0",
  "campaign_id": "hello-world",
  "stage": "classes",
  "content": {
    "classes": [
      {
        "id": "class/wanderer",
        "name": "Wanderer",
        "blurb": "Carries a light.",
        "kit": [
          { "item": "minecraft:potion", "count": 1, "name": "Potion of Night Vision" }
        ]
      }
    ]
  }
}"#;

#[test]
fn dark_prefab_without_mitigation_fails_analysis() {
    let campaign = parse_campaign(&hello_world_raw()).expect("valid campaign parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let diags = analyze_campaign(&campaign, &ForceDark(&prefabs));
    assert!(
        diags.iter().any(|d| d.code == "DW0210"),
        "expected DW0210 (dark prefab, no night-vision kit): {diags:#?}"
    );
}

#[test]
fn dark_prefab_with_night_vision_kit_passes() {
    let mut raw = hello_world_raw();
    raw.classes = NIGHT_VISION_CLASSES.to_string();
    let campaign = parse_campaign(&raw).expect("valid campaign parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let diags = analyze_campaign(&campaign, &ForceDark(&prefabs));
    assert!(
        !diags.iter().any(|d| d.code == "DW0210"),
        "night-vision kit should mitigate the dark prefab: {diags:#?}"
    );
}

/// Control: the shipped hello-room is `lit`, so the real registry yields no
/// DW0210 for the unmodified campaign.
#[test]
fn lit_prefab_needs_no_mitigation() {
    let campaign = parse_campaign(&hello_world_raw()).expect("valid campaign parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let diags = analyze_campaign(&campaign, &prefabs);
    assert!(
        !diags.iter().any(|d| d.code == "DW0210"),
        "the lit hello-room must not trip the dark-mitigation check: {diags:#?}"
    );
}
