//! **No campaign that reaches `build` has a walk cell at or below the sea**
//! (spec-0060 §10.5).
//!
//! The contract has two outcomes on an ocean and no third: a campaign builds,
//! with every placed piece's walk plane standing at `SEA_LEVEL + 1` and the sea
//! nowhere in the walk region, or it is refused at `analyze` with `DW0886`
//! naming the pool. What must not exist is the outcome that used to be the
//! normal one — a build that floods, green through every gate, played standing
//! in the water.
//!
//! So this asserts the invariant over every ocean campaign it can build, from
//! the ledgers the build itself writes:
//!
//! - `sea walk-plane binding` reports **0** walk cells at or below the sea
//!   plane, over a non-zero judged count (`DW0344`, second arm);
//! - `validation/sea-seepage.json` reports **0 submerged and 0 wading**, over a
//!   non-zero `walk_cells_examined` (`DW0851`);
//! - and the two agree, which is what makes them two observers rather than one
//!   restated.
//!
//! A zero over an empty population is the unbound vacuity mode, so every
//! denominator is asserted non-zero before its numerator is read. And the
//! invariant is perturbed toward the shape it forbids: the same world with one
//! course of its plinth carved out reds, with a perturbation nothing else in
//! this file could catch.

use std::path::{Path, PathBuf};
use std::process::{Command as Proc, Output};

mod common;

/// The sea plane this whole contract is stated against.
const SEA_LEVEL: i32 = 62;
/// One block above it: where an ocean world stands its walk plane.
const WALK_REF_Y: i32 = SEA_LEVEL + 1;

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("no-flood-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn delvec(args: &[&str]) -> Output {
    Proc::new(env!("CARGO_BIN_EXE_delvec"))
        .args(args)
        .output()
        .expect("delvec runs")
}

fn log(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

/// An ocean campaign: hello-world with `horizon: ocean`.
fn ocean_campaign(tag: &str) -> PathBuf {
    let camp = tmp(&format!("camp-{tag}"));
    common::copy_dir_all(&common::hello_world_dir(), &camp);
    let mut world: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(camp.join("world.json")).unwrap()).unwrap();
    world["dsl_version"] = serde_json::json!("0.22.0");
    let content = world["content"].as_object_mut().unwrap();
    content.insert("horizon".into(), serde_json::json!("ocean"));
    content.insert("boundary".into(), serde_json::json!({ "margin": 20 }));
    std::fs::write(
        camp.join("world.json"),
        serde_json::to_string_pretty(&world).unwrap(),
    )
    .unwrap();
    camp
}

/// Build, and hand back the exit code, everything said, and the output tree.
fn build(tag: &str, camp: &Path, prefabs: &Path) -> (i32, String, PathBuf) {
    let out = tmp(&format!("out-{tag}"));
    let r = delvec(&[
        "build",
        camp.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        prefabs.to_str().unwrap(),
    ]);
    (r.status.code().unwrap_or(-1), log(&r), out)
}

/// Every `place template` line's y — where each piece was actually seated.
fn placed_ys(out: &Path) -> Vec<i32> {
    let f = out.join("datapack/data/hello-world/function/place_all.mcfunction");
    let text = std::fs::read_to_string(&f).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
    text.lines()
        .filter_map(|l| l.split_once("place template ")?.1.split_whitespace().nth(2))
        .filter_map(|y| y.parse().ok())
        .collect()
}

/// A shore room with no water in it at all: three courses of solid plinth, a
/// floor at local y=3, walls and a lit roof.
///
/// Waterless on purpose. A piece that authors water and states no waterline is
/// `DW0886`'s unstated shore — correctly — so a fixture about anything ELSE on
/// an ocean has to be a piece with no shore to state.
fn waterless_shore(dir: &Path) {
    let size = [11, 9, 11];
    let mut cells: Vec<([i32; 3], &str)> = Vec::new();
    for x in 0..size[0] {
        for z in 0..size[2] {
            for y in 0..3 {
                cells.push(([x, y, z], "minecraft:stone"));
            }
            let lamp = matches!((x, z), (3, 3) | (3, 7) | (7, 3) | (7, 7));
            for y in 3..size[1] {
                if x == 0 || x == size[0] - 1 || z == 0 || z == size[2] - 1 || y == size[1] - 1 {
                    let roof = y == size[1] - 1;
                    cells.push((
                        [x, y, z],
                        if roof && lamp {
                            "minecraft:glowstone"
                        } else {
                            "minecraft:stone"
                        },
                    ));
                }
            }
        }
    }
    std::fs::write(
        dir.join("hello-room.nbt"),
        common::structure_nbt(size, &cells),
    )
    .unwrap();
    let path = dir.join("hello-room.json");
    let mut doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    {
        let obj = doc.as_object_mut().unwrap();
        obj.remove("waterline_y");
        obj["structure"]["size"] = serde_json::json!(size);
    }
    std::fs::write(&path, serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();
}

/// **Every ocean campaign this tree can build stands its walk plane above the
/// sea, and the two proofs that would see otherwise both report zero.**
#[test]
fn no_ocean_build_puts_a_walk_cell_at_or_below_the_sea() {
    // The population: every ocean campaign this test can build. Both shapes of
    // shore piece the fixture library offers, so a pass here is a pass over
    // pieces with a tide pool and pieces without one.
    let mut examined = 0usize;
    for (tag, waterline) in [("declared", true), ("silent", false)] {
        let dir = common::ocean_prefabs_dir(&format!("no-flood-{tag}"), common::OceanRoom::Shore);
        if !waterline {
            // A shore whose tide pool is gone: no water, so no waterline to
            // declare, and the piece is seated by its walk plane alone. That is
            // the case the retired global datum could not express at all.
            waterless_shore(&dir);
        }
        let camp = ocean_campaign(tag);
        let (code, text, out) = build(tag, &camp, &dir);
        assert_eq!(code, 0, "the `{tag}` ocean campaign builds:\n{text}");
        examined += 1;

        // The walk plane stands where the horizon says: the piece declares
        // `walk_y: 3` and is seated at `63 - 3`.
        let ys = placed_ys(&out);
        assert!(!ys.is_empty(), "`{tag}` placed nothing to judge");
        for y in &ys {
            assert_eq!(
                y + 3,
                WALK_REF_Y,
                "`{tag}` seated a piece at y={y}, so its walk plane is not at y={WALK_REF_Y}"
            );
        }

        // Observer one: the walk-plane arm of `DW0344`, over the placed pieces.
        let line = text
            .lines()
            .find(|l| l.starts_with("sea walk-plane binding:"))
            .unwrap_or_else(|| panic!("`{tag}` printed no sea walk-plane binding:\n{text}"));
        assert!(line.contains("horizon base `ocean`"), "`{tag}`: {line}");
        assert!(
            line.contains("0 stand at or below it"),
            "`{tag}` stands a body under the sea: {line}"
        );
        let judged: usize = line
            .split(" of ")
            .next()
            .and_then(|s| s.rsplit(' ').next())
            .and_then(|n| n.parse().ok())
            .unwrap_or(0);
        assert!(
            judged > 0,
            "`{tag}` judged ZERO walk cells — a zero over an empty population is the unbound \
             vacuity mode, not a pass: {line}"
        );

        // Observer two: the sea-seepage proof, which floods the world instead of
        // measuring it. Two readings of one fact, sharing no arithmetic.
        let ledger: serde_json::Value = serde_json::from_slice(
            &std::fs::read(out.join("validation/sea-seepage.json"))
                .expect("every ocean build writes the sea-seepage ledger"),
        )
        .expect("the ledger is JSON");
        assert_eq!(ledger["horizon_base"], "ocean", "`{tag}`: {ledger}");
        assert_eq!(ledger["walk_cells_submerged"], 0, "`{tag}`: {ledger}");
        assert_eq!(ledger["walk_cells_wading"], 0, "`{tag}`: {ledger}");
        assert!(
            ledger["walk_cells_examined"].as_u64().unwrap() > 0,
            "`{tag}` examined zero walk cells: {ledger}"
        );
    }
    assert_eq!(
        examined, 2,
        "the invariant was asserted over {examined} ocean build(s); a universally quantified \
         claim over an empty set is vacuous, not a pass"
    );
}

/// **The perturbation only this invariant could catch.**
///
/// The same world, with one course of the shore piece's plinth carved out by the
/// campaign's own edit script after it is placed — so every declaration in the
/// library is still honest and the defect exists only in the assembled world.
/// A body then stands at y=62, the sea's own plane, and the build must refuse.
///
/// This is what makes the assertion above evidence rather than a shape: it is a
/// world that differs from the green one by one region write, and nothing else
/// in this file would see it.
#[test]
fn a_carve_that_lowers_a_floor_to_the_sea_plane_reds() {
    let dir = common::ocean_prefabs_dir("no-flood-carved", common::OceanRoom::Shore);
    let camp = ocean_campaign("carved");
    std::fs::write(
        camp.join("world-edits.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "dsl_version": "0.22.0",
            "campaign_id": "hello-world",
            "stage": "world-edits",
            "content": {
                "batches": [{
                    "id": "batch/pit",
                    "area": "area/keep",
                    "edits": [
                        { "verb": "select", "name": "region/pit", "shape": {
                            "kind": "box",
                            "frame": {
                                "kind": "piece-local",
                                "piece": 0,
                                "prefab": "prefab/hello-room"
                            },
                            "min": [4, 2, 4], "max": [6, 2, 6]
                        }},
                        { "verb": "carve", "region": "region/pit" }
                    ]
                }]
            }
        }))
        .unwrap(),
    )
    .unwrap();
    let (code, text, _) = build("carved", &camp, &dir);
    assert_ne!(
        code, 0,
        "a walk cell carved down to the sea plane must not build:\n{text}"
    );
    assert!(
        text.contains("DW0344") || text.contains("DW0851"),
        "and it is one of the two proofs that own the sea in the walk region:\n{text}"
    );
}

/// The other outcome, and the assertion that there is no third: a campaign whose
/// pieces cannot be seated on an ocean is refused at `analyze`, having placed
/// nothing.
#[test]
fn a_pool_that_cannot_be_seated_is_refused_at_analyze_having_placed_nothing() {
    let dir = common::ocean_prefabs_dir("no-flood-unseatable", common::OceanRoom::Shore);
    let path = dir.join("hello-room.json");
    let mut doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    doc.as_object_mut().unwrap().remove("walk_y");
    std::fs::write(&path, serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();

    let camp = ocean_campaign("unseatable");
    let r = delvec(&[
        "analyze",
        camp.to_str().unwrap(),
        "--prefabs",
        dir.to_str().unwrap(),
    ]);
    let text = log(&r);
    assert_eq!(r.status.code(), Some(1), "refused at analyze:\n{text}");
    assert!(text.contains("DW0886"), "{text}");
    assert!(
        !text.contains("place template"),
        "having placed nothing:\n{text}"
    );
}

/// **`walk_y` reaches the surface it changes** (spec-0060 §10.9): perturb one
/// piece's declared walk plane and an emitted byte moves.
///
/// The byte is the `place template` line's y, which is the whole point of the
/// declaration: an ocean area's origin IS `walk_ref_y - walk_y`, so a piece that
/// says its floor is one course lower is seated one block higher, and the
/// datapack says so. A declaration that moved no emitted byte would be a field
/// the engine reads and the world does not.
///
/// The waterline is removed first, deliberately: with one declared, moving the
/// walk plane moves the waterline off the sea and `DW0344`'s first arm refuses
/// before anything is emitted — which is that rule working, and would leave this
/// one nothing to measure.
#[test]
fn perturbing_a_pieces_walk_plane_moves_an_emitted_byte() {
    let seat_at = |tag: &str, walk_y: i64| -> Vec<i32> {
        let dir = common::ocean_prefabs_dir(&format!("walk-moves-{tag}"), common::OceanRoom::Shore);
        waterless_shore(&dir);
        let path = dir.join("hello-room.json");
        let mut doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        doc.as_object_mut()
            .unwrap()
            .insert("walk_y".into(), serde_json::json!(walk_y));
        std::fs::write(&path, serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();
        let camp = ocean_campaign(tag);
        let (code, text, out) = build(tag, &camp, &dir);
        assert_eq!(code, 0, "`{tag}` builds:\n{text}");
        placed_ys(&out)
    };

    let three = seat_at("walk3", 3);
    let two = seat_at("walk2", 2);
    assert_eq!(three, vec![WALK_REF_Y - 3], "declared 3, seated at 60");
    assert_eq!(two, vec![WALK_REF_Y - 2], "declared 2, seated at 61");
    assert_ne!(
        three, two,
        "the declaration moved and the emitted placement did not — a field the engine reads \
         and the world does not"
    );
}
