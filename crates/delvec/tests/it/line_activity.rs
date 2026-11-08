//! spec-0131: every line of the plan states what the player does on it. The
//! census counts each line's acts and prices them (`ActivityCensus`), and a
//! planned quest's `goal` opens on an act its line performs (`DW1079`).
//!
//! Read over hello-world with its one quest's objectives and the plan's goal
//! replaced, so each test states exactly the line it judges.

use crate::common;

use std::collections::BTreeSet;

use delvec::compiler::telling::{self, DW_LINE_UNTRUE, Lexicon};
use delvewright_dsl::metrics::{Metrics, Reads};
use delvewright_dsl::{Act, ActivityCensus, ActivityRates, Campaign, RawCampaign, parse_campaign};
use serde_json::{Value, json};

fn read(dir: &std::path::Path, name: &str) -> String {
    std::fs::read_to_string(dir.join(name)).unwrap()
}

/// hello-world whose one quest carries `objectives` and `effects` as its
/// `on_objective_complete`, and whose one planned quest has `goal`.
fn line(goal: &str, objectives: Vec<Value>, effects: Value) -> Campaign {
    let dir = common::hello_world_dir();
    let mut q: Value = serde_json::from_str(&read(&dir, "quests.json")).unwrap();
    q["content"]["quests"][0]["objectives"] = Value::Array(objectives);
    q["content"]["quests"][0]["on_objective_complete"] = effects;
    let mut p: Value = serde_json::from_str(&read(&dir, "quest-plan.json")).unwrap();
    p["content"]["quests"][0]["goal"] = json!(goal);
    parse_campaign(&RawCampaign {
        world: read(&dir, "world.json"),
        npcs: read(&dir, "npcs.json"),
        classes: read(&dir, "classes.json"),
        quest_plan: p.to_string(),
        quests: q.to_string(),
        dialogue: read(&dir, "dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    })
    .expect("campaign parses")
}

/// hello-world with its stage-5 quests emptied: the state step 3 leaves.
fn unexpanded(goal: &str) -> Campaign {
    let dir = common::hello_world_dir();
    let mut q: Value = serde_json::from_str(&read(&dir, "quests.json")).unwrap();
    q["content"]["quests"] = json!([]);
    let mut p: Value = serde_json::from_str(&read(&dir, "quest-plan.json")).unwrap();
    p["content"]["quests"][0]["goal"] = json!(goal);
    parse_campaign(&RawCampaign {
        world: read(&dir, "world.json"),
        npcs: read(&dir, "npcs.json"),
        classes: read(&dir, "classes.json"),
        quest_plan: p.to_string(),
        quests: q.to_string(),
        dialogue: read(&dir, "dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    })
    .expect("campaign parses")
}

fn talk(id: &str) -> Value {
    json!({"type": "talk-to", "id": id, "npc": "npc/keeper", "title": "Speak to the Keeper"})
}
fn reach(id: &str) -> Value {
    json!({"type": "reach-anchor", "id": id, "anchor": "anchor/exit", "radius": 2, "title": "Go to the door"})
}
fn kill(id: &str) -> Value {
    json!({"type": "kill", "id": id, "wave": "wave/guards", "title": "Clear the guard"})
}
fn collect(id: &str) -> Value {
    json!({"type": "collect", "id": id, "item": "minecraft:paper", "count": 1,
           "anchor": "anchor/exit", "title": "Take the roll"})
}
fn interact(id: &str) -> Value {
    json!({"type": "interact", "id": id, "anchor": "anchor/exit", "title": "Pull the lever"})
}

fn census(c: &Campaign) -> ActivityCensus {
    let table = Metrics::table();
    let mut reads = Reads::new();
    ActivityCensus::of(c, &table, &mut reads).expect("the table prices every act")
}

fn rates() -> ActivityRates {
    ActivityRates::read(&Metrics::table(), &mut Reads::new()).expect("the table prices every act")
}

const GOAL: &str = "/content/quests/0/goal";

// ---------------------------------------------------------------------------
// The census
// ---------------------------------------------------------------------------

/// Every act is priced, and the price list is read whole: a run that prices a
/// line has read all seven entries, so `DW0813` can name every seed.
#[test]
fn the_table_prices_every_act_and_the_census_reads_the_whole_list() {
    let table = Metrics::table();
    let mut reads = Reads::new();
    let r = ActivityRates::read(&table, &mut reads).expect("every act has a price");
    for act in Act::ALL {
        let key = format!("pacing.{}", ActivityRates::metric_name(act));
        assert!(
            reads.read().contains(key.as_str()),
            "the price of `{}` was not read: {:?}",
            act.as_str(),
            reads.read()
        );
    }
    assert_eq!(reads.binding().read, Act::ALL.len());
    // A reach is the walk, which the route coefficient prices.
    assert_eq!(r.of(Act::Reach), 0);
    assert!(r.of(Act::Talk) > 0 && r.of(Act::Take) > 0 && r.of(Act::Use) > 0);
}

/// One line, one objective per kind: the counts and the minutes are the
/// objectives' acts at the table's prices, each objective at its dearest act.
#[test]
fn the_census_counts_each_lines_acts_and_prices_them() {
    let c = line(
        "Hear the Keeper out.",
        vec![
            talk("obj/a"),
            reach("obj/b"),
            kill("obj/c"),
            collect("obj/d"),
            interact("obj/e"),
        ],
        json!({}),
    );
    let census = census(&c);
    let r = rates();
    assert_eq!(census.lines.len(), 1);
    assert!(census.unexpanded.is_empty());
    let l = &census.lines[0];
    assert_eq!(l.quest, "quest/open-the-door");
    assert_eq!(l.objectives, 5);
    for act in [Act::Talk, Act::Reach, Act::Kill, Act::Take, Act::Use] {
        assert_eq!(l.acts.get(&act), Some(&1), "{}", act.as_str());
    }
    assert_eq!(
        l.minutes,
        u64::from(
            r.of(Act::Talk) + r.of(Act::Reach) + r.of(Act::Kill) + r.of(Act::Take) + r.of(Act::Use)
        )
    );
    assert_eq!(census.minutes, l.minutes);
    assert_eq!(census.mandatory_minutes, l.minutes);
    assert!(!l.talk_or_reach_only());
    assert!(census.talk_or_reach_only().is_empty());
    let header = &census.lines()[0];
    assert!(header.contains("1 line(s) over 5 objective(s)"), "{header}");
    assert!(header.contains("0 line(s) talk-or-reach only"), "{header}");
}

/// An objective performing several acts is one objective at its dearest act,
/// counted under each act it performs: an `interact` with `requires_item` and
/// `takes_item` is use, present and give at once and costs one use.
#[test]
fn an_objective_of_several_acts_is_priced_once_at_its_dearest() {
    let mut o = interact("obj/lay");
    o["requires_item"] = json!("minecraft:paper");
    o["takes_item"] = json!(true);
    let c = line("Lay the roll on the counter.", vec![o], json!({}));
    let census = census(&c);
    let r = rates();
    let l = &census.lines[0];
    assert_eq!(l.objectives, 1);
    assert_eq!(l.acts.get(&Act::Use), Some(&1));
    assert_eq!(l.acts.get(&Act::Present), Some(&1));
    assert_eq!(l.acts.get(&Act::Give), Some(&1));
    assert_eq!(
        l.minutes,
        u64::from(r.of(Act::Use).max(r.of(Act::Present)).max(r.of(Act::Give)))
    );
}

/// A line of conversations and walks is stated as such: the shape a campaign
/// has when every line is "go somewhere and talk", counted so it is seen.
#[test]
fn a_talk_and_walk_line_is_named_in_the_census() {
    let c = line(
        "Hear the Keeper out and leave.",
        vec![talk("obj/a"), reach("obj/b"), talk("obj/c")],
        json!({}),
    );
    let census = census(&c);
    assert!(census.lines[0].talk_or_reach_only());
    assert_eq!(census.talk_or_reach_only().len(), 1);
    let header = &census.lines()[0];
    assert!(
        header.contains("1 line(s) talk-or-reach only (`quest/open-the-door`)"),
        "{header}"
    );
}

/// The step-3 state: a plan and no stage 5. The census counts the planned
/// quests as awaiting expansion, prices nothing and refuses nothing.
#[test]
fn an_unexpanded_plan_is_counted_and_not_priced() {
    let c = unexpanded("Take the roll from the loft.");
    let census = census(&c);
    assert!(census.lines.is_empty());
    assert_eq!(census.unexpanded, vec!["quest/open-the-door".to_string()]);
    assert_eq!(census.minutes, 0);
    assert!(census.lines()[0].contains("1 planned quest(s) awaiting stage 5"));
    let (d, b) = telling::check_line_acts(&c);
    assert!(d.is_empty(), "{d:?}");
    assert_eq!((b.lines, b.expanded, b.judged, b.refused), (1, 0, 0, 0));
    assert!(b.line().contains("1 awaiting stage 5"), "{}", b.line());
}

/// The ledger states its denominator as `examined`, and the price list.
#[test]
fn the_ledger_carries_the_census_with_its_denominator() {
    let c = line("Hear the Keeper out.", vec![talk("obj/a")], json!({}));
    let j = census(&c).to_json();
    assert_eq!(j["examined"], json!(1));
    assert_eq!(j["lines"][0]["quest"], json!("quest/open-the-door"));
    assert_eq!(j["lines"][0]["acts"]["talk"], json!(1));
    assert_eq!(j["lines"][0]["talk_or_reach_only"], json!(true));
    assert_eq!(j["rates"]["reach"], json!(0));
    assert_eq!(j["target_minutes"], json!(5));
}

// ---------------------------------------------------------------------------
// DW1079 — the goal says what the line does
// ---------------------------------------------------------------------------

/// The verdict over one line: `(refused paths, judged, unknown)`.
fn judge(c: &Campaign) -> (Vec<String>, usize, usize) {
    let (d, b) = telling::check_line_acts(c);
    assert!(d.iter().all(|x| x.code == DW_LINE_UNTRUE.id()));
    (
        d.into_iter().map(|x| x.path).collect(),
        b.judged,
        b.unknown.len(),
    )
}

/// A goal whose head names an act some objective of the line performs is
/// green on every kind, and a head of the union is enough: a line of a talk
/// and a reach accepts a talk word and a reach word.
#[test]
fn a_goal_that_names_an_act_the_line_performs_is_green() {
    let cases: Vec<(&str, Vec<Value>)> = vec![
        ("Speak to the Keeper about the door.", vec![talk("obj/a")]),
        ("Go through the door.", vec![reach("obj/a")]),
        ("Clear the guard from the hall.", vec![kill("obj/a")]),
        ("Take the roll from the loft.", vec![collect("obj/a")]),
        ("Pull the lever by the gate.", vec![interact("obj/a")]),
        (
            "Hear the Keeper out and leave.",
            vec![talk("obj/a"), reach("obj/b")],
        ),
        (
            "Leave the keep once the Keeper is heard.",
            vec![talk("obj/a"), reach("obj/b")],
        ),
    ];
    for (goal, objectives) in cases {
        let (refused, judged, unknown) = judge(&line(goal, objectives, json!({})));
        assert!(refused.is_empty(), "{goal}: {refused:?}");
        assert_eq!((judged, unknown), (1, 0), "{goal}");
    }
}

/// A goal whose head names an act no objective of the line performs is
/// refused at the goal's path, on every kind, with the census in the message.
#[test]
fn a_goal_that_promises_an_act_nobody_performs_is_refused() {
    let cases: Vec<(&str, Vec<Value>, &str)> = vec![
        ("Take the Keeper's roll.", vec![talk("obj/a")], "take"),
        ("Speak to the door.", vec![reach("obj/a")], "talk"),
        ("Gather the guard's keys.", vec![kill("obj/a")], "take"),
        (
            "Clear the loft of its rolls.",
            vec![collect("obj/a")],
            "kill",
        ),
        ("Fetch the lever's handle.", vec![interact("obj/a")], "take"),
        // The motivating shape: a line of conversations whose goal promises a
        // search.
        (
            "Gather the scattered chorus.",
            vec![talk("obj/a"), talk("obj/b"), talk("obj/c")],
            "take",
        ),
    ];
    for (goal, objectives, said) in cases {
        let c = line(goal, objectives, json!({}));
        let (d, b) = telling::check_line_acts(&c);
        assert_eq!(d.len(), 1, "{goal}: {d:?}");
        assert_eq!(d[0].code, DW_LINE_UNTRUE.id());
        assert_eq!(d[0].path, GOAL);
        assert_eq!(d[0].stage, "quest-plan");
        assert!(
            d[0].message.contains(&format!("→ {said}")),
            "{}",
            d[0].message
        );
        assert!(
            d[0].message.contains("re-head the goal"),
            "{}",
            d[0].message
        );
        assert!(d[0].message.contains("`obj/a` →"), "{}", d[0].message);
        assert_eq!((b.judged, b.refused), (1, 1), "{goal}");
    }
}

/// A head the lexicon does not know states no act the engine knows: counted
/// and listed, never refused.
#[test]
fn an_unknown_head_is_counted_and_listed_not_refused() {
    let c = line(
        "Convince the Keeper to unbar the gate.",
        vec![talk("obj/a")],
        json!({}),
    );
    let (refused, judged, unknown) = judge(&c);
    assert!(refused.is_empty());
    assert_eq!((judged, unknown), (0, 1));
    let b = telling::check_line_acts(&c).1;
    assert!(
        b.line().contains("1 head(s) the lexicon does not know"),
        "{}",
        b.line()
    );
    assert!(b.line().contains("Convince the Keeper"), "{}", b.line());
}

/// A `give-item` among a talk's completion effects makes the line a taking,
/// so a `take` head over it is true; without the effect it is refused.
#[test]
fn a_give_item_on_completion_lets_the_goal_say_take() {
    let effects = json!({"obj/a": [{"type": "give-item", "item": "minecraft:paper", "count": 1}]});
    let (refused, ..) = judge(&line(
        "Take the roll from the Keeper.",
        vec![talk("obj/a")],
        effects,
    ));
    assert!(refused.is_empty(), "{refused:?}");
    let (refused, ..) = judge(&line(
        "Take the roll from the Keeper.",
        vec![talk("obj/a")],
        json!({}),
    ));
    assert_eq!(refused, vec![GOAL.to_string()]);
}

/// The object-decided heads: `find` is reach, talk or take, so it is true of
/// a walk, a conversation and a taking, and false of a fight.
#[test]
fn an_object_decided_head_passes_on_every_act_in_its_set() {
    let lex = Lexicon::english();
    let (_, said) = lex.head("Find the Keeper").expect("`find` is a head");
    let expected: BTreeSet<Act> = [Act::Reach, Act::Talk, Act::Take].into_iter().collect();
    assert_eq!(*said, expected);
    for objectives in [
        vec![reach("obj/a")],
        vec![talk("obj/a")],
        vec![collect("obj/a")],
    ] {
        let (refused, ..) = judge(&line("Find the Keeper's roll.", objectives, json!({})));
        assert!(refused.is_empty(), "{refused:?}");
    }
    let (refused, ..) = judge(&line(
        "Find the Keeper's roll.",
        vec![kill("obj/a")],
        json!({}),
    ));
    assert_eq!(refused, vec![GOAL.to_string()]);
}

/// Perturbation toward the vacuous shape: a lexicon that knows no head judges
/// nothing and refuses nothing, and says so in its binding — a run whose
/// judged count is zero on a plan with goals is the zero the gate reads.
#[test]
fn an_empty_lexicon_judges_nothing_and_the_binding_says_so() {
    let c = line("Take the Keeper's roll.", vec![talk("obj/a")], json!({}));
    let empty = Lexicon::parse("{}").expect("an empty lexicon parses");
    let (d, b) = telling::check_line_acts_with(&c, &empty);
    assert!(d.is_empty());
    assert_eq!((b.expanded, b.judged, b.unknown.len()), (1, 0, 1));
    // The same line under the engine's lexicon is refused: the lexicon is the
    // only thing that moved.
    let (d, b) = telling::check_line_acts(&c);
    assert_eq!(d.len(), 1);
    assert_eq!(b.judged, 1);
}

/// The ledger rows the build writes beside the census.
#[test]
fn the_line_binding_ledger_states_judged_as_examined() {
    let c = line("Take the Keeper's roll.", vec![talk("obj/a")], json!({}));
    let j = telling::check_line_acts(&c).1.to_json();
    assert_eq!(j["code"], json!("DW1079"));
    assert_eq!(j["examined"], json!(1));
    assert_eq!(j["refused"], json!(1));
}

/// Every goal of the hello-world fixture, the gallery primary and every overlay
/// agrees with its line: the corpus the engine itself ships is green.
#[test]
fn the_shipped_corpus_is_green() {
    let root = common::repo_root();
    let dirs = [
        root.join("crates/dsl/fixtures/valid/hello-world"),
        root.join("gallery"),
        root.join("gallery/overlays/carcass"),
        root.join("gallery/overlays/site-plan"),
        root.join("gallery/overlays/site-plan-solid"),
    ];
    // An overlay is the primary plus the documents it carries; one it does not
    // carry is the primary's.
    let primary = root.join("gallery");
    for dir in dirs {
        let doc = |name: &str| {
            if dir.join(name).is_file() {
                read(&dir, name)
            } else {
                read(&primary, name)
            }
        };
        let raw = RawCampaign {
            world: doc("world.json"),
            npcs: doc("npcs.json"),
            classes: doc("classes.json"),
            quest_plan: doc("quest-plan.json"),
            quests: doc("quests.json"),
            dialogue: doc("dialogue.json"),
            world_edits: None,
            geometry_brief: None,
            layout_graph: None,
            site_plan: None,
            detail_plan: None,
            design: None,
        };
        let c = parse_campaign(&raw).unwrap_or_else(|e| panic!("{}: {e:?}", dir.display()));
        let (d, b) = telling::check_line_acts(&c);
        assert!(d.is_empty(), "{}: {d:?}", dir.display());
        assert!(b.judged > 0, "{}: no goal judged", dir.display());
    }
}

// ---------------------------------------------------------------------------
// The pacing projection reads the acts beside the route
// ---------------------------------------------------------------------------

/// `DW0822`'s projection, at validation, states the activity minutes and the
/// price list beside the route and sums them against `target_minutes`, on a
/// fixture that carries a layout graph and a site plan.
#[test]
fn the_pacing_projection_reads_the_route_and_the_acts_as_one_figure() {
    let dir = common::repo_root().join("crates/delvec/tests/fixtures/blockout");
    let opt = |name: &str| dir.join(name).is_file().then(|| read(&dir, name));
    let raw = RawCampaign {
        world: read(&dir, "world.json"),
        npcs: read(&dir, "npcs.json"),
        classes: read(&dir, "classes.json"),
        quest_plan: read(&dir, "quest-plan.json"),
        quests: read(&dir, "quests.json"),
        dialogue: read(&dir, "dialogue.json"),
        world_edits: None,
        geometry_brief: opt("geometry-brief.json"),
        layout_graph: opt("layout-graph.json"),
        site_plan: opt("site-plan.json"),
        detail_plan: None,
        design: None,
    };
    let c = parse_campaign(&raw).expect("the blockout fixture parses");
    let diags = delvewright_dsl::validate::validate_campaign(&c);
    let pacing = diags
        .iter()
        .find(|d| d.code == "DW0822")
        .expect("a campaign with a layout graph is projected");
    let census = census(&c);
    assert!(census.objectives > 0, "the fixture has objectives to price");
    assert!(
        pacing.message.contains(&format!(
            "the quests' acts add about {} minute(s) over {} objective(s) on {} line(s)",
            census.minutes,
            census.objectives,
            census.lines.len()
        )),
        "{}",
        pacing.message
    );
    assert!(
        pacing.message.contains("minute(s) in all"),
        "{}",
        pacing.message
    );
    assert!(
        pacing.message.contains(&census.rates.line()),
        "the price list is stated: {}",
        pacing.message
    );
    // Perturbation toward the vacuous shape: the same fixture with its stage-5
    // quests emptied prices nothing, and the line says so rather than summing.
    let mut q: Value = serde_json::from_str(&raw.quests).unwrap();
    q["content"]["quests"] = json!([]);
    let empty = parse_campaign(&RawCampaign {
        quests: q.to_string(),
        ..raw
    })
    .expect("a plan awaiting stage 5 parses");
    let diags = delvewright_dsl::validate::validate_campaign(&empty);
    let pacing = diags
        .iter()
        .find(|d| d.code == "DW0822")
        .expect("the projection stands on a plan awaiting stage 5");
    assert!(
        pacing
            .message
            .contains("about 0 minute(s) over 0 objective(s) on 0 line(s)"),
        "{}",
        pacing.message
    );
    assert!(
        pacing.message.contains("planned quest(s) not yet expanded"),
        "{}",
        pacing.message
    );
}
