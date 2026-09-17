//! `delvec::drawing` — **a place's detail as an ordered list of solids the
//! engine executes** (ADR-0030, spec-0072).
//!
//! A drawing is a JSON document per place: a palette of roles, a set of named
//! bodies, a spatial contract, and an ordered list of thirteen operations over
//! the place's box in its own frame. **A later operation overwrites an earlier
//! one** — which is what a designer does, and what a box-split `Program` cannot
//! do, because a partition gives every cell to exactly one leaf.
//!
//! ```text
//! delvec drawing check   drawings/gatehouse.json
//! delvec drawing execute drawings/gatehouse.json --region 27x56x20 -o out/
//! ```
//!
//! # What it produces, and why that is the whole of it
//!
//! [`execute::execute`] yields an [`Expansion`](crate::grammar::expand::Expansion)
//! — the same thing the grammar expander yields. Everything after the blocks
//! already takes one: the gates read a `VoxelModel`, the declared anchors and
//! the resolved contract; the contract checker, the stair and fluid gates, the
//! light probe and `delvec detail`'s later steps read the same three things.
//! So a second producer inherits every gate the engine has, and this module adds
//! none of its own.
//!
//! # Determinism (ADR-0006)
//!
//! **Geometry has no seed.** Every rasterisation rule is stated in integers over
//! a box's extents and a cell's index in it; nothing is halved, nothing is
//! rounded from a float, and nothing reads a clock, an environment or a hash
//! order. The seed reaches **weighted paints** and nothing else, and it reaches
//! them by position: the draw at a cell is a pure function of the seed and the
//! cell's linear index, so editing one operation re-textures no other cell.
//!
//! # What the engine knows, the creator does not type
//!
//! A stair's `shape` is derived after the last operation, from the derivation
//! the pinned server was measured against, so a drawing that types one is
//! refused (`DW0907`) rather than silently overwritten. Connection state —
//! fences, walls, panes, bars — is **not** derived: the rule the tree holds is
//! an unmeasured reading of the game's code over a hand-declared table, so a
//! drawing writes its connections in full as every producer does today, and
//! `DW0735` binds it exactly as it binds a program. That is spec-0072
//! criterion 2's work and is recorded there as a debt, not as a pass.

#![deny(missing_docs)]

/// What the `generator` breadcrumb of a piece a drawing produced says.
pub const GENERATOR: &str = "crates/delvec/src/drawing";

/// **Where a campaign keeps its drawings**: `<campaign>/drawings/<place
/// stem>.json`.
///
/// The address is derived from the place exactly as a program's is (spec-0058
/// §2.1); the document at it is the creator's. A place whose detail is written
/// at BOTH addresses is refused before either is opened (`DW0908`): two media
/// are two buildings, and nothing can choose between them but the author.
pub const DRAWINGS_DIR: &str = "drawings";

pub mod cli;
pub mod diag;
pub mod execute;
pub mod ir;
pub mod load;
pub mod raster;

pub use diag::{Address, Refusal, STAGE};
pub use execute::{Census, ExecuteOptions, Execution, Frame, RunReport, check, execute};
pub use ir::{Define, Drawing, Op};
pub use load::{from_str, load};

/// **The drawing document's JSON Schema**, titled and described where it is
/// exported.
///
/// Derived from the Rust types like every other schema the engine exports, and
/// the shared half of it — `Expr`, `Cond`, `AnchorMark`, `Contract` and their
/// members — is generated from the **program document's own** types, so the two
/// documents cannot drift about what an expression or a contract is.
///
/// It lives here rather than in the binary because the **loader** reads it: the
/// forms a value accepts are resolved out of this schema at the pointer that
/// failed, so the refusal an author meets and the schema they were authoring
/// against are one document.
pub fn schema() -> serde_json::Value {
    let mut v = serde_json::to_value(schemars::schema_for!(ir::Drawing))
        .expect("the drawing schema serializes to JSON");
    if let Some(obj) = v.as_object_mut() {
        obj.insert("title".into(), serde_json::Value::String("Drawing".into()));
        // **Where the document lives, said by the engine that reads it.**
        // A stage document is one file named `<stage>.json`, so a tool walking
        // a campaign directory could derive every address from the export's own
        // keys. A drawing is one file per place under `drawings/`, and the
        // moment a document class stops being one file that derivation is a
        // guess: the tool would hand-write `drawings` beside `DRAWINGS_DIR`,
        // which is the second authority the export exists to prevent.
        obj.insert(
            "documents".into(),
            serde_json::Value::String(format!("{DRAWINGS_DIR}/*.json")),
        );
        obj.insert(
            "description".into(),
            serde_json::Value::String(
                "A place's detail as an ordered list of solids the engine executes (ADR-0030). \
                 One document per place, at `drawings/<place stem>.json` inside the campaign, \
                 executed in the place's box in the box's own frame: `x` east, `y` up, `z` south, \
                 the origin at the box's minimum corner, every coordinate a cell. A later \
                 operation overwrites an earlier one."
                    .into(),
            ),
        );
    }
    v
}
