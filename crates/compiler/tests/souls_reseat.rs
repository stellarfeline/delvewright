//! spec-0016 §1 (owner ruling 2026-08-05) — **the undefeated re-seat**.
//!
//! The regression this file exists for, from the drowned bell's round-five
//! playtest: a rest re-seated the `respawns_on_rest` waves around the party and
//! left every *actor* exactly as combat had left it. The barrow-warden — an
//! elite the party had woken, wounded and run away from — stayed where the chase
//! ended, at the health the chase left it; so did the ambushers staged in the
//! sewer and up in the rafters. The cause was structural: `bonfire_reseat_lines`
//! iterated `plan.reseat_waves()` and nothing else, so an actor was not
//! registered in any rest hook at all, at any position in the emission.
//!
//! The ruling: an UNDEFEATED elite — an actor the campaign unleashes, or a wave
//! billed `elite`/`boss` — is DELETED and re-summoned FRESH at its origin, at
//! full health, on a bonfire rest and on a death-respawn at that fire. A
//! defeated one stays defeated (spec-0016 §1: stage bosses never respawn on
//! rest). Ordinary `respawns_on_rest` waves keep the stationed semantics they
//! already had.
//!
//! Driven by the `souls-bonfire` fixture (the only fixture with a real bonfire),
//! with the actor surface and the wave tier declared in-test — one build per
//! shape, so each claim is read off a whole real emission.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{Campaign, parse_campaign, validate_campaign_with};

const NS: &str = "souls-bonfire";

/// The elite the party wakes and runs away from — the fixture's barrow-warden.
const ELITE: &str = "actor/barrow-warden";
/// Its `safe_local` form, as every emitted function/tag spells it.
const ELITE_SAFE: &str = "barrow_warden";
/// A staged puppet that is never unleashed: scenery, and never re-seated.
const SCENERY: &str = "actor/kneeling-effigy";
const SCENERY_SAFE: &str = "kneeling_effigy";

fn fixture_dir() -> std::path::PathBuf {
    common::compiler_fixtures_dir().join(NS)
}

/// The fixture campaign, raised to v0.7 quests (wave `tier` is a v0.7 field) and
/// given the surface this file is about:
///
/// * `actor/barrow-warden`, staged and **unleashed** from the fixture's own
///   strike trigger — the shape of every actor elite the bell declares;
/// * `actor/kneeling-effigy`, staged and never unleashed — scenery, the control;
/// * `wave/ambush` billed `boss` (it declares no `respawns_on_rest`, so it is the
///   undefeated-refresh case), `wave/guards` left ordinary and re-seating.
///
/// `unleash` decides which actors take part, exactly as the floor-gate ledger
/// decides which actors are fights (`combat::hostile_actors`).
fn fixture_campaign(with_unleash: bool) -> Campaign {
    let loaded = load_campaign_dir(&fixture_dir()).unwrap();
    let mut c = parse_campaign(&loaded.raw).expect("souls-bonfire parses");
    c.quests.dsl_version = "0.7.0".to_string();
    for a in [ELITE, SCENERY] {
        c.quests.content.actors.push(
            serde_json::from_value(serde_json::json!({
                "id": a,
                "entity": "minecraft:wither_skeleton",
                "name": "The Barrow Warden",
                "anchor": "anchor/wave",
                "facing": "north"
            }))
            .expect("actor parses"),
        );
    }
    let trigger = c
        .quests
        .content
        .triggers
        .iter_mut()
        .find(|t| t.id.as_str() == "trigger/gate-ward")
        .expect("the fixture's strike trigger");
    for a in [ELITE, SCENERY] {
        trigger.effects.push(
            serde_json::from_value(serde_json::json!({ "type": "spawn-actor", "actor": a }))
                .expect("spawn-actor parses"),
        );
    }
    if with_unleash {
        trigger.effects.push(
            serde_json::from_value(serde_json::json!({ "type": "unleash-actor", "actor": ELITE }))
                .expect("unleash-actor parses"),
        );
    }
    for w in &mut c.quests.content.waves {
        if w.id.as_str() == "wave/ambush" {
            w.tier = Some(delvewright_dsl::EncounterTier::Boss);
        }
    }
    c
}

/// Validate + plan + emit. `emit::build` validates every emitted command against
/// the pinned 1.21.11 command tree, so a clean build is itself the proof that the
/// new re-seat lines are commands the server will accept.
fn build(campaign: &Campaign) -> BuildOutput {
    let dir = fixture_dir();
    let loaded = load_campaign_dir(&dir).unwrap();
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let items = FullItemRegistry::v1_21_11();
    let entities = FullEntityRegistry::v1_21_11();
    let diags = validate_campaign_with(campaign, &items, &prefabs, &entities);
    assert!(
        diags.is_empty(),
        "the fixture must validate clean: {diags:#?}"
    );

    let plan = Plan::build(campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let bytes = std::fs::read(common::prefabs_dir().join(&piece.structure_file)).unwrap();
            structures.insert(piece.structure_file.clone(), bytes);
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

fn text(out: &BuildOutput, path: &str) -> String {
    String::from_utf8(
        out.get(path)
            .unwrap_or_else(|| {
                panic!(
                    "{path} emitted; have:\n{:#?}",
                    out.keys().collect::<Vec<_>>()
                )
            })
            .clone(),
    )
    .unwrap()
}

fn func(out: &BuildOutput, name: &str) -> String {
    text(
        out,
        &format!("datapack/data/{NS}/function/{name}.mcfunction"),
    )
}

fn packtest(out: &BuildOutput, name: &str) -> String {
    text(
        out,
        &format!("packtest-datapack/data/{NS}/test/{name}.mcfunction"),
    )
}

fn all_functions(out: &BuildOutput) -> String {
    let mut s = String::new();
    for (path, bytes) in out {
        if path.starts_with("datapack/") && path.ends_with(".mcfunction") {
            s.push_str(std::str::from_utf8(bytes).unwrap());
            s.push('\n');
        }
    }
    s
}

// ---------------------------------------------------------------------------
// 1. the actor elite — the barrow-warden regression
// ---------------------------------------------------------------------------

/// **Both rest paths dispatch the elite.** A bonfire owes the same scene reset to
/// a party that rested and to a party that died and woke there, so the re-seat
/// line has to appear in `bonfire_rest_<i>` AND in `cp_on_respawn_<i>` — this is
/// the hook the actor was missing from entirely.
#[test]
fn rest_and_death_respawn_both_reseat_the_undefeated_elite() {
    let out = build(&fixture_campaign(true));
    let want = format!(
        "execute unless entity @e[tag=dw_pup_{ELITE_SAFE}] if entity \
         @e[tag=dw_actor_{ELITE_SAFE}] run function {NS}:actor_restand_{ELITE_SAFE}"
    );
    for f in ["bonfire_rest_0", "cp_on_respawn_0"] {
        let body = func(&out, f);
        assert!(
            body.contains(&want),
            "{f} must re-seat the undefeated elite (spec-0016 §1):\n{body}"
        );
    }
}

/// **The re-seat is a delete and a fresh summon at the ORIGIN** — not a teleport,
/// not a heal. The owner's report was that the warden stayed where combat left
/// it; the fix is that the body the party wounded ceases to exist and a new one
/// stands on the actor's own anchor cell.
#[test]
fn the_elite_is_deleted_and_resummoned_at_its_origin_anchor() {
    let out = build(&fixture_campaign(true));
    let restand = func(&out, &format!("actor_restand_{ELITE_SAFE}"));
    let mut ls = restand.lines().filter(|l| !l.trim().is_empty());
    assert_eq!(
        ls.next().unwrap(),
        format!("kill @e[tag=dw_actor_{ELITE_SAFE}]"),
        "the wounded body is REMOVED first — never topped up:\n{restand}"
    );
    let summon = ls.next().expect("a summon follows the kill");
    assert!(
        summon.starts_with("summon minecraft:wither_skeleton "),
        "a fresh body of the actor's own species is summoned:\n{restand}"
    );
    assert!(
        !summon.contains('~'),
        "the fresh body stands at ABSOLUTE origin coordinates — there is no puppet \
         left to stand relative to:\n{restand}"
    );
    assert_eq!(ls.next(), None, "and nothing else happens:\n{restand}");

    // The origin really is the actor's declared anchor: the puppet's own summon
    // (`spawn_actor_<id>`, which is placed at that anchor) uses the same cell.
    let spawn = func(&out, &format!("spawn_actor_{ELITE_SAFE}"));
    let cell = |line: &str| -> String {
        line.split_whitespace()
            .skip_while(|t| !t.starts_with("minecraft:"))
            .skip(1)
            .take(3)
            .collect::<Vec<_>>()
            .join(" ")
    };
    assert_eq!(
        cell(summon),
        cell(&spawn),
        "the re-seated elite stands exactly where the campaign staged it:\n{spawn}\n{restand}"
    );
}

/// **It comes back FREED, never re-caged.** An `unleash-actor` beat fires from a
/// one-shot trigger the engine never re-arms, so a re-seat that put the puppet
/// back would leave the elite dormant `Invulnerable` scenery for the rest of the
/// delve — a worse bug than the one being fixed. The re-seated body is the same
/// twin `unleash_<id>` summons, minus the relative position.
#[test]
fn the_reseated_elite_is_the_freed_twin_not_the_cage() {
    let out = build(&fixture_campaign(true));
    let restand = func(&out, &format!("actor_restand_{ELITE_SAFE}"));
    let unleash = func(&out, &format!("unleash_{ELITE_SAFE}"));
    assert!(
        !restand.contains(&format!("spawn_actor_{ELITE_SAFE}")),
        "the re-seat never re-runs the caging spawn:\n{restand}"
    );
    assert!(
        !restand.contains(&format!("dw_pup_{ELITE_SAFE}")),
        "and stamps no puppet marker on the fresh body:\n{restand}"
    );
    let tail = |s: &str| s[s.find('{').expect("summon NBT")..].to_string();
    let twin = unleash
        .lines()
        .find(|l| l.contains("summon "))
        .expect("the unleash summons a twin");
    assert_eq!(
        tail(restand.lines().find(|l| l.starts_with("summon ")).unwrap()),
        tail(twin),
        "the re-seated body is byte-for-byte the body an unleash produces:\n{unleash}\n{restand}"
    );
}

/// **Scenery is never re-seated.** An actor the campaign only ever stages is
/// `NoAI`, knockback-immune and (undeclared `vulnerable`) `Invulnerable`: combat
/// cannot damage it or move it, and re-seating it could only undo authored
/// `move-actor` staging. The predicate is the campaign's own "unleash or
/// nothing" rule, so this actor gets no function and no dispatch line.
#[test]
fn a_staged_but_never_unleashed_actor_is_not_reseated() {
    let out = build(&fixture_campaign(true));
    let all = all_functions(&out);
    assert!(
        !all.contains(&format!("actor_restand_{SCENERY_SAFE}")),
        "a never-unleashed actor has no re-seat anywhere in the datapack"
    );
    assert!(
        all.contains(&format!("spawn_actor_{SCENERY_SAFE}")),
        "…while still being staged normally (the control is a real actor)"
    );
}

/// A campaign whose actors are ALL scenery emits no undefeated actor re-seat at
/// all — the byte-identity claim for every delve that does not use the surface.
#[test]
fn a_campaign_with_no_hostile_actor_emits_no_actor_reseat() {
    let out = build(&fixture_campaign(false));
    let all = all_functions(&out);
    assert!(
        !all.contains("actor_restand_"),
        "no unleashed actor, no actor re-seat emission"
    );
}

// ---------------------------------------------------------------------------
// 2. the billed wave — the anti-chip half
// ---------------------------------------------------------------------------

/// **A billed boss wave is refreshed while it stands, and only while it stands.**
/// The gate is the wave's own body tag, so "undefeated" is asked of the world
/// rather than of a sentinel: a boss the party killed leaves nothing to select
/// and stays dead (spec-0016 §1), one they chipped is wiped and re-seated whole.
#[test]
fn an_undefeated_boss_wave_is_reseated_on_its_own_bodies() {
    let out = build(&fixture_campaign(true));
    let want =
        format!("execute if entity @e[tag=dw_wave_ambush] run function {NS}:wave_reseat_ambush");
    for f in ["bonfire_rest_0", "cp_on_respawn_0"] {
        let body = func(&out, f);
        assert!(
            body.contains(&want),
            "{f} must refresh the undefeated boss wave:\n{body}"
        );
    }
    let reseat = func(&out, "wave_reseat_ambush");
    assert_eq!(
        reseat
            .lines()
            .filter(|l| !l.trim().is_empty())
            .collect::<Vec<_>>(),
        vec![
            "kill @e[tag=dw_wave_ambush]".to_string(),
            format!("function {NS}:spawn_ambush"),
        ],
        "the refresh is the authored wave, re-seated whole:\n{reseat}"
    );
}

/// **The ordinary `respawns_on_rest` wave is untouched by all of this.** Its
/// gate is still the seated sentinel — it comes back whether the party beat it
/// or fled it — and it must not acquire the undefeated wave's body test, which
/// would silently retire it once cleared.
#[test]
fn an_ordinary_reseat_wave_keeps_its_seated_sentinel() {
    let out = build(&fixture_campaign(true));
    let body = func(&out, "bonfire_rest_0");
    assert!(
        body.contains(&format!(
            "execute if score #wseat_guards dw.sys matches 1 run function {NS}:wave_reseat_guards"
        )),
        "the respawns_on_rest wave keeps the met-sentinel gate:\n{body}"
    );
    assert!(
        !body.contains("if entity @e[tag=dw_wave_guards]"),
        "…and never gains the undefeated body gate:\n{body}"
    );
}

/// An untiered, non-`respawns_on_rest` wave is not re-seated by anything: the
/// undefeated refresh is for encounters the content BILLS, not for every mob in
/// the delve.
#[test]
fn an_untiered_wave_is_not_refreshed_by_a_rest() {
    let mut c = fixture_campaign(true);
    for w in &mut c.quests.content.waves {
        if w.id.as_str() == "wave/ambush" {
            w.tier = None;
        }
    }
    let out = build(&c);
    let all = all_functions(&out);
    assert!(
        !all.contains("wave_reseat_ambush"),
        "an unbilled wave gets no re-seat function and no dispatch"
    );
}

// ---------------------------------------------------------------------------
// 3. the runtime claim
// ---------------------------------------------------------------------------

/// The emission's own PackTests: the claims above are about what a live 1.21.11
/// server does with these functions, so the compiler ships templates that drive
/// the REAL rest function and read the world back.
#[test]
fn the_undefeated_reseat_ships_its_packtests() {
    let out = build(&fixture_campaign(true));

    let t = packtest(&out, "souls_reseat_actor");
    assert!(t.contains(&format!("function {NS}:unleash_{ELITE_SAFE}")));
    assert!(
        t.contains(&format!(
            "data modify entity @e[tag=dw_actor_{ELITE_SAFE},limit=1] Health set value 1.0f"
        )),
        "the template chips the elite before resting — the anti-grind claim:\n{t}"
    );
    assert!(
        t.contains(&format!(
            "tag @e[tag=dw_actor_{ELITE_SAFE}] add dw_rsua_brand"
        )),
        "…and brands the body, so a survivor cannot hide inside a correct count:\n{t}"
    );
    assert!(
        t.contains(&format!("function {NS}:bonfire_rest_0")),
        "the drive is the REAL rest function, never a hand-rolled imitation:\n{t}"
    );
    assert!(
        t.contains("assert score #b_rsua dw.sys matches 0")
            && t.contains("assert score #a_rsua dw.sys matches 1")
            && t.contains("assert score #d_rsua dw.sys matches 1")
            && t.contains("assert score #c_rsua dw.sys matches 0")
            && t.contains("assert score #k_rsua dw.sys matches 0"),
        "one body, fresh, on its origin, freed — and nothing after it is killed:\n{t}"
    );

    let t = packtest(&out, "souls_reseat_undefeated");
    assert!(t.contains(&format!("function {NS}:spawn_ambush")));
    assert!(
        t.contains("assert score #n_rsuw dw.sys matches 2")
            && t.contains("assert score #b_rsuw dw.sys matches 0")
            && t.contains("assert score #k_rsuw dw.sys matches 0"),
        "the chipped boss wave comes back whole and unbranded; the beaten one stays beaten:\n{t}"
    );
}
