//! `delvewright-render` — the **GPU half** of the deterministic render layer
//! (spec-0007 rendering infra / spec-0003 visual tier, M3). Productionizes the
//! spike-render-fidelity spike.
//!
//! This crate is what `nucleation`/`wgpu` reaches, and that is the whole reason
//! it is a crate of its own: the CPU render surface — `viewer`, `scene`,
//! `panorama`, `contact-sheet`, `index`, `palette` — lives in the compiler
//! library and reaches no GPU, while `piece`, `batch` and `fidelity-gate`, the
//! three arms that mesh and rasterise, live here. Both halves are subcommands of
//! the one binary (ADR-0023 §3): this crate's [`cli`] mounts as `delvec render`.
//!
//! - [`render`] — headless GPU render wrapper (Nucleation / wgpu), and the one
//!   place a parsed structure becomes a `UniversalSchematic`.
//! - [`shots`] — per-piece shot planner (`delvec render piece`).
//! - [`view`] — author-declared cameras (`piece --view`): a bearing and a
//!   subject box, in the language `<stem>-shots.json` already writes back. It
//!   aims ONE still frame the renderer then bakes, where `delvec viewer` hands
//!   the camera to a person at review time. Both answer "the planned set is not
//!   square-on at this face"; only this one answers it in a file a report can
//!   cite.
//! - [`occupancy`] — where a body fits inside a prefab, and where its eye goes.
//! - [`detect`] — missing-texture (magenta) color-key scan (the fidelity gate).
//! - [`fidelity`] — the built-in newest-block gate fixture.
//!
//! The CPU pieces these rest on are not copied here — they are the same modules
//! `delvec` carries, named through this crate so the arms above keep one
//! spelling: [`diag`] (the shared `DW072x`/`DW079x` catalog), [`meta`] (prefab
//! metadata — sockets and anchors) and [`nbt`] (the vanilla structure reader).
//! Everything else the CPU surface owns is reached as
//! `delvewright_compiler::view::…` at its use site, so that a reader can always
//! tell which side of the shelf split a thing is on.

/// The frame detectors — **defined in `delvec`**, named here.
///
/// ADR-0021 §1: the shared render surface is one definition and this crate
/// names it. `is_featureless` and `is_magenta` are pure pixel math with no GPU
/// in them, and the CPU arms produce frames that need the same verdicts.
pub use delvewright_compiler::view::detect;
pub mod cli;
pub mod fidelity;
pub mod occupancy;
pub mod render;
pub mod shots;
pub mod view;

// One definition, two spellings — the same arrangement `delvewright_schem` has
// for `prefab`. These modules moved into `delvec` with the CPU surface; naming
// them here keeps `crate::nbt` / `crate::diag` / `crate::meta` inside this
// crate's own modules resolving to that one definition rather than to a copy.
pub use delvewright_compiler::view::{diag, meta, nbt};
