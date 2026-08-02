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

use delvewright_dsl::{Objective, QuestEffect, Trigger, is_v03, is_v04, is_v06};

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
        code: &'static str,
        /// Human-readable explanation.
        message: String,
    },
}

/// `DW0312`: a `spawn-wave` needs more standable spawn cells near its anchor than
/// the anchor's own assembled room provides (task #41). Wave-mob placement seats
/// each mob on a compiler-validated standable cell confined to that room; when the
/// wave's mob count exceeds the room's footing, the build fails here rather than
/// letting mobs pile into blocks or spill across a socket seam. Analysis-tier
/// (exit 2, like reachability `DW02xx`): the fix is a content-design capacity
/// choice — shrink the wave or use a larger room — not a compiler/geometry defect.
pub const DW_WAVE_NO_ROOM: &str = "DW0312";

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
    let ns = &plan.namespace;
    let mut out: BuildOutput = BTreeMap::new();

    // Gravity-despawn gate (task #42, owner addendum): before any downstream model
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
    // critical path) and spawn-wave mob placement (task #41), so build it once when
    // either needs it. Includes any colliding relight fixtures (campfire / floor
    // lantern) so a fixture can never wedge a required path shut *nor* be stood on
    // by a spawned mob (spec-0010: verification re-runs after placement).
    // Visual-tier player-POV shots (spec-0003): first-person cameras along the
    // proven critical-path routes. Filled inside the world block below (they need
    // the routes + the assembled occupancy for the DW0724 clear-eye self-check);
    // empty for a campaign with no walked leg, so its render plan stays byte-identical.
    let mut pov_shots: Vec<crate::render_plan::PovShot> = Vec::new();

    // Actor spawn anchors must resolve to a world position (spec-0014); a spawn is a
    // summon, not a walk, so this needs no occupancy model. DW0325 if one dangles.
    crate::nav::check_actor_placement(plan)?;
    let has_waves = !plan.campaign.quests.content.waves.is_empty();
    let (moves, actor_moves, wave_placements): (
        Vec<crate::nav::MovePlan>,
        Vec<crate::nav::ActorMovePlan>,
        WavePlacements,
    ) = if crate::nav::needs_world(plan) || has_waves {
        {
            let world = match &edit_replay {
                Some(er) => {
                    let mut occ = crate::assembled::occupancy_of(
                        er.assembled.blocks.clone(),
                        &er.assembled.open_gates,
                    );
                    occ.solid.extend(relight.extra_solid.iter().copied());
                    crate::nav::World::from_occupancy(occ)
                }
                None => {
                    crate::nav::World::from_plan_with_extra(plan, structures, &relight.extra_solid)
                }
            };
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
                crate::nav::check_stealth_zones(plan, &world)?;
                // …and the onset-survivability proof on top of them (DW0352): a
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
                // Export the DW0311-proven critical-path routes as validation
                // metadata (task #38): thinned per-leg waypoint polylines the harness
                // replays as successive nearby goals, so no single giant mineflayer A*
                // solve strands the bot on a large open cave. NOT shipped gameplay —
                // lives under `validation/` (excluded from the delve image, like
                // packtest-datapack/). Emitted only when a walked leg exists, so a
                // campaign with none stays byte-identical to before. Uses the same
                // relight-aware `world` as the DW0311 check it exports.
                let routes = crate::nav::critical_path_routes(plan, &world);
                // Structural self-check (task #45): every exported waypoint must be
                // genuinely standable in this FINAL world (settled + water-flooded +
                // fixtures). Makes it impossible to ship a waypoint the game floods
                // or walls — the water-flow / post-nav-mutation divergence class —
                // failing the build loudly (DW0314) instead of stranding the bot.
                crate::nav::verify_exported_routes(&world, &routes)?;
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
                (m, am)
            } else {
                (Vec::new(), Vec::new())
            };
            // Seat each wave mob on a validated standable cell near its anchor, in
            // room only (DW0312 if the room lacks the footing).
            let waves = plan_wave_spawns(plan, &world)?;
            (moves, actor_moves, waves)
        }
    } else {
        (Vec::new(), Vec::new(), BTreeMap::new())
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
        for piece in &area.pieces {
            if let Some(bytes) = structures.get(&piece.structure_file) {
                out.insert(
                    format!("datapack/data/{ns}/structure/{}.nbt", piece.structure_id),
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
        for piece in &area.pieces {
            if let Some(bytes) = structures.get(&piece.structure_file)
                && let Some(s) = structure_sentinel(bytes)
            {
                sentinels.insert(piece.structure_file.clone(), s);
            }
        }
    }
    let functions = emit_functions(
        plan,
        &sentinels,
        &moves,
        &actor_moves,
        &relight.placements,
        &wave_placements,
        edit_replay.as_ref().map_or(&[][..], |er| &er.commands),
        &edit_replay.as_ref().map_or(Vec::new(), |er| {
            er.batches.iter().filter_map(|b| b.bounds).collect()
        }),
    );
    for (name, body) in &functions {
        out.insert(
            format!("datapack/data/{ns}/function/{name}.mcfunction"),
            body.clone().into_bytes(),
        );
    }

    // dialogs
    for (name, value) in emit_dialogs(plan) {
        put_json(
            &mut out,
            &format!("datapack/data/{ns}/dialog/{name}.json"),
            &value,
        );
    }

    // advancements
    for (name, value) in emit_advancements(plan) {
        put_json(
            &mut out,
            &format!("datapack/data/{ns}/advancement/{name}.json"),
            &value,
        );
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
    emit_packtest(plan, &mut out, &moves, &actor_moves);

    // ---- creator overlay (playtest-only; spec-0006) ----
    // A self-contained module (crate::creator). Its `.mcfunction`s are plain
    // vanilla, so they flow through the command-tree validator below and the
    // determinism gate like the main datapack; the shipped delve image excludes
    // this directory (CI-checked, same as packtest-datapack/).
    crate::creator::emit_creator(plan, &mut out);

    // ---- server ----
    emit_server(plan, &mut out);

    // ---- critical path ----
    put_json(&mut out, "critical-path.json", &emit_critical_path(plan));

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

    // ---- NPC-skin resource pack (spec-0009) ----
    // A campaign with skinned (mannequin) NPCs ships a deterministic resource-pack
    // zip; its SHA-1 is what a client verifies against the itzg RESOURCE_PACK_SHA1
    // env. The serving/env plumbing is the packaging task's; here we emit the zip,
    // its sha1 (in the manifest), and a SKINS.md note listing the env to set.
    // The pack also carries the `delve:art` title font (spec-0014) when the
    // campaign uses the `narrate` `art` style — baked only when needed, so a
    // non-art campaign's pack is byte-identical.
    let art = crate::atmos::uses_art(plan.campaign);
    let extra_assets = if art {
        crate::atmos::art_font_assets()
    } else {
        BTreeMap::new()
    };
    let resource_pack_sha1 = if skins.is_empty() && extra_assets.is_empty() {
        None
    } else {
        let zip = crate::resourcepack::build_pack(skins, &extra_assets);
        let sha1 = crate::resourcepack::sha1_hex(&zip);
        out.insert("resourcepack.zip".to_string(), zip);
        out.insert(
            "SKINS.md".to_string(),
            pack_note(&sha1, skins, art).into_bytes(),
        );
        Some(sha1)
    };

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

    Ok(out)
}

/// The `SKINS.md` build-output note: how the packaging task wires the emitted
/// resource pack into the delve image (itzg env), plus the pack SHA-1. The pack
/// carries the mannequin NPC skins (spec-0009) and/or the `delve:art` title font
/// (spec-0014), depending on what the campaign uses.
fn pack_note(sha1: &str, skins: &BTreeMap<String, Vec<u8>>, art: bool) -> String {
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
             used by `narrate` `style: art`.\n",
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
fn sealing_commands(
    time: delvewright_dsl::WorldTime,
    weather: Option<delvewright_dsl::WorldWeather>,
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
    cmds
}

/// Yaw for a facing keyword (MC: yaw 0 = +z/south).
fn facing_yaw(facing: Option<&str>) -> i32 {
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
/// `obj/door`) as player-visible floating text (presentation hygiene, task #54
/// addendum). Titled markers are unchanged (byte-identical).
fn marker_name_fields(title: Option<&str>) -> String {
    match title {
        Some(t) => format!("CustomName:{},CustomNameVisible:1b,", snbt_string(t)),
        None => String::new(),
    }
}

/// A text-component SNBT **compound** for a player-visible string:
/// `{text:"<escaped>"}`. Used for mannequin `description` (DSL v0.4) and any
/// component-form NBT field. This is deliberately NOT the stringified-JSON form
/// `'{"text":…}'`, which 1.21.11 renders as literal raw JSON above an entity's
/// head (owner-verified). The generated summons carry no `'{"text"` substring.
fn snbt_text_component(s: &str) -> String {
    format!("{{text:{}}}", snbt_string(s))
}

/// The default main-hand weapon for a summoned mob whose natural spawns are
/// armed, or `None` for mobs that spawn unarmed. Small static table (documented
/// in the compiler README); mobs not listed (zombie, drowned — a wild trident is
/// not a default) get nothing.
fn default_mainhand(entity: &str) -> Option<&'static str> {
    match entity.strip_prefix("minecraft:").unwrap_or(entity) {
        "wither_skeleton" => Some("minecraft:stone_sword"),
        "skeleton" | "stray" => Some("minecraft:bow"),
        _ => None,
    }
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

/// The `equipment`/`drop_chances` SNBT fragment for a wave mob (no leading
/// comma), or `None` for a bare-handed mob. A mob without the v0.6 `equipment`
/// field takes the [`default_equipment`] path **unchanged** (byte-identity for
/// pre-equipment waves). With the field, explicit slots merge over the
/// armed-mob main-hand default (an explicit `main_hand` overrides it — a
/// helmeted skeleton keeps its bow). Every emitted slot carries drop chance 0:
/// players must never farm wave gear (no-grind constitution). Component-era
/// form only — see [`default_equipment`] for why legacy `ArmorItems`/
/// `HandItems` are silently ignored by 1.21.11 `/summon`. Slot order is fixed
/// (mainhand, offhand, head, chest, legs, feet) for ADR-0006 determinism.
fn wave_equipment(entity: &str, eq: Option<&delvewright_dsl::MobEquipment>) -> Option<String> {
    let Some(eq) = eq else {
        return default_equipment(entity);
    };
    let mainhand = eq.main_hand.as_deref().or_else(|| default_mainhand(entity));
    let slots: [(&str, Option<&str>); 6] = [
        ("mainhand", mainhand),
        ("offhand", eq.off_hand.as_deref()),
        ("head", eq.head.as_deref()),
        ("chest", eq.chest.as_deref()),
        ("legs", eq.legs.as_deref()),
        ("feet", eq.feet.as_deref()),
    ];
    let mut items: Vec<String> = Vec::new();
    let mut chances: Vec<String> = Vec::new();
    for (slot, item) in slots {
        if let Some(it) = item {
            items.push(format!("{slot}:{{id:\"{it}\",count:1}}"));
            chances.push(format!("{slot}:0.0f"));
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
    let Some(a) = attrs else {
        return String::new();
    };
    let mut entries: Vec<String> = Vec::new();
    let mut add = |id: &str, v: Option<f64>| {
        if let Some(x) = v {
            entries.push(format!("{{id:\"minecraft:{id}\",base:{}}}", fmt_f64(x)));
        }
    };
    add("max_health", a.max_health);
    add("attack_damage", a.attack_damage);
    add("movement_speed", a.movement_speed);
    add("follow_range", a.follow_range);
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

#[allow(clippy::too_many_arguments)]
fn emit_functions(
    plan: &Plan,
    sentinels: &Sentinels,
    moves: &[crate::nav::MovePlan],
    actor_moves: &[crate::nav::ActorMovePlan],
    relight: &[crate::light::Placement],
    wave_placements: &WavePlacements,
    world_edits: &[String],
    edit_bounds: &[([i32; 3], [i32; 3])],
) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let v03 = campaign_is_v03(plan);
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
        is_v06(c.world.dsl_version.as_str()),
    ));
    setup.push("scoreboard objectives add dw.class trigger".to_string());
    setup.push("scoreboard objectives add dw.classed dummy".to_string());
    setup.push("scoreboard objectives add dw.dlg_shown dummy".to_string());
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
    // v0.4: the per-player scratch bitmask used by display-gated dialogue choosers
    // (flag axis and/or objective-state axis). Declared only when a gated option
    // exists, so v0.2/v0.3 setup is unchanged.
    if has_gated_dialogue(c) {
        setup.push("scoreboard objectives add dw.dmask dummy".to_string());
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
    // `deathCount` respawn-detection scores. Emitted only when a checkpoint carries
    // an `on_respawn` hook (the only consumer of the marker), so pre-0.6 campaigns —
    // and checkpoint campaigns without hooks — stay byte-identical here.
    if plan.any_checkpoint_on_respawn() {
        setup.push("scoreboard players set #cp dw.sys -1".to_string());
        setup.push("scoreboard objectives add dw.deaths deathCount".to_string());
        setup.push("scoreboard objectives add dw.death_ack dummy".to_string());
    }
    // v0.6 stealth beats (spec-0014; sneak requirement removed by owner ruling
    // 2026-08-01): the active-session marker + per-player grace scores. Hidden =
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

    // --- place_all: idempotent template placement, retried from tick ---
    let mut place_all: Vec<String> = Vec::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let rot = match piece.rotation.token() {
                Some(t) => format!(" {t}"),
                None => String::new(),
            };
            place_all.push(format!(
                "place template {ns}:{} {} {} {}{rot}",
                piece.structure_id, piece.pos[0], piece.pos[1], piece.pos[2]
            ));
        }
    }
    fns.push(("place_all".to_string(), lines(&place_all)));

    // --- place_verify: sentinel check per piece; all present → setup_finish ---
    let mut place_verify: Vec<String> = Vec::new();
    place_verify.push("scoreboard players set #placeok dw.sys 0".to_string());
    let mut sentinel_count = 0u32;
    for area in &plan.areas {
        for piece in &area.pieces {
            if let Some((local, block)) = sentinels.get(&piece.structure_file) {
                let w = piece.rotation.transform(*local);
                let (sx, sy, sz) = (
                    piece.pos[0] + w[0],
                    piece.pos[1] + w[1],
                    piece.pos[2] + w[2],
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
    // single-prefab areas.
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
    setup.extend(env_trigger_setup(plan));
    // v0.6: fill each trap dispenser payload and summon disarm affordances
    // (spec-0011). Empty for a campaign with no traps → byte-identical.
    setup.extend(trap_setup(plan));
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
    tick.push("scoreboard players enable @a dw.class".to_string());
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
    tick.push(format!(
        "execute as @a unless score @s dw.classed matches 1 unless score @s dw.dlg_shown matches 1 run function {ns}:show_class"
    ));
    for class in &plan.classes {
        tick.push(format!(
            "execute as @a[scores={{dw.class={}}}] run function {ns}:class_apply_{}",
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
    if v03 {
        for q in &c.quests.content.quests {
            let qa = quest_active_score(q.id.as_str());
            for o in &q.objectives {
                if o.title().is_some() {
                    tick.push(format!(
                        "execute as @a{} unless score @s {} matches 1 run function {ns}:announce_{}",
                        pending_guard(o, &qa),
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
                    "execute as @a{} unless score {} dw.sys matches 1 run function {ns}:activate_{}",
                    pending_guard(o, &qa),
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
    for q in &c.quests.content.quests {
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
                            pending_guard(o, &qa),
                            pos[0] - 1, pos[1] - 1, pos[2] - 1,
                            safe_obj_fn(id.as_str())
                        ));
                    } else {
                        tick.push(format!(
                            "execute as @a{} if entity @s[x={},y={},z={},distance=..{}] run function {ns}:complete_{}",
                            pending_guard(o, &qa),
                            pos[0], pos[1], pos[2], radius,
                            safe_obj_fn(id.as_str())
                        ));
                    }
                }
                Objective::Kill { id, wave, .. } => {
                    tick.push(format!(
                        "execute as @a{} if score {} {} matches ..0 run function {ns}:complete_{}",
                        pending_guard(o, &qa),
                        plan::wave_counter(wave.as_str()),
                        plan::WAVE_OBJECTIVE,
                        safe_obj_fn(id.as_str())
                    ));
                }
                Objective::Interact {
                    id, requires_item, ..
                } => {
                    let trigger = plan::interact_trigger(id.as_str());
                    let item_guard = match requires_item {
                        Some(it) => format!(" if items entity @s container.* {it}"),
                        None => String::new(),
                    };
                    // The trigger is set by the bot's chat command or the
                    // interaction advancement's reward; the guard applies uniformly.
                    tick.push(format!(
                        "execute as @a[scores={{{trigger}=1..}}]{}{item_guard} run function {ns}:complete_{}",
                        pending_guard(o, &qa),
                        safe_obj_fn(id.as_str())
                    ));
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
                        pending_guard(o, &qa)
                    ));
                    tick.push(format!(
                        "execute as @a{} if score @s {COLLECT_HOLD} matches {count}.. run function {ns}:complete_{}",
                        pending_guard(o, &qa),
                        safe_obj_fn(id.as_str())
                    ));
                }
                Objective::TalkTo { .. } | Objective::Collect { .. } => {}
            }
        }
    }
    // v0.4: environment-trigger per-tick checks (empty for a campaign with no
    // triggers → byte-identical).
    tick.extend(env_trigger_tick(plan));
    // v0.6: trap disarm-affordance detection (spec-0011). Empty for a campaign with
    // no disarmable traps → byte-identical.
    tick.extend(trap_tick(plan));
    // v0.6 checkpoints (spec-0012): per-player respawn detection via the vanilla
    // `deathCount` criterion, dispatching the active checkpoint's `on_respawn`.
    // Only when a checkpoint carries an `on_respawn` hook.
    if plan.any_checkpoint_on_respawn() {
        tick.push(format!("execute as @a run function {ns}:cp_respawn_check"));
    }
    // v0.6 stealth (spec-0014): while a beat is active, run its per-tick judge.
    for beat in &plan.stealth_beats {
        tick.push(format!(
            "execute if score #stealth dw.sys matches {} run function {ns}:stealth_tick_{}",
            beat.index, beat.index
        ));
    }
    fns.push(("tick".to_string(), lines(&tick)));

    // --- v0.6 checkpoint respawn dispatch (spec-0012) ---
    fns.extend(emit_checkpoint_functions(plan));
    // --- v0.6 stealth-beat functions (spec-0014) ---
    fns.extend(emit_stealth_functions(plan));

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

    // --- show_class ---
    fns.push((
        "show_class".to_string(),
        lines(&[
            format!("dialog show @s {ns}:class_select"),
            "scoreboard players set @s dw.dlg_shown 1".to_string(),
        ]),
    ));

    // --- class apply ---
    let campaign_start = campaign_start_quests(c);
    for (i, class) in c.classes.content.classes.iter().enumerate() {
        let plan_class = &plan.classes[i];
        let mut body: Vec<String> = Vec::new();
        body.push("scoreboard players reset @s dw.class".to_string());
        for item in &class.kit {
            let comp = match &item.name {
                Some(n) => format!("[custom_name={}]", json!({ "text": n, "italic": false })),
                None => String::new(),
            };
            body.push(format!("give @s {}{} {}", item.item, comp, item.count));
        }
        body.push("scoreboard players set @s dw.classed 1".to_string());
        for qid in &campaign_start {
            body.push(format!(
                "scoreboard players set @s {} 1",
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
            for f in &opt.requires_flags {
                body.push(format!(
                    "execute unless score @s {} matches 1 run return fail",
                    plan::flag_score(f)
                ));
            }
            // v0.6: the negative gate — a `forbids_flags`-suppressed option is
            // equally inert to a direct `/trigger` once any listed flag is set.
            for f in &opt.forbids_flags {
                body.push(format!(
                    "execute if score @s {} matches 1 run return fail",
                    plan::flag_score(f)
                ));
            }
            // v0.4: set any flags this option declares (dialogue `set-flag`).
            for f in &opt.sets_flags {
                body.push(format!(
                    "scoreboard players set @s {} 1",
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
                        "execute if score @s {} matches 1 unless score @s {} matches 1 run function {ns}:complete_{}",
                        quest_active_score(qid),
                        obj_score(obj),
                        safe_obj_fn(obj)
                    ));
                }
            }
            if let Some(next) = &opt.next {
                body.push(show_node_cmd(plan, npc, next));
            }
            fns.push((format!("dlg_{}_{}", npc.safe, opt.n), lines(&body)));
        }
        // keeper interaction reward: (re)show the root dialog.
        fns.push((
            format!("talk_{}", npc.safe),
            lines(&[
                format!("advancement revoke @s only {ns}:{}_interact", npc.safe),
                show_node_cmd(plan, npc, &npc.root),
            ]),
        ));
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
                let mut ann: Vec<String> = Vec::new();
                ann.push(format!(
                    "tellraw @s {}",
                    json!([
                        { "text": "New objective: ", "color": "yellow", "bold": true },
                        { "text": title, "color": "gold" }
                    ])
                ));
                if let Some(hint) = o.hint() {
                    ann.push(format!(
                        "tellraw @s {}",
                        json!({ "text": hint, "color": "gray", "italic": true })
                    ));
                }
                ann.push("playsound minecraft:block.note_block.pling player @s".to_string());
                ann.push(format!(
                    "scoreboard players set @s {} 1",
                    announce_score(oid)
                ));
                fns.push((format!("announce_{}", safe_obj_fn(oid)), lines(&ann)));
            }

            let mut body: Vec<String> = Vec::new();
            body.push(format!("scoreboard players set @s {} 1", obj_score(oid)));
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
                    "tellraw @s {}",
                    json!([
                        { "text": "Objective complete: ", "color": "green" },
                        { "text": title, "color": "white" }
                    ])
                ));
                body.push("playsound minecraft:entity.experience_orb.pickup player @s".to_string());
            }
            // Objective-marker lifecycle (task #45): despawn every ENTITY this
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
            // runs with the acting player as `@s` (see `Executor::Player`).
            body.extend(emit_effect_bundle(
                plan,
                objective_effects(c, oid),
                Executor::Player,
            ));
            // Inter-area transport: if completing this objective moves the player
            // into a different area on the critical path, teleport them to that
            // area's entry spawn (areas are AREA_SPACING apart across void). Runs
            // after gate effects so the destination area is already unlocked.
            if let Some(pos) = plan.transport.get(oid) {
                body.push(format!("teleport @s {} {} {}", pos[0], pos[1], pos[2]));
            }
            body.push(format!(
                "function {ns}:check_q_{}",
                plan::safe_local(q.id.as_str())
            ));
            fns.push((format!("complete_{}", safe_obj_fn(oid)), lines(&body)));
        }

        // check_q_<quest>
        let mut check: Vec<String> = Vec::new();
        let mut guard = "execute".to_string();
        for o in &q.objectives {
            guard.push_str(&format!(
                " if score @s {} matches 1",
                obj_score(o.id().as_str())
            ));
        }
        guard.push_str(&format!(
            " unless score @s {} matches 1 run function {ns}:complete_q_{}",
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
            "scoreboard players set @s {} 1",
            quest_score(q.id.as_str())
        ));
        done.extend(emit_effect_bundle(plan, &q.on_complete, Executor::Player));
        // activate quests triggered by this quest's completion
        for dep in &c.quests.content.quests {
            if let Trigger::QuestComplete { quest } = &dep.trigger
                && quest.as_str() == q.id.as_str()
            {
                done.push(format!(
                    "scoreboard players set @s {} 1",
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
    cc.push("scoreboard players set @s dw.campaign 1".to_string());
    cc.push(format!("advancement grant @s only {ns}:campaign_complete"));
    cc.push(format!(
        "tellraw @s {}",
        json!([
            { "text": format!("{title} — complete."), "color": "gold" },
            { "text": "\n" },
            { "text": "A Delvewright delve.", "color": "gray" }
        ])
    ));
    // v0.3 finale fanfare (M2 fix 4): the owner finished the finale and got no
    // feedback. Show a proper title banner + play a fanfare. Gated on v0.3 so the
    // shared `campaign_complete` stays byte-identical for hello-world / keep-crawl.
    if v03 {
        cc.push(format!(
            "title @s title {}",
            json!({ "text": "Delve Complete", "color": "gold", "bold": true })
        ));
        cc.push(format!(
            "title @s subtitle {}",
            json!({ "text": title, "color": "yellow" })
        ));
        cc.push("playsound minecraft:ui.toast.challenge_complete player @s".to_string());
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
        // anchor's own room, one per mob (task #41). A wave whose spawn anchor
        // resolves in no assembled area gets no entry here and is skipped exactly
        // as before — DW0310 (check_wave_spawns) catches a dangling spawn-wave.
        let Some(cells) = wave_placements.get(w.id.as_str()) else {
            continue;
        };
        let mut body: Vec<String> = Vec::new();
        body.push(format!(
            "scoreboard players set {} {} {}",
            plan::wave_counter(w.id.as_str()),
            plan::WAVE_OBJECTIVE,
            plan::wave_total(w)
        ));
        let mut idx = 0i32;
        for mob in &w.mobs {
            // CustomName as a plain SNBT text component (M2 fix 1). Waves are
            // v0.3-only, so no v0.2 byte-identity concern.
            let name = match &mob.name {
                Some(n) => format!(",CustomName:{},CustomNameVisible:1b", snbt_string(n)),
                None => String::new(),
            };
            // Equipment: v0.6 explicit slots merged over the armed-mob default
            // (M2 fix 5: a summoned wither_skeleton/skeleton otherwise had no
            // weapon and was trivial). All drop chances 0 — never lootable.
            let equip = wave_equipment(&mob.entity, mob.equipment.as_ref())
                .map(|e| format!(",{e}"))
                .unwrap_or_default();
            // v0.4 attribute overrides (spec-0008 §4), emitted as 1.21.11
            // attribute components in the summon NBT. Empty for a plain mob.
            let attrs = attributes_snbt(mob.attributes.as_ref());
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
                body.push(format!(
                    "summon {} {} {} {} {{Tags:[\"{}\"{tmp}],PersistenceRequired:1b{name}{equip}{attrs}}}",
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
        fns.push((
            format!("spawn_{}", plan::safe_local(w.id.as_str())),
            lines(&body),
        ));
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
                                pending_guard(o, &qa),
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
    fns.extend(cutscene_fns(plan, moves, actor_moves));
    fns.extend(env_trigger_fns(plan));
    fns.extend(trap_fns(plan));
    fns.extend(boundary_fns(plan));
    fns.extend(night_vision_fns(plan));

    fns.sort_by(|a, b| a.0.cmp(&b.0));
    fns
}

/// Validated spawn cells per wave: wave id → one standable cell per mob, in
/// summon order (task #41). Only waves whose spawn anchor resolves have an entry.
type WavePlacements = BTreeMap<String, Vec<[i32; 3]>>;

/// Seat every wave's mobs on compiler-validated standable cells near the wave
/// anchor, confined to the anchor's own assembled piece so the flock never strings
/// across a socket seam into a neighbouring room (task #41: the field bug where six
/// sheep spread `+x` off the den anchor across the den↔mouth seam toward void, some
/// ending inside blocks or outside the room). Cells are chosen by ascending BFS
/// distance from the anchor with a fixed `(y, z, x)` tie-break — deterministic
/// (ADR-0006). A wave that needs more standable footing than its room offers fails
/// the build with [`DW_WAVE_NO_ROOM`] (`DW0312`). A wave whose spawn anchor resolves
/// in no assembled area is skipped (DW0310 handles the dangling reference).
fn plan_wave_spawns(
    plan: &Plan,
    world: &crate::nav::World,
) -> Result<WavePlacements, BuildFailure> {
    // Wave mobs cannot right-click a fence gate open: seat them on the
    // no-gate-use view, where a closed gate cell is a 1.5-tall barrier — never a
    // seat, and never a doorway the seating flood spills through (task #59).
    let entity_world_owned;
    let world: &crate::nav::World = if world.has_use_gates() {
        entity_world_owned = world.without_gate_use();
        &entity_world_owned
    } else {
        world
    };
    let c = plan.campaign;
    let mut out: WavePlacements = BTreeMap::new();
    for w in &c.quests.content.waves {
        let (Some(anchor), Some(area)) = (
            wave_spawn_pos(plan, w.id.as_str()),
            plan::wave_area(c, w.id.as_str()),
        ) else {
            continue;
        };
        let need = plan::wave_total(w).max(0) as usize;
        let bounds = wave_piece_bounds(plan, area, anchor);
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
    Ok(out)
}

/// The AABB of the assembled piece carrying a wave's spawn anchor — the room the
/// wave's mobs must stay inside, so the placement flood-fill never crosses a socket
/// seam. Falls back to the whole area's bounds if the anchor sits in no single
/// piece box (defensive; a single-prefab area has exactly one piece == the area).
fn wave_piece_bounds(plan: &Plan, area_id: &str, anchor: [i32; 3]) -> ([i32; 3], [i32; 3]) {
    let Some(area) = plan.areas.iter().find(|a| a.area_id == area_id) else {
        return (anchor, anchor);
    };
    for piece in &area.pieces {
        let (lo, hi) = piece.bbox();
        if (0..3).all(|i| lo[i] <= anchor[i] && anchor[i] <= hi[i]) {
            return (lo, hi);
        }
    }
    area.bounds()
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

/// Fail the build if any `spawn-wave` effect references a wave whose spawn
/// position cannot be resolved (`DW0310`). Such a wave emits no `spawn_<wave>`
/// function, yet the effect still emits a `function <ns>:spawn_<wave>` call — a
/// silently dangling reference that would never spawn the wave at runtime. A
/// compile-time diagnostic turns that content mistake into a loud build failure
/// instead of a missing enemy the QA hour has to notice.
fn check_wave_spawns(plan: &Plan) -> Result<(), BuildFailure> {
    let c = plan.campaign;
    let effects = c
        .quests
        .content
        .quests
        .iter()
        .flat_map(|q| {
            q.on_objective_complete
                .values()
                .flatten()
                .chain(&q.on_complete)
        })
        .chain(c.quests.content.triggers.iter().flat_map(|t| &t.effects));
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for e in effects {
        if let Some(wave) = e.spawn_wave() {
            let id = wave.as_str();
            if seen.insert(id) && wave_spawn_pos(plan, id).is_none() {
                return Err(BuildFailure::Diagnostic {
                    code: "DW0310",
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

/// Emit a quest effect, wrapping every command it produces in a per-player flag
/// guard when the effect declares `requires_flags` and/or `forbids_flags` (DSL
/// v0.6). The guard is `execute if score @s dw.f_<flag> matches 1 [… per
/// required flag] unless score @s dw.f_<flag> matches 1 [… per forbidden flag]
/// run <command>` — `unless … matches 1` deliberately treats an **unset** score
/// as "not set" (flag scores are never pre-initialized to 0, so a
/// `scores={…=..0}` selector would not work). These effects already run in
/// per-player context (`complete_<obj>` and `trig_<id>` are entered `as
/// @a`/`@s`), so `@s` resolves to the acting player. An ungated effect (both
/// lists empty) is emitted verbatim — byte-identical to the pre-0.6 output,
/// preserving determinism for every existing campaign.
fn emit_gated_effect(plan: &Plan, eff: &QuestEffect, body: &mut Vec<String>) {
    let flags = eff.requires_flags();
    let forbids = eff.forbids_flags();
    if flags.is_empty() && forbids.is_empty() {
        emit_quest_effect(plan, eff, body);
        return;
    }
    let mut inner: Vec<String> = Vec::new();
    emit_quest_effect(plan, eff, &mut inner);
    let guard: String = flags
        .iter()
        .map(|f| format!("if score @s {} matches 1 ", plan::flag_score(f.as_str())))
        .chain(forbids.iter().map(|f| {
            format!(
                "unless score @s {} matches 1 ",
                plan::flag_score(f.as_str())
            )
        }))
        .collect();
    for line in inner {
        body.push(format!("execute {guard}run {line}"));
    }
}

// ---------------------------------------------------------------------------
// Effect bundles and their command source (task: scheduled-executor fix)
// ---------------------------------------------------------------------------

/// The command source a generated effect bundle is entered under.
///
/// **The bug this models (AUDIT-P0).** Vanilla's `schedule function …` re-invokes
/// a function with the **server** command source: no executor, so `@s` resolves
/// to nothing and every `@s`-addressed command silently fails (a scheduled
/// `scoreboard players set @s dw.f_hidden 1` sets nobody's flag, and the
/// objective it gates never unlocks — the island's "Get Into the Shadows"
/// soft-lock). Three generated bundles are only ever reached that way: a
/// `move-npc`/`move-actor` `on_arrive` (fired from the scheduled walk driver)
/// and every `sequence` step function (fired from the scheduled timeline). They
/// are emitted with [`Executor::Server`]; every other bundle keeps
/// [`Executor::Player`] and stays byte-identical.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Executor {
    /// Entered from a per-player dispatch (`complete_<obj>` / `trig_<id>` /
    /// `cp_on_respawn_<i>` / `stealth_caught_<i>` are all entered `as @a`/`as
    /// @s`), so `@s` is the acting player and effects are emitted verbatim.
    Player,
    /// Entered from the scheduler (`schedule function …`) — the server command
    /// source, with no `@s`. Per-player effects must re-establish the party
    /// executor themselves; global effects must NOT (they would multi-fire).
    Server,
}

/// Does this effect's emitted commands address the **acting player** (`@s`)?
///
/// This is the multiplicity contract of the scheduled-executor fix: a
/// player-scoped effect runs once per player (`execute as @a … run`, exactly
/// what a top-level bundle gets from its `as @a` dispatch, at the same —
/// unmoved — command position); a global effect runs exactly once, because a
/// blanket `as @a` around a whole bundle would fire every `fill`, `summon`,
/// `schedule` and driver start once per player.
///
/// Deliberately an exhaustive match with no wildcard arm: a new effect verb must
/// state its executor scope here or the compiler refuses to build.
fn effect_is_player_scoped(eff: &QuestEffect) -> bool {
    match eff {
        // Per-player: every emitted command names `@s`, or calls a function
        // whose body does (`campaign_complete`).
        QuestEffect::CampaignComplete
        | QuestEffect::GiveItem { .. }
        | QuestEffect::SetFlag { .. }
        | QuestEffect::Narrate { .. }
        | QuestEffect::PlaySound { .. }
        | QuestEffect::DamagePlayers { .. } => true,
        // Global: world edits, entity commands, dimension-wide cuts, and calls
        // into generated functions that are themselves party-wide or
        // server-safe (`spawn_<wave>`, `spawn_actor_<id>`, `spawn_npc_<id>`,
        // `unleash_<id>`, the `mv_`/`ma_` driver starts, `cs_<key>`,
        // `stealth_begin_<i>`, and `seq_<key>` — whose step functions are
        // themselves emitted server-source-safe).
        QuestEffect::OpenGate { .. }
        | QuestEffect::CloseGate { .. }
        | QuestEffect::SpawnWave { .. }
        | QuestEffect::SetBlock { .. }
        | QuestEffect::DespawnNpc { .. }
        | QuestEffect::MoveNpc { .. }
        | QuestEffect::Cutscene { .. }
        | QuestEffect::SetTime { .. }
        | QuestEffect::SetWeather { .. }
        | QuestEffect::SetCheckpoint { .. }
        | QuestEffect::BeginStealth { .. }
        | QuestEffect::EndStealth
        | QuestEffect::SpawnActor { .. }
        | QuestEffect::DespawnActor { .. }
        | QuestEffect::MoveActor { .. }
        | QuestEffect::UnleashActor { .. }
        | QuestEffect::Sequence { .. }
        | QuestEffect::SpawnNpc { .. } => false,
    }
}

/// Splice an `execute` prefix (already space-terminated, e.g. `as @a if score @s
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

/// Emit one effect of a bundle entered with the **server** command source.
///
/// Per-player effects are re-bound to the party (`as @a`) so each player is the
/// `@s` of the effect's own commands — the same executor and the same (unmoved)
/// command position a top-level `execute as @a … run function complete_<obj>`
/// gives them. Global effects are emitted bare, so they fire exactly once.
///
/// The per-effect flag gate (v0.6) follows the executor: under `as @a` it keeps
/// its per-player spelling (`if score @s dw.f_<flag> matches 1`); on a global
/// effect there is no player to ask, so it degrades to the party predicate the
/// trigger layer already uses — `if entity @a[scores={dw.f_<flag>=1..}]` ("any
/// player holds it") and `unless entity @a[scores={…}]` ("no player holds it").
/// Note that these bundles previously dropped the gate entirely (they called
/// `emit_quest_effect`, not `emit_gated_effect`), so a gated effect inside an
/// `on_arrive`/`sequence` step fired unconditionally.
fn emit_gated_effect_server(plan: &Plan, eff: &QuestEffect, body: &mut Vec<String>) {
    let per_player = effect_is_player_scoped(eff);
    let mut inner: Vec<String> = Vec::new();
    emit_quest_effect(plan, eff, &mut inner);
    let mut prefix = String::new();
    if per_player {
        prefix.push_str("as @a ");
    }
    for f in eff.requires_flags() {
        let score = plan::flag_score(f.as_str());
        prefix.push_str(&if per_player {
            format!("if score @s {score} matches 1 ")
        } else {
            format!("if entity @a[scores={{{score}=1..}}] ")
        });
    }
    for f in eff.forbids_flags() {
        let score = plan::flag_score(f.as_str());
        prefix.push_str(&if per_player {
            format!("unless score @s {score} matches 1 ")
        } else {
            format!("unless entity @a[scores={{{score}=1..}}] ")
        });
    }
    for line in inner {
        body.push(with_execute_prefix(&prefix, line));
    }
}

/// Emit a whole effect bundle under `exec` (see [`Executor`]).
fn emit_effect_bundle<'a>(
    plan: &Plan,
    effects: impl IntoIterator<Item = &'a QuestEffect>,
    exec: Executor,
) -> Vec<String> {
    let mut body: Vec<String> = Vec::new();
    for e in effects {
        match exec {
            Executor::Player => emit_gated_effect(plan, e, &mut body),
            Executor::Server => emit_gated_effect_server(plan, e, &mut body),
        }
    }
    body
}

/// Emit a quest effect's commands into `body`.
fn emit_quest_effect(plan: &Plan, eff: &QuestEffect, body: &mut Vec<String>) {
    let ns = &plan.namespace;
    match eff {
        QuestEffect::OpenGate { anchor, .. } => {
            // Find the gate anchor across areas (first match).
            for ((_, name), resolved) in &plan.anchors {
                if name == anchor.as_str()
                    && let ResolvedAnchor::Gate { from, to, block } = resolved
                {
                    body.push(format!(
                        "fill {} {} {} {} {} {} minecraft:air replace {}",
                        from[0], from[1], from[2], to[0], to[1], to[2], block
                    ));
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
                    body.push(format!(
                        "fill {} {} {} {} {} {} {}",
                        from[0], from[1], from[2], to[0], to[1], to[2], block
                    ));
                    return;
                }
            }
        }
        QuestEffect::CampaignComplete => {
            body.push(format!("function {ns}:campaign_complete"));
        }
        QuestEffect::GiveItem {
            item, count, name, ..
        } => {
            let comp = match name {
                Some(n) => format!("[custom_name={}]", json!({ "text": n, "italic": false })),
                None => String::new(),
            };
            body.push(format!("give @s {item}{comp} {count}"));
        }
        QuestEffect::SetFlag { flag, .. } => {
            body.push(format!(
                "scoreboard players set @s {} 1",
                plan::flag_score(flag.as_str())
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
            emit_narrate(text, *style, sound.as_deref(), body);
        }
        QuestEffect::SetBlock { anchor, block, .. } => {
            if let Some(pos) = anchor_point_any(plan, anchor.as_str()) {
                body.push(format!("setblock {} {} {} {block}", pos[0], pos[1], pos[2]));
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
                movenpc_fn(npc.as_str(), to_anchor.as_str())
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
            emit_play_sound(plan, sound, at.as_ref(), *volume, *pitch, body);
        }
        QuestEffect::DamagePlayers {
            amount,
            within,
            damage_type,
            ..
        } => {
            emit_damage_players(plan, *amount, within.as_ref(), *damage_type, body);
        }
        QuestEffect::SetCheckpoint { anchor, on_respawn } => {
            emit_set_checkpoint(plan, anchor.as_str(), on_respawn, body);
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
        // --- DSL v0.6 actor staging effects (spec-0014) ---
        QuestEffect::SpawnActor { actor } => {
            body.push(format!(
                "function {ns}:spawn_actor_{}",
                plan::safe_local(actor.as_str())
            ));
        }
        QuestEffect::DespawnActor { actor, style } => {
            emit_despawn_actor(actor.as_str(), *style, body);
        }
        QuestEffect::MoveActor {
            actor, to_anchor, ..
        } => {
            body.push(format!(
                "function {ns}:{}",
                moveactor_fn(actor.as_str(), to_anchor.as_str())
            ));
        }
        QuestEffect::UnleashActor { actor } => {
            body.push(format!(
                "function {ns}:unleash_{}",
                plan::safe_local(actor.as_str())
            ));
        }
        QuestEffect::Sequence { steps } => {
            body.push(format!("function {ns}:{}", sequence_fn(steps)));
        }
        QuestEffect::SpawnNpc { npc } => {
            body.push(format!("function {ns}:{}", spawn_npc_fn(npc.as_str())));
        }
    }
}

/// Emit a `despawn-actor` inline (spec-0014). Both styles target the actor body tag
/// `dw_actor_<id>` (so a puppet **or** an unleashed twin is removed — re-caging is
/// despawn + spawn). `kill` plays the vanilla death animation in place; `vanish`
/// relocates the (Silent) body far below the floor first, so the death sequence
/// plays entirely out of the players' view — a silent removal from two intended
/// primitives (tp + kill).
fn emit_despawn_actor(actor: &str, style: delvewright_dsl::DespawnStyle, body: &mut Vec<String>) {
    use delvewright_dsl::DespawnStyle;
    let safe = plan::safe_local(actor);
    match style {
        DespawnStyle::Kill => body.push(format!("kill @e[tag=dw_actor_{safe}]")),
        DespawnStyle::Vanish => {
            body.push(format!("tp @e[tag=dw_actor_{safe}] ~ -128 ~"));
            body.push(format!("kill @e[tag=dw_actor_{safe}]"));
        }
    }
}

/// Emit a `play-sound` effect (DSL v0.6). Effects run in an `as @a` context, so
/// `@s` is each player: an anchor sound plays once per player positioned at the
/// resolved anchor (all hear it there); the default `players` target plays at each
/// player's own position (`~ ~ ~`). `at: actor` never reaches emission — it is
/// rejected at validate-time (`DW0335`) until the actors surface lands.
fn emit_play_sound(
    plan: &Plan,
    sound: &str,
    at: Option<&delvewright_dsl::SoundAt>,
    volume: Option<f64>,
    pitch: Option<f64>,
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
    let mut cmd = format!("playsound {sound} master @s");
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
    body.push(cmd);
}

/// Emit a `damage-players` effect (DSL v0.6). Effects run in an `as @a` / `as @s`
/// context, so `@s` is each acting player: `damage @s <amount> <type>` damages
/// every acting player once (in a stealth `on_caught`, the single caught player).
/// `amount` is in half-hearts (1 HP each); the type is a curated vanilla damage
/// type (default `minecraft:generic`). A `within` box narrows to acting players
/// standing inside the anchor-centred AABB — the same `@s[x=…,dx=…]` box model the
/// stealth zone check uses — preserving the per-`@s` semantics (no double-hit).
///
/// Every form is guarded by `tag=!dw_cutscene`: a player watching a cutscene is
/// never harmed by campaign machinery (see [`CUTSCENE_TAG`]).
fn emit_damage_players(
    plan: &Plan,
    amount: u32,
    within: Option<&delvewright_dsl::StealthZone>,
    damage_type: Option<delvewright_dsl::DamageKind>,
    body: &mut Vec<String>,
) {
    use delvewright_dsl::DamageKind;
    let kind = damage_type.unwrap_or(DamageKind::Generic).id();
    let cmd = format!("damage @s {amount} {kind}");
    match within {
        Some(zone) => {
            // A blank box when the anchor is unresolved (referential validation
            // reports that, DW0142) — emit nothing rather than an invalid selector.
            if let Some(pos) = anchor_point_any(plan, zone.anchor.as_str()) {
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
                body.push(format!(
                    "execute if entity @s[x={},dx={},y={},dy={},z={},dz={},tag=!{CUTSCENE_TAG}] run {cmd}",
                    lo[0], size[0], lo[1], size[1], lo[2], size[2]
                ));
            }
        }
        // The bare form still needs the cutscene guard, so it becomes a guarded
        // `execute` too (see CUTSCENE_TAG: a cutscene is pure observation —
        // campaign machinery never harms a player who is only watching).
        None => body.push(format!(
            "execute if entity @s[tag=!{CUTSCENE_TAG}] run {cmd}"
        )),
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
        if plan.any_checkpoint_on_respawn() {
            let idx = plan
                .checkpoint_for(anchor, on_respawn)
                .map(|c| c.index)
                .unwrap_or(0);
            body.push(format!("scoreboard players set #cp dw.sys {idx}"));
        }
    }
}

/// Generate the checkpoint respawn-dispatch functions (DSL v0.6, spec-0012).
/// Empty unless some checkpoint carries an `on_respawn` hook. Respawn is detected
/// via the vanilla `deathCount` criterion: a player whose death total exceeds the
/// acknowledged total respawned since last tick; the active checkpoint (`#cp`)
/// selects which per-player `on_respawn` list runs. Effects are emitted in
/// declared (deterministic) order and are expected to be idempotent (spec-0012).
fn emit_checkpoint_functions(plan: &Plan) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut fns: Vec<(String, String)> = Vec::new();
    if !plan.any_checkpoint_on_respawn() {
        return fns;
    }
    // cp_respawn_check (as @s): fire on the death-count edge, then acknowledge.
    fns.push((
        "cp_respawn_check".to_string(),
        lines(&[
            format!(
                "execute if score @s dw.deaths > @s dw.death_ack run function {ns}:cp_respawn_fire"
            ),
            "scoreboard players operation @s dw.death_ack = @s dw.deaths".to_string(),
        ]),
    ));
    // cp_respawn_fire (as @s): dispatch on the active checkpoint.
    let mut fire: Vec<String> = Vec::new();
    for c in &plan.checkpoints {
        if c.on_respawn.is_empty() {
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
        if c.on_respawn.is_empty() {
            continue;
        }
        let mut body: Vec<String> = Vec::new();
        for eff in &c.on_respawn {
            emit_quest_effect(plan, eff, &mut body);
        }
        fns.push((format!("cp_on_respawn_{}", c.index), lines(&body)));
    }
    fns
}

/// Generate the stealth-beat functions (DSL v0.6, spec-0014; sneak requirement
/// removed by owner ruling 2026-08-01 — holding sneak collided with the
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
        let mut caught: Vec<String> = vec!["scoreboard players set @s dw.st_grace 0".to_string()];
        for eff in &beat.on_caught {
            emit_quest_effect(plan, eff, &mut caught);
        }
        fns.push((format!("stealth_caught_{i}"), lines(&caught)));
    }
    fns
}

/// Emit a `narrate` line in its channel (DSL v0.4). `chat` = `tellraw`; `title`
/// / `subtitle` = the vanilla `title` command (a subtitle is paired with a blank
/// title so it renders on its own). An optional sound plays alongside.
fn emit_narrate(
    text: &str,
    style: Option<delvewright_dsl::NarrateStyle>,
    sound: Option<&str>,
    body: &mut Vec<String>,
) {
    use delvewright_dsl::NarrateStyle;
    let comp = json!({ "text": text });
    match style.unwrap_or(NarrateStyle::Chat) {
        NarrateStyle::Chat => body.push(format!("tellraw @s {comp}")),
        NarrateStyle::Title => body.push(format!("title @s title {comp}")),
        NarrateStyle::Subtitle => {
            body.push(format!("title @s title {}", json!({ "text": " " })));
            body.push(format!("title @s subtitle {comp}"));
        }
        // Large-glyph "art" title through the delve's custom resource-pack font
        // (`delve:art`, DSL v0.6). The font is uppercase-only (glyph coverage is
        // checked at compile time, DW0328), so render uppercase.
        NarrateStyle::Art => {
            let art = json!({ "text": text.to_ascii_uppercase(), "font": "delve:art" });
            body.push(format!("title @s title {art}"));
        }
    }
    if let Some(s) = sound {
        body.push(format!("playsound {s} player @s"));
    }
}

/// Resolve an anchor name to a world point by scanning every area (first match),
/// mirroring how `open-gate` resolves its anchor. `None` if unresolved.
fn anchor_point_any(plan: &Plan, anchor: &str) -> Option<[i32; 3]> {
    for ((_, name), resolved) in &plan.anchors {
        if name == anchor {
            return match resolved {
                ResolvedAnchor::Point { pos, .. } => Some(*pos),
                ResolvedAnchor::Gate { from, .. } => Some(*from),
            };
        }
    }
    None
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
            snbt_string(name)
        } else {
            let cname = json!({ "text": name }).to_string().replace('\'', "\\'");
            format!("'{cname}'")
        };
        out.push(format!(
            "summon {base} {} {} {} {{NoAI:1b,Invulnerable:1b,Silent:1b,PersistenceRequired:1b,NoGravity:1b,Rotation:[{yaw}f,0f],Tags:[\"dw_npc\",\"{}\"],CustomName:{},CustomNameVisible:1b,VillagerData:{{profession:\"minecraft:none\",type:\"minecraft:plains\",level:1}}}}",
            p[0], p[1], p[2], npc.tag, cname_field
        ));
    }
    // The interaction hitbox also carries the tag of every `strike` trigger
    // watching this NPC's anchor — see `strike_trigger_tags_at`.
    let mut tags = vec![npc.tag.clone()];
    tags.extend(strike_trigger_tags_at(c, anchor));
    let tag_list = tags
        .iter()
        .map(|t| format!("\"{t}\""))
        .collect::<Vec<_>>()
        .join(",");
    out.push(format!(
        "summon minecraft:interaction {} {} {} {{width:1.0f,height:2.0f,response:1b,Invulnerable:1b,Tags:[{tag_list}]}}",
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
/// The first `(strike trigger, npc id, npc entity tag)` triple whose trigger
/// anchor is also an NPC's stand anchor — the collision
/// [`strike_trigger_tags_at`] resolves. Campaign order (deterministic); `None`
/// when no campaign NPC shares an anchor with a `strike` trigger.
fn first_strike_trigger_on_npc<'a>(
    plan: &'a Plan,
) -> Option<(&'a delvewright_dsl::EnvTrigger, String, String)> {
    use delvewright_dsl::TriggerOn;
    let c = plan.campaign;
    for t in &c.quests.content.triggers {
        if !matches!(t.on, TriggerOn::Strike) {
            continue;
        }
        for n in &plan.npcs {
            let anchor = c
                .npcs
                .content
                .npcs
                .iter()
                .find(|d| d.id.as_str() == n.npc_id)
                .map(|d| d.anchor.as_str());
            if anchor == Some(t.at.as_str()) {
                return Some((t, n.npc_id.clone(), n.tag.clone()));
            }
        }
    }
    None
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

fn strike_trigger_tags_at(c: &delvewright_dsl::Campaign, anchor: &str) -> Vec<String> {
    use delvewright_dsl::TriggerOn;
    if anchor.is_empty() {
        return Vec::new();
    }
    c.quests
        .content
        .triggers
        .iter()
        .filter(|t| matches!(t.on, TriggerOn::Strike) && t.at.as_str() == anchor)
        .map(|t| format!("dw_trig_{}", plan::safe_local(t.id.as_str())))
        .collect()
}

/// The generated function name for a `spawn-npc` effect (DSL v0.6).
fn spawn_npc_fn(npc: &str) -> String {
    format!("spawn_npc_{}", plan::safe_local(npc))
}

/// `spawn_npc_<id>` functions (DSL v0.6): one per **deferred** stage-2 NPC, the
/// scripted-entrance dual of `despawn-npc`. Emitted only for deferred NPCs, so a
/// campaign that declares none is byte-identical to pre-0.6.
///
/// Each of the two summons is **independently** idempotent, so a re-fired
/// `spawn-npc` never doubles an entity. Body and hitbox share the per-NPC id tag,
/// so the guards discriminate on the body-only `dw_npc` tag: the body is guarded by
/// `[tag=dw_npc,tag=<id>]`, the hitbox by its negation `[tag=<id>,tag=!dw_npc]` — a
/// single `unless entity @e[tag=<id>]` guard on both lines would let the body's own
/// summon suppress the hitbox.
fn spawn_npc_fns(plan: &Plan) -> Vec<(String, String)> {
    let c = plan.campaign;
    let v03 = campaign_is_v03(plan);
    let mut out = Vec::new();
    for npc in &plan.npcs {
        if !npc_is_deferred(c, &npc.npc_id) {
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
fn movenpc_fn(npc: &str, to_anchor: &str) -> String {
    format!(
        "mv_{}_{}",
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
                    Some(delvewright_dsl::CameraSubject::Anchor { anchor, .. }) => anchor.as_str(),
                    Some(delvewright_dsl::CameraSubject::Npc { npc, .. }) => npc.as_str(),
                    Some(delvewright_dsl::CameraSubject::Actor { actor, .. }) => actor.as_str(),
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

/// A `[scores={dw.f_a=1..,…}]` selector fragment for a flag list, or `""`.
fn flag_scores_selector(flags: &[delvewright_dsl::FlagId]) -> String {
    if flags.is_empty() {
        return String::new();
    }
    let inner = flags
        .iter()
        .map(|f| format!("{}=1..", plan::flag_score(f.as_str())))
        .collect::<Vec<_>>()
        .join(",");
    format!("[scores={{{inner}}}]")
}

/// Every quest effect in the campaign (objective-complete, quest-complete, and
/// trigger effects), flattened through `sequence` steps and `move-actor` `on_arrive`
/// (spec-0014) so nested lifecycle/cutscene/actor targets are collected. Pre-0.6
/// campaigns have no nesting, so this equals the shallow list (byte-identical).
fn all_campaign_effects(c: &delvewright_dsl::Campaign) -> Vec<&QuestEffect> {
    let mut out = Vec::new();
    for q in &c.quests.content.quests {
        for e in q
            .on_objective_complete
            .values()
            .flatten()
            .chain(&q.on_complete)
        {
            push_effect_deep(e, &mut out);
        }
    }
    for t in &c.quests.content.triggers {
        for e in &t.effects {
            push_effect_deep(e, &mut out);
        }
    }
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
fn movenpc_bare(npc: &str, to_anchor: &str) -> String {
    movenpc_fn(npc, to_anchor)
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
/// a campaign with no moves (v0.2/v0.3 stay byte-identical).
///
/// An `on_arrive` bundle (DSL v0.6, parity with `move-actor`) fires on the
/// driver's **final-waypoint tick** — exactly the arrival detection `ma_tick`
/// uses — via a generated `mv_arrive_<key>` function. A bare `move-npc` emits no
/// arrive hook and stays byte-identical to pre-0.6 output.
fn movenpc_fns(plan: &Plan, moves: &[crate::nav::MovePlan]) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for m in moves {
        let start_name = movenpc_fn(&m.npc, &m.to_anchor);
        let bare = movenpc_bare(&m.npc, &m.to_anchor);
        let safe = plan::safe_local(&m.npc);
        let total = m.ticks();
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

        // start: guard re-entry, reset the tick counter, schedule the driver.
        let start = vec![
            format!("execute if score #mrun_{bare} dw.sys matches 1 run return fail"),
            format!("scoreboard players set #mrun_{bare} dw.sys 1"),
            format!("scoreboard players set #mt_{bare} dw.sys 0"),
            format!("schedule function {ns}:mv_tick_{bare} 1t"),
        ];
        out.push((start_name, lines(&start)));

        // per-tick driver: tp both body + hitbox to waypoint[t], advance, and
        // reschedule until the path is walked; the final waypoint is the target.
        let mut tick: Vec<String> = Vec::new();
        for (t, w) in m.waypoints.iter().enumerate() {
            tick.push(format!(
                "execute if score #mt_{bare} dw.sys matches {t} run tp @e[tag=dw_npc_{safe}] {} {} {}",
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
            // `schedule`, so there is no `@s` (see `Executor`).
            let arrive = emit_effect_bundle(plan, on_arrive, Executor::Server);
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
fn actor_puppet_summon(a: &delvewright_dsl::Actor, pos: [i32; 3], yaw: i32) -> String {
    let safe = plan::safe_local(a.id.as_str());
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
            .map(|n| format!(",CustomName:{},CustomNameVisible:1b", snbt_string(n)))
            .unwrap_or_default();
        let attrs = if a.vulnerable {
            ",attributes:[{id:\"minecraft:knockback_resistance\",base:1.0}]"
        } else {
            ""
        };
        format!(
            "summon {} {} {} {} {{NoAI:1b,Silent:1b,PersistenceRequired:1b,NoGravity:1b,Invulnerable:{inv}b,DeathLootTable:\"minecraft:empty\",Rotation:[{yaw}f,0f],{tags}{name}{attrs}}}",
            a.entity, p[0], p[1], p[2]
        )
    }
}

/// The `/summon` command (relative coords, run `execute at` the puppet) for an
/// actor's real-AI twin (spec-0014 `unleash`): the real `entity` with AI enabled,
/// same name and body tag (`dw_actor` + `dw_actor_<id>`), but **no** `dw_pup_<id>`
/// marker — so killing the puppet by its marker leaves the twin fighting.
fn actor_twin_summon(a: &delvewright_dsl::Actor) -> String {
    let safe = plan::safe_local(a.id.as_str());
    let name = a
        .name
        .as_deref()
        .map(|n| format!(",CustomName:{},CustomNameVisible:1b", snbt_string(n)))
        .unwrap_or_default();
    format!(
        "summon {} ~ ~ ~ {{PersistenceRequired:1b,DeathLootTable:\"minecraft:empty\",Tags:[\"dw_actor\",\"dw_actor_{safe}\"]{name}}}",
        a.entity
    )
}

/// The generated start-function name for a `move-actor` (content key).
fn moveactor_fn(actor: &str, to_anchor: &str) -> String {
    format!(
        "ma_{}_{}",
        plan::safe_local(actor),
        plan::safe_local(to_anchor)
    )
}

/// The scoreboard-safe suffix shared by a move-actor's driver functions/sentinels.
fn moveactor_bare(actor: &str, to_anchor: &str) -> String {
    moveactor_fn(actor, to_anchor)
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

/// Actor staging functions (spec-0014): a `spawn_actor_<id>` (idempotent summon) and
/// `unleash_<id>` (puppet → real-AI twin) per declared actor, plus a per-tick
/// teleport driver (with tangent yaw and an `on_arrive` bundle) per planned
/// `move-actor`. Empty for a campaign with no actors (pre-0.6 byte-identical).
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
                actor_puppet_summon(a, pos, yaw)
            )]),
        ));
        out.push((
            format!("unleash_{safe}"),
            lines(&[
                format!(
                    "execute at @e[tag=dw_pup_{safe},limit=1] run {}",
                    actor_twin_summon(a)
                ),
                format!("kill @e[tag=dw_pup_{safe}]"),
            ]),
        ));
    }
    // move-actor per-tick drivers.
    for m in actor_moves {
        let safe = plan::safe_local(&m.actor);
        let bare = moveactor_bare(&m.actor, &m.to_anchor);
        let total = m.ticks();
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

        let start = vec![
            format!("execute if score #arun_{bare} dw.sys matches 1 run return fail"),
            format!("scoreboard players set #arun_{bare} dw.sys 1"),
            format!("scoreboard players set #at_{bare} dw.sys 0"),
            format!("schedule function {ns}:ma_tick_{bare} 1t"),
        ];
        out.push((moveactor_fn(&m.actor, &m.to_anchor), lines(&start)));

        let mut tick: Vec<String> = Vec::new();
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
            // Server command source (see `Executor`): `ma_tick_<bare>` runs from
            // the scheduler, so `@s` is unbound in everything it calls.
            let arrive = emit_effect_bundle(plan, on_arrive, Executor::Server);
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
            let b = emit_effect_bundle(plan, &step.effects, Executor::Server);
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

/// Cutscene functions (spec-0008 addendum; keyframe dolly per task #64): the
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
/// **Path timing** (task #64): the dolly is arc-length parameterized (equal
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
        start.push(format!("scoreboard players set #t_{bare} dw.sys 0"));
        start.push(format!("scoreboard players set #p_{bare} dw.sys 1"));
        start.push(format!(
            "execute at @p run summon minecraft:marker ~ ~ ~ {{Tags:[\"dw_csmark_{bare}\"]}}"
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

        // Keyframe driver (task #64): every shot's keyframes laid end-to-end on
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
        // only — no sneak stat since the 2026-08-01 ruling) needs no re-sync;
        // grace is deliberately NOT reset — it neither accrued nor expired
        // during the cutscene, so the beat picks up exactly where it paused.
        end.push(format!("tag @a remove {CUTSCENE_TAG}"));
        end.push(format!("scoreboard players set #run_{bare} dw.sys 0"));
        out.push((format!("cs_end_{bare}"), lines(&end)));
    }
    out
}

/// Environment-trigger interaction-entity summons (strike/use) for
/// `setup_finish`. Approach triggers need no entity. Empty for a campaign with no
/// triggers (byte-identical v0.2/v0.3).
///
/// A `strike` trigger on an NPC's stand anchor gets **no entity of its own**:
/// the NPC's interaction hitbox is the trigger's sole carrier
/// ([`strike_trigger_tags_at`]). Emitting a second, exactly co-located hitbox
/// here made the vanilla client's entity ray-pick ambiguous — an exact tie
/// resolves to whichever entity the pick iterates first, in practice this
/// world-init summon — so every right-click landed on an entity without the
/// `dw_npc_<n>` tag and the `player_interacted_with_entity` dialogue
/// advancement never fired (round-6 island QA: after the boulder seal,
/// Polyphemus could not be talked to at all). One cell, one hitbox. The
/// trigger's lifecycle therefore follows the NPC's presence — which is also
/// its meaning: the thing being struck is the NPC.
fn env_trigger_setup(plan: &Plan) -> Vec<String> {
    use delvewright_dsl::TriggerOn;
    let mut out = Vec::new();
    for t in &plan.campaign.quests.content.triggers {
        if matches!(t.on, TriggerOn::Approach { .. }) {
            continue;
        }
        if matches!(t.on, TriggerOn::Strike) && npc_stands_at(plan, t.at.as_str()) {
            continue;
        }
        if let Some(p) = anchor_point_any(plan, t.at.as_str()) {
            let q = ent_xyz(p);
            out.push(format!(
                "summon minecraft:interaction {} {} {} {{width:1.0f,height:2.0f,response:1b,Invulnerable:1b,Tags:[\"dw_trig_{}\"]}}",
                q[0], q[1], q[2], plan::safe_local(t.id.as_str())
            ));
        }
    }
    out
}

/// Environment-trigger per-tick checks for the `tick` function. Empty for a
/// campaign with no triggers.
fn env_trigger_tick(plan: &Plan) -> Vec<String> {
    use delvewright_dsl::TriggerOn;
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for t in &plan.campaign.quests.content.triggers {
        let id = plan::safe_local(t.id.as_str());
        let once_guard = if t.once {
            format!("unless score #trig_{id} dw.sys matches 1 ")
        } else {
            String::new()
        };
        let fsel = flag_scores_selector(&t.requires_flags);
        // v0.6 negative gate: the trigger is suppressed while ANY listed flag is
        // set by ANY player (flags are campaign state; the wake beat stands the
        // retaliation trigger down for everyone). `unless entity @a[scores=…]`
        // is unset-safe: a positive `=1..` selector inside a negation matches
        // only really-set flags, so players with no score never suppress.
        let forbid_guard: String = t
            .forbids_flags
            .iter()
            .map(|f| {
                format!(
                    "unless entity @a[scores={{{}=1..}}] ",
                    plan::flag_score(f.as_str())
                )
            })
            .collect();
        match &t.on {
            TriggerOn::Strike | TriggerOn::Use => {
                let (rec, tag) = match t.on {
                    TriggerOn::Strike => ("attack", "dw_trig"),
                    _ => ("interaction", "dw_trig"),
                };
                let _ = tag;
                // Fire when the interaction entity has recorded the event and (if
                // gated) some player holds the flags; then clear the record.
                let flag_cond = if fsel.is_empty() {
                    String::new()
                } else {
                    format!("if entity @a{fsel} ")
                };
                out.push(format!(
                    "execute {once_guard}{forbid_guard}if entity @e[tag=dw_trig_{id},nbt={{{rec}:{{}}}}] {flag_cond}run function {ns}:trig_{id}"
                ));
                out.push(format!(
                    "execute as @e[tag=dw_trig_{id}] run data remove entity @s {rec}"
                ));
            }
            TriggerOn::Approach { range } => {
                if let Some(p) = anchor_point_any(plan, t.at.as_str()) {
                    out.push(format!(
                        "execute {once_guard}{forbid_guard}positioned {} {} {} if entity @a[distance=..{range}{}] run function {ns}:trig_{id}",
                        p[0], p[1], p[2],
                        if fsel.is_empty() {
                            String::new()
                        } else {
                            // merge the flag scores into the distance selector.
                            t.requires_flags
                                .iter()
                                .map(|f| format!(",{}=1..", plan::flag_score(f.as_str())))
                                .collect::<String>()
                        }
                    ));
                }
            }
        }
    }
    out
}

/// Environment-trigger effect functions (`trig_<id>`). Effects run `as @a` (only
/// players holding the trigger's flags) so `@s`-scoped effects resolve; `once`
/// sets a global sentinel so the trigger fires at most once.
fn env_trigger_fns(plan: &Plan) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for t in &plan.campaign.quests.content.triggers {
        let id = plan::safe_local(t.id.as_str());
        let mut body: Vec<String> = Vec::new();
        if t.once {
            body.push(format!("scoreboard players set #trig_{id} dw.sys 1"));
        }
        let mut effs: Vec<String> = Vec::new();
        for e in &t.effects {
            emit_gated_effect(plan, e, &mut effs);
        }
        let sel = flag_scores_selector(&t.requires_flags);
        for line in effs {
            body.push(format!("execute as @a{sel} run {line}"));
        }
        out.push((format!("trig_{id}"), lines(&body)));
    }
    out
}

// ---------------------------------------------------------------------------
// v0.6 traps (spec-0011)
// ---------------------------------------------------------------------------

/// `setup_finish` commands for traps (spec-0011): fill each `dispense` trap's
/// prefab dispenser with its static payload (`item replace block … container.0`,
/// the same deterministic mechanism as a `collect` chest — no raw NBT), and summon
/// the disarm affordance's interaction entity. The trap's *harm* needs no command:
/// the plate/tripwire/trapped-chest → dispenser redstone is already in the prefab.
/// Empty for a campaign with no traps → byte-identical.
fn trap_setup(plan: &Plan) -> Vec<String> {
    let mut out = Vec::new();
    for t in &plan.traps {
        // Fill the pre-wired dispenser with the declared payload.
        if let (Some(disp), Some((item, count))) = (t.dispenser, &t.payload) {
            out.push(format!(
                "item replace block {} {} {} container.0 with {item} {count}",
                disp[0], disp[1], disp[2]
            ));
        }
        // Summon the disarm interaction affordance (a right-click target). The
        // physical lever may also be in the prefab; this entity is the modeled,
        // provable disarm the compiler owns.
        if let Some(dis) = &t.disarm {
            let v = ent_xyz(dis.via_cell);
            out.push(format!(
                "summon minecraft:interaction {} {} {} {{width:1.0f,height:2.0f,response:1b,Invulnerable:1b,Tags:[\"dw_trapdis_{}\"]}}",
                v[0], v[1], v[2], t.safe
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
    out
}

/// Disarm functions (`trap_disarm_<id>`) for disarmable traps (spec-0011). Firing
/// once (`#trapdis_<id>` sentinel): set the disarm flag party-wide (so
/// `requires_flags` reads elsewhere see it) and **empty the dispenser** — the
/// modeled, global disarm that actually stops a redstone-native dispense trap for
/// everyone. Empty for a campaign with no disarmable traps.
fn trap_fns(plan: &Plan) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for t in &plan.traps {
        let Some(dis) = &t.disarm else {
            continue;
        };
        let id = &t.safe;
        let mut body: Vec<String> = Vec::new();
        body.push(format!("scoreboard players set #trapdis_{id} dw.sys 1"));
        body.push(format!(
            "scoreboard players set @a {} 1",
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
        out.push((format!("trap_disarm_{id}"), lines(&body)));
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
const BOUNDARY_DEFAULT_MESSAGE: &str = "The tide turns you back — the delve lies behind you.";

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
fn boundary_message(plan: &Plan) -> String {
    plan.campaign
        .world
        .content
        .boundary
        .as_ref()
        .and_then(|b| b.message.as_deref())
        .unwrap_or(BOUNDARY_DEFAULT_MESSAGE)
        .to_string()
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
        gives.push(format!(
            "effect give @a[x={},dx={},y={},dy={},z={},dz={}] minecraft:night_vision {NIGHT_VISION_SECONDS} 0 true",
            min[0],
            max[0] - min[0] + 1,
            min[1],
            max[1] - min[1] + 1,
            min[2],
            max[2] - min[2] + 1,
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
fn boundary_fns(plan: &Plan) -> Vec<(String, String)> {
    let Some(region) = playable_region(plan) else {
        return Vec::new();
    };
    let ns = &plan.namespace;
    let sel = region.inside_selector();
    let msg = json!({ "text": boundary_message(plan) });

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
            item,
            count,
            anchor,
            ..
        } => {
            if let Some(pos) = plan.point(area, anchor.as_str()) {
                cmds.push(format!(
                    "setblock {} {} {} minecraft:chest",
                    pos[0], pos[1], pos[2]
                ));
                cmds.push(format!(
                    "item replace block {} {} {} container.0 with {} {}",
                    pos[0], pos[1], pos[2], item, count
                ));
            }
        }
        Objective::Interact { id, anchor, .. } => {
            if let Some(pos) = plan.point(area, anchor.as_str()) {
                let e = ent_xyz(pos);
                cmds.push(format!(
                    "summon minecraft:interaction {} {} {} {{width:1.0f,height:2.0f,response:1b,Invulnerable:1b,Tags:[\"{}\"]}}",
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
                        "summon minecraft:item_display {} {} {} {{Glowing:1b,Tags:[\"dw_marker\",\"{}\"],{}billboard:\"center\",item:{{id:\"minecraft:lantern\",count:1}}}}",
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
                "summon minecraft:item_display {} {} {} {{Glowing:1b,Tags:[\"dw_marker\",\"{}\"],{}billboard:\"center\",item:{{id:\"minecraft:end_rod\",count:1}}}}",
                e[0], e[1], e[2], reach_marker_tag(id.as_str()), name_fields
            ));
        }
        Objective::TalkTo { .. } | Objective::Kill { .. } => {}
    }
    cmds
}

/// The despawn commands run when an objective COMPLETES (task #45): remove every
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
fn declared_flags(c: &delvewright_dsl::Campaign) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    // Descend the whole effect tree: a `set-flag` nested in a `sequence` step (or an
    // `on_respawn`/`on_caught`/`on_arrive` bundle) still emits a `dw.f_<flag>` write,
    // so its scoreboard objective must be initialized here — else the nested
    // `set-flag` writes to an uninitialized objective at runtime.
    let note = |eff: &QuestEffect, out: &mut std::collections::BTreeSet<String>| {
        eff.visit_deep(&mut |e| {
            if let Some(f) = e.set_flag() {
                out.insert(f.as_str().to_string());
            }
        });
    };
    for q in &c.quests.content.quests {
        for eff in q
            .on_objective_complete
            .values()
            .flatten()
            .chain(&q.on_complete)
        {
            note(eff, &mut out);
        }
    }
    for t in &c.quests.content.triggers {
        for eff in &t.effects {
            note(eff, &mut out);
        }
    }
    // v0.6 traps (spec-0011): a disarm's `sets_flag` needs its own scoreboard.
    for t in &c.quests.content.traps {
        if let Some(dis) = &t.disarm {
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
/// Returns the ` if …`/` unless …` fragment (leading space); callers prepend
/// `execute as @a` and append the type-specific condition + `run`. For a v0.2
/// objective (no `after`/flags) this is exactly the pre-v0.3 reach guard,
/// keeping keep-crawl byte-identical.
fn pending_guard(o: &Objective, quest_active: &str) -> String {
    let mut g = format!(" if score @s {quest_active} matches 1");
    for a in o.after() {
        g.push_str(&format!(" if score @s {} matches 1", obj_score(a.as_str())));
    }
    for f in o.requires_flags() {
        g.push_str(&format!(
            " if score @s {} matches 1",
            plan::flag_score(f.as_str())
        ));
    }
    for f in o.forbids_flags() {
        g.push_str(&format!(
            " unless score @s {} matches 1",
            plan::flag_score(f.as_str())
        ));
    }
    g.push_str(&format!(
        " unless score @s {} matches 1",
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
fn option_display_conditions(c: &delvewright_dsl::Campaign, opt: &plan::OptionPlan) -> String {
    let mut cond = String::new();
    for f in &opt.requires_flags {
        cond.push_str(&format!(" if score @s {} matches 1", plan::flag_score(f)));
    }
    // v0.6 negative gate: hidden once any forbidden flag is set (`unless …
    // matches 1` treats an unset score as "not set").
    for f in &opt.forbids_flags {
        cond.push_str(&format!(
            " unless score @s {} matches 1",
            plan::flag_score(f)
        ));
    }
    for obj in &opt.completes {
        if let Some((qid, _)) = objective_quest(c, obj) {
            cond.push_str(&format!(
                " if score @s {} matches 1 unless score @s {} matches 1",
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
                option_display_conditions(c, g),
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

fn emit_dialogs(plan: &Plan) -> Vec<(String, Value)> {
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
                "label": class.name,
                "tooltip": class.blurb,
                "action": { "type": "minecraft:run_command", "command": format!("/trigger dw.class set {}", cp.n) }
            })
        })
        .collect();
    dialogs.push((
        "class_select".to_string(),
        json!({
            "type": "minecraft:multi_action",
            "title": "Choose your class",
            "body": [{ "type": "minecraft:plain_message", "contents": "Pick the kit you will carry." }],
            "columns": 1,
            "can_close_with_escape": false,
            "after_action": "close",
            "actions": actions
        }),
    ));

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
fn build_node_dialog(
    npc_name: &str,
    text: &str,
    opts: &[&plan::OptionPlan],
    trigger_objective: &str,
) -> Value {
    if opts.is_empty() {
        json!({
            "type": "minecraft:notice",
            "title": npc_name,
            "body": [{ "type": "minecraft:plain_message", "contents": text }],
            "can_close_with_escape": true
        })
    } else {
        let actions: Vec<Value> = opts
            .iter()
            .map(|o| {
                json!({
                    "label": o.label,
                    "action": { "type": "minecraft:run_command", "command": format!("/trigger {trigger_objective} set {}", o.n) }
                })
            })
            .collect();
        json!({
            "type": "minecraft:multi_action",
            "title": npc_name,
            "body": [{ "type": "minecraft:plain_message", "contents": text }],
            "columns": 1,
            "can_close_with_escape": true,
            "after_action": "close",
            "actions": actions
        })
    }
}

fn emit_advancements(plan: &Plan) -> Vec<(String, Value)> {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let mut advs = Vec::new();

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

    // campaign-complete advancement (granted by command)
    advs.push((
        "campaign_complete".to_string(),
        json!({
            "criteria": { "granted": { "trigger": "minecraft:impossible" } },
            "display": {
                "icon": { "id": "minecraft:iron_door" },
                "title": c.world.content.title,
                "description": "You left the keep.",
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
    let (pin, sel) = pin_dummy("dw_t_camp");
    let mut body: Vec<String> = Vec::new();
    body.push(format!(
        "#> {}: objective completions set {comp_obj} (Delvewright mechanism test)",
        c.world.content.title
    ));
    body.push("# @dummy".to_string());
    body.push("# @timeout 100".to_string());
    body.push(String::new());
    body.push(format!("function {ns}:setup"));
    // Pin this test's own dummy and drive the whole chain on it alone (see
    // `pin_dummy`): `@a`-wide quest/objective writes would land on every
    // sibling test's dummy in the batch, and the closing `@p` assert could read
    // a foreign one.
    body.push(pin);
    // Actively establish the asserted baseline — on the shared-batch server
    // "never set" is not 0.
    body.push(format!("scoreboard players set {sel} {comp_obj} 0"));
    for qid in campaign_start_quests(c) {
        body.push(format!(
            "scoreboard players set {sel} {} 1",
            quest_active_score(qid)
        ));
    }
    for q in &c.quests.content.quests {
        for o in &q.objectives {
            body.push(format!(
                "execute as {sel} run function {ns}:complete_{}",
                safe_obj_fn(o.id().as_str())
            ));
        }
    }
    // `assert score` requires a single-entity selector (the pinned dummy).
    body.push(format!("assert score {sel} {comp_obj} matches {comp_val}"));

    out.insert(
        format!("packtest-datapack/data/{ns}/test/campaign.mcfunction"),
        lines(&body).into_bytes(),
    );

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
        c.world.content.title
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

    // v0.3: one focused mechanism test per gameplay verb present in the campaign,
    // plus a flag-gate test. Each drives the compiler-generated mechanic functions
    // on a dummy player (no real combat / advancement events needed) and asserts
    // the objective scoreboard. Emits nothing for a v0.2 campaign.
    emit_verb_packtests(plan, out);

    // The dialogue trigger must survive a second use with NO tick in between —
    // the singleplayer pause-freeze contract. Emits nothing for a campaign with no
    // terminal dialogue option.
    emit_dialogue_trigger_packtest(plan, out);

    // v0.4: prop-on-activation, despawn removes body+hitbox, move arrives at
    // target. Emits nothing when the campaign uses none of them.
    emit_v04_packtests(plan, out, moves);

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

    // The scheduled-executor contract (AUDIT-P0): a function reached through
    // `schedule` still lands per-player state on real players.
    emit_scheduled_executor_packtests(plan, out, moves);
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
///    emitter ([`emit_effect_bundle`] with [`Executor::Server`]) over a
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
    let title = &plan.campaign.world.content.title;
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
        }],
        Executor::Server,
    );
    out.insert(
        format!("packtest-datapack/data/{ns}/function/{SCHEDULED_PROBE}.mcfunction"),
        lines(&probe).into_bytes(),
    );
    let (pin, sel) = pin_dummy("dw_t_sexec");
    let mut t = packtest_header(&format!(
        "{title}: a SCHEDULED function still reaches players (scheduled-executor contract)"
    ));
    t.push(format!("function {ns}:setup"));
    t.push(pin);
    // Own init: the probe objective is test-only, so this template creates it and
    // clears its own dummy (never assume 0 on the shared batch server).
    t.push(format!("scoreboard objectives add {probe_score} dummy"));
    t.push(format!("scoreboard players set {sel} {probe_score} 0"));
    // The real scheduler, the real emitted bundle. Not an inline call: an inline
    // call would run as this test's dummy and pass even with the bug present.
    t.push(format!("schedule function {ns}:{SCHEDULED_PROBE} 2t"));
    t.push(format!("await score {sel} {probe_score} matches 1"));
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
    let bare = movenpc_bare(&m.npc, &m.to_anchor);
    let score = plan::flag_score(&flag);
    let (pin, sel) = pin_dummy("dw_t_sarr");
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
    t.push(pin);
    // Own init: clear the flag on this test's dummy, and release the driver's
    // re-entry latch (a sibling template may have left it armed).
    t.push(format!("scoreboard players set {sel} {score} 0"));
    t.push(format!("scoreboard players set #mrun_{bare} dw.sys 0"));
    // The REAL start function: it schedules `mv_tick_<bare>`, which walks itself
    // to the final waypoint and fires `mv_arrive_<bare>` — every hop through the
    // scheduler, with the server command source the bug hid behind. The dummy
    // stands still throughout; nothing here supplies it as an executor.
    t.push(format!(
        "function {ns}:{}",
        movenpc_fn(&m.npc, &m.to_anchor)
    ));
    t.push(format!("await score {sel} {score} matches 1"));
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
    let title = &plan.campaign.world.content.title;
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
    let title = &plan.campaign.world.content.title;

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

    let (pin, sel) = pin_dummy("dw_t_trap");
    let mut b = packtest_header(&format!(
        "{title}: trap `{}` loads its dispenser payload; disarm empties it (spec-0011)",
        t.id
    ));
    b.push(format!("function {ns}:setup"));
    // Pin this test's own dummy (see `pin_dummy`): the disarm flag is asserted
    // per-player, and it must be read off the player this test controls.
    b.push(pin);
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
        // → cannot fire) and the disarm flag is set — the trap is provably off. The
        // flag is actively cleared first: a sibling's `@a`-wide write could have
        // pre-set it, and "never set" is not 0 on the shared-batch server.
        b.push(format!(
            "scoreboard players set {sel} {} 0",
            plan::flag_score(&dis.sets_flag)
        ));
        b.push(format!("function {ns}:trap_disarm_{}", t.safe));
        b.push(format!(
            "execute store success score #tempty_trap dw.sys if data block {} {} {} Items[0]",
            disp[0], disp[1], disp[2]
        ));
        b.push("assert score #tempty_trap dw.sys matches 0".to_string());
        b.push(format!(
            "assert score {sel} {} matches 1",
            plan::flag_score(&dis.sets_flag)
        ));
    }
    out.insert(
        format!("packtest-datapack/data/{ns}/test/v06_trap.mcfunction"),
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
    let title = &plan.campaign.world.content.title;
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
fn emit_boundary_packtest(plan: &Plan, out: &mut BuildOutput) {
    let Some(region) = playable_region(plan) else {
        return;
    };
    let Some(spawn) = campaign_spawn(plan) else {
        return;
    };
    let ns = &plan.namespace;
    let title = &plan.campaign.world.content.title;
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

/// v0.6 PackTests (spec-0012 checkpoints, spec-0014 stealth). Fake players cannot
/// respawn synchronously within a plain mcfunction test, so these drive the
/// compiler-generated mechanics directly and assert their deterministic effects:
///
/// * **checkpoint**: applying the checkpoint's `spawnpoint @a` + `dw:cp pos`
///   mirror makes `storage dw:cp pos` read back the checkpoint cell — the
///   machine-checkable "last checkpoint" contract other features consume.
/// * **stealth** (zone-presence model, owner ruling 2026-08-01 — no sneak
///   requirement): the generated `stealth_eval_<i>` judge catches an exposed
///   (out-of-zone) player after `grace_ticks` and spares an in-zone one —
///   driven by teleporting the dummy in and out of the declared zone box.
fn emit_v06_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = &plan.campaign.world.content.title;

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
    for q in &c.quests.content.quests {
        for effs in q.on_objective_complete.values() {
            for eff in effs {
                scan(eff);
            }
        }
        for eff in &q.on_complete {
            scan(eff);
        }
    }
    for t in &c.quests.content.triggers {
        for eff in &t.effects {
            scan(eff);
        }
    }
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
            c.world.content.title
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
            c.world.content.title
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
            c.world.content.title
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
        let bare = moveactor_bare(&m.actor, &m.to_anchor);
        let total = m.ticks();
        let p = m.target;
        let mut b = packtest_header(&format!(
            "{}: move-actor walks its puppet to the destination cell",
            c.world.content.title
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
    // no block residue for a sibling (batch model, #140).
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
        let bare = moveactor_bare(&m.actor, &m.to_anchor);
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
            c.world.content.title
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
                    c.world.content.title, prop.block
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

    // interact-marker lifecycle (task #45): a completed interact objective leaves
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
                    c.world.content.title
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
    if let Some(npc) = c
        .quests
        .content
        .quests
        .iter()
        .flat_map(|q| {
            q.on_objective_complete
                .values()
                .flatten()
                .chain(&q.on_complete)
        })
        .chain(c.quests.content.triggers.iter().flat_map(|t| &t.effects))
        .find_map(|e| e.despawn_npc())
    {
        let safe = plan::safe_local(npc.as_str());
        let mut b = packtest_header(&format!(
            "{}: despawn-npc removes body + hitbox",
            c.world.content.title
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
            c.world.content.title,
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
            c.world.content.title
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
        let bare = movenpc_bare(&m.npc, &m.to_anchor);
        let total = m.ticks();
        let p = m.target;
        let mut b = packtest_header(&format!(
            "{}: move-npc walks to its target anchor",
            c.world.content.title
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
                        c.world.content.title
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

    // Dialogue display gating (task #54): a `completes` option is DISPLAYED iff
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
                c.world.content.title, under_test.label
            ));
            bt.push(format!("function {ns}:setup"));
            // Pin this test's own dummy (see `pin_dummy`): with one dummy PER
            // test coexisting on the batch server, an `as @a` mask run + copy
            // would read the LAST dummy the selector visits — a foreign one.
            bt.push(pin);
            let clear = |bt: &mut Vec<String>| {
                for s in &reset {
                    bt.push(format!("scoreboard players set {sel} {s} 0"));
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
            bt.push(format!("scoreboard players set {sel} {qa} 1"));
            assert_bit(&mut bt, b, true);
            // Phase C — objective complete: the option disappears again.
            bt.push(format!("scoreboard players set {sel} {os} 1"));
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
                        "scoreboard players set {sel} {} 1",
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

/// Lines that satisfy an objective's activation guard on `sel` (quest active, all
/// `after` prerequisites set, all `requires_flags` set, and any required item
/// given). With `with_flags: false` the flags are not merely omitted but actively
/// cleared: PackTest runs the whole suite as one batch on one shared server —
/// sibling templates legitimately set the same flag score on `@a` (every dummy),
/// so on this server "never set" does not mean 0.
fn packtest_preamble(quest_id: &str, o: &Objective, with_flags: bool, sel: &str) -> Vec<String> {
    let mut p = vec![format!(
        "scoreboard players set {sel} {} 1",
        quest_active_score(quest_id)
    )];
    for a in o.after() {
        p.push(format!(
            "scoreboard players set {sel} {} 1",
            obj_score(a.as_str())
        ));
    }
    for f in o.requires_flags() {
        p.push(format!(
            "scoreboard players set {sel} {} {}",
            plan::flag_score(f.as_str()),
            if with_flags { 1 } else { 0 }
        ));
    }
    // v0.6 negative gate: actively clear every forbidden flag so the objective
    // is not suppressed by a sibling template's leftover state (same batch-server
    // reasoning as the `with_flags: false` clearing above).
    for f in o.forbids_flags() {
        p.push(format!(
            "scoreboard players set {sel} {} 0",
            plan::flag_score(f.as_str())
        ));
    }
    match o {
        Objective::Collect { item, count, .. } => {
            p.push(format!("give {sel} {item} {count}"));
        }
        Objective::Interact {
            requires_item: Some(it),
            ..
        } => {
            p.push(format!("give {sel} {it} 1"));
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
    let mut first_interact = None;
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
                            w.mobs.iter().any(|m| default_mainhand(&m.entity).is_some())
                        })
                    {
                        first_armed_kill = Some((qid, o));
                    }
                }
                Objective::Collect { .. } if first_collect.is_none() => {
                    first_collect = Some((qid, o))
                }
                Objective::Interact { .. } if first_interact.is_none() => {
                    first_interact = Some((qid, o))
                }
                _ => {}
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
            c.world.content.title
        ));
        b.push(format!("function {ns}:setup"));
        // Pin this test's own dummy (see `pin_dummy`) and drive the whole chain
        // on it alone; actively zero the asserted objective first.
        b.push(pin);
        b.push(format!(
            "scoreboard players set {sel} {} 0",
            obj_score(id.as_str())
        ));
        b.extend(packtest_preamble(qid, o, true, &sel));
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
        if let Some(mob) = w
            .mobs
            .iter()
            .find(|m| default_mainhand(&m.entity).is_some())
        {
            let item = default_mainhand(&mob.entity).expect("filtered to armed mobs");
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
            "assert score {sel} {} matches 1",
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
            c.world.content.title
        ));
        b.push(format!("function {ns}:setup"));
        // Pin this test's own dummy (see `pin_dummy`) and drive/assert on it
        // alone; actively zero the asserted objective first.
        b.push(pin);
        b.push(format!(
            "scoreboard players set {sel} {} 0",
            obj_score(id.as_str())
        ));
        b.extend(packtest_preamble(qid, o, true, &sel));
        b.push(format!(
            "execute as {sel} run function {ns}:c_reward_{}",
            plan::safe_local(id.as_str())
        ));
        b.push(format!(
            "assert score {sel} {} matches 1",
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
            c.world.content.title
        ));
        b.push(format!("function {ns}:setup"));
        // Pin this test's own dummy (see `pin_dummy`) and drive/assert on it
        // alone — the old `@a`-wide preamble was the round-5 flag leak that
        // poisoned `verb_flag_gate`'s withheld phase.
        b.push(pin);
        b.push(format!(
            "scoreboard players set {sel} {} 0",
            obj_score(id.as_str())
        ));
        b.extend(packtest_preamble(qid, o, true, &sel));
        b.push(format!(
            "scoreboard players set {sel} {} 1",
            plan::interact_trigger(id.as_str())
        ));
        b.push(format!("function {ns}:tick"));
        b.push(format!(
            "assert score {sel} {} matches 1",
            obj_score(id.as_str())
        ));
        write("verb_interact", b);
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
            c.world.content.title
        ));
        b.push(format!("function {ns}:setup"));
        b.push(pin.clone());
        b.push(format!("scoreboard players set {sel} {} 0", obj_score(id)));
        b.extend(packtest_preamble(qid, o, false, &sel)); // flags withheld (cleared)
        driver(&mut b);
        b.push(format!("assert score {sel} {} matches 0", obj_score(id)));
        for f in o.requires_flags() {
            b.push(format!(
                "scoreboard players set {sel} {} 1",
                plan::flag_score(f.as_str())
            ));
        }
        driver(&mut b);
        b.push(format!("assert score {sel} {} matches 1", obj_score(id)));
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
            c.world.content.title
        ));
        b.push(format!("function {ns}:setup"));
        b.push(pin.clone());
        b.push(format!("scoreboard players set {sel} {} 0", obj_score(id)));
        // Preamble satisfies quest/after/requires and CLEARS forbids; then set
        // the forbidden flags to prove suppression.
        b.extend(packtest_preamble(qid, o, true, &sel));
        for f in o.forbids_flags() {
            b.push(format!(
                "scoreboard players set {sel} {} 1",
                plan::flag_score(f.as_str())
            ));
        }
        driver(&mut b);
        b.push(format!("assert score {sel} {} matches 0", obj_score(id)));
        for f in o.forbids_flags() {
            b.push(format!(
                "scoreboard players set {sel} {} 0",
                plan::flag_score(f.as_str())
            ));
        }
        driver(&mut b);
        b.push(format!("assert score {sel} {} matches 1", obj_score(id)));
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
            c.world.content.title
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
            c.world.content.title
        ));
        b.push(format!("function {ns}:setup"));
        // Pin this test's own dummy (see `pin_dummy`) and drive/assert on it alone.
        b.push(pin);
        b.push(format!(
            "scoreboard players set {sel} {} 0",
            obj_score(id.as_str())
        ));
        // Take the item while the objective is INACTIVE (the pre-activation pickup).
        b.push(format!("give {sel} {item} {count}"));
        // Activate WITHOUT re-giving (packtest_preamble would re-give the item, which
        // would mask the bug by producing a fresh inventory_changed): set the quest
        // active + every `after` prerequisite + every required flag by hand.
        b.push(format!(
            "scoreboard players set {sel} {} 1",
            quest_active_score(qid)
        ));
        for a in o.after() {
            b.push(format!(
                "scoreboard players set {sel} {} 1",
                obj_score(a.as_str())
            ));
        }
        for f in o.requires_flags() {
            b.push(format!(
                "scoreboard players set {sel} {} 1",
                plan::flag_score(f.as_str())
            ));
        }
        // One tick's held check completes it — no inventory_changed event occurs.
        b.push(format!("function {ns}:tick"));
        b.push(format!(
            "assert score {sel} {} matches 1",
            obj_score(id.as_str())
        ));
        write("collect_preheld", b);
    }
}

fn emit_server(plan: &Plan, out: &mut BuildOutput) {
    // Combat waves (v0.3) require a non-peaceful difficulty: peaceful *removes*
    // hostile mobs even when summoned. Wave-free campaigns stay `peaceful`
    // (hello-world / keep-crawl byte-identical). Natural spawning is still off
    // (`spawn-monsters=false` + gamerule `spawn_mobs false`); only the compiler's
    // summoned wave mobs exist.
    let difficulty = if plan.campaign.quests.content.waves.is_empty() {
        "peaceful"
    } else {
        "easy"
    };
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
        ("spawn-monsters", "false".to_string()),
        ("spawn-protection", "0".to_string()),
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
- `gamemode=adventure`, `difficulty=peaceful`, no structures/monsters.\n\n\
The compiler-emitted `#minecraft:load` bootstrap (`datapack/`) places each area's\n\
prefab with `/place template` and summons NPCs; nothing is baked into region\n\
bytes, so byte-identity (ADR-0006) covers the whole `<out>/` tree.\n",
            plan.namespace, horizon_bullet, plan.seed
        )
        .into_bytes(),
    );
}

fn emit_critical_path(plan: &Plan) -> Value {
    let steps: Vec<Value> = plan
        .critical_path
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let transport = &plan.critical_path_transport[i];
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
                Step::Collect { objective_id, item, count, pos } => json!({
                    "action": "collect", "objective": objective_id, "item": item,
                    "count": count, "pos": pos
                }),
                Step::Interact { objective_id, anchor_id, pos, command, requires_item } => json!({
                    "action": "interact", "objective": objective_id, "anchor": anchor_id,
                    "pos": pos, "command": command, "requires_item": requires_item
                }),
                Step::AssertComplete { objective, value } => json!({
                    "action": "assert-complete", "scoreboard": { "objective": objective, "value": value }
                }),
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
            if plan.critical_path_sneak[i]
                && let Some(obj) = step.as_object_mut()
            {
                obj.insert("sneak".to_string(), json!(true));
            }
            if let Some(secs) = plan.critical_path_cutscene[i]
                && secs > 0
                && let Some(obj) = step.as_object_mut()
            {
                obj.insert("cutscene_seconds".to_string(), json!(secs));
            }
            step
        })
        .collect();
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
        }
    }

    #[test]
    fn puppet_summon_is_noai_no_loot_and_tagged() {
        let a = mk_actor("actor/giant", "minecraft:warden", false);
        let s = actor_puppet_summon(&a, [10, 65, 20], facing_yaw(Some("west")));
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
        let s = actor_puppet_summon(&a, [0, 64, 0], 0);
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
        let s = actor_puppet_summon(&a, [1, 2, 3], 180);
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
        let s = actor_twin_summon(&a);
        assert!(s.starts_with("summon minecraft:warden ~ ~ ~ "));
        assert!(!s.contains("NoAI"), "the twin has real AI");
        assert!(s.contains("dw_actor_giant") && !s.contains("dw_pup"));
        assert!(s.contains("PersistenceRequired:1b"));
    }

    #[test]
    fn despawn_styles_differ() {
        let mut kill = Vec::new();
        emit_despawn_actor(
            "actor/giant",
            delvewright_dsl::DespawnStyle::Kill,
            &mut kill,
        );
        assert_eq!(kill, vec!["kill @e[tag=dw_actor_giant]".to_string()]);
        let mut vanish = Vec::new();
        emit_despawn_actor(
            "actor/giant",
            delvewright_dsl::DespawnStyle::Vanish,
            &mut vanish,
        );
        assert_eq!(
            vanish,
            vec![
                "tp @e[tag=dw_actor_giant] ~ -128 ~".to_string(),
                "kill @e[tag=dw_actor_giant]".to_string(),
            ]
        );
    }

    #[test]
    fn sequence_key_is_deterministic_and_content_addressed() {
        let step = |t: u32| delvewright_dsl::SequenceStep {
            at_ticks: t,
            effects: vec![delvewright_dsl::QuestEffect::UnleashActor {
                actor: delvewright_dsl::ActorId("actor/giant".to_string()),
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
