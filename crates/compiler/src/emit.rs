//! Deterministic emission of the `<out>/` build tree (spec-0002).
//!
//! All gameplay wiring is compiler-generated (ADR-0001): the LLM never writes
//! mcfunction. Output is a `BTreeMap<path, bytes>` so ordering is defined
//! (ADR-0006); `manifest.json` hashes make the double-build gate a one-line
//! comparison.
//!
//! JSON is serialized with `serde_json` (default `BTreeMap` maps → sorted keys)
//! plus a trailing newline; mcfunction bodies are built line-by-line. No
//! wall-clock, hostname, locale or absolute path enters any byte.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::commands::{CommandError, CommandTree};
use crate::plan::{
    self, Plan, ResolvedAnchor, Step, campaign_start_quests, obj_score, objective_effects,
    objective_quest, quest_active_score, quest_score,
};
use crate::{DELVEC_VERSION, MC_VERSION, PACK_FORMAT};

use delvewright_dsl::DwCode;
use delvewright_dsl::{
    CompareOp, EquipItem, Gate, MobEquipment, Objective, QuestEffect, StateCompare, StateId,
    StateScope, Trigger, is_v03, is_v04, is_v06,
};

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

/// `DW0312`: a `spawn-wave` needs more standable spawn cells near its anchor than
/// the anchor's own assembled room provides. Wave-mob placement seats
/// each mob on a compiler-validated standable cell confined to that room; when the
/// wave's mob count exceeds the room's footing, the build fails here rather than
/// letting mobs pile into blocks or spill across a socket seam. Analysis-tier
/// (exit 2, like reachability `DW02xx`): the fix is a content-design capacity
/// choice — shrink the wave or use a larger room — not a compiler/geometry defect.
pub const DW_WAVE_NO_ROOM: DwCode = DwCode::every_version("DW0312");

/// `DW0310`: a `spawn-wave` references a wave whose spawn anchor resolves in no
/// assembled area, so the emitted `function <ns>:spawn_<wave>` call would dangle
/// and the wave never spawn (see [`check_wave_spawns`]).
///
/// It was the workspace's last bare `"DWxxxx"` string literal in a code position
/// — every other code already went through a named constant — and typing the
/// codes is what turned that from a style difference into a compile error.
pub const DW_WAVE_SPAWN_UNRESOLVED: DwCode = DwCode::every_version("DW0310");

/// `DW0387`: a `summon: aggro-edge` wave (spec-0016 §6) whose perception ring
/// offers too few valid cells. The ring is the standable, walk-reachable,
/// line-of-sight cells at a mob's own `follow_range` from the defended anchor;
/// with fewer of them than the wave has mobs there is nowhere legal to
/// materialize. This is an error and not a silent short spawn on purpose — the
/// round-1 lesson was a "kill" objective whose wave never fully appeared, so the
/// countdown could never reach zero and the delve soft-locked with every other
/// proof green.
pub const DW_AGGRO_EDGE_NO_RING: DwCode = DwCode::every_version("DW0387");

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
pub const DW_BRANCH_TRANSPORT_DIVERGES: DwCode = DwCode::every_version("DW0494");

impl From<crate::nav::NavError> for BuildFailure {
    fn from(e: crate::nav::NavError) -> Self {
        BuildFailure::Diagnostic {
            code: e.code,
            message: e.message,
        }
    }
}

/// A placement sentinel: one known solid block of a structure, used at runtime
/// to verify a `place template` actually landed (structure_file → (local pos,
/// bare block id)). Chosen as the non-air block with the lowest `(y, z, x)` —
/// deterministic per structure bytes.
type Sentinels = BTreeMap<String, ([i32; 3], String)>;

/// Parse a gzipped vanilla structure `.nbt` and pick its sentinel block.
/// Returns `None` for unparseable or all-air structures (no runtime verify).
fn structure_sentinel(bytes: &[u8]) -> Option<([i32; 3], String)> {
    use flate2::read::GzDecoder;
    use std::io::Read;
    let mut raw = Vec::new();
    GzDecoder::new(bytes).read_to_end(&mut raw).ok()?;
    let root: fastnbt::Value = fastnbt::from_bytes(&raw).ok()?;
    let fastnbt::Value::Compound(root) = root else {
        return None;
    };
    let palette: Vec<Option<String>> = match root.get("palette") {
        Some(fastnbt::Value::List(entries)) => entries
            .iter()
            .map(|e| match e {
                fastnbt::Value::Compound(c) => match c.get("Name") {
                    Some(fastnbt::Value::String(s)) => Some(s.clone()),
                    _ => None,
                },
                _ => None,
            })
            .collect(),
        _ => return None,
    };
    let is_air = |name: &str| {
        matches!(
            name,
            "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
        )
    };
    let mut best: Option<([i32; 3], String)> = None;
    if let Some(fastnbt::Value::List(blocks)) = root.get("blocks") {
        for b in blocks {
            let fastnbt::Value::Compound(b) = b else {
                continue;
            };
            let pos: [i32; 3] = match b.get("pos") {
                Some(fastnbt::Value::List(p)) if p.len() == 3 => {
                    let mut out = [0i32; 3];
                    let mut ok = true;
                    for (i, v) in p.iter().enumerate() {
                        match v {
                            fastnbt::Value::Int(n) => out[i] = *n,
                            _ => ok = false,
                        }
                    }
                    if !ok {
                        continue;
                    }
                    out
                }
                _ => continue,
            };
            let state = match b.get("state") {
                Some(fastnbt::Value::Int(n)) => *n as usize,
                _ => continue,
            };
            let Some(Some(name)) = palette.get(state) else {
                continue;
            };
            if is_air(name) {
                continue;
            }
            let key = (pos[1], pos[2], pos[0]);
            let better = match &best {
                None => true,
                Some((bp, _)) => key < (bp[1], bp[2], bp[0]),
            };
            if better {
                best = Some((pos, name.clone()));
            }
        }
    }
    best
}

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
pub const DW_TEMPLATE_EXTENT: DwCode = DwCode::every_version("DW0803");

/// How much of the world [`check_template_extents`] actually examined.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TemplateExtentBinding {
    /// Structure templates the plan places, tiles counted individually.
    pub placed: usize,
    /// Templates whose bytes were loaded and decoded, so their declared extent
    /// could be compared — the binding count.
    pub checked: usize,
}

impl TemplateExtentBinding {
    /// The advisory a zero binding owes its reader, or `None`.
    ///
    /// A world with placed pieces whose templates none of decoded is not a
    /// clean run of this check: it is the check examining nothing while
    /// reporting success, which is the shape a green gate takes when it has
    /// stopped binding to anything.
    pub fn finding(&self) -> Option<delvewright_dsl::Diagnostic> {
        (self.placed > 0 && self.checked == 0).then(|| {
            delvewright_dsl::Diagnostic::warning(
                DW_TEMPLATE_EXTENT,
                "build",
                "template-extent binding",
                format!(
                    "the template-extent invariant examined 0 of {} placed structure \
                     template(s): none of their `.nbt` bytes were loaded or decodable, so the \
                     check passed without comparing anything. A metadata size that disagrees \
                     with its blocks would not have been seen",
                    self.placed
                ),
            )
        })
    }
}

/// Compare every placed template's declared extent against the extent its own
/// bytes declare. See [`DW_TEMPLATE_EXTENT`].
///
/// A template whose bytes are absent from `structures` is not a finding here —
/// that is `DW0300`'s job at load — but it does not count toward the binding
/// either, which is what [`TemplateExtentBinding`] exists to say out loud.
pub fn check_template_extents(
    plan: &Plan,
    structures: &BTreeMap<String, Vec<u8>>,
) -> Result<TemplateExtentBinding, BuildFailure> {
    let mut binding = TemplateExtentBinding {
        placed: 0,
        checked: 0,
    };
    for area in &plan.areas {
        for (piece, template) in area
            .pieces
            .iter()
            .flat_map(|p| p.templates.iter().map(move |t| (p, t)))
        {
            binding.placed += 1;
            let Some(bytes) = structures.get(&template.structure_file) else {
                continue;
            };
            let Some(actual) = crate::assembled::structure_size(bytes) else {
                continue;
            };
            binding.checked += 1;
            if actual != template.size {
                let whole = if piece.templates.len() == 1 {
                    String::new()
                } else {
                    format!(
                        " It is one of {} tiles of that zone, so the rest of the zone is placed \
                         around a piece that is not the shape the manifest says it is.",
                        piece.templates.len()
                    )
                };
                return Err(BuildFailure::Diagnostic {
                    code: DW_TEMPLATE_EXTENT,
                    message: format!(
                        "prefab `{}`: structure template `{}` is {}x{}x{} in its own `.nbt`, but \
                         the prefab metadata declares it {}x{}x{}. Every pass but the placement \
                         reads the declared size — the forceload span, the piece AABB the \
                         face-contract check compares, massing's footprint — so the world would \
                         be built around a shape that is not the one whose blocks arrive.{whole} \
                         The `.nbt` and its metadata are not the same export: re-export the \
                         piece, or fix whichever of the two is stale. Do NOT adjust the declared \
                         size to match: the sizes are two claims about one fact and the fix is to \
                         make them one export again",
                        piece.prefab_id,
                        template.structure_file,
                        actual[0],
                        actual[1],
                        actual[2],
                        template.size[0],
                        template.size[1],
                        template.size[2],
                    ),
                });
            }
        }
    }
    Ok(binding)
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
#[allow(clippy::too_many_arguments)]
pub fn build(
    plan: &Plan,
    input_bytes: &BTreeMap<String, Vec<u8>>,
    structures: &BTreeMap<String, Vec<u8>>,
    tree: &CommandTree,
    prefabs: &crate::registry::PrefabRegistry,
    language: Option<&str>,
    content_sha: &str,
    skins: &BTreeMap<String, Vec<u8>>,
) -> Result<BuildOutput, BuildFailure> {
    build_with_warnings(
        plan,
        input_bytes,
        structures,
        tree,
        prefabs,
        language,
        content_sha,
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
#[allow(clippy::too_many_arguments)]
pub fn build_with_warnings(
    plan: &Plan,
    input_bytes: &BTreeMap<String, Vec<u8>>,
    structures: &BTreeMap<String, Vec<u8>>,
    tree: &CommandTree,
    prefabs: &crate::registry::PrefabRegistry,
    language: Option<&str>,
    content_sha: &str,
    skins: &BTreeMap<String, Vec<u8>>,
) -> Result<(BuildOutput, Vec<delvewright_dsl::Diagnostic>), BuildFailure> {
    let ns = &plan.namespace;
    let mut out: BuildOutput = BTreeMap::new();

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
    // unsupported over the delve's `the_void` world and would despawn at placement,
    // silently deforming the shipped map. This is the authoritative direct gate —
    // it does not wait for a fall to happen to intersect the critical path (DW0311)
    // or a wave seat (DW0312). Analysis-tier (exit 2, mapped in main): a
    // prefab/generator defect the author fixes by adding a substrate. No-op for any
    // campaign whose prefabs have no gravity blocks (byte-identical output).
    if let Some(message) = crate::assembled::gravity_despawn_error(plan, structures) {
        return Err(BuildFailure::Diagnostic {
            code: crate::assembled::DW_GRAVITY_DESPAWN,
            message,
        });
    }

    // Stage-7 edit-script replay (spec-0017): apply the campaign's world edits
    // over the assembled model, re-proving the invariants after every batch
    // (gravity, relight, walkability, boundary safety — each failure names its
    // batch). `None` for a campaign without an edit script — every downstream
    // pass then takes its exact pre-stage-7 path, byte-identically.
    let edit_replay =
        crate::edit::replay(plan, prefabs, structures).map_err(|e| BuildFailure::Diagnostic {
            code: e.code,
            message: e.message,
        })?;
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
    if !plan.loot.is_empty() || !plan.collect_fills.is_empty() {
        let blocks = match &edit_replay {
            Some(er) => er.assembled.blocks.clone(),
            None => crate::assembled::assembled_blocks(plan, structures),
        };
        crate::loot::check_loot_containers(&blocks, &plan.loot).map_err(|e| {
            BuildFailure::Diagnostic {
                code: e.code,
                message: e.message,
            }
        })?;
        crate::loot::check_collect_containers(&blocks, &plan.collect_fills).map_err(|e| {
            BuildFailure::Diagnostic {
                code: e.code,
                message: e.message,
            }
        })?;
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
    let relight = match &edit_replay {
        Some(er) => crate::light::relight_over(plan, &er.assembled),
        None => crate::light::relight(plan, structures),
    };
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
    // Visual-tier player-POV shots (spec-0003): first-person cameras along the
    // proven critical-path routes. Filled inside the world block below (they need
    // the routes + the assembled occupancy for the DW0724 clear-eye self-check);
    // empty for a campaign with no walked leg, so its render plan stays byte-identical.
    let mut pov_shots: Vec<crate::render_plan::PovShot> = Vec::new();
    // spec-0025 per-branch waypoint artifacts: one
    // `validation/branch-waypoints-<branch>.json` per reachable branch, filled
    // inside the world block below (they need the assembled occupancy) and
    // emitted alongside the branch paths. Empty for a campaign with no declared
    // branch points, so nothing moves for anybody who has not opted in.
    let mut branch_waypoints: Vec<(String, Value)> = Vec::new();
    // The traversal proof's binding ledger (`compiler::traversal`), filled inside
    // the world block below. `None` for a campaign that assembles no world —
    // which is not the same fact as "examined nothing", so the artifact is
    // omitted entirely rather than emitted claiming a zero it never measured.
    let mut traversal_gate: Option<crate::traversal::TraversalGate> = None;
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
    // The lethal-volume proofs' binding ledger (`compiler::lethal`), filled inside
    // the world block below. `None` for a campaign that declares no volume — no
    // ledger, no artifact, no byte moved for anybody who has not opted in.
    let mut lethal_gate: Option<crate::lethal::LethalGate> = None;
    // The recovery stake's compile-time placement table (`compiler::stake`), and
    // the ledger of what its proofs looked at. `None` for a campaign that declares
    // no stake, which is the whole feature's byte-identity guarantee: no table, no
    // objectives, no functions, no artifact.
    let mut stake_table: Option<crate::stake::StakeTable> = None;
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
    // (`crate::teleport` records why that would be wrong).
    let teleport_gate = crate::teleport::check_bound_affordances(plan)?;

    // A dialogue node's conditionally-visible options are encoded as 2^n precomputed
    // variants; past the cap that is a pack-size decision, and past 32 it was a
    // compiler panic (DW0362).
    check_dialogue_variant_cap(plan)?;

    // Actor spawn anchors must resolve to a world position (spec-0014); a spawn is a
    // summon, not a walk, so this needs no occupancy model. DW0325 if one dangles.
    crate::nav::check_actor_placement(plan)?;

    // No body may stand on the affordance the party has to click (DW0359). Runs
    // right after the anchor-resolution seals and before any occupancy model:
    // it is pure box arithmetic over resolved cells, and it is the proof that the
    // island's giant — a 0.9 × 2.9 warden sharing `anchor/fire-pit` with two
    // interact objectives — was hiding a required beat behind its own hitbox.
    warnings.extend(crate::eclipse::check_body_eclipse(plan).map_err(|e| {
        BuildFailure::Diagnostic {
            code: e.code,
            message: e.message,
        }
    })?);

    // …and no OTHER affordance may contest the hitboxes a sealed gate arms to
    // answer a right-click (DW0422). Same box arithmetic, same tier:
    // two interaction entities in one cell is a ray-pick tie the client resolves
    // by iteration order, so one of them silently stops receiving clicks.
    crate::eclipse::check_seal_collisions(plan).map_err(|e| BuildFailure::Diagnostic {
        code: e.code,
        message: e.message,
    })?;

    // …and no two bodies the party CLICKS may stand close enough that the
    // crosshair cannot tell them apart (DW0489). `DW0359` above compares a body
    // against an affordance and skips every walker; this reads the v0.7 cast
    // ledger, which states beat by beat who is on stage together, and measures
    // the pairs it names. It is the proof the island's terminal finding needed —
    // two crew NPCs declared on one cell at the cave mouth.
    warnings.extend(
        crate::crosshair::check_crosshair_contests(plan).map_err(|e| BuildFailure::Diagnostic {
            code: e.code,
            message: e.message,
        })?,
    );
    let (moves, actor_moves, wave_placements, wave_rings, lane_routes, payload_plans): (
        Vec<crate::nav::MovePlan>,
        Vec<crate::nav::ActorMovePlan>,
        WavePlacements,
        WaveRings,
        crate::nav::LaneRoutes,
        PayloadPlans,
    ) = if assembles_world(plan) {
        {
            // spec-0022 payload verbs need the block map (a `collapse` settles
            // real blocks), not just the occupancy view.
            let blocks: BTreeMap<[i32; 3], String> = match &edit_replay {
                Some(er) => er.assembled.blocks.clone(),
                None => crate::assembled::assembled_blocks(plan, structures),
            };
            let world = match &edit_replay {
                Some(er) => {
                    let mut occ = crate::assembled::occupancy_of(
                        er.assembled.blocks.clone(),
                        &er.assembled.open_gates,
                    );
                    occ.solid.extend(relight.extra_solid.iter().copied());
                    // The ambient is the world-generator PREMISE (spec-0013), not
                    // geometry, and `from_occupancy` defaults it to `Void`. The
                    // sibling arm gets it for free through `from_plan`; this arm
                    // has to say it, or an `ocean` campaign's proofs would run
                    // against a void world that does not exist. Harmless while
                    // nothing here read it — `verify_boundary_safety` below now
                    // does.
                    // The world-load gate seals travel with this arm too, and
                    // they are the prefab's measurement, not the edit script's:
                    // a batch that writes INTO a gate region already appears as
                    // ordinary solid blocks above (and is `DW0353`'s advisory).
                    // Missing this line is how an edit-carrying campaign — the
                    // island is one — would have got a vacuous green out of the
                    // completability model while every fixture went red.
                    crate::nav::World::from_occupancy(occ)
                        .with_ambient(
                            crate::nav::Ambient::of_plan(plan),
                            crate::nav::built_volume(plan),
                        )
                        .with_world_load_seals(plan, er.assembled.gate_seals.clone())
                }
                None => {
                    crate::nav::World::from_plan_with_extra(plan, structures, &relight.extra_solid)
                }
            };

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
            // (`crate::nav::AnchorRoot`). That confinement is load-bearing here
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
            let fluid_escape = crate::nav::measure_fluid_escape(&world);
            fluid_escape_ledger = Some(fluid_escape.ledger());
            if let Some(e) = fluid_escape.finding() {
                return Err(BuildFailure::Diagnostic {
                    code: e.code,
                    message: e.message,
                });
            }

            crate::nav::verify_boundary_safety(&world, &crate::edit::anchor_starts(plan)).map_err(
                |e| BuildFailure::Diagnostic {
                    code: e.code,
                    message: e.message,
                },
            )?;

            let (moves, actor_moves) = if crate::nav::needs_world(plan) {
                let m = crate::nav::plan_moves(plan, &world)?;
                // move-actor (spec-0014): A* over the actor's footprint; DW0325 if
                // unroutable. Planned alongside move-npc from the same occupancy model.
                let am = crate::nav::plan_actor_moves(plan, &world)?;
                crate::nav::check_cutscenes(plan, &world, &m, &am)?;
                crate::nav::check_critical_path(plan, &world)?;
                // v0.6 checkpoint no-stranding + placement proofs (spec-0012,
                // DW0315/DW0316) and stealth-zone standable/reachable proofs
                // (spec-0014, DW0327), re-rooting DW0311 reachability at each beat.
                crate::nav::check_checkpoints(plan, &world)?;
                // spec-0031: the one lethal-volume obligation routing cannot see.
                // Every route proof already treats a volume's cells as impassable
                // (`nav::World::with_lethal`), so `DW0510` fell out of DW0311
                // above; a respawn SEAT inside a volume is reached by teleport and
                // routes perfectly while killing the party on arrival, forever.
                let lethal_seats = crate::lethal::check_respawn_seats(plan, campaign_spawn(plan))?;
                if !plan.lethal_volumes.is_empty() {
                    lethal_gate = Some(crate::lethal::gate(
                        plan.campaign,
                        &plan.lethal_volumes,
                        world.lethal_cells(),
                        lethal_seats,
                        crate::nav::critical_leg_count(plan),
                        // One template per resolved volume (see `emit_packtest`).
                        plan.lethal_volumes.len(),
                    ));
                }
                // spec-0032: the recovery stake's placement table and its proofs
                // (`DW0525` no route back, `DW0526` no safe footing). Placed after
                // the lethal-volume seat proof because it CONSUMES the volumes as
                // death regions — a volume that strands the party is a worse
                // finding, and it should be reported first.
                stake_table = crate::stake::build(plan, &world, campaign_spawn(plan))?;
                // …and the contract the bot tier needs to prove any of it at
                // runtime. Built here, from the SAME table the proofs above ran
                // on, because a PackTest fake player is permanently undamageable
                // (measured 2026-08-03 and 2026-08-09) and so the whole death loop
                // is the mineflayer tier's claim to make.
                death_plan = crate::deathplan::build(
                    plan,
                    &world,
                    campaign_spawn(plan),
                    stake_table.as_ref(),
                );
                crate::nav::check_stealth_zones(plan, &world)?;
                // …and the onset-survivability proof on top of them (DW0355): a
                // punishing beat must be escapable in `grace_ticks` from where the
                // player provably stands when it arms, and from every checkpoint
                // that can respawn them back into it.
                crate::nav::check_stealth_onset(plan, &world)?;
                // v0.6 trap completability proof (spec-0011, DW0342): every lethal
                // trap on the forced critical path must be avoidable, survivable
                // (`once`), or disarmable, else the party is provably killed or
                // soft-looped. Uses the move-npc waypoints (`m`) for the forced-path
                // cell set.
                crate::nav::check_traps(plan, &world, &m)?;
                // spec-0016 §2 shortcut doors (DW0373/DW0374): the long route must
                // exist while the gate is sealed, and opening the gate must
                // genuinely shorten the crossing. The critical path above was
                // already proven with every shortcut gate SEALED (Plan::build seals
                // them at step 0), so the delve is finishable the long way.
                // Refuse to place a wrong-side answer on a side the
                // geometry does not name (DW0425) — BEFORE the route proofs. A
                // door whose two sides are not even distinguishable is a
                // structural problem with the declaration, and reporting it under
                // the route proof's name (`DW0374`, "opening it must pay") would
                // send the author looking at their level layout instead.
                check_shortcut_sides(plan)?;
                // …and every click trigger must land on something (DW0426). The
                // ledger it returns is emitted below: "how many clicks did this
                // proof resolve a body for" is the one fact that distinguishes a
                // campaign whose presses all land from one that arms none.
                put_json(
                    &mut out,
                    "validation/press-bodies.json",
                    &check_trigger_bodies(plan)?.to_json(),
                );
                crate::nav::check_shortcuts(plan, &world, campaign_spawn(plan))?;
                // spec-0016 §3 ambush counterplay (DW0376): 初见杀 is legitimate,
                // a pocket with no retreat is not.
                crate::nav::check_ambushes(plan, &world, campaign_spawn(plan))?;
                // spec-0016 §4 timed gates (DW0378): a gate that punishes bad
                // timing is the point; one that punishes every timing is a slot
                // machine. At least 20% of the cycle must admit a crossing.
                crate::nav::check_timed_gates(plan, &world)?;
                // The third rung. A gate's `disarm` lever must be
                // reachable while the gate is still SHUT — a jam you can only
                // pull after surviving the crossing disables nothing (DW0393).
                crate::nav::check_timed_gate_disarms(plan, &world, campaign_spawn(plan))?;
                // spec-0016 §4 addendum — hazard observability (DW0388). The
                // dossier's strongest finding: what makes a periodic hazard fair
                // is not its ratio but that you can stand somewhere safe and
                // WATCH it before committing. Error tier for a souls campaign (it
                // declares a bonfire), warning tier otherwise.
                let unobserved =
                    crate::nav::check_hazard_observability(plan, &world, campaign_spawn(plan))?;
                // spec-0016 §7 pacing lints (DW0379 retry cost, DW0380 optional-
                // elite bypass). Warning tier: both are design judgements the
                // compiler can MEASURE but must not overrule — a long walk back
                // can be the authored point, and the owner's QA hour decides.
                pacing = crate::nav::pacing_lints(plan, &world);
                pacing.extend(unobserved);
                // Export the DW0311-proven critical-path routes as validation
                // metadata: thinned per-leg waypoint polylines the harness
                // replays as successive nearby goals, so no single giant mineflayer A*
                // solve strands the bot on a large open cave. NOT shipped gameplay —
                // lives under `validation/` (excluded from the delve image, like
                // packtest-datapack/). Emitted only when a walked leg exists, so a
                // campaign with none stays byte-identical to before. Uses the same
                // relight-aware `world` as the DW0311 check it exports.
                let routes = crate::nav::critical_path_routes(plan, &world);
                // Structural self-check: every exported waypoint must be
                // genuinely standable in this FINAL world (settled + water-flooded +
                // fixtures). Makes it impossible to ship a waypoint the game floods
                // or walls — the water-flow / post-nav-mutation divergence class —
                // failing the build loudly (DW0314) instead of stranding the bot.
                crate::nav::verify_exported_routes(&world, &routes)?;
                // Stair-orientation proof (DW0430). Nav models a stair
                // as a full cube, so a reversed stair reads as a legal one-block
                // jump and every existing proof passes — the delve ships with a
                // staircase the player must hop up tread by tread. This is the
                // one check that reads `facing`, over the same proven routes,
                // against the same assembled world.
                if !routes.is_empty() {
                    let route_cells: Vec<Vec<[i32; 3]>> =
                        routes.iter().map(|r| r.cells.clone()).collect();
                    let blocks = match &edit_replay {
                        Some(er) => er.assembled.blocks.clone(),
                        None => crate::assembled::assembled_blocks(plan, structures),
                    };
                    crate::stairs::check_stair_orientation(&blocks, Some(plan), &route_cells)?;
                }
                if !routes.is_empty() {
                    put_json(
                        &mut out,
                        "validation/critical-path-waypoints.json",
                        &crate::waypoints::waypoints_json(plan, &routes),
                    );
                }
                // Visual-tier POV cameras (spec-0003): one first-person shot per
                // corner-thinned waypoint. Self-check every eye cell is clear in
                // the FINAL assembled world (DW0724) — makes a camera looking out
                // from inside a wall a build error, the owner's exact visual-review
                // failure mode, caught at its source (the derivation).
                pov_shots = crate::render_plan::pov_shots(plan, &routes);
                let eyes: Vec<(String, [i32; 3])> = pov_shots
                    .iter()
                    .map(|s| (s.id.clone(), s.eye_cell()))
                    .collect();
                crate::nav::verify_pov_cameras(&world, &eyes)?;
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
                let realized = crate::branch::realize(plan.campaign);
                if !realized.is_empty() {
                    let flow = crate::flow::Flow::new(plan.campaign);
                    for r in &realized {
                        let Some(widx) = r.world else { continue };
                        let cp = plan
                            .branch_critical_path(&flow, &flow.playthrough_in(widx))
                            .map_err(|e| BuildFailure::Diagnostic {
                                code: e.code,
                                message: format!("branch `{}`: {}", r.branch.id, e.message),
                            })?;
                        let (region_events, ancestors) = plan.branch_gate_model(&cp);
                        let ancestor = |g: usize, s: usize| {
                            g == 0 || ancestors.get(&s).is_some_and(|a| a.contains(&g))
                        };
                        let label = |e: crate::nav::NavError| crate::nav::NavError {
                            code: e.code,
                            message: format!("branch `{}`: {}", r.branch.id, e.message),
                        };
                        crate::nav::check_branch_path(
                            &world,
                            &cp.steps,
                            &cp.transport_by_step,
                            &region_events,
                            &ancestor,
                        )
                        .map_err(label)?;
                        let branch_routes = crate::nav::branch_path_routes(
                            &world,
                            &cp.steps,
                            &cp.transport_by_step,
                            &region_events,
                            &ancestor,
                        );
                        crate::nav::verify_exported_routes(&world, &branch_routes)
                            .map_err(label)?;
                        if !branch_routes.is_empty() {
                            branch_waypoints.push((
                                r.branch.slug.clone(),
                                crate::waypoints::waypoints_json(plan, &branch_routes),
                            ));
                        }
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
                crate::clearance::check_body_clearance(plan, &world, &moves, &actor_moves)
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
                crate::traversal::check_traversal(plan, &world, &moves, &actor_moves).map_err(
                    |e| BuildFailure::Diagnostic {
                        code: e.code,
                        message: e.message,
                    },
                )?;
            warnings.extend(traversal_warnings);
            traversal_gate = Some(gate);
            // Seat each wave mob on a validated standable cell near its anchor, in
            // room only (DW0312 if the room lacks the footing) — or, for a
            // `summon: aggro-edge` wave, on its perception ring (DW0387).
            let (waves, rings) = plan_wave_spawns(plan, &world)?;
            // …and prove the sun is not going to fight the party's battle for it
            // (DW0496). Runs HERE because it needs the seated cells:
            // the question is whether open sky stands within one aggro radius of
            // where the mobs actually land, on ground they can walk to — not of
            // an anchor they stand around. The hollow-vigil gate yard is the
            // motivating case: roof and two walls carved off, noon pinned, and
            // two of three footmen dead to sunlight before the party could
            // engage them, with every other proof green.
            crate::daylight::check_daylight_staging(plan, &world, &blocks, &waves).map_err(
                |e| BuildFailure::Diagnostic {
                    code: e.code,
                    message: e.message,
                },
            )?;
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
            if crate::combat::mandatory_fights(plan).any() {
                warnings.extend(
                    crate::combat::check_winnability(plan, &world, &waves).map_err(|e| {
                        BuildFailure::Diagnostic {
                            code: e.code,
                            message: e.message,
                        }
                    })?,
                );
            }
            // The bot ladder's combat plan (spec-0023 §1/§3/§4): which
            // encounters exist, what the content bills each as, and which
            // checkpoint governs a death at it. Validation metadata only — it
            // lives under `validation/`, which `Dockerfile.delve` excludes, so
            // no shipped byte moves.
            //
            // A tier-declaring ACTOR is enough on its own to want
            // this file: the set-piece souls fight is an actor, not a wave, and
            // a campaign whose only billed elite is an actor would otherwise
            // emit no plan at all — the exact silence spec-0023's floor gate
            // must not be allowed to read as a pass. An UNTIERED hostile actor
            // is enough for the same reason and one step further
            // out: it is a fight nothing bills, so without the ledger line
            // naming it there is no artifact anywhere that says it existed.
            let tiered_actors = crate::combat::actor_encounters(plan);
            if crate::combat::has_encounters(plan)
                || !tiered_actors.is_empty()
                || crate::combat::has_untiered_hostile_actors(plan)
            {
                let mandatory = crate::combat::encounters(plan);
                warnings.extend(crate::combat::floor_coverage_warnings(
                    plan,
                    &mandatory,
                    &tiered_actors,
                ));
                put_json(
                    &mut out,
                    "validation/combat-plan.json",
                    &crate::combat::combat_plan_json(plan, &mandatory, &tiered_actors),
                );
            }
            // spec-0016 §6: resolve and prove each TD lane polyline (DW0386). The
            // proven cells are what `patrol_target` carries, so the squad is only
            // ever sent somewhere it can stand and walk to.
            let lanes = crate::nav::plan_lanes(plan, &world)?;
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
            let respawn_safety = crate::nav::check_respawn_safe_zone(plan, &world, &waves, &lanes)?;
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
            let payloads = plan_payload_verbs(plan, &world, &blocks)?;
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

    // Every campaign must resolve an ENTRY POINT (DW0345). Without one the world
    // gets no `setworldspawn`, a class-picking player is never teleported, and a
    // joining player is left to the vanilla spawn search — which a dedicated server
    // resolves to the surface but the integrated (singleplayer) server resolves to
    // the build floor, i.e. inside solid stone. This used to fail silently: an area
    // whose tileset spells the anchor `entry` instead of `spawn` compiled clean and
    // shipped a delve with no start.
    if campaign_spawn(plan).is_none() {
        return Err(BuildFailure::Diagnostic {
            code: plan::DW_NO_ENTRY_ANCHOR,
            message: format!(
                "the assembled world resolves no entry anchor — no area places a \
                 piece declaring any of {names:?} in its prefab metadata. The \
                 compiler then has no cell to call the campaign's start: no \
                 `setworldspawn`, no class-apply teleport, no first-join placement. \
                 Give the pool's entry-role prefab an entry anchor (its metadata \
                 `anchors`), or bind the area to a prefab that has one.",
                names = plan::ENTRY_ANCHOR_NAMES,
            ),
        });
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
    for area in &plan.areas {
        for template in area.pieces.iter().flat_map(|p| &p.templates) {
            if let Some(bytes) = structures.get(&template.structure_file) {
                out.insert(
                    format!("datapack/data/{ns}/structure/{}.nbt", template.structure_id),
                    bytes.clone(),
                );
            }
        }
    }

    // functions
    // Placement sentinels: one known block per distinct structure, so the
    // runtime can verify each `place template` landed (see `setup` emission).
    let mut sentinels: Sentinels = BTreeMap::new();
    for area in &plan.areas {
        for template in area.pieces.iter().flat_map(|p| &p.templates) {
            if let Some(bytes) = structures.get(&template.structure_file)
                && let Some(s) = structure_sentinel(bytes)
            {
                sentinels.insert(template.structure_file.clone(), s);
            }
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
    // lang files, puts the baked language's text on the component instead.
    let chrome = delvewright_dsl::Chrome::for_build(language);

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
    );
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
    for (name, value) in emit_advancements(plan, &chrome) {
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

    // predicates — currently only the cutscene bounce's sneak-held gate (see
    // SNEAK_HELD_PREDICATE); a cutscene-less campaign emits none.
    if campaign_has_cutscene(plan.campaign) {
        put_json(
            &mut out,
            &format!("datapack/data/{ns}/predicate/{SNEAK_HELD_PREDICATE}.json"),
            &sneak_held_predicate(),
        );
    }

    // ---- packtest datapack ----
    emit_packtest(
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
    );

    // ---- creator overlay (playtest-only; spec-0006) ----
    // A self-contained module (crate::creator). Its `.mcfunction`s are plain
    // vanilla, so they flow through the command-tree validator below and the
    // determinism gate like the main datapack; the shipped delve image excludes
    // this directory (CI-checked, same as packtest-datapack/).
    crate::creator::emit_creator(plan, &mut out, &moves, &actor_moves);

    // ---- server ----
    emit_server(plan, &mut out);

    // ---- critical path ----
    put_json(
        &mut out,
        "critical-path.json",
        &emit_critical_path(plan, &moves, &actor_moves),
    );

    // ---- visual-tier render plan (spec-0003 / spec-0007) ----
    // Deterministic camera + expect-checklist shot list for the visual tier;
    // consumed by `delve-render`. Emitted before the manifest so its hash is
    // recorded there like every other output.
    put_json(
        &mut out,
        "render-plan.json",
        &crate::render_plan::render_plan(plan, prefabs, &pov_shots),
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
    // See `crate::affordance` for the drowned-bell soft-lock this encodes.
    crate::affordance::check(&affordances(plan), &out)?;

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
    let mut fixture_gate = crate::affordance::check_fixtures(&out)?;
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

    // ---- call-graph integrity (DW0497) ----
    // Every `function <ns>:<name>` the compiler just wrote must point at a
    // function the compiler wrote. Vanilla resolves an unknown function to
    // nothing at all — no error, no log line — so an emitter whose call walk and
    // machinery walk disagree ships a verb that simply never happens. That is
    // exactly how the island's round-21 build lost two of its three storm waves
    // (see `crate::integrity`). Feature-blind and last, so it guards every
    // emitter, including ones not yet written.
    crate::integrity::check_tree(ns, &out).map_err(|e| BuildFailure::Diagnostic {
        code: e.code,
        message: e.message,
    })?;

    // ---- score-seeding integrity (DW0495) ----
    // Every `if score` / `unless score` / `scores={…}` the compiler just wrote
    // must read an entry the pack itself creates, or be written so a missing entry
    // cannot change its answer. On the pinned 1.21.11 server a score that was never
    // written is not zero — every comparison against it is false — which is how
    // `if score @s dw.deaths > @s dw.death_ack` silently swallowed every player's
    // FIRST death for as long as checkpoints have existed (see `crate::seeding`).
    // Feature-blind and read off the finished tree, beside the call-graph proof.
    crate::seeding::check_tree(ns, &out).map_err(|e| BuildFailure::Diagnostic {
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
    let art = crate::atmos::uses_art(plan.campaign);
    let mut extra_assets = if art {
        crate::atmos::art_font_assets()
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
    let resource_pack_sha1 = if skins.is_empty() && extra_assets.is_empty() {
        None
    } else {
        let zip = crate::resourcepack::build_pack(skins, &extra_assets);
        let sha1 = crate::resourcepack::sha1_hex(&zip);
        out.insert("resourcepack.zip".to_string(), zip);
        out.insert(
            "SKINS.md".to_string(),
            pack_note(&sha1, skins, art, &plan.campaign.world.content.languages).into_bytes(),
        );
        Some(sha1)
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
    out.extend(crate::branch::artifacts(plan.campaign));
    // ...and, for the harness tier, one EXECUTABLE path per reachable branch:
    // `validation/branch-path-<branch>.json`, in the same `critical-path.json`
    // contract the bot has always consumed. The plan artifact above says WHICH
    // branches exist and how a player enters them; these say what the bot walks.
    // A branch's scripted dialogue choices ride inside its own `talk-to` steps
    // (each carries the `/trigger` line of the option belonging to that branch),
    // which is the only player-legal way to actuate a server-driven dialog button.
    for (slug, path) in branch_paths(plan, &moves, &actor_moves)? {
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
    if let Some(gate) = &traversal_gate {
        put_json(&mut out, "validation/traversal-gate.json", &gate.to_json());
    }
    // The fluid-escape binding ledger (`DW0318`): the horizon the verdict was
    // stated against, the pieces and fluid cells examined, and how many cells
    // ended up outside the built volume. `None` only for a campaign that
    // assembles no world at all.
    if let Some(ledger) = &fluid_escape_ledger {
        put_json(&mut out, "validation/fluid-escape.json", ledger);
    }
    if let Some(ledger) = &gate_seal_ledger {
        put_json(&mut out, "validation/gate-seal.json", ledger);
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
        put_json(&mut out, "validation/teleport-gate.json", &gate.to_json());
    }
    if let Some(gate) = &lethal_gate {
        put_json(&mut out, "validation/lethal-gate.json", &gate.to_json());
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

    // ---- manifest (hashes of inputs + all other outputs) ----
    let manifest = emit_manifest(
        plan,
        input_bytes,
        &out,
        language,
        content_sha,
        resource_pack_sha1.as_deref(),
    );
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

/// The `assets/delvewright/lang/<mc_code>.json` assets this delve ships
/// (spec-0029 §2): `en_us.json` from the campaign's own canonical-English
/// inventory, and one file per declared language from its `l10n/<code>.json`
/// sidecar. Empty — no lang files, no behaviour change — for a campaign that
/// declares no languages, whose components' `fallback` already is the whole story.
///
/// Every file is a flat `{key: string}` map in [`BTreeMap`] order (ADR-0006), and
/// the key sets are **equal** across languages by construction: each is checked
/// against the same inventory, and a mismatch fails the build rather than shipping
/// a language with a hole in it.
/// A single-language `--lang` bake (spec-0029 §4) ships no lang files at all: its
/// strings were swapped before emission, so there is nothing for a client to
/// select between.
fn lang_assets(
    plan: &Plan,
    input_bytes: &BTreeMap<String, Vec<u8>>,
    language: Option<&str>,
) -> Result<BTreeMap<String, Vec<u8>>, BuildFailure> {
    if language.is_some_and(|l| l != delvewright_dsl::CANONICAL_LANG) {
        return Ok(BTreeMap::new());
    }
    let c = plan.campaign;
    let declared = delvewright_dsl::declared_mc_codes(c).map_err(|d| BuildFailure::Diagnostic {
        code: delvewright_dsl::codes::LANG_CODE_UNMAPPED,
        message: format!("{}: {}", d.path, d.message),
    })?;
    let mut out = BTreeMap::new();
    if declared.is_empty() {
        return Ok(out);
    }
    // The campaign reaching emission is tagged, so its inventory values are
    // translation tags; `plain` recovers the canonical English each tag carries.
    // Derived from the live inventory, never from a fixture (spec-0029 AC2).
    let english: BTreeMap<String, String> = delvewright_dsl::l10n_inventory(c)
        .into_iter()
        .map(|(k, v)| (k, plain(&v).to_string()))
        .collect();
    // Each file is the campaign's keys plus the compiler's own chrome
    // (`dsl::chrome`, spec-0029 addendum). The two key spaces are disjoint by
    // construction — chrome lives under the reserved `delvewright.` prefix, which
    // the l10n key scheme cannot produce and `DW0186` forbids a sidecar from
    // writing — so the merge can never shadow a campaign string.
    let mut put = |mc: &str, map: &BTreeMap<String, String>, chrome: BTreeMap<String, String>| {
        let mut merged = map.clone();
        merged.extend(chrome);
        let mut bytes = serde_json::to_vec_pretty(&merged).expect("lang map serializes");
        bytes.push(b'\n');
        out.insert(format!("assets/delvewright/lang/{mc}.json"), bytes);
    };
    put(
        "en_us",
        &english,
        delvewright_dsl::chrome::english_entries(),
    );

    for (lang, mc) in declared {
        let path = format!("l10n/{lang}.json");
        let Some(raw) = input_bytes.get(&path) else {
            return Err(BuildFailure::Diagnostic {
                code: delvewright_dsl::codes::L10N_MISSING,
                message: format!(
                    "declared language `{lang}` has no `{path}` among the build inputs, so the \
                     resource pack cannot carry its `assets/delvewright/lang/{mc}.json` — add \
                     the sidecar, or remove `{lang}` from `world.languages`"
                ),
            });
        };
        let doc: delvewright_dsl::L10nDoc =
            serde_json::from_slice(raw).map_err(|e| BuildFailure::Diagnostic {
                code: delvewright_dsl::codes::L10N_MISSING,
                message: format!("`{path}` is not a readable l10n sidecar: {e}"),
            })?;
        // The key sets must be EQUAL. Validation already proved it (DW0180/DW0181),
        // but the pack is where a hole becomes a player reading a raw key, so the
        // emitter proves it again over the bytes it is about to write.
        if let Some(missing) = english.keys().find(|k| !doc.content.contains_key(*k)) {
            return Err(BuildFailure::Diagnostic {
                code: delvewright_dsl::codes::L10N_MISSING,
                message: format!(
                    "`{path}` has no translation for `{missing}`, so \
                     `assets/delvewright/lang/{mc}.json` would ship a hole a `{lang}` client \
                     renders as a raw key — add `{missing}` to the sidecar"
                ),
            });
        }
        if let Some(orphan) = doc.content.keys().find(|k| !english.contains_key(*k)) {
            return Err(BuildFailure::Diagnostic {
                code: delvewright_dsl::codes::L10N_ORPHAN,
                message: format!(
                    "`{path}` carries `{orphan}`, which is not in the string inventory — remove \
                     it, so `assets/delvewright/lang/{mc}.json` and `en_us.json` carry exactly \
                     the same keys"
                ),
            });
        }
        // Chrome for a language the compiler has no table for is ABSENT rather
        // than English-under-a-translated-name: the client falls through to
        // `en_us.json` (or to the component's own fallback, for a player who
        // declined the pack) and reads English. Honest, and never disguised.
        put(mc, &doc.content, delvewright_dsl::chrome::lang_entries(mc));
    }
    Ok(out)
}

/// `DW0185`: no emitted byte may still carry a translation tag. See
/// [`DW_UNTRANSLATED_LITERAL`] for what a hit means and how to fix it.
///
/// Public so the spec-0029 test suite can drive it against a synthetic tree: the
/// only way to produce a leak from a real campaign is a defective emitter, and a
/// red that needs a defective emitter to exist is a red nobody can re-run.
pub fn check_untranslated_literals(
    out: &BuildOutput,
    pack_assets: &BTreeMap<String, Vec<u8>>,
) -> Result<(), BuildFailure> {
    // EVERY offending file, not the first: a leak is usually one emitter used from
    // several places, and a one-at-a-time diagnostic turns one fix into ten builds.
    let mut hits: Vec<String> = Vec::new();
    // The resource pack is scanned as its own assets rather than through the zip:
    // the zip embeds PNG bytes, and a byte scan of compressed/binary payloads is a
    // scan whose result depends on what a prefab happens to contain.
    for (path, bytes) in out.iter().chain(pack_assets.iter()) {
        // Classified, not guessed. A build output is either compiler-authored TEXT
        // — which this check owns — or a verbatim copy of a binary input asset,
        // which carries no authored string because the compiler never writes one
        // into it. Anything else is an output nobody has classified: it fails
        // here, so a new binary artifact cannot quietly opt out of the check.
        if is_verbatim_binary_output(path) {
            continue;
        }
        let Ok(text) = std::str::from_utf8(bytes) else {
            return Err(BuildFailure::Diagnostic {
                code: DW_UNTRANSLATED_LITERAL,
                message: format!(
                    "`{path}` is not UTF-8 text and is not a known verbatim binary output, so \
                     the untranslated-literal scan cannot read it. Classify it in \
                     `emit::is_verbatim_binary_output` (and say why in \
                     `docs/reference/compiler.md`) if it is a byte copy of an input asset"
                ),
            });
        };
        if !delvewright_dsl::has_tr_sigil(text) {
            continue;
        }
        // Name the offending key and the line, so the fix is mechanical.
        let (key, line) = text
            .lines()
            .find_map(|l| {
                let i = l.find(delvewright_dsl::TR_SIGIL)?;
                let rest = &l[i + delvewright_dsl::TR_SIGIL.len_utf8()..];
                let k = rest.split(delvewright_dsl::TR_SIGIL).next()?;
                let shown: String = l.chars().take(160).collect();
                Some((k.to_string(), shown.replace(delvewright_dsl::TR_SIGIL, "⟦")))
            })
            .unwrap_or_else(|| ("<unknown>".to_string(), String::new()));
        hits.push(format!("  {path}: `{key}` in: {line}"));
    }
    if hits.is_empty() {
        return Ok(());
    }
    let n = hits.len();
    let shown = hits.iter().take(25).cloned().collect::<Vec<_>>().join("\n");
    Err(BuildFailure::Diagnostic {
        code: DW_UNTRANSLATED_LITERAL,
        message: format!(
            "{n} emitted file(s) carry an authored player-visible string outside a text \
             component, so it would ship as a literal no client can translate. Lower each \
             through `emit::tr` / `emit::snbt_component` (which emit \
             `{{\"translate\":\"<key>\",\"fallback\":\"<English>\"}}`); if the site is genuinely \
             not a component and not read by a player — a manifest field, a reviewer \
             chronicle, `critical-path.json`, a generated PackTest source — read the string \
             through `dsl::l10n::plain` and add the site to the named-exclusion table in \
             `docs/reference/compiler.md`.\n{shown}"
        ),
    })
}

/// Whether a build output is a **verbatim copy of a binary input asset** rather
/// than compiler-authored text: a prefab structure `.nbt`, an NPC-skin PNG, the
/// art-font atlas, and the resource-pack zip that packages them (whose own
/// compiler-authored members — the lang files, the font provider — are scanned
/// separately, before zipping). The compiler writes no authored string into any of
/// these, so they are outside the `DW0185` scan; everything else must be readable
/// text, and a new output that is neither fails the scan rather than skipping it.
fn is_verbatim_binary_output(path: &str) -> bool {
    path.ends_with(".nbt") || path.ends_with(".png") || path == "resourcepack.zip"
}

/// The `SKINS.md` build-output note: how the packaging task wires the emitted
/// resource pack into the delve image (itzg env), plus the pack SHA-1. The pack
/// carries the mannequin NPC skins (spec-0009) and/or the `delve:art` title font
/// (spec-0014), depending on what the campaign uses.
fn pack_note(
    sha1: &str,
    skins: &BTreeMap<String, Vec<u8>>,
    art: bool,
    languages: &[String],
) -> String {
    let mut s = String::new();
    s.push_str("# Delve resource pack\n\n");
    s.push_str(
        "This delve ships a server resource pack (`resourcepack.zip`). The packaging\n\
         task serves it and sets the itzg env so vanilla clients receive it:\n\n",
    );
    s.push_str(&format!(
        "- `RESOURCE_PACK` = the URL the delve serves `resourcepack.zip` at\n\
         - `RESOURCE_PACK_SHA1` = `{sha1}`\n\
         - `RESOURCE_PACK_PROMPT` = a JSON text component (not a bare string)\n\n",
    ));
    if !skins.is_empty() {
        s.push_str(
            "Baked skins (`skins/<id>.png` → `assets/delvewright/textures/npc/<id>.png`):\n\n",
        );
        for id in skins.keys() {
            s.push_str(&format!("- `{id}`\n"));
        }
        s.push('\n');
    }
    if art {
        s.push_str(
            "Art-title font (spec-0014): `delve:art` — an original 5x7 pixel bitmap\n\
             font at `assets/delve/font/art.json` (+ `assets/delve/textures/font/art.png`),\n\
             used by `narrate` `style: art`.\n\n",
        );
    }
    // The pack is the LANGUAGE CARRIER now (spec-0029), not optional dressing, and
    // the person wiring it up is the one who needs to know what declining it costs.
    // Host-facing prose only — no key scheme, no pipeline (CLAUDE.md audience
    // separation).
    if !languages.is_empty() {
        s.push_str("Languages: this delve's in-game text ships in English plus ");
        s.push_str(&languages.join(", "));
        s.push_str(
            ".\nA player's own client language is used automatically; anything else\n\
             reads English. A player who DECLINES the resource-pack prompt reads\n\
             English too, and the delve is fully playable that way — the pack adds\n\
             the other languages, it is never required to finish the delve.\n",
        );
    }
    s
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

// ---------------------------------------------------------------------------
// mcfunction emission
// ---------------------------------------------------------------------------

/// The environment-sealing baseline (spec-0002 "Environment sealing").
///
/// **1.21.11 gamerule syntax — verified live against a pinned 1.21.11 server
/// (delvewright-base / itzg VANILLA), 2026-07-30.** 1.21.11 replaced the legacy
/// camelCase gamerule identifiers with a registry of snake_case names, several of
/// them renamed outright; the old spellings are rejected with "Incorrect argument
/// for command". The confirmed successors used here:
///
/// | Legacy (spec text)   | 1.21.11 accepted                       |
/// |----------------------|----------------------------------------|
/// | `doMobSpawning`      | `spawn_mobs` (umbrella natural-spawn)   |
/// | `doDaylightCycle`    | `advance_time`                          |
/// | `doWeatherCycle`     | `advance_weather`                       |
/// | `doFireTick`         | `fire_spread_radius_around_player` (int)|
/// | `mobGriefing`        | `mob_griefing`                          |
/// | `spawnRadius`        | `respawn_radius` (spawn scatter, int)   |
///
/// `doFireTick` has **no boolean successor**; 1.21.11 models fire spread as an
/// integer radius around players, so `0` disables it (the sealing intent: no
/// spreading fire). Time is pinned to the declared world `time` (DSL v0.5,
/// spec-0010; default noon = daytime 6000, the v0 default — so a campaign that
/// declares nothing is byte-identical). With `advance_time`/`advance_weather`
/// frozen, the set states persist for the whole delve. A `weather` command is
/// emitted only when the campaign declares one (clear is the vanilla default, so
/// omitting it keeps pre-v0.5 output byte-identical). Names may optionally be
/// `minecraft:`-prefixed on the server, but the bare form is accepted and matches
/// the vendored command tree (`data/commands-1.21.11.json`), so it is what we
/// emit and validate.
/// The campaign's **declared** combat difficulty (`world.difficulty`, v0.6), or
/// `None` when it declares none and the compiler's historical derivation applies.
///
/// Gated on the world stage's `dsl_version` exactly as the rest of the v0.6 world
/// surface is: validation already rejects the field on an older stage
/// (`DW0141`), and honouring the gate here too means no build path can
/// accidentally read a field the campaign is not entitled to declare.
fn declared_difficulty(c: &delvewright_dsl::Campaign) -> Option<delvewright_dsl::WorldDifficulty> {
    if is_v06(c.world.dsl_version.as_str()) {
        c.world.content.difficulty
    } else {
        None
    }
}

fn sealing_commands(
    time: delvewright_dsl::WorldTime,
    weather: Option<delvewright_dsl::WorldWeather>,
    difficulty: Option<delvewright_dsl::WorldDifficulty>,
    v06: bool,
) -> Vec<String> {
    let mut cmds = vec![
        "gamerule spawn_mobs false".to_string(),
        "gamerule advance_time false".to_string(),
        "gamerule advance_weather false".to_string(),
        "gamerule fire_spread_radius_around_player 0".to_string(),
        "gamerule mob_griefing false".to_string(),
        // Spawn scatter OFF. Vanilla scatters a first join / spawnpoint-less
        // respawn uniformly in a square of this radius around world spawn; in a box
        // garden every scattered cell is solid prefab (or void), so the only
        // correct radius is 0 — the exact anchor the compiler chose. 1.21.11
        // renamed the legacy `spawnRadius` to `respawn_radius` (the legacy spelling
        // is rejected outright); verified against the vendored 1.21.11 command tree
        // (`data/commands-1.21.11.json`, which is what the compiler's own command
        // validator checks every emitted line against).
        "gamerule respawn_radius 0".to_string(),
        // Box-garden death policy: dying must never cost quest items (a dropped
        // trial key despawns in 5 minutes = softlock for a human player).
        "gamerule keep_inventory true".to_string(),
        // The delve's own machinery must not narrate itself. Dialogue options are
        // `trigger`-type objectives (`dw.dlg_<npc>`, `dw.class`), so every option a
        // player picks runs `/trigger` and vanilla answers it in chat — "Triggered
        // [dw.dlg_antiphos]" beside the line the character just said. Command
        // feedback is engine implementation reaching
        // the player, which is what every other rule in this list exists to stop.
        // NOT version-gated: a campaign at any
        // `dsl_version` wants its dialogue to stop announcing its scoreboard.
        // rcon replies to the caller regardless of this rule, so the harness and
        // `validation/` are unaffected, and the creator overlay's log stamp is
        // `log_admin_commands`, a different rule. (The legacy camelCase spelling is
        // rejected outright by 1.21.11 — the compiler's own command validator caught
        // `sendCommandFeedback` here before it could reach a world.)
        "gamerule send_command_feedback false".to_string(),
        format!("time set {}", time.token()),
    ];
    // Traps (DSL v0.6, spec-0011) exclude TNT as a payload — no gamerule separates
    // explosion *block* damage from *entity* damage, so a TNT trap would deform the
    // sealed jigsaw world and poison every downstream proof. `tnt_explodes false` is
    // the defense-in-depth seal against a stray primed-TNT source (e.g. a dispenser
    // loaded with TNT the schema forbids anyway). Gated on the v0.6 world stage so
    // pre-0.6 fixtures stay byte-identical.
    if v06 {
        cmds.push("gamerule tnt_explodes false".to_string());
    }
    // Weather is emitted only when explicitly declared (spec-0010): clear is the
    // vanilla default, so a campaign that declares no weather emits no `weather`
    // command and stays byte-identical to pre-v0.5 output.
    if let Some(w) = weather {
        cmds.push(format!("weather {}", w.token()));
    }
    // Declared combat difficulty (v0.6). The shipped
    // `server/server.properties` already carries it, so this line is not what
    // makes the delve image correct — it is what makes the DATAPACK correct
    // wherever else it is loaded (the owner's own test save, a PackTest world
    // whose properties someone edited). `/difficulty` is idempotent — re-running
    // it with the current value is a no-op that merely reports "did not change" —
    // and it is emitted only when the field is declared, so a campaign that
    // declares none is byte-identical.
    if let Some(diff) = difficulty {
        cmds.push(format!("difficulty {}", diff.token()));
    }
    cmds
}

/// Yaw for a facing keyword (MC: yaw 0 = +z/south).
pub(crate) fn facing_yaw(facing: Option<&str>) -> i32 {
    match facing {
        Some("north") => 180,
        Some("east") => 270,
        Some("west") => 90,
        _ => 0, // south / default
    }
}

/// Whether this campaign compiles under DSL v0.3 (gate for every M2
/// presentation fix). The gate is the quests-stage version (all v0.3 surface
/// lives in stage 5) — matching [`crate::registry`]/validation. v0.2 campaigns
/// (hello-world / keep-crawl) take the untouched pre-v0.3 emission path, keeping
/// their output byte-identical.
fn campaign_is_v03(plan: &Plan) -> bool {
    is_v03(plan.campaign.quests.dsl_version.as_str())
}

/// True for DSL v0.4+ campaigns. Gates the dialogue objective-state display axis
/// (a `completes` option is hidden until its objective is active) so pre-v0.4
/// campaigns stay byte-identical.
fn campaign_is_v04(plan: &Plan) -> bool {
    is_v04(plan.campaign.quests.dsl_version.as_str())
}

/// Escape a player-facing string as a double-quoted SNBT string. On 1.21.11
/// `CustomName` is a **text component**, so a bare quoted SNBT string is read as
/// literal text (the JSON-string form `'{"text":"…"}'` renders verbatim, incl. in
/// death messages — the M2 defect). Only `\` and `"` need escaping inside SNBT.
fn snbt_string(s: &str) -> String {
    let esc = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{esc}\"")
}

/// The `CustomName:…,CustomNameVisible:1b,` NBT fragment (trailing comma) that
/// labels a floating objective marker with its objective `title`. When the
/// objective has no title, the marker carries NO name — an empty fragment — so it
/// still glows and is findable but never surfaces the raw objective id (e.g.
/// `obj/door`) as player-visible floating text — presentation hygiene. A titled
/// marker carries its title verbatim.
fn marker_name_fields(title: Option<&str>) -> String {
    match title {
        Some(t) => format!("CustomName:{},CustomNameVisible:1b,", snbt_component(t)),
        None => String::new(),
    }
}

/// A text-component SNBT **compound** for a player-visible string:
/// `{text:"<escaped>"}`. Used for mannequin `description` (DSL v0.4) and any
/// component-form NBT field. This is deliberately NOT the stringified-JSON form
/// `'{"text":…}'`, which 1.21.11 renders as literal raw JSON above an entity's
/// head (owner-verified). The generated summons carry no `'{"text"` substring.
fn snbt_text_component(s: &str) -> String {
    match delvewright_dsl::l10n_untag(s) {
        Some((key, english)) => snbt_translate(key, english),
        None => format!("{{text:{}}}", snbt_string(s)),
    }
}

// ---------------------------------------------------------------------------
// i18n v2 — authored strings become translatable components (spec-0029)
// ---------------------------------------------------------------------------

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
pub const DW_UNTRANSLATED_LITERAL: DwCode = DwCode::every_version("DW0185");

/// Lower an authored player-visible string into a JSON **text component**
/// (spec-0029 §1): a translation-tagged string becomes
/// `{"translate": "<l10n key>", "fallback": "<English source>"}`, anything else
/// (a compiler-baked literal such as the default boundary message) stays
/// `{"text": …}`.
///
/// The `fallback` rides on the component rather than on the pack's own
/// `en_us.json` deliberately: a player who **declines** the resource-pack prompt
/// has no lang files at all, and the delve must still be playable in English
/// (spec-0029 §3).
fn tr(s: &str) -> Value {
    match delvewright_dsl::l10n_untag(s) {
        Some((key, english)) => json!({ "translate": key, "fallback": english }),
        None => json!({ "text": s }),
    }
}

/// [`tr`] with extra component fields (`color`, `bold`, `italic`, `font`, …)
/// merged in. Styling is orthogonal to whether the body is a literal or a
/// translate key, so every styled site keeps its styling verbatim.
fn tr_with(s: &str, fields: &[(&str, Value)]) -> Value {
    let mut v = tr(s);
    let obj = v.as_object_mut().expect("tr() builds an object");
    for (k, val) in fields {
        obj.insert((*k).to_string(), val.clone());
    }
    v
}

/// The **SNBT** form of [`tr`], for a text component living in an NBT field
/// (`CustomName`, a mannequin `description`, an item `custom_name`). Emitted as an
/// SNBT compound, never the stringified-JSON form, for the same reason
/// [`snbt_text_component`] always was: 1.21.11 renders `'{"text":…}'` above an
/// entity's head verbatim.
fn snbt_component(s: &str) -> String {
    match delvewright_dsl::l10n_untag(s) {
        Some((key, english)) => snbt_translate(key, english),
        // An untagged string keeps the bare quoted-string component form 1.21.11
        // already read it as, so a compiler-baked name stays byte-for-byte what it
        // was before spec-0029.
        None => snbt_string(s),
    }
}

/// The SNBT `{fallback:…,translate:…}` compound both SNBT component forms share.
/// Field order is alphabetical, matching the JSON components' `BTreeMap` order so
/// the two forms read the same in a diff.
fn snbt_translate(key: &str, english: &str) -> String {
    format!(
        "{{fallback:{},translate:{}}}",
        snbt_string(english),
        snbt_string(key)
    )
}

/// The human string behind an authored value, for the **named exclusions**: sites
/// that are not text components and are not read by a player. Re-exported here so
/// every exclusion in `emit` is greppable as `plain(`.
fn plain(s: &str) -> &str {
    delvewright_dsl::l10n_plain(s)
}

/// The delve title for a **compiler artifact**, not for a player: the generated
/// PackTest sources' `#>` descriptions and the reviewer render plan. These are
/// not text components and no client ever renders them, so they carry the English
/// source string rather than a translate key — a named exclusion, listed as such
/// in `docs/reference/compiler.md`.
fn artifact_title(c: &delvewright_dsl::Campaign) -> &str {
    plain(&c.world.content.title)
}

/// The `,components:{…}` SNBT tail carrying an equipped piece's enchantments,
/// or `""` when it has none (which is what keeps every pre-enchantment campaign
/// byte-identical).
///
/// 1.21 moved enchantments onto the **item component**
/// `minecraft:enchantments`, whose value is a map of enchantment id → level.
/// Emission order is the `BTreeMap`'s id order, never hash order (ADR-0006).
fn enchantment_components(piece: &EquipItem) -> String {
    enchantment_component_tail(piece.enchantments())
}

/// The shared `,components:{"minecraft:enchantments":{…}}` renderer — one
/// implementation for equipped gear and for container loot, so the two cannot
/// disagree about the component's shape.
fn enchantment_component_tail(ench: &std::collections::BTreeMap<String, u32>) -> String {
    if ench.is_empty() {
        return String::new();
    }
    let body = ench
        .iter()
        .map(|(id, lvl)| format!("\"{id}\":{lvl}"))
        .collect::<Vec<_>>()
        .join(",");
    format!(",components:{{\"minecraft:enchantments\":{{{body}}}}}")
}

/// The default main-hand weapon for a summoned mob whose natural spawns are
/// armed, or `None` for mobs that spawn unarmed. Small static table (documented
/// in the compiler README); mobs not listed (zombie, drowned — a wild trident is
/// not a default) get nothing.
/// The pillager entry is load-bearing beyond looks (spec-0016 §6): a pillager's
/// only attack goal is the crossbow goal, so an unarmed one that acquires a
/// target has nothing runnable to do while its patrol goal is blocked by that
/// same target — it freezes in place indefinitely (live-verified on 1.21.11,
/// `docs/notes/td-routing-spike.md`). Arming it by default takes that deadlock
/// off the author's plate entirely; `DW0384` catches the one remaining way in (an
/// explicit `main_hand` override that takes the crossbow away).
fn default_mainhand(entity: &str) -> Option<&'static str> {
    match entity.strip_prefix("minecraft:").unwrap_or(entity) {
        "wither_skeleton" => Some("minecraft:stone_sword"),
        "skeleton" | "stray" => Some("minecraft:bow"),
        "pillager" => Some("minecraft:crossbow"),
        "vindicator" => Some("minecraft:iron_axe"),
        _ => None,
    }
}

/// The main-hand item a wave mob **actually spawns holding**: the v0.6
/// `equipment.main_hand` override when the author gave one, else the
/// armed-mob default table ([`default_mainhand`]).
///
/// This is the single source of truth for "what is in this mob's hand" and
/// must be used by anything that *describes* the emitted summon — notably the
/// generated `verb_kill` PackTest arming assertion. Reading the default table
/// there instead produced a self-contradicting datapack: the summon gave the
/// override (the-drowned-bell's vindicators carry `minecraft:stone_axe`) while
/// the generated test asserted the default (`minecraft:iron_axe`), so the suite
/// failed on a real server for a campaign that was in fact correct.
fn effective_mainhand<'a>(entity: &str, eq: Option<&'a MobEquipment>) -> Option<&'a str> {
    eq.and_then(|e| e.main_hand.as_ref())
        .map(|p| p.item())
        .or_else(|| default_mainhand(entity))
}

/// Default hand equipment for a summoned mob whose natural spawns are armed
/// (M2 fix 5). `/summon` gives no equipment, so a wither-skeleton boss spawned
/// unarmed was trivial. Returns an SNBT fragment (no leading comma) setting the
/// `equipment` component with a zero `drop_chances`, or `None` for unarmed mobs.
///
/// **Component-era form, not legacy `HandItems` (M2 round-2 fix 1).** Minecraft
/// 1.21.11 silently ignores `HandItems`/`HandDropChances` on `/summon` NBT — a
/// `data get entity … HandItems` after summon returns nothing and the mob is
/// bare-handed. The accepted form is the entity `equipment`/`drop_chances`
/// components: proven live via rcon (`equipment:{mainhand:{id:"minecraft:
/// stone_sword",count:1}},drop_chances:{mainhand:0.0f}` → `data get entity …
/// equipment.mainhand` returns the item; the legacy form yields "Found no
/// elements matching equipment"). The legacy form failed *silently* for a whole
/// milestone because nothing looked — the generated `verb_kill` PackTest now
/// asserts the armed mob actually holds its weapon so a regression can't hide.
fn default_equipment(entity: &str) -> Option<String> {
    default_mainhand(entity).map(|item| {
        format!("equipment:{{mainhand:{{id:\"{item}\",count:1}}}},drop_chances:{{mainhand:0.0f}}")
    })
}

/// The drop chance a **declared** drop puts on its slot (DSL v0.9).
///
/// Not `1.0`. Vanilla's `DropChances` record (pinned 1.21.11 client jar, class
/// `cgi`) has exactly two named operations here, and they say what the numbers
/// mean:
///
/// * `withGuaranteedDrop(slot)` writes the constant `2.0f` — verified in the
///   jar's bytecode (`fconst_2`), and the same value the vanilla
///   `SaddleEquipmentSlotFix` datafixer writes for a saddle a horse always
///   drops;
/// * `isPreserved(slot)` is `chance > 1.0f`.
///
/// `Mob.dropCustomDeathLoot` (class `chn`) reads both: a slot whose chance is
/// exactly `0.0f` is skipped outright; a **preserved** slot drops even when the
/// killing blow was not a player's, and — the reason `1.0f` is wrong — it skips
/// the durability randomization that a chance of `≤ 1.0` applies to a damageable
/// item. A boss axe declared as a drop must be *the* axe, not a die-roll of its
/// remaining durability: `2.0f` is the vanilla primitive for "always, unchanged",
/// and it is the only value that makes a declared drop deterministic.
const DECLARED_DROP_CHANCE: &str = "2.0f";

/// The drop chance every UNDECLARED slot keeps — today's behaviour, unchanged,
/// which is what keeps every pre-0.9 campaign byte-identical.
const NO_DROP_CHANCE: &str = "0.0f";

/// The vanilla NBT slot keys a `drops[]` list marks as guaranteed, for one
/// entity. Quest-item entries carry no slot and are absent from the set — they
/// ride the death loot table instead ([`drop_loot_table`]).
fn declared_drop_slots(drops: &[delvewright_dsl::MobDrop]) -> BTreeSet<&'static str> {
    drops
        .iter()
        .filter_map(|d| d.slot())
        .map(|s| s.nbt())
        .collect()
}

/// The chance string for `slot`, given the entity's declared drops.
fn drop_chance_for(slot: &str, declared: &BTreeSet<&'static str>) -> &'static str {
    if declared.contains(slot) {
        DECLARED_DROP_CHANCE
    } else {
        NO_DROP_CHANCE
    }
}

/// The datapack path (namespace-local) of the death loot table a declared
/// quest-item drop rides on, for one actor / one wave-mob stack.
///
/// **Why a loot table and not another equipment slot.** The `equipment` /
/// `drop_chances` compounds address the six worn slots and nothing else — a
/// quest token the fight *yields* has no slot, and hanging it in an off-hand the
/// author never dressed would be the downstream workaround the no-hack rule
/// forbids. 1.21.11 answers the slot-less half with its own primitive: `Mob`
/// (jar class `chn`) reads `DeathLootTable` (and `DeathLootTableSeed`) straight
/// off summon NBT, through the `ResourceKey<LootTable>` codec, and
/// `LivingEntity.dropAllDeathLoot` rolls it on death. The compiler already
/// writes `DeathLootTable:"minecraft:empty"` on every actor; a declared drop
/// simply points the same field at a table the compiler emits, with the item
/// entry the author declared. One roll, one entry, no RNG (ADR-0006).
fn drop_loot_path(kind: &str, id: &str) -> String {
    format!("dw_drop/{kind}_{}", plan::safe_local(id))
}

/// The `DeathLootTable` NBT value for an entity: the emitted table when it
/// declares a quest-item drop, else the `minecraft:empty` every actor has always
/// carried (byte-identity for every pre-0.9 campaign).
fn death_loot_table(ns: &str, path: Option<String>) -> String {
    match path {
        Some(p) => format!("{ns}:{p}"),
        None => "minecraft:empty".to_string(),
    }
}

/// True if this drop list contains a quest-item entry (the half that needs a
/// death loot table).
fn has_item_drop(drops: &[delvewright_dsl::MobDrop]) -> bool {
    drops.iter().any(|d| d.item().is_some())
}

/// Strip a declared drop off a body the **compiler** is about to remove.
///
/// The invariant, stated once: a declared drop is what a *player's kill* yields.
/// Every removal the compiler performs itself — the `unleash` that swaps a
/// puppet for its twin, a `despawn-actor` (either style), a souls re-seat's
/// re-caging — goes through `/kill`, and vanilla `/kill` is an ordinary death:
/// a preserved slot (chance > 1.0) drops **even when the killer is not a
/// player**. Without this line an elite would shed its axe every time the story
/// moved it, and a re-seat would turn the boss into a vending machine.
///
/// Two intended vanilla primitives, composed: `execute as … run data merge
/// entity @s` (single-entity by construction, which is what `data merge`
/// requires) writing drop chance 0 on every slot and an empty death loot table.
/// Emitted only for an actor that declares drops, so every earlier campaign's
/// removal is byte-identical.
fn strip_drops_line(tag: &str) -> String {
    format!(
        "execute as @e[tag={tag}] run data merge entity @s {{drop_chances:{{mainhand:{z},offhand:{z},head:{z},chest:{z},legs:{z},feet:{z}}},DeathLootTable:\"minecraft:empty\"}}",
        z = NO_DROP_CHANCE
    )
}

/// The `equipment`/`drop_chances` SNBT fragment for a wave mob (no leading
/// comma), or `None` for a bare-handed mob. A mob without the v0.6 `equipment`
/// field takes the [`default_equipment`] path **unchanged** (byte-identity for
/// pre-equipment waves). With the field, explicit slots merge over the
/// armed-mob main-hand default (an explicit `main_hand` overrides it — a
/// helmeted skeleton keeps its bow). Every slot the v0.9 `drops[]` list does not
/// name carries drop chance 0: players must never farm wave gear (no-grind
/// constitution); a named slot carries [`DECLARED_DROP_CHANCE`]. Component-era
/// form only — see [`default_equipment`] for why legacy `ArmorItems`/
/// `HandItems` are silently ignored by 1.21.11 `/summon`. Slot order is fixed
/// (mainhand, offhand, head, chest, legs, feet) for ADR-0006 determinism.
fn wave_equipment(
    entity: &str,
    eq: Option<&MobEquipment>,
    drops: &[delvewright_dsl::MobDrop],
) -> Option<String> {
    let declared = declared_drop_slots(drops);
    let mainhand = effective_mainhand(entity, eq);
    let Some(eq) = eq else {
        return default_equipment(entity);
    };
    // The main-hand slot is the one place a DEFAULT (a bare id, no enchantments)
    // can stand in for an authored piece, so it carries an id plus an optional
    // authored piece; the other five are authored or absent.
    let slots: [(&str, Option<&str>, Option<&EquipItem>); 6] = [
        ("mainhand", mainhand, eq.main_hand.as_ref()),
        (
            "offhand",
            eq.off_hand.as_ref().map(EquipItem::item),
            eq.off_hand.as_ref(),
        ),
        (
            "head",
            eq.head.as_ref().map(EquipItem::item),
            eq.head.as_ref(),
        ),
        (
            "chest",
            eq.chest.as_ref().map(EquipItem::item),
            eq.chest.as_ref(),
        ),
        (
            "legs",
            eq.legs.as_ref().map(EquipItem::item),
            eq.legs.as_ref(),
        ),
        (
            "feet",
            eq.feet.as_ref().map(EquipItem::item),
            eq.feet.as_ref(),
        ),
    ];
    let mut items: Vec<String> = Vec::new();
    let mut chances: Vec<String> = Vec::new();
    for (slot, item, piece) in slots {
        if let Some(it) = item {
            let comps = piece.map(enchantment_components).unwrap_or_default();
            items.push(format!("{slot}:{{id:\"{it}\",count:1{comps}}}"));
            chances.push(format!("{slot}:{}", drop_chance_for(slot, &declared)));
        }
    }
    if items.is_empty() {
        return None;
    }
    Some(format!(
        "equipment:{{{}}},drop_chances:{{{}}}",
        items.join(","),
        chances.join(",")
    ))
}

/// The `,attributes:[…]` SNBT fragment (leading comma) for a wave mob's v0.4
/// attribute overrides, or `""` when none are set. Each present field becomes a
/// `{id:"minecraft:<attr>",base:<double>}` entry; doubles are formatted with a
/// decimal point so SNBT reads them as doubles (ADR-0006 determinism).
fn attributes_snbt(attrs: Option<&delvewright_dsl::MobAttributes>) -> String {
    wrap_attribute_entries(attribute_entries(attrs))
}

/// The individual `{id:…,base:…}` entries for a [`MobAttributes`] block, in the
/// fixed schema order. Split out of [`attributes_snbt`] so the paths that add a
/// compiler-owned attribute of their own (the `vulnerable` actor's
/// knockback-immunity) can concatenate rather than fork the table — the DSL
/// exposes ONE attribute surface and there is one place that renders it.
///
/// [`MobAttributes`]: delvewright_dsl::MobAttributes
fn attribute_entries(attrs: Option<&delvewright_dsl::MobAttributes>) -> Vec<String> {
    let mut entries: Vec<String> = Vec::new();
    let Some(a) = attrs else {
        return entries;
    };
    let mut add = |id: &str, v: Option<f64>| {
        if let Some(x) = v {
            entries.push(format!("{{id:\"minecraft:{id}\",base:{}}}", fmt_f64(x)));
        }
    };
    add("max_health", a.max_health);
    add("attack_damage", a.attack_damage);
    add("movement_speed", a.movement_speed);
    add("follow_range", a.follow_range);
    entries
}

/// Wrap rendered attribute entries as the `,attributes:[…]` SNBT fragment
/// (leading comma), or `""` when there are none.
fn wrap_attribute_entries(entries: Vec<String>) -> String {
    if entries.is_empty() {
        String::new()
    } else {
        format!(",attributes:[{}]", entries.join(","))
    }
}

/// Format an `f64` deterministically for SNBT with a guaranteed decimal point
/// (so `20` renders as `20.0`, read as a double). Uses `{:?}` (shortest
/// round-trip) which is stable across platforms.
fn fmt_f64(x: f64) -> String {
    let s = format!("{x:?}");
    if s.contains('.') || s.contains('e') || s.contains("inf") || s.contains("NaN") {
        s
    } else {
        format!("{s}.0")
    }
}

/// The world position an entity is summoned at, formatted per axis: the horizontal
/// **centre** of the cell, on its floor ([`crate::nav::cell_center`]).
///
/// A block cell `(x, y, z)` spans `[x, x+1)`, and an entity's position is the centre
/// of its AABB — so summoning at the bare integer cell parks the body on the corner
/// where four columns meet, with most of it inside the neighbouring columns (a
/// 0.6-wide villager at `x = 7.0` occupies `[6.7, 7.3]`). Against a wall that reads
/// as an NPC standing inside the wall; along a walked path it is the owner's
/// "visibly passes through blocks" defect. Every entity the compiler places or
/// moves goes through this conversion.
///
/// Block-targeting commands (`setblock`, `fill`, `place`, `spawnpoint`) keep the
/// integer cell — that is the coordinate space they take.
fn ent_xyz(c: [i32; 3]) -> [String; 3] {
    let p = crate::nav::cell_center(c);
    [fmt_f64(p[0]), fmt_f64(p[1]), fmt_f64(p[2])]
}

/// Every `(chunk_x, chunk_z)` an inclusive block AABB covers, in ascending
/// order. Chunk coordinates use vanilla's floor division (negative block coords
/// belong to the chunk below, not toward zero — the fifth-level piece
/// straddling chunk `z=-1` that motivated the placement retry loop lives here).
fn chunk_span(min: [i32; 3], max: [i32; 3]) -> Vec<(i32, i32)> {
    let (x0, x1) = (min[0].div_euclid(16), max[0].div_euclid(16));
    let (z0, z1) = (min[2].div_euclid(16), max[2].div_euclid(16));
    (x0..=x1)
        .flat_map(|cx| (z0..=z1).map(move |cz| (cx, cz)))
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn emit_functions(
    plan: &Plan,
    chrome: &delvewright_dsl::Chrome,
    sentinels: &Sentinels,
    moves: &[crate::nav::MovePlan],
    actor_moves: &[crate::nav::ActorMovePlan],
    relight: &[crate::light::Placement],
    wave_placements: &WavePlacements,
    lane_routes: &crate::nav::LaneRoutes,
    world_edits: &[String],
    edit_bounds: &[([i32; 3], [i32; 3])],
    trap_gates: &BTreeMap<String, String>,
    payloads: &PayloadPlans,
    branch_transport: &BranchTransportOverlay,
    stake_table: Option<&crate::stake::StakeTable>,
) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let v03 = campaign_is_v03(plan);
    // spec-0020: every NPC's cast ledger resolved into the scenes right-click
    // swaps between. Empty for a campaign that declares no `cast`.
    let casts = crate::cast::npc_casts(c);
    let mut fns: Vec<(String, String)> = Vec::new();

    // --- load ---
    fns.push((
        "load".to_string(),
        lines(&[
            "scoreboard objectives add dw.sys dummy".to_string(),
            format!("execute unless score #init dw.sys matches 1 run function {ns}:setup"),
        ]),
    ));

    // --- setup ---
    let mut setup: Vec<String> = Vec::new();
    // Environment sealing (spec-0002): a delve is a box garden — every dynamic is
    // authored, nothing is left to vanilla chance. Emitted first, once, guarded by
    // the same `#init` flag as the rest of setup.
    setup.push(
        "# Environment sealing (spec-0002): box garden — nothing left to vanilla chance."
            .to_string(),
    );
    setup.extend(sealing_commands(
        c.world.content.time.unwrap_or_default(),
        c.world.content.weather,
        declared_difficulty(c),
        is_v06(c.world.dsl_version.as_str()),
    ));
    setup.push("scoreboard objectives add dw.class trigger".to_string());
    setup.push("scoreboard objectives add dw.classed dummy".to_string());
    setup.push("scoreboard objectives add dw.dlg_shown dummy".to_string());
    // spec-0016 §1: the bonfire's two-option answer
    // channel. `dw.rest` is a *trigger* because a dialog button runs its command
    // as the clicking player, and `/trigger` is the one command a non-operator
    // player may run. Absent for a campaign with no bonfire → byte-identical.
    if plan.bonfires().next().is_some() {
        setup.push("scoreboard objectives add dw.rest trigger".to_string());
        setup.push("scoreboard objectives add dw.rest_at dummy".to_string());
    }
    for npc in &plan.npcs {
        setup.push(format!(
            "scoreboard objectives add {} trigger",
            npc.trigger_objective
        ));
    }
    for q in &c.quests.content.quests {
        for o in &q.objectives {
            setup.push(format!(
                "scoreboard objectives add {} dummy",
                obj_score(o.id().as_str())
            ));
        }
    }
    for q in &c.quests.content.quests {
        setup.push(format!(
            "scoreboard objectives add {} dummy",
            quest_active_score(q.id.as_str())
        ));
        setup.push(format!(
            "scoreboard objectives add {} dummy",
            quest_score(q.id.as_str())
        ));
    }
    // The completion objective. It is NOT put on the sidebar: a `setdisplay
    // sidebar dw.campaign` slot would show players a permanent raw internal id
    // (`dw.campaign`), and it serves no purpose — the validation bot observes
    // completion via the anchored `[dw:complete …]` chat channel (markers.ts),
    // never the sidebar (mineflayer 4.37.x cannot decode 1.21.11 score packets).
    setup.push("scoreboard objectives add dw.campaign dummy".to_string());
    // v0.3: the shared wave countdown, per-flag scores, and interact triggers.
    // Each loop is empty for a v0.2 campaign, so hello-world / keep-crawl setup is
    // byte-identical.
    if !c.quests.content.waves.is_empty() {
        setup.push(format!(
            "scoreboard objectives add {} dummy",
            plan::WAVE_OBJECTIVE
        ));
    }
    for flag in declared_flags(c) {
        setup.push(format!(
            "scoreboard objectives add {} dummy",
            plan::flag_score(&flag)
        ));
    }
    // DSL v0.10 runtime state (spec-0031): one objective per declared datum, and
    // the `party`-scoped ones seeded to their declared initials right here —
    // `setup` runs once, at world init, which is exactly a party datum's
    // lifetime. `player`-scoped data cannot be seeded here (no player exists
    // yet); they are seeded on each player's first tick (`state_seed`). The loop
    // is empty for every pre-0.10 campaign, so their setup is byte-identical.
    for st in declared_states(c) {
        setup.push(format!(
            "scoreboard objectives add {} dummy",
            plan::state_score(st.id.as_str())
        ));
    }
    for st in declared_states(c) {
        if st.scope == StateScope::Party {
            setup.push(format!(
                "scoreboard players set {} {} {}",
                plan::PARTY,
                plan::state_score(st.id.as_str()),
                st.initial
            ));
        }
    }
    // spec-0032: a named datum's shadow score — the value it was last announced at.
    // Party-scoped ones are seeded here beside the datum itself, so a world that
    // has just loaded announces nothing.
    for st in named_states(plan) {
        setup.push(format!(
            "scoreboard objectives add {} dummy",
            state_shadow_score(st.id.as_str())
        ));
        if st.scope == StateScope::Party {
            setup.push(format!(
                "scoreboard players set {} {} {}",
                plan::PARTY,
                state_shadow_score(st.id.as_str()),
                st.initial
            ));
        }
    }
    // spec-0032: the economy's objectives and constants. Empty for a campaign that
    // declares neither a shop nor a stake → byte-identical.
    setup.extend(economy_setup(plan));
    // v0.4: the per-player scratch bitmask used by display-gated dialogue choosers
    // (flag axis and/or objective-state axis). Declared only when a gated option
    // exists, so v0.2/v0.3 setup is unchanged.
    if has_gated_dialogue(c) {
        setup.push("scoreboard objectives add dw.dmask dummy".to_string());
    }
    // spec-0020: the per-player cast-scene selector. Declared only when some
    // quest casts an NPC, so a pre-0.7 campaign's setup is unchanged.
    if !casts.is_empty() {
        setup.push(format!("scoreboard objectives add {CAST_SCORE} dummy"));
    }
    for (oid, _) in interact_objectives(c) {
        setup.push(format!(
            "scoreboard objectives add {} trigger",
            plan::interact_trigger(&oid)
        ));
    }
    // v0.3 objective-activation feedback (M2 fix 4): one "announced" flag per
    // titled objective. Empty for a v0.2 campaign, so hello-world / keep-crawl
    // setup stays byte-identical.
    if v03 {
        for q in &c.quests.content.quests {
            for o in &q.objectives {
                if o.title().is_some() {
                    setup.push(format!(
                        "scoreboard objectives add {} dummy",
                        announce_score(o.id().as_str())
                    ));
                }
            }
        }
    }
    // v0.3 collect held-count scratch (gap 13): the per-tick "already holding the
    // item" completion check stores each player's held count here before comparing
    // it to the required count. Declared only when a `collect` objective exists, so
    // a v0.2 campaign (and any v0.3 campaign without collect) stays byte-identical.
    if v03 && has_collect_objective(c) {
        setup.push(format!("scoreboard objectives add {COLLECT_HOLD} dummy"));
    }
    // v0.6 checkpoints (spec-0012): the active-checkpoint marker + the vanilla
    // `deathCount` respawn-detection scores. Emitted for EVERY campaign that
    // declares a checkpoint — the marker now also drives the respawn **re-seat**
    // as well as the `on_respawn` dispatch. Pre-0.6 / checkpoint-free
    // campaigns emit nothing here.
    if plan.any_checkpoint() {
        setup.push("scoreboard players set #cp dw.sys -1".to_string());
        setup.push("scoreboard objectives add dw.deaths deathCount".to_string());
        setup.push("scoreboard objectives add dw.death_ack dummy".to_string());
    } else if !plan.on_death().is_empty() {
        // v0.10 `on_death` (spec-0031) rides the SAME detector, so a campaign that
        // declares a death beat and no checkpoint still needs `deathCount` — but
        // not the checkpoint marker or the respawn-side ack, which nothing would
        // read. A campaign with a checkpoint already declared `dw.deaths` above.
        setup.push("scoreboard objectives add dw.deaths deathCount".to_string());
    }
    // The corpse-side acknowledgement (spec-0031). Separate from `dw.death_ack`
    // because that one is deliberately WITHHELD while the player is dead, which is
    // exactly the window `on_death` fires in. Absent — like this whole branch —
    // for a campaign that declares no `on_death`, so pre-0.10 emission is
    // byte-identical.
    if !plan.on_death().is_empty() {
        setup.push("scoreboard objectives add dw.death_seen dummy".to_string());
    }
    // v0.6 stealth beats (spec-0014; no sneak requirement): the
    // active-session marker + per-player grace scores. Hidden =
    // inside a declared zone — no sneak stat is tracked. Declared only when the
    // campaign uses `begin-stealth`.
    if !plan.stealth_beats.is_empty() {
        setup.push("scoreboard players set #stealth dw.sys 0".to_string());
        setup.push("scoreboard objectives add dw.st_grace dummy".to_string());
        setup.push("scoreboard objectives add dw.st_safe dummy".to_string());
    }
    // Force-load the chunks covering each prefab. `forceload add` only MARKS
    // chunks; freshly-generated far chunks (found live: a fifth-level piece
    // straddling chunk z=-1) are not reliably loaded within the same tick, so
    // `place template` can silently no-op with zero log output. Placement is
    // therefore NOT done here: setup only seals + forceloads, and the tick
    // function retries `place_all` + `place_verify` (sentinel-block checks)
    // until every piece is confirmed, then runs `setup_finish` exactly once.
    for area in &plan.areas {
        for piece in &area.pieces {
            let (min, max) = piece.bbox();
            setup.push(format!(
                "forceload add {} {} {} {}",
                min[0], min[2], max[0], max[2]
            ));
        }
    }
    // Stage-7 edit writes may land outside the piece bboxes (a leaning canopy,
    // a fragment stamped beside a piece) — forceload each batch's write AABB
    // too, or the `world_edits` setblocks would silently fail on unloaded
    // chunks (the same pitfall the piece forceloads exist for). Empty for a
    // campaign without an edit script → setup byte-identical.
    for (min, max) in edit_bounds {
        setup.push(format!(
            "forceload add {} {} {} {}",
            min[0], min[2], max[0], max[2]
        ));
    }
    setup.push("scoreboard players set #placed dw.sys 0".to_string());

    // The edit-script chunk ledger (map-editor audit, findings 2 + 6): which
    // chunks the `world_edits` writes need loaded, and which of those the piece
    // forceloads do NOT already cover. `forceload add` only MARKS a chunk — the
    // very reason placement is retried — so `world_edits` needs the same
    // load-convergence gate (`place_verify` below) and, being one-shot, may
    // release its own chunks afterwards.
    let piece_chunks: BTreeSet<(i32, i32)> = plan
        .areas
        .iter()
        .flat_map(|a| a.pieces.iter())
        .flat_map(|p| {
            let (min, max) = p.bbox();
            chunk_span(min, max)
        })
        .collect();
    // Chunk → a representative block cell inside BOTH the chunk and the edit
    // AABB (`execute if loaded` takes a block pos). Deterministic: `BTreeMap`
    // keyed on the chunk coordinate, first AABB to reach a chunk wins.
    let mut edit_chunks: BTreeMap<(i32, i32), [i32; 3]> = BTreeMap::new();
    for (min, max) in edit_bounds {
        for (cx, cz) in chunk_span(*min, *max) {
            edit_chunks.entry((cx, cz)).or_insert([
                (cx * 16).max(min[0]),
                min[1],
                (cz * 16).max(min[2]),
            ]);
        }
    }

    // --- place_all: idempotent template placement, retried from tick ---
    let mut place_all: Vec<String> = Vec::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let rot = match piece.rotation.token() {
                Some(t) => format!(" {t}"),
                None => String::new(),
            };
            // One command per TEMPLATE, not per piece: a zone past the vanilla
            // 48-per-axis cap ships as several of them and is placed as several
            // of them, at the world positions the plan already resolved. A
            // single-template piece has exactly one, at the piece's own
            // position, so its line is byte-identical to before.
            for template in &piece.templates {
                place_all.push(format!(
                    "place template {ns}:{} {} {} {}{rot}",
                    template.structure_id, template.pos[0], template.pos[1], template.pos[2]
                ));
            }
        }
    }
    fns.push(("place_all".to_string(), lines(&place_all)));

    // --- place_verify: sentinel check per piece + per edit chunk; all present
    // → setup_finish ---
    let mut place_verify: Vec<String> = Vec::new();
    place_verify.push("scoreboard players set #placeok dw.sys 0".to_string());
    let mut sentinel_count = 0u32;
    // Edit-script chunks that no piece bbox covers get their OWN convergence
    // sentinel (map-editor audit finding 2). Without it `setup_finish` could
    // fire the moment the pieces verify, run `world_edits` into a still-loading
    // chunk, and lose those writes permanently — vanilla `setblock` into an
    // unloaded chunk fails with no output, and `world_edits` runs exactly once.
    // Folding them into `#placeok` reuses the placement retry loop verbatim:
    // the tick function re-runs `place_verify` until every sentinel AND every
    // edit chunk reports in. Empty for a campaign whose edits stay inside the
    // pieces → `place_verify` byte-identical.
    for ((cx, cz), cell) in &edit_chunks {
        if piece_chunks.contains(&(*cx, *cz)) {
            continue;
        }
        place_verify.push(format!(
            "execute if loaded {} {} {} run scoreboard players add #placeok dw.sys 1",
            cell[0], cell[1], cell[2]
        ));
        sentinel_count += 1;
    }
    for area in &plan.areas {
        // One sentinel per TEMPLATE. A `place template` can fail for one tile
        // of a zone and land for the rest — a chunk that has not loaded yet is
        // exactly how that happens — so a per-piece sentinel would report a
        // zone placed when eight ninths of it was there.
        for (piece, template) in area
            .pieces
            .iter()
            .flat_map(|p| p.templates.iter().map(move |t| (p, t)))
        {
            if let Some((local, block)) = sentinels.get(&template.structure_file) {
                let w = piece.rotation.transform(*local);
                let (sx, sy, sz) = (
                    template.pos[0] + w[0],
                    template.pos[1] + w[1],
                    template.pos[2] + w[2],
                );
                place_verify.push(format!(
                    "execute if block {sx} {sy} {sz} {block} run scoreboard players add #placeok dw.sys 1"
                ));
                sentinel_count += 1;
            }
        }
    }
    place_verify.push(format!(
        "execute if score #placeok dw.sys matches {sentinel_count} run function {ns}:setup_finish"
    ));
    fns.push(("place_verify".to_string(), lines(&place_verify)));

    // --- setup_finish: everything that must run on real placed structures ---
    let mut setup = {
        let finished_setup = setup;
        fns.push(("setup".to_string(), {
            let mut s = finished_setup;
            s.push("scoreboard players set #init dw.sys 1".to_string());
            lines(&s)
        }));
        Vec::<String>::new()
    };
    // seal/clear sockets: open sockets get a wall fill; mated sockets get their
    // jigsaw block cleared to air, leaving a clean 3×3 passage (keep-socket-v1).
    // Runs after placement so it overwrites the raw structure blocks. Empty for
    // an area whose prefab declares no connector — including every single-prefab
    // area binding one, whose lone piece has all its sockets unmated and so gets
    // a wall fill per connector.
    for area in &plan.areas {
        for seal in &area.seals {
            setup.push(format!(
                "fill {} {} {} {} {} {} {}",
                seal.from[0],
                seal.from[1],
                seal.from[2],
                seal.to[0],
                seal.to[1],
                seal.to[2],
                seal.block
            ));
        }
    }
    // Stage-7 world edits (spec-0017): the edit script's runtime materialization,
    // applied after the socket seals and before the relight fixtures — the exact
    // order the compile-time model replayed them in (the relight pass measured
    // the EDITED world, so its fixtures must land after the edits). One function
    // call keeps setup_finish readable; the coalesced `fill`/`setblock` body
    // lives in `world_edits.mcfunction`. Empty for a campaign without an edit
    // script → setup_finish byte-identical to pre-stage-7.
    if !world_edits.is_empty() {
        setup.push(format!("function {ns}:world_edits"));
        fns.push(("world_edits".to_string(), lines(world_edits)));
    }
    // Relight fixtures (spec-0010): supplemental lighting placed after the world is
    // fully assembled (structures placed + sockets sealed), so the block writes
    // land on real geometry — the intended vanilla mechanism (consistent with v0.4
    // `set-block`). Emitted in deterministic pass order. Empty for a campaign with
    // no `lighting` declaration → setup_finish byte-identical.
    for p in relight {
        setup.push(format!(
            "setblock {} {} {} {}",
            p.pos[0], p.pos[1], p.pos[2], p.block
        ));
    }
    // Summon NPCs (body + interaction hitbox) at world init. A `deferred: true`
    // stage-2 NPC (DSL v0.6) is skipped here — it enters the world only when a
    // `spawn-npc` effect fires `spawn_npc_<id>`, which runs the very same commands
    // (`npc_summon_commands`), so a staged character is not a statue standing at
    // its mark from minute one.
    for npc in &plan.npcs {
        if npc_is_deferred(c, &npc.npc_id) {
            continue;
        }
        setup.extend(npc_summon_commands(c, plan, npc, v03));
    }
    // v0.3 collect chests, interact hitboxes/markers and reach markers are NOT
    // placed here. They are placed/summoned when their objective ACTIVATES (see the
    // activation drivers in `tick` + the `activate_<obj>` functions below), so props
    // and loot for late objectives are neither visible nor lootable from minute one,
    // and a `collect` item picked up before activation can no longer stall the
    // objective (gap 13). Empty for v0.2 campaigns (byte-identity preserved: they
    // have no collect/interact objectives, and reach markers were always v0.3-only).
    // Set world spawn to the first area's `spawn` anchor so joining players land
    // on the prefab floor instead of falling through the void world before class
    // selection teleports them.
    if let Some(pos) = campaign_spawn(plan) {
        setup.push(format!("setworldspawn {} {} {}", pos[0], pos[1], pos[2]));
        // Initialize the `dw:cp` last-checkpoint storage mirror to the spawn cell.
        // Shared contract with spec-0012 checkpoints (its `set-checkpoint` updates
        // the same `dw:cp pos`); spec-0013's boundary return reads it. The write is
        // idempotent (`set value`), and `needs_cp_init` is the single gate so the
        // two features land in either merge order without double-emitting.
        if needs_cp_init(plan) {
            setup.push(format!(
                "data modify storage dw:cp pos set value [{}, {}, {}]",
                pos[0], pos[1], pos[2]
            ));
        }
    }
    // v0.6 boundary (spec-0013): write the readable region mirror (`dw:region`,
    // analogous to `dw:cp`) and start the per-second return clock. Both lines are
    // deterministic (bounds derived from the final layout); empty for a campaign
    // with no `boundary`, so non-boundary output stays byte-identical.
    if let Some(region) = playable_region(plan) {
        setup.push(format!(
            "data modify storage dw:region bounds set value {}",
            region.bounds_snbt()
        ));
        setup.push(format!("schedule function {ns}:boundary_tick 20t"));
    }
    // v0.6 night-vision mitigation: start the per-second `effect give` clock for the
    // areas that declare it. Empty otherwise → byte-identical.
    if has_night_vision_areas(plan) {
        setup.push(format!(
            "schedule function {ns}:night_vision_tick {NIGHT_VISION_PERIOD_TICKS}t"
        ));
    }
    // v0.4: summon the interaction entities strike/use environment triggers watch
    // (empty for a campaign with no triggers → byte-identical).
    setup.extend(env_trigger_setup(plan, chrome));
    // v0.6: fill each trap dispenser payload and summon disarm affordances
    // (spec-0011). Empty for a campaign with no traps → byte-identical.
    setup.extend(trap_setup(plan, trap_gates));
    // spec-0021: fill each declared container. Empty for a campaign with no
    // `loot` -> byte-identical.
    setup.extend(loot_setup(&plan.loot));
    // spec-0016 §2: summon each shortcut's far-side unlock affordance. The gate
    // itself needs no command — it is sealed from world-load by the prefab.
    setup.extend(shortcut_setup(plan));
    // spec-0016 §4: start each timed gate's clock. The gate is sealed from
    // world-load by the prefab, so the clock's first act is always an OPEN.
    setup.extend(timed_gate_setup(plan));
    // spec-0032: arm each shop's interaction point and its visible marker. A shop
    // is furniture, so it is armed at world init exactly as a shortcut's lever is.
    setup.extend(shop_setup(plan));
    // Forceload lifecycle (map-editor audit finding 6, planner decision). The
    // edit-AABB forceloads exist for ONE reason — letting the one-shot
    // `world_edits` writes land — and `place_verify` above has now proven every
    // one of those chunks loaded. Release the ones no piece bbox covers, at the
    // very END of `setup_finish` so every other write in this function (relight
    // fixtures, NPC summons, trap hardware) has already run against loaded
    // chunks. The PIECE forceloads are deliberately untouched: the gameplay tick
    // machinery (gate fills, wave spawns, checkpoint and trap block reads) keeps
    // addressing those chunks for the whole session. Empty for a campaign whose
    // edits stay inside the pieces → `setup_finish` byte-identical.
    for ((cx, cz), cell) in &edit_chunks {
        if piece_chunks.contains(&(*cx, *cz)) {
            continue;
        }
        setup.push(format!("forceload remove {} {}", cell[0], cell[2]));
    }
    setup.push("scoreboard players set #placed dw.sys 1".to_string());
    fns.push(("setup_finish".to_string(), lines(&setup)));

    // --- tick ---
    let mut tick: Vec<String> = Vec::new();
    // Placement retry loop: until every sentinel verifies, re-place and re-check
    // each tick (idempotent; `setup_finish` fires exactly once, gated by
    // `#placed`). Converges as soon as the forceloaded chunks finish loading.
    tick.push(format!(
        "execute if score #init dw.sys matches 1 unless score #placed dw.sys matches 1 run function {ns}:place_all"
    ));
    tick.push(format!(
        "execute if score #init dw.sys matches 1 unless score #placed dw.sys matches 1 run function {ns}:place_verify"
    ));
    // Datapack-owned FIRST-JOIN placement (singleplayer parity). A joining player
    // is placed by the datapack, never by the server's interpretation of the
    // level.dat spawn: the integrated (singleplayer) server does not reliably
    // honour the emitted spawn state and drops the first join at the superflat
    // floor (x/z of world spawn, y = build-floor) — inside stone, unescapable
    // except by dying. A dedicated server places the same world correctly, so no
    // rung of the validation ladder can ever observe this. Gated on `#placed` so
    // the teleport lands on real geometry (the structures are placed over the
    // first ticks), and on the per-player `dw_joined` tag so it fires exactly once
    // per player — a relog keeps the tag and therefore the player's position, and
    // RESPAWN is untouched (that is `spawnpoint @a` + the checkpoint machinery).
    // Empty for a campaign with no `spawn` anchor → byte-identical.
    if campaign_spawn(plan).is_some() {
        tick.push(format!(
            "execute if score #placed dw.sys matches 1 as @a[tag=!dw_joined] run function {ns}:join_place"
        ));
    }
    // A player who disconnects mid-cutscene keeps `dw_cutscene` and spectator
    // across the relog, but `cs_end_<bare>` is `@a`-scoped and already ran without
    // them: they rejoin as a ghost, in a world they can fly through and not touch,
    // with no way back. `join_place` cannot help — it is gated on `dw_joined`,
    // which a relog also keeps. So the repair is its own tick clause, keyed on the
    // stuck state itself. Empty for a cutscene-less campaign → byte-identical.
    tick.extend(cutscene_repair_tick(plan));
    // The class trigger is ONE-SHOT per player. `class_apply_<c>` ends in a
    // teleport to the campaign entry point, so re-firing `/trigger dw.class`
    // mid-run would warp whoever ran it back to the start of the delve — an
    // already-classed player included, if this line were to `enable @a`
    // unconditionally, every tick,
    // forever. The vanilla trigger pattern is to re-enable only what is meant to
    // be usable, so the arming is per-player and conditional; the guard
    // lives inside `class_arm` rather than in this line so a PackTest can drive
    // the real arming path as its own dummy instead of mirroring it.
    //
    // Per-PLAYER, not party-wide: classing is per-player (`dw.classed`), so a
    // second player still on the class screen must keep an armed trigger while
    // the first is sealed.
    // DSL v0.10 runtime state (spec-0031): seed each player's `player`-scoped
    // data to their declared initials, once, on their first tick. `setup` cannot
    // do it — no player exists at world init — and the tag lives in player data,
    // so a relog does not re-seed and a datum survives a disconnect exactly as a
    // scoreboard score does. Emitted only when the campaign declares a
    // `player`-scoped datum, so every pre-0.10 tick is byte-identical.
    if declared_states(c)
        .iter()
        .any(|st| st.scope == StateScope::Player)
    {
        tick.push(format!(
            "execute as @a[tag=!{}] run function {ns}:state_seed",
            plan::STATE_SEEDED_TAG
        ));
    }
    tick.push(format!("execute as @a run function {ns}:class_arm"));
    for npc in &plan.npcs {
        tick.push(format!(
            "scoreboard players enable @a {}",
            npc.trigger_objective
        ));
    }
    // v0.3: interact triggers are enabled so the bot's `/trigger` (and re-tries)
    // work, matching the dialog trigger pattern. Empty for v0.2 campaigns.
    for (oid, _) in interact_objectives(c) {
        tick.push(format!(
            "scoreboard players enable @a {}",
            plan::interact_trigger(&oid)
        ));
    }
    // The lobby (spec-0018 `world.min_players`). A design that genuinely needs n
    // players declares it, and the delve refuses to START below n: the class
    // dialog stays shut and the waiting players get a live party-count actionbar.
    // Emitted only for `min_players >= 2`, so every 1-player campaign — i.e. every
    // pre-0.6 one — stays byte-identical here.
    let min_players = plan::min_players(c);
    let lobby_open = if min_players >= 2 {
        tick.push(format!(
            "execute store result score {LOBBY_COUNT} dw.sys if entity @a"
        ));
        tick.push(format!(
            "execute if score {LOBBY_COUNT} dw.sys matches ..{} as @a unless score @s dw.classed matches 1 run title @s actionbar {}",
            min_players - 1,
            lobby_actionbar(min_players, chrome)
        ));
        format!("if score {LOBBY_COUNT} dw.sys matches {min_players}.. ")
    } else {
        String::new()
    };
    tick.push(with_execute_prefix(
        &lobby_open,
        format!(
            "execute as @a unless score @s dw.classed matches 1 unless score @s dw.dlg_shown matches 1 run function {ns}:show_class"
        ),
    ));
    for class in &plan.classes {
        // The second seal. The arming above is what makes the trigger
        // unusable after a class; this makes any score that arrives by some
        // OTHER route inert rather than a warp. Costs one condition and closes
        // the dispatch as well as the door.
        tick.push(format!(
            "execute as @a[scores={{dw.class={}}}] unless score @s dw.classed matches 1 run function {ns}:class_apply_{}",
            class.n, class.safe
        ));
    }
    for npc in &plan.npcs {
        for opt in &npc.options {
            tick.push(format!(
                "execute as @a[scores={{{}={}}}] run function {ns}:dlg_{}_{}",
                npc.trigger_objective, opt.n, npc.safe, opt.n
            ));
        }
    }
    // v0.3 objective-activation feedback (M2 fix 4): announce a titled objective
    // the tick it becomes active (quest active, `after`/flags satisfied, not yet
    // complete) and has not been announced. Runs before the completion checks so
    // "new objective" precedes any same-tick "complete". Empty for v0.2.
    //
    // spec-0018: the whole predicate is party state now — the guard and the
    // announce-once latch both read `#party` — so the driver needs no player
    // context at all and the announce reaches the party exactly once.
    if v03 {
        for q in &c.quests.content.quests {
            let qa = quest_active_score(q.id.as_str());
            for o in &q.objectives {
                if o.title().is_some() {
                    tick.push(format!(
                        "execute{} unless score {} {} matches 1 run function {ns}:announce_{}",
                        pending_guard(plan, o, &qa),
                        plan::PARTY,
                        announce_score(o.id().as_str()),
                        safe_obj_fn(o.id().as_str())
                    ));
                }
            }
        }
    }
    // v0.3 activation-time placement (gap 13): place a `collect` chest, summon an
    // `interact` hitbox + marker, or summon a `reach` marker the tick the objective
    // ACTIVATES (same edge the announce uses), not at world setup — so late props
    // are neither visible nor lootable early. Global-once per objective, guarded by
    // a `#act_<obj>` sentinel on dw.sys, so a second player activating does not
    // re-place an already-looted chest. Empty for v0.2.
    if v03 {
        for q in &c.quests.content.quests {
            let area = plan.quest_area(q.id.as_str()).unwrap_or("");
            let qa = quest_active_score(q.id.as_str());
            for o in &q.objectives {
                if activation_commands(plan, area, o).is_empty() {
                    continue;
                }
                tick.push(format!(
                    "execute{} unless score {} dw.sys matches 1 run function {ns}:activate_{}",
                    pending_guard(plan, o, &qa),
                    activation_flag(o.id().as_str()),
                    safe_obj_fn(o.id().as_str())
                ));
            }
        }
    }
    // Per-tick objective completion checks. `reach-anchor` (proximity) is
    // unchanged for v0.2; `kill` (wave countdown reached zero) and `interact`
    // (trigger fired + optional item) are v0.3 additions. `collect` completes via
    // its `inventory_changed` advancement AND (v0.3) a per-tick held check that
    // closes the pre-activation-pickup stall (gap 13).
    //
    // ## The arming-before-adjudication invariant
    //
    // This is the ONE loop whose lines can ARM a quest: a completion line runs
    // `complete_<obj>` → `check_q_<quest>` → `complete_q_<quest>`, and that last
    // function writes `#party dw.qa_<next>` for every quest triggered by this
    // one's completion. Every other quest gate in the tick only READS those
    // scores.
    //
    // So the loop must visit an arming quest before the quest it arms, and the
    // guarantee has to be STRUCTURAL rather than a property declaration order
    // happens to have. What goes wrong otherwise is silent and costs a player
    // their click: an `interact` adjudicates under `if score #party dw.qa_<q>
    // matches 1` and then resets the trigger UNCONDITIONALLY on the next line, so
    // a click already pending when its quest is armed later in the same tick is
    // consumed with no effect. A human clicks again and never knows; a validation
    // bot clicks once and times out.
    //
    // The reset stays unconditional on purpose (owner ruling): a trigger fired
    // long before its quest was armed is DISCARDED, never banked. Banking would
    // auto-complete the objective the instant the quest armed, with no real click
    // — a worse failure than the one it would fix, because it fabricates player
    // input rather than losing it.
    //
    // `quests_in_arming_order` is a stable topological sort, so a campaign whose
    // quests are already declared in arming order — every campaign built so far —
    // emits byte-identically.
    for q in quests_in_arming_order(c) {
        let area = plan.quest_area(q.id.as_str()).unwrap_or("");
        let qa = quest_active_score(q.id.as_str());
        for o in &q.objectives {
            match o {
                Objective::ReachAnchor {
                    id, anchor, radius, ..
                } => {
                    let pos = match plan
                        .anchors
                        .get(&(area.to_string(), anchor.as_str().to_string()))
                    {
                        Some(ResolvedAnchor::Point { pos, .. }) => *pos,
                        Some(ResolvedAnchor::Gate { from, .. }) => *from,
                        None => continue,
                    };
                    // v0.3 (M2 fix 8): a point-radius `distance=..R` sphere was too
                    // tight for a human standing on the altar cell. Test a block
                    // region instead — the anchor cell with ±1 generosity on every
                    // axis (a 3×3×3 box centred on the anchor). v0.2 keeps the
                    // sphere so hello-world / keep-crawl stay byte-identical.
                    if v03 {
                        tick.push(format!(
                            "execute as @a{} if entity @s[x={},dx=2,y={},dy=2,z={},dz=2] run function {ns}:complete_{}",
                            pending_guard(plan, o, &qa),
                            pos[0] - 1, pos[1] - 1, pos[2] - 1,
                            safe_obj_fn(id.as_str())
                        ));
                    } else {
                        tick.push(format!(
                            "execute as @a{} if entity @s[x={},y={},z={},distance=..{}] run function {ns}:complete_{}",
                            pending_guard(plan, o, &qa),
                            pos[0], pos[1], pos[2], radius,
                            safe_obj_fn(id.as_str())
                        ));
                    }
                }
                Objective::Kill { id, wave, .. } => {
                    tick.push(format!(
                        "execute as @a{} if score {} {} matches ..0 run function {ns}:complete_{}",
                        pending_guard(plan, o, &qa),
                        plan::wave_counter(wave.as_str()),
                        plan::WAVE_OBJECTIVE,
                        safe_obj_fn(id.as_str())
                    ));
                }
                Objective::Interact {
                    id,
                    requires_item,
                    missing_item_hint,
                    ..
                } => {
                    let trigger = plan::interact_trigger(id.as_str());
                    // `requires_item` means HELD, not possessed: presenting the
                    // item is the action, so the gate
                    // reads the main hand (`weapon.mainhand`), not the whole
                    // inventory (`container.*`). An inventory-wide reading fires
                    // every gated interaction the moment the item is picked up
                    // anywhere — a player who right-clicks a sleeping giant with a
                    // sharpened stake in their backpack would blind it without ever
                    // raising a hand. (`collect`'s hold check below still reads
                    // `container.*`: that one genuinely counts an inventory.)
                    let item_guard = match requires_item {
                        Some(it) => format!(" if items entity @s weapon.mainhand {it}"),
                        None => String::new(),
                    };
                    // The trigger is set by the bot's chat command or the
                    // interaction advancement's reward; the guard applies uniformly.
                    tick.push(format!(
                        "execute as @a[scores={{{trigger}=1..}}]{}{item_guard} run function {ns}:complete_{}",
                        pending_guard(plan, o, &qa),
                        safe_obj_fn(id.as_str())
                    ));
                    // v0.7: the empty-hand answer. A click that reaches an OPEN
                    // interaction without the item in hand used to be met with pure
                    // silence, which reads as a broken affordance; an authored
                    // `missing_item_hint` narrates it to that player instead. Same
                    // activation guard as the completion line above (so an inactive
                    // or already-finished objective stays quiet) plus the negation
                    // of the item guard — and it sits BEFORE the trigger reset, in
                    // the same tick that consumes the click record, so one click
                    // yields exactly one line. Ordering against the completion line
                    // is immaterial (the two conditions are mutually exclusive) but
                    // is kept adjacent for readability. Absent field emits nothing.
                    if let (Some(it), Some(hint)) = (requires_item, missing_item_hint) {
                        tick.push(format!(
                            "execute as @a[scores={{{trigger}=1..}}]{} unless items entity @s weapon.mainhand {it} run tellraw @s {}",
                            pending_guard(plan, o, &qa),
                            tr(hint)
                        ));
                    }
                    // Reset the trigger every tick so a gated attempt can be retried
                    // (e.g. clicked the door before holding the key).
                    tick.push(format!(
                        "execute as @a[scores={{{trigger}=1..}}] run scoreboard players reset @s {trigger}"
                    ));
                }
                Objective::Collect {
                    id, item, count, ..
                } if v03 => {
                    // Complete for a player already holding the item (gap 13): a
                    // `collect` normally completes via an `inventory_changed`
                    // advancement whose reward revokes-to-re-arm, and that will NOT
                    // re-fire while the item is merely held — so an item pocketed
                    // before the objective activated could leave it stuck open. This
                    // per-tick held check closes it: store the held count, then
                    // complete once the guards hold and the player carries >= the
                    // required count — whether the item was taken before or after
                    // activation. `store result … if items` captures the total
                    // matching item count across the inventory.
                    tick.push(format!(
                        "execute as @a{} store result score @s {COLLECT_HOLD} if items entity @s container.* {item}",
                        pending_guard(plan, o, &qa)
                    ));
                    tick.push(format!(
                        "execute as @a{} if score @s {COLLECT_HOLD} matches {count}.. run function {ns}:complete_{}",
                        pending_guard(plan, o, &qa),
                        safe_obj_fn(id.as_str())
                    ));
                }
                Objective::TalkTo { .. } | Objective::Collect { .. } => {}
            }
        }
    }
    // v0.4: environment-trigger per-tick checks (empty for a campaign with no
    // triggers → byte-identical).
    tick.extend(env_trigger_tick(plan, chrome));
    // v0.6: trap disarm-affordance detection (spec-0011). Empty for a campaign with
    // no disarmable traps → byte-identical.
    tick.extend(trap_tick(plan));
    // spec-0016 §1: bonfire rest detection. Empty for a campaign with no bonfire
    // → byte-identical.
    tick.extend(bonfire_tick(plan));
    // spec-0016 §2: shortcut unlock detection. Empty without a shortcut →
    // byte-identical.
    tick.extend(shortcut_tick(plan));
    // Timed-gate disarm detection. Empty without a jammable gate →
    // byte-identical.
    tick.extend(timed_gate_tick(plan));
    // v0.6 checkpoints (spec-0012): per-player death detection via the vanilla
    // `deathCount` criterion — the respawn re-seat and the active
    // checkpoint's `on_respawn`. Since spec-0031 the same one detector also drives
    // the campaign's `on_death` beat on the CORPSE side of the same edge, so a
    // campaign with a death beat and no checkpoint arms the identical line. There
    // is no second detector, and this is the only place the whole delve asks
    // whether anyone has died.
    if plan.any_checkpoint() || !plan.on_death().is_empty() {
        tick.push(format!("execute as @a run function {ns}:cp_respawn_check"));
    }
    // spec-0031: lethal volumes. One driver line per declared volume; empty for a
    // campaign that declares none → byte-identical.
    tick.extend(lethal_tick(plan));
    // v0.6 stealth (spec-0014): while a beat is active, run its per-tick judge.
    for beat in &plan.stealth_beats {
        tick.push(format!(
            "execute if score #stealth dw.sys matches {} run function {ns}:stealth_tick_{}",
            beat.index, beat.index
        ));
    }
    // spec-0032: the shop answer channel and the stake marker collector. LAST, and
    // deliberately so — see `economy_tick`: a stake dropped by `on_death` earlier in
    // this same tick must have written its slot before the collector counts
    // references, or the marker would be deleted the instant it appeared.
    // spec-0032: a named datum announces its new balance whenever it changes, from
    // ANY cause. Before the economy dispatch, so a purchase made in this tick is
    // announced in the next one rather than being missed entirely.
    tick.extend(named_state_tick(plan));
    tick.extend(economy_tick(plan));
    fns.push(("tick".to_string(), lines(&tick)));

    // --- v0.6 checkpoint respawn dispatch (spec-0012) ---
    fns.extend(emit_checkpoint_functions(plan));
    // --- spec-0016 §1 bonfire rest functions ---
    fns.extend(emit_bonfire_functions(plan));
    // --- spec-0016 §2 shortcut unlock functions ---
    fns.extend(emit_shortcut_functions(plan));
    // --- The clickable body of each sealed shortcut door ---
    // Empty for a campaign with no shortcut → byte-identical output.
    fns.extend(ws_arm_fns(plan, chrome));
    // --- spec-0016 §4 timed-gate clock functions ---
    fns.extend(emit_timed_gate_functions(plan));
    // --- v0.6 stealth-beat functions (spec-0014) ---
    fns.extend(emit_stealth_functions(plan));
    // --- spec-0031 lethal-volume functions ---
    fns.extend(emit_lethal_functions(plan));
    // --- spec-0032 trade and recovery-stake functions ---
    fns.extend(emit_shop_functions(plan));
    fns.extend(emit_stake_functions(plan, stake_table));
    fns.extend(emit_named_state_functions(plan));

    // --- cs_repair: rejoin-after-cutscene repair (see the `tick` driver above) ---
    fns.extend(cutscene_repair_fns(plan));

    // --- join_place: first-join placement (see the `tick` driver above) ---
    //
    // The target is the campaign ENTRY POINT (the first area's `spawn` anchor),
    // not the live `dw:cp` checkpoint. `dw:cp` is *seeded* to this very cell at
    // setup, so the two agree at world start; they diverge only after a checkpoint
    // fires, and at that point a first-joining player is a player who has not
    // played yet — the entry point is where the campaign begins, and it is exactly
    // where `class_apply_*` teleports every player when they pick a class. Reading
    // `dw:cp` would also need a macro function (the mirror is a `[x, y, z]` list,
    // not tp-shaped arguments) for no behavioural gain.
    if let Some(pos) = campaign_spawn(plan) {
        fns.push((
            "join_place".to_string(),
            lines(&[
                format!("teleport @s {} {} {}", pos[0], pos[1], pos[2]),
                "tag @s add dw_joined".to_string(),
            ]),
        ));
    }

    // --- state_seed: per-player runtime-state initials (DSL v0.10) ---
    //
    // Run `as` each player who has not been seeded yet (see the `tick` driver).
    // The tag goes on LAST: a crash between two writes leaves the player unseeded
    // and the next tick redoes the whole block, so a partially-seeded player is
    // not a state this can reach.
    if declared_states(plan.campaign)
        .iter()
        .any(|st| st.scope == StateScope::Player)
    {
        let mut body: Vec<String> = Vec::new();
        for st in declared_states(plan.campaign) {
            if st.scope == StateScope::Player {
                body.push(format!(
                    "scoreboard players set @s {} {}",
                    plan::state_score(st.id.as_str()),
                    st.initial
                ));
            }
        }
        // spec-0032: a named datum's shadow is seeded to the same initial, so a
        // player's first tick announces nothing — the announcement is for a CHANGE.
        for st in named_states(plan) {
            if st.scope == StateScope::Player {
                body.push(format!(
                    "scoreboard players set @s {} {}",
                    state_shadow_score(st.id.as_str()),
                    st.initial
                ));
            }
        }
        body.push(format!("tag @s add {}", plan::STATE_SEEDED_TAG));
        fns.push(("state_seed".to_string(), lines(&body)));
    }

    // --- show_class ---
    fns.push((
        "show_class".to_string(),
        lines(&[
            format!("dialog show @s {ns}:class_select"),
            "scoreboard players set @s dw.dlg_shown 1".to_string(),
        ]),
    ));

    // --- class_arm: the one-shot seal on the class trigger ---
    //
    // Run by `tick` as every player, every tick. `dw.class` is a `trigger`
    // objective, and `class_apply_<c>` both consumes it (`reset` clears the
    // score AND re-locks the trigger) and ends in a teleport to the campaign
    // entry point. Re-enabling it unconditionally therefore left a live warp
    // back to the start of the delve behind every already-classed player,
    // usable by anything that can chat a command; the owner ratified sealing it
    // here, at the compiler, rather than asking every caller to know not to.
    //
    // `unless score @s dw.classed matches 1` is the whole seal, and it is
    // per-PLAYER by construction: `dw.classed` is per-player state, so a
    // second player still on the class screen keeps an armed trigger while the
    // first is sealed. It survives death and relog with the score.
    //
    // Its own function, rather than the condition inlined in the tick line, so
    // the generated PackTest can drive the REAL arming path as its own dummy
    // (`execute as <dummy> run function <ns>:class_arm`) instead of restating
    // the guard and proving only its own copy.
    fns.push((
        "class_arm".to_string(),
        lines(&[
            "execute unless score @s dw.classed matches 1 run scoreboard players enable @s dw.class"
                .to_string(),
        ]),
    ));

    // --- class apply ---
    let campaign_start = campaign_start_quests(c);
    for (i, class) in c.classes.content.classes.iter().enumerate() {
        let plan_class = &plan.classes[i];
        let mut body: Vec<String> = Vec::new();
        body.push("scoreboard players reset @s dw.class".to_string());
        for (k, item) in class.kit.iter().enumerate() {
            let give = format!(
                "give @s {}{} {}",
                item.item,
                kit_item_components(item),
                item.count
            );
            // A class kit is per-player gear by construction. `carrier: "one"`
            // (v0.6, spec-0018) marks a **party-unique** kit item — exactly one
            // copy enters the party, to the first player who takes this class —
            // latched on its own `dw.sys` sentinel so a second taker gets the rest
            // of the kit but not the singleton. Absent `carrier` → unchanged.
            if matches!(item.carrier, Some(delvewright_dsl::Carrier::One)) {
                let latch = format!("#kit_{}_{k}", plan_class.safe);
                body.push(format!(
                    "execute unless score {latch} dw.sys matches 1 run {give}"
                ));
                body.push(format!("scoreboard players set {latch} dw.sys 1"));
            } else {
                body.push(give);
            }
        }
        // spec-0016 §1: a bonfire rest refills the resting player's OWN flask, so
        // the pack has to remember which class they took — `dw.class` is a trigger
        // this function resets and `dw.classed` records only that a class was
        // taken. Emitted only when the campaign declares a flask, so every other
        // campaign's class apply is byte-identical.
        if !plan.flasks().is_empty() {
            body.push(format!("tag @s add {}", class_tag(&plan_class.safe)));
        }
        body.push("scoreboard players set @s dw.classed 1".to_string());
        // Party state (spec-0018): the campaign-start quests activate for the
        // PARTY the moment any player takes a class, so a second player who is
        // still on the class screen is not behind on the quest state.
        for qid in &campaign_start {
            body.push(format!(
                "scoreboard players set {} {} 1",
                plan::PARTY,
                quest_active_score(qid)
            ));
        }
        // teleport to the first area's spawn anchor
        if let Some(pos) = campaign_spawn(plan) {
            body.push(format!("teleport @s {} {} {}", pos[0], pos[1], pos[2]));
        }
        fns.push((format!("class_apply_{}", plan_class.safe), lines(&body)));
    }

    // --- dialog option handlers ---
    for npc in &plan.npcs {
        for opt in &npc.options {
            let mut body: Vec<String> = Vec::new();
            body.push(format!(
                "scoreboard players reset @s {}",
                npc.trigger_objective
            ));
            // Re-arm the trigger IN THIS FUNCTION, immediately after consuming it.
            //
            // `reset` both clears the score and re-locks the trigger, and the only
            // other re-enable is the per-tick `scoreboard players enable @a` at the
            // top of `tick`. On a dedicated server that is invisible: the handler
            // runs inside tick N, the next tick re-enables, and the player's next
            // click lands in tick N+1 or later. On the **integrated (singleplayer)
            // server** it is a real hole — 1.21.9+ freezes the integrated server
            // while a screen is open, and the last thing this handler does is show
            // the next dialog node. So: tick N re-enables, dispatches here, we lock
            // the trigger, we open the next screen, ticking STOPS. The player's
            // click is queued and executed the instant ticking resumes — before the
            // tick function's re-enable — and vanilla rejects it ("You can't
            // trigger this objective yet"), silently swallowing one dialogue
            // choice. A dedicated server never pauses, so no rung of the validation
            // ladder can reproduce it.
            //
            // Placed here rather than at the end of the body on purpose: the
            // flag-gate below can `return fail`, and an end-of-body re-enable would
            // be skipped on exactly the path that consumed the trigger without
            // doing anything. Nothing below re-locks it, so this position strictly
            // dominates. `enable` on an unset score initialises it to 0, which
            // matches no dispatch guard (option values are 1-based).
            //
            // The per-tick `enable @a` stays as belt-and-braces.
            body.push(format!(
                "scoreboard players enable @s {}",
                npc.trigger_objective
            ));
            // v0.4: a flag-gated option is inert until its flags are set — so a
            // direct `/trigger` (the bot's path, which bypasses the UI variant
            // hiding) cannot fire it early. `return fail` short-circuits the rest.
            // The story flags are party state (spec-0018): what one player learned
            // from an NPC opens the option for whoever next speaks to them.
            for f in &opt.requires_flags {
                body.push(format!(
                    "execute unless score {} {} matches 1 run return fail",
                    plan::PARTY,
                    plan::flag_score(f)
                ));
            }
            // v0.6: the negative gate — a `forbids_flags`-suppressed option is
            // equally inert to a direct `/trigger` once any listed flag is set.
            for f in &opt.forbids_flags {
                body.push(format!(
                    "execute if score {} {} matches 1 run return fail",
                    plan::PARTY,
                    plan::flag_score(f)
                ));
            }
            // v0.10: the numeric gate, made inert to a direct `/trigger` the same
            // way. One `return fail` per term — any single comparison failing
            // shuts the option — which is what `negate` spells.
            for clause in state_clauses(plan, &opt.requires_state, true) {
                body.push(format!("execute {clause} run return fail"));
            }
            // v0.4: set any flags this option declares (dialogue `set-flag`).
            for f in &opt.sets_flags {
                body.push(format!(
                    "scoreboard players set {} {} 1",
                    plan::PARTY,
                    plan::flag_score(f)
                ));
            }
            // v0.5: world time / weather cuts this option declares (dialogue
            // `set-time`/`set-weather`, spec-0010). Dimension-global instant cuts.
            for t in &opt.sets_time {
                body.push(format!("time set {}", t.token()));
            }
            for w in &opt.sets_weather {
                body.push(format!("weather {}", w.token()));
            }
            // v0.6: party-wide respawn checkpoints this option sets (dialogue
            // `set-checkpoint`, spec-0012).
            for (anchor, on_respawn) in &opt.sets_checkpoints {
                emit_set_checkpoint(plan, anchor, on_respawn, &mut body);
            }
            // v0.6: deferred NPCs this option brings into the world (dialogue
            // `spawn-npc`) — a character walking in mid-conversation.
            for n in &opt.spawns_npcs {
                body.push(format!("function {ns}:{}", spawn_npc_fn(n)));
            }
            for obj in &opt.completes {
                if let Some((qid, _)) = objective_quest(c, obj) {
                    body.push(format!(
                        "execute if score {p} {} matches 1 unless score {p} {} matches 1 run function {ns}:complete_{}",
                        quest_active_score(qid),
                        obj_score(obj),
                        safe_obj_fn(obj),
                        p = plan::PARTY
                    ));
                }
            }
            if let Some(next) = &opt.next {
                body.push(show_node_cmd(plan, npc, next));
            }
            fns.push((format!("dlg_{}_{}", npc.safe, opt.n), lines(&body)));
        }
        // keeper interaction reward: consume the interaction record, then show
        // whatever the cast ledger says this NPC's right-click offers right now
        // (spec-0020). With no ledger this is the single root line it always was.
        let mut talk = vec![format!(
            "advancement revoke @s only {ns}:{}_interact",
            npc.safe
        )];
        talk.extend(cast_dispatch(plan, npc, &casts));
        fns.push((format!("talk_{}", npc.safe), lines(&talk)));
        fns.extend(cast_selector_fn(plan, npc, &casts));
        fns.extend(cast_bark_fns(plan, npc, &casts));
        // v0.4: flag-gate chooser functions for gated nodes.
        for func in gated_node_choosers(plan, npc) {
            fns.push(func);
        }
    }

    // --- objective completion + quest checks ---
    for q in &c.quests.content.quests {
        let q_area = plan.quest_area(q.id.as_str()).unwrap_or("");
        for o in &q.objectives {
            let oid = o.id().as_str();
            // v0.3 activation function (gap 13): run once when the objective
            // activates (driven from `tick`) — set the global once-flag, then place
            // the objective's prop(s). Emitted only for objectives with a prop.
            if v03 {
                let cmds = activation_commands(plan, q_area, o);
                if !cmds.is_empty() {
                    let mut act = vec![format!(
                        "scoreboard players set {} dw.sys 1",
                        activation_flag(oid)
                    )];
                    act.extend(cmds);
                    fns.push((format!("activate_{}", safe_obj_fn(oid)), lines(&act)));
                }
            }
            // v0.3 objective-activation feedback (M2 fix 4): the announce function
            // shows the title + hint once and plays a subtle sound. Emitted only
            // for titled objectives (v0.3); nothing for v0.2.
            if v03 && let Some(title) = o.title() {
                // spec-0018: the objective is the PARTY's, so its title, hint and
                // cue address `@a` and the once-latch lives on the party holder —
                // one announcement per objective, heard by everyone, never a
                // per-player replay for whoever happened to be standing nearby.
                let mut ann: Vec<String> = Vec::new();
                ann.push(format!(
                    "tellraw @a {}",
                    // One sentence, one key: the title is a `with` argument rather
                    // than a second component, so a translation decides where the
                    // title sits (chrome::OBJECTIVE_NEW). `bold: false` on the
                    // argument keeps the title unbolded now that it inherits the
                    // prefix's style instead of standing beside it.
                    tr_with(
                        &chrome.get(delvewright_dsl::chrome::OBJECTIVE_NEW),
                        &[
                            ("color", json!("yellow")),
                            ("bold", json!(true)),
                            (
                                "with",
                                json!([tr_with(
                                    title,
                                    &[("color", json!("gold")), ("bold", json!(false))]
                                )])
                            ),
                        ],
                    )
                ));
                if let Some(hint) = o.hint() {
                    ann.push(format!(
                        "tellraw @a {}",
                        tr_with(hint, &[("color", json!("gray")), ("italic", json!(true))])
                    ));
                }
                ann.push("playsound minecraft:block.note_block.pling player @a".to_string());
                ann.push(format!(
                    "scoreboard players set {} {} 1",
                    plan::PARTY,
                    announce_score(oid)
                ));
                fns.push((format!("announce_{}", safe_obj_fn(oid)), lines(&ann)));
            }

            let mut body: Vec<String> = Vec::new();
            // The completing action advances the PARTY (spec-0018) — this single
            // write is what lets two players clear two arms of an AND-join in two
            // rooms and unlock the successor for both.
            body.push(format!(
                "scoreboard players set {} {} 1",
                plan::PARTY,
                obj_score(oid)
            ));
            // Machine completion-marker for the validation bot, broadcast the
            // instant this objective's score flips — BEFORE any effect that may
            // teleport, open a cutscene or complete the campaign, so the harness
            // observes each objective's own completion in path order. The critical
            // path names the objective a step must prove; this is the only evidence
            // the bot accepts for it (see `plan::marker_line`). Player chat can
            // never start with the sigil and `DW0182` reserves it in authored /
            // translated text, so it cannot be forged. `@a` for the same reason the
            // campaign marker uses it: a bot filling a seat in a multiplayer delve
            // must still see it.
            body.push(format!(
                "tellraw @a {}",
                json!({
                    "text": plan::marker_line(ns, oid),
                    "color": "dark_gray"
                })
            ));
            // v0.3 objective-completion feedback (M2 fix 4): a confirmation line +
            // sound so progress is legible. Titled objectives only; v0.2 unchanged.
            if v03 && let Some(title) = o.title() {
                body.push(format!(
                    "tellraw @a {}",
                    tr_with(
                        &chrome.get(delvewright_dsl::chrome::OBJECTIVE_COMPLETE),
                        &[
                            ("color", json!("green")),
                            (
                                "with",
                                json!([tr_with(title, &[("color", json!("white"))])])
                            ),
                        ],
                    )
                ));
                body.push("playsound minecraft:entity.experience_orb.pickup player @a".to_string());
            }
            // Objective-marker lifecycle: despawn every ENTITY this
            // objective's activation summoned, so a completed interact/reach
            // objective leaves nothing behind. Two motivations, strongest first:
            // (1) a finished interact objective must not remain clickable — its
            // `minecraft:interaction` hitbox is a game-design correctness issue, not
            // mere clutter; (2) the leaked hitboxes and wayfinding item_displays are
            // non-colliding but congest the critical-path bot's pathfinding around
            // later NPCs. Prop BLOCKS (spec-0008 interact prop, collect chest) are
            // the affordance itself — real world blocks, intended scenery — so they
            // persist; only summoned entities are removed. Gated identically to the
            // summon (v03 + a non-empty activation) so v0.2 campaigns and objectives
            // with no summon stay byte-identical.
            if v03 && !activation_commands(plan, q_area, o).is_empty() {
                body.extend(completion_cleanup(o));
            }
            // `complete_<obj>` is dispatched `as @a` from `tick`, so this bundle
            // runs with the acting player as `@s` (see `Audience::Party`).
            body.extend(emit_effect_bundle(
                plan,
                objective_effects(c, oid),
                root_audience(delvewright_dsl::EffectRootKind::ObjectiveComplete),
            ));
            // Inter-area transport: if completing this objective moves the player
            // into a different area on the critical path, teleport them to that
            // area's entry spawn (areas are AREA_SPACING apart across void). Runs
            // after gate effects so the destination area is already unlocked.
            if let Some(pos) = plan.transport.get(oid) {
                body.push(format!("teleport @s {} {} {}", pos[0], pos[1], pos[2]));
            }
            // A crossing that happens only on ONE branch. The exported
            // path never walks it, so it cannot be unconditional — it is gated on
            // exactly the flag assignment that selects its branch, the same
            // `#party` predicate a branch-gated dialogue option uses. Prefix
            // conditions do not rebind `@s`, so the acting player is still the
            // one carried. Empty for every campaign whose branches cross only
            // where the exported path already does (byte-identity).
            let mut emitted: BTreeSet<String> = BTreeSet::new();
            for row in branch_transport.get(oid).into_iter().flatten() {
                let tp = format!("teleport @s {} {} {}", row.pos[0], row.pos[1], row.pos[2]);
                let mut cmd = String::new();
                for f in &row.set {
                    cmd.push_str(&format!(
                        " if score {} {} matches 1",
                        plan::PARTY,
                        plan::flag_score(f)
                    ));
                }
                for f in &row.unset {
                    cmd.push_str(&format!(
                        " unless score {} {} matches 1",
                        plan::PARTY,
                        plan::flag_score(f)
                    ));
                }
                // A branch that pins no flags at all is indistinguishable at
                // runtime: there is nothing to condition on, so the crossing is
                // simply unconditional.
                let line = if cmd.is_empty() {
                    tp
                } else {
                    format!("execute{cmd} run {tp}")
                };
                if emitted.insert(line.clone()) {
                    body.push(line);
                }
            }
            body.push(format!(
                "function {ns}:check_q_{}",
                plan::safe_local(q.id.as_str())
            ));
            fns.push((format!("complete_{}", safe_obj_fn(oid)), lines(&body)));
        }

        // check_q_<quest>
        // The quest-level AND (every objective done) is a party predicate too:
        // whoever finishes the LAST objective completes the quest for everyone.
        let mut check: Vec<String> = Vec::new();
        let mut guard = "execute".to_string();
        for o in &q.objectives {
            guard.push_str(&format!(
                " if score {} {} matches 1",
                plan::PARTY,
                obj_score(o.id().as_str())
            ));
        }
        guard.push_str(&format!(
            " unless score {} {} matches 1 run function {ns}:complete_q_{}",
            plan::PARTY,
            quest_score(q.id.as_str()),
            plan::safe_local(q.id.as_str())
        ));
        check.push(guard);
        fns.push((
            format!("check_q_{}", plan::safe_local(q.id.as_str())),
            lines(&check),
        ));

        // complete_q_<quest>
        let mut done: Vec<String> = Vec::new();
        done.push(format!(
            "scoreboard players set {} {} 1",
            plan::PARTY,
            quest_score(q.id.as_str())
        ));
        done.extend(emit_effect_bundle(
            plan,
            &q.on_complete,
            root_audience(delvewright_dsl::EffectRootKind::QuestComplete),
        ));
        // activate quests triggered by this quest's completion
        for dep in &c.quests.content.quests {
            if let Trigger::QuestComplete { quest } = &dep.trigger
                && quest.as_str() == q.id.as_str()
            {
                done.push(format!(
                    "scoreboard players set {} {} 1",
                    plan::PARTY,
                    quest_active_score(dep.id.as_str())
                ));
            }
        }
        fns.push((
            format!("complete_q_{}", plan::safe_local(q.id.as_str())),
            lines(&done),
        ));
    }

    // --- campaign_complete (shared by campaign-complete effect) ---
    let title = &c.world.content.title;
    let mut cc: Vec<String> = Vec::new();
    // Campaign completion is the party's (spec-0018): one holder write, and the
    // advancement + fanfare granted to every member — the delve ends for all of
    // them at once, whoever struck the last blow.
    cc.push(format!(
        "scoreboard players set {} dw.campaign 1",
        plan::PARTY
    ));
    cc.push(format!("advancement grant @a only {ns}:campaign_complete"));
    cc.push(format!(
        "tellraw @a {}",
        json!([
            tr_with(
                &chrome.get(delvewright_dsl::chrome::CAMPAIGN_COMPLETE),
                &[
                    ("color", json!("gold")),
                    ("with", json!([tr(title)])),
                ],
            ),
            { "text": "\n" },
            tr_with(
                &chrome.get(delvewright_dsl::chrome::CAMPAIGN_SIGNATURE),
                &[("color", json!("gray"))],
            )
        ])
    ));
    // v0.3 finale fanfare (M2 fix 4): the owner finished the finale and got no
    // feedback. Show a proper title banner + play a fanfare. Gated on v0.3 so the
    // shared `campaign_complete` stays byte-identical for hello-world / keep-crawl.
    if v03 {
        cc.push(format!(
            "title @a title {}",
            tr_with(
                &chrome.get(delvewright_dsl::chrome::CAMPAIGN_BANNER),
                &[("color", json!("gold")), ("bold", json!(true))],
            )
        ));
        cc.push(format!(
            "title @a subtitle {}",
            tr_with(title, &[("color", json!("yellow"))])
        ));
        cc.push("playsound minecraft:ui.toast.challenge_complete player @a".to_string());
    }
    // Machine-readable completion marker for the validation bot. The bot reads
    // `dw.campaign` from the sidebar per the amended contract, BUT mineflayer
    // 4.37.x cannot parse 1.21.11 scoreboard score packets (verified live: no
    // score updates ever surface). Broadcasting a stable token in chat — which
    // mineflayer DOES parse reliably — lets the bot observe completion. Same
    // anchored grammar as the per-objective markers, with the `campaign` token;
    // the harness treats its arrival anywhere before the final step as a hard
    // error (branch incoherence: the campaign completed while steps remained).
    // `@a` so a bot filling a seat in a future multiplayer delve still sees it.
    cc.push(format!(
        "tellraw @a {}",
        json!({
            "text": plan::marker_line(ns, plan::MARKER_TOKEN_CAMPAIGN),
            "color": "dark_gray"
        })
    ));
    fns.push(("campaign_complete".to_string(), lines(&cc)));

    // --- v0.3: wave spawn functions + verb reward functions ---
    for w in &c.quests.content.waves {
        // Compiler-validated standable spawn cells near the wave anchor, in the
        // anchor's own room, one per mob. A wave whose spawn anchor
        // resolves in no assembled area gets no entry here and is skipped exactly
        // as before — DW0310 (check_wave_spawns) catches a dangling spawn-wave.
        let Some(cells) = wave_placements.get(w.id.as_str()) else {
            continue;
        };
        let mut body: Vec<String> = Vec::new();
        // spec-0016 §1: mark the wave as seated, so a bonfire rest only re-seats
        // waves the party has actually met. Emitted only for a `respawns_on_rest`
        // wave — every other campaign's `spawn_<wave>` is byte-identical.
        if w.respawns_on_rest {
            body.push(format!(
                "scoreboard players set {} dw.sys 1",
                wave_seated_holder(w.id.as_str())
            ));
        }
        body.push(format!(
            "scoreboard players set {} {} {}",
            plan::wave_counter(w.id.as_str()),
            plan::WAVE_OBJECTIVE,
            plan::wave_total(w)
        ));
        // spec-0016 §6: a lane wave spawns as a Raider PATROL SQUAD — one leader,
        // everyone `Patrolling:1b` and pointed at the first proven waypoint. The
        // squad's own march clock starts with it. Empty for every other wave, so
        // pre-§6 `spawn_<wave>` output is byte-identical.
        let lane = w.lane.as_ref().zip(lane_routes.get(w.id.as_str()));
        let mut idx = 0i32;
        for (k, mob) in w.mobs.iter().enumerate() {
            // CustomName as a plain SNBT text component (M2 fix 1). Waves are
            // v0.3-only, so no v0.2 byte-identity concern.
            let name = match &mob.name {
                Some(n) => format!(",CustomName:{},CustomNameVisible:1b", snbt_component(n)),
                None => String::new(),
            };
            // Equipment: v0.6 explicit slots merged over the armed-mob default
            // (M2 fix 5: a summoned wither_skeleton/skeleton otherwise had no
            // weapon and was trivial). Every slot the v0.9 `drops[]` list does
            // not name keeps drop chance 0 — rank-and-file gear is never
            // lootable.
            let equip = wave_equipment(&mob.entity, mob.equipment.as_ref(), &mob.drops)
                .map(|e| format!(",{e}"))
                .unwrap_or_default();
            // v0.9: a declared quest-item drop rides the mob's own
            // death loot table. Absent on every other mob, so a wave that
            // declares no item drop keeps vanilla's own table and its exact
            // pre-0.9 summon string.
            let loot = if has_item_drop(&mob.drops) {
                format!(
                    ",DeathLootTable:\"{}\"",
                    death_loot_table(
                        ns,
                        Some(drop_loot_path("wave", &format!("{}-{k}", w.id.as_str()))),
                    )
                )
            } else {
                String::new()
            };
            // v0.4 attribute overrides (spec-0008 §4), emitted as 1.21.11
            // attribute components in the summon NBT. Empty for a plain mob. A
            // lane mob's `follow_range` is FORCED to the lane's `aggro_radius`:
            // release radius and perception radius must be the same number, or a
            // patrolling raider targets a player it cannot engage and holds
            // ground mid-lane (`DW0381` rejects a contradicting override).
            let effective_attrs = match lane {
                Some((l, _)) => Some(lane_attributes(mob.attributes, l.aggro_radius)),
                None => mob.attributes,
            };
            let attrs = attributes_snbt(effective_attrs.as_ref());
            // v0.4 permanent ambient effects: applied to this stack via a temp tag
            // after summon, so they land on exactly this mob type (not the whole
            // wave). Empty for a plain mob.
            let has_effects = !mob.effects.is_empty();
            let tmp = if has_effects { ",\"dw_tmp\"" } else { "" };
            for _ in 0..mob.count {
                // Each mob takes the next validated standable cell (ascending BFS
                // distance from the anchor); `cells` has exactly one per mob. AI is
                // left enabled (no NoAI) so the mobs fight.
                let cell = cells[idx as usize];
                let c = ent_xyz(cell);
                // spec-0016 §6 patrol NBT. `patrol_target` is the **snake_case
                // int-array** form and nothing else: 1.21.11's strict codec
                // silently DROPS the legacy `PatrolTarget:{X,Y,Z}` compound, and
                // the squad then patrols to vanilla-rolled random points — the
                // working-but-drunk failure the spike caught live. `Patrolling`
                // and `PatrolLeader` keep their camelCase names.
                let patrol = match lane {
                    Some((_, wps)) => {
                        let t = wps[0];
                        let leader = if idx == 0 { ",PatrolLeader:1b" } else { "" };
                        format!(
                            ",Patrolling:1b{leader},patrol_target:[I;{},{},{}]",
                            t[0], t[1], t[2]
                        )
                    }
                    None => String::new(),
                };
                let lead_tag = match lane {
                    Some(_) if idx == 0 => {
                        format!(",\"{}\"", lane_leader_tag(w.id.as_str()))
                    }
                    _ => String::new(),
                };
                body.push(format!(
                    "summon {} {} {} {} {{Tags:[\"{}\"{lead_tag}{tmp}],PersistenceRequired:1b{name}{equip}{loot}{attrs}{patrol}}}",
                    mob.entity,
                    c[0],
                    c[1],
                    c[2],
                    plan::wave_tag(w.id.as_str())
                ));
                idx += 1;
            }
            if has_effects {
                for eff in &mob.effects {
                    body.push(format!(
                        "effect give @e[tag=dw_tmp] {} infinite {} true",
                        eff.effect, eff.amplifier
                    ));
                }
                body.push("tag @e[tag=dw_tmp] remove dw_tmp".to_string());
            }
        }
        // spec-0016 §6: the squad marches from waypoint 0 and its clock starts
        // with it. `schedule … <n>t` is replace-mode, so a re-seat (spec-0016 §1)
        // can never double the clock up.
        if let Some((_, _)) = lane {
            let safe = plan::safe_local(w.id.as_str());
            body.push(format!(
                "scoreboard players set {} dw.sys 0",
                lane_index_holder(w.id.as_str())
            ));
            body.push(format!(
                "schedule function {ns}:lane_tick_{safe} {LANE_PERIOD_TICKS}t"
            ));
        }
        fns.push((
            format!("spawn_{}", plan::safe_local(w.id.as_str())),
            lines(&body),
        ));
        if let Some((l, wps)) = lane {
            fns.push(lane_tick_fn(ns, w, l, wps));
        }
        // spec-0016 §1: the re-seat — clear survivors, then re-run the wave's own
        // spawn (same authored composition, same proven cells). Emitted for a
        // `respawns_on_rest` wave and for a billed elite/boss wave (whose rest
        // dispatch is guarded on the wave still standing — the undefeated
        // refresh), and for nothing else → byte-identical.
        if w.respawns_on_rest || plan.undefeated_reseat_waves().iter().any(|u| u.id == w.id) {
            let safe = plan::safe_local(w.id.as_str());
            fns.push((
                format!("wave_reseat_{safe}"),
                lines(&[
                    format!("kill @e[tag={}]", plan::wave_tag(w.id.as_str())),
                    format!("function {ns}:spawn_{safe}"),
                ]),
            ));
        }
        // --- The wave CENSUS probe surface ---
        //
        // The live ladder used to answer "what is standing at this encounter?" by
        // silhouette: every entity mineflayer tracked, no distance filter, any mob
        // taller than half a block. On the drowned bell that counted five ambush
        // actors and a neighbouring wave as members of whichever wave was being
        // measured, and — since they were alive on both sides of a scripted death
        // — reported them as survivors the re-seat had failed to remove.
        // The wave tag is the only exact answer to that question and the compiler
        // owns it, so the compiler owns the census too: the harness asks these
        // functions and reads numbers, instead of guessing from shapes.
        //
        // Emitted for EVERY wave — the probe is how the ladder counts any
        // encounter, not only a re-seating one. A campaign with no waves emits
        // nothing here and is byte-identical.
        {
            let safe = plan::safe_local(w.id.as_str());
            let tag = plan::wave_tag(w.id.as_str());
            let brand = plan::wave_brand_tag(w.id.as_str());
            let wid = w.id.as_str();
            // Brand / unbrand: stamp this life's mobs, and clear the stamp. The
            // unbrand selects the BRAND, not the wave, so a mob that somehow
            // outlived its wave tag still gets cleaned up.
            fns.push((
                format!("wave_brand_{safe}"),
                lines(&[format!("tag @e[tag={tag}] add {brand}")]),
            ));
            fns.push((
                format!("wave_unbrand_{safe}"),
                lines(&[format!("tag @e[tag={brand}] remove {brand}")]),
            ));
            // Per-mob accumulation, run `as` each tagged mob. Health and its
            // maximum both come from vanilla primitives — `data get entity @s
            // Health` and `attribute @s max_health get` — so "damaged" is a fact
            // the server states, never a table the compiler invents (DW0475) and
            // never a value the client happened to be sent (a live 1.21.11 server
            // does not put an unmodified max health on the wire at all, which is
            // why the silhouette probe had to guess it from the highest health it
            // had ever seen).
            //
            // Scale 100: two decimal places carried as integers, so positions and
            // health cross the chat channel exactly, with no float formatting to
            // parse. The holders are shared across waves, which is safe because a
            // census is one atomic function call.
            fns.push((
                format!("wave_census_one_{safe}"),
                lines(&[
                    "scoreboard players add #wcen_n dw.sys 1".to_string(),
                    format!(
                        "execute if entity @s[tag={brand}] run scoreboard players add #wcen_b \
                         dw.sys 1"
                    ),
                    "execute store result score #wcen_h dw.sys run data get entity @s Health 100"
                        .to_string(),
                    "execute store result score #wcen_m dw.sys run attribute @s \
                     minecraft:max_health get 100"
                        .to_string(),
                    "execute if score #wcen_h dw.sys < #wcen_m dw.sys run scoreboard players add \
                     #wcen_d dw.sys 1"
                        .to_string(),
                    "execute store result score #wcen_x dw.sys run data get entity @s Pos[0] 100"
                        .to_string(),
                    "execute store result score #wcen_y dw.sys run data get entity @s Pos[1] 100"
                        .to_string(),
                    "execute store result score #wcen_z dw.sys run data get entity @s Pos[2] 100"
                        .to_string(),
                    format!("tellraw @a {}", census_mob_component(ns, wid)),
                ]),
            ));
            // The census itself: zero the accumulators, walk the tag, then state
            // the totals. `#wcen_seq` counts censuses so the harness can tell this
            // answer from a stale one — it never has to write a delve score to ask
            // a question.
            fns.push((
                format!("wave_census_{safe}"),
                lines(&[
                    "scoreboard players add #wcen_seq dw.sys 1".to_string(),
                    "scoreboard players set #wcen_n dw.sys 0".to_string(),
                    "scoreboard players set #wcen_b dw.sys 0".to_string(),
                    "scoreboard players set #wcen_d dw.sys 0".to_string(),
                    format!("execute as @e[tag={tag}] run function {ns}:wave_census_one_{safe}"),
                    format!("tellraw @a {}", census_summary_component(ns, wid)),
                ]),
            ));
        }
        // kill reward: each slain wave mob decrements the countdown, then re-arms.
        fns.push((
            format!("k_reward_{}", plan::safe_local(w.id.as_str())),
            lines(&[
                format!(
                    "scoreboard players remove {} {} 1",
                    plan::wave_counter(w.id.as_str()),
                    plan::WAVE_OBJECTIVE
                ),
                format!(
                    "advancement revoke @s only {ns}:k_{}",
                    plan::safe_local(w.id.as_str())
                ),
            ]),
        ));
    }
    for q in &c.quests.content.quests {
        let qa = quest_active_score(q.id.as_str());
        for o in &q.objectives {
            match o {
                Objective::Interact { id, .. } => {
                    // Human click path: the interaction advancement sets the same
                    // trigger the bot chats; the per-tick handler applies guards.
                    fns.push((
                        format!("i_reward_{}", plan::safe_local(id.as_str())),
                        lines(&[
                            format!(
                                "advancement revoke @s only {ns}:i_{}",
                                plan::safe_local(id.as_str())
                            ),
                            format!(
                                "scoreboard players set @s {} 1",
                                plan::interact_trigger(id.as_str())
                            ),
                        ]),
                    ));
                }
                Objective::Collect { id, .. } => {
                    // inventory_changed reward: complete (if the quest/after/flags
                    // guards hold), then re-arm.
                    fns.push((
                        format!("c_reward_{}", plan::safe_local(id.as_str())),
                        lines(&[
                            format!(
                                "execute{} run function {ns}:complete_{}",
                                pending_guard(plan, o, &qa),
                                safe_obj_fn(id.as_str())
                            ),
                            format!(
                                "advancement revoke @s only {ns}:c_{}",
                                plan::safe_local(id.as_str())
                            ),
                        ]),
                    ));
                }
                _ => {}
            }
        }
    }

    // v0.4 generated functions: NPC moves, cutscene drivers, trigger effects.
    // Each is empty for a campaign that uses none (byte-identical v0.2/v0.3).
    fns.extend(spawn_npc_fns(plan));
    fns.extend(movenpc_fns(plan, moves));
    fns.extend(actor_fns(plan, actor_moves));
    fns.extend(sequence_fns(plan));
    fns.extend(teleport_fns(plan));
    fns.extend(cutscene_fns(plan, moves, actor_moves));
    fns.extend(env_trigger_fns(plan, chrome));
    fns.extend(trap_fns(plan, trap_gates));
    // spec-0022: the proven per-cell volley geometry and the settled collapse
    // debris. Empty for a campaign using neither verb (byte-identical).
    fns.extend(volley_fns(plan, payloads));
    fns.extend(collapse_fns(plan, payloads));
    fns.extend(boundary_fns(plan, chrome));
    fns.extend(night_vision_fns(plan));
    // v0.8 seal answers. Empty for a campaign that seals no gate.
    fns.extend(seal_fns(plan, chrome));

    fns.sort_by(|a, b| a.0.cmp(&b.0));
    fns
}

/// Validated spawn cells per wave: wave id → one standable cell per mob, in
/// summon order. Only waves whose spawn anchor resolves have an entry.
type WavePlacements = BTreeMap<String, Vec<[i32; 3]>>;

/// The **defended point** each `summon: aggro-edge` wave's perception ring is
/// measured from (spec-0016 §6): its `anchor`, snapped to standable footing. The
/// generated PackTest asserts ring distance against exactly this cell, so the
/// runtime check and the compile-time placement share one origin.
type WaveRings = BTreeMap<String, [i32; 3]>;

/// Where every wave actually IS in the assembled world: the three products of
/// wave planning, which always travel together into the generated PackTests.
/// Bundled because a template that asks "did the re-seat put them back?" needs
/// all three — the seated cells, the lane polyline, and the aggro-edge ring
/// centre — to say where "back" is.
struct WaveGeometry<'a> {
    /// DW0312-proven seated spawn cells, per wave.
    placements: &'a WavePlacements,
    /// DW0386-proven lane polylines, per lane wave.
    lanes: &'a crate::nav::LaneRoutes,
    /// The snapped ring centre of each `summon: aggro-edge` wave.
    rings: &'a WaveRings,
}

// --- spec-0016 §6: TD lanes -------------------------------------------------

/// The lane clock period, in ticks. The spike measured 20–40 ticks as the working
/// band (2 commands per mob per cycle, 0.5–1.0 ms MSPT for a four-mob squad) and
/// ran 30t (1.5 s) live: fast enough that the re-assert defeats vanilla's
/// arrival re-roll and the lone-patroller self-cancel, slow enough to cost
/// nothing.
const LANE_PERIOD_TICKS: u32 = 30;

/// How close a squad member must get to the current waypoint for the lane to
/// advance, in blocks. Measured: with the 1.5 s re-assert, an advance radius of 8
/// produced zero stalls over six waypoints.
const LANE_ADVANCE_RADIUS: u32 = 8;

/// The tag on a lane squad's `PatrolLeader`. The leader is the wave's first
/// summoned mob (deterministic), and the tag exists so the runtime — and the
/// generated PackTest — can address the one mob vanilla treats specially.
fn lane_leader_tag(wave_id: &str) -> String {
    format!("dw_lead_{}", plan::safe_local(wave_id))
}

/// The fake-player holder carrying a lane's current waypoint index on `dw.sys`.
/// One index for the whole squad: the lane is a thing the warband walks, not a
/// per-mob itinerary, which is also what keeps the re-assert to one command per
/// mob per cycle.
fn lane_index_holder(wave_id: &str) -> String {
    format!("#lane_{}", plan::safe_local(wave_id))
}

/// A lane mob's attributes with `follow_range` forced to the lane's
/// `aggro_radius` (spec-0016 §6). Perception radius and release radius are the
/// same number by construction: a patrolling raider that targets a player outside
/// its engagement range holds ground instead of marching, so any daylight between
/// the two stalls the squad mid-lane. `DW0381` rejects a contradicting authored
/// override rather than silently overwriting it.
fn lane_attributes(
    base: Option<delvewright_dsl::MobAttributes>,
    aggro_radius: u32,
) -> delvewright_dsl::MobAttributes {
    let mut a = base.unwrap_or(delvewright_dsl::MobAttributes {
        max_health: None,
        attack_damage: None,
        movement_speed: None,
        follow_range: None,
    });
    a.follow_range = Some(f64::from(aggro_radius));
    a
}

/// The per-wave lane clock (spec-0016 §6), implementing the spike's verdict
/// verbatim:
///
/// 1. **advance** — when any squad member is within [`LANE_ADVANCE_RADIUS`] of
///    the current waypoint, the shared index steps forward. Emitted in
///    DESCENDING index order so one cycle can advance at most one waypoint (an
///    ascending emission would cascade the whole lane in a single tick), and
///    driven by any squad member rather than the leader alone so a dead leader
///    cannot strand the warband on a waypoint forever.
/// 2. **release** — a mob with a player inside `aggro_radius` gets
///    `Patrolling:0b` and is thereafter a plain native hostile. Vanilla's patrol
///    goal is hard-gated on having no target, so combat-preempts-routing is
///    engine semantics; this line just makes the handover explicit and permanent
///    for as long as the player stays close.
/// 3. **re-assert** — a mob with no player inside `aggro_radius` is put back on
///    the lane. This is what defeats vanilla's random re-roll on arrival and the
///    lone-patroller self-cancel; it is inert during combat because the goal
///    cannot restart while the mob has a target.
/// 4. **re-arm** — reschedule while any squad member lives, so the clock stops
///    on its own when the wave is cleared.
fn lane_tick_fn(
    ns: &str,
    w: &delvewright_dsl::Wave,
    lane: &delvewright_dsl::WaveLane,
    wps: &[[i32; 3]],
) -> (String, String) {
    let safe = plan::safe_local(w.id.as_str());
    let tag = plan::wave_tag(w.id.as_str());
    let idx = lane_index_holder(w.id.as_str());
    let r = lane.aggro_radius;
    let adv = LANE_ADVANCE_RADIUS;
    let mut body: Vec<String> = Vec::new();
    for i in (0..wps.len().saturating_sub(1)).rev() {
        let c = ent_xyz(wps[i]);
        body.push(format!(
            "execute if score {idx} dw.sys matches {i} positioned {} {} {} if entity \
             @e[tag={tag},distance=..{adv}] run scoreboard players set {idx} dw.sys {}",
            c[0],
            c[1],
            c[2],
            i + 1
        ));
    }
    body.push(format!(
        "execute as @e[tag={tag}] at @s if entity @a[distance=..{r}] run data merge entity @s \
         {{Patrolling:0b}}"
    ));
    for (i, t) in wps.iter().enumerate() {
        body.push(format!(
            "execute if score {idx} dw.sys matches {i} as @e[tag={tag}] at @s unless entity \
             @a[distance=..{r}] run data merge entity @s {{Patrolling:1b,patrol_target:[I;{},{},{}]}}",
            t[0], t[1], t[2]
        ));
    }
    body.push(format!(
        "execute if entity @e[tag={tag}] run schedule function {ns}:lane_tick_{safe} \
         {LANE_PERIOD_TICKS}t"
    ));
    (format!("lane_tick_{safe}"), lines(&body))
}

/// Seat every wave's mobs on compiler-validated standable cells near the wave
/// anchor, confined to the anchor's own assembled piece so the flock never strings
/// across a socket seam into a neighbouring room — an unconfined flock spreads
/// `+x` off its anchor across the nearest seam toward void, some bodies
/// ending inside blocks or outside the room. Cells are chosen by ascending BFS
/// distance from the anchor with a fixed `(y, z, x)` tie-break — deterministic
/// (ADR-0006). A wave that needs more standable footing than its room offers fails
/// the build with [`DW_WAVE_NO_ROOM`] (`DW0312`). A wave whose spawn anchor resolves
/// in no assembled area is skipped (DW0310 handles the dangling reference).
fn plan_wave_spawns(
    plan: &Plan,
    world: &crate::nav::World,
) -> Result<(WavePlacements, WaveRings), BuildFailure> {
    // Wave mobs cannot right-click a fence gate open: seat them on the
    // no-gate-use view, where a closed gate cell is a 1.5-tall barrier — never a
    // seat, and never a doorway the seating flood spills through.
    let entity_world_owned;
    let world: &crate::nav::World = if world.has_use_gates() {
        entity_world_owned = world.without_gate_use();
        &entity_world_owned
    } else {
        world
    };
    let c = plan.campaign;
    let mut out: WavePlacements = BTreeMap::new();
    let mut rings: WaveRings = BTreeMap::new();
    for w in &c.quests.content.waves {
        let (Some(anchor), Some(area)) = (
            wave_spawn_pos(plan, w.id.as_str()),
            plan::wave_area(c, w.id.as_str()),
        ) else {
            continue;
        };
        let need = plan::wave_total(w).max(0) as usize;
        // spec-0016 §6: an aggro-edge wave is spirit-summoned at the edge of
        // perception instead of seated around its anchor, so its cells come from
        // per-mob rings across the whole ARENA rather than the anchor's room.
        if w.summon == Some(delvewright_dsl::WaveSummon::AggroEdge) {
            let (cells, centre) = plan_aggro_edge_spawns(plan, world, w, area, anchor)?;
            out.insert(w.id.as_str().to_string(), cells);
            rings.insert(w.id.as_str().to_string(), centre);
            continue;
        }
        // The room the wave's mobs must stay inside, so the placement flood-fill
        // never crosses a socket seam.
        let bounds = plan.piece_bounds(area, anchor);
        let cells = world.confined_standable_cells(anchor, bounds);
        if cells.len() < need {
            return Err(BuildFailure::Diagnostic {
                code: DW_WAVE_NO_ROOM,
                message: format!(
                    "spawn-wave `{wave}` needs {need} standable spawn cell(s) near \
                     anchor `{anchor_name}` in area `{area}`, but its room provides \
                     only {found}. Each wave mob must stand on validated footing \
                     inside the anchor's own piece (bounds {bounds:?}); the compiler \
                     will not pile mobs into blocks or spill them across a socket \
                     seam. Fix the content: shrink this wave's mob count (currently \
                     {need}) or spawn it in a larger room. Do NOT widen the piece's \
                     socket seams or move the anchor into an adjoining room — that \
                     reopens the cross-seam spill this guard prevents.",
                    wave = w.id.as_str(),
                    anchor_name = w.anchor.as_str(),
                    found = cells.len(),
                ),
            });
        }
        out.insert(
            w.id.as_str().to_string(),
            cells.into_iter().take(need).collect(),
        );
    }
    Ok((out, rings))
}

/// How far inside `follow_range` the aggro ring may reach, in blocks. A discrete
/// voxel grid rarely holds a standable cell at *exactly* `follow_range`, so the
/// ring is an annulus `[follow_range - 1, follow_range]` — one-sided on purpose:
/// a cell outside the mob's own perception summons a mob that stands there
/// (see [`crate::nav::World::annulus_standable_cells`]).
const AGGRO_RING_TOLERANCE: f64 = 1.0;

/// Seat a `summon: aggro-edge` wave (spec-0016 §6) on the boundary of its own
/// perception: for each mob stack, the standable, reachable, line-of-sight cells
/// at that stack's `attributes.follow_range` from the defended anchor, nearest
/// first, no two mobs sharing a cell.
///
/// The party is expected at the defended anchor (that is what "defended" means),
/// so the ring around it IS the aggro boundary of the players — the mobs
/// materialize at the edge of perception and close under pure native AI. The
/// radius is per-stack because perception is per-species: a heavier mob that
/// sees further starts further out, which is exactly the read the owner asked
/// for ("spirit-summoned at the edge, never on top of the players").
///
/// `DW0387` if a stack's ring cannot seat it. `follow_range` is guaranteed
/// present by `DW0385` at validation time; a stack without one is skipped rather
/// than guessed.
fn plan_aggro_edge_spawns(
    plan: &Plan,
    world: &crate::nav::World,
    w: &delvewright_dsl::Wave,
    area: &str,
    anchor: [i32; 3],
) -> Result<(Vec<[i32; 3]>, [i32; 3]), BuildFailure> {
    let bounds = match plan.areas.iter().find(|a| a.area_id == area) {
        Some(a) => a.bounds(),
        None => (anchor, anchor),
    };
    let centre = world.ring_centre(anchor, bounds).unwrap_or(anchor);
    let mut used: BTreeSet<[i32; 3]> = BTreeSet::new();
    let mut cells: Vec<[i32; 3]> = Vec::new();
    for mob in &w.mobs {
        let Some(radius) = mob.attributes.and_then(|a| a.follow_range) else {
            continue;
        };
        let need = mob.count as usize;
        // Band [r-2, r-1], strictly INSIDE perception. Ladder evidence (the
        // drowned bell, runs 10 and 12): the original one-sided [r-1, r] band
        // seats mobs at the marginal edge of perceiving a defender AT the
        // anchor — vanilla target acquisition at exactly `follow_range` is a
        // coin flip, and a summoned mob that acquires nobody stands idle
        // forever, timing out the kill objective. One block of margin turns
        // "materializes at the edge of what it can sense" from fiction into
        // guaranteed engagement.
        let band_outer = (radius - 1.0).max(2.0);
        let ring = world.annulus_standable_cells(anchor, bounds, band_outer, AGGRO_RING_TOLERANCE);
        let picked: Vec<[i32; 3]> = ring
            .iter()
            .copied()
            .filter(|c| !used.contains(c))
            .take(need)
            .collect();
        if picked.len() < need {
            return Err(BuildFailure::Diagnostic {
                code: DW_AGGRO_EDGE_NO_RING,
                message: format!(
                    "`summon: aggro-edge` wave `{wave}` cannot seat {need} × `{entity}` on its \
                     perception ring: at `follow_range` {radius} (the band \
                     [{band_outer}-{AGGRO_RING_TOLERANCE}, {band_outer}], one block inside perception) around defended anchor \
                     `{anchor_name}` ({anchor:?}) in area `{area}`, only {found} \
                     cell(s) are standable, walk-reachable AND in line of sight of the anchor. \
                     The mobs must materialize at the EDGE of perception (spec-0016 §6) — the \
                     compiler will not quietly drop them on the party instead, nor spawn fewer \
                     than authored (a short wave makes a `kill` countdown that never reaches \
                     zero). Fix the content: give the arena room at that radius, lower this \
                     stack's `follow_range` to a ring the arena actually has, or move the \
                     defended anchor off the wall.",
                    wave = w.id.as_str(),
                    entity = mob.entity,
                    anchor_name = w.anchor.as_str(),
                    found = ring.iter().filter(|c| !used.contains(*c)).count(),
                ),
            });
        }
        used.extend(picked.iter().copied());
        cells.extend(picked);
    }
    Ok((cells, centre))
}

/// The absolute spawn position of a wave: the world coords of its `anchor`,
/// resolved in the area of the quest (or single-area trigger) that *spawns* it —
/// see [`plan::wave_area`]. Deliberately independent of objective type, so a
/// kill-less "live threat" wave (spec-0008 §4) resolves a spawn position exactly
/// like a wave that a `kill` objective later drains.
fn wave_spawn_pos(plan: &Plan, wave_id: &str) -> Option<[i32; 3]> {
    let c = plan.campaign;
    let w = plan::wave_of(c, wave_id)?;
    let area = plan::wave_area(c, wave_id)?;
    plan.point(area, w.anchor.as_str())
}

/// The closing line on the completion advancement: the authored `world.outro`
/// (l10n key `world.outro`), else the finale quest's `goal` (key
/// `quest.<id>.goal`) — the thing the party just accomplished, already inventoried
/// and translated. Both are campaign content, so no English is baked in here.
/// Falls back to the delve title only if the finale names no planned quest, which
/// cross-stage validation (`DW0160`) already rejects.
fn campaign_outro(c: &delvewright_dsl::Campaign) -> String {
    if let Some(outro) = &c.world.content.outro {
        return outro.clone();
    }
    let finale = c.quest_plan.content.finale.as_str();
    c.quest_plan
        .content
        .quests
        .iter()
        .find(|q| q.id.as_str() == finale)
        .map(|q| q.goal.clone())
        .unwrap_or_else(|| c.world.content.title.clone())
}

/// `DW0362`: a dialogue node declares more conditionally-visible options than the
/// variant-dialog encoding can carry. Validation-tier content-shape limit.
pub const DW_DIALOGUE_VARIANT_CAP: DwCode = DwCode::every_version("DW0362");

/// The most gated options one dialogue node may declare.
///
/// Vanilla has no conditional option inside a `dialog`, so the compiler encodes
/// visibility by **precomputing every combination**: `n` gated options emit `2^n`
/// dialog JSONs plus a `2^n`-clause dispatcher keyed on a `dw.dmask` bitmask. Ten
/// is 1024 variants for a single node — already an order of magnitude past
/// anything authorable (the largest node in any shipped campaign gates four), and
/// the point past which the pack size, not the author, decides what the delve is.
///
/// There is a hard wall behind the soft one: the mask is built with `1u32 << i`
/// (undefined past 31, a debug-build panic at 32 — the original symptom of this
/// gap) and compared against a Minecraft scoreboard, i.e. an `i32`, so bit 31 is
/// unrepresentable at runtime regardless. The cap keeps the build well clear of
/// both, and turns a compiler panic into a coded content diagnostic that names the
/// node.
pub const MAX_GATED_DIALOGUE_OPTIONS: usize = 10;

/// Fail the build if any dialogue node exceeds [`MAX_GATED_DIALOGUE_OPTIONS`]
/// (`DW0362`). Runs before any variant emission so the `1u32 << n` shifts in
/// `gated_node_choosers` / `emit_dialogs` are unreachable past the cap.
fn check_dialogue_variant_cap(plan: &Plan) -> Result<(), BuildFailure> {
    let v04 = campaign_is_v04(plan);
    for npc in &plan.npcs {
        let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
        for opt in &npc.options {
            if !seen.insert(opt.node_id.as_str()) {
                continue;
            }
            let n = node_gated_options(npc, &opt.node_id, v04).len();
            if n > MAX_GATED_DIALOGUE_OPTIONS {
                return Err(BuildFailure::Diagnostic {
                    code: DW_DIALOGUE_VARIANT_CAP,
                    message: format!(
                        "dialogue node `{}` on npc `{}` declares {n} conditionally-visible \
                         options (`requires_flags` / `forbids_flags` / a `complete-objective` \
                         effect); the cap is {MAX_GATED_DIALOGUE_OPTIONS}. Vanilla cannot hide a \
                         dialog option, so the compiler precomputes every combination: this node \
                         would emit 2^{n} dialog variants and a dispatcher of the same size. \
                         Split the node into a short chain of nodes, or move some of the gating \
                         onto the objective that reaches the node.",
                        opt.node_id, npc.npc_id,
                    ),
                });
            }
        }
    }
    Ok(())
}

/// `DW0361`: two distinct generated artifacts sanitize to the same name, so one
/// would silently overwrite the other in the emitted pack.
pub const DW_NAME_COLLISION: DwCode = DwCode::every_version("DW0361");

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

/// The canonical pretty-printed bytes of an emitted JSON artifact — the same
/// rendering [`put_json`] writes, factored out so collision detection can compare
/// artifacts byte-for-byte before inserting.
fn json_bytes(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(value).expect("json serializes");
    bytes.push(b'\n');
    bytes
}

/// `DW0360`: an anchor-bearing quest/trigger effect names an anchor that resolves
/// to no world position in the assembled build. Validation-tier content mistake
/// (a typo'd or unassembled anchor), reported as a build diagnostic because only
/// the assembled world knows which anchors actually exist.
pub const DW_EFFECT_ANCHOR_UNRESOLVED: DwCode = DwCode::every_version("DW0360");

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
    crate::nav::needs_world(plan)
        || !plan.campaign.quests.content.waves.is_empty()
        || crate::clearance::has_bodies(plan)
}

/// Fail the build if any campaign effect — at **every effect root**, at **any
/// nesting depth** — names an anchor that resolves to no world position
/// (`DW0360`). This is the single resolved-anchor-or-diagnostic seal over the
/// whole anchor-bearing effect surface ([`QuestEffect::anchor_refs`], the
/// nesting-aware sibling of [`QuestEffect::nested_effect_lists`]).
///
/// It exists because every anchor consumer in [`emit_quest_effect`] fails *open*:
/// `open-gate`/`close-gate` scan `plan.anchors` for a name match and simply fall
/// out of the loop, `set-block`/`set-checkpoint`/`play-sound`/`damage-players`
/// bail out of an `if let Some(pos)`, and a cutscene waypoint silently degrades to
/// `[0, BASE_Y, 0]`. A single typo'd anchor therefore used to emit **nothing** —
/// a gate that never opens, a checkpoint that never binds — and shipped a broken
/// delve into the owner's one QA hour. `DW0142` catches what it can at DSL time,
/// but it only sees an area's declared anchor set (pool areas and cross-area
/// camera anchors are deferred to here), so this is the backstop that makes the
/// rule total.
///
/// **Total means total**. The roots come from
/// [`crate::plan::for_each_effect_root`] — the one enumeration
/// [`all_campaign_effects`], the staged-walk timeline and both halves of
/// `compiler::flow` also walk. This walk used to hand-list three of the five, so a
/// typo'd anchor in a `traps[].payload` or a dialogue option's `set-checkpoint`
/// `on_respawn` bundle was never asked the question at all: the build stayed
/// green, `trap_fire_<trap>.mcfunction` shipped with the `open-gate` simply
/// absent, and the delve the owner played had a trap that springs and does
/// nothing. A backstop that reaches four fifths of the surface is exactly the
/// silent-drop class it was written to end, so the roots are now inherited rather
/// than re-listed and a sixth root cannot be forgotten here.
fn check_effect_anchors(plan: &Plan) -> Result<(), BuildFailure> {
    let c = plan.campaign;
    // (json pointer, effect verb, anchor) for every anchor reference in the
    // campaign, deep, in deterministic content order.
    let mut refs: Vec<(String, &'static str, String)> = Vec::new();
    // This seal covers the verbs that fail OPEN — the ones whose anchor consumer
    // shrugs and emits nothing. The spec-0022 payload verbs (`volley`,
    // `collapse`) fail CLOSED instead: `plan_payload_verbs` resolves their volumes
    // with `?` and reports `DW0447`, naming the verb, the volume and the anchor.
    // Widening this walk to R4/R5 put those anchors in reach of the
    // generic message for the first time, which would have preempted the specific
    // one for no gain — so where `DW0447` runs, the fail-closed verbs keep it.
    //
    // **Only where it runs.** `plan_payload_verbs` lives inside the world block,
    // so it is reached only when the campaign assembles a world. A payload verb
    // does NOT imply that: nothing confines `volley`/`collapse` to
    // `traps[].payload` — `dsl::validate` reaches them via
    // `for_each_trap_payload_deep` inside the traps loop, which validates them
    // where they are rather than forbidding them elsewhere, and they are ordinary
    // variants of the shared effect enum. A `volley` on a quest's `on_complete` in
    // a campaign with no traps, no waves, no bodies and no walkable critical leg
    // therefore reaches emission with `DW0447` unreachable. Deferring there would
    // trade a specific message for SILENCE, so the deferral is conditional on the
    // proof actually running and this seal keeps that corner itself.
    let payload_verbs_are_proven = assembles_world(plan);
    fn descend(
        path: String,
        eff: &QuestEffect,
        payload_verbs_are_proven: bool,
        refs: &mut Vec<(String, &'static str, String)>,
    ) {
        let defer_to_dw0447 =
            payload_verbs_are_proven && (eff.volley().is_some() || eff.collapse().is_some());
        if !defer_to_dw0447 {
            for (suffix, anchor) in eff.anchor_refs() {
                refs.push((
                    format!("{path}/{suffix}"),
                    eff.verb(),
                    anchor.as_str().to_string(),
                ));
            }
        }
        for (pseg, _kseg, list) in eff.nested_effect_lists_labeled() {
            for (j, inner) in list.iter().enumerate() {
                descend(
                    format!("{path}/{pseg}/{j}"),
                    inner,
                    payload_verbs_are_proven,
                    refs,
                );
            }
        }
    }
    crate::plan::for_each_effect_root(c, &mut |site, effs| {
        for (i, eff) in effs.iter().enumerate() {
            descend(
                format!("{}/{i}", site.path),
                eff,
                payload_verbs_are_proven,
                &mut refs,
            );
        }
    });
    for (path, verb, anchor) in refs {
        if anchor_point_any(plan, &anchor).is_some() {
            continue;
        }
        return Err(BuildFailure::Diagnostic {
            code: DW_EFFECT_ANCHOR_UNRESOLVED,
            message: format!(
                "`{verb}` at `{path}` names anchor `{anchor}`, which resolves to no \
                 position in the assembled world — the effect would emit nothing at \
                 all (a gate that never opens, a block never placed, a camera stuck \
                 at the world origin). Anchor names come from prefab metadata: use \
                 one the area's prefab/pool actually exposes, and do NOT invent one."
            ),
        });
    }
    Ok(())
}

/// Fail the build if any `spawn-wave` effect references a wave whose spawn
/// position cannot be resolved (`DW0310`). Such a wave emits no `spawn_<wave>`
/// function, yet the effect still emits a `function <ns>:spawn_<wave>` call — a
/// silently dangling reference that would never spawn the wave at runtime. A
/// compile-time diagnostic turns that content mistake into a loud build failure
/// instead of a missing enemy the QA hour has to notice.
fn check_wave_spawns(plan: &Plan) -> Result<(), BuildFailure> {
    // `all_campaign_effects` is the emitter's own traversal — the one that decides
    // where a `function <ns>:spawn_<wave>` call is written. Reading the spawn sites
    // from it is what makes this check see exactly the calls that ship, including
    // the ones nested in a `sequence` step, an `on_respawn` bundle or a trap
    // payload. Scanning only the top-level chains, as this did, is how the island's
    // round-21 build shipped two `spawn_…` calls with nothing behind them.
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for e in all_campaign_effects(plan.campaign) {
        if let Some(wave) = e.spawn_wave() {
            let id = wave.as_str();
            if seen.insert(id) && wave_spawn_pos(plan, id).is_none() {
                return Err(BuildFailure::Diagnostic {
                    code: DW_WAVE_SPAWN_UNRESOLVED,
                    message: format!(
                        "`spawn-wave` references wave `{id}`, but its spawn anchor is \
                         not placed in any assembled area — the emitted \
                         `spawn_{safe}` call would dangle and the wave never spawn. \
                         Ensure a quest in the wave's area fires the `spawn-wave`, or \
                         that the wave `anchor` exists in that area's prefab pool.",
                        safe = plan::safe_local(id),
                    ),
                });
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Effect bundles: audience and command source (spec-0018 party progression)
// ---------------------------------------------------------------------------

/// Who an emitted effect bundle speaks to, and whether it has an acting player.
///
/// **Party state (spec-0018).** Objective/quest/flag progression lives on the
/// [`plan::PARTY`] holder, so a *party-fact* effect (`set-flag`, `open-gate`,
/// `spawn-*`, `set-checkpoint`, a driver start, …) names no player at all and
/// fires exactly once, under every audience. Only *player-facing* effects
/// (`narrate`, `play-sound`, `damage-players`, `give-item`) need a selector, and
/// that selector is what this enum decides.
///
/// **The scheduled-executor bug this still models (AUDIT-P0).** Vanilla's
/// `schedule function …` re-invokes a function with the **server** command
/// source: no executor, so `@s` resolves to nothing and every `@s`-addressed
/// command silently fails. Under party state a scheduled `set-flag` writes
/// `#party` and is immune by construction; what remains executor-dependent is
/// exactly one thing — a `carrier: "one"` `give-item`, which needs the acting
/// player — and [`Audience::Scheduled`] is where that has no answer (rejected at
/// validate time, `DW0357`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Audience {
    /// A **party event** entered as one player (`complete_<obj>`, `complete_q_*`,
    /// `trig_<id>`): player-facing effects address `@a` (the whole party sees the
    /// beat), and `@s` is available as the completing player for a `carrier:
    /// "one"` hand-off.
    Party,
    /// A **scheduled** bundle (`mv_arrive_*`, `ma_arrive_*`, `seq_*_<i>`), entered
    /// with the server command source: player-facing effects address `@a`; there
    /// is no `@s` at all.
    Scheduled,
    /// **One player's own** bundle: a checkpoint `on_respawn` (fired per
    /// respawning player) and a stealth `on_caught` (fired at the spotted
    /// player). Player-facing effects address `@s` — re-broadcasting one player's
    /// death or exposure to the party would duplicate their kit and their
    /// narration.
    Solo,
}

impl Audience {
    /// The selector a player-facing command uses.
    fn selector(self) -> &'static str {
        match self {
            Audience::Party | Audience::Scheduled => "@a",
            Audience::Solo => "@s",
        }
    }

    /// Is there an acting player (`@s`) in this command source?
    fn has_actor(self) -> bool {
        !matches!(self, Audience::Scheduled)
    }
}

/// **The audience each effect root's bundle is emitted under.**
///
/// One function rather than seven literals at seven call sites, because
/// `EffectRootKind::runs_with_acting_player` states the same fact for validation
/// (`DW0503`) and the two must not drift: a root whose emitted bundle quietly
/// changed audience would turn a validated `player`-scoped read into an `@s` in
/// a sourceless function, with every check green. `root_audience_matches_the_dsl`
/// binds them by equality over the closed root set.
///
/// Byte impact: none. Each arm is the literal the call site already passed.
fn root_audience(kind: delvewright_dsl::EffectRootKind) -> Audience {
    use delvewright_dsl::EffectRootKind as K;
    match kind {
        // Dispatched `as @a` from the tick: the acting player is `@s`.
        K::ObjectiveComplete | K::QuestComplete => Audience::Party,
        // The dying / respawning player's own beat.
        K::DialogueRespawn | K::OnDeath => Audience::Solo,
        // The buying player's own beat: the handler is dispatched
        // `as @a[scores={…}]`, so `@s` is whoever pressed the button.
        K::ShopOffer => Audience::Solo,
        // Polled on the tick with no executor.
        K::Trigger | K::TrapPayload | K::ShortcutUnlock => Audience::Scheduled,
    }
}

/// Emit a quest effect, wrapping every command it produces in the effect's
/// **party** flag guard when it declares `requires_flags` and/or `forbids_flags`
/// (DSL v0.6). Flags are party state (spec-0018), so the guard is one form under
/// every audience: `execute if score #party dw.f_<flag> matches 1 [… per required
/// flag] unless score #party dw.f_<flag> matches 1 [… per forbidden flag] run
/// <command>`. `unless … matches 1` deliberately treats an **unset** score as
/// "not set" (flag scores are never pre-initialized to 0, so a `scores={…=..0}`
/// selector would not work). An ungated effect (both lists empty) is emitted
/// verbatim.
fn emit_gated_effect(plan: &Plan, eff: &QuestEffect, aud: Audience, body: &mut Vec<String>) {
    let gate = eff.gate();
    let flags = gate.requires_flags;
    let forbids = gate.forbids_flags;
    let mut inner: Vec<String> = Vec::new();
    emit_quest_effect(plan, eff, aud, &mut inner);
    if gate.is_empty() {
        body.extend(inner);
        return;
    }
    // DSL v0.10: the numeric terms join the flag terms in one guard, in gate
    // field order. `state_clauses` yields ` if score …` (leading space); this
    // guard is built in the space-TERMINATED form, so the clauses are re-spaced
    // rather than concatenated verbatim.
    let guard: String = flags
        .iter()
        .map(|f| {
            format!(
                "if score {} {} matches 1 ",
                plan::PARTY,
                plan::flag_score(f.as_str())
            )
        })
        .chain(forbids.iter().map(|f| {
            format!(
                "unless score {} {} matches 1 ",
                plan::PARTY,
                plan::flag_score(f.as_str())
            )
        }))
        .chain(
            state_clauses(plan, gate.requires_state, false)
                .into_iter()
                .map(|c| format!("{c} ")),
        )
        .collect();
    for line in inner {
        body.push(with_execute_prefix(&guard, line));
    }
}

/// Splice an `execute` prefix (already space-terminated, e.g. `if score #party
/// dw.f_x matches 1 `) onto one emitted command, folding into a leading
/// `execute` when there is one rather than nesting a second `execute … run
/// execute …`.
fn with_execute_prefix(prefix: &str, line: String) -> String {
    if prefix.is_empty() {
        return line;
    }
    match line.strip_prefix("execute ") {
        Some(rest) => format!("execute {prefix}{rest}"),
        None => format!("execute {prefix}run {line}"),
    }
}

/// Emit a whole effect bundle for `aud` (see [`Audience`]).
fn emit_effect_bundle<'a>(
    plan: &Plan,
    effects: impl IntoIterator<Item = &'a QuestEffect>,
    aud: Audience,
) -> Vec<String> {
    let mut body: Vec<String> = Vec::new();
    for e in effects {
        emit_gated_effect(plan, e, aud, &mut body);
    }
    body
}

// ---------------------------------------------------------------------------
// DSL v0.10 runtime state (spec-0031) — the numeric half of the one gate
// ---------------------------------------------------------------------------

/// The declaration of a runtime datum, or `None` if the campaign declares none
/// by that id (which validation has already rejected — `DW0500`).
fn state_decl<'a>(plan: &'a Plan, id: &StateId) -> Option<&'a delvewright_dsl::StateDecl> {
    plan.campaign.quests.content.state_decl(id.as_str())
}

/// **Who** holds a datum's value: the party fake player for a `party` datum, the
/// acting player for a `player` one.
///
/// The whole content of the declared scope, in one function. An undeclared datum
/// answers `#party` so that a campaign which failed validation still emits
/// something well-formed rather than panicking mid-build.
fn state_holder(plan: &Plan, id: &StateId) -> String {
    match state_decl(plan, id).map(|s| s.scope) {
        Some(StateScope::Player) => "@s".to_string(),
        _ => plan::PARTY.to_string(),
    }
}

/// A datum's declared `initial` — the value it starts at and the value
/// `clear-state` returns it to.
fn state_initial(plan: &Plan, id: &StateId) -> i32 {
    state_decl(plan, id).map(|s| s.initial).unwrap_or(0)
}

/// The `execute` sub-clauses a gate's **numeric** terms contribute, each already
/// prefixed with a space, e.g. ` if score #party dw.s_purse matches 500..`.
///
/// With `negate` the clauses assert the gate is NOT satisfied — one clause per
/// term, which is the "any single term failing shuts it" form the trap arming
/// tick needs (there, a gate closing has to be expressible as its own condition).
///
/// Empty for a gate with no comparison, which is every pre-0.10 campaign — so
/// splicing this into an existing guard moves no existing command by a byte.
fn state_clauses(plan: &Plan, cmps: &[StateCompare], negate: bool) -> Vec<String> {
    cmps.iter()
        .map(|c| {
            let holder = state_holder(plan, &c.state);
            let obj = plan::state_score(c.state.as_str());
            // `equals`/`at-least`/`at-most` are `if … matches <range>`;
            // `not-equals` is the same range under `unless`. Negation flips the
            // keyword and nothing else, so the two readings can never disagree
            // about what the range means.
            let (positive, range) = match c.op {
                CompareOp::Equals => (true, format!("{}", c.value)),
                CompareOp::NotEquals => (false, format!("{}", c.value)),
                CompareOp::AtLeast => (true, format!("{}..", c.value)),
                CompareOp::AtMost => (true, format!("..{}", c.value)),
            };
            let kw = if positive != negate { "if" } else { "unless" };
            format!("{kw} score {holder} {obj} matches {range}")
        })
        .collect()
}

/// [`state_clauses`] in the **space-prefixed** form the `execute` builders in
/// this module splice onto a growing condition (` if score … if score …`).
fn state_cond(plan: &Plan, cmps: &[StateCompare], negate: bool) -> String {
    state_clauses(plan, cmps, negate)
        .into_iter()
        .map(|c| format!(" {c}"))
        .collect()
}

/// The whole gate as one space-prefixed `execute` condition — flags, then the
/// negative flags, then the numeric terms, in gate field order.
///
/// Empty for an ungated site, so a caller that splices it in unconditionally
/// emits exactly what it emitted before v0.10.
fn gate_cond(plan: &Plan, gate: Gate<'_>) -> String {
    let mut out = String::new();
    for f in gate.requires_flags {
        out.push_str(&format!(
            " if score {} {} matches 1",
            plan::PARTY,
            plan::flag_score(f.as_str())
        ));
    }
    for f in gate.forbids_flags {
        out.push_str(&format!(
            " unless score {} {} matches 1",
            plan::PARTY,
            plan::flag_score(f.as_str())
        ));
    }
    out.push_str(&state_cond(plan, gate.requires_state, false));
    out
}

/// The commands that force a gate's numeric terms to be satisfied (`satisfy`) or
/// violated — used by the generated PackTest preambles, which have to *drive* a
/// gate rather than merely read it.
///
/// A `not-equals` gate is satisfied by any other value, and `value + 1` is the
/// deterministic choice; `at-least`/`at-most` are satisfied at the boundary. The
/// violating value is the mirror. Both directions are needed because the flag
/// gate's own templates already prove both truth-table rows, and a numeric gate
/// that only ever proved the open row would be the weaker test.
fn state_drive_lines(plan: &Plan, cmps: &[StateCompare], satisfy: bool) -> Vec<String> {
    cmps.iter()
        .map(|c| {
            let v = match (c.op, satisfy) {
                // The boundary satisfies `equals`, `at-least` and `at-most`.
                (CompareOp::Equals, true)
                | (CompareOp::AtLeast, true)
                | (CompareOp::AtMost, true)
                | (CompareOp::NotEquals, false) => c.value,
                // One step past it violates them — and satisfies `not-equals`.
                (CompareOp::Equals, false)
                | (CompareOp::AtMost, false)
                | (CompareOp::NotEquals, true) => c.value.wrapping_add(1),
                (CompareOp::AtLeast, false) => c.value.wrapping_sub(1),
            };
            format!(
                "scoreboard players set {} {} {v}",
                state_holder(plan, &c.state),
                plan::state_score(c.state.as_str())
            )
        })
        .collect()
}

/// Every declared runtime datum, in declared order (empty for a pre-0.10
/// campaign, which is what keeps its setup byte-identical).
fn declared_states(c: &delvewright_dsl::Campaign) -> &[delvewright_dsl::StateDecl] {
    &c.quests.content.state
}

/// **The one runtime region write** (DSL v0.10, spec-0031): fill the inclusive box
/// `region` with `block`, optionally restricted to the cells currently holding
/// `only`.
///
/// Every verb that writes a region at runtime goes through here — `fill-region`
/// (author's box, author's block), `clear-region` (author's box, air),
/// `close-gate` (the gate anchor's box and its declared block) and `open-gate`
/// (the gate anchor's box, air, `replace`-filtered to the gate block so an opened
/// threshold never scrubs anything that drifted into it). The `replace` filter is
/// the only difference between the four, which is why it is a parameter here
/// rather than four spellings of `fill` in four match arms.
fn fill_region_command(region: ([i32; 3], [i32; 3]), block: &str, only: Option<&str>) -> String {
    let (from, to) = region;
    let filter = match only {
        Some(o) => format!(" replace {o}"),
        None => String::new(),
    };
    format!(
        "fill {} {} {} {} {} {} {block}{filter}",
        from[0], from[1], from[2], to[0], to[1], to[2]
    )
}

/// The block a cleared region is written with. Named because three verbs share it.
const AIR: &str = "minecraft:air";

/// Emit a quest effect's commands into `body`, addressing `aud`.
fn emit_quest_effect(plan: &Plan, eff: &QuestEffect, aud: Audience, body: &mut Vec<String>) {
    let ns = &plan.namespace;
    let who = aud.selector();
    match eff {
        QuestEffect::OpenGate { anchor, .. } => {
            // Find the gate anchor across areas (first match).
            for ((_, name), resolved) in &plan.anchors {
                if name == anchor.as_str()
                    && let ResolvedAnchor::Gate { from, to, block } = resolved
                {
                    // A region write whose box and filter the gate anchor supplies.
                    body.push(fill_region_command((*from, *to), AIR, Some(block)));
                    // …and take the seal's answer down with the seal.
                    // The hitboxes exist exactly while the region is solid: an
                    // opened threshold that still says "the way is sealed" is a
                    // lie, and an invisible box left standing in a doorway
                    // swallows right-clicks aimed through it.
                    if let Some(s) = seal_hint_for(plan, anchor.as_str()) {
                        body.push(format!("kill @e[tag=dw_seal_{}]", s.safe));
                    }
                    return;
                }
            }
        }
        QuestEffect::CloseGate { anchor, .. } => {
            // The physical dual of `open-gate`: fill the gate region with the block
            // the anchor declares (basalt boulder, iron bars, …), sealing it back
            // into a wall. A blockless gate anchor is rejected at validate-time
            // (`DW0343`), so the resolved `block` is the real fill here.
            for ((_, name), resolved) in &plan.anchors {
                if name == anchor.as_str()
                    && let ResolvedAnchor::Gate { from, to, block } = resolved
                {
                    // The same region write, with the block the anchor declares.
                    body.push(fill_region_command((*from, *to), block, None));
                    // Arm the seal's answer:
                    // a wall the party walks back to and presses must say
                    // something. Guarded on absence, so a re-fired `close-gate`
                    // never stacks a second set of hitboxes.
                    if let Some(s) = seal_hint_for(plan, anchor.as_str()) {
                        body.push(format!(
                            "execute unless entity @e[tag=dw_seal_{}] run function {ns}:{}",
                            s.safe,
                            seal_arm_fn(&s.safe)
                        ));
                    }
                    return;
                }
            }
        }
        QuestEffect::CampaignComplete { .. } => {
            body.push(format!("function {ns}:campaign_complete"));
        }
        QuestEffect::GiveItem {
            item, count, name, ..
        } => {
            let comp = match name {
                Some(n) => format!("[custom_name={}]", tr_with(n, &[("italic", json!(false))])),
                None => String::new(),
            };
            // spec-0018: a quest beat arms the whole party (`@a`) unless the item
            // declares `carrier: "one"` — one quest prop, handed to the player
            // whose action earned it (`@s`), for the party to pass around. A
            // `carrier: "one"` in a scheduler-only bundle has no acting player and
            // is rejected at validate time (`DW0357`), so `has_actor` can only be
            // false here for the party-wide default.
            let target = if eff.gives_to_one() && aud.has_actor() {
                "@s"
            } else {
                who
            };
            body.push(format!("give {target} {item}{comp} {count}"));
        }
        QuestEffect::SetFlag { flag, .. } => {
            // Party state (spec-0018): one holder, so any player's action sets the
            // story flag for everyone — and a scheduled bundle can set it too (the
            // AUDIT-P0 `@s`-in-a-schedule class of bug is structurally gone).
            body.push(format!(
                "scoreboard players set {} {} 1",
                plan::PARTY,
                plan::flag_score(flag.as_str())
            ));
        }
        // --- DSL v0.10 runtime state (spec-0031) ------------------------------
        // Each of the three is a plain `scoreboard players …` against the datum's
        // declared holder. `clear-state` WRITES the declared `initial` rather
        // than `reset`ting the score: a reset score is *absent*, and an absent
        // score makes `unless … matches` true — so a cleared datum would silently
        // satisfy a `not-equals` comparison against its own initial value.
        QuestEffect::SetState { state, value, .. } => {
            body.push(format!(
                "scoreboard players set {} {} {value}",
                state_holder(plan, state),
                plan::state_score(state.as_str())
            ));
        }
        QuestEffect::AddState { state, amount, .. } => {
            // `add` / `remove` rather than one signed `add`: vanilla's `add` takes
            // an unsigned operand and `remove` is its documented dual.
            let holder = state_holder(plan, state);
            let obj = plan::state_score(state.as_str());
            if *amount < 0 {
                body.push(format!(
                    "scoreboard players remove {holder} {obj} {}",
                    amount.unsigned_abs()
                ));
            } else {
                body.push(format!("scoreboard players add {holder} {obj} {amount}"));
            }
        }
        QuestEffect::ClearState { state, .. } => {
            body.push(format!(
                "scoreboard players set {} {} {}",
                state_holder(plan, state),
                plan::state_score(state.as_str()),
                state_initial(plan, state)
            ));
        }
        // spec-0032. The verb is one `function` call, because everything a stake
        // drop does — the retention policy, the forfeit arithmetic, the
        // compile-time placement table, the marker — is shared between every site
        // that can leave one, and a bundle inlined at each site would be that
        // chain copied per firing.
        QuestEffect::DropStake { stake, .. } => {
            body.push(format!(
                "function {ns}:stk_drop_{}",
                plan::safe_local(stake.as_str())
            ));
        }
        QuestEffect::SpawnWave { wave, .. } => {
            body.push(format!(
                "function {ns}:spawn_{}",
                plan::safe_local(wave.as_str())
            ));
        }
        // --- DSL v0.4 effects ---
        QuestEffect::Narrate {
            text, style, sound, ..
        } => {
            emit_narrate(text, *style, sound.as_deref(), who, body);
        }
        QuestEffect::SetBlock { anchor, block, .. } => {
            if let Some(pos) = anchor_point_any(plan, anchor.as_str()) {
                body.push(format!("setblock {} {} {} {block}", pos[0], pos[1], pos[2]));
            }
        }
        // --- DSL v0.10 region writes (spec-0031) ---
        // The general spelling of what `open-gate`/`close-gate` do to a gate
        // anchor's box, through the same one command builder. An unresolvable box
        // emits nothing — a dangling `region/anchor` is `DW0142`/`DW0355` at
        // validation, not a silently mis-aimed fill here.
        QuestEffect::FillRegion { .. } | QuestEffect::ClearRegion { .. } => {
            if let Some((zone, block)) = eff.region_write()
                && let Some(region) = plan.zone_box(zone)
            {
                body.push(fill_region_command(region, block.unwrap_or(AIR), None));
            }
        }
        QuestEffect::DespawnNpc { npc, .. } => {
            // Removes both the body and the interaction hitbox — both carry the
            // per-npc id tag (spec-0008 §5).
            body.push(format!(
                "kill @e[tag=dw_npc_{}]",
                plan::safe_local(npc.as_str())
            ));
        }
        QuestEffect::MoveNpc { npc, to_anchor, .. } => {
            body.push(format!(
                "function {ns}:{}",
                movenpc_fn(npc.as_str(), to_anchor.as_str(), &crate::nav::gate_key(eff),)
            ));
        }
        QuestEffect::Cutscene { .. } => {
            // Shape is policed at validation (`DW0199`); an unshaped cutscene
            // resolves to no shots and emits no call rather than a dangling one.
            if let Some(shots) = eff.cutscene_shots().filter(|s| !s.is_empty()) {
                body.push(format!("function {ns}:{}", cutscene_fn(&shots)));
            }
        }
        // --- DSL v0.5 effects (spec-0010) ---
        // Dimension-global instant cuts. The daylight/weather cycles are frozen by
        // environment sealing (`advance_time`/`advance_weather false`), so the set
        // state persists until the next cut. No selector: `/time set` and
        // `/weather` act on the whole dimension.
        QuestEffect::SetTime { time, .. } => {
            body.push(format!("time set {}", time.token()));
        }
        QuestEffect::SetWeather { weather, .. } => {
            body.push(format!("weather {}", weather.token()));
        }
        // --- DSL v0.6 effects (spec-0012 checkpoints, spec-0014 stealth + sound) ---
        QuestEffect::PlaySound {
            sound,
            at,
            volume,
            pitch,
            ..
        } => {
            emit_play_sound(plan, sound, at.as_ref(), *volume, *pitch, who, body);
        }
        QuestEffect::DamagePlayers {
            amount,
            within,
            damage_type,
            ..
        } => {
            emit_damage_players(plan, *amount, within.as_ref(), *damage_type, who, body);
        }
        QuestEffect::SetCheckpoint { anchor, on_respawn } => {
            emit_set_checkpoint(plan, anchor.as_str(), on_respawn, body);
        }
        QuestEffect::Bonfire {
            anchor, on_rest, ..
        } => {
            // Arm the rest affordance (spec-0016 §1): summon the interaction
            // entity the party right-clicks to rest. Guarded on absence so a
            // re-fired beat never stacks a second affordance (and so a `bonfire`
            // reached twice is idempotent). Nothing else happens here — the
            // checkpoint moves when the party REST, not when the beat fires.
            if let Some(bf) = plan.bonfire_for(anchor.as_str(), on_rest) {
                let v = ent_xyz(bf.pos);
                let i = bf.index;
                body.push(format!(
                    "execute unless entity @e[tag=dw_bonfire_{i}] run summon minecraft:interaction {} {} {} {{width:1.0f,height:2.0f,response:1b,Invulnerable:1b,Tags:[{FIXTURE_NBT}\"dw_bonfire_{i}\"]}}",
                    v[0], v[1], v[2]
                ));
                // …and the visible hardware, under the same absence guard so a
                // re-fired beat never stacks a second one. A rest point the
                // player cannot see is the same soft-lock class as an invisible
                // unlock lever (`DW0420`). Never retired: a bonfire is not
                // consumed by resting at it.
                let hw = crate::affordance::hardware_tag(&format!("dw_bonfire_{i}"));
                body.push(format!(
                    "execute unless entity @e[tag={hw}] run {}",
                    affordance_hardware(v, &format!("dw_bonfire_{i}"), "minecraft:campfire")
                ));
            }
        }
        QuestEffect::BeginStealth {
            zones, grace_ticks, ..
        } => {
            if let Some(beat) = plan.stealth_for(zones, *grace_ticks) {
                body.push(format!("function {ns}:stealth_begin_{}", beat.index));
            }
        }
        QuestEffect::EndStealth => {
            body.push("scoreboard players set #stealth dw.sys 0".to_string());
        }
        // spec-0022 trap-payload verbs. Both lower to a call into a generated
        // function whose body is the PROVEN geometry (per-cell velocity vectors
        // / settled debris), so the effect site itself carries no coordinates.
        QuestEffect::Volley { .. } => {
            body.push(format!("function {ns}:{}", volley_fn(eff)));
        }
        QuestEffect::Collapse { .. } => {
            body.push(format!("function {ns}:{}", collapse_fn(eff)));
        }
        // --- DSL v0.6 actor staging effects (spec-0014) ---
        QuestEffect::SpawnActor { actor, .. } => {
            body.push(format!(
                "function {ns}:spawn_actor_{}",
                plan::safe_local(actor.as_str())
            ));
        }
        QuestEffect::DespawnActor { actor, style, .. } => {
            let declares_drops = plan
                .campaign
                .quests
                .content
                .actors
                .iter()
                .any(|a| a.id.as_str() == actor.as_str() && !a.drops.is_empty());
            emit_despawn_actor(actor.as_str(), *style, declares_drops, body);
        }
        QuestEffect::MoveActor {
            actor, to_anchor, ..
        } => {
            body.push(format!(
                "function {ns}:{}",
                moveactor_fn(
                    actor.as_str(),
                    to_anchor.as_str(),
                    &crate::nav::gate_key(eff),
                )
            ));
        }
        QuestEffect::UnleashActor { actor, .. } => {
            body.push(format!(
                "function {ns}:unleash_{}",
                plan::safe_local(actor.as_str())
            ));
        }
        QuestEffect::Sequence { steps } => {
            body.push(format!("function {ns}:{}", sequence_fn(steps)));
        }
        QuestEffect::SpawnNpc { npc, .. } => {
            body.push(format!("function {ns}:{}", spawn_npc_fn(npc.as_str())));
        }
        // --- DSL v0.10 status effects (spec-0031) -----------------------------
        // Vanilla `effect give` / `effect clear`, through the SAME formatter the
        // engine's own night-vision clock has used since v0.6
        // (`effect_give_command`) — the hard-coded case is now one configured use
        // of the general verb's emission rather than a private copy of it.
        //
        // No `tag=!dw_cutscene` guard, deliberately: a status effect is not
        // inherently harm (regeneration, night vision, glowing), and the engine's
        // pre-existing region-scoped grant has never carried one. Where an author
        // wants a beat to spare an observer, the `in` filter and the effect gate
        // both say so explicitly.
        QuestEffect::GiveEffect { .. } => {
            if let Some((effect, seconds, amplifier, hide, within)) = eff.give_effect() {
                let Some(sel) = effect_selector(plan, who, within) else {
                    return;
                };
                body.push(effect_give_command(&sel, effect, seconds, amplifier, hide));
            }
        }
        QuestEffect::ClearEffect { .. } => {
            if let Some((effect, within)) = eff.clear_effect() {
                let Some(sel) = effect_selector(plan, who, within) else {
                    return;
                };
                // Vanilla's own two spellings: with an id, or bare for "all".
                body.push(match effect {
                    Some(id) => format!("effect clear {sel} {id}"),
                    None => format!("effect clear {sel}"),
                });
            }
        }
        // --- DSL v0.10 teleport (spec-0031) -----------------------------------
        // ONE command, and its selector is the volume — never the effect's
        // audience. `who` is deliberately unused here: a teleport moves what is
        // INSIDE the box, and a box does not have a party. The selector carries
        // the six box terms plus the one class exclusion every box-narrowed
        // entity selector in this engine carries — `tag=!dw_fixture`, and no
        // `type=`, no `limit=`, no `sort=`. That is what makes the selection
        // total over BODIES, and `crates/compiler/tests/v10_teleport.rs` asserts
        // exactly that against the emitted string. See `QuestEffect::Teleport`
        // for why a machinery-TYPE exemption (which `lethal_volumes[]` must
        // carry) would be wrong here, and `crate::affordance` for the class that
        // stands in its place.
        QuestEffect::Teleport { .. } => {
            // A call into the generated function, exactly as `volley` and
            // `collapse` do: the body is proven geometry, and a body that only
            // ever exists inline is a body no runtime test can call.
            if teleport_command(plan, eff).is_some() {
                body.push(format!("function {ns}:{}", teleport_fn(eff)));
            }
        }
    }
}

/// The `effect give`/`effect clear` target selector for a v0.10 status-effect
/// verb: the effect's audience, narrowed by the declared `in` box when there is
/// one.
///
/// `None` when the filter's anchor does not resolve — referential validation
/// already reports that (`DW0142`), and emitting a selector with a blank box
/// would be an invalid command rather than a diagnosis.
fn effect_selector(
    plan: &Plan,
    who: &str,
    within: Option<&delvewright_dsl::StealthZone>,
) -> Option<String> {
    match within {
        None => Some(who.to_string()),
        Some(zone) => {
            let (lo, hi) = plan.zone_box(zone)?;
            Some(format!("{who}[{}]", box_selector_args(lo, hi)))
        }
    }
}

/// Vanilla's `effect give`, always in its full five-token form.
///
/// The engine has emitted this since v0.6 and exposed no verb for it; this is the
/// one place that writes the command, used by both the author-facing
/// `give-effect` and the night-vision area mitigation
/// ([`night_vision_fns`]). The full form — duration, amplifier and
/// `hideParticles` all present — is what the mitigation already emitted, so
/// routing it through here is byte-identical for every existing campaign, and it
/// leaves nothing to a vanilla default that a future version could re-pick.
fn effect_give_command(
    selector: &str,
    effect: &str,
    seconds: u32,
    amplifier: u32,
    hide_particles: bool,
) -> String {
    format!("effect give {selector} {effect} {seconds} {amplifier} {hide_particles}")
}

/// Emit a `despawn-actor` inline (spec-0014). Both styles target the actor body tag
/// `dw_actor_<id>` (so a puppet **or** an unleashed twin is removed — re-caging is
/// despawn + spawn). `kill` plays the vanilla death animation in place; `vanish`
/// relocates the (Silent) body far below the floor first, so the death sequence
/// plays entirely out of the players' view — a silent removal from two intended
/// primitives (tp + kill).
///
/// **The relocation must be per-actor** (round-8 island QA, caught on a live
/// server). `tp <targets> ~ -128 ~` resolves `~ ~` against the **command source**,
/// not against each target, and every path that reaches a `despawn-actor` — a
/// `move-actor`'s `on_arrive`, a `sequence` step, a trigger bundle — runs from the
/// server source, whose position is world spawn. So `vanish` dropped the body at
/// (spawn.x, -128, spawn.z) rather than straight down its own column: the island's
/// herdsman, standing at `6.5, -55.5`, died at `10.0, -128.0, 9.0`. Invisible today
/// only because the `kill` lands on the very next line — but the intent of the
/// style is "out of sight, in place", and an actor that briefly exists at another
/// area's coordinates is wrong data, not a detail. `execute as … at @s` is the same
/// idiom [`emit_play_sound`] uses to make `~ ~ ~` resolve per entity.
fn emit_despawn_actor(
    actor: &str,
    style: delvewright_dsl::DespawnStyle,
    declares_drops: bool,
    body: &mut Vec<String>,
) {
    use delvewright_dsl::DespawnStyle;
    let safe = plan::safe_local(actor);
    // v0.9: a removal is not a death the player earned. Both styles
    // end in `/kill`, and a preserved drop chance survives a non-player kill, so
    // an elite the story re-cages (a souls re-seat) would shed its axe on every
    // rest. Strip the declaration off the body first; emitted only when the
    // actor declares drops, so every earlier campaign's despawn is byte-identical.
    if declares_drops {
        body.push(strip_drops_line(&format!("dw_actor_{safe}")));
    }
    match style {
        DespawnStyle::Kill => body.push(format!("kill @e[tag=dw_actor_{safe}]")),
        DespawnStyle::Vanish => {
            body.push(format!(
                "execute as @e[tag=dw_actor_{safe}] at @s run tp @s ~ -128 ~"
            ));
            body.push(format!("kill @e[tag=dw_actor_{safe}]"));
        }
    }
}

/// Emit a `play-sound` effect (DSL v0.6). `who` is the audience selector
/// (spec-0018: `@a` for a party beat, `@s` inside a solo `on_respawn`/`on_caught`
/// bundle). An `at: anchor` sound carries absolute coordinates, so every listener
/// hears it in the same place; the default `players` target is
/// listener-relative and, when a volume/pitch is declared (which forces an
/// explicit position), is emitted through `execute as <who> at @s run … ~ ~ ~` so
/// `~ ~ ~` resolves at each listener rather than at the command's own position.
/// `at: actor` never reaches emission — it is rejected at validate-time
/// (`DW0335`) until the actors surface lands.
fn emit_play_sound(
    plan: &Plan,
    sound: &str,
    at: Option<&delvewright_dsl::SoundAt>,
    volume: Option<f64>,
    pitch: Option<f64>,
    who: &str,
    body: &mut Vec<String>,
) {
    use delvewright_dsl::SoundAt;
    // Canonicalize a bare id to the default namespace so the emitted command is
    // explicit (`playsound` accepts either form).
    let sound = if sound.contains(':') {
        sound.to_string()
    } else {
        format!("minecraft:{sound}")
    };
    let pos = match at {
        Some(SoundAt::Anchor { anchor }) => match anchor_point_any(plan, anchor.as_str()) {
            Some(p) => Some(format!("{} {} {}", p[0], p[1], p[2])),
            None => return, // unresolved anchor (referential validation reports it)
        },
        Some(SoundAt::Actor { .. }) => return, // deferred: DW0335 at validate-time
        _ => None,                             // `players` (default): player-relative
    };
    let listener_relative = pos.is_none() && (volume.is_some() || pitch.is_some());
    let mut cmd = format!(
        "playsound {sound} master {}",
        if listener_relative { "@s" } else { who }
    );
    if pos.is_some() || volume.is_some() || pitch.is_some() {
        let p = pos.unwrap_or_else(|| "~ ~ ~".to_string());
        cmd.push_str(&format!(" {p}"));
        if volume.is_some() || pitch.is_some() {
            cmd.push_str(&format!(" {}", volume.unwrap_or(1.0)));
            if let Some(pt) = pitch {
                cmd.push_str(&format!(" {pt}"));
            }
        }
    }
    if listener_relative && who != "@s" {
        // `~ ~ ~` is the COMMAND's position, not the listener's — rebind so each
        // party member hears it at their own feet.
        cmd = format!("execute as {who} at @s run {cmd}");
    }
    body.push(cmd);
}

/// Emit a `damage-players` effect (DSL v0.6). `who` is the audience selector
/// (spec-0018): `@a` on a party beat — the hazard is a fact about the delve, so
/// it hits every party member once — and `@s` inside a solo `on_caught` /
/// `on_respawn` bundle, where exactly the one player it belongs to is hurt.
/// `amount` is in half-hearts (1 HP each); the type is a curated vanilla damage
/// type (default `minecraft:generic`). A `within` box narrows to players standing
/// inside the anchor-centred AABB — the same box model the stealth zone check
/// uses (no double-hit: each player is judged on their own position).
///
/// **`/damage` takes ONE entity** (the vendored command tree says
/// `amount: "single"`, and 1.21.11 refuses to load a whole function containing
/// `damage @a[…] …`), so the party form is reached by re-binding —
/// `execute as @a[…] run damage @s …` — not by widening the target. The solo
/// form keeps its `if entity @s[…]` guard.
///
/// Every form is guarded by `tag=!dw_cutscene`: a player watching a cutscene is
/// never harmed by campaign machinery (see [`CUTSCENE_TAG`]).
fn emit_damage_players(
    plan: &Plan,
    amount: u32,
    within: Option<&delvewright_dsl::StealthZone>,
    damage_type: Option<delvewright_dsl::DamageKind>,
    who: &str,
    body: &mut Vec<String>,
) {
    use delvewright_dsl::DamageKind;
    let kind = damage_type.unwrap_or(DamageKind::Generic).id();
    let filters = match within {
        Some(zone) => {
            // A blank box when the anchor is unresolved (referential validation
            // reports that, DW0142) — emit nothing rather than an invalid selector.
            let Some(pos) = anchor_point_any(plan, zone.anchor.as_str()) else {
                return;
            };
            let lo = [
                pos[0] - zone.extent[0] as i32,
                pos[1] - zone.extent[1] as i32,
                pos[2] - zone.extent[2] as i32,
            ];
            let size = [
                2 * zone.extent[0] as i32,
                2 * zone.extent[1] as i32,
                2 * zone.extent[2] as i32,
            ];
            format!(
                "x={},dx={},y={},dy={},z={},dz={},tag=!{CUTSCENE_TAG}",
                lo[0], size[0], lo[1], size[1], lo[2], size[2]
            )
        }
        // The bare form still needs the cutscene guard (see CUTSCENE_TAG: a
        // cutscene is pure observation — campaign machinery never harms a player
        // who is only watching).
        None => format!("tag=!{CUTSCENE_TAG}"),
    };
    if who == "@s" {
        body.push(format!(
            "execute if entity @s[{filters}] run damage @s {amount} {kind}"
        ));
    } else {
        body.push(format!(
            "execute as {who}[{filters}] run damage @s {amount} {kind}"
        ));
    }
}

/// Emit a `set-checkpoint` (DSL v0.6, spec-0012): the party-wide vanilla
/// `spawnpoint @a`, the `storage dw:cp pos` mirror other features read
/// (spec-0013 boundary return), and — when any checkpoint carries an
/// `on_respawn` hook — the active-checkpoint marker `#cp dw.sys` the respawn
/// dispatcher keys on. Party-wide via the explicit `@a`, regardless of the
/// caller's `@s` context.
fn emit_set_checkpoint(
    plan: &Plan,
    anchor: &str,
    on_respawn: &[QuestEffect],
    body: &mut Vec<String>,
) {
    if let Some(pos) = anchor_point_any(plan, anchor) {
        body.push(format!("spawnpoint @a {} {} {}", pos[0], pos[1], pos[2]));
        body.push(format!(
            "data modify storage dw:cp pos set value [{}, {}, {}]",
            pos[0], pos[1], pos[2]
        ));
        if plan.any_checkpoint() {
            let idx = plan
                .checkpoint_for(anchor, on_respawn)
                .map(|c| c.index)
                .unwrap_or(0);
            body.push(format!("scoreboard players set #cp dw.sys {idx}"));
        }
    }
}

/// The centre of a block cell on a horizontal axis, as the compiler writes it into
/// a `tp`. Vanilla's own respawn lands a player at `cell + 0.5` on X/Z, so the
/// re-seat has to agree with it or a correct respawn would visibly twitch. Written
/// through `f64` (not string concatenation) because `-16` centres on `-15.5`, not
/// `-16.5`; the value is exactly representable, so the text is deterministic.
fn center(cell: i32) -> String {
    format!("{:.1}", cell as f64 + 0.5)
}

/// **The death-position seam — measured, and the answer is that it needs nothing.**
///
/// Every command this returns is prepended to the corpse-side branch of
/// `cp_respawn_check`, ahead of `on_death_fire`, so whatever records "where the
/// player died" runs before any authored effect can read it. It emits **nothing**,
/// and that is a settled answer rather than a deferral.
///
/// The seam was carved because spec-0032's recovery stake needs the death position
/// and there were two candidate vanilla mechanisms — a pre-respawn death
/// advancement, or the read-only `LastDeathLocation` player NBT — whose behaviour
/// for non-entity deaths (void, fall, drowning) nobody had measured. CLAUDE.md's
/// debug doctrine answers a question like that by measurement, never by recall.
///
/// **Measured, 5 causes × 3 repeats, every repeat agreeing**
/// (`docs/notes/death-and-teleport-spike.md`): the `deathCount` edge arms **on the
/// corpse**, pre-respawn, for void, fall, drowning, lava and a mob kill alike; and
/// the corpse's own position IS the death position, stable for the whole death
/// screen (measured drift 0.000 in all 15 trials — a corpse stops falling).
///
/// So there is nothing to capture. `on_death_fire` already runs `as @s` on that
/// corpse, and `execute at @s` inside it is positioned at the death point by
/// construction — no scratch storage, no NBT read, no extra command in any
/// campaign's tick. An advancement would have been the wrong instrument
/// (`entity_killed_player` fires for one cause of five; `entity_hurt_player` fires
/// on the FIRST damage event with the player still at 16–20 HP, so it means "was
/// hurt", never "died here"), and `LastDeathLocation` would have been a redundant
/// second reading of a position the executor already stands on.
///
/// It stays a named function rather than becoming a comment for the reason it was
/// one to begin with: the alternative — writing nothing and remembering — is how a
/// seam becomes folklore.
fn death_position_capture() -> Vec<String> {
    Vec::new()
}

/// Generate the death-edge functions: the campaign's `on_death` beat (DSL v0.10,
/// spec-0031) and the checkpoint respawn dispatch (DSL v0.6, spec-0012).
///
/// **One detector, two edges.** Death is detected exactly once, by the vanilla
/// `deathCount` criterion (`dw.deaths`), and `cp_respawn_check` is the one
/// function that reads it. What spec-0031 adds is a second *acknowledgement* of
/// that same counter, not a second detector — because the two consumers want
/// opposite sides of the same event:
///
/// * `on_death` wants **the moment of death**: the player is still a corpse on
///   the death screen, standing where they died. `deathCount` has already ticked
///   up there (measured — it is why the v0.6 half needs its `alive`
///   guard at all), and `@a` matches a corpse, so the corpse side of the edge is
///   reachable with no new machinery.
/// * `on_respawn` and the re-seat want **the player who has come back**, so they
///   hold both their fire and their acknowledgement behind `alive`.
///
/// A single ack cannot serve both: `dw.death_ack` is deliberately withheld while
/// the player is dead, so on the corpse side `deaths > death_ack` stays true for
/// every tick of the death screen. `dw.death_seen` is the corpse-side ack, and it
/// exists only for a campaign that declares `on_death` — a campaign that does not
/// emits this function exactly as it did before the root existed.
///
/// **Not yet emitted: the death POSITION.** See
/// [`death_position_capture`] — the one seam a live measurement fills in.
fn emit_checkpoint_functions(plan: &Plan) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut fns: Vec<(String, String)> = Vec::new();
    let on_death = plan.on_death();
    if !plan.any_checkpoint() && on_death.is_empty() {
        return fns;
    }
    // cp_respawn_check (as @s): fire on the death-count edge, then acknowledge.
    //
    // `deathCount` ticks up the moment the player DIES, while they are still on
    // the death screen — a corpse, not a respawned player. Both the re-seat and
    // the authored `on_respawn` bundle belong to the player who has actually come
    // back, so the whole edge (fire AND acknowledge) is held until the player is
    // alive again: a dead player reads `Health: 0.0f`, and holding the ack keeps
    // the edge armed instead of burning it on the corpse.
    let alive = "unless data entity @s {Health:0.0f}";
    let dead = "if data entity @s {Health:0.0f}";
    let mut check: Vec<String> = Vec::new();
    // **The three scores this edge compares have to EXIST before it compares them.**
    // Found live by the bot tier's death-loop stage, which is the only
    // tier that can witness a player death at all; generalised into `DW0495`, which
    // then named a third objective the instance fix had missed.
    //
    // On the pinned 1.21.11 server a scoreboard entry that was never written is
    // NOT zero: every comparison against it is false, so `execute if score @s A >
    // @s B` does not fire when B has no entry (measured — see `crate::seeding`,
    // which now refuses this shape anywhere in the emitted tree as `DW0495`).
    // `dw.death_ack` and `dw.death_seen` are `dummy` objectives and `dw.deaths` is
    // `deathCount`, and a player who has never died has an entry in none of the
    // three — so the whole edge was dead on a player's FIRST death: no `on_death`
    // (no forfeit, no recovery stake), no `cp_respawn_fire` (no `on_respawn`, no
    // engine re-seat — the party landed wherever vanilla's own `/spawnpoint` hint
    // put them, a hint that cannot be trusted). Both work from the second death
    // onward whatever this does, which is why the gap is invisible to a manual
    // test: every manual test of "does dying work" dies twice.
    //
    // Seeded here rather than at a join hook because this is the one function that
    // reads them, so the two facts cannot drift apart; `add … 0` is idempotent
    // (and, on `deathCount`, does not disturb the criterion — measured: 0 before
    // the first death, 1 after), so running it every tick is a no-op after the
    // first. Emitted only for the edge the campaign declares, so a campaign with
    // neither `on_death` nor a checkpoint moves no byte.
    check.push("scoreboard players add @s dw.deaths 0".to_string());
    if !on_death.is_empty() {
        check.push("scoreboard players add @s dw.death_seen 0".to_string());
    }
    if plan.any_checkpoint() {
        check.push("scoreboard players add @s dw.death_ack 0".to_string());
    }
    // The corpse side FIRST: `on_death` is the earlier moment, and a reader of the
    // generated function should meet the two edges in the order the player lives
    // them. Ordering is otherwise immaterial — the two branches are mutually
    // exclusive by their own guards.
    if !on_death.is_empty() {
        check.extend(death_position_capture());
        check.push(format!(
            "execute {dead} if score @s dw.deaths > @s dw.death_seen run function \
             {ns}:on_death_fire"
        ));
        check.push(format!(
            "execute {dead} run scoreboard players operation @s dw.death_seen = @s dw.deaths"
        ));
    }
    if plan.any_checkpoint() {
        check.push(format!(
            "execute {alive} if score @s dw.deaths > @s dw.death_ack run function \
             {ns}:cp_respawn_fire"
        ));
        check.push(format!(
            "execute {alive} run scoreboard players operation @s dw.death_ack = @s dw.deaths"
        ));
    }
    fns.push(("cp_respawn_check".to_string(), lines(&check)));
    // on_death_fire (as @s): the campaign's death beat, for the player who died.
    //
    // `Audience::Solo`, the audience `on_respawn` and `on_caught` already use: a
    // death is one player's, and re-broadcasting it to the party would duplicate
    // their narration and their kit.
    if !on_death.is_empty() {
        fns.push((
            "on_death_fire".to_string(),
            lines(&emit_effect_bundle(
                plan,
                on_death,
                root_audience(delvewright_dsl::EffectRootKind::OnDeath),
            )),
        ));
    }
    if !plan.any_checkpoint() {
        return fns;
    }
    // cp_seat_<i> (as @s): put the respawned player ON the checkpoint cell.
    //
    // Why this exists. `set-checkpoint` records the
    // party's respawn with vanilla's `spawnpoint @a <cell>`, but `/spawnpoint` is
    // a *hint*: on death vanilla re-validates the recorded cell and, when the cell
    // or the cell above it is solid or liquid, silently discards it and respawns
    // the player at the WORLD spawn — the campaign entrance. Measured live on
    // 1.21.11: a spawnpoint on a dry cell respawns at `cell + (0.5, 0.1, 0.5)`, the
    // same spawnpoint on a water cell respawns at `setworldspawn`. Past a one-way
    // transport that is not a lost checkpoint, it is an unrecoverable softlock.
    //
    // So the delve stops delegating its own promise. `#cp dw.sys` already names
    // the checkpoint the party last armed; the re-seat teleports the respawned
    // player onto that cell's centre unconditionally. When vanilla honoured the
    // spawnpoint the player is already standing there and the teleport is a no-op
    // they cannot see; when vanilla dropped it, this is the only thing that puts
    // them back. Coordinates are compiled in — no macro, no storage read, so the
    // re-seat cannot itself fail on a malformed mirror.
    for c in &plan.checkpoints {
        fns.push((
            format!("cp_seat_{}", c.index),
            lines(&[format!(
                "tp @s {} {} {}",
                center(c.pos[0]),
                c.pos[1],
                center(c.pos[2])
            )]),
        ));
    }
    // cp_respawn_fire (as @s): dispatch on the active checkpoint.
    let reseat = bonfire_reseat_lines(plan);
    // A bonfire owes the respawning party the same scene reset a rest gives them
    // (spec-0016 §1), so it dispatches even with an empty `on_rest` when there
    // are waves to re-seat. A plain `set-checkpoint` keeps the v0.6 rule exactly.
    let dispatches = |c: &crate::plan::CheckpointPlan| {
        !c.on_respawn.is_empty() || (c.rest && !reseat.is_empty())
    };
    let mut fire: Vec<String> = Vec::new();
    // The re-seat runs FIRST and for every checkpoint: an `on_respawn` beat that
    // narrates "you wake at the mark" must be read by a player who is on it.
    for c in &plan.checkpoints {
        fire.push(format!(
            "execute if score #cp dw.sys matches {} run function {ns}:cp_seat_{}",
            c.index, c.index
        ));
    }
    for c in &plan.checkpoints {
        if !dispatches(c) {
            continue;
        }
        fire.push(format!(
            "execute if score #cp dw.sys matches {} run function {ns}:cp_on_respawn_{}",
            c.index, c.index
        ));
    }
    fns.push(("cp_respawn_fire".to_string(), lines(&fire)));
    // cp_on_respawn_<idx> (as @s): the per-player scene-reset effects.
    for c in &plan.checkpoints {
        if !dispatches(c) {
            continue;
        }
        // `Audience::Solo` (spec-0018): the checkpoint itself is party state, but
        // its `on_respawn` belongs to the ONE player who just died — re-broadcasting
        // it would re-narrate and re-gift every survivor on each death.
        //
        // A bonfire's wave re-seat (spec-0016 §1) is party state and is emitted
        // BEFORE the bundle: it names no player, so it fires exactly once for the
        // death, and it must restore the scene before the dying player's own
        // `on_rest` beats read it.
        let mut body: Vec<String> = Vec::new();
        if c.rest {
            body.extend(reseat.iter().cloned());
            // spec-0016 §1, read forward: death respawns
            // the party at the last-rested bonfire with the same hooks, and vanilla
            // already returns the dead player at full health and hunger. What it
            // does NOT restore is the flask — so without this a player who dies
            // arrives empty-handed at the very bonfire they respawned on and must
            // rest again before they can play. Retry has to be cheap, so a respawn
            // at a bonfire refills the flask exactly as a rest does.
            if !plan.flasks().is_empty() {
                body.push(format!("function {ns}:bonfire_flask"));
            }
        }
        body.extend(emit_effect_bundle(
            plan,
            &c.on_respawn,
            root_audience(delvewright_dsl::EffectRootKind::DialogueRespawn),
        ));
        fns.push((format!("cp_on_respawn_{}", c.index), lines(&body)));
    }
    fns
}

// ---------------------------------------------------------------------------
// spec-0016 §4 timed gates
// ---------------------------------------------------------------------------

/// `setup_finish` commands for timed gates (spec-0016 §4): start each gate's
/// clock, and summon the disarm affordance of any gate that
/// declares one. The gate is physically sealed by the prefab at world-load, so
/// the clock's first act is always an OPEN — a `phase` of 0 opens immediately, a
/// larger one holds the gate shut that many ticks first. Empty for a campaign
/// with no timed gate.
///
/// The affordance is the same pair a shortcut unlock and a trap disarm emit: an
/// invisible `minecraft:interaction` hitbox **plus** compiler-owned visible
/// hardware, because a hitbox alone is a lever the player cannot see — the
/// drowned-bell soft-lock class `DW0420` exists to make impossible.
fn timed_gate_setup(plan: &Plan) -> Vec<String> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for g in &plan.timed_gates {
        if g.phase == 0 {
            out.push(format!("function {ns}:tgate_open_{}", g.safe));
        } else {
            out.push(format!(
                "schedule function {ns}:tgate_open_{} {}t",
                g.safe, g.phase
            ));
        }
    }
    for g in &plan.timed_gates {
        let Some(dis) = &g.disarm else {
            continue;
        };
        let v = ent_xyz(dis.via_cell);
        out.push(format!(
            "summon minecraft:interaction {} {} {} {{width:1.0f,height:2.0f,response:1b,Invulnerable:1b,Tags:[{FIXTURE_NBT}\"dw_tgdis_{}\"]}}",
            v[0], v[1], v[2], g.safe
        ));
        out.push(affordance_hardware(
            v,
            &format!("dw_tgdis_{}", g.safe),
            "minecraft:lever",
        ));
    }
    out
}

/// Per-tick disarm detection for jammable timed gates, reusing the
/// v0.4 interaction-entity `use` primitive exactly as a trap disarm and a
/// shortcut unlock do. The `#tgdis_<id>` sentinel makes the jam fire **once**;
/// after it, there is nothing left to dispatch. Empty for a campaign with no
/// disarmable gate → byte-identical.
fn timed_gate_tick(plan: &Plan) -> Vec<String> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for g in &plan.timed_gates {
        if g.disarm.is_none() {
            continue;
        }
        let id = &g.safe;
        out.push(format!(
            "execute unless score #tgdis_{id} dw.sys matches 1 if entity @e[tag=dw_tgdis_{id},nbt={{interaction:{{}}}}] run function {ns}:tgate_disarm_{id}"
        ));
        out.push(format!(
            "execute as @e[tag=dw_tgdis_{id}] run data remove entity @s interaction"
        ));
    }
    out
}

/// Half-hearts dealt by a `crush: true` timed gate's closing edge (spec-0016 §4
/// addendum). Far above any reachable effective health: `minecraft:generic`
/// ignores armor but not absorption/resistance, and the point of a portcullis
/// is that being caught in it is not survivable by gearing.
const CRUSH_DAMAGE: u32 = 1000;

/// A `@a`-selector volume covering an inclusive block region — the same box
/// model `damage-players`' `within` filter uses. `dx/dy/dz` are spans, so a
/// one-block region is `dx=0` and still selects the whole block.
///
/// Corners are normalised because a gate region's stored corners are whatever
/// the prefab metadata declared; a selector with a negative span selects
/// nothing, which would make the crush silently no-op.
fn region_selector(from: [i32; 3], to: [i32; 3]) -> String {
    let lo = [from[0].min(to[0]), from[1].min(to[1]), from[2].min(to[2])];
    let hi = [from[0].max(to[0]), from[1].max(to[1]), from[2].max(to[2])];
    format!(
        "x={},dx={},y={},dy={},z={},dz={},tag=!{CUTSCENE_TAG}",
        lo[0],
        hi[0] - lo[0],
        lo[1],
        hi[1] - lo[1],
        lo[2],
        hi[2] - lo[2]
    )
}

/// The timed-gate clock functions (spec-0016 §4): a two-function ping-pong that
/// carries its own next hop, so the cycle is one self-sustaining chain with no
/// per-tick polling and no state to drift.
///
/// `tgate_open_<id>` clears the region (the same `fill … replace <block>`
/// `open-gate` emits) and schedules the close `open_ticks` later;
/// `tgate_close_<id>` fills it back and schedules the open `closed_ticks` later.
/// `schedule` is replace-mode in vanilla, so the clock can never double up — the
/// same property the boundary and night-vision clocks rely on. Both functions are
/// pure world edits: they name no player, so the server command source they are
/// re-entered under is irrelevant (§4 "A scheduled bundle has no `@s`").
fn emit_timed_gate_functions(plan: &Plan) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for g in &plan.timed_gates {
        let id = &g.safe;
        let (from, to) = g.gate_region;
        // The jam guard. A disarmable gate's clock lines are prefixed
        // with `execute unless score #tgdis_<id> dw.sys matches 1` — the same
        // score-guard shape a gated trap's payload uses. A gate with no `disarm`
        // emits no guard at all, so its output is byte-identical to before.
        let guard = if g.disarm.is_some() {
            Some(format!("execute unless score #tgdis_{id} dw.sys matches 1"))
        } else {
            None
        };
        // `<guard> run <cmd>` when jammable, else `<cmd>` verbatim.
        let guarded = |cmd: String| match &guard {
            Some(g) => format!("{g} run {cmd}"),
            None => cmd,
        };
        // …and the `execute` form, for a line that is already an `execute`: its
        // subcommands splice straight onto the guard rather than nesting.
        let guarded_exec = |rest: &str, cmd: &str| match &guard {
            Some(g) => format!("{g} {rest} run {cmd}"),
            None => format!("execute {rest} run {cmd}"),
        };
        out.push((
            format!("tgate_open_{id}"),
            lines(&[
                // The open itself is NEVER guarded: a jam that lands while the
                // gate is shut leaves one already-scheduled open in flight, and
                // that open is exactly what parks the portcullis in its resting
                // position. Suppressing it would freeze the gate CLOSED, which
                // is the opposite of a disarm.
                format!(
                    "fill {} {} {} {} {} {} minecraft:air replace {}",
                    from[0], from[1], from[2], to[0], to[1], to[2], g.gate_block
                ),
                guarded(format!(
                    "schedule function {ns}:tgate_close_{id} {}t",
                    g.open_ticks
                )),
            ]),
        ));
        let mut close = Vec::new();
        // spec-0016 §4 addendum: the portcullis judgement. Emitted BEFORE the
        // fill so the victim is judged on the world as it was when they
        // mistimed it — after the fill they are already inside a solid block
        // and vanilla's own suffocation would be the thing killing them, which
        // is slow, gear-dependent and escapable. Costs nothing per tick: this
        // rides the closing tick of the schedule ping-pong that already exists.
        //
        // The judgement sits INSIDE the suppressed clock, so a
        // disarmed gate can never crush — there is no closing tick left to be
        // caught by. That is not a second rule, it is the same guard.
        if g.crush {
            close.push(guarded_exec(
                &format!("as @a[{}]", region_selector(from, to)),
                &format!("damage @s {CRUSH_DAMAGE} minecraft:generic"),
            ));
        }
        close.push(guarded(format!(
            "fill {} {} {} {} {} {} {}",
            from[0], from[1], from[2], to[0], to[1], to[2], g.gate_block
        )));
        close.push(guarded(format!(
            "schedule function {ns}:tgate_open_{id} {}t",
            g.closed_ticks
        )));
        out.push((format!("tgate_close_{id}"), lines(&close)));
    }
    out.extend(emit_timed_gate_disarm_functions(plan));
    out
}

/// The `tgate_disarm_<id>` functions: jam the gate for good.
///
/// Four commands, and the ORDER is the semantics:
/// 1. latch `#tgdis_<id>` — from this instant every guarded clock line is inert,
///    including the crush;
/// 2. raise the disarm flag party-wide, so the rest of the campaign can read
///    "the party switched it off" (`requires_flags`, a dialogue gate, a quest);
/// 3. clear the span **once** — the jammed portcullis comes to rest OPEN, which
///    is what a disarm means and what a player who pulls a lever expects to see;
/// 4. retire the affordance's visible hardware. This is the ONE function allowed
///    to do that — `DW0421` fails the build if anything else reaches it.
///
/// There is deliberately **no** `schedule clear`. A close already in flight fires
/// into the guard and does nothing — including not scheduling the next open — so
/// the ping-pong dies of its own accord within one hop, and the gate is left open
/// by step 3. Clearing a schedule that may not exist would be a command that
/// fails at runtime for no gain.
fn emit_timed_gate_disarm_functions(plan: &Plan) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for g in &plan.timed_gates {
        let Some(dis) = &g.disarm else {
            continue;
        };
        let id = &g.safe;
        let (from, to) = g.gate_region;
        let body = vec![
            format!("scoreboard players set #tgdis_{id} dw.sys 1"),
            format!(
                "scoreboard players set {} {} 1",
                plan::PARTY,
                plan::flag_score(&dis.sets_flag)
            ),
            format!(
                "fill {} {} {} {} {} {} minecraft:air replace {}",
                from[0], from[1], from[2], to[0], to[1], to[2], g.gate_block
            ),
            format!(
                "kill @e[tag={}]",
                crate::affordance::hardware_tag(&format!("dw_tgdis_{id}"))
            ),
        ];
        out.push((format!("tgate_disarm_{id}"), lines(&body)));
    }
    out
}

// ---------------------------------------------------------------------------
// spec-0016 §2 shortcut doors
// ---------------------------------------------------------------------------

/// Every compiler-owned interact affordance in this campaign, in deterministic
/// order — the subjects of the `DW0420` / `DW0421` proofs.
///
/// The list is the definition of the class: a point the delve asks the player to
/// right-click. Adding a new such verb means adding it here, which is what makes
/// the proof total rather than a spot check.
fn affordances(plan: &Plan) -> Vec<crate::affordance::Affordance> {
    let mut out = Vec::new();
    for sc in &plan.shortcuts {
        out.push(crate::affordance::Affordance {
            id: sc.id.clone(),
            kind: "shortcut unlock",
            tag: format!("dw_sc_{}", sc.safe),
            // Opening the door spends the affordance.
            retired_by: Some(format!("shortcut_open_{}", sc.safe)),
        });
    }
    for t in &plan.traps {
        if t.disarm.is_some() {
            out.push(crate::affordance::Affordance {
                id: t.id.clone(),
                kind: "trap disarm",
                tag: format!("dw_trapdis_{}", t.safe),
                // Throwing the lever spends the affordance.
                retired_by: Some(format!("trap_disarm_{}", t.safe)),
            });
        }
    }
    for g in &plan.timed_gates {
        if g.disarm.is_some() {
            out.push(crate::affordance::Affordance {
                id: g.id.clone(),
                kind: "timed-gate disarm",
                tag: format!("dw_tgdis_{}", g.safe),
                // Jamming the gate spends the affordance.
                retired_by: Some(format!("tgate_disarm_{}", g.safe)),
            });
        }
    }
    for bf in plan.bonfires() {
        out.push(crate::affordance::Affordance {
            id: bf.anchor.clone(),
            kind: "bonfire",
            tag: format!("dw_bonfire_{}", bf.index),
            // A bonfire is rested at, never used up.
            retired_by: None,
        });
    }
    // spec-0032. A shop is furniture — traded with, never used up. A stake IS
    // consumed, and `stk_gc_<s>` is the single function permitted to retire it:
    // every other path (a collection, an eviction under the `replace` policy)
    // clears the per-player ledger and lets the reference count decide, which is
    // what keeps one killer for one piece of hardware (`DW0421`).
    for (i, sh, _) in shops(plan) {
        out.push(crate::affordance::Affordance {
            id: sh.id.as_str().to_string(),
            kind: "shop",
            tag: format!("dw_shop_{i}"),
            retired_by: None,
        });
    }
    for (st, safe) in stakes(plan) {
        if st.max_live() == 0 {
            continue;
        }
        out.push(crate::affordance::Affordance {
            id: st.id.as_str().to_string(),
            kind: "recovery stake",
            tag: stk_tag(&safe),
            retired_by: Some(format!("stk_gc_{safe}")),
        });
    }
    out
}

/// `setup_finish` commands for shortcut doors (spec-0016 §2): summon the
/// far-side unlock affordance (a right-click target, the same
/// `minecraft:interaction` primitive as a trap disarm) **and its visible
/// hardware**. The gate needs no command at all — it is **physically sealed in
/// the prefab** from world-load, which is precisely why the pattern needs no
/// "seal it now" verb and why permanence can be structural. Empty for a
/// campaign with no shortcut.
///
/// The hardware is not decoration. `minecraft:interaction` is an invisible
/// hitbox, so the hitbox alone asks the player to right-click a point nothing
/// marks — the drowned-bell soft-lock, where the unlock cell was bare air and
/// the only visible thing there belonged to an unrelated objective. The
/// compiler owns the affordance's visibility; it is never left to whether the
/// tileset happens to carry a lever. Proven by `DW0420`
/// ([`crate::affordance`]).
fn shortcut_setup(plan: &Plan) -> Vec<String> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for sc in &plan.shortcuts {
        let v = ent_xyz(sc.unlock);
        out.push(format!(
            "summon minecraft:interaction {} {} {} {{width:1.0f,height:2.0f,response:1b,Invulnerable:1b,Tags:[{FIXTURE_NBT}\"dw_sc_{}\"]}}",
            v[0], v[1], v[2], sc.safe
        ));
        out.push(affordance_hardware(
            v,
            &format!("dw_sc_{}", sc.safe),
            "minecraft:lever",
        ));
        // Arm the door itself, so a press from the sealed side reaches
        // something. Unlike a `close-gate` seal — which is armed by the firing
        // that seals it — a shortcut gate is sealed by the PREFAB at world-load,
        // so world init is the only moment its answer can go up. Guarded on
        // absence for the same reason the close-gate arming is: a second,
        // co-located set of hitboxes is the exact ray-pick tie `DW0422` forbids.
        out.push(format!(
            "execute unless entity @e[tag=dw_ws_{}] run function {ns}:ws_arm_{}",
            sc.safe, sc.safe
        ));
    }
    out
}

/// The visible hardware for a compiler-owned interact affordance: a glowing,
/// collision-free `item_display` at the affordance's own cell, carrying the
/// derived `dw_hw_<tag>` so [`crate::affordance`] can pair the two.
///
/// An `item_display` (not a block) because the affordance's cell must stay
/// walkable and the interaction hitbox unobstructed — the same reasoning that
/// made the interact objective's marker a display. It is deliberately
/// **nameless**: the glow says "use me" without inventing a player-facing
/// string that no campaign authored and no `l10n` sidecar could translate.
fn affordance_hardware(pos: [String; 3], tag: &str, item: &str) -> String {
    format!(
        "summon minecraft:item_display {} {} {} {{Glowing:1b,Tags:[{FIXTURE_NBT}\"dw_marker\",\"{}\"],billboard:\"center\",item:{{id:\"{item}\",count:1}}}}",
        pos[0],
        pos[1],
        pos[2],
        crate::affordance::hardware_tag(tag)
    )
}

// ---------------------------------------------------------------------------
// The seal answers (DSL v0.8)
// ---------------------------------------------------------------------------

/// How far a seal's answer hitbox protrudes past the sealed block, on every side.
///
/// **This margin is the whole mechanism.** A `minecraft:interaction` whose box
/// exactly coincides with the block it stands in loses the client's ray-pick:
/// vanilla takes the entity only when it is *strictly* nearer the eye than the
/// block hit, and a coincident box is hit at exactly the same distance. One
/// centimetre of protrusion makes the entity strictly nearer from every approach
/// angle, so pressing any face of the seal reaches the entity — while a hundredth
/// of a block never reaches into a neighbouring cell's own affordances.
pub const SEAL_MARGIN: f64 = 0.01;

/// The seal-answer entity's box size, as the `width`/`height` NBT floats: one
/// block plus [`SEAL_MARGIN`] on each side.
const SEAL_BOX_SIZE: &str = "1.02f";

/// Render a signed count of hundredths as a decimal coordinate: `6899` →
/// `68.99`, `-4450` → `-44.5`, `700` → `7.0`. Integer-only, so the emitted text
/// is exactly what it reads as (no binary-float rounding in the datapack).
fn fmt_centi(v: i64) -> String {
    let sign = if v < 0 { "-" } else { "" };
    let a = v.unsigned_abs();
    let (whole, frac) = (a / 100, a % 100);
    if frac == 0 {
        format!("{sign}{whole}.0")
    } else if frac.is_multiple_of(10) {
        format!("{sign}{whole}.{}", frac / 10)
    } else {
        format!("{sign}{whole}.{frac:02}")
    }
}

/// The seal plan for a gate anchor, if the campaign ever seals it.
fn seal_hint_for<'a>(plan: &'a Plan, anchor: &str) -> Option<&'a plan::SealHintPlan> {
    plan.seal_hints.iter().find(|s| s.anchor == anchor)
}

/// The `seal_arm_<safe>` function name: what a `close-gate` calls to give the
/// stone a voice.
fn seal_arm_fn(safe: &str) -> String {
    format!("seal_arm_{safe}")
}

/// The `dw_trig_<id>` tags every click trigger anchored **on this gate** rides,
/// in campaign declaration order (deterministic).
///
/// The round-6 rule, one layer out: one cell, one hitbox. A `strike`/`use`
/// trigger whose `at` is the gate anchor is asking the player to hit *the gate* —
/// and once the gate is sealed the gate's own hitboxes are what a click reaches.
/// Summoning the trigger a second, co-located entity is the exact ray-pick tie
/// that made the island's boulder unshippable, so the trigger's tag rides these
/// entities and [`env_trigger_setup`] summons nothing for it. The consequence is
/// also its meaning: such a trigger is live exactly while the gate is sealed.
fn seal_rider_tags(plan: &Plan, chrome: &delvewright_dsl::Chrome, anchor: &str) -> Vec<String> {
    use delvewright_dsl::TriggerOn;
    plan.emitted_triggers(chrome)
        .iter()
        .filter(|t| !matches!(t.on, TriggerOn::Approach { .. }))
        .filter(|t| t.at_anchor() == Some(anchor))
        .map(|t| format!("dw_trig_{}", plan::safe_local(t.id.as_str())))
        .collect()
}

/// The `seal_arm_<safe>` functions: one `minecraft:interaction` per
/// clickable cell of each sealed region, so the wall answers a press wherever the
/// party presses it.
///
/// Only the region's **shell** is armed ([`plan::SealHintPlan::shell_cells`]) —
/// a cell buried inside the seal has no face a crosshair can reach. Each entity
/// is one block plus [`SEAL_MARGIN`], positioned so its box brackets its cell on
/// every axis; see that constant for why the margin is not cosmetic.
///
/// Empty for a campaign that never seals a gate → byte-identical output.
fn seal_fns(plan: &Plan, chrome: &delvewright_dsl::Chrome) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for s in &plan.seal_hints {
        let mut tags = vec![format!("dw_seal_{}", s.safe)];
        tags.extend(seal_rider_tags(plan, chrome, &s.anchor));
        let tag_list = tags
            .iter()
            .map(|t| format!("\"{t}\""))
            .collect::<Vec<_>>()
            .join(",");
        let body: Vec<String> = s
            .shell_cells()
            .into_iter()
            .map(|c| {
                // Positions are built from integer hundredths, never from f64
                // arithmetic: the datapack text is part of the byte-identity
                // contract (ADR-0006) and `y - 0.01` in binary floating point is
                // not the decimal `.99` a reader (or a diff) expects.
                //
                // x/z are the cell CENTRE (the box is width-symmetric about the
                // position); y is the box's FLOOR, dropped one margin so the box
                // brackets the cell below as well as above.
                let x = fmt_centi(c[0] as i64 * 100 + 50);
                let y = fmt_centi(c[1] as i64 * 100 - 1);
                let z = fmt_centi(c[2] as i64 * 100 + 50);
                format!(
                    "summon minecraft:interaction {x} {y} {z} \
                     {{width:{SEAL_BOX_SIZE},height:{SEAL_BOX_SIZE},response:1b,Invulnerable:1b,Tags:[{FIXTURE_NBT}{tag_list}]}}"
                )
            })
            .collect();
        out.push((seal_arm_fn(&s.safe), lines(&body)));
    }
    out
}

// The `seal_hint_<safe>` reward functions and their `seal_<safe>` advancements
// are GONE (DSL v0.11). They were `close-gate`'s private copy of "a pressable
// thing answers the player who pressed it": its own advancement shape, its own
// actionbar command, its own baked English — none of which has anything to do
// with closing a gate, and none of which the second object that needed them (a
// sealed shortcut door) could reach.
//
// A seal's answer is now an ordinary `EnvTrigger{on: use, audience: presser}`
// carrying an ordinary `narrate{style: actionbar}`, synthesized by
// `plan::collect_press_answers` and emitted by `env_trigger_fns` /
// `press_dispatch_fn` / `emit_advancements` like any author's own click. The
// wording is unchanged, the revoke-every-press behaviour is unchanged, and the
// shortcut door gets all of it for free — which is the whole finding.

/// Generated `v08_seal_answers` PackTest: on a live pinned server,
/// a gate that is sealed carries the hitboxes its answer rides, arming is
/// idempotent, and re-opening it takes them away again.
///
/// What this proves and what it deliberately does not: the **presence** contract
/// is fully machine-checkable here, and it is the half that failed — the island's
/// sealed boulder had no hitbox at all, so a press reached nothing. The
/// press-to-actionbar half rides `player_interacted_with_entity`, which no
/// PackTest can fire (it needs a real client's right-click); that primitive is
/// the one every NPC dialogue and bonfire rest already runs on, and the harness
/// bot exercises it there.
///
/// Batch model: the fixture stages the seal itself and hands the world
/// back exactly as it found it — region cleared, hitboxes killed.
fn emit_seal_packtest(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let Some(s) = plan.seal_hints.first() else {
        return;
    };
    let (from, to) = s.region;
    let n = s.shell_cells().len();
    let tag = format!("dw_seal_{}", s.safe);
    let count = |score: &str| {
        format!(
            "execute store result score #{score} dw.sys if entity @e[type=minecraft:interaction,tag={tag}]"
        )
    };
    let mut b = packtest_header(&format!(
        "{}: the sealed gate `{}` carries an answer the party can press",
        artifact_title(plan.campaign),
        s.anchor
    ));
    b.push(format!("function {ns}:setup"));
    b.push("# Batch model: a sibling test may have driven the campaign past its".to_string());
    b.push("# own seal, so stage a known-OPEN gate rather than assuming one.".to_string());
    b.push(format!("kill @e[tag={tag}]"));
    b.push(format!(
        "fill {} {} {} {} {} {} minecraft:air replace {}",
        from[0], from[1], from[2], to[0], to[1], to[2], s.block
    ));
    b.push("# An open gate is nothing to press: nothing armed.".to_string());
    b.push(count("seal_before"));
    b.push("assert score #seal_before dw.sys matches 0".to_string());
    b.push(format!(
        "fill {} {} {} {} {} {} {}",
        from[0], from[1], from[2], to[0], to[1], to[2], s.block
    ));
    b.push(format!(
        "execute unless entity @e[tag={tag}] run function {ns}:{}",
        seal_arm_fn(&s.safe)
    ));
    b.push(count("seal_armed"));
    b.push(format!("assert score #seal_armed dw.sys matches {n}"));
    b.push("# A re-fired seal must not stack a second, co-located set.".to_string());
    b.push(format!(
        "execute unless entity @e[tag={tag}] run function {ns}:{}",
        seal_arm_fn(&s.safe)
    ));
    b.push(count("seal_again"));
    b.push(format!("assert score #seal_again dw.sys matches {n}"));
    b.push("# Re-opening takes the answer down with the stone (no residue).".to_string());
    b.push(format!(
        "fill {} {} {} {} {} {} minecraft:air replace {}",
        from[0], from[1], from[2], to[0], to[1], to[2], s.block
    ));
    b.push(format!("kill @e[tag={tag}]"));
    b.push(count("seal_after"));
    b.push("assert score #seal_after dw.sys matches 0".to_string());
    out.insert(
        format!("packtest-datapack/data/{ns}/test/v08_seal_answers.mcfunction"),
        lines(&b).into_bytes(),
    );
}

// ---------------------------------------------------------------------------
// The shortcut door's wrong-side answer (DSL v0.9)
// ---------------------------------------------------------------------------

/// Every shortcut door with its derived sealed side.
///
/// **Every** shortcut gets a body — a door with no answer is still a door a
/// player walks up to and pushes — so the only shortcut missing here is one whose
/// side did not resolve, and such a campaign never reaches emission:
/// [`check_shortcut_sides`] fails the build first. That is why `DW0425` binds to
/// every shortcut rather than to the ones that authored something.
fn answering_shortcuts<'a>(
    plan: &'a Plan,
) -> Vec<(&'a plan::ShortcutPlan, &'a crate::wrongside::SealedSide)> {
    plan.shortcuts
        .iter()
        .filter_map(|sc| sc.sealed_side.as_ref().map(|s| (sc, s)))
        .collect()
}

/// `DW0425`: a shortcut door whose sealed side the geometry does not name.
///
/// Build tier (exit 3), raised before any function is emitted — withhold, never
/// invent. Placing the answer on a guessed side would tell a player standing
/// exactly where the door DOES open that it cannot be opened from there, which is
/// a worse failure than the silence this feature exists to end.
fn check_shortcut_sides(plan: &Plan) -> Result<(), BuildFailure> {
    for sc in &plan.shortcuts {
        if sc.sealed_side.is_some() {
            continue;
        }
        let (lo, hi) = sc.gate_region;
        return Err(BuildFailure::Diagnostic {
            code: crate::wrongside::DW_SHORTCUT_SIDE_UNDECIDABLE,
            message: format!(
                "shortcut `{}` needs a clickable body on the sealed side of its gate `{}`, but \
                 the compiler cannot tell which side that is. The sealed side is derived from \
                 the gate slab's thin axis and the side of it the `unlock` anchor `{}` stands on; \
                 here the gate spans {lo:?}..{hi:?} and the unlock resolves to {:?}, which either \
                 gives the region no unique thinnest axis (a cube is not a doorway) or leaves the \
                 unlock level with the doorway rather than beyond it. An answer placed on a \
                 guessed side would fire where the door DOES open. Prescription: put the `unlock` \
                 clear of the gate's own span on the axis the door is thin on — which is where a \
                 far-side bar belongs anyway — or use a gate anchor whose region is a doorway \
                 slab rather than a volume.",
                sc.id, sc.gate_anchor, sc.unlock_anchor, sc.unlock,
            ),
        });
    }
    Ok(())
}

/// The `ws_arm_<safe>` functions: the **clickable body of a sealed
/// shortcut door**.
///
/// A shortcut gate is a solid slab the prefab places, and nothing gave it a body.
/// A `use`/`strike` trigger anchored on it summoned the ordinary point body — one
/// `1.0f x 2.0f` box at the region's first cell — which for the `souls-shortcut`
/// fixture lands at AABB `[4,65,6]..[5,67,7]` inside a slab occupying
/// `[4,65,6]..[6,68,7]`: flush with the block on every face it touches and
/// interior on the rest, so vanilla never finds it strictly nearer than the block
/// and no press from any angle reaches it (see [`SEAL_MARGIN`]).
///
/// The body therefore stands in the **open air in front of the bars**, one cell
/// per doorway cell, on the sealed side only
/// ([`crate::wrongside::SealedSide::approach_cells`]). That placement is also the
/// entire side mechanism: a near-side ray hits the body before the door, a
/// far-side ray hits the door and stops, because vanilla bounds its entity
/// raycast by the block hit distance. No player test, no DSL surface.
///
/// A click trigger the author anchors on the gate rides these — the same merge
/// `seal_fns` performs for a `close-gate` seal — so the author's own prose and
/// sound, gated by their own flags, are what a wrong-side press produces. The
/// compiler supplies the body; the campaign supplies the answer.
///
/// Empty for a campaign with no shortcut → byte-identical output.
fn ws_arm_fns(plan: &Plan, chrome: &delvewright_dsl::Chrome) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (sc, side) in answering_shortcuts(plan) {
        // A click trigger the author anchored on this gate rides these hitboxes
        // rather than summoning its own co-located one — the same merge
        // `seal_fns` performs for a `close-gate` seal, and the reason a trigger
        // at a gate anchor stops being a ray-pick tie.
        let mut tags = vec![format!("dw_ws_{}", sc.safe)];
        tags.extend(seal_rider_tags(plan, chrome, &sc.gate_anchor));
        let tag_list = tags
            .iter()
            .map(|t| format!("\"{t}\""))
            .collect::<Vec<_>>()
            .join(",");
        let body: Vec<String> = side
            .approach_cells()
            .into_iter()
            .map(|c| {
                // Integer hundredths, never f64 arithmetic: the datapack text is
                // part of the byte-identity contract (ADR-0006).
                let x = fmt_centi(c[0] as i64 * 100 + 50);
                let y = fmt_centi(c[1] as i64 * 100 - 1);
                let z = fmt_centi(c[2] as i64 * 100 + 50);
                format!(
                    "summon minecraft:interaction {x} {y} {z} \
                     {{width:{SEAL_BOX_SIZE},height:{SEAL_BOX_SIZE},response:1b,Invulnerable:1b,Tags:[{FIXTURE_NBT}{tag_list}]}}"
                )
            })
            .collect();
        out.push((format!("ws_arm_{}", sc.safe), lines(&body)));
    }
    out
}

/// Per-tick shortcut unlock detection (spec-0016 §2). Fires **once** — the
/// `#sc_<id>` sentinel is the structural expression of permanence: after the open
/// there is nothing left to fire, and no verb anywhere can put the gate back
/// (`DW0372` forbids `close-gate` on a shortcut gate). Empty without a shortcut.
fn shortcut_tick(plan: &Plan) -> Vec<String> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for sc in &plan.shortcuts {
        let id = &sc.safe;
        out.push(format!(
            "execute unless score #sc_{id} dw.sys matches 1 if entity @e[tag=dw_sc_{id},nbt={{interaction:{{}}}}] run function {ns}:shortcut_open_{id}"
        ));
        out.push(format!(
            "execute as @e[tag=dw_sc_{id}] run data remove entity @s interaction"
        ));
    }
    out
}

/// The `shortcut_open_<id>` functions (spec-0016 §2): latch the sentinel, clear
/// the gate region to air (the same `fill … replace <block>` an `open-gate`
/// emits), then run the `on_unlock` beat. Server-source-safe — the poll lives on
/// the tick, which has no `@s`.
fn emit_shortcut_functions(plan: &Plan) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for sc in &plan.shortcuts {
        let id = &sc.safe;
        let (from, to) = sc.gate_region;
        let mut body = vec![
            format!("scoreboard players set #sc_{id} dw.sys 1"),
            format!(
                "fill {} {} {} {} {} {} minecraft:air replace {}",
                from[0], from[1], from[2], to[0], to[1], to[2], sc.gate_block
            ),
            // The affordance is spent: the bar is thrown and the door is open,
            // so its hardware retires with it. This is the ONE function allowed
            // to remove it — `DW0421` fails the build if anything else does.
            format!(
                "kill @e[tag={}]",
                crate::affordance::hardware_tag(&format!("dw_sc_{id}"))
            ),
        ];
        // …and the door's own voice goes with the bars. An opened
        // threshold that still says "this will not open" is a lie, and an
        // invisible box left standing in a now-walkable doorway swallows
        // right-clicks aimed through it — the same retirement `open-gate`
        // performs for a `close-gate` seal.
        body.push(format!("kill @e[tag=dw_ws_{id}]"));
        body.extend(emit_effect_bundle(
            plan,
            &sc.on_unlock,
            root_audience(delvewright_dsl::EffectRootKind::ShortcutUnlock),
        ));
        out.push((format!("shortcut_open_{id}"), lines(&body)));
    }
    out
}

/// The fake-player scoreboard holder marking a `respawns_on_rest` wave as
/// **seated** — set by the wave's own `spawn_<wave>` (spec-0016 §1). A bonfire
/// only re-seats waves the party has actually met; without this a rest would
/// spawn every future wave in the delve at once.
fn wave_seated_holder(wave_id: &str) -> String {
    format!("#wseat_{}", plan::safe_local(wave_id))
}

/// The re-seat lines a bonfire runs on every rest and on every respawn at it
/// (spec-0016 §1), in a fixed order: the `respawns_on_rest` waves, then the
/// **undefeated** refresh — billed elite/boss waves, then hostile actors. Empty
/// for a campaign that declares none of that surface → byte-identical.
///
/// Two different questions are being asked here, and they take two different
/// primitives.
///
/// * *Has the party MET this wave?* — a scoreboard sentinel
///   ([`wave_seated_holder`]), written by the wave's own `spawn_<wave>`. A
///   `respawns_on_rest` wave comes back whether the party beat it or fled it, so
///   "met" is the only gate, and a wave the delve has not staged yet must not be
///   conjured by a rest.
/// * *Is this thing still STANDING?* — the presence of its own body
///   (`execute if entity`). That is the undefeated test, and it needs no state at
///   all: a boss the party killed leaves no body, so it stays dead by
///   construction (spec-0016 §1), and one they merely chipped is still there, so
///   it is wiped and re-seated whole. `despawn-actor` leaves none either, so a
///   scripted vanish is equally final.
///
/// An actor's line asks the body question twice, because an actor has two
/// postures and only one of them can have been damaged or dragged. A caged
/// puppet (`dw_pup_<id>`) is `NoAI` and knockback-immune — combat cannot move it,
/// and re-seating it would only undo authored `move-actor` staging — so it is
/// left exactly where the campaign put it. An **unleashed twin** is the elite the
/// party is actually fighting: it wears `dw_actor_<id>` and no puppet marker, and
/// the rest deletes it and stands a fresh one on its origin anchor.
fn bonfire_reseat_lines(plan: &Plan) -> Vec<String> {
    let ns = &plan.namespace;
    let mut out: Vec<String> = plan
        .reseat_waves()
        .iter()
        .map(|w| {
            format!(
                "execute if score {} dw.sys matches 1 run function {ns}:wave_reseat_{}",
                wave_seated_holder(w.id.as_str()),
                plan::safe_local(w.id.as_str())
            )
        })
        .collect();
    for w in plan.undefeated_reseat_waves() {
        out.push(format!(
            "execute if entity @e[tag={}] run function {ns}:wave_reseat_{}",
            plan::wave_tag(w.id.as_str()),
            plan::safe_local(w.id.as_str())
        ));
    }
    for a in plan.reseat_actors() {
        let safe = plan::safe_local(a.id.as_str());
        out.push(format!(
            "execute unless entity @e[tag=dw_pup_{safe}] if entity @e[tag=dw_actor_{safe}] \
             run function {ns}:actor_restand_{safe}"
        ));
    }
    out
}

/// Per-tick bonfire **choice** dispatch (spec-0016 §1).
///
/// Right-clicking a bonfire no longer rests: it opens a two-option dialog
/// (`bonfire_open_<i>`, run as the clicking player by the vanilla
/// `player_interacted_with_entity` advancement — the same primitive every
/// interact objective uses). The buttons write the player's answer into the
/// `dw.rest` **trigger** objective, which is the only command surface a
/// non-operator player has, and this tick turns that answer into the chosen
/// function. `dw.rest_at` carries WHICH bonfire the player opened, so a campaign
/// with several bonfires routes each answer to its own rest point.
///
/// `1` = *save only* (move the checkpoint, nothing else), `2` = *rest and save*
/// (the full loop). Empty for a campaign with no bonfire → byte-identical.
fn bonfire_tick(plan: &Plan) -> Vec<String> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for bf in plan.bonfires() {
        let i = bf.index;
        out.push(format!(
            "execute as @a[scores={{dw.rest=1,dw.rest_at={i}}}] run function {ns}:bonfire_pick_save_{i}"
        ));
        out.push(format!(
            "execute as @a[scores={{dw.rest=2,dw.rest_at={i}}}] run function {ns}:bonfire_pick_rest_{i}"
        ));
    }
    out
}

/// The player-local restore a **rest** performs (spec-0016 §1): health,
/// hunger/saturation, negative status effects, flask.
///
/// **Audience (reported ambiguity).** spec-0018 makes the checkpoint party state
/// and the flask/inventory per-player state; spec-0016 §1 says "player fully
/// restored" in the singular. This restores the player who chose to rest, and
/// only them — a party member elsewhere in the map keeps their wounds. The
/// checkpoint half of the same rest is still party-wide.
///
/// **Effect clearing is enumerated, never `effect clear @s`.** A bare clear would
/// also strip the per-area night-vision mitigation clock (`DW0322`'s emission)
/// and any beneficial effect the story granted, turning a rest into a debuff.
/// The list is the pinned 1.21.11 harmful set, sorted, so the emission is
/// deterministic.
const HARMFUL_EFFECTS: &[&str] = &[
    "minecraft:bad_omen",
    "minecraft:blindness",
    "minecraft:darkness",
    "minecraft:hunger",
    "minecraft:infested",
    "minecraft:levitation",
    "minecraft:mining_fatigue",
    "minecraft:nausea",
    "minecraft:oozing",
    "minecraft:poison",
    "minecraft:raid_omen",
    "minecraft:slowness",
    "minecraft:trial_omen",
    "minecraft:unluck",
    "minecraft:weakness",
    "minecraft:weaving",
    "minecraft:wind_charged",
    "minecraft:wither",
];

/// The `minecraft:potion_contents` component value of a kit item that declares
/// potion `contents` (DSL v0.8, spec-0016 §1) — compact SNBT, field order fixed
/// (`potion`, `custom_effects`, `custom_color`) so emission is deterministic.
///
/// Written straight from the DSL's fields with nothing invented: a declared
/// `duration`/`amplifier` is emitted, an absent one is left out and takes
/// vanilla's own default. That matters beyond tidiness — the replenish path
/// matches the flask by these exact components ([`kit_item_predicate`]), so any
/// value the emitter made up here would have to be re-derived identically there.
fn potion_contents_snbt(pc: &delvewright_dsl::PotionContents) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(p) = &pc.potion {
        parts.push(format!("potion:\"{p}\""));
    }
    if !pc.effects.is_empty() {
        let effects: Vec<String> = pc
            .effects
            .iter()
            .map(|e| {
                let mut f = vec![format!("id:\"{}\"", e.effect)];
                if let Some(dur) = e.duration {
                    f.push(format!("duration:{dur}"));
                }
                if let Some(amp) = e.amplifier {
                    f.push(format!("amplifier:{amp}"));
                }
                format!("{{{}}}", f.join(","))
            })
            .collect();
        parts.push(format!("custom_effects:[{}]", effects.join(",")));
    }
    if let Some(col) = &pc.color {
        // `#rrggbb` → the packed int vanilla stores. Validation (`DW0486`)
        // already proved the literal well-formed.
        if let Ok(v) = u32::from_str_radix(col.trim_start_matches('#'), 16) {
            parts.push(format!("custom_color:{v}"));
        }
    }
    format!("{{{}}}", parts.join(","))
}

/// The component suffix a kit item's `give` carries: the display name and, for a
/// potion-bearing item, its `potion_contents`. `""` for a plain unnamed item, so
/// every campaign that declares neither is byte-identical.
///
/// One function for every place a kit item is handed out — the class kit and the
/// bonfire replenish — because those two must produce the *same item*. When they
/// disagree the rest does not refill the flask, it hands the player a second,
/// subtly different one (the `clear` misses it) and the "per-rest budget"
/// contract silently becomes a stockpile.
fn kit_item_components(item: &delvewright_dsl::KitItem) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(n) = &item.name {
        parts.push(format!(
            "custom_name={}",
            tr_with(n, &[("italic", json!(false))])
        ));
    }
    if let Some(pc) = &item.contents {
        parts.push(format!("potion_contents={}", potion_contents_snbt(pc)));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("[{}]", parts.join(","))
    }
}

/// The **item predicate** that identifies this kit item for `clear` — the item id
/// plus, when it carries potion contents, an exact `potion_contents` match.
///
/// Why the components belong in the predicate: a bare `clear @s minecraft:potion`
/// takes every potion in the bag, so on a campaign whose kit holds a healing
/// flask *and* any other brew, one rest would delete the other bottle and re-give
/// only the flask. Matching the contents makes the clear name exactly the stack
/// the `give` on the next line puts back.
fn kit_item_predicate(item: &delvewright_dsl::KitItem) -> String {
    match &item.contents {
        Some(pc) => format!(
            "{}[potion_contents={}]",
            item.item,
            potion_contents_snbt(pc)
        ),
        None => item.item.clone(),
    }
}

/// The `bonfire_flask` function: refill every declared flask to its declared
/// count, for the player it runs as.
///
/// `clear` + `give` rather than `item replace`: a kit item has no fixed inventory
/// slot (the player carries it wherever they moved it), and `item replace` needs
/// one. Clearing the flask's own item predicate and re-giving the kit's exact
/// stack is slot-free, idempotent and byte-stable — and it means "replenish to
/// the declared count" is literally what the commands say, in both directions (a
/// player hoarding extra flasks is brought back DOWN to the declared count, which
/// is the souls contract: the flask is a per-rest budget, not a stockpile).
/// Cleared and re-given through the SAME pair of helpers the class kit uses, so
/// the refilled bottle is the poured-identical item, not a lookalike.
///
/// A player's class is read off the `dw_class_<safe>` tag `class_apply_<safe>`
/// adds — emitted only when the campaign declares a flask at all, so a campaign
/// without one is byte-identical down to the class-apply function.
fn emit_flask_function(plan: &Plan) -> Option<(String, String)> {
    let flasks = plan.flasks();
    if flasks.is_empty() {
        return None;
    }
    let classes = &plan.campaign.classes.content.classes;
    let mut body: Vec<String> = Vec::new();
    for (ci, ki) in flasks {
        let item = &classes[ci].kit[ki];
        let tag = class_tag(&plan.classes[ci].safe);
        body.push(format!(
            "execute if entity @s[tag={tag}] run clear @s {}",
            kit_item_predicate(item)
        ));
        body.push(format!(
            "execute if entity @s[tag={tag}] run give @s {}{} {}",
            item.item,
            kit_item_components(item),
            item.count
        ));
    }
    Some(("bonfire_flask".to_string(), lines(&body)))
}

/// The per-player tag marking which class a player took — the only thing that
/// tells a bonfire rest which flask to refill (`dw.class` is a trigger the class
/// apply resets, and `dw.classed` records only *that* a class was taken).
fn class_tag(class_safe: &str) -> String {
    format!("dw_class_{class_safe}")
}

/// The `bonfire_restore` function: the full player restore of a *rest*.
/// Emitted only for a campaign with a bonfire → byte-identical otherwise.
///
/// Healing is `instant_health`, feeding is `saturation`: vanilla exposes no
/// `/health` or `/food` command and `/data merge entity` refuses players, so
/// these two effects ARE the primitive (CLAUDE.md no-hacks: use the intended one,
/// do not invent a workaround). Both are instant/1-second and leave nothing
/// behind.
fn emit_restore_function(plan: &Plan) -> Option<(String, String)> {
    plan.bonfires().next()?;
    let ns = &plan.namespace;
    let mut body: Vec<String> = vec![
        // Amplifier 9 heals 2 × 2^10 half-hearts — past any `max_health` a kit
        // can reach, so "full" needs no health arithmetic.
        "effect give @s minecraft:instant_health 1 9 true".to_string(),
        // Saturation adds food + saturation every tick it runs; one second at
        // amplifier 9 pins both bars at full.
        "effect give @s minecraft:saturation 1 9 true".to_string(),
    ];
    body.extend(
        HARMFUL_EFFECTS
            .iter()
            .map(|e| format!("effect clear @s {e}")),
    );
    if !plan.flasks().is_empty() {
        body.push(format!("function {ns}:bonfire_flask"));
    }
    Some(("bonfire_restore".to_string(), lines(&body)))
}

/// The `bonfire_rest_<i>` functions (spec-0016 §1). Resting is the party-wide
/// event that (a) moves the respawn point to this bonfire — the same three lines
/// a `set-checkpoint` emits, so `dw:cp`, `spawnpoint` and the `#cp` marker stay
/// one shared contract — and (b) runs the `on_rest` scene reset.
///
/// **Audience (spec-0018).** Resting is a **party event** dispatched from the
/// tick, which carries no `@s`, so the bundle is emitted with
/// [`Audience::Scheduled`]: player-facing effects address `@a` — the whole party
/// rests together — and party-state effects name no player and fire once. The
/// respawn path runs the SAME authored effects through `cp_on_respawn_<i>` under
/// [`Audience::Solo`], because a death belongs to the one player who died. That
/// asymmetry is deliberate and is exactly why spec-0016 requires `on_rest` to be
/// idempotent: it is the world's single answer to both a rest and a death, read
/// at two different audiences.
fn emit_bonfire_functions(plan: &Plan) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut fns: Vec<(String, String)> = Vec::new();
    for bf in plan.bonfires() {
        let i = bf.index;
        let pos = bf.pos;
        // The three lines a `set-checkpoint` writes: vanilla's respawn point, the
        // `dw:cp` mirror every other feature reads, and the active-checkpoint
        // marker that selects the respawn hook. This IS "save".
        let save: Vec<String> = {
            let mut s = vec![
                format!("spawnpoint @a {} {} {}", pos[0], pos[1], pos[2]),
                format!(
                    "data modify storage dw:cp pos set value [{}, {}, {}]",
                    pos[0], pos[1], pos[2]
                ),
            ];
            if plan.any_checkpoint() {
                s.push(format!("scoreboard players set #cp dw.sys {i}"));
            }
            s
        };

        // --- the choice dialog opener (run AS the clicking player) ---
        // The advancement re-arms itself so a bonfire can be opened again and
        // again; `dw.rest` is reset before it is enabled so a stale answer from
        // an earlier rest can never fire the moment the dialog opens.
        fns.push((
            format!("bonfire_open_{i}"),
            lines(&[
                format!("advancement revoke @s only {ns}:bf_{i}"),
                format!("scoreboard players set @s dw.rest_at {i}"),
                "scoreboard players reset @s dw.rest".to_string(),
                "scoreboard players enable @s dw.rest".to_string(),
                format!("dialog show @s {ns}:bonfire_{i}"),
            ]),
        ));

        // --- option 1: save only. The checkpoint moves; NOTHING else happens. ---
        fns.push((format!("bonfire_save_{i}"), lines(&save)));
        fns.push((
            format!("bonfire_pick_save_{i}"),
            lines(&[
                "scoreboard players reset @s dw.rest".to_string(),
                format!("function {ns}:bonfire_save_{i}"),
            ]),
        ));

        // --- option 2: rest and save. Restore the resting player, then the
        // party-wide save + scene reset. ---
        fns.push((
            format!("bonfire_pick_rest_{i}"),
            lines(&[
                "scoreboard players reset @s dw.rest".to_string(),
                format!("function {ns}:bonfire_restore"),
                format!("function {ns}:bonfire_rest_{i}"),
            ]),
        ));

        // `bonfire_rest_<i>` stays exactly what it was: the party-wide half of a
        // rest. The respawn path and the generated PackTests both drive it
        // directly, so it must remain callable with no player restore attached.
        let mut body = save;
        body.extend(bonfire_reseat_lines(plan));
        body.extend(emit_effect_bundle(
            plan,
            &bf.on_respawn,
            Audience::Scheduled,
        ));
        fns.push((format!("bonfire_rest_{i}"), lines(&body)));
    }
    if let Some(f) = emit_restore_function(plan) {
        fns.push(f);
    }
    if let Some(f) = emit_flask_function(plan) {
        fns.push(f);
    }
    fns
}

// ---------------------------------------------------------------------------
// DSL v0.10 trade and the recovery stake (spec-0032)
// ---------------------------------------------------------------------------

/// The `dw.sys` fake player holding the amount a death is forfeiting, while it is
/// being computed. Scratch, never read outside one function chain.
const STK_AMT: &str = "#stk_amt";
/// `dw.sys` scratch holding the block coordinates of the marker under discussion.
const STK_X: &str = "#stk_x";
const STK_Y: &str = "#stk_y";
const STK_Z: &str = "#stk_z";
/// `dw.sys` scratch counting how many players still have a live stake at
/// `#stk_x/y/z` — the reference count that decides whether a marker may be retired.
const STK_REF: &str = "#stk_ref";
/// `dw.sys` scratch: whether a `collect_by: anyone` sweep actually took anything.
const STK_GOT: &str = "#stk_got";
/// The `dw.sys` fake player holding the constant `100`, for a proportional forfeit.
const STK_HUNDRED: &str = "#stk_100";

/// The per-player objective holding slot `k`'s **amount** for stake `s`.
fn stk_amount_obj(s: &str, k: u32) -> String {
    format!("dw.kv{k}_{s}")
}
/// The per-player objective holding whether slot `k` of stake `s` is **live**.
fn stk_live_obj(s: &str, k: u32) -> String {
    format!("dw.kl{k}_{s}")
}
/// The per-player objective holding one axis of slot `k`'s marker position.
fn stk_pos_obj(s: &str, k: u32, axis: usize) -> String {
    format!("dw.k{}{k}_{s}", ["x", "y", "z"][axis])
}
/// The interaction hitbox tag for stake `s`'s markers. **One tag for every marker
/// of a stake**, not one per marker: a marker is a *place*, and which players have
/// a wager there is the per-player ledger's business, not the entity's.
fn stk_tag(s: &str) -> String {
    format!("dw_stk_{s}")
}

/// Every declared stake, paired with the `safe_local` segment naming its functions
/// and objectives. Empty for a campaign that declares none, which is what keeps
/// every existing campaign's emission byte-identical.
fn stakes<'a>(plan: &'a Plan) -> Vec<(&'a delvewright_dsl::Stake, String)> {
    plan.campaign
        .quests
        .content
        .stakes
        .iter()
        .map(|s| (s, plan::safe_local(s.id.as_str())))
        .collect()
}

/// Every declared shop, paired with its index and resolved anchor cell. A shop
/// whose anchor no placed piece provides is dropped here — already `DW0142` at
/// validation, and re-reporting it from emission would blame the wrong layer.
fn shops<'a>(plan: &'a Plan) -> Vec<(usize, &'a delvewright_dsl::Shop, [i32; 3])> {
    plan.campaign
        .quests
        .content
        .shops
        .iter()
        .enumerate()
        .filter_map(|(i, sh)| plan.point_any(sh.anchor.as_str()).map(|p| (i, sh, p)))
        .collect()
}

/// The shadow objective holding the value a **named** datum was last announced at
/// (DSL v0.10, spec-0032).
fn state_shadow_score(id: &str) -> String {
    format!("dw.sh_{}", plan::safe_local(id))
}

/// Every declared datum that carries a player-visible `name` — i.e. every currency.
/// Empty for a campaign that names none, which is what keeps a spec-0031 campaign
/// byte-identical.
fn named_states<'a>(plan: &'a Plan) -> Vec<&'a delvewright_dsl::StateDecl> {
    declared_states(plan.campaign)
        .iter()
        .filter(|st| st.name.is_some())
        .collect()
}

/// **A named datum announces itself when it changes** (DSL v0.10, spec-0032): the
/// tick driver and the per-datum announcer behind it.
///
/// The announcement belongs to the DATUM, not to each verb that writes it — which
/// is the same rule that put the numeric comparison in the gate rather than in the
/// shop. Hanging it off the write sites looked simpler and was wrong twice over:
/// there are five of them (three state verbs, the stake's forfeit, the stake's
/// restore) so the readout would be five copies, and — the defect this shape was
/// written to fix, found by reading the generated `shop_pick_0_0` — a readout
/// emitted inside a gated effect carries that effect's gate, which is evaluated
/// AFTER the write it reports. Spend your last ember behind a
/// `requires_state: at-least 1` gate and the balance moves to 0, so the readout's
/// inherited guard no longer holds and the one change the player most needs to see
/// is the one they are never told about.
///
/// A shadow score per named datum removes the whole class: nothing consults a gate,
/// the announcement fires on **any** change from any cause — a purchase, a death's
/// forfeit, a stake collected, a plain `set-state` — and it fires exactly once per
/// change. The shadow is seeded alongside the datum itself, so joining a world
/// announces nothing.
fn named_state_tick(plan: &Plan) -> Vec<String> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for st in named_states(plan) {
        let id = st.id.as_str();
        let (obj, shadow) = (plan::state_score(id), state_shadow_score(id));
        let f = format!("st_show_{}", plan::safe_local(id));
        match st.scope {
            StateScope::Player => out.push(format!(
                "execute as @a unless score @s {obj} = @s {shadow} run function {ns}:{f}"
            )),
            StateScope::Party => out.push(format!(
                "execute unless score {p} {obj} = {p} {shadow} run function {ns}:{f}",
                p = plan::PARTY
            )),
        }
    }
    out
}

/// The `st_show_<datum>` functions: say the balance, then remember having said it.
///
/// The value travels as vanilla's own `{"score":…}` component, so the line the
/// player reads is the live balance rather than a number baked at emit time, and
/// the name travels as a `{translate, fallback}` component like every other
/// authored string.
fn emit_named_state_functions(plan: &Plan) -> Vec<(String, String)> {
    let mut fns = Vec::new();
    for st in named_states(plan) {
        let id = st.id.as_str();
        let (obj, shadow) = (plan::state_score(id), state_shadow_score(id));
        let name = st.name.as_deref().unwrap_or_default();
        let (who, holder) = match st.scope {
            StateScope::Player => ("@s".to_string(), "@s".to_string()),
            StateScope::Party => ("@a".to_string(), plan::PARTY.to_string()),
        };
        let component = json!([
            { "text": "" },
            tr(name),
            { "text": ": " },
            { "score": { "name": holder, "objective": obj }, "color": "gold" }
        ]);
        fns.push((
            format!("st_show_{}", plan::safe_local(id)),
            lines(&[
                format!("title {who} actionbar {component}"),
                format!("scoreboard players operation {holder} {shadow} = {holder} {obj}"),
            ]),
        ));
    }
    fns
}

/// `setup` lines declaring the economy's scoreboard objectives and constants.
///
/// Empty for a campaign that declares neither a shop nor a stake — the byte-identity
/// rule every section of `setup` follows.
fn economy_setup(plan: &Plan) -> Vec<String> {
    let mut out = Vec::new();
    if !shops(plan).is_empty() {
        // The answer channel and the routing channel, exactly the pair a bonfire
        // rest uses: `/trigger` is the only command a non-op player may run, so one
        // trigger objective carries every shop's answer and a dummy says which shop
        // this player opened.
        out.push("scoreboard objectives add dw.shop trigger".to_string());
        out.push("scoreboard objectives add dw.shop_at dummy".to_string());
    }
    let sts = stakes(plan);
    if sts.is_empty() {
        return out;
    }
    // The respawn point in force is the table's key, and `#cp` is where the runtime
    // keeps it. It is otherwise only written by `set-checkpoint` / a bonfire save,
    // so before the first checkpoint it would be ABSENT — and an absent score
    // matches no `matches` range, which would silently drop every entry-seat row of
    // the table. Seeded to −1, the value the entry seat's rows are keyed on.
    out.push("scoreboard players set #cp dw.sys -1".to_string());
    out.push(format!("scoreboard players set {STK_HUNDRED} dw.sys 100"));
    for (st, safe) in &sts {
        for k in 0..st.max_live() {
            out.push(format!(
                "scoreboard objectives add {} dummy",
                stk_amount_obj(safe, k)
            ));
            out.push(format!(
                "scoreboard objectives add {} dummy",
                stk_live_obj(safe, k)
            ));
            for axis in 0..3 {
                out.push(format!(
                    "scoreboard objectives add {} dummy",
                    stk_pos_obj(safe, k, axis)
                ));
            }
        }
        if let delvewright_dsl::Forfeit::Proportion { percent } = st.forfeit() {
            out.push(format!(
                "scoreboard players set #stk_p{percent} dw.sys {percent}"
            ));
        }
    }
    out
}

/// `setup_finish` lines arming every shop's affordance: the invisible
/// `minecraft:interaction` the player right-clicks and the glowing
/// `minecraft:item_display` that says there is something here.
///
/// Armed at world init rather than at a beat, because a shop is furniture — the
/// same reasoning that arms a shortcut's unlock lever at world init. Guarded by an
/// absence test so a `/reload` cannot stack a second hitbox in one cell (`DW0422`).
fn shop_setup(plan: &Plan) -> Vec<String> {
    let mut out = Vec::new();
    for (i, sh, pos) in shops(plan) {
        let v = ent_xyz(pos);
        let tag = format!("dw_shop_{i}");
        out.push(format!(
            "execute unless entity @e[tag={tag}] run summon minecraft:interaction {} {} {} {{width:1.0f,height:2.0f,response:1b,Invulnerable:1b,Tags:[{FIXTURE_NBT}\"{tag}\"]}}",
            v[0], v[1], v[2]
        ));
        let hw = crate::affordance::hardware_tag(&tag);
        out.push(format!(
            "execute unless entity @e[tag={hw}] run {}",
            affordance_hardware(
                v.clone(),
                &tag,
                sh.marker_item.as_deref().unwrap_or("minecraft:emerald")
            )
        ));
    }
    out
}

/// `tick` lines for the economy.
///
/// Two dispatches, both the shape the rest of the engine already uses:
/// a shop answer read off `dw.shop`/`dw.shop_at` exactly as a bonfire's is, and the
/// stake marker garbage-collector.
///
/// **Ordering is load-bearing.** These lines are appended AFTER the death-edge
/// dispatch, because a stake dropped by `on_death` in this same tick writes its
/// slot inside that dispatch: a collector running first would see a reference count
/// of zero and delete the marker it had just been given.
fn economy_tick(plan: &Plan) -> Vec<String> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for (i, sh, _) in shops(plan) {
        for (j, _) in sh.offers.iter().enumerate() {
            out.push(format!(
                "execute as @a[scores={{dw.shop={},dw.shop_at={i}}}] run function {ns}:shop_pick_{i}_{j}",
                j + 1
            ));
        }
    }
    for (st, safe) in stakes(plan) {
        // `max_live: 0` is the no-death-cost configuration: no marker is ever
        // placed, so there is no marker machinery at all — not even a collector
        // looping over an empty selector.
        if st.max_live() == 0 {
            continue;
        }
        out.push(format!(
            "execute as @e[tag={}] at @s run function {ns}:stk_gc_{safe}",
            stk_tag(&safe)
        ));
    }
    out
}

/// The commands that compute a death's forfeit into [`STK_AMT`], per the stake's
/// declared `forfeit` rule.
///
/// Integer arithmetic throughout (ADR-0006): vanilla's `*=` and `/=` are the only
/// operators involved, and `/=` floors — documented rather than papered over,
/// because a balance is non-negative by the clamp below and flooring and truncation
/// agree there.
fn stake_forfeit_lines(plan: &Plan, st: &delvewright_dsl::Stake) -> Vec<String> {
    let obj = plan::state_score(st.state.as_str());
    let mut out = Vec::new();
    match st.forfeit() {
        delvewright_dsl::Forfeit::None => {
            out.push(format!("scoreboard players set {STK_AMT} dw.sys 0"));
        }
        delvewright_dsl::Forfeit::All => {
            out.push(format!(
                "scoreboard players operation {STK_AMT} dw.sys = @s {obj}"
            ));
        }
        delvewright_dsl::Forfeit::Proportion { percent } => {
            out.push(format!(
                "scoreboard players operation {STK_AMT} dw.sys = @s {obj}"
            ));
            out.push(format!(
                "scoreboard players operation {STK_AMT} dw.sys *= #stk_p{percent} dw.sys"
            ));
            out.push(format!(
                "scoreboard players operation {STK_AMT} dw.sys /= {STK_HUNDRED} dw.sys"
            ));
        }
        delvewright_dsl::Forfeit::Fixed { amount } => {
            out.push(format!(
                "scoreboard players operation {STK_AMT} dw.sys = @s {obj}"
            ));
            // min(balance, amount) — a fixed forfeit can never overdraw a purse.
            if amount < i32::MAX {
                out.push(format!(
                    "execute if score {STK_AMT} dw.sys matches {}.. run scoreboard players set {STK_AMT} dw.sys {amount}",
                    amount.saturating_add(1)
                ));
            }
        }
    }
    // A negative balance forfeits nothing: a death must never HAND the player
    // money, which is what an unclamped `= @s <balance>` would do.
    out.push(format!(
        "execute if score {STK_AMT} dw.sys matches ..-1 run scoreboard players set {STK_AMT} dw.sys 0"
    ));
    let _ = plan;
    out
}

/// Every emitted function the recovery stake needs (DSL v0.10, spec-0032).
///
/// The chain, for one stake:
///
/// | function | run as | what it does |
/// |---|---|---|
/// | `stk_drop_<s>` | the corpse | apply the retention policy, compute and debit the forfeit, then route |
/// | `stk_route_<s>` | the corpse | **the compile-time table**, as one `execute if` chain — the death region test, the respawn-seat test, and the anchor each pair resolved to |
/// | `stk_put_<s>_<n>` | the corpse | position at table anchor `n` |
/// | `stk_here_<s>` | the corpse | position at the death point (the rule's degenerate branch) |
/// | `stk_fill_<s>` | the corpse, positioned | summon the marker if this place has none, then take the first free slot |
/// | `stk_slot_<s>_<k>` | the corpse, positioned | write the amount and the marker's position into slot `k` |
/// | `stk_evict_<s>` | the corpse | the `replace` policy: free slot 0 |
/// | `stk_collect_<s>` | the collecting player | identify which slot this marker holds, and take it |
/// | `stk_take_<s>_<k>` | the collecting player | restore the amount, clear the slot, say so |
/// | `stk_pool_<s>` | each player | the `collect_by: anyone` sweep |
/// | `stk_ref_<s>` | each player | count live slots at `#stk_x/y/z` |
/// | `stk_gc_<s>` | each marker | retire a marker nobody has a wager at — **the one legal killer of its hardware** (`DW0421`) |
///
/// **Idempotency under a double right-click in one tick** (AC6) is structural, not
/// timed: `stk_take_<s>_<k>` sets the slot's live flag to 0 as part of taking it, so
/// a second pass in the same tick matches no slot and does nothing. There is no
/// second-click window to lose a race in.
fn emit_stake_functions(
    plan: &Plan,
    table: Option<&crate::stake::StakeTable>,
) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut fns: Vec<(String, String)> = Vec::new();
    for (st, safe) in stakes(plan) {
        let max = st.max_live();
        let tag = stk_tag(&safe);
        let hw = crate::affordance::hardware_tag(&tag);
        let obj = plan::state_score(st.state.as_str());

        // --- stk_drop: policy, forfeit, route --------------------------------
        let mut drop: Vec<String> = Vec::new();
        if max > 0 {
            // "every slot is live" as one condition; with `max_live: 1` it is the
            // single `if score … matches 1` a souls loop wants.
            let full: String = (0..max)
                .map(|k| format!(" if score @s {} matches 1", stk_live_obj(&safe, k)))
                .collect();
            match st.on_full() {
                delvewright_dsl::OnFull::Keep => {
                    drop.push(format!("execute{full} run return fail"));
                }
                delvewright_dsl::OnFull::Replace => {
                    drop.push(format!("execute{full} run function {ns}:stk_evict_{safe}"));
                }
            }
            drop.extend(stake_forfeit_lines(plan, st));
            drop.push(format!(
                "scoreboard players operation @s {obj} -= {STK_AMT} dw.sys"
            ));
            drop.push(format!("function {ns}:stk_route_{safe}"));
        }
        fns.push((format!("stk_drop_{safe}"), lines(&drop)));

        if max > 0 {
            // --- stk_evict: the `replace` policy -----------------------------
            // The evicted marker is NOT killed here. Its liveness is decided by the
            // reference count in `stk_gc_<s>`, which is the one mechanism that
            // retires a marker — so an eviction whose marker sits in an unloaded
            // chunk is not a leak, it is a retirement deferred to the tick that
            // chunk next loads.
            fns.push((
                format!("stk_evict_{safe}"),
                lines(&[
                    format!("scoreboard players set @s {} 0", stk_live_obj(&safe, 0)),
                    format!("scoreboard players set @s {} 0", stk_amount_obj(&safe, 0)),
                ]),
            ));

            // --- stk_route: THE TABLE ----------------------------------------
            let mut route: Vec<String> = Vec::new();
            if let Some(t) = table {
                for row in &t.rows {
                    let seat = &t.seats[row.seat];
                    let region = &t.regions[row.region];
                    route.push(format!(
                        "execute if score #cp dw.sys matches {} if entity @s[{}] run return run function {ns}:stk_put_{safe}_{}",
                        seat.cp,
                        selector_box(region.region),
                        row.anchor
                    ));
                }
            }
            // The degenerate branch, and the common one: a death on ordinary
            // walkable ground leaves its stake where the player fell, because the
            // nearest point of the route back to a place you can walk to IS that
            // place. `DW0525` is what makes that safe to say unconditionally.
            route.push(format!("function {ns}:stk_here_{safe}"));
            fns.push((format!("stk_route_{safe}"), lines(&route)));

            if let Some(t) = table {
                for (n, a) in t.anchors.iter().enumerate() {
                    let v = ent_xyz(*a);
                    fns.push((
                        format!("stk_put_{safe}_{n}"),
                        lines(&[format!(
                            "execute positioned {} {} {} run function {ns}:stk_fill_{safe}",
                            v[0], v[1], v[2]
                        )]),
                    ));
                }
            }
            fns.push((
                format!("stk_here_{safe}"),
                lines(&[format!("execute at @s run function {ns}:stk_fill_{safe}")]),
            ));

            // --- stk_fill: the marker, then the first free slot ---------------
            let mut fill: Vec<String> = vec![
                format!(
                    "execute unless entity @e[tag={tag},distance=..1] run summon minecraft:interaction ~ ~ ~ {{width:1.0f,height:2.0f,response:1b,Invulnerable:1b,Tags:[{FIXTURE_NBT}\"{tag}\"]}}"
                ),
                format!(
                    "execute unless entity @e[tag={hw},distance=..1] run summon minecraft:item_display ~ ~ ~ {{Glowing:1b,Tags:[{FIXTURE_NBT}\"dw_marker\",\"{hw}\"],billboard:\"center\",item:{{id:\"{}\",count:1}}}}",
                    st.marker_item()
                ),
            ];
            for k in 0..max {
                fill.push(format!(
                    "execute unless score @s {} matches 1 run return run function {ns}:stk_slot_{safe}_{k}",
                    stk_live_obj(&safe, k)
                ));
            }
            fns.push((format!("stk_fill_{safe}"), lines(&fill)));

            for k in 0..max {
                let mut slot = vec![
                    format!(
                        "scoreboard players operation @s {} = {STK_AMT} dw.sys",
                        stk_amount_obj(&safe, k)
                    ),
                    format!("scoreboard players set @s {} 1", stk_live_obj(&safe, k)),
                ];
                // The position is read back off the marker rather than written from
                // the anchor the table chose, so the compile-time branch and the
                // runtime "here" branch record it the SAME way — and a collector
                // comparing the two is comparing two `data get`s of one double, not
                // a coordinate against a rounding of it.
                for axis in 0..3 {
                    slot.push(format!(
                        "execute store result score @s {} run data get entity @e[tag={tag},limit=1,sort=nearest] Pos[{axis}]",
                        stk_pos_obj(&safe, k, axis)
                    ));
                }
                fns.push((format!("stk_slot_{safe}_{k}"), lines(&slot)));
            }

            // --- stk_collect: the right-click ---------------------------------
            let mut collect: Vec<String> =
                vec![format!("advancement revoke @s only {ns}:stk_{safe}")];
            for (axis, s) in [STK_X, STK_Y, STK_Z].iter().enumerate() {
                collect.push(format!(
                    "execute at @s store result score {s} dw.sys run data get entity @e[tag={tag},limit=1,sort=nearest] Pos[{axis}]"
                ));
            }
            match st.collect_by() {
                delvewright_dsl::CollectBy::Owner => {
                    for k in 0..max {
                        collect.push(format!(
                            "execute{} run return run function {ns}:stk_take_{safe}_{k}",
                            slot_match(&safe, k)
                        ));
                    }
                }
                delvewright_dsl::CollectBy::Anyone => {
                    collect.push(format!("scoreboard players set {STK_AMT} dw.sys 0"));
                    collect.push(format!("scoreboard players set {STK_GOT} dw.sys 0"));
                    collect.push(format!("execute as @a run function {ns}:stk_pool_{safe}"));
                    collect.push(format!(
                        "execute if score {STK_GOT} dw.sys matches 1.. run scoreboard players operation @s {obj} += {STK_AMT} dw.sys"
                    ));
                    collect.push(format!(
                        "execute if score {STK_GOT} dw.sys matches 1.. run title @s actionbar {}",
                        tr(&st.collected_message)
                    ));
                }
            }
            fns.push((format!("stk_collect_{safe}"), lines(&collect)));

            if matches!(st.collect_by(), delvewright_dsl::CollectBy::Anyone) {
                let mut pool: Vec<String> = Vec::new();
                for k in 0..max {
                    pool.push(format!(
                        "execute{} run scoreboard players operation {STK_AMT} dw.sys += @s {}",
                        slot_match(&safe, k),
                        stk_amount_obj(&safe, k)
                    ));
                    pool.push(format!(
                        "execute{} run scoreboard players set {STK_GOT} dw.sys 1",
                        slot_match(&safe, k)
                    ));
                    pool.push(format!(
                        "execute{} run scoreboard players set @s {} 0",
                        slot_match(&safe, k),
                        stk_amount_obj(&safe, k)
                    ));
                    // Clearing the live flag LAST: the three lines above all test it.
                    pool.push(format!(
                        "execute{} run scoreboard players set @s {} 0",
                        slot_match(&safe, k),
                        stk_live_obj(&safe, k)
                    ));
                }
                fns.push((format!("stk_pool_{safe}"), lines(&pool)));
            } else {
                for k in 0..max {
                    let mut take = vec![
                        format!(
                            "scoreboard players operation @s {obj} += @s {}",
                            stk_amount_obj(&safe, k)
                        ),
                        format!("scoreboard players set @s {} 0", stk_amount_obj(&safe, k)),
                        format!("scoreboard players set @s {} 0", stk_live_obj(&safe, k)),
                    ];
                    take.push(format!("title @s actionbar {}", tr(&st.collected_message)));
                    fns.push((format!("stk_take_{safe}_{k}"), lines(&take)));
                }
            }

            // --- stk_ref / stk_gc: who still has a wager here ------------------
            let mut refs: Vec<String> = Vec::new();
            for k in 0..max {
                refs.push(format!(
                    "execute{} run scoreboard players add {STK_REF} dw.sys 1",
                    slot_match(&safe, k)
                ));
            }
            fns.push((format!("stk_ref_{safe}"), lines(&refs)));

            let mut gc: Vec<String> = Vec::new();
            for (axis, s) in [STK_X, STK_Y, STK_Z].iter().enumerate() {
                gc.push(format!(
                    "execute store result score {s} dw.sys run data get entity @s Pos[{axis}]"
                ));
            }
            gc.push(format!("scoreboard players set {STK_REF} dw.sys 0"));
            gc.push(format!("execute as @a run function {ns}:stk_ref_{safe}"));
            gc.push(format!(
                "execute if score {STK_REF} dw.sys matches 0 run kill @e[tag={hw},limit=1,sort=nearest]"
            ));
            gc.push(format!(
                "execute if score {STK_REF} dw.sys matches 0 run kill @s"
            ));
            fns.push((format!("stk_gc_{safe}"), lines(&gc)));
        }
    }
    fns
}

/// The `execute` sub-condition matching *this player's slot `k` is live and sits at
/// the marker under discussion* (`#stk_x/y/z`). Space-prefixed, so a caller splices
/// it straight onto `execute`.
fn slot_match(safe: &str, k: u32) -> String {
    let mut s = format!(" if score @s {} matches 1", stk_live_obj(safe, k));
    for (axis, scratch) in [STK_X, STK_Y, STK_Z].iter().enumerate() {
        s.push_str(&format!(
            " if score @s {} = {scratch} dw.sys",
            stk_pos_obj(safe, k, axis)
        ));
    }
    s
}

/// A box as an entity-selector volume (`x=…,dx=…`) — the test the corpse is put to
/// so the placement table's death-region axis is a comparison rather than a search.
///
/// `dx` is the *span*, and a selector volume is half-open on the far side, so an
/// inclusive box `[lo, hi]` spans `hi − lo + 1` cells.
fn selector_box(region: ([i32; 3], [i32; 3])) -> String {
    let (lo, hi) = region;
    format!(
        "x={},y={},z={},dx={},dy={},dz={}",
        lo[0],
        lo[1],
        lo[2],
        hi[0] - lo[0] + 1,
        hi[1] - lo[1] + 1,
        hi[2] - lo[2] + 1
    )
}

/// Every emitted function a shop needs (DSL v0.10, spec-0032).
///
/// `shop_open_<i>` is the bonfire opener with a different dialog: revoke the
/// advancement so the shop can be opened again, record which shop this player is
/// standing at, reset-then-enable the trigger (reset both clears a stale answer and
/// re-locks the trigger — the order is what stops a previous visit's answer firing
/// as the screen opens), and show the dialog.
///
/// `shop_pick_<i>_<j>` disarms first, then applies the offer's own gate as
/// `return fail` — the same inert-to-a-direct-`/trigger` discipline a dialogue
/// option's handler uses, so a bot chatting `/trigger dw.shop set 3` cannot buy
/// what the gate refuses — and then runs the offer's effects with the buying player
/// as `@s`.
fn emit_shop_functions(plan: &Plan) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut fns: Vec<(String, String)> = Vec::new();
    for (i, sh, _) in shops(plan) {
        fns.push((
            format!("shop_open_{i}"),
            lines(&[
                format!("advancement revoke @s only {ns}:shop_{i}"),
                format!("scoreboard players set @s dw.shop_at {i}"),
                "scoreboard players reset @s dw.shop".to_string(),
                "scoreboard players enable @s dw.shop".to_string(),
                format!("dialog show @s {ns}:shop_{i}"),
            ]),
        ));
        for (j, off) in sh.offers.iter().enumerate() {
            let mut body: Vec<String> = vec!["scoreboard players reset @s dw.shop".to_string()];
            for f in &off.requires_flags {
                body.push(format!(
                    "execute unless score {} {} matches 1 run return fail",
                    plan::PARTY,
                    plan::flag_score(f.as_str())
                ));
            }
            for f in &off.forbids_flags {
                body.push(format!(
                    "execute if score {} {} matches 1 run return fail",
                    plan::PARTY,
                    plan::flag_score(f.as_str())
                ));
            }
            for clause in state_clauses(plan, &off.requires_state, true) {
                body.push(format!("execute {clause} run return fail"));
            }
            body.extend(emit_effect_bundle(
                plan,
                &off.effects,
                root_audience(delvewright_dsl::EffectRootKind::ShopOffer),
            ));
            fns.push((format!("shop_pick_{i}_{j}"), lines(&body)));
        }
    }
    fns
}

/// Entity types the **engine itself** places as machinery, never as bodies a
/// player fights or talks to: the `minecraft:interaction` hitboxes behind every
/// affordance, the cutscene camera and its marks, the display entities behind an
/// art title. A lethal volume must not delete them — a volume drawn across a
/// cutscene dolly would otherwise erase the camera mid-shot, and one over a gate
/// seal would erase the thing the player presses.
///
/// The list is exactly the set the compiler `summon`s as machinery, and it is a
/// list of **types** rather than of tags on purpose: a vanilla selector cannot
/// match a tag prefix, and every alternative — one negated tag per feature — grows
/// with the engine and is silently wrong the day a feature is added. Content
/// bodies (a wave mob, an actor puppet, an NPC) are deliberately NOT here: a mob
/// that walks into the lava dies, which is the mechanism working. An NPC posted
/// inside a volume is a content defect the campaign's own placement proofs and
/// `DW0511` are there to surface, not something to hide by exempting NPCs.
///
/// **It is no longer the whole answer, and it is kept anyway.** The engine's own
/// places now declare a class ([`crate::affordance::FIXTURE_TAG`]) and the
/// volume's selector negates it like every other box-narrowed entity selector, so
/// the sentence above — "one negated tag per feature" — is answered: it is one
/// negated tag for the whole engine, forever, because the class is decided at the
/// object rather than per feature. This roster stays because it is not the same
/// claim. It says *do not aim `/damage` at a thing that cannot take it*, which is
/// still true of a `block_display` that no fixture happens to be today; the class
/// says *do not disturb a place*. Deleting the roster would trade a live
/// statement for a shorter line.
const LETHAL_EXEMPT_TYPES: [&str; 5] = [
    "minecraft:interaction",
    "minecraft:marker",
    "minecraft:item_display",
    "minecraft:block_display",
    "minecraft:text_display",
];

/// Scratch score holding the struck player's health **after** a lethal volume's
/// blow — the world state the volume's wording asserts, read back rather than
/// predicted.
///
/// **Measured, after getting it wrong once.** Vanilla refuses damage far more
/// often than "the target is dead" suggests: a player is invulnerable for **59
/// ticks (~3 s) after respawning**, and `/damage` (like `/kill`) reports success
/// and does nothing (spec-0031 spike). The obvious guard — `execute store
/// success ... run damage` — is therefore **inert**, and this was measured on the
/// pinned 1.21.11 toolserver rather than reasoned about: a PackTest dummy in
/// `playerGameType: 0` (survival) with `Invulnerable: 0` and `Health: 20f` took
/// `damage @s 1000 minecraft:fall`, ended on `Health: 20f`, and the command
/// answered **success = 1**. Reading a response that does not carry the answer is
/// the same defect as reading no response at all — which is precisely what the
/// eight legacy camelCase gamerules did at two sites in that same spike.
///
/// So the guard reads the **outcome**: the wording is printed only when the
/// player it is about actually ended the tick dead. That covers every refusal in
/// one rule and needs no list of them — the respawn window, a totem (the volume
/// struck and did not take them; the next tick will), `resistance`, creative.
/// Reset to a sentinel before the read so a failed `data get` can never leave a
/// previous player's zero behind and claim a death that did not happen.
const LETHAL_HP: &str = "#leth_hp dw.sys";

/// Damage dealt by a lethal volume, in half-hearts. Far above any reachable
/// max-health + absorption, so "lethal" is a property of the verb and not a number
/// an author has to get right. A held totem still fires — the curated
/// [`delvewright_dsl::DamageKind`] set deliberately excludes every totem-bypassing
/// vanilla type — and the next tick inside the volume kills anyway, which is the
/// totem doing its job rather than the volume failing to do its own.
const LETHAL_DAMAGE: u32 = 1000;

/// The fixture class tag as the leading element of a summon's `Tags:` NBT list —
/// *this entity's position IS engine state* ([`crate::affordance::FIXTURE_TAG`]).
///
/// Written at the summon site rather than derived by a roster in some verb,
/// because the class belongs to the object: a bonfire's hitbox is a place whether
/// the thing quantifying over it is a teleport, a lethal volume or a verb nobody
/// has written yet.
const FIXTURE_NBT: &str = "\"dw_fixture\",";

/// The borne class tag ([`crate::affordance::BORNE_TAG`]) — *this entity's
/// position belongs to a body that carries it*. Exactly one summon in the engine
/// wears it: an NPC's co-located dialogue hitbox, which must ride whatever its
/// speaker rides.
const BORNE_NBT: &str = "\"dw_borne\",";

/// An inclusive block AABB as vanilla selector arguments — `x=…,dx=…,…`, with the
/// fixture-class exclusion every box-narrowed **entity** selector carries.
///
/// One spelling for one fact. Vanilla's `dx` is a *span*, not a count, so the box
/// `lo..=hi` is `dx = hi - lo`; every anchor-centred volume in the engine
/// (`lethal_volumes[]`, a `teleport`'s `from`, a status effect's `in`) resolves
/// through [`crate::plan::Plan::zone_box`] to exactly this pair and formats it
/// here, so no two verbs can disagree by one block about what "inside" means.
///
/// The exclusion is NOT added here, because the *player* selectors
/// (`damage-players`, `give-effect`, the volume's `@a` half) share this
/// formatter and no player is a fixture. [`entity_box_selector`] is the entity
/// spelling, and `DW0545` proves nothing else reaches a box.
pub(crate) fn box_selector_args(lo: [i32; 3], hi: [i32; 3]) -> String {
    format!(
        "x={},dx={},y={},dy={},z={},dz={}",
        lo[0],
        hi[0] - lo[0],
        lo[1],
        hi[1] - lo[1],
        lo[2],
        hi[2] - lo[2]
    )
}

/// A box as the arguments of an `@e[…]` selector: the volume, then the
/// fixture-class exclusion.
///
/// **Every** entity selector narrowed by a box in this engine goes through here,
/// and `DW0545` reads the shipped datapack to prove it. One term, negating one
/// class tag — never a `type=!…` roster, which grows with the engine and, on a
/// verb that moves rather than deletes, would strip an NPC's dialogue hitbox off
/// its body.
fn entity_box_selector(lo: [i32; 3], hi: [i32; 3]) -> String {
    format!(
        "{},{}",
        box_selector_args(lo, hi),
        crate::affordance::FIXTURE_EXCLUDE
    )
}

/// The box-selector argument shared by both of a volume's selectors.
fn lethal_box(v: &crate::plan::LethalVolumePlan) -> String {
    let (lo, hi) = v.region;
    box_selector_args(lo, hi)
}

/// The per-tick driver lines for the campaign's lethal volumes (spec-0031), in
/// declaration order. Empty for a campaign that declares none, so the emitted
/// `tick` is byte-identical for everybody who has not opted in.
fn lethal_tick(plan: &Plan) -> Vec<String> {
    let ns = &plan.namespace;
    plan.lethal_volumes
        .iter()
        .map(|v| format!("function {ns}:lethal_{}", v.safe))
        .collect()
}

/// Generate one function per lethal volume (spec-0031).
///
/// Two lines, and the split is the whole design:
///
/// * **players** are re-bound one at a time (`execute as @a[…] run function`) so
///   the volume's own wording is `tellraw`n to the player it is about, in that
///   player's language — the `{"translate":…,"fallback":…}` component every
///   player-visible string in this engine goes through. It is guarded by
///   `tag=!dw_cutscene` like every other harmful piece of campaign machinery: a
///   player watching a cutscene is an observer, and the camera flies wherever the
///   shot needs it to. **The blow comes first and the wording is conditioned on
///   the player actually ending up dead** ([`LETHAL_HP`]): vanilla refuses damage
///   to a player for 59 ticks after they respawn, and a message printed ahead of
///   the swing would be a line the delve repeats for three seconds about a death
///   that is not happening.
/// * **everything else** takes the same `/damage` in one line, minus the engine's
///   own machinery ([`LETHAL_EXEMPT_TYPES`]).
///
/// The wording is delivered as a component and NOT as a custom `damage_type`
/// `message_id`. Vanilla builds a death message from `message_id` with no
/// `fallback` field, so that spelling would ship a raw translation key
/// (`death.attack.…`) to any player who declines the resource-pack prompt — which
/// spec-0029 §3 makes an invariant against, and which `DW0185` would not catch
/// because the key is not the authored string. Vanilla's own broadcast still
/// fires, worded by the declared `damage_type`: the party reads who died, the
/// victim reads what the place was.
///
/// The kill is an ordinary `/damage`, so the vanilla `deathCount` edge
/// (`dw.deaths` / `dw.death_ack`), the checkpoint re-seat (`cp_respawn_check`) and
/// `keep_inventory` all see exactly the death they already handle. There is no
/// second death detector here, and there is nothing for one to do.
fn emit_lethal_functions(plan: &Plan) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut fns: Vec<(String, String)> = Vec::new();
    for v in &plan.lethal_volumes {
        let bx = lethal_box(v);
        let kind = v.damage_type.id();
        let exempt: String = LETHAL_EXEMPT_TYPES
            .iter()
            .map(|t| format!(",type=!{t}"))
            .collect();
        fns.push((
            format!("lethal_{}", v.safe),
            lines(&[
                format!(
                    "execute as @a[{bx},tag=!{CUTSCENE_TAG}] run function {ns}:lethal_{}_kill",
                    v.safe
                ),
                format!(
                    "execute as @e[{bx},{},type=!minecraft:player{exempt}] run damage @s \
                     {LETHAL_DAMAGE} {kind}",
                    crate::affordance::FIXTURE_EXCLUDE
                ),
            ]),
        ));
        fns.push((
            format!("lethal_{}_kill", v.safe),
            lines(&[
                format!("damage @s {LETHAL_DAMAGE} {kind}"),
                format!("scoreboard players set {LETHAL_HP} 1"),
                format!("execute store result score {LETHAL_HP} run data get entity @s Health 100"),
                format!(
                    "execute if score {LETHAL_HP} matches ..0 run tellraw @s {}",
                    tr(&v.message)
                ),
            ]),
        ));
    }
    fns
}

/// Generate the stealth-beat functions (DSL v0.6, spec-0014; no sneak
/// requirement — holding sneak collides with the
/// spectator cutscene camera). For each beat: an `arm` that activates the
/// session and resets per-player grace; a per-tick judge that, per player,
/// tests "inside some zone box" (zone presence alone = hidden), tracks a grace
/// counter, and fires `on_caught` after `grace_ticks` of exposure. Zone
/// membership is a pure position selector, so the whole check is deterministic
/// and provable.
fn emit_stealth_functions(plan: &Plan) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut fns: Vec<(String, String)> = Vec::new();
    for beat in &plan.stealth_beats {
        let i = beat.index;
        // stealth_begin_<i>: activate + reset grace.
        fns.push((
            format!("stealth_begin_{i}"),
            lines(&[
                format!("scoreboard players set #stealth dw.sys {i}"),
                "execute as @a run scoreboard players set @s dw.st_grace 0".to_string(),
            ]),
        ));
        // stealth_tick_<i>: judge every player who is actually playing. A player
        // in the cutscene state is skipped entirely (CUTSCENE_TAG): the judge is
        // the only writer of `dw.st_grace`, so skipping it freezes the clock —
        // grace neither accrues nor expires, and `on_caught` cannot fire at a
        // player who is watching a cinematic in spectator mode.
        fns.push((
            format!("stealth_tick_{i}"),
            lines(&[format!(
                "execute as @a[tag=!{CUTSCENE_TAG}] run function {ns}:stealth_eval_{i}"
            )]),
        ));
        // stealth_eval_<i> (as @s): compute safe flag, update grace, fire caught.
        let mut eval: Vec<String> = vec!["scoreboard players set @s dw.st_safe 0".to_string()];
        for (_, pos, extent) in &beat.zones {
            let lo = [
                pos[0] - extent[0] as i32,
                pos[1] - extent[1] as i32,
                pos[2] - extent[2] as i32,
            ];
            let size = [
                2 * extent[0] as i32,
                2 * extent[1] as i32,
                2 * extent[2] as i32,
            ];
            eval.push(format!(
                "execute if entity @s[x={},dx={},y={},dy={},z={},dz={}] run \
                 scoreboard players set @s dw.st_safe 1",
                lo[0], size[0], lo[1], size[1], lo[2], size[2]
            ));
        }
        eval.push(
            "execute if score @s dw.st_safe matches 1 run scoreboard players set @s dw.st_grace 0"
                .to_string(),
        );
        eval.push(
            "execute if score @s dw.st_safe matches 0 run scoreboard players add @s dw.st_grace 1"
                .to_string(),
        );
        eval.push(format!(
            "execute if score @s dw.st_grace matches {}.. run function {ns}:stealth_caught_{i}",
            beat.grace_ticks
        ));
        fns.push((format!("stealth_eval_{i}"), lines(&eval)));
        // stealth_caught_<i> (as @s): reset grace, run on_caught.
        // `Audience::Solo` (spec-0018): being spotted is one player's event —
        // `stealth_eval_<i>` judges each player separately, so the consequence
        // lands on the player it judged.
        let mut caught: Vec<String> = vec!["scoreboard players set @s dw.st_grace 0".to_string()];
        caught.extend(emit_effect_bundle(plan, &beat.on_caught, Audience::Solo));
        fns.push((format!("stealth_caught_{i}"), lines(&caught)));
    }
    fns
}

/// Emit a `narrate` line in its channel (DSL v0.4). `chat` = `tellraw`; `title`
/// / `subtitle` = the vanilla `title` command (a subtitle is paired with a blank
/// title so it renders on its own). An optional sound plays alongside. `who` is
/// the audience selector (spec-0018): the story is told to the whole party
/// (`@a`), except inside a solo `on_respawn`/`on_caught` bundle (`@s`).
fn emit_narrate(
    text: &str,
    style: Option<delvewright_dsl::NarrateStyle>,
    sound: Option<&str>,
    who: &str,
    body: &mut Vec<String>,
) {
    use delvewright_dsl::NarrateStyle;
    let comp = tr(text);
    match style.unwrap_or(NarrateStyle::Chat) {
        NarrateStyle::Chat => body.push(format!("tellraw {who} {comp}")),
        NarrateStyle::Title => body.push(format!("title {who} title {comp}")),
        NarrateStyle::Subtitle => {
            body.push(format!("title {who} title {}", json!({ "text": " " })));
            body.push(format!("title {who} subtitle {comp}"));
        }
        // Large-glyph "art" title through the delve's custom resource-pack font
        // (`delve:art`, DSL v0.6). The font is uppercase-only (glyph coverage is
        // checked at compile time, DW0328), so render uppercase.
        NarrateStyle::Art => {
            let art = tr_with(text, &[("font", json!("delve:art"))]);
            body.push(format!("title {who} title {art}"));
        }
        // DSL v0.11: the reply strip above the hotbar. This is the command every
        // compiler-written reply has always used — a sealed gate's answer, a
        // checkpoint return, the lobby count — reached at last by the general
        // verb, so a campaign can write its own replies instead of the engine
        // owning them one verb at a time.
        NarrateStyle::Actionbar => body.push(format!("title {who} actionbar {comp}")),
    }
    if let Some(s) = sound {
        body.push(format!("playsound {s} player {who}"));
    }
}

/// Resolve an anchor name to a world point by scanning every area (first match),
/// mirroring how `open-gate` resolves its anchor. `None` if unresolved.
fn anchor_point_any(plan: &Plan, anchor: &str) -> Option<[i32; 3]> {
    plan.point_any(anchor)
}

/// Whether a stage-2 NPC declares `deferred: true` (DSL v0.6) — it is not summoned
/// at world init and enters only via a `spawn-npc` effect.
fn npc_is_deferred(c: &delvewright_dsl::Campaign, npc_id: &str) -> bool {
    c.npcs
        .content
        .npcs
        .iter()
        .find(|n| n.id.as_str() == npc_id)
        .map(|n| n.deferred)
        .unwrap_or(false)
}

/// The one authority for an NPC's world presence: the `/summon` commands that place
/// its body (villager re-dress or mannequin) **and** its co-located interaction
/// hitbox at its declared anchor, with its name display.
///
/// Called from exactly two places — the world-init `setup_finish` block (a normal
/// NPC) and the generated `spawn_npc_<id>` function (a `deferred` NPC, DSL v0.6) —
/// so a scripted entrance produces byte-for-byte the same entity as an init-time
/// one. Extracted for that duality; the command text is unchanged from pre-0.6, so
/// a campaign with no deferred NPC is byte-identical.
fn npc_summon_commands(
    c: &delvewright_dsl::Campaign,
    plan: &Plan,
    npc: &plan::NpcPlan,
    v03: bool,
) -> Vec<String> {
    let area = plan.npc_area(&npc.npc_id).unwrap_or("");
    let dsl_npc = c
        .npcs
        .content
        .npcs
        .iter()
        .find(|n| n.id.as_str() == npc.npc_id);
    let anchor = dsl_npc.map(|n| n.anchor.as_str()).unwrap_or("");
    let (pos, facing) = match plan.anchors.get(&(area.to_string(), anchor.to_string())) {
        Some(ResolvedAnchor::Point { pos, facing }) => (*pos, facing.as_deref()),
        _ => ([0, plan::BASE_Y, 0], None),
    };
    let name = dsl_npc.map(|n| n.name.as_str()).unwrap_or("NPC");
    let base = dsl_npc
        .map(|n| n.base_entity.as_str())
        .unwrap_or("minecraft:villager");
    let yaw = facing_yaw(facing);
    let p = ent_xyz(pos);
    let mut out = Vec::new();
    if let Some(skin) = dsl_npc.and_then(|n| n.skin.as_ref()) {
        // DSL v0.4 mannequin NPC (spec-0008 §6 / spec-0009). The label is
        // emitted as `description`, a **text-component SNBT compound**
        // (`{text:"…"}`) — NOT a stringified-JSON text component
        // (`'{"text":…}'`), which renders as literal raw JSON above the head on
        // 1.21.11 (owner-verified). NoAI/PersistenceRequired/VillagerData are
        // dropped (silently ignored on a mannequin); the interaction hitbox is
        // unchanged.
        // `pose:"standing"` is emitted explicitly: a mannequin summoned without
        // it serializes its pose as `DYING` (a gametest save-teardown warning),
        // wrong data for a standing NPC. Valid 1.21.11 mannequin poses: standing,
        // crouching, swimming, fall_flying, sleeping (spec-0009 template).
        out.push(format!(
            "summon minecraft:mannequin {} {} {} {{profile:{{texture:\"delvewright:npc/{}\",model:\"{}\"}},immovable:1b,pose:\"standing\",Invulnerable:1b,Silent:1b,Rotation:[{yaw}f,0f],description:{},Tags:[\"dw_npc\",\"{}\"]}}",
            p[0], p[1], p[2], skin.texture_id, skin.model.token(),
            snbt_text_component(name), npc.tag
        ));
    } else {
        // CustomName is a 1.21.11 text component. v0.3+ emits a plain SNBT
        // string (renders correctly, incl. death messages — M2 fix 1); v0.2
        // keeps the legacy `'{"text":…}'` form so hello-world / keep-crawl stay
        // byte-identical.
        let cname_field = if v03 {
            snbt_component(name)
        } else {
            let cname = tr(name).to_string().replace('\'', "\\'");
            format!("'{cname}'")
        };
        let pose = mannequin_pose_nbt(base);
        out.push(format!(
            "summon {base} {} {} {} {{NoAI:1b,Invulnerable:1b,Silent:1b,PersistenceRequired:1b,NoGravity:1b{pose},Rotation:[{yaw}f,0f],Tags:[\"dw_npc\",\"{}\"],CustomName:{},CustomNameVisible:1b,VillagerData:{{profession:\"minecraft:none\",type:\"minecraft:plains\",level:1}}}}",
            p[0], p[1], p[2], npc.tag, cname_field
        ));
    }
    // The interaction hitbox also carries the tag of every left-click trigger
    // that watches this NPC — see `npc_hitbox_trigger_tags`.
    let mut tags = vec![npc.tag.clone()];
    tags.extend(npc_hitbox_trigger_tags(c, anchor, &npc.npc_id));
    let tag_list = tags
        .iter()
        .map(|t| format!("\"{t}\""))
        .collect::<Vec<_>>()
        .join(",");
    out.push(format!(
        "summon minecraft:interaction {} {} {} {{width:1.0f,height:2.0f,response:1b,Invulnerable:1b,Tags:[{BORNE_NBT}{tag_list}]}}",
        p[0], p[1], p[2]
    ));
    out
}

/// The `dw_trig_<id>` tags of every `strike` trigger whose `at` anchor is
/// `anchor`, in campaign declaration order (deterministic).
///
/// **Why an NPC's hitbox wears a trigger's tag.** A `strike` trigger is detected
/// by reading the `attack` record off a `minecraft:interaction` entity — the
/// vanilla primitive for "a player left-clicked this". When the trigger's anchor
/// is also where an NPC stands, the NPC's hitbox is the entity a click actually
/// reaches, and the NPC's body is `Invulnerable`, so a trigger listening on an
/// entity of its own could simply never fire (round-4 island QA:
/// `wake-the-giant` on the sleeping giant's anchor was dead).
///
/// The NPC hitbox is the trigger's **sole** carrier: `env_trigger_setup`
/// suppresses the trigger's own summon for this collision. Round-4 shared the
/// tag but kept both entities; the two exactly co-located hitboxes then made
/// the *right*-click pick ambiguous, and when the standalone won, the dialogue
/// advancement (keyed on `Tags:["dw_npc_<n>"]`) never fired — the round-6
/// island soft-lock (Polyphemus untalkable after the boulder seal). One cell,
/// one hitbox ends both failure modes. Empty for an anchor with no co-located
/// strike trigger, so every campaign without this collision stays
/// byte-identical.
///
/// Scope: `strike` only. Right-click (`use`) on an NPC already belongs to the
/// dialogue advancement, so a co-located `use` trigger is an authoring
/// conflict, rejected at validate time (`DW0350`).
/// The first `(trigger, npc id, npc entity tag)` triple whose trigger rides an
/// NPC's own interaction hitbox — either a `strike-npc` naming that NPC (DSL
/// v0.6) or a `strike` whose anchor is that NPC's stand anchor. The collision
/// [`npc_hitbox_trigger_tags`] resolves. Campaign order (deterministic); `None`
/// when no trigger rides an NPC.
fn first_strike_trigger_on_npc<'a>(
    plan: &'a Plan,
) -> Option<(&'a delvewright_dsl::EnvTrigger, String, String)> {
    let c = plan.campaign;
    for t in &c.quests.content.triggers {
        for n in &plan.npcs {
            let decl = c
                .npcs
                .content
                .npcs
                .iter()
                .find(|d| d.id.as_str() == n.npc_id);
            let anchor = decl.map(|d| d.anchor.as_str()).unwrap_or("");
            if trigger_rides_npc(t, anchor, &n.npc_id) {
                return Some((t, n.npc_id.clone(), n.tag.clone()));
            }
        }
    }
    None
}

/// Whether `t` is a left-click trigger carried by the interaction hitbox of the
/// NPC `npc_id` standing at `anchor`.
///
/// Two spellings, one mechanism. `strike-npc` (DSL v0.6) names the NPC
/// **directly** and is the intended form: it works wherever the NPC stands and
/// whatever its body is, because it never asks for a cell. A bare `strike` whose
/// `at` happens to be the NPC's own anchor is the pre-0.6 spelling of the same
/// thing, kept working: co-locating a second interaction entity with an NPC is
/// the one-cell-two-hitboxes defect (`DW0350`/`DW0359`), so the compiler shares
/// the NPC's hitbox instead of summoning one.
fn trigger_rides_npc(t: &delvewright_dsl::EnvTrigger, anchor: &str, npc_id: &str) -> bool {
    use delvewright_dsl::TriggerOn;
    match &t.on {
        TriggerOn::StrikeNpc { npc } => npc.as_str() == npc_id,
        TriggerOn::Strike => !anchor.is_empty() && t.at_anchor() == Some(anchor),
        _ => false,
    }
}

/// True when `anchor` is a planned NPC's stand anchor — the cell where that
/// NPC's interaction hitbox lives, whether summoned at world init or by the
/// NPC's `spawn-npc` entrance (`deferred`). The suppression dual of
/// [`strike_trigger_tags_at`]: a strike trigger rides exactly the hitboxes this
/// predicate says exist.
fn npc_stands_at(plan: &Plan, anchor: &str) -> bool {
    plan.npcs.iter().any(|n| {
        plan.campaign
            .npcs
            .content
            .npcs
            .iter()
            .any(|d| d.id.as_str() == n.npc_id && d.anchor.as_str() == anchor)
    })
}

/// The `dw_trig_<id>` tags every left-click trigger riding this NPC's hitbox
/// contributes, in campaign declaration order (deterministic). Empty for an NPC
/// no trigger watches, so every campaign without one stays byte-identical.
fn npc_hitbox_trigger_tags(
    c: &delvewright_dsl::Campaign,
    anchor: &str,
    npc_id: &str,
) -> Vec<String> {
    c.quests
        .content
        .triggers
        .iter()
        .filter(|t| trigger_rides_npc(t, anchor, npc_id))
        .map(|t| format!("dw_trig_{}", plan::safe_local(t.id.as_str())))
        .collect()
}

/// The generated function name for a `spawn-npc` effect (DSL v0.6).
fn spawn_npc_fn(npc: &str) -> String {
    format!("spawn_npc_{}", plan::safe_local(npc))
}

/// Every NPC a compiled `spawn-npc` site names — the quest/trigger/trap effect
/// trees ([`all_campaign_effects`], nesting included) plus every dialogue option's
/// `spawn-npc`, which the option handler compiles the very same call for.
///
/// This is the emitted-call set for [`spawn_npc_fns`], so the two agree by
/// construction rather than by convention (`DW0497`).
fn spawn_npc_sites(c: &delvewright_dsl::Campaign) -> BTreeSet<String> {
    let mut out: BTreeSet<String> = BTreeSet::new();
    for e in all_campaign_effects(c) {
        if let Some(npc) = e.spawn_npc() {
            out.insert(npc.as_str().to_string());
        }
    }
    for tree in &c.dialogue.content.dialogues {
        for node in &tree.nodes {
            for opt in &node.options {
                for eff in &opt.effects {
                    if let Some(npc) = eff.spawn_npc() {
                        out.insert(npc.as_str().to_string());
                    }
                }
            }
        }
    }
    out
}

/// `spawn_npc_<id>` functions (DSL v0.6): one per NPC any `spawn-npc` effect
/// summons, the scripted-entrance dual of `despawn-npc`. A campaign that fires
/// none and defers none emits nothing here, so it is byte-identical to pre-0.6.
///
/// **Not "one per deferred NPC".** `DW0197` guarantees every `deferred` NPC has a
/// spawn site, so the deferred set is contained in the call set — but the converse
/// was never true: `spawn-npc` on a NON-deferred NPC is legal content (it is how a
/// character comes back after a `despawn-npc`), and it compiled a
/// `function <ns>:spawn_npc_<id>` call against a function nobody emitted. The call
/// loaded fine and did nothing, so the character stayed gone. That is the island's
/// wave defect in a second emitter, and it is why the registration walk is now the
/// call walk ([`spawn_npc_sites`]) rather than a parallel property scan. The
/// deferred set is unioned in so ordering and output for existing campaigns are
/// untouched.
///
/// Each of the two summons is **independently** idempotent, so a re-fired
/// `spawn-npc` never doubles an entity — and so an entrance fired for an NPC
/// already standing at its mark is exactly the no-op it reads as. Body and hitbox
/// share the per-NPC id tag, so the guards discriminate on the body-only `dw_npc`
/// tag: the body is guarded by `[tag=dw_npc,tag=<id>]`, the hitbox by its negation
/// `[tag=<id>,tag=!dw_npc]` — a single `unless entity @e[tag=<id>]` guard on both
/// lines would let the body's own summon suppress the hitbox.
fn spawn_npc_fns(plan: &Plan) -> Vec<(String, String)> {
    let c = plan.campaign;
    let v03 = campaign_is_v03(plan);
    let sites = spawn_npc_sites(c);
    let mut out = Vec::new();
    for npc in &plan.npcs {
        if !npc_is_deferred(c, &npc.npc_id) && !sites.contains(npc.npc_id.as_str()) {
            continue;
        }
        let cmds = npc_summon_commands(c, plan, npc, v03);
        let body: Vec<String> = cmds
            .iter()
            .map(|cmd| {
                let guard = if cmd.starts_with("summon minecraft:interaction ") {
                    format!("@e[tag={},tag=!dw_npc]", npc.tag)
                } else {
                    format!("@e[tag=dw_npc,tag={}]", npc.tag)
                };
                format!("execute unless entity {guard} run {cmd}")
            })
            .collect();
        out.push((spawn_npc_fn(&npc.npc_id), lines(&body)));
    }
    out
}

/// The generated function name for a `move-npc` effect (content-derived key, so
/// the start-caller and the generator agree without threading an index).
fn movenpc_fn(npc: &str, to_anchor: &str, gate_key: &str) -> String {
    format!(
        "mv_{}_{}{gate_key}",
        plan::safe_local(npc),
        plan::safe_local(to_anchor)
    )
}

/// The generated function name for a `cutscene` effect, derived from its
/// **normalized shot list** — so the v0.4 single-shot spelling and a one-entry
/// `shots` list name the same function (byte-identical output).
///
/// Shape: `cs_<first anchor>_<first shot seconds>_<first shot waypoints>` — the
/// pre-multi-shot name — plus a `_<digest>` suffix over the whole shot list
/// (anchors, offsets, durations, subjects) whenever the cutscene is not a bare
/// single shot without `look_at`. The readable prefix keeps generated functions
/// greppable; the digest makes the key injective, so two cutscenes that share a
/// first waypoint but differ anywhere later can never collapse onto one function.
fn cutscene_fn(shots: &[delvewright_dsl::CameraShot]) -> String {
    let head = &shots[0];
    let first = head
        .path
        .first()
        .map(|w| plan::safe_local(w.anchor.as_str()))
        .or_else(|| {
            // A styled shot may have no explicit path: key on the style + the
            // subject's id instead, so the function name stays greppable.
            head.shot_style.map(|style| {
                let subj = match &head.subject {
                    Some(delvewright_dsl::CameraSubject::Anchor(s)) => s.anchor.as_str(),
                    Some(delvewright_dsl::CameraSubject::Npc(s)) => s.npc.as_str(),
                    Some(delvewright_dsl::CameraSubject::Actor(s)) => s.actor.as_str(),
                    None => "none",
                };
                format!(
                    "{}_{}",
                    plan::safe_local(style.token()),
                    plan::safe_local(subj)
                )
            })
        })
        .unwrap_or_else(|| "none".to_string());
    let base = format!("cs_{first}_{}_{}", head.resolved_seconds(), head.path.len());
    if shots.len() == 1 && head.look_at.is_none() && head.shot_style.is_none() {
        return base;
    }
    format!("{base}_{}", cutscene_digest(shots))
}

/// A short, stable content digest of a normalized cutscene shot list: the first
/// 8 hex chars of the sha256 of a canonical textual rendering. Deterministic
/// (fixed algorithm, fixed field order, no hash-order iteration, ADR-0006).
fn cutscene_digest(shots: &[delvewright_dsl::CameraShot]) -> String {
    let mut canon = String::new();
    for shot in shots {
        canon.push_str(&format!("s={};", shot.resolved_seconds()));
        for w in &shot.path {
            canon.push_str(&format!(
                "p={}@{},{},{};",
                w.anchor.as_str(),
                w.offset[0],
                w.offset[1],
                w.offset[2]
            ));
        }
        if let Some(t) = &shot.look_at {
            canon.push_str(&format!(
                "l={}@{},{},{};",
                t.anchor.as_str(),
                t.offset[0],
                t.offset[1],
                t.offset[2]
            ));
        }
        // Styled-shot fields (v0.6, spec-0015) — appended only when present, so
        // every pre-existing shot list keeps its digest byte-for-byte.
        if let Some(style) = shot.shot_style {
            canon.push_str(&format!("y={};", style.token()));
        }
        if let Some(sub) = &shot.subject {
            canon.push_str(&format!("u={};", sub.canon()));
        }
        if let Some(sub) = &shot.subject_b {
            canon.push_str(&format!("v={};", sub.canon()));
        }
        if let Some(d) = shot.dist {
            canon.push_str(&format!("d={d:?};"));
        }
        if let Some(g) = shot.degrees {
            canon.push_str(&format!("g={g:?};"));
        }
        if let Some(b) = shot.bearing {
            canon.push_str(&format!("b={b:?};"));
        }
        canon.push('|');
    }
    sha256_hex(canon.as_bytes())[..8].to_string()
}

/// The party flag gate for a list of flags: an ` if score #party dw.f_<flag>
/// matches 1` fragment per flag (leading space), or `""` for an ungated list.
///
/// spec-0018 replaced the pre-party spelling — an `@a[scores={dw.f_a=1..}]`
/// selector asking "does some player hold it" — with a single party read. The
/// selector form is now not merely redundant but *wrong*: nothing writes a flag
/// onto a player any more, so it would never match.
fn party_flag_gate(flags: &[delvewright_dsl::FlagId]) -> String {
    flags
        .iter()
        .map(|f| {
            format!(
                " if score {} {} matches 1",
                plan::PARTY,
                plan::flag_score(f.as_str())
            )
        })
        .collect()
}

/// Every quest effect in the campaign, flattened through `sequence` steps and
/// `move-actor` `on_arrive` (spec-0014) so nested lifecycle/cutscene/actor targets
/// are collected. Pre-0.6 campaigns have no nesting, so this equals the shallow
/// list (byte-identical).
///
/// The roots come from [`crate::plan::for_each_effect_root`] — the one enumeration
/// the gate scans and the staged-walk timeline also walk. What the emitter
/// generates functions for and what the proofs check are therefore the same set by
/// construction: a `sequence`/`cutscene`/`move-actor` in **any** root gets its
/// generated function, and none of the four walks can quietly grow a different
/// idea of where effects live.
fn all_campaign_effects(c: &delvewright_dsl::Campaign) -> Vec<&QuestEffect> {
    let mut out = Vec::new();
    crate::plan::for_each_effect_root(c, &mut |_site, effs| {
        for e in effs {
            push_effect_deep(e, &mut out);
        }
    });
    out
}

/// Push `e` and every transitively nested effect, descending through every nested
/// effect list ([`QuestEffect::nested_effect_lists`]: `sequence` steps,
/// `set-checkpoint` `on_respawn`, `begin-stealth` `on_caught`, `move-actor`
/// `on_arrive`). Completeness matters: e.g. a `sequence` nested in an `on_respawn`
/// must be reached here so `sequence_fns` generates its `seq_…` function — the
/// `emit_quest_effect` for the nested effect emits a `function` call to it.
fn push_effect_deep<'a>(e: &'a QuestEffect, out: &mut Vec<&'a QuestEffect>) {
    out.push(e);
    for list in e.nested_effect_lists() {
        for inner in list {
            push_effect_deep(inner, out);
        }
    }
}

/// The scoreboard-safe suffix shared by a move's driver functions/sentinels.
fn movenpc_bare(npc: &str, to_anchor: &str, gate_key: &str) -> String {
    movenpc_fn(npc, to_anchor, gate_key)
        .strip_prefix("mv_")
        .unwrap_or("move")
        .to_string()
}

/// `move-npc` functions (spec-0008 addendum): a **collision-safe walked path**,
/// not a single teleport. The path is planned by A* over the solved voxel grid
/// (`crate::nav`) at compile time; here we emit a self-scheduling per-tick driver
/// that teleports the NPC body + interaction hitbox (both carry the id tag) along
/// the waypoint polyline at the planned speed. Client interpolation smooths the
/// per-tick jumps into a walk (spike-verified). Deduped by content key; empty for
/// a campaign with no moves.
///
/// Each `tp` carries the **planned yaw** for that tick (`nav::yaws_along`, pitch
/// always 0 — a walk is level by construction). A rotation-less `tp` leaves the
/// body's yaw at whatever its summon or previous beat set, so an NPC routed the
/// other way slides backwards for the whole walk. Actor puppets carry their
/// tangent yaw, and `move-npc` holds to the same standard.
///
/// An `on_arrive` bundle (DSL v0.6, parity with `move-actor`) fires on the
/// driver's **final-waypoint tick** — exactly the arrival detection `ma_tick`
/// uses — via a generated `mv_arrive_<key>` function. A bare `move-npc` emits no
/// arrive hook and stays byte-identical to pre-0.6 output.
///
/// # Supersession — one body, one live driver
///
/// A driver's re-entry latch `#mrun_<bare>` is keyed per **(npc, to_anchor, gate)**:
/// it stops a walk from restarting *itself* and knows nothing about the body's other
/// walks. So a second `move-npc` fired at the same NPC while an earlier walk was
/// still running used to leave **two** drivers alive, both teleporting the same
/// entity every tick; the interleave garbled the path and whichever walk had more
/// remaining ticks wrote the final position — the body parked at the FIRST walk's
/// endpoint, not the last-fired one (root-caused live on the island, 2026-08-06: a
/// 408-tick beach→mouth walk overlapped by a 21-tick walk to checkpoint-1 left the
/// NPC 3.0 blocks off its cast-ledger cell, exactly on the harness's affordance
/// radius).
///
/// The contract is now **last fired wins**, carried by a per-NPC *walk generation*
/// score `#mgen_<npc>`: starting a walk bumps the generation and stamps it onto that
/// driver's `#mown_<bare>`; every driver tick first checks its own stamp is still the
/// current generation and, if not, drops its latch and returns without teleporting.
/// The superseded driver dies on its next scheduled tick, the new walk's tp sequence
/// runs alone from its own first waypoint (waypoints are precomputed from the walk's
/// declared start, so the new leg snaps to that first waypoint — the same instant
/// snap single-walk content already gets when a walk fires while its NPC stands
/// elsewhere). The staleness test is written as the positive `if own < gen`, never as
/// `unless own = gen`: with both scores unset — a driver invoked directly, as the
/// `v04_move` PackTest does — a score comparison is *false*, and the `unless`
/// spelling would read that as "stale" and cancel a walk nothing superseded.
///
/// A body with only one planned walk can never be superseded, so it carries none of
/// this: campaigns whose NPCs each walk at most once stay byte-identical (ADR-0006).
fn movenpc_fns(plan: &Plan, moves: &[crate::nav::MovePlan]) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    // How many drivers each body owns, in the planner's deterministic order. Two or
    // more ⇒ a later walk can catch an earlier one mid-flight ⇒ that body's drivers
    // carry the generation guard.
    let mut legs: BTreeMap<&str, usize> = BTreeMap::new();
    for m in moves {
        *legs.entry(m.npc.as_str()).or_insert(0) += 1;
    }
    for m in moves {
        let start_name = movenpc_fn(&m.npc, &m.to_anchor, &m.gate_key);
        let bare = movenpc_bare(&m.npc, &m.to_anchor, &m.gate_key);
        let safe = plan::safe_local(&m.npc);
        let total = m.ticks();
        let supersedable = legs.get(m.npc.as_str()).copied().unwrap_or(0) > 1;
        // `#mown_<bare> < #mgen_<npc>` ⇔ a later walk for this body has started.
        let stale = format!("score #mown_{bare} dw.sys < #mgen_{safe} dw.sys");
        // The on_arrive bundle for this (npc, to_anchor) — the first-seen effect,
        // matching the planner's dedup order (mirrors `actor_fns`).
        let on_arrive: &[QuestEffect] = all_campaign_effects(plan.campaign)
            .into_iter()
            .find_map(|e| match e {
                QuestEffect::MoveNpc {
                    npc,
                    to_anchor,
                    on_arrive,
                    ..
                } if npc.as_str() == m.npc && to_anchor.as_str() == m.to_anchor => {
                    Some(on_arrive.as_slice())
                }
                _ => None,
            })
            .unwrap_or(&[]);

        // start: guard re-entry, take the walk generation, reset the tick counter,
        // schedule the driver. The re-entry refusal is generation-aware: a latch left
        // armed by a driver this body has already superseded must not block the
        // re-fire of that same leg (it is itself a later walk, and wins).
        let mut start = Vec::new();
        if supersedable {
            start.push(format!(
                "execute if score #mrun_{bare} dw.sys matches 1 unless {stale} run return fail"
            ));
            start.push(format!("scoreboard players add #mgen_{safe} dw.sys 1"));
            start.push(format!(
                "scoreboard players operation #mown_{bare} dw.sys = #mgen_{safe} dw.sys"
            ));
        } else {
            start.push(format!(
                "execute if score #mrun_{bare} dw.sys matches 1 run return fail"
            ));
        }
        start.push(format!("scoreboard players set #mrun_{bare} dw.sys 1"));
        start.push(format!("scoreboard players set #mt_{bare} dw.sys 0"));
        start.push(format!("schedule function {ns}:mv_tick_{bare} 1t"));
        out.push((start_name, lines(&start)));

        // per-tick driver: tp both body + hitbox to waypoint[t], advance, and
        // reschedule until the path is walked; the final waypoint is the target.
        let mut tick: Vec<String> = Vec::new();
        if supersedable {
            // Superseded: drop the latch (so this leg can be fired again later) and
            // stop — no teleport, no arrive hook, no reschedule. The `schedule` this
            // driver queued before it lost the body is what brought us here; not
            // rescheduling is what ends it.
            tick.push(format!(
                "execute if {stale} run scoreboard players set #mrun_{bare} dw.sys 0"
            ));
            tick.push(format!("execute if {stale} run return fail"));
        }
        for (t, (w, y)) in m.waypoints.iter().zip(m.yaws.iter()).enumerate() {
            tick.push(format!(
                "execute if score #mt_{bare} dw.sys matches {t} run tp @e[tag=dw_npc_{safe}] {} {} {} {y} 0",
                fmt_f64(w[0]),
                fmt_f64(w[1]),
                fmt_f64(w[2])
            ));
        }
        if !on_arrive.is_empty() {
            tick.push(format!(
                "execute if score #mt_{bare} dw.sys matches {total} run function {ns}:mv_arrive_{bare}"
            ));
        }
        tick.push(format!("scoreboard players add #mt_{bare} dw.sys 1"));
        tick.push(format!(
            "execute if score #mt_{bare} dw.sys matches {}.. run scoreboard players set #mrun_{bare} dw.sys 0",
            total + 1
        ));
        tick.push(format!(
            "execute unless score #mt_{bare} dw.sys matches {}.. run schedule function {ns}:mv_tick_{bare} 1t",
            total + 1
        ));
        out.push((format!("mv_tick_{bare}"), lines(&tick)));

        if !on_arrive.is_empty() {
            // Server command source: the driver that calls this reached us from
            // `schedule`, so there is no `@s` (see `Audience`).
            let arrive = emit_effect_bundle(plan, on_arrive, Audience::Scheduled);
            out.push((format!("mv_arrive_{bare}"), lines(&arrive)));
        }
    }
    out
}

/// The spawn yaw for an actor from its `facing` (default south = 0).
fn actor_facing_yaw(a: &delvewright_dsl::Actor) -> i32 {
    a.facing.map(|f| facing_yaw(Some(f.token()))).unwrap_or(0)
}

/// The `/summon` command for an actor's caged puppet (spec-0014). NoAI/Silent/
/// no-loot (`DeathLootTable` empty), tag `dw_actor` + `dw_actor_<id>` + a
/// puppet-only `dw_pup_<id>` marker (so `unleash`/`move` target the puppet without
/// touching a real-AI twin). `Invulnerable` unless `vulnerable`; a vulnerable puppet
/// stays knockback-immune (`knockback_resistance` 1.0) — the tower-defense creep. A
/// `skin` re-dresses it as a `minecraft:mannequin`, exactly as a stage-2 NPC.
fn actor_puppet_summon(ns: &str, a: &delvewright_dsl::Actor, pos: [i32; 3], yaw: i32) -> String {
    let safe = plan::safe_local(a.id.as_str());
    // v0.9: a declared quest-item drop points the field the puppet
    // has always carried at a table the compiler emits. `unleash` and
    // `despawn-actor` strip it again ([`strip_drops_line`]) — only a player's
    // kill yields it.
    let loot = death_loot_table(
        ns,
        has_item_drop(&a.drops).then(|| drop_loot_path("actor", a.id.as_str())),
    );
    let p = ent_xyz(pos);
    let tags = format!("Tags:[\"dw_actor\",\"dw_actor_{safe}\",\"dw_pup_{safe}\"]");
    if let Some(skin) = &a.skin {
        let desc = a
            .name
            .as_deref()
            .unwrap_or_else(|| a.id.as_str().rsplit('/').next().unwrap_or("actor"));
        format!(
            "summon minecraft:mannequin {} {} {} {{profile:{{texture:\"delvewright:npc/{}\",model:\"{}\"}},immovable:1b,pose:\"standing\",Invulnerable:1b,Silent:1b,Rotation:[{yaw}f,0f],description:{},{tags}}}",
            p[0],
            p[1],
            p[2],
            skin.texture_id,
            skin.model.token(),
            snbt_text_component(desc)
        )
    } else {
        let inv = if a.vulnerable { 0 } else { 1 };
        let name = a
            .name
            .as_deref()
            .map(|n| format!(",CustomName:{},CustomNameVisible:1b", snbt_component(n)))
            .unwrap_or_default();
        // Compiler-owned knockback-immunity first (a `vulnerable` puppet is a
        // damageable creep, never a shovable one), then whatever the author
        // declared — so a puppet with no `attributes` renders exactly the
        // pre-`attributes` string and every earlier campaign stays byte-identical.
        let mut entries: Vec<String> = Vec::new();
        if a.vulnerable {
            entries.push("{id:\"minecraft:knockback_resistance\",base:1.0}".to_string());
        }
        entries.extend(attribute_entries(a.attributes.as_ref()));
        let attrs = wrap_attribute_entries(entries);
        let pose = mannequin_pose_nbt(&a.entity);
        // spec-0021: actor gear rides on BOTH the puppet and the twin, so the
        // dormant elite the party circles is visibly the thing that stands up.
        let equip = actor_equipment(a)
            .map(|e| format!(",{e}"))
            .unwrap_or_default();
        format!(
            "summon {} {} {} {} {{NoAI:1b,Silent:1b,PersistenceRequired:1b,NoGravity:1b{pose},Invulnerable:{inv}b,DeathLootTable:\"{loot}\",Rotation:[{yaw}f,0f],{tags}{name}{attrs}{equip}}}",
            a.entity, p[0], p[1], p[2]
        )
    }
}

/// The `pose` NBT field a `minecraft:mannequin` needs, or `""` for any other
/// entity — spliced into every summon whose entity id is author-supplied.
///
/// A mannequin summoned without an explicit `pose` serializes it as `DYING`, which
/// the server then fails to encode at save time (`Failed to encode value 'DYING'`
/// in a PackTest world's teardown log) and which is simply wrong data for a
/// standing figure. The skinned NPC/actor paths hardcode `minecraft:mannequin` and
/// have always emitted it; the paths that take the entity id **from content**
/// (`npc.base_entity`, `actor.entity`, and the `unleash` twin, which has no skin
/// branch at all) did not — so an author who spelled `minecraft:mannequin` by hand
/// got the broken pose. Valid 1.21.11 mannequin poses: standing, crouching,
/// swimming, fall_flying, sleeping (spec-0009 template).
fn mannequin_pose_nbt(entity: &str) -> &'static str {
    let id = entity.strip_prefix("minecraft:").unwrap_or(entity);
    if id == "mannequin" {
        ",pose:\"standing\""
    } else {
        ""
    }
}

/// The state vanilla's `finalizeSpawn` would have given this entity, spliced into
/// every summon the compiler writes with an NBT compound — or `""` for a species
/// that needs none.
///
/// **The trap this closes** (round-8 island QA, proven on a live pinned 1.21.11
/// server). `/summon <entity> <pos>` calls the mob's `finalizeSpawn`;
/// `/summon <entity> <pos> <nbt>` — *any* NBT compound, even `{}` — does **not**.
/// The compiler always passes NBT (tags are how every entity it owns is addressed),
/// so every mob it summons is spawned un-finalized. For most species that is
/// invisible. For `minecraft:warden` it is fatal: `finalizeSpawn` is the only place
/// the `minecraft:dig_cooldown` brain memory is seeded, and a warden whose brain
/// lacks it enters the DIG activity on its first AI tick, plays the burrow
/// animation, and despawns about five seconds later. That is exactly what the
/// owner saw — strike the sleeping giant, watch him turn into a warden, watch the
/// warden immediately dig itself back into the ground.
///
/// Live A/B on the pinned server:
/// `summon minecraft:warden <pos>` → `Brain{memories:{"minecraft:dig_cooldown":{value:{},ttl:1200L}}}`;
/// `summon minecraft:warden <pos> {}` → `Brain{memories:{}}`, gone in ~5s.
///
/// The fix is to write the same data vanilla would have written — the entity's own
/// documented, codec-backed NBT, not a workaround for a missing primitive. The
/// warden refreshes the cooldown itself every tick it is awake and doing anything,
/// so seeding vanilla's own 1200-tick value is enough to keep an unleashed boss in
/// the world for as long as the campaign wants it (verified: still present and
/// roaming past 80 s, `ttl` held at 1199 by the warden's own AI).
///
/// Only species whose un-finalized state is actually *wrong* appear here, so every
/// campaign without one stays byte-identical.
fn spawn_finalize_nbt(entity: &str) -> &'static str {
    match entity.strip_prefix("minecraft:").unwrap_or(entity) {
        // `Warden.finalizeSpawn` → `setMemoryWithExpiry(DIG_COOLDOWN, Unit, 1200)`.
        "warden" => ",Brain:{memories:{\"minecraft:dig_cooldown\":{value:{},ttl:1200L}}}",
        _ => "",
    }
}

/// The `/summon` command for an actor's real-AI twin (spec-0014 `unleash`): the
/// real `entity` with AI enabled, same name and body tag (`dw_actor` +
/// `dw_actor_<id>`), but **no** `dw_pup_<id>` marker — so killing the puppet by
/// its marker leaves the twin fighting.
///
/// `at` is the position argument: `~ ~ ~` for the unleash (run `execute at` the
/// puppet, so the twin stands up exactly where the puppet knelt), or the actor's
/// absolute origin cell for the bonfire's undefeated re-seat, which has no puppet
/// left to stand at. One string, so the two paths can never drift into two
/// different bodies.
///
/// The twin is the compiler's only *free-AI* summon, so it is where
/// [`spawn_finalize_nbt`] matters: a caged puppet is `NoAI`, and a `NoAI` mob never
/// runs `customServerAiStep`, which is why the island's herdsman warden could stand
/// in the meadow indefinitely while the unleashed one burrowed away.
fn actor_twin_summon(ns: &str, a: &delvewright_dsl::Actor, at: &str) -> String {
    let safe = plan::safe_local(a.id.as_str());
    let loot = death_loot_table(
        ns,
        has_item_drop(&a.drops).then(|| drop_loot_path("actor", a.id.as_str())),
    );
    let name = a
        .name
        .as_deref()
        .map(|n| format!(",CustomName:{},CustomNameVisible:1b", snbt_component(n)))
        .unwrap_or_default();
    let pose = mannequin_pose_nbt(&a.entity);
    let finalize = spawn_finalize_nbt(&a.entity);
    // The twin inherits the puppet's gear: unleashing swaps the body, not the
    // costume. Drop chances stay 0 — killing the elite must never drop its kit.
    let equip = actor_equipment(a)
        .map(|e| format!(",{e}"))
        .unwrap_or_default();
    // The twin inherits the puppet's tuning too: the whole point of an elite's
    // `attributes` is the body that actually fights, and unleashing replaces the
    // body. Knockback-immunity deliberately does NOT ride along — that is the
    // caged creep's property, not the freed elite's.
    let attrs = attributes_snbt(a.attributes.as_ref());
    format!(
        "summon {} {at} {{PersistenceRequired:1b{pose},DeathLootTable:\"{loot}\",Tags:[\"dw_actor\",\"dw_actor_{safe}\"]{name}{finalize}{attrs}{equip}}}",
        a.entity
    )
}

/// The `equipment`/`drop_chances` SNBT fragment for an actor (no leading comma),
/// or `None` when the actor declares no gear.
///
/// Deliberately NOT the wave path's [`wave_equipment`]: that function falls back
/// to the armed-mob default table, which would silently arm every actor whose
/// entity happens to be a vindicator or skeleton and break byte-identity for
/// every campaign authored before this field existed. An actor is a directed
/// set piece — it wears exactly what the author declared, and nothing when they
/// declared nothing.
fn actor_equipment(a: &delvewright_dsl::Actor) -> Option<String> {
    let eq = a.equipment.as_ref()?;
    let declared = declared_drop_slots(&a.drops);
    let mut items: Vec<String> = Vec::new();
    let mut chances: Vec<String> = Vec::new();
    // Fixed emission order, matching the wave path (ADR-0006 determinism).
    let slots: [(&str, Option<&EquipItem>); 6] = [
        ("mainhand", eq.main_hand.as_ref()),
        ("offhand", eq.off_hand.as_ref()),
        ("head", eq.head.as_ref()),
        ("chest", eq.chest.as_ref()),
        ("legs", eq.legs.as_ref()),
        ("feet", eq.feet.as_ref()),
    ];
    for (slot, piece) in slots {
        if let Some(p) = piece {
            let comps = enchantment_components(p);
            items.push(format!("{slot}:{{id:\"{}\",count:1{comps}}}", p.item()));
            chances.push(format!("{slot}:{}", drop_chance_for(slot, &declared)));
        }
    }
    if items.is_empty() {
        return None;
    }
    Some(format!(
        "equipment:{{{}}},drop_chances:{{{}}}",
        items.join(","),
        chances.join(",")
    ))
}

/// Command storage holding the UUID of the player who most recently struck (or
/// used) a click trigger, for the duration of that trigger's own effect bundle.
///
/// Vanilla writes the clicking player's UUID into the `minecraft:interaction`
/// entity's `attack` / `interaction` record; `data modify … set from entity` is the
/// intended primitive for moving it, and command storage is the intended place to
/// park it. Written at the top of `trig_<id>` and removed at the bottom, so it is
/// live exactly while the trigger's synchronous effects run and can never go stale.
const STRIKER_STORAGE: &str = "dw:strike";

/// The storage path under [`STRIKER_STORAGE`] holding the striking player's UUID.
const STRIKER_PATH: &str = "player";

/// Whether `t` is a click trigger (`strike` / `strike-npc` / `use`) — the forms
/// whose interaction entity records *which player* acted.
fn trigger_is_click(t: &delvewright_dsl::EnvTrigger) -> bool {
    use delvewright_dsl::TriggerOn;
    matches!(
        t.on,
        TriggerOn::Strike | TriggerOn::Use | TriggerOn::StrikeNpc { .. }
    )
}

/// The NBT record a click trigger reads off its interaction entity: a left-click
/// writes `attack`, a right-click writes `interaction`.
fn trigger_record(t: &delvewright_dsl::EnvTrigger) -> &'static str {
    match t.on {
        delvewright_dsl::TriggerOn::Use => "interaction",
        _ => "attack",
    }
}

/// Whether this trigger's effect tree reaches an `unleash-actor` — the only reason
/// to capture the striker at all. Campaigns that never unleash from a click stay
/// byte-identical.
fn trigger_unleashes(t: &delvewright_dsl::EnvTrigger) -> bool {
    let mut all = Vec::new();
    for e in &t.effects {
        push_effect_deep(e, &mut all);
    }
    all.iter()
        .any(|e| matches!(e, QuestEffect::UnleashActor { .. }))
}

/// Whether any click trigger in the campaign captures a striker — i.e. whether
/// [`STRIKER_STORAGE`] can ever hold a value. Gates the aggro-lock lines in
/// `unleash_<id>` so an unrelated campaign's unleash functions are unchanged.
fn campaign_captures_striker(c: &delvewright_dsl::Campaign) -> bool {
    c.quests
        .content
        .triggers
        .iter()
        .any(|t| trigger_is_click(t) && trigger_unleashes(t))
}

/// Warden anger at which `AngerLevel` is `ANGRY` and the mob commits to a target.
/// Vanilla's own maximum (`AngerManagement`), so the lock is immediate and total.
const WARDEN_MAX_ANGER: i32 = 150;

/// The lines that lock an unleashed twin's aggression onto the player who struck
/// the trigger (owner directive, round 8): a hostile that a player *provoked* must
/// come for that player, not wander off looking for someone.
///
/// **Only species with a proven vanilla primitive get one.** `minecraft:warden`
/// persists its target list as `anger.suspects` (`AngerManagement`), a codec-backed
/// field vanilla itself round-trips, and seeding it works end to end on a live
/// pinned 1.21.11 server: the warden left its spawn cell, closed on the seeded
/// player's position and killed that player.
///
/// The `NeutralMob` pair (`AngerTime` / `AngryAt`) looks like the same primitive for
/// endermen, piglins, wolves and friends, and was tried — but on 1.21.11 neither
/// field reads back after a tick, for any of the species tested, with a real online
/// player's UUID or a synthetic one. Whatever the mechanism (the codec dropping
/// defaults, or `updatePersistentAnger` clearing the target it just resolved), the
/// data does not survive, so the compiler does not pretend it does: every non-warden
/// species is left to vanilla's own nearest-player acquisition, and that limit is
/// documented rather than papered over (CLAUDE.md: no hacks at any layer — if the
/// primitive is not really there, the feature does not get faked downstream).
///
/// Guarded on the striker storage actually holding a UUID, so an `unleash-actor`
/// fired from anywhere other than a click trigger's own bundle changes nothing.
fn aggro_lock_lines(entity: &str, safe: &str) -> Vec<String> {
    let id = entity.strip_prefix("minecraft:").unwrap_or(entity);
    if id != "warden" {
        return Vec::new();
    }
    // The twin is the only entity left wearing the body tag: `unleash_<id>` kills
    // the puppet on the line before these run.
    let target = format!("@e[tag=dw_actor_{safe},limit=1]");
    let guard = format!("execute if data storage {STRIKER_STORAGE} {STRIKER_PATH} run");
    // `AngerManagement`: a suspect list of `{uuid, anger}`. Seed one suspect at
    // vanilla's maximum anger, then overwrite its placeholder UUID from storage —
    // `data modify … set from storage` cannot create the list element, so the
    // element is written first and patched second.
    vec![
        format!(
            "{guard} data modify entity {target} anger.suspects set value [{{anger:{WARDEN_MAX_ANGER},uuid:[I;0,0,0,0]}}]"
        ),
        format!(
            "{guard} data modify entity {target} anger.suspects[0].uuid set from storage {STRIKER_STORAGE} {STRIKER_PATH}"
        ),
    ]
}

/// The generated start-function name for a `move-actor` (content key).
fn moveactor_fn(actor: &str, to_anchor: &str, gate_key: &str) -> String {
    format!(
        "ma_{}_{}{gate_key}",
        plan::safe_local(actor),
        plan::safe_local(to_anchor)
    )
}

/// The scoreboard-safe suffix shared by a move-actor's driver functions/sentinels.
fn moveactor_bare(actor: &str, to_anchor: &str, gate_key: &str) -> String {
    moveactor_fn(actor, to_anchor, gate_key)
        .strip_prefix("ma_")
        .unwrap_or("move")
        .to_string()
}

/// A deterministic content key for a `sequence` (spec-0014) — FNV-1a over the steps'
/// stable `Debug` rendering, so identical timelines share one function and different
/// ones do not collide. No wall-clock / hash-order input (ADR-0006).
fn sequence_key(steps: &[delvewright_dsl::SequenceStep]) -> String {
    let s = format!("{steps:?}");
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// The generated start-function name for a `sequence` effect (content key).
fn sequence_fn(steps: &[delvewright_dsl::SequenceStep]) -> String {
    format!("seq_{}", sequence_key(steps))
}

/// The content key naming a spec-0022 trap-payload verb's generated function.
/// FNV-1a over the effect's stable `Debug` rendering — the same scheme
/// [`sequence_key`] uses, so two identical `volley`s share one function and a
/// campaign's output is a pure function of its content (ADR-0006).
fn payload_verb_key(eff: &QuestEffect) -> String {
    let s = format!("{eff:?}");
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// The generated function name for a `volley` effect.
fn volley_fn(eff: &QuestEffect) -> String {
    format!("volley_{}", payload_verb_key(eff))
}

/// The generated function name for a `collapse` effect.
fn collapse_fn(eff: &QuestEffect) -> String {
    format!("collapse_{}", payload_verb_key(eff))
}

/// The generated function name for a `teleport` effect (DSL v0.10, spec-0031).
///
/// A named function rather than an inline line, for the same reason `volley` and
/// `collapse` have one: **the body is compiler-PROVEN geometry** — a box resolved
/// through `Plan::zone_box` and a destination resolved to a literal cell — and a
/// body that only ever exists spliced into a `seq_<hash>` beside four other
/// effects is a body no runtime test can call. The generated PackTest calls
/// exactly this function, so the runtime proof of totality binds to the emission
/// rather than to a command the test re-typed for itself.
fn teleport_fn(eff: &QuestEffect) -> String {
    format!("teleport_{}", payload_verb_key(eff))
}

/// The one emitted line of a `teleport`: move everything in the volume.
///
/// `None` when either anchor is unresolved — `check_effect_anchors` (`DW0360`)
/// owns that failure, and an invalid selector emitted here would report it as
/// something else.
fn teleport_command(plan: &Plan, eff: &QuestEffect) -> Option<String> {
    let (from, to) = eff.teleport()?;
    let (lo, hi) = plan.zone_box(from)?;
    let d = ent_xyz(anchor_point_any(plan, to.as_str())?);
    Some(format!(
        "tp @e[{}] {} {} {}",
        entity_box_selector(lo, hi),
        d[0],
        d[1],
        d[2]
    ))
}

/// One function per distinct `teleport` (deduped by content key). Empty for a
/// campaign that declares none, so pre-0.10 output is byte-identical.
fn teleport_fns(plan: &Plan) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for eff in all_campaign_effects(plan.campaign) {
        if eff.teleport().is_none() {
            continue;
        }
        let name = teleport_fn(eff);
        if !seen.insert(name.clone()) {
            continue;
        }
        let Some(cmd) = teleport_command(plan, eff) else {
            continue;
        };
        out.push((name, lines(&[cmd])));
    }
    out
}

/// Actor staging functions (spec-0014): a `spawn_actor_<id>` (idempotent summon) and
/// `unleash_<id>` (puppet → real-AI twin) per declared actor, plus a per-tick
/// teleport driver (with tangent yaw and an `on_arrive` bundle) per planned
/// `move-actor`. Empty for a campaign with no actors (pre-0.6 byte-identical).
///
/// # Supersession — one puppet, one live leg driver
///
/// A `move-actor` driver carries the identical defect `move-npc` had: its
/// re-entry latch `#arun_<bare>` is keyed per **(actor, to_anchor, gate)**, so it only
/// ever stopped a leg from restarting *itself*. Two overlapping legs on ONE puppet left
/// two live drivers both `tp`-ing the same body every tick; they fought, and the longer
/// leg — outliving the shorter — wrote the final position, parking the puppet at the
/// FIRST leg's endpoint permanently.
///
/// The contract is the same one `movenpc_fns` documents at length: **last fired wins**,
/// carried by a per-puppet leg generation `#agen_<actor>` stamped onto each driver's
/// `#aown_<bare>`. A driver whose stamp is behind the generation drops its latch and
/// returns without teleporting, arriving, or rescheduling — so it dies on its next
/// scheduled tick. The staleness test is the positive `if own < gen` for the same
/// reason: with both scores unset (a driver invoked directly, as the `v06_move_actor`
/// and `v06_arrive_handoff` PackTests do) a score comparison is *false*, so the
/// unfired-generation case reads as "not stale" and the leg runs.
///
/// A puppet with only one planned leg can never be superseded and carries none of this,
/// so pre-existing single-leg campaigns stay byte-identical (ADR-0006) — pinned
/// verbatim by `move_supersede.rs`'s `GOLDEN_ONE_LEG`.
fn actor_fns(plan: &Plan, actor_moves: &[crate::nav::ActorMovePlan]) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for a in &plan.campaign.quests.content.actors {
        let safe = plan::safe_local(a.id.as_str());
        let Some(pos) = anchor_point_any(plan, a.anchor.as_str()) else {
            continue; // resolution guaranteed by check_actor_placement (DW0325)
        };
        let yaw = actor_facing_yaw(a);
        out.push((
            format!("spawn_actor_{safe}"),
            lines(&[format!(
                "execute unless entity @e[tag=dw_actor_{safe}] run {}",
                actor_puppet_summon(ns, a, pos, yaw)
            )]),
        ));
        let mut unleash = vec![format!(
            "execute at @e[tag=dw_pup_{safe},limit=1] run {}",
            actor_twin_summon(ns, a, "~ ~ ~")
        )];
        // The unleash removes the cage by killing it, and vanilla `/kill` is an
        // ordinary death: a puppet carrying a declared drop would shed it the
        // moment the elite stood up. Strip first — the twin standing beside it
        // is the body that owes the player a prize.
        if !a.drops.is_empty() {
            unleash.push(strip_drops_line(&format!("dw_pup_{safe}")));
        }
        unleash.push(format!("kill @e[tag=dw_pup_{safe}]"));
        if campaign_captures_striker(plan.campaign) {
            unleash.extend(aggro_lock_lines(&a.entity, &safe));
        }
        out.push((format!("unleash_{safe}"), lines(&unleash)));
        // spec-0016 §1: the UNDEFEATED re-seat. A rest
        // (and a death-respawn at the same fire) deletes the elite the party is
        // still fighting and stands a FRESH body on its origin anchor: full
        // health, no accumulated chip damage, and — the reported regression — back
        // where it belongs instead of wherever the chase left it.
        //
        // Deliberately not `unleash_<id>`: there is no puppet to stand up from,
        // and re-caging one would be worse than doing nothing, because an
        // `unleash-actor` beat fires from a one-shot trigger the engine never
        // re-arms — a re-caged elite would be dormant, `Invulnerable` scenery for
        // the rest of the delve. It comes back as what it was: a freed body on its
        // own ground.
        //
        // The striker aggro lock is deliberately NOT re-applied. Nobody has
        // provoked this body yet, and spec-0016 §1's stationed rule is that
        // nothing a rest puts back may pursue across the map: it stands on its
        // anchor under vanilla-local AI, inside the `follow_range` `DW0478`
        // measured the bonfire against.
        //
        // Emitted only for an actor the campaign unleashes AND a campaign with a
        // bonfire ([`Plan::reseat_actors`]) → byte-identical everywhere else.
        if plan.reseat_actors().iter().any(|r| r.id == a.id) {
            let p = ent_xyz(pos);
            out.push((
                format!("actor_restand_{safe}"),
                lines(&[
                    format!("kill @e[tag=dw_actor_{safe}]"),
                    actor_twin_summon(ns, a, &format!("{} {} {}", p[0], p[1], p[2])),
                ]),
            ));
        }
    }
    // move-actor per-tick drivers.
    //
    // How many legs each puppet owns, in the planner's deterministic order. Two or
    // more ⇒ a later leg can catch an earlier one mid-flight ⇒ that puppet's drivers
    // carry the generation guard (see the supersession section of `movenpc_fns`).
    let mut legs: BTreeMap<&str, usize> = BTreeMap::new();
    for m in actor_moves {
        *legs.entry(m.actor.as_str()).or_insert(0) += 1;
    }
    for m in actor_moves {
        let safe = plan::safe_local(&m.actor);
        let bare = moveactor_bare(&m.actor, &m.to_anchor, &m.gate_key);
        let total = m.ticks();
        let supersedable = legs.get(m.actor.as_str()).copied().unwrap_or(0) > 1;
        // `#aown_<bare> < #agen_<actor>` ⇔ a later leg for this puppet has started.
        let stale = format!("score #aown_{bare} dw.sys < #agen_{safe} dw.sys");
        // The on_arrive bundle for this (actor, to_anchor) — the first-seen effect,
        // matching the planner's dedup order.
        let on_arrive: &[QuestEffect] = all_campaign_effects(plan.campaign)
            .into_iter()
            .find_map(|e| match e {
                QuestEffect::MoveActor {
                    actor,
                    to_anchor,
                    on_arrive,
                    ..
                } if actor.as_str() == m.actor && to_anchor.as_str() == m.to_anchor => {
                    Some(on_arrive.as_slice())
                }
                _ => None,
            })
            .unwrap_or(&[]);

        // start: guard re-entry, take the walk generation, reset the tick counter,
        // schedule the driver. The re-entry refusal is generation-aware: a latch left
        // armed by a driver this puppet has already superseded must not block the
        // re-fire of that same leg (it is itself a later leg, and wins).
        let mut start = Vec::new();
        if supersedable {
            start.push(format!(
                "execute if score #arun_{bare} dw.sys matches 1 unless {stale} run return fail"
            ));
            start.push(format!("scoreboard players add #agen_{safe} dw.sys 1"));
            start.push(format!(
                "scoreboard players operation #aown_{bare} dw.sys = #agen_{safe} dw.sys"
            ));
        } else {
            start.push(format!(
                "execute if score #arun_{bare} dw.sys matches 1 run return fail"
            ));
        }
        start.push(format!("scoreboard players set #arun_{bare} dw.sys 1"));
        start.push(format!("scoreboard players set #at_{bare} dw.sys 0"));
        start.push(format!("schedule function {ns}:ma_tick_{bare} 1t"));
        out.push((
            moveactor_fn(&m.actor, &m.to_anchor, &m.gate_key),
            lines(&start),
        ));

        let mut tick: Vec<String> = Vec::new();
        if supersedable {
            // Superseded: drop the latch (so this leg can be fired again later) and
            // stop — no teleport, no arrive hook, no reschedule. The `schedule` this
            // driver queued before it lost the puppet is what brought us here; not
            // rescheduling is what ends it.
            tick.push(format!(
                "execute if {stale} run scoreboard players set #arun_{bare} dw.sys 0"
            ));
            tick.push(format!("execute if {stale} run return fail"));
        }
        for (t, (w, y)) in m.waypoints.iter().zip(m.yaws.iter()).enumerate() {
            tick.push(format!(
                "execute if score #at_{bare} dw.sys matches {t} run tp @e[tag=dw_pup_{safe}] {} {} {} {y} 0",
                fmt_f64(w[0]),
                fmt_f64(w[1]),
                fmt_f64(w[2])
            ));
        }
        if !on_arrive.is_empty() {
            tick.push(format!(
                "execute if score #at_{bare} dw.sys matches {total} run function {ns}:ma_arrive_{bare}"
            ));
        }
        tick.push(format!("scoreboard players add #at_{bare} dw.sys 1"));
        tick.push(format!(
            "execute if score #at_{bare} dw.sys matches {}.. run scoreboard players set #arun_{bare} dw.sys 0",
            total + 1
        ));
        tick.push(format!(
            "execute unless score #at_{bare} dw.sys matches {}.. run schedule function {ns}:ma_tick_{bare} 1t",
            total + 1
        ));
        out.push((format!("ma_tick_{bare}"), lines(&tick)));

        if !on_arrive.is_empty() {
            // Server command source (see `Audience`): `ma_tick_<bare>` runs from
            // the scheduler, so `@s` is unbound in everything it calls.
            let arrive = emit_effect_bundle(plan, on_arrive, Audience::Scheduled);
            out.push((format!("ma_arrive_{bare}"), lines(&arrive)));
        }
    }
    out
}

/// `sequence` timeline functions (spec-0014): one start function that schedules each
/// step's effect-group at its exact `at_ticks` offset, plus one function per step.
/// Deduped by content key. Empty for a campaign with no sequences (byte-identical).
fn sequence_fns(plan: &Plan) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for eff in all_campaign_effects(plan.campaign) {
        let QuestEffect::Sequence { steps } = eff else {
            continue;
        };
        let key = sequence_key(steps);
        if !seen.insert(key.clone()) {
            continue;
        }
        let base = format!("seq_{key}");
        let mut start: Vec<String> = Vec::new();
        for (i, step) in steps.iter().enumerate() {
            if step.at_ticks == 0 {
                start.push(format!("function {ns}:{base}_{i}"));
            } else {
                start.push(format!(
                    "schedule function {ns}:{base}_{i} {}t",
                    step.at_ticks
                ));
            }
        }
        out.push((base.clone(), lines(&start)));
        for (i, step) in steps.iter().enumerate() {
            // EVERY step is emitted server-source-safe, not just the scheduled
            // ones: a timeline whose `at_ticks: 0` step behaved differently from
            // its `at_ticks: 20` step would be a trap, and the start function is
            // itself reachable from a scheduled bundle (a `sequence` nested in an
            // `on_arrive`). Uniformity is what makes `seq_<key>` a *global*
            // effect everywhere (see `effect_is_player_scoped`): its per-player
            // beats address the party, never one acting player.
            let b = emit_effect_bundle(plan, &step.effects, Audience::Scheduled);
            out.push((format!("{base}_{i}"), lines(&b)));
        }
    }
    out
}

/// The entity tag every player carries for the duration of a cutscene.
///
/// **Staging invariant — a cutscene is pure observation.** While a player is in
/// the cutscene state, campaign machinery must not require anything of them and
/// must not punish them: the stealth judge is suspended for that player (grace
/// neither accrues nor expires, `on_caught` cannot fire) and `damage-players`
/// skips them. Any future verb that *demands* input or *deals harm* joins this
/// list — the player is watching, not playing.
///
/// Added by the cutscene `start` alongside `gamemode spectator`, removed by the
/// `end`/restore, so the state has exactly the cinematic's lifetime.
const CUTSCENE_TAG: &str = "dw_cutscene";

/// Datapack predicate id (under the campaign namespace) matching a player whose
/// sneak key is HELD this tick — the vanilla `minecraft:player` `input`
/// sub-predicate (1.21.2+), which reads the client's raw input packet and so
/// works in every gamemode, spectator included. Sole consumer: the cutscene
/// `spectate` bounce, which must not re-attach a player whose held sneak would
/// immediately dismount them again (the round-6 camera-flicker root cause).
/// Emitted only for a campaign with at least one cutscene, so everything else
/// stays byte-identical.
const SNEAK_HELD_PREDICATE: &str = "sneak_held";

/// The `<ns>:sneak_held` predicate body (see [`SNEAK_HELD_PREDICATE`]).
fn sneak_held_predicate() -> Value {
    json!({
        "condition": "minecraft:entity_properties",
        "entity": "this",
        "predicate": {
            "type_specific": {
                "type": "minecraft:player",
                "input": { "sneak": true }
            }
        }
    })
}

/// Does the campaign play at least one real cutscene (a non-empty shot list)?
/// Gates the [`SNEAK_HELD_PREDICATE`] emission.
fn campaign_has_cutscene(campaign: &delvewright_dsl::Campaign) -> bool {
    crate::camera::cutscene_units(campaign)
        .iter()
        .any(|(eff, _)| eff.cutscene_shots().is_some_and(|s| !s.is_empty()))
}

/// The campaign-wide count of cutscenes currently playing. A refcount rather than
/// a flag because nothing forbids two cutscenes overlapping (each `cs_<bare>` only
/// guards re-entry into *itself*). Never initialized, so an `unless … matches 1..`
/// test reads correctly before the first cutscene ever runs.
const CS_LIVE: &str = "#cs_live";

/// The `tick` clause that repairs a player stranded by a mid-cutscene disconnect.
///
/// The cutscene bracket is entirely `@a`-scoped: `cs_end_<bare>` restores gamemode,
/// teleports, and removes [`CUTSCENE_TAG`] from *the players who are online when it
/// ends*. A player who dropped during the shot is not among them, so they come back
/// tagged, in spectator, with the marker they would have been teleported to already
/// killed. `join_place` is no help — it is gated on `dw_joined`, which survives a
/// relog exactly like the cutscene tag does.
///
/// The stuck state is decidable without any per-player bookkeeping: *tagged
/// `dw_cutscene` while no cutscene is playing*. Empty for a cutscene-less campaign,
/// so those packs stay byte-identical.
fn cutscene_repair_tick(plan: &Plan) -> Vec<String> {
    if !campaign_has_cutscene(plan.campaign) {
        return Vec::new();
    }
    let ns = &plan.namespace;
    vec![format!(
        "execute unless score {CS_LIVE} dw.sys matches 1.. as @a[tag={CUTSCENE_TAG}] run function {ns}:cs_repair"
    )]
}

/// The repair itself, per stranded player (`@s`): back to adventure, untagged, and
/// returned to the party's live checkpoint.
///
/// The destination is `storage dw:cp pos` — the checkpoint mirror, seeded to the
/// entry point at setup and rewritten by every `set-checkpoint` — because the
/// cutscene's own saved position (a single `dw_csmark_<bare>` marker) is destroyed
/// by `cs_end_<bare>` before this can ever run. It is a macro teleport for the same
/// reason the boundary return is one: the mirror is an `[x, y, z]` list, not
/// tp-shaped arguments. Emitted only when the campaign resolves an entry anchor,
/// which is what seeds the mirror in the first place.
fn cutscene_repair_fns(plan: &Plan) -> Vec<(String, String)> {
    if !campaign_has_cutscene(plan.campaign) {
        return Vec::new();
    }
    let ns = &plan.namespace;
    let mut repair = vec![
        "gamemode adventure @s".to_string(),
        format!("tag @s remove {CUTSCENE_TAG}"),
    ];
    let mut out = Vec::new();
    if campaign_spawn(plan).is_some() {
        for (i, axis) in ["x", "y", "z"].iter().enumerate() {
            repair.push(format!(
                "data modify storage dw:cs at.{axis} set from storage dw:cp pos[{i}]"
            ));
        }
        repair.push(format!("function {ns}:cs_repair_tp with storage dw:cs at"));
        out.push((
            "cs_repair_tp".to_string(),
            lines(&["$tp @s $(x) $(y) $(z)".to_string()]),
        ));
    }
    out.push(("cs_repair".to_string(), lines(&repair)));
    out
}

/// Cutscene functions (spec-0008 addendum; keyframe dolly): the
/// two-camera bounce. Per cutscene (deduped by content key) emits a start
/// function, a self-scheduling keyframe/`spectate` driver, and an end/restore
/// function.
///
/// Mechanic: save each player's return point (a marker at a representative
/// player), spectator, then dolly two co-located invisible cameras along the
/// shot's keyframe schedule — a `tp` every `cadence` ticks with display-entity
/// `teleport_duration` set to the cadence, so the *client* tweens position and
/// rotation between keyframes ([`crate::camera`], spike-measured) — while
/// alternating `spectate` between the pair each tick (the naive same-entity
/// re-`spectate` is a server no-op — never emitted; the bounce cannot reset an
/// in-flight tween, measurement 4). The bounce skips any player actively
/// holding sneak (`predicate=!<ns>:sneak_held`, see [`SNEAK_HELD_PREDICATE`]):
/// sneak dismounts a spectator, so re-attaching against a held key strobes.
/// On completion, restore adventure mode + teleport players back to the marker.
///
/// **Path timing**: the dolly is arc-length parameterized (equal
/// distance per time, not equal segments per time) with baked smoothstep
/// ease-in/ease-out — both fixes live in [`crate::camera::plan_shot`].
///
/// **Aim** (DSL v0.6): every dolly `tp` carries an explicit `<yaw> <pitch>`, so a
/// spectating player looks where the shot means them to look instead of at the
/// summon default (yaw 0 = south). With `look_at`, the rotation is computed per
/// keyframe from the camera's own position toward the subject point (the framing
/// holds through the whole move, with the client tweening rotation between
/// keyframes); without it, the camera faces along the eased path's direction of
/// travel. Pure `atan2` on plan coordinates, rounded to 3 decimals:
/// deterministic, no RNG, no wall clock.
///
/// **Multi-shot** (DSL v0.6): a cutscene is a list of shots played back-to-back
/// inside ONE save/restore bracket — one marker, one `gamemode spectator`, one
/// camera pair, one restore. The shots share the single `#t_<bare>` tick counter:
/// shot `k` owns the half-open-on-the-right window `[offset_k, offset_k + len_k]`
/// and the next shot starts at `offset_k + len_k + 1`, so the transition is a hard
/// cut (the next tick teleports the camera pair to the new shot's first waypoint
/// with its own aim). A one-shot cutscene reduces to exactly the pre-multi-shot
/// timeline, so the single-shot spelling is byte-identical either way.
fn cutscene_fns(
    plan: &Plan,
    moves: &[crate::nav::MovePlan],
    actor_moves: &[crate::nav::ActorMovePlan],
) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for (eff, ctx) in crate::camera::cutscene_units(plan.campaign) {
        let Some(shots) = eff.cutscene_shots().filter(|s| !s.is_empty()) else {
            continue;
        };
        // `start` = the function emit_quest_effect calls (`cs_<bare>`); `bare` is
        // the shared suffix for the tick/end functions and per-cutscene sentinels.
        // Dedup is by DSL content: two byte-identical cutscene effects share one
        // generated function, planned from the FIRST occurrence's move context
        // (deterministic — the traversal order is fixed). An author who wants a
        // styled moving-subject cutscene to differ per context gives the shots
        // distinguishing content (e.g. an explicit `seconds`).
        let start_name = cutscene_fn(&shots);
        if !seen.insert(start_name.clone()) {
            continue;
        }
        let bare = start_name
            .strip_prefix("cs_")
            .unwrap_or(&start_name)
            .to_string();
        // Expand every shot (explicit path, or `shot_style` construction) to
        // its resolved geometry + aim. The air-corridor / chord / angular
        // checks (crate::nav, DW0308/DW0347) validate these exact expansions.
        let resolved: Vec<crate::camera::ExpandedShot> = {
            let mut off: i32 = 0;
            shots
                .iter()
                .map(|shot| {
                    let ex = crate::camera::expand_shot(plan, moves, actor_moves, shot, &ctx, off);
                    off += ex.ticks + 1;
                    ex
                })
                .collect()
        };
        let first =
            resolved[0]
                .clip_polyline()
                .first()
                .copied()
                .unwrap_or([0.0, plan::BASE_Y as f64, 0.0]);

        // start
        let mut start: Vec<String> = Vec::new();
        start.push(format!(
            "execute if score #run_{bare} dw.sys matches 1 run return fail"
        ));
        start.push(format!("scoreboard players set #run_{bare} dw.sys 1"));
        // The campaign-wide "some cutscene is playing" refcount. Placed AFTER the
        // re-entry `return fail` so a re-entrant start never inflates it. Read by
        // the join repair driver — see `cutscene_repair_fns`.
        start.push(format!("scoreboard players add {CS_LIVE} dw.sys 1"));
        start.push(format!("scoreboard players set #t_{bare} dw.sys 0"));
        start.push(format!("scoreboard players set #p_{bare} dw.sys 1"));
        start.push(format!(
            "execute at @p run summon minecraft:marker ~ ~ ~ {{Tags:[{FIXTURE_NBT}\"dw_csmark_{bare}\"]}}"
        ));
        // The cutscene state marker. `gamemode spectator` already takes the
        // players' bodies out of the world; the tag is what campaign machinery
        // reads so it does not keep asking anything of a player who is only
        // watching (see CUTSCENE_TAG).
        start.push(format!("tag @a add {CUTSCENE_TAG}"));
        start.push("gamemode spectator @a".to_string());
        for cam in ["a", "b"] {
            start.push(format!(
                "summon minecraft:item_display {} {} {} {{Tags:[\"dw_cam_{bare}\",\"dw_cam{cam}_{bare}\"]}}",
                fmt_f64(first[0]), fmt_f64(first[1]), fmt_f64(first[2])
            ));
        }
        start.push(format!("schedule function {ns}:cs_tick_{bare} 1t"));
        out.push((start_name.clone(), lines(&start)));

        // Keyframe driver: every shot's keyframes laid end-to-end on
        // one counter. Each shot plans an arc-length-parameterized, eased
        // keyframe schedule (`crate::camera::plan_shot`); the client draws the
        // in-between frames via display-entity `teleport_duration` (= the
        // shot's cadence), tweening position AND rotation — see the spike
        // measurements in `crate::camera`'s module docs.
        let mut tick: Vec<String> = Vec::new();
        let mut offset: i32 = 0;
        for (si, shot) in resolved.iter().enumerate() {
            let sf = shot.frames();
            // Cadence merge + snap share the shot's first tick: the position
            // sync is flushed before entity metadata within a tick (spike
            // measurement 5), so the snap `tp` lands instantly under the OLD
            // duration (0 — the summon default, or the previous shot's reset)
            // and the new cadence governs only the keyframes that follow.
            if sf.cadence > 0 {
                tick.push(format!(
                    "execute if score #t_{bare} dw.sys matches {offset} as @e[tag=dw_cam_{bare}] run data merge entity @s {{teleport_duration:{}}}",
                    sf.cadence
                ));
            }
            for f in &sf.frames {
                tick.push(format!(
                    "execute if score #t_{bare} dw.sys matches {} run tp @e[tag=dw_cam_{bare}] {} {} {} {} {}",
                    offset + f.tick,
                    fmt_f64(f.pos[0]), fmt_f64(f.pos[1]), fmt_f64(f.pos[2]),
                    fmt_f64(f.yaw), fmt_f64(f.pitch)
                ));
            }
            // Re-arm the hard cut: reset `teleport_duration` on the shot's last
            // owned tick — no keyframe is issued then, and a metadata change
            // does not disturb an in-flight tween (measurement 4/5) — so the
            // NEXT shot's snap is instant, not a glide.
            if sf.cadence > 0 && si + 1 < resolved.len() {
                tick.push(format!(
                    "execute if score #t_{bare} dw.sys matches {} as @e[tag=dw_cam_{bare}] run data merge entity @s {{teleport_duration:0}}",
                    offset + shot.ticks
                ));
            }
            offset += shot.ticks + 1;
        }
        // The last frame emitted sits at `offset - 1`; the driver ends one tick later.
        let total: i32 = offset - 1;
        // alternate `spectate` between the two co-located cameras (the bounce):
        // parity 1 → camera a, parity 2 → camera b, flipped each tick — but
        // NEVER at a player actively holding sneak. In spectator mode the sneak
        // key dismounts the spectated entity, so an unconditional per-tick
        // re-attach strobes (attach → client dismount → attach …) for as long
        // as the key is held (round-6 owner report). The vanilla `input` player
        // predicate ([`SNEAK_HELD_PREDICATE`], 1.21.2+) reads the raw key
        // state — including in spectator — so a held sneak yields a stable
        // detached spectator (frozen, staring at the world) and release
        // re-attaches on the next bounce tick, resuming the shot.
        tick.push(format!(
            "execute if score #p_{bare} dw.sys matches 1 as @a[predicate=!{ns}:{SNEAK_HELD_PREDICATE}] run spectate @n[type=minecraft:item_display,tag=dw_cama_{bare}] @s"
        ));
        tick.push(format!(
            "execute if score #p_{bare} dw.sys matches 2 as @a[predicate=!{ns}:{SNEAK_HELD_PREDICATE}] run spectate @n[type=minecraft:item_display,tag=dw_camb_{bare}] @s"
        ));
        tick.push(format!(
            "execute if score #p_{bare} dw.sys matches 2 run scoreboard players set #p_{bare} dw.sys 1"
        ));
        tick.push(format!(
            "execute if score #p_{bare} dw.sys matches 1 run scoreboard players set #p_{bare} dw.sys 2"
        ));
        tick.push(format!("scoreboard players add #t_{bare} dw.sys 1"));
        tick.push(format!(
            "execute if score #t_{bare} dw.sys matches {}.. run function {ns}:cs_end_{bare}",
            total + 1
        ));
        tick.push(format!(
            "execute unless score #t_{bare} dw.sys matches {}.. run schedule function {ns}:cs_tick_{bare} 1t",
            total + 1
        ));
        out.push((format!("cs_tick_{bare}"), lines(&tick)));

        // end / restore: leaving spectator returns each player to their
        // pre-spectator position; the explicit tp to the saved marker makes the
        // restore robust (spec addendum: restore gamemode + position).
        let mut end: Vec<String> = vec![
            "gamemode adventure @a".to_string(),
            format!("tp @a @e[tag=dw_csmark_{bare},limit=1]"),
            format!("kill @e[tag=dw_cam_{bare}]"),
            format!("kill @e[tag=dw_csmark_{bare}]"),
        ];
        // Resume: drop the cutscene marker. The stealth judge (zone-presence
        // only — no sneak stat is tracked) needs no re-sync;
        // grace is deliberately NOT reset — it neither accrued nor expired
        // during the cutscene, so the beat picks up exactly where it paused.
        end.push(format!("tag @a remove {CUTSCENE_TAG}"));
        end.push(format!("scoreboard players set #run_{bare} dw.sys 0"));
        end.push(format!("scoreboard players remove {CS_LIVE} dw.sys 1"));
        out.push((format!("cs_end_{bare}"), lines(&end)));
    }
    out
}

/// Environment-trigger interaction-entity summons (strike/use) for
/// `setup_finish`. Approach triggers need no entity. Empty for a campaign with no
/// triggers (byte-identical v0.2/v0.3).
///
/// A left-click trigger that rides an NPC — `strike-npc` (DSL v0.6), or the
/// pre-0.6 `strike` on the NPC's own stand anchor — gets **no entity of its
/// own**: the NPC's interaction hitbox is the trigger's sole carrier
/// ([`npc_hitbox_trigger_tags`]). Emitting a second, exactly co-located hitbox
/// here made the vanilla client's entity ray-pick ambiguous — an exact tie
/// resolves to whichever entity the pick iterates first, in practice this
/// world-init summon — so every right-click landed on an entity without the
/// `dw_npc_<n>` tag and the `player_interacted_with_entity` dialogue
/// advancement never fired (round-6 island QA: after the boulder seal,
/// Polyphemus could not be talked to at all). One cell, one hitbox. The
/// trigger's lifecycle therefore follows the NPC's presence — which is also
/// its meaning: the thing being struck is the NPC.
fn env_trigger_setup(plan: &Plan, chrome: &delvewright_dsl::Chrome) -> Vec<String> {
    use delvewright_dsl::TriggerOn;
    let mut out = Vec::new();
    for t in &plan.emitted_triggers(chrome) {
        if matches!(t.on, TriggerOn::Approach { .. }) {
            continue;
        }
        // `strike-npc` never has a cell of its own; a `strike` on an NPC's stand
        // anchor gives its cell up to that NPC's hitbox. Either way, no summon.
        let Some(at) = t.at_anchor() else {
            continue;
        };
        if matches!(t.on, TriggerOn::Strike) && npc_stands_at(plan, at) {
            continue;
        }
        // Same rule, one layer out: a click trigger anchored on a gate
        // the campaign SEALS rides that seal's own hitboxes — `seal_arm_<safe>`
        // summons them wearing this trigger's tag. A second entity here would be
        // exactly co-located with them, and the ray-pick tie is what killed the
        // island's boulder hint (`DESIGN.md` round 13). One cell, one hitbox.
        let tag = format!("dw_trig_{}", plan::safe_local(t.id.as_str()));
        match crate::pressable::body_at(plan, at) {
            // An existing set covers this anchor; `seal_fns` / `ws_arm_fns` put
            // this trigger's tag on those entities. One cell, one hitbox.
            crate::pressable::Body::Rides { .. } => {}
            // The anchor is a REGION. A point body here is buried in the solid
            // block and reachable from nowhere (see `crate::pressable`), so the
            // object gets the clickable shape it actually has: one protruding box
            // per shell cell, exactly as a `close-gate` seal has always done.
            crate::pressable::Body::Region(cells) => {
                for c in cells {
                    let x = fmt_centi(c[0] as i64 * 100 + 50);
                    let y = fmt_centi(c[1] as i64 * 100 - 1);
                    let z = fmt_centi(c[2] as i64 * 100 + 50);
                    out.push(format!(
                        "summon minecraft:interaction {x} {y} {z} \
                         {{width:{SEAL_BOX_SIZE},height:{SEAL_BOX_SIZE},response:1b,Invulnerable:1b,Tags:[{FIXTURE_NBT}\"{tag}\"]}}"
                    ));
                }
            }
            // A point in open space: the ordinary body, byte-identical.
            crate::pressable::Body::Point(p) => {
                let q = ent_xyz(p);
                out.push(format!(
                    "summon minecraft:interaction {} {} {} {{width:1.0f,height:2.0f,response:1b,Invulnerable:1b,Tags:[{FIXTURE_NBT}\"{tag}\"]}}",
                    q[0], q[1], q[2]
                ));
            }
            // `DW0426` has already failed the build.
            crate::pressable::Body::Nothing => {}
        }
    }
    out
}

/// `DW0426`: every click trigger must be anchored somewhere a player can click.
///
/// Build tier (exit 3), raised before any function is emitted. The trigger
/// declares an anchor, a click and a full effect bundle, and the press lands on
/// nothing — the beat never happens and every board stays green, which is the
/// unbound-vacuity class this whole task came out of.
fn check_trigger_bodies(plan: &Plan) -> Result<crate::pressable::PressLedger, BuildFailure> {
    use delvewright_dsl::TriggerOn;
    let mut ledger = crate::pressable::PressLedger::default();
    // Every trigger this build EMITS, not only the campaign's own: a press answer
    // the compiler synthesizes lands on a body exactly as an authored click does,
    // and a proof that walked the authored list alone would leave the compiler's
    // own presses unexamined and the ledger's count short of what shipped.
    for t in &plan.emitted_triggers_unlocalized() {
        if matches!(t.on, TriggerOn::Approach { .. }) {
            continue;
        }
        let Some(at) = t.at_anchor() else {
            continue;
        };
        if matches!(t.on, TriggerOn::Strike) && npc_stands_at(plan, at) {
            ledger.push(
                t.id.as_str(),
                t.on.kind(),
                at,
                "rides the NPC's dialogue hitbox",
            );
            continue;
        }
        let body = crate::pressable::body_at(plan, at);
        if body != crate::pressable::Body::Nothing {
            ledger.push(
                t.id.as_str(),
                t.on.kind(),
                at,
                &crate::pressable::describe(&body),
            );
            continue;
        }
        return Err(BuildFailure::Diagnostic {
            code: crate::pressable::DW_TRIGGER_UNPRESSABLE,
            message: format!(
                "trigger `{}` watches a `{}` on anchor `{}`, but nothing at that anchor is \
                 clickable: it resolves to no placed piece, so the compiler has no cell to give \
                 the trigger a body at and the press can never land. The trigger's effects would \
                 simply never run, with every check green. Prescription: anchor it on a place a \
                 prefab provides (anchor names come from prefab metadata; do NOT invent one), or \
                 drop the trigger.",
                t.id,
                t.on.kind(),
                at
            ),
        });
    }
    Ok(ledger)
}

/// Environment-trigger per-tick checks for the `tick` function. Empty for a
/// campaign with no triggers.
///
/// **Two phases, not one, for the click triggers** (round-8 island QA). A click
/// trigger is `if <record present> run <effects>` followed by `data remove` of the
/// record — the removal is what makes a held-down click fire exactly once. Emitting
/// that pair *per trigger*, inline, is only sound while at most one trigger reads a
/// given interaction entity. Several `strike-npc` triggers legitimately ride ONE
/// NPC hitbox (see [`npc_hitbox_trigger_tags`]) — the island's giant carries
/// `wake-the-giant` (requires `flag/asleep`) and `his-house` (forbids it), one
/// hitbox, mutually exclusive gates. Inline removal made the FIRST-DECLARED trigger
/// consume the record even when its own gate was shut, so `his-house` could never
/// see a click and never fired: a suppressed trigger starved its siblings, and which
/// one starved depended on declaration order.
///
/// So the record is read by every trigger first and cleared afterwards: all fire
/// clauses in declaration order, then all clear clauses. The semantics become
/// order-independent — every trigger sharing a hitbox sees the same click, and each
/// fires exactly when its own gate says so. Consumption is unchanged (the record is
/// gone by the end of the same `tick` pass, so a held click still fires once).
///
/// Byte impact: a campaign whose click triggers are its last-declared triggers is
/// unchanged; any other ordering moves the clear clauses to the end of the block.
fn env_trigger_tick(plan: &Plan, chrome: &delvewright_dsl::Chrome) -> Vec<String> {
    use delvewright_dsl::TriggerOn;
    let ns = &plan.namespace;
    let mut out = Vec::new();
    // Phase 2, accumulated while phase 1 is emitted: `(tag, record)` for every
    // click trigger, in declaration order (deterministic).
    let mut clears: Vec<String> = Vec::new();
    for t in &plan.emitted_triggers(chrome) {
        // A `presser` trigger is not polled at all (DSL v0.11): its dispatch is a
        // `player_interacted_with_entity` advancement, which is the only vanilla
        // primitive that knows WHO pressed. It therefore also emits no `data
        // remove` — an advancement observes the click without consuming it, which
        // is what lets a press answer share one hitbox with a polled trigger and
        // neither eat the other's record (round-8: adjudicate conditionally,
        // consume unconditionally).
        if t.addresses_presser() {
            continue;
        }
        let id = plan::safe_local(t.id.as_str());
        let once_guard = if t.once {
            format!("unless score #trig_{id} dw.sys matches 1 ")
        } else {
            String::new()
        };
        // Flags are party state (spec-0018): the gate is a single `#party` read,
        // positive and negative alike. `unless … matches 1` is unset-safe (an
        // uninitialized flag score counts as "not set").
        let flag_guard = format!(
            "{}{}",
            party_flag_gate(&t.requires_flags),
            // DSL v0.10 (spec-0031). A trigger's arming gate is a party predicate
            // (`DW0503` keeps `player`-scoped data out of it).
            state_cond(plan, &t.requires_state, false)
        );
        let forbid_guard: String = t
            .forbids_flags
            .iter()
            .map(|f| {
                format!(
                    "unless score {} {} matches 1 ",
                    plan::PARTY,
                    plan::flag_score(f.as_str())
                )
            })
            .collect();
        match &t.on {
            TriggerOn::Strike | TriggerOn::Use | TriggerOn::StrikeNpc { .. } => {
                // The two click streams are separate NBT fields on ONE
                // `minecraft:interaction`: a left-click writes `attack`, a
                // right-click writes `interaction`. That is what lets a
                // `strike-npc` trigger share the hitbox with the NPC's dialogue
                // — the dialogue advancement reads the right-click, this reads
                // the left-click, and neither consumes the other's record.
                let rec = match t.on {
                    TriggerOn::Use => "interaction",
                    _ => "attack",
                };
                // Fire when the interaction entity has recorded the event and (if
                // gated) the party holds the flags; then clear the record.
                let flag_cond = if flag_guard.is_empty() {
                    String::new()
                } else {
                    format!("{} ", flag_guard.trim_start())
                };
                out.push(format!(
                    "execute {once_guard}{forbid_guard}if entity @e[tag=dw_trig_{id},nbt={{{rec}:{{}}}}] {flag_cond}run function {ns}:trig_{id}"
                ));
                clears.push(format!(
                    "execute as @e[tag=dw_trig_{id}] run data remove entity @s {rec}"
                ));
            }
            TriggerOn::Approach { range } => {
                if let Some(p) = t.at_anchor().and_then(|at| anchor_point_any(plan, at)) {
                    // The proximity test stays per-player (`@a[distance=…]` — SOME
                    // party member walked in); the flag gate is a party read
                    // alongside it, no longer merged into the selector.
                    out.push(format!(
                        "execute {once_guard}{forbid_guard}positioned {} {} {} if entity @a[distance=..{range}]{} run function {ns}:trig_{id}",
                        p[0], p[1], p[2], flag_guard
                    ));
                }
            }
        }
    }
    out.extend(clears);
    out
}

/// Environment-trigger effect functions (`trig_<id>`). A trigger is a **party
/// event** (spec-0018): the beat fires once and its player-facing effects address
/// `@a` on their own (see [`Audience::Party`]), so the bundle is emitted plainly
/// — no outer `as @a`, which under party state would fire every `fill`, driver
/// start and party-holder write once per player. `once` sets a global sentinel so
/// the trigger fires at most once.
///
/// The function is dispatched from `env_trigger_tick` without an executor for an
/// `approach`/`strike`/`use` trigger, so nothing here may rely on `@s` — the
/// party model means nothing needs to.
///
/// **`#trig_<id>` is written by every trigger, not only `once` ones** (round-8).
/// `once` *reads* it as its at-most-once guard, but the write is what makes trigger
/// dispatch observable at all: without it a repeatable trigger firing leaves no
/// machine-readable trace, and the shared-hitbox starvation bug — where a trigger
/// simply never fired — was invisible to every automated check the repo had. One
/// scoreboard write on a rare event buys a PackTest that can assert *which* of two
/// triggers on one hitbox actually ran (`v06_shared_hitbox`). Byte impact: one added
/// line per non-`once` trigger function.
///
/// **An `audience: presser` trigger reverses both of those** (DSL v0.11). Its
/// bundle is emitted under [`Audience::Solo`] — `@s` is the player who pressed —
/// and it is reached from a second, tiny function [`press_dispatch_fn`] that the
/// interaction advancement rewards, rather than from the tick. Everything else is
/// identical, which is the point: a press answer is not a mechanism, it is this
/// mechanism with a different addressee.
fn env_trigger_fns(plan: &Plan, chrome: &delvewright_dsl::Chrome) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for t in &plan.emitted_triggers(chrome) {
        let id = plan::safe_local(t.id.as_str());
        if t.addresses_presser() {
            out.push(press_dispatch_fn(plan, t, &id));
        }
        let mut body: Vec<String> = Vec::new();
        body.push(format!("scoreboard players set #trig_{id} dw.sys 1"));
        // Striker capture (owner directive, round 8). The click record is still on
        // the hitbox here — `env_trigger_tick` clears every record only after every
        // trigger has been offered it — so this is the one place the acting player's
        // UUID is knowable. Parked in storage rather than passed as an argument
        // because `unleash-actor` may sit behind any amount of nesting inside this
        // bundle, and removed again below so it can never leak into a later beat.
        let capture = trigger_is_click(t) && trigger_unleashes(t);
        if capture {
            let rec = trigger_record(t);
            body.push(format!(
                "data modify storage {STRIKER_STORAGE} {STRIKER_PATH} set from entity @e[tag=dw_trig_{id},limit=1] {rec}.player"
            ));
        }
        // The trigger's own flag gate is already proven by `env_trigger_tick`
        // (or by `press_<id>`) before it dispatches here; each effect still
        // carries its own gate.
        for e in &t.effects {
            emit_gated_effect(plan, e, trigger_audience(t), &mut body);
        }
        if capture {
            body.push(format!(
                "data remove storage {STRIKER_STORAGE} {STRIKER_PATH}"
            ));
        }
        out.push((format!("trig_{id}"), lines(&body)));
    }
    out
}

/// The audience one trigger's bundle is emitted under.
///
/// [`root_audience`] answers for the root *class*, and remains the authority the
/// DSL's `EffectRootKind::runs_with_acting_player` is bound to. A trigger is the
/// one root whose audience is a per-declaration fact (DSL v0.11), and this is
/// where that is resolved — bound to the DSL by
/// `EffectRootOwner::runs_with_acting_player`, which `DW0503` reads, so the
/// validator and the emitter cannot disagree about whether `@s` exists.
fn trigger_audience(t: &delvewright_dsl::EnvTrigger) -> Audience {
    if t.addresses_presser() {
        Audience::Solo
    } else {
        root_audience(delvewright_dsl::EffectRootKind::Trigger)
    }
}

/// The advancement reward function of an `audience: presser` trigger (DSL v0.11):
/// `press_<id>`, which revokes its own grant and then runs the trigger's bundle
/// **as the player who right-clicked**.
///
/// This is `seal_hint_<safe>` generalized off the verb it was built onto. The
/// revoke is what makes the object answer *every* press rather than only the
/// first — a wall is not consumed by being asked — and `once`, the flag gate and
/// the state gate are re-stated here because for a presser trigger this function
/// takes the place of the tick clause that would otherwise have carried them.
/// They are the trigger's own, spelled exactly as `env_trigger_tick` spells them,
/// so the two dispatch routes gate identically.
fn press_dispatch_fn(plan: &Plan, t: &delvewright_dsl::EnvTrigger, id: &str) -> (String, String) {
    let ns = &plan.namespace;
    let once_guard = if t.once {
        format!("unless score #trig_{id} dw.sys matches 1 ")
    } else {
        String::new()
    };
    let forbid_guard: String = t
        .forbids_flags
        .iter()
        .map(|f| {
            format!(
                "unless score {} {} matches 1 ",
                plan::PARTY,
                plan::flag_score(f.as_str())
            )
        })
        .collect();
    let flag_guard = format!(
        "{}{}",
        party_flag_gate(&t.requires_flags),
        state_cond(plan, &t.requires_state, false)
    );
    // An ungated press answer — which is every one the compiler synthesizes —
    // calls its bundle outright. `execute run function …` is legal and would
    // work, but a conditionless `execute` in shipped output reads as a guard
    // somebody deleted.
    let dispatch = if once_guard.is_empty() && forbid_guard.is_empty() && flag_guard.is_empty() {
        format!("function {ns}:trig_{id}")
    } else {
        format!("execute {once_guard}{forbid_guard}{flag_guard}run function {ns}:trig_{id}")
    };
    (
        format!("press_{id}"),
        lines(&[
            format!("advancement revoke @s only {ns}:press_{id}"),
            dispatch,
        ]),
    )
}

// ---------------------------------------------------------------------------
// v0.6 traps (spec-0011)
// ---------------------------------------------------------------------------

/// `DW0363`: a trap declares a flag gate (`requires_flags` / `forbids_flags`) but
/// its trigger hardware cannot be removed and put back exactly as authored, so the
/// compiler refuses to pretend the gate works.
pub const DW_TRAP_GATE_UNSUPPORTED: DwCode = DwCode::every_version("DW0363");

/// Trap flag-gating hardware: for every trap that declares a flag gate, the
/// trigger block its `anchor/trap` prefab metadata declares — the thing the gate
/// physically removes and restores.
///
/// The compiler owns world mutation, so "the trap is inactive while a flag is set"
/// is implemented as exactly that: the plate or tripwire is taken out of the world
/// and put back on the flag transition. The block comes from prefab metadata for
/// the same reason a `close-gate`'s fill block does (`DW0343`): the hardware is
/// baked into the `.nbt` and only the prefab author knows what it is. It is
/// restored **verbatim, blockstate and all** — stamping a bare id over an authored
/// state silently changes the block (the `DW0354` lesson).
///
/// Only a trigger whose entire state is the block itself can be gated this way. A
/// `trapped-chest` trigger carries a **block entity with an inventory** that removal
/// would destroy and the compiler could not restore, so a flag gate on one is
/// rejected (`DW0363`) rather than shipped as folklore. So is a gated trap whose
/// prefab declares no `trigger_block` at all: be loud, do not guess.
fn trap_gate_hardware(
    plan: &Plan,
    prefabs: &crate::registry::PrefabRegistry,
) -> Result<BTreeMap<String, String>, BuildFailure> {
    let mut out = BTreeMap::new();
    for t in plan.traps.iter().filter(|t| trap_is_gated(t)) {
        let declared = prefabs.trap_trigger_block(&t.at_anchor);
        let gatable = declared
            .map(|b| b.split('[').next().unwrap_or(b))
            .is_some_and(crate::assembled::is_passable_trap_trigger);
        if !gatable {
            let what = match declared {
                Some(b) => format!("declares `trigger_block` `{b}`"),
                None => "declares no `trigger_block`".to_string(),
            };
            return Err(BuildFailure::Diagnostic {
                code: DW_TRAP_GATE_UNSUPPORTED,
                message: format!(
                    "trap `{}` declares a gate (`requires_flags`/`forbids_flags`/ \
                     `requires_state`), but its `anchor/trap` marker `{}` {what}. A gate \
                     physically removes the trigger from the world while it is shut and puts \
                     it back after, so \
                     it is only sound for a trigger whose whole state is the block: a pressure \
                     plate or a tripwire. A `trapped-chest` trigger carries a block entity with \
                     an inventory that removal would destroy. Declare the plate/tripwire as \
                     `trigger_block` on the anchor's prefab metadata (with its blockstate, as a \
                     gate anchor declares its fill `block`), switch the trap to a \
                     `pressure-plate`/`tripwire` trigger, or drop the gate and gate the \
                     story beat that arms the trap instead.",
                    t.id, t.at_anchor,
                ),
            });
        }
        out.insert(t.safe.clone(), declared.unwrap_or_default().to_string());
    }
    Ok(out)
}

/// Whether `t` declares a gate at all — flags, negative flags, or (DSL v0.10) a
/// numeric comparison. An ungated trap emits nothing new, so every existing
/// campaign stays byte-identical.
///
/// All three axes, not two: the arming machinery a gated trap gets is the same
/// machinery whichever axis shut it, and a trap gated only by a number that
/// quietly skipped it would be armed forever.
fn trap_is_gated(t: &plan::TrapPlan) -> bool {
    !t.requires_flags.is_empty() || !t.forbids_flags.is_empty() || !t.requires_state.is_empty()
}

/// The `tick` clauses that open and shut every gated trap's hardware.
///
/// Edge-triggered on a per-trap sentinel `#trapgate_<safe>` (1 = armed, i.e. the
/// trigger block is in the world) so the `setblock` fires only on a transition —
/// a per-tick unconditional write would be both wasteful and wrong (it would also
/// fight the disarm path).
///
/// The gate is **campaign state, not per-player state**: flags are set by whoever
/// reaches the beat, and a trap does not become live for one player and dead for
/// another. So the guards use the same any-player form the environment triggers
/// use — `if entity @a[scores={dw.f_x=1..}]` — rather than `score @s`.
///
/// Shutting is one clause per gating flag because "not (all required set and no
/// forbidden set)" is a disjunction: any single unmet requirement, or any single
/// forbidden flag, shuts the gate on its own. Each is idempotent behind the
/// sentinel.
fn trap_gate_tick(plan: &Plan) -> Vec<String> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for t in plan.traps.iter().filter(|t| trap_is_gated(t)) {
        let id = &t.safe;
        for f in &t.requires_flags {
            out.push(format!(
                "execute if score #trapgate_{id} dw.sys matches 1 unless entity @a[scores={{{}=1..}}] run function {ns}:trap_gate_off_{id}",
                plan::flag_score(f)
            ));
        }
        for f in &t.forbids_flags {
            out.push(format!(
                "execute if score #trapgate_{id} dw.sys matches 1 if entity @a[scores={{{}=1..}}] run function {ns}:trap_gate_off_{id}",
                plan::flag_score(f)
            ));
        }
        // DSL v0.10 (spec-0031): one shut clause per numeric term — any single
        // term ceasing to hold disarms the trap, which is what `negate` spells.
        // The datum is party-scoped by construction here (`DW0503`).
        for clause in state_clauses(plan, &t.requires_state, true) {
            out.push(format!(
                "execute if score #trapgate_{id} dw.sys matches 1 {clause} run function {ns}:trap_gate_off_{id}"
            ));
        }
        let mut on = format!("execute unless score #trapgate_{id} dw.sys matches 1");
        for f in &t.requires_flags {
            on.push_str(&format!(
                " if entity @a[scores={{{}=1..}}]",
                plan::flag_score(f)
            ));
        }
        for f in &t.forbids_flags {
            on.push_str(&format!(
                " unless entity @a[scores={{{}=1..}}]",
                plan::flag_score(f)
            ));
        }
        on.push_str(&state_cond(plan, &t.requires_state, false));
        on.push_str(&format!(" run function {ns}:trap_gate_on_{id}"));
        out.push(on);
    }
    out
}

/// The `trap_gate_on_<safe>` / `trap_gate_off_<safe>` pair per gated trap: flip the
/// sentinel and write the trigger cell. `on` restores the authored block verbatim
/// (state and all); `off` clears it to air, which is what the plate/tripwire cell
/// is when the piece does not carry one.
fn trap_gate_fns(plan: &Plan, hardware: &BTreeMap<String, String>) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for t in plan.traps.iter().filter(|t| trap_is_gated(t)) {
        let id = &t.safe;
        let c = t.trigger_cell;
        let Some(block) = hardware.get(id) else {
            continue;
        };
        out.push((
            format!("trap_gate_on_{id}"),
            lines(&[
                format!("scoreboard players set #trapgate_{id} dw.sys 1"),
                format!("setblock {} {} {} {block}", c[0], c[1], c[2]),
            ]),
        ));
        out.push((
            format!("trap_gate_off_{id}"),
            lines(&[
                format!("scoreboard players set #trapgate_{id} dw.sys 0"),
                format!("setblock {} {} {} minecraft:air", c[0], c[1], c[2]),
            ]),
        ));
    }
    out
}

/// `setup_finish` commands for container fills (spec-0021): give each declared
/// container its contents with `item replace block … container.<slot>`, the same
/// deterministic mechanism a trap dispenser and a `collect` chest already use —
/// no raw NBT, no loot tables, no RNG.
///
/// **Slot assignment is positional**: the nth declared stack lands in
/// `container.<n>`. That is the whole determinism story (ADR-0006) — the same
/// DSL always produces the same chest, byte for byte, with no shuffling and no
/// seeded placement to reproduce.
///
/// The container itself is never placed here; it is prefab furniture, and
/// `DW0431` has already proven one is really there.
fn loot_setup(loot: &[crate::plan::LootPlan]) -> Vec<String> {
    let mut out = Vec::new();
    for l in loot {
        let c = l.cell;
        for (slot, it) in l.items.iter().enumerate() {
            out.push(format!(
                "item replace block {} {} {} container.{slot} with {}{} {}",
                c[0],
                c[1],
                c[2],
                it.item,
                container_stack_components(it.name.as_deref(), &it.enchantments),
                it.count
            ));
        }
    }
    out
}

/// The `[custom_name=…,enchantments=…]` component suffix a container-fill stack
/// carries in `item replace … with <item><suffix> <count>`, or `""` when it
/// carries neither — which is what keeps every unnamed, unenchanted fill
/// byte-identical to the emission that predates both fields.
///
/// ONE renderer for every container fill: spec-0021 `loot` and the DSL v0.8
/// `collect` `item_name`. A quest item named on one surface and
/// unnamed on the other would be the same defect the wave-arming table taught —
/// two places describing one stack, drifting apart the moment either moves.
/// Enchantment order is the `BTreeMap`'s id order, never hash order (ADR-0006).
fn container_stack_components(
    name: Option<&str>,
    ench: &std::collections::BTreeMap<String, u32>,
) -> String {
    let mut comps: Vec<String> = Vec::new();
    if let Some(n) = name {
        comps.push(format!(
            "custom_name={}",
            tr_with(n, &[("italic", json!(false))])
        ));
    }
    if !ench.is_empty() {
        let body = ench
            .iter()
            .map(|(id, lvl)| format!("\"{id}\":{lvl}"))
            .collect::<Vec<_>>()
            .join(",");
        comps.push(format!("enchantments={{{body}}}"));
    }
    if comps.is_empty() {
        return String::new();
    }
    format!("[{}]", comps.join(","))
}

/// The component suffix a v0.8 `collect` stack carries: its `item_name`, or `""`
/// when the objective declares none. A thin alias over
/// [`container_stack_components`] — a collect stack is a container fill, and is
/// rendered by the container fill's renderer.
fn item_component_tail(name: Option<&str>) -> String {
    container_stack_components(name, &std::collections::BTreeMap::new())
}

/// `setup_finish` commands for traps (spec-0011): fill each `dispense` trap's
/// prefab dispenser with its static payload (`item replace block … container.0`,
/// the same deterministic mechanism as a `collect` chest — no raw NBT), and summon
/// the disarm affordance's interaction entity. The trap's *harm* needs no command:
/// the plate/tripwire/trapped-chest → dispenser redstone is already in the prefab.
/// Empty for a campaign with no traps → byte-identical.
fn trap_setup(plan: &Plan, gate_hardware: &BTreeMap<String, String>) -> Vec<String> {
    let mut out = Vec::new();
    for t in &plan.traps {
        // Seed a gated trap's hardware sentinel to match the world it starts in.
        // Flags are unset at world start, so a `requires_flags` gate is shut and the
        // authored trigger comes straight back out; a `forbids_flags`-only gate is
        // open and the prefab's own block stands. Doing this at setup (rather than
        // letting the tick converge) means there is never a tick in which the trap
        // is live before its gate has been read.
        if trap_is_gated(t) && gate_hardware.contains_key(&t.safe) {
            let armed = t.requires_flags.is_empty();
            out.push(format!(
                "scoreboard players set #trapgate_{} dw.sys {}",
                t.safe,
                u8::from(armed)
            ));
            if !armed {
                let c = t.trigger_cell;
                out.push(format!("setblock {} {} {} minecraft:air", c[0], c[1], c[2]));
            }
        }
        // Fill the pre-wired dispenser with the declared payload.
        if let (Some(disp), Some((item, count))) = (t.dispenser, &t.payload) {
            out.push(format!(
                "item replace block {} {} {} container.0 with {item} {count}",
                disp[0], disp[1], disp[2]
            ));
        }
        // spec-0022: a `trapped-chest` trigger with a command payload needs a
        // detection surface, and the only player-distinct one vanilla offers is
        // the v0.4 interaction entity (`use`) — the SAME primitive the disarm
        // affordance already uses. Reading the chest's redstone output would be
        // block-power polling, which spec-0011 excluded as folklore.
        //
        // No `affordance_hardware` accompanies this one (cf. `DW0420` below):
        // the trapped chest IS the visible hardware, authored in the prefab as
        // the trap's trigger block. The invisible-lever failure that rule exists
        // to catch is an interaction with nothing to look at — not the case here.
        if !t.payload_effects.is_empty() && t.trigger == delvewright_dsl::TrapTrigger::TrappedChest
        {
            let v = ent_xyz(t.trigger_cell);
            out.push(format!(
                "summon minecraft:interaction {} {} {} {{width:1.0f,height:2.0f,response:1b,Invulnerable:1b,Tags:[{FIXTURE_NBT}\"dw_trapfire_{}\"]}}",
                v[0], v[1], v[2], t.safe
            ));
        }
        // Summon the disarm interaction affordance (a right-click target) and
        // the visible hardware that makes it findable. The prefab may ALSO dress
        // the cell, but the compiler no longer depends on that: an invisible
        // `minecraft:interaction` on its own is a lever the player cannot see
        // (the drowned-bell class, `DW0420`).
        if let Some(dis) = &t.disarm {
            let v = ent_xyz(dis.via_cell);
            out.push(format!(
                "summon minecraft:interaction {} {} {} {{width:1.0f,height:2.0f,response:1b,Invulnerable:1b,Tags:[{FIXTURE_NBT}\"dw_trapdis_{}\"]}}",
                v[0], v[1], v[2], t.safe
            ));
            out.push(affordance_hardware(
                v,
                &format!("dw_trapdis_{}", t.safe),
                "minecraft:lever",
            ));
        }
    }
    out
}

/// Per-tick disarm detection for disarmable traps (spec-0011), reusing the v0.4
/// interaction-entity `use` primitive: when a player right-clicks the disarm
/// affordance, fire the disarm once. Empty for a campaign with no disarmable traps.
fn trap_tick(plan: &Plan) -> Vec<String> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    // spec-0022: fire the command payload when the trigger is sprung. Redstone
    // keeps exactly one job — being the visible, learnable trigger — and the
    // consequence is commands, so the compiler owns the detection tick.
    //
    // Detection reuses the two primitives already in the compiler and adds none:
    // a plate/tripwire is a POSITION test on the trigger cell (the `reach-anchor`
    // idiom), a trapped chest is the v0.4 interaction `use`. Neither reads block
    // power — that would be the polling hack spec-0011 excluded.
    out.extend(trap_fire_tick(plan));
    for t in &plan.traps {
        if t.disarm.is_none() {
            continue;
        }
        let id = &t.safe;
        out.push(format!(
            "execute unless score #trapdis_{id} dw.sys matches 1 if entity @e[tag=dw_trapdis_{id},nbt={{interaction:{{}}}}] run function {ns}:trap_disarm_{id}"
        ));
        out.push(format!(
            "execute as @e[tag=dw_trapdis_{id}] run data remove entity @s interaction"
        ));
    }
    out.extend(trap_gate_tick(plan));
    out
}

/// Disarm functions (`trap_disarm_<id>`) for disarmable traps (spec-0011). Firing
/// once (`#trapdis_<id>` sentinel): set the disarm flag party-wide (so
/// `requires_flags` reads elsewhere see it) and **empty the dispenser** — the
/// modeled, global disarm that actually stops a redstone-native dispense trap for
/// everyone. Empty for a campaign with no disarmable traps.
fn trap_fns(plan: &Plan, gate_hardware: &BTreeMap<String, String>) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for t in &plan.traps {
        let Some(dis) = &t.disarm else {
            continue;
        };
        let id = &t.safe;
        let mut body: Vec<String> = Vec::new();
        body.push(format!("scoreboard players set #trapdis_{id} dw.sys 1"));
        body.push(format!(
            "scoreboard players set {} {} 1",
            plan::PARTY,
            plan::flag_score(&dis.sets_flag)
        ));
        if let Some(disp) = t.dispenser {
            // Empty the dispenser to an empty stack list — the modeled, global disarm
            // that actually stops a redstone-native dispense trap (no ammo → no fire).
            body.push(format!(
                "data modify block {} {} {} Items set value []",
                disp[0], disp[1], disp[2]
            ));
        }
        // The lever has been thrown: the disarm affordance is spent, so its
        // visible hardware retires with it. The ONE function allowed to do this
        // — `DW0421` fails the build if any other machinery reaches it.
        body.push(format!(
            "kill @e[tag={}]",
            crate::affordance::hardware_tag(&format!("dw_trapdis_{id}"))
        ));
        out.push((format!("trap_disarm_{id}"), lines(&body)));
    }
    out.extend(trap_payload_fns(plan));
    out.extend(trap_gate_fns(plan, gate_hardware));
    out
}

// ---------------------------------------------------------------------------
// spec-0022 — trap payload verbs (`volley`, `collapse`)
// ---------------------------------------------------------------------------

/// `DW0447`: a trap-payload verb centres its volume on an anchor no placed
/// prefab piece provides, so the kill zone / collapse region cannot be resolved.
pub const DW_PAYLOAD_ANCHOR_UNRESOLVED: DwCode = DwCode::every_version("DW0447");

/// A planned `volley`: the proven per-cell geometry plus its authored cadence.
struct VolleyEmit {
    key: String,
    geom: crate::nav::VolleyGeometry,
    projectile: String,
    salvos: u32,
    interval: u32,
}

/// A planned `collapse`: the settled debris plus its authored materials.
struct CollapseEmit {
    key: String,
    geom: crate::nav::CollapseGeometry,
    falling_block: String,
    then_floor: Option<String>,
}

/// Every spec-0022 payload verb in the campaign, resolved and proven. Empty for
/// a campaign that uses none, so its output stays byte-identical.
#[derive(Default)]
struct PayloadPlans {
    volleys: Vec<VolleyEmit>,
    collapses: Vec<CollapseEmit>,
}

/// Resolve and PROVE every `volley` / `collapse` in the campaign against the
/// assembled world (spec-0022).
///
/// Coverage is proven by construction: [`crate::nav::plan_volley`] returns one
/// shot per standable kill-zone cell or an error naming the cell it cannot
/// reach, and the emitter writes exactly those shots. There is no path by which
/// a volley ships covering less than its declared zone.
fn plan_payload_verbs(
    plan: &Plan,
    world: &crate::nav::World,
    blocks: &BTreeMap<[i32; 3], String>,
) -> Result<PayloadPlans, BuildFailure> {
    let mut out = PayloadPlans::default();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for eff in all_campaign_effects(plan.campaign) {
        let key = payload_verb_key(eff);
        if let Some((projectile, from_anchor, kill_zone, salvos, interval)) = eff.volley() {
            if !seen.insert(format!("v{key}")) {
                continue;
            }
            let label = format!("volley from `{from_anchor}` into `{}`", kill_zone.anchor);
            let from = plan
                .point_any(from_anchor.as_str())
                .ok_or_else(|| payload_anchor_failure(&label, from_anchor.as_str()))?;
            let region = plan
                .zone_box(kill_zone)
                .ok_or_else(|| payload_anchor_failure(&label, kill_zone.anchor.as_str()))?;
            let geom = crate::nav::plan_volley(world, from, region, &label)?;
            out.volleys.push(VolleyEmit {
                key,
                geom,
                projectile: projectile.to_string(),
                salvos,
                interval,
            });
        } else if let Some((region_anchor, falling_block, then_floor)) = eff.collapse() {
            if !seen.insert(format!("c{key}")) {
                continue;
            }
            let label = format!("collapse of `{}`", region_anchor.anchor);
            let region = plan
                .zone_box(region_anchor)
                .ok_or_else(|| payload_anchor_failure(&label, region_anchor.anchor.as_str()))?;
            let geom = crate::nav::plan_collapse(world, blocks, region, &label)?;
            out.collapses.push(CollapseEmit {
                key,
                geom,
                falling_block: falling_block.to_string(),
                then_floor: then_floor.map(str::to_string),
            });
        }
    }
    // A trap is proven in its SPRUNG state (see `check_collapses`).
    let labelled: Vec<(String, crate::nav::CollapseGeometry)> = out
        .collapses
        .iter()
        .map(|c| (format!("collapse `{}`", c.key), c.geom.clone()))
        .collect();
    crate::nav::check_collapses(plan, world, &labelled)?;
    Ok(out)
}

/// The `DW0441` failure for a payload-verb anchor that no placed piece provides.
fn payload_anchor_failure(label: &str, anchor: &str) -> BuildFailure {
    BuildFailure::Diagnostic {
        code: DW_PAYLOAD_ANCHOR_UNRESOLVED,
        message: format!(
            "{label}: anchor `{anchor}` is not provided by any placed prefab piece, so the \
             volume it centres cannot be resolved. Use an anchor name the prefab metadata \
             actually exposes (anchor names come from prefab metadata; do NOT invent one)"
        ),
    }
}

/// Format a `Motion` component deterministically. Fixed precision, so the same
/// DSL and seed produce byte-identical NBT on every platform (ADR-0006).
fn motion_component(v: f64) -> String {
    format!("{v:.6}")
}

/// The generated functions for every `volley` (spec-0022).
///
/// One start function fans out into one function per salvo — the `sequence`
/// scheduling shape, so a volley costs **nothing per tick**: no polling, no
/// clock, just `schedule` hops the server owns. Each salvo function is:
///
/// 1. the **saturation** — one projectile per standable kill-zone cell,
///    unconditional, with the compile-time velocity that reaches that cell. This
///    is the contract: the zone is blanketed, so a player inside it is hit no
///    matter which cell they are standing in, and moving between salvos does not
///    help.
/// 2. the **aimed extra** — a second projectile toward whichever cells actually
///    hold a player this tick, selected by a plain vanilla block-volume selector
///    (`@a[x=…,dx=0,…]`). Standing still therefore costs double fire, exactly as
///    spec-0022 asks, using only compile-time velocities: no runtime vector
///    arithmetic, no scoreboard math, no folklore.
///
/// Projectiles are summoned `NoGravity` so the flown path is the straight
/// segment the coverage proof checked, and `crit:0b` so damage is deterministic
/// (a random crit bonus would make the PackTest flaky).
fn volley_fns(plan: &Plan, payloads: &PayloadPlans) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for v in &payloads.volleys {
        let base = format!("volley_{}", v.key);
        let mut start: Vec<String> = Vec::new();
        for i in 0..v.salvos {
            let at = i * v.interval;
            if at == 0 {
                start.push(format!("function {ns}:{base}_s{i}"));
            } else {
                start.push(format!("schedule function {ns}:{base}_s{i} {at}t"));
            }
        }
        out.push((base.clone(), lines(&start)));

        let src = crate::nav::volley_source(v.geom.from);
        let pos = format!(
            "{} {} {}",
            motion_component(src[0]),
            motion_component(src[1]),
            motion_component(src[2])
        );
        let mut body: Vec<String> = Vec::new();
        for shot in &v.geom.shots {
            body.push(format!(
                "summon {} {pos} {{Motion:[{}d,{}d,{}d],NoGravity:1b,crit:0b,pickup:0b}}",
                v.projectile,
                motion_component(shot.motion[0]),
                motion_component(shot.motion[1]),
                motion_component(shot.motion[2])
            ));
        }
        for shot in &v.geom.shots {
            let c = shot.cell;
            body.push(format!(
                "execute if entity @a[x={},dx=0,y={},dy=0,z={},dz=0,tag=!{CUTSCENE_TAG}] run \
                 summon {} {pos} {{Motion:[{}d,{}d,{}d],NoGravity:1b,crit:0b,pickup:0b}}",
                c[0],
                c[1],
                c[2],
                v.projectile,
                motion_component(shot.motion[0]),
                motion_component(shot.motion[1]),
                motion_component(shot.motion[2])
            ));
        }
        let salvo_body = lines(&body);
        for i in 0..v.salvos {
            out.push((format!("{base}_s{i}"), salvo_body.clone()));
        }
    }
    out
}

/// Ticks of slack allowed for debris to finish falling before `then_floor`
/// paves the landing surface. A falling block accelerates at 0.04 b/t², so this
/// is generous for any box-garden room height.
const COLLAPSE_SETTLE_SLACK: i32 = 20;

/// The generated functions for every `collapse` (spec-0022).
///
/// Summon one `falling_block` per region cell that holds a block, then delete
/// the region. `HurtEntities` gives the impact damage; the debris pile then
/// suffocates whoever it lands on — the buried-alive beat redstone cannot
/// express at all. When `then_floor` is authored, a scheduled second function
/// paves the settled surface once the rubble has landed.
fn collapse_fns(plan: &Plan, payloads: &PayloadPlans) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for c in &payloads.collapses {
        let base = format!("collapse_{}", c.key);
        let mut body: Vec<String> = Vec::new();
        for cell in &c.geom.drops {
            body.push(format!(
                "summon minecraft:falling_block {} {} {} \
                 {{BlockState:{{Name:\"{}\"}},Time:1,DropItem:0b,HurtEntities:1b,\
                 FallHurtMax:40,FallHurtAmount:2.0f}}",
                f64::from(cell[0]) + 0.5,
                cell[1],
                f64::from(cell[2]) + 0.5,
                c.falling_block
            ));
        }
        let (lo, hi) = c.geom.region;
        body.push(format!(
            "fill {} {} {} {} {} {} minecraft:air",
            lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]
        ));
        if c.then_floor.is_some() {
            let delay = c.geom.max_fall * 4 + COLLAPSE_SETTLE_SLACK;
            body.push(format!("schedule function {ns}:{base}_floor {delay}t"));
        }
        out.push((base.clone(), lines(&body)));
        if let Some(floor) = &c.then_floor {
            // Pave only the TOP cell of each debris column: the surface the
            // party walks on afterwards, which is what the completability proof
            // reasoned about.
            let mut tops: BTreeMap<[i32; 2], i32> = BTreeMap::new();
            for d in &c.geom.debris {
                let e = tops.entry([d[0], d[2]]).or_insert(d[1]);
                *e = (*e).max(d[1]);
            }
            let floor_body: Vec<String> = tops
                .into_iter()
                .map(|(col, y)| format!("setblock {} {y} {} {floor}", col[0], col[1]))
                .collect();
            out.push((format!("{base}_floor"), lines(&floor_body)));
        }
    }
    out
}

/// The guard clauses that must hold for a trap's command payload to fire: the
/// flag gate (when the trap declares one) and the disarm latch (when it has a
/// disarm affordance).
///
/// The disarm latch is load-bearing in a way it was not for a redstone trap:
/// emptying a dispenser stopped a `dispense` trap for everyone, but a command
/// payload has no ammunition to remove, so "disarmed" has to be read at fire
/// time or the affordance would be decorative.
fn trap_fire_guard(t: &plan::TrapPlan) -> String {
    let mut g = String::new();
    if trap_is_gated(t) {
        g.push_str(&format!("if score #trapgate_{} dw.sys matches 1 ", t.safe));
    }
    if t.disarm.is_some() {
        g.push_str(&format!(
            "unless score #trapdis_{} dw.sys matches 1 ",
            t.safe
        ));
    }
    g
}

/// Per-tick trigger detection for traps carrying a spec-0022 command payload.
///
/// Edge-triggered on a per-trap sentinel (`#trapfire_<safe>`), so stepping onto
/// a plate fires the payload ONCE rather than every tick the player stands
/// there. A `rearm` trap clears the sentinel when the trigger cell is vacated
/// (the plate pops back up); a `once` trap never clears it, which is exactly the
/// survivability discharge `DW0342` reasons about.
///
/// A trap with no command payload emits nothing here, so every spec-0011
/// campaign stays byte-identical.
fn trap_fire_tick(plan: &Plan) -> Vec<String> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for t in &plan.traps {
        if t.payload_effects.is_empty() {
            continue;
        }
        let id = &t.safe;
        let guard = trap_fire_guard(t);
        match t.trigger {
            delvewright_dsl::TrapTrigger::TrappedChest => {
                out.push(format!(
                    "execute unless score #trapfire_{id} dw.sys matches 1 {guard}if entity \
                     @e[tag=dw_trapfire_{id},nbt={{interaction:{{}}}}] run function \
                     {ns}:trap_fire_{id}"
                ));
                out.push(format!(
                    "execute as @e[tag=dw_trapfire_{id}] run data remove entity @s interaction"
                ));
                if matches!(t.reset, delvewright_dsl::TrapReset::Rearm) {
                    out.push(format!(
                        "execute unless entity @e[tag=dw_trapfire_{id},nbt={{interaction:{{}}}}] \
                         run scoreboard players set #trapfire_{id} dw.sys 0"
                    ));
                }
            }
            delvewright_dsl::TrapTrigger::PressurePlate
            | delvewright_dsl::TrapTrigger::Tripwire => {
                let c = t.trigger_cell;
                let at = format!(
                    "x={},dx=0,y={},dy=0,z={},dz=0,tag=!{CUTSCENE_TAG}",
                    c[0], c[1], c[2]
                );
                out.push(format!(
                    "execute unless score #trapfire_{id} dw.sys matches 1 {guard}if entity \
                     @a[{at}] run function {ns}:trap_fire_{id}"
                ));
                if matches!(t.reset, delvewright_dsl::TrapReset::Rearm) {
                    out.push(format!(
                        "execute unless entity @a[{at}] run scoreboard players set \
                         #trapfire_{id} dw.sys 0"
                    ));
                }
            }
        }
    }
    out
}

/// The `trap_fire_<id>` function per command-payload trap (spec-0022): latch the
/// sentinel, then run the authored payload bundle.
///
/// The bundle is emitted under [`Audience::Scheduled`] — there is no acting
/// player. That is the honest audience for a trap: the dungeon fires at the
/// party, not at whoever happened to touch the plate, and a `volley` salvo chain
/// re-enters under the server command source anyway (where `@s` resolves to
/// nothing). Player-facing effects therefore address `@a`, and a `carrier: "one"`
/// hand-off has no answer here — the same structural guarantee scheduled
/// sequences have.
fn trap_payload_fns(plan: &Plan) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for t in &plan.traps {
        if t.payload_effects.is_empty() {
            continue;
        }
        let mut body = vec![format!(
            "scoreboard players set #trapfire_{} dw.sys 1",
            t.safe
        )];
        body.extend(emit_effect_bundle(
            plan,
            &t.payload_effects,
            root_audience(delvewright_dsl::EffectRootKind::TrapPayload),
        ));
        out.push((format!("trap_fire_{}", t.safe), lines(&body)));
    }
    out
}

/// The campaign's **entry point**: the absolute position of the first area's
/// entry anchor, resolved through [`plan::ENTRY_ANCHOR_NAMES`] (`spawn`, then
/// `entry` — one concept, two spellings in the shipped tileset library). This one
/// cell is `setworldspawn`, the class-apply teleport, the first-join placement,
/// and the `dw:cp` seed. `None` is a hard build error (`DW0345`).
fn campaign_spawn(plan: &Plan) -> Option<[i32; 3]> {
    for area in &plan.areas {
        for name in plan::ENTRY_ANCHOR_NAMES {
            if let Some(ResolvedAnchor::Point { pos, .. }) =
                plan.anchors.get(&(area.area_id.clone(), name.to_string()))
            {
                return Some(*pos);
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// v0.6 playable-region boundary (spec-0013)
// ---------------------------------------------------------------------------

/// The effective "unbounded up" ceiling for a playable region. Well above the
/// 1.21.11 build limit (y=319); no reachable adventure-mode position in a box
/// garden exceeds it, so the vertical selector is unbounded in practice.
const REGION_CEIL_Y: i32 = 1024;

/// The compiler's default boundary return message (English-first, CLAUDE.md
/// language policy). Overridable via `boundary.message`, which is then l10n
/// inventoried under `world.boundary.message`.
const _: &str = delvewright_dsl::chrome::BOUNDARY_MESSAGE.en;

/// A soft, non-alarming cue played on a boundary return.
const BOUNDARY_SOUND: &str = "minecraft:block.amethyst_block.chime";

/// The derived playable region (spec-0013): the union of every placed-piece AABB,
/// inflated horizontally by `boundary.margin`, floored at the lowest placed block
/// − 8, unbounded upward (capped at [`REGION_CEIL_Y`] for the selector). Every
/// bound is derived from the final layout, so "every anchor is inside" is
/// structural.
struct PlayableRegion {
    /// Inclusive min corner `[x, y_floor, z]`.
    min: [i32; 3],
    /// Max corner `[x, REGION_CEIL_Y, z]`.
    max: [i32; 3],
}

impl PlayableRegion {
    /// The `@s[…]` volume-selector fragment matching a player INSIDE the region.
    /// Biased inclusive (dx/dz span the far block fully) so an edge-standing player
    /// is never falsely ejected — the safe direction, further buffered by `margin`.
    fn inside_selector(&self) -> String {
        format!(
            "[x={},dx={},y={},dy={},z={},dz={}]",
            self.min[0],
            self.max[0] - self.min[0] + 1,
            self.min[1],
            self.max[1] - self.min[1],
            self.min[2],
            self.max[2] - self.min[2] + 1,
        )
    }

    /// The SNBT compound written to `dw:region bounds` — the readable region
    /// contract (mirrors `dw:cp`'s readable last-checkpoint contract).
    fn bounds_snbt(&self) -> String {
        format!(
            "{{min:[{},{},{}],max:[{},{},{}]}}",
            self.min[0], self.min[1], self.min[2], self.max[0], self.max[1], self.max[2]
        )
    }
}

/// Derive the playable region, or `None` when no `boundary` is declared (the whole
/// feature is then off and output stays byte-identical).
fn playable_region(plan: &Plan) -> Option<PlayableRegion> {
    let b = plan.campaign.world.content.boundary.as_ref()?;
    let margin = i32::from(b.margin);
    let mut min = [i32::MAX; 3];
    let mut max = [i32::MIN; 3];
    for area in &plan.areas {
        let (amin, amax) = area.bounds();
        for a in 0..3 {
            min[a] = min[a].min(amin[a]);
            max[a] = max[a].max(amax[a]);
        }
    }
    // A validated campaign always has >=1 placed area; guard defensively.
    if min[0] == i32::MAX {
        return None;
    }
    Some(PlayableRegion {
        min: [min[0] - margin, min[1] - 8, min[2] - margin],
        max: [max[0] + margin, REGION_CEIL_Y, max[2] + margin],
    })
}

/// The effective boundary return message (authored or the English default).
fn boundary_message(plan: &Plan, chrome: &delvewright_dsl::Chrome) -> String {
    match plan
        .campaign
        .world
        .content
        .boundary
        .as_ref()
        .and_then(|b| b.message.as_deref())
    {
        // Authored: an ordinary inventoried campaign string.
        Some(m) => m.to_string(),
        // Unauthored: the compiler's own line, which is chrome and ships
        // translated with the compiler (spec-0029 addendum).
        None => chrome.get(delvewright_dsl::chrome::BOUNDARY_MESSAGE),
    }
}

/// Whether the emitted setup must initialize the `dw:cp` last-checkpoint storage
/// mirror to the spawn cell. Single shared gate so the (idempotent) init line is
/// emitted exactly once regardless of merge order: a campaign needs it when it
/// declares a `set-checkpoint` (spec-0012 — the mirror must read before the first
/// checkpoint fires) OR a `boundary` (spec-0013 — its return clock reads the
/// mirror). Absent both, non-v0.6 output stays byte-identical.
fn needs_cp_init(plan: &Plan) -> bool {
    !plan.checkpoints.is_empty() || plan.campaign.world.content.boundary.is_some()
}

/// Re-application period of the night-vision clock, in ticks (1 s).
const NIGHT_VISION_PERIOD_TICKS: u32 = 20;
/// Duration handed to each `effect give`, in **seconds**. Must stay comfortably
/// above vanilla's 10 s night-vision wind-down: `GameRenderer` ramps the night-
/// vision brightness down (the flicker) once the remaining duration drops to
/// 200 ticks, so with a 1 s clock the remaining duration never falls below
/// `12 s − 1 s = 11 s` (220 ticks) and the effect never blinks. A player who walks
/// out of a mitigated area keeps it for at most this long — deliberate: shortening
/// it below ~11 s would re-introduce the flicker, and no vanilla primitive removes
/// an effect on a region exit without also stripping effects the campaign granted
/// for other reasons.
const NIGHT_VISION_SECONDS: u32 = 12;

/// Vanilla's night-vision wind-down, in **seconds**. `GameRenderer` ramps the
/// brightness down once the remaining duration drops below 200 ticks, so an
/// effect that has less than this left is *already* visibly flickering even
/// though it has not expired.
const NIGHT_VISION_FLICKER_SECONDS: u32 = 10;

/// The lease every `effect give` hands out, in seconds.
///
/// **The camera-coverage guarantee** (owner ruling, island round 16): a vision
/// effect the compiler grants must outlast any authored camera it can overlap,
/// with vanilla's flicker window to spare.
///
/// The mitigation is declared per area and re-applied by a 1 s clock to the
/// players *inside that area's box*. A player who leaves the box keeps whatever
/// is left of their lease — and the island's ending does exactly that: boarding
/// transports the party from the mitigated island to `area/open-sea` at x=256
/// and immediately plays a 15-second cutscene. They arrived holding at most 12 s,
/// so the ramp began ~1.5 s in and the effect died mid-shot. Owner playtest:
/// "the night-vision effect expires mid-ending-cutscene and flickers."
///
/// **Why the lease, and not a re-grant at the cutscene.** Re-applying the effect
/// from the cutscene driver would light up *every* player in *every* cutscene,
/// including ones who were never granted sight and cameras the author framed as
/// bright — a spectator on a night ocean would be handed cave vision. Vanilla has
/// no "extend only if present" primitive to do it selectively. Lengthening the
/// lease changes **who** has the effect not at all; it only makes the lease a
/// leaving player already holds long enough that no camera can outlive it.
///
/// **Why the campaign's longest camera.** The compiler cannot know which cutscene
/// a player who steps out of a mitigated area will land in, so the only sound
/// bound is the longest one the campaign authors. Sized to that plus the flicker
/// window plus one clock period, so the remaining duration is still above the
/// ramp threshold when the last shot ends.
///
/// The cost is stated rather than hidden: sight trails a player out of a
/// mitigated area for this long. That is the deliberate trade the pre-existing
/// 12 s already made for the same reason (no vanilla primitive strips an effect
/// on region exit without also stripping effects the story granted); this only
/// moves the number, and only for a campaign that authors a longer camera than
/// the floor.
fn night_vision_seconds(plan: &Plan) -> u32 {
    // Measured from the ticks the camera driver really runs for
    // (`camera::shot_ticks` resolves `shot_style` defaults and applies vanilla's
    // per-shot clamp), so the bound is the emitted reality, not the authored
    // intent. Rounded up to whole seconds, which is the unit `effect give` takes.
    let longest_camera_ticks: i32 = all_campaign_effects(plan.campaign)
        .into_iter()
        .filter_map(|e| e.cutscene_shots())
        .map(|shots| {
            shots
                .iter()
                .map(|s| crate::camera::shot_ticks(s.resolved_seconds()))
                .sum::<i32>()
        })
        .max()
        .unwrap_or(0);
    let longest_camera = (longest_camera_ticks.max(0) as u32).div_ceil(20);
    NIGHT_VISION_SECONDS
        .max(longest_camera + NIGHT_VISION_FLICKER_SECONDS + NIGHT_VISION_PERIOD_TICKS.div_ceil(20))
}

/// The v0.6 night-vision mitigation clock: for every area declaring
/// `mitigation: "night-vision"`, a self-rescheduling 1 s function that gives
/// `minecraft:night_vision` to the players inside **that area's placed bounds**.
///
/// This is the mechanism the `DW0210` gate now keys on (`light::area_night_vision`).
/// Before v0.6 the gate keyed on a class-kit item's display *name*, which a renamed
/// water bottle satisfied — the check passed while nothing granted night vision
/// (owner, island QA). Declaration and emission are now the same fact.
///
/// The selector box is the area's final placed bounds — compile-time literals, no
/// runtime search — so emission is deterministic. Empty for a campaign that declares
/// no mitigation, keeping pre-0.6 output byte-identical.
fn night_vision_fns(plan: &Plan) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let seconds = night_vision_seconds(plan);
    let mut gives: Vec<String> = Vec::new();
    for area in &plan.areas {
        let declared = plan
            .campaign
            .world
            .content
            .areas
            .iter()
            .find(|a| a.id.as_str() == area.area_id)
            .is_some_and(crate::light::area_night_vision);
        if !declared {
            continue;
        }
        let (min, max) = area.bounds();
        // An area's `bounds()` are inclusive corners, so the selector span is
        // `max - min + 1` — a placed area is a count of cells, where a declared
        // volume (`box_selector_args`) is a span between two corners. The two are
        // one block apart on purpose and this is the only place the difference
        // lives.
        let span = [
            max[0] - min[0] + 1,
            max[1] - min[1] + 1,
            max[2] - min[2] + 1,
        ];
        let sel = format!(
            "@a[{}]",
            box_selector_args(min, [min[0] + span[0], min[1] + span[1], min[2] + span[2]])
        );
        gives.push(effect_give_command(
            &sel,
            "minecraft:night_vision",
            seconds,
            0,
            true,
        ));
    }
    if gives.is_empty() {
        return Vec::new();
    }
    // `schedule … <n>t` uses vanilla replace-mode, so the clock can never double up.
    gives.push(format!(
        "schedule function {ns}:night_vision_tick {NIGHT_VISION_PERIOD_TICKS}t"
    ));
    vec![("night_vision_tick".to_string(), lines(&gives))]
}

/// Whether the campaign declares the night-vision mitigation on any area.
fn has_night_vision_areas(plan: &Plan) -> bool {
    plan.campaign
        .world
        .content
        .areas
        .iter()
        .any(crate::light::area_night_vision)
}

/// The v0.6 boundary clock (spec-0013): a self-rescheduling 1s (20t) region check
/// plus a per-player macro return. Empty for a campaign with no `boundary`. The
/// return teleports via `dw:cp` (the last checkpoint), so wanderers always land on
/// the current respawn anchor rather than a fixed point.
fn boundary_fns(plan: &Plan, chrome: &delvewright_dsl::Chrome) -> Vec<(String, String)> {
    let Some(region) = playable_region(plan) else {
        return Vec::new();
    };
    let ns = &plan.namespace;
    let sel = region.inside_selector();
    let msg = tr(&boundary_message(plan, chrome));

    // boundary_tick: snapshot the live checkpoint into a scratch compound, eject
    // every player outside the region to it, re-arm the clock. `schedule … 20t`
    // uses vanilla replace-mode, so the clock can never double up.
    let tick = vec![
        "data modify storage dw:region cp.x set from storage dw:cp pos[0]".to_string(),
        "data modify storage dw:region cp.y set from storage dw:cp pos[1]".to_string(),
        "data modify storage dw:region cp.z set from storage dw:cp pos[2]".to_string(),
        format!(
            "execute as @a unless entity @s{sel} run function {ns}:boundary_return with storage dw:region cp"
        ),
        format!("schedule function {ns}:boundary_tick 20t"),
    ];

    // boundary_return: a macro run per offending player (`@s`). Teleport to the
    // checkpoint, show the message on the actionbar, play a soft cue. No damage.
    let ret = vec![
        "$tp @s $(x) $(y) $(z)".to_string(),
        format!("title @s actionbar {msg}"),
        format!("playsound {BOUNDARY_SOUND} player @s ~ ~ ~ 0.6 1"),
    ];

    vec![
        ("boundary_tick".to_string(), lines(&tick)),
        ("boundary_return".to_string(), lines(&ret)),
    ]
}

/// Objective id → function-name-safe token (`obj/talk` → `o_talk`).
fn safe_obj_fn(obj_id: &str) -> String {
    format!("o_{}", plan::safe_local(obj_id))
}

/// One `dw.sys` score, as a chat component.
fn sys_score(holder: &str) -> Value {
    json!({ "score": { "name": holder, "objective": "dw.sys" } })
}

/// Join a marker's prefix and its integer fields into one `tellraw` component,
/// rendering as a single anchored line the harness parses whole.
///
/// The grammar is the completion channel's, one token further on:
/// `[dw:<token> <campaign> <wave> <n> <n> …]`. It inherits the same three
/// unforgeability properties — player chat cannot begin with the sigil, the
/// campaign id is part of the match, and `DW0182` reserves the sigil in every
/// player-visible string — so a census line is as much an oracle as a completion
/// marker is.
fn census_component(ns: &str, token: &str, wave_id: &str, holders: &[&str]) -> Value {
    let mut extra: Vec<Value> = Vec::new();
    for h in holders {
        extra.push(sys_score(h));
        extra.push(json!({ "text": " " }));
    }
    // The trailing separator becomes the closing bracket.
    extra.pop();
    extra.push(json!({ "text": "]" }));
    json!({
        "text": format!("[dw:{token} {ns} {wave_id} "),
        "color": "dark_gray",
        "extra": extra
    })
}

/// The census SUMMARY line: sequence, how many of the wave stand, how many of
/// those are branded (fought in a previous life), how many are below full health.
fn census_summary_component(ns: &str, wave_id: &str) -> Value {
    census_component(
        ns,
        plan::MARKER_TOKEN_CENSUS,
        wave_id,
        &["#wcen_seq", "#wcen_n", "#wcen_b", "#wcen_d"],
    )
}

/// One mob's line inside a census: sequence, position and health, all ×100 so
/// they cross the chat channel as exact integers.
fn census_mob_component(ns: &str, wave_id: &str) -> Value {
    census_component(
        ns,
        plan::MARKER_TOKEN_CENSUS_MOB,
        wave_id,
        &[
            "#wcen_seq",
            "#wcen_x",
            "#wcen_y",
            "#wcen_z",
            "#wcen_h",
            "#wcen_m",
        ],
    )
}

/// Per-objective "already announced" scoreboard (v0.3 objective-activation
/// feedback, M2 fix 4). Set once the objective's title/hint has been shown so the
/// announce fires exactly once per player.
fn announce_score(obj_id: &str) -> String {
    format!("dw.ann_{}", plan::safe_local(obj_id))
}

/// The entity tag on an `interact` objective's interaction hitbox.
fn interact_entity_tag(obj_id: &str) -> String {
    format!("dw_i_{}", plan::safe_local(obj_id))
}

/// The entity tag on a `reach-anchor` objective's visual marker display.
fn reach_marker_tag(obj_id: &str) -> String {
    format!("dw_r_{}", plan::safe_local(obj_id))
}

/// Scoreboard (dummy, `dw.sys`) holder for the "already holding the item" per-tick
/// collect completion check (gap 13).
const COLLECT_HOLD: &str = "dw.hold";

/// The `dw.sys` fake player holding the live online-player count, recomputed each
/// tick — the lobby gate's only input (spec-0018 `world.min_players`). Emitted
/// only for a campaign that declares `min_players >= 2`.
const LOBBY_COUNT: &str = "#lobby";

/// The lobby's waiting message (spec-0018): a live "x / n" actionbar for players
/// who have not taken a class yet, while the party is short. The count is a
/// vanilla `score` component reading [`LOBBY_COUNT`], so it updates itself
/// without any per-count emission. English-first (CLAUDE.md language policy); a
/// compiler default, not an authored string, so it is not l10n-inventoried.
fn lobby_actionbar(min_players: u8, chrome: &delvewright_dsl::Chrome) -> String {
    // One sentence with two `with` arguments — the live count and the size the
    // delve requires — so a translation orders them for its own language instead
    // of inheriting English's `<prefix> <n> / <n>`.
    tr_with(
        &chrome.get(delvewright_dsl::chrome::LOBBY_WAITING),
        &[
            ("color", json!("yellow")),
            (
                "with",
                json!([
                    { "score": { "name": LOBBY_COUNT, "objective": "dw.sys" }, "color": "gold" },
                    { "text": min_players.to_string(), "color": "gold" }
                ]),
            ),
        ],
    )
    .to_string()
}

/// The fake-player sentinel on `dw.sys` that guards an objective's activation
/// placement so it runs exactly once, world-wide (gap 13).
fn activation_flag(obj_id: &str) -> String {
    format!("#act_{}", plan::safe_local(obj_id))
}

/// Whether the campaign declares any `collect` objective (gates the `dw.hold`
/// scratch declaration so campaigns without collect stay byte-identical).
fn has_collect_objective(c: &delvewright_dsl::Campaign) -> bool {
    c.quests
        .content
        .quests
        .iter()
        .flat_map(|q| &q.objectives)
        .any(|o| matches!(o, Objective::Collect { .. }))
}

/// The world-placement commands run when an objective ACTIVATES (gap 13): a
/// `collect` chest + item fill, an `interact` hitbox + glowing lantern marker, or a
/// `reach` glowing end-rod marker. Empty for objectives with no prop (talk-to,
/// kill) or an unresolvable anchor — both the `tick` activation driver and the
/// `activate_<obj>` function key off this being non-empty, so they never diverge.
fn activation_commands(plan: &Plan, area: &str, o: &Objective) -> Vec<String> {
    let mut cmds = Vec::new();
    match o {
        Objective::Collect {
            id,
            item,
            count,
            anchor,
            item_name,
            fill_count,
            dropped_by,
            ..
        } => {
            // v0.9: a drop-gated collect is provisioned by the fight,
            // not by the world. Place nothing and fill nothing — the item exists
            // only once the boss dies, which is exactly what makes the chain
            // provable (`DW0493`) instead of merely intended.
            if dropped_by.is_some() {
                return cmds;
            }
            // v0.8: an ADOPTED container is prefab furniture standing
            // in the room already — fill it where it stands, place nothing. Absent
            // `container`, the compiler keeps conjuring its own chest at the
            // anchor exactly as it always has. The adopted cell comes from
            // `plan.collect_fills`, the same resolution `DW0438` proved.
            let adopted = plan
                .collect_fills
                .iter()
                .find(|f| f.objective_id == id.as_str())
                .map(|f| f.cell);
            let Some(pos) = adopted.or_else(|| plan.point(area, anchor.as_str())) else {
                return cmds;
            };
            if adopted.is_none() {
                cmds.push(format!(
                    "setblock {} {} {} minecraft:chest",
                    pos[0], pos[1], pos[2]
                ));
            }
            // The objective's own stack lands in `container.0`; each padding stack
            // repeats it in the slots after it, so the container READS full
            // (vanilla fullness is occupied slots, not stack size). Positional and
            // total — no RNG, nothing to reseed (ADR-0006). A campaign with
            // neither a name nor padding emits the single pre-0.8 line, byte for
            // byte.
            let stack = format!(
                "{item}{} {count}",
                item_component_tail(item_name.as_deref())
            );
            for slot in 0..=*fill_count {
                cmds.push(format!(
                    "item replace block {} {} {} container.{slot} with {stack}",
                    pos[0], pos[1], pos[2]
                ));
            }
        }
        Objective::Interact { id, anchor, .. } => {
            if let Some(pos) = plan.point(area, anchor.as_str()) {
                let e = ent_xyz(pos);
                cmds.push(format!(
                    "summon minecraft:interaction {} {} {} {{width:1.0f,height:2.0f,response:1b,Invulnerable:1b,Tags:[{FIXTURE_NBT}\"{}\"]}}",
                    e[0], e[1], e[2], interact_entity_tag(id.as_str())
                ));
                if let Some(prop) = o.prop() {
                    // v0.4: the prop block IS the affordance (spec-0008 §2) — place
                    // it at the anchor. No hologram marker: the block is visible.
                    cmds.push(format!(
                        "setblock {} {} {} {}",
                        pos[0], pos[1], pos[2], prop.block
                    ));
                } else {
                    // Visible, glowing, adventure-safe marker so a human can find the
                    // interact target (M2 fix 3): an `item_display` has no collision,
                    // so it obstructs neither movement nor the interaction hitbox.
                    // Named from the objective `title`; an untitled objective gets a
                    // nameless (but still glowing) marker rather than a raw-id label.
                    let name_fields = marker_name_fields(o.title());
                    cmds.push(format!(
                        "summon minecraft:item_display {} {} {} {{Glowing:1b,Tags:[{FIXTURE_NBT}\"dw_marker\",\"{}\"],{}billboard:\"center\",item:{{id:\"minecraft:lantern\",count:1}}}}",
                        e[0], e[1], e[2], interact_entity_tag(id.as_str()), name_fields
                    ));
                }
            }
        }
        Objective::ReachAnchor { id, anchor, .. } => {
            let pos = match plan
                .anchors
                .get(&(area.to_string(), anchor.as_str().to_string()))
            {
                Some(ResolvedAnchor::Point { pos, .. }) => *pos,
                Some(ResolvedAnchor::Gate { from, .. }) => *from,
                None => return cmds,
            };
            // A distinct, thematically neutral `end_rod` (vs. the interact lantern)
            // so a beacon-like light marks a reach destination. Named from the
            // objective `title`; untitled → nameless glow, never a raw-id label.
            let name_fields = marker_name_fields(o.title());
            let e = ent_xyz(pos);
            cmds.push(format!(
                "summon minecraft:item_display {} {} {} {{Glowing:1b,Tags:[{FIXTURE_NBT}\"dw_marker\",\"{}\"],{}billboard:\"center\",item:{{id:\"minecraft:end_rod\",count:1}}}}",
                e[0], e[1], e[2], reach_marker_tag(id.as_str()), name_fields
            ));
        }
        Objective::TalkTo { .. } | Objective::Kill { .. } => {}
    }
    cmds
}

/// The despawn commands run when an objective COMPLETES: remove every
/// entity its [`activation_commands`] summoned. The objective-scoped tag
/// (`dw_i_<obj>` on an interact's hitbox and its wayfinding marker, `dw_r_<obj>` on
/// a reach marker) is deterministic and unique to the objective, so a single tight
/// `kill @e[tag=…]` covers all of them without touching players (players never
/// carry these tags) or any other objective's markers. Interact-with-prop summons
/// only the hitbox (the prop is a block, not tagged); interact-without-prop and
/// reach also summon a `dw_marker` item_display carrying the same objective tag.
/// Prop BLOCKS and collect chests are the affordance itself and intentionally
/// persist as scenery — they are not entities and are not killed here. `collect`
/// (chest block only), `talk-to` and `kill` summon no per-objective entity, so they
/// contribute nothing.
fn completion_cleanup(o: &Objective) -> Vec<String> {
    match o {
        Objective::Interact { id, .. } => {
            vec![format!("kill @e[tag={}]", interact_entity_tag(id.as_str()))]
        }
        Objective::ReachAnchor { id, .. } => {
            vec![format!("kill @e[tag={}]", reach_marker_tag(id.as_str()))]
        }
        Objective::Collect { .. } | Objective::TalkTo { .. } | Objective::Kill { .. } => Vec::new(),
    }
}

/// The flags any `set-flag` effect produces (sorted, deduped) — quest effects,
/// plus (DSL v0.4) dialogue `set-flag` effects and environment-trigger effects.
/// Empty extra sources for v0.2/v0.3, keeping their scoreboard setup identical.
///
/// **This is emission, not a lint**. A `set-flag` whose `dw.f_<flag>`
/// objective is missing from `setup` writes to nothing: vanilla answers
/// `scoreboard players set … <undeclared> 1` with a command error and carries on,
/// so there is no crash, nothing a bot observes, and every gate on that flag
/// simply never opens. It is the `DW0497` shape — a call with no callee —
/// reproduced one layer down, at the scoreboard.
///
/// The roots therefore come from [`crate::plan::for_each_effect_root`], the one
/// enumeration [`all_campaign_effects`] itself walks, so the declaration walk and
/// the write walk cannot disagree about where a `set-flag` may live. This
/// inventory used to hand-list three of the five, and a `set-flag` in a
/// `traps[].payload` or a dialogue option's `set-checkpoint` `on_respawn` bundle
/// emitted its write against an objective nothing had created.
///
/// Depth was never the blind spot — `visit_deep` already descended `sequence`
/// steps and lifecycle bundles — so the fix is which lists the descent starts
/// from, and it is inherited rather than re-listed.
///
/// The sources below the walk are the ones that are **not** effect roots and so
/// cannot come from it: a trap's and a timed gate's `disarm.sets_flag`, the flat
/// `DialogueEffect::SetFlag` list (a `DialogueEffect` is not a `QuestEffect`), and
/// the cast ledger's flag *reads*.
fn declared_flags(c: &delvewright_dsl::Campaign) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    crate::plan::for_each_effect_root(c, &mut |_site, effs| {
        for eff in effs {
            eff.visit_deep(&mut |e| {
                if let Some(f) = e.set_flag() {
                    out.insert(f.as_str().to_string());
                }
            });
        }
    });
    // v0.6 traps (spec-0011): a disarm's `sets_flag` needs its own scoreboard.
    for t in &c.quests.content.traps {
        if let Some(dis) = &t.disarm {
            out.insert(dis.sets_flag.as_str().to_string());
        }
    }
    // A timed gate's disarm sets a flag exactly as a trap's does.
    for g in &c.quests.content.timed_gates {
        if let Some(dis) = &g.disarm {
            out.insert(dis.sets_flag.as_str().to_string());
        }
    }
    for tree in &c.dialogue.content.dialogues {
        for node in &tree.nodes {
            for opt in &node.options {
                for eff in &opt.effects {
                    if let Some(f) = eff.set_flag() {
                        out.insert(f.as_str().to_string());
                    }
                }
            }
        }
    }
    // v0.7 cast ledger (spec-0020): a per-branch cast READS its branch flags in
    // the scene selector. Reading an objective that was never declared is a
    // runtime command error, and unlike a `set-flag` write there is nothing
    // elsewhere that guarantees the declaration — a branch may legitimately be
    // gated on a flag some *other* campaign path sets. Declared here so the read
    // is always well-formed.
    for q in &c.quests.content.quests {
        for entry in q.cast.values() {
            for p in entry.placements() {
                for f in p.requires_flags.iter().chain(&p.forbids_flags) {
                    out.insert(f.as_str().to_string());
                }
            }
        }
    }
    out
}

/// `(objective id, quest id)` for every `interact` objective, in declared order.
fn interact_objectives(c: &delvewright_dsl::Campaign) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for q in &c.quests.content.quests {
        for o in &q.objectives {
            if matches!(o, Objective::Interact { .. }) {
                out.push((o.id().as_str().to_string(), q.id.as_str().to_string()));
            }
        }
    }
    out
}

/// The intra-quest activation + pending guard for an objective (v0.3): the quest
/// must be active, every `after` prerequisite and `requires_flags` flag set, no
/// `forbids_flags` flag set (v0.6 negative gate; `unless … matches 1` so an
/// unset score counts as "not set"), and the objective itself not yet complete.
/// Returns the ` if …`/` unless …` fragment (leading space); callers append the
/// type-specific condition + `run`, prepending `execute as @a` when they need a
/// player to test (proximity, inventory, a fired trigger) and a bare `execute`
/// otherwise.
///
/// **Every term reads the party holder** (spec-0018). That is what makes an
/// `after: [obj/a, obj/b]` AND-join a division of labor: player A clearing
/// `obj/a` in one room and player B clearing `obj/b` in another both write
/// `#party`, so the successor's guard opens for the whole party. It is also what
/// keeps the drivers single-fire under `as @a`: vanilla evaluates the conditions
/// per selected player in turn, so the first player's `run` sets the party score
/// and every later player's `unless score #party …` fails in the same tick.
/// Stage-5 quests in **arming order**: a quest whose completion arms another is
/// visited before the quest it arms.
///
/// The arming graph is exactly the `Trigger::QuestComplete` edges — the only two
/// trigger kinds are `CampaignStart` (a root, armed by `setup`) and
/// `QuestComplete`, so this is the whole of it.
///
/// **Stable.** Declaration order breaks every tie and seeds the ready set, so a
/// campaign already declared in arming order — which every campaign built so far
/// is — comes back in exactly its declared order and emits byte-identically. It
/// is also **total**: a cycle (already an error elsewhere; a quest cannot arm
/// itself through any path and still be reachable) leaves an unresolved tail,
/// which is appended in declaration order rather than dropped, because a lost
/// quest would be a far worse failure than a badly-ordered one.
fn quests_in_arming_order(c: &delvewright_dsl::Campaign) -> Vec<&delvewright_dsl::Quest> {
    let quests = &c.quests.content.quests;
    let index: BTreeMap<&str, usize> = quests
        .iter()
        .enumerate()
        .map(|(i, q)| (q.id.as_str(), i))
        .collect();
    let mut indegree = vec![0usize; quests.len()];
    let mut arms: Vec<Vec<usize>> = vec![Vec::new(); quests.len()];
    for (i, q) in quests.iter().enumerate() {
        if let Trigger::QuestComplete { quest } = &q.trigger
            && let Some(&p) = index.get(quest.as_str())
            && p != i
        {
            indegree[i] += 1;
            arms[p].push(i);
        }
    }
    // Kahn's algorithm with a declaration-ordered ready queue: among quests that
    // become ready together, the earliest-declared is always emitted first.
    let mut ready: Vec<usize> = (0..quests.len()).filter(|&i| indegree[i] == 0).collect();
    let mut order: Vec<usize> = Vec::with_capacity(quests.len());
    while !ready.is_empty() {
        let i = ready.remove(0);
        order.push(i);
        for &dep in &arms[i] {
            indegree[dep] -= 1;
            if indegree[dep] == 0 {
                let pos = ready.partition_point(|&r| r < dep);
                ready.insert(pos, dep);
            }
        }
    }
    let mut seen = vec![false; quests.len()];
    let mut out: Vec<&delvewright_dsl::Quest> = Vec::with_capacity(quests.len());
    for i in order {
        seen[i] = true;
        out.push(&quests[i]);
    }
    for (i, q) in quests.iter().enumerate() {
        if !seen[i] {
            out.push(q);
        }
    }
    out
}

fn pending_guard(plan: &Plan, o: &Objective, quest_active: &str) -> String {
    let p = plan::PARTY;
    let mut g = format!(" if score {p} {quest_active} matches 1");
    for a in o.after() {
        g.push_str(&format!(
            " if score {p} {} matches 1",
            obj_score(a.as_str())
        ));
    }
    // The objective's whole gate, in gate field order: required flags, forbidden
    // flags, then (DSL v0.10) the numeric terms. Written through `gate_cond` so
    // this guard cannot end up knowing about two of the gate's three fields —
    // which is exactly how the numeric axis would have been missed. Empty terms
    // contribute nothing, so a pre-0.10 campaign's guard is byte-identical.
    g.push_str(&gate_cond(plan, o.gate()));
    g.push_str(&format!(
        " unless score {p} {} matches 1",
        obj_score(o.id().as_str())
    ));
    g
}

// ---------------------------------------------------------------------------
// dialogs / advancements
// ---------------------------------------------------------------------------

/// Whether an option's display is gated (DSL v0.4+): it requires flags (flag
/// axis), forbids flags (v0.6 negative flag axis), or completes an objective
/// (objective-state axis — visible only while that objective is active). Below
/// v0.4 nothing is display-gated, so v0.2/v0.3 nodes stay byte-identical.
/// `requires_flags` is itself a v0.4 verb (`forbids_flags` v0.6), so the whole
/// predicate collapses to `false` pre-v0.4.
fn option_display_gated(opt: &plan::OptionPlan, v04: bool) -> bool {
    v04 && (!opt.requires_flags.is_empty()
        || !opt.forbids_flags.is_empty()
        // v0.10 (spec-0031): a numeric gate hides the option exactly as a flag
        // gate does — an option the player cannot pick must not be drawn.
        || !opt.requires_state.is_empty()
        || !opt.completes.is_empty())
}

/// The display-gated options of `node_id`, in declared order — the bit order of
/// the node's per-player availability mask (`dw.dmask`). Empty for an ungated
/// node (v0.2/v0.3, or a v0.4 node whose every option is unconditional).
fn node_gated_options<'a>(
    npc: &'a plan::NpcPlan,
    node_id: &str,
    v04: bool,
) -> Vec<&'a plan::OptionPlan> {
    npc.options
        .iter()
        .filter(|o| o.node_id == node_id && option_display_gated(o, v04))
        .collect()
}

/// The ` if …`/` unless …` execute fragment (leading space) that is satisfied
/// exactly when `opt` should be DISPLAYED: every `requires_flags` flag set (flag
/// axis), and — v0.4+ — every completed objective's quest active and the
/// objective itself not yet complete (objective-state axis). Mirrors the
/// click-handler guard (emit.rs ~1166) so an option is shown iff clicking it
/// would fire.
fn option_display_conditions(
    plan: &Plan,
    c: &delvewright_dsl::Campaign,
    opt: &plan::OptionPlan,
) -> String {
    let p = plan::PARTY;
    let mut cond = String::new();
    for f in &opt.requires_flags {
        cond.push_str(&format!(" if score {p} {} matches 1", plan::flag_score(f)));
    }
    // v0.6 negative gate: hidden once any forbidden flag is set (`unless …
    // matches 1` treats an unset score as "not set").
    for f in &opt.forbids_flags {
        cond.push_str(&format!(
            " unless score {p} {} matches 1",
            plan::flag_score(f)
        ));
    }
    // DSL v0.10 (spec-0031). A dialogue option's availability is computed PER
    // PLAYER (`dw.dmask`, run `as @s`), so this is the one gate site a
    // `player`-scoped datum reads from `@s` rather than from the party holder.
    cond.push_str(&state_cond(plan, &opt.requires_state, false));
    for obj in &opt.completes {
        if let Some((qid, _)) = objective_quest(c, obj) {
            cond.push_str(&format!(
                " if score {p} {} matches 1 unless score {p} {} matches 1",
                quest_active_score(qid),
                obj_score(obj)
            ));
        }
    }
    cond
}

/// The command that displays `node_id`: a direct `dialog show` for an ungated
/// node, or the availability chooser function for a gated one (which shows the
/// variant matching the player's satisfied flags + active objectives).
fn show_node_cmd(plan: &Plan, npc: &plan::NpcPlan, node_id: &str) -> String {
    let ns = &plan.namespace;
    let v04 = campaign_is_v04(plan);
    let node_safe = plan::safe_local(node_id);
    if node_gated_options(npc, node_id, v04).is_empty() {
        format!("dialog show @s {ns}:{}_{}", npc.safe, node_safe)
    } else {
        format!("function {ns}:show_{}_{}", npc.safe, node_safe)
    }
}

/// Availability chooser + mask functions for this NPC's display-gated nodes. Per
/// gated node, two functions:
///
/// * `dmask_<npc>_<node>` computes the per-player availability bitmask into
///   `dw.dmask` — bit `i` set iff the node's `i`-th gated option is currently
///   displayable (flags satisfied and every completed objective active + not yet
///   complete). Pure scoreboard math, so a PackTest can drive it and assert the
///   mask without opening a dialog.
/// * `show_<npc>_<node>` runs the mask function, then `dialog show`s the variant
///   (`<npc>_<node>__m<mask>`) whose visible options match.
fn gated_node_choosers(plan: &Plan, npc: &plan::NpcPlan) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let v04 = campaign_is_v04(plan);
    let mut out = Vec::new();
    let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for opt in &npc.options {
        if !seen.insert(opt.node_id.as_str()) {
            continue;
        }
        let gated = node_gated_options(npc, &opt.node_id, v04);
        if gated.is_empty() {
            continue;
        }
        let node_safe = plan::safe_local(&opt.node_id);

        let mut dmask = vec!["scoreboard players set @s dw.dmask 0".to_string()];
        for (i, g) in gated.iter().enumerate() {
            dmask.push(format!(
                "execute{} run scoreboard players add @s dw.dmask {}",
                option_display_conditions(plan, c, g),
                1u32 << i
            ));
        }
        out.push((format!("dmask_{}_{}", npc.safe, node_safe), lines(&dmask)));

        let mut show = vec![format!("function {ns}:dmask_{}_{}", npc.safe, node_safe)];
        for mask in 0..(1u32 << gated.len()) {
            show.push(format!(
                "execute if score @s dw.dmask matches {mask} run dialog show @s {ns}:{}_{}__m{mask}",
                npc.safe, node_safe
            ));
        }
        out.push((format!("show_{}_{}", npc.safe, node_safe), lines(&show)));
    }
    out
}

/// Whether any dialogue option is display-gated (gates the `dw.dmask`
/// declaration): a v0.4+ option that requires flags or completes an objective.
fn has_gated_dialogue(c: &delvewright_dsl::Campaign) -> bool {
    use delvewright_dsl::DialogueEffect;
    is_v04(c.quests.dsl_version.as_str())
        && c.dialogue
            .content
            .dialogues
            .iter()
            .flat_map(|t| &t.nodes)
            .flat_map(|n| &n.options)
            .any(|o| {
                !o.requires_flags.is_empty()
                    || !o.forbids_flags.is_empty()
                    || o.effects
                        .iter()
                        .any(|e| matches!(e, DialogueEffect::CompleteObjective { .. }))
            })
}

/// The per-player scene selector the cast ledger dispatches on (spec-0020).
const CAST_SCORE: &str = "dw.cast";

/// The right-click body for one NPC: the cast ledger's scene dispatch, or — for
/// an NPC no quest casts — the single `show_node_cmd(root)` line that has always
/// been there (so a campaign with no ledger is byte-identical).
///
/// ## Why re-evaluated rather than latched
///
/// `dw.qa_<quest>` is set to 1 when a quest starts and is never cleared, so it
/// reads "has begun". Emitting the selector clauses in quest-DAG order therefore
/// makes the **latest begun** quest win, and keep winning: the scene advances
/// with the story and never falls back. That is the whole retirement mechanism —
/// after the escape beat opens, Perimedes's right-click resolves to the escape
/// scene's root, and the premise root is unreachable *because the ledger says so*,
/// not because an author remembered a flag.
///
/// Scene `0` is "no declaring quest has begun yet" and shows the stage-6 root.
/// A `"none"` scene emits no action clause at all: the interaction advancement is
/// still granted and revoked one line above (the record is written and consumed),
/// and nothing opens.
fn cast_dispatch(
    plan: &Plan,
    npc: &plan::NpcPlan,
    casts: &std::collections::BTreeMap<String, crate::cast::NpcCast>,
) -> Vec<String> {
    use crate::cast::SceneAction;
    let ns = &plan.namespace;
    let Some(cast) = casts.get(&npc.npc_id) else {
        return vec![show_node_cmd(plan, npc, &npc.root)];
    };
    let mut out = vec![format!("function {ns}:cast_{}", npc.safe)];
    out.push(format!(
        "execute if score @s {CAST_SCORE} matches 0 run {}",
        show_node_cmd(plan, npc, &npc.root)
    ));
    for scene in &cast.scenes {
        let i = scene.index;
        match &scene.action {
            SceneAction::Root(root) => out.push(format!(
                "execute if score @s {CAST_SCORE} matches {i} run {}",
                show_node_cmd(plan, npc, root)
            )),
            SceneAction::Barks(_) => out.push(format!(
                "execute if score @s {CAST_SCORE} matches {i} run function {ns}:bark_{}_{i}",
                npc.safe
            )),
            // Declared silence: no clause. The click is still recorded and
            // consumed by the `advancement revoke` above.
            SceneAction::Silent => {}
        }
    }
    out
}

/// The `cast_<npc>` selector function: compute which scene governs right now into
/// the per-player `dw.cast`.
///
/// Split out of `talk_<npc>` for the same reason `dmask_<npc>_<node>` is split out
/// of `show_<npc>_<node>`: it is pure scoreboard math, so a PackTest can drive it
/// and assert which scene the ledger selected **without opening a dialog** (a
/// PackTest dummy has no client to show a screen to).
fn cast_selector_fn(
    plan: &Plan,
    npc: &plan::NpcPlan,
    casts: &std::collections::BTreeMap<String, crate::cast::NpcCast>,
) -> Option<(String, String)> {
    let cast = casts.get(&npc.npc_id)?;
    let mut body = vec![format!("scoreboard players set @s {CAST_SCORE} 0")];
    for cl in &cast.by_quest {
        // Per-branch casts add their branch gate to the same clause, so a
        // branch-divergent NPC really does dispatch per branch. Flags are party
        // state (`#party`), matching every other flag read in the dialogue path.
        let mut gate = String::new();
        for f in &cl.requires_flags {
            gate.push_str(&format!(
                " if score {} {} matches 1",
                plan::PARTY,
                plan::flag_score(f)
            ));
        }
        for f in &cl.forbids_flags {
            gate.push_str(&format!(
                " unless score {} {} matches 1",
                plan::PARTY,
                plan::flag_score(f)
            ));
        }
        // DSL v0.10 (spec-0031): the placement's numeric terms. The cast selector
        // is per-player (`dw.cast` on `@s`), so a `player`-scoped datum is legal
        // here too.
        gate.push_str(&state_cond(plan, &cl.requires_state, false));
        body.push(format!(
            "execute if score {} {} matches 1{gate} run scoreboard players set @s {CAST_SCORE} {}",
            plan::PARTY,
            quest_active_score(&cl.quest),
            cl.scene
        ));
    }
    Some((format!("cast_{}", npc.safe), lines(&body)))
}

/// One `bark_<npc>_<scene>` function per bark-pool scene: speak the next line and
/// advance the pool.
///
/// The counter is a `#bk_<npc>_<scene>` fake player on the shared `dw.sys`
/// objective — the repo's existing per-entity counter idiom — and it cycles by an
/// explicit clause ladder, so there is no RNG anywhere near a delve and the
/// n-th right-click always yields the same line.
fn cast_bark_fns(
    plan: &Plan,
    npc: &plan::NpcPlan,
    casts: &std::collections::BTreeMap<String, crate::cast::NpcCast>,
) -> Vec<(String, String)> {
    use crate::cast::SceneAction;
    let mut out = Vec::new();
    let Some(cast) = casts.get(&npc.npc_id) else {
        return out;
    };
    let name = plan
        .campaign
        .npcs
        .content
        .npcs
        .iter()
        .find(|n| n.id.as_str() == npc.npc_id)
        .map(|n| n.name.clone())
        .unwrap_or_default();
    for scene in &cast.scenes {
        let SceneAction::Barks(pool) = &scene.action else {
            continue;
        };
        let holder = format!("#bk_{}_{}", npc.safe, scene.index);
        let mut body = vec![
            format!("scoreboard players add {holder} dw.sys 1"),
            format!(
                "execute if score {holder} dw.sys matches {}.. run scoreboard players set {holder} dw.sys 1",
                pool.len() + 1
            ),
        ];
        for (i, line) in pool.iter().enumerate() {
            let comp = json!([
                tr_with(&name, &[("color", json!("yellow"))]),
                { "text": ": " },
                tr_with(line, &[("italic", json!(true))])
            ]);
            body.push(format!(
                "execute if score {holder} dw.sys matches {} run tellraw @s {comp}",
                i + 1
            ));
        }
        out.push((format!("bark_{}_{}", npc.safe, scene.index), lines(&body)));
    }
    out
}

fn emit_dialogs(plan: &Plan, chrome: &delvewright_dsl::Chrome) -> Vec<(String, Value)> {
    let c = plan.campaign;
    let v04 = campaign_is_v04(plan);
    let mut dialogs = Vec::new();

    // class selection
    let actions: Vec<Value> = plan
        .classes
        .iter()
        .zip(&c.classes.content.classes)
        .map(|(cp, class)| {
            json!({
                "label": tr(&class.name),
                "tooltip": tr(&class.blurb),
                "action": { "type": "minecraft:run_command", "command": format!("/trigger dw.class set {}", cp.n) }
            })
        })
        .collect();
    dialogs.push((
        "class_select".to_string(),
        json!({
            "type": "minecraft:multi_action",
            "title": tr(&chrome.get(delvewright_dsl::chrome::CLASS_TITLE)),
            "body": [{ "type": "minecraft:plain_message",
                       "contents": tr(&chrome.get(delvewright_dsl::chrome::CLASS_BODY)) }],
            "columns": 1,
            "can_close_with_escape": false,
            "after_action": "close",
            "actions": actions
        }),
    ));

    // spec-0016 §1: one bonfire dialog per bonfire,
    // offering EXACTLY two options — rest and save, or save only. Nothing else
    // may appear here: the ruling is that a campfire is a real interaction with a
    // real choice, not a one-click "arrive" objective. Emitted only for a campaign
    // with a bonfire → byte-identical otherwise.
    for bf in plan.bonfires() {
        let i = bf.index;
        dialogs.push((
            format!("bonfire_{i}"),
            json!({
                "type": "minecraft:multi_action",
                "title": tr(&chrome.rebind(&bf.prompt)),
                "columns": 1,
                "can_close_with_escape": true,
                "after_action": "close",
                "actions": [
                    { "label": tr(&chrome.rebind(&bf.rest_label)),
                      "action": { "type": "minecraft:run_command", "command": "/trigger dw.rest set 2" } },
                    { "label": tr(&chrome.rebind(&bf.save_label)),
                      "action": { "type": "minecraft:run_command", "command": "/trigger dw.rest set 1" } }
                ]
            }),
        ));
    }

    // spec-0032: one dialog per shop. The offers are the buttons, in declaration
    // order, and each runs `/trigger dw.shop set <n>` — the same channel the
    // bonfire's two options use, because `/trigger` is the only command a non-op
    // player may run. `DW0523` guarantees the action list is non-empty: vanilla's
    // 1.21.11 dialog codec rejects an empty one at pack load.
    for (i, sh, _) in shops(plan) {
        let actions: Vec<Value> = sh
            .offers
            .iter()
            .enumerate()
            .map(|(j, off)| {
                let mut a = json!({
                    "label": tr(&off.label),
                    "action": {
                        "type": "minecraft:run_command",
                        "command": format!("/trigger dw.shop set {}", j + 1)
                    }
                });
                if let Some(t) = &off.tooltip {
                    a.as_object_mut()
                        .expect("json! builds an object")
                        .insert("tooltip".to_string(), tr(t));
                }
                a
            })
            .collect();
        dialogs.push((
            format!("shop_{i}"),
            json!({
                "type": "minecraft:multi_action",
                "title": tr(&sh.title),
                "columns": 1,
                "can_close_with_escape": true,
                "after_action": "close",
                "actions": actions
            }),
        ));
    }

    // per-npc dialogue nodes (stage 6) → one dialog each
    for npc in &plan.npcs {
        let dsl_npc = c
            .npcs
            .content
            .npcs
            .iter()
            .find(|n| n.id.as_str() == npc.npc_id);
        let Some(dsl_npc) = dsl_npc else { continue };
        let Some(tree) = c.dialogue.content.tree_for(&npc.npc_id) else {
            continue;
        };
        for node in &tree.nodes {
            let node_opts: Vec<&plan::OptionPlan> = npc
                .options
                .iter()
                .filter(|o| o.node_id == node.id.as_str())
                .collect();
            let node_safe = plan::safe_local(node.id.as_str());
            let gated = node_gated_options(npc, node.id.as_str(), v04);
            if gated.is_empty() {
                // Ungated node → a single dialog (byte-identical to v0.2/v0.3, or a
                // v0.4 node whose every option is unconditional).
                dialogs.push((
                    format!("{}_{node_safe}", npc.safe),
                    build_node_dialog(
                        &dsl_npc.name,
                        &node.text,
                        &node_opts,
                        &npc.trigger_objective,
                    ),
                ));
            } else {
                // v0.4 display-gated node → one variant per availability bitmask.
                // Bit `i` (declared order among gated options) means "the i-th gated
                // option is displayable now": every flag it needs is set (flag axis)
                // and every objective it completes is active (objective-state axis).
                // The chooser function (`show_<npc>_<node>`) computes the live mask
                // and shows the matching variant, so a gated option is genuinely
                // absent until it is displayable (spec-0008 §1).
                for mask in 0..(1u32 << gated.len()) {
                    let mut gi = 0u32;
                    let visible: Vec<&plan::OptionPlan> = node_opts
                        .iter()
                        .copied()
                        .filter(|o| {
                            if option_display_gated(o, v04) {
                                let bit = gi;
                                gi += 1;
                                mask & (1u32 << bit) != 0
                            } else {
                                true
                            }
                        })
                        .collect();
                    dialogs.push((
                        format!("{}_{node_safe}__m{mask}", npc.safe),
                        build_node_dialog(
                            &dsl_npc.name,
                            &node.text,
                            &visible,
                            &npc.trigger_objective,
                        ),
                    ));
                }
            }
        }
    }
    dialogs
}

/// Build one node dialog from its (already flag-filtered) options. A node with no
/// visible options is a terminal `minecraft:notice` (an empty `multi_action`
/// action list crashes the 1.21.11 dialog codec at load — gap 10); otherwise a
/// `minecraft:multi_action` whose buttons fire each option's `/trigger`.
///
/// **The button's `tooltip` (v0.8).** Vanilla's dialog action button is
/// `ActionButton(CommonButtonData, Optional<DialogAction>)`, and
/// `CommonButtonData`'s codec is exactly `label` (a text component) +
/// `tooltip` (an *optional* text component) + `width` (default 150) — verified
/// against the pinned 1.21.11 client jar's codec, not folklore. The client's
/// `DialogControlSet` turns a present `tooltip` into `Tooltip.create(component)`
/// and hangs it on the button, so it renders as an ordinary hover box (wrapped at
/// 170 px), never on the button face. That is why `DW0331` does not reach it: a
/// tooltip wraps, it does not scroll. An option with no `tooltip` emits no key —
/// a pre-0.8 campaign's dialogs are byte-identical.
fn build_node_dialog(
    npc_name: &str,
    text: &str,
    opts: &[&plan::OptionPlan],
    trigger_objective: &str,
) -> Value {
    if opts.is_empty() {
        json!({
            "type": "minecraft:notice",
            "title": tr(npc_name),
            "body": [{ "type": "minecraft:plain_message", "contents": tr(text) }],
            "can_close_with_escape": true
        })
    } else {
        let actions: Vec<Value> = opts
            .iter()
            .map(|o| {
                let mut action = json!({
                    "label": tr(&o.label),
                    "action": { "type": "minecraft:run_command", "command": format!("/trigger {trigger_objective} set {}", o.n) }
                });
                if let Some(tip) = &o.tooltip {
                    action["tooltip"] = tr(tip);
                }
                action
            })
            .collect();
        json!({
            "type": "minecraft:multi_action",
            "title": tr(npc_name),
            "body": [{ "type": "minecraft:plain_message", "contents": tr(text) }],
            "columns": 1,
            "can_close_with_escape": true,
            "after_action": "close",
            "actions": actions
        })
    }
}

/// The **death loot tables** a campaign's declared quest-item drops need (DSL
/// v0.9), as `(namespace-local path, json)` pairs.
///
/// One table per declaring body, one pool, one roll, one entry per declared
/// item: nothing here rolls a die. The entry is the vanilla `minecraft:item`
/// form, and a declared display `name` becomes the `minecraft:set_name` function
/// with `target: "custom_name"` — the same component a `collect`'s `item_name`
/// writes into a container stack, so the key a boss leaves on the ground and the
/// key a barrel hands over are the same item.
///
/// Emitted only for bodies that declare an `{item}` drop, so a campaign without
/// one writes no `loot_table` directory at all and stays byte-identical.
fn emit_drop_loot_tables(plan: &Plan) -> Vec<(String, Value)> {
    let c = plan.campaign;
    let mut out: Vec<(String, Value)> = Vec::new();
    let table = |drops: &[delvewright_dsl::MobDrop]| {
        let entries: Vec<Value> = drops
            .iter()
            .filter_map(|d| {
                let item = d.item()?;
                let mut entry = json!({ "type": "minecraft:item", "name": item });
                if let Some(name) = d.name() {
                    entry["functions"] = json!([{
                        "function": "minecraft:set_name",
                        "target": "custom_name",
                        "name": { "text": name },
                    }]);
                }
                Some(entry)
            })
            .collect();
        json!({
            "type": "minecraft:entity",
            "pools": [{ "rolls": 1, "entries": entries }],
        })
    };
    for a in &c.quests.content.actors {
        if has_item_drop(&a.drops) {
            out.push((drop_loot_path("actor", a.id.as_str()), table(&a.drops)));
        }
    }
    for w in &c.quests.content.waves {
        for (k, m) in w.mobs.iter().enumerate() {
            if has_item_drop(&m.drops) {
                out.push((
                    drop_loot_path("wave", &format!("{}-{k}", w.id.as_str())),
                    table(&m.drops),
                ));
            }
        }
    }
    out
}

fn emit_advancements(plan: &Plan, chrome: &delvewright_dsl::Chrome) -> Vec<(String, Value)> {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let mut advs = Vec::new();

    // spec-0016 §1: one advancement per bonfire, so a
    // right-click opens the rest dialog AS the player who clicked. The interaction
    // entity's own `interaction` record cannot do this — it names no player the
    // `dialog show` could target — and this is the same vanilla criterion every
    // `interact` objective already runs on. `bonfire_open_<i>` revokes it, so the
    // bonfire is re-openable forever (a rest point is used, never consumed).
    for bf in plan.bonfires() {
        let i = bf.index;
        advs.push((
            format!("bf_{i}"),
            json!({
                "criteria": {
                    "interact": {
                        "trigger": "minecraft:player_interacted_with_entity",
                        "conditions": {
                            "entity": {
                                "type": "minecraft:interaction",
                                "nbt": format!("{{Tags:[\"dw_bonfire_{i}\"]}}")
                            }
                        }
                    }
                },
                "rewards": { "function": format!("{ns}:bonfire_open_{i}") }
            }),
        ));
    }

    // spec-0032: one advancement per shop, and one per stake. Both are the bonfire's
    // primitive verbatim — the interaction entity's own `interaction` record names
    // no player, and a shop has to know WHO is buying and a stake WHOSE wager is
    // being collected. Each handler revokes its own grant, so both are re-usable.
    for (i, _, _) in shops(plan) {
        advs.push((
            format!("shop_{i}"),
            json!({
                "criteria": {
                    "interact": {
                        "trigger": "minecraft:player_interacted_with_entity",
                        "conditions": {
                            "entity": {
                                "type": "minecraft:interaction",
                                "nbt": format!("{{Tags:[\"dw_shop_{i}\"]}}")
                            }
                        }
                    }
                },
                "rewards": { "function": format!("{ns}:shop_open_{i}") }
            }),
        ));
    }
    for (st, safe) in stakes(plan) {
        if st.max_live() == 0 {
            continue;
        }
        let tag = stk_tag(&safe);
        advs.push((
            format!("stk_{safe}"),
            json!({
                "criteria": {
                    "interact": {
                        "trigger": "minecraft:player_interacted_with_entity",
                        "conditions": {
                            "entity": {
                                "type": "minecraft:interaction",
                                "nbt": format!("{{Tags:[\"{tag}\"]}}")
                            }
                        }
                    }
                },
                "rewards": { "function": format!("{ns}:stk_collect_{safe}") }
            }),
        ));
    }

    // DSL v0.11: one advancement per `audience: presser` trigger, so a right-click
    // on the thing runs its bundle AS the player who pressed it. `press_<id>`
    // revokes its own grant, so the object answers every press — a wall is not
    // consumed by being asked.
    //
    // This is `seal_<safe>` lifted off `close-gate`. It keys on the trigger's own
    // `dw_trig_<id>` tag, which `seal_fns` / `ws_arm_fns` / `env_trigger_setup`
    // already put on whatever body that trigger rides or summons — so the
    // advancement needs to know nothing about seals, doors, or any future
    // pressable object class.
    for t in &plan.emitted_triggers(chrome) {
        if !t.addresses_presser() {
            continue;
        }
        let id = plan::safe_local(t.id.as_str());
        advs.push((
            format!("press_{id}"),
            json!({
                "criteria": {
                    "interact": {
                        "trigger": "minecraft:player_interacted_with_entity",
                        "conditions": {
                            "entity": {
                                "type": "minecraft:interaction",
                                "nbt": format!("{{Tags:[\"dw_trig_{id}\"]}}")
                            }
                        }
                    }
                },
                "rewards": { "function": format!("{ns}:press_{id}") }
            }),
        ));
    }

    // one interaction advancement per NPC
    for npc in &plan.npcs {
        advs.push((
            format!("{}_interact", npc.safe),
            json!({
                "criteria": {
                    "interact": {
                        "trigger": "minecraft:player_interacted_with_entity",
                        // 1.21.11's `player_interacted_with_entity` `entity` field is
                        // an Either<single entity sub-predicate, list of loot
                        // conditions>. The list form requires each entity_properties
                        // condition to carry its own `entity: "this"` key; the single
                        // sub-predicate object form is simpler and is what loads
                        // cleanly on a live server (verified in the load shakeout —
                        // the list form failed with "No key entity in MapLike").
                        "conditions": {
                            "entity": {
                                "type": "minecraft:interaction",
                                "nbt": format!("{{Tags:[\"{}\"]}}", npc.tag)
                            }
                        }
                    }
                },
                "rewards": { "function": format!("{ns}:talk_{}", npc.safe) }
            }),
        ));
    }

    // v0.3: one advancement per interact objective, collect objective and wave.
    for q in &c.quests.content.quests {
        for o in &q.objectives {
            match o {
                Objective::Interact { id, .. } => {
                    let tag = interact_entity_tag(id.as_str());
                    advs.push((
                        format!("i_{}", plan::safe_local(id.as_str())),
                        json!({
                            "criteria": {
                                "interact": {
                                    "trigger": "minecraft:player_interacted_with_entity",
                                    "conditions": {
                                        "entity": {
                                            "type": "minecraft:interaction",
                                            "nbt": format!("{{Tags:[\"{tag}\"]}}")
                                        }
                                    }
                                }
                            },
                            "rewards": { "function": format!("{ns}:i_reward_{}", plan::safe_local(id.as_str())) }
                        }),
                    ));
                }
                Objective::Collect {
                    id, item, count, ..
                } => {
                    advs.push((
                        format!("c_{}", plan::safe_local(id.as_str())),
                        json!({
                            "criteria": {
                                "got": {
                                    "trigger": "minecraft:inventory_changed",
                                    "conditions": {
                                        "items": [ { "items": item, "count": { "min": count } } ]
                                    }
                                }
                            },
                            "rewards": { "function": format!("{ns}:c_reward_{}", plan::safe_local(id.as_str())) }
                        }),
                    ));
                }
                _ => {}
            }
        }
    }
    for w in &c.quests.content.waves {
        let tag = plan::wave_tag(w.id.as_str());
        advs.push((
            format!("k_{}", plan::safe_local(w.id.as_str())),
            json!({
                "criteria": {
                    "slain": {
                        "trigger": "minecraft:player_killed_entity",
                        "conditions": {
                            "entity": { "nbt": format!("{{Tags:[\"{tag}\"]}}") }
                        }
                    }
                },
                "rewards": { "function": format!("{ns}:k_reward_{}", plan::safe_local(w.id.as_str())) }
            }),
        ));
    }

    // campaign-complete advancement (granted by command). Both player-visible
    // strings are campaign-derived and therefore localized: `localize` rewrites the
    // whole `Campaign` before emission, so whatever we read here is already in the
    // target language. The description was a hardcoded `"You left the keep."` on
    // every delve ever built — the reference keep-crawl's line, shipped verbatim to
    // a shipwreck campaign and untranslatable in every sidecar because it never
    // passed through a `Campaign` field at all.
    let outro = campaign_outro(c);
    advs.push((
        "campaign_complete".to_string(),
        json!({
            "criteria": { "granted": { "trigger": "minecraft:impossible" } },
            "display": {
                "icon": { "id": "minecraft:iron_door" },
                "title": tr(&c.world.content.title),
                "description": tr(&outro),
                "frame": "goal",
                "show_toast": true,
                "announce_to_chat": false,
                "hidden": false
            }
        }),
    ));
    advs
}

// ---------------------------------------------------------------------------
// packtest / server / critical-path / manifest
// ---------------------------------------------------------------------------

/// Emit the compiler-generated PackTest suite (spec-0003). PackTest (misode,
/// 2.4.0 for MC 1.21.11) auto-discovers `*.mcfunction` files under
/// `data/<ns>/test/`; each is one game test driven by `# @…` directive comments,
/// with `assert`/`await`/`succeed`/`fail` commands the mod adds. Run headlessly
/// with `-Dpacktest.auto` (exit code = failed tests). These functions use
/// PackTest-only commands and run on the modded validation server, so they are
/// exempt from the vanilla command-tree validator (see `is_vanilla_function`).
fn emit_packtest(
    plan: &Plan,
    out: &mut BuildOutput,
    moves: &[crate::nav::MovePlan],
    actor_moves: &[crate::nav::ActorMovePlan],
    waves: &WaveGeometry<'_>,
    payloads: &PayloadPlans,
) {
    let ns = &plan.namespace;
    let c = plan.campaign;
    put_json(
        out,
        "packtest-datapack/pack.mcmeta",
        &json!({
            "pack": {
                "description": format!("Delvewright PackTest suite: {ns}"),
                "min_format": PACK_FORMAT,
                "max_format": PACK_FORMAT,
            }
        }),
    );

    // The completion objective + value the critical path asserts on.
    let (comp_obj, comp_val) = plan
        .critical_path
        .iter()
        .find_map(|s| match s {
            Step::AssertComplete { objective, value } => Some((objective.clone(), *value)),
            _ => None,
        })
        .unwrap_or_else(|| ("dw.campaign".to_string(), 1));

    // Mechanism test: on a dummy player, run the real generated init, activate the
    // campaign-start quests (as class selection does), drive each objective's
    // generated completion function (as the dialog `/trigger` and the reach
    // proximity check do), then assert the completion objective is set. This
    // proves the compiler's objective -> quest -> campaign chain end to end
    // without needing dialog-UI clicks or bot movement (verified live: passes on
    // Fabric + PackTest 2.4.0).
    //
    // Two structural facts of the campaign shape the template (the-wake):
    //
    //   * `campaign-complete` may sit at any nesting depth (spec-0025 / DW0481) —
    //     the-wake schedules it 250t into its closing `sequence`. A same-tick
    //     `assert` after the drive is then structurally unreachable ("got 0 on
    //     tick 0"), so a campaign whose ending has a scheduled tail must AWAIT
    //     the completion objective, with the timeout sized by the tail the
    //     emitter itself scheduled.
    //   * Declared `branch_points` make some terminal objectives mutually
    //     exclusive: driving every objective in one pass reaches a state no
    //     playthrough can (both endings fired in one tick). A branch campaign
    //     therefore drives one coherent per-branch path per phase, serialized
    //     through the vanilla scheduler.
    //
    // Campaigns with no branch points and a synchronous ending keep the original
    // single-tick template byte for byte.
    let (pin, sel) = pin_dummy("dw_t_camp");
    let party = plan::PARTY;
    let branches: Vec<crate::branch::RealizedBranch> = crate::branch::realize(c)
        .into_iter()
        .filter(|r| r.world.is_some())
        .collect();
    if branches.is_empty() {
        // No declared branch points (or nothing reachable — already DW0482):
        // one coherent drive over every objective, exactly as before.
        let quests: BTreeSet<&str> = c
            .quests
            .content
            .quests
            .iter()
            .map(|q| q.id.as_str())
            .collect();
        let tail = quests_ending_tail(c, &quests, moves, actor_moves);
        // Baseline + drive. Actively establish the asserted baseline — on the
        // shared-batch server "never set" is not 0. spec-0018: the whole chain is
        // PARTY state, so the baseline, the activation and the assert all address
        // `#party`; the dummy is still what DRIVES it (`execute as {sel} run …`),
        // which is exactly the multiplayer claim — one player's action advances
        // the party.
        let mut drive: Vec<String> = Vec::new();
        drive.push(format!("scoreboard players set {party} {comp_obj} 0"));
        for qid in campaign_start_quests(c) {
            drive.push(format!(
                "scoreboard players set {party} {} 1",
                quest_active_score(qid)
            ));
        }
        for q in &c.quests.content.quests {
            for o in &q.objectives {
                drive.push(format!(
                    "execute as {sel} run function {ns}:complete_{}",
                    safe_obj_fn(o.id().as_str())
                ));
            }
        }
        let mut body: Vec<String> = Vec::new();
        body.push(format!(
            "#> {}: objective completions set {comp_obj} (Delvewright mechanism test)",
            artifact_title(c)
        ));
        body.push("# @dummy".to_string());
        if tail == 0 {
            body.push("# @timeout 100".to_string());
            body.push(String::new());
            body.push(format!("function {ns}:setup"));
            // Pin this test's own dummy and drive the whole chain on it alone (see
            // `pin_dummy`): `@a`-wide quest/objective writes would land on every
            // sibling test's dummy in the batch, and the closing `@p` assert could
            // read a foreign one.
            body.push(pin);
            body.extend(drive);
            body.push(format!(
                "assert score {party} {comp_obj} matches {comp_val}"
            ));
        } else {
            // Scheduled ending: the ending lands `tail` ticks after the terminal
            // drive, so the template awaits it (never a weaker assert — `await`
            // fails the test at timeout exactly as `assert` fails it on the
            // spot). The template now spans ticks, so its body may touch no
            // `#party` score a sibling template also touches
            // (`tests/packtest_batch.rs::party_state_across_ticks_is_owned`):
            // the baseline + drive — shared quest/flag state, written and
            // consumed atomically within one tick, exactly as in the single-tick
            // form — is hoisted into `pt_camp_drive`, leaving the awaited
            // completion objective (owned by this template alone) as the
            // template's only cross-tick surface.
            body.push(format!("# @timeout {}", 100 + tail));
            body.push(String::new());
            body.push(format!("function {ns}:setup"));
            body.push(pin);
            body.push(format!("function {ns}:pt_camp_drive"));
            body.push(format!("await score {party} {comp_obj} matches {comp_val}"));
            out.insert(
                format!("packtest-datapack/data/{ns}/function/pt_camp_drive.mcfunction"),
                lines(&drive).into_bytes(),
            );
        }
        out.insert(
            format!("packtest-datapack/data/{ns}/test/campaign.mcfunction"),
            lines(&body).into_bytes(),
        );
    } else {
        emit_branch_campaign_packtest(
            plan,
            out,
            &branches,
            moves,
            actor_moves,
            (&comp_obj, comp_val),
            (&pin, &sel),
        );
    }

    // Sealed-state test: prove the environment-sealing baseline (spec-0002) is
    // applied on boot. What PackTest / vanilla 1.21.11 lets us assert in-test:
    //   * `time set noon` — the world time has a read-back path
    //     (`time query daytime` -> 6000), so it is asserted directly here.
    //   * the five gamerules — 1.21.11 gamerule *values* have NO `execute
    //     if`/predicate read-back in vanilla, so they cannot be asserted in-game.
    //     Their presence and exact 1.21.11 form is a compile-time regression
    //     instead (crates/compiler/tests/emit.rs::environment_sealing_emitted),
    //     which is the authoritative sealing assertion.
    // Verified live: `function <ns>:setup` sets daytime to 6000 and this assert
    // passes on Fabric + PackTest 2.4.0.
    let mut sealed: Vec<String> = Vec::new();
    sealed.push(format!(
        "#> {}: environment sealed on boot (spec-0002)",
        artifact_title(c)
    ));
    sealed.push("# @dummy".to_string());
    sealed.push("# @timeout 100".to_string());
    sealed.push(String::new());
    sealed.push(format!("function {ns}:setup"));
    let sealed_time = c.world.content.time.unwrap_or_default();
    let sealed_ticks = sealed_time.daytime_ticks();
    sealed.push(format!(
        "# time set {} -> daytime {sealed_ticks} (the sole sealing command with a",
        sealed_time.token()
    ));
    sealed.push("# vanilla read-back path; gamerules are asserted at compile time).".to_string());
    sealed.push(
        "execute store result score #sealtime_sealed dw.sys run time query daytime".to_string(),
    );
    sealed.push(format!(
        "assert score #sealtime_sealed dw.sys matches {sealed_ticks}"
    ));

    out.insert(
        format!("packtest-datapack/data/{ns}/test/sealed_state.mcfunction"),
        lines(&sealed).into_bytes(),
    );

    // Declared combat difficulty (v0.6): prove on a live
    // pinned server that the difficulty the campaign DECLARED is the difficulty
    // the world runs at. Unlike the gamerules, this one has a vanilla read-back:
    // the bare `/difficulty` query command returns `Difficulty#getId()`
    // (peaceful 0 / easy 1 / normal 2 / hard 3), so `execute store result` reads
    // it exactly like `time query daytime`. The assertion covers the whole chain
    // at once — the shipped `server/server.properties` (via the compose
    // profile's shared world-settings entrypoint) and the `/difficulty` in
    // `setup` must agree with the declaration, so a regression in EITHER fails
    // here. Emitted only for a campaign that declares a difficulty.
    if let Some(diff) = declared_difficulty(c) {
        let mut df: Vec<String> = Vec::new();
        df.push(format!(
            "#> {}: the world runs at the declared difficulty `{}`",
            artifact_title(c),
            diff.token()
        ));
        df.push("# @dummy".to_string());
        df.push("# @timeout 100".to_string());
        df.push(String::new());
        df.push(format!("function {ns}:setup"));
        df.push(
            "# Bare `/difficulty` is the query form: it returns Difficulty#getId()".to_string(),
        );
        df.push("# (peaceful 0 / easy 1 / normal 2 / hard 3).".to_string());
        df.push("execute store result score #difficulty dw.sys run difficulty".to_string());
        df.push(format!(
            "assert score #difficulty dw.sys matches {}",
            diff.id()
        ));
        out.insert(
            format!("packtest-datapack/data/{ns}/test/declared_difficulty.mcfunction"),
            lines(&df).into_bytes(),
        );
    }

    // v0.3: one focused mechanism test per gameplay verb present in the campaign,
    // plus a flag-gate test. Each drives the compiler-generated mechanic functions
    // on a dummy player (no real combat / advancement events needed) and asserts
    // the objective scoreboard. Emits nothing for a v0.2 campaign.
    emit_verb_packtests(plan, out);

    // The dialogue trigger must survive a second use with NO tick in between —
    // the singleplayer pause-freeze contract. Emits nothing for a campaign with no
    // terminal dialogue option.
    emit_dialogue_trigger_packtest(plan, out);
    emit_cast_packtests(plan, out);

    // v0.4: prop-on-activation, despawn removes body+hitbox, move arrives at
    // target. Emits nothing when the campaign uses none of them.
    emit_v04_packtests(plan, out, moves);

    // round-8: two flag-gated click triggers on one NPC hitbox must both be
    // reachable. Emits nothing without such a pair.
    emit_shared_hitbox_packtest(plan, out);

    // The class trigger is one-shot per player. Emitted for every campaign
    // that declares a class, i.e. every campaign.
    emit_class_seal_packtest(plan, out);

    // A sealed gate carries the hitboxes its right-click answer rides.
    // Emits nothing for a campaign that seals no gate.
    emit_seal_packtest(plan, out);

    // v0.6: boundary return / never-move-inside (spec-0013). Emits nothing without
    // a boundary.
    emit_boundary_packtest(plan, out);
    emit_night_vision_packtest(plan, out);

    // v0.6: checkpoint respawn contract + stealth kill/spare judge (spec-0012 /
    // spec-0014). Emits nothing when the campaign uses neither.
    emit_v06_packtests(plan, out);

    // v0.6 (spec-0014): actor spawn/despawn (kill vs vanish), move-actor arrival,
    // unleash swap. Emits nothing for a campaign with no actors.
    emit_v06_actor_packtests(plan, out, actor_moves);
    // v0.6: trap payload loads into the dispenser; a disarm empties it (spec-0011).
    // Emits nothing when the campaign declares no traps.
    emit_trap_packtests(plan, out);
    emit_payload_packtests(plan, out, payloads);
    // spec-0016 §1: resting at a bonfire moves the party respawn point and
    // re-seats its `respawns_on_rest` waves. Emits nothing without a bonfire.
    // The tag census really counts the wave, and only the wave.
    emit_wave_census_packtest(plan, out);
    emit_bonfire_packtests(plan, out);
    // spec-0016 §1: a re-seated wave comes back
    // STATIONED — at its lane start / anchor, in its routed state, with no trace
    // of the previous life's feral release. Emits nothing without a bonfire and
    // a `respawns_on_rest` wave.
    emit_reseat_stationed_packtest(plan, out, waves.placements, waves.lanes);
    // spec-0016 §1: the UNDEFEATED re-seat — an elite
    // the party is still fighting is deleted and stood up fresh on its origin;
    // one they finished stays finished. Emits nothing without a bonfire and a
    // hostile actor / billed wave.
    emit_reseat_undefeated_packtests(plan, out);
    // spec-0016 §1: rest and save-only really differ.
    emit_bonfire_option_packtest(plan, out);
    // spec-0016 §2: the shortcut really opens, and opens exactly once.
    emit_shortcut_packtest(plan, out);
    // spec-0016 §4: the clock really alternates the gate region.
    emit_timed_gate_packtest(plan, out);
    emit_loot_packtest(plan, out);
    emit_actor_equipment_packtest(plan, out);
    // spec-0016 §6: the patrol NBT survives 1.21.11's strict codec, the lane
    // advances in march order, the squad is released to native AI at aggro range,
    // and an aggro-edge wave really materializes on its perception ring. Emits
    // nothing for a campaign with no lane and no aggro-edge wave.
    emit_td_lane_packtests(plan, out, waves.lanes, waves.rings);

    // The scheduled-executor contract (AUDIT-P0): a function reached through
    // `schedule` still lands per-player state on real players.
    emit_scheduled_executor_packtests(plan, out, moves);

    // spec-0018: one n-dummy division-of-labour test per AND-join.
    emit_party_join_packtests(plan, out);
}

/// Ticks between one PackTest campaign phase's ending window closing and its
/// verdict being taken — slack for the scheduler landing the ending's last
/// function plus the completion write itself.
const CAMPAIGN_PHASE_MARGIN_TICKS: u32 = 20;

/// The branch-aware campaign mechanism test: ONE template that
/// drives each reachable branch's coherent path as its own phase, serialized
/// through the vanilla scheduler, and awaits one verdict per phase.
///
/// Why one template rather than one per branch: every phase's verdict is the
/// shared completion objective, and a template that spans ticks must be the sole
/// owner of every `#party` score it depends on across ticks
/// (`tests/packtest_batch.rs::party_state_across_ticks_is_owned`) — two
/// concurrently-running branch templates zeroing and awaiting `dw.campaign`
/// would hand each other false verdicts in an order the compiler does not
/// control. Phases are strictly ordered by construction: phase *i*'s scheduled
/// check is what starts phase *i + 1*.
///
/// Each phase (`pt_camp_run_<i>`) re-baselines the WHOLE progression surface
/// (completion objective, every flag, every quest active/complete score, every
/// objective score — a fresh coherent run; a prior phase's terminal quest would
/// otherwise stay `dw.q_* = 1` and its completion-guarded `on_complete` never
/// re-fire), sets the campaign-start quests active, then drives ONLY this
/// branch's path in play order. A `talk-to` step whose branch-scripted option
/// sets flags has those flags emulated immediately before its drive — the
/// option handler is UI-bound, and this is where the real playthrough sets
/// them. The phase's verdict is taken `tail + margin` ticks later
/// (`pt_camp_check_<i>`): completion objective at its expected value counts the
/// phase into `#camp_phase`, and the template's single closing `await` demands
/// every phase counted. A missed ending leaves the count short and the await
/// times out red — never weaker than the old assert, and now quantified over
/// branches.
#[allow(clippy::too_many_arguments)]
fn emit_branch_campaign_packtest(
    plan: &Plan,
    out: &mut BuildOutput,
    branches: &[crate::branch::RealizedBranch],
    moves: &[crate::nav::MovePlan],
    actor_moves: &[crate::nav::ActorMovePlan],
    (comp_obj, comp_val): (&str, i32),
    (pin, sel): (&str, &str),
) {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let party = plan::PARTY;
    let n = branches.len();
    let mut timeout: u32 = 100;
    for (i, r) in branches.iter().enumerate() {
        let quests: BTreeSet<&str> = r.path.iter().map(|s| s.quest.as_str()).collect();
        let tail = quests_ending_tail(c, &quests, moves, actor_moves);
        let wait = tail + CAMPAIGN_PHASE_MARGIN_TICKS;
        timeout += wait;
        let mut run: Vec<String> = Vec::new();
        run.push(format!(
            "# Phase {i}: branch `{}` — full progression re-baseline, then this branch's",
            r.branch.id
        ));
        run.push(
            "# coherent path only (its scripted dialogue choices emulated as the flags".to_string(),
        );
        run.push("# those options set, at their real path positions).".to_string());
        run.push(format!("scoreboard players set {party} {comp_obj} 0"));
        for f in declared_flags(c) {
            run.push(format!(
                "scoreboard players set {party} {} 0",
                plan::flag_score(&f)
            ));
        }
        for q in &c.quests.content.quests {
            run.push(format!(
                "scoreboard players set {party} {} 0",
                quest_score(q.id.as_str())
            ));
            run.push(format!(
                "scoreboard players set {party} {} 0",
                quest_active_score(q.id.as_str())
            ));
            for o in &q.objectives {
                run.push(format!(
                    "scoreboard players set {party} {} 0",
                    obj_score(o.id().as_str())
                ));
            }
        }
        for qid in campaign_start_quests(c) {
            run.push(format!(
                "scoreboard players set {party} {} 1",
                quest_active_score(qid)
            ));
        }
        for step in &r.path {
            if let Some(opt) = step.talk_option {
                for f in option_sets_flags(plan, &step.objective, opt) {
                    run.push(format!(
                        "scoreboard players set {party} {} 1",
                        plan::flag_score(f)
                    ));
                }
            }
            run.push(format!(
                "execute as {sel} run function {ns}:complete_{}",
                safe_obj_fn(&step.objective)
            ));
        }
        run.push(format!("schedule function {ns}:pt_camp_check_{i} {wait}t"));
        out.insert(
            format!("packtest-datapack/data/{ns}/function/pt_camp_run_{i}.mcfunction"),
            lines(&run).into_bytes(),
        );

        let mut chk = vec![format!(
            "execute if score {party} {comp_obj} matches {comp_val} run \
             scoreboard players add #camp_phase dw.sys 1"
        )];
        if i + 1 < n {
            chk.push(format!("function {ns}:pt_camp_run_{}", i + 1));
        }
        out.insert(
            format!("packtest-datapack/data/{ns}/function/pt_camp_check_{i}.mcfunction"),
            lines(&chk).into_bytes(),
        );
    }

    let mut body: Vec<String> = Vec::new();
    body.push(format!(
        "#> {}: each branch's coherent path sets {comp_obj} (Delvewright mechanism test)",
        artifact_title(c)
    ));
    body.push("# @dummy".to_string());
    body.push(format!("# @timeout {timeout}"));
    body.push(String::new());
    body.push(format!("function {ns}:setup"));
    body.push(pin.to_string());
    // Own init for the phase counter — on the shared batch server "never set"
    // is not 0, and `#camp_phase` belongs to this template alone.
    body.push("scoreboard players set #camp_phase dw.sys 0".to_string());
    // The phase chain: pt_camp_run_0 -> pt_camp_check_0 -> pt_camp_run_1 -> …
    // (each check is scheduled by its run and starts the next run).
    body.push(format!("function {ns}:pt_camp_run_0"));
    body.push(format!("await score #camp_phase dw.sys matches {n}"));
    out.insert(
        format!("packtest-datapack/data/{ns}/test/campaign.mcfunction"),
        lines(&body).into_bytes(),
    );
}

/// The flags the branch-scripted dialogue option at flat index `n` (1-based, per
/// the NPC the `talk-to` objective names) sets when chosen — what the campaign
/// phase drive emulates in place of a UI click. Empty when the objective is not
/// a `talk-to` or names no such option.
fn option_sets_flags<'p>(plan: &'p Plan, objective: &str, n: usize) -> &'p [String] {
    let npc = plan
        .campaign
        .quests
        .content
        .quests
        .iter()
        .flat_map(|q| &q.objectives)
        .find_map(|o| match o {
            Objective::TalkTo { id, npc, .. } if id.as_str() == objective => Some(npc.as_str()),
            _ => None,
        });
    npc.and_then(|npc_id| plan.npcs.iter().find(|p| p.npc_id == npc_id))
        .and_then(|p| p.options.iter().find(|o| o.n == n as i32))
        .map(|o| o.sets_flags.as_slice())
        .unwrap_or(&[])
}

/// The scheduled tail (ticks) between firing `effs` and a `campaign-complete`
/// nested anywhere inside it — `None` when the bundle reaches none.
///
/// spec-0025 / DW0481 admit the ending at any nesting depth (the-wake schedules
/// its finale 250t into the closing `sequence`), so every consumer that waits
/// for the ending — the campaign PackTest, the harness completion window — must
/// wait out the tail the emitter itself scheduled. `sequence` steps add their
/// `at_ticks`; a `move-npc` / `move-actor` `on_arrive` adds the planned walk
/// duration. Reaction bundles (`on_respawn` / `on_rest` / `on_caught`) are
/// skipped: driving objective completions never fires them, and `DW0204` proves
/// the path's ending does not live exclusively there. Flag gates are ignored —
/// a gated ending yields an upper bound, and waiting longer can only wait,
/// never wrongly pass.
fn campaign_complete_tail(
    effs: &[QuestEffect],
    moves: &[crate::nav::MovePlan],
    actor_moves: &[crate::nav::ActorMovePlan],
) -> Option<u32> {
    effs.iter()
        .filter_map(|e| match e {
            QuestEffect::CampaignComplete { .. } => Some(0),
            QuestEffect::Sequence { steps } => steps
                .iter()
                .filter_map(|s| {
                    campaign_complete_tail(&s.effects, moves, actor_moves).map(|t| s.at_ticks + t)
                })
                .max(),
            QuestEffect::MoveNpc {
                npc,
                to_anchor,
                on_arrive,
                ..
            } => campaign_complete_tail(on_arrive, moves, actor_moves).map(|t| {
                t + moves
                    .iter()
                    .find(|m| m.npc == npc.as_str() && m.to_anchor == to_anchor.as_str())
                    .map(|m| m.ticks() as u32)
                    .unwrap_or(0)
            }),
            QuestEffect::MoveActor {
                actor,
                to_anchor,
                on_arrive,
                ..
            } => campaign_complete_tail(on_arrive, moves, actor_moves).map(|t| {
                t + actor_moves
                    .iter()
                    .find(|m| m.actor == actor.as_str() && m.to_anchor == to_anchor.as_str())
                    .map(|m| m.ticks() as u32)
                    .unwrap_or(0)
            }),
            _ => None,
        })
        .max()
}

/// The ending tail a driven run of `quest_ids` can schedule: the max
/// [`campaign_complete_tail`] over those quests' `on_objective_complete` bundles
/// and `on_complete`. `0` when the ending is synchronous.
fn quests_ending_tail(
    c: &delvewright_dsl::Campaign,
    quest_ids: &BTreeSet<&str>,
    moves: &[crate::nav::MovePlan],
    actor_moves: &[crate::nav::ActorMovePlan],
) -> u32 {
    c.quests
        .content
        .quests
        .iter()
        .filter(|q| quest_ids.contains(q.id.as_str()))
        .flat_map(|q| {
            q.on_objective_complete
                .values()
                .map(|effs| effs.as_slice())
                .chain(std::iter::once(q.on_complete.as_slice()))
        })
        .filter_map(|effs| campaign_complete_tail(effs, moves, actor_moves))
        .max()
        .unwrap_or(0)
}

/// The AND-joins of a campaign: every objective with **two or more** `after`
/// prerequisites, with its quest and arms, in deterministic content order.
/// `after: [obj/a, obj/b]` is the DSL's AND primitive (spec-0018 adds no new
/// stage-5 syntax), and under party progression it is exactly the shape two
/// players split between two rooms.
fn and_joins(c: &delvewright_dsl::Campaign) -> Vec<(&str, &Objective)> {
    let mut out = Vec::new();
    for q in &c.quests.content.quests {
        for o in &q.objectives {
            if o.after().len() >= 2 {
                out.push((q.id.as_str(), o));
            }
        }
    }
    out
}

/// The party-size cap: a delve is played by one party of 1–4 (CLAUDE.md).
const MAX_PARTY: usize = 4;

/// Generated **division-of-labour** PackTests (spec-0018), one per AND-join.
///
/// The claim under test is the whole point of party progression: `n` DIFFERENT
/// players each complete exactly one arm of an `after` AND-join, and the
/// successor opens **for the party**. A single-dummy test cannot make that claim
/// — it would prove only that one player can do everything in sequence, which was
/// already true before the party holder existed. So each template spawns the
/// extra members itself with PackTest's `/dummy <name> spawn` (the framework
/// `# @dummy` supplies member 1 as `@s`) and drives one arm per member.
///
/// Three assertions, in order, and the middle one is the load-bearing negative:
///
/// 1. with no arm done, the join's real emitted guard is **not** satisfied;
/// 2. after member 1's arm alone it is **still** not satisfied (the AND is a real
///    AND — the successor does not leak open on one arm);
/// 3. after every member's arm it **is** satisfied, and the LAST member — never
///    the one who cleared the first arm — completes the join, proving each member
///    sees and can consume the successor state.
///
/// Batch model: own members (spawned and removed by this template alone,
/// under names no other template uses), own scratch holder (`#pj_<obj>`), own
/// init (every party score it reads is actively baselined), and no `await` — the
/// whole body is one atomic tick, so no sibling can interleave inside it.
///
/// `n` is the arm count, raised to `world.min_players` when the campaign declares
/// a bigger mandatory party and capped at [`MAX_PARTY`]; arms are handed out
/// round-robin, so a join with more arms than members gives someone two.
fn emit_party_join_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let party = plan::PARTY;
    let min_players = plan::min_players(c) as usize;

    for (ji, (qid, join)) in and_joins(c).into_iter().enumerate() {
        let jid = join.id().as_str();
        let jsafe = plan::safe_local(jid);
        let arms: Vec<&str> = join.after().iter().map(|a| a.as_str()).collect();
        let n = arms.len().max(min_players).min(MAX_PARTY);
        // Member selectors. Member 1 is the framework dummy (`@s` — the binding
        // survives teleports and can never resolve to a neighbour's dummy);
        // members 2..n are spawned here under this template's own names.
        let name = |m: usize| format!("dwj{ji}p{m}");
        let member = |m: usize| {
            if m == 0 {
                "@s".to_string()
            } else {
                format!("@a[name={},limit=1]", name(m))
            }
        };
        let scratch = format!("#pj_{jsafe}");

        let mut b = packtest_header(&format!(
            "{}: AND-join `{jid}` divides across {n} players (spec-0018)",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        for m in 1..n {
            b.push(format!("dummy {} spawn", name(m)));
        }

        // --- own init: baseline every party score this join's guard reads ------
        let mut baseline: Vec<String> = Vec::new();
        let push_obj_baseline = |baseline: &mut Vec<String>, quest: &str, o: &Objective| {
            baseline.push(format!(
                "scoreboard players set {party} {} 1",
                quest_active_score(quest)
            ));
            for f in o.requires_flags() {
                baseline.push(format!(
                    "scoreboard players set {party} {} 1",
                    plan::flag_score(f.as_str())
                ));
            }
            for f in o.forbids_flags() {
                baseline.push(format!(
                    "scoreboard players set {party} {} 0",
                    plan::flag_score(f.as_str())
                ));
            }
        };
        // The join itself: quest active + its flag gates, its own score cleared.
        // Its `after` arms are deliberately NOT set — they are what the party
        // is about to earn.
        push_obj_baseline(&mut baseline, qid, join);
        baseline.push(format!(
            "scoreboard players set {party} {} 0",
            obj_score(jid)
        ));
        // Each arm: its own prerequisites satisfied, its own score cleared.
        for arm in &arms {
            let Some((aq, ao)) =
                objective_quest(c, arm).and_then(|(q, _)| find_objective(c, arm).map(|o| (q, o)))
            else {
                continue;
            };
            push_obj_baseline(&mut baseline, aq, ao);
            for prereq in ao.after() {
                baseline.push(format!(
                    "scoreboard players set {party} {} 1",
                    obj_score(prereq.as_str())
                ));
            }
            baseline.push(format!(
                "scoreboard players set {party} {} 0",
                obj_score(arm)
            ));
        }
        // Order-preserving dedup: sibling arms share a quest, so their
        // quest-active baselines coincide.
        let mut seen: BTreeSet<String> = BTreeSet::new();
        b.extend(baseline.into_iter().filter(|l| seen.insert(l.clone())));

        // The join's REAL emitted activation guard, materialized as a score so a
        // PackTest can assert it. Not a restatement: `pending_guard` is the very
        // function the `tick` driver uses.
        let guard = pending_guard(plan, join, &quest_active_score(qid));
        let probe = |b: &mut Vec<String>, expect: u32| {
            b.push(format!("scoreboard players set {scratch} dw.sys 0"));
            b.push(format!(
                "execute{guard} run scoreboard players set {scratch} dw.sys 1"
            ));
            b.push(format!("assert score {scratch} dw.sys matches {expect}"));
        };

        // 1. no arm done -> the join is shut.
        probe(&mut b, 0);

        // 2/3. each member clears exactly one arm (round-robin), and the party
        // score advances on THEIR action.
        for (k, arm) in arms.iter().enumerate() {
            b.push(format!(
                "execute as {} run function {ns}:complete_{}",
                member(k % n),
                safe_obj_fn(arm)
            ));
            b.push(format!("assert score {party} {} matches 1", obj_score(arm)));
            // After the FIRST arm (and while others remain) the join must still
            // be shut — the negative half that makes this an AND, not an OR.
            if k == 0 && arms.len() > 1 {
                probe(&mut b, 0);
            }
        }
        probe(&mut b, 1);

        // The successor is the PARTY's: the LAST member completes it, never the
        // one who cleared the first arm.
        b.push(format!(
            "execute as {} run function {ns}:complete_{}",
            member((arms.len() - 1) % n),
            safe_obj_fn(jid)
        ));
        b.push(format!("assert score {party} {} matches 1", obj_score(jid)));

        // No residue: the members this template spawned leave with it.
        for m in 1..n {
            b.push(format!("dummy {} leave", name(m)));
        }
        out.insert(
            format!("packtest-datapack/data/{ns}/test/party_join_{jsafe}.mcfunction"),
            lines(&b).into_bytes(),
        );
    }
}

/// The stage-5 objective with this id, across every quest.
fn find_objective<'a>(c: &'a delvewright_dsl::Campaign, id: &str) -> Option<&'a Objective> {
    c.quests
        .content
        .quests
        .iter()
        .flat_map(|q| &q.objectives)
        .find(|o| o.id().as_str() == id)
}

/// The flag a [`SCHEDULED_PROBE`] `set-flag` sets. Test-only: it exists solely in
/// the PackTest datapack, never in the shipped delve.
const SCHEDULED_PROBE_FLAG: &str = "flag/pt-sched-probe";

/// The PackTest-datapack function the scheduled-executor probe schedules.
const SCHEDULED_PROBE: &str = "pt_sched_probe";

/// PackTests for the scheduled-executor contract (AUDIT-P0).
///
/// `schedule function …` re-invokes a function with the **server** command
/// source — no executor, so every `@s`-addressed command in it silently does
/// nothing. Two templates, because one alone would not have caught the bug:
///
/// 1. `sched_executor` — **unconditional**, so every campaign (hello-world in
///    CI tier 2 included) proves the seam live on a real server. A probe
///    function in the PackTest datapack, emitted by the *real* scheduled-bundle
///    emitter ([`emit_effect_bundle`] with [`Audience::Scheduled`]) over a
///    `set-flag`, is handed to the vanilla scheduler; the test then awaits the
///    flag on its own dummy's score. Pre-fix output emits `scoreboard players
///    set @s …` here and the await times out.
/// 2. `sched_arrive_flag` — the content path, for the first `move-npc` whose
///    `on_arrive` sets a flag (the island's stealth beat). It runs the REAL
///    start function and lets the driver walk itself to the end through the
///    scheduler. The pre-existing arrive templates all call `mv_tick`/`ma_tick`
///    *inline as the dummy*, which supplies exactly the player executor the
///    scheduler does not — that is how this bug survived a green suite.
fn emit_scheduled_executor_packtests(
    plan: &Plan,
    out: &mut BuildOutput,
    moves: &[crate::nav::MovePlan],
) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let probe_score = plan::flag_score(SCHEDULED_PROBE_FLAG);

    // --- 1. the unconditional probe -------------------------------------
    // The probe body goes through the real emitter, so it carries whatever the
    // scheduled-bundle seam currently produces — this template is a live test
    // OF that seam, not a restatement of it.
    let probe = emit_effect_bundle(
        plan,
        &[delvewright_dsl::QuestEffect::SetFlag {
            flag: delvewright_dsl::FlagId(SCHEDULED_PROBE_FLAG.to_string()),
            requires_flags: Vec::new(),
            forbids_flags: Vec::new(),
            requires_state: Vec::new(),
        }],
        Audience::Scheduled,
    );
    out.insert(
        format!("packtest-datapack/data/{ns}/function/{SCHEDULED_PROBE}.mcfunction"),
        lines(&probe).into_bytes(),
    );
    let mut t = packtest_header(&format!(
        "{title}: a SCHEDULED function still reaches the party (scheduled-executor contract)"
    ));
    t.push(format!("function {ns}:setup"));
    // Own init: the probe objective is test-only, so this template creates it and
    // clears its own dummy (never assume 0 on the shared batch server).
    t.push(format!("scoreboard objectives add {probe_score} dummy"));
    // spec-0018: the probe flag is party state, so the baseline and the await
    // both address `#party`. The objective is test-only (it exists solely in the
    // PackTest datapack), so this template is its sole owner in the batch — the
    // ownership `tests/packtest_batch.rs` demands of any template that awaits.
    t.push(format!(
        "scoreboard players set {} {probe_score} 0",
        plan::PARTY
    ));
    // The real scheduler, the real emitted bundle. Not an inline call: an inline
    // call would run as this test's dummy and pass even with the bug present.
    t.push(format!("schedule function {ns}:{SCHEDULED_PROBE} 2t"));
    t.push(format!(
        "await score {} {probe_score} matches 1",
        plan::PARTY
    ));
    out.insert(
        format!("packtest-datapack/data/{ns}/test/sched_executor.mcfunction"),
        lines(&t).into_bytes(),
    );

    // --- 2. the content path: a move-npc arrival that sets a flag --------
    let arrival = moves.iter().find_map(|m| {
        all_campaign_effects(plan.campaign)
            .into_iter()
            .find_map(|e| match e {
                QuestEffect::MoveNpc {
                    npc,
                    to_anchor,
                    on_arrive,
                    ..
                } if npc.as_str() == m.npc && to_anchor.as_str() == m.to_anchor => on_arrive
                    .iter()
                    .find_map(|a| match a {
                        QuestEffect::SetFlag { flag, .. } => Some(flag.as_str().to_string()),
                        _ => None,
                    })
                    .map(|flag| (m, flag)),
                _ => None,
            })
    });
    let Some((m, flag)) = arrival else { return };
    let bare = movenpc_bare(&m.npc, &m.to_anchor, &m.gate_key);
    let score = plan::flag_score(&flag);

    // The walk is real, so the test must outlive it: the driver reschedules
    // itself once per waypoint tick.
    let mut t = vec![
        format!(
            "#> {title}: move-npc `{}` arrival sets `{flag}` through its SCHEDULED driver",
            m.npc
        ),
        "# @dummy".to_string(),
        format!("# @timeout {}", m.ticks() + 100),
        String::new(),
    ];
    t.push(format!("function {ns}:setup"));
    // Own init: clear the party flag this template alone awaits, and release the
    // driver's re-entry latch (a sibling template may have left it armed).
    t.push(format!("scoreboard players set {} {score} 0", plan::PARTY));
    t.push(format!("scoreboard players set #mrun_{bare} dw.sys 0"));
    // The REAL start function: it schedules `mv_tick_<bare>`, which walks itself
    // to the final waypoint and fires `mv_arrive_<bare>` — every hop through the
    // scheduler, with the server command source the bug hid behind. The dummy
    // stands still throughout; nothing here supplies it as an executor.
    t.push(format!(
        "function {ns}:{}",
        movenpc_fn(&m.npc, &m.to_anchor, &m.gate_key)
    ));
    t.push(format!("await score {} {score} matches 1", plan::PARTY));
    out.insert(
        format!("packtest-datapack/data/{ns}/test/sched_arrive_flag.mcfunction"),
        lines(&t).into_bytes(),
    );
}

/// The dialogue-trigger re-arm PackTest: a player consumes a dialogue trigger and
/// must be able to use it again **with the tick function never running in
/// between**. Suppressing the tick function is how a plain mcfunction emulates the
/// integrated (singleplayer) server's pause-menu tick freeze (1.21.9+), which is
/// the only condition under which the old per-tick-only re-enable lost a dialogue
/// choice — and which a dedicated server, and therefore every rung of the
/// validation ladder, can never enter.
///
/// Drives a **terminal** option (no `next`, no flag gate) so the handler contains
/// no `dialog show` — a PackTest dummy player has no client to show a screen to.
/// The re-arm is emitted immediately after the trigger reset, so it is reached on
/// every path through the handler regardless. Emits nothing when the campaign has
/// no terminal option (nothing to drive).
fn emit_dialogue_trigger_packtest(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let Some((npc, opt)) = plan.npcs.iter().find_map(|npc| {
        npc.options
            .iter()
            .find(|o| o.next.is_none() && o.requires_flags.is_empty() && o.forbids_flags.is_empty())
            .map(|o| (npc, o))
    }) else {
        return;
    };
    let trig = &npc.trigger_objective;
    let n = opt.n;

    let (pin, sel) = pin_dummy("dw_t_rearm");
    let mut b = packtest_header(&format!(
        "{title}: dialogue trigger re-arms without a tick (singleplayer pause parity)"
    ));
    b.push(format!("function {ns}:setup"));
    // Pin this test's own dummy (see `pin_dummy`) and drive/assert on it alone.
    b.push(pin);
    b.push("# The per-tick re-enable, run ONCE. Nothing below runs the tick".to_string());
    b.push("# function again: that suppression IS the integrated server's".to_string());
    b.push("# pause-menu tick freeze, which a dedicated server never enters.".to_string());
    b.push(format!("scoreboard players enable {sel} {trig}"));
    b.push(format!("execute as {sel} run trigger {trig} set {n}"));
    b.push(format!("assert score {sel} {trig} matches {n}"));
    b.push("# The tick's dispatch, hand-run: the handler consumes (and locks) the".to_string());
    b.push("# trigger, then must re-arm it itself.".to_string());
    b.push(format!(
        "execute as {sel} run function {ns}:dlg_{}_{n}",
        npc.safe
    ));
    b.push("# Second use, still with no tick in between. If the handler did not".to_string());
    b.push("# re-arm, vanilla rejects this and the score stays unset.".to_string());
    b.push(format!("execute as {sel} run trigger {trig} set {n}"));
    b.push(format!("assert score {sel} {trig} matches {n}"));

    out.insert(
        format!("packtest-datapack/data/{ns}/test/dialogue_trigger_rearm.mcfunction"),
        lines(&b).into_bytes(),
    );
}

/// spec-0021 loot PackTest: after `setup`, the declared container really holds
/// the declared stacks.
///
/// The contract worth proving on a real server is that `item replace block …
/// container.<n>` LANDED — it is the command that fails silently on a
/// non-container, and `DW0431` proves the container exists at compile time but
/// cannot prove the fill took. Asserts the first and last slot of the first
/// declared fill, by id, so a positional-slot regression is caught too.
/// Emitted only for a campaign that declares `loot` (else byte-identical).
/// The cast-ledger PackTests (spec-0020 acceptance): the root swap is observable,
/// a bark pool cycles deterministically, and a `"none"` scene consumes the
/// interaction without opening anything.
///
/// All three drive `cast_<npc>` (pure scoreboard math) rather than `talk_<npc>`
/// wherever a dialogue root is involved — a dummy player has no client to show a
/// dialog to. The `"none"` test is the exception and drives `talk_<npc>` itself,
/// precisely because a silent scene emits no `dialog show`: that is what makes
/// "the record is written and consumed, and nothing opens" directly assertable.
/// Pin every branch-gate flag an NPC's cast ledger reads to the value that
/// selects `clause`: its `requires_flags` to 1, every other flag any clause
/// reads to 0 (island r15).
///
/// The generated cast templates zero every `dw.qa_*` their dispatch reads but
/// used to leave the ledger's `requires_flags`/`forbids_flags` to whatever the
/// batch had: three sibling templates (`verb_flag_gate`, `verb_interact`,
/// `verb_interact_arming`) legitimately end with a campaign flag set to 1, so
/// whichever ran first poisoned `cast_root_swap`'s later assert — the flee
/// clause overrode the expected scene (expected `dw.cast 2`, got 3) purely by
/// batch order. Pinning at the CONSUMER is the generator-side defense: it holds
/// against any future flag-setting template, rather than trusting each one to
/// clean up. It is also what makes a `requires_flags`-gated clause assertable
/// at all — "never set" is not 1 on the shared server any more than it is 0.
/// Emits nothing for a ledger with no branch-gated clause.
fn pin_cast_clause_flags(
    b: &mut Vec<String>,
    cast: &crate::cast::NpcCast,
    clause: &crate::cast::CastClause,
) {
    let flags: BTreeSet<&str> = cast
        .by_quest
        .iter()
        .flat_map(|cl| cl.requires_flags.iter().chain(cl.forbids_flags.iter()))
        .map(|s| s.as_str())
        .collect();
    if flags.is_empty() {
        return;
    }
    b.push("# Branch-gate flags are batch state a sibling template may have".to_string());
    b.push("# left set (island r15: a verb template ended with its flag at 1".to_string());
    b.push("# and the sibling clause overrode this assert). Pin every flag the".to_string());
    b.push("# ledger reads to the value that selects the asserted scene.".to_string());
    for f in flags {
        let v = i32::from(clause.requires_flags.iter().any(|r| r == f));
        b.push(format!(
            "scoreboard players set {} {} {v}",
            plan::PARTY,
            plan::flag_score(f)
        ));
    }
}

fn emit_cast_packtests(plan: &Plan, out: &mut BuildOutput) {
    use crate::cast::SceneAction;
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let casts = crate::cast::npc_casts(plan.campaign);

    // --- root swap: one NPC whose ledger names two different roots ----------
    let swapper = plan.npcs.iter().find_map(|npc| {
        let cast = casts.get(&npc.npc_id)?;
        let roots: Vec<(u32, &String)> = cast
            .scenes
            .iter()
            .filter_map(|s| match &s.action {
                SceneAction::Root(r) => Some((s.index, r)),
                _ => None,
            })
            .collect();
        if roots.len() < 2 {
            return None;
        }
        // The two quests whose scenes those are, in ledger order.
        let first = cast.by_quest.iter().find(|c| c.scene == roots[0].0)?;
        let later = cast.by_quest.iter().find(|c| c.scene == roots[1].0)?;
        Some((npc, first.clone(), later.clone()))
    });
    if let Some((npc, first, later)) = swapper {
        let (q_first, i_first) = (first.quest.clone(), first.scene);
        let (q_later, i_later) = (later.quest.clone(), later.scene);
        let (pin, sel) = pin_dummy("dw_t_castswap");
        let mut b = packtest_header(&format!(
            "{title}: npc `{}` right-click swaps root as the story advances (cast ledger)",
            npc.npc_id
        ));
        b.push(format!("function {ns}:setup"));
        b.push(pin);
        b.push("# Only the earlier beat has begun: the ledger selects its scene.".to_string());
        for cl in &casts[&npc.npc_id].by_quest {
            b.push(format!(
                "scoreboard players set {} {} 0",
                plan::PARTY,
                quest_active_score(&cl.quest)
            ));
        }
        pin_cast_clause_flags(&mut b, &casts[&npc.npc_id], &first);
        b.push(format!(
            "scoreboard players set {} {} 1",
            plan::PARTY,
            quest_active_score(&q_first)
        ));
        b.push(format!(
            "execute as {sel} run function {ns}:cast_{}",
            npc.safe
        ));
        b.push(format!("assert score {sel} {CAST_SCORE} matches {i_first}"));
        b.push("# The later beat begins. `dw.qa_*` is never cleared, so BOTH are".to_string());
        b.push("# now set — and the later scene must win, retiring the earlier".to_string());
        b.push("# root. That is the whole retirement mechanism.".to_string());
        pin_cast_clause_flags(&mut b, &casts[&npc.npc_id], &later);
        b.push(format!(
            "scoreboard players set {} {} 1",
            plan::PARTY,
            quest_active_score(&q_later)
        ));
        b.push(format!(
            "execute as {sel} run function {ns}:cast_{}",
            npc.safe
        ));
        b.push(format!("assert score {sel} {CAST_SCORE} matches {i_later}"));
        out.insert(
            format!("packtest-datapack/data/{ns}/test/cast_root_swap.mcfunction"),
            lines(&b).into_bytes(),
        );
    }

    // --- bark pool cycles deterministically ---------------------------------
    let barker = plan.npcs.iter().find_map(|npc| {
        let cast = casts.get(&npc.npc_id)?;
        cast.scenes.iter().find_map(|s| match &s.action {
            SceneAction::Barks(pool) if pool.len() >= 2 => Some((npc, s.index, pool.len())),
            _ => None,
        })
    });
    if let Some((npc, scene, n)) = barker {
        let holder = format!("#bk_{}_{scene}", npc.safe);
        let (pin, sel) = pin_dummy("dw_t_castbark");
        let mut b = packtest_header(&format!(
            "{title}: npc `{}` bark pool cycles deterministically through {n} lines",
            npc.npc_id
        ));
        b.push(format!("function {ns}:setup"));
        b.push(pin);
        b.push("# The pool counter is shared runtime state: initialize it.".to_string());
        b.push(format!("scoreboard players set {holder} dw.sys 0"));
        for i in 1..=n {
            b.push(format!(
                "execute as {sel} run function {ns}:bark_{}_{scene}",
                npc.safe
            ));
            b.push(format!("assert score {holder} dw.sys matches {i}"));
        }
        b.push("# One more right-click wraps to the first line — never RNG.".to_string());
        b.push(format!(
            "execute as {sel} run function {ns}:bark_{}_{scene}",
            npc.safe
        ));
        b.push(format!("assert score {holder} dw.sys matches 1"));
        out.insert(
            format!("packtest-datapack/data/{ns}/test/cast_bark_cycle.mcfunction"),
            lines(&b).into_bytes(),
        );
    }

    // --- an explicit `"none"` scene answers with nothing ---------------------
    let silent = plan.npcs.iter().find_map(|npc| {
        let cast = casts.get(&npc.npc_id)?;
        let scene = cast
            .scenes
            .iter()
            .find(|s| s.action == SceneAction::Silent)?;
        let cl = cast.by_quest.iter().find(|c| c.scene == scene.index)?;
        Some((npc, scene.index, cl.clone()))
    });
    if let Some((npc, idx, cl)) = silent {
        let qid = cl.quest.clone();
        let (pin, sel) = pin_dummy("dw_t_castnone");
        let mut b = packtest_header(&format!(
            "{title}: npc `{}`'s `\"none\"` scene consumes the interaction and opens nothing",
            npc.npc_id
        ));
        b.push(format!("function {ns}:setup"));
        b.push(pin);
        for c in &casts[&npc.npc_id].by_quest {
            b.push(format!(
                "scoreboard players set {} {} 0",
                plan::PARTY,
                quest_active_score(&c.quest)
            ));
        }
        pin_cast_clause_flags(&mut b, &casts[&npc.npc_id], &cl);
        b.push(format!(
            "scoreboard players set {} {} 1",
            plan::PARTY,
            quest_active_score(&qid)
        ));
        b.push("# Grant the interaction advancement, exactly as a right-click".to_string());
        b.push("# does: the record is written.".to_string());
        b.push(format!(
            "execute as {sel} run advancement grant @s only {ns}:{}_interact",
            npc.safe
        ));
        b.push("# Run the reward the advancement fires. It must revoke (consume)".to_string());
        b.push("# the record and, for a silent scene, do nothing else.".to_string());
        b.push(format!(
            "execute as {sel} run function {ns}:talk_{}",
            npc.safe
        ));
        b.push(format!("assert score {sel} {CAST_SCORE} matches {idx}"));
        b.push("# The record is consumed, so the advancement is re-armed: a".to_string());
        b.push("# second right-click still works (no dead NPC).".to_string());
        // Vanilla has no `execute if advancement`; the selector argument is the
        // primitive for reading advancement state.
        b.push(format!(
            "execute as {sel} if entity @s[advancements={{{ns}:{}_interact=false}}] run scoreboard players set @s dw.sys 1",
            npc.safe
        ));
        b.push(format!("assert score {sel} dw.sys matches 1"));
        // Deliberately NOT asserted here: that the bark counter did not move.
        // It is a shared runtime holder, and the bark-cycle template drives it
        // over the same ticks — asserting it from two templates is exactly the
        // interleaving dependence `packtest_batch` forbids. "A silent scene
        // emits no action clause" is a property of the emitted text, so it is
        // proved in `tests/cast_emit.rs` where it is race-free by construction.
        out.insert(
            format!("packtest-datapack/data/{ns}/test/cast_none_silent.mcfunction"),
            lines(&b).into_bytes(),
        );
    }
}

fn emit_loot_packtest(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let Some(l) = plan.loot.iter().find(|l| !l.items.is_empty()) else {
        return;
    };
    let c = l.cell;
    let mut b = packtest_header(&format!(
        "{title}: loot `{}` fills its container on init (spec-0021)",
        l.id
    ));
    b.push(format!("function {ns}:setup"));
    // Slot 0 and the last slot: presence AND identity, so neither a dropped
    // fill nor a shifted slot assignment can pass.
    let checks = [
        (0usize, &l.items[0]),
        (l.items.len() - 1, l.items.last().unwrap()),
    ];
    for (n, (slot, it)) in checks.iter().enumerate() {
        b.push(format!(
            "execute store success score #loot{n} dw.sys if data block {} {} {} Items[{{Slot:{}b,id:\"{}\"}}]",
            c[0], c[1], c[2], slot, it.item
        ));
        b.push(format!("assert score #loot{n} dw.sys matches 1"));
    }
    out.insert(
        format!("packtest-datapack/data/{ns}/test/v06_loot.mcfunction"),
        lines(&b).into_bytes(),
    );
}

/// spec-0021 actor-equipment PackTest: an equipped actor's puppet spawns wearing
/// its gear, and the unleashed twin still wears it.
///
/// The handoff is the part worth proving on a real server: `unleash` kills the
/// puppet and summons a fresh entity, so gear that rode only on the puppet would
/// vanish the instant the elite came alive — a regression invisible to any
/// compile-time check. Emitted only for a campaign with an equipped actor.
fn emit_actor_equipment_packtest(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let Some(a) = plan
        .campaign
        .quests
        .content
        .actors
        .iter()
        .find(|a| a.equipment.is_some() && a.skin.is_none())
    else {
        return;
    };
    // The slot the assertion reads: prefer a hand, else the first armour piece.
    let eq = a.equipment.as_ref().expect("filtered on Some");
    let probe: Option<(&str, &EquipItem)> = [
        ("mainhand", eq.main_hand.as_ref()),
        ("offhand", eq.off_hand.as_ref()),
        ("head", eq.head.as_ref()),
        ("chest", eq.chest.as_ref()),
        ("legs", eq.legs.as_ref()),
        ("feet", eq.feet.as_ref()),
    ]
    .into_iter()
    .find_map(|(slot, p)| p.map(|p| (slot, p)));
    let Some((slot, piece)) = probe else {
        return;
    };
    let safe = plan::safe_local(a.id.as_str());
    let mut b = packtest_header(&format!(
        "{title}: actor `{}` keeps its gear across unleash (spec-0021)",
        a.id
    ));
    b.push(format!("function {ns}:setup"));
    // Clean slate: the shared batch server may already carry this actor.
    b.push(format!("kill @e[tag=dw_actor_{safe}]"));
    b.push(format!("function {ns}:spawn_actor_{safe}"));
    b.push(format!(
        "execute store success score #aeqp dw.sys if data entity @e[tag=dw_pup_{safe},limit=1] equipment.{slot}{{id:\"{}\"}}",
        piece.item()
    ));
    b.push("assert score #aeqp dw.sys matches 1".to_string());
    b.push(format!("function {ns}:unleash_{safe}"));
    // The twin is the actor-tagged entity that is NOT the puppet.
    b.push(format!(
        "execute store success score #aeqt dw.sys if data entity @e[tag=dw_actor_{safe},tag=!dw_pup_{safe},limit=1] equipment.{slot}{{id:\"{}\"}}",
        piece.item()
    ));
    b.push("assert score #aeqt dw.sys matches 1".to_string());
    b.push(format!("kill @e[tag=dw_actor_{safe}]"));
    out.insert(
        format!("packtest-datapack/data/{ns}/test/v06_actor_equipment.mcfunction"),
        lines(&b).into_bytes(),
    );
}

/// v0.6 trap PackTests (spec-0011). A fake player in a 0-player void does not tick
/// entities (the primed-TNT fuse and falling-sand freeze — see the spec's Findings),
/// so a plate → dispenser fire cannot be simulated headlessly; runtime firing
/// coverage is a GameTest concern. What is deterministically checkable in a plain
/// mcfunction — and what these assert — is the compiler's own contract: after
/// `setup`, the trap dispenser holds exactly the declared payload; after the disarm
/// function runs, the payload is gone (the modeled global disarm) and the disarm
/// flag is set. This is the machine-checkable half of acceptance criteria 3 & 4;
/// the plate-fires-and-hits half is the PackTest/GameTest layer the spec records as
/// entity-tick-limited.
fn emit_trap_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);

    // Pick the first trap that has both a dispenser payload and a disarm — it
    // exercises both the fill and the empty in one test. Else the first payload trap.
    let dispense_trap = plan
        .traps
        .iter()
        .find(|t| t.dispenser.is_some() && t.payload.is_some());
    let Some(t) = dispense_trap else {
        return;
    };
    let disp = t.dispenser.expect("filtered on Some");
    let (item, count) = t.payload.as_ref().expect("filtered on Some");
    let dis = t.disarm.as_ref();

    let mut b = packtest_header(&format!(
        "{title}: trap `{}` loads its dispenser payload; disarm empties it (spec-0011)",
        t.id
    ));
    b.push(format!("function {ns}:setup"));
    // A 0-player void does not tick entities, so a plate→dispenser fire cannot be
    // simulated here (spec-0011 Findings). Instead place the dispenser and load it
    // with the exact payload the compiler fills, then assert slot 0 is occupied —
    // the machine-checkable "payload lands" contract.
    b.push(format!(
        "setblock {} {} {} minecraft:dispenser",
        disp[0], disp[1], disp[2]
    ));
    b.push(format!(
        "item replace block {} {} {} container.0 with {item} {count}",
        disp[0], disp[1], disp[2]
    ));
    b.push(format!(
        "execute store success score #tload_trap dw.sys if data block {} {} {} Items[0]",
        disp[0], disp[1], disp[2]
    ));
    b.push("assert score #tload_trap dw.sys matches 1".to_string());
    if let Some(dis) = dis {
        // Run the REAL emitted disarm and assert the dispenser is now empty (no ammo
        // → cannot fire) and the disarm flag is set — the trap is provably off.
        // spec-0018: a disarm is a party fact (one lever, everyone's trap off), so
        // the baseline and the assert read `#party`. Cleared first: "never set" is
        // not 0 on the shared-batch server.
        b.push(format!(
            "scoreboard players set {} {} 0",
            plan::PARTY,
            plan::flag_score(&dis.sets_flag)
        ));
        b.push(format!("function {ns}:trap_disarm_{}", t.safe));
        b.push(format!(
            "execute store success score #tempty_trap dw.sys if data block {} {} {} Items[0]",
            disp[0], disp[1], disp[2]
        ));
        b.push("assert score #tempty_trap dw.sys matches 0".to_string());
        b.push(format!(
            "assert score {} {} matches 1",
            plan::PARTY,
            plan::flag_score(&dis.sets_flag)
        ));
    }
    out.insert(
        format!("packtest-datapack/data/{ns}/test/v06_trap.mcfunction"),
        lines(&b).into_bytes(),
    );
    emit_trap_gate_packtest(plan, out);
}

/// spec-0022 PackTests: the **saturation contract** and the collapse, asserted
/// on a live pinned server.
///
/// The volley test is the runtime half of the owner's ruling. It
/// runs the REAL emitted salvo function and then asserts, per standable
/// kill-zone cell, that a projectile exists on the exact trajectory that reaches
/// it — so "the volley blankets its zone" is checked in the game, not just in
/// the compiler. A dummy is parked in one cell, and then MOVED to another
/// between salvos, and each time the occupied cell must show the extra aimed
/// shot on top of the saturation one.
///
/// Assertions are **presence/count based, never "kill everything first"**: this
/// suite runs as one batch on a shared server, so a template that cleared all
/// arrows would sabotage its neighbours. Counting a trajectory that only this
/// volley can produce is residue-robust.
///
/// Boundary worth stating: impact DAMAGE cannot be asserted here. The template
/// harness is synchronous (its only directives are `@dummy` / `@timeout`, with
/// no wait primitive), and an arrow needs ticks of flight to land. What the
/// compiler pins instead is everything damage is a function of — `NoGravity` so
/// the flight path is the proven straight segment, `crit:0b` so the roll is not
/// random, and the exact `Motion` magnitude — leaving the landed-damage check to
/// the tier-3 bot playthrough.
fn emit_payload_packtests(plan: &Plan, out: &mut BuildOutput, payloads: &PayloadPlans) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);

    if let Some(v) = payloads.volleys.first() {
        let base = format!("volley_{}", v.key);
        let tag = "dw_pt_volley";
        let mut b = packtest_header(&format!(
            "{title}: a volley saturates every standable cell of its kill zone (spec-0022)"
        ));
        b.push(format!("function {ns}:setup"));
        // Pin our own dummy BEFORE any teleport to absolute campaign coords —
        // `@p` would otherwise retarget to a neighbouring test's dummy.
        b.push(format!("tag @p add {tag}"));

        let motion_nbt = |m: [f64; 3]| {
            format!(
                "{{Motion:[{}d,{}d,{}d]}}",
                motion_component(m[0]),
                motion_component(m[1]),
                motion_component(m[2])
            )
        };
        let count_line = |i: usize, m: [f64; 3], holder: &str| {
            format!(
                "execute store result score #{holder}{i} dw.sys if entity \
                 @e[type={},nbt={}]",
                "minecraft:arrow",
                motion_nbt(m)
            )
        };
        // Baselines first: "never set" is not 0 on the shared-batch server.
        for (i, shot) in v.geom.shots.iter().enumerate() {
            b.push(count_line(i, shot.motion, "vbase_"));
        }
        // Park the dummy in the FIRST zone cell, then fire salvo 0.
        let c0 = v.geom.shots[0].cell;
        b.push(format!(
            "tp @a[tag={tag}] {} {} {}",
            f64::from(c0[0]) + 0.5,
            c0[1],
            f64::from(c0[2]) + 0.5
        ));
        b.push(format!("function {ns}:{base}_s0"));
        for (i, shot) in v.geom.shots.iter().enumerate() {
            b.push(count_line(i, shot.motion, "vpost_"));
            // Saturation: EVERY cell gains at least one projectile on its own
            // trajectory, whether or not anyone is standing there. This single
            // family of assertions is the ruling.
            b.push(format!(
                "execute store result score #vgain_{i} dw.sys run scoreboard players get \
                 #vpost_{i} dw.sys"
            ));
            b.push(format!(
                "scoreboard players operation #vgain_{i} dw.sys -= #vbase_{i} dw.sys"
            ));
            let want = if i == 0 { 2 } else { 1 };
            b.push(format!("assert score #vgain_{i} dw.sys matches {want}.."));
        }
        // …and MOVING between salvos does not help: the dummy relocates to a
        // different cell and that cell now takes the extra aimed shot too.
        if v.geom.shots.len() > 1 && v.salvos > 1 {
            let c1 = v.geom.shots[1].cell;
            b.push(format!(
                "tp @a[tag={tag}] {} {} {}",
                f64::from(c1[0]) + 0.5,
                c1[1],
                f64::from(c1[2]) + 0.5
            ));
            for (i, shot) in v.geom.shots.iter().enumerate() {
                b.push(count_line(i, shot.motion, "vmid_"));
            }
            b.push(format!("function {ns}:{base}_s1"));
            for (i, shot) in v.geom.shots.iter().enumerate() {
                b.push(count_line(i, shot.motion, "vend_"));
                b.push(format!(
                    "execute store result score #vg2_{i} dw.sys run scoreboard players get \
                     #vend_{i} dw.sys"
                ));
                b.push(format!(
                    "scoreboard players operation #vg2_{i} dw.sys -= #vmid_{i} dw.sys"
                ));
                let want = if i == 1 { 2 } else { 1 };
                b.push(format!("assert score #vg2_{i} dw.sys matches {want}.."));
            }
        }
        out.insert(
            format!("packtest-datapack/data/{ns}/test/v06_volley.mcfunction"),
            lines(&b).into_bytes(),
        );
    }

    if let Some(c) = payloads.collapses.first() {
        let base = format!("collapse_{}", c.key);
        let mut b = packtest_header(&format!(
            "{title}: a collapse deletes its region and drops it as falling blocks (spec-0022)"
        ));
        b.push(format!("function {ns}:setup"));
        // Baseline the falling-block population, then bring the roof down.
        b.push(
            "execute store result score #cbase dw.sys if entity @e[type=minecraft:falling_block]"
                .to_string(),
        );
        b.push(format!("function {ns}:{base}"));
        b.push(
            "execute store result score #cpost dw.sys if entity @e[type=minecraft:falling_block]"
                .to_string(),
        );
        b.push("scoreboard players operation #cpost dw.sys -= #cbase dw.sys".to_string());
        b.push(format!(
            "assert score #cpost dw.sys matches {}..",
            c.geom.drops.len()
        ));
        // The region is genuinely gone — this is what the completability proof
        // reasoned about, so it has to be true in the world too.
        for cell in &c.geom.drops {
            b.push(format!(
                "assert block {} {} {} minecraft:air",
                cell[0], cell[1], cell[2]
            ));
        }
        out.insert(
            format!("packtest-datapack/data/{ns}/test/v06_collapse.mcfunction"),
            lines(&b).into_bytes(),
        );
    }
}

/// v0.6 trap **flag-gate** PackTest (spec-0011): the gate physically removes and
/// restores the trigger hardware, so the machine-checkable contract is the block
/// itself — while the gate is shut the trigger cell is air (a player stepping there
/// touches nothing), and when it opens the authored trigger is back, verbatim.
///
/// This is the assertion the feature never had: `requires_flags`/`forbids_flags`
/// were validated and planned but read by no emission site at all, so the
/// documented "inactive while the flag is set" behaviour simply did not exist.
fn emit_trap_gate_packtest(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    // The first trap gated by a single forbidding flag, which is the shape that can
    // be driven from a test: set the flag → shut, clear it → open.
    let Some(t) = plan
        .traps
        .iter()
        .find(|t| t.requires_flags.is_empty() && t.forbids_flags.len() == 1)
    else {
        return;
    };
    let flag = plan::flag_score(&t.forbids_flags[0]);
    let c = t.trigger_cell;
    let (pin, sel) = pin_dummy("dw_t_tgate");
    let mut b = packtest_header(&format!(
        "{title}: trap `{}` is physically disarmed while `{}` is set (spec-0011)",
        t.id, t.forbids_flags[0]
    ));
    b.push(format!("function {ns}:setup"));
    b.push(pin);
    // Start from the armed world the setup leaves behind, then shut the gate by
    // setting the flag and running the real emitted tick clause path.
    b.push(format!("function {ns}:trap_gate_on_{}", t.safe));
    b.push(format!("scoreboard players set {sel} {flag} 1"));
    b.push(format!("function {ns}:trap_gate_off_{}", t.safe));
    b.push(format!(
        "execute store success score #tgate dw.sys if block {} {} {} minecraft:air",
        c[0], c[1], c[2]
    ));
    b.push("assert score #tgate dw.sys matches 1".to_string());
    // Clear the flag and re-open: the authored trigger must be back in the world.
    b.push(format!("scoreboard players set {sel} {flag} 0"));
    b.push(format!("function {ns}:trap_gate_on_{}", t.safe));
    b.push(format!(
        "execute store success score #tgate dw.sys if block {} {} {} minecraft:air",
        c[0], c[1], c[2]
    ));
    b.push("assert score #tgate dw.sys matches 0".to_string());
    out.insert(
        format!("packtest-datapack/data/{ns}/test/v06_trap_gate.mcfunction"),
        lines(&b).into_bytes(),
    );
}

/// v0.6 night-vision PackTest: a dummy standing inside a `mitigation:
/// "night-vision"` area actually holds `minecraft:night_vision` after one clock
/// tick, and a dummy far outside the area does not.
///
/// This is the gametest that makes the mitigation un-fakeable end-to-end: the
/// `DW0210` gate keys on the declaration, and this asserts the declaration really
/// puts the effect on a player in the world. Emits nothing for a campaign that
/// declares no mitigation.
fn emit_night_vision_packtest(plan: &Plan, out: &mut BuildOutput) {
    let Some(area) = plan.areas.iter().find(|a| {
        plan.campaign
            .world
            .content
            .areas
            .iter()
            .find(|d| d.id.as_str() == a.area_id)
            .is_some_and(crate::light::area_night_vision)
    }) else {
        return;
    };
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let (min, max) = area.bounds();
    let mid = [
        (min[0] + max[0]) / 2,
        (min[1] + max[1]) / 2,
        (min[2] + max[2]) / 2,
    ];
    let mut b = packtest_header(&format!(
        "{title}: the declared night-vision mitigation really reaches a player in the area"
    ));
    b.push("effect clear @s minecraft:night_vision".to_string());
    // Inside the declared bounds: one tick of the real clock must grant the effect.
    b.push(format!("tp @s {} {} {}", mid[0], mid[1], mid[2]));
    b.push(format!("function {ns}:night_vision_tick"));
    b.push(
        "execute store success score #nv_nvis dw.sys run effect clear @s minecraft:night_vision"
            .to_string(),
    );
    b.push("assert score #nv_nvis dw.sys matches 1".to_string());
    // Far outside: the same clock tick must NOT grant it (the selector is scoped).
    b.push(format!("tp @s {} {} {}", max[0] + 1000, mid[1], mid[2]));
    b.push(format!("function {ns}:night_vision_tick"));
    b.push(
        "execute store success score #nv_nvis dw.sys run effect clear @s minecraft:night_vision"
            .to_string(),
    );
    b.push("assert score #nv_nvis dw.sys matches 0".to_string());
    out.insert(
        format!("packtest-datapack/data/{ns}/test/v06_night_vision.mcfunction"),
        lines(&b).into_bytes(),
    );
}

/// v0.6 boundary PackTests (spec-0013): a player outside the region is returned to
/// the last checkpoint; a player inside is never moved. Drives the real
/// `boundary_tick` on a dummy — its direct call IS the 1s clock's body, so no
/// schedule wait is needed (well under the 2s acceptance bound). Uses only
/// `assert score` (PackTest-known-good on the validation server): the player's
/// block-x, captured via `data get … Pos[0]`, discriminates the checkpoint from
/// the interior cell, and is robust to teleport centering (both sides floor the
/// same way). Emits nothing when the campaign declares no `boundary`.
/// **The class trigger is one-shot per player** — the seal, proved on a
/// live server.
///
/// `class_apply_<c>` ends in `teleport @s <campaign entry point>`, so a second
/// `/trigger dw.class` mid-run used to re-class whoever ran it AND warp them
/// back to the start of the delve. The compiler now arms the trigger only for a
/// player who has not classed (`class_arm`), so the seal is a property of the
/// emitted pack rather than a rule every caller has to know.
///
/// The template drives the REAL arming path as its own dummy — it never restates
/// the guard, which would prove only its own copy — and takes the one
/// unambiguous read-back vanilla offers for "was this trigger usable": the
/// success of the `trigger` command itself.
///
/// Three claims, in the order that makes them mean something:
///
/// 1. an UNCLASSED player's trigger is armed and works (the seal must not have
///    weakened the first, legitimate class — a template that only proved the
///    "no" would pass just as well against a pack where classing is broken);
/// 2. the apply consumes the trigger and records the class;
/// 3. after it, the arming path runs again and the trigger stays DEAD: the
///    `trigger` command fails, `dw.class` gets no score, so the dispatch cannot
///    fire — same class, same place, measured on the dummy's own `Pos`.
///
/// The dummy is parked away from the entry point before claim 3 precisely so a
/// warp back to it would be visible.
fn emit_class_seal_packtest(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let Some(first) = plan.classes.first() else {
        return;
    };
    let Some(entry) = campaign_spawn(plan) else {
        return;
    };
    // A genuinely DIFFERENT class for the second attempt when the campaign has
    // one, so "the class did not change" is a claim about identity and not only
    // about position.
    let second = plan.classes.get(1).unwrap_or(first);
    // Distinct from the entry cell by construction: this is where a warp would
    // be visible. Reading block-x back the way `emit_boundary_packtest` does
    // makes the assertion robust to teleport centering.
    let probe_x = entry[0] + 32;

    let mut b = packtest_header(&format!(
        "{title}: the class trigger is one-shot — a second `/trigger dw.class` cannot re-class or \
         warp"
    ));
    b.push(format!("function {ns}:setup"));
    // Own init: the batch is one shared server, so "never set" is not 0.
    b.push("scoreboard players reset @s dw.class".to_string());
    b.push("scoreboard players reset @s dw.classed".to_string());

    // --- 1. unclassed: the trigger is armed and the class can be taken --------
    b.push(format!("execute as @s run function {ns}:class_arm"));
    b.push(format!(
        "execute store success score #cls_arm1 dw.sys run trigger dw.class set {}",
        first.n
    ));
    b.push("assert score #cls_arm1 dw.sys matches 1".to_string());

    // --- 2. the apply consumes the trigger and records the class -------------
    b.push(format!("function {ns}:class_apply_{}", first.safe));
    b.push(
        "execute store success score #cls_taken dw.sys if score @s dw.classed matches 1"
            .to_string(),
    );
    b.push("assert score #cls_taken dw.sys matches 1".to_string());
    b.push(
        "execute store success score #cls_left dw.sys if score @s dw.class matches -2147483648.."
            .to_string(),
    );
    b.push("assert score #cls_left dw.sys matches 0".to_string());

    // --- 3. the seal: arm again, and the trigger stays dead ------------------
    // Park the dummy away from the entry the apply teleported it to, so the warp
    // this task exists to kill would move it.
    b.push(format!("tp @s {probe_x} {} {}", entry[1], entry[2]));
    b.push("execute store result score #cls_x dw.sys run data get entity @s Pos[0] 1".to_string());
    // Precondition: the park really landed where it was asked to, so a later
    // equality is a fact about the seal and not about a teleport that no-op'd.
    b.push(format!("assert score #cls_x dw.sys matches {probe_x}"));
    b.push(format!("execute as @s run function {ns}:class_arm"));
    b.push(format!(
        "execute store success score #cls_arm2 dw.sys run trigger dw.class set {}",
        second.n
    ));
    b.push("assert score #cls_arm2 dw.sys matches 0".to_string());
    b.push(
        "execute store success score #cls_left2 dw.sys if score @s dw.class matches -2147483648.."
            .to_string(),
    );
    b.push("assert score #cls_left2 dw.sys matches 0".to_string());
    // …so the dispatch cannot fire: same class score, same place.
    b.push(
        "execute store success score #cls_still dw.sys if score @s dw.classed matches 1"
            .to_string(),
    );
    b.push("assert score #cls_still dw.sys matches 1".to_string());
    b.push("execute store result score #cls_x2 dw.sys run data get entity @s Pos[0] 1".to_string());
    b.push(format!("assert score #cls_x2 dw.sys matches {probe_x}"));

    // The class the player actually wears, when the campaign tags it (the flask
    // path): still the first class, never the second.
    if !plan.flasks().is_empty() {
        let worn = class_tag(&first.safe);
        b.push(format!(
            "execute store success score #cls_worn dw.sys if entity @s[tag={worn}]"
        ));
        b.push("assert score #cls_worn dw.sys matches 1".to_string());
        if second.safe != first.safe {
            let other = class_tag(&second.safe);
            b.push(format!(
                "execute store success score #cls_other dw.sys if entity @s[tag={other}]"
            ));
            b.push("assert score #cls_other dw.sys matches 0".to_string());
        }
    }

    // Leave no residue for the shared batch (pin_dummy rule 3/4): the party-unique
    // kit latches this template's apply may have taken are batch-global.
    for (k, item) in plan.campaign.classes.content.classes[0]
        .kit
        .iter()
        .enumerate()
    {
        if matches!(item.carrier, Some(delvewright_dsl::Carrier::One)) {
            b.push(format!(
                "scoreboard players reset #kit_{}_{k} dw.sys",
                first.safe
            ));
        }
    }
    out.insert(
        format!("packtest-datapack/data/{ns}/test/class_trigger_once.mcfunction"),
        lines(&b).into_bytes(),
    );
}

fn emit_boundary_packtest(plan: &Plan, out: &mut BuildOutput) {
    let Some(region) = playable_region(plan) else {
        return;
    };
    let Some(spawn) = campaign_spawn(plan) else {
        return;
    };
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    // setup_finish (which writes `dw:cp`) is placement-gated and cannot run in a
    // bare PackTest, so seed the same spawn-cell value the real init would write.
    let seed_cp = format!(
        "data modify storage dw:cp pos set value [{}, {}, {}]",
        spawn[0], spawn[1], spawn[2]
    );

    // Return: a dummy far outside the region (x well past the inflated max) is
    // teleported back to the checkpoint's x within one clock tick.
    let out_x = region.max[0] + 1000;
    let mut b = packtest_header(&format!(
        "{title}: a player outside the playable region returns to the last checkpoint"
    ));
    b.push(seed_cp.clone());
    b.push(format!("tp @s {out_x} {} {}", spawn[1], spawn[2]));
    b.push(format!("function {ns}:boundary_tick"));
    b.push(
        "execute store result score #bx_bret dw.sys run data get entity @s Pos[0] 1".to_string(),
    );
    b.push(format!("assert score #bx_bret dw.sys matches {}", spawn[0]));
    out.insert(
        format!("packtest-datapack/data/{ns}/test/v06_boundary_return.mcfunction"),
        lines(&b).into_bytes(),
    );

    // Inside: a dummy at an interior cell distinct from the checkpoint is untouched.
    let in_x = spawn[0] + 5;
    let mut b = packtest_header(&format!(
        "{title}: a player inside the playable region is never moved"
    ));
    b.push(seed_cp);
    b.push(format!("tp @s {in_x} {} {}", spawn[1], spawn[2]));
    // Precondition: the interior cell really is inside the region (else the geometry
    // is too small — fail informatively rather than silently pass).
    b.push(
        "execute store result score #px_bins dw.sys run data get entity @s Pos[0] 1".to_string(),
    );
    b.push(format!(
        "assert score #px_bins dw.sys matches {}..{}",
        region.min[0], region.max[0]
    ));
    b.push(format!("function {ns}:boundary_tick"));
    b.push(
        "execute store result score #bx_bins dw.sys run data get entity @s Pos[0] 1".to_string(),
    );
    b.push(format!("assert score #bx_bins dw.sys matches {in_x}"));
    out.insert(
        format!("packtest-datapack/data/{ns}/test/v06_boundary_inside.mcfunction"),
        lines(&b).into_bytes(),
    );
}

/// spec-0016 §1 bonfire PackTests. A fake player cannot die and respawn inside a
/// plain mcfunction, so — like the spec-0012 checkpoint test — these drive the
/// REAL generated `bonfire_rest_<i>` and assert its two machine-checkable
/// contracts:
///
/// * **rest moves the checkpoint**: after the rest function runs, `storage dw:cp
///   pos` reads back the bonfire cell (the mirror every other feature consumes,
///   spec-0013's boundary return included).
/// * **rest re-seats the wave**: a `respawns_on_rest` wave that was spawned and
///   then wiped is standing again after a rest, at its authored count — and a
///   wave the party never met (seated sentinel unset) is NOT summoned by a rest,
///   which is the whole point of the sentinel.
///
/// Emits nothing for a campaign with no bonfire → byte-identical.
/// The wave census counts by TAG, proven on a live server.
///
/// The ladder's old probe counted silhouettes — every entity the client tracked,
/// anything taller than half a block — so an ambush actor standing near the fight
/// was indistinguishable from a member of it, and one alive on both sides of a
/// scripted death was reported as a survivor the re-seat had failed to remove.
/// The count lives in the datapack, where the tag lives; this
/// template is what proves the arithmetic on the pinned server rather than in a
/// unit test's imagination.
///
/// Four claims: an untagged bystander standing right there is NOT counted; a
/// branded mob is; a mob that never wore the brand is not; and a wounded mob is
/// reported wounded, from the server's own `Health` and `max_health`.
///
/// Emits nothing for a campaign with no wave → byte-identical.
fn emit_wave_census_packtest(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let Some(w) = plan.campaign.quests.content.waves.first() else {
        return;
    };
    // A wave the compiler could not place emits no `spawn_<wave>` to drive.
    if plan::wave_total(w) < 1 {
        return;
    }
    let safe = plan::safe_local(w.id.as_str());
    let tag = plan::wave_tag(w.id.as_str());
    let brand = plan::wave_brand_tag(w.id.as_str());
    let total = plan::wave_total(w);
    let species = &w.mobs[0].entity;

    let mut b = packtest_header(&format!(
        "{title}: the census counts wave `{}` by TAG — a bystander beside it is not in it",
        w.id
    ));
    b.push(format!("function {ns}:setup"));
    b.push(format!("kill @e[tag={tag}]"));
    b.push("kill @e[tag=dw_cen_bystander]".to_string());
    b.push(format!("function {ns}:spawn_{safe}"));
    // A BYSTANDER of the wave's own species, summoned on the wave's own anchor
    // cell: everything a silhouette probe uses to decide membership, and none of
    // what the census uses. It must not move a single count.
    b.push(format!(
        "execute at @e[tag={tag},limit=1] run summon {species} ~ ~ ~ \
         {{Tags:[\"dw_cen_bystander\"],PersistenceRequired:1b}}"
    ));
    b.push(format!("function {ns}:wave_census_{safe}"));
    b.push(format!("assert score #wcen_n dw.sys matches {total}"));
    b.push("assert score #wcen_b dw.sys matches 0".to_string());
    b.push("assert score #wcen_d dw.sys matches 0".to_string());
    // Brand this life's mobs. The bystander is not one of them, and the brand
    // rides the wave tag, so it cannot reach it.
    b.push(format!("function {ns}:wave_brand_{safe}"));
    b.push(format!("function {ns}:wave_census_{safe}"));
    b.push(format!("assert score #wcen_b dw.sys matches {total}"));
    b.push(format!(
        "execute store result score #cen_by dw.sys if entity @e[tag=dw_cen_bystander,tag={brand}]"
    ));
    b.push("assert score #cen_by dw.sys matches 0".to_string());
    // Wound one, and the census says so — read off the server's own Health and
    // max_health, not a table and not whatever the client was sent.
    b.push(format!(
        "data modify entity @e[tag={tag},limit=1] Health set value 1.0f"
    ));
    b.push(format!("function {ns}:wave_census_{safe}"));
    b.push("assert score #wcen_d dw.sys matches 1".to_string());
    // A re-summon is a NEW mob: the brand cannot survive it, which is exactly the
    // property the die-retry fidelity verdict rests on.
    b.push(format!("kill @e[tag={tag}]"));
    b.push(format!("function {ns}:spawn_{safe}"));
    b.push(format!("function {ns}:wave_census_{safe}"));
    b.push(format!("assert score #wcen_n dw.sys matches {total}"));
    b.push("assert score #wcen_b dw.sys matches 0".to_string());
    b.push("assert score #wcen_d dw.sys matches 0".to_string());
    // Leave no residue for the shared batch (pin_dummy rule 4).
    b.push(format!("function {ns}:wave_unbrand_{safe}"));
    b.push(format!("kill @e[tag={tag}]"));
    b.push("kill @e[tag=dw_cen_bystander]".to_string());
    out.insert(
        format!("packtest-datapack/data/{ns}/test/wave_census.mcfunction"),
        lines(&b).into_bytes(),
    );
}

fn emit_bonfire_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let Some(bf) = plan.bonfires().next() else {
        return;
    };
    let i = bf.index;
    let [x, y, z] = bf.pos;

    // --- rest moves the party checkpoint ---
    let mut b = packtest_header(&format!(
        "{title}: resting at a bonfire moves the party checkpoint (spec-0016 §1)"
    ));
    b.push(format!("function {ns}:setup"));
    // Scrub the shared mirror to a value the assert cannot pass by accident, then
    // run the real rest and read it back per-axis.
    b.push("data modify storage dw:cp pos set value [0, 0, 0]".to_string());
    b.push(format!("function {ns}:bonfire_rest_{i}"));
    for (axis, want) in [(0, x), (1, y), (2, z)] {
        b.push(format!(
            "execute store result score #bc{axis}_bfr dw.sys run data get storage dw:cp pos[{axis}]"
        ));
        b.push(format!("assert score #bc{axis}_bfr dw.sys matches {want}"));
    }
    out.insert(
        format!("packtest-datapack/data/{ns}/test/souls_bonfire_rest.mcfunction"),
        lines(&b).into_bytes(),
    );

    // --- rest re-seats a wave the party has met, and only that wave ---
    let reseat = plan.reseat_waves();
    let Some(w) = reseat.first() else {
        return;
    };
    let tag = plan::wave_tag(w.id.as_str());
    let safe = plan::safe_local(w.id.as_str());
    let seated = wave_seated_holder(w.id.as_str());
    let total = plan::wave_total(w);
    let mut b = packtest_header(&format!(
        "{title}: a bonfire rest re-seats wave `{}` — but only once met (spec-0016 §1)",
        w.id
    ));
    b.push(format!("function {ns}:setup"));
    // Entity + score residue from a sibling template is batch-global: clear both.
    b.push(format!("kill @e[tag={tag}]"));
    b.push(format!("scoreboard players set {seated} dw.sys 0"));
    // Unmet wave: a rest must NOT conjure it.
    b.push(format!("function {ns}:bonfire_rest_{i}"));
    b.push(format!(
        "execute store result score #bu_bfs dw.sys if entity @e[tag={tag}]"
    ));
    b.push("assert score #bu_bfs dw.sys matches 0".to_string());
    // Met wave: spawn it, wipe it, rest — it stands again at the authored count.
    b.push(format!("function {ns}:spawn_{safe}"));
    b.push(format!("assert score {seated} dw.sys matches 1"));
    b.push(format!("kill @e[tag={tag}]"));
    b.push(format!(
        "execute store result score #bw_bfs dw.sys if entity @e[tag={tag}]"
    ));
    b.push("assert score #bw_bfs dw.sys matches 0".to_string());
    b.push(format!("function {ns}:bonfire_rest_{i}"));
    b.push(format!(
        "execute store result score #br_bfs dw.sys if entity @e[tag={tag}]"
    ));
    b.push(format!("assert score #br_bfs dw.sys matches {total}"));
    // --- the SURVIVOR case ---
    // A wiped wave coming back proves the count. It does not prove the thing the
    // ruling is actually about: grinding a wave down one hit per life must never
    // be a valid path, so a survivor the party chipped has to be REMOVED and
    // replaced, not topped up and not left standing. Chip one mob to a sliver,
    // brand it with a tag no re-summon can carry (`spawn_<wave>` writes the
    // authored NBT and nothing else), rest, and demand the brand is gone while the
    // wave stands at full count. Identity, not just arithmetic.
    b.push(format!(
        "data modify entity @e[tag={tag},limit=1] Health set value 1.0f"
    ));
    b.push(format!("tag @e[tag={tag},limit=1] add dw_bfchip"));
    b.push("execute store result score #bp_bfs dw.sys if entity @e[tag=dw_bfchip]".to_string());
    b.push("assert score #bp_bfs dw.sys matches 1".to_string());
    b.push(format!("function {ns}:bonfire_rest_{i}"));
    b.push("execute store result score #bc_bfs dw.sys if entity @e[tag=dw_bfchip]".to_string());
    b.push("assert score #bc_bfs dw.sys matches 0".to_string());
    b.push(format!(
        "execute store result score #bf_bfs dw.sys if entity @e[tag={tag}]"
    ));
    b.push(format!("assert score #bf_bfs dw.sys matches {total}"));
    // Leave no residue for the rest of the batch (pin_dummy rule 4).
    b.push(format!("kill @e[tag={tag}]"));
    b.push(format!("scoreboard players set {seated} dw.sys 0"));
    out.insert(
        format!("packtest-datapack/data/{ns}/test/souls_bonfire_reseat.mcfunction"),
        lines(&b).into_bytes(),
    );
}

/// spec-0016 §1: **a re-seated wave comes back
/// STATIONED.**
///
/// The souls loop stands — a beaten `respawns_on_rest` wave does return on a rest
/// and on a death-respawn — but it returns to the state it was FIRST seated in,
/// never to the state the party last left it in. A lane wave re-enters its routed
/// patrol from the lane start (`Patrolling:1b` re-applied, `patrol_target` back on
/// waypoint 0, the march clock back to index 0); a non-lane wave stands at its
/// anchor under vanilla-local AI with no patrol NBT at all. Nothing re-seated may
/// pursue across the map.
///
/// The engine already satisfies this by construction — the re-seat re-enters
/// through the wave's own `spawn_<wave>`, and every piece of stationed state is
/// written there — but "by construction" is folklore until a server says so. This
/// template makes it a live claim, and it is deliberately driven from the WORST
/// state the wave can be in: dragged off its lane onto the party, released to
/// native AI by the real lane clock, its march clock run down the lane, and every
/// mob branded so a survivor cannot hide inside a correct-looking count. Then it
/// runs the REAL `bonfire_rest_<i>` and demands four things:
///
/// 1. the authored count is standing;
/// 2. **not one mob of the previous life is** — the brand is gone, so this is a
///    fresh squad, not the chased one topped up;
/// 3. every mob is back at its seating footing, within the compiler-known spread
///    of the wave's own first seated cell (this is the anti-pursuit claim: the
///    mobs were 0 blocks from the player a moment ago);
/// 4. the routed state is re-asserted (lane) or absent (non-lane).
///
/// Emits nothing without both a bonfire and a `respawns_on_rest` wave →
/// byte-identical.
fn emit_reseat_stationed_packtest(
    plan: &Plan,
    out: &mut BuildOutput,
    wave_placements: &WavePlacements,
    lane_routes: &crate::nav::LaneRoutes,
) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let Some(bf) = plan.bonfires().next() else {
        return;
    };
    let reseat = plan.reseat_waves();
    // Prefer a LANE wave when the campaign has one: its stationed state is the
    // richer claim (the routed half), and the plain-anchor half is a subset of it.
    let Some(w) = reseat
        .iter()
        .find(|w| lane_routes.contains_key(w.id.as_str()))
        .or_else(|| reseat.first())
        .copied()
    else {
        return;
    };
    let Some(cells) = wave_placements.get(w.id.as_str()) else {
        return;
    };
    let Some(&seat) = cells.first() else {
        return;
    };
    let total = plan::wave_total(w);
    if total < 1 {
        return;
    }
    let i = bf.index;
    let safe = plan::safe_local(w.id.as_str());
    let tag = plan::wave_tag(w.id.as_str());
    let brand = plan::wave_brand_tag(w.id.as_str());
    let lane = lane_routes.get(w.id.as_str());
    // How far the wave's own seating spreads from its first cell, rounded up: the
    // exact radius the compiler placed this wave inside, so the proximity claim is
    // as tight as the geometry allows rather than a guessed slack.
    let spread = cells
        .iter()
        .map(|c| {
            (0..3)
                .map(|k| f64::from(c[k] - seat[k]).powi(2))
                .sum::<f64>()
                .sqrt()
        })
        .fold(0.0_f64, f64::max)
        .ceil()
        .max(1.0) as i64;
    let (pin, sel) = pin_dummy("dw_rsst");

    let mut b = packtest_header(&format!(
        "{title}: a bonfire re-seat returns wave `{}` to its STATIONED state, never to the \
         chase (spec-0016 §1)",
        w.id
    ));
    b.push(format!("function {ns}:setup"));
    b.push(pin);
    // A rest re-seats EVERY met wave, so this template owns the whole re-seat
    // board: clear each one's entities and its seated sentinel first, and put
    // both back at the end (pin_dummy rule 4).
    for r in &reseat {
        b.push(format!("kill @e[tag={}]", plan::wave_tag(r.id.as_str())));
        b.push(format!(
            "scoreboard players set {} dw.sys 0",
            wave_seated_holder(r.id.as_str())
        ));
    }
    // Meet the wave.
    b.push(format!("function {ns}:spawn_{safe}"));
    b.push(format!(
        "assert score {} dw.sys matches 1",
        wave_seated_holder(w.id.as_str())
    ));
    // Now put it in the state the drowned bell's ladder actually died to: the
    // squad on top of the party, off its lane, feral.
    b.push(format!("execute at {sel} run tp @e[tag={tag}] ~ ~ ~"));
    if let Some(wps) = lane {
        b.push(format!("function {ns}:lane_tick_{safe}"));
        b.push(format!(
            "execute store result score #f_rsst dw.sys if entity @e[tag={tag},nbt={{Patrolling:0b}}]"
        ));
        b.push(format!("assert score #f_rsst dw.sys matches {total}"));
        // …and its march clock run down the lane, so a re-seat that forgot to
        // reset it would send the fresh squad at the lane's far end.
        b.push(format!(
            "scoreboard players set {} dw.sys {}",
            lane_index_holder(w.id.as_str()),
            wps.len().saturating_sub(1)
        ));
    }
    // Brand this life. `spawn_<wave>` writes the authored NBT and nothing else,
    // so no re-summon can carry the stamp: identity, not arithmetic.
    b.push(format!("function {ns}:wave_brand_{safe}"));

    // --- the re-seat, through the real rest function ---
    b.push(format!("function {ns}:bonfire_rest_{i}"));
    b.push(format!(
        "execute store result score #n_rsst dw.sys if entity @e[tag={tag}]"
    ));
    b.push(format!("assert score #n_rsst dw.sys matches {total}"));
    b.push(format!(
        "execute store result score #b_rsst dw.sys if entity @e[tag={brand}]"
    ));
    b.push("assert score #b_rsst dw.sys matches 0".to_string());
    // Back on their footing — the anti-pursuit claim.
    let c = ent_xyz(seat);
    b.push(format!(
        "execute positioned {} {} {} store result score #d_rsst dw.sys if entity \
         @e[tag={tag},distance=..{spread}]",
        c[0], c[1], c[2]
    ));
    b.push(format!("assert score #d_rsst dw.sys matches {total}"));
    match lane {
        Some(wps) => {
            b.push(format!(
                "execute store result score #p_rsst dw.sys if entity \
                 @e[tag={tag},nbt={{Patrolling:1b}}]"
            ));
            b.push(format!("assert score #p_rsst dw.sys matches {total}"));
            b.push(format!(
                "execute store result score #t_rsst dw.sys if entity \
                 @e[tag={tag},nbt={{patrol_target:[I;{},{},{}]}}]",
                wps[0][0], wps[0][1], wps[0][2]
            ));
            b.push(format!("assert score #t_rsst dw.sys matches {total}"));
            b.push(format!(
                "assert score {} dw.sys matches 0",
                lane_index_holder(w.id.as_str())
            ));
        }
        None => {
            // Vanilla-local AI only: a non-lane wave is never routed, so patrol
            // NBT must not appear on it — not on the first summon and not on a
            // re-seat.
            b.push(format!(
                "execute store result score #p_rsst dw.sys if entity \
                 @e[tag={tag},nbt={{Patrolling:1b}}]"
            ));
            b.push("assert score #p_rsst dw.sys matches 0".to_string());
        }
    }

    b.push(format!("function {ns}:wave_unbrand_{safe}"));
    for r in &reseat {
        b.push(format!("kill @e[tag={}]", plan::wave_tag(r.id.as_str())));
        b.push(format!(
            "scoreboard players set {} dw.sys 0",
            wave_seated_holder(r.id.as_str())
        ));
    }
    b.push(format!("tag {sel} remove dw_rsst"));
    out.insert(
        format!("packtest-datapack/data/{ns}/test/souls_reseat_stationed.mcfunction"),
        lines(&b).into_bytes(),
    );
}

/// spec-0016 §1: **an undefeated elite is put back;
/// a defeated one stays dead.**
///
/// The bell's round-five playtest found the half of the souls loop nothing was
/// driving. `respawns_on_rest` waves came back correctly — and the barrow-warden,
/// an actor elite the party had woken, wounded and run away from, stayed exactly
/// where the chase ended, at exactly the health the chase left it. So did the
/// ambushers in the sewer and up in the rafters. A rest refreshed the scene
/// around them and not them.
///
/// The two templates here are that scenario, run on the pinned server, in the
/// order it happens: meet the fight, damage it, drag it off its ground, rest —
/// then demand
///
/// 1. it is standing again, exactly one body / the authored count;
/// 2. **not the same body**: every mob of the previous life is branded with a tag
///    no summon can carry, and the brand is gone (identity, not arithmetic — the
///    anti-chip claim);
/// 3. it is back on its own ground, not where combat left it;
/// 4. an actor is put back FREED, never re-caged — a re-caged elite would be
///    dormant scenery for the rest of the delve, because the `unleash-actor` beat
///    that woke it fires from a one-shot trigger;
/// 5. and once actually killed, a rest does NOT bring it back.
///
/// Emits nothing without a bonfire, and nothing for a campaign with no hostile
/// actor and no billed elite/boss wave → byte-identical.
fn emit_reseat_undefeated_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let Some(bf) = plan.bonfires().next() else {
        return;
    };
    let i = bf.index;
    // A rest re-seats every MET `respawns_on_rest` wave too, so both templates own
    // the whole re-seat board: clear each one's entities and its seated sentinel
    // on entry and on exit (pin_dummy rule 4).
    let board: Vec<String> = plan
        .reseat_waves()
        .iter()
        .flat_map(|r| {
            [
                format!("kill @e[tag={}]", plan::wave_tag(r.id.as_str())),
                format!(
                    "scoreboard players set {} dw.sys 0",
                    wave_seated_holder(r.id.as_str())
                ),
            ]
        })
        .collect();

    // --- the actor elite (the barrow-warden's defect) ---
    if let Some(a) = plan
        .reseat_actors()
        .into_iter()
        .find(|a| anchor_point_any(plan, a.anchor.as_str()).is_some())
    {
        let safe = plan::safe_local(a.id.as_str());
        let origin = ent_xyz(anchor_point_any(plan, a.anchor.as_str()).unwrap());
        let (pin, sel) = pin_dummy("dw_rsua");
        let mut b = packtest_header(&format!(
            "{title}: a rest re-seats the undefeated elite `{}` at its origin, and never \
             resurrects a defeated one (spec-0016 §1)",
            a.id
        ));
        b.push(format!("function {ns}:setup"));
        b.push(pin);
        b.extend(board.iter().cloned());
        b.push(format!("kill @e[tag=dw_actor_{safe}]"));
        b.push("kill @e[tag=dw_rsua_brand]".to_string());
        // Meet the fight: stage the puppet, then turn it loose exactly as the
        // campaign's own beat does.
        b.push(format!("function {ns}:spawn_actor_{safe}"));
        b.push(format!("function {ns}:unleash_{safe}"));
        b.push(format!(
            "execute store result score #n_rsua dw.sys if entity @e[tag=dw_actor_{safe}]"
        ));
        b.push("assert score #n_rsua dw.sys matches 1".to_string());
        b.push(format!(
            "execute store result score #q_rsua dw.sys if entity @e[tag=dw_pup_{safe}]"
        ));
        b.push("assert score #q_rsua dw.sys matches 0".to_string());
        // The fight the owner had: the elite chases the party off its ground and
        // is chipped on the way.
        b.push(format!(
            "execute at {sel} run tp @e[tag=dw_actor_{safe}] ~ ~ ~"
        ));
        b.push(format!(
            "data modify entity @e[tag=dw_actor_{safe},limit=1] Health set value 1.0f"
        ));
        b.push(format!("tag @e[tag=dw_actor_{safe}] add dw_rsua_brand"));
        // The rest, through the REAL generated rest function.
        b.push(format!("function {ns}:bonfire_rest_{i}"));
        b.push(format!(
            "execute store result score #a_rsua dw.sys if entity @e[tag=dw_actor_{safe}]"
        ));
        b.push("assert score #a_rsua dw.sys matches 1".to_string());
        b.push(
            "execute store result score #b_rsua dw.sys if entity @e[tag=dw_rsua_brand]".to_string(),
        );
        b.push("assert score #b_rsua dw.sys matches 0".to_string());
        b.push(format!(
            "execute positioned {} {} {} store result score #d_rsua dw.sys if entity \
             @e[tag=dw_actor_{safe},distance=..2]",
            origin[0], origin[1], origin[2]
        ));
        b.push("assert score #d_rsua dw.sys matches 1".to_string());
        // Freed, not re-caged.
        b.push(format!(
            "execute store result score #c_rsua dw.sys if entity @e[tag=dw_pup_{safe}]"
        ));
        b.push("assert score #c_rsua dw.sys matches 0".to_string());
        // Defeated stays dead: no body, nothing to put back.
        b.push(format!("kill @e[tag=dw_actor_{safe}]"));
        b.push(format!("function {ns}:bonfire_rest_{i}"));
        b.push(format!(
            "execute store result score #k_rsua dw.sys if entity @e[tag=dw_actor_{safe}]"
        ));
        b.push("assert score #k_rsua dw.sys matches 0".to_string());
        b.push(format!("kill @e[tag=dw_actor_{safe}]"));
        b.extend(board.iter().cloned());
        b.push(format!("tag {sel} remove dw_rsua"));
        out.insert(
            format!("packtest-datapack/data/{ns}/test/souls_reseat_actor.mcfunction"),
            lines(&b).into_bytes(),
        );
    }

    // --- the billed elite/boss wave (the anti-chip half) ---
    let Some(w) = plan
        .undefeated_reseat_waves()
        .into_iter()
        .find(|w| plan::wave_total(w) >= 1)
    else {
        return;
    };
    let safe = plan::safe_local(w.id.as_str());
    let tag = plan::wave_tag(w.id.as_str());
    let brand = plan::wave_brand_tag(w.id.as_str());
    let total = plan::wave_total(w);
    let mut b = packtest_header(&format!(
        "{title}: a rest re-seats the undefeated boss wave `{}` whole, and never resurrects a \
         beaten one (spec-0016 §1)",
        w.id
    ));
    b.push(format!("function {ns}:setup"));
    b.extend(board.iter().cloned());
    b.push(format!("kill @e[tag={tag}]"));
    b.push(format!("function {ns}:wave_unbrand_{safe}"));
    // Meet it, and grind it down to a sliver — the path the ruling forbids.
    b.push(format!("function {ns}:spawn_{safe}"));
    b.push(format!("function {ns}:wave_brand_{safe}"));
    if total > 1 {
        b.push(format!("kill @e[tag={tag},limit={}]", total - 1));
    }
    b.push(format!(
        "data modify entity @e[tag={tag},limit=1] Health set value 1.0f"
    ));
    b.push(format!(
        "execute store result score #s_rsuw dw.sys if entity @e[tag={tag}]"
    ));
    b.push("assert score #s_rsuw dw.sys matches 1".to_string());
    b.push(format!("function {ns}:bonfire_rest_{i}"));
    b.push(format!(
        "execute store result score #n_rsuw dw.sys if entity @e[tag={tag}]"
    ));
    b.push(format!("assert score #n_rsuw dw.sys matches {total}"));
    b.push(format!(
        "execute store result score #b_rsuw dw.sys if entity @e[tag={brand}]"
    ));
    b.push("assert score #b_rsuw dw.sys matches 0".to_string());
    // Beaten stays beaten: the boss the party actually killed is not conjured back.
    b.push(format!("kill @e[tag={tag}]"));
    b.push(format!("function {ns}:bonfire_rest_{i}"));
    b.push(format!(
        "execute store result score #k_rsuw dw.sys if entity @e[tag={tag}]"
    ));
    b.push("assert score #k_rsuw dw.sys matches 0".to_string());
    b.push(format!("function {ns}:wave_unbrand_{safe}"));
    b.push(format!("kill @e[tag={tag}]"));
    b.extend(board.iter().cloned());
    out.insert(
        format!("packtest-datapack/data/{ns}/test/souls_reseat_undefeated.mcfunction"),
        lines(&b).into_bytes(),
    );
}

/// spec-0016 §1: the **two options really differ**.
///
/// The owner's ruling is that right-clicking a bonfire offers exactly *rest and
/// save* and *save only*, and that save-only does nothing but move the
/// checkpoint. That is a runtime claim about two functions, and this drives both
/// on a live server through the flask — the one restored resource a PackTest
/// dummy can actually observe.
///
/// **Why the flask and not health.** PackTest fake players are immune to
/// `/damage` (measured on the pinned toolserver, 2026-08-03: `Health` stays at
/// 20.0 through `damage @s 1000`), so a dummy can never be *hurt* and therefore
/// never be seen to be *healed* — an assertion on health would be permanently
/// red no matter how correct the engine is. Inventory has no such problem:
/// `clear <player> <item> 0` counts matching items without removing them, so the
/// template can spend the flask down to one, drive each option, and read the
/// count back. The heal/feed/cure half of a rest is proven where it can be proven
/// honestly — compiler unit tests assert the exact `effect` commands and their
/// order inside `bonfire_restore`.
///
/// Emits nothing for a campaign without both a bonfire and a flask, which
/// `DW0476` makes the same thing as "no bonfire" → byte-identical.
fn emit_bonfire_option_packtest(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let Some(bf) = plan.bonfires().next() else {
        return;
    };
    let Some(&(ci, ki)) = plan.flasks().first() else {
        return;
    };
    let i = bf.index;
    let item = &plan.campaign.classes.content.classes[ci].kit[ki];
    let ctag = class_tag(&plan.classes[ci].safe);
    let (pin, sel) = pin_dummy("dw_bfopt");
    // Counting predicate: the flask's own item predicate, so on a
    // contents-bearing flask every count below is of bottles whose
    // `potion_contents` matches EXACTLY. That is what makes this template a
    // proof of round-trip and not merely of arithmetic — a rest that re-gave a
    // differently-filled bottle (or the contents-less placeholder) would leave
    // the exact-match count at 1 while the bare-id count climbed to `count + 1`,
    // and both halves are asserted below.
    let pred = kit_item_predicate(item);
    let comp = kit_item_components(item);

    let mut b = packtest_header(&format!(
        "{title}: save-only saves and nothing else; rest refills the flask (spec-0016 §1)"
    ));
    b.push(format!("function {ns}:setup"));
    b.push(pin);
    // The dummy takes the flask's class, so `bonfire_flask`'s per-class guard
    // selects it — this is the same tag `class_apply_<class>` adds.
    b.push(format!("tag {sel} add {ctag}"));
    // Baseline: exactly ONE flask in the bag (the party has spent the rest),
    // filled exactly as the class kit fills it.
    b.push(format!("clear {sel} {}", item.item));
    b.push(format!("give {sel} {}{comp} 1", item.item));

    // --- save only: the checkpoint moves, the flask does NOT come back ---
    b.push("data modify storage dw:cp pos set value [0, 0, 0]".to_string());
    b.push(format!(
        "execute as {sel} run function {ns}:bonfire_pick_save_{i}"
    ));
    b.push(format!(
        "execute store result score #bo_save dw.sys run clear {sel} {pred} 0"
    ));
    b.push("assert score #bo_save dw.sys matches 1".to_string());
    b.push(
        "execute store result score #bo_cp dw.sys run data get storage dw:cp pos[0]".to_string(),
    );
    b.push(format!("assert score #bo_cp dw.sys matches {}", bf.pos[0]));

    // --- rest and save: the flask is replenished to its declared count ---
    b.push(format!(
        "execute as {sel} run function {ns}:bonfire_pick_rest_{i}"
    ));
    b.push(format!(
        "execute store result score #bo_rest dw.sys run clear {sel} {pred} 0"
    ));
    b.push(format!(
        "assert score #bo_rest dw.sys matches {}",
        item.count
    ));
    // …and nothing ELSE of that item id is in the bag: refilling by handing over
    // a second, differently-filled bottle is the failure this catches.
    b.push(format!(
        "execute store result score #bo_any dw.sys run clear {sel} {} 0",
        item.item
    ));
    b.push(format!(
        "assert score #bo_any dw.sys matches {}",
        item.count
    ));

    // Leave no residue for the shared batch (pin_dummy rule 4).
    b.push(format!("clear {sel} {}", item.item));
    b.push(format!("tag {sel} remove {ctag}"));
    out.insert(
        format!("packtest-datapack/data/{ns}/test/souls_bonfire_options.mcfunction"),
        lines(&b).into_bytes(),
    );
}

/// spec-0016 §4 timed-gate PackTest: the emitted clock really alternates the gate
/// region on a live server. A fake player cannot wait out a `schedule` inside a
/// plain mcfunction, so this drives the two halves of the ping-pong directly —
/// which IS the clock's body — and asserts the region's state after each. That is
/// the machine-checkable half of "a deterministic clock over the gate region";
/// the *timing* half is the compile-time `DW0378` proof, which needs no server.
/// Emits nothing for a campaign with no timed gate.
/// Pin the jam score a disarmable gate's clock is guarded by, so a template never
/// runs against a jam a sibling left behind.
///
/// PackTest shares one server across every generated template and gives no
/// ordering guarantee, so a persistent score is shared mutable state between
/// tests. `souls_timed_gate_disarm` deliberately ends DISARMED — that is its
/// subject — and any sibling that calls `tgate_close_` afterwards finds the call
/// swallowed by the jam guard. Emitting nothing for a gate with no `disarm` keeps
/// those campaigns byte-identical.
fn pin_tgdis(b: &mut Vec<String>, g: &crate::plan::TimedGatePlan) {
    if g.disarm.is_some() {
        b.push(format!("scoreboard players set #tgdis_{} dw.sys 0", g.safe));
    }
}

fn emit_timed_gate_packtest(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let Some(g) = plan.timed_gates.first() else {
        return;
    };
    let (from, to) = g.gate_region;
    let probe = from;
    let mut b = packtest_header(&format!(
        "{title}: timed gate `{}` alternates its region (spec-0016 §4)",
        g.id
    ));
    b.push(format!("function {ns}:setup"));
    // Seal first: `setup` may have already run the clock's opening move, and a
    // sibling template shares this server.
    b.push(format!(
        "fill {} {} {} {} {} {} {}",
        from[0], from[1], from[2], to[0], to[1], to[2], g.gate_block
    ));
    // …and un-jam, for the same reason the fill exists. `souls_timed_gate_disarm`
    // ends with the gate DISARMED and never restores it, `#tgdis_<id>` persists on
    // the shared server, and PackTest does not order siblings — so whenever disarm
    // runs first, this template's `tgate_close_` is swallowed by its own jam guard
    // and the re-seal assertion reads air. A template never inherits the state a
    // sibling left (the flag-leak class); it pins what it depends on.
    pin_tgdis(&mut b, g);
    b.push(format!(
        "execute store success score #tg_sealed dw.sys if block {} {} {} {}",
        probe[0], probe[1], probe[2], g.gate_block
    ));
    b.push("assert score #tg_sealed dw.sys matches 1".to_string());
    b.push(format!("function {ns}:tgate_open_{}", g.safe));
    b.push(format!(
        "execute store success score #tg_open dw.sys if block {} {} {} minecraft:air",
        probe[0], probe[1], probe[2]
    ));
    b.push("assert score #tg_open dw.sys matches 1".to_string());
    b.push(format!("function {ns}:tgate_close_{}", g.safe));
    b.push(format!(
        "execute store success score #tg_shut dw.sys if block {} {} {} {}",
        probe[0], probe[1], probe[2], g.gate_block
    ));
    b.push("assert score #tg_shut dw.sys matches 1".to_string());
    out.insert(
        format!("packtest-datapack/data/{ns}/test/souls_timed_gate.mcfunction"),
        lines(&b).into_bytes(),
    );
    emit_timed_gate_crush_packtest(plan, g, out);
    emit_timed_gate_disarm_packtest(plan, g, out);
}

/// The disarm PackTest: a **disarmed** gate stays open across several former cycle
/// boundaries, and its closing edge never fires again.
///
/// A fake player cannot wait out a `schedule`, so the template does what the
/// timed-gate template already does — drives the REAL clock functions directly,
/// which IS the clock's body. The proof is that after `tgate_disarm_<id>` runs,
/// calling `tgate_close_<id>` (the exact function the schedule would have
/// re-entered) leaves the span air, three former boundaries in a row. If the
/// guard were missing, the very first one would re-seal it.
///
/// A `crush: true` gate gets the sharper form for free: the same guarded body
/// carries the judgement, so a close that cannot fill also cannot damage — which
/// is why the compiler unit test asserts the damage line is *inside* the guard
/// rather than beside it.
///
/// Emits nothing unless the gate declares a `disarm`, so every other campaign's
/// PackTest suite is byte-identical.
fn emit_timed_gate_disarm_packtest(plan: &Plan, g: &plan::TimedGatePlan, out: &mut BuildOutput) {
    if g.disarm.is_none() {
        return;
    }
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let (from, to) = g.gate_region;
    let probe = from;
    let id = &g.safe;
    let mut b = packtest_header(&format!(
        "{title}: timed gate `{}` stays open once disarmed",
        g.id
    ));
    b.push(format!("function {ns}:setup"));
    // A sibling template shares this world: re-arm and re-seal so the fixture
    // starts from a running, shut clock whatever ran before it.
    b.push(format!("scoreboard players set #tgdis_{id} dw.sys 0"));
    b.push(format!(
        "fill {} {} {} {} {} {} {}",
        from[0], from[1], from[2], to[0], to[1], to[2], g.gate_block
    ));
    // The clock is still live: a close really does seal.
    b.push(format!("function {ns}:tgate_open_{id}"));
    b.push(format!("function {ns}:tgate_close_{id}"));
    b.push(format!(
        "execute store success score #tgd_armed dw.sys if block {} {} {} {}",
        probe[0], probe[1], probe[2], g.gate_block
    ));
    b.push("assert score #tgd_armed dw.sys matches 1".to_string());
    // Pull the lever: the span clears and the sentinel latches.
    b.push(format!("function {ns}:tgate_disarm_{id}"));
    b.push(format!(
        "execute store success score #tgd_jam dw.sys if block {} {} {} minecraft:air",
        probe[0], probe[1], probe[2]
    ));
    b.push("assert score #tgd_jam dw.sys matches 1".to_string());
    b.push(format!("assert score #tgdis_{id} dw.sys matches 1"));
    // Three former cycle boundaries. The assertion lands immediately after the
    // CLOSE — before the open half runs — because that is the only place the
    // guard is load-bearing: an unguarded close re-seals here, and an assertion
    // taken after the following open would be satisfied either way and prove
    // nothing (measured: the template that asserted after the open passed
    // against a deliberately unguarded build).
    for n in 1..=3 {
        b.push(format!("function {ns}:tgate_close_{id}"));
        b.push(format!(
            "execute store success score #tgd_c{n} dw.sys if block {} {} {} minecraft:air",
            probe[0], probe[1], probe[2]
        ));
        b.push(format!("assert score #tgd_c{n} dw.sys matches 1"));
        // …and the open half of the dead ping-pong is a harmless no-op.
        b.push(format!("function {ns}:tgate_open_{id}"));
        b.push(format!(
            "execute store success score #tgd_o{n} dw.sys if block {} {} {} minecraft:air",
            probe[0], probe[1], probe[2]
        ));
        b.push(format!("assert score #tgd_o{n} dw.sys matches 1"));
    }
    out.insert(
        format!("packtest-datapack/data/{ns}/test/souls_timed_gate_disarm.mcfunction"),
        lines(&b).into_bytes(),
    );
}

/// spec-0016 §4 addendum PackTest: a `crush: true` gate's closing edge selects
/// **exactly** the players standing in its region.
///
/// ## Why this asserts scoping rather than death
///
/// The obvious test — put a dummy in the gate, shut it, assert a corpse — cannot
/// be written. **PackTest fake players are immune to `/damage`** (measured live
/// on the pinned toolserver, 2026-08-03: a `# @dummy` reports
/// `playerGameType: 0` (survival), yet `damage @s 1000 minecraft:generic` leaves
/// `Health` at exactly 20.0, and an explicit `gamemode survival @s` first does
/// not change that). A lethality assertion against a dummy is therefore
/// permanently red no matter how correct the engine is. This is the same
/// limitation that already pushed the `damage-players` PackTest onto a zombie
/// dummy — and a zombie cannot stand in here, because the crush selects `@a`.
///
/// So the runtime rung proves the half it genuinely can, and the other halves are
/// proven where they can be proven honestly:
///
/// * **scoping** (here, live, on real assembled geometry) — the emitted selector
///   contains the player when they stand in the gate and excludes them when they
///   step clear. The selector string is the *same* one `tgate_close_<id>` runs.
/// * **lethality + ordering** — compiler unit tests assert the exact
///   `execute as @a[…] run damage @s 1000 minecraft:generic` and that it precedes
///   the `fill`.
/// * **end-to-end death** — verified live against a real mineflayer client on
///   pinned 1.21.11: parked two blocks clear of the region a player survives 30 s
///   of repeated closing ticks at full health, and one closing tick with the same
///   player standing inside kills them.
///
/// The test binds `@s`, not `@a`, on purpose: PackTest runs the whole suite in
/// ONE shared world, so a sibling template's dummy standing in the same fixture
/// cell would otherwise be counted. Emits nothing unless the gate opts in, so a
/// non-crushing campaign's PackTest suite is byte-identical.
fn emit_timed_gate_crush_packtest(plan: &Plan, g: &plan::TimedGatePlan, out: &mut BuildOutput) {
    if !g.crush {
        return;
    }
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let (from, to) = g.gate_region;
    let selector = region_selector(from, to);
    // Feet-centred on one cell of the region: provably inside the selector box.
    let inside = crate::nav::cell_center(from);
    // Two blocks past the region's far x edge: provably outside it, and checked in
    // the same tick as the teleport so no fall or suffocation can confound it.
    let clear_x = from[0].max(to[0]) + 2;

    let mut b = packtest_header(&format!(
        "{title}: timed gate `{}` judges exactly the players in its region (spec-0016 §4)",
        g.id
    ));
    b.push(format!("function {ns}:setup"));
    // Un-jam first, for the same reason the base template does: a jam left by
    // `souls_timed_gate_disarm` swallows the `tgate_close_` this test crushes
    // with, and the crush that never happens reads as a lethality failure.
    pin_tgdis(&mut b, g);
    // Open first: a mistimed crossing leaves the player standing in an open
    // gateway, which is the position the judgement must catch.
    b.push(format!("function {ns}:tgate_open_{}", g.safe));
    b.push(format!(
        "tp @s {} {} {}",
        fmt_f64(inside[0]),
        fmt_f64(inside[1]),
        fmt_f64(inside[2])
    ));
    b.push(format!(
        "execute store success score #cr_in dw.sys if entity @s[{selector}]"
    ));
    b.push("assert score #cr_in dw.sys matches 1".to_string());
    b.push(format!(
        "tp @s {} {} {}",
        fmt_f64(f64::from(clear_x) + 0.5),
        fmt_f64(inside[1]),
        fmt_f64(inside[2])
    ));
    b.push(format!(
        "execute store success score #cr_out dw.sys if entity @s[{selector}]"
    ));
    b.push("assert score #cr_out dw.sys matches 0".to_string());
    out.insert(
        format!("packtest-datapack/data/{ns}/test/souls_timed_gate_crush.mcfunction"),
        lines(&b).into_bytes(),
    );
}

/// spec-0016 §2 shortcut PackTest: the unlock really clears the gate region, and
/// the open is **permanent** — re-running the tick after the sentinel is latched
/// cannot re-seal it, because nothing in the datapack ever fills a shortcut gate
/// (`DW0372` makes that structural at compile time; this asserts the runtime side
/// on a live server). Emits nothing for a campaign with no shortcut.
fn emit_shortcut_packtest(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let Some(sc) = plan.shortcuts.first() else {
        return;
    };
    let (from, to) = sc.gate_region;
    let probe = from; // one representative cell of the gate region
    let mut b = packtest_header(&format!(
        "{title}: shortcut `{}` opens its gate, permanently (spec-0016 §2)",
        sc.id
    ));
    b.push(format!("function {ns}:setup"));
    // Re-seal the gate and clear the sentinel: a sibling template (or `setup`
    // itself, on a shared batch server) may have left either in any state.
    b.push(format!("scoreboard players set #sc_{} dw.sys 0", sc.safe));
    b.push(format!(
        "fill {} {} {} {} {} {} {}",
        from[0], from[1], from[2], to[0], to[1], to[2], sc.gate_block
    ));
    b.push(format!(
        "execute store success score #sb_scut dw.sys if block {} {} {} {}",
        probe[0], probe[1], probe[2], sc.gate_block
    ));
    b.push("assert score #sb_scut dw.sys matches 1".to_string());
    // Pull the mechanism: the gate is air and the sentinel is latched.
    b.push(format!("function {ns}:shortcut_open_{}", sc.safe));
    b.push(format!(
        "execute store success score #sa_scut dw.sys if block {} {} {} minecraft:air",
        probe[0], probe[1], probe[2]
    ));
    b.push("assert score #sa_scut dw.sys matches 1".to_string());
    b.push(format!("assert score #sc_{} dw.sys matches 1", sc.safe));
    // Permanence: the latched sentinel suppresses any further unlock dispatch, and
    // no emitted function re-fills the region — so a second pass leaves it open.
    b.push(format!("function {ns}:shortcut_open_{}", sc.safe));
    b.push(format!(
        "execute store success score #sp_scut dw.sys if block {} {} {} minecraft:air",
        probe[0], probe[1], probe[2]
    ));
    b.push("assert score #sp_scut dw.sys matches 1".to_string());
    out.insert(
        format!("packtest-datapack/data/{ns}/test/souls_shortcut.mcfunction"),
        lines(&b).into_bytes(),
    );
}

/// spec-0016 §6 PackTests: four templates, one per live-verified claim of the TD
/// lane mechanism. Emits nothing for a campaign with neither a lane nor an
/// aggro-edge wave.
///
/// * `souls_td_patrol_nbt` — **the codec trap, as a test and not a comment.**
///   1.21.11's strict codec silently DROPS the legacy `PatrolTarget:{X,Y,Z}`
///   compound; only the snake_case `patrol_target:[I;x,y,z]` int-array survives.
///   The failure mode is a squad that patrols to vanilla-rolled random points —
///   working-but-drunk, invisible to every other proof. This asserts the array
///   reads back off the summoned squad, that exactly one mob is the
///   `PatrolLeader`, and that the whole squad spawns `Patrolling:1b`.
/// * `souls_td_lane_march` — the lane advances in **march order**: arriving at
///   the current waypoint steps the index by exactly one, and standing at a
///   LATER waypoint while the index still points at an earlier one does not skip
///   ahead (the lane is walked, not teleported through).
/// * `souls_td_lane_release` — routing hands over to native AI at aggro range:
///   with no player inside `aggro_radius` the whole squad is re-asserted onto
///   the lane; with a player inside it, every mob is `Patrolling:0b`.
/// * `souls_td_aggro_edge` — a `summon: aggro-edge` wave really materializes on
///   its perception ring around the defended anchor: full authored count, every
///   mob at its own `follow_range` from the defended point, measured from the
///   same snapped centre the compiler placed them around.
fn emit_td_lane_packtests(
    plan: &Plan,
    out: &mut BuildOutput,
    lane_routes: &crate::nav::LaneRoutes,
    wave_rings: &WaveRings,
) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let waves = &plan.campaign.quests.content.waves;
    let mut write = |name: &str, body: Vec<String>| {
        out.insert(
            format!("packtest-datapack/data/{ns}/test/{name}.mcfunction"),
            lines(&body).into_bytes(),
        );
    };

    if let Some((w, lane, wps)) = waves.iter().find_map(|w| {
        let lane = w.lane.as_ref()?;
        let wps = lane_routes.get(w.id.as_str())?;
        Some((w, lane, wps))
    }) {
        let safe = plan::safe_local(w.id.as_str());
        let tag = plan::wave_tag(w.id.as_str());
        let lead = lane_leader_tag(w.id.as_str());
        let idx = lane_index_holder(w.id.as_str());
        let total = plan::wave_total(w);
        let r = lane.aggro_radius;
        let t0 = wps[0];
        // A cell 200 blocks above the lane: no player in the batch is anywhere
        // near it, so `unless entity @a[distance=..R]` is decidable from the
        // compiler's chair — the re-assert half of the clock can be asserted
        // without knowing where a sibling template parked its dummy.
        let high = |c: [i32; 3]| {
            let p = ent_xyz(c);
            format!("{} 200.0 {}", p[0], p[2])
        };

        let mut b = packtest_header(&format!(
            "{title}: lane `{}` spawns as a patrol squad, snake_case `patrol_target` and all \
             (spec-0016 §6)",
            w.id
        ));
        b.push(format!("function {ns}:setup"));
        b.push(format!("kill @e[tag={tag}]"));
        b.push(format!("function {ns}:spawn_{safe}"));
        b.push(format!(
            "execute store result score #n_tdnbt dw.sys if entity @e[tag={tag}]"
        ));
        b.push(format!("assert score #n_tdnbt dw.sys matches {total}"));
        b.push(format!(
            "execute store result score #l_tdnbt dw.sys if entity \
             @e[tag={lead},nbt={{PatrolLeader:1b}}]"
        ));
        b.push("assert score #l_tdnbt dw.sys matches 1".to_string());
        b.push(format!(
            "execute store result score #p_tdnbt dw.sys if entity @e[tag={tag},nbt={{Patrolling:1b}}]"
        ));
        b.push(format!("assert score #p_tdnbt dw.sys matches {total}"));
        b.push(format!(
            "execute store result score #t_tdnbt dw.sys if entity \
             @e[tag={tag},nbt={{patrol_target:[I;{},{},{}]}}]",
            t0[0], t0[1], t0[2]
        ));
        b.push(format!("assert score #t_tdnbt dw.sys matches {total}"));
        b.push(format!("kill @e[tag={tag}]"));
        write("souls_td_patrol_nbt", b);

        let mut b = packtest_header(&format!(
            "{title}: lane `{}` advances in march order, one waypoint at a time (spec-0016 §6)",
            w.id
        ));
        b.push(format!("function {ns}:setup"));
        b.push(format!("kill @e[tag={tag}]"));
        b.push(format!("function {ns}:spawn_{safe}"));
        b.push(format!("scoreboard players set {idx} dw.sys 0"));
        if wps.len() > 1 {
            // Standing at a LATER waypoint does not skip the lane forward: the
            // index only ever advances off the waypoint it currently names.
            b.push(format!("tp @e[tag={tag}] {}", high(wps[1])));
            b.push(format!("function {ns}:lane_tick_{safe}"));
            b.push(format!("assert score {idx} dw.sys matches 0"));
        }
        b.push(format!("tp @e[tag={tag}] {}", ent_xyz(wps[0]).join(" ")));
        b.push(format!("function {ns}:lane_tick_{safe}"));
        b.push(format!("assert score {idx} dw.sys matches 1"));
        if wps.len() > 1 {
            // …and the squad is really re-pointed at the next waypoint (high
            // above the lane, so no player is inside the release radius).
            b.push(format!("tp @e[tag={tag}] {}", high(wps[1])));
            b.push(format!("function {ns}:lane_tick_{safe}"));
            b.push(format!(
                "execute store result score #m_tdmar dw.sys if entity \
                 @e[tag={tag},nbt={{patrol_target:[I;{},{},{}]}}]",
                wps[1][0], wps[1][1], wps[1][2]
            ));
            b.push(format!("assert score #m_tdmar dw.sys matches {total}"));
        }
        b.push(format!("kill @e[tag={tag}]"));
        write("souls_td_lane_march", b);

        let (pin, sel) = pin_dummy("dw_pt_tdrel");
        let mut b = packtest_header(&format!(
            "{title}: lane `{}` marches while distant and is released to native AI at aggro \
             range (spec-0016 §6)",
            w.id
        ));
        b.push(format!("function {ns}:setup"));
        b.push(pin);
        b.push(format!("kill @e[tag={tag}]"));
        b.push(format!("function {ns}:spawn_{safe}"));
        b.push(format!("scoreboard players set {idx} dw.sys 0"));
        b.push(format!("tp @e[tag={tag}] {}", high(wps[0])));
        b.push(format!("function {ns}:lane_tick_{safe}"));
        b.push(format!(
            "execute store result score #d_tdrel dw.sys if entity @e[tag={tag},nbt={{Patrolling:1b}}]"
        ));
        b.push(format!("assert score #d_tdrel dw.sys matches {total}"));
        // Bring the squad onto the pinned dummy rather than moving the player:
        // the test's own dummy stays exactly where the batch put it, so nothing
        // it leaves behind can perturb a sibling. `{r}` blocks is the release
        // radius; 0 is inside it by any measure.
        b.push(format!("execute at {sel} run tp @e[tag={tag}] ~ ~ ~"));
        b.push(format!("function {ns}:lane_tick_{safe}"));
        b.push(format!(
            "execute store result score #a_tdrel dw.sys if entity @e[tag={tag},nbt={{Patrolling:1b}}]"
        ));
        b.push("assert score #a_tdrel dw.sys matches 0".to_string());
        b.push(format!("kill @e[tag={tag}]"));
        b.push(format!("tag {sel} remove dw_pt_tdrel"));
        write("souls_td_lane_release", b);

        // --- the re-summon re-stations the squad ---
        //
        // A wave re-seat is `kill` + the wave's own `spawn_<wave>`, and the whole
        // stationed-re-seat ruling rests on that second half putting the squad
        // back on the lane exactly as the first summon did. This drives the same
        // two commands from the far side of the mechanism's worst state — the
        // squad hauled onto the party, released to native AI by the real clock,
        // its march clock at the END of the lane — and demands the fresh squad is
        // routed from waypoint 0 again with the release gone. Emitted for every
        // lane wave, bonfire or not, so a campaign that ships lanes proves it
        // without having to ship a rest point next to one.
        let (pin, sel) = pin_dummy("dw_pt_tdrst");
        let mut b = packtest_header(&format!(
            "{title}: re-summoning lane `{}` re-stations it — the feral release does not survive \
             (spec-0016 §1/§6)",
            w.id
        ));
        b.push(format!("function {ns}:setup"));
        b.push(pin);
        b.push(format!("kill @e[tag={tag}]"));
        b.push(format!("function {ns}:spawn_{safe}"));
        b.push(format!("execute at {sel} run tp @e[tag={tag}] ~ ~ ~"));
        b.push(format!("function {ns}:lane_tick_{safe}"));
        b.push(format!(
            "execute store result score #f_tdrst dw.sys if entity @e[tag={tag},nbt={{Patrolling:0b}}]"
        ));
        b.push(format!("assert score #f_tdrst dw.sys matches {total}"));
        b.push(format!(
            "scoreboard players set {idx} dw.sys {}",
            wps.len().saturating_sub(1)
        ));
        // The re-seat body, verbatim.
        b.push(format!("kill @e[tag={tag}]"));
        b.push(format!("function {ns}:spawn_{safe}"));
        b.push(format!(
            "execute store result score #n_tdrst dw.sys if entity @e[tag={tag},nbt={{Patrolling:1b}}]"
        ));
        b.push(format!("assert score #n_tdrst dw.sys matches {total}"));
        b.push(format!(
            "execute store result score #t_tdrst dw.sys if entity \
             @e[tag={tag},nbt={{patrol_target:[I;{},{},{}]}}]",
            t0[0], t0[1], t0[2]
        ));
        b.push(format!("assert score #t_tdrst dw.sys matches {total}"));
        b.push(format!("assert score {idx} dw.sys matches 0"));
        b.push(format!("kill @e[tag={tag}]"));
        b.push(format!("tag {sel} remove dw_pt_tdrst"));
        write("souls_td_lane_reseat", b);
        let _ = r;
    }

    if let Some((w, centre)) = waves.iter().find_map(|w| {
        (w.summon == Some(delvewright_dsl::WaveSummon::AggroEdge))
            .then(|| wave_rings.get(w.id.as_str()).map(|c| (w, *c)))
            .flatten()
    }) {
        let safe = plan::safe_local(w.id.as_str());
        let tag = plan::wave_tag(w.id.as_str());
        let total = plan::wave_total(w);
        let mut b = packtest_header(&format!(
            "{title}: aggro-edge wave `{}` materializes on its perception ring, never on the \
             defended point (spec-0016 §6)",
            w.id
        ));
        b.push(format!("function {ns}:setup"));
        b.push(format!("kill @e[tag={tag}]"));
        b.push(format!("function {ns}:spawn_{safe}"));
        b.push(format!(
            "execute store result score #n_tdedg dw.sys if entity @e[tag={tag}]"
        ));
        b.push(format!("assert score #n_tdedg dw.sys matches {total}"));
        // One assertion per (species, follow_range) group: each group's mobs must
        // ALL sit in its own ring band. The band is the compiler's annulus
        // tolerance plus 0.1 for the float compare — mobs and the ring centre are
        // both addressed at cell centres, so no rounding slack is needed.
        let mut groups: BTreeMap<(String, String), i64> = BTreeMap::new();
        for m in &w.mobs {
            let Some(radius) = m.attributes.and_then(|a| a.follow_range) else {
                continue;
            };
            *groups
                .entry((m.entity.clone(), fmt_f64(radius)))
                .or_default() += i64::from(m.count);
        }
        let c = ent_xyz(centre);
        for (i, ((entity, radius), count)) in groups.iter().enumerate() {
            let radius: f64 = radius.parse().unwrap_or_default();
            let lo = fmt_f64((radius - AGGRO_RING_TOLERANCE - 0.1).max(0.0));
            let hi = fmt_f64(radius + 0.1);
            b.push(format!(
                "execute positioned {} {} {} store result score #r{i}_tdedg dw.sys if entity \
                 @e[tag={tag},type={entity},distance={lo}..{hi}]",
                c[0], c[1], c[2]
            ));
            b.push(format!("assert score #r{i}_tdedg dw.sys matches {count}"));
        }
        b.push(format!("kill @e[tag={tag}]"));
        write("souls_td_aggro_edge", b);
    }
}

/// v0.6 PackTests (spec-0012 checkpoints, spec-0014 stealth). Fake players cannot
/// respawn synchronously within a plain mcfunction test, so these drive the
/// compiler-generated mechanics directly and assert their deterministic effects:
///
/// * **checkpoint**: applying the checkpoint's `spawnpoint @a` + `dw:cp pos`
///   mirror makes `storage dw:cp pos` read back the checkpoint cell — the
///   machine-checkable "last checkpoint" contract other features consume.
/// * **stealth** (zone-presence model, no sneak
///   requirement): the generated `stealth_eval_<i>` judge catches an exposed
///   (out-of-zone) player after `grace_ticks` and spares an in-zone one —
///   driven by teleporting the dummy in and out of the declared zone box.
fn emit_v06_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);

    if let Some(cp) = plan.checkpoints.first() {
        let [x, y, z] = cp.pos;
        let (pin, sel) = pin_dummy("dw_t_cpr");
        let mut t = packtest_header(&format!(
            "{title}: checkpoint mirrors its cell into dw:cp (spec-0012)"
        ));
        t.push(format!("function {ns}:setup"));
        // Pin this test's own dummy (see `pin_dummy`): the spawnpoint write is
        // per-player, so it goes to this test's dummy, not every dummy in the
        // batch. The `dw:cp` mirror write + read-back stay within this single
        // (atomic) function, so the shared storage cannot be interleaved.
        t.push(pin);
        // Apply the exact commands a `set-checkpoint` emits, then read the mirror
        // back per-axis (rock-solid vs. an NBT compound match).
        t.push(format!("spawnpoint {sel} {x} {y} {z}"));
        t.push(format!(
            "data modify storage dw:cp pos set value [{x}, {y}, {z}]"
        ));
        t.push(
            "execute store result score #cx_cpr dw.sys run data get storage dw:cp pos[0]"
                .to_string(),
        );
        t.push(
            "execute store result score #cy_cpr dw.sys run data get storage dw:cp pos[1]"
                .to_string(),
        );
        t.push(
            "execute store result score #cz_cpr dw.sys run data get storage dw:cp pos[2]"
                .to_string(),
        );
        t.push(format!("assert score #cx_cpr dw.sys matches {x}"));
        t.push(format!("assert score #cy_cpr dw.sys matches {y}"));
        t.push(format!("assert score #cz_cpr dw.sys matches {z}"));
        out.insert(
            format!("packtest-datapack/data/{ns}/test/v06_checkpoint_respawn.mcfunction"),
            lines(&t).into_bytes(),
        );

        // --- the environmental-death variant ---
        //
        // The template above proves the RECORD; this one proves the LANDING, which
        // is the half the owner's tide-mill playtest found missing. `spawnpoint` is
        // only a hint: vanilla re-validates the recorded cell on death and silently
        // respawns at the world spawn when it is solid or liquid. Nothing about
        // that is specific to how the player died — a crush gate's
        // `damage @s 1000 minecraft:generic` leaves exactly the same `deathCount`
        // edge a mob kill does — so the test drives that edge directly, from the
        // worst starting position (the campaign entrance, where vanilla's fallback
        // drops them), and asserts the player ends on the checkpoint cell.
        //
        // Second half: the re-seat must be EDGE-triggered. A leash that re-seated
        // every tick would pin the party to the checkpoint and make the delve
        // unplayable, so the test walks the dummy away again, re-runs the check
        // with no new death, and asserts it stayed away.
        if let Some(entry) = campaign_spawn(plan) {
            let (pin, sel) = pin_dummy("dw_t_cpseat");
            let mut t = packtest_header(&format!(
                "{title}: an environmental death re-seats the player ON the checkpoint, once \
                 (spec-0012)"
            ));
            t.push(format!("function {ns}:setup"));
            t.push(pin);
            t.push(format!("scoreboard players set #cp dw.sys {}", cp.index));
            t.push(format!("scoreboard players set {sel} dw.death_ack 0"));
            t.push(format!("scoreboard players set {sel} dw.deaths 1"));
            t.push(format!(
                "tp {sel} {} {} {}",
                center(entry[0]),
                entry[1],
                center(entry[2])
            ));
            t.push(format!(
                "execute as {sel} run function {ns}:cp_respawn_check"
            ));
            for (i, axis) in ["x", "y", "z"].iter().enumerate() {
                t.push(format!(
                    "execute store result score #{axis}_cpseat dw.sys run data get entity {sel} \
                     Pos[{i}] 100"
                ));
            }
            t.push(format!(
                "assert score #x_cpseat dw.sys matches {}",
                cp.pos[0] * 100 + 50
            ));
            t.push(format!(
                "assert score #y_cpseat dw.sys matches {}",
                cp.pos[1] * 100
            ));
            t.push(format!(
                "assert score #z_cpseat dw.sys matches {}",
                cp.pos[2] * 100 + 50
            ));
            t.push(format!("assert score {sel} dw.death_ack matches 1"));
            // …and no second re-seat without a second death.
            t.push(format!(
                "tp {sel} {} {} {}",
                center(entry[0]),
                entry[1],
                center(entry[2])
            ));
            t.push(format!(
                "execute as {sel} run function {ns}:cp_respawn_check"
            ));
            t.push(format!(
                "execute store result score #x2_cpseat dw.sys run data get entity {sel} Pos[0] 100"
            ));
            t.push(format!(
                "assert score #x2_cpseat dw.sys matches {}",
                entry[0] * 100 + 50
            ));
            out.insert(
                format!("packtest-datapack/data/{ns}/test/v06_checkpoint_reseat.mcfunction"),
                lines(&t).into_bytes(),
            );
        }
    }

    if let Some(beat) = plan.stealth_beats.first() {
        let i = beat.index;
        let grace = beat.grace_ticks;
        let (_, zpos, zext) = &beat.zones[0];
        let inside = *zpos;
        let outside = [zpos[0] + zext[0] as i32 + 10, zpos[1], zpos[2]];
        let (pin, sel) = pin_dummy("dw_sttest");
        let mut t = packtest_header(&format!(
            "{title}: stealth catches the exposed, spares the hidden (spec-0014)"
        ));
        t.push(format!("function {ns}:setup"));
        // Pin this test's own dummy (see `pin_dummy`): the template teleports
        // it to absolute campaign coordinates, after which `@p` would resolve
        // to a neighbor test's dummy and the controlled state below would land
        // on — and be asserted against — the wrong player.
        t.push(pin);
        // --- spare: an in-zone player (zone presence alone = hidden) never
        //     accrues grace; an accrued grace is reset the moment they are safe. ---
        t.push(format!("function {ns}:stealth_begin_{i}"));
        // Disarm the live session marker `stealth_begin` just set: this test drives
        // `stealth_eval` explicitly, so the world `tick` loop (which runs
        // `stealth_eval` on every player while `#stealth` is armed) must NOT also
        // fire — a second judge pass in the same tick would double-count the
        // exposure (an extra grace increment per tick), corrupting the controlled
        // counts the asserts read. Runtime gameplay is unaffected (there the tick
        // loop is the sole caller); this only isolates the test.
        t.push("scoreboard players set #stealth dw.sys 0".to_string());
        t.push(format!("scoreboard players set {sel} dw.st_grace 5"));
        t.push(format!(
            "tp {sel} {} {} {}",
            inside[0], inside[1], inside[2]
        ));
        t.push(format!(
            "execute as {sel} run function {ns}:stealth_eval_{i}"
        ));
        t.push(format!("assert score {sel} dw.st_grace matches 0"));
        // --- caught: an exposed (out of every zone) player accrues grace and is
        //     caught on the grace_ticks-th judge tick (on_caught resets grace to
        //     0). This section runs LAST: the trip executes the campaign's real
        //     `on_caught`, whose effects are arbitrary content (the island's
        //     deals lethal damage) — nothing state-dependent may follow it, and
        //     the closing assert reads the dummy through the tag, which keeps
        //     matching even if `on_caught` killed it. ---
        t.push(format!("function {ns}:stealth_begin_{i}"));
        // Disarm again (this second `begin` re-armed `#stealth`); see note above.
        t.push("scoreboard players set #stealth dw.sys 0".to_string());
        t.push(format!(
            "tp {sel} {} {} {}",
            outside[0], outside[1], outside[2]
        ));
        // grace_ticks-1 judge ticks: grace climbs but has not yet tripped.
        for _ in 0..grace.saturating_sub(1) {
            t.push(format!(
                "execute as {sel} run function {ns}:stealth_eval_{i}"
            ));
        }
        t.push(format!(
            "assert score {sel} dw.st_grace matches {}",
            grace.saturating_sub(1)
        ));
        // One more tick trips on_caught, which resets grace to 0.
        t.push(format!(
            "execute as {sel} run function {ns}:stealth_eval_{i}"
        ));
        t.push(format!("assert score {sel} dw.st_grace matches 0"));
        out.insert(
            format!("packtest-datapack/data/{ns}/test/v06_stealth.mcfunction"),
            lines(&t).into_bytes(),
        );

        // --- cutscene freeze (the staging invariant, see CUTSCENE_TAG): a player
        //     in the cutscene state is exposed — outside every zone — and must
        //     still NOT accrue grace while the marker is on, then must resume
        //     accruing the moment it comes off. Driven through the real
        //     `stealth_tick` gate (not `stealth_eval`), because the gate is what
        //     the freeze lives in.
        let (fpin, fsel) = pin_dummy("dw_t_cfrz");
        let mut f = packtest_header(&format!(
            "{title}: a cutscene freezes the stealth clock, and it resumes after"
        ));
        f.push(format!("function {ns}:setup"));
        // Pin this test's own dummy (see `pin_dummy`): the template tp's it to
        // absolute campaign coordinates, after which a bare `@p` would resolve
        // to a neighbor test's dummy — and an `@a` write (state, tp, or the
        // cutscene tag itself) would land on every dummy in the batch.
        f.push(fpin);
        f.push(format!("function {ns}:stealth_begin_{i}"));
        // Disarm the live session marker so the world `tick` loop does not judge
        // in the same tick; this test drives `stealth_tick` explicitly.
        f.push("scoreboard players set #stealth dw.sys 0".to_string());
        f.push(format!("scoreboard players set {fsel} dw.st_grace 0"));
        f.push(format!(
            "tp {fsel} {} {} {}",
            outside[0], outside[1], outside[2]
        ));
        f.push(format!("tag {fsel} add {CUTSCENE_TAG}"));
        // Well past `grace_ticks` of exposure: frozen, so grace stays 0.
        for _ in 0..grace + 2 {
            f.push(format!("function {ns}:stealth_tick_{i}"));
        }
        f.push(format!("assert score {fsel} dw.st_grace matches 0"));
        // Restore drops the marker; the clock resumes from where it paused.
        f.push(format!("tag {fsel} remove {CUTSCENE_TAG}"));
        for _ in 0..grace.saturating_sub(1) {
            f.push(format!("function {ns}:stealth_tick_{i}"));
        }
        f.push(format!(
            "assert score {fsel} dw.st_grace matches {}",
            grace.saturating_sub(1)
        ));
        out.insert(
            format!("packtest-datapack/data/{ns}/test/v06_cutscene_freeze.mcfunction"),
            lines(&f).into_bytes(),
        );
    }

    // damage-players: the `/damage` primitive the effect emits actually subtracts
    // health. A 0-player void does not tick a real player, so the test drives the
    // damage on a summoned dummy (NoAI/Silent zombie, full 20 HP) with the exact
    // amount + type the first declared `damage-players` uses, then asserts its
    // Health dropped by that amount. Emitted only when the campaign uses the verb.
    if let Some((amount, kind)) = first_damage_players(plan.campaign) {
        let type_id = kind.id();
        let mut t = packtest_header(&format!(
            "{title}: damage-players subtracts {amount} half-hearts ({type_id}) (spec-0014)"
        ));
        t.push(format!("function {ns}:setup"));
        // A dummy at a fixed cell near origin: NoAI so it never moves, Silent, full
        // health. `damage` applies synchronously, so a 0-player void still shows it.
        // Pre-clear the tag first — never assume a fresh world on the shared-batch
        // server — and kill again on the way out.
        t.push("kill @e[tag=dw_dmgtest]".to_string());
        t.push(
            "summon minecraft:zombie 0 -60 0 {Tags:[\"dw_dmgtest\"],NoAI:1b,Silent:1b,\
             PersistenceRequired:1b,Health:20f}"
                .to_string(),
        );
        t.push(
            "execute store result score #hp0_dmg dw.sys run data get entity \
             @e[tag=dw_dmgtest,limit=1] Health 100"
                .to_string(),
        );
        t.push(format!(
            "damage @e[tag=dw_dmgtest,limit=1] {amount} {type_id}"
        ));
        t.push(
            "execute store result score #hp1_dmg dw.sys run data get entity \
             @e[tag=dw_dmgtest,limit=1] Health 100"
                .to_string(),
        );
        // The dummy's Health (×100) must have dropped: drop = hp0 - hp1 ≥ 1. Asserting
        // "strictly decreased" rather than an exact amount keeps the test robust across
        // damage types (armor-respecting types reduce the number, but the hit still
        // lands); the exact `damage @s <amount> <type>` string is asserted by a
        // compiler unit test.
        t.push("scoreboard players operation #drop_dmg dw.sys = #hp0_dmg dw.sys".to_string());
        t.push("scoreboard players operation #drop_dmg dw.sys -= #hp1_dmg dw.sys".to_string());
        t.push("assert score #drop_dmg dw.sys matches 1..".to_string());
        t.push("kill @e[tag=dw_dmgtest]".to_string());
        out.insert(
            format!("packtest-datapack/data/{ns}/test/v06_damage.mcfunction"),
            lines(&t).into_bytes(),
        );
    }

    // spec-0031 lethal volumes: the runtime half, one template per volume.
    emit_lethal_packtests(plan, out);
    emit_economy_packtests(plan, out);

    // spec-0031 teleport: the runtime half of TOTALITY, one template per teleport.
    emit_teleport_packtests(plan, out);
    // …and the runtime half of the fixture class (`DW0545`), one template per
    // (teleport × stake) pair — the one defect in this family that has no
    // compile-time form at all.
    emit_fixture_packtests(plan, out);
}

/// spec-0032 / `DW0545` PackTests: one template per (`teleport`, `stake`) pair,
/// each of which **leaves a real recovery-stake marker in a real teleport's
/// volume, rides, and asserts the marker stayed while a body left.**
///
/// **This is the only tier that can witness the motivating defect at all, and it
/// is worth being explicit about why.** A stake marker's position is chosen at
/// RUNTIME — the death point, or a row of the compile-time placement table picked
/// by the seat in force — so no compile-time geometry test knows where it will
/// be. That is exactly why `DW0526` (footing) and `DW0542` (an affordance bound
/// to a compile-time cell) both correctly decline it, and why the compile-time
/// half of this fix is a *class* rather than a *box test*. The compile-time proof
/// says the compiler wrote the exclusion and the marker declares the class; only
/// a live server can say vanilla's `tag=!…` really keeps that entity out of a
/// `tp`'s reach.
///
/// Three assertions, and the middle one is what stops the template being
/// vacuous:
///
/// 1. both halves of the marker — the `minecraft:interaction` the collector
///    right-clicks and the `item_display` the player sees — really are inside the
///    teleport's own selector box before anything moves (a template whose
///    fixtures landed outside would pass by examining nothing);
/// 2. a plain **body** summoned in the same box **left**, so a teleport that did
///    nothing at all cannot pass — the one-directional-falsifiability trap, where
///    a gate can only fail in the direction that never happens;
/// 3. both halves of the marker are still there.
///
/// It drives the campaign's REAL `stk_fill_<s>` and REAL `teleport_<key>`, never
/// commands it re-typed, so an emission that grows a filter or drops the class
/// reds here.
fn emit_fixture_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let sts = stakes(plan);
    if sts.is_empty() {
        return;
    }
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for eff in all_campaign_effects(plan.campaign) {
        let Some((from, _to)) = eff.teleport() else {
            continue;
        };
        let name = teleport_fn(eff);
        if !seen.insert(name.clone()) {
            continue;
        }
        let Some((lo, hi)) = plan.zone_box(from) else {
            continue;
        };
        let key = &name["teleport_".len()..];
        let bx = box_selector_args(lo, hi);
        let mid = [
            (lo[0] + hi[0]) / 2,
            (lo[1] + hi[1]) / 2,
            (lo[2] + hi[2]) / 2,
        ];
        let at = format!("{} {} {}", mid[0] as f64 + 0.5, mid[1], mid[2] as f64 + 0.5);
        for (st, safe) in &sts {
            if st.max_live() == 0 {
                continue;
            }
            let tag = stk_tag(safe);
            let hw = crate::affordance::hardware_tag(&tag);
            let body = format!("dw_fixbody_{key}_{safe}");
            let (pin, me) = pin_dummy(&format!("dw_fixtest_{key}_{safe}"));
            let sc = format!("{key}_{safe}");
            let mut t = packtest_header(&format!(
                "{title}: `{name}` carries a body out of its volume and leaves the recovery \
                 stake `{}` standing — a marker is a PLACE, and moving it would move the \
                 position its ledger recorded (DW0545)",
                st.id
            ));
            t.push(format!("function {ns}:setup"));
            t.push(pin);
            // Own entity and ledger state: a sibling template's leftovers would
            // defeat the guarded summon inside `stk_fill_<s>`.
            t.push(format!("kill @e[tag={tag}]"));
            t.push(format!("kill @e[tag={hw}]"));
            t.push(format!("kill @e[tag={body}]"));
            for k in 0..st.max_live() {
                t.push(format!(
                    "scoreboard players set {me} {} 0",
                    stk_live_obj(safe, k)
                ));
            }
            // A real marker, put down by the real drop path, in the car.
            t.push(format!(
                "execute as {me} positioned {at} run function {ns}:stk_fill_{safe}"
            ));
            // Bound, not assumed: both halves are inside the volume the `tp` sweeps.
            t.push(format!(
                "execute store result score #fx_in_{sc} dw.sys if entity @e[tag={tag},{bx}]"
            ));
            t.push(format!("assert score #fx_in_{sc} dw.sys matches 1"));
            t.push(format!(
                "execute store result score #fx_hw_{sc} dw.sys if entity @e[tag={hw},{bx}]"
            ));
            t.push(format!("assert score #fx_hw_{sc} dw.sys matches 1"));
            // A passenger, so "nothing moved" cannot read as a pass.
            t.push(format!(
                "summon minecraft:zombie {at} {{Tags:[\"{body}\"],NoAI:1b,Silent:1b,\
                 PersistenceRequired:1b}}"
            ));
            t.push(format!("function {ns}:{name}"));
            t.push(format!(
                "execute store result score #fx_body_{sc} dw.sys if entity @e[tag={body},{bx}]"
            ));
            t.push(format!("assert score #fx_body_{sc} dw.sys matches 0"));
            // …and the place stayed a place.
            t.push(format!(
                "execute store result score #fx_stay_{sc} dw.sys if entity @e[tag={tag},{bx}]"
            ));
            t.push(format!("assert score #fx_stay_{sc} dw.sys matches 1"));
            t.push(format!(
                "execute store result score #fx_hwstay_{sc} dw.sys if entity @e[tag={hw},{bx}]"
            ));
            t.push(format!("assert score #fx_hwstay_{sc} dw.sys matches 1"));
            t.push(format!("kill @e[tag={body}]"));
            t.push(format!("kill @e[tag={tag}]"));
            t.push(format!("kill @e[tag={hw}]"));
            for k in 0..st.max_live() {
                t.push(format!(
                    "scoreboard players set {me} {} 0",
                    stk_live_obj(safe, k)
                ));
            }
            out.insert(
                format!("packtest-datapack/data/{ns}/test/fixture_{sc}.mcfunction"),
                lines(&t).into_bytes(),
            );
        }
    }
}

/// The entity types a `teleport` template puts in the volume — deliberately
/// **the engine's own machinery beside a content body**.
///
/// This list is the acceptance criterion made runtime-visible. `lethal_volumes[]`
/// must exempt `interaction`, `marker` and the three display types by name
/// ([`LETHAL_EXEMPT_TYPES`]) or it would erase a cutscene camera; a `teleport`
/// exempts nothing, and the four machinery types here are exactly the ones an
/// exemption list would have dropped. If a future selector grows a `type=!…`
/// term, the entity of that type stays behind and this template reds — which is
/// the point: an NPC is a body plus a co-located `minecraft:interaction`, and a
/// verb that moves one without the other loses the delve its speaker in silence.
///
/// `(type, extra NBT)`. Every one is `Silent`/`NoAI`/persistent where the type
/// supports it, so a template can never leave a wandering body behind on the
/// shared batch server.
const TELEPORT_WITNESS_TYPES: [(&str, &str); 5] = [
    // a content body — the cargo-lift ruling: everyone on the car travels
    (
        "minecraft:zombie",
        "NoAI:1b,Silent:1b,PersistenceRequired:1b",
    ),
    // the four an exemption list would have dropped
    ("minecraft:interaction", "width:1.0f,height:2.0f"),
    ("minecraft:marker", ""),
    ("minecraft:text_display", ""),
    ("minecraft:item", r#"Item:{id:"minecraft:stone",count:1}"#),
];

/// spec-0031 PackTests: one template per resolved `teleport`, each of which
/// **puts one entity of every witness type in the volume and asserts every one of
/// them arrived**.
///
/// The compile-time test (`crates/compiler/tests/v10_teleport.rs`) proves the
/// compiler wrote no filter beyond the one class exclusion (`tag=!dw_fixture`,
/// [`crate::affordance`]). That is only half of "the selection is total over
/// bodies": the other half is vanilla's own `@e[<box>]` semantics, which no Rust
/// test can witness. This template is that half, and it calls the campaign's
/// REAL generated `teleport_<key>` function — not a command it re-typed — so a
/// selector that grows a second filter reds here. Its witnesses carry no class
/// tag, which is why "the box is then empty" is still the criterion: the
/// exclusion is about the engine's own places, and this template puts none down.
///
/// The assertion is a count, not a per-entity check: the witnesses go in tagged,
/// the box is asserted to hold all of them first (a template whose entities
/// landed outside would pass by examining nothing), the function runs, and the
/// box must then hold **zero**. Counting rather than naming keeps the claim
/// exactly "every entity in the volume left it", which is the criterion's
/// wording.
fn emit_teleport_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for eff in all_campaign_effects(plan.campaign) {
        let Some((from, _to)) = eff.teleport() else {
            continue;
        };
        let name = teleport_fn(eff);
        if !seen.insert(name.clone()) {
            continue;
        }
        let Some((lo, hi)) = plan.zone_box(from) else {
            continue;
        };
        let mid = [
            (lo[0] + hi[0]) / 2,
            (lo[1] + hi[1]) / 2,
            (lo[2] + hi[2]) / 2,
        ];
        let bx = box_selector_args(lo, hi);
        let key = &name["teleport_".len()..];
        let tag = format!("dw_tptest_{key}");
        let n = TELEPORT_WITNESS_TYPES.len();
        let mut t = packtest_header(&format!(
            "{title}: `{name}` moves EVERYTHING in its volume — no type is exempt (spec-0031)"
        ));
        t.push(format!("function {ns}:setup"));
        // Never assume a fresh world on the shared-batch server.
        t.push(format!("kill @e[tag={tag}]"));
        for (ty, nbt) in TELEPORT_WITNESS_TYPES {
            let sep = if nbt.is_empty() { "" } else { "," };
            t.push(format!(
                "summon {ty} {} {} {} {{Tags:[\"{tag}\"]{sep}{nbt}}}",
                mid[0] as f64 + 0.5,
                mid[1],
                mid[2] as f64 + 0.5
            ));
        }
        // Bound, not assumed: all N witnesses really are inside the volume's own
        // selector box before anything moves.
        t.push(format!(
            "execute store result score #tp_in_{key} dw.sys if entity @e[tag={tag},{bx}]"
        ));
        t.push(format!("assert score #tp_in_{key} dw.sys matches {n}"));
        t.push(format!("function {ns}:{name}"));
        // …and none of them is left behind. A `type=!…` term in the selector
        // leaves its entity here and this count is non-zero.
        t.push(format!(
            "execute store result score #tp_left_{key} dw.sys if entity @e[tag={tag},{bx}]"
        ));
        t.push(format!("assert score #tp_left_{key} dw.sys matches 0"));
        t.push(format!("kill @e[tag={tag}]"));
        out.insert(
            format!("packtest-datapack/data/{ns}/test/{name}.mcfunction"),
            lines(&t).into_bytes(),
        );
    }
}

/// spec-0031 PackTests: one template per resolved lethal volume, each of which
/// **puts an entity in the volume and asserts the volume kills it**.
///
/// A compile-time proof that a box is impassable proves nothing about a box that
/// kills; the two halves are independent, and a green over the first alone is the
/// vacuous pass CLAUDE.md names. So the runtime half is bound per volume, not once
/// per campaign: `validation/lethal-gate.json` reports the template count beside
/// the volume count, and a campaign with N volumes and fewer than N templates is
/// legible as such without re-deriving it.
///
/// The body drives the volume's real generated function on a summoned NoAI dummy
/// and asserts its `Health` reached zero. Zero-health rather than
/// "no entity matches": a mob killed by `/damage` plays its death animation for
/// ~20 ticks before vanilla removes it, so `unless entity` would be asserting the
/// scheduler rather than the kill. `/damage` is synchronous, so the whole claim
/// lands in one tick.
///
/// The dummy is a mob, which is exactly the line under test — the volume's
/// non-player selector, minus [`LETHAL_EXEMPT_TYPES`]. The player half runs
/// through the same `/damage` on a per-player re-bind and is asserted by the
/// compiler unit tests, which read the emitted command text directly (PackTest's
/// framework dummies are not a substitute for a real player here).
/// PackTest templates for the economy (spec-0032) — **exactly the two halves this
/// tier can genuinely witness, and no template for the half it cannot.**
///
/// What a fake player is, measured twice independently (2026-08-03 and 2026-08-09):
/// permanently undamageable, and unable to die. So this tier **cannot witness a
/// player death**, and therefore cannot prove the edge from a death to a stake
/// being placed. spec-0032's acceptance criterion 9 asks for the full loop on the
/// **bot** tier for exactly that reason, and no template is generated here that
/// would appear to cover it: a template that bound to nothing and reported green
/// is the vacuity CLAUDE.md names, and it is worse than an absence because review
/// cannot see it.
///
/// What this tier CAN witness, and does:
///
/// 1. **A purchase debits, and an unaffordable one is refused and says so.** The
///    offer handler is an ordinary function; driving it as the dummy proves the
///    gate arithmetic and the refusal path without needing a click.
/// 2. **A stake drop → collect round-trip returns the exact amount.** Both ends
///    are functions, and the amount travels through the per-player ledger, so the
///    only thing skipped is the death that would normally call `stk_drop_<s>` —
///    which is precisely the part this tier cannot have and the bot tier must.
///
/// Every template pins its own dummy on the first post-`setup` line and addresses
/// it exclusively by tag, and suffixes its `dw.sys` scratch holders, per the rules
/// on [`pin_dummy`].
fn emit_economy_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);

    // --- 1. the shop ------------------------------------------------------
    if let Some((i, sh, _)) = shops(plan).into_iter().next()
        && let Some((j, off)) = sh
            .offers
            .iter()
            .enumerate()
            .find(|(_, o)| o.effects.iter().any(|e| e.writes_state().is_some()))
        && let Some((state, _)) = off
            .effects
            .iter()
            .find_map(|e| e.writes_state())
            .map(|(s, w)| (s.clone(), w))
    {
        let obj = plan::state_score(state.as_str());
        let (pin, me) = pin_dummy("dw_shoptest");
        let mut t = packtest_header(&format!(
            "{title}: a purchase debits the purse, and one that cannot be afforded is refused \
             (spec-0032)"
        ));
        t.push(format!("function {ns}:setup"));
        t.push(pin);
        // Afford it: the balance the offer's own gate and its effects' gates both
        // read is set high enough for every comparison in the bundle to open.
        t.push(format!("scoreboard players set {me} {obj} 100"));
        t.push(format!(
            "execute as {me} run function {ns}:shop_pick_{i}_{j}"
        ));
        t.push(format!(
            "execute store result score #sh_paid dw.sys run scoreboard players get {me} {obj}"
        ));
        t.push("assert score #sh_paid dw.sys matches ..99".to_string());
        // …and cannot: with nothing in the purse the same press must move nothing.
        t.push(format!("scoreboard players set {me} {obj} 0"));
        t.push(format!(
            "execute as {me} run function {ns}:shop_pick_{i}_{j}"
        ));
        t.push(format!(
            "execute store result score #sh_broke dw.sys run scoreboard players get {me} {obj}"
        ));
        t.push("assert score #sh_broke dw.sys matches 0".to_string());
        out.insert(
            format!("packtest-datapack/data/{ns}/test/v10_shop_purchase.mcfunction"),
            lines(&t).into_bytes(),
        );
    }

    // --- 2. the stake's drop → collect round trip -------------------------
    for (st, safe) in stakes(plan) {
        if st.max_live() == 0 {
            continue;
        }
        let obj = plan::state_score(st.state.as_str());
        let tag = stk_tag(&safe);
        let (pin, me) = pin_dummy(&format!("dw_stktest_{safe}"));
        let mut t = packtest_header(&format!(
            "{title}: a stake takes the declared share and gives back exactly what it took \
             (spec-0032). NOT a death test — a PackTest fake player cannot die; the death edge \
             is the bot tier's (AC9)."
        ));
        t.push(format!("function {ns}:setup"));
        t.push(pin);
        // Own entity state: a sibling template's leftover marker would defeat the
        // guarded summon inside `stk_fill_<s>` and make the collect assert on air.
        t.push(format!("kill @e[tag={tag}]"));
        t.push(format!(
            "kill @e[tag={}]",
            crate::affordance::hardware_tag(&tag)
        ));
        for k in 0..st.max_live() {
            t.push(format!(
                "scoreboard players set {me} {} 0",
                stk_live_obj(&safe, k)
            ));
            t.push(format!(
                "scoreboard players set {me} {} 0",
                stk_amount_obj(&safe, k)
            ));
        }
        t.push(format!("scoreboard players set {me} {obj} 40"));
        t.push(format!("execute as {me} run function {ns}:stk_drop_{safe}"));
        // The forfeit really left the purse…
        t.push(format!(
            "execute store result score #stk_lost_{safe} dw.sys run scoreboard players get {me} {obj}"
        ));
        let after_drop: i32 = match st.forfeit() {
            delvewright_dsl::Forfeit::All => 0,
            delvewright_dsl::Forfeit::None => 40,
            delvewright_dsl::Forfeit::Fixed { amount } => 40 - amount.clamp(0, 40),
            delvewright_dsl::Forfeit::Proportion { percent } => {
                40 - (40 * percent.min(100) as i32) / 100
            }
        };
        t.push(format!(
            "assert score #stk_lost_{safe} dw.sys matches {after_drop}"
        ));
        // …a marker really stands where the drop put it…
        // Existence via a counted score rather than `assert entity`: `assert score`
        // is the one assertion form every generated template in this repo already
        // uses on the pinned toolserver, so this claim is made in a spelling that
        // is known to run rather than in one that merely reads well.
        t.push(format!(
            "execute store result score #stk_mark_{safe} dw.sys if entity @e[tag={tag}]"
        ));
        t.push(format!("assert score #stk_mark_{safe} dw.sys matches 1.."));
        // …and collecting gives back exactly what was taken, no more.
        t.push(format!(
            "execute as {me} run function {ns}:stk_collect_{safe}"
        ));
        t.push(format!(
            "execute store result score #stk_back_{safe} dw.sys run scoreboard players get {me} {obj}"
        ));
        t.push(format!("assert score #stk_back_{safe} dw.sys matches 40"));
        // A second press in the same breath is a no-op — the slot went dead as part
        // of being taken, so idempotence is structural rather than timed (AC6).
        t.push(format!(
            "execute as {me} run function {ns}:stk_collect_{safe}"
        ));
        t.push(format!(
            "execute store result score #stk_twice_{safe} dw.sys run scoreboard players get {me} {obj}"
        ));
        t.push(format!("assert score #stk_twice_{safe} dw.sys matches 40"));
        out.insert(
            format!("packtest-datapack/data/{ns}/test/v10_stake_{safe}.mcfunction"),
            lines(&t).into_bytes(),
        );
    }
}

fn emit_lethal_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    for v in &plan.lethal_volumes {
        let (lo, hi) = v.region;
        let mid = [
            (lo[0] + hi[0]) / 2,
            (lo[1] + hi[1]) / 2,
            (lo[2] + hi[2]) / 2,
        ];
        let tag = format!("dw_lethtest_{}", v.safe);
        let sel = format!("@e[tag={tag},limit=1]");
        let mut t = packtest_header(&format!(
            "{title}: lethal volume `{}` kills what enters it (spec-0031)",
            v.id
        ));
        t.push(format!("function {ns}:setup"));
        // Never assume a fresh world on the shared-batch server.
        t.push(format!("kill @e[tag={tag}]"));
        t.push(format!(
            "summon minecraft:zombie {} {} {} \
             {{Tags:[\"{tag}\"],NoAI:1b,Silent:1b,PersistenceRequired:1b,Health:20f}}",
            mid[0] as f64 + 0.5,
            mid[1],
            mid[2] as f64 + 0.5
        ));
        // Bound, not assumed: the dummy really is in the volume's own selector
        // box. A template whose dummy landed outside would pass this test by
        // examining nothing, which is the failure mode the ledger exists for.
        t.push(format!(
            "execute store result score #in_leth dw.sys if entity @e[tag={tag},{}]",
            lethal_box(v)
        ));
        t.push("assert score #in_leth dw.sys matches 1".to_string());
        t.push(format!("function {ns}:lethal_{}", v.safe));
        t.push(format!(
            "execute store result score #hp_leth dw.sys run data get entity {sel} Health 100"
        ));
        t.push("assert score #hp_leth dw.sys matches ..0".to_string());
        t.push(format!("kill @e[tag={tag}]"));
        out.insert(
            format!(
                "packtest-datapack/data/{ns}/test/lethal_{}.mcfunction",
                v.safe
            ),
            lines(&t).into_bytes(),
        );

        // --- the wording guard, on a real player object ---
        //
        // The template above proves the volume KILLS. This one proves it does not
        // CLAIM a death it did not cause — the half that was wrong twice.
        //
        // **What this tier can and cannot witness, measured rather than assumed.**
        // A PackTest fake player is **permanently undamageable**: on the pinned
        // 1.21.11 toolserver a dummy in `playerGameType: 0` with `Invulnerable: 0`
        // and `Health: 20f` stood inside a volume whose loop swings every tick and
        // was still at `Health: 20f` after **202 ticks** — far past vanilla's
        // 59-tick post-respawn window — and `minecraft:generic` was refused
        // identically, so it is not damage-type specific. `/damage` reported
        // **success** throughout, which is why the guard cannot be
        // `execute store success` and reads the outcome instead ([`LETHAL_HP`]).
        //
        // So a player DEATH cannot be witnessed here at all, and this template
        // does not pretend to: that claim belongs to the bot tier, which drives a
        // real client. What the dummy is perfect for is the other direction — it
        // is a body that provably never dies, which makes it a standing fixture
        // for "nothing landed", stronger than the 3-second respawn window because
        // it never expires. An unconditional `tellraw` (the first version of this
        // verb) would print the volume's wording here every tick, forever, about a
        // death that is not happening.
        let (pin, psel) = pin_dummy(&format!("dw_lethp_{}", v.safe));
        let mut p = packtest_header(&format!(
            "{title}: lethal volume `{}` withholds its wording when the blow does not land \
             (spec-0031)",
            v.id
        ));
        p.push(format!("function {ns}:setup"));
        p.push(pin);
        p.push(format!(
            "tp {psel} {} {} {}",
            mid[0] as f64 + 0.5,
            mid[1],
            mid[2] as f64 + 0.5
        ));
        // Baseline the guard to the DEAD sentinel, so a kill function that never
        // ran at all cannot pass this by leaving the score untouched — the
        // binding, not the outcome, is what a zero would hide.
        p.push(format!("scoreboard players set {LETHAL_HP} 0"));
        // The volume's own DRIVER, not its kill function: the driver is what
        // carries the `@a[<box>]` re-bind, so this binds to the player path
        // existing and reaching a player standing in the box. Calling
        // `lethal_<id>_kill` directly — as the first version did — passes
        // unchanged with the player line deleted from the driver, which is a test
        // that examines the wrong object and reports green.
        p.push(format!("function {ns}:lethal_{}", v.safe));
        p.push(format!("assert score {LETHAL_HP} matches 1.."));
        out.insert(
            format!(
                "packtest-datapack/data/{ns}/test/lethal_{}_claim.mcfunction",
                v.safe
            ),
            lines(&p).into_bytes(),
        );
    }
}

/// The `(amount, damage_type)` of the first `damage-players` effect declared in the
/// campaign (deep-walked through nested effect lists), in quest-then-trigger order.
/// `None` when the campaign uses no `damage-players`. Drives the damage PackTest.
fn first_damage_players(
    c: &delvewright_dsl::Campaign,
) -> Option<(u32, delvewright_dsl::DamageKind)> {
    use delvewright_dsl::DamageKind;
    let mut found: Option<(u32, DamageKind)> = None;
    let mut scan = |eff: &QuestEffect| {
        if found.is_none() {
            eff.visit_deep(&mut |e| {
                if found.is_none()
                    && let QuestEffect::DamagePlayers {
                        amount,
                        damage_type,
                        ..
                    } = e
                {
                    found = Some((*amount, damage_type.unwrap_or(DamageKind::Generic)));
                }
            });
        }
    };
    // Every root, inherited. Strictly additive: the roots keep their order, so the
    // "first" effect is unchanged for any campaign that has one in R1-R3 — this only
    // ever finds a `damage-players` where the generator previously found none and
    // emitted no damage PackTest at all.
    crate::plan::for_each_effect_root(c, &mut |_site, effs| {
        for eff in effs {
            scan(eff);
        }
    });
    found
}

/// v0.6 PackTests (spec-0014): a `spawn-actor` puppet appears and both despawn
/// styles remove it; a `move-actor` walks its puppet to the destination cell (its
/// `on_arrive` bundle runs on the same final tick); `unleash-actor` swaps the NoAI
/// puppet for a real-AI twin. Single-tick assertable; sequence-exact-tick timing and
/// per-tick yaw/NBT are covered by compiler unit tests (they assert the emitted
/// commands directly — stronger and faster than a timing gametest). Emits nothing
/// when the campaign declares no actors.
fn emit_v06_actor_packtests(
    plan: &Plan,
    out: &mut BuildOutput,
    actor_moves: &[crate::nav::ActorMovePlan],
) {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let actors = &c.quests.content.actors;
    if actors.is_empty() {
        return;
    }
    let mut write = |name: &str, body: Vec<String>| {
        out.insert(
            format!("packtest-datapack/data/{ns}/test/{name}.mcfunction"),
            lines(&body).into_bytes(),
        );
    };

    // The four actor tests all drive the SAME first actor through its real (and
    // therefore shared) entity tags — `spawn_actor_<id>`'s idempotence guard is
    // `unless entity @e[tag=dw_actor_<id>]`, a tag the unleashed twin also
    // carries. On the shared-batch server a sibling's leftover (e.g. the twin
    // `v06_unleash` produced) therefore no-ops a later test's spawn while
    // matching none of its puppet asserts (the round-6 island flake:
    // `v06_spawn_idempotent` counted 0 puppets). Every actor test must
    // establish its own world: clear the actor tag on entry (never assume a
    // fresh world) and clear it again on exit (leave no poison for a sibling).
    // Each template is a single atomic function, so within it the entity state
    // cannot be interleaved.

    // spawn-actor + despawn kill/vanish: the puppet appears, and either style
    // removes it. The visible difference (kill = in-place death animation, vanish =
    // silent relocate-then-kill out of view) is a client-eyes distinction; CI
    // asserts both leave zero entities under the actor tag.
    if let Some(a) = actors.first() {
        let safe = plan::safe_local(a.id.as_str());
        let mut b = packtest_header(&format!(
            "{}: spawn-actor appears; despawn kill & vanish both remove it",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        b.push(format!("kill @e[tag=dw_actor_{safe}]"));
        b.push(format!("function {ns}:spawn_actor_{safe}"));
        b.push(format!(
            "execute store result score #sp_sdsp dw.sys if entity @e[tag=dw_actor_{safe}]"
        ));
        b.push("assert score #sp_sdsp dw.sys matches 1..".to_string());
        // kill style removes it.
        b.push(format!("kill @e[tag=dw_actor_{safe}]"));
        b.push(format!(
            "execute store result score #k_sdsp dw.sys if entity @e[tag=dw_actor_{safe}]"
        ));
        b.push("assert score #k_sdsp dw.sys matches 0".to_string());
        // re-spawn (idempotent), then vanish style also removes it — which also
        // leaves the world actor-free for the next test.
        b.push(format!("function {ns}:spawn_actor_{safe}"));
        b.push(format!("tp @e[tag=dw_actor_{safe}] ~ -128 ~"));
        b.push(format!("kill @e[tag=dw_actor_{safe}]"));
        b.push(format!(
            "execute store result score #v_sdsp dw.sys if entity @e[tag=dw_actor_{safe}]"
        ));
        b.push("assert score #v_sdsp dw.sys matches 0".to_string());
        write("v06_spawn_despawn", b);
    }

    // spawn-actor is idempotent (re-caging after unleash): two spawns yield exactly
    // one puppet, not two.
    if let Some(a) = actors.first() {
        let safe = plan::safe_local(a.id.as_str());
        let mut b = packtest_header(&format!(
            "{}: spawn-actor is idempotent (one puppet, not two)",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        b.push(format!("kill @e[tag=dw_actor_{safe}]"));
        b.push(format!("function {ns}:spawn_actor_{safe}"));
        b.push(format!("function {ns}:spawn_actor_{safe}"));
        b.push(format!(
            "execute store result score #n_sidm dw.sys if entity @e[tag=dw_pup_{safe}]"
        ));
        b.push("assert score #n_sidm dw.sys matches 1".to_string());
        b.push(format!("kill @e[tag=dw_actor_{safe}]"));
        write("v06_spawn_idempotent", b);
    }

    // unleash-actor: the NoAI puppet (dw_pup) is replaced by a real-AI twin (same
    // body tag, real entity type, no puppet marker).
    if let Some(a) = actors.first() {
        let safe = plan::safe_local(a.id.as_str());
        let mut b = packtest_header(&format!(
            "{}: unleash-actor swaps the puppet for a real-AI twin",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        b.push(format!("kill @e[tag=dw_actor_{safe}]"));
        b.push(format!("function {ns}:spawn_actor_{safe}"));
        b.push(format!(
            "execute store result score #pup_unl dw.sys if entity @e[tag=dw_pup_{safe}]"
        ));
        b.push("assert score #pup_unl dw.sys matches 1".to_string());
        b.push(format!("function {ns}:unleash_{safe}"));
        // puppet marker gone, one twin of the real entity type remains.
        b.push(format!(
            "execute store result score #pup2_unl dw.sys if entity @e[tag=dw_pup_{safe}]"
        ));
        b.push("assert score #pup2_unl dw.sys matches 0".to_string());
        b.push(format!(
            "execute store result score #twin_unl dw.sys if entity @e[type={},tag=dw_actor_{safe}]",
            a.entity
        ));
        b.push("assert score #twin_unl dw.sys matches 1".to_string());
        // The twin is this test's residue — without this kill it survives the
        // test, and any later spawn no-ops against its body tag while owning no
        // puppet marker (the exact v06_spawn_idempotent red).
        b.push(format!("kill @e[tag=dw_actor_{safe}]"));
        write("v06_unleash", b);
    }

    // move-actor: fast-forward the driver to its final waypoint (running on_arrive on
    // that same tick) and assert the puppet is at the destination cell.
    if let Some(m) = actor_moves.first() {
        let safe = plan::safe_local(&m.actor);
        let bare = moveactor_bare(&m.actor, &m.to_anchor, &m.gate_key);
        let total = m.ticks();
        let p = m.target;
        let mut b = packtest_header(&format!(
            "{}: move-actor walks its puppet to the destination cell",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        b.push(format!("kill @e[tag=dw_actor_{safe}]"));
        b.push(format!("function {ns}:spawn_actor_{safe}"));
        b.push(format!("scoreboard players set #at_{bare} dw.sys {total}"));
        b.push(format!("function {ns}:ma_tick_{bare}"));
        b.push(format!(
            "execute store result score #arr_mvac dw.sys if entity @e[tag=dw_pup_{safe},x={},dx=0,y={},dy=0,z={},dz=0]",
            p[0], p[1], p[2]
        ));
        b.push("assert score #arr_mvac dw.sys matches 1..".to_string());
        b.push(format!("kill @e[tag=dw_actor_{safe}]"));
        write("v06_move_actor", b);
    }

    // Walker→NPC handoff (round-6 island QA): the first move-actor whose
    // on_arrive fires a `spawn-npc` is a scene handoff — a scripted puppet
    // walks in, vanishes, and the real (dialogue-bearing) NPC takes its place.
    // The delve soft-locks if the handoff leaves the puppet standing or the NPC
    // short an entity, so pin it end to end: drive the arrival tick and assert
    // puppet gone, NPC body present, and exactly one interaction hitbox. Every
    // campaign gate is sealed first (its `close-gate` fill): the island beat
    // fires this handoff with the boulder down, and arrival must be immune to
    // sealed terrain — the driver is a tp chain, not pathfinding. Gates are
    // re-opened afterwards (fill air replace <block>), so the template leaves
    // no block residue for a sibling (batch model).
    let handoff = actor_moves.iter().find_map(|m| {
        all_campaign_effects(c).into_iter().find_map(|e| match e {
            QuestEffect::MoveActor {
                actor,
                to_anchor,
                on_arrive,
                ..
            } if actor.as_str() == m.actor && to_anchor.as_str() == m.to_anchor => on_arrive
                .iter()
                .find_map(|a| match a {
                    QuestEffect::SpawnNpc { npc, .. } => Some(npc.as_str().to_string()),
                    _ => None,
                })
                .map(|npc| (m, npc)),
            _ => None,
        })
    });
    if let Some((m, npc_id)) = handoff
        && let Some(npc_tag) = plan
            .npcs
            .iter()
            .find(|n| n.npc_id == npc_id)
            .map(|n| n.tag.clone())
    {
        let safe = plan::safe_local(&m.actor);
        let bare = moveactor_bare(&m.actor, &m.to_anchor, &m.gate_key);
        let total = m.ticks();
        // Every distinct gate a `close-gate` effect seals, in first-appearance
        // order (deterministic).
        let mut sealed: Vec<(&[i32; 3], &[i32; 3], &String)> = Vec::new();
        let mut seen: Vec<&str> = Vec::new();
        for e in all_campaign_effects(c) {
            if let QuestEffect::CloseGate { anchor, .. } = e
                && !seen.contains(&anchor.as_str())
            {
                seen.push(anchor.as_str());
                for ((_, name), resolved) in &plan.anchors {
                    if name == anchor.as_str()
                        && let ResolvedAnchor::Gate { from, to, block } = resolved
                    {
                        sealed.push((from, to, block));
                    }
                }
            }
        }
        let mut b = packtest_header(&format!(
            "{}: move-actor arrival hands off to NPC `{npc_id}` with every gate sealed",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        b.push(format!("kill @e[tag=dw_actor_{safe}]"));
        b.push(format!("kill @e[tag={npc_tag}]"));
        for (from, to, block) in &sealed {
            b.push(format!(
                "fill {} {} {} {} {} {} {}",
                from[0], from[1], from[2], to[0], to[1], to[2], block
            ));
        }
        b.push(format!("function {ns}:spawn_actor_{safe}"));
        b.push(format!("scoreboard players set #at_{bare} dw.sys {total}"));
        b.push(format!("function {ns}:ma_tick_{bare}"));
        b.push(format!(
            "execute store result score #pup_ahof dw.sys if entity @e[tag=dw_actor_{safe}]"
        ));
        b.push("assert score #pup_ahof dw.sys matches 0".to_string());
        b.push(format!(
            "execute store result score #npc_ahof dw.sys if entity @e[tag=dw_npc,tag={npc_tag}]"
        ));
        b.push("assert score #npc_ahof dw.sys matches 1".to_string());
        b.push(format!(
            "execute store result score #box_ahof dw.sys if entity @e[type=minecraft:interaction,tag={npc_tag}]"
        ));
        b.push("assert score #box_ahof dw.sys matches 1".to_string());
        // No residue: NPC out, actor tag out, gates back open.
        b.push(format!("kill @e[tag={npc_tag}]"));
        b.push(format!("kill @e[tag=dw_actor_{safe}]"));
        for (from, to, block) in &sealed {
            b.push(format!(
                "fill {} {} {} {} {} {} minecraft:air replace {}",
                from[0], from[1], from[2], to[0], to[1], to[2], block
            ));
        }
        write("v06_arrive_handoff", b);
    }
}

/// v0.4 PackTests (spec-0008): a prop appears only once its objective activates;
/// `despawn-npc` removes the body + interaction hitbox; `move-npc` walks to the
/// target anchor. Deterministic (no combat/advancement events).
fn emit_v04_packtests(plan: &Plan, out: &mut BuildOutput, moves: &[crate::nav::MovePlan]) {
    let ns = &plan.namespace;
    let c = plan.campaign;
    if !campaign_is_v03(plan) {
        return;
    }
    let mut write = |name: &str, body: Vec<String>| {
        out.insert(
            format!("packtest-datapack/data/{ns}/test/{name}.mcfunction"),
            lines(&body).into_bytes(),
        );
    };

    // prop appears on activation: the first interact objective carrying a prop.
    'prop: for q in &c.quests.content.quests {
        let area = plan.quest_area(q.id.as_str()).unwrap_or("");
        for o in &q.objectives {
            if let Objective::Interact {
                id,
                anchor,
                prop: Some(prop),
                ..
            } = o
                && let Some(pos) = plan.point(area, anchor.as_str())
            {
                let mut b = packtest_header(&format!(
                    "{}: prop `{}` appears only when its objective activates",
                    artifact_title(c),
                    prop.block
                ));
                b.push(format!("function {ns}:setup"));
                b.push(format!(
                    "setblock {} {} {} minecraft:air",
                    pos[0], pos[1], pos[2]
                ));
                b.push(format!(
                    "assert block {} {} {} minecraft:air",
                    pos[0], pos[1], pos[2]
                ));
                b.push(format!(
                    "function {ns}:activate_{}",
                    safe_obj_fn(id.as_str())
                ));
                b.push(format!(
                    "assert block {} {} {} {}",
                    pos[0], pos[1], pos[2], prop.block
                ));
                write("v04_prop", b);
                break 'prop;
            }
        }
    }

    // interact-marker lifecycle: a completed interact objective leaves
    // NO `minecraft:interaction` hitbox behind — it must not stay clickable, and a
    // leaked hitbox congests the critical-path bot. Activate the first interact
    // objective (summons the hitbox), assert it exists, complete it, assert the
    // interaction count under its tag is 0.
    'cleanup: for q in &c.quests.content.quests {
        let area = plan.quest_area(q.id.as_str()).unwrap_or("");
        for o in &q.objectives {
            if let Objective::Interact { id, anchor, .. } = o
                && plan.point(area, anchor.as_str()).is_some()
            {
                let tag = interact_entity_tag(id.as_str());
                let (pin, sel) = pin_dummy("dw_t_iclr");
                let mut b = packtest_header(&format!(
                    "{}: completing interact `{id}` removes its interaction hitbox",
                    artifact_title(c)
                ));
                b.push(format!("function {ns}:setup"));
                // Pin this test's own dummy (see `pin_dummy`): the completion runs
                // as it alone — an `@a`-wide completion would also complete the
                // objective on every sibling test's dummy.
                b.push(pin);
                b.push(format!(
                    "function {ns}:activate_{}",
                    safe_obj_fn(id.as_str())
                ));
                b.push(format!(
                    "execute store result score #before_iclr dw.sys if entity @e[type=minecraft:interaction,tag={tag}]"
                ));
                b.push("assert score #before_iclr dw.sys matches 1..".to_string());
                b.push(format!(
                    "execute as {sel} run function {ns}:complete_{}",
                    safe_obj_fn(id.as_str())
                ));
                b.push(format!(
                    "execute store result score #after_iclr dw.sys if entity @e[type=minecraft:interaction,tag={tag}]"
                ));
                b.push("assert score #after_iclr dw.sys matches 0".to_string());
                write("v04_interact_cleanup", b);
                break 'cleanup;
            }
        }
    }

    // despawn-npc removes body + interaction hitbox (both carry the id tag).
    //
    // Every root, every depth. This picked the first `despawn-npc` out of a
    // hand-rolled three-of-five chain that was also shallow, so a campaign whose
    // only `despawn-npc` sits in a `sequence` step, a trap payload or a dialogue
    // `on_respawn` bundle generated no despawn PackTest at all — the verb shipped
    // with nothing asserting it.
    let first_despawn_npc = {
        let mut found: Option<&delvewright_dsl::NpcId> = None;
        crate::plan::for_each_effect_root(c, &mut |_site, effs| {
            for e in effs {
                e.visit_deep(&mut |x| {
                    if found.is_none() {
                        found = x.despawn_npc();
                    }
                });
            }
        });
        found
    };
    if let Some(npc) = first_despawn_npc {
        let safe = plan::safe_local(npc.as_str());
        let mut b = packtest_header(&format!(
            "{}: despawn-npc removes body + hitbox",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        b.push("scoreboard players set #placed dw.sys 1".to_string());
        // Clear EVERY planned NPC tag, not just the target's: `setup_finish`'s
        // summons are unguarded, and on the shared-batch server the world init
        // (and any sibling test) has already run it — re-running it over live
        // NPCs would duplicate every body + hitbox (mirrors `npc_summons`).
        for npc in &plan.npcs {
            b.push(format!("kill @e[tag={}]", npc.tag));
        }
        b.push(format!("function {ns}:setup_finish"));
        // A `deferred` NPC (DSL v0.6) is deliberately absent after `setup_finish` —
        // it enters via `spawn-npc`. Fire its entrance here so the despawn path is
        // exercised against the same body+hitbox pair a scripted entrance places
        // (the presence assertion below is unchanged, and stays a real assertion).
        // No line is emitted for a non-deferred target → byte-identical output for
        // campaigns that declare no deferred NPC.
        // The guard mirrors `spawn_npc_fns` exactly (planned NPC + `deferred`), so
        // the test never calls an entrance function that was not emitted.
        if plan
            .npcs
            .iter()
            .any(|n| n.npc_id == npc.as_str() && npc_is_deferred(c, &n.npc_id))
        {
            b.push(format!("function {ns}:{}", spawn_npc_fn(npc.as_str())));
        }
        // body + interaction hitbox both carry `dw_npc_<npc>` → two entities.
        b.push(format!(
            "execute store result score #before_ndsp dw.sys if entity @e[tag=dw_npc_{safe}]"
        ));
        b.push("assert score #before_ndsp dw.sys matches 2".to_string());
        b.push(format!("kill @e[tag=dw_npc_{safe}]"));
        b.push(format!(
            "execute store result score #after_ndsp dw.sys if entity @e[tag=dw_npc_{safe}]"
        ));
        b.push("assert score #after_ndsp dw.sys matches 0".to_string());
        write("v04_despawn", b);
    }

    // strike trigger on an NPC's anchor (round-4 island QA): the NPC's own
    // interaction hitbox is the entity a left-click actually reaches, so it must
    // carry the trigger's tag and its `attack` record must drive the trigger.
    // Simulating the record with `/data modify` reproduces exactly what vanilla
    // writes on a left-click — the primitive under test — without needing a bot
    // to swing. Emitted only when the collision exists.
    if let Some((trigger, npc_id, npc_tag)) = first_strike_trigger_on_npc(plan) {
        let id = plan::safe_local(trigger.id.as_str());
        let hitbox = format!("@e[type=minecraft:interaction,tag={npc_tag},limit=1]");
        let mut b = packtest_header(&format!(
            "{}: striking NPC `{npc_id}` fires trigger `{}` exactly once",
            artifact_title(c),
            trigger.id.as_str()
        ));
        b.push(format!("function {ns}:setup"));
        b.push("scoreboard players set #placed dw.sys 1".to_string());
        // Clear EVERY planned NPC tag before re-running `setup_finish`: its
        // summons are unguarded, and the world init (and any sibling test) has
        // already run it on the shared-batch server — duplicated hitboxes would
        // break the exact-count routing assert below (mirrors `npc_summons`).
        for n in &plan.npcs {
            b.push(format!("kill @e[tag={}]", n.tag));
        }
        b.push(format!("function {ns}:setup_finish"));
        // A `deferred` NPC (DSL v0.6) is deliberately absent after `setup_finish`
        // — a sleeping giant who only enters on cue is a natural strike target, so
        // fire its entrance here (mirrors the `v04_despawn` PackTest). No line is
        // emitted for a non-deferred target.
        if npc_is_deferred(c, &npc_id) {
            b.push(format!("function {ns}:{}", spawn_npc_fn(&npc_id)));
        }
        // The routing itself: the NPC's hitbox wears the trigger's tag, so the
        // trigger's single selector reaches it.
        b.push(format!(
            "execute store result score #route_stnp dw.sys if entity @e[type=minecraft:interaction,tag={npc_tag},tag=dw_trig_{id}]"
        ));
        b.push("assert score #route_stnp dw.sys matches 1".to_string());
        if trigger.once {
            b.push(format!("scoreboard players set #trig_{id} dw.sys 0"));
        }
        // Vanilla writes this compound when a player left-clicks an interaction
        // entity; write it by hand to stand in for the swing.
        b.push(format!(
            "data modify entity {hitbox} attack set value {{player:[I;0,0,0,0],timestamp:1L}}"
        ));
        b.push(format!(
            "execute store result score #rec_stnp dw.sys if data entity {hitbox} attack"
        ));
        b.push("assert score #rec_stnp dw.sys matches 1".to_string());
        b.push(format!("function {ns}:tick"));
        if trigger.once {
            b.push(format!("assert score #trig_{id} dw.sys matches 1"));
        }
        // Exactly once: the same tick pass consumed the record, so a second pass
        // over an untouched hitbox cannot re-fire.
        b.push(format!(
            "execute store result score #rec_stnp dw.sys if data entity {hitbox} attack"
        ));
        b.push("assert score #rec_stnp dw.sys matches 0".to_string());
        if trigger.once {
            b.push(format!("scoreboard players set #trig_{id} dw.sys 0"));
            b.push(format!("function {ns}:tick"));
            b.push(format!("assert score #trig_{id} dw.sys matches 0"));
        }
        // Separability — the property the whole `strike-npc` form rests on. One
        // `minecraft:interaction` records the two click kinds in two distinct
        // NBT fields: a left-click writes `attack`, a right-click writes
        // `interaction`. Write the RIGHT-click record on the shared hitbox and
        // tick: the left-click trigger must not fire and no `attack` record may
        // appear. That is what lets the NPC's dialogue keep the right-click
        // while this trigger takes the left-click on the very same entity.
        if trigger.once {
            b.push(format!("scoreboard players set #trig_{id} dw.sys 0"));
        }
        b.push(format!(
            "data modify entity {hitbox} interaction set value {{player:[I;0,0,0,0],timestamp:1L}}"
        ));
        b.push(format!(
            "execute store result score #rc_stnp dw.sys if data entity {hitbox} attack"
        ));
        b.push("assert score #rc_stnp dw.sys matches 0".to_string());
        b.push(format!("function {ns}:tick"));
        if trigger.once {
            b.push(format!("assert score #trig_{id} dw.sys matches 0"));
        }
        b.push(format!("data remove entity {hitbox} interaction"));
        write("v04_strike_npc", b);

        // Round-6 island QA regression: the owner attacked the giant, then could
        // never open its dialogue. Root cause was not the attack — it was a
        // second, exactly co-located interaction entity (the trigger's own
        // world-init summon), which the client's ray-pick tie-break preferred,
        // so right-clicks landed on an entity without the `dw_npc_<n>` tag and
        // the dialogue advancement never fired. The invariant that ends the
        // ambiguity — and the thing this test pins — is *one cell, one hitbox*:
        // the NPC's hitbox is the only interaction entity wearing the trigger's
        // tag, before AND after an attack record lands and is consumed, so any
        // click (left or right) can only ever reach the dialogue-bearing entity.
        let mut b = packtest_header(&format!(
            "{}: attack-then-talk — NPC `{npc_id}`'s hitbox is the only click target at its anchor",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        b.push("scoreboard players set #placed dw.sys 1".to_string());
        for n in &plan.npcs {
            b.push(format!("kill @e[tag={}]", n.tag));
        }
        b.push(format!("function {ns}:setup_finish"));
        if npc_is_deferred(c, &npc_id) {
            b.push(format!("function {ns}:{}", spawn_npc_fn(&npc_id)));
        }
        // One hitbox wears the trigger tag, and none wears it without also being
        // the NPC's — the standalone summon of the pre-fix emission trips this.
        b.push(format!(
            "execute store result score #one_stlk dw.sys if entity @e[type=minecraft:interaction,tag=dw_trig_{id}]"
        ));
        b.push("assert score #one_stlk dw.sys matches 1".to_string());
        b.push(format!(
            "execute store result score #orph_stlk dw.sys if entity @e[type=minecraft:interaction,tag=dw_trig_{id},tag=!{npc_tag}]"
        ));
        b.push("assert score #orph_stlk dw.sys matches 0".to_string());
        // The owner's sequence: a left-click record lands on the shared hitbox…
        // (The record is consumed by hand rather than via `tick`: a sibling
        // template's dummy may legitimately hold this trigger's gate flag, and a
        // real tick could then fire the trigger's content effects mid-test —
        // batch templates must be interleaving-independent. Consumption itself
        // is v04_strike_npc's assertion.)
        b.push(format!(
            "data modify entity {hitbox} attack set value {{player:[I;0,0,0,0],timestamp:1L}}"
        ));
        // …and the dialogue hitbox is still the one and only click target.
        b.push(format!(
            "execute store result score #one2_stlk dw.sys if entity @e[type=minecraft:interaction,tag={npc_tag}]"
        ));
        b.push("assert score #one2_stlk dw.sys matches 1".to_string());
        b.push(format!(
            "execute store result score #orph2_stlk dw.sys if entity @e[type=minecraft:interaction,tag=dw_trig_{id},tag=!{npc_tag}]"
        ));
        b.push("assert score #orph2_stlk dw.sys matches 0".to_string());
        // No residue: clear the hand-written record (the runtime consume line).
        b.push(format!(
            "execute as @e[type=minecraft:interaction,tag={npc_tag}] run data remove entity @s attack"
        ));
        write("v04_strike_talk", b);
    }

    // move-npc walks a collision-safe path that ends with the NPC at the target
    // anchor. The walk is a per-tick self-scheduling driver; to assert the
    // endpoint in a single tick, fast-forward the tick counter to the final
    // waypoint and run the driver once (the reschedule it queues is harmless in a
    // PackTest). Uses the same MovePlan the emitter drove, so the asserted target
    // is the path's real final waypoint.
    if let Some(m) = moves.first() {
        let safe = plan::safe_local(&m.npc);
        let bare = movenpc_bare(&m.npc, &m.to_anchor, &m.gate_key);
        let total = m.ticks();
        let p = m.target;
        let mut b = packtest_header(&format!(
            "{}: move-npc walks to its target anchor",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        b.push("scoreboard players set #placed dw.sys 1".to_string());
        // Clear EVERY planned NPC tag before re-running the unguarded
        // `setup_finish` (see `v04_despawn`/`npc_summons`): a duplicated walker
        // would leave a stray body behind at the start cell.
        for n in &plan.npcs {
            b.push(format!("kill @e[tag={}]", n.tag));
        }
        b.push(format!("function {ns}:setup_finish"));
        // Jump the driver to its last tick, then execute the final waypoint tp.
        b.push(format!("scoreboard players set #mt_{bare} dw.sys {total}"));
        b.push(format!("function {ns}:mv_tick_{bare}"));
        b.push(format!(
            "execute store result score #npos_nmov dw.sys if entity @e[tag=dw_npc_{safe},x={},dx=0,y={},dy=0,z={},dz=0]",
            p[0], p[1], p[2]
        ));
        b.push("assert score #npos_nmov dw.sys matches 1..".to_string());
        write("v04_move", b);
    }

    // kill-less spawn-wave (spec-0008 §4 live threat): a `spawn-wave` fired from a
    // reach/interact step — with NO `kill` objective draining that wave — still
    // spawns its mobs. Regression for the emitter bug where `wave_spawn_pos`
    // resolved a spawn position ONLY from a `kill` objective, so the `spawn_<wave>`
    // function was never emitted and the effect's `function …:spawn_<wave>` call
    // dangled (the wave silently never appeared). Picks the first such wave, spawns
    // it, and asserts exactly its mob count exists under the wave tag.
    let killed: BTreeSet<&str> = c
        .quests
        .content
        .quests
        .iter()
        .flat_map(|q| &q.objectives)
        .filter_map(|o| match o {
            Objective::Kill { wave, .. } => Some(wave.as_str()),
            _ => None,
        })
        .collect();
    'killless: for q in &c.quests.content.quests {
        for (obj_id, effs) in &q.on_objective_complete {
            let from_reach_or_interact = q.objectives.iter().any(|o| {
                o.id().as_str() == obj_id.as_str()
                    && matches!(
                        o,
                        Objective::ReachAnchor { .. } | Objective::Interact { .. }
                    )
            });
            if !from_reach_or_interact {
                continue;
            }
            for e in effs {
                if let Some(wave) = e.spawn_wave()
                    && !killed.contains(wave.as_str())
                    && let Some(w) = plan::wave_of(c, wave.as_str())
                {
                    let total = plan::wave_total(w);
                    let ws = plan::safe_local(wave.as_str());
                    let mut b = packtest_header(&format!(
                        "{}: kill-less spawn-wave `{wave}` spawns its mobs",
                        artifact_title(c)
                    ));
                    b.push(format!("function {ns}:setup"));
                    // Clear the wave tag first — a sibling test (`campaign` drives
                    // every objective completion, which can fire this very
                    // spawn-wave effect) may have already spawned it, and the
                    // exact-count assert needs a known-empty tag.
                    b.push(format!("kill @e[tag={}]", plan::wave_tag(wave.as_str())));
                    // No wave is live yet; the effect's driver spawns it.
                    b.push(format!("function {ns}:spawn_{ws}"));
                    b.push(format!(
                        "execute store result score #kw_klwv dw.sys if entity @e[tag={}]",
                        plan::wave_tag(wave.as_str())
                    ));
                    b.push(format!("assert score #kw_klwv dw.sys matches {total}"));
                    write("v04_killless_wave", b);
                    break 'killless;
                }
            }
        }
    }

    // Dialogue display gating: a `completes` option is DISPLAYED iff
    // its objective is active — its quest active and the objective not yet
    // complete — mirroring the click-handler guard. The chooser's `dmask_<npc>_<node>`
    // computes the per-player availability bitmask (bit `i` = the node's i-th
    // gated option is displayable); the variant it shows is `__m<mask>`. This test
    // drives that mask for the first gated completing option and asserts *that
    // option's isolated bit* (not the whole mask — sibling options in the node can
    // share a quest-active score) is 0 before the quest activates, 1 while active,
    // and 0 again after the objective completes. If the node also has a flag-gated
    // option, a final phase sets that flag in isolation and asserts its bit flips —
    // proving the flag axis is unchanged and independent of the objective-state axis.
    let v04 = campaign_is_v04(plan);
    'dlg: for npc in &plan.npcs {
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for probe in &npc.options {
            if !seen.insert(probe.node_id.as_str()) {
                continue;
            }
            let gated = node_gated_options(npc, &probe.node_id, v04);
            // The option under test: the first gated option that completes an
            // objective with a resolvable quest (the objective-state axis).
            let Some((b, under_test, qid, obj)) = gated.iter().enumerate().find_map(|(i, o)| {
                o.completes
                    .iter()
                    .find_map(|obj| objective_quest(c, obj).map(|(q, _)| (q, obj)))
                    .map(|(q, obj)| (i, *o, q, obj.as_str()))
            }) else {
                continue;
            };
            let node_safe = plan::safe_local(&probe.node_id);
            let dmask = format!("{ns}:dmask_{}_{}", npc.safe, node_safe);
            let qa = quest_active_score(qid);
            let os = obj_score(obj);

            // Every score any of this node's gated options reads — zeroed so the
            // mask isolates the bit under test (campaign-start quests would else
            // leave sibling bits set).
            let mut reset: BTreeSet<String> = BTreeSet::new();
            for g in &gated {
                for f in &g.requires_flags {
                    reset.insert(plan::flag_score(f));
                }
                for f in &g.forbids_flags {
                    reset.insert(plan::flag_score(f));
                }
                for o in &g.completes {
                    if let Some((q, _)) = objective_quest(c, o) {
                        reset.insert(quest_active_score(q));
                        reset.insert(obj_score(o));
                    }
                }
            }

            let (pin, sel) = pin_dummy("dw_t_dvis");
            let mut bt = packtest_header(&format!(
                "{}: dialogue option `{}` is displayed only while its objective `{obj}` is active",
                artifact_title(c),
                plain(&under_test.label)
            ));
            bt.push(format!("function {ns}:setup"));
            // Pin this test's own dummy (see `pin_dummy`): with one dummy PER
            // test coexisting on the batch server, an `as @a` mask run + copy
            // would read the LAST dummy the selector visits — a foreign one.
            bt.push(pin);
            let clear = |bt: &mut Vec<String>| {
                for s in &reset {
                    bt.push(format!("scoreboard players set {} {s} 0", plan::PARTY));
                }
            };
            // Run the mask, then ISOLATE the option-under-test's bit before the
            // assert: `(dw.dmask >> bit) & 1` via `%= 2^(bit+1)` then `/= 2^bit`. A
            // node's other gated options can share a quest-active score (e.g. two
            // options completing objectives of the same quest), so activating that
            // quest lights several bits at once — comparing the *whole* `dw.dmask`
            // would then read a sibling's bit as this option's and mis-assert.
            let assert_bit = |bt: &mut Vec<String>, bit: usize, present: bool| {
                bt.push(format!("execute as {sel} run function {dmask}"));
                // Copy the pinned dummy's mask into a fake player. `as {sel}` keeps
                // the read single-entity (`= @s …`): `scoreboard players
                // get`/`operation` reject a multi-entity selector.
                bt.push(format!(
                    "execute as {sel} run scoreboard players operation #dm_dvis dw.sys = @s dw.dmask"
                ));
                bt.push(format!(
                    "scoreboard players set #dmhi_dvis dw.sys {}",
                    1u32 << (bit + 1)
                ));
                bt.push(
                    "scoreboard players operation #dm_dvis dw.sys %= #dmhi_dvis dw.sys".to_string(),
                );
                bt.push(format!(
                    "scoreboard players set #dmlo_dvis dw.sys {}",
                    1u32 << bit
                ));
                bt.push(
                    "scoreboard players operation #dm_dvis dw.sys /= #dmlo_dvis dw.sys".to_string(),
                );
                bt.push(format!(
                    "assert score #dm_dvis dw.sys matches {}",
                    u32::from(present)
                ));
            };

            // Phase A — quest inactive: the option is hidden (its bit is 0).
            clear(&mut bt);
            assert_bit(&mut bt, b, false);
            // Phase B — quest active, objective incomplete: the option appears.
            clear(&mut bt);
            bt.push(format!("scoreboard players set {} {qa} 1", plan::PARTY));
            assert_bit(&mut bt, b, true);
            // Phase C — objective complete: the option disappears again.
            bt.push(format!("scoreboard players set {} {os} 1", plan::PARTY));
            assert_bit(&mut bt, b, false);

            // Flag axis: a flag-only gated option's bit flips with its flag alone,
            // independent of the objective-state axis.
            if let Some((bf, flag_opt)) = gated
                .iter()
                .enumerate()
                .find(|(_, o)| !o.requires_flags.is_empty() && o.completes.is_empty())
            {
                clear(&mut bt);
                for f in &flag_opt.requires_flags {
                    bt.push(format!(
                        "scoreboard players set {} {} 1",
                        plan::PARTY,
                        plan::flag_score(f)
                    ));
                }
                assert_bit(&mut bt, bf, true);
            }

            write("v04_dialogue_visibility", bt);
            break 'dlg;
        }
    }
}

/// One real `tick` pass with every player on the batch server shielded from harm.
///
/// A trigger template that asserts *which* trigger fired has to run the real `tick`,
/// which runs the trigger's real effects — and a delve's effects include
/// `damage-players` (the island's `his-house` deals 40, twice a dummy's health).
/// PackTest runs every generated template as one batch against one shared server, so
/// an unshielded pass would kill sibling templates' dummies for reasons that have
/// nothing to do with what they test.
///
/// Resistance V is total immunity to the `minecraft:generic` damage the effect
/// emits, and it is scaffolding around the pass, not part of any assertion: the
/// claims are all reads of `#trig_<id>`. The damage effect itself is pinned by its
/// own test.
fn shielded_tick(ns: &str) -> Vec<String> {
    vec![
        "effect give @a minecraft:resistance 1 4 true".to_string(),
        format!("function {ns}:tick"),
        "effect clear @a minecraft:resistance".to_string(),
    ]
}

/// The first ordered pair of click triggers that ride ONE NPC's interaction hitbox
/// and can be told apart by flags: `(npc id, npc body tag, earlier, later)`, where
/// the *later* trigger's open-assignment provably shuts the *earlier* one.
///
/// Direction matters. The starvation bug was order-dependent — the earlier-declared
/// trigger's inline `data remove` ate the click record — so the pair worth pinning
/// is exactly "the later one must still fire while the earlier one is gated off".
/// `None` when the campaign has no such pair (nothing to test, nothing emitted).
fn first_shared_hitbox_pair<'a>(
    plan: &'a Plan,
) -> Option<(
    String,
    String,
    &'a delvewright_dsl::EnvTrigger,
    &'a delvewright_dsl::EnvTrigger,
)> {
    let c = plan.campaign;
    for n in &plan.npcs {
        let anchor = c
            .npcs
            .content
            .npcs
            .iter()
            .find(|d| d.id.as_str() == n.npc_id)
            .map(|d| d.anchor.as_str())
            .unwrap_or("");
        let riders: Vec<&delvewright_dsl::EnvTrigger> = c
            .quests
            .content
            .triggers
            .iter()
            .filter(|t| trigger_rides_npc(t, anchor, &n.npc_id))
            .collect();
        for (i, a) in riders.iter().enumerate() {
            for b in &riders[i + 1..] {
                if trigger_shut_under_open(a, b) {
                    return Some((n.npc_id.clone(), n.tag.clone(), a, b));
                }
            }
        }
    }
    None
}

/// Whether `a`'s gate is shut under the flag assignment that opens `b` — every flag
/// `b` requires set, every other flag (including everything `b` forbids) unset.
fn trigger_shut_under_open(
    a: &delvewright_dsl::EnvTrigger,
    b: &delvewright_dsl::EnvTrigger,
) -> bool {
    let set: Vec<&str> = b.requires_flags.iter().map(|f| f.as_str()).collect();
    // Shut if a required flag is not among the flags this assignment sets, or a
    // forbidden flag is.
    a.requires_flags.iter().any(|f| !set.contains(&f.as_str()))
        || a.forbids_flags.iter().any(|f| set.contains(&f.as_str()))
}

/// Generated PackTest for the round-8 island defect: **two flag-gated click triggers
/// on ONE NPC hitbox, both reachable, neither starving the other**.
///
/// The island's giant carried `wake-the-giant` (requires `flag/asleep`) and
/// `his-house` (requires `flag/sealed`, forbids `flag/asleep`) on a single
/// interaction entity. The old emission cleared the `attack` record inline, per
/// trigger, immediately after that trigger's own fire clause — so the
/// earlier-declared `wake-the-giant` consumed the click even with its gate shut and
/// `his-house` could never fire. Declaration order silently decided which of two
/// legal triggers worked.
///
/// The template drives the real hardware: it writes the `attack` compound vanilla
/// writes on a left-click, runs the real `tick`, and reads the per-trigger fire
/// sentinel `#trig_<id>` (see [`env_trigger_fns`]) to see which one actually ran.
/// Both directions are asserted — the gated-off trigger must stay silent AND its
/// sibling must fire — so the test fails both on the original starvation and on any
/// future change that lets a shut gate fire.
///
/// Emitted only for a campaign that has such a pair, so every other campaign is
/// byte-identical.
fn emit_shared_hitbox_packtest(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let Some((npc_id, npc_tag, early, late)) = first_shared_hitbox_pair(plan) else {
        return;
    };
    let a = plan::safe_local(early.id.as_str());
    let b = plan::safe_local(late.id.as_str());
    let hitbox = format!("@e[type=minecraft:interaction,tag={npc_tag},limit=1]");
    let rec_late = trigger_record(late);
    let rec_early = trigger_record(early);

    // Every flag either trigger names, deduplicated in declaration order — the set
    // the template writes and must hand back untouched (flags are party state, and
    // the batch server is shared).
    let mut flags: Vec<&str> = Vec::new();
    for t in [early, late] {
        for f in t.requires_flags.iter().chain(t.forbids_flags.iter()) {
            if !flags.contains(&f.as_str()) {
                flags.push(f.as_str());
            }
        }
    }
    // `open` writes the assignment that opens `t`: every required flag set, every
    // other named flag cleared.
    let open = |t: &delvewright_dsl::EnvTrigger| -> Vec<String> {
        flags
            .iter()
            .map(|f| {
                let want = usize::from(t.requires_flags.iter().any(|r| r.as_str() == *f));
                format!(
                    "scoreboard players set {} {} {want}",
                    plan::PARTY,
                    plan::flag_score(f)
                )
            })
            .collect()
    };

    let mut t = packtest_header(&format!(
        "{}: `{}` and `{}` share NPC `{npc_id}`'s hitbox — each fires on its own flags",
        artifact_title(c),
        early.id.as_str(),
        late.id.as_str()
    ));
    t.push(format!("function {ns}:setup"));
    t.push("scoreboard players set #placed dw.sys 1".to_string());
    // Own init (batch contract): rebuild every NPC so exactly one hitbox exists.
    for n in &plan.npcs {
        t.push(format!("kill @e[tag={}]", n.tag));
    }
    t.push(format!("function {ns}:setup_finish"));
    if npc_is_deferred(c, &npc_id) {
        t.push(format!("function {ns}:{}", spawn_npc_fn(&npc_id)));
    }
    // Precondition: ONE interaction entity wears BOTH trigger tags. Without this the
    // rest of the template would pass vacuously on two separate hitboxes.
    t.push(format!(
        "execute store result score #shr_one dw.sys if entity @e[type=minecraft:interaction,tag=dw_trig_{a},tag=dw_trig_{b}]"
    ));
    t.push("assert score #shr_one dw.sys matches 1".to_string());

    // --- The regression. Later trigger open, earlier trigger gated shut. ---
    t.extend(open(late));
    t.push(format!("scoreboard players set #trig_{a} dw.sys 0"));
    t.push(format!("scoreboard players set #trig_{b} dw.sys 0"));
    t.push(format!(
        "data modify entity {hitbox} {rec_late} set value {{player:[I;0,0,0,0],timestamp:1L}}"
    ));
    t.extend(shielded_tick(ns));
    // The starved trigger: 0 before the fix, 1 after.
    t.push(format!("assert score #trig_{b} dw.sys matches 1"));
    // …and the gated-off one stayed silent, which is what made its consumption a bug.
    t.push(format!("assert score #trig_{a} dw.sys matches 0"));
    // Consumption is unchanged: the record is gone by the end of the same pass.
    t.push(format!(
        "execute store result score #shr_rec dw.sys if data entity {hitbox} {rec_late}"
    ));
    t.push("assert score #shr_rec dw.sys matches 0".to_string());

    // --- The mirror. Earlier trigger open: it fires, so both are reachable. ---
    // Rebuild the hitbox first — the earlier trigger's own effects may have removed
    // the NPC (the island's `wake-the-giant` despawns the giant it wakes).
    for n in &plan.npcs {
        t.push(format!("kill @e[tag={}]", n.tag));
    }
    t.push(format!("function {ns}:setup_finish"));
    if npc_is_deferred(c, &npc_id) {
        t.push(format!("function {ns}:{}", spawn_npc_fn(&npc_id)));
    }
    t.extend(open(early));
    t.push(format!("scoreboard players set #trig_{a} dw.sys 0"));
    t.push(format!("scoreboard players set #trig_{b} dw.sys 0"));
    t.push(format!(
        "data modify entity {hitbox} {rec_early} set value {{player:[I;0,0,0,0],timestamp:1L}}"
    ));
    t.extend(shielded_tick(ns));
    t.push(format!("assert score #trig_{a} dw.sys matches 1"));

    // Leave no poison: clear every flag written, drop any actor the fired triggers
    // staged, and put the NPCs back the way `setup_finish` makes them.
    for f in &flags {
        t.push(format!(
            "scoreboard players set {} {} 0",
            plan::PARTY,
            plan::flag_score(f)
        ));
    }
    for act in &c.quests.content.actors {
        t.push(format!(
            "kill @e[tag=dw_actor_{}]",
            plan::safe_local(act.id.as_str())
        ));
    }
    for n in &plan.npcs {
        t.push(format!("kill @e[tag={}]", n.tag));
    }
    t.push(format!("function {ns}:setup_finish"));
    out.insert(
        format!("packtest-datapack/data/{ns}/test/v06_shared_hitbox.mcfunction"),
        lines(&t).into_bytes(),
    );
}

/// The header lines shared by every generated PackTest (`# @dummy` + timeout).
fn packtest_header(title: &str) -> Vec<String> {
    vec![
        format!("#> {title}"),
        "# @dummy".to_string(),
        "# @timeout 100".to_string(),
        String::new(),
    ]
}

/// Pin a PackTest template's own dummy player: the pin line (`tag @p add …`)
/// plus the selector that addresses that dummy — and only it — thereafter.
///
/// PackTest runs the whole generated suite as ONE batch on one shared server:
/// each `# @dummy` test spawns its OWN dummy, all dummies coexist, and every
/// test function executes over the same server tick(s), in an order the
/// compiler does not control. Consequences for template authorship — the hard
/// rule is **every generated test is interleaving-independent: own dummy, own
/// scores, own init**:
///
/// 1. `@p` re-resolves from the test structure origin on every command — the
///    moment a template teleports its dummy to absolute campaign coordinates,
///    `@p` retargets to a NEIGHBOR test's dummy and all later writes/asserts
///    land on the wrong player (round-5 island red: `v06_stealth` read a
///    foreign dummy's grace). A template that drives per-player state must tag
///    its dummy on the first post-setup line — while its own dummy, inside its
///    own structure, is still the nearest player — and address it exclusively
///    through the tag (which, unlike `@p`, also keeps matching a dummy that
///    content effects have killed). A template PackTest executes AS its dummy
///    may use `@s` instead — the binding survives teleports.
/// 2. An `@a` write hits every test's dummy, so a sibling template can pre-set
///    state this test believes it controls (round-5 island red:
///    `verb_flag_gate`'s "withheld" flag arrived via `verb_interact`'s `@a`).
///    Templates never write `@a`-wide, and every score a template asserts on
///    is actively initialized by that template ("never set" is not 0 here).
/// 3. Fake-player scratch holders on `dw.sys` are batch-global: every template
///    suffixes its own (`#n_sidm`, `#bx_bret`, …) so no two templates share a
///    holder. Real runtime scores (`#stealth`, `#placed`, `#trig_<id>`, move
///    drivers) are deliberately shared — tests drive them and must initialize
///    them explicitly.
/// 4. Entity state is batch-global too: a sibling's residue can defeat a
///    guarded summon (round-6 island red: `v06_unleash`'s leftover twin
///    carried `dw_actor_<id>` with no puppet marker, so
///    `v06_spawn_idempotent`'s guarded spawns no-op'd and it counted 0
///    puppets), and re-running the unguarded `setup_finish` over live NPCs
///    duplicates them. A template clears every entity tag it counts on at
///    entry and leaves none of its own residue behind; each template is a
///    single atomic function, so within it nothing can be interleaved.
fn pin_dummy(tag: &str) -> (String, String) {
    (
        format!("tag @p add {tag}"),
        format!("@a[tag={tag},limit=1]"),
    )
}

/// The guard half of [`packtest_preamble`]: every progression term an
/// objective's activation gate READS, pinned to the value that opens (or, with
/// `with_flags: false`, withholds) it — quest active, `after` prerequisites,
/// `requires_flags`, and `forbids_flags` actively cleared.
///
/// Split out because a template that must prove something about **how the item
/// reaches the player** (the v0.8 named-stack collect) cannot use the preamble's
/// own `give`: handing the plain item over first completes the objective and
/// makes the named stack's assertion vacuous. Everything about which flags are
/// pinned stays in one place, so no template can be written that opens a gate by
/// hand and forgets one — template flag hygiene lives here.
fn packtest_guards(plan: &Plan, quest_id: &str, o: &Objective, with_flags: bool) -> Vec<String> {
    let party = plan::PARTY;
    let mut p = vec![format!(
        "scoreboard players set {party} {} 1",
        quest_active_score(quest_id)
    )];
    for a in o.after() {
        p.push(format!(
            "scoreboard players set {party} {} 1",
            obj_score(a.as_str())
        ));
    }
    for f in o.requires_flags() {
        p.push(format!(
            "scoreboard players set {party} {} {}",
            plan::flag_score(f.as_str()),
            if with_flags { 1 } else { 0 }
        ));
    }
    // v0.6 negative gate: actively clear every forbidden flag so the objective
    // is not suppressed by a sibling template's leftover state (same batch-server
    // reasoning as the `with_flags: false` clearing above).
    for f in o.forbids_flags() {
        p.push(format!(
            "scoreboard players set {party} {} 0",
            plan::flag_score(f.as_str())
        ));
    }
    // v0.10 numeric gate (spec-0031): the datum is DRIVEN to a value that opens
    // the gate, or — with `with_flags: false` — to one that shuts it, for the
    // same batch-server reason the flags are actively cleared rather than merely
    // left alone. A template that pinned the flags and left the numbers to
    // whatever a sibling template last wrote would be a coin toss dressed as a
    // proof.
    p.extend(state_drive_lines(plan, o.requires_state(), with_flags));
    p
}

/// Lines that satisfy an objective's activation guard (quest active, all `after`
/// prerequisites set, all `requires_flags` set, and any required item given to
/// `sel`). With `with_flags: false` the flags are not merely omitted but actively
/// cleared: PackTest runs the whole suite as one batch on one shared server, so
/// "never set" does not mean 0.
///
/// spec-0018: every progression term is written on the **party holder**, which is
/// the state the generated guards actually read. The holder is batch-global —
/// but each template is a single atomic mcfunction, so its baseline, its drive
/// and its assert all land inside one tick with no sibling in between (the one
/// place that stops being true is a template that `await`s, which
/// `tests/packtest_batch.rs` polices separately). Only the ITEM still goes to the
/// test's own pinned dummy.
fn packtest_preamble(
    plan: &Plan,
    quest_id: &str,
    o: &Objective,
    with_flags: bool,
    sel: &str,
) -> Vec<String> {
    let mut p = packtest_guards(plan, quest_id, o, with_flags);
    match o {
        Objective::Collect { item, count, .. } => {
            p.push(format!("give {sel} {item} {count}"));
        }
        Objective::Interact {
            requires_item: Some(it),
            ..
        } => {
            // HELD, not merely carried: the gate reads
            // `weapon.mainhand`, so the preamble must put the item THERE. `give`
            // only happened to satisfy the old inventory-wide gate because a fresh
            // dummy's first free slot is also its selected one — an accident, not a
            // guarantee, and exactly the kind of coincidence a test must not rest on.
            p.push(format!(
                "item replace entity {sel} weapon.mainhand with {it}"
            ));
        }
        _ => {}
    }
    p
}

/// Emit a per-verb mechanism PackTest for the first `kill` / `collect` /
/// `interact` objective, plus a flag-gate test for the first flag-gated
/// collect/interact objective and a forbid-gate test for the first
/// `forbids_flags`-gated one (v0.6 negative gate).
fn emit_verb_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let c = plan.campaign;

    let mut write = |name: &str, body: Vec<String>| {
        out.insert(
            format!("packtest-datapack/data/{ns}/test/{name}.mcfunction"),
            lines(&body).into_bytes(),
        );
    };

    // Collect (quest, objective) pairs by verb, in declared order.
    let mut first_kill = None;
    // Prefer a kill whose wave contains an armed mob so the armed-equipment assert
    // (M2 round-2 fix 1) is actually exercised — the equipment bug hid for a whole
    // milestone precisely because nothing looked. Falls back to the first kill.
    let mut first_armed_kill = None;
    let mut first_collect = None;
    // The first `collect` that adopts a prefab container (DSL v0.8).
    let mut first_collect_adopted = None;
    let mut first_interact = None;
    // The first `interact` that actually gates on an item — the subject of the
    // held-vs-carried test below. Distinct from `first_interact`, which may be
    // ungated and would make that test vacuous.
    let mut first_interact_item = None;
    let mut first_flag_gated = None;
    let mut first_forbid_gated = None;
    for q in &c.quests.content.quests {
        for o in &q.objectives {
            let qid = q.id.as_str();
            match o {
                Objective::Kill { wave, .. } => {
                    if first_kill.is_none() {
                        first_kill = Some((qid, o));
                    }
                    if first_armed_kill.is_none()
                        && plan::wave_of(c, wave.as_str()).is_some_and(|w| {
                            w.mobs.iter().any(|m| {
                                effective_mainhand(&m.entity, m.equipment.as_ref()).is_some()
                            })
                        })
                    {
                        first_armed_kill = Some((qid, o));
                    }
                }
                Objective::Collect { .. } if first_collect.is_none() => {
                    first_collect = Some((qid, o))
                }
                Objective::Interact { .. } => {
                    if first_interact.is_none() {
                        first_interact = Some((qid, o));
                    }
                    if first_interact_item.is_none()
                        && matches!(
                            o,
                            Objective::Interact {
                                requires_item: Some(_),
                                ..
                            }
                        )
                    {
                        first_interact_item = Some((qid, o));
                    }
                }
                _ => {}
            }
            // v0.8: the first `collect` that ADOPTS a prefab container.
            // Distinct from `first_collect`, which may keep the compiler-placed
            // chest and would make the adoption assertions vacuous.
            if first_collect_adopted.is_none() && o.collect_container().is_some() {
                first_collect_adopted = Some((qid, o));
            }
            if first_flag_gated.is_none()
                && !o.requires_flags().is_empty()
                && matches!(o, Objective::Collect { .. } | Objective::Interact { .. })
            {
                first_flag_gated = Some((qid, o));
            }
            if first_forbid_gated.is_none()
                && !o.forbids_flags().is_empty()
                && matches!(o, Objective::Collect { .. } | Objective::Interact { .. })
            {
                first_forbid_gated = Some((qid, o));
            }
        }
    }
    let first_kill = first_armed_kill.or(first_kill);

    // kill: spawn the wave, drain the countdown via the kill reward, tick,
    // assert the objective completed.
    if let Some((qid, o)) = first_kill
        && let Objective::Kill { id, wave, .. } = o
        && let Some(w) = plan::wave_of(c, wave.as_str())
    {
        let total = plan::wave_total(w);
        let ws = plan::safe_local(wave.as_str());
        let (pin, sel) = pin_dummy("dw_t_vkil");
        let mut b = packtest_header(&format!(
            "{}: kill wave `{wave}` -> countdown -> complete",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        // Pin this test's own dummy (see `pin_dummy`) and drive the whole chain
        // on it alone; actively zero the asserted objective first.
        b.push(pin);
        b.push(format!(
            "scoreboard players set {} {} 0",
            plan::PARTY,
            obj_score(id.as_str())
        ));
        b.extend(packtest_preamble(plan, qid, o, true, &sel));
        // Clear the wave tag before the fresh spawn — a sibling test may have
        // already fired this spawn-wave (`spawn_<wave>` is unguarded).
        b.push(format!("kill @e[tag={}]", plan::wave_tag(wave.as_str())));
        b.push(format!("function {ns}:spawn_{ws}"));
        b.push(format!(
            "assert score {} {} matches {total}",
            plan::wave_counter(wave.as_str()),
            plan::WAVE_OBJECTIVE
        ));
        // The armed mob really holds its weapon (M2 round-2 fix 1). `HandItems`
        // failed silently for a whole milestone because no test looked; this
        // exercises the vanilla `execute if items entity … weapon.mainhand …`
        // condition (1.21.11 `minecraft:item_slots` + `minecraft:item_predicate`)
        // and bridges the result to `assert score` — using only PackTest commands
        // known-good on the validation server, not a newer `assert items`.
        // The asserted item is the mob's *effective* main hand — an author's
        // `equipment.main_hand` override when present, the default table
        // otherwise — so the assertion always describes the summon this same
        // compiler emitted (see `effective_mainhand`).
        if let Some((mob, item)) = w
            .mobs
            .iter()
            .find_map(|m| effective_mainhand(&m.entity, m.equipment.as_ref()).map(|it| (m, it)))
        {
            b.push("scoreboard players set #armed_vkil dw.sys 0".to_string());
            b.push(format!(
                "execute if items entity @e[tag={},type={},limit=1] weapon.mainhand {item} \
                 run scoreboard players set #armed_vkil dw.sys 1",
                plan::wave_tag(wave.as_str()),
                mob.entity,
            ));
            b.push("assert score #armed_vkil dw.sys matches 1".to_string());
        }
        b.push(format!("kill @e[tag={}]", plan::wave_tag(wave.as_str())));
        for _ in 0..total {
            b.push(format!("execute as {sel} run function {ns}:k_reward_{ws}"));
        }
        b.push(format!("function {ns}:tick"));
        b.push(format!(
            "assert score {} {} matches 1",
            plan::PARTY,
            obj_score(id.as_str())
        ));
        write("verb_kill", b);
    }

    // collect: satisfy guards + hold the item, run the collect reward, assert.
    if let Some((qid, o)) = first_collect
        && let Objective::Collect { id, .. } = o
    {
        let (pin, sel) = pin_dummy("dw_t_vcol");
        let mut b = packtest_header(&format!(
            "{}: collect -> reward completes objective",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        // Pin this test's own dummy (see `pin_dummy`) and drive/assert on it
        // alone; actively zero the asserted objective first.
        b.push(pin);
        b.push(format!(
            "scoreboard players set {} {} 0",
            plan::PARTY,
            obj_score(id.as_str())
        ));
        b.extend(packtest_preamble(plan, qid, o, true, &sel));
        b.push(format!(
            "execute as {sel} run function {ns}:c_reward_{}",
            plan::safe_local(id.as_str())
        ));
        b.push(format!(
            "assert score {} {} matches 1",
            plan::PARTY,
            obj_score(id.as_str())
        ));
        write("verb_collect", b);
    }

    // interact: hold the required item, fire the trigger, tick, assert.
    if let Some((qid, o)) = first_interact
        && let Objective::Interact { id, .. } = o
    {
        let (pin, sel) = pin_dummy("dw_t_vint");
        let mut b = packtest_header(&format!(
            "{}: interact trigger + item -> complete",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        // Pin this test's own dummy (see `pin_dummy`) and drive/assert on it
        // alone — the old `@a`-wide preamble was the round-5 flag leak that
        // poisoned `verb_flag_gate`'s withheld phase.
        b.push(pin);
        b.push(format!(
            "scoreboard players set {} {} 0",
            plan::PARTY,
            obj_score(id.as_str())
        ));
        b.extend(packtest_preamble(plan, qid, o, true, &sel));
        b.push(format!(
            "scoreboard players set {sel} {} 1",
            plan::interact_trigger(id.as_str())
        ));
        b.push(format!("function {ns}:tick"));
        b.push(format!(
            "assert score {} {} matches 1",
            plan::PARTY,
            obj_score(id.as_str())
        ));
        write("verb_interact", b);

        // A click that lands before its quest is armed is DISCARDED, and a real
        // click afterwards still works.
        //
        // This is the runtime half of the arming invariant. The compile-time half
        // (`tests/tick_arming.rs`) pins that the arming quest's lines precede the
        // adjudication, so a pending click can never be lost to same-tick
        // ordering. What a live server has to show is the other half: the
        // unconditional reset really does SPEND a premature click rather than
        // bank it — because a banked click would auto-complete the objective the
        // instant the quest armed, with nobody having clicked anything.
        let (pin, sel) = pin_dummy("dw_t_varm");
        let trigger = plan::interact_trigger(id.as_str());
        let obj = obj_score(id.as_str());
        let qa = quest_active_score(qid);
        let party = plan::PARTY;
        let mut b = packtest_header(&format!(
            "{}: a click before the quest is armed is spent, not banked",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        b.push(pin);
        // Baseline: objective open, quest NOT armed, and the preamble's other
        // guards satisfied so the arming flag is the only thing standing in the
        // way. The preamble sets the quest active, so it is cleared after it.
        b.push(format!("scoreboard players set {party} {obj} 0"));
        b.extend(packtest_preamble(plan, qid, o, true, &sel));
        b.push(format!("scoreboard players set {party} {qa} 0"));

        // --- the premature click: no completion, and no banked trigger ---
        b.push(format!("scoreboard players set {sel} {trigger} 1"));
        b.push(format!("function {ns}:tick"));
        b.push(format!("assert score {party} {obj} matches 0"));
        b.push(format!(
            "execute store result score #varm_bank dw.sys if score {sel} {trigger} matches 1.."
        ));
        b.push("assert score #varm_bank dw.sys matches 0".to_string());

        // --- arming alone must not complete it: the click is genuinely gone ---
        b.push(format!("scoreboard players set {party} {qa} 1"));
        b.push(format!("function {ns}:tick"));
        b.push(format!("assert score {party} {obj} matches 0",));

        // --- and a real click, now that the quest is armed, still completes ---
        b.push(format!("scoreboard players set {sel} {trigger} 1"));
        b.push(format!("function {ns}:tick"));
        b.push(format!("assert score {party} {obj} matches 1"));
        write("verb_interact_arming", b);
    }

    // interact + `requires_item`: HELD, not merely carried. Two phases on one
    // dummy, one tick each:
    //
    //   A. the item is in the pack, the hand is empty  -> must NOT complete
    //   B. the same item is in the main hand           -> completes
    //
    // Phase A first proves the item really is carried (`if items entity @s
    // container.*` bridged to a score) — without that, "did not complete" would be
    // indistinguishable from "had no item at all", i.e. a vacuous test that the old
    // inventory-wide gate would also have passed.
    //
    // What this template deliberately does NOT assert is the `missing_item_hint`
    // narration: PackTest can observe scores, blocks and entities, but a `tellraw`
    // leaves no game state to assert against — there is no chat-log primitive, and
    // inventing a "did it narrate" scoreboard side-channel inside the emitted tick
    // would be a hack the player pays for (no-hack doctrine). The hint's emission is
    // proven instead by an exact-line assertion in `crates/compiler/tests/v07_*.rs`,
    // and its in-game appearance by the live tier. The mechanism it rides on — the
    // main-hand gate itself — is what this template proves.
    if let Some((qid, o)) = first_interact_item
        && let Objective::Interact {
            id,
            requires_item: Some(it),
            ..
        } = o
    {
        let (pin, sel) = pin_dummy("dw_t_vheld");
        let trigger = plan::interact_trigger(id.as_str());
        let party = plan::PARTY;
        let carried = "#carried_vheld";
        let mut b = packtest_header(&format!(
            "{}: `requires_item` is HELD — carried is not enough",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        b.push(pin);
        b.push(format!(
            "scoreboard players set {party} {} 0",
            obj_score(id.as_str())
        ));
        b.extend(packtest_preamble(plan, qid, o, true, &sel));
        // --- phase A: carried, not held ---
        b.push(format!(
            "item replace entity {sel} weapon.mainhand with minecraft:air"
        ));
        b.push(format!("item replace entity {sel} inventory.0 with {it}"));
        b.push(format!("scoreboard players set {carried} dw.sys 0"));
        b.push(format!(
            "execute as {sel} if items entity @s container.* {it} run scoreboard players set {carried} dw.sys 1"
        ));
        b.push(format!("assert score {carried} dw.sys matches 1"));
        b.push(format!("scoreboard players set {sel} {trigger} 1"));
        b.push(format!("function {ns}:tick"));
        b.push(format!(
            "assert score {party} {} matches 0",
            obj_score(id.as_str())
        ));
        // --- phase B: the same item, now presented ---
        b.push(format!(
            "item replace entity {sel} inventory.0 with minecraft:air"
        ));
        b.push(format!(
            "item replace entity {sel} weapon.mainhand with {it}"
        ));
        b.push(format!("scoreboard players set {sel} {trigger} 1"));
        b.push(format!("function {ns}:tick"));
        b.push(format!(
            "assert score {party} {} matches 1",
            obj_score(id.as_str())
        ));
        write("verb_interact_held", b);
    }

    // flag gate: without the flag the objective must NOT complete; with it, it
    // does. The dummy is pinned and the withheld flags actively cleared (see
    // `pin_dummy` / `packtest_preamble`): a sibling template that satisfies the
    // same gated objective (`verb_interact`) sets the flag on `@a` — every
    // dummy in the batch — so this test must establish "flag absent" itself,
    // on its own dummy, rather than assume a fresh player.
    if let Some((qid, o)) = first_flag_gated {
        let id = o.id().as_str();
        let (pin, sel) = pin_dummy("dw_flagtest");
        let driver = |b: &mut Vec<String>| match o {
            Objective::Collect { .. } => b.push(format!(
                "execute as {sel} run function {ns}:c_reward_{}",
                plan::safe_local(id)
            )),
            Objective::Interact { .. } => {
                b.push(format!(
                    "scoreboard players set {sel} {} 1",
                    plan::interact_trigger(id)
                ));
                b.push(format!("function {ns}:tick"));
            }
            _ => {}
        };
        let mut b = packtest_header(&format!(
            "{}: requires_flags gates objective `{id}`",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        b.push(pin.clone());
        let party = plan::PARTY;
        b.push(format!(
            "scoreboard players set {party} {} 0",
            obj_score(id)
        ));
        b.extend(packtest_preamble(plan, qid, o, false, &sel)); // flags withheld (cleared)
        driver(&mut b);
        b.push(format!("assert score {party} {} matches 0", obj_score(id)));
        for f in o.requires_flags() {
            b.push(format!(
                "scoreboard players set {party} {} 1",
                plan::flag_score(f.as_str())
            ));
        }
        driver(&mut b);
        b.push(format!("assert score {party} {} matches 1", obj_score(id)));
        write("verb_flag_gate", b);
    }

    // forbid gate (v0.6 negative gate): with a forbidden flag SET the objective
    // must NOT complete; with it cleared, it does. The mirror image of
    // `verb_flag_gate`, phases reversed (suppress first, then release) so both
    // truth-table rows of the negative gate are exercised on one dummy.
    if let Some((qid, o)) = first_forbid_gated {
        let id = o.id().as_str();
        let (pin, sel) = pin_dummy("dw_fbdtest");
        let driver = |b: &mut Vec<String>| match o {
            Objective::Collect { .. } => b.push(format!(
                "execute as {sel} run function {ns}:c_reward_{}",
                plan::safe_local(id)
            )),
            Objective::Interact { .. } => {
                b.push(format!(
                    "scoreboard players set {sel} {} 1",
                    plan::interact_trigger(id)
                ));
                b.push(format!("function {ns}:tick"));
            }
            _ => {}
        };
        let mut b = packtest_header(&format!(
            "{}: forbids_flags suppresses objective `{id}`",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        b.push(pin.clone());
        let party = plan::PARTY;
        b.push(format!(
            "scoreboard players set {party} {} 0",
            obj_score(id)
        ));
        // Preamble satisfies quest/after/requires and CLEARS forbids; then set
        // the forbidden flags to prove suppression.
        b.extend(packtest_preamble(plan, qid, o, true, &sel));
        for f in o.forbids_flags() {
            b.push(format!(
                "scoreboard players set {party} {} 1",
                plan::flag_score(f.as_str())
            ));
        }
        driver(&mut b);
        b.push(format!("assert score {party} {} matches 0", obj_score(id)));
        for f in o.forbids_flags() {
            b.push(format!(
                "scoreboard players set {party} {} 0",
                plan::flag_score(f.as_str())
            ));
        }
        driver(&mut b);
        b.push(format!("assert score {party} {} matches 1", obj_score(id)));
        write("verb_forbid_gate", b);
    }

    // gap 9: every NPC body actually summoned. The bot drives talk-to via a
    // `/trigger` chat command, so a failed summon (e.g. an invalid `base_entity`)
    // would still pass the ladder with no NPC in the world — a false green. This
    // asserts each NPC's body resolves to EXACTLY one entity. It summons
    // deterministically, independent of the async placement/tick loop: disarm the
    // tick placer (`#placed`) and clear any body/hitbox a prior boot or test left
    // at the same absolute coords, then run `setup_finish` once (it summons at the
    // chunks `setup` force-loads; no templates needed). v0.3-gated so v0.2
    // campaigns (hello-world has an NPC) keep byte-identical packtest output.
    if campaign_is_v03(plan) && !plan.npcs.is_empty() {
        let mut b = packtest_header(&format!(
            "{}: every NPC summon resolves to exactly one entity",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        b.push("scoreboard players set #placed dw.sys 1".to_string());
        for npc in &plan.npcs {
            b.push(format!("kill @e[tag={}]", npc.tag));
        }
        b.push(format!("function {ns}:setup_finish"));
        // A `deferred` NPC (DSL v0.6) is deliberately absent after `setup_finish` —
        // it enters via `spawn-npc`. Fire its entrance here, so this test proves the
        // deferred path summons exactly the same one body + one hitbox.
        for npc in &plan.npcs {
            if npc_is_deferred(c, &npc.npc_id) {
                b.push(format!("function {ns}:{}", spawn_npc_fn(&npc.npc_id)));
            }
        }
        for npc in &plan.npcs {
            // The NPC body carries BOTH `dw_npc` and its unique id tag; the separate
            // interaction hitbox carries only the id tag — so `dw_npc` + id tag
            // selects exactly the body. A failed body summon leaves zero.
            b.push(format!(
                "execute store result score #npc_{} dw.sys if entity @e[tag=dw_npc,tag={}]",
                npc.safe, npc.tag
            ));
            b.push(format!("assert score #npc_{} dw.sys matches 1", npc.safe));
        }
        write("npc_summons", b);
    }

    // gap 13: a collect item taken BEFORE the objective activates must still
    // complete it at activation, with no further inventory churn. Reproduces the
    // stall: pick the item up while the quest is inactive (arming and stranding the
    // re-arming `inventory_changed` advancement), THEN activate and tick once with
    // no further pickup — the per-tick held check must complete the objective.
    if let Some((qid, o)) = first_collect
        && let Objective::Collect {
            id, item, count, ..
        } = o
    {
        let (pin, sel) = pin_dummy("dw_t_cpre");
        let mut b = packtest_header(&format!(
            "{}: collect completes for an item held before activation",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        // Pin this test's own dummy (see `pin_dummy`) and drive/assert on it alone.
        b.push(pin);
        let party = plan::PARTY;
        b.push(format!(
            "scoreboard players set {party} {} 0",
            obj_score(id.as_str())
        ));
        // Take the item while the objective is INACTIVE (the pre-activation pickup).
        b.push(format!("give {sel} {item} {count}"));
        // Activate WITHOUT re-giving (packtest_preamble would re-give the item, which
        // would mask the bug by producing a fresh inventory_changed): set the quest
        // active + every `after` prerequisite + every required flag by hand.
        b.push(format!(
            "scoreboard players set {party} {} 1",
            quest_active_score(qid)
        ));
        for a in o.after() {
            b.push(format!(
                "scoreboard players set {party} {} 1",
                obj_score(a.as_str())
            ));
        }
        for f in o.requires_flags() {
            b.push(format!(
                "scoreboard players set {party} {} 1",
                plan::flag_score(f.as_str())
            ));
        }
        // One tick's held check completes it — no inventory_changed event occurs.
        b.push(format!("function {ns}:tick"));
        b.push(format!(
            "assert score {party} {} matches 1",
            obj_score(id.as_str())
        ));
        write("collect_preheld", b);
    }

    // v0.8 container adoption: the
    // objective fills the barrel the PREFAB placed, pads it so it reads full, and
    // still completes when what the player carries is the NAMED stack.
    //
    // Three things a compile-time test cannot reach, all of them silent failures
    // on a live server: `item replace block … container.<n>` against the adopted
    // cell has to actually land (it fails without output on a non-container —
    // `DW0438` proves one is there, not that the fill took); the padding has to
    // occupy the slots after it rather than overwrite slot 0; and the custom-name
    // component must not change what the adjudication sees, because the
    // completion advancement and the per-tick held check both match on ITEM ID
    // and a component that quietly excluded the stack would leave the objective
    // uncompletable with the item sitting in the player's hand.
    if let Some((qid, o)) = first_collect_adopted
        && let Objective::Collect {
            id,
            item,
            count,
            item_name,
            fill_count,
            ..
        } = o
        && let Some(fill) = plan
            .collect_fills
            .iter()
            .find(|f| f.objective_id == id.as_str())
    {
        let (pin, sel) = pin_dummy("dw_t_cadp");
        let cell = fill.cell;
        let party = plan::PARTY;
        let stack = format!(
            "{item}{} {count}",
            item_component_tail(item_name.as_deref())
        );
        let mut b = packtest_header(&format!(
            "{}: collect `{id}` fills the adopted container and completes on the named stack",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        b.push(pin);
        b.push(format!(
            "scoreboard players set {party} {} 0",
            obj_score(id.as_str())
        ));
        // Open the activation gate by hand and WITHOUT the preamble's `give`: the
        // point of this template is which stack completes the objective, and the
        // plain item handed over first would complete it before the named one is
        // ever presented.
        b.extend(packtest_guards(plan, qid, o, true));
        // Empty the adopted container first — `setup` may have run for a sibling
        // template, and this objective's own activation is guarded once per world
        // by `#act_<obj>`, so the fill is not re-run on a second call. Clearing
        // makes the count assertion below a statement about THIS activation.
        for slot in 0..=*fill_count {
            b.push(format!(
                "item replace block {} {} {} container.{slot} with minecraft:air",
                cell[0], cell[1], cell[2]
            ));
        }
        b.push(format!(
            "function {ns}:activate_{}",
            safe_obj_fn(id.as_str())
        ));
        // The fill landed, in the right number of slots: `if items block` counts
        // matching items across the whole container, so the total is the stack
        // repeated once per filled slot. A dropped fill reads 0; padding that
        // overwrote slot 0 instead of following it reads one stack short.
        let total = count * (fill_count + 1);
        b.push(format!(
            "execute store result score #cadp dw.sys if items block {} {} {} container.* {item}",
            cell[0], cell[1], cell[2]
        ));
        b.push(format!("assert score #cadp dw.sys matches {total}"));
        // The player takes the stack the container actually holds — components and
        // all, the same text the fill emitted — and the objective completes.
        b.push(format!(
            "item replace entity {sel} inventory.0 with {stack}"
        ));
        b.push(format!("function {ns}:tick"));
        b.push(format!(
            "assert score {party} {} matches 1",
            obj_score(id.as_str())
        ));
        write("collect_container", b);
    }
}

/// Shipped `view-distance`, in chunks — **10** = a 160-block render radius.
///
/// What it answers to, in the order the number was established:
///
/// * **The scenes.** Measured from the `forceload` AABBs the compiler emits for
///   the shipped campaigns, the largest delve built to date spans 114 × 165
///   blocks and the next 35 × 115. A 160-block radius therefore reaches the far
///   side of either from any standpoint inside it, and on an `ocean` horizon it
///   puts the fog line 160 blocks of open sea past the shore — already all
///   backdrop. Going up to 12 buys 32 more blocks of empty water or void on
///   every delve that exists; going down to 8 (128 blocks) would clip the long
///   axis of the largest scene from a standpoint at either end.
/// * **The existing record.** `docs/notes/horizon-library-dossier.md` §3–4 and
///   `docs/specs/spec-0026-horizon-library.md` §6 already do their vista
///   arithmetic against a shipped `view-distance` of 10 (→ 160 blocks), with 12
///   reserved as the summit horizon's floor. Writing the key makes that
///   arithmetic bind to a fact rather than to an assumption about the host.
/// * **Prod.** Perf is non-gating on the Raspberry Pi,
///   so the Pi does not push the number DOWN; it is the absence of any delve
///   content past 160 blocks that stops it going up.
///
/// It is also what both boot paths land on today, so pinning it changes no
/// player-visible behaviour — this is a determinism fix, not a retune.
pub const DELVE_VIEW_DISTANCE: u32 = 10;

/// Shipped `simulation-distance`, in chunks — **10**, and the same number as
/// [`DELVE_VIEW_DISTANCE`] for an unrelated reason. The two answer different
/// questions and are deliberately separate constants.
///
/// This value is **not** what makes a delve tick. `setup` force-loads every
/// placed piece and never releases it, so scene chunks are entity-ticking
/// wherever the party is standing; simulation distance governs only the chunks
/// around a player that are *not* scene — backdrop ocean or void, which is inert
/// by construction (`spawn-monsters=false` + the `spawn_mobs` seal, and traps are
/// command-driven, never redstone).
///
/// Its job is to make the ticking rim a **known radius**. With both distances
/// pinned, the set of chunks that can tick or be seen is bounded by the
/// force-loaded scene ∪ a Chebyshev radius of 10 (+1 for the loading margin)
/// chunks around any player — one number a whole-plane proof can be written
/// against. Unpinned, that set has no upper bound the compiler can state.
///
/// 10 is vanilla's own default and what every delve boots with today. Lowering it
/// below the view distance would be a live change to what the party experiences,
/// gated on the owner's playtest, for no measured gain; raising it would tick
/// backdrop nobody can see.
pub const DELVE_SIMULATION_DISTANCE: u32 = 10;

fn emit_server(plan: &Plan, out: &mut BuildOutput) {
    // Difficulty. Declared (`world.difficulty`, v0.6) wins; absent falls back to
    // the historical derivation, which is what keeps every pre-0.6 campaign
    // byte-identical: combat waves (v0.3) require a non-peaceful difficulty
    // because peaceful *removes* hostile mobs even when summoned, and wave-free
    // campaigns stay `peaceful` (hello-world / keep-crawl unchanged). Natural
    // spawning is off either way (`spawn-monsters=false` + gamerule `spawn_mobs
    // false`); only the compiler's own summons exist.
    let difficulty = declared_difficulty(plan.campaign)
        .map(|d| d.token())
        .unwrap_or_else(|| {
            if plan.campaign.quests.content.waves.is_empty() {
                "peaceful"
            } else {
                "easy"
            }
        });
    // Horizon (DSL v0.6, spec-0013). `void` (default/absent) keeps the empty-layer
    // superflat + `the_void` biome, byte-identical to v0.5. `ocean` swaps in a
    // pinned bedrock/stone/water superflat: from the -64 build floor, 1+118+8
    // layers top the water at y=62 (= sea level); areas are placed on that datum
    // (`plan::OCEAN_BASE_Y` = 60) so island pieces read as land ringed by the sea. No structures (generate-structures=false) or mobs (gamerule
    // spawn_mobs false); the sea is pure backdrop. The string is a fixed literal,
    // so both horizons stay deterministic (ADR-0006).
    let ocean = matches!(
        plan.campaign.world.content.horizon,
        Some(delvewright_dsl::Horizon::Ocean)
    );
    let generator_settings = if ocean {
        "{\"biome\":\"minecraft:ocean\",\"layers\":[{\"block\":\"minecraft:bedrock\",\"height\":1},{\"block\":\"minecraft:stone\",\"height\":118},{\"block\":\"minecraft:water\",\"height\":8}]}"
    } else {
        "{\"biome\":\"minecraft:the_void\",\"layers\":[]}"
    };
    // server.properties (keys sorted for determinism).
    //
    // Every key a delve's CONTENT depends on is written here, because an unwritten
    // key is decided by whichever host boots the build, and two hosts that decide
    // it differently are two different worlds (ADR-0006). The two boot paths a
    // delve actually has do not share a default source: the shipped image
    // (`validation/Dockerfile.delve`) starts from the itzg base's own
    // `/image/server.properties` template, while the owner's playtest server
    // (`tools/playtest-server.sh`, `OVERRIDE_SERVER_PROPERTIES=false`) copies THIS
    // file in and lets the vanilla jar fill in the rest. Where the two default
    // sources happen to agree it is a coincidence of an upstream file we do not
    // own, not an invariant — so a key that matters is pinned, never inherited.
    //
    // [`DELVE_VIEW_DISTANCE`] / [`DELVE_SIMULATION_DISTANCE`] carry the reasoning
    // for the two chunk-distance values; `validation/world-settings-entrypoint.sh`
    // derives both from this file, so the image cannot boot a different pair.
    let props: BTreeMap<&str, String> = BTreeMap::from([
        ("allow-nether", "false".to_string()),
        ("difficulty", difficulty.to_string()),
        ("force-gamemode", "true".to_string()),
        ("gamemode", "adventure".to_string()),
        ("generate-structures", "false".to_string()),
        ("generator-settings", generator_settings.to_string()),
        ("level-name", "world".to_string()),
        ("level-seed", plan.seed.to_string()),
        ("level-type", "minecraft:flat".to_string()),
        ("online-mode", "false".to_string()),
        ("pvp", "false".to_string()),
        ("simulation-distance", DELVE_SIMULATION_DISTANCE.to_string()),
        ("spawn-monsters", "false".to_string()),
        ("spawn-protection", "0".to_string()),
        ("view-distance", DELVE_VIEW_DISTANCE.to_string()),
    ]);
    let mut text = String::new();
    text.push_str(&format!(
        "# Generated by delvec for campaign {} (spec-0002 world strategy).\n",
        plan.namespace
    ));
    if ocean {
        text.push_str(
            "# Ocean superflat (spec-0013 backdrop) + fixed seed; created on first boot.\n",
        );
    } else {
        text.push_str("# Void/superflat + fixed seed; the world is created on first boot.\n");
    }
    for (k, v) in &props {
        text.push_str(&format!("{k}={v}\n"));
    }
    out.insert("server/server.properties".to_string(), text.into_bytes());

    out.insert(
        "server/eula-note.txt".to_string(),
        b"Accepting Mojang's EULA is the operator's action, never the compiler's.\n\
Set EULA=TRUE in the environment (or eula.txt) before running a server here.\n\
The server jar is NOT shipped (ADR-0010); it is fetched by version at run time.\n"
            .to_vec(),
    );

    let horizon_bullet = if ocean {
        "- `level-type=minecraft:flat` + a pinned bedrock/stone/water `generator-settings`\n\
  (sea level y=62, `minecraft:ocean` biome) ⇒ an island backdrop (spec-0013).\n"
    } else {
        "- `level-type=minecraft:flat` + `generator-settings` with an empty layer list and\n\
  the `minecraft:the_void` biome ⇒ a void world.\n"
    };
    out.insert(
        "server/README.md".to_string(),
        format!(
            "# server/\n\n\
Level config for campaign `{}`. The world is generated on first server boot\n\
from `server.properties` (no region files shipped, spec-0002):\n\n\
{}- `level-seed={}` pins world generation (ADR-0006); v0 uses no other randomness.\n\
- `gamemode=adventure`, `difficulty=peaceful`, no structures/monsters.\n\
- `view-distance={}` / `simulation-distance={}` (chunks) are pinned here rather\n\
  than left to the host: the delve renders and ticks the same everywhere.\n\n\
The compiler-emitted `#minecraft:load` bootstrap (`datapack/`) places each area's\n\
prefab with `/place template` and summons NPCs; nothing is baked into region\n\
bytes, so byte-identity (ADR-0006) covers the whole `<out>/` tree.\n",
            plan.namespace,
            horizon_bullet,
            plan.seed,
            DELVE_VIEW_DISTANCE,
            DELVE_SIMULATION_DISTANCE
        )
        .into_bytes(),
    );
}

/// Splice a **`rest`** step into the exported critical path after the beat that
/// arms each bonfire (spec-0016 §1; bell round-3 finding, 2026-08-03).
///
/// A bonfire arms an affordance and moves nothing until the party rests — which is
/// souls-correct and was also invisible to the validation ladder: the proven path
/// walked past every bonfire without touching it, so the checkpoint never moved,
/// and a die-retry trial respawned the bot at world spawn (the beach) instead of
/// at the fire it had just walked past. The walk-back budget blew, and the run
/// judged the *campaign* for a *proof* that never performed the player loop.
///
/// Resting is the intended loop, so the proven path performs it: after the step
/// that arms bonfire `i` (its `fire_step` — the earliest tick at which a rest is
/// possible, the same index `DW0315` roots the no-stranding proof at), the path
/// gains one `rest` step choosing **rest and save**. Several bonfires armed by the
/// same beat are spliced in bonfire order, so the emission is deterministic.
///
/// This is a *path export* change only: `plan.critical_path` is untouched, so
/// every `fire_step` index, every nav proof and every other consumer sees exactly
/// what it saw before. Emits nothing for a campaign with no bonfire →
/// byte-identical.
///
/// **Step shape** (the harness contract; execution lands in a follow-up):
/// ```json
/// { "action": "rest", "bonfire": 0, "anchor": "anchor/keeper-stand",
///   "pos": [44, 65, 2], "command": "/trigger dw.rest set 2" }
/// ```
/// The bot walks to `pos`, right-clicks the `dw_bonfire_<bonfire>` interaction —
/// which is what opens the dialog and what *enables* the trigger — and then sends
/// `command`, the exact chat line the "rest and save" button runs. The click is not
/// optional: `dw.rest` is a trigger objective and is disabled until the opener
/// enables it, so a bot that only chats the command changes nothing.
///
/// `walked` is the step list the JSON was serialized from — the exported path, or
/// (spec-0025) one branch's path, whose indices are its own. See
/// [`rest_step_index`] for how a `fire_step` crosses that boundary.
fn with_bonfire_rest_steps(plan: &Plan, walked: &[plan::Step], steps: Vec<Value>) -> Vec<Value> {
    if plan.bonfires().next().is_none() {
        return steps;
    }
    let mut out: Vec<Value> = Vec::with_capacity(steps.len());
    for (i, step) in steps.into_iter().enumerate() {
        out.push(step);
        for bf in plan
            .bonfires()
            .filter(|b| rest_step_index(plan, walked, b.fire_step) == Some(i))
        {
            out.push(json!({
                "action": "rest",
                "bonfire": bf.index,
                "anchor": bf.anchor,
                "pos": bf.pos,
                "command": "/trigger dw.rest set 2"
            }));
        }
    }
    out
}

/// Where bonfire `fire_step` — an index into the EXPORTED path — lands on `walked`.
///
/// A per-branch path (spec-0025) is a different sequence of the same steps, so the
/// index cannot be carried across: it is translated through the **objective** the
/// firing beat names, because a fire is armed by a beat, not by a position. A beat
/// that does not happen on this branch arms nothing there, and the branch path
/// carries no rest step for it. On the exported path the translation is the
/// identity (an objective appears at exactly one step), so this emits byte-for-byte
/// what it emitted before.
fn rest_step_index(plan: &Plan, walked: &[plan::Step], fire_step: usize) -> Option<usize> {
    match plan
        .critical_path
        .get(fire_step)
        .and_then(plan::Step::objective)
    {
        // `fire_step: 0` is the class-select / conservative "before everything"
        // index (see `Plan::gate_fired_before`); it precedes every path the same way.
        None => (fire_step < walked.len()).then_some(fire_step),
        Some(obj) => walked.iter().position(|s| s.objective() == Some(obj)),
    }
}

/// One executable critical path per REACHABLE enumerated branch (spec-0025 §3),
/// as `(slug, json)` in branch-enumeration order.
///
/// Empty — so byte-identical — for a campaign that declares no `branch_points`.
/// An unreachable branch contributes nothing: there is no world that plays it,
/// which `DW0482` has already failed the build for; `branch-plan.json` still names
/// it, so the harness reports it skipped rather than silently absent.
/// One branch-only inter-area crossing: where to set the party down, and the
/// flag assignment that identifies the branch it belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BranchTransport {
    /// Flags pinned SET on the branch this crossing belongs to.
    pub set: BTreeSet<String>,
    /// Flags pinned UNSET on that branch.
    pub unset: BTreeSet<String>,
    /// The branch's slug — the row's deterministic sort key, and the name of the
    /// `branch-path-<slug>.json` this crossing is the datapack half of.
    pub slug: String,
    /// The destination area's `spawn` anchor, in world coordinates.
    pub pos: [i32; 3],
}

/// Objective id → the branch-only crossings completing it must perform.
pub type BranchTransportOverlay = BTreeMap<String, Vec<BranchTransport>>;

/// The crossings that exist on a BRANCH but not on the exported path.
///
/// [`crate::plan::build_critical_path`] derives an inter-area transport map for
/// whatever playthrough it is handed, so every branch's map already exists via
/// [`Plan::branch_critical_path`] — and `branch-path-<slug>.json` publishes it to
/// the harness. Emission, however, reads only `plan.transport`, the **exported**
/// path's map. A campaign whose branch alone leaves the starting area therefore
/// promised the harness a crossing the datapack never performed, and the branch
/// run stranded where the teleport should have been (island round 21).
///
/// This is that difference, ready to be emitted as flag-gated teleports beside
/// the unconditional one. A crossing the exported path already performs is
/// omitted: it is emitted unconditionally, which is stronger. A crossing whose
/// destination *contradicts* the exported path's is [`DW_BRANCH_TRANSPORT_DIVERGES`]
/// — one objective cannot land the party in two areas.
///
/// Empty — so byte-identical to the pre-task emission — for a campaign with no
/// `branch_points`, and for one whose branches cross only where the exported path
/// already does. Deterministic: `BTreeMap` keys, rows sorted by branch slug
/// (ADR-0006).
pub fn branch_transport_overlay(plan: &Plan) -> Result<BranchTransportOverlay, BuildFailure> {
    let mut out: BranchTransportOverlay = BTreeMap::new();
    let branches = crate::branch::realize(plan.campaign);
    if branches.is_empty() {
        return Ok(out);
    }
    let flow = crate::flow::Flow::new(plan.campaign);
    for r in &branches {
        // An unreachable branch has no world to walk — `DW0482` has already
        // failed the build for it (same skip as `branch_paths`).
        let Some(w) = r.world else { continue };
        let cp = plan
            .branch_critical_path(&flow, &flow.playthrough_in(w))
            .map_err(|e| BuildFailure::Diagnostic {
                code: e.code,
                message: format!("branch `{}`: {}", r.branch.id, e.message),
            })?;
        for (oid, pos) in &cp.transport {
            match plan.transport.get(oid) {
                // Already emitted unconditionally by the exported path.
                Some(d) if d == pos => continue,
                Some(d) => {
                    return Err(BuildFailure::Diagnostic {
                        code: DW_BRANCH_TRANSPORT_DIVERGES,
                        message: format!(
                            "objective `{oid}` crosses to {d:?} on the exported path but to \
                             {pos:?} on branch `{}`; completing it can only put the party in \
                             one place — split the crossing into one objective per branch, \
                             each gated by that branch's flags",
                            r.branch.id
                        ),
                    });
                }
                None => {}
            }
            out.entry(oid.clone()).or_default().push(BranchTransport {
                set: r.branch.set.clone(),
                unset: r.branch.unset.clone(),
                slug: r.branch.slug.clone(),
                pos: *pos,
            });
        }
    }
    for rows in out.values_mut() {
        rows.sort_by(|a, b| a.slug.cmp(&b.slug));
    }
    Ok(out)
}

fn branch_paths(
    plan: &Plan,
    moves: &[crate::nav::MovePlan],
    actor_moves: &[crate::nav::ActorMovePlan],
) -> Result<Vec<(String, Value)>, BuildFailure> {
    let branches = crate::branch::realize(plan.campaign);
    if branches.is_empty() {
        return Ok(Vec::new());
    }
    let flow = crate::flow::Flow::new(plan.campaign);
    let mut out = Vec::new();
    for r in &branches {
        let Some(w) = r.world else { continue };
        let cp = plan
            .branch_critical_path(&flow, &flow.playthrough_in(w))
            .map_err(|e| BuildFailure::Diagnostic {
                code: e.code,
                message: format!("branch `{}`: {}", r.branch.id, e.message),
            })?;
        out.push((
            r.branch.slug.clone(),
            critical_path_json(
                plan,
                &cp.steps,
                &cp.transport_by_step,
                &cp.sneak_by_step,
                &cp.cutscene_by_step,
                moves,
                actor_moves,
            ),
        ));
    }
    Ok(out)
}

fn emit_critical_path(
    plan: &Plan,
    moves: &[crate::nav::MovePlan],
    actor_moves: &[crate::nav::ActorMovePlan],
) -> Value {
    critical_path_json(
        plan,
        &plan.critical_path,
        &plan.critical_path_transport,
        &plan.critical_path_sneak,
        &plan.critical_path_cutscene,
        moves,
        actor_moves,
    )
}

/// Serialize a step list in the `critical-path.json` contract (format 2).
///
/// One serializer for the exported path and for every spec-0025 per-branch path:
/// a branch run must consume a contract the harness already parses, so the branch
/// tier cannot drift into a second, less-tested shape.
fn critical_path_json(
    plan: &Plan,
    walked: &[plan::Step],
    transports: &[Option<[i32; 3]>],
    sneak: &[bool],
    cutscene: &[Option<u32>],
    moves: &[crate::nav::MovePlan],
    actor_moves: &[crate::nav::ActorMovePlan],
) -> Value {
    // Scheduled-ending tail for THIS path's quests: exported on the
    // terminal `assert-complete` step as `ending_tail_ticks`, so the harness
    // completion window covers a `sequence`-scheduled finale (the-wake: 250t)
    // exactly as it already covers `cutscene_seconds`. Omitted when 0, keeping
    // every synchronous-ending path byte-identical.
    let path_quests: BTreeSet<&str> = {
        let objs: BTreeSet<&str> = walked.iter().filter_map(plan::Step::objective).collect();
        plan.campaign
            .quests
            .content
            .quests
            .iter()
            .filter(|q| q.objectives.iter().any(|o| objs.contains(o.id().as_str())))
            .map(|q| q.id.as_str())
            .collect()
    };
    let ending_tail = quests_ending_tail(plan.campaign, &path_quests, moves, actor_moves);
    let steps: Vec<Value> = walked
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let transport = &transports[i];
            let mut step = match s {
                Step::SelectClass { class_id, command } => json!({
                    "action": "select-class", "class": class_id, "command": command
                }),
                Step::TalkTo { objective_id, npc_id, pos, command } => json!({
                    "action": "talk-to", "objective": objective_id, "npc": npc_id,
                    "pos": pos, "command": command
                }),
                Step::Reach { objective_id, anchor_id, pos, radius } => json!({
                    "action": "reach", "objective": objective_id, "anchor": anchor_id,
                    "pos": pos, "radius": radius
                }),
                Step::Kill { objective_id, wave_id, pos, tag, count } => json!({
                    "action": "kill", "objective": objective_id, "wave": wave_id,
                    "pos": pos, "tag": tag, "count": count
                }),
                Step::Collect { objective_id, item, count, pos, dropped } => {
                    let mut v = json!({
                        "action": "collect", "objective": objective_id, "item": item,
                        "count": count, "pos": pos
                    });
                    // v0.9: present only on a drop-gated collect, so every
                    // pre-0.9 campaign's `critical-path.json` is byte-identical.
                    if let Some(w) = dropped
                        && let Some(obj) = v.as_object_mut()
                    {
                        obj.insert("dropped_by".to_string(), json!(w));
                    }
                    v
                }
                Step::Interact { objective_id, anchor_id, pos, command, requires_item } => json!({
                    "action": "interact", "objective": objective_id, "anchor": anchor_id,
                    "pos": pos, "command": command, "requires_item": requires_item
                }),
                Step::AssertComplete { objective, value } => {
                    let mut v = json!({
                        "action": "assert-complete", "scoreboard": { "objective": objective, "value": value }
                    });
                    if ending_tail > 0
                        && let Some(obj) = v.as_object_mut()
                    {
                        obj.insert("ending_tail_ticks".to_string(), json!(ending_tail));
                    }
                    v
                }
            };
            // gap 8: mark a step whose completion teleports the player to another
            // area with the absolute destination, so the harness waits for the
            // position discontinuity before starting the next step.
            if let (Some(pos), Some(obj)) = (transport, step.as_object_mut()) {
                obj.insert("transport".to_string(), json!(pos));
            }
            // DSL v0.4 harness hints. `sneak` is emitted ONLY when true (absent =
            // false, per the harness contract). `cutscene_seconds` is a positive
            // integer on the step whose completion triggers the cutscene.
            if sneak[i]
                && let Some(obj) = step.as_object_mut()
            {
                obj.insert("sneak".to_string(), json!(true));
            }
            if let Some(secs) = cutscene[i]
                && secs > 0
                && let Some(obj) = step.as_object_mut()
            {
                obj.insert("cutscene_seconds".to_string(), json!(secs));
            }
            step
        })
        .collect();
    let steps = with_bonfire_rest_steps(plan, walked, steps);
    json!({
        // Campaign-derived (not the compiler's max supported version): a v0.2
        // campaign emits a v0.2 critical path, a v0.3 campaign a v0.3 one.
        "version": plan.campaign.world.dsl_version,
        // The bot-contract version, independent of the DSL version: `2` = every
        // objective-bearing step names the objective it proves, and completion is
        // proved by the anchored marker channel. The harness refuses anything else.
        "format_version": plan::CRITICAL_PATH_FORMAT_VERSION,
        "campaign_id": plan.namespace,
        "steps": steps
    })
}

fn emit_manifest(
    plan: &Plan,
    input_bytes: &BTreeMap<String, Vec<u8>>,
    out: &BuildOutput,
    language: Option<&str>,
    content_sha: &str,
    resource_pack_sha1: Option<&str>,
) -> Value {
    let inputs: BTreeMap<String, String> = input_bytes
        .iter()
        .map(|(k, v)| (k.clone(), sha256_hex(v)))
        .collect();
    let outputs: BTreeMap<String, String> = out
        .iter()
        .map(|(k, v)| (k.clone(), sha256_hex(v)))
        .collect();
    let mut manifest = json!({
        "campaign_id": plan.namespace,
        "delvec_version": DELVEC_VERSION,
        "dsl_version": plan.campaign.world.dsl_version,
        "mc_version": MC_VERSION,
        // The pinned content-repo SHA (spec-0007 Step 0), read from versions.toml
        // `[content].sha` at build time (NOT git state) so the build stays
        // deterministic + offline; "unpinned" when versions.toml is absent. This
        // closes the ADR-0006 reproducibility loop: same DSL + same seed + same
        // content_sha -> byte-identical output.
        "content_sha": content_sha,
        "inputs": inputs,
        "outputs": outputs
    });
    // Record the build language ONLY for a non-canonical build. English is the
    // implicit canonical language, so an `en` build's manifest is byte-identical to
    // a pre-i18n one (preserving the determinism regression for all campaigns that
    // do not localize).
    if let Some(lang) = language
        && lang != delvewright_dsl::CANONICAL_LANG
    {
        manifest
            .as_object_mut()
            .expect("manifest is a JSON object")
            .insert("language".to_string(), Value::String(lang.to_string()));
    }
    // Record the NPC-skin resource-pack SHA-1 (spec-0009: the pack bytes — and so
    // this hash — are part of the byte-identity contract). Absent for a campaign
    // with no skinned NPCs, keeping such builds byte-identical.
    if let Some(sha1) = resource_pack_sha1 {
        manifest
            .as_object_mut()
            .expect("manifest is a JSON object")
            .insert(
                "resource_pack_sha1".to_string(),
                Value::String(sha1.to_string()),
            );
    }
    manifest
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// Join lines with `\n` and a trailing newline.
fn lines(v: &[String]) -> String {
    let mut s = v.join("\n");
    s.push('\n');
    s
}

/// Serialize a JSON value canonically (sorted keys via serde_json default map,
/// 2-space pretty, trailing newline) into `out` at `path`.
fn put_json(out: &mut BuildOutput, path: &str, value: &Value) {
    let mut bytes = serde_json::to_vec_pretty(value).expect("json serializes");
    bytes.push(b'\n');
    out.insert(path.to_string(), bytes);
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut s = String::with_capacity(64);
    for b in digest {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

// ---------------------------------------------------------------------------
// Unit tests (helpers)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// **The emitter's audience per effect root equals the DSL's own answer**,
    /// over the closed root set, in both directions.
    ///
    /// `EffectRootKind::runs_with_acting_player` is what `DW0503` trusts when it
    /// decides whether a `player`-scoped runtime datum (spec-0031) may be read or
    /// written inside a root's bundle. If a root's emitted audience ever moved
    /// without that answer moving with it, a validated campaign would emit `@s`
    /// into a function with no command source — a silent runtime failure with
    /// every check green. This is the bind. An eighth root fails it until both
    /// sides name it.
    #[test]
    fn root_audience_matches_the_dsl() {
        for kind in delvewright_dsl::EffectRootKind::ALL {
            assert_eq!(
                root_audience(kind).has_actor(),
                kind.runs_with_acting_player(),
                "root `{}`: the emitter and `EffectRootKind::runs_with_acting_player` disagree \
                 about whether its bundle has an acting player",
                kind.label()
            );
        }
        // Binding: the loop must have examined every root, and both answers must
        // actually occur — an assertion that only ever saw `true` would pass on a
        // constant.
        let all = delvewright_dsl::EffectRootKind::ALL;
        assert_eq!(all.len(), delvewright_dsl::EffectRootKind::COUNT);
        assert!(all.iter().any(|k| k.runs_with_acting_player()));
        assert!(all.iter().any(|k| !k.runs_with_acting_player()));
    }

    #[test]
    fn facing_yaw_matches_mc_convention() {
        // MC yaw: south=0, west=90, north=180, east=270.
        assert_eq!(facing_yaw(Some("south")), 0);
        assert_eq!(facing_yaw(Some("west")), 90);
        assert_eq!(facing_yaw(Some("north")), 180);
        assert_eq!(facing_yaw(Some("east")), 270);
        assert_eq!(facing_yaw(None), 0);
    }

    #[test]
    fn snbt_string_is_a_plain_quoted_component() {
        // A bare quoted SNBT string is a valid text component (renders literally),
        // unlike the old `'{"text":…}'` JSON-string form.
        assert_eq!(
            snbt_string("Hedric of the Watch"),
            "\"Hedric of the Watch\""
        );
        // Backslash and double-quote are escaped.
        assert_eq!(snbt_string("a\"b\\c"), "\"a\\\"b\\\\c\"");
    }

    #[test]
    fn marker_name_fields_never_leak_a_raw_id() {
        // A titled marker carries its title (byte-identical to the old behavior).
        assert_eq!(
            marker_name_fields(Some("Unbar the Inner Door")),
            "CustomName:\"Unbar the Inner Door\",CustomNameVisible:1b,"
        );
        // An untitled objective yields NO name fields — the marker still glows but
        // never surfaces its raw objective id (e.g. `obj/door`) to players.
        assert_eq!(marker_name_fields(None), "");
    }

    #[test]
    fn default_equipment_arms_only_naturally_armed_mobs() {
        // wither_skeleton → stone sword via the component-era `equipment` NBT
        // with a zero `drop_chances` (1.21.11 ignores legacy `HandItems`).
        let ws = default_equipment("minecraft:wither_skeleton").unwrap();
        assert!(ws.contains("equipment:{mainhand:{id:\"minecraft:stone_sword\",count:1}}"));
        assert!(ws.contains("drop_chances:{mainhand:0.0f}"));
        // No trace of the legacy, silently-ignored form.
        assert!(!ws.contains("HandItems"));
        assert!(!ws.contains("HandDropChances"));
        // skeleton/stray → bow.
        assert!(
            default_equipment("skeleton")
                .unwrap()
                .contains("minecraft:bow")
        );
        assert!(
            default_equipment("minecraft:stray")
                .unwrap()
                .contains("minecraft:bow")
        );
        // zombie stays unarmed; drowned's trident is not a default.
        assert!(default_equipment("minecraft:zombie").is_none());
        assert!(default_equipment("minecraft:drowned").is_none());
    }

    // --- DSL v0.6 actor emission (spec-0014) ---

    fn mk_actor(id: &str, entity: &str, vulnerable: bool) -> delvewright_dsl::Actor {
        delvewright_dsl::Actor {
            id: delvewright_dsl::ActorId(id.to_string()),
            entity: entity.to_string(),
            name: Some("Boss".to_string()),
            skin: None,
            anchor: delvewright_dsl::AnchorId("anchor/stage".to_string()),
            facing: Some(delvewright_dsl::Facing::West),
            vulnerable,
            equipment: None,
            drops: Vec::new(),
            attributes: None,
            tier: None,
            traversal: None,
        }
    }

    #[test]
    fn puppet_summon_is_noai_no_loot_and_tagged() {
        let a = mk_actor("actor/giant", "minecraft:warden", false);
        let s = actor_puppet_summon("dw", &a, [10, 65, 20], facing_yaw(Some("west")));
        assert!(
            s.starts_with("summon minecraft:warden 10.5 65.0 20.5 "),
            "puppet stands at the CENTRE of its cell, not the four-column corner: {s}"
        );
        assert!(s.contains("NoAI:1b") && s.contains("Silent:1b") && s.contains("NoGravity:1b"));
        assert!(s.contains("Invulnerable:1b"));
        assert!(s.contains("DeathLootTable:\"minecraft:empty\""));
        assert!(s.contains("dw_actor_giant") && s.contains("dw_pup_giant"));
        assert!(s.contains("Rotation:[90f,0f]"));
        assert!(
            !s.contains("knockback_resistance"),
            "invulnerable puppet has no kb attr"
        );
    }

    #[test]
    fn vulnerable_puppet_is_damageable_but_knockback_immune() {
        let a = mk_actor("actor/creep", "minecraft:zombie", true);
        let s = actor_puppet_summon("dw", &a, [0, 64, 0], 0);
        assert!(
            s.contains("Invulnerable:0b"),
            "vulnerable puppet takes damage"
        );
        assert!(
            s.contains("knockback_resistance") && s.contains("base:1.0"),
            "vulnerable puppet stays knockback-immune: {s}"
        );
    }

    #[test]
    fn skinned_puppet_is_a_mannequin() {
        let mut a = mk_actor("actor/keeper", "minecraft:warden", false);
        a.skin = Some(delvewright_dsl::NpcSkin {
            texture_id: "giant-idle".to_string(),
            model: delvewright_dsl::SkinModel::Wide,
        });
        let s = actor_puppet_summon("dw", &a, [1, 2, 3], 180);
        assert!(
            s.starts_with("summon minecraft:mannequin 1.5 2.0 3.5 "),
            "mannequin stands at the centre of its cell: {s}"
        );
        assert!(s.contains("profile:{texture:\"delvewright:npc/giant-idle\",model:\"wide\"}"));
        assert!(s.contains("dw_pup_keeper"));
    }

    #[test]
    fn twin_summon_has_ai_and_no_puppet_marker() {
        let a = mk_actor("actor/giant", "minecraft:warden", false);
        let s = actor_twin_summon("dw", &a, "~ ~ ~");
        assert!(s.starts_with("summon minecraft:warden ~ ~ ~ "));
        assert!(!s.contains("NoAI"), "the twin has real AI");
        assert!(s.contains("dw_actor_giant") && !s.contains("dw_pup"));
        assert!(s.contains("PersistenceRequired:1b"));
    }

    /// v0.9: a `despawn-actor` on a drop-declaring actor strips the
    /// declaration off the body before killing it. `/kill` is an ordinary death
    /// and a preserved slot survives a non-player kill, so without this a souls
    /// re-seat would shower the party with the elite's own axe every rest.
    #[test]
    fn despawn_strips_declared_drops_first() {
        let mut cmds = Vec::new();
        emit_despawn_actor(
            "actor/giant",
            delvewright_dsl::DespawnStyle::Kill,
            true,
            &mut cmds,
        );
        assert_eq!(cmds.len(), 2, "strip then kill: {cmds:?}");
        assert!(
            cmds[0].starts_with("execute as @e[tag=dw_actor_giant] run data merge entity @s ")
                && cmds[0].contains("mainhand:0.0f")
                && cmds[0].contains("feet:0.0f")
                && cmds[0].contains("DeathLootTable:\"minecraft:empty\""),
            "{cmds:?}"
        );
        assert_eq!(cmds[1], "kill @e[tag=dw_actor_giant]");
    }

    #[test]
    fn despawn_styles_differ() {
        let mut kill = Vec::new();
        emit_despawn_actor(
            "actor/giant",
            delvewright_dsl::DespawnStyle::Kill,
            false,
            &mut kill,
        );
        assert_eq!(kill, vec!["kill @e[tag=dw_actor_giant]".to_string()]);
        let mut vanish = Vec::new();
        emit_despawn_actor(
            "actor/giant",
            delvewright_dsl::DespawnStyle::Vanish,
            false,
            &mut vanish,
        );
        // The drop is relative to each ACTOR, not to the command source — see
        // `emit_despawn_actor` for the live-observed failure the bare `tp` caused.
        assert_eq!(
            vanish,
            vec![
                "execute as @e[tag=dw_actor_giant] at @s run tp @s ~ -128 ~".to_string(),
                "kill @e[tag=dw_actor_giant]".to_string(),
            ]
        );
    }

    /// **Despawn-if-exists**: every actor-lifecycle verb has to survive the body
    /// simply not being there any more.
    ///
    /// This is not hypothetical. An `unleash`ed warden is a *real* vanilla warden,
    /// and vanilla wardens remove themselves — the ancient-city dig-down burrows
    /// the mob out of the world on its own schedule. So by the time a later beat
    /// fires `despawn-actor` (and hands off to the NPC), the entity the story
    /// thinks it is dismissing may already be gone. The staging must be a no-op
    /// then, never a hard error that takes the rest of the bundle's function down
    /// with it — and there is no dangling tag to clean up, because tags live on
    /// the entity and a removed entity takes its tags with it.
    ///
    /// The property is structural: the body is always addressed through a **plain
    /// multi-entity tag selector** — `@e[tag=dw_actor_<id>]` with no `limit=1` —
    /// so a zero-match run affects nothing and the function continues. A
    /// single-entity-arity form here would be exactly the 1.21.11 load-failure
    /// class the command-tree check guards elsewhere.
    ///
    /// A bare `@s` is equally forbidden (nothing binds it in a scheduled bundle),
    /// but an `@s` **bound by an enclosing `execute as`** in the same command is
    /// fine and is what `vanish` uses: `execute as @e[tag=…] at @s run tp @s …`
    /// runs its body zero times when nothing matches, and the `at @s` is required
    /// for the relative `~ -128 ~` to resolve at the body rather than at the
    /// command source. So the check is "every `@s` is bound", not "no `@s`" —
    /// the latter would reject a strictly more correct emission.
    #[test]
    fn actor_lifecycle_verbs_are_no_ops_when_the_body_is_already_gone() {
        for style in [
            delvewright_dsl::DespawnStyle::Kill,
            delvewright_dsl::DespawnStyle::Vanish,
        ] {
            let mut cmds = Vec::new();
            emit_despawn_actor("actor/giant", style, false, &mut cmds);
            assert!(!cmds.is_empty());
            for c in &cmds {
                assert!(
                    c.contains("@e[tag=dw_actor_giant]"),
                    "targets the body tag, so no match = no effect: {c}"
                );
                assert!(
                    !c.contains("limit=1"),
                    "no single-entity arity: a zero-match run must not fail the function: {c}"
                );
                if c.contains("@s") {
                    assert!(
                        c.contains("execute as @e[tag=dw_actor_giant]"),
                        "every `@s` must be bound by an enclosing `execute as`: {c}"
                    );
                }
            }
        }
        // The dual: re-staging after the body removed itself must work. The re-cage
        // summon is `execute unless entity` guarded, so it fires exactly when the
        // body is absent and no-ops when it is not.
        let a = mk_actor("actor/giant", "minecraft:warden", false);
        let spawn = format!(
            "execute unless entity @e[tag=dw_actor_giant] run {}",
            actor_puppet_summon("dw", &a, [0, 64, 0], 0)
        );
        assert!(
            spawn.starts_with("execute unless entity @e[tag=dw_actor_giant] run summon "),
            "re-cage is idempotent and works from nothing: {spawn}"
        );
    }

    #[test]
    fn sequence_key_is_deterministic_and_content_addressed() {
        let step = |t: u32| delvewright_dsl::SequenceStep {
            at_ticks: t,
            effects: vec![delvewright_dsl::QuestEffect::UnleashActor {
                actor: delvewright_dsl::ActorId("actor/giant".to_string()),
                happening: None,
            }],
        };
        let a = vec![step(0), step(40)];
        let b = vec![step(0), step(40)];
        let c = vec![step(0), step(41)];
        assert_eq!(
            sequence_key(&a),
            sequence_key(&b),
            "same content → same key"
        );
        assert_ne!(
            sequence_key(&a),
            sequence_key(&c),
            "different content → different key"
        );
        assert_eq!(sequence_fn(&a), format!("seq_{}", sequence_key(&a)));
    }
}

#[cfg(test)]
mod loot_emit_tests {
    use super::*;
    use crate::plan::{LootItemPlan, LootPlan};

    fn item(item: &str, count: u32, name: Option<&str>, ench: &[(&str, u32)]) -> LootItemPlan {
        LootItemPlan {
            item: item.to_string(),
            count,
            name: name.map(str::to_string),
            enchantments: ench.iter().map(|(k, v)| ((*k).to_string(), *v)).collect(),
        }
    }

    fn plan_of(items: Vec<LootItemPlan>) -> Vec<LootPlan> {
        vec![LootPlan {
            id: "loot/stores".to_string(),
            anchor: "anchor/stores".to_string(),
            cell: [10, 64, -3],
            items,
        }]
    }

    /// Slots are positional and deterministic: nth declared stack -> container.n.
    #[test]
    fn slots_are_assigned_positionally() {
        let out = loot_setup(&plan_of(vec![
            item("minecraft:cooked_cod", 3, None, &[]),
            item("minecraft:torch", 16, None, &[]),
        ]));
        assert_eq!(
            out,
            vec![
                "item replace block 10 64 -3 container.0 with minecraft:cooked_cod 3",
                "item replace block 10 64 -3 container.1 with minecraft:torch 16",
            ]
        );
    }

    #[test]
    fn a_named_stack_carries_the_custom_name_component() {
        let out = loot_setup(&plan_of(vec![item(
            "minecraft:paper",
            1,
            Some("Tide Ledger"),
            &[],
        )]));
        assert!(
            out[0].contains(r#"custom_name={"italic":false,"text":"Tide Ledger"}"#),
            "{}",
            out[0]
        );
    }

    #[test]
    fn enchantments_emit_as_the_1_21_component_map() {
        let out = loot_setup(&plan_of(vec![item(
            "minecraft:iron_sword",
            1,
            None,
            &[("minecraft:sharpness", 3), ("minecraft:knockback", 1)],
        )]));
        // BTreeMap order, never hash order (ADR-0006).
        assert!(
            out[0].contains(r#"enchantments={"minecraft:knockback":1,"minecraft:sharpness":3}"#),
            "{}",
            out[0]
        );
    }

    /// The emitted fill must be a command 1.21.11 actually accepts — the item
    /// component brackets are new ground here, and a syntax error would only
    /// surface as a silently-skipped line on a live server.
    #[test]
    fn every_emitted_fill_validates_against_the_command_tree() {
        let tree = crate::commands::CommandTree::v1_21_11();
        let out = loot_setup(&plan_of(vec![
            item("minecraft:cooked_cod", 3, None, &[]),
            item("minecraft:paper", 1, Some("Tide Ledger"), &[]),
            item(
                "minecraft:netherite_sword",
                1,
                Some("Bell-Breaker"),
                &[("minecraft:sharpness", 5), ("minecraft:unbreaking", 3)],
            ),
        ]));
        for line in &out {
            assert!(
                tree.validate_line(line).is_ok(),
                "emitted command must validate: {line}\n{:?}",
                tree.validate_line(line)
            );
        }
    }

    #[test]
    fn no_loot_emits_nothing() {
        assert!(loot_setup(&[]).is_empty());
    }

    // --- actor equipment (spec-0021) ---

    fn actor_with(eq: Option<delvewright_dsl::MobEquipment>) -> delvewright_dsl::Actor {
        delvewright_dsl::Actor {
            id: delvewright_dsl::ActorId("actor/elite".to_string()),
            entity: "minecraft:wither_skeleton".to_string(),
            name: None,
            skin: None,
            anchor: delvewright_dsl::AnchorId("anchor/stage".to_string()),
            facing: None,
            vulnerable: false,
            equipment: eq,
            attributes: None,
            tier: None,
            drops: Vec::new(),
            traversal: None,
        }
    }

    fn full_kit() -> delvewright_dsl::MobEquipment {
        use delvewright_dsl::{EnchantedItem, EquipItem};
        delvewright_dsl::MobEquipment {
            head: Some(EquipItem::Enchanted(EnchantedItem {
                item: "minecraft:netherite_helmet".to_string(),
                enchantments: [("minecraft:protection".to_string(), 4)]
                    .into_iter()
                    .collect(),
            })),
            chest: Some(EquipItem::Plain(
                "minecraft:netherite_chestplate".to_string(),
            )),
            legs: None,
            feet: None,
            main_hand: Some(EquipItem::Enchanted(EnchantedItem {
                item: "minecraft:netherite_sword".to_string(),
                enchantments: [("minecraft:sharpness".to_string(), 5)]
                    .into_iter()
                    .collect(),
            })),
            off_hand: None,
        }
    }

    /// An actor WITHOUT equipment must emit exactly what it did before the field
    /// existed — including no armed-mob default leaking in from the wave path.
    #[test]
    fn an_unequipped_actor_is_byte_identical() {
        let a = actor_with(None);
        assert_eq!(actor_equipment(&a), None);
        let puppet = actor_puppet_summon("dw", &a, [1, 2, 3], 0);
        assert!(!puppet.contains("equipment:"), "{puppet}");
        assert!(!actor_twin_summon("dw", &a, "~ ~ ~").contains("equipment:"));
    }

    /// The gear rides on BOTH bodies — the dormant puppet and the twin that
    /// replaces it — so unleashing does not undress the elite.
    #[test]
    fn equipment_lands_on_both_the_puppet_and_the_twin() {
        let a = actor_with(Some(full_kit()));
        let puppet = actor_puppet_summon("dw", &a, [1, 2, 3], 0);
        let twin = actor_twin_summon("dw", &a, "~ ~ ~");
        for s in [&puppet, &twin] {
            assert!(
                s.contains(
                    "mainhand:{id:\"minecraft:netherite_sword\",count:1,\
                     components:{\"minecraft:enchantments\":{\"minecraft:sharpness\":5}}}"
                ),
                "enchanted main hand missing:\n{s}"
            );
            assert!(
                s.contains(
                    "head:{id:\"minecraft:netherite_helmet\",count:1,\
                            components:{\"minecraft:enchantments\":{\"minecraft:protection\":4}}}"
                ),
                "enchanted helmet missing:\n{s}"
            );
            assert!(
                s.contains("chest:{id:\"minecraft:netherite_chestplate\",count:1}"),
                "plain chestplate missing:\n{s}"
            );
            // No-grind: an actor's kit is never lootable.
            assert!(
                s.contains("drop_chances:{mainhand:0.0f,head:0.0f,chest:0.0f}"),
                "zero drop chances missing:\n{s}"
            );
            assert!(!s.contains("ArmorItems") && !s.contains("HandItems"));
        }
    }
}
