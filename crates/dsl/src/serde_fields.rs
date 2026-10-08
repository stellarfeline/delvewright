//! serde field helpers shared by the stage types: the defaults and
//! `skip_serializing_if` predicates their attributes name.

/// serde default helper: `true` (used by DSL v0.4 `trigger.once`).
pub(crate) fn default_true() -> bool {
    true
}

/// serde `skip_serializing_if` helper: skip a `false` bool (DSL v0.4
/// `objective.stealth`), keeping older campaigns byte-identical.
pub(crate) fn is_false(b: &bool) -> bool {
    !*b
}

/// serde `skip_serializing_if` helper: skip a `0`.
pub(crate) fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// serde `skip_serializing_if` helper: skip a `[0, 0, 0]` offset.
pub(crate) fn is_zero3(v: &[i32; 3]) -> bool {
    *v == [0, 0, 0]
}
