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

use delvewright_dsl::{Objective, QuestEffect, Trigger, is_v03};

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
    let relight = crate::light::relight(plan, structures);
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
    let has_waves = !plan.campaign.quests.content.waves.is_empty();
    let (moves, wave_placements): (Vec<crate::nav::MovePlan>, WavePlacements) =
        if crate::nav::needs_world(plan) || has_waves {
            let world =
                crate::nav::World::from_plan_with_extra(plan, structures, &relight.extra_solid);
            let moves = if crate::nav::needs_world(plan) {
                let m = crate::nav::plan_moves(plan, &world)?;
                crate::nav::check_cutscenes(plan, &world)?;
                crate::nav::check_critical_path(plan, &world)?;
                m
            } else {
                Vec::new()
            };
            // Seat each wave mob on a validated standable cell near its anchor, in
            // room only (DW0312 if the room lacks the footing).
            let waves = plan_wave_spawns(plan, &world)?;
            (moves, waves)
        } else {
            (Vec::new(), BTreeMap::new())
        };

    // Every `spawn-wave` effect must resolve a spawn position, or its emitted
    // `function <ns>:spawn_<wave>` call would dangle to a never-emitted function and
    // the wave would silently never spawn (DW0310). Guards against the class of bug
    // where the spawn position was resolvable only via a `kill` objective.
    check_wave_spawns(plan)?;

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
        &relight.placements,
        &wave_placements,
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

    // ---- packtest datapack ----
    emit_packtest(plan, &mut out, &moves);

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
        &crate::render_plan::render_plan(plan, prefabs),
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
    let resource_pack_sha1 = if skins.is_empty() {
        None
    } else {
        let zip = crate::resourcepack::build_pack(skins);
        let sha1 = crate::resourcepack::sha1_hex(&zip);
        out.insert("resourcepack.zip".to_string(), zip);
        out.insert(
            "SKINS.md".to_string(),
            skins_note(&sha1, skins).into_bytes(),
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
/// resource pack into the delve image (itzg env), plus the pack SHA-1.
fn skins_note(sha1: &str, skins: &BTreeMap<String, Vec<u8>>) -> String {
    let mut s = String::new();
    s.push_str("# NPC skin resource pack\n\n");
    s.push_str(
        "This delve ships a server resource pack (`resourcepack.zip`) carrying the\n\
         mannequin NPC skins (spec-0009). The packaging task serves it and sets the\n\
         itzg env so vanilla clients receive it:\n\n",
    );
    s.push_str(&format!(
        "- `RESOURCE_PACK` = the URL the delve serves `resourcepack.zip` at\n\
         - `RESOURCE_PACK_SHA1` = `{sha1}`\n\
         - `RESOURCE_PACK_PROMPT` = a JSON text component (not a bare string)\n\n",
    ));
    s.push_str("Baked skins (`skins/<id>.png` → `assets/delvewright/textures/npc/<id>.png`):\n\n");
    for id in skins.keys() {
        s.push_str(&format!("- `{id}`\n"));
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
) -> Vec<String> {
    let mut cmds = vec![
        "gamerule spawn_mobs false".to_string(),
        "gamerule advance_time false".to_string(),
        "gamerule advance_weather false".to_string(),
        "gamerule fire_spread_radius_around_player 0".to_string(),
        "gamerule mob_griefing false".to_string(),
        // Box-garden death policy: dying must never cost quest items (a dropped
        // trial key despawns in 5 minutes = softlock for a human player).
        "gamerule keep_inventory true".to_string(),
        format!("time set {}", time.token()),
    ];
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

/// Escape a player-facing string as a double-quoted SNBT string. On 1.21.11
/// `CustomName` is a **text component**, so a bare quoted SNBT string is read as
/// literal text (the JSON-string form `'{"text":"…"}'` renders verbatim, incl. in
/// death messages — the M2 defect). Only `\` and `"` need escaping inside SNBT.
fn snbt_string(s: &str) -> String {
    let esc = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{esc}\"")
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

fn emit_functions(
    plan: &Plan,
    sentinels: &Sentinels,
    moves: &[crate::nav::MovePlan],
    relight: &[crate::light::Placement],
    wave_placements: &WavePlacements,
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
    setup.push("scoreboard objectives add dw.campaign dummy".to_string());
    setup.push("scoreboard objectives setdisplay sidebar dw.campaign".to_string());
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
    // v0.4: the per-player scratch bitmask used by flag-gated dialogue choosers.
    // Declared only when a gated option exists, so v0.2/v0.3 setup is unchanged.
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
    // summon NPCs (villager body + interaction hitbox)
    for npc in &plan.npcs {
        let area = plan.npc_area(&npc.npc_id).unwrap_or("");
        let anchor = c
            .npcs
            .content
            .npcs
            .iter()
            .find(|n| n.id.as_str() == npc.npc_id)
            .map(|n| n.anchor.as_str())
            .unwrap_or("");
        let (pos, facing) = match plan.anchors.get(&(area.to_string(), anchor.to_string())) {
            Some(ResolvedAnchor::Point { pos, facing }) => (*pos, facing.as_deref()),
            _ => ([0, plan::BASE_Y, 0], None),
        };
        let dsl_npc = c
            .npcs
            .content
            .npcs
            .iter()
            .find(|n| n.id.as_str() == npc.npc_id);
        let name = dsl_npc.map(|n| n.name.as_str()).unwrap_or("NPC");
        let base = dsl_npc
            .map(|n| n.base_entity.as_str())
            .unwrap_or("minecraft:villager");
        let yaw = facing_yaw(facing);
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
            setup.push(format!(
                "summon minecraft:mannequin {} {} {} {{profile:{{texture:\"delvewright:npc/{}\",model:\"{}\"}},immovable:1b,pose:\"standing\",Invulnerable:1b,Silent:1b,Rotation:[{yaw}f,0f],description:{},Tags:[\"dw_npc\",\"{}\"]}}",
                pos[0], pos[1], pos[2], skin.texture_id, skin.model.token(),
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
            setup.push(format!(
                "summon {base} {} {} {} {{NoAI:1b,Invulnerable:1b,Silent:1b,PersistenceRequired:1b,NoGravity:1b,Rotation:[{yaw}f,0f],Tags:[\"dw_npc\",\"{}\"],CustomName:{},CustomNameVisible:1b,VillagerData:{{profession:\"minecraft:none\",type:\"minecraft:plains\",level:1}}}}",
                pos[0], pos[1], pos[2], npc.tag, cname_field
            ));
        }
        setup.push(format!(
            "summon minecraft:interaction {} {} {} {{width:1.0f,height:2.0f,response:1b,Invulnerable:1b,Tags:[\"{}\"]}}",
            pos[0], pos[1], pos[2], npc.tag
        ));
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
    }
    // v0.4: summon the interaction entities strike/use environment triggers watch
    // (empty for a campaign with no triggers → byte-identical).
    setup.extend(env_trigger_setup(plan));
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
    fns.push(("tick".to_string(), lines(&tick)));

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
            // v0.4: a flag-gated option is inert until its flags are set — so a
            // direct `/trigger` (the bot's path, which bypasses the UI variant
            // hiding) cannot fire it early. `return fail` short-circuits the rest.
            for f in &opt.requires_flags {
                body.push(format!(
                    "execute unless score @s {} matches 1 run return fail",
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
            for eff in objective_effects(c, oid) {
                emit_quest_effect(plan, eff, &mut body);
            }
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
        for eff in &q.on_complete {
            emit_quest_effect(plan, eff, &mut done);
        }
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
    // mineflayer DOES parse reliably — lets the bot observe completion.
    // `<objective> <value>` mirror the assert-complete step so the harness stays
    // campaign-agnostic. `@a` so a bot filling a seat in a future multiplayer
    // delve still sees it.
    cc.push(format!(
        "tellraw @a {}",
        json!({ "text": format!("[Delvewright] complete dw.campaign 1"), "color": "dark_gray" })
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
            // Default hand equipment for mobs whose natural spawns are armed (M2
            // fix 5): a summoned wither_skeleton/skeleton otherwise had no weapon
            // and was trivial. Drop chance 0.
            let equip = default_equipment(&mob.entity)
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
                body.push(format!(
                    "summon {} {} {} {} {{Tags:[\"{}\"{tmp}],PersistenceRequired:1b{name}{equip}{attrs}}}",
                    mob.entity,
                    cell[0],
                    cell[1],
                    cell[2],
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
    fns.extend(movenpc_fns(plan, moves));
    fns.extend(cutscene_fns(plan));
    fns.extend(env_trigger_fns(plan));

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

/// Emit a quest effect's commands into `body`.
fn emit_quest_effect(plan: &Plan, eff: &QuestEffect, body: &mut Vec<String>) {
    let ns = &plan.namespace;
    match eff {
        QuestEffect::OpenGate { anchor } => {
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
        QuestEffect::CampaignComplete => {
            body.push(format!("function {ns}:campaign_complete"));
        }
        QuestEffect::GiveItem { item, count, name } => {
            let comp = match name {
                Some(n) => format!("[custom_name={}]", json!({ "text": n, "italic": false })),
                None => String::new(),
            };
            body.push(format!("give @s {item}{comp} {count}"));
        }
        QuestEffect::SetFlag { flag } => {
            body.push(format!(
                "scoreboard players set @s {} 1",
                plan::flag_score(flag.as_str())
            ));
        }
        QuestEffect::SpawnWave { wave } => {
            body.push(format!(
                "function {ns}:spawn_{}",
                plan::safe_local(wave.as_str())
            ));
        }
        // --- DSL v0.4 effects ---
        QuestEffect::Narrate { text, style, sound } => {
            emit_narrate(text, *style, sound.as_deref(), body);
        }
        QuestEffect::SetBlock { anchor, block } => {
            if let Some(pos) = anchor_point_any(plan, anchor.as_str()) {
                body.push(format!("setblock {} {} {} {block}", pos[0], pos[1], pos[2]));
            }
        }
        QuestEffect::DespawnNpc { npc } => {
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
        QuestEffect::Cutscene { path, seconds } => {
            body.push(format!("function {ns}:{}", cutscene_fn(path, *seconds)));
        }
        // --- DSL v0.5 effects (spec-0010) ---
        // Dimension-global instant cuts. The daylight/weather cycles are frozen by
        // environment sealing (`advance_time`/`advance_weather false`), so the set
        // state persists until the next cut. No selector: `/time set` and
        // `/weather` act on the whole dimension.
        QuestEffect::SetTime { time } => {
            body.push(format!("time set {}", time.token()));
        }
        QuestEffect::SetWeather { weather } => {
            body.push(format!("weather {}", weather.token()));
        }
    }
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

/// The generated function name for a `move-npc` effect (content-derived key, so
/// the start-caller and the generator agree without threading an index).
fn movenpc_fn(npc: &str, to_anchor: &str) -> String {
    format!(
        "mv_{}_{}",
        plan::safe_local(npc),
        plan::safe_local(to_anchor)
    )
}

/// The generated function name for a `cutscene` effect (content-derived key).
fn cutscene_fn(path: &[delvewright_dsl::CameraWaypoint], seconds: u32) -> String {
    let first = path
        .first()
        .map(|w| plan::safe_local(w.anchor.as_str()))
        .unwrap_or_else(|| "none".to_string());
    format!("cs_{first}_{seconds}_{}", path.len())
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
/// trigger effects), for collecting v0.4 lifecycle/cutscene targets.
fn all_campaign_effects(c: &delvewright_dsl::Campaign) -> Vec<&QuestEffect> {
    let mut out = Vec::new();
    for q in &c.quests.content.quests {
        for e in q
            .on_objective_complete
            .values()
            .flatten()
            .chain(&q.on_complete)
        {
            out.push(e);
        }
    }
    for t in &c.quests.content.triggers {
        for e in &t.effects {
            out.push(e);
        }
    }
    out
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
fn movenpc_fns(plan: &Plan, moves: &[crate::nav::MovePlan]) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for m in moves {
        let start_name = movenpc_fn(&m.npc, &m.to_anchor);
        let bare = movenpc_bare(&m.npc, &m.to_anchor);
        let safe = plan::safe_local(&m.npc);
        let total = m.ticks();

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
    }
    out
}

/// Cutscene functions (spec-0008 addendum): the two-camera bounce. Per cutscene
/// (deduped by content key) emits a start function, a self-scheduling per-tick
/// dolly/`spectate` driver, and an end/restore function.
///
/// Mechanic: save each player's return point (a marker at a representative
/// player), spectator, then each tick dolly two co-located invisible cameras
/// along the lerped waypoint polyline and alternate `spectate` between them
/// (the naive same-entity re-`spectate` is a server no-op — never emitted). On
/// completion, restore adventure mode + teleport players back to the marker.
fn cutscene_fns(plan: &Plan) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for eff in all_campaign_effects(plan.campaign) {
        let QuestEffect::Cutscene { path, seconds } = eff else {
            continue;
        };
        // `start` = the function emit_quest_effect calls (`cs_<bare>`); `bare` is
        // the shared suffix for the tick/end functions and per-cutscene sentinels.
        let start_name = cutscene_fn(path, *seconds);
        if !seen.insert(start_name.clone()) {
            continue;
        }
        let bare = start_name
            .strip_prefix("cs_")
            .unwrap_or(&start_name)
            .to_string();
        // Resolve waypoint world positions (anchor + offset, block centres). The
        // air-corridor check (crate::nav, DW0308) validates this exact polyline.
        let pts: Vec<[f64; 3]> = crate::nav::camera_points(plan, path);
        let first = pts
            .first()
            .copied()
            .unwrap_or([0.0, plan::BASE_Y as f64, 0.0]);
        let total: i32 = ((*seconds as i32) * 20).clamp(1, 400);

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
        start.push("gamemode spectator @a".to_string());
        for cam in ["a", "b"] {
            start.push(format!(
                "summon minecraft:item_display {} {} {} {{Tags:[\"dw_cam_{bare}\",\"dw_cam{cam}_{bare}\"]}}",
                fmt_f64(first[0]), fmt_f64(first[1]), fmt_f64(first[2])
            ));
        }
        start.push(format!("schedule function {ns}:cs_tick_{bare} 1t"));
        out.push((start_name.clone(), lines(&start)));

        // per-tick driver
        let mut tick: Vec<String> = Vec::new();
        for t in 0..=total {
            let p = lerp_polyline(&pts, t as f64 / total as f64);
            tick.push(format!(
                "execute if score #t_{bare} dw.sys matches {t} run tp @e[tag=dw_cam_{bare}] {} {} {}",
                fmt_f64(p[0]), fmt_f64(p[1]), fmt_f64(p[2])
            ));
        }
        // alternate `spectate` between the two co-located cameras (the bounce):
        // parity 1 → camera a, parity 2 → camera b, flipped each tick.
        tick.push(format!(
            "execute if score #p_{bare} dw.sys matches 1 as @a run spectate @n[type=minecraft:item_display,tag=dw_cama_{bare}] @s"
        ));
        tick.push(format!(
            "execute if score #p_{bare} dw.sys matches 2 as @a run spectate @n[type=minecraft:item_display,tag=dw_camb_{bare}] @s"
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
        let end: Vec<String> = vec![
            "gamemode adventure @a".to_string(),
            format!("tp @a @e[tag=dw_csmark_{bare},limit=1]"),
            format!("kill @e[tag=dw_cam_{bare}]"),
            format!("kill @e[tag=dw_csmark_{bare}]"),
            format!("scoreboard players set #run_{bare} dw.sys 0"),
        ];
        out.push((format!("cs_end_{bare}"), lines(&end)));
    }
    out
}

/// Linear interpolation along a polyline of points at parameter `s` in `[0,1]`.
fn lerp_polyline(pts: &[[f64; 3]], s: f64) -> [f64; 3] {
    if pts.is_empty() {
        return [0.0, plan::BASE_Y as f64, 0.0];
    }
    if pts.len() == 1 {
        return pts[0];
    }
    let segs = (pts.len() - 1) as f64;
    let u = (s.clamp(0.0, 1.0) * segs).min(segs);
    let i = (u.floor() as usize).min(pts.len() - 2);
    let f = u - i as f64;
    let a = pts[i];
    let b = pts[i + 1];
    [
        a[0] + (b[0] - a[0]) * f,
        a[1] + (b[1] - a[1]) * f,
        a[2] + (b[2] - a[2]) * f,
    ]
}

/// Environment-trigger interaction-entity summons (strike/use) for
/// `setup_finish`. Approach triggers need no entity. Empty for a campaign with no
/// triggers (byte-identical v0.2/v0.3).
fn env_trigger_setup(plan: &Plan) -> Vec<String> {
    use delvewright_dsl::TriggerOn;
    let mut out = Vec::new();
    for t in &plan.campaign.quests.content.triggers {
        if matches!(t.on, TriggerOn::Approach { .. }) {
            continue;
        }
        if let Some(p) = anchor_point_any(plan, t.at.as_str()) {
            out.push(format!(
                "summon minecraft:interaction {} {} {} {{width:1.0f,height:2.0f,response:1b,Invulnerable:1b,Tags:[\"dw_trig_{}\"]}}",
                p[0], p[1], p[2], plan::safe_local(t.id.as_str())
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
                    "execute {once_guard}if entity @e[tag=dw_trig_{id},nbt={{{rec}:{{}}}}] {flag_cond}run function {ns}:trig_{id}"
                ));
                out.push(format!(
                    "execute as @e[tag=dw_trig_{id}] run data remove entity @s {rec}"
                ));
            }
            TriggerOn::Approach { range } => {
                if let Some(p) = anchor_point_any(plan, t.at.as_str()) {
                    out.push(format!(
                        "execute {once_guard}positioned {} {} {} if entity @a[distance=..{range}{}] run function {ns}:trig_{id}",
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
            emit_quest_effect(plan, e, &mut effs);
        }
        let sel = flag_scores_selector(&t.requires_flags);
        for line in effs {
            body.push(format!("execute as @a{sel} run {line}"));
        }
        out.push((format!("trig_{id}"), lines(&body)));
    }
    out
}

/// The first area's `spawn` anchor absolute position.
fn campaign_spawn(plan: &Plan) -> Option<[i32; 3]> {
    for area in &plan.areas {
        if let Some(ResolvedAnchor::Point { pos, .. }) = plan
            .anchors
            .get(&(area.area_id.clone(), "spawn".to_string()))
        {
            return Some(*pos);
        }
    }
    None
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
                cmds.push(format!(
                    "summon minecraft:interaction {} {} {} {{width:1.0f,height:2.0f,response:1b,Invulnerable:1b,Tags:[\"{}\"]}}",
                    pos[0], pos[1], pos[2], interact_entity_tag(id.as_str())
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
                    // Its name derives from the objective `title` (fallback: id).
                    let marker_name = snbt_string(o.title().unwrap_or(id.as_str()));
                    cmds.push(format!(
                        "summon minecraft:item_display {} {} {} {{Glowing:1b,Tags:[\"dw_marker\",\"{}\"],CustomName:{},CustomNameVisible:1b,billboard:\"center\",item:{{id:\"minecraft:lantern\",count:1}}}}",
                        pos[0], pos[1], pos[2], interact_entity_tag(id.as_str()), marker_name
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
            // so a beacon-like light marks a reach destination.
            let marker_name = snbt_string(o.title().unwrap_or(id.as_str()));
            cmds.push(format!(
                "summon minecraft:item_display {} {} {} {{Glowing:1b,Tags:[\"dw_marker\",\"{}\"],CustomName:{},CustomNameVisible:1b,billboard:\"center\",item:{{id:\"minecraft:end_rod\",count:1}}}}",
                pos[0], pos[1], pos[2], reach_marker_tag(id.as_str()), marker_name
            ));
        }
        Objective::TalkTo { .. } | Objective::Kill { .. } => {}
    }
    cmds
}

/// The flags any `set-flag` effect produces (sorted, deduped) — quest effects,
/// plus (DSL v0.4) dialogue `set-flag` effects and environment-trigger effects.
/// Empty extra sources for v0.2/v0.3, keeping their scoreboard setup identical.
fn declared_flags(c: &delvewright_dsl::Campaign) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    for q in &c.quests.content.quests {
        let effs = q
            .on_objective_complete
            .values()
            .flatten()
            .chain(q.on_complete.iter());
        for eff in effs {
            if let Some(f) = eff.set_flag() {
                out.insert(f.as_str().to_string());
            }
        }
    }
    for t in &c.quests.content.triggers {
        for eff in &t.effects {
            if let Some(f) = eff.set_flag() {
                out.insert(f.as_str().to_string());
            }
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
/// must be active, every `after` prerequisite and `requires_flags` flag set, and
/// the objective itself not yet complete. Returns the ` if …`/` unless …` fragment
/// (leading space); callers prepend `execute as @a` and append the type-specific
/// condition + `run`. For a v0.2 objective (no `after`/flags) this is exactly the
/// pre-v0.3 reach guard, keeping keep-crawl byte-identical.
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
    g.push_str(&format!(
        " unless score @s {} matches 1",
        obj_score(o.id().as_str())
    ));
    g
}

// ---------------------------------------------------------------------------
// dialogs / advancements
// ---------------------------------------------------------------------------

/// The sorted, distinct flags gating any option of `node_id` (DSL v0.4). Empty
/// for a node with no flag-gated options (v0.2/v0.3 nodes → byte-identical).
fn node_gated_flags(npc: &plan::NpcPlan, node_id: &str) -> Vec<String> {
    let mut set: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for opt in &npc.options {
        if opt.node_id == node_id {
            for f in &opt.requires_flags {
                set.insert(f.clone());
            }
        }
    }
    set.into_iter().collect()
}

/// The command that displays `node_id`: a direct `dialog show` for an ungated
/// node, or the flag-gate chooser function for a gated one (which shows the
/// variant matching the player's flags).
fn show_node_cmd(plan: &Plan, npc: &plan::NpcPlan, node_id: &str) -> String {
    let ns = &plan.namespace;
    let node_safe = plan::safe_local(node_id);
    if node_gated_flags(npc, node_id).is_empty() {
        format!("dialog show @s {ns}:{}_{}", npc.safe, node_safe)
    } else {
        format!("function {ns}:show_{}_{}", npc.safe, node_safe)
    }
}

/// Flag-gate chooser functions (`show_<npc>_<node>`) for this NPC's gated nodes:
/// compute a per-player bitmask of satisfied gating flags into `dw.dmask`, then
/// `dialog show` the variant (`<npc>_<node>__m<mask>`) whose options are all
/// available. One chooser per gated node.
fn gated_node_choosers(plan: &Plan, npc: &plan::NpcPlan) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for opt in &npc.options {
        if !seen.insert(opt.node_id.as_str()) {
            continue;
        }
        let flags = node_gated_flags(npc, &opt.node_id);
        if flags.is_empty() {
            continue;
        }
        let node_safe = plan::safe_local(&opt.node_id);
        let mut body = vec!["scoreboard players set @s dw.dmask 0".to_string()];
        for (i, f) in flags.iter().enumerate() {
            body.push(format!(
                "execute if score @s {} matches 1 run scoreboard players add @s dw.dmask {}",
                plan::flag_score(f),
                1u32 << i
            ));
        }
        for mask in 0..(1u32 << flags.len()) {
            body.push(format!(
                "execute if score @s dw.dmask matches {mask} run dialog show @s {ns}:{}_{}__m{mask}",
                npc.safe, node_safe
            ));
        }
        out.push((format!("show_{}_{}", npc.safe, node_safe), lines(&body)));
    }
    out
}

/// Whether any dialogue option is flag-gated (gates the `dw.dmask` declaration).
fn has_gated_dialogue(c: &delvewright_dsl::Campaign) -> bool {
    c.dialogue
        .content
        .dialogues
        .iter()
        .flat_map(|t| &t.nodes)
        .flat_map(|n| &n.options)
        .any(|o| !o.requires_flags.is_empty())
}

fn emit_dialogs(plan: &Plan) -> Vec<(String, Value)> {
    let c = plan.campaign;
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
            let flags = node_gated_flags(npc, node.id.as_str());
            if flags.is_empty() {
                // Ungated node → a single dialog (byte-identical to v0.2/v0.3).
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
                // v0.4 flag-gated node → one variant per satisfied-flag bitmask.
                // The chooser function (`show_<npc>_<node>`) shows the variant
                // matching the player's flags, so a gated option is genuinely
                // absent until every flag it needs is set (spec-0008 §1).
                for mask in 0..(1u32 << flags.len()) {
                    let satisfied: std::collections::BTreeSet<&str> = flags
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| mask & (1u32 << i) != 0)
                        .map(|(_, f)| f.as_str())
                        .collect();
                    let visible: Vec<&plan::OptionPlan> = node_opts
                        .iter()
                        .copied()
                        .filter(|o| {
                            o.requires_flags
                                .iter()
                                .all(|f| satisfied.contains(f.as_str()))
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
fn emit_packtest(plan: &Plan, out: &mut BuildOutput, moves: &[crate::nav::MovePlan]) {
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
    let mut body: Vec<String> = Vec::new();
    body.push(format!(
        "#> {}: objective completions set {comp_obj} (Delvewright mechanism test)",
        c.world.content.title
    ));
    body.push("# @dummy".to_string());
    body.push("# @timeout 100".to_string());
    body.push(String::new());
    body.push(format!("function {ns}:setup"));
    for qid in campaign_start_quests(c) {
        body.push(format!(
            "scoreboard players set @a {} 1",
            quest_active_score(qid)
        ));
    }
    for q in &c.quests.content.quests {
        for o in &q.objectives {
            body.push(format!(
                "execute as @a run function {ns}:complete_{}",
                safe_obj_fn(o.id().as_str())
            ));
        }
    }
    // `assert score` requires a single-entity selector (@p = the dummy player).
    body.push(format!("assert score @p {comp_obj} matches {comp_val}"));

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
    sealed.push("execute store result score #sealtime dw.sys run time query daytime".to_string());
    sealed.push(format!(
        "assert score #sealtime dw.sys matches {sealed_ticks}"
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

    // v0.4: prop-on-activation, despawn removes body+hitbox, move arrives at
    // target. Emits nothing when the campaign uses none of them.
    emit_v04_packtests(plan, out, moves);
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
        b.push(format!("kill @e[tag=dw_npc_{safe}]"));
        b.push(format!("function {ns}:setup_finish"));
        // body + interaction hitbox both carry `dw_npc_<npc>` → two entities.
        b.push(format!(
            "execute store result score #before dw.sys if entity @e[tag=dw_npc_{safe}]"
        ));
        b.push("assert score #before dw.sys matches 2".to_string());
        b.push(format!("kill @e[tag=dw_npc_{safe}]"));
        b.push(format!(
            "execute store result score #after dw.sys if entity @e[tag=dw_npc_{safe}]"
        ));
        b.push("assert score #after dw.sys matches 0".to_string());
        write("v04_despawn", b);
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
        b.push(format!("function {ns}:setup_finish"));
        // Jump the driver to its last tick, then execute the final waypoint tp.
        b.push(format!("scoreboard players set #mt_{bare} dw.sys {total}"));
        b.push(format!("function {ns}:mv_tick_{bare}"));
        b.push(format!(
            "execute store result score #at dw.sys if entity @e[tag=dw_npc_{safe},x={},dx=0,y={},dy=0,z={},dz=0]",
            p[0], p[1], p[2]
        ));
        b.push("assert score #at dw.sys matches 1..".to_string());
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
                    // No wave is live yet; the effect's driver spawns it.
                    b.push(format!("function {ns}:spawn_{ws}"));
                    b.push(format!(
                        "execute store result score #kw dw.sys if entity @e[tag={}]",
                        plan::wave_tag(wave.as_str())
                    ));
                    b.push(format!("assert score #kw dw.sys matches {total}"));
                    write("v04_killless_wave", b);
                    break 'killless;
                }
            }
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

/// Lines that satisfy an objective's activation guard on `@a` (quest active, all
/// `after` prerequisites set, all `requires_flags` set, and any required item
/// given). Optionally omit the flags to test the gate.
fn packtest_preamble(quest_id: &str, o: &Objective, with_flags: bool) -> Vec<String> {
    let mut p = vec![format!(
        "scoreboard players set @a {} 1",
        quest_active_score(quest_id)
    )];
    for a in o.after() {
        p.push(format!(
            "scoreboard players set @a {} 1",
            obj_score(a.as_str())
        ));
    }
    if with_flags {
        for f in o.requires_flags() {
            p.push(format!(
                "scoreboard players set @a {} 1",
                plan::flag_score(f.as_str())
            ));
        }
    }
    match o {
        Objective::Collect { item, count, .. } => {
            p.push(format!("give @a {item} {count}"));
        }
        Objective::Interact {
            requires_item: Some(it),
            ..
        } => {
            p.push(format!("give @a {it} 1"));
        }
        _ => {}
    }
    p
}

/// Emit a per-verb mechanism PackTest for the first `kill` / `collect` /
/// `interact` objective, plus a flag-gate test for the first flag-gated
/// collect/interact objective.
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
        let mut b = packtest_header(&format!(
            "{}: kill wave `{wave}` -> countdown -> complete",
            c.world.content.title
        ));
        b.push(format!("function {ns}:setup"));
        b.extend(packtest_preamble(qid, o, true));
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
            b.push("scoreboard players set #armed dw.sys 0".to_string());
            b.push(format!(
                "execute if items entity @e[tag={},type={},limit=1] weapon.mainhand {item} \
                 run scoreboard players set #armed dw.sys 1",
                plan::wave_tag(wave.as_str()),
                mob.entity,
            ));
            b.push("assert score #armed dw.sys matches 1".to_string());
        }
        b.push(format!("kill @e[tag={}]", plan::wave_tag(wave.as_str())));
        for _ in 0..total {
            b.push(format!("execute as @a run function {ns}:k_reward_{ws}"));
        }
        b.push(format!("function {ns}:tick"));
        b.push(format!(
            "assert score @p {} matches 1",
            obj_score(id.as_str())
        ));
        write("verb_kill", b);
    }

    // collect: satisfy guards + hold the item, run the collect reward, assert.
    if let Some((qid, o)) = first_collect
        && let Objective::Collect { id, .. } = o
    {
        let mut b = packtest_header(&format!(
            "{}: collect -> reward completes objective",
            c.world.content.title
        ));
        b.push(format!("function {ns}:setup"));
        b.extend(packtest_preamble(qid, o, true));
        b.push(format!(
            "execute as @a run function {ns}:c_reward_{}",
            plan::safe_local(id.as_str())
        ));
        b.push(format!(
            "assert score @p {} matches 1",
            obj_score(id.as_str())
        ));
        write("verb_collect", b);
    }

    // interact: hold the required item, fire the trigger, tick, assert.
    if let Some((qid, o)) = first_interact
        && let Objective::Interact { id, .. } = o
    {
        let mut b = packtest_header(&format!(
            "{}: interact trigger + item -> complete",
            c.world.content.title
        ));
        b.push(format!("function {ns}:setup"));
        b.extend(packtest_preamble(qid, o, true));
        b.push(format!(
            "scoreboard players set @a {} 1",
            plan::interact_trigger(id.as_str())
        ));
        b.push(format!("function {ns}:tick"));
        b.push(format!(
            "assert score @p {} matches 1",
            obj_score(id.as_str())
        ));
        write("verb_interact", b);
    }

    // flag gate: without the flag the objective must NOT complete; with it, it does.
    if let Some((qid, o)) = first_flag_gated {
        let id = o.id().as_str();
        let driver = |b: &mut Vec<String>| match o {
            Objective::Collect { .. } => b.push(format!(
                "execute as @a run function {ns}:c_reward_{}",
                plan::safe_local(id)
            )),
            Objective::Interact { .. } => {
                b.push(format!(
                    "scoreboard players set @a {} 1",
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
        b.push(format!("scoreboard players set @a {} 0", obj_score(id)));
        b.extend(packtest_preamble(qid, o, false)); // flags withheld
        driver(&mut b);
        b.push(format!("assert score @p {} matches 0", obj_score(id)));
        for f in o.requires_flags() {
            b.push(format!(
                "scoreboard players set @a {} 1",
                plan::flag_score(f.as_str())
            ));
        }
        driver(&mut b);
        b.push(format!("assert score @p {} matches 1", obj_score(id)));
        write("verb_flag_gate", b);
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
        let mut b = packtest_header(&format!(
            "{}: collect completes for an item held before activation",
            c.world.content.title
        ));
        b.push(format!("function {ns}:setup"));
        b.push(format!(
            "scoreboard players set @a {} 0",
            obj_score(id.as_str())
        ));
        // Take the item while the objective is INACTIVE (the pre-activation pickup).
        b.push(format!("give @a {item} {count}"));
        // Activate WITHOUT re-giving (packtest_preamble would re-give the item, which
        // would mask the bug by producing a fresh inventory_changed): set the quest
        // active + every `after` prerequisite + every required flag by hand.
        b.push(format!(
            "scoreboard players set @a {} 1",
            quest_active_score(qid)
        ));
        for a in o.after() {
            b.push(format!(
                "scoreboard players set @a {} 1",
                obj_score(a.as_str())
            ));
        }
        for f in o.requires_flags() {
            b.push(format!(
                "scoreboard players set @a {} 1",
                plan::flag_score(f.as_str())
            ));
        }
        // One tick's held check completes it — no inventory_changed event occurs.
        b.push(format!("function {ns}:tick"));
        b.push(format!(
            "assert score @p {} matches 1",
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
    // server.properties (keys sorted for determinism).
    let props: BTreeMap<&str, String> = BTreeMap::from([
        ("allow-nether", "false".to_string()),
        ("difficulty", difficulty.to_string()),
        ("force-gamemode", "true".to_string()),
        ("gamemode", "adventure".to_string()),
        ("generate-structures", "false".to_string()),
        (
            "generator-settings",
            "{\"biome\":\"minecraft:the_void\",\"layers\":[]}".to_string(),
        ),
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
    text.push_str("# Void/superflat + fixed seed; the world is created on first boot.\n");
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

    out.insert(
        "server/README.md".to_string(),
        format!(
            "# server/\n\n\
Level config for campaign `{}`. The world is generated on first server boot\n\
from `server.properties` (no region files shipped, spec-0002):\n\n\
- `level-type=minecraft:flat` + `generator-settings` with an empty layer list and\n\
  the `minecraft:the_void` biome ⇒ a void world.\n\
- `level-seed={}` pins world generation (ADR-0006); v0 uses no other randomness.\n\
- `gamemode=adventure`, `difficulty=peaceful`, no structures/monsters.\n\n\
The compiler-emitted `#minecraft:load` bootstrap (`datapack/`) places each area's\n\
prefab with `/place template` and summons NPCs; nothing is baked into region\n\
bytes, so byte-identity (ADR-0006) covers the whole `<out>/` tree.\n",
            plan.namespace, plan.seed
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
                Step::TalkTo { npc_id, pos, command } => json!({
                    "action": "talk-to", "npc": npc_id, "pos": pos, "command": command
                }),
                Step::Reach { anchor_id, pos, radius } => json!({
                    "action": "reach", "anchor": anchor_id, "pos": pos, "radius": radius
                }),
                Step::Kill { wave_id, pos, tag, count } => json!({
                    "action": "kill", "wave": wave_id, "pos": pos, "tag": tag, "count": count
                }),
                Step::Collect { item, count, pos } => json!({
                    "action": "collect", "item": item, "count": count, "pos": pos
                }),
                Step::Interact { anchor_id, pos, command, requires_item } => json!({
                    "action": "interact", "anchor": anchor_id, "pos": pos,
                    "command": command, "requires_item": requires_item
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
}
