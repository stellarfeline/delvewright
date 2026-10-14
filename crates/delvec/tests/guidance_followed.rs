//! Guidance a player can follow (spec-0104): every act is in sight or told
//! (`DW0863`), the text names the act its type completes on (`DW1015`), the
//! opening teaches each input (the key span, `DW1016`, the teaching line), a
//! pressable stands alone among its kind (`DW1017`), and a hint's names are
//! told (`DW0982`).
//!
//! Each acceptance criterion of spec-0104 that a cargo test can hold is held
//! here, against a hello-world-shaped campaign built from its plan, or against
//! a synthetic world and synthetic acts where the criterion asks for one.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use delvec::compiler::acts::{Act, ActWalk, ActWalks, ActWhat};
use delvec::compiler::blockstate::{BlockMap, BlockState};
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
/// sight unless a test removes a line. `teaching` is the campaign's block.
fn quests(teaching: Value) -> Value {
    json!({
        "campaign_id": NS, "stage": "quests",
        "content": {
            "guidance": { "teaching": teaching },
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
    json!({ "use-block": null, "use-body": null, "attack-body": null, "attack-block": null })
}

const USE_LINE: &str = "Things here answer to a hand: [[key|use]] on them.";

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
        class: None,
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

fn verb_refusals(c: &Campaign, sidecars: &BTreeMap<String, L10nDoc>) -> Vec<String> {
    let d = promise::check_act_verbs(c, sidecars).0;
    assert!(
        d.iter().all(|d| d.code == promise::DW_ACT_MISNAMED),
        "{d:?}"
    );
    d.into_iter()
        .filter(|d| d.path.contains("objectives/0") || d.path.contains("obj.open-the-door.talk."))
        .map(|d| d.message)
        .collect()
}

/// spec-0104 criterion 3: titles carry their kind's verb, hints never name
/// only another kind's act, in English.
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
    assert!(
        b.line()
            .contains("(1 string(s) in languages with no table)")
    );
}

// ---------------------------------------------------------------------------
// §5 — the key span, the teaching line, DW1016
// ---------------------------------------------------------------------------

/// spec-0104 criterion 4: a line carrying the key span lowers to a `with`
/// argument `{"keybind":"key.use"}` under a `%1$s` fallback; the width check
/// measures it at the default key's name; a zh-cn row that drops it is
/// `DW0976`; an unknown input is `DW0975`.
#[test]
fn a_key_span_lowers_measures_and_holds_its_translation() {
    let mut q = quests(nulls());
    q["content"]["guidance"]["teaching"]["use-block"] = json!(USE_LINE);
    let dir = variant("key-span", q, dialogue(GREETING));
    let out = try_build(&dir).expect("builds");
    let teach = fn_body(&out, "teach_use_block").expect("the teaching function");
    assert!(
        teach.contains(r#""with":[{"keybind":"key.use"}]"#),
        "{teach}"
    );
    assert!(teach.contains("answer to a hand: %1$s on them."), "{teach}");

    use delvec::compiler::textfit::{default_font_width, width_for};
    use delvewright_dsl::NarrateStyle;
    assert_eq!(
        width_for(NarrateStyle::Chat, "[[key|use]]"),
        default_font_width("Right Button")
    );
    assert_eq!(
        width_for(NarrateStyle::Chat, "[[key|attack]]"),
        default_font_width("Left Button")
    );

    let (_, c) = load(&dir);
    let key = "guidance.teaching.use-block";
    let mut s = BTreeMap::new();
    s.insert(
        "zh-cn".to_string(),
        sidecar("zh-cn", &[(key, "这里的东西认得手：对它们按右键。")]),
    );
    let d = delvewright_dsl::textstyle::validate_inline_styles(&c, &s);
    assert!(
        d.iter()
            .any(|d| d.code == delvewright_dsl::textstyle::INLINE_STYLE_UNMATCHED),
        "{d:?}"
    );
    s.insert(
        "zh-cn".to_string(),
        sidecar("zh-cn", &[(key, "这里的东西认得手：对它们按[[key|use]]。")]),
    );
    assert!(delvewright_dsl::textstyle::validate_inline_styles(&c, &s).is_empty());

    let mut q = quests(nulls());
    q["content"]["guidance"]["teaching"]["use-block"] = json!("Press [[key|fly]].");
    let (_, c) = load(&variant("key-fly", q, dialogue(GREETING)));
    let d = delvewright_dsl::textstyle::validate_inline_styles(&c, &BTreeMap::new());
    assert!(
        d.iter()
            .any(|d| d.code == delvewright_dsl::textstyle::INLINE_STYLE_MALFORMED),
        "{d:?}"
    );
}

/// spec-0104 criterion 5: the first `use-block` act's activation shows the
/// line under `dw.taught_use-block`, and the second's does not; a class with
/// `null` emits nothing; two builds are byte-identical.
#[test]
fn the_first_act_of_a_class_teaches_it_once() {
    let mut q = quests(nulls());
    q["content"]["guidance"]["teaching"]["use-block"] = json!(USE_LINE);
    let dir = variant("teach-objective", q.clone(), dialogue(GREETING));
    let out = try_build(&dir).expect("builds");
    let first = fn_body(&out, "activate_o_pull").expect("the lever's activation");
    assert!(
        first.contains(
            "execute as @a unless score @s dw.taught_use-block matches 1 run function hello-world:teach_use_block"
        ),
        "{first}"
    );
    let second = fn_body(&out, "activate_o_press").expect("the button's activation");
    assert!(!second.contains("teach_"), "{second}");
    let teach = fn_body(&out, "teach_use_block").unwrap();
    assert!(teach.contains("scoreboard players set @s dw.taught_use-block 1"));
    assert!(teach.contains("scoreboard players add #taught_use-block dw.sys 1"));
    assert!(
        fn_body(&out, "teach_use_body").is_none(),
        "a `null` class emits nothing"
    );
    // The lever's activation places its prop AND teaches: the teaching line is
    // asserted once-and-not-again by a template that drives that activation.
    let t = packtest(&out, "teach_objective_pull").expect("the placing site's PackTest");
    assert_eq!(
        t.matches("function hello-world:activate_o_pull").count(),
        2,
        "{t}"
    );
    assert!(
        t.contains("assert score #taught_use-block dw.sys matches 1.."),
        "{t}"
    );
    assert!(
        t.contains("if score #taught_use-block dw.sys = #tc0_pull dw.sys"),
        "{t}"
    );
    let again = try_build(&dir).expect("builds again");
    assert_eq!(out, again, "two builds are byte-identical");

    // With no line anywhere, nothing of the surface is emitted.
    let plain = try_build(&variant("teach-none", quests(nulls()), dialogue(GREETING))).unwrap();
    assert!(!plain.keys().any(|k| k.contains("teach")));
    assert!(
        !plain
            .values()
            .any(|v| std::str::from_utf8(v).is_ok_and(|s| s.contains("dw.taught_")))
    );
}

fn packtest<'a>(out: &'a BuildOutput, name: &str) -> Option<&'a str> {
    out.get(&format!(
        "packtest-datapack/data/{NS}/test/{name}.mcfunction"
    ))
    .map(|b| std::str::from_utf8(b).unwrap())
}

/// The function names `body` calls, by `function <ns>:<name>` anywhere in a line.
fn calls(body: &str) -> Vec<String> {
    let pre = format!("function {NS}:");
    body.lines()
        .filter_map(|l| l.split_once(&pre).map(|(_, r)| r))
        .map(|r| r.split_whitespace().next().unwrap_or("").to_string())
        .collect()
}

/// Every teaching site whose class `class` is taught by `activation`: the
/// PackTest templates that drive `activation`, reset `#taught_<class>` to 0 and
/// assert it at least 1 afterwards, and whether that assertion can hold — the
/// static call closure of `activation` reaches the one command that moves the
/// count. Returns (templates asserting it, assertion satisfiable).
fn teaching_assertion(out: &BuildOutput, class: &str, activation: &str) -> (usize, bool) {
    let drive = format!("function {NS}:{activation}");
    let reset = format!("scoreboard players set #taught_{class} dw.sys 0");
    let assert = format!("assert score #taught_{class} dw.sys matches 1..");
    let asserting = out
        .iter()
        .filter(|(k, _)| k.starts_with(&format!("packtest-datapack/data/{NS}/test/")))
        .filter_map(|(_, v)| std::str::from_utf8(v).ok())
        .filter(|t| t.contains(&drive) && t.contains(&reset) && t.contains(&assert))
        .count();
    let moves = format!("scoreboard players add #taught_{class} dw.sys 1");
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut todo = vec![activation.to_string()];
    let mut reached = false;
    while let Some(f) = todo.pop() {
        if !seen.insert(f.clone()) {
            continue;
        }
        let Some(body) = fn_body(out, &f) else {
            continue;
        };
        reached |= body.lines().any(|l| l.trim() == moves);
        todo.extend(calls(body));
    }
    (asserting, reached)
}

/// spec-0104 §5.4: a teaching line emitted inside an activation that also
/// places something is asserted by a PackTest, and dropping the line from that
/// activation turns the assertion unsatisfiable. A summoning activation's
/// template removes exactly what its two calls summoned.
#[test]
fn a_placing_activation_that_teaches_is_asserted() {
    let mut q = quests(nulls());
    q["content"]["guidance"]["teaching"]["use-block"] = json!(USE_LINE);
    let out = try_build(&variant("teach-placing", q, dialogue(GREETING))).expect("builds");
    let body = fn_body(&out, "activate_o_pull").unwrap().to_string();
    assert!(
        body.lines().any(|l| l.starts_with("setblock ")),
        "the activation places: {body}"
    );
    assert_eq!(
        teaching_assertion(&out, "use-block", "activate_o_pull"),
        (1, true)
    );

    // Perturbation: the teaching line dropped from the placing activation. The
    // template still asserts the count, and nothing it drives can move it.
    let mut cut = out.clone();
    let stripped: String = body
        .lines()
        .filter(|l| !l.contains("teach_use_block"))
        .map(|l| format!("{l}\n"))
        .collect();
    assert_ne!(
        stripped.len(),
        body.len(),
        "the perturbation removed a line"
    );
    cut.insert(
        format!("datapack/data/{NS}/function/activate_o_pull.mcfunction"),
        stripped.into_bytes(),
    );
    assert_eq!(
        teaching_assertion(&cut, "use-block", "activate_o_pull"),
        (1, false),
        "the template's assertion would fail"
    );

    // A summoning placement: the lever replaced by a lantern, a prop vanilla
    // reports no use of, so the activation places it and summons a hitbox over
    // it (still a `use-block` act); the template kills the two its two calls
    // made.
    let mut q = quests(nulls());
    q["content"]["guidance"]["teaching"]["use-block"] = json!(USE_LINE);
    q["content"]["quests"][0]["objectives"][1]["prop"] = json!({ "block": "minecraft:lantern" });
    let out = try_build(&variant("teach-summoning", q, dialogue(GREETING))).expect("builds");
    let body = fn_body(&out, "activate_o_pull").unwrap();
    assert_eq!(body.matches("summon ").count(), 1, "{body}");
    let t = packtest(&out, "teach_objective_pull").expect("the summoning site's PackTest");
    assert!(t.contains("kill @e[tag=dw_i_pull,limit=2]"), "{t}");
    assert_eq!(
        teaching_assertion(&out, "use-block", "activate_o_pull"),
        (1, true)
    );
}

/// spec-0104 criterion 5: a `use` trigger that is the first `use-block` act
/// emits a firing-cell poll that shows the line and marks the player, and its
/// PackTest stands a dummy within reach and asserts one line and the score.
#[test]
fn a_use_trigger_that_teaches_polls_its_firing_cells() {
    let mut q = quests(nulls());
    q["content"]["guidance"]["teaching"]["use-block"] = json!(USE_LINE);
    // The path performs the bell first: `obj/pull` waits on its flag.
    q["content"]["triggers"] = json!([{
        "id": "trigger/the-bell", "at": "spawn", "on": { "on": "use" },
        "prop": { "block": "minecraft:bell[attachment=floor,facing=north]" },
        "effects": [{ "type": "set-flag", "flag": "flag/rung" }]
    }]);
    q["content"]["quests"][0]["objectives"][1]["requires_flags"] = json!(["flag/rung"]);
    let out = try_build(&variant("teach-trigger", q, dialogue(GREETING))).expect("builds");
    let tick = fn_body(&out, "tick").unwrap();
    assert!(
        tick.contains("run function hello-world:teach_at_the_bell"),
        "{tick}"
    );
    let poll = fn_body(&out, "teach_at_the_bell").expect("the poll");
    assert!(
        poll.contains("as @a[distance=..3,tag=!dw_cutscene] unless score @s dw.taught_use-block matches 1 run function hello-world:teach_use_block"),
        "{poll}"
    );
    let t = std::str::from_utf8(
        out.get(&format!(
            "packtest-datapack/data/{NS}/test/teach_trigger_the_bell.mcfunction"
        ))
        .expect("the PackTest"),
    )
    .unwrap();
    assert!(t.contains("function hello-world:teach_at_the_bell"), "{t}");
    assert!(
        t.contains("assert score #taught_use-block dw.sys matches 1"),
        "{t}"
    );
}

fn teaching_refusals(name: &str, q: Value) -> (promise::TeachingBinding, Vec<String>) {
    let dir = variant(name, q, dialogue(GREETING));
    let (_, c) = load(&dir);
    let plan = Plan::build(&c, &prefabs()).expect("plan");
    let (b, r) = promise::check_teaching(&plan);
    assert!(
        r.iter().all(|f| f.code == promise::DW_INPUT_UNTAUGHT),
        "{r:?}"
    );
    (b, r.into_iter().map(|f| f.message).collect())
}

/// spec-0104 criterion 6: the three shapes of `DW1016`.
#[test]
fn teaching_and_need_must_agree() {
    // A: the path needs `use-block` and `teaching` says nothing of it.
    let mut t = nulls();
    t.as_object_mut().unwrap().remove("use-block");
    let (_, r) = teaching_refusals("teach-a", quests(t));
    assert_eq!(r.len(), 1, "{r:?}");
    assert!(
        r[0].contains("`use-block` is needed and never taught"),
        "{}",
        r[0]
    );
    // Silent with the line; silent with `null`, counted as taught by the level.
    let mut t = nulls();
    t["use-block"] = json!(USE_LINE);
    let (b, r) = teaching_refusals("teach-line", quests(t));
    assert!(r.is_empty(), "{r:?}");
    assert_eq!((b.lines, b.level), (1, 3));
    let (b, r) = teaching_refusals("teach-null", quests(nulls()));
    assert!(r.is_empty(), "{r:?}");
    assert_eq!((b.required, b.lines, b.level), (2, 0, 4), "{}", b.line());
    // B: a line nobody is owed.
    let mut t = nulls();
    t["attack-block"] = json!("Some stones want a blow: [[key|attack]] on them.");
    let (_, r) = teaching_refusals("teach-b", quests(t));
    assert_eq!(r.len(), 1, "{r:?}");
    assert!(
        r[0].contains("no walked path requires the class"),
        "{}",
        r[0]
    );
    // C: a line with no span, and one with the other input's span.
    for line in [
        "Things here answer to a hand.",
        "Things answer to [[key|attack]].",
    ] {
        let mut t = nulls();
        t["use-block"] = json!(line);
        let (_, r) = teaching_refusals("teach-c", quests(t));
        assert_eq!(r.len(), 1, "{line}: {r:?}");
        assert!(r[0].contains("does not say the key"), "{}", r[0]);
    }
}

// ---------------------------------------------------------------------------
// §6 — DW1017
// ---------------------------------------------------------------------------

fn twins(name: &str, q: Value, extra: &[([i32; 3], &str)], at: &str) -> Vec<String> {
    let dir = variant(name, q, dialogue(GREETING));
    let (_, c) = load(&dir);
    let plan = Plan::build(&c, &prefabs()).expect("plan");
    let cell = plan.point_any(at).expect("the anchor");
    let mut blocks: BlockMap = BlockMap::new();
    for (d, b) in extra {
        blocks.insert(
            [cell[0] + d[0], cell[1] + d[1], cell[2] + d[2]],
            BlockState::from(*b),
        );
    }
    let r = promise::check_pressables_alone(&plan, &blocks).1;
    assert!(
        r.iter()
            .all(|f| f.code == promise::DW_PRESSABLE_AMONG_TWINS),
        "{r:?}"
    );
    r.into_iter().map(|f| f.message).collect()
}

/// spec-0104 criterion 7.
#[test]
fn a_pressable_stands_alone_among_its_kind() {
    let lever = "minecraft:lever[face=floor,facing=north]";
    let r = twins(
        "twin-2",
        quests(nulls()),
        &[([2, 0, 0], lever)],
        "anchor/exit",
    );
    assert_eq!(r.len(), 1, "{r:?}");
    assert!(
        r[0].contains("at 2.0 blocks") && r[0].contains("`obj/pull`"),
        "{}",
        r[0]
    );
    assert!(
        twins(
            "twin-4",
            quests(nulls()),
            &[([4, 0, 0], lever)],
            "anchor/exit"
        )
        .is_empty()
    );
    let r = twins(
        "twin-state",
        quests(nulls()),
        &[([1, 0, 0], "minecraft:lever[face=wall,facing=east]")],
        "anchor/exit",
    );
    assert_eq!(r.len(), 1, "state is ignored: {r:?}");

    // An unmarked prop-less interact on an authored lamp beside a second lamp,
    // then the same marked.
    let lamp_q = |marker: Option<&str>| {
        let mut q = quests(nulls());
        let o = &mut q["content"]["quests"][0]["objectives"][1];
        o.as_object_mut().unwrap().remove("prop");
        o["title"] = json!("Light the lamp");
        if let Some(m) = marker {
            o["marker"] = json!(m);
        } else {
            o["marker"] = json!("hidden");
        }
        q
    };
    let lamps = [
        ([0, 0, 0], "minecraft:redstone_lamp"),
        ([0, 0, 2], "minecraft:redstone_lamp"),
    ];
    let r = twins("lamp", lamp_q(None), &lamps, "anchor/exit");
    assert_eq!(r.len(), 1, "{r:?}");
    assert!(twins("lamp-marked", lamp_q(Some("shown")), &lamps, "anchor/exit").is_empty());

    // A `strike` trigger over an authored block among its twins.
    let mut q = quests(nulls());
    q["content"]["triggers"] = json!([{
        "id": "trigger/the-stone", "at": "spawn", "on": { "on": "strike" },
        "effects": [{ "type": "set-flag", "flag": "flag/struck" }]
    }]);
    let r = twins(
        "strike",
        q,
        &[
            ([0, 0, 0], "minecraft:smooth_stone_slab"),
            ([1, 0, 1], "minecraft:smooth_stone_slab"),
        ],
        "spawn",
    );
    assert_eq!(r.len(), 1, "{r:?}");
    assert!(
        r[0].contains("`strike` trigger `trigger/the-stone`"),
        "{}",
        r[0]
    );
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
    assert!(
        promise::TeachingBinding::default()
            .line()
            .starts_with("DW1016 binding: 0 class(es) required over 0 walk(s)")
    );
    assert!(
        promise::TwinBinding::default()
            .line()
            .starts_with("DW1017 binding: 0 pressable(s)")
    );
}
