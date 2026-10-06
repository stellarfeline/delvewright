//! **A sky is stated in a designer's words** (spec-0081): a time names the sun
//! or the moon, where it stands and what phase it shows, and the engine
//! computes the ticks. One test per acceptance criterion the integration tier
//! owns; the position table, the vendored keyframes and the sun-track agreement
//! are unit tests beside the code that reads them (`delvewright_dsl::celestial`,
//! `compiler::view::scene`).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use delvec::compiler::commands::CommandTree;
use delvec::compiler::light::effective_sky;
use delvec::compiler::view::camera::{self, ApprovedRow, Camera, CameraSky, Source};
use delvec::compiler::view::scene;
use delvewright_dsl::{Stage, WorldTime, WorldWeather, stage_schema, validate_campaign};

mod common;

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn tmp(tag: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("celestial-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn log(o: &Output) -> String {
    format!(
        "exit {:?}\n--- stdout\n{}\n--- stderr\n{}",
        o.status.code(),
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

fn time(json: &str) -> WorldTime {
    serde_json::from_str(json).unwrap_or_else(|e| panic!("{json}: {e}"))
}

/// A hello-world campaign whose world states `world_time`, with `cuts`
/// appended to its first objective's bundle as `set-time` effects.
fn campaign(tag: &str, world_time: serde_json::Value, cuts: &[serde_json::Value]) -> PathBuf {
    let camp = tmp(&format!("camp-{tag}"));
    common::copy_dir_all(&common::hello_world_dir(), &camp);
    common::patch_file(&camp.join("world.json"), |v| {
        v["content"]["time"] = world_time;
    });
    if !cuts.is_empty() {
        common::patch_file(&camp.join("quests.json"), |v| {
            let q = &mut v["content"]["quests"][0];
            let id = q["objectives"][0]["id"].as_str().unwrap().to_string();
            let bundle = q["on_objective_complete"][&id]
                .as_array_mut()
                .expect("hello-world fires an effect on its first objective");
            for t in cuts {
                bundle.push(serde_json::json!({ "type": "set-time", "time": t }));
            }
        });
    }
    camp
}

fn run(args: &[&str]) -> Output {
    Command::new(BIN).args(args).output().unwrap()
}

fn build(tag: &str, camp: &Path) -> (Output, PathBuf) {
    let out = tmp(&format!("out-{tag}"));
    let prefabs = common::prefabs_dir();
    let o = run(&[
        "build",
        camp.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        prefabs.to_str().unwrap(),
    ]);
    (o, out)
}

fn validate(camp: &Path) -> Output {
    let prefabs = common::prefabs_dir();
    run(&[
        "validate",
        camp.to_str().unwrap(),
        "--prefabs",
        prefabs.to_str().unwrap(),
    ])
}

fn read(out: &Path, rel: &str) -> String {
    std::fs::read_to_string(out.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// Every `time set` line under `dir`, in path order.
fn time_sets(dir: &Path) -> Vec<String> {
    let mut files = Vec::new();
    fn walk(d: &Path, out: &mut Vec<PathBuf>) {
        let mut es: Vec<_> = std::fs::read_dir(d)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        es.sort();
        for p in es {
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().and_then(|e| e.to_str()) == Some("mcfunction") {
                out.push(p);
            }
        }
    }
    walk(dir, &mut files);
    files
        .iter()
        .flat_map(|f| {
            std::fs::read_to_string(f)
                .unwrap()
                .lines()
                .filter(|l| l.starts_with("time set "))
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Criterion 1 — the surface
// ---------------------------------------------------------------------------

fn defs(schema: &serde_json::Value) -> &serde_json::Map<String, serde_json::Value> {
    schema["$defs"].as_object().expect("a schema with $defs")
}

/// Every object in `v` that has a `time` property referring to `WorldTime`.
fn time_refs(v: &serde_json::Value) -> usize {
    match v {
        serde_json::Value::Object(o) => {
            let here = o
                .get("properties")
                .and_then(|p| p.get("time"))
                .and_then(|t| t.get("$ref"))
                .and_then(|r| r.as_str())
                == Some("#/$defs/WorldTime");
            usize::from(here) + o.values().map(time_refs).sum::<usize>()
        }
        serde_json::Value::Array(a) => a.iter().map(time_refs).sum(),
        _ => 0,
    }
}

#[test]
fn every_site_exports_the_keyword_or_the_celestial_object() {
    let world = stage_schema(Stage::World);
    let d = defs(&world);
    let wt = &d["WorldTime"];
    let any: Vec<&str> = wt["anyOf"]
        .as_array()
        .expect("WorldTime is a union")
        .iter()
        .map(|b| b["$ref"].as_str().unwrap())
        .collect();
    assert_eq!(any, ["#/$defs/TimeKeyword", "#/$defs/CelestialTime"]);
    let kw: Vec<String> = serde_json::to_string(&d["TimeKeyword"])
        .unwrap()
        .split('"')
        .filter(|s| ["day", "noon", "dusk", "night", "midnight", "dawn"].contains(s))
        .map(str::to_string)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    assert_eq!(kw.len(), 6, "the six keywords: {kw:?}");
    let ct = &d["CelestialTime"]["properties"];
    for p in ["sun", "moon", "phase"] {
        assert!(ct.get(p).is_some(), "CelestialTime.{p}: {ct}");
    }
    assert_eq!(d["MoonPhase"]["type"], "string");
    let positions = serde_json::to_string(&d["Position"]).unwrap();
    for p in [
        "rising",
        "just-risen",
        "high",
        "setting",
        "just-set",
        "below",
    ] {
        assert!(positions.contains(&format!("\"{p}\"")), "{p}: {positions}");
    }
    assert_eq!(
        world["properties"]["content"]["$ref"],
        "#/$defs/WorldContent"
    );
    assert_eq!(
        d["WorldContent"]["properties"]["time"]["$ref"],
        "#/$defs/WorldTime"
    );
    for (stage, at_least) in [(Stage::Quests, 1), (Stage::Dialogue, 1), (Stage::Design, 1)] {
        let s = stage_schema(stage);
        let n = time_refs(&s);
        assert!(
            n >= at_least,
            "{stage:?}: {n} time field(s) refer to WorldTime"
        );
        assert!(defs(&s).contains_key("CelestialTime"), "{stage:?}");
    }
    let cams = camera::record_schema();
    assert!(
        time_refs(&cams) >= 1,
        "the camera record's sky.time is a WorldTime"
    );
}

// ---------------------------------------------------------------------------
// Criterion 4 — emission
// ---------------------------------------------------------------------------

#[test]
fn a_new_moon_just_risen_emits_its_day_and_the_noon_cut_keeps_it() {
    let camp = campaign(
        "new-moon",
        serde_json::json!({"moon": "just-risen", "phase": "new-moon"}),
        &[serde_json::json!("noon")],
    );
    let (o, out) = build("new-moon", &camp);
    assert_eq!(o.status.code(), Some(0), "{}", log(&o));
    let setup = read(&out, "datapack/data/hello-world/function/setup.mcfunction");
    assert!(setup.lines().any(|l| l == "time set 108959"), "{setup}");
    let sets = time_sets(&out.join("datapack"));
    assert!(sets.contains(&"time set 102000".to_string()), "{sets:?}");
    assert!(
        !sets.iter().any(|l| l == "time set noon"),
        "a keyword cut in a day-4 world keeps the day: {sets:?}"
    );
    let sealed = read(
        &out,
        "packtest-datapack/data/hello-world/test/sealed_state.mcfunction",
    );
    for want in [
        "assert score #sealtime_sealed dw.sys matches 12959",
        "assert score #sealday_sealed dw.sys matches 4",
        "execute store success score #sealmoon_sealed dw.sys if predicate hello-world:moon_new-moon",
        "assert score #sealmoon_sealed dw.sys matches 1",
    ] {
        assert!(sealed.lines().any(|l| l == want), "{want}\n{sealed}");
    }
    let pred: serde_json::Value = serde_json::from_str(&read(
        &out,
        "packtest-datapack/data/hello-world/predicate/moon_new-moon.json",
    ))
    .unwrap();
    assert_eq!(
        pred,
        serde_json::json!({
            "condition": "minecraft:time_check",
            "period": 192000,
            "value": {"min": 96000, "max": 119999},
        })
    );
    // The emitted lines pass the pinned command tree.
    let tree = CommandTree::v1_21_11();
    for line in sets.iter().chain(
        sealed
            .lines()
            .filter(|l| l.starts_with("execute"))
            .map(str::to_string)
            .collect::<Vec<_>>()
            .iter(),
    ) {
        tree.validate_line(line)
            .unwrap_or_else(|e| panic!("`{line}` is off the pinned tree: {e:?}"));
    }
}

/// A keyword world's sealing line and sealed-state test do not move: no day
/// assertion, no predicate.
#[test]
fn a_keyword_world_seals_as_it_always_did() {
    let camp = campaign("keyword", serde_json::json!("night"), &[]);
    let (o, out) = build("keyword", &camp);
    assert_eq!(o.status.code(), Some(0), "{}", log(&o));
    let setup = read(&out, "datapack/data/hello-world/function/setup.mcfunction");
    assert!(setup.lines().any(|l| l == "time set night"), "{setup}");
    let sealed = read(
        &out,
        "packtest-datapack/data/hello-world/test/sealed_state.mcfunction",
    );
    assert!(!sealed.contains("time query day\n"), "{sealed}");
    assert!(!sealed.contains("predicate"), "{sealed}");
    assert!(
        !out.join("packtest-datapack/data/hello-world/predicate")
            .exists()
    );
}

// ---------------------------------------------------------------------------
// Criterion 5 — the refusals
// ---------------------------------------------------------------------------

fn dw0931(world_time: serde_json::Value, cuts: &[serde_json::Value]) -> Vec<String> {
    let camp = campaign("parse", world_time, cuts);
    let loaded =
        delvec::compiler::load::load_campaign_dir(&camp).unwrap_or_else(|e| panic!("load: {e:?}"));
    let c = delvewright_dsl::parse_campaign(&loaded.raw).expect("the campaign parses");
    validate_campaign(&c)
        .into_iter()
        .filter(|d| d.code == "DW0931")
        .map(|d| d.message)
        .collect()
}

#[test]
fn each_broken_shape_is_dw0931_with_its_remedy() {
    // Shape 1: neither or both bodies.
    let both = dw0931(serde_json::json!({"sun": "high", "moon": "below"}), &[]);
    assert_eq!(both.len(), 1, "{both:?}");
    assert!(both[0].contains("NAME ONE BODY"), "{both:?}");
    let neither = dw0931(serde_json::json!({"phase": "new-moon"}), &[]);
    assert_eq!(neither.len(), 1, "{neither:?}");
    assert!(neither[0].contains("names neither"), "{neither:?}");
    // Shape 2: a phase where the moon is below — under a noon sun, at the
    // moon's nadir, under a sun just risen.
    for t in [
        serde_json::json!({"sun": "high", "phase": "new-moon"}),
        serde_json::json!({"moon": "below", "phase": "new-moon"}),
        serde_json::json!({"sun": "just-risen", "phase": "new-moon"}),
    ] {
        let m = dw0931(t.clone(), &[]);
        assert_eq!(m.len(), 1, "{t}: {m:?}");
        assert!(m[0].contains("REMOVE `phase`"), "{m:?}");
        assert!(m[0].contains("below the horizon"), "{m:?}");
    }
    // Shape 3: the world's moon is up and nobody named its phase.
    let m = dw0931(serde_json::json!({"sun": "just-set"}), &[]);
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(m[0].contains("STATE `phase`"), "{m:?}");
    assert!(
        m[0].contains("full-moon, waning-gibbous"),
        "the eight names: {m:?}"
    );
    // ...which a cut may leave out: it is the world's.
    assert!(
        dw0931(
            serde_json::json!({"moon": "high", "phase": "new-moon"}),
            &[serde_json::json!({"sun": "just-set"})]
        )
        .is_empty()
    );
    // A world refused for an unnamed moon has no phase a cut can restate: the
    // cut's own phase is not a second finding.
    let m = dw0931(
        serde_json::json!({"moon": "just-risen"}),
        &[serde_json::json!({"moon": "high", "phase": "full-moon"})],
    );
    assert_eq!(m.len(), 1, "only the world's own refusal: {m:?}");
    assert!(m[0].contains("STATE `phase`"), "{m:?}");
    // Shape 4: a cut restating the world's phase.
    let m = dw0931(
        serde_json::json!({"moon": "high", "phase": "new-moon"}),
        &[serde_json::json!({"moon": "rising", "phase": "new-moon"})],
    );
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(m[0].contains("which is the world's"), "{m:?}");
    // A well-formed celestial world and cuts: nothing.
    assert!(
        dw0931(
            serde_json::json!({"moon": "just-risen", "phase": "new-moon"}),
            &[
                serde_json::json!({"sun": "high"}),
                serde_json::json!({"moon": "just-set"}),
                serde_json::json!({"sun": "just-set", "phase": "waxing-crescent"}),
            ]
        )
        .is_empty()
    );
}

/// A phase outside the eight and a position outside the six are the schema's
/// refusal, `DW0100`, not a fifth shape.
#[test]
fn a_ninth_phase_and_a_seventh_position_are_dw0100() {
    for t in [
        serde_json::json!({"moon": "high", "phase": "blood-moon"}),
        serde_json::json!({"moon": "overhead", "phase": "new-moon"}),
    ] {
        let camp = campaign("dw0100", t.clone(), &[]);
        let o = validate(&camp);
        let out = log(&o);
        assert_eq!(o.status.code(), Some(1), "{t}: {out}");
        assert!(out.contains("DW0100"), "{t}: {out}");
        assert!(!out.contains("DW0931"), "{t}: {out}");
    }
}

// ---------------------------------------------------------------------------
// Criterion 6 — the readers
// ---------------------------------------------------------------------------

#[test]
fn the_light_model_judges_any_tick_by_the_plateau() {
    for (t, want) in [
        (WorldTime::Day, 15),
        (WorldTime::Noon, 15),
        (WorldTime::Dusk, 4),
        (WorldTime::Night, 4),
        (WorldTime::Midnight, 4),
        (WorldTime::Dawn, 4),
    ] {
        assert_eq!(
            effective_sky(t, WorldWeather::Clear),
            want,
            "{}",
            t.keyword()
        );
    }
    for tick in [12782, 13047, 23218, 23397] {
        assert_eq!(delvec::compiler::light::sky_base_judged(tick), 4, "{tick}");
    }
    assert_eq!(delvec::compiler::light::sky_base_judged(280), 15);
    for (t, want) in [
        (r#"{"sun":"setting"}"#, 4),
        (r#"{"sun":"just-set","phase":"new-moon"}"#, 4),
        (r#"{"sun":"rising"}"#, 4),
        (r#"{"moon":"just-set"}"#, 4),
        (r#"{"sun":"high"}"#, 15),
    ] {
        assert_eq!(effective_sky(time(t), WorldWeather::Clear), want, "{t}");
    }
}

fn design(camp: &Path, rows: &[(&str, serde_json::Value)]) {
    let refs: Vec<serde_json::Value> = rows
        .iter()
        .map(|(name, t)| {
            let p = camp.join("design").join(format!("{name}.png"));
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, b"an approved picture").unwrap();
            serde_json::json!({
                "name": name,
                "shows": "the picture, in one sentence",
                "time": t,
                "weather": "clear",
            })
        })
        .collect();
    std::fs::write(
        camp.join("design.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "dsl_version": delvewright_dsl::DSL_VERSION,
            "campaign_id": "hello-world",
            "stage": "design",
            "content": { "references": refs },
        }))
        .unwrap(),
    )
    .unwrap();
}

/// `DW0890` compares clocks: a `midnight` row (day 0) against a world whose
/// moon is new and high (day 4) is two hours that differ by their day; a
/// `{"moon": "high"}` row takes the world's phase and is the world's hour.
#[test]
fn the_design_record_compares_clocks_not_spellings() {
    let world = serde_json::json!({"moon": "high", "phase": "new-moon"});
    let camp = campaign("dw0890-red", world.clone(), &[]);
    design(&camp, &[("concept/crypt", serde_json::json!("midnight"))]);
    let o = validate(&camp);
    let out = log(&o);
    assert_eq!(o.status.code(), Some(1), "{out}");
    assert!(out.contains("DW0890"), "{out}");
    assert!(
        out.contains(r#"{"moon":"high","phase":"new-moon"} (day 4, 18000)"#),
        "the world's side, spelled and clocked: {out}"
    );
    assert!(
        out.contains("midnight (day 0, 18000)"),
        "the row's side: {out}"
    );

    let camp = campaign("dw0890-green", world, &[]);
    design(
        &camp,
        &[("concept/crypt", serde_json::json!({"moon": "high"}))],
    );
    let o = validate(&camp);
    let out = log(&o);
    assert_eq!(o.status.code(), Some(0), "{out}");
    assert!(!out.contains("DW0890 [error]"), "{out}");
}

// ---------------------------------------------------------------------------
// Criterion 7 — renders
// ---------------------------------------------------------------------------

#[test]
fn a_celestial_camera_renders_its_positions_sun_and_says_the_moon_is_not_drawn() {
    let plan = include_bytes!("fixtures/view/render-plan-ocean.json");
    let sky = CameraSky {
        time: time(r#"{"moon":"just-risen"}"#),
        weather: WorldWeather::Clear,
    };
    let cam = Camera {
        after: None,
        answers: "concept/quay".into(),
        exposure: 1.0,
        fov: 60.0,
        height: 90,
        name: "moonrise".into(),
        pitch: 10.0,
        pos: [10.5, 70.0, -4.25],
        sky: Some(sky),
        source: Source::Estimated,
        spp: 16,
        width: 160,
        yaw: 30.0,
    };
    let rows = [ApprovedRow {
        name: "concept/quay".into(),
        shows: String::new(),
        sky: Some(CameraSky {
            time: WorldTime::Day,
            weather: WorldWeather::Clear,
        }),
    }];
    let sheet = camera::CameraSheet {
        campaign_id: camera::plan_campaign_id(plan).unwrap(),
        cameras: vec![cam.clone()],
    };
    let e = camera::emit(
        plan,
        &sheet,
        &rows,
        &camera::EmitOptions {
            world_paths: [("moonrise".to_string(), "/abs/world".to_string())].into(),
            ..camera::EmitOptions::default()
        },
    )
    .unwrap();
    let scene_json: serde_json::Value = serde_json::from_slice(&e.scenes[0].1).unwrap();
    let want = scene::sun_at(12959);
    assert_eq!(scene_json["sun"]["altitude"], want.altitude, "{scene_json}");
    assert_eq!(scene_json["sun"]["azimuth"], want.azimuth, "{scene_json}");
    let line = camera::resolve_sky(&cam, &rows).unwrap().line();
    assert!(line.contains("moon not drawn by the renderer"), "{line}");
    // A camera under a noon sun says nothing of the moon.
    let noon = Camera {
        sky: Some(CameraSky {
            time: WorldTime::Noon,
            weather: WorldWeather::Clear,
        }),
        ..cam
    };
    let line = camera::resolve_sky(&noon, &rows).unwrap().line();
    assert!(!line.contains("moon not drawn"), "{line}");
    // `--sky` spells a celestial time in JSON, split off the last comma.
    let parsed = CameraSky::parse(r#"{"moon":"high","phase":"new-moon"},thunder"#).unwrap();
    assert_eq!(parsed.time, time(r#"{"moon":"high","phase":"new-moon"}"#));
    assert_eq!(parsed.weather, WorldWeather::Thunder);
}

// ---------------------------------------------------------------------------
// Criterion 8 — the binding line
// ---------------------------------------------------------------------------

#[test]
fn every_build_prints_its_clocks_zeroes_included() {
    let camp = campaign("lines-keyword", serde_json::json!("noon"), &[]);
    let (o, _) = build("lines-keyword", &camp);
    let out = log(&o);
    assert_eq!(o.status.code(), Some(0), "{out}");
    assert!(
        out.contains("clock: world noon -> day 0 daytime 6000; sun +90.00°, moon below the horizon (full-moon)"),
        "{out}"
    );
    assert!(
        out.contains(
            "clocks: 1 world + 0 cut(s); 0 celestial, 1 keyword; phases stated {}; days {0}"
        ),
        "{out}"
    );

    let camp = campaign(
        "lines-celestial",
        serde_json::json!({"moon": "just-risen", "phase": "new-moon"}),
        &[serde_json::json!("noon")],
    );
    let (o, _) = build("lines-celestial", &camp);
    let out = log(&o);
    assert_eq!(o.status.code(), Some(0), "{out}");
    assert!(
        out.contains(
            "clock: world {\"moon\":\"just-risen\",\"phase\":\"new-moon\"} -> day 4 daytime 12959 \
             (dayTime 108959); sun -2.86° W, moon +2.86° E new-moon; sky light judged 4, game 8; \
             burns: no"
        ),
        "{out}"
    );
    assert!(
        out.contains("clock: set-time noon -> day 4 daytime 6000 (dayTime 102000); sun +90.00°"),
        "{out}"
    );
    assert!(
        out.contains(
            "clocks: 1 world + 1 cut(s); 1 celestial, 1 keyword; phases stated {new-moon}; days {4}"
        ),
        "{out}"
    );
}

// ---------------------------------------------------------------------------
// Criterion 10 — one hour, two spellings
// ---------------------------------------------------------------------------

#[test]
fn the_moon_rising_and_the_sun_setting_are_one_clock_and_one_line() {
    let world = WorldTime::Night;
    let a = time(r#"{"moon":"rising"}"#);
    let b = time(r#"{"sun":"setting"}"#);
    use delvewright_dsl::TimeSite;
    assert_eq!(a.clock(TimeSite::Cut, world), b.clock(TimeSite::Cut, world));
    assert_eq!(a.clock(TimeSite::Sky, world), b.clock(TimeSite::Sky, world));
    let line = |t: WorldTime| format!("time set {}", t.token(t.clock(TimeSite::Cut, world)));
    assert_eq!(line(a), line(b));
    assert_eq!(line(a), "time set 12782");
    // And through a build: two cuts, one line each, the same bytes.
    let camp = campaign(
        "two-spellings",
        serde_json::json!("night"),
        &[
            serde_json::json!({"moon": "rising"}),
            serde_json::json!({"sun": "setting"}),
        ],
    );
    let (o, out) = build("two-spellings", &camp);
    assert_eq!(o.status.code(), Some(0), "{}", log(&o));
    let sets: Vec<String> = time_sets(&out.join("datapack"))
        .into_iter()
        .filter(|l| l == "time set 12782")
        .collect();
    assert_eq!(sets.len(), 2, "{sets:?}");
}
