//! `delvewright-render` — the deterministic render layer (spec-0007 rendering
//! infra / spec-0003 visual tier, M3). Productionizes the spike-render-fidelity
//! spike.
//!
//! - [`nbt`] — vanilla-structure `.nbt` → Nucleation adapter.
//! - [`cutaway`] — which solid the viewer is inside: the shot's half-space clips.
//! - [`shots`] — per-piece shot planner (`delve-render piece`).
//! - [`render`] — headless GPU render wrapper (Nucleation / wgpu).
//! - [`detect`] — missing-texture (magenta) color-key scan (the fidelity gate).
//! - [`fidelity`] — the built-in newest-block gate fixture.
//! - [`meta`] — prefab metadata (sockets/anchors) for interior shots.
//! - [`scene`] — Chunky scene emission from the compiler's `render-plan.json`
//!   (free-camera path — the renderer for the first-person player-POV shots).
//! - [`panorama`] — the whole-map 45° oblique release panorama.
//! - [`cache`] — Chunky's derived per-scene caches, and their invalidation.
//! - [`index`] — shot index: (image ↔ expect) pairs for the vision reviewer.
//! - [`sheet`] — the contact sheet: many candidate renders on one page, ordered
//!   by a similarity score that RANKS and never gates (spec-0027 §3 curation,
//!   spec-0028 §3).
//! - [`font`] — the built-in 5×7 bitmap font the sheet labels cells with.
//! - [`diag`] — diagnostics + exit codes (`DW072x`).

pub mod cache;
pub mod cutaway;
pub mod detect;
pub mod diag;
pub mod fidelity;
pub mod font;
pub mod index;
pub mod meta;
pub mod nbt;
pub mod panorama;
pub mod render;
pub mod scene;
pub mod sheet;
pub mod shots;
