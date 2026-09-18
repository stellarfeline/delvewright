//! spec-0016 §1 — **the undefeated re-seat**.
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

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{Campaign, DSL_VERSION, parse_campaign, validate_campaign_with};

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
    c.quests.dsl_version = DSL_VERSION.to_string();
    // A mark apiece. Both bodies are staged and neither is ever removed, so one
    // cell for the two of them is two live bodies on one mark (`DW0896`) and the
    // fixture would not build — which is the rule, not a fixture inconvenience.
    for (a, anchor) in [(ELITE, "anchor/wave"), (SCENERY, "anchor/npc-stand")] {
        c.quests.content.actors.push(
            serde_json::from_value(serde_json::json!({
                "id": a,
                "entity": "minecraft:wither_skeleton",
                "name": "The Barrow Warden",
                "anchor": anchor,
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

/// spec-0073: the one advisory a `boss`-billed fight with no `health_bar` owes
/// (`DW0912`, warning tier). The fixture's diagnostics must be EXACTLY those —
/// one per such fight, counted from the campaign itself — and nothing else.
fn assert_only_boss_advisories(c: &Campaign, diags: &[delvewright_dsl::Diagnostic]) {
    let owed = delvewright_dsl::fights(c)
        .iter()
        .filter(|f| f.tier == Some(delvewright_dsl::EncounterTier::Boss) && f.bar.is_none())
        .count();
    assert!(
        diags
            .iter()
            .all(|d| d.code == "DW0912" && d.severity == delvewright_dsl::Severity::Warning),
        "the fixture must validate clean but for the boss advisory: {diags:#?}"
    );
    assert_eq!(diags.len(), owed, "{diags:#?}");
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
    assert_only_boss_advisories(campaign, &diags);

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

/// The unseen removal of every body carrying `tag` — the lines
/// `emit::removal_lines` writes for a re-seat, spelled out.
fn unseen_removal(tag: &str) -> Vec<String> {
    vec![
        format!(
            "execute if entity @e[tag={tag}] run schedule function {NS}:unseen_sweep 5t replace"
        ),
        format!("execute as @e[tag={tag}] on passengers run ride @s dismount"),
        format!("execute as @e[tag={tag}] at @s run tp @s ~ -128 ~"),
        format!(
            "execute as @e[tag={tag}] run data merge entity @s {{Tags:[\"dw_unseen\"],NoGravity:1b,NoAI:1b,Silent:1b}}"
        ),
    ]
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
    for want in unseen_removal(&format!("dw_actor_{ELITE_SAFE}")) {
        assert_eq!(
            ls.next().unwrap(),
            want,
            "the wounded body is REMOVED first, unseen — never topped up:\n{restand}"
        );
    }
    let summon = ls.next().expect("a summon follows the removal");
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
    let mut want = unseen_removal("dw_wave_ambush");
    want.push(format!("function {NS}:spawn_ambush"));
    assert_eq!(
        reseat
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>(),
        want,
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

// ---------------------------------------------------------------------------
// 4. a removal the compiler performs yields nothing
// ---------------------------------------------------------------------------

/// The fixture with a declared drop on every body a rest or a beat can remove:
/// the elite actor (puppet and twin), the scenery actor (removed by a
/// `despawn-actor`, `vanish` style — the style whose removal is two commands),
/// the billed boss wave and the ordinary `respawns_on_rest` wave.
fn fixture_campaign_with_drops() -> Campaign {
    let mut c = fixture_campaign(true);
    let drop = |name: &str| {
        serde_json::from_value::<delvewright_dsl::MobDrop>(serde_json::json!({
            "item": "minecraft:tripwire_hook",
            "name": name
        }))
        .expect("drop parses")
    };
    // Only a billed fight leaves anything behind (`DW0491`).
    for a in &mut c.quests.content.actors {
        a.tier = Some(delvewright_dsl::EncounterTier::Elite);
        a.drops.push(drop("Warden Key"));
    }
    for w in &mut c.quests.content.waves {
        w.tier.get_or_insert(delvewright_dsl::EncounterTier::Elite);
        for m in &mut w.mobs {
            m.drops.push(drop("Guard Key"));
        }
    }
    let trigger = c
        .quests
        .content
        .triggers
        .iter_mut()
        .find(|t| t.id.as_str() == "trigger/gate-ward")
        .expect("the fixture's strike trigger");
    trigger.effects.push(
        serde_json::from_value(serde_json::json!({
            "type": "despawn-actor",
            "actor": SCENERY,
            "style": "vanish"
        }))
        .expect("despawn-actor parses"),
    );
    c
}

/// Every tag a body carrying compiler-written loot NBT wears, read off the
/// emitted `summon` lines themselves: a summon whose NBT points `DeathLootTable`
/// at a compiler-emitted drop table, or marks a slot with the guaranteed-drop
/// chance.
fn loot_bearing_tags(out: &BuildOutput) -> std::collections::BTreeSet<String> {
    let mut tags = std::collections::BTreeSet::new();
    for (path, bytes) in out {
        if !(path.starts_with("datapack/") && path.ends_with(".mcfunction")) {
            continue;
        }
        for line in std::str::from_utf8(bytes).unwrap().lines() {
            let Some(at) = line.find("summon ") else {
                continue;
            };
            let summon = &line[at..];
            let carries = summon.contains(&format!("DeathLootTable:\"{NS}:dw_drop/"))
                || summon
                    .split("drop_chances:{")
                    .nth(1)
                    .and_then(|r| r.split('}').next())
                    .is_some_and(|chances| chances.contains(":2.0f"));
            if !carries {
                continue;
            }
            let list = summon
                .split("Tags:[")
                .nth(1)
                .and_then(|r| r.split(']').next())
                .expect("every compiler summon carries its Tags");
            for t in list.split(',') {
                tags.insert(t.trim_matches('"').to_string());
            }
        }
    }
    tags
}

/// Every `tag=` a line's selectors name.
fn selector_tags(line: &str) -> Vec<String> {
    line.split("tag=")
        .skip(1)
        .map(|r| {
            r.chars()
                .take_while(|c| !matches!(c, ',' | ']' | ' '))
                .collect()
        })
        .collect()
}

/// **The general form**: a declared drop is what a PLAYER's kill yields. Every
/// removal the shipped datapack makes of a loot-bearing body must be preceded,
/// in the same function, by the strip for that same tag — vanilla `/kill` is an
/// ordinary death, and a preserved slot or a death loot table rolls whoever the
/// killer was. A removal is a `kill` whose selector reaches the body, or the
/// retag to `dw_unseen` that hands it to `unseen_sweep`'s `kill` under the world
/// (after that retag no selector naming its own tag can reach it again).
///
/// The playtest this exists for: a rest re-seated an undefeated billed elite
/// (`wave_reseat_<wave>`) with a bare `kill`, and the party picked the elite's
/// quest key up off the floor where he had stood — once per rest, and once per
/// death at the fire — before ever fighting him.
///
/// The loot-bearing set is read off the emitted summons, not listed here, so a
/// body a later change teaches to carry loot is covered without an edit; and the
/// sites are enumerated from every function, so a new removal is too.
#[test]
fn every_compiler_removal_strips_declared_loot_first() {
    let out = build(&fixture_campaign_with_drops());
    let loot = loot_bearing_tags(&out);
    for want in [
        format!("dw_actor_{ELITE_SAFE}"),
        format!("dw_pup_{ELITE_SAFE}"),
        format!("dw_actor_{SCENERY_SAFE}"),
        "dw_wave_ambush".to_string(),
        "dw_wave_guards".to_string(),
    ] {
        assert!(
            loot.contains(&want),
            "the fixture's drop-declaring body `{want}` is read as loot-bearing: {loot:?}"
        );
    }

    let mut bound: Vec<String> = Vec::new();
    let mut unstripped: Vec<String> = Vec::new();
    for (path, bytes) in &out {
        if !(path.starts_with("datapack/") && path.ends_with(".mcfunction")) {
            continue;
        }
        let body = std::str::from_utf8(bytes).unwrap();
        let mut stripped: std::collections::BTreeSet<String> = Default::default();
        for line in body.lines() {
            if line.contains("run data merge entity @s {drop_chances:{")
                && line.contains("DeathLootTable:\"minecraft:empty\"")
            {
                stripped.extend(selector_tags(line));
                continue;
            }
            let is_kill = line.starts_with("kill ") || line.contains(" run kill ");
            let is_unseen_exit = line.contains("run data merge entity @s {Tags:[\"dw_unseen\"]");
            if !is_kill && !is_unseen_exit {
                continue;
            }
            for t in selector_tags(line).into_iter().filter(|t| loot.contains(t)) {
                let site = format!("{path}: {line}");
                if stripped.contains(&t) {
                    bound.push(site);
                } else {
                    unstripped.push(site);
                }
            }
        }
    }
    eprintln!(
        "removal-strip binding: {} loot-bearing tag(s) read off the summons; {} removal(s) of \
         one examined, {} stripped first, {} bare.",
        loot.len(),
        bound.len() + unstripped.len(),
        bound.len(),
        unstripped.len()
    );
    assert!(
        unstripped.is_empty(),
        "{} of {} removal(s) of a loot-bearing body run a bare `kill`, which yields the \
         declared drop to nobody's kill:\n{}",
        unstripped.len(),
        unstripped.len() + bound.len(),
        unstripped.join("\n")
    );
    // Binding: every removal site this fixture is built to reach was examined.
    for site in [
        "wave_reseat_ambush.mcfunction",
        "wave_reseat_guards.mcfunction",
        &format!("actor_restand_{ELITE_SAFE}.mcfunction"),
        &format!("unleash_{ELITE_SAFE}.mcfunction"),
    ] {
        assert!(
            bound.iter().any(|b| b.contains(site)),
            "the scan reached `{site}` (bound {}):\n{}",
            bound.len(),
            bound.join("\n")
        );
    }
    assert!(
        bound.iter().any(|b| b.contains(&format!(
            "@e[tag=dw_actor_{SCENERY_SAFE}] run data merge entity @s {{Tags:[\"dw_unseen\"]"
        ))),
        "the scan reached the `despawn-actor` (bound {}):\n{}",
        bound.len(),
        bound.join("\n")
    );
}

/// The runtime half ships as a generated PackTest: every drop-declaring body a
/// rest re-seats is met, dragged onto the party, unleashed and rested through
/// the REAL functions, each followed by the REAL `unseen_sweep`, and no item may
/// lie where the removed bodies die (Y −128 in the party's column) after either
/// — then a bare `kill` of each fresh body at that same place must yield one, so
/// the zero is a measurement and not an empty room. Absent when no re-seated body declares a
/// drop.
#[test]
fn the_removal_rule_ships_its_packtest() {
    let out = build(&fixture_campaign_with_drops());
    let t = packtest(&out, "souls_reseat_yields_nothing");
    for want in [
        format!("function {NS}:unleash_{ELITE_SAFE}"),
        format!("function {NS}:bonfire_rest_0\nfunction {NS}:unseen_sweep\n"),
        "assert score #u_rsyn dw.sys matches 0".to_string(),
        "assert score #r_rsyn dw.sys matches 0".to_string(),
    ] {
        assert!(t.contains(&want), "`{want}` in the template:\n{t}");
    }
    for tag in [
        format!("dw_actor_{ELITE_SAFE}"),
        "dw_wave_ambush".to_string(),
        "dw_wave_guards".to_string(),
    ] {
        assert!(
            t.contains(&format!(
                "execute at @a[tag=dw_rsyn,limit=1] positioned ~ -128 ~ run tp @e[tag={tag}] ~ ~ ~\n\
                 kill @e[tag={tag}]\n\
                 execute at @a[tag=dw_rsyn,limit=1] positioned ~ -128 ~ store result score #p_rsyn"
            )),
            "each re-seated body `{tag}` owes its own non-vacuity control:\n{t}"
        );
    }
    let bare = build(&fixture_campaign(true));
    assert!(
        !bare.contains_key(&format!(
            "packtest-datapack/data/{NS}/test/souls_reseat_yields_nothing.mcfunction"
        )),
        "no drop-declaring re-seated body, no template"
    );
}
