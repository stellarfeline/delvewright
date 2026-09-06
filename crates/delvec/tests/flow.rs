//! The branch-coherent flow model (`compiler::flow`) and its diagnostics.
//!
//! Pins the three soundness defects the model fixes, in both directions:
//!
//! * **false green** — a flag gated on itself (or on a flag only a mutually
//!   exclusive branch produces), and a `set-flag` nested in an `on_respawn` /
//!   `on_caught` reaction bundle, are no longer unconditional producers;
//! * **false red** — flags set from a dialogue option, an environment trigger,
//!   or a trap `disarm` ARE producers, so legitimate content stops dying as
//!   spurious `DW0203`;
//! * **incoherent export** — the critical path is one branch's playthrough and
//!   must replay legally step by step (`DW0204`).
//!
//! The `branch-endings` fixture is a faithful reduction of the island bug: one
//! dialogue node offers `flag/wait` / `flag/flee`, each flag opens its own
//! ending, the finale's stage-4 `depends_on` names both, and the completion
//! bundle re-sets each flag gated on itself (the island's idiom — the exact
//! shape that made the old union fixpoint call both branches producible).

mod common;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use delvewright_compiler::analyze::analyze_campaign;
use delvewright_compiler::flow::{Flow, PathStep, Playthrough};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, parse_campaign};

fn branch_endings_dir() -> PathBuf {
    common::compiler_fixtures_dir().join("branch-endings")
}

fn parse(dir: &Path) -> Campaign {
    let loaded = load_campaign_dir(dir).expect("fixture loads");
    parse_campaign(&loaded.raw).expect("fixture parses")
}

/// Materialize `branch-endings` with `quests.json` replaced by `patch` (a raw
/// stage-5 envelope) and parse it. Structural parse only — these tests exercise
/// the flow model, not anchor resolution.
fn variant(name: &str, quests: serde_json::Value) -> Campaign {
    let dst = std::env::temp_dir().join(format!("dw-flow-{name}"));
    let _ = std::fs::remove_dir_all(&dst);
    common::materialize_from(
        &branch_endings_dir(),
        &serde_json::json!({ "documents": { "quests": quests } }),
        &dst,
    );
    parse(&dst)
}

/// Materialize `branch-endings` with an arbitrary set of stage documents
/// replaced, and parse it (structural parse only, as above).
fn variant_docs(name: &str, docs: serde_json::Value) -> Campaign {
    let dst = std::env::temp_dir().join(format!("dw-flow-{name}"));
    let _ = std::fs::remove_dir_all(&dst);
    common::materialize_from(
        &branch_endings_dir(),
        &serde_json::json!({ "documents": docs }),
        &dst,
    );
    parse(&dst)
}

/// One of the fixture's stage documents, as a mutable JSON value.
fn stage_json(stage: &str) -> serde_json::Value {
    serde_json::from_str(
        &std::fs::read_to_string(branch_endings_dir().join(format!("{stage}.json"))).unwrap(),
    )
    .unwrap()
}

/// The fixture's stage-5 document, as a mutable JSON value.
fn quests_json() -> serde_json::Value {
    stage_json("quests")
}

fn codes(c: &Campaign) -> Vec<String> {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    analyze_campaign(c, &prefabs)
        .into_iter()
        .map(|d| d.code)
        .collect()
}

// ---------------------------------------------------------------------------
// branch coherence
// ---------------------------------------------------------------------------

/// The reduction of the island bug analyzes clean: both endings are legitimate,
/// each on its own branch. (Under the old union fixpoint this same fixture is
/// clean too — but only because a self-gated `set-flag` was mistaken for an
/// unconditional producer; see `xor_branches_are_mutually_exclusive`.)
#[test]
fn branch_endings_analyzes_clean() {
    let c = parse(&branch_endings_dir());
    assert!(codes(&c).is_empty(), "{:?}", codes(&c));
}

/// The exported path is ONE branch: the `flag/wait` ending. The abandoned
/// `flag/flee` ending's objective is not on it, and the `talk-to` takes the
/// option that sets `flag/wait` — not the first completing option in the tree.
#[test]
fn exported_path_is_a_single_branch() {
    let c = parse(&branch_endings_dir());
    let flow = Flow::new(&c);
    let p = flow.playthrough();
    assert!(!p.degenerate);
    let objs: Vec<&str> = p.steps.iter().map(|s| s.objective.as_str()).collect();
    assert_eq!(objs, ["obj/decide", "obj/watch", "obj/walk-out"]);
    assert!(
        !p.quests.iter().any(|q| q == "quest/bolt"),
        "the abandoned ending's quest must not be on the path: {:?}",
        p.quests
    );
    // Option 2 is "We hold" (sets flag/wait); option 3 is "I run" (flag/flee).
    assert_eq!(p.steps[0].talk_option, Some(2));
    flow.replay(&p).expect("the exported path must replay");
}

/// **The pre-fix path fails the new check.** Rebuild exactly what the old
/// extractor produced — the finale's whole `depends_on` closure (both endings)
/// with the first completing dialogue option — and replay it: the first
/// incoherent step is the abandoned ending's objective, whose gating flag the
/// chosen branch never sets.
#[test]
fn pre_fix_mixed_branch_path_fails_replay() {
    let c = parse(&branch_endings_dir());
    let flow = Flow::new(&c);
    let pre = Playthrough {
        quests: vec![
            "quest/decide".to_string(),
            "quest/bolt".to_string(),
            "quest/hold".to_string(),
        ],
        steps: vec![
            PathStep {
                quest: "quest/decide".to_string(),
                objective: "obj/decide".to_string(),
                talk_option: Some(2), // first completing option = the `flag/wait` one
            },
            PathStep {
                quest: "quest/bolt".to_string(),
                objective: "obj/bolt".to_string(),
                talk_option: None,
            },
            PathStep {
                quest: "quest/hold".to_string(),
                objective: "obj/watch".to_string(),
                talk_option: Some(6),
            },
            PathStep {
                quest: "quest/hold".to_string(),
                objective: "obj/walk-out".to_string(),
                talk_option: None,
            },
        ],
        cyclic: false,
        degenerate: false,
    };
    let err = flow
        .replay(&pre)
        .expect_err("the mixed-branch path must not replay");
    assert_eq!(err.position, 2);
    assert_eq!(err.objective, "obj/bolt");
    assert!(
        err.reason.contains("flag/flee"),
        "the failure must name the branch flag: {}",
        err.reason
    );
}

/// A path that keeps both endings *satisfiable* (no branch flags at all) still
/// fails: the first ending's `campaign-complete` fires mid-path. `DW0204`.
#[test]
fn ungated_double_ending_is_dw0204() {
    let mut q = quests_json();
    // Drop every flag gate: both endings become completable in one world, so the
    // whole closure lands on one path — and the abandoned ending ends the delve
    // three steps early.
    strip_flag_gates(&mut q);
    let c = variant("double-ending", q);
    assert!(
        codes(&c).contains(&"DW0204".to_string()),
        "expected DW0204, got {:?}",
        codes(&c)
    );
}

/// The negative-gate half of the compensating check: the fixpoint deliberately
/// ignores `forbids_flags` for producibility, so only the ordered replay can see
/// that a step forbids a flag an earlier step on the same path already set.
/// `DW0204`.
#[test]
fn forbidden_flag_set_earlier_on_the_path_is_dw0204() {
    let mut q = quests_json();
    for quest in q["content"]["quests"].as_array_mut().unwrap() {
        if quest["id"] == "quest/hold" {
            for obj in quest["objectives"].as_array_mut().unwrap() {
                if obj["id"] == "obj/walk-out" {
                    obj["forbids_flags"] = serde_json::json!(["flag/wait"]);
                }
            }
        }
    }
    let c = variant("forbids-late", q);
    let cs = codes(&c);
    assert!(
        cs.contains(&"DW0204".to_string()),
        "expected DW0204: {cs:?}"
    );
}

// ---------------------------------------------------------------------------
// producer model
// ---------------------------------------------------------------------------

/// XOR: two options of one node that set conflicting flags cannot both hold, so
/// an objective requiring BOTH is completable in no world — `DW0203`. The old
/// union fixpoint called it reachable.
#[test]
fn xor_branches_are_mutually_exclusive() {
    let mut q = quests_json();
    for quest in q["content"]["quests"].as_array_mut().unwrap() {
        if quest["id"] == "quest/hold" {
            for obj in quest["objectives"].as_array_mut().unwrap() {
                if obj["id"] == "obj/watch" {
                    obj["requires_flags"] = serde_json::json!(["flag/wait", "flag/flee"]);
                }
            }
        }
    }
    let c = variant("xor", q);
    let flow = Flow::new(&c);
    assert!(
        !flow.any_completable().contains("obj/watch"),
        "an objective needing both sides of one dialogue branch is unreachable"
    );
    assert!(codes(&c).contains(&"DW0203".to_string()));
}

/// A `set-flag` gated on a flag only the *other* branch produces is not a
/// producer on this branch: nothing can satisfy an objective that reads it.
#[test]
fn cross_branch_gated_producer_is_not_unconditional() {
    let mut q = quests_json();
    // quest/decide's bundle sets flag/ghost, but only while flag/flee is set.
    for quest in q["content"]["quests"].as_array_mut().unwrap() {
        if quest["id"] == "quest/decide" {
            quest["on_objective_complete"]["obj/decide"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({
                    "type": "set-flag",
                    "flag": "flag/ghost",
                    "requires_flags": ["flag/flee"]
                }));
        }
        // ...and the wait branch's finale objective reads it.
        if quest["id"] == "quest/hold" {
            for obj in quest["objectives"].as_array_mut().unwrap() {
                if obj["id"] == "obj/walk-out" {
                    obj["requires_flags"] = serde_json::json!(["flag/wait", "flag/ghost"]);
                }
            }
        }
    }
    let c = variant("cross-gate", q);
    let flow = Flow::new(&c);
    assert!(!flow.any_completable().contains("obj/walk-out"));
    assert!(codes(&c).contains(&"DW0203".to_string()));
}

/// A `set-flag` nested in a `set-checkpoint`'s `on_respawn` reaction bundle
/// fires at a statically unknowable time, so it is NOT a producer — the
/// conservative stance `continuity.rs` already takes.
#[test]
fn reaction_bundle_set_flag_is_not_a_producer() {
    let mut q = quests_json();
    for quest in q["content"]["quests"].as_array_mut().unwrap() {
        if quest["id"] == "quest/decide" {
            quest["on_objective_complete"]["obj/decide"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({
                    "type": "set-checkpoint",
                    "anchor": "anchor/keeper-stand",
                    "on_respawn": [{ "type": "set-flag", "flag": "flag/revived" }]
                }));
        }
        if quest["id"] == "quest/hold" {
            for obj in quest["objectives"].as_array_mut().unwrap() {
                if obj["id"] == "obj/walk-out" {
                    obj["requires_flags"] = serde_json::json!(["flag/wait", "flag/revived"]);
                }
            }
        }
    }
    let c = variant("reaction", q);
    let flow = Flow::new(&c);
    assert!(
        !flow.any_completable().contains("obj/walk-out"),
        "an on_respawn set-flag must not count as a producer"
    );
    assert!(codes(&c).contains(&"DW0203".to_string()));
}

/// Dialogue reachability honors the gates on **intermediate** options, not just
/// on the completing one. `obj/watch`'s completing option is ungated (`DW0191`
/// requires that), but it sits behind a node whose only entrance requires
/// `flag/wait` — so with nothing left that can produce `flag/wait` (only the
/// self-gated re-affirm effect, which produces nothing), the objective is
/// unreachable. The old walk ignored intermediate gates and called it reachable.
#[test]
fn gated_intermediate_option_blocks_dialogue_reachability() {
    let mut dlg = stage_json("dialogue");
    // Drop the option that sets flag/wait; the flee option and the flag/wait-gated
    // door into `dlg/watch` both stay.
    let nodes = dlg["content"]["dialogues"][0]["nodes"]
        .as_array_mut()
        .unwrap();
    let opts = nodes[0]["options"].as_array_mut().unwrap();
    opts.retain(|o| {
        !o["effects"]
            .as_array()
            .is_some_and(|es| es.iter().any(|e| e["flag"] == "flag/wait"))
    });
    let mut q = quests_json();
    // The remaining flee option must still complete obj/decide, and obj/watch must
    // stop demanding flag/wait so the ONLY thing left blocking it is the gated
    // intermediate option.
    for quest in q["content"]["quests"].as_array_mut().unwrap() {
        for obj in quest["objectives"].as_array_mut().unwrap() {
            if obj["id"] == "obj/watch" || obj["id"] == "obj/walk-out" {
                obj.as_object_mut().unwrap().remove("requires_flags");
            }
        }
    }
    let c = variant_docs(
        "gated-intermediate",
        serde_json::json!({ "dialogue": dlg, "quests": q }),
    );
    let flow = Flow::new(&c);
    assert!(
        !flow.any_completable().contains("obj/watch"),
        "the completing option sits behind a flag-gated intermediate option"
    );
    assert!(codes(&c).contains(&"DW0203".to_string()), "{:?}", codes(&c));
}

/// A dialogue option's `set-flag` IS a producer. This is the false-red the old
/// model produced on the whole `branch-endings` fixture — `flag/wait` has no
/// quest-side producer at all, only the dialogue option.
#[test]
fn dialogue_option_set_flag_is_a_producer() {
    let c = parse(&branch_endings_dir());
    let flow = Flow::new(&c);
    for oid in ["obj/watch", "obj/walk-out", "obj/bolt"] {
        assert!(
            flow.any_completable().contains(oid),
            "{oid} is gated on a dialogue-set flag and must be reachable"
        );
    }
}

/// An environment trigger's `set-flag` IS a producer (a `strike`/`use`/
/// `approach` trigger is player-initiated — ambient, no DAG position).
#[test]
fn trigger_set_flag_is_a_producer() {
    let mut q = quests_json();
    q["content"]["triggers"] = serde_json::json!([{
        "id": "trigger/pull-lever",
        "at": "anchor/exit",
        "on": { "on": "use" },
        "effects": [{ "type": "set-flag", "flag": "flag/lever" }]
    }]);
    for quest in q["content"]["quests"].as_array_mut().unwrap() {
        if quest["id"] == "quest/hold" {
            for obj in quest["objectives"].as_array_mut().unwrap() {
                if obj["id"] == "obj/walk-out" {
                    obj["requires_flags"] = serde_json::json!(["flag/wait", "flag/lever"]);
                }
            }
        }
    }
    let c = variant("trigger", q);
    let flow = Flow::new(&c);
    assert!(flow.any_completable().contains("obj/walk-out"));
    assert!(codes(&c).is_empty(), "{:?}", codes(&c));
}

/// A trap's `disarm.sets_flag` IS a producer (same ambient reasoning: the player
/// walks up and disarms it).
#[test]
fn trap_disarm_flag_is_a_producer() {
    let mut q = quests_json();
    q["content"]["traps"] = serde_json::json!([{
        "id": "trap/dart",
        "at": "anchor/trap",
        "trigger": "pressure-plate",
        "effect": { "dispense": { "item": "minecraft:arrow", "count": 8 } },
        "disarm": { "via": "anchor/keeper-stand", "sets_flag": "flag/disarmed" }
    }]);
    for quest in q["content"]["quests"].as_array_mut().unwrap() {
        if quest["id"] == "quest/hold" {
            for obj in quest["objectives"].as_array_mut().unwrap() {
                if obj["id"] == "obj/walk-out" {
                    obj["requires_flags"] = serde_json::json!(["flag/wait", "flag/disarmed"]);
                }
            }
        }
    }
    // Structural parse only: `anchor/trap` is not in the hello-room prefab, so
    // this variant is deliberately never validated/built — the flow model reads
    // the declaration, not the geometry.
    let c = variant("trap", q);
    let flow = Flow::new(&c);
    assert!(flow.any_completable().contains("obj/walk-out"));
}

/// Every quest and objective the model can reach in *some* world is reachable —
/// the abandoned ending is not reported dead just because the exported path does
/// not take it.
#[test]
fn abandoned_branch_is_not_reported_dead() {
    let c = parse(&branch_endings_dir());
    let flow = Flow::new(&c);
    assert!(flow.any_active().contains("quest/bolt"));
    assert!(flow.any_completed().contains("quest/bolt"));
    let cs: BTreeSet<String> = codes(&c).into_iter().collect();
    assert!(!cs.contains("DW0202") && !cs.contains("DW0203"), "{cs:?}");
}

/// Remove every `requires_flags` from objectives and from the self-gated
/// `set-flag` effects, leaving both endings unconditionally completable.
fn strip_flag_gates(q: &mut serde_json::Value) {
    for quest in q["content"]["quests"].as_array_mut().unwrap() {
        for obj in quest["objectives"].as_array_mut().unwrap() {
            obj.as_object_mut().unwrap().remove("requires_flags");
        }
        if let Some(map) = quest
            .get_mut("on_objective_complete")
            .and_then(|m| m.as_object_mut())
        {
            for effs in map.values_mut() {
                for e in effs.as_array_mut().unwrap() {
                    e.as_object_mut().unwrap().remove("requires_flags");
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// optional participation (`DW0205`)
// ---------------------------------------------------------------------------
//
// The owner's contract: *the mainline must be completable with zero optional
// participation.* The fixtures below are the island's owner-hit softlock reduced
// to its structure, in both directions — a beat that is genuinely elective
// passes, a beat the fiction disguises as elective and the graph is load-bearing
// on reds.

/// The island `quest/shipwrecked` shape, parameterized on how (or whether) the
/// way to the "Lead on." button — the one that completes the beat AFTER the
/// drowned — is gated. Three objectives: a `talk-to` whose completion spawns the
/// wave, the `kill` on that wave, and a `talk-to` that ends the delve.
///
/// `gated` puts the completing option behind a second node the player can only
/// navigate to once the surf is dead. That is the shape both rules admit: the
/// completing option itself stays **ungated** (`DW0191` demands that — a
/// `talk-to` must never be able to deadlock the moment it activates), and the
/// path to it is what carries the gate.
fn beach_docs(gated: bool, keepsake: bool) -> serde_json::Value {
    let lead_on = serde_json::json!({
        "label": "Lead on.",
        "effects": [{ "type": "complete-objective", "objective": "obj/climb-out" }]
    });
    let mut greeting = vec![serde_json::json!({
        "label": "We climb.",
        "effects": [{ "type": "complete-objective", "objective": "obj/muster" }]
    })];
    let mut nodes = Vec::new();
    if gated {
        greeting.push(serde_json::json!({
            "label": "The surf is done.",
            "requires_flags": ["flag/ashore"],
            "next": "dlg/ledge"
        }));
        nodes.push(serde_json::json!({
            "id": "dlg/ledge",
            "text": "The last of them slid back under the foam. Up, then.",
            "options": [lead_on]
        }));
    } else {
        greeting.push(lead_on);
    }
    if keepsake {
        // Genuinely optional participation: a keepsake nothing on the mainline
        // ever reads, offered from the first tick and never required.
        greeting.push(serde_json::json!({
            "label": "Take the wine-skin.",
            "effects": [{ "type": "set-flag", "flag": "flag/keepsake" }]
        }));
    }
    nodes.insert(
        0,
        serde_json::json!({
            "id": "dlg/greeting",
            "text": "Twelve of us on this beach, Captain, and I do not like that smoke.",
            "options": greeting
        }),
    );
    serde_json::json!({
        "quest-plan": {
            "dsl_version": "0.19.0",
            "campaign_id": "hello-world",
            "stage": "quest-plan",
            "content": {
                "quests": [{
                    "id": "quest/beach",
                    "goal": "Muster the crew, hold the surf, and climb for the smoke.",
                    "area": "area/keep",
                    "npcs": ["npc/keeper"],
                    "depends_on": [],
                    "mandatory": true,
                    "act": 1
                }],
                "finale": "quest/beach"
            }
        },
        "quests": {
            "dsl_version": "0.19.0",
            "campaign_id": "hello-world",
            "stage": "quests",
            "content": {
                "quests": [{
                    "id": "quest/beach",
                    "trigger": { "type": "campaign-start" },
                    "objectives": [
                        { "type": "talk-to", "id": "obj/muster", "npc": "npc/keeper" },
                        {
                            "type": "kill", "id": "obj/surf", "wave": "wave/surf",
                            "after": ["obj/muster"]
                        },
                        {
                            "type": "talk-to", "id": "obj/climb-out", "npc": "npc/keeper",
                            "after": ["obj/surf"]
                        }
                    ],
                    "on_objective_complete": {
                        "obj/muster": [{ "type": "spawn-wave", "wave": "wave/surf" }],
                        "obj/surf": [{ "type": "set-flag", "flag": "flag/ashore" }],
                        "obj/climb-out": [{ "type": "campaign-complete" }]
                    },
                    "on_complete": []
                }],
                "waves": [{
                    "id": "wave/surf",
                    "anchor": "anchor/exit",
                    "mobs": [{ "entity": "minecraft:drowned", "count": 3 }]
                }]
            }
        },
        "dialogue": {
            "dsl_version": "0.19.0",
            "campaign_id": "hello-world",
            "stage": "dialogue",
            "content": {
                "dialogues": [{
                    "npc": "npc/keeper",
                    "root": "dlg/greeting",
                    "nodes": nodes
                }]
            }
        }
    })
}

/// The island r16 softlock, reduced: "Lead on." (completing `obj/climb-out`) sits
/// beside "We climb." (completing `obj/muster`) from the first tick, ungated. A
/// player takes it, the drowned never come out of the surf, and the quest cannot
/// complete — the exact structure the owner hit live. `DW0205` names the
/// objective, the beat, and the `after` edge.
#[test]
fn a_disguised_mainline_beat_reds() {
    let c = variant_docs("optional-red", beach_docs(false, false));
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let d = analyze_campaign(&c, &prefabs);
    let hit = d
        .iter()
        .find(|x| x.code == "DW0205")
        .unwrap_or_else(|| panic!("the island's muster/surf structure must red: {d:#?}"));
    assert!(hit.message.contains("obj/climb-out"), "{}", hit.message);
    assert!(hit.message.contains("obj/muster"), "{}", hit.message);
    assert!(
        hit.message.contains("declares `after` on `obj/muster`"),
        "the dependency edge must be named: {}",
        hit.message
    );
    assert!(
        hit.message
            .contains("`obj/muster` is what spawns `wave/surf`"),
        "the staging the skip costs must be named: {}",
        hit.message
    );
}

/// The same campaign with the button gated on the flag the skipped beats produce:
/// "Lead on." cannot appear until the surf is dead. Nothing is skippable, and the
/// genuinely optional keepsake option — offered from the first tick, read by
/// nothing — does not make the campaign red.
#[test]
fn a_legitimately_optional_beat_passes() {
    let c = variant_docs("optional-green", beach_docs(true, true));
    assert!(
        !codes(&c).contains(&"DW0205".to_string()),
        "a gated approach and an elective keepsake are legal: {:?}",
        codes(&c)
    );
    // …and the remedy the diagnostic prescribes is one the rest of the validator
    // admits: the completing option stayed ungated, so `DW0191` is silent too.
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let d = delvewright_dsl::validate_campaign_with(
        &c,
        &delvewright_compiler::registry::FullItemRegistry::v1_21_11(),
        &prefabs,
        &delvewright_compiler::registry::FullEntityRegistry::v1_21_11(),
    );
    assert!(
        !d.iter().any(|x| x.code == "DW0191"),
        "the fix must not trade DW0205 for DW0191: {d:#?}"
    );
}

/// The other dependency edge: the offered objective declares no `after`, but
/// requires a flag the skipped beat is what sets (the island's `obj/the-stone` /
/// `flag/sealed` shape). Same skip, different edge, and the message says so.
#[test]
fn a_flag_edge_skip_reds() {
    let docs = serde_json::json!({
        "quest-plan": {
            "dsl_version": "0.19.0", "campaign_id": "hello-world", "stage": "quest-plan",
            "content": {
                "quests": [{
                    "id": "quest/hide", "goal": "Take cover, then answer the stone.",
                    "area": "area/keep", "npcs": ["npc/keeper"],
                    "depends_on": [], "mandatory": true, "act": 1
                }],
                "finale": "quest/hide"
            }
        },
        "quests": {
            "dsl_version": "0.19.0", "campaign_id": "hello-world", "stage": "quests",
            "content": {
                "quests": [{
                    "id": "quest/hide",
                    "trigger": { "type": "campaign-start" },
                    "objectives": [
                        {
                            "type": "reach-anchor", "id": "obj/take-cover",
                            "anchor": "anchor/exit", "radius": 2
                        },
                        {
                            "type": "talk-to", "id": "obj/the-stone", "npc": "npc/keeper",
                            "requires_flags": ["flag/sealed"]
                        }
                    ],
                    "on_objective_complete": {
                        "obj/take-cover": [{ "type": "set-flag", "flag": "flag/sealed" }],
                        "obj/the-stone": [{ "type": "campaign-complete" }]
                    },
                    "on_complete": []
                }]
            }
        },
        "dialogue": {
            "dsl_version": "0.19.0", "campaign_id": "hello-world", "stage": "dialogue",
            "content": {
                "dialogues": [{
                    "npc": "npc/keeper", "root": "dlg/greeting",
                    "nodes": [{
                        "id": "dlg/greeting",
                        "text": "The stone is across the mouth and he is coming back.",
                        "options": [{
                            "label": "The stone.",
                            "effects": [{ "type": "complete-objective", "objective": "obj/the-stone" }]
                        }]
                    }]
                }]
            }
        }
    });
    let c = variant_docs("optional-flag-edge", docs);
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let d = analyze_campaign(&c, &prefabs);
    let hit = d
        .iter()
        .find(|x| x.code == "DW0205")
        .unwrap_or_else(|| panic!("a flag-edge skip must red: {d:#?}"));
    assert!(
        hit.message
            .contains("requires `flag/sealed`, and `obj/take-cover` is what sets it"),
        "the flag edge must be named: {}",
        hit.message
    );
}
