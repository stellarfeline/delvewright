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
use crate::compiler::timeline::Region;

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

/// What the periodic span reads fog from (spec-0086 §4.3 × spec-0080): the
/// biome map in every configuration the route can stand under — the first
/// tick's, then after each `set-atmosphere` repaint in route order, accumulated
/// as spec-0086 §4.6 accumulates block writes — and each declared atmosphere's
/// `visual/fog_end_distance`. A biome no atmosphere declares reads the
/// attribute's default.
pub struct Fog {
    /// `(label, map)` per configuration; the first is the first tick's.
    states: Vec<(String, crate::compiler::horizon::BiomeMap)>,
    /// Atmosphere biome id → its fog end, for every atmosphere that sets one.
    ends: BTreeMap<String, f64>,
}

impl Fog {
    /// The fog of a planned campaign: its biome map ([`crate::compiler::horizon::biome_map`],
    /// the one authority for which biome a cell stands in), every repaint a
    /// route step can stand after ([`crate::compiler::horizon::repaint_volume`]),
    /// and every declared atmosphere's fog end.
    pub fn of(plan: &crate::compiler::plan::Plan<'_>) -> Self {
        let ends = plan
            .campaign
            .world
            .content
            .atmospheres
            .iter()
            .filter_map(|a| {
                let v = a.attributes.iter().find_map(|(k, v)| {
                    (crate::compiler::atmosphere::canonical_id(k) == "visual/fog_end_distance")
                        .then_some(v)
                })?;
                Some((
                    crate::compiler::atmosphere::biome_id(&plan.namespace, a.id.as_str()),
                    fog_end_value(v),
                ))
            })
            .collect();
        let first = crate::compiler::horizon::biome_map(plan);
        // Every repaint, at the step its root fires (a root with no step of its
        // own — a trigger, a trap — at step 0, meeting every configuration),
        // in route order then declaration order (ADR-0006).
        let mut repaints: Vec<(usize, String, Region, String)> = Vec::new();
        crate::compiler::plan::for_each_gate_effect(plan.campaign, &mut |site, e| {
            let Verb::SetAtmosphere { atmosphere, .. } = &e.verb else {
                return;
            };
            let Some((lo, hi)) = crate::compiler::horizon::repaint_volume(plan, e) else {
                return;
            };
            let biome = first.biome_of(atmosphere.as_ref().map(|a| a.as_str()));
            repaints.push((
                crate::compiler::plan::root_step(plan, &site.root),
                format!(
                    "after the `set-atmosphere` at `{}` (critical-path step {})",
                    site.path,
                    crate::compiler::plan::root_step(plan, &site.root)
                ),
                crate::compiler::atmosphere::painted_box(lo, hi),
                biome,
            ));
        });
        repaints.sort_by_key(|r| r.0);
        let mut states = vec![("at the first tick".to_string(), first)];
        for (_, label, cells, biome) in repaints {
            let next = states.last().expect("the first tick's state").1.repainted(
                cells,
                &biome,
                crate::compiler::horizon::PaintSource::Band,
            );
            states.push((label, next));
        }
        Fog { states, ends }
    }
}

/// An atmosphere's `visual/fog_end_distance` as the client resolves it over
/// the default: a plain number, or vanilla's float modifier form applied to
/// [`FOG_END_DEFAULT`]. A form `DW0928` would refuse reads the default, the
/// largest the span can be asked to close against.
fn fog_end_value(v: &serde_json::Value) -> f64 {
    if let Some(x) = v.as_f64() {
        return x;
    }
    let arg = v.get("argument").and_then(serde_json::Value::as_f64);
    let d = FOG_END_DEFAULT;
    match (v.get("modifier").and_then(serde_json::Value::as_str), arg) {
        (Some("override"), Some(a)) => a,
        (Some("add"), Some(a)) => d + a,
        (Some("subtract"), Some(a)) => d - a,
        (Some("multiply"), Some(a)) => d * a,
        (Some("minimum"), Some(a)) => d.min(a),
        (Some("maximum"), Some(a)) => d.max(a),
        _ => d,
    }
}

/// The fog end the client reads at `eye` (spec-0086 §4.3): the kernel-weighted
/// mean of every nearby biome's `visual/fog_end_distance`, each biome weighed by
/// [`crate::compiler::horizon::camera_mix`] over the painted biome map — the
/// port [`crate::compiler::horizon::camera_weight`] reads. The one site the
/// periodic span reads fog at, in configuration `state` of [`Fog`]; with no
/// atmosphere painted, every biome reads the attribute's default.
pub fn fog_end_at(fog: &Fog, state: usize, eye: [f64; 3]) -> f64 {
    let Some((_, map)) = fog.states.get(state) else {
        return FOG_END_DEFAULT;
    };
    crate::compiler::horizon::camera_mix(map, eye)
        .iter()
        .map(|(biome, w)| w * fog.ends.get(biome).copied().unwrap_or(FOG_END_DEFAULT))
        .sum()
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
    /// This loop's gate as the region model reads one (spec-0086 §5.1 ×
    /// spec-0088 §4.1): the three axes as declared, and the gate's term
    /// reduction ([`crate::compiler::plan::gate_terms_of`]) over the declared
    /// loop's own gate.
    pub fn staged_gate(&self, campaign: &Campaign) -> crate::compiler::plan::StagedGate {
        let terms = campaign
            .quests
            .content
            .loops
            .iter()
            .find(|d| d.id.as_str() == self.id)
            .map(|d| crate::compiler::plan::gate_terms_of(campaign, d.gate()))
            .unwrap_or_default();
        crate::compiler::plan::StagedGate {
            requires_flags: self.requires_flags.clone(),
            forbids_flags: self.forbids_flags.clone(),
            requires_state: self.requires_state.clone(),
            terms,
        }
    }

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

    /// The slab cell the exercise step crosses at: the slab's lowest course in
    /// its anchor's column — the feet cell of a slab drawn over a passage's
    /// open cross-section — for a horizontal crossing, and the anchor cell for a
    /// vertical one.
    pub fn cross(&self) -> [i32; 3] {
        match self.axis() {
            Some(1) | None => self.anchor_cell,
            Some(_) => [self.anchor_cell[0], self.slab.0[1], self.anchor_cell[2]],
        }
    }

    /// The landing cell the exercise step names: where every crossing at
    /// [`Self::cross`] puts the body down.
    pub fn transport(&self) -> [i32; 3] {
        shift(self.cross(), self.offset)
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

    /// Whether `cell` lies beyond the slab's plane **in line with it**: on the
    /// far side from the approach, and inside the slab's own cross-section
    /// widened by that cross-section's width on each other axis — where a body
    /// arrives by passing through the slab, as opposed to somewhere else on the
    /// far side of an infinite plane.
    pub fn beyond_in_line(&self, cell: [i32; 3]) -> bool {
        let Some(a) = self.axis() else {
            return false;
        };
        self.beyond(cell)
            && (0..3).filter(|&i| i != a).all(|i| {
                let w = self.slab.1[i] - self.slab.0[i] + 1;
                self.slab.0[i] - w <= cell[i] && cell[i] <= self.slab.1[i] + w
            })
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
    pub writes: Vec<(Region, RegionWrite)>,
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
        writes: &mut Vec<(Region, RegionWrite)>,
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
        writes: &mut Vec<(Region, RegionWrite)>,
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
    /// Every loop-owned datum's value after the step (spec-0086 §5.2): what the
    /// region model's datum replay reads from here on (spec-0088 §4.1), since
    /// nothing but a loop writes these.
    pub owned_after: BTreeMap<String, i64>,
    /// Every flag an exercise's `on_cross` has set by the end of the step,
    /// credited to the region model as set, forced, at the step.
    pub flags_after: Vec<String>,
}

// ---------------------------------------------------------------------------
// The splice: the exercise step and the seal, built beside the critical path
// ---------------------------------------------------------------------------

/// What [`LoopSplice::finish`] hands the critical path: the seal and exercise
/// region events, in the path's own step space, and one record per exercise.
#[derive(Clone, Debug, Default)]
pub struct Spliced {
    /// Every exercise's forced region writes. The slab itself is not an event:
    /// whether it holds is read off its gate per configuration by the region
    /// model ([`crate::compiler::nav::liveness_of`]), from the datum replay this
    /// record dates (`exercises`).
    pub events: Vec<RegionEvent>,
    /// The data only loops write ([`LoopReplay`]).
    pub owned: BTreeSet<String>,
    /// One record per exercise step, in path order.
    pub exercises: Vec<ExerciseRecord>,
    /// Per loop (declaration order), the gate at each step of the path, after
    /// that step; `None` is undatable.
    pub holds: Vec<Vec<Option<bool>>>,
    /// Per loop, beside `holds`: the gate term that holds it at each step, in
    /// words — what a route failure names.
    pub terms: Vec<Vec<String>>,
}

impl Spliced {
    /// Re-index into a path where steps were spliced in front of others
    /// (spec-0083 §3.4, a taken link): `moved[k]` is old step `k`'s new index
    /// and `new_len` the new path's length. The seals and exercise writes fire
    /// at their own step's new index, and each exercise record names its
    /// step's new index. `holds` and `terms` are indexed by gate reading, not
    /// by step, and do not move.
    pub fn reindex(&mut self, moved: &[usize], new_len: usize) {
        let shift = new_len.saturating_sub(moved.len());
        let at = |k: usize| moved.get(k).copied().unwrap_or(k + shift);
        for e in &mut self.events {
            e.fire_step = at(e.fire_step);
        }
        for x in &mut self.exercises {
            x.step = at(x.step);
        }
    }
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
    writes: Vec<(usize, Region, RegionWrite)>,
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
    /// loop whose slab the leg there crosses while it holds — `next` lies beyond
    /// the slab's plane in line with the slab ([`LoopPlan::beyond_in_line`]) and
    /// the party does not already stand there, in the loop's own area. This is read
    /// before a block is placed, so it is a reading of the plan, not of the
    /// span; a leg it misses is still judged by the route proof, which refuses
    /// it naming the loop (`DW0311`). A leg out of another area starts at that area's `entry`, where
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
            if l.beyond_in_line(from) || !l.beyond_in_line(next) {
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
                cross: l.cross(),
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
                owned_after: self.replay.values.clone(),
                flags_after: self.replay.flags.iter().cloned().collect(),
            });
            self.record();
            self.party = Some((area.to_string(), l.transport()));
        }
    }

    /// The path is built: the seal events from each loop's gate along it, and
    /// every exercise's forced writes at its own step.
    pub fn finish(mut self) -> Spliced {
        self.spliced.owned = self.replay.owned.clone();
        for (idx, r, w) in self.writes {
            self.spliced.events.push(RegionEvent::forced(r, w, idx));
        }
        self.spliced
    }
}

// ---------------------------------------------------------------------------
// The world half: the slab, the span, the tiling, the bodies (spec-0086 §4)
// ---------------------------------------------------------------------------

/// What one loop's world proof examined — one row of `validation/loop-gate.json`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LoopRow {
    /// The loop.
    pub id: String,
    /// Its offset `d`.
    pub offset: [i32; 3],
    /// The crossing axis (`x`, `y`, `z`), when the slab is one.
    pub axis: Option<usize>,
    /// The slab's cells.
    pub slab_cells: usize,
    /// The eyes the span was grown from.
    pub eyes: usize,
    /// The least and greatest fog end an eye reads.
    pub fog: (f64, f64),
    /// The grown periodic span, when it closed.
    pub span: Option<([i32; 3], [i32; 3])>,
    /// Its cells.
    pub span_cells: usize,
    /// Growth steps to a closed view.
    pub steps: usize,
    /// Boundary cells of the final span closed by geometry.
    pub closed_geometry: usize,
    /// Boundary cells closed by fog.
    pub closed_fog: usize,
    /// Faces left open: zero on a pass, the refusal's count otherwise.
    pub open_faces: usize,
    /// Cells in sight of an eye inside the span.
    pub visible: usize,
    /// Configurations the block and light comparison ran over.
    pub configurations: usize,
    /// Declared volumes intersecting the span.
    pub volumes: usize,
    /// Compiler-placed bodies found in the span.
    pub bodies: usize,
    /// Exercise steps on the default path, with their `times` and step index.
    pub exercises: Vec<(usize, u32)>,
}

/// What the loop proofs examined, over every loop (spec-0086 §8).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LoopBinding {
    /// One row per resolved loop, declaration order.
    pub rows: Vec<LoopRow>,
}

impl LoopBinding {
    fn sum(&self, f: impl Fn(&LoopRow) -> usize) -> usize {
        self.rows.iter().map(f).sum()
    }

    /// The binding line, printed on every build that declares a loop.
    pub fn line(&self) -> String {
        let (lo, hi) = self.rows.iter().fold((f64::INFINITY, 0.0f64), |(a, b), r| {
            if r.eyes == 0 {
                (a, b)
            } else {
                (a.min(r.fog.0), b.max(r.fog.1))
            }
        });
        let fog = if lo.is_finite() {
            format!("{lo}..{hi}")
        } else {
            "none".to_string()
        };
        let met = self.rows.iter().filter(|r| !r.exercises.is_empty()).count();
        format!(
            "loop binding: {} loop(s); slab cells {}; eyes {} (fog end {fog} blocks as the kernel \
             reads it); span {} cells grown to a closed view in {} steps, boundary cells closed by \
             geometry {} and by fog {}, open faces {}; visible cells {} compared as blocks and as \
             light at 2 skies over {} configuration(s); volumes in span {}, bodies in span {}; \
             forced route meets {} of {} holding, exercise steps {}",
            self.rows.len(),
            self.sum(|r| r.slab_cells),
            self.sum(|r| r.eyes),
            self.sum(|r| r.span_cells),
            self.sum(|r| r.steps),
            self.sum(|r| r.closed_geometry),
            self.sum(|r| r.closed_fog),
            self.sum(|r| r.open_faces),
            self.sum(|r| r.visible),
            self.sum(|r| r.configurations),
            self.sum(|r| r.volumes),
            self.sum(|r| r.bodies),
            met,
            self.rows.len(),
            self.sum(|r| r.exercises.len()),
        )
    }

    /// `validation/loop-gate.json`.
    pub fn to_json(&self) -> serde_json::Value {
        let axis = |a: Option<usize>| a.map(|a| ["x", "y", "z"][a]);
        serde_json::json!({
            "spec": "spec-0086",
            "loops": self.rows.len(),
            "span_reach": SPAN_REACH,
            "rows": self.rows.iter().map(|r| serde_json::json!({
                "id": r.id,
                "offset": r.offset,
                "axis": axis(r.axis),
                "slab_cells": r.slab_cells,
                "eyes": r.eyes,
                "fog_end": [r.fog.0, r.fog.1],
                "span": r.span.map(|(a, b)| [a, b]),
                "span_cells": r.span_cells,
                "steps": r.steps,
                "closed_by_geometry": r.closed_geometry,
                "closed_by_fog": r.closed_fog,
                "open_faces": r.open_faces,
                "visible": r.visible,
                "skies": 2,
                "configurations": r.configurations,
                "volumes_in_span": r.volumes,
                "bodies_in_span": r.bodies,
                "exercise": r.exercises.iter().map(|(step, times)| serde_json::json!({
                    "step": step, "times": times
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
            "unchecked": [
                "a client frame: whether a client shows anything at the seam is a client fact; \
                 the engine proves the packet relative and the view identical",
                "particles and positional sounds alive in the span at the move",
                "items on the ground and projectiles",
                "a witness: a second player in sight of the mover sees the body jump",
                "chunk streaming at the far ring, and a client render distance below the server's",
            ],
        })
    }
}

/// The eye points of every landing cell a body can stand in (spec-0086 §4.3):
/// the player's eye over its feet at the cell centre and toward each horizontal
/// corner of the hitbox.
fn eyes(l: &LoopPlan, world: &crate::compiler::nav::World) -> Vec<[f64; 3]> {
    let (lo, hi) = l.landing();
    let half = delvewright_dsl::metrics::PLAYER_WIDTH / 2.0;
    let mut out = Vec::new();
    for x in lo[0]..=hi[0] {
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                let c = [x, y, z];
                if !world.is_standable(c) {
                    continue;
                }
                let ey = world.feet_y(c) + delvewright_dsl::metrics::PLAYER_EYE_HEIGHT;
                let (cx, cz) = (f64::from(x) + 0.5, f64::from(z) + 0.5);
                out.push([cx, ey, cz]);
                for (dx, dz) in [(-half, -half), (-half, half), (half, -half), (half, half)] {
                    out.push([cx + dx, ey, cz + dz]);
                }
            }
        }
    }
    out
}

/// The nine points of `c` a sightline may end on: its centre and its eight
/// corners, each drawn a hair inside the cell so the segment ends in `c`.
fn targets(c: [i32; 3]) -> [[f64; 3]; 9] {
    let m = [
        f64::from(c[0]) + 0.5,
        f64::from(c[1]) + 0.5,
        f64::from(c[2]) + 0.5,
    ];
    let mut out = [m; 9];
    let e = 1e-3;
    let mut k = 1;
    for dx in [e, 1.0 - e] {
        for dy in [e, 1.0 - e] {
            for dz in [e, 1.0 - e] {
                out[k] = [
                    f64::from(c[0]) + dx,
                    f64::from(c[1]) + dy,
                    f64::from(c[2]) + dz,
                ];
                k += 1;
            }
        }
    }
    out
}

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// How an eye sees a cell.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Sight {
    /// No sightline reaches it.
    Hidden,
    /// A sightline reaches it, past the eye's fog end.
    Fogged,
    /// A sightline reaches it inside the fog.
    Seen,
}

/// Whether any eye sees `c` (spec-0086 §4.3): a segment from the eye to `c`'s
/// centre or a corner, walked by [`crate::compiler::nav::walk_cells`], meets no
/// cell [`crate::compiler::nav::World::blocks_camera`] holds for before `c`.
/// Returns the best answer over every eye, and the eye that gave it.
fn sight(
    world: &crate::compiler::nav::World,
    eyes: &[[f64; 3]],
    fogs: &[f64],
    c: [i32; 3],
) -> (Sight, Option<[f64; 3]>) {
    let mut best = (Sight::Hidden, None);
    for (k, &eye) in eyes.iter().enumerate() {
        let fog = fogs[k];
        for t in targets(c) {
            let blocked = crate::compiler::nav::walk_cells(eye, t, |cell| {
                cell != c && world.blocks_camera(cell)
            })
            .is_some();
            if blocked {
                continue;
            }
            if dist(eye, t) < fog {
                return (Sight::Seen, Some(eye));
            }
            best = (Sight::Fogged, Some(eye));
        }
    }
    best
}

/// The cells on one face of box `b`: face `2a` is the low side of axis `a`,
/// `2a + 1` the high side.
fn face_cells(b: ([i32; 3], [i32; 3]), face: usize) -> Vec<[i32; 3]> {
    let a = face / 2;
    let v = if face.is_multiple_of(2) {
        b.0[a]
    } else {
        b.1[a]
    };
    let mut out = Vec::new();
    for x in b.0[0]..=b.1[0] {
        for y in b.0[1]..=b.1[1] {
            for z in b.0[2]..=b.1[2] {
                let c = [x, y, z];
                if c[a] == v {
                    out.push(c);
                }
            }
        }
    }
    out
}

const FACE_WORDS: [&str; 6] = ["west", "east", "down", "up", "north", "south"];

/// The grown span, or why it did not close.
struct Span {
    b: ([i32; 3], [i32; 3]),
    steps: usize,
    closed_geometry: usize,
    closed_fog: usize,
}

/// Grow the periodic span from the slab and its landing until no open boundary
/// cell is in sight of an eye inside its fog (spec-0086 §4.3).
fn grow(
    l: &LoopPlan,
    world: &crate::compiler::nav::World,
    eyes: &[[f64; 3]],
    fogs: &[f64],
    built: &[([i32; 3], [i32; 3])],
) -> Result<Span, String> {
    let fog_of = |eye: [f64; 3]| {
        eyes.iter()
            .position(|e| *e == eye)
            .map_or(FOG_END_DEFAULT, |k| fogs[k])
    };
    let (llo, lhi) = l.landing();
    let mut b = (
        [
            l.slab.0[0].min(llo[0]),
            l.slab.0[1].min(llo[1]),
            l.slab.0[2].min(llo[2]),
        ],
        [
            l.slab.1[0].max(lhi[0]),
            l.slab.1[1].max(lhi[1]),
            l.slab.1[2].max(lhi[2]),
        ],
    );
    let in_built = |c: [i32; 3]| built.iter().any(|bx| inside(*bx, c));
    let mut steps = 0usize;
    loop {
        steps += 1;
        let mut grew = false;
        for (face, face_word) in FACE_WORDS.iter().enumerate() {
            for c in face_cells(b, face) {
                if world.blocks_camera(c) {
                    continue;
                }
                let (s, eye) = sight(world, eyes, fogs, c);
                if s != Sight::Seen {
                    continue;
                }
                let eye = eye.expect("a seen cell names its eye");
                if !in_built(c) {
                    // Above a built column is the sky; anywhere else is the void
                    // beside the build.
                    let above = built.iter().any(|bx| {
                        (bx.0[0]..=bx.1[0]).contains(&c[0])
                            && (bx.0[2]..=bx.1[2]).contains(&c[2])
                            && c[1] > bx.1[1]
                    });
                    let word = if above {
                        "up, into open sky above the built volume"
                    } else {
                        face_word
                    };
                    return Err(format!(
                        "the eye at [{:.2}, {:.2}, {:.2}] (fog end {} blocks) sees the open cell \
                         [{}, {}, {}] on the span's {word} face, outside every placed piece — the \
                         view leaves the built volume, and nothing outside it moves with the body",
                        eye[0],
                        eye[1],
                        eye[2],
                        fog_of(eye),
                        c[0],
                        c[1],
                        c[2]
                    ));
                }
                let a = face / 2;
                if face % 2 == 0 {
                    b.0[a] -= 1;
                } else {
                    b.1[a] += 1;
                }
                if b.0[a] < llo[a] - SPAN_REACH || b.1[a] > lhi[a] + SPAN_REACH {
                    return Err(format!(
                        "the eye at [{:.2}, {:.2}, {:.2}] (fog end {} blocks) sees the open cell \
                         [{}, {}, {}] on the span's {} face, and the span has grown {SPAN_REACH} \
                         cells past the landing slab without the view closing",
                        eye[0],
                        eye[1],
                        eye[2],
                        fog_of(eye),
                        c[0],
                        c[1],
                        c[2],
                        face_word
                    ));
                }
                grew = true;
                break;
            }
        }
        if !grew {
            break;
        }
    }
    // What closed each boundary cell of the final span.
    let mut shell: BTreeSet<[i32; 3]> = BTreeSet::new();
    for face in 0..6 {
        shell.extend(face_cells(b, face));
    }
    let (mut geo, mut fog) = (0usize, 0usize);
    for c in shell {
        if world.blocks_camera(c) {
            geo += 1;
            continue;
        }
        match sight(world, eyes, fogs, c).0 {
            Sight::Fogged => fog += 1,
            _ => geo += 1,
        }
    }
    Ok(Span {
        b,
        steps,
        closed_geometry: geo,
        closed_fog: fog,
    })
}

/// One configuration of the comparison world: the writes applied, and what to
/// call it.
struct Config {
    label: String,
    writes: Vec<(Region, String)>,
}

/// Every runtime write a configuration may apply, in path order, as
/// `(step, label, region, block)` — air for a clear.
fn block_writes(plan: &crate::compiler::plan::Plan) -> Vec<(usize, String, Region, String)> {
    let mut out: Vec<(usize, String, Region, String)> = Vec::new();
    let air = "minecraft:air".to_string();
    crate::compiler::plan::for_each_gate_effect(plan.campaign, &mut |site, e| {
        // A loop the path exercises has its `on_cross` writes judged crossing
        // by crossing, at the step that makes them; one it does not exercise
        // may answer any body's crossing, so its writes are judged here, from
        // the start.
        if let crate::compiler::plan::EffectRoot::LoopCross(l) = site.root
            && plan
                .loop_exercises
                .iter()
                .any(|x| x.r#loop == l.id.as_str())
        {
            return;
        }
        let step = crate::compiler::plan::root_step(plan, &site.root);
        let label = format!("`{}` at `{}`", e.verb.tag(), site.path);
        match &e.verb {
            Verb::FillRegion { region, block, .. } => {
                if let Some(r) = plan.zone_box(region) {
                    out.push((step, label, r, block.clone()));
                }
            }
            Verb::ClearRegion { region, .. } => {
                if let Some(r) = plan.zone_box(region) {
                    out.push((step, label, r, air.clone()));
                }
            }
            Verb::SetBlock { anchor, block, .. } => {
                if let Some(p) = plan.point_any(anchor.as_str()) {
                    out.push((step, label, (p, p), block.clone()));
                }
            }
            Verb::CloseGate { anchor, .. } | Verb::OpenGate { anchor, .. } => {
                if let Some((from, to, block)) =
                    crate::compiler::plan::gate_region_block_any(&plan.anchors, anchor.as_str())
                {
                    let fills = matches!(e.verb, Verb::CloseGate { .. });
                    out.push((
                        step,
                        label,
                        (from, to),
                        if fills { block } else { air.clone() },
                    ));
                }
            }
            _ => {}
        }
    });
    // A timed gate's two phases are two configurations of its region.
    for g in &plan.timed_gates {
        out.push((
            0,
            format!("timed gate `{}` closed", g.id),
            g.gate_region,
            g.gate_block.clone(),
        ));
        out.push((
            0,
            format!("timed gate `{}` open", g.id),
            g.gate_region,
            air.clone(),
        ));
    }
    out.sort_by_key(|w| w.0);
    out
}

/// The world as shipped, before any runtime write: the assembled blocks, the
/// relight fixtures, and every gate the placed world authors shut.
fn shipped(
    plan: &crate::compiler::plan::Plan,
    blocks: &crate::compiler::blockstate::BlockMap,
    placements: &[crate::compiler::light::Placement],
    seals: &[crate::compiler::assembled::GateSeal],
    clip: ([i32; 3], [i32; 3]),
) -> BTreeMap<[i32; 3], String> {
    let mut m: BTreeMap<[i32; 3], String> = blocks
        .range(clip.0..=clip.1)
        .filter(|(c, _)| inside(clip, **c))
        .map(|(c, b)| (*c, b.as_str().to_string()))
        .collect();
    for p in placements {
        if inside(clip, p.pos) {
            m.insert(p.pos, p.block.clone());
        }
    }
    for s in seals.iter().filter(|s| s.sealed()) {
        if let Some((from, to, block)) =
            crate::compiler::plan::gate_region_block_any(&plan.anchors, &s.anchor)
        {
            write_box(&mut m, (from, to), &block, clip);
        }
    }
    m
}

fn write_box(
    m: &mut BTreeMap<[i32; 3], String>,
    r: ([i32; 3], [i32; 3]),
    block: &str,
    clip: ([i32; 3], [i32; 3]),
) {
    for x in r.0[0].min(r.1[0])..=r.0[0].max(r.1[0]) {
        for y in r.0[1].min(r.1[1])..=r.0[1].max(r.1[1]) {
            for z in r.0[2].min(r.1[2])..=r.0[2].max(r.1[2]) {
                let c = [x, y, z];
                if !inside(clip, c) {
                    continue;
                }
                if block == "minecraft:air" {
                    m.remove(&c);
                } else {
                    m.insert(c, block.to_string());
                }
            }
        }
    }
}

/// Everything [`check`] reads.
pub struct Inputs<'a> {
    /// The plan.
    pub plan: &'a crate::compiler::plan::Plan<'a>,
    /// The campaign's nav world (premises applied).
    pub world: &'a crate::compiler::nav::World,
    /// The assembled (edited) block map.
    pub blocks: &'a crate::compiler::blockstate::BlockMap,
    /// The relight pass's fixtures.
    pub placements: &'a [crate::compiler::light::Placement],
    /// The world-load gate seals.
    pub seals: &'a [crate::compiler::assembled::GateSeal],
    /// Where the seating pass put each wave's bodies.
    pub wave_seats: &'a BTreeMap<String, Vec<[i32; 3]>>,
}

/// **The loop's world proofs** (spec-0086 §4): the slab (`DW0945`), the closed
/// view (`DW0947`), the bodies (`DW0948`) and the tiling (`DW0946`), loop by
/// loop. The binding is returned beside the first refusal so the caller prints
/// it either way.
pub fn check(i: &Inputs<'_>) -> (LoopBinding, Option<crate::compiler::failure::Failure>) {
    use crate::compiler::failure::Failure;
    let plan = i.plan;
    let mut binding = LoopBinding::default();
    let built: Vec<([i32; 3], [i32; 3])> = plan.placed_pieces().map(|p| p.bbox()).collect();
    let writes = block_writes(plan);
    let (times, weathers) = crate::compiler::light::reachable_time_weather(plan.campaign);
    let dark = crate::compiler::light::darkest_effective_sky(plan.campaign);
    let mut bright = 0u8;
    for &t in &times {
        for &w in &weathers {
            bright = bright.max(crate::compiler::light::effective_sky(t, w));
        }
    }
    let skies = [("darkest", dark), ("brightest", bright)];
    let mut refusal: Option<Failure> = None;
    let fog = Fog::of(plan);
    for l in &plan.loops {
        let mut row = LoopRow {
            id: l.id.clone(),
            offset: l.offset,
            axis: l.axis(),
            slab_cells: box_cells(l.slab),
            exercises: plan
                .loop_exercises
                .iter()
                .filter(|e| e.r#loop == l.id)
                .map(|e| (e.step, e.times))
                .collect(),
            ..LoopRow::default()
        };
        let result = check_one(i, l, &built, &writes, &skies, &fog, &mut row);
        binding.rows.push(row);
        if let Err(f) = result
            && refusal.is_none()
        {
            refusal = Some(f);
        }
    }
    (binding, refusal)
}

fn box_cells(b: ([i32; 3], [i32; 3])) -> usize {
    (0..3)
        .map(|a| (b.1[a] - b.0[a] + 1).max(0) as usize)
        .product()
}

fn cell_words(c: [i32; 3]) -> String {
    format!("[{}, {}, {}]", c[0], c[1], c[2])
}

fn box_words(b: ([i32; 3], [i32; 3])) -> String {
    format!("{}..{}", cell_words(b.0), cell_words(b.1))
}

fn check_one(
    i: &Inputs<'_>,
    l: &LoopPlan,
    built: &[([i32; 3], [i32; 3])],
    writes: &[(usize, String, Region, String)],
    skies: &[(&str, u8); 2],
    fog: &Fog,
    row: &mut LoopRow,
) -> Result<(), crate::compiler::failure::Failure> {
    use crate::compiler::failure::Failure;
    let plan = i.plan;
    let world = i.world;
    let slab_fault = |fault: String| Failure {
        code: DW_LOOP_SLAB,
        message: format!(
            "loop `{}`: its slab {} with offset [{}, {}, {}] is not a slab the engine can poll — \
             {fault}. Reshape the slab or move the landing (`to`).",
            l.id,
            box_words(l.slab),
            l.offset[0],
            l.offset[1],
            l.offset[2]
        ),
    };
    // ---- §4.2 the slab ----
    let Some(a) = l.axis() else {
        return Err(slab_fault(if l.offset == [0, 0, 0] {
            "the offset is zero (`to` names the slab's own anchor cell), so the body would be \
             moved nowhere: d[a] = 0 on every axis"
                .to_string()
        } else {
            "the offset moves along more than one axis, so a body is not returned through the \
             plane it crossed: a slab is crossed along one axis, and the landing lies straight \
             back along it"
                .to_string()
        }));
    };
    let t = l.thickness(a);
    if l.offset[a].abs() < t {
        return Err(slab_fault(format!(
            "the move does not clear the slab: |d[{ax}]| = {} is less than the slab's thickness \
             t = {t} along {ax}, so a moved body is still in the slab next tick and is moved again \
             every tick it stands there",
            l.offset[a].abs(),
            ax = ["x", "y", "z"][a]
        )));
    }
    let (speed, reach, what) = if a == 1 {
        (
            delvewright_dsl::metrics::POLL_FALL_BLOCKS_PER_TICK,
            delvewright_dsl::metrics::PLAYER_HEIGHT,
            "a falling body's limit speed (`metrics::POLL_FALL_BLOCKS_PER_TICK`, the fall law's \
             fixed point)",
        )
    } else {
        (
            delvewright_dsl::metrics::POLL_HORIZONTAL_BLOCKS_PER_TICK,
            delvewright_dsl::metrics::PLAYER_WIDTH,
            "the fastest horizontal tick a body has (`metrics::POLL_HORIZONTAL_BLOCKS_PER_TICK`, \
             sprint-jumping)",
        )
    };
    if speed >= f64::from(t) + reach {
        return Err(slab_fault(format!(
            "it is too thin to catch a crossing along {ax}: a one-tick poll sees a body in the \
             slab for a window of t + reach = {t} + {reach} = {window} blocks, and {what} is \
             {speed} blocks a tick, so a body can pass through between two polls. Thicken the \
             slab to at least {need} cells along {ax}",
            ax = ["x", "y", "z"][a],
            window = f64::from(t) + reach,
            need = (speed - reach).floor() as i32 + 1,
        )));
    }
    for other in &plan.loops {
        if other.id == l.id {
            continue;
        }
        if boxes_meet(l.slab, other.slab) || boxes_meet(l.slab, other.landing()) {
            return Err(slab_fault(format!(
                "it overlaps loop `{}`'s slab or landing ({} / {}), so one crossing would be \
                 answered by two loops",
                other.id,
                box_words(other.slab),
                box_words(other.landing())
            )));
        }
    }
    // spec-0083 struck the seal-lifting list; every authored teleport's source
    // volume, link or gather, is the one list.
    for tp in &crate::compiler::link::source_volumes(plan) {
        if boxes_meet(l.slab, *tp) {
            return Err(slab_fault(format!(
                "it overlaps a `teleport` volume {}, so a body in it would be carried twice",
                box_words(*tp)
            )));
        }
    }
    for v in &plan.lethal_volumes {
        let k = delvewright_dsl::metrics::keep_out_box(
            delvewright_dsl::metrics::Body::PLAYER,
            v.region.0,
            v.region.1,
        );
        if boxes_meet(l.slab, k) {
            return Err(slab_fault(format!(
                "it overlaps lethal volume `{}`'s keep-out {}, so a body crossing it may be \
                 killed and moved in one tick",
                v.id,
                box_words(k)
            )));
        }
    }
    for x in l.slab.0[0]..=l.slab.1[0] {
        for y in l.slab.0[1]..=l.slab.1[1] {
            for z in l.slab.0[2]..=l.slab.1[2] {
                if !world.is_clear([x, y, z]) {
                    return Err(slab_fault(format!(
                        "its cell {} is not passable (a block or water stands there), and every \
                         cell of a slab is one a body can be in — draw the slab over the open \
                         cross-section of the passage and no further",
                        cell_words([x, y, z])
                    )));
                }
            }
        }
    }
    if !world.is_standable(l.cross()) {
        return Err(slab_fault(format!(
            "its lowest course in its anchor's column, {}, is not a cell a body stands in, and \
             the exercise step crosses the slab there — draw the slab down to the passage floor",
            cell_words(l.cross())
        )));
    }
    // A body crossing a horizontal slab may be in the air: vanilla selects on
    // hitbox intersection, so the slab must reach above the highest feet a jump
    // from its floor puts a body at, or a jumping body passes over it unmoved.
    // The apex is `MAX_JUMP_RISE_16` sixteenths and the next sixteenth is out of
    // reach (`metrics`), so the slab's top face must stand at least that next
    // sixteenth over the floor course's walk plane.
    if a != 1 {
        let above_16 =
            i64::from(l.slab.1[1] + 1 - l.cross()[1]) * delvewright_dsl::metrics::FULL_16;
        let need_16 = delvewright_dsl::metrics::MAX_JUMP_RISE_16 + 1;
        if above_16 < need_16 {
            return Err(slab_fault(format!(
                "it is too low to catch a jumping body: its top face stands {above}/16 of a \
                 block over the walk plane at {}, and a jump from that floor lifts a body's feet \
                 to {apex}/16 (`metrics::MAX_JUMP_RISE_16`), so a body jumping across passes \
                 over the slab between two polls. Draw the slab at least {courses} courses tall \
                 from the passage floor",
                cell_words(l.cross()),
                above = above_16,
                apex = delvewright_dsl::metrics::MAX_JUMP_RISE_16,
                courses = (need_16 + delvewright_dsl::metrics::FULL_16 - 1)
                    / delvewright_dsl::metrics::FULL_16,
            )));
        }
    }

    // ---- §4.3 the periodic span, in every fog configuration ----
    //
    // The view is judged under the biome map at the first tick and after each
    // `set-atmosphere` repaint the route can stand after (spec-0086 §4.6 does
    // the same for block writes): a repaint that clears the fog over the span
    // opens a view the first tick's fog closed. The span is the union of the
    // spans every configuration grows, and the visible set the union of what
    // each configuration's eyes see inside their own fog.
    let eyes = eyes(l, world);
    row.eyes = eyes.len();
    if eyes.is_empty() {
        row.open_faces = 1;
        return Err(Failure {
            code: DW_LOOP_OPEN_VIEW,
            message: format!(
                "loop `{}`: no cell of its landing slab {} is one a body can stand in, so there is \
                 no eye the landing can be proven seamless from. Put the landing (`to`) on the \
                 passage floor, the same distance back as the corridor repeats",
                l.id,
                box_words(l.landing())
            ),
        });
    }
    let mut fog_lo = f64::INFINITY;
    let mut fog_hi: f64 = 0.0;
    let mut span: Option<Span> = None;
    let mut seen: BTreeSet<[i32; 3]> = BTreeSet::new();
    for (k, (state_label, _)) in fog.states.iter().enumerate() {
        let fogs: Vec<f64> = eyes.iter().map(|e| fog_end_at(fog, k, *e)).collect();
        for f in &fogs {
            fog_lo = fog_lo.min(*f);
            fog_hi = fog_hi.max(*f);
        }
        row.fog = (fog_lo, fog_hi);
        let this = match grow(l, world, &eyes, &fogs, built) {
            Ok(s) => s,
            Err(why) => {
                row.open_faces = 1;
                let when = if k == 0 {
                    String::new()
                } else {
                    format!(" under the sky {state_label}")
                };
                return Err(Failure {
                    code: DW_LOOP_OPEN_VIEW,
                    message: format!(
                        "loop `{}`: the view out of its landing is open{when} — {why}. A body \
                         moved by [{}, {}, {}] would see that cell stand {} blocks nearer. Close \
                         the view inside the span: turn or jog the corridor, or put a door, a \
                         grille or a pillar across the line — or give the place an atmosphere \
                         whose fog end the eye reads whole in every configuration the route \
                         stands under",
                        l.id,
                        l.offset[0],
                        l.offset[1],
                        l.offset[2],
                        l.offset[a].abs()
                    ),
                });
            }
        };
        for x in this.b.0[0]..=this.b.1[0] {
            for y in this.b.0[1]..=this.b.1[1] {
                for z in this.b.0[2]..=this.b.1[2] {
                    let c = [x, y, z];
                    if sight(world, &eyes, &fogs, c).0 == Sight::Seen {
                        seen.insert(c);
                    }
                }
            }
        }
        span = Some(match span {
            None => this,
            Some(prev) => Span {
                b: (
                    std::array::from_fn(|i| prev.b.0[i].min(this.b.0[i])),
                    std::array::from_fn(|i| prev.b.1[i].max(this.b.1[i])),
                ),
                steps: prev.steps.max(this.steps),
                closed_geometry: prev.closed_geometry.max(this.closed_geometry),
                closed_fog: prev.closed_fog.max(this.closed_fog),
            },
        });
    }
    let span = span.expect("the first tick's configuration is always judged");
    row.span = Some(span.b);
    row.span_cells = box_cells(span.b);
    row.steps = span.steps;
    row.closed_geometry = span.closed_geometry;
    row.closed_fog = span.closed_fog;
    let visible: Vec<[i32; 3]> = seen.into_iter().collect();
    row.visible = visible.len();

    // ---- §4.6 bodies ----
    let mut bodies: Vec<(String, [i32; 3])> =
        crate::compiler::lethal::posted_places(plan, None, i.wave_seats)
            .into_iter()
            .map(|p| (p.label, p.cell))
            .collect();
    bodies.extend(
        crate::compiler::eclipse::affordances(plan)
            .into_iter()
            .map(|a| {
                (
                    format!("the {} `{}` at anchor `{}`", a.kind, a.id, a.anchor),
                    a.pos,
                )
            }),
    );
    let inside_span: Vec<&(String, [i32; 3])> =
        bodies.iter().filter(|(_, c)| inside(span.b, *c)).collect();
    row.bodies = inside_span.len();
    if let Some((label, c)) = inside_span.first() {
        return Err(Failure {
            code: DW_LOOP_BODY,
            message: format!(
                "loop `{}`: {label} stands at {}, inside the loop's periodic span {} — a body has an \
                 identity the move cannot repeat, so a body moved a bay back would see the same \
                 figure twice, or none. Move the body out of the span; a figure that appears \
                 mid-loop is placed by an `on_cross` effect outside the visible cells, or summoned \
                 after the release",
                l.id,
                cell_words(*c),
                box_words(span.b)
            ),
        });
    }

    // ---- §4.4–§4.6 the tiling, in every configuration ----
    let d = l.offset;
    let image = |c: [i32; 3]| [c[0] - d[0], c[1] - d[1], c[2] - d[2]];
    let both = (
        [
            span.b.0[0].min(span.b.0[0] - d[0]),
            span.b.0[1].min(span.b.0[1] - d[1]),
            span.b.0[2].min(span.b.0[2] - d[2]),
        ],
        [
            span.b.1[0].max(span.b.1[0] - d[0]),
            span.b.1[1].max(span.b.1[1] - d[1]),
            span.b.1[2].max(span.b.1[2] - d[2]),
        ],
    );
    let top = built.iter().map(|b| b.1[1]).max().unwrap_or(both.1[1]);
    let clip = (
        [both.0[0] - 16, both.0[1] - 16, both.0[2] - 16],
        [both.1[0] + 16, top.max(both.1[1] + 16), both.1[2] + 16],
    );
    // Volumes in the span tile or are refused — in every configuration the
    // route stands under: a staged lethal volume (spec-0088) counts where it may
    // be live there ([`crate::compiler::nav::World::staged_liveness`]), so a
    // volume live in one bay and its image dead in the next is a seam.
    let keep_out = |r: Region| {
        delvewright_dsl::metrics::keep_out_box(delvewright_dsl::metrics::Body::PLAYER, r.0, r.1)
    };
    let staged_ids: Vec<String> = world
        .staged_volumes()
        .iter()
        .map(|v| v.id.clone())
        .collect();
    let ancestor = |g: usize, s: usize| plan.gate_fired_before(g, s);
    let mut live_sets: Vec<BTreeSet<String>> = Vec::new();
    for arrival in 0..=plan.critical_path.len() {
        let set: BTreeSet<String> = staged_ids
            .iter()
            .zip(world.staged_liveness(&plan.region_events, arrival, &ancestor))
            .filter(|(_, l)| l.may)
            .map(|(id, _)| id.clone())
            .collect();
        if !live_sets.contains(&set) {
            live_sets.push(set);
        }
    }
    let span_cells: Vec<[i32; 3]> = (span.b.0[0]..=span.b.1[0])
        .flat_map(|x| {
            (span.b.0[1]..=span.b.1[1])
                .flat_map(move |y| (span.b.0[2]..=span.b.1[2]).map(move |z| [x, y, z]))
        })
        .collect();
    let mut counted: BTreeSet<String> = BTreeSet::new();
    for live in &live_sets {
        let mut volumes: Vec<(String, Region)> = plan
            .lethal_volumes
            .iter()
            .filter(|v| v.staged.is_none() || live.contains(&v.id))
            .map(|v| {
                (
                    format!("lethal volume `{}`'s keep-out", v.id),
                    keep_out(v.region),
                )
            })
            .collect();
        volumes.extend(
            crate::compiler::link::source_volumes(plan)
                .into_iter()
                .map(|b| ("a `teleport` volume".to_string(), b)),
        );
        volumes.retain(|(_, b)| boxes_meet(*b, both));
        counted.extend(volumes.iter().map(|(l, b)| format!("{l}{b:?}")));
        let when = if staged_ids.is_empty() {
            String::new()
        } else if live.is_empty() {
            " (in a configuration where no staged lethal volume may be live)".to_string()
        } else {
            format!(
                " (in the configuration where {} may be live)",
                live.iter()
                    .map(|id| format!("`{id}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        for (label, b) in &volumes {
            let in_vol = |c: [i32; 3]| volumes.iter().any(|(_, v)| inside(*v, c));
            if let Some(c) = span_cells
                .iter()
                .find(|c| in_vol(**c) != in_vol(image(**c)))
            {
                row.volumes = counted.len();
                return Err(Failure {
                    code: DW_LOOP_TILING,
                    message: format!(
                        "loop `{}`: {label} {} lies in the periodic span without its image under \
                         the offset{when} — the span's cell {} is {} and the cell it stands for \
                         from the slab, {}, is {}. A pit the player sees in one bay and not the \
                         next is the frame jump by other means. Make the sections the same: put \
                         the same volume, live from the same stage, under the other bay, or move \
                         it out of the span",
                        l.id,
                        box_words(*b),
                        cell_words(*c),
                        if in_vol(*c) {
                            "inside it"
                        } else {
                            "outside it"
                        },
                        cell_words(image(*c)),
                        if in_vol(image(*c)) {
                            "inside it"
                        } else {
                            "outside it"
                        },
                    ),
                });
            }
        }
    }
    row.volumes = counted.len();
    let base = shipped(plan, i.blocks, i.placements, i.seals, clip);
    let mut configs: Vec<Config> = vec![Config {
        label: "the world as it is placed, before any runtime write".to_string(),
        writes: Vec::new(),
    }];
    let mut acc: Vec<(Region, String)> = Vec::new();
    for (step, label, r, block) in writes {
        if !boxes_meet(*r, clip) {
            continue;
        }
        acc.push((*r, block.clone()));
        configs.push(Config {
            label: format!("after {label} (critical-path step {step})"),
            writes: acc.clone(),
        });
    }
    // The exercise's own writes, crossing by crossing (spec-0086 §4.6).
    let li = plan
        .campaign
        .quests
        .content
        .loops
        .iter()
        .position(|d| d.id.as_str() == l.id)
        .unwrap_or(0);
    for e in plan.loop_exercises.iter().filter(|e| e.r#loop == l.id) {
        for (n, fired) in e.fired.iter().enumerate() {
            let mut any = false;
            let mut named: Vec<String> = Vec::new();
            for &fi in fired {
                let Some(eff) = l.on_cross.get(fi) else {
                    continue;
                };
                let w = match &eff.verb {
                    Verb::FillRegion { region, block, .. } => {
                        plan.zone_box(region).map(|r| (r, block.clone()))
                    }
                    Verb::ClearRegion { region, .. } => plan
                        .zone_box(region)
                        .map(|r| (r, "minecraft:air".to_string())),
                    Verb::SetBlock { anchor, block, .. } => plan
                        .point_any(anchor.as_str())
                        .map(|p| ((p, p), block.clone())),
                    _ => None,
                };
                if let Some(w) = w
                    && boxes_meet(w.0, clip)
                {
                    acc.push(w);
                    any = true;
                    named.push(format!(
                        "`{}` at `/content/loops/{li}/on_cross/{fi}`",
                        eff.verb.tag()
                    ));
                }
            }
            if any {
                configs.push(Config {
                    label: format!(
                        "after crossing {} of the exercise step (critical-path step {}), which \
                         runs {}",
                        n + 1,
                        e.step,
                        named.join(", ")
                    ),
                    writes: acc.clone(),
                });
            }
        }
    }
    row.configurations = configs.len();
    for cfg in &configs {
        let mut m = base.clone();
        for (r, b) in &cfg.writes {
            write_box(&mut m, *r, b, clip);
        }
        let block_at = |c: [i32; 3]| m.get(&c).map(String::as_str).unwrap_or("minecraft:air");
        let diffs: Vec<[i32; 3]> = visible
            .iter()
            .copied()
            .filter(|c| block_at(*c) != block_at(image(*c)))
            .collect();
        if let Some(first) = diffs.first() {
            let shown: Vec<String> = diffs
                .iter()
                .take(6)
                .map(|c| {
                    format!(
                        "{} holds `{}` and {} holds `{}`",
                        cell_words(*c),
                        block_at(*c),
                        cell_words(image(*c)),
                        block_at(image(*c))
                    )
                })
                .collect();
            let _ = first;
            return Err(Failure {
                code: DW_LOOP_TILING,
                message: format!(
                    "loop `{}`: {} visible cell(s) of the periodic span differ from the cell \
                     each is seen as from the slab, in the configuration {} — {}. The view from \
                     the landing would not be the view from the slab. Make the two sections the \
                     same; never shorten the view to hide the difference",
                    l.id,
                    diffs.len(),
                    cfg.label,
                    shown.join("; ")
                ),
            });
        }
        let model =
            crate::compiler::light::LightModel::from_blocks_within(m.clone(), clip.0, clip.1);
        for (sky_word, sky) in skies {
            let lit = model.flood(*sky);
            let at = |c: [i32; 3]| lit.get(&c).copied().unwrap_or(0);
            if let Some(c) = visible.iter().find(|c| at(**c) != at(image(**c))) {
                return Err(Failure {
                    code: DW_LOOP_TILING,
                    message: format!(
                        "loop `{}`: the visible cell {} is lit {} and the cell it is seen as from \
                         the slab, {}, is lit {}, at the campaign's {sky_word} reachable sky \
                         ({sky}), in the configuration {} — something outside the visible cells \
                         lights two sections differently, a lamp round a corner or a hole in the \
                         roof over the next bay. Make the sections the same",
                        l.id,
                        cell_words(*c),
                        at(*c),
                        cell_words(image(*c)),
                        at(image(*c)),
                        cfg.label
                    ),
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::assembled::Occupancy;
    use crate::compiler::nav::{Premises, World};

    fn world(solid: &[[i32; 3]], tall: &[[i32; 3]], flooded: &[[i32; 3]]) -> World {
        World::from_occupancy(
            Occupancy {
                solid: solid.iter().copied().collect(),
                tall: tall.iter().copied().collect(),
                use_gates: BTreeSet::new(),
                flooded: flooded.iter().copied().collect(),
                partial: BTreeMap::new(),
                waterloggable: BTreeSet::new(),
                lava: BTreeSet::new(),
            },
            Premises::geometry_only(),
        )
    }

    /// The sightline is `nav::walk_cells` stopped by `World::blocks_camera`: a
    /// fence across the line stops it, water across it does not (spec-0086
    /// criterion 4).
    #[test]
    fn a_fence_stops_the_sight_and_water_does_not() {
        let eye = [[0.5, 1.62, 0.5]];
        let target = [0, 1, 6];
        let open = world(&[], &[], &[]);
        assert_eq!(
            sight(&open, &eye, &[FOG_END_DEFAULT], target).0,
            Sight::Seen
        );
        // A fence on every cell of the plane between: the fence's own class is
        // what blocks it, not a solid.
        let fence: Vec<[i32; 3]> = (-2..=2)
            .flat_map(|x| (-1..=4).map(move |y| [x, y, 3]))
            .collect();
        let fenced = world(&[], &fence, &[]);
        assert_eq!(
            sight(&fenced, &eye, &[FOG_END_DEFAULT], target).0,
            Sight::Hidden
        );
        let flooded = world(&[], &[], &fence);
        assert_eq!(
            sight(&flooded, &eye, &[FOG_END_DEFAULT], target).0,
            Sight::Seen
        );
        // The target cell itself may be a wall: it is what the eye sees.
        let wall = world(&[target], &[], &[]);
        assert_eq!(
            sight(&wall, &eye, &[FOG_END_DEFAULT], target).0,
            Sight::Seen
        );
    }

    fn plan_loop(offset: [i32; 3]) -> LoopPlan {
        LoopPlan {
            id: "loop/t".into(),
            safe: "t".into(),
            area: "area/a".into(),
            anchor_cell: [0, 2, 10],
            slab: ([-1, 1, 10], [1, 3, 10]),
            to_cell: shift([0, 2, 10], offset),
            offset,
            requires_flags: Vec::new(),
            forbids_flags: vec!["flag/f".into()],
            requires_state: Vec::new(),
            counts: None,
            on_cross: Vec::new(),
        }
    }

    #[test]
    fn the_slab_reads_its_axis_its_approach_and_its_crossing_cell() {
        let l = plan_loop([0, 0, -6]);
        assert_eq!(l.axis(), Some(2));
        assert_eq!(l.thickness(2), 1);
        assert_eq!(l.cross(), [0, 1, 10]);
        assert_eq!(l.transport(), [0, 1, 4]);
        assert!(l.on_approach([0, 1, 4]));
        assert!(!l.on_approach([0, 1, 11]));
        assert!(l.beyond([0, 1, 11]));
        assert_eq!(l.landing(), ([-1, 1, 4], [1, 3, 4]));
        assert_eq!(plan_loop([0, 0, 0]).axis(), None);
        assert_eq!(plan_loop([1, 0, -6]).axis(), None);
    }

    #[test]
    fn a_gate_reads_flags_and_party_data_and_an_undatable_value_holds() {
        let mut l = plan_loop([0, 0, -6]);
        let mut st = GateState::default();
        assert_eq!(st.holds(&l), Some(true));
        st.flags.insert("flag/f".into());
        assert_eq!(st.holds(&l), Some(false));
        l.forbids_flags.clear();
        l.requires_state = vec![StateCompare {
            state: delvewright_dsl::StateId("state/n".into()),
            op: CompareOp::AtMost,
            value: 1,
        }];
        st.data.insert("state/n".into(), None);
        assert_eq!(st.holds(&l), None, "undatable");
        st.data.insert("state/n".into(), Some(2));
        assert_eq!(st.holds(&l), Some(false));
    }
}
