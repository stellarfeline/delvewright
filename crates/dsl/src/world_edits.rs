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
