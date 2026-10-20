//! spec-0110 criterion 4: one sight rule. `delvec snapshot` on every view of
//! the gallery's render plan writes `occluded` and `out_of_frame` equal,
//! target for target, to `framing::sight` called with the manifest's camera
//! over the occupancy world the snapshot reads — and `sight` has one
//! definition in the crate.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use delvec::compiler::framing::{CellBox, Pose, sight};
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

/// The gallery copied to `dir` with its generated pieces written by
/// `gallery/prefab-generator` into `dir.prefabs`, as `gallery-prefabs.py`
/// writes them for the primary (the overlays' detailed places are not read
/// by the primary).
fn gallery_with_pieces(dir: &Path) -> PathBuf {
    let _ = std::fs::remove_dir_all(dir);
    common::copy_dir_all(&common::repo_root().join("gallery"), dir);
    let pieces = dir.with_extension("prefabs");
    let _ = std::fs::remove_dir_all(&pieces);
    std::fs::create_dir_all(&pieces).unwrap();
    let out = Command::new(env!("CARGO"))
        .args(["run", "-q", "--locked", "--release", "--manifest-path"])
        .arg(common::repo_root().join("gallery/prefab-generator/Cargo.toml"))
        .arg("--")
        .arg(&pieces)
        .arg("--skins")
        .arg(dir.join("skins"))
        .arg("--design")
        .arg(dir.join("design"))
        .arg("--textures")
        .arg(dir.join("textures"))
        .arg("--terrain")
        .arg(dir.join("terrain"))
        .output()
        .expect("the gallery generator runs");
    assert!(
        out.status.success(),
        "the gallery generator exited {:?}: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    pieces
}

#[test]
fn the_snapshot_manifest_is_the_sight_rules_answer_on_every_render_plan_view() {
    let tmp = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("framing-agree");
    let campaign_dir = tmp.join("gallery");
    let pieces = gallery_with_pieces(&campaign_dir);

    // The world the snapshot reads, built the way it builds it.
    let loaded = delvec::compiler::load::load_campaign_dir(&campaign_dir).unwrap();
    let campaign = delvewright_dsl::parse_campaign(&loaded.raw).expect("parses");
    let prefabs = PrefabRegistry::load_dir(&pieces).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plans");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    if let Some(s) = &plan.surround {
        structures.extend(s.structures.clone());
    }
    let mut files: Vec<String> = plan
        .areas
        .iter()
        .flat_map(|a| a.pieces.iter().flat_map(|p| &p.templates))
        .map(|t| t.structure_file.clone())
        .collect();
    files.extend(delvec::compiler::edit::fragment_structure_files(
        &campaign, &prefabs,
    ));
    for f in files {
        if !structures.contains_key(&f) {
            structures.insert(f.clone(), std::fs::read(pieces.join(&f)).unwrap());
        }
    }
    let assembled = match delvec::compiler::edit::replay_view(&plan, &prefabs, &structures) {
        Ok(Some(er)) => er.assembled,
        Ok(None) => delvec::compiler::assembled::assemble(&plan, &structures),
        Err(e) => panic!("{}: {}", e.code, e.message),
    };
    let world = delvec::compiler::nav::World::from_occupancy(
        delvec::compiler::assembled::occupancy_over(&assembled.blocks, &assembled.open_gates),
        delvec::compiler::nav::Premises::geometry_only(),
    );
    let targets = delvec::compiler::snapshot::collect_targets(&plan);

    let views: serde_json::Value = serde_json::from_slice(
        &std::fs::read(common::repo_root().join("gallery/render-plan.json")).unwrap(),
    )
    .unwrap();
    let views = views["views"].as_array().unwrap();
    assert!(!views.is_empty());
    let (mut compared, mut in_frame, mut occluded) = (0usize, 0usize, 0usize);
    for (i, v) in views.iter().enumerate() {
        let id = v["id"].as_str().unwrap();
        let png = tmp.join(format!("view-{i}.png"));
        let r = Command::new(BIN)
            .args(["snapshot", campaign_dir.to_str().unwrap(), "--shot", id])
            .arg("--prefabs")
            .arg(&pieces)
            .arg("--out")
            .arg(&png)
            .output()
            .unwrap();
        assert!(
            r.status.success(),
            "{id}: {}",
            String::from_utf8_lossy(&r.stderr)
        );
        let m: serde_json::Value = serde_json::from_slice(
            &std::fs::read(tmp.join(format!("view-{i}.manifest.json"))).unwrap(),
        )
        .unwrap();
        let cam = &m["camera"];
        let pos: Vec<f64> = cam["pos"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_f64().unwrap())
            .collect();
        let pose = Pose {
            pos: [pos[0], pos[1], pos[2]],
            yaw: cam["yaw"].as_f64().unwrap(),
            pitch: cam["pitch"].as_f64().unwrap(),
        };
        let fov = cam["fov"].as_f64().unwrap();
        let aspect = m["image"]["width"].as_f64().unwrap() / m["image"]["height"].as_f64().unwrap();
        let key = |t: &serde_json::Value| {
            format!(
                "{}|{}|{}",
                t["kind"].as_str().unwrap(),
                t["area"].as_str().unwrap_or(""),
                t["id"].as_str().unwrap()
            )
        };
        let listed: BTreeMap<String, Option<bool>> = m["targets"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| (key(t), t["occluded"].as_bool()))
            .chain(
                m["out_of_frame"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|t| (key(t), None)),
            )
            .collect();
        let blocks = |c: [i32; 3]| world.blocks_camera(c);
        for t in &targets {
            let k = format!("{}|{}|{}", t.kind, t.area, t.id);
            let s = sight(
                &pose,
                fov,
                aspect,
                t.centre(),
                &CellBox {
                    min: t.min,
                    max: t.max,
                },
                &blocks,
            );
            let got = listed
                .get(&k)
                .unwrap_or_else(|| panic!("{id}: {k} absent from the manifest"));
            match got {
                Some(occ) => {
                    assert!(
                        s.in_frame,
                        "{id}: {k} is in the manifest's frame and not sight's"
                    );
                    assert_eq!(*occ, s.occluded(), "{id}: {k} occlusion disagrees");
                    in_frame += 1;
                    occluded += usize::from(*occ);
                }
                None => {
                    // Out of the manifest's frame: either outside the
                    // action-safe rectangle, or a box with no pixels on screen.
                    if s.in_frame {
                        let scr = delvec::compiler::snapshot::screen_box(
                            &delvec::compiler::snapshot::Camera {
                                pos: pose.pos,
                                yaw: pose.yaw,
                                pitch: pose.pitch,
                                fov,
                            },
                            m["image"]["width"].as_u64().unwrap() as u32,
                            m["image"]["height"].as_u64().unwrap() as u32,
                            t,
                        );
                        assert!(
                            scr.is_none(),
                            "{id}: {k} is in sight's frame and not the manifest's"
                        );
                    }
                }
            }
            compared += 1;
        }
    }
    eprintln!(
        "framing/snapshot agreement: {} view(s), {compared} target judgement(s) compared, \
         {in_frame} in frame, {occluded} occluded",
        views.len()
    );
    assert!(
        in_frame > 0 && compared > in_frame,
        "a zero binding is a finding"
    );
}

#[test]
fn sight_has_one_definition() {
    let src = common::repo_root().join("crates/delvec/src");
    let mut hits = Vec::new();
    let mut stack = vec![src];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                for (i, l) in std::fs::read_to_string(&p).unwrap().lines().enumerate() {
                    if l.contains("fn sight(") {
                        hits.push(format!("{}:{}", p.display(), i + 1));
                    }
                }
            }
        }
    }
    assert_eq!(hits.len(), 1, "{hits:?}");
}
