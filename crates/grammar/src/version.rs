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
//! anything earlier — which is what lets a document at `1.0.0` keep compiling to
//! the same bytes forever.
//!
//! # Why an optional field needs this, and why a new node gets it anyway
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
//! A new node is fenced too, for the other direction: a document that *declares*
//! `1.0.0` and writes `claim` or `bind` is claiming a compatibility it does not
//! have, and the fence refuses it where it is written. Serde protects the old
//! engine; the fence keeps the declared number honest.
//!
//! The fence cannot reach an engine older than the fence itself — that engine's
//! refusal would have to be code it already carries. What it does is make every
//! optional field from `1.1.0` on self-announcing, which is why the ledger in
//! `tools/check-grammar-ir-compat.py` names every one of them and why a new one
//! is a red until it is either at `1.0.0` or fenced here.
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
//! number whose surface is introduced by a sibling change still in flight is
//! **reserved** here rather than skipped ([`RESERVED_VERSIONS`]). A skipped
//! number is a free number, and a free number is one two changes can take.
//! `tools/check-version-ledger-uniqueness.py` holds that against `origin/main`
//! for every version ledger in the repo, so the rule is a gate rather than a
//! thing someone remembers.
//!
//! A reserved version is **in the ledger and not accepted**. It has to be:
//! refusing it is the only loud answer this crate has for a surface it does not
//! implement. A reservation is deleted by the change that defines the constant
//! it names, in the same edit.

/// The latest program document version this crate implements — what
/// [`Program::new`](crate::ir::Program::new) stamps on a program built today.
pub const LATEST_PROGRAM_VERSION: &str = "1.3.0";

/// Every program document version the format has, oldest first — the ledger.
///
/// Each is an **additive superset** of the previous:
///
/// * `1.0.0` — rules, splits, permuting reorientations and marks.
/// * `1.1.0` — the frame's direction: `mirror` on a `reorient` request and on an
///   `orientation` guard.
/// * `1.2.0` — the spatial contract: the program-level `contract` block and the
///   scope-bound `claim` node.
/// * `1.3.0` — the scope's names as a frame: the `bind` node.
pub const SUPPORTED_PROGRAM_VERSIONS: &[&str] = &["1.0.0", "1.1.0", "1.2.0", "1.3.0"];

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
///
/// Empty today: every number in the ledger names a surface this crate
/// implements.
pub const RESERVED_VERSIONS: &[(&str, &str)] = &[];

/// The version at which a frame may carry a reflection.
pub const MIRROR_SINCE: &str = "1.1.0";

/// The version at which the spatial contract surface becomes writable.
pub const CONTRACT_SINCE: &str = "1.2.0";

/// The version at which a scope's names may be rebound by a frame.
pub const BIND_SINCE: &str = "1.3.0";

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
        "1.3.0" => 3,
        _ => 0,
    }
}

/// True if `version` may write a reflected frame.
pub fn has_mirror(version: &str) -> bool {
    is_supported_version(version) && minor_ordinal(version) >= minor_ordinal(MIRROR_SINCE)
}

/// True if `version` may write the spatial contract surface.
pub fn has_contract(version: &str) -> bool {
    is_supported_version(version) && minor_ordinal(version) >= minor_ordinal(CONTRACT_SINCE)
}

/// True if `version` may write a binding frame.
pub fn has_bind(version: &str) -> bool {
    is_supported_version(version) && minor_ordinal(version) >= minor_ordinal(BIND_SINCE)
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

    /// Every fence in the ledger, and the surfaces below it that must stay shut.
    ///
    /// One table rather than three tests, because the property is the ledger's,
    /// not any one surface's: a fence opens at its own number and at nothing
    /// earlier, and an unknown version opens nothing at all. A new fence
    /// constant with no row here is the omission this shape makes visible.
    /// One fence: the constant's name, the version it opens at, and its predicate.
    type Fence = (&'static str, &'static str, fn(&str) -> bool);

    #[test]
    fn every_fence_opens_exactly_at_its_own_version() {
        let fences: &[Fence] = &[
            ("MIRROR_SINCE", MIRROR_SINCE, has_mirror as fn(&str) -> bool),
            ("CONTRACT_SINCE", CONTRACT_SINCE, has_contract),
            ("BIND_SINCE", BIND_SINCE, has_bind),
        ];
        assert_eq!(
            fences.len(),
            SUPPORTED_PROGRAM_VERSIONS.len() - 1,
            "binding count {}: every ledger version above 1.0.0 names one surface, so a \
             fence is missing from this table or from the ledger",
            fences.len()
        );
        for (name, since, open) in fences {
            assert!(is_supported_version(since), "{name}");
            assert!(open(since), "{name} does not open at {since}");
            for v in SUPPORTED_PROGRAM_VERSIONS {
                if minor_ordinal(v) < minor_ordinal(since) {
                    assert!(!open(v), "{name} is open at {v}, which is below {since}");
                }
            }
            assert!(!open("9.9.9"), "{name}: an unknown version enables nothing");
        }
    }

    /// A refusal names the versions it would accept, never the whole ledger —
    /// listing a reserved version as acceptable is how the refusal becomes
    /// advice to try something that will be refused again.
    #[test]
    fn accepted_versions_is_the_ledger_minus_its_reservations() {
        let accepted: Vec<&str> = accepted_versions().collect();
        assert!(accepted.iter().all(|v| is_supported_version(v)));
        assert!(
            accepted.iter().all(|v| reserved_for(v).is_none()),
            "a reserved version reached the accepted list"
        );
        assert_eq!(
            accepted.len() + RESERVED_VERSIONS.len(),
            SUPPORTED_PROGRAM_VERSIONS.len()
        );
        for (version, anchor) in RESERVED_VERSIONS {
            assert!(
                SUPPORTED_PROGRAM_VERSIONS.contains(version),
                "{version} is reserved for {anchor} but is not in the ledger — a number \
                 outside the ledger is a free number"
            );
            assert!(
                !is_supported_version(version),
                "{version} is reserved for {anchor} yet this crate accepts it; a document \
                 declaring it would be built with that surface dropped"
            );
        }
        assert!(reserved_for(LATEST_PROGRAM_VERSION).is_none());
    }
}
