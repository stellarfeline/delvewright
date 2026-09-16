//! **A generator deletes nothing it did not write.**
//!
//! # The defect this exists to close
//!
//! Every generator emits a whole prefab document with `serde_json` and writes it
//! over whatever was there. Everything a later step added is therefore deleted
//! by the next run: the anchors a campaign binds by name, an `"role": "entry"`
//! declaration, a `shown_faces` list, a spatial contract, a lighting verdict
//! measured at admission rather than estimated at generation. Nothing bound the
//! generators to preserve any of it, so the rule was transplanted by hand at
//! every library round and the deletion surfaced only as a campaign that stopped
//! resolving an anchor it names.
//!
//! Measured by regenerating the whole library into a scratch tree and diffing it
//! against the committed one: **88 leaf keys across 7 of 31 generator-written
//! documents** — 40 anchors, 4 entry roles, 4 `shown_faces` and 5 lighting
//! verdicts.
//!
//! # The rule, and why it is in the generators rather than in a gate alone
//!
//! A gate that only reds tells the next library round to hand-merge, which is
//! the transplant this replaces. So the safe path is the default: every
//! generator writes its documents through [`write_preserving`], which merges its
//! own output ONTO whatever is already on disk. A gate still exists
//! (`tools/check-generator-preservation.py`) and proves the property over every
//! generator, because a rule with no check is a sentence.
//!
//! # What "preserve" means, exactly
//!
//! Object by object, key by key:
//!
//! * A key the generator writes takes the generator's value. The generator is
//!   the authority on what it measures — a walk plane, a connector, a size.
//! * A key only the existing document has is KEPT, at whatever depth it sits.
//!   That is the whole rule: an anchor the generator never heard of survives, and
//!   so does a `role` inside an anchor the generator does write.
//! * Where both sides hold an object, the merge recurses. Where either side
//!   holds anything else — a scalar, an array — the generator's value stands
//!   whole. An array is a measurement of the bytes (a connector list, a
//!   `shown_faces` list) and merging two of them element by element would invent
//!   a third document neither side wrote.
//!
//! A document with nothing beside it on disk is written as-is, so a fresh output
//! directory is exactly what it always was.

use std::path::Path;

use serde_json::{Map, Value};

/// Merge `generated` onto `existing`, keeping every key only `existing` has.
///
/// Pure, so it is testable without a filesystem; [`write_preserving`] is this
/// plus the read and the write.
#[must_use]
pub fn merge_preserving(existing: &Value, generated: &Value) -> Value {
    match (existing, generated) {
        (Value::Object(old), Value::Object(new)) => {
            let mut out: Map<String, Value> = Map::new();
            for (k, v) in new {
                out.insert(
                    k.clone(),
                    match old.get(k) {
                        Some(prev) => merge_preserving(prev, v),
                        None => v.clone(),
                    },
                );
            }
            for (k, v) in old {
                if !out.contains_key(k) {
                    out.insert(k.clone(), v.clone());
                }
            }
            Value::Object(out)
        }
        _ => generated.clone(),
    }
}

/// Every leaf key path in a document, as `a.b[0].c` strings, sorted.
///
/// What the preservation gate compares: a key is either there after a
/// regeneration or it is not, and a VALUE that moved is the generator
/// re-measuring rather than deleting. Public because the gate and this module's
/// own tests must count the same paths.
#[must_use]
pub fn leaf_paths(v: &Value) -> Vec<String> {
    fn walk(v: &Value, at: &str, out: &mut Vec<String>) {
        match v {
            Value::Object(m) if !m.is_empty() => {
                for (k, child) in m {
                    let next = if at.is_empty() {
                        k.clone()
                    } else {
                        format!("{at}.{k}")
                    };
                    walk(child, &next, out);
                }
            }
            Value::Array(a) if !a.is_empty() => {
                for (i, child) in a.iter().enumerate() {
                    walk(child, &format!("{at}[{i}]"), out);
                }
            }
            _ => out.push(at.to_string()),
        }
    }
    let mut out = Vec::new();
    walk(v, "", &mut out);
    out.sort();
    out
}

/// **Write a generated document without deleting what it did not write.**
///
/// `generated` is serialised as canonical pretty JSON with a trailing newline —
/// the form every generator already wrote — after being merged onto the document
/// at `path`, if there is one.
///
/// Panics rather than returning: a generator that cannot write its own output
/// has produced nothing, and every other rule in this crate fails the same way
/// so that the run stops at the defect instead of shipping half a library.
pub fn write_preserving<T: serde::Serialize>(path: &Path, generated: &T) {
    let generated = serde_json::to_value(generated)
        .unwrap_or_else(|e| panic!("{}: the document serializes: {e}", path.display()));
    let merged = match std::fs::read_to_string(path) {
        Err(_) => generated,
        Ok(text) => {
            let existing: Value = serde_json::from_str(&text).unwrap_or_else(|e| {
                panic!(
                    "{}: a document already there does not parse, so nothing can be preserved \
                     from it — repair or delete it before regenerating: {e}",
                    path.display()
                )
            });
            let merged = merge_preserving(&existing, &generated);
            // The rule, asserted where it is performed rather than only in the
            // gate: a run that dropped a key is a run that must not write.
            let before = leaf_paths(&existing);
            let after = leaf_paths(&merged);
            let lost: Vec<&String> = before.iter().filter(|k| !after.contains(k)).collect();
            assert!(
                lost.is_empty(),
                "{}: regenerating would delete {} key(s) the generator did not write: {lost:?}",
                path.display(),
                lost.len(),
            );
            merged
        }
    };
    let text = serde_json::to_string_pretty(&merged)
        .unwrap_or_else(|e| panic!("{}: the document serializes: {e}", path.display()))
        + "\n";
    std::fs::write(path, text).unwrap_or_else(|e| panic!("{}: cannot write: {e}", path.display()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The keys a later step added survive a regeneration — at the top level and
    /// inside an anchor the generator does write.
    #[test]
    fn a_key_the_generator_did_not_write_survives() {
        let existing = json!({
            "prefab_id": "prefab/x",
            "walk_y": 1,
            "shown_faces": ["north"],
            "spatial_contract": { "entry": "hall" },
            "anchors": {
                "anchor/arrival": { "pos": [1, 1, 1], "role": "entry", "note": "the way in" },
                "anchor/hand-added": { "pos": [2, 1, 2] }
            }
        });
        let generated = json!({
            "prefab_id": "prefab/x",
            "walk_y": 2,
            "anchors": {
                "anchor/arrival": { "pos": [1, 2, 1] }
            }
        });
        let merged = merge_preserving(&existing, &generated);
        // The generator's own measurements win.
        assert_eq!(merged["walk_y"], json!(2));
        assert_eq!(merged["anchors"]["anchor/arrival"]["pos"], json!([1, 2, 1]));
        // And nothing it did not write is gone.
        assert_eq!(merged["shown_faces"], json!(["north"]));
        assert_eq!(merged["spatial_contract"]["entry"], json!("hall"));
        assert_eq!(merged["anchors"]["anchor/arrival"]["role"], json!("entry"));
        assert_eq!(
            merged["anchors"]["anchor/arrival"]["note"],
            json!("the way in")
        );
        assert_eq!(
            merged["anchors"]["anchor/hand-added"]["pos"],
            json!([2, 1, 2])
        );
        for k in leaf_paths(&existing) {
            assert!(
                leaf_paths(&merged).contains(&k),
                "`{k}` was deleted by a regeneration"
            );
        }
    }

    /// An array is replaced whole: it is a measurement of the bytes, and merging
    /// two of them element by element invents a document neither side wrote.
    #[test]
    fn an_array_is_the_generators_answer_whole() {
        let existing = json!({ "connectors": [{ "name": "a" }, { "name": "b" }] });
        let generated = json!({ "connectors": [{ "name": "a" }] });
        let merged = merge_preserving(&existing, &generated);
        assert_eq!(merged["connectors"], json!([{ "name": "a" }]));
    }

    /// A document with nothing beside it is written as it was generated.
    #[test]
    fn a_fresh_tree_writes_what_was_generated() {
        let generated = json!({ "prefab_id": "prefab/x" });
        assert_eq!(merge_preserving(&json!({}), &generated), generated);
    }

    /// Leaf paths name arrays by index and objects by key, so the gate compares
    /// the same things this module preserves.
    #[test]
    fn leaf_paths_reach_into_arrays_and_objects() {
        let paths = leaf_paths(&json!({
            "a": { "b": 1 },
            "c": [ { "d": 2 }, 3 ],
            "e": [],
            "f": {}
        }));
        assert_eq!(paths, vec!["a.b", "c[0].d", "c[1]", "e", "f"]);
    }
}
