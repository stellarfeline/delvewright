//! A look build (ADR-0040 §3): `emit::build_look` writes the player tree a
//! proven build writes, and no check refuses it.

use std::collections::BTreeMap;
use std::path::Path;

use crate::common;
use delvec::compiler::check::is_player_path;
use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildFailure, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, parse_campaign};

/// The proven build and the look build of `campaign`, over the engine's own
/// prefab library.
fn both(
    campaign: &Campaign,
    inputs: &BTreeMap<String, Vec<u8>>,
) -> (
    Result<BuildOutput, BuildFailure>,
    Result<BuildOutput, BuildFailure>,
) {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for t in plan.placed_pieces().flat_map(|p| &p.templates) {
        let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
        structures.insert(t.structure_file.clone(), bytes);
    }
    let tree = CommandTree::v1_21_11();
    let skins = BTreeMap::new();
    (
        emit::build(&plan, inputs, &structures, &tree, &prefabs, None, &skins),
        emit::build_look(&plan, inputs, &structures, &tree, &prefabs, None, &skins),
    )
}

fn loaded(dir: &Path) -> (Campaign, BTreeMap<String, Vec<u8>>) {
    let loaded = load_campaign_dir(dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("the fixture parses");
    (campaign, loaded.inputs)
}

fn manifest(out: &BuildOutput) -> serde_json::Value {
    serde_json::from_slice(&out["manifest.json"]).expect("a manifest")
}

/// **A look build is the proven build's player tree, byte for byte, and its
/// manifest.** Over every fixture below: every file a look build writes but
/// `manifest.json` is a player path and equals the proven build's file at that
/// path; every player file of the proven build is in the look build; the look
/// build holds no validation file; and `look_only` is `true` in the look
/// build's manifest and absent from the proven one's. States how many files it
/// compared and refuses a run that compared none. This is what holds a check to
/// writing no player byte: a check that did would be missing from the look
/// build and the two trees would differ.
#[test]
fn a_look_build_writes_the_proven_builds_player_tree_and_nothing_else() {
    let mut compared = 0usize;
    let mut validation = 0usize;
    for dir in [
        common::hello_world_dir(),
        common::keep_crawl_dir(),
        common::keep_trial_dir(),
        common::cutscene_shots_dir(),
        common::keep_vertical_dir(),
    ] {
        let name = dir.file_name().unwrap().to_string_lossy().to_string();
        let (campaign, inputs) = loaded(&dir);
        let (proven, look) = both(&campaign, &inputs);
        let proven = proven.unwrap_or_else(|e| panic!("{name}: the proven build refuses: {e:?}"));
        let look = look.unwrap_or_else(|e| panic!("{name}: the look build refuses: {e:?}"));
        for (path, bytes) in &look {
            if path == "manifest.json" {
                continue;
            }
            assert!(
                is_player_path(path),
                "{name}: the look build wrote `{path}`, a file of the validation tree"
            );
            assert_eq!(
                proven.get(path),
                Some(bytes),
                "{name}: `{path}` differs between the look build and the proven build"
            );
            compared += 1;
        }
        for path in proven.keys().filter(|p| is_player_path(p)) {
            assert!(
                look.contains_key(path),
                "{name}: the proven build's player file `{path}` is missing from the look build"
            );
        }
        validation += proven
            .keys()
            .filter(|p| !is_player_path(p) && *p != "manifest.json")
            .count();
        assert_eq!(
            manifest(&look)["look_only"],
            true,
            "{name}: the look build's mark"
        );
        assert!(
            manifest(&proven).get("look_only").is_none(),
            "{name}: a proven build carries no `look_only`"
        );
    }
    assert!(compared > 0, "no player file was compared");
    assert!(
        validation > 0,
        "the proven builds wrote no validation file, so nothing was left out of a look build"
    );
    eprintln!(
        "look binding: {compared} player file(s) equal in both builds over 5 campaign(s); \
         {validation} validation file(s) the proven builds wrote and the look builds did not"
    );
}

/// **No check refuses a look build.** keep-trial with its first wave grown past
/// the footing its room offers: the proven build refuses (`DW0312`, the check
/// that reads the seating's shortfall), and the look build of the same
/// documents writes its tree, seating the mobs the room has cells for. The
/// refusal is the bench's, so the same derivation that came up short wrote
/// both.
#[test]
fn a_shortfall_a_check_refuses_does_not_stop_a_look_build() {
    let (mut campaign, inputs) = loaded(&common::keep_trial_dir());
    let wave = campaign
        .quests
        .content
        .waves
        .first_mut()
        .expect("keep-trial declares a wave");
    let id = wave.id.as_str().to_string();
    let mob = wave.mobs.first_mut().expect("a wave has a mob");
    mob.count = serde_json::from_value(serde_json::json!(400)).expect("a count");
    let (proven, look) = both(&campaign, &inputs);
    match proven {
        Err(BuildFailure::Diagnostic { code, message }) => {
            assert_eq!(code.to_string(), "DW0312", "{message}");
            assert!(message.contains(&id), "{message}");
        }
        other => panic!(
            "the proven build must refuse the wave: {:?}",
            other.map(|o| o.len())
        ),
    }
    let look = look.expect("the look build is refused by no check");
    assert_eq!(manifest(&look)["look_only"], true);
    assert!(
        look.keys()
            .all(|p| p == "manifest.json" || is_player_path(p))
    );
}

fn structures_of(plan: &Plan) -> BTreeMap<String, Vec<u8>> {
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for t in plan.placed_pieces().flat_map(|p| &p.templates) {
        let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
        structures.insert(t.structure_file.clone(), bytes);
    }
    structures
}

/// **A replay that judges nothing writes what a judging one writes.** The
/// edit replay takes its judge from the build's bench (`edit::replay_taking`'s
/// `enforce`), so a look build replays the script and proves no batch. Over
/// the edit fixture, the replay `delvec build` runs and the one that enforces
/// nothing are held to each other on everything the build writes from: the
/// edited world's blocks, the load commands, each by-effect batch's commands
/// and cells, each batch's bounds and the placement rows. Refuses a fixture
/// that replays no batch.
#[test]
fn an_unjudged_replay_writes_what_a_judged_one_writes() {
    use delvec::compiler::edit;
    let dir = common::compiler_fixtures_dir().join("v06-edits");
    let (campaign, _) = loaded(&dir);
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let structures = structures_of(&plan);
    let judged = edit::replay(&plan, &prefabs, &structures)
        .expect("the judged replay passes")
        .expect("the fixture has edits");
    let unjudged = edit::replay_view(&plan, &prefabs, &structures)
        .expect("the unjudged replay runs")
        .expect("the fixture has edits");
    assert!(!judged.batches.is_empty(), "the fixture replays no batch");
    assert_eq!(judged.commands, unjudged.commands, "the load commands");
    assert_eq!(
        judged.by_effect, unjudged.by_effect,
        "the by-effect commands"
    );
    assert_eq!(judged.by_effect_writes, unjudged.by_effect_writes);
    type Bounds = Option<([i32; 3], [i32; 3])>;
    let bounds = |r: &edit::EditReplay| -> Vec<(String, Bounds)> {
        r.batches.iter().map(|b| (b.id.clone(), b.bounds)).collect()
    };
    assert_eq!(bounds(&judged), bounds(&unjudged), "each batch's bounds");
    assert_eq!(
        format!("{:?}", judged.placements),
        format!("{:?}", unjudged.placements),
        "the placement rows"
    );
    let cells = |r: &edit::EditReplay| -> Vec<([i32; 3], String)> {
        r.assembled
            .blocks
            .iter()
            .map(|(c, b)| (c, b.to_string()))
            .collect()
    };
    let (a, b) = (cells(&judged), cells(&unjudged));
    assert!(!a.is_empty());
    assert_eq!(a, b, "the edited world's blocks");
    eprintln!(
        "replay binding: {} batch(es), {} load command(s), {} edited cell(s) equal judged and \
         unjudged",
        judged.batches.len(),
        judged.commands.len(),
        a.len()
    );
}

/// **A relight pass that judges nothing places the fixtures a judging one
/// places.** The pass takes its judge from the build's bench
/// (`light::relight_judging`), so a look build places the fixtures an area's
/// declared `lighting` asks for and measures no darkness gate. hello-world at
/// night under thunder with a lantern relight declared: both passes place the
/// same fixtures, in the same order, and at least one; and with no lighting
/// declared the unjudged pass surveys nothing — it judges no cell — where the
/// judged one judges some.
#[test]
fn an_unjudged_relight_places_the_fixtures_a_judged_one_places() {
    use delvec::compiler::light;
    use delvewright_dsl::{AreaLighting, Fixture, WorldTime, WorldWeather};
    let (mut campaign, _) = loaded(&common::hello_world_dir());
    campaign.world.content.time = WorldTime::Night;
    campaign.world.content.weather = WorldWeather::Thunder;
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    for declared in [true, false] {
        campaign.world.content.areas[0].lighting = declared.then_some(AreaLighting {
            fixture: Fixture::Lantern,
            min_light: 10,
        });
        let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
        let assembled = delvec::compiler::assembled::assemble(&plan, &structures_of(&plan));
        let geometry = light::geometry_world(&assembled);
        let judged = light::relight_judging(&plan, &assembled, &geometry, None, true);
        let unjudged = light::relight_judging(&plan, &assembled, &geometry, None, false);
        assert_eq!(
            judged.placements, unjudged.placements,
            "lighting declared: {declared}"
        );
        assert!(unjudged.diagnostics.is_empty());
        if declared {
            assert!(
                !judged.placements.is_empty(),
                "the fixture places no lantern"
            );
        } else {
            assert!(
                judged.binding.judged > 0,
                "the judged pass surveys the room"
            );
            assert_eq!(
                unjudged.binding.judged, 0,
                "the unjudged pass surveys nothing"
            );
        }
    }
}
