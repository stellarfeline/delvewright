//! Guidance a player can follow (spec-0104): every act is in sight or told
//! (`DW0863`), a title that does not name the act its type completes on is a
//! warning (`DW1015`), and a hint's names are told (`DW0982`).
//!
//! Each acceptance criterion of spec-0104 that a cargo test can hold is held
//! here, against a hello-world-shaped campaign built from its plan, or against
//! a synthetic world and synthetic acts where the criterion asks for one.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use delvec::compiler::acts::{Act, ActWalk, ActWalks, ActWhat};
use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::nav::World;
use delvec::compiler::plan::Plan;
use delvec::compiler::promise;
use delvec::compiler::registry::PrefabRegistry;
use delvec::compiler::telling;
use delvewright_dsl::l10n::L10nDoc;
use delvewright_dsl::{Campaign, parse_campaign};
use serde_json::{Value, json};

const NS: &str = "hello-world";

// ---------------------------------------------------------------------------
// The campaign under test
// ---------------------------------------------------------------------------

fn cast() -> Value {
    json!({ "npc/keeper": { "at": "anchor/keeper-stand", "dialogue": "dlg/greeting",
                            "doing": "keeping the gate" } })
}

fn dialogue(text: &str) -> Value {
    json!({
        "campaign_id": NS, "stage": "dialogue",
        "content": { "dialogues": [{
            "npc": "npc/keeper", "root": "dlg/greeting",
            "nodes": [{
                "id": "dlg/greeting",
                "text": text,
                "options": [{
                    "label": "Open the door, please.",
                    "effects": [{ "type": "complete-objective", "objective": "obj/talk" }]
                }]
            }]
        }] }
    })
}

const GREETING: &str = "Halt, traveler. This keep is mine to guard, and the door stays shut.";

/// Talk to the keeper, pull a lever at the exit, press a button at the
/// keeper's post — every one titled and hinted, so nothing here is refused for
/// sight unless a test removes a line. `guidance` is the campaign's block.
fn quests(guidance: Value) -> Value {
    json!({
        "campaign_id": NS, "stage": "quests",
        "content": {
            "guidance": guidance,
            "quests": [{
                "id": "quest/open-the-door",
                "trigger": { "type": "campaign-start" },
                "objectives": [
                    { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper",
                      "title": "Talk to the keeper", "hint": "He stands by the door." },
                    { "type": "interact", "id": "obj/pull", "anchor": "anchor/exit",
                      "prop": { "block": "minecraft:lever[face=floor,facing=north]" },
                      "title": "Pull the lever", "hint": "It is set in the floor by the door.",
                      "after": ["obj/talk"] },
                    { "type": "interact", "id": "obj/press", "anchor": "anchor/keeper-stand",
                      "prop": { "block": "minecraft:stone_button[face=floor,facing=north]" },
                      "title": "Press the button", "hint": "It is set in the floor at his post.",
                      "after": ["obj/pull"] }
                ],
                "on_objective_complete": {
                    "obj/talk": [{ "type": "open-gate", "anchor": "anchor/door" }]
                },
                "on_complete": [{ "type": "campaign-complete" }],
                "cast": cast()
            }]
        }
    })
}

fn nulls() -> Value {
    json!({})
}

fn variant(name: &str, mut quests: Value, mut dialogue: Value) -> PathBuf {
    let base = common::hello_world_dir();
    let npcs: Value =
        serde_json::from_str(&std::fs::read_to_string(base.join("npcs.json")).unwrap()).unwrap();
    common::declare_story(&mut quests, &npcs, &dialogue);
    common::declare_dialogue_story(&mut dialogue);
    let dst = std::env::temp_dir().join(format!("dw-guidance-{name}"));
    let _ = std::fs::remove_dir_all(&dst);
    common::materialize_from(
        &base,
        &json!({ "documents": { "quests": quests, "dialogue": dialogue } }),
        &dst,
    );
    dst
}

fn load(dir: &Path) -> (delvec::compiler::load::LoadedCampaign, Campaign) {
    let loaded = load_campaign_dir(dir).expect("variant loads");
    let campaign = parse_campaign(&loaded.raw).expect("variant parses");
    (loaded, campaign)
}

fn prefabs() -> PrefabRegistry {
    PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap()
}

fn try_build(dir: &Path) -> Result<BuildOutput, String> {
    let (loaded, mut campaign) = load(dir);
    // The strings as every real build emits them: tagged with their l10n key.
    delvewright_dsl::l10n::tag_translatables(&mut campaign);
    let prefabs = prefabs();
    let plan = Plan::build(&campaign, &prefabs).map_err(|e| format!("{e:?}"))?;
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
    .map_err(|e| match e {
        emit::BuildFailure::Diagnostic { code, message } => format!("{code}: {message}"),
        other => format!("{other:?}"),
    })
}

fn fn_body<'a>(out: &'a BuildOutput, name: &str) -> Option<&'a str> {
    out.get(&format!("datapack/data/{NS}/function/{name}.mcfunction"))
        .map(|b| std::str::from_utf8(b).unwrap())
}

// ---------------------------------------------------------------------------
// §3 — DW0863, every act
// ---------------------------------------------------------------------------

fn walls(cells: &[[i32; 3]]) -> World {
    World::from_solid_cells(cells.iter().copied().collect::<BTreeSet<_>>())
}

/// A wall across x = 5, three cells high, standing between the two ends of
/// every synthetic line below.
fn wall_at_x5() -> Vec<[i32; 3]> {
    let mut out = Vec::new();
    for y in 63..=67 {
        for z in -2..=2 {
            out.push([5, y, z]);
        }
    }
    out
}

fn one_walk(acts: Vec<Act>) -> ActWalks {
    ActWalks {
        walks: vec![ActWalk {
            label: "the test walk".into(),
            acts,
            gates: None,
        }],
    }
}

fn act(id: &str, what: ActWhat, kind: &'static str, from: [i32; 3], target: [i32; 3]) -> Act {
    Act {
        step: 1,
        id: id.into(),
        what,
        kind,
        from: Some(from),
        from_what: "the previous act's cell".into(),
        target,
        body: false,
    }
}

/// The plan of the campaign above with `obj/pull` stripped of its lines, so
/// arm 1 does not hold for it.
fn plan_with_untitled_pull(name: &str) -> (Campaign, PrefabRegistry, PathBuf) {
    let mut q = quests(nulls());
    let pull = &mut q["content"]["quests"][0]["objectives"][1];
    pull.as_object_mut().unwrap().remove("title");
    pull.as_object_mut().unwrap().remove("hint");
    let dir = variant(name, q, dialogue(GREETING));
    let (_, c) = load(&dir);
    (c, prefabs(), dir)
}

const PULL: ActWhat = ActWhat::Objective {
    quest: 0,
    objective: 1,
};

/// spec-0104 criterion 2 (a)–(d): an untitled `interact` down a clear corridor
/// is in sight and counted so; a wall between is refused naming the first
/// blocking cell; 65 blocks of clear line is refused naming the distance; the
/// same act announced with a hint passes by its lines.
#[test]
fn an_untitled_interact_is_found_in_sight_and_refused_behind_a_wall_or_too_far() {
    let (c, prefabs, _) = plan_with_untitled_pull("sight");
    let plan = Plan::build(&c, &prefabs).expect("plan");
    let a = |target| one_walk(vec![act("obj/pull", PULL, "interact", [0, 64, 0], target)]);

    let (b, r) = promise::judge_acts(&plan, &walls(&[]), &a([10, 64, 0]));
    assert!(r.is_empty(), "{r:?}");
    assert_eq!((b.acts, b.in_sight, b.refused), (1, 1, 0), "{}", b.line());
    assert!(
        b.line()
            .starts_with("DW0863 binding: 1 act(s) over 1 walk(s)")
    );

    let (b, r) = promise::judge_acts(&plan, &walls(&wall_at_x5()), &a([10, 64, 0]));
    assert_eq!(b.refused, 1);
    let msg = &r[0].message;
    assert_eq!(r[0].code, promise::DW_FIGHT_UNSIGNED);
    assert!(msg.contains("stopped first at [5, 65, 0]"), "{msg}");
    assert!(
        msg.contains("Give `obj/pull` a `title` and a `hint`"),
        "{msg}"
    );

    let (_, r) = promise::judge_acts(&plan, &walls(&[]), &a([65, 64, 0]));
    let msg = &r[0].message;
    assert!(
        msg.contains("65.0 blocks away, past the 64 blocks"),
        "{msg}"
    );

    let dir = variant("sight-announced", quests(nulls()), dialogue(GREETING));
    let (_, c2) = load(&dir);
    let prefabs2 = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan2 = Plan::build(&c2, &prefabs2).expect("plan");
    let (b, r) = promise::judge_acts(&plan2, &walls(&wall_at_x5()), &a([10, 64, 0]));
    assert!(r.is_empty(), "{r:?}");
    assert_eq!((b.announced, b.in_sight), (1, 0));
}

/// spec-0104 criterion 2 (e): a `use` trigger the path performs behind a wall
/// is refused, and its remedy is the trigger's — it has no lines.
#[test]
fn a_trigger_behind_a_wall_is_refused_with_the_triggers_remedy() {
    let (c, prefabs, _) = plan_with_untitled_pull("trigger-sight");
    let plan = Plan::build(&c, &prefabs).expect("plan");
    let w = one_walk(vec![act(
        "trigger/the-bell",
        ActWhat::Trigger("trigger/the-bell".into()),
        "use",
        [0, 64, 0],
        [10, 64, 0],
    )]);
    let (b, r) = promise::judge_acts(&plan, &walls(&wall_at_x5()), &w);
    assert_eq!((b.triggers, b.refused), (1, 1));
    assert!(
        r[0].message.contains("A trigger has no lines"),
        "{}",
        r[0].message
    );
    let (b, r) = promise::judge_acts(&plan, &walls(&[]), &w);
    assert!(r.is_empty());
    assert_eq!(b.in_sight, 1);
}

/// spec-0104 criterion 2 (f) and (g): a kill in sight at 20 blocks with a
/// reach of 16 is refused naming the reach, and passes at 12; a kill in reach
/// behind a wall is refused — the tightening, asserted.
#[test]
fn a_kill_is_found_only_in_sight_and_in_reach() {
    let open = walls(&[]);
    let why = promise::act_unfound(&open, [0, 64, 0], [20, 64, 0], true, Some((16.0, "test")));
    assert_eq!(why.len(), 1, "{why:?}");
    assert!(
        why[0].contains("past the wave's reach of 16 blocks"),
        "{why:?}"
    );
    assert!(
        promise::act_unfound(&open, [0, 64, 0], [12, 64, 0], true, Some((16.0, "test"))).is_empty()
    );
    let why = promise::act_unfound(
        &walls(&wall_at_x5()),
        [0, 64, 0],
        [10, 64, 0],
        true,
        Some((16.0, "test")),
    );
    assert_eq!(why.len(), 1, "in reach, and hidden: {why:?}");
    assert!(why[0].contains("hidden"), "{why:?}");
}

/// spec-0104 criterion 2: the first act of a path is judged from the entry
/// cell — moving the entry behind a wall moves the verdict.
#[test]
fn the_first_act_is_judged_from_the_entry_cell() {
    let mut q = quests(nulls());
    let talk = &mut q["content"]["quests"][0]["objectives"][0];
    talk.as_object_mut().unwrap().remove("title");
    talk.as_object_mut().unwrap().remove("hint");
    let dir = variant("entry", q, dialogue(GREETING));
    let (_, c) = load(&dir);
    let plan = Plan::build(&c, &prefabs()).expect("plan");
    let keeper = plan
        .critical_path
        .iter()
        .find(|s| s.objective() == Some("obj/talk"))
        .and_then(|s| s.pos())
        .expect("the talk step");
    // A wall three cells in front of the keeper, along z.
    let mut cells = Vec::new();
    for x in keeper[0] - 3..=keeper[0] + 3 {
        for y in keeper[1] - 1..=keeper[1] + 3 {
            cells.push([x, y, keeper[2] - 3]);
        }
    }
    let world = walls(&cells);
    let judge = |entry: [i32; 3]| {
        let w = delvec::compiler::acts::acts_of_from(
            &plan,
            "the critical path",
            &plan.critical_path,
            &plan.critical_path_transport,
            Some(entry),
        );
        let first = w.acts.first().cloned().expect("an act");
        assert_eq!(first.id, "obj/talk");
        assert_eq!(first.from_what, "the entry cell");
        promise::judge_acts(&plan, &world, &ActWalks { walks: vec![w] }).1
    };
    let near = [keeper[0], keeper[1], keeper[2] - 1];
    let far = [keeper[0], keeper[1], keeper[2] - 6];
    assert!(
        judge(near)
            .iter()
            .all(|f| !f.message.contains("`obj/talk`")),
        "the entry on the keeper's side sees him"
    );
    let r = judge(far);
    assert!(
        r.iter()
            .any(|f| f.message.contains("`obj/talk`") && f.message.contains("the entry cell")),
        "the entry behind the wall does not: {r:?}"
    );
}

// ---------------------------------------------------------------------------
// §4 — DW1015
// ---------------------------------------------------------------------------

fn titled(kind_obj: Value) -> Campaign {
    let mut q = quests(nulls());
    q["content"]["quests"][0]["objectives"][0] = kind_obj;
    let mut d = dialogue(GREETING);
    if q["content"]["quests"][0]["objectives"][0]["type"] != "talk-to" {
        d["content"]["dialogues"][0]["nodes"][0]["options"] = json!([{ "label": "Farewell." }]);
    }
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let dir = variant(&format!("verbs-{n}"), q, d);
    load(&dir).1
}

fn obj(kind: &str, title: Option<&str>, hint: Option<&str>) -> Value {
    let mut o = match kind {
        "talk-to" => json!({ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }),
        "reach-anchor" => {
            json!({ "type": "reach-anchor", "id": "obj/talk", "anchor": "anchor/exit", "radius": 2 })
        }
        "interact" => json!({ "type": "interact", "id": "obj/talk", "anchor": "anchor/exit" }),
        "kill" => json!({ "type": "kill", "id": "obj/talk", "wave": "wave/none" }),
        _ => json!({ "type": "collect", "id": "obj/talk", "item": "minecraft:stick",
                     "count": 1, "anchor": "anchor/exit" }),
    };
    if let Some(t) = title {
        o["title"] = json!(t);
    }
    if let Some(h) = hint {
        o["hint"] = json!(h);
    }
    o
}

/// The `DW1015` findings on the objective under test — every one a warning.
fn verb_refusals(c: &Campaign, sidecars: &BTreeMap<String, L10nDoc>) -> Vec<String> {
    let d = promise::check_act_verbs(c, sidecars).0;
    assert!(
        d.iter().all(|d| d.code == promise::DW_ACT_MISNAMED
            && d.severity == delvewright_dsl::Severity::Warning),
        "{d:?}"
    );
    d.into_iter()
        .filter(|d| d.path.contains("objectives/0") || d.path.contains("obj.open-the-door.talk."))
        .map(|d| d.message)
        .collect()
}

/// spec-0104 criterion 3: a title without its kind's verb, and a hint naming
/// only another kind's act, are `DW1015` warnings, in English.
#[test]
fn a_title_names_its_kinds_act() {
    let none = BTreeMap::new();
    let c = titled(obj("talk-to", Some("Talk to the miller"), None));
    assert!(verb_refusals(&c, &none).is_empty());
    let c = titled(obj("reach-anchor", Some("Talk to the miller"), None));
    let r = verb_refusals(&c, &none);
    assert_eq!(r.len(), 1, "{r:?}");
    assert!(r[0].contains("`talk` is a `talk-to` verb"), "{}", r[0]);
    let c = titled(obj("talk-to", Some("Go to the mill"), None));
    assert_eq!(verb_refusals(&c, &none).len(), 1);
    for kind in ["talk-to", "reach-anchor", "interact", "kill", "collect"] {
        let c = titled(obj(kind, Some("The Mill"), None));
        let r = verb_refusals(&c, &none);
        assert_eq!(r.len(), 1, "{kind}: {r:?}");
        assert!(r[0].contains("names no act"), "{}", r[0]);
        let own = match kind {
            "talk-to" => "Talk to the miller",
            "reach-anchor" => "Go to the mill",
            "interact" => "Ring the bell",
            "kill" => "Clear the yard",
            _ => "Take the stick",
        };
        let c = titled(obj(
            kind,
            Some(own),
            Some("Past the bell, left of the arch."),
        ));
        assert!(verb_refusals(&c, &none).is_empty(), "{kind}");
    }
    let c = titled(obj(
        "interact",
        Some("Ring the bell"),
        Some("Walk past the slate."),
    ));
    let r = verb_refusals(&c, &none);
    assert_eq!(r.len(), 1, "{r:?}");
    assert!(r[0].contains("hint"), "{}", r[0]);
    let c = titled(obj(
        "reach-anchor",
        Some("Go to the mill"),
        Some("Walk past the slate."),
    ));
    assert!(verb_refusals(&c, &none).is_empty());
}

fn sidecar(lang: &str, rows: &[(&str, &str)]) -> L10nDoc {
    serde_json::from_value(json!({
        "dsl_version": delvewright_dsl::DSL_VERSION,
        "campaign_id": NS,
        "kind": "l10n",
        "lang": lang,
        "content": rows.iter().map(|(k, v)| (k.to_string(), json!(v))).collect::<serde_json::Map<_, _>>(),
    }))
    .expect("a sidecar")
}

/// spec-0104 criterion 3, the zh-cn table and a language with none.
#[test]
fn a_sidecar_title_is_judged_by_its_own_table_or_counted_unjudged() {
    let c = titled(obj("talk-to", Some("Talk to the miller"), None));
    let key = "obj.open-the-door.talk.title";
    let mut s = BTreeMap::new();
    s.insert("zh-cn".to_string(), sidecar("zh-cn", &[(key, "去磨坊")]));
    let r = verb_refusals(&c, &s);
    assert_eq!(r.len(), 1, "{r:?}");
    assert!(r[0].contains("`去` is a `reach-anchor` verb"), "{}", r[0]);
    s.insert("zh-cn".to_string(), sidecar("zh-cn", &[(key, "找磨坊主")]));
    assert!(verb_refusals(&c, &s).is_empty());
    s.insert(
        "fr".to_string(),
        sidecar("fr", &[(key, "Parler au meunier")]),
    );
    let (_, b) = promise::check_act_verbs(&c, &s);
    assert_eq!((b.languages, b.unjudged), (2, 1), "{}", b.line());
    assert!(b.line().ends_with("; 0 warned"), "{}", b.line());
    assert!(
        b.line()
            .contains("(1 string(s) in languages with no table)")
    );
}

/// spec-0104 criterion 3: `delvec validate` exits 0 on a document whose only
/// finding is a `DW1015` warning, and prints the warning and its binding.
#[test]
fn a_misnamed_title_warns_and_validation_exits_zero() {
    for (n, (kind, title)) in [
        ("reach-anchor", "Talk to the miller"),
        ("talk-to", "Go to the mill"),
        ("talk-to", "The Mill"),
    ]
    .into_iter()
    .enumerate()
    {
        let mut q = quests(nulls());
        let o = &mut q["content"]["quests"][0]["objectives"][0];
        if kind == "reach-anchor" {
            *o = json!({ "type": "reach-anchor", "id": "obj/talk", "anchor": "anchor/keeper-stand",
                         "radius": 2, "title": title, "hint": "He stands by the door." });
        } else {
            o["title"] = json!(title);
        }
        let mut d = dialogue(GREETING);
        if kind != "talk-to" {
            d["content"]["dialogues"][0]["nodes"][0]["options"] = json!([{ "label": "Farewell." }]);
        }
        let dir = variant(&format!("verb-exit-{n}"), q, d);
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_delvec"))
            .arg("--prefabs")
            .arg(common::prefabs_dir())
            .arg("validate")
            .arg(&dir)
            .output()
            .expect("`delvec validate` runs");
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(out.status.code(), Some(0), "{title}: {text}");
        assert!(
            text.lines()
                .any(|l| l.starts_with("DW1015") && l.contains(title)),
            "{title}: {text}"
        );
        assert!(text.contains("; 1 warned"), "{title}: {text}");
    }
}

// ---------------------------------------------------------------------------
// §7 — DW0982, widened
// ---------------------------------------------------------------------------

/// spec-0104 criterion 8: a hint naming the Keep — a place nothing has told
/// yet — is refused naming the objective; once the keeper's line says what it
/// is, the same hint on a later objective passes; a title that introduces the
/// name by apposition passes.
#[test]
fn a_hint_names_only_what_the_party_has_been_told() {
    let mut q = quests(nulls());
    q["content"]["quests"][0]["objectives"][0]["hint"] = json!("He waits inside the Keep.");
    let (_, c) = load(&variant("told-early", q.clone(), dialogue(GREETING)));
    let (r, b) = telling::check_names_told_bound(&c);
    let r: Vec<_> = r
        .iter()
        .filter(|x| x.code == telling::DW_NAME_UNTOLD)
        .collect();
    assert_eq!(r.len(), 1, "{r:?}");
    assert!(
        r[0].message.contains("objective `obj/talk`"),
        "{}",
        r[0].message
    );
    assert!(b.objective_declared > 0 && b.line().contains("objective string(s)"));

    // The keeper says what the Keep is; the hint stands on the objective after
    // him.
    let mut q2 = quests(nulls());
    q2["content"]["quests"][0]["objectives"][1]["hint"] =
        json!("It is set in the floor of the Keep.");
    let told = "This is the Keep, the last tower on the moor, and the door stays shut.";
    let (_, c) = load(&variant("told-late", q2, dialogue(told)));
    let r = telling::check_names_told_bound(&c).0;
    assert!(r.iter().all(|x| x.code != telling::DW_NAME_UNTOLD), "{r:?}");

    // A title that introduces it.
    q["content"]["quests"][0]["objectives"][0]["title"] =
        json!("Talk to the keeper of the Keep, the last tower on the moor");
    q["content"]["quests"][0]["objectives"][0]["hint"] = json!("He stands by the door.");
    let (_, c) = load(&variant("told-apposition", q, dialogue(GREETING)));
    let r = telling::check_names_told_bound(&c).0;
    assert!(r.iter().all(|x| x.code != telling::DW_NAME_UNTOLD), "{r:?}");
}

// ---------------------------------------------------------------------------
// §8 — binding lines
// ---------------------------------------------------------------------------

/// spec-0104 criterion 9: each binding line states its zeroes.
#[test]
fn every_binding_line_states_its_zeroes() {
    assert_eq!(
        promise::ActBinding::default().line(),
        "DW0863 binding: 0 act(s) over 0 walk(s) — 0 announced with a hint, 0 in sight (0 kill(s) \
         also in reach), 0 trigger(s) judged by sight alone; 0 refused; 0 kill(s) on no walk judged \
         from the acts that fire their wave"
    );
    assert!(
        promise::ActVerbBinding::default()
            .line()
            .starts_with("DW1015 binding: 0 title(s)")
    );
}
