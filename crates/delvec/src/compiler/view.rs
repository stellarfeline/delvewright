//! The **render surface** — the visual channel that ships in the one binary a
//! creator installs (ADR-0021 §1, ADR-0023 §3).
//!
//! Every review frame, showcase camera and panorama is drawn here, in this
//! process, by Lumenwright linked as a library ([`frames`], spec-0112 §6): on
//! the GPU wgpu finds, or on the CPU when there is none, the same frame either
//! way. The other arms are CPU: JSON emission, PNG compositing, and a
//! self-contained HTML page. `nucleation` has exactly one legitimate home,
//! which is the prefab-piece render crate (`crates/delvec/src/render`).
//!
//! The arms it backs, all reachable as `delvec` subcommands:
//!
//! - [`frameset`] — `delvec frames`: a build's review set, drawn, with the shot
//!   index beside it.
//! - [`review`] — the worlds a review frame stands in, written by the build.
//! - [`viewer`] — prefabs → one self-contained interactive HTML page, the camera
//!   a reviewer drives.
//! - [`panorama`] — the oblique exterior of the built place, the storybook shot,
//!   solved for a subject.
//! - [`camera`] — the showcase camera record (`design/cameras.json`): a stated
//!   camera on the assembled world, estimated from an approved image or placed
//!   by hand — the one reader of that record.
//! - [`sheet`] — the contact sheet: many candidate renders on one page, ordered
//!   by a similarity score that RANKS and never gates.
//! - [`index`] — the shot index: (image ↔ expect) pairs for the vision reviewer,
//!   each saying what its frame shows.
//! - [`blockcolor`] — blockstate → colour and shape, derived from the client jar,
//!   which is also what `palette` prints.
//!
//! - [`showing`] — what has to be true of a piece before a person is asked to
//!   look at it: somebody measured its light (`DW0894`), and how much of its
//!   roofed floor no body can reach (`DW0895`). Bound at every arm that puts a
//!   piece in front of an eye.
//! - [`sight`] — whether an eye-level frame shows a room or a surface at arm's
//!   length (`DW0893`), and where a body stands back along an anchor's facing so
//!   the frame shows the room it stands in.
//!
//! and the pieces those rest on: [`frames`] (the one function every frame goes
//! through), [`scene`] (the sky a frame is taken under, and the plan it is read
//! from), [`world`] (the one writer of a world), [`assets`] (lazy read access to
//! the client jar / resource pack), [`font`] (the built-in 5×7 bitmap font the
//! sheet labels cells with), [`meta`] (prefab metadata — sockets/anchors),
//! [`nbt`] (the vanilla structure reader), [`tileset`] (a zone past the
//! 48-per-axis template cap, reassembled from its manifest so an author reviews
//! one scene and never a fragment), and [`diag`] (the `DW072x` / `DW079x`
//! diagnostics and exit codes, shared with the prefab-piece arms).

pub mod assets;
pub mod beat;
pub mod blockcolor;
pub mod camera;
pub mod cli;
pub mod detect;
pub mod diag;
pub mod font;
pub mod frames;
pub mod frameset;
pub mod index;
pub mod meta;
pub mod nbt;
pub mod panorama;
pub mod review;
pub mod scene;
pub mod sheet;
pub mod showing;
pub mod sight;
pub mod tileset;
pub mod viewer;
pub mod world;
