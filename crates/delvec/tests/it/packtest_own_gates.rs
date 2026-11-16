//! Every gate a generated PackTest template drives through is the template's
//! own (`delvec::compiler::batchstate::own_gates`, judged by `DW0807`).
//!
//! Two shapes a festival town met, each reduced to a fixture: a mode that
//! holds a quest with an AND-joined objective (the division-of-labour
//! template's guard reads the map's mode), and a limit whose expiry fails a
//! fight a trigger's gate stands over (the expiry template reaches that
//! trigger's gate and the fight's guards). Each builds, and the template that
//! met the wall is one the pass gave its terms.

use crate::common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildFailure, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::DSL_VERSION;
use delvewright_dsl::parse_campaign;
use serde_json::{Value, json};

fn scratch(kind: &str) -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let d = std::env::temp_dir().join(format!(
        "dw-own-gates-{kind}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn copy_fixture(src: &Path, kind: &str) -> PathBuf {
    let dst = scratch(kind);
    for f in common::STAGE_FILES {
        std::fs::copy(src.join(f), dst.join(f)).unwrap();
    }
    dst
}

fn build(dir: &Path) -> Result<BuildOutput, BuildFailure> {
    let loaded = load_campaign_dir(dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("valid campaign parses");
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
    let tree = CommandTree::v1_21_11();
    emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &tree,
        &prefabs,
        None,
        &BTreeMap::new(),
    )
}

fn own_gates(out: &BuildOutput) -> Value {
    serde_json::from_slice(&out["validation/packtest-own-gates.json"]).unwrap()
}

/// The terms the pass wrote into `template`, empty when it wrote none.
fn written(record: &Value, template: &str) -> Vec<String> {
    record["owned"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["template"] == template)
        .map(|r| {
            r["writes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn unwrap_build(r: Result<BuildOutput, BuildFailure>) -> BuildOutput {
    match r {
        Ok(out) => out,
        Err(BuildFailure::Diagnostic { code, message }) => {
            panic!("build refused: {code}: {message}")
        }
        Err(BuildFailure::Validation(e)) => panic!("command validation: {e:?}"),
    }
}

/// The shape both cases stand on: and-join's greeting talk runs on a clock,
/// and a mode, entered when the north ward is drawn, keeps the wards and so
/// holds the greeting's clock. The wards' completion leaves the mode, which
/// starts the held clock again; its expiry completes the greeting, which arms
/// the wards and so writes their objectives' scores.
fn held_clock(d: &mut Value, gate: Option<Value>) {
    d["dsl_version"] = json!(DSL_VERSION);
    d["content"]["modes"] = json!([{ "id": "mode/the-wards", "keeps": ["quest/wards"] }]);
    common::objective_effects(d, 1, "obj/ward-north")
        .push(json!({ "type": "enter-mode", "mode": "mode/the-wards" }));
    let talk = &mut d["content"]["quests"][0]["objectives"][0];
    talk["limit"] = json!({
        "from": "activation",
        "ticks": 600,
        "on_expire": [{ "type": "narrate", "style": "chat", "text": "The keeper turns away." }]
    });
    if let Some(g) = gate {
        talk["requires_state"] = g;
    }
}

/// The mode case: the division-of-labour template's drive completes the
/// wards, leaves the mode, and reaches the held clock's expiry, whose guards
/// read the map's mode and the third quest's scores.
fn and_join_held_by_a_mode() -> PathBuf {
    let dir = copy_fixture(
        &common::compiler_fixtures_dir().join("and-join"),
        "mode-join",
    );
    common::patch_file(&dir.join("quests.json"), |d| held_clock(d, None));
    dir
}

/// The trigger case: the clock's gate reads a census, so its expiry template
/// stops the census tick while the clock runs and starts it again after; that
/// tick reaches a trigger on a second census that fails the shrine, which
/// completes the wards, leaves the mode and expires the clock: the template
/// reaches the trigger's gate and the wards' completion guard.
fn and_join_with_a_raid_on_the_clock() -> PathBuf {
    let dir = copy_fixture(&common::compiler_fixtures_dir().join("and-join"), "raid");
    common::patch_file(&dir.join("quests.json"), |d| {
        held_clock(
            d,
            Some(json!([{ "op": "at-least", "state": "state/sheep-home", "value": 0 }])),
        );
        d["content"]["state"] = json!([
            {
                "id": "state/sheep-home",
                "scope": "party",
                "counts": { "of": { "kind": "minecraft:sheep" }, "region": { "anchor": "anchor/door", "extent": [3, 1, 3] } }
            },
            {
                "id": "state/raiders-in",
                "scope": "party",
                "initial": 0,
                "counts": { "of": { "kind": "minecraft:pillager" }, "region": { "anchor": "anchor/objective", "extent": [3, 2, 3] } }
            }
        ]);
        d["content"]["triggers"] = json!([{
            "id": "trigger/raid",
            "on": { "on": "body-enters", "state": "state/raiders-in" },
            "requires_state": [{ "op": "at-least", "state": "state/raiders-in", "value": 1 }],
            "forbids_flags": ["flag/raided"],
            "effects": [
                { "type": "set-flag", "flag": "flag/raided" },
                { "type": "fail-objective", "objective": "obj/shrine" }
            ]
        }]);
    });
    dir
}

/// Build `dir`, print what the pass did, and return the output.
fn built(dir: &Path) -> BuildOutput {
    let out = unwrap_build(build(dir));
    let _ = std::fs::remove_dir_all(dir);
    out
}

/// Whether `term` is written in `template`'s body before its first line
/// matching `drive`.
fn written_before(out: &BuildOutput, template: &str, term: &str, drive: &str) -> bool {
    let body = String::from_utf8(
        out[&format!("packtest-datapack/data/and-join/test/{template}.mcfunction")].clone(),
    )
    .unwrap();
    let lines: Vec<&str> = body.lines().collect();
    let w = lines
        .iter()
        .position(|l| l.starts_with(&format!("scoreboard players set #party {term} ")));
    let d = lines.iter().position(|l| l.contains(drive));
    matches!((w, d), (Some(w), Some(d)) if w < d)
}

#[test]
fn a_join_template_under_a_mode_owns_the_mode_and_the_held_quest() {
    let out = built(&and_join_held_by_a_mode());
    let record = own_gates(&out);
    let terms = written(&record, "party_join_shrine");
    for t in ["dw.mode", "dw.qa_greet", "dw.x_talk"] {
        assert!(
            terms.iter().any(|x| x == t),
            "the join template was not given `{t}`: {record}"
        );
        assert!(
            written_before(
                &out,
                "party_join_shrine",
                t,
                "function and-join:complete_o_"
            ),
            "`{t}` is not written before the drive"
        );
    }
    assert!(
        record["unvalued"].as_array().unwrap().is_empty(),
        "{record}"
    );
}

#[test]
fn an_expiry_template_owns_the_gate_of_a_trigger_its_tick_reaches() {
    let out = built(&and_join_with_a_raid_on_the_clock());
    let record = own_gates(&out);
    let terms = written(&record, "limit_o_talk_expires");
    for t in ["dw.f_raided", "dw.s_raiders_in"] {
        assert!(
            terms.iter().any(|x| x == t),
            "the expiry template was not given `{t}`: {record}"
        );
    }
    assert!(
        record["unvalued"].as_array().unwrap().is_empty(),
        "{record}"
    );
}
