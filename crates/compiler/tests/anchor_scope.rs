//! **A name is not an identity across areas.**
//!
//! `plan.anchors` is keyed by `(area, name)` and the by-name lookups discarded
//! the area, returning the first entry whose NAME matched, across every area,
//! first match wins. The area was always in the key.
//!
//! The scope decision is not made here. The diagnostics catalog already states
//! it for the four gate verbs (`DW0857`): **the scope of uniqueness for an
//! anchor name is the AREA**, and that is the scope the DSL tier already
//! resolves every reference in. What was missing is the compiler honouring it,
//! which `DW0857`'s own row names — *an escape hatch would have to be the author
//! naming which area they meant, which is the area-scoped resolution this
//! diagnostic exists because the compiler does not have.*
//!
//! Measured on a campaign of eight zones: one NPC declared at `anchor/lampman`
//! in his home zone and cast at `anchor/lampman` in seven others, with **two**
//! zones providing that name. Every beat resolved to the home cell — the
//! escort's destination was the cell he already stood on — and the one zone with
//! its own station for him was never used.
//!
//! What is asserted here, and the pairing is the point: the beat's own area wins
//! when it provides the name (the repair), **and** a name exactly one area
//! provides still resolves from anywhere (the crossing, which is not what is
//! refused). A test with only the first half would pass on a compiler that had
//! simply stopped resolving across areas at all.

mod common;

use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::{Plan, ResolvedAnchor, Step};
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::parse_campaign;
use serde_json::json;

/// The name both buildings answer to in the perturbed fixture.
const NAME: &str = "anchor/keeper-stand";

/// A two-area campaign in which the beat and the body are in different areas.
///
/// Base is `talkto-cast-pos`, whose `quest/ask` already casts `npc/keeper` at
/// [`NAME`] — so the cast row is the fixture's, not this test's invention. Two
/// things are added: a second area, and the beat moved into it.
///
/// `annex_provides_the_name` is the whole perturbation. `prefab/hello-room`
/// declares [`NAME`]; `prefab/keep-room-small-a` does not. So the perturbed tree
/// has TWO areas providing it and the control has one — and nothing else about
/// the two trees differs.
fn fixture(tag: &str, annex_provides_the_name: bool) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("dw-anchor-scope-{tag}"));
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
    // The beat plays in the annex; `npc/keeper` still lives in `area/keep`.
    common::patch_file(&dir.join("quest-plan.json"), |p| {
        for q in p["content"]["quests"].as_array_mut().expect("quests[]") {
            if q["id"] == "quest/ask" {
                q["area"] = json!("area/annex");
            }
        }
    });
    dir
}

/// Build the fixture's plan and hand it to `f`. A closure because `Plan` borrows
/// the campaign it was planned from.
fn with_plan<T>(tag: &str, annex_provides: bool, f: impl FnOnce(&Plan) -> T) -> T {
    let dir = fixture(tag, annex_provides);
    let loaded = load_campaign_dir(&dir).expect("fixture campaign loads");
    let campaign = parse_campaign(&loaded.raw).expect("fixture campaign parses");
    let reg = PrefabRegistry::load_dir(&common::prefabs_dir()).expect("library loads");
    let plan = Plan::build(&campaign, &reg).expect("fixture plans");
    let out = f(&plan);
    let _ = std::fs::remove_dir_all(&dir);
    out
}

fn cell(r: &ResolvedAnchor) -> [i32; 3] {
    match r {
        ResolvedAnchor::Point { pos, .. } => *pos,
        ResolvedAnchor::Gate { from, .. } => *from,
    }
}

/// The strict `(area, name)` lookup, so expectations are read out of the table
/// rather than hard-coded.
fn at(plan: &Plan, area: &str) -> Option<[i32; 3]> {
    plan.anchors
        .get(&(area.to_string(), NAME.to_string()))
        .map(cell)
}

/// The cell the `talk-to` beat resolved the NPC to.
fn talk_pos(plan: &Plan) -> [i32; 3] {
    plan.critical_path
        .iter()
        .find_map(|s| match s {
            Step::TalkTo { pos, .. } => Some(*pos),
            _ => None,
        })
        .expect("the ask beat is a talk-to step")
}

/// **The repair.** Two areas provide the name. The beat is planned in
/// `area/annex`, so that is the building it means — not `area/keep`, which is
/// merely where the NPC was declared and merely what sorts first.
///
/// Asserted against the annex's own cell rather than against *not the keep cell*,
/// because a compiler that resolved to nothing would satisfy the weaker form.
#[test]
fn a_cast_beat_resolves_in_the_area_the_beat_plays_in() {
    with_plan("both", true, |plan| {
        let annex = at(plan, "area/annex").expect("the annex provides the name when perturbed");
        let keep = at(plan, "area/keep").expect("the keep declares it too");
        // Non-vacuity, in the test that depends on it: if the two areas answered
        // alike, the assertion below would pass on the unrepaired compiler.
        assert_ne!(
            annex, keep,
            "the fixture is only a test of scope if the two areas resolve differently"
        );
        assert_eq!(
            talk_pos(plan),
            annex,
            "the beat plays in `area/annex` and that area provides the name, so the body \
             stands there — not in the NPC's home area, and not in whichever id sorts first"
        );
    });
}

/// **The crossing, which is NOT what is refused.** Only `area/keep` provides the
/// name; the beat is planned in `area/annex`. One provider is an answer, so it
/// resolves from anywhere exactly as it always did — the half that keeps the
/// repair from being a widening, and the half a "stop crossing areas" change
/// would break.
#[test]
fn a_name_one_area_provides_still_resolves_from_another() {
    with_plan("one", false, |plan| {
        assert!(
            at(plan, "area/annex").is_none(),
            "the control must not carry the perturbation"
        );
        let keep = at(plan, "area/keep").expect("the keep provides the name");
        assert_eq!(
            talk_pos(plan),
            keep,
            "one provider is unambiguous, so the beat still reaches across the boundary"
        );
    });
}

/// **The authority states a binding count that is measured, not written down
/// beside the thing it counts.** `providers` is the question the by-name lookups
/// never asked; a constant here would be the vacuity the count exists to expose.
#[test]
fn the_provider_count_is_measured_not_asserted() {
    let two = with_plan("count-two", true, |plan| plan.anchors.providers(NAME).len());
    let one = with_plan("count-one", false, |plan| {
        plan.anchors.providers(NAME).len()
    });
    assert_eq!(
        (two, one),
        (2, 1),
        "the count must track the library: two providers under the perturbation, one \
         without it. Equal counts would mean the perturbation never bound"
    );
}
