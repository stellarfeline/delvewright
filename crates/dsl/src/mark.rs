//! Marks: a point the campaign places relative to an anchor (spec-0066).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::AnchorId;
use crate::serde_fields::is_zero3;

#[cfg(doc)]
use crate::{Actor, CastPlace, Npc, SoundAt, Verb};

/// **A mark** (spec-0066): an anchor and an integer block offset from it, the
/// one declaration of a point the campaign places relative to a piece.
///
/// Its cell is the anchor's resolved cell plus `offset`, in world axes after
/// the piece's placement. It is what a body stands on ([`Npc::offset`],
/// [`Actor::offset`]), where a walk ends ([`Verb::MoveNpc`], [`Verb::MoveActor`],
/// [`Verb::Teleport`]), where the cast ledger says a body is
/// ([`CastPlace::Mark`]), where a sound plays ([`SoundAt::Anchor`]), and every
/// camera position in a shot: a dolly waypoint (`path`), an aim target
/// (`look_at`) and an anchor subject (`subject`). The roles are fields; the
/// type is one.
///
/// A mark's cell lies inside the placed piece its anchor belongs to (`DW0897`):
/// an offset says *where beside this place*, never *which place*.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Mark {
    /// The anchor the mark is relative to.
    pub anchor: AnchorId,
    /// Integer `[x, y, z]` block offset from the anchor (default `[0, 0, 0]`).
    #[serde(default, skip_serializing_if = "is_zero3")]
    pub offset: [i32; 3],
}

impl Mark {
    /// A mark at `anchor` with no offset.
    pub fn at(anchor: AnchorId) -> Self {
        Mark {
            anchor,
            offset: [0, 0, 0],
        }
    }

    /// Whether the offset is non-zero.
    pub fn is_offset(&self) -> bool {
        !is_zero3(&self.offset)
    }

    /// The mark's cell, given the cell its anchor resolved to.
    pub fn cell(&self, anchor_cell: [i32; 3]) -> [i32; 3] {
        offset_cell(anchor_cell, self.offset)
    }

    /// The mark as a diagnostic spells it: the anchor alone at a zero offset,
    /// else `anchor + [x, y, z]`.
    pub fn display(&self) -> String {
        if self.is_offset() {
            format!(
                "{} + [{}, {}, {}]",
                self.anchor, self.offset[0], self.offset[1], self.offset[2]
            )
        } else {
            self.anchor.as_str().to_string()
        }
    }
}

/// `cell + offset`, componentwise: the one arithmetic a [`Mark`] adds.
pub fn offset_cell(cell: [i32; 3], offset: [i32; 3]) -> [i32; 3] {
    [
        cell[0] + offset[0],
        cell[1] + offset[1],
        cell[2] + offset[2],
    ]
}
