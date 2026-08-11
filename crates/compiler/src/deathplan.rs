//! `validation/death-plan.json` — **the bot tier's contract for dying** (spec-0031
//! lethal volumes, spec-0032 `on_death` + the recovery stake).
//!
//! # Why this artifact has to exist at all
//!
//! A PackTest fake player is permanently undamageable — measured independently on
//! 2026-08-03 and again on 2026-08-09 — so that tier **cannot witness a player
//! death**. Every runtime claim about the death loop therefore belongs to the
//! mineflayer tier, which drives a real client that can really die, and a real
//! client needs to be *told what the campaign promised* before it can say whether
//! the promise was kept. `combat-plan.json` is the same idea for fighting; this is
//! the same idea for dying.
//!
//! # What it carries, and what it deliberately does not
//!
//! Every field here is a **declaration** — a lethal volume's box and its wording,
//! a stake's forfeit rule and its collect policy, the currency's objective, the
//! respawn seats — plus the one thing the compiler *computed* and therefore owes a
//! runtime check on: [`crate::stake`]'s placement table, as (seat, region) → anchor.
//!
//! It carries **no emitted function name, no generated command and no objective
//! the engine invented for its own bookkeeping** (`dw.kl0_*`, `#stk_amt`, …). That
//! is the whole discipline of the file: an assertion written by reading the
//! emitter cannot fail when the emitter is wrong, so the bot is handed the promise
//! and left to observe reality against it. The one apparent exception, the
//! currency's scoreboard objective, is not one: spec-0032 *decided* that a
//! currency IS a scoreboard ledger ("Currency is a ledger, not an item"), so its
//! objective is the declaration, not an implementation detail.
//!
//! # Binding (`docs/reference/playtest-methodology.md` rule 1)
//!
//! [`DeathPlanGate`] states what the bot tier will be able to examine before it
//! examines anything: how many volumes, stakes, seats and table rows exist. A plan
//! that binds to nothing says so in the file, so a green run over an empty contract
//! can never read as a proven death loop.

use delvewright_dsl::{Campaign, QuestEffect};
use serde_json::{Value, json};

use crate::nav::World;
use crate::plan::Plan;
use crate::stake::StakeTable;

/// The contract's own shape version, beside the campaign's `dsl_version` (which
/// this artifact carries as `version`, exactly as `combat-plan.json` does). Bumped
/// when a field the harness reads changes meaning; the harness refuses a
/// `format_version` it does not know, so an old bot can never silently
/// under-assert a new build.
pub const DEATH_PLAN_FORMAT_VERSION: u32 = 1;

/// What the bot tier will be able to examine — counted at build time, so a reader
/// of the artifact alone can tell an empty contract from a proven one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeathPlanGate {
    /// Lethal volumes that resolved to a box (a place the bot can walk into and die).
    pub volumes: usize,
    /// Effects the campaign's `on_death` bundle fires, at every nesting depth.
    pub on_death_effects: usize,
    /// Recovery stakes declared.
    pub stakes: usize,
    /// Respawn seats the placement table is keyed on.
    pub seats: usize,
    /// Rows of the placement table — one per (seat, death region) pair.
    pub rows: usize,
}

impl DeathPlanGate {
    /// Whether this contract lets the bot tier prove anything about dying.
    ///
    /// A campaign with no lethal volume has nowhere for the bot to *cause* a death
    /// from content, and a campaign with no `on_death` has no consequence to
    /// assert. Either alone makes the plan inert.
    pub fn unbound(&self) -> bool {
        self.volumes == 0 || self.on_death_effects == 0
    }

    /// Why the contract binds to nothing, for the run report to print verbatim.
    /// `None` when it binds.
    pub fn reason(&self) -> Option<String> {
        match (self.volumes, self.on_death_effects) {
            (0, 0) => Some(
                "this campaign declares neither a lethal volume nor an `on_death` bundle: the bot \
                 tier has no content-caused death to walk into and no promised consequence to \
                 assert"
                    .to_string(),
            ),
            (0, _) => Some(
                "this campaign declares an `on_death` bundle but no lethal volume, so the bot \
                 tier has no place it can walk into to CAUSE a death from content — the promise \
                 is untestable from a client until the campaign declares one"
                    .to_string(),
            ),
            (_, 0) => Some(
                "this campaign declares a lethal volume but no `on_death` bundle: the bot can die \
                 in it, and there is no declared consequence for the death to have"
                    .to_string(),
            ),
            _ => None,
        }
    }

    /// The ledger, as it rides inside the artifact.
    pub fn to_json(&self) -> Value {
        json!({
            "lethal_volumes": self.volumes,
            "on_death_effects": self.on_death_effects,
            "stakes": self.stakes,
            "respawn_seats": self.seats,
            "placement_rows": self.rows,
            "unbound": self.unbound(),
            "reason": self.reason(),
        })
    }
}

/// Every effect in `effs`, at every nesting depth.
///
/// Reads [`QuestEffect::nested_effect_lists`] — the DSL's own answer to "what does
/// this effect contain" — rather than matching on the variants that happen to nest
/// today. A hand-rolled descent here would be the #301/#302/#321 defect in a new
/// place: it would silently stop counting the moment a later verb grew a body.
fn deep_effects(effs: &[QuestEffect]) -> Vec<&QuestEffect> {
    let mut out: Vec<&QuestEffect> = Vec::new();
    let mut stack: Vec<&[QuestEffect]> = vec![effs];
    while let Some(list) = stack.pop() {
        for e in list {
            out.push(e);
            for nested in e.nested_effect_lists() {
                stack.push(nested);
            }
        }
    }
    out
}

/// An l10n-tagged authored string, split into the key the delve ships it under and
/// the canonical English a client that has no translation will actually read.
///
/// The bot asserts the **English**, because that is what a run against the default
/// build puts on the wire (every component carries `fallback`); the key travels
/// beside it so a localized run can assert the same line by its key instead.
fn worded(s: &str) -> (Option<&str>, &str) {
    match delvewright_dsl::l10n::untag(s) {
        Some((key, english)) => (Some(key), english),
        None => (None, s),
    }
}

/// A `[lo, hi]` inclusive box, as the harness reads it.
fn boxed(lo: [i32; 3], hi: [i32; 3]) -> Value {
    json!({ "lo": lo, "hi": hi })
}

/// The declared currencies, keyed by the state id a stake names, with the
/// scoreboard objective spec-0032 decided they are kept in.
fn currency(c: &Campaign, state_id: &str) -> Value {
    let decl = c
        .quests
        .content
        .state
        .iter()
        .find(|s| s.id.as_str() == state_id);
    let (name_key, name) = match decl.and_then(|d| d.name.as_deref()) {
        Some(n) => {
            let (k, e) = worded(n);
            (k.map(str::to_string), Some(e.to_string()))
        }
        None => (None, None),
    };
    json!({
        "state": state_id,
        // spec-0032: "Currency is a ledger, not an item" — the objective IS the
        // declaration, which is why naming it here is not reading the emitter.
        "objective": crate::plan::state_score(state_id),
        "initial": decl.map(|d| d.initial),
        "scope": decl.map(|d| match d.scope {
            delvewright_dsl::StateScope::Player => "player",
            delvewright_dsl::StateScope::Party => "party",
        }),
        "name": name,
        "name_key": name_key,
    })
}

/// Build the artifact for a campaign that has a death loop to prove.
///
/// Returns `None` — no file, no byte moved — for a campaign that declares no
/// lethal volume, no `on_death` and no stake. Every campaign written before
/// spec-0031 is in that set, which is the feature's byte-identity guarantee.
///
/// `table` is [`crate::stake::build`]'s answer, `None` for a campaign that declares
/// no stake. Its absence is recorded as an empty placement rather than as a hole:
/// the seats are still enumerated, because "respawn puts the player where the
/// checkpoint in force says" is a promise a lethal volume makes with or without a
/// purse to lose.
pub fn build(
    plan: &Plan,
    world: &World,
    entry: Option<[i32; 3]>,
    table: Option<&StakeTable>,
) -> Option<Value> {
    let c = plan.campaign;
    let on_death = plan.on_death();
    let stakes = &c.quests.content.stakes;
    if plan.lethal_volumes.is_empty() && on_death.is_empty() && stakes.is_empty() {
        return None;
    }

    let volumes: Vec<Value> = plan
        .lethal_volumes
        .iter()
        .map(|v| {
            let (key, english) = worded(&v.message);
            json!({
                "id": v.id,
                "region": boxed(v.region.0, v.region.1),
                "message": english,
                "message_key": key,
                "damage_type": v.damage_type.id(),
            })
        })
        .collect();

    let deep = deep_effects(on_death);
    let mut drops: Vec<&str> = deep
        .iter()
        .filter_map(|e| match e {
            QuestEffect::DropStake { stake, .. } => Some(stake.as_str()),
            _ => None,
        })
        .collect();
    drops.dedup();

    let stakes_json: Vec<Value> = stakes
        .iter()
        .map(|s| {
            let (msg_key, msg) = worded(&s.collected_message);
            let forfeit = match s.forfeit() {
                delvewright_dsl::Forfeit::All => json!({ "kind": "all" }),
                delvewright_dsl::Forfeit::None => json!({ "kind": "none" }),
                delvewright_dsl::Forfeit::Proportion { percent } => {
                    json!({ "kind": "proportion", "percent": percent })
                }
                delvewright_dsl::Forfeit::Fixed { amount } => {
                    json!({ "kind": "fixed", "amount": amount })
                }
            };
            json!({
                "id": s.id,
                "currency": currency(c, s.state.as_str()),
                "forfeit": forfeit,
                "max_live": s.max_live(),
                "on_full": s.on_full().token(),
                "collect_by": s.collect_by().token(),
                "collected_message": msg,
                "collected_message_key": msg_key,
                "marker_item": s.marker_item(),
            })
        })
        .collect();

    // The seats. Taken from the placement table when there is one — so the bot
    // compares against exactly the seats the table was keyed on — and derived the
    // same way when there is not, which is [`crate::stake::seats`]'s whole reason
    // for being public.
    let seats_src = match table {
        Some(t) => t.seats.clone(),
        None => crate::stake::seats(plan, world, entry),
    };
    let seats: Vec<Value> = seats_src
        .iter()
        .map(|s| json!({ "cp": s.cp, "label": s.label, "cell": s.cell }))
        .collect();

    // The regions and rows. A region carries the lethal volume's id when it IS one,
    // so the harness matches a death region to the box it walked into by NAME
    // rather than by reproducing this module's ordering arithmetic.
    let (regions, rows) = match table {
        Some(t) => (
            t.regions
                .iter()
                .map(|r| {
                    // Matched by BOX, not by index. The table happens to order its
                    // lethal regions exactly as `plan.lethal_volumes` does today, and
                    // an index here would inherit that as an invisible contract — a
                    // later reordering in `compiler::stake` would silently hand the
                    // bot tier the wrong volume's name and every assertion would
                    // still be green.
                    let volume = r.lethal.then(|| {
                        plan.lethal_volumes
                            .iter()
                            .find(|v| v.region == r.region)
                            .map(|v| v.id.clone())
                    });
                    json!({
                        "label": r.label,
                        "lethal": r.lethal,
                        "volume": volume.flatten(),
                        "region": boxed(r.region.0, r.region.1),
                    })
                })
                .collect::<Vec<_>>(),
            t.rows
                .iter()
                .map(|r| {
                    json!({
                        "seat": r.seat,
                        "region": r.region,
                        "anchor": t.anchors[r.anchor],
                    })
                })
                .collect::<Vec<_>>(),
        ),
        None => (Vec::new(), Vec::new()),
    };

    let gate = DeathPlanGate {
        volumes: plan.lethal_volumes.len(),
        on_death_effects: deep.len(),
        stakes: stakes.len(),
        seats: seats.len(),
        rows: rows.len(),
    };

    Some(json!({
        "campaign_id": c.world.campaign_id,
        "version": c.world.dsl_version,
        "format_version": DEATH_PLAN_FORMAT_VERSION,
        "lethal_volumes": volumes,
        "on_death": { "effects": deep.len(), "drops_stake": drops },
        "stakes": stakes_json,
        "placement": { "seats": seats, "regions": regions, "rows": rows },
        "binding": gate.to_json(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A contract with no lethal volume cannot prove a content-caused death, and
    /// says so rather than passing quietly — playtest-methodology rule 1.
    #[test]
    fn a_plan_with_no_volume_is_unbound_and_names_the_reason() {
        let g = DeathPlanGate {
            volumes: 0,
            on_death_effects: 3,
            stakes: 1,
            seats: 2,
            rows: 4,
        };
        assert!(g.unbound());
        let why = g.reason().expect("an unbound gate owes a reason");
        assert!(why.contains("no lethal volume"), "{why}");
        assert_eq!(g.to_json()["unbound"], json!(true));
    }

    /// …and the mirror: a place to die with nothing promised for it.
    #[test]
    fn a_plan_with_no_on_death_is_unbound_too() {
        let g = DeathPlanGate {
            volumes: 1,
            on_death_effects: 0,
            ..DeathPlanGate::default()
        };
        assert!(g.unbound());
        assert!(g.reason().unwrap().contains("no `on_death` bundle"));
    }

    #[test]
    fn a_plan_with_both_binds() {
        let g = DeathPlanGate {
            volumes: 1,
            on_death_effects: 1,
            stakes: 1,
            seats: 2,
            rows: 4,
        };
        assert!(!g.unbound());
        assert_eq!(g.reason(), None);
        assert_eq!(g.to_json()["placement_rows"], json!(4));
    }

    /// The nested descent counts what a `sequence` hides. Written against the DSL's
    /// own `nested_effect_lists`, so a verb that grows a body later is counted
    /// without anybody remembering to come back here.
    #[test]
    fn deep_effects_reaches_inside_a_sequence() {
        let json = serde_json::json!([
            { "type": "narrate", "text": "you fall" },
            { "type": "sequence", "steps": [
                { "at_ticks": 0, "effects": [ { "type": "drop-stake", "stake": "stake/embers" } ] }
            ] }
        ]);
        let effs: Vec<QuestEffect> = serde_json::from_value(json).expect("fixture parses");
        let deep = deep_effects(&effs);
        assert_eq!(deep.len(), 3, "narrate + sequence + the nested drop-stake");
        assert!(
            deep.iter()
                .any(|e| matches!(e, QuestEffect::DropStake { .. })),
            "a nested drop-stake must be reachable — a hand-rolled walk over the top \
             level would report this campaign as promising nothing on death"
        );
    }

    /// The wording split: the bot asserts the English a default build puts on the
    /// wire, and carries the key for a localized run.
    #[test]
    fn worded_splits_a_tagged_string_into_key_and_english() {
        let tagged = delvewright_dsl::l10n::tag("lethal.the-drop.message", "The floor gives way.");
        assert_eq!(
            worded(&tagged),
            (Some("lethal.the-drop.message"), "The floor gives way.")
        );
        assert_eq!(worded("untagged"), (None, "untagged"));
    }
}
