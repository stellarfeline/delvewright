//! task #125 — the campaign mechanism PackTest vs scheduled endings and
//! declared branches (the-wake escalation).
//!
//! Two defects in one template, both structural:
//!
//!   1. `campaign-complete` may sit at any nesting depth (spec-0025 / DW0481);
//!      the-wake schedules it 250t into its closing `sequence`. The old template
//!      drove every completion and `assert`ed **in the same tick**, so the
//!      assert was structurally unreachable ("Expected #party dw.campaign to
//!      match 1, but got 0 on tick 0").
//!   2. It drove BOTH branches' terminal objectives in one tick — a state no
//!      playthrough reaches; the template did not model branches at all.
//!
//! The fixes under test: a scheduled ending is `await`ed with the timeout sized
//! by the tail the emitter itself scheduled; a branch campaign drives one
//! coherent per-branch path per scheduler-serialized phase; and the ending tail
//! is exported on the terminal `assert-complete` step (`ending_tail_ticks`) so
//! the harness completion window covers it too.

mod common;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::parse_campaign;

/// Per-call scratch dir (cargo runs tests in parallel; see `packtest_batch.rs`).
fn scratch_dir(kind: &str) -> std::path::PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "dw-packtest-campaign-{kind}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ))
}

/// Build a campaign directory through the real emitter (mirrors the other
/// integration suites' builders).
fn build_dir(dir: &Path) -> BuildOutput {
    let loaded = load_campaign_dir(dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("valid campaign parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");

    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let bytes = std::fs::read(common::prefabs_dir().join(&piece.structure_file)).unwrap();
            structures.insert(piece.structure_file.clone(), bytes);
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
        "unpinned",
        &BTreeMap::new(),
    )
    .expect("emission succeeds")
}

fn text(out: &BuildOutput, path: &str) -> String {
    String::from_utf8(
        out.get(path)
            .unwrap_or_else(|| panic!("missing {path}: {:?}", out.keys()))
            .clone(),
    )
    .unwrap()
}

/// hello-world with its finale `campaign-complete` moved 240t into a closing
/// `sequence` — the-wake's shape, minimized.
fn build_scheduled_hello_world() -> BuildOutput {
    let src = common::hello_world_dir();
    let dst = scratch_dir("sched");
    let _ = std::fs::remove_dir_all(&dst);
    std::fs::create_dir_all(&dst).unwrap();
    for f in common::STAGE_FILES {
        std::fs::copy(src.join(f), dst.join(f)).unwrap();
    }
    let search = r#"        "on_complete": [
          {
            "type": "campaign-complete"
          }
        ]"#;
    let replace = r#"        "on_complete": [
          {
            "type": "sequence",
            "steps": [
              { "at_ticks": 0, "effects": [ { "type": "narrate", "style": "chat", "text": "The door swings wide." } ] },
              { "at_ticks": 240, "effects": [ { "type": "campaign-complete" } ] }
            ]
          }
        ]"#;
    let qp = dst.join("quests.json");
    let q = std::fs::read_to_string(&qp)
        .unwrap()
        .replace("\"dsl_version\": \"0.2.0\"", "\"dsl_version\": \"0.6.0\"")
        .replace(search, replace);
    assert!(q.contains("at_ticks"), "quests.json patch applied");
    std::fs::write(&qp, q).unwrap();
    let out = build_dir(&dst);
    let _ = std::fs::remove_dir_all(&dst);
    out
}

/// branch-two-endings with the bolt branch's `campaign-complete` moved 200t into
/// a `sequence` — a branch campaign whose branches differ in ending tail.
fn build_scheduled_branch_fixture() -> BuildOutput {
    let src = common::compiler_fixtures_dir().join("branch-two-endings");
    let dst = scratch_dir("branch-sched");
    let _ = std::fs::remove_dir_all(&dst);
    std::fs::create_dir_all(&dst).unwrap();
    for f in common::STAGE_FILES {
        std::fs::copy(src.join(f), dst.join(f)).unwrap();
    }
    // Wrap the losing ending in a `sequence` so the ending fires 200 ticks
    // later — the AWAITED-ending shape the template below must handle.
    common::patch_file(&dst.join("quests.json"), |d| {
        let effects = common::objective_effects(d, 1, "obj/bolt");
        let idx = effects
            .iter()
            .position(|e| e["type"] == "campaign-complete")
            .expect("quest/bolt still ends the campaign on obj/bolt");
        let ending = effects[idx].clone();
        effects[idx] = serde_json::json!({
            "type": "sequence",
            "steps": [ { "at_ticks": 200, "effects": [ending] } ]
        });
    });
    let out = build_dir(&dst);
    let _ = std::fs::remove_dir_all(&dst);
    out
}

/// A scheduled ending is AWAITED, with the template timeout sized by the tail
/// the emitter itself scheduled — never a same-tick assert (the-wake defect 1).
#[test]
fn scheduled_ending_campaign_template_awaits_the_tail() {
    let out = build_scheduled_hello_world();
    let test = text(
        &out,
        "packtest-datapack/data/hello-world/test/campaign.mcfunction",
    );
    assert!(
        test.contains("await score #party dw.campaign matches 1"),
        "the template must await the scheduled ending:\n{test}"
    );
    assert!(
        !test.contains("assert score"),
        "a same-tick assert is structurally unreachable past a scheduled ending:\n{test}"
    );
    assert!(
        test.contains("# @timeout 340"),
        "timeout = 100 base + the 240t scheduled tail:\n{test}"
    );
    // The template spans ticks, so shared progression state (quest-active
    // baselines) is hoisted into the drive function: the template's only
    // cross-tick surface is the completion objective it alone owns
    // (packtest_batch::party_state_across_ticks_is_owned).
    assert!(
        test.contains("function hello-world:pt_camp_drive"),
        "the drive is hoisted into pt_camp_drive:\n{test}"
    );
    let drive = text(
        &out,
        "packtest-datapack/data/hello-world/function/pt_camp_drive.mcfunction",
    );
    for line in [
        "scoreboard players set #party dw.campaign 0",
        "run function hello-world:complete_o_talk",
        "run function hello-world:complete_o_exit",
    ] {
        assert!(
            drive.contains(line),
            "drive must contain `{line}`:\n{drive}"
        );
    }

    // The same tail is exported for the harness tier: the terminal
    // `assert-complete` step carries `ending_tail_ticks`.
    let cp: serde_json::Value = serde_json::from_slice(&out["critical-path.json"]).unwrap();
    let last = cp["steps"].as_array().unwrap().last().unwrap();
    assert_eq!(last["action"], "assert-complete");
    assert_eq!(
        last["ending_tail_ticks"], 240,
        "the exported completion window must cover the scheduled tail: {last}"
    );
}

/// A synchronous-ending, branch-free campaign keeps the original single-tick
/// template — and its exported path carries no tail field (byte-identity).
#[test]
fn synchronous_ending_keeps_the_single_tick_template() {
    let out = build_dir(&common::hello_world_dir());
    let test = text(
        &out,
        "packtest-datapack/data/hello-world/test/campaign.mcfunction",
    );
    assert!(test.contains("assert score #party dw.campaign matches 1"));
    assert!(test.contains("# @timeout 100"));
    assert!(!test.contains("await"));
    assert!(
        !out.contains_key("packtest-datapack/data/hello-world/function/pt_camp_drive.mcfunction")
    );
    let cp: serde_json::Value = serde_json::from_slice(&out["critical-path.json"]).unwrap();
    let last = cp["steps"].as_array().unwrap().last().unwrap();
    assert_eq!(last["action"], "assert-complete");
    assert!(
        last.get("ending_tail_ticks").is_none(),
        "no tail field for a synchronous ending: {last}"
    );
}

/// A branch campaign's mechanism test drives one coherent path per branch —
/// never both terminals in one pass (the-wake defect 2) — with each phase's
/// verdict taken after that branch's own scheduled tail.
#[test]
fn branch_campaign_template_drives_each_branch_coherently() {
    let out = build_scheduled_branch_fixture();
    let ns = "hello-world"; // the fixture reuses hello-world's campaign_id

    let test = text(
        &out,
        &format!("packtest-datapack/data/{ns}/test/campaign.mcfunction"),
    );
    assert!(
        test.contains("await score #camp_phase dw.sys matches 2"),
        "one verdict per reachable branch:\n{test}"
    );
    assert!(
        test.contains(&format!("function {ns}:pt_camp_run_0")),
        "the template starts the phase chain:\n{test}"
    );
    // timeout = 100 base + (hold: 0 tail + 20 margin) + (bolt: 200 tail + 20 margin)
    assert!(
        test.contains("# @timeout 340"),
        "timeout sums each phase's tail + margin:\n{test}"
    );

    let run0 = text(
        &out,
        &format!("packtest-datapack/data/{ns}/function/pt_camp_run_0.mcfunction"),
    );
    let run1 = text(
        &out,
        &format!("packtest-datapack/data/{ns}/function/pt_camp_run_1.mcfunction"),
    );

    // Phase 0 = branch/hold: decide -> watch -> walk-out, never the bolt terminal.
    for f in [
        "complete_o_decide",
        "complete_o_watch",
        "complete_o_walk_out",
    ] {
        assert!(run0.contains(f), "hold phase drives {f}:\n{run0}");
    }
    assert!(
        !run0.contains("complete_o_bolt"),
        "the hold phase must not drive the bolt branch's terminal:\n{run0}"
    );
    // The branch's scripted dialogue choice is emulated as the flag it sets.
    assert!(
        run0.contains("scoreboard players set #party dw.f_wait 1"),
        "hold phase sets its branch flag:\n{run0}"
    );
    assert!(
        !run0.contains("dw.f_flee 1"),
        "hold phase must not set the sibling branch's flag:\n{run0}"
    );

    // Phase 1 = branch/bolt: decide -> bolt, never the hold-only objectives.
    assert!(
        run1.contains("complete_o_bolt"),
        "bolt phase drives its terminal:\n{run1}"
    );
    assert!(
        !run1.contains("complete_o_watch") && !run1.contains("complete_o_walk_out"),
        "the bolt phase must not drive the hold branch's objectives:\n{run1}"
    );
    assert!(
        run1.contains("scoreboard players set #party dw.f_flee 1"),
        "bolt phase sets its branch flag:\n{run1}"
    );
    // Each phase re-baselines the whole progression surface (a prior phase's
    // completed quest would otherwise keep its guarded on_complete from
    // re-firing).
    for line in [
        "scoreboard players set #party dw.campaign 0",
        "scoreboard players set #party dw.q_bolt 0",
        "scoreboard players set #party dw.o_decide 0",
        "scoreboard players set #party dw.f_wait 0",
    ] {
        assert!(
            run1.contains(line),
            "phase re-baseline must contain `{line}`:\n{run1}"
        );
    }
    // The bolt phase's verdict waits out ITS ending tail (200t) + margin.
    assert!(
        run1.contains(&format!("schedule function {ns}:pt_camp_check_1 220t")),
        "the bolt verdict is scheduled past the branch's 200t tail:\n{run1}"
    );
    assert!(
        run0.contains(&format!("schedule function {ns}:pt_camp_check_0 20t")),
        "the hold verdict needs only the margin (synchronous ending):\n{run0}"
    );

    // The checks: verdict + chain to the next phase; the last check ends the chain.
    let chk0 = text(
        &out,
        &format!("packtest-datapack/data/{ns}/function/pt_camp_check_0.mcfunction"),
    );
    let chk1 = text(
        &out,
        &format!("packtest-datapack/data/{ns}/function/pt_camp_check_1.mcfunction"),
    );
    for chk in [&chk0, &chk1] {
        assert!(
            chk.contains("execute if score #party dw.campaign matches 1 run")
                && chk.contains("scoreboard players add #camp_phase dw.sys 1"),
            "each check takes the phase verdict:\n{chk}"
        );
    }
    assert!(chk0.contains(&format!("function {ns}:pt_camp_run_1")));
    assert!(
        !chk1.contains("pt_camp_run"),
        "the last check ends the chain:\n{chk1}"
    );

    // Every phase function is reachable from the template through the packtest
    // function graph (PackTest scans /test/ only; an orphan under /function/
    // would never run).
    let mut reached: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut frontier = vec![test.clone()];
    while let Some(body) = frontier.pop() {
        for (path, bytes) in &out {
            let Some(name) = path
                .strip_prefix(&format!("packtest-datapack/data/{ns}/function/"))
                .and_then(|p| p.strip_suffix(".mcfunction"))
            else {
                continue;
            };
            if body.contains(&format!("{ns}:{name}")) && reached.insert(name.to_string()) {
                frontier.push(String::from_utf8(bytes.clone()).unwrap());
            }
        }
    }
    for path in out.keys() {
        if let Some(name) = path
            .strip_prefix(&format!("packtest-datapack/data/{ns}/function/"))
            .and_then(|p| p.strip_suffix(".mcfunction"))
            && name.starts_with("pt_camp_")
        {
            assert!(
                reached.contains(name),
                "phase function `{name}` is not reachable from the campaign template"
            );
        }
    }

    // Harness tier: each branch path exports its OWN ending tail.
    let bolt: serde_json::Value =
        serde_json::from_slice(&out["validation/branch-path-bolt.json"]).unwrap();
    let last = bolt["steps"].as_array().unwrap().last().unwrap();
    assert_eq!(last["ending_tail_ticks"], 200, "bolt path tail: {last}");
    let hold: serde_json::Value =
        serde_json::from_slice(&out["validation/branch-path-hold.json"]).unwrap();
    let last = hold["steps"].as_array().unwrap().last().unwrap();
    assert!(
        last.get("ending_tail_ticks").is_none(),
        "hold's ending is synchronous: {last}"
    );
}

/// The unpatched branch fixture (both endings synchronous) still gets the
/// branch-aware phased template — branch coherence is about exclusivity, not
/// about scheduling.
#[test]
fn synchronous_branch_campaign_still_gets_per_branch_phases() {
    let out = build_dir(&common::compiler_fixtures_dir().join("branch-two-endings"));
    let ns = "hello-world";
    let test = text(
        &out,
        &format!("packtest-datapack/data/{ns}/test/campaign.mcfunction"),
    );
    assert!(test.contains("await score #camp_phase dw.sys matches 2"));
    assert!(
        test.contains("# @timeout 140"),
        "100 base + 2 × 20t margin:\n{test}"
    );
    let run0 = text(
        &out,
        &format!("packtest-datapack/data/{ns}/function/pt_camp_run_0.mcfunction"),
    );
    assert!(!run0.contains("complete_o_bolt"));
}
