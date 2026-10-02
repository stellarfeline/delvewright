//! `DW0496` — a daylight-burning body may not be staged where the sun can
//! reach it.
//!
//! ## The incident this exists for (`hollow-vigil`)
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

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildFailure};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
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
    let prefabs = PrefabRegistry::load_dir(&common::shown_prefabs_dir("daylight")).unwrap();
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
        "must forbid the fix that is not sanctioned: {message}"
    );
}

/// **The `DW0496` / `DW0898` pair** (spec-0067 §4.2, criterion 12). A zombie
/// horse burns in daylight and its body draws no head slot, so the head piece
/// the zombie is prescribed would itself be refused. The prescription is read
/// from the body table: roofing, and never `equipment.head`. The zombie, whose
/// body draws a head, is prescribed both.
#[test]
fn the_prescription_names_a_head_piece_only_for_a_body_that_shows_one() {
    let tmp = TempCampaign::new("zombie-horse");
    campaign_with(tmp.path(), true, |_, quests| {
        set_mobs(
            quests,
            serde_json::json!([
                { "entity": "minecraft:zombie_horse", "count": 1, "name": "The Dead Destrier" }
            ]),
        );
    });
    let err = build(tmp.path()).expect_err("a zombie horse burns under open sky at noon");
    assert_eq!(code_of(&err), "DW0496", "{}", message_of(&err));
    let horse = message_of(&err);
    assert!(horse.contains("Roof the ground"), "names roofing: {horse}");
    assert!(
        !horse.contains("Give this stack `equipment.head`"),
        "prescribes no head piece to a body that shows none: {horse}"
    );

    let tmp = TempCampaign::new("zombie-pair");
    campaign_with(tmp.path(), true, |_, _| {});
    let zombie = message_of(&build(tmp.path()).expect_err("the zombie burns"));
    assert!(
        zombie.contains("Give this stack `equipment.head`") && zombie.contains("roof the ground"),
        "the zombie is prescribed both remedies: {zombie}"
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

/// Night is not a burning hour. (Not a *prescription* — `set-time` is
/// forbidden — but the rule must not fire on a delve authored at night.)
#[test]
fn a_night_world_is_silent() {
    let tmp = TempCampaign::new("night");
    campaign_with(tmp.path(), true, |world, _| {
        world["content"]["time"] = serde_json::json!("midnight");
    });
    build(tmp.path()).expect("nothing burns at midnight");
}

/// Rain protects only where it falls. The burn tick skips a body that is "in
/// rain", and the pinned game counts a body in rain only where the biome at its
/// cell precipitates. A void delve's play box stands in the delve's own void
/// biome, which rains (vanilla's `minecraft:the_void` does not), so a declared
/// `rain` reaches the garrison and it does not burn.
#[test]
fn rain_protects_a_void_delve() {
    let tmp = TempCampaign::new("rain-void");
    campaign_with(tmp.path(), true, |world, _| {
        world["content"]["weather"] = serde_json::json!("rain");
    });
    build(tmp.path()).expect("rain falls on the delve's own void biome, so the garrison stays wet");
}

/// The same holds for `thunder`: vanilla thunder is rain with strikes.
#[test]
fn thunder_protects_a_void_delve() {
    let tmp = TempCampaign::new("thunder-void");
    campaign_with(tmp.path(), true, |world, _| {
        world["content"]["weather"] = serde_json::json!("thunder");
    });
    build(tmp.path()).expect("thunder rains on the delve's own void biome");
}

/// The dual: over an ocean horizon the play box stands in `minecraft:ocean`,
/// which rains, so the declared rain reaches the garrison and it does not burn.
#[test]
fn rain_protects_where_it_falls() {
    let tmp = TempCampaign::new("rain-ocean");
    campaign_with(tmp.path(), true, |world, _| {
        world["content"]["weather"] = serde_json::json!("rain");
        world["content"]["horizon"] = serde_json::json!("ocean");
        world["content"]["boundary"] = serde_json::json!({});
    });
    build(tmp.path()).expect("rain falls on the ocean biome, so the garrison stays wet");
}

/// `dusk` (12000) is inside the pinned `minecraft:day` timeline's
/// `monsters_burn` window, which only turns off at tick 12542.
#[test]
fn dusk_burns() {
    let tmp = TempCampaign::new("dusk");
    campaign_with(tmp.path(), true, |world, _| {
        world["content"]["time"] = serde_json::json!("dusk");
    });
    let err = build(tmp.path()).expect_err("the pinned game burns undead at dusk");
    assert_eq!(code_of(&err), "DW0496", "{}", message_of(&err));
    assert!(message_of(&err).contains("`dusk`"), "{}", message_of(&err));
}

/// `dawn` (23000) is before the window turns back on at tick 23460.
#[test]
fn dawn_is_silent() {
    let tmp = TempCampaign::new("dawn");
    campaign_with(tmp.path(), true, |world, _| {
        world["content"]["time"] = serde_json::json!("dawn");
    });
    build(tmp.path()).expect("nothing burns at dawn");
}

/// Prepend a clock cut to the bundle that seats the garrison.
fn cut_before_the_spawn(quests: &mut serde_json::Value, cut: serde_json::Value) {
    quests["content"]["quests"][0]["on_objective_complete"]["obj/muster"]
        .as_array_mut()
        .unwrap()
        .insert(0, cut);
}

/// A delve that opens at midnight and brings the sun up in the very bundle
/// that seats the garrison fights it in daylight. The rule used to withhold on
/// any campaign that cuts its clock.
#[test]
fn a_cut_to_noon_before_the_spawn_is_dw0496() {
    let tmp = TempCampaign::new("cut-noon");
    campaign_with(tmp.path(), true, |world, quests| {
        world["content"]["time"] = serde_json::json!("midnight");
        cut_before_the_spawn(
            quests,
            serde_json::json!({ "type": "set-time", "time": "noon" }),
        );
    });
    let err = build(tmp.path()).expect_err("the garrison is seated at noon");
    assert_eq!(code_of(&err), "DW0496", "{}", message_of(&err));
    assert!(message_of(&err).contains("`noon`"), "{}", message_of(&err));
}

/// The dual: a noon delve whose night falls in the bundle that seats the
/// garrison fights it in the dark, and the declared noon is behind it.
#[test]
fn a_cut_to_midnight_before_the_spawn_is_silent() {
    let tmp = TempCampaign::new("cut-midnight");
    campaign_with(tmp.path(), true, |_, quests| {
        cut_before_the_spawn(
            quests,
            serde_json::json!({ "type": "set-time", "time": "midnight" }),
        );
    });
    build(tmp.path()).expect("the garrison is seated after night falls");
}

/// A cut after the garrison is dead cannot burn it: the `kill` objective
/// closes before the quest's own `on_complete` runs, and no rest re-seats it.
#[test]
fn a_cut_to_noon_after_the_kill_is_silent() {
    let tmp = TempCampaign::new("cut-after");
    campaign_with(tmp.path(), true, |world, quests| {
        world["content"]["time"] = serde_json::json!("midnight");
        quests["content"]["quests"][0]["on_complete"]
            .as_array_mut()
            .unwrap()
            .insert(0, serde_json::json!({ "type": "set-time", "time": "noon" }));
    });
    build(tmp.path()).expect("the sun comes up on a dead garrison");
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
/// equipped with helmets through commands"), and its body draws no head slot,
/// so the prescription names a roof and never the head piece — and the head
/// piece itself is refused where it is declared (`DW0898`), so the pair of
/// gates never prescribes what the other refuses.
#[test]
fn a_phantom_is_prescribed_a_roof_and_refused_a_helmet() {
    let tmp = TempCampaign::new("phantom");
    campaign_with(tmp.path(), true, |_, quests| {
        set_mobs(
            quests,
            serde_json::json!([{
                "entity": "minecraft:phantom",
                "count": 1,
                "name": "The Long Night"
            }]),
        );
    });
    let err = build(tmp.path()).expect_err("a phantom burns under open sky at noon");
    assert_eq!(code_of(&err), "DW0496");
    let message = message_of(&err);
    assert!(message.contains("phantom"), "names the species: {message}");
    assert!(
        message.contains("Roof the ground")
            && !message.contains("Give this stack `equipment.head`"),
        "prescribes a roof and no head piece: {message}"
    );

    let helmeted = TempCampaign::new("phantom-helmet");
    campaign_with(helmeted.path(), true, |_, quests| {
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
    let loaded = load_campaign_dir(helmeted.path()).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("fixture parses");
    let prefabs = PrefabRegistry::load_dir(&common::shown_prefabs_dir("daylight")).unwrap();
    let refused: Vec<Diagnostic> = validate_campaign_with(
        &campaign,
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    )
    .into_iter()
    .filter(|d| d.code == "DW0898")
    .collect();
    assert_eq!(
        refused.len(),
        1,
        "a helmet on a phantom is refused: {refused:#?}"
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

// --- spec-0080: the biome a fight stands in is the map's -----------------------

/// An atmosphere the creator declared dry. Under a rainy `dusk` the keep would
/// be wet in the horizon's biome; carried by the yard, it is not.
fn dry_yard(world: &mut serde_json::Value, precipitation: &str) {
    world["content"]["time"] = serde_json::json!("dusk");
    world["content"]["weather"] = serde_json::json!("rain");
    world["content"]["atmospheres"] = serde_json::json!([
        { "id": "atmosphere/yard", "precipitation": precipitation }
    ]);
}

/// A body staged in a `precipitation: none` atmosphere under `dusk` + `rain`
/// burns: the creator has said no rain falls there, and the proof holds them to
/// it.
#[test]
fn a_dry_atmosphere_under_rain_at_dusk_is_dw0496() {
    let tmp = TempCampaign::new("atm-dry");
    campaign_with(tmp.path(), true, |world, _| {
        dry_yard(world, "none");
        world["content"]["areas"][0]["atmosphere"] = serde_json::json!("atmosphere/yard");
    });
    let err = build(tmp.path()).expect_err("no rain falls in the yard's atmosphere");
    assert_eq!(code_of(&err), "DW0496", "{}", message_of(&err));
    assert!(
        message_of(&err).contains("daylight-yard:atmosphere/yard"),
        "the message names the biome the cell stands in: {}",
        message_of(&err)
    );
}

/// The same body in a `rain` atmosphere stays wet.
#[test]
fn a_rainy_atmosphere_under_rain_at_dusk_is_silent() {
    let tmp = TempCampaign::new("atm-rain");
    campaign_with(tmp.path(), true, |world, _| {
        dry_yard(world, "rain");
        world["content"]["areas"][0]["atmosphere"] = serde_json::json!("atmosphere/yard");
    });
    build(tmp.path()).expect("rain falls in the yard's atmosphere");
}

/// A trigger repaints the fight's ground dry: a cut of the same kind as
/// `set-weather`, with no place in the DAG, so the fight can stand in it.
#[test]
fn a_repaint_to_dry_from_a_trigger_is_dw0496() {
    let tmp = TempCampaign::new("atm-repaint");
    campaign_with(tmp.path(), true, |world, quests| {
        dry_yard(world, "none");
        quests["content"]["triggers"] = serde_json::json!([{
            "id": "trigger/the-air-turns",
            "at": "anchor/exit",
            "on": { "on": "approach", "range": 2 },
            "effects": [
                { "type": "set-atmosphere", "atmosphere": "atmosphere/yard", "place": "area/keep" }
            ]
        }]);
    });
    let err = build(tmp.path()).expect_err("the repaint dries the yard");
    assert_eq!(code_of(&err), "DW0496", "{}", message_of(&err));
    // The control: the same yard with no repaint stands in the horizon's
    // biome, which rains, so what reds above is the repaint and nothing else.
    let control = TempCampaign::new("atm-repaint-control");
    campaign_with(control.path(), true, |world, _| dry_yard(world, "none"));
    build(control.path()).expect("without the repaint the yard is wet");
}
