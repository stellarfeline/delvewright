//! The single canonical JSON writer.
//!
//! Canonical form: 2-space pretty printing, struct-declaration field order
//! (serde default), sorted map keys (via `BTreeMap`), and a trailing newline.
//! Parsing a valid stage document and re-serializing it canonically is
//! byte-identical to the on-disk fixture (enforced by `tests/roundtrip.rs`).

use serde::Serialize;

/// Serialize `value` to canonical JSON (2-space pretty, trailing newline).
pub fn to_canonical_string<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    let mut buf = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"  ");
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
    value.serialize(&mut ser)?;
    let mut s = String::from_utf8(buf).expect("serde_json emits valid UTF-8");
    s.push('\n');
    Ok(s)
}
