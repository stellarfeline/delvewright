//! Deterministic emission of the `<out>/` build tree (spec-0002).
//!
//! All gameplay wiring is compiler-generated (ADR-0001): the LLM never writes
//! mcfunction. Output is a `BTreeMap<path, bytes>` so ordering is defined
//! (ADR-0006); `manifest.json` hashes make the double-build gate a one-line
//! comparison.
//!
//! JSON is serialized with `serde_json` (default `BTreeMap` maps → sorted keys)
//! plus a trailing newline; mcfunction bodies are built line-by-line. No
//! wall-clock, hostname, locale or absolute path enters any byte — and no
//! ambient state either: every value a build writes is a function of the
//! documents, prefabs and flags it was handed, never of where it was launched
//! from or what happens to sit above that directory.

use crate::compiler::failure::Failure;

use delvewright_dsl::Verb;

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use sha2::{Digest, Sha256};

use crate::compiler::commands::{CommandError, CommandTree};

use crate::compiler::plan::{
    self, Plan, ResolvedAnchor, Step, campaign_start_quests, obj_score, objective_effects,
    objective_quest, quest_active_score, quest_score,
};

use crate::compiler::{DELVEC_VERSION, MC_VERSION, PACK_FORMAT};

use delvewright_dsl::{
    CompareOp, EquipItem, EquipSlot, Gate, MobEquipment, Objective, QuestEffect, StateCompare,
    StateId, StateScope, Trigger,
};

use delvewright_dsl::{DwCode, ExitTier};

mod actor;
mod advancement;
mod affordance;
mod atmosphere;
mod boundary;
mod branch;
mod cast;
mod checkpoint;
mod class;
mod coords;
mod cutscene;
mod dialogue;
mod economy;
mod effect;
mod equipment;
mod firework;
mod functions;
mod lane;
mod lethal;
mod lightning;
mod r#loop;
mod loot;
mod manifest;
mod npc;
mod objective;
mod onkill;
mod packtest;
mod piece;
mod proofs;
mod quest;
mod removal;
mod respawn;
mod routes;
mod seal;
mod sequence;
mod server;
mod shortcut;
mod stake;
mod state;
mod stealth;
mod teleport;
#[cfg(test)]
mod tests;
mod text;
mod timed_gate;
mod trap;
mod trigger;
mod wave;
mod world;

use actor::*;
use advancement::*;
use affordance::*;
pub use atmosphere::*;
pub use boundary::*;
pub use branch::*;
use cast::*;
use checkpoint::*;
use class::*;
pub(crate) use coords::*;
pub(crate) use cutscene::*;
pub use dialogue::*;
use economy::*;
pub use effect::*;
pub(crate) use equipment::*;
use firework::*;
use functions::*;
use lane::*;
use lethal::*;
use lightning::*;
use r#loop::*;
use loot::*;
use manifest::*;
pub(crate) use npc::*;
use objective::*;
use onkill::*;
pub use piece::*;
use proofs::*;
use quest::*;
use removal::*;
use respawn::*;
use routes::*;
pub use seal::*;
use sequence::*;
pub use server::*;
use shortcut::*;
use stake::*;
use state::*;
pub use stealth::*;
use teleport::*;
pub use text::*;
use timed_gate::*;
use trap::*;
use trigger::*;
use wave::*;
use world::*;

/// The emitted build tree: relative path → file bytes.
pub type BuildOutput = BTreeMap<String, Vec<u8>>;

/// Why a build failed. Either emitted vanilla commands failed the command-tree
/// validator, or a geometry/navigation check raised a `DW03xx` diagnostic
/// (`DW0307` unroutable `move-npc`, `DW0308` cutscene camera clipping a solid).
#[derive(Debug)]
pub enum BuildFailure {
    /// One or more emitted `.mcfunction` commands failed validation.
    Validation(Vec<CommandError>),
    /// A coded build diagnostic (exit 3), printed like a solver `DW03xx` error.
    Diagnostic {
        /// The stable diagnostic code.
        code: DwCode,
        /// Human-readable explanation.
        message: String,
    },
}

delvewright_dsl::dw_code! {
    /// `DW0312`: a `spawn-wave` needs more standable spawn cells near its anchor than
    /// the anchor's own assembled room provides. Wave-mob placement seats
    /// each mob on a compiler-validated standable cell confined to that room; when the
    /// wave's mob count exceeds the room's footing, the build fails here rather than
    /// letting mobs pile into blocks or spill across a socket seam. Analysis-tier
    /// (exit 2, like reachability `DW02xx`): the fix is a content-design capacity
    /// choice — shrink the wave or use a larger room — not a compiler/geometry defect.
    pub const DW_WAVE_NO_ROOM: DwCode = DwCode::new("DW0312", ExitTier::Analysis);
}

delvewright_dsl::dw_code! {
    /// `DW0310`: a `spawn-wave` references a wave whose spawn anchor resolves in no
    /// assembled area, so the emitted `function <ns>:spawn_<wave>` call would dangle
    /// and the wave never spawn (see [`check_wave_spawns`]).
    ///
    /// It was the workspace's last bare `"DWxxxx"` string literal in a code position
    /// — every other code already went through a named constant — and typing the
    /// codes is what turned that from a style difference into a compile error.
    pub const DW_WAVE_SPAWN_UNRESOLVED: DwCode = DwCode::new("DW0310", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0387`: a `summon: aggro-edge` wave (spec-0016 §6) whose perception ring
    /// offers too few valid cells. The ring is the standable, walk-reachable,
    /// line-of-sight cells at a mob's own `follow_range` from the defended anchor;
    /// with fewer of them than the wave has mobs there is nowhere legal to
    /// materialize. This is an error and not a silent short spawn on purpose — the
    /// round-1 lesson was a "kill" objective whose wave never fully appeared, so the
    /// countdown could never reach zero and the delve soft-locked with every other
    /// proof green.
    pub const DW_AGGRO_EDGE_NO_RING: DwCode = DwCode::new("DW0387", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0494`: completing ONE objective would cross into two different areas —
    /// one destination on the exported path, another on a branch.
    ///
    /// The crossing is emitted into the objective's own completion bundle, so the
    /// two destinations would be two teleports in one function body, and which one
    /// the party lands on would depend on command order rather than on the branch
    /// they are actually playing. There is no runtime distinction to gate on either:
    /// the exported path's crossing is unconditional by construction. The content
    /// fix is to split the objective — one crossing objective per branch, each
    /// gated by that branch's own flags.
    pub const DW_BRANCH_TRANSPORT_DIVERGES: DwCode = DwCode::new("DW0494", ExitTier::Build);
}

impl From<Failure> for BuildFailure {
    fn from(e: Failure) -> Self {
        BuildFailure::Diagnostic {
            code: e.code,
            message: e.message,
        }
    }
}

delvewright_dsl::dw_code! {
    /// `DW0803`: a placed structure template is not the size the prefab metadata
    /// says it is.
    ///
    /// Two documents claim the same fact — the `.nbt`'s own `size` tag, and the
    /// metadata's `structure.size` (or a tile's `structure_set.parts[].size`) — and
    /// **every pass but the placement itself reads the metadata's**. The forceload
    /// span, the piece AABB the mating check compares, massing's footprint and the
    /// tiling arithmetic that puts a tile at its offset are all computed from the
    /// declared size; the blocks come from the bytes. When they disagree the world
    /// is built wrong in a way no other check can see, because each half is
    /// internally consistent.
    ///
    /// Tiling is what makes this reachable: a zone's manifest and its tiles are
    /// several files that a `cp`, a partial re-export or a hand edit can leave at
    /// different ages, and a stale tile then lands at the offset the manifest gives
    /// it — sliding part of a building through the rest of it. A single-template
    /// prefab has the same exposure and had the same silence.
    ///
    /// Build tier: the world would be wrong, so it is not built.
    pub const DW_TEMPLATE_EXTENT: DwCode = DwCode::new("DW0803", ExitTier::Build);
}

/// Build the full `<out>/` tree from a plan and the prefab structure bytes
/// (`structure_file` → raw `.nbt`). Runs the command-tree validator over every
/// emitted `.mcfunction`; a validation failure is a build error.
///
/// `language` is the target build language (i18n): `None` or `Some("en")` is the
/// canonical English build (the manifest records no `language`, so an English
/// build stays byte-identical to a pre-i18n one); `Some("<code>")` records the
/// language in the manifest. The `plan`'s campaign must already be localized to
/// that language by the caller ([`delvewright_dsl::localize`]).
pub fn build(
    plan: &Plan,
    input_bytes: &BTreeMap<String, Vec<u8>>,
    structures: &BTreeMap<String, Vec<u8>>,
    tree: &CommandTree,
    prefabs: &crate::compiler::registry::PrefabRegistry,
    language: Option<&str>,
    skins: &BTreeMap<String, Vec<u8>>,
) -> Result<BuildOutput, BuildFailure> {
    build_with_warnings(
        plan,
        input_bytes,
        structures,
        tree,
        prefabs,
        language,
        skins,
    )
    .map(|(out, _)| out)
}

/// [`build`], additionally returning the advisory diagnostics the build raised.
///
/// Warning-tier findings that only the *built* model can see (currently the
/// stage-7 edit replay's `DW0353`/`DW0354`) have no other channel: they are
/// discovered after `dsl::validate` has run and long after `analyze`. `build`
/// stays the discard-warnings convenience wrapper so every existing caller —
/// and every test asserting byte-identical output — is untouched.
pub fn build_with_warnings(
    plan: &Plan,
    input_bytes: &BTreeMap<String, Vec<u8>>,
    structures: &BTreeMap<String, Vec<u8>>,
    tree: &CommandTree,
    prefabs: &crate::compiler::registry::PrefabRegistry,
    language: Option<&str>,
    skins: &BTreeMap<String, Vec<u8>>,
) -> Result<(BuildOutput, Vec<delvewright_dsl::Diagnostic>), BuildFailure> {
    let ns = &plan.namespace;
    let mut out: BuildOutput = BTreeMap::new();

    // Every campaign must resolve an ENTRY POINT (DW0345). Without one the world
    // gets no `setworldspawn`, a class-picking player is never teleported, and a
    // joining player is left to the vanilla spawn search — which a dedicated server
    // resolves to the surface but the integrated (singleplayer) server resolves to
    // the build floor, i.e. inside solid stone. This used to fail silently: an area
    // whose tileset spells the anchor `entry` instead of `spawn` compiled clean and
    // shipped a delve with no start.
    //
    // FIRST, before any model, any walk and any other check. A world with no
    // start does not have a walking problem: with nothing to transport the party
    // into, an inter-area crossing is left to the critical-path walk, which then
    // reports `DW0311` — *the player cannot walk from [5, 65, 8] to
    // [262, 66, 1]* — about a crossing that was never meant to be walked. That is
    // the symptom reported as the fault, and it sent a reader to look for a
    // wedged doorway in a world whose real defect was that nothing said where the
    // party arrives.
    if campaign_spawn(plan).is_none() {
        return Err(BuildFailure::Diagnostic {
            code: plan::DW_NO_ENTRY_ANCHOR,
            message: format!(
                "the assembled world resolves no entry anchor — no area places a \
                 piece whose prefab metadata declares an anchor with \
                 `\"role\": \"{role}\"`. The compiler then has no cell to call the \
                 campaign's start: no `setworldspawn`, no class-apply teleport, no \
                 first-join placement. An anchor's NAME is never consulted for this, \
                 so no spelling supplies it. Fix it where the anchors are declared: \
                 give the piece the party arrives in an anchor at that cell and put \
                 `\"role\": \"{role}\"` on it (in a pool, that is the prefab the \
                 layout is seeded from), or bind the area to a prefab that already \
                 has one. Every producer can write it: `delvec prefab anchor <nbt> \
                 --name <anchor> --pos <x,y,z> --role {role}` for a hand-built or \
                 ingested piece, `\"role\": \"{role}\"` on the `mark` for a grammar \
                 program.",
                role = plan::AnchorRole::Entry,
            ),
        });
    }

    // **Which biome stands where** (spec-0080 §4): the map every reader asks,
    // its build-tier refusals (`DW0929`: two carried places whose 4-cells meet
    // under different atmospheres; a repaint past the map's extent), and the
    // binding line — printed on every build, a campaign with no atmosphere
    // included, because a zero is a measurement.
    let biome_map = {
        let map = crate::compiler::horizon::biome_map(plan);
        crate::compiler::horizon::check_paints(plan, &map).map_err(|e| {
            BuildFailure::Diagnostic {
                code: e.code,
                message: e.message,
            }
        })?;
        eprintln!("{}", atmosphere_binding(plan, &map).line());
        map
    };

    // The templates are the size their metadata says they are (DW0803). Bound
    // here, before any model is built out of them, because every later pass —
    // the forceload span, the mating check, massing, the whole assembled world
    // — is computed from the metadata's `size` while the blocks come from the
    // bytes, and nothing else compares the two.
    let template_binding = check_template_extents(plan, structures)?;
    // Stated with the verdict: a check that examined nothing is not a pass.
    let mut extent_findings = template_binding.finding().into_iter().collect::<Vec<_>>();

    // Gravity-despawn gate: before any downstream model
    // is built, reject a prefab whose gravity floor (sand/gravel/…) sits
    // unsupported over the delve's void world and would despawn at placement,
    // silently deforming the shipped map. This is the authoritative direct gate —
    // it does not wait for a fall to happen to intersect the critical path (DW0311)
    // or a wave seat (DW0312). Analysis-tier (exit 2, mapped in main): a
    // prefab/generator defect the author fixes by adding a substrate. No-op for any
    // campaign whose prefabs have no gravity blocks (byte-identical output).
    //
    // The world is assembled here, once, and every model below is derived from
    // this one assembly (or from the edit replay's edited copy of it).
    let mut pristine = Some(crate::compiler::assembled::assemble(plan, structures));
    if let Some(message) = crate::compiler::assembled::gravity_despawn_error_of(
        plan,
        pristine.as_ref().expect("assembled above"),
    ) {
        return Err(BuildFailure::Diagnostic {
            code: crate::compiler::assembled::DW_GRAVITY_DESPAWN,
            message,
        });
    }

    // Stage-7 edit-script replay (spec-0017): apply the campaign's world edits
    // over the assembled model, re-proving the invariants after every batch
    // (gravity, relight, walkability, boundary safety — each failure names its
    // batch). `None` for a campaign without an edit script — every downstream
    // pass then takes its exact pre-stage-7 path, byte-identically.
    let edit_replay =
        crate::compiler::edit::replay_taking(plan, prefabs, structures, &mut pristine).map_err(
            |e| BuildFailure::Diagnostic {
                code: e.code,
                message: e.message,
            },
        )?;
    // The one assembled world every pass below reads: the edited copy when the
    // campaign has an edit script, the pristine assembly otherwise.
    let assembled: &crate::compiler::assembled::Assembled = match &edit_replay {
        Some(er) => &er.assembled,
        None => pristine
            .as_ref()
            .expect("an assembly the replay did not take"),
    };
    // Advisory findings the replay raised (`DW0353` gate-region collisions,
    // `DW0354` broken block support) — reported by the caller, never fatal.
    // Advisory findings the PLACEMENT stage raised (`DW0498`: a pool draw that
    // seats the same anchor-bearing prefab twice) lead, since they describe the
    // world every later pass reasons over, then the replay's own.
    let mut warnings: Vec<delvewright_dsl::Diagnostic> = plan.warnings.clone();
    warnings.append(&mut extent_findings);
    warnings.extend(
        edit_replay
            .as_ref()
            .map_or_else(Vec::new, |er| er.warnings.clone()),
    );
    // spec-0016 §7 pacing lints, filled in by the nav stage below.
    let mut pacing: Vec<delvewright_dsl::Diagnostic> = Vec::new();

    // spec-0021 container proof (DW0431). Runs off the assembled world — over
    // the EDITED model when an edit script exists, since a stage-7 batch can
    // legitimately be what puts the barrel there. Independent of nav, because a
    // campaign may declare loot without ever walking a leg.
    // The same proof serves the v0.8 `collect` container adoption
    // (DW0438): an adopted container is prefab furniture on exactly the terms a `loot`
    // container is, so it is proven off the same assembled (or edited) world, in
    // the same pass, rather than by a second model that could disagree with this
    // one about what is in the room.
    let has_steps = plan
        .campaign
        .quests
        .content
        .triggers
        .iter()
        .any(|t| matches!(t.on, delvewright_dsl::TriggerOn::Step));
    // A fill whose cell lies in a place still standing in its stand-in is owed by
    // that place's detail: judged by the build that details it, counted here.
    let (judged_loot, judged_fills, containers) = crate::compiler::loot::split_owed_by_detail(
        &assembled.blocks,
        plan.blockout.as_ref(),
        &plan.loot,
        &plan.collect_fills,
    );
    eprintln!("{}", containers.line());
    if !plan.loot.is_empty()
        || !plan.collect_fills.is_empty()
        || !plan.traps.is_empty()
        || has_steps
    {
        let blocks = &assembled.blocks;
        // What the world actually HAS, computed once and handed to both proofs.
        // A refusal that tells an author to point at "an anchor whose cell
        // already has one" owes the list, and it is the same list for both:
        // `DW0431` and `DW0438` ask one question about one object class through
        // two doors, so a second derivation here would be a second answer able
        // to disagree with the first.
        let available = crate::compiler::loot::container_anchors(
            blocks,
            &plan.anchors,
            &plan.loot,
            &plan.collect_fills,
        );
        crate::compiler::loot::check_loot_containers(blocks, &judged_loot, &available).map_err(
            |e| BuildFailure::Diagnostic {
                code: e.code,
                message: e.message,
            },
        )?;
        crate::compiler::loot::check_collect_containers(blocks, &judged_fills, &available)
            .map_err(|e| BuildFailure::Diagnostic {
                code: e.code,
                message: e.message,
            })?;
        // DW0917: a trap's trigger is prefab hardware on the same terms as a
        // container, so it is proven off the same block map.
        crate::compiler::trap_trigger::check_trap_triggers(blocks, &plan.traps, &plan.anchors)
            .map_err(|e| BuildFailure::Diagnostic {
                code: e.code,
                message: e.message,
            })?;
        // DW0917's `step` half: a step trigger's plate is the same hardware.
        let steps: Vec<crate::compiler::trap_trigger::StepCell<'_>> = plan
            .campaign
            .quests
            .content
            .triggers
            .iter()
            .filter(|t| matches!(t.on, delvewright_dsl::TriggerOn::Step))
            .filter_map(|t| {
                let anchor = t.at_anchor()?;
                Some(crate::compiler::trap_trigger::StepCell {
                    id: t.id.as_str(),
                    anchor,
                    cell: plan.point_any(anchor)?,
                })
            })
            .collect();
        crate::compiler::trap_trigger::check_step_triggers(blocks, &steps, &plan.anchors).map_err(
            |e| BuildFailure::Diagnostic {
                code: e.code,
                message: e.message,
            },
        )?;
    }

    // v0.4 navigation planning over the solved voxel grid (spec-0008 addendum):
    // collision-safe `move-npc` walked paths (DW0307) + cutscene air-corridor
    // checks (DW0308). Only built when the campaign uses those verbs, so v0.2/v0.3
    // output stays byte-identical (no world, no moves → the driver emitters are
    // empty exactly as before).
    // DW0311 also rides on this model: every walked critical-path leg must be
    // routable over the assembled seams (the compile-time counterpart to the
    // runtime critical-path bot).
    // Assembled-world lighting + deterministic relight pass (spec-0010): measure
    // real light over the assembled world, place declared fixtures, and gate on
    // measured darkness. Runs before nav verification so the fixtures it adds are
    // re-verified for walkability below. A `DW0210`/`DW0211` diagnostic
    // fails the build (exit 2, mapped in main). Empty for a campaign with no dark
    // reachable cells and no `lighting` declaration → output byte-identical.
    // The geometry is classified once: relight surveys it as it stands, and the
    // campaign's world below is the same cells under the campaign's premises.
    let geometry = crate::compiler::light::geometry_world(assembled);
    let relight = crate::compiler::light::relight_with(plan, assembled, &geometry);
    if let Some(diag) = relight.diagnostics.first() {
        return Err(BuildFailure::Diagnostic {
            code: diag.code,
            message: diag.message.clone(),
        });
    }

    // The voxel occupancy model backs both nav verification (move-npc / cutscene /
    // critical path) and spawn-wave mob placement, so build it once when
    // either needs it. The relight fixtures stand in it, each classified by the
    // one collision table ([`crate::compiler::light::lit_world`]), so a fixture
    // can never wedge a required path shut unseen (spec-0010: verification
    // re-runs after placement).
    //
    // It is built for EVERY campaign, not only the ones `assembles_world` says
    // need nav: the visual tier's clear-eye proof (`DW0724`) is owed by every
    // campaign that emits a render plan, and a render plan is emitted
    // unconditionally. Keying the model to the nav predicate would have left a
    // campaign with no walked leg deriving seven kinds of camera against no world
    // at all — a zero binding wearing a pass's clothes, which is precisely the
    // shape that let a camera stand inside a ceiling lantern for as long as it did.
    // The campaign's premises about this world — the generator ambient, the
    // built extent, the declared lethal volumes, the measured world-load gate
    // seals, the clocked gate regions and the teleport sources — travel as one
    // value, [`crate::compiler::nav::Premises`], so an edited world and a
    // pristine one carry the identical set.
    let world = crate::compiler::light::lit_world(
        assembled,
        &relight,
        geometry,
        crate::compiler::nav::Premises::of_plan(plan, assembled.gate_seals.clone()),
    );

    // What the camera reads in each carried place (spec-0080 §2.2): its fog
    // and sky are the client's blend over the 4-cells within `BLEND_REACH`, so
    // a place too small for its paint shows a sky mixed with the one outside
    // it. A box's eyes stand over its floor; an area's are the party's walk
    // inside its bounds, read off this world.
    if biome_map.places().next().is_some() {
        let walk = world.reachable_walkable_rooted(&crate::compiler::edit::anchor_starts(plan));
        for p in biome_map.places() {
            let crate::compiler::horizon::PaintSource::Place { place, .. } = &p.source else {
                continue;
            };
            if let Some((whole, eyes, best, worst)) =
                crate::compiler::horizon::standing_reach(plan, &biome_map, place, &p.biome, &walk)
            {
                eprintln!(
                    "atmosphere reach: `{place}` — {whole} of {eyes} standing eye(s) read `{}` \
                     whole; the best reads {:.1}% of it, the worst {:.1}%",
                    p.biome,
                    best * 100.0,
                    worst * 100.0
                );
            }
        }
    }

    // ---- the links the route proof takes (spec-0083 §3.4) ----
    //
    // Whether a walk fails is a question about blocks, so it is asked here, the
    // first point the final world exists. Every leg a walk cannot cross is
    // retried through the links live at its end, and each one taken is spliced
    // into the plan's ONE path ([`Plan::relinked`]) — so the walk proof, the
    // branch proofs, the waypoint export, the bot contract, the leave proof and
    // both party populations below all read the same steps. A campaign whose
    // legs all walk takes nothing and keeps the plan it was handed, byte for
    // byte.
    let relinked = crate::compiler::nav::with_links_taken(plan, prefabs, &world)?;
    let plan: &Plan = relinked.as_ref().unwrap_or(plan);

    // ---- spec-0082: the assemblies (`DW0936`–`DW0938`) ----
    //
    // The binding is kept: the critical path carries the bot's witness of each
    // blow, and the staging record states which were witnessed.
    let assembly_binding: Option<crate::compiler::assembly::AssemblyBinding> =
        Some(prove_assemblies(plan, &world)?);

    // ---- the stage-5 blockout battery (spec-0049 §5.3) ----
    prove_blockout(plan, assembled, &mut warnings)?;

    // ---- the declaration proofs: no occupancy model, `DW0360` first ----
    let teleport_gate = prove_declarations(plan, &mut warnings)?;

    // ---- the world block: every proof over the assembled world ----
    let WorldProofs {
        moves,
        actor_moves,
        wave_placements,
        wave_rings,
        lane_routes,
        payload_plans,
        pov_shots,
        branch_waypoints,
        branch_takes,
        legs_carried,
        path_legs,
        traversal_gate,
        gate_seal_ledger,
        fluid_escape_ledger,
        sea_seepage_ledger,
        piece_exposure_ledger,
        lethal_gate,
        loop_gate,
        firework_gate,
        cutscene_shots_judged,
        lightning_gate,
        stake_table,
        death_plan,
    } = prove_world(
        plan,
        &world,
        assembled,
        prefabs,
        structures,
        &relight,
        &mut out,
        &mut warnings,
        &mut pacing,
    )?;

    // Every `spawn-wave` effect must resolve a spawn position, or its emitted
    // `function <ns>:spawn_<wave>` call would dangle to a never-emitted function and
    // the wave would silently never spawn (DW0310). Guards against the class of bug
    // where the spawn position was resolvable only via a `kill` objective.
    check_wave_spawns(plan)?;
    // DW0863 (spec-0093 §5): a `kill` objective is announced with a hint, or its
    // wave arrives within reach of the act that spawns it. Judged here, where
    // the acts have places; the binding is printed whichever way it goes.
    {
        let (fights, verdict) = crate::compiler::promise::check_fight_signposts(plan);
        eprintln!("{}", fights.line());
        if let Err(f) = verdict {
            return Err(BuildFailure::Diagnostic {
                code: f.code,
                message: f.message,
            });
        }
    }
    // DW0963 (spec-0093 §7): an unmarked `interact` with no `prop` stands on a
    // block the piece authored, or the party is asked to press empty space.
    // Read over the settled bytes, which only an assembled world has.
    if assembles_world(plan) {
        let (pressables, verdict) =
            crate::compiler::promise::check_pressables_visible(plan, &assembled.blocks);
        eprintln!("{}", pressables.line());
        if let Err(f) = verdict {
            return Err(BuildFailure::Diagnostic {
                code: f.code,
                message: f.message,
            });
        }
    }

    // ---- datapack ----
    put_json(
        &mut out,
        "datapack/pack.mcmeta",
        &json!({
            "pack": {
                "description": format!("Delvewright delve: {ns}"),
                "min_format": PACK_FORMAT,
                "max_format": PACK_FORMAT,
            }
        }),
    );
    put_json(
        &mut out,
        "datapack/data/minecraft/tags/function/load.json",
        &json!({ "values": [format!("{ns}:load")] }),
    );
    put_json(
        &mut out,
        "datapack/data/minecraft/tags/function/tick.json",
        &json!({ "values": [format!("{ns}:tick")] }),
    );

    // structures (one `.nbt` per distinct structure id, even if reused across
    // several placed pieces — the insert is idempotent, same bytes). Each is
    // shipped as `/place template` must receive it: a `structure_void` cell is
    // one the piece does not place, which is how every model above read it,
    // and the game would write the void block over whatever stands there
    // (`admit::structure::as_placed`).
    let placed: Vec<_> = plan.placed_pieces().flat_map(|p| &p.templates).collect();
    let shipped: BTreeMap<&str, Vec<u8>> = {
        let files: BTreeSet<&str> = placed
            .iter()
            .map(|t| t.structure_file.as_str())
            .filter(|f| structures.contains_key(*f))
            .collect();
        let files: Vec<&str> = files.into_iter().collect();
        let bytes = crate::par::map(&files, |f| {
            let raw = &structures[*f];
            crate::admit::structure::as_placed(raw).unwrap_or_else(|| raw.clone())
        });
        files.into_iter().zip(bytes).collect()
    };
    for template in &placed {
        if let Some(bytes) = shipped.get(template.structure_file.as_str()) {
            out.insert(
                format!("datapack/data/{ns}/structure/{}.nbt", template.structure_id),
                bytes.clone(),
            );
        }
    }

    // functions
    // Placement sentinels: one known block per distinct structure, read off the
    // bytes that ship, so the runtime can verify each `place template` landed
    // (see `setup` emission).
    let mut sentinels: Sentinels = BTreeMap::new();
    let picked = crate::par::map(&placed, |template| {
        shipped
            .get(template.structure_file.as_str())
            .and_then(|bytes| structure_sentinel(bytes))
    });
    for (template, picked) in placed.into_iter().zip(picked) {
        if let Some(s) = picked {
            sentinels.insert(template.structure_file.clone(), s);
        }
    }
    // Trap flag gating (DSL v0.6): resolve the authored trigger hardware for every
    // gated trap, rejecting a trigger the compiler cannot restore (DW0363).
    let trap_gates = trap_gate_hardware(plan, prefabs)?;
    // The inter-area crossings that exist only on a branch. Computed
    // here so `DW0494` fails the build before a single function is emitted.
    let branch_transport = branch_transport_overlay(plan)?;

    // spec-0029 addendum: the compiler's own on-screen strings. The default
    // multi-language build leaves them tagged with their `delvewright.ui.…` key
    // (the pack's lang files carry every language); a `--lang` bake, which ships no
    // lang files, puts the baked language's text on the component instead. The
    // campaign id is what namespaces those keys into this delve's own vocabulary,
    // so a pack another delve left applied cannot answer them.
    let chrome =
        delvewright_dsl::Chrome::for_build(plan.campaign.world.campaign_id.as_str(), language);

    // spec-0094: what the assembly judgement proved about each locked strike
    // is what the emitter writes — the plans travel, they are never re-derived.
    let asm_locks = assembly_binding
        .as_ref()
        .map(|b| b.plans.clone())
        .unwrap_or_default();
    let functions = emit_functions(
        plan,
        &chrome,
        &sentinels,
        &moves,
        &actor_moves,
        &relight.placements,
        &wave_placements,
        &lane_routes,
        edit_replay.as_ref().map_or(&[][..], |er| &er.commands),
        &edit_replay.as_ref().map_or(Vec::new(), |er| {
            er.batches.iter().filter_map(|b| b.bounds).collect()
        }),
        &trap_gates,
        &payload_plans,
        &branch_transport,
        stake_table.as_ref(),
        &asm_locks,
    );
    // `DW0852` over the FINAL function list — after every emitter has had its say,
    // so a later pass that rewrote a judge cannot slip past a check that ran
    // earlier. The ledger is UNCONDITIONAL, on the `fluid-escape.json` terms and
    // not the `gate-seal.json` ones: this file is what the staging gate reads as
    // this row's binding count, and an absent file there is reported as
    // MISSING-CHECK — "nobody ran it" — where the truth about a campaign with no
    // stealth beat is "nothing here can carry the defect". Those are different
    // facts and the gate can only tell them apart if the file exists and says
    // zero.
    let audit = audit_stealth_judges(&functions, plan.stealth_beats.len());
    if let Some((code, message)) = audit.finding() {
        return Err(BuildFailure::Diagnostic { code, message });
    }
    put_json(&mut out, "validation/stealth-judge.json", &audit.ledger());
    for (name, body) in &functions {
        insert_unique(
            &mut out,
            format!("datapack/data/{ns}/function/{name}.mcfunction"),
            body.clone().into_bytes(),
            "function",
            name,
        )?;
    }

    // dialogs
    for (name, value) in emit_dialogs(plan, &chrome) {
        insert_unique(
            &mut out,
            format!("datapack/data/{ns}/dialog/{name}.json"),
            json_bytes(&value),
            "dialog",
            &name,
        )?;
    }

    // advancements
    for (name, value) in emit_advancements(plan, &chrome, &wave_placements) {
        insert_unique(
            &mut out,
            format!("datapack/data/{ns}/advancement/{name}.json"),
            json_bytes(&value),
            "advancement",
            &name,
        )?;
    }

    // death loot tables — v0.9 declared quest-item drops only; a campaign that
    // declares none writes no `loot_table` directory (byte-identity).
    for (name, value) in emit_drop_loot_tables(plan) {
        insert_unique(
            &mut out,
            format!("datapack/data/{ns}/loot_table/{name}.json"),
            json_bytes(&value),
            "loot table",
            &name,
        )?;
    }

    // item modifiers — the bonfire rest's mend (spec-0016 §1); a campaign with no
    // bonfire emits none.
    if plan.bonfires().next().is_some() {
        insert_unique(
            &mut out,
            format!("datapack/data/{ns}/item_modifier/{BONFIRE_MEND}.json"),
            json_bytes(&bonfire_mend_modifier()),
            "item modifier",
            BONFIRE_MEND,
        )?;
    }

    // spec-0095: the stand-in's profile, filled from the player it stands for.
    if cutscene_parties(plan)
        .iter()
        .any(|(_, p)| *p == delvewright_dsl::CutsceneParty::Present)
    {
        insert_unique(
            &mut out,
            format!(
                "datapack/data/{ns}/loot_table/{}.json",
                crate::compiler::standin::STANDIN_LOOT
            ),
            json_bytes(&crate::compiler::standin::loot_table()),
            "loot table",
            crate::compiler::standin::STANDIN_LOOT,
        )?;
    }

    // predicates — currently only the sneak-held gate (see
    // SNEAK_HELD_PREDICATE) the cutscene bounce and the respawn wait's view
    // binding read; a campaign with neither emits none.
    if campaign_has_cutscene(plan.campaign) || respawn_wait(plan).is_some() {
        put_json(
            &mut out,
            &format!("datapack/data/{ns}/predicate/{SNEAK_HELD_PREDICATE}.json"),
            &sneak_held_predicate(),
        );
    }

    // ---- packtest datapack ----
    packtest::emit_packtest(
        plan,
        &mut out,
        &moves,
        &actor_moves,
        &WaveGeometry {
            placements: &wave_placements,
            lanes: &lane_routes,
            rings: &wave_rings,
        },
        &payload_plans,
        &asm_locks,
    );

    // ---- creator overlay (playtest-only; spec-0006) ----
    // A self-contained module (crate::compiler::creator). Its `.mcfunction`s are plain
    // vanilla, so they flow through the command-tree validator below and the
    // determinism gate like the main datapack; the shipped delve image excludes
    // this directory (CI-checked, same as packtest-datapack/).
    crate::compiler::creator::emit_creator(plan, &mut out, &moves, &actor_moves);

    // ---- server ----
    emit_ground_biome(plan, &mut out);
    emit_server(plan, &mut out);

    // ---- critical path ----
    let exported_routes: &[crate::compiler::nav::LegRoute] = path_legs
        .iter()
        .find(|(slug, _, _)| slug == "critical-path")
        .map(|(_, _, r)| r.as_slice())
        .unwrap_or(&[]);
    let mut cp = emit_critical_path(plan, &moves, &actor_moves, exported_routes);
    if let Some(mut b) = assembly_binding {
        if let Some(steps) = cp.get_mut("steps").and_then(Value::as_array_mut) {
            b.witnessed = crate::compiler::assembly::with_witness_steps(
                steps,
                crate::compiler::assembly::witness_steps(&b),
            );
        }
        if b.declared > 0 {
            put_json(&mut out, "validation/assembly.json", &b.to_json());
        }
    }
    put_json(&mut out, "critical-path.json", &cp);

    // ---- visual-tier render plan (spec-0003 / spec-0007) ----
    // Deterministic camera + expect-checklist shot list for the visual tier;
    // consumed by `delvec render`. Emitted before the manifest so its hash is
    // recorded there like every other output.
    //
    // `render_plan` is the only way to a render-plan value, and it takes the
    // assembled world because every camera in it owes the `DW0724` clear-eye
    // proof — every kind, not the one that needed it first. It also states the
    // proof's binding count in the artifact and hands back a warning when that
    // count is zero.
    let (render_plan_doc, camera_warnings) = crate::compiler::render_plan::render_plan(
        plan,
        prefabs,
        &pov_shots,
        &world,
        Some(&crate::compiler::view::beat::picture_base(
            plan,
            assembled,
            &relight.placements,
        )),
    )?;
    warnings.extend(camera_warnings);
    put_json(&mut out, "render-plan.json", &render_plan_doc);
    // spec-0091: the served view distance, what the build judged against it
    // (every showcase camera and cutscene shot — a refusal stopped the build
    // before here), and the cost stated to the host.
    eprintln!(
        "{}",
        crate::compiler::served::Binding {
            chunks: delvewright_dsl::viewdistance::chunks(plan.campaign),
            declared: plan.campaign.world.content.view_distance.is_some(),
            showcase_cameras: render_plan_doc["camera_eye_proof"]["showcase"]
                .as_u64()
                .unwrap_or(0) as usize,
            cutscene_shots: cutscene_shots_judged,
        }
        .line()
    );

    // ---- the checks read off the finished tree ----
    check_finished_tree(plan, tree, input_bytes, &mut out, &mut warnings)?;

    // ---- NPC-skin resource pack (spec-0009) ----
    // A campaign with skinned (mannequin) NPCs ships a deterministic resource-pack
    // zip; its SHA-1 is what a client verifies against the itzg RESOURCE_PACK_SHA1
    // env. The serving/env plumbing is the packaging task's; here we emit the zip,
    // its sha1 (in the manifest), and a SKINS.md note listing the env to set.
    // The pack also carries the `delve:art` title font (spec-0014) when the
    // campaign uses the `narrate` `art` style — baked only when needed, so a
    // non-art campaign's pack is byte-identical.
    let art = crate::compiler::atmos::uses_art(plan.campaign);
    let mut extra_assets = if art {
        crate::compiler::atmos::art_font_assets()
    } else {
        BTreeMap::new()
    };
    // i18n v2 (spec-0029 §2): the pack is also the language carrier. One
    // `assets/delvewright/lang/<mc_code>.json` per declared language plus
    // `en_us.json`, so a client that speaks one of them auto-selects it; every
    // other client — and every player who declines the pack — reads the
    // `fallback` English riding on each component.
    let lang_files = lang_assets(plan, input_bytes, language)?;
    extra_assets.extend(lang_files);
    // spec-0084: the vanilla textures this delve replaces, at the vanilla path,
    // through the one funnel every pack entry takes. Resolved by the same
    // function `delvec validate` refused with, over the same bytes — which the
    // loader made build inputs, so the manifest hashes them.
    let (textures, texture_diags) = crate::compiler::textures::resolve(plan.campaign, |p| {
        input_bytes.get(p).map(Vec::as_slice)
    });
    if let Some(d) = texture_diags.into_iter().next() {
        return Err(BuildFailure::Diagnostic {
            code: d.code,
            message: format!("world.json {}: {}", d.path, d.message),
        });
    }
    extra_assets.extend(crate::compiler::textures::pack_entries(&textures));
    // spec-0097: the sheets a model's boxes judged, over the bytes the pack
    // bakes. `delvec validate` refused any it would refuse; the ledger is the
    // count, so a judgement that stopped reaching a sheet reads as zero.
    let mut sheets = crate::compiler::skinparts::Binding::default();
    let _ = crate::compiler::skinparts::check_skins(plan.campaign, skins, &mut sheets);
    crate::compiler::skinparts::count_texture_rows(plan.campaign, &textures, &[], &mut sheets);
    if let Some((code, message)) = sheets.first_refusal.clone() {
        return Err(BuildFailure::Diagnostic { code, message });
    }
    put_json(&mut out, "validation/sheet-gate.json", &sheets.to_json());
    let resource_pack = if skins.is_empty() && extra_assets.is_empty() {
        None
    } else {
        let zip = crate::compiler::resourcepack::build_pack(skins, &extra_assets);
        let sha1 = crate::compiler::resourcepack::sha1_hex(&zip);
        let overrides = crate::compiler::textures::overrides_vanilla(extra_assets.keys());
        out.insert("resourcepack.zip".to_string(), zip);
        out.insert(
            "SKINS.md".to_string(),
            pack_note(
                &sha1,
                skins,
                plan.campaign.world.campaign_id.as_str(),
                art,
                &plan.campaign.world.content.languages,
                &textures,
                plan.campaign.world.content.require_resource_pack,
            )
            .into_bytes(),
        );
        Some((sha1, overrides))
    };

    // spec-0025 validation metadata: `branch-plan.json` (the branch set, each
    // one's flag assignment, its critical path and the dialogue choices that
    // enter it — what the harness scripts its per-branch runs from) and one
    // `branch-chronicle-<branch>.md` per branch for the generation-time
    // narrative review. Both are pure functions of the campaign document, so
    // they are byte-identical across builds (ADR-0006), and both are EMPTY for a
    // campaign that declares no branch points — nothing moves for anybody who
    // has not opted in. Emitted before the manifest so its hashes cover them,
    // exactly like `critical-path-waypoints.json`.
    out.extend(crate::compiler::branch::artifacts(plan.campaign));
    // ...and, for the harness tier, one EXECUTABLE path per reachable branch:
    // `validation/branch-path-<branch>.json`, in the same `critical-path.json`
    // contract the bot has always consumed. The plan artifact above says WHICH
    // branches exist and how a player enters them; these say what the bot walks.
    // A branch's scripted dialogue choices ride inside its own `talk-to` steps
    // (each carries the `/trigger` line of the option belonging to that branch),
    // which is the only player-legal way to actuate a server-driven dialog button.
    for (slug, path) in branch_paths(plan, &moves, &actor_moves, &branch_takes, &path_legs)? {
        put_json(
            &mut out,
            &format!("validation/branch-path-{slug}.json"),
            &path,
        );
    }
    // ...and each reachable branch's own waypoint artifact, derived
    // in the world block above from the same assembled model its per-branch
    // DW0311 proof ran over. The harness derives the name from the branch's
    // `branch-path-<slug>.json`, so the two files are one contract.
    for (slug, wp) in &branch_waypoints {
        put_json(
            &mut out,
            &format!("validation/branch-waypoints-{slug}.json"),
            wp,
        );
    }
    // The traversal proof's binding ledger (`compiler::traversal`,
    // playtest-methodology.md rule 1): how many legs and route cells it examined,
    // per locomotion class, and which of its rules bind at all. A green that
    // matched nothing must be legible as such WITHOUT the reader re-deriving it
    // from an empty diagnostics list — and the capability axis is its own way to
    // bind to nothing, since every class that carries an exemption is a class
    // some rule does not examine. `gate_use.cells` counts every non-gate-opening
    // body regardless of class, so the count itself shows that rule is total.
    // The placement gate's binding ledger (`DW0864`, hv-10): what every
    // `scatter`/`plant` verb declared and what it delivered, over the DECLARED
    // domain rather than over the part of it that turned out to be usable. The
    // rows are emitted whether or not any of them is short, because the finding
    // this closes was a class nobody had a number for at all — the acceptance
    // proxy was a rendered shot, and one bearing cannot see a region.
    if let Some(er) = &edit_replay {
        put_json(
            &mut out,
            "validation/placement-gate.json",
            &crate::compiler::edit::placement_gate_json(&er.placements),
        );
    }
    if let Some(gate) = &traversal_gate {
        put_json(&mut out, "validation/traversal-gate.json", &gate.to_json());
    }
    // The piece-mating binding ledger (`DW0780`/`DW0781`): how many placed
    // pieces touch, in how many pairs, how many of those pairs a declared face
    // crosses, and which of the two declarations each piece was judged by. It
    // is emitted on every build, zeroes included, and `examined` is the count
    // over the PLACEMENT rather than over the declarations — a ledger keyed on
    // declarations reads a library that declares nothing as an honest zero,
    // which is the sentence this check printed while passing.
    put_json(
        &mut out,
        "validation/piece-mating.json",
        &plan.face_binding.to_json(
            crate::compiler::faces::placed_pieces(&plan.areas),
            plan.campaign.site_plan.is_some(),
        ),
    );
    // The fluid-escape binding ledger (`DW0318`): the horizon the verdict was
    // stated against, the pieces and fluid cells examined, and how many cells
    // ended up outside the built volume. `None` only for a campaign that
    // assembles no world at all.
    if let Some(ledger) = &fluid_escape_ledger {
        put_json(&mut out, "validation/fluid-escape.json", ledger);
    }
    // The sea-seepage binding ledger (`DW0851`): the horizon, the open contact
    // face the built volume presents to the ambient sea, how far the sea reaches
    // inside it, and how much of the walk region it submerges or wades. `None`
    // only for a campaign that assembles no world at all.
    if let Some(ledger) = &sea_seepage_ledger {
        put_json(&mut out, "validation/sea-seepage.json", ledger);
    }
    // The piece-exposure binding ledger (`DW0885`): the horizon, the solid
    // boundary the placed pieces carry, how much of it nothing stands in front
    // of, how much of that the party's own air reaches, and how many
    // `shown_faces` declarations were read and bound. `None` only for a campaign
    // that assembles no world at all.
    if let Some(ledger) = &piece_exposure_ledger {
        put_json(&mut out, "validation/piece-exposure.json", ledger);
    }
    if let Some(ledger) = &gate_seal_ledger {
        put_json(&mut out, "validation/gate-seal.json", ledger);
    }
    // **What the derivation massed** (spec-0098 §8): every place this world
    // stands a stand-in in, by name, beside the binding line that counts them.
    // A stand-in never ships — the staging gate reads this, the one event
    // between a build and a player, and refuses any place named here.
    if let Some(b) = &plan.blockout {
        put_json(
            &mut out,
            "validation/blockout.json",
            &serde_json::json!({
                "line": b.binding.line(),
                "boxes": b.binding.boxes,
                "detailed": b.binding.detailed,
                "massed": b.massed,
            }),
        );
    }
    // The way gate's binding ledger (`compiler::ways`, spec-0042 AC11,
    // playtest-methodology.md rule 1): every contingent way the placed world
    // stages, what opens it and at which quest-DAG point — or that nothing does,
    // with the cell count standing behind it — plus how many required elements
    // the reachability half examined. A campaign whose world stages no way emits
    // no file, so a file that exists and reports zero ways is a finding rather
    // than an absence.
    if let Some(gate) = &plan.way_gate {
        put_json(&mut out, "validation/ways.json", &gate.to_json());
    }
    // The lethal-volume proofs' binding ledger (`compiler::lethal`,
    // playtest-methodology.md rule 1): how many volumes were declared, how many
    // resolved to a box on the solved layout, how many world cells they close, and
    // how many respawn seats and critical-path legs were tested against them. A
    // campaign that declares no volume emits no file, so a file that exists and
    // reports zero is a finding rather than an absence.
    if teleport_gate.declared > 0 {
        // One template per distinct teleport (`emit_teleport_packtests` dedupes by
        // the same content key `teleport_fns` does), counted from the emission
        // rather than from the declaration, so the ledger reports what was really
        // generated.
        let mut gate = teleport_gate;
        gate.packtests = teleport_fns(plan).len();
        gate.legs_carried = legs_carried;
        put_json(&mut out, "validation/teleport-gate.json", &gate.to_json());
    }
    if let Some(gate) = &lethal_gate {
        put_json(&mut out, "validation/lethal-gate.json", &gate.to_json());
    }
    if let Some(gate) = &loop_gate {
        put_json(&mut out, "validation/loop-gate.json", &gate.to_json());
    }
    if let Some(gate) = firework_gate.as_ref().filter(|g| g.declared > 0) {
        put_json(&mut out, "validation/firework-gate.json", &gate.to_json());
    }
    if let Some(gate) = lightning_gate.as_ref().filter(|g| g.declared > 0) {
        put_json(&mut out, "validation/lightning-gate.json", &gate.to_json());
    }
    // The recovery stake's binding ledger (`compiler::stake`, spec-0032 AC10): how
    // many stakes were declared, how many respawn seats and death regions the
    // placement table is keyed on, how many quest states its reachability was
    // intersected over, and how many rows it proved. Same rule as above — a
    // campaign that declares no stake emits no file, so a file reporting a zero
    // binding is a finding rather than an absence.
    if let Some(t) = &stake_table {
        put_json(&mut out, "validation/stake-gate.json", &t.gate.to_json());
    }
    // The bot tier's death contract (`compiler::deathplan`): what the campaign
    // PROMISES a death does, so the mineflayer tier can assert it against a real
    // client that really died. Same rule again — a campaign that declares no
    // volume, no `on_death` and no stake emits no file at all.
    if let Some(dp) = &death_plan {
        put_json(&mut out, "validation/death-plan.json", dp);
    }
    // spec-0080 §5.2: what each repaint must tell a connected client — the
    // chunks of its painted 4-cell box, and the completion marker of the bundle
    // that fires it — so the bot tier asserts the `chunk_biomes` packets and
    // the absent reload. A campaign with no `set-atmosphere` emits no file.
    if let Some(rp) = atmosphere_repaint_plan(plan) {
        put_json(&mut out, "validation/atmosphere-repaints.json", &rp);
    }
    // spec-0080 §5.3: which biome each carried place stands in at the first
    // tick, as the biome map states it, so a reader that draws a scene inside
    // a place (`delvec view palette --build … --place …`) tints it under that
    // place's own biome rather than a guess. Emitted only when a place carries
    // an atmosphere.
    {
        let map = crate::compiler::horizon::biome_map(plan);
        let places: Vec<serde_json::Value> = map
            .places()
            .filter_map(|p| match &p.source {
                crate::compiler::horizon::PaintSource::Place { place, atmosphere } => Some(json!({
                    "place": place,
                    "atmosphere": atmosphere,
                    "biome": p.biome,
                    "precipitates": p.precipitates,
                    "cells": [p.cells.0, p.cells.1],
                })),
                crate::compiler::horizon::PaintSource::Band => None,
            })
            .collect();
        if !places.is_empty() {
            put_json(
                &mut out,
                "validation/biome-map.json",
                &json!({ "ground": map.ground.id, "places": places }),
            );
        }
    }
    // **The design gate's ledger** (`crate::compiler::design`, spec-0061 §6):
    // how many approved reference images the record holds, how many image files
    // stand under `design/`, which skies the rows state and which skies this
    // world can reach. Written on EVERY build, unlike its neighbours above: a
    // campaign with no approved design is exactly the state the staging gate
    // refuses, so `references: 0` is a number that has to be readable, and an
    // absent file would say *I could not look* — a different fact, and one the
    // gate reds as format rot rather than as a missing design.
    {
        let (_, binding, findings) =
            crate::compiler::design::check(plan.campaign, &plan.design_files);
        put_json(
            &mut out,
            "validation/design-record.json",
            &crate::compiler::design::record(&binding, &findings),
        );
    }

    // ---- manifest (hashes of inputs + all other outputs) ----
    let manifest = emit_manifest(plan, input_bytes, &out, language, resource_pack.as_ref());
    put_json(&mut out, "manifest.json", &manifest);

    // ---- untranslated-literal scan (DW0185, spec-0029) ----
    // Feature-blind and last, exactly like the call-graph integrity check above:
    // every authored player-visible string entered this build carrying its l10n
    // key (`dsl::tag_translatables`), and an emitter either lowered it into a text
    // component (`tr` / `snbt_component`) or read it as a named exclusion
    // (`plain`). A tag still present in the finished tree is a site that did
    // neither — a string that would ship as an untranslatable literal. This is the
    // whole reason spec-0029's risk is an invariant rather than an audit.
    check_untranslated_literals(&out, &extra_assets)?;

    warnings.extend(pacing);
    Ok((out, warnings))
}

/// Re-validate every emitted vanilla `.mcfunction` in a built tree (used by
/// tests). PackTest functions are excluded — see [`is_vanilla_function`].
pub fn validate_emitted(out: &BuildOutput, tree: &CommandTree) -> Vec<CommandError> {
    let mut errors = Vec::new();
    for (path, bytes) in out {
        if is_vanilla_function(path)
            && let Ok(body) = std::str::from_utf8(bytes)
        {
            errors.extend(tree.validate_function(body));
        }
    }
    errors
}

/// A `.mcfunction` that must pass the vanilla 1.21.11 command-tree validator.
/// The `packtest-datapack/` suite uses PackTest-only commands (`assert`, …) and
/// runs on the modded validation server, so it is exempt (spec-0003/ADR-0003:
/// mods are tooling-only, never the player-facing datapack).
fn is_vanilla_function(path: &str) -> bool {
    path.ends_with(".mcfunction") && !path.starts_with("packtest-datapack/")
}

delvewright_dsl::dw_code! {
    /// `DW0185`: an authored player-visible string reached the built tree **outside**
    /// a text component — its translation tag ([`delvewright_dsl::TR_SIGIL`]) is still
    /// in the emitted bytes. Either the emitter must lower it through [`tr`] /
    /// [`snbt_component`] (so a client renders the player's own language), or, if the
    /// site is genuinely not player-facing and not a component (a manifest field, a
    /// reviewer chronicle, the bot's `critical-path.json`, a generated PackTest
    /// source), it must read the string through `dsl::l10n::plain` and be listed in
    /// `docs/reference/compiler.md`'s named-exclusion table.
    ///
    /// Build-tier and feature-blind: it reads the finished tree, so it guards every
    /// emitter, including ones not yet written. This is the invariant that replaces
    /// "we enumerated every emission site once" with "the compiler re-proves it on
    /// every build" (spec-0029 Risks).
    pub const DW_UNTRANSLATED_LITERAL: DwCode = DwCode::new("DW0185", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0362`: a dialogue node declares more conditionally-visible options than the
    /// variant-dialog encoding can carry. Validation-tier content-shape limit.
    pub const DW_DIALOGUE_VARIANT_CAP: DwCode = DwCode::new("DW0362", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0361`: two distinct generated artifacts sanitize to the same name, so one
    /// would silently overwrite the other in the emitted pack.
    pub const DW_NAME_COLLISION: DwCode = DwCode::new("DW0361", ExitTier::Build);
}

/// Insert an emitted artifact, refusing to let one silently overwrite another
/// (`DW0361`).
///
/// Generated names are built by underscore-joining [`plan::safe_local`] outputs,
/// and `safe_local` is doubly lossy: it drops the `<kind>/` prefix and maps `-`,
/// `/` and `.` all to `_`. So a wave `wave/npc-x` and an NPC `npc/x` both name
/// `spawn_npc_x`, and `move-npc npc/guard-a → anchor/post` collides with
/// `npc/guard → anchor/a-post` (which also aliases their tick counters and
/// re-entry sentinels — two live movement drivers sharing one score). The output
/// map is a `BTreeMap`, so the loser used to vanish without a word: the wave simply
/// never spawned.
///
/// Re-emitting the **same bytes** under the same name is fine and expected (the
/// emitters dedup by content key, and several are called per-consumer), so only a
/// genuine divergence fails the build.
fn insert_unique(
    out: &mut BuildOutput,
    path: String,
    bytes: Vec<u8>,
    kind: &str,
    name: &str,
) -> Result<(), BuildFailure> {
    if let Some(existing) = out.get(&path)
        && existing != &bytes
    {
        return Err(BuildFailure::Diagnostic {
            code: DW_NAME_COLLISION,
            message: format!(
                "two different generated {kind}s both sanitize to `{name}` — one would \
                 silently overwrite the other at `{path}`. Generated names drop an id's \
                 `<kind>/` prefix and fold `-`, `/` and `.` into `_`, so ids that look \
                 distinct can collide (e.g. wave `wave/npc-x` with npc `npc/x`, or \
                 `move-npc npc/guard-a → anchor/post` with `npc/guard → anchor/a-post`). \
                 Rename one of the colliding ids so their sanitized local parts differ."
            ),
        });
    }
    out.insert(path, bytes);
    Ok(())
}

delvewright_dsl::dw_code! {
    /// `DW0360`: an anchor-bearing quest/trigger effect names an anchor that resolves
    /// to no world position in the assembled build. Validation-tier content mistake
    /// (a typo'd or unassembled anchor), reported as a build diagnostic because only
    /// the assembled world knows which anchors actually exist.
    pub const DW_EFFECT_ANCHOR_UNRESOLVED: DwCode = DwCode::new("DW0360", ExitTier::Build);
}

/// Whether [`build`] assembles the voxel world — and therefore whether every
/// proof that needs it actually runs, including [`plan_payload_verbs`] and its
/// `DW0447`.
///
/// Extracted so the world block and [`check_effect_anchors`] read **one**
/// predicate. A check that defers to another check must know whether that other
/// check runs at all, and a second hand-copied answer to "does this campaign
/// assemble a world" would be exactly the drift this task exists to end, one
/// question further out.
fn assembles_world(plan: &Plan) -> bool {
    crate::compiler::nav::needs_world(plan)
        || !plan.campaign.quests.content.waves.is_empty()
        || crate::compiler::clearance::has_bodies(plan)
        // spec-0068: a firework's roof proof is a question about blocks, so a
        // campaign whose only reason to assemble the world is a rocket still
        // assembles it — otherwise `DW0899` would be declared, compiled and
        // never asked of exactly the campaign that needs it most.
        || crate::compiler::firework::declares_one(plan)
        // spec-0092: the struck block and the reach are questions about the
        // assembled world, for the firework's reason.
        || crate::compiler::lightning::declares_one(plan)
}

delvewright_dsl::dw_code! {
    /// `DW0852`: **a stealth judge asks a player for something other than where they
    /// are.**
    ///
    /// A stealth beat is hiding, and hiding is a place. `emit_stealth_functions` has
    /// promised that since v0.6 — *"zone presence alone = hidden"*, in its own doc
    /// comment — because the alternative collides with the spectator cutscene camera
    /// and, more to the point, because a beat that demands a posture is a beat that
    /// demands something the fiction never asked for. A playtester met that as a
    /// stealth scene that quietly required crouching, which nothing in the story had
    /// said.
    ///
    /// A promise in a doc comment is a doc line, and this project's own doctrine says
    /// a doc line is not an invocation. This is the invocation: it reads the FINAL
    /// emitted function list — not one emitter's return value, so a later pass that
    /// rewrote a judge would not slip past — and holds every per-player test to
    /// **position arguments and nothing else**.
    ///
    /// ## Why an allowlist, and why it is scoped to the eval function
    ///
    /// The rule is stated as *which selector arguments may appear* (`x`/`dx`/`y`/`dy`
    /// /`z`/`dz`), never as a list of forbidden ones. A denylist of `nbt`, `predicate`
    /// and friends is the wrong question asked correctly: the next demand on a player
    /// will be spelled some way nobody has thought of, and a denylist answers "not one
    /// of the six I knew about" with an honest no.
    ///
    /// It examines `stealth_eval_*`, the per-player judge, and deliberately not
    /// `stealth_tick_*`, whose `@a[tag=!dw_cutscene]` is non-positional and correct:
    /// skipping a player watching a cinematic is the grace clock being frozen, not a
    /// demand made of that player. The distinction is between *who is judged* and
    /// *what the judgement asks for*, and only the second is this rule's business.
    ///
    /// ## What stops it going quiet
    ///
    /// The count of judges it found must equal the count of beats the plan holds. A
    /// rename that made the judge functions invisible to this scan would otherwise
    /// examine zero, find nothing, and pass — the truncated-input vacuity mode, where
    /// the number is neither zero nor wrong but is about a smaller world than the one
    /// the check claims to cover.
    pub const DW_STEALTH_JUDGE_NOT_POSITIONAL: DwCode = DwCode::new("DW0852", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0363`: a trap declares a flag gate (`requires_flags` / `forbids_flags`) but
    /// its trigger hardware cannot be removed and put back exactly as authored, so the
    /// compiler refuses to pretend the gate works.
    pub const DW_TRAP_GATE_UNSUPPORTED: DwCode = DwCode::new("DW0363", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0447`: a trap-payload verb centres its volume on an anchor no placed
    /// prefab piece provides, so the kill zone / collapse region cannot be resolved.
    pub const DW_PAYLOAD_ANCHOR_UNRESOLVED: DwCode = DwCode::new("DW0447", ExitTier::Build);
}
