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
    /// One input as the reading reports it: key, writer kinds, seeded, dated quests.
    type Row<'a> = (String, Vec<&'a str>, bool, Vec<Option<String>>);
    let rows: Vec<Row<'_>> = v
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

// ---------------------------------------------------------------------------
// Criteria 7 and 11 — the ledger, the binding line, through the binary.
// ---------------------------------------------------------------------------

/// A private copy of `tests/fixtures/elective-reach` with `patch` applied to
/// its quests document.
fn reach_copy(tag: &str, patch: impl FnOnce(&mut Value)) -> std::path::PathBuf {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("endings-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    common::copy_dir_all(
        &common::compiler_fixtures_dir().join("elective-reach"),
        &dir,
    );
    common::patch_file(&dir.join("quests.json"), |q| {
        // The hello room's door is shut at world load; the opening beat opens
        // it, so the build's route proof has a way to the exit.
        q["content"]["quests"][0]["on_complete"] = json!([{
            "type": "open-gate", "anchor": "anchor/door",
            "happening": {"subject": "anchor/door", "text": "The Keeper opens the door.",
                          "verb": "opens"}
        }]);
        patch(q);
    });
    dir
}

fn delvec_run(args: &[&str]) -> (i32, String) {
    let o = std::process::Command::new(env!("CARGO_BIN_EXE_delvec"))
        .args(args)
        .output()
        .expect("delvec runs");
    (
        o.status.code().unwrap_or(-1),
        format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        ),
    )
}

/// Criterion 7: every aftermath effect of the scene is listed with its
/// observable — a block at its cell, the clock — or `emission-only`, and the
/// path's terminal step and the ending's PackTest carry them.
#[test]
fn the_ledger_lists_each_aftermath_effect_with_its_observable() {
    let dir = reach_copy("aftermath", |q| {
        q["content"]["quests"][2]["on_complete"]
            .as_array_mut()
            .unwrap()
            .extend([
                json!({"type": "set-block", "anchor": "anchor/exit", "block": "minecraft:moss_block"}),
                json!({"type": "give-item", "item": "minecraft:bread", "count": 1}),
                json!({"type": "set-time", "time": "noon"}),
            ]);
    });
    let out = dir.with_extension("out");
    let prefabs = common::prefabs_dir();
    let (code, log) = delvec_run(&[
        "build",
        dir.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--prefabs",
        prefabs.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{log}");
    let ledger: Value = serde_json::from_str(
        &std::fs::read_to_string(out.join("validation/endings.json")).unwrap(),
    )
    .unwrap();
    let home = &ledger["endings"][0];
    assert_eq!(home["id"], "ending/home");
    let after: Vec<(&str, Value)> = home["aftermath"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| (a["verb"].as_str().unwrap(), a["observable"].clone()))
        .collect();
    assert_eq!(after.len(), 3, "{after:?}");
    assert_eq!(after[0].0, "set-block");
    assert_eq!(after[0].1["kind"], "block");
    assert_eq!(after[0].1["block"], "minecraft:moss_block");
    assert_eq!(after[0].1["first"], after[0].1["last"]);
    assert_eq!(after[1], ("give-item", json!("emission-only")));
    assert_eq!(after[2].1, json!({"kind": "time", "daytime": 6000}));
    // The bot's half: the exported path's terminal step carries them.
    let path: Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("critical-path.json")).unwrap())
            .unwrap();
    let last = path["steps"].as_array().unwrap().last().unwrap().clone();
    assert_eq!(last["aftermath"].as_array().unwrap().len(), 3, "{last}");
    // The PackTest half: one template per (ending, world that reaches it).
    let worlds = home["worlds"].as_array().unwrap().len();
    assert_eq!(
        home["packtests"].as_array().unwrap().len(),
        worlds,
        "{home}"
    );
    for t in home["packtests"].as_array().unwrap() {
        let name = t["template"].as_str().unwrap();
        let body = std::fs::read_to_string(out.join(format!(
            "packtest-datapack/data/hello-world/test/{name}.mcfunction"
        )))
        .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(body.contains("assert block"), "{body}");
        assert!(body.contains("time query daytime"), "{body}");
        assert!(
            body.contains("function hello-world:complete_q_end"),
            "{body}"
        );
    }
}

/// A `campaign-complete` nested under a timeline owes its PackTests exactly as
/// one at the root of its bundle does, and its final scene is the whole bundle
/// the act fires — so an epilogue line spoken before the banner, in the root
/// list, is one of the ending's inputs.
#[test]
fn a_nested_ending_owes_its_templates_and_reads_its_whole_bundle() {
    let dir = reach_copy("nested", |q| {
        q["content"]["quests"][2]["on_complete"] = json!([
            {"type": "narrate", "style": "chat", "text": "Counted.",
             "when": {"requires_state": [{"op": "at-least", "state": "state/tally", "value": 1}]}},
            {"type": "sequence", "steps": [{"at_ticks": 20, "effects": [
                {"type": "set-block", "anchor": "anchor/exit", "block": "minecraft:moss_block"},
                cc("ending/home", Value::Null)
            ]}]}
        ]);
    });
    let out = dir.with_extension("out");
    let prefabs = common::prefabs_dir();
    let (code, log) = delvec_run(&[
        "build",
        dir.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--prefabs",
        prefabs.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{log}");
    let ledger: Value = serde_json::from_str(
        &std::fs::read_to_string(out.join("validation/endings.json")).unwrap(),
    )
    .unwrap();
    let home = &ledger["endings"][0];
    assert_eq!(
        home["path"],
        "/content/quests/2/on_complete/1/steps/0/effects/1"
    );
    assert_eq!(home["scene"], "/content/quests/2/on_complete");
    let keys: Vec<&str> = home["inputs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["key"].as_str().unwrap())
        .collect();
    assert_eq!(keys, vec!["state/tally"], "{home}");
    let worlds = home["worlds"].as_array().unwrap().len();
    assert!(worlds > 0, "{home}");
    let templates = home["packtests"].as_array().unwrap();
    assert_eq!(templates.len(), worlds, "{home}");
    for t in templates {
        let name = t["template"].as_str().unwrap();
        let body = std::fs::read_to_string(out.join(format!(
            "packtest-datapack/data/hello-world/test/{name}.mcfunction"
        )))
        .unwrap_or_else(|e| panic!("{name}: {e}"));
        let run = body
            .find("function hello-world:complete_q_end")
            .expect(&body);
        let step = body.find("schedule clear hello-world:").expect(&body);
        let check = body.find("assert block").expect(&body);
        assert!(run < step && step < check, "{body}");
    }
}

/// The exported critical path carries only what one world shows. Two aftermath
/// lines read a tally an ambient trigger also moves, which the mainline's replay
/// cannot date; the path's terminal step lists the one its own story world (the
/// *none* world: nothing elective performed, the tally at 0) fires, and every
/// world path lists only its own.
#[test]
fn the_exported_path_carries_only_what_one_world_shows() {
    let dir = reach_copy("one-world", |q| {
        q["content"]["triggers"][0]["effects"]
            .as_array_mut()
            .unwrap()
            .push(json!({"type": "add-state", "state": "state/tally", "amount": 1}));
        q["content"]["quests"][2]["on_complete"]
            .as_array_mut()
            .unwrap()
            .extend([
                json!({"type": "set-block", "anchor": "anchor/exit", "block": "minecraft:moss_block",
                       "when": {"requires_state": [{"op": "at-least", "state": "state/tally", "value": 1}]}}),
                json!({"type": "set-block", "anchor": "anchor/exit", "block": "minecraft:mud",
                       "when": {"requires_state": [{"op": "at-most", "state": "state/tally", "value": 0}]}}),
            ]);
    });
    let out = dir.with_extension("out");
    let prefabs = common::prefabs_dir();
    let (code, log) = delvec_run(&[
        "build",
        dir.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--prefabs",
        prefabs.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{log}");
    let read = |p: &str| -> Value {
        serde_json::from_str(&std::fs::read_to_string(out.join(p)).unwrap()).unwrap()
    };
    let blocks = |path: &Value| -> Vec<String> {
        path["steps"].as_array().unwrap().last().unwrap()["aftermath"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|x| x["observable"]["block"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    };
    assert_eq!(
        blocks(&read("critical-path.json")),
        vec!["minecraft:mud".to_string()],
        "the mainline is the world that performs nothing: the tally stays at 0"
    );
    let plan = read("validation/branch-plan.json");
    let rows = plan["branches"].as_array().unwrap();
    assert!(rows.len() >= 2, "{plan}");
    for row in rows {
        let Some(file) = row["path"].as_str() else {
            continue;
        };
        let b = blocks(&read(&format!("validation/{file}")));
        assert_eq!(b.len(), 1, "{}: {b:?}", row["id"]);
    }
}

/// Build `tests/fixtures/elective-reach` with `patch`, and hand back each world
/// row with its chronicle and whether it performs the bell.
fn chronicles(tag: &str, patch: impl FnOnce(&mut Value)) -> Vec<(Value, String, bool)> {
    let dir = reach_copy(tag, patch);
    let out = dir.with_extension("out");
    let prefabs = common::prefabs_dir();
    let (code, log) = delvec_run(&[
        "build",
        dir.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--prefabs",
        prefabs.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{log}");
    let plan: Value = serde_json::from_str(
        &std::fs::read_to_string(out.join("validation/branch-plan.json")).unwrap(),
    )
    .unwrap();
    let rows: Vec<(Value, String, bool)> = plan["branches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            let file = row["chronicle"].as_str().unwrap();
            let md = std::fs::read_to_string(out.join("validation").join(file)).unwrap();
            let performs = row["participation"]["performs"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a == "trigger/bell");
            (row.clone(), md, performs)
        })
        .collect();
    assert!(
        rows.iter().any(|r| r.2) && rows.iter().any(|r| !r.2),
        "fixture premise: one world rings the bell and one does not"
    );
    rows
}

/// A world chronicle dates an elective ambient act's beat where the world
/// performs it: the bell, here a writer of the tally the final scene reads, is
/// an entry deed, so its `happening` is a `deed` line in front of the step it
/// is performed at; the world that leaves it alone has no bell line at all.
#[test]
fn a_world_chronicle_dates_the_deed_it_performs_and_omits_the_one_it_skips() {
    let rows = chronicles("chronicle-deed", |q| {
        q["content"]["triggers"][0]["effects"]
            .as_array_mut()
            .unwrap()
            .push(json!({"type": "add-state", "state": "state/tally", "amount": 1}));
    });
    for (row, md, performs) in rows {
        let storyline = md.split("## Endings reached").next().unwrap();
        assert!(!md.contains("## Ambient beats"), "{}:\n{md}", row["id"]);
        if performs {
            assert!(
                !row["entry_deeds"].as_array().unwrap().is_empty(),
                "fixture premise: the bell is an entry deed: {row}"
            );
            let bell = storyline
                .find("(deed `/content/triggers/0/effects/0`) — The bell rings.")
                .unwrap_or_else(|| panic!("{} dates no bell deed:\n{md}", row["id"]));
            // The entry deed lands in front of the walk out (the latest step
            // before the scene at which its gate holds, spec-0108 §4.1).
            let decided = storyline.find("The Keeper listens.").expect(&md);
            let out = storyline.find("obj/walk-out is reached.").expect(&md);
            assert!(
                decided < bell && bell < out,
                "{}: the bell is not dated in front of the step it is rung at:\n{md}",
                row["id"]
            );
        } else {
            assert!(!md.contains("The bell rings."), "{}:\n{md}", row["id"]);
        }
    }
}

/// An elective act a world performs with no entry deed (its flag is credited,
/// nothing it writes is read by an ending) has no position: it stays an
/// ambient beat in the world that performs it, and the world that skips it does
/// not list it.
#[test]
fn an_undated_elective_beat_is_listed_only_where_the_world_performs_it() {
    for (row, md, performs) in chronicles("chronicle-ambient", |_| {}) {
        assert_eq!(
            md.contains("## Ambient beats") && md.contains("The bell rings."),
            performs,
            "{}:\n{md}",
            row["id"]
        );
    }
}

/// Criterion 11: the binding line is printed on every validate, analyze and
/// build, every count stated, zeroes included — here a campaign whose ending's
/// scene carries no gate at all.
#[test]
fn the_binding_line_states_every_count_zeroes_included() {
    let dir = reach_copy("binding", |q| {
        q["content"]["quests"][2]["on_complete"] = json!([cc("ending/home", Value::Null)]);
        // Nothing reads the tally now, so nothing writes it either (`DW0502`).
        q["content"]["state"] = json!([]);
        q["content"]["quests"][1]["on_complete"] = json!([]);
    });
    let out = dir.with_extension("out");
    let prefabs = common::prefabs_dir();
    let p = prefabs.to_str().unwrap();
    let d = dir.to_str().unwrap();
    let line = "ending binding: 1 ending(s); 0 input(s) read, 0 seeded (0 with a deed among their \
                writers), 0 final-scene only; 2 story world(s) enumerated (0 unrealizable \
                target(s), 0 at the bound); 0 aftermath effect(s), 0 of them observable after the \
                scene.";
    for args in [
        vec!["validate", d, "--prefabs", p],
        vec!["analyze", d, "--prefabs", p],
        vec!["build", d, "--out", out.to_str().unwrap(), "--prefabs", p],
    ] {
        let (code, log) = delvec_run(&args);
        assert_eq!(code, 0, "{args:?}: {log}");
        assert!(
            log.contains(line),
            "{args:?} printed no binding line:\n{log}"
        );
        assert!(
            log.contains("ending/home: reads 0 input(s) — none; aftermath: 0 effect(s) []"),
            "{args:?}:\n{log}"
        );
    }
}

// ---------------------------------------------------------------------------
// Criterion 12 — the world order is canonical.
// ---------------------------------------------------------------------------

/// Two optional quests, each `add-state 1`, declared in `order`.
fn two_tallies(order: [&str; 2]) -> Campaign {
    let mut planned_rows = vec![planned("quest/decide", &[], true)];
    let mut quests = vec![decide_quest()];
    for id in order {
        planned_rows.push(planned(id, &["quest/decide"], false));
        quests.push(reach_quest(
            id,
            &format!("obj/{}", id.trim_start_matches("quest/")),
            Some("quest/decide"),
            json!([{"type": "add-state", "state": "state/tally", "amount": 1}]),
        ));
    }
    planned_rows.push(planned("quest/end", &["quest/decide"], true));
    let at = |op: &str, v: i32| json!({"requires_state": [{"state": "state/tally", "op": op, "value": v}]});
    quests.push(reach_quest(
        "quest/end",
        "obj/walk-out",
        Some("quest/decide"),
        json!([
            cc("ending/none", at("at-most", 0)),
            cc("ending/one", at("equals", 1)),
            cc("ending/two", at("at-least", 2)),
        ]),
    ));
    campaign(
        json!({"finale": "quest/end", "quests": planned_rows}),
        json!({"state": [{"id": "state/tally", "scope": "party"}], "quests": quests}),
    )
}

#[test]
fn permuting_two_optional_quests_keeps_every_world_id_and_path() {
    let a = two_tallies(["quest/alpha", "quest/beta"]);
    let b = two_tallies(["quest/beta", "quest/alpha"]);
    let (fa, fb) = (Flow::new(&a), Flow::new(&b));
    let worlds = |f: &Flow<'_>| {
        f.story()
            .worlds
            .iter()
            .enumerate()
            .map(|(k, w)| {
                let pt = f.story_playthrough(k);
                (
                    k,
                    w.performs.iter().cloned().collect::<Vec<_>>(),
                    pt.steps
                        .iter()
                        .map(|s| s.objective.clone())
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>()
    };
    let (wa, wb) = (worlds(&fa), worlds(&fb));
    assert_eq!(wa, wb);
    assert!(wa.len() >= 3, "{wa:?}");
    // And every ending is reached either way.
    let (d, _) = ending::check_bound(&a);
    assert!(codes(&d, "DW1030").is_empty(), "{d:#?}");
}

// ---------------------------------------------------------------------------
// Criterion 10, the compiler's half — the runner's input.
// ---------------------------------------------------------------------------

/// `tests/fixtures/ending-runs`: two endings on how often the bell (an elective
/// `use` trigger past the Keeper's door) was rung, an epilogue line on a candle
/// a shop sells. Four story worlds, and every one exports a path: the one that
/// rings the bell carries it as an entry deed, performed after the door is open,
/// and the two whose witness buys the candle perform the purchase by a mock step
/// (spec-0108 §6.5) — the shop's own `shop_pick` function, asserted by the flag
/// it sets.
#[test]
fn the_runner_fixture_exports_every_world_and_mocks_the_purchase() {
    let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("endings-runs.out");
    let _ = std::fs::remove_dir_all(&out);
    let fixture = common::compiler_fixtures_dir().join("ending-runs");
    let prefabs = common::prefabs_dir();
    let (code, log) = delvec_run(&[
        "build",
        fixture.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--prefabs",
        prefabs.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{log}");
    let plan: Value = serde_json::from_str(
        &std::fs::read_to_string(out.join("validation/branch-plan.json")).unwrap(),
    )
    .unwrap();
    let rows = plan["branches"].as_array().unwrap();
    assert_eq!(rows.len(), 4, "{plan:#}");
    assert!(
        rows.iter()
            .all(|r| r["path"].is_string() && r["path_reason"].is_null()),
        "{plan:#}"
    );
    let mut endings: Vec<&str> = rows
        .iter()
        .map(|r| r["endings"][0].as_str().unwrap())
        .collect();
    endings.sort_unstable();
    assert_eq!(
        endings,
        [
            "ending/rung",
            "ending/rung",
            "ending/silent",
            "ending/silent"
        ]
    );
    let read = |r: &Value| -> Value {
        serde_json::from_str(
            &std::fs::read_to_string(out.join("validation").join(r["path"].as_str().unwrap()))
                .unwrap(),
        )
        .unwrap()
    };
    // Each world that buys the candle performs the purchase by mock, in front of
    // the walk-out, and nothing else does.
    let buyers: Vec<&Value> = rows
        .iter()
        .filter(|r| {
            r["entry_deeds"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["act"] == "shop/stall/offer-0")
        })
        .collect();
    assert_eq!(buyers.len(), 2, "{plan:#}");
    for r in &rows[..] {
        let path = read(r);
        let mocks: Vec<&Value> = path["steps"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|s| s["action"] == "mock")
            .collect();
        if !buyers.iter().any(|b| b["id"] == r["id"]) {
            assert!(mocks.is_empty(), "{}: {mocks:?}", r["id"]);
            continue;
        }
        assert_eq!(mocks.len(), 1, "{}: {path:#}", r["id"]);
        let m = mocks[0];
        assert_eq!(m["act"], "shop/stall/offer-0");
        assert_eq!(m["kind"], "shop-offer");
        assert_eq!(m["function"], "hello-world:shop_pick_0_0");
        assert!(
            out.join("datapack/data/hello-world/function/shop_pick_0_0.mcfunction")
                .is_file()
        );
        let flag = |negate: bool| {
            json!([{"objective": "dw.f_candle", "holder": "#party", "min": 1, "max": 1,
                    "negate": negate}])
        };
        assert_eq!(m["before"], flag(true), "{m:#}");
        assert_eq!(m["after"], flag(false), "{m:#}");
        assert!(m.get("keep_out").is_none(), "a mock walks nowhere: {m:#}");
    }
    let walkable: Vec<&Value> = rows
        .iter()
        .filter(|r| !buyers.iter().any(|b| b["id"] == r["id"]))
        .collect();
    // The deed world's path rings the bell after the talk that opens the door.
    let rung = walkable
        .iter()
        .find(|r| r["endings"][0] == "ending/rung")
        .unwrap();
    assert_eq!(rung["entry_deeds"][0]["act"], "trigger/bell");
    let path: Value = serde_json::from_str(
        &std::fs::read_to_string(out.join("validation").join(rung["path"].as_str().unwrap()))
            .unwrap(),
    )
    .unwrap();
    let actions: Vec<String> = path["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            format!(
                "{} {}",
                s["action"].as_str().unwrap(),
                s["objective"]
                    .as_str()
                    .or(s["trigger"].as_str())
                    .unwrap_or("")
            )
        })
        .collect();
    assert_eq!(
        actions,
        [
            "select-class ",
            "talk-to obj/decide",
            "trigger trigger/bell",
            "reach obj/walk-out",
            "assert-complete "
        ]
    );
    // The bell stands inside the completion volume of `obj/walk-out`, armed
    // once the door opens and due only after the bell: the press is performed
    // from a cell outside it, and the step names the volume it keeps out of.
    let bell = &path["steps"][2];
    let walk_out = &path["steps"][3]["completion"];
    assert_eq!(bell["keep_out"][0]["objective"], "obj/walk-out", "{bell:#}");
    assert_eq!(bell["keep_out"][0]["lo"], walk_out["lo"]);
    assert_eq!(bell["keep_out"][0]["hi"], walk_out["hi"]);
    let stand: Vec<i64> = bell["stand"]
        .as_array()
        .unwrap_or_else(|| panic!("the bell step names no stand: {bell:#}"))
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect();
    let lo: Vec<i64> = walk_out["lo"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect();
    let hi: Vec<i64> = walk_out["hi"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect();
    // Outside by the selector's hitbox rule: a body anywhere in the stand cell
    // reaches 0.3 past it, so it misses the box only from a column more than one
    // cell clear of it on some horizontal axis.
    assert!(
        [0usize, 2]
            .iter()
            .any(|&i| stand[i] < lo[i] - 1 || stand[i] > hi[i] + 1),
        "stand {stand:?} meets the volume {lo:?}..{hi:?}"
    );
    // The reach step itself walks INTO its own volume and keeps out of nothing.
    assert!(
        path["steps"][3].get("keep_out").is_none(),
        "{:#}",
        path["steps"][3]
    );
}

/// The same fixture with `obj/walk-out`'s radius widened to 3, so its volume
/// holds every cell the bell can be rung from: the deed world cannot ring it without
/// completing the walk-out first, and the build refuses it rather than export it
/// as walkable.
#[test]
fn a_deed_no_stance_outside_an_armed_volume_performs_is_refused() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("endings-runs-swallowed");
    let _ = std::fs::remove_dir_all(&dir);
    common::copy_dir_all(&common::compiler_fixtures_dir().join("ending-runs"), &dir);
    common::patch_file(&dir.join("quests.json"), |q| {
        q["content"]["quests"][1]["objectives"][0]["radius"] = json!(3);
    });
    let out = dir.with_extension("out");
    let _ = std::fs::remove_dir_all(&out);
    let prefabs = common::prefabs_dir();
    let (code, log) = delvec_run(&[
        "build",
        dir.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--prefabs",
        prefabs.to_str().unwrap(),
    ]);
    assert_ne!(code, 0, "{log}");
    assert!(
        log.contains("DW0311 [error]")
            && log.contains("trigger `trigger/bell`")
            && log.contains("completion volume of `obj/walk-out`"),
        "{log}"
    );
    assert!(!out.join("validation/world-path-3.json").exists());
}
