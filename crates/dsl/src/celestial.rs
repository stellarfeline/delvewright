//! **The clock, read from the pinned game** (spec-0081): the moon's eight
//! phases, the sun's eased path, the sky-light and monster-burn keyframes, and
//! the twelve ticks a body's position names.
//!
//! The game's clock is one number, `dayTime`. `time set` writes it absolutely;
//! `time query daytime` reads it modulo 24000 and `time query day` divided by
//! it; the moon's phase is that quotient modulo 8. [`Clock`] is that number
//! split the way the two read-backs split it.
//!
//! ## One authority
//!
//! `data/timeline-day-1.21.11.json` and `data/timeline-moon-1.21.11.json` are
//! `data/minecraft/timeline/day.json` and `moon.json` of the pinned server jar,
//! byte for byte plus one trailing newline (`tools/maintenance/extract-timelines.py`
//! checks them against the jar; provenance in `crates/delvec/data/PROVENANCE.md`).
//! Every value below that the game owns is read from them and from nothing
//! typed here: the phase names and their order, the `sun_angle` keyframes and
//! their cubic-bezier ease, the `sky_light_level` ramp and the `monsters_burn`
//! window.
//!
//! ## The position table is derived, then frozen
//!
//! [`position_tick`] is twelve integer constants, because the emission path
//! carries no trigonometry (ADR-0006). `the_position_table_is_the_curve`
//! re-derives every one from the vendored track, the client's two sky-quad
//! constants and the disc fraction of the celestial textures, and fails when a
//! constant differs from its derivation by a tick.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::OnceLock;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};

/// Ticks in one day: `time query daytime` is `dayTime` modulo this.
pub const DAY_TICKS: i64 = 24_000;

const DAY_TIMELINE: &str = include_str!("../data/timeline-day-1.21.11.json");
const MOON_TIMELINE: &str = include_str!("../data/timeline-moon-1.21.11.json");

const SUN_ANGLE_TRACK: &str = "minecraft:visual/sun_angle";
const SKY_LIGHT_TRACK: &str = "minecraft:gameplay/sky_light_level";
const MONSTERS_BURN_TRACK: &str = "minecraft:gameplay/monsters_burn";
const MOON_PHASE_TRACK: &str = "minecraft:visual/moon_phase";

#[derive(Debug, Deserialize)]
struct Timeline {
    period_ticks: i64,
    tracks: BTreeMap<String, Track>,
}

#[derive(Debug, Deserialize)]
struct Track {
    #[serde(default)]
    ease: Option<serde_json::Value>,
    keyframes: Vec<Keyframe>,
}

#[derive(Debug, Deserialize)]
struct Keyframe {
    ticks: i64,
    value: serde_json::Value,
}

fn day_timeline() -> &'static Timeline {
    static T: OnceLock<Timeline> = OnceLock::new();
    T.get_or_init(|| serde_json::from_str(DAY_TIMELINE).expect("the vendored day timeline parses"))
}

fn moon_timeline() -> &'static Timeline {
    static T: OnceLock<Timeline> = OnceLock::new();
    T.get_or_init(|| {
        serde_json::from_str(MOON_TIMELINE).expect("the vendored moon timeline parses")
    })
}

fn track(t: &'static Timeline, name: &str) -> &'static Track {
    t.tracks
        .get(name)
        .unwrap_or_else(|| panic!("the vendored timeline has no track `{name}`"))
}

/// A track's keyframes as `(ticks, value)`, in file order — what a test reads
/// the game's numbers back through.
pub fn day_keyframes(name: &str) -> Vec<(i64, serde_json::Value)> {
    track(day_timeline(), name)
        .keyframes
        .iter()
        .map(|k| (k.ticks, k.value.clone()))
        .collect()
}

/// The ease a track declares, as the file spells it (`None` = linear).
pub fn day_ease(name: &str) -> Option<serde_json::Value> {
    track(day_timeline(), name).ease.clone()
}

/// CSS-convention cubic bezier through (0,0), (x1,y1), (x2,y2), (1,1): solve
/// the x-curve for the parameter, sample the y-curve. Newton–Raphson from the
/// progress itself, with bisection as the floor — arithmetic only, no
/// trigonometry, so the value is a function of the IEEE operations alone.
fn cubic_bezier(p: [f64; 4], x: f64) -> f64 {
    let [x1, y1, x2, y2] = p;
    let bez = |a: f64, b: f64, s: f64| {
        let u = 1.0 - s;
        3.0 * u * u * s * a + 3.0 * u * s * s * b + s * s * s
    };
    let dbez = |a: f64, b: f64, s: f64| {
        let u = 1.0 - s;
        3.0 * u * u * a + 6.0 * u * s * (b - a) + 3.0 * s * s * (1.0 - b)
    };
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let mut s = x;
    let mut solved = false;
    for _ in 0..16 {
        let err = bez(x1, x2, s) - x;
        if err.abs() < 1e-13 {
            solved = true;
            break;
        }
        let d = dbez(x1, x2, s);
        if d.abs() < 1e-9 {
            break;
        }
        s -= err / d;
        if !(0.0..=1.0).contains(&s) {
            break;
        }
    }
    if !solved {
        let (mut lo, mut hi) = (0.0f64, 1.0f64);
        for _ in 0..200 {
            s = 0.5 * (lo + hi);
            if bez(x1, x2, s) < x {
                lo = s;
            } else {
                hi = s;
            }
        }
    }
    bez(y1, y2, s)
}

/// Sample a numeric track at a (fractional) tick: the segment from the last
/// keyframe at or before the tick to the next one, wrapping over the period,
/// eased by the track's ease (absent = linear, `constant` = step).
fn sample(t: &'static Timeline, name: &str, tick: f64) -> f64 {
    let tr = track(t, name);
    let period = t.period_ticks as f64;
    let mut at = tick.rem_euclid(period);
    let kf = &tr.keyframes;
    let n = kf.len();
    let num = |k: &Keyframe| k.value.as_f64().expect("a numeric keyframe");
    let i = match kf.iter().rposition(|k| (k.ticks as f64) <= at) {
        Some(i) => i,
        None => {
            at += period;
            n - 1
        }
    };
    let a_ticks = kf[i].ticks as f64;
    let (b, b_ticks) = if i + 1 < n {
        (&kf[i + 1], kf[i + 1].ticks as f64)
    } else {
        (&kf[0], kf[0].ticks as f64 + period)
    };
    let (va, vb) = (num(&kf[i]), num(b));
    if b_ticks <= a_ticks {
        return va;
    }
    let progress = (at - a_ticks) / (b_ticks - a_ticks);
    let eased = match &tr.ease {
        None => progress,
        Some(serde_json::Value::String(s)) if s == "constant" => 0.0,
        Some(serde_json::Value::Object(o)) if o.contains_key("cubic_bezier") => {
            let c: Vec<f64> = o["cubic_bezier"]
                .as_array()
                .expect("cubic_bezier is an array")
                .iter()
                .map(|v| v.as_f64().expect("a bezier control value"))
                .collect();
            cubic_bezier([c[0], c[1], c[2], c[3]], progress)
        }
        Some(other) => panic!("track `{name}` declares an ease this reader does not know: {other}"),
    };
    va + (vb - va) * eased
}

/// The `visual/sun_angle` of the pinned day timeline at a `daytime` tick, in
/// degrees: 0 at noon, growing westward, 180 at midnight. The moon's angle is
/// this plus 180 (`moon_angle` is the same track at 540 → 180).
pub fn sun_angle_degrees(daytime: f64) -> f64 {
    sample(day_timeline(), SUN_ANGLE_TRACK, daytime)
}

/// The sun's altitude above the horizon, in degrees, at a `daytime` tick: the
/// body travels a great circle through the zenith, so an angle `α` from the
/// zenith is an altitude of `90 − α` on the western half of the arc and
/// `α − 270` on the eastern half. Piecewise linear in the angle — no
/// trigonometry.
pub fn sun_altitude_degrees(daytime: f64) -> f64 {
    altitude_of_angle(sun_angle_degrees(daytime))
}

/// Altitude in degrees of a body standing `angle` degrees from the zenith.
pub fn altitude_of_angle(angle: f64) -> f64 {
    let a = angle.rem_euclid(360.0);
    if a <= 180.0 { 90.0 - a } else { a - 270.0 }
}

/// Whether the sun at this `daytime` stands on the western half of its arc —
/// past noon and before midnight. At noon and midnight exactly the side means
/// nothing and this answers `false` (east), so a value is a function of the
/// tick alone.
pub fn sun_is_west(daytime: f64) -> bool {
    let a = sun_angle_degrees(daytime).rem_euclid(360.0);
    a > 0.0 && a < 180.0
}

/// The `gameplay/sky_light_level` multiplier of the pinned day timeline at a
/// `daytime` tick: 1.0 on the day plateau, 4/15 on the night plateau, linear
/// ramps between.
pub fn sky_light_factor(daytime: i64) -> f64 {
    sample(day_timeline(), SKY_LIGHT_TRACK, daytime as f64)
}

/// Whether the `gameplay/monsters_burn` track of the pinned day timeline is on
/// at a `daytime` tick (a step track: the value of the last keyframe at or
/// before the tick, wrapping).
pub fn monsters_burn(daytime: i64) -> bool {
    let tr = track(day_timeline(), MONSTERS_BURN_TRACK);
    let at = daytime.rem_euclid(day_timeline().period_ticks);
    let k = tr
        .keyframes
        .iter()
        .rev()
        .find(|k| k.ticks <= at)
        .or_else(|| tr.keyframes.last())
        .expect("monsters_burn has keyframes");
    k.value.as_bool().expect("monsters_burn is boolean")
}

/// The two ticks the `monsters_burn` track switches at: `(off, on)` — 12542 and
/// 23460 in the pinned file.
pub fn monsters_burn_switches() -> (i64, i64) {
    let kf = &track(day_timeline(), MONSTERS_BURN_TRACK).keyframes;
    let at = |v: bool| {
        kf.iter()
            .find(|k| k.value.as_bool() == Some(v))
            .map(|k| k.ticks)
            .expect("monsters_burn switches both ways")
    };
    (at(false), at(true))
}

/// The moon's eight phases, kebab-cased, in the pinned `moon.json`'s order:
/// index `i` is the phase of day `i` modulo 8.
pub fn phase_names() -> &'static [String] {
    static N: OnceLock<Vec<String>> = OnceLock::new();
    N.get_or_init(|| {
        track(moon_timeline(), MOON_PHASE_TRACK)
            .keyframes
            .iter()
            .map(|k| {
                k.value
                    .as_str()
                    .expect("a moon phase keyframe names a phase")
                    .replace('_', "-")
            })
            .collect()
    })
}

/// The moon cycle in days: the number of phase keyframes.
pub fn moon_cycle_days() -> i64 {
    phase_names().len() as i64
}

/// The `time_check` period over which a predicate reads the phase: the moon
/// timeline's own period.
pub fn moon_period_ticks() -> i64 {
    moon_timeline().period_ticks
}

/// **A body in the sky**: the sun or the moon. The moon stands exactly opposite
/// the sun (`moon_angle` is `sun_angle` + 180), so the two are one position seen
/// from two sides.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Body {
    Sun,
    Moon,
}

impl Body {
    /// The word a document writes for this body.
    pub fn keyword(self) -> &'static str {
        match self {
            Body::Sun => "sun",
            Body::Moon => "moon",
        }
    }
}

/// **Where a body stands on its arc** — one point of it, defined by the body's
/// drawn disc and the horizon (spec-0081 §3.2).
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Position {
    /// The disc's centre on the eastern horizon.
    Rising,
    /// The disc stands wholly clear of the eastern horizon.
    JustRisen,
    /// The body at the zenith.
    High,
    /// The disc's centre on the western horizon.
    Setting,
    /// The disc has wholly gone below the western horizon.
    JustSet,
    /// The body at the nadir — the other body is `high`.
    Below,
}

/// Every position, in declaration order.
pub const POSITIONS: [Position; 6] = [
    Position::Rising,
    Position::JustRisen,
    Position::High,
    Position::Setting,
    Position::JustSet,
    Position::Below,
];

impl Position {
    /// The word a document writes for this position.
    pub fn keyword(self) -> &'static str {
        match self {
            Position::Rising => "rising",
            Position::JustRisen => "just-risen",
            Position::High => "high",
            Position::Setting => "setting",
            Position::JustSet => "just-set",
            Position::Below => "below",
        }
    }
}

/// Half the angle the sun's DRAWN disc spans, in hundredths of a degree:
/// `atan(30 × 8/32 / 100)` = 4.29° — the client's `SUN_SIZE` 30 at height 100,
/// of which the texture's full-brightness disc is 8 of 32 pixels.
pub const SUN_DISC_HALF_CENTIDEG: i32 = 429;

/// Half the moon's drawn disc: `atan(20 × 8/32 / 100)` = 2.86° (`MOON_SIZE` 20).
pub const MOON_DISC_HALF_CENTIDEG: i32 = 286;

/// The `daytime` tick at which `body` stands at `pos` — the integer nearest the
/// pinned track's solution. Derived once and frozen; the test
/// `the_position_table_is_the_curve` re-derives every entry.
pub const fn position_tick(body: Body, pos: Position) -> i64 {
    match (body, pos) {
        (Body::Sun, Position::Rising) => 23218,
        (Body::Sun, Position::JustRisen) => 23486,
        (Body::Sun, Position::High) => 6000,
        (Body::Sun, Position::Setting) => 12782,
        (Body::Sun, Position::JustSet) => 13047,
        (Body::Sun, Position::Below) => 18000,
        (Body::Moon, Position::Rising) => 12782,
        (Body::Moon, Position::JustRisen) => 12959,
        (Body::Moon, Position::High) => 18000,
        (Body::Moon, Position::Setting) => 23218,
        (Body::Moon, Position::JustSet) => 23397,
        (Body::Moon, Position::Below) => 6000,
    }
}

/// The named body's altitude at `pos`, in hundredths of a degree, by the
/// position's definition.
pub const fn position_altitude_centideg(body: Body, pos: Position) -> i32 {
    let half = match body {
        Body::Sun => SUN_DISC_HALF_CENTIDEG,
        Body::Moon => MOON_DISC_HALF_CENTIDEG,
    };
    match pos {
        Position::Rising | Position::Setting => 0,
        Position::JustRisen => half,
        Position::High => 9000,
        Position::JustSet => -half,
        Position::Below => -9000,
    }
}

/// Whether the moon is at or above the horizon at a `daytime` tick: from the
/// sun's setting to its rising, both inclusive (the moon's centre is on the
/// horizon at those two ticks).
pub fn moon_up(daytime: i64) -> bool {
    let d = daytime.rem_euclid(DAY_TICKS);
    (position_tick(Body::Sun, Position::Setting)..=position_tick(Body::Sun, Position::Rising))
        .contains(&d)
}

/// **One of the moon's eight phases**, held as its index (0 = `full-moon`, the
/// phase of day 0). Written in a document as the pinned game's own name,
/// kebab-cased; the names are DATA, read from the vendored `moon.json`, never an
/// enum this engine authors (spec-0039: vanilla registry values are data).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MoonPhase(u8);

impl MoonPhase {
    /// The phase of day `day` (modulo the cycle).
    pub fn of_day(day: i64) -> MoonPhase {
        MoonPhase(day.rem_euclid(moon_cycle_days()) as u8)
    }

    /// The index into the cycle: the day, modulo 8, this phase shows on.
    pub fn index(self) -> i64 {
        i64::from(self.0)
    }

    /// The pinned game's name, kebab-cased (`new-moon`).
    pub fn name(self) -> &'static str {
        phase_names()[self.0 as usize].as_str()
    }

    /// The phase a document names, or `None` for a word the pinned game does
    /// not use.
    pub fn parse(s: &str) -> Option<MoonPhase> {
        phase_names()
            .iter()
            .position(|n| n == s)
            .map(|i| MoonPhase(i as u8))
    }

    /// The eight names, `a, b, …`, for a message.
    pub fn names_line() -> String {
        phase_names().join(", ")
    }
}

impl Serialize for MoonPhase {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.name())
    }
}

impl<'de> Deserialize<'de> for MoonPhase {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<MoonPhase, D::Error> {
        let s = String::deserialize(de)?;
        MoonPhase::parse(&s).ok_or_else(|| {
            serde::de::Error::custom(format!(
                "unknown moon phase `{s}`: the pinned game names eight, {}",
                MoonPhase::names_line()
            ))
        })
    }
}

impl JsonSchema for MoonPhase {
    fn schema_name() -> Cow<'static, str> {
        "MoonPhase".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "description": format!(
                "One of the moon's eight phases, named as the pinned game's moon timeline \
                 names them, kebab-cased: {}. Data, not an engine vocabulary.",
                MoonPhase::names_line()
            ),
        })
    }
}

/// **A sky stated in a designer's words**: one body, where it stands, and —
/// where the moon shows — its phase (spec-0081 §3.1). Exactly one of `sun` and
/// `moon` (`DW0931`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CelestialTime {
    /// Where the sun stands. Exclusive with `moon`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sun: Option<Position>,
    /// Where the moon stands. Exclusive with `sun`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moon: Option<Position>,
    /// The moon's phase. Required on `world.time` whenever the moon is at or
    /// above the horizon at the stated position, refused wherever it is below.
    /// Absent on a `set-time`, a design row or a camera sky = the world's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<MoonPhase>,
}

impl CelestialTime {
    /// The one body this statement names and where it stands, or `None` when it
    /// names neither or both (`DW0931`).
    pub fn stated(self) -> Option<(Body, Position)> {
        match (self.sun, self.moon) {
            (Some(p), None) => Some((Body::Sun, p)),
            (None, Some(p)) => Some((Body::Moon, p)),
            _ => None,
        }
    }

    /// The `daytime` tick this statement names. A statement naming neither or
    /// both bodies is refused by `DW0931` before anything reads its hour; it
    /// reads as its `sun` if it has one, else its `moon`, else noon, so the
    /// function is total.
    pub fn daytime(self) -> i64 {
        match (self.sun, self.moon) {
            (Some(p), _) => position_tick(Body::Sun, p),
            (None, Some(p)) => position_tick(Body::Moon, p),
            (None, None) => position_tick(Body::Sun, Position::High),
        }
    }

    /// The moon's altitude at the stated position, in hundredths of a degree.
    pub fn moon_altitude_centideg(self) -> i32 {
        match self.stated() {
            Some((Body::Moon, p)) => position_altitude_centideg(Body::Moon, p),
            Some((Body::Sun, p)) => -position_altitude_centideg(Body::Sun, p),
            None => -position_altitude_centideg(Body::Sun, Position::High),
        }
    }

    /// Whether the moon is at or above the horizon at the stated position.
    pub fn moon_shows(self) -> bool {
        self.moon_altitude_centideg() >= 0
    }

    /// The canonical JSON spelling — `{"moon":"high","phase":"new-moon"}`.
    pub fn spelling(self) -> String {
        serde_json::to_string(&self).expect("a celestial time serialises")
    }
}

/// **The clock one time value sets**: `dayTime` split the way the game's two
/// read-backs split it. Two time values are equal when their clocks are.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Clock {
    /// `time query day`: `dayTime / 24000`.
    pub day: i64,
    /// `time query daytime`: `dayTime % 24000`.
    pub daytime: i64,
}

impl Clock {
    /// The absolute `dayTime`: `day × 24000 + daytime` — the integer `time set`
    /// takes.
    pub fn absolute(self) -> i64 {
        self.day * DAY_TICKS + self.daytime
    }

    /// The phase the moon shows on this clock's day.
    pub fn phase(self) -> MoonPhase {
        MoonPhase::of_day(self.day)
    }

    /// `(day D, T)` — how a message prints a clock beside its spelling.
    pub fn label(self) -> String {
        format!("(day {}, {})", self.day, self.daytime)
    }
}

/// Where a celestial time is written, for the rules of its shape (`DW0931`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CelestialSite {
    /// `world.time`.
    World,
    /// A `set-time` effect, quest or dialogue.
    Cut,
    /// A design record row's `time`.
    Row,
    /// A showcase camera's `sky.time`.
    Camera,
}

/// An altitude in hundredths of a degree, as a message prints it: `+2.86°`.
pub fn degrees_label(centideg: i32) -> String {
    let sign = if centideg < 0 { "-" } else { "+" };
    let a = centideg.unsigned_abs();
    format!("{sign}{}.{:02}°", a / 100, a % 100)
}

/// **The rules of one celestial value's shape** (spec-0081 §6), as the message
/// each broken rule earns — empty when the value is well formed. One function,
/// so `delvec validate` (the world, every cut, every design row) and the camera
/// record's reader say the same sentence about the same value.
pub fn shape_findings(
    t: CelestialTime,
    site: CelestialSite,
    world: crate::WorldTime,
) -> Vec<String> {
    let spelling = t.spelling();
    let mut out = Vec::new();
    let Some(_) = t.stated() else {
        let which = if t.sun.is_some() { "both" } else { "neither" };
        out.push(if which == "both" {
            format!(
                "`{spelling}` names both `sun` and `moon`. A celestial time is ONE body and where \
                 it stands: the moon stands exactly opposite the sun, so `{{\"moon\": \"rising\"}}` \
                 and `{{\"sun\": \"setting\"}}` are one hour spelled from two sides, and two \
                 bodies are two hours or one written twice. NAME ONE BODY: keep the one the \
                 sentence is about and delete the other"
            )
        } else {
            format!(
                "`{spelling}` names neither `sun` nor `moon`, so it states no hour. A celestial \
                 time is one body and where it stands. NAME ONE BODY: add `sun` or `moon` with one \
                 of the six positions — rising, just-risen, high, setting, just-set, below"
            )
        });
        return out;
    };
    let alt = t.moon_altitude_centideg();
    if let Some(p) = t.phase
        && !t.moon_shows()
    {
        out.push(format!(
            "`{spelling}` states the phase `{}` where the moon stands at {} — below the horizon, \
             where nobody can see it. A judgement about a body nobody can see reaches nothing. \
             REMOVE `phase` from this time",
            p.name(),
            degrees_label(alt),
        ));
    }
    if site == CelestialSite::World && t.phase.is_none() && t.moon_shows() {
        out.push(format!(
            "`world.time` `{spelling}` puts the moon at {} — at or above the horizon, where the \
             party sees it — and states no `phase`. The phase the party sees is a judgement \
             nobody may leave to a default. STATE `phase`, one of the eight the pinned game \
             names: {}",
            degrees_label(alt),
            MoonPhase::names_line(),
        ));
    }
    // The world's phase is only a fact when the world's own time is well formed:
    // a world refused for naming no body, or for leaving a visible moon
    // unnamed, has no phase for a cut to restate, and its own refusal says so.
    let world_phase_known = match world.celestial() {
        None => true,
        Some(w) => w.stated().is_some() && (w.phase.is_some() || !w.moon_shows()),
    };
    if site != CelestialSite::World
        && world_phase_known
        && let Some(p) = t.phase
        && t.moon_shows()
        && p == MoonPhase::of_day(world.world_day())
    {
        out.push(format!(
            "`{spelling}` states the phase `{}`, which is the world's: `world.time` `{}` shows \
             `{}` on day {}. A phase left out here is the world's, so this one is a copy that \
             goes stale the moment the world's changes. REMOVE `phase` from this time",
            p.name(),
            world.keyword(),
            p.name(),
            world.world_day(),
        ));
    }
    out
}

/// `DW0931` over every time a campaign's documents state: the world's, every
/// `set-time` at every effect root and depth, every dialogue `set-time`, and
/// every design record row. (A camera's sky is the camera record's reader's:
/// `delvec`'s design gate, through [`shape_findings`].)
pub(crate) fn check(c: &crate::envelope::Campaign, d: &mut Vec<crate::diagnostic::Diagnostic>) {
    use crate::diagnostic::{Diagnostic, codes};
    let world = c.world.content.time;
    let mut push = |stage: &str, path: String, t: crate::WorldTime, site: CelestialSite| {
        if let Some(ct) = t.celestial() {
            for m in shape_findings(ct, site, world) {
                d.push(Diagnostic::error(
                    codes::CELESTIAL_TIME,
                    stage,
                    path.clone(),
                    m,
                ));
            }
        }
    };
    push("world", "/content/time".into(), world, CelestialSite::World);
    crate::stages::for_each_campaign_effect(c, &mut |path, _site, e| {
        if let Some(t) = e.set_time() {
            push("quests", format!("{path}/time"), t, CelestialSite::Cut);
        }
    });
    for (i, tree) in c.dialogue.content.dialogues.iter().enumerate() {
        for (j, node) in tree.nodes.iter().enumerate() {
            for (k, opt) in node.options.iter().enumerate() {
                for (l, e) in opt.effects.iter().enumerate() {
                    if let Some(t) = e.set_time() {
                        push(
                            "dialogue",
                            format!(
                                "/content/dialogues/{i}/nodes/{j}/options/{k}/effects/{l}/time"
                            ),
                            t,
                            CelestialSite::Cut,
                        );
                    }
                }
            }
        }
    }
    if let Some(design) = &c.design {
        for (i, r) in design.content.references.iter().enumerate() {
            push(
                "design",
                format!("/content/references/{i}/time"),
                r.time,
                CelestialSite::Row,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The client's sky constants, recorded with the reading that measured
    /// them: `SkyRenderer` draws the sun on a quad of `SUN_SIZE` 30 and the moon
    /// on one of `MOON_SIZE` 20, both at height 100; each celestial texture is
    /// 32 × 32 with a full-brightness disc 8 pixels wide (the sun's middle row
    /// reads `1, 2, 4, 7, 10, 14, 19, 23, 27, 32, 36, 40, 255 ×8, 40 … 1`). A
    /// client-jar pin bump re-measures them.
    const SUN_SIZE: f64 = 30.0;
    const MOON_SIZE: f64 = 20.0;
    const QUAD_HEIGHT: f64 = 100.0;
    const DISC_FRACTION: f64 = 8.0 / 32.0;

    fn half_disc_deg(size: f64) -> f64 {
        (size * DISC_FRACTION / QUAD_HEIGHT).atan().to_degrees()
    }

    /// The real tick at which the sun's altitude is `alt` degrees on the given
    /// half of the arc, by bisection on the vendored track.
    fn solve_sun(alt: f64, west: bool) -> f64 {
        // The western half runs noon (6000) -> midnight (18000), the altitude
        // falling; the eastern half midnight (18000) -> noon (30000), rising.
        let (mut lo, mut hi) = if west {
            (6000.0, 18000.0)
        } else {
            (18000.0, 30000.0)
        };
        for _ in 0..200 {
            let mid = 0.5 * (lo + hi);
            let a = sun_altitude_degrees(mid);
            let above = a > alt;
            // west: altitude decreases with time; east: it increases.
            if above == west {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        (0.5 * (lo + hi)).rem_euclid(DAY_TICKS as f64)
    }

    #[test]
    fn the_position_table_is_the_curve() {
        let sun_half = half_disc_deg(SUN_SIZE);
        let moon_half = half_disc_deg(MOON_SIZE);
        assert_eq!(
            (sun_half * 100.0).round() as i32,
            SUN_DISC_HALF_CENTIDEG,
            "{sun_half}"
        );
        assert_eq!(
            (moon_half * 100.0).round() as i32,
            MOON_DISC_HALF_CENTIDEG,
            "{moon_half}"
        );
        // The moon stands opposite the sun: the moon at altitude `a` on one
        // side is the sun at `-a` on the other.
        let derive = |body: Body, pos: Position| -> i64 {
            let half = match body {
                Body::Sun => sun_half,
                Body::Moon => moon_half,
            };
            // (altitude of the named body, the named body is in the west)
            let (alt, west) = match pos {
                Position::Rising => (0.0, false),
                Position::JustRisen => (half, false),
                Position::Setting => (0.0, true),
                Position::JustSet => (-half, true),
                Position::High | Position::Below => (f64::NAN, false),
            };
            let (zenith, nadir) = (6000, 18000);
            match (body, pos) {
                (Body::Sun, Position::High) | (Body::Moon, Position::Below) => zenith,
                (Body::Sun, Position::Below) | (Body::Moon, Position::High) => nadir,
                (Body::Sun, _) => solve_sun(alt, west).round() as i64,
                (Body::Moon, _) => solve_sun(-alt, !west).round() as i64,
            }
        };
        let mut checked = 0;
        for body in [Body::Sun, Body::Moon] {
            for pos in POSITIONS {
                let want = derive(body, pos);
                assert_eq!(
                    position_tick(body, pos),
                    want,
                    "{body:?} {pos:?}: the frozen tick differs from the derivation"
                );
                checked += 1;
            }
        }
        assert_eq!(checked, 12, "every entry of the table was derived");
        // The zenith and nadir are where the track says they are.
        assert!((sun_altitude_degrees(6000.0) - 90.0).abs() < 1e-9);
        assert!((sun_altitude_degrees(18000.0) + 90.0).abs() < 1e-9);
        // One hour, spelled from two sides, by construction.
        assert_eq!(
            position_tick(Body::Moon, Position::Rising),
            position_tick(Body::Sun, Position::Setting)
        );
    }

    #[test]
    fn the_eight_phases_are_the_pinned_names_in_order() {
        assert_eq!(
            phase_names(),
            [
                "full-moon",
                "waning-gibbous",
                "third-quarter",
                "waning-crescent",
                "new-moon",
                "waxing-crescent",
                "first-quarter",
                "waxing-gibbous",
            ]
        );
        let kf: Vec<i64> = track(moon_timeline(), MOON_PHASE_TRACK)
            .keyframes
            .iter()
            .map(|k| k.ticks)
            .collect();
        assert_eq!(kf, (0..8).map(|i| i * DAY_TICKS).collect::<Vec<_>>());
        assert_eq!(moon_period_ticks(), 192_000);
        assert_eq!(MoonPhase::of_day(4).name(), "new-moon");
        assert_eq!(MoonPhase::of_day(8).name(), "full-moon");
    }

    #[test]
    fn the_burn_and_sky_light_keyframes_are_the_pinned_ones() {
        let burn: Vec<(i64, bool)> = day_keyframes(MONSTERS_BURN_TRACK)
            .into_iter()
            .map(|(t, v)| (t, v.as_bool().unwrap()))
            .collect();
        assert_eq!(burn, [(12542, false), (23460, true)]);
        let sky: Vec<(i64, f64)> = day_keyframes(SKY_LIGHT_TRACK)
            .into_iter()
            .map(|(t, v)| (t, v.as_f64().unwrap()))
            .collect();
        assert_eq!(
            sky,
            [
                (133, 1.0),
                (11867, 1.0),
                (13670, 0.26666668),
                (22330, 0.26666668)
            ]
        );
        assert_eq!(day_ease(SKY_LIGHT_TRACK), None, "linear");
        // Read at the keywords (§2.5).
        let level = |t: i64| (15.0 * sky_light_factor(t)).round() as i64;
        assert_eq!(level(6000), 15);
        assert_eq!(level(12000), 14);
        assert_eq!(level(13000), 8);
        assert_eq!(level(23000), 8);
        assert_eq!(level(18000), 4);
        assert!(monsters_burn(12541) && !monsters_burn(12542));
        assert!(!monsters_burn(23459) && monsters_burn(23460));
        assert!(monsters_burn(0));
    }

    #[test]
    fn the_sun_track_is_the_eased_one() {
        assert_eq!(
            day_ease(SUN_ANGLE_TRACK),
            Some(serde_json::json!({"cubic_bezier": [0.362, 0.241, 0.638, 0.759]}))
        );
        assert!((sun_angle_degrees(6000.0) - 0.0).abs() < 1e-9);
        assert!((sun_angle_degrees(18000.0) - 180.0).abs() < 1e-9);
        // The keywords' altitudes on the pinned track (§2.3).
        let alt = |t: f64| sun_altitude_degrees(t);
        assert!((alt(1000.0) - 27.55).abs() < 0.01, "{}", alt(1000.0));
        assert!((alt(12000.0) - 12.37).abs() < 0.01, "{}", alt(12000.0));
        assert!((alt(13000.0) + 3.52).abs() < 0.01, "{}", alt(13000.0));
        assert!((alt(23000.0) + 3.52).abs() < 0.01, "{}", alt(23000.0));
        assert!(sun_is_west(12000.0) && !sun_is_west(23000.0));
        assert!(!sun_is_west(6000.0) && !sun_is_west(18000.0));
    }

    #[test]
    fn a_phase_outside_the_eight_is_a_parse_error() {
        assert!(serde_json::from_str::<MoonPhase>("\"new-moon\"").is_ok());
        let e = serde_json::from_str::<MoonPhase>("\"blood-moon\"").unwrap_err();
        assert!(e.to_string().contains("the pinned game names eight"), "{e}");
    }
}
