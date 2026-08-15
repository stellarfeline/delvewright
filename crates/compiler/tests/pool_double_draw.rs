//! `DW0498` — a pool draw that seats the same anchored prefab twice.
//!
//! The failure this diagnostic replaces was silence at the pool declaration
//! followed by a per-anchor `DW0305` at whichever use site happened to reference
//! an ambiguous anchor first. The author discovered the constraint one use site
//! at a time; the pool that caused it never said a word.
//!
//! Fixtures are built against a **private** copy of the prefab library
//! (`common::copy_dir_all`) so the content repo is never written to, and the
//! pools are synthesized so the double draw is a fact of the pool's shape rather
//! than a lucky seed: the double-draw pool declares exactly ONE `connector`-role
//! member and that member carries anchors, so every filler slot the area's
//! `pieces` budget opens can only be filled by it.

mod common;

use std::path::{Path, PathBuf};

use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::pool::DW_POOL_DOUBLE_DRAW;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{Diagnostic, Severity, parse_campaign};

/// A private prefab-library copy carrying the synthetic pools this file needs.
///
/// Two fixture prefabs are synthesized alongside them: `prefab/fixture-hall-a`
/// and `prefab/fixture-hall-b`, both the shipped straight corridor's geometry
/// (same `.nbt`) with anchors bolted on. The library's real anchored rooms are
/// all single-socket dead ends, so they cannot chain into a spine; a chainable
/// piece that *declares* anchors is exactly the shape a repeated draw makes
/// ambiguous, and it has to be built here rather than imposed on the content
/// repo.
///
/// * `pool/double-draw` — entry + ONE anchored connector + the shrine terminal.
///   Every filler slot can only be that connector, so a 4-piece area seats it
///   twice. The repeat is a fact of the pool's membership, not of the seed.
/// * `pool/single-draw` — two distinct anchored connectors, drawn at a budget
///   that opens exactly one filler slot, so nothing repeats.
fn prefabs_with_pools(tag: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("pool-double-draw-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    common::copy_dir_all(&common::prefabs_dir(), &dir);

    // Fixture prefabs: the straight corridor's geometry, with anchors.
    let corridor: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("keep-corridor-straight.json")).unwrap(),
    )
    .unwrap();
    for (id, file) in [
        ("prefab/fixture-hall-a", "fixture-hall-a.json"),
        ("prefab/fixture-hall-b", "fixture-hall-b.json"),
    ] {
        let mut meta = corridor.clone();
        meta["prefab_id"] = serde_json::json!(id);
        meta["anchors"] = serde_json::json!({
            "anchor/chest":     { "pos": [2, 1, 2], "facing": "north" },
            "anchor/npc-stand": { "pos": [2, 1, 4], "facing": "south" }
        });
        std::fs::write(dir.join(file), serde_json::to_string_pretty(&meta).unwrap()).unwrap();
    }

    let pools_path = dir.join("pools.json");
    let mut pools: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&pools_path).unwrap()).unwrap();
    pools["pools"]["pool/double-draw"] = serde_json::json!({
        "members": [
            { "prefab": "prefab/keep-spawn-hall",  "weight": 1, "role": "entry" },
            { "prefab": "prefab/fixture-hall-a",   "weight": 1, "role": "connector" },
            { "prefab": "prefab/keep-shrine",      "weight": 1, "role": "terminal" }
        ]
    });
    pools["pools"]["pool/single-draw"] = serde_json::json!({
        "members": [
            { "prefab": "prefab/keep-spawn-hall",  "weight": 1, "role": "entry" },
            { "prefab": "prefab/fixture-hall-a",   "weight": 1, "role": "connector" },
            { "prefab": "prefab/fixture-hall-b",   "weight": 1, "role": "connector" },
            { "prefab": "prefab/keep-shrine",      "weight": 1, "role": "terminal" }
        ]
    });
    std::fs::write(&pools_path, serde_json::to_string_pretty(&pools).unwrap()).unwrap();
    dir
}

/// keep-crawl with its pool area rebound to `pool_id` at a fixed piece count, and
/// its `open-gate` effect dropped so `anchor/objective` is the area's only
/// campaign-referenced anchor (one terminal → the straight spine, whose filler
/// draw is fully determined by the pool's connector membership).
fn campaign_on_pool(tag: &str, pool_id: &str, pieces: u32) -> PathBuf {
    campaign_on_pool_with(tag, pool_id, pieces, false)
}

/// As [`campaign_on_pool`]; with `use_ambiguous_anchor` the keeper is moved into
/// the pool area and stood on `anchor/npc-stand` — an anchor the doubled
/// connector carries, i.e. the use site that turns the ambiguity into a hard
/// `DW0305`.
fn campaign_on_pool_with(
    tag: &str,
    pool_id: &str,
    pieces: u32,
    use_ambiguous_anchor: bool,
) -> PathBuf {
    let dst = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("pool-campaign-{tag}"));
    let _ = std::fs::remove_dir_all(&dst);

    let base = common::keep_crawl_dir();
    let mut world: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(base.join("world.json")).unwrap()).unwrap();
    world["content"]["areas"][1]["prefab_pool"] = serde_json::json!(pool_id);
    world["content"]["areas"][1]["pieces"] = serde_json::json!({ "min": pieces, "max": pieces });

    let mut quests: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(base.join("quests.json")).unwrap()).unwrap();
    quests["content"]["quests"][0]["on_objective_complete"]["obj/talk"] = serde_json::json!([]);

    let mut documents = serde_json::json!({ "world": world, "quests": quests });
    if use_ambiguous_anchor {
        let mut npcs: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(base.join("npcs.json")).unwrap())
                .unwrap();
        npcs["content"]["npcs"][0]["area"] = serde_json::json!("area/keep");
        npcs["content"]["npcs"][0]["anchor"] = serde_json::json!("anchor/npc-stand");
        documents["npcs"] = npcs;
    }

    common::materialize_from(&base, &serde_json::json!({ "documents": documents }), &dst);
    dst
}

fn plan_warnings(campaign_dir: &Path, prefabs_dir: &Path) -> Vec<Diagnostic> {
    let loaded = load_campaign_dir(campaign_dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("fixture campaign parses");
    let prefabs = PrefabRegistry::load_dir(prefabs_dir).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("fixture plans");
    plan.warnings.clone()
}

fn placed_ids(campaign_dir: &Path, prefabs_dir: &Path) -> Vec<String> {
    let loaded = load_campaign_dir(campaign_dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).unwrap();
    let prefabs = PrefabRegistry::load_dir(prefabs_dir).unwrap();
    let plan = Plan::build(&campaign, &prefabs).unwrap();
    plan.areas[1]
        .pieces
        .iter()
        .map(|p| p.prefab_id.clone())
        .collect()
}

/// RED→GREEN: a pool whose only filler is an anchored prefab seats that prefab
/// twice, and the build now says so ONCE, at the pool/area declaration — naming
/// the prefab, the repeat count, and every anchor the repeat makes ambiguous.
#[test]
fn double_drawn_anchored_prefab_is_dw0498() {
    let prefabs = prefabs_with_pools("double");
    let dir = campaign_on_pool("double", "pool/double-draw", 4);

    // Precondition (the fixture is only meaningful if the draw really repeats).
    let ids = placed_ids(&dir, &prefabs);
    assert_eq!(
        ids.iter().filter(|p| *p == "prefab/fixture-hall-a").count(),
        2,
        "fixture must double-draw the anchored connector, placed: {ids:?}"
    );

    let warnings = plan_warnings(&dir, &prefabs);
    let hits: Vec<&Diagnostic> = warnings.iter().filter(|d| d.code == "DW0498").collect();
    assert_eq!(
        hits.len(),
        1,
        "exactly one DW0498 for the area, got: {warnings:#?}"
    );
    let d = hits[0];
    assert_eq!(d.code, DW_POOL_DOUBLE_DRAW);
    assert_eq!(
        d.severity,
        Severity::Warning,
        "a double draw with no ambiguous-anchor USE is legal today — advisory only"
    );
    assert_eq!(d.stage, "world");
    assert_eq!(
        d.path, "/content/areas/1",
        "reported at the area declaration"
    );

    let m = &d.message;
    assert!(m.contains("pool/double-draw"), "names the pool: {m}");
    assert!(m.contains("area/keep"), "names the area: {m}");
    assert!(
        m.contains("prefab/fixture-hall-a"),
        "names the doubled prefab: {m}"
    );
    assert!(m.contains(" 2 "), "names the repeat count: {m}");
    // Every anchor the doubled prefab declares becomes ambiguous.
    assert!(
        m.contains("anchor/chest"),
        "names the ambiguous anchor: {m}"
    );
    assert!(
        m.contains("anchor/npc-stand"),
        "names every ambiguous anchor, not just the first: {m}"
    );
    // The prescription, and the consequence it is protecting against.
    assert!(m.contains("DW0305"), "names the use-site consequence: {m}");
    assert!(
        m.contains("distinct") || m.contains("variant"),
        "prescribes widening the pool: {m}"
    );
    // A prefab the draw seated exactly once must not be dragged in.
    assert!(
        !m.contains("prefab/keep-shrine"),
        "only doubled prefabs are named: {m}"
    );
}

/// Control 1 — a pool with room for distinct fillers, drawn at a piece budget
/// that opens one filler slot: nothing repeats, nothing is said.
#[test]
fn single_draw_pool_is_silent() {
    let prefabs = prefabs_with_pools("single");
    let dir = campaign_on_pool("single", "pool/single-draw", 3);

    let ids = placed_ids(&dir, &prefabs);
    let mut sorted = ids.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        ids.len(),
        "control draws no piece twice: {ids:?}"
    );

    let warnings = plan_warnings(&dir, &prefabs);
    assert!(
        !warnings.iter().any(|d| d.code == "DW0498"),
        "a draw with no repeat is silent: {warnings:#?}"
    );
}

/// Control 2 — the shipped shape. `keep-crawl` on `pool/stone-keep` repeats its
/// corridor fillers happily, and MUST stay silent: an anchorless filler makes no
/// anchor ambiguous, and a warning on every campaign that uses fillers would be
/// noise, not information.
#[test]
fn repeated_anchorless_filler_is_silent() {
    let prefabs = common::prefabs_dir();
    let dir = common::keep_crawl_dir();

    let ids = placed_ids(&dir, &prefabs);
    let repeats = ids
        .iter()
        .filter(|p| ids.iter().filter(|q| q == p).count() > 1)
        .count();
    assert!(
        repeats > 0,
        "keep-crawl really does repeat a filler: {ids:?}"
    );

    let warnings = plan_warnings(&dir, &prefabs);
    assert!(
        !warnings.iter().any(|d| d.code == "DW0498"),
        "anchorless fillers repeat by design: {warnings:#?}"
    );
}

/// The use site the warning is about. Standing an NPC on an anchor the doubled
/// connector carries still fails the build with `DW0305` — unchanged, that is
/// the correct verdict — but the failure arrives WITH the pool-level
/// explanation instead of alone, so the author is not left with the symptom and
/// no way to infer the pool.
#[test]
fn dw0305_use_site_carries_the_pool_explanation() {
    let prefabs = prefabs_with_pools("use-site");
    let dir = campaign_on_pool_with("use-site", "pool/double-draw", 4, true);

    let loaded = load_campaign_dir(&dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).unwrap();
    let registry = PrefabRegistry::load_dir(&prefabs).unwrap();
    let err = match Plan::build(&campaign, &registry) {
        Err(e) => e,
        Ok(_) => panic!("the referenced anchor is ambiguous — the build must fail"),
    };

    assert_eq!(err.code, "DW0305", "the use-site verdict is unchanged");
    assert!(
        err.message.contains("anchor/npc-stand"),
        "DW0305 still names the anchor: {}",
        err.message
    );
    let expl = err
        .warnings
        .iter()
        .find(|d| d.code == DW_POOL_DOUBLE_DRAW)
        .unwrap_or_else(|| panic!("the failure carries its DW0498: {:#?}", err.warnings));
    assert!(
        expl.message.contains("prefab/fixture-hall-a") && expl.message.contains("pool/double-draw"),
        "the explanation names the pool and the repeated piece: {}",
        expl.message
    );
}

/// A pool that repeats an anchored piece stays deterministic: same campaign, same
/// seed → the same single diagnostic, byte-for-byte (ADR-0006).
#[test]
fn dw0498_is_deterministic() {
    let prefabs = prefabs_with_pools("determinism");
    let dir = campaign_on_pool("determinism", "pool/double-draw", 4);
    let a = plan_warnings(&dir, &prefabs);
    let b = plan_warnings(&dir, &prefabs);
    assert_eq!(a, b, "the pool diagnostic is a function of the pinned draw");
}
