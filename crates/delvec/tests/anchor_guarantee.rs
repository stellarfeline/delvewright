//! `DW0889` — the anchors an area guarantees, answered before anything is placed.
//!
//! The rule these tests hold is not *the report fires*; it is **the report and
//! the solver agree about which pieces a draw seats**. So every case here is
//! written as a pair: what `compiler::guarantee` says about an area, and what
//! `Plan::build` actually places in it. A guarantee that could disagree with the
//! layout would be a worse answer than no answer at all, because a creator would
//! design against it.
//!
//! Fixtures use a **private** copy of the prefab library (`common::copy_dir_all`)
//! so the content repository is never written to, and the pools are synthesized
//! so what is being tested is a fact about the pool's shape rather than a lucky
//! seed.

mod common;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use delvec::compiler::guarantee::{self, DW_UNGUARANTEED_ANCHOR};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, Severity, parse_campaign};

/// A private library copy carrying the synthetic pools these tests need.
///
/// Three fixture pieces are synthesized from the shipped straight corridor's
/// geometry, each with anchors bolted on, because the library's own anchored
/// rooms are single-socket dead ends and cannot chain into a spine:
///
/// * `prefab/fx-entry` — the entry member. Its anchors are the unconditional
///   guarantee.
/// * `prefab/fx-filler` — a `connector`. Its anchors exist only if the filler
///   draw picks it.
/// * `prefab/fx-room` — a `room`. Its anchors exist only if the campaign
///   requires one of them.
fn fixture_prefabs(tag: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("anchor-guarantee-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    common::copy_dir_all(&common::prefabs_dir(), &dir);

    let corridor: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("keep-corridor-straight.json")).unwrap(),
    )
    .unwrap();
    let spawn: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("keep-spawn-hall.json")).unwrap())
            .unwrap();

    // The entry keeps the spawn hall's geometry AND its `spawn` anchor — a world
    // with no entry anchor is `DW0345` and would fail for a different reason.
    let mut entry = spawn.clone();
    entry["prefab_id"] = serde_json::json!("prefab/fx-entry");
    let mut entry_anchors = spawn["anchors"].clone();
    entry_anchors["anchor/lobby"] = serde_json::json!({ "pos": [3, 1, 3], "facing": "north" });
    entry["anchors"] = entry_anchors;
    std::fs::write(
        dir.join("fx-entry.json"),
        serde_json::to_string_pretty(&entry).unwrap(),
    )
    .unwrap();

    for (id, file, anchor) in [
        ("prefab/fx-filler", "fx-filler.json", "anchor/bay"),
        ("prefab/fx-room", "fx-room.json", "anchor/cell"),
    ] {
        let mut meta = corridor.clone();
        meta["prefab_id"] = serde_json::json!(id);
        meta["anchors"] = serde_json::json!({ anchor: { "pos": [2, 1, 2], "facing": "north" } });
        std::fs::write(dir.join(file), serde_json::to_string_pretty(&meta).unwrap()).unwrap();
    }

    let pools_path = dir.join("pools.json");
    let mut pools: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&pools_path).unwrap()).unwrap();
    pools["pools"]["pool/fx"] = serde_json::json!({
        "members": [
            { "prefab": "prefab/fx-entry",  "weight": 1, "role": "entry" },
            { "prefab": "prefab/fx-filler", "weight": 1, "role": "connector" },
            { "prefab": "prefab/fx-room",   "weight": 1, "role": "room" }
        ]
    });
    // The same three pieces with NO entry-role member: what a pool that cannot
    // seed a layout guarantees, which is nothing.
    pools["pools"]["pool/fx-headless"] = serde_json::json!({
        "members": [
            { "prefab": "prefab/fx-filler", "weight": 1, "role": "connector" },
            { "prefab": "prefab/fx-room",   "weight": 1, "role": "room" }
        ]
    });
    std::fs::write(&pools_path, serde_json::to_string_pretty(&pools).unwrap()).unwrap();
    dir
}

/// keep-crawl with its pool area rebound to `pool_id` at a fixed piece count, and
/// every reference into that area pointed at an anchor the fixture entry piece
/// provides. `patch` gets the parsed `quests.json` so each case declares the one
/// reference it is about and nothing else.
fn campaign(
    tag: &str,
    pool_id: &str,
    pieces: u32,
    patch: impl FnOnce(&mut serde_json::Value),
) -> PathBuf {
    let dst = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("anchor-guarantee-c-{tag}"));
    let _ = std::fs::remove_dir_all(&dst);

    let base = common::keep_crawl_dir();
    let mut world: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(base.join("world.json")).unwrap()).unwrap();
    world["content"]["areas"][1]["prefab_pool"] = serde_json::json!(pool_id);
    world["content"]["areas"][1]["pieces"] = serde_json::json!({ "min": pieces, "max": pieces });

    let mut quests: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(base.join("quests.json")).unwrap()).unwrap();
    // The stock campaign hangs an `open-gate` on `anchor/gate` and an objective on
    // `anchor/objective`; this synthetic pool carries neither, and both are
    // required uses, so they would decide the guarantee instead of the case.
    quests["content"]["quests"][1]["on_objective_complete"]["obj/arrive"] = serde_json::json!([]);
    quests["content"]["quests"][1]["objectives"][1]["anchor"] = serde_json::json!("anchor/lobby");

    patch(&mut quests);

    common::materialize_from(
        &base,
        &serde_json::json!({ "documents": { "world": world, "quests": quests } }),
        &dst,
    );
    dst
}

fn load(dir: &Path, prefabs_dir: &Path) -> (Campaign, PrefabRegistry) {
    let loaded = load_campaign_dir(dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("fixture campaign parses");
    let prefabs = PrefabRegistry::load_dir(prefabs_dir).unwrap();
    (campaign, prefabs)
}

/// What the solver actually seated in the pool area, so a claim about the
/// guarantee is checked against the layout rather than against itself.
fn placed(campaign: &Campaign, prefabs: &PrefabRegistry) -> BTreeSet<String> {
    let plan = Plan::build(campaign, prefabs).expect("fixture plans");
    plan.areas[1]
        .pieces
        .iter()
        .map(|p| p.prefab_id.clone())
        .collect()
}

// ---------------------------------------------------------------------------
// What a pool guarantees, from the library alone
// ---------------------------------------------------------------------------

/// The unconditional guarantee is the ENTRY member's anchors and nothing else —
/// and `delvec prefab anchors` reads this same answer with no campaign at all.
#[test]
fn a_pools_unconditional_guarantee_is_its_entry_members_anchors() {
    let prefabs_dir = fixture_prefabs("pool");
    let prefabs = PrefabRegistry::load_dir(&prefabs_dir).unwrap();
    let g = guarantee::pool_guarantee(&prefabs, "pool/fx").expect("the fixture pool is declared");

    assert_eq!(g.entry.as_deref(), Some("prefab/fx-entry"));
    assert!(g.unconditional.contains("anchor/lobby"));
    assert!(
        !g.unconditional.contains("anchor/bay") && !g.unconditional.contains("anchor/cell"),
        "a filler's and a room's anchors are not unconditional: {:?}",
        g.unconditional
    );
    assert!(
        g.vocabulary.contains("anchor/bay") && g.vocabulary.contains("anchor/cell"),
        "but they ARE in the pool's vocabulary: {:?}",
        g.vocabulary
    );
    assert!(
        g.unconditional.len() < g.vocabulary.len(),
        "the whole finding is that the guarantee is a strict subset"
    );
    // The report the creator reads states its own denominator.
    let head = &guarantee::report_lines(&g)[0];
    assert!(head.contains("3 member(s)"), "{head}");
    assert!(head.contains("prefab/fx-entry"), "{head}");
    assert!(
        head.contains(&format!(
            "guarantees {} of {} anchor name(s)",
            g.unconditional.len(),
            g.vocabulary.len()
        )),
        "{head}"
    );
}

/// A pool with no `entry`-role member guarantees NOTHING, and the report says so
/// rather than going quiet — the state the build meets as `DW0301`.
#[test]
fn a_pool_with_no_entry_member_guarantees_nothing() {
    let prefabs_dir = fixture_prefabs("headless");
    let prefabs = PrefabRegistry::load_dir(&prefabs_dir).unwrap();
    let g = guarantee::pool_guarantee(&prefabs, "pool/fx-headless").expect("declared");
    assert_eq!(g.entry, None);
    assert!(g.unconditional.is_empty());
    assert!(
        guarantee::report_lines(&g)[0].contains("DW0301"),
        "the report names what a draw of this pool meets: {:?}",
        guarantee::report_lines(&g)[0]
    );
}

// ---------------------------------------------------------------------------
// The degenerate cases the rule has to be right in
// ---------------------------------------------------------------------------

/// **`min == max == members` does NOT seat every member**, which is the belief
/// this test exists to refute. Fillers are drawn from the `connector` members
/// *with replacement*, so a three-piece budget over a three-member pool seats the
/// entry and two connector draws, and never the `room`.
#[test]
fn a_budget_equal_to_the_member_count_does_not_seat_every_member() {
    let prefabs_dir = fixture_prefabs("degenerate");
    let dir = campaign("degenerate", "pool/fx", 3, |_| {});
    let (c, prefabs) = load(&dir, &prefabs_dir);

    let seated = placed(&c, &prefabs);
    assert!(
        !seated.contains("prefab/fx-room"),
        "the room member is not seated by a full-count budget: {seated:?}"
    );

    let g = &guarantee::areas(&c, &prefabs, &guarantee::names_used(&c))[0];
    assert!(
        !g.guaranteed.contains("anchor/cell"),
        "and the guarantee agrees with the layout: {:?}",
        g.guaranteed
    );
}

/// An area that binds a single `prefab` guarantees everything that piece
/// declares, so it has no subset to report and contributes no pool area at all.
#[test]
fn a_single_prefab_area_has_no_guarantee_to_report() {
    let prefabs_dir = fixture_prefabs("single");
    let dir = common::keep_crawl_dir();
    let (c, prefabs) = load(&dir, &prefabs_dir);
    let (binding, diags) = guarantee::check(&c, &prefabs);
    assert!(
        binding.pool_areas < binding.areas,
        "keep-crawl's first area binds one piece: {}",
        binding.line()
    );
    assert!(
        diags.iter().all(|d| d.path != "/content/areas/area/hall"),
        "nothing is reported about a single-prefab area"
    );
}

// ---------------------------------------------------------------------------
// The report itself
// ---------------------------------------------------------------------------

/// **The report, and the perturbation only it could catch.** The primary binds
/// `anchor/bay` — a name only the `connector` filler declares — through an
/// environment trigger, which is a use `required_anchors_for_area` does not reach
/// in a multi-area campaign. `DW0889` names it, as an advisory, at the area. The
/// filler draw DOES seat that connector at this budget, so the build is green and
/// stays green: this is the report firing on a campaign nothing else objects to,
/// which is the whole reason it is a report and not a refusal.
#[test]
fn an_anchor_only_a_filler_declares_is_reported() {
    let prefabs_dir = fixture_prefabs("filler");
    let dir = campaign("filler", "pool/fx", 2, |quests| {
        quests["content"]["triggers"] = serde_json::json!([{
            "id": "trigger/fx-bay",
            "at": "anchor/bay",
            "on": { "on": "approach", "range": 3 },
            "effects": [{ "type": "narrate", "style": "chat", "text": "A bay." }]
        }]);
    });
    let (c, prefabs) = load(&dir, &prefabs_dir);

    let (binding, diags) = guarantee::check(&c, &prefabs);
    let hits: Vec<_> = diags.iter().filter(|d| d.code == "DW0889").collect();
    assert_eq!(hits.len(), 1, "one report for the one name: {diags:#?}");
    let d = hits[0];
    assert_eq!(d.code, DW_UNGUARANTEED_ANCHOR);
    assert_eq!(
        d.severity,
        Severity::Warning,
        "the filler draw may well seat it, and the build still refuses if it did not"
    );
    assert_eq!(d.stage, "world");
    assert_eq!(d.path, "/content/areas/area/keep");

    let m = &d.message;
    assert!(m.contains("anchor/bay"), "names the anchor: {m}");
    assert!(m.contains("pool/fx"), "names the pool: {m}");
    assert!(m.contains("prefab/fx-filler"), "names the carrier: {m}");
    assert!(m.contains("role `connector`"), "names the role: {m}");
    assert!(
        m.contains("3 member(s)") && m.contains("`pieces` 2..2"),
        "states its denominator: {m}"
    );
    assert!(
        m.contains("delvec prefab anchors --pool pool/fx"),
        "names the remedy that answers the question with no campaign: {m}"
    );
    assert_eq!(binding.outside, 1, "{}", binding.line());
    assert!(binding.pool_areas >= 1, "{}", binding.line());

    // And the campaign it fires on BUILDS, with the anchor really seated. This
    // is the claim the advisory tier rests on: if a report of this shape could
    // only ever appear on a campaign that fails anyway, it should have been a
    // refusal.
    let seated = placed(&c, &prefabs);
    assert!(
        seated.contains("prefab/fx-filler"),
        "this budget's filler draw does seat the carrier: {seated:?}"
    );
}

// ---------------------------------------------------------------------------
// The library-only door
// ---------------------------------------------------------------------------

/// `delvec prefab anchors` answers the same question with NO campaign, which is
/// the whole reason it exists: the anchors are chosen at an authoring step where
/// there is no campaign directory for a campaign verb to read.
#[test]
fn the_library_command_answers_with_no_campaign_at_all() {
    let prefabs_dir = fixture_prefabs("cli");
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_delvec"))
        .args(["--prefabs", prefabs_dir.to_str().unwrap()])
        .args(["prefab", "anchors", "--pool", "pool/fx", "--json"])
        .output()
        .expect("delvec runs");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let doc: serde_json::Value = serde_json::from_slice(&out.stdout).expect("one JSON object");
    let pool = &doc["pools"][0];
    assert_eq!(pool["pool"], "pool/fx");
    assert_eq!(pool["entry_member"], "prefab/fx-entry");
    let guaranteed: Vec<String> = serde_json::from_value(pool["guaranteed"].clone()).unwrap();
    assert!(
        guaranteed.contains(&"anchor/lobby".to_string()),
        "{guaranteed:?}"
    );
    assert!(
        !guaranteed.contains(&"anchor/bay".to_string()),
        "{guaranteed:?}"
    );
    let vocab: Vec<String> = serde_json::from_value(pool["vocabulary"].clone()).unwrap();
    assert!(vocab.contains(&"anchor/bay".to_string()), "{vocab:?}");
    // The binding, over the objects, on the machine path too.
    assert_eq!(doc["binding"]["pools_reported"], 1);
    assert!(doc["binding"]["members"].as_u64().unwrap() >= 3);

    // And a pool the library does not declare is refused with the set it does.
    let bad = std::process::Command::new(env!("CARGO_BIN_EXE_delvec"))
        .args(["--prefabs", prefabs_dir.to_str().unwrap()])
        .args(["prefab", "anchors", "--pool", "pool/not-a-pool"])
        .output()
        .expect("delvec runs");
    assert!(!bad.status.success());
    let err = String::from_utf8_lossy(&bad.stderr);
    assert!(
        err.contains("pool/fx"),
        "names what the library does declare: {err}"
    );
}

/// **The perturbation, over a fixture rebuilt from a scratch copy rather than
/// restored from git.** The same campaign, with the same anchor bound through a
/// REQUIRED use instead — a `reach-anchor` objective rather than a trigger — so
/// the solver must seat its carrier and the report goes silent. One field of one
/// document differs between this case and the one above, and it is the field the
/// rule is about: a narrower or a wider check would not tell these two documents
/// apart.
#[test]
fn requiring_the_same_anchor_promotes_it_into_the_guarantee_and_the_report_goes_quiet() {
    let prefabs_dir = fixture_prefabs("promote");
    let dir = campaign("promote", "pool/fx", 2, |quests| {
        quests["content"]["quests"][1]["objectives"][1]["anchor"] = serde_json::json!("anchor/bay");
    });
    let (c, prefabs) = load(&dir, &prefabs_dir);

    let g = &guarantee::areas(&c, &prefabs, &guarantee::names_used(&c))[0];
    assert!(
        g.forced.contains(&"prefab/fx-filler".to_string()),
        "the required NPC stand forces its carrier: {:?}",
        g.forced
    );
    assert!(g.guaranteed.contains("anchor/bay"));
    assert!(
        g.outside.is_empty(),
        "nothing left to report: {:?}",
        g.outside
    );

    // And the layout does what the guarantee claims.
    assert!(placed(&c, &prefabs).contains("prefab/fx-filler"));

    let (_, diags) = guarantee::check(&c, &prefabs);
    assert!(diags.iter().all(|d| d.code != "DW0889"), "{diags:#?}");
}

/// A `room` member's anchor, bound through the same non-required use, is
/// reported with the OTHER half of the message — the layout seats it only on
/// demand, which is a different fact from a filler draw and is said differently.
#[test]
fn a_room_members_anchor_is_reported_as_seated_only_on_demand() {
    let prefabs_dir = fixture_prefabs("room");
    let dir = campaign("room", "pool/fx", 3, |quests| {
        quests["content"]["triggers"] = serde_json::json!([{
            "id": "trigger/fx-cell",
            "at": "anchor/cell",
            "on": { "on": "approach", "range": 3 },
            "effects": [{ "type": "narrate", "style": "chat", "text": "A cell." }]
        }]);
    });
    let (c, prefabs) = load(&dir, &prefabs_dir);
    let (_, diags) = guarantee::check(&c, &prefabs);
    let d = diags
        .iter()
        .find(|d| d.code == "DW0889")
        .expect("the room's anchor is outside the guarantee");
    assert!(
        d.message
            .contains("only when this campaign requires an anchor that piece carries"),
        "the on-demand half, not the filler half: {}",
        d.message
    );
    assert!(d.message.contains("role `room`"), "{}", d.message);
}

/// A name the pool declares nowhere is NOT this code's business: `DW0302` owns
/// it at the use site, and two codes for one fact is the defect.
#[test]
fn a_name_no_member_declares_is_not_reported_here() {
    let prefabs_dir = fixture_prefabs("invented");
    let dir = campaign("invented", "pool/fx", 3, |quests| {
        quests["content"]["triggers"] = serde_json::json!([{
            "id": "trigger/fx-nowhere",
            "at": "anchor/no-such-place",
            "on": { "on": "approach", "range": 3 },
            "effects": [{ "type": "narrate", "style": "chat", "text": "Nowhere." }]
        }]);
    });
    let (c, prefabs) = load(&dir, &prefabs_dir);
    let (_, diags) = guarantee::check(&c, &prefabs);
    assert!(
        diags
            .iter()
            .all(|d| !d.message.contains("anchor/no-such-place")),
        "{diags:#?}"
    );
}

/// The binding line is printed on a campaign this reports nothing about, with
/// its denominator — a check that says nothing must still say what over.
#[test]
fn the_binding_line_states_its_denominator_on_a_quiet_run() {
    let prefabs_dir = fixture_prefabs("quiet");
    let dir = campaign("quiet", "pool/fx", 3, |_| {});
    let (c, prefabs) = load(&dir, &prefabs_dir);
    let (binding, diags) = guarantee::check(&c, &prefabs);
    assert!(diags.is_empty(), "{diags:#?}");
    let line = binding.line();
    assert!(line.contains("1 of 2 area(s) bind a pool"), "{line}");
    assert!(line.contains("3 member(s)"), "{line}");
    assert!(line.contains("0 reported outside a guarantee"), "{line}");
}
