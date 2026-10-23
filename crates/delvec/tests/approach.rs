//! **A body the party talks to faces the party as they walk up** (`DW1025`,
//! `compiler::approach`).
//!
//! hello-world's keeper is talked to at `anchor/keeper-stand` (`[5, 65, 4]`),
//! and the route proof brings the party into talking reach from `[4, 65, 2]`,
//! north and a little west of the body: bearing 153°. The pinned prefab
//! declares the stand facing `north` (yaw 180), 27° off that approach. Each case
//! varies what the rule reads — the anchor's declared facing, in a scratch copy
//! of the prefab's metadata, and the keeper's `watch` — one at a time:
//!
//! * declaring no `watch`, the keeper watches the nearest player by default
//!   (`dsl::DEFAULT_WATCH_WITHIN`), so a back-turned stand builds;
//! * a class watch turns only to that class, so it is judged by its yaw from
//!   the route's approach: `west` (63° off) faces the party, `east` (117°) and
//!   `south` (153°) turn the keeper's back to them;
//! * `watch: "none"` holds the stand's facing, judged from every cell a player
//!   can talk to the keeper from: in the middle of the room no facing faces
//!   them all, and stood against the north wall (offset `[0, 0, -3]`) the
//!   `south` facing does and the `north` one does not.

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildFailure};
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};
use serde_json::{Value, json};

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

/// A prefab directory holding hello-room with `anchor/keeper-stand` facing `facing`.
fn prefabs(facing: &str) -> PathBuf {
    // One directory per call: the cases run in parallel, and two of them
    // sharing a facing would delete each other's copy.
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "approach-prefabs-{facing}-{}-{n}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let src = common::prefabs_dir();
    std::fs::copy(src.join("hello-room.nbt"), dir.join("hello-room.nbt")).unwrap();
    let mut meta: Value =
        serde_json::from_str(&std::fs::read_to_string(src.join("hello-room.json")).unwrap())
            .unwrap();
    assert_eq!(
        meta["anchors"]["anchor/keeper-stand"]["facing"], "north",
        "the pinned stand's facing is the baseline every case here varies"
    );
    meta["anchors"]["anchor/keeper-stand"]["facing"] = json!(facing);
    std::fs::write(
        dir.join("hello-room.json"),
        serde_json::to_string_pretty(&meta).unwrap(),
    )
    .unwrap();
    dir
}

fn campaign(npcs: impl FnOnce(&mut Value)) -> Campaign {
    campaign_at([0, 0, 0], npcs)
}

/// hello-world with the keeper (and the cast's mark for it) at `offset` from
/// its stand.
fn campaign_at(offset: [i32; 3], npcs: impl FnOnce(&mut Value)) -> Campaign {
    let raw = RawCampaign {
        world: read_hw("world.json"),
        npcs: common::patch_doc(&read_hw("npcs.json"), |n| {
            if offset != [0, 0, 0] {
                n["content"]["npcs"][0]["offset"] = json!(offset);
            }
            npcs(n);
        }),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: common::patch_doc(&read_hw("quests.json"), |q| {
            if offset != [0, 0, 0] {
                q["content"]["quests"][0]["cast"]["npc/keeper"]["at"] =
                    json!({ "anchor": "anchor/keeper-stand", "offset": offset });
            }
        }),
        dialogue: read_hw("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    };
    parse_campaign(&raw).expect("campaign parses")
}

/// The build's verdict: `Ok` or the refusing code and message.
fn build(c: &Campaign, facing: &str) -> Result<(), (String, String)> {
    let dir = prefabs(facing);
    let reg = PrefabRegistry::load_dir(&dir).expect("prefabs load");
    let plan = Plan::build(c, &reg).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                structures.insert(
                    t.structure_file.clone(),
                    std::fs::read(dir.join(&t.structure_file)).unwrap(),
                );
            }
        }
    }
    match emit::build(
        &plan,
        &BTreeMap::new(),
        &structures,
        &CommandTree::v1_21_11(),
        &reg,
        None,
        &BTreeMap::new(),
    ) {
        Ok(_) => Ok(()),
        Err(BuildFailure::Diagnostic { code, message }) => Err((code.to_string(), message)),
        Err(other) => panic!("the build failed outside a diagnostic: {other:?}"),
    }
}

fn watch(who: Value, within: u32) -> impl FnOnce(&mut Value) {
    move |n: &mut Value| {
        n["content"]["npcs"][0]["watch"] = json!({ "who": who, "within": within });
    }
}

fn class_watch() -> impl FnOnce(&mut Value) {
    watch(json!({ "class": "class/wanderer" }), 3)
}

fn held(n: &mut Value) {
    n["content"]["npcs"][0]["watch"] = json!("none");
}

#[test]
fn the_pinned_keeper_faces_the_party_and_builds() {
    assert_eq!(build(&campaign(|_| {}), "north"), Ok(()));
}

#[test]
fn a_keeper_declaring_no_watch_turns_to_the_party_by_default() {
    // The stand faces away from the approach; the default watch answers it.
    assert_eq!(build(&campaign(|_| {}), "east"), Ok(()));
    assert_eq!(build(&campaign(|_| {}), "south"), Ok(()));
}

#[test]
fn a_class_watched_keeper_facing_the_approach_builds() {
    assert_eq!(build(&campaign(class_watch()), "west"), Ok(()));
}

#[test]
fn a_class_watched_keeper_with_its_back_to_the_approach_is_refused() {
    // The party member who talks may be of another class, so the stand is
    // judged by its own yaw.
    let (code, message) = build(&campaign(class_watch()), "east").expect_err("refused");
    assert_eq!(code, "DW1025", "{message}");
    for needle in [
        "`npc/keeper`",
        "`obj/talk`",
        "the critical path",
        "[4, 65, 2]",
        "117 degrees",
        "\"within\": 3",
    ] {
        assert!(message.contains(needle), "missing {needle}: {message}");
    }
}

#[test]
fn a_class_watched_keeper_facing_away_from_the_approach_is_refused() {
    let (code, message) = build(&campaign(class_watch()), "south").expect_err("refused");
    assert_eq!(code, "DW1025", "{message}");
    assert!(message.contains("153 degrees"), "{message}");
}

#[test]
fn a_nearest_watch_that_reaches_the_approach_answers_a_back_turned_stand() {
    assert_eq!(build(&campaign(watch(json!("nearest"), 3)), "east"), Ok(()));
}

#[test]
fn a_held_keeper_is_judged_from_every_cell_not_only_the_route() {
    // The pinned stand faces the route's approach (27 degrees off), so the
    // route's judgement passes it; declaring `none`, it is judged from every
    // cell a player can talk to it from, and the room is open behind it.
    let (code, message) = build(&campaign(held), "north").expect_err("refused");
    assert_eq!(code, "DW1025", "{message}");
    for needle in [
        "`npc/keeper` declares `watch: \"none\"`",
        "14 of the 39 cell(s)",
        "[5, 65, 5]",
        "No one facing faces every one of those cells",
        "within 8 blocks",
    ] {
        assert!(message.contains(needle), "missing {needle}: {message}");
    }
}

#[test]
fn a_held_keeper_with_a_wall_at_its_back_and_facing_the_room_builds() {
    assert_eq!(build(&campaign_at([0, 0, -3], held), "south"), Ok(()));
}

#[test]
fn a_held_keeper_facing_the_wall_is_told_the_facing_that_faces_the_room() {
    let (code, message) = build(&campaign_at([0, 0, -3], held), "north").expect_err("refused");
    assert_eq!(code, "DW1025", "{message}");
    assert!(
        message.contains("Declare `anchor/keeper-stand`'s facing `south`"),
        "{message}"
    );
    // The remedy names a command the binary has: the anchor verb is `prefab`'s.
    assert!(
        message.contains("`delvec prefab anchor <piece>.nbt --name anchor/keeper-stand"),
        "{message}"
    );
}

#[test]
fn a_cell_is_in_interaction_reach_by_the_eye_to_the_hitbox() {
    use delvec::compiler::approach::within_interaction_reach;
    let feet = [0.5, 64.0, 0.5];
    let villager_box = delvec::compiler::bodies::answer_box_dims("minecraft:villager");
    assert_eq!(villager_box, (1.0, 2.0));
    // Level, three blocks out: the eye is 2.5 from the hitbox's near face.
    assert!(within_interaction_reach(villager_box, feet, [3, 64, 0]));
    // Level, four blocks out: 3.5.
    assert!(!within_interaction_reach(villager_box, feet, [4, 64, 0]));
    // Two blocks out and three down: the eye (1.62 up) is 1.38 below the
    // hitbox's foot and 1.5 out, about 2.04 away.
    assert!(within_interaction_reach(villager_box, feet, [2, 61, 0]));
    // Two blocks out and five up: the eye is 4.62 above the hitbox's top.
    assert!(!within_interaction_reach(villager_box, feet, [2, 69, 0]));
}
