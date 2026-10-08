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
mod quest;
mod removal;
mod respawn;
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
use quest::*;
use removal::*;
use respawn::*;
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
        crate::compiler::loot::check_loot_containers(blocks, &plan.loot, &available).map_err(
            |e| BuildFailure::Diagnostic {
                code: e.code,
                message: e.message,
            },
        )?;
        crate::compiler::loot::check_collect_containers(blocks, &plan.collect_fills, &available)
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
    // measured darkness. Runs before nav verification so the colliding fixtures it
    // adds are re-verified for walkability below. A `DW0210`/`DW0211` diagnostic
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
    // either needs it. Includes any colliding relight fixtures (campfire / floor
    // lantern) so a fixture can never wedge a required path shut *nor* be stood on
    // by a spawned mob (spec-0010: verification re-runs after placement).
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
    let world = geometry
        .with_premises(crate::compiler::nav::Premises::of_plan(
            plan,
            assembled.gate_seals.clone(),
        ))
        .with_extra_solid(&relight.extra_solid);

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
    let assembly_binding: Option<crate::compiler::assembly::AssemblyBinding>;
    //
    // Asked over the world the other proofs read, before any route is derived:
    // a hitbox, its reach and where a blow lands are facts about cells, and
    // nothing below changes them. The binding and the cost the host meets are
    // printed on every build, zeroes included, before the verdict is taken.
    {
        let entry = campaign_spawn(plan);
        let open = world.without_exclusions();
        let population = crate::compiler::lethal::walked_population(plan, &open, entry);
        let roots = crate::compiler::lethal::stands_at_roots(plan, entry);
        let returned = playable_region(plan).map(|r| (r.min, r.max));
        // Where the party can walk while each performed trigger is the next
        // beat — `DW0924`'s reading: a gate a later beat opens is shut. A
        // trigger step proves no objective, so the configuration is the one
        // the next objective step stands under: the strike is made on the way
        // to it, after every beat before it.
        let reaches = |trigger: &str, lo: [f64; 3], hi: [f64; 3]| {
            let step = plan
                .critical_path
                .iter()
                .position(
                    |s| matches!(s, Step::Trigger { trigger_id, .. } if trigger_id == trigger),
                )
                .map(|t| {
                    (t..plan.critical_path.len())
                        .find(|&i| plan.critical_path[i].objective().is_some())
                        .unwrap_or(t)
                });
            let config = step.and_then(|s| crate::compiler::nav::world_while_next(plan, &world, s));
            let ground = config.as_ref().unwrap_or(&world);
            ground
                .reachable_walkable(&roots)
                .into_iter()
                .filter(|p| !crate::compiler::nav::returned_from(returned, *p))
                .any(|p| crate::compiler::strand::eye_reaches_box(ground, p, lo, hi))
        };
        let (binding, findings) = crate::compiler::assembly::check(plan, &population, &reaches);
        assembly_binding = Some(binding.clone());
        eprintln!("{}", binding.line());
        eprintln!("{}", binding.cost_line());
        if let Some((first, rest)) = findings.split_first() {
            for extra in rest {
                eprintln!("{} [error] build: {}", extra.code, extra.message);
            }
            return Err(BuildFailure::Diagnostic {
                code: first.code,
                message: first.message.clone(),
            });
        }
    }

    // ---- the stage-5 blockout battery (spec-0049 §5.3) ----
    //
    // **Bound here, and here is the only door.** This is the one function that
    // turns a `Plan` into a datapack, so a site-plan world cannot be built,
    // packaged or shipped without being judged against the plan it was derived
    // from. There is no flag, no subcommand and no line in a document to
    // remember; a campaign with no site plan gets `None` and nothing runs, which
    // is why every other campaign's output is byte-identical.
    //
    // It runs over the world model above — the same occupancy every other proof
    // in this function is taken under, edits and relight included — because a
    // battery that re-derived its own world would be judging a world nobody
    // ships. `DW0836`/`DW0837`/`DW0838` refuse (exit 3); `DW0821` and `DW0822`
    // are advisories that travel to the walk sheet, and the binding line is
    // stated whether anything was found or not.
    {
        let blocks = &assembled.blocks;
        // What the DERIVATION bound to, beside what its observer did. Printing
        // only the battery's line stated what was examined and never what was
        // built — and at stage 6 the difference is the whole reading: `detailed`
        // is how much of this map is a building and how much is still massing.
        if let Some(b) = &plan.blockout {
            eprintln!("{}", b.binding.line());
        }
        // What the ocean-datum invariant (`DW0344`) examined: how many placed
        // pieces declare a waterline, how many stand in the sea, and how many of
        // those were held to it. Printed on every build, ocean or not, because a
        // check that only speaks when it finds something cannot be told from one
        // that never ran.
        eprintln!("{}", plan.waterline.line());
        // What the horizon built, and — the half that matters — which authority
        // stated the rectangle it built around. A surround that ringed the
        // placed footprint and one that ringed the declared region look
        // identical from outside, right up until somebody details a place and
        // the mountains move. So the line names the authority, and the zeros are
        // findings rather than silences.
        if let Some(surround) = &plan.surround {
            eprintln!("{}", surround.binding.line());
            if surround.binding.floor_cells == 0 {
                eprintln!(
                    "surround binding 0: the horizon built terrain but no standable gap-floor \
                     cell, so the un-climbability proof flooded from nowhere and its green means \
                     nothing. A surround with no floor is a wall around a hole."
                );
            }
        }
        if let Some(battery) = crate::compiler::blockout::check(plan, blocks) {
            eprintln!("{}", battery.binding.line());
            let refusals: Vec<&(delvewright_dsl::DwCode, delvewright_dsl::Diagnostic)> =
                battery.refusals().collect();
            if let Some(((code, refusal), rest)) = refusals.split_first() {
                // **Every rule that saw this defect, not only the one that stops
                // the build.** The failure channel carries one code and one
                // message (`BuildFailure`), which is the compiler's contract and
                // does not move; what used to be lost is that one derivation
                // defect is routinely seen by two of these rules, and a report
                // naming only the first sends a creator round the loop twice.
                // The line states the whole refusal set, and the ones after the
                // first print their own messages here — the failing one is
                // printed by the caller through the ordinary diagnostic channel,
                // so nothing is said twice.
                let mut per_code: BTreeMap<String, usize> = BTreeMap::new();
                for (c, _) in &refusals {
                    *per_code.entry(c.to_string()).or_default() += 1;
                }
                eprintln!(
                    "blockout battery: {} refusal(s) — {}; the build stops at the first.",
                    refusals.len(),
                    per_code
                        .iter()
                        .map(|(c, n)| format!("{c} ×{n}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                // The same cap `BuildFailure::Validation` prints under, and for
                // the same reason: the set is what a reader needs, the whole
                // list is what a terminal loses. The count above is never
                // capped, so the cap cannot hide how much was found.
                const LISTED: usize = 20;
                for (c, d) in rest.iter().take(LISTED) {
                    eprintln!("{c} [error] {}: {}", d.path, d.message);
                }
                if rest.len() > LISTED {
                    eprintln!(
                        "  … and {} further refusal(s), not listed",
                        rest.len() - LISTED
                    );
                }
                return Err(BuildFailure::Diagnostic {
                    code: *code,
                    message: refusal.message.clone(),
                });
            }
            warnings.extend(battery.advisories());
        }
    }

    // Visual-tier player-POV shots (spec-0003): first-person cameras along the
    // proven critical-path routes. Filled inside the world block below (they need
    // the routes); empty for a campaign with no walked leg, so its render plan
    // stays byte-identical.
    let mut pov_shots: Vec<crate::compiler::render_plan::PovShot> = Vec::new();
    // spec-0025 per-branch waypoint artifacts: one
    // `validation/branch-waypoints-<branch>.json` per reachable branch, filled
    // inside the world block below (they need the assembled occupancy) and
    // emitted alongside the branch paths. Empty for a campaign with no declared
    // branch points, so nothing moves for anybody who has not opted in.
    let mut branch_waypoints: Vec<(String, Value)> = Vec::new();
    // Per branch, the links its own path takes (spec-0083 §6), decided over the
    // world below and spliced again where `branch-path-<slug>.json` is written,
    // so the bot walks the steps the branch proof judged.
    let mut branch_takes: BTreeMap<String, plan::LinkTakes> = BTreeMap::new();
    // Legs the default path crosses by a link — `teleport-gate.json`'s
    // `legs_carried`.
    let mut legs_carried = 0usize;
    // Every exported path's steps and proven routes — the exported path and each
    // reachable branch's — kept for the run-back finder, which needs the seated
    // hostiles and the lanes that are only resolved further down.
    let mut path_legs: Vec<(String, Vec<plan::Step>, Vec<crate::compiler::nav::LegRoute>)> =
        Vec::new();
    // The traversal proof's binding ledger (`compiler::traversal`), filled inside
    // the world block below. `None` for a campaign that assembles no world —
    // which is not the same fact as "examined nothing", so the artifact is
    // omitted entirely rather than emitted claiming a zero it never measured.
    let mut traversal_gate: Option<crate::compiler::traversal::TraversalGate> = None;
    // The world-load gate ledger (`compiler::assembled`,
    // playtest-methodology.md rule 1): what the completability model measured
    // about every gate the layout resolved, and how many of them it treats as
    // shut. `None` for a campaign whose layout resolves no gate anchor, so a file
    // that exists and reports `"modelled_as_sealed": 0` is a finding rather than
    // an absence.
    let mut gate_seal_ledger: Option<serde_json::Value> = None;
    // The fluid-escape proof's binding ledger (`compiler::nav`, `DW0318`): how
    // much water the assembled world holds and how much of it ended up outside
    // every placed piece, stated against the horizon. Filled by every campaign
    // that assembles a world — including a bone-dry one, which then ships a
    // ledger reading zero rather than nothing at all.
    let mut fluid_escape_ledger: Option<serde_json::Value> = None;
    // The sea-seepage proof's binding ledger (`compiler::nav`, `DW0851`): how much
    // open face the built volume presents to the ambient sea, how far the sea gets
    // in, and how much of the walk region it covers. Filled by every campaign that
    // assembles a world — a `horizon: void` one included, which then ships a
    // ledger saying `"horizon": "void"` and zeroes, rather than nothing at all.
    let mut sea_seepage_ledger: Option<serde_json::Value> = None;
    // The piece-exposure binding ledger (`compiler::burial`, `DW0885`): how much
    // solid boundary the placed pieces put on their own boxes, how much of it
    // has nothing in front of it, and how much of THAT stands in air the party
    // can be in. Filled by every campaign that assembles a world, zeroes
    // included — a build whose party never gets outdoors ships a ledger saying
    // so, which is a different artifact from one that never ran the question.
    let mut piece_exposure_ledger: Option<serde_json::Value> = None;
    // The lethal-volume proofs' binding ledger (`compiler::lethal`), filled inside
    // the world block below. `None` for a campaign that declares no volume — no
    // ledger, no artifact, no byte moved for anybody who has not opted in.
    let mut lethal_gate: Option<crate::compiler::lethal::LethalGate> = None;
    // The loop proofs' binding ledger (`compiler::loop`, spec-0086 §8). `None`
    // for a campaign that declares no loop — no line, no artifact; a ledger that
    // exists and reports zero is a finding.
    let mut loop_gate: Option<crate::compiler::r#loop::LoopBinding> = None;
    // The firework proofs' binding ledger (`compiler::firework`, spec-0068 §5),
    // filled inside the world block below. `None` for a campaign that declares no
    // firework — no ledger, no artifact, no byte moved for anybody who has not
    // opted in; a ledger that exists and reports zero columns is a finding.
    let mut firework_gate: Option<crate::compiler::firework::FireworkGate> = None;
    // spec-0091: how many cutscene shots were judged against the served view
    // distance, for the binding line printed beside the render plan.
    let mut cutscene_shots_judged = 0usize;
    // The strike proofs' binding ledger (`compiler::lightning`, spec-0092 §5),
    // filled beside the firework's: `None` for a campaign that declares no
    // strike, so nobody who has not opted in moves a byte.
    let mut lightning_gate: Option<crate::compiler::lightning::LightningGate> = None;
    // The recovery stake's compile-time placement table (`compiler::stake`), and
    // the ledger of what its proofs looked at. `None` for a campaign that declares
    // no stake, which is the whole feature's byte-identity guarantee: no table, no
    // objectives, no functions, no artifact.
    let mut stake_table: Option<crate::compiler::stake::StakeTable> = None;
    // The bot tier's contract for DYING (`compiler::deathplan`): the lethal volumes
    // it may walk into, the wording each promises, the `on_death` consequences, the
    // stake rules and the placement table's rows. `None` for a campaign that
    // declares none of the three, and for one that assembles no world — a
    // contract nobody can walk is not the same fact as an empty one.
    let mut death_plan: Option<Value> = None;

    // Every anchor-bearing effect, at every nesting depth, must resolve to a real
    // world position or the build stops (DW0360). This runs FIRST among the
    // referential proofs deliberately: emission fails open on an unresolved anchor
    // (it emits nothing) and the geometry proofs downstream fail *loudly but
    // wrongly* — an unresolved cutscene waypoint degrades to the world origin and
    // is then reported as a camera clipping a wall (DW0308), which sends the author
    // to move a shot that was never the problem. Name the root cause instead.
    check_effect_anchors(plan)?;

    // spec-0031: a `teleport` moves EVERYTHING inside its volume, so the volume
    // may not cover an affordance the engine bound to hardware it cannot move.
    // Runs here — right after the anchor-resolution seal and before any occupancy
    // model — because it is pure box arithmetic over resolved cells, and because
    // the alternative it replaces is a runtime type-exemption list
    // (`crate::compiler::teleport` records why that would be wrong).
    let teleport_gate = crate::compiler::teleport::check_bound_affordances(plan)?;

    // A dialogue node's conditionally-visible options are encoded as 2^n precomputed
    // variants; past the cap that is a pack-size decision, and past 32 it was a
    // compiler panic (DW0362).
    check_dialogue_variant_cap(plan)?;

    // Actor spawn anchors must resolve to a world position (spec-0014); a spawn is a
    // summon, not a walk, so this needs no occupancy model. DW0325 if one dangles.
    crate::compiler::nav::check_actor_placement(plan)?;

    // …and no two bodies that are in the world at the same time may be declared
    // on the same cell (DW0896). Runs here, with the anchor-resolution seals and
    // before any occupancy model, because it is arithmetic over resolved cells
    // and over a declaration the author can read: seven actors on one anchor
    // emitted seven identical `summon` lines and the build exited 0. Its binding
    // line prints whether or not it found anything — a count only says something
    // when the run that found nothing prints it too — and prints before the
    // refusal, so a refused run still states what it examined.
    // Every mark a body is put on stays inside the piece its anchor belongs to
    // (DW0897, spec-0066): an offset says where beside a place, never which
    // place. Runs before the occupancy rules that read those cells, and prints
    // its binding line on every run, zeroes included.
    let (marks, marks_verdict) = crate::compiler::mark::check_marks_in_piece(plan);
    eprintln!("{}", marks.line());
    marks_verdict.map_err(|e| BuildFailure::Diagnostic {
        code: e.code,
        message: e.message,
    })?;

    let (one_mark, one_mark_verdict) = crate::compiler::cohabit::check_one_body_per_mark(plan);
    eprintln!("{}", one_mark.line());
    one_mark_verdict.map_err(|e| BuildFailure::Diagnostic {
        code: e.code,
        message: e.message,
    })?;

    // No body may stand on the affordance the party has to click (DW0359). Runs
    // right after the anchor-resolution seals and before any occupancy model:
    // it is pure box arithmetic over resolved cells, and it is the proof that the
    // island's giant — a 0.9 × 2.9 warden sharing `anchor/fire-pit` with two
    // interact objectives — was hiding a required beat behind its own hitbox.
    warnings.extend(
        crate::compiler::eclipse::check_body_eclipse(plan).map_err(|e| {
            BuildFailure::Diagnostic {
                code: e.code,
                message: e.message,
            }
        })?,
    );

    // …and no OTHER affordance may contest the hitboxes a sealed gate arms to
    // answer a right-click (DW0422). Same box arithmetic, same tier:
    // two interaction entities in one cell is a ray-pick tie the client resolves
    // by iteration order, so one of them silently stops receiving clicks.
    crate::compiler::eclipse::check_seal_collisions(plan).map_err(|e| {
        BuildFailure::Diagnostic {
            code: e.code,
            message: e.message,
        }
    })?;

    // A shortcut whose sealed side the geometry does not name (DW0425) is
    // refused BEFORE the affordance proof below, and that ordering is load-bearing
    // rather than tidy. A `use` trigger on such a gate cannot ride the door —
    // `pressable::body_at` only rides a shortcut whose sealed side resolved — so
    // it falls back to summoning its own box on the gate anchor, exactly where the
    // shortcut's `unlock` affordance may also stand. `DW0878` would then report a
    // hitbox tie that is a CONSEQUENCE of the undecidable side, sending the author
    // to move an anchor when what they have to fix is the door. Withhold the
    // downstream verdict; name the cause. (It also widens `DW0425` to a campaign
    // that assembles no world, which is a structural fault about the declaration
    // either way.)
    check_shortcut_sides(plan)?;

    // …and no two AFFORDANCES may stand on one cell either (DW0878). The three
    // proofs above each need one side of their pair to be something else — a
    // standing body for `DW0359`, a compiler-owned press set for `DW0422`, a cast
    // ledger entry for `DW0489` — so an `interact` objective and a `use` trigger
    // on one anchor were nobody's rule, and the engine's own gallery shipped
    // exactly that pair. Same box arithmetic, same tier, same authority
    // (`eclipse::affordances`).
    crate::compiler::eclipse::check_affordance_contests(plan).map_err(|e| {
        BuildFailure::Diagnostic {
            code: e.code,
            message: e.message,
        }
    })?;

    // …and a recovery stake's marker is a PLACE, so every stake that can leave one
    // has to agree what a place looks like (DW0880). Same family, same tier, and
    // here for the same reason: one place holds ONE `minecraft:interaction`,
    // because the placement table is keyed on (seat, region) rather than on the
    // stake and the rule's common branch positions at a runtime death point — so
    // four stakes a death drops are four coincident boxes unless the hardware
    // belongs to the place. It does; this is what keeps its one face decidable.
    crate::compiler::stake::check_marker_faces(plan.campaign).map_err(|e| {
        BuildFailure::Diagnostic {
            code: e.code,
            message: e.message,
        }
    })?;

    // …and no two bodies the party CLICKS may stand close enough that the
    // crosshair cannot tell them apart (DW0489). `DW0359` above compares a body
    // against an affordance and skips every walker; this reads the v0.7 cast
    // ledger, which states beat by beat who is on stage together, and measures
    // the pairs it names. It is the proof the island's terminal finding needed —
    // two crew NPCs declared on one cell at the cave mouth.
    warnings.extend(
        crate::compiler::crosshair::check_crosshair_contests(plan).map_err(|e| {
            BuildFailure::Diagnostic {
                code: e.code,
                message: e.message,
            }
        })?,
    );
    let (moves, actor_moves, wave_placements, wave_rings, lane_routes, payload_plans): (
        Vec<crate::compiler::nav::MovePlan>,
        Vec<crate::compiler::nav::ActorMovePlan>,
        WavePlacements,
        WaveRings,
        crate::compiler::nav::LaneRoutes,
        PayloadPlans,
    ) = if assembles_world(plan) {
        {
            // spec-0022 payload verbs need the block map (a `collapse` settles
            // real blocks), not just the occupancy view.
            let blocks: &crate::compiler::blockstate::BlockMap = &assembled.blocks;
            if world.has_gate_anchors() {
                gate_seal_ledger = Some(world.gate_seal_ledger());
            }

            // DW0322 over the FINISHED world, for every campaign that assembles
            // one.
            //
            // This proof had exactly one call site: inside the stage-7 edit
            // replay, once per batch. Stage 8 is skipped entirely for a campaign
            // with no edits — so a campaign that only places pieces never had its
            // boundary proven at all, and could ship a reachable walkable cell
            // one step from a void drop. The guarantee is a property of the
            // ASSEMBLED WORLD, not of having edited it; keying it to the edit
            // script bound it to the wrong thing.
            //
            // The per-batch call stays: it names WHICH batch broke the boundary,
            // which this one cannot. This is the floor under both — run last,
            // over the world that actually ships.
            //
            // ERROR TIER, unwindowed. A
            // reachable walkable cell one step from a bottomless column is a
            // player who leaves the world; that is not a style note the author
            // may carry for a version, so there is no deprecation window and the
            // message offers none. The per-batch call inside the edit replay is
            // an error for the same reason and stays one — it additionally names
            // WHICH batch broke the boundary, which this floor cannot.
            //
            // The fix is always GEOMETRY or the world-generator premise, never a
            // declaration: `Ambient` (spec-0013 `horizon`) states what an
            // unmodelled column contains, and `boundary`'s return clock is a
            // runtime rescue that this proof deliberately does not read — being
            // teleported back after falling out is not the guarantee.
            //
            // The walk region this proof examines is rooted at every resolved
            // anchor, SEATED INSIDE THE PIECE THAT DECLARES IT
            // (`crate::compiler::nav::AnchorRoot`). That confinement is load-bearing here
            // and was the finding this call site was born with: an unconfined
            // nearest-standable snap ignores solid geometry, so an anchor a
            // `collapse` payload must declare in the ceiling snapped UP through
            // it onto the room's ROOF — and this proof then demanded a safe edge
            // on a bare platform in a void world, which no free-standing prefab
            // can satisfy. Five `v06_trap_payloads` fixtures were red for exactly
            // that, while their twelve siblings — identical geometry, anchors one
            // block lower — were green, which is what proved the interior walk
            // region boundary-safe and the roof a disconnected component.
            // `DW0318` over the finished world: fluid that ran out of the built
            // volume. It runs BEFORE the boundary proof, and the order is
            // load-bearing. Two independent reasons, and the second is why this
            // is not a style choice:
            //
            // 1. The boundary proof cannot see this at all. It examines
            //    reachable WALKABLE cells, and a flooded cell is impassable and
            //    never floor, so escaped water is in neither the reachable set
            //    nor its neighbour scan, under either horizon.
            // 2. Worse, escaped water makes the boundary proof LIE. Its
            //    per-column fall-arrest scan (`nav::boundary_void`'s `col_min`)
            //    counts a flooded cell as arrest, so a bottomless column with a
            //    waterfall running down it reads as supported — and the proof
            //    goes quiet on precisely the columns the water escaped through.
            //    Measured on the shipped `island-beach-camp` piece placed under
            //    `horizon: void`: 9792 escaped cells, and `DW0322` silent.
            //
            // So escaping fluid is a false premise of the proof that runs next,
            // exactly as an unsettled gravity block is a false premise of
            // everything downstream of `DW0313`. Clear it first.
            //
            // The ledger is measured unconditionally and emitted below, even
            // when the verdict is a pass and even when the world holds no water
            // at all: "0 fluid cells examined" is the reading a dry campaign
            // should be able to show, and a check whose only output is silence
            // is one nobody can tell apart from a check that did not run.
            // Measured here for the LEDGER, which is owed even on a pass and
            // even by a bone-dry world. The sequencing itself is no longer this
            // call site's to hold: `verify_boundary_safety` runs the same proof
            // first, because a boundary verdict taken over a world the water is
            // still running out of is not a verdict (see its doc comment). This
            // call and that one cannot disagree — the measurement is pure and
            // reads the same two sets.
            let fluid_escape = crate::compiler::nav::measure_fluid_escape(&world);
            fluid_escape_ledger = Some(fluid_escape.ledger());
            if let Some(e) = fluid_escape.finding() {
                return Err(BuildFailure::Diagnostic {
                    code: e.code,
                    message: e.message,
                });
            }

            // Measured here for the LEDGER only, on the same terms as the fluid
            // escape above: the refusal itself lives inside
            // `verify_boundary_safety`, which owns the sequence, and this call
            // cannot disagree with it because the measurement is pure and reads
            // the same sets. A pass owes the numbers as much as a failure does —
            // `contact_face_cells: 0` is a watertight hull saying so.
            let party_walk =
                world.reachable_walkable_rooted(&crate::compiler::edit::anchor_starts(plan));
            let seepage = crate::compiler::nav::measure_sea_seepage(&world, &party_walk);
            eprintln!("{}", seepage.line());
            sea_seepage_ledger = Some(seepage.ledger());

            // **`DW0344`, second arm: no walk cell of a placed piece stands at
            // or below the sea plane.** The static half of this rule is
            // `DW0886`, asked of the library before anything is placed; this is
            // the backstop for what the documents could not know — a floor a
            // stage-7 edit script carved after placement above all. Bound here
            // because this is the first point at which the walk region and the
            // placed boxes are both in hand, and printed before it is raised,
            // for the same vacuity reason the exposure line is.
            let (sea_walk, sea_walk_findings) =
                crate::compiler::plan::check_ocean_walk_plane(plan, prefabs, &party_walk);
            eprintln!("{}", sea_walk.line());
            if let Some((first, rest)) = sea_walk_findings.split_first() {
                for extra in rest {
                    eprintln!("{} [error] build: {}", extra.code, extra.message);
                }
                return Err(BuildFailure::Diagnostic {
                    code: crate::compiler::plan::DW_OCEAN_WATERLINE,
                    message: first.message.clone(),
                });
            }

            // **`DW0885`: a piece's outside answers for itself, or the world
            // buries it.** Bound here because this is the one function that
            // turns a plan into a datapack AND the first point at which the
            // three things the question needs are all in hand: the assembled
            // blocks (what the build really writes, a valley's terrain
            // included), the placed boxes, and the walk region the party is
            // proved to stand in. `Plan::build` cannot answer it — it runs
            // before a single `.nbt` byte is read, so it knows where the boxes
            // are and nothing about which of their boundary cells are solid.
            // The binding line is printed BEFORE the refusal is raised, and the
            // ordering is the vacuity rule rather than a nicety: the run that
            // finds something owes its reader the same counts as the run that
            // finds nothing, and a check whose line appears only on a pass is one
            // whose denominator nobody can read at the moment it matters.
            let (exposure, findings) = crate::compiler::burial::check(
                plan,
                prefabs,
                blocks,
                structures,
                &world,
                &party_walk,
            );
            eprintln!("{}", exposure.line());
            piece_exposure_ledger = Some(exposure.to_json());
            if let Some((first, rest)) = findings.split_first() {
                // Every piece this world stands unanswered, not only the one
                // that stops the build. The failure channel carries one message
                // (`BuildFailure`), which is the compiler's contract and does
                // not move; what would otherwise be lost is that a placement
                // defect is routinely several pieces at once, so the rest print
                // here and the first travels as the refusal.
                for extra in rest {
                    eprintln!("{} [error] build: {}", extra.code, extra.message);
                }
                return Err(BuildFailure::Diagnostic {
                    code: crate::compiler::burial::DW_PIECE_EXPOSED,
                    message: first.message.clone(),
                });
            }

            // **The surround bounds the map, proven rather than promised**
            // (`DW0854`). The generator guarantees that no surround column
            // stands exactly one block above the gap-floor datum, so the
            // floor's own walkable component is bounded above by the datum and
            // the first thing outward of it is a two-block riser, which
            // vanilla's auto-step and jump cannot take. That is an argument
            // about the generator; this is a measurement of the world.
            //
            // It has to be a second measurement, because a great deal happens
            // between the two: gravity settles, a stage-7 edit script may carve,
            // a palette may change, and any of those can put a riser on the
            // slope the generator never wrote. So the proof floods the SAME nav
            // model every route proof uses, from the surround's own gap-floor
            // cells, and asks whether anything it reaches lies outward of the
            // crest line — sharing none of the generator's arithmetic, which is
            // what makes it an observer rather than a restatement.
            //
            // Its binding is on the plan (`SurroundBinding::floor_cells`) and is
            // printed with every surround build, because a flood that started
            // from nowhere passes for free and looks exactly like this one.
            if let Some(surround) = &plan.surround
                && let Err(cell) = surround.valley.verify_unclimbable(&world)
            {
                {
                    return Err(BuildFailure::Diagnostic {
                        code: crate::compiler::surround::DW_VALLEY_CLIMB,
                        message: format!(
                            "the surround's inner slope has grown a standable staircase: a walk \
                             starting on the gap floor stands at [{}, {}, {}], outward of the \
                             crest line, so the landform no longer bounds the map. The generator \
                             leaves nothing standing one block over the gap floor and keeps trees \
                             off the inner wall, so a step up from the floor was put back by \
                             something later — an edit batch, a gravity settle, or a palette \
                             whose blocks are a different height. It flooded from {} standable \
                             gap-floor cells",
                            cell[0], cell[1], cell[2], surround.binding.floor_cells,
                        ),
                    });
                }
            }

            crate::compiler::nav::verify_boundary_safety(
                &world,
                &crate::compiler::edit::anchor_starts(plan),
            )
            .map_err(|e| BuildFailure::Diagnostic {
                code: e.code,
                message: e.message,
            })?;

            // spec-0092 §10: the boundary and the world agree (`DW0960`) — every
            // place a body is put stands inside a region that returns, and a
            // region that does not return encloses a world nobody can leave. The
            // walk region is computed only for the second shape, which reads it.
            {
                let region = playable_region_box(plan);
                let returns = boundary_returns(plan);
                let starts = crate::compiler::edit::anchor_starts(plan);
                let (reachable, sea_entry) = if region.is_some() && !returns {
                    (
                        world.reachable_walkable_rooted(&starts),
                        crate::compiler::nav::open_sea_entry(&world, &starts),
                    )
                } else {
                    (BTreeSet::new(), None)
                };
                let (gate, findings) = crate::compiler::bound::judge(
                    &crate::compiler::bound::places(plan),
                    region,
                    returns,
                    &reachable,
                    sea_entry,
                );
                eprintln!("{}", gate.line());
                if let Some(first) = findings.first() {
                    return Err(BuildFailure::Diagnostic {
                        code: first.code,
                        message: first.message.clone(),
                    });
                }
            }

            // spec-0092 §10: a link whose root plays a cutscene before the carry
            // takes everyone or no one — `cs_end` puts every player on the cell the
            // presser stood on — so a press from a cell outside its volume strands
            // the whole party (`DW0932`, the fault "pressed from outside its
            // volume"). The route proof stands one chosen cell inside the volume;
            // this asks every cell a press reaches from.
            {
                let gathered: Vec<&crate::compiler::link::LinkPlan> = plan
                    .links
                    .iter()
                    .filter(|l| l.gathered_by.is_some())
                    .collect();
                let reachable = if gathered.is_empty() {
                    BTreeSet::new()
                } else {
                    world.reachable_walkable_rooted(&crate::compiler::edit::anchor_starts(plan))
                };
                let mut outside_cells = 0usize;
                for l in &gathered {
                    let cells = crate::compiler::nav::press_cells_outside(&world, l, &reachable);
                    outside_cells += cells.len();
                    if let Some(first) = cells.first() {
                        let listed = cells
                            .iter()
                            .take(6)
                            .map(|c| format!("{c:?}"))
                            .collect::<Vec<_>>()
                            .join(", ");
                        return Err(BuildFailure::Diagnostic {
                            code: crate::compiler::plan::DW_TELEPORT_LINK,
                            message: format!(
                                "the link `{t}` (`{p}`) carries {b} after the cutscene at `{cs}` \
                                 ends, and `cs_end` puts every player on the cell the presser \
                                 stood on — so the carry takes everyone or no one. {n} cell(s) a \
                                 body can walk to perform the trigger from outside its volume: \
                                 {listed}{more}. Pressed from {first:?}, the whole party is put \
                                 down there and nobody is carried. Fault: pressed from outside its \
                                 volume. Remedy: widen the volume over every cell the press \
                                 reaches from, or move the trigger's body where it can be reached \
                                 only from inside the volume — its own cell included, which the \
                                 volume may not cover (`DW0542`), so the body stands where no \
                                 body can stand: over a rail, a post or open water.",
                                t = l.trigger_id,
                                p = l.path,
                                b = l.box_words(),
                                cs = l.gathered_by.as_deref().unwrap_or(""),
                                n = cells.len(),
                                more = if cells.len() > 6 { ", …" } else { "" },
                            ),
                        });
                    }
                }
                eprintln!(
                    "link gathering binding: {} link(s) carried after their root's cutscene, {} \
                     press cell(s) outside their volumes over {} walk cell(s)",
                    gathered.len(),
                    outside_cells,
                    reachable.len()
                );
            }

            // Seat each wave mob on a validated standable cell near its anchor, in
            // room only (DW0312 if the room lacks the footing) — or, for a
            // `summon: aggro-edge` wave, on its perception ring (DW0387).
            //
            // **Before the proofs, because a wave's seats are posted places.** A
            // seated mob is put where it is by declaration and not by walking,
            // exactly like an NPC's anchor, so `lethal::check_respawn_seats` has
            // to be able to see it — and the cells are a measurement of the
            // assembled world (the footing the room really offers), which no
            // reading of the plan alone can produce. Everything below is a
            // consumer of this answer; nothing it needs is computed below it.
            let (waves, rings) = plan_wave_spawns(plan, &world)?;

            // **spec-0068: a firework bursts in open air, clear of every posted
            // body** (`DW0899`). Asked here, immediately after the seating pass,
            // because a wave's seats ARE posted places and the reach rule reads
            // `DW0511`'s own enumeration — so the proof has to run where that
            // enumeration is complete. It reads the plan, the assembled blocks
            // and the seating, never a route, so nothing below it is lost by
            // asking it first.
            //
            // The line is printed whether or not it found anything, and before
            // the verdict is taken: a refusal owes its reader the same
            // denominators a pass does.
            {
                let (binding, findings) =
                    crate::compiler::firework::check(plan, blocks, campaign_spawn(plan), &waves);
                eprintln!("{}", binding.line());
                firework_gate = Some(binding);
                if let Some((first, rest)) = findings.split_first() {
                    for extra in rest {
                        eprintln!("{} [error] build: {}", extra.code, extra.message);
                    }
                    return Err(BuildFailure::Diagnostic {
                        code: first.code,
                        message: first.message.clone(),
                    });
                }
            }
            // **spec-0092: a lightning bolt strikes clear of every posted body and
            // every block it would rewrite** (`DW0958`, `DW0959`). Asked where the
            // firework is, for the firework's reason: its reach rule reads
            // `DW0511`'s enumeration, which is complete only once the seating has
            // run. The line prints before the verdict, zeroes included.
            {
                let (binding, findings) =
                    crate::compiler::lightning::check(plan, blocks, campaign_spawn(plan), &waves);
                eprintln!("{}", binding.line());
                lightning_gate = Some(binding);
                if let Some((first, rest)) = findings.split_first() {
                    for extra in rest {
                        eprintln!("{} [error] build: {}", extra.code, extra.message);
                    }
                    return Err(BuildFailure::Diagnostic {
                        code: first.code,
                        message: first.message.clone(),
                    });
                }
            }
            let (moves, actor_moves) = if crate::compiler::nav::needs_world(plan) {
                let m = crate::compiler::nav::plan_moves(plan, &world)?;
                // move-actor (spec-0014): A* over the actor's footprint; DW0325 if
                // unroutable. Planned alongside move-npc from the same occupancy model.
                let am = crate::compiler::nav::plan_actor_moves(plan, &world)?;
                cutscene_shots_judged =
                    crate::compiler::nav::check_cutscenes(plan, &world, &m, &am)?;
                // spec-0031: the one lethal-volume obligation routing cannot see.
                // A respawn SEAT inside a volume is reached by teleport and routes
                // perfectly while killing the party on arrival, forever. The wave
                // seating goes in with it: a mob put on a cell by the seating pass
                // is put there by declaration too, and its body is not the walker
                // the footing was proven for.
                //
                // **Before the route proofs, and that ordering is a judgement.** A
                // volume that swallows a posted place usually closes the route to
                // it as well, so the two findings arrive together — and `DW0510`
                // then sends the author to "move the volume, or give the party a
                // route around it" when what is actually wrong is that their
                // Keeper is standing in the pit. The specific cause is the more
                // actionable message, and the route closure is its symptom. This
                // proof reads the plan and the seating, never the routes, so
                // nothing is lost by asking it first.
                let lethal_seats = crate::compiler::lethal::check_respawn_seats(
                    plan,
                    campaign_spawn(plan),
                    &waves,
                )?;
                // spec-0062: **danger is visible, or the engine refuses it.**
                // `DW0891`, asked BEFORE every route proof and after `DW0511`,
                // and the order is the judgement §4 records: a volume that
                // catches walked floor usually closes a route as well and often
                // sits under a reach, so asked first the refusal names the
                // cause, and `DW0510`, `DW0850` and `DW0881` then judge a volume
                // the player can see. This proof reads the plan's volumes, the
                // lethality-free population and the block map, never a route, so
                // nothing is lost by asking it here.
                let danger = if plan.lethal_volumes.is_empty() {
                    crate::compiler::lethal::DangerVisibility::default()
                } else {
                    let blocks = &assembled.blocks;
                    // `DW0922` / `DW0923`: where each seated wave's members can
                    // get to by the movement a mob has. Measured before `DW0891`,
                    // because a volume only a mob can enter is a volume a
                    // modelled body reaches and `DW0891`'s zero-binding finding
                    // has to know it; judged after `DW0891`, so a volume the
                    // player cannot see is named for that first.
                    let wave_lethal =
                        crate::compiler::lethal::wave_reach(plan, &world, blocks, &waves);
                    let (mut binding, verdict) = crate::compiler::lethal::check_danger_is_visible(
                        plan,
                        &world,
                        blocks,
                        campaign_spawn(plan),
                    );
                    binding.credit_waves(&wave_lethal);
                    // Stated whether it found anything or not, and before the
                    // verdict is taken: a refusal owes its reader the population
                    // it was measured against as much as a pass does.
                    eprintln!("{}", binding.line());
                    eprintln!("{}", wave_lethal.line());
                    verdict?;
                    warnings.extend(binding.findings());
                    wave_lethal.verdict()?;
                    put_json(
                        &mut out,
                        "validation/wave-lethal.json",
                        &wave_lethal.to_json(),
                    );
                    binding
                };
                // spec-0085: `DW0943`, **a blinding beside a drop** — after
                // `DW0891`, so every volume it reasons about is one the player
                // could see, and before the route proofs. The binding is printed
                // whether or not the campaign grants a blinding, zeroes included.
                let (blind, blind_verdict) =
                    crate::compiler::blind::check(plan, &world, campaign_spawn(plan));
                eprintln!("{}", blind.line());
                blind_verdict?;
                // spec-0086 §4: a loop's slab, its closed view, the bodies in its
                // span and its tiling, over the world as shipped (relight
                // fixtures and world-load seals included). Before the route
                // proofs, because a loop that cannot be polled or seen through is
                // the cause, and a route closed by its slab is the consequence.
                if !plan.loops.is_empty() {
                    let (binding, refusal) =
                        crate::compiler::r#loop::check(&crate::compiler::r#loop::Inputs {
                            plan,
                            world: &world,
                            blocks: &assembled.blocks,
                            placements: &relight.placements,
                            seals: &assembled.gate_seals,
                            wave_seats: &waves,
                        });
                    eprintln!("{}", binding.line());
                    loop_gate = Some(binding);
                    if let Some(f) = refusal {
                        return Err(BuildFailure::Diagnostic {
                            code: f.code,
                            message: f.message,
                        });
                    }
                }
                // DW0311, with its binding stated whichever way it goes
                // (spec-0083 §5): every leg partitioned into walked, carried by
                // a crossing and carried by a link.
                let (route_binding, route_verdict) =
                    crate::compiler::nav::check_critical_path_bound(plan, &world);
                eprintln!("{}", route_binding.line());
                route_verdict?;
                legs_carried = route_binding.carried;
                // v0.6 checkpoint no-stranding + placement proofs (spec-0012,
                // DW0315/DW0316) and stealth-zone standable/reachable proofs
                // (spec-0014, DW0327), re-rooting DW0311 reachability at each beat.
                crate::compiler::nav::check_checkpoints(plan, &world)?;
                // DW0921: no place a body can reach from the route by walking,
                // falling or jumping is one it cannot leave. The binding is
                // printed before the verdict is taken, like DW0891's.
                let (leave_binding, leave_verdict) = crate::compiler::nav::check_bodies_can_leave(
                    plan,
                    &world,
                    playable_region(plan).map(|r| (r.min, r.max)),
                );
                eprintln!("{}", leave_binding.line());
                leave_verdict?;
                put_json(
                    &mut out,
                    "validation/leave-proof.json",
                    &leave_binding.to_json(),
                );
                // DW0924: a body a `kill` objective waits on cannot get to a
                // place it survives and the party cannot strike it from. After
                // DW0921 because it reads the same playable region and a party
                // that can be trapped is the worse finding.
                {
                    let strand = crate::compiler::strand::check(
                        plan,
                        &world,
                        &waves,
                        &crate::compiler::lethal::stands_at_roots(plan, campaign_spawn(plan)),
                        playable_region(plan).map(|r| (r.min, r.max)),
                    );
                    if strand.waves > 0 {
                        eprintln!("{}", strand.line());
                    }
                    strand.verdict()?;
                    if strand.waves > 0 {
                        put_json(&mut out, "validation/strand.json", &strand.to_json());
                    }
                }
                if !plan.lethal_volumes.is_empty() {
                    lethal_gate = Some(crate::compiler::lethal::gate(
                        plan.campaign,
                        &plan.lethal_volumes,
                        world.lethal_cells(),
                        lethal_seats,
                        crate::compiler::nav::critical_leg_count(plan),
                        // One template per resolved volume, and a `_shut` one
                        // more per staged volume (see `emit_lethal_packtests`).
                        plan.lethal_volumes.len()
                            + plan
                                .lethal_volumes
                                .iter()
                                .filter(|v| v.staged.is_some())
                                .count(),
                        danger,
                    ));
                    if let Some(g) = lethal_gate.as_mut() {
                        g.blind = blind;
                    }
                }
                // spec-0032: the recovery stake's placement table and its proofs
                // (`DW0525` no route back, `DW0526` no safe footing). Placed after
                // the lethal-volume seat proof because it CONSUMES the volumes as
                // death regions — a volume that strands the party is a worse
                // finding, and it should be reported first.
                stake_table = crate::compiler::stake::build(plan, &world, campaign_spawn(plan))?;
                // …and the contract the bot tier needs to prove any of it at
                // runtime. Built here, from the SAME table the proofs above ran
                // on, because a PackTest fake player is permanently undamageable
                // (measured 2026-08-03 and 2026-08-09) and so the whole death loop
                // is the mineflayer tier's claim to make.
                death_plan = crate::compiler::deathplan::build(
                    plan,
                    &world,
                    campaign_spawn(plan),
                    stake_table.as_ref(),
                );
                crate::compiler::nav::check_stealth_zones(plan, &world)?;
                // …and the onset-survivability proof on top of them (DW0355): a
                // punishing beat must be escapable in `grace_ticks` from where the
                // player provably stands when it arms, and from every checkpoint
                // that can respawn them back into it.
                crate::compiler::nav::check_stealth_onset(plan, &world)?;
                // v0.6 trap completability proof (spec-0011, DW0342): every lethal
                // trap on the forced critical path must be avoidable, survivable
                // (`once`), or disarmable, else the party is provably killed or
                // soft-looped. Uses the move-npc waypoints (`m`) for the forced-path
                // cell set.
                crate::compiler::nav::check_traps(plan, &world, &m)?;
                // spec-0016 §2 shortcut doors (DW0373/DW0374): the long route must
                // exist while the gate is sealed, and opening the gate must
                // genuinely shorten the crossing. The critical path above was
                // already proven with every shortcut gate SEALED (Plan::build seals
                // them at step 0), so the delve is finishable the long way.
                // `DW0425` (the wrong-side refusal) is raised with the hitbox
                // proofs above, well before this block — its own call site says
                // why it has to reach the author first.
                //
                // Every click trigger must land on something (DW0426). The
                // ledger it returns is emitted below: "how many clicks did this
                // proof resolve a body for" is the one fact that distinguishes a
                // campaign whose presses all land from one that arms none.
                put_json(
                    &mut out,
                    "validation/press-bodies.json",
                    &check_trigger_bodies(plan)?.to_json(),
                );
                crate::compiler::nav::check_shortcuts(plan, &world, campaign_spawn(plan))?;
                // spec-0016 §3 ambush counterplay (DW0376): 初见杀 is legitimate,
                // a pocket with no retreat is not.
                crate::compiler::nav::check_ambushes(plan, &world, campaign_spawn(plan))?;
                // spec-0016 §4 timed gates (DW0378): a gate that punishes bad
                // timing is the point; one that punishes every timing is a slot
                // machine. At least 20% of the cycle must admit a crossing.
                crate::compiler::nav::check_timed_gates(plan, &world)?;
                // The third rung. A gate's `disarm` lever must be
                // reachable while the gate is still SHUT — a jam you can only
                // pull after surviving the crossing disables nothing (DW0393).
                crate::compiler::nav::check_timed_gate_disarms(plan, &world, campaign_spawn(plan))?;
                // spec-0016 §4 addendum — hazard observability (DW0388). The
                // dossier's strongest finding: what makes a periodic hazard fair
                // is not its ratio but that you can stand somewhere safe and
                // WATCH it before committing. Error tier for a souls campaign (it
                // declares a bonfire), warning tier otherwise.
                let unobserved = crate::compiler::nav::check_hazard_observability(
                    plan,
                    &world,
                    campaign_spawn(plan),
                )?;
                // spec-0016 §7 pacing lints (DW0379 retry cost, DW0380 optional-
                // elite bypass). Warning tier: both are design judgements the
                // compiler can MEASURE but must not overrule — a long walk back
                // can be the authored point, and the owner's QA hour decides.
                pacing = crate::compiler::nav::pacing_lints(plan, &world);
                pacing.extend(unobserved);
                // Export the DW0311-proven critical-path routes as validation
                // metadata: thinned per-leg waypoint polylines the harness
                // replays as successive nearby goals, so no single giant mineflayer A*
                // solve strands the bot on a large open cave. NOT shipped gameplay —
                // lives under `validation/` (excluded from the delve image, like
                // packtest-datapack/). Emitted only when a walked leg exists, so a
                // campaign with none stays byte-identical to before. Uses the same
                // relight-aware `world` as the DW0311 check it exports.
                let routes = crate::compiler::nav::critical_path_routes(plan, &world);
                // Structural self-check: every exported waypoint must be
                // genuinely standable in this FINAL world (settled + water-flooded +
                // fixtures). Makes it impossible to ship a waypoint the game floods
                // or walls — the water-flow / post-nav-mutation divergence class —
                // failing the build loudly (DW0314) instead of stranding the bot.
                crate::compiler::nav::verify_exported_routes(&world, &routes)?;
                // spec-0065 §4.3: what the furniture exclusion bound, over the
                // same world and the same legs the proofs above walked. Printed
                // on every build that walks, zeroes included.
                let furniture =
                    crate::compiler::nav::furniture_binding(plan, &world, &routes, &m, &am);
                eprintln!("{}", furniture.line());
                put_json(
                    &mut out,
                    "validation/furniture-gate.json",
                    &furniture.to_json(),
                );
                // `DW0850`: the volume that completes a `reach` and the footing
                // a body can reach it from are the same place. Bound HERE, to
                // the same build event and the same final world the waypoint
                // proof judges — the endpoint snap searches three blocks and
                // the completion cube reaches one, so a route can be proven,
                // exported and walked to a cell that never fires the objective.
                crate::compiler::reach::check_reach_completion(
                    plan,
                    &world,
                    &routes,
                    campaign_spawn(plan),
                )?;
                // `DW0881`: the other direction of the same sentence. `DW0850`
                // asks whether the party can complete this at all; this asks
                // whether anybody can complete it WITHOUT arriving. The volume is
                // centred on the anchor in all three axes and vanilla tests it
                // against the whole body box, so a raised anchor whose radius
                // reaches the floor below completes from that floor and the party
                // never climbs. Bound here, to the same event and the same final
                // world, and its binding line is printed whether it found
                // anything or not — a count only says something when the run that
                // found nothing prints it too.
                let (reach_footprint, off_floor) = crate::compiler::reach::check_reach_footprint(
                    plan,
                    &world,
                    campaign_spawn(plan),
                );
                eprintln!("{}", reach_footprint.line());
                off_floor?;
                // Stair-orientation proof (DW0430). Nav models a stair
                // as a full cube, so a reversed stair reads as a legal one-block
                // jump and every existing proof passes — the delve ships with a
                // staircase the player must hop up tread by tread. This is the
                // one check that reads `facing`, over the same proven routes,
                // against the same assembled world.
                if !routes.is_empty() {
                    let route_cells: Vec<Vec<[i32; 3]>> =
                        routes.iter().map(|r| r.cells.clone()).collect();
                    let blocks = &assembled.blocks;
                    crate::compiler::stairs::check_stair_orientation(
                        blocks,
                        Some(plan),
                        &route_cells,
                    )?;
                }
                if !routes.is_empty() {
                    put_json(
                        &mut out,
                        "validation/critical-path-waypoints.json",
                        &crate::compiler::waypoints::waypoints_json(plan, &routes),
                    );
                }
                path_legs.push((
                    "critical-path".to_string(),
                    plan.critical_path.clone(),
                    routes.clone(),
                ));
                // Visual-tier POV cameras (spec-0003): one first-person shot per
                // corner-thinned waypoint. Their eye cells are proven clear in the
                // FINAL assembled world with every other kind's, at the one place
                // a render plan can be built (`PlannedShots::into_document`) —
                // this used to be a POV-only check here, which is exactly why the
                // identical defect on a seam camera was invisible.
                pov_shots = crate::compiler::render_plan::pov_shots(plan, &routes);
                // spec-0025 branch navigation, made first-class. The
                // DW0311 proof above quantifies over the DEFAULT playthrough
                // only, and the waypoint export followed it — so a branch run
                // walked its fork-divergent legs with no proof behind them and
                // no waypoints under them (single-goal navigation, which is
                // terrain-flaky exactly where the proven path is deterministic).
                // Here every REACHABLE branch's exported path gets both halves:
                // its own DW0311 (each walked leg routed over this same
                // assembled world, under the BRANCH's own causal gate seals, in
                // its own step space — `Plan::branch_gate_model`; default-path
                // indices belong to a different sequence and must never be
                // inherited) and its own waypoint artifact, derived from those
                // proven routes exactly as the critical path's is (same
                // thinning, same DW0314 standability self-check, same JSON
                // shape — `waypoints_json`). Deterministic: branches enumerate
                // in declaration-order (ADR-0006).
                let realized = crate::compiler::branch::realize(plan.campaign);
                if !realized.is_empty() {
                    let flow = crate::compiler::flow::Flow::new(plan.campaign);
                    for r in &realized {
                        let Some(widx) = r.world else { continue };
                        let label = |e: Failure| Failure {
                            code: e.code,
                            message: format!("branch `{}`: {}", r.branch.id, e.message),
                        };
                        let branch_fail = |e: plan::PlanError| BuildFailure::Diagnostic {
                            code: e.failure.code,
                            message: format!("branch `{}`: {}", r.branch.id, e.failure.message),
                        };
                        let unlinked = plan
                            .branch_critical_path(&flow, &flow.playthrough_in(widx))
                            .map_err(branch_fail)?;
                        // The branch's own links (spec-0083 §6): taken over its
                        // own steps and gate model, then spliced into its path.
                        let takes = if plan.links.is_empty() {
                            plan::LinkTakes::default()
                        } else {
                            let (region_events, ancestors) = plan.branch_gate_model(&unlinked);
                            let ancestor = |g: usize, s: usize| {
                                g == 0 || ancestors.get(&s).is_some_and(|a| a.contains(&g))
                            };
                            crate::compiler::nav::take_branch_links(
                                plan,
                                &world,
                                campaign_spawn(plan),
                                &unlinked,
                                &region_events,
                                &ancestor,
                            )
                            .map_err(label)?
                        };
                        let cp = if takes.is_empty() {
                            unlinked
                        } else {
                            plan.branch_critical_path_linked(
                                &flow,
                                &flow.playthrough_in(widx),
                                &takes,
                            )
                            .map_err(branch_fail)?
                        };
                        branch_takes.insert(r.branch.slug.clone(), takes);
                        let (region_events, ancestors) = plan.branch_gate_model(&cp);
                        let ancestor = |g: usize, s: usize| {
                            g == 0 || ancestors.get(&s).is_some_and(|a| a.contains(&g))
                        };
                        crate::compiler::nav::check_branch_path(
                            plan,
                            &world,
                            campaign_spawn(plan),
                            &cp,
                            &region_events,
                            &ancestor,
                        )
                        .map_err(label)?;
                        let branch_routes = crate::compiler::nav::branch_path_routes(
                            &world,
                            campaign_spawn(plan),
                            &cp.steps,
                            &cp.transport_by_step,
                            &region_events,
                            &ancestor,
                        );
                        crate::compiler::nav::verify_exported_routes(&world, &branch_routes)
                            .map_err(label)?;
                        if !branch_routes.is_empty() {
                            branch_waypoints.push((
                                r.branch.slug.clone(),
                                crate::compiler::waypoints::waypoints_json(plan, &branch_routes),
                            ));
                        }
                        path_legs.push((r.branch.slug.clone(), cp.steps.clone(), branch_routes));
                    }
                }
                (m, am)
            } else {
                (Vec::new(), Vec::new())
            };
            // Body clearance (DW0450/DW0451): no NPC or actor body may
            // occupy the same space as block geometry — not at the anchor it is
            // summoned on, and not at any tick of any walked leg. A walked
            // destination was already safe by construction (endpoint snapping +
            // passable-cell A*); a `summon` does no snapping, which is how the
            // island shipped a 2.9-tall warden inside the cliff face beside its
            // cave mouth with every other proof green. Runs after the moves are
            // planned because the walked waypoints are half of what it proves.
            warnings.extend(
                crate::compiler::clearance::check_body_clearance(
                    plan,
                    &world,
                    &moves,
                    &actor_moves,
                )
                .map_err(|e| BuildFailure::Diagnostic {
                    code: e.code,
                    message: e.message,
                })?,
            );
            // …and the move that got the body there must be one the body can
            // make (DW0452/DW0453, island round 21). `clearance` proves where a
            // body IS; this proves what it DID. The two island sightings it
            // exists for: eight sheep walking through a closed fence gate the
            // owner could not walk through herself, and a sheep leaving the
            // beach fold by stepping onto its wall's full-cube course instead of
            // using the pen's opening. Capabilities come from the entity, so a
            // spider routed over a wall stays silent and a sheep does not.
            let (traversal_warnings, gate) =
                crate::compiler::traversal::check_traversal(plan, &world, &moves, &actor_moves)
                    .map_err(|e| BuildFailure::Diagnostic {
                        code: e.code,
                        message: e.message,
                    })?;
            warnings.extend(traversal_warnings);
            traversal_gate = Some(gate);
            // …and prove the sun is not going to fight the party's battle for it
            // (DW0496). Runs HERE because it needs the seated cells:
            // the question is whether open sky stands within one aggro radius of
            // where the mobs actually land, on ground they can walk to — not of
            // an anchor they stand around. The hollow-vigil gate yard is the
            // motivating case: roof and two walls carved off, noon pinned, and
            // two of three footmen dead to sunlight before the party could
            // engage them, with every other proof green.
            crate::compiler::daylight::check_daylight_staging(
                plan,
                &world,
                &assembled.blocks,
                &waves,
            )
            .map_err(|e| BuildFailure::Diagnostic {
                code: e.code,
                message: e.message,
            })?;
            // …and its mirror: prove the body will fight at all (DW0920). A
            // drowned takes no land target while the level is bright, so a choir
            // staged on dry ground under a bright hour walks to its water and
            // leaves the party a fight nobody answers — vesperhold's Undertide
            // Pool. Same seated cells, same reach, same population.
            let (engage, refused) =
                crate::compiler::engage::check_engagement(plan, &world, blocks, &waves);
            eprintln!("{}", engage.line());
            if let Some(e) = refused {
                return Err(BuildFailure::Diagnostic {
                    code: e.code,
                    message: e.message,
                });
            }
            // spec-0023 §2: the winnability arithmetic. Runs here because it
            // needs the SEATED spawn cells (the exact cells the datapack will
            // summon on) as well as the campaign's declarations — a hostile the
            // party cannot reach is a property of where it actually lands, not
            // of where its anchor is.
            //
            // Gated on EVERY fight, wave-shaped or actor-shaped
            // (`combat::mandatory_fights`). It used to be gated on `kill`-a-wave
            // alone, which meant a delve whose combat is entirely actors ran none
            // of spec-0023 at all — the whole pass silently inapplicable, with
            // every board green.
            if crate::compiler::combat::mandatory_fights(plan).any() {
                warnings.extend(
                    crate::compiler::combat::check_winnability(plan, &world, &waves).map_err(
                        |e| BuildFailure::Diagnostic {
                            code: e.code,
                            message: e.message,
                        },
                    )?,
                );
            }
            // The bot ladder's combat plan (spec-0023 §1): which encounters exist,
            // what the content bills each as, which checkpoint governs a death at
            // it, and — the muster — what each wave DECLARES its bodies to be, so
            // the ladder can read the live ones against it. Validation metadata
            // only: it lives under `validation/`, which `Dockerfile.delve`
            // excludes, so no shipped byte moves.
            //
            // spec-0016 §6: resolve and prove each TD lane polyline (DW0386). The
            // proven cells are what `patrol_target` carries, so the squad is only
            // ever sent somewhere it can stand and walk to.
            //
            // Before the combat plan: a run-back is measured against where the
            // hostiles actually are, and a lane wave is where it marches.
            let lanes = crate::compiler::nav::plan_lanes(plan, &world)?;
            // A campaign whose only fight is an ACTOR still ships a plan. It has no
            // `encounters[]` for the ladder to read or clear, but `fights` is the
            // binding count for the whole combat pass — and a five-hostile campaign
            // that emits no plan at all reads as combat-free, which is the silence
            // the block was added for.
            if crate::compiler::combat::has_encounters(plan)
                || crate::compiler::combat::mandatory_fights(plan).any()
            {
                let mandatory = crate::compiler::combat::encounters(plan);
                // Run-backs (spec-0016 §1): a cleared `respawns_on_rest` wave a
                // rest re-seats beside a leg the path walks afterwards. Measured
                // over every exported path, against the same aggro model the
                // respawn safe zone uses.
                let sources = crate::compiler::nav::aggro_sources(plan, &world, &waves, &lanes);
                let legs: Vec<crate::compiler::combat::PathLegs<'_>> = path_legs
                    .iter()
                    .map(|(label, steps, routes)| crate::compiler::combat::PathLegs {
                        label: label.clone(),
                        steps,
                        routes,
                    })
                    .collect();
                let run_backs = crate::compiler::combat::run_backs(plan, &world, &legs, &sources);
                put_json(
                    &mut out,
                    "validation/combat-plan.json",
                    &crate::compiler::combat::combat_plan_json(plan, &mandatory, &run_backs),
                );
            }
            // spec-0016 §1: the RESPAWN-POINT safe zone
            // (DW0478). Runs here because it needs both halves of where the
            // hostiles actually are — the seated spawn cells above and the lane
            // polylines just resolved — measured against every rest point. A
            // respawn point inside a hostile's aggro range is a soft-lock: rest
            // and death both deliver the party into contact on arrival.
            //
            // "Every rest point" is every `CheckpointPlan`, bonfire or plain
            // `set-checkpoint`. The ledger states how many pairs were compared,
            // because a proof that examined nothing must not read as a pass.
            let respawn_safety =
                crate::compiler::nav::check_respawn_safe_zone(plan, &world, &waves, &lanes)?;
            put_json(
                &mut out,
                "validation/respawn-safety.json",
                &respawn_safety.to_json(),
            );
            // spec-0022: resolve and prove every `volley` / `collapse`. Volley
            // coverage is proven by construction (one shot per standable
            // kill-zone cell, or DW0442 naming the cell it cannot reach), and a
            // collapse must leave the critical path completable in its SPRUNG
            // state (DW0445).
            let payloads = plan_payload_verbs(plan, &world, blocks)?;
            (moves, actor_moves, waves, rings, lanes, payloads)
        }
    } else {
        (
            Vec::new(),
            Vec::new(),
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeMap::new(),
            PayloadPlans::default(),
        )
    };

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
    // several placed pieces — the insert is idempotent, same bytes)
    for template in plan.placed_pieces().flat_map(|p| &p.templates) {
        if let Some(bytes) = structures.get(&template.structure_file) {
            out.insert(
                format!("datapack/data/{ns}/structure/{}.nbt", template.structure_id),
                bytes.clone(),
            );
        }
    }

    // functions
    // Placement sentinels: one known block per distinct structure, so the
    // runtime can verify each `place template` landed (see `setup` emission).
    let mut sentinels: Sentinels = BTreeMap::new();
    let placed: Vec<_> = plan.placed_pieces().flat_map(|p| &p.templates).collect();
    let picked = crate::par::map(&placed, |template| {
        structures
            .get(&template.structure_file)
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

    // ---- validate every emitted vanilla mcfunction ----
    let mut errors = Vec::new();
    for (path, bytes) in &out {
        if is_vanilla_function(path)
            && let Ok(body) = std::str::from_utf8(bytes)
        {
            errors.extend(tree.validate_function(body));
        }
    }
    if !errors.is_empty() {
        return Err(BuildFailure::Validation(errors));
    }

    // ---- affordance-hardware self-check (DW0420 / DW0421) ----
    // Every right-click target the compiler owns must be VISIBLE in the shipped
    // datapack, and only its own consumption may retire that visibility. Read
    // off the finished tree, so it judges the commands that actually ship.
    // See `crate::compiler::affordance` for the drowned-bell soft-lock this encodes.
    crate::compiler::affordance::check(&affordances(plan), &out)?;

    // ---- fixture-class self-check (DW0545) ----
    // `DW0421` above is tag-keyed and asks who may DESTROY an affordance's
    // hardware. A region verb selects by BOX and MOVES what it finds, so it slips
    // past that entirely — which is how a lift carries a recovery stake's marker
    // away from the position its ledger recorded, after which `stk_gc_<s>` deletes
    // the marker and the wager with it. So the same rule is stated one verb wider,
    // over the emitted tree: every engine-summoned hitbox, mark and display
    // declares whether it is a PLACE or is carried by a BODY, and no box-narrowed
    // entity selector may reach a place. Feature-blind, so a region verb nobody
    // has written yet is covered by existing.
    let mut fixture_gate = crate::compiler::affordance::check_fixtures(&out)?;
    // Counted off the shipped tree rather than reported by the emitter that wrote
    // them, so the ledger states what a reader can go and open.
    fixture_gate.packtests = out
        .keys()
        .filter(|p| p.starts_with("packtest-datapack/") && p.contains("/test/fixture_"))
        .count();
    put_json(
        &mut out,
        "validation/fixture-gate.json",
        &fixture_gate.to_json(),
    );

    // ---- the party is seen in its own cutscenes (spec-0095, DW0971) ----
    // Every cutscene declared `present` places its stand-ins before the party
    // goes to spectator and removes them at its end; every `absent` one places
    // none. Read off the shipped tree against the declarations.
    let parties = cutscene_parties(plan);
    if !parties.is_empty() {
        let gate = crate::compiler::standin::check(ns, &parties, &out)?;
        eprintln!("{}", gate.line());
        put_json(&mut out, "validation/stand-in-gate.json", &gate.to_json());
    }

    // ---- a watcher is out of play everywhere (spec-0077 §5, DW0926) ----
    // A cutscene holds every player in the observation state, and a respawn wait
    // holds one while the rest play on. Every positional player selector in the
    // shipped tree must exclude the observation tag or stand at a site
    // `crate::compiler::observer::ALLOWED` names with its reason (an engine
    // self-check: see `crate::compiler::observer::check`). Feature-blind, read
    // off the shipped bytes, and run on every build.
    let census = crate::compiler::observer::check(&out).map_err(|e| BuildFailure::Diagnostic {
        code: e.code,
        message: e.message,
    })?;
    eprintln!("{}", census.binding());
    put_json(
        &mut out,
        "validation/observer-census.json",
        &census.to_json(),
    );

    // ---- the effect-root walk's own binding ledger ----
    // Every other proof in this compiler publishes its binding as a
    // `validation/*.json`; the walk that underpins most of them published a
    // stderr STRING, so nothing downstream could assert it bound to anything.
    // A build whose effect walk reaches zero bundles is a build where every
    // effect-shaped proof is vacuous, and until this file existed that was not
    // a fact any gate could read (spec-0039 criterion 6).
    let root_binding =
        crate::compiler::plan::for_each_effect_root(plan.campaign, &mut |_site, _effs| {});
    put_json(
        &mut out,
        "validation/effect-roots.json",
        &root_binding.to_json(),
    );

    // ---- call-graph integrity (DW0497) ----
    // Every `function <ns>:<name>` the compiler just wrote must point at a
    // function the compiler wrote. Vanilla resolves an unknown function to
    // nothing at all — no error, no log line — so an emitter whose call walk and
    // machinery walk disagree ships a verb that simply never happens. That is
    // exactly how the island's round-21 build lost two of its three storm waves
    // (see `crate::compiler::integrity`). Feature-blind and last, so it guards every
    // emitter, including ones not yet written.
    crate::compiler::integrity::check_tree(ns, &out).map_err(|e| BuildFailure::Diagnostic {
        code: e.code,
        message: e.message,
    })?;

    // ---- PackTest batch-state ownership (DW0807) ----
    // The generated suite runs as ONE batch on ONE shared server, so a template
    // that runs the real `tick` and asserts on a gated outcome must OWN every
    // `#party` term that gate reads — otherwise its verdict is decided by
    // whichever sibling ran last, and the campaign-playthrough template holds the
    // whole party ledger across ticks (see `crate::compiler::batchstate`). Feature-blind and
    // read off the shipped bytes, so it guards templates not yet written.
    let batch_binding = crate::compiler::batchstate::check_tree(ns, &out).map_err(|e| {
        BuildFailure::Diagnostic {
            code: e.code,
            message: e.message,
        }
    })?;
    warnings.extend(batch_binding.finding());

    // ---- runtime-watch coverage of per-object bodies (DW0810) ----
    // A mechanic whose runtime body is emitted PER OBJECT gets one body per
    // declared object, over its own region, with its own judgement — so a suite
    // that drives one of them has proven nothing about the next. The timed-gate
    // emitter bound `first()` and shipped a three-gate level whose LETHAL gate
    // was the third with no runtime proof at all, green throughout (see
    // `crate::compiler::watch`). Read off the shipped bytes with no table of mechanics, so
    // it guards emitters not yet written.
    let watch_ids = crate::compiler::watch::declared_ids(input_bytes);
    let (watch_binding, unwatched) = crate::compiler::watch::check_tree(ns, &out, &watch_ids);

    // ---- undischarged per-object watch claims (DW0811) ----
    // The refusal half, and it is drawn one step in from `DW0810` on purpose.
    // Nothing in the finished tree separates "the emitter meant to prove every
    // member and skipped some" from "the suite drives one exemplar by design" —
    // eight standing gallery families are honestly the second — so a refusal read
    // off the bytes alone would need a per-family allowlist, which is an opt-out
    // the defect can supply. The distinction lives in the EMITTER, so the emitter
    // registers its claim over the plan's own authored list and the claim is
    // judged against the shipped suite: `declared` cannot shrink when the walk
    // skips members, and `invoked` cannot be faked because it is read off bytes.
    let watch_claims = packtest::watch_claims(plan);
    let (claim_binding, breaches) = crate::compiler::watch::check_claims(ns, &out, &watch_claims);

    put_json(
        &mut out,
        "validation/watch-ledger.json",
        &watch_binding.to_json(&unwatched),
    );
    put_json(
        &mut out,
        "validation/watch-claims.json",
        &claim_binding.to_json(&breaches),
    );

    if let Some(d) = crate::compiler::watch::claim_finding(&claim_binding, &breaches) {
        return Err(BuildFailure::Diagnostic {
            code: crate::compiler::watch::DW_CLAIM_NOT_DISCHARGED,
            message: d.message,
        });
    }
    warnings.extend(crate::compiler::watch::finding(&watch_binding, &unwatched));

    // ---- score-seeding integrity (DW0495) ----
    // Every `if score` / `unless score` / `scores={…}` the compiler just wrote
    // must read an entry the pack itself creates, or be written so a missing entry
    // cannot change its answer. On the pinned 1.21.11 server a score that was never
    // written is not zero — every comparison against it is false — which is how
    // `if score @s dw.deaths > @s dw.death_ack` silently swallowed every player's
    // FIRST death for as long as checkpoints have existed (see `crate::compiler::seeding`).
    // Feature-blind and read off the finished tree, beside the call-graph proof.
    crate::compiler::seeding::check_tree(ns, &out).map_err(|e| BuildFailure::Diagnostic {
        code: e.code,
        message: e.message,
    })?;

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
