//! **The two places where a piece's bytes are not what the world will hold.**
//!
//! Every gate before this one judges the model as written. These two judge it
//! as the game will *settle* it — because two of the things a builder writes
//! are not stored facts but claims the world re-derives:
//!
//! - a stair's `shape`, which vanilla recomputes from the stair's neighbours on
//!   every horizontal block update at that cell
//!   ([`delvewright_schem::stairs`]);
//! - a body of fluid, which runs the moment the chunk ticks
//!   ([`delvewright_schem::fluid`]).
//!
//! Both fail in the same, worst way. Nothing upstream of the server disagrees:
//! the `.nbt` carries the authored state, the review render draws it, the
//! contact sheet the owner approves shows it, every gate is green — and the
//! world shows something else. A mitred kerb pointed across its run survives
//! every tool this project owns and flattens to `straight` in game; a channel
//! with one wall course missing renders as a channel and ships as a flood.
//!
//! # Why these are engine gates and not one zone's checks
//!
//! Both are properties of *any* piece with stairs or fluid in it, in any
//! campaign, in any genre — the rule reads blocks and knows nothing about what
//! the building is for. The zone round that first found each of them wrote
//! them as assertions about one zone's coordinates, which is the form that
//! protects one zone once. Everything else that round wrote — where a doorway
//! is, how wide a channel runs — is a claim about that zone's design and stays
//! there.

use delvewright_schem::blocks::BlockRegistry;
use delvewright_schem::fluid::{self, Wetness};
use delvewright_schem::stairs::{self, Facing, Half, Shape, Stair};

use crate::model::VoxelModel;

/// One stair whose written `shape` is not the shape vanilla derives at its
/// cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShapeMismatch {
    /// Where it stands.
    pub cell: [i32; 3],
    /// The block id, for the author to find it by.
    pub block: String,
    /// Which way it faces, which is what the derivation turns on.
    pub facing: Facing,
    /// Which half it occupies.
    pub half: Half,
    /// What the piece says.
    pub authored: Shape,
    /// What the game will say.
    pub derived: Shape,
}

/// What the stair gate examined and what it found.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ShapeAudit {
    /// **Stairs examined** — the gate's binding count. Zero means the piece has
    /// no stairs, and the verdict binds to nothing.
    pub bound: usize,
    /// Every disagreement, in cell order.
    pub mismatches: Vec<ShapeMismatch>,
}

/// Read a cell as a stair, if it is one.
fn stair_at(registry: &BlockRegistry, model: &VoxelModel, pos: [i32; 3]) -> Option<Stair> {
    let state = model.get(pos)?;
    if !registry.is_stairs(&state.name) {
        return None;
    }
    Some(Stair {
        facing: Facing::parse(state.properties.get("facing")?)?,
        half: Half::parse(state.properties.get("half")?)?,
    })
}

/// **Every stair in the piece, judged against the shape vanilla derives for
/// it.**
///
/// A cell outside the region reads as "no stair", which is the only thing a
/// single piece can say: it is judged on its own bytes, and the neighbour a
/// placed piece will actually have belongs to the face contract, not here.
pub fn stair_shapes(model: &VoxelModel) -> ShapeAudit {
    let registry = BlockRegistry::v1_21_11();
    let mut audit = ShapeAudit::default();
    for pos in model.region().positions() {
        let Some(state) = model.get(pos) else {
            continue;
        };
        if !registry.is_stairs(&state.name) {
            continue;
        }
        audit.bound += 1;
        // A stair that writes no `shape` at all makes no claim here, so
        // nothing can disagree with it.
        let Some(authored) = state.properties.get("shape").and_then(|s| Shape::parse(s)) else {
            continue;
        };
        let Some(here) = stair_at(registry, model, pos) else {
            continue;
        };
        let derived = stairs::derive_shape(here, |dir| {
            let step = dir.step();
            stair_at(
                registry,
                model,
                [pos[0] + step[0], pos[1] + step[1], pos[2] + step[2]],
            )
        });
        if derived != authored {
            audit.mismatches.push(ShapeMismatch {
                cell: pos,
                block: state.name.clone(),
                facing: here.facing,
                half: here.half,
                authored,
                derived,
            });
        }
    }
    audit
}

/// The one-line message the `stair-shape` gate carries when it is red.
pub fn shape_detail(audit: &ShapeAudit) -> String {
    let named: Vec<String> = audit
        .mismatches
        .iter()
        .take(6)
        .map(|m| {
            format!(
                "{},{},{} {} facing={} half={} is written shape={} and derives shape={}",
                m.cell[0], m.cell[1], m.cell[2], m.block, m.facing, m.half, m.authored, m.derived
            )
        })
        .collect();
    format!(
        "{}: {} of {} stair(s) claim a `shape` the game does not derive at their cell — {}{}. A \
         stair's shape is NOT stored: vanilla recomputes it from the stair's neighbours on every \
         horizontal block update, so this piece renders one way in every tool here and resets in \
         the world. Build the neighbours the shape needs, or write the shape the neighbours give",
        stairs::DW_STAIR_SHAPE_DERIVED,
        audit.mismatches.len(),
        audit.bound,
        named.join("; "),
        if audit.mismatches.len() > named.len() {
            format!(" (+{} more)", audit.mismatches.len() - named.len())
        } else {
            String::new()
        }
    )
}

/// Why one cell of fluid will not stay where it was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Leak {
    /// The cell beside or below it is open, so the body runs into it.
    Escape {
        /// The source cell.
        from: [i32; 3],
        /// The cell it runs into.
        into: [i32; 3],
        /// What is in that cell, as written.
        into_block: String,
    },
    /// A fluid cell written mid-flow. `level` is a value the game derives and
    /// re-derives; a body of fluid is saturated by construction (spec-0038).
    NotSaturated {
        /// The cell.
        cell: [i32; 3],
        /// The block as written.
        block: String,
        /// Its `level`.
        level: u8,
    },
}

/// A source cell on the piece's own outer face, or beside a `structure_void`:
/// the fluid runs into a cell **this piece does not decide**.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtTheEdge {
    /// The source cell.
    pub from: [i32; 3],
    /// The direction it runs, as an offset.
    pub step: [i32; 3],
}

/// What the fluid gate examined and what it found.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FluidAudit {
    /// **Fluid cells examined** — the gate's binding count, and the cells the
    /// obligations bind: `minecraft:water` / `minecraft:lava` blocks, the only
    /// fluid that runs. Zero means the piece has no body of fluid in it and
    /// the verdict binds to nothing.
    pub bound: usize,
    /// Cells written `waterlogged=true`. Wet, measured not to spread, and
    /// therefore under no obligation — counted so that "this piece holds no
    /// fluid" and "this piece holds only still fluid" are different lines.
    pub held: usize,
    /// Every way the body will move, in cell order. **This is what reddens the
    /// gate**, and every one of them is decided by the piece's own bytes.
    pub leaks: Vec<Leak>,
    /// Every run direction that leaves the piece. Reported, never red — see
    /// [`fluid_bodies`].
    pub at_edge: Vec<AtTheEdge>,
}

/// **Every body of fluid in the piece, judged on whether it stays where it was
/// written.**
///
/// Two obligations, and they are one rule — *this fluid is where the author put
/// it and stays there* — reported under one code because they are one fact a
/// reviewer needs (the `DW0727` precedent):
///
/// - **saturated**: every fluid cell is a source. A `level` other than 0 is a
///   state the game derives from its neighbours and re-derives on its own
///   clock; a piece cannot pin one (spec-0038's ruling, and ADR-0006 — a world
///   that heals no longer matches the bytes that built it).
/// - **contained**: no source has an open cell beside or below it. Open means
///   air, and only air: a block written `waterlogged=false` is a wall, which is
///   measured rather than assumed ([`delvewright_schem::fluid`]).
///
/// Fluid never runs upward, so a body's open top is not a leak — an authored
/// pool is a pool.
///
/// # Where the piece stops being able to promise anything
///
/// A source on the piece's own outer face runs into a cell that is not in the
/// piece: the sea a shoreline piece sits in, or the next piece along. Those are
/// **enumerated with their count and never red**, because the fact is not in
/// these bytes — three shipped, owner-accepted prefabs (a beach camp, a galley,
/// a cave shore) are shoreline pieces whose water is the ocean, and a gate that
/// called them broken would be wrong rather than strict.
///
/// The direction that leaves open is stated rather than hidden: an author could
/// in principle answer a real red by extending the body to the region face, and
/// this gate would then only count it. **What closes it is the placement**, and
/// it is closed: the compiler's `DW0318` takes the assembled world and refuses
/// any fluid cell lying outside every placed piece under a void horizon, where
/// there is nothing beyond a face to hold it. That is the deferral answered at
/// the layer that has the fact, rather than a fifth declaration surface here
/// asking an author to promise it. The count is still printed on every run,
/// because it is what tells a reviewer that this piece is relying on its
/// neighbour.
pub fn fluid_bodies(model: &VoxelModel) -> FluidAudit {
    let mut audit = FluidAudit::default();
    // Down first, then the four sides: the order fluid itself takes, so the
    // first named leak is the one an author will see first.
    let runs: [[i32; 3]; 5] = [
        [0, -1, 0],
        Facing::North.step(),
        Facing::South.step(),
        Facing::East.step(),
        Facing::West.step(),
    ];
    for pos in model.region().positions() {
        let Some(state) = model.get(pos) else {
            continue;
        };
        match fluid::wetness(&state.name, &state.properties) {
            Wetness::Dry => continue,
            // Wet and still. It is not a body that runs, so it owes nothing —
            // and it is a wall for anything beside it, like any other block.
            Wetness::Held => {
                audit.held += 1;
                continue;
            }
            Wetness::Flowing(level) => {
                audit.bound += 1;
                audit.leaks.push(Leak::NotSaturated {
                    cell: pos,
                    block: state.name.clone(),
                    level,
                });
                continue;
            }
            Wetness::Source => audit.bound += 1,
        }
        for step in runs {
            let into = [pos[0] + step[0], pos[1] + step[1], pos[2] + step[2]];
            match model.get(into) {
                None => audit.at_edge.push(AtTheEdge { from: pos, step }),
                Some(neighbour) if fluid::is_structure_void(&neighbour.name) => {
                    audit.at_edge.push(AtTheEdge { from: pos, step })
                }
                // Another fluid cell is the same body, not a way out of it.
                Some(neighbour) if fluid::is_fluid(&neighbour.name) => {}
                Some(neighbour) => {
                    if !fluid::holds_fluid(&neighbour.name, &neighbour.properties) {
                        audit.leaks.push(Leak::Escape {
                            from: pos,
                            into,
                            into_block: neighbour.to_string(),
                        });
                    }
                }
            }
        }
    }
    audit
}

/// The one-line message the `fluid-contained` gate carries when it is red.
pub fn fluid_detail(audit: &FluidAudit) -> String {
    let named: Vec<String> = audit
        .leaks
        .iter()
        .take(6)
        .map(|leak| match leak {
            Leak::Escape {
                from,
                into,
                into_block,
            } => format!(
                "{},{},{} runs into {},{},{} ({})",
                from[0], from[1], from[2], into[0], into[1], into[2], into_block
            ),
            Leak::NotSaturated { cell, block, level } => format!(
                "{},{},{} is {} at level={} — mid-flow, not a source",
                cell[0], cell[1], cell[2], block, level
            ),
        })
        .collect();
    format!(
        "{}: {} way(s) out of a body of {} fluid cell(s) — {}{}. A body of fluid is saturated and \
         walled by construction: every cell a source, and nothing open beside or below it. This \
         piece renders as still water in every tool here and runs on the first tick in the world",
        fluid::DW_FLUID_ESCAPES,
        audit.leaks.len(),
        audit.bound,
        named.join("; "),
        if audit.leaks.len() > named.len() {
            format!(" (+{} more)", audit.leaks.len() - named.len())
        } else {
            String::new()
        }
    )
}

/// The line the gate carries when it holds: what it examined, what it counted,
/// and what it could not decide.
pub fn fluid_summary(audit: &FluidAudit) -> String {
    let mut s = format!(
        "{} fluid cell(s), every one a source with nothing open beside or below it",
        audit.bound
    );
    if audit.held > 0 {
        s.push_str(&format!(
            "; {} cell(s) written `waterlogged=true`, which hold their water and spread nothing",
            audit.held
        ));
    }
    if !audit.at_edge.is_empty() {
        let named: Vec<String> = audit
            .at_edge
            .iter()
            .take(3)
            .map(|e| format!("{},{},{}", e.from[0], e.from[1], e.from[2]))
            .collect();
        s.push_str(&format!(
            "; {} run direction(s) leave the piece (from {}{}) — what is beyond a face is not in \
             these bytes, so this is counted and not judged. The placement decides it: `DW0318` \
             refuses this water if the piece is placed against nothing under a void horizon",
            audit.at_edge.len(),
            named.join(", "),
            if audit.at_edge.len() > named.len() {
                " and others"
            } else {
                ""
            }
        ));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::BlockState;
    use crate::geom::Box3;

    fn model(size: [u32; 3]) -> VoxelModel {
        VoxelModel::new(Box3::at_origin(size))
    }

    fn put(m: &mut VoxelModel, pos: [i32; 3], state: &str) {
        m.set(pos, &state.parse::<BlockState>().unwrap()).unwrap();
    }

    /// A stone shell with one interior cell, so a body of fluid in the middle
    /// is contained by construction and every escape below is one the test put
    /// there.
    fn walled(interior: [i32; 3]) -> VoxelModel {
        let mut m = model([3, 3, 3]);
        for pos in m.region().positions() {
            if pos != interior {
                put(&mut m, pos, "minecraft:stone");
            }
        }
        m
    }

    #[test]
    fn a_walled_source_is_contained_and_binds() {
        let mut m = walled([1, 1, 1]);
        put(&mut m, [1, 1, 1], "minecraft:water[level=0]");
        let audit = fluid_bodies(&m);
        assert_eq!(audit.bound, 1);
        assert!(audit.leaks.is_empty(), "{:?}", audit.leaks);
    }

    #[test]
    fn an_open_cell_beside_a_source_is_a_leak() {
        let mut m = walled([1, 1, 1]);
        put(&mut m, [1, 1, 1], "minecraft:water[level=0]");
        put(&mut m, [2, 1, 1], "minecraft:air");
        let audit = fluid_bodies(&m);
        assert_eq!(audit.leaks.len(), 1);
        assert!(matches!(audit.leaks[0], Leak::Escape { into, .. } if into == [2, 1, 1]));
    }

    /// A grate in the wall of a basin. Measured on the pinned server: spreading
    /// water does not fill a block written dry, so the grate is a wall — and
    /// the same grate written wet is a still cell that spreads nothing, so it
    /// is a wall too, counted separately.
    #[test]
    fn a_grate_in_the_wall_is_a_wall_wet_or_dry() {
        let mut m = walled([1, 1, 1]);
        put(&mut m, [1, 1, 1], "minecraft:water[level=0]");
        put(
            &mut m,
            [2, 1, 1],
            "minecraft:iron_bars[east=false,north=true,south=true,waterlogged=false,west=false]",
        );
        let audit = fluid_bodies(&m);
        assert!(audit.leaks.is_empty(), "{:?}", audit.leaks);
        assert_eq!(audit.bound, 1);
        assert_eq!(audit.held, 0);
        put(
            &mut m,
            [2, 1, 1],
            "minecraft:iron_bars[east=false,north=true,south=true,waterlogged=true,west=false]",
        );
        let audit = fluid_bodies(&m);
        assert!(audit.leaks.is_empty(), "{:?}", audit.leaks);
        assert_eq!(audit.bound, 1, "the fluid block is the body");
        assert_eq!(audit.held, 1, "the wet grate is counted, and owes nothing");
    }

    #[test]
    fn a_flowing_cell_is_not_a_body() {
        let mut m = walled([1, 1, 1]);
        put(&mut m, [1, 1, 1], "minecraft:water[level=3]");
        let audit = fluid_bodies(&m);
        assert_eq!(audit.bound, 1);
        assert!(matches!(
            audit.leaks[0],
            Leak::NotSaturated { level: 3, .. }
        ));
    }

    #[test]
    fn an_open_top_is_not_a_leak() {
        let mut m = walled([1, 1, 1]);
        put(&mut m, [1, 1, 1], "minecraft:water[level=0]");
        put(&mut m, [1, 2, 1], "minecraft:air");
        assert!(fluid_bodies(&m).leaks.is_empty());
    }

    #[test]
    fn fluid_on_the_pieces_own_face_is_counted_and_not_judged() {
        let mut m = model([1, 1, 1]);
        put(&mut m, [0, 0, 0], "minecraft:water[level=0]");
        let audit = fluid_bodies(&m);
        assert_eq!(audit.bound, 1);
        assert!(audit.leaks.is_empty(), "{:?}", audit.leaks);
        assert_eq!(audit.at_edge.len(), 5, "one per run direction");
        assert!(fluid_summary(&audit).contains("leave the piece"));
    }

    #[test]
    fn a_lone_stair_written_straight_agrees() {
        let mut m = model([3, 1, 3]);
        put(
            &mut m,
            [1, 0, 1],
            "minecraft:oak_stairs[facing=north,half=bottom,shape=straight,waterlogged=false]",
        );
        let audit = stair_shapes(&m);
        assert_eq!(audit.bound, 1);
        assert!(audit.mismatches.is_empty());
    }

    #[test]
    fn a_corner_written_where_no_corner_is_derived_is_red() {
        let mut m = model([3, 1, 3]);
        put(
            &mut m,
            [1, 0, 1],
            "minecraft:oak_stairs[facing=north,half=bottom,shape=outer_left,waterlogged=false]",
        );
        let audit = stair_shapes(&m);
        assert_eq!(audit.mismatches.len(), 1);
        assert_eq!(audit.mismatches[0].derived, Shape::Straight);
        assert_eq!(audit.mismatches[0].authored, Shape::OuterLeft);
        // The same claim with the stair the corner needs in front of it: north
        // is −z, and north's counter-clockwise is west.
        put(
            &mut m,
            [1, 0, 0],
            "minecraft:oak_stairs[facing=west,half=bottom,shape=straight,waterlogged=false]",
        );
        let audit = stair_shapes(&m);
        assert_eq!(audit.bound, 2);
        assert!(audit.mismatches.is_empty(), "{:?}", audit.mismatches);
    }

    #[test]
    fn a_stair_mitres_against_a_stair_of_another_material() {
        let mut m = model([3, 1, 3]);
        put(
            &mut m,
            [1, 0, 1],
            "minecraft:oak_stairs[facing=north,half=bottom,shape=outer_left,waterlogged=false]",
        );
        put(
            &mut m,
            [1, 0, 0],
            "minecraft:stone_brick_stairs[facing=west,half=bottom,shape=straight,waterlogged=false]",
        );
        assert!(stair_shapes(&m).mismatches.is_empty());
    }
}
