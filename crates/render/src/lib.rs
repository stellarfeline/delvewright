//! `delvewright-render` — the deterministic render layer (spec-0007 rendering
//! infra / spec-0003 visual tier, M3). Productionizes the spike-render-fidelity
//! spike.
//!
//! - [`nbt`] — vanilla-structure `.nbt` → Nucleation adapter.
//! - [`shots`] — per-piece shot planner (`delve-render piece`).
//! - [`render`] — headless GPU render wrapper (Nucleation / wgpu).
//! - [`detect`] — missing-texture (magenta) color-key scan (the fidelity gate).
//! - [`fidelity`] — the built-in newest-block gate fixture.
//! - [`meta`] — prefab metadata (sockets/anchors) for interior shots.
//! - [`scene`] — Chunky scene emission from the compiler's `render-plan.json`
//!   (free-camera path — the renderer for the first-person player-POV shots).
//! - [`index`] — shot index: (image ↔ expect) pairs for the vision reviewer.
//! - [`diag`] — diagnostics + exit codes (`DW072x`).

pub mod detect;
pub mod diag;
pub mod fidelity;
pub mod index;
pub mod meta;
pub mod nbt;
pub mod render;
pub mod scene;
pub mod shots;
