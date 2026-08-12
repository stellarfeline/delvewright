//! Reading an expanded model the way a player meets it: what a body can stand
//! in, what a walker can reach, and what an eye can see.
//!
//! The sightline walk is the compiler's: eye at 1.62 over the watch cell, target
//! at 1.0 over the observed cell, Amanatides–Woo cell traversal, both endpoint
//! cells exempt (`crates/compiler/src/nav.rs`, which `DW0388` uses). The
//! generator's idea of a sightline and the compiler's must not drift apart, so
//! the shape is copied rather than reinvented.
//!
//! `tests/staging.rs` carries its own copy of these helpers: it landed first,
//! and two more vocabulary families are in review against that file right now.
//! Folding it onto this module is a follow-up that costs nothing to do later and
//! would cost every one of those PRs a conflict to do now.

#![allow(dead_code)]

use std::collections::BTreeMap;

use delvewright_grammar::ir::Program;
use delvewright_grammar::{Anchor, Box3, ExpandOptions, Expansion, VoxelModel, expand};

/// Player eye height over the cell they stand in (vanilla 1.62), as `DW0388`.
const EYE_HEIGHT: f64 = 1.62;

/// Expand, or fail naming the program and the error.
pub fn expand_at(program: &Program, region: Box3, seed: u64) -> Expansion {
    expand(program, region, &ExpandOptions::seeded(seed))
        .unwrap_or_else(|e| panic!("{}: {e}", program.name))
}

pub use delvewright_grammar::nav::{
    connected, ends, passable, reachable_with_fall, solid, standable, standable_cells,
};

/// Whether an observer standing on `watch` can see the space a body standing on
/// `target` occupies. Both endpoint cells are exempt: the observer's own head,
/// and the volume being looked at.
pub fn sees(model: &VoxelModel, watch: [i32; 3], target: [i32; 3]) -> Result<(), [i32; 3]> {
    let eye = [
        watch[0] as f64 + 0.5,
        watch[1] as f64 + EYE_HEIGHT,
        watch[2] as f64 + 0.5,
    ];
    let mass = [
        target[0] as f64 + 0.5,
        target[1] as f64 + 1.0,
        target[2] as f64 + 0.5,
    ];
    let eye_cell = [watch[0], watch[1] + 1, watch[2]];
    match walk_cells(eye, mass, |c| {
        c != eye_cell && c != target && !passable(model, c)
    }) {
        Some(cell) => Err(cell),
        None => Ok(()),
    }
}

/// Walk every unit cell the segment `a → b` passes through, in order, returning
/// the first for which `hit` holds. Amanatides–Woo voxel traversal.
pub fn walk_cells(a: [f64; 3], b: [f64; 3], hit: impl Fn([i32; 3]) -> bool) -> Option<[i32; 3]> {
    let mut cell = [
        a[0].floor() as i32,
        a[1].floor() as i32,
        a[2].floor() as i32,
    ];
    let end = [
        b[0].floor() as i32,
        b[1].floor() as i32,
        b[2].floor() as i32,
    ];
    let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let mut step = [0i32; 3];
    let mut t_max = [f64::INFINITY; 3];
    let mut t_delta = [f64::INFINITY; 3];
    for i in 0..3 {
        if d[i] > 0.0 {
            step[i] = 1;
            t_max[i] = ((cell[i] + 1) as f64 - a[i]) / d[i];
            t_delta[i] = 1.0 / d[i];
        } else if d[i] < 0.0 {
            step[i] = -1;
            t_max[i] = (cell[i] as f64 - a[i]) / d[i];
            t_delta[i] = -1.0 / d[i];
        }
    }
    if hit(cell) {
        return Some(cell);
    }
    let budget: i64 = (0..3)
        .map(|i| (end[i] - cell[i]).unsigned_abs() as i64)
        .sum();
    for _ in 0..budget {
        let axis = if t_max[0] <= t_max[1] && t_max[0] <= t_max[2] {
            0
        } else if t_max[1] <= t_max[2] {
            1
        } else {
            2
        };
        cell[axis] += step[axis];
        t_max[axis] += t_delta[axis];
        if hit(cell) {
            return Some(cell);
        }
    }
    None
}

/// Anchors whose exported name starts with `anchor/<stem>-`, in index order.
pub fn indexed(anchors: &BTreeMap<String, Anchor>, stem: &str) -> Vec<[i32; 3]> {
    let prefix = format!("anchor/{stem}-");
    let mut found: Vec<(u32, [i32; 3])> = anchors
        .iter()
        .filter_map(|(name, a)| {
            let n: u32 = name.strip_prefix(&prefix)?.parse().ok()?;
            Some((n, a.pos))
        })
        .collect();
    found.sort_unstable();
    found.into_iter().map(|(_, pos)| pos).collect()
}
