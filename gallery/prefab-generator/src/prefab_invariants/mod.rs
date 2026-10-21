//! **The rules the gallery's generator obeys before it writes bytes.**
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
//! A module of the gallery's generator, its only user. `delvewright-dsl` is
//! its one engine dependency; it never reaches `delvec`.

pub mod connections;
pub mod document;
pub mod invariants;
pub mod walkplane;
pub mod waterline;
