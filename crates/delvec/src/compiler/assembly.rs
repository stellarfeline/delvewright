//! **A fixed thing that can be hit and hits back** (spec-0082): the assembly's
//! build-tier judgements and its emission.
//!
//! An assembly is a root `minecraft:item_display` at a [`Mark`], one
//! `minecraft:block_display` per rig part riding it, and — when declared — one
//! `minecraft:interaction` a player strikes. It moves through the clips of a
//! library rig ([`delvewright_dsl::rig`]) and, while a player stands in its
//! arming region, runs a strike pattern whose landing is an ordinary effect
//! list.
//!
//! ## What is judged here, and why here
//!
//! Each rule needs a cell, so each is build tier (exit 3), raised from
//! [`judge`] over one assembly at a time:
//!
//! * **`DW0936` — the hitbox.** Width over 6 or height over 22 has a slab vanilla
//!   never detects an attack on (Minecraft Wiki, *Interaction*, *Usage*: within
//!   3.3 blocks of its position toward −X/−Z, 19.3 toward +X/+Z, 22.6 above).
//!   The box must meet the footprint of what spawns — the initial clip's first
//!   frame, or the rest pose — so the player hits what the player sees (the
//!   `DW0420` rule, on this hardware). And a `strike-assembly` on an assembly
//!   with no hitbox is a beat that can never happen.
//! * **`DW0937` — reach.** A `strike-assembly` the critical path performs owes a
//!   standable cell of the party's population within
//!   [`crate::compiler::strand::STRIKE_REACH`] of the hitbox's box, measured by
//!   [`crate::compiler::strand::eye_reaches_box`], the measure `DW0924` uses.
//! * **`DW0938` — the strike.** Danger is visible, or the engine refuses it. A
//!   blow lands only where it was announced (every landing box, grown to its
//!   keep-out, lies inside the arming region's keep-out; a `damage-players`
//!   with no `in` strikes every player in the world and is refused), and the
//!   blow's area is the area the limb comes down on, both ways
//!   ([`correspondence`]): the strike clip's last frame comes down on every
//!   standable cell of the landing box — a part meets the footprint, within
//!   [`FLOOR_BAND`] of the floor, of a body standing there — and every
//!   standable cell it comes down on that its first frame did not is caught by
//!   the landing box, to within the box's keep-out ring. An aimed pattern is judged per
//!   facing a player in the arming region can draw, the box and the limb
//!   turned together.
//!
//! **Not judged, by a standing ruling** (spec-0016 §3, applied in spec-0082
//! §5.4): when a blow lands and how hard. The wind-up's length, the hold and
//! `amount` are the creator's, down to a one-frame wind-up, a hold of 0 and a
//! blow that kills an unhurt player. The staging record
//! (`validation/assembly.json`) states each step's numbers beside its caught
//! cells; the engine does not judge them.
//!
//! ## Emission
//!
//! Per assembly `<s>` (its id's local part, `safe_local`):
//!
//! * `asm_spawn_<s>` → `asm_summon_<s>` when no root stands: the root, the
//!   parts (each `ride … mount` the root — a star, so a `tp` of the root carries
//!   every part and no part depends on another's seat), the hitbox, and the
//!   state on `dw.sys` (`#asm_<s>_live` and the clip/frame/strike holders).
//! * `asm_despawn_<s>`: every entity tagged `dw_asm_<s>` killed; a display
//!   entity has no death, so nothing is seen.
//! * `asm_frame_<s>_<clip>_<frame>`: one `data merge` per part with the
//!   transform turned by `facing` and the clip's cadence as
//!   `interpolation_duration`.
//! * `asm_tick_<s>`, run from `tick` while the assembly is live: a tick counter
//!   against the clip's cadence, the frame counter's loop-or-hold rule
//!   (`asm_adv_<s>`), one macro dispatch (`asm_apply_<s>`), then the strike
//!   machine (idle → wind-up → hold → strike → land).
//! * `asm_cue_<s>_<clip>` — what `play-clip` calls: the clip becomes the one
//!   the assembly returns to, and plays now unless a strike step is in flight.
//!
//! Every entity is tagged `dw_fixture`: its position is engine state, so a
//! region `teleport` never carries an assembly away (and never unseats a part,
//! which a teleported passenger would be — spec-0082 §8 row 3).

use std::collections::BTreeSet;

use delvewright_dsl::metrics::{Body, keep_out_box};
use delvewright_dsl::rig::{self, Rig, Transform};
use delvewright_dsl::{
    Assembly, AssemblyHitbox, DwCode, ExitTier, Facing, QuestEffect, StealthZone, TriggerOn, Verb,
};

use crate::compiler::failure::Failure;
use crate::compiler::plan::{Plan, Step, safe_local};

/// `DW0936`: an assembly's hitbox is out of vanilla's attack-detection bounds,
/// does not meet the footprint of what spawns, or is absent where a
/// `strike-assembly` needs it.
pub const DW_ASSEMBLY_HITBOX: DwCode = DwCode::new("DW0936", ExitTier::Build);

/// `DW0937`: a `strike-assembly` the critical path performs has no cell of the
/// party's population within a strike of the hitbox.
pub const DW_ASSEMBLY_REACH: DwCode = DwCode::new("DW0937", ExitTier::Build);

/// `DW0938`: a blow lands where it was not announced, or where the limb is not.
pub const DW_ASSEMBLY_STRIKE: DwCode = DwCode::new("DW0938", ExitTier::Build);

/// The widest hitbox vanilla detects an attack on across its whole face: an
/// interaction registers attacks only within 3.3 blocks of its position toward
/// −X/−Z (Minecraft Wiki, *Interaction*, *Usage*), so a half-width over 3 is a
/// slab nothing can hit. 6, the even bound under it.
pub const MAX_HITBOX_WIDTH: f64 = 6.0;

/// The tallest: attacks register within 22.6 blocks above the position; 22,
/// the whole-block bound under it.
pub const MAX_HITBOX_HEIGHT: f64 = 22.0;

/// How high above the floor a blow is judged to have landed, in blocks: the
/// floor band (spec-0082 §5.4 shape 2). A limb that came down is on the floor
/// where it lands; one hanging above a body's knees has not landed on it.
pub const FLOOR_BAND: f64 = 1.0;

/// **A standing body's footprint in the floor band** at a feet cell, relative
/// to the mark's cell (which spans `[0, 1]`): the body's own width round the
/// cell's centre, from its floor up [`FLOOR_BAND`]. What a landed limb meets
/// when it comes down where a player stands.
pub fn floor_band_body(f: [i32; 3]) -> ([f64; 3], [f64; 3]) {
    let half = Body::PLAYER.half_width();
    let (x, y, z) = (f64::from(f[0]), f64::from(f[1]), f64::from(f[2]));
    (
        [x + 0.5 - half, y, z + 0.5 - half],
        [x + 0.5 + half, y + FLOOR_BAND, z + 0.5 + half],
    )
}

/// The widening, in degrees, of each facing's sector when the compiler asks
/// which facings a player in the arming region can draw (spec-0082 §5.7): the
/// run-time choice reads the bearing as a whole number of `1/facings` degrees,
/// so a bearing within this of a sector's edge may fall either side, and both
/// facings are proved.
pub const AIM_SECTOR_MARGIN_DEG: f64 = 0.5;

/// The `interpolation_duration` a return to the rest pose is drawn over, in
/// ticks: the spike's own summon cadence (spec-0082 §8).
pub const REST_INTERPOLATION: u32 = 5;

/// How many cells a refusal names before it counts the rest.
const NAME_LIMIT: usize = 24;

/// The NBT storage the frame dispatch reads its clip and frame from.
pub const STORAGE: &str = "dw:asm";

/// One assembly whose mark resolved and whose rig the build holds.
pub struct Placed<'a> {
    /// Index in `assemblies[]`.
    pub index: usize,
    /// The declaration.
    pub decl: &'a Assembly,
    /// Its rig.
    pub rig: &'a Rig,
    /// The function/tag-safe local id.
    pub safe: String,
    /// The area its anchor resolved in.
    pub area: String,
    /// The mark's cell: the anchor's cell plus the offset.
    pub mark: [i32; 3],
}

/// Every assembly whose anchor resolves and whose rig the plan holds, in
/// declaration order. An unresolved anchor is `DW0360`'s (raised by
/// `emit::check_effect_anchors`), a missing rig `DW0935`'s.
pub fn placed<'a>(plan: &'a Plan<'a>) -> Vec<Placed<'a>> {
    let mut out = Vec::new();
    for (index, a) in plan.campaign.quests.content.assemblies.iter().enumerate() {
        let Some(rig) = plan.rigs.get(a.rig.as_str()) else {
            continue;
        };
        let Some((area, cell)) = plan.point_any_site(a.at.anchor.as_str()) else {
            continue;
        };
        out.push(Placed {
            index,
            decl: a,
            rig,
            safe: safe_local(a.id.as_str()),
            area,
            mark: a.at.cell(cell),
        });
    }
    out
}

impl Placed<'_> {
    /// The cell the hitbox's bottom centre stands in.
    pub fn hitbox_cell(&self) -> [i32; 3] {
        let o = self
            .decl
            .hitbox
            .as_ref()
            .map(|h| h.offset)
            .unwrap_or([0; 3]);
        delvewright_dsl::offset_cell(self.mark, o)
    }

    /// The pose the parts are summoned in: the initial clip's first frame, or
    /// the rest pose.
    pub fn spawn_pose(&self) -> Vec<Transform> {
        self.decl
            .initial
            .as_ref()
            .and_then(|c| self.rig.clips.get(c))
            .and_then(|c| c.frames.first().cloned())
            .or_else(|| self.rig.rest_pose())
            .unwrap_or_default()
    }

    /// The world cells a pose's parts meet.
    pub fn world_cells(&self, frame: &[Transform]) -> BTreeSet<[i32; 3]> {
        offset_all(&rig::frame_footprint(frame, self.decl.facing()), self.mark)
    }
}

/// Where a strike step that sets its own pace (`ticks_per_frame`) plays its
/// clips: two emitted clips after the rig's own, its wind-up then its strike,
/// in step order, counting only steps whose clips the rig holds. `None` for a
/// step at the clips' own pace.
pub fn paced_index(
    rig: &Rig,
    pattern: &[delvewright_dsl::StrikeStep],
    j: usize,
) -> Option<(usize, usize)> {
    let mut next = rig.clips.len();
    for (i, step) in pattern.iter().enumerate() {
        if step.ticks_per_frame.is_none() {
            continue;
        }
        let w = rig.clips.contains_key(&step.windup).then(|| {
            next += 1;
            next - 1
        });
        let m = rig.clips.contains_key(&step.strike).then(|| {
            next += 1;
            next - 1
        });
        if i == j {
            return w.zip(m);
        }
    }
    None
}

/// The drawn facing a run-time pick `k` resolves to: `k` itself when it is
/// drawn, else the drawn facing fewest steps round from it (the lower on a
/// tie). The compiler emits no facing it has not proved.
fn nearest_drawn(drawn: &[u32], k: u32, n: u32) -> u32 {
    drawn
        .iter()
        .copied()
        .min_by_key(|&d| {
            let a = (i64::from(d) - i64::from(k)).rem_euclid(i64::from(n));
            (a.min(i64::from(n) - a), d)
        })
        .unwrap_or(0)
}

/// Shift a set of mark-relative cells to the world.
fn offset_all(cells: &BTreeSet<[i32; 3]>, mark: [i32; 3]) -> BTreeSet<[i32; 3]> {
    cells
        .iter()
        .map(|c| delvewright_dsl::offset_cell(mark, *c))
        .collect()
}

/// A hitbox's continuous box, from its bottom-centre cell.
pub fn hitbox_box(cell: [i32; 3], h: &AssemblyHitbox) -> ([f64; 3], [f64; 3]) {
    let cx = f64::from(cell[0]) + 0.5;
    let cz = f64::from(cell[2]) + 0.5;
    let y = f64::from(cell[1]);
    let half = h.width / 2.0;
    (
        [cx - half, y, cz - half],
        [cx + half, y + h.height, cz + half],
    )
}

/// Whether a cell's unit box overlaps a continuous box with positive volume.
fn cell_meets(c: [i32; 3], lo: [f64; 3], hi: [f64; 3]) -> bool {
    (0..3).all(|i| {
        let a = f64::from(c[i]);
        a < hi[i] && a + 1.0 > lo[i]
    })
}

/// Whether `c` lies in the inclusive box `lo..=hi`.
fn in_box(c: [i32; 3], (lo, hi): ([i32; 3], [i32; 3])) -> bool {
    (0..3).all(|i| lo[i] <= c[i] && c[i] <= hi[i])
}

// ---------------------------------------------------------------------------
// The judgement
// ---------------------------------------------------------------------------

/// One landing: a `damage-players` inside a step's `on_land`, at any depth.
#[derive(Clone, Debug)]
pub struct Landing {
    /// JSON pointer to the effect.
    pub path: String,
    /// Its `in` box, resolved; `None` when the effect declares no `in`.
    pub within: Option<([i32; 3], [i32; 3])>,
    /// Whether it declared an `in` at all (a declared box whose anchor does
    /// not resolve is `DW0360`'s and is skipped here).
    pub declares_in: bool,
    /// `amount`, for the staging record.
    pub amount: u32,
    /// Whether it stands inside another effect's list rather than at the top
    /// of `on_land` — where an aimed pattern cannot turn it.
    pub nested: bool,
}

/// One strike step, as the judgement reads it.
#[derive(Clone, Debug)]
pub struct StepSubject<'a> {
    /// The step's index in the pattern.
    pub index: usize,
    /// The wind-up clip.
    pub windup: &'a str,
    /// The hold.
    pub hold: u32,
    /// The strike clip.
    pub strike: &'a str,
    /// The step's own pace, when it sets one.
    pub ticks_per_frame: Option<u32>,
    /// The landings in its `on_land`.
    pub landings: Vec<Landing>,
}

/// An inclusive box of cells, `(lo, hi)`.
pub type CellBox = ([i32; 3], [i32; 3]);

/// Whether a strike from somewhere the party can walk reaches a box, asked per
/// performing trigger: `(trigger, lo, hi)`.
pub type Reaches<'r> = dyn Fn(&str, [f64; 3], [f64; 3]) -> bool + 'r;

/// What [`judge`] reads of one assembly: resolved, so it can be built by hand
/// in a test.
pub struct Subject<'a> {
    /// The assembly id.
    pub id: &'a str,
    /// JSON pointer to the declaration.
    pub path: String,
    /// The mark's cell.
    pub mark: [i32; 3],
    /// Its facing.
    pub facing: Facing,
    /// Its rig.
    pub rig: &'a Rig,
    /// The declared initial clip.
    pub initial: Option<&'a str>,
    /// The declared hitbox.
    pub hitbox: Option<&'a AssemblyHitbox>,
    /// The arming region, resolved, with the steps — `None` without `strikes`.
    pub strikes: Option<(CellBox, Vec<StepSubject<'a>>)>,
    /// `strike-assembly` triggers naming it, by id.
    pub struck_by: Vec<&'a str>,
    /// Of those, the ones the critical path performs.
    pub performed: Vec<&'a str>,
    /// The aimed pattern's facing count (`strikes.aim.facings`); `None` when
    /// the pattern is not aimed.
    pub aim: Option<u32>,
}

impl Subject<'_> {
    /// How many facings the pattern is spaced over: the aim's count, or 1.
    pub fn facing_count(&self) -> u32 {
        self.aim.unwrap_or(1)
    }

    /// The facings a blow can take: for an aimed pattern every facing a player
    /// in the arming region can draw ([`drawn_facings`]), else the declared
    /// facing alone.
    pub fn facings(&self) -> Vec<u32> {
        match (&self.strikes, self.aim) {
            (Some((arming, _)), Some(n)) => {
                drawn_facings(self.mark, *arming, n, rig::facing_angle(self.facing))
            }
            _ => vec![0],
        }
    }
}

// ---------------------------------------------------------------------------
// Facings (spec-0082 §5.7)
// ---------------------------------------------------------------------------

/// The turn, in radians about `+y` (`+z` toward `+x`), of facing `k` of `n`
/// from the declared facing.
pub fn facing_turn(k: u32, n: u32) -> f64 {
    if k == 0 {
        0.0
    } else {
        std::f64::consts::TAU * f64::from(k) / f64::from(n)
    }
}

/// The root's yaw, in Minecraft degrees, that draws facing `k` of `n`. A
/// display entity draws its transformation turned by its own yaw, and a turn of
/// `+a` about `+y` is a yaw of `-a` (yaw 90 faces west, `-x`). The rig's frames
/// already carry the declared facing, so facing 0 is yaw 0. In `(-180, 180]`.
pub fn root_yaw(k: u32, n: u32) -> f64 {
    let mut y = -360.0 * f64::from(k) / f64::from(n);
    while y <= -180.0 {
        y += 360.0;
    }
    if y == 0.0 { 0.0 } else { y }
}

/// A yaw as a command token: at most four decimals, trailing zeros dropped.
pub fn yaw_token(y: f64) -> String {
    let t = format!("{y:.4}");
    let t = t.trim_end_matches('0').trim_end_matches('.').to_string();
    if t == "-0" { "0".to_string() } else { t }
}

/// Wrap an angle to `(-π, π]`.
fn wrap(a: f64) -> f64 {
    let t = std::f64::consts::TAU;
    let mut a = a % t;
    if a <= -std::f64::consts::PI {
        a += t;
    }
    if a > std::f64::consts::PI {
        a -= t;
    }
    a
}

/// The bearing, as a turn about `+y` from `+z`, of a horizontal offset.
fn bearing(dx: f64, dz: f64) -> f64 {
    dx.atan2(dz)
}

/// **The facings a player in the arming region can draw** (spec-0082 §5.7):
/// those of the `n` facings (spaced from `base`, the declared facing's turn)
/// whose sector — half a step either side, widened by
/// [`AIM_SECTOR_MARGIN_DEG`] — meets the bearings, from the mark cell's
/// centre, of every position a body selected by the arming region can stand
/// at (the region grown by the body's half-width). All of them when the mark
/// lies inside that. Ascending; never empty.
pub fn drawn_facings(mark: [i32; 3], arming: CellBox, n: u32, base: f64) -> Vec<u32> {
    if n <= 1 {
        return vec![0];
    }
    let half = Body::PLAYER.half_width();
    let lo = [f64::from(arming.0[0]) - half, f64::from(arming.0[2]) - half];
    let hi = [
        f64::from(arming.1[0]) + 1.0 + half,
        f64::from(arming.1[2]) + 1.0 + half,
    ];
    let m = [f64::from(mark[0]) + 0.5, f64::from(mark[2]) + 0.5];
    if lo[0] <= m[0] && m[0] <= hi[0] && lo[1] <= m[1] && m[1] <= hi[1] {
        return (0..n).collect();
    }
    let centre = bearing((lo[0] + hi[0]) / 2.0 - m[0], (lo[1] + hi[1]) / 2.0 - m[1]);
    let rel: Vec<f64> = [
        [lo[0], lo[1]],
        [lo[0], hi[1]],
        [hi[0], lo[1]],
        [hi[0], hi[1]],
    ]
    .iter()
    .map(|c| wrap(bearing(c[0] - m[0], c[1] - m[1]) - centre))
    .collect();
    let rmin = rel.iter().copied().fold(f64::INFINITY, f64::min);
    let rmax = rel.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let mid = centre + (rmin + rmax) / 2.0;
    let half_span = (rmax - rmin) / 2.0;
    let step = std::f64::consts::TAU / f64::from(n);
    let margin = AIM_SECTOR_MARGIN_DEG.to_radians();
    let out: Vec<u32> = (0..n)
        .filter(|&k| {
            let at = base + facing_turn(k, n);
            wrap(at - mid).abs() <= half_span + step / 2.0 + margin
        })
        .collect();
    if out.is_empty() { vec![0] } else { out }
}

/// The facing the run-time choice draws for a body at `cell`: the facing
/// nearest the bearing from the mark cell's centre to the cell's centre.
pub fn facing_for(mark: [i32; 3], cell: [i32; 3], n: u32, base: f64) -> u32 {
    if n <= 1 {
        return 0;
    }
    let b = bearing(f64::from(cell[0] - mark[0]), f64::from(cell[2] - mark[2]));
    let step = std::f64::consts::TAU / f64::from(n);
    let k = (wrap(b - base) / step).round() as i64;
    k.rem_euclid(i64::from(n)) as u32
}

/// **A landing box turned to a facing** (spec-0082 §5.7): every cell whose
/// centre, turned back by `turn` about the vertical axis through the mark
/// cell's centre, lies in the box; the box's own cells at turn 0. Every
/// course of the box keeps its height.
pub fn turned_region(mark: [i32; 3], (lo, hi): CellBox, turn: f64) -> BTreeSet<[i32; 3]> {
    if turn == 0.0 {
        return cells_of((lo, hi)).into_iter().collect();
    }
    let m = [f64::from(mark[0]) + 0.5, f64::from(mark[2]) + 0.5];
    let (sn, cs) = turn.sin_cos();
    // Forward: (x, z) turned by +turn about the mark.
    let fwd = |x: f64, z: f64| {
        let (dx, dz) = (x - m[0], z - m[1]);
        (m[0] + dx * cs + dz * sn, m[1] - dx * sn + dz * cs)
    };
    let back = |x: f64, z: f64| {
        let (dx, dz) = (x - m[0], z - m[1]);
        (m[0] + dx * cs - dz * sn, m[1] + dx * sn + dz * cs)
    };
    let (x0, x1) = (f64::from(lo[0]), f64::from(hi[0]) + 1.0);
    let (z0, z1) = (f64::from(lo[2]), f64::from(hi[2]) + 1.0);
    let corners = [fwd(x0, z0), fwd(x0, z1), fwd(x1, z0), fwd(x1, z1)];
    let bx0 = corners
        .iter()
        .map(|c| c.0)
        .fold(f64::INFINITY, f64::min)
        .floor() as i32
        - 1;
    let bx1 = corners
        .iter()
        .map(|c| c.0)
        .fold(f64::NEG_INFINITY, f64::max)
        .ceil() as i32
        + 1;
    let bz0 = corners
        .iter()
        .map(|c| c.1)
        .fold(f64::INFINITY, f64::min)
        .floor() as i32
        - 1;
    let bz1 = corners
        .iter()
        .map(|c| c.1)
        .fold(f64::NEG_INFINITY, f64::max)
        .ceil() as i32
        + 1;
    const EPS: f64 = 1e-9;
    let mut out = BTreeSet::new();
    for x in bx0..=bx1 {
        for z in bz0..=bz1 {
            let (px, pz) = back(f64::from(x) + 0.5, f64::from(z) + 0.5);
            if x0 <= px + EPS && px < x1 - EPS && z0 <= pz + EPS && pz < z1 - EPS {
                for y in lo[1]..=hi[1] {
                    out.insert([x, y, z]);
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// The correspondence (spec-0082 §5.4 shape 2)
// ---------------------------------------------------------------------------

/// **The standable cells a pose lands on**: every walked cell whose standing
/// body's footprint in the floor band ([`floor_band_body`]) a part of `frame`,
/// turned `turn` about the mark, meets — judged exactly, part box against
/// footprint box. World cells.
pub fn struck_cells(
    frame: &[Transform],
    turn: f64,
    mark: [i32; 3],
    population: &dyn Fn([i32; 3]) -> bool,
) -> BTreeSet<[i32; 3]> {
    let mut out = BTreeSet::new();
    for t in frame {
        let t = t.turned(turn);
        for c in t.cells() {
            let f = delvewright_dsl::offset_cell(mark, c);
            if out.contains(&f) || !population(f) {
                continue;
            }
            let (lo, hi) = floor_band_body(c);
            if t.meets_box(lo, hi) {
                out.insert(f);
            }
        }
    }
    out
}

/// The standable cells a body can be caught from by a landing region: the
/// union of every region cell's keep-out ([`keep_out_box`]) — the region and
/// the ring round it a body standing at its edge reaches into.
pub fn caught_cells(
    region: &BTreeSet<[i32; 3]>,
    population: &dyn Fn([i32; 3]) -> bool,
) -> BTreeSet<[i32; 3]> {
    let mut out = BTreeSet::new();
    for c in region {
        for k in cells_of(keep_out_box(Body::PLAYER, *c, *c)) {
            if population(k) {
                out.insert(k);
            }
        }
    }
    out
}

/// **What a landing and its limb disagree on** — the one rule of the strike's
/// correspondence, both ways (spec-0082 §5.4 shape 2, settled in §11: a
/// strike's hit area corresponds as closely as it can to what its animation
/// shows).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Correspondence {
    /// Standable cells of the landing region the strike clip's last frame does
    /// not land on: no part meets the footprint, in the floor band, of a body
    /// standing there. A blow lands where no limb is seen.
    pub unstruck: Vec<[i32; 3]>,
    /// Standable cells the last frame comes down on — it lands on a body
    /// standing there and the strike clip's first frame did not — that the
    /// landing does not catch, even by its keep-out ring (the stated
    /// tolerance: one cell round the region for a player, a body there reaching
    /// into it). A limb comes down where no blow lands.
    pub uncaught: Vec<[i32; 3]>,
    /// The standable cells of the region (what `unstruck` is drawn from).
    pub landing: Vec<[i32; 3]>,
    /// Every standable cell a body can be caught from.
    pub caught: Vec<[i32; 3]>,
    /// The standable cells the last frame comes down on (what `uncaught` is
    /// drawn from).
    pub comes_down: Vec<[i32; 3]>,
    /// The standable cells the last frame lands on.
    pub struck: Vec<[i32; 3]>,
    /// Whether anything comes down within the blow's catch: some cell the
    /// last frame lands on, and the first frame did not, is caught. A strike
    /// clip that brings nothing down onto the landing — every part over it
    /// stood there before the strike began — is no blow at all.
    pub delivered: bool,
}

impl Correspondence {
    /// Whether the two agree.
    pub fn holds(&self) -> bool {
        self.unstruck.is_empty() && self.uncaught.is_empty() && self.delivered
    }
}

/// Judge a landing region against what the limb lands on: `struck`, the
/// cells its last frame lands on, and `before`, the cells the pose the strike
/// began from already stood on ([`struck_cells`] at the facing the region is
/// turned to).
pub fn correspondence(
    region: &BTreeSet<[i32; 3]>,
    before: &BTreeSet<[i32; 3]>,
    struck: &BTreeSet<[i32; 3]>,
    population: &dyn Fn([i32; 3]) -> bool,
) -> Correspondence {
    let landing: BTreeSet<[i32; 3]> = region.iter().copied().filter(|c| population(*c)).collect();
    let caught = caught_cells(region, population);
    let comes_down: BTreeSet<[i32; 3]> = struck.difference(before).copied().collect();
    let delivered = comes_down.iter().any(|c| caught.contains(c));
    Correspondence {
        unstruck: landing.difference(struck).copied().collect(),
        uncaught: comes_down.difference(&caught).copied().collect(),
        landing: landing.into_iter().collect(),
        caught: caught.into_iter().collect(),
        comes_down: comes_down.into_iter().collect(),
        struck: struck.iter().copied().collect(),
        delivered,
    }
}

/// One step's record, for the staging artifact.
#[derive(Clone, Debug, PartialEq)]
pub struct StepRecord {
    /// The assembly.
    pub assembly: String,
    /// The step index.
    pub step: usize,
    /// Ticks from the step beginning to the wind-up's last frame.
    pub windup_ticks: u32,
    /// The hold.
    pub hold: u32,
    /// Ticks from the strike clip starting to the landing: to the tick the
    /// client has drawn its last frame whole ([`rig::Clip::landing_ticks`]).
    pub strike_ticks: u32,
    /// Every landing's `amount`.
    pub amounts: Vec<u32>,
    /// Every caught cell of every landing box at every facing, in the world.
    pub caught: Vec<[i32; 3]>,
    /// How many facings the pattern is spaced over (1 when not aimed).
    pub facing_count: u32,
    /// Per facing a blow can take.
    pub facings: Vec<FacingRecord>,
}

/// One facing of one step, for the staging artifact and the bot.
#[derive(Clone, Debug, PartialEq)]
pub struct FacingRecord {
    /// The facing's index, `0..facing_count`.
    pub k: u32,
    /// The root's yaw that draws it.
    pub yaw: f64,
    /// The standable cells its landing regions catch a body from.
    pub caught: Vec<[i32; 3]>,
    /// The standable cells its limb comes down on.
    pub comes_down: Vec<[i32; 3]>,
    /// A cell of the landing, inside the arming region and under the limb,
    /// from which a body draws this facing: where the bot stands to take the
    /// blow. `None` when there is none.
    pub stand: Option<[i32; 3]>,
}

/// What one [`judge`] examined.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Judged {
    /// Hitboxes examined.
    pub hitboxes: usize,
    /// Strike steps checked.
    pub steps: usize,
    /// Facings judged across those steps.
    pub facings: usize,
    /// Per-step records.
    pub records: Vec<StepRecord>,
}

/// **Judge one assembly** (`DW0936`, `DW0937`, `DW0938`, spec-0082 §5.3–§5.4).
///
/// `population` answers whether a cell is in the walked population `P`
/// ([`crate::compiler::lethal::walked_population`]); `reaches(trigger, lo,
/// hi)` whether some cell the party can walk to while that trigger's step is
/// next holds an eye within a strike of the box.
/// Returns what was examined beside every refusal, in rule order.
pub fn judge(
    s: &Subject<'_>,
    population: &dyn Fn([i32; 3]) -> bool,
    reaches: &Reaches<'_>,
) -> (Judged, Vec<Failure>) {
    let mut j = Judged::default();
    let mut out: Vec<Failure> = Vec::new();

    // ---- DW0936: the hitbox ----
    match s.hitbox {
        None => {
            for t in &s.struck_by {
                out.push(Failure::new(
                    DW_ASSEMBLY_HITBOX,
                    format!(
                        "trigger `{t}` fires on `strike-assembly` at assembly `{}`, which declares \
                         no `hitbox` — there is nothing for a player to strike, so the trigger can \
                         never fire and its effects never run. Give the assembly a `hitbox` \
                         ({{width, height}}, the box a player hits), or drop the trigger",
                        s.id
                    ),
                ));
            }
        }
        Some(h) => {
            j.hitboxes += 1;
            if !(h.width > 0.0 && h.width <= MAX_HITBOX_WIDTH) {
                out.push(Failure::new(
                    DW_ASSEMBLY_HITBOX,
                    format!(
                        "assembly `{}` ({}/hitbox/width) declares a hitbox {} wide. Vanilla detects \
                         an attack on a `minecraft:interaction` only within 3.3 blocks of its \
                         position toward -X and -Z (Minecraft Wiki, Interaction), so a box wider \
                         than {MAX_HITBOX_WIDTH} has a slab no swing registers on. Declare a width \
                         over 0 and at most {MAX_HITBOX_WIDTH}",
                        s.id, s.path, h.width
                    ),
                ));
            }
            if !(h.height > 0.0 && h.height <= MAX_HITBOX_HEIGHT) {
                out.push(Failure::new(
                    DW_ASSEMBLY_HITBOX,
                    format!(
                        "assembly `{}` ({}/hitbox/height) declares a hitbox {} tall. Vanilla \
                         detects an attack on a `minecraft:interaction` only within 22.6 blocks \
                         above its position (Minecraft Wiki, Interaction), so a box taller than \
                         {MAX_HITBOX_HEIGHT} has a slab no swing registers on. Declare a height \
                         over 0 and at most {MAX_HITBOX_HEIGHT}",
                        s.id, s.path, h.height
                    ),
                ));
            }
            let cell = delvewright_dsl::offset_cell(s.mark, h.offset);
            let (lo, hi) = hitbox_box(cell, h);
            // ---- DW0937: reach, for the strikes the path performs ----
            // Asked before the mark rule: a box no swing can reach is the
            // first thing wrong with it, wherever its parts stand.
            for t in &s.performed {
                if !reaches(t, lo, hi) {
                    out.push(Failure::new(
                        DW_ASSEMBLY_REACH,
                        format!(
                            "the critical path performs trigger `{t}`, a `strike-assembly` on \
                             assembly `{}`, and no cell the party can walk to holds an eye within \
                             a strike ({} blocks, the player's interaction range) of its hitbox \
                             at {lo:?}..{hi:?}. The beat can never happen: the party walks up and \
                             cannot reach the thing. Lower the mark or the hitbox's `offset` \
                             toward a floor the party stands on, or give the party footing \
                             within reach",
                            s.id,
                            crate::compiler::strand::STRIKE_REACH
                        ),
                    ));
                }
            }
            let pose = s
                .initial
                .and_then(|c| s.rig.clips.get(c))
                .and_then(|c| c.frames.first().cloned())
                .or_else(|| s.rig.rest_pose())
                .unwrap_or_default();
            // At every facing the pattern can turn it to: a turned thing goes
            // back to its clip at the facing it struck from.
            let base = rig::facing_angle(s.facing);
            let n = s.facing_count();
            for k in s.facings() {
                let seen = offset_all(
                    &rig::frame_footprint_turned(&pose, base + facing_turn(k, n)),
                    s.mark,
                );
                if !seen.iter().any(|c| cell_meets(*c, lo, hi)) {
                    out.push(Failure::new(
                        DW_ASSEMBLY_HITBOX,
                        format!(
                            "assembly `{}`'s hitbox ({}/hitbox) spans {lo:?}..{hi:?} and meets none \
                             of the {} cell(s) its parts stand in when it spawns ({}){}: {}. The \
                             player strikes what the player sees, and a box beside the thing is a \
                             box nobody aims at. Move the hitbox's `offset` (or size it) so it \
                             covers the parts",
                            s.id,
                            s.path,
                            seen.len(),
                            match s.initial {
                                Some(c) => format!("the first frame of `{c}`"),
                                None => "the rest pose".to_string(),
                            },
                            facing_words(k, n),
                            rig::cells_line(&seen)
                        ),
                    ));
                }
            }
        }
    }

    // ---- DW0938: the strike ----
    let Some((arming, steps)) = &s.strikes else {
        return (j, out);
    };
    if steps.is_empty() {
        out.push(Failure::new(
            DW_ASSEMBLY_STRIKE,
            format!(
                "assembly `{}` declares `strikes` with an empty `pattern`: an arming region and no \
                 blow. Write at least one step, or drop `strikes`",
                s.id
            ),
        ));
    }
    let body = Body::PLAYER;
    let arm_keep = keep_out_box(body, arming.0, arming.1);
    let base = rig::facing_angle(s.facing);
    let n = s.facing_count();
    let facings = s.facings();
    for step in steps {
        j.steps += 1;
        let strike_clip = s.rig.clips.get(step.strike);
        let windup_clip = s.rig.clips.get(step.windup);
        let mut record = StepRecord {
            assembly: s.id.to_string(),
            step: step.index,
            windup_ticks: windup_clip
                .map(|c| paced(c, step.ticks_per_frame).0)
                .unwrap_or(0),
            hold: step.hold,
            strike_ticks: strike_clip
                .map(|c| paced(c, step.ticks_per_frame).1)
                .unwrap_or(0),
            amounts: step.landings.iter().map(|l| l.amount).collect(),
            caught: Vec::new(),
            facing_count: n,
            facings: Vec::new(),
        };
        // What the limb lands on at each facing: the pose the strike began
        // from, and its last frame.
        let limb_at = |k: u32| -> (BTreeSet<[i32; 3]>, BTreeSet<[i32; 3]>) {
            let turn = base + facing_turn(k, n);
            let lands = |f: Option<&Vec<Transform>>| {
                f.map(|f| struck_cells(f, turn, s.mark, population))
                    .unwrap_or_default()
            };
            (
                lands(strike_clip.and_then(|c| c.frames.first())),
                lands(strike_clip.and_then(|c| c.frames.last())),
            )
        };
        let mut per_facing: Vec<FacingRecord> = facings
            .iter()
            .map(|&k| FacingRecord {
                k,
                yaw: root_yaw(k, n),
                caught: Vec::new(),
                comes_down: Vec::new(),
                stand: None,
            })
            .collect();
        for l in &step.landings {
            let Some(within) = l.within else {
                if !l.declares_in {
                    out.push(Failure::new(
                        DW_ASSEMBLY_STRIKE,
                        format!(
                            "assembly `{}`'s strike step {} lands a `damage-players` with no `in` \
                             ({}). A landing runs with no acting player, so a box-less blow \
                             strikes every player in the world, wherever they stand — a blow \
                             nobody was shown. Give it an `in` box inside the arming region, \
                             under the strike clip's last frame",
                            s.id, step.index, l.path
                        ),
                    ));
                }
                continue;
            };
            if s.aim.is_some() && l.nested {
                out.push(Failure::new(
                    DW_ASSEMBLY_STRIKE,
                    format!(
                        "assembly `{}`'s strike pattern is aimed, and its step {} lands a \
                         `damage-players` ({}) inside another effect's list. An aimed pattern \
                         turns every landing box with the facing it strikes from, and only a box \
                         at the top of `on_land` is turned; this one would land where the first \
                         facing put it whichever way the thing turned. Move the `damage-players` \
                         to the top of `on_land` (a `when` on it is kept)",
                        s.id, step.index, l.path
                    ),
                ));
                continue;
            }
            for (fi, &k) in facings.iter().enumerate() {
                let region = turned_region(s.mark, within, facing_turn(k, n));
                // Shape 1: it lands only where it was announced.
                let outside: Vec<[i32; 3]> = region
                    .iter()
                    .copied()
                    .filter(|c| {
                        let keep = keep_out_box(body, *c, *c);
                        !(in_box(keep.0, arm_keep) && in_box(keep.1, arm_keep))
                    })
                    .collect();
                if !outside.is_empty() {
                    let keep = region
                        .iter()
                        .fold(([i32::MAX; 3], [i32::MIN; 3]), |(lo, hi), c| {
                            let (a, b) = keep_out_box(body, *c, *c);
                            (
                                [lo[0].min(a[0]), lo[1].min(a[1]), lo[2].min(a[2])],
                                [hi[0].max(b[0]), hi[1].max(b[1]), hi[2].max(b[2])],
                            )
                        });
                    out.push(Failure::new(
                        DW_ASSEMBLY_STRIKE,
                        format!(
                            "assembly `{}`'s strike step {} lands a blow ({}){} whose box catches \
                             a body from the feet cells {:?}..={:?}, and the arming region \
                             `while_in` catches one only from {:?}..={:?}. A player who never \
                             entered the arming region would be struck by a blow that was never \
                             wound up for them. Shrink or move the landing box inside `while_in`, \
                             or widen `while_in` to cover it",
                            s.id,
                            step.index,
                            l.path,
                            facing_words(k, n),
                            keep.0,
                            keep.1,
                            arm_keep.0,
                            arm_keep.1
                        ),
                    ));
                    continue;
                }
                // Shape 2: the blow's area is the area the limb comes down on.
                let (first, last) = limb_at(k);
                let c = correspondence(&region, &first, &last, population);
                j.facings += 1;
                record.caught.extend(c.caught.iter().copied());
                let fr = &mut per_facing[fi];
                fr.caught.extend(c.caught.iter().copied());
                fr.comes_down.extend(c.comes_down.iter().copied());
                if fr.stand.is_none() {
                    fr.stand = stand_cell(s.mark, *arming, &c, k, n, base);
                }
                if !c.unstruck.is_empty() {
                    out.push(Failure::new(
                        DW_ASSEMBLY_STRIKE,
                        format!(
                            "assembly `{}`'s strike step {} lands a blow ({}){} on {} standable \
                             cell(s) the strike clip `{}` never comes down on — on its last \
                             frame, drawn whole when the blow lands, no part meets a body \
                             standing there within {FLOOR_BAND} block of the floor (the body's \
                             own width round the cell's centre): {}. The thing the player saw \
                             come down must be the thing that hurt them. Where the last frame \
                             comes down on a standing body: {}. Move the landing box under the \
                             limb (`delvec rig describe` prints the footprint), or choose a \
                             strike clip that comes down on it",
                            s.id,
                            step.index,
                            l.path,
                            facing_words(k, n),
                            c.unstruck.len(),
                            step.strike,
                            cells_named(&c.unstruck),
                            if c.struck.is_empty() {
                                "nowhere — it comes down on no standable cell".to_string()
                            } else {
                                cells_named(&c.struck)
                            }
                        ),
                    ));
                }
                if c.unstruck.is_empty() && !c.landing.is_empty() && !c.delivered {
                    out.push(Failure::new(
                        DW_ASSEMBLY_STRIKE,
                        format!(
                            "assembly `{}`'s strike step {} lands a blow ({}){} that nothing \
                             comes down onto: the strike clip `{}` lands on no cell the blow \
                             catches that its first frame did not already stand on — what is \
                             over the landing box when the blow lands was there before the \
                             strike began, so a player there is hurt by a blow nobody saw \
                             fall. Choose a strike clip that comes down on the landing box, or \
                             move the box to where it comes down (`delvec rig describe` prints \
                             the footprint)",
                            s.id,
                            step.index,
                            l.path,
                            facing_words(k, n),
                            step.strike
                        ),
                    ));
                }
                if !c.uncaught.is_empty() {
                    out.push(Failure::new(
                        DW_ASSEMBLY_STRIKE,
                        format!(
                            "assembly `{}`'s strike step {}: the strike clip `{}` comes down on {} \
                             standable cell(s){} its blow ({}) does not land on — its last frame \
                             meets a body standing there within {FLOOR_BAND} block of the floor, \
                             its first frame did not, and the body is not caught by the landing \
                             box even by the box's edge (its keep-out, one cell round it for a \
                             player): {}. The \
                             blow's area must be the area the limb comes down on; a long limb's \
                             blow is a long area along where it lands. Widen the landing box to \
                             cover them (`delvec rig describe` prints the footprint), or choose a \
                             strike clip that comes down only where the blow lands",
                            s.id,
                            step.index,
                            step.strike,
                            c.uncaught.len(),
                            facing_words(k, n),
                            l.path,
                            cells_named(&c.uncaught)
                        ),
                    ));
                }
            }
        }
        record.caught.sort();
        record.caught.dedup();
        for fr in &mut per_facing {
            fr.caught.sort();
            fr.caught.dedup();
            fr.comes_down.sort();
            fr.comes_down.dedup();
        }
        record.facings = per_facing;
        j.records.push(record);
    }
    (j, out)
}

/// A clip's run at a pace: ticks from the switch to its last frame applied,
/// and to that frame drawn whole — [`rig::Clip::length_ticks`] and
/// [`rig::Clip::landing_ticks`] at `tpf` ticks per frame, or the clip's own.
pub fn paced(clip: &rig::Clip, tpf: Option<u32>) -> (u32, u32) {
    let t = tpf.unwrap_or(clip.ticks_per_frame);
    let length = 1 + (clip.frames.len().saturating_sub(1) as u32) * t;
    (length, length + t)
}

/// ", at facing k of n (turned D degrees)" for an aimed pattern; nothing for
/// one facing.
fn facing_words(k: u32, n: u32) -> String {
    if n <= 1 {
        String::new()
    } else {
        format!(
            " at facing {k} of {n} (turned {} degrees from the declared facing, root yaw {})",
            yaw_token(360.0 * f64::from(k) / f64::from(n)),
            yaw_token(root_yaw(k, n))
        )
    }
}

/// Cells as a refusal names them, at most [`NAME_LIMIT`] and a count.
fn cells_named(cells: &[[i32; 3]]) -> String {
    let shown = cells
        .iter()
        .take(NAME_LIMIT)
        .map(|c| format!("[{}, {}, {}]", c[0], c[1], c[2]))
        .collect::<Vec<_>>()
        .join(" ");
    if cells.len() > NAME_LIMIT {
        format!("{shown}, and {} more", cells.len() - NAME_LIMIT)
    } else {
        shown
    }
}

/// Where a body stands to take facing `k`'s blow: a landing cell under the
/// limb, inside the arming region itself, from which the run-time choice draws
/// `k` — the one whose bearing is nearest the facing's own, then in cell
/// order.
fn stand_cell(
    mark: [i32; 3],
    arming: CellBox,
    c: &Correspondence,
    k: u32,
    n: u32,
    base: f64,
) -> Option<[i32; 3]> {
    let struck: BTreeSet<[i32; 3]> = c.struck.iter().copied().collect();
    let at = base + facing_turn(k, n);
    c.landing
        .iter()
        .copied()
        .filter(|x| struck.contains(x) && in_box(*x, arming) && facing_for(mark, *x, n, base) == k)
        .min_by(|a, b| {
            let off = |x: [i32; 3]| {
                wrap(bearing(f64::from(x[0] - mark[0]), f64::from(x[2] - mark[2])) - at).abs()
            };
            off(*a).total_cmp(&off(*b)).then(a.cmp(b))
        })
}

/// Every cell of an inclusive box.
fn cells_of((lo, hi): ([i32; 3], [i32; 3])) -> Vec<[i32; 3]> {
    let mut out = Vec::new();
    for x in lo[0]..=hi[0] {
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                out.push([x, y, z]);
            }
        }
    }
    out
}

/// Every `damage-players` inside a list, at any depth, with its pointer.
fn landings(plan: &Plan, path: &str, effs: &[QuestEffect], nested: bool, out: &mut Vec<Landing>) {
    for (i, e) in effs.iter().enumerate() {
        let here = format!("{path}/{i}");
        if let Verb::DamagePlayers { amount, .. } = &e.verb {
            // spec-0085: `in` is the effect envelope's field, read through
            // the one accessor every damage reader takes.
            let within = e.damage_within();
            out.push(Landing {
                path: here.clone(),
                within: within.and_then(|z: &StealthZone| plan.zone_box(z)),
                declares_in: within.is_some(),
                amount: *amount,
                nested,
            });
        }
        for (seg, _, list) in e.nested_effect_lists_labeled() {
            landings(plan, &format!("{here}/{seg}"), list, true, out);
        }
    }
}

/// Build one placed assembly's [`Subject`].
pub fn subject<'a>(plan: &'a Plan<'a>, p: &Placed<'a>) -> Subject<'a> {
    let path = format!("/content/assemblies/{}", p.index);
    let strikes = p.decl.strikes.as_ref().and_then(|st| {
        let arming = plan.zone_box(&st.while_in)?;
        let steps = st
            .pattern
            .iter()
            .enumerate()
            .map(|(i, step)| {
                let mut ls = Vec::new();
                landings(
                    plan,
                    &format!("{path}/strikes/pattern/{i}/on_land"),
                    &step.on_land,
                    false,
                    &mut ls,
                );
                StepSubject {
                    index: i,
                    windup: step.windup.as_str(),
                    hold: step.hold,
                    strike: step.strike.as_str(),
                    ticks_per_frame: step.ticks_per_frame,
                    landings: ls,
                }
            })
            .collect();
        Some((arming, steps))
    });
    let struck_by: Vec<&str> = plan
        .campaign
        .quests
        .content
        .triggers
        .iter()
        .filter(
            |t| matches!(&t.on, TriggerOn::StrikeAssembly { assembly } if assembly == &p.decl.id),
        )
        .map(|t| t.id.as_str())
        .collect();
    let performed: Vec<&str> = struck_by
        .iter()
        .copied()
        .filter(|t| {
            plan.critical_path
                .iter()
                .any(|s| matches!(s, Step::Trigger { trigger_id, .. } if trigger_id == t))
        })
        .collect();
    Subject {
        id: p.decl.id.as_str(),
        path,
        mark: p.mark,
        facing: p.decl.facing(),
        rig: p.rig,
        initial: p.decl.initial.as_deref(),
        hitbox: p.decl.hitbox.as_ref(),
        strikes,
        struck_by,
        performed,
        aim: p
            .decl
            .strikes
            .as_ref()
            .and_then(|st| st.aim.as_ref())
            .map(|a| a.facings.get()),
    }
}

// ---------------------------------------------------------------------------
// The binding line
// ---------------------------------------------------------------------------

/// What the assembly rules examined on one build (spec-0082 §5.6), printed on
/// every build, zeroes included.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AssemblyBinding {
    /// Assemblies declared.
    pub declared: usize,
    /// Parts across the placed assemblies.
    pub parts: usize,
    /// Clips across their rigs.
    pub clips: usize,
    /// Hitboxes examined.
    pub hitboxes: usize,
    /// Strike steps checked.
    pub steps: usize,
    /// Facings judged across those steps (one per landing per facing a blow
    /// can take).
    pub facings: usize,
    /// Refusals.
    pub refused: usize,
    /// The keyframe writes per tick the declared cadences add up to, at worst:
    /// each assembly's part count over its fastest clip's cadence.
    pub writes_per_tick: f64,
    /// Per-step records.
    pub records: Vec<StepRecord>,
    /// Per assembly with a strike pattern: a standable cell no body standing
    /// in can be selected by the arming region — where the bot stands to see
    /// that no blow is wound up for it. `None` when the walked population has
    /// no such cell.
    pub spared: Vec<(String, Option<[i32; 3]>)>,
    /// The assemblies whose blow the critical path's bot witnesses
    /// ([`with_witness_steps`]); filled when the path is written.
    pub witnessed: Vec<String>,
}

impl AssemblyBinding {
    /// The binding line.
    pub fn line(&self) -> String {
        format!(
            "assembly binding: {} assembl(ies) declared, {} part(s), {} clip(s), {} hitbox(es) \
             examined, {} strike step(s) checked over {} facing(s), {} refused",
            self.declared,
            self.parts,
            self.clips,
            self.hitboxes,
            self.steps,
            self.facings,
            self.refused
        )
    }

    /// The cost the host meets (spec-0082 §5.6): the engine states the number,
    /// the host is never a cap on the capability.
    pub fn cost_line(&self) -> String {
        format!(
            "assembly cost: {} display part(s) in all; at most {:.1} keyframe write(s) per tick at \
             the declared cadences (measured on the pinned server, spec-0082 §8 row 7: about 0.3 \
             ms per 34-part assembly per tick at a 5-tick cadence, 0.9 ms at every tick)",
            self.parts, self.writes_per_tick
        )
    }

    /// The staging record, `validation/assembly.json`.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "codes": ["DW0936", "DW0937", "DW0938"],
            "declared": self.declared,
            "parts": self.parts,
            "clips": self.clips,
            "hitboxes": self.hitboxes,
            "steps": self.steps,
            "facings": self.facings,
            "refused": self.refused,
            "writes_per_tick": self.writes_per_tick,
            "strike_steps": self.records.iter().map(|r| serde_json::json!({
                "assembly": r.assembly,
                "step": r.step,
                "windup_ticks": r.windup_ticks,
                "hold": r.hold,
                "strike_ticks": r.strike_ticks,
                "amounts": r.amounts,
                "caught": r.caught,
                "facing_count": r.facing_count,
                "facings": r.facings.iter().map(|f| serde_json::json!({
                    "k": f.k,
                    "yaw": f.yaw,
                    "caught": f.caught,
                    "comes_down": f.comes_down,
                    "stand": f.stand,
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
            "spared": self.spared.iter().map(|(a, c)| serde_json::json!({
                "assembly": a,
                "stand": c,
            })).collect::<Vec<_>>(),
            "witnessed": self.witnessed,
        })
    }
}

/// **Judge every assembly in the build** and return what was examined beside
/// the refusals. `population` is the walked population `P`; `reaches` is as
/// for [`judge`].
pub fn check(
    plan: &Plan<'_>,
    population: &BTreeSet<[i32; 3]>,
    reaches: &Reaches<'_>,
) -> (AssemblyBinding, Vec<Failure>) {
    let within = |c: [i32; 3]| population.contains(&c);
    let mut b = AssemblyBinding {
        declared: plan.campaign.quests.content.assemblies.len(),
        ..AssemblyBinding::default()
    };
    let mut failures = Vec::new();
    for p in placed(plan) {
        b.parts += p.rig.parts.len();
        b.clips += p.rig.clips.len();
        let fastest = p
            .rig
            .clips
            .values()
            .map(|c| c.ticks_per_frame.max(1))
            .min()
            .unwrap_or(1);
        b.writes_per_tick += p.rig.parts.len() as f64 / f64::from(fastest);
        let s = subject(plan, &p);
        let (j, f) = judge(&s, &within, reaches);
        b.hitboxes += j.hitboxes;
        b.steps += j.steps;
        b.facings += j.facings;
        b.records.extend(j.records);
        if let Some((arming, _)) = &s.strikes {
            b.spared
                .push((s.id.to_string(), spared_cell(population, *arming)));
        }
        failures.extend(f);
    }
    b.refused = failures.len();
    (b, failures)
}

/// A standable cell outside the arming region's keep-out — no body standing
/// in it can be selected — as near the region as the population has one (by
/// the larger of the x and z gaps), then in cell order.
fn spared_cell(population: &BTreeSet<[i32; 3]>, arming: CellBox) -> Option<[i32; 3]> {
    let keep = keep_out_box(Body::PLAYER, arming.0, arming.1);
    let gap = |c: [i32; 3]| {
        let d = |i: usize| (arming.0[i] - c[i]).max(c[i] - arming.1[i]).max(0);
        d(0).max(d(2))
    };
    population
        .iter()
        .copied()
        .filter(|c| !in_box(*c, keep))
        .min_by(|a, b| gap(*a).cmp(&gap(*b)).then(a.cmp(b)))
}

// ---------------------------------------------------------------------------
// The bot's witness of a blow
// ---------------------------------------------------------------------------

/// How long past the pattern's cycles a witness waits, in ticks: two seconds
/// of slack for a server that runs a tick late.
pub const WITNESS_SLACK_TICKS: u32 = 40;

/// **The critical-path steps that witness each strike pattern** (spec-0082
/// §5.4, §5.7): per assembly with a pattern, the bot stands on facing 0's
/// stand cell (a landing cell under the limb from which a body draws that
/// facing) until a blow takes health from it, then on a cell no body in which
/// the arming region can select, for as long, and is not struck. The window is
/// one cycle of every step plus the longest step again — a bot that arrives
/// while a blow aimed elsewhere is in flight still sees the next whole cycle —
/// plus [`WITNESS_SLACK_TICKS`]. An assembly without a stand cell or a spared
/// cell gets no witness, and the record says so (`witnessed`).
pub fn witness_steps(b: &AssemblyBinding) -> Vec<(String, Vec<serde_json::Value>)> {
    let mut out: Vec<(String, Vec<serde_json::Value>)> = Vec::new();
    for (id, spared) in &b.spared {
        let steps: Vec<&StepRecord> = b.records.iter().filter(|r| &r.assembly == id).collect();
        let cycle = |r: &StepRecord| r.windup_ticks + r.hold + r.strike_ticks;
        let window = steps.iter().map(|r| cycle(r)).sum::<u32>()
            + steps.iter().map(|r| cycle(r)).max().unwrap_or(0)
            + WITNESS_SLACK_TICKS;
        let Some(first) = steps.first() else { continue };
        let Some(face) = first.facings.iter().find(|f| f.stand.is_some()) else {
            continue;
        };
        let (Some(stand), Some(spared)) = (face.stand, spared) else {
            continue;
        };
        out.push((
            id.clone(),
            vec![
                serde_json::json!({
                    "action": "witness-strike",
                    "assembly": id,
                    "expect": "struck",
                    "pos": stand,
                    "step": first.step,
                    "facing": face.k,
                    "facing_count": first.facing_count,
                    "yaw": face.yaw,
                    "amount": first.amounts.first().copied().unwrap_or(0),
                    "window_ticks": window,
                }),
                serde_json::json!({
                    "action": "witness-strike",
                    "assembly": id,
                    "expect": "spared",
                    "pos": spared,
                    "window_ticks": window,
                }),
            ],
        ));
    }
    out
}

/// Put each assembly's witness steps on the critical path, just before the
/// first step that strikes that assembly — the last moment the thing is
/// certainly standing. Returns the assemblies witnessed.
pub fn with_witness_steps(
    steps: &mut Vec<serde_json::Value>,
    witnesses: Vec<(String, Vec<serde_json::Value>)>,
) -> Vec<String> {
    let mut placed = Vec::new();
    for (id, ws) in witnesses {
        let Some(at) = steps
            .iter()
            .position(|s| s["action"] == "trigger" && s["assembly"] == id.as_str())
        else {
            continue;
        };
        for (k, w) in ws.into_iter().enumerate() {
            steps.insert(at + k, w);
        }
        placed.push(id);
    }
    placed
}

// ---------------------------------------------------------------------------
// Emission
// ---------------------------------------------------------------------------

/// The tag every entity of an assembly carries.
pub fn tag(safe: &str) -> String {
    format!("dw_asm_{safe}")
}

/// The hitbox's tag: what a `strike-assembly` polls and clears.
pub fn hit_tag(safe: &str) -> String {
    format!("dw_asm_{safe}_hit")
}

/// The root's tag.
pub fn root_tag(safe: &str) -> String {
    format!("dw_asm_{safe}_root")
}

/// A part's tag.
pub fn part_tag(safe: &str, i: usize) -> String {
    format!("dw_asm_{safe}_p{i}")
}

/// A `dw.sys` holder of the assembly's state.
pub fn holder(safe: &str, what: &str) -> String {
    format!("#asm_{safe}_{what}")
}

/// The function a `spawn-assembly` calls.
pub fn spawn_fn(safe: &str) -> String {
    format!("asm_spawn_{safe}")
}

/// The function a `despawn-assembly` calls.
pub fn despawn_fn(safe: &str) -> String {
    format!("asm_despawn_{safe}")
}

/// The function a `play-clip` calls.
pub fn cue_fn(safe: &str, clip: usize) -> String {
    format!("asm_cue_{safe}_{clip}")
}

/// The per-tick driver.
pub fn tick_fn(safe: &str) -> String {
    format!("asm_tick_{safe}")
}

/// The landing function.
pub fn land_fn(safe: &str) -> String {
    format!("asm_land_{safe}")
}

/// A float as SNBT `f`, four decimals, `-0` folded to `0`.
fn f4(v: f64) -> String {
    let s = format!("{v:.4}");
    let s = if s == "-0.0000" {
        "0.0000".to_string()
    } else {
        s
    };
    format!("{s}f")
}

/// A transform as `transformation` SNBT.
pub fn transformation(t: &Transform) -> String {
    let j = |v: &[f64]| v.iter().map(|x| f4(*x)).collect::<Vec<_>>().join(",");
    format!(
        "{{translation:[{}],left_rotation:[{}],scale:[{}],right_rotation:[{}]}}",
        j(&t.translation),
        j(&t.left_rotation),
        j(&t.scale),
        j(&t.right_rotation)
    )
}

/// An entity position: the cell's centre on its floor.
fn pos(c: [i32; 3]) -> String {
    format!(
        "{} {} {}",
        f64::from(c[0]) + 0.5,
        f64::from(c[1]),
        f64::from(c[2]) + 0.5
    )
}

/// The tick line that drives one live assembly.
pub fn tick_lines(plan: &Plan<'_>) -> Vec<String> {
    let ns = &plan.namespace;
    placed(plan)
        .iter()
        .map(|p| {
            format!(
                "execute if score {} dw.sys matches 1 run function {ns}:{}",
                holder(&p.safe, "live"),
                tick_fn(&p.safe)
            )
        })
        .collect()
}

/// The commands a verb naming an assembly lowers to, or `None` for any other
/// verb. Empty for an assembly the build does not place.
pub fn verb_lines(plan: &Plan<'_>, verb: &Verb) -> Option<Vec<String>> {
    let ns = &plan.namespace;
    let (id, clip) = match verb {
        Verb::SpawnAssembly { assembly } | Verb::DespawnAssembly { assembly } => {
            (assembly.as_str(), None)
        }
        Verb::PlayClip { assembly, clip } => (assembly.as_str(), Some(clip.as_str())),
        _ => return None,
    };
    let Some(p) = placed(plan).into_iter().find(|p| p.decl.id.as_str() == id) else {
        return Some(Vec::new());
    };
    Some(match (verb, clip) {
        (Verb::SpawnAssembly { .. }, _) => vec![format!("function {ns}:{}", spawn_fn(&p.safe))],
        (Verb::DespawnAssembly { .. }, _) => {
            vec![format!("function {ns}:{}", despawn_fn(&p.safe))]
        }
        (_, Some(c)) => match p.rig.clip_index(c) {
            Some(k) => vec![format!("function {ns}:{}", cue_fn(&p.safe, k))],
            None => Vec::new(),
        },
        _ => Vec::new(),
    })
}

/// The function an aimed wind-up calls to turn the root to facing `k`.
pub fn aim_fn(safe: &str, k: u32) -> String {
    format!("asm_aim_{safe}_{k}")
}

/// The tag a turned landing marks the players it catches with, for one blow.
pub fn struck_tag(safe: &str) -> String {
    format!("dw_asm_{safe}_struck")
}

/// **A turned landing region's blow** (spec-0082 §5.7): every player whose
/// body meets the region is tagged once — the region as runs of cells along
/// `x`, each one selector volume — then each tagged player is dealt the blow
/// once and the tag is cleared.
pub fn region_damage_lines(
    safe: &str,
    region: &BTreeSet<[i32; 3]>,
    amount: u32,
    kind: &str,
) -> Vec<String> {
    let t = struck_tag(safe);
    let Some(y0) = region.iter().map(|c| c[1]).min() else {
        return Vec::new();
    };
    let y1 = region.iter().map(|c| c[1]).max().unwrap_or(y0);
    // The region's columns, by z then x.
    let cols: BTreeSet<(i32, i32)> = region.iter().map(|c| (c[2], c[0])).collect();
    let mut runs: Vec<(i32, i32, i32)> = Vec::new();
    for (z, x) in cols {
        match runs.last_mut() {
            Some((rz, _, x1)) if *rz == z && *x1 + 1 == x => *x1 = x,
            _ => runs.push((z, x, x)),
        }
    }
    let mut out: Vec<String> = runs
        .iter()
        .map(|(z, x0, x1)| {
            format!(
                "tag @a[{},tag=!{}] add {t}",
                crate::compiler::emit::box_selector_args([*x0, y0, *z], [*x1, y1, *z]),
                crate::compiler::emit::CUTSCENE_TAG
            )
        })
        .collect();
    out.push(format!(
        "execute as @a[tag={t}] run damage @s {amount} {kind}"
    ));
    out.push(format!("tag @a[tag={t}] remove {t}"));
    out
}

/// Wraps lines lowered for an effect in that effect's own `when`.
pub type Guard<'g> = dyn Fn(&QuestEffect, Vec<String>, &mut Vec<String>) + 'g;

/// Every function the build's assemblies need. `land` lowers one effect of an
/// `on_land` bundle (the ordinary effect emitter, with no acting player);
/// `guard` wraps lines this module wrote for an effect in that effect's own
/// `when`, as `land` would have.
pub fn assembly_functions(
    plan: &Plan<'_>,
    land: &dyn Fn(&QuestEffect, &mut Vec<String>),
    guard: &Guard<'_>,
) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut out: Vec<(String, String)> = Vec::new();
    let join = |v: Vec<String>| {
        let mut s = v.join("\n");
        s.push('\n');
        s
    };
    for p in placed(plan) {
        let s = &p.safe;
        let facing = p.decl.facing();
        let h = |w: &str| holder(s, w);
        let clips: Vec<(&String, &rig::Clip)> = p.rig.clips.iter().collect();
        // Every clip as it is played: the rig's own at their cadence, then one
        // per strike step that sets its own pace, for its wind-up and its
        // strike (`paced_index`).
        let mut plays: Vec<(&rig::Clip, u32)> =
            clips.iter().map(|(_, c)| (*c, c.ticks_per_frame)).collect();
        if let Some(st) = &p.decl.strikes {
            for step in &st.pattern {
                if let Some(tpf) = step.ticks_per_frame {
                    for name in [&step.windup, &step.strike] {
                        if let Some(c) = p.rig.clips.get(name) {
                            plays.push((c, tpf));
                        }
                    }
                }
            }
        }
        let initial = p.decl.initial.as_deref().and_then(|c| p.rig.clip_index(c));
        let spawn_tpf = initial
            .and_then(|k| clips.get(k))
            .map(|(_, c)| c.ticks_per_frame)
            .unwrap_or(1);

        // ---- spawn / summon / despawn ----
        out.push((
            spawn_fn(s),
            join(vec![format!(
                "execute unless entity @e[tag={}] run function {ns}:asm_summon_{s}",
                root_tag(s)
            )]),
        ));
        let fixture = crate::compiler::affordance::FIXTURE_TAG;
        let mut summon = vec![format!("kill @e[tag={}]", tag(s))];
        summon.push(format!(
            "summon minecraft:item_display {} {{Tags:[\"{fixture}\",\"{}\",\"{}\"]}}",
            pos(p.mark),
            tag(s),
            root_tag(s)
        ));
        for (i, (part, t)) in p.rig.parts.iter().zip(p.spawn_pose()).enumerate() {
            summon.push(format!(
                "summon minecraft:block_display {} {{Tags:[\"{fixture}\",\"{}\",\"{}_part\",\"{}\"],block_state:{},interpolation_duration:{spawn_tpf},transformation:{}}}",
                pos(p.mark),
                tag(s),
                tag(s),
                part_tag(s, i),
                part.block_state_snbt(),
                transformation(&t.faced(facing)),
            ));
            summon.push(format!(
                "ride @e[tag={},limit=1] mount @e[tag={},limit=1]",
                part_tag(s, i),
                root_tag(s)
            ));
        }
        if let Some(hb) = &p.decl.hitbox {
            summon.push(format!(
                "summon minecraft:interaction {} {{width:{}f,height:{}f,response:1b,Tags:[\"{fixture}\",\"{}\",\"{}\"]}}",
                pos(p.hitbox_cell()),
                hb.width,
                hb.height,
                tag(s),
                hit_tag(s)
            ));
        }
        summon.push(format!("scoreboard players set {} dw.sys 1", h("live")));
        summon.push(format!("scoreboard players set {} dw.sys 0", h("sm")));
        summon.push(format!("scoreboard players set {} dw.sys 0", h("step")));
        summon.push(format!("scoreboard players set {} dw.sys 0", h("aim")));
        summon.push(format!(
            "scoreboard players set {} dw.sys {}",
            h("base"),
            initial.map(|k| k as i64).unwrap_or(-1)
        ));
        summon.push(format!("function {ns}:asm_resume_{s}"));
        out.push((format!("asm_summon_{s}"), join(summon)));
        out.push((
            despawn_fn(s),
            join(vec![
                format!("kill @e[tag={}]", tag(s)),
                format!("scoreboard players set {} dw.sys 0", h("live")),
                format!("scoreboard players set {} dw.sys 0", h("sm")),
            ]),
        ));

        // ---- clip playback ----
        for (k, (c, tpf)) in plays.iter().enumerate() {
            out.push((
                format!("asm_play_{s}_{k}"),
                join(vec![
                    format!("scoreboard players set {} dw.sys {k}", h("clip")),
                    format!("scoreboard players set {} dw.sys -1", h("f")),
                    format!("scoreboard players set {} dw.sys {}", h("t"), tpf - 1),
                    format!("scoreboard players set {} dw.sys {tpf}", h("tpf")),
                    format!("scoreboard players set {} dw.sys 0", h("done")),
                ]),
            ));
            for (f, frame) in c.frames.iter().enumerate() {
                let body: Vec<String> = frame
                    .iter()
                    .enumerate()
                    .map(|(i, t)| {
                        format!(
                            "execute as @e[tag={},limit=1] run data merge entity @s {{start_interpolation:0,interpolation_duration:{tpf},transformation:{}}}",
                            part_tag(s, i),
                            transformation(&t.faced(facing))
                        )
                    })
                    .collect();
                out.push((format!("asm_frame_{s}_{k}_{f}"), join(body)));
            }
        }
        for (k, _) in clips.iter().enumerate() {
            out.push((
                cue_fn(s, k),
                join(vec![
                    format!("scoreboard players set {} dw.sys {k}", h("base")),
                    format!(
                        "execute if score {} dw.sys matches 1 if score {} dw.sys matches 0 run function {ns}:asm_play_{s}_{k}",
                        h("live"),
                        h("sm")
                    ),
                ]),
            ));
        }
        // The rest pose, for an assembly that returns to no clip.
        let rest: Vec<String> = p
            .rig
            .rest_pose()
            .unwrap_or_default()
            .iter()
            .enumerate()
            .map(|(i, t)| {
                format!(
                    "execute as @e[tag={},limit=1] run data merge entity @s {{start_interpolation:0,interpolation_duration:{},transformation:{}}}",
                    part_tag(s, i),
                    REST_INTERPOLATION,
                    transformation(&t.faced(facing))
                )
            })
            .chain([
                format!("scoreboard players set {} dw.sys -1", h("clip")),
                format!("scoreboard players set {} dw.sys 1", h("done")),
            ])
            .collect();
        out.push((format!("asm_rest_{s}"), join(rest)));
        let mut resume: Vec<String> = (0..clips.len())
            .map(|k| {
                format!(
                    "execute if score {} dw.sys matches {k} run function {ns}:asm_play_{s}_{k}",
                    h("base")
                )
            })
            .collect();
        resume.push(format!(
            "execute if score {} dw.sys matches ..-1 run function {ns}:asm_rest_{s}",
            h("base")
        ));
        out.push((format!("asm_resume_{s}"), join(resume)));

        // ---- the frame driver ----
        let mut adv = vec![
            format!("scoreboard players set {} dw.sys 0", h("t")),
            format!("scoreboard players add {} dw.sys 1", h("f")),
        ];
        for (k, (c, _)) in plays.iter().enumerate() {
            let n = c.frames.len();
            if c.looping {
                adv.push(format!(
                    "execute if score {} dw.sys matches {k} if score {} dw.sys matches {n}.. run scoreboard players set {} dw.sys 0",
                    h("clip"),
                    h("f"),
                    h("f")
                ));
            } else {
                adv.push(format!(
                    "execute if score {} dw.sys matches {k} if score {} dw.sys matches {}.. run scoreboard players set {} dw.sys 1",
                    h("clip"),
                    h("f"),
                    n - 1,
                    h("done")
                ));
            }
        }
        adv.push(format!(
            "execute store result storage {STORAGE} {s}.c int 1 run scoreboard players get {} dw.sys",
            h("clip")
        ));
        adv.push(format!(
            "execute store result storage {STORAGE} {s}.f int 1 run scoreboard players get {} dw.sys",
            h("f")
        ));
        adv.push(format!(
            "function {ns}:asm_apply_{s} with storage {STORAGE} {s}"
        ));
        out.push((format!("asm_adv_{s}"), join(adv)));
        out.push((
            format!("asm_apply_{s}"),
            join(vec![format!("$function {ns}:asm_frame_{s}_$(c)_$(f)")]),
        ));

        // ---- the strike machine ----
        let mut tick = vec![
            format!("scoreboard players add {} dw.sys 1", h("t")),
            format!(
                "execute if score {} dw.sys matches 0.. unless score {} dw.sys matches 1 if score {} dw.sys >= {} dw.sys run function {ns}:asm_adv_{s}",
                h("clip"),
                h("done"),
                h("t"),
                h("tpf")
            ),
        ];
        if let Some(st) = &p.decl.strikes
            && !st.pattern.is_empty()
            && let Some(arming) = plan.zone_box(&st.while_in)
        {
            let sel = format!(
                "@a[{},tag=!{}]",
                crate::compiler::emit::box_selector_args(arming.0, arming.1),
                crate::compiler::emit::CUTSCENE_TAG
            );
            let idx = |c: &str| p.rig.clip_index(c);
            tick.push(format!(
                "execute if score {} dw.sys matches 0 unless entity {sel} run scoreboard players set {} dw.sys 0",
                h("sm"),
                h("step")
            ));
            tick.push(format!(
                "execute if score {} dw.sys matches 0 if entity {sel} run function {ns}:asm_begin_{s}",
                h("sm")
            ));
            tick.push(format!(
                "execute if score {} dw.sys matches 2 run scoreboard players add {} dw.sys 1",
                h("sm"),
                h("hold")
            ));
            tick.push(format!(
                "execute if score {} dw.sys matches 2 if score {} dw.sys >= {} dw.sys run function {ns}:asm_swing_{s}",
                h("sm"),
                h("hold"),
                h("holdn")
            ));
            tick.push(format!(
                "execute if score {} dw.sys matches 1 if score {} dw.sys matches 1 run function {ns}:asm_hold_{s}",
                h("sm"),
                h("done")
            ));
            // The blow lands once the client has drawn the strike's last frame
            // whole — one cadence after it is applied — so the limb the player
            // sees is the pose the strike check judged (spec-0082 §5.4).
            tick.push(format!(
                "execute if score {} dw.sys matches 3 if score {} dw.sys matches 1 if score {} dw.sys >= {} dw.sys run function {ns}:{}",
                h("sm"),
                h("done"),
                h("t"),
                h("tpf"),
                land_fn(s)
            ));
            let mut begin = vec![format!("scoreboard players set {} dw.sys 1", h("sm"))];
            // An aimed pattern turns to its target before the wind-up plays.
            let n = st.aim.as_ref().map(|a| a.facings.get()).unwrap_or(1);
            let drawn: Vec<u32> = if st.aim.is_some() {
                drawn_facings(p.mark, arming, n, rig::facing_angle(facing))
            } else {
                vec![0]
            };
            if st.aim.is_some() {
                let root = format!("@e[tag={},limit=1]", root_tag(s));
                let target = format!(
                    "@a[{},tag=!{},sort=nearest,limit=1]",
                    crate::compiler::emit::box_selector_args(arming.0, arming.1),
                    crate::compiler::emit::CUTSCENE_TAG
                );
                let base_deg = rig::facing_angle(facing).to_degrees().round() as i64;
                let n64 = i64::from(n);
                begin.push(format!(
                    "execute as {root} at @s facing entity {target} feet run tp @s ~ ~ ~ ~ 0"
                ));
                begin.push(format!(
                    "execute store result score {} dw.sys run data get entity {root} Rotation[0] {n}",
                    h("yaw")
                ));
                // k = floor((-yaw*n - base*n + 180) / 360) mod n: the facing
                // nearest the bearing, with the scoreboard's floor division and
                // non-negative remainder (both measured on the pinned server).
                for (op, c) in [
                    ("*=", -1),
                    ("+=", 180 - base_deg * n64),
                    ("/=", 360),
                    ("%=", n64),
                ] {
                    begin.push(format!("scoreboard players set {} dw.sys {c}", h("c")));
                    begin.push(format!(
                        "scoreboard players operation {} dw.sys {op} {} dw.sys",
                        h("yaw"),
                        h("c")
                    ));
                }
                for k in 0..n {
                    let to = nearest_drawn(&drawn, k, n);
                    begin.push(format!(
                        "execute if score {} dw.sys matches {k} run function {ns}:{}",
                        h("yaw"),
                        aim_fn(s, to)
                    ));
                }
                for &k in &drawn {
                    out.push((
                        aim_fn(s, k),
                        join(vec![
                            format!("tp {root} {} {} 0", pos(p.mark), yaw_token(root_yaw(k, n))),
                            format!("scoreboard players set {} dw.sys {k}", h("aim")),
                        ]),
                    ));
                }
            }
            let mut hold = vec![
                format!("scoreboard players set {} dw.sys 2", h("sm")),
                format!("scoreboard players set {} dw.sys 0", h("hold")),
            ];
            let mut swing = vec![format!("scoreboard players set {} dw.sys 3", h("sm"))];
            let mut landing = Vec::new();
            for (j, step) in st.pattern.iter().enumerate() {
                let paced = paced_index(p.rig, &st.pattern, j);
                if let Some(k) = paced.map(|(w, _)| w).or_else(|| idx(&step.windup)) {
                    begin.push(format!(
                        "execute if score {} dw.sys matches {j} run function {ns}:asm_play_{s}_{k}",
                        h("step")
                    ));
                }
                hold.push(format!(
                    "execute if score {} dw.sys matches {j} run scoreboard players set {} dw.sys {}",
                    h("step"),
                    h("holdn"),
                    step.hold
                ));
                if let Some(k) = paced.map(|(_, m)| m).or_else(|| idx(&step.strike)) {
                    swing.push(format!(
                        "execute if score {} dw.sys matches {j} run function {ns}:asm_play_{s}_{k}",
                        h("step")
                    ));
                }
                if !step.on_land.is_empty() && st.aim.is_none() {
                    landing.push(format!(
                        "execute if score {} dw.sys matches {j} run function {ns}:asm_land_{s}_{j}",
                        h("step")
                    ));
                    let mut body = Vec::new();
                    for e in &step.on_land {
                        land(e, &mut body);
                    }
                    out.push((format!("asm_land_{s}_{j}"), join(body)));
                }
                if !step.on_land.is_empty() && st.aim.is_some() {
                    for &k in &drawn {
                        landing.push(format!(
                            "execute if score {} dw.sys matches {j} if score {} dw.sys matches {k} run function {ns}:asm_land_{s}_{j}_{k}",
                            h("step"),
                            h("aim")
                        ));
                        let mut body = Vec::new();
                        for e in &step.on_land {
                            // spec-0085: `in` is the effect envelope's field,
                            // read through the one accessor every damage
                            // reader takes.
                            match (&e.verb, e.damage_within()) {
                                (
                                    Verb::DamagePlayers {
                                        amount,
                                        damage_type,
                                        ..
                                    },
                                    Some(z),
                                ) => {
                                    let Some(bx) = plan.zone_box(z) else {
                                        continue;
                                    };
                                    let region = turned_region(p.mark, bx, facing_turn(k, n));
                                    let kind = damage_type
                                        .unwrap_or(delvewright_dsl::DamageKind::Generic)
                                        .id();
                                    guard(
                                        e,
                                        region_damage_lines(s, &region, *amount, kind),
                                        &mut body,
                                    );
                                }
                                _ => land(e, &mut body),
                            }
                        }
                        out.push((format!("asm_land_{s}_{j}_{k}"), join(body)));
                    }
                }
            }
            landing.push(format!("scoreboard players add {} dw.sys 1", h("lands")));
            landing.push(format!("scoreboard players add {} dw.sys 1", h("step")));
            landing.push(format!(
                "execute if score {} dw.sys matches {}.. run scoreboard players set {} dw.sys 0",
                h("step"),
                st.pattern.len(),
                h("step")
            ));
            landing.push(format!("scoreboard players set {} dw.sys 0", h("sm")));
            landing.push(format!("function {ns}:asm_resume_{s}"));
            out.push((format!("asm_begin_{s}"), join(begin)));
            out.push((format!("asm_hold_{s}"), join(hold)));
            out.push((format!("asm_swing_{s}"), join(swing)));
            out.push((land_fn(s), join(landing)));
        }
        out.push((tick_fn(s), join(tick)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use delvewright_dsl::rig::{Clip, PartKind, RigPart, RigProvenance};
    use std::collections::BTreeMap;

    fn block(t: [f64; 3], scale: [f64; 3]) -> Transform {
        Transform {
            translation: t,
            left_rotation: [0.0, 0.0, 0.0, 1.0],
            scale,
            right_rotation: [0.0, 0.0, 0.0, 1.0],
        }
    }

    fn cube(t: [f64; 3]) -> Transform {
        block(t, [1.0, 1.0, 1.0])
    }

    /// The slab a strike lays: three cells wide on x, five long on z, from
    /// one cell in front of the mark, `lift` above the floor.
    fn slab(lift: f64) -> Transform {
        block([-1.5, lift, 0.5], [3.0, 1.0, 5.0])
    }

    /// A one-part rig: `idle` stands the cube on the mark cell, `windup` lifts
    /// it a cell, `strike` brings it from there down into a slab three wide
    /// and five long lying in front of the mark (+z) — world cells x 9..=11,
    /// z 11..=15 on the floor course `y = 1` for a mark at [10, 1, 10].
    fn rig_with(landing: Transform) -> Rig {
        let mut clips = BTreeMap::new();
        let clip = |frames: Vec<Transform>, looping| Clip {
            ticks_per_frame: 5,
            looping,
            frames: frames.into_iter().map(|t| vec![t]).collect(),
        };
        clips.insert(
            "idle".to_string(),
            clip(vec![cube([-0.5, 0.0, -0.5])], true),
        );
        clips.insert(
            "windup".to_string(),
            clip(vec![cube([-0.5, 1.0, -0.5])], false),
        );
        clips.insert(
            "strike".to_string(),
            clip(vec![cube([-0.5, 1.0, -0.5]), landing], false),
        );
        Rig {
            rig_version: 1,
            parts: vec![RigPart {
                id: "a".into(),
                kind: PartKind::Block,
                block: "minecraft:stone".into(),
                rest: None,
            }],
            clips,
            provenance: RigProvenance {
                generator: "test".into(),
                source: "original".into(),
                spdx: "GPL-3.0-or-later".into(),
            },
        }
    }

    fn rig() -> Rig {
        rig_with(slab(0.0))
    }

    fn subject<'a>(
        r: &'a Rig,
        hitbox: Option<&'a AssemblyHitbox>,
        landing: Option<([i32; 3], [i32; 3])>,
    ) -> Subject<'a> {
        Subject {
            id: "assembly/limb",
            path: "/content/assemblies/0".into(),
            mark: [10, 1, 10],
            facing: Facing::South,
            rig: r,
            initial: Some("idle"),
            hitbox,
            strikes: Some((
                ([6, 1, 6], [14, 3, 16]),
                vec![StepSubject {
                    index: 0,
                    windup: "windup",
                    hold: 20,
                    strike: "strike",
                    ticks_per_frame: None,
                    landings: vec![Landing {
                        path: "/content/assemblies/0/strikes/pattern/0/on_land/0".into(),
                        within: landing,
                        declares_in: landing.is_some(),
                        amount: 6,
                        nested: false,
                    }],
                }],
            )),
            struck_by: vec![],
            performed: vec![],
            aim: None,
        }
    }

    /// The floor: every cell of the course `y = 1`.
    fn floor(c: [i32; 3]) -> bool {
        c[1] == 1
    }

    fn near(_: &str, _: [f64; 3], _: [f64; 3]) -> bool {
        true
    }

    fn codes(f: &[Failure]) -> Vec<&str> {
        f.iter().map(|x| x.code.id()).collect()
    }

    /// The slab's own line, one cell in from every edge, is a landing whose
    /// keep-out is exactly the slab: green both ways.
    #[test]
    fn a_blow_lands_where_the_limb_comes_down() {
        let r = rig();
        let ok = subject(&r, None, Some(([10, 1, 12], [10, 1, 14])));
        let (j, f) = judge(&ok, &floor, &near);
        assert!(
            f.is_empty(),
            "{:?}",
            f.iter().map(|x| &x.message).collect::<Vec<_>>()
        );
        assert_eq!(j.facings, 1);
        assert_eq!(j.records[0].caught.len(), 15);
        assert_eq!(j.records[0].facings[0].comes_down.len(), 15);
        assert_eq!(j.records[0].facings[0].stand, Some([10, 1, 12]));
    }

    /// Defect 1: a limb that stops above the floor band has not landed on
    /// anybody. The slab hangs a block up, and two — in the column a
    /// floor-to-three-above band read as reached — and every landing cell is
    /// refused.
    #[test]
    fn a_limb_above_the_floor_band_has_not_landed() {
        for lift in [1.0, 2.0] {
            let r = rig_with(slab(lift));
            let s = subject(&r, None, Some(([10, 1, 12], [10, 1, 14])));
            let (_, f) = judge(&s, &floor, &near);
            assert_eq!(codes(&f), vec!["DW0938"], "lift {lift}");
            assert!(
                f[0].message.contains("never comes down on"),
                "{}",
                f[0].message
            );
            assert!(
                f[0].message.contains("comes down on no standable cell"),
                "{}",
                f[0].message
            );
        }
    }

    /// Defect 2: a one-cell landing under the five-long slab is refused — the
    /// limb comes down on cells the blow does not catch.
    #[test]
    fn a_one_cell_landing_under_a_long_limb_is_refused() {
        let r = rig();
        let s = subject(&r, None, Some(([10, 1, 13], [10, 1, 13])));
        let (_, f) = judge(&s, &floor, &near);
        assert_eq!(codes(&f), vec!["DW0938"]);
        assert!(f[0].message.contains("comes down on 6"), "{}", f[0].message);
        assert!(f[0].message.contains("[10, 1, 11]"), "{}", f[0].message);
        assert!(f[0].message.contains("[10, 1, 15]"), "{}", f[0].message);
    }

    /// The tolerance is the keep-out ring and no more: a landing one cell
    /// short of the slab's inner line at the far end leaves the slab's last
    /// row a ring away — still caught; two short, refused.
    #[test]
    fn the_tolerance_is_the_keep_out_ring() {
        let r = rig();
        // Landing z 12..=13: its keep-out reaches z 14, the slab's z 15 row
        // is two away.
        let s = subject(&r, None, Some(([10, 1, 12], [10, 1, 13])));
        let (_, f) = judge(&s, &floor, &near);
        assert_eq!(codes(&f), vec!["DW0938"]);
        assert!(f[0].message.contains("[10, 1, 15]"), "{}", f[0].message);
        assert!(!f[0].message.contains("[10, 1, 14]"), "{}", f[0].message);
    }

    /// A landing box beyond the limb is refused both ways: it catches where
    /// the limb is not, and the limb comes down where it does not land.
    #[test]
    fn a_blow_beside_the_limb_is_refused_both_ways() {
        let r = rig();
        let bad = subject(&r, None, Some(([13, 1, 13], [13, 1, 13])));
        let (_, f) = judge(&bad, &floor, &near);
        assert_eq!(codes(&f), vec!["DW0938", "DW0938"]);
        assert!(
            f[0].message.contains("never comes down on"),
            "{}",
            f[0].message
        );
        assert!(f[1].message.contains("comes down on"), "{}", f[1].message);
    }

    /// The pose the strike began from is not where it came down: a part that
    /// stands over a floor behind the mark through the whole strike (the
    /// limb's root) is not owed a landing there; the same part arriving there only
    /// on the last frame is.
    #[test]
    fn where_the_limb_already_stood_is_not_owed_a_blow() {
        let root = cube([-0.5, 0.0, -2.5]);
        let two = |first_root: Transform| {
            let mut r = rig();
            r.parts.push(r.parts[0].clone());
            r.parts[1].id = "b".into();
            for c in r.clips.values_mut() {
                for f in &mut c.frames {
                    f.push(root.clone());
                }
            }
            r.clips.get_mut("strike").unwrap().frames[0][1] = first_root;
            r
        };
        let stood = two(root.clone());
        let s = subject(&stood, None, Some(([10, 1, 12], [10, 1, 14])));
        let (j, f) = judge(&s, &floor, &near);
        assert!(
            f.is_empty(),
            "{:?}",
            f.iter().map(|x| &x.message).collect::<Vec<_>>()
        );
        assert!(!j.records[0].facings[0].comes_down.contains(&[10, 1, 8]));
        // The root swung in from elsewhere: now it came down there, and the
        // landing does not catch the mark's floor.
        let arrived = two(cube([5.5, 0.0, -0.5]));
        let s = subject(&arrived, None, Some(([10, 1, 12], [10, 1, 14])));
        let (_, f) = judge(&s, &floor, &near);
        assert_eq!(codes(&f), vec!["DW0938"]);
        assert!(f[0].message.contains("[10, 1, 8]"), "{}", f[0].message);
    }

    /// A blow on a cell a part already stood on, from a strike that brings
    /// nothing down, is no blow: refused even though the cell is stood on.
    #[test]
    fn a_strike_that_brings_nothing_down_is_refused() {
        let mut r = rig();
        r.parts.push(r.parts[0].clone());
        r.parts[1].id = "b".into();
        for c in r.clips.values_mut() {
            for f in &mut c.frames {
                f.push(cube([-0.5, 0.0, -2.5]));
            }
        }
        let mut s = subject(&r, None, Some(([10, 1, 8], [10, 1, 8])));
        if let Some((_, steps)) = &mut s.strikes {
            steps[0].strike = "windup";
        }
        let (_, f) = judge(&s, &floor, &near);
        assert_eq!(codes(&f), vec!["DW0938"]);
        assert!(
            f[0].message.contains("nothing comes down onto"),
            "{}",
            f[0].message
        );
    }

    /// A landing box one cell outside the arming region's keep-out is refused
    /// (shape 1); the same box one cell inside is not refused for it.
    #[test]
    fn a_blow_lands_only_where_it_was_announced() {
        let r = rig();
        // The arming keep-out on z is 5..=17 (the box 6..=16 grown by one).
        let outside = subject(&r, None, Some(([10, 1, 17], [10, 1, 17])));
        let (_, f) = judge(&outside, &|_| false, &near);
        assert_eq!(codes(&f), vec!["DW0938"]);
        assert!(f[0].message.contains("never wound up"), "{}", f[0].message);
        let inside = subject(&r, None, Some(([10, 1, 16], [10, 1, 16])));
        let (_, f) = judge(&inside, &|_| false, &near);
        assert!(f.is_empty(), "{:?}", codes(&f));
    }

    /// A box-less `damage-players` in a landing is refused.
    #[test]
    fn a_boxless_blow_is_refused() {
        let r = rig();
        let s = subject(&r, None, None);
        let (_, f) = judge(&s, &|_| true, &near);
        assert_eq!(codes(&f), vec!["DW0938"]);
    }

    /// A one-frame wind-up, a hold of 0 and a blow of 40 build green with no
    /// finding at all: no telegraph rule and no damage rule (spec-0016 §3).
    /// The strike lands one cadence after its last frame is applied.
    #[test]
    fn no_telegraph_or_damage_rule() {
        let r = rig();
        let mut s = subject(&r, None, Some(([10, 1, 12], [10, 1, 14])));
        if let Some((_, steps)) = &mut s.strikes {
            steps[0].hold = 0;
            steps[0].landings[0].amount = 40;
        }
        let (j, f) = judge(&s, &floor, &near);
        assert!(f.is_empty());
        assert_eq!(j.records[0].amounts, vec![40]);
        assert_eq!(j.records[0].hold, 0);
        assert_eq!(j.records[0].windup_ticks, 1);
        // Two frames at 5: applied on tick 6, drawn whole on tick 11.
        assert_eq!(j.records[0].strike_ticks, 11);
    }

    // ---- aim (spec-0082 §5.7) ----

    /// Facing k of n is a turn of k/n about +y and a root yaw of -k/n turns.
    #[test]
    fn facings_turn_and_yaw() {
        assert_eq!(root_yaw(0, 8), 0.0);
        assert_eq!(root_yaw(2, 8), -90.0);
        assert_eq!(root_yaw(4, 8), 180.0);
        assert_eq!(root_yaw(6, 8), 90.0);
        assert_eq!(yaw_token(root_yaw(1, 16)), "-22.5");
        assert_eq!(yaw_token(root_yaw(5, 8)), "135");
        assert!((facing_turn(2, 8) - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
    }

    /// A box turned a quarter turn is the box's cells turned exactly; turned
    /// an eighth, the cells whose centres fall inside it.
    #[test]
    fn a_landing_box_turns_about_the_mark() {
        let m = [10, 1, 10];
        let line = ([10, 1, 12], [10, 1, 14]);
        let quarter = turned_region(m, line, facing_turn(2, 8));
        assert_eq!(
            quarter,
            [[12, 1, 10], [13, 1, 10], [14, 1, 10]]
                .into_iter()
                .collect()
        );
        let eighth = turned_region(m, line, facing_turn(1, 8));
        assert!(
            eighth.contains(&[11, 1, 11]) || eighth.contains(&[12, 1, 12]),
            "{eighth:?}"
        );
        assert!(eighth.iter().all(|c| c[0] > 10 && c[2] > 10), "{eighth:?}");
        assert_eq!(turned_region(m, line, 0.0).len(), 3);
    }

    /// An arming region all round the mark draws every facing; one on its
    /// south side only draws the facings whose sector meets it.
    #[test]
    fn the_facings_a_player_can_draw() {
        let m = [10, 1, 10];
        assert_eq!(drawn_facings(m, ([6, 1, 6], [14, 3, 16]), 8, 0.0).len(), 8);
        let south = drawn_facings(m, ([9, 1, 14], [11, 1, 16]), 8, 0.0);
        assert_eq!(south, vec![0, 1, 7]);
        assert_eq!(facing_for(m, [10, 1, 15], 8, 0.0), 0);
        assert_eq!(facing_for(m, [15, 1, 10], 8, 0.0), 2);
        assert_eq!(facing_for(m, [5, 1, 10], 8, 0.0), 6);
        assert_eq!(drawn_facings(m, ([9, 1, 14], [11, 1, 16]), 1, 0.0), vec![0]);
    }

    /// An aimed pattern is judged at every facing it can draw, the limb and
    /// the box turned together: green at all eight; a nested blow is refused.
    #[test]
    fn an_aimed_strike_is_judged_per_facing() {
        let r = rig();
        let mut s = subject(&r, None, Some(([10, 1, 12], [10, 1, 14])));
        s.aim = Some(4);
        let (j, f) = judge(&s, &floor, &near);
        assert!(
            f.is_empty(),
            "{:?}",
            f.iter().map(|x| &x.message).collect::<Vec<_>>()
        );
        assert_eq!(j.facings, 4);
        let ks: Vec<u32> = j.records[0].facings.iter().map(|x| x.k).collect();
        assert_eq!(ks, vec![0, 1, 2, 3]);
        assert_eq!(j.records[0].facings[1].stand, Some([12, 1, 10]));
        if let Some((_, steps)) = &mut s.strikes {
            steps[0].landings[0].nested = true;
        }
        let (_, f) = judge(&s, &floor, &near);
        assert_eq!(codes(&f), vec!["DW0938"]);
        assert!(f[0].message.contains("aimed"), "{}", f[0].message);
    }

    /// A one-cell landing under the long slab is refused at the facing it is
    /// refused at, and the refusal names the facing.
    #[test]
    fn an_aimed_refusal_names_its_facing() {
        let r = rig();
        let mut s = subject(&r, None, Some(([10, 1, 13], [10, 1, 13])));
        s.aim = Some(4);
        let (_, f) = judge(&s, &floor, &near);
        assert_eq!(f.len(), 4);
        assert!(f[1].message.contains("facing 1 of 4"), "{}", f[1].message);
    }

    /// The region's blow tags each caught player once, runs of cells along
    /// x one selector each, deals the blow once, and clears the tag.
    #[test]
    fn a_turned_blow_is_dealt_once() {
        let region: BTreeSet<[i32; 3]> = [[11, 1, 11], [12, 1, 11], [12, 1, 12]]
            .into_iter()
            .collect();
        let lines = region_damage_lines("limb", &region, 6, "minecraft:generic");
        assert_eq!(
            lines,
            vec![
                "tag @a[x=11,dx=1,y=1,dy=0,z=11,dz=0,tag=!dw_cutscene] add dw_asm_limb_struck",
                "tag @a[x=12,dx=0,y=1,dy=0,z=12,dz=0,tag=!dw_cutscene] add dw_asm_limb_struck",
                "execute as @a[tag=dw_asm_limb_struck] run damage @s 6 minecraft:generic",
                "tag @a[tag=dw_asm_limb_struck] remove dw_asm_limb_struck",
            ]
        );
        assert_eq!(nearest_drawn(&[0, 1, 7], 4, 8), 1);
        assert_eq!(nearest_drawn(&[0, 1, 7], 5, 8), 7);
        assert_eq!(nearest_drawn(&[0, 1, 7], 1, 8), 1);
    }

    /// Width 7 and height 23 are refused; 6 and 22 are not; a hitbox beside
    /// the parts is refused naming their cells.
    #[test]
    fn the_hitbox_bounds_and_its_mark() {
        let r = rig();
        let at = |w: f64, h: f64, o: [i32; 3]| AssemblyHitbox {
            width: w,
            height: h,
            offset: o,
        };
        for (w, h, refused) in [(7.0, 2.0, true), (2.0, 23.0, true), (6.0, 22.0, false)] {
            let hb = at(w, h, [0, 0, 0]);
            let mut s = subject(&r, Some(&hb), Some(([10, 1, 12], [10, 1, 12])));
            s.strikes = None;
            let (_, f) = judge(&s, &|_| true, &near);
            assert_eq!(!f.is_empty(), refused, "{w}x{h}: {:?}", codes(&f));
        }
        let hb = at(1.0, 1.0, [5, 0, 0]);
        let mut s = subject(&r, Some(&hb), None);
        s.strikes = None;
        let (_, f) = judge(&s, &|_| true, &near);
        assert_eq!(codes(&f), vec!["DW0936"]);
        assert!(f[0].message.contains("[10, 1, 10]"), "{}", f[0].message);
    }

    /// A performed strike with nothing in reach is `DW0937`; a struck assembly
    /// with no hitbox is `DW0936`.
    #[test]
    fn reach_and_the_missing_hitbox() {
        let r = rig();
        let hb = AssemblyHitbox {
            width: 1.0,
            height: 2.0,
            offset: [0, 0, 0],
        };
        let mut s = subject(&r, Some(&hb), None);
        s.strikes = None;
        s.struck_by = vec!["trigger/hit"];
        s.performed = vec!["trigger/hit"];
        let (_, f) = judge(&s, &|_| true, &|_, _, _| false);
        assert_eq!(codes(&f), vec!["DW0937"]);
        let mut s = subject(&r, None, None);
        s.strikes = None;
        s.struck_by = vec!["trigger/hit"];
        let (_, f) = judge(&s, &|_| true, &near);
        assert_eq!(codes(&f), vec!["DW0936"]);
    }

    #[test]
    fn transformation_snbt_is_fixed_point() {
        let t = cube([-0.5, 0.0, -0.0]);
        assert_eq!(
            transformation(&t),
            "{translation:[-0.5000f,0.0000f,0.0000f],left_rotation:[0.0000f,0.0000f,0.0000f,1.0000f],scale:[1.0000f,1.0000f,1.0000f],right_rotation:[0.0000f,0.0000f,0.0000f,1.0000f]}"
        );
    }
}
