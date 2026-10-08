//! Stage 7 — world edits (the map editor's edit script, DSL v0.6, spec-0017).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{AnchorId, AreaId, EditBatchId, Fixture, PrefabId, RegionId};

/// Stage 7 payload (optional; DSL v0.6, spec-0017): the map-editor edit script.
///
/// The artifact of record for L3 world detailing: an ordered list of edit
/// batches the compiler replays deterministically **after** world assembly.
/// The world files are never truth — same DSL + same edits + same seed →
/// byte-identical world (ADR-0006). A campaign without a `world-edits.json`
/// builds byte-identically to one from before this stage existed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorldEditsContent {
    /// Ordered edit batches, replayed in order. After every batch the post-edit
    /// invariants re-prove (walkability, sealing + relight, boundary safety),
    /// so each batch is a valid, snapshot-reviewable world state.
    pub batches: Vec<EditBatch>,
}

/// One edit batch: an ordered group of edit verbs applied to a single area,
/// checked and snapshot-rendered as a unit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EditBatch {
    /// Batch id (`batch/<kebab>`), unique in the script. Also the batch's
    /// snapshot name and seed-stream label — renaming a batch deliberately
    /// reseeds its noise.
    pub id: EditBatchId,
    /// The stage-1 area this batch edits. Frames and regions resolve against
    /// this area's placed pieces and anchors.
    pub area: AreaId,
    /// Authoring context (why this batch exists). Machine-ignored; **excluded**
    /// from l10n like `theme`/`premise`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Ordered edit verbs. `select` defines named regions; later verbs in the
    /// same batch refer back to them (strictly backward, like every DSL ref).
    pub edits: Vec<WorldEdit>,
}

/// One edit verb (spec-0017 L3). Every verb operates on named regions and every
/// seeded verb derives its noise stream from the campaign seed + its script
/// position (`edits/<batch>/<index>`) — no wall clock, no unseeded RNG.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "verb", rename_all = "kebab-case", deny_unknown_fields)]
pub enum WorldEdit {
    /// Define a named region (`region/<kebab>`) for later verbs in this batch.
    Select {
        /// The region name being defined (unique within the batch).
        name: RegionId,
        /// The region's shape (box, surface band, palette match, or a
        /// composition of earlier regions).
        shape: RegionShape,
    },
    /// Fill every cell of a region from a seeded palette recipe (value-noise
    /// keyed, so picks cluster into patches — never a uniform fill).
    Fill {
        /// The region (an earlier `select` in this batch) to fill.
        region: RegionId,
        /// The palette recipe to fill with.
        recipe: PaletteRecipe,
    },
    /// Like `fill`, but only rewrites cells whose current block matches one of
    /// `matching` (base ids; blockstate suffixes are ignored when matching).
    Replace {
        /// The region (an earlier `select` in this batch) to edit.
        region: RegionId,
        /// Base block ids to rewrite (e.g. `["minecraft:stone"]`).
        matching: Vec<String>,
        /// The palette recipe to rewrite them with.
        recipe: PaletteRecipe,
    },
    /// Clear a region to air. Sealing-aware: the carved region re-enters the
    /// sealing + relight passes and every walkability invariant re-proves.
    Carve {
        /// The region (an earlier `select` in this batch) to clear.
        region: RegionId,
    },
    /// Reshape terrain surface within a region (raise / lower / smooth).
    Morph {
        /// The region (an earlier `select` in this batch) whose columns to
        /// reshape. The region defines the footprint and where each column's
        /// surface is read (the highest occupied cell in the region's y-range);
        /// `raise`/`smooth` may add cells above the region's top — reshaping
        /// upward is the point — while removal only touches region cells.
        region: RegionId,
        /// The surface operation.
        op: MorphOp,
    },
    /// Seeded dressing scatter (spec-0017): drop weighted single-block
    /// dressing (flora, rocks, props) onto standable cells of a region —
    /// air cells with an occupied cell directly below — honoring keep-clear
    /// envelopes (`avoid`). Per-cell white-noise density gate (dressing wants
    /// speckle, not the fill verbs' clustered patches), deterministic from the
    /// campaign seed + script position.
    Scatter {
        /// The region (an earlier `select` in this batch) to dress.
        region: RegionId,
        /// Weighted dressing blocks (blockstate suffixes allowed).
        items: Vec<PaletteBlock>,
        /// Per-candidate placement probability in `(0, 1]`.
        density: f64,
        /// Keep-clear envelopes: earlier regions whose cells (and columns —
        /// matched by `(x, z)`) never receive dressing.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        avoid: Vec<RegionId>,
        /// Optional minimum spacing: when set, candidates are taken in
        /// descending noise order and one is rejected while another accepted
        /// candidate is closer than this on **both** horizontal axes (the
        /// generators' spread rule).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        spacing: Option<u32>,
        /// Optional cap on how many items are placed (highest-noise first).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<u32>,
    },
    /// Structural flora (spec-0017): plant hand-shaped trees via the
    /// lean-or-grow canopy rules — a canopy that would reach a
    /// keep-clear (`avoid`) column first leans one block away from it; if that
    /// still covers the corridor the tree grows tall instead, arching its
    /// whole canopy 3 blocks above the trunk's floor. No leaf is ever sliced.
    Plant {
        /// The region (an earlier `select` in this batch) to plant in. Trunk
        /// cells are standable region cells (air over an occupied cell).
        region: RegionId,
        /// The tree species (canopy shape rules are per-species).
        tree: TreeKind,
        /// How many trees to plant (≥ 1; highest-noise candidates first).
        count: u32,
        /// Keep-clear envelopes: trunks never stand in these columns and
        /// canopies lean/grow to clear them.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        avoid: Vec<RegionId>,
        /// Minimum trunk spacing (reject when closer on BOTH axes; default 4).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        spacing: Option<u32>,
    },
    /// Stamp a prefab fragment (spec-0017): copy a library prefab's
    /// non-air cells into the world at a frame-resolved position. The fragment
    /// is a first-class library prefab — its provenance/license metadata is
    /// recorded and validated exactly like any placed prefab (ADR-0013);
    /// nothing outside the library can be stamped.
    Fragment {
        /// The library prefab to stamp.
        prefab: PrefabId,
        /// The frame `at` resolves in.
        frame: EditFrame,
        /// Where the fragment's local `(0, 0, 0)` lands (frame coordinates).
        at: [i32; 3],
        /// Placement rotation (default `none`), the same quarter-turn set as
        /// `/place template`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rotation: Option<FragmentRotation>,
    },
    /// Explicit region relight (spec-0017, spec-0010 machinery): run the
    /// deterministic fixture-placement pass over ONE region and bake the
    /// resulting fixtures into the edit script's writes — authorial control of
    /// where fixtures land, instead of the whole-area pass's greedy siting.
    /// (The whole-area relight still re-proves after every batch either way.)
    Relight {
        /// The region (an earlier `select` in this batch) to relight: its
        /// reachable walkable cells are brought to `min_light`.
        region: RegionId,
        /// Fixture override; default = the area's declared `lighting.fixture`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fixture: Option<Fixture>,
        /// Target light override (1..=14); default = the area's declared
        /// `lighting.min_light`. Required (with `fixture`) when the area
        /// declares no `lighting`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min_light: Option<u8>,
    },
    /// L2 massing (spec-0017): replace a placed piece with another
    /// library prefab that re-mates every currently-mated socket at its exact
    /// world pose (any rotation; overlap-checked). Applied at **plan** time —
    /// the whole downstream ladder (anchors, gate reachability, assembly,
    /// relight, nav, L3 detailing) re-runs over the massaged layout. Massing
    /// verbs live in massing-only batches, ordered before every detailing
    /// batch.
    SwapPiece {
        /// The piece's placement index in its area's solved layout (0-based,
        /// entry first).
        piece: u32,
        /// The prefab the indexed piece must currently be (drift guard).
        prefab: PrefabId,
        /// The library prefab to swap in.
        with: PrefabId,
    },
    /// L2 massing (spec-0017): attach a new piece at a specific **open**
    /// (unmated) socket of an existing piece — the targeted form of the
    /// solver's frontier attach. The socket opens (its seal becomes a
    /// passage); the new piece's other sockets seal.
    InsertPiece {
        /// The host piece's placement index.
        at_piece: u32,
        /// The prefab the host piece must currently be (drift guard).
        prefab: PrefabId,
        /// The host's connector index (prefab metadata `connectors` order).
        socket: u32,
        /// The library prefab to attach.
        insert: PrefabId,
    },
    /// L2 massing (spec-0017): remove a **leaf** piece (exactly one
    /// mated socket; never the entry piece). The neighbour's socket unmates
    /// and re-seals. Removal shifts later placement indices — order removals
    /// before other index-referencing massing verbs.
    RemovePiece {
        /// The piece's placement index.
        piece: u32,
        /// The prefab the indexed piece must currently be (drift guard).
        prefab: PrefabId,
    },
    /// L2 massing (spec-0017): override one socket's seal — `open`
    /// clears the opening to a passage, `sealed` walls it up — independent of
    /// its mated state (sealing a mated doorway makes a wall between joined
    /// pieces; opening an unmated exterior socket exposes the outside, which
    /// the boundary-safety proof then judges).
    RewireSocket {
        /// The piece's placement index.
        piece: u32,
        /// The prefab the indexed piece must currently be (drift guard).
        prefab: PrefabId,
        /// The connector index (prefab metadata `connectors` order).
        socket: u32,
        /// The socket's new state.
        state: SocketState,
    },
    /// L2 massing (spec-0017): re-pick this piece from its area pool's
    /// compatible members (weighted, seeded from the campaign seed + this
    /// verb's script position — moving the verb deliberately re-rolls). The
    /// current prefab is excluded, so a reseed always changes the piece or
    /// errors loudly.
    ReseedPiece {
        /// The piece's placement index.
        piece: u32,
        /// The prefab the indexed piece must currently be (drift guard).
        prefab: PrefabId,
    },
}

/// A socket seal state for `rewire-socket` (spec-0017).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SocketState {
    /// The opening is cleared to a passage.
    Open,
    /// The opening is walled up.
    Sealed,
}

/// A tree species for the `plant` verb (spec-0017). One species per
/// canopy-rule implementation; the shipped rule set is the lean-or-grow oak.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TreeKind {
    /// Small hand-shaped oak (3–4 logs, 5-wide leaf ball) with the
    /// lean-or-grow corridor rules.
    Oak,
}

/// A `fragment` stamp rotation (spec-0017) — the `/place template`
/// quarter-turn set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum FragmentRotation {
    /// No rotation.
    None,
    /// 90° clockwise.
    Clockwise90,
    /// 180°.
    Clockwise180,
    /// 90° counterclockwise.
    Counterclockwise90,
}

/// A `select` verb's shape (spec-0017): primitive shapes resolve in a declared
/// frame; compositions combine earlier regions of the same batch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum RegionShape {
    /// An inclusive axis-aligned box, `min`/`max` in the declared frame.
    Box {
        /// The coordinate frame `min`/`max` resolve in.
        frame: EditFrame,
        /// Inclusive minimum corner (frame coordinates).
        min: [i32; 3],
        /// Inclusive maximum corner (frame coordinates; each axis ≥ `min`).
        max: [i32; 3],
    },
    /// The band of cells at `from..=to` blocks relative to each column's
    /// terrain surface (the highest non-air cell of the column) within an
    /// earlier region. `from: 1, to: 3` is the 3 cells of air-space above the
    /// surface; `from: 0, to: 0` is the surface cells themselves; negative
    /// offsets reach below the surface.
    SurfaceBand {
        /// The earlier region whose columns are scanned.
        over: RegionId,
        /// Inclusive band start, relative to each column's surface y.
        from: i32,
        /// Inclusive band end (≥ `from`), relative to each column's surface y.
        to: i32,
    },
    /// The cells of an earlier region whose current block matches one of
    /// `blocks` (base ids; blockstate suffixes ignored when matching).
    PaletteMatch {
        /// The earlier region to filter.
        within: RegionId,
        /// Base block ids to match (e.g. `["minecraft:grass_block"]`).
        blocks: Vec<String>,
    },
    /// The union of earlier regions.
    Union {
        /// The earlier regions to unite (≥ 2).
        of: Vec<RegionId>,
    },
    /// The intersection of earlier regions.
    Intersect {
        /// The earlier regions to intersect (≥ 2).
        of: Vec<RegionId>,
    },
    /// An earlier region minus other earlier regions.
    Subtract {
        /// The earlier region to start from.
        base: RegionId,
        /// The earlier regions to remove from it (≥ 1).
        remove: Vec<RegionId>,
    },
}

/// The coordinate frame a primitive [`RegionShape`] resolves in (spec-0017):
/// piece-local or anchor-relative — never raw world coordinates, so an edit
/// script survives a layout's world placement moving.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum EditFrame {
    /// The local frame of a placed piece of the batch's area: `[0, 0, 0]` is
    /// the piece's structure origin, axes as authored (the compiler applies
    /// the piece's placed rotation).
    PieceLocal {
        /// The piece's placement index in the area's solved layout (0-based,
        /// entry piece first — the order `delvec snapshot`'s manifest lists).
        piece: u32,
        /// The prefab the indexed piece must be (a drift guard: if a re-solve
        /// changed the layout, the mismatch is a loud compile error, never a
        /// silently misplaced edit).
        prefab: PrefabId,
    },
    /// Relative to a resolved anchor of the batch's area: `[0, 0, 0]` is the
    /// anchor cell, axes world-aligned.
    AnchorRelative {
        /// The anchor (prefab metadata, resolved by the compiler).
        anchor: AnchorId,
    },
}

/// A surface operation for the `morph` verb (spec-0017).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum MorphOp {
    /// Raise each column's surface by up to `by` blocks, drawing the added
    /// cells from `recipe` (value-noise keyed per cell, so a raised band reads
    /// as natural strata, not an extruded slab).
    Raise {
        /// How many blocks to raise each column (≥ 1).
        by: u32,
        /// The palette recipe for the added cells.
        recipe: PaletteRecipe,
    },
    /// Lower each column's surface by up to `by` blocks (carving the topmost
    /// solid cells to air).
    Lower {
        /// How many blocks to lower each column (≥ 1).
        by: u32,
    },
    /// Relax each column's surface toward the mean of its cardinal neighbours
    /// (one block per pass), turning steps into slopes. Added cells draw from
    /// `recipe`; removed cells carve to air. Deterministic double-buffered
    /// passes in fixed scan order.
    Smooth {
        /// Relaxation passes (≥ 1).
        passes: u32,
        /// The palette recipe for cells a pass adds.
        recipe: PaletteRecipe,
    },
}

/// A seeded palette recipe (spec-0017): weighted blocks picked per cell by a
/// smooth value-noise sample, the island/cave generators' proven primitive —
/// picks cluster into strata/patches instead of per-cell speckle, and a
/// single-entry recipe is the degenerate (discouraged) uniform case.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PaletteRecipe {
    /// Weighted palette entries (≥ 1; ≥ 2 for any visible surface).
    pub blocks: Vec<PaletteBlock>,
    /// Noise frequency in blocks⁻¹ (default `0.35` — patches a few blocks
    /// across). Larger = smaller patches. Must be finite and > 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<f64>,
}

/// One weighted entry of a [`PaletteRecipe`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PaletteBlock {
    /// Vanilla block id (validated against the pinned 1.21.11 registry), with
    /// an optional verbatim blockstate suffix (`minecraft:oak_leaves[persistent=true]`).
    pub block: String,
    /// Relative weight (finite, > 0).
    pub weight: f64,
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

use std::collections::BTreeSet;

use crate::diagnostic::{Diagnostic, DwCode, ExitTier, codes};
use crate::envelope::{Campaign, Stage};
use crate::registry::BlockRegistry;
use crate::validate::split_blockstate;

crate::dw_code! {
    /// (v0.6, spec-0017) A stage-7 edit script is structurally invalid: an edit
    /// names a region no earlier `select` in its batch defined, a composition
    /// (`union`/`intersect`/`subtract`) lists too few regions, a box `min`
    /// exceeds `max` on an axis, a surface band's `from` exceeds `to`, a palette
    /// recipe is empty / carries a non-positive or non-finite weight or `scale`,
    /// a `matching` list is empty, or a morph `by`/`passes` is 0. (Unknown block
    /// ids in recipes reuse [`BLOCK_UNKNOWN`] / `DW0193`; id-syntax and
    /// duplicate-name violations reuse `DW0110`/`DW0111`.)
    pub const EDIT_INVALID: DwCode = DwCode::new("DW0162", ExitTier::Build);
}

/// Structural validation of the stage-7 edit script: id syntax/uniqueness
/// (`DW0110`/`DW0111`), area refs (`DW0112`), strictly-backward region refs and
/// shape/recipe well-formedness (`DW0162`) and block ids (`DW0193`).
/// Frame/region *resolution* against the solved layout
/// is the compiler's job (`DW0323`) — validation never needs prefabs.
pub(crate) fn world_edits_checks(
    c: &Campaign,
    blocks: &dyn BlockRegistry,
    d: &mut Vec<Diagnostic>,
) {
    let Some(env) = &c.world_edits else {
        return;
    };
    let stage = Stage::WorldEdits.name();

    let mut areas: BTreeSet<&str> = c
        .world
        .content
        .areas
        .iter()
        .map(|a| a.id.as_str())
        .collect();
    // A site-plan campaign has no `areas[]` — `DW0839` refuses one that does —
    // and exactly one place instead: the site the plan lays out. A batch names
    // it like any other area, so it is a declared area id here for the same
    // reason `areas[]` entries are.
    //
    // One of two area-id sets (the other is [`crate::world::declared_area_ids`]),
    // and the only one that used to omit this. The pair it made was unsatisfiable: `DW0839` REQUIRES a
    // site-plan campaign to declare no `areas[]`, and every batch of a stage-7
    // edit script was then checked against a set that could only be empty. So no
    // site-plan campaign could carry an edit script at all, and the repair the
    // message prescribes — use one of the world stage's area ids — names a set
    // the other rule guarantees is empty. Each gate was right on its own terms;
    // the union had no green state. What it cost is every build-tier check a
    // stage-7 script is the only route to: content could not reach them from a
    // site-plan campaign at all.
    if c.site_plan.is_some() {
        areas.insert(crate::siteplan::SITE_AREA);
    }

    // Small helpers, each pushing at most one diagnostic.
    fn bad_syntax(d: &mut Vec<Diagnostic>, stage: &str, path: String, what: &str, id: &str) {
        d.push(Diagnostic::error(
            codes::ID_SYNTAX,
            stage,
            path,
            format!(
                "malformed {what} id `{id}` (expected `{}`)",
                what_pattern(what)
            ),
        ));
    }
    fn what_pattern(what: &str) -> String {
        format!("{what}/<kebab>")
    }
    fn check_region_ref(
        d: &mut Vec<Diagnostic>,
        stage: &str,
        regions: &BTreeSet<&str>,
        path: String,
        r: &crate::ids::RegionId,
    ) {
        if !r.is_valid_syntax() {
            bad_syntax(d, stage, path, "region", r.as_str());
        } else if !regions.contains(r.as_str()) {
            d.push(Diagnostic::error(
                EDIT_INVALID,
                stage,
                path,
                format!(
                    "region `{r}` is not defined by an earlier `select` in this batch — every \
                     region reference is strictly backward within its batch; add a `select` verb \
                     naming `{r}` above this edit (or fix the name)"
                ),
            ));
        }
    }
    fn check_recipe(
        d: &mut Vec<Diagnostic>,
        stage: &str,
        blocks: &dyn BlockRegistry,
        path: &str,
        recipe: &crate::PaletteRecipe,
    ) {
        if recipe.blocks.is_empty() {
            d.push(Diagnostic::error(
                EDIT_INVALID,
                stage,
                format!("{path}/blocks"),
                "palette recipe has no entries — give it at least one weighted block (and \
                 prefer ≥ 2 so the seeded noise reads as natural variation, never a uniform \
                 fill)"
                    .to_string(),
            ));
        }
        for (i, b) in recipe.blocks.iter().enumerate() {
            if !(b.weight.is_finite() && b.weight > 0.0) {
                d.push(Diagnostic::error(
                    EDIT_INVALID,
                    stage,
                    format!("{path}/blocks/{i}/weight"),
                    format!(
                        "palette weight `{}` for `{}` must be a finite number > 0",
                        b.weight, b.block
                    ),
                ));
            }
            check_edit_block(
                d,
                stage,
                blocks,
                format!("{path}/blocks/{i}/block"),
                &b.block,
            );
        }
        if let Some(scale) = recipe.scale
            && !(scale.is_finite() && scale > 0.0)
        {
            d.push(Diagnostic::error(
                EDIT_INVALID,
                stage,
                format!("{path}/scale"),
                format!("recipe `scale` `{scale}` must be a finite number > 0 (blocks⁻¹)"),
            ));
        }
    }
    fn check_edit_block(
        d: &mut Vec<Diagnostic>,
        stage: &str,
        blocks: &dyn BlockRegistry,
        path: String,
        block: &str,
    ) {
        match split_blockstate(block) {
            Ok(base) => {
                if !blocks.contains(base) {
                    d.push(Diagnostic::error(
                        codes::BLOCK_UNKNOWN,
                        stage,
                        path,
                        format!(
                            "block `{block}` is not a known 1.21.11 block id — use a valid \
                             namespaced block id (e.g. `minecraft:mossy_stone_bricks`)"
                        ),
                    ));
                }
            }
            Err(reason) => {
                d.push(Diagnostic::error(codes::BLOCK_UNKNOWN, stage, path, reason));
            }
        }
    }

    // A verb's phase: L2 massing (applied at plan time, over the jigsaw
    // layout) vs L3 detailing (applied at replay time, over the assembled
    // blocks). A batch never mixes phases, and every massing batch precedes
    // every detailing batch — the replay applies all massing first by
    // construction, so an interleaved script would misrepresent its own order.
    fn is_massing(edit: &WorldEdit) -> bool {
        matches!(
            edit,
            WorldEdit::SwapPiece { .. }
                | WorldEdit::InsertPiece { .. }
                | WorldEdit::RemovePiece { .. }
                | WorldEdit::RewireSocket { .. }
                | WorldEdit::ReseedPiece { .. }
        )
    }

    let mut batch_ids: BTreeSet<&str> = BTreeSet::new();
    let mut seen_detailing = false;
    for (bi, batch) in env.content.batches.iter().enumerate() {
        let bpath = format!("/batches/{bi}");
        let massing_count = batch.edits.iter().filter(|e| is_massing(e)).count();
        if massing_count > 0 && massing_count < batch.edits.len() {
            d.push(Diagnostic::error(
                EDIT_INVALID,
                stage,
                format!("{bpath}/edits"),
                format!(
                    "batch `{}` mixes L2 massing and L3 detailing verbs — massing applies at \
                     plan time (before assembly), detailing at replay time, so a mixed batch \
                     cannot execute in its written order. Split it into a massing batch and a \
                     detailing batch",
                    batch.id
                ),
            ));
        }
        if massing_count > 0 && seen_detailing {
            d.push(Diagnostic::error(
                EDIT_INVALID,
                stage,
                bpath.to_string(),
                format!(
                    "massing batch `{}` follows a detailing batch — every massing batch must \
                     precede every detailing batch (massing reshapes the layout the detailing \
                     verbs' frames resolve against). Move it up the script",
                    batch.id
                ),
            ));
        }
        if massing_count == 0 && !batch.edits.is_empty() {
            seen_detailing = true;
        }
        if !batch.id.is_valid_syntax() {
            bad_syntax(d, stage, format!("{bpath}/id"), "batch", batch.id.as_str());
        } else if !batch_ids.insert(batch.id.as_str()) {
            d.push(Diagnostic::error(
                codes::ID_DUPLICATE,
                stage,
                format!("{bpath}/id"),
                format!(
                    "duplicate batch id `{}` — batch ids are unique across the edit script \
                     (they name snapshots and seed streams)",
                    batch.id
                ),
            ));
        }
        if !areas.contains(batch.area.as_str()) {
            d.push(Diagnostic::error(
                codes::DANGLING_REF,
                stage,
                format!("{bpath}/area"),
                format!(
                    "batch `{}` targets area `{}` which this campaign does not declare — {}",
                    batch.id,
                    batch.area,
                    crate::placement::Placement::of(c).area_remedy(),
                ),
            ));
        }

        // Regions defined so far in THIS batch (strictly backward references).
        let mut regions: BTreeSet<&str> = BTreeSet::new();
        for (ei, edit) in batch.edits.iter().enumerate() {
            let epath = format!("{bpath}/edits/{ei}");
            match edit {
                WorldEdit::Select { name, shape } => {
                    match shape {
                        RegionShape::Box { frame, min, max } => {
                            if min.iter().zip(max).any(|(lo, hi)| lo > hi) {
                                d.push(Diagnostic::error(
                                    EDIT_INVALID,
                                    stage,
                                    format!("{epath}/shape"),
                                    format!(
                                        "box region `{name}` has min {min:?} > max {max:?} on \
                                         an axis — corners are inclusive with min ≤ max per axis"
                                    ),
                                ));
                            }
                            match frame {
                                EditFrame::PieceLocal { prefab, .. } => {
                                    if !prefab.is_valid_syntax() {
                                        bad_syntax(
                                            d,
                                            stage,
                                            format!("{epath}/shape/frame/prefab"),
                                            "prefab",
                                            prefab.as_str(),
                                        );
                                    }
                                }
                                EditFrame::AnchorRelative { anchor } => {
                                    if !anchor.is_valid_syntax() {
                                        bad_syntax(
                                            d,
                                            stage,
                                            format!("{epath}/shape/frame/anchor"),
                                            "anchor",
                                            anchor.as_str(),
                                        );
                                    }
                                }
                            }
                        }
                        RegionShape::SurfaceBand { over, from, to } => {
                            check_region_ref(
                                d,
                                stage,
                                &regions,
                                format!("{epath}/shape/over"),
                                over,
                            );
                            if from > to {
                                d.push(Diagnostic::error(
                                    EDIT_INVALID,
                                    stage,
                                    format!("{epath}/shape"),
                                    format!(
                                        "surface band `{name}` has from {from} > to {to} — the \
                                         band is inclusive with from ≤ to (offsets relative to \
                                         each column's surface)"
                                    ),
                                ));
                            }
                        }
                        RegionShape::PaletteMatch { within, blocks: bl } => {
                            check_region_ref(
                                d,
                                stage,
                                &regions,
                                format!("{epath}/shape/within"),
                                within,
                            );
                            if bl.is_empty() {
                                d.push(Diagnostic::error(
                                    EDIT_INVALID,
                                    stage,
                                    format!("{epath}/shape/blocks"),
                                    format!(
                                        "palette-match region `{name}` lists no blocks — name \
                                         at least one base block id to match"
                                    ),
                                ));
                            }
                            for (i, b) in bl.iter().enumerate() {
                                check_edit_block(
                                    d,
                                    stage,
                                    blocks,
                                    format!("{epath}/shape/blocks/{i}"),
                                    b,
                                );
                            }
                        }
                        RegionShape::Union { of } | RegionShape::Intersect { of } => {
                            if of.len() < 2 {
                                d.push(Diagnostic::error(
                                    EDIT_INVALID,
                                    stage,
                                    format!("{epath}/shape/of"),
                                    format!(
                                        "composition region `{name}` lists {} region(s) — a \
                                         union/intersection needs at least 2 (a single-region \
                                         composition is just the region; use it directly)",
                                        of.len()
                                    ),
                                ));
                            }
                            for (i, r) in of.iter().enumerate() {
                                check_region_ref(
                                    d,
                                    stage,
                                    &regions,
                                    format!("{epath}/shape/of/{i}"),
                                    r,
                                );
                            }
                        }
                        RegionShape::Subtract { base, remove } => {
                            check_region_ref(
                                d,
                                stage,
                                &regions,
                                format!("{epath}/shape/base"),
                                base,
                            );
                            if remove.is_empty() {
                                d.push(Diagnostic::error(
                                    EDIT_INVALID,
                                    stage,
                                    format!("{epath}/shape/remove"),
                                    format!(
                                        "subtract region `{name}` removes nothing — list at \
                                         least one region to subtract (or use `base` directly)"
                                    ),
                                ));
                            }
                            for (i, r) in remove.iter().enumerate() {
                                check_region_ref(
                                    d,
                                    stage,
                                    &regions,
                                    format!("{epath}/shape/remove/{i}"),
                                    r,
                                );
                            }
                        }
                    }
                    if !name.is_valid_syntax() {
                        bad_syntax(d, stage, format!("{epath}/name"), "region", name.as_str());
                    } else if !regions.insert(name.as_str()) {
                        d.push(Diagnostic::error(
                            codes::ID_DUPLICATE,
                            stage,
                            format!("{epath}/name"),
                            format!(
                                "duplicate region name `{name}` in batch `{}` — region names \
                                 are unique within their batch",
                                batch.id
                            ),
                        ));
                    }
                }
                WorldEdit::Fill { region, recipe } => {
                    check_region_ref(d, stage, &regions, format!("{epath}/region"), region);
                    check_recipe(d, stage, blocks, &format!("{epath}/recipe"), recipe);
                }
                WorldEdit::Replace {
                    region,
                    matching,
                    recipe,
                } => {
                    check_region_ref(d, stage, &regions, format!("{epath}/region"), region);
                    if matching.is_empty() {
                        d.push(Diagnostic::error(
                            EDIT_INVALID,
                            stage,
                            format!("{epath}/matching"),
                            "replace matches no blocks — list at least one base block id to \
                             rewrite (an unconditional rewrite is `fill`)"
                                .to_string(),
                        ));
                    }
                    for (i, b) in matching.iter().enumerate() {
                        check_edit_block(d, stage, blocks, format!("{epath}/matching/{i}"), b);
                    }
                    check_recipe(d, stage, blocks, &format!("{epath}/recipe"), recipe);
                }
                WorldEdit::Carve { region } => {
                    check_region_ref(d, stage, &regions, format!("{epath}/region"), region);
                }
                WorldEdit::Morph { region, op } => {
                    check_region_ref(d, stage, &regions, format!("{epath}/region"), region);
                    match op {
                        MorphOp::Raise { by, recipe } => {
                            if *by == 0 {
                                d.push(Diagnostic::error(
                                    EDIT_INVALID,
                                    stage,
                                    format!("{epath}/op/by"),
                                    "morph raise `by` is 0 — a zero raise is a no-op; give a \
                                     positive height (or drop the edit)"
                                        .to_string(),
                                ));
                            }
                            check_recipe(d, stage, blocks, &format!("{epath}/op/recipe"), recipe);
                        }
                        MorphOp::Lower { by } => {
                            if *by == 0 {
                                d.push(Diagnostic::error(
                                    EDIT_INVALID,
                                    stage,
                                    format!("{epath}/op/by"),
                                    "morph lower `by` is 0 — a zero lower is a no-op; give a \
                                     positive depth (or drop the edit)"
                                        .to_string(),
                                ));
                            }
                        }
                        MorphOp::Smooth { passes, recipe } => {
                            if *passes == 0 {
                                d.push(Diagnostic::error(
                                    EDIT_INVALID,
                                    stage,
                                    format!("{epath}/op/passes"),
                                    "morph smooth `passes` is 0 — a zero-pass smooth is a \
                                     no-op; give a positive pass count (or drop the edit)"
                                        .to_string(),
                                ));
                            }
                            check_recipe(d, stage, blocks, &format!("{epath}/op/recipe"), recipe);
                        }
                    }
                }
                WorldEdit::Scatter {
                    region,
                    items,
                    density,
                    avoid,
                    spacing: _,
                    limit,
                } => {
                    check_region_ref(d, stage, &regions, format!("{epath}/region"), region);
                    for (i, r) in avoid.iter().enumerate() {
                        check_region_ref(d, stage, &regions, format!("{epath}/avoid/{i}"), r);
                    }
                    if items.is_empty() {
                        d.push(Diagnostic::error(
                            EDIT_INVALID,
                            stage,
                            format!("{epath}/items"),
                            "scatter has no items — give it at least one weighted dressing \
                             block"
                                .to_string(),
                        ));
                    }
                    for (i, b) in items.iter().enumerate() {
                        if !(b.weight.is_finite() && b.weight > 0.0) {
                            d.push(Diagnostic::error(
                                EDIT_INVALID,
                                stage,
                                format!("{epath}/items/{i}/weight"),
                                format!(
                                    "scatter item weight `{}` for `{}` must be a finite \
                                     number > 0",
                                    b.weight, b.block
                                ),
                            ));
                        }
                        check_edit_block(
                            d,
                            stage,
                            blocks,
                            format!("{epath}/items/{i}/block"),
                            &b.block,
                        );
                    }
                    if !(density.is_finite() && *density > 0.0 && *density <= 1.0) {
                        d.push(Diagnostic::error(
                            EDIT_INVALID,
                            stage,
                            format!("{epath}/density"),
                            format!(
                                "scatter `density` `{density}` must be in (0, 1] — it is the \
                                 per-candidate placement probability"
                            ),
                        ));
                    }
                    if let Some(limit) = limit
                        && *limit == 0
                    {
                        d.push(Diagnostic::error(
                            EDIT_INVALID,
                            stage,
                            format!("{epath}/limit"),
                            "scatter `limit` is 0 — a zero-item scatter is a no-op; give a \
                             positive cap (or drop the field for no cap)"
                                .to_string(),
                        ));
                    }
                }
                WorldEdit::Plant {
                    region,
                    tree: _,
                    count,
                    avoid,
                    spacing: _,
                } => {
                    check_region_ref(d, stage, &regions, format!("{epath}/region"), region);
                    for (i, r) in avoid.iter().enumerate() {
                        check_region_ref(d, stage, &regions, format!("{epath}/avoid/{i}"), r);
                    }
                    if *count == 0 {
                        d.push(Diagnostic::error(
                            EDIT_INVALID,
                            stage,
                            format!("{epath}/count"),
                            "plant `count` is 0 — a zero-tree plant is a no-op; give a \
                             positive count (or drop the edit)"
                                .to_string(),
                        ));
                    }
                }
                WorldEdit::Fragment {
                    prefab,
                    frame,
                    at: _,
                    rotation: _,
                } => {
                    if !prefab.is_valid_syntax() {
                        bad_syntax(
                            d,
                            stage,
                            format!("{epath}/prefab"),
                            "prefab",
                            prefab.as_str(),
                        );
                    }
                    match frame {
                        EditFrame::PieceLocal { prefab, .. } => {
                            if !prefab.is_valid_syntax() {
                                bad_syntax(
                                    d,
                                    stage,
                                    format!("{epath}/frame/prefab"),
                                    "prefab",
                                    prefab.as_str(),
                                );
                            }
                        }
                        EditFrame::AnchorRelative { anchor } => {
                            if !anchor.is_valid_syntax() {
                                bad_syntax(
                                    d,
                                    stage,
                                    format!("{epath}/frame/anchor"),
                                    "anchor",
                                    anchor.as_str(),
                                );
                            }
                        }
                    }
                }
                WorldEdit::Relight {
                    region,
                    fixture,
                    min_light,
                } => {
                    check_region_ref(d, stage, &regions, format!("{epath}/region"), region);
                    if let Some(ml) = min_light
                        && !(1..=14).contains(ml)
                    {
                        d.push(Diagnostic::error(
                            EDIT_INVALID,
                            stage,
                            format!("{epath}/min_light"),
                            format!(
                                "relight `min_light` {ml} out of range — vanilla block light \
                                 is 1..=14 (15 is only at the emitter itself)"
                            ),
                        ));
                    }
                    // Without an area `lighting` declaration the verb has no
                    // fixture/target to fall back on — both overrides required.
                    let area_lighting = c
                        .world
                        .content
                        .areas
                        .iter()
                        .find(|a| a.id.as_str() == batch.area.as_str())
                        .and_then(|a| a.lighting);
                    if area_lighting.is_none() && (fixture.is_none() || min_light.is_none()) {
                        d.push(Diagnostic::error(
                            EDIT_INVALID,
                            stage,
                            epath.to_string(),
                            format!(
                                "relight in batch `{}`: area `{}` declares no `lighting`, so \
                                 the verb must carry BOTH `fixture` and `min_light` (there is \
                                 nothing to default to). Declare area lighting or add the \
                                 overrides",
                                batch.id, batch.area
                            ),
                        ));
                    }
                }
                WorldEdit::SwapPiece {
                    piece: _,
                    prefab,
                    with,
                } => {
                    for (what, id) in [("prefab", prefab.as_str()), ("prefab", with.as_str())] {
                        if !crate::ids::is_prefixed(id, "prefab") {
                            bad_syntax(d, stage, epath.to_string(), what, id);
                        }
                    }
                }
                WorldEdit::InsertPiece {
                    at_piece: _,
                    prefab,
                    socket: _,
                    insert,
                } => {
                    for id in [prefab.as_str(), insert.as_str()] {
                        if !crate::ids::is_prefixed(id, "prefab") {
                            bad_syntax(d, stage, epath.to_string(), "prefab", id);
                        }
                    }
                }
                WorldEdit::RemovePiece { piece: _, prefab }
                | WorldEdit::ReseedPiece { piece: _, prefab }
                | WorldEdit::RewireSocket { prefab, .. } => {
                    if !prefab.is_valid_syntax() {
                        bad_syntax(
                            d,
                            stage,
                            format!("{epath}/prefab"),
                            "prefab",
                            prefab.as_str(),
                        );
                    }
                }
            }
        }
    }
}
