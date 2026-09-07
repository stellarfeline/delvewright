//! **`nav::move_target` asks the one authority for where a body stands.**
//!
//! `plan::body_station` is the one authority for where a body stands, asked
//! with the caller's own scope — `BodyScope::Declared` for the world-init
//! summon, `BodyScope::Beat` for the cast ledger's per-beat station. Before this,
//! `nav::move_target` answered the same question with a THIRD private rule: the
//! anchor in the NPC's own (home) area first, else the first area — in
//! `BTreeMap` order — that happened to provide the name, with no regard for
//! which quest's bundle actually fired the walk.
//!
//! That rule and `body_station`'s can disagree, and the shape is the one
//! already closed for the cast ledger: an anchor name **two areas answer to** — the
//! NPC's own home area, and the area the move's own quest plays in. Home always
//! won under the old rule, silently discarding what the story's own quest
//! placement said. `body_station`'s `BodyScope::Beat` reads the beat's area
//! FIRST, exactly as the cast ledger already does, so the fix is to route
//! `move_target` through it with that same scope.
//!
//! The fixture is `talkto-cast-pos` (`npc/keeper` declared in `area/keep`) plus
//! one added area, `area/annex`, bound to `prefab/keep-spawn-hall` — which
//! provides `anchor/exit` (the existing fixture's `move-npc` destination) but
//! NOT `anchor/keeper-stand` (the existing fixture's cast anchor). That keeps
//! the perturbation to exactly the fact under test: the cast row still resolves
//! in `area/keep` (matching the effect history, so `DW0461` stays silent) while
//! `anchor/exit` becomes a name two areas answer to.

mod common;

use std::collections::BTreeSet;

use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::nav::{self, World};
use delvewright_compiler::plan::{Plan, ResolvedAnchor};
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_compiler::timeline;
use delvewright_dsl::{Verb, parse_campaign};
use serde_json::json;

/// The destination `move-npc` already walks to in `talkto-cast-pos`'s
/// `quest/ask` (`on_objective_complete/obj/ask`), unmodified by this fixture.
const TO_ANCHOR: &str = "anchor/exit";

/// Build the perturbed fixture: `area/annex` added (bound to a prefab that
/// provides [`TO_ANCHOR`] but not the cast anchor), `quest/ask` moved into it
/// (behind a preceding `quest/arrive` so the party's own first move is still
/// makeable — a crossing rides on the completion of the objective the party
/// leaves from, spec-0008 addendum), everything else byte-for-byte the base
/// fixture.
///
/// `guard` additionally puts a `when` on the very `move-npc` under test, with
/// the `set-flag` that opens it one effect earlier in the same bundle. That is
/// the cross-feature case: one guard on the effect, a destination scoped to the
/// beat.
fn fixture(guard: bool) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(if guard {
        "dw-move-npc-beat-scope-guarded"
    } else {
        "dw-move-npc-beat-scope"
    });
    let _ = std::fs::remove_dir_all(&dir);
    common::copy_dir_all(
        &common::compiler_fixtures_dir().join("talkto-cast-pos"),
        &dir,
    );

    common::patch_file(&dir.join("world.json"), |w| {
        let areas = w["content"]["areas"].as_array_mut().expect("areas[]");
        areas.push(json!({
            "id": "area/annex",
            "name": "The Annex",
            "prefab": "prefab/keep-spawn-hall",
        }));
    });
    // The party's own first move: `quest/arrive` gives it something to do in
    // `area/keep` (where the campaign starts) before `quest/ask`'s beat crosses
    // into `area/annex` — otherwise the crossing has no leaving objective and is
    // `DW0873`. Not part of the perturbation: it says nothing about anchor scope.
    common::patch_file(&dir.join("quests.json"), |q| {
        let quests = q["content"]["quests"].as_array_mut().expect("quests[]");
        for x in quests.iter_mut() {
            if x["id"] == "quest/ask" {
                x["trigger"] = json!({ "type": "quest-complete", "quest": "quest/arrive" });
                if guard {
                    let effs = x["on_objective_complete"]["obj/ask"]
                        .as_array_mut()
                        .expect("quest/ask's obj/ask bundle");
                    let mv = effs
                        .iter_mut()
                        .find(|e| e["type"] == "move-npc")
                        .expect("the bundle's move-npc");
                    mv["when"] = json!({ "requires_flags": ["flag/asked"] });
                    effs.insert(0, json!({ "type": "set-flag", "flag": "flag/asked" }));
                }
            }
        }
        quests.insert(
            0,
            json!({
                "id": "quest/arrive",
                "trigger": { "type": "campaign-start" },
                "objectives": [
                    { "id": "obj/arrive", "type": "reach-anchor",
                      "anchor": "anchor/keeper-stand", "radius": 2 }
                ],
                "on_objective_complete": {},
                "on_complete": [],
            }),
        );
    });
    common::patch_file(&dir.join("quest-plan.json"), |p| {
        let quests = p["content"]["quests"].as_array_mut().expect("quests[]");
        for x in quests.iter_mut() {
            if x["id"] == "quest/ask" {
                // The perturbation: `quest/ask`'s own beat now plays in
                // `area/annex`, which provides `anchor/exit` too.
                x["area"] = json!("area/annex");
                x["depends_on"] = json!(["quest/arrive"]);
            } else if x["id"] == "quest/farewell" {
                // `quest/ask`'s `move-npc` really does leave `npc/keeper` in
                // `area/annex` now (continuity's own `here_area` model, which
                // this fixture does not touch, already reads it that way).
                // `quest/farewell` casts him at that same `anchor/exit`, so its
                // own beat has to follow him there too — otherwise the fixture
                // would trip `DW0461` on a stale cast row that has nothing to
                // do with the fact under test.
                x["area"] = json!("area/annex");
            }
        }
        quests.insert(
            0,
            json!({
                "id": "quest/arrive", "goal": "Take stock of the keep before asking.",
                "area": "area/keep", "npcs": [], "depends_on": [],
                "mandatory": true, "act": 1,
            }),
        );
    });
    dir
}

fn build_plan(
    guard: bool,
) -> (
    std::path::PathBuf,
    delvewright_dsl::Campaign,
    PrefabRegistry,
) {
    let dir = fixture(guard);
    let loaded = load_campaign_dir(&dir).expect("fixture campaign loads");
    let campaign = parse_campaign(&loaded.raw).expect("fixture campaign parses");
    let reg = PrefabRegistry::load_dir(&common::prefabs_dir()).expect("library loads");
    (dir, campaign, reg)
}

/// **The regression.** `anchor/exit` resolves to two different world positions
/// depending on which area answers it — the fixture is only a test of scope if
/// they genuinely differ. `move_target`'s new rule must pick the area
/// `quest/ask`'s own bundle plays in (`area/annex`), not the NPC's home
/// (`area/keep`), which is what the old by-name scan always returned.
///
/// `nav::plan_moves` is exercised directly against an EMPTY `World` (no solid
/// cells at all, so nothing is standable anywhere) — the first thing every
/// resolved destination hits is `DW_MOVE_UNROUTABLE`'s "no standable floor cell
/// near destination anchor" message, which names the resolved position. That
/// isolates the fact under test (which position `move_target` picked) from
/// pathfinding, which is a separate, already-covered concern, and from the
/// unrelated question of whether these two void-separated areas could ever be
/// connected by a real walk (`DW0872`: a change of area is never one).
#[test]
fn move_npc_asks_the_beat_area_before_home() {
    let (dir, campaign, reg) = build_plan(false);
    let plan = Plan::build(&campaign, &reg).expect("fixture plans");

    let keep_pos = plan
        .anchors
        .get(&("area/keep".to_string(), TO_ANCHOR.to_string()))
        .expect("area/keep provides anchor/exit (unperturbed)");
    let annex_pos = plan
        .anchors
        .get(&("area/annex".to_string(), TO_ANCHOR.to_string()))
        .expect("area/annex provides anchor/exit too — the whole perturbation");
    let cell = |r: &ResolvedAnchor| match r {
        ResolvedAnchor::Point { pos, .. } => *pos,
        ResolvedAnchor::Gate { from, .. } => *from,
    };
    let (keep_pos, annex_pos) = (cell(keep_pos), cell(annex_pos));
    assert_ne!(
        keep_pos, annex_pos,
        "the fixture is only a test of scope if the two areas resolve `anchor/exit` \
         differently"
    );

    let world = World::from_solid_cells(BTreeSet::new());
    let err = nav::plan_moves(&plan, &world)
        .expect_err("an empty world makes every destination fail the standable-floor check");

    let annex_needle = format!("{annex_pos:?}");
    let keep_needle = format!("{keep_pos:?}");
    assert!(
        err.message.contains(&annex_needle),
        "the destination must resolve in `area/annex` — the area `quest/ask`'s own bundle \
         plays in — not silently fall back to the npc's home area: {}",
        err.message
    );
    assert!(
        !err.message.contains(&keep_needle),
        "home must not win over the beat when the beat itself provides the name: {}",
        err.message
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// **Every existing nav test stays green.** The pre-existing, unperturbed
/// `talkto-cast-pos` fixture (`quest/ask` still plays in `area/keep`, matching
/// `npc/keeper`'s own home) resolves `anchor/exit` to the SAME position under
/// the new rule as it always did: with no distinct beat, `BodyScope::Beat`'s
/// `beat` and `home` are the same area, so this is the ordinary, unperturbed
/// case the old rule got right too.
#[test]
fn an_unperturbed_move_still_resolves_in_the_shared_home_and_beat_area() {
    let dir = common::compiler_fixtures_dir().join("talkto-cast-pos");
    let loaded = load_campaign_dir(&dir).expect("base fixture loads");
    let campaign = parse_campaign(&loaded.raw).expect("base fixture parses");
    let reg = PrefabRegistry::load_dir(&common::prefabs_dir()).expect("library loads");
    let plan = Plan::build(&campaign, &reg).expect("base fixture plans");

    let keep_pos = plan
        .anchors
        .get(&("area/keep".to_string(), TO_ANCHOR.to_string()))
        .expect("area/keep provides anchor/exit");
    let keep_pos = match keep_pos {
        ResolvedAnchor::Point { pos, .. } => *pos,
        ResolvedAnchor::Gate { from, .. } => *from,
    };

    let world = World::from_solid_cells(BTreeSet::new());
    let err = nav::plan_moves(&plan, &world)
        .expect_err("an empty world makes every destination fail the standable-floor check");
    let needle = format!("{keep_pos:?}");
    assert!(
        err.message.contains(&needle),
        "the base fixture's own move must still resolve in `area/keep`: {}",
        err.message
    );
}

/// **The cross-feature pair.** `nav::plan_moves` is the one loop where this
/// branch's guard and `move_target`'s beat scope meet: it destructures
/// `Verb::MoveNpc` out of `eff.verb`, reads the branch condition off `eff.when`
/// (through [`nav::gate_key`], which is what names the walk driver), and asks
/// `move_target` for the destination with the beat the same walk yielded.
///
/// Either half can break while the other passes. A guard read from the wrong
/// place leaves the leg ungated — one driver for two branches, which is the
/// island's Eurylochus defect [`BranchGate`] exists for — and a destination
/// resolved by the npc's home rather than by the beat walks the body to the
/// wrong `anchor/exit`. So they are asserted together, on ONE effect.
///
/// The fixture is [`fixture`]'s perturbation plus a `when` on that very
/// `move-npc`. Nothing about a guard touches which area answers `anchor/exit`,
/// so the beat-scoped resolution is what must still happen.
#[test]
fn a_guarded_move_is_gated_and_still_resolves_in_its_beat_area() {
    let (dir, campaign, reg) = build_plan(true);
    let plan = Plan::build(&campaign, &reg).expect("the guarded fixture plans");

    let cell = |r: &ResolvedAnchor| match r {
        ResolvedAnchor::Point { pos, .. } => *pos,
        ResolvedAnchor::Gate { from, .. } => *from,
    };
    let annex_pos = cell(
        plan.anchors
            .get(&("area/annex".to_string(), TO_ANCHOR.to_string()))
            .expect("area/annex provides anchor/exit"),
    );
    let keep_pos = cell(
        plan.anchors
            .get(&("area/keep".to_string(), TO_ANCHOR.to_string()))
            .expect("area/keep provides anchor/exit"),
    );
    assert_ne!(keep_pos, annex_pos, "the perturbation must still bite");

    // Half one: the guard is a property of the EFFECT, and the walk driver reads
    // it there. Read out of `timeline::walk` — the enumeration `plan_moves`
    // itself walks — rather than by searching the campaign a second way.
    let moves: Vec<_> = timeline::walk(&plan)
        .into_iter()
        .map(|(e, _)| e)
        .filter(|e| matches!(e.verb, Verb::MoveNpc { .. }))
        .collect();
    assert_eq!(moves.len(), 1, "the fixture declares exactly one move-npc");
    assert_eq!(
        moves[0].requires_flags().len(),
        1,
        "the guard is read through the one accessor, off `when` and not off the verb"
    );
    assert!(
        !nav::gate_key(moves[0]).is_empty(),
        "a `when` on the effect must reach the walk driver's branch key — an empty \
         key is one driver for both branches"
    );

    // Half two: the destination is still the beat's area, not the npc's home.
    let world = World::from_solid_cells(BTreeSet::new());
    let err = nav::plan_moves(&plan, &world)
        .expect_err("an empty world makes every destination fail the standable-floor check");
    assert!(
        err.message.contains(&format!("{annex_pos:?}")),
        "a guarded move resolves in the beat's area exactly as an unguarded one does: {}",
        err.message
    );
    assert!(
        !err.message.contains(&format!("{keep_pos:?}")),
        "home must not win over the beat because the effect gained a guard: {}",
        err.message
    );

    let _ = std::fs::remove_dir_all(&dir);
}
