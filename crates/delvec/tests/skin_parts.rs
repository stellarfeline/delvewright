//! spec-0097: the skin toolchain and the compiler read one model-part table,
//! and what the composer writes is what the compiler admits.
//!
//! - The `SkinLayer` members a creator may hide are the layers the table read
//!   from the pinned jar, one for one.
//! - Every golden sheet `delve_skin` commits, judged by the compiler's own rule
//!   against the model its cast entry names, draws neither `DW0978` nor
//!   `DW0979` — so the composer cannot produce either refusal, and the claim is
//!   checked through the rule rather than a second copy of it.

mod common;

use std::collections::BTreeSet;
use std::path::Path;

use delvec::compiler::skinparts;
use delvewright_dsl::SkinLayer;

const SKIN_TOOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tools/creator/skin/tests/fixtures"
);

#[test]
fn the_layers_a_mannequin_hides_are_the_jars() {
    let ours: Vec<&str> = SkinLayer::ALL.iter().map(|l| l.token()).collect();
    assert_eq!(ours, skinparts::table().mannequin.layers);
    assert_eq!(skinparts::table().mannequin.field, "hidden_layers");
    for l in SkinLayer::ALL {
        let json = serde_json::to_string(&l).unwrap();
        assert_eq!(
            json,
            format!("\"{}\"", l.token()),
            "serde spells it as the jar does"
        );
    }
}

/// The table key a cast entry's sheet is drawn to, read the way `delve_skin`
/// reads it (`CastEntry.model_key`).
fn model_key(row: &serde_json::Value) -> String {
    match row.get("entity").and_then(|e| e.as_str()) {
        Some(e) if e != "mannequin" => e.to_string(),
        _ => match row["model"].as_str() {
            Some("slim") => "player_slim".to_string(),
            _ => "player".to_string(),
        },
    }
}

#[test]
fn every_composed_golden_is_a_sheet_the_compiler_admits() {
    let dir = Path::new(SKIN_TOOL);
    let mut sheets: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.to_string_lossy().ends_with(".cast.json"))
        .collect();
    sheets.sort();
    let mut judged = 0;
    let mut models = BTreeSet::new();
    for sheet in &sheets {
        let doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(sheet).unwrap()).unwrap();
        for row in doc["skins"].as_array().unwrap() {
            let id = row["texture_id"].as_str().unwrap();
            let key = model_key(row);
            let model = &skinparts::table().models[&key];
            let png = std::fs::read(dir.join("golden").join(format!("{id}.png"))).unwrap();
            let img = image::load_from_memory(&png).unwrap().to_rgba8();
            let opaque = skinparts::judge(&key, model, &img)
                .unwrap_or_else(|r| panic!("golden `{id}` on `{key}`: {} {}", r.code, r.reason));
            assert!(opaque > 0, "golden `{id}` paints nothing");
            judged += 1;
            models.insert(key);
        }
    }
    assert!(
        judged >= 7,
        "{judged} golden sheet(s) judged; the fixtures hold seven"
    );
    assert!(
        models.contains("zombie") && models.contains("drowned_outer_layer"),
        "the goldens reach a mob model and an outer layer: {models:?}"
    );
}

/// The sheet judgement states what it bound (spec-0097 §4.3): `validate` names
/// how many sheets it judged, of how many, against which model. On a campaign
/// with one skinned NPC that is one of one, on `player` — and a skin judged by
/// nothing would read zero here before it read green anywhere.
#[test]
fn validate_states_how_many_sheets_it_judged() {
    let campaign = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/v04-showcase");
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_delvec"))
        .args([
            "validate",
            campaign.to_str().unwrap(),
            "--prefabs",
            common::prefabs_dir().to_str().unwrap(),
        ])
        .output()
        .expect("run delvec");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let line = text
        .lines()
        .find(|l| l.starts_with("sheet binding: "))
        .unwrap_or_else(|| panic!("no sheet binding line:\n{text}"));
    assert!(
        line.starts_with("sheet binding: 1 of 1 sheet(s) judged")
            && line.contains("by model: player 1;")
            && line.ends_with("0 refused (DW0978/DW0979)"),
        "{line}"
    );
}
