//! spec-0131: every line of the plan states its activity. The census counts
//! each line's acts and prices them (`ActivityCensus`), and a planned quest's
//! declared `activity` is carried out by its expansion (`DW1079`).
//!
//! Read over hello-world with its one quest's objectives and the plan's
//! declaration replaced, so each test states exactly the line it judges.

use crate::common;

use std::collections::BTreeSet;

use delvewright_dsl::metrics::{Metrics, Reads};
use delvewright_dsl::quest::check::{ACTIVITY_NOT_CARRIED_OUT, check_activity};
use delvewright_dsl::{Act, ActivityCensus, ActivityRates, Campaign, RawCampaign, parse_campaign};
use serde_json::{Value, json};

fn read(dir: &std::path::Path, name: &str) -> String {
    std::fs::read_to_string(dir.join(name)).unwrap()
}

fn raw_with(plan: &Value, quests: &Value) -> RawCampaign {
    let dir = common::hello_world_dir();
    RawCampaign {
        world: read(&dir, "world.json"),
        npcs: read(&dir, "npcs.json"),
        classes: read(&dir, "classes.json"),
        quest_plan: plan.to_string(),
        quests: quests.to_string(),
        dialogue: read(&dir, "dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    }
}

/// hello-world whose one quest carries `objectives` and `effects` as its
/// `on_objective_complete`, and whose one planned quest declares `activity`.
fn line(activity: &[&str], objectives: Vec<Value>, effects: Value) -> Campaign {
    let dir = common::hello_world_dir();
    let mut q: Value = serde_json::from_str(&read(&dir, "quests.json")).unwrap();
    q["content"]["quests"][0]["objectives"] = Value::Array(objectives);
    q["content"]["quests"][0]["on_objective_complete"] = effects;
    let mut p: Value = serde_json::from_str(&read(&dir, "quest-plan.json")).unwrap();
    p["content"]["quests"][0]["activity"] = json!(activity);
    parse_campaign(&raw_with(&p, &q)).expect("campaign parses")
}

/// hello-world with its stage-5 quests emptied: the state step 3 leaves.
fn unexpanded(activity: &[&str]) -> Campaign {
    let dir = common::hello_world_dir();
    let mut q: Value = serde_json::from_str(&read(&dir, "quests.json")).unwrap();
    q["content"]["quests"] = json!([]);
    let mut p: Value = serde_json::from_str(&read(&dir, "quest-plan.json")).unwrap();
    p["content"]["quests"][0]["activity"] = json!(activity);
    parse_campaign(&raw_with(&p, &q)).expect("campaign parses")
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

const ACTIVITY: &str = "/content/quests/0/activity";

// ---------------------------------------------------------------------------
// The surface
// ---------------------------------------------------------------------------

/// `activity` is required on a planned quest, and its members are the act
/// vocabulary spelled in kebab-case; a plan without it, or with a word outside
/// the vocabulary, does not parse.
#[test]
fn the_declaration_is_required_and_closed() {
    let dir = common::hello_world_dir();
    let q: Value = serde_json::from_str(&read(&dir, "quests.json")).unwrap();
    let mut p: Value = serde_json::from_str(&read(&dir, "quest-plan.json")).unwrap();
    assert!(
        p["content"]["quests"][0]["activity"].is_array(),
        "hello-world declares its line"
    );
    p["content"]["quests"][0]
        .as_object_mut()
        .unwrap()
        .remove("activity");
    assert!(
        parse_campaign(&raw_with(&p, &q)).is_err(),
        "a plan with no `activity` parses"
    );
    p["content"]["quests"][0]["activity"] = json!(["search"]);
    assert!(
        parse_campaign(&raw_with(&p, &q)).is_err(),
        "a word outside the vocabulary parses"
    );
    for act in Act::ALL {
        p["content"]["quests"][0]["activity"] = json!([act.as_str()]);
        assert!(
            parse_campaign(&raw_with(&p, &q)).is_ok(),
            "`{}` is a member",
            act.as_str()
        );
    }
}

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
        &["talk", "reach", "kill", "take", "use"],
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
    let c = line(&["give"], vec![o], json!({}));
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
        &["talk", "reach"],
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
/// quests as awaiting expansion, prices nothing, and the declaration is not
/// judged and refuses nothing.
#[test]
fn an_unexpanded_plan_is_counted_and_not_priced() {
    let c = unexpanded(&["take"]);
    let census = census(&c);
    assert!(census.lines.is_empty());
    assert_eq!(census.unexpanded, vec!["quest/open-the-door".to_string()]);
    assert_eq!(census.minutes, 0);
    assert!(census.lines()[0].contains("1 planned quest(s) awaiting stage 5"));
    let (d, b) = check_activity(&c);
    assert!(d.is_empty(), "{d:?}");
    assert_eq!((b.lines, b.judged, b.refused), (1, 0, 0));
    assert!(b.line().contains("1 awaiting stage 5"), "{}", b.line());
}

/// The ledger states its denominator as `examined`, and the price list.
#[test]
fn the_ledger_carries_the_census_with_its_denominator() {
    let c = line(&["talk"], vec![talk("obj/a")], json!({}));
    let j = census(&c).to_json();
    assert_eq!(j["examined"], json!(1));
    assert_eq!(j["lines"][0]["quest"], json!("quest/open-the-door"));
    assert_eq!(j["lines"][0]["acts"]["talk"], json!(1));
    assert_eq!(j["lines"][0]["talk_or_reach_only"], json!(true));
    assert_eq!(j["rates"]["reach"], json!(0));
    assert_eq!(j["target_minutes"], json!(5));
}

// ---------------------------------------------------------------------------
// DW1079 — the declared activity is carried out
// ---------------------------------------------------------------------------

/// The verdict over one line: `(refused paths, judged, lines beyond their declaration)`.
fn judge(c: &Campaign) -> (Vec<String>, usize, usize) {
    let (d, b) = check_activity(c);
    assert!(d.iter().all(|x| x.code == ACTIVITY_NOT_CARRIED_OUT.id()));
    (
        d.into_iter().map(|x| x.path).collect(),
        b.judged,
        b.undeclared.len(),
    )
}

/// A declaration every act of which some objective of the line performs is
/// green on every kind, on a union of two, and on the richer shapes.
#[test]
fn a_declaration_the_line_carries_out_is_green() {
    let mut takes = interact("obj/lay");
    takes["requires_item"] = json!("minecraft:paper");
    takes["takes_item"] = json!(true);
    let cases: Vec<(&[&str], Vec<Value>)> = vec![
        (&["talk"], vec![talk("obj/a")]),
        (&["reach"], vec![reach("obj/a")]),
        (&["kill"], vec![kill("obj/a")]),
        (&["take"], vec![collect("obj/a")]),
        (&["use"], vec![interact("obj/a")]),
        (&["talk", "reach"], vec![talk("obj/a"), reach("obj/b")]),
        (&["use", "present", "give"], vec![takes]),
    ];
    for (activity, objectives) in cases {
        let (refused, judged, beyond) = judge(&line(activity, objectives, json!({})));
        assert!(refused.is_empty(), "{activity:?}: {refused:?}");
        assert_eq!((judged, beyond), (1, 0), "{activity:?}");
    }
}

/// A declared act no objective of the line performs is refused at the
/// declaration's path, on every kind, with the census in the message.
#[test]
fn a_declared_act_nobody_performs_is_refused() {
    let cases: Vec<(&[&str], Vec<Value>, &str)> = vec![
        (&["take"], vec![talk("obj/a")], "take"),
        (&["talk"], vec![reach("obj/a")], "talk"),
        (&["take"], vec![kill("obj/a")], "take"),
        (&["kill"], vec![collect("obj/a")], "kill"),
        (&["take"], vec![interact("obj/a")], "take"),
        (&["use", "present"], vec![interact("obj/a")], "present"),
        // The motivating shape: a line of conversations declared as a search.
        (
            &["take", "talk"],
            vec![talk("obj/a"), talk("obj/b"), talk("obj/c")],
            "take",
        ),
    ];
    for (activity, objectives, missing) in cases {
        let c = line(activity, objectives, json!({}));
        let (d, b) = check_activity(&c);
        assert_eq!(d.len(), 1, "{activity:?}: {d:?}");
        assert_eq!(d[0].code, ACTIVITY_NOT_CARRIED_OUT.id());
        assert_eq!(d[0].path, ACTIVITY);
        assert_eq!(d[0].stage, "quest-plan");
        assert!(
            d[0].message
                .contains(&format!("so the plan promises {missing} and")),
            "{}",
            d[0].message
        );
        assert!(d[0].message.contains("`obj/a` →"), "{}", d[0].message);
        assert_eq!((b.judged, b.refused), (1, 1), "{activity:?}");
    }
}

/// A line that does something declares what: an empty declaration over
/// objectives that perform acts is refused; over no objective it is not.
#[test]
fn an_empty_declaration_over_a_line_that_does_something_is_refused() {
    let c = line(&[], vec![talk("obj/a")], json!({}));
    let (d, b) = check_activity(&c);
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].path, ACTIVITY);
    assert!(
        d[0].message.contains("declares no activity"),
        "{}",
        d[0].message
    );
    assert_eq!(b.refused, 1);
    let c = line(&[], vec![], json!({}));
    let (d, _) = check_activity(&c);
    assert!(d.is_empty(), "{d:?}");
}

/// An act the line performs beyond its declaration is counted in the binding
/// and refused nothing: the check confirms a declaration, it never infers one.
#[test]
fn an_act_beyond_the_declaration_is_counted_not_refused() {
    let c = line(&["talk"], vec![talk("obj/a"), collect("obj/b")], json!({}));
    let (refused, judged, beyond) = judge(&c);
    assert!(refused.is_empty());
    assert_eq!((judged, beyond), (1, 1));
    let b = check_activity(&c).1;
    assert!(
        b.line().contains(
            "1 line(s) perform an act beyond their declaration (`quest/open-the-door` also take)"
        ),
        "{}",
        b.line()
    );
}

/// A `give-item` among a talk's completion effects makes the line a taking,
/// so `take` declared over it is carried out; without the effect it is not.
#[test]
fn a_give_item_on_completion_carries_out_a_declared_take() {
    let effects = json!({"obj/a": [{"type": "give-item", "item": "minecraft:paper", "count": 1}]});
    let (refused, ..) = judge(&line(&["talk", "take"], vec![talk("obj/a")], effects));
    assert!(refused.is_empty(), "{refused:?}");
    let (refused, ..) = judge(&line(&["talk", "take"], vec![talk("obj/a")], json!({})));
    assert_eq!(refused, vec![ACTIVITY.to_string()]);
}

/// Perturbation toward the vacuous shape: the one thing that moves between a
/// green run and a red one is the declaration, over the same expansion.
#[test]
fn the_declaration_alone_turns_the_verdict() {
    let objectives = || vec![talk("obj/a"), reach("obj/b")];
    let (d, b) = check_activity(&line(&["talk", "reach"], objectives(), json!({})));
    assert!(d.is_empty());
    assert_eq!((b.judged, b.refused), (1, 0));
    let (d, b) = check_activity(&line(&["talk", "reach", "take"], objectives(), json!({})));
    assert_eq!(d.len(), 1);
    assert_eq!((b.judged, b.refused), (1, 1));
}

/// The ledger rows the build writes beside the census.
#[test]
fn the_activity_binding_ledger_states_judged_as_examined() {
    let c = line(&["take"], vec![talk("obj/a")], json!({}));
    let j = check_activity(&c).1.to_json();
    assert_eq!(j["code"], json!("DW1079"));
    assert_eq!(j["examined"], json!(1));
    assert_eq!(j["refused"], json!(1));
}

/// The check runs inside the validation pass: a campaign whose declaration is
/// not carried out is refused by `validate_campaign`, not only by the check
/// called on its own.
#[test]
fn the_validation_pass_runs_the_activity_check() {
    let c = line(&["take"], vec![talk("obj/a")], json!({}));
    let codes: BTreeSet<String> = delvewright_dsl::validate::validate_campaign(&c)
        .into_iter()
        .map(|d| d.code)
        .collect();
    assert!(codes.contains("DW1079"), "{codes:?}");
}

/// Every declaration of the hello-world fixture, the gallery primary and every
/// overlay is carried out: the corpus the engine itself ships is green.
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
        let (d, b) = check_activity(&c);
        assert!(d.is_empty(), "{}: {d:?}", dir.display());
        assert!(b.judged > 0, "{}: no line judged", dir.display());
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
