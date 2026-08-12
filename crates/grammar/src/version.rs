//! The grammar program document's own version, and the fence new surface rides.
//!
//! A [`Program`](crate::ir::Program) is a long-lived on-disk document
//! (ADR-0018 §4/§5), and the crate's semver covers its Rust API rather than that
//! document. Two compatibility surfaces, so the document carries its own
//! version: an engine that meets a version it does not know **refuses** instead
//! of parsing what it recognises and quietly emitting a different world.
//!
//! The shape is `delvewright-dsl`'s, transplanted rather than invented. A
//! supported list, an ordinal, and every version predicate written as
//! `ordinal(v) >= n`, so introducing a version is one edit in one place. A
//! construct introduced at version *n* is refused in a document that declares
//! anything earlier — the same "you may not write surface your declared version
//! does not have" fence the campaign stages use, which is what lets an older
//! document keep compiling to the same bytes forever.

/// The latest program document version this crate implements — what
/// [`Program::new`](crate::ir::Program::new) stamps on a program built today.
pub const LATEST_PROGRAM_VERSION: &str = "1.1.0";

/// Every program document version this crate accepts, oldest first.
///
/// Each is an **additive superset** of the previous: `1.0.0` is the surface of
/// rules, splits, reorientations and marks; `1.1.0` adds the spatial contract —
/// the program-level `contract` block and the scope-bound `claim` node.
pub const SUPPORTED_PROGRAM_VERSIONS: &[&str] = &["1.0.0", "1.1.0"];

/// The version at which the spatial contract surface becomes writable.
pub const CONTRACT_SINCE: &str = "1.1.0";

/// True if `version` is a program document version this crate accepts.
pub fn is_supported_version(version: &str) -> bool {
    SUPPORTED_PROGRAM_VERSIONS.contains(&version)
}

/// The minor ordinal of a supported version (`1.1.0` → 1); `0` for anything this
/// crate does not accept.
///
/// Unsupported documents are refused before any predicate below is consulted, so
/// the shared `0` between "1.0.0" and "unknown" is never load-bearing; it only
/// guarantees that an unknown version can never *enable* a fenced construct.
pub fn minor_ordinal(version: &str) -> u32 {
    match version {
        "1.0.0" => 0,
        "1.1.0" => 1,
        _ => 0,
    }
}

/// True if `version` may write the spatial contract surface.
pub fn has_contract(version: &str) -> bool {
    is_supported_version(version) && minor_ordinal(version) >= minor_ordinal(CONTRACT_SINCE)
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
    fn the_contract_fence_opens_exactly_at_its_version() {
        assert!(!has_contract("1.0.0"));
        assert!(has_contract("1.1.0"));
        assert!(!has_contract("9.9.9"), "an unknown version enables nothing");
        assert!(is_supported_version(CONTRACT_SINCE));
    }
}
