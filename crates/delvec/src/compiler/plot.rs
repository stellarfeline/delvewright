//! **A plot is built as it was planned** (spec-0125 §5): the parts a node
//! declares in `holds[]`, bound by the piece's contract with `part`, measured
//! and judged.
//!
//! # What is measured
//!
//! A part's **built play space** is the union of the xz projections of every
//! contract space and out-of-walk region that names it, counted in columns
//! (§5.2). The measurement reads the piece's own declarations — the contract
//! the export wrote into its metadata — which the contract gates prove against
//! the bytes; nothing here reads a wall off a block. The ledger's ridge and
//! roofed share are read off the piece's own blocks, because they are facts
//! about what stands over the part's columns.
//!
//! # What is refused — `DW1074`, one code, four shapes (§5.3)
//!
//! 1. *unbuilt*: a declared part no space and no region names;
//! 2. *undeclared*: a `part` naming an id the node does not declare, or any
//!    `part` on a piece whose node declares no `holds[]`;
//! 3. *not walked*: a `reached` part none of whose columns holds a cell the
//!    piece's own flood reaches ([`crate::grammar::contract::walked_cells`]);
//! 4. *out of size*: a part whose built play space is under three quarters or
//!    over five quarters of its declared `cells`.
//!
//! Shapes 1, 2 and 4 read only the piece's metadata and are judged by
//! [`crate::compiler::detail::check`], the funnel every verb's validation goes
//! through, `delvec detail --check` and `build` included. Shape 3 needs the
//! piece's blocks: `delvec detail` judges it over the expansion, and the build
//! judges it again over the piece's shipped templates ([`ledger`]). All four
//! go through [`judge`], one implementation.
//!
//! # What is stated and never refused (§4.3)
//!
//! `validation/plot-program.json`: per plot the coverage, programmed share and
//! remainder; per structure part its setbacks, gaps and ridge; per part its
//! roofed share and each bound region with an enclosed space's clearance. A
//! plot outside the community's bands is a finding at the plan, never a
//! refusal.

use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::layout::{Node, Part, PartKind};
use delvewright_dsl::prefab::{Region, SpatialContract};
use delvewright_dsl::{Campaign, Diagnostic, DwCode, ExitTier, NodeId};
use serde_json::{Value, json};

use crate::compiler::plan::Plan;
use crate::compiler::registry::PrefabRegistry;
use crate::grammar::model::VoxelModel;

delvewright_dsl::dw_code! {
    /// `DW1074`: **a part not built as declared** (spec-0125 §5.3). A plot's
    /// declared part is bound by no contract space or region, a `part` names
    /// an id the node does not declare, a walked part holds no cell the
    /// piece's own flood reaches, or a part's built play space is outside a
    /// quarter of its declared `cells`. Detail tier.
    pub const DW_PART_NOT_BUILT: DwCode = DwCode::new("DW1074", ExitTier::Build);
}

/// The stage every diagnostic here carries.
const STAGE: &str = "detail-plan";

/// One contract space or out-of-walk region bound to a part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundRegion {
    /// The region's name in the contract.
    pub name: String,
    /// `enclosed` | `open_top` | `open` for a space; `no_body` for an
    /// out-of-walk region.
    pub envelope: String,
    /// Its piece-local cells.
    pub cells: BTreeSet<[i32; 3]>,
}

impl BoundRegion {
    /// Courses from its lowest cell to its highest — an enclosed space's
    /// clearance (`place-craft.md` §3).
    #[must_use]
    pub fn clearance(&self) -> i32 {
        let lo = self.cells.iter().map(|c| c[1]).min().unwrap_or(0);
        let hi = self.cells.iter().map(|c| c[1]).max().unwrap_or(-1);
        hi - lo + 1
    }
}

/// One declared part and what the piece binds to it.
#[derive(Debug, Clone)]
pub struct PartBuilt<'a> {
    /// The declaration.
    pub part: &'a Part,
    /// Every region naming it.
    pub regions: Vec<BoundRegion>,
    /// The union of their xz projections, piece-local `[x, z]`.
    pub columns: BTreeSet<[i32; 2]>,
}

impl PartBuilt<'_> {
    /// The built play space, in cells.
    #[must_use]
    pub fn built(&self) -> usize {
        self.columns.len()
    }

    /// Whether the built play space is within a quarter of the declared
    /// `cells` (§5.3 shape 4): inside [`size_band`].
    #[must_use]
    pub fn in_size(&self) -> bool {
        let (lo, hi) = size_band(self.part.cells);
        (lo..=hi).contains(&(self.built() as u64))
    }
}

/// **The built play space a part of `cells` declared cells may have** (§5.3
/// shape 4), inclusive: three quarters to five quarters of the declaration,
/// in whole columns. The one answer `DW1074` judges by, the allocation hands
/// each part (`bands`) and `delvec detail` states per part.
#[must_use]
pub fn size_band(cells: u32) -> (u64, u64) {
    let d = u64::from(cells);
    ((3 * d).div_ceil(4), 5 * d / 4)
}

impl Binding<'_> {
    /// `sizes: part/a 35 of 24..40, …` — each bound part's built play space
    /// against its band, the parts outside it marked.
    #[must_use]
    pub fn sizes_line(&self) -> String {
        let parts: Vec<String> = self
            .parts
            .iter()
            .filter(|p| !p.regions.is_empty())
            .map(|p| {
                let (lo, hi) = size_band(p.part.cells);
                format!(
                    "`{}` {} column(s) of {lo}..{hi}{}",
                    p.part.id,
                    p.built(),
                    if p.in_size() { "" } else { " (OUT)" }
                )
            })
            .collect();
        format!(
            "part sizes (built play space of the band DW1074 allows): {}",
            if parts.is_empty() {
                "none bound".to_string()
            } else {
                parts.join(", ")
            }
        )
    }
}

/// A plot's parts, bound from a piece's contract.
#[derive(Debug, Clone)]
pub struct Binding<'a> {
    /// The place.
    pub node: &'a NodeId,
    /// Every declared part, in declaration order.
    pub parts: Vec<PartBuilt<'a>>,
    /// `(region, part)` for every `part` the node does not declare.
    pub undeclared: Vec<(String, String)>,
}

fn region_cells(boxes: &[Region]) -> BTreeSet<[i32; 3]> {
    let mut out = BTreeSet::new();
    for r in boxes {
        for x in r.from[0]..=r.to[0] {
            for y in r.from[1]..=r.to[1] {
                for z in r.from[2]..=r.to[2] {
                    out.insert([x, y, z]);
                }
            }
        }
    }
    out
}

/// **Bind a node's parts from a piece's contract** — every space and region
/// naming each declared part, and every `part` naming none.
#[must_use]
pub fn bind<'a>(node: &'a NodeId, holds: &'a [Part], contract: &SpatialContract) -> Binding<'a> {
    let mut regions: Vec<(String, String, Option<&String>, &[Region])> = Vec::new();
    for (name, s) in &contract.spaces {
        regions.push((name.clone(), s.envelope.clone(), s.part.as_ref(), &s.boxes));
    }
    for (name, r) in &contract.no_body {
        regions.push((
            name.clone(),
            "no_body".to_string(),
            r.part.as_ref(),
            &r.boxes,
        ));
    }
    let declared: BTreeSet<&str> = holds.iter().map(|p| p.id.0.as_str()).collect();
    let mut undeclared = Vec::new();
    for (name, _, part, _) in &regions {
        if let Some(part) = part.filter(|p| !declared.contains(p.as_str())) {
            undeclared.push((name.clone(), part.clone()));
        }
    }
    let parts = holds
        .iter()
        .map(|p| {
            let bound: Vec<BoundRegion> = regions
                .iter()
                .filter(|(_, _, part, _)| part.is_some_and(|x| *x == p.id.0))
                .map(|(name, envelope, _, boxes)| BoundRegion {
                    name: name.clone(),
                    envelope: envelope.clone(),
                    cells: region_cells(boxes),
                })
                .collect();
            let columns = bound
                .iter()
                .flat_map(|r| r.cells.iter().map(|c| [c[0], c[2]]))
                .collect();
            PartBuilt {
                part: p,
                regions: bound,
                columns,
            }
        })
        .collect();
    Binding {
        node,
        parts,
        undeclared,
    }
}

/// What a piece binds of its plot: `parts: B of K bound, W of R walked`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PartsCount {
    /// Declared parts some region names.
    pub bound: usize,
    /// Declared parts.
    pub parts: usize,
    /// Of the walked parts, those the flood reaches.
    pub walked: usize,
    /// Parts declared walked (`reached`, the default).
    pub reached: usize,
}

impl PartsCount {
    /// `parts: B of K bound, W of R walked`.
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "parts: {} of {} bound, {} of {} walked",
            self.bound, self.parts, self.walked, self.reached
        )
    }
}

/// Whether a part's columns hold a cell of `walked`.
fn is_walked(p: &PartBuilt<'_>, walked: &BTreeSet<[i32; 3]>) -> bool {
    walked.iter().any(|c| p.columns.contains(&[c[0], c[2]]))
}

/// **`DW1074` over one piece** (§5.3). `walked` is the piece's own flood,
/// piece-local, when the caller holds its blocks; `None` judges shapes 1, 2
/// and 4 and leaves the walk to a caller that does. The message names the
/// part, its use, the shape and the numbers, and both remedies.
#[must_use]
pub fn judge(
    b: &Binding<'_>,
    piece: &str,
    walked: Option<&BTreeSet<[i32; 3]>>,
) -> (Vec<Diagnostic>, PartsCount) {
    let mut d = Vec::new();
    let mut n = PartsCount {
        parts: b.parts.len(),
        ..PartsCount::default()
    };
    let node = b.node;
    let at = format!("/content/details/{node}");
    for (region, part) in &b.undeclared {
        let why = if b.parts.is_empty() {
            format!("`{node}` declares no `holds[]`, so it is not a plot and has no part to name")
        } else {
            format!(
                "`{node}` declares {} part(s) — {} — and none of them is `{part}`",
                b.parts.len(),
                b.parts
                    .iter()
                    .map(|p| format!("`{}`", p.part.id))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        d.push(Diagnostic::error(
            DW_PART_NOT_BUILT,
            STAGE,
            at.clone(),
            format!(
                "undeclared: `{piece}`'s contract region {region:?} names the part `{part}`, and \
                 {why}. Rename the part on the piece to one the plan declares, or drop it; or, \
                 when the plan is what is wrong, declare the part in the node's `holds[]`."
            ),
        ));
    }
    for p in &b.parts {
        let (id, use_) = (&p.part.id, &p.part.r#use);
        if p.regions.is_empty() {
            d.push(Diagnostic::error(
                DW_PART_NOT_BUILT,
                STAGE,
                at.clone(),
                format!(
                    "unbuilt: `{node}` declares the part `{id}` ({use_}, {} cell(s)) and no space \
                     or out-of-walk region of `{piece}`'s contract names it with `part`. Name \
                     the spaces and regions that are it; or, when the plan is what is wrong, \
                     remove the part from `holds[]`.",
                    p.part.cells
                ),
            ));
            continue;
        }
        n.bound += 1;
        if p.part.reached {
            n.reached += 1;
        }
        if let Some(walked) = walked
            && p.part.reached
        {
            if is_walked(p, walked) {
                n.walked += 1;
            } else {
                d.push(Diagnostic::error(
                    DW_PART_NOT_BUILT,
                    STAGE,
                    at.clone(),
                    format!(
                        "not walked: `{node}`'s part `{id}` ({use_}) is walked by declaration and \
                         none of its {} column(s) — bound by {} — holds a cell `{piece}`'s own \
                         flood reaches from its entry. Open a way into it on the piece; or, when \
                         the plan is what is wrong, declare the part `reached: false`.",
                        p.built(),
                        p.regions
                            .iter()
                            .map(|r| format!("{:?}", r.name))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                ));
            }
        }
        if !p.in_size() {
            d.push(Diagnostic::error(
                DW_PART_NOT_BUILT,
                STAGE,
                at.clone(),
                format!(
                    "out of size: `{node}`'s part `{id}` ({use_}) declares {} cell(s) and \
                     `{piece}` builds {} — its regions claim {} column(s), outside a quarter of \
                     the declaration ({} to {}). Resize the part's spaces on the piece; or, when \
                     the plan is what is wrong, change the part's `cells`.",
                    p.part.cells,
                    p.built(),
                    p.built(),
                    size_band(p.part.cells).0,
                    size_band(p.part.cells).1,
                ),
            ));
        }
    }
    if walked.is_none() {
        n.walked = 0;
    }
    (d, n)
}

/// **The parts `delvec detail` hands the expansion** (spec-0125 §5.1): the
/// node's `holds[]`, read from the layout graph — a derivation, never typed.
/// Empty for a place that is not a plot, or a campaign with no graph.
#[must_use]
pub fn handed_parts(c: &Campaign, node: &NodeId) -> Vec<Part> {
    c.layout_graph
        .as_ref()
        .and_then(|g| g.content.nodes.iter().find(|n| n.id == *node))
        .map(|n| n.holds.clone())
        .unwrap_or_default()
}

/// The node a bound piece's contract is judged against: the graph's node of
/// that place, or `None` when the campaign has no graph.
fn node_of<'a>(c: &'a Campaign, node: &NodeId) -> Option<&'a Node> {
    c.layout_graph
        .as_ref()
        .and_then(|g| g.content.nodes.iter().find(|n| n.id == *node))
}

/// **`DW1074` over metadata** — shapes 1, 2 and 4 for every bound piece whose
/// node is a plot or whose contract names a part. What
/// [`crate::compiler::detail::check`] calls; returns the diagnostics and the
/// number of pieces judged.
#[must_use]
pub fn check_bound(c: &Campaign, prefabs: &PrefabRegistry) -> (Vec<Diagnostic>, usize) {
    let mut d = Vec::new();
    let mut judged = 0usize;
    let Some(plan) = c.detail_plan.as_ref() else {
        return (d, judged);
    };
    for row in &plan.content.details {
        let Some(node) = node_of(c, &row.place) else {
            continue;
        };
        let Some(contract) = prefabs
            .get(row.piece.as_str())
            .and_then(|m| m.spatial_contract.as_ref())
        else {
            continue;
        };
        let names_a_part = contract.spaces.values().any(|s| s.part.is_some())
            || contract.no_body.values().any(|r| r.part.is_some());
        if node.holds.is_empty() && !names_a_part {
            continue;
        }
        judged += 1;
        let (found, _) = judge(
            &bind(&node.id, &node.holds, contract),
            row.piece.as_str(),
            None,
        );
        d.extend(found);
    }
    (d, judged)
}

/// One part's entry in the ledger.
#[derive(Debug, Clone, PartialEq)]
pub struct PartEntry {
    /// `part/<id>`.
    pub id: String,
    /// `structure` | `ground`.
    pub kind: &'static str,
    /// The author's words.
    pub use_: String,
    /// Walked by declaration.
    pub reached: bool,
    /// Declared cells.
    pub declared: u32,
    /// Built columns.
    pub built: usize,
    /// Declared height, on a structure.
    pub height: Option<u32>,
    /// The highest cell a body cannot be in over its columns, in courses over
    /// the walk plane; `None` when none stands over it.
    pub ridge: Option<i32>,
    /// The fraction of its columns with a cell a body cannot be in above head
    /// height, within the box's air.
    pub roofed_share: f64,
    /// Per side (`east`, `north`, `south`, `west`): play-space columns between
    /// its built columns and the box's edge. Structures only.
    pub setbacks: Option<[i32; 4]>,
    /// To each other structure part: columns strictly between the two.
    pub gaps: Vec<(String, i32)>,
    /// Each bound region: name, envelope and, when enclosed, its clearance.
    pub regions: Vec<(String, String, Option<i32>)>,
    /// Piece-local centre of its columns at the walk plane, for the frame.
    pub centre: Option<[f64; 2]>,
}

/// One plot's entry in the ledger.
#[derive(Debug, Clone, PartialEq)]
pub struct PlotEntry {
    /// The place.
    pub node: String,
    /// The box's `extent`.
    pub extent: [i64; 2],
    /// The parts, declared and built.
    pub parts: Vec<PartEntry>,
    /// `None` while the place is massed.
    pub built: Option<PlotShares>,
}

/// A built plot's shares of its play space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlotShares {
    /// Structure parts' built cells over the play space.
    pub coverage: f64,
    /// Every part's built cells over the play space.
    pub programmed: f64,
    /// Play space no part claims, in cells.
    pub remainder: usize,
    /// And as a share.
    pub remainder_share: f64,
}

/// `validation/plot-program.json` and its line.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Ledger {
    /// One entry per plot, in graph order.
    pub plots: Vec<PlotEntry>,
}

fn pct(v: f64) -> i64 {
    (v * 100.0).round() as i64
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

impl Ledger {
    /// `plot program: P plot(s), K part(s) built B of K; coverage min–max %,
    /// remainder min–max %`.
    #[must_use]
    pub fn line(&self) -> String {
        let parts: usize = self.plots.iter().map(|p| p.parts.len()).sum();
        let built: usize = self
            .plots
            .iter()
            .flat_map(|p| p.parts.iter())
            .filter(|p| p.built > 0)
            .count();
        let shares: Vec<PlotShares> = self.plots.iter().filter_map(|p| p.built).collect();
        let range = |f: fn(&PlotShares) -> f64| -> String {
            if shares.is_empty() {
                return "none built".to_string();
            }
            let lo = shares.iter().map(|s| pct(f(s))).min().unwrap_or(0);
            let hi = shares.iter().map(|s| pct(f(s))).max().unwrap_or(0);
            format!("{lo}–{hi} %")
        };
        format!(
            "plot program: {} plot(s), {parts} part(s) built {built} of {parts}; coverage {}, \
             remainder {}",
            self.plots.len(),
            range(|s| s.coverage),
            range(|s| s.remainder_share),
        )
    }

    /// The ledger's JSON.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let side = ["east", "north", "south", "west"];
        json!({
            "line": self.line(),
            "plots": self.plots.iter().map(|p| {
                json!({
                    "node": p.node,
                    "extent": p.extent,
                    "play_space": p.extent[0] * p.extent[1],
                    "built": p.built.is_some(),
                    "reason": if p.built.is_none() { Value::String("massed".into()) } else { Value::Null },
                    "coverage": p.built.map(|s| round3(s.coverage)),
                    "programmed_share": p.built.map(|s| round3(s.programmed)),
                    "remainder": p.built.map(|s| s.remainder),
                    "remainder_share": p.built.map(|s| round3(s.remainder_share)),
                    "parts": p.parts.iter().map(|x| json!({
                        "id": x.id,
                        "kind": x.kind,
                        "use": x.use_,
                        "reached": x.reached,
                        "declared": x.declared,
                        "built": p.built.map(|_| x.built),
                        "height": x.height,
                        "ridge": x.ridge,
                        "roofed_share": p.built.map(|_| round3(x.roofed_share)),
                        "setbacks": x.setbacks.map(|s| {
                            side.iter().zip(s).map(|(k, v)| ((*k).to_string(), json!(v))).collect::<serde_json::Map<_, _>>()
                        }),
                        "gaps": x.gaps.iter().map(|(o, g)| json!({"to": o, "gap": g})).collect::<Vec<_>>(),
                        "regions": x.regions.iter().map(|(n, e, c)| json!({"name": n, "envelope": e, "clearance": c})).collect::<Vec<_>>(),
                    })).collect::<Vec<_>>(),
                })
            }).collect::<Vec<_>>(),
        })
    }
}

/// A bound plot's piece, located: the placement and its contract.
struct Located<'p> {
    pos: [i32; 3],
    size: [i32; 3],
    templates: Vec<([i32; 3], &'p str)>,
    contract: &'p SpatialContract,
    prefab: String,
}

fn locate<'p>(
    plan: &'p Plan,
    prefabs: &'p PrefabRegistry,
    node: &NodeId,
) -> Option<(Located<'p>, &'p delvewright_dsl::siteplan::PlacedBox)> {
    let b = plan.blockout.as_ref()?;
    let pb = b.boxes.iter().find(|x| x.node == *node)?;
    let piece = plan
        .campaign
        .detail_plan
        .as_ref()?
        .content
        .details
        .iter()
        .find(|r| r.place == *node)?
        .piece
        .as_str();
    let (slo, shi) = pb.space();
    let p = plan.areas.iter().flat_map(|a| a.pieces.iter()).find(|p| {
        let (min, max) = p.bbox();
        p.prefab_id == piece
            && (0..3).all(|i| i64::from(min[i]) <= slo[i] && shi[i] <= i64::from(max[i]))
    })?;
    let contract = prefabs.get(piece)?.spatial_contract.as_ref()?;
    Some((
        Located {
            pos: p.pos,
            size: p.size,
            templates: p
                .templates
                .iter()
                .map(|t| {
                    (
                        [
                            t.pos[0] - p.pos[0],
                            t.pos[1] - p.pos[1],
                            t.pos[2] - p.pos[2],
                        ],
                        t.structure_file.as_str(),
                    )
                })
                .collect(),
            contract,
            prefab: piece.to_string(),
        },
        pb,
    ))
}

fn centre_of(cols: &BTreeSet<[i32; 2]>) -> Option<[f64; 2]> {
    if cols.is_empty() {
        return None;
    }
    let n = cols.len() as f64;
    let x = cols.iter().map(|c| f64::from(c[0])).sum::<f64>() / n;
    let z = cols.iter().map(|c| f64::from(c[1])).sum::<f64>() / n;
    Some([x, z])
}

/// **The ledger over a built world, and shape 3 of `DW1074` judged again**
/// over each bound plot piece's shipped templates. Returns the ledger and the
/// refusals.
#[must_use]
pub fn ledger(
    plan: &Plan,
    prefabs: &PrefabRegistry,
    structures: &BTreeMap<String, Vec<u8>>,
) -> (Ledger, Vec<Diagnostic>) {
    let mut out = Ledger::default();
    let mut refused = Vec::new();
    let Some(graph) = plan.campaign.layout_graph.as_ref().map(|g| &g.content) else {
        return (out, refused);
    };
    let extent_of = |node: &NodeId| -> [i64; 2] {
        plan.campaign
            .site_plan
            .as_ref()
            .and_then(|sp| sp.content.boxes.iter().find(|b| b.node == *node))
            .map_or([0, 0], |b| {
                [i64::from(b.extent[0].get()), i64::from(b.extent[1].get())]
            })
    };
    for node in graph.nodes.iter().filter(|n| !n.holds.is_empty()) {
        let extent = extent_of(&node.id);
        let massed = |node: &Node| PlotEntry {
            node: node.id.0.clone(),
            extent,
            parts: node
                .holds
                .iter()
                .map(|p| PartEntry {
                    id: p.id.0.clone(),
                    kind: p.kind.word(),
                    use_: p.r#use.clone(),
                    reached: p.reached,
                    declared: p.cells,
                    built: 0,
                    height: p.height,
                    ridge: None,
                    roofed_share: 0.0,
                    setbacks: None,
                    gaps: Vec::new(),
                    regions: Vec::new(),
                    centre: None,
                })
                .collect(),
            built: None,
        };
        let Some((loc, pb)) = locate(plan, prefabs, &node.id) else {
            out.plots.push(massed(node));
            continue;
        };
        let bytes: Option<Vec<([i32; 3], &[u8])>> = loc
            .templates
            .iter()
            .map(|(o, f)| structures.get(*f).map(|b| (*o, b.as_slice())))
            .collect();
        let model: Option<VoxelModel> =
            bytes.and_then(|t| crate::admit::settling::grid_of_bytes(loc.size, &t).ok());
        let binding = bind(&node.id, &node.holds, loc.contract);
        let walked = model
            .as_ref()
            .map(|m| crate::grammar::contract::walked_cells(m, loc.contract));
        let (found, _) = judge(&binding, &loc.prefab, walked.as_ref());
        // Shapes 1, 2 and 4 were refused at validation; shape 3 is the
        // build's to add.
        refused.extend(
            found
                .into_iter()
                .filter(|d| d.message.starts_with("not walked")),
        );
        // Piece-local play space, walk plane and air.
        let (slo, shi) = pb.space();
        let to_local = |v: i64, a: usize| (v - i64::from(loc.pos[a])) as i32;
        let (x0, x1) = (to_local(slo[0], 0), to_local(shi[0], 0));
        let (z0, z1) = (to_local(slo[2], 2), to_local(shi[2], 2));
        let walk_y = to_local(slo[1], 1);
        let top_y = to_local(shi[1], 1);
        let in_play = |c: &[i32; 2]| (x0..=x1).contains(&c[0]) && (z0..=z1).contains(&c[1]);
        let play = ((x1 - x0 + 1) * (z1 - z0 + 1)).max(0) as usize;
        let blocked = |x: i32, y: i32, z: i32| -> bool {
            model.as_ref().is_some_and(|m| {
                m.get([x, y, z]).is_some_and(|b| b.name != "minecraft:air")
                    && !crate::grammar::nav::passable(m, [x, y, z])
            })
        };
        let structures_built: Vec<&PartBuilt<'_>> = binding
            .parts
            .iter()
            .filter(|p| p.part.kind == PartKind::Structure)
            .collect();
        let mut parts = Vec::new();
        for p in &binding.parts {
            let cols = &p.columns;
            let ridge = cols
                .iter()
                .filter_map(|c| {
                    (walk_y..=walk_y + loc.size[1])
                        .rev()
                        .find(|y| blocked(c[0], *y, c[1]))
                })
                .max()
                .map(|y| y - walk_y + 1);
            let roofed = cols
                .iter()
                .filter(|c| (walk_y + 2..=top_y).any(|y| blocked(c[0], y, c[1])))
                .count();
            let roofed_share = if cols.is_empty() {
                0.0
            } else {
                roofed as f64 / cols.len() as f64
            };
            let setbacks = (p.part.kind == PartKind::Structure && !cols.is_empty()).then(|| {
                let mx = cols.iter().map(|c| c[0]).max().unwrap_or(0);
                let nx = cols.iter().map(|c| c[0]).min().unwrap_or(0);
                let mz = cols.iter().map(|c| c[1]).max().unwrap_or(0);
                let nz = cols.iter().map(|c| c[1]).min().unwrap_or(0);
                [x1 - mx, nz - z0, z1 - mz, nx - x0]
            });
            let gaps = if p.part.kind == PartKind::Structure {
                structures_built
                    .iter()
                    .filter(|o| o.part.id != p.part.id && !o.columns.is_empty() && !cols.is_empty())
                    .map(|o| {
                        let g = cols
                            .iter()
                            .flat_map(|a| {
                                o.columns
                                    .iter()
                                    .map(move |b| (a[0] - b[0]).abs().max((a[1] - b[1]).abs()) - 1)
                            })
                            .min()
                            .unwrap_or(0);
                        (o.part.id.0.clone(), g.max(0))
                    })
                    .collect()
            } else {
                Vec::new()
            };
            parts.push(PartEntry {
                id: p.part.id.0.clone(),
                kind: p.part.kind.word(),
                use_: p.part.r#use.clone(),
                reached: p.part.reached,
                declared: p.part.cells,
                built: p.built(),
                height: p.part.height,
                ridge,
                roofed_share,
                setbacks,
                gaps,
                regions: p
                    .regions
                    .iter()
                    .map(|r| {
                        (
                            r.name.clone(),
                            r.envelope.clone(),
                            (r.envelope == "enclosed").then(|| r.clearance()),
                        )
                    })
                    .collect(),
                centre: centre_of(cols),
            });
        }
        let in_play_count = |f: &dyn Fn(&PartBuilt<'_>) -> bool| -> usize {
            binding
                .parts
                .iter()
                .filter(|p| f(p))
                .flat_map(|p| p.columns.iter())
                .filter(|c| in_play(c))
                .collect::<BTreeSet<_>>()
                .len()
        };
        let structure_cells = in_play_count(&|p| p.part.kind == PartKind::Structure);
        let all_cells = in_play_count(&|_| true);
        let share = |n: usize| {
            if play == 0 {
                0.0
            } else {
                n as f64 / play as f64
            }
        };
        out.plots.push(PlotEntry {
            node: node.id.0.clone(),
            extent,
            parts,
            built: Some(PlotShares {
                coverage: share(structure_cells),
                programmed: share(all_cells),
                remainder: play.saturating_sub(all_cells),
                remainder_share: share(play.saturating_sub(all_cells)),
            }),
        });
    }
    (out, refused)
}

/// **Every detailed plot's parts, in world cells** — what the part frames of
/// the render plan aim at (spec-0125 §6.2): per plot node, each part's id,
/// use and the world centre of its built columns at the walk plane. Reads
/// metadata only.
#[must_use]
pub fn part_aims(
    plan: &Plan,
    prefabs: &PrefabRegistry,
) -> BTreeMap<String, Vec<(String, String, [f64; 2])>> {
    let mut out = BTreeMap::new();
    let Some(graph) = plan.campaign.layout_graph.as_ref().map(|g| &g.content) else {
        return out;
    };
    for node in graph.nodes.iter().filter(|n| !n.holds.is_empty()) {
        let Some((loc, _)) = locate(plan, prefabs, &node.id) else {
            continue;
        };
        let binding = bind(&node.id, &node.holds, loc.contract);
        let aims = binding
            .parts
            .iter()
            .filter_map(|p| {
                centre_of(&p.columns).map(|[x, z]| {
                    (
                        p.part.id.0.clone(),
                        p.part.r#use.clone(),
                        [
                            f64::from(loc.pos[0]) + x + 0.5,
                            f64::from(loc.pos[2]) + z + 0.5,
                        ],
                    )
                })
            })
            .collect();
        out.insert(node.id.0.clone(), aims);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use delvewright_dsl::prefab::{ContractNoBody, ContractSpace};

    fn part(id: &str, kind: PartKind, cells: u32, reached: bool) -> Part {
        Part {
            id: delvewright_dsl::ids::PartId(id.to_string()),
            kind,
            r#use: format!("the {id}"),
            cells,
            height: None,
            reached,
        }
    }

    fn region(from: [i32; 3], to: [i32; 3]) -> Region {
        Region { from, to }
    }

    /// The plot of spec-0125 §3.2 in small: a house of two rooms (an L, 6 + 3
    /// columns), a yard of 12 columns, and a bed of 2 columns nobody walks.
    fn holds() -> Vec<Part> {
        vec![
            part("part/house", PartKind::Structure, 9, true),
            part("part/yard", PartKind::Ground, 12, true),
            part("part/bed", PartKind::Ground, 2, false),
        ]
    }

    fn contract(house_part: Option<&str>, yard_part: Option<&str>) -> SpatialContract {
        let space = |envelope: &str, boxes: Vec<Region>, part: Option<&str>| ContractSpace {
            envelope: envelope.to_string(),
            boxes,
            part: part.map(str::to_string),
        };
        SpatialContract {
            entry: "yard".to_string(),
            spaces: [
                (
                    "hall".to_string(),
                    space("enclosed", vec![region([0, 1, 0], [2, 3, 1])], house_part),
                ),
                (
                    "wing".to_string(),
                    space("enclosed", vec![region([0, 1, 2], [0, 3, 4])], house_part),
                ),
                (
                    "yard".to_string(),
                    space("open", vec![region([3, 1, 0], [5, 2, 3])], yard_part),
                ),
            ]
            .into_iter()
            .collect(),
            no_body: [(
                "bed".to_string(),
                ContractNoBody {
                    reason: "a bed".to_string(),
                    boxes: vec![region([3, 1, 4], [4, 1, 4])],
                    part: Some("part/bed".to_string()),
                },
            )]
            .into_iter()
            .collect(),
            edges: Vec::new(),
            faces: Vec::new(),
            no_body_majority_ack: None,
        }
    }

    fn node() -> NodeId {
        NodeId("node/lot".to_string())
    }

    /// Every cell of the house and the yard: what a flood that walks both
    /// reaches.
    fn walked_all() -> BTreeSet<[i32; 3]> {
        let mut w = BTreeSet::new();
        for x in 0..=5 {
            for z in 0..=4 {
                w.insert([x, 1, z]);
            }
        }
        w
    }

    fn shapes(d: &[Diagnostic]) -> Vec<&str> {
        d.iter()
            .map(|x| x.message.split(':').next().unwrap_or(""))
            .collect()
    }

    #[test]
    fn three_parts_bound_built_and_walked_pass_with_their_line() {
        let h = holds();
        let n = node();
        let c = contract(Some("part/house"), Some("part/yard"));
        let b = bind(&n, &h, &c);
        let built: Vec<usize> = b.parts.iter().map(PartBuilt::built).collect();
        assert_eq!(built, [9, 12, 2], "the L's two rooms are one part");
        let (d, count) = judge(&b, "prefab/lot", Some(&walked_all()));
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(count.line(), "parts: 3 of 3 bound, 2 of 2 walked");
        assert!(d.iter().all(|x| x.code == DW_PART_NOT_BUILT.id()));
    }

    #[test]
    fn a_part_no_region_names_is_unbuilt() {
        let h = holds();
        let n = node();
        let (d, count) = judge(&bind(&n, &h, &contract(None, Some("part/yard"))), "p", None);
        assert_eq!(shapes(&d), ["unbuilt"], "{d:?}");
        assert!(d[0].message.contains("part/house") && d[0].message.contains("the part/house"));
        assert_eq!(d[0].code, DW_PART_NOT_BUILT.id());
        assert_eq!((count.bound, count.parts), (2, 3));
    }

    #[test]
    fn a_part_the_node_does_not_declare_is_undeclared_and_so_is_any_part_on_a_non_plot() {
        let h = holds();
        let n = node();
        let (d, _) = judge(
            &bind(&n, &h, &contract(Some("part/house"), Some("part/garden"))),
            "p",
            None,
        );
        assert_eq!(shapes(&d), ["undeclared", "unbuilt"], "{d:?}");
        assert!(d[0].message.contains("part/garden"));
        let none: Vec<Part> = Vec::new();
        let (d, _) = judge(
            &bind(&n, &none, &contract(Some("part/house"), Some("part/yard"))),
            "p",
            None,
        );
        assert_eq!(shapes(&d), ["undeclared"; 4], "{d:?}");
        assert!(d[0].message.contains("declares no `holds[]`"));
    }

    #[test]
    fn a_walked_part_the_flood_never_reaches_is_refused_and_a_seen_one_is_not() {
        let h = holds();
        let n = node();
        let c = contract(Some("part/house"), Some("part/yard"));
        // The flood walks the yard and never the house; the bed is never
        // walked either, and is declared seen.
        let walked: BTreeSet<[i32; 3]> = walked_all().into_iter().filter(|c| c[0] >= 3).collect();
        let walked: BTreeSet<[i32; 3]> = walked.into_iter().filter(|c| c[2] < 4).collect();
        let (d, count) = judge(&bind(&n, &h, &c), "p", Some(&walked));
        assert_eq!(shapes(&d), ["not walked"], "{d:?}");
        assert!(d[0].message.contains("part/house"));
        assert_eq!(count.line(), "parts: 3 of 3 bound, 1 of 2 walked");
    }

    #[test]
    fn the_size_band_is_three_to_five_quarters_in_whole_columns_and_is_stated_per_part() {
        assert_eq!(size_band(38), (29, 47));
        assert_eq!(size_band(4), (3, 5));
        assert_eq!(size_band(1), (1, 1));
        let holds = holds();
        let n = node();
        let b = bind(&n, &holds, &contract(Some("part/house"), Some("part/yard")));
        let line = b.sizes_line();
        for p in &b.parts {
            if p.regions.is_empty() {
                continue;
            }
            let (lo, hi) = size_band(p.part.cells);
            assert!(
                line.contains(&format!(
                    "`{}` {} column(s) of {lo}..{hi}",
                    p.part.id,
                    p.built()
                )),
                "{line}"
            );
            assert_eq!(p.in_size(), (lo..=hi).contains(&(p.built() as u64)));
        }
    }

    #[test]
    fn a_part_outside_a_quarter_of_its_cells_is_out_of_size_at_both_ends() {
        let n = node();
        let c = contract(Some("part/house"), Some("part/yard"));
        // The yard builds 12 columns: inside a quarter for a declaration of
        // 10 (12 ≤ 12.5) up to 16 (12 ≥ 12); 9 and 17 fall outside.
        for (declared, refused) in [(9, true), (10, false), (12, false), (16, false), (17, true)] {
            let mut h = holds();
            h[1].cells = declared;
            let (d, _) = judge(&bind(&n, &h, &c), "p", None);
            assert_eq!(
                shapes(&d).contains(&"out of size"),
                refused,
                "the yard declared {declared}: {d:?}"
            );
            if refused {
                assert!(
                    d[0].message
                        .contains(&format!("declares {declared} cell(s)"))
                );
                assert!(d[0].message.contains("builds 12"));
            }
        }
    }
}
