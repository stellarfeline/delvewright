//! **Every class that can declare a skin is a class the bake walks, or CI is red.**
//!
//! A `skin` makes a body ship as a `minecraft:mannequin` whose
//! `profile.texture` resolves to `delvewright:npc/<texture_id>`, and the only
//! place that texture can come from is the per-delve resource pack. So a class
//! that may declare a skin owes exactly two outcomes and no third: the PNG is
//! **baked and served**, or its absence is **refused by name** (`DW0309`).
//!
//! The third state is what shipped. `read_skins` walked
//! `campaign.npcs.content.npcs` by hand, so a stage-5 actor's skin was read into
//! its summon, emitted as a texture reference, and then neither baked nor
//! refused — deleting an npc's PNG exited 3 with `DW0309`, and deleting an
//! actor's built green with the mannequin wearing whatever the client had. Same
//! deletion, one class fatal and the other invisible.
//!
//! This test is what keeps the repair from decaying. Like
//! `gate_consumers.rs` it does **not** read a hand-written list of classes:
//! it enumerates them from the generated JSON Schema, which `schemars` derives
//! from the Rust types, and requires every schema object that declares `skin` to
//! be a class [`BodyRef`] covers — because the walk the bake and the refusal
//! share is a filter over [`body_sites`], and a class outside that closed set is
//! a skin no bake can reach.
//!
//! It is a test rather than a `DW` diagnostic for the reason `gate_consumers.rs`
//! gives: the defect it catches is in the **compiler**, not in a campaign, so
//! there is no campaign input for a build-time diagnostic to fire on.

use std::collections::BTreeSet;

use delvewright_dsl::BodyRef;
use delvewright_dsl::envelope::Stage;
use serde_json::Value;

/// Every `$defs` object across every stage schema that declares a `skin`
/// property, by its schema name.
fn skin_declaring_classes() -> BTreeSet<String> {
    let mut out: BTreeSet<String> = BTreeSet::new();
    // `Stage::ALL`, never a list written here — `gate_consumers.rs` records what
    // a hand-written stage list cost when two documents landed at 0.13.0.
    for stage in Stage::ALL {
        let schema = delvewright_dsl::stage_schema(stage);
        if let Some(defs) = schema.get("$defs").and_then(Value::as_object) {
            for (name, def) in defs {
                collect(name, def, &mut out);
            }
        }
    }
    out
}

/// Walk one `$defs` entry, descending into `oneOf`/`anyOf`/`allOf` so an
/// internally-tagged enum's variants are visited as the separate object schemas
/// they are.
fn collect(name: &str, def: &Value, out: &mut BTreeSet<String>) {
    if let Some(props) = def.get("properties").and_then(Value::as_object)
        && props.contains_key("skin")
    {
        out.insert(name.to_string());
    }
    for key in ["oneOf", "anyOf", "allOf"] {
        if let Some(list) = def.get(key).and_then(Value::as_array) {
            for sub in list {
                collect(name, sub, out);
            }
        }
    }
}

/// Every object class in every stage schema — the denominator the binding count
/// below is a fraction OF, so "two classes declare a skin" is a measurement
/// rather than an impression.
fn object_classes() -> usize {
    fn walk(v: &Value, n: &mut usize) {
        match v {
            Value::Object(m) => {
                if m.get("properties").and_then(Value::as_object).is_some() {
                    *n += 1;
                }
                for x in m.values() {
                    walk(x, n);
                }
            }
            Value::Array(a) => {
                for x in a {
                    walk(x, n);
                }
            }
            _ => {}
        }
    }
    let mut n = 0usize;
    for stage in Stage::ALL {
        walk(&delvewright_dsl::stage_schema(stage), &mut n);
    }
    n
}

/// **A skin belongs to a body.** Every schema object that declares one is a
/// class the body walk covers.
#[test]
fn every_skin_declaring_class_is_a_body_the_walk_covers() {
    let declaring = skin_declaring_classes();
    let denominator = object_classes();

    // Binding first: a walk that found nothing would pass every assertion below,
    // which is the unbound green this project counts as a finding, not a pass.
    assert!(
        !declaring.is_empty(),
        "binding: the schema walk found NO class declaring `skin` at all across {denominator} \
         object schemas — the walk is broken, not the DSL"
    );
    println!(
        "skin binding: {} of {denominator} object schemas declare `skin` ({:?}); \
         `BodyRef` covers {:?}",
        declaring.len(),
        declaring,
        BodyRef::ALL_CLASSES
    );

    let covered: BTreeSet<String> = BodyRef::ALL_CLASSES
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    let unreachable: Vec<&String> = declaring.difference(&covered).collect();
    assert!(
        unreachable.is_empty(),
        "{} schema class(es) declare a `skin` that no body walk reaches: {unreachable:?}. \
         `dsl::body_skin_sites` is a filter over `body_sites`, and `body_sites` enumerates \
         `BodyRef`'s closed set — so a skin declared outside that set is emitted into a \
         mannequin's `profile.texture` and then never baked into the resource pack and never \
         refused by `DW0309`, which is the exact defect this file exists to prevent. Make the \
         new class a `BodyRef` variant (every consumer then has to say what it does with it), \
         never a second bake path.",
        unreachable.len()
    );

    // And the closed set is not wider than the schema: a name here that no
    // schema object has is a typo the compiler cannot catch, and would make the
    // check above pass for a class that does not exist.
    let all_objects: BTreeSet<String> = {
        let mut names = BTreeSet::new();
        for stage in Stage::ALL {
            if let Some(defs) = delvewright_dsl::stage_schema(stage)
                .get("$defs")
                .and_then(Value::as_object)
            {
                names.extend(defs.keys().cloned());
            }
        }
        names
    };
    for class in BodyRef::ALL_CLASSES {
        assert!(
            all_objects.contains(class),
            "`BodyRef::ALL_CLASSES` names `{class}`, which no stage schema declares — the \
             join between the Rust set and the schema is spelled wrong, so the coverage \
             check above is comparing against a class that does not exist"
        );
    }
}
