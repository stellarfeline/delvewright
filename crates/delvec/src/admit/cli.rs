//! The `delvec prefab` command line, as a type; [`run`] is what runs it.
//!
//! The type is public for one reason beyond the `delvec` binary mounting it:
//! the set of commands this surface has is a fact a TEST must be able to read. Every command
//! that opens a piece an author named owes a lone tile a refusal (`DW0739`), and
//! the only way to check that every one of them does is to enumerate them from
//! the parser itself — a hand-written list of doors is what let the guard reach
//! two of three in the first place. `crates/delvec/tests/prefab_fragment_doors.rs`
//! walks [`PrefabCommand`]'s clap tree; a command added here and classified
//! nowhere is a red.

use std::path::PathBuf;

use clap::{Args, Subcommand};

use crate::admit::light::DEFAULT_DARK_THRESHOLD;
pub use crate::admit::run::run;

/// `delvec prefab`: the command line, as a type.
#[derive(Clone, Args)]
pub struct PrefabArgs {
    #[command(subcommand)]
    pub command: PrefabCommand,
}

#[derive(Clone, Subcommand)]
pub enum PrefabCommand {
    /// **Can this library stand on this horizon** (spec-0060 §6): per pool, the
    /// verdict and the reason, with a numerator and a denominator at pool,
    /// member and declaration level.
    ///
    /// It reads a library and a base and answers the pairing question BEFORE a
    /// campaign is authored against either. Same implementation as the
    /// compiler's own validation check, so a library can never be seatable
    /// according to the tool and refused by the build.
    Seating {
        /// The horizon base to seat against — a base `delvec schema --stage
        /// world` declares (`void`, `ocean`, `valley`).
        #[arg(long)]
        horizon: String,
    },
    /// **Which anchors does a pool guarantee**: per pool, the member count, the
    /// `entry` member every draw seats, the anchor names that member declares —
    /// the whole unconditional guarantee — and every other name in the pool's
    /// vocabulary with the carrier it would have to arrive on.
    ///
    /// It reads a LIBRARY and nothing else, which is the point: the anchors a
    /// campaign hangs its design on are chosen at the third authoring step,
    /// where `quests.json` and `dialogue.json` do not exist yet and every
    /// campaign verb refuses a directory that is not all six documents
    /// (`DW0874`). Same implementation as the compiler's own `DW0889`, so the
    /// tool and the build cannot disagree about what a pool guarantees.
    Anchors {
        /// One pool id (`pool/<name>`). Omitted, every pool the library declares.
        #[arg(long)]
        pool: Option<String>,
    },
    /// Mechanical NBT palette audit (CI gate): allowlist + code-injection forbid.
    Audit {
        /// Input structure `.nbt`; the `.json` manifest of a zone that ships as
        /// a tile set, which audits every tile and returns ONE zone verdict; or
        /// a prefab LIBRARY DIRECTORY, which sweeps every document in it for a
        /// declared waterline its own bytes do not bear out (`DW0887`).
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
    /// **Measure the piece's own planes** and, with `--write`, declare them:
    /// `walk_y` (spec-0060 §4) and, where the piece authors water, `waterline_y`.
    ///
    /// Both are measurements of the bytes, and every generator reads them back
    /// out of the blocks it just laid. This is that measurement for a piece no
    /// generator wrote — an ingested hero asset, a hand-authored room — which
    /// otherwise had no way to state a `walk_y` at all except by typing one, and
    /// a census derivable from the object is never hand-written. It is the
    /// `lighting` verb's shape, for the other two numbers a document declares
    /// about its own bytes.
    Planes {
        /// Input structure `.nbt`, or the `.json` manifest of a zone that ships
        /// as a tile set — which is measured as one assembled building.
        nbt: PathBuf,
        /// Persist the measured planes into the prefab's metadata.
        #[arg(long)]
        write: bool,
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
    /// Build a walkable browse world + datapack from prefabs.
    Gallery {
        /// What to show: a directory of candidate `.nbt` (+ optional sibling
        /// metadata), one piece's `.nbt`, or the `.json` manifest of a zone that
        /// ships as a tile set — which is shown as one whole building.
        path: PathBuf,
        /// Output directory.
        #[arg(short = 'o', long)]
        out: PathBuf,
        /// Gallery id (default: the directory or file name).
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

#[derive(Clone, Subcommand)]
pub enum CatalogCmd {
    /// Validate one or more catalog card files.
    Validate {
        /// Card JSON file(s).
        files: Vec<PathBuf>,
    },
}
