//! `delvewright-grammar` — Box-Split Grammars as Delvewright's prefab back end
//! (spec-0027, phase 1).
//!
//! A **grammar program** ([`ir::Program`]) is a set of named rules over integer
//! voxel boxes. Expanding one against a box and a seed subdivides that box —
//! `split` cuts an axis into absolute and relative pieces, `reorient` renames
//! the axes, guards select between alternatives by the scope's own dimensions,
//! and leaves fill with block states — until every leaf is a terminal. The
//! result is a [`model::VoxelModel`].
//!
//! The point of the shape (spec-0027 §1): frontier models are semantically
//! right and geometrically weak, so the model authors *rules*, this crate does
//! the geometry, and machine gates judge the result. The grammar program is the
//! artifact of record; the model is derived.
//!
//! ```
//! use delvewright_grammar::{Box3, ExpandOptions, expand, library};
//!
//! let temple = library::temple();
//! let region = Box3::at_origin([13, 14, 21]);
//! let a = expand(&temple, region, &ExpandOptions::seeded(7)).unwrap();
//! let b = expand(&temple, region, &ExpandOptions::seeded(7)).unwrap();
//! assert_eq!(a.model.canonical_bytes(), b.model.canonical_bytes());
//! assert!(a.model.filled_cells() > 0);
//! ```
//!
//! # Provenance
//!
//! This crate is a Rust port of the Box-Split Grammar core of
//! [`yawgmoth/GDMC25`](https://github.com/yawgmoth/GDMC25) (BSD-3-Clause,
//! Copyright 2025 Slothlab — the licence is reproduced verbatim in
//! `LICENSE-GDMC25` beside this crate's manifest), which implements Markus
//! Eger, *Box-Split Grammars* (FDG '22, DOI 10.1145/3555858.3555865). Each
//! module names the upstream file it came from and states where the port
//! deliberately diverges. See `docs/ACKNOWLEDGEMENTS.md`.
//!
//! # Determinism (ADR-0006)
//!
//! Same program + same region + same seed gives byte-identical output. Every
//! random choice comes from [`rng::Rng`] seeded by the caller; every map is a
//! `BTreeMap`; cell iteration is fixed; nothing reads the clock, the
//! environment or a path.
//!
//! # Export
//!
//! [`export::export_prefab`] freezes one expansion as a vanilla structure `.nbt`
//! plus the sibling metadata JSON the compiler's prefab registry reads, carrying
//! a provenance row (program hash + seed) that regenerates those exact bytes.
//! Jigsaw connectors are deliberately not emitted yet, and the lighting profile
//! is `unmeasured` rather than a fabricated measurement — see that module.
//!
//! # Anchors
//!
//! An anchor is metadata, not geometry, so it is **declared**: a rule body wraps
//! the piece it is talking about in [`ir::Node::Mark`], naming a cell of that
//! scope, and expansion collects the declarations into
//! [`Expansion::anchors`](expand::Expansion::anchors) for the export to write
//! out. Nothing infers an anchor from the blocks afterwards.
//!
//! # Composition
//!
//! A `call` reaches only rules of its own program, so a program built out of
//! other programs needs them copied in: [`compose::include`] does that under a
//! prefix, rewriting every rule, parameter and role reference and deliberately
//! leaving anchor names alone — an anchor name is the campaign's contract, not
//! the program's. Moving one is an explicit, per-anchor decision written at the
//! include site ([`compose::include_renaming`]), which is how two pieces that
//! happen to share a stem are composed into one zone. [`library::bell`] is what
//! it all exists for — one grammar program per campaign zone, composed from the
//! staging vocabulary.
//!
//! # Demonstration coverage
//!
//! [`coverage`] counts, over the rule library, how many times each IR construct
//! is written, and reports a zero as a finding: an author is sent to the corpus
//! rather than to the schema, so a construct no example writes does not exist in
//! practice. It measures **demonstration, not expressiveness** — see that
//! module, which says so in its own output.
//!
//! # Not built yet
//!
//! The craft-rule diagnostics of spec-0027 §4, jigsaw connector emission, and
//! the JSON *schema* stage that will sit in front of [`ir::Program`] (which
//! already serialises) are later phases of the same spec. Nothing here is
//! reachable from `delvec`, and nothing here ships in a delve — it is
//! generation-time tooling (ADR-0003).

#![deny(missing_docs)]

pub mod block;
pub mod compose;
pub mod coverage;
pub mod eval;
pub mod expand;
pub mod explain;
pub mod export;
pub mod gates;
pub mod geom;
pub mod ir;
pub mod library;
pub mod model;
pub mod nav;
pub mod orient;
pub mod rng;
pub mod split;

pub use block::BlockState;
pub use compose::{AnchorRenames, ComposeError, entry, include, include_renaming};
pub use expand::{
    Anchor, ExpandError, ExpandOptions, Expansion, Limits, RejectedAlternative, ScopeAt, Stats,
    expand,
};
pub use explain::GuardLeaf;
pub use export::{
    AnchorMetadata, ExportError, PrefabExport, PrefabMetadata, export_prefab, program_hash,
};
pub use gates::{Gate, Report, judge};
pub use geom::{Axis, Box3, Orientation};
pub use ir::{Facing, Mark, MarkAt, MarkIndex, Program, ProgramError, Side};
pub use model::VoxelModel;
