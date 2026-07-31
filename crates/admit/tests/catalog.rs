//! Catalog card schema + license-allowlist validation.

use delvewright_admit::catalog::{CatalogCard, license_allowed};

fn card_json(quality: u8, spdx: &str, source: &str, url: &str) -> String {
    let url_field = if url.is_empty() {
        String::new()
    } else {
        format!(",\n    \"url\": \"{url}\"")
    };
    format!(
        r#"{{
  "asset_id": "acme/keep/gatehouse",
  "description": "A ruined stone gatehouse. Two floors, an arched entry, moss on the north face.",
  "tags": {{
    "theme": ["keep"],
    "era_style": "medieval",
    "palette": ["stone_bricks", "cobblestone"],
    "condition": "ruined",
    "scale_class": "medium",
    "piece_types": ["set-piece"],
    "interior_exterior": "both",
    "biome_fit": ["plains"]
  }},
  "style_fit": {{ "verdict": "approve", "rationale": "matches the stone-keep tileset" }},
  "quality": {quality},
  "renders": ["catalog/renders/acme-keep-gatehouse-ext-ne.png"],
  "demand_categories": ["set-pieces"],
  "license": {{ "spdx": "{spdx}", "source": "{source}"{url_field} }}
}}"#
    )
}

#[test]
fn valid_card_parses_and_validates() {
    let card = CatalogCard::from_json(&card_json(
        4,
        "CC0-1.0",
        "modrinth",
        "https://modrinth.com/x",
    ))
    .unwrap();
    assert!(card.validate().is_empty());
    assert_eq!(card.asset_id, "acme/keep/gatehouse");
    // round-trips through canonical JSON.
    let re = CatalogCard::from_json(&card.to_json()).unwrap();
    assert_eq!(re.quality, 4);
}

#[test]
fn deny_unknown_fields_rejects_extra_keys() {
    let mut v: serde_json::Value =
        serde_json::from_str(&card_json(4, "CC0-1.0", "modrinth", "https://x")).unwrap();
    v["surprise"] = serde_json::json!("nope");
    let err = CatalogCard::from_json(&v.to_string()).unwrap_err();
    assert!(
        err.contains("surprise") || err.to_lowercase().contains("unknown"),
        "{err}"
    );
}

#[test]
fn quality_out_of_range_is_an_error() {
    let card = CatalogCard::from_json(&card_json(6, "CC0-1.0", "modrinth", "https://x")).unwrap();
    let diags = card.validate();
    assert!(diags.iter().any(|d| d.code == "DW0740"));
}

#[test]
fn nc_nd_sa_and_unknown_licenses_are_rejected() {
    for bad in [
        "CC-BY-NC-4.0",
        "CC-BY-ND-4.0",
        "CC-BY-SA-4.0",
        "WTFPL",
        "Proprietary",
    ] {
        assert!(license_allowed(bad).is_err(), "{bad} should be rejected");
        let card = CatalogCard::from_json(&card_json(3, bad, "modrinth", "https://x")).unwrap();
        assert!(
            card.validate().iter().any(|d| d.code == "DW0741"),
            "{bad} should raise DW0741"
        );
    }
}

#[test]
fn allowed_licenses_pass() {
    for ok in [
        "CC0-1.0",
        "CC-BY-4.0",
        "MIT",
        "Apache-2.0",
        "GPL-3.0-or-later",
        "original",
    ] {
        assert!(license_allowed(ok).is_ok(), "{ok} should be allowed");
    }
}

#[test]
fn non_original_license_needs_a_url() {
    // CC-BY with no url: "free download" is not proof of licensing.
    let card = CatalogCard::from_json(&card_json(3, "CC-BY-4.0", "planetminecraft", "")).unwrap();
    assert!(card.validate().iter().any(|d| d.code == "DW0741"));
    // an original asset needs no external url.
    let card = CatalogCard::from_json(&card_json(3, "original", "original", "")).unwrap();
    assert!(card.validate().is_empty());
}
