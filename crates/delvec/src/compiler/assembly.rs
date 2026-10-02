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
//!   blow is where the limb is (every caught cell of a landing box has a cell
//!   of the strike clip's last-frame footprint in its column, from its floor to
//!   three cells above it).
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

/// How far above a caught cell's floor the blow may land and still be the
/// thing that hurt the player standing there (spec-0082 §5.4 shape 2).
pub const LANDING_REACH_CELLS: i32 = 3;

/// The `interpolation_duration` a return to the rest pose is drawn over, in
/// ticks: the spike's own summon cadence (spec-0082 §8).
pub const REST_INTERPOLATION: u32 = 5;

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
    /// The landings in its `on_land`.
    pub landings: Vec<Landing>,
}

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
    pub strikes: Option<(([i32; 3], [i32; 3]), Vec<StepSubject<'a>>)>,
    /// `strike-assembly` triggers naming it, by id.
    pub struck_by: Vec<&'a str>,
    /// Of those, the ones the critical path performs.
    pub performed: Vec<&'a str>,
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
    /// Ticks from the strike clip starting to the landing.
    pub strike_ticks: u32,
    /// Every landing's `amount`.
    pub amounts: Vec<u32>,
    /// Every caught cell of every landing box, in the world.
    pub caught: Vec<[i32; 3]>,
}

/// What one [`judge`] examined.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Judged {
    /// Hitboxes examined.
    pub hitboxes: usize,
    /// Strike steps checked.
    pub steps: usize,
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
    reaches: &dyn Fn(&str, [f64; 3], [f64; 3]) -> bool,
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
            let seen = offset_all(&rig::frame_footprint(&pose, s.facing), s.mark);
            if !seen.iter().any(|c| cell_meets(*c, lo, hi)) {
                out.push(Failure::new(
                    DW_ASSEMBLY_HITBOX,
                    format!(
                        "assembly `{}`'s hitbox ({}/hitbox) spans {lo:?}..{hi:?} and meets none of \
                         the {} cell(s) its parts stand in when it spawns ({}): {}. The player \
                         strikes what the player sees, and a box beside the thing is a box \
                         nobody aims at. Move the hitbox's `offset` (or size it) so it covers the \
                         parts",
                        s.id,
                        s.path,
                        seen.len(),
                        match s.initial {
                            Some(c) => format!("the first frame of `{c}`"),
                            None => "the rest pose".to_string(),
                        },
                        rig::cells_line(&seen)
                    ),
                ));
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
    for step in steps {
        j.steps += 1;
        let strike_clip = s.rig.clips.get(step.strike);
        let windup_clip = s.rig.clips.get(step.windup);
        let limb = strike_clip
            .map(|c| offset_all(&rig::last_frame_footprint(c, s.facing), s.mark))
            .unwrap_or_default();
        let mut record = StepRecord {
            assembly: s.id.to_string(),
            step: step.index,
            windup_ticks: windup_clip.map(|c| c.length_ticks()).unwrap_or(0),
            hold: step.hold,
            strike_ticks: strike_clip.map(|c| c.length_ticks()).unwrap_or(0),
            amounts: step.landings.iter().map(|l| l.amount).collect(),
            caught: Vec::new(),
        };
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
            // Shape 1: it lands only where it was announced.
            let keep = keep_out_box(body, within.0, within.1);
            if !(in_box(keep.0, arm_keep) && in_box(keep.1, arm_keep)) {
                out.push(Failure::new(
                    DW_ASSEMBLY_STRIKE,
                    format!(
                        "assembly `{}`'s strike step {} lands a blow ({}) whose box catches a body \
                         from the feet cells {:?}..={:?}, and the arming region `while_in` catches \
                         one only from {:?}..={:?}. A player who never entered the arming region \
                         would be struck by a blow that was never wound up for them. Shrink or \
                         move the landing box inside `while_in`, or widen `while_in` to cover it",
                        s.id, step.index, l.path, keep.0, keep.1, arm_keep.0, arm_keep.1
                    ),
                ));
                continue;
            }
            // Shape 2: the blow is where the limb is.
            let caught: Vec<[i32; 3]> = cells_of(keep)
                .into_iter()
                .filter(|c| population(*c))
                .collect();
            let unmet: Vec<[i32; 3]> = caught
                .iter()
                .copied()
                .filter(|c| {
                    !(0..=LANDING_REACH_CELLS).any(|dy| limb.contains(&[c[0], c[1] + dy, c[2]]))
                })
                .collect();
            record.caught.extend(caught.iter().copied());
            if !unmet.is_empty() {
                out.push(Failure::new(
                    DW_ASSEMBLY_STRIKE,
                    format!(
                        "assembly `{}`'s strike step {} lands a blow ({}) on {} standable cell(s) \
                         the strike clip `{}` never reaches — no part stands in the column from \
                         the cell's floor to {LANDING_REACH_CELLS} above it on the clip's last \
                         frame: {}. The thing the player saw come down must be the thing that \
                         hurt them. The limb's last frame covers {}. Move the landing box under \
                         the limb (`delvec rig describe` prints the footprint), or choose a \
                         strike clip that reaches it",
                        s.id,
                        step.index,
                        l.path,
                        unmet.len(),
                        step.strike,
                        unmet
                            .iter()
                            .map(|c| format!("[{}, {}, {}]", c[0], c[1], c[2]))
                            .collect::<Vec<_>>()
                            .join(" "),
                        rig::cells_line(&limb)
                    ),
                ));
            }
        }
        record.caught.sort();
        record.caught.dedup();
        j.records.push(record);
    }
    (j, out)
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
fn landings(plan: &Plan, path: &str, effs: &[QuestEffect], out: &mut Vec<Landing>) {
    for (i, e) in effs.iter().enumerate() {
        let here = format!("{path}/{i}");
        if let Verb::DamagePlayers { amount, within, .. } = &e.verb {
            out.push(Landing {
                path: here.clone(),
                within: within.as_ref().and_then(|z: &StealthZone| plan.zone_box(z)),
                declares_in: within.is_some(),
                amount: *amount,
            });
        }
        for (seg, _, list) in e.nested_effect_lists_labeled() {
            landings(plan, &format!("{here}/{seg}"), list, out);
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
                    &mut ls,
                );
                StepSubject {
                    index: i,
                    windup: step.windup.as_str(),
                    hold: step.hold,
                    strike: step.strike.as_str(),
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
    /// Refusals.
    pub refused: usize,
    /// The keyframe writes per tick the declared cadences add up to, at worst:
    /// each assembly's part count over its fastest clip's cadence.
    pub writes_per_tick: f64,
    /// Per-step records.
    pub records: Vec<StepRecord>,
}

impl AssemblyBinding {
    /// The binding line.
    pub fn line(&self) -> String {
        format!(
            "assembly binding: {} assembl(ies) declared, {} part(s), {} clip(s), {} hitbox(es) \
             examined, {} strike step(s) checked, {} refused",
            self.declared, self.parts, self.clips, self.hitboxes, self.steps, self.refused
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
            })).collect::<Vec<_>>(),
        })
    }
}

/// **Judge every assembly in the build** and return what was examined beside
/// the refusals. `population` and `reaches` are as for [`judge`].
pub fn check(
    plan: &Plan<'_>,
    population: &dyn Fn([i32; 3]) -> bool,
    reaches: &dyn Fn(&str, [f64; 3], [f64; 3]) -> bool,
) -> (AssemblyBinding, Vec<Failure>) {
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
        let (j, f) = judge(&s, population, reaches);
        b.hitboxes += j.hitboxes;
        b.steps += j.steps;
        b.records.extend(j.records);
        failures.extend(f);
    }
    b.refused = failures.len();
    (b, failures)
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

/// Every function the build's assemblies need. `land` lowers one effect of an
/// `on_land` bundle (the ordinary effect emitter, with no acting player).
pub fn functions(
    plan: &Plan<'_>,
    land: &dyn Fn(&QuestEffect, &mut Vec<String>),
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
        for (k, (_, c)) in clips.iter().enumerate() {
            out.push((
                format!("asm_play_{s}_{k}"),
                join(vec![
                    format!("scoreboard players set {} dw.sys {k}", h("clip")),
                    format!("scoreboard players set {} dw.sys -1", h("f")),
                    format!(
                        "scoreboard players set {} dw.sys {}",
                        h("t"),
                        c.ticks_per_frame - 1
                    ),
                    format!(
                        "scoreboard players set {} dw.sys {}",
                        h("tpf"),
                        c.ticks_per_frame
                    ),
                    format!("scoreboard players set {} dw.sys 0", h("done")),
                ]),
            ));
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
            for (f, frame) in c.frames.iter().enumerate() {
                let body: Vec<String> = frame
                    .iter()
                    .enumerate()
                    .map(|(i, t)| {
                        format!(
                            "execute as @e[tag={},limit=1] run data merge entity @s {{start_interpolation:0,interpolation_duration:{},transformation:{}}}",
                            part_tag(s, i),
                            c.ticks_per_frame,
                            transformation(&t.faced(facing))
                        )
                    })
                    .collect();
                out.push((format!("asm_frame_{s}_{k}_{f}"), join(body)));
            }
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
        for (k, (_, c)) in clips.iter().enumerate() {
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
            tick.push(format!(
                "execute if score {} dw.sys matches 3 if score {} dw.sys matches 1 run function {ns}:{}",
                h("sm"),
                h("done"),
                land_fn(s)
            ));
            let mut begin = vec![format!("scoreboard players set {} dw.sys 1", h("sm"))];
            let mut hold = vec![
                format!("scoreboard players set {} dw.sys 2", h("sm")),
                format!("scoreboard players set {} dw.sys 0", h("hold")),
            ];
            let mut swing = vec![format!("scoreboard players set {} dw.sys 3", h("sm"))];
            let mut landing = Vec::new();
            for (j, step) in st.pattern.iter().enumerate() {
                if let Some(k) = idx(&step.windup) {
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
                if let Some(k) = idx(&step.strike) {
                    swing.push(format!(
                        "execute if score {} dw.sys matches {j} run function {ns}:asm_play_{s}_{k}",
                        h("step")
                    ));
                }
                if !step.on_land.is_empty() {
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

    fn cube(t: [f64; 3]) -> Transform {
        Transform {
            translation: t,
            left_rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
            right_rotation: [0.0, 0.0, 0.0, 1.0],
        }
    }

    /// A one-part rig: `idle` stands the cube on the mark cell, `windup` keeps
    /// it there, `strike` lays it two cells south (+z) on the floor.
    fn rig() -> Rig {
        let mut clips = BTreeMap::new();
        let still = |t: [f64; 3], looping| Clip {
            ticks_per_frame: 5,
            looping,
            frames: vec![vec![cube(t)]],
        };
        clips.insert("idle".to_string(), still([-0.5, 0.0, -0.5], true));
        clips.insert("windup".to_string(), still([-0.5, 1.0, -0.5], false));
        clips.insert("strike".to_string(), still([-0.5, 0.0, 1.5], false));
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
                    landings: vec![Landing {
                        path: "/content/assemblies/0/strikes/pattern/0/on_land/0".into(),
                        within: landing,
                        declares_in: landing.is_some(),
                        amount: 6,
                    }],
                }],
            )),
            struck_by: vec![],
            performed: vec![],
        }
    }

    fn floor(c: [i32; 3]) -> bool {
        c[1] == 1
    }

    fn near(_: &str, _: [f64; 3], _: [f64; 3]) -> bool {
        true
    }

    fn codes(f: &[Failure]) -> Vec<&str> {
        f.iter().map(|x| x.code.id()).collect()
    }

    /// The landing box under the strike's last frame, inside the arming
    /// region, is green; one cell further it is refused (shape 2).
    #[test]
    fn a_blow_lands_where_the_limb_is() {
        let r = rig();
        // The strike lays the cube at [10, 1, 12]; a body is caught from the
        // keep-out of the box, one cell wider on x and z.
        let ok = subject(&r, None, Some(([10, 1, 12], [10, 1, 12])));
        let (_, f) = judge(&ok, &|c| floor(c) && c == [10, 1, 12], &near);
        assert!(f.is_empty(), "{:?}", codes(&f));
        let bad = subject(&r, None, Some(([10, 1, 14], [10, 1, 14])));
        let (_, f) = judge(&bad, &|c| floor(c) && c == [10, 1, 14], &near);
        assert_eq!(codes(&f), vec!["DW0938"]);
        assert!(f[0].message.contains("[10, 1, 14]"), "{}", f[0].message);
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
    #[test]
    fn no_telegraph_or_damage_rule() {
        let r = rig();
        let mut s = subject(&r, None, Some(([10, 1, 12], [10, 1, 12])));
        if let Some((_, steps)) = &mut s.strikes {
            steps[0].hold = 0;
            steps[0].landings[0].amount = 40;
        }
        let (j, f) = judge(&s, &|c| c == [10, 1, 12], &near);
        assert!(f.is_empty());
        assert_eq!(j.records[0].amounts, vec![40]);
        assert_eq!(j.records[0].hold, 0);
        assert_eq!(j.records[0].windup_ticks, 1);
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
