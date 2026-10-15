//! The reach graph over the gallery's own hall (spec-0113 A7): the shipped
//! launch set has converged — a half-degree facing step and a half strafe add
//! no rest state to the closure the build takes.

mod common;

use delvec::compiler::nav::{Premises, ReachParams, World};

#[test]
fn the_facing_step_has_converged_over_the_gallery_hall() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("reach-graph-gallery");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let out = std::process::Command::new(env!("CARGO"))
        .args(["run", "-q", "--locked", "--release", "--manifest-path"])
        .arg(common::repo_root().join("prefabs/gallery-generator/Cargo.toml"))
        .arg("--")
        .arg(&dir)
        .output()
        .expect("the gallery generator runs");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let bytes = std::fs::read(dir.join("gallery-hall.nbt")).expect("the hall's bytes");
    let cells: std::collections::BTreeMap<[i32; 3], String> =
        delvec::compiler::assembled::structure_named_cells(&bytes)
            .into_iter()
            .collect();
    let world = World::from_occupancy(
        delvec::compiler::assembled::occupancy_of(cells, &Default::default()),
        Premises::geometry_only(),
    );
    // The near hall's floor, where the party arrives.
    let root = world
        .snap([15, 1, 8], 3)
        .expect("the near hall has a floor");
    let coarse = world.body_reach(&[root]).cells();
    let fine = world
        .body_reach_at(
            &[root],
            &ReachParams {
                facing_step: 0.5,
                strafes: vec![-1.0, -0.5, 0.0, 0.5, 1.0],
                ..ReachParams::shipped(false)
            },
        )
        .cells();
    assert!(
        coarse.len() > 300,
        "the closure binds the hall: {}",
        coarse.len()
    );
    let added: Vec<&[i32; 3]> = fine.difference(&coarse).collect();
    assert!(added.is_empty(), "the finer launch set adds {added:?}");
}
