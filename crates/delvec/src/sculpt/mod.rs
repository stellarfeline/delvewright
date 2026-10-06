//! **`delvec sculpt`: a producer of prefabs from a declared form** (spec-0087).
//!
//! The grammar back end has no smooth curve, no diagonal and no noise by design.
//! This is the second back end beside it: a [`form::Form`] states a body as
//! implicit solids over its own ground, and [`sculpt`] writes the ordinary
//! prefab a campaign binds — structure parts and a metadata document — so every
//! gate downstream of admission is unchanged and no stage document learns a new
//! word.
//!
//! It writes only what it can prove about its own output, as the grammar does:
//!
//! - the grammar's always-on gates run over the sculpted model through the same
//!   functions ([`crate::grammar::gates`]), each with its binding count, sealed
//!   red on zero ([`crate::grammar::gates::seal_zero_bindings`]);
//! - `walk_y`, `shown_faces` and the lighting profile are read back off the
//!   blocks just laid, never typed;
//! - **the entry stands and is met from grade**: every anchor's cell is
//!   standable and the `role: entry` anchor is in the walk
//!   [`crate::schem::nav::ground_entry`] seeds — or `DW0952`;
//! - **no pocket**: the leave relation `DW0921` floods
//!   ([`crate::compiler::nav::World::trapped_places`], over
//!   [`crate::compiler::nav::World::body_moves`]) is run over the piece alone
//!   from every anchor, and a reachable place from which no walk, fall, jump or
//!   swim gets back to the ground at the piece's edge refuses the sculpt —
//!   `DW0952`.
//!
//! Every byte is a pure function of the form, the seed and the engine
//! (ADR-0006): the same inputs write the same parts and metadata.

pub mod cli;
pub mod fit;
pub mod form;
pub mod grid;
pub mod hull;

use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::prefab::{Anchor, AnchorRole, GeneratedBy, License, PrefabMeta};
use delvewright_dsl::{DwCode, ExitTier};

use crate::grammar::gates::{self, Gate};
use crate::grammar::geom::Box3;
use crate::grammar::model::VoxelModel;
use crate::schem::nav::{self as walk, Voxels as _};

pub use form::Form;

/// `DW0951`: the form document is refused where it is read, before anything is
/// fitted (spec-0087 §3.2, §3.5). The phase decides the exit — the form's
/// validation, exit 1 — so the code declares [`ExitTier::Build`], which is what
/// it would stop were it ever raised with a build under way.
pub const DW_ORGANIC_FORM: DwCode = DwCode::new("DW0951", ExitTier::Build);

/// `DW0952`: the sculpted body is refused — an anchor that does not stand, an
/// entry grade does not reach, or a place a body gets into and not out of
/// (spec-0087 §3.4).
pub const DW_ORGANIC_BODY: DwCode = DwCode::new("DW0952", ExitTier::Build);

/// What the `generator` breadcrumb of a sculpted structure says.
pub const GENERATOR: &str = "crates/delvec/src/sculpt";

/// How many pockets a `DW0952` refusal names before summarising the rest — the
/// number `DW0921` names.
pub const POCKETS_NAMED: usize = 6;

/// The pocket proof's reading, printed on every run whatever the verdict.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PocketReading {
    /// Places a body gets into and not out of.
    pub places: usize,
    /// Cells in them.
    pub cells: usize,
    /// Cells a body can reach from the anchors.
    pub reached: usize,
    /// Anchors the proof started from.
    pub anchors: usize,
    /// Up to [`POCKETS_NAMED`] places, in words.
    pub named: Vec<String>,
}

impl PocketReading {
    /// `pockets: P place(s), C cell(s), of R cell(s) a body can reach from E anchor(s)`.
    pub fn line(&self) -> String {
        format!(
            "pockets: {} place(s), {} cell(s), of {} cell(s) a body can reach from {} anchor(s)",
            self.places, self.cells, self.reached, self.anchors
        )
    }
}

/// The entry walk's reading.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EntryReading {
    /// Standable cells in the piece.
    pub standable: usize,
    /// Standable cells at grade on the box's vertical faces.
    pub grade_cells: usize,
    /// Standable cells a body walks to from them.
    pub reached: usize,
    /// Standing anchors (furniture excepted) the walk from grade reaches.
    pub anchors_walked: usize,
    /// Standing anchors (furniture excepted).
    pub anchors: usize,
}

/// What the fit and the output measured, for the binding lines.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Readings {
    /// Cells in the box.
    pub cells: usize,
    /// Cells holding a block.
    pub filled: usize,
    /// Solid sub-voxels stamped.
    pub sub_voxels: usize,
    /// Structure files written.
    pub parts: usize,
    /// What the fit counted.
    pub fit: fit::FitCounts,
    /// Stairs placed.
    pub stairs: usize,
    /// Slabs placed.
    pub slabs: usize,
    /// The walk plane written.
    pub walk_y: Option<i32>,
    /// The sides written shown.
    pub shown_faces: Vec<String>,
    /// The entry walk.
    pub entry: EntryReading,
    /// The pocket proof.
    pub pockets: PocketReading,
    /// The light probe's entry cells and measured cells.
    pub light_entry_cells: usize,
    /// Cells the light probe measured.
    pub light_measured_cells: usize,
    /// The piece `y` of body `y = 0`.
    pub body_floor: i64,
    /// What each `hull` light placed, and the light its sources bring to the
    /// room.
    pub hull_lights: Vec<hull::HullReading>,
}

/// A sculpted piece, rendered but not yet written.
#[derive(Debug, Clone)]
pub struct Sculpture {
    /// The prefab id stem the files carry.
    pub id: String,
    /// The model the files are a snapshot of.
    pub model: VoxelModel,
    /// Every structure file, `(filename, gzip bytes)`.
    pub files: Vec<(String, Vec<u8>)>,
    /// The metadata.
    pub metadata: PrefabMeta,
    /// The metadata as written, with its trailing newline.
    pub metadata_json: String,
    /// Every gate, with its binding count.
    pub gates: Vec<Gate>,
    /// What was measured.
    pub readings: Readings,
}

impl Sculpture {
    /// Write the metadata and every structure file into `dir`, which must exist.
    pub fn write_to_dir(&self, dir: &std::path::Path) -> std::io::Result<()> {
        for (file, bytes) in &self.files {
            std::fs::write(dir.join(file), bytes)?;
        }
        std::fs::write(dir.join(format!("{}.json", self.id)), &self.metadata_json)
    }
}

/// Why a form did not become a prefab.
#[derive(Debug, Clone)]
pub enum SculptError {
    /// `DW0951`, one line per refusal; nothing was fitted.
    Form(Vec<String>),
    /// `DW0952`, one line per refusal, with what was measured.
    Body(Vec<String>, Box<Readings>, Vec<Gate>),
    /// A machine gate went red over the sculpted model.
    Gates(Vec<Gate>, Box<Readings>),
    /// The model could not be frozen (a refusal of the shared writer).
    Export(String),
}

impl SculptError {
    /// The code a refusal is raised under, if it has one.
    pub fn code(&self) -> Option<DwCode> {
        match self {
            SculptError::Form(_) => Some(DW_ORGANIC_FORM),
            SculptError::Body(..) => Some(DW_ORGANIC_BODY),
            SculptError::Gates(..) | SculptError::Export(_) => None,
        }
    }
}

/// Parse a form document, as `delvec sculpt` reads one. A document that does
/// not parse is the document arm's refusal too (`DW0951`).
pub fn parse_form(text: &str) -> Result<Form, SculptError> {
    serde_json::from_str::<Form>(text).map_err(|e| {
        SculptError::Form(vec![format!(
            "the form does not parse as a form document: {e}. `delvec schema --stage \
             sculpt-form` prints its shape"
        )])
    })
}

/// **Sculpt a form** at `seed` into a prefab with id `prefab/<id>` (the form's
/// own `id` when `id` is `None`).
pub fn sculpt(form: &Form, seed: u64, id: Option<&str>) -> Result<Sculpture, SculptError> {
    form.check().map_err(SculptError::Form)?;
    let id = match id {
        Some(id) if crate::grammar::export::is_valid_id(id) => id.to_string(),
        Some(id) => {
            return Err(SculptError::Form(vec![format!(
                "--id {id:?} is not an id of lowercase letters, digits and hyphens"
            )]));
        }
        None => form.stem().expect("Form::check checked the id").to_string(),
    };

    // Steps 1–7: stamp, fit, refit, apron, islands, lights, shade, states.
    let (grid, owners) = grid::stamp(form, seed);
    let sub_voxels = grid.count();
    let (mut blocks, counts) = fit::fit(form, &grid, &owners);
    drop(grid);
    let hull_lights = hull::place(form, &mut blocks, seed);
    let (tone, pick) = fit::shade(&blocks, seed);
    let states = fit::states(form, &blocks, &owners, &tone, &pick);

    let region = Box3::at_origin(form.extent);
    let mut model = VoxelModel::new(region);
    let [_, dy, dz] = blocks.dims;
    let mut stairs = 0usize;
    let mut slabs = 0usize;
    for (i, state) in states.iter().enumerate() {
        let Some(state) = state else { continue };
        let pos = [
            (i / (dy * dz)) as i32,
            ((i / dz) % dy) as i32,
            (i % dz) as i32,
        ];
        match blocks.kind[i] {
            fit::Kind::Stair(..) | fit::Kind::Cover(_, fit::CoverShape::Stair(..)) => stairs += 1,
            fit::Kind::SlabBottom
            | fit::Kind::SlabTop
            | fit::Kind::Cover(_, fit::CoverShape::Slab(_)) => slabs += 1,
            _ => {}
        }
        model.set(pos, state).map_err(|e| {
            SculptError::Export(format!("the sculpted model's palette is full: {e}"))
        })?;
    }

    let mut readings = Readings {
        cells: region.volume() as usize,
        filled: model.filled_cells(),
        sub_voxels,
        fit: counts,
        stairs,
        slabs,
        body_floor: form.body_floor(),
        hull_lights,
        ..Readings::default()
    };

    // The grammar's always-on gates, through the grammar's own functions.
    let mut gate_list = vec![gates::gate_blocks_exist(&model)];
    let used = gates::placed_states(&model);
    gate_list.push(gates::gate_shape_complete(&used));
    gate_list.push(gates::gate_states_complete(&used));
    gate_list.extend(gates::gate_stair_shape(&model));
    gate_list.extend(gates::gate_fluid_contained(&model));
    gate_list.push(gates::gate_non_empty(&model));
    let (mut findings, mut enumeration) = (Vec::new(), Vec::new());
    gates::seal_zero_bindings(&mut gate_list, &mut findings, &mut enumeration);

    // What the piece says about itself, read off the blocks.
    readings.walk_y = walk_plane(&model);
    readings.shown_faces = shown_faces(&model);
    let standable = walk::standable_cells(&model);
    let grade = walk::ground_entry(&model);
    let walked = walk::reachable_from(&model, &standable, &grade);
    let standing: Vec<[i32; 3]> = form
        .anchors
        .values()
        .filter(|a| a.role != Some(AnchorRole::Furniture))
        .map(|a| a.pos)
        .collect();
    readings.entry = EntryReading {
        standable: standable.len(),
        grade_cells: grade.len(),
        reached: walked.len(),
        anchors_walked: standing.iter().filter(|p| walked.contains(*p)).count(),
        anchors: standing.len(),
    };

    if gate_list.iter().any(Gate::failed) {
        return Err(SculptError::Gates(gate_list, Box::new(readings)));
    }

    // The entry stands and is met from grade; every other anchor stands.
    let mut refusals = Vec::new();
    for (name, a) in &form.anchors {
        if a.role == Some(AnchorRole::Furniture) {
            continue; // furniture is stood beside, never on
        }
        if !walk::standable(&model, a.pos) {
            refusals.push(format!(
                "anchor {name:?} at {:?} is not a cell a body can stand in after the fit ({}) — \
                 the geometry is carved around the anchor table, so move the anchor or the \
                 solids until it stands on the body or the ground with two clear courses over it",
                a.pos,
                describe_cell(&model, a.pos)
            ));
        } else if a.role == Some(AnchorRole::Entry) && !walked.contains(&a.pos) {
            refusals.push(format!(
                "entry anchor {name:?} at {:?} stands, but the walk from grade at the box's \
                 vertical faces ({} cell(s)) does not reach it: it is on a ledge grade does not \
                 reach. Put the entry on the ground the walk starts from, or give the way to it \
                 as a `shelf`",
                a.pos,
                grade.len()
            ));
        }
    }

    // Hull light reaches the room: the engine's own light model, block light
    // only, over the piece; a source placed nowhere is a zero binding.
    hull_light_reaches(&model, &mut readings.hull_lights, &mut refusals);

    // No pocket: the leave relation `DW0921` floods, over the piece alone.
    readings.pockets = pockets(form, &model, &grade);
    if readings.pockets.places > 0 {
        let more = readings
            .pockets
            .places
            .saturating_sub(readings.pockets.named.len());
        refusals.push(format!(
            "{}: {} place(s) a body can get into and not out of — {}{}. The remedy is in the \
             form: a `shelf` out of the pocket, a `cut` that opens it, more `sink`, or a \
             different profile",
            readings.pockets.line(),
            readings.pockets.places,
            readings.pockets.named.join("; "),
            if more > 0 {
                format!("; and {more} more")
            } else {
                String::new()
            }
        ));
    }
    if !refusals.is_empty() {
        return Err(SculptError::Body(refusals, Box::new(readings), gate_list));
    }

    // Freeze through the one structure-template byte boundary.
    let frozen = crate::grammar::export::freeze_model(&model, &id, GENERATOR)
        .map_err(|e| SculptError::Export(e.to_string()))?;
    readings.parts = frozen.files.len();

    let probe = crate::admit::light::probe(
        &model,
        crate::admit::light::DEFAULT_DARK_THRESHOLD,
        crate::admit::light::SkyClaim::OpenAir,
    );
    readings.light_entry_cells = probe.entry_cells;
    readings.light_measured_cells = probe.measured_cells;
    let lighting = if probe.is_unbound() {
        delvewright_dsl::Lighting::unmeasured()
    } else {
        crate::admit::meta::lighting_from_probe(&probe)
    };

    let hash = form::form_hash(form);
    let size = [
        form.extent[0] as i32,
        form.extent[1] as i32,
        form.extent[2] as i32,
    ];
    let metadata = PrefabMeta {
        prefab_id: format!("prefab/{id}"),
        structure: frozen.structure,
        structure_set: frozen.structure_set,
        anchors: anchors(form),
        connectors: Vec::new(),
        lighting: Some(lighting),
        license: Some(License {
            source: "original".to_string(),
            spdx: "GPL-3.0-or-later".to_string(),
            note: "Original Delvewright asset, sculpted by `delvec sculpt` from a form document \
                   of implicit solids. No third-party geometry is ingested; the fitter is a port \
                   credited in docs/ACKNOWLEDGEMENTS.md."
                .to_string(),
            provenance: format!(
                "Sculpted deterministically by {GENERATOR} (spec-0087) from form {:?} ({hash}) \
                 at seed {seed} over a {}x{}x{} box; ADR-0006: the inputs in `generated_by` \
                 regenerate these bytes.",
                form.id, size[0], size[1], size[2]
            ),
            generated_by: Some(GeneratedBy {
                generator: "sculpt".to_string(),
                program: form.id.clone(),
                program_hash: hash,
                seed,
                region: size,
                params: BTreeMap::new(),
                roles: BTreeMap::new(),
            }),
        }),
        walk_y: readings.walk_y,
        waterline_y: None,
        shown_faces: readings.shown_faces.clone(),
        spatial_contract: None,
        footprint_class: None,
        extra: BTreeMap::new(),
    };
    let metadata_json = metadata.to_json();
    Ok(Sculpture {
        id,
        model,
        files: frozen.files,
        metadata,
        metadata_json,
        gates: gate_list,
        readings,
    })
}

/// The anchors as prefab metadata writes them.
fn anchors(form: &Form) -> BTreeMap<String, Anchor> {
    form.anchors
        .iter()
        .map(|(name, a)| {
            let mut anchor = Anchor {
                pos: Some(a.pos),
                facing: a.facing.clone(),
                ..Anchor::default()
            };
            anchor.role = a.role;
            anchor.region = a.region.clone();
            (name.clone(), anchor)
        })
        .collect()
}

/// **The piece's walk plane** (spec-0060 §4): the lowest local `y` holding a
/// standable cell, through the engine's own standable rule — the grammar
/// export's measurement, taken the same way.
pub fn walk_plane(model: &VoxelModel) -> Option<i32> {
    let origin_y = model.origin()[1];
    walk::standable_cells(model)
        .iter()
        .map(|c| c[1] - origin_y)
        .min()
}

/// **The sides the bytes put a block on**, in the six-word vocabulary, sorted:
/// what a sculpted piece declares as finished exterior surface. Derived, so
/// `DW0885`'s second arm (a side declared shown with no block on it) cannot be
/// raised over it.
pub fn shown_faces(model: &VoxelModel) -> Vec<String> {
    let o = model.origin();
    let m = model.maximum();
    let filled = |p: [i32; 3]| model.get(p).is_some_and(|b| !b.is_air());
    let side = |axis: usize, at: i32| {
        let (a1, a2) = match axis {
            0 => (1, 2),
            1 => (0, 2),
            _ => (0, 1),
        };
        (o[a1]..m[a1]).any(|u| {
            (o[a2]..m[a2]).any(|v| {
                let mut p = [0; 3];
                p[axis] = at;
                p[a1] = u;
                p[a2] = v;
                filled(p)
            })
        })
    };
    let mut out = Vec::new();
    for (name, axis, at) in [
        ("down", 1, o[1]),
        ("east", 0, m[0] - 1),
        ("north", 2, o[2]),
        ("south", 2, m[2] - 1),
        ("up", 1, m[1] - 1),
        ("west", 0, o[0]),
    ] {
        if side(axis, at) {
            out.push(name.to_string());
        }
    }
    out
}

/// What is at a cell, in words for a refusal.
fn describe_cell(model: &VoxelModel, pos: [i32; 3]) -> String {
    let at = |p: [i32; 3]| {
        model
            .get(p)
            .map_or("outside the box".to_string(), |b| b.to_string())
    };
    format!(
        "feet {}, head {}, floor {}",
        at(pos),
        at([pos[0], pos[1] + 1, pos[2]]),
        at([pos[0], pos[1] - 1, pos[2]])
    )
}

/// Every non-air cell of the model, by its block state string.
fn block_map(model: &VoxelModel) -> BTreeMap<[i32; 3], String> {
    model
        .region()
        .positions()
        .filter_map(|p| {
            let b = model.get(p)?;
            (!b.is_air()).then(|| (p, b.to_string()))
        })
        .collect()
}

/// **The hull's light reaches the room** (spec-0087 §9): flood the piece with
/// the compiler's one light model ([`crate::compiler::light::LightModel`]),
/// block light only, and read each `hull` source's room cell. A `hull` entry
/// that placed no source, or a source whose room cell measures 0, refuses the
/// sculpt.
fn hull_light_reaches(
    model: &VoxelModel,
    readings: &mut [hull::HullReading],
    refusals: &mut Vec<String>,
) {
    if readings.is_empty() {
        return;
    }
    let max = model.maximum();
    let light = crate::compiler::light::LightModel::from_blocks_within(
        block_map(model),
        model.origin(),
        [max[0] - 1, max[1] - 1, max[2] - 1],
    )
    .flood(0);
    for r in readings.iter_mut() {
        if r.sources.is_empty() {
            let [w, v, f] = r.candidates;
            refusals.push(format!(
                "lights[{}] (hull) placed no source: {} candidate cell(s) in `within` on the \
                 surfaces `on` names ({w} wall, {v} vault, {f} floor){}. Widen `within`, name \
                 another surface, or thicken the body round the room",
                r.light,
                w + v + f,
                if r.mode == form::LightMode::Recessed {
                    ", counting only those where a recess fits — a cover in the surface, the \
                     source behind it and a slot beside it, sealed by solid body"
                } else {
                    ""
                }
            ));
            continue;
        }
        let levels: Vec<u8> = r
            .sources
            .iter()
            .map(|s| light.get(&s.room).copied().unwrap_or(0))
            .collect();
        let lo = *levels.iter().min().expect("non-empty");
        let hi = *levels.iter().max().expect("non-empty");
        r.room_light = Some((lo, hi));
        let dark: Vec<String> = r
            .sources
            .iter()
            .zip(&levels)
            .filter(|(_, l)| **l == 0)
            .take(POCKETS_NAMED)
            .map(|(s, _)| format!("{:?} (room cell {:?})", s.at, s.room))
            .collect();
        if !dark.is_empty() {
            refusals.push(format!(
                "lights[{}] (hull): the light model measures no light in the room in front of \
                 {} source(s): {}. The light leaves a recess through its slot; a slot the body \
                 closes is a source the room never sees",
                r.light,
                levels.iter().filter(|l| **l == 0).count(),
                dark.join("; ")
            ));
        }
    }
}

/// **The pocket proof** over the piece alone (spec-0087 §3.4): the leave
/// relation `DW0921` floods, from every anchor, with the ground at the box's
/// vertical faces as the way out and the box's outside as gone.
fn pockets(form: &Form, model: &VoxelModel, grade: &BTreeSet<[i32; 3]>) -> PocketReading {
    let blocks = block_map(model);
    let world = crate::compiler::nav::World::from_occupancy(
        crate::compiler::assembled::occupancy_of(blocks, &BTreeSet::new()),
        crate::compiler::nav::Premises::geometry_only(),
    );
    let roots: Vec<[i32; 3]> = form
        .anchors
        .values()
        .filter(|a| a.role != Some(AnchorRole::Furniture))
        .map(|a| a.pos)
        .filter(|p| world.is_standable(*p))
        .collect();
    let max = model.maximum();
    let returned = Some(([0, 0, 0], [max[0] - 1, max[1] - 1, max[2] - 1]));
    let judged = world.trapped_places(&roots, grade, returned);
    let trapped: BTreeSet<[i32; 3]> = judged.pockets.iter().flatten().copied().collect();
    PocketReading {
        places: judged.pockets.len(),
        cells: trapped.len(),
        reached: judged.reached.len(),
        anchors: roots.len(),
        named: judged
            .pockets
            .iter()
            .take(POCKETS_NAMED)
            .map(|p| {
                format!(
                    "{} cell(s) around {:?}: {}, and no walk, fall, jump or swim leads from any \
                     of them back to the ground at the piece's edge",
                    p.len(),
                    p[0],
                    world.way_in_words(p, &trapped, &judged.preds)
                )
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grammar::block::BlockState;
    use crate::sculpt::form::{LightMode, Surface};
    use crate::sculpt::hull::{HullReading, Source};

    fn state(s: &str) -> BlockState {
        s.parse().expect("a block state")
    }

    /// A source whose room cell the model sees no light in refuses the
    /// sculpt, and one it sees lit does not: the arm the slot geometry keeps
    /// from firing, held here over a model built to fire it.
    #[test]
    fn a_hull_source_the_room_never_sees_is_refused() {
        let mut model = VoxelModel::new(Box3::at_origin([7, 5, 7]));
        for p in model.region().positions().collect::<Vec<_>>() {
            model.set(p, &state("minecraft:stone")).unwrap();
        }
        let lantern = [3, 2, 3];
        model
            .set(
                lantern,
                &state("minecraft:soul_lantern[hanging=false,waterlogged=false]"),
            )
            .unwrap();
        // A sealed pocket two blocks away, and an open one beside the source.
        let sealed = [3, 2, 5];
        let open = [4, 2, 3];
        model.set(sealed, &state("minecraft:air")).unwrap();
        model.set(open, &state("minecraft:air")).unwrap();
        let reading = |room| HullReading {
            light: 0,
            mode: LightMode::Recessed,
            spacing: "3".to_string(),
            candidates: [1, 0, 0],
            sources: vec![Source {
                at: lantern,
                host: lantern,
                room,
                surface: Surface::Wall,
            }],
            room_light: None,
        };
        let mut refusals = Vec::new();
        let mut dark = vec![reading(sealed)];
        hull_light_reaches(&model, &mut dark, &mut refusals);
        assert_eq!(dark[0].room_light, Some((0, 0)));
        assert_eq!(refusals.len(), 1, "{refusals:?}");
        assert!(
            refusals[0].contains("measures no light in the room"),
            "{refusals:?}"
        );

        let mut refusals = Vec::new();
        let mut lit = vec![reading(open)];
        hull_light_reaches(&model, &mut lit, &mut refusals);
        assert_eq!(lit[0].room_light, Some((9, 9)));
        assert!(refusals.is_empty(), "{refusals:?}");
    }
}
