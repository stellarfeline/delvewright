//! **The surface a creator writes** (spec-0072 criterion 1): the drawing's
//! schema, the thirteen operations it declares, and the fact that its shared
//! halves are the program document's own types rather than copies of them.
//!
//! The schema is read the way its consumer reads it — out of
//! `delvec schema --stage drawing` and `--stage all`, through the binary, not
//! out of a Rust call — because the coverage gate and the authoring agent both
//! read the export and neither can see a `schema_for!`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn delvec() -> Command {
    Command::new(env!("CARGO_BIN_EXE_delvec"))
}

fn schema(stage: &str) -> Value {
    let out = delvec()
        .args(["schema", "--stage", stage])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "`delvec schema --stage {stage}` exited {:?}: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("the export is JSON")
}

/// The `$defs` of a schema, or an empty map.
fn defs(schema: &Value) -> &serde_json::Map<String, Value> {
    static EMPTY: std::sync::LazyLock<serde_json::Map<String, Value>> =
        std::sync::LazyLock::new(serde_json::Map::new);
    schema
        .get("$defs")
        .and_then(Value::as_object)
        .unwrap_or(&EMPTY)
}

/// The verbs the operation union declares, read out of the `oneOf`'s own `op`
/// constants.
fn verbs(schema: &Value) -> BTreeSet<String> {
    let op = defs(schema).get("Op").expect("the union is exported");
    op.get("oneOf")
        .and_then(Value::as_array)
        .expect("the union is a oneOf")
        .iter()
        .map(|branch| {
            let props = branch
                .get("properties")
                .and_then(Value::as_object)
                .expect("each branch is an object with properties");
            props
                .get("op")
                .and_then(|o| o.get("const"))
                .and_then(Value::as_str)
                .expect("each branch pins its own `op`")
                .to_string()
        })
        .collect()
}

/// **Thirteen operations, and the five ADR-0030 named that are struck.**
///
/// A verb struck because a *field* says it for every solid — `clear`,
/// `replace`, `disc`, `dome`, `flight` — must not be reachable, or the surface
/// offers two spellings of one thing and a reviewer has to know which one an
/// author meant.
#[test]
fn the_operation_union_declares_thirteen_verbs_and_none_of_the_struck_five() {
    let s = schema("drawing");
    let got = verbs(&s);
    let want: BTreeSet<String> = [
        "box", "cylinder", "sphere", "prism", "pyramid", "line", "scope", "use", "repeat",
        "mirror", "grammar", "mark", "claim",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    assert_eq!(got, want, "the thirteen, exactly");
    assert_eq!(got.len(), 13);
    for struck in ["clear", "replace", "disc", "dome", "flight"] {
        assert!(
            !got.contains(struck),
            "`{struck}` is struck (spec-0072 §3.1): a field on the remaining operations says it \
             for every solid"
        );
    }
}

/// `--stage all` is the enumeration of **what a creator can write**, and a
/// drawing is one of those. It is not a `Stage` — a stage is one file named
/// `<stage>.json` and a campaign holds one drawing per place — so it answers to
/// its own name here and carries the same schema.
#[test]
fn the_drawing_is_part_of_stage_all_and_is_the_same_document_there() {
    let all = schema("all");
    let carried = all
        .get("drawing")
        .expect("`--stage all` carries the drawing");
    assert_eq!(carried, &schema("drawing"), "the same document, once");

    // `prefab-metadata` stays out of `all`, and the reason is the contrast:
    // no creator writes it.
    assert!(
        all.get("prefab-metadata").is_none(),
        "a library asset's metadata is not a document a creator writes"
    );
}

/// **The shared halves are the program document's own types**, byte for byte.
///
/// The drawing's schema must carry the *same* `Expr`, `Cond`, `Mark` and
/// `Contract` a grammar program's types generate. Comparing the definitions
/// rather than their names is what makes this a check: two structurally
/// different types called `Expr` would pass a name comparison and would be two
/// algebras.
#[test]
fn the_shared_definitions_are_generated_from_the_program_documents_own_types() {
    let s = schema("drawing");
    let d = defs(&s);
    let mut bound = 0usize;
    for (name, own) in [
        (
            "Expr",
            serde_json::to_value(schemars::schema_for!(delvec::grammar::ir::Expr)).unwrap(),
        ),
        (
            "Cond",
            serde_json::to_value(schemars::schema_for!(delvec::grammar::ir::Cond)).unwrap(),
        ),
        // The exported name says which document's mark it is: a schema
        // export's `$defs` is one flat namespace over every document a creator
        // writes, and the campaign DSL declares a `Mark` of its own.
        (
            "AnchorMark",
            serde_json::to_value(schemars::schema_for!(delvec::grammar::Mark)).unwrap(),
        ),
        (
            "Contract",
            serde_json::to_value(schemars::schema_for!(delvec::grammar::Contract)).unwrap(),
        ),
        (
            "States",
            serde_json::to_value(schemars::schema_for!(delvec::grammar::ir::States)).unwrap(),
        ),
    ] {
        let carried = d
            .get(name)
            .unwrap_or_else(|| panic!("the drawing's schema carries `{name}`"));
        // A standalone `schema_for!` is the type's own definition plus a
        // `$schema`/`title` header and its own `$defs`; the carried one is the
        // body alone. Compare the body — and normalise the ONE difference that
        // is about where the schema is rooted rather than about the type: a
        // recursive type refers to itself as `#` when it is the root document
        // and as `#/$defs/<name>` when it is one definition among many.
        let mut standalone = own.clone();
        let obj = standalone.as_object_mut().unwrap();
        obj.remove("$schema");
        obj.remove("$defs");
        obj.remove("title");
        let standalone = rooted_at(standalone, name);
        let mut here = carried.clone();
        if let Some(o) = here.as_object_mut() {
            o.remove("title");
        }
        assert_eq!(
            here, standalone,
            "`{name}` in the drawing's schema is not the program document's own type"
        );
        bound += 1;
    }
    assert_eq!(bound, 5, "binding count: the shared definitions compared");
}

/// A standalone schema's self-reference (`#`) rewritten as the one it has when
/// it stands among other definitions (`#/$defs/<name>`).
///
/// The only difference between the two forms, and it is about where the
/// document is rooted rather than about the type: normalising it is what lets
/// the comparison be an equality instead of a resemblance.
fn rooted_at(value: Value, name: &str) -> Value {
    match value {
        Value::Array(a) => Value::Array(a.into_iter().map(|v| rooted_at(v, name)).collect()),
        Value::Object(o) => Value::Object(
            o.into_iter()
                .map(|(k, v)| {
                    // Only a `$ref`, and only the whole-document one: a
                    // description that happens to contain a `#` is prose.
                    if k == "$ref" && v.as_str() == Some("#") {
                        (k, Value::String(format!("#/$defs/{name}")))
                    } else {
                        (k, rooted_at(v, name))
                    }
                })
                .collect(),
        ),
        other => other,
    }
}

/// The drawing module's own source, so the check is about what the tree
/// contains and not about what one schema happened to export.
fn drawing_sources() -> Vec<(PathBuf, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/drawing");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("the drawing module exists") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            let text = std::fs::read_to_string(&path).unwrap();
            out.push((path, text));
        }
    }
    assert!(!out.is_empty(), "binding count: the module has sources");
    out
}

/// **The drawing module defines no type of a shared name.**
///
/// `Expr`, `Cond`, `Paint`, `Mark`, `Contract` and `States` belong to the
/// program document. A copy here would be a second expression algebra, a second
/// guard algebra, a second anchor or a second contract — each of which would
/// have its own checker, and the two would disagree only in a built world.
///
/// The source is read rather than the schema, because a private copy that
/// happened to serialise identically today is still a second definition
/// tomorrow.
#[test]
fn the_drawing_module_defines_no_type_of_a_shared_name() {
    let mut examined = 0usize;
    for (path, text) in drawing_sources() {
        examined += 1;
        for name in ["Expr", "Cond", "Paint", "Mark", "Contract", "States"] {
            for kind in ["struct", "enum", "type"] {
                let needle = format!("{kind} {name} ");
                let alt = format!("{kind} {name}<");
                let braced = format!("{kind} {name} {{");
                assert!(
                    !text.contains(&needle) && !text.contains(&alt) && !text.contains(&braced),
                    "{}: defines its own `{name}`. The program document owns it; a drawing uses \
                     it by reference (spec-0072 §2.1).",
                    path.display()
                );
            }
        }
    }
    assert!(examined >= 5, "binding count: {examined} source file(s)");
}

/// The five ADR-0030 verbs that are struck are struck **in the tree**, not only
/// in the export: no `Op` variant answers to any of their names.
#[test]
fn no_struck_verb_has_an_operation_type() {
    for (path, text) in drawing_sources() {
        for struck in ["ClearOp", "ReplaceOp", "DiscOp", "DomeOp", "FlightOp"] {
            assert!(
                !text.contains(struck),
                "{}: `{struck}` exists, and the verb it is for is struck",
                path.display()
            );
        }
    }
}
