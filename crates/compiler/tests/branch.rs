//! Branch-complete narrative verification (spec-0025): enumeration, the six
//! `DW048x` proofs, and the two validation artifacts.
//!
//! Fixture shape: `branch-two-endings` — the keep-crawl reduced to one fork
//! (hold the gate / bolt for the road) running to two distinct endings, at DSL
//! v0.8.0 with a full `happening` account and per-branch casts. It is the GREEN
//! reference; every red test is that fixture with one field moved, which is what
//! makes each diagnostic's cause unambiguous.
//!
//! The red family deliberately replays the island round-13 shape: a branch
//! opened, and a later quest's cast still belonging to the other branch.
//!
//! Each branch opens the keep gate for itself — the hold branch when the watch is
//! stood, the bolt branch by throwing the bar off on the way out. That is not
//! decoration: with only the hold branch's `open-gate`, the bolt branch's run for
//! the exit crossed a portcullis nothing on that branch ever lifted, and the first
//! live branch run (spec-0025 harness tier) stranded on it. A branch path is
//! flow-proven, not yet nav-proven — see the "Known gap" note in
//! `docs/reference/compiler.md` §"The branch artifacts".

mod common;

use delvewright_compiler::branch;
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};
use serde_json::Value;

fn fixture_dir() -> std::path::PathBuf {
    common::compiler_fixtures_dir().join("branch-two-endings")
}

fn doc(name: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(fixture_dir().join(name)).unwrap()).unwrap()
}

/// The green fixture, with `patch` applied to the named stage documents.
fn campaign_with(patch: impl Fn(&mut Value, &mut Value, &mut Value)) -> Campaign {
    let (mut plan, mut quests, mut dialogue) = (
        doc("quest-plan.json"),
        doc("quests.json"),
        doc("dialogue.json"),
    );
    patch(&mut plan, &mut quests, &mut dialogue);
    parse_campaign(&RawCampaign {
        world: std::fs::read_to_string(fixture_dir().join("world.json")).unwrap(),
        npcs: std::fs::read_to_string(fixture_dir().join("npcs.json")).unwrap(),
        classes: std::fs::read_to_string(fixture_dir().join("classes.json")).unwrap(),
        quest_plan: plan.to_string(),
        quests: quests.to_string(),
        dialogue: dialogue.to_string(),
        world_edits: None,
    })
    .expect("fixture must parse")
}

fn green() -> Campaign {
    campaign_with(|_, _, _| {})
}

fn codes(c: &Campaign) -> Vec<String> {
    branch::check_branches(c)
        .iter()
        .map(|d| d.code.clone())
        .collect()
}

fn find(diags: Vec<delvewright_dsl::Diagnostic>, code: &str) -> delvewright_dsl::Diagnostic {
    diags
        .iter()
        .find(|d| d.code == code)
        .cloned()
        .unwrap_or_else(|| panic!("expected {code}, got: {diags:#?}"))
}

/// A quest of `quests.json`, by id.
fn quest<'a>(quests: &'a mut Value, id: &str) -> &'a mut Value {
    quests["content"]["quests"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|q| q["id"] == id)
        .unwrap()
}

// --- enumeration + the green reference ------------------------------------

/// One branch point with two branches enumerates exactly two branches, each
/// pinning its own flag SET and its sibling's UNSET.
#[test]
fn one_point_enumerates_the_product_with_siblings_pinned_unset() {
    let c = green();
    let bs = branch::enumerate(&c);
    assert_eq!(bs.len(), 2, "{bs:#?}");
    let hold = bs.iter().find(|b| b.id == "branch/hold").unwrap();
    assert!(hold.set.contains("flag/wait"));
    assert!(
        hold.unset.contains("flag/flee"),
        "a branch must pin its siblings' flags UNSET: {hold:#?}"
    );
    let bolt = bs.iter().find(|b| b.id == "branch/bolt").unwrap();
    assert!(bolt.set.contains("flag/flee"));
    assert!(bolt.unset.contains("flag/wait"));
}

/// The whole green fixture raises nothing.
#[test]
fn green_two_branch_fixture_is_clean() {
    let diags = branch::check_branches(&green());
    assert!(
        diags.is_empty(),
        "expected a clean campaign, got: {diags:#?}"
    );
}

/// Each branch is realized against its own world, reaches the ending it
/// declares, and gets a chronicle that runs start → ending.
#[test]
fn each_branch_reaches_the_ending_it_declares() {
    let c = green();
    let rs = branch::realize(&c);
    for r in &rs {
        assert!(r.world.is_some(), "{} is not realized", r.branch.id);
        assert!(
            r.branch.leads_to.iter().all(|e| r.endings.contains(e)),
            "{} declares {:?} but reaches {:?}",
            r.branch.id,
            r.branch.leads_to,
            r.endings
        );
        assert!(!r.chronicle.is_empty(), "{} has no chronicle", r.branch.id);
    }
}

/// The bolt branch's chronicle never mentions the hold branch's beats — the
/// account is per branch, not a merge.
#[test]
fn a_branch_chronicle_carries_only_its_own_beats() {
    let c = green();
    let rs = branch::realize(&c);
    let bolt = rs.iter().find(|r| r.branch.id == "branch/bolt").unwrap();
    let nodes: Vec<&str> = bolt.chronicle.iter().map(|l| l.node.as_str()).collect();
    assert!(nodes.contains(&"quest/bolt"), "{nodes:?}");
    assert!(
        !nodes.contains(&"quest/hold"),
        "the bolt chronicle must not carry the hold branch's quest: {nodes:?}"
    );
}

// --- DW0480: undeclared story fork ----------------------------------------

/// A campaign whose flags fork casts/staging/structure with NO declared branch
/// point fails: an undeclared fork is a branch nothing verifies.
#[test]
fn undeclared_fork_is_dw0480() {
    let c = campaign_with(|plan, _, _| {
        plan["content"]
            .as_object_mut()
            .unwrap()
            .remove("branch_points");
    });
    let d = find(branch::check_branches(&c), "DW0480");
    assert!(
        d.message.contains("flag/wait") || d.message.contains("flag/flee"),
        "{}",
        d.message
    );
    assert!(d.message.contains("UNDECLARED"), "{}", d.message);
}

/// A flag every playthrough sets is ordinary sequencing, not a fork — declaring
/// it nowhere raises nothing.
#[test]
fn an_unconditional_flag_is_not_a_fork() {
    let c = campaign_with(|plan, quests, _| {
        plan["content"]
            .as_object_mut()
            .unwrap()
            .remove("branch_points");
        // Every branch sets `flag/greeted`, and it gates an objective.
        quest(quests, "quest/decide")["on_objective_complete"]["obj/decide"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({ "type": "set-flag", "flag": "flag/greeted" }));
        quest(quests, "quest/hold")["objectives"][0]["requires_flags"]
            .as_array_mut()
            .unwrap()
            .push(Value::from("flag/greeted"));
    });
    let hits: Vec<delvewright_dsl::Diagnostic> = branch::check_branches(&c)
        .into_iter()
        .filter(|d| d.code == "DW0480" && d.message.contains("flag/greeted"))
        .collect();
    assert!(
        hits.is_empty(),
        "an always-set flag must not be reported as a fork: {hits:#?}"
    );
}

// --- DW0481: the forcing function ------------------------------------------

/// A quest with no `happening` fails at 0.8.0.
#[test]
fn quest_without_happening_is_dw0481() {
    let c = campaign_with(|_, quests, _| {
        quest(quests, "quest/hold")
            .as_object_mut()
            .unwrap()
            .remove("happening");
    });
    let d = find(branch::check_branches(&c), "DW0481");
    assert!(d.message.contains("quest/hold"), "{}", d.message);
}

/// An objective with no `happening` fails at 0.8.0.
#[test]
fn objective_without_happening_is_dw0481() {
    let c = campaign_with(|_, quests, _| {
        quest(quests, "quest/hold")["objectives"][0]
            .as_object_mut()
            .unwrap()
            .remove("happening");
    });
    let d = find(branch::check_branches(&c), "DW0481");
    assert!(d.message.contains("obj/watch"), "{}", d.message);
}

/// A staging effect with no `happening` fails at 0.8.0 — the `open-gate` here.
#[test]
fn staging_effect_without_happening_is_dw0481() {
    let c = campaign_with(|_, quests, _| {
        quest(quests, "quest/hold")["on_objective_complete"]["obj/watch"][0]
            .as_object_mut()
            .unwrap()
            .remove("happening");
    });
    let d = find(branch::check_branches(&c), "DW0481");
    assert!(d.message.contains("open-gate"), "{}", d.message);
}

/// A dialogue option that SETS A FLAG is a story-weight beat and must declare
/// one; an option that only walks the tree needs none.
#[test]
fn story_weight_dialogue_option_without_happening_is_dw0481() {
    let c = campaign_with(|_, _, dialogue| {
        dialogue["content"]["dialogues"][0]["nodes"][0]["options"][1]
            .as_object_mut()
            .unwrap()
            .remove("happening");
    });
    let d = find(branch::check_branches(&c), "DW0481");
    assert!(d.message.contains("story-weight"), "{}", d.message);
    // The plain "Who are you?" option declares none and is never reported.
    assert!(
        !branch::check_branches(&green())
            .iter()
            .any(|x| x.code == "DW0481")
    );
}

// --- DW0482: terminality ---------------------------------------------------

/// A branch nobody can take reaches no ending.
#[test]
fn unreachable_branch_is_dw0482() {
    let c = campaign_with(|plan, _, _| {
        // Hold BOTH forking flags: the two dialogue options are XOR, so no
        // playthrough produces both.
        plan["content"]["branch_points"][0]["branches"][1]["flags"] =
            serde_json::json!(["flag/flee", "flag/wait"]);
    });
    let d = find(branch::check_branches(&c), "DW0482");
    assert!(d.message.contains("NOT REACHABLE"), "{}", d.message);
    assert!(d.message.contains("branch/bolt"), "{}", d.message);
}

/// A branch that declares an ending it never reaches fails, naming the ending
/// that actually fires.
#[test]
fn branch_reaching_the_wrong_ending_is_dw0482() {
    let c = campaign_with(|plan, _, _| {
        plan["content"]["branch_points"][0]["branches"][1]["leads_to"] = Value::from("ending/held");
    });
    let d = find(branch::check_branches(&c), "DW0482");
    assert!(d.message.contains("ending/held"), "{}", d.message);
    assert!(d.message.contains("ending/abandoned"), "{}", d.message);
}

// --- DW0483: cast continuity (the island round-13 defect) ------------------

/// The round-13 shape: the branch opened, and a later quest's cast still
/// belongs to the other branch — here because a placement was left UNGATED, so
/// it co-selects on the branch that already has its own.
#[test]
fn post_fork_cast_belonging_to_the_other_branch_is_dw0483() {
    let c = campaign_with(|_, quests, _| {
        quest(quests, "quest/hold")["cast"]["npc/keeper"][1]
            .as_object_mut()
            .unwrap()
            .remove("requires_flags");
    });
    let d = find(branch::check_branches(&c), "DW0483");
    assert!(d.message.contains("branch/hold"), "{}", d.message);
    assert!(d.message.contains("npc/keeper"), "{}", d.message);
    assert!(d.message.contains("quest/hold"), "{}", d.message);
    assert!(d.message.contains("2 placements select"), "{}", d.message);
}

/// The mirror: a post-fork quest where NO per-branch placement selects — the
/// NPC has no declared position on that branch at all.
#[test]
fn post_fork_cast_selecting_nothing_is_dw0483() {
    let c = campaign_with(|_, quests, _| {
        quest(quests, "quest/hold")["cast"]["npc/keeper"][0]["requires_flags"] =
            serde_json::json!(["flag/flee"]);
    });
    let d = find(branch::check_branches(&c), "DW0483");
    assert!(
        d.message.contains("NO per-branch placement"),
        "{}",
        d.message
    );
}

// --- DW0484: exclusive-content leakage -------------------------------------

/// The mourning-scene shape: an ambient producer sets the OTHER branch's flag on
/// every playthrough, so branch-A-gated content is reachable under branch B.
#[test]
fn sibling_flag_produced_on_every_branch_is_dw0484() {
    let c = campaign_with(|_, quests, _| {
        quests["content"]["triggers"] = serde_json::json!([{
            "id": "trigger/bell",
            "at": "anchor/door",
            "on": { "on": "use" },
            "effects": [{ "type": "set-flag", "flag": "flag/wait" }]
        }]);
    });
    let d = find(branch::check_branches(&c), "DW0484");
    assert!(d.message.contains("LEAKS"), "{}", d.message);
    assert!(d.message.contains("branch/bolt"), "{}", d.message);
    assert!(d.message.contains("flag/wait"), "{}", d.message);
}

// --- DW0485: hard event contradictions -------------------------------------

/// `dies` then acts, on one branch, with both chronicle lines shown.
#[test]
fn dies_then_acts_on_one_branch_is_dw0485() {
    let c = campaign_with(|_, quests, _| {
        quest(quests, "quest/decide")["objectives"][0]["happening"] = serde_json::json!({
            "verb": "dies",
            "text": "The Keeper is dragged into the moor before the party can speak.",
            "subject": "npc/keeper"
        });
    });
    let d = find(branch::check_branches(&c), "DW0485");
    assert!(d.message.contains("acts after it dies"), "{}", d.message);
    assert!(d.message.contains("npc/keeper"), "{}", d.message);
    // Both lines, with their chronicle positions.
    assert!(d.message.contains("[dies]"), "{}", d.message);
    assert!(d.message.contains("[survives]"), "{}", d.message);
    assert!(d.message.contains("branch/hold"), "{}", d.message);
}

/// `seals` then used — the gate walked through after it was sealed.
#[test]
fn seals_then_used_is_dw0485() {
    let c = campaign_with(|_, quests, _| {
        quest(quests, "quest/hold")["on_objective_complete"]["obj/watch"][0]["happening"] = serde_json::json!({
            "verb": "seals",
            "text": "The Keeper drops the bar across the gate for good.",
            "subject": "anchor/door"
        });
        quest(quests, "quest/hold")["objectives"][1]["happening"] = serde_json::json!({
            "verb": "departs",
            "text": "The party walks out through the gate.",
            "subject": "anchor/door"
        });
    });
    let d = find(branch::check_branches(&c), "DW0485");
    assert!(d.message.contains("after it is sealed"), "{}", d.message);
}

/// `loses` then `loses` with no `gains` between — the token spent twice.
#[test]
fn loses_then_spent_again_is_dw0485() {
    let c = campaign_with(|_, quests, _| {
        quest(quests, "quest/decide")["objectives"][0]["happening"] = serde_json::json!({
            "verb": "loses",
            "text": "The party hands over the road-warden's token.",
            "subject": "item/warden-token"
        });
        quest(quests, "quest/hold")["objectives"][0]["happening"] = serde_json::json!({
            "verb": "loses",
            "text": "The party hands over the road-warden's token.",
            "subject": "item/warden-token"
        });
    });
    let d = find(branch::check_branches(&c), "DW0485");
    assert!(d.message.contains("spent twice over"), "{}", d.message);
}

/// `departs` cleared by `arrives` is NOT a contradiction — the state machine
/// forgets, it does not accumulate.
#[test]
fn departs_then_arrives_then_acts_is_clean() {
    let c = campaign_with(|_, quests, _| {
        quest(quests, "quest/decide")["objectives"][0]["happening"] = serde_json::json!({
            "verb": "departs", "text": "The Keeper steps inside.", "subject": "npc/keeper"
        });
        quest(quests, "quest/hold")["happening"] = serde_json::json!({
            "verb": "arrives", "text": "The Keeper comes back out.", "subject": "npc/keeper"
        });
    });
    assert!(
        !codes(&c).contains(&"DW0485".to_string()),
        "{:#?}",
        branch::check_branches(&c)
    );
}

// --- the version fence -----------------------------------------------------

/// Below 0.8.0 nothing in this module fires — which is what keeps every 0.6/0.7
/// campaign's datapack byte-identical.
#[test]
fn pre_08_campaign_raises_no_branch_diagnostics() {
    let hw = common::hello_world_dir();
    let c = parse_campaign(&RawCampaign {
        world: std::fs::read_to_string(hw.join("world.json")).unwrap(),
        npcs: std::fs::read_to_string(hw.join("npcs.json")).unwrap(),
        classes: std::fs::read_to_string(hw.join("classes.json")).unwrap(),
        quest_plan: std::fs::read_to_string(hw.join("quest-plan.json")).unwrap(),
        quests: std::fs::read_to_string(hw.join("quests.json")).unwrap(),
        dialogue: std::fs::read_to_string(hw.join("dialogue.json")).unwrap(),
        world_edits: None,
    })
    .expect("hello-world parses");
    assert!(branch::check_branches(&c).is_empty());
    assert!(
        branch::artifacts(&c).is_empty(),
        "a pre-0.8 campaign emits no branch artifacts"
    );
}

// --- artifacts -------------------------------------------------------------

/// The two artifacts are emitted, named per branch, and byte-identical across
/// runs (ADR-0006).
#[test]
fn artifacts_are_emitted_and_deterministic() {
    let c = green();
    let a = branch::artifacts(&c);
    let b = branch::artifacts(&c);
    assert_eq!(a, b, "branch artifacts must be byte-identical across runs");
    let keys: Vec<&String> = a.keys().collect();
    assert_eq!(
        keys,
        vec![
            "validation/branch-chronicle-bolt.md",
            "validation/branch-chronicle-hold.md",
            "validation/branch-plan.json",
        ]
    );
    let plan: Value = serde_json::from_slice(&a["validation/branch-plan.json"]).unwrap();
    let branches = plan["branches"].as_array().unwrap();
    assert_eq!(branches.len(), 2);
    // Every branch names its flags, its path and the choices that enter it.
    for b in branches {
        assert!(b["flags"]["set"].as_array().unwrap().len() == 1);
        assert!(b["flags"]["unset"].as_array().unwrap().len() == 1);
        assert!(!b["critical_path"].as_array().unwrap().is_empty());
        assert!(!b["entry_choices"].as_array().unwrap().is_empty());
    }
    let md = String::from_utf8(a["validation/branch-chronicle-hold.md"].clone()).unwrap();
    assert!(md.contains("# Branch chronicle — `branch/hold`"), "{md}");
    assert!(md.contains("ending/held"), "{md}");
    // The chronicle is readable start → ending: the first line is the opening
    // beat and the endings section is last.
    let first = md.find("1. **arrives**").expect("opening beat");
    let last = md.find("## Endings reached").expect("endings section");
    assert!(first < last, "{md}");
}

/// **How a branch choice is actuated** (spec-0025 §3, harness half). A 1.21.11
/// dialog button is client-rendered, so no bot can click one; every option is
/// backed by the `/trigger dw.dlg_<npc> set <n>` the button itself runs, and the
/// plan carries that line so the harness never has to reconstruct the compiler's
/// id mangling (which would be game logic in a harness that holds none).
#[test]
fn entry_choices_carry_the_command_that_takes_them() {
    let c = green();
    let a = branch::artifacts(&c);
    let plan: Value = serde_json::from_slice(&a["validation/branch-plan.json"]).unwrap();
    let by_id: std::collections::BTreeMap<&str, &Value> = plan["branches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| (b["id"].as_str().unwrap(), b))
        .collect();
    // The two branches are entered by two DIFFERENT options of the same NPC —
    // which is exactly what makes a branch run a scripted-choice run.
    let hold = &by_id["branch/hold"]["entry_choices"][0];
    let bolt = &by_id["branch/bolt"]["entry_choices"][0];
    assert_eq!(hold["npc"], "npc/keeper");
    assert_eq!(hold["command"], "/trigger dw.dlg_keeper set 2");
    assert_eq!(bolt["command"], "/trigger dw.dlg_keeper set 3");
    assert_ne!(hold["option"], bolt["option"]);
    // Each reachable branch names the executable path the harness walks for it.
    assert_eq!(by_id["branch/hold"]["path"], "branch-path-hold.json");
    assert_eq!(by_id["branch/bolt"]["path"], "branch-path-bolt.json");
}

/// An option index is 1-based across ONE NPC's tree, so it must be resolved
/// against the tree of the NPC the step's own `talk-to` names. Resolved against
/// every tree, the same ordinal names a different option of a different speaker —
/// and the harness would then chat a line that enters no branch at all.
#[test]
fn an_entry_choice_is_resolved_against_its_own_speaker() {
    let c = campaign_with(|_, quests, dialogue| {
        // A second NPC whose 2nd/3rd options set nothing, sharing the ordinals the
        // Keeper's branching options use.
        dialogue["content"]["dialogues"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "npc": "npc/watcher",
                "root": "dlg/watcher",
                "nodes": [{
                    "id": "dlg/watcher",
                    "text": "The watcher says nothing you have not heard.",
                    "options": [
                        { "label": "Rain again." },
                        { "label": "Still here?" },
                        { "label": "Goodnight." }
                    ]
                }]
            }));
        let _ = quests;
    });
    // Adding a silent second speaker must not add, move or rename a single entry
    // choice: every one still belongs to the Keeper.
    let a = branch::artifacts(&c);
    let plan: Value = serde_json::from_slice(&a["validation/branch-plan.json"]).unwrap();
    for b in plan["branches"].as_array().unwrap() {
        let choices = b["entry_choices"].as_array().unwrap();
        assert_eq!(choices.len(), 1, "{b}");
        assert_eq!(choices[0]["npc"], "npc/keeper");
    }
}
