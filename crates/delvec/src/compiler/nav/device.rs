//! **A device does what it reads as** (spec-0124): a door the story addresses
//! is the shutter of its way (`DW1069`), a key's effect is in sight or shown
//! (`DW1071`), and every door and fence gate in the built world closes an
//! opening (`DW1072`).
//!
//! The document arms of `DW1069` — a `shut: door` on a connection that cannot
//! hold a door (shape *c*) and a `prop` or `release` written on the key
//! anchored at a door-shut seam (shape *d*) — read the documents alone and are
//! refused at validation ([`check_document`]). Its world arms — a door the
//! story addresses that is no seam's shutter (shape *a*) and a shutter onto
//! nothing (shape *b*) — read the assembled world, as `DW1071` and `DW1072`
//! do, and are the build's.

use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::siteplan::{SitePlan, door_shut_seam_anchors, seam_anchor};
use delvewright_dsl::{Campaign, Diagnostic, DwCode, ExitTier, Objective};

delvewright_dsl::dw_code! {
    /// `DW1069`: a door the story addresses is not the shutter of a way, or a
    /// door-shut seam is declared where no door can stand (spec-0124 §7.1).
    pub const DW_DOOR_NO_WAY: DwCode = DwCode::new("DW1069", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW1071`: a key's effect is neither in sight of where the key is used
    /// nor shown by a shot (spec-0124 §7.3).
    pub const DW_EFFECT_OUT_OF_SIGHT: DwCode = DwCode::new("DW1071", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW1072`: a door or fence gate in the built world closes no opening
    /// (spec-0124 §7.4).
    pub const DW_DOOR_CLOSES_NOTHING: DwCode = DwCode::new("DW1072", ExitTier::Build);
}

/// **`DW1069`, shapes *c* and *d*** (spec-0124 §5.1, §5.2): over the layout
/// graph, the site plan and the quests. A campaign with no door-shut seam
/// judges nothing and returns no diagnostic.
#[must_use]
pub fn check_document(c: &Campaign) -> Vec<Diagnostic> {
    let mut d = Vec::new();
    let Some(graph) = c.layout_graph.as_ref().map(|g| &g.content) else {
        return d;
    };
    let plan = c.site_plan.as_ref().map(|_| SitePlan::of(c));
    for (ei, e) in graph.edges.iter().enumerate() {
        if !e.door_shut() {
            continue;
        }
        let path = format!("/content/edges/{ei}/gating/shut");
        let class = e.class();
        if class != "walk" {
            d.push(shape_c(
                path,
                format!(
                    "`{}` is a `{class}`: a door stands in a wall between two floors a body \
                     walks, and a {class} has no such opening",
                    e.id()
                ),
            ));
            continue;
        }
        let Some(plan) = plan.as_ref() else {
            continue; // with no site plan there is no seam to stand a door in
        };
        let Some(seam) = plan.seams.iter().find(|s| &s.edge == e.id()) else {
            d.push(shape_c(
                path,
                format!(
                    "`{}` has no seam in the site plan: a door stands in a seam's opening",
                    e.id()
                ),
            ));
            continue;
        };
        let (lo, hi) = seam.opening;
        let ext = |i: usize| hi[i] - lo[i] + 1;
        let along = 2 - seam.normal_axis;
        let door_sized = seam.normal_axis != 1
            && seam.crossing == delvewright_dsl::siteplan::Crossing::Portal
            && ext(along) == 1
            && ext(1) == 2;
        if !door_sized {
            d.push(shape_c(
                path,
                format!(
                    "`{}`'s seam opening is {} wide and {} high{}: a door is one cell wide and \
                     two high, and stands in a wall — give the seam the `door` opening, or keep \
                     the way shut by bars",
                    e.id(),
                    if seam.normal_axis == 1 {
                        ext(0)
                    } else {
                        ext(along)
                    },
                    if seam.normal_axis == 1 {
                        ext(2)
                    } else {
                        ext(1)
                    },
                    if seam.normal_axis == 1 {
                        " through a floor"
                    } else {
                        ""
                    }
                ),
            ));
        }
    }
    let door_seams = door_shut_seam_anchors(c);
    if door_seams.is_empty() {
        return d;
    }
    let quests = &c.quests.content;
    for (ti, g) in quests.timed_gates.iter().enumerate() {
        if g.crush && door_seams.contains(g.gate.as_str()) {
            d.push(shape_c(
                format!("/content/timed_gates/{ti}/crush"),
                format!(
                    "timed gate `{}` crushes at `{}`, a door-shut seam: a leaf is not a \
                     portcullis — drop `crush`, or shut the seam by bars",
                    g.id.as_str(),
                    g.gate.as_str()
                ),
            ));
        }
    }
    for (qi, q) in quests.quests.iter().enumerate() {
        for (oi, o) in q.objectives.iter().enumerate() {
            let Objective::Interact {
                id,
                anchor,
                prop,
                release,
                ..
            } = o
            else {
                continue;
            };
            if !door_seams.contains(anchor.as_str()) {
                continue;
            }
            for (field, written) in [("prop", prop.is_some()), ("release", *release)] {
                if written {
                    d.push(Diagnostic::error(
                        DW_DOOR_NO_WAY,
                        "quests",
                        format!("/content/quests/{qi}/objectives/{oi}/{field}"),
                        format!(
                            "shape d: `interact` `{}` is anchored at `{}`, a door-shut seam, and \
                             writes `{field}`: the seam's door is the pressable, and the verb \
                             that opens the gate releases it to the hand — a second statement \
                             of what the seam already declares. Remove `{field}`",
                            id.as_str(),
                            anchor.as_str()
                        ),
                    ));
                }
            }
        }
    }
    d
}

fn shape_c(path: String, why: String) -> Diagnostic {
    Diagnostic::error(
        DW_DOOR_NO_WAY,
        "layout-graph",
        path,
        format!(
            "shape c: `shut: door` cannot stand here — {why} (spec-0124 §5.1). A door-shut seam is \
             a `walk` through a one-wide, two-high opening in a wall"
        ),
    )
}

/// The door-shut seams of a campaign as `(gate anchor, edge)`, in graph order.
#[must_use]
pub fn door_seam_edges(c: &Campaign) -> BTreeMap<String, String> {
    c.layout_graph
        .as_ref()
        .map(|g| {
            g.content
                .edges
                .iter()
                .filter(|e| e.door_shut())
                .map(|e| (seam_anchor(e.id()), e.id().as_str().to_string()))
                .collect()
        })
        .unwrap_or_default()
}

/// Every gated connection of a campaign that is built shut, by edge id.
#[must_use]
pub fn gated_edges(c: &Campaign) -> BTreeSet<String> {
    c.layout_graph
        .as_ref()
        .map(|g| {
            g.content
                .edges
                .iter()
                .filter(|e| e.shuts())
                .map(|e| e.id().as_str().to_string())
                .collect()
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// §6.5 — every door and fence gate closes an opening (`DW1072`)
// ---------------------------------------------------------------------------

use super::census::{DoorEntry, Reading};
use super::{Confinement, Premises, World};
use crate::compiler::blockstate::{BlockMap, BlockState};
use delvewright_dsl::blockshape::{bare_id, state_value};

/// One door or fence gate as `DW1072` judged it.
#[derive(Debug, Clone, PartialEq)]
pub struct DoorRow {
    /// The lower half's (or the gate's) cell.
    pub cell: [i32; 3],
    /// Its block id, states stripped.
    pub block: String,
    /// `door` or `gate`.
    pub kind: &'static str,
    /// The decorative declaration it stands under, `(what, why exempt)`, when
    /// one passes it: `no hand opens` or `out of reach`.
    pub decorative: Option<(String, &'static str)>,
    /// Clause 1's failure: the cells round the leaf a body passes through.
    pub clause1: Option<String>,
    /// Clause 2's failure: the far cell and what stands there.
    pub clause2: Option<String>,
    /// For a declared door judged out of reach (or refused in reach): the
    /// least distance, the rest cell it is measured from, and the eye.
    pub reach: Option<(f64, [i32; 3], &'static str)>,
    /// The refusal, when refused.
    pub refusal: Option<String>,
}

/// What `DW1072` bound on one build.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DoorsLedger {
    /// Every door and gate in the assembled world, judged or exempt.
    pub rows: Vec<DoorRow>,
    /// The census's count over its bound pieces (`doors framed: D of N`'s N).
    pub in_pieces: usize,
    /// Configurations judged: `C₀` and one `C_D` per door judged.
    pub configurations: usize,
    /// Decorative entries aimed at no door or gate, as `(cell, what)`.
    pub aimed_at_nothing: Vec<([i32; 3], String)>,
}

impl DoorsLedger {
    /// Declared decorative and exempt.
    #[must_use]
    pub fn decorative(&self) -> usize {
        self.rows.iter().filter(|r| r.decorative.is_some()).count()
    }

    /// Refused rows, and declarations aimed at nothing.
    #[must_use]
    pub fn refused(&self) -> usize {
        self.rows.iter().filter(|r| r.refusal.is_some()).count() + self.aimed_at_nothing.len()
    }

    /// The binding line every build that judges the world prints, zeroes
    /// included (spec-0124 §6.5).
    #[must_use]
    pub fn line(&self) -> String {
        let n = self.rows.len();
        let d = self.decorative();
        let cells: Vec<String> = self
            .rows
            .iter()
            .filter_map(|r| {
                r.decorative
                    .as_ref()
                    .map(|(_, why)| format!("{:?} ({why})", r.cell))
            })
            .collect();
        format!(
            "{} binding: {n} door(s) and gate(s) in the assembled world ({} in bound pieces, the \
             census's N); {d} declared decorative{}; {} judged over {} configuration(s); {} refused",
            DW_DOOR_CLOSES_NOTHING.id(),
            self.in_pieces,
            if cells.is_empty() {
                String::new()
            } else {
                format!(" [{}]", cells.join(", "))
            },
            n - d,
            self.configurations,
            self.refused()
        )
    }

    /// The ledger's `doors` rows (`validation/devices.json`).
    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "code": DW_DOOR_CLOSES_NOTHING.id(),
            "doors_and_gates": self.rows.len(),
            "in_bound_pieces": self.in_pieces,
            "decorative": self.decorative(),
            "judged": self.rows.len() - self.decorative(),
            "configurations": self.configurations,
            "refused": self.refused(),
            "doors": self.rows.iter().map(|r| serde_json::json!({
                "cell": r.cell,
                "block": r.block,
                "kind": r.kind,
                "status": match &r.decorative {
                    Some((_, why)) => format!("decorative ({why})"),
                    None => "judged".to_string(),
                },
                "what": r.decorative.as_ref().map(|(w, _)| w),
                "clause1": r.clause1,
                "clause2": r.clause2,
                "reach": r.reach.map(|(d, from, eye)| serde_json::json!({
                    "distance": (d * 100.0).round() / 100.0,
                    "from": from,
                    "eye": eye,
                })),
                "refused": r.refusal.is_some(),
            })).collect::<Vec<_>>(),
            "aimed_at_nothing": self.aimed_at_nothing.iter().map(|(c, w)| serde_json::json!({
                "at": c, "what": w,
            })).collect::<Vec<_>>(),
        })
    }
}

/// The horizontal unit vector a door's or gate's `facing` names.
fn facing_vec(block: &str) -> [i32; 3] {
    match state_value(block, "facing") {
        Some("south") => [0, 0, 1],
        Some("east") => [1, 0, 0],
        Some("west") => [-1, 0, 0],
        _ => [0, 0, -1],
    }
}

fn add(a: [i32; 3], b: [i32; 3], k: i32) -> [i32; 3] {
    [a[0] + b[0] * k, a[1] + b[1] * k, a[2] + b[2] * k]
}

/// `name` with its `open` property set to `open`, every other property kept.
fn with_open(name: &str, open: bool) -> String {
    let Some((id, rest)) = name.split_once('[') else {
        return format!("{name}[open={open}]");
    };
    let props: Vec<String> = rest
        .trim_end_matches(']')
        .split(',')
        .filter(|p| !p.is_empty() && !p.starts_with("open="))
        .map(str::to_string)
        .chain(std::iter::once(format!("open={open}")))
        .collect();
    format!("{id}[{}]", props.join(","))
}

/// Whether a block is a door or a fence gate — what §6.5 judges.
pub fn door_or_gate(name: &str) -> bool {
    let b = bare_id(name);
    b.ends_with("_door") || b.ends_with("_fence_gate")
}

/// **One configuration of a door's neighbourhood** (spec-0124 §6.5): the
/// assembled blocks within `half` of `centre`, every door and gate shut, `open`
/// alone standing open, read as the reach graph reads a world in which no
/// body opens anything — each leaf as it stands
/// ([`World::without_gate_use`]). A door no hand opens is a full cube to the
/// occupancy model, whatever its state; its open state is read through the
/// one door shape class (`blockshape::leaf`: every door id takes the same box
/// for the same `facing`, `hinge` and `open`), as an oak door's.
fn configuration(blocks: &BlockMap, centre: [i32; 3], open: Option<&[[i32; 3]]>) -> World {
    let lo = [centre[0] - 5, centre[1] - 4, centre[2] - 5];
    let hi = [centre[0] + 6, centre[1] + 6, centre[2] + 6];
    let mut local: BTreeMap<[i32; 3], String> = BTreeMap::new();
    let mut open_set: BTreeSet<[i32; 3]> = BTreeSet::new();
    for (&c, b) in blocks.range(lo..hi) {
        if !(lo[0]..hi[0]).contains(&c[0])
            || !(lo[1]..hi[1]).contains(&c[1])
            || !(lo[2]..hi[2]).contains(&c[2])
        {
            continue;
        }
        let name = b.as_str();
        if delvewright_dsl::blockshape::is_air(name) {
            continue;
        }
        if door_or_gate(name) {
            let opened = open.is_some_and(|o| o.contains(&c));
            let mut state = with_open(name, opened);
            if opened {
                open_set.insert(c);
                if !delvewright_dsl::blockshape::opens_by_hand(name)
                    && bare_id(name).ends_with("_door")
                {
                    let props = state.split_once('[').map_or("", |(_, p)| p);
                    state = format!("minecraft:oak_door[{props}");
                }
            }
            local.insert(c, state);
        } else {
            local.insert(c, name.to_string());
        }
    }
    // `geometry_only`: whether a body passes round a leaf, or through it open,
    // is a fact of the blocks beside it; no horizon, volume or gate premise of
    // the campaign moves it (`tests/premise_declines.rs`).
    let mut w = World::from_occupancy(
        crate::compiler::assembled::occupancy_of(local, &open_set),
        Premises::geometry_only(),
    );
    w.piece = Some((lo, hi));
    w.without_gate_use()
}

/// The two side cells of a door or gate, before and behind along its facing,
/// each the cell at its course or the one under it when that is where a body
/// rests.
fn sides(w: &World, cell: [i32; 3], f: [i32; 3]) -> [[i32; 3]; 2] {
    let rest = |c: [i32; 3]| {
        let under = [c[0], c[1] - 1, c[2]];
        if w.rest_kind(c).is_none() && w.rest_kind(under).is_some() {
            under
        } else {
            c
        }
    };
    [rest(add(cell, f, -1)), rest(add(cell, f, 1))]
}

/// The window a door's two clauses are asked over: the three columns across
/// it (the door's and its jambs'), one cell before and behind, from a course
/// under it to the course over its upper half.
fn in_window(cell: [i32; 3], f: [i32; 3], c: [i32; 3]) -> bool {
    let across = [f[2].abs(), 0, f[0].abs()];
    let d = [c[0] - cell[0], c[1] - cell[1], c[2] - cell[2]];
    let along_d = d[0] * f[0] + d[2] * f[2];
    let across_d = d[0] * across[0] + d[2] * across[2];
    (-1..=1).contains(&along_d) && (-1..=1).contains(&across_d) && (-1..=2).contains(&d[1])
}

/// The cells a body reaches from `root` in `w` without leaving `cell`'s window
/// — the reach graph's own confined walk ([`World::body_flood`]).
fn reached(w: &World, cell: [i32; 3], f: [i32; 3], root: [i32; 3]) -> BTreeSet<[i32; 3]> {
    let keep = move |c: [i32; 3]| in_window(cell, f, c);
    w.body_flood(&[root], &Confinement::cells(&keep))
}

/// **Whether a door or gate closes an opening**: clause 1 (shut, nothing
/// passes through it) over `C₀`, clause 2 (open, the area behind it is
/// reached through it) over `C_D`, each from both sides as root. Returns the
/// two failures in words.
#[must_use]
pub fn closes(blocks: &BlockMap, entry: &DoorEntry) -> (Option<String>, Option<String>) {
    let cell = entry.cell;
    let Some(name) = blocks.get(&cell).map(BlockState::as_str) else {
        return (None, Some("no block stands at the door's cell".into()));
    };
    let f = facing_vec(name);
    let is_door = bare_id(name).ends_with("_door");
    let shut = configuration(blocks, cell, None);
    let [s1, s2] = sides(&shut, cell, f);
    let mut clause1 = None;
    for (from, to) in [(s1, s2), (s2, s1)] {
        let got = reached(&shut, cell, f, from);
        if got.contains(&to) {
            let round: Vec<[i32; 3]> = got
                .iter()
                .filter(|c| **c != from && **c != to && !(c[0] == cell[0] && c[2] == cell[2]))
                .copied()
                .collect();
            clause1 = Some(format!(
                "shut, a body passes from {from:?} to {to:?} round the leaf, through {}",
                round
                    .iter()
                    .map(|c| format!("{c:?}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            break;
        }
    }
    let mut leaf = vec![cell];
    if is_door {
        leaf.push([cell[0], cell[1] + 1, cell[2]]);
    }
    let open = configuration(blocks, cell, Some(&leaf));
    let [o1, o2] = sides(&open, cell, f);
    let mut clause2 = None;
    for to in [o1, o2] {
        if open.rest_kind(to).is_none() {
            let under = [to[0], to[1] - 1, to[2]];
            let what = |c: [i32; 3]| {
                blocks
                    .get(&c)
                    .map_or("air".to_string(), |b| format!("`{}`", b.as_str()))
            };
            clause2 = Some(format!(
                "open, the cell behind it {to:?} holds no body ({} there, {} under it)",
                what(to),
                what(under)
            ));
            break;
        }
    }
    for (from, to) in [(o1, o2), (o2, o1)] {
        if clause2.is_some() {
            break;
        }
        if !reached(&open, cell, f, from).contains(&to) {
            clause2 = Some(format!(
                "open, a body at {from:?} does not reach {to:?} through it"
            ));
            break;
        }
    }
    (clause1, clause2)
}

/// The least distance from the eye volume over rest cell `rest` (feet at
/// `feet`) to the box of `cell`, and which eye it is measured from (spec-0124
/// §5.4): the body's footprint `[0.3, 0.7]` in `x` and `z`, the eye from the
/// crouched height up to the standing eye raised by the reach graph's jump.
#[must_use]
pub fn eye_distance(rest: [i32; 3], feet: f64, cell: [i32; 3]) -> (f64, &'static str) {
    use delvewright_dsl::metrics::{
        MAX_JUMP_RISE_16, PLAYER_CROUCHED_EYE_HEIGHT, PLAYER_EYE_HEIGHT,
    };
    let gap = |lo: f64, hi: f64, blo: f64, bhi: f64| {
        if hi < blo {
            blo - hi
        } else if lo > bhi {
            lo - bhi
        } else {
            0.0
        }
    };
    let x = gap(
        f64::from(rest[0]) + 0.3,
        f64::from(rest[0]) + 0.7,
        f64::from(cell[0]),
        f64::from(cell[0]) + 1.0,
    );
    let z = gap(
        f64::from(rest[2]) + 0.3,
        f64::from(rest[2]) + 0.7,
        f64::from(cell[2]),
        f64::from(cell[2]) + 1.0,
    );
    let jump = MAX_JUMP_RISE_16 as f64 / 16.0;
    let (ylo, yhi) = (
        feet + PLAYER_CROUCHED_EYE_HEIGHT,
        feet + PLAYER_EYE_HEIGHT + jump,
    );
    let y = gap(ylo, yhi, f64::from(cell[1]), f64::from(cell[1]) + 1.0);
    let eye = if f64::from(cell[1]) > yhi {
        "the jump apex"
    } else if f64::from(cell[1]) + 1.0 < ylo {
        "the crouched eye"
    } else {
        "an eye between crouched and the jump apex"
    };
    ((x * x + y * y + z * z).sqrt(), eye)
}

/// The nearest rest state to `cells` among `rest` by [`eye_distance`]:
/// `(distance, rest cell, eye)`, or `None` with no rest state.
fn nearest(
    world: &World,
    rest: &BTreeSet<[i32; 3]>,
    cells: &[[i32; 3]],
) -> Option<(f64, [i32; 3], &'static str)> {
    let fp = super::Footprint::player();
    let mut best: Option<(f64, [i32; 3], &'static str)> = None;
    for &r in rest {
        let feet = world.feet_16_fp(r, &fp) as f64 / 16.0;
        for &c in cells {
            let (d, eye) = eye_distance(r, feet, c);
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, r, eye));
            }
        }
    }
    best
}

/// **`DW1072`** over the assembled world (spec-0124 §6.5, §5.4): every door
/// and fence gate the census enters over `lo..=hi`, judged by [`closes`];
/// each declared decorative one exempt when no hand opens it or no rest state
/// of `rest` is within [`delvewright_dsl::metrics::BLOCK_INTERACTION_RANGE`]
/// of its cells. `world` is the world the rest states are a reach over, read
/// for a rest state's feet height.
pub fn check_doors(
    blocks: &BlockMap,
    lo: [i32; 3],
    hi: [i32; 3],
    decorative: &[([i32; 3], String)],
    world: &World,
    rest: &dyn Fn() -> BTreeSet<[i32; 3]>,
    in_pieces: usize,
) -> (DoorsLedger, Vec<String>) {
    let entries = super::census::doors(&Reading::new(blocks), lo, hi);
    let mut ledger = DoorsLedger {
        in_pieces,
        configurations: 1,
        ..DoorsLedger::default()
    };
    let mut refusals = Vec::new();
    let mut rest_cells: Option<BTreeSet<[i32; 3]>> = None;
    for (at, what) in decorative {
        if !entries.iter().any(|e| e.cell == *at) {
            ledger.aimed_at_nothing.push((*at, what.clone()));
            refusals.push(format!(
                "a `decorative` entry ({what}) is aimed at {at:?}, which holds no door or fence \
                 gate (`{}`): an exemption aimed at nothing (spec-0124 §5.4). Point `at` at the \
                 door's lower cell or the gate's cell, or remove the entry",
                blocks.get(at).map_or("air", BlockState::as_str)
            ));
        }
    }
    for e in &entries {
        let name = blocks.get(&e.cell).map_or("", BlockState::as_str);
        let kind = if e.block.ends_with("_door") {
            "door"
        } else {
            "gate"
        };
        let mut row = DoorRow {
            cell: e.cell,
            block: e.block.clone(),
            kind,
            decorative: None,
            clause1: None,
            clause2: None,
            reach: None,
            refusal: None,
        };
        if let Some((_, what)) = decorative.iter().find(|(at, _)| *at == e.cell) {
            if !delvewright_dsl::blockshape::opens_by_hand(name) {
                row.decorative = Some((what.clone(), "no hand opens"));
                ledger.rows.push(row);
                continue;
            }
            let rs = rest_cells.get_or_insert_with(rest);
            let mut cells = vec![e.cell];
            if kind == "door" {
                cells.push([e.cell[0], e.cell[1] + 1, e.cell[2]]);
            }
            let near = nearest(world, rs, &cells);
            row.reach = near;
            match near {
                Some((d, _, _)) if d < delvewright_dsl::metrics::BLOCK_INTERACTION_RANGE => {
                    let (d, from, eye) = near.expect("matched");
                    let msg = format!(
                        "the {kind} `{}` at {:?}, declared `decorative` ({what}), is one a hand \
                         opens and a player can touch: from the rest cell {from:?}, {eye}, its \
                         box is {d:.2} blocks away, inside the hand's {} (spec-0124 §5.4). \
                         Raise it out of reach, or make it close a way",
                        e.block,
                        e.cell,
                        delvewright_dsl::metrics::BLOCK_INTERACTION_RANGE
                    );
                    row.refusal = Some(msg.clone());
                    refusals.push(msg);
                }
                _ => {
                    row.decorative = Some((what.clone(), "out of reach"));
                }
            }
            ledger.rows.push(row);
            continue;
        }
        ledger.configurations += 1;
        let (c1, c2) = closes(blocks, e);
        if c1.is_some() || c2.is_some() {
            let clause = match (&c1, &c2) {
                (Some(a), Some(b)) => format!("clause 1 — {a}; clause 2 — {b}"),
                (Some(a), None) => format!("clause 1 — {a}"),
                (None, Some(b)) => format!("clause 2 — {b}"),
                (None, None) => unreachable!(),
            };
            let opening: Vec<String> = e
                .framing
                .iter()
                .filter(|f| f.role == "jamb")
                .map(|f| format!("{:?} {}", f.cell, if f.present { "walled" } else { "open" }))
                .collect();
            let msg = format!(
                "the {kind} `{}` at {:?} closes no opening: {clause} (its jambs: {}). A {kind} \
                 does a {kind}'s job — shut, nothing passes through it; open, the place behind it \
                 is reached through it (spec-0124 §6.5). Remove it, join wall to both sides of \
                 it, or — where it is a decoration — declare it `decorative` on the piece's \
                 contract, which passes only a {kind} no hand opens or none a player can touch \
                 ({} blocks from any eye)",
                e.block,
                e.cell,
                opening.join(", "),
                delvewright_dsl::metrics::BLOCK_INTERACTION_RANGE
            );
            row.refusal = Some(msg.clone());
            refusals.push(msg);
        }
        row.clause1 = c1;
        row.clause2 = c2;
        ledger.rows.push(row);
    }
    (ledger, refusals)
}

// ---------------------------------------------------------------------------
// §6.1 — a story-addressed door is a seam's shutter (`DW1069` a, b)
// ---------------------------------------------------------------------------

use crate::compiler::plan::Plan;
use crate::compiler::pressable::{Owner, in_region};

/// What `DW1069`'s world arms bound on one build.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WayBlockBinding {
    /// Pressables the build places.
    pub pressables: usize,
    /// Of those, way-blocks the story addresses: a block a hand opens pressed
    /// by an `interact`, a `use` trigger, a shortcut's unlock or a disarm.
    pub way_blocks: usize,
    /// Door-shut seams.
    pub door_seams: usize,
    /// Gated connections built shut.
    pub gated: usize,
    /// Refused.
    pub refused: usize,
}

impl WayBlockBinding {
    /// The binding line (spec-0124 §7.1).
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "device binding ({}): {} way-block(s) among {} pressable(s); {} door-shut seam(s) \
             among {} gated connection(s); {} refused",
            DW_DOOR_NO_WAY.id(),
            self.way_blocks,
            self.pressables,
            self.door_seams,
            self.gated,
            self.refused
        )
    }
}

/// **`DW1069`, shapes *a* and *b***: every way-block the story addresses
/// stands in a door-shut seam's gate region, and every door-shut seam opens
/// onto a cell a body can be in on the side it opens to.
#[must_use]
pub fn check_way_blocks(plan: &Plan, world: &World) -> (WayBlockBinding, Vec<String>) {
    let c = plan.campaign;
    let seams = door_seam_edges(c);
    let mut b = WayBlockBinding {
        pressables: plan.presses().len(),
        door_seams: seams.len(),
        gated: gated_edges(c).len(),
        ..WayBlockBinding::default()
    };
    let regions: Vec<(String, ([i32; 3], [i32; 3]))> = seams
        .keys()
        .filter_map(|a| {
            crate::compiler::pressable::gate_region(plan, a).map(|(f, t, _)| (a.clone(), (f, t)))
        })
        .collect();
    let mut refusals = Vec::new();
    for p in plan.presses() {
        let addressed = matches!(
            p.owner,
            Owner::Interact { .. }
                | Owner::Trigger { .. }
                | Owner::Shortcut { .. }
                | Owner::TrapDisarm { .. }
        );
        if !addressed || !delvewright_dsl::blockshape::opens_by_hand(&p.block) {
            continue;
        }
        b.way_blocks += 1;
        if regions
            .iter()
            .any(|(_, r)| p.cells().iter().all(|c| in_region(*c, *r)))
        {
            continue;
        }
        b.refused += 1;
        let nearest = plan
            .anchors
            .iter()
            .filter_map(|((_, n), r)| match r {
                crate::compiler::plan::ResolvedAnchor::Gate { from, .. }
                    if n.starts_with("anchor/seam-") =>
                {
                    let d = (0..3).map(|i| (from[i] - p.cell[i]).abs()).sum::<i32>();
                    Some((d, n.clone(), *from))
                }
                _ => None,
            })
            .min();
        refusals.push(format!(
            "shape a: {} presses `{}` at {:?}, a block a hand opens — a door the story \
             addresses — and it stands in no door-shut seam: to the layout graph the way it \
             reads as is not there, so pressing it opens nothing a proof walks (spec-0124 \
             §6.1).{} Either declare the connection gated with `\"shut\": \"door\"` and anchor \
             the step on its `anchor/seam-<edge>` (the seam stands the door), or make the \
             pressable a block that is not a way — a lever, a button, a bell",
            p.owner.word(),
            p.block,
            p.cell,
            nearest
                .map(|(d, n, at)| format!(" The nearest seam is `{n}` at {at:?}, {d} cell(s) off."))
                .unwrap_or_default()
        ));
    }
    // Shape b: the far side of every door-shut seam holds a body.
    let site = delvewright_dsl::siteplan::SitePlan::of(c);
    let graph = c.layout_graph.as_ref().map(|g| &g.content);
    for s in site.seams.iter().filter(|s| s.door.is_some()) {
        let Some((lo, _)) = s.bar() else { continue };
        let lo = [lo[0] as i32, lo[1] as i32, lo[2] as i32];
        let n = s.normal_axis;
        let toward_b = s.face.vector()[n] as i32;
        let opens = graph
            .and_then(|g| g.edges.iter().find(|e| e.id() == &s.edge))
            .and_then(|e| e.opens_from());
        // The side it opens to: away from `opens_from`; both for `either`.
        let fars: Vec<i32> = match opens {
            Some(delvewright_dsl::layout::OpensFrom::A) => vec![toward_b],
            Some(delvewright_dsl::layout::OpensFrom::B) => vec![-toward_b],
            _ => vec![toward_b, -toward_b],
        };
        for k in fars {
            let mut far = lo;
            far[n] += k;
            let under = [far[0], far[1] - 1, far[2]];
            if world.rest_kind(far).is_some() || world.rest_kind(under).is_some() {
                continue;
            }
            b.refused += 1;
            refusals.push(format!(
                "shape b: the door-shut seam of `{}` opens onto {far:?}, where no body can be \
                 and none can stand on the cell under it: a shutter onto nothing is a door \
                 that leads nowhere (spec-0124 §6.1). Floor the cell behind the door, or move \
                 the seam",
                s.edge,
            ));
        }
    }
    (b, refusals)
}

// ---------------------------------------------------------------------------
// §6.3 — a key's effect is in sight or shown (`DW1071`)
// ---------------------------------------------------------------------------

/// What `DW1071` bound on one build.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyBinding {
    /// Objectives in the campaign.
    pub objectives: usize,
    /// `interact` objectives with `requires_item`.
    pub keys: usize,
    /// World writes their completions perform.
    pub writes: usize,
    /// Of those, in sight from the key's cell.
    pub in_sight: usize,
    /// Of those, shown by a shot in the same completion.
    pub shown: usize,
    /// Of those, a door-shut seam's release at the key's own anchor.
    pub own_release: usize,
    /// Refused.
    pub refused: usize,
}

impl KeyBinding {
    /// The binding line (spec-0124 §7.3).
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "key binding ({}): {} key objective(s) among {}; {} write(s) judged, {} in sight, {} \
             shown by a shot, {} the key's own door released; {} refused",
            DW_EFFECT_OUT_OF_SIGHT.id(),
            self.keys,
            self.objectives,
            self.writes,
            self.in_sight,
            self.shown,
            self.own_release,
            self.refused
        )
    }
}

/// Whether a cutscene in `effects` frames `region`: a shot's (or the
/// cutscene's) `look_at` or anchor subject resolving within two cells of it,
/// or naming `anchor` itself.
fn shown(
    plan: &Plan,
    effects: &[&delvewright_dsl::QuestEffect],
    region: ([i32; 3], [i32; 3]),
    anchor: Option<&str>,
) -> bool {
    use delvewright_dsl::{CameraSubject, Verb};
    let near = |m: &delvewright_dsl::Mark| {
        if anchor == Some(m.anchor.as_str()) {
            return true;
        }
        crate::compiler::plan::point_any_in(&plan.anchors, m.anchor.as_str()).is_some_and(|p| {
            let p = [p[0] + m.offset[0], p[1] + m.offset[1], p[2] + m.offset[2]];
            (0..3).all(|i| {
                let (a, b) = (region.0[i].min(region.1[i]), region.0[i].max(region.1[i]));
                a - 2 <= p[i] && p[i] <= b + 2
            })
        })
    };
    let mut stack: Vec<&delvewright_dsl::QuestEffect> = effects.to_vec();
    while let Some(e) = stack.pop() {
        if let Verb::Cutscene {
            shots,
            look_at,
            subject,
            ..
        } = &e.verb
        {
            let subj =
                |s: &Option<CameraSubject>| matches!(s, Some(CameraSubject::Anchor(m)) if near(m));
            if look_at.as_ref().is_some_and(near) || subj(subject) {
                return true;
            }
            if shots
                .iter()
                .any(|s| s.look_at.as_ref().is_some_and(near) || subj(&s.subject))
            {
                return true;
            }
        }
        for list in e.nested_effect_lists() {
            stack.extend(list.iter());
        }
    }
    false
}

/// **`DW1071`**: every world write a key's completion performs is in sight of
/// the key's cell by `DW0863`'s own sight predicate
/// ([`crate::compiler::promise::act_unfound`]), or shown by a shot in the
/// same completion; a door-shut seam's release at the key's own anchor
/// passes by construction.
#[must_use]
pub fn check_keys(plan: &Plan, world: &World) -> (KeyBinding, Vec<String>) {
    let mut b = KeyBinding::default();
    let mut refusals = Vec::new();
    for q in &plan.campaign.quests.content.quests {
        for o in &q.objectives {
            b.objectives += 1;
            let delvewright_dsl::Objective::Interact {
                id,
                anchor,
                requires_item: Some(_),
                ..
            } = o
            else {
                continue;
            };
            b.keys += 1;
            let from = crate::compiler::pressable::press_of_objective(plan, id.as_str())
                .map(|p| p.cell)
                .or_else(|| crate::compiler::plan::point_any_in(&plan.anchors, anchor.as_str()));
            let Some(from) = from else { continue };
            let effects: Vec<&delvewright_dsl::QuestEffect> = q
                .on_objective_complete
                .get(o.id())
                .map(|v| v.iter().collect())
                .unwrap_or_default();
            let mut stack = effects.clone();
            while let Some(e) = stack.pop() {
                for list in e.nested_effect_lists() {
                    stack.extend(list.iter());
                }
                let gate = e.gate_region_write().map(|(a, _)| a.as_str().to_string());
                for (region, _, _) in
                    crate::compiler::plan::effect_writes(e, &plan.anchors, &plan.ways)
                {
                    b.writes += 1;
                    if gate.as_deref() == Some(anchor.as_str()) && plan.door_shut(anchor.as_str()) {
                        b.own_release += 1;
                        continue;
                    }
                    let cells = crate::compiler::assembled::region_cells(region.0, region.1);
                    let mut why: Option<Vec<String>> = None;
                    let mut seen = false;
                    for c in cells {
                        let w = crate::compiler::promise::act_unfound(world, from, c, false, None);
                        if w.is_empty() {
                            seen = true;
                            break;
                        }
                        why.get_or_insert(w);
                    }
                    if seen {
                        b.in_sight += 1;
                        continue;
                    }
                    if shown(plan, &effects, region, gate.as_deref()) {
                        b.shown += 1;
                        continue;
                    }
                    b.refused += 1;
                    refusals.push(format!(
                        "`interact` `{}` is a key used at {from:?}, and its completion writes \
                         {:?}..={:?}{} out of the presser's sight — {} — with no shot in the same \
                         completion framing it: a key whose effect lands nowhere the player can \
                         see is a cause with no visible lock (spec-0124 §6.3). Stand the device \
                         where its effect is in view, or add a `cutscene` whose shot looks at it",
                        id.as_str(),
                        region.0,
                        region.1,
                        gate.as_deref()
                            .map(|g| format!(" (`{g}`)"))
                            .unwrap_or_default(),
                        why.unwrap_or_default().join("; ")
                    ));
                }
            }
        }
    }
    (b, refusals)
}

/// `validation/devices.json`: the three build-tier ledgers of spec-0124.
#[must_use]
pub fn devices_json(
    ways: &WayBlockBinding,
    keys: &KeyBinding,
    doors: &DoorsLedger,
) -> serde_json::Value {
    serde_json::json!({
        "way_blocks": {
            "code": DW_DOOR_NO_WAY.id(),
            "pressables": ways.pressables,
            "way_blocks": ways.way_blocks,
            "door_seams": ways.door_seams,
            "gated": ways.gated,
            "refused": ways.refused,
        },
        "keys": {
            "code": DW_EFFECT_OUT_OF_SIGHT.id(),
            "objectives": keys.objectives,
            "keys": keys.keys,
            "writes": keys.writes,
            "in_sight": keys.in_sight,
            "shown": keys.shown,
            "own_release": keys.own_release,
            "refused": keys.refused,
        },
        "doors": doors.to_json(),
    })
}

/// Every `decorative[]` entry of every unturned placed piece's contract, in
/// world cells, with its `what` (spec-0124 §5.4). Every piece a detail plan
/// binds is placed unturned; a turned area piece's entries are not read.
#[must_use]
pub fn decorative_cells(
    plan: &Plan,
    prefabs: &crate::compiler::registry::PrefabRegistry,
) -> Vec<([i32; 3], String)> {
    let mut out = Vec::new();
    for p in plan.areas.iter().flat_map(|a| a.pieces.iter()) {
        let Some(meta) = prefabs.get(p.prefab_id.as_str()) else {
            continue;
        };
        let Some(contract) = meta.spatial_contract.as_ref() else {
            continue;
        };
        if p.rotation != crate::compiler::solver::Rotation::None {
            continue;
        }
        for d in &contract.decorative {
            out.push((
                [p.pos[0] + d.at[0], p.pos[1] + d.at[1], p.pos[2] + d.at[2]],
                d.what.clone(),
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const STONE: &str = "minecraft:stone";

    fn door(id: &str, facing: &str, half: &str) -> String {
        format!("minecraft:{id}[facing={facing},half={half},hinge=left,open=false,powered=false]")
    }

    /// A stone floor at y=0 over x, z in -6..=6, with `extra` on it.
    fn yard(extra: &[([i32; 3], String)]) -> BlockMap {
        let mut m: BTreeMap<[i32; 3], String> = BTreeMap::new();
        for x in -6..=6 {
            for z in -6..=6 {
                m.insert([x, 0, z], STONE.into());
            }
        }
        for (c, b) in extra {
            m.insert(*c, b.clone());
        }
        crate::compiler::blockstate::interned(m)
    }

    /// A door of `id` at [0,1,0] facing north, in a wall along x at z=0
    /// `courses` high (0 = no wall), with `jambs` overriding the four jamb
    /// cells ([-1,1,0], [-1,2,0], [1,1,0], [1,2,0]) and `more` added.
    fn walled(id: &str, courses: i32, more: &[([i32; 3], String)]) -> Vec<([i32; 3], String)> {
        let mut v = Vec::new();
        for x in -6..=6 {
            if x == 0 {
                continue;
            }
            for y in 1..=courses {
                v.push(([x, y, 0], STONE.to_string()));
            }
        }
        v.push(([0, 1, 0], door(id, "north", "lower")));
        v.push(([0, 2, 0], door(id, "north", "upper")));
        v.extend(more.iter().cloned());
        v
    }

    fn judge(blocks: &BlockMap, decorative: &[([i32; 3], String)]) -> (DoorsLedger, Vec<String>) {
        let w = World::from_solid_cells(BTreeSet::new());
        check_doors(
            blocks,
            [-6, 0, -6],
            [6, 4, 6],
            decorative,
            &w,
            &BTreeSet::new,
            0,
        )
    }

    /// **spec-0124 criterion 9a**: a door free on a floor, and beside a
    /// one-course counter, close nothing (clause 1); set in a two-course wall
    /// with floor both sides it is green; with stone behind it, clause 2.
    #[test]
    fn a_door_closes_an_opening_or_is_refused_dw1072() {
        assert_eq!(DW_DOOR_CLOSES_NOTHING.id(), "DW1072");
        let free = yard(&walled("spruce_door", 0, &[]));
        let (l, r) = judge(&free, &[]);
        assert_eq!(r.len(), 1, "{r:?}");
        assert!(l.rows[0].clause1.is_some(), "{:?}", l.rows[0]);
        assert!(r[0].contains("clause 1"), "{}", r[0]);
        assert!(r[0].contains("[-1, 1, 0] open"), "{}", r[0]);

        let stall = yard(&walled(
            "spruce_door",
            0,
            &[([1, 1, 0], "minecraft:spruce_slab[type=top]".to_string())],
        ));
        let (l, r) = judge(&stall, &[]);
        assert_eq!(r.len(), 1, "{r:?}");
        assert!(l.rows[0].clause1.is_some());

        let walled2 = yard(&walled("spruce_door", 2, &[]));
        let (l, r) = judge(&walled2, &[]);
        assert!(r.is_empty(), "{r:?}");
        assert_eq!(l.rows.len(), 1);
        assert_eq!(l.configurations, 2, "C0 and one C_D");

        let onto_stone = yard(&walled(
            "spruce_door",
            2,
            &[([0, 1, 1], STONE.into()), ([0, 2, 1], STONE.into())],
        ));
        let (l, r) = judge(&onto_stone, &[]);
        assert_eq!(r.len(), 1, "{r:?}");
        assert!(l.rows[0].clause1.is_none());
        let c2 = l.rows[0].clause2.as_deref().unwrap();
        assert!(
            c2.contains("[0, 1, 1]") && c2.contains("minecraft:stone"),
            "{c2}"
        );
    }

    /// An iron door and a copper door in the wall are green and counted; a
    /// trapdoor lying on open floor is not entered.
    #[test]
    fn iron_and_copper_doors_close_their_openings_and_a_hatch_is_not_judged() {
        for id in ["iron_door", "copper_door"] {
            let b = yard(&walled(id, 2, &[]));
            let (l, r) = judge(&b, &[]);
            assert!(r.is_empty(), "{id}: {r:?}");
            assert_eq!(l.rows.len(), 1, "{id}");
        }
        let hatch = yard(&[(
            [2, 1, 2],
            "minecraft:oak_trapdoor[facing=north,half=bottom,open=false]".to_string(),
        )]);
        let (l, r) = judge(&hatch, &[]);
        assert!(r.is_empty() && l.rows.is_empty(), "{:?}", l.rows);
    }

    /// A fence gate in its fence line is green; one cell off it, free on the
    /// floor, it is refused on clause 1.
    #[test]
    fn a_gate_in_its_fence_line_closes_it() {
        let gate = "minecraft:oak_fence_gate[facing=north,in_wall=false,open=false,powered=false]";
        let mut line: Vec<([i32; 3], String)> = (-6..=6)
            .filter(|x| *x != 0)
            .map(|x| ([x, 1, 0], "minecraft:oak_fence".to_string()))
            .collect();
        line.push(([0, 1, 0], gate.to_string()));
        let (l, r) = judge(&yard(&line), &[]);
        assert!(r.is_empty(), "{r:?}");
        assert_eq!(l.rows[0].kind, "gate");
        let mut off = line.clone();
        off.retain(|(c, _)| *c != [0, 1, 0]);
        off.push(([0, 1, 0], "minecraft:oak_fence".to_string()));
        off.push(([0, 1, 3], gate.to_string()));
        let (l, r) = judge(&yard(&off), &[]);
        assert_eq!(r.len(), 1, "{r:?}");
        assert!(l.rows[0].clause1.is_some());
    }

    /// **criterion 9b**: two doors in one wall onto one yard are green; the
    /// second's far cell stone refuses it alone on clause 2.
    #[test]
    fn each_door_of_a_room_is_judged_alone() {
        let two = |second_behind_stone: bool| {
            let mut v = walled("oak_door", 2, &[]);
            v.retain(|(c, _)| *c != [3, 1, 0] && *c != [3, 2, 0]);
            v.push(([3, 1, 0], door("oak_door", "north", "lower")));
            v.push(([3, 2, 0], door("oak_door", "north", "upper")));
            if second_behind_stone {
                v.push(([3, 1, 1], STONE.into()));
                v.push(([3, 2, 1], STONE.into()));
            }
            yard(&v)
        };
        let (l, r) = judge(&two(false), &[]);
        assert!(r.is_empty(), "{r:?}");
        assert_eq!(
            (l.rows.len(), l.configurations),
            (2, 3),
            "N + 1 configurations"
        );
        let (l, r) = judge(&two(true), &[]);
        assert_eq!(r.len(), 1, "{r:?}");
        let refused: Vec<[i32; 3]> = l
            .rows
            .iter()
            .filter(|x| x.refusal.is_some())
            .map(|x| x.cell)
            .collect();
        assert_eq!(refused, vec![[3, 1, 0]]);
        assert!(
            l.rows
                .iter()
                .find(|x| x.cell == [3, 1, 0])
                .unwrap()
                .clause1
                .is_none()
        );
    }

    /// **criterion 9c**: an iron door laid flat under a `decorative` entry is
    /// exempt (`no hand opens`) and printed; removing the entry refuses it on
    /// clause 1; an entry aimed at stone is refused.
    #[test]
    fn a_decorative_door_no_hand_opens_is_exempt_and_only_the_exemption_passes_it() {
        let bench = yard(&[
            ([0, 1, 0], door("iron_door", "north", "lower")),
            ([0, 2, 0], door("iron_door", "north", "upper")),
        ]);
        let decl = vec![([0, 1, 0], "the smith's bench top".to_string())];
        let (l, r) = judge(&bench, &decl);
        assert!(r.is_empty(), "{r:?}");
        assert_eq!(l.decorative(), 1);
        assert!(
            l.line()
                .contains("1 declared decorative [[0, 1, 0] (no hand opens)]"),
            "{}",
            l.line()
        );
        assert_eq!(
            l.to_json()["doors"][0]["status"],
            "decorative (no hand opens)"
        );
        let (_, r) = judge(&bench, &[]);
        assert_eq!(r.len(), 1, "the exemption is the only thing that passed it");
        assert!(r[0].contains("clause 1"), "{}", r[0]);
        let (l, r) = judge(&bench, &[([4, 0, 4], "a ghost".to_string())]);
        assert_eq!(r.len(), 2, "{r:?}");
        assert_eq!(l.aimed_at_nothing.len(), 1);
    }

    /// **criterion 9d**: the eye distance is the least between the eye box
    /// over a rest cell and the door cell's box, by hand: a door eight
    /// courses above the street's feet is `√(0.3² + 5.13²)` from the jump apex
    /// over the next street cell; one course lower, `√(0.3² + 4.13²)`.
    #[test]
    fn the_eye_reaches_from_the_crouch_to_the_jump_apex() {
        let (d, eye) = eye_distance([0, 1, -1], 1.0, [0, 9, 0]);
        assert!(
            (d - (0.3f64.powi(2) + 5.13f64.powi(2)).sqrt()).abs() < 1e-9,
            "{d}"
        );
        assert_eq!(eye, "the jump apex");
        assert!(d >= delvewright_dsl::metrics::BLOCK_INTERACTION_RANGE);
        let (d, _) = eye_distance([0, 1, -1], 1.0, [0, 8, 0]);
        assert!(
            (d - (0.3f64.powi(2) + 4.13f64.powi(2)).sqrt()).abs() < 1e-9,
            "{d}"
        );
        assert!(d < delvewright_dsl::metrics::BLOCK_INTERACTION_RANGE);
        assert_eq!(delvewright_dsl::metrics::MAX_JUMP_RISE_16, 20);
    }

    /// A wooden loft door declared decorative is exempt out of reach, and
    /// refused within reach naming the rest cell and the distance; a stoop
    /// under it (a new rest cell) refuses it.
    #[test]
    fn a_decorative_wooden_door_passes_only_out_of_reach() {
        let loft = |y: i32| {
            yard(&[
                ([0, y, 0], door("oak_door", "north", "lower")),
                ([0, y + 1, 0], door("oak_door", "north", "upper")),
                ([0, y - 1, 1], STONE.into()),
            ])
        };
        let decl = vec![([0, 9, 0], "a loft door".to_string())];
        let street: BTreeSet<[i32; 3]> = [[0, 1, -1], [1, 1, -1], [-1, 1, -1]].into();
        let w = World::from_solid_cells(BTreeSet::new());
        let run = |b: &BlockMap, decl: &[([i32; 3], String)], rest: BTreeSet<[i32; 3]>| {
            check_doors(
                b,
                [-6, 0, -6],
                [6, 12, 6],
                decl,
                &w,
                &move || rest.clone(),
                0,
            )
        };
        let (l, r) = run(&loft(9), &decl, street.clone());
        assert!(r.is_empty(), "{r:?}");
        let row = &l.rows[0];
        assert_eq!(row.decorative.as_ref().unwrap().1, "out of reach");
        let (d, from, eye) = row.reach.unwrap();
        assert!((d - 5.1388).abs() < 1e-3, "{d}");
        assert_eq!((from, eye), ([0, 1, -1], "the jump apex"));
        let decl8 = vec![([0, 8, 0], "a loft door".to_string())];
        let (_, r) = run(&loft(8), &decl8, street.clone());
        assert_eq!(r.len(), 1, "{r:?}");
        assert!(
            r[0].contains("[0, 1, -1]") && r[0].contains("4.14"),
            "{}",
            r[0]
        );
        assert!(
            r[0].contains("Raise it out of reach, or make it close a way"),
            "{}",
            r[0]
        );
        let mut stoop = street.clone();
        stoop.insert([0, 6, -1]);
        let (_, r) = run(&loft(9), &decl, stoop);
        assert_eq!(r.len(), 1, "{r:?}");
        assert!(r[0].contains("[0, 6, -1]"), "{}", r[0]);
    }

    /// **spec-0124 criterion 8, the predicate half**: `DW1071` asks `DW0863`'s
    /// own sight predicate (`promise::act_unfound`) of a key's cell: a region
    /// behind a full wall is out of sight and named by its blocking cell, the
    /// same wall of glass is looked through, and a region past 64 blocks is
    /// out of sight by distance.
    #[test]
    fn a_key_sees_by_the_one_sight_predicate_dw1071() {
        assert_eq!(DW_EFFECT_OUT_OF_SIGHT.id(), "DW1071");
        let ground = |wall: Option<&str>, far: i32| {
            let mut m: BTreeMap<[i32; 3], String> = BTreeMap::new();
            for x in -2..=far + 2 {
                for z in -2..=2 {
                    m.insert([x, 0, z], STONE.into());
                }
            }
            if let Some(w) = wall {
                for z in -2..=2 {
                    for y in 1..=6 {
                        m.insert([5, y, z], w.to_string());
                    }
                }
            }
            World::from_occupancy(
                crate::compiler::assembled::occupancy_of(m, &BTreeSet::new()),
                Premises::geometry_only(),
            )
        };
        let unfound = |w: &World, target: [i32; 3]| {
            crate::compiler::promise::act_unfound(w, [0, 1, 0], target, false, None)
        };
        let walled = ground(Some(STONE), 12);
        let why = unfound(&walled, [10, 1, 0]);
        assert!(!why.is_empty() && why[0].contains("hidden"), "{why:?}");
        let glass = ground(Some("minecraft:glass"), 12);
        assert!(unfound(&glass, [10, 1, 0]).is_empty());
        let open = ground(None, 72);
        let why = unfound(&open, [70, 1, 0]);
        assert!(why[0].contains("past the 64 blocks"), "{why:?}");
        let b = KeyBinding {
            objectives: 7,
            keys: 2,
            writes: 2,
            in_sight: 1,
            shown: 1,
            ..KeyBinding::default()
        };
        assert_eq!(
            b.line(),
            "key binding (DW1071): 2 key objective(s) among 7; 2 write(s) judged, 1 in sight, 1 \
             shown by a shot, 0 the key's own door released; 0 refused"
        );
    }

    /// **The agreement** between the census's framing and the reach graph's
    /// clause 1: over every assignment of stone, air or a slab to the four
    /// jamb cells, a door whose jambs all stop a body closes its opening; an
    /// unframed one is refused unless the reach graph finds no body fits the
    /// gap, and that count is stated.
    #[test]
    fn the_census_framing_and_clause_one_agree_over_every_jamb() {
        let kinds = [Some(STONE), None, Some("minecraft:stone_slab[type=bottom]")];
        let jambs = [[-1, 1, 0], [-1, 2, 0], [1, 1, 0], [1, 2, 0]];
        let mut judged = 0;
        let mut holes_no_body_passes = 0;
        for code in 0..81u32 {
            let mut v = walled("oak_door", 0, &[]);
            let mut k = code;
            for j in jambs {
                if let Some(b) = kinds[(k % 3) as usize] {
                    v.push((j, b.to_string()));
                }
                k /= 3;
            }
            let b = yard(&v);
            let entries = super::super::census::doors(&Reading::new(&b), [-6, 0, -6], [6, 4, 6]);
            let framed = entries[0]
                .framing
                .iter()
                .filter(|f| f.role == "jamb")
                .all(|f| f.present);
            let (c1, _) = closes(&b, &entries[0]);
            if framed {
                assert!(c1.is_none(), "jambs {code}: framed, yet {c1:?}");
            } else if c1.is_none() {
                // An unframed opening no body passes: a jamb gap under a
                // block a body cannot stand in — the graph's answer rules.
                holes_no_body_passes += 1;
            }
            judged += 1;
        }
        assert_eq!(judged, 81);
        // The census's framing is a sufficient condition and the reach graph
        // is the authority: of the unframed assignments, the graph closes the
        // 20 whose gaps no body fits through (measured).
        assert_eq!(holes_no_body_passes, 20, "measured");
    }
}
