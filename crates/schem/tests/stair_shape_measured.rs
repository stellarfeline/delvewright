//! **The stair derivation, replayed against the game's own answers.**
//!
//! `stairs::derive_shape` states what vanilla will do to a stair's `shape`, and
//! a gate that ships that claim is only as good as the claim. So the claim is
//! not read out of anyone's memory of vanilla's source: a field of 758 random
//! stairs — two stair blocks, both halves, all four facings, air holes — was
//! placed, settled and read back cell by cell on the pinned 1.21.11 server
//! (`tools/spike-block-settling/`), and this test replays every one of those
//! cells through the implementation.
//!
//! A disagreement here is the implementation being wrong about the game, which
//! is the only way this gate can produce a false verdict. Re-measure with
//! `EULA=TRUE tools/spike-block-settling/run.sh` after any change to the pin.

use std::collections::BTreeMap;

use delvewright_schem::stairs::{Facing, Half, Shape, Stair, derive_shape};
use serde::Deserialize;

#[derive(Deserialize)]
struct Observations {
    minecraft_version: String,
    stair_field: Field,
}

#[derive(Deserialize)]
struct Field {
    cells: Vec<Cell>,
}

#[derive(Deserialize)]
struct Cell {
    x: i32,
    z: i32,
    block: String,
    facing: Option<String>,
    half: Option<String>,
    observed_shape: Option<String>,
}

fn observations() -> Observations {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tools/spike-block-settling/observations.json"
    );
    let text =
        std::fs::read_to_string(path).expect("the measured field is committed beside the spike");
    serde_json::from_str(&text).expect("observations.json parses")
}

#[test]
fn every_measured_cell_agrees_with_the_derivation() {
    let obs = observations();
    assert_eq!(
        obs.minecraft_version,
        delvewright_schem::blocks::MC_VERSION,
        "the field was measured on a different Minecraft than the pin"
    );

    let mut field: BTreeMap<(i32, i32), Stair> = BTreeMap::new();
    for cell in &obs.stair_field.cells {
        if cell.block == "minecraft:air" {
            continue;
        }
        field.insert(
            (cell.x, cell.z),
            Stair {
                facing: Facing::parse(cell.facing.as_deref().unwrap()).unwrap(),
                half: Half::parse(cell.half.as_deref().unwrap()).unwrap(),
            },
        );
    }

    let mut bound = 0usize;
    let mut seen: BTreeMap<Shape, usize> = BTreeMap::new();
    let mut wrong: Vec<String> = Vec::new();
    for cell in &obs.stair_field.cells {
        let Some(observed) = cell.observed_shape.as_deref().and_then(Shape::parse) else {
            continue;
        };
        let here = field[&(cell.x, cell.z)];
        let derived = derive_shape(here, |dir| {
            let step = dir.step();
            field.get(&(cell.x + step[0], cell.z + step[2])).copied()
        });
        bound += 1;
        *seen.entry(observed).or_default() += 1;
        if derived != observed {
            wrong.push(format!(
                "{},{}: {} facing={} half={} — the game derived {observed}, this crate derives \
                 {derived}",
                cell.x, cell.z, cell.block, here.facing, here.half
            ));
        }
    }

    // The fixture's own vacuity guards. A field that produced one shape, or no
    // stairs, would agree with almost any implementation.
    assert!(
        bound >= 500,
        "the replay bound {bound} measured stair(s) — the fixture is not a field"
    );
    assert_eq!(
        seen.len(),
        5,
        "the measured field shows only {:?} — a field missing a shape cannot falsify a \
         derivation of it",
        seen.keys().collect::<Vec<_>>()
    );
    assert!(
        wrong.is_empty(),
        "{} of {bound} measured cell(s) disagree with the game:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
}

/// **Which blocks are stairs, derived twice and compared.**
///
/// `BlockRegistry::is_stairs` reads the pinned block registry: the block whose
/// `shape` property takes vanilla's five stair values. The pinned block
/// classification derives the same set from a different artifact — Mojang's own
/// `#stairs` block tag (`tools/extract-block-classification.py`). Deriving it
/// rather than listing it is what keeps a version that adds a stair from going
/// silently unjudged; deriving it TWICE is what keeps the derivation honest, as
/// a predicate that quietly matched 57 of 58 blocks would be invisible.
#[test]
fn the_stair_set_is_the_same_set_by_two_derivations() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../compiler/data/block-classification-1.21.11.json"
    );
    let text = std::fs::read_to_string(path).expect("the pinned block classification");
    let doc: serde_json::Value = serde_json::from_str(&text).expect("classification parses");
    let registry = delvewright_schem::blocks::BlockRegistry::v1_21_11();

    let mut by_tag: Vec<String> = Vec::new();
    let mut by_property: Vec<String> = Vec::new();
    for (name, entry) in doc["blocks"].as_object().expect("blocks object") {
        if entry["form"].as_str() == Some("stair") {
            by_tag.push(name.clone());
        }
        if registry.is_stairs(name) {
            by_property.push(name.clone());
        }
    }
    by_tag.sort();
    by_property.sort();
    assert!(
        by_tag.len() >= 50,
        "the tag derivation found {} stair(s) — one of the two artifacts is not what it was",
        by_tag.len()
    );
    assert_eq!(
        by_property, by_tag,
        "the two derivations name different blocks"
    );
}

/// The measured field is what this test binds to, so the corner cases have to
/// be IN it: a replay over a field of straight runs would agree with an
/// implementation that always answered `straight`.
#[test]
fn the_measured_field_carries_every_corner() {
    let obs = observations();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for cell in &obs.stair_field.cells {
        if let Some(shape) = &cell.observed_shape {
            *counts.entry(shape.clone()).or_default() += 1;
        }
    }
    for shape in [
        "straight",
        "inner_left",
        "inner_right",
        "outer_left",
        "outer_right",
    ] {
        assert!(
            counts.get(shape).copied().unwrap_or(0) >= 20,
            "the field holds {} cell(s) of shape {shape}",
            counts.get(shape).copied().unwrap_or(0)
        );
    }
}
