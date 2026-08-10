//! **Every gate consumer carries the whole gate, or CI is red.**
//!
//! Until DSL v0.10 the campaign's gate was two fields, `requires_flags` and
//! `forbids_flags`, declared twenty-eight separate times — once per objective
//! kind, once per gatable effect verb, and once each on the environment trigger,
//! the trap, the dialogue option and the cast placement. Nothing in the type
//! system said they were one thing.
//!
//! spec-0031 needed a third field on that gate: a numeric comparison against a
//! declared runtime datum. The shape of the code offered exactly one obvious
//! place to put it — the verb that first asked for it — and CLAUDE.md names that
//! move as the defect: *generality is decided at the FIRST site; a second bespoke
//! field is the defect, not the fix.*
//!
//! This test is what keeps the decision from decaying. It does **not** read a
//! hand-written list of consumers — a hand-written list is the exact defect this
//! project has paid for six times over in the `for_each_effect_root` family
//! (#301, #302, #321). It enumerates the consumers **from the generated JSON
//! Schema**, which `schemars` derives from the Rust types, and requires every
//! object schema that declares `requires_flags` to declare `requires_state` too.
//!
//! So a twenty-ninth gate consumer added tomorrow with only the flag pair on it
//! is red here, before it can ship a delve where a numeric gate silently cannot
//! be written.
//!
//! It is a test rather than a `DW` diagnostic for the same reason
//! `l10n_surface.rs` is: the defect it catches is in the **compiler**, not in a
//! campaign, so there is no campaign input for a build-time diagnostic to fire
//! on.

use std::collections::BTreeSet;

use delvewright_dsl::envelope::Stage;
use delvewright_dsl::gate::GateConsumer;
use serde_json::Value;

/// How many object schemas across the seven stage documents declare a gate.
///
/// Five objective kinds + nineteen gatable effect verbs + the environment
/// trigger + the trap + the dialogue option + the cast placement. Asserted
/// exactly rather than as a lower bound: this is the **binding count** the
/// generality claim rests on, and a green that bound to fewer sites than it
/// thinks is vacuous, not a pass (CLAUDE.md). Changing it is a deliberate act —
/// a new gate consumer, or a verb becoming gatable — and it should be visible in
/// the diff that does it.
const GATE_SITES: usize = 28;

/// The gate's fields, as they are spelled in the schema. Every site must declare
/// all of them.
const GATE_FIELDS: [&str; 3] = ["requires_flags", "forbids_flags", "requires_state"];

/// One object schema that declares a gate: `(type, variant)`. `variant` is the
/// serde tag for an enum variant, or an empty string for a plain struct.
type Site = (String, String);

/// Every gate-declaring object schema across the seven stage schemas, with the
/// gate fields each of them actually declares.
fn gate_sites() -> Vec<(Site, BTreeSet<String>)> {
    let mut out: Vec<(Site, BTreeSet<String>)> = Vec::new();
    let mut seen: BTreeSet<Site> = BTreeSet::new();
    for stage in [
        Stage::World,
        Stage::Npcs,
        Stage::Classes,
        Stage::QuestPlan,
        Stage::Quests,
        Stage::Dialogue,
        Stage::WorldEdits,
    ] {
        let schema = delvewright_dsl::stage_schema(stage);
        if let Some(defs) = schema.get("$defs").and_then(Value::as_object) {
            for (name, def) in defs {
                collect(name, def, &mut seen, &mut out);
            }
        }
    }
    out
}

/// Walk one `$defs` entry, descending into `oneOf`/`anyOf`/`allOf` so an
/// internally-tagged enum's variants are visited as the separate object schemas
/// they are.
fn collect(
    name: &str,
    def: &Value,
    seen: &mut BTreeSet<Site>,
    out: &mut Vec<(Site, BTreeSet<String>)>,
) {
    if let Some(props) = def.get("properties").and_then(Value::as_object)
        && props.contains_key("requires_flags")
    {
        // The serde tag, when this is an enum variant: `{"type": {"const": "…"}}`.
        let variant = props
            .get("type")
            .and_then(|t| t.get("const"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let site: Site = (name.to_string(), variant);
        if seen.insert(site.clone()) {
            let declared: BTreeSet<String> = GATE_FIELDS
                .iter()
                .filter(|f| props.contains_key(**f))
                .map(|f| (*f).to_string())
                .collect();
            out.push((site, declared));
        }
    }
    for key in ["oneOf", "anyOf", "allOf"] {
        if let Some(list) = def.get(key).and_then(Value::as_array) {
            for sub in list {
                collect(name, sub, seen, out);
            }
        }
    }
}

/// **The gate is one object.** Every schema object that declares any part of the
/// gate declares all of it.
#[test]
fn every_gate_consumer_carries_the_numeric_comparison() {
    let sites = gate_sites();

    // Binding first: a walk that found nothing would pass every assertion below.
    assert!(
        !sites.is_empty(),
        "binding: the schema walk found no gate-declaring object at all — the walk is \
         broken, not the DSL"
    );
    println!(
        "gate binding: {} gate-declaring object schemas examined ({} expected)",
        sites.len(),
        GATE_SITES
    );

    let incomplete: Vec<String> = sites
        .iter()
        .filter(|(_, declared)| declared.len() != GATE_FIELDS.len())
        .map(|((ty, variant), declared)| {
            let missing: Vec<&str> = GATE_FIELDS
                .iter()
                .filter(|f| !declared.contains(**f))
                .copied()
                .collect();
            let who = if variant.is_empty() {
                ty.clone()
            } else {
                format!("{ty}::{variant}")
            };
            format!("{who} is missing {missing:?}")
        })
        .collect();
    assert!(
        incomplete.is_empty(),
        "{} gate consumer(s) carry only part of the gate. A gate is ONE object — flags, \
         negative flags and the numeric comparison together — and a consumer that carries \
         two of the three simply cannot express the third, which is how a second bespoke \
         field ends up looking like the fix (CLAUDE.md). Add the missing field(s) to the \
         DECLARATION, and teach `dsl::gate` and the emitter about them, rather than adding \
         a new mechanism beside them: {incomplete:#?}",
        incomplete.len()
    );

    assert_eq!(
        sites.len(),
        GATE_SITES,
        "the number of gate-declaring object schemas changed. That is not wrong on its own \
         — a new gate consumer, or a verb becoming gatable, is a real change — but it is \
         the binding count this file's generality claim rests on, so it is stated \
         explicitly and updated deliberately. Sites found: {:#?}",
        sites.iter().map(|(s, _)| s).collect::<Vec<_>>()
    );
}

/// The **Rust-side** enumeration agrees with the schema's: every consumer class
/// in the closed set answers the questions a gate consumer has to answer, and
/// there are as many classes as there are kinds of gate-declaring object.
///
/// The schema test above proves no DECLARATION is partial; this one proves the
/// closed set that walks them is not narrower than the surface.
#[test]
fn the_consumer_set_covers_every_declaring_type() {
    // The distinct declaring TYPES in the schema (an enum counts once, however
    // many variants it has): Objective, QuestEffect, EnvTrigger, Trap,
    // DialogueOption, CastPlacement.
    let types: BTreeSet<String> = gate_sites().into_iter().map(|((ty, _), _)| ty).collect();
    println!(
        "gate consumer binding: {} declaring types, {} consumer classes",
        types.len(),
        GateConsumer::COUNT
    );
    assert_eq!(
        types.len(),
        GateConsumer::COUNT,
        "`GateConsumer` is the closed set of things that carry a gate, and the schema says \
         there are {} distinct declaring types: {types:#?}. A type in the schema with no \
         `GateConsumer` variant is a gate no proof written against `Gate` can reach.",
        types.len()
    );
    for k in GateConsumer::ALL {
        assert!(!k.label().is_empty(), "{k:?} has no label");
        assert!(matches!(k.stage(), "quests" | "dialogue"), "{k:?}");
        // Every class must have an answer to "does emission evaluate this gate
        // against an acting player?" — it is what makes a `player`-scoped datum's
        // readability decidable (`DW0503`) rather than guessed. `Effect` answers
        // `None` ("ask the root"), which is an answer; a class that could not
        // answer at all would not compile.
        let _: Option<bool> = k.evaluates_per_player();
    }
    // Binding: exactly one class defers to the root, and both definite answers
    // occur — a set that answered the same thing everywhere would make `DW0503`'s
    // per-site reasoning vacuous.
    assert_eq!(
        GateConsumer::ALL
            .iter()
            .filter(|k| k.evaluates_per_player().is_none())
            .count(),
        1,
        "only the effect class defers to the effect root"
    );
    assert!(
        GateConsumer::ALL
            .iter()
            .any(|k| k.evaluates_per_player() == Some(true))
            && GateConsumer::ALL
                .iter()
                .any(|k| k.evaluates_per_player() == Some(false)),
        "both definite answers must occur"
    );
}
