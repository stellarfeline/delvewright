//! The **CPU render surface** — the part of the visual channel that ships in the
//! one binary a creator installs (ADR-0021 §1).
//!
//! Everything here is CPU: JSON emission, PNG compositing, and a self-contained
//! HTML page. Nothing in this module reaches a GPU, a Vulkan loader or
//! `nucleation`, so every arm below runs on a machine with no adapter. The
//! `view::` prefix is what makes that edge visible in review: a `use
//! nucleation::…` under this directory is the defect, and it has exactly one
//! legitimate home, which is the render crate.
//!
//! The arms it backs, all reachable as `delvec` subcommands:
//!
//! - [`viewer`] — prefabs → one self-contained interactive HTML page, the camera
//!   a reviewer drives.
//! - [`scene`] — Chunky scene emission from the compiler's `render-plan.json`
//!   (the free-camera path behind the first-person player-POV shots).
//! - [`panorama`] — the whole-map 45° oblique release panorama.
//! - [`sheet`] — the contact sheet: many candidate renders on one page, ordered
//!   by a similarity score that RANKS and never gates.
//! - [`index`] — shot index: (image ↔ expect) pairs for the vision reviewer.
//! - [`blockcolor`] — blockstate → colour and shape, derived from the client jar,
//!   which is also what `palette` prints.
//!
//! and the pieces those rest on: [`assets`] (lazy read access to the client jar /
//! resource pack), [`cache`] (Chunky's derived per-scene caches and their
//! invalidation), [`font`] (the built-in 5×7 bitmap font the sheet labels cells
//! with), [`meta`] (prefab metadata — sockets/anchors), [`nbt`] (the vanilla
//! structure reader), [`tileset`] (a zone past the 48-per-axis template cap,
//! reassembled from its manifest so an author reviews one scene and never a
//! fragment), and [`diag`] (the `DW072x` / `DW079x` diagnostics and exit codes,
//! shared with the GPU arms).
//!
//! What is deliberately NOT here is the GPU half — `piece`, `batch` and
//! `fidelity-gate`, which mesh and rasterise through `nucleation`/`wgpu`. Those
//! live in `crates/render` and are built from a checkout rather than shipped on
//! the shelf (ADR-0021 §3). That is a statement about distribution and never
//! about capability: the source build is what guarantees a creator can run every
//! validation the pipeline needs, and the skill's `Init` section builds the arm
//! at the step that needs it.

pub mod assets;
pub mod blockcolor;
pub mod cache;
pub mod cli;
pub mod detect;
pub mod diag;
pub mod font;
pub mod index;
pub mod meta;
pub mod nbt;
pub mod panorama;
pub mod scene;
pub mod sheet;
pub mod tileset;
pub mod viewer;
