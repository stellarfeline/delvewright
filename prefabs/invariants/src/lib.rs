//! **The rules every prefab generator obeys before it writes bytes.**
//!
//! Three modules, one authority each, and each of them a gate rather than a
//! convenience: [`invariants`] is the `assert!`-shaped record of every
//! debugging lesson a tileset has cost — route walkability, stair-flank
//! sealing, anchor sanity, gravity substrate, sightlines, fluid containment —
//! [`connections`] is the derivation those gates are stated over, which
//! computes each shape-carrying property from the blocks beside the cell and
//! **refuses**, by panic, a face vanilla publishes no answer for — and
//! [`walkplane`] is the one measurement every generator writes into the
//! document it emits, the piece's own `walk_y`.
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
pub mod invariants;
pub mod walkplane;
