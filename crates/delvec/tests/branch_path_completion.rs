//! Every exported branch path ends where the delve ends (`DW0204` over branch
//! paths).
//!
//! A branch path (`validation/branch-path-<slug>.json`) is walked by the branch
//! runs exactly as the critical path is walked by the bot ladder, and the
//! harness fails a walk whose `campaign-complete` arrives while objective steps
//! remain. The gallery is the case: its optional `quest/the-reliquary` and its
//! finale `quest/far-hall` are both ready once `quest/near-hall` completes, and
//! the branch ordering broke that tie by id, so the reliquary's three objectives
//! were exported after `obj/reach-the-end` — three steps past the end of the
//! delve. Both halves are pinned here, on the gallery itself:
//!
//! * the ordering: a quest that can end the delve is walked last, so every
//!   reachable branch path of the gallery replays with `campaign-complete` at its
//!   final step;
//! * the gate: `DW0204` replays every exported branch path, so an ordering that
//!   puts objective steps past the ending is refused at build time, named by
//!   branch and by the file the harness would have walked.

mod common;

use std::path::{Path, PathBuf};

use delvec::compiler::analyze::analyze_campaign;
use delvec::compiler::branch::realize;
use delvec::compiler::flow::{Flow, Playthrough};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, parse_campaign};

/// The gallery's primary point, materialised the way `tools/ci/gallery_domain.py`
/// materialises it (the top-level documents and `l10n/`), with `quest_plan`
/// optionally rewritten.
fn gallery_primary(name: &str, quest_plan: impl FnOnce(&mut serde_json::Value)) -> PathBuf {
    let src = common::repo_root().join("gallery");
    let dst = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("branch-path-{name}"));
    let _ = std::fs::remove_dir_all(&dst);
    std::fs::create_dir_all(&dst).unwrap();
    for entry in std::fs::read_dir(&src).unwrap() {
        let path = entry.unwrap().path();
        let file = path.file_name().unwrap().to_string_lossy().to_string();
        if path.is_dir() {
            if file == "l10n" {
                common::copy_dir_all(&path, &dst.join("l10n"));
            }
            continue;
        }
        if file.ends_with(".json") {
            std::fs::copy(&path, dst.join(&file)).unwrap();
        }
    }
    common::patch_file(&dst.join("quest-plan.json"), quest_plan);
    dst
}

fn parse(dir: &Path) -> Campaign {
    let loaded = load_campaign_dir(dir).expect("gallery loads");
    parse_campaign(&loaded.raw).expect("gallery parses")
}

fn dw0204(c: &Campaign) -> Vec<String> {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    analyze_campaign(c, &prefabs)
        .into_iter()
        .filter(|d| d.code == "DW0204")
        .map(|d| d.message)
        .collect()
}

/// Every reachable branch's exported path, as the export builds it.
fn branch_paths(c: &Campaign) -> Vec<(String, Playthrough)> {
    let flow = Flow::new(c);
    realize(c)
        .into_iter()
        .filter_map(|r| {
            r.world
                .map(|w| (r.branch.id.clone(), flow.playthrough_in(w)))
        })
        .collect()
}

fn objectives(p: &Playthrough) -> Vec<&str> {
    p.steps.iter().map(|s| s.objective.as_str()).collect()
}

#[test]
fn every_gallery_branch_path_ends_where_the_delve_ends() {
    let c = parse(&gallery_primary("as-committed", |_| {}));
    let flow = Flow::new(&c);
    let paths = branch_paths(&c);
    // Binding: the gallery declares two branches and both are reachable; a
    // strand that is not on them would make the order question vacuous.
    assert_eq!(paths.len(), 2, "branch paths walked: {paths:?}");
    for (id, p) in &paths {
        let objs = objectives(p);
        assert_eq!(
            objs.last().copied(),
            Some("obj/reach-the-end"),
            "branch `{id}` must end on the finale's last objective: {objs:?}"
        );
        let strand = objs.iter().filter(|o| o.starts_with("obj/look-in")).count();
        assert_eq!(
            strand, 3,
            "branch `{id}` walks the optional strand: {objs:?}"
        );
        flow.replay(p)
            .unwrap_or_else(|f| panic!("branch `{id}`'s path must replay: {}", f.message()));
    }
    assert!(dw0204(&c).is_empty(), "{:?}", dw0204(&c));
}

/// The pre-fix order, rebuilt by hand: the reliquary's objectives moved behind
/// the finale's. The replay refuses it at the step the harness failed on.
#[test]
fn a_branch_path_that_runs_past_the_ending_does_not_replay() {
    let c = parse(&gallery_primary("pre-fix-order", |_| {}));
    let flow = Flow::new(&c);
    let (id, mut p) = branch_paths(&c).into_iter().next().unwrap();
    let (strand, rest): (Vec<_>, Vec<_>) = p
        .steps
        .drain(..)
        .partition(|s| s.quest == "quest/the-reliquary");
    assert_eq!(strand.len(), 3, "branch `{id}`");
    p.steps = rest.into_iter().chain(strand).collect();
    let err = flow
        .replay(&p)
        .expect_err("a path with objective steps past `campaign-complete` must not replay");
    assert_eq!(err.objective, "obj/reach-the-end");
    assert!(
        err.reason.contains("3 step(s) before the end of the path"),
        "{}",
        err.reason
    );
}

/// The perturbation only the branch half of `DW0204` can catch: the reliquary
/// made to depend on the finale. The critical path never carries an optional
/// quest, so it replays clean; only the branch paths walk the strand, and they
/// can walk it only after the delve has ended.
#[test]
fn dw0204_refuses_a_branch_path_with_steps_past_the_ending() {
    let c = parse(&gallery_primary("strand-after-finale", |plan| {
        let quests = plan["content"]["quests"].as_array_mut().unwrap();
        let reliquary = quests
            .iter_mut()
            .find(|q| q["id"] == "quest/the-reliquary")
            .expect("the gallery's optional strand");
        reliquary["depends_on"] = serde_json::json!(["quest/far-hall"]);
    }));
    let flow = Flow::new(&c);
    flow.replay(&flow.playthrough())
        .expect("the critical path carries no optional quest and still replays");
    let msgs = dw0204(&c);
    assert_eq!(msgs.len(), 2, "one refusal per branch: {msgs:?}");
    for (msg, slug) in msgs.iter().zip(["read", "skipped"]) {
        assert!(
            msg.contains(&format!("`validation/branch-path-{slug}.json`")),
            "the refusal names the file the harness walks: {msg}"
        );
        assert!(
            msg.contains("before the end of the path"),
            "the refusal says the delve ends early: {msg}"
        );
    }
}
