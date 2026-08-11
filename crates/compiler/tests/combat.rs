//! spec-0023 §2 — compile-time combat winnability (`DW0470`–`DW0475`), plus the
//! floor-gate coverage ledger (`DW0477`, task #113).
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
        // `wave/guards` is `souls-bonfire`'s only `respawns_on_rest` wave, and
        // souls ruling 5/7 (task #160) forbids a `tier: boss` wave from
        // re-seating on rest (`DW0499`) — a combination this test, about the
        // combat plan's tier bookkeeping, is not exercising. Clear it so the
        // mutation stays isolated to the one field under test.
        quests["content"]["waves"][0]["respawns_on_rest"] = serde_json::json!(false);
    });
    let (out, _) = build(tmp.path()).expect("a tiered wave builds");
    let plan = out
        .get("validation/combat-plan.json")
        .expect("the combat plan is emitted for a campaign with encounters");
    let json: serde_json::Value = serde_json::from_slice(plan).unwrap();
    assert_eq!(json["encounters"][0]["tier"], "boss");
    assert_eq!(json["encounters"][0]["wave"], "wave/guards");
    // No `checkpoint`: souls-bonfire's only rest point is armed by `obj/slay`'s
    // OWN completion — the very kill this encounter is — so nothing governs a
    // death during the fight. See
    // `a_checkpoint_armed_by_the_encounters_own_step_does_not_govern_it`, which
    // pins that shape deliberately; this assertion used to read the opposite and
    // was encoding the off-by-one.
    assert!(
        json["encounters"][0]["checkpoint"].is_null(),
        "nothing is armed yet at this encounter: {json}"
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

// ---------------------------------------------------------------------------
// The floor gate's coverage ledger (task #113): an elite implemented as an
// ACTOR used to be structurally invisible to the inverted floor gate, so an
// empty finding list read as a pass over a fight nobody had.
// ---------------------------------------------------------------------------

/// The tiered actor every case below shares: a wither skeleton kneeling on the
/// ambush anchor, billed `elite`, with the health that makes it one.
fn barrow_warden() -> serde_json::Value {
    serde_json::json!({
        "id": "actor/barrow-warden",
        "entity": "minecraft:wither_skeleton",
        "name": "The Barrow Warden",
        "anchor": "anchor/wave",
        "tier": "elite",
        "attributes": { "max_health": 60.0 }
    })
}

/// The beat that turns the kneeling puppet into a fight: strike the keeper's
/// body, and the thing behind you stands up.
fn unleash_trigger() -> serde_json::Value {
    serde_json::json!({
        "id": "trigger/warden-answers",
        "on": { "on": "strike-npc", "npc": "npc/keeper" },
        "once": true,
        "effects": [
            { "type": "spawn-actor", "actor": "actor/barrow-warden" },
            { "type": "unleash-actor", "actor": "actor/barrow-warden" }
        ]
    })
}

/// Materialize souls-bonfire with the tiered actor plus whatever `triggers`
/// the case wants appended, and return the parsed combat plan and diagnostics.
fn build_with_actor(
    tmp: &TempCampaign,
    actor: serde_json::Value,
    extra_triggers: Vec<serde_json::Value>,
) -> (serde_json::Value, Vec<Diagnostic>, BuildOutput) {
    campaign_with(tmp.path(), |quests, _| {
        quests["dsl_version"] = serde_json::json!("0.8.0");
        quests["content"]["actors"] = serde_json::json!([actor]);
        let triggers = quests["content"]["triggers"].as_array_mut().unwrap();
        triggers.extend(extra_triggers);
    });
    let (out, diags) = build(tmp.path()).expect("a tiered actor builds");
    let plan = out
        .get("validation/combat-plan.json")
        .expect("the combat plan is emitted");
    let json: serde_json::Value = serde_json::from_slice(plan).unwrap();
    (json, diags, out)
}

#[test]
fn an_unleashed_tiered_actor_is_a_covered_encounter() {
    let tmp = TempCampaign::new();
    let (json, diags, _) = build_with_actor(&tmp, barrow_warden(), vec![unleash_trigger()]);

    // It is in the plan at all — the gap this closes.
    let a = &json["actors"][0];
    assert_eq!(a["actor"], "actor/barrow-warden");
    assert_eq!(a["tier"], "elite");
    assert_eq!(a["entity"], "minecraft:wither_skeleton");
    assert_eq!(a["anchor"], "anchor/wave");
    assert_eq!(a["tag"], "dw_actor_barrow_warden");
    assert_eq!(a["attributes"]["max_health"], 60.0);
    assert!(
        a["pos"].is_array(),
        "the harness needs somewhere to walk: {a}"
    );

    // …with the beat that starts the fight, named well enough to fire.
    let unleash = &a["unleashed_by"][0];
    assert_eq!(unleash["site"], "trigger");
    assert_eq!(unleash["owner"], "trigger/warden-answers");
    assert_eq!(unleash["on"], "strike-npc");
    assert_eq!(unleash["npc"], "npc/keeper");
    assert_eq!(a["spawned_by"][0]["owner"], "trigger/warden-answers");

    // …and the floor gate says out loud that it covers it.
    assert_eq!(a["floor_gate"]["covered"], true);
    let covered = json["floor_gate"]["covered"].as_array().unwrap();
    assert!(
        covered
            .iter()
            .any(|e| e["kind"] == "actor" && e["id"] == "actor/barrow-warden"),
        "{json}"
    );
    assert!(
        json["floor_gate"]["not_covered"]
            .as_array()
            .unwrap()
            .is_empty(),
        "{json}"
    );
    assert!(!has_code(&diags, "DW0477"), "{diags:#?}");
}

#[test]
fn a_tiered_actor_nobody_unleashes_is_dw0477_not_silence() {
    // The defect this task kills: the actor is billed `elite`, the run report's
    // finding list comes back empty, and nothing anywhere says the fight was
    // never had.
    let tmp = TempCampaign::new();
    let (json, diags, _) = build_with_actor(&tmp, barrow_warden(), vec![]);

    assert_eq!(json["actors"][0]["floor_gate"]["covered"], false);
    let not_covered = &json["floor_gate"]["not_covered"][0];
    assert_eq!(not_covered["id"], "actor/barrow-warden");
    assert_eq!(not_covered["tier"], "elite");
    assert!(
        not_covered["reason"]
            .as_str()
            .unwrap()
            .contains("no `spawn-actor` effect"),
        "the reason must name the missing beat: {json}"
    );

    let d = diags
        .iter()
        .find(|d| d.code == "DW0477")
        .unwrap_or_else(|| panic!("expected DW0477: {diags:#?}"));
    assert_eq!(d.severity, delvewright_dsl::Severity::Warning);
    assert_eq!(d.path, "/content/actors/0/tier");
    assert!(d.message.contains("not covered"), "{}", d.message);
}

#[test]
fn a_staged_but_never_unleashed_puppet_is_scenery_not_a_fight() {
    let tmp = TempCampaign::new();
    let spawn_only = serde_json::json!({
        "id": "trigger/warden-kneels",
        "on": { "on": "strike-npc", "npc": "npc/keeper" },
        "once": true,
        "effects": [{ "type": "spawn-actor", "actor": "actor/barrow-warden" }]
    });
    let (json, diags, _) = build_with_actor(&tmp, barrow_warden(), vec![spawn_only]);
    let reason = json["floor_gate"]["not_covered"][0]["reason"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(reason.contains("Invulnerable"), "{reason}");
    assert!(has_code(&diags, "DW0477"), "{diags:#?}");

    // …and the `vulnerable` variant gets its OWN reason: a NoAI creep cannot
    // fight back, so a first-try win by the bot would be an artifact of the
    // check rather than a finding about the encounter.
    let tmp2 = TempCampaign::new();
    let mut vulnerable = barrow_warden();
    vulnerable["vulnerable"] = serde_json::json!(true);
    let spawn_only2 = serde_json::json!({
        "id": "trigger/warden-kneels",
        "on": { "on": "strike-npc", "npc": "npc/keeper" },
        "once": true,
        "effects": [{ "type": "spawn-actor", "actor": "actor/barrow-warden" }]
    });
    let (json2, _, _) = build_with_actor(&tmp2, vulnerable, vec![spawn_only2]);
    let reason2 = json2["floor_gate"]["not_covered"][0]["reason"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(reason2.contains("never attacks"), "{reason2}");
}

#[test]
fn an_untiered_hostile_actor_lands_in_not_covered() {
    // Task #121: the ledger's own blind spot. The campaign unleashes a real-AI
    // body on the party and declares nothing about it, so before this it
    // appeared on NEITHER side — and an empty ledger reads as "everything is
    // covered" when it means "nothing was even assessed".
    let tmp = TempCampaign::new();
    let mut untiered = barrow_warden();
    untiered.as_object_mut().unwrap().remove("tier");
    let (json, diags, _) = build_with_actor(&tmp, untiered, vec![unleash_trigger()]);

    // It is not a TIERED actor, so it stays out of the trial array…
    assert!(json["actors"].as_array().unwrap().is_empty(), "{json}");
    // …and it is not covered, with `tier: null` saying why in one field.
    let not_covered = &json["floor_gate"]["not_covered"][0];
    assert_eq!(not_covered["kind"], "actor");
    assert_eq!(not_covered["id"], "actor/barrow-warden");
    assert!(not_covered["tier"].is_null(), "{json}");
    assert!(
        not_covered["reason"].as_str().unwrap().contains("UNTIERED"),
        "the reason must name the omission: {json}"
    );
    assert!(
        json["floor_gate"]["covered"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["id"] != "actor/barrow-warden"),
        "{json}"
    );
    // DW0477 is about a BILLING the gate cannot hold; nothing was billed here,
    // so the ledger line is the whole record and no warning is raised.
    assert!(!has_code(&diags, "DW0477"), "{diags:#?}");
}

#[test]
fn a_staged_untiered_puppet_is_not_a_hostile() {
    // Hostility is "unleashed", the same rule the die-retry / assist machinery
    // uses: a staged puppet is `NoAI` and knockback-immune, so it never attacks
    // and there is nothing for the gate to have assessed. Scenery must not fill
    // the ledger, or the ledger stops being readable.
    let tmp = TempCampaign::new();
    let mut untiered = barrow_warden();
    untiered.as_object_mut().unwrap().remove("tier");
    untiered["vulnerable"] = serde_json::json!(true);
    let spawn_only = serde_json::json!({
        "id": "trigger/warden-kneels",
        "on": { "on": "strike-npc", "npc": "npc/keeper" },
        "once": true,
        "effects": [{ "type": "spawn-actor", "actor": "actor/barrow-warden" }]
    });
    let (json, _, _) = build_with_actor(&tmp, untiered, vec![spawn_only]);
    assert!(
        json["floor_gate"]["not_covered"]
            .as_array()
            .unwrap()
            .is_empty(),
        "{json}"
    );
}

#[test]
fn an_untiered_hostile_is_reason_enough_to_ship_a_ledger() {
    // hello-world has no `kill` step and no tiered actor, so it emitted NO
    // combat plan and the run report said `present: false` — "this build cannot
    // tell you". Unleash one unbilled body in it and that answer becomes a lie
    // by omission, so the ledger must ship. (The campaign with no fight at all
    // still emits nothing — `a_combat_free_campaign_emits_no_combat_plan` — and
    // that is what keeps `present: false` meaningful.)
    let tmp = TempCampaign::new();
    common::materialize_from(
        &common::hello_world_dir(),
        &serde_json::json!({}),
        tmp.path(),
    );
    let quests_path = tmp.path().join("quests.json");
    let mut quests: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&quests_path).unwrap()).unwrap();
    quests["dsl_version"] = serde_json::json!("0.8.0");
    quests["content"]["actors"] = serde_json::json!([{
        "id": "actor/barrow-warden",
        "entity": "minecraft:wither_skeleton",
        "anchor": "anchor/exit",
    }]);
    quests["content"]["triggers"] = serde_json::json!([{
        "id": "trigger/warden-answers",
        "on": { "on": "strike-npc", "npc": "npc/keeper" },
        "once": true,
        "effects": [
            { "type": "spawn-actor", "actor": "actor/barrow-warden" },
            { "type": "unleash-actor", "actor": "actor/barrow-warden" }
        ]
    }]);
    std::fs::write(&quests_path, serde_json::to_string_pretty(&quests).unwrap()).unwrap();

    let (out, _) = build(tmp.path()).expect("an untiered hostile builds");
    let json: serde_json::Value = serde_json::from_slice(
        out.get("validation/combat-plan.json")
            .expect("an untiered hostile is reason enough to ship the ledger"),
    )
    .unwrap();
    assert_eq!(
        json["floor_gate"]["not_covered"][0]["id"],
        "actor/barrow-warden"
    );
    assert!(
        json["floor_gate"]["not_covered"][0]["tier"].is_null(),
        "{json}"
    );
}

#[test]
fn an_optional_tiered_wave_is_uncovered_too() {
    // The same silence, on the shape that already had a `tier`: `wave/ambush`
    // has no `kill` objective, so billing it `elite` claims something no proof
    // ever measures.
    let tmp = TempCampaign::new();
    campaign_with(tmp.path(), |quests, _| {
        quests["dsl_version"] = serde_json::json!("0.7.0");
        quests["content"]["waves"][1]["tier"] = serde_json::json!("elite");
    });
    let (out, diags) = build(tmp.path()).expect("an optional tiered wave builds");
    let json: serde_json::Value =
        serde_json::from_slice(out.get("validation/combat-plan.json").unwrap()).unwrap();
    let not_covered = &json["floor_gate"]["not_covered"][0];
    assert_eq!(not_covered["kind"], "wave");
    assert_eq!(not_covered["id"], "wave/ambush");
    assert!(
        not_covered["reason"]
            .as_str()
            .unwrap()
            .contains("no `kill` objective"),
        "{json}"
    );
    let d = diags.iter().find(|d| d.code == "DW0477").unwrap();
    assert_eq!(d.path, "/content/waves/1/tier");
}

#[test]
fn declaring_an_actor_tier_moves_no_shipped_byte() {
    // The version fence exists so a tier is pure validation metadata. Compile
    // the same campaign with and without the field and compare EVERY output
    // outside `validation/` byte for byte (`manifest.json` indexes the whole
    // tree, `validation/` included, so it is the one documented exception —
    // exactly as `critical-path-waypoints.json` and the combat plan itself
    // already are).
    let untiered = TempCampaign::new();
    let mut plain = barrow_warden();
    plain.as_object_mut().unwrap().remove("tier");
    let (_, _, out_plain) = build_with_actor(&untiered, plain, vec![unleash_trigger()]);

    let tiered = TempCampaign::new();
    let (_, _, out_tiered) = build_with_actor(&tiered, barrow_warden(), vec![unleash_trigger()]);

    let shipped = |o: &BuildOutput| -> Vec<(String, Vec<u8>)> {
        o.iter()
            .filter(|(k, _)| !k.starts_with("validation/") && k.as_str() != "manifest.json")
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    };
    assert_eq!(
        shipped(&out_plain),
        shipped(&out_tiered),
        "an actor `tier` must not reach a shipped byte"
    );
    // The untiered actor is also absent from the plan entirely — an untiered
    // actor carries no floor expectation, exactly like an untiered wave.
    let plan_plain: serde_json::Value =
        serde_json::from_slice(out_plain.get("validation/combat-plan.json").unwrap()).unwrap();
    assert!(
        plan_plain["actors"].as_array().unwrap().is_empty(),
        "{plan_plain}"
    );
}

// ---------------------------------------------------------------------------
// Binding counts (playtest-methodology.md rule 1): a ledger that examined zero
// objects must say so, additively, never by leaving `covered`/`not_covered`
// (or `actors[]`) merely empty.
// ---------------------------------------------------------------------------

#[test]
fn an_empty_floor_gate_and_actor_gate_state_their_own_zero() {
    // The exact defect class rule 1 names: `souls-bonfire`, UNMODIFIED, has a
    // real mandatory encounter (`wave/guards`, a `kill` step on the critical
    // path) that nothing bills `elite`/`boss`, and no actor at all — the same
    // shape `nobodys-cave-island` shipped green for nineteen rounds. Before
    // this task, `floor_gate` was `{covered: [], not_covered: []}` with no way
    // to tell "examined and found nothing wrong" from "examined nothing".
    let tmp = TempCampaign::new();
    campaign_with(tmp.path(), |_, _| {});
    let (out, _) = build(tmp.path()).expect("the untouched fixture builds");
    let json: serde_json::Value =
        serde_json::from_slice(out.get("validation/combat-plan.json").unwrap()).unwrap();

    assert_eq!(json["floor_gate"]["examined"], 0, "{json}");
    assert_eq!(json["floor_gate"]["unbound"], true, "{json}");
    assert!(
        json["floor_gate"]["covered"].as_array().unwrap().is_empty(),
        "{json}"
    );
    assert!(
        json["floor_gate"]["not_covered"]
            .as_array()
            .unwrap()
            .is_empty(),
        "{json}"
    );
    let floor_reason = json["floor_gate"]["reason"].as_str().expect("{json}");
    assert!(
        floor_reason.contains("nothing to hold"),
        "the reason must say what zero means: {floor_reason}"
    );

    assert_eq!(json["actors_gate"]["examined"], 0, "{json}");
    assert_eq!(json["actors_gate"]["unbound"], true, "{json}");
    let actors_reason = json["actors_gate"]["reason"].as_str().expect("{json}");
    assert!(
        actors_reason.contains("floor_gate.not_covered"),
        "the reason must point a reader at the OTHER ledger untiered hostiles \
         actually land in: {actors_reason}"
    );
}

#[test]
fn a_covered_floor_gate_states_its_nonzero_binding_and_carries_no_reason() {
    // The green case, for contrast: an actual `elite` fight the gate covers
    // reports `examined: 1`, `unbound: false`, and NO `reason` key at all — the
    // key exists exactly to explain a zero, and its presence on a bound gate
    // would be the same noise the ledger exists to avoid.
    let tmp = TempCampaign::new();
    let (json, _, _) = build_with_actor(&tmp, barrow_warden(), vec![unleash_trigger()]);

    assert_eq!(json["floor_gate"]["examined"], 1, "{json}");
    assert_eq!(json["floor_gate"]["unbound"], false, "{json}");
    assert!(json["floor_gate"].get("reason").is_none(), "{json}");

    assert_eq!(json["actors_gate"]["examined"], 1, "{json}");
    assert_eq!(json["actors_gate"]["unbound"], false, "{json}");
    assert!(json["actors_gate"].get("reason").is_none(), "{json}");
}

#[test]
fn an_all_ordinary_actor_binds_the_actor_gate_but_not_the_floor_gate() {
    // The two counts are DIFFERENT QUESTIONS, not two spellings of one fact.
    // `actors[]` holds every actor that declares ANY tier, `ordinary` included;
    // the floor gate only ever holds `elite`/`boss`. A tier declared
    // `ordinary` is a statement (spec-0023) — it binds the actor ledger while
    // leaving the floor gate with nothing to hold.
    let tmp = TempCampaign::new();
    let mut ordinary = barrow_warden();
    ordinary["tier"] = serde_json::json!("ordinary");
    let (json, diags, _) = build_with_actor(&tmp, ordinary, vec![unleash_trigger()]);

    assert_eq!(json["actors_gate"]["examined"], 1, "{json}");
    assert_eq!(json["actors_gate"]["unbound"], false, "{json}");

    assert_eq!(json["floor_gate"]["examined"], 0, "{json}");
    assert_eq!(json["floor_gate"]["unbound"], true, "{json}");
    assert!(
        json["floor_gate"]["covered"].as_array().unwrap().is_empty(),
        "{json}"
    );
    assert!(
        json["floor_gate"]["not_covered"]
            .as_array()
            .unwrap()
            .is_empty(),
        "{json}"
    );
    // An `ordinary`-billed actor is not a `DW0477` finding either — nothing was
    // billed hard, so there is nothing the floor gate failed to hold.
    assert!(!has_code(&diags, "DW0477"), "{diags:#?}");
}

// ---------------------------------------------------------------------------
// The governing checkpoint, and the one coordinate system (#221 follow-up).
// ---------------------------------------------------------------------------

/// Parse both harness documents out of one build.
fn path_and_plan(out: &BuildOutput) -> (serde_json::Value, serde_json::Value) {
    let path: serde_json::Value =
        serde_json::from_slice(out.get("critical-path.json").expect("path exported")).unwrap();
    let plan: serde_json::Value = serde_json::from_slice(
        out.get("validation/combat-plan.json")
            .expect("plan emitted"),
    )
    .unwrap();
    (path, plan)
}

#[test]
fn a_checkpoint_armed_by_the_encounters_own_step_does_not_govern_it() {
    // souls-bonfire's exact shape, and the reason this is a defect rather than a
    // taste: the bonfire is fired by `obj/slay`'s completion — the completion of
    // the very kill the encounter IS — so at any death DURING that fight the
    // fire has not been armed, let alone rested at, and the party returns to
    // world spawn. `fire_step <= i` handed the encounter a respawn point one
    // beat in its own future.
    let tmp = TempCampaign::new();
    campaign_with(tmp.path(), |_, _| {});
    let (out, _) = build(tmp.path()).expect("the untouched fixture builds");
    let (path, plan) = path_and_plan(&out);

    let enc = &plan["encounters"][0];
    assert_eq!(enc["wave"], "wave/guards");
    assert!(
        enc["checkpoint"].is_null(),
        "a checkpoint armed by this encounter's own step must not govern it: {plan}"
    );

    // …and this really is the same-step case, not merely a campaign with no
    // checkpoints: the exported path rests at the bonfire on the step directly
    // AFTER the kill, which is what "armed by the kill's own completion" looks
    // like from the outside.
    let steps = path["steps"].as_array().unwrap();
    let kill = enc["step"].as_u64().unwrap() as usize;
    assert_eq!(steps[kill]["action"], "kill");
    assert_eq!(steps[kill + 1]["action"], "rest", "{path}");
}

#[test]
fn a_checkpoint_armed_earlier_governs_the_encounter() {
    // The same fixture with the bonfire moved one beat earlier — armed by
    // `obj/talk` instead of `obj/slay`. Now it IS armed before the fight, so it
    // governs, which is what proves the rule is `< i` and not "never".
    let tmp = TempCampaign::new();
    campaign_with(tmp.path(), |quests, _| {
        let slay = quests["content"]["quests"][1]["on_objective_complete"]["obj/slay"]
            .as_array_mut()
            .unwrap();
        let at = slay
            .iter()
            .position(|e| e["type"] == "bonfire")
            .expect("the fixture's bonfire");
        let bonfire = slay.remove(at);
        quests["content"]["quests"][0]["on_objective_complete"]["obj/talk"]
            .as_array_mut()
            .unwrap()
            .push(bonfire);
    });
    let (out, _) = build(tmp.path()).expect("the moved bonfire builds");
    let (_, plan) = path_and_plan(&out);
    assert!(
        plan["encounters"][0]["checkpoint"].is_array(),
        "a checkpoint armed strictly before the encounter governs it: {plan}"
    );
}

#[test]
fn the_combat_plan_step_indexes_the_exported_path() {
    // One coordinate system for every harness document. `plan.critical_path` and
    // the exported `critical-path.json` drift by one per bonfire armed earlier
    // (spec-0016 §1 splices a `rest` step after each arming beat), and the combat
    // plan's `step` claimed to be an exported index while being an internal one.
    // Proven against the real emitted documents, not against the arithmetic:
    // whatever the splice does, the step the plan points at must BE the
    // encounter's kill.
    //
    // The bonfire is moved to `obj/talk` so the two coordinates genuinely differ
    // — with the fixture's own placement they coincide, and a test that cannot
    // fail proves nothing.
    let tmp = TempCampaign::new();
    campaign_with(tmp.path(), |quests, _| {
        let slay = quests["content"]["quests"][1]["on_objective_complete"]["obj/slay"]
            .as_array_mut()
            .unwrap();
        let at = slay.iter().position(|e| e["type"] == "bonfire").unwrap();
        let bonfire = slay.remove(at);
        quests["content"]["quests"][0]["on_objective_complete"]["obj/talk"]
            .as_array_mut()
            .unwrap()
            .push(bonfire);
    });
    let (out, _) = build(tmp.path()).expect("builds");
    let (path, plan) = path_and_plan(&out);
    let steps = path["steps"].as_array().unwrap();

    // The splice really did move things: a `rest` step sits before the kill.
    let rest_at = steps
        .iter()
        .position(|s| s["action"] == "rest")
        .expect("a rest step");
    let kill_at = steps
        .iter()
        .position(|s| s["action"] == "kill")
        .expect("a kill step");
    assert!(rest_at < kill_at, "{path}");

    for enc in plan["encounters"].as_array().unwrap() {
        let i = enc["step"].as_u64().unwrap() as usize;
        assert_eq!(
            steps[i]["action"], "kill",
            "step {i} is not the kill: {path}"
        );
        assert_eq!(steps[i]["wave"], enc["wave"], "{path}");
        assert_eq!(steps[i]["objective"], enc["objective"], "{path}");
    }
    // …and the internal index it came from is genuinely a different number, so
    // this test would have failed before the reconciliation.
    assert_eq!(kill_at, 3, "{path}");
}

// --- the wave census probe (task #123, #230) --------------------------------

/// The ladder used to answer "what is standing at this encounter?" by silhouette
/// — every entity the client tracked, no distance filter, anything taller than
/// half a block. That set is not the wave: on the drowned bell it swept in two
/// ambush husks 57 blocks away and a neighbouring wave, so a 2-mob wave read as 4
/// standing, and those bystanders — alive on both sides of a scripted death —
/// were reported as survivors the re-seat had failed to remove. The re-seat was
/// innocent.
///
/// Only the server can see the wave tag, so the compiler owns the census. These
/// three functions are the whole probe surface, and the plan NAMES them so the
/// harness never re-derives `safe_local`.
#[test]
fn every_wave_carries_a_tag_census_probe() {
    let tmp = TempCampaign::new();
    campaign_with(tmp.path(), |_, _| {});
    let (out, _) = build(tmp.path()).expect("the reference campaign builds");

    let body = |name: &str| -> String {
        let path = format!("datapack/data/{NS}/function/{name}.mcfunction");
        String::from_utf8(
            out.get(&path)
                .unwrap_or_else(|| panic!("missing {name}"))
                .clone(),
        )
        .unwrap()
    };

    // Brand / unbrand ride the wave's own tag, so a stamp can only ever land on
    // this wave. The unbrand selects the BRAND, so a mob that somehow outlived
    // its wave tag is still cleaned up.
    assert_eq!(
        body("wave_brand_guards").trim(),
        "tag @e[tag=dw_wave_guards] add dw_brand_guards"
    );
    assert_eq!(
        body("wave_unbrand_guards").trim(),
        "tag @e[tag=dw_brand_guards] remove dw_brand_guards"
    );

    // The census walks the TAG — never a type, a radius or a silhouette.
    let census = body("wave_census_guards");
    assert!(
        census.contains("execute as @e[tag=dw_wave_guards] run function"),
        "the census iterates the wave tag: {census}"
    );
    assert!(
        census.contains("scoreboard players add #wcen_seq dw.sys 1"),
        "each census takes a sequence number, so a stale answer is tellable: {census}"
    );
    for zeroed in ["#wcen_n", "#wcen_b", "#wcen_d"] {
        assert!(
            census.contains(&format!("scoreboard players set {zeroed} dw.sys 0")),
            "every accumulator is zeroed before the walk: {census}"
        );
    }
    assert!(
        census.contains("[dw:census ") && census.contains("wave/guards"),
        "the totals are stated on the anchored marker channel: {census}"
    );

    // Health comes from vanilla's own commands — never a table the compiler
    // refuses to invent (DW0475), and never a value the client happened to be
    // sent (an unmodified max health is not on the wire at all).
    let one = body("wave_census_one_guards");
    assert!(
        one.contains("run data get entity @s Health 100")
            && one.contains("run attribute @s minecraft:max_health get 100")
            && one.contains("execute if score #wcen_h dw.sys < #wcen_m dw.sys"),
        "damaged is decided from the server's own health and maximum: {one}"
    );
    assert!(
        one.contains("execute if entity @s[tag=dw_brand_guards]"),
        "carried-over is decided by the brand, by identity: {one}"
    );
    assert!(
        one.contains("[dw:censusmob "),
        "each mob states its own position and health: {one}"
    );
}

/// The harness calls what the plan names. `safe_local` is a compiler naming rule,
/// and a harness that re-derived it would be exactly the downstream folklore
/// CLAUDE.md forbids — so the probe's three function ids travel in the plan.
#[test]
fn the_combat_plan_names_the_census_probe() {
    let tmp = TempCampaign::new();
    campaign_with(tmp.path(), |_, _| {});
    let (out, _) = build(tmp.path()).expect("the reference campaign builds");
    let json: serde_json::Value =
        serde_json::from_slice(out.get("validation/combat-plan.json").unwrap()).unwrap();
    let c = &json["encounters"][0]["census"];
    assert_eq!(c["census"], format!("{NS}:wave_census_guards"));
    assert_eq!(c["brand"], format!("{NS}:wave_brand_guards"));
    assert_eq!(c["unbrand"], format!("{NS}:wave_unbrand_guards"));
}

// ---------------------------------------------------------------------------
// `fights` — the binding count for the whole spec-0023 pass (staging-gate row
// `bell-05`). The pass used to be gated on `kill`-a-wave, the VERB, so a delve
// whose combat is entirely actors ran none of DW0470–DW0475 and reported
// `encounters: 0` with nothing saying that was a coverage fact.
// ---------------------------------------------------------------------------

/// Turn souls-bonfire's `kill` objective into a walk, so the campaign has no
/// mandatory WAVE fight left — the island's shape, where every hostile is an
/// actor. The flag chain is untouched: the objective still completes and still
/// fires its bundle.
fn no_mandatory_wave(quests: &mut serde_json::Value) {
    for q in quests["content"]["quests"].as_array_mut().unwrap() {
        for o in q["objectives"].as_array_mut().unwrap() {
            if o["id"] == "obj/slay" {
                o["type"] = serde_json::json!("reach-anchor");
                o.as_object_mut().unwrap().remove("wave");
                o["anchor"] = serde_json::json!("anchor/wave");
                o["radius"] = serde_json::json!(3);
            }
        }
    }
}

fn strip_food(classes: &mut serde_json::Value) {
    for class in classes["content"]["classes"].as_array_mut().unwrap() {
        class["kit"].as_array_mut().unwrap().retain(|item| {
            let id = item["item"].as_str().unwrap_or_default();
            !id.contains("bread") && !id.contains("beef") && !id.contains("stew")
        });
    }
}

/// **The red this round was built to produce.** A campaign whose only combat is
/// an actor it turns loose, with no food anywhere, raised NOTHING before: the
/// whole winnability pass was gated on `has_encounters`, which is zero here.
#[test]
fn a_foodless_party_fighting_only_actors_warns_dw0474() {
    let tmp = TempCampaign::new();
    campaign_with(tmp.path(), |quests, classes| {
        quests["dsl_version"] = serde_json::json!("0.8.0");
        no_mandatory_wave(quests);
        quests["content"]["actors"] = serde_json::json!([barrow_warden()]);
        quests["content"]["triggers"]
            .as_array_mut()
            .unwrap()
            .push(unleash_trigger());
        strip_food(classes);
    });
    let (out, warnings) = build(tmp.path()).expect("no sustain is a warning, not a failure");

    let plan: serde_json::Value =
        serde_json::from_slice(out.get("validation/combat-plan.json").unwrap()).unwrap();
    assert_eq!(
        plan["encounters"].as_array().unwrap().len(),
        0,
        "the wave half is genuinely empty — this is the vacuity, not a rigged case: {plan}"
    );
    assert_eq!(plan["fights"]["waves"].as_array().unwrap().len(), 0);
    assert_eq!(plan["fights"]["actors"][0], "actor/barrow-warden");
    assert_eq!(plan["fights"]["total"], 1);
    assert_eq!(plan["fights"]["unbound"], false);
    assert!(plan["fights"]["reason"].is_null());

    assert!(
        has_code(&warnings, "DW0474"),
        "a delve with a fight and no food is DW0474 whichever shape the fight takes: {warnings:#?}"
    );
}

/// The same campaign WITH food is green — so the warning above is about the
/// sustain, not about the widening.
#[test]
fn the_same_actor_only_campaign_with_food_is_clean() {
    let tmp = TempCampaign::new();
    campaign_with(tmp.path(), |quests, _| {
        quests["dsl_version"] = serde_json::json!("0.8.0");
        no_mandatory_wave(quests);
        quests["content"]["actors"] = serde_json::json!([barrow_warden()]);
        quests["content"]["triggers"]
            .as_array_mut()
            .unwrap()
            .push(unleash_trigger());
    });
    let (_, warnings) = build(tmp.path()).expect("builds");
    assert!(!has_code(&warnings, "DW0474"), "{warnings:#?}");
}

/// A combat-free campaign states its own zero rather than being silent about it.
#[test]
fn a_campaign_with_no_fight_of_either_shape_says_so() {
    let tmp = TempCampaign::new();
    campaign_with(tmp.path(), |quests, _| no_mandatory_wave(quests));
    let (out, _) = build(tmp.path()).expect("builds");
    // No `kill` step and no unleashed actor: `combat-plan.json` is not emitted at
    // all, which is the pre-existing contract. The point of the case is that
    // `fights` never reports a zero as though it had been measured.
    assert!(
        !out.contains_key("validation/combat-plan.json"),
        "a campaign with no fight of either shape emits no plan"
    );
}

#[test]
fn the_fights_block_counts_a_wave_fight_too() {
    let tmp = TempCampaign::new();
    campaign_with(tmp.path(), |_, _| {});
    let (out, _) = build(tmp.path()).expect("the untouched fixture builds");
    let plan: serde_json::Value =
        serde_json::from_slice(out.get("validation/combat-plan.json").unwrap()).unwrap();
    assert_eq!(plan["fights"]["waves"][0], "wave/guards");
    assert_eq!(plan["fights"]["total"], 1);
}
