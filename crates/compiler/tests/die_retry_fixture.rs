//! `tests/fixtures/die-retry` — the smallest campaign that lets the bot tier's
//! **die-retry** stage bind.
//!
//! # Why a fixture had to be built for it
//!
//! The die-retry stage's premise is *"dying is always safe"*: death → respawn at
//! the governing checkpoint → walk back → re-engage, with no progression lost. It
//! is the load-bearing combat proof of a souls-shaped delve, and its precondition
//! is an **armed checkpoint before a mandatory encounter**. An encounter with none
//! is excluded with an advisory — correctly, because where a campaign puts
//! its rest points is `DW0379`/`DW0315`/`DW0316`'s judgement, not the bot's.
//!
//! Measured 2026-08-11 across every campaign and fixture in **both** repos: not
//! one build satisfied it. `keep-trial` and `hollow-vigil` field encounters with
//! no checkpoint before them; `nobodys-cave-island` compiles to zero mandatory
//! encounters; the drowned bell has no stage documents yet. So the stage had
//! reported green over **zero scripted deaths** everywhere it had ever run — the
//! same shape as the island's combat floor gate examining zero enemies for
//! nineteen rounds. The run report now states that as a binding count
//! (`die_retry_binding`), and this fixture is what gives the stage something to
//! examine.
//!
//! # Why it is a keep and not a room
//!
//! The first version put the checkpoint and the guard in one `prefab/hello-room`,
//! four blocks apart. `DW0478` refuses that, and is right to: a respawn point
//! inside a hostile's perception radius delivers the party into contact on the
//! tick they arrive, which is a soft-lock and the opposite of the property this
//! fixture exists to exercise. No placement inside an 11x11x6 room can satisfy
//! it — the room's longest diagonal is under the default 16-block
//! `follow_range`, so the constraint is geometric and not a matter of where the
//! anchors sit. Shrinking `follow_range` to buy the clearance is what the
//! diagnostic tells an author not to do, because it retunes the fight to hide a
//! placement bug.
//!
//! So the area is the `pool/stone-keep` jigsaw, whose pieces carry every anchor
//! this fixture names (`keeper-stand`, `door`, `exit`) in separate rooms: the
//! guard rises 27.7 blocks from the stone the party comes back to, and the walk
//! between them IS the loop under test. It is still one area — areas sit
//! `AREA_SPACING` apart across void, which a pathfinder-free bot cannot cross.
//!
//! # What this test holds
//!
//! Not that the loop passes — a server and a bot decide that, and
//! `.github/workflows/release.yml` runs them. What it holds is the property that
//! makes the loop *reachable at all*, and which nothing else in the repo asserts:
//! this campaign compiles to a mandatory encounter whose governing checkpoint is
//! set by an EARLIER step. Without it the fixture could quietly drift back into
//! the state every other campaign is in, and the ladder would go on passing.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};

fn fixture_dir() -> std::path::PathBuf {
    common::compiler_fixtures_dir().join("die-retry")
}

fn parse_dir(dir: &std::path::Path) -> Campaign {
    let read = |n: &str| std::fs::read_to_string(dir.join(n)).unwrap();
    parse_campaign(&RawCampaign {
        world: read("world.json"),
        npcs: read("npcs.json"),
        classes: read("classes.json"),
        quest_plan: read("quest-plan.json"),
        quests: read("quests.json"),
        dialogue: read("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
    })
    .expect("campaign parses")
}

fn build(campaign: &Campaign, dir: &std::path::Path) -> BuildOutput {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    emit::build(
        &plan,
        &common::campaign_inputs(dir),
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
    .expect("the die-retry fixture must build")
}

fn json(out: &BuildOutput, path: &str) -> serde_json::Value {
    let bytes = out.get(path).unwrap_or_else(|| panic!("{path} is emitted"));
    serde_json::from_slice(bytes).expect("valid json")
}

/// The one property that makes the die-retry stage reachable: a mandatory
/// encounter, with a checkpoint the campaign sets at an EARLIER step.
#[test]
fn the_fixture_gives_the_die_retry_stage_something_to_bind_on() {
    let campaign = parse_dir(&fixture_dir());
    let out = build(&campaign, &fixture_dir());
    let plan = json(&out, "validation/combat-plan.json");

    let encounters = plan["encounters"]
        .as_array()
        .expect("the plan carries an encounters array");
    assert_eq!(
        encounters.len(),
        1,
        "exactly one mandatory encounter — the SMALLEST thing that binds, so a \
         failure here is about the loop and never about which fight it was: {plan:#}"
    );
    let enc = &encounters[0];

    // The precondition, stated the way `checkpointPrecondition` reads it: a
    // governing checkpoint must EXIST. `null` here is the state of every other
    // campaign in both repos, and it is what makes the stage skip and report a
    // green having scripted no death.
    let cp = enc["checkpoint"].as_array().unwrap_or_else(|| {
        panic!(
            "the encounter has NO governing checkpoint, so the die-retry stage will \
             skip it and pass having proved nothing — which is the exact condition \
             this fixture was built to escape: {plan:#}"
        )
    });
    assert_eq!(cp.len(), 3, "the checkpoint resolves to a cell: {plan:#}");

    // …and it must be armed BEFORE the fight. A checkpoint set by the encounter's
    // own kill step is not a checkpoint the party can die back to.
    let step = enc["step"].as_u64().expect("the encounter names its step");
    assert!(
        step >= 1,
        "the encounter cannot be the first step of the path, or nothing can have \
         fired a checkpoint ahead of it: {plan:#}"
    );

    // A plain `set-checkpoint` is armed the moment it fires; a bonfire is armed
    // only once the party RESTS at it, which the die-retry precondition treats as
    // unarmed until the path performs the rest. This fixture uses the former on
    // purpose — it is the shorter of the two loops, and the stage's own
    // precondition check is what would otherwise be under test here.
    let path = json(&out, "critical-path.json");
    let rests = path["steps"]
        .as_array()
        .expect("the path has steps")
        .iter()
        .filter(|s| s["action"] == "rest")
        .count();
    assert_eq!(
        rests, 0,
        "this fixture arms its checkpoint with `set-checkpoint`, not a bonfire — a \
         bonfire would make the stage's precondition depend on a rest step and turn \
         the fixture into a test of the precondition rather than of the loop"
    );
}

/// The fixture stays minimal. A fixture that grows is a fixture whose ladder run
/// gets slower and whose failures stop being about the thing it exists for.
#[test]
fn the_fixture_stays_the_smallest_thing_that_binds() {
    let campaign = parse_dir(&fixture_dir());
    assert_eq!(campaign.quests.content.waves.len(), 1, "one wave");
    assert_eq!(campaign.quests.content.quests.len(), 2, "two quests");
    let out = build(&campaign, &fixture_dir());
    let steps = json(&out, "critical-path.json")["steps"]
        .as_array()
        .expect("the path has steps")
        .len();
    assert!(
        steps <= 6,
        "the whole path is a class pick, a conversation, a fight and the completion \
         assertion; {steps} steps means something else got added"
    );
}
