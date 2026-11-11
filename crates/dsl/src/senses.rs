//! **What a kind hears** (spec-0132 §5.2): the registry of kinds that hunt by
//! sound, as data the exported path hands the bot.
//!
//! A record states, for one vanilla kind, how far a sound reaches it and how
//! long it takes to forget everything a sound earned it, each measured on the
//! pinned server and read from `docs/reference/`. The bot reads the record
//! off a pursuer's row and never holds a copy: before a move a crouched body
//! cannot make silently, it waits for every live pursuer to stand beyond the
//! reach, and gives up after the forgetting time. A kind absent from the table
//! hears nothing the engine knows of, and the bot waits for it never.
//!
//! Determinism (ADR-0006): the table is a slice in a fixed order.

/// One kind's hearing, measured on the pinned server.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KindHearing {
    /// The vanilla entity id, e.g. `minecraft:warden`.
    pub kind: &'static str,
    /// How far a vibration reaches it, in blocks, measured from its feet to
    /// the sound's cell in all three axes.
    pub reach: u32,
    /// How long, in game ticks, until no anger a sound earned it remains: the
    /// most anger it holds at one body over the rate it decays.
    pub forget_ticks: u32,
    /// Where in `docs/reference/` both were measured.
    pub read_site: &'static str,
}

/// **The registry of kinds that hear**, in a fixed order.
pub const KIND_HEARING: &[KindHearing] = &[KindHearing {
    kind: "minecraft:warden",
    reach: 16,
    // At most 150 anger at one body, decaying 1 a second: 150 s.
    forget_ticks: 150 * 20,
    read_site: "docs/reference/warden-behaviour.md §1, §2, §8",
}];

/// The hearing of `kind` (with or without the `minecraft:` namespace), or
/// `None` for a kind that hears nothing the engine knows of.
pub fn kind_hearing(kind: &str) -> Option<&'static KindHearing> {
    let kind = kind.strip_prefix("minecraft:").unwrap_or(kind);
    KIND_HEARING
        .iter()
        .find(|r| r.kind.strip_prefix("minecraft:") == Some(kind))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_warden_hears_sixteen_blocks_and_forgets_in_150_seconds() {
        let w = kind_hearing("warden").expect("the warden hears");
        assert_eq!(kind_hearing("minecraft:warden"), Some(w));
        assert_eq!((w.reach, w.forget_ticks), (16, 3000));
        assert_eq!(kind_hearing("minecraft:wither"), None);
    }
}
