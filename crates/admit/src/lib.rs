//! `delve-admit` — the prefab **admission** half of the spec-0007 asset pipeline
//! (M3). Turns an approved, `delve-schem`-converted `.nbt` candidate into a
//! library-grade prefab, and gates community contributions.
//!
//! Modules:
//! - [`audit`] — the mechanical NBT palette audit (CI gate: allowlist + forbid).
//! - [`allowlist`] — the configurable block allowlist.
//! - [`structure`] — an editable structure `.nbt` (read / inspect / mutate / write).
//! - [`socket`] — jigsaw socket carving.
//! - [`jigsaw`] — foreign worldgen jigsaw resolution (import-time neutralization).
//! - [`light`] — the static block-light probe.
//! - [`meta`] — prefab metadata (`<id>.json`), the generator-compatible shape.
//! - [`catalog`] — `catalog/<id>.json` cards + license allowlist.
//! - [`gallery`] — the browse-world emitter + `dw.note` curation harvest.
//!
//! Determinism (ADR-0006): no wall-clock, no unseeded RNG, no hash-order iteration
//! — all maps are `BTreeMap`/sorted, structure bytes are gzip-framed with mtime 0.

pub mod allowlist;
pub mod audit;
pub mod catalog;
pub mod diag;
pub mod fixtures;
pub mod gallery;
pub mod jigsaw;
pub mod light;
pub mod meta;
pub mod socket;
pub mod spatial;
pub mod structure;

/// The deterministic NBT value type (re-exported from `delve-schem`) for callers
/// and tests that build block-entity payloads.
pub use delvewright_schem::nbt::Nbt;
