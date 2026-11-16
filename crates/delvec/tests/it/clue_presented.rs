//! spec-0143: a step's clue is presented before the step (`DW0863`), and a
//! clue names a text that exists (`DW1136`).
//!
//! The castle shape — four presses far apart and out of each other's sight,
//! their order told by a note — is built on the hello-world keep: the note is
//! an `interact` with a `record`, the fires are `use` triggers the path
//! performs to pay the flags the last objective requires. The sight tests
//! move the walls of a synthetic world around the plan's own cells.

use crate::common;

use std::collections::BTreeSet;
use std::path::PathBuf;

use delvec::compiler::acts::{self, ActWalks, ActWhat};
use delvec::compiler::clue;
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::nav::World;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, parse_campaign};
use serde_json::{Value, json};

const FIRES: [(&str, &str); 4] = [
    ("kitchen", "anchor/keeper-stand"),
    ("gate", "anchor/door"),
    ("hall", "anchor/latch"),
    ("lord", "anchor/exit"),
];

fn happening(text: &str) -> Value {
    json!({ "text": text, "verb": "learns" })
}

/// The note: an `interact` whose `record` is the line the journal keeps.
fn note(after: Value) -> Value {
    json!({ "type": "interact", "id": "obj/find-the-note", "anchor": "anchor/latch",
            "title": "Read the steward's note", "hint": "It lies by the wall.",
            "record": "The note gave the order of the watch fires: cook, porter, steward, lord.",
            "happening": happening("the party reads the note"), "after": after })
}

/// The objective the fires are lit for: it requires every fire's flag, so the
/// path performs the four triggers in front of it.
fn grate(id: &str, after: Value, extra_flags: &[&str]) -> Value {
    let mut flags: Vec<String> = FIRES
        .iter()
        .map(|(n, _)| format!("flag/fire-{n}"))
        .collect();
    flags.extend(extra_flags.iter().map(|f| (*f).to_string()));
    json!({ "type": "reach-anchor", "id": id, "anchor": "anchor/exit", "radius": 2,
            "title": "Go back to the grate", "hint": "Light the fires in the note's order.",
            "happening": happening("the grate lifts"),
            "requires_flags": flags, "after": after })
}

/// The four fires, each stating the clue its order rests on.
fn fires(clues: Value) -> Value {
    Value::Array(
        FIRES
            .iter()
            .map(|(n, at)| {
                json!({ "id": format!("trigger/fire-{n}"), "at": at, "on": { "on": "use" },
                        "clues": clues,
                        "effects": [{ "type": "set-flag", "flag": format!("flag/fire-{n}") }] })
            })
            .collect(),
    )
}

/// A side quest the finale does not depend on, holding `objectives`.
fn with_side_quest(dir: &std::path::Path, objectives: Value) {
    common::patch_file(&dir.join("quest-plan.json"), |p| {
        p["content"]["quests"].as_array_mut().unwrap().push(json!({
            "id": "quest/side", "act": 1, "activity": ["reach"], "area": "area/keep",
            "depends_on": [], "goal": "Poke about the keep.", "mandatory": false, "npcs": []
        }));
    });
    common::patch_file(&dir.join("quests.json"), |q| {
        q["content"]["quests"].as_array_mut().unwrap().push(json!({
            "id": "quest/side", "trigger": { "type": "campaign-start" },
            "happening": happening("the party pokes about"),
            "objectives": objectives, "on_complete": []
        }));
    });
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dw-clue-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    common::copy_dir_all(&common::hello_world_dir(), &dir);
    dir
}

/// The hello-world keep with `edit` applied to its main quest's objectives
/// and `triggers` declared.
fn keep(name: &str, triggers: Value, edit: impl FnOnce(&mut Vec<Value>, &mut Value)) -> PathBuf {
    let dir = scratch(name);
    common::patch_file(&dir.join("quests.json"), |q| {
        q["content"]["triggers"] = triggers;
        let quest = &mut q["content"]["quests"][0];
        let mut objectives = quest["objectives"].as_array().unwrap().clone();
        let mut bundles = quest["on_objective_complete"].clone();
        edit(&mut objectives, &mut bundles);
        quest["objectives"] = Value::Array(objectives);
        quest["on_objective_complete"] = bundles;
    });
    dir
}

fn load(dir: &std::path::Path) -> Campaign {
    let loaded = load_campaign_dir(dir).expect("the variant loads");
    parse_campaign(&loaded.raw).expect("the variant parses")
}

fn prefabs() -> PrefabRegistry {
    PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap()
}

fn walls(cells: &[[i32; 3]]) -> World {
    World::from_solid_cells(cells.iter().copied().collect::<BTreeSet<_>>())
}

/// The plan's own walks with the four fires moved 77 blocks apart, each in a
/// closed cell of its own, so no fire is in sight of anything.
fn far_apart(plan: &Plan) -> (ActWalks, World) {
    let mut walks = acts::walks(plan).clone();
    let mut cells = Vec::new();
    let mut n = 0;
    for w in &mut walks.walks {
        for a in &mut w.acts {
            if matches!(&a.what, ActWhat::Trigger(t) if t.starts_with("trigger/fire-")) {
                a.target = [1000 + 77 * n, 64, 1000];
                n += 1;
                for dx in -1..=1 {
                    for dy in -1..=2 {
                        for dz in -1..=1 {
                            if (dx, dy, dz) != (0, 0, 0) {
                                cells.push([a.target[0] + dx, a.target[1] + dy, a.target[2] + dz]);
                            }
                        }
                    }
                }
            }
        }
    }
    (walks, walls(&cells))
}

fn fire_targets(walks: &ActWalks) -> Vec<[i32; 3]> {
    walks.walks[0]
        .acts
        .iter()
        .filter(|a| matches!(&a.what, ActWhat::Trigger(t) if t.starts_with("trigger/fire-")))
        .map(|a| a.target)
        .collect()
}

fn record_clue() -> Value {
    json!([{ "by": "record", "objective": "obj/find-the-note" }])
}

/// spec-0143 criterion 2: the castle. Four presses 77 blocks apart, each
/// walled off from the others, their order told by a note the story forces
/// first (`after`). Nothing is refused, and the binding says what was judged.
#[test]
fn four_fires_out_of_sight_pass_when_the_note_is_forced_first() {
    let dir = keep("castle", fires(record_clue()), |objs, _| {
        objs.push(note(json!(["obj/talk"])));
        objs.push(grate(
            "obj/lift-the-grate",
            json!(["obj/find-the-note"]),
            &[],
        ));
    });
    let c = load(&dir);
    let plan = Plan::build(&c, &prefabs()).expect("plan");
    let (walks, world) = far_apart(&plan);
    let at = fire_targets(&walks);
    assert_eq!(at.len(), 4, "the path performs the four fires");
    // The shape the withdrawn rule refused: 77 blocks from fire to fire, and
    // each hidden from the one before it.
    for pair in at.windows(2) {
        assert_eq!(pair[1][0] - pair[0][0], 77);
        let why = clue::unseen(&world, pair[0], pair[1], false).expect("out of sight");
        assert!(why.contains("past the 64 blocks"), "{why}");
    }
    let (b, r) = clue::judge(&plan, &world, &walks);
    assert!(r.is_empty(), "{r:?}");
    assert_eq!(
        (
            b.stated,
            b.clues,
            b.main,
            b.optional,
            b.judged,
            b.text_presented,
            b.refused
        ),
        (4, 4, 4, 0, 4, 4, 0),
        "{}",
        b.line()
    );
    assert!(
        b.line().starts_with(
            "DW0863 binding: 4 of 8 act(s) state 4 clue(s) — 4 on the main line of 1 walk(s)"
        ),
        "{}",
        b.line()
    );
    let ledger = b.to_json();
    assert_eq!(ledger["examined"], 4);
    assert_eq!(ledger["rows"][0]["line"], "main");
    assert_eq!(
        ledger["rows"][0]["clues"][0]["verdicts"][0]["presented"],
        true
    );
}

/// spec-0143 criterion 3, first arm: the same castle with the note in a chest
/// off the main line — an optional quest's objective. Every fire is refused,
/// naming that the note is not on the main line.
#[test]
fn the_fires_red_when_the_note_is_in_an_optional_quest() {
    let dir = keep("optional-note", fires(record_clue()), |objs, _| {
        objs.push(grate("obj/lift-the-grate", json!(["obj/talk"]), &[]));
    });
    with_side_quest(&dir, json!([note(json!([]))]));
    let c = load(&dir);
    let plan = Plan::build(&c, &prefabs()).expect("plan");
    let (walks, world) = far_apart(&plan);
    let (b, r) = clue::judge(&plan, &world, &walks);
    assert_eq!((b.main, b.judged, b.refused), (4, 4, 4), "{}", b.line());
    assert_eq!(r.len(), 4);
    for f in &r {
        assert_eq!(f.code, clue::DW_CLUE_UNPRESENTED);
        assert!(
            f.message
                .contains("`obj/find-the-note` is not on the main line")
                && f.message.contains("optional detour"),
            "{}",
            f.message
        );
    }
    assert!(
        r[0].message.contains("/content/triggers/0/clues/0"),
        "{}",
        r[0].message
    );
}

/// spec-0143 criterion 3, second arm: the note on the main line, with nothing
/// that forces it before the fires — a run may light them first.
#[test]
fn the_fires_red_when_nothing_forces_the_note_first() {
    let dir = keep("unforced-note", fires(record_clue()), |objs, _| {
        objs.push(note(json!(["obj/talk"])));
        objs.push(grate("obj/lift-the-grate", json!(["obj/talk"]), &[]));
    });
    let c = load(&dir);
    let plan = Plan::build(&c, &prefabs()).expect("plan");
    let (walks, world) = far_apart(&plan);
    let (b, r) = clue::judge(&plan, &world, &walks);
    assert_eq!((b.main, b.refused), (4, 4), "{}", b.line());
    assert!(
        r.iter().all(|f| f.code == clue::DW_CLUE_UNPRESENTED
            && (f.message.contains("nothing forces that order")
                || f.message.contains("is performed after"))),
        "{r:?}"
    );
}

/// spec-0143 criterion 4: the optional-note shape as a side puzzle. The fires
/// are lit for an objective of the optional quest, so no walked path performs
/// them in front of a main-line objective: they are owed nothing, and counted.
#[test]
fn a_side_puzzle_is_owed_nothing() {
    let dir = keep("side-puzzle", fires(record_clue()), |_, _| {});
    with_side_quest(
        &dir,
        json!([note(json!([])), grate("obj/side-grate", json!([]), &[])]),
    );
    let c = load(&dir);
    let plan = Plan::build(&c, &prefabs()).expect("plan");
    let (b, r) = clue::check_presented(&plan, &walls(&[]));
    assert!(r.is_empty(), "{r:?}");
    assert_eq!(
        (b.stated, b.main, b.optional, b.judged, b.refused),
        (4, 0, 4, 0, 0),
        "{}",
        b.line()
    );
    assert!(
        b.line().contains("4 optional act(s) owed nothing"),
        "{}",
        b.line()
    );
    assert_eq!(b.to_json()["rows"][0]["line"], "optional");
}

/// spec-0143 criterion 5: a flag forces. The note's completion is the one
/// thing that sets a flag the grate requires, and no `after` names it: the
/// note is forced first. Give the flag a second setter and it forces nothing.
#[test]
fn the_sole_setter_of_a_required_flag_is_forced_first() {
    let shape = |name: &str, second_setter: bool| {
        let dir = keep(name, fires(record_clue()), |objs, bundles| {
            objs.push(note(json!(["obj/talk"])));
            objs.push(grate(
                "obj/lift-the-grate",
                json!(["obj/talk"]),
                &["flag/note-read"],
            ));
            bundles["obj/find-the-note"] =
                json!([{ "type": "set-flag", "flag": "flag/note-read" }]);
            if second_setter {
                bundles["obj/talk"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({ "type": "set-flag", "flag": "flag/note-read" }));
            }
        });
        let c = load(&dir);
        let plan = Plan::build(&c, &prefabs()).expect("plan");
        let (walks, world) = far_apart(&plan);
        let (b, r) = clue::judge(&plan, &world, &walks);
        (b, r)
    };
    let (b, r) = shape("flag-forces", false);
    assert!(r.is_empty(), "{r:?}");
    assert_eq!(
        (b.main, b.text_presented, b.refused),
        (4, 4, 0),
        "{}",
        b.line()
    );
    let (b, r) = shape("flag-two-setters", true);
    assert_eq!((b.main, b.refused), (4, 4), "{}", b.line());
    assert!(
        r.iter().all(|f| f.code == clue::DW_CLUE_UNPRESENTED),
        "{r:?}"
    );
}

/// A `hint` clue: the step's own hint is on its open page; the hint of an
/// objective forced first is read; the hint of an optional objective is not.
#[test]
fn a_hint_clue_is_presented_by_the_steps_own_page_or_a_forced_one() {
    let shape = |name: &str, of: &str, side: bool| {
        let dir = keep(name, json!([]), |objs, _| {
            let mut g = grate("obj/lift-the-grate", json!(["obj/talk"]), &[]);
            g.as_object_mut().unwrap().remove("requires_flags");
            g["clues"] = json!([{ "by": "hint", "objective": of }]);
            objs.push(g);
        });
        if side {
            with_side_quest(&dir, json!([note(json!([]))]));
        }
        let c = load(&dir);
        let plan = Plan::build(&c, &prefabs()).expect("plan");
        clue::check_presented(&plan, &walls(&[]))
    };
    let (b, r) = shape("hint-own", "obj/lift-the-grate", false);
    assert!(r.is_empty(), "{r:?}");
    assert_eq!((b.main, b.text_presented), (1, 1), "{}", b.line());
    let (b, r) = shape("hint-forced", "obj/talk", false);
    assert!(r.is_empty(), "{r:?}");
    assert_eq!(b.text_presented, 1, "{}", b.line());
    let (b, r) = shape("hint-optional", "obj/find-the-note", true);
    assert_eq!(b.refused, 1, "{}", b.line());
    assert!(
        r[0].message.contains("is not on the main line"),
        "{}",
        r[0].message
    );
}

/// spec-0143 criterion 6: a `sight` clue. Down a clear line from where the
/// main line stands it is presented; closed in a shell of solid cells it is
/// in sight from nowhere and refused naming the cell that stops the line; an
/// anchor with no cell is `DW1136`.
#[test]
fn a_sight_clue_is_presented_from_a_cell_the_main_line_stands_on() {
    let shape = |name: &str, of: &str| {
        let dir = keep(name, json!([]), |objs, _| {
            let last = objs.last_mut().unwrap();
            last["clues"] = json!([{ "by": "sight", "of": of }]);
        });
        load(&dir)
    };
    let c = shape("sight", "anchor/latch");
    let prefabs = prefabs();
    let plan = Plan::build(&c, &prefabs).expect("plan");
    let target = plan
        .point_any("anchor/latch")
        .expect("the anchor has a cell");

    let (b, r) = clue::check_presented(&plan, &walls(&[]));
    assert!(r.is_empty(), "{r:?}");
    assert_eq!(
        (b.main, b.sight_presented, b.refused),
        (1, 1, 0),
        "{}",
        b.line()
    );

    let mut shell = Vec::new();
    for dx in -1..=1 {
        for dy in -1..=1 {
            for dz in -1..=1 {
                if (dx, dy, dz) != (0, 0, 0) {
                    shell.push([target[0] + dx, target[1] + dy, target[2] + dz]);
                }
            }
        }
    }
    let (b, r) = clue::check_presented(&plan, &walls(&shell));
    assert_eq!((b.sight_presented, b.refused), (0, 1), "{}", b.line());
    assert_eq!(r[0].code, clue::DW_CLUE_UNPRESENTED);
    assert!(
        r[0].message.contains("is in sight from none of the") && r[0].message.contains("stopped"),
        "{}",
        r[0].message
    );

    let c = shape("sight-nowhere", "anchor/no-such-place");
    let plan = Plan::build(&c, &prefabs).expect("plan");
    let (_, r) = clue::check_presented(&plan, &walls(&[]));
    assert!(
        r.iter()
            .any(|f| f.code == clue::DW_CLUE_NAMES_NOTHING
                && f.message.contains("resolves to no cell")),
        "{r:?}"
    );
}

/// spec-0143 criterion 7: `DW1136`, each arm at the clue's own pointer, and
/// silent once the text is written.
#[test]
fn a_clue_names_a_text_that_exists() {
    let shape = |name: &str, clue_v: Value, strip: Option<&'static str>| {
        let dir = keep(name, fires(json!([clue_v])), |objs, _| {
            let mut n = note(json!(["obj/talk"]));
            if let Some(f) = strip {
                n.as_object_mut().unwrap().remove(f);
            }
            objs.push(n);
            objs.push(grate(
                "obj/lift-the-grate",
                json!(["obj/find-the-note"]),
                &[],
            ));
        });
        clue::check(&load(&dir))
    };
    let record = json!({ "by": "record", "objective": "obj/find-the-note" });
    let hint = json!({ "by": "hint", "objective": "obj/find-the-note" });

    let (d, b) = shape("names-ok", record.clone(), None);
    assert!(d.is_empty(), "{d:?}");
    assert_eq!(
        (b.acts, b.stated, b.clues, b.refused),
        (8, 4, 4, 0),
        "{}",
        b.line()
    );

    let (d, b) = shape("names-no-record", record, Some("record"));
    assert_eq!((d.len(), b.refused), (4, 4));
    assert_eq!(d[0].code, "DW1136");
    assert_eq!(d[0].path, "/content/triggers/0/clues/0");
    assert!(d[0].message.contains("has no `record`"), "{}", d[0].message);

    let (d, _) = shape("names-hint-ok", hint.clone(), None);
    assert!(d.is_empty(), "{d:?}");
    let (d, _) = shape("names-no-hint", hint, Some("hint"));
    assert_eq!(d.len(), 4);
    assert!(d[0].message.contains("has no `hint`"), "{}", d[0].message);

    let (d, _) = shape(
        "names-nobody",
        json!({ "by": "record", "objective": "obj/nobody" }),
        None,
    );
    assert_eq!(d.len(), 4);
    assert!(
        d[0].message.contains("no objective `obj/nobody` exists"),
        "{}",
        d[0].message
    );
}

/// spec-0143 criterion 8: the withdrawn rule. A step that states no clue is
/// owed nothing, wherever it stands: the four fires far apart and walled off,
/// with no clue, raise nothing.
#[test]
fn a_step_that_states_no_clue_is_owed_nothing() {
    let dir = keep("no-clue", fires(json!([])), |objs, _| {
        let mut n = note(json!(["obj/talk"]));
        n.as_object_mut().unwrap().remove("hint");
        objs.push(n);
        objs.push(grate(
            "obj/lift-the-grate",
            json!(["obj/find-the-note"]),
            &[],
        ));
    });
    let c = load(&dir);
    let plan = Plan::build(&c, &prefabs()).expect("plan");
    let (walks, world) = far_apart(&plan);
    assert_eq!(fire_targets(&walks).len(), 4);
    let (b, r) = clue::judge(&plan, &world, &walks);
    assert!(r.is_empty(), "{r:?}");
    assert_eq!((b.stated, b.judged, b.refused), (0, 0, 0), "{}", b.line());
    assert!(
        b.line()
            .ends_with("8 act(s) state no clue and are owed nothing"),
        "{}",
        b.line()
    );
}
