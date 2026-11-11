//! A body carries another (spec-0135): the seat, the medium, the ride leg and
//! the refusals, over a one-room bay the test writes itself.
//!
//! The room is a 21 × 8 × 9 stone box whose floor is the course `y = 3`, so a
//! body walks on `y = 4`. A pool fills `x = 6..=12` across the room, three deep
//! (`y = 1..=3`), its surface the course `y = 3`: the walk cannot cross it, a
//! boat can. The keeper stands west of it and the exit east, so the path's
//! second leg crosses the pool. Variants lay a bridge, move a mark, shrink a
//! bounds box, put a rail line on the floor.

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
const SIZE: [i32; 3] = [21, 8, 9];
const POOL: std::ops::RangeInclusive<i32> = 6..=12;

/// What a variant lays over the bay.
#[derive(Clone, Copy, Default)]
struct Bay {
    /// A stone bridge across the pool along `z = 7`.
    bridge: bool,
    /// A rail line on the west floor at `z = 2`, `x = 1..=5`, its west end a
    /// powered launcher against the wall.
    rails: bool,
}

fn cells(bay: Bay) -> Vec<([i32; 3], &'static str)> {
    let [sx, sy, sz] = SIZE;
    let mut by: BTreeMap<[i32; 3], &'static str> = BTreeMap::new();
    for x in 0..sx {
        for z in 0..sz {
            for y in 0..sy {
                let shell = x == 0 || x == sx - 1 || z == 0 || z == sz - 1;
                let pool = POOL.contains(&x) && !shell && (1..=3).contains(&y);
                if pool {
                    by.insert([x, y, z], "minecraft:water[level=0]");
                } else if y < 4 || y == sy - 1 || shell {
                    by.insert([x, y, z], "minecraft:stone");
                }
            }
        }
    }
    if bay.bridge {
        for x in POOL {
            by.insert([x, 3, 7], "minecraft:stone");
        }
    }
    if bay.rails {
        by.insert(
            [1, 4, 2],
            "minecraft:powered_rail[powered=true,shape=east_west,waterlogged=false]",
        );
        for x in 2..=5 {
            by.insert(
                [x, 4, 2],
                "minecraft:rail[shape=east_west,waterlogged=false]",
            );
        }
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
        "anchor/rail-end": { "pos": [1, 4, 2] },
        "anchor/room": { "pos": [10, 4, 4] }
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
    let vehicles = if vehicles.is_empty() {
        String::new()
    } else {
        format!(r#", "vehicles": [ {vehicles} ]"#)
    };
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
          {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }},
          {{ "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit-west",
             "radius": 1, "after": ["obj/talk"] }}
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

/// Criterion 4: a pool the walk cannot cross is a ride leg by boat, with its
/// medium cells, board and leave; the contract carries it.
#[test]
fn a_pool_the_walk_cannot_cross_is_ridden() {
    let out = build("pool", Bay::default(), &two_boats());
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
    // The piece is placed 64 over its own origin.
    assert_eq!(
        [first[0], first[2]],
        [8, 3],
        "the medium starts at the seat: {first:?}"
    );
    let board = ride["board"].as_array().unwrap()[0].as_i64().unwrap();
    let leave = ride["leave"].as_array().unwrap()[0].as_i64().unwrap();
    assert!(
        board < i64::from(*POOL.start()) && leave > i64::from(*POOL.end()),
        "{ride}"
    );
    assert!(ride["rate"].as_f64().unwrap() > 0.0);
}

/// Criterion 4: the same pool with a bridge is walked; no ride leg.
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

/// Criterion 4's perturbation: bounds that exclude the far shore turn the
/// leg to `DW0311`, with no other change.
#[test]
fn bounds_short_of_the_far_shore_carry_nothing() {
    let short = format!(
        "{}, {}",
        boat("gull", "dock", [1, 3, 4]),
        boat("tern", "dock-two", [1, 3, 4])
    );
    let (code, msg) = refused("short", Bay::default(), &short);
    assert_eq!(code, "DW0311", "{msg}");
}

/// Criterion 5: a two-seat boat on a leg no walk crosses is `DW1099`, and two
/// boats are green (the first test).
#[test]
fn one_boat_cannot_carry_the_party() {
    let (code, msg) = refused("one", Bay::default(), &boat("gull", "dock", [8, 3, 4]));
    assert_eq!(code, "DW1099", "{msg}");
    assert!(msg.contains("seat 2"), "{msg}");
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
        { "id": "vehicle/cart", "entity": "minecraft:minecart",
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
                "schedule function hello_world:vehicle_reseat_{s} 100t replace"
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
    }
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
