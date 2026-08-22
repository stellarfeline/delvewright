//! The prefab metadata document (`<id>.json`, beside the structure `.nbt`) —
//! the **one** definition of its shape.
//!
//! A prefab is a *pair* of files: a gzip-framed structure template and this
//! sibling JSON that says what the template is, where its anchors and sockets
//! are, how lit it is, what it claims about the space inside it, and what
//! regenerates it. Both halves are produced and consumed by several tools of
//! several ages — the grammar back end and the hand-written generators write the
//! pair from scratch, `delve-admit` reads it and writes it back after every
//! admission step, `delvec` reads it to plan a world, `delve-render` reads it to
//! aim a camera — so the document's shape is defined once, here, and every one
//! of them reads that definition instead of a copy of it.
//!
//! # Why the definition lives in the DSL crate
//!
//! Not because prefab metadata is DSL surface — it is a library-asset document —
//! but because this is the only crate every reader can depend on. `delvec` is
//! published to crates.io and may only depend on published crates, and this crate
//! is the one it already depends on. The alternative was a copy inside `delvec`,
//! which is what existed and what this module replaces. The crate already owns
//! the document's `lighting` block ([`crate::registry::Lighting`], whose field
//! names are this file's field names) and the anchor surface DSL validation
//! resolves refs against ([`crate::registry::AnchorRegistry`]), so the document's
//! remaining blocks join a shape that was already half here.
//!
//! # Reading is total, writing preserves
//!
//! Every field a producer may legitimately omit is `Option`/`default` and is
//! omitted (never `null`) on write, so a legacy prefab that predates a field
//! still loads and a piece that has never been probed does not have to invent a
//! measurement. Field order is the emission order, and it is the order the
//! library's checked-in prefabs already use, so a reviewer diffing a generated
//! piece against a hand-built one sees only values change.
//!
//! Keys this version has never heard of are **kept**, in [`PrefabMeta::extra`]
//! and [`Anchor::extra`], and written back out. That is not politeness to the
//! future; it is the only behaviour that is neither an outage nor silent data
//! loss. See the `deny_unknown_fields` note below.
//!
//! # `deny_unknown_fields`, decided rather than inherited
//!
//! The attribute is right on a document whose reader is also its **owner**: a
//! campaign stage document is authored against a versioned schema, a typo there
//! is the bug the attribute exists to catch, and forward compatibility is
//! handled by the `dsl_version` fence instead. Every stage struct in
//! [`crate::stages`] keeps it for exactly that reason.
//!
//! It is wrong on a **consumer that is not the owner**, which is what every
//! reader of this document is. Here a new key is not a typo — it is a newer
//! producer meeting an older reader, which happens on every mixed-version pair
//! of engine and content library. Refusing turns a forward addition into a hard
//! failure at the layer with the least context; the compiler's private copy of
//! this shape did exactly that, and the first grammar-exported prefab carrying a
//! new key would have failed every campaign build.
//!
//! Tolerating alone is not the fix either, because a tool that reads this
//! document, edits one block and writes it back deletes everything it does not
//! model — and does so while every test it has passes. That is not
//! hypothetical: `license.generated_by` was dropped that way once, and
//! `waterline_y` — a field five shipped island prefabs carry and the
//! ocean-horizon placement check keys off — was being dropped that way at the
//! time this module was written.
//!
//! So the rule is: **this document's structs neither refuse an unknown key nor
//! discard it.** They keep it, and the reader that wants to say something about
//! it says it as a diagnostic (`DW0543`) rather than as a parse failure. The
//! blocks whose own definition lives elsewhere are the exception and say why at
//! their field.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::registry::Lighting;
use crate::split::TileSet;

/// The one `.json` in a prefab library that is **not** a prefab document:
/// the pool declaration (`{"pools": {...}}`), read by the compiler's registry.
///
/// Named once because more than one tool walks the library directory — the
/// registry, `delvec view`'s page builder, `delve-render batch` — and each of
/// them opens every `.json` it finds. A walker that does not know this name
/// hands a pool file to [`PrefabMeta::from_json`] and reports it as a malformed
/// prefab, which is a true statement about the bytes and a wrong one about the
/// file.
pub const POOLS_FILE: &str = "pools.json";

/// A prefab's sibling metadata file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrefabMeta {
    /// The DSL prefab id, `prefab/<id>`.
    pub prefab_id: String,
    /// The structure-template reference, for a piece whose blocks fit one
    /// template.
    ///
    /// Exactly one of this and [`Self::structure_set`] is present — see the
    /// type's own note on the two packagings, and [`Self::from_json`], which is
    /// where "exactly one" is enforced.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structure: Option<StructureMeta>,
    /// The tile set, for a piece whose blocks did not fit one template.
    ///
    /// **Packaging, not authoring.** A zone past the 48-per-axis structure cap
    /// ships as several `.nbt` files plus this manifest; everything else about
    /// the document — the id, the zone-local `anchors`, the `connectors`, the
    /// one `lighting` block, the one provenance row — is what it is for a
    /// single-template piece, because it describes the same building. Nothing
    /// that refers to a piece may ask which of the two it is: read
    /// [`Self::templates`].
    ///
    /// This was a second document type (`TileSetMeta`, in the schem crate),
    /// field-for-field this one with `structure` swapped for `structure_set`.
    /// The copy had already lost `waterline_y`, so a tiled shore could not
    /// declare the waterline the ocean-horizon invariant (`DW0344`) keys off and
    /// went silently unchecked. One document is what makes that class of drift
    /// unrepresentable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structure_set: Option<TileSet>,
    /// Named anchors, keyed by DSL anchor name. `{}` for a piece that declares
    /// none.
    #[serde(default)]
    pub anchors: BTreeMap<String, Anchor>,
    /// Jigsaw sockets. `[]` for a piece that is placed directly rather than
    /// drawn from a pool.
    #[serde(default)]
    pub connectors: Vec<Connector>,
    /// The lighting declaration.
    ///
    /// Absent means legacy metadata that predates the field, which is a
    /// different claim from `{"profile": "unmeasured"}` — the positive statement
    /// that a measurement is owed.
    ///
    /// The block's own shape is [`Lighting`], and it is **the one part of this
    /// document that still refuses a key it does not know**. Its job is a rule
    /// about values — a measured profile must carry its measurement, an
    /// `unmeasured` one must not — so a misspelled measurement key there is a
    /// claim quietly becoming its own absence, which the profile/measurement
    /// agreement alone does not catch for `rationale` or `method`. The cost is
    /// real and is stated where an author will meet it
    /// (`docs/reference/prefab-procedure.md` §9): a key added inside `lighting`
    /// is a hard parse failure for an older engine, so adding one is a
    /// `dsl_version` matter rather than a metadata edit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lighting: Option<Lighting>,
    /// Licence, provenance prose, and the machine-readable provenance row.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<License>,
    /// The local y of this piece's **top authored water block** — its waterline
    /// — for open-air pieces built to a tileset convention that authors a sea.
    /// Consumed by the ocean-horizon placement invariant (`DW0344`): in a
    /// `horizon: ocean` world the declared waterline must land at world sea
    /// level. Absent for pieces that author no sea, which are then not checked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waterline_y: Option<i32>,
    /// The piece's spatial contract, when it declares one.
    ///
    /// Absent means legacy metadata — the piece makes no spatial claim — exactly
    /// as an absent `lighting` block differs from `unmeasured`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spatial_contract: Option<SpatialContract>,
    /// **The size class of box this piece is built to fill** — a name from the
    /// metrics table's `size-class.*` ladder (spec-0050 §5).
    ///
    /// Optional for the library at large, and that is deliberate rather than
    /// lax: every piece in the library predates the field, and `DW0848` binds
    /// only where the claim is made. What is *not* optional is the claim being
    /// true — a piece declaring a class its own bytes could serve no box of is
    /// refused at admission and again wherever a detail plan consumes it, so a
    /// pre-check-era piece cannot be consumed unjudged.
    ///
    /// Absent means what absence means everywhere in this document: the claim is
    /// not made. A piece bound by a `details[]` row is still checked for exact
    /// frame equality (`DW0843`) whether or not it declares — that is the
    /// consumer's exact check, and this is the library's approximate one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub footprint_class: Option<String>,
    /// Every top-level key this version does not model, kept verbatim so that
    /// reading and writing the document is not the same as editing it.
    ///
    /// A reader that wants to report one has it in hand; a reader that does not
    /// care carries it through. Emitted after the modelled keys, in key order.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// A piece's declared spaces, out-of-walk regions and edges, **already
/// resolved**: every box is a local cell range of these exact bytes.
///
/// Resolved rather than parametric on purpose. A grammar program's declarations
/// are scope-bound and mean different boxes at different parameters, so the only
/// contract that can describe *this* `.nbt` is the one its own expansion
/// produced. That is also what lets a hand-built piece carry the same block: it
/// has no parameters to resolve, so the two routes write the same shape and one
/// reader serves both.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpatialContract {
    /// The space a body enters at.
    pub entry: String,
    /// Named spaces.
    #[serde(default)]
    pub spaces: BTreeMap<String, ContractSpace>,
    /// Named standable-but-out-of-walk regions.
    #[serde(default)]
    pub no_body: BTreeMap<String, ContractNoBody>,
    /// The graph, in declaration order.
    #[serde(default)]
    pub edges: Vec<ContractEdge>,
    /// **The piece's face contract**: every `exterior` edge, as the side of the
    /// piece it is on and the opening it leaves there.
    ///
    /// Derived from the edges and the blocks at export time and written out, so
    /// that assembly can ask whether two pieces fit without opening either
    /// `.nbt`. It is the thing an `exterior` edge IS from the outside: an edge
    /// with no cells is a claim nothing can mate with, and one whose opening
    /// does not answer its neighbour's is two pieces that were each approved
    /// alone and do not assemble.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub faces: Vec<ContractFace>,
    /// The author's acknowledgement that this piece is mostly out-of-walk.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_body_majority_ack: Option<String>,
}

/// `DW0848`: a piece's declared footprint class disagrees with its bytes.
pub const DW_FOOTPRINT_CLASS: crate::DwCode = crate::DwCode::every_version("DW0848");

/// **Judge a piece's declared `footprint_class` against its own structure
/// size.**
///
/// One authority with two doors, on the pattern spec-0036 §1c fixed for the
/// spatial contract: `delve-admit audit` asks it at the admission event, where
/// the library's integrity lives, and `delvewright_compiler::detail` asks it
/// again wherever a `detail-plan` row consumes the piece. Two implementations
/// that agreed until they did not is the failure this shape removes.
///
/// What it asks, and each half is a fact about the geometry rather than a
/// preference:
///
/// 1. **The name is in the table** — otherwise `DW0812`, as for any document
///    naming a table entry.
/// 2. **The horizontal extents could be a box of that class.** A detail frame's
///    footprint IS its box's footprint (`Frame::of` grows the play space
///    downward only), so a piece whose `x` or `z` falls outside the class's
///    `min_footprint..=max_footprint` could fill no box of it.
/// 3. **They sit on the kit grid.** A site-plan box's extent is a multiple of
///    the grid quantum (`DW0825`), so a piece off the grid could fill no box at
///    all, of any class.
/// 4. **The height leaves the class its clearance.** A frame is the play space
///    plus one floor course, so a piece under `min_clearance + 1` is short of
///    the shallowest box of its class.
///
/// Returns `None` for a piece that declares no class — which is the honest
/// answer, and why the caller states how many pieces declared one against how
/// many it examined.
#[must_use]
pub fn check_footprint_class(
    meta: &PrefabMeta,
    stage: &str,
    path: &str,
    reads: &mut crate::metrics::Reads,
) -> Option<crate::Diagnostic> {
    let named = meta.footprint_class.as_deref()?;
    let table = crate::metrics::Metrics::table();
    let entry = match table.resolve(crate::metrics::MetricKind::SizeClass, named) {
        Ok(e) => e,
        Err(unknown) => return Some(unknown.diagnostic(stage, path)),
    };
    let crate::metrics::MetricValue::SizeClass(class) = *entry.value(reads) else {
        return None; // an internal table defect, which `Metrics::self_check` owns.
    };
    let size = meta.size();
    let grid = table.grid(reads);
    let q = grid.map_or(1, |g| i64::from(g.quantum).max(1));
    let (sx, sy, sz) = (i64::from(size[0]), i64::from(size[1]), i64::from(size[2]));
    let (minf, maxf) = (class.min_footprint, class.max_footprint);
    let mut why: Vec<String> = Vec::new();
    if sx < i64::from(minf[0]) || sx > i64::from(maxf[0]) {
        why.push(format!(
            "its x extent is {sx}, and a `{named}` box is {}..={} on x",
            minf[0], maxf[0]
        ));
    }
    if sz < i64::from(minf[1]) || sz > i64::from(maxf[1]) {
        why.push(format!(
            "its z extent is {sz}, and a `{named}` box is {}..={} on z",
            minf[1], maxf[1]
        ));
    }
    if sx % q != 0 || sz % q != 0 {
        why.push(format!(
            "its footprint {sx}x{sz} is off the kit grid, whose quantum is {q} — every site-plan \
             box's extent is a multiple of it (`DW0825`)"
        ));
    }
    let least = i64::from(class.min_clearance) + 1;
    if sy < least {
        why.push(format!(
            "it is {sy} cells tall, and the shallowest `{named}` frame is {least} — {} of \
             clearance plus the one floor course a piece owns",
            class.min_clearance
        ));
    }
    if why.is_empty() {
        return None;
    }
    Some(crate::Diagnostic::error(
        DW_FOOTPRINT_CLASS,
        stage,
        path,
        format!(
            "`{id}` declares `footprint_class: \"{named}\"` and its own bytes could serve no box \
             of that class: {why}. The declaration is a claim about what this piece is FOR, and a \
             site plan hands a piece the exact frame of the box it fills — so a piece whose \
             extents no box of the class can have is a piece no `details[]` row could ever bind. \
             Either correct the class name, or rebuild the piece to a frame of the class it \
             claims. Structure size is {sx}x{sy}x{sz}.",
            id = meta.prefab_id,
            why = why.join("; "),
        ),
    ))
}

/// One face of the piece's face contract.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContractFace {
    /// The space the way in or out belongs to.
    pub space: String,
    /// The edge's class: `walk` | `stair` | `drop` | `barred` | `vision`.
    pub class: String,
    /// Which side of the piece: `east` | `west` | `up` | `down` | `south` |
    /// `north`.
    pub dir: String,
    /// The opening, as an inclusive local cell range flat in the face's own
    /// axis.
    pub opening: Region,
}

/// One entry of `spatial_contract.spaces`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContractSpace {
    /// `enclosed` | `open_top` | `open`.
    pub envelope: String,
    /// The cells it covers.
    pub boxes: Vec<Region>,
}

/// One entry of `spatial_contract.no_body`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContractNoBody {
    /// Why these cells are out of play, in the author's words. Which exemption
    /// the region qualifies for is a fact about the blocks and is not recorded
    /// here.
    pub reason: String,
    /// The cells it covers.
    pub boxes: Vec<Region>,
}

/// One entry of `spatial_contract.edges`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContractEdge {
    /// A declared space name, or `exterior`.
    pub a: String,
    /// A declared space name, or `exterior`.
    pub b: String,
    /// `walk` | `stair` | `drop` | `barred` | `vision`.
    pub class: String,
    /// The declared level change, on the classes that carry one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rise: Option<i64>,
    /// The opening or transit volume, when the edge declares one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub via: Option<ContractVolume>,
    /// The bar, on a `barred` edge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bar: Option<ContractBar>,
    /// **The contingency**, on a traversal edge that content opens: the region
    /// the edge is severed by as built, and which direction opening it goes.
    ///
    /// Absent on every edge that is what it claims to be as shipped, which is
    /// why a piece that declares none writes no key at all and its metadata is
    /// byte-for-byte what it was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub way: Option<ContractWay>,
}

/// A contingent edge's way: the region that decides whether the edge is
/// crossable, and which direction opening it moves in.
///
/// The dual of [`ContractBar`], and the reason `bar` is not extended in place:
/// an existing piece's metadata says `bar` and keeps saying `bar`. The
/// **checker** normalises the two into one prover; the document keeps both
/// spellings, so nothing already written moves a byte.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContractWay {
    /// `laid` — the region is empty as built and opening fills it with
    /// [`block`](ContractWay::block); or `cleared` — the region stands in
    /// `block` as built and opening voids it.
    pub opens: String,
    /// The region's name, which is what content addresses.
    pub region: String,
    /// The cells it covers.
    pub boxes: Vec<Region>,
    /// The palette role the way is made of, in the author's own vocabulary.
    ///
    /// Provenance for a reader, and never a second authority: what an opening
    /// writes is [`block`](ContractWay::block), because a role name means
    /// nothing outside the program that bound it. Recorded because a reviewer
    /// reading this document otherwise has no way back to the declaration —
    /// `minecraft:oak_planks` says what the cells become and `"tread"` says
    /// what the author called it.
    ///
    /// Optional for the reason [`License::generated_by`] is: a role is a
    /// *program's* vocabulary, so an expansion always has one and a hand-built
    /// or ingested piece — which names its blocks directly — has none at all.
    /// Writing an invented role there would be a fact about nothing.
    ///
    /// [`ContractBar`] carries no such field, and deliberately: adding one
    /// would move the exported bytes of every piece that already declares a
    /// bar, which spec-0042 §2.3 forbids. The checker reads neither.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// The block state the way is made of: what a `laid` way is filled with,
    /// and what a `cleared` way stands in.
    pub block: String,
}

/// An edge's own volume — an opening, a stair's treads, a fall column.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContractVolume {
    /// The region's name, which is what content binds to.
    pub region: String,
    /// The cells it covers.
    pub boxes: Vec<Region>,
}

/// A `barred` edge's bar: the region that stands in the way, and its block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContractBar {
    /// The region's name.
    pub region: String,
    /// The cells it covers.
    pub boxes: Vec<Region>,
    /// The block state the bar is built from.
    pub block: String,
}

/// The `structure` block: which file, how big, for which MC version.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructureMeta {
    /// The `.nbt` filename, relative to this metadata file.
    pub file: String,
    /// The datapack structure id (a path segment).
    pub id: String,
    /// Structure extent `[x, y, z]`.
    pub size: [i32; 3],
    /// The MC data version the structure targets (ADR-0009).
    pub data_version: i32,
    /// Provenance breadcrumb: what wrote the `.nbt`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generator: Option<String>,
}

/// One structure template a piece's blocks arrive in, and where in the piece it
/// sits.
///
/// **The unit every placer works in.** A single-template prefab has exactly one,
/// at `offset` `[0, 0, 0]`; a tiled zone has one per tile at its manifest
/// offset. Nothing that places, stamps or reads a piece's blocks needs to know
/// which of the two it was handed — that is the whole point of the type, and the
/// reason [`PrefabMeta::templates`] is the only way to reach a `.nbt` filename.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PieceTemplate<'a> {
    /// The datapack structure id (a path segment).
    pub id: &'a str,
    /// The `.nbt` filename, relative to the metadata file.
    pub file: &'a str,
    /// This template's origin in **piece-local** coordinates — add it to a
    /// template-local cell to get the piece cell. `[0, 0, 0]` for a
    /// single-template piece.
    pub offset: [i32; 3],
    /// The template's extent `[x, y, z]`.
    pub size: [i32; 3],
}

/// One entry of the `anchors` map.
///
/// A point anchor carries `pos` (+ optionally `facing`); a gate anchor carries a
/// `region` (+ optionally `block`); a trap anchor also carries the hardware the
/// prefab pre-wired for it. All of those are the same object class — a named
/// place in a piece — so they live in one type and each writes only the keys it
/// means.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Anchor {
    /// Local cell `[x, y, z]`, relative to the structure origin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<[i32; 3]>,
    /// Cardinal facing keyword.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facing: Option<String>,
    /// Local cell range, for a gate anchor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<Region>,
    /// Block id filling a gate region (e.g. `minecraft:iron_bars`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block: Option<String>,
    /// **Which element of the piece's spatial contract this anchor lands in** —
    /// `space:<name>`, `no_body:<name>`, `via:<name>` or `bar:<name>`.
    ///
    /// A campaign binds content to an anchor by name; what says whether that
    /// place is play space, a door or exterior dressing is the contract, and a
    /// reader who has only the anchor list cannot tell. Absent on a piece that
    /// declares no contract, and on an anchor that lands in nothing the contract
    /// accounts for — which is a finding the checker raises rather than a
    /// silence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolves_to: Option<String>,
    /// The pre-wired dispenser socket cell (local coords) for an `anchor/trap`
    /// marker. `pos` is the trap's trigger/hazard cell (the plate, tripwire or
    /// chest modelled as the hazard); `dispenser` is the separate cell holding
    /// the empty dispenser whose payload is filled at compile time. Absent for
    /// every non-trap anchor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dispenser: Option<[i32; 3]>,
    /// The block the prefab wired as this `anchor/trap`'s **trigger** — the
    /// plate or tripwire sitting on `pos` — with its full blockstate exactly as
    /// authored (`minecraft:oak_pressure_plate[powered=false]`), because
    /// flag-gating a trap physically removes and restores this block and must
    /// put back what was there. The gate-anchor `block` above is the same
    /// contract for a sealed gate. Absent for every non-trap anchor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_block: Option<String>,
    /// Every anchor key this version does not model, kept verbatim. The anchor
    /// block is where this document has grown most often — `resolves_to`,
    /// `dispenser` and `trigger_block` were each a new key on a shipped
    /// document — so it captures for the same reason [`PrefabMeta::extra`]
    /// does.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl Anchor {
    /// The point-anchor shape: a cell and a facing.
    pub fn point(pos: [i32; 3], facing: impl Into<String>) -> Anchor {
        Anchor {
            pos: Some(pos),
            facing: Some(facing.into()),
            ..Anchor::default()
        }
    }
}

/// A gate anchor's region and fill block, in **piece-local** coordinates — the
/// answer [`PrefabMeta::gate_anchor`] gives, and the only shape either reader
/// works from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateAnchor {
    /// Low corner of the region, piece-local.
    pub from: [i32; 3],
    /// High corner of the region, piece-local, inclusive.
    pub to: [i32; 3],
    /// The block the region is filled with and cleared of.
    pub block: String,
}

impl GateAnchor {
    /// The region as a reader sees it in a diagnostic.
    fn extent(&self) -> String {
        format!(
            "[{},{},{}]..[{},{},{}]",
            self.from[0], self.from[1], self.from[2], self.to[0], self.to[1], self.to[2]
        )
    }
}

/// The contract-element name a `bar:<region>` [`Anchor::resolves_to`] carries,
/// and nothing else's. Every other element kind is a place a body stands, looks
/// through or walks over — not a thing content fills.
fn bar_name(resolves_to: &str) -> Option<&str> {
    resolves_to.strip_prefix("bar:")
}

/// The one box `boxes` exactly fills, or `None` when they do not fill one.
///
/// A contract region is a **list** of boxes, and a gate is a **single** box: the
/// compiler fills it and clears it as one region, and every consumer of a
/// resolved gate — the assembler that voids it, the seal that rebuilds it, the
/// nav model that walks through it — is written against two corners. So the
/// question is not "what box contains these" but "do these BE a box": a bounding
/// box that the members do not fill would hand every one of those consumers
/// cells the contract never called bar, and the assembler would delete them.
///
/// Exact, not approximate: the members must be pairwise disjoint and their
/// volumes must sum to the bounding box's. A doorway declared as a lintel row
/// plus the two jambs under it is one box and passes; a doorway plus a
/// threshold nub hanging off its corner is not, and is refused.
fn one_box(boxes: &[Region]) -> Option<Region> {
    let first = boxes.first()?;
    let mut from = first.from;
    let mut to = first.to;
    for b in &boxes[1..] {
        for a in 0..3 {
            from[a] = from[a].min(b.from[a]).min(b.to[a]);
            to[a] = to[a].max(b.from[a]).max(b.to[a]);
        }
    }
    let vol = |f: [i32; 3], t: [i32; 3]| -> i64 {
        (0..3)
            .map(|a| i64::from(t[a] - f[a]) + 1)
            .try_fold(1i64, |acc, n| if n > 0 { acc.checked_mul(n) } else { None })
            .unwrap_or(0)
    };
    let total: i64 = boxes.iter().map(|b| vol(b.from, b.to)).sum();
    if total != vol(from, to) {
        return None;
    }
    for (i, a) in boxes.iter().enumerate() {
        for b in &boxes[i + 1..] {
            if (0..3).all(|k| a.from[k] <= b.to[k] && b.from[k] <= a.to[k]) {
                return None;
            }
        }
    }
    Some(Region { from, to })
}

/// Where an anchor is — the only part of an anchor an editing tool declares.
///
/// Deliberately a different type from [`Anchor`]: the whole anchor is what a
/// caller must not be able to hand an editing step, because constructing one
/// means filling in — and therefore erasing — the hardware, provenance and
/// unknown keys the caller knows nothing about. See [`PrefabMeta::edit_anchor`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AnchorEdit {
    /// Local cell `[x, y, z]`, for a point anchor.
    pub pos: Option<[i32; 3]>,
    /// Cardinal facing keyword.
    pub facing: Option<String>,
    /// Local cell range, for a gate anchor.
    pub region: Option<Region>,
    /// Block id filling a gate region.
    pub block: Option<String>,
}

/// An inclusive local cell range.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Region {
    /// Low corner `[x, y, z]`.
    pub from: [i32; 3],
    /// High corner `[x, y, z]`.
    pub to: [i32; 3],
}

/// One jigsaw socket declared by a prefab.
///
/// `local_pos` is the socket's wall cell (bottom-centre of the opening) in the
/// prefab's local coordinates; `facing` is the cardinal direction the opening
/// faces outward. Two sockets mate by placing the child so its socket sits one
/// block beyond the parent's, facing the opposite way.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Connector {
    /// Jigsaw `name`.
    pub name: String,
    /// Jigsaw `target`.
    pub target: String,
    /// The socket's wall cell, local coords `[x, y, z]`.
    pub local_pos: [i32; 3],
    /// Cardinal direction the opening faces outward.
    pub facing: String,
    /// Opening extent `[width, height]`.
    pub opening: [i32; 2],
    /// Jigsaw joint.
    pub joint: String,
}

/// The profile of a prefab whose light nothing has measured.
pub const UNMEASURED: &str = "unmeasured";

/// The `license` block: the human half and the machine half of provenance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct License {
    /// Where the asset came from (`original`, or a named upstream).
    pub source: String,
    /// SPDX id (ADR-0013).
    pub spdx: String,
    /// Human note.
    pub note: String,
    /// Human-readable provenance sentence.
    pub provenance: String,
    /// The machine-readable provenance row: what regenerates these exact bytes.
    ///
    /// Absent for a piece nothing can regenerate — an ingested community build,
    /// or a hand-edited one. Present, it is the ADR-0006 claim in a form a tool
    /// can act on rather than a sentence a human can read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_by: Option<GeneratedBy>,
}

/// Everything needed to reproduce the `.nbt` byte for byte (ADR-0006).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeneratedBy {
    /// The back end that produced the bytes.
    pub generator: String,
    /// The source program's name.
    pub program: String,
    /// `sha256:<64 hex>` over the program's canonical JSON.
    pub program_hash: String,
    /// The expansion seed.
    pub seed: u64,
}

impl PrefabMeta {
    /// Parse metadata from JSON text.
    ///
    /// **The one reader of both packagings.** "Which shape is this" has exactly
    /// two answers and no third, so a document declaring neither block, or both,
    /// is refused here rather than handed half-read to a step that will place
    /// some of its blocks. A tile set is validated as it is read
    /// ([`TileSet::validate`]) for the same reason: a manifest that does not
    /// tile its own volume reassembles into a building with a hole in it and
    /// reports success.
    pub fn from_json(text: &str) -> Result<PrefabMeta, String> {
        let meta: PrefabMeta =
            serde_json::from_str(text).map_err(|e| format!("invalid prefab metadata: {e}"))?;
        match (&meta.structure, &meta.structure_set) {
            (None, None) => {
                return Err("prefab metadata has neither a `structure` block nor a \
                            `structure_set` block — it does not say what blocks it describes"
                    .to_string());
            }
            (Some(_), Some(_)) => {
                return Err(
                    "prefab metadata has BOTH a `structure` block and a `structure_set` \
                            block — a piece's blocks arrive one way or the other, and a reader \
                            cannot be asked which one is the building"
                        .to_string(),
                );
            }
            (None, Some(set)) => set
                .validate()
                .map_err(|e| format!("`structure_set`: {e}"))?,
            (Some(_), None) => {}
        }
        Ok(meta)
    }

    /// Every structure template this piece's blocks arrive in, in a
    /// deterministic order (grid order for a tile set).
    ///
    /// Empty only for a value built in code that declares neither block, which
    /// [`Self::from_json`] refuses — nothing read from disk is in that state.
    pub fn templates(&self) -> Vec<PieceTemplate<'_>> {
        if let Some(s) = &self.structure {
            return vec![PieceTemplate {
                id: &s.id,
                file: &s.file,
                offset: [0, 0, 0],
                size: s.size,
            }];
        }
        self.structure_set
            .iter()
            .flat_map(|set| {
                set.parts.iter().map(|p| PieceTemplate {
                    id: &p.id,
                    file: &p.file,
                    offset: p.offset,
                    size: p.size,
                })
            })
            .collect()
    }

    /// The piece's extent `[x, y, z]` — the WHOLE building, whichever packaging
    /// its blocks arrived in. `[0, 0, 0]` only for the value
    /// [`Self::templates`] documents as unreachable from disk.
    pub fn size(&self) -> [i32; 3] {
        match (&self.structure, &self.structure_set) {
            (Some(s), _) => s.size,
            (None, Some(set)) => set.size,
            (None, None) => [0, 0, 0],
        }
    }

    /// The MC data version the piece's templates target (ADR-0009), when it
    /// declares one.
    pub fn data_version(&self) -> Option<i32> {
        match (&self.structure, &self.structure_set) {
            (Some(s), _) => Some(s.data_version),
            (None, Some(set)) => Some(set.data_version),
            (None, None) => None,
        }
    }

    /// True when the piece's blocks arrive as several templates — a fact about
    /// packaging that only a tool reporting on packaging may ask.
    pub fn is_tiled(&self) -> bool {
        self.structure_set.is_some()
    }

    /// The filename stem the piece's files are named from — the single
    /// template's `id`, or the tile set's `base`. They are the same concept
    /// under two keys, so a diagnostic that wants to name the piece's document
    /// asks here rather than reaching into one packaging.
    pub fn base(&self) -> &str {
        match (&self.structure, &self.structure_set) {
            (Some(s), _) => &s.id,
            (None, Some(set)) => &set.base,
            (None, None) => "",
        }
    }

    /// The tile grid `[x, y, z]`; `[1, 1, 1]` for a piece that fit one
    /// template. Packaging, like [`Self::is_tiled`].
    pub fn grid(&self) -> [i32; 3] {
        self.structure_set
            .as_ref()
            .map_or([1, 1, 1], |set| set.grid)
    }

    /// Read the document at `path`, or `Ok(None)` when there is no file there.
    pub fn read(path: &Path) -> Result<Option<PrefabMeta>, String> {
        if !path.exists() {
            return Ok(None);
        }
        let text =
            std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
        PrefabMeta::from_json(&text).map(Some)
    }

    /// Load `<nbt_path>.json` (the sibling metadata), or `Ok(None)` when absent.
    pub fn beside_nbt(nbt_path: &Path) -> Result<Option<PrefabMeta>, String> {
        let json_path = nbt_path.with_extension("json");
        if !json_path.exists() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(&json_path)
            .map_err(|e| format!("read {}: {e}", json_path.display()))?;
        Ok(Some(PrefabMeta::from_json(&text)?))
    }

    /// Serialize as canonical pretty JSON with a trailing newline.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("prefab metadata serializes") + "\n"
    }

    /// Every key of this document — top level and per anchor — that this version
    /// does not model, as `(where, key)` pairs in a stable order.
    ///
    /// `where` is `""` for a top-level key and the anchor's name for an anchor
    /// key. A reader that wants to say something about a key it kept asks here;
    /// nothing has to re-open the file to find out.
    pub fn unknown_keys(&self) -> Vec<(&str, &str)> {
        let mut out: Vec<(&str, &str)> = self
            .extra
            .keys()
            .map(|k| ("", k.as_str()))
            .collect::<Vec<_>>();
        for (name, anchor) in &self.anchors {
            for key in anchor.extra.keys() {
                out.push((name.as_str(), key.as_str()));
            }
        }
        out
    }

    /// A minimal skeleton for a freshly admitted external piece.
    pub fn skeleton(
        id: &str,
        size: [i32; 3],
        data_version: i32,
        generator: &str,
        license: License,
    ) -> PrefabMeta {
        PrefabMeta {
            prefab_id: format!("prefab/{id}"),
            structure: Some(StructureMeta {
                file: format!("{id}.nbt"),
                id: id.to_string(),
                size,
                data_version,
                generator: Some(generator.to_string()),
            }),
            structure_set: None,
            anchors: BTreeMap::new(),
            connectors: Vec::new(),
            lighting: Some(Lighting {
                method: Some("not yet probed".to_string()),
                ..Lighting::unmeasured()
            }),
            license: Some(license),
            waterline_y: None,
            spatial_contract: None,
            // A freshly admitted piece makes no claim about which size class of
            // box it fills, and inventing one from its bytes would be the
            // inference this document does not do: the claim is the author's.
            footprint_class: None,
            extra: BTreeMap::new(),
        }
    }

    /// Annotate a named anchor's **place**, creating the anchor when it is not
    /// there yet.
    ///
    /// An anchor is an object, not a value. A tool that names where the anchor
    /// is has said nothing about the hardware the prefab wired at it
    /// ([`Anchor::dispenser`], [`Anchor::trigger_block`]), about which contract
    /// element an exporter resolved it into ([`Anchor::resolves_to`]), or about
    /// any key this version has never heard of ([`Anchor::extra`]) — so none of
    /// those is touched. Replacing the whole anchor instead is the same silent
    /// deletion this type exists to prevent at the top level, one level down,
    /// and on the block of the document that has grown most often.
    ///
    /// The place itself is one property expressed two ways — a cell or a region
    /// — so an edit redeclares all four of its fields together and a `pos` does
    /// supersede a stale `region`.
    pub fn edit_anchor(&mut self, name: &str, edit: AnchorEdit) {
        let anchor = self.anchors.entry(name.to_string()).or_default();
        anchor.pos = edit.pos;
        anchor.facing = edit.facing;
        anchor.region = edit.region;
        anchor.block = edit.block;
    }

    /// **The ONE authority on the region and block a gate anchor names**, in
    /// piece-local coordinates.
    ///
    /// A gate anchor is declared in one of two forms, and the compiler must read
    /// both identically:
    ///
    /// * **explicitly** — the anchor carries its own [`region`](Anchor::region)
    ///   and [`block`](Anchor::block), which is how a hand-authored piece has
    ///   always written one;
    /// * **through the piece's spatial contract** — the anchor carries a
    ///   [`resolves_to`](Anchor::resolves_to) of `bar:<region>`, which is what an
    ///   exporter writes. The cells and the block already live in that edge's
    ///   [`ContractBar`], so repeating them on the anchor would be a second
    ///   authority for one fact, and the exporter rightly does not.
    ///
    /// Nothing derived either form from the other, and the whole compiler read
    /// only the first. A piece declaring its gates the second way therefore had
    /// no gate at all: the anchor resolved to a bare point, every verb that fills
    /// or clears a gate was refused (`DW0343`), and the information the refusal
    /// asked for was sitting in the same document.
    ///
    /// Both readers ask this function and nothing else — the planner, which
    /// resolves the anchor to world cells, and
    /// [`AnchorRegistry`](crate::AnchorRegistry)'s prefab implementation, which
    /// answers whether the compiler can fill it — so the two cannot come to
    /// disagree about what a gate is or where it stands.
    ///
    /// Three answers, and the third is the one that earns the `Result`:
    ///
    /// * `Ok(None)` — not a gate anchor. A point anchor, a trap anchor, or an
    ///   anchor whose `resolves_to` names a space, a `no_body` region, a `via`
    ///   volume or a `way`. None of those is a thing content fills or clears.
    /// * `Ok(Some(gate))` — a gate, and here is the box and the block.
    /// * `Err(why)` — declared as a gate and **not resolvable to one fillable
    ///   box**. Refused rather than guessed, because every alternative writes
    ///   blocks somewhere the document did not ask for. `why` is a clause the
    ///   caller folds into its own diagnostic.
    ///
    /// A `way` is deliberately not a gate here. Its
    /// [`opens`](ContractWay::opens) is a direction — `laid` cells are absent as
    /// built and appear when opened, `cleared` cells stand and are voided — and
    /// the single-region gate model carries no direction to put it in. Reading
    /// one as a gate would silently pick a direction for the author.
    pub fn gate_anchor(&self, name: &str) -> Result<Option<GateAnchor>, String> {
        let Some(anchor) = self.anchors.get(name) else {
            return Ok(None);
        };
        let contract_bar = match anchor.resolves_to.as_deref().and_then(bar_name) {
            Some(region) => Some(self.contract_bar(name, region)?),
            None => None,
        };
        match (&anchor.region, contract_bar) {
            (None, None) => Ok(None),
            // The contract form. The block is the contract's, which is the block
            // the piece was actually built out of.
            (None, Some(bar)) => Ok(Some(bar)),
            // The explicit form, unchanged since it was the only one. A region
            // with no `block` is what `DW0343` has always refused: the compiler
            // is being asked to fill cells with a block nothing names.
            (Some(region), None) => match &anchor.block {
                Some(block) => Ok(Some(GateAnchor {
                    from: region.from,
                    to: region.to,
                    block: block.clone(),
                })),
                None => Err(format!(
                    "gate anchor `{name}` declares a `region` and no `block`, so nothing says \
                     what the region is filled with"
                )),
            },
            // Both forms. Agreement is fine and is what a piece that was
            // hand-authored and later exported looks like; a disagreement is
            // refused rather than resolved by precedence, because whichever one
            // this function preferred would be a rule no reader of the document
            // could see.
            (Some(region), Some(bar)) => {
                let explicit = GateAnchor {
                    from: region.from,
                    to: region.to,
                    block: anchor.block.clone().unwrap_or_default(),
                };
                if explicit == bar {
                    return Ok(Some(bar));
                }
                Err(format!(
                    "gate anchor `{name}` declares a `region` and also resolves into contract bar \
                     `{bar_region}`, and the two disagree — the anchor says {ea} filled with \
                     `{eb}`, the bar says {ba} filled with `{bb}`. One place stands in one way: \
                     delete the anchor's `region`/`block` and let the contract say it, or correct \
                     the contract",
                    bar_region = anchor
                        .resolves_to
                        .as_deref()
                        .and_then(bar_name)
                        .unwrap_or_default(),
                    ea = explicit.extent(),
                    eb = explicit.block,
                    ba = bar.extent(),
                    bb = bar.block,
                ))
            }
        }
    }

    /// The bar `region` of this piece's spatial contract, as one fillable box.
    fn contract_bar(&self, anchor: &str, region: &str) -> Result<GateAnchor, String> {
        let bar = self
            .spatial_contract
            .as_ref()
            .into_iter()
            .flat_map(|c| c.edges.iter())
            .filter_map(|e| e.bar.as_ref())
            .find(|b| b.region == region)
            .ok_or_else(|| {
                format!(
                    "gate anchor `{anchor}` resolves into contract bar `{region}`, and this \
                     piece's spatial contract declares no bar of that name — the anchor's \
                     `resolves_to` is written from the contract, so the two have come apart. \
                     Re-export the piece"
                )
            })?;
        let one = one_box(&bar.boxes).ok_or_else(|| {
            format!(
                "gate anchor `{anchor}` resolves into contract bar `{region}`, whose {n} boxes do \
                 not fill their own bounding box — so there is no single region the compiler can \
                 fill or clear without writing blocks into cells the contract does not call bar. \
                 Declare the bar as one box, or as boxes that tile one",
                n = bar.boxes.len()
            )
        })?;
        Ok(GateAnchor {
            from: one.from,
            to: one.to,
            block: bar.block.clone(),
        })
    }

    /// Append a socket connector (idempotent by `local_pos` + `facing`).
    pub fn add_connector(&mut self, c: Connector) {
        if !self
            .connectors
            .iter()
            .any(|x| x.local_pos == c.local_pos && x.facing == c.facing)
        {
            self.connectors.push(c);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole reason this type is not two types: an editing tool reads a
    /// document, changes one block of it, and writes it back. Anything it does
    /// not model is deleted, and nothing says so.
    #[test]
    fn a_read_modify_write_round_trip_keeps_every_field() {
        let text = r#"{
  "prefab_id": "prefab/chapel-ward",
  "structure": {
    "file": "chapel-ward.nbt",
    "id": "chapel-ward",
    "size": [16, 9, 26],
    "data_version": 4671,
    "generator": "crates/grammar"
  },
  "anchors": {
    "anchor/bell": { "pos": [3, 1, 4], "facing": "north" },
    "anchor/ward": { "region": { "from": [0, 0, 0], "to": [2, 2, 2] }, "block": "minecraft:stone" }
  },
  "connectors": [],
  "lighting": { "profile": "unmeasured" },
  "license": {
    "source": "original",
    "spdx": "GPL-3.0-or-later",
    "note": "n",
    "provenance": "p",
    "generated_by": {
      "generator": "grammar",
      "program": "bell_chapel_ward",
      "program_hash": "sha256:00",
      "seed": 1
    }
  },
  "waterline_y": 2
}
"#;
        let mut meta = PrefabMeta::from_json(text).unwrap();
        meta.lighting = Some(Lighting {
            profile: crate::registry::LightingProfile::Dark,
            measured_min_light: Some(0),
            measured: Some("2026-08-11".to_string()),
            rationale: None,
            method: Some("static estimate".to_string()),
        });
        let after: serde_json::Value = serde_json::from_str(&meta.to_json()).unwrap();
        let before: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(
            after["license"]["generated_by"], before["license"]["generated_by"],
            "the provenance row must survive an edit to an unrelated block"
        );
        assert_eq!(after["anchors"], before["anchors"]);
        assert_eq!(after["structure"], before["structure"]);
        assert_eq!(
            after["waterline_y"], before["waterline_y"],
            "a declared waterline must survive an edit to an unrelated block"
        );
    }

    /// The same guarantee for a key no version of this type has ever heard of.
    /// This is the general form of the `waterline_y` and `generated_by` losses:
    /// the type cannot enumerate what has not been invented, so it keeps it.
    #[test]
    fn a_key_this_version_does_not_model_survives_the_round_trip() {
        let text = r#"{
  "prefab_id": "prefab/x",
  "structure": { "file": "x.nbt", "id": "x", "size": [3, 3, 3], "data_version": 4671 },
  "anchors": { "anchor/a": { "pos": [1, 1, 1], "acoustics": "reverberant" } },
  "connectors": [],
  "lighting": { "profile": "unmeasured" },
  "from_the_future": { "nested": [1, 2, 3] }
}
"#;
        let mut meta = PrefabMeta::from_json(text).unwrap();
        assert_eq!(
            meta.unknown_keys(),
            vec![("", "from_the_future"), ("anchor/a", "acoustics")],
            "both unknown keys must be reportable, top level and per anchor"
        );
        meta.connectors.push(Connector {
            name: "keep:socket".to_string(),
            target: "keep:socket".to_string(),
            local_pos: [0, 0, 0],
            facing: "north".to_string(),
            opening: [3, 3],
            joint: "aligned".to_string(),
        });
        let after: serde_json::Value = serde_json::from_str(&meta.to_json()).unwrap();
        let before: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(after["from_the_future"], before["from_the_future"]);
        assert_eq!(
            after["anchors"]["anchor/a"]["acoustics"],
            before["anchors"]["anchor/a"]["acoustics"]
        );
    }

    /// The same guarantee **inside** an anchor, which is where the document's
    /// round trip is finest-grained and where the top-level guarantee above says
    /// nothing at all.
    ///
    /// An editing step that re-annotates an anchor already on the piece names
    /// only where it is. The dispenser cell the prefab wired, the trigger block
    /// it must put back, the contract element the exporter resolved, and a key
    /// no version has heard of are all properties of the anchor and not of the
    /// edit — so all four survive, and only the place changes.
    #[test]
    fn re_annotating_an_anchor_keeps_the_hardware_the_piece_carries() {
        let text = r#"{
  "prefab_id": "prefab/trap-room",
  "structure": { "file": "trap-room.nbt", "id": "trap-room", "size": [7, 5, 7], "data_version": 4671 },
  "anchors": {
    "anchor/trap": {
      "pos": [3, 1, 3],
      "facing": "north",
      "resolves_to": "space:hall",
      "dispenser": [3, 2, 4],
      "trigger_block": "minecraft:oak_pressure_plate[powered=false]",
      "acoustics": "reverberant"
    }
  },
  "connectors": [],
  "lighting": { "profile": "unmeasured" }
}
"#;
        let mut meta = PrefabMeta::from_json(text).unwrap();
        meta.edit_anchor(
            "anchor/trap",
            AnchorEdit {
                pos: Some([4, 1, 3]),
                facing: Some("south".to_string()),
                ..AnchorEdit::default()
            },
        );
        let after: serde_json::Value = serde_json::from_str(&meta.to_json()).unwrap();
        let a = &after["anchors"]["anchor/trap"];
        assert_eq!(
            a["pos"],
            serde_json::json!([4, 1, 3]),
            "the place is edited"
        );
        assert_eq!(a["facing"], serde_json::json!("south"));
        assert_eq!(
            a["dispenser"],
            serde_json::json!([3, 2, 4]),
            "the pre-wired dispenser cell is the piece's hardware, not the edit's"
        );
        assert_eq!(
            a["trigger_block"],
            serde_json::json!("minecraft:oak_pressure_plate[powered=false]"),
            "flag-gating a trap has to put this exact block back"
        );
        assert_eq!(a["resolves_to"], serde_json::json!("space:hall"));
        assert_eq!(
            a["acoustics"],
            serde_json::json!("reverberant"),
            "a key this version does not model is the anchor's too"
        );

        // A gate anchor's region and a point anchor's cell are one property, so
        // naming the cell supersedes the region rather than leaving both.
        meta.edit_anchor(
            "anchor/gate",
            AnchorEdit {
                region: Some(Region {
                    from: [0, 0, 0],
                    to: [1, 2, 0],
                }),
                block: Some("minecraft:iron_bars".to_string()),
                ..AnchorEdit::default()
            },
        );
        meta.edit_anchor(
            "anchor/gate",
            AnchorEdit {
                pos: Some([0, 1, 0]),
                ..AnchorEdit::default()
            },
        );
        let after: serde_json::Value = serde_json::from_str(&meta.to_json()).unwrap();
        let g = &after["anchors"]["anchor/gate"];
        assert_eq!(g["pos"], serde_json::json!([0, 1, 0]));
        assert!(g.get("region").is_none(), "{g}");
        assert!(g.get("block").is_none(), "{g}");
    }

    /// A tiled zone's manifest is a prefab document like any other: it is read,
    /// one block of it is edited, and it is written back whole — and the keys
    /// this version does not model survive.
    ///
    /// It used to be a *second type* (`TileSetMeta`), field-for-field this one.
    /// The copy is what this test is really about: the same round trip, on the
    /// same struct, is what makes a block added here reach both packagings.
    #[test]
    fn a_tile_set_manifest_round_trips_through_an_edit() {
        let text = r#"{
  "prefab_id": "prefab/notre-dame",
  "structure_set": {
    "base": "notre-dame",
    "size": [31, 48, 93],
    "part_max": 48,
    "grid": [1, 1, 2],
    "data_version": 4671,
    "generator": "crates/grammar",
    "parts": [
      { "file": "notre-dame.x0y0z0.nbt", "id": "a", "grid_index": [0,0,0], "offset": [0,0,0], "size": [31,48,48] },
      { "file": "notre-dame.x0y0z1.nbt", "id": "b", "grid_index": [0,0,1], "offset": [0,0,48], "size": [31,48,45] }
    ]
  },
  "anchors": { "anchor/crossing": { "pos": [15, 1, 56], "facing": "south" } },
  "connectors": [],
  "lighting": { "profile": "unmeasured" },
  "waterline_y": 12,
  "license": {
    "source": "original",
    "spdx": "GPL-3.0-or-later",
    "note": "n",
    "provenance": "p",
    "generated_by": { "generator": "grammar", "program": "nd", "program_hash": "sha256:00", "seed": 1 }
  },
  "a_key_no_engine_models": { "kept": true }
}
"#;
        let mut meta = PrefabMeta::from_json(text).unwrap();
        assert!(meta.is_tiled());
        assert_eq!(meta.size(), [31, 48, 93]);
        assert_eq!(meta.data_version(), Some(4671));
        assert_eq!(meta.license.as_ref().unwrap().spdx, "GPL-3.0-or-later");
        // The whole point of one document: a block the copy had lost is here.
        assert_eq!(meta.waterline_y, Some(12));

        // The templates, in grid order, with their piece-local offsets.
        let templates = meta.templates();
        assert_eq!(templates.len(), 2);
        assert_eq!(templates[0].file, "notre-dame.x0y0z0.nbt");
        assert_eq!(templates[0].offset, [0, 0, 0]);
        assert_eq!(templates[1].id, "b");
        assert_eq!(templates[1].offset, [0, 0, 48]);
        assert_eq!(templates[1].size, [31, 48, 45]);

        meta.lighting = Some(Lighting {
            profile: crate::registry::LightingProfile::Lit,
            measured_min_light: Some(6),
            measured: Some(String::new()),
            rationale: None,
            method: Some("static estimate".to_string()),
        });
        let after: serde_json::Value = serde_json::from_str(&meta.to_json()).unwrap();
        let before: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(after["license"], before["license"]);
        assert_eq!(after["structure_set"], before["structure_set"]);
        assert_eq!(after["anchors"], before["anchors"]);
        assert_eq!(after["waterline_y"], before["waterline_y"]);
        assert_eq!(after["lighting"]["profile"], "lit");
        assert!(
            after.get("structure").is_none(),
            "a tiled document must not grow an empty `structure` key: {after}"
        );
        // Reading is total here too: a key this version has never heard of
        // survives an edit rather than being deleted by the tool that made it.
        assert_eq!(
            after["a_key_no_engine_models"], before["a_key_no_engine_models"],
            "an unmodelled key must survive a read-modify-write"
        );
    }

    /// A single-template piece is one template at the origin, so nothing that
    /// places blocks has to ask which packaging it was handed.
    #[test]
    fn a_single_template_piece_is_one_template_at_the_origin() {
        let text = r#"{
  "prefab_id": "prefab/x",
  "structure": { "file": "x.nbt", "id": "x", "size": [3, 4, 5], "data_version": 4671 }
}
"#;
        let meta = PrefabMeta::from_json(text).unwrap();
        assert!(!meta.is_tiled());
        assert_eq!(meta.size(), [3, 4, 5]);
        assert_eq!(
            meta.templates(),
            vec![PieceTemplate {
                id: "x",
                file: "x.nbt",
                offset: [0, 0, 0],
                size: [3, 4, 5],
            }]
        );
    }

    /// "Which shape is this" has exactly two answers and no third: a document
    /// with neither block, and one with both, are refusals rather than a
    /// half-read document handed to a step that places some of its blocks.
    #[test]
    fn a_document_that_does_not_say_what_blocks_it_describes_is_refused() {
        let err = PrefabMeta::from_json(r#"{"prefab_id":"prefab/x"}"#).unwrap_err();
        assert!(err.contains("structure_set"), "{err}");
        assert!(err.contains("structure"), "{err}");

        let both = r#"{
  "prefab_id": "prefab/x",
  "structure": { "file": "x.nbt", "id": "x", "size": [3, 3, 3], "data_version": 4671 },
  "structure_set": {
    "base": "x", "size": [3, 3, 3], "part_max": 48, "grid": [1, 1, 1],
    "data_version": 4671, "generator": "g",
    "parts": [ { "file": "x.x0y0z0.nbt", "id": "x0", "grid_index": [0,0,0], "offset": [0,0,0], "size": [3,3,3] } ]
  }
}
"#;
        let err = PrefabMeta::from_json(both).unwrap_err();
        assert!(err.contains("BOTH"), "{err}");
    }

    /// A manifest that does not tile its own zone is refused **by the reader**,
    /// so every consumer of the document meets it at the same place — and none
    /// of them reassembles a building with a hole in it and reports success.
    #[test]
    fn a_manifest_that_does_not_tile_its_zone_is_refused_by_the_reader() {
        let text = r#"{
  "prefab_id": "prefab/holed",
  "structure_set": {
    "base": "holed", "size": [4, 4, 100], "part_max": 48, "grid": [1, 1, 1],
    "data_version": 4671, "generator": "g",
    "parts": [ { "file": "holed.x0y0z0.nbt", "id": "h0", "grid_index": [0,0,0], "offset": [0,0,0], "size": [4,4,48] } ]
  }
}
"#;
        let err = PrefabMeta::from_json(text).unwrap_err();
        assert!(err.contains("cover"), "{err}");
        assert!(err.contains("hole"), "{err}");
    }

    /// A piece nothing has regenerated has no row, and the key is absent rather
    /// than `null` — `null` reads as "measured, and the answer is nothing".
    #[test]
    fn absent_optional_fields_are_omitted_not_nulled() {
        let meta = PrefabMeta::skeleton(
            "ingested",
            [3, 3, 3],
            4671,
            "delve-admit (external admission)",
            License {
                source: "unknown".to_string(),
                spdx: "UNKNOWN".to_string(),
                note: String::new(),
                provenance: String::new(),
                generated_by: None,
            },
        );
        let json = meta.to_json();
        assert!(!json.contains("generated_by"), "{json}");
        assert!(!json.contains("null"), "{json}");
        assert!(json.contains("\"connectors\": []"), "{json}");
        assert_eq!(PrefabMeta::from_json(&json).unwrap(), meta);
    }
}
