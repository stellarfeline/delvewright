//! **DSL v0.10 (spec-0031): the campaign's `on_death` beat, end to end.**
//!
//! `on_death` is effect root R7 — the campaign-wide bundle that runs at the
//! moment a player dies. `effect_root_walkers` proves every *walker* reaches it.
//! This file proves the two things that are specific to the death edge itself:
//!
//! 1. **There is exactly one death detector.** The v0.6 checkpoint machinery
//!    already detects death via the vanilla `deathCount` criterion (`dw.deaths`)
//!    and dispatches from one function, `cp_respawn_check`, off one `tick` line.
//!    `on_death` rides that edge. What it adds is a second *acknowledgement* of
//!    the same counter (`dw.death_seen`), because the existing one is
//!    deliberately withheld while the player is dead — which is precisely the
//!    window `on_death` fires in.
//! 2. **A campaign that declares no death beat is unchanged.** Not "nearly
//!    unchanged": the corpse-side branch, the `on_death_fire` function and the
//!    `dw.death_seen` objective are all absent, so pre-0.10 emission is
//!    byte-identical.
//!
//! **Contingent, and deliberately not asserted here.** Nothing in this file
//! records or reads the *position* a player died at. Which vanilla mechanism can
//! do that — the pre-respawn death advancement, or the `LastDeathLocation` player
//! NBT — is unverified for non-entity deaths (void, fall, drowning) and is being
//! measured on a live pinned 1.21.11 server. `emit::death_position_capture` is
//! the seam that measurement fills in; it emits nothing today, and a test that
//! asserted a mechanism would be asserting a guess.
//!
//! Likewise **the corpse-side positive is not provable from PackTest**: a
//! generated template drives a fake player, and a fake player is alive. What is
//! provable there, and is proven below at compile time, is the guard's shape —
//! an alive player never fires the beat, and the edge stays armed for the tick
//! the player is actually a corpse.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::load::{LoadedCampaign, load_campaign_dir};
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{Campaign, QuestEffect, parse_campaign, validate_campaign_with};

/// A campaign **with** checkpoints (so both sides of the edge are live).
const CP: &str = "v06-checkpoints";
/// A campaign with **no** checkpoint at all — the case that proves `on_death`
/// arms the detector on its own rather than borrowing a checkpoint's.
const NOCP: &str = "souls-shortcut";

const BEAT: &str = "The dark takes you, and something falls from your hand.";

fn on_death_bundle() -> Vec<QuestEffect> {
    serde_json::from_str(&format!(
        r#"[ {{ "type": "narrate", "style": "chat", "text": "{BEAT}" }} ]"#
    ))
    .expect("death beat parses")
}

fn load(ns: &str) -> LoadedCampaign {
    load_campaign_dir(&common::compiler_fixtures_dir().join(ns)).unwrap()
}

/// Parse the fixture, optionally binding a death beat on it.
fn campaign(loaded: &LoadedCampaign, with_on_death: bool) -> Campaign {
    let mut c = parse_campaign(&loaded.raw).expect("fixture parses");
    if with_on_death {
        c.quests.dsl_version = "0.10.0".to_string();
        c.quests.content.on_death = on_death_bundle();
    }
    c
}

fn build(loaded: &LoadedCampaign, c: &Campaign) -> BuildOutput {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let diags = common::fenced_diagnostics(
        c,
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    );
    assert!(diags.is_empty(), "fixture must validate clean: {diags:#?}");
    let plan = Plan::build(c, &prefabs).expect("plan builds");
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
                common::compiler_fixtures_dir()
                    .join(c.world.campaign_id.as_str())
                    .join("skins")
                    .join(format!("{}.png", skin.texture_id)),
            )
            .expect("skin png present");
            skins.insert(skin.texture_id.clone(), png);
        }
    }
    emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        "unpinned",
        &skins,
    )
    .expect("every emitted command validates")
}

fn body<'a>(out: &'a BuildOutput, ns: &str, name: &str) -> Option<&'a str> {
    out.get(&format!("datapack/data/{ns}/function/{name}.mcfunction"))
        .map(|b| std::str::from_utf8(b).unwrap())
}

// ---------------------------------------------------------------------------
// 1. the detector is shared, and the two edges are opposite sides of it
// ---------------------------------------------------------------------------

/// **The motivating scenario.** With a death beat declared, `cp_respawn_check`
/// dispatches `on_death_fire` on the CORPSE side of the same `dw.deaths` edge the
/// respawn machinery reads from the other side — and the two branches are
/// mutually exclusive by their own `Health:0.0f` guards, so neither can fire in
/// the other's window.
#[test]
fn the_death_beat_rides_the_existing_edge_on_the_corpse_side() {
    let loaded = load(CP);
    let out = build(&loaded, &campaign(&loaded, true));
    let check = body(&out, CP, "cp_respawn_check").expect("the detector exists");

    assert!(
        check.contains(
            "execute if data entity @s {Health:0.0f} if score @s dw.deaths > @s dw.death_seen \
             run function v06-checkpoints:on_death_fire"
        ),
        "the death beat fires while the player is still a corpse: {check}"
    );
    assert!(
        check.contains(
            "execute if data entity @s {Health:0.0f} run scoreboard players operation @s \
             dw.death_seen = @s dw.deaths"
        ),
        "…and acknowledges on the corpse side, so it fires ONCE per death rather \
         than every tick of the death screen: {check}"
    );
    // `dw.death_seen` is a `dummy` objective, so a player who has never died has
    // NO score in it — and
    // `execute if score @s A > @s B` with B unset does not fire (measured on the
    // pinned 1.21.11 server; `scoreboard players add <e> <obj> 0` is what creates
    // the entry). Without the seed the whole `on_death` bundle is dead on a
    // player's FIRST death — no forfeit, no recovery stake — and works from the
    // second onward, which is why compile-time shape proofs cannot see it. The
    // seed must PRECEDE the read, so the order is what is asserted.
    let seed = check
        .lines()
        .position(|l| l.trim() == "scoreboard players add @s dw.death_seen 0")
        .unwrap_or_else(|| panic!("the corpse-side acknowledgement is seeded: {check}"));
    let read = check
        .lines()
        .position(|l| l.contains("if score @s dw.deaths > @s dw.death_seen"))
        .expect("the corpse-side edge reads it");
    assert!(
        seed < read,
        "the acknowledgement must EXIST before the edge compares against it, or a \
         player's first death fires nothing at all: {check}"
    );
    // The v0.6 half is untouched and still waits for a living player.
    assert!(
        check.contains(
            "execute unless data entity @s {Health:0.0f} if score @s dw.deaths > @s dw.death_ack \
             run function v06-checkpoints:cp_respawn_fire"
        ),
        "the respawn half still holds for a player who has actually come back: {check}"
    );
    // Order: the moment of death, then the return. A reader meets the two edges
    // in the order the player lives them.
    let dies = check.find("on_death_fire").unwrap();
    let returns = check.find("cp_respawn_fire").unwrap();
    assert!(
        dies < returns,
        "death is dispatched before respawn: {check}"
    );
}

/// **One detector, not two.** The whole delve asks "has anyone died?" in exactly
/// one place: one `deathCount` objective, one `tick` line, one dispatcher.
#[test]
fn there_is_exactly_one_death_detector() {
    let loaded = load(CP);
    let out = build(&loaded, &campaign(&loaded, true));
    let all: String = out
        .iter()
        .filter(|(p, _)| p.starts_with("datapack/") && p.ends_with(".mcfunction"))
        .map(|(_, b)| std::str::from_utf8(b).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        all.matches("scoreboard objectives add dw.deaths deathCount")
            .count(),
        1,
        "one deathCount objective"
    );
    assert_eq!(
        all.matches("run function v06-checkpoints:cp_respawn_check")
            .count(),
        1,
        "one dispatcher call, on the tick"
    );
    let tick = body(&out, CP, "tick").expect("tick exists");
    assert!(
        tick.contains("execute as @a run function v06-checkpoints:cp_respawn_check"),
        "and it is the tick that runs it, per player: {tick}"
    );
}

/// The beat is the **dying player's**, not the party's — `Audience::Solo`, the
/// audience `on_respawn` and `on_caught` already use. Re-broadcasting one
/// player's death would re-narrate it to every survivor.
#[test]
fn the_beat_addresses_the_player_who_died() {
    let loaded = load(CP);
    let out = build(&loaded, &campaign(&loaded, true));
    let fire = body(&out, CP, "on_death_fire").expect("the beat has its own function");
    assert!(fire.contains(BEAT), "the authored line is lowered: {fire}");
    assert!(
        fire.contains("@s") && !fire.contains("@a"),
        "the beat addresses the dying player alone: {fire}"
    );
}

/// A campaign with **no checkpoint** arms the detector on its own: `on_death` is
/// not a checkpoint feature that happens to be reachable, it is a root of its
/// own. What it must NOT drag in is the respawn half — there is no checkpoint to
/// come back to, so no `#cp` marker, no re-seat and no `dw.death_ack`.
#[test]
fn a_death_beat_arms_the_detector_without_any_checkpoint() {
    let loaded = load(NOCP);
    let out = build(&loaded, &campaign(&loaded, true));
    let setup = body(&out, NOCP, "setup").expect("setup exists");
    assert!(
        setup.contains("scoreboard objectives add dw.deaths deathCount"),
        "the death counter is declared: {setup}"
    );
    assert!(
        setup.contains("scoreboard objectives add dw.death_seen dummy"),
        "so is the corpse-side ack: {setup}"
    );
    assert!(
        !setup.contains("dw.death_ack") && !setup.contains("#cp dw.sys"),
        "but nothing of the respawn half, which this campaign has no use for: {setup}"
    );
    let check = body(&out, NOCP, "cp_respawn_check").expect("the detector exists");
    assert!(
        check.contains("on_death_fire") && !check.contains("cp_respawn_fire"),
        "the dispatcher carries only the side this campaign uses: {check}"
    );
    assert!(
        body(&out, NOCP, "cp_seat_0").is_none(),
        "and no re-seat is generated for a campaign with nowhere to re-seat to"
    );
}

// ---------------------------------------------------------------------------
// 2. absence costs nothing
// ---------------------------------------------------------------------------

/// **A campaign that declares no death beat emits exactly what it did before the
/// root existed.** The control for every assertion above: without it, they would
/// all pass equally for an implementation that armed the corpse-side machinery
/// unconditionally and changed every existing campaign's datapack.
#[test]
fn a_campaign_without_a_death_beat_is_byte_identical() {
    let loaded = load(CP);
    let out = build(&loaded, &campaign(&loaded, false));

    let check = body(&out, CP, "cp_respawn_check").expect("the detector exists");
    // The v0.6 dispatcher, unchanged to the byte by the `on_death` surface — with
    // the two seeds `DW0495` requires and no corpse-side ack. The seeds belong to
    // the checkpoint edge, not to the death beat: a campaign that declares neither
    // still emits this function at all only because it has a checkpoint, and
    // `dw.death_seen` is the line that must stay absent here.
    assert_eq!(
        check,
        "scoreboard players add @s dw.deaths 0\n\
         scoreboard players add @s dw.death_ack 0\n\
         execute unless data entity @s {Health:0.0f} if score @s dw.deaths > @s dw.death_ack run \
         function v06-checkpoints:cp_respawn_fire\n\
         execute unless data entity @s {Health:0.0f} run scoreboard players operation @s \
         dw.death_ack = @s dw.deaths\n",
        "the v0.6 dispatcher, unchanged to the byte"
    );
    assert!(
        body(&out, CP, "on_death_fire").is_none(),
        "no death-beat function is generated"
    );
    let setup = body(&out, CP, "setup").expect("setup exists");
    assert!(
        !setup.contains("dw.death_seen"),
        "and no corpse-side ack is declared: {setup}"
    );
}

/// The same campaign at both ends: with the beat removed the whole shipped
/// datapack is identical to the pre-0.10 build, file for file and byte for byte.
/// Stated over the tree rather than over one function, because "byte-identical"
/// is a claim about the artifact.
#[test]
fn removing_the_beat_restores_the_whole_tree() {
    let loaded = load(CP);
    let plain = build(&loaded, &campaign(&loaded, false));
    // The same parse, the same plan, the same emitter — the ONLY difference is
    // the empty bundle, which must therefore be the only thing the output can
    // depend on.
    let mut c = campaign(&loaded, true);
    c.quests.content.on_death.clear();
    let cleared = build(&loaded, &c);
    let names: Vec<&String> = plain.keys().collect();
    assert!(!names.is_empty(), "the build produced files to compare");
    assert_eq!(
        plain.len(),
        cleared.len(),
        "the same file set is produced either way"
    );
    for (path, bytes) in &plain {
        assert_eq!(
            cleared.get(path),
            Some(bytes),
            "{path} differs between a pre-0.10 campaign and a 0.10 campaign with \
             an empty death beat"
        );
    }
}

// ---------------------------------------------------------------------------
// 3. the version fence
// ---------------------------------------------------------------------------

/// Declaring a death beat below 0.10.0 is `DW0141`, like every other newer
/// construct — the version contract stays exact, so an older campaign cannot
/// acquire the surface by accident.
#[test]
fn a_death_beat_below_0_10_is_reserved() {
    let loaded = load(CP);
    let mut c = parse_campaign(&loaded.raw).expect("fixture parses");
    c.quests.content.on_death = on_death_bundle();
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let codes: Vec<String> = validate_campaign_with(
        &c,
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    )
    .into_iter()
    .map(|d| d.code)
    .collect();
    assert!(
        codes.contains(&"DW0141".to_string()),
        "an `on_death` on a pre-0.10 stage is reserved: {codes:?}"
    );
}

/// …and the control: the identical campaign at 0.10.0 validates clean, so the
/// test above is measuring the fence and not a malformed bundle.
#[test]
fn the_control_the_same_beat_at_0_10_validates() {
    let loaded = load(CP);
    let c = campaign(&loaded, true);
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let diags = validate_campaign_with(
        &c,
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    );
    assert!(diags.is_empty(), "{diags:#?}");
}
