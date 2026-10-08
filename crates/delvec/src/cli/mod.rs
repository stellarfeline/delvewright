//! The bodies of the compiler surface's subcommands, one file per object a
//! subcommand acts on. `main.rs` declares `Cli` and `Command` and dispatches
//! here; the mounted surfaces keep their own `cli` modules.

pub(crate) mod campaign;
pub(crate) mod document;
pub(crate) mod edit;
pub(crate) mod l10n;
pub(crate) mod metrics;
pub(crate) mod report;
pub(crate) mod view;
