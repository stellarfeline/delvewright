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
//!
//! # One number, one surface
//!
//! [`SUPPORTED_PROGRAM_VERSIONS`] is the **ledger of the document format**, not
//! an inventory of what this crate happens to have built. A number in it names
//! exactly one surface, and names the same surface in every engine that knows
//! the number. Two changes that each take *the next free number* for a different
//! surface produce two engines that both call themselves `1.1.0` and disagree
//! about what a `1.1.0` document means — and then an engine accepts a document
//! declaring a version it "knows" and silently drops the half it does not
//! implement. That is the exact failure the fence exists to prevent,
//! reintroduced by the fence's own numbering.
//!
//! So a number is claimed once, by the change that introduces its surface, and a
//! number whose surface is introduced by a sibling change is **reserved** here
//! rather than skipped ([`RESERVED_VERSIONS`]). A skipped number is a free
//! number, and a free number is one two changes can take.
//! `tools/check-version-ledger-uniqueness.py` holds that against `origin/main`
//! for every version ledger in the repo, so the rule is a gate rather than a
//! thing someone remembers.
//!
//! A reserved version is **in the ledger and not accepted**. It has to be:
//! refusing it is the only loud answer this crate has. The IR does not yet carry
//! ADR-0018 §7.3's `deny_unknown_fields`, so a document declaring a reserved
//! version and writing that version's surface would otherwise deserialise with
//! the unknown fields dropped, expand, gate green, and build the wrong shape.

/// The latest program document version this crate implements — what
/// [`Program::new`](crate::ir::Program::new) stamps on a program built today.
pub const LATEST_PROGRAM_VERSION: &str = "1.2.0";

/// Every program document version the format has, oldest first — the ledger.
///
/// Each is an **additive superset** of the previous:
///
/// * `1.0.0` — rules, splits, reorientations and marks.
/// * `1.1.0` — the frame's direction: `mirror` on a `reorient` request and on an
///   `orientation` guard. Reserved here, see [`RESERVED_VERSIONS`].
/// * `1.2.0` — the spatial contract: the program-level `contract` block and the
///   scope-bound `claim` node.
pub const SUPPORTED_PROGRAM_VERSIONS: &[&str] = &["1.0.0", "1.1.0", "1.2.0"];

/// Ledger entries whose surface a **sibling** change introduces: the version,
/// and the name of the fence constant that change defines for it.
///
/// A reservation is what keeps a number from being taken twice while the change
/// that owns it is still in flight. It is deleted by the change that defines the
/// named constant, in the same edit — and a reservation whose constant is
/// already present in the same tree is a red
/// (`tools/check-version-ledger-uniqueness.py`), because at that point the
/// surface has landed and the reservation is refusing a version the engine can
/// honour.
pub const RESERVED_VERSIONS: &[(&str, &str)] = &[("1.1.0", "MIRROR_SINCE")];

/// The version at which the spatial contract surface becomes writable.
pub const CONTRACT_SINCE: &str = "1.2.0";

/// The fence constant that introduces `version`'s surface, when `version` is a
/// ledger entry this crate does not implement; `None` otherwise.
pub fn reserved_for(version: &str) -> Option<&'static str> {
    RESERVED_VERSIONS
        .iter()
        .find(|(v, _)| *v == version)
        .map(|(_, anchor)| *anchor)
}

/// Every program document version this crate will build — the ledger minus its
/// reservations. What a refusal names, because naming the whole ledger would
/// list versions the refusal was issued *for*.
pub fn accepted_versions() -> impl Iterator<Item = &'static str> {
    SUPPORTED_PROGRAM_VERSIONS
        .iter()
        .copied()
        .filter(|v| reserved_for(v).is_none())
}

/// True if `version` is a program document version this crate accepts: in the
/// ledger, and not reserved for a surface this crate does not implement.
pub fn is_supported_version(version: &str) -> bool {
    SUPPORTED_PROGRAM_VERSIONS.contains(&version) && reserved_for(version).is_none()
}

/// The minor ordinal of a ledger version (`1.2.0` → 2); `0` for anything the
/// ledger does not name.
///
/// Reserved and unknown versions are refused before any predicate below is
/// consulted, so the shared `0` between "1.0.0" and "unknown" is never
/// load-bearing; it only guarantees that an unknown version can never *enable* a
/// fenced construct. A reserved version keeps its own ordinal so the ledger
/// stays a contiguous sequence — the surface it names sits at that ordinal
/// whether or not this crate implements it.
pub fn minor_ordinal(version: &str) -> u32 {
    match version {
        "1.0.0" => 0,
        "1.1.0" => 1,
        "1.2.0" => 2,
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
        assert!(has_contract("1.2.0"));
        assert!(!has_contract("9.9.9"), "an unknown version enables nothing");
        assert!(is_supported_version(CONTRACT_SINCE));
    }

    /// A reserved number is spoken for by the ledger and refused by the engine.
    ///
    /// Both halves matter. In the ledger, so the number cannot be handed to a
    /// second surface; refused, so a document that declares it cannot be built
    /// with the surface this crate does not implement silently dropped.
    #[test]
    fn a_reserved_version_is_in_the_ledger_and_is_not_accepted() {
        assert!(
            !RESERVED_VERSIONS.is_empty(),
            "binding count 0: nothing is reserved, so this test examined no version. \
             Delete it with the last reservation, never leave it standing empty."
        );
        for (version, anchor) in RESERVED_VERSIONS {
            assert!(
                SUPPORTED_PROGRAM_VERSIONS.contains(version),
                "{version} is reserved but is not in the ledger — a number outside \
                 the ledger is a free number"
            );
            assert!(
                !is_supported_version(version),
                "{version} is reserved for {anchor} yet this crate accepts it; a \
                 document declaring it would be built with that surface dropped"
            );
            assert!(
                !has_contract(version),
                "no fence may open at a reserved version"
            );
            assert_eq!(reserved_for(version), Some(*anchor));
        }
        assert!(reserved_for(LATEST_PROGRAM_VERSION).is_none());
    }

    /// A refusal names the versions it would accept, never the whole ledger —
    /// listing a reserved version as acceptable is how the refusal becomes
    /// advice to try something that will be refused again.
    #[test]
    fn accepted_versions_is_the_ledger_minus_its_reservations() {
        let accepted: Vec<&str> = accepted_versions().collect();
        assert_eq!(accepted, vec!["1.0.0", "1.2.0"]);
        assert!(accepted.iter().all(|v| is_supported_version(v)));
        assert_eq!(
            accepted.len() + RESERVED_VERSIONS.len(),
            SUPPORTED_PROGRAM_VERSIONS.len()
        );
    }
}
