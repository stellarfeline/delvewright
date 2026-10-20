//! A click box wins the pick (`DW1013`): the one rule every compiler-placed
//! `minecraft:interaction` box is built by, and the check that reads the shipped
//! bytes to prove it.
//!
//! ## The tie-break
//!
//! The client's crosshair takes an entity only when the entity's box is
//! **strictly** nearer the eye than the block the same ray hits; at equal
//! distance the block wins. A box whose face lies on the face of a block it
//! stands in — a `1.0`-wide box over a full-cube prop, a `2.0`-tall box whose top
//! is the top of an authored block in its head cell — is hit at exactly the
//! block's distance along every ray that reaches that face, so the click goes to
//! the block and the box never hears it. [cited: the engine's own statement of
//! the tie-break, first written for the seal answers; the pinned client's pick is
//! `GameRenderer.pick`, which compares the entity hit against the block hit and
//! keeps the entity only when nearer.]
//!
//! ## The rule ([`ClickBox`])
//!
//! A box placed at a cell is the box the emitter asks for — one block across and
//! `n` tall for every press body, the declared size for an assembly's hitbox —
//! with **each face that lies within [`CLICK_MARGIN`] of a block-grid plane
//! pushed outward to [`CLICK_MARGIN`] past that plane**. A face already further
//! from the grid is left where it was asked for. So every whole cell the box
//! covers is covered with a margin on every face, and the box is entered strictly
//! before any block outline inside those cells from every eye outside the box —
//! and an eye inside the box picks the box at distance zero. The margin is one
//! hundredth of a block: enough to be strictly nearer at every angle (the ray
//! meets the box face at least the margin before the block face behind it), and
//! never enough to reach into a neighbouring cell's own affordances.
//!
//! The rule is stated once, here. Every emitter that summons a click box at a
//! compile-time cell builds it through [`ClickBox`]; none writes a width, a
//! height or a floor offset of its own.
//!
//! ## The check ([`check`])
//!
//! Read off the finished tree, like `DW0545`: every `summon minecraft:interaction`
//! in every shipped function is a subject, whichever emitter wrote it. Three
//! kinds, each determined by the summon itself:
//!
//! * **placed** — absolute coordinates and the fixture class: a place the party
//!   clicks. Judged: every face of the box, as the server reads it (position as a
//!   double, width and height as the `f` floats they are written as), lies at
//!   least [`MIN_PROTRUSION`] off the block grid. A face on the grid is the tie,
//!   and the build is refused under `DW1013`.
//! * **borne** — the borne class: an NPC's dialogue hitbox, which stands in the
//!   air the NPC's own body stands in and travels with it (`tp`), so no grid
//!   holds still under it. Counted, not judged.
//! * **runtime-positioned** — relative coordinates (`~`): a recovery stake's
//!   marker, placed where a player died. No compile-time cell exists to judge.
//!   Counted, not judged.
//!
//! The lemma that makes the face test the whole proof: if a box covers a cell
//! and none of its faces is within [`MIN_PROTRUSION`] of a grid plane, the cell's
//! unit cube lies strictly inside the box, and every block outline lies inside
//! its unit cube, so every ray from an eye outside the box that reaches the
//! block's outline has already crossed the box's surface — at least
//! [`MIN_PROTRUSION`] earlier along the ray. That is every click ray from every
//! cell, not a sample of them. Reach is a separate fact: the entity ray is
//! traced only 3.0 blocks against the block ray's 4.5, so from between those
//! distances the block is picked whatever the box does; the cells a box "can be
//! clicked from" are the cells within the entity reach.

use std::collections::BTreeMap;

use delvewright_dsl::{DwCode, ExitTier};
use serde_json::{Value, json};

use crate::compiler::affordance::{BORNE_TAG, FIXTURE_TAG, selector_terms, shipped_functions};
use crate::compiler::commands::tokenize;
use crate::compiler::emit::BuildOutput;
use crate::compiler::failure::Failure;

delvewright_dsl::dw_code! {
    /// `DW1013`: a click box placed at a cell has a face on the block grid, so a
    /// block it stands in ties the crosshair's pick and wins it. An **engine
    /// self-check**: the emitter that summoned the box owes building it through
    /// `compiler::clickbox::ClickBox`.
    pub const DW_CLICK_BOX_FLUSH: DwCode = DwCode::new("DW1013", ExitTier::Build);
}

/// How far a click box protrudes past each grid plane its asked-for face lies
/// near, in blocks. Written into the datapack in hundredths ([`CLICK_MARGIN_CENTI`]).
pub const CLICK_MARGIN: f64 = 0.01;

/// [`CLICK_MARGIN`] in hundredths of a block — the unit the rule computes in, so
/// the emitted text is exactly what it reads as (ADR-0006).
pub const CLICK_MARGIN_CENTI: i64 = 1;

/// The least distance, in blocks, a placed box's face may lie from a grid plane
/// for [`check`] to call the box strictly nearer than the block behind it. A
/// tenth of the rule's margin [authored: far above the float rounding of a
/// width written as an `f` float (under 1e-6 for any box the engine places) and
/// of the client's double-precision ray clip, so a box this check passes is
/// strictly nearer in the client's arithmetic too].
pub const MIN_PROTRUSION: f64 = 0.001;

/// Exact decimal in units of 10⁻¹⁰ block — wide enough that a declared size's
/// nine decimals and its half are both exact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Dec(i128);

const UNIT: i128 = 10_000_000_000;

impl Dec {
    fn int(v: i64) -> Self {
        Dec(v as i128 * UNIT)
    }

    fn centi(v: i64) -> Self {
        Dec(v as i128 * (UNIT / 100))
    }

    /// A declared `f64` read as the decimal it was written as (its shortest
    /// round-trip text), to nine decimals.
    fn of_f64(v: f64) -> Self {
        let text = format!("{v}");
        let (neg, digits) = match text.strip_prefix('-') {
            Some(d) => (true, d),
            None => (false, text.as_str()),
        };
        let (whole, frac) = digits.split_once('.').unwrap_or((digits, ""));
        let whole: i128 = whole.parse().unwrap_or(0);
        let mut frac_units: i128 = 0;
        let mut place = UNIT / 10;
        for (i, c) in frac.chars().enumerate() {
            let d = c.to_digit(10).unwrap_or(0) as i128;
            if i < 9 {
                frac_units += d * place;
                place /= 10;
            } else {
                // Round half away from zero at the tenth decimal.
                if d >= 5 {
                    frac_units += UNIT / 1_000_000_000;
                }
                break;
            }
        }
        let m = whole * UNIT + frac_units;
        Dec(if neg { -m } else { m })
    }

    fn half(self) -> Self {
        Dec(self.0 / 2)
    }

    /// The nearest integer, as a `Dec`.
    fn nearest_int(self) -> Self {
        let q = self.0.div_euclid(UNIT);
        let r = self.0.rem_euclid(UNIT);
        Dec(if r * 2 >= UNIT {
            (q + 1) * UNIT
        } else {
            q * UNIT
        })
    }

    /// The decimal text: `68.99`, `-44.5`, `7.0`, `1.02`.
    fn text(self) -> String {
        let sign = if self.0 < 0 { "-" } else { "" };
        let a = self.0.unsigned_abs();
        let u = UNIT as u128;
        let (whole, frac) = (a / u, a % u);
        if frac == 0 {
            return format!("{sign}{whole}.0");
        }
        let digits = format!("{frac:010}");
        format!("{sign}{whole}.{}", digits.trim_end_matches('0'))
    }
}

impl std::ops::Add for Dec {
    type Output = Dec;
    fn add(self, o: Dec) -> Dec {
        Dec(self.0 + o.0)
    }
}

impl std::ops::Sub for Dec {
    type Output = Dec;
    fn sub(self, o: Dec) -> Dec {
        Dec(self.0 - o.0)
    }
}

/// A face pushed outward past the grid plane it lies near: `lo` faces move down
/// the axis, `hi` faces up.
fn snap(face: Dec, outward: i128) -> Dec {
    let margin = Dec::centi(CLICK_MARGIN_CENTI);
    let g = face.nearest_int();
    if (face - g).0.abs() <= margin.0 {
        Dec(g.0 + outward * margin.0)
    } else {
        face
    }
}

/// A click box at a cell, built by the one rule (see the module docs).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClickBox {
    /// The entity position: the box's horizontal centre and its floor.
    pos: [Dec; 3],
    width: Dec,
    height: Dec,
}

impl ClickBox {
    /// The press-body column: one block across, `cells` tall, standing in `cell`.
    pub fn column(cell: [i32; 3], cells: u32) -> Self {
        Self::fitted(cell, Dec::int(1), Dec::int(i64::from(cells)))
    }

    /// A declared size, `width` across and `height` tall, its bottom centre on
    /// `cell`'s floor centre (an assembly's hitbox).
    pub fn sized(cell: [i32; 3], width: f64, height: f64) -> Self {
        Self::fitted(cell, Dec::of_f64(width), Dec::of_f64(height))
    }

    fn fitted(cell: [i32; 3], width: Dec, height: Dec) -> Self {
        let half_cell = Dec::centi(50);
        let horizontal = |c: i32| {
            let centre = Dec::int(i64::from(c)) + half_cell;
            let lo = snap(centre - width.half(), -1);
            let hi = snap(centre + width.half(), 1);
            // The centre sits on a half-block, so a face and its mirror lie the
            // same distance from the grid and snap together: the box stays
            // centred on the cell.
            (centre, hi - lo)
        };
        let (x, w) = horizontal(cell[0]);
        let (z, _) = horizontal(cell[2]);
        let floor = snap(Dec::int(i64::from(cell[1])), -1);
        let top = snap(Dec::int(i64::from(cell[1])) + height, 1);
        ClickBox {
            pos: [x, floor, z],
            width: w,
            height: top - floor,
        }
    }

    /// The position as the three coordinate tokens a `summon` carries.
    pub fn xyz(&self) -> String {
        format!(
            "{} {} {}",
            self.pos[0].text(),
            self.pos[1].text(),
            self.pos[2].text()
        )
    }

    /// The box's size as the leading NBT fields: `width:1.02f,height:2.02f`.
    pub fn size_nbt(&self) -> String {
        format!(
            "width:{}f,height:{}f",
            self.width.text(),
            self.height.text()
        )
    }

    /// The whole `summon` command; `rest` is the NBT after the size
    /// (`response:1b,Invulnerable:1b,Tags:[…]`).
    pub fn summon(&self, rest: &str) -> String {
        format!(
            "summon minecraft:interaction {} {{{},{rest}}}",
            self.xyz(),
            self.size_nbt()
        )
    }
}

/// What [`check`] examined, stated whichever way it went.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClickBoxBinding {
    /// Every `summon minecraft:interaction` in the shipped functions.
    pub summons: usize,
    /// Of them, placed at a cell and judged, by the family of their first
    /// non-class tag (`dw_i`, `dw_trig`, `dw_seal`, …).
    pub placed: BTreeMap<String, usize>,
    /// Of them, borne by a body (an NPC's dialogue hitbox): not judged.
    pub borne: usize,
    /// Of them, at a runtime position (`~`): not judged.
    pub runtime: usize,
}

impl ClickBoxBinding {
    /// How many placed boxes were judged.
    pub fn judged(&self) -> usize {
        self.placed.values().sum()
    }

    /// The one line a build prints about this rule.
    pub fn line(&self) -> String {
        let families = if self.placed.is_empty() {
            "none".to_string()
        } else {
            self.placed
                .iter()
                .map(|(k, v)| format!("{k} {v}"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        format!(
            "DW1013 binding: {} of {} interaction summon(s) placed at a cell and judged ({families}); \
             {} borne by an NPC body, {} at a runtime position — every judged box's faces \
             at least {MIN_PROTRUSION} block off the block grid",
            self.judged(),
            self.summons,
            self.borne,
            self.runtime
        )
    }

    /// The `validation/click-box.json` ledger.
    pub fn to_json(&self) -> Value {
        json!({
            "code": DW_CLICK_BOX_FLUSH,
            "summons": self.summons,
            "judged": self.judged(),
            "placed": self.placed,
            "borne": self.borne,
            "runtime": self.runtime,
            "margin": CLICK_MARGIN,
            "min_protrusion": MIN_PROTRUSION,
        })
    }
}

/// One `summon minecraft:interaction` as the server reads it.
struct Summoned {
    coords: [String; 3],
    nbt: String,
}

/// The interaction summon on this line, if any — after any `execute … run`.
fn summoned(line: &str) -> Option<Summoned> {
    let (tokens, _) = tokenize(line.trim()).ok()?;
    let i = tokens
        .windows(2)
        .position(|w| w[0] == "summon" && w[1] == "minecraft:interaction")?;
    let coords = [
        tokens.get(i + 2)?.clone(),
        tokens.get(i + 3)?.clone(),
        tokens.get(i + 4)?.clone(),
    ];
    let nbt = tokens.get(i + 5).cloned().unwrap_or_default();
    Some(Summoned { coords, nbt })
}

/// A top-level numeric NBT field (`width:1.02f`) as the float the server stores,
/// widened to the double its box is built in. Vanilla's default is `1.0`.
fn float_field(nbt: &str, key: &str) -> f64 {
    let inner = nbt
        .strip_prefix('{')
        .and_then(|s| s.strip_suffix('}'))
        .unwrap_or("");
    selector_terms(inner)
        .filter_map(|t| t.split_once(':'))
        .find(|(k, _)| k.trim() == key)
        .and_then(|(_, v)| {
            v.trim()
                .trim_end_matches(['f', 'F', 'd', 'D'])
                .parse::<f32>()
                .ok()
        })
        .map_or(1.0, f64::from)
}

/// The tags in a summon's `Tags:[…]`.
fn tags(nbt: &str) -> Vec<String> {
    let Some(start) = nbt.find("Tags:[") else {
        return Vec::new();
    };
    let rest = &nbt[start + "Tags:[".len()..];
    let Some(end) = rest.find(']') else {
        return Vec::new();
    };
    rest[..end]
        .split(',')
        .map(|t| t.trim().trim_matches('"').to_string())
        .filter(|t| !t.is_empty())
        .collect()
}

/// `dw_trig_wake` → `dw_trig`; `dw_asm_crane_hit` → `dw_asm`.
fn family(tag: &str) -> String {
    let mut parts = tag.splitn(3, '_');
    match (parts.next(), parts.next()) {
        (Some(a), Some(b)) => format!("{a}_{b}"),
        _ => tag.to_string(),
    }
}

/// The face of a placed box that lies on the grid, if one does: `(axis, which,
/// value)`.
fn flush_face(s: &Summoned) -> Option<(&'static str, &'static str, f64)> {
    let x: f64 = s.coords[0].parse().ok()?;
    let y: f64 = s.coords[1].parse().ok()?;
    let z: f64 = s.coords[2].parse().ok()?;
    let w = float_field(&s.nbt, "width");
    let h = float_field(&s.nbt, "height");
    let faces = [
        ("x", "west", x - w / 2.0),
        ("x", "east", x + w / 2.0),
        ("y", "floor", y),
        ("y", "top", y + h),
        ("z", "north", z - w / 2.0),
        ("z", "south", z + w / 2.0),
    ];
    faces
        .into_iter()
        .find(|(_, _, v)| (v - v.round()).abs() < MIN_PROTRUSION)
}

/// Prove every click box placed at a cell is strictly nearer than any block it
/// stands in, from every eye outside it (`DW1013`). Reads the shipped functions.
pub fn check(out: &BuildOutput) -> Result<ClickBoxBinding, Failure> {
    let mut b = ClickBoxBinding::default();
    for (name, body) in shipped_functions(out) {
        for line in body.lines() {
            if !line.contains("summon minecraft:interaction") {
                continue;
            }
            let Some(s) = summoned(line) else {
                return Err(Failure::new(
                    DW_CLICK_BOX_FLUSH,
                    format!(
                        "`{name}` summons a `minecraft:interaction` this check cannot read as \
                         `summon minecraft:interaction <x> <y> <z> {{…}}`. A click box the check \
                         cannot read is a box nobody has shown wins the crosshair's pick. \
                         Command: `{}`.",
                        line.trim()
                    ),
                ));
            };
            b.summons += 1;
            let t = tags(&s.nbt);
            if t.iter().any(|t| t == BORNE_TAG) {
                b.borne += 1;
                continue;
            }
            if s.coords
                .iter()
                .any(|c| c.starts_with('~') || c.starts_with('^'))
            {
                b.runtime += 1;
                continue;
            }
            let fam = t
                .iter()
                .find(|t| *t != FIXTURE_TAG)
                .map(|t| family(t))
                .unwrap_or_else(|| "untagged".to_string());
            if let Some((axis, which, v)) = flush_face(&s) {
                return Err(Failure::new(
                    DW_CLICK_BOX_FLUSH,
                    format!(
                        "`{name}` places a click box whose {which} face lies on the block grid \
                         ({axis} = {v}). The crosshair takes the box only when it is strictly \
                         nearer than the block the same ray hits; a block in the cell this box \
                         stands in shares that face, ties every ray that reaches it, and wins \
                         the click — the prop takes many presses, or none. The emitter must \
                         build the box through `compiler::clickbox::ClickBox`, which pushes \
                         each face {CLICK_MARGIN} past the grid. Command: `{}`.",
                        line.trim()
                    ),
                ));
            }
            *b.placed.entry(fam).or_insert(0) += 1;
        }
    }
    Ok(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(lines: &[&str]) -> BuildOutput {
        let mut out = BuildOutput::new();
        out.insert(
            "datapack/data/t/function/setup.mcfunction".to_string(),
            format!("{}\n", lines.join("\n")).into_bytes(),
        );
        out
    }

    #[test]
    fn the_column_brackets_its_cell_on_every_face() {
        let b = ClickBox::column([4, 65, -7], 2);
        assert_eq!(b.xyz(), "4.5 64.99 -6.5");
        assert_eq!(b.size_nbt(), "width:1.02f,height:2.02f");
        let seal = ClickBox::column([68, 0, -45], 1);
        assert_eq!(seal.xyz(), "68.5 -0.01 -44.5");
        assert_eq!(seal.size_nbt(), "width:1.02f,height:1.02f");
    }

    #[test]
    fn a_declared_size_moves_only_the_faces_on_the_grid() {
        // 1.5 wide: faces a quarter block off the grid, left where declared; the
        // floor sits on the grid and the 2.5 top does not.
        let b = ClickBox::sized([0, 64, 0], 1.5, 2.5);
        assert_eq!(b.xyz(), "0.5 63.99 0.5");
        assert_eq!(b.size_nbt(), "width:1.5f,height:2.51f");
        // 0.98 wide: faces 0.01 inside the cell, within the margin, pushed out.
        let n = ClickBox::sized([0, 64, 0], 0.98, 1.0);
        assert_eq!(n.size_nbt(), "width:1.02f,height:1.02f");
        // 3.0 wide: faces on the grid two cells out.
        let w = ClickBox::sized([-3, 64, 2], 3.0, 2.0);
        assert_eq!(w.xyz(), "-2.5 63.99 2.5");
        assert_eq!(w.size_nbt(), "width:3.02f,height:2.02f");
    }

    #[test]
    fn every_box_the_rule_builds_passes_the_check() {
        let mut lines = Vec::new();
        for cell in [[0, 64, 0], [-16, -1, 7], [1000, 300, -1000]] {
            lines.push(ClickBox::column(cell, 2).summon("Tags:[\"dw_fixture\",\"dw_i_a\"]"));
            lines.push(ClickBox::column(cell, 1).summon("Tags:[\"dw_fixture\",\"dw_seal_a\"]"));
            lines.push(
                ClickBox::sized(cell, 0.98, 2.5).summon("Tags:[\"dw_fixture\",\"dw_asm_a_hit\"]"),
            );
        }
        let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        let b = check(&tree(&refs)).expect("the rule's own boxes are strictly nearer");
        assert_eq!(b.judged(), 9);
        assert_eq!(b.summons, 9);
    }

    /// The perturbation: the shape every interact objective shipped with — a
    /// `1.0 × 2.0` box at the cell centre, flush with a full-cube prop.
    #[test]
    fn a_flush_box_is_refused() {
        let out = tree(&[
            "summon minecraft:interaction 4.5 65.0 6.5 {width:1.0f,height:2.0f,response:1b,Invulnerable:1b,Tags:[\"dw_fixture\",\"dw_i_o_open\"]}",
        ]);
        let e = check(&out).expect_err("a flush box ties the block");
        assert_eq!(e.code, DW_CLICK_BOX_FLUSH);
        assert!(e.message.contains("west face"), "{}", e.message);
    }

    /// Only the floor on the grid is still a tie with a block's bottom face.
    #[test]
    fn a_box_flush_on_one_face_is_refused() {
        let out = tree(&[
            "execute unless entity @e[tag=dw_shop_0] run summon minecraft:interaction 4.5 65.0 6.5 {width:1.02f,height:2.02f,Tags:[\"dw_fixture\",\"dw_shop_0\"]}",
        ]);
        let e = check(&out).expect_err("the floor is on the grid");
        assert!(e.message.contains("floor face"), "{}", e.message);
    }

    #[test]
    fn borne_and_runtime_boxes_are_counted_not_judged() {
        let out = tree(&[
            "summon minecraft:interaction 4.5 65.0 6.5 {width:1.0f,height:2.0f,Tags:[\"dw_borne\",\"dw_npc_keeper\"]}",
            "execute unless entity @e[tag=dw_stk,distance=..1] run summon minecraft:interaction ~ ~ ~ {width:1.0f,height:2.0f,Tags:[\"dw_fixture\",\"dw_stk\"]}",
        ]);
        let b = check(&out).expect("neither kind is judged");
        assert_eq!((b.summons, b.borne, b.runtime, b.judged()), (2, 1, 1, 0));
    }

    #[test]
    fn the_size_defaults_are_vanillas() {
        // No width/height: vanilla's 1.0 × 1.0, on the grid.
        let out =
            tree(&["summon minecraft:interaction 4.5 64.99 6.5 {Tags:[\"dw_fixture\",\"dw_x\"]}"]);
        let e = check(&out).expect_err("a default-sized box is flush");
        assert!(e.message.contains("west face"), "{}", e.message);
    }

    #[test]
    fn code_is_dw1013() {
        assert_eq!(DW_CLICK_BOX_FLUSH, "DW1013");
    }
}
