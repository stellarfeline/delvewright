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
//! ## The rule ([`ClickBox::fitted_to`])
//!
//! A click box stands over **one real block** — a door's two cells — and is
//! fitted to the block's own outline (spec-0117 §3a; the outline is the pinned
//! jar's `getShape`, read from [`delvewright_dsl::blockshape::outline`]). With
//! the outline's envelope `E` and `m` = [`CLICK_MARGIN`]: the box is
//! `max(x extent, z extent) + 2m` wide (an `interaction` is a square in plan,
//! one `width` for both horizontal axes) and `y extent + 2m` tall, its floor
//! `m` under `E`'s; on the longer horizontal axis it is centred on `E`, on the
//! other on `E` moved by the least amount that keeps it inside the body's cells
//! expanded by `m`. An assembly's declared hitbox is sized by
//! [`ClickBox::sized`]: each face within [`CLICK_MARGIN`] of a grid plane is
//! pushed out to [`CLICK_MARGIN`] past it.
//!
//! The rule is stated once, here. Every emitter that summons a click box at a
//! compile-time cell builds it through [`ClickBox`]; none writes a width, a
//! height or a floor offset of its own.
//!
//! ## The check ([`check`])
//!
//! Read off the finished tree: every `summon minecraft:interaction` in every
//! shipped function is a subject, whichever emitter wrote it. Four kinds, each
//! determined by the summon itself:
//!
//! * **placed over a block** — absolute coordinates and the fixture class: a
//!   pressable's click box. Judged against the block it stands over (read from
//!   the shipped `setup_finish`, or the gate's block for a gate's own answer),
//!   with the box as the server reads it (position as a double, width and
//!   height as the `f` floats they are written as): every box of the outline
//!   lies inside the click box by at least [`MIN_PROTRUSION`] on every face, and
//!   the click box lies inside the body's cells expanded by [`CLICK_MARGIN`].
//!   A box over air, or over a block with no outline, is refused too.
//! * **an assembly hitbox** (`dw_asm`) — judged against the block grid: every
//!   face at least [`MIN_PROTRUSION`] off it.
//! * **borne** — an NPC's dialogue hitbox or an actor's answer box, which stands
//!   in the air its body stands in and travels with it. Counted, not judged.
//! * **runtime-positioned** — relative coordinates (`~`): a recovery stake's
//!   marker. Counted, not judged.
//!
//! The lemma that makes the face test the whole proof: if every box of the
//! block's outline lies strictly inside the click box, every ray from an eye
//! outside the click box that reaches the block's outline has crossed the click
//! box's surface first — at least [`MIN_PROTRUSION`] earlier along the ray — so
//! the client's pick (the entity when strictly nearer) takes the box. That is
//! every click ray from every cell, not a sample of them. Reach is a separate
//! fact: the entity ray is traced only 3.0 blocks against the block ray's 4.5,
//! so from between those distances the block is picked whatever the box does.

use std::collections::BTreeMap;

use delvewright_dsl::{DwCode, ExitTier};
use serde_json::{Value, json};

use crate::compiler::affordance::{BORNE_TAG, FIXTURE_TAG, selector_terms, shipped_functions};
use crate::compiler::commands::tokenize;
use crate::compiler::emit::BuildOutput;
use crate::compiler::failure::Failure;

delvewright_dsl::dw_code! {
    /// `DW1013`: a click box placed at a cell does not enclose the outline of
    /// the block it stands over by [`MIN_PROTRUSION`] on every face, reaches
    /// past that block's cells, or stands over no block at all — so the block
    /// ties the crosshair's pick and wins it, or the box takes a neighbour's
    /// press. An **engine self-check**: the emitter that summoned the box owes
    /// building it through `compiler::clickbox::ClickBox::fitted_to`.
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

    /// A bound in sixteenths of a block, exact where it can be and rounded
    /// outward (down for a minimum, up for a maximum) where it cannot.
    fn sixteenths(s: delvewright_dsl::blockshape::Sixteenths, up: bool) -> Self {
        let num = i128::from(s.num) * UNIT;
        let den = 16 * i128::from(s.den);
        let q = num.div_euclid(den);
        let r = num.rem_euclid(den);
        Dec(if up && r != 0 { q + 1 } else { q })
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
    /// **The fitted rule** (spec-0117 §3a): the click box over `block` standing
    /// at `cell` — a door's lower half spans its two cells — fitted to the
    /// block's own outline ([`delvewright_dsl::blockshape::outline`]).
    ///
    /// With the outline's envelope `E` over the body's cells and `m` =
    /// [`CLICK_MARGIN`]: the box is `max(x extent, z extent) + 2m` wide and
    /// `y extent + 2m` tall, its floor `m` under `E`'s; on the longer horizontal
    /// axis it is centred on `E`; on the other it is centred on `E` moved by the
    /// least amount that keeps it inside the body's cells expanded by `m`. So
    /// every box of the outline lies inside the click box by at least `m` on
    /// every face, and the click box reaches no neighbouring cell. `None` when
    /// the block has no outline or one that leaves its cells (`DW1059`).
    pub fn fitted_to(cell: [i32; 3], block: &str) -> Option<Self> {
        let boxes = delvewright_dsl::blockshape::server_outline(block);
        if boxes.is_empty() {
            return None;
        }
        let cells: i64 = if crate::compiler::pressable::is_door(block) {
            2
        } else {
            1
        };
        let mut lo = [Dec(i128::MAX); 3];
        let mut hi = [Dec(i128::MIN); 3];
        for k in 0..cells {
            for b in &boxes {
                for a in 0..3 {
                    let base = Dec::int(i64::from(cell[a]) + if a == 1 { k } else { 0 });
                    lo[a] = lo[a].min(base + Dec::sixteenths(b.min[a], false));
                    hi[a] = hi[a].max(base + Dec::sixteenths(b.max[a], true));
                }
            }
        }
        let cell_lo = [
            Dec::int(i64::from(cell[0])),
            Dec::int(i64::from(cell[1])),
            Dec::int(i64::from(cell[2])),
        ];
        let cell_hi = [
            cell_lo[0] + Dec::int(1),
            cell_lo[1] + Dec::int(cells),
            cell_lo[2] + Dec::int(1),
        ];
        if (0..3).any(|a| lo[a] < cell_lo[a] || hi[a] > cell_hi[a]) {
            return None;
        }
        let m = Dec::centi(CLICK_MARGIN_CENTI);
        let ex = hi[0] - lo[0];
        let ez = hi[2] - lo[2];
        let width = ex.max(ez) + m + m;
        let height = (hi[1] - lo[1]) + m + m;
        let floor = lo[1] - m;
        let centre = |a: usize, longer: bool| {
            let c = Dec((lo[a].0 + hi[a].0) / 2);
            if longer {
                return c;
            }
            let half = width.half();
            let min_c = cell_lo[a] - m + half;
            let max_c = cell_hi[a] + m - half;
            c.max(min_c).min(max_c)
        };
        Some(ClickBox {
            pos: [centre(0, ex >= ez), floor, centre(2, ez >= ex)],
            width,
            height,
        })
    }

    /// A declared size, `width` across and `height` tall, its bottom centre on
    /// `cell`'s floor centre (an assembly's hitbox).
    pub fn sized(cell: [i32; 3], width: f64, height: f64) -> Self {
        Self::snapped(cell, Dec::of_f64(width), Dec::of_f64(height))
    }

    fn snapped(cell: [i32; 3], width: Dec, height: Dec) -> Self {
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

    /// The width as the decimal the summon writes (`0.395`).
    pub fn width_text(&self) -> String {
        self.width.text()
    }

    /// The height as the decimal the summon writes (`2.02`).
    pub fn height_text(&self) -> String {
        self.height.text()
    }

    /// The box as the server builds it: `(min, max)` corners, in blocks.
    pub fn aabb(&self) -> ([f64; 3], [f64; 3]) {
        let f = |d: Dec| d.0 as f64 / UNIT as f64;
        let (x, y, z) = (f(self.pos[0]), f(self.pos[1]), f(self.pos[2]));
        let (w, h) = (f(self.width), f(self.height));
        (
            [x - w / 2.0, y, z - w / 2.0],
            [x + w / 2.0, y + h, z + w / 2.0],
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

/// What carries a borne box, read off the box's own tags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Bearer {
    /// An NPC's dialogue hitbox (`dw_npc_<id>`).
    Npc,
    /// An actor's answer box (spec-0119, [`crate::compiler::bodies::ANSWER_BOX_TAG`]).
    ActorAnswer,
    /// A borne box wearing neither tag.
    Other,
}

impl Bearer {
    /// What bears a box wearing `tags`.
    fn of(tags: &[String]) -> Self {
        if tags
            .iter()
            .any(|t| t.starts_with(crate::compiler::bodies::ANSWER_BOX_TAG))
        {
            Bearer::ActorAnswer
        } else if tags.iter().any(|t| t.starts_with("dw_npc_")) {
            Bearer::Npc
        } else {
            Bearer::Other
        }
    }

    /// The words the binding line and the ledger use.
    pub fn name(self) -> &'static str {
        match self {
            Bearer::Npc => "NPC dialogue hitbox(es)",
            Bearer::ActorAnswer => "actor answer box(es)",
            Bearer::Other => "other borne box(es)",
        }
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
    /// Of them, borne by a body: not judged.
    pub borne: usize,
    /// The borne boxes by what bears them, read off each summon's own tags
    /// ([`Bearer`]); sums to [`Self::borne`].
    pub borne_by: BTreeMap<Bearer, usize>,
    /// Of them, at a runtime position (`~`): not judged.
    pub runtime: usize,
    /// Of the judged, fitted to a two-cell body (a door).
    pub two_cell: usize,
    /// Assembly hitboxes (`dw_asm`), judged against the block grid.
    pub assembly: usize,
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
        let bearers = [Bearer::Npc, Bearer::ActorAnswer]
            .into_iter()
            .chain(
                self.borne_by
                    .contains_key(&Bearer::Other)
                    .then_some(Bearer::Other),
            )
            .map(|k| {
                format!(
                    "{} {}",
                    self.borne_by.get(&k).copied().unwrap_or(0),
                    k.name()
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "DW1013 binding: {} of {} interaction summon(s) placed at a cell and judged against \
             its block ({families}); {} fitted to a two-cell body; {} assembly hitbox(es) judged \
             against the grid; {} borne by a body ({bearers}), {} at a runtime position — every \
             judged box encloses its block's outline by at least {MIN_PROTRUSION} and lies within \
             its cells by at most {CLICK_MARGIN}",
            self.judged(),
            self.summons,
            self.two_cell,
            self.assembly,
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
            "borne_by": self
                .borne_by
                .iter()
                .map(|(k, v)| (k.name().to_string(), json!(v)))
                .collect::<serde_json::Map<String, Value>>(),
            "runtime": self.runtime,
            "two_cell": self.two_cell,
            "assembly": self.assembly,
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

/// The `setblock` lines `setup_finish` ships, by cell: the one list of placed
/// props the written world also lays.
fn setup_blocks(out: &BuildOutput) -> BTreeMap<[i32; 3], String> {
    let mut m = BTreeMap::new();
    for (name, body) in shipped_functions(out) {
        if name != "setup_finish" {
            continue;
        }
        for line in body.lines() {
            let Ok((t, _)) = tokenize(line.trim()) else {
                continue;
            };
            if t.len() >= 5 && t[0] == "setblock" {
                let p: Option<Vec<i32>> = t[1..4].iter().map(|v| v.parse().ok()).collect();
                if let Some(p) = p {
                    m.insert([p[0], p[1], p[2]], t[4].clone());
                }
            }
        }
    }
    m
}

/// The first face on which the box `(lo, hi)` fails to enclose the outline box
/// `(olo, ohi)` by at least [`MIN_PROTRUSION`]: `(face, the box's bound, the
/// outline's bound)`.
fn uncovered_face(
    lo: [f64; 3],
    hi: [f64; 3],
    olo: [f64; 3],
    ohi: [f64; 3],
) -> Option<(&'static str, f64, f64)> {
    const LO: [&str; 3] = ["west", "floor", "north"];
    const HI: [&str; 3] = ["east", "top", "south"];
    for a in 0..3 {
        if lo[a] > olo[a] - MIN_PROTRUSION {
            return Some((LO[a], lo[a], olo[a]));
        }
        if hi[a] < ohi[a] + MIN_PROTRUSION {
            return Some((HI[a], hi[a], ohi[a]));
        }
    }
    None
}

/// Prove every click box placed at a cell encloses the outline of the block it
/// stands over and stays within that block's cells (`DW1013`). Reads the
/// shipped functions.
///
/// The block under a box is read from the shipped `setup_finish` (the one
/// `setblock` list the written world also lays; a door's two cells from its
/// lower half), or, for a press whose block the compiler does not place (a
/// gate's own answer, a door-shut seam's key), from `gate_blocks` — the
/// gate's block at the cell, which the prefab, the derivation or a
/// `close-gate` lays. A box over a
/// cell neither names, or over a block with no outline, has nothing under it
/// and is refused. An assembly's declared hitbox (`dw_asm`) is judged against
/// the block grid, because the assembly is an entity and not a block.
pub fn check(
    out: &BuildOutput,
    gate_blocks: &BTreeMap<[i32; 3], String>,
) -> Result<ClickBoxBinding, Failure> {
    let placed_blocks = setup_blocks(out);
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
                *b.borne_by.entry(Bearer::of(&t)).or_insert(0) += 1;
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
            if fam == "dw_asm" {
                if let Some((axis, which, v)) = flush_face(&s) {
                    return Err(Failure::new(
                        DW_CLICK_BOX_FLUSH,
                        format!(
                            "`{name}` places an assembly hitbox whose {which} face lies on the \
                             block grid ({axis} = {v}). The crosshair takes the box only when it \
                             is strictly nearer than the block the same ray hits; a block in the \
                             cell it stands in shares that face and wins the click. The emitter \
                             must build the box through `compiler::clickbox::ClickBox::sized`. \
                             Command: `{}`.",
                            line.trim()
                        ),
                    ));
                }
                b.assembly += 1;
                continue;
            }
            let num = |i: usize| s.coords[i].parse::<f64>().ok();
            let (Some(x), Some(y), Some(z)) = (num(0), num(1), num(2)) else {
                return Err(Failure::new(
                    DW_CLICK_BOX_FLUSH,
                    format!(
                        "`{name}` places a click box at coordinates this check cannot read. \
                         Command: `{}`.",
                        line.trim()
                    ),
                ));
            };
            let w = float_field(&s.nbt, "width");
            let h = float_field(&s.nbt, "height");
            let lo = [x - w / 2.0, y, z - w / 2.0];
            let hi = [x + w / 2.0, y + h, z + w / 2.0];
            let cell = [
                x.floor() as i32,
                (y + CLICK_MARGIN + 1e-6).floor() as i32,
                z.floor() as i32,
            ];
            let block = placed_blocks
                .get(&cell)
                .or_else(|| gate_blocks.get(&cell))
                .filter(|b| !delvewright_dsl::blockshape::is_air(b));
            let Some(block) = block else {
                return Err(Failure::new(
                    DW_CLICK_BOX_FLUSH,
                    format!(
                        "`{name}` places a click box over {cell:?}, and `setup_finish` places no \
                         block there and no gate's block stands there: the box stands in the \
                         air, an invisible thing a player cannot find. A click box stands over \
                         a real block only (spec-0117 §3a). Command: `{}`.",
                        line.trim()
                    ),
                ));
            };
            let outline = delvewright_dsl::blockshape::server_outline(block);
            if outline.is_empty() {
                return Err(Failure::new(
                    DW_CLICK_BOX_FLUSH,
                    format!(
                        "`{name}` places a click box over `{block}` at {cell:?}, a blockstate the \
                         pinned game gives no outline: there is nothing under the box to \
                         press. Command: `{}`.",
                        line.trim()
                    ),
                ));
            }
            let door = crate::compiler::pressable::is_door(block);
            let cells = if door { 2 } else { 1 };
            let six = |v: delvewright_dsl::blockshape::Sixteenths| {
                f64::from(v.num) / (16.0 * f64::from(v.den))
            };
            for k in 0..cells {
                for ob in &outline {
                    let olo = [
                        f64::from(cell[0]) + six(ob.min[0]),
                        f64::from(cell[1] + k) + six(ob.min[1]),
                        f64::from(cell[2]) + six(ob.min[2]),
                    ];
                    let ohi = [
                        f64::from(cell[0]) + six(ob.max[0]),
                        f64::from(cell[1] + k) + six(ob.max[1]),
                        f64::from(cell[2]) + six(ob.max[2]),
                    ];
                    if let Some((face, at, outline_at)) = uncovered_face(lo, hi, olo, ohi) {
                        let sixteenth = ((outline_at - outline_at.floor()) * 16.0).round();
                        return Err(Failure::new(
                            DW_CLICK_BOX_FLUSH,
                            format!(
                                "`{name}` places a click box over `{block}` at {cell:?} whose \
                                 {face} face ({at}) does not cover the block's outline there \
                                 ({outline_at}, sixteenth {sixteenth}) by {MIN_PROTRUSION}: a \
                                 ray reaching that face of the block meets the block first, and \
                                 the press goes to the block. The box must be built by \
                                 `compiler::clickbox::ClickBox::fitted_to`. Command: `{}`.",
                                line.trim()
                            ),
                        ));
                    }
                }
            }
            let cell_lo = [f64::from(cell[0]), f64::from(cell[1]), f64::from(cell[2])];
            let cell_hi = [
                cell_lo[0] + 1.0,
                cell_lo[1] + f64::from(cells),
                cell_lo[2] + 1.0,
            ];
            let tol = CLICK_MARGIN + 1e-6;
            if (0..3).any(|a| lo[a] < cell_lo[a] - tol || hi[a] > cell_hi[a] + tol) {
                return Err(Failure::new(
                    DW_CLICK_BOX_FLUSH,
                    format!(
                        "`{name}` places a click box over `{block}` at {cell:?} that reaches \
                         past its cell(s) by more than {CLICK_MARGIN}: it would take the press \
                         meant for the block beside it. Command: `{}`.",
                        line.trim()
                    ),
                ));
            }
            if door {
                b.two_cell += 1;
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
            "datapack/data/t/function/setup_finish.mcfunction".to_string(),
            format!("{}\n", lines.join("\n")).into_bytes(),
        );
        out
    }

    fn fit(cell: [i32; 3], block: &str) -> ClickBox {
        ClickBox::fitted_to(cell, block).unwrap_or_else(|| panic!("{block} has an outline"))
    }

    /// spec-0117 criterion 17: the fitted rule's values, derived from §2b by
    /// hand and asserted as exact decimal text.
    #[test]
    fn the_fitted_rule_gives_each_kind_its_box() {
        let lever = fit(
            [4, 65, 6],
            "minecraft:lever[face=floor,facing=north,powered=false]",
        );
        assert_eq!(lever.size_nbt(), "width:0.52f,height:0.395f");
        assert_eq!(lever.xyz(), "4.5 64.99 6.5");
        let button = fit(
            [4, 65, 6],
            "minecraft:stone_button[face=wall,facing=north,powered=false]",
        );
        assert_eq!(button.size_nbt(), "width:0.395f,height:0.27f");
        // Its outline's centre is z 6.9375; the box moves 0.125 toward the cell.
        assert_eq!(button.xyz(), "4.5 65.365 6.8125");
        for facing in ["north", "east", "south", "west"] {
            let door = fit(
                [4, 65, 6],
                &format!("minecraft:spruce_door[facing={facing},half=lower,hinge=left,open=false]"),
            );
            assert_eq!(door.size_nbt(), "width:1.02f,height:2.02f", "{facing}");
            assert_eq!(door.xyz(), "4.5 64.99 6.5", "{facing}");
        }
        let campfire = fit([4, 65, 6], "minecraft:campfire[lit=true]");
        assert_eq!(campfire.size_nbt(), "width:1.02f,height:0.4575f");
        let chest = fit([4, 65, 6], "minecraft:chest[facing=west,type=single]");
        assert_eq!(chest.size_nbt(), "width:0.895f,height:0.895f");
        assert_eq!(chest.xyz(), "4.5 64.99 6.5");
    }

    /// Every box the rule builds lies within its cells expanded by the margin
    /// and encloses every outline box by at least `MIN_PROTRUSION` — the two
    /// facts `DW1013` proves, over a spread of kinds and cells.
    #[test]
    fn every_fitted_box_passes_the_check() {
        let mut lines = Vec::new();
        let blocks = [
            "minecraft:lever[face=wall,facing=east,powered=false]",
            "minecraft:lever[face=ceiling,facing=south,powered=true]",
            "minecraft:oak_button[face=floor,facing=west,powered=false]",
            "minecraft:bell[attachment=single_wall,facing=north]",
            "minecraft:spruce_door[facing=east,half=lower,hinge=right,open=true]",
            "minecraft:chest[facing=north,type=left]",
            "minecraft:campfire",
            "minecraft:cobbled_deepslate",
            "minecraft:oak_trapdoor[facing=south,half=top,open=false]",
        ];
        for (i, block) in blocks.iter().enumerate() {
            let cell = [-16 + 3 * i as i32, -1 + i as i32, 7 - 2 * i as i32];
            lines.push(format!(
                "setblock {} {} {} {block}",
                cell[0], cell[1], cell[2]
            ));
            lines.push(fit(cell, block).summon(&format!(
                "response:1b,Invulnerable:1b,Tags:[\"dw_fixture\",\"dw_i_k{i}\"]"
            )));
        }
        let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        let b = check(&tree(&refs), &BTreeMap::new()).expect("the rule's own boxes enclose");
        assert_eq!(b.judged(), blocks.len());
        assert_eq!(b.two_cell, 1);
    }

    /// The perturbation only this check catches: a lever's box built one
    /// margin short on one face. Fitted, its south face stands at z 6.76, the
    /// margin past the outline's 12/16; moved 0.01 north it meets the outline at
    /// 6.75 exactly. Every face is off the grid, so the old face-off-the-grid
    /// check passes it; it reds here naming the face.
    #[test]
    fn a_box_one_margin_short_of_its_outline_is_refused() {
        let block = "minecraft:lever[face=floor,facing=north,powered=false]";
        let fitted = fit([4, 65, 6], block);
        assert_eq!(fitted.xyz(), "4.5 64.99 6.5");
        let out = tree(&[
            &format!("setblock 4 65 6 {block}"),
            "summon minecraft:interaction 4.5 64.99 6.49 {width:0.52f,height:0.395f,response:1b,Invulnerable:1b,Tags:[\"dw_fixture\",\"dw_trig_x\"]}",
        ]);
        let e = check(&out, &BTreeMap::new()).expect_err("a short box loses the pick");
        assert_eq!(e.code, DW_CLICK_BOX_FLUSH);
        assert!(e.message.contains("south face"), "{}", e.message);
        assert!(e.message.contains("sixteenth 12"), "{}", e.message);
    }

    /// A box over a cell `setup_finish` leaves as air has nothing under it.
    #[test]
    fn a_box_over_air_is_refused() {
        let out = tree(&[
            "summon minecraft:interaction 4.5 64.99 6.5 {width:1.02f,height:1.02f,response:1b,Invulnerable:1b,Tags:[\"dw_fixture\",\"dw_trig_x\"]}",
        ]);
        let e = check(&out, &BTreeMap::new()).expect_err("a box in the air");
        assert!(e.message.contains("[4, 65, 6]"), "{}", e.message);
    }

    /// A gate's own answer stands over the gate's block, which the prefab or a
    /// `close-gate` lays: judged against it.
    #[test]
    fn a_gate_box_is_judged_against_the_gate_block() {
        let bars = "minecraft:iron_bars";
        let b = fit([4, 65, 6], bars);
        let out = tree(&[&b.summon("Tags:[\"dw_fixture\",\"dw_gate_door\"]")]);
        let mut gates = BTreeMap::new();
        gates.insert([4, 65, 6], bars.to_string());
        assert_eq!(check(&out, &gates).unwrap().judged(), 1);
    }

    #[test]
    fn a_declared_size_moves_only_the_faces_on_the_grid() {
        let b = ClickBox::sized([0, 64, 0], 1.5, 2.5);
        assert_eq!(b.xyz(), "0.5 63.99 0.5");
        assert_eq!(b.size_nbt(), "width:1.5f,height:2.51f");
        let n = ClickBox::sized([0, 64, 0], 0.98, 1.0);
        assert_eq!(n.size_nbt(), "width:1.02f,height:1.02f");
        let w = ClickBox::sized([-3, 64, 2], 3.0, 2.0);
        assert_eq!(w.xyz(), "-2.5 63.99 2.5");
        assert_eq!(w.size_nbt(), "width:3.02f,height:2.02f");
    }

    /// An assembly's hitbox is judged against the grid, as before.
    #[test]
    fn an_assembly_box_flush_with_the_grid_is_refused() {
        let out = tree(&[
            "summon minecraft:interaction 4.5 65.0 6.5 {width:1.0f,height:2.0f,Tags:[\"dw_fixture\",\"dw_asm_crane_hit\"]}",
        ]);
        let e = check(&out, &BTreeMap::new()).expect_err("a flush box ties the block");
        assert!(e.message.contains("west face"), "{}", e.message);
        let ok = ClickBox::sized([4, 65, 6], 0.98, 2.5)
            .summon("Tags:[\"dw_fixture\",\"dw_asm_crane_hit\"]");
        assert_eq!(check(&tree(&[&ok]), &BTreeMap::new()).unwrap().assembly, 1);
    }

    #[test]
    fn borne_and_runtime_boxes_are_counted_not_judged() {
        let out = tree(&[
            "summon minecraft:interaction 4.5 65.0 6.5 {width:1.0f,height:2.0f,Tags:[\"dw_borne\",\"dw_npc_keeper\"]}",
            "execute unless entity @e[tag=dw_stk,distance=..1] run summon minecraft:interaction ~ ~ ~ {width:1.0f,height:2.0f,Tags:[\"dw_fixture\",\"dw_stk\"]}",
        ]);
        let b = check(&out, &BTreeMap::new()).expect("neither kind is judged");
        assert_eq!((b.summons, b.borne, b.runtime, b.judged()), (2, 1, 1, 0));
    }

    /// The line names what bears each borne box, counted from the summons'
    /// own tags: an NPC's dialogue hitbox and an actor's answer box (spec-0119).
    #[test]
    fn the_line_says_what_bears_each_borne_box() {
        let out = tree(&[
            "summon minecraft:interaction 4.5 65.0 6.5 {width:1.0f,height:2.0f,Tags:[\"dw_borne\",\"dw_npc_keeper\"]}",
            "summon minecraft:interaction 8.5 65.0 6.5 {width:1.0f,height:2.0f,Tags:[\"dw_borne\",\"dw_npc_clerk\",\"dw_trig_dw_answer_npc_clerk_struck\"]}",
            "execute at @e[tag=dw_pup_statue,limit=1] unless entity @e[tag=dw_abody_statue] run summon minecraft:interaction ~ ~ ~ {width:1.0f,height:2.0f,Tags:[\"dw_borne\",\"dw_actor_statue\",\"dw_abody_statue\"]}",
        ]);
        let b = check(&out, &BTreeMap::new()).expect("borne boxes are counted");
        assert_eq!(b.borne, 3);
        assert_eq!(b.borne_by.get(&Bearer::Npc), Some(&2));
        assert_eq!(b.borne_by.get(&Bearer::ActorAnswer), Some(&1));
        let line = b.line();
        assert!(
            line.contains("3 borne by a body (2 NPC dialogue hitbox(es), 1 actor answer box(es))"),
            "{line}"
        );
        assert_eq!(b.to_json()["borne_by"]["actor answer box(es)"], 1);
        let none = check(&tree(&[]), &BTreeMap::new()).unwrap().line();
        assert!(
            none.contains("0 borne by a body (0 NPC dialogue hitbox(es), 0 actor answer box(es))"),
            "{none}"
        );
    }

    #[test]
    fn code_is_dw1013() {
        assert_eq!(DW_CLICK_BOX_FLUSH, "DW1013");
    }
}
