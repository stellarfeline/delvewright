//! **Reading a drawing, with the pointer of the value that failed.**
//!
//! spec-0072 §8 addresses every refusal by the operation's path in the document
//! the creator wrote. A refusal at *load* is the one an author meets first, and
//! it is the one that had no address: `serde_json`'s error names a line and a
//! column, and for a flattened or tagged field that is where the **enclosing
//! object closes**. Walked by hand, a mis-formed `mark.at` in the eighth
//! operation of a small tower reported line 30 — the closing brace of
//! `defines` — and said nothing about which operation or which field. In a
//! 300-operation drawing that is unrepairable.
//!
//! Two mechanisms, and neither is per-field:
//!
//! * **the pointer** comes from deserialising through a path-tracking layer
//!   (`serde_path_to_error`), so every field of every operation is covered by
//!   construction. A hand-written check per field would be exactly the thing
//!   that misses the field nobody thought of;
//! * **the forms a value accepts** come from the document's own **exported
//!   schema**, resolved at that pointer, with the document itself used to pick
//!   the branch of a tagged union. The schema is generated from the same Rust
//!   types the parse just refused, so the two cannot disagree about what is
//!   accepted, and a form added later is named the day it exists.

use std::path::Path;

use delvewright_dsl::codes;
use serde_json::Value;

use crate::drawing::diag::Refusal;
use crate::drawing::ir::Drawing;

/// Read a drawing from disk: the schema, then the one `dsl_version`.
///
/// Both refusals are the campaign format's own (`DW0100`, `DW0102`, ADR-0024),
/// because a drawing is a campaign document and an author who has met them in a
/// stage document has met them here.
pub fn load(path: &Path) -> Result<Drawing, Refusal> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        Refusal::at_field(
            codes::SCHEMA,
            "/",
            format!("cannot read {}: {e}", path.display()),
        )
    })?;
    from_str(&text, &path.display().to_string())
}

/// The same, over text a caller already holds. `source` is what the message
/// calls the document.
pub fn from_str(text: &str, source: &str) -> Result<Drawing, Refusal> {
    let mut de = serde_json::Deserializer::from_str(text);
    let drawing: Drawing = match serde_path_to_error::deserialize(&mut de) {
        Ok(d) => d,
        Err(e) => {
            let message = e.inner().to_string();
            // The document as a tree: for extending the pointer below the line
            // `serde`'s buffering hides, and for choosing the branch of a tagged
            // union the author actually wrote. A document that will not parse as
            // JSON at all has no value to point at, and says so.
            let tree: Option<Value> = serde_json::from_str(text).ok();
            let schema = crate::drawing::schema();
            let mut pointer = pointer_of(e.path());
            if let Some(t) = &tree {
                pointer = deepen(&schema, t, &pointer);
            }
            let forms = tree
                .as_ref()
                .map(|t| accepted_forms(&schema, t, &pointer))
                .unwrap_or_default();
            let names_them = forms.iter().all(|f| message.contains(f.as_str()));
            let said = if forms.is_empty() || names_them {
                String::new()
            } else {
                format!(
                    " The forms this value accepts are {}.",
                    forms
                        .iter()
                        .map(|f| format!("`{f}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            return Err(Refusal::at_field(
                codes::SCHEMA,
                if pointer.is_empty() { "/" } else { &pointer },
                format!("{source} is not a drawing: {message}.{said}"),
            ));
        }
    };
    let engine = crate::compiler::DSL_VERSION;
    if drawing.dsl_version != engine {
        return Err(Refusal::at_field(
            codes::DSL_VERSION,
            "/dsl_version",
            format!(
                "{source} declares `dsl_version` {:?}; this engine reads {engine:?}. There is one \
                 campaign format number and every document of a campaign declares it (ADR-0024).",
                drawing.dsl_version
            ),
        ));
    }
    Ok(drawing)
}

/// A tracked path as an RFC 6901 JSON pointer.
///
/// A `Segment::Enum` names a **variant**, not a member of the document, so it is
/// not part of the pointer: an author following `/ops/1/Box/role` would look for
/// a key their document does not have. An `Unknown` segment is a hole in the
/// tracking and is written as `?` rather than dropped, so a pointer that cannot
/// be trusted says so instead of reading as one that can.
fn pointer_of(path: &serde_path_to_error::Path) -> String {
    use serde_path_to_error::Segment;
    let mut out = String::new();
    for segment in path.iter() {
        match segment {
            Segment::Seq { index } => {
                out.push('/');
                out.push_str(&index.to_string());
            }
            Segment::Map { key } => {
                out.push('/');
                out.push_str(&key.replace('~', "~0").replace('/', "~1"));
            }
            Segment::Enum { .. } => {}
            _ => out.push_str("/?"),
        }
    }
    out
}

/// **The deepest value the schema refuses**, starting from where the tracker
/// stopped.
///
/// The tracker stops at the operation, and it cannot do better: `serde` buffers
/// the content of an internally-tagged enum (`{"op": …}`) and of a flattened
/// field (`Mark`'s `at`) before it reads either, so the layer that records the
/// path is no longer the layer doing the work. Below that line the **schema** is
/// the authority for what the document may hold, and it is walked here: every
/// member of every object, every element of every array, against the branch the
/// document itself selects.
///
/// It walks members rather than naming fields, so a field added later is covered
/// the day it exists. It extends the pointer only where the schema **refuses**
/// the value, so a walk that finds nothing leaves the tracker's own answer
/// standing rather than guessing a deeper one.
fn deepen(schema: &Value, document: &Value, pointer: &str) -> String {
    let mut node = resolve(schema, schema);
    let mut here = document.clone();
    for raw in pointer.split('/').skip(1).filter(|s| !s.is_empty()) {
        let key = unescape(raw);
        let Some(next) = step(schema, &node, &here, &key) else {
            return pointer.to_string();
        };
        node = next;
        here = member(&here, &key);
    }
    match refused_member(schema, &node, &here) {
        Some(deeper) => format!("{pointer}{deeper}"),
        None => pointer.to_string(),
    }
}

/// The deepest member of `value` the schema refuses, as a pointer relative to
/// it.
fn refused_member(root: &Value, node: &Value, value: &Value) -> Option<String> {
    let keys: Vec<String> = match value {
        Value::Object(o) => o.keys().cloned().collect(),
        Value::Array(a) => (0..a.len()).map(|i| i.to_string()).collect(),
        _ => return None,
    };
    for key in keys {
        // A key the schema does not have is an unknown field, and `serde` has
        // already named it in its own words; there is nothing deeper to say.
        let Some(sub) = step(root, node, value, &key) else {
            continue;
        };
        let v = member(value, &key);
        if !admits(root, &sub, &v) {
            return Some(format!("/{}", escape(&key)));
        }
        if let Some(deeper) = refused_member(root, &sub, &v) {
            return Some(format!("/{}{deeper}", escape(&key)));
        }
    }
    None
}

/// Whether the schema admits this value.
///
/// Two questions only, and both are ones a wrong answer cannot be given to: a
/// **closed set of values** must hold the value, and a **single declared type**
/// must be the value's own. Everything else — a union of shapes, a number's
/// range, a pattern — is left alone, because a walk that guessed would send an
/// author to a field that is right.
fn admits(root: &Value, node: &Value, value: &Value) -> bool {
    let node = resolve(root, node);
    let closed = closed_values(root, &node);
    if !closed.is_empty() {
        return value
            .as_str()
            .is_some_and(|s| closed.iter().any(|c| c == s));
    }
    match node.get("type").and_then(Value::as_str) {
        Some("object") => value.is_object(),
        Some("array") => value.is_array(),
        Some("string") => value.is_string(),
        Some("boolean") => value.is_boolean(),
        Some("integer") | Some("number") => value.is_number(),
        Some("null") => value.is_null(),
        _ => true,
    }
}

/// **The closed set of values the schema accepts at `pointer`**, or nothing.
///
/// The document is walked beside the schema so that a tagged union resolves to
/// the branch the author actually wrote: `/ops/7/mark/at` is `Mark`'s `at` only
/// once `ops[7].op` has picked the `mark` branch out of thirteen.
fn accepted_forms(schema: &Value, document: &Value, pointer: &str) -> Vec<String> {
    let mut node = resolve(schema, schema);
    let mut here = document.clone();
    for raw in pointer.split('/').skip(1).filter(|s| !s.is_empty()) {
        let key = unescape(raw);
        let Some(next) = step(schema, &node, &here, &key) else {
            return Vec::new();
        };
        node = next;
        here = member(&here, &key);
    }
    closed_values(schema, &node)
}

/// One member of an object, or one element of an array, or nothing.
fn member(value: &Value, key: &str) -> Value {
    if let Some(v) = value.get(key) {
        return v.clone();
    }
    value
        .as_array()
        .and_then(|a| key.parse::<usize>().ok().and_then(|i| a.get(i)))
        .cloned()
        .unwrap_or(Value::Null)
}

fn escape(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

fn unescape(key: &str) -> String {
    key.replace("~1", "/").replace("~0", "~")
}

/// Follow a `$ref` to the definition it names. Anything else is itself.
fn resolve(root: &Value, node: &Value) -> Value {
    let Some(r) = node.get("$ref").and_then(Value::as_str) else {
        return node.clone();
    };
    let Some(name) = r.strip_prefix("#/$defs/") else {
        return root.clone();
    };
    root.get("$defs")
        .and_then(|d| d.get(name))
        .cloned()
        .unwrap_or_else(|| node.clone())
}

/// Every branch a schema node offers, `$ref`s followed: itself, plus every
/// member of its `allOf` and `anyOf`.
///
/// **`oneOf` is deliberately not flattened here.** It is the one construct the
/// document itself settles — a tagged union, where exactly one branch is the
/// operation the author wrote — and flattening it would let a key from any of
/// the other twelve answer for the one they did write. [`step`] reads it with
/// the document in hand instead. `allOf` is a conjunction (a flattened field),
/// and `anyOf` is what schemars writes for an optional, whose null arm carries
/// no properties; both are safe to merge.
fn branches(root: &Value, node: &Value) -> Vec<Value> {
    let mut out = vec![resolve(root, node)];
    for key in ["allOf", "anyOf"] {
        if let Some(list) = node.get(key).and_then(Value::as_array) {
            for b in list {
                out.push(resolve(root, b));
            }
        }
    }
    out
}

/// The schema one step in: `key` of an object, or the items of an array.
///
/// A union is narrowed by the **document**: where a branch pins a property to a
/// `const` and the document gives that property that value, that branch is the
/// one the author wrote. Where **no** branch matches — which is the case being
/// diagnosed — every branch is kept, so the forms named are all of them and not
/// whichever happened to be written first.
fn step(root: &Value, node: &Value, document: &Value, key: &str) -> Option<Value> {
    if key.parse::<usize>().is_ok() && document.is_array() {
        for b in branches(root, node) {
            if let Some(items) = b.get("items") {
                return Some(resolve(root, items));
            }
        }
        return None;
    }
    let mut matched: Vec<Value> = Vec::new();
    let mut every: Vec<Value> = Vec::new();
    // Whether ANY branch of a union answered to the document's own tag. It is
    // not the same question as whether a branch carries this key: a `box` that
    // is asked for `mark` has matched its branch and simply has no such member,
    // and falling back to the other twelve there would name a form from an
    // operation the author did not write.
    let mut tag_settled = false;
    for b in branches(root, node) {
        if let Some(list) = b.get("oneOf").and_then(Value::as_array) {
            for one in list {
                let one = resolve(root, one);
                let matches = tag_matches(&one, document);
                tag_settled |= matches;
                let Some(p) = one.get("properties").and_then(|p| p.get(key)) else {
                    continue;
                };
                let p = resolve(root, p);
                if matches {
                    matched.push(p.clone());
                }
                every.push(p);
            }
        }
        if let Some(p) = b.get("properties").and_then(|p| p.get(key)) {
            let p = resolve(root, p);
            matched.push(p.clone());
            every.push(p);
            continue;
        }
        // A map keyed by a name the author chose — `defines`, `params`,
        // `palette`. The schema says what a VALUE of it is and nothing about
        // the keys, so every key steps to the same place.
        if let Some(p) = b.get("additionalProperties").filter(|p| p.is_object()) {
            let p = resolve(root, p);
            matched.push(p.clone());
            every.push(p);
        }
    }
    let chosen = if tag_settled || !matched.is_empty() {
        matched
    } else {
        every
    };
    match chosen.len() {
        0 => None,
        1 => chosen.into_iter().next(),
        // Several branches admit this key and the document settles none of
        // them: the value has to answer one of them, which is exactly `anyOf`.
        _ => Some(serde_json::json!({ "anyOf": chosen })),
    }
}

/// True when every `const` this branch pins is the value the document gives.
///
/// A branch that pins nothing matches everything, which is what an untagged
/// member is.
fn tag_matches(branch: &Value, document: &Value) -> bool {
    let Some(props) = branch.get("properties").and_then(Value::as_object) else {
        return true;
    };
    for (name, spec) in props {
        let Some(want) = spec.get("const") else {
            continue;
        };
        match document.get(name) {
            Some(got) if got == want => {}
            Some(_) => return false,
            // The document does not say, so this branch is not ruled out.
            None => {}
        }
    }
    true
}

/// The values a node accepts, when it accepts a closed set of them.
///
/// Three shapes, because schemars writes three: a bare `const`, an `enum` list,
/// and a union whose branches each pin one — the last is the interesting one,
/// because a tagged union's tag property is the value an author writes and its
/// name is the last segment of the pointer.
fn closed_values(root: &Value, node: &Value) -> Vec<String> {
    let node = resolve(root, node);
    let mut out: Vec<String> = Vec::new();
    if let Some(c) = node.get("const").and_then(Value::as_str) {
        out.push(c.to_string());
    }
    if let Some(list) = node.get("enum").and_then(Value::as_array) {
        out.extend(list.iter().filter_map(Value::as_str).map(str::to_string));
    }
    for key in ["oneOf", "anyOf"] {
        if let Some(list) = node.get(key).and_then(Value::as_array) {
            for b in list {
                let b = resolve(root, b);
                if let Some(c) = b.get("const").and_then(Value::as_str) {
                    out.push(c.to_string());
                }
                if let Some(inner) = b.get("enum").and_then(Value::as_array) {
                    out.extend(inner.iter().filter_map(Value::as_str).map(str::to_string));
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pointer is what the document's own keys spell, and a variant name is
    /// not one of them.
    #[test]
    fn a_pointer_escapes_what_rfc_6901_escapes() {
        // Built by hand rather than by a parse, because what is under test is
        // the rendering and not the tracking.
        assert_eq!(pointer_of_parts(&[]), "");
        assert_eq!(
            pointer_of_parts(&["ops", "7", "mark", "at"]),
            "/ops/7/mark/at"
        );
        assert_eq!(pointer_of_parts(&["a/b"]), "/a~1b");
        assert_eq!(pointer_of_parts(&["a~b"]), "/a~0b");
    }

    fn pointer_of_parts(parts: &[&str]) -> String {
        let mut out = String::new();
        for p in parts {
            out.push('/');
            out.push_str(&p.replace('~', "~0").replace('/', "~1"));
        }
        out
    }

    /// The accepted forms are read off the **exported schema**, at the pointer,
    /// with the document choosing the branch — so the four forms `at` takes are
    /// found under the `mark` operation and not under the twelve others.
    #[test]
    fn the_accepted_forms_come_from_the_schema_at_the_pointer() {
        let schema = crate::drawing::schema();
        let document: Value = serde_json::from_str(
            r#"{ "ops": [ { "op": "box", "role": "wall" },
                          { "op": "mark", "mark": { "anchor": "g", "at": 1 } } ] }"#,
        )
        .unwrap();
        let mut forms = accepted_forms(&schema, &document, "/ops/1/mark/at");
        forms.sort();
        assert_eq!(
            forms,
            ["corner_min", "face_center", "floor_center", "offset"],
            "the four forms `MarkAt` takes"
        );

        // The control: the same key under a different operation resolves to
        // nothing, so the walk is reading the branch and not the name.
        assert!(
            accepted_forms(&schema, &document, "/ops/0/mark/at").is_empty(),
            "a `box` has no `mark`"
        );
        // And a field with no closed set of values offers none.
        assert!(accepted_forms(&schema, &document, "/ops/0/role").is_empty());
    }
}
