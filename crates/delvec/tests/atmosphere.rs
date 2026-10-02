//! spec-0080 — a place has its own sky: what a campaign may write, what the
//! compiler emits for it, and that every proof reads one biome map.
//!
//! The fixture is hello-world (one area, `area/keep`, on the `hello-room`
//! piece) with atmospheres added by each case, so what each case changes is
//! exactly the thing it names.

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, Diagnostic, RawCampaign, parse_campaign};
use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn hw(name: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap())
        .unwrap()
}

/// The spike's station-2 block (`tools/spike-eldritch-visuals/` on
/// `research/eldritch-visuals`, `wrong_place.json`), its `attributes` verbatim.
fn wrong_place_attributes() -> Value {
    json!({
        "minecraft:audio/ambient_sounds": {
            "additions": { "sound": "minecraft:ambient.crimson_forest.additions", "tick_chance": 0.0111 },
            "loop": "minecraft:ambient.soul_sand_valley.loop",
            "mood": { "block_search_extent": 8, "offset": 2.0, "sound": "minecraft:ambient.soul_sand_valley.mood", "tick_delay": 1200 }
        },
        "minecraft:audio/background_music": {},
        "minecraft:audio/music_volume": 0.0,
        "minecraft:visual/ambient_particles": [
            { "particle": { "type": "minecraft:ash" }, "probability": 0.02 },
            { "particle": { "type": "minecraft:crimson_spore" }, "probability": 0.004 }
        ],
        "minecraft:visual/cloud_color": "#e04a1424",
        "minecraft:visual/cloud_fog_end_distance": 40.0,
        "minecraft:visual/fog_color": "#56602f",
        "minecraft:visual/fog_end_distance": 26.0,
        "minecraft:visual/fog_start_distance": 1.0,
        "minecraft:visual/sky_color": "#3b4a1e",
        "minecraft:visual/sky_fog_end_distance": 30.0,
        "minecraft:visual/sky_light_color": "#b6e07a",
        "minecraft:visual/sky_light_factor": 0.55,
        "minecraft:visual/star_brightness": 0.9,
        "minecraft:visual/water_fog_color": "#120a16",
        "minecraft:visual/water_fog_end_distance": 6.0
    })
}

/// hello-world with `atmospheres` declared, the keep carrying `carried`, and
/// `extra` appended to the `obj/talk` bundle.
fn campaign(atmospheres: Value, carried: Option<&str>, extra: Vec<Value>) -> Campaign {
    let mut world = hw("world.json");
    world["content"]["atmospheres"] = atmospheres;
    if let Some(id) = carried {
        world["content"]["areas"][0]["atmosphere"] = json!(id);
    }
    let mut quests = hw("quests.json");
    let bundle = quests["content"]["quests"][0]["on_objective_complete"]["obj/talk"]
        .as_array_mut()
        .expect("hello-world's talk bundle");
    bundle.extend(extra);
    let s = |v: &Value| serde_json::to_string(v).unwrap();
    let raw = RawCampaign {
        world: s(&world),
        npcs: s(&hw("npcs.json")),
        classes: s(&hw("classes.json")),
        quest_plan: s(&hw("quest-plan.json")),
        quests: s(&quests),
        dialogue: s(&hw("dialogue.json")),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    };
    parse_campaign(&raw).expect("the campaign parses")
}

fn atmosphere(id: &str, precipitation: &str, attributes: Value) -> Value {
    json!({ "id": id, "attributes": attributes, "precipitation": precipitation })
}

fn try_build(c: &Campaign) -> Result<BuildOutput, emit::BuildFailure> {
    let dir = common::prefabs_dir();
    let prefabs = PrefabRegistry::load_dir(&dir).unwrap();
    let plan = Plan::build(c, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                structures.insert(
                    t.structure_file.clone(),
                    std::fs::read(dir.join(&t.structure_file)).unwrap(),
                );
            }
        }
    }
    emit::build(
        &plan,
        &BTreeMap::new(),
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
}

fn file(out: &BuildOutput, path: &str) -> String {
    String::from_utf8(
        out.get(path)
            .unwrap_or_else(|| panic!("`{path}` is emitted"))
            .clone(),
    )
    .unwrap()
}

fn fillbiome_lines(out: &BuildOutput) -> Vec<String> {
    out.iter()
        .filter(|(p, _)| p.starts_with("datapack/") && p.ends_with(".mcfunction"))
        .flat_map(|(_, b)| {
            std::str::from_utf8(b)
                .unwrap()
                .lines()
                .filter(|l| l.starts_with("fillbiome "))
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .collect()
}

fn codes(d: &[Diagnostic]) -> Vec<String> {
    d.iter().map(|x| x.code.to_string()).collect()
}

fn check(c: &Campaign) -> Vec<Diagnostic> {
    delvec::compiler::atmosphere::check(c)
}

// --- the registry ------------------------------------------------------------

/// AC1: the three counts, and every admitted id the spike's station 2 carries
/// parses under its vendored shape.
#[test]
fn every_attribute_the_lab_walked_parses_under_its_vendored_shape() {
    let reg = delvec::compiler::atmosphere::Registry::v1_21_11();
    let count = |s: &str| reg.rows().filter(|r| r.scope == s).count();
    assert_eq!(
        (count("admitted"), count("overridden"), count("gameplay")),
        (20, 5, 20)
    );
    let attrs = wrong_place_attributes();
    let mut parsed = 0;
    for (key, value) in attrs.as_object().unwrap() {
        let row = reg
            .row(key)
            .unwrap_or_else(|| panic!("`{key}` is registered"));
        assert_eq!(row.scope, "admitted", "{key}");
        reg.value(row, value)
            .unwrap_or_else(|e| panic!("`{key}` parses: {e}"));
        parsed += 1;
    }
    assert_eq!(parsed, 16, "the station-2 block sets 16 of the 20");
}

// --- refusals (DW0928, DW0929, DW0930) -----------------------------------------

fn refused_attr(attrs: Value) -> Vec<Diagnostic> {
    let c = campaign(
        json!([atmosphere("atmosphere/wrong-place", "none", attrs)]),
        Some("atmosphere/wrong-place"),
        vec![],
    );
    check(&c)
}

#[test]
fn an_id_the_game_never_registered_is_dw0928_with_the_nearest_named() {
    let d = refused_attr(json!({ "visual/sky_colour": "#3b4a1e" }));
    assert_eq!(codes(&d), ["DW0928"]);
    assert!(
        d[0].message.contains("`visual/sky_color`"),
        "{}",
        d[0].message
    );
}

#[test]
fn an_id_the_day_cycle_overrides_is_dw0928() {
    let d = refused_attr(json!({ "visual/sun_angle": 90.0 }));
    assert_eq!(codes(&d), ["DW0928"]);
    assert!(d[0].message.contains("world-wide"), "{}", d[0].message);
}

#[test]
fn a_gameplay_id_is_dw0928() {
    let d = refused_attr(json!({ "gameplay/monsters_burn": false }));
    assert_eq!(codes(&d), ["DW0928"]);
    assert!(d[0].message.contains("monsters_burn"), "{}", d[0].message);
}

#[test]
fn a_value_not_in_its_shape_is_dw0928() {
    for attrs in [
        json!({ "visual/sky_color": 7 }),
        json!({ "visual/cloud_color": "#e04a14" }),
        json!({ "visual/ambient_particles": [{ "particle": { "type": "minecraft:ash" } }] }),
        json!({ "visual/fog_end_distance": { "argument": 0.5, "modifier": "alpha_blend" } }),
    ] {
        assert_eq!(codes(&refused_attr(attrs.clone())), ["DW0928"], "{attrs}");
    }
}

/// The range is the codec's: `sky_light_factor` is `UNIT_FLOAT`,
/// `fog_end_distance` `NON_NEGATIVE_FLOAT`, and `fog_start_distance` is
/// unbounded, so the engine bounds nothing there.
#[test]
fn a_value_outside_the_codec_range_is_dw0928_and_an_unbounded_one_is_not() {
    assert_eq!(
        codes(&refused_attr(json!({ "visual/sky_light_factor": 1.5 }))),
        ["DW0928"]
    );
    assert_eq!(
        codes(&refused_attr(json!({ "visual/fog_end_distance": -1.0 }))),
        ["DW0928"]
    );
    assert!(refused_attr(json!({ "visual/fog_start_distance": -40.0 })).is_empty());
}

#[test]
fn a_sound_or_particle_the_pinned_registries_lack_is_dw0928() {
    assert_eq!(
        codes(&refused_attr(
            json!({ "audio/ambient_sounds": { "loop": "minecraft:ambient.nowhere.loop" } })
        )),
        ["DW0928"]
    );
    assert_eq!(
        codes(&refused_attr(
            json!({ "visual/default_dripstone_particle": { "type": "minecraft:smog" } })
        )),
        ["DW0928"]
    );
    // A particle that takes options is not a `{type}` value.
    assert_eq!(
        codes(&refused_attr(
            json!({ "visual/default_dripstone_particle": { "type": "minecraft:dust" } })
        )),
        ["DW0928"]
    );
}

#[test]
fn a_repaint_naming_neither_or_both_volumes_is_dw0929() {
    let atm = json!([atmosphere("atmosphere/wrong-place", "none", json!({}))]);
    let both = campaign(
        atm.clone(),
        None,
        vec![
            json!({ "type": "set-atmosphere", "atmosphere": "atmosphere/wrong-place",
                     "region": { "anchor": "anchor/exit", "extent": [2, 2, 2] }, "place": "area/keep" }),
        ],
    );
    assert_eq!(codes(&check(&both)), ["DW0929"]);
    let neither = campaign(
        atm,
        None,
        vec![json!({ "type": "set-atmosphere", "atmosphere": "atmosphere/wrong-place" })],
    );
    assert_eq!(codes(&check(&neither)), ["DW0929"]);
}

#[test]
fn an_atmosphere_nothing_stands_in_is_dw0930() {
    let c = campaign(
        json!([atmosphere("atmosphere/wrong-place", "none", json!({}))]),
        None,
        vec![],
    );
    assert_eq!(codes(&check(&c)), ["DW0930"]);
}

#[test]
fn a_climate_against_its_precipitation_is_dw0930() {
    let mut a = atmosphere("atmosphere/wrong-place", "snow", json!({}));
    a["climate"] = json!({ "temperature": 0.8, "downfall": 0.4 });
    let c = campaign(json!([a]), Some("atmosphere/wrong-place"), vec![]);
    assert_eq!(codes(&check(&c)), ["DW0930"]);
    let mut a = atmosphere("atmosphere/wrong-place", "rain", json!({}));
    a["climate"] = json!({ "temperature": 0.1, "downfall": 0.4 });
    let c = campaign(json!([a]), Some("atmosphere/wrong-place"), vec![]);
    assert_eq!(codes(&check(&c)), ["DW0930"]);
}

#[test]
fn a_duplicate_atmosphere_is_dw0930() {
    let a = atmosphere("atmosphere/wrong-place", "none", json!({}));
    let c = campaign(
        json!([a.clone(), a]),
        Some("atmosphere/wrong-place"),
        vec![],
    );
    assert_eq!(codes(&check(&c)), ["DW0930"]);
}

/// A repaint past the map's extent is refused at the build, where the extent
/// exists.
#[test]
fn a_repaint_past_the_edge_of_the_map_is_dw0929_at_the_build() {
    let c = campaign(
        json!([atmosphere("atmosphere/wrong-place", "none", json!({}))]),
        None,
        vec![
            json!({ "type": "set-atmosphere", "atmosphere": "atmosphere/wrong-place",
                     "region": { "anchor": "anchor/exit", "extent": [64, 2, 2] } }),
        ],
    );
    match try_build(&c) {
        Err(emit::BuildFailure::Diagnostic { code, message }) => {
            assert_eq!(code.to_string(), "DW0929", "{message}");
        }
        other => panic!("expected DW0929, got {:?}", other.map(|o| o.len())),
    }
}

// --- emission -----------------------------------------------------------------

fn carried_and_repainted() -> Campaign {
    campaign(
        json!([
            {
                "id": "atmosphere/wrong-place",
                "attributes": wrong_place_attributes(),
                "tint": { "grass": "#6B6A2A", "foliage": "#5a4a2a", "dry_foliage": "#4a3a2a", "water": "#1a0f1f" },
                "precipitation": "none",
                "climate": { "temperature": 0.8, "downfall": 0.4 }
            },
            atmosphere("atmosphere/still", "rain", json!({ "visual/sky_color": "#12345a" }))
        ]),
        Some("atmosphere/wrong-place"),
        vec![
            json!({ "type": "set-atmosphere", "atmosphere": "atmosphere/still",
                    "region": { "anchor": "anchor/exit", "extent": [1, 1, 1] } }),
            json!({ "type": "set-atmosphere", "atmosphere": null, "place": "area/keep" }),
        ],
    )
}

/// AC3: the biome file, its tag, the bootstrap paint over the carried place's
/// bounds, and a repaint over `Plan::zone_box` and over the place.
#[test]
fn an_atmosphere_ships_as_a_biome_painted_at_setup_and_by_a_beat() {
    let c = carried_and_repainted();
    assert!(check(&c).is_empty(), "{:#?}", check(&c));
    let out = try_build(&c).expect("builds");
    let biome: Value = serde_json::from_str(&file(
        &out,
        "datapack/data/hello-world/worldgen/biome/atmosphere/wrong-place.json",
    ))
    .unwrap();
    assert_eq!(biome["features"], json!([]));
    assert_eq!(biome["has_precipitation"], json!(false));
    assert_eq!(biome["temperature"], json!(0.8));
    assert_eq!(biome["downfall"], json!(0.4));
    assert_eq!(
        biome["effects"]["grass_color"],
        json!("#6b6a2a"),
        "lower-cased"
    );
    assert_eq!(
        biome["attributes"]["minecraft:visual/sky_color"],
        json!("#3b4a1e")
    );
    assert_eq!(biome["attributes"].as_object().unwrap().len(), 16);
    let tag = file(
        &out,
        "datapack/data/minecraft/tags/worldgen/biome/without_wandering_trader_spawns.json",
    );
    assert!(tag.contains("hello-world:atmosphere/wrong-place"), "{tag}");
    assert!(tag.contains("hello-world:atmosphere/still"), "{tag}");
    let setup = file(
        &out,
        "datapack/data/hello-world/function/setup_finish.mcfunction",
    );
    assert!(
        setup.contains("function hello-world:atmosphere_bootstrap"),
        "{setup}"
    );
    let boot = file(
        &out,
        "datapack/data/hello-world/function/atmosphere_bootstrap.mcfunction",
    );
    assert_eq!(boot.lines().count(), 1, "{boot}");
    assert!(
        boot.ends_with("hello-world:atmosphere/wrong-place\n"),
        "{boot}"
    );
    let lines = fillbiome_lines(&out);
    assert!(
        lines
            .iter()
            .any(|l| l.ends_with("hello-world:atmosphere/still")),
        "the region repaint: {lines:#?}"
    );
    assert!(
        lines.iter().any(|l| l.ends_with("hello-world:void")),
        "the paint-back to the horizon's biome: {lines:#?}"
    );
    let packtests: Vec<&String> = out
        .keys()
        .filter(|p| p.contains("/test/atmosphere_"))
        .collect();
    assert_eq!(packtests.len(), 3, "{packtests:#?}");
}

/// AC4: a float's spelling moves no emitted byte.
#[test]
fn a_float_spelling_moves_no_byte() {
    let spelled = |v: Value| {
        let c = campaign(
            json!([atmosphere(
                "atmosphere/wrong-place",
                "none",
                json!({ "visual/fog_start_distance": v })
            )]),
            Some("atmosphere/wrong-place"),
            vec![],
        );
        try_build(&c).expect("builds")
    };
    let a = spelled(json!(1.0));
    let b = spelled(json!(1));
    assert_eq!(a, b, "`1.0` and `1` are one value");
}

/// §7.3, perturbation acceptance: each declaration moves the byte it owns.
#[test]
fn each_declaration_moves_the_byte_it_owns() {
    let base = try_build(&carried_and_repainted()).expect("builds");
    let path = "datapack/data/hello-world/worldgen/biome/atmosphere/wrong-place.json";
    // The sky colour moves the biome file.
    let mut c = carried_and_repainted();
    c.world.content.atmospheres[0]
        .attributes
        .insert("minecraft:visual/sky_color".to_string(), json!("#3b4a1f"));
    let moved = try_build(&c).expect("builds");
    assert_ne!(file(&base, path), file(&moved, path));
    // The carried id moves the bootstrap line.
    let mut c = carried_and_repainted();
    c.world.content.areas[0].atmosphere = Some(delvewright_dsl::AtmosphereId(
        "atmosphere/still".to_string(),
    ));
    let moved = try_build(&c).expect("builds");
    let boot = "datapack/data/hello-world/function/atmosphere_bootstrap.mcfunction";
    assert_ne!(file(&base, boot), file(&moved, boot));
    // Removing the repaints removes their lines.
    let mut c = carried_and_repainted();
    let bundle = c.quests.content.quests[0]
        .on_objective_complete
        .get_mut(&delvewright_dsl::ObjectiveId("obj/talk".to_string()))
        .unwrap();
    bundle.retain(|e| !matches!(e.verb, delvewright_dsl::Verb::SetAtmosphere { .. }));
    let moved = try_build(&c).expect("builds");
    assert_eq!(fillbiome_lines(&base).len(), 3);
    assert_eq!(
        fillbiome_lines(&moved).len(),
        1,
        "only the bootstrap paint is left"
    );
}

/// AC3: one writer of `fillbiome` under `crates/delvec/src/`.
#[test]
fn one_writer_of_fillbiome() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut writers = Vec::new();
    let mut stack = vec![src];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let p = entry.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|e| e == "rs") {
                let text = std::fs::read_to_string(&p).unwrap();
                for (i, line) in text.lines().enumerate() {
                    if line.contains("\"fillbiome ") {
                        writers.push(format!("{}:{}", p.display(), i + 1));
                    }
                }
            }
        }
    }
    assert_eq!(writers.len(), 1, "{writers:#?}");
    assert!(
        writers[0].contains("compiler/atmosphere.rs"),
        "{writers:#?}"
    );
}

/// AC5: the map is the only reader of the ground biome and the surround's
/// bands — `daylight::biome_at` is gone.
#[test]
fn the_biome_map_is_the_one_reader_of_which_biome_is_where() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/compiler");
    let mut readers = Vec::new();
    for entry in std::fs::read_dir(&src).unwrap() {
        let p = entry.unwrap().path();
        if p.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let text = std::fs::read_to_string(&p).unwrap();
        for (i, line) in text.lines().enumerate() {
            // `emit_ground_biome` writes the biome files from the map's own
            // `ground`; it is a writer named after the thing, not a reader.
            let code = line
                .split("//")
                .next()
                .unwrap_or_default()
                .replace("emit_ground_biome", "");
            if code.contains("ground_biome(") && !code.contains("fn ground_biome") {
                readers.push(format!("{}:{} ground_biome", p.display(), i + 1));
            }
            if code.contains("surround.biome") || code.contains("fn biome_at") {
                readers.push(format!("{}:{} {}", p.display(), i + 1, code.trim()));
            }
        }
    }
    assert_eq!(readers.len(), 2, "{readers:#?}");
    assert!(
        readers.iter().all(|r| r.contains("horizon.rs")),
        "only the map's constructor reads them: {readers:#?}"
    );
}

/// AC7: the binding line, on every build — a measured zero included.
#[test]
fn every_build_prints_the_atmosphere_binding_line() {
    let out = std::env::temp_dir().join(format!("delvec-atmosphere-cli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    let run = std::process::Command::new(BIN)
        .arg("build")
        .arg(common::hello_world_dir())
        .arg("-o")
        .arg(&out)
        .arg("--prefabs")
        .arg(common::prefabs_dir())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(run.status.success(), "{stderr}");
    assert!(
        stderr.contains(
            "atmosphere binding: 0 declared; 0 of 1 place(s) carry one; 0 repaint effect(s)"
        ),
        "{stderr}"
    );
    let _ = std::fs::remove_dir_all(&out);
}
