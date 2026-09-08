//! **The rules every prefab generator obeys before it writes bytes.**
//!
//! Five modules, one authority each, and each of them a gate rather than a
//! convenience: [`invariants`] is the `assert!`-shaped record of every
//! debugging lesson a tileset has cost — route walkability, stair-flank
//! sealing, anchor sanity, gravity substrate, sightlines, fluid containment —
//! [`connections`] is the derivation those gates are stated over, which
//! computes each shape-carrying property from the blocks beside the cell and
//! **refuses**, by panic, a face vanilla publishes no answer for — [`walkplane`]
//! and [`waterline`] are the two measurements every generator writes into the
//! document it emits, the piece's own `walk_y` and, where it authors a shore,
//! its own `waterline_y` — and [`document`] is how every generator WRITES that
//! document, merging its own output onto whatever is already there so that a
//! re-run deletes no key it did not write. Neither measurement is ever typed: a
//! number a generator states rather than reads is a claim its own bytes may
//! already have stopped bearing out, and `DW0887`/`DW0888` are where a library
//! learns it did.
//!
//! This is a crate because the alternative was seven copies. Every generator
//! used to reach these two files by `#[path = "../../invariants.rs"]`, a source
//! include: the rule stayed single but the compilation did not, and the unit
//! tests below ran only when somebody named one particular generator's manifest
//! on a `cargo test` command line. A dependency edge is what says *one build,
//! one test run, every generator* — and it costs nothing that mattered, because
//! what had to be true was that these rules never enter the shipped `delvec`,
//! and `[workspace] exclude` in the repository root is what establishes that.
//!
//! The generators depend on this crate; nothing depends on the generators.

pub mod connections;
pub mod document;
pub mod invariants;
pub mod walkplane;
pub mod waterline;
