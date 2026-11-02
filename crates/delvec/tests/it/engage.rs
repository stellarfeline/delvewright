//! `DW0920` — a body staged as a fight the party must win is one its own
//! vanilla AI will engage under the delve's hour, weather and footing.
//!
//! ## The incident this exists for (`vesperhold`)
//!
//! The Drowned Choir, a `kill`-adjudicated wave of drowned in an underground
//! pool, under a delve pinned to dusk in rain: the choir walked toward its
//! walled-off well and never struck the party, because a drowned takes no land
//! target while the level is bright. Every proof was green.
//!
//! The `daylight-yard` fixture (a room, one `kill` wave, clear noon) carries it in
//! miniature once its garrison is a helmeted drowned — the helmet keeps `DW0496`,
//! which runs first, out of the way. Each case changes exactly one thing.

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildFailure};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{Severity, parse_campaign, validate_campaign_with};

fn fixture_dir() -> std::path::PathBuf {
    common::compiler_fixtures_dir().join("daylight-yard")
}

struct TempCampaign(std::path::PathBuf);

impl TempCampaign {
    fn new(tag: &str) -> Self {
        let base = std::env::temp_dir().join(format!(
            "delvewright-engage-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        TempCampaign(base)
    }
}

impl Drop for TempCampaign {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The fixture with its garrison replaced by helmeted drowned, then `mutate`.
fn choir(dst: &Path, mutate: impl FnOnce(&mut serde_json::Value, &mut serde_json::Value)) {
    common::materialize_from(&fixture_dir(), &serde_json::json!({}), dst);
    std::fs::copy(
        fixture_dir().join("world-edits.json"),
        dst.join("world-edits.json"),
    )
    .unwrap();
    let read = |f: &str| -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(dst.join(f)).unwrap()).unwrap()
    };
    let mut world = read("world.json");
    let mut quests = read("quests.json");
    quests["content"]["waves"][0]["mobs"] = serde_json::json!([{
        "entity": "minecraft:drowned",
        "count": 2,
        "name": "Drowned Chorister",
        "equipment": { "head": "minecraft:leather_helmet" }
    }]);
    mutate(&mut world, &mut quests);
    for (f, v) in [("world.json", &world), ("quests.json", &quests)] {
        std::fs::write(dst.join(f), serde_json::to_string_pretty(v).unwrap()).unwrap();
    }
}

fn build(dir: &Path) -> Result<(), BuildFailure> {
    let loaded = load_campaign_dir(dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("fixture parses");
    let prefabs = PrefabRegistry::load_dir(&common::shown_prefabs_dir("engage")).unwrap();
    let diags = validate_campaign_with(
        &campaign,
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    );
    assert!(
        diags.iter().all(|d| d.severity != Severity::Error),
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
    emit::build_with_warnings(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
    .map(|_| ())
}

fn refusal(err: BuildFailure) -> (String, String) {
    match err {
        BuildFailure::Diagnostic { code, message } => (code.to_string(), message),
        other => panic!("expected a coded build diagnostic, got {other:?}"),
    }
}

/// A set-time / set-weather pair to `midnight` in `clear`.
fn to_midnight() -> serde_json::Value {
    serde_json::json!([
        { "type": "set-time", "time": "midnight" },
        { "type": "set-weather", "weather": "clear" }
    ])
}

// --- red -------------------------------------------------------------------

/// The incident's shape: a drowned wave the party must kill, on dry ground,
/// under a bright hour.
#[test]
fn a_drowned_wave_on_dry_ground_at_noon_is_dw0920() {
    let tmp = TempCampaign::new("red");
    choir(&tmp.0, |_, _| {});
    let (code, message) = refusal(build(&tmp.0).expect_err("a choir that will not fight"));
    assert_eq!(code, "DW0920", "{message}");
    assert!(
        message.contains("wave/garrison"),
        "names the fight: {message}"
    );
    assert!(
        message.contains("minecraft:drowned"),
        "names the body: {message}"
    );
    assert!(message.contains("`noon`"), "names the hour: {message}");
}

/// Rain does not darken noon enough: still bright, still refused — the state
/// vesperhold ships (dusk in rain) is the same class.
#[test]
fn rain_is_still_bright() {
    let tmp = TempCampaign::new("rain");
    choir(&tmp.0, |world, _| {
        world["content"]["weather"] = serde_json::json!("rain");
    });
    let (code, message) = refusal(build(&tmp.0).expect_err("noon in rain is bright"));
    assert_eq!(code, "DW0920", "{message}");
}

/// A cut to midnight that can only happen after the kill completes does not
/// darken the fight: the rule dates the clock against the fight.
#[test]
fn a_dark_cut_after_the_kill_does_not_save_it() {
    let tmp = TempCampaign::new("cut-after");
    choir(&tmp.0, |_, quests| {
        quests["content"]["quests"][0]["on_objective_complete"]["obj/purge"] = to_midnight();
    });
    let (code, message) = refusal(build(&tmp.0).expect_err("the cut comes too late"));
    assert_eq!(code, "DW0920", "{message}");
}

// --- green: each condition is necessary --------------------------------------

/// Thunder darkens the level below the drowned's threshold.
#[test]
fn thunder_is_silent() {
    let tmp = TempCampaign::new("thunder");
    choir(&tmp.0, |world, _| {
        world["content"]["weather"] = serde_json::json!("thunder");
    });
    build(&tmp.0).expect("a drowned fights a land target in a thunderstorm");
}

/// Night is dark.
#[test]
fn midnight_is_silent() {
    let tmp = TempCampaign::new("midnight");
    choir(&tmp.0, |world, _| {
        world["content"]["time"] = serde_json::json!("midnight");
    });
    build(&tmp.0).expect("a drowned fights a land target at night");
}

/// The same cut, fired by the objective that summons the wave: it lands before
/// the kill, so the fight may be fought in the dark and the rule withholds.
#[test]
fn a_dark_cut_before_the_kill_is_silent() {
    let tmp = TempCampaign::new("cut-before");
    choir(&tmp.0, |_, quests| {
        let muster = &mut quests["content"]["quests"][0]["on_objective_complete"]["obj/muster"];
        for e in to_midnight().as_array().unwrap() {
            muster.as_array_mut().unwrap().push(e.clone());
        }
    });
    build(&tmp.0).expect("the fight can be fought at midnight");
}

/// A body whose targeting does not read the hour.
#[test]
fn a_helmeted_zombie_is_silent() {
    let tmp = TempCampaign::new("zombie");
    choir(&tmp.0, |_, quests| {
        quests["content"]["waves"][0]["mobs"][0]["entity"] = serde_json::json!("minecraft:zombie");
    });
    build(&tmp.0).expect("a zombie fights on land at noon");
}

/// The prescribed remedy, and the one vesperhold took: the fight's floor is
/// waterlogged bottom slabs, so a body standing on it stands in water. Same
/// wave, same noon.
#[test]
fn a_waterlogged_floor_is_silent() {
    let tmp = TempCampaign::new("wet");
    choir(&tmp.0, |_, _| {});
    wet_floor(&tmp.0, true);
    build(&tmp.0).expect("a drowned fights a body standing in water at noon");
}

/// The perturbation only this rule sees: the same slabs, dry. Nothing about the
/// geometry the route proofs walk changes; the water does.
#[test]
fn the_same_floor_dry_is_dw0920() {
    let tmp = TempCampaign::new("dry-slabs");
    choir(&tmp.0, |_, _| {});
    wet_floor(&tmp.0, false);
    let (code, message) = refusal(build(&tmp.0).expect_err("dry slabs are dry ground"));
    assert_eq!(code, "DW0920", "{message}");
}

/// Lay the room's floor as bottom slabs, waterlogged or not, by a stage-7 fill —
/// over a course of stone, since the fixture's floor is the bottom of its piece
/// and a waterlogged slab over the void pours out of the world (`DW0318`).
fn wet_floor(dir: &Path, waterlogged: bool) {
    let path = dir.join("world-edits.json");
    let mut edits: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    edits["content"]["batches"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "area": "area/keep",
            "id": "batch/flood-the-floor",
            "note": "The yard floor laid as bottom slabs.",
            "edits": [
                {
                    "verb": "select",
                    "name": "region/under-floor",
                    "shape": {
                        "kind": "box",
                        "frame": { "kind": "piece-local", "piece": 0, "prefab": "prefab/hello-room" },
                        "min": [0, -1, 0],
                        "max": [10, -1, 10]
                    }
                },
                {
                    "verb": "fill",
                    "region": "region/under-floor",
                    "recipe": {
                        "blocks": [{ "block": "minecraft:stone", "weight": 1.0 }],
                        "scale": 0.5
                    }
                },
                {
                    "verb": "select",
                    "name": "region/yard-floor",
                    "shape": {
                        "kind": "box",
                        "frame": { "kind": "piece-local", "piece": 0, "prefab": "prefab/hello-room" },
                        "min": [1, 0, 1],
                        "max": [9, 0, 9]
                    }
                },
                {
                    "verb": "fill",
                    "region": "region/yard-floor",
                    "recipe": {
                        "blocks": [{
                            "block": format!(
                                "minecraft:tuff_slab[type=bottom,waterlogged={waterlogged}]"
                            ),
                            "weight": 1.0
                        }],
                        "scale": 0.5
                    }
                }
            ]
        }));
    std::fs::write(&path, serde_json::to_string_pretty(&edits).unwrap()).unwrap();
}
