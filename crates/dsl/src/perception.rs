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

// ---------------------------------------------------------------------------
// Validation — the checks `dsl::validate` runs over this object (ADR-0031)
// ---------------------------------------------------------------------------

use crate::diagnostic::{Diagnostic, codes};
use crate::envelope::Campaign;
use crate::{QuestEffect, Verb};

/// **The perception surface, at every effect root** (spec-0085).
///
/// Four rules over one descent (the single root enumeration and the single
/// nesting authority, so a beat inside a `sequence` step of a dialogue
/// `on_respawn` is asked exactly what a top-level one is):
///
/// * `DW0941` — a `particle` id the pinned registry does not hold, or one whose
///   type takes options;
/// * `DW0100` — a `particle` `count` of zero, which the exported schema refuses
///   (`minimum: 1`) and serde does not;
/// * `DW0942` — the envelope's `audience` or `in` on a verb the emitter fires
///   once for the world ([`Verb::addresses_players`]);
/// * `DW0944` — in one timeline, a sight grant whose window overlaps a cutscene
///   step's and ends inside it or within its own wind-down after it.
pub(crate) fn perception_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    fn descend(stage: &'static str, path: String, eff: &QuestEffect, d: &mut Vec<Diagnostic>) {
        perception_one(stage, &path, eff, d);
        for (pseg, _kseg, list) in eff.nested_effect_lists_labeled() {
            if !matches!(eff.verb, Verb::Sequence { .. }) {
                // A nested bundle is a timeline of one step, at tick 0.
                sight_under_camera(stage, &[(0, format!("{path}/{pseg}"), list)], d);
            }
            for (j, inner) in list.iter().enumerate() {
                descend(stage, format!("{path}/{pseg}/{j}"), inner, d);
            }
        }
    }
    crate::effects::for_each_effect_root(c, &mut |site, effs| {
        // A root's bundle fires all at once: a timeline of one step, at tick 0.
        sight_under_camera(site.stage, &[(0, site.path.clone(), effs)], d);
        for (i, eff) in effs.iter().enumerate() {
            descend(site.stage, format!("{}/{i}", site.path), eff, d);
        }
    });
}

/// One effect's perception rules, at the pointer it was found at.
fn perception_one(stage: &'static str, path: &str, eff: &QuestEffect, d: &mut Vec<Diagnostic>) {
    use crate::perception;
    if let Verb::Particle {
        particle, count, ..
    } = &eff.verb
    {
        match perception::particle_takes_options(particle) {
            None => d.push(Diagnostic::error(
                codes::PERCEPTION_UNKNOWN_PARTICLE,
                stage,
                format!("{path}/particle"),
                format!(
                    "`particle` `{particle}` is not a particle type the pinned 1.21.11 game \
                     registers ({} types, `crates/dsl/data/particles-1.21.11.json`). Use a \
                     registered id — the full-screen face is `minecraft:elder_guardian`",
                    perception::particle_registry().len()
                ),
            )),
            Some(true) => d.push(Diagnostic::error(
                codes::PERCEPTION_UNKNOWN_PARTICLE,
                stage,
                format!("{path}/particle"),
                format!(
                    "`particle` `{particle}` is a type that takes options (a colour, a block, an \
                     item, a destination), and the `particle` verb carries none, so the game would \
                     refuse the command. A particle with options is excluded until the engine \
                     states what each one takes; choose a type a bare id spawns"
                ),
            )),
            Some(false) => {}
        }
        if *count == Some(0) {
            d.push(Diagnostic::error(
                codes::SCHEMA,
                stage,
                format!("{path}/count"),
                "`particle` `count` is 0. Zero is vanilla's spelling of a different thing — one \
                 particle given a velocity — and the schema's minimum is 1. Write the number of \
                 particles, at least 1"
                    .to_string(),
            ));
        }
    }
    if !eff.addresses_players() {
        for (field, present) in [
            ("audience", eff.audience.is_some()),
            ("in", eff.within.is_some()),
        ] {
            if present {
                d.push(Diagnostic::error(
                    codes::PERCEPTION_AUDIENCE_ON_A_PARTY_FACT,
                    stage,
                    format!("{path}/{field}"),
                    format!(
                        "`{}` declares `{field}`, and a `{}` fires once for the world — it \
                         addresses no player, so there is no audience to narrow. `audience` and \
                         `in` belong on the effects a player sees, hears or receives (`narrate`, \
                         `play-sound`, `particle`, `give-effect`, `clear-effect`, \
                         `damage-players`, `give-item`); a `sequence`'s steps each state their \
                         own. Remove `{field}` here, or move it onto those effects",
                        eff.verb.tag(),
                        eff.verb.tag()
                    ),
                ));
            }
        }
    }
    if let Verb::Sequence { steps } = &eff.verb {
        let groups: Vec<(u32, String, &[QuestEffect])> = steps
            .iter()
            .enumerate()
            .map(|(si, st)| {
                (
                    st.at_ticks,
                    format!("{path}/steps/{si}/effects"),
                    st.effects.as_slice(),
                )
            })
            .collect();
        sight_under_camera(stage, &groups, d);
    }
}

/// `DW0944`: in one timeline, a sight grant that ends under a camera
/// (spec-0085 §5.3). A bundle — a root's list or a nested one — fires all at
/// once, so it is judged as a timeline of one step at tick 0; a `sequence` is
/// judged step by step.
///
/// A grant's window is `[at_ticks, at_ticks + 20 × seconds)`; a cutscene step's
/// is `[at_ticks, at_ticks + 20 × Σ shot seconds)`. A grant that overlaps a
/// shot's window and ends at or after its start and before its end plus the
/// effect's wind-down ([`crate::perception::sight_wind_down_ticks`]) starts
/// ramping down on screen. Nothing outside one timeline is examined: a grant
/// with no cutscene in its timeline is not this rule's business.
fn sight_under_camera(
    stage: &'static str,
    groups: &[(u32, String, &[QuestEffect])],
    d: &mut Vec<Diagnostic>,
) {
    let mut shots: Vec<(String, u32, u32)> = Vec::new();
    for (at, list_path, effects) in groups {
        for (ei, e) in effects.iter().enumerate() {
            if let Some(list) = e.cutscene_shots().filter(|l| !l.is_empty()) {
                let len: u32 = list.iter().map(|s| s.resolved_seconds() * 20).sum();
                shots.push((format!("{list_path}/{ei}"), *at, at + len));
            }
        }
    }
    if shots.is_empty() {
        return;
    }
    for (at, list_path, effects) in groups {
        for (ei, e) in effects.iter().enumerate() {
            let Some((effect, seconds, _, _, _)) = e.give_effect() else {
                continue;
            };
            let Some(wind) = crate::perception::sight_wind_down_ticks(effect) else {
                continue;
            };
            let begin = *at;
            let end = begin + seconds * 20;
            for (shot, c0, c1) in &shots {
                let (c0, c1) = (*c0, *c1);
                let overlaps = begin < c1 && end > c0;
                if overlaps && end < c1 + wind {
                    d.push(Diagnostic::error(
                        codes::PERCEPTION_SIGHT_UNDER_A_CAMERA,
                        stage,
                        format!("{list_path}/{ei}"),
                        format!(
                            "`give-effect` `{effect}` runs from tick {begin} to tick {end} of \
                             this timeline, and the cutscene at `{shot}` holds the camera from \
                             tick {c0} to tick {c1}. The grant ends at tick {end}, inside the \
                             shot or within {wind} tick(s) of its end — the effect's wind-down \
                             — so it starts ramping down on screen. A granted sight effect \
                             outlasts any authored camera it overlaps, plus its wind-down: write \
                             a `seconds` of at least {need}, or start it after the shot (a later \
                             `at_ticks`)",
                            need = (c1 + wind - begin).div_ceil(20),
                        ),
                    ));
                }
            }
        }
    }
}
