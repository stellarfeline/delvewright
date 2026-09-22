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

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildFailure, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{DSL_VERSION, Diagnostic, parse_campaign, validate_campaign_with};

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
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
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
        &skins,
    )
}

/// The failing build's diagnostic code, or a panic naming what happened instead.
fn failure_code(dir: &Path) -> String {
    match build(dir) {
        Err(BuildFailure::Diagnostic { code, message }) => {
            // The remediation contract: every message says what, where
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
        quests["dsl_version"] = serde_json::json!(DSL_VERSION);
        quests["content"]["waves"][0]["tier"] = serde_json::json!("boss");
        // `wave/guards` is `souls-bonfire`'s only `respawns_on_rest` wave, and
        // souls ruling 5/7 forbids a `tier: boss` wave from
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
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    let out = emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
    .expect("hello-world builds");
    assert!(!out.contains_key("validation/combat-plan.json"));
}

// ---------------------------------------------------------------------------
// The floor gate's coverage ledger: an elite implemented as an
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
        quests["dsl_version"] = serde_json::json!(DSL_VERSION);
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
fn declaring_an_actor_tier_moves_no_shipped_byte() {
    // A tier is pure validation metadata. Compile
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

// ---------------------------------------------------------------------------
// The governing checkpoint, and the one coordinate system.
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

// --- the wave census probe --------------------------------

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
        quests["dsl_version"] = serde_json::json!(DSL_VERSION);
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
        quests["dsl_version"] = serde_json::json!(DSL_VERSION);
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

// ---------------------------------------------------------------------------
// The targeting policy the harness used to carry (the census round). Two facts
// the bot cannot derive and used to hardcode: which bodies are never a fight,
// and when to stop swinging at one.
// ---------------------------------------------------------------------------

/// Rewrite the fixture's `npcs.json` after materialization, so a case can move an
/// NPC onto a different body without touching quests or classes.
fn with_npcs(dst: &Path, mutate: impl FnOnce(&mut serde_json::Value)) {
    let p = dst.join("npcs.json");
    let mut npcs: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    mutate(&mut npcs);
    std::fs::write(&p, serde_json::to_string_pretty(&npcs).unwrap()).unwrap();
}

#[test]
fn the_path_states_which_kinds_are_never_a_fight() {
    // souls-bonfire stages one skinned NPC, which the emitter embodies as a
    // `minecraft:mannequin`. That the harness must not swing at a mannequin is
    // not a fact about Minecraft — it is a fact about what THIS compiler summons
    // an NPC as, and this is the field that says it.
    let tmp = TempCampaign::new();
    campaign_with(tmp.path(), |_, _| {});
    let (out, _) = build(tmp.path()).expect("the reference campaign builds");
    let (path, _) = path_and_plan(&out);
    assert_eq!(path["format_version"], 4);
    let nc = &path["non_combatants"];
    assert_eq!(
        nc["kinds"],
        serde_json::json!(["mannequin"]),
        "the skinned keeper's body is the one kind nothing may attack: {nc}"
    );
    assert_eq!(nc["examined"], 1, "one NPC examined: {nc}");
    assert_eq!(nc["unbound"], false);
    assert!(
        nc.get("reason").is_none(),
        "a bound census carries no reason to explain: {nc}"
    );
    assert_eq!(nc["ambiguous"], serde_json::json!([]));
}

#[test]
fn an_npc_bodied_as_a_wave_mob_is_named_ambiguous_not_excluded() {
    // The collision the harness's literal set could never have seen: an NPC on a
    // `base_entity` the party also has to kill. Excluding the kind would make
    // `wave/guards` unwinnable, so the fightable kind wins — and the plan SAYS so
    // rather than leaving the bot to swing at a quest-giver in silence.
    let tmp = TempCampaign::new();
    campaign_with(tmp.path(), |_, _| {});
    with_npcs(tmp.path(), |npcs| {
        let npc = &mut npcs["content"]["npcs"][0];
        npc.as_object_mut().unwrap().remove("skin");
        npc["base_entity"] = serde_json::json!("minecraft:zombie");
    });
    let (out, _) = build(tmp.path()).expect("the mutated campaign builds");
    let (path, _) = path_and_plan(&out);
    let nc = &path["non_combatants"];
    assert_eq!(
        nc["kinds"],
        serde_json::json!([]),
        "zombie is `wave/guards`, so it may not be excluded: {nc}"
    );
    assert_eq!(nc["examined"], 1);
    let amb = nc["ambiguous"].as_array().expect("an array");
    assert_eq!(amb.len(), 1, "{nc}");
    assert_eq!(amb[0]["kind"], "zombie");
    let why = amb[0]["why"].as_str().unwrap();
    assert!(why.contains("npc/keeper"), "{why}");
    assert!(why.contains("unwinnable"), "{why}");
}

/// **The remedy an ambiguity names has to be one the object can still take.**
/// souls-bonfire's keeper already declares a `skin`, so its body is already a
/// `minecraft:mannequin`; put a mannequin on the fightable side and the census
/// used to answer with "give it a `skin`" — advice that NPC had taken before the
/// collision existed, and the only NPC-side advice it printed. Neither NPC-side
/// move is open here (its `base_entity` is not the body it wears either), so the
/// remedy has to point at the other side of the collision.
///
/// This is the shape a delve walks into the moment every character is bodied as a
/// mannequin.
#[test]
fn an_ambiguity_on_the_mannequin_body_names_a_remedy_the_npc_can_take() {
    let tmp = TempCampaign::new();
    campaign_with(tmp.path(), |quests, _| {
        // The fightable side becomes a mannequin, so every skinned NPC in the
        // campaign now collides with it.
        quests["content"]["waves"][0]["mobs"][0]["entity"] =
            serde_json::json!("minecraft:mannequin");
    });
    let (out, _) = build(tmp.path()).expect("the mutated campaign builds");
    let (path, _) = path_and_plan(&out);
    let nc = &path["non_combatants"];
    let amb = nc["ambiguous"].as_array().expect("an array");
    assert_eq!(amb.len(), 1, "the keeper's mannequin body collides: {nc}");
    assert_eq!(amb[0]["kind"], "mannequin");
    let why = amb[0]["why"].as_str().unwrap();
    assert!(why.contains("npc/keeper"), "{why}");
    assert!(
        !why.contains("give it a `skin`"),
        "the remedy tells an NPC that already wears a `skin` to put one on — a move it \
         cannot take, and the only NPC-side one offered: {why}"
    );
    assert!(
        why.contains("the move is on the other side of the collision"),
        "an unreachable remedy must be replaced by the reachable one, not merely \
         dropped: {why}"
    );
}

/// The sibling case, so the repair above is a narrowing and not a blanket
/// deletion: on a collision the NPC *can* move off, both NPC-side moves are still
/// offered — and the `skin` one is offered because nothing here fights a mannequin.
#[test]
fn an_ambiguity_the_npc_can_move_off_still_offers_both_moves() {
    let tmp = TempCampaign::new();
    campaign_with(tmp.path(), |_, _| {});
    with_npcs(tmp.path(), |npcs| {
        let npc = &mut npcs["content"]["npcs"][0];
        npc.as_object_mut().unwrap().remove("skin");
        npc["base_entity"] = serde_json::json!("minecraft:zombie");
    });
    let (out, _) = build(tmp.path()).expect("the mutated campaign builds");
    let (path, _) = path_and_plan(&out);
    let why = path["non_combatants"]["ambiguous"][0]["why"]
        .as_str()
        .unwrap();
    assert!(why.contains("give it a `skin`"), "{why}");
    assert!(why.contains("`base_entity`"), "{why}");
}

#[test]
fn a_campaign_with_no_npcs_states_its_own_zero() {
    // playtest-methodology rule 1: an empty `kinds` list has two readings — "no
    // NPC exists" and "the census never ran" — and only one of them is fine.
    let tmp = TempCampaign::new();
    common::materialize_from(
        &common::compiler_fixtures_dir().join("souls-td-lanes"),
        &serde_json::json!({}),
        tmp.path(),
    );
    let (out, _) = build(tmp.path()).expect("souls-td-lanes builds");
    let (path, _) = path_and_plan(&out);
    let nc = &path["non_combatants"];
    assert_eq!(nc["examined"], 0, "{nc}");
    assert_eq!(nc["unbound"], true, "{nc}");
    assert!(
        nc["reason"]
            .as_str()
            .is_some_and(|r| r.contains("stages no NPC")),
        "an unbound census states why: {nc}"
    );
}

// ---------------------------------------------------------------------------
// Run-backs (spec-0016 §1, spec-0023 §3): a cleared `respawns_on_rest` wave
// that a rest the path performs puts back beside a leg the path walks
// afterwards is an encounter, and the combat plan says so.
// ---------------------------------------------------------------------------

fn run_backs_of(out: &BuildOutput) -> Vec<serde_json::Value> {
    let plan: serde_json::Value =
        serde_json::from_slice(out.get("validation/combat-plan.json").unwrap()).unwrap();
    plan["run_backs"]
        .as_array()
        .unwrap_or_else(|| panic!("the combat plan states its run-backs: {plan}"))
        .clone()
}

/// `souls-bonfire`: `wave/guards` stands at the keeper's stand, is cleared by
/// `obj/slay`, and `obj/slay` itself arms the bonfire the path then rests at —
/// which re-seats the guards beside the walk on to the chest.
#[test]
fn a_rest_that_re_seats_a_cleared_wave_beside_the_next_leg_is_a_run_back() {
    let tmp = TempCampaign::new();
    campaign_with(tmp.path(), |quests, _| {
        quests["dsl_version"] = serde_json::json!(DSL_VERSION);
    });
    let (out, _) = build(tmp.path()).expect("souls-bonfire builds");
    let rbs = run_backs_of(&out);
    let guards: Vec<&serde_json::Value> =
        rbs.iter().filter(|r| r["wave"] == "wave/guards").collect();
    assert_eq!(guards.len(), 1, "one run-back for the guards: {rbs:#?}");
    let rb = guards[0];
    assert_eq!(rb["objective"], "obj/slay");
    assert_eq!(rb["bonfire"], 0);
    assert!(
        rb["before"].as_str().is_some_and(|b| b.starts_with("obj/")),
        "keyed by the beat whose leg re-crosses it: {rb}"
    );
    assert!(
        rb["distance"].as_f64().unwrap() <= rb["radius"].as_f64().unwrap(),
        "the crossing is inside the wave's own aggro radius: {rb}"
    );
    assert_eq!(rb["paths"], serde_json::json!(["critical-path"]));
}

/// A wave that does not come back after a rest is not met again.
#[test]
fn a_wave_that_stays_down_is_no_run_back() {
    let tmp = TempCampaign::new();
    campaign_with(tmp.path(), |quests, _| {
        quests["dsl_version"] = serde_json::json!(DSL_VERSION);
        quests["content"]["waves"][0]["respawns_on_rest"] = serde_json::json!(false);
    });
    let (out, _) = build(tmp.path()).expect("souls-bonfire builds");
    assert!(
        run_backs_of(&out)
            .iter()
            .all(|r| r["wave"] != "wave/guards"),
        "the guards stay down"
    );
}
