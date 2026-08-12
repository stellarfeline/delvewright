//! The grammar program document's own version, and the fence new surface rides.
//!
//! ADR-0018 §7 decided this before the first `Program` was checked in anywhere,
//! and this is the module that carries it out.
//!
//! A [`Program`](crate::ir::Program) is a long-lived on-disk document: the
//! authoring form `delve-grammar --file` reads, and the form `show` prints. The
//! crate's semver covers its Rust API, not that document. Two compatibility
//! surfaces, so the document carries its own version — and an engine that meets
//! a version it does not know **refuses**, instead of parsing the parts it
//! recognises and quietly emitting a different world.
//!
//! The shape is `delvewright-dsl`'s per-stage fence, transplanted rather than
//! invented: a supported list, an ordinal, and every predicate written as
//! `ordinal(v) >= n`, so introducing a version is one edit in one place. A
//! construct introduced at version *n* is refused in a document that declares
//! anything earlier.
//!
//! # Why an optional field needs this and a new node does not
//!
//! An unknown `Node` variant is safe on its own: `Node` is an internally tagged
//! enum, so an engine that predates the variant meets an `"op"` it does not know
//! and fails loud at `serde`, and every exhaustive `match` in this crate forces
//! an arm for it at compile time.
//!
//! A `#[serde(default)]` **struct field** has neither property. It rides through
//! every walk untouched in both directions: an engine that predates the field
//! deserialises the document with the field's default, expands, gates green, and
//! writes different geometry with nothing to say about it. `mirror` is the first
//! such field this document has grown, and the version fence is what makes an
//! engine that cannot honour it refuse the document instead of silently building
//! the unreflected shape.
//!
//! The fence cannot reach an engine older than the fence itself — that engine's
//! refusal would have to be code it already carries. What it does is make every
//! optional field from `1.1.0` on self-announcing, which is why the ledger in
//! `tools/check-grammar-optional-fields.py` names every one of them and why a
//! new one is a red until it is either at `1.0.0` or fenced here.

/// The latest program document version this crate implements — what
/// [`Program::new`](crate::ir::Program::new) stamps on a program built today.
pub const LATEST_PROGRAM_VERSION: &str = "1.1.0";

/// Every program document version this crate accepts, oldest first.
///
/// Each is an **additive superset** of the previous: `1.0.0` is the surface of
/// rules, splits, permuting reorientations and marks; `1.1.0` adds the frame's
/// direction — the `mirror` field of a `reorient` request and of an
/// `orientation` guard.
pub const SUPPORTED_PROGRAM_VERSIONS: &[&str] = &["1.0.0", "1.1.0"];

/// The version at which a frame may carry a reflection.
pub const MIRROR_SINCE: &str = "1.1.0";

/// True if `version` is a program document version this crate accepts.
pub fn is_supported_version(version: &str) -> bool {
    SUPPORTED_PROGRAM_VERSIONS.contains(&version)
}

/// The minor ordinal of a supported version (`1.1.0` → 1); `0` for anything this
/// crate does not accept.
///
/// Unsupported documents are refused before any predicate below is consulted, so
/// the shared `0` between `"1.0.0"` and an unknown version is never load-bearing;
/// it only guarantees that an unknown version can never *enable* a fenced
/// construct.
pub fn minor_ordinal(version: &str) -> u32 {
    match version {
        "1.0.0" => 0,
        "1.1.0" => 1,
        _ => 0,
    }
}

/// True if `version` may write a reflected frame.
pub fn has_mirror(version: &str) -> bool {
    is_supported_version(version) && minor_ordinal(version) >= minor_ordinal(MIRROR_SINCE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_latest_version_is_supported_and_is_the_newest_entry() {
        assert!(is_supported_version(LATEST_PROGRAM_VERSION));
        assert_eq!(
            SUPPORTED_PROGRAM_VERSIONS.last(),
            Some(&LATEST_PROGRAM_VERSION)
        );
        // Every entry has an ordinal of its own: a version added to the list but
        // not to `minor_ordinal` would silently share `1.0.0`'s fence.
        let mut seen = Vec::new();
        for v in SUPPORTED_PROGRAM_VERSIONS {
            let n = minor_ordinal(v);
            assert!(
                !seen.contains(&n),
                "{v} shares an ordinal with an earlier version"
            );
            seen.push(n);
        }
        assert!(
            seen.windows(2).all(|w| w[0] < w[1]),
            "the list is oldest first"
        );
    }

    #[test]
    fn the_mirror_fence_opens_exactly_at_its_version() {
        assert!(!has_mirror("1.0.0"));
        assert!(has_mirror("1.1.0"));
        assert!(!has_mirror("9.9.9"), "an unknown version enables nothing");
        assert!(is_supported_version(MIRROR_SINCE));
    }
}
