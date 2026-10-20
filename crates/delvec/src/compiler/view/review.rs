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
    /// The ledger rows its arrival holds, by index into the build's ledger
    /// ([`crate::compiler::placed::rows_at`]).
    pub rows: Vec<usize>,
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
    frame_worlds_with(plan, world, None)
}

/// [`frame_worlds`], a world's identity also holding the placements ledger's
/// rows its arrival holds.
pub fn frame_worlds_with(
    plan: &Plan,
    world: &World,
    ledger: Option<&crate::compiler::placed::Ledger>,
) -> FrameWorlds {
    let path = PathModel::critical(plan);
    // Every clock a configuration holds is one the campaign can reach
    // (`light::reachable_clocks`, the set `DW0890` holds the design to): two
    // walks of one campaign's set-time and set-weather effects, which must agree.
    let (reachable_times, reachable_weathers) =
        crate::compiler::light::reachable_clocks(plan.campaign);
    let world_time = plan.campaign.world.content.time;
    let mut worlds: Vec<FrameWorld> = Vec::new();
    let mut per_arrival = Vec::new();
    let mut arrivals = Vec::new();
    for s in 0..=plan.critical_path.len() {
        let config = path.at(plan, world, s);
        arrivals.push(path.at(plan, world, s));
        let clock = Clock::at(plan, s);
        let at = clock.time.clock(clock.site, world_time);
        assert!(
            reachable_times.iter().any(|(c, _)| *c == at)
                && reachable_weathers.contains(&clock.weather),
            "the clock arriving at step {s} ({at:?}, {:?}) is no clock the campaign reaches — the \
             clock walk and `light::reachable_clocks` disagree",
            clock.weather
        );
        // One world per distinct BYTES and clock: two region states that lay
        // the same cells (one differs only in a write no configuration lays)
        // are one picture.
        let laid: BTreeMap<[i32; 3], Option<String>> = config
            .laid()
            .into_iter()
            .map(|(c, b)| (c, b.map(str::to_string)))
            .collect();
        let rows: Vec<usize> = match ledger {
            Some(l) => crate::compiler::placed::rows_at(plan, l, s)
                .into_iter()
                .filter_map(|r| l.rows.iter().position(|x| std::ptr::eq(x, r)))
                .collect(),
            None => Vec::new(),
        };
        let found = worlds
            .iter()
            .position(|w| w.laid == laid && w.clock == clock && w.rows == rows);
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
                    rows,
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

    /// The block entities a block map holds: one at every cell whose block
    /// carries one ([`crate::compiler::registry::block_entity_type`]), because
    /// the server makes it whenever it places the block, compound or none —
    /// the template's compound wherever the cell still holds the template's
    /// block, and otherwise the type alone, every other key being the
    /// server's own default for that type.
    pub fn block_entities(&self, blocks: &BlockMap) -> BTreeMap<[i32; 3], Nbt> {
        blocks
            .iter()
            .filter_map(|(c, state)| {
                let kind = crate::compiler::registry::block_entity_type(
                    crate::compiler::assembled::base_id(state.as_str()),
                )?;
                let nbt = match self.template_block_entities.get(c) {
                    Some((placed, nbt)) if placed == state => nbt.clone(),
                    _ => Nbt::Compound(vec![("id".to_string(), Nbt::Str(kind.to_string()))]),
                };
                Some((*c, nbt))
            })
            .collect()
    }

    /// **Encode a world**: `blocks` (a configuration's bytes) with the regions
    /// its writes `laid` held out of the generator's ground, the ledger `rows`
    /// its arrival holds laid in firing order (a block at its cell, a
    /// container's item into the container's compound, a body into the
    /// entity file), its block entities, its light and `day_time`.
    pub fn encode_blocks(
        &self,
        blocks: &BlockMap,
        laid: &[Region],
        day_time: i64,
        rows: &[&crate::compiler::placed::Placed],
    ) -> (BlockMap, WrittenWorld) {
        use crate::compiler::placed::PlacedObject;
        let mut blocks = blocks.clone();
        let mut ground = self.ground.clone();
        for (lo, hi) in laid {
            ground.mark_box(*lo, *hi);
        }
        let mut items: BTreeMap<[i32; 3], Vec<(String, String)>> = BTreeMap::new();
        let mut entities: Vec<EntityRecord> = Vec::new();
        for r in rows {
            match &r.object {
                PlacedObject::Block { cell, state } => {
                    ground.mark(*cell);
                    if crate::compiler::assembled::base_id(state) == "minecraft:air" {
                        blocks.remove(cell);
                    } else {
                        blocks.insert(*cell, crate::compiler::blockstate::BlockState::new(state));
                    }
                }
                PlacedObject::Item { cell, slot, stack } => {
                    items
                        .entry(*cell)
                        .or_default()
                        .push((slot.clone(), stack.clone()));
                }
                PlacedObject::Entity { .. } => entities.extend(r.entity_record()),
                PlacedObject::Removed { .. } => {}
            }
        }
        let mut bes = self.block_entities(&blocks);
        for (cell, stacks) in &items {
            // An item goes into the container's block entity; a cell whose
            // block carries none holds no item, on the server as here.
            let Some(entry) = bes.get_mut(cell) else {
                continue;
            };
            let mut list: Vec<Nbt> = match entry.get("Items") {
                Some(Nbt::List(l)) => l.clone(),
                _ => Vec::new(),
            };
            for (slot, stack) in stacks {
                if let Some(item) = item_compound(slot, stack) {
                    list.push(item);
                }
            }
            entry.set("Items", Nbt::List(list));
        }
        let biome = |c: [i32; 3]| self.biome.at(c).0.to_string();
        let w = crate::compiler::view::world::encode_world(&WorldInput {
            blocks: &blocks,
            biome: &biome,
            spawn: self.spawn,
            ground: &ground,
            extent: Some(self.extent),
            block_entities: &bes,
            entities: &entities,
            day_time: Some(day_time),
        });
        (blocks, w)
    }

    /// A configuration's world over `picture`, at `clock`, with the ledger
    /// rows `rows` laid.
    pub fn encode(
        &self,
        plan: &Plan,
        picture: &BlockMap,
        config: &Configuration,
        clock: &Clock,
        rows: &[&crate::compiler::placed::Placed],
    ) -> (BlockMap, WrittenWorld) {
        self.encode_blocks(
            &config.blocks(picture),
            &config.laid_regions(),
            clock.day_time(plan),
            rows,
        )
    }
}

/// A container slot's item compound from an `item replace` stack:
/// `container.<n>` and `<id>[<components>] <count>`. `None` for a slot that is
/// no container slot or a stack that does not read.
fn item_compound(slot: &str, stack: &str) -> Option<Nbt> {
    let n: i8 = slot.strip_prefix("container.")?.parse().ok()?;
    let (item, count) = stack.rsplit_once(' ')?;
    let count: i32 = count.parse().ok()?;
    let (id, comps) = match item.split_once('[') {
        Some((id, rest)) => (id, Some(rest.strip_suffix(']')?)),
        None => (item, None),
    };
    let mut c = vec![
        ("Slot".to_string(), Nbt::Byte(n)),
        ("id".to_string(), Nbt::Str(id.to_string())),
        ("count".to_string(), Nbt::Int(count)),
    ];
    if let Some(comps) = comps {
        let mut entries = Vec::new();
        for part in split_top_level(comps) {
            let (k, v) = part.split_once('=')?;
            let key = if k.contains(':') {
                k.to_string()
            } else {
                format!("minecraft:{k}")
            };
            entries.push((key, crate::compiler::placed::parse_snbt(v).ok()?));
        }
        c.push(("components".to_string(), Nbt::Compound(entries)));
    }
    Some(Nbt::Compound(c))
}

/// Split `text` at the commas no bracket, brace or quote encloses.
fn split_top_level(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut depth, mut quote, mut start) = (0i32, None::<char>, 0usize);
    let mut escaped = false;
    for (i, ch) in text.char_indices() {
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == q {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => quote = Some(ch),
            '[' | '{' => depth += 1,
            ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                out.push(&text[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}

/// A body a world holds, as a frame shows it: the target (kind `body`, named
/// by its first tag that is no class tag and its cell), its entity kind without
/// the namespace, and the compass direction it faces.
pub struct BodyTarget {
    pub target: crate::compiler::framing::Target,
    pub kind: String,
    pub facing: &'static str,
}

/// The bodies of `rows` as targets a frame can show (spec-0112 §7.5).
pub fn body_targets(rows: &[&crate::compiler::placed::Placed]) -> Vec<BodyTarget> {
    // The tags that say what class of body a thing is, not which one.
    let class = [
        crate::compiler::affordance::FIXTURE_TAG,
        crate::compiler::affordance::BORNE_TAG,
        "dw_npc",
        "dw_marker",
        "dw_actor",
        "dw_watch",
        "dw_unseen",
    ];
    // An entity the game draws nothing for is no body a frame can show: a
    // hitbox (`interaction`) and a `marker` carry a body's tags, not its looks.
    let invisible = ["minecraft:interaction", "minecraft:marker"];
    let mut out: Vec<BodyTarget> = Vec::new();
    for r in rows {
        let Some(e) = r.entity_record() else { continue };
        if invisible.contains(&e.kind.as_str()) {
            continue;
        }
        // A display entity with nothing to display draws nothing in the game
        // either: an assembly's root `item_display`, which carries no item.
        let shows_nothing = match e.kind.as_str() {
            "minecraft:item_display" => e.nbt.get("item").is_none(),
            "minecraft:block_display" => e.nbt.get("block_state").is_none(),
            "minecraft:text_display" => e.nbt.get("text").is_none(),
            _ => false,
        };
        if shows_nothing {
            continue;
        }
        let id = e
            .tags
            .iter()
            .find(|t| !class.contains(&t.as_str()))
            .cloned()
            .unwrap_or_else(|| e.kind.clone());
        let cell = [
            e.pos[0].floor() as i32,
            e.pos[1].floor() as i32,
            e.pos[2].floor() as i32,
        ];
        let yaw = (f64::from(e.yaw)).rem_euclid(360.0);
        let facing = match ((yaw + 45.0) / 90.0).floor() as i32 % 4 {
            0 => "south",
            1 => "west",
            2 => "north",
            _ => "east",
        };
        let mut name = format!("{id}@{},{},{}", cell[0], cell[1], cell[2]);
        let same = out
            .iter()
            .filter(|b| b.target.id.starts_with(&name))
            .count();
        if same > 0 {
            name = format!("{name}#{}", same + 1);
        }
        out.push(BodyTarget {
            target: crate::compiler::framing::Target::point(name, "body", String::new(), cell),
            kind: e
                .kind
                .strip_prefix("minecraft:")
                .unwrap_or(&e.kind)
                .to_string(),
            facing,
        });
    }
    out
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
        pos: [
            p.first()?.as_f64()?,
            p.get(1)?.as_f64()?,
            p.get(2)?.as_f64()?,
        ],
        yaw: cam["yaw"].as_f64()?,
        pitch: cam["pitch"].as_f64()?,
        fov: cam["fov"]
            .as_f64()
            .unwrap_or(crate::compiler::render_plan::DEFAULT_FOV_DEG),
    })
}

/// One target as a shot's `shows` lists it: the manifest's entry, which the
/// frame copies verbatim, with the body's kind and facing where it is one.
fn shown(r: &crate::compiler::framing::ResolvedTarget<'_>, bodies: &[BodyTarget]) -> Value {
    let mut v = r.entry();
    if let Some(b) = bodies.iter().find(|b| b.target == *r.target()) {
        v["body"] = json!(b.kind);
        v["facing"] = json!(b.facing);
    }
    v
}

/// What one shot shows (spec-0112 §7.5): its `shows` and `out_of_frame`, by
/// the one sight rule ([`crate::compiler::framing::resolve_targets`]) over the
/// grid of the world it stands in.
pub fn frame_row(
    grid: &crate::compiler::framing::VoxelGrid,
    cam: &crate::compiler::framing::Camera,
    size: u32,
    targets: &[crate::compiler::framing::Target],
    bodies: &[BodyTarget],
) -> (Vec<Value>, Vec<Value>) {
    let (inside, outside) =
        crate::compiler::framing::resolve_targets(grid, cam, size, size, targets);
    (
        inside.iter().map(|r| shown(r, bodies)).collect(),
        outside
            .iter()
            .map(|t| crate::compiler::framing::target_json(t))
            .collect(),
    )
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
/// What a build's review set is made of beyond its plan and picture: the
/// worlds its frames stand in, the placements ledger they lay, and the
/// cutscene proof whose shots are framed.
pub struct ReviewOf<'a> {
    pub frames: &'a FrameWorlds,
    pub ledger: &'a crate::compiler::placed::Ledger,
    pub cutscenes: &'a crate::compiler::nav::CutsceneProof,
}

pub fn write_review(
    plan: &Plan,
    picture: &BlockMap,
    maker: &WorldMaker,
    of: ReviewOf<'_>,
    doc: &Value,
    out: &mut BTreeMap<String, Vec<u8>>,
) -> Review {
    let ReviewOf {
        frames,
        ledger,
        cutscenes,
    } = of;
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
        let rows: Vec<&crate::compiler::placed::Placed> =
            fw.rows.iter().map(|i| &ledger.rows[*i]).collect();
        let (blocks, w) = maker.encode(plan, picture, &fw.config, &fw.clock, &rows);
        let bodies = body_targets(&rows);
        let mut targets = targets.clone();
        targets.extend(bodies.iter().map(|b| b.target.clone()));
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
            let (shows, out_of_frame) = frame_row(&grid, &cam, size, &targets, &bodies);
            shot_rows.insert(
                i,
                json!({
                    "id": s["id"],
                    "world": key,
                    "mode": s["mode"],
                    "sky": s["sky"],
                    "size": size,
                    "shows": shows,
                    "out_of_frame": out_of_frame,
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
    let light_quick = shots.iter().filter(|s| light_question(s)).count();
    let cutscene_frames = shots.iter().filter(|s| s["kind"] == "cutscene").count();
    let framed_shots: std::collections::BTreeSet<(usize, usize, usize)> = cutscenes
        .frames
        .iter()
        .map(|f| (f.step, f.n, f.shot))
        .collect();
    let framed: std::collections::BTreeSet<(usize, usize)> =
        framed_shots.iter().map(|(s, n, _)| (*s, *n)).collect();
    let framed_names: Vec<String> = framed
        .iter()
        .map(|(s, n)| {
            let id = plan
                .critical_path
                .get(*s)
                .and_then(crate::compiler::view::beat::step_id)
                .map_or_else(|| format!("step-{}", s + 1), str::to_string);
            format!("{id} #{n}")
        })
        .collect();
    lines.insert(
        0,
        format!(
            "review frames: {} shot(s) in {} world(s) ({at_load} at load, {} after a step); \
             {quick} quick, {} hq, {light_quick} light frame(s) rendered quick; \
             {cutscene_frames} cutscene frame(s) of {} shot(s) in {} cutscene(s), {} cutscene(s) \
             no critical step fires{}; cameras proven clear: {} of {} in their own worlds",
            rows.len(),
            keys.len(),
            rows.len() - at_load,
            rows.len() - quick,
            framed_shots.len(),
            framed.len(),
            cutscenes.unfired.len(),
            if cutscenes.unfired.is_empty() {
                String::new()
            } else {
                format!(": {}", cutscenes.unfired.join(", "))
            },
            doc["camera_eye_proof"]["cameras"].as_u64().unwrap_or(0),
            shots.len(),
        ),
    );
    let record = json!({
        "worlds": worlds_json,
        "cutscenes": {
            "framed": framed_names,
            "unfired": cutscenes.unfired,
        },
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
        || shot["sky"]["weather"]
            .as_str()
            .is_some_and(|w| w != "clear")
}

/// What no review frame shows, by name (spec-0112 §11).
pub const UNCHECKED: [&str; 5] = [
    "beds, shulker boxes, the conduit, open lids, a ringing bell, italic or obfuscated sign text",
    "the weather in the air, clouds, hq-mode fog and the moon",
    "a night-vision room's lightmap: judge in game under night vision",
    "a body's pose at a tick, the party and its stand-ins, a collapse's debris, a set-atmosphere repaint, a flood's spread past its laid cells, an unforced write",
    "a cutscene no critical step fires",
];
