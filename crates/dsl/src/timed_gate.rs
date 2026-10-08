//! Timed gates: a door that closes on a clock once opened.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::serde_fields::{is_false, is_zero};
use crate::{AnchorId, FlagId, TimedGateId};

#[cfg(doc)]
use crate::TrapDisarm;

/// A stage-5 **timed gate** (spec-0016 §4): a gate region driven by a
/// deterministic open/close clock, so passage is a timing read rather than a
/// permanent state.
///
/// **The proof is deliberately NOT all-phase passability** — a gate that
/// punishes bad timing is the entire point. What the
/// compiler requires is that the gate is *readable*: the set of entry phases from
/// which a walking player clears the span before it shuts must cover **≥ 20% of
/// the cycle** (`DW0378`). Below that it is a coin flip, not a skill, and no
/// amount of learning the level makes it fair.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimedGate {
    /// Unique timed-gate id (`timed-gate/<kebab>`).
    pub id: TimedGateId,
    /// The gate anchor the clock drives. Its prefab metadata must declare a fill
    /// `block` (`DW0343`), the same requirement `close-gate` and `shortcut` have.
    pub gate: AnchorId,
    /// Ticks the gate stays OPEN each cycle (> 0).
    pub open_ticks: u32,
    /// Ticks the gate stays CLOSED each cycle (> 0).
    pub closed_ticks: u32,
    /// Ticks after world init before the first open window begins (default 0) —
    /// how two gates in the same room are put out of step with each other. Must
    /// be less than the full cycle.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub phase: u32,
    /// Whether the gate **kills** a player caught inside its region when it
    /// shuts (spec-0016 §4 addendum). A portcullis
    /// that merely shoves you aside teaches nothing; mistiming the crossing is
    /// supposed to be a death you learn from, which is the whole point of §4's
    /// ≥20%-of-cycle window proof — the window is fair, so the penalty can be
    /// absolute.
    ///
    /// This is a real judgement issued by command on the closing tick, not
    /// suffocation: vanilla's in-wall damage is slow, gear-dependent and
    /// escapable, so it would make the portcullis a suggestion. `damage` at the
    /// closing edge is exact and unarguable.
    ///
    /// **Defaults to `false`**, so every campaign authored before this field
    /// existed compiles byte-identically; a delve opts its portcullis in.
    #[serde(default, skip_serializing_if = "is_false")]
    pub crush: bool,
    /// Optional **disarm** affordance (souls dossier §5.2): the third
    /// rung of the hazard ladder — readable, avoidable, and finally *disable-able*.
    /// The real games' best timed hazards can be removed for good (Smouldering
    /// Lake's ballista, the Fringefolk chariot); a clock the party can only ever
    /// dance with is one rung short of the vocabulary.
    ///
    /// Interacting with the affordance suppresses the clock **permanently, with
    /// the gate resting OPEN** — a jammed portcullis stays up. Permanence is
    /// structural exactly as a `shortcut`'s is: no emitted function ever re-arms
    /// the clock, and `DW0389` refuses a campaign that spells a re-seal.
    ///
    /// **Defaults to absent**, so every campaign authored before this field
    /// existed compiles byte-identically; a delve opts its portcullis in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disarm: Option<TimedGateDisarm>,
}

/// A [`TimedGate`]'s disarm affordance (souls dossier §5.2) — the
/// exact shape a trap's [`TrapDisarm`] takes, and deliberately so: one affordance
/// grammar for every mechanism the party can switch off.
///
/// The player acts on the `via` anchor (a compiler-emitted interaction entity
/// plus its visible hardware, `DW0420`) to jam the gate. The clock stops with the
/// span cleared, `sets_flag` is raised party-wide, and nothing in the delve can
/// put the gate back.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimedGateDisarm {
    /// The anchor the player interacts with to jam the gate. Must be an anchor
    /// some area's prefab provides, and never the gate anchor itself — the
    /// mechanism belongs beside the doorway, not inside the span that crushes.
    pub via: AnchorId,
    /// The flag set when the gate is disarmed (a new flag this gate produces;
    /// other objectives/triggers may read it via `requires_flags`).
    pub sets_flag: FlagId,
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

use std::collections::BTreeSet;

use crate::diagnostic::{Diagnostic, codes};
use crate::envelope::Campaign;
use crate::registry::AnchorRegistry;
use crate::validate::{
    AnchorProviders, for_each_effect_deep, for_each_trigger_effect_deep, station_kind_diag,
};

/// Validate the stage-5 `timed_gates` section (spec-0016 §4), `DW0377` /
/// `DW0389`.
///
/// The structural half only: ids, a cycle that actually cycles, a phase inside
/// the cycle, one owner per gate region, and a `disarm.via` that
/// resolves to a real anchor outside the span it jams. The *design* half — that
/// the gate is a timing read and not a coin flip — needs the nav model's crossing
/// time and lives in `compiler::nav` (`DW0378`). The fill-block requirement is
/// `DW0343`, the same rule `close-gate` and `shortcut` obey.
///
/// `DW0389` is the permanence rule, and it is the exact mirror of a shortcut's
/// `DW0372`: a disarmed gate rests OPEN forever, so no `close-gate` anywhere may
/// name it. Making that structural is cheaper and safer than trusting every
/// author never to reach for the re-seal verb.
///
/// Anchor resolution stays lenient for pool areas the compiler resolves later —
/// the same policy as the trap and shortcut checks.
pub(crate) fn timed_gate_checks(
    c: &Campaign,
    anchors: &dyn AnchorRegistry,
    d: &mut Vec<Diagnostic>,
) {
    let quests = &c.quests.content;
    if quests.timed_gates.is_empty() {
        return;
    }
    let providers = AnchorProviders::build(c, anchors);
    let shortcut_gates: BTreeSet<&str> = quests.shortcuts.iter().map(|s| s.gate.as_str()).collect();
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut driven: BTreeSet<&str> = BTreeSet::new();
    for (i, g) in quests.timed_gates.iter().enumerate() {
        let err = |path: String, msg: String, d: &mut Vec<Diagnostic>| {
            d.push(Diagnostic::error(
                codes::TIMED_GATE_INVALID,
                "quests",
                path,
                msg,
            ));
        };
        if !g.id.is_valid_syntax() {
            err(
                format!("/content/timed_gates/{i}/id"),
                format!(
                    "malformed timed-gate id `{}` — ids must be lowercase kebab-case with the \
                     `timed-gate/` prefix (e.g. `timed-gate/piston-hall`)",
                    g.id
                ),
                d,
            );
        }
        if !seen.insert(g.id.as_str()) {
            err(
                format!("/content/timed_gates/{i}/id"),
                format!("duplicate timed-gate id `{}` — rename one", g.id),
                d,
            );
        }
        for (field, ticks) in [
            ("open_ticks", g.open_ticks),
            ("closed_ticks", g.closed_ticks),
        ] {
            if ticks == 0 {
                err(
                    format!("/content/timed_gates/{i}/{field}"),
                    format!(
                        "timed gate `{}` declares `{field}: 0` — a gate that never {} is not a \
                         timing gate. Use `open-gate`/`close-gate` for a one-way state change, or \
                         give both halves of the cycle a real duration.",
                        g.id,
                        if field == "open_ticks" {
                            "opens"
                        } else {
                            "closes"
                        }
                    ),
                    d,
                );
            }
        }
        let cycle = g.open_ticks.saturating_add(g.closed_ticks);
        if cycle > 0 && g.phase >= cycle {
            err(
                format!("/content/timed_gates/{i}/phase"),
                format!(
                    "timed gate `{}` declares `phase: {}` at or beyond its own {cycle}-tick cycle \
                     — a phase is an offset INTO the cycle, so it must be less than it (use \
                     `phase % cycle`).",
                    g.id, g.phase
                ),
                d,
            );
        }
        // The clock fills and clears a REGION twice a cycle, so `gate` demands a
        // gate station. This is the first shape question asked of
        // `timed_gates[].gate` at this tier at all: the name itself is resolved
        // only by the compiler, so a point named here used to travel all the way
        // to `DW0343`.
        if let Some(f) = station_kind_diag(
            &providers,
            g.gate.as_str(),
            crate::layout::StationKind::Gate,
            "a timed gate's `gate`",
            "quests",
            format!("/content/timed_gates/{i}/gate"),
        ) {
            d.push(f);
        }
        if !driven.insert(g.gate.as_str()) {
            err(
                format!("/content/timed_gates/{i}/gate"),
                format!(
                    "gate `{}` is driven by two timed gates — two clocks filling and clearing the \
                     same region race every tick and the region's state becomes emission order, \
                     not design. One clock per gate.",
                    g.gate
                ),
                d,
            );
        }
        if shortcut_gates.contains(g.gate.as_str()) {
            err(
                format!("/content/timed_gates/{i}/gate"),
                format!(
                    "gate `{}` is both a `shortcut` gate and a `timed-gate` — a shortcut opens \
                     PERMANENTLY (spec-0016 §2) and a clock would re-seal it every cycle, which \
                     is exactly the re-seal `DW0358` exists to forbid. Use two different gates.",
                    g.gate
                ),
                d,
            );
        }
        // The disarm affordance, the same two rules a trap's obeys.
        if let Some(dis) = &g.disarm {
            if let Some(f) = station_kind_diag(
                &providers,
                dis.via.as_str(),
                crate::layout::StationKind::Point,
                "a timed gate's disarm affordance",
                "quests",
                format!("/content/timed_gates/{i}/disarm/via"),
            ) {
                d.push(f);
            }
            if !providers.resolvable(dis.via.as_str()) {
                err(
                    format!("/content/timed_gates/{i}/disarm/via"),
                    format!(
                        "timed-gate `disarm.via` anchor `{}` is not provided by any area's \
                         prefab — {}",
                        dis.via,
                        providers.anchor_remedy(
                            "use an anchor some area's prefab exposes for the jam affordance \
                             (anchor names come from prefab metadata; do NOT invent one)"
                        ),
                    ),
                    d,
                );
            }
            if dis.via == g.gate {
                err(
                    format!("/content/timed_gates/{i}/disarm/via"),
                    format!(
                        "timed gate `{}` puts its `disarm.via` on its own gate anchor `{}` — the \
                         jam lever would stand inside the span the portcullis closes on (and, \
                         with `crush`, kills in). The affordance belongs on ground the player \
                         can reach and hold WITHOUT gambling on the clock, which is the entire \
                         point of the third rung.",
                        g.id, g.gate
                    ),
                    d,
                );
            }
        }
    }

    // `close-gate` may never target a disarmable timed gate: a disarm leaves the
    // portcullis jammed OPEN forever, so permanence is structural (`DW0389`, the
    // mirror of a shortcut's `DW0372`).
    let disarmed: BTreeSet<&str> = quests
        .timed_gates
        .iter()
        .filter(|g| g.disarm.is_some())
        .map(|g| g.gate.as_str())
        .collect();
    if disarmed.is_empty() {
        return;
    }
    let report = |path: String, anchor: &str, d: &mut Vec<Diagnostic>| {
        d.push(Diagnostic::error(
            codes::TIMED_GATE_REARMED,
            "quests",
            path,
            format!(
                "`close-gate` targets `{anchor}`, the gate of a `timed-gate` that declares a \
                 `disarm` — a disarmed gate rests OPEN permanently (souls dossier \
                 §5.2: a hazard the party has switched off stays off), so nothing may re-arm \
                 its clock. Use a different gate for the beat that must re-seal, or drop the \
                 `disarm` and keep the clock running."
            ),
        ));
    };
    for (qi, q) in quests.quests.iter().enumerate() {
        for_each_effect_deep(q, |path, eff| {
            if let Some(a) = eff.close_gate_anchor()
                && disarmed.contains(a.as_str())
            {
                report(format!("/content/quests/{qi}/{path}/anchor"), a.as_str(), d);
            }
        });
    }
    for (ti, t) in quests.triggers.iter().enumerate() {
        for_each_trigger_effect_deep(t, |path, eff| {
            if let Some(a) = eff.close_gate_anchor()
                && disarmed.contains(a.as_str())
            {
                report(
                    format!("/content/triggers/{ti}/{path}/anchor"),
                    a.as_str(),
                    d,
                );
            }
        });
    }
}
