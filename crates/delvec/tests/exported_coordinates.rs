//! **Every harness document that names an exported step names the step the
//! exported path carries there.** The death plan's seats (`in_force_after`) and
//! crush gates (`crossed_at`) and the combat plan's encounters (`step`) are
//! indices into `critical-path.json`, whose layout `Plan::exported_layout`
//! decides: the walked steps, a `rest` where the party rests at each bonfire
//! (deferred past a crossing out of the fire's area), and each assembly's
//! witness steps before its first strike.
//!
//! Asked of the gallery primary, which carries all three splices: a bonfire
//! armed in the near hall and rested at after the long hall, and the sentinel's
//! witness steps. Each document's index is held to the step it must name, read
//! off the emitted path itself — never off the arithmetic that produced it.

mod common;

use std::path::Path;
use std::process::Command;

use serde_json::Value;

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn read(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

/// The objective whose `on_objective_complete` sets each checkpoint anchor.
fn firing_objectives(quests: &Value) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for q in quests["content"]["quests"].as_array().unwrap() {
        if let Some(map) = q["on_objective_complete"].as_object() {
            for (obj, effs) in map {
                for e in effs.as_array().unwrap() {
                    if e["type"] == "set-checkpoint" {
                        out.push((e["anchor"].as_str().unwrap().to_string(), obj.clone()));
                    }
                }
            }
        }
    }
    out
}

#[test]
fn every_exported_index_a_harness_document_states_is_the_step_it_names() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("exported-coordinates");
    let (campaign, prefabs) = common::gallery_primary_at(&dir.join("gallery"));
    let out = dir.join("build");
    let _ = std::fs::remove_dir_all(&out);
    let r = Command::new(BIN)
        .arg("build")
        .arg(&campaign)
        .arg("-o")
        .arg(&out)
        .arg("--prefabs")
        .arg(&prefabs)
        .output()
        .expect("delvec runs");
    assert!(
        r.status.success(),
        "the gallery primary builds:\n{}",
        String::from_utf8_lossy(&r.stderr)
    );
    let path = read(&out.join("critical-path.json"));
    let steps = path["steps"].as_array().unwrap();
    let death = read(&out.join("validation/death-plan.json"));
    let combat = read(&out.join("validation/combat-plan.json"));
    let quests = read(&campaign.join("quests.json"));
    let fired_by = firing_objectives(&quests);
    // Anchors a dialogue option also sets: their seats are judged as plain
    // steps only, since the option's step is not an objective's own.
    let dialogue = std::fs::read_to_string(campaign.join("dialogue.json")).unwrap();
    let set_in_dialogue = |anchor: &str| {
        let d: Value = serde_json::from_str(&dialogue).unwrap();
        let mut found = false;
        let mut stack = vec![&d];
        while let Some(v) = stack.pop() {
            match v {
                Value::Object(m) => {
                    if m.get("type") == Some(&Value::from("set-checkpoint"))
                        && m.get("anchor") == Some(&Value::from(anchor))
                    {
                        found = true;
                    }
                    stack.extend(m.values());
                }
                Value::Array(a) => stack.extend(a.iter()),
                _ => {}
            }
        }
        found
    };

    // The splices this test exists for are present, so it can tell the
    // coordinate systems apart.
    let rests = steps.iter().filter(|s| s["action"] == "rest").count();
    let witnesses = steps
        .iter()
        .filter(|s| s["action"] == "witness-strike")
        .count();
    assert!(
        rests > 0 && witnesses > 0,
        "{rests} rest(s), {witnesses} witness step(s)"
    );

    let mut judged = Vec::new();
    let mut bad = Vec::new();
    for seat in death["placement"]["seats"].as_array().unwrap() {
        let label = seat["label"].as_str().unwrap();
        let Some(at) = seat["in_force_after"].as_i64() else {
            continue;
        };
        if at < 0 {
            continue;
        }
        let step = &steps[at as usize];
        let anchor = label.split('`').nth(1).unwrap_or("");
        if label.starts_with("bonfire") {
            judged.push(label.to_string());
            if step["action"] != "rest" || step["bonfire"] != seat["cp"] {
                bad.push(format!("{label}: in force after {at}, which is {step}"));
            }
        } else if let Some((_, obj)) = fired_by
            .iter()
            .find(|(a, _)| a == anchor && !set_in_dialogue(anchor))
        {
            // A checkpoint an objective's completion sets: the step at its
            // index proves that objective (several checkpoints may share an
            // anchor; one of them names the step).
            // A `reach` that completes on landing completes during the carry
            // before it, so its checkpoint is in force after the carrier.
            let next = steps[at as usize + 1..]
                .iter()
                .find(|s| s["action"] != "rest" && s["action"] != "witness-strike");
            let names = |o: &str| {
                step["objective"] == o
                    || next
                        .is_some_and(|n| n["objective"] == o && n["completed_on_landing"] == true)
            };
            let any = fired_by
                .iter()
                .filter(|(a, _)| a == anchor)
                .any(|(_, o)| names(o));
            judged.push(label.to_string());
            if !any {
                bad.push(format!(
                    "{label} (set when `{obj}` completes): in force after {at}, which is {step}"
                ));
            }
        } else if step["action"] == "rest" || step["action"] == "witness-strike" {
            judged.push(label.to_string());
            bad.push(format!(
                "{label}: in force after {at}, a spliced step {step}"
            ));
        }
    }
    let encounters = combat["encounters"].as_array().unwrap();
    for e in encounters {
        let at = e["step"].as_u64().unwrap() as usize;
        judged.push(format!("encounter {}", e["objective"]));
        if steps[at]["action"] != "kill" || steps[at]["objective"] != e["objective"] {
            bad.push(format!(
                "encounter {}: step {at}, which is {}",
                e["objective"], steps[at]
            ));
        }
    }
    for g in death["crush_gates"].as_array().unwrap() {
        if let Some(at) = g["crossed_at"].as_u64() {
            judged.push(format!("crush gate {}", g["id"]));
            let a = &steps[at as usize]["action"];
            if a == "rest" || a == "witness-strike" {
                bad.push(format!(
                    "crush gate {}: crossed at a spliced step {at}",
                    g["id"]
                ));
            }
        }
    }
    eprintln!(
        "exported coordinates: {} index(es) judged over {} exported step(s) ({rests} rest, \
         {witnesses} witness): {}",
        judged.len(),
        steps.len(),
        judged.join(", ")
    );
    assert!(judged.len() >= 3, "too little was judged: {judged:?}");
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
