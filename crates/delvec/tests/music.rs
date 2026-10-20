//! spec-0109 — a scene states its music: every biome a camera can stand in
//! declares a track or silence (`DW1031`), the ground states its own, a story
//! stage moves the music by repainting the place, `stop-sound` is the cut, a
//! played sound names its category, and a cutscene's `hears` is confirmed
//! against the biome its camera passes (`DW1032`), and a cutscene's own
//! `music` is painted on every cell its camera can be in and restored.
//!
//! The fixture is hello-world (one area, `area/keep`, on the `hello-room`
//! piece) with each case's edits, so what each case changes is exactly the
//! thing it names.

mod common;

use std::collections::BTreeMap;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, Diagnostic, RawCampaign, parse_campaign, validate_campaign};
use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn hw(name: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap())
        .unwrap()
}

/// hello-world with `world` and `quests` edited by `f`.
fn raw(f: impl FnOnce(&mut Value, &mut Value)) -> RawCampaign {
    let mut world = hw("world.json");
    let mut quests = hw("quests.json");
    f(&mut world, &mut quests);
    let s = |v: &Value| serde_json::to_string(v).unwrap();
    RawCampaign {
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
    }
}

fn campaign(f: impl FnOnce(&mut Value, &mut Value)) -> Campaign {
    parse_campaign(&raw(f)).expect("the campaign parses")
}

fn talk(quests: &mut Value) -> &mut Vec<Value> {
    quests["content"]["quests"][0]["on_objective_complete"]["obj/talk"]
        .as_array_mut()
        .expect("hello-world's talk bundle")
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

fn ledger(out: &BuildOutput) -> Value {
    serde_json::from_str(&file(out, "validation/music.json")).unwrap()
}

fn codes(d: &[Diagnostic]) -> Vec<String> {
    d.iter().map(|x| x.code.to_string()).collect()
}

fn failure_code(e: &emit::BuildFailure) -> String {
    match e {
        emit::BuildFailure::Diagnostic { code, .. } => code.to_string(),
        other => format!("{other:?}"),
    }
}

fn failure_message(e: &emit::BuildFailure) -> String {
    match e {
        emit::BuildFailure::Diagnostic { message, .. } => message.clone(),
        other => format!("{other:?}"),
    }
}

/// A silence that fades and a track that replaces and loops.
fn hush() -> Value {
    json!({
        "id": "atmosphere/hush",
        "attributes": { "audio/background_music": {}, "audio/music_volume": 0.0 },
        "precipitation": "none"
    })
}

fn dread() -> Value {
    json!({
        "id": "atmosphere/dread",
        "attributes": { "audio/background_music": { "default": {
            "sound": "minecraft:music_disc.otherside",
            "min_delay": 0, "max_delay": 0, "replace_current_music": true
        } } },
        "precipitation": "none"
    })
}

fn cutscene(path: Value, hears: Option<Value>) -> Value {
    let mut c = json!({ "type": "cutscene", "seconds": 2, "path": path });
    if let Some(h) = hears {
        c["hears"] = h;
    }
    c
}

fn mark(offset: [i32; 3]) -> Value {
    json!({ "anchor": "anchor/keeper-stand", "offset": offset })
}

// --- criterion 1: the surface ---------------------------------------------------

fn schema(stage: &str) -> Value {
    let out = std::process::Command::new(BIN)
        .args(["schema", "--stage", stage])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}

fn variant<'a>(defs: &'a Value, union: &str, tag: &str) -> &'a Value {
    defs[union]["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["properties"]["type"]["const"] == tag)
        .unwrap_or_else(|| panic!("`{tag}` is a variant of `{union}`"))
}

fn keys(v: &Value) -> Vec<String> {
    let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
    k.sort();
    k
}

/// The exported surface: `horizon` a required object with `attributes` and no
/// string form; `stop-sound {category, sound}`; one `SoundCategory` of eleven
/// words on `play-sound`, `stop-sound` and `pulses[]`; `hears` on `cutscene`.
#[test]
fn the_surface_is_exported() {
    let w = schema("world");
    let defs = &w["$defs"];
    let content = &defs["WorldContent"];
    assert!(
        content["required"]
            .as_array()
            .unwrap()
            .contains(&json!("horizon")),
        "{}",
        content["required"]
    );
    assert_eq!(
        content["properties"]["horizon"]["$ref"],
        json!("#/$defs/HorizonSpec"),
        "the object form alone, no string branch"
    );
    assert_eq!(
        keys(&defs["HorizonSpec"]["properties"]),
        ["attributes", "base", "ratio", "rim_height"]
    );
    assert!(defs.get("Horizon").is_none(), "no wire enum is exported");

    let q = schema("quests");
    let defs = &q["$defs"];
    let stop = variant(defs, "QuestEffect", "stop-sound");
    let fields: Vec<String> = keys(&stop["properties"])
        .into_iter()
        .filter(|k| k != "type")
        .collect();
    // The envelope (`when`, `audience`, `in`) rides beside every verb.
    let own: Vec<&String> = fields
        .iter()
        .filter(|k| !["when", "audience", "in"].contains(&k.as_str()))
        .collect();
    assert_eq!(own, ["category", "sound"], "{fields:?}");
    let words: Vec<&str> = defs["SoundCategory"]["enum"]
        .as_array()
        .or(defs["SoundCategory"]["oneOf"].as_array())
        .unwrap()
        .iter()
        .map(|v| v.as_str().or(v["const"].as_str()).unwrap())
        .collect();
    assert_eq!(
        words,
        [
            "master", "music", "record", "weather", "block", "hostile", "neutral", "player",
            "ambient", "voice", "ui"
        ]
    );
    let refers = |v: &Value| v.to_string().contains("#/$defs/SoundCategory");
    assert!(refers(&stop["properties"]["category"]));
    assert!(refers(
        &variant(defs, "QuestEffect", "play-sound")["properties"]["category"]
    ));
    assert!(refers(&defs["Pulse"]["properties"]["category"]));
    let hears = &variant(defs, "QuestEffect", "cutscene")["properties"]["hears"];
    assert!(
        hears.to_string().contains("#/$defs/AtmosphereId"),
        "{hears}"
    );
    // `music` on `cutscene`: an attribute map (an object whose values are any
    // JSON), as `atmospheres[].attributes` and `horizon.attributes` are.
    let music = &variant(defs, "QuestEffect", "cutscene")["properties"]["music"];
    assert!(
        music.to_string().contains("\"object\"")
            && music.to_string().contains("additionalProperties"),
        "{music}"
    );
}

/// One declaration of `SoundCategory` in the DSL crate's sources.
#[test]
fn one_sound_category() {
    let src = common::repo_root().join("crates/dsl/src");
    let mut found = Vec::new();
    let mut stack = vec![src];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                let text = std::fs::read_to_string(&p).unwrap();
                if text.contains("enum SoundCategory") {
                    found.push(p);
                }
            }
        }
    }
    assert_eq!(found.len(), 1, "{found:?}");
}

// --- criterion 2: the obligation --------------------------------------------------

fn refused_1031(f: impl FnOnce(&mut Value, &mut Value)) -> Vec<Diagnostic> {
    let c = campaign(f);
    let d: Vec<Diagnostic> = validate_campaign(&c)
        .into_iter()
        .filter(|d| d.code == "DW1031")
        .collect();
    assert_eq!(d.len(), 1, "{d:#?}");
    let m = &d[0].message;
    assert!(m.contains("`{}` for silence"), "both spellings: {m}");
    assert!(m.contains("\"default\""), "both spellings: {m}");
    d
}

#[test]
fn a_scene_that_does_not_state_its_music_is_dw1031() {
    // An atmosphere without `audio/background_music`.
    refused_1031(|w, q| {
        w["content"]["atmospheres"] = json!([{ "id": "atmosphere/hush", "precipitation": "none",
            "attributes": { "visual/sky_color": "#3b4a1e" } }]);
        w["content"]["areas"][0]["atmosphere"] = json!("atmosphere/hush");
        let _ = q;
    });
    // `horizon: "void"`, the retired string form.
    refused_1031(|w, _| w["content"]["horizon"] = json!("void"));
    // `horizon` absent.
    refused_1031(|w, _| {
        w["content"].as_object_mut().unwrap().remove("horizon");
    });
    // The object form with no `background_music`.
    refused_1031(|w, _| w["content"]["horizon"] = json!({ "base": "void" }));
}

#[test]
fn music_nobody_can_hear_and_a_ground_that_paints_its_sky_are_dw1031() {
    // A `creative` entry: no delve body is in creative mode.
    let d = refused_1031_any(|w, _| {
        w["content"]["horizon"]["attributes"]["audio/background_music"] = json!({
            "creative": { "sound": "minecraft:music.creative", "min_delay": 12000, "max_delay": 24000 }
        });
    });
    assert!(d.message.contains("creative"), "{}", d.message);
    // A `visual/` id under `horizon.attributes`.
    let d = refused_1031_any(|w, _| {
        w["content"]["horizon"]["attributes"]["visual/sky_color"] = json!("#7ba4ff");
    });
    assert!(d.message.contains("audio/"), "{}", d.message);
}

fn refused_1031_any(f: impl FnOnce(&mut Value, &mut Value)) -> Diagnostic {
    let c = campaign(f);
    let d: Vec<Diagnostic> = validate_campaign(&c)
        .into_iter()
        .filter(|d| d.code == "DW1031")
        .collect();
    assert_eq!(d.len(), 1, "{d:#?}");
    d[0].clone()
}

/// The stated forms validate clean of `DW1031`.
#[test]
fn a_scene_that_states_its_music_validates() {
    let c = campaign(|w, _| {
        w["content"]["atmospheres"] = json!([hush(), dread()]);
        w["content"]["areas"][0]["atmosphere"] = json!("atmosphere/hush");
    });
    let d = validate_campaign(&c);
    let d: Vec<&Diagnostic> = d
        .iter()
        .filter(|d| d.code == "DW1031" || d.code == "DW0928")
        .collect();
    assert!(d.is_empty(), "{d:#?}");
}

// --- criterion 3: the ground ------------------------------------------------------

/// A void or valley ground carries exactly the declared audio beside its sky;
/// an ocean ground is `<ns>:ocean`, the pinned `minecraft:ocean` in every field
/// but the declared audio.
#[test]
fn the_ground_carries_the_declared_audio() {
    let declared = json!({
        "audio/background_music": { "default": {
            "sound": "minecraft:music.overworld.forest", "min_delay": 12000, "max_delay": 24000 } },
        "audio/music_volume": 0.5
    });
    for base in ["void", "valley"] {
        let c = campaign(|w, _| {
            w["content"]["horizon"] = json!({ "base": base, "attributes": declared.clone() });
        });
        let g = delvec::compiler::horizon::ground_biome(&c, "t");
        assert_eq!(g.id, "t:void", "{base}");
        assert_eq!(
            keys(&g.definition["attributes"]),
            [
                "minecraft:audio/background_music",
                "minecraft:audio/music_volume",
                "minecraft:visual/sky_color"
            ],
            "{base}"
        );
        assert_eq!(
            g.definition["attributes"]["minecraft:audio/background_music"]["default"]["sound"],
            json!("minecraft:music.overworld.forest")
        );
    }
    let c = campaign(|w, _| {
        w["content"]["horizon"] = json!({ "base": "ocean", "attributes": declared.clone() });
    });
    let g = delvec::compiler::horizon::ground_biome(&c, "t");
    assert_eq!((g.id.as_str(), g.path), ("t:ocean", "ocean"));
    let pinned: Value = serde_json::from_str(
        &std::fs::read_to_string(
            common::repo_root().join("crates/delvec/data/biome-ocean-1.21.11.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let mut expected = pinned.clone();
    let attrs = expected["attributes"].as_object_mut().unwrap();
    attrs.insert(
        "minecraft:audio/background_music".into(),
        json!({ "default": { "max_delay": 24000, "min_delay": 12000,
                             "sound": "minecraft:music.overworld.forest" } }),
    );
    attrs.insert("minecraft:audio/music_volume".into(), json!(0.5));
    assert_eq!(g.definition, expected, "every field but the declared audio");
    assert_ne!(
        pinned["attributes"]["minecraft:audio/background_music"],
        expected["attributes"]["minecraft:audio/background_music"],
        "vanilla's ocean music never ships"
    );
}

/// The void ground is written with the declared audio, and no vanilla biome is
/// overridden.
#[test]
fn the_void_ground_ships_the_declared_silence() {
    let c = campaign(|_, _| {});
    let out = try_build(&c).expect("builds");
    let void: Value = serde_json::from_str(&file(
        &out,
        "datapack/data/hello-world/worldgen/biome/void.json",
    ))
    .unwrap();
    assert_eq!(
        void["attributes"],
        json!({ "minecraft:audio/background_music": {}, "minecraft:visual/sky_color": "#7ba4ff" })
    );
    assert!(
        !out.keys()
            .any(|k| k.starts_with("datapack/data/minecraft/worldgen/biome/")),
        "no vanilla biome is overridden"
    );
}

// --- criterion 4: emission ----------------------------------------------------------

fn function_lines(out: &BuildOutput, needle: &str) -> Vec<String> {
    let mut v: Vec<String> = out
        .iter()
        .filter(|(p, _)| p.starts_with("datapack/") && p.ends_with(".mcfunction"))
        .flat_map(|(_, b)| {
            std::str::from_utf8(b)
                .unwrap()
                .lines()
                .filter(|l| l.contains(needle))
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .collect();
    v.sort();
    v
}

#[test]
fn stop_sound_writes_the_three_shapes_and_a_category_moves_its_token() {
    let c = campaign(|_, q| {
        talk(q).extend([
            json!({ "type": "stop-sound", "category": "music" }),
            json!({ "type": "stop-sound", "category": "ambient", "sound": "minecraft:ambient.cave" }),
            json!({ "type": "stop-sound", "sound": "entity.warden.heartbeat" }),
            json!({ "type": "play-sound", "sound": "minecraft:music.dragon", "category": "music" }),
            json!({ "type": "play-sound", "sound": "minecraft:block.bell.use" }),
        ]);
        q["content"]["pulses"] = json!([
            { "id": "pulse/drum", "sound": "minecraft:block.note_block.basedrum",
              "at": { "anchor": "anchor/keeper-stand" }, "place": "area/keep",
              "every": 20, "floor": 0.5, "category": "music" },
            { "id": "pulse/tick", "sound": "minecraft:block.note_block.hat",
              "at": { "anchor": "anchor/keeper-stand" }, "place": "area/keep",
              "every": 20, "floor": 0.5 }
        ]);
    });
    // `emit::build` walks every line against the pinned command tree.
    let out = try_build(&c).expect("builds");
    let stops = function_lines(&out, "stopsound ");
    for want in [
        "stopsound @a music",
        "stopsound @a ambient minecraft:ambient.cave",
        "stopsound @a * minecraft:entity.warden.heartbeat",
    ] {
        assert!(
            stops.iter().any(|l| l.ends_with(want)),
            "{want}: {stops:#?}"
        );
    }
    let plays = function_lines(&out, "playsound ");
    assert!(
        plays
            .iter()
            .any(|l| l.ends_with("run playsound minecraft:music.dragon music @s")),
        "{plays:#?}"
    );
    assert!(
        plays
            .iter()
            .any(|l| l.ends_with("run playsound minecraft:block.bell.use master @s")),
        "{plays:#?}"
    );
    assert!(
        plays
            .iter()
            .any(|l| l.contains("playsound minecraft:block.note_block.basedrum music @a[")),
        "{plays:#?}"
    );
    assert!(
        plays
            .iter()
            .any(|l| l.contains("playsound minecraft:block.note_block.hat master @a[")),
        "{plays:#?}"
    );
    let l = ledger(&out);
    let cats: Vec<&str> = l["categories"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["category"].as_str().unwrap())
        .collect();
    assert_eq!(
        cats.iter().filter(|c| **c != "master").count(),
        2,
        "{cats:?}"
    );
    assert_eq!(l["stop_sounds"].as_array().unwrap().len(), 3);
}

/// A boxed cut reaches a watcher: no `tag=!dw_cutscene`, counted at the
/// observer census's `sound cut` site.
#[test]
fn a_boxed_cut_is_heard_by_a_watcher() {
    let c = campaign(|_, q| {
        talk(q).push(json!({ "type": "stop-sound", "category": "music",
            "in": { "anchor": "anchor/keeper-stand", "extent": [3, 2, 3] } }));
    });
    let out = try_build(&c).expect("builds");
    let stops = function_lines(&out, "stopsound ");
    assert_eq!(stops.len(), 1, "{stops:#?}");
    assert!(stops[0].contains("stopsound @a[") && !stops[0].contains("dw_cutscene"));
    let census: Value =
        serde_json::from_str(&file(&out, "validation/observer-census.json")).unwrap();
    assert!(
        census["allowed"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["site"] == "sound cut" && r["count"] == 1),
        "{census}"
    );
}

// --- criterion 5: a cutscene's hearing ---------------------------------------------

/// hello-world whose keep stands in a fading silence, with `before` pushed onto
/// the talk bundle ahead of one cutscene over `path` that declares `hears`.
fn heard(before: Vec<Value>, path: Value, hears: Option<Value>) -> Campaign {
    campaign(|w, q| {
        w["content"]["atmospheres"] = json!([hush(), dread()]);
        w["content"]["areas"][0]["atmosphere"] = json!("atmosphere/hush");
        talk(q).extend(before);
        talk(q).push(cutscene(path, hears));
        // The dread is painted somewhere by something, or it is `DW0930`.
        if !talk(q).iter().any(|e| e["type"] == "set-atmosphere") {
            talk(q).push(
                json!({ "type": "set-atmosphere", "atmosphere": "atmosphere/dread",
                "region": { "anchor": "anchor/exit", "extent": [0, 0, 0] } }),
            );
        }
    })
}

fn repaint_dread() -> Value {
    json!({ "type": "set-atmosphere", "atmosphere": "atmosphere/dread",
        "region": { "anchor": "anchor/keeper-stand", "extent": [1, 1, 1] } })
}

/// Inside the keeper's 4-cell column, both waypoints.
fn inside() -> Value {
    json!([mark([0, 1, 0]), mark([1, 1, 0])])
}

#[test]
fn a_camera_that_stays_in_the_declared_place_is_confirmed() {
    let out = try_build(&heard(vec![], inside(), Some(json!("atmosphere/hush"))))
        .expect("the camera stays in the hush");
    let l = ledger(&out);
    assert_eq!(l["cutscenes"][0]["verdict"], json!("confirmed"));
    assert_eq!(l["cutscenes"][0]["cells_judged"], json!(1));
    assert!(
        !out.keys()
            .any(|k| k.contains("/test/") && k.contains("hears")),
        "no PackTest re-proves the hearing (spec-0109 §5.4)"
    );
}

#[test]
fn a_waypoint_moved_into_another_place_is_dw1032() {
    let path = json!([mark([0, 1, 0]), mark([3, 1, 0])]);
    let e = try_build(&heard(
        vec![repaint_dread()],
        path,
        Some(json!("atmosphere/dread")),
    ))
    .expect_err("one waypoint stands in the hush");
    assert_eq!(failure_code(&e), "DW1032");
    let m = failure_message(&e);
    assert!(m.contains("shot 0") && m.contains("[2, 16, 1]"), "{m}");
    assert!(m.contains("hello-world:atmosphere/hush"), "{m}");
}

/// The repaint before the beat decides what is judged: the same camera hears
/// the dread only after the repaint the forced route fires ahead of it.
#[test]
fn a_repaint_before_the_beat_changes_what_is_heard() {
    let e = try_build(&heard(vec![], inside(), Some(json!("atmosphere/dread"))))
        .expect_err("without the repaint the keep is the hush");
    assert_eq!(failure_code(&e), "DW1032", "{}", failure_message(&e));
    let out = try_build(&heard(
        vec![repaint_dread()],
        inside(),
        Some(json!("atmosphere/dread")),
    ))
    .expect("after the repaint the camera hears the dread");
    let l = ledger(&out);
    assert_eq!(l["cutscenes"][0]["verdict"], json!("confirmed"));
    assert_eq!(
        l["cutscenes"][0]["configuration"]["repaints"],
        json!(["/content/quests/0/on_objective_complete/obj/talk/1"])
    );
}

#[test]
fn a_camera_over_the_ground_hears_null() {
    let out = try_build(&heard(
        vec![],
        json!([mark([0, 30, 0]), mark([20, 30, 0])]),
        Some(Value::Null),
    ))
    .expect("the camera stands over the void ground");
    let l = ledger(&out);
    assert_eq!(l["cutscenes"][0]["verdict"], json!("confirmed"));
    assert_eq!(l["cutscenes"][0]["hears"], Value::Null);
}

#[test]
fn hearing_an_undeclared_atmosphere_is_dw0112() {
    let c = heard(vec![], inside(), Some(json!("atmosphere/nowhere")));
    let d = validate_campaign(&c);
    assert!(
        d.iter()
            .any(|d| d.code == "DW0112" && d.path.ends_with("/hears")),
        "{:#?}",
        codes(&d)
    );
}

// --- criterion 6: determinism --------------------------------------------------------

#[test]
fn a_volume_spelling_moves_no_byte() {
    let spelled = |v: Value| {
        let c = campaign(|w, _| {
            w["content"]["horizon"]["attributes"]["audio/music_volume"] = v;
        });
        try_build(&c).expect("builds")
    };
    assert_eq!(
        spelled(json!(0.0)),
        spelled(json!(0)),
        "`0.0` and `0` are one value"
    );
}

// --- criterion 8: the binding line -----------------------------------------------------

/// Every build prints the line, zeroes included, and writes the ledger with
/// the hash of the declarations it read.
#[test]
fn every_build_prints_the_music_binding_line() {
    let out_dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("music-binding-line");
    let _ = std::fs::remove_dir_all(&out_dir);
    let r = std::process::Command::new(BIN)
        .args([
            "build",
            common::hello_world_dir().to_str().unwrap(),
            "-o",
            out_dir.to_str().unwrap(),
            "--prefabs",
            common::prefabs_dir().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&r.stderr);
    assert!(r.status.success(), "{err}");
    let line = err
        .lines()
        .find(|l| l.starts_with("music binding:"))
        .unwrap_or_else(|| panic!("no binding line: {err}"));
    assert_eq!(
        line,
        "music binding: 1 biome(s) a camera can stand in — 0 atmosphere(s) and the ground \
         declared (0 with a track, 1 silent: 0 fading, 1 playing out), 0 band biome(s) \
         undeclared; 0 cutscene(s), 0 declare what they hear, 0 of 0 confirmed, 0 declare \
         their music over 0 painted 4-cell(s) in 0 paint line(s) and 0 variant biome(s); 0 \
         stop-sound effect(s), 0 of 0 cut a play-out, 0 a stinger, 0 a hiccup; 0 sound(s) in \
         a category other than master"
    );
    let l: Value = serde_json::from_str(
        &std::fs::read_to_string(out_dir.join("validation/music.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(l["biomes"][0]["kind"], json!("ground"));
    assert_eq!(l["biomes"][0]["entering"], json!("plays-out-at-gain"));
    assert_eq!(l["measured_sha256"].as_str().map(str::len), Some(64));
    // The fields the withdrawn live checks consumed are gone (spec-0109 §5.4).
    let text = l.to_string();
    for gone in ["\"station\"", "\"after\"", "\"performed\"", "\"not_stood\""] {
        assert!(!text.contains(gone), "{gone} in {text}");
    }
}

/// The words of the cut: a spaced or silent scene is cut, a looping one
/// hiccups, and a category the music manager does not play is a stinger's cut.
#[test]
fn each_stop_sound_says_what_it_cuts_where_it_fires() {
    let c = campaign(|w, q| {
        w["content"]["atmospheres"] = json!([dread()]);
        w["content"]["areas"][0]["atmosphere"] = json!("atmosphere/dread");
        talk(q).extend([
            json!({ "type": "stop-sound", "category": "music" }),
            json!({ "type": "stop-sound", "category": "ambient", "sound": "minecraft:ambient.cave" }),
        ]);
        q["content"]["quests"][0]["on_objective_complete"]["obj/exit"] = json!([
            { "type": "set-atmosphere", "atmosphere": null, "place": "area/keep" },
            { "type": "stop-sound" }
        ]);
    });
    let out = try_build(&c).expect("builds");
    let l = ledger(&out);
    let cuts: Vec<(&str, &str)> = l["stop_sounds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["biome"].as_str().unwrap(), r["cut"].as_str().unwrap()))
        .collect();
    // In the effect walk's order: `obj/exit`'s bundle sorts before
    // `obj/talk`'s.
    assert_eq!(
        cuts,
        [
            ("hello-world:void", "cut"),
            ("hello-world:atmosphere/dread", "hiccup"),
            ("hello-world:atmosphere/dread", "stinger"),
        ]
    );
}

// --- spec-0109 §3.6: a cutscene's own music -------------------------------------------

/// A single-file track that replaces at once, the recommended score.
fn lament() -> Value {
    json!({ "audio/background_music": { "default": {
        "sound": "minecraft:music_disc.pigstep",
        "min_delay": 0, "max_delay": 0, "replace_current_music": true
    } } })
}

/// A cutscene over `path` that declares `music`; a `path` that is a list of
/// paths is one shot each.
fn scored(path: Value, music: Value) -> Value {
    let mut c = if path[0].is_array() {
        json!({ "type": "cutscene", "shots": path.as_array().unwrap().iter()
            .map(|p| json!({ "path": p, "seconds": 2 })).collect::<Vec<_>>() })
    } else {
        cutscene(path, None)
    };
    c["music"] = music;
    c
}

/// hello-world whose keep carries the hush and whose keeper's cell the talk
/// beat repaints with the dread, then plays `cutscenes`.
fn hall(cutscenes: Vec<Value>) -> Campaign {
    campaign(|w, q| {
        w["content"]["atmospheres"] = json!([hush(), dread()]);
        w["content"]["areas"][0]["atmosphere"] = json!("atmosphere/hush");
        talk(q).push(repaint_dread());
        talk(q).extend(cutscenes);
    })
}

/// Two shots: one at the keeper's cell, one over the ground above the keep's
/// paint.
fn rising() -> Value {
    json!([
        [mark([0, 1, 0]), mark([0, 2, 0])],
        [mark([0, 30, 0]), mark([4, 30, 0])]
    ])
}

/// A cutscene's `music` states its music by the one rule (`DW1031`, §6.1).
#[test]
fn a_cutscene_that_scores_nothing_is_dw1031() {
    let one = |music: Value| {
        let c = campaign(|_, q| talk(q).push(scored(inside(), music)));
        let d: Vec<Diagnostic> = validate_campaign(&c)
            .into_iter()
            .filter(|d| d.code == "DW1031")
            .collect();
        assert_eq!(d.len(), 1, "{d:#?}");
        assert_eq!(d[0].stage, "quests");
        d[0].clone()
    };
    let d = one(json!({}));
    assert!(d.path.ends_with("/music"), "{}", d.path);
    assert!(d.message.contains("`{}` for silence") && d.message.contains("\"default\""));
    let d = one(json!({ "audio/background_music": {
        "creative": { "sound": "minecraft:music.creative", "min_delay": 0, "max_delay": 0 } } }));
    assert!(d.path.ends_with("/creative"), "{}", d.path);
    let mut sky = lament();
    sky["visual/sky_color"] = json!("#7ba4ff");
    let d = one(sky);
    assert!(d.path.ends_with("/visual~1sky_color"), "{}", d.path);
    // Stated: a track, or a scored silence with a gain.
    for music in [
        lament(),
        json!({ "audio/background_music": {}, "audio/music_volume": 0.0 }),
    ] {
        let c = campaign(|_, q| talk(q).push(scored(inside(), music)));
        assert!(
            !validate_campaign(&c)
                .iter()
                .chain(delvec::compiler::atmosphere::check(&c).iter())
                .any(|d| d.code == "DW1031" || d.code == "DW0928"),
            "a stated score validates"
        );
    }
    // A value outside its shape is the attribute reader's `DW0928`.
    let c = campaign(|_, q| {
        talk(q).push(scored(
            inside(),
            json!({ "audio/background_music": {}, "audio/music_volume": 2.0 }),
        ))
    });
    let d = delvec::compiler::atmosphere::check(&c);
    assert!(
        d.iter()
            .any(|d| d.code == "DW0928" && d.path.ends_with("/music/audio~1music_volume")),
        "{:#?}",
        codes(&d)
    );
}

/// Criterion 9: one variant per base the camera's cells can wear — the hush
/// the keep carries, the dread the beat repaints, the ground above — each the
/// base's emitted definition with the two music ids swapped; a base's
/// `music_volume: 0.0` is not carried over; two cutscenes with one declaration
/// emit one set.
#[test]
fn a_score_derives_one_variant_per_base() {
    let out = try_build(&hall(vec![scored(rising(), lament())])).expect("builds");
    let variants: Vec<&String> = out
        .keys()
        .filter(|k| k.starts_with("datapack/data/hello-world/worldgen/biome/music/"))
        .collect();
    assert_eq!(variants.len(), 3, "{variants:#?}");
    let digest = variants[0]
        .trim_start_matches("datapack/data/hello-world/worldgen/biome/music/")
        .split('/')
        .next()
        .unwrap()
        .to_string();
    assert_eq!(digest.len(), 8);
    let biome = |p: &str| -> Value { serde_json::from_str(&file(&out, p)).unwrap() };
    for base in ["atmosphere/hush", "atmosphere/dread", "void"] {
        let v = biome(&format!(
            "datapack/data/hello-world/worldgen/biome/music/{digest}/{base}.json"
        ));
        let b = biome(&format!(
            "datapack/data/hello-world/worldgen/biome/{base}.json"
        ));
        let strip = |mut x: Value| {
            let a = x["attributes"].as_object_mut().unwrap();
            a.remove("minecraft:audio/background_music");
            a.remove("minecraft:audio/music_volume");
            x
        };
        assert_eq!(strip(v.clone()), strip(b), "{base}: field for field");
        assert_eq!(
            v["attributes"]["minecraft:audio/background_music"]["default"]["sound"],
            json!("minecraft:music_disc.pigstep"),
            "{base}"
        );
        assert!(
            v["attributes"]
                .get("minecraft:audio/music_volume")
                .is_none(),
            "{base}: the hush's 0.0 is not carried over"
        );
    }
    // A second cutscene with the same declaration shares the set.
    let two = try_build(&hall(vec![
        scored(rising(), lament()),
        scored(json!([[mark([1, 1, 0])], [mark([8, 30, 0])]]), lament()),
    ]))
    .expect("builds");
    let n = two
        .keys()
        .filter(|k| k.starts_with("datapack/data/hello-world/worldgen/biome/music/"))
        .count();
    assert_eq!(n, 3);
    // The variants join the void biome's tags with their bases.
    let tags = file(
        &out,
        "datapack/data/minecraft/tags/worldgen/biome/without_wandering_trader_spawns.json",
    );
    assert!(
        tags.contains(&format!("hello-world:music/{digest}/void")),
        "{tags}"
    );
}

fn function(out: &BuildOutput, name: &str) -> Vec<String> {
    let (_, body) = out
        .iter()
        .find(|(p, _)| {
            p.starts_with("datapack/data/hello-world/function/")
                && p.ends_with(&format!("/{name}.mcfunction"))
        })
        .unwrap_or_else(|| panic!("`{name}` is emitted"));
    std::str::from_utf8(body)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect()
}

fn function_named(out: &BuildOutput, prefix: &str) -> (String, Vec<String>) {
    let (p, body) = out
        .iter()
        .find(|(p, _)| {
            p.starts_with("datapack/data/hello-world/function/")
                && p.rsplit('/').next().unwrap().starts_with(prefix)
        })
        .unwrap_or_else(|| panic!("a `{prefix}…` function is emitted"));
    (
        p.rsplit('/')
            .next()
            .unwrap()
            .trim_end_matches(".mcfunction")
            .to_string(),
        std::str::from_utf8(body)
            .unwrap()
            .lines()
            .map(str::to_string)
            .collect(),
    )
}

/// Criterion 10: the bracket — the paint latches then repaints with `replace`,
/// the restore drops the latch and re-arms it on a failed line, the start
/// paints before the camera is taken, the end restores after the players
/// return, the tick and `load` carry their lines; a campaign with no score
/// emits none of it.
#[test]
fn the_score_is_painted_and_restored_by_the_bracket() {
    let out = try_build(&hall(vec![scored(rising(), lament())])).expect("builds");
    let (paint_name, paint) = function_named(&out, "cs_paint_");
    let bare = paint_name.trim_start_matches("cs_paint_").to_string();
    assert!(
        bare.contains("_m"),
        "the score's digest keys the bracket: {bare}"
    );
    assert_eq!(
        paint[0],
        format!("scoreboard players set #cs_paint_{bare} dw.sys 1")
    );
    assert!(paint.len() > 1);
    for l in &paint[1..] {
        assert!(
            l.starts_with("fillbiome ") && l.contains(" replace "),
            "{l}"
        );
        let v = delvec::compiler::atmosphere::line_volume(l).unwrap();
        assert!(v <= delvec::compiler::atmosphere::FILLBIOME_CAP, "{l}");
        // `fillbiome <box> <variant> replace <base>`.
        let t: Vec<&str> = l.split(' ').collect();
        assert!(t[7].starts_with("hello-world:music/"), "{l}");
        assert!(
            t[7].ends_with(t[9].trim_start_matches("hello-world:")),
            "{l}"
        );
    }
    let unpaint = function(&out, &format!("cs_unpaint_{bare}"));
    assert_eq!(
        unpaint[0],
        format!("scoreboard players set #cs_paint_{bare} dw.sys 0")
    );
    assert_eq!(unpaint.len(), 1 + 2 * (paint.len() - 1));
    for (k, l) in paint[1..].iter().enumerate() {
        let t: Vec<&str> = l.split(' ').collect();
        let reversed = format!(
            "{} {} {} {} {} {} {} {} replace {}",
            t[0], t[1], t[2], t[3], t[4], t[5], t[6], t[9], t[7]
        );
        assert_eq!(
            unpaint[1 + 2 * k],
            format!("execute store success score #cs_ok dw.sys run {reversed}")
        );
        assert_eq!(
            unpaint[2 + 2 * k],
            format!(
                "execute if score #cs_ok dw.sys matches 0 run scoreboard players set #cs_paint_{bare} dw.sys 1"
            )
        );
    }
    let start = function(&out, &format!("cs_{bare}"));
    let at = |v: &[String], line: &str| v.iter().position(|l| l == line);
    assert!(
        at(&start, &format!("function hello-world:cs_paint_{bare}")).unwrap()
            < at(&start, "gamemode spectator @a").unwrap()
    );
    let end = function(&out, &format!("cs_end_{bare}"));
    let tp = end.iter().position(|l| l.starts_with("tp @a ")).unwrap();
    assert!(at(&end, &format!("function hello-world:cs_unpaint_{bare}")).unwrap() > tp);
    let tick = function(&out, "tick");
    assert!(tick.contains(&"function hello-world:cs_unpaint_tick".to_string()));
    assert_eq!(
        function(&out, "cs_unpaint_tick"),
        [format!(
            "execute if score #cs_paint_{bare} dw.sys matches 1 unless score #run_{bare} dw.sys matches 1 run function hello-world:cs_unpaint_{bare}"
        )]
    );
    let load = function(&out, "load");
    assert_eq!(
        load.last().unwrap(),
        &format!(
            "execute unless score #run_{bare} dw.sys matches 1 run scoreboard players set #cs_paint_{bare} dw.sys 1"
        )
    );

    // Without the score: no paint, no variant, no tick line, a two-line load,
    // and the cutscene's bracket under its old name.
    let mut unscored = scored(rising(), lament());
    unscored.as_object_mut().unwrap().remove("music");
    let plain = try_build(&hall(vec![unscored])).expect("builds");
    assert!(!plain.keys().any(|k| k.contains("cs_paint_")
        || k.contains("cs_unpaint")
        || k.contains("/worldgen/biome/music/")));
    assert!(
        !function(&plain, "tick")
            .iter()
            .any(|l| l.contains("cs_unpaint"))
    );
    assert_eq!(function(&plain, "load").len(), 2);
    let (shots_key, _) = bare.rsplit_once("_m").unwrap();
    function(&plain, &format!("cs_{shots_key}"));
}

/// The painted cells are the ledger's, every one inside the map, and a shot
/// that leaves the map's extent is `DW0929`.
#[test]
fn a_score_beyond_the_map_is_dw0929() {
    let e = try_build(&hall(vec![scored(
        json!([mark([0, 30, 0]), mark([400, 30, 0])]),
        lament(),
    )]))
    .expect_err("the camera flies past the map");
    assert_eq!(failure_code(&e), "DW0929", "{}", failure_message(&e));
    assert!(failure_message(&e).contains("own `music`"));
}

/// Criterion 5's last clause: `hears` is judged over the places themselves,
/// before the cutscene's own paint.
#[test]
fn a_scored_cutscene_hears_the_place_under_its_score() {
    let mut c = scored(inside(), lament());
    c["hears"] = json!("atmosphere/hush");
    let out = try_build(&campaign(|w, q| {
        w["content"]["atmospheres"] = json!([hush(), dread()]);
        w["content"]["areas"][0]["atmosphere"] = json!("atmosphere/hush");
        talk(q).push(
            json!({ "type": "set-atmosphere", "atmosphere": "atmosphere/dread",
            "region": { "anchor": "anchor/exit", "extent": [0, 0, 0] } }),
        );
        talk(q).push(c);
    }))
    .expect("the camera's place is the hush, whatever the score");
    let l = ledger(&out);
    assert_eq!(l["cutscenes"][0]["verdict"], json!("confirmed"));
    let m = &l["cutscenes"][0]["music"];
    assert_eq!(m["entering"], json!("replaces"));
    assert_eq!(m["reaches"], json!(["area/keep"]));
    assert_eq!(
        m["way_out"],
        json!([{ "base": "hello-world:atmosphere/hush", "entering": "fades" }])
    );
    let line = &l["biomes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|b| b["kind"] == "variant")
        .count();
    assert_eq!(*line, 1);
}

/// Criterion 6: a volume spelled `0` or `0.0` in a cutscene's `music` is one
/// declaration — no byte moves, the variant ids included.
#[test]
fn a_score_volume_spelling_moves_no_byte() {
    let spelled = |v: Value| {
        let mut m = lament();
        m["audio/music_volume"] = v;
        try_build(&hall(vec![scored(rising(), m)])).expect("builds")
    };
    assert_eq!(spelled(json!(0.0)), spelled(json!(0)));
}

/// Criterion 8: the line counts the scores, and the ledger carries each
/// variant with its base and deriver.
#[test]
fn the_ledger_carries_the_score() {
    let out = try_build(&hall(vec![scored(rising(), lament())])).expect("builds");
    let l = ledger(&out);
    let variants: Vec<&Value> = l["biomes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|b| b["kind"] == "variant")
        .collect();
    assert_eq!(variants.len(), 3);
    for v in &variants {
        assert!(v["base"].is_string() && v["derived_by"].as_array().unwrap().len() == 1);
        assert_eq!(v["entering"], json!("replaces"));
    }
    let m = &l["cutscenes"][0]["music"];
    let cells = m["painted_cells"].as_array().unwrap().len();
    assert!(cells > 1);
    let lines = m["paint_lines"].as_u64().unwrap();
    assert_eq!(
        lines as usize,
        function_named(&out, "cs_paint_").1.len() - 1,
        "the ledger's line count is the function's"
    );
}

/// Criterion 11: one `cutscene_music_<n>` per declaring cutscene, running the
/// real functions, claimed per object (`DW0811`).
#[test]
fn every_scored_cutscene_owns_its_template() {
    let c = hall(vec![cutscene(inside(), None), scored(rising(), lament())]);
    let out = try_build(&c).expect("builds");
    let t = file(
        &out,
        "packtest-datapack/data/hello-world/test/cutscene_music_1.mcfunction",
    );
    let (paint_name, _) = function_named(&out, "cs_paint_");
    let bare = paint_name.trim_start_matches("cs_paint_");
    for want in [
        format!("function hello-world:cs_paint_{bare}"),
        format!("function hello-world:cs_unpaint_{bare}"),
        "function hello-world:cs_unpaint_tick".to_string(),
        "function hello-world:load".to_string(),
        format!("assert score #cs_paint_{bare} dw.sys matches 0"),
        format!("assert score #cs_paint_{bare} dw.sys matches 1"),
        "execute if biome 6 66 6 hello-world:atmosphere/dread".to_string(),
    ] {
        assert!(t.contains(&want), "{want}: {t}");
    }
    assert!(!out.keys().any(|k| k.contains("/test/cutscene_music_0")));
    let dir = common::prefabs_dir();
    let prefabs = PrefabRegistry::load_dir(&dir).unwrap();
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let claim = delvec::compiler::watch::Claim {
        mechanic: "cutscene-music",
        kind: delvec::compiler::watch::ClaimKind::Template,
        families: vec!["cutscene_music_".to_string()],
        declared: delvec::compiler::music::scored_ids(&plan),
    };
    assert_eq!(claim.declared.iter().collect::<Vec<_>>(), ["1"]);
    let (binding, breaches) =
        delvec::compiler::watch::check_claims("hello-world", &out, std::slice::from_ref(&claim));
    assert_eq!((binding.bodies_judged, breaches.len()), (1, 0));
    let mut without = out.clone();
    without.remove("packtest-datapack/data/hello-world/test/cutscene_music_1.mcfunction");
    let (binding, breaches) =
        delvec::compiler::watch::check_claims("hello-world", &without, &[claim]);
    assert_eq!((binding.bodies_judged, breaches.len()), (1, 1));
}

/// Endings × music: a cutscene that declares its own `music` and a `then`
/// that changes the world (spec-0108 §3.6, spec-0109 §3.6) emits both. Its
/// one name carries both suffixes, `_then_` first; the start paints the score
/// before the camera is taken; the end restores it after the players return
/// and runs the world change after that. Each half dropped leaves the other's
/// bytes and removes only its own.
#[test]
fn a_scored_cutscene_with_a_then_emits_the_score_and_the_world_change() {
    let then =
        json!([{ "type": "set-block", "anchor": "anchor/exit", "block": "minecraft:light" }]);
    let both = {
        let mut c = scored(inside(), lament());
        c["then"] = then.clone();
        c
    };
    let out = try_build(&hall(vec![both])).expect("builds");
    let (paint_name, _) = function_named(&out, "cs_paint_");
    let bare = paint_name.trim_start_matches("cs_paint_").to_string();
    let (t, m) = (bare.find("_then_"), bare.rfind("_m"));
    assert!(
        matches!((t, m), (Some(t), Some(m)) if t < m),
        "one name, the `then` digest before the score's: {bare}"
    );
    let at = |f: &[String], needle: &str| f.iter().position(|l| l.contains(needle));
    let start = function(&out, &format!("cs_{bare}"));
    assert!(
        at(&start, &format!("function hello-world:cs_paint_{bare}")).unwrap()
            < at(&start, "gamemode spectator @a").unwrap()
    );
    let end = function(&out, &format!("cs_end_{bare}"));
    let tp = end.iter().position(|l| l.starts_with("tp @a ")).unwrap();
    let unpaint = at(&end, &format!("function hello-world:cs_unpaint_{bare}")).unwrap();
    let change = end
        .iter()
        .position(|l| l.starts_with("setblock ") && l.ends_with(" minecraft:light"))
        .unwrap_or_else(|| panic!("the `then` world change runs from `cs_end`: {end:#?}"));
    assert!(tp < unpaint && tp < change, "{end:#?}");

    // Without the `then`: the score's bracket stands, no world change, no `_then_`.
    let out = try_build(&hall(vec![scored(inside(), lament())])).expect("builds");
    let (paint_name, _) = function_named(&out, "cs_paint_");
    let bare = paint_name.trim_start_matches("cs_paint_").to_string();
    assert!(!bare.contains("_then_"), "{bare}");
    let end = function(&out, &format!("cs_end_{bare}"));
    assert!(
        !end.iter().any(|l| l.ends_with(" minecraft:light")),
        "{end:#?}"
    );

    // Without the score: the world change stands, no paint, no `_m`.
    let mut plain = cutscene(inside(), None);
    plain["then"] = then;
    let out = try_build(&hall(vec![plain])).expect("builds");
    assert!(!out.keys().any(|k| k.contains("cs_paint_")));
    let (end_name, end) = function_named(&out, "cs_end_");
    assert!(
        end_name.contains("_then_") && !end_name.contains("_m"),
        "{end_name}"
    );
    assert!(
        end.iter()
            .any(|l| l.starts_with("setblock ") && l.ends_with(" minecraft:light")),
        "{end:#?}"
    );
}
