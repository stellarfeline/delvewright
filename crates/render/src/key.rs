//! The **plan key**: what the program knows about a building, drawn.
//!
//! # The defect this exists to close
//!
//! A candidate handed to a reviewer as one three-quarter render is a grey solid.
//! It cannot answer the questions a massing decision is actually made of —
//! *where does the party come in, where does it leave, which cells can be walked
//! on, where is every declared anchor* — and the pipeline knew all of them
//! before it drew anything: the rules declare their anchors, and the walkable
//! floor is derived. Dropping them before the picture is drawn is the
//! computed-then-discarded defect; a reviewer being asked to mentally compile a
//! grey mass back into a plan is the cost.
//!
//! So the key is the compiled reality rendered back into the reviewer's medium:
//! a plan of the piece with its floor shaded by level, its boundary openings
//! marked, and every declared anchor numbered on the plan and named underneath
//! with its position, facing and the rule that put it there.
//!
//! # It draws only what it is told, and says which parts nobody told it
//!
//! Standability is [`crate::meta::Floor`], computed by the producer of the piece
//! (`delvewright-grammar`'s `floor`, the same rule the generator's own gates
//! assert with) and read here as data. This module derives no geometry of its
//! own beyond *is there any block in this column*, which is a fact about the
//! `.nbt` and not a claim about walking. A piece whose metadata carries no floor
//! gets a footprint plan and a header line saying so — never an inferred one,
//! because a picture that disagreed with a gate about where a player can stand
//! would be worse than no picture.
//!
//! **Entry and exit are authored, not derived.** The key marks them when a
//! producer declares them and otherwise says `entry/exit: not declared` on the
//! page. It never nominates an opening as the door: which way in a building has
//! is a design decision, and inventing one on a curation page is inventing the
//! decision the page exists to ask for.
//!
//! # Determinism
//!
//! Pure CPU: integer arithmetic, the built-in bitmap font, no GPU and no
//! textures. Same inputs, byte-identical PNG — stronger than the GPU shots,
//! which are pixel-stable rather than byte-stable.

use std::collections::BTreeSet;

use crate::diag::{DW_KEY_BINDING, Diagnostic};
use crate::font;
use crate::meta::{Floor, PrefabMeta};
use crate::nbt::Structure;

/// How much of the piece the key actually annotated.
///
/// Every validation artifact states its binding count (CLAUDE.md), and this one
/// is the count whose silence let the gap survive: nothing ever said *0 anchors
/// drawn*. A key that annotated nothing looks exactly like a key of a piece with
/// nothing to annotate unless it is made to say which it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyBinding {
    /// Anchors in the metadata.
    pub anchors_total: usize,
    /// Of those, the ones that resolved to a cell and reached the plan.
    pub anchors_drawn: usize,
    /// Boundary opening cells drawn.
    pub openings_drawn: usize,
    /// Columns of the plan with a walkable floor.
    pub standable_columns: usize,
    /// Columns of the plan holding any block at all.
    pub occupied_columns: usize,
    /// Whether a floor plan was supplied. `false` means the piece is drawn as a
    /// footprint, and the page says so.
    pub floor_supplied: bool,
    /// Anchors naming the way in / the way onward.
    pub ways_declared: usize,
}

impl KeyBinding {
    /// True when the key annotated no anchor at all — a finding, not a pass.
    pub fn binds_to_nothing(&self) -> bool {
        self.anchors_drawn == 0
    }

    /// The zero-binding finding (`DW0729`), or nothing.
    ///
    /// Warning tier rather than error: an unannotated key is still an honest
    /// picture of a shape, and the repair is upstream in the program that
    /// declares no `mark` — no amount of rendering recovers an anchor nobody
    /// declared. What it must never do is pass silently.
    pub fn diagnose(&self, stem: &str) -> Option<Diagnostic> {
        if !self.binds_to_nothing() {
            return None;
        }
        Some(Diagnostic::warning(
            DW_KEY_BINDING,
            format!(
                "{stem}: the plan key annotated 0 of {} anchor(s) — the page shows a shape and \
                 names nothing on it. An anchor is DECLARED by a rule (`mark`); no amount of \
                 rendering can recover one that was never declared",
                self.anchors_total
            ),
        ))
    }
}

impl std::fmt::Display for KeyBinding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "key bound to {}/{} anchor(s), {} opening(s), {} floor column(s){}",
            self.anchors_drawn,
            self.anchors_total,
            self.openings_drawn,
            self.standable_columns,
            if self.floor_supplied {
                ""
            } else {
                " (no floor supplied - footprint only)"
            }
        )
    }
}

const BG: [u8; 4] = [20, 20, 24, 255];
const PANEL: [u8; 4] = [32, 32, 38, 255];
const VOID: [u8; 4] = [28, 28, 34, 255];
const MASS: [u8; 4] = [72, 72, 80, 255];
const FG: [u8; 4] = [230, 230, 236, 255];
const DIM: [u8; 4] = [150, 150, 162, 255];
const WARN: [u8; 4] = [250, 196, 90, 255];
const OPENING: [u8; 4] = [96, 208, 255, 255];
const ANCHOR: [u8; 4] = [255, 116, 96, 255];
const WAY: [u8; 4] = [120, 240, 150, 255];
const PAD: u32 = 10;
const SCALE: u32 = 2;

/// The floor ramp: low ground cool and dark, high ground warm and light, so
/// storeys and stair runs read as a gradient rather than as one flat tone.
fn floor_tone(y: i32, span: (i32, i32)) -> [u8; 4] {
    let (lo, hi) = span;
    let t = if hi > lo {
        ((y - lo) as f32) / ((hi - lo) as f32)
    } else {
        0.0
    };
    let r = (40.0 + t * 190.0) as u8;
    let g = (66.0 + t * 148.0) as u8;
    let b = (110.0 + t * 20.0) as u8;
    [r, g, b, 255]
}

/// One anchor resolved onto the plan.
struct Marked {
    n: usize,
    x: i32,
    z: i32,
    y: i32,
    name: String,
    facing: String,
    declared_by: String,
    is_way: bool,
}

/// Columns of the structure that hold any block.
///
/// Not a walkability claim — "there is matter here" — and used only to draw the
/// silhouette of a piece whose producer supplied no floor.
fn occupied_columns(st: &Structure) -> BTreeSet<(i32, i32)> {
    let air: BTreeSet<usize> = st
        .palette
        .iter()
        .enumerate()
        .filter(|(_, s)| s.as_str() == "minecraft:air" || s.starts_with("minecraft:air["))
        .map(|(i, _)| i)
        .collect();
    st.blocks
        .iter()
        .filter(|(_, idx)| !air.contains(idx))
        .map(|(p, _)| (p[0], p[2]))
        .collect()
}

fn fill_rect(img: &mut image::RgbaImage, x: u32, y: u32, w: u32, h: u32, c: [u8; 4]) {
    let (iw, ih) = (img.width(), img.height());
    for py in y..(y + h).min(ih) {
        for px in x..(x + w).min(iw) {
            img.put_pixel(px, py, image::Rgba(c));
        }
    }
}

/// Draw the plan key for one piece.
///
/// `px` is the target side of the plan panel; the cell size is the largest
/// integer that fits the piece into it, never below 1, so a 125-long zone and a
/// 9-long room are both legible and neither is drawn at a fractional scale.
pub fn draw(
    id: &str,
    st: &Structure,
    meta: Option<&PrefabMeta>,
    px: u32,
) -> (image::RgbaImage, KeyBinding) {
    let (sx, sz) = (st.size[0].max(1) as u32, st.size[2].max(1) as u32);
    let cell = (px / sx.max(sz)).clamp(1, 16);
    let plan_w = sx * cell;
    let plan_h = sz * cell;

    let floor: Option<&Floor> = meta.and_then(|m| m.floor.as_ref());
    let span = floor.and_then(Floor::level_span);
    let occupied = occupied_columns(st);

    // Resolve the anchors onto plan cells. A region anchor resolves to its
    // centre; an anchor with neither a position nor a region cannot be drawn,
    // and is counted as such rather than dropped.
    let mut marks: Vec<Marked> = Vec::new();
    let mut anchors_total = 0usize;
    if let Some(m) = meta {
        let ways: BTreeSet<&String> = m.declared_entries.iter().chain(&m.declared_exits).collect();
        for (name, a) in &m.anchors {
            anchors_total += 1;
            let pos = a.pos.or_else(|| {
                a.region.as_ref().map(|r| {
                    [
                        (r.from[0] + r.to[0]) / 2,
                        (r.from[1] + r.to[1]) / 2,
                        (r.from[2] + r.to[2]) / 2,
                    ]
                })
            });
            let Some(p) = pos else { continue };
            marks.push(Marked {
                n: marks.len() + 1,
                x: p[0],
                y: p[1],
                z: p[2],
                name: name.clone(),
                facing: a.facing.clone().unwrap_or_else(|| "-".to_string()),
                declared_by: a.declared_by.clone().unwrap_or_else(|| "-".to_string()),
                is_way: ways.contains(name),
            });
        }
    }

    let opening_cells: BTreeSet<(i32, i32)> = meta
        .and_then(|m| m.openings.as_ref())
        .map(|o| o.by_face.values().flatten().map(|c| (c[0], c[2])).collect())
        .unwrap_or_default();
    let openings_drawn = meta
        .and_then(|m| m.openings.as_ref())
        .map(|o| o.total())
        .unwrap_or(0);

    let standable_columns = floor
        .map(|f| f.columns.iter().filter(|c| c.is_some()).count())
        .unwrap_or(0);

    let binding = KeyBinding {
        anchors_total,
        anchors_drawn: marks.len(),
        openings_drawn,
        standable_columns,
        occupied_columns: occupied.len(),
        floor_supplied: floor.is_some(),
        ways_declared: meta
            .map(|m| m.declared_entries.len() + m.declared_exits.len())
            .unwrap_or(0),
    };

    // ---- header + legend text -------------------------------------------
    let mut header: Vec<(String, [u8; 4])> = vec![
        (
            format!(
                "{id}  plan key  {}x{}x{}",
                st.size[0], st.size[1], st.size[2]
            ),
            FG,
        ),
        (
            format!(
                "floor {} of {} column(s){}{}   openings {}   anchors {}/{}",
                standable_columns,
                sx * sz,
                match span {
                    Some((lo, hi)) if hi > lo => format!(", y {lo}..{hi}"),
                    Some((lo, _)) => format!(", y {lo}"),
                    None => String::new(),
                },
                match floor.map(|f| f.multi_level_columns).unwrap_or(0) {
                    0 => String::new(),
                    n => format!(" (+{n} col. with an upper level, not drawn)"),
                },
                openings_drawn,
                binding.anchors_drawn,
                anchors_total,
            ),
            if binding.binds_to_nothing() {
                WARN
            } else {
                DIM
            },
        ),
    ];
    if !binding.floor_supplied {
        header.push(("NO FLOOR SUPPLIED - footprint only".to_string(), WARN));
    }
    if binding.ways_declared == 0 {
        header.push((
            "entry/exit NOT DECLARED - blue is every boundary cell".to_string(),
            WARN,
        ));
    }
    if binding.binds_to_nothing() {
        header.push(("0 ANCHORS DRAWN - nothing is named here".to_string(), WARN));
    }

    let legend: Vec<String> = marks
        .iter()
        .map(|m| {
            format!(
                "{:>2}. {}  at {},{},{}  facing {}  by {}",
                m.n, m.name, m.x, m.y, m.z, m.facing, m.declared_by
            )
        })
        .collect();

    let line_h = font::text_height(SCALE) + PAD / 2;
    let leg_h = font::text_height(1) + 3;
    let header_h = header.len() as u32 * line_h + PAD;
    let legend_w = legend
        .iter()
        .map(|t| font::text_width(t, 1))
        .max()
        .unwrap_or(0);
    let legend_rows_h = legend.len() as u32 * leg_h;
    // The legend goes beside the plan when the plan is tall enough to hold it,
    // and under it otherwise: a long thin zone leaves a column of empty page,
    // and a wide flat one has nowhere to put a column.
    let beside = !legend.is_empty() && legend_rows_h <= plan_h;
    // The page is at least as wide as its own widest claim, measured at the
    // scale that claim is drawn at. A truncated binding count is a binding count
    // nobody read, which is the failure mode this whole key exists to remove.
    let header_w = header
        .iter()
        .map(|(t, _)| font::text_width(t, SCALE))
        .max()
        .unwrap_or(0);
    let body_w = if beside {
        plan_w + PAD * 3 + legend_w
    } else {
        (plan_w + PAD * 2).max(legend_w + PAD * 2)
    };
    let width = body_w.max(header_w + PAD * 2).max(320);
    let height = header_h + plan_h + PAD * 2 + leg_h + if beside { 0 } else { legend_rows_h } + PAD;

    let mut img = image::RgbaImage::from_pixel(width, height, image::Rgba(BG));
    let mut y = PAD;
    for (text, color) in &header {
        let fitted = font::fit(text, width - PAD * 2, SCALE);
        font::draw_text(&mut img, PAD, y, &fitted, SCALE, *color);
        y += line_h;
    }

    // ---- the plan --------------------------------------------------------
    let px0 = PAD;
    let py0 = y;
    fill_rect(&mut img, px0, py0, plan_w, plan_h, PANEL);
    for gx in 0..sx {
        for gz in 0..sz {
            let tone = match floor.and_then(|f| f.at(gx, gz)) {
                Some(c) => floor_tone(c.y, span.unwrap_or((c.y, c.y))),
                None if occupied.contains(&(gx as i32, gz as i32)) => MASS,
                None => VOID,
            };
            fill_rect(&mut img, px0 + gx * cell, py0 + gz * cell, cell, cell, tone);
        }
    }
    // Boundary openings sit on top of the floor: the cells a body crosses at.
    for (ox, oz) in &opening_cells {
        if *ox < 0 || *oz < 0 || *ox as u32 >= sx || *oz as u32 >= sz {
            continue;
        }
        fill_rect(
            &mut img,
            px0 + *ox as u32 * cell,
            py0 + *oz as u32 * cell,
            cell,
            cell,
            OPENING,
        );
    }
    // Anchors last, so nothing covers them.
    for m in &marks {
        if m.x < 0 || m.z < 0 || m.x as u32 >= sx || m.z as u32 >= sz {
            continue;
        }
        let ax = px0 + m.x as u32 * cell;
        let az = py0 + m.z as u32 * cell;
        let color = if m.is_way { WAY } else { ANCHOR };
        fill_rect(&mut img, ax, az, cell, cell, color);
        // The facing tick: one cell-thick stub on the side the anchor looks
        // toward, so a body's heading is on the plan and not only in the list.
        let (tx, tz) = match m.facing.as_str() {
            "north" => (ax, az.saturating_sub(cell)),
            "south" => (ax, az + cell),
            "west" => (ax.saturating_sub(cell), az),
            "east" => (ax + cell, az),
            _ => (ax, az),
        };
        if (tx, tz) != (ax, az) {
            fill_rect(
                &mut img,
                tx,
                tz,
                cell.max(2) / 2 + 1,
                cell.max(2) / 2 + 1,
                color,
            );
        }
        font::draw_text(
            &mut img,
            ax + 1,
            az + 1,
            &m.n.to_string(),
            1,
            [16, 16, 20, 255],
        );
    }

    // Axis note, under the plan.
    font::draw_text(
        &mut img,
        PAD,
        py0 + plan_h + PAD,
        "plan: local x right, local z down; brighter floor is higher ground",
        1,
        DIM,
    );
    let (lx, mut ly, room) = if beside {
        (px0 + plan_w + PAD, py0, legend_w)
    } else {
        (PAD, py0 + plan_h + PAD + leg_h, width - PAD * 2)
    };
    for (i, line) in legend.iter().enumerate() {
        let color = if marks[i].is_way { WAY } else { ANCHOR };
        font::draw_text(&mut img, lx, ly, &font::fit(line, room, 1), 1, color);
        ly += leg_h;
    }

    (img, binding)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn structure() -> Structure {
        // A 5x3x7 shell: a stone floor with air over it.
        let mut blocks = Vec::new();
        for x in 0..5 {
            for z in 0..7 {
                blocks.push(([x, 0, z], 1usize));
                blocks.push(([x, 1, z], 0usize));
                blocks.push(([x, 2, z], 0usize));
            }
        }
        Structure {
            size: [5, 3, 7],
            palette: vec!["minecraft:air".into(), "minecraft:stone".into()],
            blocks,
        }
    }

    fn meta(json: &str) -> PrefabMeta {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn a_piece_with_no_metadata_says_it_annotated_nothing() {
        let (img, binding) = draw("bare", &structure(), None, 256);
        assert_eq!(binding.anchors_total, 0);
        assert!(binding.binds_to_nothing());
        assert!(!binding.floor_supplied);
        // The silhouette still reaches the page — 35 columns hold stone.
        assert_eq!(binding.occupied_columns, 35);
        assert!(img.width() >= 320 && img.height() > 0);
        let d = binding
            .diagnose("bare")
            .expect("a zero binding is a finding");
        assert_eq!(d.code, "DW0729");
        assert!(d.message.contains("0 of 0 anchor(s)"), "{}", d.message);
    }

    #[test]
    fn anchors_floor_and_openings_all_bind() {
        let m = meta(
            r#"{
              "anchors": {
                "anchor/gate":  { "pos": [2,1,0], "facing": "north", "declared_by": "gate/span" },
                "entry":        { "pos": [2,1,6], "facing": "south", "declared_by": "plan/mouth" }
              },
              "declared_entries": ["entry"],
              "floor": {
                "size": [5,3,7],
                "columns": [
                  {"y":1,"levels":1},{"y":1,"levels":1},{"y":1,"levels":1},{"y":1,"levels":1},
                  {"y":1,"levels":1},{"y":1,"levels":1},{"y":1,"levels":1},
                  {"y":1,"levels":2},null,null,null,null,null,null,
                  null,null,null,null,null,null,null,
                  null,null,null,null,null,null,null,
                  null,null,null,null,null,null,null
                ],
                "standable_cells": 9,
                "multi_level_columns": 1
              },
              "openings": { "by_face": { "z-min": [[2,1,0]], "z-max": [[2,1,6]] } }
            }"#,
        );
        let (_img, binding) = draw("annotated", &structure(), Some(&m), 256);
        assert_eq!((binding.anchors_total, binding.anchors_drawn), (2, 2));
        assert!(!binding.binds_to_nothing());
        assert!(binding.diagnose("annotated").is_none());
        assert_eq!(binding.openings_drawn, 2);
        assert_eq!(binding.standable_columns, 8);
        assert!(binding.floor_supplied);
        assert_eq!(binding.ways_declared, 1);
        assert!(binding.to_string().contains("2/2 anchor(s)"), "{binding}");
    }

    #[test]
    fn an_anchor_that_resolves_to_no_cell_is_counted_not_dropped() {
        let m = meta(r#"{ "anchors": { "anchor/nowhere": { "facing": "north" } } }"#);
        let (_img, binding) = draw("partial", &structure(), Some(&m), 256);
        assert_eq!((binding.anchors_total, binding.anchors_drawn), (1, 0));
        assert!(
            binding.binds_to_nothing(),
            "an anchor the key could not place must not read as one it drew"
        );
    }

    #[test]
    fn the_same_inputs_draw_the_same_bytes() {
        let m = meta(
            r#"{ "anchors": { "a": { "pos": [1,1,1], "facing": "east" } },
                 "openings": { "by_face": { "x-min": [[0,1,1]] } } }"#,
        );
        let a = draw("det", &structure(), Some(&m), 256).0;
        let b = draw("det", &structure(), Some(&m), 256).0;
        assert_eq!(a.into_raw(), b.into_raw());
    }
}
