//! spec-0091: a campaign declares the view distance its far views need —
//! `world.view_distance`, the radius it serves, the far views judged against
//! it (`DW0956`), and the cost the build states to the host.
//!
//! The showcase-camera shape is proven in `compiler::view::camera`'s own tests
//! (`a_showcase_camera_whose_subject_is_past_the_served_radius_is_refused`);
//! the cutscene shape is proven here through a real build.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildFailure, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvec::compiler::served;
use delvewright_dsl::viewdistance::{self, CEILING, FLOOR};
use delvewright_dsl::{Campaign, parse_campaign, validate_campaign_with};

fn campaign(dir: &Path) -> Campaign {
    let loaded = load_campaign_dir(dir).unwrap();
    parse_campaign(&loaded.raw).expect("fixture parses")
}

fn codes(c: &Campaign) -> Vec<String> {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    validate_campaign_with(
        c,
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    )
    .into_iter()
    .map(|d| d.code.to_string())
    .collect()
}

fn messages(c: &Campaign, code: &str) -> Vec<String> {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    validate_campaign_with(
        c,
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    )
    .into_iter()
    .filter(|d| d.code == code)
    .map(|d| d.message)
    .collect()
}

fn try_build(dir: &Path, c: &Campaign) -> Result<BuildOutput, BuildFailure> {
    let loaded = load_campaign_dir(dir).unwrap();
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(c, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
}

fn text(out: &BuildOutput, path: &str) -> String {
    String::from_utf8(
        out.get(path)
            .unwrap_or_else(|| panic!("{path} emitted"))
            .clone(),
    )
    .unwrap()
}

fn property(out: &BuildOutput, file: &str, key: &str) -> String {
    text(out, file)
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{key}=")).map(str::to_string))
        .unwrap_or_else(|| panic!("`{key}` is not written to {file}"))
}

// ---------------------------------------------------------------------------
// The served radius, as one number
// ---------------------------------------------------------------------------

#[test]
fn the_served_radius_is_sixteen_blocks_per_chunk_and_the_remedy_is_the_fewest_chunks() {
    assert_eq!(viewdistance::served_radius_blocks(FLOOR), 160.0);
    assert_eq!(viewdistance::served_radius_blocks(CEILING), 512.0);
    assert_eq!(viewdistance::chunks_for(159.0), FLOOR);
    assert_eq!(viewdistance::chunks_for(161.0), 11);
    assert_eq!(viewdistance::chunks_for(450.0), 29);
    assert_eq!(viewdistance::chunks_for(9000.0), CEILING);
}

// ---------------------------------------------------------------------------
// Shape A — the declared number's range (validation tier)
// ---------------------------------------------------------------------------

#[test]
fn a_declared_view_distance_outside_the_served_range_is_refused() {
    let dir = common::hello_world_dir();
    for n in [0u8, 9, 33, 40] {
        let mut c = campaign(&dir);
        c.world.content.view_distance = Some(n);
        let m = messages(&c, "DW0956");
        assert_eq!(m.len(), 1, "{n}: {m:?}");
        assert!(
            m[0].contains(&format!("= {n} chunks")) && m[0].contains("10..=32"),
            "{}",
            m[0]
        );
    }
    for n in [FLOOR, 16, CEILING] {
        let mut c = campaign(&dir);
        c.world.content.view_distance = Some(n);
        assert!(!codes(&c).contains(&"DW0956".to_string()), "{n}");
    }
}

// ---------------------------------------------------------------------------
// Shapes B and C — a site-plan sightline or view past the radius (validation tier)
// ---------------------------------------------------------------------------

fn blockout_dir() -> PathBuf {
    common::compiler_fixtures_dir().join("blockout")
}

#[test]
fn a_view_aimed_past_the_served_radius_is_refused_and_the_named_declaration_serves_it() {
    let mut c = campaign(&blockout_dir());
    assert!(
        !codes(&c).contains(&"DW0956".to_string()),
        "the fixture's views are near"
    );
    let plan = c.site_plan.as_mut().unwrap();
    let view = &mut plan.content.views[0];
    view.look_at = [view.eye[0], view.eye[1], view.eye[2] + 400];
    let m = messages(&c, "DW0956");
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(
        m[0].contains("the view `view/the-approach`")
            && m[0].contains("reaches 400.0 blocks")
            && m[0].contains("the engine's floor of 10 chunks")
            && m[0].contains("serves 160 blocks")
            && m[0].contains("Declare `world.view_distance: 25`"),
        "{}",
        m[0]
    );
    // The remedy the message names reaches a different verdict; one chunk less does not.
    c.world.content.view_distance = Some(24);
    let m = messages(&c, "DW0956");
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(
        m[0].contains("the declared `world.view_distance` of 24 chunks"),
        "{}",
        m[0]
    );
    c.world.content.view_distance = Some(25);
    assert!(messages(&c, "DW0956").is_empty());
}

#[test]
fn a_sightline_longer_than_the_served_radius_is_refused() {
    let mut c = campaign(&blockout_dir());
    let plan = c.site_plan.as_mut().unwrap();
    let line = &mut plan.content.sightlines[0];
    line.to = [line.from[0] + 300, line.from[1], line.from[2]];
    let m = messages(&c, "DW0956");
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(
        m[0].contains("the vista `edge/loft-sightline`")
            && m[0].contains("world.view_distance: 19"),
        "{}",
        m[0]
    );
    c.world.content.view_distance = Some(19);
    assert!(messages(&c, "DW0956").is_empty());
}

#[test]
fn a_view_past_what_any_distance_serves_names_the_ceiling() {
    let mut c = campaign(&blockout_dir());
    c.world.content.view_distance = Some(CEILING);
    let plan = c.site_plan.as_mut().unwrap();
    let view = &mut plan.content.views[0];
    view.look_at = [view.eye[0] + 600, view.eye[1], view.eye[2]];
    let m = messages(&c, "DW0956");
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(
        m[0].contains("past what the pinned server can serve") && !m[0].contains("Declare"),
        "{}",
        m[0]
    );
}

#[test]
fn the_document_binding_line_states_the_distance_and_what_was_judged() {
    let mut c = campaign(&blockout_dir());
    let b = viewdistance::checks(&c, &mut Vec::new());
    assert_eq!(
        (b.chunks, b.declared, b.sightlines, b.views, b.beyond),
        (FLOOR, false, 1, 1, 0)
    );
    assert!(
        b.line()
            .contains("10 chunk(s) (the engine's floor, undeclared) serve 160 blocks")
    );
    assert!(
        b.line()
            .contains("1 sightline(s) and 1 view(s) judged against it, 0 beyond it")
    );
    c.world.content.view_distance = Some(20);
    let b = viewdistance::checks(&c, &mut Vec::new());
    assert!(
        b.line().contains("20 chunk(s) (declared) serve 320 blocks"),
        "{}",
        b.line()
    );
    // An out-of-range number is judged at the floor, so the lines are still judged.
    c.world.content.view_distance = Some(99);
    let b = viewdistance::checks(&c, &mut Vec::new());
    assert_eq!((b.chunks, b.declared, b.sightlines), (FLOOR, true, 1));
}

// ---------------------------------------------------------------------------
// Shape E — a cutscene shot past the radius (build tier)
// ---------------------------------------------------------------------------

#[test]
fn a_cutscene_shot_aimed_past_the_served_radius_is_refused_at_build() {
    // Unmoved, the fixture builds at the floor.
    let dir = common::cutscene_shots_dir();
    try_build(&dir, &campaign(&dir)).expect("the fixture's own shots are near");
    // The same campaign with its first shot's `look_at` pushed 300 blocks south.
    let far_dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("view-distance-far-shot");
    let _ = std::fs::remove_dir_all(&far_dir);
    common::copy_dir_all(&dir, &far_dir);
    common::patch_file(&far_dir.join("quests.json"), |doc| {
        let moved = push_look_at_away(doc, 300);
        assert_eq!(
            moved, 1,
            "the fixture's first cutscene shot carries a look_at"
        );
    });
    let far = campaign(&far_dir);
    assert!(codes(&far).is_empty(), "{:?}", codes(&far));
    let Err(err) = try_build(&far_dir, &far) else {
        panic!("a 300-block aim at the floor is refused");
    };
    let BuildFailure::Diagnostic { code, message } = &err else {
        panic!("{err:?}");
    };
    assert_eq!(code.id(), "DW0956");
    assert!(
        message.contains("cutscene: shot 0")
            && message.contains("served view distance reaches 160 blocks")
            && message.contains("Declare `world.view_distance: 19`"),
        "{message}"
    );
    // The remedy the message names builds, and is what the server is told.
    let mut served = far.clone();
    served.world.content.view_distance = Some(19);
    let out = try_build(&far_dir, &served).expect("served, the shot builds");
    assert_eq!(
        property(&out, "server/server.properties", "view-distance"),
        "19"
    );
}

/// Move the first cutscene shot's `look_at` `dz` blocks along z, in the raw
/// quests document; returns how many shots were moved.
fn push_look_at_away(doc: &mut serde_json::Value, dz: i64) -> usize {
    let mut moved = 0;
    fn walk(v: &mut serde_json::Value, dz: i64, moved: &mut usize) {
        match v {
            serde_json::Value::Object(map) => {
                if map.get("type").and_then(|t| t.as_str()) == Some("cutscene")
                    && *moved == 0
                    && let Some(offset) = map
                        .get_mut("shots")
                        .and_then(|s| s.get_mut(0))
                        .and_then(|s| s.get_mut("look_at"))
                        .and_then(|l| l.get_mut("offset"))
                        .and_then(|o| o.get_mut(2))
                {
                    *offset = serde_json::json!(offset.as_i64().unwrap() + dz);
                    *moved += 1;
                    return;
                }
                for (_, child) in map.iter_mut() {
                    walk(child, dz, moved);
                }
            }
            serde_json::Value::Array(items) => {
                for child in items.iter_mut() {
                    walk(child, dz, moved);
                }
            }
            _ => {}
        }
    }
    walk(doc, dz, &mut moved);
    moved
}

// ---------------------------------------------------------------------------
// Emission — the properties file, the cost statement, the README
// ---------------------------------------------------------------------------

#[test]
fn the_declared_distance_is_served_and_its_cost_is_stated() {
    let dir = common::hello_world_dir();
    let out = try_build(&dir, &campaign(&dir)).unwrap();
    assert_eq!(
        property(&out, "server/server.properties", "view-distance"),
        FLOOR.to_string()
    );
    assert_eq!(
        property(&out, "server/server.properties", "simulation-distance"),
        emit::DELVE_SIMULATION_DISTANCE.to_string()
    );
    assert_eq!(
        property(&out, "server/resources.properties", "heap-max"),
        served::heap_max_label(FLOOR)
    );
    assert_eq!(
        property(&out, "server/resources.properties", "players"),
        "4"
    );
    let readme = text(&out, "server/README.md");
    assert!(
        readme.contains("the engine's floor, nothing declared"),
        "{readme}"
    );
    assert!(
        readme.contains(&format!("heap-max={}", served::heap_max_label(FLOOR))),
        "{readme}"
    );

    let mut c = campaign(&dir);
    c.world.content.view_distance = Some(CEILING);
    let out = try_build(&dir, &c).unwrap();
    assert_eq!(
        property(&out, "server/server.properties", "view-distance"),
        "32"
    );
    // The simulation distance never moves with the view distance.
    assert_eq!(
        property(&out, "server/server.properties", "simulation-distance"),
        emit::DELVE_SIMULATION_DISTANCE.to_string()
    );
    assert_eq!(
        property(&out, "server/resources.properties", "heap-max"),
        served::heap_max_label(CEILING)
    );
    assert!(served::heap_max_gib(CEILING) > served::heap_max_gib(FLOOR));
    let readme = text(&out, "server/README.md");
    assert!(
        readme.contains("declared in `world.view_distance`")
            && readme.contains("at least 32 chunks"),
        "{readme}"
    );
}

#[test]
fn the_cost_model_is_the_rigs() {
    // 10 chunks: the server sends 473 chunks to one client (the rig's reading).
    assert_eq!(served::sent_chunks(10), 473);
    assert_eq!(served::sent_chunks(16), 1057);
    assert_eq!(served::sent_chunks(24), 2181);
    assert_eq!(served::sent_chunks(32), 3725);
    let b = served::Binding {
        chunks: 24,
        declared: true,
        showcase_cameras: 2,
        cutscene_shots: 3,
    };
    let line = b.line();
    assert!(
        line.contains("24 chunk(s) (declared) serve 384 blocks"),
        "{line}"
    );
    assert!(
        line.contains("2 showcase camera(s) and 3 cutscene shot(s) judged"),
        "{line}"
    );
    assert!(
        line.contains(&format!("heap-max {}", served::heap_max_label(24))),
        "{line}"
    );
    assert!(line.contains("4 players × 2181 chunks each"), "{line}");
}
