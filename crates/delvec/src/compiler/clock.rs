//! **The clock binding line** (spec-0081 §5.5): one line per time value the
//! campaign states — the world's and each `set-time` cut's — and a summary,
//! printed on every run through the validation funnel.
//!
//! Each line spells what the value resolves to: the day and the `daytime`
//! (with the absolute `dayTime` where it is not day 0's), where the sun and the
//! moon stand, the phase the moon shows, the sky light this engine judges beside
//! the game's own value at that tick, and whether the pinned game burns undead
//! there. A campaign of keywords prints `0 celestial` and `days {0}` — the
//! measured zero, with the full moon it implies spelled on the world's line.

use std::collections::BTreeSet;

use delvewright_dsl::celestial::{self, sun_altitude_degrees, sun_is_west};
use delvewright_dsl::{Campaign, Clock, TimeSite, WorldTime};

use crate::compiler::daylight::hour_burns;
use crate::compiler::light::{sky_base_judged, sky_light_game};

/// `+2.86° E` — an altitude with its side; the zenith and nadir carry none.
fn body_line(alt: f64, west: bool) -> String {
    let side = if alt.abs() >= 90.0 - 1e-9 {
        String::new()
    } else if west {
        " W".to_string()
    } else {
        " E".to_string()
    };
    format!("{alt:+.2}°{side}")
}

/// One value's line: `<what> <spelling> -> day D daytime T[ (dayTime N)]; sun …,
/// moon …; sky light judged J, game G; burns: yes|no`.
fn line(what: &str, t: WorldTime, clock: Clock) -> String {
    let d = clock.daytime as f64;
    let sun_alt = sun_altitude_degrees(d);
    let west = sun_is_west(d);
    let phase = clock.phase().name();
    let moon = if celestial::moon_up(clock.daytime) {
        format!("moon {} {phase}", body_line(-sun_alt, !west))
    } else {
        format!("moon below the horizon ({phase})")
    };
    let absolute = if clock.day == 0 {
        String::new()
    } else {
        format!(" (dayTime {})", clock.absolute())
    };
    format!(
        "clock: {what} {} -> day {} daytime {}{absolute}; sun {}, {moon}; sky light judged {}, \
         game {}; burns: {}",
        t.keyword(),
        clock.day,
        clock.daytime,
        body_line(sun_alt, west),
        sky_base_judged(clock.daytime),
        sky_light_game(clock.daytime),
        if hour_burns(t) { "yes" } else { "no" },
    )
}

/// Every clock line and the summary, in a fixed order: the world, then every
/// quest `set-time` in the effect walk's order, then every dialogue `set-time`.
pub fn binding_lines(c: &Campaign) -> Vec<String> {
    let world = c.world.content.time;
    let mut cuts: Vec<WorldTime> = Vec::new();
    delvewright_dsl::for_each_campaign_effect(c, &mut |_path, _site, e| {
        if let Some(t) = e.set_time() {
            cuts.push(t);
        }
    });
    for tree in &c.dialogue.content.dialogues {
        for node in &tree.nodes {
            for opt in &node.options {
                for e in &opt.effects {
                    if let Some(t) = e.set_time() {
                        cuts.push(t);
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    let world_clock = world.clock(TimeSite::Sky, world);
    out.push(line("world", world, world_clock));
    let mut days: BTreeSet<i64> = BTreeSet::from([world_clock.day]);
    for &t in &cuts {
        let k = t.clock(TimeSite::Cut, world);
        days.insert(k.day);
        out.push(line("set-time", t, k));
    }
    let all: Vec<WorldTime> = std::iter::once(world).chain(cuts.iter().copied()).collect();
    let celestial = all.iter().filter(|t| !t.is_keyword()).count();
    let phases: BTreeSet<_> = all.iter().filter_map(|t| t.stated_phase()).collect();
    out.push(format!(
        "clocks: 1 world + {} cut(s); {celestial} celestial, {} keyword; phases stated {{{}}}; \
         days {{{}}}",
        cuts.len(),
        all.len() - celestial,
        phases
            .iter()
            .map(|p| p.name())
            .collect::<Vec<_>>()
            .join(", "),
        days.iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join(", "),
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_keyword_world_spells_its_full_moon_and_day_zero() {
        let l = line("world", WorldTime::Noon, WorldTime::Noon.world_clock());
        assert_eq!(
            l,
            "clock: world noon -> day 0 daytime 6000; sun +90.00°, moon below the horizon \
             (full-moon); sky light judged 15, game 15; burns: yes"
        );
        let l = line("world", WorldTime::Night, WorldTime::Night.world_clock());
        assert!(l.contains("moon +3.52° E full-moon"), "{l}");
        assert!(l.contains("sky light judged 4, game 8"), "{l}");
    }

    #[test]
    fn a_new_moon_just_risen_is_day_four() {
        let t: WorldTime =
            serde_json::from_str(r#"{"moon":"just-risen","phase":"new-moon"}"#).unwrap();
        let l = line("world", t, t.world_clock());
        assert_eq!(
            l,
            "clock: world {\"moon\":\"just-risen\",\"phase\":\"new-moon\"} -> day 4 daytime 12959 \
             (dayTime 108959); sun -2.86° W, moon +2.86° E new-moon; sky light judged 4, game 8; \
             burns: no"
        );
    }
}
