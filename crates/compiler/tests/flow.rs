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

/// The fixture's stage-5 document, as a mutable JSON value.
fn quests_json() -> serde_json::Value {
    serde_json::from_str(
        &std::fs::read_to_string(branch_endings_dir().join("quests.json")).unwrap(),
    )
    .unwrap()
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
