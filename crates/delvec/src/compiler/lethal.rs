//! Lethal volumes (DSL v0.10, spec-0031): the proofs a box that kills owes the
//! completability model, and the binding ledger that says what they looked at.
//!
//! ## Where the reasoning lives, and why it is split
//!
//! A lethal volume is geometry, so most of its completability reasoning is not
//! here: [`crate::compiler::nav::World`] carries its cells as **impassable** and every route
//! proof in the engine inherits that for free — the critical path (`DW0510`), the
//! checkpoint no-stranding proof (`DW0315`), the branch paths, the trap forced-cell
//! set, the exported harness waypoints. That is the whole point of putting it in
//! the world rather than in a check of its own: a fourth consumer inherits the
//! proof instead of re-deriving it, exactly as `close-gate`'s seal does.
//!
//! What is left is the one obligation routing cannot see, because it is not about
//! walking: **the places the campaign PUTS something.** A respawn seat or a posted
//! body inside a lethal volume routes perfectly and is killed on arrival — the
//! party forever (the death loop), an NPC once and silently. Both are
//! [`DW_LETHAL_RESPAWN_SEAT`], because both are the same defect: a position
//! reached by declaration rather than by walking.
//!
//! `seats` in the ledger counts every such place, not only the respawn ones.
//!
//! ## A body has a width
//!
//! "Inside a volume" is a question about a **body**, not about a cell. The volume
//! kills with a box selector, and the server adjudicates a box selector on hitbox
//! intersection — so a 1.4-wide spider standing on the cell beside a volume's
//! face is inside it, and a 0.6-wide villager on the same cell is not. Every
//! place here therefore carries the hitbox of the body that lands on it
//! ([`PostedPlace`]) and is judged by
//! [`delvewright_dsl::metrics::body_meets_volume`], the one answer the routing
//! model and the emitted selector are also written against.
//!
//! One class of place is chosen by the compiler rather than by the campaign — a
//! **wave's seats**, taken from whatever standable footing the anchor's room
//! offers. That footing is proven for a PLAYER's body, so a body taller than one
//! reaches a volume from a seat a walker stands on safely; and the author has no
//! post to move. Same rule, same code, different prescription — see
//! [`ChosenBy`].
//!
//! ## Binding (`docs/reference/playtest-methodology.md` rule 1)
//!
//! [`LethalGate`] states what was examined: how many volumes were declared, how
//! many resolved, how many cells they hold, how many respawn seats were tested and
//! how many critical-path legs were routed against them. A zero volume count is
//! reported as unbound rather than as a pass — a campaign that declares no volume
//! emits no ledger at all, so a ledger that exists and says zero is a finding.

use delvewright_dsl::Campaign;

use crate::compiler::failure::Failure;
use crate::compiler::plan::{LethalVolumePlan, Plan};
use delvewright_dsl::{DwCode, ExitTier};

/// `DW0511`: a **posted place** — somewhere the campaign requires the party or a
/// declared body to BE — lies inside a lethal volume (spec-0031).
///
/// One rule, because it is one defect: *a body is put here by declaration, not by
/// walking, so no route proof can see it.* Three families of site fall under it.
///
/// * **Respawn seats** — the campaign's entry spawn, a `set-checkpoint` cell, a
///   `bonfire` cell. The death loop: the party dies on arrival and is re-seated to
///   die again, forever. `/spawnpoint` is only a hint and the engine re-seats on
///   the death edge, so nothing downstream can rescue it. The exact dual of
///   `DW0315`/`DW0316` for the hazard the party respawns *into*.
/// * **Posted bodies** — a stage-2 NPC's anchor, a per-quest `cast` placement, a
///   stage-5 actor's anchor. A volume's entity sweep exempts the engine's own
///   machinery types and deliberately NOT content bodies (a mob that walks into
///   the lava dies, which is the mechanism working) — so an NPC posted inside one
///   is deleted on the first tick, the delve loses its speaker, and every static
///   proof stays green. Found while writing this feature's own CI fixture, which
///   is exactly the shape the rule now refuses.
/// * **Wave seats** — the cells `emit::plan_wave_spawns` stands a wave's mobs on.
///   The same defect with a different author: the cell is chosen by the compiler
///   rather than written by the campaign, so the message says a different thing
///   about what to move ([`ChosenBy`]) and the rule is unchanged. It is not its
///   own code, and that is a judgement worth recording rather than assuming: the
///   seating is drawn from the footing a PLAYER can stand on, which now excludes
///   every cell a player's own hitbox could meet a volume from, and that ring is
///   the same ring for every body in the engine's dims table up to two blocks
///   wide. What is left is a body more than two blocks TALL seated exactly one
///   cell below where a player's head would already have been refused — a
///   warden, an iron golem or a ravager under a volume that floats two courses
///   above the floor. One rule, one code, and the prescription branches.
pub const DW_LETHAL_RESPAWN_SEAT: DwCode = DwCode::new("DW0511", ExitTier::Build);

/// The binding ledger for the lethal-volume proofs.
#[derive(Clone, Debug, Default)]
pub struct LethalGate {
    /// Volumes the campaign declared.
    pub declared: usize,
    /// Volumes that resolved to a box on the solved layout. A gap between this
    /// and [`Self::declared`] means an anchor no placed piece provides — already
    /// `DW0142` at validation, restated here so a reader of the ledger alone
    /// cannot mistake a dropped volume for a proven one.
    pub resolved: usize,
    /// World cells those boxes cover — what the navigation model actually made
    /// impassable.
    pub cells: usize,
    /// Respawn seats tested against every volume (`DW0511`).
    pub seats: usize,
    /// Critical-path legs routed over the world with lethality applied
    /// (`DW0510` / `DW0311`).
    pub legs: usize,
    /// PackTest templates generated for these volumes — the runtime half. A
    /// compile-time-only green over a runtime mechanism is the vacuity this
    /// number exists to make visible.
    pub packtests: usize,
    /// What `DW0891` looked at, per volume and over the campaign (spec-0062 §5).
    pub visibility: DangerVisibility,
}

impl LethalGate {
    /// Whether this proof matched nothing at all.
    pub fn unbound(&self) -> bool {
        self.resolved == 0
    }

    /// The ledger as the `validation/lethal-gate.json` artifact.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "volumes": { "declared": self.declared, "resolved": self.resolved },
            "staged": self.visibility.staged.iter().map(|r| serde_json::json!({
                "id": r.id,
                "gate_terms": r.gate_terms.iter().map(|t| serde_json::json!({
                    "objective": t.objective,
                    "holder": t.holder(),
                    "min": t.min,
                    "max": t.max,
                    "negate": t.negate,
                })).collect::<Vec<_>>(),
                "configurations": {
                    "judged": r.judged,
                    "may_live": r.may_live,
                    "is_live": r.is_live,
                    "of": r.of,
                },
            })).collect::<Vec<_>>(),
            "cells": self.cells,
            "respawn_seats_examined": self.seats,
            "critical_path_legs_examined": self.legs,
            "packtest_templates": self.packtests,
            "unbound": self.unbound(),
            "danger_visibility": self.visibility.to_json(),
        })
    }
}

// ---------------------------------------------------------------------------
// Danger is visible, or the engine refuses it (spec-0062)
// ---------------------------------------------------------------------------

/// `DW0891`: **a killing volume the player cannot see** (spec-0062 §4).
///
/// One code, three shapes, one rule — *a killing volume and what shows it
/// agree*. The full derivation is on
/// [`delvewright_dsl::codes::LETHAL_INVISIBLE`], which is where the code itself
/// is declared: the document arm lives in `dsl::validate`, so a second constant
/// here would be one number for two rules.
pub const DW_LETHAL_INVISIBLE: DwCode = delvewright_dsl::codes::LETHAL_INVISIBLE;

/// What `DW0891` examined for one volume in one configuration (spec-0062 §5,
/// spec-0088 §5) — one row of the ledger per (volume, configuration) pair.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VolumeVisibility {
    /// The volume's authored id.
    pub id: String,
    /// The configuration this row judged: `None` for a volume live from
    /// world-load, judged once over the world as built; for a staged volume the
    /// first critical step whose arrival is judged under that configuration.
    pub configuration: Option<usize>,
    /// For a staged volume, whether it may be live in this configuration —
    /// `false` is the last configuration before a switch-on, judged because a
    /// body standing in the keep-out when the gate flips has no tick in which to
    /// step out (spec-0088 §2.5).
    pub live: bool,
    /// The walked population this row was measured against.
    pub population: usize,
    /// The cells a player body can be caught from —
    /// [`delvewright_dsl::metrics::keep_out_box`] of the resolved region.
    pub keep_out: ([i32; 3], [i32; 3]),
    /// `K ∩ P`: the keep-out cells the party can walk to, over the world with
    /// lethality removed. Sorted (ADR-0006).
    pub caught: Vec<[i32; 3]>,
    /// Of [`Self::caught`], the cells whose floor or own block is one of
    /// [`Self::shown_by`] in this configuration's bytes.
    pub shown: Vec<[i32; 3]>,
    /// The blocks the volume declares as showing it, as declared.
    pub shown_by: Vec<String>,
    /// The first body the engine models that can get its hitbox into the
    /// volume in this configuration, in words — a player moving from where the
    /// campaign puts the party, else a wave member
    /// ([`DangerVisibility::credit_waves`]). `None` is a zero binding for this
    /// row.
    pub reached_by: Option<String>,
}

impl VolumeVisibility {
    /// This row of the ledger.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "configuration": self.configuration,
            "live": self.live,
            "population": self.population,
            "keep_out": { "lo": self.keep_out.0, "hi": self.keep_out.1 },
            "caught": self.caught.len(),
            "caught_cells": self.caught,
            "shown": self.shown.len(),
            "shown_by": self.shown_by,
            "reached_by": self.reached_by,
        })
    }
}

/// One staged volume's configurations (spec-0088 §9): its gate as terms, and
/// how many of the path's configurations were judged, may hold it live, and
/// hold it live on the forced route, out of how many.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StagedRow {
    /// The volume's authored id.
    pub id: String,
    /// The gate, as the bot tier and the emitter read it.
    pub gate_terms: Vec<crate::compiler::plan::GateTerm>,
    /// Configurations `DW0891` judged this volume in.
    pub judged: usize,
    /// Configurations in which it may be live.
    pub may_live: usize,
    /// Configurations in which it is live on the forced route.
    pub is_live: usize,
    /// Configurations the critical path passes.
    pub of: usize,
    /// When `is_live` is zero: the critical step the gate was first read at, and
    /// the term that never held on the forced route (`DW0954`).
    pub unmet: Option<(usize, String)>,
}

/// What `DW0891` examined over the whole campaign (spec-0062 §5).
///
/// A volume that catches nothing prints its zero **beside the population**, so a
/// pit whose keep-out lies wholly under the floor reads as *checked and clear*
/// rather than as *unbound*: the denominator is what tells the two apart.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DangerVisibility {
    /// Resolved volumes — the denominator `N`.
    pub declared: usize,
    /// Cells the party can walk to from everywhere it is PUT, over the world as
    /// built with lethality removed — the population a volume live from
    /// world-load is judged against.
    pub population: usize,
    /// Distinct configurations judged (the world as built counts as one when
    /// any volume is live from world-load) — `C`.
    pub configurations: usize,
    /// One row per (volume, configuration) pair, volumes in declaration order
    /// and configurations in path order — `K` rows.
    pub volumes: Vec<VolumeVisibility>,
    /// One entry per staged volume, in declaration order — `S` entries.
    pub staged: Vec<StagedRow>,
    /// `shown_by` entries examined, over every volume.
    pub declarations: usize,
    /// Of those, the ones some caught cell bears out in some judged row.
    pub borne_out: usize,
}

impl DangerVisibility {
    /// Caught cells over every row.
    pub fn caught(&self) -> usize {
        self.volumes.iter().map(|v| v.caught.len()).sum()
    }

    /// Of those, the ones that show their hazard.
    pub fn shown(&self) -> usize {
        self.volumes.iter().map(|v| v.shown.len()).sum()
    }

    /// The distinct volume ids with a row, in row order.
    fn ids(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for v in &self.volumes {
            if !out.contains(&v.id.as_str()) {
                out.push(v.id.as_str());
            }
        }
        out
    }

    /// Volumes no body the engine models can get into, in any row in which
    /// the volume may be live.
    pub fn unreached(&self) -> usize {
        self.ids()
            .into_iter()
            .filter(|id| {
                !self
                    .volumes
                    .iter()
                    .any(|v| v.id == *id && v.live && v.reached_by.is_some())
            })
            .count()
    }

    /// Rows a modelled body reaches — `P`.
    fn reached_rows(&self) -> usize {
        self.volumes
            .iter()
            .filter(|v| v.reached_by.is_some())
            .count()
    }

    /// The one line this proof owes its reader.
    pub fn line(&self) -> String {
        let pops = self.volumes.iter().map(|v| v.population);
        let (lo, hi) = (pops.clone().min().unwrap_or(0), pops.max().unwrap_or(0));
        format!(
            "danger-visibility binding: {} volume(s), {} staged; judged over {} configuration(s) \
             as {} (volume, configuration) pair(s) against walked populations of {lo}..{hi} \
             cell(s); {} caught, {} shown, {} read as safe floor; {} declaration(s) of {} borne out \
             by the bytes; {} of {} volume(s) reached by a body the engine models (in {} of {} \
             pairs).",
            self.declared,
            self.staged.len(),
            self.configurations,
            self.volumes.len(),
            self.caught(),
            self.shown(),
            self.caught() - self.shown(),
            self.borne_out,
            self.declarations,
            self.declared - self.unreached(),
            self.declared,
            self.reached_rows(),
            self.volumes.len(),
        )
    }

    /// Credit every row no player reaches with the first wave member that
    /// does (`DW0922` / `DW0923`'s findings). A body is a body: a volume only a
    /// mob can enter is bound. Waves are judged against every volume as live
    /// (spec-0088 §6), so the credit reaches every row of the volume.
    pub fn credit_waves(&mut self, waves: &WaveLethalBinding) {
        for v in &mut self.volumes {
            if v.reached_by.is_some() {
                continue;
            }
            v.reached_by = waves
                .as_built
                .iter()
                .chain(&waves.opened)
                .find(|f| f.volume == v.id)
                .map(|f| format!("wave `{}`'s `{}`", f.wave, f.entity));
        }
    }

    /// The finding a zero binding owes its reader: one warning per volume no
    /// modelled body can get into (`DW0891`'s vacuity half).
    ///
    /// Not a refusal. The usual cause is a movement the engine does not model —
    /// a player diving to the bottom of a flooded shaft — and refusing would
    /// reject a legitimate design for an engine limitation. What it may not be is
    /// silent: an empty catch over a volume nothing can enter looks exactly like
    /// an empty catch over a volume that is clear of the floor.
    pub fn findings(&self) -> Vec<delvewright_dsl::Diagnostic> {
        self.ids()
            .into_iter()
            .filter(|id| {
                !self
                    .volumes
                    .iter()
                    .any(|v| v.id == *id && v.live && v.reached_by.is_some())
            })
            .map(|id| {
                let examined: Vec<String> = self
                    .volumes
                    .iter()
                    .filter(|v| v.id == id && v.live)
                    .filter_map(|v| v.configuration)
                    .map(|s| format!("critical step {s}"))
                    .collect();
                let where_ = if examined.is_empty() {
                    String::new()
                } else {
                    format!(
                        " It is live from a story stage, and the configurations examined in \
                         which it may be live arrive at {}.",
                        examined.join(", ")
                    )
                };
                delvewright_dsl::Diagnostic::warning(
                    DW_LETHAL_INVISIBLE,
                    "build",
                    "danger-visibility binding",
                    format!(
                        "lethal volume `{id}` is reached by no body the engine models: no cell a \
                         player can walk, fall, jump or swim to from where the campaign puts the \
                         party, and no cell a wave member can walk, fall or sink to within its \
                         follow range, holds a body the volume catches.{where_} So the \
                         visibility proof caught nothing here because nothing can be caught, not \
                         because the volume is clear of the floor — this is a zero binding. A \
                         body may still get in by a movement the model does not make (diving, a \
                         diagonal jump, climbing), or nothing ever will. If a player is meant to \
                         be able to die here, the engine has not proven they can get in, and the \
                         bot cannot be sent there; if nothing is, delete the volume."
                    ),
                )
            })
            .chain(self.staged.iter().filter_map(|r| {
                let (step, term) = r.unmet.as_ref()?;
                Some(delvewright_dsl::Diagnostic::warning(
                    DW_LETHAL_STAGE_UNMET,
                    "build",
                    "danger-visibility binding",
                    format!(
                        "lethal volume `{}` is live from a story stage and the forced route \
                         passes no configuration in which it is live: its gate, first read at \
                         critical step {step}, never holds on the forced route — {term}. So the \
                         ladder cannot exercise it, the bot's death loop will find it shut, and \
                         only the configurations in which it may be live were judged for \
                         visibility. No change is required: a hazard the party need never arm is \
                         a design, and the bot's report will say it was not exercised.",
                        r.id
                    ),
                ))
            }))
            .collect()
    }

    /// The ledger's `danger_visibility` object.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "population": self.population,
            "configurations": self.configurations,
            "pairs": self.volumes.len(),
            "caught": self.caught(),
            "shown": self.shown(),
            "reads_as_safe_floor": self.caught() - self.shown(),
            "declarations": { "examined": self.declarations, "borne_out": self.borne_out },
            "unreached": self.unreached(),
            "volumes": self.volumes.iter().map(VolumeVisibility::to_json).collect::<Vec<_>>(),
        })
    }
}

/// `DW0954`: **a staged volume the forced route never meets live**
/// (spec-0088 §9). Advisory: a hazard the party need never arm is a design, but
/// the ladder cannot exercise it, the bot's death loop will find it shut, and
/// only its may-be-live configurations were judged for visibility — so the
/// build says so.
pub const DW_LETHAL_STAGE_UNMET: DwCode = DwCode::new("DW0954", ExitTier::Build);

/// Does the block under or in `cell` show one of `shown_by`?
///
/// **Under or in, never beside** (spec-0062 §2). A shore cell next to lava
/// stands on stone and holds air; the lava beside it is not what the body is on.
/// A block with a collision top (`magma_block`, `cactus`) is the FLOOR under the
/// cell; one with an empty collision shape (`fire`, `sweet_berry_bush`) stands
/// IN it, passable to the walker. Both readings are here because the model
/// already knows which a block is, and asking for one alone would miss the other
/// kind of signal entirely.
///
/// Matched on the bare id, so a `campfire[lit=true]` in the bytes answers a
/// `minecraft:campfire` declaration — the same reading
/// [`delvewright_dsl::blockshape::hurts_body`] takes.
fn cell_shows(
    blocks: &crate::compiler::blockstate::BlockMap,
    cell: [i32; 3],
    shown_by: &[String],
) -> bool {
    let under = [cell[0], cell[1] - 1, cell[2]];
    [under, cell].iter().any(|c| {
        blocks.get(c).is_some_and(|b| {
            let bare = delvewright_dsl::blockshape::bare_id(b);
            shown_by
                .iter()
                .any(|s| delvewright_dsl::blockshape::bare_id(s) == bare)
        })
    })
}

/// **Every cell the party is PUT at** — the roots of the population `P`
/// (spec-0062 §2 decision 1).
///
/// The entry spawn, every `set-checkpoint` and `bonfire` seat, and every
/// transit-teleport destination. Rooted at all of them and not at the entry
/// alone, as `DW0881`'s population is: a party teleported into an area stands on
/// that area's floor, and a rule that judged only what walks from the door would
/// be silent about every area reached by a teleport.
///
/// Deterministic: entry, then checkpoints in content order, then teleports in
/// declaration order (ADR-0006).
pub fn population_roots(plan: &Plan, entry: Option<[i32; 3]>) -> Vec<[i32; 3]> {
    let mut out: Vec<[i32; 3]> = Vec::new();
    out.extend(entry);
    out.extend(plan.checkpoints.iter().map(|cp| cp.pos));
    out.extend(plan.transit_teleports.iter().map(|(_, to)| *to));
    out
}

/// `DW0891`: **prove no killing volume reaches a cell the player would read as
/// safe floor** (spec-0062).
///
/// The ruling this implements is the owner's, and it is why the constraint is
/// here rather than in the walk graph: *avoiding a lethal volume's collateral
/// damage is never done by marking ground that looks walkable as unwalkable —
/// the player does not know.* The keep-out the routing model already refuses
/// ([`crate::compiler::nav::World::meets_lethal_fp`]) answers a different
/// question, asked earlier: **does this volume, as declared, reach a cell the
/// player would read as safe floor?** If it does, the declaration is refused and
/// the creator moves the volume.
///
/// # The population is the lethality-free one, and that is load-bearing
///
/// The walk model already refuses the keep-out, so a population taken from the
/// lethal-APPLIED world can never contain a caught cell: over that world this
/// check is green for every volume ever written, while binding to nothing. It
/// therefore reads the counterfactual [`crate::compiler::nav::World::without_exclusions`] —
/// the identical world `DW0510` is already derived from — and
/// `the_population_is_the_lethality_free_one` perturbs it back to the vacuous
/// shape and asserts the zero (spec-0062 §10.4).
///
/// It lifts **every** semantic exclusion, furniture included (spec-0065 §4.2).
/// A furniture declaration withholds cells from walking; were the population
/// taken with it applied, marking the stone round a pit as furniture would make
/// the pit's caught floor vanish from `P` and this check go green. The table a
/// keep-out catches is caught floor, because the body that climbs it dies.
///
/// # What it does NOT do
///
/// Nothing here changes the walk graph. The router still refuses every cell of
/// the keep-out, the recovery stake still chooses its lip outside it, and
/// `death-plan.json` still carries it to the bot. A visible hazard is still a
/// hazard; what `DW0891` guarantees is that the cells the walk graph loses are
/// cells a player could see were dangerous.
///
/// # A staged volume is judged per configuration (spec-0088 §5)
///
/// A volume live from a story stage has no one assembled world: the floor round
/// it may be plain stone before the flip and settled debris after. So it is
/// judged in every configuration the critical path passes in which it may be
/// live, and in the last configuration before each switch-on — a body standing
/// in the keep-out when the gate flips is killed in the same server tick, so
/// the floor it stood on must already have read as danger, in that
/// configuration's own bytes ([`crate::compiler::nav::Configuration::blocks`]).
/// A volume live from world-load is judged once, over the world as built, as it
/// always was.
///
/// Returns the binding beside the verdict, so the line a run prints is a count
/// over every volume rather than over the ones that preceded the failure.
pub fn check_danger_is_visible(
    plan: &Plan,
    world: &crate::compiler::nav::World,
    blocks: &crate::compiler::blockstate::BlockMap,
    entry: Option<[i32; 3]>,
) -> (DangerVisibility, Result<(), Failure>) {
    let mut binding = DangerVisibility {
        declared: plan.lethal_volumes.len(),
        ..DangerVisibility::default()
    };
    if plan.lethal_volumes.is_empty() {
        return (binding, Ok(()));
    }
    let roots = population_roots(plan, entry);
    // The counterfactual, not the world the router walks. See the note above.
    let open = world.without_exclusions();
    let as_built = open.reachable_walkable(&roots);
    binding.population = as_built.len();

    // The configurations, for the staged volumes: every one the critical path
    // passes, then every one each reachable branch's own path passes — a volume
    // one branch arms is met on that branch's path and nowhere else. Built only
    // when a staged volume is declared, so a campaign that stages nothing
    // measures exactly what it did.
    let staged_any = plan.lethal_volumes.iter().any(|v| v.staged.is_some());
    let mut configs: Vec<crate::compiler::nav::Configuration> = Vec::new();
    // (path label, per step: (configuration, liveness)), the critical path first.
    let mut paths: Vec<(String, Vec<(usize, Vec<crate::compiler::nav::Liveness>)>)> = Vec::new();
    let ancestor = |g: usize, s: usize| plan.gate_fired_before(g, s);
    if staged_any {
        paths.push((
            "the critical path".to_string(),
            crate::compiler::nav::configurations_along(
                world,
                &plan.region_events,
                &ancestor,
                plan.critical_path.len(),
                &mut configs,
            ),
        ));
        let realized = crate::compiler::branch::realize(plan.campaign);
        if !realized.is_empty() {
            let flow = crate::compiler::flow::Flow::new(plan.campaign);
            for r in &realized {
                let Some(widx) = r.world else { continue };
                // A branch whose path cannot be built is refused by the branch
                // proofs, with its own message; it has no configurations here.
                let Ok(cp) = plan.branch_critical_path(&flow, &flow.playthrough_in(widx)) else {
                    continue;
                };
                let (events, ancestors) = plan.branch_gate_model(&cp);
                let anc = |g: usize, s: usize| {
                    g == 0 || ancestors.get(&s).is_some_and(|a| a.contains(&g))
                };
                paths.push((
                    format!("branch `{}`'s path", r.branch.id),
                    crate::compiler::nav::configurations_along(
                        world,
                        &events,
                        &anc,
                        cp.steps.len(),
                        &mut configs,
                    ),
                ));
            }
        }
    }
    // Per configuration, built on first use: its lethality-free world, its
    // walked population, and its bytes.
    type Judged = (
        crate::compiler::nav::World,
        std::collections::BTreeSet<[i32; 3]>,
        crate::compiler::blockstate::BlockMap,
    );
    let mut judged_cfg: std::collections::BTreeMap<usize, Judged> =
        std::collections::BTreeMap::new();
    let mut used_cfg: std::collections::BTreeSet<Option<usize>> = std::collections::BTreeSet::new();

    let body = delvewright_dsl::metrics::Body::PLAYER;
    let mut rows: Vec<Judgement> = Vec::new();
    let mut staged_idx = 0usize;
    for (vi, v) in plan.lethal_volumes.iter().enumerate() {
        let (klo, khi) = delvewright_dsl::metrics::keep_out_box(body, v.region.0, v.region.1);
        let row_of = |w: &crate::compiler::nav::World,
                      population: &std::collections::BTreeSet<[i32; 3]>,
                      bytes: &crate::compiler::blockstate::BlockMap,
                      configuration: Option<usize>,
                      live: bool| {
            let caught: Vec<[i32; 3]> = population
                .iter()
                .copied()
                .filter(|c| (0..3).all(|i| klo[i] <= c[i] && c[i] <= khi[i]))
                .collect();
            let shown: Vec<[i32; 3]> = caught
                .iter()
                .copied()
                .filter(|&c| cell_shows(bytes, c, &v.shown_by))
                .collect();
            let proots: Vec<[i32; 3]> = population.iter().copied().collect();
            let reach = w.reach_into_volumes(
                &proots,
                &crate::compiler::nav::Footprint::player(),
                false,
                None,
                &[v.region],
            );
            VolumeVisibility {
                id: v.id.clone(),
                configuration,
                live,
                population: population.len(),
                keep_out: (klo, khi),
                caught,
                shown,
                shown_by: v.shown_by.clone(),
                reached_by: reach.hits[0].as_ref().map(|h| {
                    format!(
                        "a player, by {} from {:?}",
                        h.how,
                        h.path.last().copied().unwrap_or(klo)
                    )
                }),
            }
        };
        let Some(gate) = &v.staged else {
            used_cfg.insert(None);
            rows.push(Judgement {
                vi,
                row: row_of(&open, &as_built, blocks, None, true),
                before: false,
                ci: None,
                path: String::new(),
                switch_at: 0,
            });
            continue;
        };
        // Which configurations this volume is judged in: every one in which it
        // may be live, and, on each path, the one before each switch-on
        // (`before`, with the step it switches on at and the path it is on).
        let si = staged_idx;
        staged_idx += 1;
        // (configuration, before, path label, the step it first may be live at,
        // path index)
        let mut wanted: Vec<(usize, bool, String, usize, usize)> = Vec::new();
        for (pi, (label, per_step)) in paths.iter().enumerate() {
            for (s, (ci, live)) in per_step.iter().enumerate() {
                if !live[si].may {
                    continue;
                }
                if !wanted.iter().any(|w| w.0 == *ci) {
                    wanted.push((*ci, false, label.clone(), s, pi));
                }
                if s > 0 && !per_step[s - 1].1[si].may {
                    let prev = per_step[s - 1].0;
                    if !wanted.iter().any(|w| w.0 == prev) {
                        wanted.push((prev, true, label.clone(), s, pi));
                    }
                }
            }
        }
        // Path order, then each path's own step order.
        wanted.sort_by_key(|w| (w.4, configs[w.0].step));
        // The critical path's configurations — what the ledger counts against.
        let critical: Vec<usize> = {
            let mut seen: Vec<usize> = Vec::new();
            for (ci, _) in paths.first().map(|p| p.1.as_slice()).unwrap_or(&[]) {
                if !seen.contains(ci) {
                    seen.push(*ci);
                }
            }
            seen
        };
        let mut srow = StagedRow {
            id: v.id.clone(),
            gate_terms: gate.terms.clone(),
            judged: wanted.len(),
            may_live: critical
                .iter()
                .filter(|ci| configs[**ci].live.get(si).is_some_and(|l| l.may))
                .count(),
            is_live: critical
                .iter()
                .filter(|ci| configs[**ci].live.get(si).is_some_and(|l| l.is))
                .count(),
            of: critical.len(),
            unmet: None,
        };
        if srow.is_live == 0 {
            let first = wanted.first().map_or(0, |w| configs[w.0].step);
            let last = plan.critical_path.len().saturating_sub(1);
            srow.unmet = Some((
                first,
                never_held_term(gate, &plan.region_events, last, &ancestor),
            ));
        }
        binding.staged.push(srow);
        for (ci, before, path, switch_at, _) in wanted {
            let (w, population, bytes) = judged_cfg.entry(ci).or_insert_with(|| {
                let cw = configs[ci]
                    .world(world)
                    .map_or_else(|| world.without_exclusions(), |w| w.without_exclusions());
                let pop = cw.reachable_walkable(&roots);
                let bytes = configs[ci].blocks(blocks);
                (cw, pop, bytes)
            });
            used_cfg.insert(Some(ci));
            rows.push(Judgement {
                vi,
                row: row_of(w, population, bytes, Some(configs[ci].step), !before),
                before,
                ci: Some(ci),
                path,
                switch_at,
            });
        }
    }
    binding.configurations = used_cfg.len();

    // A staged volume judged in no configuration is a defect of the
    // enumeration, never a line (spec-0088 §9).
    if let Some(r) = binding.staged.iter().find(|r| r.judged == 0) {
        let verdict = Failure {
            code: DW_LETHAL_INVISIBLE,
            message: format!(
                "lethal volume `{}` is live from a story stage and the visibility proof judged it \
                 in none of the {} configuration(s) the critical path and the branch paths pass: \
                 its gate may hold in none of them, so no floor was examined for it. A staged \
                 volume no path can meet is either a gate nothing on any path can open — check \
                 the terms ({}) against the beats that set them — or a defect of this \
                 enumeration; it is refused rather than reported as checked.",
                r.id,
                configs.len(),
                plan.lethal_volumes
                    .iter()
                    .find(|v| v.id == r.id)
                    .and_then(|v| v.staged.as_ref())
                    .map(|g| g.words())
                    .unwrap_or_default(),
            ),
        };
        binding.volumes = rows.into_iter().map(|j| j.row).collect();
        return (binding, Err(verdict));
    }

    // Declarations: a block is borne out when some judged row of its volume
    // shows it on a caught cell, in that row's bytes.
    for (vi, v) in plan.lethal_volumes.iter().enumerate() {
        binding.declarations += v.shown_by.len();
        binding.borne_out += v
            .shown_by
            .iter()
            .filter(|b| {
                rows.iter()
                    .filter(|j| j.vi == vi)
                    .any(|j| borne(j, b, blocks, &configs))
            })
            .count();
    }

    // The verdict, per row in order: caught floor that shows nothing first,
    // because a volume that catches floor is wrong about the world and a
    // fiction is only wrong about the document.
    let mut verdict: Option<Failure> = None;
    for j in &rows {
        let (row, before) = (&j.row, &j.before);
        let v = &plan.lethal_volumes[j.vi];
        let unseen: Vec<[i32; 3]> = row
            .caught
            .iter()
            .copied()
            .filter(|c| !row.shown.contains(c))
            .collect();
        if unseen.is_empty() {
            continue;
        }
        let declared = if v.shown_by.is_empty() {
            "It declares no `shown_by` at all".to_string()
        } else {
            format!(
                "It declares `shown_by` {}, which no cell of this floor bears out{}",
                v.shown_by
                    .iter()
                    .map(|b| format!("`{b}`"))
                    .collect::<Vec<_>>()
                    .join(", "),
                if row.configuration.is_some() {
                    " in this configuration's bytes"
                } else {
                    ""
                }
            )
        };
        let cells = crate::compiler::failure::cells_by_floor(&unseen);
        let message = match (row.configuration, *before, &v.staged) {
            (Some(step), true, Some(gate)) => format!(
                "lethal volume `{}` goes live at step {} of {} and a body may be standing on these \
                 cells when it does; nothing in the world before that beat says they kill: \
                 {cells} ({} cell(s), the configuration arriving at critical step {step}, before \
                 its gate — {} — holds). A body standing in a volume's keep-out when its gate \
                 flips is killed in the same server tick, with no tick in which to step off, so \
                 the floor it stood on must read as danger BEFORE the beat. {declared}. Roof or \
                 wall the volume's cells off until the beat that opens them (the beat that arms \
                 the volume is usually the beat that should open the way to it); lower the volume \
                 so its keep-out's top course lies under the floor a body stands on before the \
                 flip; or author one of the blocks vanilla hurts a body with under those cells, \
                 visible before the flip, and declare it in `shown_by`. Do not declare the floor \
                 a place nobody walks, and do not fire the flag later to pass — a hazard that \
                 arrives silently under a body is the finding, wherever on the path it arrives.",
                row.id,
                j.switch_at,
                j.path,
                unseen.len(),
                gate.words(),
            ),
            (configuration, _, _) => {
                let in_config = match (configuration, &v.staged) {
                    (Some(step), Some(gate)) => format!(
                        " In the configuration arriving at step {step} of {}, where its gate \
                         ({}) may hold.",
                        j.path,
                        gate.words()
                    ),
                    _ => String::new(),
                };
                format!(
                    "lethal volume `{}` catches {} cell(s) of floor the party walks, and nothing \
                     in the world says so: {cells}.{in_config} A volume kills by a box selector \
                     and the server adjudicates that on hitbox INTERSECTION, so the cells a body \
                     can be caught from are the volume's own box widened by half a body — the \
                     keep-out {:?}..={:?} — and every one of these is standing room a player \
                     reads as ordinary floor. {declared}. Danger is visible, or the engine \
                     refuses it: no cell of the keep-out may be floor the party walks unless the \
                     block under or in it is one of `shown_by`. Lower the volume so its \
                     keep-out's top course lies UNDER the floor (a pit's volume sits at the \
                     pit's bottom, on an anchor at the pit's bottom); draw its `extent` in so \
                     the keep-out stops one cell short of the floor; or author one of the \
                     blocks vanilla hurts a body with under those cells and declare it in \
                     `shown_by`. Do NOT repair this by marking the floor unwalkable — the \
                     compiler knows and the player does not.",
                    row.id,
                    unseen.len(),
                    row.keep_out.0,
                    row.keep_out.1,
                )
            }
        };
        verdict = Some(Failure {
            code: DW_LETHAL_INVISIBLE,
            message,
        });
        break;
    }
    if verdict.is_none() {
        'volumes: for (vi, v) in plan.lethal_volumes.iter().enumerate() {
            let mine: Vec<&VolumeVisibility> =
                rows.iter().filter(|j| j.vi == vi).map(|j| &j.row).collect();
            for block in &v.shown_by {
                if rows
                    .iter()
                    .filter(|j| j.vi == vi)
                    .any(|j| borne(j, block, blocks, &configs))
                {
                    continue;
                }
                let caught: usize = mine.iter().map(|r| r.caught.len()).sum();
                let configs_words = if v.staged.is_some() {
                    format!(" in any of the {} configuration(s) judged", mine.len())
                } else {
                    String::new()
                };
                verdict = Some(Failure {
                    code: DW_LETHAL_INVISIBLE,
                    message: format!(
                        "lethal volume `{}` declares `shown_by` block `{block}`, and it stands \
                         under or in none of the {caught} cell(s) of walked floor this volume \
                         catches{configs_words}. A declaration is a claim about the assembled \
                         bytes and this one is not borne out by them{}. Delete the declaration, \
                         or author `{block}` under the cells this volume catches.",
                        v.id,
                        if caught == 0 {
                            " — the volume catches no walked floor at all, so it needs no signal \
                             and the declaration is what is wrong"
                        } else {
                            ""
                        },
                    ),
                });
                break 'volumes;
            }
        }
    }
    binding.volumes = rows.into_iter().map(|j| j.row).collect();
    (binding, verdict.map_or(Ok(()), Err))
}

/// One (volume, configuration) row with what the verdict needs beside it: the
/// volume's index, whether the row is the configuration before a switch-on,
/// the configuration's index (`None` for a volume live from world-load, judged
/// over the world as built), the path the configuration was reached on, and
/// the step of that path the volume first may be live at.
struct Judgement {
    vi: usize,
    row: VolumeVisibility,
    before: bool,
    ci: Option<usize>,
    path: String,
    switch_at: usize,
}

/// Whether a row bears out `block` on one of its caught cells, read in the
/// bytes of the configuration it judged.
fn borne(
    j: &Judgement,
    block: &str,
    blocks: &crate::compiler::blockstate::BlockMap,
    configs: &[crate::compiler::nav::Configuration],
) -> bool {
    let owned;
    let bytes = match j.ci.and_then(|ci| configs.get(ci)) {
        Some(c) => {
            owned = c.blocks(blocks);
            &owned
        }
        None => blocks,
    };
    let one = [block.to_string()];
    j.row.caught.iter().any(|&c| cell_shows(bytes, c, &one))
}

/// The first term of `gate` that does not hold on the forced route at the
/// arrival `at` — what `DW0954` names.
fn never_held_term(
    gate: &crate::compiler::plan::StagedGate,
    events: &crate::compiler::plan::RegionEvents,
    at: usize,
    ancestor: &dyn Fn(usize, usize) -> bool,
) -> String {
    for f in &gate.requires_flags {
        let one = crate::compiler::plan::StagedGate {
            requires_flags: vec![f.clone()],
            forbids_flags: Vec::new(),
            requires_state: Vec::new(),
            terms: Vec::new(),
        };
        if !crate::compiler::nav::liveness_of(&one, events, at, ancestor).is {
            return format!("`{f}` is required and no forced beat on the path sets it");
        }
    }
    for f in &gate.forbids_flags {
        let one = crate::compiler::plan::StagedGate {
            requires_flags: Vec::new(),
            forbids_flags: vec![f.clone()],
            requires_state: Vec::new(),
            terms: Vec::new(),
        };
        if !crate::compiler::nav::liveness_of(&one, events, at, ancestor).is {
            return format!("`{f}` is forbidden and something can set it before the volume is met");
        }
    }
    for c in &gate.requires_state {
        let one = crate::compiler::plan::StagedGate {
            requires_flags: Vec::new(),
            forbids_flags: Vec::new(),
            requires_state: vec![c.clone()],
            terms: Vec::new(),
        };
        if !crate::compiler::nav::liveness_of(&one, events, at, ancestor).is {
            return format!(
                "`{}` is never decided true by the replay of the forced route",
                c.state.as_str()
            );
        }
    }
    format!("the gate ({}) never holds", gate.words())
}

/// One posted place: what a diagnostic calls it, the cell the campaign puts a
/// body on, and **the body that lands there**.
///
/// The third field is the whole of what a cell could not say. A volume kills by a
/// box selector and vanilla adjudicates that on hitbox intersection, so whether a
/// post is inside a volume is a question about a body's box and not about a cell:
/// a 1.4-wide iron golem seated on the cell beside a pit's face is standing in
/// the pit as far as the selector is concerned, and a 0.6-wide villager on the
/// same cell is not.
pub struct PostedPlace {
    /// What the diagnostic names this place.
    pub label: String,
    /// The cell the campaign posts a body on.
    pub cell: [i32; 3],
    /// The hitbox of the body that lands there.
    pub body: delvewright_dsl::metrics::Body,
    /// Who chose the cell — which decides the remedy, and so the code.
    pub chosen_by: ChosenBy,
}

/// **Who chose a posted place's cell.** One code either way — the rule is the
/// same defect — but the PRESCRIPTION differs, because an author can only act on
/// a position they wrote.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChosenBy {
    /// The campaign: an entry spawn, a checkpoint anchor, an NPC's or actor's
    /// post, a `cast` placement. *Move the post, or shrink the volume* — both
    /// things the author wrote down.
    Campaign,
    /// The **compiler**, seating a wave's mobs on the standable footing its room
    /// offers. There is no post to move and an author told to move one would go
    /// looking for a declaration that does not exist, so the prescription names
    /// the room, the anchor and the volume instead.
    WaveSeating,
}

/// The body a stage-2 NPC actually wears, as a hitbox — through
/// [`crate::compiler::nav::npc_body_entity`], so a skinned NPC is measured as the mannequin
/// it ships as rather than as the `base_entity` it declares.
fn npc_body(n: &delvewright_dsl::Npc) -> delvewright_dsl::metrics::Body {
    let (w, h) = crate::compiler::nav::entity_dims(&crate::compiler::nav::npc_body_entity(n));
    delvewright_dsl::metrics::Body::new(w, h)
}

/// Every place the campaign PUTS something: the party's respawn seats, then every
/// declared body's post, then every cell a wave's mobs are seated on.
///
/// Deterministic throughout — entry, then checkpoints in content order, then NPCs
/// in declaration order with their cast placements, then actors, then waves in
/// declaration order with their stacks in declaration order (ADR-0006).
///
/// `wave_seats` is the seating [`crate::compiler::plan::wave_seats`] pairs off
/// `emit::plan_wave_spawns`'s answer. It is an argument rather than something
/// derived here because a wave's cells are a **measurement of the assembled
/// world** — the standable footing its room actually offers — and the plan alone
/// cannot state them. A campaign whose waves have not been seated (a proof run
/// before that pass) passes an empty map and the wave arm binds to nothing, which
/// is why the caller's ledger counts what it examined.
pub fn posted_places(
    plan: &Plan,
    entry: Option<[i32; 3]>,
    wave_seats: &std::collections::BTreeMap<String, Vec<[i32; 3]>>,
) -> Vec<PostedPlace> {
    let player = delvewright_dsl::metrics::Body::PLAYER;
    let mut out: Vec<PostedPlace> = Vec::new();
    fn declared(
        out: &mut Vec<PostedPlace>,
        label: String,
        cell: [i32; 3],
        body: delvewright_dsl::metrics::Body,
    ) {
        out.push(PostedPlace {
            label,
            cell,
            body,
            chosen_by: ChosenBy::Campaign,
        });
    }
    let push = declared;
    if let Some(pos) = entry {
        push(
            &mut out,
            "the campaign's entry spawn".to_string(),
            pos,
            player,
        );
    }
    for cp in &plan.checkpoints {
        let kind = if cp.rest { "bonfire" } else { "checkpoint" };
        push(
            &mut out,
            format!("{kind} anchor `{}`", cp.anchor),
            cp.pos,
            player,
        );
    }
    let c = plan.campaign;
    for npc in &c.npcs.content.npcs {
        if let Some(pos) = plan.body_point(delvewright_dsl::BodyRef::Npc(npc)) {
            push(
                &mut out,
                format!("npc `{}`'s post `{}`", npc.id, npc.anchor),
                pos,
                npc_body(npc),
            );
        }
    }
    // Per-quest `cast` placements move an NPC between beats, so the stage-2 anchor
    // alone is not the whole answer: a keeper who is safe at his post and posted
    // into the pit for act two is the same defect one quest later.
    for q in &c.quests.content.quests {
        for (npc, entry) in &q.cast {
            for pl in entry.placements() {
                let Some(mark) = pl.at.mark() else { continue };
                let at = mark.display();
                let Some(pos) = plan.point_any(mark.anchor.as_str()).map(|p| mark.cell(p)) else {
                    continue;
                };
                let body = c
                    .npcs
                    .content
                    .npcs
                    .iter()
                    .find(|n| n.id.as_str() == npc.as_str())
                    .map_or(player, npc_body);
                push(
                    &mut out,
                    format!("npc `{npc}`'s `cast` placement `{at}` in quest `{}`", q.id),
                    pos,
                    body,
                );
            }
        }
    }
    for a in &c.quests.content.actors {
        if let Some(pos) = plan.body_point(delvewright_dsl::BodyRef::Actor(a)) {
            let (w, h) =
                crate::compiler::nav::entity_dims(&crate::compiler::nav::actor_body_entity(a));
            push(
                &mut out,
                format!("actor `{}`'s post `{}`", a.id, a.anchor),
                pos,
                delvewright_dsl::metrics::Body::new(w, h),
            );
        }
    }
    for w in &c.quests.content.waves {
        let Some(cells) = wave_seats.get(w.id.as_str()) else {
            continue;
        };
        for (entity, cell) in crate::compiler::plan::wave_seats(w, cells) {
            let (bw, bh) = crate::compiler::nav::entity_dims(entity);
            out.push(PostedPlace {
                label: format!("wave `{}`'s seat for `{entity}`", w.id),
                cell,
                body: delvewright_dsl::metrics::Body::new(bw, bh),
                chosen_by: ChosenBy::WaveSeating,
            });
        }
    }
    out
}

/// Prove no posted place stands a body inside a lethal volume (`DW0511`).
///
/// "Inside" is judged of the BODY, not of the cell: each place carries the hitbox
/// of what lands on it, seated at the coordinates a summon writes
/// ([`crate::compiler::nav::cell_center`]), and
/// [`delvewright_dsl::metrics::body_meets_volume`] answers. The name is the one
/// the diagnostic has always had; what it examines is every posted place, respawn
/// seats included.
///
/// `entry` is the campaign's resolved entry-spawn cell, threaded in by the caller
/// (the emitter already resolves it); `None` for a layout with no entry anchor,
/// which is `DW0345` upstream and not this proof's finding to make.
///
/// Returns the seats examined on success, so the caller's ledger reports a real
/// count rather than a number derived a second time.
pub fn check_respawn_seats(
    plan: &Plan,
    entry: Option<[i32; 3]>,
    wave_seats: &std::collections::BTreeMap<String, Vec<[i32; 3]>>,
) -> Result<usize, Failure> {
    let seats = posted_places(plan, entry, wave_seats);
    if plan.lethal_volumes.is_empty() {
        return Ok(seats.len());
    }
    for place in &seats {
        // The body's own box, at the coordinates a summon writes for that cell —
        // `nav::cell_center` is the one place that says where a body seated on a
        // cell stands, and `metrics::body_meets_volume` the one place that says
        // whether a box meets a volume.
        let feet = crate::compiler::nav::cell_center(place.cell);
        let blamed: Vec<&str> = plan
            .lethal_volumes
            .iter()
            .filter(|v| {
                delvewright_dsl::metrics::body_meets_volume(
                    feet, place.body, v.region.0, v.region.1,
                )
            })
            .map(|v| v.id.as_str())
            .collect();
        if blamed.is_empty() {
            continue;
        }
        let names = blamed
            .iter()
            .map(|i| format!("`{i}`"))
            .collect::<Vec<_>>()
            .join(", ");
        let label = &place.label;
        let pos = place.cell;
        let (w, h) = (place.body.width, place.body.height);
        let as_live = as_live_words(plan, &blamed);
        // One code, because it is one defect. What branches is the PRESCRIPTION:
        // an author can act on a post they wrote, and cannot act on a cell the
        // seating pass chose for them.
        let harm = match place.chosen_by {
            ChosenBy::Campaign => {
                "Whatever the campaign puts here is put here by declaration, not by walking, so \
                 no route proof can see it: a respawn seat means the party dies on arrival and \
                 is re-seated to die again on every death, forever (`/spawnpoint` is only a \
                 hint and the engine re-seats on the death edge, so nothing downstream can \
                 rescue it); a posted body means the volume deletes it on the first tick and \
                 the delve loses it in silence. Move the post clear of the volume — clear of \
                 its FACES, not merely out of its cells — or shrink the volume's `extent`; do \
                 NOT delete the volume to silence the proof."
            }
            ChosenBy::WaveSeating => {
                "The mob is killed on the tick it is summoned, the wave's counter never comes \
                 down, and any objective that waits for that wave to be cleared waits forever. \
                 Nothing here was authored as a position: the compiler seats a wave on the \
                 standable footing its anchor's own room offers, and that footing is proven for \
                 a PLAYER's body — this body is larger than one. So there is no post to move. \
                 Move the wave's `anchor` clear of the volume, shrink the volume's `extent`, or \
                 give the wave a body that fits where its room can seat it; do NOT delete the \
                 volume to silence the proof."
            }
        };
        return Err(Failure {
            code: DW_LETHAL_RESPAWN_SEAT,
            message: format!(
                "{label} at {pos:?} stands a body whose hitbox is {w} x {h} blocks INSIDE lethal \
                 volume(s) {names}.{as_live} A volume kills by a box selector and the server \
                 adjudicates that on hitbox INTERSECTION, not on the cell a body stands in — so \
                 a body reaches out of its own cell, and this place is inside the volume even \
                 where its cell is not. {harm}"
            ),
        });
    }
    Ok(seats.len())
}

/// The as-live quantifier a posted place's or a wave's refusal states when a
/// volume it names is live from a story stage (spec-0088 §6): the body is judged
/// against it as live whether or not it is live when the body is put there,
/// because a post, a seat or a reach that meets it is the same defect arriving
/// later.
fn as_live_words(plan: &Plan, ids: &[&str]) -> String {
    let staged: Vec<String> = plan
        .lethal_volumes
        .iter()
        .filter(|v| v.staged.is_some() && ids.contains(&v.id.as_str()))
        .map(|v| format!("`{}`", v.id))
        .collect();
    if staged.is_empty() {
        String::new()
    } else {
        format!(
            " {} {} live from a story stage; the body is judged against it as live, whether or \
             not it is live when the body is put there — a body in a pit that wakes later is \
             deleted, re-seated or thinned when it wakes. Move the body before the beat (a \
             `cast` placement, a `move-actor`), or move the volume.",
            staged.join(", "),
            if staged.len() == 1 { "is" } else { "are" }
        )
    }
}

/// The ledger for a finished build: what the campaign declared, what resolved,
/// and what each proof examined.
pub fn gate(
    c: &Campaign,
    volumes: &[LethalVolumePlan],
    cells: usize,
    seats: usize,
    legs: usize,
    packtests: usize,
    visibility: DangerVisibility,
) -> LethalGate {
    LethalGate {
        declared: c.quests.content.lethal_volumes.len(),
        resolved: volumes.len(),
        cells,
        seats,
        legs,
        packtests,
        visibility,
    }
}

// ---------------------------------------------------------------------------
// A wave does not walk into a killing volume (DW0922, DW0923)
// ---------------------------------------------------------------------------

/// `DW0922`: **a seated wave whose members can reach a lethal volume.**
///
/// A volume's entity sweep kills every body that is not a player, wave members
/// included — the mechanism working, when the party leads a mob there. The
/// defect is a wave seated so that its own members get there unled: by the
/// movement a mob has ([`crate::compiler::nav::World::mob_moves`] — walking, a
/// step or jump onto a block at most one higher, a drop off any edge, sinking in
/// water, and never a gap jump), within its follow range of its seat
/// ([`crate::compiler::nav::World::reach_into_volumes`]). The wave thins itself
/// before the party touches it, on every life, and anything a member drops lands
/// inside the box.
///
/// Judged over the world as built, every fence gate shut — a mob cannot open
/// one. The same reach through a barrier a player can open is `DW0923`.
pub const DW_WAVE_REACHES_LETHAL: DwCode = DwCode::new("DW0922", ExitTier::Build);

/// `DW0923`: **a seated wave that reaches a lethal volume through a barrier the
/// party can leave open.** Every fence gate, door and trapdoor a player opens by
/// hand ([`delvewright_dsl::blockshape::is_player_openable`]) is judged in its
/// open state: a player who opens one may leave it open and die, and the wave
/// re-seated after that death walks through it. Raised only where `DW0922` is
/// not: the as-built reach is clear, and the opened one is not.
pub const DW_WAVE_REACHES_LETHAL_OPENED: DwCode = DwCode::new("DW0923", ExitTier::Build);

/// One way a wave member gets into a lethal volume.
#[derive(Clone, Debug)]
pub struct WaveLethalFinding {
    /// The wave's id.
    pub wave: String,
    /// The member's entity id.
    pub entity: String,
    /// The follow range its reach was bounded by, in blocks.
    pub radius: f64,
    /// The volume's id.
    pub volume: String,
    /// How it got in.
    pub hit: crate::compiler::nav::VolumeHit,
    /// For `DW0923`, the barriers left open on the way, as `(cell, block)`.
    pub opened: Vec<([i32; 3], String)>,
    /// Whether the volume is live from a story stage (spec-0088 §6) — judged
    /// as live, and said so.
    pub staged: bool,
}

impl WaveLethalFinding {
    fn route(&self) -> String {
        let seat = self.hit.path.first().copied().unwrap_or([0, 0, 0]);
        let from = self.hit.path.last().copied().unwrap_or(seat);
        let at = self.hit.way_in.last().copied().unwrap_or(from);
        // The turns of the way, so a reader can find the doorway it took.
        let turns: Vec<String> = self
            .hit
            .path
            .iter()
            .enumerate()
            .filter(|(i, c)| {
                let (i, c) = (*i, **c);
                if i == 0 || i + 1 == self.hit.path.len() {
                    return false;
                }
                let (a, b) = (self.hit.path[i - 1], self.hit.path[i + 1]);
                (0..3).any(|k| c[k] - a[k] != b[k] - c[k])
            })
            .map(|(_, c)| format!("{c:?}"))
            .collect();
        let by_way = if turns.is_empty() {
            String::new()
        } else {
            format!(" by way of {}", turns.join(", "))
        };
        let as_live = if self.staged {
            " (the volume is live from a story stage; the wave is judged against it as live, \
             because a wave that can walk into it thins itself once it wakes)"
        } else {
            ""
        };
        format!(
            "wave `{}`'s `{}` seated at {seat:?} walks {} move(s){by_way} to {from:?} and gets \
             into lethal volume `{}`{as_live} by {} at {at:?}, inside its follow range of {} \
             block(s)",
            self.wave,
            self.entity,
            self.hit.path.len().saturating_sub(1),
            self.volume,
            self.hit.how,
            self.radius,
        )
    }
}

/// What `DW0922` and `DW0923` examined, whether or not they refused.
#[derive(Clone, Debug, Default)]
pub struct WaveLethalBinding {
    /// Waves the seating pass seated.
    pub waves: usize,
    /// Mob stacks with at least one seat.
    pub stacks: usize,
    /// Seats flooded from.
    pub seats: usize,
    /// Lethal volumes judged against.
    pub volumes: usize,
    /// Cells a member can reach as built, summed over stacks.
    pub reached: usize,
    /// Player-openable barrier cells in the assembled world.
    pub openable: usize,
    /// Cells a member can reach with every one of those open, summed over stacks.
    pub reached_open: usize,
    /// `DW0922`: reach as built, first per `(wave, volume)`.
    pub as_built: Vec<WaveLethalFinding>,
    /// `DW0923`: reach only with the barriers open, first per `(wave, volume)`.
    pub opened: Vec<WaveLethalFinding>,
}

impl WaveLethalBinding {
    /// The one line a build prints about these proofs.
    pub fn line(&self) -> String {
        format!(
            "wave-lethal binding: {} seated wave(s), {} stack(s) over {} seat(s), flooded by the \
             movement a mob has within its follow range against {} lethal volume(s): {} cell(s) a \
             member can reach as built, {} with all {} barrier cell(s) a player can open left \
             open; {} (wave, volume) pair(s) reached as built (DW0922), {} only through an open \
             barrier (DW0923)",
            self.waves,
            self.stacks,
            self.seats,
            self.volumes,
            self.reached,
            self.reached_open,
            self.openable,
            self.as_built.len(),
            self.opened.len(),
        )
    }

    /// The same counts as `validation/wave-lethal.json`.
    pub fn to_json(&self) -> serde_json::Value {
        let row = |f: &WaveLethalFinding| {
            serde_json::json!({
                "wave": f.wave,
                "entity": f.entity,
                "volume": f.volume,
                "how": f.hit.how,
                "path": f.hit.path,
                "way_in": f.hit.way_in,
                "opened": f.opened.iter().map(|(c, b)| serde_json::json!({"cell": c, "block": b})).collect::<Vec<_>>(),
            })
        };
        serde_json::json!({
            "codes": [DW_WAVE_REACHES_LETHAL.id(), DW_WAVE_REACHES_LETHAL_OPENED.id()],
            "waves": self.waves,
            "stacks": self.stacks,
            "seats": self.seats,
            "volumes": self.volumes,
            "reached": self.reached,
            "openable": self.openable,
            "reached_open": self.reached_open,
            "as_built": self.as_built.iter().map(row).collect::<Vec<_>>(),
            "opened": self.opened.iter().map(row).collect::<Vec<_>>(),
        })
    }

    /// The verdict: the first `DW0922`, else the first `DW0923`.
    pub fn verdict(&self) -> Result<(), Failure> {
        let tail = |list: &[WaveLethalFinding]| {
            if list.len() < 2 {
                return String::new();
            }
            let rest: Vec<String> = list[1..]
                .iter()
                .map(|f| format!("wave `{}`'s `{}` into `{}`", f.wave, f.entity, f.volume))
                .collect();
            format!("; the same holds for {}", rest.join(", "))
        };
        if let Some(f) = self.as_built.first() {
            return Err(Failure {
                code: DW_WAVE_REACHES_LETHAL,
                message: format!(
                    "{}{}. A lethal volume kills every body that is not a player, wave members \
                     included, so this wave thins itself before the party touches it, on every \
                     life, and whatever a member drops lands in the box. A mob walks, steps or \
                     jumps onto a block at most one higher, drops off any edge and sinks in \
                     water; it never jumps a gap. Move the seat — the wave's `anchor` — so no \
                     member's reach meets the volume, or move the hazard where this wave cannot \
                     walk, fall or sink into it: behind a rise of two blocks, or across a gap of \
                     open air. Never buy the distance by shrinking `follow_range`, which retunes \
                     the fight to hide the placement; do not ring the hazard with blocks nobody \
                     can see, and do not delete the volume to silence this.",
                    f.route(),
                    tail(&self.as_built),
                ),
            });
        }
        if let Some(f) = self.opened.first() {
            let barriers = if f.opened.is_empty() {
                "a barrier a player can open".to_string()
            } else {
                f.opened
                    .iter()
                    .map(|(c, b)| format!("`{b}` at {c:?}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            return Err(Failure {
                code: DW_WAVE_REACHES_LETHAL_OPENED,
                message: format!(
                    "{} — once the party leaves open {barriers}{}. Shut, it keeps the wave out; \
                     open, it lets the wave in. Every fence gate, door and trapdoor a player \
                     opens by hand is judged open, because a player who opens one may leave it \
                     open and die, and the wave re-seated after that death walks through. Take \
                     the opening away rather than trust it to be shut: replace the barrier with \
                     a crossing the party makes and a mob does not — a jump across a dry cut, a \
                     gap of open air a player jumps and a mob never does — or move the wave's \
                     `anchor` or the volume so the opened way does not join them.",
                    f.route(),
                    tail(&self.opened),
                ),
            });
        }
        Ok(())
    }
}

/// `DW0922` and `DW0923` over every seated wave: flood each mob stack's
/// movement from its seats, within its follow range, over the world with no
/// lethal exclusion — once with the world as built (every fence gate shut, as a
/// mob finds it) and once with every barrier a player can open standing open —
/// and record the first way each wave gets into each volume.
///
/// The follow range is a lane's `aggro_radius` (which the compiler emits as each
/// lane mob's `follow_range`), else the stack's declared `follow_range`, else
/// [`crate::compiler::nav::DEFAULT_FOLLOW_RANGE`] — the radius `DW0478` reads.
/// The roots are the seats the seating pass chose, so `wave_seats` is that
/// pass's answer, handed over as `DW0511` takes it.
pub fn wave_reach(
    plan: &Plan,
    world: &crate::compiler::nav::World,
    blocks: &crate::compiler::blockstate::BlockMap,
    wave_seats: &std::collections::BTreeMap<String, Vec<[i32; 3]>>,
) -> WaveLethalBinding {
    use crate::compiler::nav::World;
    let mut b = WaveLethalBinding {
        volumes: plan.lethal_volumes.len(),
        ..WaveLethalBinding::default()
    };
    if plan.lethal_volumes.is_empty() {
        return b;
    }
    let vols: Vec<([i32; 3], [i32; 3])> = plan.lethal_volumes.iter().map(|v| v.region).collect();
    let open = world.without_exclusions();
    let shut_owned;
    let shut: &World = if open.has_use_gates() {
        shut_owned = open.without_gate_use();
        &shut_owned
    } else {
        &open
    };
    let openable: std::collections::BTreeSet<[i32; 3]> = blocks
        .iter()
        .filter(|(_, n)| delvewright_dsl::blockshape::is_player_openable(n.as_str()))
        .map(|(c, _)| *c)
        .collect();
    b.openable = openable.len();
    let opened_owned = (!openable.is_empty()).then(|| open.with_openings_open(&openable));
    for w in &plan.campaign.quests.content.waves {
        let Some(cells) = wave_seats.get(w.id.as_str()) else {
            continue;
        };
        b.waves += 1;
        let mut seat = 0usize;
        for mob in &w.mobs {
            let roots: Vec<[i32; 3]> = (0..mob.count as usize)
                .filter_map(|i| cells.get(seat + i).copied())
                .collect();
            seat += mob.count as usize;
            if roots.is_empty() {
                continue;
            }
            b.stacks += 1;
            b.seats += roots.len();
            let radius = match &w.lane {
                Some(l) => f64::from(l.aggro_radius),
                None => mob
                    .attributes
                    .and_then(|a| a.follow_range)
                    .unwrap_or(f64::from(crate::compiler::nav::DEFAULT_FOLLOW_RANGE)),
            };
            let fp = crate::compiler::nav::entity_footprint(&mob.entity);
            let built = shut.reach_into_volumes(&roots, &fp, true, Some(radius), &vols);
            b.reached += built.reached.len();
            let opened = opened_owned
                .as_ref()
                .map(|o| o.reach_into_volumes(&roots, &fp, true, Some(radius), &vols));
            b.reached_open += opened
                .as_ref()
                .map_or(built.reached.len(), |r| r.reached.len());
            for (i, v) in plan.lethal_volumes.iter().enumerate() {
                let seen = |list: &[WaveLethalFinding]| {
                    list.iter()
                        .any(|f| f.wave == w.id.as_str() && f.volume == v.id)
                };
                let finding = |hit: &crate::compiler::nav::VolumeHit, opened| WaveLethalFinding {
                    wave: w.id.as_str().to_string(),
                    entity: mob.entity.clone(),
                    radius,
                    volume: v.id.clone(),
                    hit: hit.clone(),
                    opened,
                    staged: v.staged.is_some(),
                };
                if let Some(hit) = &built.hits[i] {
                    if !seen(&b.as_built) {
                        b.as_built.push(finding(hit, Vec::new()));
                    }
                    continue;
                }
                let Some(hit) = opened.as_ref().and_then(|r| r.hits[i].as_ref()) else {
                    continue;
                };
                if seen(&b.opened) || seen(&b.as_built) {
                    continue;
                }
                b.opened
                    .push(finding(hit, barriers_on(hit, &openable, blocks)));
            }
        }
    }
    // A pair first opened by one stack and then reached as built by another is
    // the as-built finding.
    let built: Vec<(String, String)> = b
        .as_built
        .iter()
        .map(|f| (f.wave.clone(), f.volume.clone()))
        .collect();
    b.opened
        .retain(|f| !built.contains(&(f.wave.clone(), f.volume.clone())));
    b
}

/// The player-openable barriers a way into a volume passes: the cells the body
/// occupies on its way (feet and head) and the cells it falls or sinks through,
/// else any barrier beside them.
fn barriers_on(
    hit: &crate::compiler::nav::VolumeHit,
    openable: &std::collections::BTreeSet<[i32; 3]>,
    blocks: &crate::compiler::blockstate::BlockMap,
) -> Vec<([i32; 3], String)> {
    let mut on: std::collections::BTreeSet<[i32; 3]> = std::collections::BTreeSet::new();
    for c in hit.path.iter().chain(&hit.way_in) {
        for dy in [0, 1] {
            let cell = [c[0], c[1] + dy, c[2]];
            if openable.contains(&cell) {
                on.insert(cell);
            }
        }
    }
    if on.is_empty() {
        for c in hit.path.iter().chain(&hit.way_in) {
            for d in [[-1, 0], [1, 0], [0, -1], [0, 1]] {
                for dy in [-1, 0, 1] {
                    let cell = [c[0] + d[0], c[1] + dy, c[2] + d[1]];
                    if openable.contains(&cell) {
                        on.insert(cell);
                    }
                }
            }
        }
    }
    on.into_iter()
        .map(|c| {
            (
                c,
                blocks
                    .get(&c)
                    .map(|b| b.as_str().to_string())
                    .unwrap_or_default(),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, reached_by: Option<&str>) -> VolumeVisibility {
        VolumeVisibility {
            id: id.to_string(),
            configuration: None,
            live: true,
            population: 10,
            keep_out: ([0, 0, 0], [0, 0, 0]),
            caught: Vec::new(),
            shown: Vec::new(),
            shown_by: Vec::new(),
            reached_by: reached_by.map(str::to_string),
        }
    }

    fn hit() -> crate::compiler::nav::VolumeHit {
        crate::compiler::nav::VolumeHit {
            path: vec![[0, 1, 0], [1, 1, 0]],
            way_in: vec![[2, 1, 0]],
            how: "stepping in",
        }
    }

    fn finding(volume: &str, opened: Vec<([i32; 3], String)>) -> WaveLethalFinding {
        WaveLethalFinding {
            wave: "wave/choir".to_string(),
            entity: "minecraft:drowned".to_string(),
            radius: 16.0,
            volume: volume.to_string(),
            hit: hit(),
            opened,
            staged: false,
        }
    }

    /// A volume no modelled body gets into is a zero binding, and it is said:
    /// one `DW0891` warning per such volume, none for a reached one.
    #[test]
    fn a_volume_no_body_reaches_is_a_dw0891_warning() {
        let d = DangerVisibility {
            declared: 2,
            population: 10,
            configurations: 1,
            volumes: vec![
                row("lethal/well", None),
                row("lethal/pit", Some("a player")),
            ],
            staged: Vec::new(),
            declarations: 0,
            borne_out: 0,
        };
        let f = d.findings();
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].code, "DW0891");
        assert_eq!(f[0].severity, delvewright_dsl::Severity::Warning);
        assert!(f[0].message.contains("lethal/well"), "{}", f[0].message);
        assert!(
            d.line().contains("1 of 2 volume(s) reached"),
            "{}",
            d.line()
        );
    }

    /// A volume only a wave member gets into is bound: a body is a body.
    #[test]
    fn a_wave_member_binds_a_volume_no_player_reaches() {
        let mut d = DangerVisibility {
            declared: 1,
            population: 10,
            configurations: 1,
            volumes: vec![row("lethal/well", None)],
            staged: Vec::new(),
            declarations: 0,
            borne_out: 0,
        };
        let waves = WaveLethalBinding {
            opened: vec![finding("lethal/well", Vec::new())],
            ..WaveLethalBinding::default()
        };
        d.credit_waves(&waves);
        assert!(d.findings().is_empty());
        assert_eq!(d.unreached(), 0);
    }

    /// As-built reach is `DW0922` and outranks a reach through an opened barrier;
    /// the opened one alone is `DW0923`, naming the barrier.
    #[test]
    fn the_verdict_is_dw0922_before_dw0923() {
        let gate = vec![([1, 1, 0], "minecraft:oak_fence_gate".to_string())];
        let both = WaveLethalBinding {
            as_built: vec![finding("lethal/a", Vec::new())],
            opened: vec![finding("lethal/b", gate.clone())],
            ..WaveLethalBinding::default()
        };
        assert_eq!(both.verdict().unwrap_err().code.id(), "DW0922");
        let opened = WaveLethalBinding {
            opened: vec![finding("lethal/b", gate)],
            ..WaveLethalBinding::default()
        };
        let err = opened.verdict().unwrap_err();
        assert_eq!(err.code.id(), "DW0923");
        assert!(
            err.message.contains("minecraft:oak_fence_gate"),
            "{}",
            err.message
        );
        assert!(WaveLethalBinding::default().verdict().is_ok());
    }
}
