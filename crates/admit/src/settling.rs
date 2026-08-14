//! **The settling gates' second door** — the same two rules over a piece
//! nobody generated.
//!
//! `delve-grammar expand` judges an expansion before it freezes it; this is the
//! other end, where a hand-built or ingested `.nbt` arrives with no program
//! behind it. Both doors call the same implementation
//! ([`delvewright_grammar::settle`]) over the same kind of argument — a block
//! grid — because two checkers over one rule agree right up until they do not,
//! and the disagreement surfaces as a piece that admits clean and reds at
//! expansion, or the other way round.
//!
//! # Why a tile set is assembled first
//!
//! A zone past the structure-template cap ships as several `.nbt` files. Both
//! rules read a cell's NEIGHBOURS, so judging a tile on its own would report
//! every channel and every stair run that crosses a tile seam as broken — the
//! tiling is packaging, and packaging must not change a verdict. The zone is
//! reassembled into one grid and judged once.

use delvewright_grammar::block::BlockState;
use delvewright_grammar::geom::Box3;
use delvewright_grammar::model::VoxelModel;
use delvewright_grammar::settle;
use delvewright_schem::fluid;
use delvewright_schem::split::TilePart;
use delvewright_schem::stairs;

use crate::diag::Diagnostic;
use crate::structure::Structure;

/// Write one structure's blocks into `model` at `offset`.
fn blit(model: &mut VoxelModel, s: &Structure, offset: [i32; 3]) {
    for x in 0..s.size[0] {
        for y in 0..s.size[1] {
            for z in 0..s.size[2] {
                let Some(entry) = s.entry_at([x, y, z]) else {
                    continue;
                };
                let mut block = BlockState::simple(&entry.name);
                block.properties = entry.properties.clone();
                let at = [x + offset[0], y + offset[1], z + offset[2]];
                // A manifest that places a tile outside the zone it declares is
                // the manifest's own defect and belongs to the split reader; a
                // cell dropped here would only turn it into a wrong verdict.
                if model.get(at).is_some() {
                    let _ = model.set(at, &block);
                }
            }
        }
    }
}

/// The whole zone as one grid, from its tiles and their offsets.
pub fn zone_grid(zone_size: [i32; 3], tiles: &[(TilePart, Structure)]) -> VoxelModel {
    let size = [
        zone_size[0].max(0) as u32,
        zone_size[1].max(0) as u32,
        zone_size[2].max(0) as u32,
    ];
    let mut model = VoxelModel::new(Box3::at_origin(size));
    for (part, s) in tiles {
        blit(&mut model, s, part.offset);
    }
    model
}

/// What the two rules examined in one piece, and what they found.
///
/// The counts are carried rather than printed because they belong in the
/// REPORT: `delve-admit audit`'s report is the machine-readable artifact, and a
/// binding count that is only ever a log line is a binding count no reader can
/// act on. A diagnostic is raised only when something is wrong — a piece with
/// no stairs and no fluid is not two warnings, it is two zeroes in the report.
pub struct Settling {
    /// Stairs examined — the `stair-shape` binding count.
    pub stairs_examined: usize,
    /// Fluid cells examined — the `fluid-contained` binding count.
    pub fluid_cells_examined: usize,
    /// Cells written `waterlogged=true`: wet, still, and under no obligation.
    pub fluid_held_cells: usize,
    /// Run directions that leave the piece, where its own bytes decide nothing.
    pub fluid_at_edge: usize,
    /// One error per rule that was broken; empty when both hold.
    pub diagnostics: Vec<Diagnostic>,
}

/// **Judge a block grid on what the world will settle it into.**
pub fn judge(grid: &VoxelModel) -> Settling {
    let mut diagnostics = Vec::new();

    let shapes = settle::stair_shapes(grid);
    if !shapes.mismatches.is_empty() {
        let first = shapes.mismatches[0].cell;
        diagnostics.push(
            Diagnostic::error(
                stairs::DW_STAIR_SHAPE_DERIVED,
                settle::shape_detail(&shapes),
            )
            .at(first),
        );
    }

    let bodies = settle::fluid_bodies(grid);
    if !bodies.leaks.is_empty() {
        diagnostics.push(Diagnostic::error(
            fluid::DW_FLUID_ESCAPES,
            settle::fluid_detail(&bodies),
        ));
    }

    Settling {
        stairs_examined: shapes.bound,
        fluid_cells_examined: bodies.bound,
        fluid_held_cells: bodies.held,
        fluid_at_edge: bodies.at_edge.len(),
        diagnostics,
    }
}
