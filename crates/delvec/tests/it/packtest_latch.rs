//! A generated PackTest owns what its drive reads: it opens every completion's
//! latch before calling `complete_<obj>`, and it writes the map's mode before
//! any guard of its own that reads it. The suite runs as one batch on one
//! server in an order the compiler does not choose, so a template that leaves
//! either to a sibling passes or fails on the order alone.
//!
//! Two proofs on emitted bytes. The latch: in every file of the emitted
//! PackTest datapack, each `function <ns>:complete_o_<x>` is preceded by the
//! line that makes `dw.o_<x>` not-done on `#party`, counted with its
//! denominator. The mode: in a campaign whose mode holds a quest, the
//! division-of-labour template and the dialogue-mask template write
//! `dw.mode` before the first line that reads it.

use crate::common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::{DSL_VERSION, parse_campaign};
use serde_json::json;

fn build(dir: &Path) -> BuildOutput {
    let loaded = load_campaign_dir(dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("fixture parses");
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
    .expect("emission succeeds")
}

/// `and-join` with a mode that keeps the wards, so it holds the greeting and
/// the division-of-labour template's guard reads the map's mode.
fn and_join_under_a_mode() -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let dst = std::env::temp_dir().join(format!(
        "dw-packtest-latch-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dst);
    std::fs::create_dir_all(&dst).unwrap();
    let src = common::compiler_fixtures_dir().join("and-join");
    for f in common::STAGE_FILES {
        std::fs::copy(src.join(f), dst.join(f)).unwrap();
    }
    common::patch_file(&dst.join("quests.json"), |d| {
        d["dsl_version"] = json!(DSL_VERSION);
        d["content"]["modes"] = json!([{ "id": "mode/the-wards", "keeps": ["quest/wards"] }]);
        common::objective_effects(d, 1, "obj/ward-north")
            .push(json!({ "type": "enter-mode", "mode": "mode/the-wards" }));
    });
    dst
}

fn text(out: &BuildOutput, key: &str) -> String {
    String::from_utf8(out[key].clone()).unwrap()
}

/// Every emitted PackTest file (templates and the suite functions they call).
fn suite_files(out: &BuildOutput) -> Vec<(&String, String)> {
    out.iter()
        .filter(|(k, _)| {
            k.starts_with("packtest-datapack/data/")
                && k.ends_with(".mcfunction")
                && (k.contains("/test/") || k.contains("/function/"))
        })
        .map(|(k, v)| (k, String::from_utf8(v.clone()).unwrap()))
        .collect()
}

/// The sites in `files` that call an objective's completion, as
/// `(site, latch open before it)`.
fn completion_sites(files: &[(&String, String)]) -> Vec<(String, bool)> {
    let mut sites = Vec::new();
    for (name, body) in files {
        let lines: Vec<&str> = body.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let Some(at) = line.find(":complete_o_") else {
                continue;
            };
            if !line[..at].contains("function ") {
                continue;
            }
            let rest = &line[at + ":complete_o_".len()..];
            let x: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            let open = format!("scoreboard players set #party dw.o_{x} 0");
            sites.push((
                format!("{name}:{}", i + 1),
                lines[..i].iter().any(|l| *l == open),
            ));
        }
    }
    sites
}

fn assert_every_completion_opens_its_latch(out: &BuildOutput, what: &str) -> usize {
    let files = suite_files(out);
    let sites = completion_sites(&files);
    assert!(
        !sites.is_empty(),
        "{what}: no completion site found in {} PackTest file(s)",
        files.len()
    );
    let shut: Vec<&String> = sites.iter().filter(|(_, o)| !o).map(|(s, _)| s).collect();
    assert!(
        shut.is_empty(),
        "{what}: {} of {} completion site(s) in {} PackTest file(s) drive `complete_o_*` with its \
         latch not opened first: {shut:?}",
        shut.len(),
        sites.len(),
        files.len()
    );
    sites.len()
}

#[test]
fn every_generated_completion_drive_opens_its_latch_first() {
    let mut total = 0;
    for ns in ["souls-bonfire", "and-join", "keep-trial"] {
        let dir = if ns == "keep-trial" {
            common::keep_trial_dir()
        } else {
            common::compiler_fixtures_dir().join(ns)
        };
        let out = build(&dir);
        total += assert_every_completion_opens_its_latch(&out, ns);
    }
    assert!(total >= 10, "only {total} completion site(s) bound");
}

#[test]
fn a_mode_held_template_writes_the_mode_before_it_reads_it() {
    let dir = and_join_under_a_mode();
    let out = build(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    assert_every_completion_opens_its_latch(&out, "and-join under a mode");
    let mut judged = 0;
    for key in [
        "packtest-datapack/data/and-join/test/party_join_shrine.mcfunction",
        "packtest-datapack/data/and-join/test/dlg_mask_warden_greeting.mcfunction",
    ] {
        let body = text(&out, key);
        let lines: Vec<&str> = body.lines().collect();
        let read = lines
            .iter()
            .position(|l| l.starts_with("execute") && l.contains("score #party dw.mode matches"))
            .or_else(|| {
                lines
                    .iter()
                    .position(|l| l.contains("run function and-join:dmask_"))
            })
            .unwrap_or_else(|| panic!("{key}: no line reads the mode"));
        let wrote = lines
            .iter()
            .position(|l| l.starts_with("scoreboard players set #party dw.mode "))
            .unwrap_or_else(|| panic!("{key}: the mode is never written"));
        assert!(
            wrote < read,
            "{key}: the mode is written at line {} after the first read at line {}",
            wrote + 1,
            read + 1
        );
        judged += 1;
    }
    assert_eq!(judged, 2);
}
