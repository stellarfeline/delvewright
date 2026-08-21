//! `DW0495` — no emitted comparison reads a score entry the pack never creates.
//!
//! The general form of the first-death defect: on
//! the pinned 1.21.11 server a scoreboard entry that was never written is not
//! zero, it is **false to every question**, so `execute if score @s dw.deaths >
//! @s dw.death_ack` did not fire while `dw.death_ack` had no entry and every
//! player's FIRST death silently did nothing. Feature-blind, read off the
//! finished tree: the rule is "a comparison has a writer", which needs no
//! knowledge of checkpoints, shops or stakes and therefore guards every future
//! emitter too.
//!
//! What these tests hold that the module's own unit tests cannot: that the rule
//! is an invariant of REAL emission rather than of hand-written fixtures, and —
//! the vacuity obligation (CLAUDE.md: *a green gate that binds to nothing is
//! vacuous, not a pass*) — that the check examines a stated, non-zero number of
//! real comparisons on each of them.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildFailure, BuildOutput};
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_compiler::seeding;
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};

fn parse_dir(dir: &std::path::Path) -> Campaign {
    let read = |n: &str| std::fs::read_to_string(dir.join(n)).unwrap();
    parse_campaign(&RawCampaign {
        world: read("world.json"),
        npcs: read("npcs.json"),
        classes: read("classes.json"),
        quest_plan: read("quest-plan.json"),
        quests: read("quests.json"),
        dialogue: read("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
    })
    .expect("campaign parses")
}

fn build(campaign: &Campaign, dir: &std::path::Path) -> Result<BuildOutput, BuildFailure> {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(campaign, &prefabs).expect("plan builds");
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
        &common::campaign_inputs(dir),
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
}

fn fixture(name: &str) -> std::path::PathBuf {
    common::compiler_fixtures_dir().join(name)
}

/// Every shipped fixture emits a tree whose score reads are all backed — and the
/// check states how many it examined on each, so a fixture that stopped emitting
/// comparisons at all could not read as a pass.
///
/// `emit::build` runs the check itself, so a build that returns `Ok` has already
/// passed it; this re-asserts it explicitly, names the fixture on failure, and
/// pins the binding.
#[test]
fn shipped_fixtures_emit_only_backed_score_reads() {
    // The floors are deliberately loose — they exist to catch a walker that stops
    // binding, not to freeze emission — but they are per fixture, so a campaign
    // whose comparisons vanished cannot hide behind the corpus total. **Entity
    // reads are floored separately** because they are the whole point: a walker
    // that lost `@s` while still counting thousands of `#` singletons would look
    // busier than ever and prove nothing about the defect this exists for.
    for (dir, min_reads, min_entity_reads) in [
        (common::hello_world_dir(), 20, 10),
        (common::keep_trial_dir(), 60, 10),
        (common::cutscene_shots_dir(), 50, 15),
        (fixture("economy"), 60, 30),
        (fixture("souls-bonfire"), 300, 25),
        (fixture("v06-checkpoints"), 300, 25),
        (fixture("lethal-volume"), 30, 15),
        // The die-retry fixture. A shipped fixture outside this sweep is a
        // hole in DW0495's binding, and this one emits the death edge the rule exists for.
        (fixture("die-retry"), 40, 15),
    ] {
        let campaign = parse_dir(&dir);
        let ns = campaign.world.campaign_id.as_str().to_string();
        let out = build(&campaign, &dir)
            .unwrap_or_else(|e| panic!("fixture `{}` must build: {e:?}", dir.display()));
        let census = seeding::census(&ns, &out);
        assert!(
            census.findings.is_empty(),
            "fixture `{}` reads an entry nothing creates:\n{}",
            dir.display(),
            census.findings.join("\n")
        );
        assert!(
            census.comparisons >= min_reads && census.entity_reads >= min_entity_reads,
            "fixture `{}` bound too little to mean anything: {census:?}",
            dir.display()
        );
    }
}

/// The two evidence forms that are not just "a write on the line above" bind on
/// real emission, not only on the module's hand-written cases.
///
/// Without this, the guard rule and the driver rule could quietly stop matching
/// anything and every fixture would still be green — the unbound-gate shape
/// CLAUDE.md names. `economy` is the one fixture that exercises both: a stake
/// ledger whose coordinates are only ever provable behind its own `kl matches 1`,
/// and a shop whose `shop_at` is only provable behind its `shop=1`, plus the
/// per-player `state_seed` hook the tick driver reaches unconditioned.
#[test]
fn the_guard_and_driver_rules_bind_on_real_emission() {
    let dir = fixture("economy");
    let campaign = parse_dir(&dir);
    let ns = campaign.world.campaign_id.as_str().to_string();
    let out = build(&campaign, &dir).expect("economy builds");
    let census = seeding::census(&ns, &out);
    assert!(
        census.admitted_by_guard >= 6,
        "the guard rule admitted almost nothing on a campaign built out of guards: {census:?}"
    );
    assert!(
        census.admitted_by_driver >= 2,
        "the driver rule admitted nothing on a campaign with per-player state: {census:?}"
    );
}

/// The instance, at the emitter: the death edge's three scores are created
/// **before** the edge compares them, and the order is what is asserted — a seed
/// after the comparison is the same bug with the lines swapped.
#[test]
fn the_death_edge_seeds_its_scores_before_it_compares_them() {
    let dir = fixture("economy");
    let campaign = parse_dir(&dir);
    let ns = campaign.world.campaign_id.as_str().to_string();
    let out = build(&campaign, &dir).expect("economy builds");
    let body = String::from_utf8(
        out[&format!("datapack/data/{ns}/function/cp_respawn_check.mcfunction")].clone(),
    )
    .unwrap();
    let at = |needle: &str| {
        body.find(needle)
            .unwrap_or_else(|| panic!("`{needle}` missing from:\n{body}"))
    };
    // The first comparison in the function is the earliest moment any of the three
    // can be read, so every seed must precede it.
    let first_comparison = at("if score @s dw.deaths");
    for score in ["dw.deaths", "dw.death_seen", "dw.death_ack"] {
        assert!(
            at(&format!("scoreboard players add @s {score} 0")) < first_comparison,
            "`{score}` is seeded after the edge that reads it:\n{body}"
        );
    }
}

/// Reintroducing the defect into the tree is refused, with the code CI keys off.
/// The drift direction is the real one: a comparison added without its seed.
#[test]
fn an_unseeded_comparison_is_refused_with_dw0495() {
    let out: BTreeMap<String, Vec<u8>> = [(
        "datapack/data/isle/function/cp_respawn_check.mcfunction".to_string(),
        b"execute unless data entity @s {Health:0.0f} if score @s dw.deaths > @s dw.death_ack \
          run function isle:cp_respawn_fire\n"
            .to_vec(),
    )]
    .into_iter()
    .collect();
    let e = seeding::check_tree("isle", &out).expect_err("an unseeded comparison must be refused");
    assert_eq!(e.code, "DW0495");
    assert_eq!(seeding::census("isle", &out).findings.len(), 2);
}

/// A campaign with neither `on_death` nor a checkpoint emits no seeding at all —
/// the check demands nothing where there is nothing to compare.
#[test]
fn a_campaign_without_a_death_edge_seeds_nothing() {
    let dir = common::hello_world_dir();
    let campaign = parse_dir(&dir);
    let ns = campaign.world.campaign_id.as_str().to_string();
    let out = build(&campaign, &dir).expect("hello-world builds");
    assert!(
        !out.contains_key(&format!(
            "datapack/data/{ns}/function/cp_respawn_check.mcfunction"
        )),
        "hello-world declares no death edge, so it must emit no check for one"
    );
}
