//! Anchors: the anchor table, how a named anchor resolves to a world cell, the
//! anchors an area's assembly must provide, and the furniture regions.

use super::*;

/// A resolved anchor (absolute world coords).
pub enum ResolvedAnchor {
    /// A point with optional facing.
    Point {
        /// Absolute position.
        pos: [i32; 3],
        /// Facing keyword, if any.
        facing: Option<String>,
    },
    /// A gate region of `block`.
    Gate {
        /// Absolute min/max corners.
        from: [i32; 3],
        /// The opposite corner.
        to: [i32; 3],
        /// Filling block id.
        block: String,
    },
}

/// **Where the party begins the delve**: the area it starts in, and the cell it
/// stands on — the first area in declaration order that resolves an entry point
/// (spec-0046, through [`AnchorTable::entry_anchor`]).
///
/// The single resolver for the delve's *start*, as [`AnchorTable::entry_anchor`]
/// is the single resolver for an *area's* entry. Everything that needs to know
/// where the party begins asks here: `setworldspawn`, the class-apply teleport,
/// the first-join placement, [`DW_NO_ENTRY_ANCHOR`] — and the leg enumeration in
/// [`build_critical_path`], because the party's first leg begins at this cell.
/// It used to be a private helper inside emission, which is the reason the
/// party's own starting cell was not a member of any leg population.
pub fn resolve_campaign_start(
    areas: &[AreaPlacement],
    anchors: &AnchorTable,
) -> Option<(String, [i32; 3])> {
    areas
        .iter()
        .find_map(|a| match anchors.entry_anchor(&a.area_id) {
            Some(ResolvedAnchor::Point { pos, .. }) => Some((a.area_id.clone(), *pos)),
            _ => None,
        })
}

/// The plan's resolved anchors, **and what the pieces said they were for**.
///
/// A campaign addresses an anchor by name, so for everything a campaign speaks
/// about, a `BTreeMap<(area, name), ResolvedAnchor>` is the whole story — which
/// is why this derefs to one and every consumer that knows the name it wants
/// reads it exactly as before.
///
/// The entry point is the one place a campaign never names: the compiler has to
/// *find* it. Finding it by matching a spelling made it a fact about the
/// producer that wrote the piece rather than about the piece, and one producer
/// — the grammar back end, whose anchor keys are always `anchor/<stem>` — could
/// not spell either name at all. So the piece declares a role, this table
/// indexes it while the anchors are being resolved, and
/// [`AnchorTable::entry_anchor`] is the one place that decides.
///
/// The index is keyed by role rather than shaped around the entry: a second
/// role costs a variant of [`AnchorRole`] and nothing here, and its
/// one-per-area refusal is the same refusal.
#[derive(Default)]
pub struct AnchorTable {
    at: BTreeMap<(String, String), ResolvedAnchor>,
    /// `(area, role)` → the name of the anchor that declared it. At most one
    /// per pair, by [`DW_TWO_ENTRY_ANCHORS`].
    roles: BTreeMap<(String, AnchorRole), String>,
}

impl std::ops::Deref for AnchorTable {
    type Target = BTreeMap<(String, String), ResolvedAnchor>;

    fn deref(&self) -> &Self::Target {
        &self.at
    }
}

/// Iterating the table is iterating the map — `Deref` does not reach a `for`
/// loop, and every sweep over `plan.anchors` is one.
impl<'a> IntoIterator for &'a AnchorTable {
    type Item = (&'a (String, String), &'a ResolvedAnchor);
    type IntoIter = std::collections::btree_map::Iter<'a, (String, String), ResolvedAnchor>;

    fn into_iter(self) -> Self::IntoIter {
        self.at.iter()
    }
}

impl AnchorTable {
    /// Resolve one anchor a placed piece declared, and record any role it
    /// declared with it.
    ///
    /// First-wins on the position, matching the pool path's own
    /// `or_insert_with`: a pool that seats the same anchor-bearing prefab twice
    /// keeps the first carrier (and raises `DW0498` about it). The role index
    /// follows the same rule for the same name, and refuses a **different**
    /// name claiming a role this area has already given away.
    pub(super) fn declare(
        &mut self,
        area: &str,
        name: &str,
        meta: &AnchorMeta,
        resolve: impl FnOnce() -> ResolvedAnchor,
    ) -> Result<(), PlanError> {
        self.at
            .entry((area.to_string(), name.to_string()))
            .or_insert_with(resolve);
        if let Some(role) = meta.role {
            self.record_role(area, name, role)?;
        }
        Ok(())
    }

    /// Seat one anchor the **derivation** produced — a site plan's synthesized
    /// vocabulary (`crate::compiler::blockout`) and the pieces a detail plan stands in it
    /// (`crate::compiler::detail`).
    ///
    /// **Last-wins, and that is the difference from [`AnchorTable::declare`].**
    /// A prefab-placed area resolves first-wins because a pool can seat the same
    /// anchor-bearing piece twice and the first carrier is the one `DW0498`
    /// reports. A derived area is the other way round on purpose: the derivation
    /// names `anchor/node-…` at the massing's own footing, and a detail piece
    /// standing there is then the truth about where that place is, so it
    /// overwrites. Collapsing the two rules into one would silently move a
    /// detailed place's anchor back onto the massing.
    ///
    /// The role index is the *same* rule in both, because what an anchor is for
    /// is a property of the anchor rather than of the producer. A derived area
    /// cannot reach [`DW_TWO_ENTRY_ANCHORS`]: the derivation is one producer
    /// holding one name-keyed map, so it has nowhere to write a second claim.
    /// The refusal exists for a prefab **library**, where two independently
    /// authored pieces can each claim the same role in one area.
    pub(super) fn place(
        &mut self,
        area: &str,
        name: &str,
        resolved: ResolvedAnchor,
        role: Option<AnchorRole>,
    ) -> Result<(), PlanError> {
        self.at
            .insert((area.to_string(), name.to_string()), resolved);
        if let Some(role) = role {
            self.record_role(area, name, role)?;
        }
        Ok(())
    }

    /// Record that `name` is the anchor `area` gave `role` to — the one place
    /// the role index is written, so the refusal below cannot be bypassed by
    /// arriving through a different producer.
    fn record_role(&mut self, area: &str, name: &str, role: AnchorRole) -> Result<(), PlanError> {
        // A role naming a kind of place (furniture) is held by as many anchors
        // as the pieces declare; only a role naming THE place is indexed here.
        if !role.one_per_area() {
            return Ok(());
        }
        let held = self
            .roles
            .entry((area.to_string(), role))
            .or_insert_with(|| name.to_string());
        if held != name {
            return Err(PlanError::new(
                DW_TWO_ENTRY_ANCHORS,
                format!(
                    "area `{area}` declares the anchor role `{role}` twice: `{held}` and \
                     `{name}` both carry `\"role\": \"{role}\"` in their prefab metadata. An \
                     area has one place the party arrives at, and the compiler will not pick \
                     between two — a silently-chosen spawn is a moved start nothing reports. \
                     Fix it in the prefab metadata that declares the anchors: keep the role \
                     on the one cell the party should arrive at and drop the `role` key from \
                     the other (the anchor itself stays, and content can still bind it by \
                     name). If the two anchors are in two pieces of one `prefab_pool`, only \
                     the piece that seeds the layout should carry it. Take the role off where \
                     it was written: `delvec prefab anchor <nbt> --name <anchor> --pos <x,y,z> \
                     --no-role` for a hand-built or ingested piece, or drop `role` from the \
                     `mark` for a grammar program. Renaming an anchor achieves nothing — the \
                     role is the only thing the compiler reads here"
                ),
            ));
        }
        Ok(())
    }

    /// The name of the anchor `area` gave `role` to, if any.
    pub fn role_name(&self, area: &str, role: AnchorRole) -> Option<&str> {
        self.roles
            .get(&(area.to_string(), role))
            .map(String::as_str)
    }

    /// One area's **entry anchor**: the anchor declaring [`AnchorRole::Entry`].
    ///
    /// **The single resolver, and the role is the whole of it.** Every consumer
    /// of an entry point reaches it through this or through the [`Plan`] methods
    /// below, and none matches an anchor name: the three that once did
    /// (inter-area transport, the POV shot planner, the trap-safety start set)
    /// each asked an honest question about the wrong key and got an honest
    /// `None`, so every area whose tileset spelled the anchor differently was
    /// silently never transported into, never framed and never counted as a
    /// place a player can start from. A spelling is a fact about the producer
    /// that wrote the piece; the role is a fact about the piece, and it is what
    /// every producer writes — `mark { role }` on a grammar program,
    /// `delvec prefab anchor --role` on a hand-built or ingested one, and the
    /// derivation's own [`crate::compiler::blockout`] anchor.
    pub fn entry_anchor(&self, area: &str) -> Option<&ResolvedAnchor> {
        self.entry_anchor_name(area)
            .and_then(|name| self.at.get(&(area.to_string(), name.to_string())))
    }

    /// The **name** of `area`'s entry anchor, resolved the same way — for the
    /// gate-deadlock proof, which reads its start node out of prefab metadata
    /// rather than out of the resolved map and must not re-spell the question.
    pub fn entry_anchor_name(&self, area: &str) -> Option<String> {
        self.role_name(area, AnchorRole::Entry).map(str::to_string)
    }

    /// Every area that provides `name`, in `BTreeMap` order.
    ///
    /// The question the by-name lookups never asked. **The scope of uniqueness
    /// for an anchor name is the AREA** (`DW0857`), so more than one provider
    /// means the name *alone* does not pick out a place in the world — and a
    /// lookup that answers anyway has asked an honest question about the wrong
    /// object.
    pub fn providers(&self, name: &str) -> Vec<&str> {
        self.at
            .keys()
            .filter(|(_, n)| n == name)
            .map(|(a, _)| a.as_str())
            .collect()
    }

    /// **The one authority for what an anchor reference means.**
    ///
    /// Resolution is *scoped*, because a name is an identity within an area and
    /// nowhere wider. The rule, and it is the DSL tier's own rule rather than a
    /// new one — `dsl::validate` resolves every reference against the anchors of
    /// the quest's own area and makes exactly one exception, a camera, which may
    /// fly anywhere:
    ///
    /// 1. A reference that belongs to an area resolves **in that area**. This is
    ///    the step the by-name lookups skipped, and it is the whole defect: the
    ///    area was always in the key and the lookup threw it away.
    /// 2. Otherwise the name may still cross — **what is refused is the
    ///    ambiguity, not the crossing** — so a name exactly one area provides
    ///    resolves from anywhere, exactly as it always did.
    /// 3. A name more than one *other* area provides is [`AnchorHit::Ambiguous`].
    ///    Nothing an author can see says which building is meant, and picking
    ///    the first would settle it by whichever area id sorts first.
    ///
    /// A caller that goes through this function never sees a guess: the
    /// ambiguous arm is [`AnchorHit::Ambiguous`], which the cast path raises as
    /// [`crate::compiler::gates::DW_ANCHOR_AMBIGUOUS`].
    ///
    /// **That is not yet true of the compiler as a whole, and the gap is stated
    /// rather than implied.** One site raises `DW0859` today. The remaining
    /// by-name helpers — `point_any`, `zone_box`, `gate_region_block_any` and
    /// the emitter's `anchor_point_any` — still take the first match across
    /// areas, because the objects they resolve for have no area to be scoped to:
    /// measured from `schema --stage all`, **exactly one anchor-bearing object
    /// schema (`Npc`) carries an `area`** — the half that reproduces on every
    /// counting basis tried. **The denominator does not, and saying so is the
    /// point:** an independent round counting from the same export got 21 named
    /// schemas carrying a typed reference, 46 counting inline sub-schemas and 41
    /// reaching one transitively, and no basis it tried reproduces a single
    /// agreed total. The claim is therefore stated at the strength of the
    /// evidence — the fact that holds, not a number two methods disagree about.
    /// Closing the rest is a DSL surface question — those objects must
    /// first be able to say which area they belong to — not a compiler one, so
    /// it is a version-ledger change and is deliberately not made here.
    pub fn resolve(&self, scope: AnchorScope<'_>, name: &str) -> AnchorHit<'_> {
        // 1. The referring area owns the name if it provides it.
        if let AnchorScope::Area(area) = scope
            && let Some((key, anchor)) =
                self.at.get_key_value(&(area.to_string(), name.to_string()))
        {
            return AnchorHit::Found {
                area: key.0.as_str(),
                anchor,
            };
        }
        // 2/3. Crossing is allowed while it is unambiguous.
        let mut across = self.at.iter().filter(|((_, n), _)| n == name);
        match (across.next(), across.next()) {
            (None, _) => AnchorHit::Missing,
            (Some(((area, _), anchor)), None) => AnchorHit::Found {
                area: area.as_str(),
                anchor,
            },
            (Some(((a, _), _)), Some(((b, _), _))) => {
                let mut areas = vec![a.as_str(), b.as_str()];
                areas.extend(across.map(|((x, _), _)| x.as_str()));
                AnchorHit::Ambiguous(areas)
            }
        }
    }

    /// [`AnchorTable::resolve`] reduced to a cell, for the callers that only
    /// want a position. An ambiguous reference yields `None` rather than a
    /// guess — the campaign carrying one is refused at validation, so this arm
    /// is unreachable in a build.
    pub fn point_scoped(&self, scope: AnchorScope<'_>, name: &str) -> Option<[i32; 3]> {
        match self.resolve(scope, name) {
            AnchorHit::Found { anchor, .. } => Some(match anchor {
                ResolvedAnchor::Point { pos, .. } => *pos,
                ResolvedAnchor::Gate { from, .. } => *from,
            }),
            _ => None,
        }
    }
}

/// The scope an anchor reference resolves in.
///
/// Not a qualifier an author writes — that was considered and refused, because
/// an author naming which area they meant *is* the area-scoped resolution the
/// compiler should have been doing all along. The scope is derived from the
/// referring object: a quest's area, an NPC's area, or the absence of one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnchorScope<'a> {
    /// The reference belongs to this area and resolves there first.
    Area(&'a str),
    /// Global **by design**, matching the DSL tier's own two exceptions: an
    /// environment trigger (triggers are global) and a cutscene camera (a camera
    /// legitimately flies across areas). Such a reference has no area to be
    /// scoped to, so an ambiguous name is refused rather than guessed.
    Global,
}

/// What [`AnchorTable::resolve`] found.
///
/// Deliberately not `Debug`: printing a hit would mean printing a
/// [`ResolvedAnchor`], and a derived `Debug` on a type the emitter reads is how
/// a content key acquires a field nobody meant to put in it.
pub enum AnchorHit<'a> {
    /// Exactly one place is meant, and this is the area it lives in.
    Found {
        /// The area the name resolved in — the referring area when it provided
        /// the name, otherwise the single area that does.
        area: &'a str,
        /// The resolved anchor itself.
        anchor: &'a ResolvedAnchor,
    },
    /// More than one area provides the name and the reference's own scope does
    /// not settle it. The areas, in `BTreeMap` order, for the diagnostic.
    Ambiguous(Vec<&'a str>),
    /// No placed piece provides the name at all (`DW0142` / `DW0360`).
    Missing,
}

impl<'a> Plan<'a> {
    /// One area's **entry point** — the cell a body arrives at when it enters
    /// this area — resolved through [`AnchorTable::entry_anchor`].
    ///
    /// The single place a consumer asks "where does the party start here". Every
    /// consumer goes through this, [`Plan::entry_point_facing`] or
    /// [`Plan::entry_points`]; none matches an anchor name itself. A gate anchor
    /// is not an entry point (there is no cell to stand on), so it resolves to
    /// `None` rather than to a plane.
    pub fn entry_point(&self, area: &str) -> Option<[i32; 3]> {
        match self.anchors.entry_anchor(area) {
            Some(ResolvedAnchor::Point { pos, .. }) => Some(*pos),
            _ => None,
        }
    }

    /// **Where the party begins the delve** — the area it starts in and the cell
    /// it stands on — through [`resolve_campaign_start`].
    pub fn campaign_start(&self) -> Option<(String, [i32; 3])> {
        resolve_campaign_start(&self.areas, &self.anchors)
    }

    /// One area's entry point with the facing it was declared with — the POV
    /// planner needs the direction as well as the cell.
    pub fn entry_point_facing(&self, area: &str) -> Option<([i32; 3], Option<String>)> {
        match self.anchors.entry_anchor(area) {
            Some(ResolvedAnchor::Point { pos, facing }) => Some((*pos, facing.clone())),
            _ => None,
        }
    }

    /// **Every** area's entry point, in area order — the cells a body can begin
    /// a walk from.
    ///
    /// The sweep half of the question, for the consumer that wants the whole
    /// start set rather than one area's (the trap-safety proof roots its
    /// disarm-reachability search here). It exists so that consumer cannot ask
    /// the question itself: it used to sweep the anchor map for a literal name,
    /// which is how every island-tileset area went uncounted.
    pub fn entry_points(&self) -> impl Iterator<Item = [i32; 3]> + '_ {
        self.areas
            .iter()
            .filter_map(|area| self.entry_point(&area.area_id))
    }

    /// [`Self::point_any`] with the area that answered: the first `(area, name)`
    /// in the anchor table's order whose name is `anchor`.
    pub fn point_any_site(&self, anchor: &str) -> Option<(String, [i32; 3])> {
        self.anchors
            .iter()
            .find(|((_, n), _)| n == anchor)
            .map(|((area, _), resolved)| {
                (
                    area.clone(),
                    match resolved {
                        ResolvedAnchor::Point { pos, .. } => *pos,
                        ResolvedAnchor::Gate { from, .. } => *from,
                    },
                )
            })
    }

    /// Resolve `(area, anchor)` to a point position, if it is a point anchor.
    pub fn point(&self, area_id: &str, anchor: &str) -> Option<[i32; 3]> {
        match self.anchors.get(&(area_id.to_string(), anchor.to_string())) {
            Some(ResolvedAnchor::Point { pos, .. }) => Some(*pos),
            _ => None,
        }
    }

    /// Resolve an anchor **by name alone**, across areas — the area-agnostic
    /// lookup `open-gate` / `move-npc` destinations / actor spawns already use.
    /// `Point` yields its cell, `Gate` its `from` corner; `None` when no placed
    /// piece provides the name. First match in `anchors` order (a `BTreeMap`, so
    /// deterministic).
    pub fn point_any(&self, anchor: &str) -> Option<[i32; 3]> {
        point_any_in(&self.anchors, anchor)
    }
}

/// **Every place the campaign says the party has to reach**, resolved
/// (spec-0042 §2.5), for the way-disposition gate to judge against a piece's
/// contract.
///
/// The set is [`required_anchors_for_area`]'s — the anchors the layout solver is
/// already required to guarantee, which is this engine's existing definition of
/// "a campaign reference to a place": NPC stands, `reach-anchor` / `collect` /
/// `interact` targets, wave spawns and lane waypoints, every anchor-bearing
/// effect. Re-deriving a narrower list here would be a second enumeration that
/// drifts; a wider one would judge places nobody is required to visit.
///
/// An element carries the step it must be reachable BY when the campaign orders
/// it — an objective's own critical-path step. Everything else carries none and
/// is judged on the weaker half alone: it must be behind a way something forces
/// open, in no particular order. That asymmetry is the honest one — a body placed
/// at world load has no step, and inventing one would order a thing the campaign
/// never ordered.
pub(super) fn collect_required_elements(
    campaign: &Campaign,
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
    objective_steps: &BTreeMap<String, usize>,
) -> Vec<crate::compiler::ways::RequiredElement> {
    // anchor name → the earliest objective that targets it, with its step. The
    // earliest, because an anchor two objectives share must be reachable by the
    // time the FIRST of them asks the party to stand there.
    let mut by_objective: BTreeMap<&str, (&str, usize)> = BTreeMap::new();
    for quest in &campaign.quests.content.quests {
        for objective in &quest.objectives {
            let anchor = match objective {
                Objective::ReachAnchor { anchor, .. }
                | Objective::Collect { anchor, .. }
                | Objective::Interact { anchor, .. } => anchor.as_str(),
                Objective::Kill { .. } | Objective::TalkTo { .. } => continue,
            };
            let Some(step) = objective_steps.get(objective.id().as_str()).copied() else {
                continue;
            };
            let entry = by_objective
                .entry(anchor)
                .or_insert((objective.id().as_str(), step));
            if step < entry.1 {
                *entry = (objective.id().as_str(), step);
            }
        }
    }
    let mut out = Vec::new();
    for area in &campaign.world.content.areas {
        let area_id = area.id.as_str();
        for name in required_anchors_for_area(campaign, area_id) {
            let Some(resolved) = anchors.get(&(area_id.to_string(), name.clone())) else {
                continue;
            };
            let pos = match resolved {
                ResolvedAnchor::Point { pos, .. } => *pos,
                ResolvedAnchor::Gate { from, .. } => *from,
            };
            let (what, by_step) = match by_objective.get(name.as_str()) {
                Some((oid, step)) => (
                    format!("objective `{oid}` (at anchor `{name}`)"),
                    Some(*step),
                ),
                None => (format!("the campaign reference to anchor `{name}`"), None),
            };
            out.push(crate::compiler::ways::RequiredElement {
                what,
                area_id: area_id.to_string(),
                pos,
                by_step,
            });
        }
    }
    out
}

/// The set of anchor names the campaign references inside `area_id`: NPC stands
/// (NPCs in this area), `reach-anchor` targets and `open-gate` anchors (quests
/// planned in this area). Sorted + deduped for deterministic solver input. These
/// are the anchors the solver must guarantee exist in the assembled layout.
///
/// Reachable outside this module because it is the engine's definition of *a
/// campaign reference the layout must honour*, and two passes now rest on it:
/// this one, which hands the set to the solver, and
/// [`crate::compiler::guarantee`], which reports at validation which anchors an
/// area therefore guarantees. Re-deriving it there would be exactly the second
/// enumeration this function's own consumers already refuse to write.
pub(crate) fn required_anchors_for_area(campaign: &Campaign, area_id: &str) -> Vec<String> {
    let mut set: BTreeSet<String> = BTreeSet::new();
    for npc in &campaign.npcs.content.npcs {
        if npc.area.as_str() == area_id {
            set.insert(npc.anchor.as_str().to_string());
        }
    }
    // Which planned quests belong to this area.
    let quest_area: BTreeMap<&str, &str> = campaign
        .quest_plan
        .content
        .quests
        .iter()
        .map(|q| (q.id.as_str(), q.area.as_str()))
        .collect();
    for q in &campaign.quests.content.quests {
        if quest_area.get(q.id.as_str()).copied() != Some(area_id) {
            continue;
        }
        for o in &q.objectives {
            match o {
                Objective::ReachAnchor { anchor, .. } | Objective::Collect { anchor, .. } => {
                    set.insert(anchor.as_str().to_string());
                }
                Objective::Interact { anchor, .. } => {
                    set.insert(anchor.as_str().to_string());
                }
                // Wave spawn anchors are registered below via `wave_area`, driven
                // by the `spawn-wave` effect (the true spawn site) rather than the
                // `kill` objective — so a kill-less live-threat wave is placed too.
                Objective::Kill { .. } | Objective::TalkTo { .. } => {}
            }
            // v0.8: an adopted container is a piece of hardware the
            // objective cannot do without — a pool draw that omits its carrier
            // leaves the collect with nothing to fill, so it joins the required
            // set exactly as a lane waypoint does. Absent field adds nothing.
            if let Some(cont) = o.collect_container() {
                set.insert(cont.as_str().to_string());
            }
        }
        for e in q
            .on_objective_complete
            .values()
            .flatten()
            .chain(&q.on_complete)
        {
            collect_effect_anchors(e, &mut set);
        }
    }
    // Wave spawn anchors: a `spawn-wave` effect materializes its mobs at the wave's
    // `anchor` in the area of the quest (or single-area trigger) that fires it —
    // independent of any `kill` objective. Register the anchor for that area so the
    // solver guarantees a piece providing it; a kill-less live-threat wave would
    // otherwise resolve no spawn position and its `spawn_<wave>` call would dangle.
    for w in &campaign.quests.content.waves {
        if wave_area(campaign, w.id.as_str()) == Some(area_id) {
            set.insert(w.anchor.as_str().to_string());
            // spec-0016 §6: a TD lane's waypoints are places the squad has to
            // reach, so the solver must guarantee a piece providing each one —
            // exactly like the wave's own spawn anchor. Without this a pool area
            // simply may not draw the piece carrying a waypoint, and the lane
            // fails DW0386 ("resolves nowhere") for a reason the author cannot
            // act on: the anchor IS in the pool, the layout just did not use it.
            if let Some(lane) = &w.lane {
                set.extend(lane.waypoints.iter().map(|a| a.as_str().to_string()));
            }
        }
    }
    // Environment triggers (v0.4) are global. When the campaign has a single area,
    // their `at` and effect anchors must be provided by that area's assembly. For
    // a multi-area campaign, a trigger anchor is expected to coincide with an
    // objective anchor (already required above); over-provisioning every area is
    // avoided so the solver is not asked for an anchor an area's pool cannot fit.
    if campaign.world.content.areas.len() == 1 {
        for t in &campaign.quests.content.triggers {
            if let Some(at) = t.at_anchor() {
                set.insert(at.to_string());
            }
            for e in &t.effects {
                collect_effect_anchors(e, &mut set);
            }
        }
    }
    set.into_iter().collect()
}

/// Collect the anchors a v0.4 quest effect references (`open-gate`, `set-block`,
/// `move-npc` target, `cutscene` waypoints) into `set`, so the layout solver
/// guarantees they exist in the assembled area.
fn collect_effect_anchors(e: &QuestEffect, set: &mut BTreeSet<String>) {
    if let Some(a) = e.open_gate_anchor() {
        set.insert(a.as_str().to_string());
    }
    if let Some(a) = e.close_gate_anchor() {
        set.insert(a.as_str().to_string());
    }
    if let Some((a, _)) = e.set_block() {
        set.insert(a.as_str().to_string());
    }
    if let Some((_, to)) = e.move_npc() {
        set.insert(to.anchor.as_str().to_string());
    }
    // Every shot's waypoints, plus each shot's `look_at` subject — the camera is
    // aimed at that world point, so the area's assembly must provide its anchor.
    if let Some(shots) = e.cutscene_shots() {
        for shot in &shots {
            for w in &shot.path {
                set.insert(w.anchor.as_str().to_string());
            }
            if let Some(t) = &shot.look_at {
                set.insert(t.anchor.as_str().to_string());
            }
        }
    }
}

/// Resolve a placed-piece anchor to absolute world coords (transforming through
/// the piece's pos + rotation).
pub(super) fn resolve_piece_anchor(
    placed: &solver::PlacedPiece,
    meta: &PrefabMeta,
    name: &str,
    am: &AnchorMeta,
) -> ResolvedAnchor {
    if let Some(gate) = local_gate(meta, name, am) {
        ResolvedAnchor::Gate {
            from: solver::transform_point(placed, gate.from),
            to: solver::transform_point(placed, gate.to),
            block: gate.block,
        }
    } else {
        ResolvedAnchor::Point {
            pos: solver::transform_point(placed, anchor_point(am)),
            facing: solver::transform_facing(placed, am.facing.as_deref()),
        }
    }
}

/// The piece-local gate box an anchor names, asked of the ONE authority
/// ([`PrefabMeta::gate_anchor`]) rather than read off `region`/`block` here.
///
/// Both resolvers go through this and through nothing else, so the piece-local
/// answer is computed once and the two differ only in how they carry it into
/// world space — which is the entire difference between a placed piece and a
/// single-prefab area, and the only difference there should ever have been.
///
/// A gate the authority REFUSES keeps the reading it has always had. The refusal
/// is a validation finding (`DW0343`) that names the anchor and says why, and a
/// campaign carrying one does not build; making the planner re-read it as a point
/// as well would move the emission of a campaign that is being refused anyway,
/// for no reader's benefit.
fn local_gate(meta: &PrefabMeta, name: &str, am: &AnchorMeta) -> Option<GateAnchor> {
    match meta.gate_anchor(name) {
        Ok(gate) => gate,
        // Furniture is never refused as a gate (the authority answers `None`), so
        // this arm cannot reach one; the guard says so where a region is read.
        Err(_) if am.role == Some(AnchorRole::Furniture) => None,
        Err(_) => am.region.as_ref().map(|r| GateAnchor {
            from: r.from,
            to: r.to,
            block: am
                .block
                .clone()
                .unwrap_or_else(|| "minecraft:air".to_string()),
        }),
    }
}

pub(super) fn resolve_anchor(
    origin: [i32; 3],
    meta: &PrefabMeta,
    name: &str,
    am: &AnchorMeta,
) -> ResolvedAnchor {
    let add = |p: [i32; 3]| [origin[0] + p[0], origin[1] + p[1], origin[2] + p[2]];
    if let Some(gate) = local_gate(meta, name, am) {
        ResolvedAnchor::Gate {
            from: add(gate.from),
            to: add(gate.to),
            block: gate.block,
        }
    } else {
        ResolvedAnchor::Point {
            pos: add(anchor_point(am)),
            facing: am.facing.clone(),
        }
    }
}

/// One placed furniture region: `(anchor name, inclusive world box)` (spec-0065).
pub type FurnitureRegion = (String, ([i32; 3], [i32; 3]));

/// **Every furniture region the placed pieces declare**, in world space
/// (spec-0065 §4.1).
///
/// Read off the placed pieces rather than off the anchor table, and that is the
/// point: the table resolves a name first-wins, so a pool that seats one
/// furnished piece twice would lose the second table — and the second table's
/// blocks are in the world all the same. Detail pieces are placed pieces too, so
/// they are reached by the same walk. Order: area, placed piece, anchor name
/// (the prefab document's map is a `BTreeMap`).
pub(super) fn collect_furniture(
    areas: &[AreaPlacement],
    prefabs: &PrefabRegistry,
) -> Vec<FurnitureRegion> {
    let mut out = Vec::new();
    for area in areas {
        for piece in &area.pieces {
            let Some(meta) = prefabs.get(&piece.prefab_id) else {
                continue;
            };
            for (name, am) in &meta.anchors {
                if am.role != Some(AnchorRole::Furniture) {
                    continue;
                }
                // The role with no region is `DW0888`'s first shape, refused by
                // the byte-claim check; there is nothing here to exclude.
                let Some(region) = &am.region else {
                    continue;
                };
                let world = |local: [i32; 3]| {
                    let t = piece.rotation.transform(local);
                    [
                        piece.pos[0] + t[0],
                        piece.pos[1] + t[1],
                        piece.pos[2] + t[2],
                    ]
                };
                let (a, b) = (world(region.from), world(region.to));
                let lo = [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])];
                let hi = [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])];
                out.push((name.clone(), (lo, hi)));
            }
        }
    }
    out
}

/// **The one cell a non-gate anchor names**, piece-local: its `pos`, or — for a
/// named place declared as a region, such as furniture (spec-0065 §3.4) — the
/// region's `from` corner, which is the cell every region anchor already
/// resolves to when a point is asked of it ([`anchor_node`]).
pub(super) fn anchor_point(am: &AnchorMeta) -> [i32; 3] {
    am.pos
        .or_else(|| am.region.as_ref().map(|r| r.from))
        .unwrap_or([0, 0, 0])
}

/// Resolve an anchor name to a point cell by scanning every area's resolved
/// anchors (first match), mirroring the emitter's `anchor_point_any`.
pub(crate) fn point_any(
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
    name: &str,
) -> Option<[i32; 3]> {
    for ((_, n), resolved) in anchors {
        if n == name {
            return match resolved {
                ResolvedAnchor::Point { pos, .. } => Some(*pos),
                ResolvedAnchor::Gate { from, .. } => Some(*from),
            };
        }
    }
    None
}

/// The absolute gate region **and fill block** a gate anchor resolves to. `None`
/// if the anchor is not a gate region.
///
/// The block is not an extra the callers happen to want: a `close-gate` is a
/// region write like any other, and what a write leaves behind is decided by what
/// it writes ([`RegionWrite::of_block`]) — a gate anchor declaring a fluid seals
/// nothing a body can stand on. Resolving the region without the block is what let
/// that conclusion be assumed instead of derived, so there is deliberately no
/// region-only variant of this lookup.
pub(crate) fn gate_region_block_any(
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
    name: &str,
) -> Option<([i32; 3], [i32; 3], String)> {
    for ((_, n), resolved) in anchors {
        if n == name
            && let ResolvedAnchor::Gate { from, to, block } = resolved
        {
            return Some((*from, *to, block.clone()));
        }
    }
    None
}

/// Resolve an anchor by name alone over a resolved-anchor map — the free-function
/// core of [`Plan::point_any`], so the planning stage can resolve a box *while*
/// building the `Plan` (which is where the region-write model is collected) rather
/// than needing a finished one.
pub(crate) fn point_any_in(
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
    anchor: &str,
) -> Option<[i32; 3]> {
    anchors.iter().find_map(|((_, name), resolved)| {
        (name == anchor).then_some(match resolved {
            ResolvedAnchor::Point { pos, .. } => *pos,
            ResolvedAnchor::Gate { from, .. } => *from,
        })
    })
}
