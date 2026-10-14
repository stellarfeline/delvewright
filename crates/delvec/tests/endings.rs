//! spec-0108 — an ending reads the run: what each ending's final scene reads
//! (§3.2–§3.4), the campaign's declaration of it (`DW1029`, §3.5), the story
//! worlds the readings distinguish (§4) and the endings no world reaches
//! (`DW1030`, §7.2). Library-level: every fixture is a campaign document parsed
//! the way the engine parses one, judged by the one entry the validation funnel
//! calls ([`ending::check_bound`]) and its parts.

mod common;

use delvec::compiler::ending;
use delvec::compiler::flow::Flow;
use delvewright_dsl::{Campaign, Diagnostic, RawCampaign, parse_campaign};
use serde_json::{Value, json};

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        common::compiler_fixtures_dir()
            .join("branch-two-endings")
            .join(name),
    )
    .unwrap()
}

fn envelope(stage: &str, content: Value) -> String {
    json!({
        "campaign_id": "hello-world",
        "dsl_version": delvewright_dsl::DSL_VERSION,
        "stage": stage,
        "content": content,
    })
    .to_string()
}

/// A planned quest row.
fn planned(id: &str, deps: &[&str], mandatory: bool) -> Value {
    json!({
        "act": 1, "area": "area/keep", "depends_on": deps,
        "goal": format!("Do {id}."), "id": id, "mandatory": mandatory,
        "npcs": ["npc/keeper"]
    })
}

/// A stage-5 quest: one `reach-anchor` objective `obj` at the exit, opened by
/// `after` (`None` = campaign start), with `on_complete`.
fn reach_quest(id: &str, obj: &str, after: Option<&str>, on_complete: Value) -> Value {
    let trigger = match after {
        None => json!({"type": "campaign-start"}),
        Some(q) => json!({"type": "quest-complete", "quest": q}),
    };
    json!({
        "id": id,
        "happening": {"text": format!("{id} opens."), "verb": "learns"},
        "trigger": trigger,
        "objectives": [{
            "id": obj, "type": "reach-anchor", "anchor": "anchor/exit", "radius": 2,
            "happening": {"text": format!("{obj} is reached."), "verb": "arrives"}
        }],
        "on_complete": on_complete,
    })
}

/// The opening quest: talk to the Keeper, whose one option completes it and
/// sets `flag/promised`.
fn decide_quest() -> Value {
    json!({
        "id": "quest/decide",
        "happening": {"text": "The party reaches the Keeper.", "verb": "arrives"},
        "trigger": {"type": "campaign-start"},
        "cast": {"npc/keeper": {"at": "anchor/keeper-stand", "dialogue": "dlg/greeting",
                                "doing": "keeping the door"}},
        "objectives": [{
            "id": "obj/decide", "type": "talk-to", "npc": "npc/keeper",
            "happening": {"text": "The Keeper listens.", "verb": "learns"}
        }],
        "on_complete": [],
    })
}

fn dialogue() -> String {
    envelope(
        "dialogue",
        json!({"dialogues": [{
            "npc": "npc/keeper", "root": "dlg/greeting",
            "nodes": [{
                "id": "dlg/greeting", "text": "Well?",
                "options": [{
                    "label": "We promise.",
                    "happening": {"subject": "npc/keeper", "text": "The party promises.",
                                  "verb": "believes"},
                    "effects": [
                        {"type": "set-flag", "flag": "flag/promised"},
                        {"type": "complete-objective", "objective": "obj/decide"}
                    ]
                }]
            }]
        }]}),
    )
}

fn campaign(plan: Value, quests: Value) -> Campaign {
    parse_campaign(&RawCampaign {
        world: fixture("world.json"),
        npcs: fixture("npcs.json"),
        classes: fixture("classes.json"),
        quest_plan: envelope("quest-plan", plan),
        quests: envelope("quests", quests),
        dialogue: dialogue(),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    })
    .expect("fixture parses")
}

fn cc(id: &str, when: Value) -> Value {
    let mut e = json!({"type": "campaign-complete", "ending": id,
                       "happening": {"text": format!("{id}."), "verb": "survives"}});
    if !when.is_null() {
        e["when"] = when;
    }
    e
}

fn codes(d: &[Diagnostic], code: &str) -> Vec<Diagnostic> {
    d.iter().filter(|x| x.code == code).cloned().collect()
}

// ---------------------------------------------------------------------------
// Criterion 2 — the reading.
// ---------------------------------------------------------------------------

/// One ending reading four inputs: a flag written in its own quest (final
/// scene), a flag a dialogue option in another quest sets (seeded, said), a
/// flag another quest's objective sets (seeded, did), and a datum another
/// quest's completion moves (seeded, finished).
fn four_inputs() -> Campaign {
    let plan = json!({
        "finale": "quest/end",
        "quests": [
            planned("quest/decide", &[], true),
            planned("quest/chore", &["quest/decide"], true),
            planned("quest/end", &["quest/chore"], true),
        ],
    });
    let mut chore = reach_quest(
        "quest/chore",
        "obj/fetch",
        Some("quest/decide"),
        json!([{"type": "add-state", "state": "state/chores", "amount": 1}]),
    );
    chore["on_objective_complete"] =
        json!({"obj/fetch": [{"type": "set-flag", "flag": "flag/fetched"}]});
    let mut end = reach_quest(
        "quest/end",
        "obj/walk-out",
        Some("quest/chore"),
        json!([cc(
            "ending/home",
            json!({
                "requires_flags": ["flag/at-the-end", "flag/promised", "flag/fetched"],
                "requires_state": [{"state": "state/chores", "op": "at-least", "value": 1}]
            })
        )]),
    );
    end["on_objective_complete"] =
        json!({"obj/walk-out": [{"type": "set-flag", "flag": "flag/at-the-end"}]});
    campaign(
        plan,
        json!({
            "state": [{"id": "state/chores", "scope": "party"}],
            "quests": [decide_quest(), chore, end],
        }),
    )
}

#[test]
fn an_ending_reads_every_gate_of_its_final_scene_with_each_writers_kind() {
    let c = four_inputs();
    let flow = Flow::new(&c);
    let a = ending::analyse(&c, flow.story());
    assert_eq!(a.endings.len(), 1);
    let v = &a.endings[0];
    assert_eq!(v.ending.id, "ending/home");
    assert_eq!(v.host_quest.as_deref(), Some("quest/end"));
    let rows: Vec<(String, Vec<&str>, bool, Vec<Option<String>>)> = v
        .inputs
        .iter()
        .map(|i| {
            (
                i.key.id().to_string(),
                i.writers.iter().map(|w| w.kind.token()).collect(),
                i.seeded,
                i.writers.iter().map(|w| w.quest.clone()).collect(),
            )
        })
        .collect();
    let q = |s: &str| Some(s.to_string());
    assert_eq!(
        rows,
        vec![
            (
                "flag/at-the-end".into(),
                vec!["did"],
                false,
                vec![q("quest/end")]
            ),
            (
                "flag/fetched".into(),
                vec!["did"],
                true,
                vec![q("quest/chore")]
            ),
            (
                "flag/promised".into(),
                vec!["said"],
                true,
                vec![q("quest/decide")]
            ),
            (
                "state/chores".into(),
                vec!["finished"],
                true,
                vec![q("quest/chore")]
            ),
        ]
    );
    assert_eq!(v.seeded(), 3);
    assert_eq!(v.seeded_deeds(), 2);
    // Reached by every world (no elective act writes an input).
    assert!(!v.worlds.is_empty(), "{:?}", v.worlds);
}

#[test]
fn an_ungated_ending_reads_zero_inputs_and_says_so() {
    let c = four_inputs();
    let mut quests: Value = serde_json::to_value(&c.quests).unwrap();
    quests["content"]["quests"][2]["on_complete"][0]
        .as_object_mut()
        .unwrap()
        .remove("when");
    let c = campaign(
        serde_json::to_value(&c.quest_plan).unwrap()["content"].clone(),
        quests["content"].clone(),
    );
    let (d, b) = ending::check_bound(&c);
    assert!(
        codes(&d, "DW1029").is_empty() && codes(&d, "DW1030").is_empty(),
        "{d:#?}"
    );
    assert_eq!(b.inputs, 0);
    assert!(b.line().contains("0 input(s) read"), "{}", b.line());
    assert!(
        b.per_ending[0].contains("reads 0 input(s) — none"),
        "{}",
        b.per_ending[0]
    );
}

// ---------------------------------------------------------------------------
// Criterion 3 — the declaration.
// ---------------------------------------------------------------------------

fn with_declaration(decl: Value) -> Campaign {
    let c = four_inputs();
    let mut plan = serde_json::to_value(&c.quest_plan).unwrap()["content"].clone();
    plan["endings_read"] = decl;
    campaign(
        plan,
        serde_json::to_value(&c.quests).unwrap()["content"].clone(),
    )
}

#[test]
fn dw1029_fires_on_at_least_with_the_counts_in_the_message() {
    let (d, _) = ending::check_bound(&with_declaration(json!({"at_least": 4})));
    let got = codes(&d, "DW1029");
    assert_eq!(got.len(), 1, "{got:#?}");
    let m = &got[0].message;
    assert!(m.contains("`ending/home` reads 3 seeded input(s)"), "{m}");
    assert!(m.contains("at least 4 (`endings_read.at_least`)"), "{m}");
    assert!(m.contains("flag/at-the-end: did, final-scene"), "{m}");
    assert_eq!(got[0].path, "/content/quests/2/on_complete/0");
    // The declaration it meets is silent.
    let (d, _) = ending::check_bound(&with_declaration(json!({"at_least": 3})));
    assert!(codes(&d, "DW1029").is_empty(), "{d:#?}");
}

#[test]
fn dw1029_fires_on_deeds_at_least_on_its_own() {
    let (d, _) = ending::check_bound(&with_declaration(json!({"deeds_at_least": 3})));
    let got = codes(&d, "DW1029");
    assert_eq!(got.len(), 1, "{got:#?}");
    let m = &got[0].message;
    assert!(m.contains("reads 2 seeded input(s) a deed writes"), "{m}");
    assert!(m.contains("`endings_read.deeds_at_least`"), "{m}");
}

#[test]
fn dw1029_is_silent_without_a_declaration() {
    let (d, b) = ending::check_bound(&four_inputs());
    assert!(codes(&d, "DW1029").is_empty(), "{d:#?}");
    assert!(b.line().contains("3 seeded"), "{}", b.line());
}

#[test]
fn a_declaration_over_zero_endings_states_zero_and_refuses_nothing() {
    let c = with_declaration(json!({"at_least": 3, "deeds_at_least": 1}));
    let mut quests: Value = serde_json::to_value(&c.quests).unwrap();
    quests["content"]["quests"][2]["on_complete"] = json!([]);
    let c = campaign(
        serde_json::to_value(&c.quest_plan).unwrap()["content"].clone(),
        quests["content"].clone(),
    );
    let (d, b) = ending::check_bound(&c);
    assert!(
        codes(&d, "DW1029").is_empty() && codes(&d, "DW1030").is_empty(),
        "{d:#?}"
    );
    assert!(
        b.line().starts_with("ending binding: 0 ending(s);"),
        "{}",
        b.line()
    );
}

// ---------------------------------------------------------------------------
// Criterion 4 — story worlds.
// ---------------------------------------------------------------------------

/// One optional tally quest and one elective trigger, each `add-state 1` on
/// `state/tally`; the finale's scene ends three ways on the tally, and
/// `extra` adds more lines to it.
fn tally(extra: Vec<Value>) -> Campaign {
    let plan = json!({
        "finale": "quest/end",
        "quests": [
            planned("quest/decide", &[], true),
            planned("quest/tally", &["quest/decide"], false),
            planned("quest/end", &["quest/decide"], true),
        ],
    });
    let tally_q = reach_quest(
        "quest/tally",
        "obj/count",
        Some("quest/decide"),
        json!([{"type": "add-state", "state": "state/tally", "amount": 1}]),
    );
    let at = |op: &str, v: i32| json!({"requires_state": [{"state": "state/tally", "op": op, "value": v}]});
    let mut scene = vec![
        cc("ending/none", at("at-most", 0)),
        cc("ending/one", at("equals", 1)),
        cc("ending/many", at("at-least", 2)),
    ];
    scene.extend(extra);
    let end = reach_quest(
        "quest/end",
        "obj/walk-out",
        Some("quest/decide"),
        json!(scene),
    );
    campaign(
        plan,
        json!({
            "state": [{"id": "state/tally", "scope": "party"}],
            "triggers": [{
                "id": "trigger/bell", "at": "anchor/door", "on": {"on": "use"},
                "effects": [{"type": "add-state", "state": "state/tally", "amount": 1}]
            }],
            "quests": [decide_quest(), tally_q, end],
        }),
    )
}

#[test]
fn a_tally_over_two_elective_writers_reaches_all_three_endings() {
    let c = tally(Vec::new());
    let flow = Flow::new(&c);
    let story = flow.story();
    let electives: Vec<&str> = story.electives.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(electives, vec!["quest/tally", "trigger/bell"]);
    // The extremes plus a witness for `equals 1`.
    let origins: Vec<(&str, Vec<&String>)> = story
        .worlds
        .iter()
        .map(|w| (w.origin.token(), w.performs.iter().collect()))
        .collect();
    assert_eq!(origins.len(), 3, "{origins:?}");
    assert_eq!(origins[0].0, "all");
    assert_eq!(origins[1], ("none", vec![]));
    assert_eq!(origins[2].0, "witness");
    assert_eq!(origins[2].1.len(), 1, "{origins:?}");
    let (d, b) = ending::check_bound(&c);
    assert!(codes(&d, "DW1030").is_empty(), "{d:#?}");
    assert!(
        b.line().contains("3 story world(s) enumerated"),
        "{}",
        b.line()
    );
    let a = ending::analyse(&c, story);
    for v in &a.endings {
        assert_eq!(
            v.worlds.len(),
            1,
            "{} reached by {:?}",
            v.ending.id,
            v.worlds
        );
    }
    // The tally is dated in a story world: the trigger's write is an entry deed.
    let one = &story.runs[2];
    let deeds = &one.playthrough.story.as_ref().unwrap().deeds;
    if story.worlds[2].performs.contains("trigger/bell") {
        assert_eq!(deeds.len(), 1, "{deeds:?}");
    } else {
        assert!(deeds.is_empty(), "{deeds:?}");
    }
}

#[test]
fn a_target_no_subset_reaches_is_unrealizable_with_the_subset_sum_reason() {
    let c = tally(vec![cc(
        "ending/legion",
        json!({"requires_state": [{"state": "state/tally", "op": "at-least", "value": 3}]}),
    )]);
    let flow = Flow::new(&c);
    let story = flow.story();
    let t = story
        .targets
        .iter()
        .find(|t| t.classes.iter().any(|(_, cl)| cl.words() == "3.."))
        .expect("a `3..` target");
    assert!(t.witness.is_none());
    let r = t.reason.as_deref().unwrap();
    assert!(
        r.contains(
            "`state/tally` in 3..: no subset of elective amounts reaches it (reachable: 0, 1, 2)"
        ),
        "{r}"
    );
    let (d, _) = ending::check_bound(&c);
    let got = codes(&d, "DW1030");
    assert_eq!(got.len(), 1, "{got:#?}");
    assert!(
        got[0].message.contains("`ending/legion`"),
        "{}",
        got[0].message
    );
    assert!(
        got[0].message.contains("reachable: 0, 1, 2"),
        "{}",
        got[0].message
    );
}

/// The base campaign the reachability codes are compared on: today's worlds
/// are one per dialogue selection, and story worlds do not change their count.
#[test]
fn todays_worlds_are_untouched_by_story_worlds() {
    let c = tally(Vec::new());
    let flow = Flow::new(&c);
    assert_eq!(flow.world_count(), 1, "one dialogue selection, one world");
    assert_eq!(flow.story_count(), 3);
    // The all world is today's world: same flags, same quests.
    assert_eq!(flow.story_flags(0), flow.world_flags(0));
    assert_eq!(flow.story_completed(0), flow.world_completed(0));
}

// ---------------------------------------------------------------------------
// Criterion 5 — DW1030.
// ---------------------------------------------------------------------------

#[test]
fn dw1030_fires_for_an_ending_no_world_fires() {
    let c = tally(vec![cc(
        "ending/never",
        json!({"requires_state": [{"state": "state/tally", "op": "at-least", "value": 99}]}),
    )]);
    let (d, _) = ending::check_bound(&c);
    let got = codes(&d, "DW1030");
    assert_eq!(got.len(), 1, "{got:#?}");
    let m = &got[0].message;
    assert!(
        m.contains("`ending/never` is an ending no story world reaches"),
        "{m}"
    );
    assert!(m.contains("no story world fires it"), "{m}");
    assert!(m.contains("state/tally at-least 99"), "{m}");
    assert!(
        m.contains("Remedy: give the input an elective writer"),
        "{m}"
    );
}

#[test]
fn dw1030_refuses_an_ending_unproven_at_the_bound_and_names_it() {
    let c = tally(Vec::new());
    let flow = Flow::new(&c);
    // A test-only override drives the cap to one world: only the *all* world
    // is enumerated, so the endings it does not fire are unproven, never clean.
    let story = flow.story_within(1);
    assert_eq!(story.worlds.len(), 1);
    assert!(story.at_bound > 0);
    let a = ending::analyse(&c, &story);
    let d = ending::check(&c, &story, &a);
    let got = codes(&d, "DW1030");
    assert_eq!(got.len(), 2, "{got:#?}");
    for x in &got {
        assert!(
            x.message.contains("stopped at the bound of 1 world(s)"),
            "{}",
            x.message
        );
        assert!(x.message.contains("refused as unproven"), "{}", x.message);
    }
}

#[test]
fn dw1030_refuses_an_ending_read_against_a_datum_a_stake_forfeits() {
    let c = tally(Vec::new());
    let mut quests: Value = serde_json::to_value(&c.quests).unwrap();
    quests["content"]["stakes"] = json!([{
        "id": "stake/tally", "state": "state/tally", "marker_item": "minecraft:lantern",
        "collect_by": "anyone", "collected_message": "You take it back.",
        "forfeit": {"kind": "proportion", "percent": 50}
    }]);
    let c = campaign(
        serde_json::to_value(&c.quest_plan).unwrap()["content"].clone(),
        quests["content"].clone(),
    );
    let flow = Flow::new(&c);
    assert!(flow.story().undatable.contains("state/tally"));
    let (d, _) = ending::check_bound(&c);
    let got = codes(&d, "DW1030");
    assert_eq!(got.len(), 3, "{got:#?}");
    for x in &got {
        assert!(x.message.contains("refused as unproven"), "{}", x.message);
        assert!(x.message.contains("state/tally"), "{}", x.message);
    }
}

#[test]
fn a_repeatable_strike_trigger_is_decided_in_every_world() {
    let c = tally(Vec::new());
    let mut quests: Value = serde_json::to_value(&c.quests).unwrap();
    // Only the trigger writes the tally now, and it is a repeatable strike.
    quests["content"]["quests"][1]["on_complete"] = json!([]);
    quests["content"]["triggers"][0]["on"] = json!({"on": "strike"});
    quests["content"]["triggers"][0]["once"] = json!(false);
    quests["content"]["quests"][2]["on_complete"]
        .as_array_mut()
        .unwrap()
        .remove(2);
    let c = campaign(
        serde_json::to_value(&c.quest_plan).unwrap()["content"].clone(),
        quests["content"].clone(),
    );
    let (d, _) = ending::check_bound(&c);
    assert!(codes(&d, "DW1030").is_empty(), "{d:#?}");
    let flow = Flow::new(&c);
    let a = ending::analyse(&c, flow.story());
    let reached = |id: &str| {
        let v = a.endings.iter().find(|v| v.ending.id == id).unwrap();
        v.worlds
            .iter()
            .map(|k| flow.story().worlds[*k].performs.contains("trigger/bell"))
            .collect::<Vec<_>>()
    };
    assert!(reached("ending/none").contains(&false));
    assert!(reached("ending/one").contains(&true));
}

/// The tally campaign with `flag/counted` set only by the optional tally quest
/// and read by the finale's cast — a deed-made fork — and `branch_points`.
fn counted_fork(points: Value) -> Campaign {
    let c = tally(Vec::new());
    let mut quests: Value = serde_json::to_value(&c.quests).unwrap();
    quests["content"]["quests"][1]["on_complete"]
        .as_array_mut()
        .unwrap()
        .push(json!({"type": "set-flag", "flag": "flag/counted"}));
    quests["content"]["quests"][2]["cast"] = json!({"npc/keeper": [
        {"at": "anchor/keeper-stand", "dialogue": "dlg/greeting", "doing": "tallying",
         "requires_flags": ["flag/counted"]},
        {"at": "offstage", "forbids_flags": ["flag/counted"]}
    ]});
    let mut plan = serde_json::to_value(&c.quest_plan).unwrap()["content"].clone();
    plan["branch_points"] = points;
    campaign(plan, quests["content"].clone())
}

/// spec-0108 §4.4: `DW0480` reaches a fork a deed makes. The only writer of
/// `flag/counted` is an optional quest, so the *all* world sets it and the
/// *none* world does not; it gates a cast placement, so it is an undeclared
/// fork until a branch point owns it — and a two-branch point whose second
/// branch pins nothing set (the *did not* side) makes it green.
#[test]
fn dw0480_reaches_a_fork_a_deed_makes_until_it_is_declared() {
    let d = delvec::compiler::branch::check_branches(&counted_fork(json!([])));
    let got = codes(&d, "DW0480");
    assert_eq!(got.len(), 1, "{d:#?}");
    assert!(
        got[0].message.contains("`flag/counted`"),
        "{}",
        got[0].message
    );
    let declared = counted_fork(json!([{
        "id": "branch-point/count", "forks_on": ["flag/counted"], "opens_at": "quest/decide",
        "branches": [
            {"id": "branch/counted", "flags": ["flag/counted"], "leads_to": "quest/end"},
            {"id": "branch/uncounted", "flags": [], "leads_to": "quest/end"}
        ]
    }]));
    let d = delvec::compiler::branch::check_branches(&declared);
    assert!(d.is_empty(), "{d:#?}");
    let realized = delvec::compiler::branch::realize(&declared);
    assert!(realized.iter().all(|r| r.world.is_some()), "{realized:#?}");
}

/// spec-0108 criterion 4, the reachability half: `tests/fixtures/elective-reach`
/// is a campaign whose optional quest is reachable only at full participation —
/// its one objective waits on a flag only an elective `use` trigger sets. Its
/// `DW0202`/`DW0203`/`DW0879` findings are exactly the set `delvec analyze`
/// raised on it at `d336a2b1d` (the fixture with its envelope at that tree's
/// `dsl_version`): none. Today's worlds still decide those codes.
#[test]
fn reachability_codes_on_an_elective_only_quest_match_the_base_tree() {
    let dir = common::compiler_fixtures_dir().join("elective-reach");
    let read = |f: &str| std::fs::read_to_string(dir.join(f)).unwrap();
    let c = parse_campaign(&RawCampaign {
        world: read("world.json"),
        npcs: read("npcs.json"),
        classes: read("classes.json"),
        quest_plan: read("quest-plan.json"),
        quests: read("quests.json"),
        dialogue: read("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    })
    .expect("fixture parses");
    let prefabs =
        delvec::compiler::registry::PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let mut found: Vec<String> = delvec::compiler::analyze::analyze_campaign(&c, &prefabs)
        .into_iter()
        .chain(delvec::compiler::statepath::check(&c).0)
        .map(|d| d.code)
        .filter(|code| matches!(code.as_str(), "DW0202" | "DW0203" | "DW0879"))
        .collect();
    found.sort();
    // Measured at d336a2b1d: `delvec analyze` printed DW0482, DW0484 and
    // DW0467 on this fixture and none of the three codes compared here.
    let at_base: Vec<String> = Vec::new();
    assert_eq!(found, at_base);
    let flow = Flow::new(&c);
    assert_eq!(flow.world_count(), 1);
    // The optional quest completes in today's world.
    assert!(flow.world_completed(0).contains("quest/tally"));
}
