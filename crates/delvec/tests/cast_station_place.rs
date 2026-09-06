//! **A body summoned at one place and a plan that sends the party to another.**
//!
//! `DW0461` exists to prove that *declaring an anchor does not teleport
//! anybody*: the ledger's `at` must equal where the effect history leaves the
//! NPC. It compared the two anchor **names**, and an anchor name is an identity
//! within an AREA and nowhere wider — so where two areas both declare
//! `anchor/keeper-stand`, the ledger row and the history were the same string,
//! the proof found them equal, and the compiler moved the body 256 blocks into
//! another building with no diagnostic at all.
//!
//! Measured on the perturbed fixture below, on the revision this test lands
//! against: the datapack summoned the body **and its interaction hitbox** at
//! `5.5 65.0 4.5` in `area/keep` while the critical path sent the party to
//! `[261, 65, 4]` in `area/annex`. The campaign compiled clean. The bot would
//! have walked to an empty cell with nobody to click.
//!
//! The pairing is the point, and both halves are asserted here: the perturbed
//! tree is **refused**, and the control tree — identical but for which prefab
//! `area/annex` binds — still **compiles and still crosses the boundary** to the
//! Keeper's one station. A test with only the first half would pass on a
//! compiler that had simply stopped letting a cast row reach another area.

mod common;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::{Plan, ResolvedAnchor, Step};
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::parse_campaign;
use serde_json::json;
use std::collections::BTreeMap;

/// The name both buildings answer to under the perturbation.
const NAME: &str = "anchor/keeper-stand";

/// A two-area campaign in which the beat plays in `area/annex` while
/// `npc/keeper` lives — and is declared, and is summoned — in `area/keep`.
///
/// **Nothing in this campaign ever moves him.** There is no `move-npc`
/// anywhere, which is what makes the relocation silent rather than declared.
///
/// `annex_provides_the_name` is the whole perturbation: `prefab/hello-room`
/// declares [`NAME`], `prefab/keep-room-small-a` does not. Nothing else about
/// the two trees differs.
fn fixture(tag: &str, annex_provides_the_name: bool) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("dw-cast-station-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    common::copy_dir_all(
        &common::compiler_fixtures_dir().join("talkto-cast-pos"),
        &dir,
    );
    let annex_piece = if annex_provides_the_name {
        "prefab/hello-room"
    } else {
        "prefab/keep-room-small-a"
    };
    common::patch_file(&dir.join("world.json"), |w| {
        let areas = w["content"]["areas"].as_array_mut().expect("areas[]");
        areas.push(json!({
            "id": "area/annex",
            "name": "The Annex",
            "prefab": annex_piece,
        }));
    });
    common::patch_file(&dir.join("quest-plan.json"), |p| {
        for q in p["content"]["quests"].as_array_mut().expect("quests[]") {
            if q["id"] == "quest/ask" {
                q["area"] = json!("area/annex");
            }
        }
    });
    dir
}

/// The same fixture, plus a beat in the area the party starts in.
///
/// Without it the campaign is refused as `DW0873` (the party's first leg is a
/// crossing nothing can carry) — a true refusal about a different subject, and
/// one that names the quest plan and `world.areas` while saying nothing about
/// the anchor name that decided where the beat stands. `quest/arrive` is not
/// part of the perturbation: both trees carry it identically, and its only job
/// is to make them campaigns a party could start, so that what differs between
/// them is still only which areas provide [`NAME`].
fn fixture_playable(tag: &str, annex_provides: bool) -> std::path::PathBuf {
    let dir = fixture(tag, annex_provides);
    common::patch_file(&dir.join("quests.json"), |q| {
        let quests = q["content"]["quests"].as_array_mut().expect("quests[]");
        for x in quests.iter_mut() {
            if x["id"] == "quest/ask" {
                x["trigger"] = json!({ "type": "quest-complete", "quest": "quest/arrive" });
            }
        }
        quests.insert(
            0,
            json!({
                "id": "quest/arrive",
                "trigger": { "type": "campaign-start" },
                "objectives": [
                    { "id": "obj/arrive", "type": "reach-anchor",
                      "anchor": NAME, "radius": 2 }
                ],
                "on_objective_complete": {},
                "on_complete": [],
            }),
        );
    });
    common::patch_file(&dir.join("quest-plan.json"), |p| {
        let quests = p["content"]["quests"].as_array_mut().expect("quests[]");
        for q in quests.iter_mut() {
            if q["id"] == "quest/ask" {
                q["depends_on"] = json!(["quest/arrive"]);
            }
        }
        quests.insert(
            0,
            json!({
                "id": "quest/arrive", "goal": "Cross the yard to the Keeper's door.",
                "area": "area/keep", "npcs": [], "depends_on": [],
                "mandatory": true, "act": 1,
            }),
        );
    });
    dir
}

fn cell(r: &ResolvedAnchor) -> [i32; 3] {
    match r {
        ResolvedAnchor::Point { pos, .. } => *pos,
        ResolvedAnchor::Gate { from, .. } => *from,
    }
}

/// **The refusal.** Two areas provide the name, the beat plays in one and the
/// body lives in the other, and nothing walks him across.
#[test]
fn a_shared_name_that_moves_a_body_is_dw0461() {
    let dir = fixture_playable("shared", true);
    let loaded = load_campaign_dir(&dir).expect("fixture loads");
    let campaign = parse_campaign(&loaded.raw).expect("fixture parses");
    let reg = PrefabRegistry::load_dir(&common::prefabs_dir()).expect("library loads");
    let err = Plan::build(&campaign, &reg)
        .err()
        .expect("a cast row that moves a body 256 blocks must be refused, not compiled");
    assert_eq!(
        err.failure.code, "DW0461",
        "the refusal must be the placement proof, not a generic build error: {err:#?}"
    );
    // The message has to carry BOTH places, because the whole finding is that
    // one name meant two of them.
    for expected in [
        "area/keep",
        "area/annex",
        NAME,
        "[5, 65, 4]",
        "[261, 65, 4]",
    ] {
        assert!(
            err.failure.message.contains(expected),
            "the message must name both buildings and both cells (missing `{expected}`): {}",
            err.failure.message
        );
    }
    // The remedy has to be one this author can perform: the binding in
    // `world.areas[]`, never an edit to a prefab library they do not own.
    assert!(
        err.failure.message.contains("`world.areas[]`"),
        "{}",
        err.failure.message
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// **The crossing, which is NOT what is refused.** One provider is an answer, so
/// the beat still reaches across the boundary to the Keeper's one station — and
/// the datapack summons him at that same cell. The half that keeps the repair
/// from being a widening.
#[test]
fn one_provider_still_crosses_and_the_two_authorities_agree() {
    let dir = fixture_playable("control", false);
    let loaded = load_campaign_dir(&dir).expect("fixture loads");
    let campaign = parse_campaign(&loaded.raw).expect("fixture parses");
    let reg = PrefabRegistry::load_dir(&common::prefabs_dir()).expect("library loads");
    let plan = Plan::build(&campaign, &reg).expect("one provider is unambiguous — this compiles");

    assert!(
        plan.anchors
            .get(&("area/annex".to_string(), NAME.to_string()))
            .is_none(),
        "the control must not carry the perturbation"
    );
    let keep = cell(
        plan.anchors
            .get(&("area/keep".to_string(), NAME.to_string()))
            .expect("the keep provides the name"),
    );
    let talk = plan
        .critical_path
        .iter()
        .find_map(|s| match s {
            Step::TalkTo { pos, .. } => Some(*pos),
            _ => None,
        })
        .expect("the ask beat is a talk-to step");
    assert_eq!(
        talk, keep,
        "one provider resolves from anywhere, so the beat still crosses to the Keeper"
    );

    // …and the shipped datapack puts a body there. This is the assertion the
    // defect broke: the summon and the plan are two authorities that must name
    // one cell, and comparing them is the only thing that would have caught it.
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file))
                    .expect("placed structure reads");
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    let out = delvewright_compiler::emit::build(
        &plan,
        &BTreeMap::new(),
        &structures,
        &CommandTree::v1_21_11(),
        &reg,
        None,
        &BTreeMap::new(),
    )
    .expect("the control campaign builds");
    let want = format!(
        "summon minecraft:villager {}.5 {}.0 {}.5 ",
        keep[0], keep[1], keep[2]
    );
    let summons: Vec<&str> = out
        .values()
        .flat_map(|b| {
            std::str::from_utf8(b)
                .unwrap_or("")
                .lines()
                .filter(|l| l.starts_with("summon minecraft:villager"))
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(
        summons.len(),
        1,
        "one NPC, one body — the binding count this assertion depends on: {summons:?}"
    );
    assert!(
        summons[0].starts_with(&want),
        "the datapack must summon the body at the very cell the plan sends the party to \
         (`{want}`): {}",
        summons[0]
    );
    let _ = std::fs::remove_dir_all(&dir);
}
