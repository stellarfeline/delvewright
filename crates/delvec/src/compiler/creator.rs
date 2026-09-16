//! Creator overlay emission (`creator-datapack/`), spec-0006 M2.
//!
//! A THIRD compiler output directory beside `datapack/` and `packtest-datapack/`,
//! mounted only by the compose `playtest` profile and **never** baked into the
//! shipped delve image (CI-checked, same exclusion guarantee as PackTest). It is a
//! plain **vanilla 1.21.11** datapack — no mods (ADR-0003) — so every emitted
//! `.mcfunction` is validated by the command-tree checker like the main datapack,
//! and the whole subtree is covered by the ADR-0006 determinism gate (its bytes
//! enter the same `BuildOutput` map + `manifest.json` hashes).
//!
//! ## What it does
//!
//! Adds a `dw.note` **trigger** objective and a per-tick handler. When the creator
//! fires `/trigger dw.note`, the overlay stamps **one machine-readable line** into
//! the server log, then the creator types the actual note as a normal chat message
//! (also logged). The harvester (`delvec harvest`) pairs the two after the session.
//!
//! ### The stamp line
//!
//! ```text
//! [DelveNote] pos=[x,y,z] area=<area-id|none> quests=<obj-id:v,…> nearest_npc=<npc-id|none>
//! [DelveNoteQuests] <obj-id:v,…>
//! ```
//!
//! A chat message holds at most 256 characters (`commands::MESSAGE_MAX_CHARS`),
//! and a campaign's objectives do not fit one: the `quests=` field carries the
//! objectives that fit its line, and each `[DelveNoteQuests]` line after it — in
//! the same function, so consecutive in the log — carries the next ones. The
//! harvester appends a continuation line's objectives to the stamp before it.
//!
//! - **pos** — an **entity-NBT macro read** of the triggering player: the handler
//!   reads `Pos[0..2]` off the player entity into `storage <ns>:note` as ints, then
//!   invokes the emit function `with storage <ns>:note` so the macro substitutes
//!   `$(x) $(y) $(z)`. (Feet position rounded to block ints — adequate for area
//!   resolution and a "roughly here" marker; documented.)
//! - **area** — resolved **in-game** from compiler-baked area AABBs (each area's
//!   `origin..origin+size`). Defaults to `none` outside every area.
//! - **quests** — the live scoreboard state of every objective, read per-objective
//!   into the macro (`<obj-id>:<0|1>`). Runtime-only fact; the harvester regroups
//!   it into per-quest `quest_state` via the layout manifest.
//! - **nearest_npc** — resolved **in-game**: the nearest `dw_npc` body to the
//!   player, mapped to its DSL npc id. Defaults to `none`.
//!
//! **Emission channel — `say`, not `tellraw @a` (documented deviation from the
//! spec's literal wording).** The spec says "stamp via `tellraw @a`", but a
//! `tellraw`/system message to players is **not written to the server stdout log**
//! that the harvester parses (verified: only player chat and `say` reach the
//! console). `say` is the reliable vanilla command that lands the line in the
//! server log; it is macro-expanded exactly as `tellraw` would have been. The
//! creator/bot also sees it in chat, which doubles as capture confirmation.
//!
//! ### Area/context resolution split (the documented choice, spec-0006 §1/§3)
//!
//! The overlay resolves the facts that need live context — `pos`, `quests` — plus
//! the two cheap static lookups (`area`, `nearest_npc`) in-game, so the log line is
//! self-describing. The harvester then enriches each note from the emitted
//! `creator-datapack/layout.json`: `area → prefab` and the flat objective states →
//! per-quest `quest_state`. Layout data therefore lives in one place (the overlay's
//! `layout.json`) and is the harvester's only campaign input.
//!
//! ## A showcase camera placed by hand (spec-0069)
//!
//! Two more triggers, in every overlay:
//!
//! - **`dw.free`** leaves the body and comes back to it. The first fire records
//!   where the player stands, which way they look and their game mode on a
//!   marker at that spot (its chunk force-loaded unless something already forces
//!   it), and puts them in spectator; the second puts the game mode back,
//!   teleports them to the marker and removes it (and the force-load it added).
//!   A datapack function runs at the server's function permission level (2 by
//!   default), which `gamemode` and `forceload` need, so a player nobody opped
//!   can fly.
//! - **`dw.cam set <n>`** (`/trigger dw.cam` is slot 1) stamps the player's eye
//!   and rotation into the log as fixed-point integers:
//!
//! ```text
//! [DelveCamera] slot=<n> eye=<x_mb>,<y_mb>,<z_mb> yaw=<centi-degrees> pitch=<centi-degrees> in=<air|block>
//! ```
//!
//!   The eye is `execute anchored eyes positioned ^ ^ ^`, read off a one-command
//!   marker in milli-blocks, so it is the eye in any pose — standing, sneaking,
//!   spectating inside a wall. The rotation is the entity's own `Rotation` in
//!   centi-degrees, the camera record's convention. `in` says whether the eye's
//!   cell held anything but air; it refuses nothing. `delvec harvest` divides
//!   the integers back out into `camera-report.json`.

use serde_json::json;

use crate::compiler::emit::BuildOutput;
use crate::compiler::nav::{ActorMovePlan, MovePlan};
use crate::compiler::plan::{self, Plan, ResolvedAnchor, obj_score};
use crate::compiler::rehearsal::{Inventory, RehearsalShot};
use crate::compiler::{DSL_VERSION, PACK_FORMAT};

/// The trigger objective the creator fires (`/trigger dw.note`).
const NOTE_OBJECTIVE: &str = "dw.note";
/// The scratch storage the stamp writes and the emit macro reads.
fn note_storage(ns: &str) -> String {
    format!("{ns}:note")
}

/// Emit the creator overlay into `out` (paths under `creator-datapack/`).
pub fn emit_creator(
    plan: &Plan,
    out: &mut BuildOutput,
    moves: &[MovePlan],
    actor_moves: &[ActorMovePlan],
) {
    let ns = &plan.namespace;
    let inv = crate::compiler::rehearsal::inventory(plan, moves, actor_moves);

    // pack.mcmeta (same format contract as the main datapack).
    put_json(
        out,
        "creator-datapack/pack.mcmeta",
        &json!({
            "pack": {
                "description": format!("Delvewright creator overlay: {ns} (playtest-only, spec-0006)"),
                "min_format": PACK_FORMAT,
                "max_format": PACK_FORMAT,
            }
        }),
    );

    // load/tick tags MERGE with the main datapack's tags (vanilla tag merge,
    // replace=false by default), so the overlay adds its functions alongside the
    // campaign's without touching the main datapack.
    put_json(
        out,
        "creator-datapack/data/minecraft/tags/function/load.json",
        &json!({ "values": [format!("{ns}:creator/init")] }),
    );
    put_json(
        out,
        "creator-datapack/data/minecraft/tags/function/tick.json",
        &json!({ "values": [format!("{ns}:creator/tick")] }),
    );

    // functions
    for (name, body) in emit_functions(plan, &inv) {
        out.insert(
            format!("creator-datapack/data/{ns}/function/creator/{name}.mcfunction"),
            body.into_bytes(),
        );
    }

    // harvester layout manifest
    put_json(
        out,
        "creator-datapack/layout.json",
        &emit_layout(plan, &inv),
    );
}

/// The overlay's `creator/*` functions: `(local-name, body)`.
fn emit_functions(plan: &Plan, inv: &Inventory) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let storage = note_storage(ns);
    let mut fns: Vec<(String, String)> = Vec::new();

    // init: (idempotent) register the trigger objective.
    let mut init = vec![format!(
        "scoreboard objectives add {NOTE_OBJECTIVE} trigger"
    )];
    init.extend(camera_init());
    init.extend(rehearsal_init(ns, inv));
    fns.push(("init".to_string(), lines(&init)));

    // tick: keep the trigger armed for everyone; on fire, stamp per player.
    let mut tick = vec![
        format!("scoreboard players enable @a {NOTE_OBJECTIVE}"),
        format!(
            "execute as @a[scores={{{NOTE_OBJECTIVE}=1..}}] at @s run function {ns}:creator/stamp"
        ),
    ];
    tick.extend(camera_tick(ns));
    tick.extend(rehearsal_tick(ns, inv));
    fns.push(("tick".to_string(), lines(&tick)));

    // stamp: runs `as <player> at <player>`. Gathers pos/area/quests/npc into
    // scratch storage, then macro-emits the line.
    let mut stamp: Vec<String> = Vec::new();
    stamp.push(format!("scoreboard players reset @s {NOTE_OBJECTIVE}"));
    // pos — entity-NBT macro read (rounded to block ints).
    for (i, axis) in ["x", "y", "z"].iter().enumerate() {
        stamp.push(format!(
            "execute store result storage {storage} {axis} int 1 run data get entity @s Pos[{i}]"
        ));
    }
    // area — in-game AABB resolution (default none).
    stamp.push(format!(
        "data modify storage {storage} area set value \"none\""
    ));
    for area in &plan.areas {
        let (min, max) = area.bounds();
        let [ox, oy, oz] = min;
        stamp.push(format!(
            "execute if entity @s[x={ox},dx={},y={oy},dy={},z={oz},dz={}] run data modify storage {storage} area set value \"{}\"",
            max[0] - min[0],
            max[1] - min[1],
            max[2] - min[2],
            area.area_id,
        ));
    }
    // nearest_npc — in-game nearest `dw_npc` body → its id (default none).
    stamp.push(format!(
        "data modify storage {storage} npc set value \"none\""
    ));
    for npc in &plan.npcs {
        stamp.push(format!(
            "execute positioned as @s as @e[tag=dw_npc,sort=nearest,limit=1] if entity @s[tag={}] run data modify storage {storage} npc set value \"{}\"",
            npc.tag, npc.npc_id,
        ));
    }
    // quests — live per-objective scoreboard state (unset → 0). Read off the
    // party holder (spec-0018): objective completion is a fact about the party,
    // so a creator note records the delve's progress, not the note-taker's.
    for (key, obj_id) in objectives(plan) {
        stamp.push(format!(
            "execute store result storage {storage} {key} int 1 run scoreboard players get {} {}",
            plan::PARTY,
            obj_score(&obj_id)
        ));
    }
    stamp.push(format!("function {ns}:creator/emit with storage {storage}"));
    fns.push(("stamp".to_string(), lines(&stamp)));

    // emit: the stamp and its continuation lines. `say` so they reach the server
    // stdout log (see module docs). Objective ids are baked into the text; only
    // the live values are macro-substituted.
    let entries: Vec<String> = objectives(plan)
        .into_iter()
        .map(|(key, obj_id)| format!("{obj_id}:$({key})"))
        .collect();
    let longest = |ids: &mut dyn Iterator<Item = usize>| ids.max().unwrap_or(0).max(4);
    let area_chars = longest(&mut plan.areas.iter().map(|a| a.area_id.chars().count()));
    let npc_chars = longest(&mut plan.npcs.iter().map(|n| n.npc_id.chars().count()));
    let head = "$say [DelveNote] pos=[$(x),$(y),$(z)] area=$(area) nearest_npc=$(npc) quests=";
    // The line's longest substitution: three coordinates, the longest area and
    // NPC id, and every objective value at an integer's full width.
    let head_chars = substituted_chars(head) + 3 * INT_CHARS + area_chars + npc_chars;
    fns.push((
        "emit".to_string(),
        lines(&chunk_stamp(head, head_chars, &entries)),
    ));

    fns.extend(camera_fns(ns));
    fns.extend(rehearsal_fns(ns, inv));

    fns.sort_by(|a, b| a.0.cmp(&b.0));
    fns
}

// ---------------------------------------------------------------------------
// A showcase camera placed by hand (spec-0069)
// ---------------------------------------------------------------------------

/// The trigger that stamps the player's eye and rotation (`/trigger dw.cam set <slot>`).
pub const CAMERA_TRIGGER: &str = "dw.cam";
/// The trigger that leaves the body and comes back to it.
pub const FREE_TRIGGER: &str = "dw.free";
/// Scratch registers for both handlers.
const CAM_OBJ: &str = "dw.cm";
/// A player's free-flight id, and the id of the marker holding their place.
const FREE_ID: &str = "dw.fid";
/// The game mode the player left (`playerGameType`), on their marker.
const FREE_MODE: &str = "dw.fmode";
/// `1` on a marker whose chunk this overlay force-loaded, so only that is undone.
const FREE_LOADED: &str = "dw.fload";
/// Tag on a player who is out of their body.
const FREE_TAG: &str = "dw_free";
/// Tag on the marker holding a player's place.
const FREE_HOME: &str = "dw_free_home";
/// Tag on the one marker a handler is working with.
const FREE_THIS: &str = "dw_free_this";
/// Tag on the one-command eye probe.
const CAM_PROBE: &str = "dw_cam_probe";
/// `playerGameType` → the `gamemode` that restores it.
const GAME_MODES: [(i32, &str); 4] = [
    (0, "survival"),
    (1, "creative"),
    (2, "adventure"),
    (3, "spectator"),
];

fn camera_storage(ns: &str) -> String {
    format!("{ns}:camera")
}

/// `creator/init` additions: the two triggers and their scratch objectives.
fn camera_init() -> Vec<String> {
    let mut out: Vec<String> = [CAMERA_TRIGGER, FREE_TRIGGER]
        .iter()
        .map(|t| format!("scoreboard objectives add {t} trigger"))
        .collect();
    for obj in [CAM_OBJ, FREE_ID, FREE_MODE, FREE_LOADED] {
        out.push(format!("scoreboard objectives add {obj} dummy"));
    }
    out
}

/// `creator/tick` additions: arm both triggers for everyone and dispatch a fired
/// one. Never a reset here — see [`rehearsal_tick`] for why a tick that resets a
/// trigger it enables disarms it.
fn camera_tick(ns: &str) -> Vec<String> {
    vec![
        format!("scoreboard players enable @a {CAMERA_TRIGGER}"),
        format!("scoreboard players enable @a {FREE_TRIGGER}"),
        format!(
            "execute as @a[scores={{{CAMERA_TRIGGER}=1..}}] at @s run function {ns}:creator/camera/cam"
        ),
        format!(
            "execute as @a[scores={{{FREE_TRIGGER}=1..}}] at @s run function {ns}:creator/camera/free"
        ),
    ]
}

/// Every `creator/camera/*` function, as `(local-name, body)`.
fn camera_fns(ns: &str) -> Vec<(String, String)> {
    let storage = camera_storage(ns);
    let probe = format!("@e[type=minecraft:marker,tag={CAM_PROBE},limit=1]");
    let this = format!("@e[type=minecraft:marker,tag={FREE_THIS},limit=1]");
    let f = |name: &str| format!("camera/{name}");
    let mut fns: Vec<(String, String)> = Vec::new();

    // --- dw.cam: runs `as <player> at <player>` -----------------------------
    let mut cam = vec![
        format!(
            "execute store result storage {storage} slot int 1 run scoreboard players get @s {CAMERA_TRIGGER}"
        ),
        format!("scoreboard players reset @s {CAMERA_TRIGGER}"),
        format!("kill @e[type=minecraft:marker,tag={CAM_PROBE}]"),
        format!(
            "execute anchored eyes positioned ^ ^ ^ run summon minecraft:marker ~ ~ ~ {{Tags:[\"{CAM_PROBE}\"]}}"
        ),
    ];
    for (i, axis) in ["x", "y", "z"].iter().enumerate() {
        cam.push(format!(
            "execute store result storage {storage} {axis} int 1 run data get entity {probe} Pos[{i}] 1000"
        ));
    }
    for (i, key) in ["yaw", "pitch"].iter().enumerate() {
        cam.push(format!(
            "execute store result storage {storage} {key} int 1 run data get entity @s Rotation[{i}] 100"
        ));
    }
    cam.push(format!(
        "data modify storage {storage} in set value \"air\""
    ));
    cam.push(format!(
        "execute anchored eyes positioned ^ ^ ^ unless block ~ ~ ~ minecraft:air unless block ~ ~ ~ \
         minecraft:cave_air unless block ~ ~ ~ minecraft:void_air run data modify storage {storage} in \
         set value \"block\""
    ));
    cam.push(format!("kill @e[type=minecraft:marker,tag={CAM_PROBE}]"));
    cam.push(format!(
        "function {ns}:creator/camera/stamp with storage {storage}"
    ));
    fns.push((f("cam"), lines(&cam)));
    fns.push((
        f("stamp"),
        lines(&[
            "$say [DelveCamera] slot=$(slot) eye=$(x),$(y),$(z) yaw=$(yaw) pitch=$(pitch) in=$(in)"
                .to_string(),
        ]),
    ));

    // --- dw.free: runs `as <player> at <player>` ----------------------------
    fns.push((
        f("free"),
        lines(&[
            format!("scoreboard players reset @s {FREE_TRIGGER}"),
            format!(
                "execute unless score @s {FREE_ID} matches 1.. run function {ns}:creator/camera/free_id"
            ),
            format!("scoreboard players operation #fid {CAM_OBJ} = @s {FREE_ID}"),
            format!("tag @e[type=minecraft:marker,tag={FREE_THIS}] remove {FREE_THIS}"),
            // Every place marker carries its id from the summon on; seeding it
            // here too keeps the comparison below a comparison between entries.
            format!(
                "scoreboard players add @e[type=minecraft:marker,tag={FREE_HOME}] {FREE_ID} 0"
            ),
            format!(
                "execute as @e[type=minecraft:marker,tag={FREE_HOME}] if score @s {FREE_ID} = #fid {CAM_OBJ} run tag @s add {FREE_THIS}"
            ),
            // Read the state once, then branch: the two arms change what the
            // test reads, so each is chosen before either runs.
            format!("scoreboard players set #back {CAM_OBJ} 0"),
            format!(
                "execute if entity @s[tag={FREE_TAG}] if entity {this} run scoreboard players set #back {CAM_OBJ} 1"
            ),
            format!(
                "execute if score #back {CAM_OBJ} matches 1 run function {ns}:creator/camera/free_back"
            ),
            format!(
                "execute if score #back {CAM_OBJ} matches 0 run function {ns}:creator/camera/free_leave"
            ),
            format!("tag @e[type=minecraft:marker,tag={FREE_THIS}] remove {FREE_THIS}"),
        ]),
    ));
    fns.push((
        f("free_id"),
        lines(&[
            format!("scoreboard players add #next {CAM_OBJ} 1"),
            format!("scoreboard players operation @s {FREE_ID} = #next {CAM_OBJ}"),
        ]),
    ));
    // Leave: a stale place of this player's (a body that left twice without
    // coming back) is dropped first, so one player holds one place.
    let leave = vec![
        format!(
            "execute as {this} at @s if score @s {FREE_LOADED} matches 1 run forceload remove ~ ~"
        ),
        format!("kill @e[type=minecraft:marker,tag={FREE_THIS}]"),
        format!("summon minecraft:marker ~ ~ ~ {{Tags:[\"{FREE_HOME}\",\"{FREE_THIS}\"]}}"),
        format!("tp {this} ~ ~ ~ ~ ~"),
        format!("scoreboard players operation {this} {FREE_ID} = #fid {CAM_OBJ}"),
        format!(
            "execute store result score {this} {FREE_MODE} run data get entity @s playerGameType"
        ),
        format!("execute store success score #forced {CAM_OBJ} run forceload query ~ ~"),
        format!("scoreboard players set {this} {FREE_LOADED} 0"),
        format!(
            "execute if score #forced {CAM_OBJ} matches 0 run scoreboard players set {this} {FREE_LOADED} 1"
        ),
        format!("execute if score #forced {CAM_OBJ} matches 0 run forceload add ~ ~"),
        format!("tag @s add {FREE_TAG}"),
        "gamemode spectator @s".to_string(),
    ];
    fns.push((f("free_leave"), lines(&leave)));
    let mut back = vec![format!(
        "execute store result score #mode {CAM_OBJ} run scoreboard players get {this} {FREE_MODE}"
    )];
    for (value, mode) in GAME_MODES {
        back.push(format!(
            "execute if score #mode {CAM_OBJ} matches {value} run gamemode {mode} @s"
        ));
    }
    back.extend([
        format!("tp @s {this}"),
        format!(
            "execute as {this} at @s if score @s {FREE_LOADED} matches 1 run forceload remove ~ ~"
        ),
        format!("kill @e[type=minecraft:marker,tag={FREE_THIS}]"),
        format!("tag @s remove {FREE_TAG}"),
    ]);
    fns.push((f("free_back"), lines(&back)));
    fns
}

/// Standing eye height, milli-blocks — the metrics table's figure, the number a
/// test holds the overlay's `anchored eyes` read to.
const STANDING_EYE_MB: i32 = (delvewright_dsl::metrics::PLAYER_EYE_HEIGHT * 1000.0) as i32;

/// The height the camera templates stand their dummy at: above anything a
/// campaign builds and below the build limit, so the eye's cell is air unless
/// the template puts a block there.
const CAMERA_TEST_Y: i32 = 300;

/// The PackTest templates that prove the hand camera on a server (spec-0069
/// criteria 2 and 3), as `(path, body)`. They drive the overlay's own handlers,
/// so the server that runs them loads `creator-datapack/` beside the suite.
///
/// `column` is a block column the campaign's own setup force-loads (the corner
/// of its first area). The templates stand there rather than at the test's own
/// position: PackTest places a batch millions of blocks out, where a position in
/// milli-blocks overflows a score and every horizontal reading saturates to the
/// same number — a comparison that could not fail.
///
/// Each settles its dummy in open air for two ticks — a player's pose is
/// recomputed on its tick, and the eye height follows the pose — and then does
/// everything it asserts inside one tick, so no sibling in the batch can move
/// the dummy between the stamp and the reading it is held to.
pub fn packtests(ns: &str, title: &str, column: [i32; 2]) -> Vec<(String, String)> {
    let [cx, cz] = column;
    let at = |dx: f64, y: i32, dz: f64| {
        format!(
            "{} {y} {}",
            f64::from(cx) + 0.5 + dx,
            f64::from(cz) + 0.5 + dz
        )
    };
    let storage = camera_storage(ns);
    let path = |name: &str| format!("packtest-datapack/data/{ns}/test/{name}.mcfunction");
    let y = CAMERA_TEST_Y;
    // The eye, read back off the stamp's storage, against the dummy's own
    // position plus the eye height: within one milli-block on every axis.
    let eye_within = |tag: &str, eye_mb: i32| -> Vec<String> {
        let mut out = Vec::new();
        for (i, axis) in ["x", "y", "z"].iter().enumerate() {
            let (want, got) = (format!("#{tag}_w{axis}"), format!("#{tag}_g{axis}"));
            out.push(format!(
                "execute store result score {want} {CAM_OBJ} run data get entity @s Pos[{i}] 1000"
            ));
            if *axis == "y" {
                out.push(format!("scoreboard players add {want} {CAM_OBJ} {eye_mb}"));
            }
            out.push(format!(
                "execute store result score {got} {CAM_OBJ} run data get storage {storage} {axis}"
            ));
            out.push(format!(
                "scoreboard players operation {got} {CAM_OBJ} -= {want} {CAM_OBJ}"
            ));
            out.push(format!("assert score {got} {CAM_OBJ} matches -1..1"));
        }
        out
    };
    // The slot, the rotation and the verdict the stamp substitutes, read off the
    // storage its macro reads. PackTest's chat condition hears system messages
    // only, and `say` is a chat message, so the log line itself is held by the
    // live flow (`validation/rehearsal-flow.sh`).
    let stamped = |tag: &str, snbt: &str| -> Vec<String> {
        vec![
            format!(
                "execute store success score #{tag}_stamp {CAM_OBJ} if data storage {storage} {snbt}"
            ),
            format!("assert score #{tag}_stamp {CAM_OBJ} matches 1"),
        ]
    };
    let fire = |slot: u32| -> Vec<String> {
        vec![
            format!("scoreboard players set @s {CAMERA_TRIGGER} {slot}"),
            format!("execute at @s run function {ns}:creator/camera/cam"),
        ]
    };
    let mut tests = Vec::new();

    // --- standing, in open air ----------------------------------------------
    let mut stand = vec![
        format!(
            "#> {title}: `/trigger dw.cam` stamps a standing player's eye and rotation (spec-0069)"
        ),
        "# @dummy".to_string(),
        "# @timeout 100".to_string(),
        String::new(),
        "gamemode adventure @s".to_string(),
        format!("tp @s {} 30 15", at(0.0, y, 0.0)),
        "await delay 2t".to_string(),
        "gamemode adventure @s".to_string(),
        format!("tp @s {} 30 15", at(0.0, y, 0.0)),
    ];
    stand.extend(fire(1));
    stand.extend(stamped("cst", "{slot:1,yaw:3000,pitch:1500,in:\"air\"}"));
    stand.extend(eye_within("cst", STANDING_EYE_MB));
    tests.push((path("creator_camera_standing"), lines(&stand)));

    // --- spectating inside a solid block --------------------------------------
    let eye_cell = y + 1;
    let mut wall = vec![
        format!(
            "#> {title}: `/trigger dw.cam` stamps a spectator's eye inside a block, and refuses nothing (spec-0069)"
        ),
        "# @dummy".to_string(),
        "# @timeout 100".to_string(),
        String::new(),
        "gamemode spectator @s".to_string(),
        format!("tp @s {} -90 -30", at(0.0, y, 0.0)),
        "await delay 2t".to_string(),
        "gamemode spectator @s".to_string(),
        format!("setblock {cx} {eye_cell} {cz} minecraft:stone"),
        format!("tp @s {} -90 -30", at(0.0, y, 0.0)),
    ];
    wall.extend(fire(7));
    wall.extend(stamped(
        "cwl",
        "{slot:7,yaw:-9000,pitch:-3000,in:\"block\"}",
    ));
    wall.extend(eye_within("cwl", STANDING_EYE_MB));
    wall.push(format!("setblock {cx} {eye_cell} {cz} minecraft:air"));
    wall.push("gamemode adventure @s".to_string());
    tests.push((path("creator_camera_in_a_block"), lines(&wall)));

    // --- dw.free: leave, fly, come back ---------------------------------------
    let mut free = vec![
        format!("#> {title}: `/trigger dw.free` leaves the body and returns to it (spec-0069)"),
        "# @dummy".to_string(),
        "# @timeout 100".to_string(),
        String::new(),
        "gamemode adventure @s".to_string(),
        format!("tp @s {} 45 10", at(0.0, y, 0.0)),
        format!("execute store result score #cfr_x {CAM_OBJ} run data get entity @s Pos[0] 1000"),
        format!("execute store result score #cfr_y {CAM_OBJ} run data get entity @s Pos[1] 1000"),
        format!("execute store result score #cfr_z {CAM_OBJ} run data get entity @s Pos[2] 1000"),
        format!("scoreboard players set @s {FREE_TRIGGER} 1"),
        format!("execute at @s run function {ns}:creator/camera/free"),
        "assert entity @s[gamemode=spectator]".to_string(),
        format!("tp @s {} 200 -40", at(6.0, y - 10, -5.0)),
        format!("scoreboard players set @s {FREE_TRIGGER} 1"),
        format!("execute at @s run function {ns}:creator/camera/free"),
        "assert entity @s[gamemode=adventure]".to_string(),
    ];
    for (i, axis) in ["x", "y", "z"].iter().enumerate() {
        free.push(format!(
            "execute store result score #cfr_g{axis} {CAM_OBJ} run data get entity @s Pos[{i}] 1000"
        ));
        free.push(format!(
            "scoreboard players operation #cfr_g{axis} {CAM_OBJ} -= #cfr_{axis} {CAM_OBJ}"
        ));
        free.push(format!("assert score #cfr_g{axis} {CAM_OBJ} matches 0"));
    }
    free.push(format!(
        "execute store result score #cfr_yaw {CAM_OBJ} run data get entity @s Rotation[0] 100"
    ));
    free.push(format!("assert score #cfr_yaw {CAM_OBJ} matches 4500"));
    // The place it kept is gone with the return.
    free.push(format!(
        "scoreboard players operation #cfr_fid {CAM_OBJ} = @s {FREE_ID}"
    ));
    free.push(format!("scoreboard players set #cfr_left {CAM_OBJ} 0"));
    free.push(format!(
        "execute as @e[type=minecraft:marker,tag={FREE_HOME}] if score @s {FREE_ID} = #cfr_fid {CAM_OBJ} run scoreboard players add #cfr_left {CAM_OBJ} 1"
    ));
    free.push(format!("assert score #cfr_left {CAM_OBJ} matches 0"));
    tests.push((path("creator_free_returns"), lines(&free)));
    tests
}

// ---------------------------------------------------------------------------
// Cutscene rehearsal / shot calibration (spec-0019)
// ---------------------------------------------------------------------------
//
// The overlay carries a **shot proposal** in `dw:rehearsal` data storage, baked
// at load time from the compiled DSL values ([`crate::compiler::rehearsal::inventory`]).
// The calibration triggers mutate that proposal and nothing else — never the
// datapack — so reposition / re-aim / re-time cycle indefinitely inside one game
// session with no rebuild and no rejoin (spec-0019 §1). `dw.done` stamps the
// whole current proposal as machine-readable `[DelveShot]` lines through the same
// `say` channel `[DelveNote]` uses (a `tellraw` never reaches the server log).
//
// **Everything in the proposal is an integer block cell.** That is the DSL's own
// granularity (`anchor + integer offset`, resolved to the cell centre by
// `nav::anchor_offset_point`), so the write-back round trip is lossless; and it
// is the only NBT numeric type whose SNBT form carries no type suffix, so a
// macro substitution (`$(x)`) yields `12`, never `12.5d` — which would be an
// unparseable argument to `say`/`tp`. See `crate::compiler::rehearsal` for the convention.

/// Trigger objectives the calibration surface arms, in emission order.
/// `dw.mark`/`dw.aim`/`dw.faster`/`dw.slower` take a **1-based** shot id;
/// `dw.mark set -<s>` resets shot `s` to its compiled values (1-based because
/// `-0 == 0` cannot express "reset shot 0"). `dw.done` takes no argument.
const CALIBRATION_TRIGGERS: [&str; 5] = ["dw.mark", "dw.aim", "dw.faster", "dw.slower", "dw.done"];

/// The overlay's private scoreboard (scratch registers + integer constants).
const RH_OBJ: &str = "dw.rh";
/// Data storage holding the proposal (`base` = immutable compiled defaults,
/// `shots` = the live proposal, `arg` = macro scratch).
const RH_STORAGE: &str = "dw:rehearsal";
/// Entity tag of the one-tick `dw.aim` raycast probe.
const RH_PROBE: &str = "dw_rh_probe";
/// Player tag marking "already saw the shot roster this session".
const RH_ROSTER: &str = "dw_rh_roster";
/// Raycast granularity (blocks per step) and step budget: 256 × 0.25 = 64 blocks.
const RH_RAY_STEP: &str = "0.25";
/// Raycast step budget.
const RH_RAY_STEPS: i32 = 256;
/// Player eye height in milli-blocks — the metrics table's figure (spec-0049 §2)
/// scaled into the integer units the emitted commands do their arithmetic in,
/// rather than a fourth site that happened to spell 1.62 correctly.
const RH_EYE_MB: i32 = (delvewright_dsl::metrics::PLAYER_EYE_HEIGHT * 1000.0) as i32;
/// Duration clamp for `dw.faster` / `dw.slower` (seconds), spec-0019 §3.
const RH_MIN_SECONDS: i32 = 2;
/// Upper duration clamp.
const RH_MAX_SECONDS: i32 = 30;

/// `creator/init` additions: the trigger objectives, the scratch objective with
/// its integer constants, and a **once-only** seeding of the proposal.
///
/// The seed is guarded on `unless data storage dw:rehearsal shots` so a
/// `/reload` (which re-runs `#minecraft:load`) does **not** discard a proposal
/// the creator is midway through — the whole point of spec-0019 is that the
/// adjust/replay loop survives inside one session.
fn rehearsal_init(ns: &str, inv: &Inventory) -> Vec<String> {
    if inv.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<String> = CALIBRATION_TRIGGERS
        .iter()
        .map(|t| format!("scoreboard objectives add {t} trigger"))
        .collect();
    out.push(format!("scoreboard objectives add {RH_OBJ} dummy"));
    for (name, value) in [
        ("#k1", 1),
        ("#k2", RH_MIN_SECONDS),
        ("#k20", 20),
        ("#k30", RH_MAX_SECONDS),
        ("#k100", 100),
        ("#k1000", 1000),
        ("#kneg", -1),
    ] {
        out.push(format!("scoreboard players set {name} {RH_OBJ} {value}"));
    }
    out.push(format!(
        "execute unless data storage {RH_STORAGE} shots run function {ns}:creator/rehearsal/defaults"
    ));
    out
}

/// `creator/tick` additions: keep every calibration trigger armed, dispatch a
/// fired one, and stamp the shot roster once per player.
///
/// **A `trigger` objective is armed by the score entry, so `scoreboard players
/// reset` DISARMS it.** Vanilla stores "this player may `/trigger` this
/// objective" as a *lock flag on the score entry itself*; deleting the entry
/// deletes the permission with it, and `scoreboard players enable` re-creates
/// the entry at 0. A tick that both `enable`s an objective and `reset`s it
/// therefore leaves it permanently unusable — every `/trigger` answers "You
/// cannot trigger this objective yet", with nothing in the server log to say so.
///
/// This cost a live debugging round (spec-0019, round 1): a per-tick "hygiene"
/// clause resetting the no-op value (`scores={dw.mark=0}`) matched the very
/// entry `enable` had just created, so `dw.mark`/`dw.aim`/`dw.faster`/`dw.slower`
/// never fired while `dw.done` — which has no such clause — worked perfectly.
/// The tick therefore **never** resets a calibration trigger; only a handler
/// does, after the trigger has actually fired, and the next tick's `enable`
/// re-arms it. Pinned by
/// `rehearsal::the_tick_never_resets_a_trigger_it_arms`.
fn rehearsal_tick(ns: &str, inv: &Inventory) -> Vec<String> {
    if inv.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<String> = CALIBRATION_TRIGGERS
        .iter()
        .map(|t| format!("scoreboard players enable @a {t}"))
        .collect();
    // `dw.mark set <s>` marks, `set -<s>` resets shot `s` to its compiled
    // values. `set 0` names no shot: it matches no dispatch and is simply left
    // alone (see the note above on why it must NOT be cleared here).
    out.push(format!(
        "execute as @a[scores={{dw.mark=1..}}] at @s run function {ns}:creator/rehearsal/mark"
    ));
    out.push(format!(
        "execute as @a[scores={{dw.mark=..-1}}] run function {ns}:creator/rehearsal/reset"
    ));
    out.push(format!(
        "execute as @a[scores={{dw.aim=1..}}] at @s run function {ns}:creator/rehearsal/aim"
    ));
    for (trigger, func) in [("dw.faster", "faster"), ("dw.slower", "slower")] {
        out.push(format!(
            "execute as @a[scores={{{trigger}=1..}}] run function {ns}:creator/rehearsal/{func}"
        ));
    }
    out.push(format!(
        "execute as @a[scores={{dw.done=1..}}] run function {ns}:creator/rehearsal/done"
    ));
    out.push(format!(
        "execute as @a[tag=!{RH_ROSTER}] run function {ns}:creator/rehearsal/roster"
    ));
    out
}

/// Every `creator/rehearsal/*` function, as `(local-name, body)`.
fn rehearsal_fns(ns: &str, inv: &Inventory) -> Vec<(String, String)> {
    if inv.is_empty() {
        return Vec::new();
    }
    let n = inv.shots.len() as i32;
    let range = format!("0..{}", n - 1);
    let mut fns: Vec<(String, String)> = Vec::new();
    let f = |name: &str| format!("rehearsal/{name}");

    // --- seeding -----------------------------------------------------------
    let base: Vec<String> = inv.shots.iter().map(shot_snbt).collect();
    fns.push((
        f("defaults"),
        lines(&[
            format!("data modify storage {RH_STORAGE} campaign set value \"{ns}\""),
            format!("data modify storage {RH_STORAGE} free set value 0"),
            format!(
                "data modify storage {RH_STORAGE} base set value [{}]",
                base.join(",")
            ),
            format!("data modify storage {RH_STORAGE} shots set from storage {RH_STORAGE} base"),
        ]),
    ));

    // --- roster ------------------------------------------------------------
    // Compile-time constant: which id names which shot. Without it the creator
    // has no way to know what `dw.mark set 3` addresses. One `say` per shot,
    // after a count: a chat message holds at most 256 characters, and a roster
    // is as long as the campaign has shots (the command-tree check refuses a
    // longer message, `commands::MESSAGE_MAX_CHARS`).
    let mut roster = vec![
        format!("tag @s add {RH_ROSTER}"),
        format!("say [DelveShotRoster] shots={n}"),
    ];
    roster.extend(inv.shots.iter().map(|s| {
        format!(
            "say [DelveShotRoster] {}={}#{}",
            s.id, s.pointer, s.shot_index
        )
    }));
    fns.push((f("roster"), lines(&roster)));

    // --- dw.mark -----------------------------------------------------------
    fns.push((
        f("mark"),
        lines(&[
            format!("scoreboard players operation #s {RH_OBJ} = @s dw.mark"),
            "scoreboard players reset @s dw.mark".to_string(),
            format!("scoreboard players remove #s {RH_OBJ} 1"),
            format!(
                "execute if score #s {RH_OBJ} matches {range} run function {ns}:creator/rehearsal/mark_at"
            ),
        ]),
    ));
    // The creator's EYE cell: `data get … Pos[i] 1000` gives milli-blocks, and
    // scoreboard `/=` floors, so a negative coordinate lands in the right cell
    // (plain `int 1` truncation towards zero would be off by one below 0).
    let mut mark_at: Vec<String> = Vec::new();
    for (i, axis) in ["x", "y", "z"].iter().enumerate() {
        mark_at.push(format!(
            "execute store result score #{axis} {RH_OBJ} run data get entity @s Pos[{i}] 1000"
        ));
    }
    mark_at.push(format!("scoreboard players add #y {RH_OBJ} {RH_EYE_MB}"));
    for axis in ["x", "y", "z"] {
        mark_at.push(format!(
            "scoreboard players operation #{axis} {RH_OBJ} /= #k1000 {RH_OBJ}"
        ));
    }
    mark_at.extend(store_args(&["s", "x", "y", "z"]));
    mark_at.push(format!(
        "function {ns}:creator/rehearsal/mark_apply with storage {RH_STORAGE} arg"
    ));
    fns.push((f("mark_at"), lines(&mark_at)));
    // First mark after a (re)set REPLACES the compiled path — "first call = path
    // start, second = path end" (spec-0019 §3) only reads that way from an empty
    // path. Every later mark appends. The `marked` flag is read into a score
    // rather than matched in the NBT path so the two branches are exclusive by
    // construction (a compound-match would re-fire after the first branch set it).
    fns.push((
        f("mark_apply"),
        lines(&[
            format!("scoreboard players set #m {RH_OBJ} 0"),
            format!(
                "$execute store result score #m {RH_OBJ} run data get storage {RH_STORAGE} shots[$(s)].marked"
            ),
            format!(
                "execute if score #m {RH_OBJ} matches 1 run function {ns}:creator/rehearsal/mark_next with storage {RH_STORAGE} arg"
            ),
            format!(
                "execute if score #m {RH_OBJ} matches 0 run function {ns}:creator/rehearsal/mark_first with storage {RH_STORAGE} arg"
            ),
        ]),
    ));
    fns.push((
        f("mark_first"),
        lines(&[
            format!(
                "$data modify storage {RH_STORAGE} shots[$(s)].path set value [{{x:$(x),y:$(y),z:$(z)}}]"
            ),
            format!("$data modify storage {RH_STORAGE} shots[$(s)].pstr set value \"$(x),$(y),$(z)\""),
            format!("$data modify storage {RH_STORAGE} shots[$(s)].marked set value 1"),
        ]),
    ));
    fns.push((
        f("mark_next"),
        lines(&[
            format!(
                "$data modify storage {RH_STORAGE} shots[$(s)].path append value {{x:$(x),y:$(y),z:$(z)}}"
            ),
            format!(
                "$data modify storage {RH_STORAGE} arg.old set from storage {RH_STORAGE} shots[$(s)].pstr"
            ),
            format!(
                "function {ns}:creator/rehearsal/mark_join with storage {RH_STORAGE} arg"
            ),
        ]),
    ));
    fns.push((
        f("mark_join"),
        lines(&[format!(
            "$data modify storage {RH_STORAGE} shots[$(s)].pstr set value \"$(old);$(x),$(y),$(z)\""
        )]),
    ));

    // --- dw.mark set -<s> (reset) ------------------------------------------
    fns.push((
        f("reset"),
        lines(&[
            format!("scoreboard players operation #s {RH_OBJ} = @s dw.mark"),
            "scoreboard players reset @s dw.mark".to_string(),
            format!("scoreboard players operation #s {RH_OBJ} *= #kneg {RH_OBJ}"),
            format!("scoreboard players remove #s {RH_OBJ} 1"),
            format!(
                "execute if score #s {RH_OBJ} matches {range} run function {ns}:creator/rehearsal/reset_at"
            ),
        ]),
    ));
    let mut reset_at = store_args(&["s"]);
    reset_at.push(format!(
        "function {ns}:creator/rehearsal/reset_apply with storage {RH_STORAGE} arg"
    ));
    fns.push((f("reset_at"), lines(&reset_at)));
    fns.push((
        f("reset_apply"),
        lines(&[format!(
            "$data modify storage {RH_STORAGE} shots[$(s)] set from storage {RH_STORAGE} base[$(s)]"
        )]),
    ));

    // --- dw.aim (raycast) ---------------------------------------------------
    fns.push((
        f("aim"),
        lines(&[
            format!("scoreboard players operation #s {RH_OBJ} = @s dw.aim"),
            "scoreboard players reset @s dw.aim".to_string(),
            format!("scoreboard players remove #s {RH_OBJ} 1"),
            format!(
                "execute if score #s {RH_OBJ} matches {range} run function {ns}:creator/rehearsal/aim_cast"
            ),
        ]),
    ));
    // A bounded, one-shot ray from the creator's eyes along their view — the
    // vanilla `execute positioned ^ ^ ^<step>` composition, run once on demand
    // (never a per-tick poll). The hit cell is read back off a marker because
    // vanilla has no position→score primitive; the marker lives for the length
    // of this one command chain.
    fns.push((
        f("aim_cast"),
        lines(&[
            format!("kill @e[tag={RH_PROBE}]"),
            format!("scoreboard players set #ray {RH_OBJ} 0"),
            format!(
                "execute anchored eyes positioned ^ ^ ^ run function {ns}:creator/rehearsal/ray"
            ),
            format!(
                "execute if entity @e[tag={RH_PROBE}] run function {ns}:creator/rehearsal/aim_at"
            ),
            format!(
                "execute unless entity @e[tag={RH_PROBE}] run say [DelveShotAim] no block within range — aim unchanged"
            ),
            format!("kill @e[tag={RH_PROBE}]"),
        ]),
    ));
    fns.push((
        f("ray"),
        lines(&[
            format!("scoreboard players add #ray {RH_OBJ} 1"),
            format!(
                "execute unless block ~ ~ ~ minecraft:air unless block ~ ~ ~ minecraft:cave_air \
                 unless block ~ ~ ~ minecraft:void_air unless block ~ ~ ~ minecraft:water \
                 unless entity @e[tag={RH_PROBE}] run summon minecraft:marker ~ ~ ~ {{Tags:[\"{RH_PROBE}\"]}}"
            ),
            format!(
                "execute unless entity @e[tag={RH_PROBE}] if score #ray {RH_OBJ} matches ..{} \
                 positioned ^ ^ ^{RH_RAY_STEP} run function {ns}:creator/rehearsal/ray",
                RH_RAY_STEPS - 1
            ),
        ]),
    ));
    let mut aim_at: Vec<String> = Vec::new();
    for (i, axis) in ["x", "y", "z"].iter().enumerate() {
        aim_at.push(format!(
            "execute store result score #{axis} {RH_OBJ} run data get entity @e[tag={RH_PROBE},limit=1] Pos[{i}] 1000"
        ));
        aim_at.push(format!(
            "scoreboard players operation #{axis} {RH_OBJ} /= #k1000 {RH_OBJ}"
        ));
    }
    aim_at.extend(store_args(&["s", "x", "y", "z"]));
    aim_at.push(format!(
        "function {ns}:creator/rehearsal/aim_apply with storage {RH_STORAGE} arg"
    ));
    fns.push((f("aim_at"), lines(&aim_at)));
    fns.push((
        f("aim_apply"),
        lines(&[
            format!(
                "$data modify storage {RH_STORAGE} shots[$(s)].look set value {{x:$(x),y:$(y),z:$(z)}}"
            ),
            format!("$data modify storage {RH_STORAGE} shots[$(s)].lstr set value \"$(x),$(y),$(z)\""),
        ]),
    ));

    // --- dw.faster / dw.slower ---------------------------------------------
    // ∓20 % with a floor of one whole second, then clamped to 2..30 s. The step
    // is `max(1, round-down 20 %)` so a short shot still converges instead of
    // sticking at its integer fixpoint (4 s × 0.8 = 3.2 → 3, but 2 s × 0.8 = 1.6
    // → 1 would leave `seconds` unchanged under plain integer scaling).
    for (name, trigger, op) in [("faster", "dw.faster", "-="), ("slower", "dw.slower", "+=")] {
        fns.push((
            f(name),
            lines(&[
                format!("scoreboard players operation #s {RH_OBJ} = @s {trigger}"),
                format!("scoreboard players reset @s {trigger}"),
                format!("scoreboard players remove #s {RH_OBJ} 1"),
                format!(
                    "execute if score #s {RH_OBJ} matches {range} run function {ns}:creator/rehearsal/{name}_at"
                ),
            ]),
        ));
        let mut body = store_args(&["s"]);
        body.push(format!(
            "function {ns}:creator/rehearsal/read_sec with storage {RH_STORAGE} arg"
        ));
        body.push(format!(
            "scoreboard players operation #d {RH_OBJ} = #sec {RH_OBJ}"
        ));
        body.push(format!(
            "scoreboard players operation #d {RH_OBJ} *= #k20 {RH_OBJ}"
        ));
        body.push(format!(
            "scoreboard players operation #d {RH_OBJ} /= #k100 {RH_OBJ}"
        ));
        body.push(format!(
            "scoreboard players operation #d {RH_OBJ} > #k1 {RH_OBJ}"
        ));
        body.push(format!(
            "scoreboard players operation #sec {RH_OBJ} {op} #d {RH_OBJ}"
        ));
        body.push(format!(
            "scoreboard players operation #sec {RH_OBJ} > #k2 {RH_OBJ}"
        ));
        body.push(format!(
            "scoreboard players operation #sec {RH_OBJ} < #k30 {RH_OBJ}"
        ));
        body.push(format!(
            "execute store result storage {RH_STORAGE} arg.v int 1 run scoreboard players get #sec {RH_OBJ}"
        ));
        body.push(format!(
            "function {ns}:creator/rehearsal/write_sec with storage {RH_STORAGE} arg"
        ));
        fns.push((f(&format!("{name}_at")), lines(&body)));
    }
    fns.push((
        f("read_sec"),
        lines(&[format!(
            "$execute store result score #sec {RH_OBJ} run data get storage {RH_STORAGE} shots[$(s)].seconds"
        )]),
    ));
    fns.push((
        f("write_sec"),
        lines(&[format!(
            "$data modify storage {RH_STORAGE} shots[$(s)].seconds set value $(v)"
        )]),
    ));

    // --- dw.done (the single harvest) --------------------------------------
    let mut done = vec!["scoreboard players reset @s dw.done".to_string()];
    for s in &inv.shots {
        done.push(format!(
            "function {ns}:creator/rehearsal/stamp_{} with storage {RH_STORAGE} shots[{}]",
            s.id,
            s.id - 1
        ));
    }
    fns.push((f("done"), lines(&done)));
    for s in &inv.shots {
        // Only the live values are substituted; identity (`shot`, `beat`, `ptr`)
        // is compile-time constant, so the stamp always names the DSL location a
        // patch must be applied to.
        fns.push((
            f(&format!("stamp_{}", s.id)),
            lines(&[format!(
                "$say [DelveShot] shot={} beat={} ptr={} idx={} seconds=$(seconds) look_at=$(lstr) path=$(pstr)",
                s.id, s.beat, s.pointer, s.shot_index
            )]),
        ));
    }

    fns
}

/// `execute store result storage <RH_STORAGE> arg.<k> int 1 run scoreboard
/// players get #<k> <RH_OBJ>` for each scratch register `k` — the scores→macro
/// hand-off every calibration verb ends with.
fn store_args(keys: &[&str]) -> Vec<String> {
    keys.iter()
        .map(|k| {
            format!(
                "execute store result storage {RH_STORAGE} arg.{k} int 1 run scoreboard players get #{k} {RH_OBJ}"
            )
        })
        .collect()
}

/// One shot's compiled default as compact SNBT (no spaces: the command-tree
/// validator tokenizes on whitespace, so an NBT argument must be a single token).
///
/// `pstr`/`lstr` are the pre-formatted strings the `[DelveShot]` stamp
/// substitutes — maintained alongside the numeric `path`/`look` by every verb
/// that writes them, so the harvestable line never has to serialize a list
/// through a macro (an SNBT list would carry NBT type suffixes).
fn shot_snbt(shot: &RehearsalShot) -> String {
    let path = shot
        .path
        .iter()
        .map(|c| format!("{{x:{},y:{},z:{}}}", c[0], c[1], c[2]))
        .collect::<Vec<_>>()
        .join(",");
    let pstr = shot
        .path
        .iter()
        .map(|c| format!("{},{},{}", c[0], c[1], c[2]))
        .collect::<Vec<_>>()
        .join(";");
    let (look, lstr) = match shot.look_at {
        Some(c) => (
            format!("{{x:{},y:{},z:{}}}", c[0], c[1], c[2]),
            format!("{},{},{}", c[0], c[1], c[2]),
        ),
        None => ("{}".to_string(), "none".to_string()),
    };
    format!(
        "{{id:{},beat:{},marked:0,seconds:{},path:[{path}],pstr:\"{pstr}\",look:{look},lstr:\"{lstr}\"}}",
        shot.id, shot.beat, shot.seconds
    )
}

/// Every objective as `(storage-key, dsl-id)`, in a stable order (quest order,
/// then declared objective order). The key mirrors the scoreboard name minus the
/// `dw.` prefix (`obj/talk` → `o_talk`).
fn objectives(plan: &Plan) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for q in &plan.campaign.quests.content.quests {
        for o in &q.objectives {
            let id = o.id().as_str().to_string();
            let key = format!("o_{}", plan::safe_local(&id));
            out.push((key, id));
        }
    }
    out
}

/// The nearest resolved position of an NPC (its declared anchor within its area),
/// for the layout manifest. Mirrors the main emitter's NPC placement.
fn npc_pos(plan: &Plan, npc_id: &str) -> [i32; 3] {
    let area = plan.npc_area(npc_id).unwrap_or("");
    let decl = plan
        .campaign
        .npcs
        .content
        .npcs
        .iter()
        .find(|n| n.id.as_str() == npc_id);
    let anchor = decl.map(|n| n.anchor.as_str()).unwrap_or("");
    let offset = decl.map(|n| n.offset).unwrap_or([0, 0, 0]);
    match plan.anchors.get(&(area.to_string(), anchor.to_string())) {
        Some(ResolvedAnchor::Point { pos, .. }) => delvewright_dsl::offset_cell(*pos, offset),
        Some(ResolvedAnchor::Gate { from, .. }) => delvewright_dsl::offset_cell(*from, offset),
        None => [0, plan::BASE_Y, 0],
    }
}

/// The harvester's only campaign input: area→prefab, objective→quest, npc→pos —
/// plus, since spec-0019, the **resolved-anchor manifest** (`anchors`) and the
/// rehearsal shot roster (`shots`).
///
/// `anchors` is the vocabulary `delvec calibrate` snaps a harvested proposal
/// back onto: every declared anchor with the absolute cell it resolved to in the
/// assembled world. It lives here rather than in a new build output because it
/// is a creator-loop artifact — the shipped image never carries
/// `creator-datapack/` (CI-checked), and adding a new top-level output file
/// would change every campaign's `manifest.json`.
fn emit_layout(plan: &Plan, inv: &Inventory) -> serde_json::Value {
    let areas: Vec<serde_json::Value> = plan
        .areas
        .iter()
        .map(|a| {
            let (min, max) = a.bounds();
            // `prefab` names the area's entry piece (the whole prefab for a
            // single-prefab area). `size` is the union AABB extent.
            let prefab = a.pieces.first().map(|p| p.prefab_id.as_str()).unwrap_or("");
            json!({
                "id": a.area_id,
                "prefab": prefab,
                "origin": min,
                "size": [max[0] - min[0] + 1, max[1] - min[1] + 1, max[2] - min[2] + 1],
            })
        })
        .collect();
    let mut objectives_out: Vec<serde_json::Value> = Vec::new();
    for q in &plan.campaign.quests.content.quests {
        for o in &q.objectives {
            objectives_out.push(json!({
                "id": o.id().as_str(),
                "quest": q.id.as_str(),
            }));
        }
    }
    let npcs: Vec<serde_json::Value> = plan
        .npcs
        .iter()
        .map(|n| {
            json!({
                "id": n.npc_id,
                "pos": npc_pos(plan, &n.npc_id),
            })
        })
        .collect();
    // Resolved anchors, in the plan's own (area, name) order — a `BTreeMap`, so
    // the listing is deterministic. A gate anchor resolves to its `from` corner,
    // exactly as `nav::anchor_offset_point` does, so a calibrated offset means
    // the same thing to the compiler as it does here.
    let anchors: Vec<serde_json::Value> = plan
        .anchors
        .iter()
        .map(|((area, name), resolved)| {
            let (kind, pos) = match resolved {
                ResolvedAnchor::Point { pos, .. } => ("point", *pos),
                ResolvedAnchor::Gate { from, .. } => ("gate", *from),
            };
            json!({ "id": name, "area": area, "kind": kind, "pos": pos })
        })
        .collect();
    let shots: Vec<serde_json::Value> = inv
        .shots
        .iter()
        .map(|s| {
            json!({
                "shot": s.id,
                "beat": s.beat,
                "pointer": s.pointer,
                "shot_index": s.shot_index,
                "seconds": s.seconds,
            })
        })
        .collect();
    json!({
        "version": DSL_VERSION,
        "campaign_id": plan.namespace,
        "areas": areas,
        "objectives": objectives_out,
        "npcs": npcs,
        "anchors": anchors,
        "shots": shots,
    })
}

// --- helpers (mirror emit.rs conventions) ---

/// The widest text an `int` macro value substitutes to (`-2147483648`).
const INT_CHARS: usize = 11;

/// A macro line's text with every `$(name)` removed and its leading `$` and
/// command dropped, in characters: what the message argument holds before any
/// value is substituted.
fn substituted_chars(line: &str) -> usize {
    let body = line.strip_prefix("$say ").unwrap_or(line);
    let mut n = 0;
    let mut rest = body;
    while let Some(start) = rest.find("$(") {
        n += rest[..start].chars().count();
        rest = match rest[start..].find(')') {
            Some(end) => &rest[start + end + 1..],
            None => "",
        };
    }
    n + rest.chars().count()
}

/// `head` followed by as many `obj:$(key)` entries as keep the line inside a chat
/// message at its longest substitution (every value at [`INT_CHARS`]), then one
/// `[DelveNoteQuests]` line per further run of entries that fits.
fn chunk_stamp(head: &str, head_chars: usize, entries: &[String]) -> Vec<String> {
    use crate::compiler::commands::MESSAGE_MAX_CHARS;
    let cont = "$say [DelveNoteQuests] ";
    let entry_chars = |e: &str| substituted_chars(e) + INT_CHARS;
    let mut out = Vec::new();
    let (mut line, mut chars, mut first) = (head.to_string(), head_chars, true);
    let mut empty = true;
    for e in entries {
        let add = entry_chars(e) + usize::from(!empty);
        if chars + add > MESSAGE_MAX_CHARS && !empty {
            out.push(line);
            line = cont.to_string();
            chars = substituted_chars(cont);
            first = false;
            empty = true;
        }
        if !empty {
            line.push(',');
        }
        line.push_str(e);
        chars += entry_chars(e) + usize::from(!empty);
        empty = false;
    }
    if !empty || first {
        out.push(line);
    }
    out
}

fn lines(v: &[String]) -> String {
    let mut s = v.join("\n");
    s.push('\n');
    s
}

fn put_json(out: &mut BuildOutput, path: &str, value: &serde_json::Value) {
    let mut bytes = serde_json::to_vec_pretty(value).expect("json serializes");
    bytes.push(b'\n');
    out.insert(path.to_string(), bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The eye height reaches the emitted commands as an integer, and the scaling
    /// that gets it there is a truncating float cast — so the one thing worth
    /// pinning is that it truncates to the figure the overlay has always emitted
    /// rather than to the one below it.
    /// However many objectives a campaign has, every stamp line stays inside a
    /// chat message at its longest substitution, and every objective is on
    /// exactly one line.
    #[test]
    fn a_note_stamp_splits_its_objectives_across_lines_that_fit() {
        let head = "$say [DelveNote] pos=[$(x),$(y),$(z)] area=$(area) nearest_npc=$(npc) quests=";
        let head_chars = substituted_chars(head) + 3 * INT_CHARS + 40 + 40;
        let entries: Vec<String> = (0..60)
            .map(|i| format!("obj/a-long-objective-name-{i}:$(o_a_long_objective_name_{i})"))
            .collect();
        let out = chunk_stamp(head, head_chars, &entries);
        assert!(out.len() > 2, "{out:?}");
        assert!(out[0].starts_with(head));
        let mut seen = Vec::new();
        for (i, line) in out.iter().enumerate() {
            // Every placeholder at an integer's width, the area and NPC ids at
            // the 40 characters stated above.
            let placeholders = line.matches("$(").count();
            let longest = if i == 0 {
                substituted_chars(line) + INT_CHARS * (placeholders - 2) + 80
            } else {
                substituted_chars(line) + INT_CHARS * placeholders
            };
            assert!(longest <= 256, "line {i} reaches {longest}: {line}");
            let fields = if i == 0 {
                line.strip_prefix(head).unwrap()
            } else {
                line.strip_prefix("$say [DelveNoteQuests] ").unwrap()
            };
            seen.extend(fields.split(',').map(str::to_string));
        }
        assert_eq!(seen, entries);
        // No objectives: one line, an empty field.
        assert_eq!(chunk_stamp(head, head_chars, &[]), vec![head.to_string()]);
    }

    #[test]
    fn the_eye_height_scales_into_milli_blocks_without_losing_a_unit() {
        assert_eq!(RH_EYE_MB, 1620);
    }
}
