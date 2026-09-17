//! **Where a scope's frame puts a thing** — the three rules a grammar program
//! and a drawing both apply, extracted so there is one of each.
//!
//! A frame is a signed permutation ([`Orientation`]): which world axis each
//! local axis names, and which way along it local coordinates run. Three
//! questions are asked of it by both producers of an
//! [`Expansion`](crate::grammar::expand::Expansion), and each had exactly one
//! implementation while there was one producer:
//!
//! * **what a state written in the scope's axes says in the world's**
//!   ([`resolve_states`]) — the `local` paint of a program, and every paint of a
//!   drawing;
//! * **which cell of the scope a [`Mark`] names** ([`mark_cell`]);
//! * **which way an unstated [`Mark`] faces** ([`mark_facing`]).
//!
//! They live here rather than on the expander because a second copy of any of
//! them would be a second answer: a drawing whose anchors landed a cell off a
//! program's would be two engines, and the difference would only ever be visible
//! in a built world.

use crate::grammar::eval::EvalError;
use crate::grammar::geom::{Axis, Box3, Orientation};
use crate::grammar::ir::{Expr, Facing, Mark, MarkAt, Side, States};
use crate::schem::blocks::BlockRegistry;

/// A frame written for a person to find: `x->X,y->Y,z->-Z`, local to world,
/// with a leading `-` on an axis that runs backwards.
///
/// A diagnostic that named only the permutation would print `x->X,y->Y,z->Z`
/// for a reflected identity frame — an author reading that would look for a
/// reorientation there is none of, and the reflection that actually turned their
/// block would not appear anywhere in the message.
pub fn frame_label(orient: Orientation) -> String {
    let axis = |local: Axis| {
        format!(
            "{}{:?}",
            if orient.reversed(local) { "-" } else { "" },
            orient.axis(local)
        )
    };
    format!(
        "x->{},y->{},z->{}",
        axis(Axis::X),
        axis(Axis::Y),
        axis(Axis::Z)
    )
}

/// A state whose image the pinned vocabulary does not determine under the frame
/// it was written in — the `DW0738` finding, before it is dressed as one
/// producer's error type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unresolvable {
    /// The state, vanilla string form, as authored in the local frame.
    pub state: String,
    /// The property with no image, as `key=value`.
    pub property: String,
    /// The frame, as [`frame_label`] writes it.
    pub orientation: String,
}

/// **Read states in the scope's own axis names and return them in the world's.**
///
/// The transform is the registry's
/// ([`BlockRegistry::permuted_properties`]) — the same one the `DW0736`
/// predicate runs to decide that an unframed literal landed wrong, so a state
/// one of them calls wrong is never one the other quietly writes.
///
/// It is handed **both halves of the frame**: the axis permutation and the
/// reflection. Handing it the permutation alone would be the short circuit the
/// `DW0736` judge once had, except that here it does not miss a defect, it
/// writes one — a pure reflection has the identity permutation, so every
/// mirrored body would silently take the unmirrored state.
///
/// A property whose image the pinned vocabulary does not determine is refused
/// rather than guessed: there is no correct block to write, and writing a
/// plausible one is how a wrong facing gets frozen into a `.nbt`.
pub fn resolve_states(states: &States, orient: Orientation) -> Result<States, Unresolvable> {
    let registry = BlockRegistry::v1_21_11();
    let perm = [
        orient.axis(Axis::X).index(),
        orient.axis(Axis::Y).index(),
        orient.axis(Axis::Z).index(),
    ];
    let reflected = orient.mirror.axes();
    states.map(|block| {
        match registry.permuted_properties(&block.name, &block.properties, perm, reflected) {
            Ok(properties) => Ok(crate::grammar::block::BlockState {
                name: block.name.clone(),
                properties,
            }),
            Err(property) => Err(Unresolvable {
                state: block.to_string(),
                property,
                orientation: frame_label(orient),
            }),
        }
    })
}

/// Why a [`Mark`] could not be placed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkError {
    /// An `offset` expression could not be evaluated.
    Eval {
        /// The failure.
        error: EvalError,
        /// Which local axis the expression was written on.
        axis: Axis,
    },
    /// The mark aimed at a cell outside the scope it was written in.
    Outside {
        /// The cell it asked for, world space.
        cell: [i64; 3],
    },
}

/// **The world cell a mark names**, refused if it is not one of the scope's own.
///
/// `eval` evaluates one [`Expr`] in whatever environment the caller's scope
/// carries — which is the only thing the two producers differ about, so it is
/// the only thing passed in.
pub fn mark_cell(
    mark: &Mark,
    region: Box3,
    orient: Orientation,
    eval: &mut dyn FnMut(&Expr) -> Result<i64, EvalError>,
) -> Result<[i32; 3], MarkError> {
    let size = region.size;
    // Extent along the world axis a local axis names.
    let extent = |local: Axis| size[orient.axis(local).index()] as i64;
    // Centre of an extent, rounding down; 0 for a degenerate axis, which the
    // bounds check below then reports.
    let mid = |n: i64| (n - 1).max(0) / 2;

    // Offsets from the scope's minimum **world** corner, per world axis.
    //
    // Every `at` but `floor_center` names a cell in LOCAL terms, so it is
    // computed in local coordinates and put through the frame once, at the end:
    // a reflected axis counts from the far end of the box, which is exactly what
    // makes the mirror image of a rule land on the mirror image of its anchor.
    let mut delta = [0i64; 3];
    let mut local = [Option::<i64>::None; 3];
    match &mark.at {
        MarkAt::CornerMin => local = [Some(0), Some(0), Some(0)],
        MarkAt::FloorCenter => {
            // Gravity is a world fact, so this one position ignores the frame
            // entirely — both halves of it.
            delta[Axis::X.index()] = mid(size[Axis::X.index()] as i64);
            delta[Axis::Y.index()] = 0;
            delta[Axis::Z.index()] = mid(size[Axis::Z.index()] as i64);
        }
        MarkAt::FaceCenter { axis, side } => {
            for l in Axis::ALL {
                local[l.index()] = Some(if l == *axis {
                    match side {
                        Side::Min => 0,
                        Side::Max => (extent(l) - 1).max(0),
                    }
                } else {
                    mid(extent(l))
                });
            }
        }
        MarkAt::Offset { x, y, z } => {
            for (l, expr) in [(Axis::X, x), (Axis::Y, y), (Axis::Z, z)] {
                let value = eval(expr).map_err(|error| MarkError::Eval { error, axis: l })?;
                local[l.index()] = Some(value);
            }
        }
    }
    for l in Axis::ALL {
        if let Some(coord) = local[l.index()] {
            delta[orient.axis(l).index()] = orient.offset(l, coord, size);
        }
    }

    let cell = [
        region.origin[0] as i64 + delta[0],
        region.origin[1] as i64 + delta[1],
        region.origin[2] as i64 + delta[2],
    ];
    let inside = (0..3).all(|a| delta[a] >= 0 && delta[a] < size[a] as i64);
    if !inside {
        return Err(MarkError::Outside { cell });
    }
    Ok([cell[0] as i32, cell[1] as i32, cell[2] as i32])
}

/// **Which way a mark faces**: what it declared, or the direction of
/// *decreasing local `Z`* — the way this engine's frame says travel runs.
///
/// Which world direction that is depends on both halves of the frame: the world
/// axis local `Z` names, and whether local `Z` runs down it. `None` for a scope
/// whose local `Z` is the vertical: there is no cardinal direction to derive,
/// and the caller says so rather than guessing.
pub fn mark_facing(mark: &Mark, orient: Orientation) -> Option<Facing> {
    if let Some(f) = mark.facing {
        return Some(f);
    }
    Some(match (orient.axis(Axis::Z), orient.reversed(Axis::Z)) {
        (Axis::Z, false) => Facing::North,
        (Axis::Z, true) => Facing::South,
        (Axis::X, false) => Facing::West,
        (Axis::X, true) => Facing::East,
        (Axis::Y, _) => return None,
    })
}
