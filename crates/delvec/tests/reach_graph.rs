//! The reach graph over the gallery's own hall (spec-0113 A7): the shipped
//! launch set has converged — a half-degree facing step and a half strafe add
//! no rest state to the closure the build takes.

mod common;

use delvec::compiler::nav::{Premises, ReachParams, World};

/// The gallery hall as the generator writes it, as a nav world.
fn gallery_hall(name: &str) -> World {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let out = std::process::Command::new(env!("CARGO"))
        .args(["run", "-q", "--locked", "--release", "--manifest-path"])
        .arg(common::repo_root().join("gallery/prefab-generator/Cargo.toml"))
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
    World::from_occupancy(
        delvec::compiler::assembled::occupancy_of(cells, &Default::default()),
        Premises::geometry_only(),
    )
}

#[test]
fn the_facing_step_has_converged_over_the_gallery_hall() {
    let world = gallery_hall("reach-graph-gallery");
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

/// spec-0113 §9: the gallery's fenced garden is a place only the graph sees —
/// the walk never enters the bed, the graph reaches it over the fence from
/// the boulder, and every cell of it is left again by the step.
#[test]
fn the_garden_bed_is_reached_by_the_graph_and_left() {
    let world = gallery_hall("reach-graph-garden");
    let root = world
        .snap([15, 1, 8], 3)
        .expect("the near hall has a floor");
    let bed = [[10, 3, 10]];
    let walk = world.walk_region(&[root]);
    let graph = world.body_reach(&[root]).cells();
    for c in bed {
        assert!(!walk.contains(&c), "the walk entered the bed at {c:?}");
        assert!(
            graph.contains(&c),
            "the graph did not reach the bed at {c:?}"
        );
        let back = world.body_reach(&[c]).cells();
        assert!(back.contains(&root), "{c:?} cannot leave the bed");
    }
    assert!(
        graph.iter().any(|c| world.is_perched(*c)),
        "no fence-top rest state in the hall's graph"
    );
}
