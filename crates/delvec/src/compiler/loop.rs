//! **An endless corridor** (spec-0086): a slab whose crossing returns a body by
//! a whole-block offset to an earlier section that looks exactly the same, held
//! until the party releases it.
//!
//! The module owns the loop end to end, in two halves.
//!
//! * **The plan half** ([`LoopPlan`], [`resolve`], [`LoopReplay`]) runs inside
//!   `Plan::build`, before a block is read: it resolves each declared loop to
//!   its slab, its landing and its offset, and replays its gate along a path so
//!   the critical path can splice the **exercise step** (spec-0086 §5.2) and the
//!   route proof can read the slab as a **gated seal** (§5.1) — solid while the
//!   loop holds, clear once it stands down. The seal is not a write object of
//!   its own: it is the gate's state, read at each step from the same journal
//!   every other gate is read from, and handed to the one region model as a
//!   [`RegionWrite::Hold`] at the step the gate opens and a
//!   [`RegionWrite::Unseal`] at the step it shuts.
//! * **The world half** ([`check`]) runs inside the build over the assembled
//!   blocks: the slab rule (`DW0945`), the periodic span and its closed view
//!   (`DW0947`), identical blocks and light inside it in every configuration the
//!   route passes through (`DW0946`), and no compiler-placed body in it
//!   (`DW0948`). Its binding line prints on every build that declares a loop,
//!   refusals included, and `validation/loop-gate.json` carries the same counts.
//!
//! # What "seamless" is claimed to mean
//!
//! At the moment a body is moved from `p` to `p + d`, everything it could see
//! from `p + d` is what it could see from `p`, cell for cell. The server keeps
//! the body's position inside its cell, its facing and its velocity, and the
//! client is told one fully relative update (spec-0086 §2), so the only thing
//! that can betray the move is the world. The engine proves the view **closed**
//! — by geometry, or by an atmosphere's fog where one is painted — and
//! **identical** inside the closure.
//!
//! # Fog
//!
//! A fog end is a per-biome attribute read through the client's blend kernel
//! around the eye. This tree paints no atmosphere, so every eye reads the
//! attribute's default ([`FOG_END_DEFAULT`]) and no cell inside the span's
//! reach is closed by fog: [`fog_end_at`] is the one site a painted biome map
//! is read at, and it answers the default until one exists.

use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::{
    Campaign, CompareOp, DwCode, ExitTier, Loop, QuestEffect, StateCompare, StateWrite, Verb,
};

use crate::compiler::plan::{RegionEvent, RegionWrite, ResolvedAnchor};

/// `DW0945`: **the slab's geometry** (spec-0086 §4.2) — not a slab, a move that
/// does not clear it, a slab too thin for the one-tick poll along its axis, a
/// slab overlapping another loop's slab or landing, a `teleport` volume or a
/// lethal keep-out, or a slab cell a body cannot be in. Build tier.
pub const DW_LOOP_SLAB: DwCode = DwCode::new("DW0945", ExitTier::Build);

/// `DW0946`: **a visible cell whose block or light differs from its image**
/// under the loop's offset (spec-0086 §4.4–§4.6), in any configuration the
/// route passes through, or a declared volume in the span without its image.
/// Build tier.
pub const DW_LOOP_TILING: DwCode = DwCode::new("DW0946", ExitTier::Build);

/// `DW0947`: **the view out of the landing is open** (spec-0086 §4.3): the
/// periodic span grows [`SPAN_REACH`] cells on some face, or into open sky,
/// without geometry or the eye's fog closing it. Build tier.
pub const DW_LOOP_OPEN_VIEW: DwCode = DwCode::new("DW0947", ExitTier::Build);

/// `DW0948`: **a compiler-placed body inside the periodic span** (spec-0086
/// §4.6): a body has an identity the move cannot repeat. Build tier.
pub const DW_LOOP_BODY: DwCode = DwCode::new("DW0948", ExitTier::Build);

/// `DW0950`: **advisory — the forced route never meets the loop while it
/// holds** (spec-0086 §5.2). A declared mechanism nobody is made to experience
/// is reported, not refused.
pub const DW_LOOP_UNMET: DwCode = DwCode::new("DW0950", ExitTier::Build);

/// `R`: how far the periodic span may grow from the landing slab on any face
/// before the view is called open (spec-0086 §4.3). The campaign's render
/// horizon is not the engine's to know, so the bound is the engine's, named,
/// and printed.
pub const SPAN_REACH: i32 = 128;

/// The fog end a biome declares when it declares none — vanilla's default for
/// `visual/fog_end_distance`, in blocks.
pub const FOG_END_DEFAULT: f64 = 1024.0;

/// How many crossings the exercise step will count before it decides the
/// loop's own writes never shut its gate (spec-0086 §5.2).
pub const EXERCISE_CROSSINGS_MAX: u32 = 64;

/// The fog end the client reads at `eye` (spec-0086 §4.3): the kernel-weighted
/// mean of every nearby biome's `visual/fog_end_distance`. The one site the
/// periodic span reads fog at; with no atmosphere painted, every biome reads
/// the attribute's default.
pub fn fog_end_at(_eye: [f64; 3]) -> f64 {
    FOG_END_DEFAULT
}

/// One resolved loop: its slab, where its anchor cell lands, the offset that
/// moves it there, its gate, its count and its answer.
#[derive(Clone, Debug)]
pub struct LoopPlan {
    /// The authored id (`loop/<kebab>`).
    pub id: String,
    /// `safe_local(id)` — the segment that names the emitted functions.
    pub safe: String,
    /// The area the slab's anchor resolved in.
    pub area: String,
    /// The slab's anchor cell.
    pub anchor_cell: [i32; 3],
    /// Inclusive world corners of the slab (`anchor ± extent`).
    pub slab: ([i32; 3], [i32; 3]),
    /// The cell the slab's anchor cell lands on: `to`'s mark.
    pub to_cell: [i32; 3],
    /// `d = cell(to) − cell(region.anchor)`, whole blocks by construction.
    pub offset: [i32; 3],
    /// The gate, as declared.
    pub requires_flags: Vec<String>,
    /// Flags any of which stands the loop down.
    pub forbids_flags: Vec<String>,
    /// Numeric gate terms.
    pub requires_state: Vec<StateCompare>,
    /// The `party` datum each move raises, if declared.
    pub counts: Option<String>,
    /// The dungeon's answer to a move.
    pub on_cross: Vec<QuestEffect>,
}

impl LoopPlan {
    /// The crossing axis: the one axis the offset moves along. `None` when the
    /// offset is zero or moves along more than one axis — not a slab.
    pub fn axis(&self) -> Option<usize> {
        let moved: Vec<usize> = (0..3).filter(|&i| self.offset[i] != 0).collect();
        match moved.as_slice() {
            [a] => Some(*a),
            _ => None,
        }
    }

    /// The slab's thickness along `axis`, in cells.
    pub fn thickness(&self, axis: usize) -> i32 {
        self.slab.1[axis] - self.slab.0[axis] + 1
    }

    /// The landing slab: the slab carried by the offset.
    pub fn landing(&self) -> ([i32; 3], [i32; 3]) {
        (
            shift(self.slab.0, self.offset),
            shift(self.slab.1, self.offset),
        )
    }

    /// Whether `cell` lies inside the slab.
    pub fn slab_contains(&self, cell: [i32; 3]) -> bool {
        inside(self.slab, cell)
    }

    /// The landing cell the exercise step names: where the slab's anchor cell
    /// is put down.
    pub fn transport(&self) -> [i32; 3] {
        shift(self.anchor_cell, self.offset)
    }

    /// The side of the slab a body approaches from, along the crossing axis:
    /// `-1` when the approach is at lower coordinates, `+1` at higher. The
    /// landing is always on it, because the move carries the slab out of
    /// itself toward the approach.
    pub fn approach_sign(&self) -> Option<i32> {
        self.axis().map(|a| self.offset[a].signum())
    }

    /// Whether `cell` lies on the approach side of the slab's plane.
    pub fn on_approach(&self, cell: [i32; 3]) -> bool {
        let (Some(a), Some(s)) = (self.axis(), self.approach_sign()) else {
            return false;
        };
        if s < 0 {
            cell[a] < self.slab.0[a]
        } else {
            cell[a] > self.slab.1[a]
        }
    }

    /// Whether `cell` lies beyond the slab's plane, on the far side from the
    /// approach — where a body arrives only by crossing.
    pub fn beyond(&self, cell: [i32; 3]) -> bool {
        let (Some(a), Some(s)) = (self.axis(), self.approach_sign()) else {
            return false;
        };
        if s < 0 {
            cell[a] > self.slab.1[a]
        } else {
            cell[a] < self.slab.0[a]
        }
    }

    /// The gate's terms in words, for a message naming what is still open.
    pub fn gate_words(&self) -> String {
        let mut out: Vec<String> = Vec::new();
        for f in &self.requires_flags {
            out.push(format!("`requires_flags: {f}`"));
        }
        for f in &self.forbids_flags {
            out.push(format!("`forbids_flags: {f}`"));
        }
        for c in &self.requires_state {
            out.push(format!(
                "`requires_state: {} {} {}`",
                c.state.as_str(),
                c.op.token(),
                c.value
            ));
        }
        out.join(", ")
    }
}

/// `a + d`, componentwise.
pub fn shift(a: [i32; 3], d: [i32; 3]) -> [i32; 3] {
    [a[0] + d[0], a[1] + d[1], a[2] + d[2]]
}

/// Whether `cell` lies in the inclusive box `b`.
pub fn inside(b: ([i32; 3], [i32; 3]), cell: [i32; 3]) -> bool {
    (0..3).all(|i| b.0[i].min(b.1[i]) <= cell[i] && cell[i] <= b.0[i].max(b.1[i]))
}

/// Whether two inclusive boxes share a cell.
pub fn boxes_meet(a: ([i32; 3], [i32; 3]), b: ([i32; 3], [i32; 3])) -> bool {
    (0..3).all(|i| a.0[i].max(b.0[i]) <= a.1[i].min(b.1[i]))
}

/// Resolve every declared loop whose two anchors a placed piece provides, in
/// declaration order. A loop whose anchor resolves nowhere is absent here;
/// validation names it (`DW0142`).
pub fn resolve(
    campaign: &Campaign,
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
) -> Vec<LoopPlan> {
    campaign
        .quests
        .content
        .loops
        .iter()
        .filter_map(|l| resolve_one(l, anchors))
        .collect()
}

fn resolve_one(l: &Loop, anchors: &BTreeMap<(String, String), ResolvedAnchor>) -> Option<LoopPlan> {
    let site = |name: &str| -> Option<(String, [i32; 3])> {
        anchors
            .iter()
            .find(|((_, n), _)| n == name)
            .map(|((area, _), r)| {
                (
                    area.clone(),
                    match r {
                        ResolvedAnchor::Point { pos, .. } => *pos,
                        ResolvedAnchor::Gate { from, .. } => *from,
                    },
                )
            })
    };
    let (area, anchor_cell) = site(l.region.anchor.as_str())?;
    let (_, to_anchor) = site(l.to.anchor.as_str())?;
    let to_cell = l.to.cell(to_anchor);
    let e = l.region.extent;
    let slab = (
        [
            anchor_cell[0] - e[0] as i32,
            anchor_cell[1] - e[1] as i32,
            anchor_cell[2] - e[2] as i32,
        ],
        [
            anchor_cell[0] + e[0] as i32,
            anchor_cell[1] + e[1] as i32,
            anchor_cell[2] + e[2] as i32,
        ],
    );
    Some(LoopPlan {
        id: l.id.as_str().to_string(),
        safe: crate::compiler::plan::safe_local(l.id.as_str()),
        area,
        anchor_cell,
        slab,
        to_cell,
        offset: [
            to_cell[0] - anchor_cell[0],
            to_cell[1] - anchor_cell[1],
            to_cell[2] - anchor_cell[2],
        ],
        requires_flags: l
            .requires_flags
            .iter()
            .map(|f| f.as_str().to_string())
            .collect(),
        forbids_flags: l
            .forbids_flags
            .iter()
            .map(|f| f.as_str().to_string())
            .collect(),
        requires_state: l.requires_state.clone(),
        counts: l.counts.as_ref().map(|c| c.as_str().to_string()),
        on_cross: l.on_cross.clone(),
    })
}

// ---------------------------------------------------------------------------
// The replay: the gate's state along a path
// ---------------------------------------------------------------------------

/// The party's state at one point of a path, as the loop gate reads it: the
/// flags held and every declared datum's value, `None` where no ordered walk
/// can name it.
#[derive(Clone, Debug, Default)]
pub struct GateState {
    /// Flags held.
    pub flags: BTreeSet<String>,
    /// Datum values; `None` is undatable.
    pub data: BTreeMap<String, Option<i64>>,
}

impl GateState {
    fn satisfies(&self, c: &StateCompare) -> Option<bool> {
        let v = (*self.data.get(c.state.as_str())?)?;
        let want = i64::from(c.value);
        Some(match c.op {
            CompareOp::Equals => v == want,
            CompareOp::NotEquals => v != want,
            CompareOp::AtLeast => v >= want,
            CompareOp::AtMost => v <= want,
        })
    }

    /// Whether a gate of these three lists is open here: `Some(false)` when a
    /// term decides it shut, `None` when a numeric term reads an undatable
    /// value and nothing else shut it.
    fn open(
        &self,
        requires: &[String],
        forbids: &[String],
        state: &[StateCompare],
    ) -> Option<bool> {
        if !requires.iter().all(|f| self.flags.contains(f)) {
            return Some(false);
        }
        if forbids.iter().any(|f| self.flags.contains(f)) {
            return Some(false);
        }
        let mut unknown = false;
        for c in state {
            match self.satisfies(c) {
                Some(false) => return Some(false),
                None => unknown = true,
                Some(true) => {}
            }
        }
        if unknown { None } else { Some(true) }
    }

    /// Whether loop `l` holds here. `None` is undatable, which every reader
    /// treats as holding: a seal the proof cannot show open stays shut.
    pub fn holds(&self, l: &LoopPlan) -> Option<bool> {
        self.open(&l.requires_flags, &l.forbids_flags, &l.requires_state)
    }

    /// The first gate term that is open here, in words, for a message naming
    /// why the loop still holds.
    pub fn open_term(&self, l: &LoopPlan) -> String {
        if let Some(f) = l.forbids_flags.iter().find(|f| !self.flags.contains(*f)) {
            return format!("`forbids_flags: {f}` (not set)");
        }
        if let Some(c) = l.requires_state.first() {
            let v = self.data.get(c.state.as_str()).copied().flatten();
            return format!(
                "`requires_state: {} {} {}` (the datum reads {})",
                c.state.as_str(),
                c.op.token(),
                c.value,
                v.map(|v| v.to_string())
                    .unwrap_or_else(|| "a value no ordered walk can name".to_string())
            );
        }
        if let Some(f) = l.requires_flags.iter().find(|f| self.flags.contains(*f)) {
            return format!("`requires_flags: {f}` (set)");
        }
        format!("{{{}}}", l.gate_words())
    }
}

/// What one exercise of a loop performs.
#[derive(Clone, Debug, Default)]
pub struct Exercise {
    /// How many crossings the step makes.
    pub times: u32,
    /// Whether the gate is shut after them.
    pub releases: bool,
    /// The region writes the crossings' `on_cross` effects perform, in order,
    /// as `(zone or gate anchor resolution, write)` — credited as forced at the
    /// step.
    pub writes: Vec<(([i32; 3], [i32; 3]), RegionWrite)>,
    /// The JSON pointers of the `on_cross` effects that fired, per crossing, in
    /// order — the configurations the world half checks.
    pub fired: Vec<Vec<usize>>,
}

/// The loop replay: the data only loops write, carried along a path beside the
/// flow model's own walk, and every flag an `on_cross` has set.
///
/// A datum some other root also writes is not the loop's to date, and reads as
/// undatable here exactly as it does in the flow model.
#[derive(Clone, Debug)]
pub struct LoopReplay {
    /// Data written by loops and by nothing else.
    owned: BTreeSet<String>,
    /// Their current values.
    values: BTreeMap<String, i64>,
    /// Every declared datum's initial value.
    initial: BTreeMap<String, i64>,
    /// Flags an exercise's `on_cross` set.
    flags: BTreeSet<String>,
}

impl LoopReplay {
    /// A replay at the start of the delve.
    pub fn new(campaign: &Campaign) -> Self {
        let initial: BTreeMap<String, i64> = campaign
            .quests
            .content
            .state
            .iter()
            .map(|s| (s.id.as_str().to_string(), i64::from(s.initial)))
            .collect();
        // Every datum a loop writes — its `counts`, or a write in its `on_cross`.
        let mut by_loops: BTreeSet<String> = BTreeSet::new();
        for l in &campaign.quests.content.loops {
            if let Some(c) = &l.counts {
                by_loops.insert(c.as_str().to_string());
            }
            for e in &l.on_cross {
                e.visit_deep(&mut |x| {
                    if let Some((id, _)) = x.writes_state() {
                        by_loops.insert(id.as_str().to_string());
                    }
                });
            }
        }
        // …minus every datum anything else writes.
        let mut by_others: BTreeSet<String> = BTreeSet::new();
        crate::compiler::plan::for_each_effect_root(campaign, &mut |site, effs| {
            if matches!(site.root, crate::compiler::plan::EffectRoot::LoopCross(_)) {
                return;
            }
            for e in effs {
                e.visit_deep(&mut |x| {
                    if let Some((id, _)) = x.writes_state() {
                        by_others.insert(id.as_str().to_string());
                    }
                });
            }
        });
        for s in &campaign.quests.content.stakes {
            by_others.insert(s.state.as_str().to_string());
        }
        let owned: BTreeSet<String> = by_loops
            .difference(&by_others)
            .filter(|id| initial.contains_key(*id))
            .cloned()
            .collect();
        let values = owned
            .iter()
            .map(|id| (id.clone(), initial.get(id).copied().unwrap_or(0)))
            .collect();
        LoopReplay {
            owned,
            values,
            initial,
            flags: BTreeSet::new(),
        }
    }

    /// The gate state at a point of the path: the flow walk's flags and data
    /// there, with the loop-owned data and the `on_cross` flags laid over them.
    pub fn state(
        &self,
        flags: &BTreeSet<String>,
        data: &BTreeMap<String, Option<i64>>,
    ) -> GateState {
        let mut st = GateState {
            flags: flags.union(&self.flags).cloned().collect(),
            data: data.clone(),
        };
        for (id, v) in &self.values {
            st.data.insert(id.clone(), Some(*v));
        }
        st
    }

    /// Apply one effect list's writes under `st`, as one crossing's `on_cross`
    /// runs: an effect fires when its own gate is open, a reaction bundle never
    /// fires. Returns the indices of the top-level effects that fired.
    fn fire(
        &mut self,
        effs: &[QuestEffect],
        st: &mut GateState,
        anchors: &BTreeMap<(String, String), ResolvedAnchor>,
        writes: &mut Vec<(([i32; 3], [i32; 3]), RegionWrite)>,
    ) -> Vec<usize> {
        let mut fired = Vec::new();
        for (i, e) in effs.iter().enumerate() {
            let gate = e.gate();
            let requires: Vec<String> = gate
                .requires_flags
                .iter()
                .map(|f| f.as_str().to_string())
                .collect();
            let forbids: Vec<String> = gate
                .forbids_flags
                .iter()
                .map(|f| f.as_str().to_string())
                .collect();
            if st.open(&requires, &forbids, gate.requires_state) != Some(true) {
                continue;
            }
            fired.push(i);
            match &e.verb {
                Verb::SetFlag { flag, .. } => {
                    st.flags.insert(flag.as_str().to_string());
                    self.flags.insert(flag.as_str().to_string());
                }
                Verb::SetCheckpoint { .. } | Verb::Bonfire { .. } | Verb::BeginStealth { .. } => {
                    continue;
                }
                _ => {}
            }
            if let Some((id, w)) = e.writes_state() {
                let id = id.as_str().to_string();
                if self.owned.contains(&id) {
                    let init = self.initial.get(&id).copied().unwrap_or(0);
                    let cur = self.values.get(&id).copied().unwrap_or(init);
                    let next = match w {
                        StateWrite::Set(v) => i64::from(v),
                        StateWrite::Add(v) => cur + i64::from(v),
                        StateWrite::Clear => init,
                    };
                    self.values.insert(id.clone(), next);
                    st.data.insert(id, Some(next));
                } else {
                    st.data.insert(id, None);
                }
            }
            match (e.gate_region_write(), e.region_write()) {
                (Some((anchor, fills)), _) => {
                    if let Some((from, to, block)) =
                        crate::compiler::plan::gate_region_block_any(anchors, anchor.as_str())
                    {
                        writes.push((
                            (from, to),
                            if fills {
                                RegionWrite::of_block(&block)
                            } else {
                                RegionWrite::Unseal
                            },
                        ));
                    }
                }
                (_, Some((zone, block))) => {
                    if let Some(r) = crate::compiler::plan::zone_box_in(anchors, zone) {
                        writes.push((
                            r,
                            match block {
                                Some(b) => RegionWrite::of_block(b),
                                None => RegionWrite::Clear,
                            },
                        ));
                    }
                }
                _ => {}
            }
            for list in e.nested_effect_lists() {
                self.fire(list, st, anchors, writes);
            }
        }
        fired
    }

    /// **Exercise `l` from `st`**: cross it until its own count and `on_cross`
    /// writes shut its gate, up to [`EXERCISE_CROSSINGS_MAX`], and once when
    /// they never do (spec-0086 §5.2). The replay carries the writes forward.
    pub fn exercise(
        &mut self,
        l: &LoopPlan,
        st: &GateState,
        anchors: &BTreeMap<(String, String), ResolvedAnchor>,
    ) -> Exercise {
        // Find `times` on a scratch copy first, so a loop its own writes never
        // release is crossed once and no more.
        let mut probe = self.clone();
        let mut pst = st.clone();
        let mut times = None;
        for n in 1..=EXERCISE_CROSSINGS_MAX {
            probe.cross(l, &mut pst, anchors, &mut Vec::new());
            if pst.holds(l) == Some(false) {
                times = Some(n);
                break;
            }
        }
        let (times, releases) = match times {
            Some(n) => (n, true),
            None => (1, false),
        };
        let mut st = st.clone();
        let mut ex = Exercise {
            times,
            releases,
            ..Exercise::default()
        };
        for _ in 0..times {
            let fired = self.cross(l, &mut st, anchors, &mut ex.writes);
            ex.fired.push(fired);
        }
        ex
    }

    /// One crossing: the count first, then the answer.
    fn cross(
        &mut self,
        l: &LoopPlan,
        st: &mut GateState,
        anchors: &BTreeMap<(String, String), ResolvedAnchor>,
        writes: &mut Vec<(([i32; 3], [i32; 3]), RegionWrite)>,
    ) -> Vec<usize> {
        if let Some(c) = &l.counts {
            if self.owned.contains(c) {
                let init = self.initial.get(c).copied().unwrap_or(0);
                let next = self.values.get(c).copied().unwrap_or(init) + 1;
                self.values.insert(c.clone(), next);
                st.data.insert(c.clone(), Some(next));
            } else {
                st.data.insert(c.clone(), None);
            }
        }
        self.fire(&l.on_cross, st, anchors, writes)
    }
}

/// The seal's state along a path, as region events (spec-0086 §5.1): a
/// [`RegionWrite::Hold`] over the slab at each step the gate opens — step `0`
/// when it holds from the start — and a [`RegionWrite::Unseal`] at each step it
/// shuts. `holds` is the gate at each step, after that step; an undatable gate
/// holds.
pub fn seal_events(l: &LoopPlan, holds: &[Option<bool>], terms: &[String]) -> Vec<RegionEvent> {
    let mut out = Vec::new();
    let mut prev = false;
    for (k, h) in holds.iter().enumerate() {
        let cur = *h != Some(false);
        if cur != prev || (k == 0 && cur) {
            out.push(if cur {
                let term = terms.get(k).map(String::as_str).unwrap_or("");
                RegionEvent::held(
                    l.slab,
                    k,
                    format!(
                        "the slab of loop `{}` ([{}, {}, {}]..[{}, {}, {}]), which holds from \
                         critical-path step {k}: read there, its gate term {term} is open",
                        l.id,
                        l.slab.0[0],
                        l.slab.0[1],
                        l.slab.0[2],
                        l.slab.1[0],
                        l.slab.1[1],
                        l.slab.1[2],
                    ),
                )
            } else {
                RegionEvent::forced(l.slab, RegionWrite::Unseal, k)
            });
        }
        prev = cur;
    }
    out
}

/// The exercise step's record, carried on the critical path beside the step
/// itself so the route proof and the ledger read one answer.
#[derive(Clone, Debug)]
pub struct ExerciseRecord {
    /// The loop exercised.
    pub r#loop: String,
    /// The step's index on its path.
    pub step: usize,
    /// How many crossings it makes.
    pub times: u32,
    /// Whether its own writes release the loop.
    pub releases: bool,
    /// The top-level `on_cross` effects each crossing fired.
    pub fired: Vec<Vec<usize>>,
}

// ---------------------------------------------------------------------------
// The splice: the exercise step and the seal, built beside the critical path
// ---------------------------------------------------------------------------

/// What [`LoopSplice::finish`] hands the critical path: the seal and exercise
/// region events, in the path's own step space, and one record per exercise.
#[derive(Clone, Debug, Default)]
pub struct Spliced {
    /// The slab seals (spec-0086 §5.1) and every exercise's forced writes.
    pub events: Vec<RegionEvent>,
    /// One record per exercise step, in path order.
    pub exercises: Vec<ExerciseRecord>,
    /// Per loop (declaration order), the gate at each step of the path, after
    /// that step; `None` is undatable.
    pub holds: Vec<Vec<Option<bool>>>,
    /// Per loop, beside `holds`: the gate term that holds it at each step, in
    /// words — what a route failure names.
    pub terms: Vec<Vec<String>>,
}

/// The loop half of building one path (spec-0086 §5.2): it follows the party
/// step by step, and in front of a step whose leg crosses a holding slab it
/// splices the exercise step. Empty — and inert — for a campaign that declares
/// no loop, so such a path is built byte-identically.
pub struct LoopSplice {
    loops: Vec<LoopPlan>,
    replay: LoopReplay,
    /// Where the party stands, and in which area.
    party: Option<(String, [i32; 3])>,
    /// The flow walk's flags and data where the party stands.
    flags: BTreeSet<String>,
    data: BTreeMap<String, Option<i64>>,
    spliced: Spliced,
    writes: Vec<(usize, ([i32; 3], [i32; 3]), RegionWrite)>,
}

impl LoopSplice {
    /// Start a path at `start`, with the flow walk's state before any step.
    pub fn new(
        campaign: &Campaign,
        anchors: &BTreeMap<(String, String), ResolvedAnchor>,
        start: Option<(String, [i32; 3])>,
        flags: BTreeSet<String>,
        data: BTreeMap<String, Option<i64>>,
    ) -> Self {
        let loops = resolve(campaign, anchors);
        let n = loops.len();
        LoopSplice {
            loops,
            replay: LoopReplay::new(campaign),
            party: start,
            flags,
            data,
            spliced: Spliced {
                holds: vec![Vec::new(); n],
                terms: vec![Vec::new(); n],
                ..Spliced::default()
            },
            writes: Vec::new(),
        }
    }

    /// Whether there is anything to splice at all.
    pub fn is_empty(&self) -> bool {
        self.loops.is_empty()
    }

    /// The flow walk has moved on: this is its state after the step just taken.
    pub fn advance(&mut self, flags: BTreeSet<String>, data: BTreeMap<String, Option<i64>>) {
        self.flags = flags;
        self.data = data;
    }

    /// Record the gate at the step just pushed (index `steps.len() - 1`).
    pub fn record(&mut self) {
        let st = self.replay.state(&self.flags, &self.data);
        for (i, l) in self.loops.iter().enumerate() {
            self.spliced.holds[i].push(st.holds(l));
            self.spliced.terms[i].push(st.open_term(l));
        }
    }

    /// The party now stands at `pos` in `area`.
    pub fn stand(&mut self, area: &str, pos: [i32; 3]) {
        self.party = Some((area.to_string(), pos));
    }

    /// **Before a step at `next` in `area`**: splice an exercise step for every
    /// loop whose slab the leg there crosses while it holds — the party stands on
    /// the slab's approach side and `next` lies beyond its plane, in the loop's
    /// own area. A leg out of another area starts at that area's `entry`, where
    /// the crossing puts the party down. Each spliced step is pushed onto
    /// `steps`, keyed into `acts` by its loop id, and recorded.
    pub fn before(
        &mut self,
        steps: &mut Vec<crate::compiler::plan::Step>,
        acts: &mut BTreeMap<String, usize>,
        next: [i32; 3],
        area: &str,
        entry: Option<[i32; 3]>,
        anchors: &BTreeMap<(String, String), ResolvedAnchor>,
    ) {
        for li in 0..self.loops.len() {
            let l = &self.loops[li];
            if l.area != area || l.axis().is_none() {
                continue;
            }
            let from = match &self.party {
                Some((a, p)) if a == area => Some(*p),
                _ => entry,
            };
            let Some(from) = from else {
                continue;
            };
            if !(l.on_approach(from) && l.beyond(next)) {
                continue;
            }
            let st = self.replay.state(&self.flags, &self.data);
            if st.holds(l) == Some(false) {
                continue;
            }
            let l = l.clone();
            let ex = self.replay.exercise(&l, &st, anchors);
            let idx = steps.len();
            steps.push(crate::compiler::plan::Step::Loop {
                loop_id: l.id.clone(),
                pos: l.transport(),
                cross: l.anchor_cell,
                offset: l.offset,
                times: ex.times,
                transport: l.transport(),
            });
            acts.insert(l.id.clone(), idx);
            for (r, w) in &ex.writes {
                self.writes.push((idx, *r, *w));
            }
            self.spliced.exercises.push(ExerciseRecord {
                r#loop: l.id.clone(),
                step: idx,
                times: ex.times,
                releases: ex.releases,
                fired: ex.fired,
            });
            self.record();
            self.party = Some((area.to_string(), l.transport()));
        }
    }

    /// The path is built: the seal events from each loop's gate along it, and
    /// every exercise's forced writes at its own step.
    pub fn finish(mut self) -> Spliced {
        for (i, l) in self.loops.iter().enumerate() {
            self.spliced.events.extend(seal_events(
                l,
                &self.spliced.holds[i],
                &self.spliced.terms[i],
            ));
        }
        for (idx, r, w) in self.writes {
            self.spliced.events.push(RegionEvent::forced(r, w, idx));
        }
        self.spliced
    }
}
