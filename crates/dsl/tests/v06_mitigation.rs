//! A place's `mitigation` — the first-class darkness declaration that replaced
//! the `DW0210` display-name heuristic — written on the place's site-plan box
//! (`boxes[].mitigation`, spec-0106 §3.2).

mod common;

use delvewright_dsl::RawCampaign;

fn campaign(mitigation: Option<&str>) -> RawCampaign {
    let mut raw = common::valid_raw();
    if let Some(m) = mitigation {
        let plan = raw
            .site_plan
            .take()
            .expect("hello-world carries a site plan");
        raw.site_plan = Some(common::patch_doc(&plan, |v| {
            v["content"]["boxes"][0]["mitigation"] = serde_json::json!(m);
        }));
    }
    raw
}

/// `mitigation: "night-vision"` on a box validates clean.
#[test]
fn mitigation_validates_clean() {
    let diags = common::check_campaign(&campaign(Some("night-vision")));
    assert!(
        diags.is_empty(),
        "expected zero diagnostics for a box mitigation, got: {diags:#?}"
    );
}

/// An unknown `mitigation` value is a schema rejection (`DW0100`), not a silent
/// pass — the enum is closed.
#[test]
fn unknown_mitigation_value_is_dw0100() {
    let diags = common::check_campaign(&campaign(Some("renamed-potion")));
    assert!(
        diags.iter().any(|d| d.code == "DW0100"),
        "an unknown mitigation value must be a schema error: {diags:#?}"
    );
}

/// Absent `mitigation` validates — the field is optional.
#[test]
fn absent_mitigation_is_unchanged() {
    let diags = common::check_campaign(&campaign(None));
    assert!(
        diags.is_empty(),
        "absent mitigation must validate: {diags:#?}"
    );
}
