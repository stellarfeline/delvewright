//! A click trigger gets the body of the **object** at its anchor.
//!
//! ## The finding
//!
//! `EnvTrigger` is already the campaign's general "click a thing, run anything"
//! verb — any anchor, both clicks, the full effect vocabulary, flag gates and
//! `once`. Nothing was missing at the response layer. What was missing is
//! underneath: **the trigger's body is a point at a cell, and an object in the
//! scene is a shape.**
//!
//! Measured on the `souls-shortcut` fixture before this change. A `use` trigger
//! anchored on the shortcut's gate compiled with **zero diagnostics** and emitted
//!
//! ```text
//! summon minecraft:interaction 4.5 65.0 6.5 {width:1.0f,height:2.0f,…,Tags:["dw_trig_…"]}
//! ```
//!
//! whose AABB is `[4.0,65.0,6.0]..[5.0,67.0,7.0]` — inside a doorway slab
//! occupying `[4.0,65.0,6.0]..[6.0,68.0,7.0]`. Flush with the block on the faces
//! it touches, strictly interior on the rest. Vanilla bounds its entity raycast
//! by the block hit and takes the entity only when it is *strictly* nearer, so
//! **that trigger is pressable from no angle at all** — and one box would have
//! covered one of the doorway's six cells even if it were.
//!
//! `close-gate` had solved this privately (shell cells + `SEAL_MARGIN`); nothing
//! else could reach that machinery. These tests pin the general form.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildFailure, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{AnchorId, Campaign, EnvTrigger, parse_campaign, validate_campaign_with};

const NS: &str = "souls-shortcut";

fn fixture() -> Campaign {
    let dir = common::compiler_fixtures_dir().join(NS);
    let loaded = load_campaign_dir(&dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("souls-shortcut parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let diags = validate_campaign_with(
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
            let bytes = std::fs::read(common::prefabs_dir().join(&piece.structure_file)).unwrap();
            structures.insert(piece.structure_file.clone(), bytes);
        }
    }
    emit::build(
        &plan,
        &BTreeMap::new(),
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        "unpinned",
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

/// Every `summon minecraft:interaction` line in the shipped pack carrying `tag`,
/// as `(x, y, z, width, height)`.
fn bodies(out: &BuildOutput, tag: &str) -> Vec<(f64, f64, f64, f64, f64)> {
    let mut v = Vec::new();
    for (path, bytes) in out {
        if !path.starts_with("datapack/") || !path.ends_with(".mcfunction") {
            continue;
        }
        for line in std::str::from_utf8(bytes).unwrap().lines() {
            if !line.starts_with("summon minecraft:interaction") || !line.contains(tag) {
                continue;
            }
            let t: Vec<&str> = line.split_whitespace().collect();
            let grab = |k: &str| {
                line.split(k)
                    .nth(1)
                    .and_then(|r| r.split('f').next())
                    .and_then(|r| r.parse::<f64>().ok())
                    .unwrap_or_else(|| panic!("no {k} in {line}"))
            };
            v.push((
                t[2].parse().unwrap(),
                t[3].parse().unwrap(),
                t[4].parse().unwrap(),
                grab("width:"),
                grab("height:"),
            ));
        }
    }
    v
}

/// The gate slab's solid AABB: cell `c` occupies `[c, c+1]` on each axis.
const SLAB_LO: [f64; 3] = [4.0, 65.0, 6.0];
const SLAB_HI: [f64; 3] = [6.0, 68.0, 7.0];

// ---------------------------------------------------------------------------
// The general fix: a region anchor gets the object's shape
// ---------------------------------------------------------------------------

/// **The regression, stated as geometry.** Every body a click trigger on the
/// barred door is dispatched from must have at least one face strictly outside
/// the solid slab — otherwise vanilla can never find it nearer than the block and
/// the press reaches nothing. On `origin/main` the single emitted body failed
/// this on all six faces.
#[test]
fn the_doors_bodies_are_reachable_from_outside_the_block() {
    let out = build();
    let bs = bodies(&out, "dw_trig_bars_wont_give");
    assert!(!bs.is_empty(), "the trigger must have a body at all");
    for (x, y, z, w, h) in bs {
        let lo = [x - w / 2.0, y, z - w / 2.0];
        let hi = [x + w / 2.0, y + h, z + w / 2.0];
        let protrudes = (0..3).any(|i| lo[i] < SLAB_LO[i] || hi[i] > SLAB_HI[i]);
        assert!(
            protrudes,
            "body {lo:?}..{hi:?} is sealed inside the door {SLAB_LO:?}..{SLAB_HI:?} \
             and no press can reach it"
        );
    }
}

/// …and the object is answered over its whole clickable surface, not at one
/// corner of it. The doorway is six cells.
#[test]
fn every_cell_of_the_doorway_answers() {
    let out = build();
    assert_eq!(
        bodies(&out, "dw_trig_bars_wont_give").len(),
        6,
        "one body per doorway cell"
    );
}

/// The general form, on an anchor with no shortcut and no seal: a trigger on a
/// plain **gate region** anchor now gets that region's clickable shell instead of
/// one buried point box. This is the half that fixes objects nobody has thought
/// about yet.
#[test]
fn a_plain_region_anchor_also_gets_a_region_body() {
    let mut c = fixture();
    // Drop the shortcut so `anchor/door` is an ordinary gate region, and put a
    // `use` trigger on it.
    c.quests.content.shortcuts.clear();
    c.quests.content.triggers = vec![
        serde_json::from_str::<EnvTrigger>(
            r#"{ "id": "trigger/press-the-door", "at": "anchor/door", "on": { "on": "use" },
                 "once": false,
                 "effects": [ { "type": "narrate", "text": "Cold iron.", "style": "chat" } ] }"#,
        )
        .expect("trigger parses"),
    ];
    let out = try_build(&c).expect("builds");
    let bs = bodies(&out, "dw_trig_press_the_door");
    assert_eq!(bs.len(), 6, "the region's whole shell is armed: {bs:?}");
    assert!(
        bs.iter().all(|&(_, _, _, w, h)| w > 1.0 && h > 1.0),
        "each body protrudes past the block it stands in: {bs:?}"
    );
}

/// A trigger anchored on a point in open air is **unchanged** — the ordinary
/// `1.0f x 2.0f` body. Every campaign that only ever anchored triggers in the air
/// emits byte-identically.
#[test]
fn a_point_anchor_is_untouched() {
    let mut c = fixture();
    c.quests.content.shortcuts.clear();
    c.quests.content.triggers = vec![
        serde_json::from_str::<EnvTrigger>(
            r#"{ "id": "trigger/at-the-exit", "at": "anchor/exit", "on": { "on": "use" },
                 "effects": [ { "type": "narrate", "text": "Air.", "style": "chat" } ] }"#,
        )
        .expect("trigger parses"),
    ];
    let out = try_build(&c).expect("builds");
    assert_eq!(
        bodies(&out, "dw_trig_at_the_exit"),
        vec![(5.5, 65.0, 8.5, 1.0, 2.0)],
        "the point body is exactly what it always was"
    );
}

// ---------------------------------------------------------------------------
// Sidedness, as a property of the object
// ---------------------------------------------------------------------------

/// **The answer must never fire where it would be false.** The owner's line is
/// "the door cannot be opened from this side"; said to a player standing where it
/// *does* open, that is a lie, and a lie teaches something wrong where silence
/// teaches nothing.
///
/// The mechanism is placement, not a player test: the bodies stand in the open
/// air on the sealed side (`z = 5`), so a near-side ray reaches them before the
/// block and a far-side ray hits the door and stops. No DSL surface, and no
/// presser identity needed — which matters, because a trigger is dispatched from
/// the tick under the server command source and never knows who pressed it.
#[test]
fn the_door_answers_only_from_the_sealed_side() {
    let out = build();
    let bs = bodies(&out, "dw_trig_bars_wont_give");
    assert!(
        bs.iter().all(|&(_, _, z, ..)| z < 6.0),
        "every body stands in front of the bars on the sealed side: {bs:?}"
    );
    // The unlock is at z=8, so the far side is z>=7: nothing may stand there.
    assert!(
        bs.iter().all(|&(_, _, z, ..)| z < 7.0),
        "nothing on the side the door opens from: {bs:?}"
    );
}

/// The bodies retire with the bars. An opened doorway that still answers "it will
/// not open" is a lie, and an invisible box left standing in a now-walkable
/// threshold swallows right-clicks aimed through it.
#[test]
fn opening_the_shortcut_takes_the_bodies_down() {
    let open = function(&build(), "shortcut_open_inner_door");
    let fill = open
        .lines()
        .position(|l| l.starts_with("fill ") && l.contains("minecraft:air replace"))
        .expect("the unlock clears the gate region");
    let kill = open
        .lines()
        .position(|l| l == "kill @e[tag=dw_ws_inner_door]")
        .expect("the unlock retires the door's bodies");
    assert!(kill > fill, "the bars go first, their body after: {open}");
}

/// One cell, one hitbox. The trigger **rides** the door's bodies rather than
/// summoning its own co-located set — a second box there is the exact ray-pick
/// tie `DW0422` exists to forbid, and the one that killed the island's boulder.
#[test]
fn the_trigger_rides_the_door_rather_than_contesting_it() {
    let out = build();
    let arm = function(&out, "ws_arm_inner_door");
    assert_eq!(
        arm.matches("\"dw_ws_inner_door\",\"dw_trig_bars_wont_give\"")
            .count(),
        6,
        "the trigger's tag rides every one of the door's bodies: {arm}"
    );
    assert!(
        !function(&out, "setup_finish")
            .contains("Tags:[\"dw_fixture\",\"dw_trig_bars_wont_give\"]"),
        "and it summons nothing of its own"
    );
}

/// The author's own effects are what a press produces — prose and sound, gated by
/// the author's own flags. The compiler supplies the body; the campaign supplies
/// the answer, and the compiler writes no player-facing prose here at all.
#[test]
fn the_answer_is_the_campaigns_own() {
    let body = function(&build(), "trig_bars_wont_give");
    assert!(
        body.contains("You set your shoulder to the bars")
            && body.contains("playsound minecraft:block.chain.hit"),
        "the author's prose and sound are what the press runs: {body}"
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
        !out.keys().any(|p| p.contains("/ws_")),
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
        .find(|p| p["anchor"] == "anchor/door")
        .unwrap_or_else(|| panic!("the door's own press is in the ledger: {l}"));
    assert!(
        door["body"].as_str().unwrap().contains("shortcut door"),
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
    let out = try_build(&c).expect("a campaign with no trigger builds");
    let l = press_ledger(&out);
    assert_eq!(l["examined"], 0);
    assert_eq!(l["unbound"], true);
    assert!(
        l["reason"]
            .as_str()
            .unwrap_or_default()
            .contains("has not passed it, it is outside it"),
        "the zero must be a named finding, never silence: {l}"
    );
}
