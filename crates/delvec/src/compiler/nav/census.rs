//! **The piece census** (spec-0123 §3.5): what one building is made of, stated
//! for the review and never a refusal.
//!
//! Whether a door was meant to stand in a wall, whether the heads of a front's
//! openings were meant to share a line, whether a third block kind has a job
//! and whether a thing in a square reads as a fountain are the author's intent
//! and the reviewer's eye. No number here decides any of them, so nothing here
//! refuses: the census measures what a body cannot be in and what a face is
//! made of, and `validation/piece-census.json` hands the measurement to the
//! review, which reads it beside the threshold frame
//! ([`crate::compiler::render_plan`]).
//!
//! # What is entered, per bound piece
//!
//! A bound piece is one the detail plan binds to a site-plan place: the
//! placed piece of that row whose frame holds the place's play space.
//!
//! 1. **Doors and gates, and their framing.** Every `*_door` lower half: its
//!    two jambs — the cells beside it perpendicular to its `facing`, at both of
//!    its courses — and the cell over its upper half. Every `*_fence_gate`: the
//!    two cells beside it along its axis, at its course. Each framing cell is
//!    recorded present (a cell a body cannot be in) or absent. Another door
//!    block of the same kind beside a door is the other leaf of a double door,
//!    and that leaf's far jamb is the one recorded. Trapdoors are not entered:
//!    a hatch lies in a floor or a wall that is its own frame. The derivation's
//!    own bars stand in the ring the whole writes and are not a door block.
//! 2. **Openings per face.** Per shown face (the prefab's `shown_faces`,
//!    rotated into the world) and per open side's first inward layer: every
//!    4-connected run of cells a body can be in, from the floor course to the
//!    top of the play space, an open side's own cells excluded — its head and
//!    sill course against the walk plane, and its width.
//! 3. **Kinds per face.** The distinct block ids of the face's cells, states
//!    stripped, with each one's cell count and its longest straight run.
//! 4. **Water.** The piece's water cells, with the highest and lowest course
//!    against the walk plane.
//!
//! # The predicate, cited and authored
//!
//! A framing cell is present when a body cannot be in it by the nav model's one
//! predicate (`can_be`, spec-0123 §3.1) **and** the cell is not free air: the
//! predicate reads standing, so it answers *no body here* for an upper course
//! over open air as surely as for a wall, and a jamb of air two courses up is
//! no jamb. A cell a body can be in is an opening's; so is a door, trapdoor or
//! fence-gate block, which is the leaf of an opening rather than its wall.

use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::blockshape::{bare_id, state_value};
use serde_json::{Value, json};

use super::World;
use crate::compiler::blockstate::BlockMap;
use crate::compiler::plan::Plan;
use crate::compiler::registry::PrefabRegistry;

/// One framing cell of a door or a gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Framing {
    /// `jamb` (a door's or a gate's side) or `head` (over a door).
    pub role: &'static str,
    /// The cell.
    pub cell: [i32; 3],
    /// Whether a body cannot be in it.
    pub present: bool,
}

/// One door or gate, entered with its framing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoorEntry {
    /// The lower half's (or the gate's) cell.
    pub cell: [i32; 3],
    /// Its block id, states stripped.
    pub block: String,
    /// The other leaf of a double door, when one stands beside it.
    pub pair: Option<[i32; 3]>,
    /// Every framing cell, jambs first.
    pub framing: Vec<Framing>,
}

impl DoorEntry {
    /// Every framing cell present.
    #[must_use]
    pub fn framed(&self) -> bool {
        self.framing.iter().all(|f| f.present)
    }
}

/// One opening of a face: a 4-connected run of cells a body can be in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opening {
    /// Its top course, against the walk plane (`0` is the walk plane).
    pub head: i32,
    /// Its bottom course, against the walk plane.
    pub sill: i32,
    /// Its width along the face.
    pub width: i32,
    /// Its cells.
    pub cells: usize,
    /// Its low corner along the face, for a reader looking for it.
    pub from: [i32; 3],
}

/// One block kind of a face.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kind {
    /// The block id, states stripped.
    pub block: String,
    /// Its cells in the face.
    pub cells: usize,
    /// Its longest unbroken straight run, along the face or up it.
    pub longest_run: usize,
}

/// One face of a piece.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceEntry {
    /// The world side.
    pub face: &'static str,
    /// `shown` (a shown face's ring) or `open side` (an open side's first
    /// inward layer).
    pub layer: &'static str,
    /// Its openings, in face order.
    pub openings: Vec<Opening>,
    /// Its block kinds, by id.
    pub kinds: Vec<Kind>,
}

/// The piece's water.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Water {
    /// Water cells (a water block, or a waterlogged one).
    pub cells: usize,
    /// The highest course holding water, against the walk plane.
    pub highest: Option<i32>,
    /// The lowest.
    pub lowest: Option<i32>,
}

/// One bound piece's entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PieceEntry {
    /// The place.
    pub node: String,
    /// The piece.
    pub prefab: String,
    /// The walk plane's world `y`.
    pub walk_y: i32,
    /// Its doors and gates.
    pub doors: Vec<DoorEntry>,
    /// Its shown faces and open sides.
    pub faces: Vec<FaceEntry>,
    /// Its water.
    pub water: Water,
}

/// **The census of every bound piece** — the measurement the review reads.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Census {
    /// One entry per bound piece, in plan box order.
    pub pieces: Vec<PieceEntry>,
}

impl Census {
    fn totals(&self) -> (usize, usize, usize, usize, usize) {
        let faces = self.pieces.iter().map(|p| p.faces.len()).sum();
        let openings = self
            .pieces
            .iter()
            .flat_map(|p| p.faces.iter())
            .map(|f| f.openings.len())
            .sum();
        let water = self.pieces.iter().map(|p| p.water.cells).sum();
        let doors = self.pieces.iter().map(|p| p.doors.len()).sum();
        let framed = self
            .pieces
            .iter()
            .flat_map(|p| p.doors.iter())
            .filter(|d| d.framed())
            .count();
        (faces, openings, water, doors, framed)
    }

    /// `piece census: P piece(s), F face(s), O opening(s), W water cell(s);
    /// doors framed: D of N` — zeroes stated.
    #[must_use]
    pub fn line(&self) -> String {
        let (faces, openings, water, doors, framed) = self.totals();
        format!(
            "piece census: {} piece(s), {faces} face(s), {openings} opening(s), {water} water \
             cell(s); doors framed: {framed} of {doors}",
            self.pieces.len()
        )
    }

    /// `validation/piece-census.json`.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let (faces, openings, water, doors, framed) = self.totals();
        let pieces: Vec<Value> = self
            .pieces
            .iter()
            .map(|p| {
                json!({
                    "node": p.node,
                    "prefab": p.prefab,
                    "walk_y": p.walk_y,
                    "doors": p.doors.iter().map(|d| json!({
                        "cell": d.cell,
                        "block": d.block,
                        "pair": d.pair,
                        "framed": d.framed(),
                        "framing": d.framing.iter().map(|f| json!({
                            "role": f.role,
                            "cell": f.cell,
                            "present": f.present,
                        })).collect::<Vec<_>>(),
                    })).collect::<Vec<_>>(),
                    "faces": p.faces.iter().map(|f| json!({
                        "face": f.face,
                        "layer": f.layer,
                        "openings": f.openings.iter().map(|o| json!({
                            "head": o.head,
                            "sill": o.sill,
                            "width": o.width,
                            "cells": o.cells,
                            "from": o.from,
                        })).collect::<Vec<_>>(),
                        "kinds": f.kinds.iter().map(|k| json!({
                            "block": k.block,
                            "cells": k.cells,
                            "longest_run": k.longest_run,
                        })).collect::<Vec<_>>(),
                    })).collect::<Vec<_>>(),
                    "water": {
                        "cells": p.water.cells,
                        "highest": p.water.highest,
                        "lowest": p.water.lowest,
                    },
                })
            })
            .collect();
        json!({
            "census": {
                "pieces": self.pieces.len(),
                "faces": faces,
                "openings": openings,
                "water_cells": water,
                "doors": doors,
                "doors_framed": framed,
            },
            "pieces": pieces,
        })
    }
}

/// The blocks and the body model one piece is read in.
pub struct Reading<'a> {
    blocks: &'a BlockMap,
    world: World,
}

impl<'a> Reading<'a> {
    /// The body model over `blocks`, geometry only: whether a body can be in
    /// a cell beside a door, and what a face is made of, are facts of the
    /// blocks — a measurement for the review that refuses nothing — and no
    /// premise of the campaign (horizon, volume, gate) changes them.
    #[must_use]
    pub fn new(blocks: &'a BlockMap) -> Self {
        let world = World::from_occupancy(
            crate::compiler::assembled::occupancy_over(blocks, &BTreeSet::new()),
            super::Premises::geometry_only(),
        );
        Reading { blocks, world }
    }

    fn id(&self, c: [i32; 3]) -> Option<&'static str> {
        self.blocks.get(&c).map(|b| b.as_str())
    }

    /// A leaf: a door, trapdoor or fence-gate block.
    fn leaf(&self, c: [i32; 3]) -> bool {
        self.id(c).is_some_and(|b| {
            let bare = bare_id(b);
            bare.ends_with("_door") || bare.ends_with("_trapdoor") || bare.ends_with("_fence_gate")
        })
    }

    /// A cell a body cannot be in, and not free air (module docs).
    fn present(&self, c: [i32; 3]) -> bool {
        !super::shutmodel::can_be(&self.world, c)
            && (!self.world.is_clear(c) || self.world.is_barrier(c))
    }

    /// A cell of an opening: a body can be in it, or it holds a leaf.
    fn opening(&self, c: [i32; 3]) -> bool {
        self.leaf(c) || !self.present(c)
    }
}

const DOOR_FACES: [(&str, [i32; 3]); 4] = [
    ("north", [0, 0, -1]),
    ("south", [0, 0, 1]),
    ("east", [1, 0, 0]),
    ("west", [-1, 0, 0]),
];

fn facing_vec(b: &str) -> [i32; 3] {
    let f = state_value(b, "facing").unwrap_or("north");
    DOOR_FACES
        .iter()
        .find(|(n, _)| *n == f)
        .map_or([0, 0, -1], |(_, v)| *v)
}

fn add(a: [i32; 3], b: [i32; 3], k: i32) -> [i32; 3] {
    [a[0] + b[0] * k, a[1] + b[1] * k, a[2] + b[2] * k]
}

/// Every door and gate in `lo..=hi`, entered with its framing.
#[must_use]
pub fn doors(r: &Reading<'_>, lo: [i32; 3], hi: [i32; 3]) -> Vec<DoorEntry> {
    let mut out = Vec::new();
    for (&c, b) in r.blocks.range(lo..=hi) {
        if !(lo[0]..=hi[0]).contains(&c[0])
            || !(lo[1]..=hi[1]).contains(&c[1])
            || !(lo[2]..=hi[2]).contains(&c[2])
        {
            continue;
        }
        let name = b.as_str();
        let bare = bare_id(name);
        let f = facing_vec(name);
        // The axis across the leaf: perpendicular to its facing, horizontal.
        let side = [f[2].abs(), 0, f[0].abs()];
        if bare.ends_with("_door") && state_value(name, "half") == Some("lower") {
            let mut framing = Vec::new();
            let mut pair = None;
            for k in [-1, 1] {
                let next = add(c, side, k);
                let jamb = if r.id(next).is_some_and(|n| bare_id(n) == bare) {
                    pair = Some(next);
                    add(c, side, 2 * k)
                } else {
                    next
                };
                for dy in 0..2 {
                    let cell = [jamb[0], jamb[1] + dy, jamb[2]];
                    framing.push(Framing {
                        role: "jamb",
                        cell,
                        present: r.present(cell),
                    });
                }
            }
            let head = [c[0], c[1] + 2, c[2]];
            framing.push(Framing {
                role: "head",
                cell: head,
                present: r.present(head),
            });
            out.push(DoorEntry {
                cell: c,
                block: bare.to_string(),
                pair,
                framing,
            });
        } else if bare.ends_with("_fence_gate") {
            let framing = [-1, 1]
                .into_iter()
                .map(|k| {
                    let cell = add(c, side, k);
                    Framing {
                        role: "jamb",
                        cell,
                        present: r.present(cell),
                    }
                })
                .collect();
            out.push(DoorEntry {
                cell: c,
                block: bare.to_string(),
                pair: None,
                framing,
            });
        }
    }
    out
}

/// One layer of a face: a plane at `t` on axis `n`, `along` over `a0..=a1`,
/// courses `y0..=y1`, read against the walk plane `walk_y`.
#[derive(Debug, Clone, Copy)]
pub struct Layer {
    /// The world side it faces.
    pub face: &'static str,
    /// `shown` or `open side`.
    pub layer: &'static str,
    /// The thin axis: 0 or 2.
    pub n: usize,
    /// The plane's coordinate on it.
    pub t: i32,
    /// The run along the face, inclusive.
    pub a0: i32,
    /// Its high end.
    pub a1: i32,
    /// The floor course.
    pub y0: i32,
    /// The top of the play space.
    pub y1: i32,
    /// The walk plane.
    pub walk_y: i32,
}

impl Layer {
    fn cell(&self, u: i32, y: i32) -> [i32; 3] {
        let mut c = [0, y, 0];
        c[self.n] = self.t;
        c[2 - self.n] = u;
        c
    }
}

/// The openings and the kinds of one layer.
#[must_use]
pub fn face(r: &Reading<'_>, l: &Layer) -> FaceEntry {
    let mut open: BTreeSet<(i32, i32)> = BTreeSet::new();
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    for u in l.a0..=l.a1 {
        for y in l.y0..=l.y1 {
            let c = l.cell(u, y);
            if r.opening(c) {
                open.insert((u, y));
            }
            if let Some(b) = r.id(c) {
                let bare = bare_id(b);
                if !matches!(bare, "air" | "cave_air" | "structure_void") {
                    *kinds.entry(bare.to_string()).or_default() += 1;
                }
            }
        }
    }
    // 4-connected runs over the open cells, in face order.
    let mut seen: BTreeSet<(i32, i32)> = BTreeSet::new();
    let mut openings = Vec::new();
    for &start in &open {
        if !seen.insert(start) {
            continue;
        }
        let mut stack = vec![start];
        let mut run = vec![start];
        while let Some((u, y)) = stack.pop() {
            for nb in [(u - 1, y), (u + 1, y), (u, y - 1), (u, y + 1)] {
                if open.contains(&nb) && seen.insert(nb) {
                    stack.push(nb);
                    run.push(nb);
                }
            }
        }
        let umin = run.iter().map(|c| c.0).min().unwrap_or(0);
        let umax = run.iter().map(|c| c.0).max().unwrap_or(0);
        let ymin = run.iter().map(|c| c.1).min().unwrap_or(0);
        let ymax = run.iter().map(|c| c.1).max().unwrap_or(0);
        openings.push(Opening {
            head: ymax - l.walk_y,
            sill: ymin - l.walk_y,
            width: umax - umin + 1,
            cells: run.len(),
            from: l.cell(umin, ymin),
        });
    }
    let kinds = kinds
        .into_iter()
        .map(|(block, cells)| {
            let same = |u: i32, y: i32| r.id(l.cell(u, y)).is_some_and(|b| bare_id(b) == block);
            let mut longest = 0usize;
            for y in l.y0..=l.y1 {
                let mut run = 0usize;
                for u in l.a0..=l.a1 {
                    run = if same(u, y) { run + 1 } else { 0 };
                    longest = longest.max(run);
                }
            }
            for u in l.a0..=l.a1 {
                let mut run = 0usize;
                for y in l.y0..=l.y1 {
                    run = if same(u, y) { run + 1 } else { 0 };
                    longest = longest.max(run);
                }
            }
            Kind {
                block,
                cells,
                longest_run: longest,
            }
        })
        .collect();
    FaceEntry {
        face: l.face,
        layer: l.layer,
        openings,
        kinds,
    }
}

/// The water in `lo..=hi`, against the walk plane.
#[must_use]
pub fn water(r: &Reading<'_>, lo: [i32; 3], hi: [i32; 3], walk_y: i32) -> Water {
    let mut w = Water::default();
    for (&c, b) in r.blocks.range(lo..=hi) {
        if !(lo[0]..=hi[0]).contains(&c[0])
            || !(lo[1]..=hi[1]).contains(&c[1])
            || !(lo[2]..=hi[2]).contains(&c[2])
        {
            continue;
        }
        let name = b.as_str();
        let wet = matches!(bare_id(name), "water" | "bubble_column")
            || state_value(name, "waterlogged") == Some("true");
        if wet {
            let y = c[1] - walk_y;
            w.cells += 1;
            w.highest = Some(w.highest.map_or(y, |h| h.max(y)));
            w.lowest = Some(w.lowest.map_or(y, |h| h.min(y)));
        }
    }
    w
}

fn narrow(v: i64) -> i32 {
    i32::try_from(v).unwrap_or(if v < 0 { i32::MIN } else { i32::MAX })
}

/// **The census over a built world** — every bound piece of the plan, read in
/// the blocks as shipped.
#[must_use]
pub fn census(plan: &Plan, prefabs: &PrefabRegistry, blocks: &BlockMap) -> Census {
    let mut out = Census::default();
    let Some(b) = plan.blockout.as_ref() else {
        return out;
    };
    let placements: Vec<&crate::compiler::plan::PiecePlacement> =
        plan.areas.iter().flat_map(|a| a.pieces.iter()).collect();
    let bound: BTreeMap<&str, &str> = plan
        .campaign
        .detail_plan
        .as_ref()
        .map(|d| {
            d.content
                .details
                .iter()
                .map(|r| (r.place.0.as_str(), r.piece.as_str()))
                .collect()
        })
        .unwrap_or_default();
    for pb in &b.boxes {
        let Some(piece) = bound.get(pb.node.0.as_str()) else {
            continue;
        };
        let (slo, shi) = pb.space();
        let (slo, shi) = (slo.map(narrow), shi.map(narrow));
        let Some(p) = placements.iter().find(|p| {
            let (min, max) = p.bbox();
            p.prefab_id == *piece && (0..3).all(|i| min[i] <= slo[i] && shi[i] <= max[i])
        }) else {
            continue;
        };
        let (min, max) = p.bbox();
        // The piece's blocks with a margin of one, so a framing cell on the
        // frame's edge reads its neighbour.
        let window: BlockMap = blocks
            .range([min[0] - 2, i32::MIN, i32::MIN]..=[max[0] + 2, i32::MAX, i32::MAX])
            .filter(|(c, _)| {
                (min[1] - 2..=max[1] + 2).contains(&c[1])
                    && (min[2] - 2..=max[2] + 2).contains(&c[2])
            })
            .map(|(c, s)| (*c, *s))
            .collect();
        let r = Reading::new(&window);
        let walk_y = narrow(pb.floor);
        let shown: Vec<&'static str> = prefabs
            .get(&p.prefab_id)
            .map(|m| {
                m.shown_faces
                    .iter()
                    .filter_map(|s| crate::compiler::faces::dir_vector(s))
                    .map(|v| crate::compiler::faces::rotate_dir(p.rotation, v))
                    .map(crate::compiler::faces::dir_name)
                    .collect()
            })
            .unwrap_or_default();
        let [x0, x1, z0, z1] = pb.foot.map(narrow);
        let open: Vec<&'static str> = pb.open_sides().iter().map(|s| s.face.as_str()).collect();
        let mut faces = Vec::new();
        for (name, n, ring, inner, a0, a1) in [
            ("east", 0usize, x1 + 1, x1, z0, z1),
            ("north", 2, z0 - 1, z0, x0, x1),
            ("south", 2, z1 + 1, z1, x0, x1),
            ("west", 0, x0 - 1, x0, z0, z1),
        ] {
            let is_open = open.contains(&name);
            if !is_open && !shown.contains(&name) {
                continue;
            }
            let layer = Layer {
                face: name,
                layer: if is_open { "open side" } else { "shown" },
                n,
                t: if is_open { inner } else { ring },
                a0,
                a1,
                y0: walk_y - 1,
                y1: shi[1],
                walk_y,
            };
            faces.push(face(&r, &layer));
        }
        out.pieces.push(PieceEntry {
            node: pb.node.0.clone(),
            prefab: p.prefab_id.clone(),
            walk_y,
            doors: doors(&r, min, max),
            faces,
            water: water(&r, min, max, walk_y),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::blockstate::BlockState;

    const STONE: &str = "minecraft:stone";
    const DOOR_LO: &str =
        "minecraft:oak_door[facing=south,half=lower,hinge=left,open=false,powered=false]";
    const DOOR_HI: &str =
        "minecraft:oak_door[facing=south,half=upper,hinge=left,open=false,powered=false]";
    const COUNTER: &str = "minecraft:spruce_slab[type=top,waterlogged=false]";
    const BEAM: &str = "minecraft:stripped_spruce_log[axis=x]";

    /// The ruling's smithy (spec-0123 §3.5): a floor at y 0, the play space
    /// from y 1, an open south side at z 6, and one cell in from it (z 5) a
    /// one-course counter at x 2..=3 and a two-course oak door at x 5, air over
    /// both.
    fn smithy() -> BlockMap {
        let mut m = BlockMap::new();
        for x in 0..=8 {
            for z in 0..=8 {
                m.insert([x, 0, z], BlockState::new(STONE));
            }
        }
        for x in 2..=3 {
            m.insert([x, 1, 5], BlockState::new(COUNTER));
        }
        m.insert([5, 1, 5], BlockState::new(DOOR_LO));
        m.insert([5, 2, 5], BlockState::new(DOOR_HI));
        m
    }

    #[test]
    fn the_rulings_door_is_entered_with_its_head_absent() {
        let m = smithy();
        let r = Reading::new(&m);
        let d = doors(&r, [0, 0, 0], [8, 6, 8]);
        assert_eq!(d.len(), 1, "{d:?}");
        let head = d[0].framing.iter().find(|f| f.role == "head").unwrap();
        assert_eq!(head.cell, [5, 3, 5]);
        assert!(!head.present, "air over the door is no head");
        assert!(!d[0].framed());
        let c = Census {
            pieces: vec![PieceEntry {
                node: "node/smithy".into(),
                prefab: "prefab/smithy".into(),
                walk_y: 1,
                doors: d,
                faces: Vec::new(),
                water: Water::default(),
            }],
        };
        assert!(c.line().ends_with("doors framed: 0 of 1"), "{}", c.line());
    }

    #[test]
    fn a_beam_and_posts_frame_the_door() {
        let mut m = smithy();
        for x in 2..=8 {
            m.insert([x, 3, 5], BlockState::new(BEAM));
        }
        for y in 1..=2 {
            m.insert([4, y, 5], BlockState::new(STONE));
            m.insert([6, y, 5], BlockState::new(STONE));
        }
        let r = Reading::new(&m);
        let d = doors(&r, [0, 0, 0], [8, 6, 8]);
        assert!(d[0].framed(), "{d:?}");
        let c = Census {
            pieces: vec![PieceEntry {
                node: "node/smithy".into(),
                prefab: "prefab/smithy".into(),
                walk_y: 1,
                doors: d,
                faces: Vec::new(),
                water: Water::default(),
            }],
        };
        assert!(c.line().ends_with("doors framed: 1 of 1"), "{}", c.line());
    }

    #[test]
    fn a_double_door_records_its_far_jambs() {
        let mut m = smithy();
        m.insert([6, 1, 5], BlockState::new(DOOR_LO));
        m.insert([6, 2, 5], BlockState::new(DOOR_HI));
        let r = Reading::new(&m);
        let d = doors(&r, [0, 0, 0], [8, 6, 8]);
        assert_eq!(d.len(), 2);
        assert_eq!(d[0].pair, Some([6, 1, 5]));
        assert!(d[0].framing.iter().any(|f| f.cell == [7, 1, 5]));
        assert!(!d[0].framing.iter().any(|f| f.cell == [6, 1, 5]));
    }

    #[test]
    fn a_face_states_its_openings_and_kinds() {
        let mut m = smithy();
        // A wall at z 5 from x 0 to 8, courses 1..=3, with the counter's and
        // the door's cells left as they stand.
        for x in 0..=8 {
            for y in 1..=3 {
                if (x == 5 && y <= 2) || ((2..=3).contains(&x) && y >= 2) {
                    continue;
                }
                if (2..=3).contains(&x) && y == 1 {
                    continue;
                }
                m.insert([x, y, 5], BlockState::new(STONE));
            }
        }
        let r = Reading::new(&m);
        let l = Layer {
            face: "south",
            layer: "shown",
            n: 2,
            t: 5,
            a0: 0,
            a1: 8,
            y0: 0,
            y1: 3,
            walk_y: 1,
        };
        let f = face(&r, &l);
        assert_eq!(f.openings.len(), 2, "{f:?}");
        let counter = &f.openings[0];
        assert_eq!((counter.sill, counter.head, counter.width), (1, 2, 2));
        let door = &f.openings[1];
        assert_eq!((door.sill, door.head, door.width), (0, 1, 1));
        assert!(
            f.kinds
                .iter()
                .any(|k| k.block == "stone" && k.longest_run == 9)
        );
    }

    #[test]
    fn water_is_counted_against_the_walk_plane() {
        let mut m = smithy();
        m.insert([1, 2, 1], BlockState::new("minecraft:water[level=0]"));
        m.insert([1, 1, 1], BlockState::new("minecraft:water[level=0]"));
        let r = Reading::new(&m);
        let w = water(&r, [0, 0, 0], [8, 6, 8], 1);
        assert_eq!((w.cells, w.highest, w.lowest), (2, Some(1), Some(0)));
        let none = water(&r, [0, 0, 0], [0, 0, 0], 1);
        assert_eq!((none.cells, none.highest), (0, None));
    }
}
