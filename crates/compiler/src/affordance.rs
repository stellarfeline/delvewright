//! Affordance hardware — the visible half of every right-click target the
//! compiler owns (`DW0420` / `DW0421`).
//!
//! ## The defect this module exists to make impossible
//!
//! The drowned-bell playtest soft-locked on a shortcut whose unlock "lever
//! vanished". Reproduced live on pinned 1.21.11: the lever never existed. The
//! compiler emitted the shortcut unlock as a bare, **invisible**
//! `minecraft:interaction` entity and documented the gap away — *"the physical
//! lever may also be in the prefab"* — delegating the affordance's visibility to
//! tileset folklore. The tileset carried no lever, so `anchor/l3-unlock` was an
//! air cell holding one invisible entity.
//!
//! What the player saw and then lost belonged to *unrelated machinery*: a
//! `reach-anchor` objective declared on the SAME anchor summoned a glowing,
//! named marker at the identical coordinates. Walking into its completion box
//! ran that objective's `kill @e[tag=dw_r_…]`, so the glow the player was
//! walking toward disappeared at the instant of arrival — leaving a right-click
//! target with no visible representation and the gate still sealed.
//!
//! Two independent failures, hence two proofs:
//!
//! * **`DW0420`** — an affordance with no compiler-owned visible hardware at
//!   all. The engine may never again ship a right-click target the player
//!   cannot see, whatever the tileset does or does not happen to carry.
//! * **`DW0421`** — an affordance whose hardware is destroyed by machinery that
//!   does not own it. Only the affordance's own consumption may retire its
//!   hardware; an unrelated `kill` reaching it (a tag collision, a cleanup pass
//!   widened by a later change) is the erasure class caught here.
//!
//! Both are **emission self-checks**: they read the finished datapack rather
//! than the plan, so they judge the commands that actually ship. Neither can
//! fire on a correct build — that is the point. They are the standing proof that
//! the fix stays fixed, in the same family as the exported-waypoint and
//! POV-camera self-checks.

use crate::emit::{BuildFailure, BuildOutput};

/// `DW0420`: a compiler-owned interact affordance with **no visible hardware**.
///
/// The player is expected to right-click a point in the world; nothing the
/// compiler emits makes that point visible. Vanilla's `minecraft:interaction`
/// is an invisible hitbox by design, so an affordance built from one alone is
/// findable only by luck. This is an error and not a warning because the
/// failure mode is a soft-lock: the drowned bell's shortcut was the only route
/// back, and an unfindable lever is an unopenable door.
pub const DW_AFFORDANCE_INVISIBLE: &str = "DW0420";

/// `DW0421`: an affordance's visible hardware is destroyed by a function that
/// does not own the affordance.
///
/// Hardware may be retired by exactly one thing — the affordance's own
/// consumption (a shortcut's `shortcut_open_*`, a trap's `trap_disarm_*`).
/// Anything else reaching it (a cleanup pass whose selector widened, a tag
/// collision of the `DW0361` family) erases the player's only way to find a
/// live affordance, which is how the drowned bell read as a vanished lever.
pub const DW_AFFORDANCE_HARDWARE_ERASED: &str = "DW0421";

/// The entity tag carried by an affordance's visible hardware, derived from the
/// affordance's own tag so the pairing is structural and needs no bookkeeping.
pub fn hardware_tag(affordance_tag: &str) -> String {
    format!("dw_hw_{affordance_tag}")
}

/// A compiler-owned interact affordance: a point the player must right-click for
/// the delve to progress (or to rest).
#[derive(Debug, Clone)]
pub struct Affordance {
    /// The authored id this affordance came from, for diagnostics
    /// (`shortcut/chapel-door`, `trap/dart-gallery`, `anchor/barrow-fire`).
    pub id: String,
    /// What kind of affordance it is, for diagnostics ("shortcut unlock", …).
    pub kind: &'static str,
    /// The affordance's own interaction-entity tag (`dw_sc_…`, `dw_trapdis_…`,
    /// `dw_bonfire_…`).
    pub tag: String,
    /// The unqualified name of the one function allowed to retire this
    /// affordance's hardware — its own consumption. `None` for hardware that is
    /// never retired (a bonfire is permanent scenery: it is rested at, never
    /// used up).
    pub retired_by: Option<String>,
}

/// Prove every affordance in `affordances` is visible in the shipped datapack
/// and that nothing but its owner destroys it (`DW0420` / `DW0421`).
///
/// Reads the emitted tree so the proof is about shipped commands, not intent.
/// Only the delve's own datapack is examined: `packtest-datapack/` is tooling
/// that never runs in a player's world and legitimately kills whatever it needs
/// in order to set a fixture up (`ADR-0003`).
pub fn check(affordances: &[Affordance], out: &BuildOutput) -> Result<(), BuildFailure> {
    let fns = shipped_functions(out);
    for a in affordances {
        let hw = hardware_tag(&a.tag);
        // DW0420 — something must MAKE it visible. A `summon` carrying the
        // hardware tag is the compiler's own visible display; a `setblock` at
        // the cell is the block form. Either satisfies the obligation.
        let visible = fns
            .iter()
            .any(|(_, body)| body.lines().any(|l| is_summon_of(l, &hw)));
        if !visible {
            return Err(BuildFailure::Diagnostic {
                code: DW_AFFORDANCE_INVISIBLE,
                message: format!(
                    "{} `{}` has no visible hardware: the datapack summons its \
                     `minecraft:interaction` hitbox (tag `{}`) but never a display \
                     tagged `{}`, so the player is asked to right-click a point \
                     nothing marks. An invisible affordance is a soft-lock \
                     (the drowned-bell shortcut); the compiler owns the \
                     affordance's visibility and must never leave it to the \
                     tileset.",
                    a.kind, a.id, a.tag, hw
                ),
            });
        }
        // DW0421 — only the owner may retire it.
        for (name, body) in &fns {
            if Some(name.as_str()) == a.retired_by.as_deref() {
                continue;
            }
            if let Some(line) = body.lines().find(|l| is_kill_of(l, &hw)) {
                return Err(BuildFailure::Diagnostic {
                    code: DW_AFFORDANCE_HARDWARE_ERASED,
                    message: format!(
                        "{} `{}` has its visible hardware (tag `{}`) destroyed by \
                         `{}`, which does not own it — only {} may retire it. \
                         Command: `{}`. Machinery that erases a live affordance's \
                         hardware leaves the player a right-click target they \
                         cannot see (the drowned-bell soft-lock).",
                        a.kind,
                        a.id,
                        hw,
                        name,
                        match a.retired_by.as_deref() {
                            Some(f) => format!("`{f}`"),
                            None => "nothing (this hardware is permanent)".to_string(),
                        },
                        line.trim()
                    ),
                });
            }
        }
    }
    Ok(())
}

/// The shipped delve's `.mcfunction` bodies as `(unqualified name, body)`, in
/// deterministic path order. Excludes `packtest-datapack/` (tooling, ADR-0003).
fn shipped_functions(out: &BuildOutput) -> Vec<(String, String)> {
    out.iter()
        .filter(|(p, _)| {
            p.starts_with("datapack/") && p.ends_with(".mcfunction") && p.contains("/function/")
        })
        .filter_map(|(p, b)| {
            let body = std::str::from_utf8(b).ok()?;
            let name = p
                .rsplit_once("/function/")?
                .1
                .strip_suffix(".mcfunction")?
                .to_string();
            Some((name, body.to_string()))
        })
        .collect()
}

/// Does this line summon a display carrying `tag`?
fn is_summon_of(line: &str, tag: &str) -> bool {
    line.contains("summon ") && mentions_tag_in_nbt(line, tag)
}

/// Does this line `kill` entities selected by `tag`?
///
/// Matches the tag inside a selector rather than anywhere in the line, so a
/// `kill @e[tag=dw_hw_x_extra]` is not mistaken for one targeting `dw_hw_x`.
fn is_kill_of(line: &str, tag: &str) -> bool {
    line.contains("kill @") && selector_mentions_tag(line, tag)
}

/// `Tags:["…","<tag>"]` — the summon NBT form.
fn mentions_tag_in_nbt(line: &str, tag: &str) -> bool {
    line.contains(&format!("\"{tag}\""))
}

/// `tag=<tag>` bounded by a selector delimiter, so `dw_hw_a` never matches
/// `tag=dw_hw_ab`.
fn selector_mentions_tag(line: &str, tag: &str) -> bool {
    let needle = format!("tag={tag}");
    let mut from = 0;
    while let Some(i) = line[from..].find(&needle) {
        let at = from + i;
        let after = line[at + needle.len()..].chars().next();
        // A selector argument ends at `,` or `]`; anything else means the tag
        // continues and this is a different, longer tag.
        if matches!(after, Some(',') | Some(']') | None) {
            return true;
        }
        from = at + needle.len();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn out_with(fns: &[(&str, &str)]) -> BuildOutput {
        let mut out: BuildOutput = BTreeMap::new();
        for (name, body) in fns {
            out.insert(
                format!("datapack/data/ns/function/{name}.mcfunction"),
                body.as_bytes().to_vec(),
            );
        }
        out
    }

    fn shortcut() -> Affordance {
        Affordance {
            id: "shortcut/chapel-door".to_string(),
            kind: "shortcut unlock",
            tag: "dw_sc_chapel_door".to_string(),
            retired_by: Some("shortcut_open_chapel_door".to_string()),
        }
    }

    /// The drowned-bell defect itself: an affordance emitted as nothing but an
    /// invisible `minecraft:interaction`.
    #[test]
    fn dw0420_invisible_affordance() {
        let out = out_with(&[(
            "setup_finish",
            "summon minecraft:interaction 82.5 71.0 -102.5 {Tags:[\"dw_sc_chapel_door\"]}",
        )]);
        let err = check(&[shortcut()], &out).unwrap_err();
        let BuildFailure::Diagnostic { code, message } = err else {
            panic!("expected a coded diagnostic");
        };
        assert_eq!(code, DW_AFFORDANCE_INVISIBLE);
        assert!(message.contains("shortcut/chapel-door"), "{message}");
        assert!(message.contains("dw_hw_dw_sc_chapel_door"), "{message}");
    }

    /// With compiler-owned hardware alongside the hitbox, the build is clean.
    #[test]
    fn visible_affordance_passes() {
        let out = out_with(&[(
            "setup_finish",
            "summon minecraft:interaction 82.5 71.0 -102.5 {Tags:[\"dw_sc_chapel_door\"]}\n\
             summon minecraft:item_display 82.5 71.0 -102.5 {Glowing:1b,Tags:[\"dw_marker\",\"dw_hw_dw_sc_chapel_door\"]}",
        )]);
        check(&[shortcut()], &out).unwrap();
    }

    /// The owner MAY retire its own hardware — that is the shortcut opening.
    #[test]
    fn owner_may_retire_its_own_hardware() {
        let out = out_with(&[
            (
                "setup_finish",
                "summon minecraft:item_display 1 2 3 {Tags:[\"dw_hw_dw_sc_chapel_door\"]}",
            ),
            (
                "shortcut_open_chapel_door",
                "kill @e[tag=dw_hw_dw_sc_chapel_door]",
            ),
        ]);
        check(&[shortcut()], &out).unwrap();
    }

    /// The erasure class: unrelated machinery kills a live affordance's hardware.
    #[test]
    fn dw0421_foreign_machinery_erases_hardware() {
        let out = out_with(&[
            (
                "setup_finish",
                "summon minecraft:item_display 1 2 3 {Tags:[\"dw_hw_dw_sc_chapel_door\"]}",
            ),
            ("complete_o_the_bar", "kill @e[tag=dw_hw_dw_sc_chapel_door]"),
        ]);
        let err = check(&[shortcut()], &out).unwrap_err();
        let BuildFailure::Diagnostic { code, message } = err else {
            panic!("expected a coded diagnostic");
        };
        assert_eq!(code, DW_AFFORDANCE_HARDWARE_ERASED);
        assert!(message.contains("complete_o_the_bar"), "{message}");
        assert!(
            message.contains("shortcut_open_chapel_door"),
            "names the only legitimate retirer: {message}"
        );
    }

    /// Permanent hardware (a bonfire) may be retired by nothing at all.
    #[test]
    fn dw0421_permanent_hardware_has_no_legitimate_killer() {
        let bonfire = Affordance {
            id: "anchor/barrow-fire".to_string(),
            kind: "bonfire",
            tag: "dw_bonfire_0".to_string(),
            retired_by: None,
        };
        let out = out_with(&[
            (
                "setup_finish",
                "summon minecraft:item_display 1 2 3 {Tags:[\"dw_hw_dw_bonfire_0\"]}",
            ),
            ("some_cleanup", "kill @e[tag=dw_hw_dw_bonfire_0]"),
        ]);
        let err = check(&[bonfire], &out).unwrap_err();
        let BuildFailure::Diagnostic { code, message } = err else {
            panic!("expected a coded diagnostic");
        };
        assert_eq!(code, DW_AFFORDANCE_HARDWARE_ERASED);
        assert!(message.contains("permanent"), "{message}");
    }

    /// A longer tag sharing our prefix is a different affordance, not a hit —
    /// the `DW0361` name-collision lesson applied to the matcher itself.
    #[test]
    fn tag_matching_is_exact_not_prefix() {
        assert!(selector_mentions_tag("kill @e[tag=dw_hw_a]", "dw_hw_a"));
        assert!(selector_mentions_tag(
            "kill @e[tag=dw_hw_a,type=x]",
            "dw_hw_a"
        ));
        assert!(!selector_mentions_tag("kill @e[tag=dw_hw_ab]", "dw_hw_a"));
    }

    /// PackTest fixtures set themselves up destructively and are not the delve.
    #[test]
    fn packtest_functions_are_not_judged() {
        let mut out = out_with(&[(
            "setup_finish",
            "summon minecraft:item_display 1 2 3 {Tags:[\"dw_hw_dw_sc_chapel_door\"]}",
        )]);
        out.insert(
            "packtest-datapack/data/ns/test/t.mcfunction".to_string(),
            b"kill @e[tag=dw_hw_dw_sc_chapel_door]".to_vec(),
        );
        check(&[shortcut()], &out).unwrap();
    }
}
