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
    /// Resolve foreign worldgen jigsaw markers to their `final_state` (import-time
    /// neutralization).
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
        /// Region `x1,y1,z1:x2,y2,z2` — a gate's span, or furniture's own blocks.
        #[arg(long)]
        region: Option<String>,
        /// Block id (for gate anchors).
        #[arg(long)]
        block: Option<String>,
        /// What the anchor is FOR, from the engine's closed vocabulary: `entry`,
        /// the cell a body arrives at when it enters the area this piece is
        /// placed in; or `furniture`, a `--region` of blocks a body stands beside
        /// and is never walked onto (spec-0065). Omitted, an existing role is
        /// kept.
        #[arg(long)]
        role: Option<String>,
        /// Declare that this anchor has NO role, removing one it carries.
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
    /// **Lift a prefab out of a built campaign** (spec-0106 §3.9): build the
    /// campaign in memory, cut the world as shipped to a scope and write the
    /// piece — its tiles and a document whose every field is derived: the
    /// openings at its boundary, its ground, anchors, stations, contract,
    /// light, the bodies the source stood inside it, what the scope severed
    /// (`residue`) and the lock record of its source. Reads the library at
    /// `--prefabs`; writes `<id>.json` and its tiles into `-o`.
    Extract {
        /// The built campaign's directory.
        campaign: PathBuf,
        /// What to lift: `node/<a>[,node/<b>…]` (one place or several),
        /// `node/<place>/<space>` (a space of a bound piece's contract),
        /// `anchor/<gate>` (a gate station's region) or `x0,y0,z0..x1,y1,z1`
        /// (a cuboid of world cells, inclusive).
        #[arg(long)]
        scope: String,
        /// The prefab library directory to write the piece into.
        #[arg(short = 'o', long)]
        out: PathBuf,
        /// The piece's id. Omitted, it is derived from the scope: the place's
        /// stem, the first and last places' stems, the place and its space,
        /// or the campaign and the cut's low corner.
        #[arg(long)]
        id: Option<String>,
    },
    /// **Seat an extracted prefab as a place** (spec-0106 §3.9): write the
    /// place's `boxes[]` row, its stations, its `details[]` row and one seam per
    /// joined opening, every field derived from the piece but the two the
    /// author decides — where it stands, and which neighbour each opening
    /// joins. Judged before anything is written; an opening answered by
    /// nothing is `DW1022`. Reads the library at `--prefabs`.
    Seat {
        /// The campaign to seat it in.
        campaign: PathBuf,
        /// The extracted piece (`prefab/<id>`).
        prefab: String,
        /// The layout-graph node it becomes (declared there first).
        #[arg(long)]
        node: String,
        /// Where its cuboid's low corner stands, `x,z`.
        #[arg(long)]
        at: String,
        /// The world `y` of its walk plane. Omitted, the piece's bottom course
        /// rests on the highest terrain under its footprint and ring (on a
        /// `solid` site, on the plan's floor).
        #[arg(long)]
        floor: Option<i64>,
        /// Join a declared opening to a connection of the layout graph,
        /// `<opening>=<edge>`; the seam is written at the opening's cells.
        #[arg(long = "join", value_name = "OPENING=EDGE")]
        joins: Vec<String>,
        /// Dispose a declared opening to the site's commons (an `open` site).
        #[arg(long = "commons", value_name = "OPENING")]
        commons: Vec<String>,
        /// Seat a station under another name, `<from>=<to>` — where the
        /// campaign already holds the piece's name (`DW0870`).
        #[arg(long = "rename", value_name = "FROM=TO")]
        renames: Vec<String>,
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
