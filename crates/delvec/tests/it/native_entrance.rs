//! spec-0132 — **a body enters the way its kind does**, and the exported path
//! names the bodies each step is walked beside.
//!
//! Driven by the `souls-bonfire` fixture (a real bonfire, so the re-seat is
//! emitted), with the actors this file is about declared in-test: a warden that
//! digs out of the ground, unleashed by the first quest's beat and vanished by
//! the inner door's, and the shapes each claim needs beside it. Every claim is
//! read off a whole real emission.

use crate::common;

use std::collections::BTreeMap;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{
    BodyRef, Campaign, DSL_VERSION, Severity, parse_campaign, validate_campaign_with,
};
use serde_json::{Value, json};

const NS: &str = "souls-bonfire";

/// The warden that digs out of the ground.
const SLEEPER: &str = "actor/the-sleeper";
const SLEEPER_SAFE: &str = "the_sleeper";

fn fixture_dir() -> std::path::PathBuf {
    common::compiler_fixtures_dir().join(NS)
}

fn effect(v: Value) -> delvewright_dsl::QuestEffect {
    serde_json::from_value(v).expect("effect parses")
}

/// The fixture, raised to the current version, with `actor/the-sleeper` — a
/// warden that enters natively and is outrun — unleashed when `obj/talk`
/// completes and vanished when `obj/door` does.
fn base() -> Campaign {
    let loaded = load_campaign_dir(&fixture_dir()).unwrap();
    let mut c = parse_campaign(&loaded.raw).expect("souls-bonfire parses");
    c.quests.dsl_version = DSL_VERSION.to_string();
    c.quests.content.actors.push(
        serde_json::from_value(json!({
            "id": SLEEPER,
            "entity": "minecraft:warden",
            "anchor": "anchor/wave",
            "facing": "east",
            "entrance": "native",
            "outrun": true
        }))
        .expect("actor parses"),
    );
    beat(&mut c, "obj/talk").push(effect(json!({
        "type": "unleash-actor",
        "actor": SLEEPER,
        "happening": { "verb": "arrives", "text": "the sleeper digs out of the floor" }
    })));
    beat(&mut c, "obj/door").push(effect(json!({
        "type": "despawn-actor",
        "actor": SLEEPER,
        "style": "vanish",
        "happening": { "verb": "departs", "text": "the sleeper sinks away" }
    })));
    c
}

/// The bundle an objective's completion fires.
fn beat<'a>(c: &'a mut Campaign, objective: &str) -> &'a mut Vec<delvewright_dsl::QuestEffect> {
    c.quests
        .content
        .quests
        .iter_mut()
        .find_map(|q| {
            q.on_objective_complete
                .iter_mut()
                .find(|(k, _)| k.as_str() == objective)
                .map(|(_, v)| v)
        })
        .unwrap_or_else(|| panic!("`{objective}` has a bundle"))
}

/// Every error validation raises (warnings are advisory).
fn validation_errors(c: &Campaign) -> Vec<delvewright_dsl::Diagnostic> {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    validate_campaign_with(
        c,
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    )
    .into_iter()
    .filter(|d| d.severity == Severity::Error)
    .collect()
}

/// Validate + plan + emit, or the code that stopped it.
fn try_build(c: &Campaign) -> Result<(BuildOutput, Plan<'_>), String> {
    let errors = validation_errors(c);
    if !errors.is_empty() {
        return Err(format!("validation: {errors:#?}"));
    }
    let loaded = load_campaign_dir(&fixture_dir()).unwrap();
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(c, &prefabs)
        .map_err(|e| format!("{}: {}", e.failure.code, e.failure.message))?;
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    let mut skins: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for npc in &c.npcs.content.npcs {
        if let Some(skin) = &npc.skin {
            let png = std::fs::read(
                fixture_dir()
                    .join("skins")
                    .join(format!("{}.png", skin.texture_id)),
            )
            .expect("skin png present");
            skins.insert(skin.texture_id.clone(), png);
        }
    }
    let out = emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &skins,
    )
    .map_err(|e| format!("{e:?}"))?;
    Ok((out, plan))
}

fn build(c: &Campaign) -> (BuildOutput, Plan<'_>) {
    try_build(c).unwrap_or_else(|e| panic!("the fixture builds: {e}"))
}

fn text(out: &BuildOutput, path: &str) -> String {
    String::from_utf8(
        out.get(path)
            .unwrap_or_else(|| panic!("{path} emitted"))
            .clone(),
    )
    .unwrap()
}

fn func(out: &BuildOutput, name: &str) -> String {
    text(
        out,
        &format!("datapack/data/{NS}/function/{name}.mcfunction"),
    )
}

fn critical_path(out: &BuildOutput) -> Value {
    serde_json::from_str(&text(out, "critical-path.json")).unwrap()
}

/// `(the step's objective / trigger / action, the actors it is walked beside)`, per step.
fn pursued(cp: &Value) -> Vec<(String, Vec<String>)> {
    cp["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            let what = s["objective"]
                .as_str()
                .or(s["trigger"].as_str())
                .or(s["action"].as_str())
                .unwrap()
                .to_string();
            let who = s["pursuers"]
                .as_array()
                .unwrap_or_else(|| panic!("every step carries `pursuers`: {s}"))
                .iter()
                .map(|p| p["actor"].as_str().unwrap().to_string())
                .collect();
            (what, who)
        })
        .collect()
}

/// The `summon` lines of a function.
fn summons(f: &str) -> Vec<&str> {
    f.lines().filter(|l| l.contains("summon ")).collect()
}

// ---------------------------------------------------------------------------
// §2.3 — the entrance on the twin's summon
// ---------------------------------------------------------------------------

/// Criterion 1: the warden twin's summon carries `is_emerging` ttl 133 beside
/// `dig_cooldown` under ONE `Brain` key — on both arrival lines, the one at the
/// puppet and the one at the mark.
#[test]
fn the_warden_twin_enters_from_the_ground() {
    let c = base();
    let (out, _) = build(&c);
    let unleash = func(&out, &format!("unleash_{SLEEPER_SAFE}"));
    let lines = summons(&unleash);
    assert_eq!(lines.len(), 2, "two arrival lines:\n{unleash}");
    for l in lines {
        assert_eq!(l.matches("Brain:").count(), 1, "one Brain key: {l}");
        assert!(
            l.contains(
                "Brain:{memories:{\"minecraft:dig_cooldown\":{value:{},ttl:1200L},\"minecraft:is_emerging\":{value:{},ttl:133L}}}"
            ),
            "both memories under the one key: {l}"
        );
    }
}

/// Criterion 2: the wither twin charges up — `Invul:220` and `Health` at a
/// third of its maximum, the default 300 and a declared 600.
///
/// A wither stands 3.5 blocks tall, more than any room of the keep fixture
/// gives it (`DW0450`), so this is built over a copy of the gallery with the
/// wither at the hall's vantage, unleashed when the party climbs onto the
/// ferry's cabin. The gallery itself carries no wither (spec-0132 §7).
#[test]
fn the_wither_twin_charges_up() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("native-entrance-wither");
    let (campaign, prefabs) = common::gallery_primary_at(&dir.join("gallery"));
    for (n, (extra, health)) in [
        (json!({}), "Health:100.0f"),
        (
            json!({ "attributes": { "max_health": 600.0 } }),
            "Health:200.0f",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let case = dir.join(format!("case-{n}"));
        let _ = std::fs::remove_dir_all(&case);
        common::copy_dir_all(&campaign, &case);
        common::patch_file(&case.join("quests.json"), |v| {
            let mut a = json!({
                "id": "actor/the-withered",
                "entity": "minecraft:wither",
                "anchor": "anchor/vantage",
                "entrance": "native"
            });
            for (k, x) in extra.as_object().unwrap() {
                a[k] = x.clone();
            }
            v["content"]["actors"].as_array_mut().unwrap().push(a);
            let q = v["content"]["quests"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|q| q["id"] == "quest/far-hall")
                .unwrap();
            let b = &mut q["on_objective_complete"]["obj/climb-onto-the-cabin"];
            if b.is_null() {
                *b = json!([]);
            }
            b.as_array_mut().unwrap().push(json!({
                "type": "unleash-actor",
                "actor": "actor/the-withered",
                "happening": { "verb": "arrives", "text": "the withered thing charges" }
            }));
        });
        let out = dir.join(format!("out-{n}"));
        let _ = std::fs::remove_dir_all(&out);
        let r = std::process::Command::new(env!("CARGO_BIN_EXE_delvec"))
            .arg("build")
            .arg(&case)
            .arg("-o")
            .arg(&out)
            .arg("--prefabs")
            .arg(&prefabs)
            .output()
            .expect("delvec runs");
        assert!(
            r.status.success(),
            "the gallery with a wither builds:\n{}",
            String::from_utf8_lossy(&r.stderr)
        );
        let unleash = std::fs::read_to_string(
            out.join("datapack/data/gallery/function/unleash_the_withered.mcfunction"),
        )
        .unwrap();
        let lines = summons(&unleash);
        assert_eq!(lines.len(), 2, "{unleash}");
        for l in lines {
            assert!(l.contains(",Invul:220,"), "the charge-up: {l}");
            assert!(l.contains(health), "a third of the maximum ({health}): {l}");
            assert!(!l.contains("Brain:"), "a wither needs no finalization: {l}");
        }
    }
}

/// Criterion 5: the bonfire's re-seat restores a body that already arrived —
/// no entrance on it.
#[test]
fn a_restand_does_not_enter_again() {
    let c = base();
    let (out, _) = build(&c);
    let restand = func(&out, &format!("actor_restand_{SLEEPER_SAFE}"));
    let twin = summons(&restand);
    assert_eq!(twin.len(), 1, "{restand}");
    assert!(!twin[0].contains("is_emerging"), "{restand}");
    assert!(
        twin[0].contains("Brain:{memories:{\"minecraft:dig_cooldown\":{value:{},ttl:1200L}}}"),
        "the finalization stays: {restand}"
    );
}

// ---------------------------------------------------------------------------
// §3 — the arrival without a puppet
// ---------------------------------------------------------------------------

/// Criterion 4: with no puppet standing, the twin is summoned at the actor's
/// own mark (`Plan::body_point`), wearing its `facing`, guarded on its body
/// tag; and the generated PackTest unleashes with no puppet and finds one body
/// at the mark.
#[test]
fn an_unleash_with_no_puppet_arrives_at_the_mark() {
    let c = base();
    let (out, plan) = build(&c);
    let a = c
        .quests
        .content
        .actors
        .iter()
        .find(|a| a.id.as_str() == SLEEPER)
        .unwrap();
    let mark = plan
        .body_point(BodyRef::Actor(a))
        .expect("the mark resolves");
    let at = format!("{}.5 {}.0 {}.5", mark[0], mark[1], mark[2]);
    let unleash = func(&out, &format!("unleash_{SLEEPER_SAFE}"));
    let want = format!(
        "execute unless entity @e[tag=dw_actor_{SLEEPER_SAFE}] run summon minecraft:warden {at} {{Rotation:[270f,0f],"
    );
    assert!(
        unleash.lines().any(|l| l.starts_with(&want)),
        "`{want}` in:\n{unleash}"
    );
    // The puppet arm is still there, and still keyed on the puppet.
    assert!(unleash.lines().any(|l| l.starts_with(&format!(
        "execute at @e[tag=dw_pup_{SLEEPER_SAFE},limit=1] run summon minecraft:warden ~ ~ ~ "
    ))));
    let pt = text(
        &out,
        &format!("packtest-datapack/data/{NS}/test/v06_unleash_{SLEEPER_SAFE}.mcfunction"),
    );
    for want in [
        format!(
            "execute store result score #mark_unl_{SLEEPER_SAFE} dw.sys if entity @e[type=minecraft:warden,tag=dw_actor_{SLEEPER_SAFE},x={}.5,y={}.0,z={}.5,distance=..0.5]\nassert score #mark_unl_{SLEEPER_SAFE} dw.sys matches 1\n",
            mark[0], mark[1], mark[2]
        ),
        format!("assert score #once_unl_{SLEEPER_SAFE} dw.sys matches 1\n"),
    ] {
        assert!(
            pt.contains(&want),
            "`{want}` in the unleash PackTest:\n{pt}"
        );
    }
}

/// Criterion 7 (the compiler half): the ambush sugar gives an entering actor
/// its `unleash-actor` alone, so the campaign builds — no `DW1083` — and the
/// generated trigger stands no puppet for it.
#[test]
fn an_ambush_of_an_entering_body_spawns_no_puppet() {
    let mut c = base();
    // The sleeper is the ambush's, not the talk beat's.
    beat(&mut c, "obj/talk").retain(|e| e.actor_ref().is_none_or(|a| a.as_str() != SLEEPER));
    c.quests.content.ambushes.push(
        serde_json::from_value(json!({
            "id": "ambush/the-floor",
            "at": "anchor/chest",
            "actors": [SLEEPER],
            "trigger": { "on": "approach", "range": 3 },
            "happening": { "verb": "arrives", "text": "the floor gives up the sleeper" }
        }))
        .unwrap(),
    );
    c.quests.content.expand_ambushes();
    let t = c
        .quests
        .content
        .triggers
        .iter()
        .find(|t| t.id.as_str() == "trigger/the-floor")
        .expect("the ambush desugars to its trigger");
    let verbs: Vec<bool> = t
        .effects
        .iter()
        .map(|e| matches!(e.verb, delvewright_dsl::Verb::UnleashActor { .. }))
        .collect();
    assert_eq!(verbs, vec![true], "the unleash alone");
    assert!(
        t.effects[0].happening.is_some(),
        "the declaration rides on the beat it springs"
    );
    let (out, _) = build(&c);
    assert!(func(&out, &format!("unleash_{SLEEPER_SAFE}")).contains("is_emerging"));
}

// ---------------------------------------------------------------------------
// §5.3 — the pursuers on the exported path
// ---------------------------------------------------------------------------

/// Criterion 9: the `outrun` bodies loose while each step is walked, under
/// format 6 — three shapes. Every step carries the key.
#[test]
fn pursuers_are_the_outrun_bodies_loose_while_a_step_is_walked() {
    // Unleashed at `obj/talk`'s completion, vanished at `obj/door`'s: loose
    // from the step after the unleash through the step before the despawn,
    // the rest between them included.
    let (out, _) = build(&base());
    let cp = critical_path(&out);
    assert_eq!(cp["format_version"], json!(6));
    let mine: Vec<(String, bool)> = pursued(&cp)
        .into_iter()
        .map(|(what, who)| (what, who == vec![SLEEPER.to_string()]))
        .collect();
    assert_eq!(
        mine,
        vec![
            ("select-class".to_string(), false),
            ("obj/talk".to_string(), false),
            ("obj/slay".to_string(), true),
            ("rest".to_string(), true),
            ("obj/key".to_string(), true),
            ("obj/door".to_string(), false),
            ("obj/shrine".to_string(), false),
            ("assert-complete".to_string(), false),
        ]
    );
    let row = &cp["steps"][2]["pursuers"][0];
    assert_eq!(
        row,
        &json!({
            "actor": SLEEPER,
            "kind": "minecraft:warden",
            "census": format!("{NS}:actor_census_{SLEEPER_SAFE}"),
            "entrance_ticks": 133
        })
    );
    let census = func(&out, &format!("actor_census_{SLEEPER_SAFE}"));
    assert!(census.contains(&format!(
        "@e[type=minecraft:warden,tag=dw_actor_{SLEEPER_SAFE},tag=!dw_pup_{SLEEPER_SAFE},limit=1]"
    )));

    // The step's own trigger: the strike the path performs in front of
    // `obj/talk` unleashes it, so `obj/talk` is walked beside it.
    let mut c = base();
    beat(&mut c, "obj/talk").retain(|e| e.actor_ref().is_none_or(|a| a.as_str() != SLEEPER));
    let ward = c
        .quests
        .content
        .triggers
        .iter_mut()
        .find(|t| t.id.as_str() == "trigger/gate-ward")
        .unwrap();
    ward.effects.push(effect(json!({
        "type": "unleash-actor",
        "actor": SLEEPER,
        "happening": { "verb": "arrives", "text": "the ward wakes the sleeper" }
    })));
    let (out, _) = build(&c);
    let rows = pursued(&critical_path(&out));
    let performed = rows
        .iter()
        .position(|(what, _)| what == "trigger/gate-ward");
    if let Some(k) = performed {
        assert!(
            rows[k].1.is_empty(),
            "the performing step is not walked beside it: {rows:?}"
        );
        assert_eq!(rows[k + 1].1, vec![SLEEPER.to_string()], "{rows:?}");
    } else {
        // Not performed on the path: undated, a pursuer from the first
        // standing step on.
        let cp = critical_path(&out);
        let first = cp["steps"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| !s["pursuers"].as_array().unwrap().is_empty())
            .expect("an undated unleash pursues the path");
        assert_eq!(first["pursuers"][0]["undated"], json!(true));
    }

    // An undated unleash — a trap-free trigger the path never performs.
    let mut c = base();
    beat(&mut c, "obj/talk").retain(|e| e.actor_ref().is_none_or(|a| a.as_str() != SLEEPER));
    let hum = c
        .quests
        .content
        .triggers
        .iter_mut()
        .find(|t| t.id.as_str() == "trigger/shrine-hum")
        .unwrap();
    hum.effects.push(effect(json!({
        "type": "unleash-actor",
        "actor": SLEEPER,
        "happening": { "verb": "arrives", "text": "the hum wakes the sleeper" }
    })));
    let (out, _) = build(&c);
    let cp = critical_path(&out);
    let rows = pursued(&cp);
    assert_eq!(
        rows.iter()
            .map(|(w, who)| (w.as_str(), !who.is_empty()))
            .collect::<Vec<_>>(),
        vec![
            ("select-class", false),
            ("obj/talk", true),
            ("obj/slay", true),
            ("rest", true),
            ("obj/key", true),
            ("obj/door", false),
            ("obj/shrine", false),
            ("assert-complete", false),
        ],
        "undated from the first standing step, until the dated despawn"
    );
    assert_eq!(cp["steps"][1]["pursuers"][0]["undated"], json!(true));
}

/// Criterion 9's perturbation, and DW1082's second arm: an `outrun` actor
/// unleashed only by the last quest's `on_complete` is loose on no step.
#[test]
fn an_outrun_body_loose_on_no_step_is_refused_at_build() {
    let mut c = base();
    beat(&mut c, "obj/talk").retain(|e| e.actor_ref().is_none_or(|a| a.as_str() != SLEEPER));
    beat(&mut c, "obj/door").retain(|e| e.actor_ref().is_none_or(|a| a.as_str() != SLEEPER));
    let trial = c
        .quests
        .content
        .quests
        .iter_mut()
        .find(|q| q.id.as_str() == "quest/trial")
        .unwrap();
    trial.on_complete.insert(
        0,
        effect(json!({
            "type": "unleash-actor",
            "actor": SLEEPER,
            "happening": { "verb": "arrives", "text": "too late" }
        })),
    );
    let err = try_build(&c).err().expect("refused");
    assert!(err.starts_with("DW1082:"), "{err}");
    assert!(err.contains(SLEEPER), "{err}");
}

// ---------------------------------------------------------------------------
// §3, §6 — the cutscene of the arrival
// ---------------------------------------------------------------------------

fn with_arrival_shot(cutscene_first: bool) -> Campaign {
    let mut c = base();
    beat(&mut c, "obj/talk").retain(|e| e.actor_ref().is_none_or(|a| a.as_str() != SLEEPER));
    let unleash = json!({
        "type": "unleash-actor",
        "actor": SLEEPER,
        "happening": { "verb": "arrives", "text": "the sleeper digs out of the floor" }
    });
    let shot = json!({
        "type": "cutscene",
        "shots": [{
            "path": [
                { "anchor": "anchor/wave", "offset": [4, 2, 0] },
                { "anchor": "anchor/wave", "offset": [4, 2, 1] }
            ],
            "seconds": 2,
            "subject": { "actor": SLEEPER }
        }]
    });
    let effects = if cutscene_first {
        vec![shot, unleash]
    } else {
        vec![unleash, shot]
    };
    beat(&mut c, "obj/talk").push(effect(json!({
        "type": "sequence",
        "steps": [{ "at_ticks": 0, "effects": effects }]
    })));
    c
}

/// Criterion 8: a cutscene framing a body placed only by its `unleash-actor`
/// is proven, not refused; the framing record states the body present. The
/// cutscene ahead of the unleash is `DW1034`.
#[test]
fn a_shot_of_the_arrival_sees_the_body() {
    let (out, _) = build(&with_arrival_shot(false));
    let ledger: Value = serde_json::from_str(&text(&out, "validation/shot-subjects.json")).unwrap();
    let presence = ledger["cutscenes"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|c| c["shots"].as_array().unwrap())
        .flat_map(|s| s["subjects"].as_array().cloned().unwrap_or_default())
        .find(|s| {
            s["kind"] == json!("actor")
                && s["subject"].as_str().is_some_and(|x| x.contains(SLEEPER))
        })
        .map(|s| s["presence"].clone())
        .expect("the shot frames the sleeper");
    assert!(
        presence
            .as_str()
            .unwrap()
            .starts_with("placed by /content/quests/"),
        "{presence}"
    );
    let err = try_build(&with_arrival_shot(true)).err().expect("refused");
    assert!(err.contains("DW1034"), "{err}");
    assert!(err.contains("`spawn-actor` or `unleash-actor`"), "{err}");
}

// ---------------------------------------------------------------------------
// Criterion 3 — an actor declaring neither field is unchanged
// ---------------------------------------------------------------------------

/// An actor with every field but `entrance` and `outrun` set.
fn with_guardian(c: &mut Campaign) {
    c.quests.content.actors.push(
        serde_json::from_value(json!({
            "id": "actor/the-guardian",
            "entity": "minecraft:zombie",
            "name": "The Guardian",
            "anchor": "anchor/npc-stand",
            "facing": "north",
            "equipment": { "head": "minecraft:iron_helmet", "main_hand": "minecraft:iron_sword" },
            "attributes": { "max_health": 30.0, "attack_damage": 4.0, "movement_speed": 0.23, "follow_range": 12.0 },
            "tier": "elite",
            "drops": [{ "slot": "head" }],
            "health_bar": { "range": 16 },
            "watch": { "who": "nearest", "within": 4 },
            "on_kill": { "effects": [{ "type": "set-flag", "flag": "flag/guardian-down" }] },
            "answers": { "struck": "Iron rings.", "touched": "Cold iron." }
        }))
        .expect("guardian parses"),
    );
    beat(c, "obj/talk").push(effect(
        json!({ "type": "spawn-actor", "actor": "actor/the-guardian" }),
    ));
    beat(c, "obj/slay").push(effect(json!({
        "type": "unleash-actor",
        "actor": "actor/the-guardian",
        "happening": { "verb": "arrives", "text": "the guardian stands" }
    })));
}

/// Criterion 3: the puppet summon, the twin summon at the puppet and the
/// re-seat of an actor declaring neither field are byte-identical to the
/// ground revision's (captured from it by building this fixture there).
#[test]
fn an_actor_without_an_entrance_is_unchanged() {
    let mut c = base();
    with_guardian(&mut c);
    let (out, _) = build(&c);
    let spawn = func(&out, "spawn_actor_the_guardian");
    let unleash = func(&out, "unleash_the_guardian");
    let restand = func(&out, "actor_restand_the_guardian");
    let puppet = spawn
        .lines()
        .find(|l| l.contains("summon minecraft:zombie"))
        .unwrap();
    let twin = unleash
        .lines()
        .find(|l| l.starts_with("execute at @e[tag=dw_pup_the_guardian,limit=1] run summon"))
        .unwrap();
    let reseat = restand.lines().find(|l| l.starts_with("summon ")).unwrap();
    assert_eq!(puppet, GROUND_PUPPET, "the puppet summon moved");
    assert_eq!(twin, GROUND_TWIN, "the twin summon moved");
    assert_eq!(reseat, GROUND_RESEAT, "the re-seat moved");
}

// The ground revision's bytes for `actor/the-guardian` (`with_guardian` over the
// same fixture), captured by building it at the revision this branch forks
// from, 86689c648, before any of spec-0132 existed.
const GROUND_PUPPET: &str = r#"execute unless entity @e[tag=dw_actor_the_guardian] run summon minecraft:zombie 6.5 65.0 10.5 {NoAI:1b,Silent:1b,PersistenceRequired:1b,NoGravity:1b,Invulnerable:1b,DeathLootTable:"minecraft:empty",Rotation:[180f,0f],Tags:["dw_actor","dw_actor_the_guardian","dw_pup_the_guardian","dw_watch"],CustomName:"The Guardian",CustomNameVisible:1b,attributes:[{id:"minecraft:max_health",base:30.0},{id:"minecraft:attack_damage",base:4.0},{id:"minecraft:movement_speed",base:0.23},{id:"minecraft:follow_range",base:12.0}],equipment:{mainhand:{id:"minecraft:iron_sword",count:1},head:{id:"minecraft:iron_helmet",count:1}},drop_chances:{mainhand:0.0f,head:2.0f}}"#;
const GROUND_TWIN: &str = r#"execute at @e[tag=dw_pup_the_guardian,limit=1] run summon minecraft:zombie ~ ~ ~ {PersistenceRequired:1b,DeathLootTable:"minecraft:empty",Tags:["dw_actor","dw_actor_the_guardian"],CustomName:"The Guardian",CustomNameVisible:1b,attributes:[{id:"minecraft:max_health",base:30.0},{id:"minecraft:attack_damage",base:4.0},{id:"minecraft:movement_speed",base:0.23},{id:"minecraft:follow_range",base:12.0}],equipment:{mainhand:{id:"minecraft:iron_sword",count:1},head:{id:"minecraft:iron_helmet",count:1}},drop_chances:{mainhand:0.0f,head:2.0f}}"#;
const GROUND_RESEAT: &str = r#"summon minecraft:zombie 6.5 65.0 10.5 {PersistenceRequired:1b,DeathLootTable:"minecraft:empty",Tags:["dw_actor","dw_actor_the_guardian"],CustomName:"The Guardian",CustomNameVisible:1b,attributes:[{id:"minecraft:max_health",base:30.0},{id:"minecraft:attack_damage",base:4.0},{id:"minecraft:movement_speed",base:0.23},{id:"minecraft:follow_range",base:12.0}],equipment:{mainhand:{id:"minecraft:iron_sword",count:1},head:{id:"minecraft:iron_helmet",count:1}},drop_chances:{mainhand:0.0f,head:2.0f}}"#;
