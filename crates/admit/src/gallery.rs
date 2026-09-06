//! **Gallery world**: one command turns a set of converted candidate pieces into a
//! browse world + datapack. Pieces are placed in a labelled grid (name + asset-id
//! visible in-game via `text_display`), the creator-overlay `dw.note` capture is
//! wired in (spec-0006 reuse), and a `curate` harvest reads the captured notes into
//! a per-asset curation report that merges back into catalog cards.
//!
//! ## Composition, not duplication
//!
//! The note stamp/emit functions reproduce the proven `crates/compiler/src/creator.rs`
//! pattern, but with **per-asset AABBs** so a note resolves `area=<asset-id>`. The
//! gallery emits a `gallery-layout.json` that is shape-compatible with the
//! orchestrator's `Layout`, so `curate` reuses the **exact** `delvec harvest` server-log
//! parser and pairing heuristic (each grid cell is an "area", each asset its own
//! "prefab"). Nothing about note capture is re-implemented.
//!
//! The datapack is deterministic (sorted paths, fixed grid order by asset-id, gzip
//! structures copied verbatim). The *world* is generated on first boot from the
//! bootstrap (void + `/place template`), like every delve.

use std::collections::BTreeMap;

use delvewright_compiler::commands::{CommandError, CommandTree};
use delvewright_orchestrator::{Layout, harvest};
use serde::Serialize;

use crate::catalog::{Curation, CurationNote};
use crate::structure::Structure;

/// Datapack namespace for the gallery.
const NS: &str = "admit";
/// Base Y for the grid floor (matches the compiler's world scheme).
const BASE_Y: i32 = 64;
/// Blocks of gap between grid cells.
const MARGIN: i32 = 3;
/// The report schema version.
pub const CURATION_VERSION: &str = "0.1.0";

/// One candidate piece to display.
#[derive(Debug, Clone)]
pub struct Candidate {
    /// Stable asset id (from the sibling metadata's `prefab_id`, else file stem).
    pub asset_id: String,
    /// Human label shown in-game (falls back to the asset id).
    pub label: String,
    /// Raw gzip-framed structure `.nbt` bytes (copied verbatim into the datapack).
    pub nbt: Vec<u8>,
    /// Structure size (parsed for grid layout + AABB).
    pub size: [i32; 3],
}

impl Candidate {
    /// Build a candidate from raw `.nbt` bytes, resolving size + a default label.
    pub fn from_nbt(asset_id: &str, label: &str, nbt: Vec<u8>) -> Result<Candidate, String> {
        let size = Structure::read(&nbt)?.size;
        Ok(Candidate {
            asset_id: asset_id.to_string(),
            label: label.to_string(),
            nbt,
            size,
        })
    }
}

/// A candidate placed at a grid origin.
struct Placed<'a> {
    cand: &'a Candidate,
    /// Sanitized id used for the structure resource path + selectors + labels.
    safe: String,
    origin: [i32; 3],
}

/// Lay candidates out in a `cols`-wide grid, deterministically ordered by asset id.
fn plan<'a>(cands: &'a [Candidate], cols: usize) -> Vec<Placed<'a>> {
    let cols = cols.max(1);
    let mut ordered: Vec<&Candidate> = cands.iter().collect();
    ordered.sort_by(|a, b| a.asset_id.cmp(&b.asset_id));
    let cell_w = ordered.iter().map(|c| c.size[0]).max().unwrap_or(1);
    let cell_l = ordered.iter().map(|c| c.size[2]).max().unwrap_or(1);
    ordered
        .into_iter()
        .enumerate()
        .map(|(i, cand)| {
            let col = (i % cols) as i32;
            let row = (i / cols) as i32;
            let x0 = col * (cell_w + MARGIN);
            let z0 = row * (cell_l + MARGIN);
            Placed {
                cand,
                safe: sanitize(&cand.asset_id),
                origin: [x0, BASE_Y, z0],
            }
        })
        .collect()
}

/// Emit the whole gallery output tree (`path -> bytes`), deterministically.
///
/// Every `.mcfunction` in the tree is checked against the pinned 1.21.11
/// Brigadier command tree before it is returned — the same gate `delvec` applies
/// to its own emission, reached through the same vendored artifact rather than a
/// second copy of the rule. See [`validate_functions`] for why this is emission
/// and not a test.
pub fn emit(
    gallery_id: &str,
    cands: &[Candidate],
    cols: usize,
) -> Result<BTreeMap<String, Vec<u8>>, Vec<CommandError>> {
    let out = emit_unchecked(gallery_id, cands, cols);
    let errors = validate_functions(&out);
    if errors.is_empty() {
        Ok(out)
    } else {
        Err(errors)
    }
}

/// Check every `.mcfunction` in an emitted gallery tree against the pinned
/// 1.21.11 command tree.
///
/// **Why this is emission and not a test**: the gallery's `load` and
/// `finish` functions each carried a line 1.21.11 refuses to parse — four legacy
/// camelCase gamerules and a `text_opacity:255b` that overflows a signed byte.
/// A function with one unparseable line does not fail that line; the server
/// **drops the whole function** ("Failed to load function admit:load"), so no
/// objective was created, no chunk was forceloaded, no piece was placed and no
/// label was summoned. The gallery world booted, answered `list`, and was empty.
/// Nothing read the server's answer, so nothing could fail — and a check that
/// only runs in `cargo test` is a check the operator running `delvec prefab
/// gallery` on a fresh piece does not run.
pub fn validate_functions(out: &BTreeMap<String, Vec<u8>>) -> Vec<CommandError> {
    let tree = CommandTree::v1_21_11();
    let mut errors = Vec::new();
    for (path, bytes) in out {
        if path.ends_with(".mcfunction")
            && let Ok(body) = std::str::from_utf8(bytes)
        {
            errors.extend(tree.validate_function(body));
        }
    }
    errors
}

/// The raw output tree, before command validation. Private on purpose: there is
/// no caller outside this module that should be able to obtain an unvalidated
/// gallery, which is the whole point of [`emit`] returning a `Result`.
fn emit_unchecked(gallery_id: &str, cands: &[Candidate], cols: usize) -> BTreeMap<String, Vec<u8>> {
    let placed = plan(cands, cols);
    let mut out: BTreeMap<String, Vec<u8>> = BTreeMap::new();

    // pack.mcmeta (same format contract as the main datapack, [94,1]).
    put_json(
        &mut out,
        "datapack/pack.mcmeta",
        &serde_json::json!({
            "pack": {
                "description": format!("delvec prefab gallery: {gallery_id}"),
                "min_format": [94, 1],
                "max_format": [94, 1],
            }
        }),
    );

    put_json(
        &mut out,
        "datapack/data/minecraft/tags/function/load.json",
        &serde_json::json!({ "values": [format!("{NS}:load")] }),
    );
    put_json(
        &mut out,
        "datapack/data/minecraft/tags/function/tick.json",
        &serde_json::json!({ "values": [format!("{NS}:tick")] }),
    );

    // structures (verbatim gzip bytes).
    for p in &placed {
        out.insert(
            format!("datapack/data/{NS}/structure/{}.nbt", p.safe),
            p.cand.nbt.clone(),
        );
    }

    // functions.
    for (name, body) in emit_functions(&placed) {
        out.insert(
            format!("datapack/data/{NS}/function/{name}.mcfunction"),
            body.into_bytes(),
        );
    }

    // gallery-layout.json (orchestrator-`Layout`-compatible; extra fields ignored).
    put_json(
        &mut out,
        "gallery-layout.json",
        &emit_layout(gallery_id, &placed),
    );

    // server config (void world, creative for browsing).
    out.insert(
        "server/server.properties".to_string(),
        server_properties(gallery_id).into_bytes(),
    );
    out.insert(
        "server/eula-note.txt".to_string(),
        b"Accepting Mojang's EULA is the operator's action, never the tool's.\n\
Set EULA=TRUE in the environment (or eula.txt) before running a server here.\n"
            .to_vec(),
    );

    out
}

/// The gallery's functions: `(name, body)`.
fn emit_functions(placed: &[Placed]) -> Vec<(String, String)> {
    let mut fns: Vec<(String, String)> = Vec::new();
    let spawn = [-3, BASE_Y, -3];

    // load: objectives, gamerules, forceload, warmup counter.
    //
    // 1.21.11 gamerule identifiers, measured on the pinned server
    // (2026-08-11, against `1.21.11 / data 4671`): the whole registry
    // is snake_case and several rules were reworded, so every legacy camelCase
    // spelling answers "Incorrect argument for command" — `doDaylightCycle` ->
    // `advance_time`, `doWeatherCycle` -> `advance_weather`, `doMobSpawning` ->
    // `spawn_mobs`, `doImmediateRespawn` -> `immediate_respawn`. The compiler's
    // sealing baseline has always used the new names; this file did not, and
    // because a rejected line kills the ENTIRE function at load, the gallery's
    // objectives and forceloads never ran either.
    let mut load: Vec<String> = vec![
        "scoreboard objectives add dw.note trigger".to_string(),
        "scoreboard objectives add admit.sys dummy".to_string(),
        "gamerule advance_time false".to_string(),
        "gamerule advance_weather false".to_string(),
        "gamerule spawn_mobs false".to_string(),
        "gamerule immediate_respawn true".to_string(),
        "scoreboard players set #t admit.sys 0".to_string(),
    ];
    // forceload each cell + the spawn platform (chunk regions).
    load.push(format!(
        "forceload add {} {} {} {}",
        spawn[0] - 2,
        spawn[2] - 2,
        spawn[0] + 2,
        spawn[2] + 2
    ));
    for p in placed {
        let [ox, _, oz] = p.origin;
        load.push(format!(
            "forceload add {} {} {} {}",
            ox,
            oz,
            ox + p.cand.size[0] - 1,
            oz + p.cand.size[2] - 1
        ));
    }
    fns.push(("load".to_string(), lines(&load)));

    // tick: note capture + timed place/finish (forceloaded chunks need a few ticks).
    let tick = vec![
        "scoreboard players enable @a dw.note".to_string(),
        format!("execute as @a[scores={{dw.note=1..}}] at @s run function {NS}:stamp"),
        "execute unless score #t admit.sys matches 7.. run scoreboard players add #t admit.sys 1"
            .to_string(),
        format!("execute if score #t admit.sys matches 3 run function {NS}:place"),
        format!("execute if score #t admit.sys matches 6 run function {NS}:finish"),
    ];
    fns.push(("tick".to_string(), lines(&tick)));

    // place: idempotent template placement.
    let place: Vec<String> = placed
        .iter()
        .map(|p| {
            format!(
                "place template {NS}:{} {} {} {}",
                p.safe, p.origin[0], p.origin[1], p.origin[2]
            )
        })
        .collect();
    fns.push(("place".to_string(), lines(&place)));

    // finish: spawn platform + labels + worldspawn (runs once).
    let mut finish: Vec<String> = Vec::new();
    finish.push(format!(
        "fill {} {} {} {} {} {} minecraft:stone_bricks",
        spawn[0] - 2,
        spawn[1] - 1,
        spawn[2] - 2,
        spawn[0] + 2,
        spawn[1] - 1,
        spawn[2] + 2
    ));
    finish.push(format!(
        "setworldspawn {} {} {}",
        spawn[0], spawn[1], spawn[2]
    ));
    for p in placed {
        let [ox, oy, oz] = p.origin;
        let lx = ox + p.cand.size[0] / 2;
        let ly = oy + p.cand.size[1] + 1;
        let lz = oz + p.cand.size[2] / 2;
        let text = json_text(&format!("{}  [{}]", p.cand.label, p.cand.asset_id));
        // `text_opacity` is an NBT **byte**, so its range is -128..=127 and
        // "fully opaque" is the vanilla default `-1b`, not `255b`. `255b`
        // overflows the parser ("Failed to parse number: Value out of range"),
        // which took `admit:finish` — the spawn platform, the worldspawn and
        // every label with it — out of the pack at load time.
        finish.push(format!(
            "summon minecraft:text_display {lx} {ly} {lz} {{Tags:[\"admit_label\"],billboard:\"center\",text:'{text}',text_opacity:-1b,see_through:1b}}"
        ));
    }
    fns.push(("finish".to_string(), lines(&finish)));

    // stamp + emit: the note capture (per-asset AABB resolution).
    let mut stamp: Vec<String> = vec!["scoreboard players reset @s dw.note".to_string()];
    for (i, axis) in ["x", "y", "z"].iter().enumerate() {
        stamp.push(format!(
            "execute store result storage {NS}:note {axis} int 1 run data get entity @s Pos[{i}]"
        ));
    }
    stamp.push(format!(
        "data modify storage {NS}:note area set value \"none\""
    ));
    for p in placed {
        let [ox, oy, oz] = p.origin;
        stamp.push(format!(
            "execute if entity @s[x={ox},dx={},y={oy},dy={},z={oz},dz={}] run data modify storage {NS}:note area set value \"{}\"",
            p.cand.size[0] - 1,
            p.cand.size[1] - 1,
            p.cand.size[2] - 1,
            p.cand.asset_id,
        ));
    }
    stamp.push(format!(
        "data modify storage {NS}:note npc set value \"none\""
    ));
    stamp.push(format!("function {NS}:emit with storage {NS}:note"));
    fns.push(("stamp".to_string(), lines(&stamp)));

    fns.push((
        "emit".to_string(),
        lines(&[
            "$say [DelveNote] pos=[$(x),$(y),$(z)] area=$(area) quests= nearest_npc=$(npc)"
                .to_string(),
        ]),
    ));

    fns.sort_by(|a, b| a.0.cmp(&b.0));
    fns
}

/// The gallery layout manifest (orchestrator `Layout` shape + grid extras).
fn emit_layout(gallery_id: &str, placed: &[Placed]) -> serde_json::Value {
    let areas: Vec<serde_json::Value> = placed
        .iter()
        .map(|p| {
            serde_json::json!({
                "id": p.cand.asset_id,
                "prefab": p.cand.asset_id,
                "label": p.cand.label,
                "origin": p.origin,
                "size": p.cand.size,
            })
        })
        .collect();
    serde_json::json!({
        "version": CURATION_VERSION,
        "campaign_id": gallery_id,
        "areas": areas,
        "objectives": [],
    })
}

fn server_properties(gallery_id: &str) -> String {
    let props: BTreeMap<&str, String> = BTreeMap::from([
        ("allow-flight", "true".to_string()),
        ("allow-nether", "false".to_string()),
        ("difficulty", "peaceful".to_string()),
        ("force-gamemode", "true".to_string()),
        ("gamemode", "creative".to_string()),
        ("generate-structures", "false".to_string()),
        (
            "generator-settings",
            "{\"biome\":\"minecraft:the_void\",\"layers\":[]}".to_string(),
        ),
        ("level-name", "world".to_string()),
        ("level-type", "minecraft:flat".to_string()),
        ("online-mode", "false".to_string()),
        ("pvp", "false".to_string()),
        ("spawn-monsters", "false".to_string()),
        ("spawn-protection", "0".to_string()),
    ]);
    let mut text =
        format!("# delvec prefab gallery world for {gallery_id} (browse-only, spec-0007).\n");
    text.push_str("# Void world; the grid is placed by the datapack bootstrap on first boot.\n");
    for (k, v) in &props {
        text.push_str(&format!("{k}={v}\n"));
    }
    text
}

// -------------------------------------------------------------------------
// Curation harvest (dw.note round-trip -> per-asset report).
// -------------------------------------------------------------------------

/// The per-asset curation report — the gallery walk's structured output.
#[derive(Debug, Clone, Serialize)]
pub struct CurationReport {
    pub version: String,
    pub gallery_id: String,
    /// Notes grouped by asset id (the note's resolved `area`).
    pub assets: BTreeMap<String, Vec<CurationNote>>,
    /// Notes that resolved to no piece (`area=none`).
    pub unresolved: Vec<CurationNote>,
}

impl CurationReport {
    pub fn to_json(&self) -> String {
        let mut s = serde_json::to_string_pretty(self).expect("curation report serializes");
        s.push('\n');
        s
    }
}

/// Harvest a gallery playtest server log + `gallery-layout.json` into a per-asset
/// curation report, reusing the orchestrator's note parser/pairer.
pub fn curate(log: &str, layout_json: &str) -> Result<CurationReport, String> {
    let layout = Layout::from_json(layout_json)?;
    let gallery_id = layout.campaign_id.clone();
    let report = harvest(log, &layout);

    let mut assets: BTreeMap<String, Vec<CurationNote>> = BTreeMap::new();
    let mut unresolved: Vec<CurationNote> = Vec::new();
    for n in report.notes {
        let cn = CurationNote {
            at: n.at,
            text: n.text,
            pos: n.pos,
        };
        match n.area {
            Some(asset) => assets.entry(asset).or_default().push(cn),
            None => unresolved.push(cn),
        }
    }
    Ok(CurationReport {
        version: CURATION_VERSION.to_string(),
        gallery_id,
        assets,
        unresolved,
    })
}

/// Fold a curation report's notes into a catalog card's `curation` field.
pub fn merge_into_card(notes: &[CurationNote], existing: Option<Curation>) -> Curation {
    let mut cur = existing.unwrap_or(Curation {
        notes: Vec::new(),
        summary: None,
    });
    for n in notes {
        // idempotent: skip an already-present (at, text) note.
        if !cur.notes.iter().any(|e| e.at == n.at && e.text == n.text) {
            cur.notes.push(n.clone());
        }
    }
    cur
}

// -------------------------------------------------------------------------
// helpers
// -------------------------------------------------------------------------

/// Sanitize an asset id into a valid resource path / selector-safe token.
pub fn sanitize(id: &str) -> String {
    id.chars()
        .map(|c| match c {
            'a'..='z' | '0'..='9' | '/' | '.' | '_' | '-' => c,
            'A'..='Z' => c.to_ascii_lowercase(),
            _ => '_',
        })
        .collect()
}

/// A JSON text component (SNBT-embeddable) escaping quotes/backslashes.
fn json_text(s: &str) -> String {
    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("{{\"text\":\"{escaped}\"}}")
}

fn lines(v: &[String]) -> String {
    let mut s = v.join("\n");
    s.push('\n');
    s
}

fn put_json(out: &mut BTreeMap<String, Vec<u8>>, path: &str, value: &serde_json::Value) {
    let mut bytes = serde_json::to_vec_pretty(value).expect("json serializes");
    bytes.push(b'\n');
    out.insert(path.to_string(), bytes);
}
