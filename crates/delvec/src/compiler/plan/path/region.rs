//! Region events: every region write the path performs, and when it fires.

use super::*;

/// **One runtime region write**, collected in deterministic content order — the
/// single completability model of "a box the delve fills or clears while it is
/// running", whichever verb spelled it.
///
/// Five verbs produce these and none of them owns the rule: `close-gate` fills a
/// prefab gate anchor's region with the block that anchor declares and `open-gate`
/// clears it (DSL v0.6); `fill-region` and `clear-region` do the same to an
/// author-declared box (DSL v0.10, spec-0031); `open-way` does it to the cells a
/// placed piece's spatial contract exports, in the direction that contract
/// declares (DSL v0.12, spec-0042); a `shortcut`'s gate is registered filled from
/// world-load. The occupancy model (`crate::compiler::assembled`) treats every
/// *gate* cell as always passable — the conservative "assume the gate the player
/// needs is opened" stance `DW0306` checks — and a `fill` is the physical dual:
/// the critical-path / checkpoint reachability proofs treat the region as
/// **solid** on any walked leg reached *after* the latest write at or before it is
/// a fill, so a path that must cross it fails `DW0311`/`DW0315`. A `clear` is the
/// other direction, and is credited the same way: the region is **passable** from
/// the DAG point at which it fires (`nav::World::with_cleared`).
///
/// The type is named for the object it acts on — a region — and not for the verb
/// that first needed it (CLAUDE.md): the third consumer inherits this proof
/// instead of re-deriving it, which is exactly what `open-gate`/`close-gate`
/// having owned it privately prevented.
/// What a runtime region write leaves in the region — read straight off the
/// command the verb emits, because that is the only thing the model may conclude.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionWrite {
    /// Every cell becomes solid: `close-gate` (the gate anchor's declared block),
    /// `fill-region` (the author's block), a `shortcut`'s world-load seal.
    ///
    /// "Solid" is a claim about the **block**, not about the write. Only a write
    /// whose block collides leaves a wall or a floor behind, so the block is
    /// classified once, by [`RegionWrite::of_block`]: a fluid lands in
    /// [`RegionWrite::Flood`], a block a body passes through in
    /// [`RegionWrite::Pass`]. A partial floor (a bottom slab) is modelled as a
    /// full cube here — the region model carries no partial heights — which can
    /// only refuse a step vanilla admits.
    Fill,
    /// Every cell becomes a block **a body passes through**: a `fill-region` /
    /// `close-gate` / `open-way` whose block has no collision a walk reads — a
    /// sculk vein, glow lichen, a flower, a torch (no box), a carpet (a box under
    /// the auto-step), a vine or a ladder (a climb, not a wall)
    /// ([`delvewright_dsl::blockshape::Collision::Thin`],
    /// [`delvewright_dsl::blockshape::Collision::Climbable`]).
    ///
    /// To the walk it is a [`RegionWrite::Clear`] — the fill destroys whatever
    /// the box held and leaves cells a body occupies — credited only when the
    /// party is forced to cause it, and dated by its step even under a trigger,
    /// because it can only open a region. To the bytes it is a fill: it lays its
    /// block, so the configuration's block map shows the vein, not air.
    Pass,
    /// Every cell becomes **free fluid**: a `fill-region` / `close-gate` /
    /// `shortcut` seal whose block is water or lava
    /// ([`crate::compiler::assembled::is_fluid`]).
    ///
    /// A separate case from [`RegionWrite::Fill`] because the two conclusions are
    /// opposite where it matters. A fill of stone is impassable **and** floor; a
    /// fill of water is impassable and **never** floor. Collapsing them says a body
    /// stands on a water surface, and the nav model's `flooded` set — impassable,
    /// never standable — is precisely the set that already says otherwise, so this
    /// is a classification the model was missing, not a capability.
    ///
    /// **What it does not model**: the fluid's spread beyond the written region.
    /// Vanilla flows a source outward at world-tick; this marks the written cells
    /// and no more, so the model can under-mark the wet set exactly as
    /// [`crate::compiler::nav::World::with_cleared`] documents for a clear that opens a dry
    /// region into adjacent water. Both are the same missing input — a runtime
    /// block map to re-derive the flood from — and both are stated in
    /// `docs/reference/compiler.md` rather than left to be discovered.
    Flood,
    /// Every cell becomes empty: `clear-region`, whose emitted
    /// `fill … minecraft:air` carries no `replace` filter and so removes whatever
    /// is there.
    Clear,
    /// **Only the gate's own block** becomes empty: `open-gate`, whose emitted fill
    /// is `replace`-filtered to the block the gate anchor declares.
    ///
    /// A third case rather than a synonym for [`RegionWrite::Clear`], because the
    /// emitted commands differ and so does what may be concluded from them. The
    /// assembled world already holds every gate cell empty, so an unseal removes
    /// nothing the model believed was there — an unfiltered clear does. Collapsing
    /// the two says an `open-gate` deletes a `collapse`'s debris resting in the
    /// doorway, which in game it plainly does not (`DW0445`, measured:
    /// `v06_trap_payloads::collapse_that_buries_the_critical_path_is_dw0445` goes
    /// green — i.e. stops proving anything — the moment they are collapsed). An
    /// unseal still takes part in latest-write-wins, which is how a later
    /// `open-gate` cancels an earlier `close-gate`.
    Unseal,
}

impl RegionWrite {
    /// **The one place a block id becomes a region write's conclusion.** Every
    /// site that turns "this verb fills that box with that block" into a model
    /// update goes through here, so no two of them can disagree about what a
    /// fluid leaves behind.
    ///
    /// It reads [`delvewright_dsl::blockshape::collision_class`] — the collision
    /// table measured from the pinned jar, the one the static occupancy model
    /// classifies every assembled cell by — because the question "what does this
    /// block do to a walker" belongs to the block, not to the verb that wrote it.
    /// A waterlogged block a body would collide with is deliberately a
    /// [`RegionWrite::Fill`]: its cell is occupied by the host block and is
    /// genuine floor (see `is_fluid`'s note). A waterlogged block a body passes
    /// through leaves the cell's water free, and is a [`RegionWrite::Flood`].
    /// An air block is a [`RegionWrite::Clear`]: `fill … minecraft:air` is what a
    /// `clear-region` emits.
    pub fn of_block(block: &str) -> RegionWrite {
        use delvewright_dsl::blockshape::{Collision, collision_class};
        match collision_class(block) {
            Collision::Air => RegionWrite::Clear,
            Collision::Fluid => RegionWrite::Flood,
            Collision::Thin(_) | Collision::Climbable => {
                if crate::compiler::assembled::is_waterlogged(block) {
                    RegionWrite::Flood
                } else {
                    RegionWrite::Pass
                }
            }
            Collision::PartialFloor(_)
            | Collision::FullCube
            | Collision::TallBarrier
            | Collision::FenceGate => RegionWrite::Fill,
        }
    }

    /// Whether this write **overwrites** the region with a block, rather than
    /// emptying it — true for [`RegionWrite::Fill`], [`RegionWrite::Flood`] and
    /// [`RegionWrite::Pass`], because a `fill … minecraft:water` or a
    /// `fill … minecraft:sculk_vein` destroys whatever was in the box exactly as
    /// a `fill … minecraft:stone` does. It says nothing about whether the result
    /// is standable or passable; that is [`RegionWrite::closes`]'s.
    pub fn fills(&self) -> bool {
        matches!(
            self,
            RegionWrite::Fill | RegionWrite::Flood | RegionWrite::Pass
        )
    }

    /// Whether this write **can make the region impassable** —
    /// [`RegionWrite::Fill`] and [`RegionWrite::Flood`]. A write that closes is
    /// credited even when nobody has to cause it, and a trigger's is assumed
    /// from the start; a write that only opens (a clear, an unseal, a pass) is
    /// credited only when forced, at its own step.
    pub fn closes(&self) -> bool {
        matches!(self, RegionWrite::Fill | RegionWrite::Flood)
    }
}

/// One resolved region write: the inclusive world box, and what the write leaves
/// in it. A verb resolves to a LIST of these, because a way is a region of as
/// many boxes as its contract gave it and each is written by its own `fill`.
type ResolvedWrite = (([i32; 3], [i32; 3]), RegionWrite, Option<String>);

#[derive(Clone, Debug)]
pub struct RegionEvent {
    /// The region's inclusive corners (absolute world coords).
    pub region: ([i32; 3], [i32; 3]),
    /// What this write leaves in the region.
    pub write: RegionWrite,
    /// The `critical_path` step index at which this firing happens.
    pub fire_step: usize,
    /// **Whether the party is guaranteed to cause this firing**, computed from the
    /// quest graph and the effect's root — never asserted by an author, because the
    /// DSL has no surface on which to assert it (see [`collect_region_events`]).
    ///
    /// Private, with [`RegionEvent::forced`] / [`RegionEvent::unforced`] as the only
    /// ways in, so a `RegionEvent` **cannot be built without answering this
    /// question**. It is the same move [`RegionWrite::of_block`] makes for the block:
    /// the model's premises are constructed, not defaulted.
    forced: bool,
    /// The beat this firing hangs off, in words, for a diagnostic to name. Empty for
    /// a forced write, which never needs blaming.
    blame: String,
    /// **The block this write lays**, as its emitted command writes it
    /// (spec-0088 §5): the fill's block for a [`RegionWrite::Fill`] or
    /// [`RegionWrite::Flood`], `None` for a clear or an unseal (air) and for a
    /// world-load seal (the bytes already hold what the prefab put there).
    /// Read only by [`crate::compiler::nav`]'s per-configuration block map, which
    /// `DW0891` asks whether a cell is shown in that configuration.
    block: Option<String>,
}

impl RegionEvent {
    /// A write the party **cannot avoid causing**: a quest bundle they must complete,
    /// an environment trigger, or a wall the placed world is born holding.
    pub fn forced(region: ([i32; 3], [i32; 3]), write: RegionWrite, fire_step: usize) -> Self {
        RegionEvent {
            region,
            write,
            fire_step,
            forced: true,
            blame: String::new(),
            block: None,
        }
    }

    /// A write that **may never happen**: a sprung trap, a bought offer, a death, a
    /// shortcut taken from the far side. `blame` names the beat in words.
    pub fn unforced(
        region: ([i32; 3], [i32; 3]),
        write: RegionWrite,
        fire_step: usize,
        blame: impl Into<String>,
    ) -> Self {
        RegionEvent {
            region,
            write,
            fire_step,
            forced: false,
            blame: blame.into(),
            block: None,
        }
    }

    /// This write, stating the block its command lays (see [`Self::block`]).
    #[must_use]
    pub fn laying(mut self, block: &str) -> Self {
        self.block = Some(block.to_string());
        self
    }

    /// The block this write lays, when it lays one.
    pub fn block(&self) -> Option<&str> {
        self.block.as_deref()
    }

    /// Whether this write overwrites the region with a block
    /// ([`RegionWrite::fills`]).
    pub fn fills(&self) -> bool {
        self.write.fills()
    }

    /// Whether the party is guaranteed to cause this firing.
    pub fn is_forced(&self) -> bool {
        self.forced
    }

    /// The beat this firing hangs off, in words; empty when it is forced.
    pub fn blame(&self) -> &str {
        &self.blame
    }
}

/// One `set-flag` the campaign can perform (spec-0088 §4.2): the flag, the
/// critical-path step it fires at, and whether the party is forced to cause it —
/// read by [`firing_of`] for an effect, off the path's own `talk-to` choice for
/// a dialogue option, and unforced at step 0 for a disarm.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlagEvent {
    /// The flag set.
    pub flag: String,
    /// The `critical_path` step at which it fires.
    pub fire_step: usize,
    /// Whether the party is guaranteed to cause it.
    pub forced: bool,
}

/// The replay's datum values along one path (spec-0088 §4.1): what each
/// declared datum holds as the party walks up to each objective step, and at the
/// end — `None` where no ordered walk can name it — plus every datum some
/// unforced root writes. What a staged volume's numeric terms are read against.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DataReplay {
    /// Critical-path step of an objective → every datum's value before it.
    pub before: BTreeMap<usize, BTreeMap<String, Option<i64>>>,
    /// Every datum's value once the path has played.
    pub end: BTreeMap<String, Option<i64>>,
    /// Datums an unforced firing writes.
    pub unforced_writers: BTreeSet<String>,
}

impl DataReplay {
    /// The values the party holds walking the leg that arrives at `arrival`:
    /// the first objective step at or after it, else the end of the path.
    pub fn at(&self, arrival: usize) -> &BTreeMap<String, Option<i64>> {
        self.before
            .range(arrival..)
            .next()
            .map_or(&self.end, |(_, v)| v)
    }
}

/// **Everything the region model reads off one path**: the runtime region
/// writes, and — for a lethal volume live from a story stage (spec-0088) — every
/// flag write and the replay's datum values, in the same step space.
///
/// One value, so the exported path and each branch path ([`Plan::branch_gate_model`])
/// hand the region model their own flags with their own writes, and no caller
/// can route a leg with one path's writes and another's flags. Derefs to the
/// writes, which is what every reader that predates staged volumes reads.
#[derive(Clone, Debug, Default)]
pub struct RegionEvents {
    writes: Vec<RegionEvent>,
    /// Every `set-flag` the campaign can perform, with its step and forcedness.
    pub flags: Vec<FlagEvent>,
    /// The replay's datum values along the path.
    pub data: DataReplay,
}

impl From<Vec<RegionEvent>> for RegionEvents {
    fn from(writes: Vec<RegionEvent>) -> Self {
        RegionEvents {
            writes,
            ..RegionEvents::default()
        }
    }
}

impl std::ops::Deref for RegionEvents {
    type Target = Vec<RegionEvent>;
    fn deref(&self) -> &Vec<RegionEvent> {
        &self.writes
    }
}

impl<'a> IntoIterator for &'a RegionEvents {
    type Item = &'a RegionEvent;
    type IntoIter = std::slice::Iter<'a, RegionEvent>;
    fn into_iter(self) -> Self::IntoIter {
        self.writes.iter()
    }
}

impl std::ops::DerefMut for RegionEvents {
    fn deref_mut(&mut self) -> &mut Vec<RegionEvent> {
        &mut self.writes
    }
}

/// Collect every `open-gate` / `close-gate` firing (DSL v0.6) that emission can
/// lower, resolving each anchor to its gate region and rooting it at its firing
/// step. Feeds the `close-gate` completability model in `crate::compiler::nav`
/// (`DW0311`/`DW0315`/`DW0342`/`DW0410`).
///
/// Walks [`for_each_gate_effect`] — the **same** traversal the seal planner and
/// `gates::check_seal_hints` walk — so the model and the emission cannot disagree
/// about which firings exist. A model that saw only three of the five roots
/// emission reaches would leave a `close-gate` in a `traps[].payload` or a
/// dialogue option's `on_respawn` bundle filled in the datapack while every nav
/// proof believed the wall was open. Nesting is descended by that traversal, so a
/// gate effect inside a `sequence` step / lifecycle bundle is registered at its
/// root's firing step. An effect whose anchor is not a resolvable gate is skipped
/// (a point anchor / bad close-gate is a validation concern, `DW0142`/`DW0343`).
///
/// **When** a firing happens is read off the site's [`EffectRoot`]:
///
/// - a quest `on_objective_complete` fires at that objective's step, an
///   `on_complete` at the quest's completion step — the player is *forced* through
///   both on a path that plays them, so both directions are modelled; on a path
///   that never plays them they are unforced ([`firing_of`]);
/// - an environment trigger's **openings** fire at the `trigger` step the path
///   performs it in (`trigger_step`), and a trigger the path never performs opens
///   nothing; its **fills** are rooted at step 0, forced, which seals every leg
///   against them — a wall is assumed up from the start and never assumed down
///   before somebody strikes it;
/// - a trap payload and a dialogue-hosted `on_respawn` bundle have no step of
///   their own (a sprung trap, a death), so both are rooted conservatively at
///   step 0, which precedes every leg.
///
/// The **optional** roots — a trap the party may never trip, a death nobody is
/// forced to suffer, an offer nobody is forced to buy — register their *filling*
/// writes only. An unguaranteed firing may be assumed to have happened exactly when
/// assuming so is conservative: it can seal a region (the proof must survive the
/// seal), it can never unseal one (the proof may not lean on a wall the player might
/// never open). That is the same rule a shortcut gate already obeys — sealed for the
/// whole model, because the delve must be finishable the long way.
///
/// **A fill from such a root is registered, and marked unforced, because "it sealed"
/// and "you can stand on it" are two different conclusions and only the first is
/// conservative.** The same solid block that walls a doorway floors the cell above
/// it, so a fill assumed-to-have-happened both blocks the party (harder — sound) and
/// carries them (easier — unsound). Dropping the event would lose the seal; keeping
/// it as an ordinary fill lends the forced path footing off a beat nobody has to
/// play. So the event is kept and the *uncertainty travels with it*
/// ([`RegionEvent::is_forced`]); `crate::compiler::nav` is where the two conclusions part.
///
/// A **flood** needs no such split and gets none: a flooded cell is impassable and
/// never floor, which is already the pointwise-worst of "the water is there" and "it
/// is not", so an unforced flood is exactly as conservative as a forced one.
pub(in crate::compiler::plan) fn collect_region_events(
    campaign: &Campaign,
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
    path: &PathFiring,
    ways: &crate::compiler::ways::WayStaging,
) -> Vec<RegionEvent> {
    // spec-0051 §8.6: the skippable-root class, widened. A bundle rooted in an
    // OPTIONAL quest is one the party may never fire, so it seals like a trap
    // payload and lays no footing the forced path may stand on — the same rule
    // with a wider denominator, read off the ONE authority
    // (`QuestPlanContent::optional`) rather than a private `!q.mandatory`.
    //
    // This is the direction that ships if it is wrong: crediting an optional
    // quest's `clear-region` as forced proves a leg walkable across a hole the
    // party only opens by playing content nobody makes them play.
    let optional = campaign.quest_plan.content.optional();
    let mut out = Vec::new();
    for_each_gate_effect(campaign, &mut |site, e| {
        // How a firing is BLAMED when it turns out to be unforced. Worded off the
        // root the site already carries, so the diagnostic names the beat an author
        // can go and look at rather than a JSON pointer alone.
        let blame = || match site.root {
            EffectRoot::TrapPayload(t) => {
                format!(
                    "the payload of trap `{}`, which the party may never spring",
                    t.id
                )
            }
            EffectRoot::DialogueRespawn => format!(
                "a `set-checkpoint` `on_respawn` bundle at `{}`, which runs only if somebody dies",
                site.path
            ),
            EffectRoot::ShortcutUnlock => format!(
                "a shortcut's `on_unlock` bundle at `{}`, which fires only if the party opens the \
                 shortcut from its far side",
                site.path
            ),
            EffectRoot::OnDeath => format!(
                "the campaign's `on_death` bundle at `{}`, which runs only if somebody dies",
                site.path
            ),
            EffectRoot::ShopOffer => format!(
                "a shop offer's effects at `{}`, which fire only if the party buys it",
                site.path
            ),
            EffectRoot::OnKill(f) => format!(
                "the `on_kill` bundle of {} `{}`, which fires only on a kill a player is \
                 credited with",
                f.word(),
                f.id()
            ),
            EffectRoot::LoopCross(l) => format!(
                "the `on_cross` bundle of loop `{}` at `{}`, which fires only on a crossing \
                 the path does not make",
                l.id, site.path
            ),
            // The two DAG roots reach this arm when their owning quest is
            // OPTIONAL (spec-0051 §8.6), or when this path never plays the beat
            // at all (a branch the path does not take) — on a path that plays a
            // mandatory beat it is forced and a forced event carries no blame.
            // Naming the quest is the whole value: "a beat nobody has to play" is
            // unactionable, and "the completion of optional quest `quest/crypt`"
            // sends the author to the strand that laid the footing.
            EffectRoot::ObjectiveComplete { quest, objective }
                if !path.obj_step.contains_key(objective) =>
            {
                format!("the `{objective}` bundle of quest `{quest}`, which this path never plays")
            }
            EffectRoot::ObjectiveComplete { quest, objective } if optional.contains(quest) => {
                format!(
                    "the `{objective}` bundle of optional quest `{quest}`, which the party may \
                     never play"
                )
            }
            // On the path, mandatory, and still unforced: the line's own gate,
            // or one enclosing it, does not hold where the path reaches it (or
            // holds only on a value no ordered walk can date).
            EffectRoot::ObjectiveComplete { objective, .. } => format!(
                "the line at `{}` of the `{objective}` bundle, whose gate does not hold where \
                 this path plays it",
                site.path
            ),
            EffectRoot::QuestComplete(q) if !path.quests.contains(q.id.as_str()) => format!(
                "the completion of quest `{}`, which this path never plays",
                q.id
            ),
            EffectRoot::QuestComplete(q) if optional.contains(q.id.as_str()) => format!(
                "the completion of optional quest `{}`, which the party may never play",
                q.id
            ),
            EffectRoot::QuestComplete(q) => format!(
                "the line at `{}` of quest `{}`'s completion, whose gate does not hold where \
                 this path plays it",
                site.path, q.id
            ),
            // A trigger this path never performs. Only its openings are unforced,
            // and an unforced opening is dropped before it is blamed; worded
            // rather than `unreachable!()` so a later change cannot panic here.
            EffectRoot::Trigger(t) => format!(
                "the effects of trigger `{}`, which the critical path never performs",
                t.id
            ),
            EffectRoot::AssemblyLand(m) => format!(
                "a strike `on_land` bundle of assembly `{}` at `{}`, which fires only if a \
                 blow lands on a player who stood in its reach",
                m.id, site.path
            ),
        };
        let (fire_step, forced) = firing_of(site, path, &optional);
        // The three spellings of one write. A gate names a prefab gate anchor and
        // takes that anchor's box and its `replace`-filtered clear; a
        // `fill-region`/`clear-region` names its own anchor-centred box and clears
        // it outright; an `open-way` names a placed piece's exported way and takes
        // its cells, its block and its direction from the piece's metadata. None
        // of the three owns the model.
        //
        // A list rather than one box, because a way is a region with as many
        // boxes as the contract gave it, and each is written by its own `fill`.
        let resolved: Vec<ResolvedWrite> =
            match (e.gate_region_write(), e.region_write(), e.way_write()) {
                (Some((anchor, fills)), _, _) => gate_region_block_any(anchors, anchor.as_str())
                    .map(|(from, to, gate_block)| {
                        vec![if fills {
                            (
                                (from, to),
                                RegionWrite::of_block(&gate_block),
                                Some(gate_block),
                            )
                        } else {
                            ((from, to), RegionWrite::Unseal, None)
                        }]
                    })
                    .unwrap_or_default(),
                (_, Some((zone, block)), _) => zone_box_in(anchors, zone)
                    .map(|r| {
                        vec![match block {
                            Some(b) => (r, RegionWrite::of_block(b), Some(b.to_string())),
                            None => (r, RegionWrite::Clear, None),
                        }]
                    })
                    .unwrap_or_default(),
                // An unresolvable way reference is `DW0547`'s finding, raised by
                // `crate::compiler::ways` before this model is consulted; here it simply
                // contributes nothing, exactly as a dangling anchor does.
                (_, _, Some((piece, name))) => ways
                    .resolve(piece.as_str(), name)
                    .map(|w| {
                        let (write, laid) = match w.sign {
                            crate::compiler::ways::Sign::Laid => {
                                (RegionWrite::of_block(&w.block), Some(w.block.clone()))
                            }
                            crate::compiler::ways::Sign::Cleared => (RegionWrite::Clear, None),
                        };
                        w.boxes.iter().map(|b| (*b, write, laid.clone())).collect()
                    })
                    .unwrap_or_default(),
                _ => return,
            };
        if resolved.is_empty() {
            return; // an unresolvable anchor is DW0142/DW0343/DW0360's finding
        }
        for (region, write, laid) in resolved {
            // A trigger's FILL keeps the treatment it has always had — fired at
            // step 0 and forced, which seals every leg against it. Only its
            // openings are dated by the step that performs it: a wall is assumed
            // up from the start, never assumed down before somebody strikes it.
            let (fire_step, forced) =
                if write.closes() && matches!(site.root, EffectRoot::Trigger(_)) {
                    (0, true)
                } else {
                    (fire_step, forced)
                };
            if !write.closes() && !forced {
                // An optional firing may make a region impassable, never passable — a
                // flood is credited for the same reason a fill is: the proof must
                // survive it.
                continue;
            }
            let ev = if forced {
                RegionEvent::forced(region, write, fire_step)
            } else {
                RegionEvent::unforced(region, write, fire_step, blame())
            };
            out.push(match &laid {
                Some(b) => ev.laying(b),
                None => ev,
            });
        }
    });
    out
}

/// **Every flag write and datum the region model reads off one path**
/// (spec-0088 §4.2): the [`RegionEvents`] beside `writes`, in the path's own
/// step space.
///
/// The flags are collected by the walk [`collect_region_events`] takes, and
/// dated by the same [`firing_of`]:
///
/// - a `set-flag` effect at any root: [`firing_of`]'s step and forcedness;
/// - a dialogue option's `set-flag`: unforced at step 0 — a button can be
///   pressed whenever its own gate holds, which no step bounds — and, for the
///   option a `talk-to` on this path takes, forced at that objective's step too;
/// - a trap's or a timed gate's `disarm.sets_flag`: unforced at step 0 — an act
///   nothing forces;
///
/// and a setter in a bundle of a quest this path's world never completes, or an
/// unforced setter of a branch flag this path's world never holds
/// ([`PathFiring::branch_excluded`]), is dropped: the world that takes that
/// alternative is a different path, judged on its own.
///
/// The data are the guaranteed replay's values ([`PathFiring::data_before`]),
/// keyed by critical-path step, plus every datum an unforced firing writes.
pub(crate) fn region_events_of(
    campaign: &Campaign,
    writes: Vec<RegionEvent>,
    path: &PathFiring,
    npcs: &[NpcPlan],
) -> RegionEvents {
    let optional = campaign.quest_plan.content.optional();
    let mut flags: Vec<FlagEvent> = Vec::new();
    let mut unforced_writers: BTreeSet<String> = BTreeSet::new();
    for_each_gate_effect(campaign, &mut |site, e| {
        let set = match &e.verb {
            Verb::SetFlag { flag, .. } => Some(flag.as_str()),
            _ => None,
        };
        let writes_state = e.writes_state().map(|(id, _)| id.as_str().to_string());
        if set.is_none() && writes_state.is_none() {
            return;
        }
        // A bundle of a quest this path's world never completes never fires on
        // it: the world that completes it is a different path, judged on its
        // own.
        let quest = match &site.root {
            EffectRoot::ObjectiveComplete { quest, .. } => Some(*quest),
            EffectRoot::QuestComplete(q) => Some(q.id.as_str()),
            _ => None,
        };
        if quest.is_some_and(|q| !path.quests.contains(q)) {
            return;
        }
        let (fire_step, forced) = firing_of(site, path, &optional);
        if let Some(flag) = set {
            flags.push(FlagEvent {
                flag: flag.to_string(),
                fire_step,
                forced,
            });
        }
        if let Some(id) = writes_state
            && !forced
        {
            unforced_writers.insert(id);
        }
    });
    for npc in npcs {
        for opt in &npc.options {
            for f in &opt.sets_flags {
                flags.push(FlagEvent {
                    flag: f.clone(),
                    fire_step: 0,
                    forced: false,
                });
                for (obj, (who, n)) in &path.talk_taken {
                    if *who == npc.npc_id
                        && i32::try_from(*n).is_ok_and(|n| n == opt.n)
                        && let Some(&step) = path.obj_step.get(obj)
                    {
                        flags.push(FlagEvent {
                            flag: f.clone(),
                            fire_step: step,
                            forced: true,
                        });
                    }
                }
            }
        }
    }
    let disarms = campaign
        .quests
        .content
        .traps
        .iter()
        .filter_map(|t| t.disarm.as_ref().map(|d| d.sets_flag.as_str()))
        .chain(
            campaign
                .quests
                .content
                .timed_gates
                .iter()
                .filter_map(|g| g.disarm.as_ref().map(|d| d.sets_flag.as_str())),
        );
    for f in disarms {
        flags.push(FlagEvent {
            flag: f.to_string(),
            fire_step: 0,
            forced: false,
        });
    }
    // A branch alternative this path's world does not take is set by nothing
    // on this path, whatever root its setters hang off.
    flags.retain(|e| e.forced || !path.branch_excluded.contains(&e.flag));
    let before = path
        .data_before
        .iter()
        .filter_map(|(obj, vals)| Some((*path.obj_step.get(obj)?, vals.clone())))
        .collect();
    RegionEvents {
        writes,
        flags,
        data: DataReplay {
            before,
            end: path.data_end.clone(),
            unforced_writers,
        },
    }
}

/// **The loop half of a path's datum replay and flags** (spec-0086 §5.2 ×
/// spec-0088 §4.1): the data only loops write are dated by the path's exercise
/// steps — each objective step after an exercise, and the end of the path,
/// reads the loop-owned values the exercise left — and every flag an
/// exercise's `on_cross` set is set, forced, at that step. So a loop slab's
/// gate is read by [`crate::compiler::nav::liveness_of`] from the same events
/// every staged gate is, and a loop-owned datum is never an undatable write.
pub(in crate::compiler::plan) fn with_loop_exercises(
    mut events: RegionEvents,
    spliced: &crate::compiler::r#loop::Spliced,
) -> RegionEvents {
    for id in &spliced.owned {
        events.data.unforced_writers.remove(id);
    }
    for ex in &spliced.exercises {
        // The leg that arrives at the exercise step itself is walked before its
        // crossings: it reads the values as they stood, not as they will be.
        let standing = events.data.at(ex.step).clone();
        events.data.before.entry(ex.step).or_insert(standing);
        for (step, vals) in events.data.before.iter_mut() {
            if *step > ex.step {
                for (id, v) in &ex.owned_after {
                    vals.insert(id.clone(), Some(*v));
                }
            }
        }
        for (id, v) in &ex.owned_after {
            events.data.end.insert(id.clone(), Some(*v));
        }
        for f in &ex.flags_after {
            events.flags.push(FlagEvent {
                flag: f.clone(),
                fire_step: ex.step,
                forced: true,
            });
        }
    }
    events
}

/// **When a firing happens, and whether the party can avoid causing it** — read
/// off the site's [`EffectRoot`] and nowhere else.
///
/// One function because it is one reading. [`collect_region_events`] credits the
/// geometry from it and [`collect_way_openings`] states the disposition from it;
/// two copies of this match would be two instruments that agree until the day a
/// root is added to one of them.
///
/// - a quest `on_objective_complete` fires at that objective's step, an
///   `on_complete` at the quest's completion step — the player is *forced*
///   through both on a path that plays them; a path that never performs the
///   objective, or never completes the quest (a branch it does not take), is
///   unforced at step 0, whatever the quest's optionality says about the paths
///   that do;
/// - an environment trigger fires at the `trigger` step the path performs it in,
///   forced; one the path never performs is unforced at step 0;
/// - a trap payload and a dialogue-hosted `on_respawn` bundle have no step of
///   their own (a sprung trap, a death), so both are rooted conservatively at
///   step 0, which precedes every leg.
///
/// The **optional** roots — a trap the party may never trip, a death nobody is
/// forced to suffer, an offer nobody is forced to buy, a shortcut opened from its
/// far side — are unforced: every shortcut gate is registered sealed at step 0 so
/// the delve is proven completable with no shortcut ever taken, which is exactly
/// "the party may never fire this bundle".
pub(crate) fn firing_of(
    site: &GateSite<'_>,
    path: &PathFiring,
    optional: &BTreeSet<&str>,
) -> (usize, bool) {
    let fires = path.fires(&site.path);
    match &site.root {
        // Fired at the objective's own step on a path that performs it, and
        // forced there only for a line the path's replay fires with its whole
        // gate decided open. A path that never performs the objective never
        // fires its bundle, so it is unforced there — exactly a trigger the
        // path never performs — whatever the quest's optionality says about the
        // paths that do.
        EffectRoot::ObjectiveComplete { quest, objective } => match path.obj_step.get(*objective) {
            Some(&s) => (s, fires && !optional.contains(*quest)),
            None => (0, false),
        },
        // The same rule over the quest: its completion fires on a path that
        // completes it, and on no other.
        EffectRoot::QuestComplete(q) if path.quests.contains(q.id.as_str()) => (
            quest_complete_step(q, &path.obj_step),
            fires && !optional.contains(q.id.as_str()),
        ),
        EffectRoot::QuestComplete(_) => (0, false),
        // Performed by the path at its own `trigger` step, or not at all: a
        // trigger nobody on the path fires is as optional as a trap nobody
        // springs. A line of it is forced only where its gate holds at that step.
        EffectRoot::Trigger(t) => match path.trigger_step.get(t.id.as_str()) {
            Some(&s) => (s, fires),
            None => (0, false),
        },
        EffectRoot::TrapPayload(_)
        | EffectRoot::DialogueRespawn
        | EffectRoot::ShortcutUnlock
        | EffectRoot::OnDeath
        | EffectRoot::ShopOffer
        | EffectRoot::OnKill(_)
        | EffectRoot::AssemblyLand(_)
        | EffectRoot::LoopCross(_) => (0, false),
    }
}

/// The critical-path step an effect root's bundle fires at on the default path,
/// for a reader that orders writes along it (spec-0086 §4.6): an objective's
/// step, a quest's completion step, and step `0` — preceding everything — for
/// every root with no step of its own.
pub(crate) fn root_step(plan: &Plan, root: &EffectRoot<'_>) -> usize {
    match root {
        EffectRoot::ObjectiveComplete { objective, .. } => {
            plan.objective_steps.get(*objective).copied().unwrap_or(0)
        }
        EffectRoot::QuestComplete(q) => quest_complete_step(q, &plan.objective_steps),
        _ => 0,
    }
}

/// Every `open-way` the campaign writes, with the quest-DAG point it fires at and
/// whether the party is forced to cause it (spec-0042 §2.5).
///
/// The same [`for_each_gate_effect`] walk and the same [`firing_of`] reading the
/// region-write model uses, so an `open-way` nested in a `sequence` step, a trap
/// payload or a shop offer is found by existing rather than by being remembered —
/// and is judged unforced there for the same reason its fill is.
pub(crate) fn collect_way_openings(
    campaign: &Campaign,
    path: &PathFiring,
) -> Vec<crate::compiler::ways::WayOpening> {
    // The same widening as `collect_region_events`, for the same reason: an
    // `open-way` fired from an optional quest is a way the party may never open.
    let optional = campaign.quest_plan.content.optional();
    let mut out = Vec::new();
    for_each_gate_effect(campaign, &mut |site, e| {
        let Some((piece, name)) = e.way_write() else {
            return;
        };
        let (fire_step, forced) = firing_of(site, path, &optional);
        out.push(crate::compiler::ways::WayOpening {
            prefab_id: piece.as_str().to_string(),
            way: name.to_string(),
            path: site.path.clone(),
            stage: site.stage,
            fire_step,
            forced,
        });
    });
    out
}
