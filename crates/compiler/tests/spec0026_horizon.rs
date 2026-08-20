//! spec-0026 foundation fixtures: the per-area `walk_y` placement datum
//! (`DW0367`), the empirical flood-level proof (`DW0364` — the tide-mill
//! class as a permanent red fixture), the flatland ambient emission, and the
//! double-build byte-identity gates for the landed horizon kinds (acceptance
//! criteria 1/2/3).

mod common;

use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn delvec(args: &[&str]) -> Output {
    Command::new(BIN).args(args).output().expect("run delvec")
}

fn code(out: &Output) -> i32 {
    out.status.code().unwrap_or(-1)
}

fn tmp(name: &str) -> std::path::PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn read_tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut map = BTreeMap::new();
    fn walk(base: &Path, dir: &Path, map: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(base, &path, map);
            } else {
                let rel = path
                    .strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                map.insert(rel, std::fs::read(&path).unwrap());
            }
        }
    }
    walk(root, root, &mut map);
    map
}

/// Materialize hello-world with a patched stage-1 world document. `horizon` is
/// raw JSON (string or object form); `version` is the world stage's
/// `dsl_version`.
fn campaign_with_horizon(name: &str, version: &str, horizon: &str) -> std::path::PathBuf {
    let camp = tmp(name);
    common::copy_dir_all(&common::hello_world_dir(), &camp);
    let mut world: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(camp.join("world.json")).unwrap()).unwrap();
    world["dsl_version"] = serde_json::json!(version);
    let content = world["content"].as_object_mut().unwrap();
    content.insert("horizon".into(), serde_json::from_str(horizon).unwrap());
    content.insert("boundary".into(), serde_json::json!({ "margin": 20 }));
    std::fs::write(
        camp.join("world.json"),
        serde_json::to_string_pretty(&world).unwrap(),
    )
    .unwrap();
    camp
}

fn build_into(camp: &Path, out: &Path, prefabs: &Path) -> Output {
    delvec(&[
        "build",
        camp.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        prefabs.to_str().unwrap(),
        "--json",
    ])
}

/// Set (or remove) `walk_y` in one prefab metadata file of a private copy.
fn set_walk_y(prefabs: &Path, prefab: &str, walk_y: Option<i64>) {
    let path = prefabs.join(format!("{prefab}.json"));
    let mut meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    match walk_y {
        Some(v) => {
            meta["walk_y"] = serde_json::json!(v);
        }
        None => {
            meta.as_object_mut().unwrap().remove("walk_y");
        }
    }
    std::fs::write(&path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();
}

/// `DW0367` (spec-0026 §2): a piece placed in a non-void horizon whose prefab
/// metadata declares no `walk_y` is a build error — the compiler refuses to
/// guess a tileset datum (the folklore that flooded the tide mill).
#[test]
fn missing_walk_y_in_ocean_exits_3_with_dw0367() {
    let prefabs = tmp("dw0367-prefabs");
    common::copy_dir_all(&common::prefabs_dir(), &prefabs);
    set_walk_y(&prefabs, "hello-room", None);

    let camp = campaign_with_horizon("dw0367-camp", "0.6.0", "\"ocean\"");
    let out = tmp("dw0367-out");
    let b = build_into(&camp, &out, &prefabs);
    assert_eq!(code(&b), 3, "missing walk_y should exit 3");
    let stdout = String::from_utf8_lossy(&b.stdout);
    assert!(stdout.contains("DW0367"), "expected DW0367:\n{stdout}");

    // The same library in a VOID world needs no datum: builds clean.
    let void_camp = tmp("dw0367-void-camp");
    common::copy_dir_all(&common::hello_world_dir(), &void_camp);
    let out_void = tmp("dw0367-void-out");
    let v = build_into(&void_camp, &out_void, &prefabs);
    assert_eq!(
        code(&v),
        0,
        "void world needs no walk_y: {}",
        String::from_utf8_lossy(&v.stdout)
    );
}

/// The tide-mill class as a permanent fixture (spec-0026 acceptance
/// criterion 2): an interior piece mis-datumed by tileset folklore — `walk_y`
/// declared as the island convention (3) while its real walk plane is local 1,
/// and **no `waterline_y`**, so `DW0344` never looks — lands its standable
/// cells at world y=61, one block under the sea. Before spec-0026 this built
/// GREEN and flooded on first boot; now the empirical flood proof rejects it
/// (`DW0364`, exit 3, no exemption). Correcting the datum declaration to what
/// the piece really authors (walk_y=1 → base 62) makes the SAME content green
/// and dry, and the emitted base y is asserted (criterion 2's red→green pair).
#[test]
fn flooded_interior_is_dw0364_and_corrected_datum_is_green_and_dry() {
    let prefabs = tmp("dw0364-prefabs");
    common::copy_dir_all(&common::prefabs_dir(), &prefabs);
    // hello-room's metadata declares no waterline_y (interior piece) — assert
    // that stays true, because the DW0344 gap this fixture guards is exactly
    // "no waterline, so nothing ever looked".
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(prefabs.join("hello-room.json")).unwrap())
            .unwrap();
    assert!(
        meta.get("waterline_y").is_none(),
        "fixture premise: the piece declares no waterline_y"
    );
    set_walk_y(&prefabs, "hello-room", Some(3));

    let camp = campaign_with_horizon("dw0364-camp", "0.6.0", "\"ocean\"");
    let out = tmp("dw0364-out");
    let b = build_into(&camp, &out, &prefabs);
    assert_eq!(code(&b), 3, "flooded interior should exit 3");
    let stdout = String::from_utf8_lossy(&b.stdout);
    assert!(stdout.contains("DW0364"), "expected DW0364:\n{stdout}");

    // Correct the datum declaration: the piece's real walk plane is local 1.
    set_walk_y(&prefabs, "hello-room", Some(1));
    let out_ok = tmp("dw0364-out-ok");
    let ok = build_into(&camp, &out_ok, &prefabs);
    assert_eq!(
        code(&ok),
        0,
        "corrected walk_y must build dry: {}",
        String::from_utf8_lossy(&ok.stdout)
    );
    // The emitted base y is the datum equation's: walk_ref 63 − walk_y 1 = 62,
    // landing the walk plane at 63 — one block above the sea.
    let place = std::fs::read_to_string(
        out_ok.join("datapack/data/hello-world/function/place_all.mcfunction"),
    )
    .unwrap();
    assert!(
        place.contains("place template hello-world:hello-room 0 62 0"),
        "corrected datum must place at y=62:\n{place}"
    );
}

/// Acceptance criterion 3: the ocean ISLAND datum is unchanged — a tileset
/// declaring the island convention (`walk_y: 3`, the real island/tk-shore
/// value) keeps base y = 60, exactly the old global `OCEAN_BASE_Y`. The datum
/// rework must not move the island. Asserted against the real content
/// library's `island-beach-camp` (walk_y 3) via a plan-level build of a
/// minimal campaign binding it.
#[test]
fn island_walk_y_3_keeps_base_60() {
    use delvewright_compiler::plan::Plan;
    use delvewright_compiler::registry::PrefabRegistry;

    let world = r#"{
      "dsl_version": "0.6.0",
      "campaign_id": "island-datum",
      "stage": "world",
      "content": {
        "title": "Datum probe",
        "theme": "An island that must not move.",
        "premise": "Placement is a proof, not a habit.",
        "seed": 20260804,
        "target_minutes": 5,
        "horizon": "ocean",
        "boundary": { "margin": 20 },
        "areas": [
          { "id": "area/island", "name": "The Island", "prefab": "prefab/island-beach-camp" }
        ]
      }
    }"#;
    let stage = |stage: &str, content: &str| -> String {
        format!(
            r#"{{ "dsl_version": "0.2.0", "campaign_id": "island-datum",
                 "stage": "{stage}", "content": {content} }}"#
        )
    };
    let raw = delvewright_dsl::RawCampaign {
        world: world.to_string(),
        npcs: stage("npcs", r#"{ "npcs": [] }"#),
        classes: stage("classes", r#"{ "classes": [] }"#),
        quest_plan: stage("quest-plan", r#"{ "quests": [], "finale": "quest/none" }"#),
        quests: stage("quests", r#"{ "quests": [] }"#),
        dialogue: stage("dialogue", r#"{ "dialogues": [] }"#),
        world_edits: None,
    };
    let campaign = delvewright_dsl::parse_campaign(&raw).expect("parse");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).expect("prefabs");
    let plan = Plan::build(&campaign, &prefabs).expect("plan");
    assert_eq!(
        plan.areas[0].pieces[0].pos,
        [0, 60, 0],
        "island walk_y=3 must keep the historical base y=60 (walk_ref 63 − 3)"
    );
}

/// The flatland foundation (spec-0026 §1/§3 ambient half): builds clean at
/// 0.12.0, ships the pinned bedrock/dirt/grass `generator-settings` with the
/// plains biome, places the scene on the datum (walk_ref 64 − walk_y 1 = 63,
/// so the walk plane sits exactly one block over the grass), and a double
/// build is byte-identical (acceptance criterion 1 for the flatland kind).
#[test]
fn flatland_builds_on_the_datum_byte_identical() {
    let pf = common::prefabs_dir();
    let camp = campaign_with_horizon("flatland-camp", "0.12.0", "\"flatland\"");
    let out_a = tmp("flatland-a");
    let out_b = tmp("flatland-b");
    for out in [&out_a, &out_b] {
        let r = build_into(&camp, out, &pf);
        assert_eq!(
            code(&r),
            0,
            "flatland build: {}",
            String::from_utf8_lossy(&r.stdout)
        );
    }
    let a = read_tree(&out_a);
    let b = read_tree(&out_b);
    assert_eq!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>());
    for (path, bytes) in &a {
        assert_eq!(bytes, &b[path], "flatland byte mismatch in {path}");
    }

    let props = String::from_utf8(a["server/server.properties"].clone()).unwrap();
    assert!(
        props.contains(
            "generator-settings={\"biome\":\"minecraft:plains\",\"layers\":[{\"block\":\"minecraft:bedrock\",\"height\":1},{\"block\":\"minecraft:dirt\",\"height\":126},{\"block\":\"minecraft:grass_block\",\"height\":1}]}"
        ),
        "flatland must ship the pinned grass superflat:\n{props}"
    );
    let place =
        String::from_utf8(a["datapack/data/hello-world/function/place_all.mcfunction"].clone())
            .unwrap();
    assert!(
        place.contains("place template hello-world:hello-room 0 63 0"),
        "flatland datum: walk_ref 64 − walk_y 1 = 63:\n{place}"
    );
}

/// The object form of an already-landed base emits byte-identically to its
/// string shorthand (`{base:"ocean"}` ≡ `"ocean"`), so the v0.12 surface adds
/// zero emission drift for existing horizons (criterion 1's fence half).
#[test]
fn ocean_object_form_emits_byte_identical_to_the_string() {
    let pf = common::prefabs_dir();
    let camp_s = campaign_with_horizon("ocean-str", "0.12.0", "\"ocean\"");
    let camp_o = campaign_with_horizon("ocean-obj", "0.12.0", "{ \"base\": \"ocean\" }");
    let out_s = tmp("ocean-str-out");
    let out_o = tmp("ocean-obj-out");
    for (camp, out) in [(&camp_s, &out_s), (&camp_o, &out_o)] {
        let r = build_into(camp, out, &pf);
        assert_eq!(code(&r), 0, "{}", String::from_utf8_lossy(&r.stdout));
    }
    let s = read_tree(&out_s);
    let o = read_tree(&out_o);
    assert_eq!(s.keys().collect::<Vec<_>>(), o.keys().collect::<Vec<_>>());
    for (path, bytes) in &s {
        // The stage documents themselves are build inputs recorded into the
        // manifest; the world.json literally differs (that IS the two forms).
        if path == "manifest.json" {
            continue;
        }
        assert_eq!(bytes, &o[path], "object-form drift in {path}");
    }
}
