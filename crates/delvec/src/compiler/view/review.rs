//! **The worlds review frames stand in** (spec-0112 §3–§5, §7).
//!
//! A review frame is a picture of one configuration: a block map, the light it
//! makes and a clock. A POV frame stands in its leg's configuration, a
//! cutscene frame in the configuration arriving at the step after the one that
//! fires it, and every other kind at load. The bytes are the engine's, never a
//! server's: each world is written by [`crate::compiler::view::world`] from the
//! model the proofs read ([`crate::compiler::nav::Configuration::blocks`] over
//! [`crate::compiler::view::beat::picture_base`]), so two builds of one tree
//! write byte-identical worlds (ADR-0006).
//!
//! [`frame_worlds`] enumerates the distinct configurations the critical path
//! arrives under — its region state and its clock — keyed by first arrival:
//! [`AT_LOAD`], or `after-<step>` for the step whose completion brought it
//! about. [`WorldMaker`] writes one. [`review_record`] is what the build states
//! about them: per world its counts and hashes, per shot its world, mode and
//! what it shows.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use delvewright_dsl::{TimeSite, WorldTime, WorldWeather};

use crate::compiler::blockstate::BlockMap;
use crate::compiler::nav::{Configuration, World};
use crate::compiler::plan::Plan;
use crate::compiler::view::beat::{AT_LOAD, PathModel, after_key, step_id};
use crate::compiler::view::world::{EntityRecord, Ground, Nbt, WorldInput, WrittenWorld};

/// **A configuration's clock** (spec-0112 §7.6): the latest forced `set-time`
/// and `set-weather` whose step is a causal ancestor of the arrival, else the
/// world's declared `time` and `weather`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Clock {
    pub time: WorldTime,
    /// Whether `time` is the world's own (`Sky`) or a cut's (`Cut`).
    pub site: TimeSite,
    pub weather: WorldWeather,
    /// The step that set the time, when a step did.
    pub time_by: Option<usize>,
    /// The step that set the weather, when a step did.
    pub weather_by: Option<usize>,
}

impl Clock {
    /// The world's own clock.
    pub fn of_world(plan: &Plan) -> Clock {
        let w = &plan.campaign.world.content;
        Clock {
            time: w.time,
            site: TimeSite::Sky,
            weather: w.weather,
            time_by: None,
            weather_by: None,
        }
    }

    /// The clock arriving at `arrival` on the critical path.
    pub fn at(plan: &Plan, arrival: usize) -> Clock {
        let mut c = Clock::of_world(plan);
        let mut time_at: Option<usize> = None;
        let mut weather_at: Option<usize> = None;
        for ev in plan.clock_events.iter().filter(|e| e.forced) {
            if !(ev.fire_step < arrival && plan.gate_fired_before(ev.fire_step, arrival)) {
                continue;
            }
            if let Some(t) = ev.time
                && time_at.is_none_or(|s| ev.fire_step >= s)
            {
                time_at = Some(ev.fire_step);
                c.time = t;
                c.site = TimeSite::Cut;
                c.time_by = Some(ev.fire_step);
            }
            if let Some(w) = ev.weather
                && weather_at.is_none_or(|s| ev.fire_step >= s)
            {
                weather_at = Some(ev.fire_step);
                c.weather = w;
                c.weather_by = Some(ev.fire_step);
            }
        }
        c
    }

    /// The `daytime` tick the sun is a function of.
    pub fn daytime_ticks(&self) -> i64 {
        self.time.daytime_ticks()
    }

    /// `level.dat`'s `DayTime`: the absolute clock, its day included.
    pub fn day_time(&self, plan: &Plan) -> i64 {
        self.time
            .clock(self.site, plan.campaign.world.content.time)
            .absolute()
    }

    /// The step that set this clock last, by id, or `None` for the world's
    /// own.
    pub fn set_by(&self, plan: &Plan) -> Option<String> {
        let s = self.time_by.max(self.weather_by)?;
        plan.critical_path
            .get(s)
            .and_then(step_id)
            .map(str::to_string)
            .or_else(|| Some(format!("step {}", s + 1)))
    }

    /// The `sky` a shot states: `{time, daytime_ticks, weather, set_by}`.
    pub fn to_json(&self, plan: &Plan) -> Value {
        json!({
            "time": self.time,
            "daytime_ticks": self.daytime_ticks(),
            "weather": self.weather.keyword(),
            "set_by": self.set_by(plan),
        })
    }
}

/// One world a review frame stands in.
pub struct FrameWorld {
    /// [`AT_LOAD`], or `after-<step>`.
    pub key: String,
    /// The first critical-path arrival under it.
    pub arrival: usize,
    /// The step it is after, by id; `None` at load.
    pub after: Option<String>,
    pub config: Configuration,
    pub clock: Clock,
    /// The cells the configuration's writes lay and the block each lays: what
    /// makes two configurations one world.
    laid: BTreeMap<[i32; 3], Option<String>>,
}

/// Every distinct world the critical path arrives under, and per arrival
/// (`0..=critical_path.len()`) the index of its world and its own
/// configuration — the one the proofs ask of a frame standing there, which
/// may hold a write no world lays (an unforced fill).
pub struct FrameWorlds {
    pub worlds: Vec<FrameWorld>,
    pub per_arrival: Vec<usize>,
    pub arrivals: Vec<Configuration>,
}

impl FrameWorlds {
    /// The world of arrival `s` (the end state past the path's last step).
    pub fn at(&self, s: usize) -> &FrameWorld {
        let i = self
            .per_arrival
            .get(s)
            .or(self.per_arrival.last())
            .copied()
            .unwrap_or(0);
        &self.worlds[i]
    }

    /// The world keyed `key`.
    pub fn get(&self, key: &str) -> Option<&FrameWorld> {
        self.worlds.iter().find(|w| w.key == key)
    }
}

/// **The configurations the critical path arrives under**, one per distinct
/// region state and clock, in first-arrival order — the load world first.
pub fn frame_worlds(plan: &Plan, world: &World) -> FrameWorlds {
    let path = PathModel::critical(plan);
    let mut worlds: Vec<FrameWorld> = Vec::new();
    let mut per_arrival = Vec::new();
    let mut arrivals = Vec::new();
    for s in 0..=plan.critical_path.len() {
        let config = path.at(plan, world, s);
        arrivals.push(path.at(plan, world, s));
        let clock = Clock::at(plan, s);
        // One world per distinct BYTES and clock: two region states that lay
        // the same cells (one differs only in a write no configuration lays)
        // are one picture.
        let laid: BTreeMap<[i32; 3], Option<String>> = config
            .laid()
            .into_iter()
            .map(|(c, b)| (c, b.map(str::to_string)))
            .collect();
        let found = worlds
            .iter()
            .position(|w| w.laid == laid && w.clock == clock);
        let i = match found {
            Some(i) => i,
            None => {
                let after = plan.critical_path[..s]
                    .iter()
                    .rev()
                    .find_map(step_id)
                    .map(str::to_string);
                let mut key = match (&after, worlds.is_empty()) {
                    (_, true) | (None, _) => AT_LOAD.to_string(),
                    (Some(a), false) => after_key(a, None),
                };
                // A key is a world's identity: a second world under one key
                // (two states between one step and the next) takes the
                // arrival as a suffix.
                if worlds.iter().any(|w| w.key == key) {
                    key = format!("{key}-at-{}", s + 1);
                }
                worlds.push(FrameWorld {
                    key,
                    arrival: s,
                    after: if worlds.is_empty() { None } else { after },
                    config,
                    clock,
                    laid,
                });
                worlds.len() - 1
            }
        };
        per_arrival.push(i);
    }
    FrameWorlds {
        worlds,
        per_arrival,
        arrivals,
    }
}

/// A region a configuration's writes lay, `(min, max)` inclusive.
pub type Region = ([i32; 3], [i32; 3]);

/// **The one writer of a review frame's world**: everything a world is made of
/// beyond its configuration, gathered once per build. `delvec build`, `delvec
/// written-world`, `delvec cameras` and `delvec frames --campaign` all write
/// through it, so a world is one function of its configuration.
#[derive(Clone)]
pub struct WorldMaker {
    biome: crate::compiler::horizon::BiomeMap,
    spawn: [i32; 3],
    /// The generator's ground, held out of every cell the build and its setup
    /// reach.
    ground: Ground,
    /// The loaded extent every chunk of which is written.
    pub extent: ([i32; 3], [i32; 3]),
    /// The templates' block entities, by cell, with the block they belong to.
    template_block_entities: BTreeMap<[i32; 3], (crate::compiler::blockstate::BlockState, Nbt)>,
}

impl WorldMaker {
    /// Gather the writer's inputs from a build's assembly; `picture` is the
    /// bytes a picture starts from ([`crate::compiler::view::beat::picture_base`]).
    pub fn new(
        plan: &Plan,
        assembled: &crate::compiler::assembled::Assembled,
        picture: &BlockMap,
        structures: &BTreeMap<String, Vec<u8>>,
    ) -> WorldMaker {
        let mut ground = Ground::of_base(
            delvewright_dsl::horizon_base(&plan.campaign.world.content.horizon),
            assembled.reached.clone(),
        );
        for c in picture.keys() {
            ground.mark(*c);
        }
        WorldMaker {
            biome: crate::compiler::horizon::biome_map(plan),
            spawn: plan.campaign_start().map_or([0, 64, 0], |(_, p)| p),
            ground,
            extent: crate::compiler::render_plan::loaded_extent(plan),
            template_block_entities: crate::compiler::assembled::template_block_entities(
                plan, structures,
            ),
        }
    }

    /// The block entities a block map holds: a template's compound wherever
    /// the cell still holds the template's block.
    pub fn block_entities(&self, blocks: &BlockMap) -> BTreeMap<[i32; 3], Nbt> {
        self.template_block_entities
            .iter()
            .filter(|(c, (state, _))| blocks.get(*c) == Some(state))
            .map(|(c, (_, nbt))| (*c, nbt.clone()))
            .collect()
    }

    /// **Encode a world**: `blocks` (a configuration's bytes) with the regions
    /// its writes `laid` held out of the generator's ground, its block
    /// entities, `entities`, its light and `day_time`.
    pub fn encode_blocks(
        &self,
        blocks: &BlockMap,
        laid: &[Region],
        day_time: i64,
        entities: &[EntityRecord],
    ) -> WrittenWorld {
        let mut ground = self.ground.clone();
        for (lo, hi) in laid {
            ground.mark_box(*lo, *hi);
        }
        let bes = self.block_entities(blocks);
        let biome = |c: [i32; 3]| self.biome.at(c).0.to_string();
        crate::compiler::view::world::encode_world(&WorldInput {
            blocks,
            biome: &biome,
            spawn: self.spawn,
            ground: &ground,
            extent: Some(self.extent),
            block_entities: &bes,
            entities,
            day_time: Some(day_time),
        })
    }

    /// A configuration's world over `picture`, at `clock`.
    pub fn encode(
        &self,
        plan: &Plan,
        picture: &BlockMap,
        config: &Configuration,
        clock: &Clock,
        entities: &[EntityRecord],
    ) -> (BlockMap, WrittenWorld) {
        let blocks = config.blocks(picture);
        let w = self.encode_blocks(
            &blocks,
            &config.laid_regions(),
            clock.day_time(plan),
            entities,
        );
        (blocks, w)
    }
}

/// The binding line of one written world (spec-0112 §9).
pub fn world_line(key: &str, w: &WrittenWorld, dir: &str, ms: u128) -> String {
    format!(
        "world: {key} {} chunk(s), {} cell(s), {} ground cell(s), {} block entit(ies), {} \
         entit(ies), light arrays over {} chunk(s), sha256 {} -> {dir}, written in {ms} ms",
        w.chunks, w.cells, w.ground, w.block_entities, w.entities, w.chunks, w.sha256
    )
}

/// What the build states about one written world in `validation/review-frames.json`.
pub fn world_json(key: &str, w: &WrittenWorld) -> Value {
    json!({
        "key": key,
        "chunks": w.chunks,
        "cells": w.cells,
        "ground_cells": w.ground,
        "block_entities": w.block_entities,
        "entities": w.entities,
        "light_cells": w.light_cells,
        "light_sha256": w.light_sha256,
        "sha256": w.sha256,
    })
}

/// The camera a plan shot states, read back from its JSON.
pub fn shot_camera(shot: &Value) -> Option<crate::compiler::framing::Camera> {
    let cam = &shot["camera"];
    let p = cam["pos"].as_array()?;
    Some(crate::compiler::framing::Camera {
        pos: [p.first()?.as_f64()?, p.get(1)?.as_f64()?, p.get(2)?.as_f64()?],
        yaw: cam["yaw"].as_f64()?,
        pitch: cam["pitch"].as_f64()?,
        fov: cam["fov"]
            .as_f64()
            .unwrap_or(crate::compiler::render_plan::DEFAULT_FOV_DEG),
    })
}

/// One target as a shot's `shows` lists it: the manifest's entry, which the
/// frame copies verbatim.
fn shown(plan: &Plan, r: &crate::compiler::framing::ResolvedTarget<'_>) -> Value {
    let mut v = crate::compiler::framing::target_json(r.target);
    if let Some(body) = crate::compiler::framing::body_of(plan, r.target) {
        v["body"] = json!(body);
    }
    v["screen_bbox"] = json!({
        "x": r.bbox.x, "y": r.bbox.y, "w": r.bbox.w, "h": r.bbox.h,
    });
    v["occluded"] = json!(r.occluded);
    v["distance"] = json!(crate::compiler::framing::round3(r.distance));
    v
}

/// What [`write_review`] made: the binding lines it owes the build's output.
pub struct Review {
    pub lines: Vec<String>,
}

/// **The build's review record** (spec-0112 §5.1, §9): every world a shot of
/// `doc` (the render plan just built) stands in — and the load world, always —
/// written under `validation/worlds/<key>/` into `out`, and
/// `validation/review-frames.json` beside them: per world its counts and
/// hashes, per shot its world, mode, sky and what it shows
/// ([`crate::compiler::framing::resolve_targets`] over the shot's own world).
pub fn write_review(
    plan: &Plan,
    picture: &BlockMap,
    maker: &WorldMaker,
    frames: &FrameWorlds,
    doc: &Value,
    out: &mut BTreeMap<String, Vec<u8>>,
) -> Review {
    let shots = doc["shots"].as_array().cloned().unwrap_or_default();
    // The worlds asked: the load world, then every key a shot names, in
    // first-asked order.
    let mut keys: Vec<String> = vec![frames.worlds[0].key.clone()];
    for s in &shots {
        if let Some(k) = s["world"].as_str()
            && !keys.iter().any(|x| x == k)
        {
            keys.push(k.to_string());
        }
    }
    let targets = crate::compiler::framing::collect_targets(plan);
    let size = crate::compiler::view::frames::REVIEW_SIZE;
    let mut lines = Vec::new();
    let mut worlds_json = Vec::new();
    let mut shot_rows: BTreeMap<usize, Value> = BTreeMap::new();
    let mut sha_of: BTreeMap<String, String> = BTreeMap::new();
    for key in &keys {
        let Some(fw) = frames.get(key) else { continue };
        let started = std::time::Instant::now();
        let (blocks, w) = maker.encode(plan, picture, &fw.config, &fw.clock, &[]);
        let ms = started.elapsed().as_millis();
        let dir = format!("validation/worlds/{key}");
        for (rel, bytes) in &w.files {
            out.insert(format!("{dir}/{rel}"), bytes.clone());
        }
        lines.push(world_line(key, &w, &dir, ms));
        worlds_json.push(world_json(key, &w));
        sha_of.insert(key.clone(), w.sha256.clone());
        // What each shot standing in this world shows: one grid, one sight
        // rule.
        let grid = crate::compiler::framing::VoxelGrid::build(&blocks);
        for (i, s) in shots.iter().enumerate() {
            if s["world"].as_str() != Some(key.as_str()) {
                continue;
            }
            let Some(cam) = shot_camera(s) else { continue };
            let (inside, outside) =
                crate::compiler::framing::resolve_targets(&grid, &cam, size, size, &targets);
            shot_rows.insert(
                i,
                json!({
                    "id": s["id"],
                    "world": key,
                    "mode": s["mode"],
                    "sky": s["sky"],
                    "size": size,
                    "shows": inside.iter().map(|r| shown(plan, r)).collect::<Vec<_>>(),
                    "out_of_frame": outside
                        .iter()
                        .map(|t| crate::compiler::framing::target_json(t))
                        .collect::<Vec<_>>(),
                }),
            );
        }
    }
    let rows: Vec<Value> = shot_rows.into_values().collect();
    // The canonical hash: everything the engine derives about the set, and
    // never a pixel (spec-0112 §6.3).
    let digest_input: Vec<Value> = rows
        .iter()
        .zip(&shots)
        .map(|(r, s)| {
            json!({
                "id": r["id"],
                "world": r["world"],
                "world_sha256": sha_of.get(r["world"].as_str().unwrap_or("")),
                "camera": s["camera"],
                "mode": r["mode"],
                "sky": r["sky"],
                "shows": r["shows"],
            })
        })
        .collect();
    let digest = {
        use sha2::{Digest, Sha256};
        let bytes = serde_json::to_vec(&digest_input).expect("the digest input serializes");
        format!("{:x}", Sha256::digest(&bytes))
    };
    let at_load = rows.iter().filter(|r| r["world"] == AT_LOAD).count();
    let quick = rows.iter().filter(|r| r["mode"] == "quick").count();
    let light_quick = shots
        .iter()
        .filter(|s| light_question(s))
        .count();
    lines.insert(
        0,
        format!(
            "review frames: {} shot(s) in {} world(s) ({at_load} at load, {} after a step); \
             {quick} quick, {} hq, {light_quick} light frame(s) rendered quick; cameras proven \
             clear: {} of {} in their own worlds",
            rows.len(),
            keys.len(),
            rows.len() - at_load,
            rows.len() - quick,
            doc["camera_eye_proof"]["cameras"].as_u64().unwrap_or(0),
            shots.len(),
        ),
    );
    let record = json!({
        "worlds": worlds_json,
        "shots": rows,
        "shots_digest": digest,
        "unchecked": UNCHECKED,
    });
    let mut bytes = serde_json::to_vec_pretty(&record).expect("the review record serializes");
    bytes.push(b'\n');
    out.insert("validation/review-frames.json".to_string(), bytes);
    Review { lines }
}

/// Whether a shot's question is light or atmosphere (spec-0112 §6.1 item 4):
/// its area declares `lighting` or carries a `dark` stamp, its sky was set by
/// a step, or its weather is not clear. Such a shot is drawn hq from phase 2;
/// until then it is drawn quick and counted.
pub fn light_question(shot: &Value) -> bool {
    shot.get("lighting").is_some()
        || !shot["sky"]["set_by"].is_null()
        || shot["sky"]["weather"].as_str().is_some_and(|w| w != "clear")
}

/// What no review frame shows, by name (spec-0112 §11).
pub const UNCHECKED: [&str; 7] = [
    "items: dropped items, item frames' content, item displays, held items and equipment",
    "text displays",
    "beds, shulker boxes, the conduit, open lids, a ringing bell, italic or obfuscated sign text",
    "the weather in the air, clouds, hq-mode fog and the moon",
    "a night-vision room's lightmap: judge in game under night vision",
    "a body's pose at a tick, the party and its stand-ins, a collapse's debris, a set-atmosphere repaint, a flood's spread past its laid cells, an unforced write",
    "a cutscene no critical step fires",
];
