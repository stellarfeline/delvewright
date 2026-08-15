//! spec-0016 §6 (TD raider lanes + aggro-edge summoning) end-to-end tests,
//! driven by the `souls-td-lanes` fixture: a ruined keep where an illager
//! warband walks a three-waypoint circuit of the halls, while the drowned are
//! spirit-summoned at the edge of their own perception around the post the
//! party holds.
//!
//! The fixture area is a **prefab-pool draw**, not a single prefab, on purpose:
//! that is the layout the lane surface is hardest on (the solver has to be told
//! the waypoints are required, or it simply may not draw the pieces carrying
//! them), and it is the layout real campaigns use.
//!
//! A clean build is itself the DW0386 proof (every lane leg resolves, stands,
//! walks and is longer than 10 blocks) and the DW0387 proof (both drowned find
//! a standable, reachable, line-of-sight cell on their ring).

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildFailure, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{parse_campaign, validate_campaign_with};

const NS: &str = "souls-td-lanes";

fn fixture_dir() -> std::path::PathBuf {
    common::compiler_fixtures_dir().join(NS)
}

fn build_dir(dir: &Path) -> Result<BuildOutput, BuildFailure> {
    let loaded = load_campaign_dir(dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("campaign parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let items = FullItemRegistry::v1_21_11();
    let entities = FullEntityRegistry::v1_21_11();
    let diags = validate_campaign_with(&campaign, &items, &prefabs, &entities);
    assert!(diags.is_empty(), "fixture must validate clean: {diags:#?}");
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
}

fn build_fixture() -> BuildOutput {
    build_dir(&fixture_dir()).expect("the souls-td-lanes fixture builds clean")
}

/// The fixture with `quests.json` textually patched, built in a scratch dir.
fn build_patched(tag: &str, from: &str, to: &str) -> Result<BuildOutput, BuildFailure> {
    let dst = std::env::temp_dir().join(format!("dw-td-lanes-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dst);
    std::fs::create_dir_all(&dst).unwrap();
    for f in common::STAGE_FILES {
        std::fs::copy(fixture_dir().join(f), dst.join(f)).unwrap();
    }
    let qp = dst.join("quests.json");
    let q = std::fs::read_to_string(&qp).unwrap();
    assert!(
        q.contains(from),
        "patch anchor `{from}` present in quests.json"
    );
    std::fs::write(&qp, q.replace(from, to)).unwrap();
    let out = build_dir(&dst);
    let _ = std::fs::remove_dir_all(&dst);
    out
}

fn fn_body<'a>(out: &'a BuildOutput, name: &str) -> &'a str {
    let path = format!("datapack/data/{NS}/function/{name}.mcfunction");
    std::str::from_utf8(
        out.get(&path)
            .unwrap_or_else(|| panic!("missing fn {name}")),
    )
    .unwrap()
}

fn template<'a>(out: &'a BuildOutput, name: &str) -> &'a str {
    let path = format!("packtest-datapack/data/{NS}/test/{name}.mcfunction");
    std::str::from_utf8(
        out.get(&path)
            .unwrap_or_else(|| panic!("missing packtest {name}")),
    )
    .unwrap()
}

fn code_of(e: &BuildFailure) -> &str {
    match e {
        BuildFailure::Diagnostic { code, .. } => code.id(),
        other => panic!("expected a diagnostic, got {other:?}"),
    }
}

// --- the emission trap: snake_case or nothing ------------------------------

/// The single most important assertion in this file. 1.21.11's strict codec
/// **silently drops** the legacy `PatrolTarget:{X,Y,Z}` compound — the squad then
/// patrols to vanilla-rolled random points and reads as working-but-drunk. Only
/// the snake_case `patrol_target:[I;x,y,z]` int-array routes, so the shape of
/// that key is pinned as a test rather than left as a comment in the emitter.
#[test]
fn lane_squad_spawns_with_snake_case_patrol_target() {
    let out = build_fixture();
    let spawn = fn_body(&out, "spawn_warband");
    assert!(
        spawn.contains("patrol_target:[I;"),
        "the lane squad carries the snake_case int-array patrol target: {spawn}"
    );
    assert!(
        !spawn.contains("PatrolTarget"),
        "the legacy PascalCase compound is silently dropped by 1.21.11 and must \
         never be emitted: {spawn}"
    );
    // `Patrolling` and `PatrolLeader` keep their camelCase names — the rename is
    // specific to the target key, not a blanket convention change.
    assert_eq!(
        spawn.matches("Patrolling:1b").count(),
        3,
        "every squad member spawns patrolling: {spawn}"
    );
    assert_eq!(
        spawn.matches("PatrolLeader:1b").count(),
        1,
        "exactly one PatrolLeader (a lone leader is what vanilla expects): {spawn}"
    );
    assert!(
        spawn.contains("\"dw_lead_warband\""),
        "the leader is addressable at runtime: {spawn}"
    );
}

/// The compiler arms the lane roster itself: a pillager gets its crossbow (its
/// ONLY attack goal is crossbow-gated — unarmed it deadlocks on acquiring a
/// target), a vindicator its axe.
#[test]
fn lane_roster_is_armed_by_default() {
    let out = build_fixture();
    let spawn = fn_body(&out, "spawn_warband");
    assert_eq!(
        spawn.matches("minecraft:crossbow").count(),
        2,
        "both pillagers hold a crossbow: {spawn}"
    );
    assert!(
        spawn.contains("minecraft:iron_axe"),
        "the vindicator holds an axe: {spawn}"
    );
    assert!(
        spawn.contains("drop_chances:{mainhand:0.0f}"),
        "wave gear is never farmable (no-grind constitution): {spawn}"
    );
}

/// `aggro_radius` reaches the mobs as their `follow_range` attribute: release
/// radius and perception radius are the same number by construction.
#[test]
fn lane_follow_range_equals_aggro_radius() {
    let out = build_fixture();
    let spawn = fn_body(&out, "spawn_warband");
    assert_eq!(
        spawn
            .matches("{id:\"minecraft:follow_range\",base:16.0}")
            .count(),
        3,
        "every lane mob's follow_range is the lane's aggro_radius: {spawn}"
    );
    let tick = fn_body(&out, "lane_tick_warband");
    assert!(
        tick.contains("@a[distance=..16]"),
        "the release radius is the same 16: {tick}"
    );
}

/// **Stationed re-seat, lane half**. A wave re-seat is
/// `kill` + the wave's own `spawn_<wave>`, so everything that stations a lane
/// squad has to be written by `spawn_<wave>` and by nothing else — otherwise a
/// re-seated warband keeps the previous life's routing and marches on from
/// wherever the party last dragged it.
#[test]
fn the_spawn_alone_stations_the_lane_squad() {
    let out = build_fixture();
    let spawn = fn_body(&out, "spawn_warband");
    // Everyone routed, from waypoint 0.
    assert_eq!(
        spawn.matches("Patrolling:1b").count(),
        3,
        "every mob of the fresh squad is routed: {spawn}"
    );
    assert_eq!(
        spawn.matches("patrol_target:[I;").count(),
        3,
        "…and every one of them is pointed at a waypoint: {spawn}"
    );
    // The march clock is reset by the SPAWN, not by the re-seat wrapper, and
    // re-armed through the same replace-mode schedule.
    assert!(
        spawn.contains("scoreboard players set #lane_warband dw.sys 0"),
        "the spawn puts the march clock back at the lane start: {spawn}"
    );
    assert!(
        spawn.contains(&format!("schedule function {NS}:lane_tick_warband 30t")),
        "the spawn re-arms the lane clock (replace mode, so a re-seat cannot double it): {spawn}"
    );
}

/// The same claim on a live server, driven from the state that killed the
/// drowned bell's ladder: the squad hauled onto the party, released to native AI
/// by the real clock, its march clock at the lane's far end.
#[test]
fn the_lane_reseat_is_packtested() {
    let out = build_fixture();
    let t = template(&out, "souls_td_lane_reseat");
    assert!(
        t.contains("assert score #f_tdrst dw.sys matches 3"),
        "the squad is genuinely FERAL before the re-seat: {t}"
    );
    assert!(
        t.contains("scoreboard players set #lane_warband dw.sys 2"),
        "…and its march clock is run to the end of the lane: {t}"
    );
    assert!(
        t.contains("assert score #n_tdrst dw.sys matches 3"),
        "after the re-summon every mob is routed again — the release did not survive: {t}"
    );
    assert!(
        t.contains("assert score #lane_warband dw.sys matches 0"),
        "…and the march restarts from the lane start: {t}"
    );
}

// --- the lane clock --------------------------------------------------------

/// The clock implements the spike's verdict: advance on arrival, release at
/// aggro, re-assert while distant, and stop when the squad is dead.
#[test]
fn lane_tick_advances_releases_reasserts_and_self_terminates() {
    let out = build_fixture();
    let tick = fn_body(&out, "lane_tick_warband");
    assert!(
        tick.contains("run data merge entity @s {Patrolling:0b}"),
        "a player inside the radius releases the mob to native AI: {tick}"
    );
    assert!(
        tick.contains("unless entity @a[distance=..16] run data merge entity @s {Patrolling:1b,"),
        "a mob with nobody near is put back on the lane: {tick}"
    );
    assert!(
        tick.contains("scoreboard players set #lane_warband dw.sys"),
        "the shared waypoint index advances: {tick}"
    );
    assert!(
        tick.contains("execute if entity @e[tag=dw_wave_warband] run schedule function"),
        "the clock re-arms only while the squad lives: {tick}"
    );
    // The advance lines are emitted in DESCENDING index order so one cycle can
    // step at most one waypoint — an ascending emission would cascade the whole
    // lane inside a single tick.
    let advances: Vec<usize> = tick
        .lines()
        .filter(|l| l.contains("positioned") && l.contains("run scoreboard players set"))
        .map(|l| {
            l.split("matches ")
                .nth(1)
                .and_then(|r| r.split_whitespace().next())
                .and_then(|n| n.parse::<usize>().ok())
                .expect("an index in the guard")
        })
        .collect();
    assert!(advances.len() >= 2, "several advance guards: {tick}");
    assert!(
        advances.windows(2).all(|w| w[0] > w[1]),
        "advance guards descend so a cycle cannot cascade: {advances:?}\n{tick}"
    );
}

/// The march clock starts with the squad and nowhere else: a wave that is never
/// spawned never ticks.
#[test]
fn spawn_starts_the_lane_clock_at_waypoint_zero() {
    let out = build_fixture();
    let spawn = fn_body(&out, "spawn_warband");
    assert!(
        spawn.contains("scoreboard players set #lane_warband dw.sys 0"),
        "the lane starts at its first waypoint: {spawn}"
    );
    assert!(
        spawn.contains(&format!("schedule function {NS}:lane_tick_warband 30t")),
        "the clock is armed by the spawn: {spawn}"
    );
}

// --- aggro-edge summoning --------------------------------------------------

/// An aggro-edge wave never spawns at its anchor: every mob materializes on the
/// ring at its own `follow_range` from the defended point.
#[test]
fn aggro_edge_mobs_spawn_on_the_perception_ring() {
    let out = build_fixture();
    let spawn = fn_body(&out, "spawn_drowned");
    let ring = template(&out, "souls_td_aggro_edge");
    // The PackTest's ring band is derived from the same centre the placement
    // used, so parse the centre out of it and check the emitted summons against
    // the authored follow_range of 12.
    let centre: Vec<f64> = ring
        .lines()
        .find(|l| l.contains("positioned"))
        .expect("a positioned assertion")
        .split_whitespace()
        .skip_while(|t| *t != "positioned")
        .skip(1)
        .take(3)
        .map(|t| t.parse::<f64>().unwrap())
        .collect();
    let mut seen = 0;
    for line in spawn.lines().filter(|l| l.starts_with("summon ")) {
        let p: Vec<f64> = line
            .split_whitespace()
            .skip(2)
            .take(3)
            .map(|t| t.parse::<f64>().unwrap())
            .collect();
        let d =
            ((p[0] - centre[0]).powi(2) + (p[1] - centre[1]).powi(2) + (p[2] - centre[2]).powi(2))
                .sqrt();
        assert!(
            (11.0..=12.0).contains(&d),
            "a drowned materialized {d:.2} blocks from the defended point, outside \
             the one-sided ring [follow_range-1, follow_range] = [11, 12]: {line}"
        );
        seen += 1;
    }
    assert_eq!(seen, 2, "both drowned are summoned: {spawn}");
    // No patrol NBT: species without patrol AI never march a lane.
    assert!(
        !spawn.contains("Patrolling"),
        "an aggro-edge wave carries no patrol NBT: {spawn}"
    );
}

// --- generated PackTests ---------------------------------------------------

/// Four runtime templates, one per claim the mechanism makes.
#[test]
fn four_td_packtests_are_generated() {
    let out = build_fixture();
    for t in [
        "souls_td_patrol_nbt",
        "souls_td_lane_march",
        "souls_td_lane_release",
        "souls_td_aggro_edge",
    ] {
        let body = template(&out, t);
        assert!(body.starts_with("#> "), "{t} has a title: {body}");
        assert!(
            body.contains("assert score "),
            "{t} actually asserts something: {body}"
        );
        assert!(
            body.contains("kill @e[tag=dw_wave_"),
            "{t} clears wave entities (batch model: own init, no residue): {body}"
        );
    }
    // The codec assertion is a runtime read-back of the int-array, not a text
    // check on the emitter.
    assert!(
        template(&out, "souls_td_patrol_nbt").contains("nbt={patrol_target:[I;"),
        "the snake_case round-trip is asserted on a live server"
    );
    // Release is asserted in both directions.
    let rel = template(&out, "souls_td_lane_release");
    assert!(rel.contains("assert score #d_tdrel dw.sys matches 3"));
    assert!(rel.contains("assert score #a_tdrel dw.sys matches 0"));
}

/// A campaign with no lane and no aggro-edge wave emits none of the §6 surface —
/// pre-§6 output stays byte-identical.
#[test]
fn a_plain_wave_campaign_emits_no_lane_machinery() {
    let dir = common::keep_trial_dir();
    let out = build_dir(&dir).expect("keep-trial builds");
    assert!(
        !out.keys().any(|k| k.contains("lane_tick_")),
        "no lane clock for a campaign that declares no lane"
    );
    assert!(
        !out.keys().any(|k| k.contains("souls_td_")),
        "no §6 PackTests for a campaign that declares no lane"
    );
    let spawns: Vec<&String> = out
        .keys()
        .filter(|k| k.contains("/function/spawn_") && k.ends_with(".mcfunction"))
        .collect();
    assert!(!spawns.is_empty(), "keep-trial has waves");
    for k in spawns {
        let body = std::str::from_utf8(&out[k]).unwrap();
        assert!(
            !body.contains("Patrolling") && !body.contains("patrol_target"),
            "no patrol NBT leaks into a plain wave: {body}"
        );
    }
}

/// A lane in a **pool** area only works if the layout solver knows the waypoints
/// are required: otherwise it may simply not draw the piece carrying one, and the
/// lane fails `DW0386` ("resolves nowhere") for a reason the author cannot act on
/// — the anchor IS in the pool, the layout just did not use it. The fixture's area
/// is a `pool/stone-keep` draw, so a clean build is that registration's proof;
/// this pins it by name.
#[test]
fn lane_waypoints_are_required_anchors_in_a_pool_area() {
    let dir = fixture_dir();
    let loaded = load_campaign_dir(&dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).unwrap();
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let area = plan
        .areas
        .iter()
        .find(|a| a.area_id == "area/keep")
        .expect("the keep area");
    assert!(
        area.pieces.len() > 1,
        "the fixture area really is a multi-piece pool draw"
    );
    let lane = campaign.quests.content.waves[0]
        .lane
        .as_ref()
        .expect("the fixture's first wave has a lane");
    for wp in &lane.waypoints {
        assert!(
            plan.point("area/keep", wp.as_str()).is_some(),
            "the solver placed a piece providing lane waypoint `{wp}`"
        );
    }
}

// --- geometry proofs -------------------------------------------------------

/// `DW0386`: a lane leg of 10 blocks or less. Vanilla re-rolls a patrol target to
/// a random point once the patroller is within 10 blocks of it, so a tighter lane
/// is one the engine quietly stops following.
#[test]
fn a_short_lane_leg_is_dw0386() {
    // Point the FIRST waypoint at the squad's own spawn anchor's neighbour four
    // blocks away: every other leg still passes, so the diagnostic is the spacing
    // rule and nothing else.
    let err = build_patched(
        "shortleg",
        "\"waypoints\": [\n            \"anchor/keeper-stand\",",
        "\"waypoints\": [\n            \"spawn\",",
    )
    .expect_err("a short leg must fail the build");
    assert_eq!(code_of(&err), "DW0386", "{err:?}");
    match &err {
        BuildFailure::Diagnostic { message, .. } => assert!(
            message.contains("4.0 blocks") && message.contains("not more than 10"),
            "the message names the measured leg: {message}"
        ),
        other => panic!("expected a diagnostic, got {other:?}"),
    }
}

/// `DW0387`: an aggro-edge wave whose perception ring holds no valid cell. A ring
/// far outside the arena is the ordinary way to get here — and it is an error, not
/// a silent short spawn (a short wave makes a `kill` countdown that never reaches
/// zero).
#[test]
fn an_unreachable_aggro_ring_is_dw0387() {
    let err = build_patched("noring", "\"follow_range\": 12", "\"follow_range\": 64")
        .expect_err("a ring outside the arena must fail the build");
    assert_eq!(code_of(&err), "DW0387", "{err:?}");
}

/// ADR-0006: the lane polyline and the ring placement are new ordering logic
/// (BFS + a squared-distance sort), so the fixture is pinned by a double-build
/// byte-identity gate like every other emission path.
#[test]
fn double_build_is_byte_identical() {
    let a = build_fixture();
    let b = build_fixture();
    assert_eq!(a.len(), b.len(), "same file set");
    for (path, bytes) in &a {
        assert_eq!(
            bytes,
            b.get(path).unwrap_or_else(|| panic!("{path} missing")),
            "double-build mismatch in {path}"
        );
    }
}
