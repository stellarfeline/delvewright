//! Freezing an expansion into a prefab: a vanilla structure `.nbt` plus the
//! sibling metadata JSON the rest of the pipeline already reads (spec-0027 §2,
//! acceptance criterion 4).
//!
//! The grammar program is the artifact of record; the `.nbt` is a *snapshot* of
//! one expansion of it. So the export takes the program, the region and the
//! options — not a finished [`VoxelModel`] — and expands them itself. That is
//! what makes the provenance row unforgeable: the hash and the seed in the
//! metadata are the ones that produced the bytes beside it, because there was no
//! opportunity for them to be anything else.
//!
//! The `.nbt` bytes come from [`delvewright_schem::convert::build_region`], the
//! same emitter the `.schem` asset pipeline uses, so a grammar prefab and a
//! hand-built one are byte-shaped identically: sorted palette, `x`→`y`→`z` block
//! order, gzip with a pinned mtime (ADR-0006).
//!
//! Anchors come from the rules themselves: `anchors` holds exactly what the
//! program's [`Node::Mark`](crate::ir::Node::Mark) declarations produced, in the
//! hand-built `{ pos, facing }` shape, and is `{}` for a program that marks
//! nothing. Nothing here reads the block pattern to guess at one (no-hack:
//! post-hoc inference is exactly the downstream folklore the layering rule
//! forbids).
//!
//! # Zones larger than one structure template
//!
//! Vanilla caps a structure template at 48 blocks per axis. That cap is a
//! **packaging** fact about a file format, and a creator's design must never
//! bend to satisfy it: a zone bigger than 48 on some
//! axis is exported as a *tile set* — several `.nbt` files plus one manifest —
//! and [`export_zone`] decides which of the two shapes it writes from the region
//! alone. Nothing an author writes mentions 48.
//!
//! The cut positions are not this module's invention. `delve-schem` has tiled
//! oversize `.schem` imports since spec-0007, so
//! [`delvewright_schem::split::plan_split`] is *the* tiling of this project, and
//! grammar export calls it. Two paths that tile the same volume the same way
//! need one reassembler, not two.
//!
//! A tile is packaging and nothing else. The gates judge the whole expansion
//! ([`crate::gates`]), the palette legality check below runs over the whole
//! model, and the anchors in the manifest are zone-relative — so no verdict and
//! no coordinate in the output depends on where a cut fell.
//!
//! # What this module deliberately does not emit
//!
//! * **Connectors.** Jigsaw socketing of grammar prefabs needs the tileset
//!   conventions to be settled first; a guessed socket is worse than none. The
//!   key is emitted as an empty list rather than omitted: "this piece has no
//!   sockets" and "this metadata predates sockets" are not the same claim, and
//!   `delve-admit socket` appends to it afterwards.
//! * **A lighting measurement.** See [`LIGHTING_PROFILE`].

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use sha2::{Digest, Sha256};

use delvewright_schem::convert::{self, DATA_VERSION};
use delvewright_schem::schematic::{BlockState as SchemBlockState, ParsedSchematic};
/// The tiling contract, defined once in the crate that owns tiling and re-exported
/// here so a manifest writer and a manifest reader can never drift apart.
pub use delvewright_schem::split::{TilePart, TileSet};
use delvewright_schem::split::{part_filename, plan_split};

use crate::expand::{ExpandError, ExpandOptions, Expansion, expand};
use crate::geom::Box3;
use crate::ir::Program;
use crate::model::VoxelModel;

/// What the `generator` breadcrumb of an exported structure says.
pub const GENERATOR: &str = "crates/grammar";

/// The lighting profile every grammar-exported prefab carries.
///
/// A prefab's lighting profile is a **measurement**, taken by the live 1.21.11
/// probe loop the hand-built pieces went through. Expansion cannot know it: the
/// grammar places blocks, not photons. Declaring `lit` here would be a fabricated
/// measurement, and declaring nothing would be indistinguishable from legacy
/// metadata that predates the field — so the export declares `unmeasured`, which
/// is the true statement, and admission to a campaign still runs the probe.
pub const LIGHTING_PROFILE: &str = delvewright_schem::prefab::UNMEASURED;

/// Vanilla caps a structure template at 48 blocks per axis.
///
/// This is the largest volume **one file** can hold, not the largest zone a
/// creator may design. [`export_zone`] absorbs the cap by tiling; the only
/// caller for which it is still a refusal is [`export_prefab`], the
/// single-template writer that tiling is built out of.
pub const MAX_STRUCTURE_AXIS: u32 = 48;

// ---------------------------------------------------------------------------
// Provenance
// ---------------------------------------------------------------------------

/// The canonical bytes a program's hash is taken over: its serde JSON form.
///
/// The IR round-trips through JSON losslessly *and* stably (`tests/library.rs`),
/// and every map in it is a `BTreeMap`, so these bytes depend on the program's
/// content and nothing else — not on authoring order, not on how the program was
/// built (`Program::new(..).rule(..)` and `serde_json::from_str` give the same
/// hash), and not on this crate's internal layout.
pub fn canonical_program_bytes(program: &Program) -> Vec<u8> {
    serde_json::to_vec(program).expect("a Program serialises to JSON")
}

/// `sha256:<64 hex digits>` over [`canonical_program_bytes`].
pub fn program_hash(program: &Program) -> String {
    let digest = Sha256::digest(canonical_program_bytes(program));
    let mut out = String::with_capacity(7 + 64);
    out.push_str("sha256:");
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

// ---------------------------------------------------------------------------
// Metadata
// ---------------------------------------------------------------------------

// The metadata document's shape is `delvewright_schem::prefab` — the crate that
// also writes the `.nbt` half of the pair. This module produces the document; it
// does not define it. A private definition here would mean the admission tools
// that read the file back parse it through a *different* type, and a type that
// models fewer fields deletes the rest the first time it writes: that is exactly
// how `license.generated_by` — the ADR-0006 row this whole module exists to emit
// — got dropped by the next documented step in the procedure.
pub use delvewright_schem::prefab::{
    Anchor as AnchorMetadata, Connector, ContractBar, ContractEdge, ContractFace, ContractNoBody,
    ContractSpace, ContractVolume, GeneratedBy, License as LicenseMetadata,
    Lighting as LightingMetadata, PrefabMeta as PrefabMetadata, Region as RegionMetadata,
    SpatialContract, StructureMeta as StructureMetadata,
};

/// The manifest of a zone too big for one structure template.
///
/// Defined beside [`PrefabMetadata`] in the crate that owns the document's
/// shape, for the reason that module's header gives: a document written by one
/// tool and edited by another must have exactly one definition, or the editor
/// deletes whatever it does not model. It is the *reading* half that was
/// missing — a write-only manifest is a document no admission step can correct,
/// and the steps handed one answered about a single tile instead.
///
/// It stays a type of its own rather than a variant of `PrefabMeta` for the
/// reason its `structure_set` doc gives: `PrefabMeta` *requires* `structure`, so
/// a tool that has not learned about tile sets fails to parse this document
/// instead of reading it as a prefab with no blocks.
pub use delvewright_schem::prefab::TileSetMeta as TileSetMetadata;

// ---------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------

/// One exported prefab: the two files, named and rendered, but not yet written.
///
/// Keeping the bytes in hand rather than writing them straight out is what lets
/// the determinism test compare two exports without touching a filesystem.
#[derive(Debug, Clone)]
pub struct PrefabExport {
    /// The DSL prefab id, `prefab/<id>`.
    pub prefab_id: String,
    /// The structure filename, `<id>.nbt`.
    pub structure_file: String,
    /// The metadata filename, `<id>.json`.
    pub metadata_file: String,
    /// The gzip-framed structure template.
    pub nbt: Vec<u8>,
    /// The metadata, as the pretty JSON text that goes on disk (with its
    /// trailing newline).
    pub metadata_json: String,
    /// The typed metadata, for callers that would otherwise re-parse it.
    pub metadata: PrefabMetadata,
    /// The expansion the `.nbt` is a snapshot of.
    pub expansion: Expansion,
}

impl PrefabExport {
    /// Write both files into `dir`, which must exist.
    pub fn write_to_dir(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::write(dir.join(&self.structure_file), &self.nbt)?;
        std::fs::write(dir.join(&self.metadata_file), &self.metadata_json)
    }
}

/// One tile's file: the name it lands under and the bytes that go in it.
#[derive(Debug, Clone)]
pub struct TileFile {
    /// The `.nbt` filename.
    pub file: String,
    /// The gzip-framed structure template.
    pub nbt: Vec<u8>,
}

/// A zone exported as several structure templates plus one manifest.
#[derive(Debug, Clone)]
pub struct TileSetExport {
    /// The DSL prefab id, `prefab/<id>`.
    pub prefab_id: String,
    /// The manifest filename, `<id>.json` — the same name a single-template
    /// prefab's metadata has, because it is the same thing to open.
    pub metadata_file: String,
    /// The manifest, as the pretty JSON text that goes on disk.
    pub metadata_json: String,
    /// The typed manifest, for callers that would otherwise re-parse it.
    pub metadata: TileSetMetadata,
    /// The tiles, in `x`→`y`→`z` grid order and index-aligned with
    /// `metadata.structure_set.parts`.
    pub tiles: Vec<TileFile>,
    /// The expansion the tiles are a snapshot of — the **whole** zone, which is
    /// what every gate and every measurement is about.
    pub expansion: Expansion,
}

impl TileSetExport {
    /// Write the manifest and every tile into `dir`, which must exist.
    pub fn write_to_dir(&self, dir: &Path) -> std::io::Result<()> {
        for tile in &self.tiles {
            std::fs::write(dir.join(&tile.file), &tile.nbt)?;
        }
        std::fs::write(dir.join(&self.metadata_file), &self.metadata_json)
    }
}

/// A frozen zone: one structure template, or a tile set plus its manifest.
///
/// Which of the two is a fact about the region, decided by [`export_zone`] and
/// by nothing an author writes. Callers that do not care — and most should not —
/// use the accessors, which read the same for both.
#[derive(Debug, Clone)]
pub enum ZoneExport {
    /// The zone fit one structure template.
    Single(PrefabExport),
    /// The zone needed tiling.
    Tiled(TileSetExport),
}

impl ZoneExport {
    /// Write every file into `dir`, which must exist.
    pub fn write_to_dir(&self, dir: &Path) -> std::io::Result<()> {
        match self {
            ZoneExport::Single(e) => e.write_to_dir(dir),
            ZoneExport::Tiled(e) => e.write_to_dir(dir),
        }
    }

    /// The DSL prefab id, `prefab/<id>`.
    pub fn prefab_id(&self) -> &str {
        match self {
            ZoneExport::Single(e) => &e.prefab_id,
            ZoneExport::Tiled(e) => &e.prefab_id,
        }
    }

    /// The one file a consumer opens to learn what this zone is.
    pub fn metadata_file(&self) -> &str {
        match self {
            ZoneExport::Single(e) => &e.metadata_file,
            ZoneExport::Tiled(e) => &e.metadata_file,
        }
    }

    /// Its text, as written.
    pub fn metadata_json(&self) -> &str {
        match self {
            ZoneExport::Single(e) => &e.metadata_json,
            ZoneExport::Tiled(e) => &e.metadata_json,
        }
    }

    /// Every structure file, in grid order.
    pub fn structure_files(&self) -> Vec<&str> {
        match self {
            ZoneExport::Single(e) => vec![e.structure_file.as_str()],
            ZoneExport::Tiled(e) => e.tiles.iter().map(|t| t.file.as_str()).collect(),
        }
    }

    /// The tile grid; `[1, 1, 1]` when the zone fit one template.
    pub fn grid(&self) -> [i32; 3] {
        match self {
            ZoneExport::Single(_) => [1, 1, 1],
            ZoneExport::Tiled(e) => e.metadata.structure_set.grid,
        }
    }

    /// The whole expansion — the semantic unit, whatever the packaging.
    pub fn expansion(&self) -> &Expansion {
        match self {
            ZoneExport::Single(e) => &e.expansion,
            ZoneExport::Tiled(e) => &e.expansion,
        }
    }
}

/// Why an expansion could not be frozen into a prefab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportError {
    /// The program did not expand.
    Expand(ExpandError),
    /// The requested id is not usable as a structure path segment.
    BadId {
        /// The id as given.
        id: String,
    },
    /// The region is degenerate; there is no structure to write.
    EmptyRegion {
        /// The region's extents.
        size: [u32; 3],
    },
    /// The region exceeds the vanilla structure cap on some axis.
    TooLarge {
        /// The region's extents.
        size: [u32; 3],
        /// The per-axis cap.
        cap: u32,
    },
    /// The model contains blocks a structure template may not carry.
    ForbiddenBlocks {
        /// One line per offending cell, as the strip reported it.
        reasons: Vec<String>,
    },
    /// The model contains block states Minecraft 1.21.11 does not have.
    UnknownBlocks {
        /// One line per offending block state, with the cells it covers.
        reasons: Vec<String>,
    },
    /// The expansion's blocks disagree with the spatial contract it declares.
    ///
    /// Refused **here**, at the writer, and not only in `gates::judge`. The
    /// event this guards is "a `.nbt` whose metadata claims something untrue of
    /// its own bytes exists on disk", and freezing is that event; a check that
    /// lived only in the CLI's judging step would be skipped by every other
    /// caller of this function, which is the shape of a gate that protects
    /// nothing (CLAUDE.md).
    Contract {
        /// One line per failed obligation, with its binding count.
        gates: Vec<String>,
    },
    /// The model contains block states omitting shape-carrying (multipart)
    /// properties (`DW0735`, `delvewright_schem::blocks::DW_SHAPE_OMITTED`).
    ShapeOmissions {
        /// One line per offending block state, with the cells it covers.
        reasons: Vec<String>,
    },
    /// The expansion filled orientation-sensitive block states into scopes
    /// whose frame turns or reflects them, with no `orientation` guard
    /// (`DW0736`, `delvewright_schem::blocks::DW_ORIENTED_FILL_UNGUARDED`).
    UnguardedOrientedFills {
        /// One line per finding.
        reasons: Vec<String>,
    },
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExportError::Expand(e) => write!(f, "{e}"),
            ExportError::BadId { id } => write!(
                f,
                "{id:?} is not a usable structure id: use lowercase letters, digits and \
                 hyphens (it becomes a datapack path segment and a filename)"
            ),
            ExportError::EmptyRegion { size } => write!(
                f,
                "the region is {}x{}x{} — a prefab needs at least one cell on every axis",
                size[0], size[1], size[2]
            ),
            ExportError::TooLarge { size, cap } => write!(
                f,
                "the region is {}x{}x{} but one structure template caps every axis at {cap} — \
                 export it with `export_zone`, which tiles. Reaching this from anywhere but the \
                 single-template writer is a bug in the toolchain, never something for an author \
                 to work around",
                size[0], size[1], size[2]
            ),
            ExportError::ForbiddenBlocks { reasons } => write!(
                f,
                "the expanded model contains blocks a structure template may not carry, and \
                 exporting would silently replace them with air: {}",
                reasons.join("; ")
            ),
            ExportError::UnknownBlocks { reasons } => write!(
                f,
                "the expanded model paints block states Minecraft {} does not have: {}",
                delvewright_schem::blocks::MC_VERSION,
                reasons.join("; ")
            ),
            ExportError::Contract { gates } => write!(
                f,
                "the expanded model disagrees with the spatial contract this program declares, so \
                 freezing it would put a prefab on disk whose metadata describes a building it is \
                 not true of: {}. Change the blocks until the contract is true of them, or change \
                 the declared contract to what this building actually is",
                gates.join("; ")
            ),
            ExportError::ShapeOmissions { reasons } => write!(
                f,
                "{}: the expanded model paints block states that omit shape-carrying \
                 (multipart) properties, which place disconnected — a wall with no \
                 connection state written is an isolated post: {}",
                delvewright_schem::blocks::DW_SHAPE_OMITTED,
                reasons.join("; ")
            ),
            ExportError::UnguardedOrientedFills { reasons } => write!(
                f,
                "{}: the expansion filled orientation-sensitive block states into \
                 turned or reflected scopes with no `orientation` guard, so their \
                 literal facing/axis/connections land however the scope was framed: {}. \
                 Write one alternative per frame, each guarded with the `orientation` \
                 cond and carrying the matching state",
                delvewright_schem::blocks::DW_ORIENTED_FILL_UNGUARDED,
                reasons.join("; ")
            ),
        }
    }
}

impl std::error::Error for ExportError {}

impl From<ExpandError> for ExportError {
    fn from(e: ExpandError) -> Self {
        ExportError::Expand(e)
    }
}

/// True for an id usable both as a datapack path segment and as a filename.
///
/// Public because the id is chosen — from `--id`, a library program id or an
/// input filename — long before [`export_prefab`] is reached, and a caller that
/// cannot ask the question early has no choice but to do the whole expansion
/// first and refuse afterwards, on top of a gate report that already said
/// `pass`.
pub fn is_valid_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !id.starts_with('-')
        && !id.ends_with('-')
}

/// Expand `program` over `region` and freeze the result as **one** structure
/// template plus its metadata.
///
/// This is the single-template writer, and it refuses a region past
/// [`MAX_STRUCTURE_AXIS`]. Callers outside this module want [`export_zone`],
/// which is the same thing for a region that fits and a tile set for one that
/// does not — an author's region is never the wrong size.
///
/// The returned bytes are a pure function of `(program, region.size, options)`:
/// re-running this call reproduces both files byte for byte (ADR-0006), and the
/// metadata's provenance row names exactly the inputs that do so. The region's
/// *origin* is not part of that row because it cannot be: a structure template
/// is local-coordinate, so moving the box moves nothing in the output.
pub fn export_prefab(
    program: &Program,
    region: Box3,
    options: &ExpandOptions,
    id: &str,
) -> Result<PrefabExport, ExportError> {
    if !is_valid_id(id) {
        return Err(ExportError::BadId { id: id.to_string() });
    }
    if region.is_empty() {
        return Err(ExportError::EmptyRegion { size: region.size });
    }
    if region.size.iter().any(|&s| s > MAX_STRUCTURE_AXIS) {
        return Err(ExportError::TooLarge {
            size: region.size,
            cap: MAX_STRUCTURE_AXIS,
        });
    }

    let expansion = expand(program, region, options)?;
    let palette = zone_palette(&expansion.model);
    refuse_unknown_states(&expansion.model, &palette)?;
    // The block-spelling family first, the contract second, and the order is
    // load-bearing: a state that omits its connections or lands the wrong way
    // round changes what the bytes MEAN, so the contract check would otherwise
    // read a building whose walls are isolated posts and answer about that one.
    refuse_unguarded_oriented_fills(&expansion)?;
    refuse_broken_contract(&expansion)?;
    let nbt = part_nbt(&expansion.model, &palette, region)?;

    let size = [
        region.size[0] as i32,
        region.size[1] as i32,
        region.size[2] as i32,
    ];
    let hash = program_hash(program);
    let metadata = PrefabMetadata {
        prefab_id: format!("prefab/{id}"),
        structure: StructureMetadata {
            file: format!("{id}.nbt"),
            id: id.to_string(),
            size,
            data_version: DATA_VERSION,
            generator: Some(GENERATOR.to_string()),
        },
        anchors: anchor_metadata(&expansion),
        // The export emits no jigsaw connectors (see the module header), and
        // says so with an empty list rather than by omitting the key: a piece
        // with no sockets and a piece whose metadata predates sockets are not
        // the same claim.
        connectors: Vec::new(),
        lighting: Some(LightingMetadata::unmeasured()),
        license: Some(license_metadata(program, &hash, options.seed, size, None)),
        waterline_y: None,
        spatial_contract: contract_metadata(&expansion),
        extra: BTreeMap::new(),
    };
    let metadata_json = metadata.to_json();

    Ok(PrefabExport {
        prefab_id: metadata.prefab_id.clone(),
        structure_file: metadata.structure.file.clone(),
        metadata_file: format!("{id}.json"),
        nbt,
        metadata_json,
        metadata,
        expansion,
    })
}

/// Expand `program` over `region` and freeze it, tiling if it does not fit.
///
/// **This is the export.** [`export_prefab`] is the writer for one structure
/// template; this is the one that knows a template is not the unit a creator
/// designs in. A region within [`MAX_STRUCTURE_AXIS`] on every axis produces
/// exactly what `export_prefab` produces, byte for byte, under the same two
/// filenames — tiling adds nothing to the shape of the ordinary case. A region
/// past it produces a tile set and a manifest, and still never a refusal.
///
/// Where the cuts fall is [`plan_split`]'s answer, a pure function of the region
/// and the cap: no RNG, no clock, no dependence on the program, the seed, or the
/// blocks (ADR-0006).
pub fn export_zone(
    program: &Program,
    region: Box3,
    options: &ExpandOptions,
    id: &str,
) -> Result<ZoneExport, ExportError> {
    if !is_valid_id(id) {
        return Err(ExportError::BadId { id: id.to_string() });
    }
    if region.is_empty() {
        return Err(ExportError::EmptyRegion { size: region.size });
    }

    let size = [
        region.size[0] as i32,
        region.size[1] as i32,
        region.size[2] as i32,
    ];
    let part_max = MAX_STRUCTURE_AXIS as i32;
    let plan = plan_split(size, part_max);
    if plan.is_single() {
        return export_prefab(program, region, options, id).map(ZoneExport::Single);
    }

    // One expansion for the whole zone. The tiles are cut out of it afterwards,
    // so the blocks a tile holds cannot depend on the tiling.
    let expansion = expand(program, region, options)?;
    let palette = zone_palette(&expansion.model);
    refuse_unknown_states(&expansion.model, &palette)?;
    // The block-spelling family first, the contract second, and the order is
    // load-bearing: a state that omits its connections or lands the wrong way
    // round changes what the bytes MEAN, so the contract check would otherwise
    // read a building whose walls are isolated posts and answer about that one.
    refuse_unguarded_oriented_fills(&expansion)?;
    refuse_broken_contract(&expansion)?;

    let mut tiles = Vec::with_capacity(plan.parts.len());
    let mut parts = Vec::with_capacity(plan.parts.len());
    for part in &plan.parts {
        let part_box = Box3::new(
            [
                region.origin[0] + part.offset[0],
                region.origin[1] + part.offset[1],
                region.origin[2] + part.offset[2],
            ],
            [
                part.size[0] as u32,
                part.size[1] as u32,
                part.size[2] as u32,
            ],
        );
        let nbt = part_nbt(&expansion.model, &palette, part_box)?;
        let file = part_filename(id, part.grid_index);
        parts.push(TilePart {
            id: file
                .strip_suffix(".nbt")
                .expect("part_filename ends in .nbt")
                .to_string(),
            file: file.clone(),
            grid_index: part.grid_index,
            offset: part.offset,
            size: part.size,
        });
        tiles.push(TileFile { file, nbt });
    }

    let hash = program_hash(program);
    let metadata = TileSetMetadata {
        prefab_id: format!("prefab/{id}"),
        structure_set: TileSet {
            base: id.to_string(),
            size,
            part_max,
            grid: plan.grid,
            data_version: DATA_VERSION,
            generator: GENERATOR.to_string(),
            parts,
        },
        anchors: anchor_metadata(&expansion),
        // Same claim, same key, same reason as the single-template export: an
        // empty list says "no sockets", an absent key says nothing at all.
        connectors: Vec::new(),
        lighting: Some(LightingMetadata::unmeasured()),
        license: Some(license_metadata(
            program,
            &hash,
            options.seed,
            size,
            Some((plan.grid, tiles.len())),
        )),
        spatial_contract: contract_metadata(&expansion),
        // A freshly exported manifest models every key it writes; the map is
        // what a LATER engine's key survives in on the way back out.
        extra: BTreeMap::new(),
    };
    let metadata_json =
        serde_json::to_string_pretty(&metadata).expect("tile-set manifest serialises") + "\n";

    Ok(ZoneExport::Tiled(TileSetExport {
        prefab_id: metadata.prefab_id.clone(),
        metadata_file: format!("{id}.json"),
        metadata_json,
        metadata,
        tiles,
        expansion,
    }))
}

/// The anchors an expansion declared, in the metadata shape. Zone-relative in
/// both export shapes, because a mark is a fact about the building and a tile
/// boundary is not part of the building.
/// Each anchor also carries **which contract element it lands in**
/// (spec-0036 §1b/§2.7): a campaign binds content to an anchor, and the thing
/// that says whether that place is play space, a door or dressing is the
/// contract. Resolved here, from the resolved contract alone, so the metadata a
/// reader gets and the element the checker's anchor obligation reads are the
/// same string.
fn anchor_metadata(expansion: &Expansion) -> BTreeMap<String, AnchorMetadata> {
    let contract = contract_metadata(expansion);
    expansion
        .anchors
        .iter()
        .map(|(name, anchor)| {
            let mut meta = AnchorMetadata::point(anchor.pos, anchor.facing.to_string());
            meta.resolves_to = contract
                .as_ref()
                .and_then(|c| crate::contract::resolves_to(c, anchor.pos));
            (name.clone(), meta)
        })
        .collect()
}

/// The spatial contract an expansion resolved, in the metadata shape.
///
/// Zone-relative in both export shapes, for the reason the anchors are: a
/// declaration is a fact about the building, and a tile boundary is not part of
/// the building.
///
/// Nothing here is inferred and nothing is checked. The declarations went in as
/// the program's intent and come out as the boxes that intent resolved to; a
/// space with no boxes is written with none, because a zero binding is a finding
/// for whatever reads the contract and deleting it would hide the finding.
pub fn contract_metadata(expansion: &Expansion) -> Option<SpatialContract> {
    let mut out = contract_without_faces(expansion)?;
    // The face contract is derived once, here, and written down — so assembly
    // asks the metadata rather than reopening the `.nbt`, and so the faces a
    // reviewer reads are the faces the checker judged.
    out.faces = crate::contract::exterior_faces(&expansion.model, &out)
        .into_iter()
        .map(|f| ContractFace {
            space: f.space,
            class: f.class,
            dir: f.dir.as_str().to_string(),
            opening: RegionMetadata {
                from: [
                    f.cells.iter().map(|c| c[0]).min().unwrap_or(0),
                    f.cells.iter().map(|c| c[1]).min().unwrap_or(0),
                    f.cells.iter().map(|c| c[2]).min().unwrap_or(0),
                ],
                to: [
                    f.cells.iter().map(|c| c[0]).max().unwrap_or(0),
                    f.cells.iter().map(|c| c[1]).max().unwrap_or(0),
                    f.cells.iter().map(|c| c[2]).max().unwrap_or(0),
                ],
            },
        })
        .collect();
    Some(out)
}

/// The resolved contract without its derived face contract — what
/// [`crate::contract::exterior_faces`] reads, so the derivation cannot depend on
/// its own output.
fn contract_without_faces(expansion: &Expansion) -> Option<SpatialContract> {
    let contract = expansion.contract.as_ref()?;
    let ranges = |boxes: &[Box3]| -> Vec<RegionMetadata> { boxes.iter().map(range).collect() };
    Some(SpatialContract {
        entry: contract.entry.clone(),
        spaces: contract
            .spaces
            .iter()
            .map(|(name, space)| {
                (
                    name.clone(),
                    ContractSpace {
                        envelope: space.envelope.as_str().to_string(),
                        boxes: ranges(&space.region.boxes),
                    },
                )
            })
            .collect(),
        no_body: contract
            .no_body
            .iter()
            .map(|(name, region)| {
                (
                    name.clone(),
                    ContractNoBody {
                        reason: region.reason.clone(),
                        boxes: ranges(&region.region.boxes),
                    },
                )
            })
            .collect(),
        edges: contract
            .edges
            .iter()
            .map(|edge| ContractEdge {
                a: edge.a.clone(),
                b: edge.b.clone(),
                class: edge.class.to_string(),
                rise: edge.rise,
                via: edge.via.as_ref().map(|v| ContractVolume {
                    region: v.region.clone(),
                    boxes: ranges(&v.boxes),
                }),
                bar: edge.bar.as_ref().map(|b| ContractBar {
                    region: b.region.clone(),
                    boxes: ranges(&b.boxes),
                    block: b.block.to_string(),
                }),
            })
            .collect(),
        faces: Vec::new(),
        no_body_majority_ack: contract.no_body_majority_ack.clone(),
    })
}

/// A half-open box as the metadata's inclusive `from`/`to` range.
///
/// The document has exactly one way to name a range of cells — the one a gate
/// anchor already uses — so a contract box is that same type rather than a
/// second spelling of it.
fn range(b: &Box3) -> RegionMetadata {
    RegionMetadata {
        from: b.origin,
        to: [
            b.origin[0] + b.size[0] as i32 - 1,
            b.origin[1] + b.size[1] as i32 - 1,
            b.origin[2] + b.size[2] as i32 - 1,
        ],
    }
}

/// The `license` block, shared by both export shapes so the provenance sentence
/// cannot drift between them. `tiling` is `Some((grid, count))` for a tile set.
fn license_metadata(
    program: &Program,
    hash: &str,
    seed: u64,
    size: [i32; 3],
    tiling: Option<([i32; 3], usize)>,
) -> LicenseMetadata {
    let packaging = match tiling {
        None => String::new(),
        Some((grid, count)) => format!(
            " The zone exceeds the {MAX_STRUCTURE_AXIS}-per-axis structure-template cap, so it is \
             packaged as {count} tile(s) in a {}x{}x{} grid; the tiling is a pure function of the \
             region and changes nothing about the expansion.",
            grid[0], grid[1], grid[2]
        ),
    };
    LicenseMetadata {
        source: "original".to_string(),
        spdx: "GPL-3.0-or-later".to_string(),
        note: "Original Delvewright project asset, derived from a grammar program in this \
               repository. No third-party material is ingested at expansion time; the \
               generator itself is a port credited in docs/ACKNOWLEDGEMENTS.md."
            .to_string(),
        provenance: format!(
            "Generated deterministically by {GENERATOR} (spec-0027) from grammar program \
             {:?} ({hash}) at seed {seed} over a {}x{}x{} region; ADR-0006: those four inputs \
             regenerate this NBT byte for byte.{packaging}",
            program.name, size[0], size[1], size[2]
        ),
        // Optional in the document — an ingested piece has nothing that
        // regenerates it — and never optional here: an expansion always does.
        generated_by: Some(GeneratedBy {
            generator: "grammar".to_string(),
            program: program.name.clone(),
            program_hash: hash.to_string(),
            seed,
        }),
    }
}

/// The palette every tile of a zone shares.
///
/// One palette for the whole model, not one per tile: the ordering is then the
/// same in every file of a set, so a reviewer diffing two tiles sees blocks
/// move and never indices renumber. Unused entries in a tile's palette are
/// legal and cost a few bytes.
struct ZonePalette {
    states: Vec<SchemBlockState>,
    index_of: BTreeMap<String, i32>,
}

fn zone_palette(model: &VoxelModel) -> ZonePalette {
    // Palette index 0 is air in both representations, and both lay cells out
    // x-major, so the model's own cell order is already the schematic's.
    let states: Vec<SchemBlockState> = model
        .palette()
        .iter()
        .map(|b| SchemBlockState {
            name: b.name.clone(),
            properties: b.properties.clone(),
        })
        .collect();
    let index_of: BTreeMap<String, i32> = states
        .iter()
        .enumerate()
        .map(|(i, b)| (b.to_state_string(), i as i32))
        .collect();
    ZonePalette { states, index_of }
}

/// Refuse block states Minecraft 1.21.11 does not have.
///
/// Spelling, checked by the emitter (CLAUDE.md: the operator running
/// the tool does not run `cargo test`). A structure template loads an unknown
/// block as AIR, so this is the one class of defect that costs the whole piece
/// and reports nothing at all.
///
/// It runs over the **whole model**, once, before any tiling: the cell counts it
/// reports are the zone's, so a set's refusal reads identically to a single
/// prefab's and never depends on which tile a bad block landed in.
/// Refuse to freeze an expansion whose blocks disagree with the contract it
/// declares.
///
/// Every writer goes through here, so the guarded event — a prefab on disk whose
/// metadata describes a building it is not true of — cannot happen without the
/// obligations having run. The CLI judges first and prints a report; a library
/// caller that never judges is refused here instead of shipping the artifact.
fn refuse_broken_contract(expansion: &Expansion) -> Result<(), ExportError> {
    let Some(contract) = contract_metadata(expansion) else {
        return Ok(());
    };
    let anchors: std::collections::BTreeMap<String, [i32; 3]> = expansion
        .anchors
        .iter()
        .map(|(name, a)| (name.clone(), a.pos))
        .collect();
    let verdict = crate::contract::check(&expansion.model, &contract, &anchors);
    let failed: Vec<String> = verdict
        .gates
        .iter()
        .filter(|g| !g.passed())
        .map(|g| format!("{} (examined {}): {}", g.id, g.bound, g.detail))
        .collect();
    if failed.is_empty() {
        Ok(())
    } else {
        Err(ExportError::Contract { gates: failed })
    }
}

fn refuse_unknown_states(model: &VoxelModel, palette: &ZonePalette) -> Result<(), ExportError> {
    let mut cells_per_state = vec![0usize; palette.states.len()];
    for pos in model.region().positions() {
        let state = model.get(pos).expect("positions() stays inside the region");
        cells_per_state[palette.index_of[&state.to_string()] as usize] += 1;
    }

    let registry = delvewright_schem::blocks::BlockRegistry::v1_21_11();
    let unknown: Vec<String> = palette
        .states
        .iter()
        .zip(&cells_per_state)
        .filter(|&(_, &cells)| cells > 0)
        .filter_map(|(state, &cells)| {
            registry
                .validate(&state.name, &state.properties)
                .err()
                .map(|e| format!("{e} ({cells} cell(s))"))
        })
        .collect();
    if !unknown.is_empty() {
        return Err(ExportError::UnknownBlocks { reasons: unknown });
    }

    // The shape half of the same spelling rule (`DW0735`): a state that omits
    // a multipart property compiles, loads, and places disconnected. Checked
    // by the emitter for the same reason the id check is.
    let omissions: Vec<String> = palette
        .states
        .iter()
        .zip(&cells_per_state)
        .filter(|&(_, &cells)| cells > 0)
        .filter_map(|(state, &cells)| {
            let omitted = registry.omitted_shape_carrying(&state.name, &state.properties);
            if omitted.is_empty() {
                None
            } else {
                Some(format!(
                    "{} omits {} ({cells} cell(s))",
                    state.to_state_string(),
                    omitted.join(", ")
                ))
            }
        })
        .collect();
    if omissions.is_empty() {
        Ok(())
    } else {
        Err(ExportError::ShapeOmissions { reasons: omissions })
    }
}

/// Refuse an expansion that filled orientation-sensitive states unguarded
/// (`DW0736`). The findings were collected during expansion, where the scope
/// orientations exist; freezing them into a `.nbt` is what would make the
/// wrong facing permanent, so the export is where the refusal bites for
/// callers that skip the gate report.
fn refuse_unguarded_oriented_fills(expansion: &Expansion) -> Result<(), ExportError> {
    if expansion.oriented.unguarded.is_empty() {
        return Ok(());
    }
    Err(ExportError::UnguardedOrientedFills {
        reasons: expansion
            .oriented
            .unguarded
            .iter()
            .map(|f| {
                format!(
                    "rule {:?} fills {} whose {} lands wrong under {}",
                    f.rule, f.state, f.property, f.orientation
                )
            })
            .collect(),
    })
}

/// Render one box of a model as a gzip-framed vanilla structure template.
///
/// `part` is in model coordinates and must lie inside `model.region()`; for an
/// untiled export it *is* the whole region, which is why the ordinary case
/// cannot drift from what it emitted before tiling existed.
///
/// Goes through the `.schem` pipeline's emitter rather than a second one of our
/// own: one structure writer means one set of determinism guarantees and one
/// place where the 1.21.11 shape is defined.
fn part_nbt(model: &VoxelModel, palette: &ZonePalette, part: Box3) -> Result<Vec<u8>, ExportError> {
    let size = [
        part.size[0] as i32,
        part.size[1] as i32,
        part.size[2] as i32,
    ];

    let mut blocks = vec![0i32; (size[0] * size[1] * size[2]) as usize];
    for (i, pos) in part.positions().enumerate() {
        let state = model.get(pos).expect("a part stays inside the model");
        blocks[i] = palette.index_of[&state.to_string()];
    }

    let schem = ParsedSchematic {
        version: 3,
        // The model is authored *for* the pinned version, never migrated into it.
        source_data_version: Some(DATA_VERSION),
        size,
        offset: [0, 0, 0],
        palette: palette.states.clone(),
        blocks,
        block_entities: Vec::new(),
    };

    let mut diagnostics = Vec::new();
    let nbt = convert::build_region(&schem, [0, 0, 0], size, &mut diagnostics);
    if !diagnostics.is_empty() {
        // The emitter's safety strip would replace these with air. A grammar
        // that asked for a command block meant to ask for one, so silently
        // shipping a hole is the worst of the three options; refuse instead.
        //
        // Positions are rebased to ZONE coordinates. A cell named at "3,2,1 of
        // tile x0y0z1" is a cell an author cannot find in their own design.
        let base = [
            part.origin[0] - model.region().origin[0],
            part.origin[1] - model.region().origin[1],
            part.origin[2] - model.region().origin[2],
        ];
        return Err(ExportError::ForbiddenBlocks {
            reasons: diagnostics
                .iter()
                .map(|d| match d.pos {
                    Some(p) => format!(
                        "{} at {},{},{}",
                        d.message,
                        p[0] + base[0],
                        p[1] + base[1],
                        p[2] + base[2]
                    ),
                    None => d.message.clone(),
                })
                .collect(),
        });
    }
    Ok(nbt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::BlockState;
    use crate::ir::{Material, Node};

    fn one_block_program(name: &str, block: &str) -> Program {
        Program::new(name, "all").rule(
            "all",
            Node::Fill {
                material: Material::block(BlockState::simple(block)),
            },
        )
    }

    #[test]
    fn the_program_hash_follows_content_not_construction() {
        let built = one_block_program("slab", "stone");
        let json = serde_json::to_string(&built).unwrap();
        let parsed: Program = serde_json::from_str(&json).unwrap();
        assert_eq!(program_hash(&built), program_hash(&parsed));
        assert_ne!(
            program_hash(&built),
            program_hash(&one_block_program("slab", "andesite")),
            "a different program must hash differently"
        );
        let hash = program_hash(&built);
        assert!(
            hash.starts_with("sha256:") && hash.len() == 7 + 64,
            "{hash}"
        );
    }

    #[test]
    fn ids_that_are_not_path_segments_are_refused() {
        let program = one_block_program("slab", "stone");
        let region = Box3::at_origin([2, 2, 2]);
        for id in [
            "",
            "Temple",
            "temple/boss",
            "../etc",
            "temple_hall",
            "-x",
            "x-",
        ] {
            assert_eq!(
                export_prefab(&program, region, &ExpandOptions::seeded(0), id).unwrap_err(),
                ExportError::BadId { id: id.to_string() },
                "{id:?} should not be usable as a structure id"
            );
        }
        assert!(export_prefab(&program, region, &ExpandOptions::seeded(0), "temple-hall").is_ok());
    }

    #[test]
    fn regions_a_structure_template_cannot_hold_are_refused() {
        let program = one_block_program("slab", "stone");
        let opts = ExpandOptions::seeded(0);
        assert_eq!(
            export_prefab(&program, Box3::at_origin([4, 0, 4]), &opts, "p").unwrap_err(),
            ExportError::EmptyRegion { size: [4, 0, 4] }
        );
        assert_eq!(
            export_prefab(&program, Box3::at_origin([49, 4, 4]), &opts, "p").unwrap_err(),
            ExportError::TooLarge {
                size: [49, 4, 4],
                cap: MAX_STRUCTURE_AXIS
            }
        );
        assert!(export_prefab(&program, Box3::at_origin([48, 48, 48]), &opts, "p").is_ok());
    }

    /// The `minecraft:chain` finding, at the grammar's own emitter: 1.21.11
    /// renamed the block, and a structure template loads an unknown id as air —
    /// so a program that asks for one would export clean and ship a hole.
    #[test]
    fn a_block_1_21_11_does_not_have_is_refused_with_the_rename_named() {
        let program = one_block_program("ropes", "minecraft:chain");
        let err = export_prefab(
            &program,
            Box3::at_origin([2, 2, 2]),
            &ExpandOptions::seeded(0),
            "ropes",
        )
        .unwrap_err();
        let ExportError::UnknownBlocks { reasons } = &err else {
            panic!("expected an unknown-block refusal, got {err}");
        };
        assert_eq!(reasons.len(), 1, "one line per state: {reasons:?}");
        assert!(reasons[0].contains("8 cell(s)"), "{reasons:?}");
        assert!(err.to_string().contains("minecraft:iron_chain"), "{err}");

        // ...and the rename itself exports.
        assert!(
            export_prefab(
                &one_block_program("ropes", "minecraft:iron_chain"),
                Box3::at_origin([2, 2, 2]),
                &ExpandOptions::seeded(0),
                "ropes",
            )
            .is_ok()
        );
    }

    /// A property value that does not exist is the same class of defect and is
    /// caught by the same call — vanilla would drop the whole state, not just
    /// the property.
    #[test]
    fn an_impossible_property_value_is_refused_too() {
        let program = one_block_program("steps", "minecraft:oak_stairs[facing=up]");
        let err = export_prefab(
            &program,
            Box3::at_origin([2, 2, 2]),
            &ExpandOptions::seeded(0),
            "steps",
        )
        .unwrap_err();
        assert!(matches!(err, ExportError::UnknownBlocks { .. }), "{err}");
    }

    #[test]
    fn a_block_the_structure_strip_would_eat_is_refused_not_silently_dropped() {
        let program = one_block_program("armed", "minecraft:command_block");
        let err = export_prefab(
            &program,
            Box3::at_origin([2, 2, 2]),
            &ExpandOptions::seeded(0),
            "armed",
        )
        .unwrap_err();
        let ExportError::ForbiddenBlocks { reasons } = &err else {
            panic!("expected a forbidden-block refusal, got {err}");
        };
        assert_eq!(reasons.len(), 8, "one per cell: {reasons:?}");
        assert!(err.to_string().contains("command_block"), "{err}");
    }
}
