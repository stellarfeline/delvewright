//! Reading an expanded model as a **place a body occupies**: what is passable,
//! what is a floor, which cells a player can stand in, and where that floor
//! meets the edge of the box.
//!
//! # One authority for "standable"
//!
//! This rule is not new here. It is the one the grammar's own gates have always
//! asserted with — `tests/support/mod.rs` and `tests/staging.rs` both walk a
//! model this way, and every "the lane is a chain segment", "the alcove is
//! reachable", "the perch is standable" claim in the piece library rests on it.
//! What it lacked was a home outside `#[cfg(test)]`, so a *shipping* consumer —
//! the sweep, and through it the owner's contact sheet — had no way to say what
//! the gates say. Moving the definition here rather than writing a second one is
//! the whole point: a picture that disagreed with the gate about which cells a
//! player can walk on would be worse than no picture.
//!
//! The compiler has its own standability rule
//! (`crates/compiler/src/nav.rs`), and it is deliberately **not** this one. It
//! reads a compiled campaign's `World` — a structure that knows about water,
//! lethal volumes, 1.5-tall barriers and multi-column footprints, none of which
//! exist as concepts in a bare [`VoxelModel`] of block states. The two agree
//! where they overlap (feet and head clear, solid floor directly below) and the
//! compiler's is strictly stricter. Sharing one function would mean lifting
//! `World` into this crate, which is the tail wagging the dog; what is shared
//! instead is the *definition*, restated here in the terms this layer has, and
//! the divergence is named so a future reader does not mistake it for drift.
//!
//! # Derived, never authored
//!
//! Everything in this module is **computed from the blocks**. That is what
//! makes it free: no rule has to declare it and no rule can get it wrong. It is
//! also what it cannot do — a floor cell says a body *can* stand there, never
//! that this is where the party comes in. Which opening is the entrance is an
//! authored fact, and it belongs to whatever declares it (an
//! [`crate::ir::Node::Mark`] today, a face contract later). Nothing here
//! guesses at one.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::geom::Box3;
use crate::model::VoxelModel;

/// Cells a body and a sightline pass through.
///
/// Everything the piece library places is a full block except air and a floor
/// skull, which is neither a barrier nor an occluder — and which sits on the
/// exact cell an anchor names, so a naive "not air means solid" predicate would
/// report the teaching niche unreachable. Outside the region counts as
/// blocking: a body that has left the model has left the thing being measured.
pub fn passable(model: &VoxelModel, pos: [i32; 3]) -> bool {
    match model.get(pos) {
        None => false,
        Some(block) => block.is_air() || block.name.ends_with("_skull"),
    }
}

/// A full block: what a floor is made of, and what stops an eye.
pub fn solid(model: &VoxelModel, pos: [i32; 3]) -> bool {
    model.get(pos).is_some() && !passable(model, pos)
}

/// A cell a player can stand in: two blocks of clearance over a full floor.
pub fn standable(model: &VoxelModel, pos: [i32; 3]) -> bool {
    let [x, y, z] = pos;
    passable(model, pos) && passable(model, [x, y + 1, z]) && solid(model, [x, y - 1, z])
}

/// Every standable cell of the model, in cell order.
pub fn standable_cells(model: &VoxelModel) -> BTreeSet<[i32; 3]> {
    model
        .region()
        .positions()
        .filter(|&p| standable(model, p))
        .collect()
}

/// One `(x, z)` column of the floor plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Column {
    /// Local `y` of the **lowest** standable cell in this column — the floor a
    /// body walking in at ground level meets.
    pub y: i32,
    /// How many standable levels this column carries in total. `> 1` means the
    /// column has a gallery, a rafter or an upper storey that a single-height
    /// plan cannot show, which is why the number travels with the plan instead
    /// of being lost in it.
    pub levels: u32,
}

/// The walkable floor of a model, as a plan.
///
/// Positions are **local to the region**, rebased off its origin, exactly as
/// [`crate::expand::Anchor`] is: a structure template is local-coordinate, so
/// moving the box moves nothing in here either.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FloorPlan {
    /// Region extent `[x, y, z]`, so a reader can index `columns` without the
    /// model.
    pub size: [u32; 3],
    /// One entry per `(x, z)` column, `x`-major (`x * size_z + z`), `None` where
    /// nothing in the column can be stood in.
    pub columns: Vec<Option<Column>>,
    /// Standable cells in total — the count every artifact drawn from this plan
    /// states, because a plan of zero cells and a plan nobody computed look the
    /// same on a page.
    pub standable_cells: usize,
    /// Columns carrying more than one standable level.
    pub multi_level_columns: usize,
}

impl FloorPlan {
    /// Read a model's floor.
    pub fn of(model: &VoxelModel) -> FloorPlan {
        let region = model.region();
        let size = region.size;
        let (sx, sy, sz) = (size[0] as i32, size[1] as i32, size[2] as i32);
        let mut columns = vec![None; (size[0] as usize) * (size[2] as usize)];
        let mut standable_cells = 0usize;
        let mut multi_level_columns = 0usize;
        for x in 0..sx {
            for z in 0..sz {
                let mut lowest: Option<i32> = None;
                let mut levels = 0u32;
                for y in 0..sy {
                    let cell = [
                        region.origin[0] + x,
                        region.origin[1] + y,
                        region.origin[2] + z,
                    ];
                    if standable(model, cell) {
                        levels += 1;
                        if lowest.is_none() {
                            lowest = Some(y);
                        }
                    }
                }
                standable_cells += levels as usize;
                if levels > 1 {
                    multi_level_columns += 1;
                }
                if let Some(y) = lowest {
                    columns[(x as usize) * (size[2] as usize) + z as usize] =
                        Some(Column { y, levels });
                }
            }
        }
        FloorPlan {
            size,
            columns,
            standable_cells,
            multi_level_columns,
        }
    }

    /// Columns with a floor in them.
    pub fn standable_columns(&self) -> usize {
        self.columns.iter().filter(|c| c.is_some()).count()
    }

    /// Distinct floor levels across the plan — one for a single-storey piece.
    pub fn levels(&self) -> usize {
        let mut ys: BTreeSet<i32> = BTreeSet::new();
        for c in self.columns.iter().flatten() {
            ys.insert(c.y);
        }
        ys.len()
    }
}

/// The four horizontal faces of a box, named the way a reader looks at a plan.
///
/// `y-min` / `y-max` are deliberately absent: a body does not walk out through
/// the floor or the ceiling, so a "standable cell on the top face" is not a way
/// out of the piece and listing it would pad the count that matters.
pub const FACES: [&str; 4] = ["x-min", "x-max", "z-min", "z-max"];

/// Where the floor reaches the edge of the box: per face, the standable cells
/// lying on it.
///
/// This is the derived half of the owner's "where do they come in, where do they
/// leave" — **every** place a body could cross the boundary, which is a fact
/// about the geometry. Which of them is the entrance is not a fact about the
/// geometry, and this type does not pretend otherwise; see the module note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Openings {
    /// Face name → the standable cells on it, in cell order. Local positions.
    pub by_face: BTreeMap<String, Vec<[i32; 3]>>,
}

impl Openings {
    /// Read a model's boundary openings.
    pub fn of(model: &VoxelModel) -> Openings {
        let region = model.region();
        let mut by_face: BTreeMap<String, Vec<[i32; 3]>> =
            FACES.iter().map(|f| (f.to_string(), Vec::new())).collect();
        for cell in standable_cells(model) {
            for (face, on) in faces_of(cell, region) {
                if on {
                    by_face
                        .get_mut(face)
                        .expect("every face is seeded above")
                        .push(local(cell, region));
                }
            }
        }
        Openings { by_face }
    }

    /// Total cells across every face.
    pub fn total(&self) -> usize {
        self.by_face.values().map(Vec::len).sum()
    }

    /// Faces that carry at least one opening.
    pub fn open_faces(&self) -> Vec<&str> {
        self.by_face
            .iter()
            .filter(|(_, cells)| !cells.is_empty())
            .map(|(f, _)| f.as_str())
            .collect()
    }
}

/// Rebase a world cell onto the region origin.
fn local(cell: [i32; 3], region: Box3) -> [i32; 3] {
    [
        cell[0] - region.origin[0],
        cell[1] - region.origin[1],
        cell[2] - region.origin[2],
    ]
}

/// Which of [`FACES`] a cell lies on. A corner cell lies on two, and is counted
/// on both — it really is a way out in both directions.
fn faces_of(cell: [i32; 3], region: Box3) -> [(&'static str, bool); 4] {
    let max = region.maximum();
    [
        ("x-min", cell[0] == region.origin[0]),
        ("x-max", cell[0] == max[0] - 1),
        ("z-min", cell[2] == region.origin[2]),
        ("z-max", cell[2] == max[2] - 1),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::BlockState;
    use crate::expand::{ExpandOptions, expand};
    use crate::ir::{Material, Node, Program};
    use crate::library::bell;

    /// A solid box: nothing is standable inside it, so every derived count is
    /// zero and says so rather than being absent.
    #[test]
    fn a_solid_body_has_no_floor_and_no_openings() {
        let program = Program::new("solid", "all").rule(
            "all",
            Node::Fill {
                material: Material::block(BlockState::simple("stone")),
            },
        );
        let out = expand(
            &program,
            Box3::at_origin([5, 5, 5]),
            &ExpandOptions::seeded(1),
        )
        .unwrap();
        let plan = FloorPlan::of(&out.model);
        assert_eq!(plan.standable_cells, 0);
        assert_eq!(plan.standable_columns(), 0);
        assert_eq!(plan.levels(), 0);
        assert_eq!(Openings::of(&out.model).total(), 0);
    }

    /// Every zone of the campaign registry: the floor binds, and it binds on
    /// every one of them. A plan that came out empty for a *building* is the
    /// unbound green CLAUDE.md names — the picture would be blank and nothing
    /// would say why.
    ///
    /// Binding: 8 zones, each with a non-empty floor and at least one boundary
    /// opening.
    #[test]
    fn every_zone_has_a_floor_that_reaches_its_boundary() {
        let opts = ExpandOptions::seeded(1);
        let mut checked = 0usize;
        for z in bell::ZONES {
            let program = (z.program)();
            let out = expand(&program, Box3::at_origin(z.region), &opts)
                .unwrap_or_else(|e| panic!("{}: {e}", z.id));
            let plan = FloorPlan::of(&out.model);
            assert!(
                plan.standable_cells > 0,
                "{}: no cell of this zone can be stood in",
                z.id
            );
            assert_eq!(
                plan.columns.len(),
                (z.region[0] * z.region[2]) as usize,
                "{}: the plan is not one entry per column",
                z.id
            );
            let openings = Openings::of(&out.model);
            assert!(
                openings.total() > 0,
                "{}: the floor never reaches the edge of the box, so no body can enter or \
                 leave it",
                z.id
            );
            checked += 1;
        }
        assert_eq!(checked, bell::ZONES.len(), "a zone went unmeasured");
    }

    /// The plan does not silently flatten a storey away: a zone with galleries
    /// reports the columns that carry more than one level, and at least one
    /// zone of the library does.
    #[test]
    fn a_column_with_two_floors_says_so() {
        let opts = ExpandOptions::seeded(1);
        let multi: Vec<&str> = bell::ZONES
            .iter()
            .filter(|z| {
                let program = (z.program)();
                let out = expand(&program, Box3::at_origin(z.region), &opts).unwrap();
                FloorPlan::of(&out.model).multi_level_columns > 0
            })
            .map(|z| z.id)
            .collect();
        assert!(
            !multi.is_empty(),
            "no zone carries a second standable level any more, so the multi-level count is \
             bound to nothing measured — re-derive it before trusting it"
        );
    }

    #[test]
    fn the_plan_is_deterministic_and_local_to_the_region() {
        let program = (bell::ZONES[0].program)();
        let size = bell::ZONES[0].region;
        let opts = ExpandOptions::seeded(3);
        let here = expand(&program, Box3::at_origin(size), &opts).unwrap();
        let there = expand(
            &program,
            Box3 {
                origin: [100, 40, -70],
                size,
            },
            &opts,
        )
        .unwrap();
        assert_eq!(FloorPlan::of(&here.model), FloorPlan::of(&there.model));
        assert_eq!(Openings::of(&here.model), Openings::of(&there.model));
    }
}
