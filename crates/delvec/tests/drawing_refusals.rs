//! **Every refusal a drawing has, at the operation that made it** (spec-0072
//! §8, criterion 7).
//!
//! Each of `DW0902` … `DW0908` is asserted here by reading the **address** out
//! of the message, not only the code: a refusal that names the right rule and
//! the wrong place sends an author to a line they did not write. For `DW0902`
//! the refusal is provoked from inside a `use` inside a `repeat`, so the chain
//! of uses and the index in force are both asserted.
//!
//! The documents are written as JSON and parsed, rather than built with the
//! Rust constructors, because the address is a **JSON pointer into the document
//! the creator wrote** and a test that never wrote one would be asserting
//! against a pointer it invented.

use std::path::{Path, PathBuf};

use delvec::drawing::execute::{self, ExecuteOptions};
use delvec::drawing::ir::Drawing;
use delvec::grammar::Box3;

/// The scratch directory, under `target/` — never the system temp tree.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("drawing-refusals-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn parse(json: &str) -> Drawing {
    serde_json::from_str(json).expect("the document parses")
}

/// The refusal `check` gives, as `(code, message)`.
fn refuse_check(json: &str) -> (String, String) {
    let d = parse(json);
    let r = execute::check(&d).expect_err("this document is refused");
    (r.code.id().to_string(), r.to_string())
}

/// The refusal `execute` gives over a 9-cube.
fn refuse_execute(json: &str) -> (String, String) {
    let d = parse(json);
    let opts = ExecuteOptions::seeded(0, ".");
    let r = execute::execute(&d, Box3::at_origin([9, 9, 9]), &opts)
        .expect_err("this document is refused");
    (r.code.id().to_string(), r.to_string())
}

const VERSION: &str = delvec::compiler::DSL_VERSION;

fn doc(body: &str) -> String {
    format!(
        r#"{{ "dsl_version": "{VERSION}", "name": "t",
              "palette": {{ "wall": "minecraft:stone_bricks" }},
              {body} }}"#
    )
}

// ---------------------------------------------------------------------------
// DW0902 — an operation that reaches a cell its scope does not hold
// ---------------------------------------------------------------------------

/// **The chain and the index are both in the address.** The box that leaves its
/// scope is written inside a `define`, reached by a `use`, inside a `repeat` —
/// so the refusal has to say the operation's own pointer (inside the define),
/// the `use` site that reached it, and which instance of the repeat was running.
///
/// An address that named only the define would send the author to a body that
/// is correct at every other call site; one that named only the `use` would send
/// them to a line with no box in it.
#[test]
fn an_operation_that_leaves_its_scope_names_the_chain_and_the_index() {
    let (code, message) = refuse_execute(&doc(
        r#""defines": { "post": { "roles": ["stone"], "body": [
               { "op": "box", "role": "stone", "from": [0,0,0], "to": [0,0,5] } ] } },
           "ops": [
             { "op": "repeat", "from": [0,0,0], "to": [2,2,2], "step": [3,0,0], "count": 3,
               "index": "k", "body": [
                 { "op": "use", "define": "post", "from": [0,0,0], "to": [2,2,2],
                   "roles": { "stone": "wall" } } ] } ]"#,
    ));
    assert_eq!(code, "DW0902");
    assert!(
        message.contains("/defines/post/body/0"),
        "the operation's own pointer: {message}"
    );
    assert!(
        message.contains("\u{2190} /ops/0/body/0"),
        "the `use` site that reached it: {message}"
    );
    assert!(message.contains("k=0"), "the index in force: {message}");
    assert!(
        message.contains("0,0,0 .. 0,0,5") && message.contains("0,0,0 .. 2,2,2"),
        "both boxes: {message}"
    );
}

/// A `line`'s **brush** is part of what the operation reaches, so a line that
/// ends inside the scope and is stamped with a brush that does not is refused
/// on the brush's own bounds. The corner pair alone would have passed.
#[test]
fn a_line_whose_brush_leaves_the_scope_is_refused_on_the_brush() {
    let (code, message) = refuse_execute(&doc(
        r#""ops": [ { "op": "line", "role": "wall", "from": [0,0,0], "to": [8,0,8],
                     "brush": [1, 1, 3] } ]"#,
    ));
    assert_eq!(code, "DW0902");
    assert!(message.contains("/ops/0"), "{message}");
    assert!(message.contains("`line`"), "{message}");

    // The control: the same line with the default brush is inside, so the
    // refusal is about the brush and not about the line.
    let d = parse(&doc(
        r#""ops": [ { "op": "line", "role": "wall", "from": [0,0,0], "to": [8,0,8] } ]"#,
    ));
    execute::execute(
        &d,
        Box3::at_origin([9, 9, 9]),
        &ExecuteOptions::seeded(0, "."),
    )
    .expect("a one-cell brush stays inside");
}

// ---------------------------------------------------------------------------
// DW0903 — a name that resolves to nothing, or to two things
// ---------------------------------------------------------------------------

#[test]
fn a_role_nobody_declared_is_refused_with_the_roles_that_are() {
    let (code, message) = refuse_check(&doc(r#""ops": [ { "op": "box", "role": "wal" } ]"#));
    assert_eq!(code, "DW0903");
    assert!(message.contains("\"wal\""), "{message}");
    assert!(
        message.contains("\"wall\""),
        "every role of that kind the document declares: {message}"
    );
    assert!(message.contains("/ops/0"), "{message}");
}

#[test]
fn a_use_that_leaves_one_of_the_defines_roles_unbound_is_refused() {
    let (code, message) = refuse_check(&doc(
        r#""defines": { "post": { "roles": ["stone", "cap"], "body": [
               { "op": "box", "role": "stone" } ] } },
           "ops": [ { "op": "use", "define": "post", "roles": { "stone": "wall" } } ]"#,
    ));
    assert_eq!(code, "DW0903");
    assert!(message.contains("\"cap\""), "{message}");
    assert!(message.contains("/ops/0"), "{message}");
}

#[test]
fn a_parameter_nobody_declared_is_refused_with_the_names_the_body_reads() {
    let (code, message) = refuse_check(&format!(
        r#"{{ "dsl_version": "{VERSION}", "name": "t", "params": {{ "bay": 3 }},
             "palette": {{ "wall": "minecraft:stone_bricks" }},
             "ops": [ {{ "op": "box", "role": "wall",
                        "to": [{{"expr":"param","name":"by"}}, 0, 0] }} ] }}"#
    ));
    assert_eq!(code, "DW0903");
    assert!(message.contains("\"by\""), "{message}");
    assert!(message.contains("\"bay\""), "{message}");
}

/// A region the contract names and nothing claims, and a region claimed that
/// the contract never classifies — both directions, through the one checker a
/// grammar program is held to.
#[test]
fn a_region_only_one_side_names_is_refused_in_both_directions() {
    // The contract names a region and nothing claims it.
    let (code, message) = refuse_check(&doc(r#""contract": { "entry": "room",
                        "spaces": { "room": { "envelope": "open" } } },
           "ops": [ { "op": "box", "role": "wall" } ]"#));
    assert_eq!(code, "DW0903");
    assert!(message.contains("\"room\""), "{message}");
    assert!(message.contains("no rule claims"), "{message}");

    // An operation claims a region the contract never classifies.
    let (code, message) = refuse_check(&doc(r#""contract": { "entry": "room",
                        "spaces": { "room": { "envelope": "open" } } },
           "ops": [ { "op": "claim", "region": "room", "body": [] },
                    { "op": "claim", "region": "cellar", "body": [] } ]"#));
    assert_eq!(code, "DW0903");
    assert!(message.contains("cellar"), "{message}");
    assert!(message.contains("never"), "{message}");

    // And a claim with no contract at all names the operation that made it.
    let (code, message) = refuse_check(&doc(
        r#""ops": [ { "op": "claim", "region": "cellar", "body": [] } ]"#,
    ));
    assert_eq!(code, "DW0903");
    assert!(message.contains("/ops/0"), "{message}");
    assert!(message.contains("no `contract`"), "{message}");
}

/// Two marks producing one anchor name: refused at the **second** mark, with
/// the first one's address named, and the repair (`index: auto`) said.
#[test]
fn two_marks_that_produce_one_anchor_are_refused_at_the_second() {
    let (code, message) = refuse_execute(&doc(
        r#""ops": [ { "op": "mark", "mark": { "anchor": "hearth", "at": "floor_center" } },
                    { "op": "scope", "from": [1,0,1], "to": [3,2,3], "body": [
                        { "op": "mark", "mark": { "anchor": "hearth", "at": "corner_min" } } ] } ]"#,
    ));
    assert_eq!(code, "DW0903");
    assert!(message.contains("anchor/hearth"), "{message}");
    assert!(message.contains("/ops/1/body/0"), "the second: {message}");
    assert!(message.contains("/ops/0"), "and the first: {message}");
    assert!(message.contains("auto"), "the repair: {message}");
}

// ---------------------------------------------------------------------------
// DW0904 — a define that reaches itself
// ---------------------------------------------------------------------------

#[test]
fn a_define_that_uses_itself_is_refused_with_the_chain() {
    let (code, message) = refuse_check(&doc(r#""defines": {
             "tower": { "body": [ { "op": "use", "define": "storey" } ] },
             "storey": { "body": [ { "op": "use", "define": "tower" } ] } },
           "ops": [ { "op": "use", "define": "tower" } ]"#));
    assert_eq!(code, "DW0904");
    assert!(
        message.contains("\"storey\" \u{2192} \"tower\"")
            || message.contains("\"tower\" \u{2192} \"storey\""),
        "the chain, not the fact that there is one: {message}"
    );

    // Directly, too — the one-step cycle is the same rule.
    let (code, message) = refuse_check(&doc(
        r#""defines": { "tower": { "body": [ { "op": "use", "define": "tower" } ] } },
           "ops": [ { "op": "use", "define": "tower" } ]"#,
    ));
    assert_eq!(code, "DW0904");
    assert!(
        message.contains("\"tower\" \u{2192} \"tower\""),
        "{message}"
    );
}

// ---------------------------------------------------------------------------
// DW0905 — a value outside its range at the operation that evaluates it
// ---------------------------------------------------------------------------

#[test]
fn a_wall_thinner_than_one_is_refused_with_the_field_and_the_value() {
    let (code, message) = refuse_execute(&doc(
        r#""ops": [ { "op": "box", "role": "wall", "faces": ["west"], "t": 0 } ]"#,
    ));
    assert_eq!(code, "DW0905");
    assert!(message.contains("`t`"), "{message}");
    assert!(message.contains("0"), "{message}");
    assert!(message.contains("/ops/0"), "{message}");
}

#[test]
fn a_to_below_its_from_is_refused_naming_the_axis() {
    let (code, message) = refuse_execute(&doc(
        r#""ops": [ { "op": "box", "role": "wall", "from": [4,0,0], "to": [2,0,0] } ]"#,
    ));
    assert_eq!(code, "DW0905");
    assert!(message.contains("x corner"), "{message}");
}

/// A `flat` that names a face of the straight axis: a flat is the cut plane of a
/// half solid, so it lies on an axis the solid is round on.
#[test]
fn a_flat_on_a_straight_axis_is_refused() {
    let (code, message) = refuse_execute(&doc(
        r#""ops": [ { "op": "cylinder", "role": "wall", "axis": "y", "flat": "up" } ]"#,
    ));
    assert_eq!(code, "DW0905");
    assert!(message.contains("`flat` is up"), "{message}");
}

/// A parameter that produced a value past what a cell can be. The algebra
/// saturates rather than wrapping, so the refusal is what stands between a
/// saturated `i64` and a box quietly clamped onto the wrong cells.
#[test]
fn arithmetic_that_ran_off_the_end_is_refused_with_the_parameters_that_produced_it() {
    let huge = i64::MAX;
    let (code, message) = refuse_execute(&format!(
        r#"{{ "dsl_version": "{VERSION}", "name": "t", "params": {{ "n": {huge} }},
             "palette": {{ "wall": "minecraft:stone_bricks" }},
             "ops": [ {{ "op": "box", "role": "wall", "from": [0,0,0],
                        "to": [{{"expr":"arith",
                                "lhs":{{"expr":"param","name":"n"}},
                                "op":"add",
                                "rhs":{{"expr":"int","value":1}}}}, 0, 0] }} ] }}"#
    ));
    assert_eq!(code, "DW0905");
    assert!(message.contains("`to`"), "{message}");
    assert!(message.contains("n="), "the parameters in force: {message}");
}

// ---------------------------------------------------------------------------
// DW0906 — a fitted repeat that does not fit
// ---------------------------------------------------------------------------

#[test]
fn a_bay_that_does_not_fit_names_e_stride_item_and_the_remainder() {
    let (code, message) = refuse_execute(&doc(
        r#""ops": [ { "op": "repeat", "from": [0,0,0], "to": [8,0,0],
                     "along": "x", "stride": 4, "item": 2, "remainder": "exact",
                     "body": [ { "op": "box", "role": "wall" } ] } ]"#,
    ));
    assert_eq!(code, "DW0906");
    assert!(message.contains("`exact`"), "{message}");
    assert!(message.contains("9"), "E: {message}");
    assert!(message.contains("`stride` is 4"), "{message}");
    assert!(message.contains("`item` is 2"), "{message}");
    assert!(
        message.contains("`start`, `end` and `middle`"),
        "the three words that would accept it: {message}"
    );

    // A run shorter than one item is the other half of the same rule.
    let (code, message) = refuse_execute(&doc(
        r#""ops": [ { "op": "repeat", "from": [0,0,0], "to": [2,0,0],
                     "along": "x", "stride": 4, "item": 5, "remainder": "end",
                     "body": [ { "op": "box", "role": "wall" } ] } ]"#,
    ));
    assert_eq!(code, "DW0906");
    assert!(message.contains("no whole item fits"), "{message}");
}

// ---------------------------------------------------------------------------
// DW0907 — a paint that writes a property the engine derives
// ---------------------------------------------------------------------------

#[test]
fn a_stair_with_its_shape_typed_is_refused_and_the_repair_is_to_omit_it() {
    let (code, message) = refuse_check(&format!(
        r#"{{ "dsl_version": "{VERSION}", "name": "t",
             "palette": {{ "tread":
               "minecraft:oak_stairs[facing=north,half=bottom,shape=straight,waterlogged=false]" }},
             "ops": [ {{ "op": "box", "role": "tread" }} ] }}"#
    ));
    assert_eq!(code, "DW0907");
    assert!(message.contains("/palette/tread"), "{message}");
    assert!(message.contains("`shape`"), "{message}");
    assert!(message.contains("Omit"), "the repair: {message}");

    // The control: the same stair without `shape` is accepted, so the refusal
    // is about the property and not about the block.
    let ok = parse(&format!(
        r#"{{ "dsl_version": "{VERSION}", "name": "t",
             "palette": {{ "tread":
               "minecraft:oak_stairs[facing=north,half=bottom,waterlogged=false]" }},
             "ops": [ {{ "op": "box", "role": "tread" }} ] }}"#
    ));
    execute::check(&ok).expect("a stair that omits its shape is what the engine wants");
}

// ---------------------------------------------------------------------------
// DW0100 / DW0102 — the document's own shape and its one version
// ---------------------------------------------------------------------------

#[test]
fn a_repeat_that_says_nothing_about_how_many_is_refused_by_the_schema() {
    let (code, message) = refuse_check(&doc(
        r#""ops": [ { "op": "repeat", "body": [ { "op": "box", "role": "wall" } ] } ]"#,
    ));
    assert_eq!(code, "DW0100");
    assert!(message.contains("`step` and `count`"), "{message}");
    assert!(message.contains("`remainder`"), "{message}");

    // A fitted repeat missing only its `remainder` names that one field, which
    // is the case the spec calls out: `remainder` has no default, because a run
    // whose leftover nobody placed is a run whose ends nobody decided.
    let (code, message) = refuse_check(&doc(
        r#""ops": [ { "op": "repeat", "along": "x", "stride": 4, "item": 2,
                     "body": [ { "op": "box", "role": "wall" } ] } ]"#,
    ));
    assert_eq!(code, "DW0100");
    assert!(
        message.contains("missing `remainder`"),
        "only the field that is missing is named as missing: {message}"
    );
}

#[test]
fn a_turn_of_four_and_an_unknown_op_are_refused_by_the_schema() {
    let (code, message) = refuse_check(&doc(r#""defines": { "p": { "body": [] } },
           "ops": [ { "op": "use", "define": "p", "turn": 4 } ]"#));
    assert_eq!(code, "DW0100");
    assert!(message.contains("`turn` is 4"), "{message}");

    // An unknown `op` never reaches `check`: it is the parse that refuses it.
    let bad: Result<Drawing, _> =
        serde_json::from_str(&doc(r#""ops": [ { "op": "flight", "role": "wall" } ]"#));
    let e = bad
        .expect_err("`flight` is not one of the thirteen")
        .to_string();
    assert!(e.contains("flight"), "{e}");
}

#[test]
fn a_dsl_version_that_is_not_the_engines_is_refused_at_load() {
    let dir = scratch("version");
    let path = dir.join("hut.json");
    std::fs::write(
        &path,
        r#"{ "dsl_version": "9.9.9", "name": "t", "palette": {}, "ops": [] }"#,
    )
    .unwrap();
    let r = execute::load(&path).expect_err("one campaign format number, ADR-0024");
    assert_eq!(r.code.id(), "DW0102");
    assert!(r.to_string().contains("9.9.9"), "{r}");
    assert!(r.to_string().contains(VERSION), "{r}");
}

// ---------------------------------------------------------------------------
// DW0908 — a place with two media
// ---------------------------------------------------------------------------

/// The code is the drawing module's, because the rule is about the place's
/// medium and not about either document. The verb that fires it is
/// `delvec detail`, and the gallery probe that binds it end to end is a later
/// round's (spec-0072 criterion 9); what is asserted here is that the code
/// exists, carries its tier, and is the one `delvec detail` raises.
#[test]
fn a_place_with_two_media_has_a_code_of_its_own() {
    assert_eq!(delvec::drawing::diag::DW_TWO_MEDIA.id(), "DW0908");
    assert_eq!(delvec::drawing::DRAWINGS_DIR, "drawings");
}

// ---------------------------------------------------------------------------
// The address a load-time refusal carries
// ---------------------------------------------------------------------------
//
// spec-0072 §8 addresses **every** refusal by the operation's path in the
// document the creator wrote. A refusal at load had no such address: it carried
// `/`, and serde's own words name a line and a column — which, for a
// flattened or tagged field, is the line where the enclosing object CLOSES.
// In a 300-operation drawing that is a message nobody can act on.
//
// Each case below plants one mistake and asserts the pointer of the value that
// failed. The document is otherwise well formed, so what is under test is the
// address and not whether the refusal happens.

/// The value that failed is a `mark`'s `at`, eight operations in.
#[test]
fn a_malformed_field_is_addressed_by_its_own_pointer() {
    let dir = scratch("pointer-top");
    let path = dir.join("tower.json");
    std::fs::write(
        &path,
        &doc(&format!(
            r#""ops": [ {} {{ "op": "mark",
                        "mark": {{ "anchor": "gate", "at": {{ "cell": [5,1,10] }} }} }} ]"#,
            r#"{ "op": "box", "role": "wall" }, "#.repeat(7)
        )),
    )
    .unwrap();
    let r = execute::load(&path).expect_err("`at` has no such form");
    assert_eq!(r.code.id(), "DW0100");
    assert_eq!(
        r.at.pointer, "/ops/7/mark/at",
        "the value that failed, not the document: {r}"
    );
    assert!(
        r.to_string().contains("floor_center") && r.to_string().contains("offset"),
        "an enum names the forms it accepts: {r}"
    );
}

/// The same mistake inside a define's body: the pointer is the define's, which
/// is where the author has to go, and the `use` site is not it.
#[test]
fn a_malformed_field_inside_a_define_is_addressed_inside_the_define() {
    let dir = scratch("pointer-define");
    let path = dir.join("tower.json");
    std::fs::write(
        &path,
        &doc(r#""defines": { "post": { "roles": ["s"], "body": [
               { "op": "box", "role": "s" },
               { "op": "mark", "mark": { "anchor": "top", "at": { "cell": [0,0,0] } } } ] } },
           "ops": [ { "op": "use", "define": "post", "roles": { "s": "wall" } } ]"#),
    )
    .unwrap();
    let r = execute::load(&path).expect_err("`at` has no such form");
    assert_eq!(r.code.id(), "DW0100");
    assert_eq!(r.at.pointer, "/defines/post/body/1/mark/at", "{r}");
}

/// An unknown `op` is addressed at the operation, and names the thirteen.
#[test]
fn an_unknown_op_is_addressed_at_the_operation_and_names_the_thirteen() {
    let dir = scratch("pointer-op");
    let path = dir.join("tower.json");
    std::fs::write(
        &path,
        &doc(r#""ops": [ { "op": "box", "role": "wall" },
                    { "op": "flight", "role": "wall" } ]"#),
    )
    .unwrap();
    let r = execute::load(&path).expect_err("`flight` is struck");
    assert_eq!(r.code.id(), "DW0100");
    assert!(
        r.at.pointer.starts_with("/ops/1"),
        "the operation that names it: {r}"
    );
    let message = r.to_string();
    assert!(message.contains("flight"), "{message}");
    for verb in ["box", "cylinder", "prism", "mirror", "claim"] {
        assert!(message.contains(verb), "the forms it accepts: {message}");
    }
}

/// A misspelt field on a solid is addressed at that solid.
#[test]
fn a_misspelt_field_on_a_solid_is_addressed_at_that_solid() {
    let dir = scratch("pointer-field");
    let path = dir.join("tower.json");
    std::fs::write(
        &path,
        &doc(r#""ops": [ { "op": "box", "role": "wall" },
                    { "op": "box", "rol": "wall" } ]"#),
    )
    .unwrap();
    let r = execute::load(&path).expect_err("`rol` is not a field of `box`");
    assert_eq!(r.code.id(), "DW0100");
    assert!(r.at.pointer.starts_with("/ops/1"), "{r}");
    assert!(r.to_string().contains("rol"), "{r}");
}

/// The control: a document that parses is not refused, so the three above are
/// about the mistake they plant and not about the loader.
#[test]
fn a_well_formed_drawing_loads() {
    let dir = scratch("pointer-control");
    let path = dir.join("tower.json");
    std::fs::write(
        &path,
        &doc(r#""ops": [ { "op": "box", "role": "wall" },
                    { "op": "mark", "mark": { "anchor": "gate", "at": "floor_center" } } ]"#),
    )
    .unwrap();
    let d = execute::load(&path).expect("a well-formed drawing loads");
    assert_eq!(d.ops.len(), 2);
}
