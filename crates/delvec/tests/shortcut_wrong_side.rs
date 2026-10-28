//! A sealed shortcut door is pressed from both sides, and its sides are the
//! compiler's to derive (`compiler::wrongside`, spec-0117 §3a).
//!
//! The door's body is its own block: the gate's own answer, a `use` trigger with
//! `serves: {gate}` at a cell of the bars, is a click box fitted over that cell,
//! armed at world-load and taken down by `shortcut_open_<id>`. A presser on the
//! sealed side hears the trigger's line; one on the far side hears the
//! shortcut's `answers.not_yet`. Which side is which is derived from the
//! geometry (`DW0425` when it cannot be), never typed.

mod common;

use std::collections::BTreeMap;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildFailure, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{AnchorId, Campaign, EnvTrigger, parse_campaign};

const NS: &str = "souls-shortcut";

fn fixture() -> Campaign {
    let dir = common::compiler_fixtures_dir().join(NS);
    let loaded = load_campaign_dir(&dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("souls-shortcut parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let diags = common::validation_diagnostics(
        &campaign,
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    );
    assert!(diags.is_empty(), "fixture must validate clean: {diags:#?}");
    campaign
}

fn try_build(campaign: &Campaign) -> Result<BuildOutput, BuildFailure> {
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
        &BTreeMap::new(),
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
}

fn build() -> BuildOutput {
    try_build(&fixture()).expect("every emitted command validates")
}

fn function(out: &BuildOutput, name: &str) -> String {
    let suffix = format!("/function/{name}.mcfunction");
    out.iter()
        .find(|(p, _)| p.starts_with("datapack/") && p.ends_with(&suffix))
        .map(|(_, b)| String::from_utf8(b.clone()).unwrap())
        .unwrap_or_else(|| panic!("no shipped function `{name}`"))
}

/// The door's own answer, from the sealed side; the shortcut's far-side line,
/// from the other. Both read one derived side volume, in complement.
#[test]
fn the_door_answers_each_side_with_its_own_line() {
    let out = build();
    let own = function(&out, "press_inner_door_barred");
    let far = function(&out, "press_dw_answer_sc_inner_door_not_yet");
    let vol = |body: &str, word: &str| {
        body.split_whitespace()
            .collect::<Vec<_>>()
            .windows(3)
            .find(|w| w[0] == word && w[1] == "entity" && w[2].starts_with("@s[x="))
            .map(|w| w[2].to_string())
    };
    let sealed = vol(&own, "if")
        .unwrap_or_else(|| panic!("the own answer is held to the sealed side: {own}"));
    let not_sealed = vol(&far, "unless")
        .unwrap_or_else(|| panic!("the far-side line is held off the sealed side: {far}"));
    assert_eq!(sealed, not_sealed, "one derived volume, read both ways");
    // The sealed side is the spawn's (z < 6): the volume ends short of the door.
    let field = |k: &str| -> i32 {
        sealed
            .trim_start_matches("@s[")
            .trim_end_matches(']')
            .split(',')
            .find_map(|kv| kv.strip_prefix(&format!("{k}=")))
            .unwrap()
            .parse()
            .unwrap()
    };
    assert!(field("z") + field("dz") < 6, "{sealed}");
}

/// The box retires with the bars: an opened doorway that still intercepts a
/// press would swallow right-clicks aimed through it.
#[test]
fn opening_the_shortcut_takes_the_box_down() {
    let open = function(&build(), "shortcut_open_inner_door");
    let fill = open
        .lines()
        .position(|l| l.starts_with("fill ") && l.contains("minecraft:air replace"))
        .expect("the unlock clears the gate region");
    let kill = open
        .lines()
        .position(|l| l == "kill @e[tag=dw_gate_door]")
        .expect("the unlock retires the door's box");
    assert!(kill > fill, "the bars go first, their box after: {open}");
}

/// The author's own effects are what a press produces; the compiler writes no
/// player-facing prose for the door.
#[test]
fn the_answer_is_the_campaigns_own() {
    let body = function(&build(), "trig_inner_door_barred");
    assert!(
        body.contains("The door is barred from the other side."),
        "the author's line is what the press runs: {body}"
    );
}

// ---------------------------------------------------------------------------
// The diagnostics
// ---------------------------------------------------------------------------

/// `DW0426` — the unbound-vacuity class, as a check. A click trigger anchored
/// where nothing is clickable declares an anchor, a click and a full effect
/// bundle, emits, and the press lands on nothing: the beat never happens and
/// every board stays green. This is the shape of the gap the task came from.
#[test]
fn a_trigger_that_can_never_be_pressed_is_dw0426() {
    let mut c = fixture();
    c.quests.content.triggers = vec![
        serde_json::from_str::<EnvTrigger>(
            r#"{ "id": "trigger/nowhere", "at": "anchor/not-a-place", "on": { "on": "use" },
                 "effects": [ { "type": "narrate", "text": "x", "style": "chat" } ] }"#,
        )
        .expect("trigger parses"),
    ];
    let err = try_build(&c).expect_err("an unpressable trigger must fail the build");
    let BuildFailure::Diagnostic { code, message } = err else {
        panic!("expected a diagnostic, got {err:?}");
    };
    assert_eq!(code, "DW0426");
    assert!(
        message.contains("trigger/nowhere") && message.contains("anchor/not-a-place"),
        "DW0426 names the trigger and its anchor: {message}"
    );
}

/// `DW0425` — the compiler will not guess which side of a door is sealed. Here
/// the unlock resolves level with the doorway on the gate's thin axis, so the
/// geometry names no side, and placing the bodies on a guess would put the
/// author's "it will not open" exactly where it does.
#[test]
fn an_underivable_side_is_dw0425() {
    let mut c = fixture();
    c.quests.content.shortcuts[0].unlock = AnchorId("anchor/door".to_string());
    let err = try_build(&c).expect_err("an underivable side must fail the build");
    let BuildFailure::Diagnostic { code, message } = err else {
        panic!("expected a diagnostic, got {err:?}");
    };
    assert_eq!(code, "DW0425");
    assert!(
        message.contains("shortcut/inner-door") && message.contains("anchor/door"),
        "DW0425 names the shortcut and its gate: {message}"
    );
}

/// A campaign with no shortcut emits none of the door machinery.
#[test]
fn a_campaign_without_shortcuts_emits_no_door_bodies() {
    let mut c = fixture();
    c.quests.content.shortcuts.clear();
    c.quests.content.triggers.clear();
    let out = try_build(&c).expect("builds");
    assert!(
        !out.keys().any(|p| p.contains("/seal_arm_")),
        "no door machinery without a shortcut"
    );
}

// ---------------------------------------------------------------------------
// `validation/press-bodies.json` — DW0426's binding count (staging-gate row
// `bell-11`). DW0426 is error-tier, so a build that ships proves "no press
// lands on nothing"; that sentence is equally true of a campaign that arms no
// press at all, and only the count separates the two.
// ---------------------------------------------------------------------------

fn press_ledger(out: &BuildOutput) -> serde_json::Value {
    serde_json::from_slice(
        out.get("validation/press-bodies.json")
            .expect("every build that assembles a world states this proof's binding count"),
    )
    .unwrap()
}

#[test]
fn the_press_ledger_names_every_click_and_the_body_it_landed_on() {
    let l = press_ledger(&build());
    assert_eq!(l["code"], "DW0426");
    assert_eq!(l["unbound"], false);
    assert!(l["reason"].is_null());
    let n = l["examined"].as_u64().unwrap();
    assert!(n >= 1, "the fixture arms a click on the door: {l}");
    assert_eq!(l["presses"].as_array().unwrap().len() as u64, n);
    let door = l["presses"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["anchor"] == "anchor/door-leaf")
        .unwrap_or_else(|| panic!("the door's own press is in the ledger: {l}"));
    assert!(
        door["body"].as_str().unwrap().contains("gate"),
        "the ledger records WHICH body the click landed on, not merely that it landed: {door}"
    );
}

/// The zero, named. A campaign that arms no click at all has not PASSED
/// `DW0426`; it is outside it, and the ledger says so rather than shipping an
/// empty array a reader has to notice.
#[test]
fn a_campaign_that_arms_no_click_states_its_own_zero() {
    let mut c = fixture();
    c.quests.content.triggers.clear();
    c.quests.content.shortcuts.clear();
    let out = try_build(&c).expect("a campaign with no trigger builds");
    let l = press_ledger(&out);
    assert_eq!(l["examined"], 0);
    assert_eq!(l["unbound"], true);
    assert!(
        l["reason"]
            .as_str()
            .unwrap_or_default()
            .contains("nothing here can be pressed"),
        "the zero must be a named finding, never silence: {l}"
    );
}
