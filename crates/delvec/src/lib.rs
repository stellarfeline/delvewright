//! `delvec` — the delve creator as a library: the whole engine behind the one
//! binary (ADR-0023 §3, ADR-0025). Every capability is a module here and a
//! subcommand of `src/main.rs`; nothing is a crate of its own except the
//! campaign format, `delvewright_dsl`, which numbers its own version line.
//!
//! - [`admit`]: prefab admission — palette audit, jigsaw sockets, anchors, lighting, catalog cards and the gallery world (`delvec prefab`).

pub mod admit;
