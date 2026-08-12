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
//! # What this module deliberately does not emit
//!
//! * **Connectors.** Jigsaw socketing of grammar prefabs needs the tileset
//!   conventions to be settled first; a guessed socket is worse than none.
//! * **A lighting measurement.** See [`LIGHTING_PROFILE`].

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use serde::Serialize;
use sha2::{Digest, Sha256};

use delvewright_schem::convert::{self, DATA_VERSION};
use delvewright_schem::schematic::{BlockState as SchemBlockState, ParsedSchematic};

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
pub const LIGHTING_PROFILE: &str = "unmeasured";

/// Vanilla caps a structure template at 48 blocks per axis. A grammar prefab is
/// one template with one metadata file, so an oversize region is refused rather
/// than tiled — reassembling tiles is a jigsaw design, not an export detail.
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

/// A grammar prefab's sibling metadata file.
///
/// Field order is the emission order and matches the hand-built prefabs
/// (`prefab_id`, `structure`, `anchors`, `lighting`, `license`) so a reviewer
/// diffing a grammar piece against a hand one sees only the values change.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PrefabMetadata {
    /// The DSL prefab id, `prefab/<id>`.
    pub prefab_id: String,
    /// The structure-template reference.
    pub structure: StructureMetadata,
    /// Named anchors, exactly the ones the program's `mark` declarations
    /// produced. Empty for a program that marks nothing.
    pub anchors: BTreeMap<String, AnchorMetadata>,
    /// The lighting declaration.
    pub lighting: LightingMetadata,
    /// Licence and provenance.
    pub license: LicenseMetadata,
}

/// The `structure` block: which file, how big, for which MC version.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StructureMetadata {
    /// The `.nbt` filename, relative to this metadata file.
    pub file: String,
    /// The datapack structure id (a path segment).
    pub id: String,
    /// Structure extent `[x, y, z]`.
    pub size: [i32; 3],
    /// The MC data version the structure targets (ADR-0009).
    pub data_version: i32,
    /// Provenance breadcrumb: what wrote the `.nbt`.
    pub generator: String,
}

/// One entry of the `anchors` map: the point-anchor shape the hand-built
/// prefabs use, field for field (`pos` then `facing`), so the engine's
/// `PrefabRegistry` reads a grammar prefab's anchors with the same code path.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AnchorMetadata {
    /// Local cell `[x, y, z]`, relative to the structure origin.
    pub pos: [i32; 3],
    /// Cardinal facing keyword.
    pub facing: String,
}

/// The `lighting` block. Carries a profile and, deliberately, no measurement.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LightingMetadata {
    /// Always [`LIGHTING_PROFILE`].
    pub profile: String,
}

/// The `license` block, plus the machine-readable provenance row of spec-0027 §2.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LicenseMetadata {
    /// Where the asset came from (`original`).
    pub source: String,
    /// SPDX id.
    pub spdx: String,
    /// Human note (ADR-0013).
    pub note: String,
    /// Human-readable provenance sentence.
    pub provenance: String,
    /// The machine-readable provenance row: what regenerates these exact bytes.
    pub generated_by: GeneratedBy,
}

/// Everything needed to reproduce the `.nbt` byte for byte (ADR-0006).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GeneratedBy {
    /// The back end (`grammar`).
    pub generator: String,
    /// The grammar program's name.
    pub program: String,
    /// `sha256:` over the program's canonical JSON ([`program_hash`]).
    pub program_hash: String,
    /// The expansion seed.
    pub seed: u64,
}

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
                "the region is {}x{}x{} but a vanilla structure template caps every axis at \
                 {cap}; a piece this big has to be authored as several jigsaw-socketed prefabs",
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
fn is_valid_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !id.starts_with('-')
        && !id.ends_with('-')
}

/// Expand `program` over `region` and freeze the result as prefab `id`.
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
    let nbt = structure_nbt(&expansion.model)?;

    let size = [
        region.size[0] as i32,
        region.size[1] as i32,
        region.size[2] as i32,
    ];
    let hash = program_hash(program);
    let anchors: BTreeMap<String, AnchorMetadata> = expansion
        .anchors
        .iter()
        .map(|(name, anchor)| {
            (
                name.clone(),
                AnchorMetadata {
                    pos: anchor.pos,
                    facing: anchor.facing.to_string(),
                },
            )
        })
        .collect();
    let metadata = PrefabMetadata {
        prefab_id: format!("prefab/{id}"),
        structure: StructureMetadata {
            file: format!("{id}.nbt"),
            id: id.to_string(),
            size,
            data_version: DATA_VERSION,
            generator: GENERATOR.to_string(),
        },
        anchors,
        lighting: LightingMetadata {
            profile: LIGHTING_PROFILE.to_string(),
        },
        license: LicenseMetadata {
            source: "original".to_string(),
            spdx: "GPL-3.0-or-later".to_string(),
            note: "Original Delvewright project asset, derived from a grammar program in this \
                   repository. No third-party material is ingested at expansion time; the \
                   generator itself is a port credited in docs/ACKNOWLEDGEMENTS.md."
                .to_string(),
            provenance: format!(
                "Generated deterministically by {GENERATOR} (spec-0027) from grammar program \
                 {:?} ({hash}) at seed {} over a {}x{}x{} region; ADR-0006: those four inputs \
                 regenerate this NBT byte for byte.",
                program.name, options.seed, size[0], size[1], size[2]
            ),
            generated_by: GeneratedBy {
                generator: "grammar".to_string(),
                program: program.name.clone(),
                program_hash: hash.clone(),
                seed: options.seed,
            },
        },
    };
    let metadata_json =
        serde_json::to_string_pretty(&metadata).expect("prefab metadata serialises") + "\n";

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

/// Render a model as a gzip-framed vanilla structure template.
///
/// Goes through the `.schem` pipeline's emitter rather than a second one of our
/// own: one structure writer means one set of determinism guarantees and one
/// place where the 1.21.11 shape is defined.
fn structure_nbt(model: &VoxelModel) -> Result<Vec<u8>, ExportError> {
    let region = model.region();
    let size = [
        region.size[0] as i32,
        region.size[1] as i32,
        region.size[2] as i32,
    ];

    // Palette index 0 is air in both representations, and both lay cells out
    // x-major, so the model's own cell order is already the schematic's.
    let palette: Vec<SchemBlockState> = model
        .palette()
        .iter()
        .map(|b| SchemBlockState {
            name: b.name.clone(),
            properties: b.properties.clone(),
        })
        .collect();
    let index_of: BTreeMap<String, i32> = palette
        .iter()
        .enumerate()
        .map(|(i, b)| (b.to_state_string(), i as i32))
        .collect();

    let mut blocks = vec![0i32; (size[0] * size[1] * size[2]) as usize];
    let mut cells_per_state = vec![0usize; palette.len()];
    for (i, pos) in region.positions().enumerate() {
        let state = model.get(pos).expect("positions() stays inside the region");
        let index = index_of[&state.to_string()];
        blocks[i] = index;
        cells_per_state[index as usize] += 1;
    }

    // Spelling, checked by the emitter (CLAUDE.md, task #70: the operator
    // running the tool does not run `cargo test`). A structure template loads an
    // unknown block as AIR, so this is the one class of defect that costs the
    // whole piece and reports nothing at all.
    let registry = delvewright_schem::blocks::BlockRegistry::v1_21_11();
    let unknown: Vec<String> = palette
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

    let schem = ParsedSchematic {
        version: 3,
        // The model is authored *for* the pinned version, never migrated into it.
        source_data_version: Some(DATA_VERSION),
        size,
        offset: [0, 0, 0],
        palette,
        blocks,
        block_entities: Vec::new(),
    };

    let mut diagnostics = Vec::new();
    let nbt = convert::build_region(&schem, [0, 0, 0], size, &mut diagnostics);
    if !diagnostics.is_empty() {
        // The emitter's safety strip would replace these with air. A grammar
        // that asked for a command block meant to ask for one, so silently
        // shipping a hole is the worst of the three options; refuse instead.
        return Err(ExportError::ForbiddenBlocks {
            reasons: diagnostics
                .iter()
                .map(|d| match d.pos {
                    Some(p) => format!("{} at {},{},{}", d.message, p[0], p[1], p[2]),
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
