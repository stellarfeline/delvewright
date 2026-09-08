//! **The settling gates' second door** — the same two rules over a piece
//! nobody generated.
//!
//! `delvec grammar expand` judges an expansion before it freezes it; this is the
//! other end, where a hand-built or ingested `.nbt` arrives with no program
//! behind it. Both doors call the same implementation
//! ([`delvec::grammar::settle`]) over the same kind of argument — a block
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

use crate::grammar::block::BlockState;
use crate::grammar::geom::Box3;
use crate::grammar::model::VoxelModel;
use crate::grammar::settle;
use crate::schem::fluid;
use crate::schem::split::TilePart;
use crate::schem::stairs;

use crate::admit::diag::Diagnostic;
use crate::admit::structure::Structure;

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

/// **A whole piece as one grid, read from its library files** — whichever of
/// the two packagings its blocks arrived in.
///
/// The same reassembly [`zone_grid`] performs, reached from a piece's own
/// metadata rather than from a `.json` path an author typed: a caller that has
/// a [`PrefabMeta`] and the directory it came from should not have to know
/// whether the piece is one template or nine, because packaging is not part of
/// what the piece IS. `PrefabMeta::templates` is the flattener both cases go
/// through, and its `offset` is already the whole-piece offset.
///
/// Returns the grid and the number of `.nbt` files opened, because a reader
/// that says "I examined this piece" owes its denominator: a manifest whose
/// tiles are missing yields a grid of air and would otherwise answer
/// confidently about nothing.
pub fn piece_grid(
    meta: &delvewright_dsl::prefab::PrefabMeta,
    dir: &std::path::Path,
) -> Result<(VoxelModel, usize), String> {
    let (grid, facts) = piece_bytes(meta, dir)?;
    Ok((grid, facts.opened))
}

/// The block a jigsaw socket IS. Spelled once, for the same reason the
/// waterline's water is: the carver that writes one, the reader that finds one
/// and the check that holds a declaration to one must all mean the same block.
pub const JIGSAW: &str = "minecraft:jigsaw";

/// **A jigsaw block as the bytes hold it** — the socket a piece really has.
///
/// Every field is what the `.nbt` says, never what a document says: the block
/// state's `orientation` property and the strings its block entity carries. A
/// connector declaration is judged against this and against nothing else.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Jigsaw {
    /// The block state's `orientation` property (`north_up`, …), when it writes
    /// one.
    pub orientation: Option<String>,
    /// The block entity's `name`.
    pub name: Option<String>,
    /// The block entity's `target`.
    pub target: Option<String>,
    /// The block entity's `joint`.
    pub joint: Option<String>,
}

/// **What a piece's own files say about themselves**, beside the grid of blocks
/// they assemble into.
///
/// A [`VoxelModel`] carries neither a `DataVersion` nor a block entity, so two
/// facts a document makes claims about are not in it: which game version its
/// templates were written for, and what the jigsaw markers in them say. They are
/// collected here, at the one read, so a checker never has to open a template a
/// second time and no two readers of one piece can come to different answers
/// about it.
///
/// The piece's own EXTENT is deliberately not among them: `DW0803` is the one
/// authority on the declared size against the bytes, and a second measurement
/// carried here is a second answer waiting to be given.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ByteFacts {
    /// `.nbt` templates opened. A denominator: a piece whose tiles are missing
    /// cannot be reported as examined.
    pub opened: usize,
    /// Each opened template's own `DataVersion`, in template order.
    pub data_versions: Vec<i32>,
    /// Every `minecraft:jigsaw` block the piece authors, by piece-local cell.
    pub jigsaws: std::collections::BTreeMap<[i32; 3], Jigsaw>,
}

impl ByteFacts {
    /// Read the facts off structures a caller has already parsed, each with the
    /// piece-local offset it sits at (`[0, 0, 0]` for a single template).
    pub fn of(templates: &[([i32; 3], &Structure)]) -> ByteFacts {
        let mut facts = ByteFacts::default();
        for (offset, s) in templates {
            facts.opened += 1;
            facts.data_versions.push(s.data_version);
            for b in &s.blocks {
                let entry = &s.palette[b.state as usize];
                if entry.name != JIGSAW {
                    continue;
                }
                let be = b.nbt.as_ref().and_then(crate::schem::nbt::Nbt::as_compound);
                let field = |k: &str| {
                    be.and_then(|c| c.get(k))
                        .and_then(crate::schem::nbt::Nbt::as_str)
                        .map(str::to_string)
                };
                facts.jigsaws.insert(
                    [
                        b.pos[0] + offset[0],
                        b.pos[1] + offset[1],
                        b.pos[2] + offset[2],
                    ],
                    Jigsaw {
                        orientation: entry.properties.get("orientation").cloned(),
                        name: field("name"),
                        target: field("target"),
                        joint: field("joint"),
                    },
                );
            }
        }
        facts
    }
}

/// **A whole piece, read once**: the assembled grid and the facts its own files
/// carry.
///
/// [`piece_grid`] is this without the facts, kept because most callers want only
/// the blocks; both go through here, so a piece is never opened twice with two
/// answers about how big it is.
pub fn piece_bytes(
    meta: &delvewright_dsl::prefab::PrefabMeta,
    dir: &std::path::Path,
) -> Result<(VoxelModel, ByteFacts), String> {
    let size = meta.size();
    let mut model = VoxelModel::new(Box3::at_origin([
        size[0].max(0) as u32,
        size[1].max(0) as u32,
        size[2].max(0) as u32,
    ]));
    let mut read: Vec<([i32; 3], Structure)> = Vec::new();
    for t in meta.templates() {
        let path = dir.join(t.file);
        let bytes = std::fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let s = Structure::read(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        blit(&mut model, &s, t.offset);
        read.push((t.offset, s));
    }
    let borrowed: Vec<([i32; 3], &Structure)> = read.iter().map(|(o, s)| (*o, s)).collect();
    Ok((model, ByteFacts::of(&borrowed)))
}

/// What the two rules examined in one piece, and what they found.
///
/// The counts are carried rather than printed because they belong in the
/// REPORT: `delvec prefab audit`'s report is the machine-readable artifact, and a
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
    // The one thing this rule deliberately does not judge, said out loud. A
    // body that reaches the piece's own outer face is a claim about the piece's
    // NEIGHBOUR, and these bytes cannot make it — a shoreline piece's water is
    // the sea. It is also the direction in which the gate could be answered
    // rather than fixed, so a reviewer is told the count every time.
    if !bodies.at_edge.is_empty() {
        let from = bodies.at_edge[0].from;
        diagnostics.push(
            Diagnostic::warning(
                fluid::DW_FLUID_ESCAPES,
                format!(
                    "a body of fluid reaches this piece's own outer face in {} run direction(s) \
                     — what is beyond a face is not in these bytes, so this is counted and not \
                     judged. Whatever this piece is placed against decides where that water goes, \
                     and the compiler holds it to that: `DW0318` refuses a build whose fluid ends \
                     up outside every placed piece under a void horizon",
                    bodies.at_edge.len()
                ),
            )
            .at(from),
        );
    }

    Settling {
        stairs_examined: shapes.bound,
        fluid_cells_examined: bodies.bound,
        fluid_held_cells: bodies.held,
        fluid_at_edge: bodies.at_edge.len(),
        diagnostics,
    }
}
