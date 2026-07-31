//! spec-0010 end-to-end emission tests: declared time/weather + relight fixtures
//! flow through the real build path (`emit::build`) deterministically.
//!
//! These complement the assembled-light unit tests in `crate::light` (the 10
//! acceptance criteria at the algorithm level). Here we prove the *emission*
//! criteria over a real prefab: relight `setblock`s land in the init path, time
//! and weather commands are emitted, and the whole tree is byte-identical across
//! builds (ADR-0006), and the mitigation gate maps to exit 2.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildFailure, BuildOutput};
use delvewright_compiler::light;
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{AreaLighting, Campaign, Fixture, WorldTime, WorldWeather, parse_campaign};

/// Parse the real hello-world campaign (a single lit prefab area).
fn hello_world() -> Campaign {
    let loaded = load_campaign_dir(&common::hello_world_dir()).unwrap();
    parse_campaign(&loaded.raw).expect("hello-world parses")
}

/// A synthetic gzipped structure `.nbt` for an `[sx,sy,sz]` hollow stone box: a
/// shell (floor, ceiling, four walls) with a **dark, walkable** air interior and
/// no light source. Built in-code, no network assets (mirrors the admit fixture
/// style). `lights` place a glowstone at those local cells.
fn dark_box_nbt(size: [i32; 3], lights: &[[i32; 3]]) -> Vec<u8> {
    use fastnbt::Value;
    let [sx, sy, sz] = size;
    let mut blocks: Vec<Value> = Vec::new();
    let mut push = |x: i32, y: i32, z: i32, state: i32| {
        let mut c = std::collections::HashMap::new();
        c.insert(
            "pos".to_string(),
            Value::List(vec![Value::Int(x), Value::Int(y), Value::Int(z)]),
        );
        c.insert("state".to_string(), Value::Int(state));
        blocks.push(Value::Compound(c));
    };
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                let shell = y == 0 || y == sy - 1 || x == 0 || x == sx - 1 || z == 0 || z == sz - 1;
                if shell {
                    push(x, y, z, 1); // stone
                }
            }
        }
    }
    for l in lights {
        push(l[0], l[1], l[2], 2); // glowstone
    }
    let palette = Value::List(vec![
        pal_entry("minecraft:air"),
        pal_entry("minecraft:stone"),
        pal_entry("minecraft:glowstone"),
    ]);
    let mut root = std::collections::HashMap::new();
    root.insert("DataVersion".to_string(), Value::Int(4671));
    root.insert(
        "size".to_string(),
        Value::List(vec![Value::Int(sx), Value::Int(sy), Value::Int(sz)]),
    );
    root.insert("palette".to_string(), palette);
    root.insert("blocks".to_string(), Value::List(blocks));
    root.insert("entities".to_string(), Value::List(vec![]));
    let raw = fastnbt::to_bytes(&Value::Compound(root)).unwrap();
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    std::io::Write::write_all(&mut gz, &raw).unwrap();
    gz.finish().unwrap()
}

fn pal_entry(name: &str) -> fastnbt::Value {
    let mut c = std::collections::HashMap::new();
    c.insert("Name".to_string(), fastnbt::Value::String(name.to_string()));
    fastnbt::Value::Compound(c)
}

/// Build hello-world's plan but feed a **synthetic** structure (dark box) in place
/// of the real prefab bytes, so the assembled-light gate sees a dark interior.
fn build_with_structure(campaign: &Campaign, nbt: Vec<u8>) -> Result<BuildOutput, BuildFailure> {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            structures.insert(piece.structure_file.clone(), nbt.clone());
        }
    }
    let tree = CommandTree::v1_21_11();
    emit::build(
        &plan,
        &load_campaign_dir(&common::hello_world_dir())
            .unwrap()
            .inputs,
        &structures,
        &tree,
        &prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
}

/// Build a (possibly v0.5-mutated) campaign through the real emit path.
fn build(campaign: &Campaign) -> Result<BuildOutput, BuildFailure> {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(campaign, &prefabs).expect("plan builds");
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
        &load_campaign_dir(&common::hello_world_dir())
            .unwrap()
            .inputs,
        &structures,
        &tree,
        &prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
}

/// The `setup_finish.mcfunction` body of a build output.
fn setup_finish(out: &BuildOutput) -> String {
    let key = out
        .keys()
        .find(|k| k.ends_with("/function/setup_finish.mcfunction"))
        .expect("setup_finish emitted");
    String::from_utf8(out[key].clone()).unwrap()
}

/// The `setup.mcfunction` body (carries the sealing baseline).
fn setup(out: &BuildOutput) -> String {
    let key = out
        .keys()
        .find(|k| k.ends_with("/function/setup.mcfunction"))
        .expect("setup emitted");
    String::from_utf8(out[key].clone()).unwrap()
}

/// Criterion 1 + 2 + 10 (emission): a v0.5 campaign that declares a lantern
/// relight (forcing fixtures over the lit room) plus night/thunder emits relight
/// `setblock`s and time/weather commands, only registry fixture blocks, and
/// builds byte-identically twice.
#[test]
fn relight_and_timeweather_emit_byte_identically() {
    let mut c = hello_world();
    c.world.dsl_version = "0.5.0".to_string();
    c.world.content.time = Some(WorldTime::Night);
    c.world.content.weather = Some(WorldWeather::Thunder);
    c.world.content.areas[0].lighting = Some(AreaLighting {
        fixture: Fixture::Lantern,
        min_light: 10,
    });

    let a = build(&c).expect("v0.5 relight build succeeds");
    let b = build(&c).expect("second build succeeds");
    assert_eq!(a, b, "same DSL + seed → byte-identical output (ADR-0006)");

    let sf = setup_finish(&a);
    let fixtures: Vec<&str> = sf
        .lines()
        .filter(|l| l.starts_with("setblock ") && l.contains("minecraft:lantern"))
        .collect();
    assert!(
        !fixtures.is_empty(),
        "expected relight lantern setblocks in setup_finish:\n{sf}"
    );
    // Only registry fixture blocks are ever emitted by relight.
    for l in sf.lines().filter(|l| l.starts_with("setblock ")) {
        assert!(
            ["torch", "lantern", "campfire", "shroomlight"]
                .iter()
                .any(|f| l.contains(&format!("minecraft:{f}"))),
            "relight emitted a non-registry block: {l}"
        );
    }

    // Declared night + thunder appear in the sealing baseline.
    let s = setup(&a);
    assert!(s.contains("time set night"), "time set night missing:\n{s}");
    assert!(
        s.contains("weather thunder"),
        "weather thunder missing:\n{s}"
    );
}

/// Criterion 9 vs 10 (measured, end-to-end): the same lit prefab area with no
/// declaration builds clean under every reachable state (the interior is lit by
/// its own fixtures); adding no `lighting` never regresses the baseline.
#[test]
fn undeclared_lit_area_builds_clean() {
    let c = hello_world(); // v0.2 baseline, lit prefab
    assert!(build(&c).is_ok(), "the lit hello-room must build clean");
}

/// Criterion 6 (end-to-end, exit 2): a dark reachable area with no `lighting`
/// declaration and no night-vision kit fails the build with `DW0210`.
#[test]
fn crit6_dark_undeclared_build_fails_dw0210() {
    let c = hello_world(); // no lighting, no night-vision kit
    let err = build_with_structure(&c, dark_box_nbt([11, 6, 11], &[])).unwrap_err();
    match err {
        BuildFailure::Diagnostic { code, .. } => assert_eq!(code, "DW0210"),
        other => panic!("expected DW0210, got {other:?}"),
    }
}

/// Criterion 5 (end-to-end): the same dark area builds clean once a class kit
/// grants night vision (retained mitigation).
#[test]
fn crit5_dark_with_night_vision_builds() {
    let mut c = hello_world();
    // Add a night-vision kit item to the first class.
    c.classes.content.classes[0]
        .kit
        .push(delvewright_dsl::KitItem {
            item: "minecraft:potion".to_string(),
            count: 1,
            name: Some("Potion of Night Vision".to_string()),
        });
    // A dark box but night vision mitigates → no DW0210 (nav may still object, but
    // the lighting gate must pass): assert it is not a DW0210 failure.
    match build_with_structure(&c, dark_box_nbt([11, 6, 11], &[])) {
        Ok(_) => {}
        Err(BuildFailure::Diagnostic { code, .. }) => {
            assert_ne!(code, "DW0210", "night vision must mitigate the dark area")
        }
        Err(other) => panic!("unexpected {other:?}"),
    }
}

/// Criterion 7 (end-to-end, exit 2): a declared fixture that cannot reach an
/// unsatisfiable `min_light` fails with `DW0211`. A floor `torch` (block light
/// 14) can never raise a required-path cell to 14 (no torch may sit on it, and a
/// neighbour contributes at most 13).
#[test]
fn crit7_unsatisfiable_build_fails_dw0211() {
    let mut c = hello_world();
    c.world.dsl_version = "0.5.0".to_string();
    c.world.content.areas[0].lighting = Some(AreaLighting {
        fixture: Fixture::Torch,
        min_light: 14,
    });
    let err = build_with_structure(&c, dark_box_nbt([11, 6, 11], &[])).unwrap_err();
    match err {
        BuildFailure::Diagnostic { code, .. } => assert_eq!(code, "DW0211"),
        other => panic!("expected DW0211, got {other:?}"),
    }
}

/// Sanity: the assembled-light model measures the lit hello-room as not-dark
/// (no `DW0210` for the shipped campaign), matching its `lit` admission profile.
#[test]
fn hello_room_measures_not_dark() {
    let c = hello_world();
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&c, &prefabs).unwrap();
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let bytes = std::fs::read(common::prefabs_dir().join(&piece.structure_file)).unwrap();
            structures.insert(piece.structure_file.clone(), bytes);
        }
    }
    let r = light::relight(&plan, &structures);
    assert!(
        r.diagnostics.is_empty(),
        "lit hello-room must not trip DW0210: {:?}",
        r.diagnostics
    );
}
