//! spec-0022 — command-driven trap payloads, end-to-end.
//!
//! Redstone keeps exactly one job (the trigger); the consequence is commands.
//! These tests pin the two new payload verbs and, above all, the owner's
//! **saturation ruling**: a volley BLANKETS its kill zone, and that
//! is proven at compile time rather than hoped for at runtime.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildFailure, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{parse_campaign, validate_campaign_with};

const NS: &str = "hello-world";

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A prefab copy exposing the spec-0022 surface: the `anchor/trap` trigger, a
/// `anchor/gallery` firing slot, a `anchor/killzone` REGION over the floor the
/// party crosses, and a `anchor/ceiling` region to bring down.
fn payload_prefabs(name: &str, extra: &[(&str, serde_json::Value)]) -> PathBuf {
    let dir = tmp(name);
    common::copy_dir_all(&common::prefabs_dir(), &dir);
    let path = dir.join("hello-room.json");
    let mut meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let anchors = meta
        .get_mut("anchors")
        .and_then(|a| a.as_object_mut())
        .unwrap();
    anchors.insert(
        "anchor/trap".to_string(),
        serde_json::json!({ "pos": [5, 1, 6], "dispenser": [4, 1, 6] }),
    );
    anchors.insert(
        "anchor/lever".to_string(),
        serde_json::json!({ "pos": [3, 1, 6] }),
    );
    for (k, v) in extra {
        anchors.insert((*k).to_string(), v.clone());
    }
    std::fs::write(&path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();
    dir
}

fn world_v06() -> serde_json::Value {
    serde_json::json!({
        "dsl_version": "0.6.0",
        "campaign_id": "hello-world",
        "stage": "world",
        "content": {
            "title": "The Keeper's Door",
            "theme": "A lonely keep at the edge of the moor.",
            "premise": "One locked door stands between you and the road home.",
            "seed": 20260729,
            "target_minutes": 5,
            "areas": [ { "id": "area/keep", "name": "The Keep", "prefab": "prefab/hello-room" } ]
        }
    })
}

fn quests_v06(trap: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "dsl_version": "0.6.0",
        "campaign_id": "hello-world",
        "stage": "quests",
        "content": {
            "quests": [ {
                "id": "quest/open-the-door",
                "trigger": { "type": "campaign-start" },
                "objectives": [
                    { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" },
                    { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit", "radius": 2, "after": ["obj/talk"] }
                ],
                "on_objective_complete": { "obj/talk": [ { "type": "open-gate", "anchor": "anchor/door" } ] },
                "on_complete": [ { "type": "campaign-complete" } ]
            } ],
            "traps": [ trap ]
        }
    })
}

/// Build a hello-world variant carrying one payload trap.
fn build_payload(
    name: &str,
    trap: serde_json::Value,
    extra_anchors: &[(&str, serde_json::Value)],
) -> Result<BuildOutput, BuildFailure> {
    let camp_dir = tmp(&format!("{name}-camp"));
    let patch = serde_json::json!({
        "documents": { "world": world_v06(), "quests": quests_v06(trap) }
    });
    common::materialize_from(&common::hello_world_dir(), &patch, &camp_dir);
    let prefabs_dir = payload_prefabs(&format!("{name}-prefabs"), extra_anchors);

    let loaded = load_campaign_dir(&camp_dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("campaign parses");
    let prefabs = PrefabRegistry::load_dir(&prefabs_dir).unwrap();
    let items = FullItemRegistry::v1_21_11();
    let entities = FullEntityRegistry::v1_21_11();
    let diags = validate_campaign_with(&campaign, &items, &prefabs, &entities);
    assert!(diags.is_empty(), "must validate clean: {diags:#?}");

    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(prefabs_dir.join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
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
        &BTreeMap::new(),
    )
}

fn text(out: &BuildOutput, key: &str) -> String {
    String::from_utf8(
        out.get(key)
            .unwrap_or_else(|| panic!("missing {key}"))
            .clone(),
    )
    .unwrap()
}

fn fn_body(out: &BuildOutput, name: &str) -> String {
    text(
        out,
        &format!("datapack/data/{NS}/function/{name}.mcfunction"),
    )
}

/// Every generated function name, for locating the content-keyed volley fns.
fn fn_names(out: &BuildOutput) -> Vec<String> {
    let prefix = format!("datapack/data/{NS}/function/");
    let mut v: Vec<String> = out
        .keys()
        .filter_map(|k| k.strip_prefix(&prefix)?.strip_suffix(".mcfunction"))
        .map(str::to_string)
        .collect();
    v.sort();
    v
}

fn gallery() -> (&'static str, serde_json::Value) {
    ("anchor/gallery", serde_json::json!({ "pos": [5, 2, 2] }))
}

fn killzone() -> (&'static str, serde_json::Value) {
    ("anchor/killzone", serde_json::json!({ "pos": [5, 1, 4] }))
}

/// The kill zone as authored: the 3x3 floor patch centred on `anchor/killzone`.
fn zone_json() -> serde_json::Value {
    serde_json::json!({ "anchor": "anchor/killzone", "extent": [1, 0, 1] })
}

fn volley_trap(payload_extra: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "id": "trap/stair-volley",
        "at": "anchor/trap",
        "trigger": "pressure-plate",
        "lethality": "harmful",
        "reset": "rearm",
        "payload": [ payload_extra ]
    })
}

/// Validation-tier diagnostics for a payload trap (no build).
fn diags_for(
    name: &str,
    trap: serde_json::Value,
    extra_anchors: &[(&str, serde_json::Value)],
) -> Vec<delvewright_dsl::Diagnostic> {
    let camp_dir = tmp(&format!("{name}-camp"));
    let patch = serde_json::json!({
        "documents": { "world": world_v06(), "quests": quests_v06(trap) }
    });
    common::materialize_from(&common::hello_world_dir(), &patch, &camp_dir);
    let prefabs_dir = payload_prefabs(&format!("{name}-prefabs"), extra_anchors);
    let loaded = load_campaign_dir(&camp_dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("campaign parses");
    let prefabs = PrefabRegistry::load_dir(&prefabs_dir).unwrap();
    validate_campaign_with(
        &campaign,
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    )
}

fn codes(d: &[delvewright_dsl::Diagnostic]) -> Vec<&str> {
    d.iter().map(|x| x.code.as_str()).collect()
}

fn err_code(e: &BuildFailure) -> String {
    format!("{e:?}")
}

// ---------------------------------------------------------------------------
// The saturation contract
// ---------------------------------------------------------------------------

/// A volley BLANKETS its kill zone: every standable cell of the zone receives
/// an unconditional projectile every salvo, on a trajectory computed for that
/// cell. This is the whole ruling — a player inside the zone is hit wherever
/// they stand, so escaping means LEAVING the zone rather than strafing luckily.
#[test]
fn volley_saturates_every_standable_kill_zone_cell() {
    let out = build_payload(
        "p-saturate",
        volley_trap(serde_json::json!({
            "type": "volley",
            "from_anchor": "anchor/gallery",
            "kill_zone": zone_json()
        })),
        &[gallery(), killzone()],
    )
    .expect("builds");

    // The kill zone is the 3x3 floor patch [4..6] x [3..5] at y=65; every cell
    // of it is standable in the open room, so every cell must be covered.
    let salvo = fn_names(&out)
        .into_iter()
        .find(|n| n.starts_with("volley_") && n.ends_with("_s0"))
        .expect("a first-salvo function exists");
    let body = fn_body(&out, &salvo);
    let mut covered = 0;
    for x in 4..=6 {
        for z in 3..=5 {
            covered += 1;
            assert!(
                body.contains(&format!("x={x},dx=0,y=65,dy=0,z={z},dz=0")),
                "kill-zone cell [{x},65,{z}] gets no aimed shot:\n{body}"
            );
        }
    }
    assert_eq!(covered, 9);
    // Nine unconditional saturation shots (one per cell) + nine conditional
    // aimed extras. The unconditional ones are what make coverage independent
    // of where the player is standing THIS tick.
    let unconditional = body
        .lines()
        .filter(|l| l.starts_with("summon minecraft:arrow"))
        .count();
    assert_eq!(
        unconditional, 9,
        "saturation must be unconditional:\n{body}"
    );
    let aimed = body
        .lines()
        .filter(|l| l.starts_with("execute if entity @a[") && l.contains("summon minecraft:arrow"))
        .count();
    assert_eq!(aimed, 9, "one aimed extra per cell:\n{body}");

    // Projectiles fly the exact straight segment the coverage proof checked.
    assert!(body.contains("NoGravity:1b"), "{body}");
    // Deterministic damage — a random crit bonus would make assertions flaky.
    assert!(body.contains("crit:0b"), "{body}");
}

/// Salvos repeat the whole pattern on a `schedule` chain, so a volley costs
/// **nothing per tick** — the timed-gate precedent.
#[test]
fn volley_salvos_are_scheduled_and_cost_nothing_per_tick() {
    let out = build_payload(
        "p-salvos",
        volley_trap(serde_json::json!({
            "type": "volley",
            "from_anchor": "anchor/gallery",
            "kill_zone": zone_json(),
            "salvos": 4,
            "interval": 15
        })),
        &[gallery(), killzone()],
    )
    .expect("builds");

    let start = fn_names(&out)
        .into_iter()
        .find(|n| n.starts_with("volley_") && !n.contains("_s"))
        .expect("a volley start function exists");
    let body = fn_body(&out, &start);
    assert!(
        body.contains(&format!("function {NS}:{start}_s0")),
        "{body}"
    );
    for (i, at) in [(1, 15), (2, 30), (3, 45)] {
        assert!(
            body.contains(&format!("schedule function {NS}:{start}_s{i} {at}t")),
            "salvo {i} must be scheduled at {at}t:\n{body}"
        );
    }
    // Every salvo fires the identical saturating pattern.
    let s0 = fn_body(&out, &format!("{start}_s0"));
    for i in 1..4 {
        assert_eq!(s0, fn_body(&out, &format!("{start}_s{i}")));
    }
    let tick = fn_body(&out, "tick");
    assert!(
        !tick.contains("volley_"),
        "a volley costs nothing per tick: {tick}"
    );
}

/// The trigger fires the payload, edge-triggered so standing on a plate is one
/// event, and re-arms when the cell is vacated.
#[test]
fn a_payload_trap_emits_edge_triggered_detection() {
    let out = build_payload(
        "p-detect",
        volley_trap(serde_json::json!({
            "type": "volley",
            "from_anchor": "anchor/gallery",
            "kill_zone": zone_json()
        })),
        &[gallery(), killzone()],
    )
    .expect("builds");
    let tick = fn_body(&out, "tick");
    assert!(
        tick.contains(
            "execute unless score #trapfire_stair_volley dw.sys matches 1 if entity \
             @a[x=5,dx=0,y=65,dy=0,z=6,dz=0,tag=!dw_cutscene] run function \
             hello-world:trap_fire_stair_volley"
        ),
        "{tick}"
    );
    assert!(
        tick.contains(
            "execute unless entity @a[x=5,dx=0,y=65,dy=0,z=6,dz=0,tag=!dw_cutscene] run \
             scoreboard players set #trapfire_stair_volley dw.sys 0"
        ),
        "rearm clause missing: {tick}"
    );
    let fire = fn_body(&out, "trap_fire_stair_volley");
    assert!(fire.contains("scoreboard players set #trapfire_stair_volley dw.sys 1"));
    assert!(fire.contains("function hello-world:volley_"), "{fire}");
}

// ---------------------------------------------------------------------------
// The coverage proof
// ---------------------------------------------------------------------------

/// `DW0442`: a gallery slot that cannot see the whole zone is a BUILD ERROR
/// naming the uncovered cell — the compile-time form of the saturation ruling.
#[test]
fn volley_with_a_blocked_line_of_fire_is_dw0442() {
    // The gallery sits on the far side of the doorway wall (z=6) from the kill
    // zone, so the wall stops the shots.
    let e = build_payload(
        "p-blocked",
        volley_trap(serde_json::json!({
            "type": "volley",
            "from_anchor": "anchor/gallery-far",
            "kill_zone": zone_json()
        })),
        &[
            (
                "anchor/gallery-far",
                serde_json::json!({ "pos": [1, 1, 8] }),
            ),
            killzone(),
        ],
    )
    .expect_err("a blocked gallery slot must not build");
    let msg = err_code(&e);
    assert!(msg.contains("DW0442"), "{msg}");
    assert!(msg.contains("no line of fire"), "{msg}");
    // It names the cell it cannot cover, so the author can fix the geometry.
    assert!(msg.contains("kill-zone cell ["), "{msg}");
}

/// `DW0446`: a firing slot buried in solid geometry never releases anything.
#[test]
fn volley_slot_inside_solid_geometry_is_dw0446() {
    let e = build_payload(
        "p-slot",
        volley_trap(serde_json::json!({
            "type": "volley",
            "from_anchor": "anchor/gallery-solid",
            "kill_zone": zone_json()
        })),
        &[
            (
                "anchor/gallery-solid",
                serde_json::json!({ "pos": [5, 0, 2] }),
            ),
            killzone(),
        ],
    )
    .expect_err("a solid gallery slot must not build");
    assert!(err_code(&e).contains("DW0446"), "{}", err_code(&e));
}

/// `DW0444`: a kill zone with no standable cell has nothing to saturate.
#[test]
fn volley_kill_zone_with_no_standable_cell_is_dw0444() {
    let e = build_payload(
        "p-empty-zone",
        volley_trap(serde_json::json!({
            "type": "volley",
            "from_anchor": "anchor/gallery",
            "kill_zone": { "anchor": "anchor/killzone-air", "extent": [1, 0, 1] }
        })),
        &[
            gallery(),
            (
                "anchor/killzone-air",
                serde_json::json!({ "pos": [5, 4, 4] }),
            ),
        ],
    )
    .expect_err("an unstandable kill zone must not build");
    assert!(err_code(&e).contains("DW0444"), "{}", err_code(&e));
}

/// `DW0447`: a kill zone centred on an anchor no prefab provides cannot be
/// resolved — reported rather than silently degenerating to an empty (and so
/// vacuously "covered") zone.
#[test]
fn volley_kill_zone_on_an_unknown_anchor_is_dw0447() {
    let e = build_payload(
        "p-point-zone",
        volley_trap(serde_json::json!({
            "type": "volley",
            "from_anchor": "anchor/gallery",
            "kill_zone": { "anchor": "anchor/nope", "extent": [1, 0, 1] }
        })),
        &[gallery()],
    )
    .expect_err("an unresolvable kill-zone anchor must not build");
    assert!(err_code(&e).contains("DW0447"), "{}", err_code(&e));
}

// ---------------------------------------------------------------------------
// Validation tier
// ---------------------------------------------------------------------------

/// `DW0440`: a trap with neither a redstone `effect` nor a command `payload`
/// does nothing at all.
#[test]
fn a_trap_with_no_consequence_is_dw0440() {
    let d = diags_for(
        "p-mute",
        serde_json::json!({
            "id": "trap/mute",
            "at": "anchor/trap",
            "trigger": "pressure-plate"
        }),
        &[],
    );
    assert!(codes(&d).contains(&"DW0440"), "{d:#?}");
}

/// `DW0443`: the volley cadence is bounded — `salvos x cells` is an entity
/// count, and a volley spread over minutes stops reading as one trap event.
#[test]
fn volley_cadence_out_of_range_is_dw0443() {
    let d = diags_for(
        "p-salvos-hi",
        volley_trap(serde_json::json!({
            "type": "volley",
            "from_anchor": "anchor/gallery",
            "kill_zone": zone_json(),
            "salvos": 99
        })),
        &[gallery(), killzone()],
    );
    assert!(codes(&d).contains(&"DW0443"), "{d:#?}");

    let d = diags_for(
        "p-interval-0",
        volley_trap(serde_json::json!({
            "type": "volley",
            "from_anchor": "anchor/gallery",
            "kill_zone": zone_json(),
            "interval": 0
        })),
        &[gallery(), killzone()],
    );
    assert!(codes(&d).contains(&"DW0443"), "{d:#?}");
}

/// `DW0441` at validation tier: unknown projectile / block ids.
#[test]
fn unknown_payload_verb_ids_are_dw0441() {
    let d = diags_for(
        "p-bad-proj",
        volley_trap(serde_json::json!({
            "type": "volley",
            "projectile": "minecraft:not_a_projectile",
            "from_anchor": "anchor/gallery",
            "kill_zone": zone_json()
        })),
        &[gallery(), killzone()],
    );
    assert!(codes(&d).contains(&"DW0441"), "{d:#?}");

    let d = diags_for(
        "p-bad-block",
        volley_trap(serde_json::json!({
            "type": "collapse",
            "region_anchor": { "anchor": "anchor/ceiling", "extent": [0, 0, 0] },
            "falling_block": "minecraft:not_a_block"
        })),
        &[],
    );
    assert!(codes(&d).contains(&"DW0441"), "{d:#?}");
}

// ---------------------------------------------------------------------------
// Byte-identity
// ---------------------------------------------------------------------------

/// A spec-0011 redstone trap — no `payload` — emits exactly what it emitted
/// before: no detection tick, no fire function, no payload machinery.
#[test]
fn a_payload_free_trap_emits_no_new_machinery() {
    let out = build_payload(
        "p-legacy",
        serde_json::json!({
            "id": "trap/dart-hall",
            "at": "anchor/trap",
            "trigger": "pressure-plate",
            "effect": { "dispense": { "item": "minecraft:arrow", "count": 8 } },
            "lethality": "harmful",
            "reset": "rearm"
        }),
        &[],
    )
    .expect("builds");
    let tick = fn_body(&out, "tick");
    assert!(!tick.contains("trapfire"), "{tick}");
    assert!(!tick.contains("volley_"), "{tick}");
    for n in fn_names(&out) {
        assert!(!n.starts_with("volley_"), "{n}");
        assert!(!n.starts_with("collapse_"), "{n}");
        assert!(!n.starts_with("trap_fire_"), "{n}");
    }
    // The legacy dispenser fill is untouched.
    let setup = fn_body(&out, "setup_finish");
    assert!(
        setup.contains("item replace block 4 65 6 container.0 with minecraft:arrow 8"),
        "{setup}"
    );
}

// ---------------------------------------------------------------------------
// collapse
// ---------------------------------------------------------------------------

fn collapse_trap(zone: serde_json::Value, extra: serde_json::Value) -> serde_json::Value {
    let mut eff = serde_json::json!({ "type": "collapse", "region_anchor": zone });
    if let (Some(o), Some(x)) = (eff.as_object_mut(), extra.as_object()) {
        for (k, v) in x {
            o.insert(k.clone(), v.clone());
        }
    }
    serde_json::json!({
        "id": "trap/cave-in",
        "at": "anchor/trap",
        "trigger": "tripwire",
        "lethality": "harmful",
        "reset": "once",
        "payload": [ eff ]
    })
}

fn ceiling() -> (&'static str, serde_json::Value) {
    ("anchor/ceiling", serde_json::json!({ "pos": [4, 5, 3] }))
}

fn ceiling_zone() -> serde_json::Value {
    serde_json::json!({ "anchor": "anchor/ceiling", "extent": [1, 0, 1] })
}

/// A collapse deletes the region and drops it as `falling_block` entities —
/// the buried-alive beat redstone cannot express at all.
#[test]
fn collapse_drops_the_ceiling_and_clears_the_region() {
    let out = build_payload(
        "p-collapse",
        collapse_trap(ceiling_zone(), serde_json::json!({})),
        &[ceiling()],
    )
    .expect("builds");
    let name = fn_names(&out)
        .into_iter()
        .find(|n| n.starts_with("collapse_"))
        .expect("a collapse function exists");
    let body = fn_body(&out, &name);

    // Nine ceiling cells (the 3x3 box at y=69) each become a falling block.
    let summons = body
        .lines()
        .filter(|l| l.starts_with("summon minecraft:falling_block"))
        .count();
    assert_eq!(summons, 9, "{body}");
    assert!(
        body.contains("BlockState:{Name:\"minecraft:gravel\"}"),
        "default falling block is gravel: {body}"
    );
    // Impact damage is what makes it a trap rather than a scenery effect.
    assert!(body.contains("HurtEntities:1b"), "{body}");
    // …and the region is genuinely gone afterwards.
    assert!(body.contains("fill 3 69 2 5 69 4 minecraft:air"), "{body}");
    // A collapse with no `then_floor` schedules no paving pass.
    assert!(!body.contains("schedule"), "{body}");
}

/// `then_floor` paves the settled surface once the rubble has landed — a
/// scheduled second pass, because the debris is in flight when the trap fires.
#[test]
fn collapse_then_floor_paves_the_settled_surface() {
    let out = build_payload(
        "p-collapse-floor",
        collapse_trap(
            ceiling_zone(),
            serde_json::json!({ "falling_block": "minecraft:sand", "then_floor": "minecraft:gravel" }),
        ),
        &[ceiling()],
    )
    .expect("builds");
    let name = fn_names(&out)
        .into_iter()
        .find(|n| n.starts_with("collapse_") && !n.ends_with("_floor"))
        .expect("a collapse function exists");
    let body = fn_body(&out, &name);
    assert!(
        body.contains("BlockState:{Name:\"minecraft:sand\"}"),
        "{body}"
    );
    assert!(
        body.contains(&format!("schedule function {NS}:{name}_floor")),
        "{body}"
    );
    let floor = fn_body(&out, &format!("{name}_floor"));
    // One paving setblock per debris column, on the cell the completability
    // proof treated as the new walking surface.
    let n = floor.lines().filter(|l| l.starts_with("setblock ")).count();
    assert_eq!(n, 9, "{floor}");
    assert!(floor.contains("minecraft:gravel"), "{floor}");
}

/// `DW0444`: a collapse region holding no blocks would drop nothing.
#[test]
fn collapse_region_with_no_blocks_is_dw0444() {
    let e = build_payload(
        "p-collapse-empty",
        collapse_trap(
            serde_json::json!({ "anchor": "anchor/hollow", "extent": [1, 0, 1] }),
            serde_json::json!({}),
        ),
        &[("anchor/hollow", serde_json::json!({ "pos": [5, 3, 3] }))],
    )
    .expect_err("an empty collapse region must not build");
    assert!(err_code(&e).contains("DW0444"), "{}", err_code(&e));
}

/// `DW0445`: a collapse that buries the only route is a build error.
///
/// The proof runs on the world where the trap has ALREADY fired, because a
/// player will step on the trigger — the same pessimism the `shortcut` seal
/// applies in the other direction (a shortcut is proven never-taken).
#[test]
fn collapse_that_buries_the_critical_path_is_dw0445() {
    // The lintel over the only doorway: its debris falls into the doorway
    // columns and seals the one route from spawn to exit.
    let e = build_payload(
        "p-collapse-buries",
        collapse_trap(
            serde_json::json!({ "anchor": "anchor/lintel", "extent": [1, 1, 0] }),
            serde_json::json!({}),
        ),
        &[("anchor/lintel", serde_json::json!({ "pos": [4, 4, 6] }))],
    )
    .expect_err("burying the critical path must not build");
    let msg = err_code(&e);
    assert!(msg.contains("DW0445"), "{msg}");
    assert!(msg.contains("no longer completable"), "{msg}");
}

/// The runtime half of the saturation ruling: the generated PackTest asserts,
/// per kill-zone cell, that a projectile exists on the trajectory reaching it.
#[test]
fn volley_packtest_asserts_coverage_of_every_cell() {
    let out = build_payload(
        "p-packtest",
        volley_trap(serde_json::json!({
            "type": "volley",
            "from_anchor": "anchor/gallery",
            "kill_zone": zone_json()
        })),
        &[gallery(), killzone()],
    )
    .expect("builds");
    let t = text(
        &out,
        &format!("packtest-datapack/data/{NS}/test/v06_volley.mcfunction"),
    );
    // One saturation assertion per standable cell (9), each demanding a gain.
    let asserts = t
        .lines()
        .filter(|l| l.starts_with("assert score #vgain_"))
        .count();
    assert_eq!(asserts, 9, "{t}");
    // The occupied cell must show the aimed EXTRA on top of the saturation shot.
    assert!(
        t.contains("assert score #vgain_0 dw.sys matches 2.."),
        "{t}"
    );
    // …and moving between salvos does not help: the new cell takes the extra.
    assert!(t.contains("assert score #vg2_1 dw.sys matches 2.."), "{t}");
    // The dummy is pinned before any absolute-coordinate teleport.
    let tag_line = t.find("tag @p add dw_pt_volley").expect("dummy pinned");
    let first_tp = t.find("tp @a[tag=dw_pt_volley]").expect("dummy moved");
    assert!(tag_line < first_tp, "pin the dummy before teleporting: {t}");
}

/// The collapse PackTest proves the region is really gone and the debris really
/// spawned — the two facts the completability proof assumed.
#[test]
fn collapse_packtest_asserts_region_gone_and_debris_spawned() {
    let out = build_payload(
        "p-packtest-collapse",
        collapse_trap(ceiling_zone(), serde_json::json!({})),
        &[ceiling()],
    )
    .expect("builds");
    let t = text(
        &out,
        &format!("packtest-datapack/data/{NS}/test/v06_collapse.mcfunction"),
    );
    assert!(t.contains("assert score #cpost dw.sys matches 9.."), "{t}");
    let air = t.lines().filter(|l| l.starts_with("assert block ")).count();
    assert_eq!(air, 9, "{t}");
}
