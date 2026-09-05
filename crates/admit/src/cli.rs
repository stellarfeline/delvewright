//! The `delve-admit` command line, as a type.
//!
//! It sits in the library rather than in `main.rs` for one reason: the set of
//! commands this binary has is a fact a TEST must be able to read. Every command
//! that opens a piece an author named owes a lone tile a refusal (`DW0739`), and
//! the only way to check that every one of them does is to enumerate them from
//! the parser itself — a hand-written list of doors is what let the guard reach
//! two of three in the first place. `tests/fragment_doors.rs` walks
//! [`Cli::command()`]; a command added here and classified nowhere is a red.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::light::DEFAULT_DARK_THRESHOLD;

#[derive(Parser)]
#[command(
    name = "delve-admit",
    about = "Prefab admission pipeline: audit, socket carving, anchors, lighting, catalog cards, gallery world",
    version
)]
pub struct Cli {
    /// Emit diagnostics as one JSON object per line.
    #[arg(long, global = true)]
    pub json: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Mechanical NBT palette audit (CI gate): allowlist + code-injection forbid.
    Audit {
        /// Input structure `.nbt`, or the `.json` manifest of a zone that ships
        /// as a tile set — which audits every tile and returns ONE zone verdict.
        nbt: PathBuf,
        /// A JSON allowlist override (replaces the built-in default).
        #[arg(long)]
        allowlist: Option<PathBuf>,
        /// Write the machine-readable report here instead of stdout.
        #[arg(short = 'o', long)]
        report: Option<PathBuf>,
    },
    /// Carve a jigsaw socket into a piece (updates the `.nbt` + metadata).
    Socket {
        nbt: PathBuf,
        /// Jigsaw cell (bottom-centre of the opening): `x,y,z`.
        #[arg(long)]
        pos: String,
        /// Outward facing: north|south|east|west.
        #[arg(long)]
        facing: String,
        /// Opening `w,h` (default 3,3).
        #[arg(long, default_value = "3,3")]
        opening: String,
        /// Jigsaw name (default keep:socket).
        #[arg(long, default_value = "keep:socket")]
        name: String,
        /// Jigsaw target (default keep:socket).
        #[arg(long, default_value = "keep:socket")]
        target: String,
        /// Jigsaw pool (default keep:pool).
        #[arg(long, default_value = "keep:pool")]
        pool: String,
    },
    /// Resolve foreign worldgen jigsaw markers to their `final_state` (import-time
    /// neutralization; run BEFORE `socket`).
    ResolveJigsaw { nbt: PathBuf },
    /// Annotate a named anchor into a piece's metadata.
    Anchor {
        nbt: PathBuf,
        /// Anchor name (e.g. `anchor/npc-stand`).
        #[arg(long)]
        name: String,
        /// Point position `x,y,z`.
        #[arg(long)]
        pos: Option<String>,
        /// Facing keyword for a point anchor.
        #[arg(long)]
        facing: Option<String>,
        /// Gate region `x1,y1,z1:x2,y2,z2`.
        #[arg(long)]
        region: Option<String>,
        /// Block id (for gate anchors).
        #[arg(long)]
        block: Option<String>,
        /// What the anchor is FOR, from the engine's closed vocabulary — today
        /// `entry`, the cell a body arrives at when it enters the area this
        /// piece is placed in. Omitted, an existing role is kept.
        #[arg(long)]
        role: Option<String>,
        /// Declare that this anchor has NO role, removing one it carries — the
        /// remedy `DW0804` prescribes when two anchors in an area both claim
        /// one.
        #[arg(long, conflicts_with = "role")]
        no_role: bool,
    },
    /// Static block-light probe over player space -> declared lighting profile.
    Lighting {
        /// Input structure `.nbt`, or the `.json` manifest of a zone that ships
        /// as a tile set — which reassembles the zone and probes it as one
        /// building.
        nbt: PathBuf,
        /// Persist the measured profile into the prefab's metadata.
        #[arg(long)]
        write: bool,
        /// Dark threshold (floor block-light below this = dark).
        #[arg(long, default_value_t = DEFAULT_DARK_THRESHOLD)]
        dark_threshold: i32,
    },
    /// Validate catalog card(s) (`catalog/<id>.json`).
    Catalog {
        #[command(subcommand)]
        cmd: CatalogCmd,
    },
    /// Build a gallery browse world + datapack from converted candidate pieces.
    Gallery {
        /// Directory of candidate `.nbt` (+ optional sibling metadata).
        dir: PathBuf,
        /// Output directory.
        #[arg(short = 'o', long)]
        out: PathBuf,
        /// Gallery id (default: the directory name).
        #[arg(long)]
        id: Option<String>,
        /// Grid columns.
        #[arg(long, default_value_t = 4)]
        cols: usize,
    },
    /// Harvest a gallery playtest server log into a per-asset curation report.
    Curate {
        /// The server stdout log from the gallery playtest.
        log: PathBuf,
        /// The gallery's `gallery-layout.json`.
        #[arg(long)]
        layout: PathBuf,
        /// Write the curation report here instead of stdout.
        #[arg(short = 'o', long)]
        out: Option<PathBuf>,
    },
    /// Merge a curation report's notes into catalog cards.
    CurateMerge {
        /// The curation report from `curate`.
        report: PathBuf,
        /// The catalog directory (`catalog/<id>.json`).
        #[arg(long)]
        catalog: PathBuf,
    },
}

#[derive(Subcommand)]
pub enum CatalogCmd {
    /// Validate one or more catalog card files.
    Validate {
        /// Card JSON file(s).
        files: Vec<PathBuf>,
    },
}
