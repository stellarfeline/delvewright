//! A body carries another (spec-0135): the seat, the medium, the ride leg and
//! the refusals, over a one-room bay the test writes itself.
//!
//! The room is a 21 × 9 × 9 stone box whose floor is the course `y = 3`, so a
//! body walks on `y = 4`. A pool fills `x = 6..=12` across the room, three deep
//! (`y = 1..=3`), its surface the course `y = 3`: the party swims it, and a
//! boat floats on it. The keeper stands west of it and the exit east, so the path's
//! second leg crosses the pool. Variants lay a bridge, move a mark, shrink a
//! bounds box, lay a rail line across, raise a step or a ledge, and declare
//! the second leg ridden.

use crate::common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildFailure, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, DSL_VERSION, parse_campaign};

const ROOM: &str = "vehicle-bay";
const SIZE: [i32; 3] = [21, 9, 9];
const POOL: std::ops::RangeInclusive<i32> = 6..=12;

/// What a variant lays over the bay.
#[derive(Clone, Copy, Default)]
struct Bay {
    /// A stone bridge across the pool along `z = 7`.
    bridge: bool,
    /// A rail line along `z = 7`, `x = 1..=14`, on the floor and a causeway
    /// through the pool, its west end against the wall and a stop past its
    /// east end.
    rails: bool,
    /// The rail line's west end is a powered rail that is powered: with the
    /// wall behind it, a launcher.
    launcher: bool,
    /// A stone course this many blocks high across the east floor at
    /// `x = 14`: one is a step, two a wall.
    step: Option<i32>,
    /// A platform two blocks over the east floor (`x = 14..=18`, `z = 1..=2`)
    /// with a rail along `z = 1`: no walk climbs it, a body drops from it.
    ledge: bool,
    /// The ledge's rail has nothing behind its first cell: an open end.
    ledge_open: bool,
    /// The ledge runs on to `z = 5`, so a goal on it stands clear of the rail.
    ledge_deep: bool,
    /// The vehicle the leg to the exit is declared ridden on.
    ride: Option<&'static str>,
    /// The anchor the second objective is reached at, when not the exit.
    goal: Option<&'static str>,
    /// Extra members of the quests document's content: `"state": […], …`.
    more: &'static str,
    /// Extra members of the second objective, from a leading comma.
    gate: &'static str,
    /// A carrot on a stick in every class's kit.
    carrot: bool,
    /// A second class, whose kit carries no carrot on a stick.
    second_class: bool,
    /// A rail cell at `[16, 4, 7]` in a nook walled off from the room: no
    /// walk reaches it.
    nook: bool,
    /// The pool runs `x = 4..=16`, too wide for a hand on either shore to
    /// reach its middle, with crates — barrels standing one block over the
    /// water, touching its surface — at `[11, 4, 4]`, and at `[11, 4, 6]`
    /// with `second_crate`.
    wide: bool,
    /// The second crate of the wide pool.
    second_crate: bool,
    /// The objectives after `obj/talk`, written whole (each from a leading
    /// comma), in place of the reach-anchor `obj/exit`.
    objectives: Option<&'static str>,
    /// A stone block at this cell: a lid over a body's head.
    lid: Option<[i32; 3]>,
}

/// The pool's courses across the room.
fn pool(bay: Bay) -> std::ops::RangeInclusive<i32> {
    if bay.wide { 4..=16 } else { POOL }
}

fn cells(bay: Bay) -> Vec<([i32; 3], &'static str)> {
    let [sx, sy, sz] = SIZE;
    let mut by: BTreeMap<[i32; 3], &'static str> = BTreeMap::new();
    for x in 0..sx {
        for z in 0..sz {
            for y in 0..sy {
                let shell = x == 0 || x == sx - 1 || z == 0 || z == sz - 1;
                let pool = pool(bay).contains(&x) && !shell && (1..=3).contains(&y);
                if pool {
                    by.insert([x, y, z], "minecraft:water[level=0]");
                } else if y < 4 || y == sy - 1 || shell {
                    by.insert([x, y, z], "minecraft:stone");
                }
            }
        }
    }
    if bay.wide {
        by.insert([11, 4, 4], "minecraft:barrel[facing=up,open=false]");
    }
    if bay.second_crate {
        by.insert([11, 4, 6], "minecraft:barrel[facing=up,open=false]");
    }
    if bay.bridge {
        for x in POOL {
            by.insert([x, 3, 7], "minecraft:stone");
        }
    }
    if bay.rails {
        for x in 1..=14 {
            by.insert([x, 3, 7], "minecraft:stone");
            by.insert(
                [x, 4, 7],
                if x == 1 && bay.launcher {
                    "minecraft:powered_rail[powered=true,shape=east_west,waterlogged=false]"
                } else {
                    "minecraft:rail[shape=east_west,waterlogged=false]"
                },
            );
        }
        by.insert([15, 4, 7], "minecraft:stone");
    }
    if let Some(high) = bay.step {
        for z in 1..=7 {
            for y in 4..4 + high {
                by.insert([14, y, z], "minecraft:stone");
            }
        }
    }
    if bay.ledge {
        for x in 14..=18 {
            for z in 1..=(if bay.ledge_deep { 5 } else { 2 }) {
                by.insert([x, 4, z], "minecraft:stone");
                by.insert([x, 5, z], "minecraft:stone");
            }
            by.insert(
                [x, 6, 1],
                "minecraft:rail[shape=east_west,waterlogged=false]",
            );
        }
        if !bay.ledge_open {
            for y in 4..=6 {
                by.insert([13, y, 1], "minecraft:stone");
            }
        }
    }
    if bay.nook {
        for y in 4..=6 {
            for x in 15..=17 {
                by.insert([x, y, 6], "minecraft:stone");
            }
            by.insert([15, y, 7], "minecraft:stone");
            by.insert([17, y, 7], "minecraft:stone");
        }
        by.insert(
            [16, 4, 7],
            "minecraft:rail[shape=east_west,waterlogged=false]",
        );
    }
    if let Some(c) = bay.lid {
        by.insert(c, "minecraft:stone");
    }
    for x in [3, 7, 11, 15, 18] {
        for z in [2, 6] {
            by.insert([x, sy - 1, z], "minecraft:glowstone");
        }
    }
    by.into_iter().collect()
}

fn anchors() -> serde_json::Value {
    serde_json::json!({
        "spawn": { "pos": [2, 4, 4], "facing": "east", "role": "entry" },
        "anchor/keeper-stand": { "pos": [3, 4, 6], "facing": "east" },
        "anchor/exit-west": { "pos": [17, 4, 4] },
        "anchor/exit": { "pos": [17, 4, 4] },
        "anchor/dock": { "pos": [8, 3, 3] },
        "anchor/dock-two": { "pos": [8, 3, 5] },
        "anchor/bay": { "pos": [9, 3, 4] },
        "anchor/floor": { "pos": [3, 4, 3] },
        "anchor/rail-end": { "pos": [1, 4, 7] },
        "anchor/ledge-end": { "pos": [18, 6, 5] },
        "anchor/rail-mid": { "pos": [10, 4, 6] },
        "anchor/room": { "pos": [10, 4, 4] },
        "anchor/nook": { "pos": [16, 4, 7] },
        "anchor/near-dock": { "pos": [5, 3, 3] },
        "anchor/near-dock-two": { "pos": [5, 3, 6] },
        "anchor/far-shore": { "pos": [19, 4, 4] },
        "anchor/crate": { "pos": [11, 4, 4] },
        "anchor/crate-alongside": { "pos": [10, 3, 4] },
        "anchor/crate-two": { "pos": [11, 4, 6] },
        "anchor/crate-two-alongside": { "pos": [12, 3, 6] },
        "anchor/cp-bridge": { "pos": [9, 4, 7] },
        "anchor/cp-east": { "pos": [14, 4, 7] },
        "anchor/cp-high": { "pos": [9, 7, 4] }
    })
}

fn scratch(tag: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("vehicle")
        .join(format!("{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Write the bay with `vehicles` (a JSON array body) and `talk` (the
/// effects the first beat runs).
fn write(tag: &str, bay: Bay, vehicles: &str, talk: &str) -> (PathBuf, PathBuf) {
    let root = scratch(tag);
    let lib = root.join("prefabs");
    common::write_single_prefab(&lib, ROOM, SIZE, &cells(bay), anchors());
    let dir = common::campaign_bound_to(&root.join("campaign"), ROOM);
    common::patch_file(&dir.join("world.json"), |v| {
        v["content"]["areas"][0]["mitigation"] = serde_json::json!("night-vision");
    });
    if bay.carrot {
        common::patch_file(&dir.join("classes.json"), |v| {
            for c in v["content"]["classes"].as_array_mut().unwrap() {
                c["kit"]
                    .as_array_mut()
                    .unwrap()
                    .push(serde_json::json!({ "item": "minecraft:carrot_on_a_stick", "count": 1 }));
            }
        });
    }
    if bay.second_class {
        common::patch_file(&dir.join("classes.json"), |v| {
            let mut second = v["content"]["classes"][0].clone();
            second["id"] = serde_json::json!("class/second");
            second["name"] = serde_json::json!("Second");
            second["kit"]
                .as_array_mut()
                .unwrap()
                .retain(|k| k["item"] != "minecraft:carrot_on_a_stick");
            v["content"]["classes"].as_array_mut().unwrap().push(second);
        });
    }
    let ride = bay
        .ride
        .map(|v| format!(r#", "ride": "{v}""#))
        .unwrap_or_default();
    let goal = bay.goal.unwrap_or("anchor/exit-west");
    let gate = bay.gate;
    let more = if bay.more.is_empty() {
        String::new()
    } else {
        format!(", {}", bay.more)
    };
    let vehicles = if vehicles.is_empty() {
        more
    } else {
        format!(r#", "vehicles": [ {vehicles} ]{more}"#)
    };
    let objectives = bay.objectives.map(str::to_string).unwrap_or_else(|| {
        format!(
            r#",
          {{ "type": "reach-anchor", "id": "obj/exit", "anchor": "{goal}",
             "radius": 1, "after": ["obj/talk"]{ride}{gate} }}"#
        )
    });
    let quests = format!(
        r#"{{
  "dsl_version": "{DSL_VERSION}",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {{
    "quests": [
      {{
        "id": "quest/open-the-door",
        "trigger": {{ "type": "campaign-start" }},
        "objectives": [
          {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }}{objectives}
        ],
        "on_objective_complete": {{
          "obj/talk": [ {talk} ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ]{vehicles}
  }}
}}"#
    );
    std::fs::write(dir.join("quests.json"), quests).unwrap();
    common::declare_story_dir(&dir);
    (dir, lib)
}

fn campaign(dir: &Path) -> Campaign {
    let loaded = load_campaign_dir(dir).unwrap();
    let mut c = parse_campaign(&loaded.raw).expect("the case parses");
    delvewright_dsl::tag_translatables(&mut c);
    c
}

fn validate(dir: &Path, lib: &Path) -> Vec<delvewright_dsl::Diagnostic> {
    let c = campaign(dir);
    let prefabs = PrefabRegistry::load_dir(lib).unwrap();
    delvewright_dsl::validate_campaign_with(
        &c,
        &delvec::compiler::registry::FullItemRegistry::v1_21_11(),
        &prefabs,
        &delvec::compiler::registry::FullEntityRegistry::v1_21_11(),
    )
}

fn try_build(dir: &Path, lib: &Path) -> Result<BuildOutput, BuildFailure> {
    let refused: Vec<_> = validate(dir, lib)
        .into_iter()
        .filter(|d| d.severity == delvewright_dsl::Severity::Error)
        .collect();
    assert!(
        refused.is_empty(),
        "the case does not validate: {refused:#?}"
    );
    let c = campaign(dir);
    let prefabs = PrefabRegistry::load_dir(lib).unwrap();
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let mut s = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                s.insert(
                    t.structure_file.clone(),
                    std::fs::read(lib.join(&t.structure_file)).unwrap(),
                );
            }
        }
    }
    let inputs = load_campaign_dir(dir).unwrap().inputs;
    emit::build_with_warnings(
        &plan,
        &inputs,
        &s,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
    .map(|(out, _)| out)
}

fn build(tag: &str, bay: Bay, vehicles: &str) -> BuildOutput {
    let (dir, lib) = write(tag, bay, vehicles, "");
    match try_build(&dir, &lib) {
        Ok(x) => x,
        Err(BuildFailure::Diagnostic { code, message }) => {
            panic!("expected the build to succeed, got {code}: {message}")
        }
        Err(e) => panic!("expected the build to succeed, got {e:?}"),
    }
}

fn refused(tag: &str, bay: Bay, vehicles: &str) -> (String, String) {
    let (dir, lib) = write(tag, bay, vehicles, "");
    match try_build(&dir, &lib) {
        Ok(_) => panic!("expected the build to be refused"),
        Err(BuildFailure::Diagnostic { code, message }) => (code.to_string(), message),
        Err(e) => panic!("expected a diagnostic, got {e:?}"),
    }
}

fn text(out: &BuildOutput, path: &str) -> String {
    String::from_utf8(out.get(path).unwrap_or_else(|| panic!("no {path}")).clone()).unwrap()
}

fn function(out: &BuildOutput, name: &str) -> String {
    let key = out
        .keys()
        .find(|k| {
            k.starts_with("datapack/data/") && k.ends_with(&format!("/function/{name}.mcfunction"))
        })
        .unwrap_or_else(|| panic!("no function {name}"))
        .clone();
    text(out, &key)
}

fn rides(out: &BuildOutput) -> Vec<serde_json::Value> {
    let cp: serde_json::Value = serde_json::from_str(&text(out, "critical-path.json")).unwrap();
    cp["steps"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["action"] == "ride")
        .cloned()
        .collect()
}

fn boat(id: &str, dock: &str, extent: [u32; 3]) -> String {
    format!(
        r#"{{ "id": "vehicle/{id}", "entity": "minecraft:oak_boat",
            "at": {{ "anchor": "anchor/{dock}" }},
            "bounds": {{ "anchor": "anchor/bay", "extent": [{}, {}, {}] }} }}"#,
        extent[0], extent[1], extent[2]
    )
}

fn two_boats() -> String {
    format!(
        "{}, {}",
        boat("gull", "dock", [8, 3, 4]),
        boat("tern", "dock-two", [8, 3, 4])
    )
}

/// Criterion 4: the party swims the pool until the leg is declared ridden;
/// declared, it is a ride leg by boat, with its medium cells, board and leave,
/// and the contract carries it.
#[test]
fn a_pool_is_swum_until_its_leg_is_declared_ridden() {
    let gull = boat("gull", "dock", [8, 3, 4]);
    assert!(rides(&build("pool-swum", Bay::default(), &gull)).is_empty());
    let out = build(
        "pool-ridden",
        Bay {
            ride: Some("vehicle/gull"),
            ..Bay::default()
        },
        &gull,
    );
    let r = rides(&out);
    assert_eq!(r.len(), 1, "{r:#?}");
    let ride = &r[0];
    assert_eq!(ride["vehicle"], "vehicle/gull");
    assert_eq!(ride["kind"], "boat");
    let medium = ride["medium"].as_array().unwrap();
    assert!(medium.len() >= 2, "{ride}");
    let first: Vec<i64> = medium[0]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect();
    assert_eq!(
        [first[0], first[2]],
        [8, 3],
        "the medium starts at the seat: {first:?}"
    );
    let (board, leave) = (xs(&ride["board"]), xs(&ride["leave"]));
    assert!(
        board < i64::from(*POOL.start()) && leave > i64::from(*POOL.end()),
        "{ride}"
    );
    assert_eq!(ride["rate"].as_f64().unwrap(), 5.0);
}

/// Criterion 4: the same pool with a bridge and no declaration is walked.
#[test]
fn a_bridged_pool_is_walked() {
    let out = build(
        "bridge",
        Bay {
            bridge: true,
            ..Bay::default()
        },
        &two_boats(),
    );
    assert!(rides(&out).is_empty());
}

/// Criterion 4's perturbation: bounds that hold the boat off both shores turn
/// the declared leg to `DW0311`, with no other change.
#[test]
fn bounds_short_of_the_shore_carry_nothing() {
    let (code, msg) = refused(
        "short",
        Bay {
            ride: Some("vehicle/gull"),
            ..Bay::default()
        },
        &boat("gull", "dock", [1, 3, 4]),
    );
    assert_eq!(code, "DW0311", "{msg}");
    assert!(
        msg.contains("declared ridden on `vehicle/gull`") && msg.contains("after leaving it"),
        "{msg}"
    );
}

fn ledge_cart() -> &'static str {
    r#"{ "id": "vehicle/v", "entity": "minecraft:minecart", "facing": "east",
        "at": { "anchor": "anchor/floor", "offset": [11, 2, -2] },
        "bounds": { "anchor": "anchor/room", "extent": [9, 3, 3] } }"#
}

/// Criterion 5: a declared ride on a leg no walk crosses carries the whole
/// party or is `DW1099` — a one-seat cart up to a ledge nobody climbs.
#[test]
fn a_required_ride_seats_the_party() {
    let (code, msg) = refused(
        "ledge-required",
        Bay {
            bridge: true,
            ledge: true,
            ledge_deep: true,
            ride: Some("vehicle/v"),
            goal: Some("anchor/ledge-end"),
            ..Bay::default()
        },
        ledge_cart(),
    );
    assert_eq!(code, "DW1099", "{msg}");
    assert!(msg.contains("seat 1 of the party's 4"), "{msg}");
}

/// Criterion 6: a census of a vehicle's riders is written by the tick — the
/// datum set to nothing, then one added for every player in the box whose
/// vehicle is the vehicle's body — and a `body-enters` over it fires on the
/// first rider.
#[test]
fn a_census_counts_a_vehicles_riders() {
    let more = r#""state": [ { "id": "state/aboard", "scope": "party",
            "counts": { "region": { "anchor": "anchor/bay", "extent": [8, 3, 4] },
                        "of": { "riders": "vehicle/gull" } } } ],
        "triggers": [ { "id": "trigger/cast-off",
            "on": { "on": "body-enters", "state": "state/aboard" },
            "requires_state": [ { "state": "state/aboard", "op": "at-least", "value": 1 } ],
            "effects": [ { "type": "narrate", "text": "The Gull casts off.", "style": "chat" } ] } ]"#;
    let out = build(
        "riders",
        Bay {
            more,
            ..Bay::default()
        },
        &boat("gull", "dock", [8, 3, 4]),
    );
    let tick = function(&out, "census_tick");
    let lines: Vec<&str> = tick.lines().collect();
    assert_eq!(
        lines[0], "scoreboard players set #party dw.s_aboard 0",
        "{tick}"
    );
    assert!(
        lines[1].starts_with("execute as @a[x=")
            && lines[1].ends_with(
                ",tag=!dw_cutscene] on vehicle if entity @s[tag=dw_vehicle_gull] run scoreboard players add #party dw.s_aboard 1"
            ),
        "{tick}"
    );
    assert!(
        tick.contains("if score #party dw.s_aboard matches 1.. ")
            && tick.contains("run function hello-world:be_cast_off"),
        "{tick}"
    );
    // The riders of a vehicle nobody declared are a dangling reference.
    let (dir, lib) = write(
        "riders-dangling",
        Bay {
            more,
            ..Bay::default()
        },
        &boat("tern", "dock", [8, 3, 4]),
        "",
    );
    let d = validate(&dir, &lib);
    assert!(
        d.iter()
            .any(|x| x.code == "DW0112" && x.message.contains("vehicle/gull")),
        "{d:#?}"
    );
}

/// Criterion 5: a mark outside `bounds` is `DW1099`.
#[test]
fn a_mark_outside_its_bounds_is_refused() {
    let v = r#"{ "id": "vehicle/gull", "entity": "minecraft:oak_boat",
        "at": { "anchor": "anchor/dock" },
        "bounds": { "anchor": "anchor/exit", "extent": [1, 1, 1] } }"#;
    let (code, msg) = refused("outside", Bay::default(), v);
    assert_eq!(code, "DW1099", "{msg}");
}

/// Criterion 3: `DW1097` on a boat over stone, a cart off a rail, a pig over
/// water.
#[test]
fn a_vehicle_out_of_its_medium_is_refused() {
    let cases = [
        (
            "boat-floor",
            r#"{ "id": "vehicle/v", "entity": "minecraft:oak_boat",
            "at": { "anchor": "anchor/floor" },
            "bounds": { "anchor": "anchor/room", "extent": [9, 3, 3] } }"#,
        ),
        (
            "cart-floor",
            r#"{ "id": "vehicle/v", "entity": "minecraft:minecart",
            "at": { "anchor": "anchor/floor" },
            "bounds": { "anchor": "anchor/room", "extent": [9, 3, 3] } }"#,
        ),
        (
            "pig-water",
            r#"{ "id": "vehicle/v", "entity": "minecraft:pig",
            "at": { "anchor": "anchor/bay" },
            "bounds": { "anchor": "anchor/room", "extent": [9, 3, 3] } }"#,
        ),
    ];
    for (tag, v) in cases {
        let (code, msg) = refused(
            tag,
            Bay {
                bridge: true,
                ..Bay::default()
            },
            v,
        );
        assert_eq!(code, "DW1097", "{tag}: {msg}");
    }
}

/// Criterion 3: each kind green in its medium, and what its seat emits.
#[test]
fn each_kind_is_seated_in_its_medium() {
    let vs = r#"{ "id": "vehicle/gull", "entity": "minecraft:oak_boat", "name": "The Gull",
            "at": { "anchor": "anchor/dock" },
            "bounds": { "anchor": "anchor/bay", "extent": [8, 3, 4] } },
        { "id": "vehicle/cart", "entity": "minecraft:minecart", "facing": "east",
            "at": { "anchor": "anchor/rail-end" },
            "bounds": { "anchor": "anchor/room", "extent": [9, 3, 3] } },
        { "id": "vehicle/race-pig", "entity": "minecraft:pig",
            "at": { "anchor": "anchor/floor" },
            "bounds": { "anchor": "anchor/room", "extent": [9, 3, 3] } },
        { "id": "vehicle/mare", "entity": "minecraft:horse",
            "at": { "anchor": "anchor/keeper-stand", "offset": [1, 0, 0] },
            "bounds": { "anchor": "anchor/room", "extent": [9, 3, 3] },
            "equipment": { "body": { "item": "minecraft:iron_horse_armor" } } }"#;
    let out = build(
        "kinds",
        Bay {
            bridge: true,
            rails: true,
            ..Bay::default()
        },
        vs,
    );
    let setup = function(&out, "setup_finish");
    for s in ["gull", "cart", "race_pig", "mare"] {
        assert!(
            setup.contains(&format!("vehicle_seat_{s}\n")),
            "{s}: {setup}"
        );
        assert!(
            setup.contains(&format!(
                "schedule function hello-world:vehicle_reseat_{s} 100t replace"
            )) || setup.contains(&format!("vehicle_reseat_{s} 100t replace")),
            "{s}"
        );
        let seat = function(&out, &format!("vehicle_seat_{s}"));
        assert!(seat.contains("Invulnerable:1b"), "{seat}");
        assert!(seat.contains(&format!("\"dw_vehicle_{s}\"")), "{seat}");
        let reseat = function(&out, &format!("vehicle_reseat_{s}"));
        assert!(
            reseat.contains(&format!("vehicle_reseat_{s} 100t replace")),
            "{reseat}"
        );
        assert!(reseat.contains("on passengers on vehicle"), "{reseat}");
        // A riderless body away from its mark is the one re-seated.
        assert!(
            reseat.contains(&format!(
                "execute as @e[tag=dw_vehicle_{s}] unless entity @s[x="
            )) && reseat.contains(",distance=..1.5] run tag @s add dw_vehicle_reseat"),
            "{reseat}"
        );
    }
    // A boat is summoned afloat in its water cell, where the pinned server's
    // save holds it at rest (the record §5: 0.52326431274413 over the cell's
    // floor), and its re-seat measures the mark from there: the body the
    // summon places is the body the save holds (`DW1040`). Summoned on the
    // water's top face, it settled a block lower than the ledger wrote it.
    let gull = function(&out, "vehicle_seat_gull");
    let summon = gull
        .lines()
        .find(|l| l.starts_with("summon minecraft:oak_boat "))
        .unwrap_or_else(|| panic!("{gull}"));
    let y: f64 = summon
        .split(' ')
        .nth(3)
        .and_then(|t| t.parse().ok())
        .expect(summon);
    assert!(
        (y - y.floor() - 0.523_264_312_744_13).abs() < 1e-9,
        "the boat is summoned at y {y}, not afloat in its water cell: {summon}"
    );
    let reseat = function(&out, "vehicle_reseat_gull");
    assert!(reseat.contains(&format!(",y={y:?},")), "{reseat}");
    // A riderless mount waits without its AI — it neither walks nor turns
    // from the yaw it was seated at (the record §13: with its AI, a mount at
    // speed nothing turned within a second, and the save held the turn) — and
    // boarding gives its AI back; a cart is pushed the way it faces; a boat is
    // neither.
    let board = function(&out, "vehicle_board");
    for s in ["race_pig", "mare"] {
        let seat = function(&out, &format!("vehicle_seat_{s}"));
        let summon = seat
            .lines()
            .find(|l| l.starts_with("summon "))
            .unwrap_or_else(|| panic!("{seat}"));
        assert!(summon.contains("NoAI:1b"), "{summon}");
        assert!(!seat.contains("movement_speed"), "{seat}");
        assert!(
            board.contains(&format!(
                "execute on vehicle run data merge entity @s[tag=dw_vehicle_{s}] {{NoAI:0b}}"
            )),
            "{board}"
        );
    }
    assert!(
        board.contains(
            "execute on vehicle run data merge entity @s[tag=dw_vehicle_cart] {Motion:[0.3d,0.0d,0.0d]}"
        ),
        "{board}"
    );
    assert!(!board.contains("gull"), "{board}");
    assert!(
        board.ends_with("advancement revoke @s only hello-world:vehicle_board\n"),
        "{board}"
    );
    let gull_seat = function(&out, "vehicle_seat_gull");
    assert!(
        gull_seat
            .lines()
            .filter(|l| l.starts_with("summon "))
            .all(|l| !l.contains("NoAI")),
        "{gull_seat}"
    );
    let adv = text(
        &out,
        out.keys()
            .find(|k| k.ends_with("/advancement/vehicle_board.json"))
            .expect("the boarding advancement"),
    );
    assert!(adv.contains("minecraft:started_riding"), "{adv}");
    let pig = function(&out, "vehicle_seat_race_pig");
    assert!(pig.contains("saddle:{id:\"minecraft:saddle\""), "{pig}");
    assert!(!pig.contains("Tame:1b"), "{pig}");
    let mare = function(&out, "vehicle_seat_mare");
    assert!(
        mare.contains("Tame:1b") && mare.contains("iron_horse_armor"),
        "{mare}"
    );
    let gull = function(&out, "vehicle_seat_gull");
    assert!(
        gull.contains("summon minecraft:oak_boat") && gull.contains("CustomName"),
        "{gull}"
    );
    assert!(!gull.contains("saddle"), "{gull}");
}

/// Criterion 3: `seat-vehicle` runs the seat function; `despawn-vehicle`
/// emits the removal lines.
#[test]
fn the_two_verbs_seat_and_remove() {
    let vs = boat("gull", "dock", [8, 3, 4]);
    let talk = r#"{ "type": "despawn-vehicle", "vehicle": "vehicle/gull" },
                  { "type": "seat-vehicle", "vehicle": "vehicle/gull" }"#;
    let (dir, lib) = write(
        "verbs",
        Bay {
            bridge: true,
            ..Bay::default()
        },
        &vs,
        talk,
    );
    let out = try_build(&dir, &lib).unwrap_or_else(|e| panic!("{e:?}"));
    let all: String = out
        .iter()
        .filter(|(k, _)| k.ends_with(".mcfunction"))
        .map(|(_, v)| String::from_utf8_lossy(v).into_owned())
        .collect();
    assert!(all.contains("execute as @e[tag=dw_vehicle_gull] on passengers run ride @s dismount"));
    // Once in `setup_finish`, once for the beat's `seat-vehicle`.
    assert!(all.matches(":vehicle_seat_gull\n").count() >= 2, "{all}");
    // A despawn stops the re-seat that would put the body back; a seat arms it.
    assert!(
        all.contains("schedule clear hello-world:vehicle_reseat_gull"),
        "{all}"
    );
}

/// Criterion 2 (validation half): an entity that carries no rider is
/// `DW1096`; a dangling vehicle reference is named.
#[test]
fn an_entity_that_carries_no_rider_is_refused() {
    for e in [
        "minecraft:oak_chest_boat",
        "minecraft:hopper_minecart",
        "minecraft:llama",
        "minecraft:zombie",
        "minecraft:nautilus",
    ] {
        let v = format!(
            r#"{{ "id": "vehicle/v", "entity": "{e}", "at": {{ "anchor": "anchor/floor" }},
                "bounds": {{ "anchor": "anchor/room", "extent": [9, 3, 3] }} }}"#
        );
        let (dir, lib) = write("rider", Bay::default(), &v, "");
        let d = validate(&dir, &lib);
        assert!(d.iter().any(|x| x.code == "DW1096"), "{e}: {d:#?}");
    }
    let (dir, lib) = write(
        "dangling",
        Bay::default(),
        "",
        r#"{ "type": "seat-vehicle", "vehicle": "vehicle/none" }"#,
    );
    let d = validate(&dir, &lib);
    assert!(
        d.iter().any(|x| x.message.contains("unknown vehicle")),
        "{d:#?}"
    );
}

/// Criterion 5: a cart whose medium no walk reaches strands its rider —
/// `DW1098`, naming the cell.
#[test]
fn a_cart_no_walk_reaches_strands_its_rider() {
    let v = r#"{ "id": "vehicle/v", "entity": "minecraft:minecart", "facing": "east",
        "at": { "anchor": "anchor/nook" },
        "bounds": { "anchor": "anchor/nook", "extent": [1, 1, 1] } }"#;
    let (code, msg) = refused(
        "nook",
        Bay {
            bridge: true,
            nook: true,
            ..Bay::default()
        },
        v,
    );
    assert_eq!(code, "DW1098", "{msg}");
    assert!(
        msg.contains("vehicle/v") && msg.contains("stranded"),
        "{msg}"
    );
}

fn cart() -> &'static str {
    r#"{ "id": "vehicle/cart", "entity": "minecraft:minecart", "facing": "east",
        "at": { "anchor": "anchor/rail-end" },
        "bounds": { "anchor": "anchor/room", "extent": [9, 3, 3] } }"#
}

fn pig() -> &'static str {
    r#"{ "id": "vehicle/race-pig", "entity": "minecraft:pig",
        "at": { "anchor": "anchor/floor" },
        "bounds": { "anchor": "anchor/room", "extent": [9, 3, 3] } }"#
}

fn xs(v: &serde_json::Value) -> i64 {
    v.as_array().unwrap()[0].as_i64().unwrap()
}

/// A boat with nobody in it is not in the boarding function, and a campaign
/// with only boats emits neither it nor its advancement.
#[test]
fn a_campaign_of_boats_answers_no_boarding() {
    let out = build("boats-only", Bay::default(), &two_boats());
    assert!(
        !out.keys().any(|k| k.contains("vehicle_board")),
        "a boat is neither pushed nor held"
    );
}

/// Criterion 3: a cart seated on a launcher, or facing across its rail, is
/// `DW1097` — it would not be at its mark when a body came for it.
#[test]
fn a_cart_that_does_not_rest_at_its_mark_is_refused() {
    let (code, msg) = refused(
        "launcher",
        Bay {
            rails: true,
            launcher: true,
            ..Bay::default()
        },
        cart(),
    );
    assert_eq!(code, "DW1097", "{msg}");
    assert!(msg.contains("launches a cart nobody sits in"), "{msg}");
    let across = cart().replace(r#""facing": "east""#, r#""facing": "south""#);
    let (code, msg) = refused(
        "across",
        Bay {
            rails: true,
            ..Bay::default()
        },
        &across,
    );
    assert_eq!(code, "DW1097", "{msg}");
}

/// Criterion 4: a leg declared ridden on a cart is a ride leg from the mark
/// to the run's end though a walk crosses it — and the same bay with no
/// declaration is walked.
#[test]
fn a_declared_leg_is_ridden_on_the_cart_though_a_walk_crosses() {
    let bay = Bay {
        rails: true,
        ..Bay::default()
    };
    assert!(rides(&build("cart-undeclared", bay, cart())).is_empty());
    let out = build(
        "cart-declared",
        Bay {
            ride: Some("vehicle/cart"),
            ..bay
        },
        cart(),
    );
    let r = rides(&out);
    assert_eq!(r.len(), 1, "{r:#?}");
    let ride = &r[0];
    assert_eq!(ride["vehicle"], "vehicle/cart");
    assert_eq!(ride["kind"], "minecart");
    let medium = ride["medium"].as_array().unwrap();
    assert_eq!(medium.len(), 14, "{ride}");
    assert_eq!(
        [xs(&medium[0]), xs(&medium[13])],
        [1, 14],
        "the run is the line, mark to end: {ride}"
    );
    assert!(xs(&ride["leave"]) >= 13, "{ride}");
    assert_eq!(ride["rate"].as_f64().unwrap(), 2.4);
    // The ride stands directly in front of the step whose leg it carries.
    let cp: serde_json::Value = serde_json::from_str(&text(&out, "critical-path.json")).unwrap();
    let steps = cp["steps"].as_array().unwrap();
    let at = steps.iter().position(|s| s["action"] == "ride").unwrap();
    assert_eq!(steps[at + 1]["objective"], "obj/exit", "{steps:#?}");
}

/// Criterion 4: a cart that faces its dead end goes nowhere, and a leg
/// declared ridden on it is `DW0311` naming the vehicle and why.
#[test]
fn a_declared_ride_that_goes_nowhere_is_refused() {
    let dead = cart().replace(r#""facing": "east""#, r#""facing": "west""#);
    let (code, msg) = refused(
        "cart-dead-end",
        Bay {
            rails: true,
            ride: Some("vehicle/cart"),
            ..Bay::default()
        },
        &dead,
    );
    assert_eq!(code, "DW0311", "{msg}");
    assert!(
        msg.contains("declared ridden on `vehicle/cart`")
            && msg.contains("goes nowhere")
            && msg.contains("The party can walk this leg"),
        "{msg}"
    );
}

/// Criterion 4: a mount rises a block and not two. Over a one-block step the
/// declared ride takes the pig across to the leg's end; at a two-block wall
/// its medium stops, and the leg — which no walk crosses either — is `DW0311`
/// naming the ride.
#[test]
fn a_mount_rises_a_block_and_not_two() {
    let bay = Bay {
        bridge: true,
        step: Some(1),
        ride: Some("vehicle/race-pig"),
        carrot: true,
        ..Bay::default()
    };
    let out = build("pig-step", bay, pig());
    let r = rides(&out);
    assert_eq!(r.len(), 1, "{r:#?}");
    assert_eq!(r[0]["kind"], "mount");
    assert_eq!(r[0]["steering_item"], "minecraft:carrot_on_a_stick");
    let far = r[0]["medium"]
        .as_array()
        .unwrap()
        .iter()
        .map(xs)
        .max()
        .unwrap();
    assert!(
        far > 14 && xs(&r[0]["leave"]) > 14,
        "the step is crossed: {}",
        r[0]
    );
    let (code, msg) = refused(
        "pig-wall",
        Bay {
            step: Some(2),
            ..bay
        },
        pig(),
    );
    assert_eq!(code, "DW0311", "{msg}");
    assert!(
        msg.contains("declared ridden on `vehicle/race-pig`")
            && msg.contains("The party cannot walk this leg either"),
        "{msg}"
    );
}

/// Criterion 2 (the path's half): a ride on a pig with no carrot on a stick
/// in hand before it is `DW1096`.
#[test]
fn a_ridden_pig_owes_its_steering_item() {
    let (code, msg) = refused(
        "pig-no-carrot",
        Bay {
            bridge: true,
            ride: Some("vehicle/race-pig"),
            ..Bay::default()
        },
        pig(),
    );
    assert_eq!(code, "DW1096", "{msg}");
    assert!(msg.contains("minecraft:carrot_on_a_stick"), "{msg}");
}

/// The kit rule `DW1090` shares (`MovementItems::first_held`): a carrot on a
/// stick in one class's kit of two is a path that rides with an empty hand.
#[test]
fn a_carrot_in_one_class_of_two_is_dw1096() {
    let (code, msg) = refused(
        "pig-one-class",
        Bay {
            bridge: true,
            ride: Some("vehicle/race-pig"),
            carrot: true,
            second_class: true,
            ..Bay::default()
        },
        pig(),
    );
    assert_eq!(code, "DW1096", "{msg}");
}

/// Whether a player's body with its feet anywhere in `cell` can touch the
/// inclusive cell box `lo..=hi` — the swept body box the bot's walk is held
/// to (`harness` `volumeReachesCell`) and the proof's keep-out rule is
/// (`World::body_can_meet_volume`).
fn body_touches(cell: [i64; 3], lo: [i64; 3], hi: [i64; 3]) -> bool {
    let (c, lo, hi) = (
        cell.map(|v| v as f64),
        lo.map(|v| v as f64),
        hi.map(|v| v as f64),
    );
    let over = |a0: f64, a1: f64, b0: f64, b1: f64| a0 < b1 && b0 < a1;
    over(c[0] - 0.3, c[0] + 1.3, lo[0], hi[0] + 1.0)
        && over(c[2] - 0.3, c[2] + 1.3, lo[2], hi[2] + 1.0)
        && over(c[1], c[1] + 1.8, lo[1], hi[1] + 1.0)
}

fn cell(v: &serde_json::Value) -> [i64; 3] {
    let a = v.as_array().unwrap();
    [
        a[0].as_i64().unwrap(),
        a[1].as_i64().unwrap(),
        a[2].as_i64().unwrap(),
    ]
}

/// The proof and the bot keep a ride out of an armed objective's completion
/// volume by one rule: no cell a ride boards from, passes or leaves at is one
/// a body touches the volume from, so the ride ends short of its own
/// objective and the party walks in. Asked of every ride the bay's three
/// kinds make to the exit, against the step's own exported `keep_out`.
#[test]
fn a_ride_keeps_out_of_its_objectives_volume_by_the_walks_rule() {
    let cases: [(&str, Bay, String); 3] = [
        (
            "keep-pig",
            Bay {
                bridge: true,
                ride: Some("vehicle/race-pig"),
                carrot: true,
                ..Bay::default()
            },
            pig().to_string(),
        ),
        (
            "keep-boat",
            Bay {
                ride: Some("vehicle/gull"),
                ..Bay::default()
            },
            boat("gull", "dock", [8, 3, 4]),
        ),
        (
            "keep-cart",
            Bay {
                rails: true,
                ride: Some("vehicle/cart"),
                ..Bay::default()
            },
            cart().to_string(),
        ),
    ];
    let mut judged = 0;
    for (tag, bay, vehicles) in cases {
        let r = rides(&build(tag, bay, &vehicles));
        assert_eq!(r.len(), 1, "{tag}: {r:#?}");
        let keep = r[0]["keep_out"]
            .as_array()
            .expect("the ride step keeps out");
        assert!(!keep.is_empty(), "{tag}: the leg's own objective is armed");
        // The walk to the vehicle goes round its body: the step names the
        // cells, the mark among them and the boarding cell not.
        let body: Vec<[i64; 3]> = r[0]["body"].as_array().unwrap().iter().map(cell).collect();
        let mark = cell(&r[0]["medium"][0]);
        assert!(body.contains(&mark), "{tag}: {}", r[0]);
        assert!(body.contains(&[mark[0], mark[1] + 1, mark[2]]), "{tag}");
        assert!(!body.contains(&cell(&r[0]["board"])), "{tag}");
        let mut cells: Vec<[i64; 3]> = r[0]["medium"]
            .as_array()
            .unwrap()
            .iter()
            .map(cell)
            .collect();
        cells.push(cell(&r[0]["board"]));
        cells.push(cell(&r[0]["leave"]));
        for k in keep {
            let (lo, hi) = (cell(&k["lo"]), cell(&k["hi"]));
            for c in &cells {
                judged += 1;
                assert!(
                    !body_touches(*c, lo, hi),
                    "{tag}: ride cell {c:?} touches {lo:?}..{hi:?}: {}",
                    r[0]
                );
            }
        }
    }
    assert!(judged > 20, "{judged} cell(s) judged");
}

/// A cart cannot stop short of where its rails take it: a line that runs
/// through its objective's completion volume carries no declared ride there.
#[test]
fn a_cart_whose_run_enters_its_objectives_volume_is_refused() {
    let (code, msg) = refused(
        "cart-into-volume",
        Bay {
            rails: true,
            ride: Some("vehicle/cart"),
            goal: Some("anchor/rail-mid"),
            ..Bay::default()
        },
        cart(),
    );
    assert_eq!(code, "DW0311", "{msg}");
    assert!(msg.contains("from the saddle"), "{msg}");
}

/// A leg declared ridden on a vehicle the path has despawned is `DW0311`
/// at the plan; seated again in the same bundle, it builds and is ridden.
#[test]
fn a_ride_after_its_vehicles_despawn_is_refused() {
    let bay = Bay {
        rails: true,
        ride: Some("vehicle/cart"),
        ..Bay::default()
    };
    let gone = r#"{ "type": "despawn-vehicle", "vehicle": "vehicle/cart" }"#;
    let (dir, lib) = write("ride-after-despawn", bay, cart(), gone);
    let c = campaign(&dir);
    let prefabs = PrefabRegistry::load_dir(&lib).unwrap();
    let err = match Plan::build(&c, &prefabs) {
        Ok(_) => panic!("expected the plan to be refused"),
        Err(e) => format!("{e:?}"),
    };
    assert!(
        err.contains("DW0311") && err.contains("takes that vehicle away"),
        "{err}"
    );
    let back = format!(r#"{gone}, {{ "type": "seat-vehicle", "vehicle": "vehicle/cart" }}"#);
    let (dir, lib) = write("ride-after-reseat", bay, cart(), &back);
    let out = try_build(&dir, &lib).expect("seated again, the ride stands");
    assert_eq!(rides(&out).len(), 1);
}

/// Criterion 3: a cart at the open end of its line is `DW1097` — a body that
/// walks into it rolls it off its rail, where the boarding push dies.
#[test]
fn a_cart_at_the_open_end_of_its_line_is_refused() {
    let (code, msg) = refused(
        "cart-open-end",
        Bay {
            bridge: true,
            ledge: true,
            ledge_open: true,
            ..Bay::default()
        },
        ledge_cart(),
    );
    assert_eq!(code, "DW1097", "{msg}");
    assert!(msg.contains("rolls it off its rail"), "{msg}");
}

/// spec-0135 §5: a ridden objective that waits for its own rider. The ride
/// goes into the objective's volume — it completes under the rider and
/// nowhere else — and the `ride` step carries the census the bot, aboard,
/// answers. Without the gate the same ride keeps out of the volume and
/// carries none.
#[test]
fn a_ride_whose_objective_waits_for_its_rider_enters_its_volume() {
    let pig_ride = |tag: &str, gate: &'static str, more: &'static str| {
        let out = build(
            tag,
            Bay {
                bridge: true,
                ride: Some("vehicle/race-pig"),
                carrot: true,
                gate,
                more,
                ..Bay::default()
            },
            pig(),
        );
        let r = rides(&out);
        assert_eq!(r.len(), 1, "{tag}: {r:#?}");
        r[0].clone()
    };
    // The objective's volume, as the ungated ride's step names it (a gated
    // objective is not armed while its gate is shut, so its own step names
    // none).
    let free = pig_ride("aboard-ungated", "", "");
    let k = free["keep_out"][0].clone();
    let (lo, hi) = (cell(&k["lo"]), cell(&k["hi"]));
    let touches = |ride: &serde_json::Value| {
        ride["medium"]
            .as_array()
            .unwrap()
            .iter()
            .map(cell)
            .chain([cell(&ride["leave"])])
            .any(|c| body_touches(c, lo, hi))
    };
    assert!(!touches(&free), "{free}");
    assert!(free.get("census").is_none(), "{free}");
    let gated = pig_ride(
        "aboard-gated",
        r#", "requires_state": [ { "state": "state/aboard", "op": "at-least", "value": 1 } ]"#,
        r#""state": [ { "id": "state/aboard", "scope": "party",
            "counts": { "region": { "anchor": "anchor/room", "extent": [9, 3, 3] },
                        "of": { "riders": "vehicle/race-pig" } } } ]"#,
    );
    assert!(touches(&gated), "the ride goes into the volume: {gated}");
    let row = &gated["census"][0];
    assert_eq!(row["state"], "state/aboard", "{gated}");
    assert_eq!(row["riders"], "dw_vehicle_race_pig");
    assert_eq!(row["count"], 1);
    assert!(row.get("select").is_none());
}

/// A `ride` that names no declared vehicle is refused at validation.
#[test]
fn a_ride_on_an_undeclared_vehicle_is_dangling() {
    let (dir, lib) = write(
        "ride-dangling",
        Bay {
            bridge: true,
            ride: Some("vehicle/none"),
            ..Bay::default()
        },
        pig(),
        "",
    );
    let d = validate(&dir, &lib);
    assert!(
        d.iter()
            .any(|x| x.code == "DW0112" && x.message.contains("rides unknown vehicle")),
        "{d:#?}"
    );
}

/// Criterion 5: the way off is judged on the reach graph. A cart whose rail
/// runs along a ledge no walk climbs strands nobody — a rider who leaves
/// drops to the floor the party walks.
#[test]
fn a_ledge_a_rider_drops_from_strands_nobody() {
    build(
        "ledge",
        Bay {
            bridge: true,
            ledge: true,
            ..Bay::default()
        },
        ledge_cart(),
    );
}

fn with_vehicle_field(v: &str, field: &str) -> String {
    let t = v.trim_end();
    format!("{}, {field} }}", &t[..t.len() - 1])
}

/// spec-0135 §2: a vehicle seated `by-story` stands nowhere at load — no seat
/// in `setup_finish` — and a leg ridden on it before any `seat-vehicle` is
/// `DW0311`; seated at the talk's beat, the build rides it.
#[test]
fn a_vehicle_seated_by_story_stands_nowhere_until_its_seat() {
    let bay = Bay {
        rails: true,
        ride: Some("vehicle/cart"),
        ..Bay::default()
    };
    let late = with_vehicle_field(cart(), r#""seated": "by-story""#);
    let (dir, lib) = write("by-story-unseated", bay, &late, "");
    let c = campaign(&dir);
    let prefabs = PrefabRegistry::load_dir(&lib).unwrap();
    let err = match Plan::build(&c, &prefabs) {
        Ok(_) => panic!("expected the plan to be refused"),
        Err(e) => format!("{e:?}"),
    };
    assert!(
        err.contains("DW0311") && err.contains("seated `by-story`"),
        "{err}"
    );
    let seat = r#"{ "type": "seat-vehicle", "vehicle": "vehicle/cart" }"#;
    let (dir, lib) = write("by-story-seated", bay, &late, seat);
    let out = try_build(&dir, &lib).expect("seated at the talk, the ride stands");
    assert_eq!(rides(&out).len(), 1);
    let setup = function(&out, "setup_finish");
    assert!(!setup.contains("vehicle_seat_cart"), "{setup}");
    assert!(!setup.contains("vehicle_reseat_cart"), "{setup}");
    // The written world's ledger agrees: the cart's body is placed at the
    // talk's step, never at load.
    let fired = seat_firings(&out, "vehicle_seat_cart");
    assert!(
        !fired.is_empty() && fired.iter().all(|f| f.get("step").is_some()),
        "a by-story vehicle's body is placed at the seat's step, never at load: {fired:?}"
    );
    // The same vehicle seated at load is seated in `setup_finish`.
    let (dir, lib) = write("at-load-seated", bay, cart(), "");
    let out = try_build(&dir, &lib).expect("seated at load");
    assert!(function(&out, "setup_finish").contains("vehicle_seat_cart"));
    let fired = seat_firings(&out, "vehicle_seat_cart");
    assert!(
        !fired.is_empty() && fired.iter().all(|f| f == "setup"),
        "an at-load vehicle's body is placed at load: {fired:?}"
    );
}

/// When each body row of `function` in `validation/placed.json` fires.
fn seat_firings(out: &BuildOutput, function: &str) -> Vec<serde_json::Value> {
    let placed: serde_json::Value =
        serde_json::from_str(&text(out, "validation/placed.json")).unwrap();
    placed["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["function"] == function && r["object"].get("entity").is_some())
        .map(|r| r["fired"].clone())
        .collect()
}

/// spec-0135 §3: a vehicle left where its rider leaves it is never re-seated
/// while it stands — its re-seat holds a riderless mount where it is without
/// its AI and seats only a missing body — and its PackTest is `_stays`.
#[test]
fn a_vehicle_left_where_its_rider_leaves_it_stays() {
    let bay = Bay {
        bridge: true,
        ..Bay::default()
    };
    let stays = with_vehicle_field(pig(), r#""left": "where-left""#);
    let out = build("left-where-left", bay, &stays);
    let reseat = function(&out, "vehicle_reseat_race_pig");
    assert!(!reseat.contains("distance=..1.5"), "{reseat}");
    assert!(
        reseat.contains(
            "execute as @e[tag=dw_vehicle_race_pig,tag=dw_vehicle_reseat] run data merge entity @s {NoAI:1b}"
        ),
        "{reseat}"
    );
    assert!(
        reseat.contains(
            "execute unless entity @e[tag=dw_vehicle_race_pig] run function hello-world:vehicle_seat_race_pig"
        ),
        "{reseat}"
    );
    assert!(
        !reseat
            .contains("if entity @e[tag=dw_vehicle_race_pig,tag=dw_vehicle_reseat] run function"),
        "{reseat}"
    );
    let tests: Vec<&String> = out
        .keys()
        .filter(|k| k.contains("/test/vehicle_race_pig_"))
        .collect();
    assert!(
        tests
            .iter()
            .any(|k| k.ends_with("vehicle_race_pig_stays.mcfunction")),
        "{tests:?}"
    );
    assert!(
        !tests
            .iter()
            .any(|k| k.ends_with("vehicle_race_pig_reseats.mcfunction")),
        "{tests:?}"
    );
    let stays_t = text(
        &out,
        tests
            .iter()
            .find(|k| k.ends_with("_stays.mcfunction"))
            .unwrap(),
    );
    assert!(
        stays_t.contains("assert entity @e[tag=dw_vehicle_race_pig,nbt={NoAI:1b}]"),
        "{stays_t}"
    );
    // At its mark (the default), the riderless body away from it is re-seated.
    let out = build("left-at-its-mark", bay, pig());
    let reseat = function(&out, "vehicle_reseat_race_pig");
    assert!(reseat.contains("distance=..1.5"), "{reseat}");
    assert!(!reseat.contains("NoAI"), "{reseat}");
}

/// spec-0135 §3, §8: a second ride on a vehicle that stays where its rider
/// left it, with no `seat-vehicle` between the rides, is `DW1107`; the same
/// two rides on a vehicle that returns to its mark are not.
#[test]
fn a_second_ride_on_a_vehicle_left_where_its_rider_left_it_is_dw1107() {
    let bay = Bay {
        rails: true,
        ride: Some("vehicle/cart"),
        ..Bay::default()
    };
    let twice = |tag: &str, v: &str| -> Result<(), String> {
        let (dir, lib) = write(tag, bay, v, "");
        common::patch_file(&dir.join("quests.json"), |q| {
            let objs = q["content"]["quests"][0]["objectives"]
                .as_array_mut()
                .unwrap();
            let mut again = objs[1].clone();
            again["id"] = serde_json::json!("obj/again");
            again["after"] = serde_json::json!(["obj/exit"]);
            objs.push(again);
        });
        let c = campaign(&dir);
        let prefabs = PrefabRegistry::load_dir(&lib).unwrap();
        Plan::build(&c, &prefabs)
            .map(|_| ())
            .map_err(|e| format!("{e:?}"))
    };
    let stays = with_vehicle_field(cart(), r#""left": "where-left""#);
    let err = twice("left-twice", &stays).expect_err("a second ride from where it was left");
    assert!(err.contains("DW1107") && err.contains("obj/again"), "{err}");
    if let Err(e) = twice("returns-twice", cart()) {
        assert!(!e.contains("DW1107"), "{e}");
    }
}

/// Two boats moored by the west shore of the wide pool: no walk reaches a
/// crate, so the ride carries the party of four (`DW1099`).
fn skiff() -> String {
    r#"{ "id": "vehicle/gull", "entity": "minecraft:oak_boat",
        "at": { "anchor": "anchor/near-dock" },
        "bounds": { "anchor": "anchor/bay", "extent": [8, 3, 4] } },
       { "id": "vehicle/tern", "entity": "minecraft:oak_boat",
        "at": { "anchor": "anchor/near-dock-two" },
        "bounds": { "anchor": "anchor/bay", "extent": [8, 3, 4] } }"#
        .to_string()
}

/// The crate in the middle of the wide pool, taken from the boat.
const ONE_CRATE: &str = r#",
          { "type": "collect", "id": "obj/crate", "item": "minecraft:cod", "count": 1,
            "anchor": "anchor/crate-alongside", "holder": "anchor/crate",
            "after": ["obj/talk"], "ride": "vehicle/gull" },
          { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/far-shore",
            "radius": 1, "after": ["obj/crate"] }"#;

/// A crate standing over open water that no hand on a shore reaches, its
/// collect declared ridden: the act is done from the seat — the ride ends at
/// a medium cell from which the barrel is in the rider's reach and in sight,
/// the party stays aboard for the act, and the walk on starts where a ride on
/// from there leaves it.
#[test]
fn a_crate_out_on_the_water_is_opened_from_the_boat() {
    let bay = Bay {
        wide: true,
        objectives: Some(ONE_CRATE),
        ..Bay::default()
    };
    let out = build("crate-from-the-seat", bay, &skiff());
    let r = rides(&out);
    assert_eq!(
        r.len(),
        2,
        "a ride out, the act from its seat, a ride on: {r:#?}"
    );
    let (out_ride, on) = (&r[0], &r[1]);
    assert_eq!(out_ride["boards"], true, "{out_ride}");
    assert_eq!(out_ride["stays"], true, "{out_ride}");
    let end = cell(out_ride["medium"].as_array().unwrap().last().unwrap());
    assert_eq!(
        cell(&out_ride["leave"]),
        end,
        "a ride that stays ends at its last medium cell"
    );
    // The bay sits 64 courses up: the pool's surface course is y = 67.
    assert_eq!(end[1], 67, "on the water: {end:?}");
    // The rider's eye at the end cell is within the block reach of the barrel.
    let eye = [
        end[0] as f64 + 0.5,
        67.0 + 0.523 + 0.1875 - 0.6 + 1.62,
        end[2] as f64 + 0.5,
    ];
    let gap = |e: f64, lo: f64| (lo - e).max(e - (lo + 1.0)).max(0.0);
    let d =
        (gap(eye[0], 11.0).powi(2) + gap(eye[1], 68.0).powi(2) + gap(eye[2], 4.0).powi(2)).sqrt();
    assert!(
        d <= 4.5,
        "the barrel is {d} from the rider's eye at {end:?}"
    );
    assert_eq!(on["boards"], false, "the ride on starts aboard: {on}");
    assert_eq!(on["stays"], false, "{on}");
    assert_eq!(cell(&on["medium"].as_array().unwrap()[0]), end, "{on}");
    assert!(xs(&on["leave"]) > 16, "left on the east shore: {on}");
}

/// The same crate with the boat's bounds short of every cell from which a
/// rider reaches it: the ride reaches nothing, and the leg is `DW0311`.
#[test]
fn a_crate_no_ride_reaches_is_refused() {
    let bay = Bay {
        wide: true,
        objectives: Some(ONE_CRATE),
        ..Bay::default()
    };
    let short = r#"{ "id": "vehicle/gull", "entity": "minecraft:oak_boat",
        "at": { "anchor": "anchor/near-dock" },
        "bounds": { "anchor": "anchor/near-dock", "extent": [0, 3, 2] } },
       { "id": "vehicle/tern", "entity": "minecraft:oak_boat",
        "at": { "anchor": "anchor/near-dock-two" },
        "bounds": { "anchor": "anchor/near-dock-two", "extent": [0, 3, 1] } }"#;
    let (code, msg) = refused("crate-out-of-reach", bay, short);
    assert_eq!(code, "DW0311", "{msg}");
    assert!(msg.contains("declared ridden on `vehicle/gull`"), "{msg}");
}

/// Two crates in a row: the second ride goes on from where the first left
/// the party, aboard, with no boarding between them.
#[test]
fn two_crates_are_opened_without_leaving_the_boat() {
    let bay = Bay {
        wide: true,
        second_crate: true,
        objectives: Some(
            r#",
          { "type": "collect", "id": "obj/crate", "item": "minecraft:cod", "count": 1,
            "anchor": "anchor/crate-alongside", "holder": "anchor/crate",
            "after": ["obj/talk"], "ride": "vehicle/gull" },
          { "type": "collect", "id": "obj/crate-two", "item": "minecraft:salmon", "count": 1,
            "anchor": "anchor/crate-two-alongside", "holder": "anchor/crate-two",
            "after": ["obj/crate"], "ride": "vehicle/gull" },
          { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/far-shore",
            "radius": 1, "after": ["obj/crate-two"] }"#,
        ),
        ..Bay::default()
    };
    let out = build("two-crates", bay, &skiff());
    let r = rides(&out);
    assert_eq!(r.len(), 3, "{r:#?}");
    let flags: Vec<(bool, bool)> = r
        .iter()
        .map(|x| {
            (
                x["boards"].as_bool().unwrap(),
                x["stays"].as_bool().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        flags,
        [(true, true), (false, true), (false, false)],
        "{r:#?}"
    );
    for w in r.windows(2) {
        assert_eq!(
            cell(w[1]["medium"].as_array().unwrap().first().unwrap()),
            cell(w[0]["medium"].as_array().unwrap().last().unwrap()),
            "each ride goes on from where the last one ended: {r:#?}"
        );
    }
}

/// A ride step is a path act: its arrival takes what precedes the objective
/// it carries the party toward, as a trigger step's does, never the world as
/// it loaded. Without it every proof that sweeps arrivals — the recovery
/// stake's way home (`DW0525`) among them — read a ride's arrival as a world
/// where no beat had fired, every door the path opened shut again.
#[test]
fn a_ride_arrives_after_what_precedes_its_objective() {
    let (dir, lib) = write(
        "ride-ancestors",
        Bay {
            ride: Some("vehicle/gull"),
            ..Bay::default()
        },
        &boat("gull", "dock", [8, 3, 4]),
        "",
    );
    let c = campaign(&dir);
    let prefabs = PrefabRegistry::load_dir(&lib).unwrap();
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let mut s = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                s.insert(
                    t.structure_file.clone(),
                    std::fs::read(lib.join(&t.structure_file)).unwrap(),
                );
            }
        }
    }
    let world = delvec::compiler::nav::World::from_plan(&plan, &s);
    let linked = delvec::compiler::nav::with_links_taken(&plan, &prefabs, &world)
        .expect("the route proof takes its ride")
        .expect("a ride is spliced");
    let steps = &linked.critical_path;
    let at = |f: &dyn Fn(&delvec::compiler::plan::Step) -> bool| {
        steps.iter().position(f).expect("the step is on the path")
    };
    let talk = at(&|s| s.objective() == Some("obj/talk"));
    let ride = at(&|s| matches!(s, delvec::compiler::plan::Step::Ride { .. }));
    let exit = at(&|s| s.objective() == Some("obj/exit"));
    assert!(talk < ride && ride < exit, "{talk} < {ride} < {exit}");
    assert!(
        linked.gate_fired_before(talk, ride),
        "the talk (step {talk}) precedes the ride (step {ride}) toward the exit"
    );
    assert!(linked.gate_fired_before(talk, exit));
}

/// A world-edits batch judges the vehicle class before the route's verdict,
/// as the final build does: a leg declared ridden on a boat seated on the
/// floor, walled off by a course two high, is refused as the boat out of its
/// medium (`DW1097`) at the batch's door, never as the walk it is not
/// (`DW0311`). The batch only stamps one cell of the shell; red on a batch
/// door that asks the route proof alone.
#[test]
fn a_batch_names_an_unseated_ride_by_its_vehicle_not_as_a_walk() {
    let boat = r#"{ "id": "vehicle/v", "entity": "minecraft:oak_boat",
        "at": { "anchor": "anchor/floor" },
        "bounds": { "anchor": "anchor/room", "extent": [9, 3, 3] } }"#;
    let bay = Bay {
        step: Some(2),
        ride: Some("vehicle/v"),
        ..Bay::default()
    };
    // The final build's own door, with no batch.
    let (code, msg) = refused("unseated-final", bay, boat);
    assert_eq!(code, "DW1097", "{msg}");

    let (dir, lib) = write("unseated-batch", bay, boat, "");
    let doc = serde_json::json!({
        "dsl_version": DSL_VERSION,
        "campaign_id": "hello-world",
        "stage": "world-edits",
        "content": { "batches": [{
            "id": "batch/stamp",
            "area": "area/keep",
            "edits": [
                {
                    "verb": "select",
                    "name": "region/corner",
                    "shape": {
                        "kind": "box",
                        "frame": { "kind": "piece-local", "piece": 0, "prefab": format!("prefab/{ROOM}") },
                        "min": [20, 0, 0],
                        "max": [20, 0, 0]
                    }
                },
                {
                    "verb": "fill",
                    "region": "region/corner",
                    "recipe": { "blocks": [{ "block": "minecraft:stone", "weight": 1.0 }], "scale": 1.0 }
                }
            ]
        }]}
    });
    std::fs::write(
        dir.join("world-edits.json"),
        serde_json::to_string_pretty(&doc).unwrap(),
    )
    .unwrap();
    let (code, msg) = match try_build(&dir, &lib) {
        Ok(_) => panic!("expected the build to be refused"),
        Err(BuildFailure::Diagnostic { code, message }) => (code.to_string(), message),
        Err(e) => panic!("expected a diagnostic, got {e:?}"),
    };
    assert!(
        msg.contains("after world-edits batch `batch/stamp`"),
        "the batch's door is the one that refuses: {code}: {msg}"
    );
    assert_eq!(code, "DW1097", "{msg}");
}

/// Two crates, the second on a limit: in the story world that lets it lapse,
/// the lapse comes right after the first crate is opened from the seat. A
/// lapse walks nowhere (spec-0133 §8), so the party waits where it sits,
/// aboard, and the walk on starts where a ride on leaves it — never a walk
/// to the barrel the first crate's act named, which a rider acting from the
/// seat never stood in. Red on a route proof that rode the party off the
/// boat and walked it to the barrel for the wait.
#[test]
fn a_lapse_after_an_act_from_the_seat_is_waited_aboard() {
    let bay = Bay {
        wide: true,
        second_crate: true,
        objectives: Some(
            r#",
          { "type": "collect", "id": "obj/crate", "item": "minecraft:cod", "count": 1,
            "anchor": "anchor/crate-alongside", "holder": "anchor/crate",
            "after": ["obj/talk"], "ride": "vehicle/gull" },
          { "type": "collect", "id": "obj/crate-two", "item": "minecraft:salmon", "count": 1,
            "anchor": "anchor/crate-two-alongside", "holder": "anchor/crate-two",
            "after": ["obj/crate"], "ride": "vehicle/gull", "limit": { "ticks": 2400 } },
          { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/far-shore",
            "radius": 1, "after": ["obj/crate-two"] }"#,
        ),
        ..Bay::default()
    };
    let out = build("lapse-aboard", bay, &skiff());
    // The world that lets the second crate lapse is exported as a world path
    // whose step for it is an `expire`.
    let lapsed: Vec<Vec<serde_json::Value>> = out
        .iter()
        .filter(|(k, _)| k.ends_with(".json"))
        .filter_map(|(_, v)| serde_json::from_slice::<serde_json::Value>(v).ok())
        .filter_map(|v| v["steps"].as_array().cloned())
        .filter(|steps| steps.iter().any(|s| s["action"] == "expire"))
        .collect();
    assert!(
        !lapsed.is_empty(),
        "a world path lets the second crate lapse"
    );
    for steps in &lapsed {
        let i = steps
            .iter()
            .position(|s| s["action"] == "expire")
            .expect("the lapse");
        let ride = &steps[i - 2];
        assert_eq!(ride["action"], "ride", "{steps:#?}");
        assert_eq!(ride["stays"], true, "{ride}");
        let seat = cell(ride["medium"].as_array().unwrap().last().unwrap());
        assert_eq!(
            cell(&steps[i]["pos"]),
            seat,
            "waited where it sits: {}",
            steps[i]
        );
        assert_eq!(cell(&steps[i]["stand"]), seat, "{}", steps[i]);
        // The export names the seat (format 11), so the harness waits aboard
        // rather than walk from a vehicle — red on an export that left the
        // bot to guess, which refused the wait as a walk with no ride off.
        assert_eq!(steps[i]["aboard"], "vehicle/gull", "{}", steps[i]);
        let on = &steps[i + 1];
        assert_eq!(
            on["action"], "ride",
            "a ride on leaves the boat: {steps:#?}"
        );
        assert_eq!(on["boards"], false, "{on}");
        assert_eq!(cell(&on["medium"].as_array().unwrap()[0]), seat, "{on}");
    }
}

/// Two crates on one clock — each a window of the same length from its own
/// activation, both opened by the same talk — and the world that lets the
/// first lapse. The lapse is waited until its window runs out, and the second
/// crate's window, started at the same tick, runs out with it: waited in the
/// first crate's place in the order, the lapse leaves the second barrel shut
/// to the path that opens it next (the town's cargo worlds: crate 1 let lapse,
/// then crate 2 never completed). The lapse goes after the second crate, and
/// the exported path waits it there, aboard, after the act from the seat.
/// Red on an order that kept the lapse where the all world's crate stands.
#[test]
fn a_lapse_waits_after_every_crate_whose_clock_runs_beside_it() {
    let bay = Bay {
        wide: true,
        second_crate: true,
        objectives: Some(
            r#",
          { "type": "collect", "id": "obj/crate", "item": "minecraft:cod", "count": 1,
            "anchor": "anchor/crate-alongside", "holder": "anchor/crate",
            "after": ["obj/talk"], "ride": "vehicle/gull", "limit": { "ticks": 2400 } },
          { "type": "collect", "id": "obj/crate-two", "item": "minecraft:salmon", "count": 1,
            "anchor": "anchor/crate-two-alongside", "holder": "anchor/crate-two",
            "after": ["obj/talk"], "ride": "vehicle/gull", "limit": { "ticks": 2400 } },
          { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/far-shore",
            "radius": 1, "after": ["obj/crate", "obj/crate-two"] }"#,
        ),
        ..Bay::default()
    };
    let out = build("lapse-clock", bay, &skiff());
    let worlds: Vec<Vec<serde_json::Value>> = out
        .iter()
        .filter(|(k, _)| k.ends_with(".json"))
        .filter_map(|(_, v)| serde_json::from_slice::<serde_json::Value>(v).ok())
        .filter_map(|v| v["steps"].as_array().cloned())
        .filter(|steps| {
            steps
                .iter()
                .any(|s| s["action"] == "expire" && s["objective"] == "obj/crate")
        })
        .collect();
    assert!(
        !worlds.is_empty(),
        "a world path lets the first crate lapse"
    );
    for steps in &worlds {
        let at = |action: &str, obj: &str| {
            steps
                .iter()
                .position(|s| s["action"] == action && s["objective"] == obj)
        };
        let lapse = at("expire", "obj/crate").expect("the lapse");
        let second = at("collect", "obj/crate-two").expect("the second crate");
        assert!(
            second < lapse,
            "the second crate's clock runs through the lapse, so it is opened first: {steps:#?}"
        );
        assert_eq!(
            steps[lapse]["aboard"], "vehicle/gull",
            "waited aboard after the act from the seat: {}",
            steps[lapse]
        );
    }
}

// ---------------------------------------------------------------------------
// spec-0135 §13: a body that rides, a vehicle per player, a board of places
// ---------------------------------------------------------------------------

/// A row of pigs on the west floor, `[2, 4, 2]` east by `spacing`, one per
/// player present when the talk seats them.
fn piglets(spacing: [i32; 3]) -> String {
    format!(
        r#"{{ "id": "vehicle/piglets", "entity": "minecraft:pig",
            "at": {{ "anchor": "anchor/floor", "offset": [-1, 0, -1] }},
            "bounds": {{ "anchor": "anchor/floor", "extent": [3, 2, 3] }},
            "count": {{ "base": 1, "per_player": 1 }},
            "spacing": [{}, {}, {}], "seated": "by-story" }}"#,
        spacing[0], spacing[1], spacing[2]
    )
}

const SEAT_PIGLETS: &str = r#"{ "type": "seat-vehicle", "vehicle": "vehicle/piglets" }"#;

/// A count per player: the seat measures the party and seats one body per
/// player, each at its own mark and with its own re-seat, and the PackTest
/// steps the four party sizes. Perturbation: a spacing that carries the last
/// body into the pool is `DW1097` naming that body's seat — every body is
/// judged, not the first alone.
#[test]
fn a_vehicle_seats_one_body_per_player_present() {
    let (dir, lib) = write("piglets", Bay::default(), &piglets([1, 0, 0]), SEAT_PIGLETS);
    let out = try_build(&dir, &lib).expect("the row of pigs builds");
    let seat = function(&out, "vehicle_seat_piglets");
    assert!(
        seat.contains(
            "execute store result score #pn_v_piglets dw.sys if entity @a[tag=!dw_cutscene]"
        ) && seat.contains("function hello-world:vehicle_seat_piglets_party"),
        "{seat}"
    );
    let party = function(&out, "vehicle_seat_piglets_party");
    assert!(
        party.contains("function hello-world:vehicle_seat_piglets_b0\n"),
        "{party}"
    );
    for (k, t) in [(1, 2), (2, 3), (3, 4)] {
        assert!(
            party.contains(&format!(
                "execute if score #pn_v_piglets dw.sys matches {t}.. run function hello-world:vehicle_seat_piglets_b{k}"
            )),
            "body {k} needs {t} players: {party}"
        );
    }
    for k in 0..4 {
        let body = function(&out, &format!("vehicle_seat_piglets_b{k}"));
        assert!(
            body.contains(&format!("summon minecraft:pig {}.5 68.0 2.5", 2 + k))
                && body
                    .contains(r#"Tags:["dw_vehicle","dw_vehicle_piglets","dw_vehicle_piglets_b"#),
            "body {k}: {body}"
        );
    }
    let reseat = function(&out, "vehicle_reseat_piglets_b3");
    assert!(
        reseat.contains(
            "execute unless entity @e[tag=dw_vehicle_piglets_b3] if score #pn_v_piglets dw.sys matches 4.. run function hello-world:vehicle_seat_piglets_b3"
        ),
        "a missing body is put back only when its party had it: {reseat}"
    );
    let t = out
        .iter()
        .find(|(k, _)| k.ends_with("/test/vehicle_piglets_party.mcfunction"))
        .map(|(_, v)| String::from_utf8(v.clone()).unwrap())
        .expect("the per-party template");
    for n in 1..=4 {
        assert!(
            t.contains(&format!("scoreboard players set #pn_v_piglets dw.sys {n}")),
            "{t}"
        );
        assert!(t.contains(&format!("matches {n}\n")), "{t}");
    }
    // The perturbation: a step of two puts body 2 at x = 6, over the pool.
    let (dir, lib) = write(
        "piglets-wet",
        Bay::default(),
        &piglets([2, 0, 0]),
        SEAT_PIGLETS,
    );
    match try_build(&dir, &lib) {
        Err(BuildFailure::Diagnostic { code, message }) => {
            assert_eq!(code.to_string(), "DW1097", "{message}");
            assert!(message.contains("[6, 68, 2]"), "{message}");
        }
        other => panic!(
            "expected DW1097 on the wet body, got {:?}",
            other.map(|_| ())
        ),
    }
}

/// A rider is summoned as its mount's passenger, the wave's tag on the rider
/// alone: the kill count, the census and the muster read riders, and the
/// mounts leave after them by the wave's one removal.
#[test]
fn a_wave_body_rides_its_mount_and_only_the_rider_is_counted() {
    let bay = Bay {
        more: r#""waves": [ { "id": "wave/horsemen", "anchor": "anchor/floor",
            "mobs": [ { "entity": "minecraft:zombie", "count": 2,
                        "equipment": { "main_hand": "minecraft:iron_spear" },
                        "mount": { "entity": "minecraft:zombie_horse" } } ] } ]"#,
        ..Bay::default()
    };
    let talk = r#"{ "type": "spawn-wave", "wave": "wave/horsemen" },
                  { "type": "despawn-wave", "wave": "wave/horsemen" }"#;
    let (dir, lib) = write("horsemen", bay, "", talk);
    let out = try_build(&dir, &lib).expect("the horsemen build");
    let spawn = function(&out, "spawn_horsemen");
    let summons: Vec<&str> = spawn.lines().filter(|l| l.contains("summon")).collect();
    assert_eq!(summons.len(), 2, "{spawn}");
    for l in &summons {
        assert!(
            l.starts_with("summon minecraft:zombie_horse ")
                && l.contains(r#"Tags:["dw_mount","dw_wmount_horsemen"],PersistenceRequired:1b,Passengers:[{id:"minecraft:zombie",Tags:["dw_wave_horsemen"]"#)
                && l.contains("iron_spear"),
            "{l}"
        );
        let mount_part = &l[..l.find("Passengers").unwrap()];
        assert!(
            !mount_part.contains("dw_wave_horsemen"),
            "the mount is no member: {l}"
        );
    }
    let census = function(&out, "wave_census_horsemen");
    assert!(census.contains("@e[tag=dw_wave_horsemen]") && !census.contains("dw_wmount"));
    let talked = out
        .iter()
        .filter(|(k, _)| k.contains("/function/") && !k.contains("/test/"))
        .map(|(_, v)| String::from_utf8_lossy(v).to_string())
        .find(|f| f.contains("function hello-world:spawn_horsemen"))
        .expect("the talk's bundle");
    let rider = talked
        .find("dw_wave_horsemen")
        .expect("the riders' removal");
    let mount = talked
        .find("dw_wmount_horsemen")
        .expect("the mounts' removal");
    assert!(rider < mount, "the riders leave first: {talked}");
}

fn racer(more: &str) -> String {
    format!(
        r#""actors": [ {{ "id": "actor/racer", "entity": "minecraft:villager", "name": "Ulla",
            "anchor": "anchor/floor"{more} }} ]"#
    )
}

/// The horse actor `actor/racer` is seated on, standing on the racer's mark.
fn racer_horse(rider: &str) -> String {
    format!(
        r#"{{ "id": "vehicle/racer-horse", "entity": "minecraft:horse",
            "at": {{ "anchor": "anchor/floor" }},
            "bounds": {{ "anchor": "anchor/floor", "extent": [3, 2, 3] }}{rider} }}"#
    )
}

const SEATS_RACER: &str = r#", "rider": "actor/racer""#;

/// A seated actor rides a vehicle of `vehicles[]` (spec-0135 §13): put
/// aboard by `ride` when the vehicle is seated and when the actor is
/// spawned, walked by its vehicle — on its own feet only while the vehicle
/// is not standing — and its twin takes the puppet's seat with the mount's
/// AI given back; the clearance proof reads the stacked box. Perturbation: a
/// lid three blocks over the floor clears a villager (1.95) and not a
/// villager on a horse (1.6 + 1.95), so the `rider` alone turns the proof
/// red.
#[test]
fn an_actor_seated_on_a_vehicle_rides_and_walks_it() {
    let more: &'static str = Box::leak(racer("").into_boxed_str());
    let talk = r#"{ "type": "spawn-actor", "actor": "actor/racer",
                    "happening": { "subject": "actor/racer", "verb": "arrives", "text": "Ulla rides in." } },
                  { "type": "move-actor", "actor": "actor/racer", "to": { "anchor": "anchor/rail-end" }, "speed": 0.1,
                    "happening": { "subject": "actor/racer", "verb": "departs", "text": "Ulla rides off." } },
                  { "type": "unleash-actor", "actor": "actor/racer" }"#;
    let bay = Bay {
        more,
        ..Bay::default()
    };
    let (dir, lib) = write("racer", bay, &racer_horse(SEATS_RACER), talk);
    let out = try_build(&dir, &lib).expect("the seated actor builds");
    let aboard = "execute if entity @e[tag=dw_vehicle_racer_horse] as @e[tag=dw_pup_racer,limit=1] run ride @s mount @e[tag=dw_vehicle_racer_horse,limit=1]";
    let spawn = function(&out, "spawn_actor_racer");
    let summon = spawn.find("summon minecraft:villager").expect("the puppet");
    let ride = spawn.find(aboard).unwrap_or_else(|| panic!("{spawn}"));
    assert!(
        summon < ride,
        "the puppet is put aboard once it stands: {spawn}"
    );
    let seat = function(&out, "vehicle_seat_racer_horse");
    assert!(
        seat.contains(aboard),
        "the seat puts the actor aboard: {seat}"
    );
    let tick = out
        .iter()
        .find(|(k, _)| k.contains("/function/ma_tick_racer"))
        .map(|(_, v)| String::from_utf8(v.clone()).unwrap())
        .expect("the walk's driver");
    assert!(
        tick.contains(
            "as @e[tag=dw_pup_racer] on vehicle if entity @s[tag=dw_vehicle_racer_horse] run tp @s "
        ) && tick
            .contains("unless entity @e[tag=dw_vehicle_racer_horse] run tp @e[tag=dw_pup_racer] "),
        "the walk moves the vehicle, and the rider only afoot: {tick}"
    );
    let unleash = function(&out, "unleash_racer");
    let twin = "@e[tag=dw_actor_racer,tag=!dw_pup_racer,type=!minecraft:interaction,limit=1]";
    assert!(
        unleash.contains(&format!(
            "execute if entity @e[tag=dw_vehicle_racer_horse] as {twin} run ride @s mount @e[tag=dw_vehicle_racer_horse,limit=1]"
        )) && unleash.contains(&format!(
            "execute as {twin} on vehicle run data merge entity @s[tag=dw_vehicle_racer_horse] {{NoAI:0b}}"
        )),
        "the twin takes the seat, the mount its AI: {unleash}"
    );
    assert!(
        !out.keys().any(|k| k.contains("dw_mount")) && !spawn.contains("Passengers"),
        "a seated actor is no wave mount: {spawn}"
    );
    // The lid over the racer's mark, three cells of headroom.
    let lidded = |tag: &str, rider: &str| {
        let bay = Bay {
            more,
            lid: Some([3, 7, 3]),
            ..Bay::default()
        };
        let talk = r#"{ "type": "spawn-actor", "actor": "actor/racer",
                        "happening": { "subject": "actor/racer", "verb": "arrives", "text": "Ulla rides in." } }"#;
        let (dir, lib) = write(tag, bay, &racer_horse(rider), talk);
        try_build(&dir, &lib)
    };
    assert!(
        lidded("racer-lid-afoot", "").is_ok(),
        "a villager and a horse each fit three blocks"
    );
    match lidded("racer-lid-seated", SEATS_RACER) {
        Err(BuildFailure::Diagnostic { code, message }) => {
            assert_eq!(code.to_string(), "DW0450", "{message}");
        }
        other => panic!(
            "expected DW0450 for the stacked box, got {:?}",
            other.map(|_| ())
        ),
    }
}

/// A board: its objective headed by its title at setup, shown and hidden in
/// the sidebar, a player placed by the function run as them and a named body
/// under its own holder and name, each place written once.
#[test]
fn a_board_lists_the_places_the_story_writes() {
    let more: &'static str = Box::leak(
        format!(
            r#"{}, "boards": [ {{ "id": "board/race", "title": "The Pig Race" }} ]"#,
            racer("")
        )
        .into_boxed_str(),
    );
    let talk = r#"{ "type": "show-board", "board": "board/race" },
                  { "type": "place-on-board", "board": "board/race", "audience": "actor" },
                  { "type": "place-on-board", "board": "board/race", "audience": { "actor": "actor/racer" } },
                  { "type": "hide-board", "board": "board/race" }"#;
    let bay = Bay {
        more,
        ..Bay::default()
    };
    let (dir, lib) = write("board", bay, "", talk);
    let out = try_build(&dir, &lib).expect("the board builds");
    let setup = out
        .iter()
        .filter(|(k, _)| k.ends_with("/function/setup.mcfunction"))
        .map(|(_, v)| String::from_utf8(v.clone()).unwrap())
        .next()
        .expect("setup");
    assert!(
        setup.contains("scoreboard objectives add dw.bd.race dummy"),
        "{setup}"
    );
    let player = function(&out, "board_place_race");
    let row = function(&out, "board_row_race_racer");
    // Rows: four players and the one named body.
    for (f, h) in [(&player, "@s"), (&row, "dw.r.racer")] {
        assert!(
            f.starts_with(&format!(
                "execute if score {h} dw.bd.race matches -2147483648.. run return 0\n"
            )),
            "a holder already placed keeps its place: {f}"
        );
        assert!(
            f.contains(&format!("scoreboard players set {h} dw.bd.race 6")),
            "{f}"
        );
        assert!(
            f.contains("matches 5 run scoreboard players display numberformat"),
            "{f}"
        );
        assert!(!f.contains("matches 6 run"), "{f}");
    }
    assert!(
        row.contains("scoreboard players display name dw.r.racer dw.bd.race"),
        "{row}"
    );
    assert!(
        !player.contains("display name"),
        "a player's row shows the player's name: {player}"
    );
    let talked = out
        .iter()
        .filter(|(k, _)| k.contains("/function/") && !k.contains("/test/"))
        .map(|(_, v)| String::from_utf8_lossy(v).to_string())
        .find(|f| f.contains("setdisplay sidebar dw.bd.race"))
        .expect("the talk's bundle");
    assert!(
        talked.contains("execute as @s run function hello-world:board_place_race"),
        "{talked}"
    );
    assert!(
        talked.contains("function hello-world:board_row_race_racer"),
        "{talked}"
    );
    assert!(
        talked.contains("scoreboard objectives setdisplay sidebar\n"),
        "{talked}"
    );
}

/// The re-seat template judges a body nobody rides. A ridden body is in use
/// and its re-seat leaves it where it is, by design; the seat puts a declared
/// `rider` aboard wherever that actor stands, and another test of the suite
/// may have left her standing. So `_reseats` sets every passenger down after
/// the seat and before it moves the body: whether the rider was in the world
/// when the template ran no longer decides its verdict.
#[test]
fn the_reseat_template_moves_a_body_nobody_rides() {
    let more: &'static str = Box::leak(racer("").into_boxed_str());
    let talk = r#"{ "type": "spawn-actor", "actor": "actor/racer",
                    "happening": { "subject": "actor/racer", "verb": "arrives", "text": "Ulla rides in." } }"#;
    let bay = Bay {
        more,
        ..Bay::default()
    };
    let (dir, lib) = write("racer-reseats", bay, &racer_horse(SEATS_RACER), talk);
    let out = try_build(&dir, &lib).expect("the seated actor builds");
    let t = text(
        &out,
        "packtest-datapack/data/hello-world/test/vehicle_racer_horse_reseats.mcfunction",
    );
    let at = |needle: &str| {
        t.find(needle)
            .unwrap_or_else(|| panic!("`{needle}` is not in the template: {t}"))
    };
    let seat = at("function hello-world:vehicle_seat_racer_horse\n");
    let down = at("execute as @e[tag=dw_vehicle_racer_horse] on passengers run ride @s dismount\n");
    let moved = at("tp @e[tag=dw_vehicle_racer_horse] ");
    let reseat = at("function hello-world:vehicle_reseat_racer_horse\n");
    assert!(
        seat < down && down < moved && moved < reseat,
        "the body is set riderless after its seat and before it is moved: {t}"
    );
}

// ---------------------------------------------------------------------------
// spec-0139: a race's course
// ---------------------------------------------------------------------------

/// A race over the bay: `checkpoints` are anchors, each a one-column box.
fn race(checkpoints: &[&str], more: &str) -> String {
    let cps: Vec<String> = checkpoints
        .iter()
        .map(|a| format!(r#"{{ "at": {{ "anchor": "anchor/{a}" }}, "extent": [0, 1, 0] }}"#))
        .collect();
    format!(
        r#""races": [ {{ "id": "race/pig-race", "checkpoints": [ {} ],
            "missed": "You missed a checkpoint — go back."{more} }} ]"#,
        cps.join(", ")
    )
}

const START_RACE: &str = r#"{ "type": "start-race", "race": "race/pig-race" }"#;

/// The bay a course is ridden over: the bridge across the pool, a pig, a
/// carrot on a stick in the kit, and the exit's leg declared ridden and run
/// as the race's course.
fn course_bay(race: String) -> Bay {
    Bay {
        bridge: true,
        ride: Some("vehicle/race-pig"),
        carrot: true,
        gate: r#", "course": "race/pig-race""#,
        more: Box::leak(race.into_boxed_str()),
        ..Bay::default()
    }
}

fn course_build(tag: &str, bay: Bay, talk: &str) -> Result<BuildOutput, (String, String)> {
    let (dir, lib) = write(tag, bay, pig(), talk);
    // A refusal the plan itself makes (`DW1119`) is read off the plan.
    let c = campaign(&dir);
    let prefabs = PrefabRegistry::load_dir(&lib).unwrap();
    if let Err(e) = Plan::build(&c, &prefabs) {
        return Err((e.failure.code.to_string(), e.failure.message));
    }
    match try_build(&dir, &lib) {
        Ok(out) => Ok(out),
        Err(BuildFailure::Diagnostic { code, message }) => Err((code.to_string(), message)),
        Err(e) => panic!("expected a build or a diagnostic, got {e:?}"),
    }
}

/// Where in a ride's medium the first cell of column `[x, z]` stands.
fn passes(medium: &[serde_json::Value], x: i64, z: i64) -> Option<usize> {
    medium.iter().position(|c| {
        let c = cell(c);
        c[0] == x && c[2] == z
    })
}

/// spec-0139 criterion 4: a leg declared a race's course is one ride whose
/// medium passes a cell of every checkpoint in the order the race writes
/// them, whatever the shorter way is; the race is emitted with its body, its
/// start and its end; and the objective waits for the race to be over.
/// Perturbation: the same two checkpoints written the other way round are
/// passed the other way round.
#[test]
fn a_course_is_ridden_through_its_checkpoints_in_the_order_written() {
    let out = course_build(
        "course-in-order",
        course_bay(race(&["cp-bridge", "cp-east"], "")),
        START_RACE,
    )
    .unwrap_or_else(|(c, m)| panic!("{c}: {m}"));
    let r = rides(&out);
    assert_eq!(r.len(), 1, "one ride carries the course: {r:#?}");
    let medium = r[0]["medium"].as_array().unwrap();
    let (a, b) = (
        passes(medium, 9, 7).expect("the bridge checkpoint"),
        passes(medium, 14, 7).expect("the east checkpoint"),
    );
    assert!(a < b, "the bridge checkpoint is passed first: {medium:?}");

    let back = course_build(
        "course-reversed",
        course_bay(race(&["cp-east", "cp-bridge"], "")),
        START_RACE,
    )
    .unwrap_or_else(|(c, m)| panic!("{c}: {m}"));
    let r = rides(&back);
    let medium = r[0]["medium"].as_array().unwrap();
    let east = passes(medium, 14, 7).expect("the east checkpoint");
    let bridge = medium
        .iter()
        .rposition(|c| cell(c)[0] == 9 && cell(c)[2] == 7)
        .expect("the bridge checkpoint");
    assert!(
        east < bridge,
        "written the other way round, the east checkpoint is passed first and the ride comes \
         back for the bridge: {medium:?}"
    );

    let body = function(&out, "race_pig_race_body");
    let line = |starts: &str| {
        body.lines()
            .find(|l| l.starts_with(starts))
            .unwrap_or_else(|| panic!("no line `{starts}…` in the body: {body}"))
    };
    let credit = line("execute if score @s dw.rn.pig_race matches 0 if entity @s[x=9,dx=0,");
    assert!(
        credit.ends_with(",z=7,dz=0] run return run function hello-world:race_pig_race_pass"),
        "{credit}"
    );
    let second = line("execute if score @s dw.rn.pig_race matches 1 if entity @s[x=14,dx=0,");
    assert!(second.ends_with("race_pig_race_pass"), "{second}");
    let missed = line(
        "execute if score @s dw.rn.pig_race matches ..0 if entity \
         @s[type=minecraft:player,x=14,dx=0,",
    );
    assert!(
        missed.contains(",z=7,dz=0] run return run title @s actionbar "),
        "a player in the second checkpoint with the first unpassed reads `missed`: {missed}"
    );
    assert_eq!(
        body.lines().filter(|l| l.contains("actionbar")).count(),
        1,
        "the first checkpoint is never further on than a racer's next: {body}"
    );
    assert!(function(&out, "race_pig_race_pass").contains(
        "execute if score @s dw.rc.pig_race matches 2.. run return run function \
             hello-world:race_pig_race_finish"
    ));
    let finish = function(&out, "race_pig_race_finish");
    assert!(
        finish.contains(
            "execute unless entity @a[tag=dw_rcg_pig_race] run function \
             hello-world:race_pig_race_end"
        ),
        "{finish}"
    );
    assert!(
        function(&out, "race_pig_race_end")
            .contains("scoreboard players set #rfin_pig_race dw.sys 1")
    );
    let tick = function(&out, "tick");
    assert!(
        tick.contains(
            "execute if score #ron_pig_race dw.sys matches 1 as @a run function \
             hello-world:race_pig_race_body"
        ),
        "{tick}"
    );
    // The objective completes only once the race is over, and is told before.
    let guarded: Vec<&str> = tick
        .lines()
        .filter(|l| l.contains("#rfin_pig_race dw.sys matches 1"))
        .collect();
    assert!(
        !guarded.is_empty() && guarded.iter().all(|l| l.contains("complete_o_exit")),
        "the exit's completion waits on the race, and nothing else does: {guarded:#?}"
    );
}

/// spec-0139 criterion 5: the grace, the board and the listed actors are in
/// the finish — the first finish schedules the end, every finish places its
/// racer, and the race is over when no player and no listed actor is short.
#[test]
fn a_finish_places_its_racer_and_starts_the_grace() {
    let more = format!(
        "{}, {}, {}",
        race(
            &["cp-bridge", "cp-east"],
            r#", "laps": 2, "grace_ticks": 600, "board": "board/race",
               "racers": ["actor/racer"], "passed": "Checkpoint""#
        ),
        racer(""),
        r#""boards": [ { "id": "board/race", "title": "The Race" } ]"#
    );
    let out = course_build("course-finish", course_bay(more), START_RACE)
        .unwrap_or_else(|(c, m)| panic!("{c}: {m}"));
    let finish = function(&out, "race_pig_race_finish");
    for line in [
        "execute if entity @s[type=minecraft:player] run function hello-world:board_place_race",
        "execute if entity @s[tag=dw_actor_racer,type=!minecraft:interaction] run function \
         hello-world:board_row_race_racer",
        "execute unless score #r1st_pig_race dw.sys matches 1 run schedule function \
         hello-world:race_pig_race_end 600t replace",
        "execute unless entity @a[tag=dw_rcg_pig_race] unless entity \
         @e[tag=dw_actor_racer,type=!minecraft:interaction,tag=dw_rcg_pig_race] run function \
         hello-world:race_pig_race_end",
    ] {
        assert!(
            finish.contains(line),
            "`{line}` is not in the finish: {finish}"
        );
    }
    assert!(function(&out, "tick").contains(
        "execute if score #ron_pig_race dw.sys matches 1 as \
             @e[tag=dw_actor_racer,type=!minecraft:interaction] run function \
             hello-world:race_pig_race_body"
    ));
    let pass = function(&out, "race_pig_race_pass");
    assert!(pass.contains("\"text\":\"/4\""), "two laps of two: {pass}");
    assert!(
        function(&out, "race_pig_race_start")
            .contains("schedule clear hello-world:race_pig_race_end")
    );
    // Two laps: the ride passes the bridge, the east, and the bridge again.
    let r = rides(&out);
    let medium = r[0]["medium"].as_array().unwrap();
    let first = passes(medium, 9, 7).unwrap();
    let east = first + passes(&medium[first..], 14, 7).unwrap();
    assert!(
        passes(&medium[east..], 9, 7).is_some(),
        "the second lap comes back to the bridge: {medium:?}"
    );
    for t in ["race_pig_race_in_order", "race_pig_race_skip"] {
        let t = text(
            &out,
            &format!("packtest-datapack/data/hello-world/test/{t}.mcfunction"),
        );
        assert!(t.contains("function hello-world:race_pig_race_body"), "{t}");
    }
}

/// `DW1119`: a course whose race no line starts before its objective, and
/// one whose last line before it is an `end-race`.
#[test]
fn a_course_its_path_never_starts_is_dw1119() {
    let bay = || course_bay(race(&["cp-bridge", "cp-east"], ""));
    let (code, msg) = course_build("course-unstarted", bay(), "").unwrap_err();
    assert_eq!(code, "DW1119", "{msg}");
    assert!(
        msg.contains("no `start-race` of it fires before step"),
        "{msg}"
    );
    let ended = format!(r#"{START_RACE}, {{ "type": "end-race", "race": "race/pig-race" }}"#);
    let (code, msg) = course_build("course-ended", bay(), &ended).unwrap_err();
    assert_eq!(code, "DW1119", "{msg}");
    assert!(msg.contains("an `end-race` fires at step"), "{msg}");
}

/// `DW1118`: a checkpoint whose box holds no cell a body stands in, and two
/// checkpoints that follow one another and share a cell.
#[test]
fn a_checkpoint_nobody_can_be_in_and_two_that_overlap_are_dw1118() {
    let (code, msg) = course_build(
        "course-high",
        course_bay(race(&["cp-bridge", "cp-high"], "")),
        START_RACE,
    )
    .unwrap_err();
    assert_eq!(code, "DW1118", "{msg}");
    assert!(
        msg.contains("checkpoint 2") && msg.contains("holds no cell"),
        "{msg}"
    );
    let (code, msg) = course_build(
        "course-overlap",
        course_bay(race(&["cp-bridge", "cp-bridge"], "")),
        START_RACE,
    )
    .unwrap_err();
    assert_eq!(code, "DW1118", "{msg}");
    assert!(msg.contains("share a cell"), "{msg}");
}

/// A course the vehicle's medium does not reach is the leg's `DW0311`,
/// naming the course: with no bridge, a pig crosses no pool.
#[test]
fn a_course_the_ride_cannot_reach_is_dw0311() {
    let bay = Bay {
        bridge: false,
        ..course_bay(race(&["cp-east"], ""))
    };
    let (code, msg) = course_build("course-unreached", bay, START_RACE).unwrap_err();
    assert_eq!(code, "DW0311", "{msg}");
    assert!(msg.contains("the leg runs a race's course"), "{msg}");
}

/// spec-0139 criterion 6: a `move-actor` `along` a race walks through a cell
/// of each checkpoint before it goes to its mark. Without `along` the same
/// walk goes straight there and passes neither.
#[test]
fn a_walk_along_a_race_passes_every_checkpoint() {
    let walk = |along: &str| {
        format!(
            r#"{START_RACE},
               {{ "type": "spawn-actor", "actor": "actor/racer",
                  "happening": {{ "subject": "actor/racer", "verb": "arrives", "text": "Ulla lines up." }} }},
               {{ "type": "move-actor", "actor": "actor/racer", "to": {{ "anchor": "anchor/floor", "offset": [1, 0, 0] }}{along},
                  "happening": {{ "subject": "actor/racer", "verb": "departs", "text": "Ulla runs." }} }}"#
        )
    };
    let more = || {
        format!(
            "{}, {}",
            race(&["cp-bridge", "cp-east"], r#", "racers": ["actor/racer"]"#),
            racer("")
        )
    };
    let routed = |tag: &str, along: &str| -> String {
        let out = course_build(tag, course_bay(more()), &walk(along))
            .unwrap_or_else(|(c, m)| panic!("{c}: {m}"));
        out.iter()
            .filter(|(k, _)| k.contains("/function/ma_"))
            .map(|(_, v)| String::from_utf8(v.clone()).unwrap())
            .collect::<Vec<_>>()
            .join("\n")
    };
    // The waypoints the driver teleports the body through, as `(x, z)`.
    let waypoints = |driver: &str| -> Vec<(f64, f64)> {
        driver
            .lines()
            .filter_map(|l| {
                let t: Vec<&str> = l.split_whitespace().collect();
                let at = t.iter().position(|w| *w == "tp")?;
                Some((t.get(at + 2)?.parse().ok()?, t.get(at + 4)?.parse().ok()?))
            })
            .collect()
    };
    // In the column of cells `[x, 7]`: the checkpoint's one-column box.
    let within = |w: &[(f64, f64)], x: f64| {
        w.iter()
            .position(|(wx, wz)| (x..x + 1.0).contains(wx) && (7.0..8.0).contains(wz))
    };
    let along = waypoints(&routed("walk-along", r#", "along": "race/pig-race""#));
    let (a, b) = (within(&along, 9.0), within(&along, 14.0));
    assert!(
        a.is_some() && b.is_some() && a < b,
        "the walk passes through the bridge checkpoint's column and then the east one's: \
         {along:?}"
    );
    let straight = waypoints(&routed("walk-straight", ""));
    assert!(
        !straight.is_empty() && straight.iter().all(|(x, _)| *x < 6.0),
        "a walk that follows no course goes one block east and no further: {straight:?}"
    );
}
