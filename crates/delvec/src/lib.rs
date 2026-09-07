//! `delvec` — the delve creator as a library: the whole engine behind the one
//! binary (ADR-0023 §3, ADR-0025). Every capability is a module here and a
//! subcommand of `src/main.rs`; nothing is a crate of its own except the
//! campaign format, `delvewright_dsl`, which numbers its own version line.
//!
//! - [`render`]: the GPU render arms — per-prefab shot sets and the missing-texture fidelity gate through Nucleation and wgpu (`delvec render`).
//! - [`admit`]: prefab admission — palette audit, jigsaw sockets, anchors, lighting, catalog cards and the gallery world (`delvec prefab`).

pub mod admit;
pub mod render;
