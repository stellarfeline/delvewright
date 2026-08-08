//! `DW0496` — a daylight-burning body may not be staged where the sun can
//! reach it (task #189).
//!
//! ## The incident this exists for (owner playtest, `hollow-vigil`, 2026-08-05)
//!
//! The walls-down round carved the gate yard's roof and two of its walls open
//! to the sky; the world is pinned `time set noon`; the first zombie wave
//! musters a short walk from that yard. Chased out of the keep, the footmen
//! burned — two of three dead to sunlight in under twenty seconds, outside the
//! carved north wall — and the encounter the party was supposed to *fight* was
//! decided by the weather. Every proof was green, because nothing at compile
//! time related "this body burns in daylight" to "this is a fight".
//!
//! The `daylight-yard` fixture is that geometry in miniature: the hello-room
//! with its ceiling carved off, a world pinned to clear noon, and an
//! unhelmeted zombie wave adjudicated by a `kill` objective. Each case changes
//! exactly ONE thing about it, so what the diagnostic reacts to is
//! unambiguous.

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildFailure};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{Diagnostic, Severity, parse_campaign, validate_campaign_with};

const NS: &str = "daylight-yard";

fn fixture_dir() -> std::path::PathBuf {
    common::compiler_fixtures_dir().join(NS)
}

/// A scratch campaign directory under the system temp dir, removed on drop.
struct TempCampaign(std::path::PathBuf);

impl TempCampaign {
    fn new(tag: &str) -> Self {
        let base = std::env::temp_dir().join(format!(
            "delvewright-daylight-{tag}-{}-{:?}",
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

/// Materialize the whole fixture (including its stage-7 edit script) into
/// `dst`, then hand the parsed `world.json` / `quests.json` to `mutate` so one
/// case can change exactly one thing. Returning `false` from `keep_edits`
/// drops the edit script — the "roof it instead" remedy.
fn campaign_with(
    dst: &Path,
    keep_edits: bool,
    mutate: impl FnOnce(&mut serde_json::Value, &mut serde_json::Value),
) {
    common::materialize_from(&fixture_dir(), &serde_json::json!({}), dst);
    if keep_edits {
        std::fs::copy(
            fixture_dir().join("world-edits.json"),
            dst.join("world-edits.json"),
        )
        .unwrap();
    }
    let world_path = dst.join("world.json");
    let quests_path = dst.join("quests.json");
    let mut world: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&world_path).unwrap()).unwrap();
    let mut quests: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&quests_path).unwrap()).unwrap();
    mutate(&mut world, &mut quests);
    std::fs::write(&world_path, serde_json::to_string_pretty(&world).unwrap()).unwrap();
    std::fs::write(&quests_path, serde_json::to_string_pretty(&quests).unwrap()).unwrap();
}

/// Build a materialized campaign directory; `Ok` carries the advisory
/// diagnostics.
fn build(dir: &Path) -> Result<Vec<Diagnostic>, BuildFailure> {
    let loaded = load_campaign_dir(dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("fixture parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let items = FullItemRegistry::v1_21_11();
    let entities = FullEntityRegistry::v1_21_11();
    let diags = validate_campaign_with(&campaign, &items, &prefabs, &entities);
    assert!(
        diags.iter().all(|d| d.severity != Severity::Error),
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
    emit::build_with_warnings(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
    .map(|(_, warnings)| warnings)
}

/// Replace the wave's single mob stack.
fn set_mobs(quests: &mut serde_json::Value, mobs: serde_json::Value) {
    quests["content"]["waves"][0]["mobs"] = mobs;
}

fn code_of(err: &BuildFailure) -> String {
    match err {
        BuildFailure::Diagnostic { code, .. } => (*code).to_string(),
        other => panic!("expected a coded build diagnostic, got {other:?}"),
    }
}

fn message_of(err: &BuildFailure) -> String {
    match err {
        BuildFailure::Diagnostic { message, .. } => message.clone(),
        other => panic!("expected a coded build diagnostic, got {other:?}"),
    }
}

// --- red -------------------------------------------------------------------

/// The incident's shape: sky over the arena, noon pinned, a bare-headed zombie
/// the party is required to kill. The build stops.
#[test]
fn unhelmeted_zombie_under_open_sky_at_noon_is_dw0496() {
    let tmp = TempCampaign::new("red");
    campaign_with(tmp.path(), true, |_, _| {});
    let err = build(tmp.path()).expect_err("a wave that burns instead of fighting must fail");
    assert_eq!(code_of(&err), "DW0496", "{}", message_of(&err));
    let message = message_of(&err);
    // The message must name the fight, the species and the remedy — those three
    // are the whole content of the bug.
    assert!(
        message.contains("wave/garrison"),
        "must name the encounter: {message}"
    );
    assert!(
        message.contains("minecraft:zombie"),
        "must name the species: {message}"
    );
    assert!(
        message.contains("equipment.head"),
        "must prescribe the sanctioned remedy: {message}"
    );
    assert!(
        message.contains("set-time"),
        "must forbid the fix the owner ruled out: {message}"
    );
}

// --- green: the two sanctioned remedies -------------------------------------

/// The owner-sanctioned fix, and the one `hollow-vigil` shipped: a helmet.
#[test]
fn a_helmet_clears_dw0496() {
    let tmp = TempCampaign::new("helm");
    campaign_with(tmp.path(), true, |_, quests| {
        set_mobs(
            quests,
            serde_json::json!([{
                "entity": "minecraft:zombie",
                "count": 2,
                "name": "Hollow Footman",
                "equipment": { "head": "minecraft:leather_helmet" }
            }]),
        );
    });
    build(tmp.path()).expect("a helmeted garrison fights in daylight");
}

/// The other remedy: put the roof back. Same wave, same bare heads, no sky.
#[test]
fn roofing_the_arena_clears_dw0496() {
    let tmp = TempCampaign::new("roof");
    campaign_with(tmp.path(), false, |_, _| {});
    build(tmp.path()).expect("a roofed arena needs no helmets");
}

// --- green: the conditions that must each be necessary ----------------------

/// Night is not a burning hour. (Not a *prescription* — the owner ruled
/// `set-time` out — but the rule must not fire on a delve authored at night.)
#[test]
fn a_night_world_is_silent() {
    let tmp = TempCampaign::new("night");
    campaign_with(tmp.path(), true, |world, _| {
        world["content"]["time"] = serde_json::json!("midnight");
    });
    build(tmp.path()).expect("nothing burns at midnight");
}

/// Rain suppresses the burn tick outright (`isInWaterOrRain`), so a rained-on
/// delve is silent.
#[test]
fn rain_is_silent() {
    let tmp = TempCampaign::new("rain");
    campaign_with(tmp.path(), true, |world, _| {
        world["content"]["weather"] = serde_json::json!("rain");
    });
    build(tmp.path()).expect("a mob in the rain does not burn");
}

/// A husk is undead and is NOT in vanilla's `#minecraft:burn_in_daylight` — the
/// desert garrison is a legitimate open-air wave.
#[test]
fn a_husk_is_silent() {
    let tmp = TempCampaign::new("husk");
    campaign_with(tmp.path(), true, |_, quests| {
        set_mobs(
            quests,
            serde_json::json!([
                { "entity": "minecraft:husk", "count": 2, "name": "Sand-Choked Footman" }
            ]),
        );
    });
    build(tmp.path()).expect("husks do not burn");
}

/// A wither skeleton IS in the tag and still never burns: it is fire-immune.
/// The tag says which types run the burn tick, not which types the fire hurts.
#[test]
fn a_wither_skeleton_is_silent() {
    let tmp = TempCampaign::new("wither");
    campaign_with(tmp.path(), true, |_, quests| {
        set_mobs(
            quests,
            serde_json::json!([
                { "entity": "minecraft:wither_skeleton", "count": 1, "name": "The First Warden" }
            ]),
        );
    });
    build(tmp.path()).expect("fire-immune bodies do not burn");
}

/// A skeleton is in the tag, is not fire-immune, and burns — the proof that the
/// rule is about the tag and not about the word "zombie".
#[test]
fn an_unhelmeted_skeleton_is_dw0496() {
    let tmp = TempCampaign::new("skeleton");
    campaign_with(tmp.path(), true, |_, quests| {
        set_mobs(
            quests,
            serde_json::json!([
                { "entity": "minecraft:skeleton", "count": 2, "name": "Hollow Archer" }
            ]),
        );
    });
    let err = build(tmp.path()).expect_err("skeletons burn too");
    assert_eq!(code_of(&err), "DW0496");
}

/// A phantom burns *through* a helmet (wiki, 1.21.11: "They burn even when
/// equipped with helmets through commands"), so the head slot is no exemption
/// and the prescription must not offer one.
#[test]
fn a_helmeted_phantom_is_still_dw0496() {
    let tmp = TempCampaign::new("phantom");
    campaign_with(tmp.path(), true, |_, quests| {
        set_mobs(
            quests,
            serde_json::json!([{
                "entity": "minecraft:phantom",
                "count": 1,
                "name": "The Long Night",
                "equipment": { "head": "minecraft:leather_helmet" }
            }]),
        );
    });
    let err = build(tmp.path()).expect_err("a helmet does not save a phantom");
    assert_eq!(code_of(&err), "DW0496");
    assert!(
        message_of(&err).contains("phantom"),
        "must name the species whose helmet does not work: {}",
        message_of(&err)
    );
}

/// The fixture's own control: an untouched clean build is silent about
/// everything except the arithmetic it cannot do (`DW0475`, vanilla stats).
#[test]
fn the_roofed_fixture_warns_only_about_vanilla_stats() {
    let tmp = TempCampaign::new("control");
    campaign_with(tmp.path(), false, |_, _| {});
    let warnings = build(tmp.path()).expect("the roofed fixture builds");
    assert!(
        !warnings.iter().any(|d| d.code == "DW0496"),
        "DW0496 is an error, never a warning: {warnings:#?}"
    );
}
