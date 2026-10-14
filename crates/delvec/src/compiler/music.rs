//! A scene states its music (spec-0109): what every biome a camera can stand
//! in plays, what each cutscene's camera hears, what each `stop-sound` cuts,
//! and the binding and ledger every build writes about them.
//!
//! The pinned client reads music from the biome under the camera and nothing
//! else in a loaded world (`Minecraft.getSituationalMusic`). The engine already
//! owns that biome: the atmospheres a campaign declares and the ground the
//! horizon lays ([`crate::compiler::horizon::biome_map`]). The document tier
//! (`DW1031`, `dsl::music`) makes every one of them state its music; this
//! module reads the statements back as the client will meet them:
//!
//! * per biome, the word for what a playing track does when the camera enters
//!   it (`replaces`, `plays-out`, `fades`, `plays-out-at-gain` — spec-0109
//!   §3.1's table), and the first route cell standing in it, the bot's
//!   listening station;
//! * per cutscene, the biome every 4-cell its camera passes through wears in
//!   the configuration the forced route reaches it in — the biome map after
//!   each `set-atmosphere` in route order, the reading the fog proof takes —
//!   refused (`DW1032`) where it declared `hears` and a cell wears another;
//! * per `stop-sound`, the biome the forced route stands in when it fires and
//!   what the cut is there: the cut of a play-out, a stinger's cut, or a
//!   hiccup in a scene whose track restarts.
//!
//! No instrument outside a client observes the music manager, so the fade, the
//! loop seam and the toast are not claimed here (spec-0109 §5.3).

use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::{DwCode, ExitTier, QuestEffect, SoundCategory, Verb};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::compiler::failure::Failure;
use crate::compiler::horizon::{BiomeMap, PaintSource};
use crate::compiler::nav::{ActorMovePlan, LegRoute, MovePlan};
use crate::compiler::plan::{EffectRoot, Plan};
use crate::compiler::pulse::Station;

delvewright_dsl::dw_code! {
    /// `DW1032`: **a cutscene that hears what it did not declare** (spec-0109
    /// §6.2). A cutscene with `hears` whose camera passes a 4-cell wearing
    /// another biome in the judged configuration. Build tier.
    pub const DW_CUTSCENE_HEARS: DwCode = DwCode::new("DW1032", ExitTier::Build);
}

/// What a playing track does when the camera enters a biome (spec-0109 §3.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entering {
    /// A track with `replace_current_music: true`: the playing one is cut and
    /// the declared one starts.
    Replaces,
    /// A track without replace: the playing one plays to its end.
    PlaysOut,
    /// Silence at `music_volume` 0: the playing track fades out and stops.
    Fades,
    /// Silence at a gain above 0: the playing track plays to its end at it.
    PlaysOutAtGain,
}

impl Entering {
    /// The ledger's word.
    pub fn word(self) -> &'static str {
        match self {
            Entering::Replaces => "replaces",
            Entering::PlaysOut => "plays-out",
            Entering::Fades => "fades",
            Entering::PlaysOutAtGain => "plays-out-at-gain",
        }
    }

    /// Whether the biome declares silence above water.
    pub fn silent(self) -> bool {
        matches!(self, Entering::Fades | Entering::PlaysOutAtGain)
    }
}

/// `audio/music_volume` as the client resolves it over the attribute's default
/// (1.0): a plain number, or vanilla's float modifier form applied to 1.0;
/// clamped to the attribute's `[0, 1]`.
fn effective_volume(v: Option<&Value>) -> f64 {
    let Some(v) = v else {
        return 1.0;
    };
    let x = if let Some(x) = v.as_f64() {
        x
    } else {
        let arg = v.get("argument").and_then(Value::as_f64);
        match (v.get("modifier").and_then(Value::as_str), arg) {
            (Some("override"), Some(a)) => a,
            (Some("add"), Some(a)) => 1.0 + a,
            (Some("subtract"), Some(a)) => 1.0 - a,
            (Some("multiply"), Some(a)) => a,
            (Some("minimum"), Some(a)) => a.min(1.0),
            (Some("maximum"), Some(a)) => a.max(1.0),
            _ => 1.0,
        }
    };
    x.clamp(0.0, 1.0)
}

/// One biome's music statement, read from an attribute map.
#[derive(Clone, Debug, PartialEq)]
pub struct Statement {
    /// The canonical `audio/background_music` value (`{}` for silence).
    pub music: Value,
    /// The declared `audio/music_volume`, as written.
    pub music_volume: Option<Value>,
    /// What a playing track does on entering.
    pub entering: Entering,
}

/// Read an attribute map's music statement through the one attribute reader.
/// `None` when it declares no `audio/background_music` (`DW1031`).
pub fn statement(attributes: &BTreeMap<String, Value>) -> Option<Statement> {
    let raw = delvewright_dsl::music::background_music(attributes)?;
    let reg = crate::compiler::atmosphere::Registry::v1_21_11();
    let music = reg
        .row(delvewright_dsl::music::BACKGROUND_MUSIC)
        .and_then(|row| reg.value(row, raw).ok())
        .unwrap_or_else(|| raw.clone());
    // The declared gain in its canonical form (the biome file's), so `0` and
    // `0.0` are one value here too.
    let music_volume = attributes
        .iter()
        .find(|(k, _)| delvewright_dsl::music::canonical_attribute(k) == "audio/music_volume")
        .map(|(_, v)| {
            reg.row("audio/music_volume")
                .and_then(|row| reg.value(row, v).ok())
                .unwrap_or_else(|| v.clone())
        });
    let entering = match music.get("default") {
        Some(track) => {
            if track
                .get("replace_current_music")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                Entering::Replaces
            } else {
                Entering::PlaysOut
            }
        }
        None => {
            if effective_volume(music_volume.as_ref()) == 0.0 {
                Entering::Fades
            } else {
                Entering::PlaysOutAtGain
            }
        }
    };
    Some(Statement {
        music,
        music_volume,
        entering,
    })
}

/// The music a valley band's vanilla biome carries, read from the pinned
/// 1.21.11 server jar (`data/minecraft/worldgen/biome/<name>.json`; the
/// overworld dimension type `data/minecraft/dimension_type/overworld.json` for
/// what a biome declaring none inherits). `(declares, hears)`: the biome's own
/// `audio/background_music`, or `None` when it declares none and the
/// dimension's plays.
fn band_music(biome: &str) -> (Option<Value>, Value) {
    let dimension = json!({
        "creative": { "max_delay": 24000, "min_delay": 12000, "sound": "minecraft:music.creative" },
        "default": { "max_delay": 24000, "min_delay": 12000, "sound": "minecraft:music.game" }
    });
    match biome {
        "minecraft:cherry_grove" => {
            let own = json!({
                "default": {
                    "max_delay": 24000,
                    "min_delay": 12000,
                    "sound": "minecraft:music.overworld.cherry_grove"
                }
            });
            (Some(own.clone()), own)
        }
        _ => (None, dimension),
    }
}

/// One biome a camera can stand in.
#[derive(Clone, Debug, PartialEq)]
pub struct BiomeRow {
    /// The namespaced biome id.
    pub id: String,
    /// `atmosphere`, `ground` or `band`.
    pub kind: &'static str,
    /// The atmosphere id, for an atmosphere.
    pub atmosphere: Option<String>,
    /// The campaign's statement; `None` for a band (undeclared by the
    /// campaign).
    pub statement: Option<Statement>,
    /// For a band, the vanilla declaration and what it plays.
    pub vanilla: Option<(Option<Value>, Value)>,
    /// The first route cell whose eye stands in it, in the configuration of
    /// its leg.
    pub station: Option<Station>,
}

/// One cutscene, judged.
#[derive(Clone, Debug, PartialEq)]
pub struct CutsceneRow {
    /// The stage document and JSON pointer of the effect.
    pub stage: &'static str,
    /// The effect's JSON pointer.
    pub path: String,
    /// The declaration: `None` absent, `Some(None)` the ground, `Some(Some)`
    /// an atmosphere id.
    pub hears: Option<Option<String>>,
    /// Whether the forced route reaches it.
    pub reached: bool,
    /// The repaints of the judged configuration, as `set-atmosphere` paths in
    /// route order.
    pub repaints: Vec<String>,
    /// The 4-cells judged, in camera order, with the biome each wears.
    pub cells: Vec<([i32; 3], String, usize)>,
    /// The first cell wearing a biome other than the declared one, with its
    /// shot.
    pub refusal: Option<(usize, [i32; 3], String, String)>,
}

impl CutsceneRow {
    /// `confirmed`, `refused` or `undeclared`.
    pub fn verdict(&self) -> &'static str {
        match (&self.hears, &self.refusal) {
            (None, _) => "undeclared",
            (Some(_), None) => "confirmed",
            (Some(_), Some(_)) => "refused",
        }
    }
}

/// What a `stop-sound` cuts where it fires.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cut {
    /// It reaches the music in a scene that declares silence or a spaced
    /// track: the play-out stops, and nothing starts before the scene's own
    /// delay (never, for silence).
    Cut,
    /// It cannot reach the music manager's track: a played sound's cut.
    Stinger,
    /// It reaches the music in a scene whose track restarts at once
    /// ([`RESTART_TICKS`]): a hiccup of a few ticks.
    Hiccup,
}

impl Cut {
    /// The ledger's word.
    pub fn word(self) -> &'static str {
        match self {
            Cut::Cut => "cut",
            Cut::Stinger => "stinger",
            Cut::Hiccup => "hiccup",
        }
    }
}

/// One `stop-sound`.
#[derive(Clone, Debug, PartialEq)]
pub struct StopRow {
    /// The stage document.
    pub stage: &'static str,
    /// The effect's JSON pointer.
    pub path: String,
    /// The declared category.
    pub category: Option<SoundCategory>,
    /// The declared sound, as written.
    pub sound: Option<String>,
    /// The effect's declared audience (`None`: its root's).
    pub audience: Option<Value>,
    /// The completion-marker token of the bundle that fires it, or `None`.
    pub after: Option<String>,
    /// The biome the forced route stands in when it fires, or `None` when the
    /// route does not perform it.
    pub biome: Option<String>,
    /// What it cuts there.
    pub cut: Option<Cut>,
}

/// One played sound's category.
#[derive(Clone, Debug, PartialEq)]
pub struct CategoryRow {
    /// `play-sound` or `pulse`.
    pub kind: &'static str,
    /// The stage document.
    pub stage: &'static str,
    /// The JSON pointer.
    pub path: String,
    /// The category written.
    pub category: SoundCategory,
}

/// The music ledger (spec-0109 §5.1).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MusicLedger {
    /// Every biome a camera can stand in.
    pub biomes: Vec<BiomeRow>,
    /// Every cutscene.
    pub cutscenes: Vec<CutsceneRow>,
    /// Every `stop-sound`.
    pub stops: Vec<StopRow>,
    /// Every played sound and its category.
    pub categorised: Vec<CategoryRow>,
    /// The canonical sha256 of the declarations the ledger read
    /// ([`measured_hash`]).
    pub measured_sha256: String,
}

impl MusicLedger {
    /// The binding line every build prints (spec-0109 §5.1).
    pub fn line(&self) -> String {
        let declared: Vec<&BiomeRow> = self
            .biomes
            .iter()
            .filter(|b| b.statement.is_some())
            .collect();
        let a = declared.len();
        let t = declared
            .iter()
            .filter(|b| b.statement.as_ref().is_some_and(|s| !s.entering.silent()))
            .count();
        let f = declared
            .iter()
            .filter(|b| {
                b.statement
                    .as_ref()
                    .is_some_and(|s| s.entering == Entering::Fades)
            })
            .count();
        let o = declared
            .iter()
            .filter(|b| {
                b.statement
                    .as_ref()
                    .is_some_and(|s| s.entering == Entering::PlaysOutAtGain)
            })
            .count();
        let u = self.biomes.iter().filter(|b| b.kind == "band").count();
        let c = self.cutscenes.len();
        let h = self.cutscenes.iter().filter(|r| r.hears.is_some()).count();
        let g = self
            .cutscenes
            .iter()
            .filter(|r| r.verdict() == "confirmed")
            .count();
        let k = self.stops.len();
        let cut = |w: Cut| self.stops.iter().filter(|s| s.cut == Some(w)).count();
        let p = self
            .categorised
            .iter()
            .filter(|r| r.category != SoundCategory::Master)
            .count();
        format!(
            "music binding: {} biome(s) a camera can stand in — {a} atmosphere(s) and the ground \
             declared ({t} with a track, {} silent: {f} fading, {o} playing out), {u} band \
             biome(s) undeclared; {c} cutscene(s), {h} declare what they hear, {g} of {h} \
             confirmed; {k} stop-sound effect(s), {} of {k} cut a play-out, {} a stinger, {} a \
             hiccup; {p} sound(s) in a category other than master",
            self.biomes.len(),
            f + o,
            cut(Cut::Cut),
            cut(Cut::Stinger),
            cut(Cut::Hiccup),
        )
    }

    /// `validation/music.json` (spec-0109 §5.1).
    pub fn to_json(&self) -> Value {
        let station = |s: &Option<Station>| match s {
            Some(s) => json!({ "cell": s.cell, "step": s.step, "before": s.before }),
            None => Value::Null,
        };
        json!({
            "measured_sha256": self.measured_sha256,
            "biomes": self.biomes.iter().map(|b| {
                let mut row = json!({
                    "id": b.id,
                    "kind": b.kind,
                    "atmosphere": b.atmosphere,
                    "station": station(&b.station),
                    "not_stood": if b.station.is_some() { Value::Null } else { json!("no station") },
                });
                let o = row.as_object_mut().expect("a row is an object");
                match (&b.statement, &b.vanilla) {
                    (Some(s), _) => {
                        o.insert("music".into(), music_json(&s.music));
                        o.insert("music_volume".into(), s.music_volume.clone().unwrap_or(Value::Null));
                        o.insert("entering".into(), json!(s.entering.word()));
                    }
                    (None, Some((declares, hears))) => {
                        o.insert("music".into(), music_json(hears));
                        o.insert("music_volume".into(), Value::Null);
                        o.insert("entering".into(), json!(entering_of_track(hears)));
                        o.insert("vanilla".into(), json!({
                            "declares": declares,
                            "inherits_dimension": declares.is_none(),
                        }));
                    }
                    (None, None) => {}
                }
                row
            }).collect::<Vec<_>>(),
            "cutscenes": self.cutscenes.iter().map(|r| {
                let mut wears: BTreeMap<&str, usize> = BTreeMap::new();
                for (_, biome, _) in &r.cells {
                    *wears.entry(biome.as_str()).or_default() += 1;
                }
                json!({
                    "stage": r.stage,
                    "effect": r.path,
                    "hears": match &r.hears {
                        None => json!("absent"),
                        Some(None) => Value::Null,
                        Some(Some(id)) => json!(id),
                    },
                    "configuration": {
                        "reached": r.reached,
                        "judged_at": if r.reached { "where the forced route reaches it" } else { "the first tick (the forced route never reaches it)" },
                        "repaints": r.repaints,
                    },
                    "cells_judged": r.cells.len(),
                    "wears": wears.iter().map(|(b, n)| json!({ "biome": b, "cells": n })).collect::<Vec<_>>(),
                    "verdict": r.verdict(),
                    "refused_at": r.refusal.as_ref().map(|(shot, cell, wears, declared)| json!({
                        "shot": shot, "cell": cell, "wears": wears, "declared": declared,
                    })),
                    "not_stood": "cutscene",
                })
            }).collect::<Vec<_>>(),
            "stop_sounds": self.stops.iter().map(|s| json!({
                "stage": s.stage,
                "effect": s.path,
                "category": s.category.map(SoundCategory::token),
                "sound": s.sound.as_ref().map(|x| namespaced(x)),
                "audience": s.audience,
                "after": s.after,
                "biome": s.biome,
                "cut": s.cut.map(Cut::word),
                "performed": s.biome.is_some(),
            })).collect::<Vec<_>>(),
            "categories": self.categorised.iter().map(|r| json!({
                "kind": r.kind,
                "stage": r.stage,
                "path": r.path,
                "category": r.category.token(),
            })).collect::<Vec<_>>(),
        })
    }
}

fn namespaced(s: &str) -> String {
    if s.contains(':') {
        s.to_string()
    } else {
        format!("minecraft:{s}")
    }
}

/// `silence`, or the `default` / `underwater` tracks.
fn music_json(music: &Value) -> Value {
    let pick = |k: &str| music.get(k).cloned();
    match (pick("default"), pick("underwater")) {
        (None, None) => json!("silence"),
        (d, u) => json!({ "default": d, "underwater": u }),
    }
}

fn entering_of_track(music: &Value) -> &'static str {
    match music.get("default") {
        Some(t) if t.get("replace_current_music").and_then(Value::as_bool) == Some(true) => {
            "replaces"
        }
        Some(_) => "plays-out",
        None => "plays-out-at-gain",
    }
}

// ---------------------------------------------------------------------------
// The configurations the forced route stands in
// ---------------------------------------------------------------------------

/// The critical-path step a root's bundle fires at on the forced route, or
/// `None` when the route never fires it (a trap, a respawn, a trigger the
/// path does not perform).
fn root_firing(plan: &Plan, root: &EffectRoot<'_>) -> Option<usize> {
    match root {
        EffectRoot::ObjectiveComplete { objective, .. } => {
            plan.objective_steps.get(*objective).copied()
        }
        EffectRoot::QuestComplete(q) => q
            .objectives
            .iter()
            .any(|o| plan.objective_steps.contains_key(o.id().as_str()))
            .then(|| crate::compiler::plan::root_step(plan, root)),
        EffectRoot::Trigger(t) => plan
            .critical_path
            .iter()
            .position(|s| s.trigger() == Some(t.id.as_str())),
        _ => None,
    }
}

/// One effect the music reading orders along the route.
struct Event<'a> {
    /// `(step, walk index)` when the route fires it.
    key: Option<(usize, usize)>,
    stage: &'static str,
    path: String,
    effect: &'a QuestEffect,
    /// The completion-marker token the bundle that fires it broadcasts, or
    /// `None` when nothing the bot sees marks the moment (the reading
    /// `validation/atmosphere-repaints.json` takes).
    after: Option<String>,
}

fn events<'a>(plan: &Plan<'a>) -> Vec<Event<'a>> {
    let mut out = Vec::new();
    let mut n = 0usize;
    crate::compiler::plan::for_each_gate_effect(plan.campaign, &mut |site, e| {
        let key = root_firing(plan, &site.root).map(|s| (s, n));
        n += 1;
        // A nested list fires at a moment its root's marker does not mark: a
        // `sequence` step at its offset, an arrival, a rest, a respawn, a catch.
        let top_level = ![
            "/steps/",
            "/on_respawn/",
            "/on_rest/",
            "/on_caught/",
            "/on_arrive/",
        ]
        .iter()
        .any(|seg| site.path.contains(seg));
        let after = if e.when.is_some() || !top_level {
            None
        } else {
            match &site.root {
                EffectRoot::ObjectiveComplete { objective, .. } => Some(objective.to_string()),
                EffectRoot::Trigger(t) if crate::compiler::plan::trigger_may_be_performed(t) => {
                    Some(t.id.as_str().to_string())
                }
                _ => None,
            }
        };
        out.push(Event {
            key,
            stage: site.stage,
            path: site.path.clone(),
            effect: e,
            after,
        });
    });
    out
}

/// The biome map with every repaint the route has fired before `key` (all
/// those strictly before it in route order), and the repaints' paths.
fn configuration(
    plan: &Plan<'_>,
    first: &BiomeMap,
    events: &[Event<'_>],
    before: Option<(usize, usize)>,
) -> (BiomeMap, Vec<String>) {
    let Some(before) = before else {
        return (first.clone(), Vec::new());
    };
    let mut fired: Vec<&Event<'_>> = events
        .iter()
        .filter(|e| matches!(e.effect.verb, Verb::SetAtmosphere { .. }))
        .filter(|e| e.key.is_some_and(|k| k < before))
        .collect();
    fired.sort_by_key(|e| e.key);
    let mut map = first.clone();
    let mut paths = Vec::new();
    for e in fired {
        let Verb::SetAtmosphere { atmosphere, .. } = &e.effect.verb else {
            continue;
        };
        let Some((lo, hi)) = crate::compiler::horizon::repaint_volume(plan, e.effect) else {
            continue;
        };
        let biome = first.biome_of(atmosphere.as_ref().map(|a| a.as_str()));
        map = map.repainted(
            crate::compiler::atmosphere::painted_box(lo, hi),
            &biome,
            PaintSource::Band,
        );
        paths.push(e.path.clone());
    }
    (map, paths)
}

/// The biome a block cell's 4-cell wears, read at the cell's 2-mod-4 sample.
fn wears(map: &BiomeMap, quart: [i32; 3]) -> String {
    map.at(sample(quart)).0.to_string()
}

/// The 2-mod-4 block of a 4-cell — the block that always reads its own cell.
pub fn sample(quart: [i32; 3]) -> [i32; 3] {
    [quart[0] * 4 + 2, quart[1] * 4 + 2, quart[2] * 4 + 2]
}

fn quart_of(p: [f64; 3]) -> [i32; 3] {
    std::array::from_fn(|i| (p[i] / 4.0).floor() as i32)
}

/// Every 4-cell a straight segment from `a` to `b` crosses, in order, its
/// endpoints' cells included (a voxel walk over the 4-block grid).
fn segment_quarts(a: [f64; 3], b: [f64; 3], out: &mut Vec<[i32; 3]>) {
    let mut cell = quart_of(a);
    let end = quart_of(b);
    out.push(cell);
    let d: [f64; 3] = std::array::from_fn(|i| (b[i] - a[i]) / 4.0);
    let p: [f64; 3] = std::array::from_fn(|i| a[i] / 4.0);
    let step: [i32; 3] = std::array::from_fn(|i| {
        if d[i] > 0.0 {
            1
        } else if d[i] < 0.0 {
            -1
        } else {
            0
        }
    });
    let mut t_max: [f64; 3] = std::array::from_fn(|i| {
        if step[i] == 0 {
            f64::INFINITY
        } else {
            let edge = if step[i] > 0 {
                f64::from(cell[i] + 1)
            } else {
                f64::from(cell[i])
            };
            (edge - p[i]) / d[i]
        }
    });
    let t_delta: [f64; 3] = std::array::from_fn(|i| {
        if step[i] == 0 {
            f64::INFINITY
        } else {
            1.0 / d[i].abs()
        }
    });
    // A segment crosses at most |Δ| cells per axis; the bound keeps a
    // degenerate float from walking forever.
    let budget: i32 = (0..3).map(|i| (end[i] - cell[i]).abs()).sum::<i32>() + 3;
    for _ in 0..budget {
        if cell == end {
            break;
        }
        let axis = (0..3)
            .min_by(|&x, &y| t_max[x].total_cmp(&t_max[y]))
            .expect("three axes");
        if t_max[axis] > 1.0 {
            break;
        }
        cell[axis] += step[axis];
        t_max[axis] += t_delta[axis];
        out.push(cell);
    }
    if *out.last().expect("pushed") != end {
        out.push(end);
    }
}

/// Every 4-cell a camera polyline passes through — its waypoints and every
/// cell the straight segments between them cross — in camera order, each once.
pub fn polyline_quarts(pts: &[[f64; 3]]) -> Vec<[i32; 3]> {
    let mut raw = Vec::new();
    match pts {
        [] => {}
        [p] => raw.push(quart_of(*p)),
        _ => {
            for w in pts.windows(2) {
                segment_quarts(w[0], w[1], &mut raw);
            }
        }
    }
    let mut seen = BTreeSet::new();
    raw.into_iter().filter(|c| seen.insert(*c)).collect()
}

/// One cutscene's judgement: the configuration it is heard in and the 4-cells
/// its camera passes through, read by the build (`DW1032`) and by its PackTest
/// template (`music_hears_<n>`) alike.
#[derive(Clone, Debug, PartialEq)]
pub struct Judged {
    /// The row the ledger carries.
    pub row: CutsceneRow,
    /// The biome the declaration names, for a cutscene with `hears`.
    pub declared: Option<String>,
    /// The repaint volumes and biomes of the judged configuration, in order:
    /// `(min, max, biome)` in block corners, the lines the verb emits.
    pub repaint_volumes: Vec<([i32; 3], [i32; 3], String)>,
}

/// Judge every cutscene (spec-0109 §3.5), in effect-walk order.
pub fn judge_cutscenes(
    plan: &Plan<'_>,
    moves: &[MovePlan],
    actor_moves: &[ActorMovePlan],
) -> Vec<Judged> {
    let first = crate::compiler::horizon::biome_map(plan);
    let evs = events(plan);
    let units = crate::compiler::camera::cutscene_units(plan.campaign);
    let mut out = Vec::new();
    for ev in &evs {
        let Verb::Cutscene { hears, .. } = &ev.effect.verb else {
            continue;
        };
        let Some(shots) = ev.effect.cutscene_shots() else {
            continue;
        };
        let ctx = units
            .iter()
            .find(|(e, _)| std::ptr::eq(*e, ev.effect))
            .map(|(_, c)| c.clone())
            .unwrap_or_default();
        let (map, repaints) = configuration(plan, &first, &evs, ev.key);
        let repaint_volumes = repaint_volumes(plan, &first, &evs, ev.key);
        let declared = hears
            .as_ref()
            .map(|h| first.biome_of(h.as_ref().map(|a| a.as_str())));
        let mut cells: Vec<([i32; 3], String, usize)> = Vec::new();
        let mut seen: BTreeSet<[i32; 3]> = BTreeSet::new();
        let mut refusal = None;
        let mut offset = 0i32;
        for (si, shot) in shots.iter().enumerate() {
            let ex =
                crate::compiler::camera::expand_shot(plan, moves, actor_moves, shot, &ctx, offset);
            offset += ex.ticks + 1;
            for q in polyline_quarts(ex.clip_polyline()) {
                if !seen.insert(q) {
                    continue;
                }
                let biome = wears(&map, q);
                if refusal.is_none()
                    && let Some(d) = &declared
                    && *d != biome
                {
                    refusal = Some((si, q, biome.clone(), d.clone()));
                }
                cells.push((q, biome, si));
            }
        }
        out.push(Judged {
            row: CutsceneRow {
                stage: ev.stage,
                path: ev.path.clone(),
                hears: hears
                    .as_ref()
                    .map(|h| h.as_ref().map(|a| a.as_str().to_string())),
                reached: ev.key.is_some(),
                repaints,
                cells,
                refusal,
            },
            declared,
            repaint_volumes,
        });
    }
    out
}

fn repaint_volumes(
    plan: &Plan<'_>,
    first: &BiomeMap,
    events: &[Event<'_>],
    before: Option<(usize, usize)>,
) -> Vec<([i32; 3], [i32; 3], String)> {
    let Some(before) = before else {
        return Vec::new();
    };
    let mut fired: Vec<&Event<'_>> = events
        .iter()
        .filter(|e| matches!(e.effect.verb, Verb::SetAtmosphere { .. }))
        .filter(|e| e.key.is_some_and(|k| k < before))
        .collect();
    fired.sort_by_key(|e| e.key);
    fired
        .into_iter()
        .filter_map(|e| {
            let Verb::SetAtmosphere { atmosphere, .. } = &e.effect.verb else {
                return None;
            };
            let (lo, hi) = crate::compiler::horizon::repaint_volume(plan, e.effect)?;
            Some((
                lo,
                hi,
                first.biome_of(atmosphere.as_ref().map(|a| a.as_str())),
            ))
        })
        .collect()
}

/// The longest delay, in ticks, after which a track the client re-arms is
/// still heard as the same scene's music resuming rather than as silence: the
/// `CONSTANT` frequency's own gap (`MusicManager$MusicFrequency`, 100 ticks).
pub const RESTART_TICKS: i64 = 100;

/// Whether a biome's track restarts at once after a cut (spec-0109 §2.3): it
/// declares a `default` track whose `max_delay` is at most [`RESTART_TICKS`],
/// so the client's countdown re-arms within five seconds. A spaced track waits
/// its declared delay, and silence never restarts: a cut there is a cut.
fn restarts_at_once(music: &Value) -> bool {
    music
        .get("default")
        .and_then(|t| t.get("max_delay"))
        .and_then(Value::as_i64)
        .is_some_and(|d| d <= RESTART_TICKS)
}

/// Whether a `stop-sound` reaches the music manager's track: the `music`
/// category, every category, or a named `music.*` event in any category.
fn reaches_music(category: Option<SoundCategory>, sound: Option<&str>) -> bool {
    match category {
        Some(SoundCategory::Music) => true,
        Some(_) => false,
        None => sound.is_none_or(|s| namespaced(s).starts_with("minecraft:music.")),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// The canonical sha256 of what a music reading measured: every declaration
/// the ledger reads, and nothing it does not — each atmosphere and the
/// horizon as declared, each place's carried atmosphere, and every
/// `set-atmosphere`, `cutscene` (its normalized shots and `hears`),
/// `stop-sound` and `play-sound` with its JSON path, and each pulse's
/// category. A change music does not read — a translated line, a field of
/// another mechanic, the other spelling of one cutscene — moves no byte of
/// the ledger, and a change it does read moves the hash.
///
/// A number is hashed as the value it is, so a float written `0` or `0.0` —
/// one value to every reader — is one declaration to the hash as well.
fn measured_hash(plan: &Plan<'_>, events: &[Event<'_>]) -> String {
    fn integral(v: Value) -> Value {
        match v {
            Value::Number(n) => match n.as_f64() {
                Some(f) if n.is_f64() && f.fract() == 0.0 && f.abs() < 9.0e15 => json!(f as i64),
                _ => Value::Number(n),
            },
            Value::Array(a) => Value::Array(a.into_iter().map(integral).collect()),
            Value::Object(o) => {
                Value::Object(o.into_iter().map(|(k, v)| (k, integral(v))).collect())
            }
            other => other,
        }
    }
    let c = plan.campaign;
    let effects: Vec<Value> = events
        .iter()
        .filter_map(|e| {
            let read = match &e.effect.verb {
                Verb::Cutscene { hears, .. } => json!({
                    "shots": e.effect.cutscene_shots(),
                    "hears": hears,
                }),
                Verb::SetAtmosphere { .. } | Verb::StopSound { .. } | Verb::PlaySound { .. } => {
                    serde_json::to_value(&e.effect.verb).ok()?
                }
                _ => return None,
            };
            Some(json!({ "path": e.path, "stage": e.stage, "read": read }))
        })
        .collect();
    let subject = json!({
        "atmospheres": c.world.content.atmospheres,
        "horizon": c.world.content.horizon,
        "places": crate::compiler::horizon::carried_places(c),
        "effects": effects,
        "pulses": c.quests.content.pulses.iter().map(|p| json!({
            "id": p.id.as_str(),
            "category": p.category.unwrap_or_default().token(),
        })).collect::<Vec<_>>(),
    });
    let text =
        delvewright_dsl::to_canonical_string(&integral(subject)).expect("a JSON value serialises");
    sha256_hex(text.as_bytes())
}

/// **Read every music statement back** (spec-0109 §5.1, §6.2) over the plan,
/// the forced route's legs (empty for a campaign that walks none) and the
/// planned moves a styled shot's subject rides. The ledger is returned beside
/// the verdict so the caller prints the line on every build, a refusal
/// included.
pub fn measure(
    plan: &Plan<'_>,
    routes: &[LegRoute],
    moves: &[MovePlan],
    actor_moves: &[ActorMovePlan],
) -> (MusicLedger, Result<(), Failure>) {
    let c = plan.campaign;
    let first = crate::compiler::horizon::biome_map(plan);
    let evs = events(plan);
    let mut ledger = MusicLedger {
        measured_sha256: measured_hash(plan, &evs),
        ..MusicLedger::default()
    };
    // The biomes: every declared atmosphere, the ground, and each band.
    for a in &c.world.content.atmospheres {
        ledger.biomes.push(BiomeRow {
            id: crate::compiler::atmosphere::biome_id(&plan.namespace, a.id.as_str()),
            kind: "atmosphere",
            atmosphere: Some(a.id.as_str().to_string()),
            statement: statement(&a.attributes),
            vanilla: None,
            station: None,
        });
    }
    let empty = BTreeMap::new();
    let ground_attrs = c
        .world
        .content
        .horizon
        .as_ref()
        .and_then(|h| h.attributes())
        .unwrap_or(&empty);
    ledger.biomes.push(BiomeRow {
        id: first.ground.id.clone(),
        kind: "ground",
        atmosphere: None,
        statement: statement(ground_attrs),
        vanilla: None,
        station: None,
    });
    let bands: BTreeSet<&str> = first
        .paints
        .iter()
        .filter(|p| p.source == PaintSource::Band)
        .map(|p| p.biome.as_str())
        .collect();
    for b in bands {
        ledger.biomes.push(BiomeRow {
            id: b.to_string(),
            kind: "band",
            atmosphere: None,
            statement: None,
            vanilla: Some(band_music(b)),
            station: None,
        });
    }
    // The listening stations: the first route cell whose eye stands in each
    // biome, the leg read in the configuration it is walked in (every repaint
    // fired at an earlier step).
    let token = |step: usize| {
        plan.critical_path
            .get(step)
            .and_then(|s| s.objective().or(s.trigger()))
            .map(str::to_string)
    };
    let mut leg_maps: BTreeMap<usize, BiomeMap> = BTreeMap::new();
    for leg in routes {
        let map = leg_maps
            .entry(leg.to_step)
            .or_insert_with(|| configuration(plan, &first, &evs, Some((leg.to_step, 0))).0)
            .clone();
        for &cell in &leg.cells {
            let eye = [cell[0], cell[1] + 1, cell[2]];
            let biome = map.at(eye).0;
            if let Some(row) = ledger
                .biomes
                .iter_mut()
                .find(|r| r.id == biome && r.station.is_none())
                && let Some(before) = token(leg.to_step)
            {
                row.station = Some(Station {
                    cell,
                    step: leg.to_step,
                    before,
                });
            }
        }
    }
    // The cutscenes.
    let judged = judge_cutscenes(plan, moves, actor_moves);
    let mut refusal: Option<Failure> = None;
    for j in &judged {
        if refusal.is_none()
            && let Some((shot, cell, wears, declared)) = &j.row.refusal
        {
            let at = sample(*cell);
            refusal = Some(Failure {
                code: DW_CUTSCENE_HEARS,
                message: format!(
                    "the cutscene at {} {} declares `hears: {}`, and shot {shot}'s camera passes \
                     the 4-cell {cell:?} (sampled at {at:?}), which wears `{wears}` in the \
                     configuration it is judged in ({}) — so the camera hears that biome's \
                     music, not the one declared (`{declared}`). Move the shot's path so it \
                     stays inside the declared place, carry the declared atmosphere on the place \
                     the camera crosses, or repaint the crossed volume with it in a beat before \
                     this one (`set-atmosphere`).",
                    j.row.stage,
                    j.row.path,
                    match &j.row.hears {
                        Some(Some(id)) => id.clone(),
                        _ => "null".to_string(),
                    },
                    if j.row.reached {
                        format!(
                            "where the forced route reaches it, after {} repaint(s)",
                            j.row.repaints.len()
                        )
                    } else {
                        "the first tick: the forced route never reaches it".to_string()
                    },
                ),
            });
        }
        ledger.cutscenes.push(j.row.clone());
    }
    // The stop-sounds: where the route stands when each fires, and what the
    // cut is there.
    let firing_cell = |step: usize| -> Option<[i32; 3]> {
        routes
            .iter()
            .rfind(|l| l.to_step == step)
            .and_then(|l| l.cells.last().copied())
    };
    for ev in &evs {
        match &ev.effect.verb {
            Verb::StopSound { category, sound } => {
                let biome = ev.key.and_then(|(step, _)| {
                    let cell = firing_cell(step)?;
                    let (map, _) = configuration(plan, &first, &evs, ev.key);
                    Some(map.at([cell[0], cell[1] + 1, cell[2]]).0.to_string())
                });
                let cut = biome.as_ref().map(|b| {
                    if !reaches_music(*category, sound.as_deref()) {
                        return Cut::Stinger;
                    }
                    let restarts =
                        ledger.biomes.iter().find(|r| r.id == *b).is_some_and(|r| {
                            match (&r.statement, &r.vanilla) {
                                (Some(s), _) => restarts_at_once(&s.music),
                                (None, Some((_, hears))) => restarts_at_once(hears),
                                (None, None) => false,
                            }
                        });
                    if restarts { Cut::Hiccup } else { Cut::Cut }
                });
                ledger.stops.push(StopRow {
                    stage: ev.stage,
                    path: ev.path.clone(),
                    category: *category,
                    sound: sound.clone(),
                    audience: ev
                        .effect
                        .audience
                        .as_ref()
                        .and_then(|a| serde_json::to_value(a).ok()),
                    after: ev.after.clone(),
                    biome,
                    cut,
                });
            }
            Verb::PlaySound { category, .. } => ledger.categorised.push(CategoryRow {
                kind: "play-sound",
                stage: ev.stage,
                path: ev.path.clone(),
                category: category.unwrap_or_default(),
            }),
            _ => {}
        }
    }
    for (i, p) in c.quests.content.pulses.iter().enumerate() {
        ledger.categorised.push(CategoryRow {
            kind: "pulse",
            stage: "quests",
            path: format!("/content/pulses/{i}"),
            category: p.category.unwrap_or_default(),
        });
    }
    (
        ledger,
        match refusal {
            Some(f) => Err(f),
            None => Ok(()),
        },
    )
}

/// The cutscenes that declare `hears`, as the PackTest claim's declared list
/// (`DW0811`): each one's index among every cutscene [`judge_cutscenes`]
/// judges, read from the authored documents rather than from the judgement, so
/// an emitter that skips one cannot shrink it.
pub fn hearing_ids(plan: &Plan<'_>) -> BTreeSet<String> {
    events(plan)
        .iter()
        .filter(|e| matches!(e.effect.verb, Verb::Cutscene { .. }))
        .filter(|e| e.effect.cutscene_shots().is_some())
        .enumerate()
        .filter(|(_, e)| matches!(e.effect.verb, Verb::Cutscene { hears: Some(_), .. }))
        .map(|(n, _)| n.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_segment_crosses_every_quart_it_passes() {
        // Along x, 0.5 → 12.5 crosses quarts 0, 1, 2, 3.
        let q = polyline_quarts(&[[0.5, 1.0, 1.0], [12.5, 1.0, 1.0]]);
        assert_eq!(q, vec![[0, 0, 0], [1, 0, 0], [2, 0, 0], [3, 0, 0]]);
        // A diagonal touches the cells between, never skipping a face.
        let d = polyline_quarts(&[[0.5, 0.5, 0.5], [7.5, 0.5, 7.5]]);
        assert_eq!(d.first(), Some(&[0, 0, 0]));
        assert_eq!(d.last(), Some(&[1, 0, 1]));
        for w in d.windows(2) {
            let steps: i32 = (0..3).map(|i| (w[1][i] - w[0][i]).abs()).sum();
            assert_eq!(steps, 1, "{d:?}");
        }
        // A static shot is its one cell; negative coordinates floor.
        assert_eq!(polyline_quarts(&[[-0.5, 3.0, -4.0]]), vec![[-1, 0, -1]]);
    }

    #[test]
    fn the_entering_word_is_the_tables() {
        let m = |v: Value| {
            let mut a = BTreeMap::new();
            a.insert("audio/background_music".to_string(), v);
            a
        };
        let track = |replace: bool| {
            json!({ "default": {
                "sound": "minecraft:music_disc.otherside",
                "min_delay": 0, "max_delay": 0, "replace_current_music": replace
            }})
        };
        assert_eq!(
            statement(&m(track(true))).unwrap().entering,
            Entering::Replaces
        );
        assert_eq!(
            statement(&m(track(false))).unwrap().entering,
            Entering::PlaysOut
        );
        let mut silent = m(json!({}));
        assert_eq!(
            statement(&silent).unwrap().entering,
            Entering::PlaysOutAtGain
        );
        silent.insert("audio/music_volume".to_string(), json!(0.0));
        assert_eq!(statement(&silent).unwrap().entering, Entering::Fades);
        silent.insert("audio/music_volume".to_string(), json!(0.5));
        assert_eq!(
            statement(&silent).unwrap().entering,
            Entering::PlaysOutAtGain
        );
        assert!(statement(&BTreeMap::new()).is_none());
    }

    #[test]
    fn a_cut_reaches_the_music_only_through_its_category() {
        assert!(reaches_music(Some(SoundCategory::Music), None));
        assert!(reaches_music(None, None));
        assert!(reaches_music(None, Some("music.game")));
        assert!(!reaches_music(
            None,
            Some("minecraft:entity.warden.heartbeat")
        ));
        assert!(!reaches_music(
            Some(SoundCategory::Ambient),
            Some("minecraft:ambient.cave")
        ));
    }
}
