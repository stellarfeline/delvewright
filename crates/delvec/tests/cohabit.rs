//! `DW0896` — one mark, one body.
//!
//! The castle-tour muster, in its exact shape: a captain of the guard and six
//! men-at-arms, every one of them declaring `anchor/stop-gate`, built green and
//! emitted `summon minecraft:mannequin 77.5 68.0 40.5` seven times. What kept
//! them from being a pile of mannequins in one block was that each `move-actor`
//! walked off the mark at a different speed.
//!
//! What these pin is the QUANTIFIER, in both directions. The rule is
//! co-existence, not a shared anchor: an actor and an npc taking turns on one
//! mark is the handoff the engine supports and must stay buildable, and a body
//! the compiler cannot prove is still standing must withhold the error rather
//! than invent one.

mod common;

use std::collections::BTreeMap;

use delvec::compiler::cohabit::{DW_ONE_MARK_TWO_BODIES, check_one_body_per_mark};
use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildFailure};
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};
use serde_json::{Value, json};

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

/// A second stage-2 npc, on `anchor`. `deferred` decides whether it stands there
/// from world init or waits for a `spawn-npc`.
fn second_npc(anchor: &str, deferred: bool) -> Value {
    json!({
        "id": "npc/second",
        "name": "The Second",
        "role": "flavor",
        "area": "area/keep",
        "anchor": anchor,
        "base_entity": "minecraft:villager",
        "deferred": deferred,
        "persona": {
            "archetype": "silent double",
            "speech_style": "none",
            "motivation": "stand where the keeper stands",
        },
    })
}

/// An actor on `anchor`. Nothing here is `vulnerable`, so whether its lifetime is
/// bounded is decided entirely by the effects the fixture writes.
fn actor(id: &str, anchor: &str) -> Value {
    json!({ "id": id, "entity": "minecraft:villager", "anchor": anchor })
}

/// A `happening`, which every staging beat owes (`DW0481`).
fn happening(subject: &str, verb: &str) -> Value {
    json!({ "subject": subject, "verb": verb, "text": "a fixture beat" })
}

/// The hello-world campaign with `npcs[]` and `quests[]` rewritten by `edit`.
fn campaign_with(edit: impl FnOnce(&mut Value, &mut Value)) -> Campaign {
    let mut npcs: Value = serde_json::from_str(&read_hw("npcs.json")).unwrap();
    let mut quests: Value = serde_json::from_str(&read_hw("quests.json")).unwrap();
    edit(&mut npcs, &mut quests);
    let raw = RawCampaign {
        world: read_hw("world.json"),
        npcs: npcs.to_string(),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: quests.to_string(),
        dialogue: read_hw("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    };
    parse_campaign(&raw).expect("the fixture campaign parses")
}

/// Ask `DW0896` about a campaign, through the plan alone — no world, no
/// emission, because the rule is arithmetic over resolved cells.
fn verdict(c: &Campaign) -> (delvec::compiler::cohabit::CohabitBinding, Option<String>) {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(c, &prefabs).expect("plan builds");
    let (binding, r) = check_one_body_per_mark(&plan);
    let msg = match r {
        Ok(()) => None,
        Err(f) => {
            assert_eq!(f.code, DW_ONE_MARK_TWO_BODIES, "{}", f.message);
            Some(f.message)
        }
    };
    (binding, msg)
}

/// The `sequence` the muster is: `first` summoned at tick 0, `second` at tick 30,
/// with nothing between them that removes anybody.
fn muster_sequence(first: &str, second: &str) -> Value {
    json!({
        "type": "sequence",
        "steps": [
            { "at_ticks": 0, "effects": [
                { "type": "spawn-actor", "actor": first, "happening": happening(first, "arrives") } ] },
            { "at_ticks": 30, "effects": [
                { "type": "spawn-actor", "actor": second, "happening": happening(second, "arrives") } ] },
        ],
    })
}

// ---------------------------------------------------------------------------
// The world-init arm
// ---------------------------------------------------------------------------

/// Two bodies that stand on their mark from world init, on one cell. No timeline
/// is consulted and none is needed: both are summoned at tick 0, unconditionally
/// and at once.
#[test]
fn two_world_init_bodies_on_one_cell_are_dw0896() {
    let c = campaign_with(|npcs, _| {
        npcs["content"]["npcs"]
            .as_array_mut()
            .unwrap()
            .push(second_npc("anchor/keeper-stand", false));
    });
    let (b, msg) = verdict(&c);
    let msg = msg.expect("two bodies standing on one cell from world init is refused");
    assert!(
        msg.contains("npc/keeper") && msg.contains("npc/second"),
        "the message must name both bodies: {msg}"
    );
    assert!(
        msg.contains("anchor/keeper-stand"),
        "the message must name the mark: {msg}"
    );
    assert!(
        msg.contains("world cell ["),
        "the message must name the cell they share: {msg}"
    );
    assert_eq!(b.refused, 1, "one pair, one finding");
    assert_eq!(b.at_init, 2);
}

/// …and the same two bodies on two marks are not. The rule is the cell, and the
/// no-false-positive guard for the most ordinary staging in the DSL.
#[test]
fn two_world_init_bodies_on_two_cells_are_fine() {
    let c = campaign_with(|npcs, _| {
        npcs["content"]["npcs"]
            .as_array_mut()
            .unwrap()
            .push(second_npc("anchor/exit", false));
    });
    let (b, msg) = verdict(&c);
    assert!(msg.is_none(), "two marks, two bodies: {msg:?}");
    assert_eq!(b.refused, 0);
    assert!(
        b.pairs > 0,
        "the pass must be a pass over a comparison actually made, not a walk that \
         examined nothing: {b:?}"
    );
}

// ---------------------------------------------------------------------------
// The handoff — the case a rule keyed to a shared anchor would refuse
// ---------------------------------------------------------------------------

/// An actor and a `deferred` npc on ONE mark, taking turns: the actor is
/// summoned, walked off and removed, and only then does the npc take the cell.
/// This is the gallery's `anchor/muster`, and it must stay buildable.
#[test]
fn a_handoff_on_one_mark_is_not_refused() {
    let c = campaign_with(|npcs, quests| {
        npcs["content"]["npcs"]
            .as_array_mut()
            .unwrap()
            .push(second_npc("anchor/exit", true));
        quests["content"]["actors"] = json!([actor("actor/usher", "anchor/exit")]);
        quests["content"]["quests"][0]["on_objective_complete"]["obj/talk"]
            .as_array_mut()
            .unwrap()
            .extend([
                json!({ "type": "spawn-actor", "actor": "actor/usher",
                        "happening": happening("actor/usher", "arrives") }),
                json!({ "type": "despawn-actor", "actor": "actor/usher", "style": "vanish",
                        "happening": happening("actor/usher", "departs") }),
                json!({ "type": "spawn-npc", "npc": "npc/second",
                        "happening": happening("npc/second", "arrives") }),
            ]);
    });
    let (b, msg) = verdict(&c);
    assert!(
        msg.is_none(),
        "two bodies taking turns on one mark is the supported handoff: {msg:?}"
    );
    assert_eq!(b.refused, 0);
    assert_eq!(
        b.cells, 2,
        "three bodies, two marks: the handoff pair shares one"
    );
    assert!(b.entries >= 2, "both entries were judged: {b:?}");
}

// ---------------------------------------------------------------------------
// The muster arm — a body still standing when another is summoned onto its mark
// ---------------------------------------------------------------------------

/// The castle muster. Two actors on one mark, summoned from two steps of one
/// `sequence`; nothing removes the first, so it is provably still standing at
/// tick 30 when the second is summoned onto its coordinate.
#[test]
fn a_body_summoned_onto_a_standing_body_is_dw0896() {
    let c = campaign_with(|_, quests| {
        quests["content"]["actors"] = json!([
            actor("actor/captain", "anchor/exit"),
            actor("actor/man-at-arms", "anchor/exit"),
        ]);
        quests["content"]["quests"][0]["on_objective_complete"]["obj/talk"]
            .as_array_mut()
            .unwrap()
            .push(muster_sequence("actor/captain", "actor/man-at-arms"));
    });
    let (b, msg) = verdict(&c);
    let msg = msg.expect("a body summoned onto a standing body's mark is refused");
    assert!(
        msg.contains("actor/captain") && msg.contains("actor/man-at-arms"),
        "the message must name both bodies: {msg}"
    );
    assert_eq!(b.refused, 1);
    assert_eq!(
        b.permanent, 3,
        "nothing in this fixture removes any of the three bodies"
    );
}

/// …and the same pair on two marks is not refused, over the same timeline. This
/// is the pass that proves the arm is doing arithmetic on cells rather than
/// counting spawns.
#[test]
fn two_bodies_summoned_onto_two_marks_are_fine() {
    let c = campaign_with(|_, quests| {
        quests["content"]["actors"] = json!([
            actor("actor/captain", "anchor/exit"),
            actor("actor/man-at-arms", "spawn"),
        ]);
        quests["content"]["quests"][0]["on_objective_complete"]["obj/talk"]
            .as_array_mut()
            .unwrap()
            .push(muster_sequence("actor/captain", "actor/man-at-arms"));
    });
    let (b, msg) = verdict(&c);
    assert!(msg.is_none(), "two marks: {msg:?}");
    assert_eq!(b.refused, 0);
    assert!(
        b.entries == 2 && b.pairs > 0,
        "both spawns were judged, against a live body: {b:?}"
    );
}

/// The deliberate withholding. The first body carries a `despawn-actor`
/// somewhere in the campaign, so the compiler cannot prove it is still standing
/// when the second is summoned — and says nothing rather than guessing. This is
/// why the muster's six men-at-arms are caught against the captain and not
/// against each other.
#[test]
fn a_removable_body_withholds_the_error() {
    let c = campaign_with(|_, quests| {
        quests["content"]["actors"] = json!([
            actor("actor/captain", "anchor/exit"),
            actor("actor/man-at-arms", "anchor/exit"),
        ]);
        let mut seq = muster_sequence("actor/captain", "actor/man-at-arms");
        // The captain leaves — somewhere the compiler cannot order against the
        // second spawn, which is exactly the point.
        seq["steps"][1]["effects"].as_array_mut().unwrap().insert(
            0,
            json!({ "type": "despawn-actor", "actor": "actor/captain", "style": "vanish",
                    "happening": happening("actor/captain", "departs") }),
        );
        quests["content"]["quests"][0]["on_objective_complete"]["obj/talk"]
            .as_array_mut()
            .unwrap()
            .push(seq);
    });
    let (b, msg) = verdict(&c);
    assert!(
        msg.is_none(),
        "a body the campaign can remove is not provably standing: {msg:?}"
    );
    assert_eq!(
        b.permanent, 2,
        "the captain is removable; the keeper and the man-at-arms are not"
    );
}

/// A CONDITIONAL entry proves nothing about what it leaves standing — it may
/// never fire — so the body it summons never enters the live set.
#[test]
fn a_conditional_entry_does_not_prove_a_body_is_standing() {
    let c = campaign_with(|_, quests| {
        quests["content"]["actors"] = json!([
            actor("actor/captain", "anchor/exit"),
            actor("actor/man-at-arms", "anchor/exit"),
        ]);
        let mut seq = muster_sequence("actor/captain", "actor/man-at-arms");
        seq["steps"][0]["effects"][0]["when"] = json!({ "requires_flags": ["flag/mustered"] });
        quests["content"]["quests"][0]["on_objective_complete"]["obj/talk"]
            .as_array_mut()
            .unwrap()
            .extend([json!({ "type": "set-flag", "flag": "flag/mustered" }), seq]);
    });
    let (_, msg) = verdict(&c);
    assert!(
        msg.is_none(),
        "an entry that may not fire proves nothing about what is standing: {msg:?}"
    );
}

/// The other direction of that asymmetry: an entry is JUDGED whether or not its
/// own firing is conditional, because it collides on every run where it fires.
#[test]
fn a_conditional_entry_onto_a_standing_body_is_still_dw0896() {
    let c = campaign_with(|_, quests| {
        quests["content"]["actors"] = json!([
            actor("actor/captain", "anchor/exit"),
            actor("actor/man-at-arms", "anchor/exit"),
        ]);
        let mut seq = muster_sequence("actor/captain", "actor/man-at-arms");
        seq["steps"][1]["effects"][0]["when"] = json!({ "requires_flags": ["flag/mustered"] });
        quests["content"]["quests"][0]["on_objective_complete"]["obj/talk"]
            .as_array_mut()
            .unwrap()
            .extend([json!({ "type": "set-flag", "flag": "flag/mustered" }), seq]);
    });
    let (_, msg) = verdict(&c);
    assert!(
        msg.is_some(),
        "a conditional summon onto an occupied mark collides whenever it fires"
    );
}

/// A body that walks off its mark is still the body that mark belongs to. The
/// verdict may not depend on `move-actor` speeds — which is the whole of what
/// the muster was relying on.
#[test]
fn walking_the_first_body_off_its_mark_does_not_clear_it() {
    let c = campaign_with(|_, quests| {
        quests["content"]["actors"] = json!([
            actor("actor/captain", "anchor/exit"),
            actor("actor/man-at-arms", "anchor/exit"),
        ]);
        let mut seq = muster_sequence("actor/captain", "actor/man-at-arms");
        seq["steps"][0]["effects"].as_array_mut().unwrap().push(
            json!({ "type": "move-actor", "actor": "actor/captain", "to_anchor": "anchor/exit",
                    "speed": 0.24, "happening": happening("actor/captain", "departs") }),
        );
        quests["content"]["quests"][0]["on_objective_complete"]["obj/talk"]
            .as_array_mut()
            .unwrap()
            .push(seq);
    });
    let (_, msg) = verdict(&c);
    assert!(
        msg.is_some(),
        "a walk is not a removal: the mark still belongs to a living body"
    );
}

// ---------------------------------------------------------------------------
// Bound to the build
// ---------------------------------------------------------------------------

/// The gate is bound to the event it guards: an ordinary `delvec build` refuses,
/// with this code, and does not merely own a function nothing calls.
#[test]
fn the_build_itself_refuses_and_names_dw0896() {
    let c = campaign_with(|npcs, _| {
        npcs["content"]["npcs"]
            .as_array_mut()
            .unwrap()
            .push(second_npc("anchor/keeper-stand", false));
    });
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    let err = emit::build_with_warnings(
        &plan,
        &BTreeMap::new(),
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
    .expect_err("the build refuses two bodies on one mark");
    let BuildFailure::Diagnostic { code, message } = err else {
        panic!("expected a coded build diagnostic");
    };
    assert_eq!(code, DW_ONE_MARK_TWO_BODIES, "{message}");
}

/// The stock hello-world says nothing, and says so with a count: one body, one
/// cell, nothing to compare. A binding line that only appears when something is
/// wrong cannot be told from one that never ran.
#[test]
fn the_binding_line_states_its_denominator_on_a_campaign_with_nothing_to_find() {
    let c = campaign_with(|_, _| {});
    let (b, msg) = verdict(&c);
    assert!(msg.is_none());
    assert_eq!(b.bodies, 1);
    assert_eq!(b.placed, 1);
    assert_eq!(b.refused, 0);
    let line = b.line();
    assert!(
        line.contains("1 body(ies) declared") && line.contains("0 refused (DW0896)"),
        "{line}"
    );
}
