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

use crate::common;

use delvec::compiler::branch;
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
    campaign_with_cast(|plan, quests, dialogue, _| patch(plan, quests, dialogue))
}

/// [`campaign_with`], reaching `npcs.json` as well — what a test needs when the
/// question is about which SPEAKER something resolves to, since that is only
/// answerable with more than one speaker in the campaign.
fn campaign_with_cast(patch: impl Fn(&mut Value, &mut Value, &mut Value, &mut Value)) -> Campaign {
    let (mut plan, mut quests, mut dialogue, mut npcs) = (
        doc("quest-plan.json"),
        doc("quests.json"),
        doc("dialogue.json"),
        doc("npcs.json"),
    );
    patch(&mut plan, &mut quests, &mut dialogue, &mut npcs);
    parse_campaign(&RawCampaign {
        world: std::fs::read_to_string(fixture_dir().join("world.json")).unwrap(),
        npcs: npcs.to_string(),
        classes: std::fs::read_to_string(fixture_dir().join("classes.json")).unwrap(),
        quest_plan: plan.to_string(),
        quests: quests.to_string(),
        dialogue: dialogue.to_string(),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
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

/// A dialogue option's ordinal is 1-based **within its own NPC's tree**, so the
/// chronicle must resolve it there. With a second speaker declared FIRST, the
/// campaign-wide enumeration answers about that speaker's option instead — an
/// honest answer about the wrong object — and the fork's own line disappears
/// from both accounts, leaving two chronicles that differ only in their
/// headers. Which is the whole thing step 11 reads.
#[test]
fn a_forks_own_line_is_read_out_of_the_forking_npcs_tree() {
    let c = campaign_with_cast(|_, _, dlg, npcs| {
        // A second speaker, ahead of the Keeper in both stage documents, with
        // enough options to swallow the Keeper's ordinals 2 and 3 and not one
        // `happening` between them.
        let keeper = npcs["content"]["npcs"][0].clone();
        let mut sexton = keeper.clone();
        sexton["id"] = Value::from("npc/sexton");
        sexton["name"] = Value::from("The Sexton");
        sexton["role"] = Value::from("flavor");
        npcs["content"]["npcs"] = serde_json::json!([sexton, keeper]);
        let tree = serde_json::json!({
            "npc": "npc/sexton",
            "root": "dlg/sexton-root",
            "nodes": [{
                "id": "dlg/sexton-root",
                "text": "The Sexton is counting the drowned road's markers and does not look up.",
                "options": [
                    { "label": "How many are there?" },
                    { "label": "Who set them?" },
                    { "label": "Nothing." },
                    { "label": "We will leave you to it." },
                ],
            }],
        });
        let trees = dlg["content"]["dialogues"].as_array_mut().unwrap();
        trees.insert(0, tree);
    });
    assert_eq!(c.dialogue.content.dialogues[0].npc.as_str(), "npc/sexton");
    let rs = branch::realize(&c);
    for (id, want) in [
        ("branch/hold", "stand the watch"),
        ("branch/bolt", "refuses the watch"),
    ] {
        let r = rs.iter().find(|r| r.branch.id == id).unwrap();
        let choice = r
            .chronicle
            .iter()
            .find(|l| l.kind == "choice")
            .unwrap_or_else(|| panic!("{id} has no choice line: {:#?}", r.chronicle));
        assert!(
            choice.node.starts_with("npc/keeper dlg/greeting#"),
            "the line must name the Keeper's own node and ordinal: {}",
            choice.node
        );
        assert!(choice.text.contains(want), "{id}: {}", choice.text);
    }
    // …and that is the divergence, so the two accounts are not the same text.
    let hold = rs.iter().find(|r| r.branch.id == "branch/hold").unwrap();
    let bolt = rs.iter().find(|r| r.branch.id == "branch/bolt").unwrap();
    assert_ne!(
        hold.chronicle
            .iter()
            .filter(|l| l.kind == "choice")
            .collect::<Vec<_>>(),
        bolt.chronicle
            .iter()
            .filter(|l| l.kind == "choice")
            .collect::<Vec<_>>(),
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

/// Cut the ONE `next` link into `dlg/watch`, leaving the `quest/hold` cast
/// ledger as its only way in — the shape the ledger exists for, an NPC's
/// right-click being a different scene once a quest has begun.
///
/// The node is reachable, so the `obj/watch` its option completes is
/// completable, so the hold branch runs to its ending and nothing is raised.
/// The flow model used to walk from the stage-6 `root` alone and reported the
/// branch `NOT REACHABLE`.
#[test]
fn a_node_only_the_cast_ledger_opens_is_reachable() {
    let c = campaign_with(|_, _, dlg| {
        let opts = dlg["content"]["dialogues"][0]["nodes"][0]["options"]
            .as_array_mut()
            .unwrap();
        let before = opts.len();
        opts.retain(|o| o["next"] != "dlg/watch");
        assert_eq!(before - opts.len(), 1, "exactly one `next` link is cut");
    });
    assert!(
        !c.quests.content.quests.is_empty(),
        "the fixture still parses"
    );
    let diags = branch::check_branches(&c);
    assert!(
        diags.is_empty(),
        "a node the cast ledger opens is reachable: {diags:#?}"
    );
}

/// The same cut, with the ledger root taken away too: now NOTHING opens
/// `dlg/watch`, `obj/watch` completes on no playthrough, and the hold branch
/// reaches no ending. Broadening the walk must not cost this refusal — it is
/// the reason `DW0482` exists.
#[test]
fn a_node_nothing_opens_is_still_dw0482() {
    let c = campaign_with(|_, quests, dlg| {
        let opts = dlg["content"]["dialogues"][0]["nodes"][0]["options"]
            .as_array_mut()
            .unwrap();
        let before = opts.len();
        opts.retain(|o| o["next"] != "dlg/watch");
        assert_eq!(before - opts.len(), 1, "exactly one `next` link is cut");
        // The ledger's root swap becomes a bark pool: a body with lines and no
        // options, which opens no node at all.
        let placement = &mut quest(quests, "quest/hold")["cast"]["npc/keeper"][0];
        assert_eq!(placement["dialogue"], "dlg/watch");
        placement["dialogue"] = serde_json::json!({ "barks": ["Still here."] });
    });
    let d = find(branch::check_branches(&c), "DW0482");
    assert!(d.message.contains("branch/hold"), "{}", d.message);
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

/// The mourning-scene shape: a producer every playthrough performs — the
/// mandatory opening quest's own completion — sets the OTHER branch's flag, so
/// branch-A-gated content is reachable under branch B.
#[test]
fn sibling_flag_produced_on_every_branch_is_dw0484() {
    let c = campaign_with(|_, quests, _| {
        quest(quests, "quest/decide")["on_complete"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({ "type": "set-flag", "flag": "flag/wait" }));
    });
    let d = find(branch::check_branches(&c), "DW0484");
    assert!(d.message.contains("LEAKS"), "{}", d.message);
    assert!(d.message.contains("branch/bolt"), "{}", d.message);
    assert!(d.message.contains("flag/wait"), "{}", d.message);
}

/// spec-0108 §4.4: an elective ambient producer is a participation a world
/// states, not a firing on every branch. A `use` trigger nothing on the
/// mainline presses sets `flag/wait`; the world that does not press it
/// realizes `branch/bolt` (its `flag/wait` pinned unset), so nothing leaks.
#[test]
fn an_elective_producer_of_a_sibling_flag_is_a_world_that_skips_it() {
    let c = campaign_with(|_, quests, _| {
        quests["content"]["triggers"] = serde_json::json!([{
            "id": "trigger/bell",
            "at": "anchor/door",
            "on": { "on": "use" },
            "effects": [{ "type": "set-flag", "flag": "flag/wait" }]
        }]);
    });
    let d = branch::check_branches(&c);
    assert!(!d.iter().any(|x| x.code == "DW0484"), "{d:#?}");
    let bolt = branch::realize(&c)
        .into_iter()
        .find(|r| r.branch.id == "branch/bolt")
        .unwrap();
    let flow = delvec::compiler::flow::Flow::new(&c);
    let w = bolt.world.expect("branch/bolt is realized");
    assert!(
        !flow.story().worlds[w].performs.contains("trigger/bell"),
        "the realizing world skips the bell: {:?}",
        flow.story().worlds[w]
    );
}

/// A free choice is a choice (the town's grey mare): a dialogue option that
/// completes no objective sets a fork's flag, beside an option that sets
/// nothing. A player may leave the Keeper's lore unasked, so a world that never
/// takes it realizes the branch that pins its flag unset — neither unreachable
/// (`DW0482`) nor leaking (`DW0484`).
#[test]
fn a_dialogue_option_that_completes_nothing_is_a_choice_a_player_may_decline() {
    let c = campaign_with(|plan, _, dialogue| {
        dialogue["content"]["dialogues"][0]["nodes"][1]["options"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "label": "Tell me where the old road ran.",
                "effects": [{ "type": "set-flag", "flag": "flag/told" }],
                "happening": { "subject": "npc/keeper", "text": "The Keeper tells the party where the old road ran.", "verb": "learns" }
            }));
        plan["content"]["branch_points"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "id": "branch-point/road", "opens_at": "quest/decide", "forks_on": ["flag/told"],
                "branches": [
                    { "id": "branch/told", "flags": ["flag/told"], "leads_to": "quest/decide" },
                    { "id": "branch/untold", "flags": [], "leads_to": "quest/decide" }
                ]
            }));
    });
    let d = branch::check_branches(&c);
    let about: Vec<_> = d
        .iter()
        .filter(|x| (x.code == "DW0482" || x.code == "DW0484") && x.message.contains("told"))
        .collect();
    assert!(about.is_empty(), "{about:#?}");
    let untold = branch::realize(&c)
        .into_iter()
        .filter(|r| r.branch.id.contains("branch/untold"))
        .collect::<Vec<_>>();
    assert_eq!(untold.len(), 2, "one per gate branch: {untold:#?}");
    assert!(untold.iter().all(|r| r.world.is_some()), "{untold:#?}");
}

/// The same option where it is the only way to answer the Keeper — it
/// completes the talk the mainline needs — is taken in every world, so its
/// fork's unset branch is still `NOT REACHABLE` and still leaks.
#[test]
fn a_dialogue_option_that_completes_an_objective_is_still_taken_in_every_world() {
    let c = campaign_with(|plan, quests, dialogue| {
        let greeting = &mut dialogue["content"]["dialogues"][0]["nodes"][0]["options"];
        for o in greeting.as_array_mut().unwrap() {
            if o["label"] == "We hold." || o["label"] == "I run." {
                o["effects"]
                    .as_array_mut()
                    .unwrap()
                    .push(serde_json::json!({ "type": "set-flag", "flag": "flag/told" }));
            }
        }
        let _ = quests;
        plan["content"]["branch_points"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "id": "branch-point/road", "opens_at": "quest/decide", "forks_on": ["flag/told"],
                "branches": [
                    { "id": "branch/told", "flags": ["flag/told"], "leads_to": "quest/decide" },
                    { "id": "branch/untold", "flags": [], "leads_to": "quest/decide" }
                ]
            }));
    });
    let d = find(branch::check_branches(&c), "DW0482");
    assert!(d.message.contains("branch/untold"), "{}", d.message);
    let d = find(branch::check_branches(&c), "DW0484");
    assert!(d.message.contains("branch/untold"), "{}", d.message);
}

/// Two elective acts on two forks (the town's shop offer and its stable-yard
/// trigger): the all world performs both and the none world neither, so a
/// branch that takes one and leaves the other has no world but a witness the
/// story search runs over the acts a fork turns on.
#[test]
fn a_branch_one_elective_takes_and_another_leaves_has_a_witness() {
    let c = campaign_with(|plan, quests, _| {
        quests["content"]["triggers"] = serde_json::json!([
            { "id": "trigger/bell", "at": "anchor/door", "on": { "on": "use" },
              "effects": [{ "type": "set-flag", "flag": "flag/rang" }] },
            { "id": "trigger/lamp", "at": "anchor/exit", "on": { "on": "use" },
              "effects": [{ "type": "set-flag", "flag": "flag/lit" }] }
        ]);
        for (id, flag) in [("bell", "flag/rang"), ("lamp", "flag/lit")] {
            plan["content"]["branch_points"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({
                    "id": format!("branch-point/{id}"), "opens_at": "quest/decide", "forks_on": [flag],
                    "branches": [
                        { "id": format!("branch/{id}-yes"), "flags": [flag], "leads_to": "quest/decide" },
                        { "id": format!("branch/{id}-no"), "flags": [], "leads_to": "quest/decide" }
                    ]
                }));
        }
    });
    let d = branch::check_branches(&c);
    let about: Vec<_> = d
        .iter()
        .filter(|x| x.code == "DW0482" || x.code == "DW0484")
        .collect();
    assert!(about.is_empty(), "{about:#?}");
}

/// Two independent failure forks on the Keeper's door (the town's goats and
/// raid): a bell and a lamp the party is asked to mind, each lost by an
/// elective act that fails its task, each act reachable only once a rope
/// (an elective the forks do not turn on) is climbed. With `exclusive`, the
/// lamp can be lost only once the bell is minded, so a night that loses both
/// cannot be played.
fn two_failure_forks(exclusive: bool) -> Campaign {
    campaign_with(|plan, quests, _| {
        let decide = quest(quests, "quest/decide");
        for (thing, anchor) in [("bell", "anchor/door"), ("lamp", "anchor/exit")] {
            decide["objectives"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({
                    "id": format!("obj/{thing}"), "type": "reach-anchor", "anchor": anchor, "radius": 2,
                    "happening": { "text": format!("The party minds the {thing}."), "verb": "learns" }
                }));
            decide["on_objective_complete"][format!("obj/{thing}")] =
                serde_json::json!([{ "type": "set-flag", "flag": format!("flag/{thing}-kept") }]);
        }
        let lost = |thing: &str, other: &str| {
            let mut t = serde_json::json!({
                "id": format!("trigger/{thing}-lost"), "at": "anchor/door", "on": { "on": "use" },
                "requires_flags": ["flag/rope"],
                "effects": [
                    { "type": "set-flag", "flag": format!("flag/{thing}-lost") },
                    { "type": "fail-objective", "objective": format!("obj/{thing}") }
                ]
            });
            if exclusive && thing == "lamp" {
                t["requires_flags"] =
                    serde_json::json!(["flag/rope", format!("flag/{other}-kept")]);
            }
            t
        };
        quests["content"]["triggers"] = serde_json::json!([
            { "id": "trigger/rope", "at": "anchor/exit", "on": { "on": "use" },
              "effects": [{ "type": "set-flag", "flag": "flag/rope" }] },
            lost("bell", "lamp"),
            lost("lamp", "bell")
        ]);
        for thing in ["bell", "lamp"] {
            plan["content"]["branch_points"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({
                    "id": format!("branch-point/{thing}"), "opens_at": "quest/decide",
                    "forks_on": [format!("flag/{thing}-kept"), format!("flag/{thing}-lost")],
                    "branches": [
                        { "id": format!("branch/{thing}-kept"), "flags": [format!("flag/{thing}-kept")], "leads_to": "quest/decide" },
                        { "id": format!("branch/{thing}-lost"), "flags": [format!("flag/{thing}-lost")], "leads_to": "quest/decide" }
                    ]
                }));
        }
    })
}

/// The branches of [`two_failure_forks`] a story world realizes, and those
/// `DW0482` refuses as unreachable.
fn realized_and_refused(c: &Campaign) -> (Vec<String>, Vec<String>) {
    let realized = branch::realize(c)
        .into_iter()
        .filter(|r| r.world.is_some())
        .map(|r| r.branch.id)
        .collect();
    let refused = branch::check_branches(c)
        .into_iter()
        .filter(|d| d.code == "DW0482" && d.message.contains("NOT REACHABLE"))
        .map(|d| d.message.split('`').nth(1).unwrap().to_string())
        .collect();
    (realized, refused)
}

/// A night that loses both the bell and the lamp is a play: the rope climbed,
/// then both losses. No story world composed two losses, so the four
/// combinations reached every branch but the two (one per gate branch) that
/// lose both, and `DW0482` refused them; the branch search runs every
/// participation of the acts their flags turn on, and finds the world.
#[test]
fn two_losses_on_one_night_are_a_world() {
    let c = two_failure_forks(false);
    let (realized, refused) = realized_and_refused(&c);
    assert_eq!(
        realized.len(),
        8,
        "every declared combination is realized: {realized:#?}"
    );
    assert!(refused.is_empty(), "{refused:#?}");
    let d = branch::check_branches(&c);
    assert!(
        !d.iter().any(|x| x.code == "DW0482" || x.code == "DW0484"),
        "{d:#?}"
    );
    let flow = delvec::compiler::flow::Flow::new(&c);
    let both = branch::realize(&c)
        .into_iter()
        .find(|r| r.branch.id == "branch/bell-lost+branch/hold+branch/lamp-lost")
        .unwrap();
    let w = &flow.story().worlds[both.world.unwrap()];
    assert_eq!(w.origin.token(), "branch", "{w:?}");
    for act in ["trigger/rope", "trigger/bell-lost", "trigger/lamp-lost"] {
        assert!(w.performs.contains(act), "{act} in {w:?}");
    }
    let (_, bind) = branch::check_branches_bound(&c);
    let line = bind.search.line();
    assert!(
        line.contains("2 declared branch(es) no chosen world realized"),
        "{line}"
    );
    assert!(line.contains("2 realized, 0 realized by none"), "{line}");
}

/// The same forks with the lamp lost only once the bell is minded: a night
/// that loses both cannot be played, so those two branches still refuse,
/// each naming what the search ran, and every other branch is realized.
#[test]
fn two_losses_each_forbidding_the_other_still_refuse() {
    let c = two_failure_forks(true);
    let (realized, mut refused) = realized_and_refused(&c);
    refused.sort();
    assert_eq!(realized.len(), 6, "{realized:#?}");
    assert_eq!(
        refused,
        vec![
            "branch/bell-lost+branch/bolt+branch/lamp-lost".to_string(),
            "branch/bell-lost+branch/hold+branch/lamp-lost".to_string(),
        ]
    );
    let d = find(branch::check_branches(&c), "DW0482");
    assert!(
        d.message.contains("the branch search ran")
            && d.message.contains("trigger/bell-lost, trigger/lamp-lost"),
        "{}",
        d.message
    );
    let (_, bind) = branch::check_branches_bound(&c);
    assert!(
        bind.search
            .line()
            .contains("0 realized, 2 realized by none"),
        "{}",
        bind.search.line()
    );
}

// --- DW0485: hard event contradictions -------------------------------------

/// `dies` then acts, on one branch, with both chronicle lines shown.
#[test]
fn dies_then_acts_on_one_branch_is_dw0485() {
    let c = dies_then_acts_on_one_branch_is_dw0485_campaign();
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
    let c = seals_then_used_is_dw0485_campaign();
    let d = find(branch::check_branches(&c), "DW0485");
    assert!(d.message.contains("after it is sealed"), "{}", d.message);
}

/// `loses` then `loses` with no `gains` between — the token spent twice.
#[test]
fn loses_then_spent_again_is_dw0485() {
    let c = loses_then_spent_again_is_dw0485_campaign();
    let d = find(branch::check_branches(&c), "DW0485");
    assert!(d.message.contains("spent twice over"), "{}", d.message);
}

/// `departs` cleared by `arrives` is NOT a contradiction — the state machine
/// forgets, it does not accumulate.
#[test]
fn departs_then_arrives_then_acts_is_clean() {
    let c = departs_then_arrives_then_acts_is_clean_campaign();
    assert!(
        !codes(&c).contains(&"DW0485".to_string()),
        "{:#?}",
        branch::check_branches(&c)
    );
}

// --- DW0485 over every legal order ----------------------------------------

/// The hold branch with its gate **sealed** when the watch is stood, and an
/// optional strand — `quest/look-back`, opened by the fork and on no ending's
/// dependency chain — whose one beat walks back in through that gate. The
/// exported order walks the strand before the watch, because a ready strand is
/// walked before a quest that can end the delve; a player may just as well
/// stand the watch first and go back afterwards. `then` patches `quests.json`
/// once the strand is in it.
fn optional_strand_fixture(then: impl Fn(&mut Value)) -> Campaign {
    campaign_with(|plan, quests, _| {
        add_optional_strand(plan, quests);
        then(quests);
    })
}

/// The documents half of [`optional_strand_fixture`].
fn add_optional_strand(plan: &mut Value, quests: &mut Value) {
    plan["content"]["quests"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "act": 2,
            "area": "area/keep",
            "depends_on": ["quest/decide"],
            "goal": "Go back for a last look at the gate on the way out.",
            "activity": ["talk"],
            "id": "quest/look-back",
            "mandatory": false,
            "npcs": []
        }));
    quest(quests, "quest/hold")["on_objective_complete"]["obj/watch"][0] = serde_json::json!({
        "type": "close-gate",
        "anchor": "anchor/door",
        "happening": {
            "verb": "seals",
            "text": "The Keeper drops the bar across the gate for good.",
            "subject": "anchor/door"
        }
    });
    let objective = serde_json::json!({
        "id": "obj/look-back",
        "type": "reach-anchor",
        "anchor": "anchor/exit",
        "radius": 2,
        "happening": {
            "verb": "arrives",
            "text": "The party slips back in through the gate for one last look.",
            "subject": "anchor/door"
        }
    });
    quests["content"]["quests"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "id": "quest/look-back",
            "trigger": {"type": "quest-complete", "quest": "quest/decide"},
            "happening": {
                "verb": "learns",
                "text": "The party turns back toward the gate."
            },
            "objectives": [objective],
            "on_objective_complete": {},
            "on_complete": []
        }));
}

/// **The reproduction.** The exported order does not show the clash — the
/// one-order rule run over every realized branch finds nothing — and the proof
/// still refuses it, naming the legal order that does show it.
#[test]
fn an_optional_beat_after_a_seal_in_a_non_exported_order_is_dw0485() {
    let c = optional_strand_fixture(|_| {});
    let realized = branch::realize(&c);
    let hold = realized
        .iter()
        .find(|r| r.branch.id == "branch/hold")
        .expect("the hold branch is realized");
    let exported: Vec<&str> = hold.path.iter().map(|s| s.objective.as_str()).collect();
    let at = |o: &str| exported.iter().position(|x| *x == o).unwrap();
    assert!(
        at("obj/look-back") < at("obj/watch"),
        "the premise: the exported order walks the strand before the seal: {exported:?}"
    );
    let mut one_order = Vec::new();
    for r in &realized {
        branch::check_contradictions(r, &mut one_order);
    }
    assert!(
        one_order.is_empty(),
        "the exported order shows no clash: {one_order:#?}"
    );
    let d = find(branch::check_branches(&c), "DW0485");
    assert!(d.message.contains("after it is sealed"), "{}", d.message);
    assert!(d.message.contains("branch/hold"), "{}", d.message);
    assert!(d.message.contains("anchor/door"), "{}", d.message);
    assert!(
        d.message
            .contains("other than the exported one (`obj/decide` → `obj/watch` → `obj/look-back`)"),
        "the refusal names the order that shows it: {}",
        d.message
    );
}

/// The same strand, closed before the seal by a flag the strand itself sets
/// and the watch requires: no legal order walks it after the seal, so nothing
/// is refused. What the every-order walk builds is a legal order, never merely
/// a permutation of the path.
#[test]
fn a_strand_the_seal_must_wait_for_is_clean() {
    let c = a_strand_the_seal_must_wait_for_is_clean_campaign();
    let (diags, bind) = branch::check_branches_bound(&c);
    assert!(!diags.iter().any(|d| d.code == "DW0485"), "{diags:#?}");
    // Not vacuous: the search walked the same steps as in the open strand,
    // where it refuses, and found here no legal order that walks the strand
    // after the seal.
    let (_, open) = branch::check_branches_bound(&optional_strand_fixture(|_| {}));
    assert_eq!(bind.steps, open.steps, "{bind:?} vs {open:?}");
    assert!(bind.states > 0, "{bind:?}");
    assert!(
        open.refused > 0 && bind.refused == 0,
        "{bind:?} vs {open:?}"
    );
    assert_eq!(bind.unproven, 0, "{bind:?}");
}

/// The clash carried by the strand's OWN line: `quest/look-back` is about the
/// gate, its two beats are not, and they may be walked in either order. The
/// quest's line plays at whichever of them comes first, so the order that shows
/// the clash has neither of them before the seal: walking either early carries
/// the quest's line before the seal.
#[test]
fn a_strand_s_own_line_after_a_seal_is_dw0485() {
    let c = a_strand_s_own_line_after_a_seal_is_dw0485_campaign();
    let d = find(branch::check_branches(&c), "DW0485");
    assert!(d.message.contains("after it is sealed"), "{}", d.message);
    assert!(d.message.contains("quest/look-back"), "{}", d.message);
}

/// The clash carried by the strand's COMPLETION: its two beats say nothing
/// about the gate and may be walked in either order, and finishing the strand
/// walks the party back in through it. Whichever beat is walked last completes the
/// strand, so the order that shows the clash walks the other one first —
/// after the seal as well as before it.
#[test]
fn a_strand_completed_after_a_seal_is_dw0485() {
    let c = a_strand_completed_after_a_seal_is_dw0485_campaign();
    let d = find(branch::check_branches(&c), "DW0485");
    assert!(d.message.contains("after it is sealed"), "{}", d.message);
    assert!(d.message.contains("/on_complete/0"), "{}", d.message);
}

// --- DW0485 where gates do not only grow with the state ---------------------

/// [`optional_strand_fixture`], plus a second optional strand, `quest/linger`,
/// also opened by the fork: one beat that says nothing about the gate and sets
/// `flag/lingered`. On its own it changes nothing about the clash; each test
/// below makes the first strand read the flag, so that walking `quest/linger`
/// early closes the clash an order that skips it shows. `then` patches
/// `quests.json` once both strands are in it.
fn lingering_strand_fixture(then: impl Fn(&mut Value)) -> Campaign {
    campaign_with(|plan, quests, _| {
        add_optional_strand(plan, quests);
        plan["content"]["quests"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "act": 2,
                "area": "area/keep",
                "depends_on": ["quest/decide"],
                "goal": "Linger in the yard a while.",
                "activity": ["talk"],
                "id": "quest/linger",
                "mandatory": false,
                "npcs": []
            }));
        quests["content"]["quests"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "id": "quest/linger",
                "trigger": {"type": "quest-complete", "quest": "quest/decide"},
                "happening": {"verb": "learns", "text": "The party lingers in the yard."},
                "objectives": [{
                    "id": "obj/linger",
                    "type": "reach-anchor",
                    "anchor": "anchor/exit",
                    "radius": 2,
                    "happening": {"verb": "learns", "text": "Nothing moves on the moor."}
                }],
                "on_objective_complete": {
                    "obj/linger": [{"type": "set-flag", "flag": "flag/lingered"}]
                },
                "on_complete": []
            }));
        then(quests);
    })
}

/// The objectives of the play order a beyond-the-exported-order `DW0485`
/// prints, in walk order.
fn printed_order(message: &str) -> Vec<String> {
    let start = message
        .find("other than the exported one (")
        .unwrap_or_else(|| panic!("no order printed: {message}"))
        + "other than the exported one (".len();
    let end = start + message[start..].find(')').unwrap();
    message[start..end]
        .split(" → ")
        .map(|s| s.trim_matches('`').to_string())
        .collect()
}

/// No realized branch's exported order shows a clash: whatever `DW0485` finds
/// is found beyond it.
fn assert_exported_orders_clean(c: &Campaign) {
    let mut one_order = Vec::new();
    for r in &branch::realize(c) {
        branch::check_contradictions(r, &mut one_order);
    }
    assert!(
        one_order.is_empty(),
        "the premise: no exported order shows a clash: {one_order:#?}"
    );
}

/// The strand's beat **forbids** the flag the other strand sets. Every order
/// that walks `quest/linger` first shuts the strand, and an order that takes
/// every step it can before the seal walks it first; a player who stands the
/// watch, goes back, and never lingers walks in through a sealed gate.
#[test]
fn a_strand_another_strand_shuts_is_judged_in_the_order_that_skips_it() {
    let c = a_strand_another_strand_shuts_is_judged_in_the_order_that_skips_it_campaign();
    assert_exported_orders_clean(&c);
    let diags = branch::check_branches(&c);
    let d = find(diags, "DW0485");
    assert!(d.message.contains("after it is sealed"), "{}", d.message);
    assert!(d.message.contains("branch/hold"), "{}", d.message);
    let order = printed_order(&d.message);
    let at = |o: &str| order.iter().position(|x| x == o);
    assert!(
        at("obj/watch") < at("obj/look-back")
            && at("obj/linger").is_none_or(|l| l > at("obj/look-back").unwrap()),
        "the printed order walks the seal, then the strand, without lingering first: {order:?}"
    );
}

/// The same shape carried by a beat's **effect gate**: the strand's beat says
/// nothing about the gate itself, and its bundle walks the party in through it
/// unless `flag/lingered` holds.
#[test]
fn a_beat_a_forbids_gate_withholds_in_one_order_is_judged_in_the_order_that_fires_it() {
    let c =
        a_beat_a_forbids_gate_withholds_in_one_order_is_judged_in_the_order_that_fires_it_campaign(
        );
    assert_exported_orders_clean(&c);
    let d = find(branch::check_branches(&c), "DW0485");
    assert!(d.message.contains("after it is sealed"), "{}", d.message);
    assert!(
        d.message.contains("/on_objective_complete/obj/look-back/0"),
        "{}",
        d.message
    );
}

/// A **requires** gate, no `forbids_flags` anywhere: the strand's bundle lifts
/// the bar before it walks in, but only once `flag/lingered` holds. An order
/// that has done everything it can before the seal has lingered, so the bar is
/// lifted and nothing clashes; an order that never lingers walks in through the
/// sealed gate.
#[test]
fn a_beat_a_requires_gate_opens_in_one_order_is_judged_in_the_order_that_does_not() {
    let c =
        a_beat_a_requires_gate_opens_in_one_order_is_judged_in_the_order_that_does_not_campaign();
    assert_exported_orders_clean(&c);
    let d = find(branch::check_branches(&c), "DW0485");
    assert!(d.message.contains("after it is sealed"), "{}", d.message);
    assert!(
        d.message.contains("/on_objective_complete/obj/look-back/1"),
        "{}",
        d.message
    );
    let order = printed_order(&d.message);
    assert!(
        !order.iter().any(|o| o == "obj/linger"),
        "the order that shows it never lingers: {order:?}"
    );
}

/// The control: the seal itself waits for the lingering (`obj/watch` requires
/// `flag/lingered`), so every legal order has shut the strand before the gate is
/// sealed. A proof that judged a permutation rather than a legal order would
/// refuse this.
#[test]
fn a_strand_shut_before_the_seal_in_every_order_is_clean() {
    let c = a_strand_shut_before_the_seal_in_every_order_is_clean_campaign();
    let diags = branch::check_branches(&c);
    assert!(!diags.iter().any(|d| d.code == "DW0485"), "{diags:#?}");
}

/// **Between the two beats, everything that can be walked is walked.** The seal
/// is a quest of its own (`quest/bar`); it opens `quest/yard`, whose two beats
/// say nothing about the gate, and whose completion walks the party back in
/// through it; and `quest/lift`, whose beat lifts the bar. The exported order
/// lifts the bar before `quest/yard` completes. Another legal order walks
/// both of `quest/yard`'s beats straight after the seal and never lifts it.
///
/// Neither of `quest/yard`'s beats is legal before the seal, so whichever is
/// the later beat of the clash, the other is legal only after the seal too —
/// and the clash plays only when both are done. A proof that stopped walking
/// the steps between the two beats as soon as the later one was legal would
/// take the later beat with its quest still open, and see nothing.
#[test]
fn a_clash_that_needs_every_step_between_its_beats_is_dw0485() {
    let c = a_clash_that_needs_every_step_between_its_beats_is_dw0485_campaign();
    assert_exported_orders_clean(&c);
    let d = find(branch::check_branches(&c), "DW0485");
    assert!(d.message.contains("after it is sealed"), "{}", d.message);
    assert!(d.message.contains("/on_complete/0"), "{}", d.message);
    let order = printed_order(&d.message);
    let at = |o: &str| order.iter().position(|x| x == o);
    assert!(
        at("obj/bar") < at("obj/yard-a")
            && at("obj/bar") < at("obj/yard-b")
            && at("obj/lift").is_none(),
        "the order seals, walks both of the strand's beats, and never lifts the bar: {order:?}"
    );
}

// --- DW0485 where a story world performs a deed in front of a step ----------

/// The keep with a landing to hold and a story world that loses it (the
/// town's raid, reduced), on the hold branch. Two strands open once the party
/// has decided: `quest/landing` declares the objectives `landing` in that
/// order, and `quest/strolls` the rest of `obj/stroll-1`, `obj/stroll-2`.
/// `obj/muster` musters the party at the quay; `obj/stand` holds the landing
/// (`dies`, about `anchor/exit`) and is legal only once the party has
/// mustered, and `obj/muster` sets `flag/mustered`; a stroll is a beat a
/// player may walk at any point after the decision, and `obj/stroll-1` sets
/// `flag/strolled`. `trigger/lose` is the elective loss: it fails `obj/stand`,
/// so the world that performs it does so in front of `obj/stand`'s step (its
/// entry deed), gated on `gate` when one is named. `lines` names each objective whose own line `arrives` about
/// `anchor/exit` — the landing coming ashore.
fn landing_fixture(landing: &[&str], gate: Option<&str>, lines: &[&str]) -> Campaign {
    landing_fixture_with(landing, gate, lines, false)
}

/// [`landing_fixture`], the loss `forbids` its own `flag/lost` when
/// `forbids_own` — the town's raid as written: `trigger/cake-taken` forbids the
/// `flag/cake-taken` it sets, so it fires once.
fn landing_fixture_with(
    landing: &[&str],
    gate: Option<&str>,
    lines: &[&str],
    forbids_own: bool,
) -> Campaign {
    campaign_with(|plan, quests, _| {
        let objective = |id: &str| {
            let mut o = serde_json::json!({
                "id": id, "type": "reach-anchor", "anchor": "anchor/exit", "radius": 2,
                "requires_flags": ["flag/wait"],
                "happening": { "verb": "learns", "text": "The party walks the wall." }
            });
            if id == "obj/stand" {
                o["after"] = serde_json::json!(["obj/muster"]);
                o["happening"] = serde_json::json!({
                    "verb": "dies", "subject": "anchor/exit",
                    "text": "The landing is driven back to its boats."
                });
            }
            if lines.contains(&id) {
                o["happening"] = serde_json::json!({
                    "verb": "arrives", "subject": "anchor/exit",
                    "text": "The raiders come ashore at the quay."
                });
            }
            o
        };
        let strolls: Vec<&str> = ["obj/stroll-1", "obj/stroll-2"]
            .into_iter()
            .filter(|o| !landing.contains(o))
            .collect();
        for (id, objectives) in [("quest/landing", landing), ("quest/strolls", &strolls[..])] {
            plan_quest(plan, id, &["quest/decide"]);
            let mut done = serde_json::Map::new();
            if objectives.contains(&"obj/stand") {
                done.insert(
                    "obj/stand".into(),
                    serde_json::json!([{ "type": "set-flag", "flag": "flag/kept" }]),
                );
            }
            if objectives.contains(&"obj/muster") {
                done.insert(
                    "obj/muster".into(),
                    serde_json::json!([{ "type": "set-flag", "flag": "flag/mustered" }]),
                );
            }
            if objectives.contains(&"obj/stroll-1") {
                done.insert(
                    "obj/stroll-1".into(),
                    serde_json::json!([{ "type": "set-flag", "flag": "flag/strolled" }]),
                );
            }
            quests["content"]["quests"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({
                    "id": id,
                    "trigger": { "type": "quest-complete", "quest": "quest/decide" },
                    "happening": { "verb": "learns", "text": "The night goes on." },
                    "objectives": objectives.iter().map(|o| objective(o)).collect::<Vec<_>>(),
                    "on_objective_complete": done,
                    "on_complete": []
                }));
        }
        let mut lose = serde_json::json!({
            "id": "trigger/lose", "at": "anchor/door", "on": { "on": "use" },
            "effects": [
                { "type": "set-flag", "flag": "flag/lost" },
                { "type": "fail-objective", "objective": "obj/stand" }
            ]
        });
        if let Some(gate) = gate {
            lose["requires_flags"] = serde_json::json!([gate]);
        }
        if forbids_own {
            lose["forbids_flags"] = serde_json::json!(["flag/lost"]);
        }
        quests["content"]["triggers"] = serde_json::json!([lose]);
        plan["content"]["branch_points"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "id": "branch-point/landing", "opens_at": "quest/decide",
                "forks_on": ["flag/kept", "flag/lost"],
                "branches": [
                    { "id": "branch/kept", "flags": ["flag/kept"], "leads_to": "quest/decide" },
                    { "id": "branch/lost", "flags": ["flag/lost"], "leads_to": "quest/decide" }
                ]
            }));
    })
}

/// The branch of [`landing_fixture`] that holds the gate and loses the
/// landing, realized: its exported path and the steps its world performs a
/// deed in front of, by objective.
fn lost_landing(c: &Campaign) -> (Vec<String>, Vec<(String, String)>) {
    let flow = delvec::compiler::flow::Flow::new(c);
    let r = branch::realize(c)
        .into_iter()
        .find(|r| r.branch.id == "branch/hold+branch/lost")
        .expect("the hold+lost branch is enumerated");
    let w = r
        .world
        .expect("a story world realizes the hold+lost branch");
    let pt = flow.story_playthrough(w);
    let ctx = pt.story.clone().expect("a story world's playthrough");
    let path: Vec<String> = pt.steps.iter().map(|s| s.objective.clone()).collect();
    let deeds = ctx
        .deeds
        .iter()
        .map(|(i, a)| (path[*i].clone(), a.clone()))
        .collect();
    (path, deeds)
}

/// Where `o` stands on `path`.
fn at(path: &[String], o: &str) -> usize {
    path.iter()
        .position(|x| x == o)
        .unwrap_or_else(|| panic!("{o} on {path:?}"))
}

/// The `DW0485` refusals of the hold+lost branch.
fn lost_landing_refusals(c: &Campaign) -> Vec<String> {
    branch::check_branches(c)
        .into_iter()
        .filter(|d| d.code == "DW0485" && d.message.contains("branch/hold+branch/lost"))
        .map(|d| d.message)
        .collect()
}

/// The campaign [`a_deed_is_performed_in_front_of_its_own_step_in_every_order`]
/// judges: the landing comes ashore at the muster, the loss can be performed
/// only once the party has mustered and is performed in front of `obj/stand`,
/// and the strolls are a strand of their own.
fn a_deed_in_front_of_its_own_step_campaign() -> Campaign {
    landing_fixture(
        &["obj/muster", "obj/stand"],
        Some("flag/mustered"),
        &["obj/muster"],
    )
}

/// **The town's raid, reduced.** The world that loses the landing performs the
/// loss in front of `obj/stand`, and the exported path walks the strolls
/// after it. A player may walk the strolls first; an order that does reaches
/// the stand's place in the count before the party has mustered. The deed
/// belongs to `obj/stand`'s step, not to whatever step comes at that count,
/// and it can be performed only once the party has mustered: performed at the
/// count it would fail the landing before it has come ashore, open `obj/stand`
/// early (a failed objective's step is no act of the party's), and print its
/// `dies` before the muster's `arrives`. Every order performs it in front of
/// `obj/stand`, after the muster, so no order shows a clash.
#[test]
fn a_deed_is_performed_in_front_of_its_own_step_in_every_order() {
    let c = a_deed_in_front_of_its_own_step_campaign();
    let (path, deeds) = lost_landing(&c);
    assert!(
        at(&path, "obj/stand") < at(&path, "obj/stroll-1")
            && at(&path, "obj/stand") < at(&path, "obj/stroll-2"),
        "the premise: the strolls come after the stand on the exported path: {path:?}"
    );
    assert_eq!(
        deeds,
        [("obj/stand".to_string(), "trigger/lose".to_string())],
        "the premise: the loss is performed in front of the stand"
    );
    let refused = lost_landing_refusals(&c);
    assert!(refused.is_empty(), "{refused:#?}");
    // Not vacuous: the search walked the branches to the end.
    let (_, bind) = branch::check_branches_bound(&c);
    assert!(bind.states > 0 && bind.unproven == 0, "{bind:?}");
}

/// The campaign [`a_deed_whose_gate_is_shut_where_its_step_stands_is_not_performed`]
/// judges: the loss needs the first stroll walked, and that stroll brings the
/// landing ashore.
fn a_deed_whose_gate_is_shut_campaign() -> Campaign {
    landing_fixture(
        &["obj/muster", "obj/stroll-1", "obj/stand"],
        Some("flag/strolled"),
        &["obj/stroll-1"],
    )
}

/// **A deed honours its own gate.** The loss is performed in front of
/// `obj/stand` once `obj/stroll-1` has set `flag/strolled`, and that stroll
/// brings the landing ashore. An order that musters and goes straight to the
/// stand reaches it with the loss's gate shut: this world cannot perform the
/// loss there, so that order is not one this world plays, and performing the
/// loss anyway would print the landing's `dies` before its `arrives`. Every
/// order this world plays walks the first stroll before the stand.
#[test]
fn a_deed_whose_gate_is_shut_where_its_step_stands_is_not_performed() {
    let c = a_deed_whose_gate_is_shut_campaign();
    let (path, deeds) = lost_landing(&c);
    assert!(
        at(&path, "obj/stroll-1") < at(&path, "obj/stand"),
        "the premise: the exported path strolls before the stand: {path:?}"
    );
    assert_eq!(
        deeds,
        [("obj/stand".to_string(), "trigger/lose".to_string())]
    );
    let refused = lost_landing_refusals(&c);
    assert!(refused.is_empty(), "{refused:#?}");
}

/// The campaign [`a_deed_in_its_place_still_shows_a_true_clash`] judges.
fn a_deed_in_its_place_with_a_true_clash_campaign() -> Campaign {
    landing_fixture(
        &["obj/muster", "obj/stroll-1", "obj/stand"],
        None,
        &["obj/stroll-1"],
    )
}

/// **A true clash still refuses.** The same documents with the loss ungated:
/// a player may muster, lose the landing in front of `obj/stand` and only then
/// walk the stroll that brings it ashore. That order is one this world plays,
/// the deed in its own place, and it shows the clash; the exported order does
/// not.
#[test]
fn a_deed_in_its_place_still_shows_a_true_clash() {
    let c = a_deed_in_its_place_with_a_true_clash_campaign();
    let (_, deeds) = lost_landing(&c);
    assert_eq!(
        deeds,
        [("obj/stand".to_string(), "trigger/lose".to_string())]
    );
    let refused = lost_landing_refusals(&c);
    assert_eq!(refused.len(), 1, "{refused:#?}");
    let m = &refused[0];
    assert!(m.contains("`anchor/exit` acts after it dies"), "{m}");
    assert!(
        m.contains("[dies] obj/stand") && m.contains("[arrives] obj/stroll-1"),
        "{m}"
    );
    let order = printed_order(m);
    assert!(
        at(&order, "obj/muster") < at(&order, "obj/stand")
            && at(&order, "obj/stand") < at(&order, "obj/stroll-1"),
        "the order performs the deed in its place, the stroll after it: {order:?}"
    );
}

/// The campaign [`a_deed_that_forbids_its_own_flag_is_performed_where_it_stands`]
/// judges: [`a_deed_in_front_of_its_own_step_campaign`], the loss forbidding the
/// flag it sets.
fn a_deed_that_forbids_its_own_flag_campaign() -> Campaign {
    landing_fixture_with(
        &["obj/muster", "obj/stand"],
        Some("flag/mustered"),
        &["obj/muster"],
        true,
    )
}

/// **A deed writes nothing before it is performed, from the world's first
/// state on.** The loss forbids `flag/lost`, the flag it sets, so it fires
/// once. The story run places it in front of `obj/stand` with its gate open;
/// the replay of the world's exported path (`DW0204`) and every order the
/// branch search walks start from the same first state, so `flag/lost` is not
/// held there either and the deed is performed where it stands.
#[test]
fn a_deed_that_forbids_its_own_flag_is_performed_where_it_stands() {
    let c = a_deed_that_forbids_its_own_flag_campaign();
    let (_, deeds) = lost_landing(&c);
    assert_eq!(
        deeds,
        [("obj/stand".to_string(), "trigger/lose".to_string())],
        "the premise: the story run performs the loss in front of the stand"
    );
    let prefabs =
        delvec::compiler::registry::PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let bad: Vec<_> = delvec::compiler::analyze::analyze_campaign(&c, &prefabs)
        .into_iter()
        .filter(|d| d.code == "DW0204")
        .map(|d| d.message)
        .collect();
    assert!(bad.is_empty(), "{bad:#?}");
    let refused = lost_landing_refusals(&c);
    assert!(refused.is_empty(), "{refused:#?}");
    let (_, bind) = branch::check_branches_bound(&c);
    assert!(bind.states > 0 && bind.unproven == 0, "{bind:?}");
}

/// The campaign [`a_true_clash_behind_a_deed_that_forbids_its_own_flag_still_refuses`]
/// judges: [`a_deed_in_its_place_with_a_true_clash_campaign`], the loss
/// forbidding the flag it sets.
fn a_true_clash_behind_a_deed_that_forbids_its_own_flag_campaign() -> Campaign {
    landing_fixture_with(
        &["obj/muster", "obj/stroll-1", "obj/stand"],
        None,
        &["obj/stroll-1"],
        true,
    )
}

/// **The search walks past such a deed.** [`a_deed_in_its_place_still_shows_a_true_clash`]
/// with the loss forbidding its own flag: an order that musters, loses the
/// landing in front of `obj/stand` and only then strolls still shows the
/// clash. A walk that read the deed's own flag as held from the start stopped
/// at the deed, and the clash went unseen.
#[test]
fn a_true_clash_behind_a_deed_that_forbids_its_own_flag_still_refuses() {
    let c = a_true_clash_behind_a_deed_that_forbids_its_own_flag_campaign();
    let refused = lost_landing_refusals(&c);
    assert_eq!(refused.len(), 1, "{refused:#?}");
    let m = &refused[0];
    assert!(
        m.contains("[dies] obj/stand") && m.contains("[arrives] obj/stroll-1"),
        "{m}"
    );
}

/// **One rule places a story world's deed and says whether it fires**, and
/// every reader of the world's path agrees with it. Per story world of every
/// fixture in this file: the story run that exports the path (its deeds, what
/// it fired, the first step it calls incoherent), the replay `DW0204` makes of
/// it (`Flow::replay`), and the walk the branch search starts every order from
/// (`Flow::walk_for`, `Walk::legal`/`Walk::take`) taking the exported order.
/// Red on any difference. Bound: worlds and deeds counted.
#[test]
fn the_story_run_the_replay_and_the_order_walk_agree_on_every_deed() {
    use delvec::compiler::flow::Flow;
    let (mut worlds, mut deeds, mut bad) = (0usize, 0usize, Vec::new());
    for (name, c) in &contradiction_fixtures() {
        let flow = Flow::new(c);
        for (k, run) in flow.story().runs.iter().enumerate() {
            worlds += 1;
            let pt = &run.playthrough;
            let ctx = pt.story.clone().expect("a story world's playthrough");
            let mut w = flow.walk_for(pt);
            let mut walked: Vec<(usize, String)> = Vec::new();
            let mut fired: std::collections::BTreeSet<String> = Default::default();
            let mut first_illegal = None;
            for (i, step) in pt.steps.iter().enumerate() {
                if !w.legal(step) {
                    first_illegal.get_or_insert(i + 1);
                }
                let j = w.take(step);
                for (a, f) in &j.deeds {
                    walked.push((i, a.clone()));
                    fired.extend(f.iter().cloned());
                }
                fired.extend(j.fired.iter().cloned());
            }
            let incoherent = run.incoherent.as_ref().map(|why| {
                why.strip_prefix("step ")
                    .and_then(|r| r.split(' ').next())
                    .and_then(|n| n.parse::<usize>().ok())
                    .expect("`step N (...)`")
            });
            let replayed = flow.replay(pt).err().map(|f| f.position);
            deeds += ctx.deeds.len();
            if walked != ctx.deeds {
                bad.push(format!(
                    "{name} world/{k}: run {:?}, walk {walked:?}",
                    ctx.deeds
                ));
            }
            if fired != run.fired {
                bad.push(format!(
                    "{name} world/{k}: fired by the run only {:?}, by the walk only {:?}",
                    run.fired.difference(&fired).collect::<Vec<_>>(),
                    fired.difference(&run.fired).collect::<Vec<_>>()
                ));
            }
            if first_illegal != incoherent {
                bad.push(format!(
                    "{name} world/{k}: the run's first incoherent step {incoherent:?}, the walk's {first_illegal:?}"
                ));
            }
            if replayed.is_some_and(|p| Some(p) != first_illegal && first_illegal.is_some()) {
                bad.push(format!(
                    "{name} world/{k}: the replay refuses at {replayed:?}, the walk at {first_illegal:?}"
                ));
            }
            if first_illegal.is_some() && replayed.is_none() {
                bad.push(format!(
                    "{name} world/{k}: the walk refuses, the replay does not"
                ));
            }
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
    assert!(worlds > 50 && deeds > 5, "worlds {worlds}, deeds {deeds}");
}

// --- the DW0485 fixtures, by name ----------------------------------------

/// The campaign [`dies_then_acts_on_one_branch_is_dw0485`] judges.
fn dies_then_acts_on_one_branch_is_dw0485_campaign() -> Campaign {
    campaign_with(|_, quests, _| {
        quest(quests, "quest/decide")["objectives"][0]["happening"] = serde_json::json!({
            "verb": "dies",
            "text": "The Keeper is dragged into the moor before the party can speak.",
            "subject": "npc/keeper"
        });
    })
}

/// The campaign [`seals_then_used_is_dw0485`] judges.
fn seals_then_used_is_dw0485_campaign() -> Campaign {
    campaign_with(|_, quests, _| {
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
    })
}

/// The campaign [`loses_then_spent_again_is_dw0485`] judges.
fn loses_then_spent_again_is_dw0485_campaign() -> Campaign {
    campaign_with(|_, quests, _| {
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
    })
}

/// The campaign [`departs_then_arrives_then_acts_is_clean`] judges.
fn departs_then_arrives_then_acts_is_clean_campaign() -> Campaign {
    campaign_with(|_, quests, _| {
        quest(quests, "quest/decide")["objectives"][0]["happening"] = serde_json::json!({
            "verb": "departs", "text": "The Keeper steps inside.", "subject": "npc/keeper"
        });
        quest(quests, "quest/hold")["happening"] = serde_json::json!({
            "verb": "arrives", "text": "The Keeper comes back out.", "subject": "npc/keeper"
        });
    })
}

/// The campaign [`a_strand_the_seal_must_wait_for_is_clean`] judges.
fn a_strand_the_seal_must_wait_for_is_clean_campaign() -> Campaign {
    optional_strand_fixture(|quests| {
        quest(quests, "quest/look-back")["on_complete"] = serde_json::json!([
            {"type": "set-flag", "flag": "flag/looked-back"}
        ]);
        quest(quests, "quest/hold")["objectives"][0]["requires_flags"] =
            serde_json::json!(["flag/wait", "flag/looked-back"]);
    })
}

/// The campaign [`a_strand_s_own_line_after_a_seal_is_dw0485`] judges.
fn a_strand_s_own_line_after_a_seal_is_dw0485_campaign() -> Campaign {
    optional_strand_fixture(|quests| {
        let q = quest(quests, "quest/look-back");
        q["happening"] = serde_json::json!({
            "verb": "arrives",
            "text": "The party goes back in through the gate.",
            "subject": "anchor/door"
        });
        q["objectives"][0]["happening"] = serde_json::json!({
            "verb": "learns",
            "text": "The yard is empty."
        });
        q["objectives"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "id": "obj/look-up",
                "type": "reach-anchor",
                "anchor": "anchor/exit",
                "radius": 2,
                "happening": {"verb": "learns", "text": "The wall-walk is empty too."}
            }));
    })
}

/// The campaign [`a_strand_completed_after_a_seal_is_dw0485`] judges.
fn a_strand_completed_after_a_seal_is_dw0485_campaign() -> Campaign {
    optional_strand_fixture(|quests| {
        let q = quest(quests, "quest/look-back");
        q["objectives"][0]["happening"] = serde_json::json!({
            "verb": "learns",
            "text": "The yard is empty."
        });
        q["objectives"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "id": "obj/look-up",
                "type": "reach-anchor",
                "anchor": "anchor/exit",
                "radius": 2,
                "happening": {"verb": "learns", "text": "The wall-walk is empty too."}
            }));
        q["on_complete"] = serde_json::json!([{
            "type": "narrate",
            "text": "Nothing left to see out here.",
            "happening": {
                "verb": "arrives",
                "text": "The party comes back in through the gate.",
                "subject": "anchor/door"
            }
        }]);
    })
}

/// The campaign [`a_strand_another_strand_shuts_is_judged_in_the_order_that_skips_it`] judges.
fn a_strand_another_strand_shuts_is_judged_in_the_order_that_skips_it_campaign() -> Campaign {
    lingering_strand_fixture(|quests| {
        quest(quests, "quest/look-back")["objectives"][0]["forbids_flags"] =
            serde_json::json!(["flag/lingered"]);
    })
}

/// The campaign [`a_beat_a_forbids_gate_withholds_in_one_order_is_judged_in_the_order_that_fires_it`] judges.
fn a_beat_a_forbids_gate_withholds_in_one_order_is_judged_in_the_order_that_fires_it_campaign()
-> Campaign {
    lingering_strand_fixture(|quests| {
        let q = quest(quests, "quest/look-back");
        q["objectives"][0]["happening"] =
            serde_json::json!({"verb": "learns", "text": "The yard is empty."});
        q["on_objective_complete"]["obj/look-back"] = serde_json::json!([{
            "type": "narrate",
            "text": "You slip back in.",
            "when": {"forbids_flags": ["flag/lingered"]},
            "happening": {
                "verb": "arrives",
                "text": "The party slips back in through the gate.",
                "subject": "anchor/door"
            }
        }]);
    })
}

/// The campaign [`a_beat_a_requires_gate_opens_in_one_order_is_judged_in_the_order_that_does_not`] judges.
fn a_beat_a_requires_gate_opens_in_one_order_is_judged_in_the_order_that_does_not_campaign()
-> Campaign {
    lingering_strand_fixture(|quests| {
        let q = quest(quests, "quest/look-back");
        q["objectives"][0]["happening"] =
            serde_json::json!({"verb": "learns", "text": "The yard is empty."});
        q["on_objective_complete"]["obj/look-back"] = serde_json::json!([
            {
                "type": "open-gate",
                "anchor": "anchor/door",
                "when": {"requires_flags": ["flag/lingered"]},
                "happening": {
                    "verb": "opens",
                    "text": "Having lingered, the party finds the bar loose and lifts it.",
                    "subject": "anchor/door"
                }
            },
            {
                "type": "narrate",
                "text": "You slip back in.",
                "happening": {
                    "verb": "arrives",
                    "text": "The party slips back in through the gate.",
                    "subject": "anchor/door"
                }
            }
        ]);
    })
}

/// The campaign [`a_strand_shut_before_the_seal_in_every_order_is_clean`] judges.
fn a_strand_shut_before_the_seal_in_every_order_is_clean_campaign() -> Campaign {
    lingering_strand_fixture(|quests| {
        quest(quests, "quest/look-back")["objectives"][0]["forbids_flags"] =
            serde_json::json!(["flag/lingered"]);
        quest(quests, "quest/hold")["objectives"][0]["requires_flags"] =
            serde_json::json!(["flag/wait", "flag/lingered"]);
    })
}

/// The campaign [`a_clash_that_needs_every_step_between_its_beats_is_dw0485`] judges.
fn a_clash_that_needs_every_step_between_its_beats_is_dw0485_campaign() -> Campaign {
    campaign_with(|plan, quests, _| {
        let pq = plan["content"]["quests"].as_array_mut().unwrap();
        for (id, deps) in [
            ("quest/bar", vec!["quest/decide"]),
            ("quest/lift", vec!["quest/bar"]),
            ("quest/yard", vec!["quest/bar"]),
        ] {
            pq.push(serde_json::json!({
                "act": 2,
                "area": "area/keep",
                "depends_on": deps,
                "goal": "A beat of the night.",
                "activity": ["talk"],
                "id": id,
                "mandatory": true,
                "npcs": []
            }));
        }
        for q in pq.iter_mut() {
            if q["id"] == "quest/hold" {
                q["depends_on"] =
                    serde_json::json!(["quest/decide", "quest/bolt", "quest/lift", "quest/yard"]);
            }
        }
        let beat = |id: &str, text: &str| {
            serde_json::json!({
                "id": id,
                "type": "reach-anchor",
                "anchor": "anchor/exit",
                "radius": 2,
                "happening": {"verb": "learns", "text": text}
            })
        };
        let qs = quests["content"]["quests"].as_array_mut().unwrap();
        qs.push(serde_json::json!({
            "id": "quest/bar",
            "trigger": {"type": "quest-complete", "quest": "quest/decide"},
            "happening": {"verb": "learns", "text": "Night falls on the keep."},
            "objectives": [beat("obj/bar", "The party bars the gate.")],
            "on_objective_complete": {"obj/bar": [{
                "type": "close-gate",
                "anchor": "anchor/door",
                "happening": {
                    "verb": "seals",
                    "text": "The bar drops across the gate.",
                    "subject": "anchor/door"
                }
            }]},
            "on_complete": []
        }));
        qs.push(serde_json::json!({
            "id": "quest/lift",
            "trigger": {"type": "quest-complete", "quest": "quest/bar"},
            "happening": {"verb": "learns", "text": "Someone knocks at the gate."},
            "objectives": [beat("obj/lift", "The party goes to the gate.")],
            "on_objective_complete": {"obj/lift": [{
                "type": "open-gate",
                "anchor": "anchor/door",
                "happening": {
                    "verb": "opens",
                    "text": "The party lifts the bar again.",
                    "subject": "anchor/door"
                }
            }]},
            "on_complete": []
        }));
        qs.push(serde_json::json!({
            "id": "quest/yard",
            "trigger": {"type": "quest-complete", "quest": "quest/bar"},
            "happening": {"verb": "learns", "text": "The yard needs checking."},
            "objectives": [
                beat("obj/yard-a", "The well is empty."),
                beat("obj/yard-b", "The stable is empty.")
            ],
            "on_objective_complete": {},
            "on_complete": [{
                "type": "narrate",
                "text": "Back inside.",
                "happening": {
                    "verb": "arrives",
                    "text": "The party comes back in through the gate.",
                    "subject": "anchor/door"
                }
            }]
        }));
    })
}

/// The campaign [`a_stated_subject_beats_the_effect_s_own_object`] judges.
fn a_stated_subject_beats_the_effect_s_own_object_campaign() -> Campaign {
    campaign_with(|_, quests, _| {
        quest(quests, "quest/hold")["on_objective_complete"]["obj/watch"][0] = serde_json::json!({
            "type": "close-gate",
            "anchor": "anchor/door",
            "happening": {
                "verb": "seals",
                "text": "The Keeper shuts himself in behind the bar.",
                "subject": "npc/keeper"
            }
        });
        quest(quests, "quest/hold")["objectives"][1]["happening"] = serde_json::json!({
            "verb": "departs",
            "text": "The party walks out through the gate.",
            "subject": "anchor/door"
        });
    })
}

/// Every `DW0485` fixture in this file, by the test that judges it.
fn contradiction_fixtures() -> Vec<(&'static str, Campaign)> {
    vec![
        ("green", green()),
        ("dies_then_acts_on_one_branch_is_dw0485", dies_then_acts_on_one_branch_is_dw0485_campaign()),
        ("seals_then_used_is_dw0485", seals_then_used_is_dw0485_campaign()),
        ("loses_then_spent_again_is_dw0485", loses_then_spent_again_is_dw0485_campaign()),
        ("departs_then_arrives_then_acts_is_clean", departs_then_arrives_then_acts_is_clean_campaign()),
        ("a_strand_the_seal_must_wait_for_is_clean", a_strand_the_seal_must_wait_for_is_clean_campaign()),
        ("a_strand_s_own_line_after_a_seal_is_dw0485", a_strand_s_own_line_after_a_seal_is_dw0485_campaign()),
        ("a_strand_completed_after_a_seal_is_dw0485", a_strand_completed_after_a_seal_is_dw0485_campaign()),
        ("a_strand_another_strand_shuts_is_judged_in_the_order_that_skips_it", a_strand_another_strand_shuts_is_judged_in_the_order_that_skips_it_campaign()),
        ("a_beat_a_forbids_gate_withholds_in_one_order_is_judged_in_the_order_that_fires_it", a_beat_a_forbids_gate_withholds_in_one_order_is_judged_in_the_order_that_fires_it_campaign()),
        ("a_beat_a_requires_gate_opens_in_one_order_is_judged_in_the_order_that_does_not", a_beat_a_requires_gate_opens_in_one_order_is_judged_in_the_order_that_does_not_campaign()),
        ("a_strand_shut_before_the_seal_in_every_order_is_clean", a_strand_shut_before_the_seal_in_every_order_is_clean_campaign()),
        ("a_clash_that_needs_every_step_between_its_beats_is_dw0485", a_clash_that_needs_every_step_between_its_beats_is_dw0485_campaign()),
        ("a_stated_subject_beats_the_effect_s_own_object", a_stated_subject_beats_the_effect_s_own_object_campaign()),
        ("optional_strand", optional_strand_fixture(|_| {})),
        ("derived_subject", derived_subject_fixture()),
        ("curious_strand", curious_strand_campaign()),
        ("bar_and_lift", bar_and_lift_campaign()),
        ("curious_after_lingering", curious_after_lingering_campaign()),
        ("knock_counted", knock_counted_campaign()),
        ("spooked_by_a_trigger", spooked_by_a_trigger_campaign()),
        ("knock_count", knock_count_campaign()),
        ("lift_gated_on_a_later_sibling", lift_gated_on_a_later_sibling_campaign()),
        ("a_deed_in_front_of_its_own_step", a_deed_in_front_of_its_own_step_campaign()),
        ("a_deed_whose_gate_is_shut", a_deed_whose_gate_is_shut_campaign()),
        ("a_deed_in_its_place_with_a_true_clash", a_deed_in_its_place_with_a_true_clash_campaign()),
        ("a_deed_that_forbids_its_own_flag", a_deed_that_forbids_its_own_flag_campaign()),
        (
            "a_true_clash_behind_a_deed_that_forbids_its_own_flag",
            a_true_clash_behind_a_deed_that_forbids_its_own_flag_campaign(),
        ),
        ("a_purse_two_tallies_fill", a_purse_two_tallies_fill_campaign()),
        ("a_bar_a_strand_must_be_primed_for", a_bar_a_strand_must_be_primed_for_campaign()),
        ("a_count_two_strands_write", a_count_two_strands_write_campaign()),
    ]
}

/// **Two strands that write one count, one by setting it.** On the hold
/// branch the watch seals the gate. `quest/count-set` sets `state/knocks` to
/// five and `quest/count-add` adds one, each in its own quest, and neither
/// reads the count; `quest/look-in` walks back in through the gate while the
/// count is at most five. Adding then setting leaves five, setting then adding
/// six: the two writes do not commute, so the two strands conflict although
/// neither reads what the other writes.
fn a_count_two_strands_write_campaign() -> Campaign {
    campaign_with(|plan, quests, _| {
        quest(quests, "quest/hold")["on_objective_complete"]["obj/watch"][0] = serde_json::json!({
            "type": "close-gate",
            "anchor": "anchor/door",
            "happening": {
                "verb": "seals",
                "text": "The Keeper drops the bar across the gate.",
                "subject": "anchor/door"
            }
        });
        quests["content"]["state"] = serde_json::json!([{
            "id": "state/knocks",
            "initial": 0,
            "note": "How many knocks the party has answered.",
            "scope": "party"
        }]);
        for id in ["quest/count-set", "quest/count-add", "quest/look-in"] {
            plan_quest(plan, id, &["quest/decide"]);
        }
        let qs = quests["content"]["quests"].as_array_mut().unwrap();
        qs.push(after_the_fork(
            "quest/count-set",
            "obj/count-set",
            serde_json::json!([]),
            serde_json::json!([{"type": "set-state", "state": "state/knocks", "value": 5}]),
        ));
        qs.push(after_the_fork(
            "quest/count-add",
            "obj/count-add",
            serde_json::json!([]),
            serde_json::json!([{"type": "add-state", "state": "state/knocks", "amount": 1}]),
        ));
        qs.push(after_the_fork(
            "quest/look-in",
            "obj/look-in",
            serde_json::json!([]),
            serde_json::json!([{
                "type": "narrate",
                "text": "You slip back in.",
                "when": {"requires_state": [
                    {"op": "at-most", "state": "state/knocks", "value": 5}
                ]},
                "happening": {
                    "verb": "arrives",
                    "text": "The party slips back in through the gate.",
                    "subject": "anchor/door"
                }
            }]),
        ));
    })
}

// --- the stubborn set -------------------------------------------------------

/// A quest of one objective after the fork, with `bundle` as its completion.
fn after_the_fork(id: &str, obj: &str, requires: Value, bundle: Value) -> Value {
    serde_json::json!({
        "id": id,
        "trigger": {"type": "quest-complete", "quest": "quest/decide"},
        "happening": {"verb": "learns", "text": "A beat of the night."},
        "objectives": [{
            "id": obj,
            "type": "reach-anchor",
            "anchor": "anchor/exit",
            "radius": 2,
            "requires_flags": requires,
            "happening": {"verb": "learns", "text": "The party crosses the yard."}
        }],
        "on_objective_complete": {obj: bundle},
        "on_complete": []
    })
}

/// **Two strands that fill one purse, and a gate the purse pays.** On the hold
/// branch the watch seals the gate. `quest/tally-a` and `quest/tally-b` each
/// count, then pay a coin into `state/coins` behind a gate on their own count:
/// neither strand is quiet, and neither reads anything the other writes, since
/// two additions to one datum land on one value in either order.
/// `quest/pay-the-gate` walks back in through the gate once two coins are
/// paid, which after the seal is a clash. The stubborn set walks one tally at
/// a time and still reaches every value of the purse at the gate.
fn a_purse_two_tallies_fill_campaign() -> Campaign {
    campaign_with(|plan, quests, _| {
        quest(quests, "quest/hold")["on_objective_complete"]["obj/watch"][0] = serde_json::json!({
            "type": "close-gate",
            "anchor": "anchor/door",
            "happening": {
                "verb": "seals",
                "text": "The Keeper drops the bar across the gate.",
                "subject": "anchor/door"
            }
        });
        quests["content"]["state"] = serde_json::json!([{
            "id": "state/coins",
            "initial": 0,
            "note": "The coins the party has paid into the purse.",
            "scope": "party"
        }]);
        for t in ["a", "b"] {
            let id = format!("quest/tally-{t}");
            plan_quest(plan, &id, &["quest/decide"]);
            let (count, pay) = (format!("obj/{t}-count"), format!("obj/{t}-pay"));
            let counted = format!("flag/{t}-counted");
            let mut q = after_the_fork(
                &id,
                &count,
                serde_json::json!([]),
                serde_json::json!([{"type": "set-flag", "flag": counted}]),
            );
            q["objectives"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({
                    "id": pay,
                    "type": "reach-anchor",
                    "anchor": "anchor/exit",
                    "radius": 2,
                    "after": [count],
                    "happening": {"verb": "learns", "text": "A coin goes in the purse."}
                }));
            q["on_objective_complete"][pay.as_str()] = serde_json::json!([
                {
                    "type": "narrate",
                    "text": "The coin clinks.",
                    "when": {"requires_flags": [counted]}
                },
                {"type": "add-state", "state": "state/coins", "amount": 1}
            ]);
            quests["content"]["quests"].as_array_mut().unwrap().push(q);
        }
        plan_quest(plan, "quest/pay-the-gate", &["quest/decide"]);
        quests["content"]["quests"]
            .as_array_mut()
            .unwrap()
            .push(after_the_fork(
                "quest/pay-the-gate",
                "obj/pay-the-gate",
                serde_json::json!([]),
                serde_json::json!([{
                    "type": "narrate",
                    "text": "You pay your way in.",
                    "when": {"requires_state": [
                        {"op": "at-least", "state": "state/coins", "value": 2}
                    ]},
                    "happening": {
                        "verb": "arrives",
                        "text": "The party pays its way back in through the gate.",
                        "subject": "anchor/door"
                    }
                }]),
            ));
    })
}

/// **A clash that needs a step the stubborn set must add for a step it cannot
/// take yet.** `quest/a-step-out` walks out by the postern; `quest/bar-the-exit`
/// bars the postern, and is legal only once `quest/prime` has set
/// `flag/primed`; `quest/prime` is not quiet (one of its lines is gated). Priming,
/// barring, then stepping out uses the postern after it is barred. Stepping
/// out conflicts with barring, which is not legal at the start, so a stubborn
/// set that starts from stepping out must add what can make barring legal —
/// priming — or it walks stepping out first and never sees the clash.
fn a_bar_a_strand_must_be_primed_for_campaign() -> Campaign {
    campaign_with(|plan, quests, _| {
        for id in ["quest/a-step-out", "quest/prime", "quest/bar-the-exit"] {
            plan_quest(plan, id, &["quest/decide"]);
        }
        let qs = quests["content"]["quests"].as_array_mut().unwrap();
        qs.push(after_the_fork(
            "quest/a-step-out",
            "obj/step-out",
            serde_json::json!([]),
            serde_json::json!([{
                "type": "narrate",
                "text": "The moor is quiet.",
                "happening": {
                    "verb": "arrives",
                    "text": "The party steps out by the postern and back.",
                    "subject": "anchor/exit"
                }
            }]),
        ));
        qs.push(after_the_fork(
            "quest/prime",
            "obj/prime",
            serde_json::json!([]),
            serde_json::json!([
                {"type": "set-flag", "flag": "flag/primed"},
                {
                    "type": "narrate",
                    "text": "Somebody laughs on the wall.",
                    "when": {"requires_flags": ["flag/lingered"]}
                }
            ]),
        ));
        qs.push(after_the_fork(
            "quest/bar-the-exit",
            "obj/bar-the-exit",
            serde_json::json!(["flag/primed"]),
            serde_json::json!([{
                "type": "narrate",
                "text": "The bolt goes home.",
                "happening": {
                    "verb": "seals",
                    "text": "The party bars the postern behind it.",
                    "subject": "anchor/exit"
                }
            }]),
        ));
    })
}

#[test]
fn a_stubborn_set_walks_one_strand_at_a_time_and_keeps_the_clash() {
    let c = a_purse_two_tallies_fill_campaign();
    assert_exported_orders_clean(&c);
    let (diags, bind) = branch::check_branches_bound(&c);
    let d = find(diags, "DW0485");
    assert!(d.message.contains("after it is sealed"), "{}", d.message);
    let order = printed_order(&d.message);
    let at = |o: &str| order.iter().position(|x| x == o);
    assert!(
        at("obj/a-pay") < at("obj/pay-the-gate") && at("obj/b-pay") < at("obj/pay-the-gate"),
        "both coins are paid before the gate: {order:?}"
    );
    assert!(
        bind.stubborn > 0,
        "the stubborn set left a step out: {bind:?}"
    );
}

#[test]
fn a_stubborn_set_adds_what_can_open_a_step_it_conflicts_with() {
    let c = a_bar_a_strand_must_be_primed_for_campaign();
    assert_exported_orders_clean(&c);
    let (diags, bind) = branch::check_branches_bound(&c);
    let d = find(diags, "DW0485");
    assert!(d.message.contains("after it is sealed"), "{}", d.message);
    let order = printed_order(&d.message);
    let at = |o: &str| order.iter().position(|x| x == o);
    assert!(
        at("obj/prime") < at("obj/bar-the-exit") && at("obj/bar-the-exit") < at("obj/step-out"),
        "the order primes, bars, then steps out: {order:?}"
    );
    assert!(
        bind.stubborn > 0,
        "the stubborn set left a step out: {bind:?}"
    );
}

/// **The footprints against the replay.** Over every `DW0485` fixture, at
/// every play state some legal order reaches: two legal steps the footprints
/// call free of conflict commute — each stays legal after the other, both
/// orders reach one play state, each writes the same lines — and a step that is
/// not legal stays not legal after any legal step outside its necessary
/// enabling set. Bound: both kinds of check run, many times over.
#[test]
fn the_footprints_agree_with_the_replay() {
    let (mut pairs, mut sets) = (0usize, 0usize);
    for (name, c) in &contradiction_fixtures() {
        let (p, s, bad) = branch::footprints_against_the_replay(c);
        assert!(bad.is_empty(), "{name}: {bad:#?}");
        pairs += p;
        sets += s;
    }
    assert!(pairs > 100, "pairs of free steps checked: {pairs}");
    assert!(sets > 100, "steps outside an enabling set checked: {sets}");
}

/// **Two orders that do the same steps can hold different flags.**
/// `obj/look-up`, a second beat of `quest/linger`, sets `flag/curious` only
/// while the party has not lingered;
/// the strand's beat (legal only once the party has lingered) walks in through
/// the gate only when `flag/curious` holds. Looking up, lingering, standing the
/// watch, then going back walks in through the sealed gate; lingering before
/// looking up does the same steps and never does. A search that merged the two
/// on the steps alone would judge only one of them.
fn curious_strand_campaign() -> Campaign {
    lingering_strand_fixture(|quests| {
        let q = quest(quests, "quest/look-back");
        q["objectives"][0]["happening"] =
            serde_json::json!({"verb": "learns", "text": "The yard is empty."});
        q["objectives"][0]["requires_flags"] = serde_json::json!(["flag/lingered"]);
        q["on_objective_complete"]["obj/look-back"] = serde_json::json!([{
            "type": "narrate",
            "text": "You slip back in.",
            "when": {"requires_flags": ["flag/curious"]},
            "happening": {
                "verb": "arrives",
                "text": "The party slips back in through the gate.",
                "subject": "anchor/door"
            }
        }]);
        let q = quest(quests, "quest/linger");
        q["objectives"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "id": "obj/look-up",
                "type": "reach-anchor",
                "anchor": "anchor/exit",
                "radius": 2,
                "happening": {"verb": "learns", "text": "The wall-walk is empty."}
            }));
        q["on_objective_complete"]["obj/look-up"] = serde_json::json!([{
            "type": "set-flag",
            "flag": "flag/curious",
            "when": {"forbids_flags": ["flag/lingered"]}
        }]);
    })
}

#[test]
fn two_orders_of_the_same_steps_that_hold_different_flags_are_both_judged() {
    let c = curious_strand_campaign();
    assert_exported_orders_clean(&c);
    let d = find(branch::check_branches(&c), "DW0485");
    assert!(d.message.contains("after it is sealed"), "{}", d.message);
    let order = printed_order(&d.message);
    let at = |o: &str| order.iter().position(|x| x == o);
    assert!(
        at("obj/look-up") < at("obj/linger"),
        "the order looks up before it lingers: {order:?}"
    );
}

/// Adds quest `id` to the plan, depending on `deps`.
fn plan_quest(plan: &mut Value, id: &str, deps: &[&str]) {
    plan["content"]["quests"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "act": 2,
            "area": "area/keep",
            "depends_on": deps,
            "goal": "A beat of the night.",
            "activity": ["talk"],
            "id": id,
            "mandatory": false,
            "npcs": []
        }));
}

/// **Two orders that do the same steps and hold the same flags can leave the
/// gate in different states.** `quest/bar` seals the gate and `quest/lift`
/// opens it, in either order; `quest/yard`, legal once both are done, walks in
/// through it. Barring then lifting leaves it open; lifting then barring leaves
/// it sealed. The exported order bars first.
fn bar_and_lift_campaign() -> Campaign {
    campaign_with(|plan, quests, _| {
        plan_quest(plan, "quest/bar", &["quest/decide"]);
        plan_quest(plan, "quest/lift", &["quest/decide"]);
        plan_quest(plan, "quest/yard", &["quest/bar", "quest/lift"]);
        let one = |id: &str, obj: &str, requires: Value, bundle: Value| {
            serde_json::json!({
                "id": id,
                "trigger": {"type": "quest-complete", "quest": "quest/decide"},
                "happening": {"verb": "learns", "text": "A beat of the night."},
                "objectives": [{
                    "id": obj,
                    "type": "reach-anchor",
                    "anchor": "anchor/exit",
                    "radius": 2,
                    "requires_flags": requires,
                    "happening": {"verb": "learns", "text": "The party crosses the yard."}
                }],
                "on_objective_complete": {obj: bundle},
                "on_complete": []
            })
        };
        let qs = quests["content"]["quests"].as_array_mut().unwrap();
        qs.push(one(
            "quest/bar",
            "obj/bar",
            serde_json::json!([]),
            serde_json::json!([
                {"type": "set-flag", "flag": "flag/barred"},
                {
                    "type": "close-gate",
                    "anchor": "anchor/door",
                    "happening": {
                        "verb": "seals",
                        "text": "The bar drops across the gate.",
                        "subject": "anchor/door"
                    }
                }
            ]),
        ));
        qs.push(one(
            "quest/lift",
            "obj/lift",
            serde_json::json!([]),
            serde_json::json!([
                {"type": "set-flag", "flag": "flag/lifted"},
                {
                    "type": "open-gate",
                    "anchor": "anchor/door",
                    "happening": {
                        "verb": "opens",
                        "text": "The party lifts the bar.",
                        "subject": "anchor/door"
                    }
                }
            ]),
        ));
        qs.push(one(
            "quest/yard",
            "obj/yard",
            serde_json::json!(["flag/barred", "flag/lifted"]),
            serde_json::json!([{
                "type": "narrate",
                "text": "Back inside.",
                "happening": {
                    "verb": "arrives",
                    "text": "The party comes back in through the gate.",
                    "subject": "anchor/door"
                }
            }]),
        ));
    })
}

#[test]
fn two_orders_that_leave_a_subject_in_different_states_are_both_judged() {
    let c = bar_and_lift_campaign();
    assert_exported_orders_clean(&c);
    let d = find(branch::check_branches(&c), "DW0485");
    assert!(d.message.contains("after it is sealed"), "{}", d.message);
    let order = printed_order(&d.message);
    let at = |o: &str| order.iter().position(|x| x == o);
    assert!(
        at("obj/lift") < at("obj/bar") && at("obj/bar") < at("obj/yard"),
        "the order lifts, then bars, then walks in: {order:?}"
    );
}

/// **A beat whose own effect is gated fires differently by when it is
/// taken.** `obj/look-up`, a second beat of `quest/linger`, says nothing a rule
/// reads and sets `flag/curious` — but only once the party has lingered. The
/// strand needs `flag/curious` and walks in through the gate. Lingering, then
/// looking up, then standing the watch and going back walks in through the
/// sealed gate; looking up before lingering never opens the strand at all. A
/// search that took `obj/look-up` first wherever it could, as though its effect
/// fired the same either way, would never see the strand.
fn curious_after_lingering_campaign() -> Campaign {
    lingering_strand_fixture(|quests| {
        quest(quests, "quest/look-back")["objectives"][0]["requires_flags"] =
            serde_json::json!(["flag/curious"]);
        let q = quest(quests, "quest/linger");
        q["objectives"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "id": "obj/look-up",
                "type": "reach-anchor",
                "anchor": "anchor/exit",
                "radius": 2,
                "happening": {"verb": "learns", "text": "The wall-walk is empty."}
            }));
        q["on_objective_complete"]["obj/look-up"] = serde_json::json!([{
            "type": "set-flag",
            "flag": "flag/curious",
            "when": {"requires_flags": ["flag/lingered"]}
        }]);
    })
}

#[test]
fn a_beat_whose_own_effect_is_gated_is_judged_in_every_order() {
    let c = curious_after_lingering_campaign();
    assert_exported_orders_clean(&c);
    let d = find(branch::check_branches(&c), "DW0485");
    assert!(d.message.contains("after it is sealed"), "{}", d.message);
    let order = printed_order(&d.message);
    let at = |o: &str| order.iter().position(|x| x == o);
    assert!(
        at("obj/linger") < at("obj/look-up") && at("obj/watch") < at("obj/look-back"),
        "the order lingers, looks up, seals, then walks in: {order:?}"
    );
}

/// The strand of [`optional_strand_fixture`] with a second beat,
/// `obj/look-up`, that says nothing a rule reads; `bundle` is its effects.
fn strand_with_a_second_beat(bundle: Value, then: impl Fn(&mut Value)) -> Campaign {
    optional_strand_fixture(|quests| {
        let q = quest(quests, "quest/look-back");
        q["objectives"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "id": "obj/look-up",
                "type": "reach-anchor",
                "anchor": "anchor/exit",
                "radius": 2,
                "happening": {"verb": "learns", "text": "The wall-walk is empty."}
            }));
        q["on_objective_complete"]["obj/look-up"] = bundle.clone();
        then(quests);
    })
}

/// **A beat that writes a datum a gate compares changes what a later beat
/// fires.** `obj/look-up` counts a knock; the strand's walk-in beat fires only
/// while no knock has been counted. Standing the watch and going straight back
/// walks in through the sealed gate; looking up first never does.
fn knock_counted_campaign() -> Campaign {
    strand_with_a_second_beat(
        serde_json::json!([{"type": "add-state", "state": "state/knocks", "amount": 1}]),
        |quests| {
            quests["content"]["state"] = serde_json::json!([{
                "id": "state/knocks",
                "initial": 0,
                "note": "How many knocks the party has answered.",
                "scope": "party"
            }]);
            let q = quest(quests, "quest/look-back");
            q["objectives"][0]["happening"] =
                serde_json::json!({"verb": "learns", "text": "The yard is empty."});
            q["on_objective_complete"]["obj/look-back"] = serde_json::json!([{
                "type": "narrate",
                "text": "You slip back in.",
                "when": {"requires_state": [
                    {"op": "at-most", "state": "state/knocks", "value": 0}
                ]},
                "happening": {
                    "verb": "arrives",
                    "text": "The party slips back in through the gate.",
                    "subject": "anchor/door"
                }
            }]);
        },
    )
}

#[test]
fn a_beat_that_writes_a_compared_datum_is_judged_in_every_order() {
    let c = knock_counted_campaign();
    assert_exported_orders_clean(&c);
    let d = find(branch::check_branches(&c), "DW0485");
    assert!(d.message.contains("after it is sealed"), "{}", d.message);
    let order = printed_order(&d.message);
    assert!(
        !order.iter().any(|o| o == "obj/look-up"),
        "the order never looks up: {order:?}"
    );
}

/// **A flag can reach a gate through an ambient producer.** `obj/look-up` sets
/// `flag/curious`, which nothing reads but a trigger that then sets
/// `flag/spooked` — and the strand forbids `flag/spooked`. Standing the watch
/// and going straight back walks in through the sealed gate; looking up first
/// shuts the strand.
fn spooked_by_a_trigger_campaign() -> Campaign {
    strand_with_a_second_beat(
        serde_json::json!([{"type": "set-flag", "flag": "flag/curious"}]),
        |quests| {
            quests["content"]["triggers"] = serde_json::json!([{
                "id": "trigger/spook",
                "at": "anchor/exit",
                "on": {"on": "approach", "range": 3},
                "once": true,
                "requires_flags": ["flag/curious"],
                "effects": [{"type": "set-flag", "flag": "flag/spooked"}]
            }]);
            quest(quests, "quest/look-back")["objectives"][0]["forbids_flags"] =
                serde_json::json!(["flag/spooked"]);
        },
    )
}

#[test]
fn a_flag_that_reaches_a_gate_through_a_trigger_is_judged_in_every_order() {
    let c = spooked_by_a_trigger_campaign();
    assert_exported_orders_clean(&c);
    let d = find(branch::check_branches(&c), "DW0485");
    assert!(d.message.contains("after it is sealed"), "{}", d.message);
    let order = printed_order(&d.message);
    assert!(
        !order.iter().any(|o| o == "obj/look-up"),
        "the order never looks up: {order:?}"
    );
}

/// **Two orders that do the same steps and hold the same flags can hold
/// different values.** The strand's two new beats set the knock count to five
/// and add one to it, in either order; its walk-in beat, which waits for both,
/// fires only while the count is at most five. Adding then setting leaves five,
/// and the party walks in through the sealed gate; setting then adding leaves
/// six, and it never does. The exported order sets first.
fn knock_count_campaign() -> Campaign {
    optional_strand_fixture(|quests| {
        quests["content"]["state"] = serde_json::json!([{
            "id": "state/knocks",
            "initial": 0,
            "note": "How many knocks the party has answered.",
            "scope": "party"
        }]);
        let q = quest(quests, "quest/look-back");
        let beat = |id: &str| {
            serde_json::json!({
                "id": id,
                "type": "reach-anchor",
                "anchor": "anchor/exit",
                "radius": 2,
                "happening": {"verb": "learns", "text": "Someone knocks."}
            })
        };
        let objectives = q["objectives"].as_array_mut().unwrap();
        objectives.push(beat("obj/count-set"));
        objectives.push(beat("obj/count-add"));
        q["objectives"][0]["after"] = serde_json::json!(["obj/count-set", "obj/count-add"]);
        q["objectives"][0]["happening"] =
            serde_json::json!({"verb": "learns", "text": "The yard is empty."});
        q["on_objective_complete"] = serde_json::json!({
            "obj/count-set": [{"type": "set-state", "state": "state/knocks", "value": 5}],
            "obj/count-add": [{"type": "add-state", "state": "state/knocks", "amount": 1}],
            "obj/look-back": [{
                "type": "narrate",
                "text": "You slip back in.",
                "when": {"requires_state": [
                    {"op": "at-most", "state": "state/knocks", "value": 5}
                ]},
                "happening": {
                    "verb": "arrives",
                    "text": "The party slips back in through the gate.",
                    "subject": "anchor/door"
                }
            }]
        });
    })
}

#[test]
fn two_orders_that_leave_a_datum_at_different_values_are_both_judged() {
    let c = knock_count_campaign();
    assert_exported_orders_clean(&c);
    let d = find(branch::check_branches(&c), "DW0485");
    assert!(d.message.contains("after it is sealed"), "{}", d.message);
    let order = printed_order(&d.message);
    let at = |o: &str| order.iter().position(|x| x == o);
    assert!(
        at("obj/count-add") < at("obj/count-set"),
        "the order adds, then sets: {order:?}"
    );
}

/// **The replay's gate test decides which beats the chronicle shows.** On the
/// hold branch the watch seals the gate; walking out, the bundle first lifts
/// the bar — gated on `flag/unbarred`, which a LATER effect of the same bundle
/// sets — and then walks through. The replay reaches the lift before the flag
/// is set, so the lift never plays and the party walks through a sealed gate.
/// A chronicle that judged the gate against the flags the step ends with would
/// show the lift, and the clash would vanish.
fn lift_gated_on_a_later_sibling_campaign() -> Campaign {
    campaign_with(|_, quests, _| {
        let q = quest(quests, "quest/hold");
        q["on_objective_complete"]["obj/watch"][0] = serde_json::json!({
            "type": "close-gate",
            "anchor": "anchor/door",
            "happening": {
                "verb": "seals",
                "text": "The Keeper drops the bar across the gate.",
                "subject": "anchor/door"
            }
        });
        let walk_out = q["on_objective_complete"]["obj/walk-out"]
            .as_array_mut()
            .unwrap();
        for (i, e) in [
            serde_json::json!({
                "type": "open-gate",
                "anchor": "anchor/door",
                "when": {"requires_flags": ["flag/unbarred"]},
                "happening": {
                    "verb": "opens",
                    "text": "The party lifts the bar.",
                    "subject": "anchor/door"
                }
            }),
            serde_json::json!({"type": "set-flag", "flag": "flag/unbarred"}),
            serde_json::json!({
                "type": "narrate",
                "text": "Out through the gate.",
                "happening": {
                    "verb": "departs",
                    "text": "The party walks out through the gate.",
                    "subject": "anchor/door"
                }
            }),
        ]
        .into_iter()
        .enumerate()
        {
            walk_out.insert(i, e);
        }
    })
}

#[test]
fn a_beat_whose_gate_a_later_sibling_opens_does_not_play() {
    let d = find(
        branch::check_branches(&lift_gated_on_a_later_sibling_campaign()),
        "DW0485",
    );
    assert!(d.message.contains("after it is sealed"), "{}", d.message);
    assert!(
        d.message.contains("/on_objective_complete/obj/walk-out/2"),
        "{}",
        d.message
    );
    assert!(
        !d.message.contains("other than the exported one"),
        "the exported order shows it: {}",
        d.message
    );
}

/// **The search against a second method.** For every `DW0485` fixture in this
/// file, the clashes `DW0485` reads (the exported order, then the merged,
/// reduced search) equal the clashes found by walking every legal order one at
/// a time with no merged states and no quiet steps. Bound: the comparison
/// covers every fixture, and the enumeration finds a clash in most of them.
#[test]
fn the_search_finds_what_walking_every_order_finds() {
    let fixtures = contradiction_fixtures();
    let mut with_clash = 0usize;
    for (name, c) in &fixtures {
        let (searched, enumerated) = branch::clashes_by_search_and_by_enumeration(c);
        assert_eq!(
            searched, enumerated,
            "{name}: the search and the enumeration disagree"
        );
        with_clash += usize::from(!enumerated.is_empty());
    }
    assert_eq!(fixtures.len(), 31, "every DW0485 fixture is compared");
    assert_eq!(
        with_clash, 24,
        "the comparison binds: the fixtures that carry a clash carry one in both"
    );
}

/// `DW0927`: a branch with more play states than the search walks is **refused as
/// unproven, by name** — never called clean. Six optional strands opened by the
/// fork, each one beat that opens the gate the watch seals on the hold branch:
/// no order clashes, and a player may walk the six in any interleaving with the
/// watch. Under the shipped bound the branch is searched to the end and is
/// clean; under a bound of ten states the same branch is refused, and the
/// refusal says why.
#[test]
fn a_branch_past_the_bound_is_dw0927() {
    let c = campaign_with(|plan, quests, _| {
        quest(quests, "quest/hold")["on_objective_complete"]["obj/watch"][0] = serde_json::json!({
            "type": "close-gate",
            "anchor": "anchor/door",
            "happening": {
                "verb": "seals",
                "text": "The Keeper drops the bar across the gate.",
                "subject": "anchor/door"
            }
        });
        for k in 0..6 {
            let id = format!("quest/knock-{k}");
            plan["content"]["quests"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({
                    "act": 2,
                    "area": "area/keep",
                    "depends_on": ["quest/decide"],
                    "goal": "Answer a knock at the gate.",
                    "activity": ["talk"],
                    "id": id,
                    "mandatory": false,
                    "npcs": []
                }));
            let obj = format!("obj/knock-{k}");
            quests["content"]["quests"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({
                    "id": id,
                    "trigger": {"type": "quest-complete", "quest": "quest/decide"},
                    "happening": {"verb": "learns", "text": "Someone knocks."},
                    "objectives": [{
                        "id": obj,
                        "type": "reach-anchor",
                        "anchor": "anchor/exit",
                        "radius": 2,
                        "happening": {"verb": "learns", "text": "The party goes to the gate."}
                    }],
                    "on_objective_complete": {obj.clone(): [{
                        "type": "open-gate",
                        "anchor": "anchor/door",
                        "happening": {
                            "verb": "opens",
                            "text": "The party lifts the bar for whoever knocks.",
                            "subject": "anchor/door"
                        }
                    }]},
                    "on_complete": []
                }));
        }
    });
    let (diags, bind) = branch::check_branches_bound(&c);
    assert!(!diags.iter().any(|d| d.code == "DW0485"), "{diags:#?}");
    assert_eq!(bind.unproven, 0, "{bind:?}");
    assert!(
        bind.states > 10,
        "the shipped bound walked past ten: {bind:?}"
    );
    let (diags, bind) = branch::check_branches_within(&c, 10);
    // The bolt branch never seals the gate, so its knocks say nothing a rule
    // can read and are all quiet: it is walked in one line and stays under ten.
    assert_eq!(
        bind.unproven, 1,
        "the hold branch stops at the bound: {bind:?}"
    );
    assert!(
        !diags.iter().any(|d| d.code == "DW0485"),
        "an unproven branch is not a clash: {diags:#?}"
    );
    let d = find(diags, "DW0927");
    assert!(d.message.contains("UNPROVEN"), "{}", d.message);
    assert!(d.message.contains("branch/hold"), "{}", d.message);
    assert!(
        d.message.contains("bound of 10 distinct play states"),
        "{}",
        d.message
    );
    assert!(
        bind.line()
            .contains("1 branch(es) unproven at the bound of 10 state(s) per branch (DW0927)"),
        "{}",
        bind.line()
    );
}

// --- the derived subject (spec-0071 §3) ------------------------------------

/// The hold branch's gate is **sealed** by an effect that states no
/// `happening.subject`, and the party walks out through it two beats later. The
/// clash is visible only because the seal's subject is the gate it names: the id
/// is two keys to the left of the `happening` and a creator who typed it again
/// would be typing a derivation.
#[test]
fn a_seal_with_no_stated_subject_is_still_about_its_gate() {
    let d = find(branch::check_branches(&derived_subject_fixture()), "DW0485");
    assert!(d.message.contains("after it is sealed"), "{}", d.message);
    assert!(d.message.contains("anchor/door"), "{}", d.message);
}

/// The fixture the §3 tests share: one `close-gate` whose beat names no subject,
/// and a later beat that walks through the same gate.
fn derived_subject_fixture() -> Campaign {
    campaign_with(|_, quests, _| {
        quest(quests, "quest/hold")["on_objective_complete"]["obj/watch"][0] = serde_json::json!({
            "type": "close-gate",
            "anchor": "anchor/door",
            "happening": {
                "verb": "seals",
                "text": "The Keeper drops the bar across the gate for good."
            }
        });
        quest(quests, "quest/hold")["objectives"][1]["happening"] = serde_json::json!({
            "verb": "departs",
            "text": "The party walks out through the gate.",
            "subject": "anchor/door"
        });
    })
}

/// **The perturbation only this rule can catch.** The same realized branch, with
/// the derived subject taken back out of the chronicle line that carries it, and
/// nothing else changed: the contradiction disappears. What makes `DW0485` see
/// this clash is the derivation and not the fixture.
#[test]
fn without_the_derived_subject_the_same_branch_is_silent() {
    let c = derived_subject_fixture();
    let mut realized = branch::realize(&c);
    let mut stripped = 0usize;
    for r in &mut realized {
        for line in &mut r.chronicle {
            // The seal states no subject of its own — the document is right
            // there in the fixture above — so this is exactly the derivation.
            if line.verb == delvewright_dsl::HappeningVerb::Seals && line.subject.is_some() {
                line.subject = None;
                stripped += 1;
            }
        }
    }
    assert!(stripped > 0, "the perturbation bound to something");
    let mut d = Vec::new();
    for r in &realized {
        branch::check_contradictions(r, &mut d);
    }
    assert!(
        d.is_empty(),
        "with the subject un-derived the proof has nothing to reason over: {d:#?}"
    );
}

/// A stated subject wins over the effect's own object: the caller knows more.
/// Here the seal is about the Keeper, not about the gate he bars, so the party's
/// walk out through the gate is no contradiction at all.
#[test]
fn a_stated_subject_beats_the_effect_s_own_object() {
    let c = a_stated_subject_beats_the_effect_s_own_object_campaign();
    assert!(
        !codes(&c).contains(&"DW0485".to_string()),
        "{:#?}",
        branch::check_branches(&c)
    );
}

// --- the artifacts ---------------------------------------------------------

// --- artifacts -------------------------------------------------------------

/// The plan row of the story world that realizes declared branch `id`
/// (spec-0108 §4.3: the plan's rows are worlds, each naming the branches its
/// flags realize).
fn row_of<'a>(plan: &'a Value, id: &str) -> &'a Value {
    plan["branches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["branches"].as_array().unwrap().iter().any(|b| b == id))
        .unwrap_or_else(|| panic!("no world realizes {id}: {plan:#}"))
}

/// The artifacts are emitted, one row and one chronicle per story world, and
/// byte-identical across runs (ADR-0006).
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
            "validation/branch-plan.json",
            "validation/world-chronicle-0.md",
            "validation/world-chronicle-1.md",
        ]
    );
    let plan: Value = serde_json::from_slice(&a["validation/branch-plan.json"]).unwrap();
    let branches = plan["branches"].as_array().unwrap();
    assert_eq!(branches.len(), 2);
    // Every world names its flags, its path and the choices that enter it.
    for b in branches {
        assert!(b["flags"]["set"].as_array().unwrap().len() == 1);
        assert!(b["flags"]["unset"].as_array().unwrap().len() == 1);
        assert!(!b["critical_path"].as_array().unwrap().is_empty());
        assert!(!b["entry_choices"].as_array().unwrap().is_empty());
        assert_eq!(b["branches"].as_array().unwrap().len(), 1, "{b}");
    }
    assert_eq!(plan["branches_unrealized"], serde_json::json!([]));
    let hold = row_of(&plan, "branch/hold");
    let k = hold["id"].as_str().unwrap().trim_start_matches("world/");
    let md = String::from_utf8(a[&format!("validation/world-chronicle-{k}.md")].clone()).unwrap();
    assert!(
        md.contains(&format!("# World chronicle — `world/{k}`")),
        "{md}"
    );
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
    // The two branches are entered by two DIFFERENT options of the same NPC —
    // which is exactly what makes a branch run a scripted-choice run.
    let hold = &row_of(&plan, "branch/hold")["entry_choices"][0];
    let bolt = &row_of(&plan, "branch/bolt")["entry_choices"][0];
    assert_eq!(hold["npc"], "npc/keeper");
    assert_eq!(hold["command"], "/trigger dw.dlg_keeper set 2");
    assert_eq!(bolt["command"], "/trigger dw.dlg_keeper set 3");
    assert_ne!(hold["option"], bolt["option"]);
    // Each world names the executable path the harness walks for it.
    for b in ["branch/hold", "branch/bolt"] {
        let row = row_of(&plan, b);
        let k = row["id"].as_str().unwrap().trim_start_matches("world/");
        assert_eq!(row["path"], format!("world-path-{k}.json"));
    }
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
