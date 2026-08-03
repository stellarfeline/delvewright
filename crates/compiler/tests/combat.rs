//! spec-0023 §2 — compile-time combat winnability (`DW0470`–`DW0475`).
//!
//! Every case is the `souls-bonfire` fixture (a `kill` objective on
//! `wave/guards`, behind a bonfire) with ONE field changed, so what the
//! diagnostic reacts to is unambiguous. The clean build of the untouched fixture
//! is itself a case: it must stay green, and it must warn `DW0475` because its
//! guards run on vanilla stats no vanilla data publishes.

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildFailure, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{Diagnostic, parse_campaign, validate_campaign_with};

const NS: &str = "souls-bonfire";

fn fixture_dir() -> std::path::PathBuf {
    common::compiler_fixtures_dir().join(NS)
}

/// A scratch campaign directory under the cargo target dir, removed on drop.
/// (`tempfile` is not a dependency of this crate, and one test helper is not a
/// reason to add one.)
struct TempCampaign(std::path::PathBuf);

impl TempCampaign {
    fn new() -> Self {
        let base = std::env::temp_dir().join(format!(
            "delvewright-combat-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        TempCampaign(base)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempCampaign {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Materialize `souls-bonfire` into `dst`, then hand its parsed `quests.json` /
/// `classes.json` to `mutate` so a case can change exactly one thing.
fn campaign_with(dst: &Path, mutate: impl FnOnce(&mut serde_json::Value, &mut serde_json::Value)) {
    common::materialize_from(&fixture_dir(), &serde_json::json!({}), dst);
    let quests_path = dst.join("quests.json");
    let classes_path = dst.join("classes.json");
    let mut quests: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&quests_path).unwrap()).unwrap();
    let mut classes: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&classes_path).unwrap()).unwrap();
    mutate(&mut quests, &mut classes);
    std::fs::write(&quests_path, serde_json::to_string_pretty(&quests).unwrap()).unwrap();
    std::fs::write(
        &classes_path,
        serde_json::to_string_pretty(&classes).unwrap(),
    )
    .unwrap();
}

/// Build a materialized campaign directory, returning the output plus the
/// advisory diagnostics the build raised.
fn build(dir: &Path) -> Result<(BuildOutput, Vec<Diagnostic>), BuildFailure> {
    let loaded = load_campaign_dir(dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("fixture parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let items = FullItemRegistry::v1_21_11();
    let entities = FullEntityRegistry::v1_21_11();
    let diags = validate_campaign_with(&campaign, &items, &prefabs, &entities);
    assert!(
        diags
            .iter()
            .all(|d| d.severity != delvewright_dsl::Severity::Error),
        "the fixture mutation must stay schema-valid: {diags:#?}"
    );
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let bytes = std::fs::read(common::prefabs_dir().join(&piece.structure_file)).unwrap();
            structures.insert(piece.structure_file.clone(), bytes);
        }
    }
    let mut skins: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for npc in &campaign.npcs.content.npcs {
        if let Some(skin) = &npc.skin {
            let png = std::fs::read(
                fixture_dir()
                    .join("skins")
                    .join(format!("{}.png", skin.texture_id)),
            )
            .expect("skin png present");
            skins.insert(skin.texture_id.clone(), png);
        }
    }
    emit::build_with_warnings(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        "unpinned",
        &skins,
    )
}

/// The failing build's diagnostic code, or a panic naming what happened instead.
fn failure_code(dir: &Path) -> String {
    match build(dir) {
        Err(BuildFailure::Diagnostic { code, message }) => {
            // The remediation contract (task #39): every message says what, where
            // and how — a bare code would be a regression.
            assert!(
                message.len() > 200,
                "{code} must carry its arithmetic and prescription: {message}"
            );
            code.to_string()
        }
        Err(other) => panic!("expected a diagnostic, got {other:?}"),
        Ok(_) => panic!("expected the build to fail"),
    }
}

fn has_code(diags: &[Diagnostic], code: &str) -> bool {
    diags.iter().any(|d| d.code == code)
}

#[test]
fn resistance_five_on_a_required_kill_is_dw0470() {
    let tmp = TempCampaign::new();
    campaign_with(tmp.path(), |quests, _| {
        let mobs = quests["content"]["waves"][0]["mobs"]
            .as_array_mut()
            .unwrap();
        mobs[0]["effects"] = serde_json::json!([
            {"effect": "minecraft:resistance", "amplifier": 4}
        ]);
    });
    assert_eq!(failure_code(tmp.path()), "DW0470");
}

#[test]
fn resistance_four_on_the_same_mob_still_builds() {
    // 80% reduction is an extremely tanky elite and entirely legal — the code is
    // about immunity, not about difficulty.
    let tmp = TempCampaign::new();
    campaign_with(tmp.path(), |quests, _| {
        let mobs = quests["content"]["waves"][0]["mobs"]
            .as_array_mut()
            .unwrap();
        mobs[0]["effects"] = serde_json::json!([
            {"effect": "minecraft:resistance", "amplifier": 3}
        ]);
    });
    build(tmp.path()).expect("a survivable elite builds");
}

#[test]
fn health_beyond_the_swing_budget_is_dw0472() {
    let tmp = TempCampaign::new();
    campaign_with(tmp.path(), |quests, _| {
        let mobs = quests["content"]["waves"][0]["mobs"]
            .as_array_mut()
            .unwrap();
        mobs[0]["attributes"] = serde_json::json!({"max_health": 1024.0});
        mobs[0]["count"] = serde_json::json!(4);
    });
    assert_eq!(failure_code(tmp.path()), "DW0472");
}

#[test]
fn a_tuned_but_finite_elite_builds_and_needs_no_warning() {
    let tmp = TempCampaign::new();
    campaign_with(tmp.path(), |quests, _| {
        for mob in quests["content"]["waves"][0]["mobs"]
            .as_array_mut()
            .unwrap()
        {
            mob["attributes"] = serde_json::json!({"max_health": 60.0});
        }
    });
    let (_, warnings) = build(tmp.path()).expect("a 60-HP elite builds");
    assert!(
        !has_code(&warnings, "DW0475"),
        "a fully-declared encounter needs no unproven warning: {warnings:#?}"
    );
}

#[test]
fn an_unavoidable_forty_damage_beat_is_dw0473() {
    let tmp = TempCampaign::new();
    campaign_with(tmp.path(), |quests, _| {
        let quest = &mut quests["content"]["quests"][0];
        let bundle = quest["on_objective_complete"]["obj/talk"]
            .as_array_mut()
            .unwrap();
        bundle.push(serde_json::json!({"type": "damage-players", "amount": 40}));
    });
    assert_eq!(failure_code(tmp.path()), "DW0473");
}

#[test]
fn the_same_hit_inside_a_zone_is_dodgeable_and_builds() {
    // A `within` box makes the hit positional — standing elsewhere is the
    // counterplay, which is exactly the avoidable case spec-0023 allows.
    let tmp = TempCampaign::new();
    campaign_with(tmp.path(), |quests, _| {
        let quest = &mut quests["content"]["quests"][0];
        let bundle = quest["on_objective_complete"]["obj/talk"]
            .as_array_mut()
            .unwrap();
        bundle.push(serde_json::json!({
            "type": "damage-players",
            "amount": 40,
            "in": {"anchor": "anchor/gate", "extent": [3, 2, 3]}
        }));
    });
    build(tmp.path()).expect("a dodgeable one-shot builds");
}

#[test]
fn a_hit_that_leaves_one_heart_builds() {
    let tmp = TempCampaign::new();
    campaign_with(tmp.path(), |quests, _| {
        let quest = &mut quests["content"]["quests"][0];
        let bundle = quest["on_objective_complete"]["obj/talk"]
            .as_array_mut()
            .unwrap();
        bundle.push(serde_json::json!({"type": "damage-players", "amount": 19}));
    });
    build(tmp.path()).expect("19 of 20 HP is a beating, not a scripted death");
}

#[test]
fn a_foodless_party_with_mandatory_combat_warns_dw0474() {
    let tmp = TempCampaign::new();
    campaign_with(tmp.path(), |_, classes| {
        for class in classes["content"]["classes"].as_array_mut().unwrap() {
            let kit = class["kit"].as_array_mut().unwrap();
            kit.retain(|item| {
                let id = item["item"].as_str().unwrap_or_default();
                !id.contains("bread") && !id.contains("beef") && !id.contains("stew")
            });
        }
    });
    let (_, warnings) = build(tmp.path()).expect("no sustain is a warning, not a build failure");
    assert!(has_code(&warnings, "DW0474"), "{warnings:#?}");
}

#[test]
fn vanilla_stat_mobs_warn_dw0475_and_still_build() {
    // Drop the guards' declared health: the mob now runs on vanilla stats, which
    // Mojang publishes nowhere, so no numeric bound can be computed — and the
    // compiler says so instead of inventing a health table.
    let tmp = TempCampaign::new();
    campaign_with(tmp.path(), |quests, _| {
        let mobs = quests["content"]["waves"][0]["mobs"]
            .as_array_mut()
            .unwrap();
        mobs[0].as_object_mut().unwrap().remove("attributes");
    });
    let (_, warnings) = build(tmp.path()).expect("a vanilla-stat wave builds clean");
    assert!(has_code(&warnings, "DW0475"), "{warnings:#?}");
}

#[test]
fn the_combat_plan_is_validation_only_and_names_the_tier() {
    let tmp = TempCampaign::new();
    campaign_with(tmp.path(), |quests, _| {
        quests["dsl_version"] = serde_json::json!("0.7.0");
        quests["content"]["waves"][0]["tier"] = serde_json::json!("boss");
    });
    let (out, _) = build(tmp.path()).expect("a tiered wave builds");
    let plan = out
        .get("validation/combat-plan.json")
        .expect("the combat plan is emitted for a campaign with encounters");
    let json: serde_json::Value = serde_json::from_slice(plan).unwrap();
    assert_eq!(json["encounters"][0]["tier"], "boss");
    assert_eq!(json["encounters"][0]["wave"], "wave/guards");
    assert!(
        json["encounters"][0]["checkpoint"].is_array(),
        "the bonfire governs a death at this encounter: {json}"
    );
    // Validation metadata only — nothing under `datapack/` may mention it.
    assert!(
        out.keys()
            .all(|k| !k.starts_with("datapack/") || !k.contains("combat-plan")),
        "the combat plan must never reach the shipped datapack"
    );
}

#[test]
fn a_combat_free_campaign_emits_no_combat_plan() {
    // hello-world has no `kill` step at all, so the whole pass is skipped and its
    // output is byte-identical to before spec-0023.
    let loaded = load_campaign_dir(&common::hello_world_dir()).unwrap();
    let campaign = parse_campaign(&loaded.raw).unwrap();
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).unwrap();
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let bytes = std::fs::read(common::prefabs_dir().join(&piece.structure_file)).unwrap();
            structures.insert(piece.structure_file.clone(), bytes);
        }
    }
    let out = emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
    .expect("hello-world builds");
    assert!(!out.contains_key("validation/combat-plan.json"));
}
