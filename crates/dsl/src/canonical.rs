//! The single canonical JSON writer.
//!
//! Canonical form is defined in exactly one place — [`crate::fmt`] — and this is
//! the typed door to it: serialize with serde, then put the bytes through the
//! formatter. Full rules and rationale: `docs/reference/compiler.md` §9.
//!
//! **Why it delegates rather than defining its own form** (task #52). Before
//! `delvec fmt` existed this function *was* the canonical form, and its form was
//! serde's — struct-declaration field order, whatever `PrettyFormatter` emits.
//! `delvec edit apply` writes `world-edits.json` with it. Had `fmt` shipped with
//! a second, key-sorted form, the compiler would have written a file its own
//! `fmt --check` immediately rejected, and an author who ran both tools in the
//! same loop could not have satisfied both. Two writers claiming "canonical" for
//! one file class is the capability-duplication defect (`CLAUDE.md`,
//! Methodology): the form belongs to the *format*, not to whichever writer
//! needed it first. So there is one definition and this routes through it.
//!
//! The round-trip fixture gate (`tests/roundtrip.rs`) is unchanged and now
//! proves both properties at once: serde loses no field, **and** the fixture on
//! disk is in `delvec fmt` canonical form.

use serde::Serialize;

/// Serialize `value` to canonical JSON: [`crate::fmt`]'s form, with the trailing
/// newline it includes.
///
/// # Errors
/// Serialization failure. The re-format step cannot fail on serde's own output —
/// it is valid JSON with no duplicate keys by construction — so a formatter
/// refusal here is an internal invariant break and panics rather than inventing
/// an error variant callers would have to interpret.
pub fn to_canonical_string<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    let mut buf = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"  ");
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
    value.serialize(&mut ser)?;
    let pretty = String::from_utf8(buf).expect("serde_json emits valid UTF-8");
    Ok(crate::fmt::format_text(&pretty)
        .expect("serde_json output is valid, duplicate-free JSON, so the formatter accepts it"))
}

#[cfg(test)]
mod tests {
    use serde::Serialize;

    #[derive(Serialize)]
    struct Doc {
        /// Declared LAST on purpose: serde would emit it last, canonical form
        /// sorts it first. If this test ever passes with `zebra` last, the
        /// delegation has been undone.
        zebra: u32,
        alpha: Vec<u32>,
    }

    #[test]
    fn the_typed_writer_emits_the_formatter_s_form() {
        let s = super::to_canonical_string(&Doc {
            zebra: 1,
            alpha: vec![3, 1, 2],
        })
        .unwrap();
        assert_eq!(
            s,
            "{\n  \"alpha\": [\n    3,\n    1,\n    2\n  ],\n  \"zebra\": 1\n}\n"
        );
        // And what the compiler writes is what `delvec fmt --check` accepts.
        assert_eq!(crate::fmt::format_text(&s).unwrap(), s);
    }
}
