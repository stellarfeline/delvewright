//! A perception beat's surface, and **every game fact it is built on, in one
//! file** (spec-0085).
//!
//! A perception beat is no verb of its own: it is a `sequence` of `give-effect`,
//! `particle` and `play-sound` effects addressed by the envelope's `audience`
//! and `in`. What this module holds is the game facts those pieces consume —
//! which particle types exist and which take options, which status effects hide
//! the floor, which effects change what the player sees and how long each takes
//! to wind down — each with the wiki page or the measurement it came from, so a
//! re-pin is one diff here and the validator, the blind-reach proof and the
//! skill page all read the same table.
//!
//! The rules built on the facts (`DW0941`, `DW0943`, `DW0944`) are **authored**
//! and live with the checks that state them.

use std::collections::BTreeMap;
use std::sync::OnceLock;

/// The wiki pages the facts below were read from (spec-0085 *Research*). Named so
/// a re-pin knows what to re-read.
pub const WIKI_PAGES: [&str; 6] = [
    "Blindness",
    "Darkness",
    "Nausea",
    "Night Vision",
    "Commands/particle",
    "Sprinting",
];

/// The pinned particle-type registry, derived from the server jar by
/// `tools/maintenance/extract-particle-registry.py` (provenance in
/// `crates/delvec/data/PROVENANCE.md`).
const PARTICLES_JSON: &str = include_str!("../data/particles-1.21.11.json");

/// `id → takes options`, over every particle type the pinned game registers.
pub fn particle_registry() -> &'static BTreeMap<String, bool> {
    static REG: OnceLock<BTreeMap<String, bool>> = OnceLock::new();
    REG.get_or_init(|| {
        let raw: BTreeMap<String, serde_json::Value> =
            serde_json::from_str(PARTICLES_JSON).expect("vendored particle registry is valid JSON");
        raw.into_iter()
            .map(|(id, v)| {
                let takes = v
                    .get("options")
                    .and_then(serde_json::Value::as_bool)
                    .expect("every particle registry entry states `options`");
                (id, takes)
            })
            .collect()
    })
}

/// The namespaced spelling of a particle id (`minecraft:` when none is written).
pub fn namespaced(id: &str) -> String {
    if id.contains(':') {
        id.to_string()
    } else {
        format!("minecraft:{id}")
    }
}

/// What the pinned registry says about a particle id: `None` for an id the game
/// does not register, `Some(true)` for a type that takes options, `Some(false)`
/// for one a bare id spawns.
pub fn particle_takes_options(id: &str) -> Option<bool> {
    particle_registry().get(&namespaced(id)).copied()
}

/// **The status effects whose screen hides the floor** (spec-0085 §4.1, §6.2) —
/// the effects a grant of which owes the blind-reach proof (`DW0943`), each with
/// the wiki page that says what it does to the screen and whether it forbids the
/// sprint.
///
/// `blindness`: "a thick black fog" leaving only the immediate area, and "the
/// effect also prevents the player from sprinting" [cited — *Blindness*].
/// `darkness`: the brightness cycles "between complete darkness, and relative
/// visibility … roughly 14 blocks" [cited — *Darkness*]; the sprint is not
/// forbidden. `nausea` warps the view and hides nothing, so it is not here.
pub const BLINDING: [(&str, bool, &str); 2] = [
    ("minecraft:blindness", true, "Blindness"),
    ("minecraft:darkness", false, "Darkness"),
];

/// Whether a status effect id hides the floor, and if so whether it forbids the
/// sprint: `None` for an effect that is not blinding.
pub fn blinding(effect: &str) -> Option<bool> {
    let id = namespaced(effect);
    BLINDING
        .iter()
        .find(|(e, _, _)| *e == id)
        .map(|(_, no_sprint, _)| *no_sprint)
}

/// **The sight effects** (spec-0085 §5.3) — the status effects that change what
/// the player sees, so that one ending under an authored camera begins ramping
/// down on screen — each with its **wind-down** in ticks: how long before its
/// end the screen starts to change back.
///
/// The set is the findings-ledger row's. The pages read for this spec
/// (*Night Vision*, *Blindness*, *Darkness*) state no wind-down, so:
///
/// * **night vision, 200 ticks** — the number this engine already stood on for
///   the area mitigation's lease: the client's `GameRenderer` ramps the
///   night-vision brightness down (the flicker) once the remaining duration
///   drops to 200 ticks. The mitigation lease reads it from here, so the two
///   uses are one number;
/// * **blindness and darkness, 20 ticks** — **authored from memory** (the fog
///   fades over about a second), named as such, and looked at on the demo
///   level. A longer wind-down refuses more grants, never fewer.
pub const SIGHT: [(&str, u32, &str); 3] = [
    ("minecraft:night_vision", 200, "Night Vision"),
    ("minecraft:blindness", 20, "Blindness"),
    ("minecraft:darkness", 20, "Darkness"),
];

/// The wind-down in ticks of a sight effect, or `None` for an effect that is
/// not one.
pub fn sight_wind_down_ticks(effect: &str) -> Option<u32> {
    let id = namespaced(effect);
    SIGHT
        .iter()
        .find(|(e, _, _)| *e == id)
        .map(|(_, ticks, _)| *ticks)
}

/// The **body moves** a blinding grant of `seconds` reaches (spec-0085 §6.2):
/// `ceil(seconds × speed)`, at walking speed for an effect that forbids the
/// sprint and at sprinting speed for one that does not.
pub fn reach_moves(seconds: u32, forbids_sprint: bool) -> usize {
    let speed = if forbids_sprint {
        crate::metrics::WALK_SPEED_BLOCKS_PER_SECOND
    } else {
        crate::metrics::SPRINT_SPEED_BLOCKS_PER_SECOND
    };
    (f64::from(seconds) * speed).ceil() as usize
}
