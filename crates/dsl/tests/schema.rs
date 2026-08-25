//! Exported JSON Schemas accept every valid fixture and reject the schema-level
//! invalid fixtures (proves the LLM-facing schema matches the Rust truth).

mod common;

use delvewright_dsl::{Stage, stage_schema};

fn assert_valid_against_stage(name: &str, stage: Stage) {
    let src = common::read_valid(name);
    let instance: serde_json::Value = serde_json::from_str(&src).unwrap();
    let schema = stage_schema(stage);
    let validator = jsonschema::validator_for(&schema).expect("compile schema");
    if let Err(err) = validator.validate(&instance) {
        panic!("valid fixture {name} rejected by its schema: {err}");
    }
}

#[test]
fn schemas_accept_valid_fixtures() {
    assert_valid_against_stage("world.json", Stage::World);
    assert_valid_against_stage("npcs.json", Stage::Npcs);
    assert_valid_against_stage("classes.json", Stage::Classes);
    assert_valid_against_stage("quest-plan.json", Stage::QuestPlan);
    assert_valid_against_stage("quests.json", Stage::Quests);
    assert_valid_against_stage("dialogue.json", Stage::Dialogue);
}

#[test]
fn schemas_reject_schema_level_invalid_fixtures() {
    let fixtures = common::load_invalid();
    let mut checked = 0;
    for (name, fixture) in &fixtures {
        if !fixture.schema_reject {
            continue;
        }
        checked += 1;
        let mut any_rejected = false;
        for (stage_name, doc) in &fixture.documents {
            let stage = common::stage_of(stage_name);
            let schema = stage_schema(stage);
            let validator = jsonschema::validator_for(&schema).expect("compile schema");
            if !validator.is_valid(doc) {
                any_rejected = true;
            }
        }
        assert!(
            any_rejected,
            "schema-level fixture {name} was not rejected by any stage schema"
        );
    }
    assert!(
        checked > 0,
        "no schema-level invalid fixtures were exercised"
    );
}

// ---------------------------------------------------------------------------
// The authoring documents say what a person has to know to write them
// ---------------------------------------------------------------------------
//
// A rule stated only in a doc comment on a private struct is a rule nobody
// authoring a campaign will ever read. These assert over the EXPORTED schema —
// the artifact `delvec schema` prints and the skill tells an author to consult —
// never over the source, because reading the source is exactly the thing an
// author cannot be asked to do.

/// The `$defs` entry for `name` in a stage's exported schema.
fn defs_of(stage: Stage, name: &str) -> serde_json::Value {
    let schema = stage_schema(stage);
    schema
        .get("$defs")
        .and_then(|d| d.get(name))
        .unwrap_or_else(|| panic!("the {} schema defines `{name}`", stage.name()))
        .clone()
}

/// **The one-cell separation rule is in the document a person reads.**
///
/// It is the most load-bearing convention in a site plan: get it wrong and
/// every pair of connected boxes is flush and `DW0828` refuses the lot. The
/// gap value is asserted against [`SHARED_FACE_GAP_CELLS`] — the same constant
/// `shared_face` compares against — rather than against the literal `1`, so the
/// rule an author reads and the rule the checks enforce cannot drift: change
/// the constant without rewriting the description and this reds.
#[test]
fn the_site_plan_schema_states_the_one_cell_separation_rule() {
    use delvewright_dsl::siteplan::SHARED_FACE_GAP_CELLS;

    let plan_box = defs_of(Stage::SitePlan, "PlanBox");
    // Collapse the wrapping before matching. A description is prose and wraps
    // wherever the source line ended, so a phrase search over the raw string
    // answers "absent" for a sentence that is plainly there — and absent reads
    // as a real finding. The subject of these assertions is what the sentence
    // SAYS, never where its line breaks fell.
    let whole = unwrap_prose(
        plan_box["description"]
            .as_str()
            .expect("PlanBox is described"),
    );
    let extent = unwrap_prose(
        plan_box["properties"]["extent"]["description"]
            .as_str()
            .expect("`extent` is described"),
    );

    // Stated where the author is, and stated with the number the checks use.
    let gap = format!("{SHARED_FACE_GAP_CELLS}");
    assert!(
        whole.contains(&format!("separated by exactly {} cell", spell(SHARED_FACE_GAP_CELLS)))
            || whole.contains(&format!("separated by exactly {gap} cell")),
        "the PlanBox description must state the separation rule with the enforced \
         number ({SHARED_FACE_GAP_CELLS}); it says:\n{whole}"
    );
    // The half an author gets wrong first: `extent` is interior, not the
    // building, so the wall is outside it.
    for needle in ["play space", "DW0828"] {
        assert!(
            extent.contains(needle),
            "the `extent` description must say `{needle}` so a person reading only \
             that field knows what they are declaring; it says:\n{extent}"
        );
    }
    // And a worked coordinate, because the rule is an off-by-one and prose
    // about off-by-ones is how off-by-ones survive being read.
    assert!(
        whole.contains("x 4..7"),
        "the PlanBox description must work one example through, so the reader can \
         check their arithmetic against it; it says:\n{whole}"
    );
}

/// Every run of whitespace collapsed to one space, so a phrase that wrapped
/// across a line break in the source is still one phrase here.
fn unwrap_prose(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The English for a small count, so the description can read as prose while
/// still being asserted against the constant.
fn spell(n: i64) -> &'static str {
    match n {
        1 => "one",
        2 => "two",
        3 => "three",
        _ => panic!(
            "the separation rule is worded for a small gap; {n} needs the description \
             reworded and this test extended"
        ),
    }
}

/// **Every document answers to its own name**, which is the name `DW0100`
/// prints when it will not parse. Binding is computed from `Stage::ALL`, so a
/// stage added later is covered the day it exists rather than when somebody
/// remembers to extend a list here.
#[test]
fn every_stage_document_is_exportable_by_its_own_name() {
    let mut exported = 0;
    for stage in Stage::ALL {
        let schema = stage_schema(stage);
        assert!(
            schema.get("properties").is_some(),
            "the `{}` schema is an object schema",
            stage.name()
        );
        // The name is a real, non-empty, lower-kebab token a person can type.
        let name = stage.name();
        assert!(
            !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c == '-'),
            "`{name}` must be typeable as a `--stage` argument"
        );
        exported += 1;
    }
    assert_eq!(
        exported,
        Stage::ALL.len(),
        "binding: {exported} of {} stage document(s) exported by name",
        Stage::ALL.len()
    );
    assert!(exported > 0, "a zero binding here would be a vacuous pass");
}
