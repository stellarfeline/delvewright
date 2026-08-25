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
    box_nbt(size, lights, "minecraft:glowstone")
}

/// [`dark_box_nbt`], with the emitter named: the same hollow stone shell, but
/// `lights` are cells of the SHELL replaced by `light_block` — a lamp set into
/// the wall face, which is where one goes (a lamp never occupies a cell a body
/// stands in). Lets a test light a room with something other than glowstone and
/// ask whether the assembled-light model can see it.
fn box_nbt(size: [i32; 3], lights: &[[i32; 3]], light_block: &str) -> Vec<u8> {
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
    let lit: std::collections::BTreeSet<[i32; 3]> = lights.iter().copied().collect();
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                let shell = y == 0 || y == sy - 1 || x == 0 || x == sx - 1 || z == 0 || z == sz - 1;
                if shell && !lit.contains(&[x, y, z]) {
                    push(x, y, z, 1); // stone
                }
            }
        }
    }
    for l in lights {
        push(l[0], l[1], l[2], 2); // the emitter
    }
    let palette = Value::List(vec![
        pal_entry("minecraft:air"),
        pal_entry("minecraft:stone"),
        pal_entry(light_block),
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

/// A structure palette entry. Accepts a full blockstate string
/// (`minecraft:candle[candles=4,lit=true]`) and splits it the way a vanilla
/// `.nbt` holds one: `Name` plus a `Properties` compound — which is exactly what
/// `assembled::…` reassembles into `name[k=v,…]` for the light model to read.
fn pal_entry(name: &str) -> fastnbt::Value {
    let (id, state) = match name.split_once('[') {
        Some((id, rest)) => (id, rest.trim_end_matches(']')),
        None => (name, ""),
    };
    let mut c = std::collections::HashMap::new();
    c.insert("Name".to_string(), fastnbt::Value::String(id.to_string()));
    if !state.is_empty() {
        let mut props = std::collections::HashMap::new();
        for kv in state.split(',') {
            let (k, v) = kv.split_once('=').expect("a palette property is k=v");
            props.insert(k.to_string(), fastnbt::Value::String(v.to_string()));
        }
        c.insert("Properties".to_string(), fastnbt::Value::Compound(props));
    }
    fastnbt::Value::Compound(c)
}

/// Build hello-world's plan but feed a **synthetic** structure (dark box) in place
/// of the real prefab bytes, so the assembled-light gate sees a dark interior.
fn build_with_structure(campaign: &Campaign, nbt: Vec<u8>) -> Result<BuildOutput, BuildFailure> {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for t in area.pieces.iter().flat_map(|p| &p.templates) {
            structures.insert(t.structure_file.clone(), nbt.clone());
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

/// Build `campaign` against a synthetic dark structure the way `delvec build
/// --lang <lang>` does: determine the night-vision `DW0210` verdict on the
/// **canonical English** campaign first, then localize a clone with `translations`
/// (the l10n sidecar swap) before planning + emitting. This mirrors `main.rs` so a
/// test can prove the lighting gate reaches the same verdict in every language.
fn build_localized(
    campaign_en: &Campaign,
    nbt: Vec<u8>,
    lang: &str,
    translations: &BTreeMap<String, String>,
) -> Result<BuildOutput, BuildFailure> {
    let mut c = campaign_en.clone();
    delvewright_dsl::localize(&mut c, translations);
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for t in area.pieces.iter().flat_map(|p| &p.templates) {
            structures.insert(t.structure_file.clone(), nbt.clone());
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
        Some(lang),
        "unpinned",
        &BTreeMap::new(),
    )
}

/// True unless the build failed specifically with `DW0210` (a nav/other failure is
/// treated as "lighting gate passed" — matching `crit5_dark_with_night_vision_builds`).
fn passes_dw0210(r: Result<BuildOutput, BuildFailure>) -> bool {
    !matches!(r, Err(BuildFailure::Diagnostic { code, .. }) if code == "DW0210")
}

/// Build a (possibly v0.5-mutated) campaign through the real emit path.
fn build(campaign: &Campaign) -> Result<BuildOutput, BuildFailure> {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
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

/// Four lit candles set into each wall of the same box, and nothing else: the
/// build must succeed. This is the room a designer actually writes — the
/// fiction-correct low, warm, domestic source — and until `emission()` learned
/// the candle it measured as pitch dark and `DW0210` refused it however bright
/// it was in the game.
///
/// `wall_candles` are the four cells, one per wall, at head height.
#[test]
fn a_room_lit_only_by_candles_builds() {
    let c = hello_world();
    let r = build_with_structure(
        &c,
        box_nbt(
            [11, 6, 11],
            &wall_candles(),
            "minecraft:candle[candles=4,lit=true,waterlogged=false]",
        ),
    );
    match &r {
        Ok(_) => {}
        Err(BuildFailure::Diagnostic { code, message }) => assert_ne!(
            *code, "DW0210",
            "four lit candles per wall are 12 block light each; the room is not dark: {message}"
        ),
        Err(other) => panic!("unexpected {other:?}"),
    }
}

/// The same room with the candles UNLIT still fails `DW0210`, and that is the
/// direction that matters: vanilla places a candle unlit, an unlit candle emits
/// nothing, and a repair that made every candle bright would have broken the
/// never-overestimate contract while looking like a success here.
#[test]
fn a_room_of_unlit_candles_is_still_dark() {
    let c = hello_world();
    let err = build_with_structure(
        &c,
        box_nbt(
            [11, 6, 11],
            &wall_candles(),
            "minecraft:candle[candles=4,lit=false,waterlogged=false]",
        ),
    )
    .unwrap_err();
    match err {
        BuildFailure::Diagnostic { code, .. } => assert_eq!(code, "DW0210"),
        other => panic!("expected DW0210 for unlit candles, got {other:?}"),
    }
}

/// One emitter cell in the middle of each of the four walls of an `[11, 6, 11]`
/// box, at head height.
fn wall_candles() -> Vec<[i32; 3]> {
    vec![[0, 3, 5], [10, 3, 5], [5, 3, 0], [5, 3, 10]]
}

/// Criterion 5 (end-to-end): the same dark area builds clean once the area
/// **declares** `mitigation: "night-vision"` (DSL v0.6) — and the build actually
/// emits the clocked `effect give` that backs the declaration.
#[test]
fn crit5_dark_with_declared_night_vision_builds() {
    let mut c = hello_world();
    c.world.dsl_version = "0.6.0".to_string();
    c.world.content.areas[0].mitigation = Some(delvewright_dsl::AreaMitigation::NightVision);
    // A dark box but the declared mitigation applies → no DW0210 (nav may still
    // object, but the lighting gate must pass).
    let r = build_with_structure(&c, dark_box_nbt([11, 6, 11], &[]));
    match &r {
        Ok(_) => {}
        Err(BuildFailure::Diagnostic { code, .. }) => assert_ne!(
            *code, "DW0210",
            "a declared night-vision mitigation must satisfy the darkness gate"
        ),
        Err(other) => panic!("unexpected {other:?}"),
    }
    // The gate and the feature are the same fact: the build emits the clock.
    let out = r.expect("the declared-mitigation build succeeds");
    let tick = out
        .keys()
        .find(|k| k.ends_with("/function/night_vision_tick.mcfunction"))
        .map(|k| String::from_utf8(out[k].clone()).unwrap())
        .expect("declaring the mitigation emits night_vision_tick");
    assert!(
        tick.contains("effect give @a[x=") && tick.contains("minecraft:night_vision 12 0 true"),
        "the clock must give hidden-particle night vision to players in the area box:\n{tick}"
    );
    assert!(
        tick.contains("schedule function") && tick.contains("night_vision_tick 20t"),
        "the clock must re-arm itself every second:\n{tick}"
    );
    assert!(
        setup_finish(&out).contains("night_vision_tick 20t"),
        "world init must start the clock"
    );
}

/// The dead heuristic: a class kit item merely *named* "Potion of Night Vision" —
/// a bare `minecraft:potion`, i.e. a renamed water bottle — grants nothing in the
/// world and must NOT satisfy `DW0210`.
///
/// This is the regression for the owner's island finding: the pre-0.6 name
/// heuristic accepted exactly this, so the check passed while the feature did not
/// exist. Semantics never key on player-facing free text.
#[test]
fn renamed_potion_kit_item_no_longer_mitigates_dw0210() {
    let mut c = hello_world();
    c.classes.content.classes[0]
        .kit
        .push(delvewright_dsl::KitItem {
            item: "minecraft:potion".to_string(),
            count: 1,
            name: Some("Potion of Night Vision".to_string()),
            carrier: None,
            flask: false,
            // Deliberately contents-less: this IS the Uncraftable Potion the
            // test is about. (`DW0487` refuses it at 0.8.0; this campaign's
            // classes stage is earlier, which is why the item survives to reach
            // the mitigation heuristic at all.)
            contents: None,
        });
    match build_with_structure(&c, dark_box_nbt([11, 6, 11], &[])).unwrap_err() {
        BuildFailure::Diagnostic { code, .. } => assert_eq!(
            code, "DW0210",
            "a renamed water bottle is not a night-vision mitigation"
        ),
        other => panic!("expected DW0210, got {other:?}"),
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

/// Regression: the `DW0210` mitigation verdict is **language-independent**. It is
/// now so by construction — the signal is a stage-1 `mitigation` declaration, not a
/// localizable display string — so an `en` build and a `zh-cn` build of the same
/// campaign reach the same verdict with nothing threaded past localization.
#[test]
fn dw0210_night_vision_verdict_is_language_independent() {
    let mut c = hello_world();
    c.world.dsl_version = "0.6.0".to_string();
    c.world.content.languages = vec!["zh-cn".to_string()];
    c.world.content.areas[0].mitigation = Some(delvewright_dsl::AreaMitigation::NightVision);
    let dark = dark_box_nbt([11, 6, 11], &[]);

    assert!(
        passes_dw0210(build_with_structure(&c, dark.clone())),
        "the declared mitigation must satisfy DW0210 in the English build"
    );

    // Identity translation for the whole inventory, so localize runs cleanly.
    let tr: BTreeMap<String, String> = delvewright_dsl::l10n_inventory(&c).into_iter().collect();
    assert!(
        passes_dw0210(build_localized(&c, dark, "zh-cn", &tr)),
        "the same declaration must satisfy DW0210 in the zh-cn build"
    );
}

/// Complementary verdict: with **no** `mitigation` declaration, a dark undeclared area
/// fails `DW0210` in every build language (en and zh-cn alike) — the gate is not
/// silently suppressed by localization either.
#[test]
fn dw0210_fires_in_every_language_without_night_vision() {
    let mut c = hello_world();
    c.world.content.languages = vec!["zh-cn".to_string()];
    let dark = dark_box_nbt([11, 6, 11], &[]);

    match build_with_structure(&c, dark.clone()).unwrap_err() {
        BuildFailure::Diagnostic { code, .. } => assert_eq!(code, "DW0210"),
        other => panic!("en: expected DW0210, got {other:?}"),
    }
    // Identity translation for the (unchanged) inventory, so localize runs cleanly.
    let tr: BTreeMap<String, String> = delvewright_dsl::l10n_inventory(&c).into_iter().collect();
    match build_localized(&c, dark, "zh-cn", &tr).unwrap_err() {
        BuildFailure::Diagnostic { code, .. } => assert_eq!(code, "DW0210"),
        other => panic!("zh-cn: expected DW0210, got {other:?}"),
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
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    let r = light::relight(&plan, &structures);
    assert!(
        r.diagnostics.is_empty(),
        "lit hello-room must not trip DW0210: {:?}",
        r.diagnostics
    );
}

/// The render-plan `lighting` stamp (dark-review tier): POV and interior shots
/// carry `{"profile","mitigation"}` derived purely from the area's stage-1
/// declarations — dark+mitigation for a mitigation-only area, lit when
/// `lighting` is declared (with the mitigation still recorded when both are) —
/// and **no `lighting` key at all** for an undeclared campaign, so plans
/// without declarations stay byte-identical.
#[test]
fn render_plan_lighting_stamp_follows_the_declarations() {
    let stamped_shots = |out: &BuildOutput| -> Vec<serde_json::Value> {
        let rp: serde_json::Value =
            serde_json::from_slice(out.get("render-plan.json").unwrap()).unwrap();
        rp["shots"].as_array().unwrap().clone()
    };

    // Baseline: hello-world declares neither → no shot carries the key.
    let c0 = hello_world();
    let out0 = build(&c0).expect("baseline builds");
    for s in stamped_shots(&out0) {
        assert!(
            s.get("lighting").is_none(),
            "undeclared campaign must emit no lighting stamp: {s}"
        );
    }

    // mitigation only → every POV/interior shot stamped dark + night-vision.
    // (One nbt buffer for both builds: `dark_box_nbt` serializes HashMap
    // compounds, so two *calls* give differently-ordered — thus different —
    // input bytes; determinism is same input → same output.)
    let dark = dark_box_nbt([11, 6, 11], &[]);
    let mut c1 = hello_world();
    c1.world.dsl_version = "0.6.0".to_string();
    c1.world.content.areas[0].mitigation = Some(delvewright_dsl::AreaMitigation::NightVision);
    let out1 = build_with_structure(&c1, dark.clone()).expect("declared-mitigation build succeeds");
    let shots1 = stamped_shots(&out1);
    let mut saw = (false, false);
    for s in &shots1 {
        match s["kind"].as_str().unwrap() {
            k @ ("pov" | "interior") => {
                assert_eq!(s["lighting"]["profile"], "dark", "kind {k}: {s}");
                assert_eq!(s["lighting"]["mitigation"], "night-vision", "kind {k}");
                if k == "pov" {
                    saw.0 = true;
                } else {
                    saw.1 = true;
                }
            }
            // The stamp's scope is exactly the POV/interior review tier.
            _ => assert!(s.get("lighting").is_none(), "only POV/interior: {s}"),
        }
    }
    assert!(saw.0 && saw.1, "both a pov and an interior shot stamped");

    // lighting + mitigation → lit profile, mitigation still recorded (fixtures
    // light the scene, so the render layer must not emulate).
    let mut c2 = hello_world();
    c2.world.dsl_version = "0.6.0".to_string();
    c2.world.content.areas[0].lighting = Some(AreaLighting {
        fixture: Fixture::Lantern,
        min_light: 7,
    });
    c2.world.content.areas[0].mitigation = Some(delvewright_dsl::AreaMitigation::NightVision);
    let out2 = build_with_structure(&c2, dark_box_nbt([11, 6, 11], &[]))
        .expect("relit+mitigated build succeeds");
    let pov2 = stamped_shots(&out2)
        .into_iter()
        .find(|s| s["kind"] == "pov")
        .expect("a pov shot");
    assert_eq!(pov2["lighting"]["profile"], "lit");
    assert_eq!(pov2["lighting"]["mitigation"], "night-vision");

    // Stamped builds stay byte-identical across a double build (ADR-0006).
    let again = build_with_structure(&c1, dark.clone()).unwrap();
    assert_eq!(out1, again, "stamped build is deterministic");
}

/// `dusk` / `dawn`: vanilla's `/time set` primitive
/// takes a raw tick count as well as its four keywords, so the states worth naming
/// for a delve's pacing are not limited to the keywords. The DSL names the beat;
/// the compiler emits the tick form — and the sealed-state PackTest, which reads
/// the world time back with `time query daytime`, asserts the exact value.
#[test]
fn dusk_and_dawn_emit_the_vanilla_tick_form() {
    // `dusk` is the SUNSET ONSET (12000), not 13000: 13000 is the instant the sun
    // has finished setting, which the `night` keyword already sets — so 13000 would
    // make `dusk` a synonym of `night` instead of its own beat.
    for (time, ticks) in [(WorldTime::Dusk, 12000), (WorldTime::Dawn, 23000)] {
        let mut c = hello_world();
        c.world.dsl_version = "0.5.0".to_string();
        c.world.content.time = Some(time);
        let out = build(&c).expect("a dusk/dawn campaign builds");
        let s = setup(&out);
        assert!(
            s.contains(&format!("time set {ticks}")),
            "expected `time set {ticks}` in setup:\n{s}"
        );
        let sealed = String::from_utf8(
            out[out
                .keys()
                .find(|k| k.ends_with("/test/sealed_state.mcfunction"))
                .expect("sealed_state emitted")]
            .clone(),
        )
        .unwrap();
        assert!(
            sealed.contains(&format!(
                "assert score #sealtime_sealed dw.sys matches {ticks}"
            )),
            "the sealed-state test must assert the declared daytime:\n{sealed}"
        );
        assert_eq!(build(&c).unwrap(), out, "byte-identical rebuild (ADR-0006)");
    }
    // The distinction is the whole point: `dusk` must not collapse onto `night`.
    assert_ne!(
        WorldTime::Dusk.daytime_ticks(),
        WorldTime::Night.daytime_ticks(),
        "dusk is the sunset onset, night is the sun already down — a shared tick \
         value would make one keyword a synonym of the other"
    );
}

/// The four states vanilla names keep emitting their KEYWORD verbatim — the whole
/// point of the one keyword/tick table is that adding dusk/dawn moves no shipped
/// campaign's bytes.
#[test]
fn the_vanilla_keywords_still_emit_keywords() {
    for (time, token) in [
        (WorldTime::Day, "day"),
        (WorldTime::Noon, "noon"),
        (WorldTime::Night, "night"),
        (WorldTime::Midnight, "midnight"),
    ] {
        let mut c = hello_world();
        c.world.dsl_version = "0.5.0".to_string();
        c.world.content.time = Some(time);
        let s = setup(&build(&c).expect("builds"));
        assert!(
            s.contains(&format!("time set {token}")),
            "expected the `{token}` keyword, not a tick count:\n{s}"
        );
    }
}
