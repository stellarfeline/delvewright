//! Pulses: a sound that beats over a place (spec-0102).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{Guard, Mark, PlaceRef, PulseId};

/// A stage-5 **pulse** (spec-0102): a vanilla sound event that sounds from a
/// mark on a fixed interval, to every player standing in a place, loudest at
/// the mark, while its gate holds.
///
/// # A declared thing with a gate, not a verb pair
///
/// A pulse is a runtime object the campaign declares and a story stage
/// switches, in the class of a lethal volume (`when`), a loop and a timed gate
/// (a self-sustaining `schedule` chain). A `start` / `stop` verb pair would
/// re-introduce order — whichever ran last wins — where a gate is a
/// declaration every proof reads at every configuration through the one
/// `Plan::gate_terms`. A beat that quickens is a second pulse on the same
/// source with a disjoint gate.
///
/// # The reach is derived, never typed
///
/// The creator writes how loud the beat is at the farthest cell a body can
/// stand in inside the place ([`Pulse::floor`]); the compiler measures that
/// distance and derives the range and the emitted volume from it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Pulse {
    /// Unique pulse id (`pulse/<kebab>`).
    pub id: PulseId,
    /// A pinned vanilla sound event, `minecraft:` optional — validated against
    /// the one registry every sound id is (`DW0326`).
    pub sound: String,
    /// The source: the anchor's cell plus `offset`, bound to its piece
    /// (`DW0897`). The sound stands at the cell's centre; it need not lie inside
    /// the place it is heard in.
    pub at: Mark,
    /// Where it is heard: exactly one of `region` / `place` (`DW0929`). The
    /// bounds are taken as they are.
    #[serde(flatten)]
    pub heard: PlaceRef,
    /// The interval in server ticks, at least 1 (`DW0993`).
    pub every: u32,
    /// How loud the beat is at the farthest cell a body can stand in inside the
    /// place, as a fraction of the loudness at the source: `0 ≤ floor < 1`
    /// (`DW0993`). Required — there is no default loudness nobody declared.
    pub floor: f64,
    /// The pitch, `[0, 2]` (`DW0993`); default 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pitch: Option<f64>,
    /// **When it beats**: the one [`Guard`] every gated object carries. Absent,
    /// the pulse beats from world load to the end; present, it beats while the
    /// gate holds. `when: {}` and a term on a `player`-scoped datum are refused
    /// (`DW0993`): a pulse is a party fact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Guard>,
}

impl Pulse {
    /// The pitch the line is written with.
    pub fn pitch(&self) -> f64 {
        self.pitch.unwrap_or(1.0)
    }
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

use std::collections::BTreeSet;

use crate::diagnostic::{Diagnostic, DwCode, ExitTier, codes};
use crate::envelope::Campaign;
use crate::registry::AnchorRegistry;
use crate::validate::{AnchorProviders, station_kind_diag};

crate::dw_code! {
    /// (spec-0102 §6.1) **A pulse declared against itself.** An `every` of 0
    /// (`schedule … 0t` is the same tick again, forever); a `floor` outside
    /// `[0, 1)`; a `pitch` outside `[0, 2]`; `when: {}` (a stage with no term);
    /// a `requires_state` term on a `player`-scoped datum (a pulse is a party
    /// fact); a duplicate id. Validation-tier (exit 1). Prescription: the
    /// value inside its range, leave `when` out or name a flag or a `party`
    /// datum, or a distinct id.
    pub const PULSE_DECL: DwCode = DwCode::new("DW0993", ExitTier::Build);
}

/// Stage-5 pulse checks (spec-0102 §6.1): id syntax, `DW0993`'s shapes, the
/// source and region anchors resolvable, a named place that exists.
///
/// The sound id is `DW0326` and the neither/both of `region` / `place` is
/// `DW0929`, both raised by the compiler where every sound and every place
/// reference is read; a mark outside its piece is `DW0897`.
pub(crate) fn pulse_checks(c: &Campaign, anchors: &dyn AnchorRegistry, d: &mut Vec<Diagnostic>) {
    let pulses = &c.quests.content.pulses;
    if pulses.is_empty() {
        return;
    }
    let providers = AnchorProviders::build(c, anchors);
    let places = crate::world::place_ids(c);
    let mut seen_id: BTreeSet<&str> = BTreeSet::new();
    for (i, p) in pulses.iter().enumerate() {
        let at = |tail: &str| format!("/content/pulses/{i}{tail}");
        if !p.id.is_valid_syntax() {
            d.push(Diagnostic::error(
                codes::ID_SYNTAX,
                "quests",
                at("/id"),
                format!(
                    "malformed pulse id `{}` — pulse ids must be lowercase kebab-case with the \
                     `pulse/` prefix (e.g. `pulse/the-heart`)",
                    p.id
                ),
            ));
        }
        if !seen_id.insert(p.id.as_str()) {
            d.push(Diagnostic::error(
                PULSE_DECL,
                "quests",
                at("/id"),
                format!(
                    "pulse id `{}` is declared twice. Two pulses may share a source and a sound \
                     (a quickening beat is two declarations); they may not share an id — give \
                     each a distinct id",
                    p.id
                ),
            ));
        }
        if p.every == 0 {
            d.push(Diagnostic::error(
                PULSE_DECL,
                "quests",
                at("/every"),
                format!(
                    "pulse `{}` beats `every: 0` ticks — `schedule … 0t` is the same tick again, \
                     forever. Write the interval in server ticks, at least 1 (20 ticks is one \
                     second)",
                    p.id
                ),
            ));
        }
        if !(p.floor.is_finite() && (0.0..1.0).contains(&p.floor)) {
            d.push(Diagnostic::error(
                PULSE_DECL,
                "quests",
                at("/floor"),
                format!(
                    "pulse `{}` declares `floor: {}` — the loudness at the farthest cell a body \
                     can stand in, as a fraction of the loudness at the source, is at least 0 \
                     and less than 1 (at 1 the reach would be infinite). Write the value \
                     inside `[0, 1)`",
                    p.id, p.floor
                ),
            ));
        }
        if let Some(pitch) = p.pitch
            && !(pitch.is_finite() && (0.0..=2.0).contains(&pitch))
        {
            d.push(Diagnostic::error(
                PULSE_DECL,
                "quests",
                at("/pitch"),
                format!(
                    "pulse `{}` declares `pitch: {pitch}`, outside the `[0, 2]` the pinned \
                     `playsound` accepts. Write the value inside `[0, 2]`",
                    p.id
                ),
            ));
        }
        if let Some(g) = &p.when
            && g.requires_flags.is_empty()
            && g.forbids_flags.is_empty()
            && g.requires_state.is_empty()
        {
            d.push(Diagnostic::error(
                PULSE_DECL,
                "quests",
                at("/when"),
                format!(
                    "pulse `{}` declares `when: {{}}` — a stage with no term. A pulse that beats \
                     from world load is spelled by leaving `when` out; to stage it, name a flag \
                     (`requires_flags` / `forbids_flags`) or a `party`-scoped datum \
                     (`requires_state`)",
                    p.id
                ),
            ));
        }
        let mut anchors_named: Vec<(String, &str, &str)> =
            vec![("/at/anchor".to_string(), p.at.anchor.as_str(), "a pulse's source")];
        if let Some(r) = &p.heard.region {
            anchors_named.push((
                "/region/anchor".to_string(),
                r.anchor.as_str(),
                "a pulse's region centre",
            ));
        }
        for (field, anchor, what) in anchors_named {
            if let Some(f) = station_kind_diag(
                &providers,
                anchor,
                crate::layout::StationKind::Point,
                what,
                "quests",
                at(&field),
            ) {
                d.push(f);
            }
            if !providers.resolvable(anchor) {
                d.push(Diagnostic::error(
                    codes::ANCHOR_UNRESOLVED,
                    "quests",
                    at(&field),
                    format!(
                        "pulse `{}` names anchor `{anchor}` ({what}), which no prefab bound in \
                         this campaign provides — {}",
                        p.id,
                        providers.anchor_remedy(
                            "use an anchor the prefab exposes (anchor names come from prefab \
                             metadata; do NOT invent one)"
                        ),
                    ),
                ));
            }
        }
        if let Some(place) = &p.heard.place {
            crate::ids::dangling(
                d,
                places.contains(place.as_str()),
                "quests",
                at("/place"),
                format!(
                    "pulse `{}` is heard in unknown place `{place}` — name an `area/…` from \
                     `world.areas[]` or a site-plan box's `node/…`",
                    p.id
                ),
            );
        }
    }
}
