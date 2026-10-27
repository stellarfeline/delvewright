//! Guidance a player can follow (spec-0104): every act is in sight or told
//! (`DW0863`), a hint that names an act names its own (a `DW1045` warning),
//! and a hint's names are told (`DW0982`).
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
    let dst = std::env::temp_dir().join(format!("dw-guidance-{}-{name}", std::process::id()));
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
            passes: Vec::new(),
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
    // Credited by its lines at the step the path performs it, where the
    // replay holds it armed.
    let step = plan2
        .critical_path
        .iter()
        .position(|s| s.objective() == Some("obj/pull"))
        .expect("obj/pull is on the path");
    let announced = one_walk(vec![Act {
        step,
        ..act("obj/pull", PULL, "interact", [0, 64, 0], [10, 64, 0])
    }]);
    let (b, r) = promise::judge_acts(&plan2, &walls(&wall_at_x5()), &announced);
    assert!(r.is_empty(), "{r:?}");
    assert_eq!((b.announced, b.in_sight), (1, 0));
}

/// The walk back to an announced objective whose gate opens only at its own
/// door: `obj/press` waits on a flag that only an `approach` trigger at the
/// keeper's post sets. The replay holds it armed at its own step, after the
/// trigger the path performs in front of it, and not at the trigger's: so
/// its announcement is nowhere on screen while the party walks to the post.
/// That leg is judged — refused with the act out of sight, found with it in
/// sight.
#[test]
fn a_walk_to_an_objective_that_arms_at_its_own_door_is_judged() {
    let shape = |name: &str| {
        let mut q = quests(nulls());
        q["content"]["triggers"] = json!([{
            "id": "trigger/at-the-post",
            "at": "anchor/keeper-stand",
            "on": { "on": "approach", "range": 2 },
            "effects": [{ "type": "set-flag", "flag": "flag/at-the-post" }]
        }]);
        q["content"]["quests"][0]["objectives"][2]["requires_flags"] = json!(["flag/at-the-post"]);
        let dir = variant(name, q, dialogue(GREETING));
        let (_, c) = load(&dir);
        c
    };
    let prefabs = prefabs();
    let c = shape("arms-at-its-door");
    let plan = Plan::build(&c, &prefabs).expect("plan");
    let at = |id: &str| {
        plan.critical_path
            .iter()
            .position(|s| s.objective() == Some(id) || s.trigger() == Some(id))
            .unwrap_or_else(|| panic!("{id} is on the path"))
    };
    let (t, p) = (at("trigger/at-the-post"), at("obj/press"));
    assert!(
        t < p,
        "the path performs the trigger in front of the objective"
    );
    let armed = &plan.region_events.armed;
    assert!(armed[p].contains("obj/press"), "{armed:?}");
    assert!(!armed[t].contains("obj/press"), "{armed:?}");
    let press = ActWhat::Objective {
        quest: 0,
        objective: 2,
    };
    let walk = |t: usize, p: usize| ActWalks {
        walks: vec![ActWalk {
            label: "the test walk".into(),
            acts: vec![Act {
                step: p,
                ..act(
                    "obj/press",
                    press.clone(),
                    "interact",
                    [3, 64, 0],
                    [10, 64, 0],
                )
            }],
            passes: vec![delvec::compiler::acts::Pass {
                step: t,
                name: "`trigger/at-the-post`".into(),
                from: Some([0, 64, 0]),
                from_what: "the previous act's cell".into(),
                target: [3, 64, 0],
            }],
            gates: None,
        }],
    };
    let (b, r) = promise::judge_acts(&plan, &walls(&wall_at_x5()), &walk(t, p));
    assert_eq!(
        (b.passes, b.refused, b.announced),
        (1, 1, 1),
        "{}",
        b.line()
    );
    assert_eq!(r[0].code, promise::DW_FIGHT_UNSIGNED);
    assert!(
        r[0].message.contains("`trigger/at-the-post`")
            && r[0]
                .message
                .contains("(`obj/press`) are not yet armed on the way"),
        "{}",
        r[0].message
    );
    let (b, r) = promise::judge_acts(&plan, &walls(&[]), &walk(t, p));
    assert!(r.is_empty(), "{r:?}");
    assert_eq!(b.passes_in_sight, 1, "{}", b.line());
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
// §5 — a hint that names an act names its own (DW1045's hint arm)
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

/// The hint findings on the objective under test — every one a `DW1045`
/// warning at the hint's path.
fn hint_warnings(c: &Campaign) -> Vec<String> {
    let d = telling::check_hint_acts(c).0;
    assert!(
        d.iter().all(|d| d.code == telling::DW_ACT_UNTRUE
            && d.severity == delvewright_dsl::Severity::Warning
            && d.path.ends_with("/hint")),
        "{d:?}"
    );
    d.into_iter()
        .filter(|d| d.path.contains("objectives/0"))
        .map(|d| d.message)
        .collect()
}

/// spec-0104 §5, folded into `DW1045`: a hint naming only an act its
/// completion does not perform is a warning; a hint naming its own act, or no
/// act at all (a place), is not reported.
#[test]
fn a_hint_that_names_an_act_names_its_own() {
    let c = titled(obj(
        "interact",
        Some("Ring the bell"),
        Some("Walk past the slate."),
    ));
    let r = hint_warnings(&c);
    assert_eq!(r.len(), 1, "{r:?}");
    assert!(r[0].contains("`walk` → reach"), "{}", r[0]);
    let c = titled(obj(
        "reach-anchor",
        Some("Go to the mill"),
        Some("Walk past the slate."),
    ));
    assert!(hint_warnings(&c).is_empty());
    let c = titled(obj(
        "interact",
        Some("Ring the bell"),
        Some("Walk past the slate; pull the cord."),
    ));
    assert!(
        hint_warnings(&c).is_empty(),
        "a hint that also names its own act"
    );
    let c = titled(obj(
        "interact",
        Some("Ring the bell"),
        Some("At the far end of the nave; the hall's door stands open."),
    ));
    assert!(
        hint_warnings(&c).is_empty(),
        "an act word inside a clause is not its head"
    );
    for (kind, own) in [
        ("talk-to", "Talk to the miller"),
        ("reach-anchor", "Go to the mill"),
        ("interact", "Ring the bell"),
        ("kill", "Clear the yard"),
        ("collect", "Take the stick"),
    ] {
        let c = titled(obj(
            kind,
            Some(own),
            Some("Past the bell, left of the arch."),
        ));
        assert!(hint_warnings(&c).is_empty(), "{kind}: a place names no act");
    }
    let (_, b) = telling::check_hint_acts(&titled(obj(
        "interact",
        Some("Ring the bell"),
        Some("Walk past the slate."),
    )));
    assert_eq!(
        (b.hinted >= 1, b.naming >= 1, b.warned),
        (true, true, 1),
        "{}",
        b.line()
    );
}

/// One lexicon, two readers: a title's head and a hint's clause heads are read
/// by the same rule. For every phrase the lexicon holds, a line made of that
/// phrase alone answers the same phrase and the same acts through both, and so
/// does a line that opens on a place clause and then states it.
#[test]
fn the_title_head_and_the_hint_reader_agree_on_every_phrase() {
    let lex = telling::Lexicon::english();
    let mut n = 0;
    for (phrase, acts) in lex.phrases() {
        let head = lex.head(phrase).expect("a phrase is its own head");
        let read = lex.clause_heads(phrase);
        assert_eq!(read.len(), 1, "`{phrase}`: {read:?}");
        assert_eq!((&head.0, head.1), (&read[0].0, read[0].1), "`{phrase}`");
        assert_eq!(head.1, acts, "`{phrase}`");
        let placed = lex.clause_heads(&format!("Past the bell, {phrase} the lever."));
        assert_eq!(placed.len(), 1, "`{phrase}` after a place: {placed:?}");
        assert_eq!((&placed[0].0, placed[0].1), (&head.0, head.1), "`{phrase}`");
        n += 1;
    }
    assert!(n > 0, "the lexicon holds phrases");
}

/// `delvec validate` exits 0 on a document whose only hint finding is the
/// `DW1045` warning, and prints the warning and its binding.
#[test]
fn a_misnamed_hint_warns_and_validation_exits_zero() {
    let mut q = quests(nulls());
    q["content"]["quests"][0]["objectives"][0]["hint"] = json!("Walk past the slate.");
    let dir = variant("hint-exit", q, dialogue(GREETING));
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
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(
        text.lines()
            .any(|l| l.starts_with("DW1045") && l.contains("Walk past the slate.")),
        "{text}"
    );
    assert!(
        text.contains("hint act binding (DW1045)") && text.contains("; 1 warned"),
        "{text}"
    );
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
         also in reach), 0 trigger(s) judged by sight alone, 0 announced but not yet armed on the \
         way and judged by sight; 0 place(s) walked through — 0 with an announced objective armed, \
         0 with the next act in sight; 0 refused; 0 kill(s) on no walk judged from the acts that \
         fire their wave"
    );
    assert_eq!(
        telling::HintActBinding::default().line(),
        "hint act binding (DW1045): 0 of 0 English hint(s) name an act, over 0 objective(s); 0 \
         warned"
    );
    assert_eq!(
        promise::CompassBinding::default().line(),
        "compass binding: 0 tracked objective(s) of 0 — 0 by their own field, 0 by the campaign \
         default; 0 refused"
    );
    assert_eq!(
        promise::HighlightBinding::default().line(),
        "highlight binding: 0 highlight(s) over 0 act(s) — 0 on a block, 0 on a body; 0 refused"
    );
}

// ---------------------------------------------------------------------------
// §6 — the compass, DW1017
// ---------------------------------------------------------------------------

/// The campaign above with the button opened beside the lever (both after the
/// talk), so two objectives are open at once.
fn two_open(guidance: Value) -> Value {
    let mut q = quests(guidance);
    q["content"]["quests"][0]["objectives"][2]["after"] = json!(["obj/talk"]);
    q
}

fn all_text(out: &BuildOutput) -> String {
    let mut s = String::new();
    for (k, v) in out {
        if k.starts_with("datapack/")
            && let Ok(t) = std::str::from_utf8(v)
        {
            s.push_str(t);
        }
    }
    s
}

/// spec-0104 criterion 4, the bytes: the kit gives the compass, the refresh
/// resolves the followed objective in document order and aims it through the
/// one macro over every carried slot, the cycle wraps, the lines are told on a
/// change and the others' count only with two open, and two builds are
/// byte-identical. A campaign that hides the compass and opts nothing in
/// emits none of it.
#[test]
fn the_compass_follows_an_open_objective() {
    let dir = variant("compass", two_open(json!({})), dialogue(GREETING));
    let out = try_build(&dir).expect("builds");
    let apply = fn_body(&out, "class_apply_warden").or_else(|| {
        out.iter()
            .find(|(k, _)| k.contains("/function/class_apply_"))
            .map(|(_, v)| std::str::from_utf8(v).unwrap())
    });
    let apply = apply.expect("a class apply");
    assert!(
        apply.contains("function hello-world:compass_give"),
        "{apply}"
    );
    let give = fn_body(&out, "compass_give").unwrap();
    assert!(
        give.contains("custom_data={dw_compass:1b}") && give.contains("custom_name="),
        "{give}"
    );
    let aim = fn_body(&out, "compass_aim").unwrap();
    assert_eq!(aim.lines().count(), 41, "every carried slot: {aim}");
    assert!(
        aim.contains(
            "lodestone_tracker={target:{pos:[I;$(x),$(y),$(z)],dimension:\"minecraft:overworld\"},tracked:false}"
        ),
        "{aim}"
    );
    assert!(aim.contains("weapon.offhand") && !aim.contains("container.36"));
    let spin = fn_body(&out, "compass_spin").unwrap();
    assert!(spin.contains("lodestone_tracker={tracked:false}"), "{spin}");
    let refresh = fn_body(&out, "compass_refresh").unwrap();
    assert!(
        refresh.contains("if score @s dw.track matches 2"),
        "{refresh}"
    );
    assert!(
        refresh.contains("function hello-world:compass_tell"),
        "{refresh}"
    );
    let followed = fn_body(&out, "compass_aim_followed").unwrap();
    assert!(
        followed.contains("compass_aim_body_talk"),
        "a talk-to follows the body: {followed}"
    );
    let body = fn_body(&out, "compass_aim_body_talk").unwrap();
    assert!(
        body.contains("@e[tag=dw_npc_keeper,type=!minecraft:interaction,limit=1] Pos[0]"),
        "{body}"
    );
    // The menu: one button per pending tracked objective, each a pick.
    let menu = fn_body(&out, "compass_menu").unwrap();
    assert!(
        menu.contains("type:\"minecraft:multi_action\"") && menu.contains("pause:false"),
        "{menu}"
    );
    for k in 1..=3 {
        assert!(
            menu.contains(&format!("command:\"trigger dw.cpick set {k}\"")),
            "button {k}: {menu}"
        );
    }
    assert_eq!(
        fn_body(&out, "compass_menu_show").unwrap().trim(),
        "$dialog show @s $(dialog)"
    );
    let used = fn_body(&out, "compass_used").unwrap();
    assert!(
        used.contains("advancement revoke @s only hello-world:compass_use")
            && used.contains("unless score @s dw.cuse matches 1.."),
        "{used}"
    );
    let pick = fn_body(&out, "compass_pick").unwrap();
    assert!(
        pick.contains("scoreboard players operation @s dw.track = @s dw.cpick")
            && pick.contains("scoreboard players set @s dw.told -1"),
        "{pick}"
    );
    assert!(
        give.contains("consumable={consume_seconds:1000000f"),
        "{give}"
    );
    let tell = fn_body(&out, "compass_tell").unwrap();
    assert!(tell.contains("compass.following") && tell.contains("compass.nothing"));
    assert!(
        tell.contains("if score #gopen dw.guide matches 2..")
            && tell.contains(r#"{"keybind":"key.use"}"#),
        "{tell}"
    );
    let adv = out
        .get("datapack/data/hello-world/advancement/compass_use.json")
        .expect("the compass's use advancement");
    let adv = std::str::from_utf8(adv).unwrap();
    assert!(
        adv.contains("minecraft:using_item") && adv.contains("dw_compass"),
        "{adv}"
    );
    let again = try_build(&dir).expect("builds again");
    assert_eq!(out, again, "two builds are byte-identical");

    // A hidden objective is never a target.
    let mut q = two_open(json!({}));
    q["content"]["quests"][0]["objectives"][2]["compass"] = json!("hidden");
    let out = try_build(&variant("compass-hidden-one", q, dialogue(GREETING))).unwrap();
    let refresh = fn_body(&out, "compass_refresh").unwrap();
    assert!(!refresh.contains("dw.track matches 3"), "{refresh}");
    assert!(!refresh.contains("dw.o_press"), "{refresh}");

    // Hidden campaign-wide and nothing opted in: no compass at all.
    let out = try_build(&variant(
        "compass-none",
        two_open(json!({ "compass": "hidden" })),
        dialogue(GREETING),
    ))
    .unwrap();
    assert!(
        !out.keys().any(|k| k.contains("compass")),
        "no compass file"
    );
    let all = all_text(&out);
    assert!(!all.contains("dw.track") && !all.contains("dw.cpick"));
    let prefix = std::str::from_utf8(
        out.get("datapack/data/hello-world/item_modifier/journal_prefix.json")
            .unwrap(),
    )
    .unwrap();
    assert!(
        !prefix.contains("journal.compass"),
        "no compass page: {prefix}"
    );
}

/// spec-0104 criterion 5.
#[test]
fn a_compass_on_an_objective_nobody_is_told_about_is_dw1017() {
    let mut q = quests(nulls());
    let pull = &mut q["content"]["quests"][0]["objectives"][1];
    pull.as_object_mut().unwrap().remove("title");
    pull.as_object_mut().unwrap().remove("hint");
    pull["compass"] = json!("shown");
    let (_, c) = load(&variant("dw1017-untitled", q.clone(), dialogue(GREETING)));
    let (d, b) = promise::check_compass(&c);
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].code, promise::DW_COMPASS_UNTOLD);
    assert!(d[0].message.contains("obj/pull") && d[0].message.contains("no `title`"));
    assert_eq!(b.refused, 1);
    q["content"]["quests"][0]["objectives"][1]["title"] = json!("Pull the lever");
    q["content"]["quests"][0]["objectives"][1]["announcement"] = json!("hidden");
    let (_, c) = load(&variant("dw1017-hidden", q.clone(), dialogue(GREETING)));
    let (d, _) = promise::check_compass(&c);
    assert!(d[0].message.contains("announcement: hidden"), "{d:?}");
    q["content"]["quests"][0]["objectives"][1]
        .as_object_mut()
        .unwrap()
        .remove("announcement");
    let (_, c) = load(&variant("dw1017-told", q, dialogue(GREETING)));
    let (d, b) = promise::check_compass(&c);
    assert!(d.is_empty(), "{d:?}");
    assert_eq!(
        b.line(),
        "compass binding: 3 tracked objective(s) of 3 — 1 by their own field, 2 by the campaign \
         default; 0 refused"
    );
}

// ---------------------------------------------------------------------------
// §7 — the highlight, DW1016
// ---------------------------------------------------------------------------

/// spec-0104 criterion 6, the bytes.
#[test]
fn a_highlight_lights_the_thing_while_armed_and_near() {
    let mut q = quests(nulls());
    let objs = &mut q["content"]["quests"][0]["objectives"];
    objs[1]["highlight"] = json!({ "within": 6 });
    objs[0]["highlight"] = json!({ "within": 4 });
    objs[2]["prop"] = json!({ "block": "minecraft:bell[attachment=floor,facing=north]" });
    objs[2]["title"] = json!("Ring the bell");
    objs[2]["highlight"] = json!({ "within": 6 });
    let out = try_build(&variant("highlight", q, dialogue(GREETING))).expect("builds");
    // The activation places the prop and nothing else: a display exists
    // only while the highlight is lit, summoned by `hl_hold_<a>` alone.
    let act = fn_body(&out, "activate_o_pull").unwrap();
    assert!(!act.contains("dw_hl"), "{act}");
    let show = fn_body(&out, "hl_hold_o_pull").unwrap();
    assert!(
        show.contains("summon minecraft:block_display")
            && show.contains("Name:\"minecraft:lever\"")
            && show.contains("face:\"floor\"")
            && show.contains("Glowing:1b")
            && show.contains("scale:[1.002f,1.002f,1.002f]")
            && show.contains("translation:[-0.001f,-0.001f,-0.001f]")
            && show.contains("glow_color_override:16755200"),
        "{show}"
    );
    let bell = fn_body(&out, "hl_hold_o_press").unwrap();
    assert!(
        bell.contains("summon minecraft:item_display") && bell.contains("id:\"minecraft:bell\""),
        "a bell is drawn by its renderer: {bell}"
    );
    // Each display covers the cell its prop is set in: the block display at
    // the cell's corner, written `x.0` so the summon cannot centre it; the
    // item display at the cell's centre.
    let cell_of = |f: &str, block: &str| -> [i32; 3] {
        let body = fn_body(&out, f).unwrap();
        let line = body
            .lines()
            .find(|l| l.starts_with("setblock ") && l.contains(block))
            .unwrap_or_else(|| panic!("{f} sets {block}: {body}"));
        let t: Vec<i32> = line
            .split(' ')
            .skip(1)
            .take(3)
            .map(|v| v.parse().unwrap())
            .collect();
        [t[0], t[1], t[2]]
    };
    let summon_at = |body: &str, kind: &str| -> String {
        let line = body
            .lines()
            .find_map(|l| l.split_once(&format!("run summon {kind} ")))
            .unwrap_or_else(|| panic!("a {kind}: {body}"))
            .1;
        line.split(' ').take(3).collect::<Vec<_>>().join(" ")
    };
    let [x, y, z] = cell_of("activate_o_pull", "minecraft:lever");
    assert_eq!(
        summon_at(show, "minecraft:block_display"),
        format!("{x}.0 {y}.0 {z}.0"),
        "{show}"
    );
    let [x, y, z] = cell_of("activate_o_press", "minecraft:bell");
    assert_eq!(
        summon_at(bell, "minecraft:item_display"),
        format!(
            "{:.1} {:.1} {:.1}",
            f64::from(x) + 0.5,
            f64::from(y) + 0.5,
            f64::from(z) + 0.5
        ),
        "{bell}"
    );
    let poll = fn_body(&out, "hl_poll_o_pull").unwrap();
    assert!(
        poll.contains("if score #gs_pull dw.guide matches 1")
            && poll.contains("@a[distance=..6,tag=!dw_cutscene]"),
        "{poll}"
    );
    // Lit, the displays are held to their cells; dark, none exists.
    assert!(
        poll.contains("matches 1 run function hello-world:hl_hold_o_pull"),
        "{poll}"
    );
    let on = fn_body(&out, "hl_on_o_pull").unwrap();
    assert!(on.contains("function hello-world:hl_hold_o_pull"), "{on}");
    let pull_off = fn_body(&out, "hl_off_o_pull").unwrap();
    assert!(pull_off.contains("kill @e[tag=dw_hl_o_pull]"), "{pull_off}");
    let talk = fn_body(&out, "hl_on_o_talk").unwrap();
    assert!(
        talk.contains(
            "effect give @e[tag=dw_npc_keeper,type=!minecraft:interaction] minecraft:glowing"
        ) && talk.contains("team join dw_hl"),
        "{talk}"
    );
    let off = fn_body(&out, "hl_off_o_talk").unwrap();
    assert!(
        off.contains("effect clear") && off.contains("team leave"),
        "{off}"
    );
    let done = fn_body(&out, "complete_o_pull").unwrap();
    assert!(
        done.contains("function hello-world:hl_off_o_pull"),
        "{done}"
    );
    let setup = fn_body(&out, "setup").unwrap();
    assert!(setup.contains("team modify dw_hl color gold"), "{setup}");
    assert!(
        out.contains_key(
            "packtest-datapack/data/hello-world/test/guide_highlight_o_pull.mcfunction"
        )
    );
    assert!(out.contains_key(
        "packtest-datapack/data/hello-world/test/guide_highlight_spent_o_pull.mcfunction"
    ));
}

/// spec-0104 criterion 6: a wave and a `use` trigger are lit the same way.
#[test]
fn a_wave_and_a_trigger_are_lit() {
    let v = std::fs::read_to_string(common::keep_trial_dir().join("quests.json")).unwrap();
    let mut q: Value = serde_json::from_str(&v).unwrap();
    for quest in q["content"]["quests"].as_array_mut().unwrap() {
        for o in quest["objectives"].as_array_mut().unwrap() {
            if o["type"] == "kill" {
                o["highlight"] = json!({ "within": 10 });
            }
        }
    }
    let dir = std::env::temp_dir().join(format!("dw-guidance-wave-hl-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    common::materialize_from(
        &common::keep_trial_dir(),
        &json!({ "documents": { "quests": q } }),
        &dir,
    );
    let out = try_build_ns(&dir, "keep-trial");
    let on = std::str::from_utf8(
        out.get("datapack/data/keep-trial/function/hl_on_o_slay.mcfunction")
            .expect("the kill's highlight"),
    )
    .unwrap();
    assert!(on.contains("@e[tag=dw_wave_"), "{on}");
}

fn try_build_ns(dir: &Path, _ns: &str) -> BuildOutput {
    try_build(dir).expect("builds")
}

/// spec-0104 criterion 7.
#[test]
fn a_highlight_with_nothing_to_light_is_dw1016() {
    let c = titled(
        json!({ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper",
        "title": "Talk to the keeper", "highlight": { "within": 6 } }),
    );
    let (d, b) = promise::check_highlights(&c);
    assert!(d.is_empty(), "{d:?}");
    assert_eq!((b.highlights, b.on_body), (1, 1));
    let mut c2 = c.clone();
    let o: delvewright_dsl::Objective = serde_json::from_value(json!({
        "type": "collect", "id": "obj/bone", "item": "minecraft:bone", "count": 1,
        "anchor": "anchor/exit", "dropped_by": "wave/none", "highlight": { "within": 6 }
    }))
    .unwrap();
    c2.quests.content.quests[0].objectives.push(o);
    let (d, b) = promise::check_highlights(&c2);
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].code, promise::DW_HIGHLIGHT_INERT);
    assert!(d[0].message.contains("obj/bone"));
    assert_eq!(b.refused, 1);
    for (on, ok) in [
        (json!({ "on": "approach", "range": 3 }), false),
        (json!({ "on": "step" }), false),
        (
            json!({ "on": "strike-assembly", "assembly": "assembly/x" }),
            false,
        ),
        (json!({ "on": "use" }), true),
        (json!({ "on": "strike" }), true),
        (json!({ "on": "strike-npc", "npc": "npc/keeper" }), true),
    ] {
        let mut c3 = c.clone();
        let mut t = json!({ "id": "trigger/t", "on": on, "effects": [],
                            "highlight": { "within": 6 } });
        if t["on"]["on"] != "strike-npc" && t["on"]["on"] != "strike-assembly" {
            t["at"] = json!("anchor/exit");
        }
        let t: delvewright_dsl::EnvTrigger = serde_json::from_value(t).unwrap();
        c3.quests.content.triggers.push(t);
        let (d, _) = promise::check_highlights(&c3);
        assert_eq!(d.is_empty(), ok, "{on}: {d:?}");
        if !ok {
            assert!(d[0].message.contains("trigger/t"), "{d:?}");
        }
    }
}

// ---------------------------------------------------------------------------
// §8 — the journal
// ---------------------------------------------------------------------------

const PAGE_ONE: &str = "Things on this shore answer to a hand: [[key|use]] on a lever.";
const PAGE_TWO: &str = "The ferry comes when the bell is rung twice.";

/// spec-0104 criterion 9, the bytes.
#[test]
fn the_journal_holds_the_manual_the_open_and_the_done() {
    let g = json!({ "manual": { "pages": [PAGE_ONE, PAGE_TWO] } });
    let out = try_build(&variant("journal", quests(g), dialogue(GREETING))).expect("builds");
    let apply = out
        .iter()
        .find(|(k, _)| k.contains("/function/class_apply_"))
        .map(|(_, v)| std::str::from_utf8(v).unwrap())
        .unwrap();
    assert!(
        apply.contains("function hello-world:journal_give"),
        "{apply}"
    );
    let give = fn_body(&out, "journal_give").unwrap();
    assert!(
        give.contains("minecraft:written_book[custom_data={dw_journal:1b},item_name={")
            && give.contains("delvewright.ui.journal.title")
            && give.contains("written_book_content={title:\"\""),
        "{give}"
    );
    let im = |n: &str| {
        out.get(&format!("datapack/data/hello-world/item_modifier/{n}.json"))
            .map(|b| serde_json::from_slice::<Value>(b).unwrap())
    };
    let prefix = im("journal_prefix").unwrap();
    assert_eq!(prefix["function"], "minecraft:set_written_book_pages");
    assert_eq!(prefix["mode"], "replace_all");
    let pages = prefix["pages"].as_array().unwrap();
    assert_eq!(pages.len(), 3, "the compass page and two manual pages");
    let text = serde_json::to_string(&prefix).unwrap();
    assert!(text.contains(r#"{"keybind":"key.use"}"#), "{text}");
    for o in ["talk", "pull", "press"] {
        assert_eq!(im(&format!("journal_open_{o}")).unwrap()["mode"], "append");
        assert!(im(&format!("journal_open_following_{o}")).is_some());
        assert!(im(&format!("journal_done_{o}")).is_some());
    }
    let write = fn_body(&out, "journal_write").unwrap();
    let first = write.lines().next().unwrap();
    assert_eq!(
        first,
        "$item modify entity @s $(slot) hello-world:journal_prefix"
    );
    assert!(write.contains(
        "if score @s dw.track matches 2 run item modify entity @s $(slot) hello-world:journal_open_following_pull"
    ));
    assert!(write.contains(
        "$execute if score #party dw.o_pull matches 1 run item modify entity @s $(slot) hello-world:journal_done_pull"
    ));
    // The announcement lines of spec-0093 stay beside the journal.
    assert!(fn_body(&out, "announce_o_pull").is_some());

    // An unannounced objective has no page.
    let mut q = quests(nulls());
    q["content"]["quests"][0]["objectives"][1]["announcement"] = json!("hidden");
    q["content"]["quests"][0]["objectives"][1]
        .as_object_mut()
        .unwrap()
        .remove("hint");
    let out = try_build(&variant("journal-quiet", q, dialogue(GREETING))).unwrap();
    assert!(!out.contains_key("datapack/data/hello-world/item_modifier/journal_open_pull.json"));

    // No announced objective, no compass, no manual: no book.
    let mut q = quests(nulls());
    for o in q["content"]["quests"][0]["objectives"]
        .as_array_mut()
        .unwrap()
    {
        let o = o.as_object_mut().unwrap();
        o.remove("title");
        o.remove("hint");
    }
    let (_, c) = load(&variant("journal-none-src", q.clone(), dialogue(GREETING)));
    assert!(
        c.quests.content.quests[0]
            .objectives
            .iter()
            .all(|o| o.title().is_none())
    );
    if let Ok(out) = try_build(&variant("journal-none", q, dialogue(GREETING))) {
        assert!(
            !out.keys()
                .any(|k| k.contains("journal") || k.contains("guide"))
        );
        assert!(!all_text(&out).contains("dw.guide"));
    }
}

/// spec-0104 criterion 9: a page over the book's box is a `DW0330` warning
/// naming the page and the language; the key span is measured at its default
/// key name; a zh-cn row that drops the key span is refused.
#[test]
fn a_journal_page_fits_its_book() {
    use delvec::compiler::textfit;
    let long = "A line that says a great deal about the shore. ".repeat(12);
    let g = json!({ "manual": { "pages": [PAGE_ONE, long] } });
    let (_, c) = load(&variant("journal-long", quests(g), dialogue(GREETING)));
    let (d, measured) = textfit::check_book_pages(&c, &BTreeMap::new());
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].code, textfit::DW_TEXT_OVERRUNS_SCREEN);
    assert_eq!(d[0].severity, delvewright_dsl::Severity::Warning);
    assert!(d[0].path.contains("guidance.manual.pages.1") && d[0].path.starts_with("en"));
    assert!(measured.iter().any(|m| m.1 == "the compass page"));
    assert_eq!(
        textfit::book_lines("[[key|use]]", false),
        1,
        "a key span is one short line"
    );
    assert!(textfit::book_lines(&"word ".repeat(40), false) > 3);

    let key = "guidance.manual.pages.0";
    let mut s = BTreeMap::new();
    s.insert(
        "zh-cn".to_string(),
        sidecar("zh-cn", &[(key, "这里的东西认得手：按右键。")]),
    );
    let (_, c) = load(&variant(
        "journal-key",
        quests(json!({ "manual": { "pages": [PAGE_ONE] } })),
        dialogue(GREETING),
    ));
    let d = delvewright_dsl::textstyle::validate_inline_styles(&c, &s);
    assert!(
        d.iter()
            .any(|d| d.code == delvewright_dsl::textstyle::INLINE_STYLE_UNMATCHED),
        "{d:?}"
    );
}

/// spec-0104 §6.3: a compass following a `kill` re-aims on the twenty-tick
/// clock at the living body of the wave nearest the player holding it, and at
/// the wave's anchor while none is alive.
#[test]
fn a_compass_following_a_kill_points_at_the_nearest_living_body() {
    let out = try_build(&common::keep_trial_dir()).expect("keep-trial builds");
    let body = |name: &str| -> &str {
        out.get(&format!(
            "datapack/data/keep-trial/function/{name}.mcfunction"
        ))
        .map(|b| std::str::from_utf8(b).unwrap())
        .unwrap_or_else(|| panic!("{name} emitted"))
    };
    let followed = body("compass_aim_followed");
    let k = followed
        .lines()
        .find(|l| l.ends_with("run function keep-trial:compass_aim_wave_slay"))
        .and_then(|l| l.split("matches ").nth(1))
        .and_then(|r| r.split(' ').next())
        .unwrap_or_else(|| panic!("the kill dispatches its wave aim: {followed}"));
    assert!(
        followed.contains(&format!(
            "execute if score @s dw.track matches {k} run scoreboard players set @s dw.tbody 1"
        )),
        "a followed kill is re-aimed on the clock: {followed}"
    );
    let tick = out
        .iter()
        .filter(|(p, _)| p.contains("/function/") && p.ends_with(".mcfunction"))
        .map(|(_, b)| std::str::from_utf8(b).unwrap())
        .find(|b| {
            b.contains("as @a[scores={dw.tbody=1}] run function keep-trial:compass_aim_followed")
        })
        .expect("the clock line");
    assert!(
        tick.contains("execute if score #gclock dw.guide matches 0 as @a[scores={dw.tbody=1}]"),
        "{tick}"
    );
    // The anchor the fallback names is the cell the wave is summoned around.
    let spawn = body("spawn_guards");
    assert!(
        spawn.contains("summon minecraft:zombie 11.5 65.0 12.5"),
        "{spawn}"
    );
    assert_eq!(
        body("compass_aim_wave_slay").lines().collect::<Vec<_>>(),
        vec![
            "tag @e[tag=dw_caim] remove dw_caim",
            "execute at @s run tag @e[tag=dw_wave_guards,sort=nearest,limit=1] add dw_caim",
            "execute store result storage keep-trial:guide aim.x int 1 run data get entity @e[tag=dw_caim,limit=1] Pos[0]",
            "execute store result storage keep-trial:guide aim.y int 1 run data get entity @e[tag=dw_caim,limit=1] Pos[1]",
            "execute store result storage keep-trial:guide aim.z int 1 run data get entity @e[tag=dw_caim,limit=1] Pos[2]",
            "execute if entity @e[tag=dw_caim,limit=1] run function keep-trial:compass_aim with storage keep-trial:guide aim",
            "execute unless entity @e[tag=dw_caim,limit=1] run function keep-trial:compass_aim {x:11,y:65,z:12}",
            "tag @e[tag=dw_caim] remove dw_caim",
        ]
    );
    // The generated PackTest drives two bodies, kills the nearer, and re-aims.
    let pt = out
        .get("packtest-datapack/data/keep-trial/test/guide_compass_wave.mcfunction")
        .map(|b| std::str::from_utf8(b).unwrap())
        .expect("the wave compass PackTest");
    assert_eq!(pt.matches("summon minecraft:zombie").count(), 2, "{pt}");
    assert!(
        pt.contains("kill @e[tag=dw_pt_cmpw_near]")
            && pt.contains("function keep-trial:compass_aim_followed")
            && pt.contains("@e[tag=dw_pt_cmpw_far,limit=1] Pos[0]"),
        "{pt}"
    );
    // Every other body of the wave is held out of it before the first body
    // is summoned and handed back after the last is killed: a sibling's body
    // standing nearer than the far one would otherwise be the compass's aim.
    let at = |needle: &str| {
        pt.find(needle)
            .unwrap_or_else(|| panic!("`{needle}` missing:\n{pt}"))
    };
    let hold = at("tag @e[tag=dw_wave_guards,tag=!dw_pt_cmpw] add dw_pt_cmpw_held");
    assert!(
        hold < at("summon minecraft:zombie")
            && at("kill @e[tag=dw_pt_cmpw]\n")
                < at("tag @e[tag=dw_pt_cmpw_held] add dw_wave_guards"),
        "{pt}"
    );
}
