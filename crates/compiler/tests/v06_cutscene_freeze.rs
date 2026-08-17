//! The cutscene staging invariant: **a cutscene is pure observation.**
//!
//! While a player is in the cutscene state (`dw_cutscene`, held for exactly the
//! cinematic's lifetime) campaign machinery must neither require anything of
//! them nor punish them: the stealth judge is suspended for that player (grace
//! neither accrues nor expires, `on_caught` cannot fire) and `damage-players`
//! skips them. Driven by the `v06-checkpoints` fixture, which carries both a
//! stealth beat and a cutscene.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::parse_campaign;

const NS: &str = "v06-checkpoints";

fn build_fixture() -> BuildOutput {
    let dir = common::compiler_fixtures_dir().join(NS);
    let loaded = load_campaign_dir(&dir).unwrap();
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
    let mut skins: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for npc in &campaign.npcs.content.npcs {
        if let Some(skin) = &npc.skin {
            let png = std::fs::read(dir.join("skins").join(format!("{}.png", skin.texture_id)))
                .expect("skin png present");
            skins.insert(skin.texture_id.clone(), png);
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
        &skins,
    )
    .expect("every emitted command validates")
}

/// The body of the one generated function whose path contains `needle`.
fn one_fn(out: &BuildOutput, needle: &str) -> String {
    let key = out
        .keys()
        .find(|k| k.starts_with("datapack/") && k.contains(needle))
        .unwrap_or_else(|| panic!("no function matching {needle}"))
        .clone();
    String::from_utf8(out[&key].clone()).unwrap()
}

/// The cutscene `start` function (`cs_<bare>`, not `cs_tick_`/`cs_end_`).
fn cutscene_start(out: &BuildOutput) -> String {
    let key = out
        .keys()
        .find(|k| {
            k.starts_with("datapack/")
                && k.contains(&format!("data/{NS}/function/cs_"))
                && !k.contains("function/cs_tick_")
                && !k.contains("function/cs_end_")
        })
        .expect("cutscene start emitted")
        .clone();
    String::from_utf8(out[&key].clone()).unwrap()
}

/// The bracket: `start` arms the cutscene state alongside `gamemode spectator`,
/// `end` releases it alongside the gamemode restore. Exactly the cinematic's
/// lifetime — no wider, no narrower.
#[test]
fn cutscene_bracket_arms_and_releases_the_state_tag() {
    let out = build_fixture();
    let start = cutscene_start(&out);
    assert!(
        start.contains("tag @a add dw_cutscene"),
        "cutscene start must arm the state:\n{start}"
    );
    let spectator = start.find("gamemode spectator @a").expect("spectator save");
    let armed = start.find("tag @a add dw_cutscene").unwrap();
    assert!(armed < spectator, "state is armed before players are moved");

    let end = one_fn(&out, &format!("data/{NS}/function/cs_end_"));
    assert!(
        end.contains("tag @a remove dw_cutscene"),
        "cutscene end must release the state:\n{end}"
    );
    assert!(
        end.contains("gamemode adventure @a"),
        "cutscene end restores the gamemode:\n{end}"
    );
}

/// The stealth judge never runs for a player in the cutscene state. The judge is
/// the only writer of `dw.st_grace`, so skipping it is what freezes the clock.
#[test]
fn stealth_judge_skips_players_in_a_cutscene() {
    let out = build_fixture();
    let tick = one_fn(&out, &format!("data/{NS}/function/stealth_tick_"));
    assert!(
        tick.contains("as @a[tag=!dw_cutscene] run function"),
        "the stealth tick must skip cutscene viewers:\n{tick}"
    );
    assert!(
        !tick.contains("as @a run function"),
        "no unguarded judge pass may survive:\n{tick}"
    );
}

/// The restore leaves the stealth state entirely alone. Under the zone-presence
/// model (no sneak stat, so no ack to re-sync) the
/// judge reads only the player's position; `dw.st_grace` is frozen, not reset,
/// so the beat resumes exactly where it paused.
#[test]
fn cutscene_end_touches_no_stealth_state() {
    let out = build_fixture();
    let end = one_fn(&out, &format!("data/{NS}/function/cs_end_"));
    assert!(
        !end.contains("dw.st_"),
        "restore must NOT touch any stealth score — the judge is position-only \
         and grace is frozen, not reset:\n{end}"
    );
}

/// The generated PackTest proves the freeze at runtime: an exposed player accrues
/// no grace while the state is on, and resumes accruing the moment it comes off.
#[test]
fn cutscene_freeze_packtest_asserts_both_halves() {
    let out = build_fixture();
    let path = format!("packtest-datapack/data/{NS}/test/v06_cutscene_freeze.mcfunction");
    let body = std::str::from_utf8(out.get(&path).expect("freeze packtest emitted")).unwrap();
    const SEL: &str = "@a[tag=dw_t_cfrz,limit=1]";
    assert!(
        body.contains("tag @p add dw_t_cfrz"),
        "packtest pins its own dummy (batch model):\n{body}"
    );
    assert!(
        body.contains(&format!("tag {SEL} add dw_cutscene"))
            && body.contains(&format!("tag {SEL} remove dw_cutscene")),
        "packtest must drive both halves of the state, on its pinned dummy only:\n{body}"
    );
    assert!(
        body.contains(&format!("function {NS}:stealth_tick_")),
        "packtest must drive the real gate, not the judge directly:\n{body}"
    );
    // Frozen half asserts grace stayed 0; resumed half asserts it climbed again.
    let (frozen, resumed) = body
        .split_once(&format!("tag {SEL} remove dw_cutscene"))
        .expect("both halves present");
    assert!(
        frozen.contains(&format!("assert score {SEL} dw.st_grace matches 0")),
        "frozen half must assert no accrual:\n{frozen}"
    );
    assert!(
        resumed.lines().any(|l| {
            l.starts_with(&format!("assert score {SEL} dw.st_grace matches")) && !l.ends_with(" 0")
        }),
        "resumed half must assert the clock restarted:\n{resumed}"
    );
    // The pin is the only `@p`, and no bare `@a` write survives — after the tp to
    // campaign coordinates `@p` would resolve to a neighbor test's dummy, and an
    // `@a` write (state, tp, or the cutscene tag itself) would hit every dummy.
    assert_eq!(
        body.matches("@p").count(),
        1,
        "the pin is the only `@p` in the freeze test:\n{body}"
    );
    assert!(
        !body.contains("@a "),
        "no bare `@a` writes in the freeze test:\n{body}"
    );
}
