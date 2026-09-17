//! **Where a drawing is refused** (spec-0072 §8): the seven codes, and the
//! address every one of them carries.
//!
//! A refusal is addressed by **the operation's path in the document the creator
//! wrote** — a JSON pointer, `/ops/12/body/3` — followed, inside a define, by
//! the chain of uses that reached it, the `repeat` indices in force and the
//! mirror side. A cell coordinate is added where there is one; it is never the
//! address, because a cell is where the mistake landed and the pointer is where
//! it was made.

use std::fmt;

use delvewright_dsl::{Diagnostic, DwCode, ExitTier};

use crate::drawing::ir::Horizontal;

/// The stage name a drawing's diagnostics carry.
pub const STAGE: &str = "drawing";

/// `DW0902`: **an operation reaches a cell its scope does not hold** — a
/// solid's box, a `line`'s brush, a `mark`, a `claim`, a `grammar` box, outside
/// the place's box or the enclosing `scope` / `use` / `repeat` slice. Refused at
/// that operation, before it paints, naming the operation, the scope it left and
/// both boxes.
///
/// The rule is stated as *a cell its scope does not hold*, never *outside the
/// box*: ADR-0030 §5 will hand a place its **net cells** instead of a box, and a
/// mask is a narrower holding handed to the executor as an input, not a second
/// refusal.
pub const DW_LEFT_ITS_SCOPE: DwCode = DwCode::new("DW0902", ExitTier::Build);

/// `DW0903`: **a name resolves to nothing, or to two things** — a role (on a
/// solid, in `where`, in `use.roles`, in the contract's `block`), a define, a
/// parameter, a region the contract names and nothing claims or the reverse, an
/// anchor two marks produce. Says the kind, the name, and every name of that
/// kind the document declares.
pub const DW_UNKNOWN_NAME: DwCode = DwCode::new("DW0903", ExitTier::Build);

/// `DW0904`: **a define reaches itself**, directly or through others. Refused at
/// validation, before anything executes, with the chain named. There is no
/// recursion in a drawing, so there is no depth to limit.
pub const DW_DEFINE_REACHES_ITSELF: DwCode = DwCode::new("DW0904", ExitTier::Build);

/// `DW0905`: **a value outside its range at the operation that evaluates it** —
/// `to` below `from`, a `t` / `rise` / `run` / `stride` / `item` below 1, a
/// negative `count`, division or remainder by zero, a coordinate past what a
/// cell can be, a `flat` that is not a face of a round axis, a round axis over
/// the arithmetic's own bound, a place past the volume budget, more than the
/// instance ceiling. Names the field, the value, the range and the parameters
/// that produced it.
pub const DW_OUT_OF_RANGE: DwCode = DwCode::new("DW0905", ExitTier::Build);

/// `DW0906`: **a fitted `repeat` does not fit** — `exact` with cells left over,
/// or no whole item. Names `E`, `stride`, `item`, `r` and the three remainder
/// words that would accept it.
pub const DW_DOES_NOT_FIT: DwCode = DwCode::new("DW0906", ExitTier::Build);

/// `DW0907`: **a paint writes a property the engine derives** — `shape` on a
/// stair. Refused at validation rather than silently overwritten, so `DW0801` is
/// unreachable from a drawing and the repair is to omit the property.
pub const DW_DERIVED_PROPERTY: DwCode = DwCode::new("DW0907", ExitTier::Build);

/// `DW0738` as a code a drawing's refusal can carry.
///
/// The **rule** is the block model's, declared once there
/// ([`crate::schem::blocks::DW_LOCAL_FRAME_UNRESOLVABLE`]) as the bare id the
/// schematic and grammar tools print. A drawing's refusals are campaign-facing
/// diagnostics and so carry a tier; the id comes from that one declaration
/// rather than being restated here, so this is a dressing and not a second
/// declaration of the code.
pub const DW_UNRESOLVABLE_FRAME: DwCode = DwCode::new(
    crate::schem::blocks::DW_LOCAL_FRAME_UNRESOLVABLE,
    ExitTier::Build,
);

/// `DW0908`: **a place with two media** — `programs/<stem>.json` and
/// `drawings/<stem>.json` both present. Refused at `delvec detail`, before
/// either is opened, naming both paths.
pub const DW_TWO_MEDIA: DwCode = DwCode::new("DW0908", ExitTier::Build);

/// **Where an operation is, in the document the creator wrote.**
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Address {
    /// The operation's JSON pointer, e.g. `/ops/12/body/3`.
    pub pointer: String,
    /// The chain of `use` sites that reached it, innermost first. Empty for an
    /// operation written at the top level.
    pub through: Vec<String>,
    /// The `repeat` indices in force, outermost first.
    pub indices: Vec<(String, i64)>,
    /// The `mirror` axis this instance is reflected across, when it is the
    /// reflected pass of one.
    pub mirrored: Option<Horizontal>,
    /// The cell, where the refusal has one. Never the address.
    pub cell: Option<[i64; 3]>,
}

impl Address {
    /// The address of the operation at `index` of the container at `base`.
    pub fn at(base: &str, index: usize) -> Address {
        Address {
            pointer: format!("{base}/{index}"),
            ..Address::default()
        }
    }

    /// The same address, with a cell named.
    pub fn at_cell(mut self, cell: [i64; 3]) -> Address {
        self.cell = Some(cell);
        self
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.pointer)?;
        for site in &self.through {
            write!(f, " \u{2190} {site}")?;
        }
        let mut parts: Vec<String> = self
            .indices
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect();
        if let Some(axis) = self.mirrored {
            parts.push(format!("mirrored across {axis}"));
        }
        if !parts.is_empty() {
            write!(f, " ({})", parts.join(", "))?;
        }
        if let Some(c) = self.cell {
            write!(f, " at cell {},{},{}", c[0], c[1], c[2])?;
        }
        Ok(())
    }
}

/// One refused drawing: the code, where it was refused, and what it says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// The rule.
    pub code: DwCode,
    /// The operation, and the chain that reached it.
    ///
    /// Boxed, so a `Result<_, Refusal>` costs a pointer rather than the whole
    /// address — the shape `ExpandError` already takes for its scope record,
    /// and the reason `clippy::result_large_err` exists.
    pub at: Box<Address>,
    /// What is wrong, in the author's own vocabulary.
    pub message: String,
}

impl Refusal {
    /// A refusal at an address.
    pub fn new(code: DwCode, at: Address, message: impl Into<String>) -> Refusal {
        Refusal {
            code,
            at: Box::new(at),
            message: message.into(),
        }
    }

    /// A refusal about the document as a whole — the header, the palette, the
    /// contract — addressed by the pointer of the field that carries it.
    pub fn at_field(code: DwCode, pointer: &str, message: impl Into<String>) -> Refusal {
        Refusal::new(
            code,
            Address {
                pointer: pointer.to_string(),
                ..Address::default()
            },
            message,
        )
    }

    /// The diagnostic a campaign-facing caller prints.
    pub fn diagnostic(&self) -> Diagnostic {
        Diagnostic::error(
            self.code,
            STAGE,
            self.at.pointer.clone(),
            format!("{} — at {}", self.message, self.at),
        )
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {} — at {}", self.code.id(), self.message, self.at)
    }
}

impl std::error::Error for Refusal {}
