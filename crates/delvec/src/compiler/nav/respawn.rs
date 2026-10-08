//! The respawn-point safe zone: no cell the party comes back to life on is
//! inside a hostile's aggro range (`DW0478`).

use super::*;
use crate::compiler::failure::Failure;
use crate::compiler::plan::Plan;
use delvewright_dsl::Verb;
use std::collections::BTreeMap;

/// One hostile force as the bonfire safe-zone proof (`DW0478`) sees it: a
/// perception radius plus every cell the force provably occupies.
#[derive(Clone, Debug)]
pub struct AggroSource {
    /// What the message calls it (`wave/gate-assault`, `actor/barrow-warden`).
    pub id: String,
    /// The perception radius, in blocks — the declared `follow_range`, a lane's
    /// `aggro_radius` (which the compiler emits AS `follow_range`), or
    /// [`DEFAULT_FOLLOW_RANGE`].
    pub radius: f64,
    /// Why this radius is the number it is, for the message.
    pub radius_source: &'static str,
    /// Every occupied cell: what it is, where it is, and the extra reach margin
    /// added on top of `radius` — `0.0` for a stationary cell (seated spawn,
    /// staging anchor), [`LANE_MARCH_DRIFT`] for a lane path cell, because the
    /// squad marches a corridor around the polyline, not the polyline itself.
    pub cells: Vec<(&'static str, [i32; 3], f64)>,
}

/// `DW0478`: **no respawn point may sit inside any hostile's aggro range**
/// (spec-0016 §1).
///
/// A **respawn point** is every resolved [`crate::compiler::plan::CheckpointPlan`] — a
/// `bonfire` and a plain `set-checkpoint` alike. The proof is about the cell a
/// dead player materialises on, and vanilla returns them to both by the same
/// `spawnpoint` mechanism, so keying it to the `rest` flag examined one variant
/// of a sum type and silently skipped its sibling (see [`DW_RESPAWN_IN_AGGRO`]).
///
/// The rule, verbatim: for every wave / actor hostile, the distance from the
/// respawn cell to that hostile's spawn cell — or to any cell of its lane path —
/// must EXCEED that hostile's `follow_range` (the declared attribute; the
/// documented default when undeclared). For a **lane path cell** the term is
/// `follow_range + `[`LANE_MARCH_DRIFT`]: the squad
/// marches a measured corridor around the polyline, so the centre-line distance
/// understates its real aggro reach. Stationary cells keep the plain term.
///
/// What "occupies" means per force:
/// * a plain wave — its DW0312-proven seated spawn cells (where the datapack
///   actually summons it, not where its anchor is);
/// * an `aggro-edge` wave — the same, which for it is its perception ring;
/// * a **lane** wave — those cells PLUS the marched polyline: every cell of every
///   A*-proven leg from the form-up point through the waypoints, because a lane
///   wave's whole design is that it walks that corridor while the party is
///   elsewhere — and each of those cells carries the [`LANE_MARCH_DRIFT`]
///   margin, because the squad's measured march is a corridor around the
///   polyline, not the polyline. This is the shape that killed the drowned
///   bell's ladder bot: a re-seated gate squad marched its lane, and the lane
///   ended a couple of blocks outside a bonfire the party had just rested at;
/// * an **actor** the campaign declares as a fighter — `unleash-actor`ed
///   somewhere, or staged `vulnerable` — at its staging anchor. Fighter-ness is
///   read off the campaign's own declarations and never guessed from the species:
///   the pinned entity registry is a membership set with no mob-category data
///   (the same rule `DW0469` is built on), so the compiler cannot and does not
///   ask whether `minecraft:sheep` is a monster.
///
/// Radius per force: a lane's `aggro_radius` (emitted verbatim as each lane mob's
/// `follow_range`), else the largest declared `follow_range` among its mobs, else
/// [`DEFAULT_FOLLOW_RANGE`] — one documented number, never a per-species table the
/// compiler would have to invent (`DW0475`'s rule).
///
/// A campaign with no respawn point at all, or with no hostile force at all,
/// proves nothing here — and [`RespawnSafetyLedger`] says so out loud rather than
/// returning a silent `Ok`.
pub fn check_respawn_safe_zone(
    plan: &Plan,
    world: &World,
    placements: &BTreeMap<String, Vec<[i32; 3]>>,
    lanes: &LaneRoutes,
) -> Result<RespawnSafetyLedger, Failure> {
    let reign_ends = plan.respawn_reign_ends();
    let rest_points: Vec<RestPoint> = plan
        .checkpoints
        .iter()
        .zip(&reign_ends)
        .map(|(c, end)| RestPoint {
            index: c.index,
            anchor: c.anchor.clone(),
            kind: if c.rest { "bonfire" } else { "set-checkpoint" },
            pos: c.pos,
            fire_step: c.fire_step,
            reign_end: *end,
        })
        .collect();
    let sources = aggro_sources(plan, world, placements, lanes);
    let evidence = crate::compiler::respawn::Evidence::build(plan, world);
    let table = respawn_evidence_table(plan, &evidence, &rest_points, &sources);
    let onsets: BTreeMap<String, usize> = sources
        .iter()
        .map(|s| (s.id.clone(), evidence.onset(&s.id).step))
        .collect();
    let ledger = RespawnSafetyLedger::new(&rest_points, &sources, &onsets, &table);
    if ledger.pairs == 0 {
        return Ok(ledger);
    }
    let violations = respawn_violations(&rest_points, &sources, &onsets, &table);
    if let Some(err) = respawn_error(&violations) {
        return Err(err);
    }
    Ok(ledger)
}

/// Whether a hostile force can be in the world while a respawn point still
/// governs where a dead player lands.
///
/// Both halves are the campaign's own declarations: the force's onset is the
/// earliest beat that stages it, bounded by its gates and its bearer
/// ([`crate::compiler::respawn::Evidence::onset`]), and the respawn
/// point's reign ends when a later `set-checkpoint` replaces it
/// ([`Plan::respawn_reign_ends`]). A bonfire never stops reigning, so every
/// bonfire is compared against every force exactly as before.
///
/// This is not a relaxation of the geometry — the clearance demanded of an
/// overlapping pair is unchanged, to the block. It is what makes the proof about
/// a *respawn point* rather than about a bonfire: a plain checkpoint is
/// superseded, so a body staged two quests after it was retired can no more meet
/// the party there than a body in another campaign can.
fn contemporaneous(rest: &RestPoint, hostile_onset: usize) -> bool {
    rest.reign_end.is_none_or(|end| hostile_onset < end)
}

/// One cell a dead player can materialise on, as the `DW0478` proof sees it.
#[derive(Clone, Debug)]
pub struct RestPoint {
    /// The [`crate::compiler::plan::CheckpointPlan::index`] this stands for — the key every
    /// evidence route is asked against, so a credit names the same object the
    /// ledger row does.
    pub index: usize,
    /// The checkpoint anchor name.
    pub anchor: String,
    /// `"bonfire"` or `"set-checkpoint"` — recorded so a reader can see WHICH
    /// respawn points were examined, and notice at a glance if one kind is
    /// missing from a campaign that has them.
    pub kind: &'static str,
    /// The resolved absolute respawn cell.
    pub pos: [i32; 3],
    /// The step at which this respawn point STARTS governing — for a bonfire, the
    /// beat that arms it. The dominance credit's window opens here: a close
    /// encounter in a different reign proves nothing about this retry
    /// (spec-0044 §6).
    pub fire_step: usize,
    /// The step at which this respawn point stops governing, `None` = never
    /// ([`crate::compiler::plan::Plan::respawn_reign_ends`]).
    pub reign_end: Option<usize>,
}

/// **The evidence routes, as one table** (spec-0044 §5): per force its perception
/// onset, per pair the bound that skipped it or the credit that answered it.
///
/// It exists so [`RespawnSafetyLedger`] stays `Plan`-free — the binding count and
/// the zero reasons are what that type is for, and they must stay assertable on
/// synthetic inputs. `Default` is the pre-amendment engine exactly: plain staging
/// onsets, no bound, no credit.
#[derive(Clone, Debug, Default)]
struct RespawnEvidenceTable {
    /// Per force: which bound produced its onset, and the sentence for it.
    onsets: BTreeMap<String, crate::compiler::respawn::Onset>,
    /// Per `(respawn point index, force)`: the bound that removed it from the
    /// comparison set.
    skips: BTreeMap<(usize, String), crate::compiler::respawn::Skip>,
    /// Per `(respawn point index, force)`: the evidence that answers an
    /// overlapping geometry.
    credits: BTreeMap<(usize, String), crate::compiler::respawn::Credit>,
}

impl RespawnEvidenceTable {
    /// The kind and sentence for a force's onset — the pre-amendment wording when
    /// no bound moved it, so a campaign whose forces are simply staged late reads
    /// exactly as it always did.
    fn onset_reason(&self, force: &str, step: usize) -> (&'static str, String) {
        match self.onsets.get(force) {
            Some(o) => (o.kind, o.reason.clone()),
            None => (
                "onset",
                format!("`{force}` is first staged at critical-path step {step}"),
            ),
        }
    }
}

/// What the `DW0478` proof quantified over on this build.
///
/// Emitted as `validation/respawn-safety.json`. A proof that examined nothing is
/// not a pass, and the only way to tell the two apart is to publish the count
/// (CLAUDE.md: *every validation artifact states its binding count; a zero
/// binding is a finding*). This ledger exists because `DW0478` spent its whole
/// life returning `Ok(())` on `nobodys-cave-island` — three respawn points, five
/// unleashed hostiles, zero comparisons — and nothing anywhere said so.
#[derive(Clone, Debug)]
pub struct RespawnSafetyLedger {
    /// Every respawn point in content order, with the forces it was measured
    /// against and the forces it was not.
    pub rest_points: Vec<RestPoint>,
    /// Every hostile force's id, in content order.
    pub hostiles: Vec<String>,
    /// Per respawn point, in the same order: the ids it was compared against.
    pub compared: Vec<Vec<String>>,
    /// Per respawn point, in the same order: every force it was NOT compared
    /// against, with the bound that excluded it.
    pub skipped: Vec<Vec<(String, crate::compiler::respawn::Skip)>>,
    /// Per respawn point, in the same order: every compared force whose geometry
    /// overlaps and which the campaign nonetheless supplies evidence for
    /// (spec-0044 §5). A credit is as auditable as a skip, and its `kind` is
    /// computed from the object rather than selected by the author.
    pub credited: BTreeMap<(usize, String), crate::compiler::respawn::Credit>,
    /// Per force, the perception onset the comparison window was cut at.
    pub onsets: BTreeMap<String, usize>,
    /// The comparisons actually made — the proof's binding count.
    pub pairs: usize,
}

impl RespawnSafetyLedger {
    /// The ledger, over a **precomputed evidence table** (spec-0044 §5).
    ///
    /// `Plan`-free on purpose: the binding count and the zero reasons are what
    /// this type exists for, and they must stay assertable on synthetic inputs.
    /// An empty table is the pre-amendment behaviour exactly — every force's
    /// onset is its plain staging beat, no pair is credited, and the comparison
    /// window is the reign window and nothing else.
    fn new(
        rest_points: &[RestPoint],
        sources: &[AggroSource],
        onsets: &BTreeMap<String, usize>,
        table: &RespawnEvidenceTable,
    ) -> Self {
        let mut compared = Vec::new();
        let mut skipped = Vec::new();
        let mut pairs = 0;
        for r in rest_points {
            let mut yes = Vec::new();
            let mut no = Vec::new();
            for s in sources {
                let onset = onsets.get(&s.id).copied().unwrap_or(0);
                if !contemporaneous(r, onset) {
                    let (kind, why) = table.onset_reason(&s.id, onset);
                    no.push((
                        s.id.clone(),
                        crate::compiler::respawn::Skip {
                            kind,
                            reason: format!(
                                "{why}, and this `set-checkpoint` stops governing at step {} — a \
                                 later `set-checkpoint` has replaced it before the force can \
                                 reach the reign, so no death can ever deliver the party here \
                                 while it is in the world",
                                r.reign_end.unwrap_or(usize::MAX)
                            ),
                        },
                    ));
                    continue;
                }
                if let Some(skip) = table.skips.get(&(r.index, s.id.clone())) {
                    no.push((s.id.clone(), skip.clone()));
                    continue;
                }
                yes.push(s.id.clone());
                pairs += 1;
            }
            compared.push(yes);
            skipped.push(no);
        }
        Self {
            rest_points: rest_points.to_vec(),
            hostiles: sources.iter().map(|s| s.id.clone()).collect(),
            compared,
            skipped,
            credited: table.credits.clone(),
            onsets: onsets.clone(),
            pairs,
        }
    }

    /// Whether the proof examined nothing at all.
    pub fn unbound(&self) -> bool {
        self.pairs == 0
    }

    /// Why it examined nothing, when it did — named, so a zero reads as a finding
    /// instead of as a green.
    pub fn reason(&self) -> Option<String> {
        if self.pairs > 0 {
            return None;
        }
        Some(
            match (self.rest_points.is_empty(), self.hostiles.is_empty()) {
                (true, true) => {
                    "this campaign declares no respawn point and no hostile force, so no \
                             cell a player comes back to life on can be inside anything's aggro \
                             range"
                        .to_string()
                }
                (true, false) => format!(
                    "this campaign declares {} hostile force(s) but no `set-checkpoint` and no \
                 `bonfire`: every death returns the party to world spawn, which this proof does \
                 not model",
                    self.hostiles.len()
                ),
                (false, true) => format!(
                    "this campaign declares {} respawn point(s) but no hostile force at all (no wave \
                 with a seated spawn, no actor the campaign unleashes or stages `vulnerable`), so \
                 there is no aggro range for one to be inside",
                    self.rest_points.len()
                ),
                (false, false) => format!(
                    "this campaign declares {} respawn point(s) and {} hostile force(s), and NO pair \
                 of them is ever in the world at the same time — every force is first staged \
                 after the respawn point that could have met it was already replaced",
                    self.rest_points.len(),
                    self.hostiles.len()
                ),
            },
        )
    }

    /// The ledger as the `validation/respawn-safety.json` artifact.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "code": DW_RESPAWN_IN_AGGRO,
            "rest_points": self
                .rest_points
                .iter()
                .zip(&self.compared)
                .zip(&self.skipped)
                .map(|((r, yes), no)| serde_json::json!({
                    "anchor": r.anchor,
                    "kind": r.kind,
                    "pos": r.pos,
                    "fire_step": r.fire_step,
                    "reign_end": r.reign_end,
                    "compared_against": yes,
                    "not_compared": no
                        .iter()
                        .map(|(id, skip)| serde_json::json!({
                            "id": id,
                            "kind": skip.kind,
                            "reason": skip.reason,
                        }))
                        .collect::<Vec<_>>(),
                    "credited": self
                        .credited
                        .iter()
                        .filter(|((i, _), _)| *i == r.index)
                        .map(|((_, id), c)| serde_json::json!({
                            "id": id,
                            "kind": c.kind,
                            "reason": c.reason,
                            "post_reset_state": c.state,
                        }))
                        .collect::<Vec<_>>(),
                }))
                .collect::<Vec<_>>(),
            "examined": self.rest_points.len(),
            "hostiles": self.hostiles,
            "pairs": self.pairs,
            "credits": self.credited.len(),
            "unbound": self.unbound(),
            "reason": self.reason(),
        })
    }
}

/// Ask every evidence route, once per pair (spec-0044 §3/§4/§6).
///
/// Order is the spec's own: the comparison window first (a bound narrows what is
/// compared, never what is demanded of a compared pair), then — for a pair whose
/// geometry actually overlaps — the reset the respawn point performs and the
/// dominance the campaign's forced path already delivers.
///
/// A credit is recorded only where it MATTERS: on a pair that would otherwise be
/// red. A credit on a pair that already clears would be noise in a ledger whose
/// whole job is to be auditable.
fn respawn_evidence_table(
    plan: &Plan,
    evidence: &crate::compiler::respawn::Evidence,
    rest_points: &[RestPoint],
    sources: &[AggroSource],
) -> RespawnEvidenceTable {
    let mut table = RespawnEvidenceTable::default();
    for s in sources {
        table.onsets.insert(s.id.clone(), evidence.onset(&s.id));
    }
    for r in rest_points {
        let Some(cp) = plan.checkpoints.iter().find(|c| c.index == r.index) else {
            continue;
        };
        for s in sources {
            if !contemporaneous(r, evidence.onset(&s.id).step) {
                continue;
            }
            if let Some(skip) = evidence.bearer_bound(cp, &s.id) {
                table.skips.insert((r.index, s.id.clone()), skip);
                continue;
            }
            if nearest_offending(r, s).is_none() {
                continue;
            }
            if let Some(c) = credit_for(evidence, cp, r, s) {
                table.credits.insert((r.index, s.id.clone()), c);
            }
        }
    }
    table
}

/// The evidence a compared pair supplies, in the order spec-0044 §7 states it:
/// the reset the respawn point's own bundle performs, then the dominance the
/// campaign's own forced path already delivers. `None` is the conservative
/// answer, and it leaves the pair red.
fn credit_for(
    evidence: &crate::compiler::respawn::Evidence,
    cp: &crate::compiler::plan::CheckpointPlan,
    rest: &RestPoint,
    src: &AggroSource,
) -> Option<crate::compiler::respawn::Credit> {
    use crate::compiler::respawn::ResetState;
    let state = evidence.reset_state(cp, &src.id);
    if let ResetState::Removed(why) = &state {
        return Some(crate::compiler::respawn::Credit {
            kind: "reset",
            reason: why.clone(),
            state: format!("`{}` has no cells in the world the reset leaves", src.id),
        });
    }
    // Dominance measures against STATIONARY cells only: a lane wave's smeared
    // march corridor is every cell the squad sweeps over time, and the path
    // crossing it is not a proven meeting. A pair whose violation is on a lane
    // cell therefore has no dominance route at all (spec-0044 §6).
    let stationary: Vec<[i32; 3]> = src
        .cells
        .iter()
        .filter(|(_, _, drift)| *drift == 0.0)
        .map(|(_, cell, _)| *cell)
        .collect();
    let (_, _, offending, drift) = nearest_offending(rest, src)?;
    if drift > 0.0 {
        return None;
    }
    let mut credit = evidence.dominance(cp, rest.reign_end, &src.id, &stationary, offending)?;
    if let ResetState::ReStaged(why) = &state {
        credit.state = why.clone();
    }
    Some(credit)
}

/// The nearest cell of `src` that the geometry rule condemns against `rest`, if
/// any — `(what it is, where, how far, the extra reach margin it carries)`.
///
/// Nearest first, then by cell, so the message is deterministic. This IS the
/// clearance rule (spec-0016 §1, verbatim): the distance must EXCEED
/// `follow_range`, plus the measured marching drift for a lane path cell.
fn nearest_offending(
    rest: &RestPoint,
    src: &AggroSource,
) -> Option<(&'static str, [i32; 3], f64, f64)> {
    src.cells
        .iter()
        .map(|(what, cell, drift)| (*what, *cell, cell_distance(rest.pos, *cell), *drift))
        .filter(|(_, _, d, drift)| *d <= src.radius + drift)
        .min_by(|a, b| {
            a.2.partial_cmp(&b.2)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.1.cmp(&b.1))
        })
}

/// Every hostile force in the campaign, in deterministic content order (waves
/// then actors, each in declaration order).
/// **A wave's reach**: the radius inside which its bodies acquire a player —
/// the lane's `aggro_radius`, else the largest `follow_range` any of its mobs
/// declares, else [`DEFAULT_FOLLOW_RANGE`] — with the words a message names it
/// by. The one reading `DW0380`, `DW0478` and `DW0863` share: a fight that
/// starts within this distance of the party finds the party.
pub(crate) fn wave_aggro_radius(w: &delvewright_dsl::Wave) -> (f64, &'static str) {
    match &w.lane {
        Some(l) => (f64::from(l.aggro_radius), "the lane's `aggro_radius`"),
        None => match w
            .mobs
            .iter()
            .filter_map(|m| m.attributes.and_then(|a| a.follow_range))
            .fold(None::<f64>, |acc, r| Some(acc.map_or(r, |a| a.max(r))))
        {
            Some(r) => (r, "the wave's declared `follow_range`"),
            None => (
                f64::from(DEFAULT_FOLLOW_RANGE),
                "the default `follow_range` (none declared)",
            ),
        },
    }
}

pub(crate) fn aggro_sources(
    plan: &Plan,
    world: &World,
    placements: &BTreeMap<String, Vec<[i32; 3]>>,
    lanes: &LaneRoutes,
) -> Vec<AggroSource> {
    let c = plan.campaign;
    let mut out = Vec::new();
    for w in &c.quests.content.waves {
        let mut cells: Vec<(&'static str, [i32; 3], f64)> = placements
            .get(w.id.as_str())
            .into_iter()
            .flatten()
            .map(|p| ("seated spawn cell", *p, 0.0))
            .collect();
        let (radius, radius_source) = wave_aggro_radius(w);
        if let Some(wps) = lanes.get(w.id.as_str()) {
            cells.extend(
                lane_march_cells(plan, world, w, wps)
                    .into_iter()
                    .map(|p| ("lane path cell", p, LANE_MARCH_DRIFT)),
            );
        }
        if cells.is_empty() {
            continue;
        }
        out.push(AggroSource {
            id: w.id.as_str().to_string(),
            radius,
            radius_source,
            cells,
        });
    }
    for a in &c.quests.content.actors {
        if !actor_fights(c, a) {
            continue;
        }
        let Some(pos) = plan.body_point(delvewright_dsl::BodyRef::Actor(a)) else {
            continue;
        };
        let (radius, radius_source) = match a.attributes.and_then(|at| at.follow_range) {
            Some(r) => (r, "the actor's declared `follow_range`"),
            None => (
                f64::from(DEFAULT_FOLLOW_RANGE),
                "the default `follow_range` (none declared)",
            ),
        };
        out.push(AggroSource {
            id: a.id.as_str().to_string(),
            radius,
            radius_source,
            cells: vec![("staging anchor", pos, 0.0)],
        });
    }
    out
}

/// Whether the campaign declares this actor as something that FIGHTS — the same
/// declaration-based test `DW0469` uses: an `unleash-actor` beat (the author
/// asking for a real-AI twin) or `vulnerable: true` (a damageable target). Never
/// inferred from the species.
fn actor_fights(c: &delvewright_dsl::Campaign, a: &delvewright_dsl::Actor) -> bool {
    if a.vulnerable {
        return true;
    }
    let mut unleashed = false;
    delvewright_dsl::for_each_campaign_effect(c, &mut |_, _, eff| {
        if let Verb::UnleashActor { actor, .. } = &eff.verb
            && actor.as_str() == a.id.as_str()
        {
            unleashed = true;
        }
    });
    unleashed
}

/// Every cell a lane squad provably walks: the A*-proven legs from the wave's
/// form-up footing through each waypoint, over the same **no-gate-use** view
/// [`plan_lanes`] proved them on. An unroutable leg is `DW0386`'s business and is
/// simply skipped here.
fn lane_march_cells(
    plan: &Plan,
    world: &World,
    w: &delvewright_dsl::Wave,
    wps: &[[i32; 3]],
) -> Vec<[i32; 3]> {
    let entity_world_owned;
    let world: &World = if world.has_use_gates() {
        entity_world_owned = world.without_gate_use();
        &entity_world_owned
    } else {
        world
    };
    let Some(area) = crate::compiler::plan::wave_area(plan.campaign, w.id.as_str()) else {
        return Vec::new();
    };
    let Some(anchor) = plan.point(area, w.anchor.as_str()) else {
        return Vec::new();
    };
    let Some(mut prev) = world.snap_standable(anchor, SNAP_RADIUS) else {
        return Vec::new();
    };
    let mut out = vec![prev];
    for wp in wps {
        if let Some(path) = world.find_path(prev, *wp) {
            out.extend(path);
        }
        prev = *wp;
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// One pair the geometry rule condemns and no evidence route credits.
struct RespawnViolation {
    anchor: String,
    pos: [i32; 3],
    force: String,
    what: &'static str,
    cell: [i32; 3],
    dist: f64,
    reach: String,
}

/// **Every** violating pair, in content order (spec-0044 §5).
///
/// The diagnostic used to return at the first pair, so enumerating a campaign's
/// violations at all required a patched binary — which is how six false verdicts
/// stayed invisible behind one. One build now states them all.
///
/// The clearance demanded of a compared, uncredited pair is spec-0016 §1
/// verbatim: the distance must EXCEED `follow_range`, plus the measured marching
/// drift for a lane path cell. Nothing about the demand moves.
fn respawn_violations(
    rest_points: &[RestPoint],
    sources: &[AggroSource],
    onsets: &BTreeMap<String, usize>,
    table: &RespawnEvidenceTable,
) -> Vec<RespawnViolation> {
    let mut out = Vec::new();
    for rest in rest_points {
        for src in sources {
            if !contemporaneous(rest, onsets.get(&src.id).copied().unwrap_or(0)) {
                continue;
            }
            // The SAME table the ledger reports from: a pair the ledger says
            // was never compared, or was answered, must not be condemned here.
            // Two readings of one question is how a ledger starts lying.
            let key = (rest.index, src.id.clone());
            if table.skips.contains_key(&key) || table.credits.contains_key(&key) {
                continue;
            }
            let Some((what, cell, dist, drift)) = nearest_offending(rest, src) else {
                continue;
            };
            let reach = if drift > 0.0 {
                format!(
                    "the {reach:.1}-block reach: the {radius:.1}-block perception radius \
                     ({radius_source}) plus the {drift:.1}-block measured marching drift — a \
                     lane squad marches a corridor around its polyline, not the line itself \
                     (td-routing-spike dossier)",
                    reach = src.radius + drift,
                    radius = src.radius,
                    radius_source = src.radius_source,
                )
            } else {
                format!(
                    "the {radius:.1}-block perception radius ({radius_source})",
                    radius = src.radius,
                    radius_source = src.radius_source,
                )
            };
            out.push(RespawnViolation {
                anchor: rest.anchor.clone(),
                pos: rest.pos,
                force: src.id.clone(),
                what,
                cell,
                dist,
                reach,
            });
        }
    }
    out
}

/// The `DW0478` diagnostic for a whole build: the first pair in the shape it has
/// always had, then every other pair the same build condemns.
///
/// What a red claims is what declarations can carry (spec-0044 §2): **nothing the
/// campaign declares separates this respawn from a soft-lock**. So the message
/// prescribes the three evidence routes before it prescribes moving anything, and
/// still never offers shrinking `follow_range`.
fn respawn_error(violations: &[RespawnViolation]) -> Option<Failure> {
    let first = violations.first()?;
    let others: Vec<String> = violations
        .iter()
        .skip(1)
        .map(|v| {
            format!(
                "`{}` ({:?}) x `{}`: its {} {:?} at {:.1} blocks, within {}",
                v.anchor, v.pos, v.force, v.what, v.cell, v.dist, v.reach
            )
        })
        .collect();
    let also = if others.is_empty() {
        "This build condemns no other pair.".to_string()
    } else {
        format!(
            "This build condemns {} pairs in total; the others are: {}.",
            violations.len(),
            others.join("; ")
        )
    };
    Some(Failure {
        code: DW_RESPAWN_IN_AGGRO,
        message: format!(
            "respawn point `{anchor}` ({pos:?}) sits INSIDE the aggro range of `{id}`: its \
             {what} {cell:?} is {dist:.1} blocks away, within {reach}. A respawn point is \
             where the party comes back after a death — and, for a bonfire, where every \
             `respawns_on_rest` wave is put back on its feet. What this red claims is what \
             declarations can carry: NOTHING THIS CAMPAIGN DECLARES SEPARATES THIS RETRY \
             FROM A SOFT-LOCK (spec-0044 §2) — whether the loop is winnable is a combat \
             question this compiler refuses to simulate. Three kinds of evidence answer it, \
             and each is checked before this fires: (1) the respawn point's own \
             `on_respawn` / `on_rest` bundle UNCONDITIONALLY removes or re-places the force, \
             so the world the reset leaves does not hold it where it was; (2) the force's \
             staging cannot meet this reign at all — its gate flags cannot be set in time, \
             the `strike-npc` trigger's bearer is gone, or the body is a `NoAI` puppet for \
             every instant compared; (3) the campaign's own FORCED critical path already \
             walks the party into that same force inside this same reign, no farther away \
             and against no fresher a body. Supply one of those, or move the respawn point \
             out of the danger — into a side room, behind the threshold, past the end of the \
             lane — or move the force's anchor / lane. The rule is the same for a plain \
             `set-checkpoint` and for a `bonfire`: vanilla returns a dead player to either \
             by the identical `spawnpoint` mechanism, so the hazard is a property of the \
             CELL, never of the verb that named it. Do NOT shrink `follow_range` to buy the \
             clearance: that retunes the fight to hide a placement bug. {also}",
            anchor = first.anchor,
            pos = first.pos,
            id = first.force,
            what = first.what,
            cell = first.cell,
            dist = first.dist,
            reach = first.reach,
        ),
    })
}

/// The pure geometry core of [`check_respawn_safe_zone`] (unit-testable without a
/// [`Plan`]), with **no evidence route credited** — exactly the demand a compared,
/// uncredited pair faces, unchanged to the block.
#[cfg(test)]
fn verify_respawn_safe_zone(
    rest_points: &[RestPoint],
    sources: &[AggroSource],
    onsets: &BTreeMap<String, usize>,
) -> Result<(), Failure> {
    let none = RespawnEvidenceTable::default();
    match respawn_error(&respawn_violations(rest_points, sources, onsets, &none)) {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

/// Euclidean distance between two cells, in blocks.
fn cell_distance(a: [i32; 3], b: [i32; 3]) -> f64 {
    (0..3)
        .map(|i| f64::from(a[i] - b[i]).powi(2))
        .sum::<f64>()
        .sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::BTreeMap;

    /// A hostile force for the `DW0478` proof. A "lane path cell" carries the
    /// [`LANE_MARCH_DRIFT`] margin exactly as [`aggro_sources`] assigns it;
    /// every stationary cell carries none.
    fn src(id: &str, radius: f64, cells: &[(&'static str, [i32; 3])]) -> AggroSource {
        AggroSource {
            id: id.to_string(),
            radius,
            radius_source: "the wave's declared `follow_range`",
            cells: cells
                .iter()
                .map(|(what, cell)| {
                    let drift = if *what == "lane path cell" {
                        LANE_MARCH_DRIFT
                    } else {
                        0.0
                    };
                    (*what, *cell, drift)
                })
                .collect(),
        }
    }

    /// A permanently-reigning bonfire at `pos` — what every pre-existing `DW0478`
    /// case is, so those tests read exactly as they did before the proof learned
    /// about plain checkpoints.
    fn fire(anchor: &str, pos: [i32; 3]) -> RestPoint {
        RestPoint {
            index: 0,
            anchor: anchor.to_string(),
            kind: "bonfire",
            pos,
            fire_step: 0,
            reign_end: None,
        }
    }

    /// A plain `set-checkpoint` that stops governing at `reign_end`.
    fn checkpoint(anchor: &str, pos: [i32; 3], reign_end: usize) -> RestPoint {
        RestPoint {
            index: 0,
            anchor: anchor.to_string(),
            kind: "set-checkpoint",
            pos,
            fire_step: 0,
            reign_end: Some(reign_end),
        }
    }

    /// No force declares an onset, so every one is conservatively live from step 0.
    fn from_the_start() -> BTreeMap<String, usize> {
        BTreeMap::new()
    }

    /// `DW0478`: a bonfire inside a wave's perception radius. The party respawns
    /// into contact — a soft-lock, not a difficulty choice — so this is an error,
    /// and the message must name the clearance the geometry is short by.
    #[test]
    fn a_bonfire_inside_an_aggro_radius_is_dw0478() {
        let bonfires = vec![fire("anchor/chapel", [34, 71, -113])];
        let sources = vec![src(
            "wave/gate-assault",
            16.0,
            &[("seated spawn cell", [34, 71, -103])],
        )];
        let err = verify_respawn_safe_zone(&bonfires, &sources, &from_the_start())
            .expect_err("a fire 10 blocks inside a 16-block perception radius is a soft-lock");
        assert_eq!(err.code, DW_RESPAWN_IN_AGGRO); // DW0478
        assert!(
            err.message.contains("anchor/chapel") && err.message.contains("wave/gate-assault"),
            "the message names both sides of the violation: {}",
            err.message
        );
        assert!(
            err.message.contains("10.0 blocks"),
            "the message states the measured distance: {}",
            err.message
        );
        assert!(
            err.message.contains("Do NOT shrink `follow_range`"),
            "the prescription must not offer retuning the fight as a fix: {}",
            err.message
        );
    }

    /// The LANE half of the same rule: the squad's seated cells can be far away
    /// and the fire still be unsafe, because a lane wave walks its polyline while
    /// the party is elsewhere. This is the drowned-bell shape — a fire beside the
    /// end of a siege lane.
    #[test]
    fn a_bonfire_beside_a_lane_path_is_dw0478() {
        let bonfires = vec![fire("anchor/l2-bonfire", [34, 71, -113])];
        let sources = vec![src(
            "wave/gate-assault",
            16.0,
            &[
                ("seated spawn cell", [12, 71, -84]),
                ("lane path cell", [24, 71, -110]),
            ],
        )];
        let err = verify_respawn_safe_zone(&bonfires, &sources, &from_the_start())
            .expect_err("the lane reaches it");
        assert_eq!(err.code, DW_RESPAWN_IN_AGGRO);
        assert!(
            err.message.contains("lane path cell"),
            "the message must say it is the MARCH that reaches the fire, not the seating: {}",
            err.message
        );
    }

    /// The DRIFT half of the lane term: a fire that
    /// clears the centre-line polyline by less than the measured marching drift
    /// is still inside the squad's real aggro reach, because the squad marches a
    /// corridor around the polyline (td-routing-spike dossier: followers max 7.9
    /// blocks off-lane). This is the drowned bell's chapel fire — 18.0 blocks
    /// from a 16-`follow_range` lane, and run nine died to it live at 17.7.
    #[test]
    fn a_bonfire_clearing_the_centre_line_but_not_the_march_corridor_is_dw0478() {
        let bonfires = vec![fire("anchor/chapel", [18, 64, 0])];
        let sources = vec![src(
            "wave/bell-siege",
            16.0,
            &[("lane path cell", [0, 64, 0])],
        )];
        let err = verify_respawn_safe_zone(&bonfires, &sources, &from_the_start())
            .expect_err("18.0 blocks clears follow_range 16 but not 16 + 7.9 drift");
        assert_eq!(err.code, DW_RESPAWN_IN_AGGRO); // DW0478
        assert!(
            err.message.contains("marching drift") && err.message.contains("td-routing-spike"),
            "the message must name the drift term and its constraint source: {}",
            err.message
        );
        assert!(
            err.message.contains("23.9"),
            "the message states the full reach (16 + 7.9): {}",
            err.message
        );
    }

    /// The drift margin belongs to the MARCH alone: a stationary seated cell at
    /// the same 18.0 blocks from the same 16-block radius is legal, because a
    /// force that never walks has no corridor around a polyline it never marches.
    #[test]
    fn a_stationary_cell_at_the_same_distance_carries_no_drift_margin() {
        let bonfires = vec![fire("anchor/chapel", [18, 64, 0])];
        let sources = vec![src(
            "wave/bell-siege",
            16.0,
            &[("seated spawn cell", [0, 64, 0])],
        )];
        assert!(
            verify_respawn_safe_zone(&bonfires, &sources, &from_the_start()).is_ok(),
            "the drift term is specifically for lane-marching squads"
        );
    }

    /// Clearance strictly greater than the radius is legal — the rule is
    /// "must exceed", so the boundary itself is not a violation.
    #[test]
    fn a_bonfire_outside_every_aggro_radius_is_clean() {
        let bonfires = vec![fire("anchor/beach", [0, 64, 0])];
        let sources = vec![
            src("wave/near", 8.0, &[("seated spawn cell", [9, 64, 0])]),
            src("wave/far", 16.0, &[("lane path cell", [0, 64, 40])]),
        ];
        assert!(verify_respawn_safe_zone(&bonfires, &sources, &from_the_start()).is_ok());
    }

    /// A campaign with no rest point proves nothing here: the rule is about where
    /// the party is DELIVERED, and without a bonfire nothing delivers them.
    #[test]
    fn hostiles_without_a_bonfire_are_not_the_safe_zone_proof_s_business() {
        let sources = vec![src(
            "wave/anything",
            64.0,
            &[("seated spawn cell", [0, 64, 0])],
        )];
        assert!(verify_respawn_safe_zone(&[], &sources, &from_the_start()).is_ok());
    }

    /// **The sibling case, and the whole point of `bell-08`.** The identical
    /// geometry that is `DW0478` for a bonfire is `DW0478` for a plain
    /// `set-checkpoint`: the party is delivered onto that cell by the same
    /// vanilla `spawnpoint`, so the hazard belongs to the CELL. For nineteen-plus
    /// island rounds this proof examined zero objects on a campaign with three
    /// checkpoints, because it filtered on `rest == true`.
    #[test]
    fn a_plain_set_checkpoint_inside_an_aggro_radius_is_dw0478() {
        let rest = vec![checkpoint("anchor/checkpoint-3", [34, 71, -113], 99)];
        let sources = vec![src(
            "actor/polyphemus-blinded",
            16.0,
            &[("staging anchor", [34, 71, -103])],
        )];
        let err = verify_respawn_safe_zone(&rest, &sources, &from_the_start()).expect_err(
            "a set-checkpoint 10 blocks inside a 16-block radius is the same soft-lock",
        );
        assert_eq!(err.code, DW_RESPAWN_IN_AGGRO); // DW0478
        assert!(
            err.message.contains("anchor/checkpoint-3")
                && err
                    .message
                    .contains("the same for a plain `set-checkpoint`"),
            "the message must say the rule does not care which verb placed the cell: {}",
            err.message
        );
    }

    /// The reign model, in the direction that makes it honest: a force first
    /// staged AFTER a plain checkpoint has been replaced can never meet the party
    /// there, so it is not compared. This is not a relaxation of the geometry —
    /// the same pair at the same distance IS a violation while both are live.
    #[test]
    fn a_replaced_checkpoint_is_not_measured_against_a_body_staged_later() {
        let rest = vec![checkpoint("anchor/checkpoint-1", [34, 71, -113], 7)];
        let sources = vec![src(
            "wave/storm-shore",
            48.0,
            &[("seated spawn cell", [34, 71, -103])],
        )];
        let late: BTreeMap<String, usize> = [("wave/storm-shore".to_string(), 12)].into();
        assert!(
            verify_respawn_safe_zone(&rest, &sources, &late).is_ok(),
            "a checkpoint retired at step 7 cannot deliver anybody to a wave first seated at 12"
        );
        assert!(
            verify_respawn_safe_zone(&rest, &sources, &from_the_start()).is_err(),
            "the SAME geometry is a violation the moment the two are contemporaneous — the \
             window narrows what is compared, never what is demanded of a compared pair"
        );
    }

    /// A bonfire never stops reigning, so it is compared against a force staged
    /// at any step whatsoever — byte-for-byte the behaviour before the window
    /// existed.
    #[test]
    fn a_bonfire_is_compared_against_a_force_staged_at_any_later_step() {
        let rest = vec![fire("anchor/chapel", [34, 71, -113])];
        let sources = vec![src(
            "wave/gate-assault",
            16.0,
            &[("seated spawn cell", [34, 71, -103])],
        )];
        let late: BTreeMap<String, usize> = [("wave/gate-assault".to_string(), 999)].into();
        assert_eq!(
            verify_respawn_safe_zone(&rest, &sources, &late)
                .expect_err("a fire the party can return to forever meets everything")
                .code,
            DW_RESPAWN_IN_AGGRO
        );
    }

    /// The binding count is published, and a zero says WHY. A proof that examined
    /// nothing is the vacuity this whole ledger exists to break, so the artifact
    /// must never be able to look like a pass.
    #[test]
    fn the_ledger_states_its_binding_count_and_names_a_zero() {
        let sources = vec![src("wave/x", 8.0, &[("seated spawn cell", [0, 64, 0])])];
        let bound = RespawnSafetyLedger::new(
            &[
                fire("anchor/a", [99, 64, 0]),
                checkpoint("anchor/b", [98, 64, 0], 5),
            ],
            &sources,
            &from_the_start(),
            &Default::default(),
        );
        assert_eq!(bound.pairs, 2, "two rest points x one force");
        assert!(!bound.unbound() && bound.reason().is_none());

        let no_rest =
            RespawnSafetyLedger::new(&[], &sources, &from_the_start(), &Default::default());
        assert!(no_rest.unbound());
        assert!(
            no_rest.reason().unwrap().contains("no `set-checkpoint`"),
            "a zero must name which half of the proof was missing"
        );

        let no_hostiles = RespawnSafetyLedger::new(
            &[fire("anchor/a", [0, 64, 0])],
            &[],
            &from_the_start(),
            &Default::default(),
        );
        assert!(no_hostiles.unbound());
        assert!(
            no_hostiles
                .reason()
                .unwrap()
                .contains("no hostile force at all")
        );

        // The third zero, and the one a reader would otherwise never suspect:
        // both halves exist and no pair is ever contemporaneous.
        let never_meet = RespawnSafetyLedger::new(
            &[checkpoint("anchor/b", [0, 64, 0], 3)],
            &sources,
            &[("wave/x".to_string(), 9)].into(),
            &Default::default(),
        );
        assert!(never_meet.unbound());
        assert!(
            never_meet.reason().unwrap().contains("at the same time"),
            "a campaign whose respawn points and hostiles never coexist is a real zero, and it \
             is named rather than reported as a pass: {:?}",
            never_meet.reason()
        );
    }

    /// **The ledger and the red list read ONE table.** A pair the ledger reports
    /// as never compared must not be condemned by the same build: two readings of
    /// one question is how a ledger starts lying, and this proof had exactly that
    /// defect for the length of one afternoon — the skip was recorded and the pair
    /// was still red.
    #[test]
    fn a_skipped_pair_is_never_condemned_and_a_credited_one_is_never_either() {
        let rest = vec![checkpoint("anchor/chapel", [34, 71, -113], 99)];
        let sources = vec![src(
            "wave/gate-assault",
            16.0,
            &[("seated spawn cell", [34, 71, -103])],
        )];
        // With no evidence at all the geometry condemns it.
        assert_eq!(
            respawn_violations(&rest, &sources, &from_the_start(), &Default::default()).len(),
            1
        );
        let mut skipped = RespawnEvidenceTable::default();
        skipped.skips.insert(
            (0, "wave/gate-assault".to_string()),
            crate::compiler::respawn::Skip {
                kind: "bearer-bound",
                reason: "for the test".into(),
            },
        );
        assert!(respawn_violations(&rest, &sources, &from_the_start(), &skipped).is_empty());
        let mut credited = RespawnEvidenceTable::default();
        credited.credits.insert(
            (0, "wave/gate-assault".to_string()),
            crate::compiler::respawn::Credit {
                kind: "dominated",
                reason: "for the test".into(),
                state: String::new(),
            },
        );
        assert!(respawn_violations(&rest, &sources, &from_the_start(), &credited).is_empty());
    }
}
