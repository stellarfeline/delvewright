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
//! (also logged). The harvester (`delve-harvest`) pairs the two after the session.
//!
//! ### The stamp line
//!
//! ```text
//! [DelveNote] pos=[x,y,z] area=<area-id|none> quests=<obj-id:v,…> nearest_npc=<npc-id|none>
//! ```
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

use serde_json::json;

use crate::emit::BuildOutput;
use crate::plan::{self, Plan, ResolvedAnchor, obj_score};
use crate::{DSL_VERSION, PACK_FORMAT};

/// The trigger objective the creator fires (`/trigger dw.note`).
const NOTE_OBJECTIVE: &str = "dw.note";
/// The scratch storage the stamp writes and the emit macro reads.
fn note_storage(ns: &str) -> String {
    format!("{ns}:note")
}

/// Emit the creator overlay into `out` (paths under `creator-datapack/`).
pub fn emit_creator(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;

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
    for (name, body) in emit_functions(plan) {
        out.insert(
            format!("creator-datapack/data/{ns}/function/creator/{name}.mcfunction"),
            body.into_bytes(),
        );
    }

    // harvester layout manifest
    put_json(out, "creator-datapack/layout.json", &emit_layout(plan));
}

/// The overlay's `creator/*` functions: `(local-name, body)`.
fn emit_functions(plan: &Plan) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let storage = note_storage(ns);
    let mut fns: Vec<(String, String)> = Vec::new();

    // init: (idempotent) register the trigger objective.
    fns.push((
        "init".to_string(),
        lines(&[format!(
            "scoreboard objectives add {NOTE_OBJECTIVE} trigger"
        )]),
    ));

    // tick: keep the trigger armed for everyone; on fire, stamp per player.
    fns.push((
        "tick".to_string(),
        lines(&[
            format!("scoreboard players enable @a {NOTE_OBJECTIVE}"),
            format!(
                "execute as @a[scores={{{NOTE_OBJECTIVE}=1..}}] at @s run function {ns}:creator/stamp"
            ),
        ]),
    ));

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

    // emit: the single macro line. `say` so it reaches the server stdout log
    // (see module docs). Objective ids are baked into the text; only the live
    // values are macro-substituted.
    let quests: Vec<String> = objectives(plan)
        .into_iter()
        .map(|(key, obj_id)| format!("{obj_id}:$({key})"))
        .collect();
    let quests = quests.join(",");
    let macro_line = format!(
        "$say [DelveNote] pos=[$(x),$(y),$(z)] area=$(area) quests={quests} nearest_npc=$(npc)"
    );
    fns.push(("emit".to_string(), lines(&[macro_line])));

    fns.sort_by(|a, b| a.0.cmp(&b.0));
    fns
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
    let anchor = plan
        .campaign
        .npcs
        .content
        .npcs
        .iter()
        .find(|n| n.id.as_str() == npc_id)
        .map(|n| n.anchor.as_str())
        .unwrap_or("");
    match plan.anchors.get(&(area.to_string(), anchor.to_string())) {
        Some(ResolvedAnchor::Point { pos, .. }) => *pos,
        Some(ResolvedAnchor::Gate { from, .. }) => *from,
        None => [0, plan::BASE_Y, 0],
    }
}

/// The harvester's only campaign input: area→prefab, objective→quest, npc→pos.
fn emit_layout(plan: &Plan) -> serde_json::Value {
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
    json!({
        "version": DSL_VERSION,
        "campaign_id": plan.namespace,
        "areas": areas,
        "objectives": objectives_out,
        "npcs": npcs,
    })
}

// --- helpers (mirror emit.rs conventions) ---

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
