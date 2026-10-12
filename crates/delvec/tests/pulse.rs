//! A sound beats over a place (spec-0102): the surface, the emission, the
//! derived range, determinism, the refusals and the PackTest templates, over a
//! one-room keep the test writes itself.
//!
//! The room is a 15 × 8 × 9 stone box whose floor is the course `y = 3`, so
//! every interior cell of `y = 4` is standable and nothing else is. The
//! expected standable cells of a box are derived from that geometry here —
//! not read back from the engine's walk model — so the range assertion
//! compares the engine's measurement with a second reading that shares none
//! of its configuration.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildFailure, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, DSL_VERSION, parse_campaign};

const ROOM: &str = "pulse-room";
const SIZE: [i32; 3] = [15, 8, 9];
/// The piece-local walk course.
const WALK_Y: i32 = 4;

fn cells() -> Vec<([i32; 3], &'static str)> {
    let [sx, sy, sz] = SIZE;
    let mut by: BTreeMap<[i32; 3], &'static str> = BTreeMap::new();
    for x in 0..sx {
        for z in 0..sz {
            for y in 0..sy {
                let shell = x == 0 || x == sx - 1 || z == 0 || z == sz - 1;
                if y < WALK_Y || y == sy - 1 || shell {
                    by.insert([x, y, z], "minecraft:stone");
                }
            }
        }
    }
    for x in [3, 7, 11] {
        for z in [2, 6] {
            by.insert([x, sy - 1, z], "minecraft:glowstone");
        }
    }
    by.into_iter().collect()
}

fn anchors() -> serde_json::Value {
    serde_json::json!({
        "spawn": { "pos": [2, 4, 4], "facing": "east", "role": "entry" },
        "anchor/keeper-stand": { "pos": [12, 4, 6], "facing": "west" },
        "anchor/exit-west": { "pos": [2, 4, 2] },
        "anchor/exit": { "pos": [2, 4, 2] },
        "anchor/seat": { "pos": [2, 4, 6] },
        "anchor/hearth": { "pos": [7, 4, 4] },
        "anchor/wall": { "pos": [0, 5, 4] }
    })
}

fn scratch(tag: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("pulse")
        .join(format!("{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Write the keep with `pulses` (a JSON array body) and `talk` (the effects
/// the first beat runs). Returns `(campaign dir, prefab dir)`.
fn write(tag: &str, pulses: &str, talk: &str) -> (PathBuf, PathBuf) {
    let root = scratch(tag);
    let lib = root.join("prefabs");
    common::write_single_prefab(&lib, ROOM, SIZE, &cells(), anchors());
    let dir = common::campaign_bound_to(&root.join("campaign"), ROOM);
    common::patch_file(&dir.join("world.json"), |v| {
        v["content"]["areas"][0]["mitigation"] = serde_json::json!("night-vision");
    });
    let pulses = if pulses.is_empty() {
        String::new()
    } else {
        format!(r#", "pulses": [ {pulses} ]"#)
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
    ]{pulses}
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

/// Every diagnostic `delvec validate` raises: the DSL's own, and the
/// compiler's validation-tier checks over sounds and places.
fn validate(dir: &Path, lib: &Path) -> Vec<delvewright_dsl::Diagnostic> {
    let c = campaign(dir);
    let prefabs = PrefabRegistry::load_dir(lib).unwrap();
    let mut d = delvewright_dsl::validate_campaign_with(
        &c,
        &delvec::compiler::registry::FullItemRegistry::v1_21_11(),
        &prefabs,
        &delvec::compiler::registry::FullEntityRegistry::v1_21_11(),
    );
    d.extend(delvec::compiler::atmos::check_sounds(&c));
    d.extend(delvec::compiler::atmosphere::check(&c));
    d
}

fn try_build(dir: &Path, lib: &Path) -> Result<(BuildOutput, Vec<String>), BuildFailure> {
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
    .map(|(out, w)| {
        (
            out,
            w.iter()
                .map(|w| format!("{}: {}", w.code, w.message))
                .collect(),
        )
    })
}

fn build(tag: &str, pulses: &str, talk: &str) -> (BuildOutput, Vec<String>) {
    let (dir, lib) = write(tag, pulses, talk);
    match try_build(&dir, &lib) {
        Ok(x) => x,
        Err(BuildFailure::Diagnostic { code, message }) => {
            panic!("expected the build to succeed, got {code}: {message}")
        }
        Err(e) => panic!("expected the build to succeed, got {e:?}"),
    }
}

fn text(out: &BuildOutput, path: &str) -> Option<String> {
    out.get(path).map(|b| String::from_utf8(b.clone()).unwrap())
}

fn function(out: &BuildOutput, name: &str) -> Option<String> {
    let key = out
        .keys()
        .find(|k| {
            k.starts_with("datapack/data/") && k.ends_with(&format!("/function/{name}.mcfunction"))
        })?
        .clone();
    text(out, &key)
}

fn body_lines(s: &str) -> Vec<String> {
    s.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect()
}

const SET_FLAG: &str = r#"{ "type": "set-flag", "flag": "flag/door-shut" }"#;

const GATED: &str = r#"{ "id": "pulse/the-heart", "sound": "entity.warden.heartbeat",
    "at": { "anchor": "anchor/hearth", "offset": [0, -1, 0] },
    "region": { "anchor": "anchor/hearth", "extent": [5, 1, 3] },
    "every": 30, "floor": 0.4,
    "when": { "requires_flags": ["flag/door-shut"], "forbids_flags": ["flag/heart-found"] } }"#;

const UNGATED: &str = r#"{ "id": "pulse/the-drip", "sound": "block.pointed_dripstone.drip_water",
    "at": { "anchor": "anchor/hearth" },
    "region": { "anchor": "anchor/hearth", "extent": [2, 1, 2] },
    "every": 40, "floor": 0.2, "pitch": 1.5 }"#;

/// Criterion 1: `delvec schema --stage quests` exports `pulses[]` with
/// exactly the nine fields, and one `PlaceRef` names the `region` / `place`
/// pair for both carriers.
#[test]
fn the_surface_is_nine_fields_and_one_place_ref() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_delvec"))
        .args(["schema", "--stage", "quests"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let schema: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let defs = schema
        .get("$defs")
        .or_else(|| schema.get("definitions"))
        .expect("the schema has definitions");
    let pulse = &defs["Pulse"]["properties"];
    let mut keys: Vec<&str> = pulse
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "at", "every", "floor", "id", "pitch", "place", "region", "sound", "when"
        ]
    );
    // One declaration of the pair under crates/dsl/src: the `PlaceRef` struct.
    let src = common::repo_root().join("crates/dsl/src");
    let mut decls = 0usize;
    for f in walk(&src) {
        let s = std::fs::read_to_string(&f).unwrap();
        decls += s.matches("pub region: Option<StealthZone>,").count();
        assert!(
            !s.contains("        region: Option<StealthZone>,"),
            "a second, unshared `region` / `place` pair in {f:?}"
        );
    }
    assert_eq!(decls, 1, "the `region` / `place` pair is declared once");
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
    out.sort();
    out
}

/// Criterion 2, gated: `pulse_<s>` carries exactly spec-0102 §4.2's lines in
/// order, `setup_finish` seeds the holder, `tick` carries the one edge line.
#[test]
fn a_gated_pulse_is_the_latch_the_beat_the_hop_and_one_clearing_line_per_term() {
    let (out, _) = build("gated", GATED, SET_FLAG);
    let ns = "hello-world";
    let body = body_lines(&function(&out, "pulse_the_heart").expect("the pulse function"));
    let terms =
        "if score #party dw.f_door_shut matches 1 unless score #party dw.f_heart_found matches 1";
    assert_eq!(body.len(), 5, "{body:#?}");
    assert_eq!(body[0], "scoreboard players set #pulse_the_heart dw.sys 1");
    assert!(
        body[1].starts_with(&format!(
            "execute {terms} run playsound minecraft:entity.warden.heartbeat master @a["
        )) && body[1].contains(",tag=!dw_cutscene] "),
        "{}",
        body[1]
    );
    assert_eq!(
        body[2],
        format!("execute {terms} run schedule function {ns}:pulse_the_heart 30t replace")
    );
    assert_eq!(
        body[3],
        "execute unless score #party dw.f_door_shut matches 1 run scoreboard players set #pulse_the_heart dw.sys 0"
    );
    assert_eq!(
        body[4],
        "execute if score #party dw.f_heart_found matches 1 run scoreboard players set #pulse_the_heart dw.sys 0"
    );
    let setup = function(&out, "setup_finish").unwrap();
    assert!(
        setup
            .lines()
            .any(|l| l == "scoreboard players set #pulse_the_heart dw.sys 0")
    );
    let tick = function(&out, "tick").unwrap();
    let edge: Vec<&str> = tick
        .lines()
        .filter(|l| l.contains("pulse_the_heart"))
        .collect();
    assert_eq!(
        edge,
        [format!(
            "execute {terms} unless score #pulse_the_heart dw.sys matches 1 run function {ns}:pulse_the_heart"
        )
        .as_str()]
    );
    assert!(text(&out, "validation/pulses.json").is_some());
}

/// Criterion 2, ungated: two lines, started by `setup_finish`, no holder, no
/// tick line.
#[test]
fn an_ungated_pulse_is_two_lines_started_by_setup() {
    let (out, _) = build("ungated", UNGATED, SET_FLAG);
    let body = body_lines(&function(&out, "pulse_the_drip").unwrap());
    assert_eq!(body.len(), 2, "{body:#?}");
    assert!(
        body[0].starts_with("playsound minecraft:block.pointed_dripstone.drip_water master @a[")
    );
    assert!(
        body[0].ends_with(" 1.5"),
        "the pitch is written: {}",
        body[0]
    );
    assert_eq!(
        body[1],
        "schedule function hello-world:pulse_the_drip 40t replace"
    );
    let setup = function(&out, "setup_finish").unwrap();
    assert!(
        setup
            .lines()
            .any(|l| l == "function hello-world:pulse_the_drip")
    );
    assert!(!setup.contains("#pulse_the_drip"));
    assert!(!function(&out, "tick").unwrap().contains("pulse_"));
}

/// Criterion 2, none: no function, no holder, no tick line, no ledger.
#[test]
fn a_campaign_with_no_pulse_emits_none_of_it() {
    let (out, _) = build("none", "", SET_FLAG);
    assert!(out.keys().all(|k| !k.contains("pulse_")), "a pulse file");
    assert!(text(&out, "validation/pulses.json").is_none());
    for f in ["tick", "setup_finish"] {
        assert!(!function(&out, f).unwrap().contains("#pulse_"), "{f}");
    }
}

/// The playsound's `<x> <y> <z> <V>` and the box, parsed off the emitted line.
fn sound_line(line: &str) -> ([f64; 3], f64, ([i32; 3], [i32; 3])) {
    let after = line.split("tag=!dw_cutscene] ").nth(1).unwrap();
    let nums: Vec<f64> = after.split(' ').map(|n| n.parse().unwrap()).collect();
    let sel = line.split("@a[").nth(1).unwrap().split(']').next().unwrap();
    let mut kv: BTreeMap<&str, i32> = BTreeMap::new();
    for part in sel.split(',') {
        if let Some((k, v)) = part.split_once('=')
            && let Ok(n) = v.parse()
        {
            kv.insert(k, n);
        }
    }
    let lo = [kv["x"], kv["y"], kv["z"]];
    let hi = [lo[0] + kv["dx"], lo[1] + kv["dy"], lo[2] + kv["dz"]];
    ([nums[0], nums[1], nums[2]], nums[3], (lo, hi))
}

/// The farthest standing eye of `bx` from `src`, from the room's geometry:
/// the box's middle course is the walk course (every box here is centred on
/// a walk-level anchor), and every interior cell of it is standable.
fn expected_far(src: [f64; 3], bx: ([i32; 3], [i32; 3]), interior: ([i32; 3], [i32; 3])) -> f64 {
    let (lo, hi) = bx;
    let y = (lo[1] + hi[1]) / 2;
    let mut far = 0.0f64;
    for x in lo[0].max(interior.0[0])..=hi[0].min(interior.1[0]) {
        for z in lo[2].max(interior.0[2])..=hi[2].min(interior.1[2]) {
            let eye = [f64::from(x) + 0.5, f64::from(y) + 1.62, f64::from(z) + 0.5];
            let d = (0..3)
                .map(|i| (eye[i] - src[i]).powi(2))
                .sum::<f64>()
                .sqrt();
            far = far.max(d);
        }
    }
    far
}

/// Criterion 3: `V = max(far / (16 · (1 − floor)), 1)` on the emitted line,
/// for three floors, with `far` taken from the room's geometry; and a source
/// outside the box is measured from where it stands.
#[test]
fn the_volume_is_derived_from_the_farthest_standing_eye() {
    for (tag, floor, at) in [
        ("range-a", "0.0", "anchor/hearth"),
        ("range-b", "0.4", "anchor/hearth"),
        ("range-c", "0.9", "anchor/keeper-stand"),
    ] {
        let pulse = format!(
            r#"{{ "id": "pulse/the-heart", "sound": "entity.warden.heartbeat",
                "at": {{ "anchor": "{at}" }},
                "region": {{ "anchor": "anchor/hearth", "extent": [2, 1, 1] }},
                "every": 30, "floor": {floor} }}"#
        );
        let (out, _) = build(tag, &pulse, SET_FLAG);
        let line = body_lines(&function(&out, "pulse_the_heart").unwrap())[0].clone();
        let (src, v, bx) = sound_line(&line);
        // The room's interior, in world coordinates: the box's own walk
        // course and the shell one cell outside the anchor's room.
        let origin = [bx.0[0] - 5, bx.0[1] - (WALK_Y - 1), bx.0[2] - 3];
        let interior = (
            [origin[0] + 1, 0, origin[2] + 1],
            [origin[0] + SIZE[0] - 2, 0, origin[2] + SIZE[2] - 2],
        );
        let far = expected_far(src, bx, interior);
        let floor: f64 = floor.parse().unwrap();
        let want = (far / (16.0 * (1.0 - floor))).max(1.0);
        assert!(
            (v - want).abs() < 1e-9,
            "{tag}: V {v} on the line, {want} from far {far} (floor {floor}): {line}"
        );
        if at == "anchor/keeper-stand" {
            let inside =
                (0..3).all(|i| f64::from(bx.0[i]) <= src[i] && src[i] <= f64::from(bx.1[i]) + 1.0);
            assert!(!inside, "the source stands outside the box: {line}");
            assert!(v > 1.0, "a source outside the box reaches further: {line}");
        }
    }
}

/// Criterion 4: `0.4` and `0.40` are one number, and the emitted bytes do not
/// move.
#[test]
fn a_floors_spelling_moves_no_byte() {
    let a = build("spell-a", GATED, SET_FLAG).0;
    let b = build(
        "spell-b",
        &GATED.replace("\"floor\": 0.4", "\"floor\": 0.40"),
        SET_FLAG,
    )
    .0;
    assert!(GATED.contains("\"floor\": 0.4,"));
    let pick = |o: &BuildOutput| -> Vec<(String, Vec<u8>)> {
        o.iter()
            // The structure file is this test's own prefab, written by each
            // case into its own library (the gzip stream carries its write), so
            // it is an input that differs, not an emission; every other byte of
            // the datapack is compared.
            .filter(|(k, _)| {
                (k.starts_with("datapack/") && !k.contains("/structure/"))
                    || k.starts_with("validation/pulses")
            })
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    };
    let (pa, pb) = (pick(&a), pick(&b));
    let moved: Vec<&String> = pa
        .iter()
        .zip(&pb)
        .filter(|(x, y)| x != y)
        .map(|(x, _)| &x.0)
        .collect();
    assert_eq!(pa.len(), pb.len());
    assert!(moved.is_empty(), "bytes moved in {moved:?}");
}

fn codes(d: &[delvewright_dsl::Diagnostic]) -> Vec<String> {
    d.iter().map(|d| d.code.clone()).collect()
}

/// Criterion 5, §6.1: each shape of `DW0993`, refused at the document with
/// its remedy named.
#[test]
fn a_pulse_declared_against_itself_is_dw0993() {
    let cases: [(&str, String, &str); 6] = [
        ("every-0", GATED.replace("\"every\": 30", "\"every\": 0"), "at least 1"),
        ("floor-1", GATED.replace("\"floor\": 0.4", "\"floor\": 1.0"), "inside `[0, 1)`"),
        ("floor-neg", GATED.replace("\"floor\": 0.4", "\"floor\": -0.1"), "inside `[0, 1)`"),
        ("pitch-3", UNGATED.replace("\"pitch\": 1.5", "\"pitch\": 3.0"), "inside `[0, 2]`"),
        (
            "when-empty",
            GATED.replace(
                r#""when": { "requires_flags": ["flag/door-shut"], "forbids_flags": ["flag/heart-found"] }"#,
                r#""when": {}"#,
            ),
            "leaving `when` out",
        ),
        ("duplicate", format!("{GATED}, {GATED}"), "distinct id"),
    ];
    for (tag, pulses, remedy) in cases {
        let (dir, lib) = write(tag, &pulses, SET_FLAG);
        let d = validate(&dir, &lib);
        let hit = d.iter().find(|d| d.code == "DW0993");
        let hit = hit.unwrap_or_else(|| panic!("{tag}: no DW0993 in {:?}", codes(&d)));
        assert!(hit.message.contains(remedy), "{tag}: {}", hit.message);
    }
}

/// Criterion 5, §6.1: a `player`-scoped term in `when` is `DW0993`, and the
/// same term on a `party` datum validates.
#[test]
fn a_beat_one_player_hears_is_dw0993() {
    for (tag, scope, refused) in [("player", "player", true), ("party", "party", false)] {
        let (dir, lib) = write(
            &format!("scope-{tag}"),
            &GATED.replace(
                r#""when": { "requires_flags": ["flag/door-shut"], "forbids_flags": ["flag/heart-found"] }"#,
                r#""when": { "requires_state": [{ "state": "state/heat", "op": "at-least", "value": 1 }] }"#,
            ),
            r#"{ "type": "set-state", "state": "state/heat", "value": 1 }"#,
        );
        common::patch_file(&dir.join("quests.json"), |v| {
            v["content"]["state"] = serde_json::json!([{
                "id": "state/heat", "initial": 0, "scope": scope, "note": "how hot the heart runs"
            }]);
        });
        let d = validate(&dir, &lib);
        let hit = d.iter().find(|d| d.code == "DW0993");
        assert_eq!(hit.is_some(), refused, "{tag}: {:?}", codes(&d));
        if let Some(h) = hit {
            assert!(h.message.contains("`party`-scoped datum"), "{}", h.message);
        }
    }
}

/// Criterion 5: neither or both of `region` / `place` is `DW0929`, the rule a
/// repaint already answers to.
#[test]
fn a_pulse_heard_both_here_and_there_is_dw0929() {
    let both = UNGATED.replace("\"every\": 40", "\"place\": \"area/keep\", \"every\": 40");
    let neither = UNGATED.replace(
        r#""region": { "anchor": "anchor/hearth", "extent": [2, 1, 2] },"#,
        "",
    );
    for (tag, pulses, words) in [
        ("both", both, "names both"),
        ("neither", neither, "names neither"),
    ] {
        let (dir, lib) = write(tag, &pulses, SET_FLAG);
        let d = validate(&dir, &lib);
        let hit = d
            .iter()
            .find(|d| d.code == "DW0929")
            .unwrap_or_else(|| panic!("{tag}: {:?}", codes(&d)));
        assert!(hit.message.contains(words) && hit.message.contains("Keep exactly one"));
    }
}

/// Criterion 5: an unregistered sound is `DW0326`, through the one rule.
#[test]
fn a_beat_the_game_never_heard_is_dw0326() {
    let (dir, lib) = write(
        "sound",
        &UNGATED.replace(
            "block.pointed_dripstone.drip_water",
            "block.pointed_dripstone.drip_waters",
        ),
        SET_FLAG,
    );
    let d = validate(&dir, &lib);
    assert!(
        d.iter()
            .any(|d| d.code == "DW0326" && d.path == "/content/pulses/0/sound"),
        "{:?}",
        codes(&d)
    );
}

/// Criterion 5, §6.2: a box with no standable cell is `DW0994`, naming the
/// box and the nearest standable cell; drawn over the floor, it builds.
#[test]
fn a_beat_in_solid_rock_is_dw0994_and_the_box_is_the_remedy() {
    let rock = UNGATED.replace(
        r#""region": { "anchor": "anchor/hearth", "extent": [2, 1, 2] }"#,
        r#""region": { "anchor": "anchor/wall", "extent": [0, 0, 0] }"#,
    );
    let (dir, lib) = write("rock", &rock, SET_FLAG);
    match try_build(&dir, &lib) {
        Err(BuildFailure::Diagnostic { code, message }) => {
            assert_eq!(code.to_string(), "DW0994", "{message}");
            assert!(message.contains("nearest standable cell"), "{message}");
            assert!(message.contains("Draw the box over floor"), "{message}");
        }
        Err(e) => panic!("{e:?}"),
        Ok(_) => panic!("a box in the wall builds"),
    }
    build("rock-moved", UNGATED, SET_FLAG);
}

/// Criterion 5, §6.3: a pulse the forced route never hears live in its box is
/// the `DW0995` advisory, naming the term that never held; and its ledger row
/// says `not_heard: no station`.
#[test]
fn a_pulse_the_route_never_hears_is_the_dw0995_advisory() {
    let never = GATED.replace("flag/door-shut", "flag/never-set");
    let (out, warnings) = build("never", &never, SET_FLAG);
    let w = warnings
        .iter()
        .find(|w| w.starts_with("DW0995"))
        .unwrap_or_else(|| panic!("{warnings:#?}"));
    assert!(w.contains("`flag/never-set` is required"), "{w}");
    let ledger: serde_json::Value =
        serde_json::from_str(&text(&out, "validation/pulses.json").unwrap()).unwrap();
    assert_eq!(ledger["pulses"][0]["not_heard"], "no station");
    // The gate the forced route opens: heard, with a station, no advisory.
    let (out, warnings) = build("heard", GATED, SET_FLAG);
    assert!(
        !warnings.iter().any(|w| w.starts_with("DW0995")),
        "{warnings:#?}"
    );
    let ledger: serde_json::Value =
        serde_json::from_str(&text(&out, "validation/pulses.json").unwrap()).unwrap();
    assert!(
        ledger["pulses"][0]["stations"]["listening"]["cell"].is_array(),
        "{ledger:#}"
    );
}

/// **A fill a body walks through moves neither the place nor its stations**
/// (spec-0102 × the collision-table classification): the first beat, which
/// opens the heart's gate, also lays a sculk vein over every walk-course cell
/// of the heart's box. The vein is a Pass, so the body still stands on the
/// floor beneath it and the ledger — far, range, volume, the listening and the
/// silent stations — is byte-identical to the keep without it. The identical
/// fill of stone is a Fill: the body steps up onto it, so the listening
/// station rides one block higher, while `far` — measured over the place as
/// assembled at load (spec-0102 §4.1) — does not move.
#[test]
fn a_fill_a_body_walks_through_moves_neither_the_place_nor_its_stations() {
    let fill = |block: &str| {
        format!(
            r#"{SET_FLAG}, {{ "type": "fill-region",
              "region": {{ "anchor": "anchor/hearth", "extent": [5, 0, 3] }},
              "block": "{block}" }}"#
        )
    };
    let ledger = |out: &BuildOutput| -> serde_json::Value {
        serde_json::from_str(&text(out, "validation/pulses.json").unwrap()).unwrap()
    };
    let (bare, _) = build("pass-bare", GATED, SET_FLAG);
    let (vein, _) = build("pass-vein", GATED, &fill("minecraft:sculk_vein[down=true]"));
    assert_eq!(
        text(&vein, "validation/pulses.json"),
        text(&bare, "validation/pulses.json")
    );
    let bare = ledger(&bare);
    let listening = |l: &serde_json::Value| l["pulses"][0]["stations"]["listening"]["cell"].clone();
    assert!(listening(&bare).is_array(), "{bare:#}");
    let (stone, _) = build("pass-stone", GATED, &fill("minecraft:stone"));
    let stone = ledger(&stone);
    assert_eq!(stone["pulses"][0]["far"], bare["pulses"][0]["far"]);
    let (b, s) = (listening(&bare), listening(&stone));
    assert_eq!(
        (s[0].clone(), s[1].as_i64().unwrap() - 1, s[2].clone()),
        (b[0].clone(), b[1].as_i64().unwrap(), b[2].clone()),
        "the stone raises the station one block: {stone:#}"
    );
}

/// Criterion 6: the ledger carries every row field of §5.1.
#[test]
fn the_ledger_carries_every_row_field() {
    let (out, _) = build("ledger", &format!("{GATED}, {UNGATED}"), SET_FLAG);
    let ledger: serde_json::Value =
        serde_json::from_str(&text(&out, "validation/pulses.json").unwrap()).unwrap();
    assert_eq!(ledger["declared"], 2);
    for row in ledger["pulses"].as_array().unwrap() {
        for key in [
            "id",
            "sound",
            "source",
            "box",
            "every",
            "floor",
            "pitch",
            "far",
            "range",
            "volume",
            "gate_terms",
            "stations",
        ] {
            assert!(row.get(key).is_some(), "{key} missing from {row:#}");
        }
    }
    assert_eq!(
        ledger["pulses"][0]["gate_terms"].as_array().unwrap().len(),
        2
    );
    assert_eq!(
        ledger["pulses"][1]["gate_terms"].as_array().unwrap().len(),
        0
    );
}

/// Criterion 7: two templates per gated pulse and one per ungated, each
/// ending in `schedule clear`.
#[test]
fn the_templates_are_two_per_gated_pulse_and_one_per_ungated() {
    let (out, _) = build("packtest", &format!("{GATED}, {UNGATED}"), SET_FLAG);
    let mut names: Vec<&str> = out
        .keys()
        .filter(|k| k.starts_with("packtest-datapack/") && k.contains("/test/pulse_"))
        .map(|k| k.rsplit('/').next().unwrap())
        .collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "pulse_the_drip.mcfunction",
            "pulse_the_heart.mcfunction",
            "pulse_the_heart_shut.mcfunction"
        ]
    );
    for k in out.keys().filter(|k| k.contains("/test/pulse_")) {
        let t = text(&out, k).unwrap();
        let clear = t
            .lines()
            .position(|l| l.contains("run schedule clear hello-world:pulse_"));
        assert!(clear.is_some(), "{k} clears its schedule:\n{t}");
        let after: Vec<&str> = t.lines().skip(clear.unwrap() + 1).collect();
        assert!(
            after
                .iter()
                .all(|l| !l.contains("function hello-world:pulse_")),
            "{k}: nothing re-arms the chain after the clear:\n{t}"
        );
    }
}
