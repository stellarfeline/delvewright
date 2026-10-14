//! spec-0109 — a scene states its music: every biome a camera can stand in
//! declares a track or silence (`DW1031`), the ground states its own, a story
//! stage moves the music by repainting the place, `stop-sound` is the cut, a
//! played sound names its category, and a cutscene's `hears` is confirmed
//! against the biome its camera passes (`DW1032`).
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
    assert_eq!(delvewright_dsl::DSL_VERSION, "0.47.0");
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
        .filter(|d| d.code.to_string() == "DW1031")
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
            "attributes": { "visual/sky_color": "#123456" } }]);
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
        .filter(|d| d.code.to_string() == "DW1031")
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
        .filter(|d| d.code.to_string() == "DW1031" || d.code.to_string() == "DW0928")
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
            .any(|l| l.contains("playsound minecraft:music.dragon music @a")),
        "{plays:#?}"
    );
    assert!(
        plays
            .iter()
            .any(|l| l.contains("playsound minecraft:block.bell.use master @a")),
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
    let t = file(
        &out,
        "packtest-datapack/data/hello-world/test/music_hears_0.mcfunction",
    );
    assert!(
        t.contains("execute if biome 6 66 6 hello-world:atmosphere/hush"),
        "the 2-mod-4 sample is read: {t}"
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
    let t = file(
        &out,
        "packtest-datapack/data/hello-world/test/music_hears_0.mcfunction",
    );
    assert!(
        t.contains("execute unless biome") && t.contains("hello-world:atmosphere/hush"),
        "a ground cell outside every paint reads that no atmosphere stands there: {t}"
    );
}

#[test]
fn hearing_an_undeclared_atmosphere_is_dw0112() {
    let c = heard(vec![], inside(), Some(json!("atmosphere/nowhere")));
    let d = validate_campaign(&c);
    assert!(
        d.iter()
            .any(|d| d.code.to_string() == "DW0112" && d.path.ends_with("/hears")),
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
/// the hash of the documents it read.
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
        "music binding: 1 biome(s) a camera can stand in — 1 atmosphere(s) and the ground \
         declared (0 with a track, 1 silent: 0 fading, 1 playing out), 0 band biome(s) \
         undeclared; 0 cutscene(s), 0 declare what they hear, 0 of 0 confirmed; 0 \
         stop-sound effect(s), 0 of 0 cut a play-out, 0 a stinger, 0 a hiccup; 0 sound(s) in \
         a category other than master"
    );
    let l: Value = serde_json::from_str(
        &std::fs::read_to_string(out_dir.join("validation/music.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(l["biomes"][0]["kind"], json!("ground"));
    assert_eq!(l["biomes"][0]["entering"], json!("plays-out-at-gain"));
    for doc in ["world", "quests", "dialogue"] {
        assert_eq!(
            l["documents"][doc].as_str().map(str::len),
            Some(64),
            "{doc}"
        );
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
