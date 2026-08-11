//! `tests/fixtures/die-retry` — the smallest campaign that lets the bot tier's
//! **die-retry** stage bind (task #68).
//!
//! # Why a fixture had to be built for it
//!
//! The die-retry stage's premise is *"dying is always safe"*: death → respawn at
//! the governing checkpoint → walk back → re-engage, with no progression lost. It
//! is the load-bearing combat proof of a souls-shaped delve, and its precondition
//! is an **armed checkpoint before a mandatory encounter**. An encounter with none
//! is excluded with an advisory (#223) — correctly, because where a campaign puts
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
//! # Why the keep is five pieces and not one room
//!
//! The precondition has a second half nobody had had to satisfy before, because
//! nobody had satisfied the first: the governing checkpoint must ALSO clear every
//! contemporaneous hostile's perception radius (`DW0478`). Built over a single
//! `prefab/hello-room`, this fixture put the marked stone 3 blocks from the guard
//! that kills the party — a respawn straight back into contact, which is the
//! soft-lock `DW0478` exists to refuse. The fixture was defective content and the
//! diagnostic was right.
//!
//! The two obligations are jointly satisfiable, but only above a **minimum world
//! scale**: the governing checkpoint and the fight it governs must be more than
//! one perception radius apart (16 blocks by default; a lane wave's
//! `aggro_radius` + 7.9 blocks of measured marching drift) and still walkably
//! connected, because the bot's loop is respawn → *walk back* → re-engage. An
//! 11x11 room cannot hold both, so a one-room delve is a shape that can ship but
//! can never bind this stage. **The relief `DW0478` grants a plain
//! `set-checkpoint` — a reign that ends before the force's onset — is
//! structurally unavailable here**: a checkpoint retired before the wave is
//! staged is by definition not that encounter's governing checkpoint. Geometry is
//! the only free variable, which is why the fix was the world and not the beats.
//!
//! Five pieces is the floor, not a preference: `pool/stone-keep` seats the
//! required pieces as entry → gate room (`anchor/keeper-stand`) → boss hall
//! (`anchor/boss`), and the solver splits filler evenly across the gaps *before*
//! each required room, so a 4-piece budget spends its one filler ahead of the
//! gate room and leaves the stone 15.0 blocks from the guard — still inside the
//! 16. The fifth piece is the first that buys a corridor BETWEEN them (23.0
//! blocks). Nothing else about the campaign changed: same two quests, same one
//! wave, same four critical-path steps.
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
    })
    .expect("campaign parses")
}

fn build(campaign: &Campaign, dir: &std::path::Path) -> BuildOutput {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let bytes = std::fs::read(common::prefabs_dir().join(&piece.structure_file)).unwrap();
            structures.insert(piece.structure_file.clone(), bytes);
        }
    }
    emit::build(
        &plan,
        &common::campaign_inputs(dir),
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        "unpinned",
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

    // …and that checkpoint is a cell the party can survive arriving on. The build
    // succeeding is not by itself evidence of that: `DW0478` is silent on a
    // campaign it never examined, so the fixture states its binding count the way
    // every validation artifact must. A zero here would mean this fixture dodged
    // the safe-zone proof rather than passing it.
    let safety = json(&out, "validation/respawn-safety.json");
    assert_eq!(
        safety["unbound"], false,
        "the fixture's checkpoint must be MEASURED against the guard, not merely \
         left unexamined: {safety:#}"
    );
    assert!(
        safety["pairs"].as_u64().unwrap_or(0) >= 1,
        "one respawn point against the one force that can kill the party: {safety:#}"
    );

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
///
/// Minimal now has a floor it did not have when the campaign was one room: the
/// world must be wide enough for `DW0478` (see the module docs). So the guard
/// counts the ASSEMBLY as well as the beats — a piece budget that drifts upward
/// would otherwise be the one dimension of this fixture nothing watches.
#[test]
fn the_fixture_stays_the_smallest_thing_that_binds() {
    let campaign = parse_dir(&fixture_dir());
    assert_eq!(campaign.quests.content.waves.len(), 1, "one wave");
    assert_eq!(campaign.quests.content.quests.len(), 2, "two quests");
    assert_eq!(campaign.world.content.areas.len(), 1, "one area");

    // Five placed pieces, and five is the FLOOR rather than a taste: four leaves
    // the marked stone 15.0 blocks from the guard's seated cell, inside the
    // 16-block default `follow_range`, and the build is `DW0478`. Widening the
    // world is the only lever left — shrinking the guard's perception radius is
    // what the diagnostic itself forbids, and retiring the checkpoint would
    // delete the very precondition this fixture exists to satisfy.
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let placed: usize = plan.areas.iter().map(|a| a.pieces.len()).sum();
    assert_eq!(
        placed, 5,
        "the keep is the smallest assembly that puts the checkpoint more than one \
         perception radius from the fight it governs; {placed} pieces means the \
         fixture grew for some other reason"
    );

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
