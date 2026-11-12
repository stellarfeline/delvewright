//! spec-0137 — the journal keeps what was done. The compiler's half, proven on
//! emitted bytes: the surface in the schema; a recorded objective's finished
//! entry as one head page and one page per line the story can write, each its
//! own item modifier appended under its own held score; `record-outcome` as
//! the held score set and the epoch moved; the bytes of a campaign that
//! records nothing; a hidden task kept once it is over; `DW1104`; the pages
//! measured in every language; the ledger and the PackTest.
//!
//! The fixture is hello-world: talk to the keeper (`obj/talk`), then go
//! through the door he opens (`obj/exit`).

use crate::common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{parse_campaign, validate_campaign_with};
use serde_json::{Value, json};

const NS: &str = "hello-world";

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dw-records-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// hello-world with `quests` and `dialogue` patched.
fn fixture(
    tag: &str,
    quests: impl FnOnce(&mut Value),
    dialogue: impl FnOnce(&mut Value),
) -> PathBuf {
    let dir = scratch(tag);
    common::copy_dir_all(&common::hello_world_dir(), &dir);
    common::patch_file(&dir.join("quests.json"), quests);
    common::patch_file(&dir.join("dialogue.json"), dialogue);
    dir
}

fn outcome(objective: &str, line: &str) -> Value {
    json!({"type": "record-outcome", "objective": objective, "line": line})
}

/// The three-site door: a `record`, a line on the keeper's answer, a line in
/// `obj/talk`'s completion bundle, a line in the quest's `on_complete`, and a
/// minute's `limit` with its own line in `on_expire`.
fn three_ways(tag: &str) -> PathBuf {
    fixture(
        tag,
        |q| {
            let exit = &mut q["content"]["quests"][0]["objectives"][1];
            exit["record"] = json!("The keeper's door was the only way on.");
            exit["limit"] = json!({"ticks": 1200, "on_expire": [
                outcome("obj/exit", "We waited too long, and the door shut on us.")
            ]});
            common::objective_effects(q, 0, "obj/talk")
                .push(outcome("obj/exit", "He opened it the moment we asked."));
            q["content"]["quests"][0]["on_complete"]
                .as_array_mut()
                .unwrap()
                .insert(0, outcome("obj/exit", "We went through, and did not look back."));
        },
        |d| {
            d["content"]["dialogues"][0]["nodes"][0]["options"][1]["effects"]
                .as_array_mut()
                .unwrap()
                .insert(0, outcome("obj/exit", "We asked the keeper, politely."));
        },
    )
}

fn diagnostics(dir: &Path) -> Vec<delvewright_dsl::Diagnostic> {
    let loaded = load_campaign_dir(dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("fixture parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    validate_campaign_with(
        &campaign,
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    )
}

fn build(dir: &Path) -> BuildOutput {
    let errors: Vec<_> = diagnostics(dir)
        .into_iter()
        .filter(|d| d.severity == delvewright_dsl::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "the fixture must validate: {errors:#?}");
    let loaded = load_campaign_dir(dir).unwrap();
    let mut campaign = parse_campaign(&loaded.raw).expect("fixture parses");
    // As the build does: every authored string carries its key to the page.
    delvewright_dsl::l10n::tag_translatables(&mut campaign);
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
    .unwrap_or_else(|e| panic!("the fixture builds: {e:?}"))
}

fn text(out: &BuildOutput, path: &str) -> String {
    String::from_utf8(
        out.get(path)
            .unwrap_or_else(|| panic!("no `{path}` in the build"))
            .clone(),
    )
    .unwrap()
}

fn func(out: &BuildOutput, name: &str) -> String {
    text(out, &format!("datapack/data/{NS}/function/{name}.mcfunction"))
}

fn modifier_path(name: &str) -> String {
    format!("datapack/data/{NS}/item_modifier/{name}.json")
}

fn modifier(out: &BuildOutput, name: &str) -> Value {
    serde_json::from_str(&text(out, &modifier_path(name))).unwrap()
}

/// The translate keys a modifier's page carries, in order.
fn keys(v: &Value) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::Object(m) => {
                if let Some(k) = m.get("translate").and_then(Value::as_str) {
                    out.push(k.to_string());
                }
                for x in m.values() {
                    walk(x, out);
                }
            }
            Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    walk(v, &mut out);
    out
}

fn ends(keys: &[String], suffix: &str) -> bool {
    keys.iter().any(|k| k.ends_with(suffix))
}

/// Criterion 1: the surface in the exported schema.
#[test]
fn the_schema_carries_the_record_and_the_outcome() {
    let quests = delvewright_dsl::stage_schema(delvewright_dsl::Stage::Quests);
    let kinds = quests["$defs"]["Objective"]["oneOf"].as_array().unwrap();
    assert_eq!(kinds.len(), 5, "five objective kinds");
    for k in kinds {
        assert!(
            k["properties"].get("record").is_some(),
            "every kind carries `record`: {k}"
        );
    }
    let verb = quests["$defs"]["QuestEffect"]["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["properties"]["type"]["const"] == "record-outcome")
        .expect("the verb record-outcome is exported");
    for f in ["objective", "line"] {
        assert!(verb["properties"].get(f).is_some(), "{f}: {verb}");
        assert!(
            verb["required"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r == f),
            "{f} is required"
        );
    }
    let dialogue = delvewright_dsl::stage_schema(delvewright_dsl::Stage::Dialogue);
    let arm = dialogue["$defs"]["DialogueEffect"]["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["properties"]["type"]["const"] == "record-outcome")
        .expect("the dialogue arm is exported");
    assert!(arm["properties"].get("line").is_some());
}

/// Criterion 2 (amended): one head page and one page per line, each its own
/// committed modifier; the ordinals in walk order with the option's last.
#[test]
fn each_state_of_a_finished_entry_is_one_committed_page() {
    let dir = three_ways("pages");
    let out = build(&dir);
    let head = keys(&modifier(&out, "journal_done_exit"));
    assert!(ends(&head, ".title"), "the head carries the title: {head:?}");
    assert!(ends(&head, ".record"), "the head carries the record: {head:?}");
    // Walk order: `on_objective_complete[obj/talk]` (1), the quest's
    // `on_complete` (2), the limit's `on_expire` (3), the answer (4).
    let want = [
        (1, ".oc.talk.1.outcome"),
        (2, ".done.0.outcome"),
        (3, ".ox.exit.0.outcome"),
        (4, ".opt.1.fx.0.outcome"),
    ];
    let mut read = 0;
    for (k, suffix) in want {
        let page = keys(&modifier(&out, &format!("journal_done_exit.{k}")));
        assert_eq!(page.len(), 1, "line {k} is a page of its own: {page:?}");
        assert!(page[0].ends_with(suffix), "line {k}: {page:?}");
        read += 1;
    }
    assert!(!out.contains_key(&modifier_path("journal_done_exit.5")));
    assert_eq!(read + 1, 5, "5 of 5 files read");
    // An expired task is a finished entry like any other: no page of its own.
    assert!(
        !out.keys().any(|k| k.contains("journal_expired")),
        "no expired page exists"
    );
}

/// Criterion 3: the rewrite and the write.
#[test]
fn the_story_writes_the_way_and_the_book_follows() {
    let dir = three_ways("write");
    let out = build(&dir);
    let write = func(&out, "journal_write");
    let settled = "if score #party dw.os_exit matches 1";
    assert!(
        write.contains(&format!(
            "$execute {settled} run item modify entity @s $(slot) {NS}:journal_done_exit"
        )),
        "{write}"
    );
    for k in 1..=4 {
        assert!(
            write.contains(&format!(
                "$execute {settled} if score #jr_exit.{k} dw.guide matches 1 run item modify entity @s $(slot) {NS}:journal_done_exit.{k}"
            )),
            "line {k}: {write}"
        );
    }
    let writes = |f: &str, k: usize| {
        f.contains(&format!("scoreboard players set #jr_exit.{k} dw.guide 1"))
            && f.contains("scoreboard players add #epoch dw.guide 1")
    };
    let all: String = out
        .iter()
        .filter(|(p, _)| p.starts_with(&format!("datapack/data/{NS}/function/")))
        .map(|(_, b)| String::from_utf8_lossy(b).into_owned())
        .collect::<Vec<_>>()
        .join("\n");
    for k in 1..=4 {
        assert!(writes(&all, k), "site {k} is written somewhere");
    }
    let option = func(&out, "dlg_keeper_2");
    assert!(writes(&option, 4), "the answer writes its line: {option}");
    let setup = func(&out, "setup");
    for k in 1..=4 {
        assert!(
            setup.contains(&format!("scoreboard players set #jr_exit.{k} dw.guide 0")),
            "line {k} is seeded: {setup}"
        );
    }
}

/// Criterion 4: a campaign that records nothing keeps its journal's bytes.
#[test]
fn an_unrecorded_campaign_emits_what_it_emitted() {
    let dir = fixture("plain", |_| {}, |_| {});
    let out = build(&dir);
    let write = func(&out, "journal_write");
    let finished: Vec<&str> = write
        .lines()
        .filter(|l| l.contains("journal_done_"))
        .collect();
    assert_eq!(
        finished,
        vec![
            "$execute if score #party dw.o_talk matches 1 run item modify entity @s $(slot) hello-world:journal_done_talk",
            "$execute if score #party dw.o_exit matches 1 run item modify entity @s $(slot) hello-world:journal_done_exit",
        ],
        "{write}"
    );
    let done = modifier(&out, "journal_done_exit");
    assert_eq!(
        done["pages"][0]["extra"].as_array().unwrap().len(),
        3,
        "heading, gap, title: {done}"
    );
    assert!(!out.keys().any(|k| k.contains("journal_done_exit.1")));
    assert!(!out.contains_key("validation/journal-records.json"));
    assert!(!out.values().any(|b| String::from_utf8_lossy(b).contains("#jr_")));
    // Perturbation: one `record` adds that objective's head bytes and the ledger.
    let dir = fixture(
        "plain-rec",
        |q| q["content"]["quests"][0]["objectives"][1]["record"] = json!("A door."),
        |_| {},
    );
    let out = build(&dir);
    let done = modifier(&out, "journal_done_exit");
    assert!(ends(&keys(&done), ".record"), "{done}");
    assert!(out.contains_key("validation/journal-records.json"));
}

/// Criterion 5 (amended): a hidden task is hidden before it starts, shows
/// its hint once started, tells its completion, and is kept once over.
#[test]
fn a_task_nobody_announced_is_kept_once_done() {
    let hidden = |q: &mut Value, record: bool, hint: bool| {
        let exit = &mut q["content"]["quests"][0]["objectives"][1];
        exit["announcement"] = json!("hidden");
        if !hint {
            exit.as_object_mut().unwrap().remove("hint");
        }
        if record {
            exit["record"] = json!("The door was the only way on.");
        }
    };
    let dir = fixture("hidden", |q| hidden(q, true, false), |_| {});
    let out = build(&dir);
    assert!(!out.contains_key(&format!(
        "datapack/data/{NS}/function/announce_o_exit.mcfunction"
    )));
    assert!(!out.contains_key(&modifier_path("journal_open_exit")));
    assert!(out.contains_key(&modifier_path("journal_done_exit")));
    let complete = func(&out, "complete_o_exit");
    assert!(
        complete.contains("objective.complete") || complete.contains("Objective complete"),
        "the completion is told: {complete}"
    );
    // Not recorded and no hint: no page at all.
    let dir = fixture("hidden-bare", |q| hidden(q, false, false), |_| {});
    let out = build(&dir);
    assert!(!out.keys().any(|k| k.contains("journal_done_exit") || k.contains("journal_open_exit")));
    // A hint: an open page once it starts, still no announcement.
    let dir = fixture("hidden-hint", |q| hidden(q, false, true), |_| {});
    let out = build(&dir);
    assert!(out.contains_key(&modifier_path("journal_open_exit")));
    assert!(!out.contains_key(&format!(
        "datapack/data/{NS}/function/announce_o_exit.mcfunction"
    )));
}

fn records_check(dir: &Path) -> Vec<delvewright_dsl::Diagnostic> {
    let c = common::campaign_at(dir);
    delvec::compiler::recording::check(&c).0
}

fn has_1104(d: &[delvewright_dsl::Diagnostic]) -> usize {
    d.iter().filter(|x| x.code.as_str() == "DW1104").count()
}

/// A second quest whose one objective never opens: its gate reads a flag
/// nothing sets.
fn stranded(q: &mut Value) {
    q["content"]["quests"].as_array_mut().unwrap().push(json!({
        "id": "quest/stranded",
        "on_complete": [],
        "trigger": {"type": "campaign-start"},
        "happening": {"text": "the party hears of a far door", "verb": "learns"},
        "objectives": [{
            "type": "reach-anchor", "id": "obj/far", "anchor": "anchor/exit", "radius": 2,
            "requires_flags": ["flag/never"],
            "happening": {"text": "the party reaches the far door", "verb": "arrives"},
            "limit": {"ticks": 1200, "on_expire": [outcome("obj/far", "It closed before we came.")]}
        }]
    }));
}

/// Criterion 6: `DW1104`, three refused shapes and two passing.
#[test]
fn dw1104_a_way_no_story_reaches() {
    // Refused 1: a site behind a flag nothing sets.
    let dir = fixture(
        "w1",
        |q| {
            let mut e = outcome("obj/exit", "We forced it.");
            e["when"] = json!({"requires_flags": ["flag/never"]});
            common::objective_effects(q, 0, "obj/talk").push(e);
        },
        |_| {},
    );
    assert_eq!(has_1104(&records_check(&dir)), 1, "behind a flag nothing sets");
    // Refused 2 and 3: an `on_expire` of an objective no world expires, and a
    // site naming an objective that settles in no world.
    let dir = fixture(
        "w2",
        |q| {
            stranded(q);
            common::objective_effects(q, 0, "obj/talk")
                .push(outcome("obj/far", "We set out for the far door."));
        },
        |_| {},
    );
    let d = records_check(&dir);
    assert_eq!(has_1104(&d), 2, "{d:#?}");
    assert!(d.iter().any(|x| x.path.contains("/limit/on_expire")));
    assert!(d.iter().any(|x| x.path.contains("/on_objective_complete/")));
    // Passing: the same site ungated; a recorded objective with a way that
    // has no line.
    let dir = fixture(
        "w3",
        |q| {
            q["content"]["quests"][0]["objectives"][1]["record"] = json!("A door.");
            common::objective_effects(q, 0, "obj/talk").push(outcome("obj/exit", "We asked."));
        },
        |_| {},
    );
    assert_eq!(has_1104(&records_check(&dir)), 0);
    let dir = fixture(
        "w4",
        |q| {
            q["content"]["quests"][0]["objectives"][1]["limit"] = json!({"ticks": 1200});
            common::objective_effects(q, 0, "obj/talk").push(outcome("obj/exit", "We asked."));
        },
        |_| {},
    );
    assert_eq!(has_1104(&records_check(&dir)), 0, "the expiry has no line");
}

/// Criterion 7 (amended): every finished page measured, in every language;
/// an overlong record and an overlong line each warn `DW0330` naming the
/// objective and, for a line, its site.
#[test]
fn a_finished_page_fits_in_every_language() {
    let dir = three_ways("fit");
    let c = common::campaign_at(&dir);
    let (_, measured) = delvec::compiler::textfit::check_book_pages(&c, &BTreeMap::new());
    let finished = measured
        .iter()
        .filter(|(_, p, _)| p.starts_with("finished page of"))
        .count();
    // Σ over recorded objectives of (1 + sites) × languages = (1 + 4) × 1.
    assert_eq!(finished, 5, "{measured:#?}");
    let long = "We waited at the door until the light went grey and the birds went quiet. ".repeat(7);
    let dir = fixture(
        "fit-long",
        |q| {
            q["content"]["quests"][0]["objectives"][1]["record"] = json!(long.clone());
            common::objective_effects(q, 0, "obj/talk").push(outcome("obj/exit", &long));
        },
        |_| {},
    );
    let c = common::campaign_at(&dir);
    let (d, _) = delvec::compiler::textfit::check_book_pages(&c, &BTreeMap::new());
    let over: Vec<&String> = d
        .iter()
        .filter(|x| x.code.as_str() == "DW0330")
        .map(|x| &x.message)
        .collect();
    assert_eq!(over.len(), 2, "{over:#?}");
    assert!(over.iter().all(|m| m.contains("obj/exit")));
    assert!(over.iter().any(|m| m.contains("on_objective_complete")));
    assert!(over.iter().all(|m| m.contains("`en`")));
}

/// Criterion 8 (compiler half): one PackTest per recorded objective, its count
/// the ledger's.
#[test]
fn every_recorded_objective_owes_its_packtest() {
    let dir = three_ways("pt");
    let out = build(&dir);
    let ledger: Value =
        serde_json::from_str(&text(&out, "validation/journal-records.json")).unwrap();
    let n = ledger["objectives"].as_array().unwrap().len();
    let templates = out
        .keys()
        .filter(|k| k.contains("/test/guide_journal_record_"))
        .count();
    assert_eq!((n, templates), (1, 1));
    let t = text(
        &out,
        &format!("packtest-datapack/data/{NS}/test/guide_journal_record_exit.mcfunction"),
    );
    assert!(t.contains("function hello-world:journal_rewrite"), "{t}");
    assert!(t.contains("#jr_exit.4"), "{t}");
}

/// The ledger: every recorded objective, and per world what its walk writes.
#[test]
fn the_ledger_says_what_each_world_leaves_written() {
    let dir = three_ways("ledger");
    let out = build(&dir);
    let ledger: Value =
        serde_json::from_str(&text(&out, "validation/journal-records.json")).unwrap();
    let o = &ledger["objectives"][0];
    assert_eq!(o["objective"], "obj/exit");
    assert_eq!(o["outcomes"].as_array().unwrap().len(), 4);
    assert!(o["record_key"].as_str().unwrap().ends_with(".record"));
    let worlds = ledger["worlds"].as_array().unwrap();
    assert!(!worlds.is_empty());
    let rows: Vec<&Value> = worlds
        .iter()
        .flat_map(|w| w["finished"].as_array().unwrap())
        .collect();
    // The world that completes the door writes the answer, the talk's line
    // and the quest's; the world that lets it expire writes the expiry's.
    assert!(rows.iter().any(|r| r["settles"] == "done" && r["written"] == json!([1, 2, 4])
        || r["settles"] == "done" && r["written"] == json!([4, 1, 2])),
        "{rows:#?}");
    assert!(rows.iter().any(|r| r["settles"] == "expired"), "{rows:#?}");
    assert_eq!(
        ledger["counts"]["rows"].as_u64().unwrap() as usize,
        rows.len()
    );
}

/// The one numbering's keys are the inventory's keys.
#[test]
fn every_site_key_is_an_inventory_key() {
    let dir = three_ways("keys");
    let c = common::campaign_at(&dir);
    let inv: std::collections::BTreeSet<String> = delvewright_dsl::l10n::inventory(&c)
        .into_iter()
        .map(|(k, _)| k)
        .collect();
    let sites = delvewright_dsl::record::outcome_sites(&c);
    assert_eq!(sites.len(), 4);
    for s in &sites {
        assert!(inv.contains(&s.key), "{} is not in the inventory", s.key);
    }
    assert!(inv.contains("obj.open_the_door.exit.record") || inv.contains("obj.open-the-door.exit.record"));
}

/// The reviewer's table: every recorded objective, its record, each line
/// with its site and the worlds that end holding it.
#[test]
fn the_reviewers_table_counts_the_worlds_per_line() {
    let dir = three_ways("table");
    let c = common::campaign_at(&dir);
    let t = delvec::compiler::recording::review_table(&c);
    assert!(t[0].contains("1 recorded objective(s), 4 line(s)"), "{t:#?}");
    assert!(t.iter().any(|l| l.contains("record: The keeper's door")), "{t:#?}");
    assert_eq!(t.iter().filter(|l| l.contains("    line ")).count(), 4, "{t:#?}");
    assert!(t.iter().any(|l| l.contains("We waited too long") && l.contains("held in 1 of")), "{t:#?}");
    let none = fixture("table-none", |_| {}, |_| {});
    assert!(delvec::compiler::recording::review_table(&common::campaign_at(&none)).is_empty());
}
