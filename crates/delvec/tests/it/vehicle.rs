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
    /// The ledge runs on to `z = 5`, so a goal on it stands clear of the rail.
    ledge_deep: bool,
    /// The vehicle the leg to the exit is declared ridden on.
    ride: Option<&'static str>,
    /// The anchor the second objective is reached at, when not the exit.
    goal: Option<&'static str>,
    /// Extra members of the quests document's content: `"state": […], …`.
    more: &'static str,
    /// A carrot on a stick in every class's kit.
    carrot: bool,
    /// A second class, whose kit carries no carrot on a stick.
    second_class: bool,
    /// A rail cell at `[16, 4, 7]` in a nook walled off from the room: no
    /// walk reaches it.
    nook: bool,
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
        "anchor/nook": { "pos": [16, 4, 7] }
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
          {{ "type": "reach-anchor", "id": "obj/exit", "anchor": "{goal}",
             "radius": 1, "after": ["obj/talk"]{ride} }}
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
    // A riderless mount waits, and boarding gives it its speed back; a cart
    // is pushed the way it faces; a boat is neither.
    let board = function(&out, "vehicle_board");
    for s in ["race_pig", "mare"] {
        let seat = function(&out, &format!("vehicle_seat_{s}"));
        assert!(
            seat.contains(&format!(
                "attribute @e[tag=dw_vehicle_{s},limit=1] minecraft:movement_speed modifier add hello-world:vehicle_wait -1 add_multiplied_total"
            )),
            "{seat}"
        );
        assert!(
            board.contains(&format!(
                "execute on vehicle run attribute @s[tag=dw_vehicle_{s}] minecraft:movement_speed modifier remove hello-world:vehicle_wait"
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
    assert!(!function(&out, "vehicle_seat_gull").contains("vehicle_wait"));
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
