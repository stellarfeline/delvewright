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
//!
//! # The third proof: the same rule, one verb wider (`DW0544`)
//!
//! `DW0421` says *only the owner may **retire** an affordance's hardware*, and it
//! decides that by reading the **tag** in a `kill` selector. A region verb does
//! not select by tag — it selects by a **box** — and it does not destroy, it
//! **moves**. So a `teleport` whose volume happens to cover a recovery stake's
//! marker carries it away, and every tag-keyed proof in this module looks
//! straight past it. The consequence is not cosmetic: `stk_gc_<s>` retires a
//! marker no player holds a wager *at that position*, so the tick after the ride
//! the marker is deleted and the staked value is gone for good.
//!
//! The two fixes that suggest themselves are both defects in CLAUDE.md's
//! catalogue. "Teleport exempts engine machinery" is a general mechanism
//! privately re-implemented inside a verb (shape 2) — the exemption list would
//! live in the verb, keyed to what that verb happens to do. "The stake ledger
//! survives its marker moving" is shape 1, a capability keyed to the wrong
//! object — the stake compensating for a selector that grabbed something it
//! should never have grabbed. The real shape is the third: **the general
//! mechanism exists and its binding is too narrow to reach the objects it
//! should.**
//!
//! So the class is named at the OBJECT, once, and every region verb reads it.
//! Every entity the compiler summons declares which of two things it is:
//!
//! * [`FIXTURE_TAG`] (`dw_fixture`) — **a place.** Its position IS engine state:
//!   an affordance's `minecraft:interaction` hitbox, the visible hardware beside
//!   it, a stake marker left at a death, the mark a cutscene records to put a
//!   player back. Moving it does not move a thing, it *rewrites a fact*.
//! * [`BORNE_TAG`] (`dw_borne`) — **carried by a body.** An NPC is a body plus a
//!   co-located dialogue hitbox; the hitbox's position is the NPC's, and the two
//!   travel together or the delve silently loses its speaker. Owner ruling
//!   2026-08-08: everyone on the car travels.
//!
//! A cutscene *camera* is deliberately neither: its own driver re-asserts its
//! position every tick, so it is a body the engine flies rather than a place it
//! recorded, and a displacement it corrects on the next tick is not a defect.
//!
//! [`check_fixtures`] then proves two things over the shipped datapack, and they
//! are one rule seen from its two ends:
//!
//! 1. every engine-summoned hitbox, mark and piece of hardware declares its class
//!    — so a new affordance joins by existing, not by being remembered;
//! 2. every selector narrowed by a **positional box** excludes the fixture class
//!    — so a new region verb cannot reach a place by forgetting to say it will
//!    not.
//!
//! This is why the answer is NOT a type exemption. `lethal_volumes[]` names five
//! machinery **types**, which happens to cover every fixture today; a `teleport`
//! that copied that list would tear an NPC's dialogue hitbox off its body,
//! because a type says nothing about whether the thing wearing it is a place or a
//! passenger. The class does.

use crate::emit::{BuildFailure, BuildOutput};
use delvewright_dsl::DwCode;

/// `DW0420`: a compiler-owned interact affordance with **no visible hardware**.
///
/// The player is expected to right-click a point in the world; nothing the
/// compiler emits makes that point visible. Vanilla's `minecraft:interaction`
/// is an invisible hitbox by design, so an affordance built from one alone is
/// findable only by luck. This is an error and not a warning because the
/// failure mode is a soft-lock: the drowned bell's shortcut was the only route
/// back, and an unfindable lever is an unopenable door.
pub const DW_AFFORDANCE_INVISIBLE: DwCode = DwCode::every_version("DW0420");

/// `DW0421`: an affordance's visible hardware is destroyed by a function that
/// does not own the affordance.
///
/// Hardware may be retired by exactly one thing — the affordance's own
/// consumption (a shortcut's `shortcut_open_*`, a trap's `trap_disarm_*`).
/// Anything else reaching it (a cleanup pass whose selector widened, a tag
/// collision of the `DW0361` family) erases the player's only way to find a
/// live affordance, which is how the drowned bell read as a vanished lever.
pub const DW_AFFORDANCE_HARDWARE_ERASED: DwCode = DwCode::every_version("DW0421");

/// `DW0544`: an engine **fixture** — an entity whose position is engine state —
/// is reachable by a selector that quantifies over a **box**.
///
/// `DW0421`'s rule, one verb wider and one binding wider: only an affordance's
/// owner may disturb its hardware, and *moving* it is disturbing it. The rule is
/// stated over the emitted datapack, from its two ends, because either end alone
/// is a green that binds to nothing:
///
/// * a summon that declares neither class — the fixture is invisible to every
///   region selector's exclusion, so the exclusion protects nothing;
/// * a positional-box selector with no exclusion — the class exists and this verb
///   does not read it.
///
/// Both are compiler defects, never authoring ones: no campaign JSON can cause
/// either, and no campaign JSON can fix either. That is why the message is
/// addressed to whoever is changing the engine.
pub const DW_FIXTURE_REACHABLE: DwCode = DwCode::every_version("DW0544");

/// The class tag on every engine-summoned entity whose position **is** engine
/// state — an affordance hitbox, its visible hardware, a stake marker, a
/// cutscene's return mark. Region verbs that move entities exclude it.
pub const FIXTURE_TAG: &str = "dw_fixture";

/// The class tag on every engine-summoned entity **carried by a body**: today,
/// exactly an NPC's co-located dialogue hitbox. Region verbs move it, because
/// leaving it behind is how a delve keeps its speaker and loses its speech.
pub const BORNE_TAG: &str = "dw_borne";

/// The selector term every box-narrowed selector carries so it cannot reach a
/// fixture. One negated tag, not a roster of types: the roster grows with the
/// engine, the class does not.
pub const FIXTURE_EXCLUDE: &str = "tag=!dw_fixture";

/// The entity tag carried by an affordance's visible hardware, derived from the
/// affordance's own tag so the pairing is structural and needs no bookkeeping.
pub fn hardware_tag(affordance_tag: &str) -> String {
    format!("dw_hw_{affordance_tag}")
}

/// The binding ledger for the fixture-class proof
/// (`docs/reference/playtest-methodology.md` rule 1).
///
/// Both counts matter and for opposite reasons. Zero **fixtures** means the class
/// is empty, so every selector exclusion in the build is inert. Zero **box
/// selectors** means no region verb was examined, so the clause that matters to
/// the motivating defect looked at nothing. A build reporting either is not a
/// pass; it is a campaign that does not exercise the rule, and the ledger says so
/// rather than leaving a reader to assume.
///
/// **The two are reported separately, and that is deliberate.** The rule has two
/// arms and most campaigns bind one and not the other:
/// `nobodys-cave-island` declares no `teleport` and no `lethal_volumes[]`, so its
/// ledger reads 47 fixtures, 5 borne, and **zero** box selectors. A bare
/// `unbound: true` on a build that genuinely examined 47 objects is how a reader
/// learns to skip the field — and a gate everyone skips is one of this project's
/// named vacuity modes with extra steps. So [`Self::unbound_reason`] says which
/// arm found nothing and why, in the words the round summary needs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FixtureGate {
    /// Engine-summoned entities that declared [`FIXTURE_TAG`].
    pub fixtures: usize,
    /// Engine-summoned entities that declared [`BORNE_TAG`] (an NPC hitbox).
    pub borne: usize,
    /// Selectors narrowed by a positional box that were checked for the
    /// exclusion — the region verbs of this build.
    pub box_selectors: usize,
    /// PackTest templates generated for the one defect in this family that has
    /// no compile-time form: a runtime-placed marker inside a real teleport
    /// volume. A compile-time-only green over a runtime mechanism is the vacuity
    /// this number exists to make visible. Filled by the emitter.
    pub packtests: usize,
}

impl FixtureGate {
    /// Which arm of the rule examined nothing, in the words a round summary
    /// needs — or `None` when both bound.
    ///
    /// A campaign with no region verb at all is the ordinary case, not a defect,
    /// and saying so is the difference between a ledger a reader acts on and one
    /// they learn to skip.
    pub fn unbound_reason(&self) -> Option<&'static str> {
        match (self.fixtures, self.box_selectors) {
            (0, 0) => Some(
                "the campaign summons no engine fixture AND declares no region verb: neither \
                 arm of the rule had anything to examine",
            ),
            (0, _) => Some(
                "the campaign summons no engine fixture, so every selector exclusion in this \
                 build is inert — there is nothing for it to keep out",
            ),
            (_, 0) => Some(
                "the campaign declares no region verb (`teleport`, `lethal_volumes[]`), so no \
                 box-narrowed selector was examined. The class itself is bound; the clause the \
                 stake-marker defect lives in is not exercised here",
            ),
            _ => None,
        }
    }

    /// Whether this proof matched nothing on at least one arm.
    pub fn unbound(&self) -> bool {
        self.unbound_reason().is_some()
    }

    /// The ledger as the `validation/fixture-gate.json` artifact.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "fixtures_declared": self.fixtures,
            "borne_declared": self.borne,
            "box_selectors_examined": self.box_selectors,
            "packtest_templates": self.packtests,
            "unbound": self.unbound(),
            "unbound_reason": self.unbound_reason(),
        })
    }
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

/// Prove that no selector narrowed by a **positional box** can reach an engine
/// fixture, and return the binding ledger (`DW0544`).
///
/// Reads the shipped datapack, like [`check`], so the verdict is about commands
/// that ship rather than about intent — and so it covers every engine-summoned
/// hitbox, including the ones neither affordance authority knows by name (a
/// `close-gate` seal answer, a shortcut's wrong-side answer). A rule stated over
/// the emitted text needs no roster to stay total.
pub fn check_fixtures(out: &BuildOutput) -> Result<FixtureGate, BuildFailure> {
    let mut gate = FixtureGate::default();
    for (name, body) in shipped_functions(out) {
        for line in body.lines() {
            if let Some(kind) = summoned_class_subject(line) {
                let fixture = mentions_tag_in_nbt(line, FIXTURE_TAG);
                let borne = mentions_tag_in_nbt(line, BORNE_TAG);
                if fixture == borne {
                    return Err(BuildFailure::Diagnostic {
                        code: DW_FIXTURE_REACHABLE,
                        message: format!(
                            "`{name}` summons {kind} that declares {} class tag. Every entity \
                             the engine summons must say whether its position IS engine state \
                             (`{FIXTURE_TAG}` — an affordance hitbox, its hardware, a stake \
                             marker, a cutscene return mark) or belongs to a body that carries \
                             it (`{BORNE_TAG}` — an NPC's dialogue hitbox, which travels with \
                             its speaker per the owner's cargo-lift ruling). Region verbs \
                             quantify over a box and cannot ask a type; the class is the only \
                             thing they can read. Command: `{}`.",
                            if fixture { "BOTH" } else { "no" },
                            line.trim()
                        ),
                    });
                }
                if fixture {
                    gate.fixtures += 1;
                } else {
                    gate.borne += 1;
                }
            }
            for sel in box_narrowed_entity_selectors(line) {
                gate.box_selectors += 1;
                if !selector_has_term(&sel, FIXTURE_EXCLUDE) {
                    return Err(BuildFailure::Diagnostic {
                        code: DW_FIXTURE_REACHABLE,
                        message: format!(
                            "`{name}` selects entities by a positional box without excluding the \
                             fixture class: `@e[{sel}]`. A box reaches whatever stands in it, \
                             and what stands in it includes the engine's own places — a stake \
                             marker at a death, a bonfire's hitbox, a shortcut lever. A verb \
                             that MOVES them carries a fact away from the position that \
                             recorded it (`stk_gc_<s>` then deletes the marker and the wager \
                             with it); a verb that HARMS them erases hardware `DW0421` says \
                             only its owner may retire. Add `{FIXTURE_EXCLUDE}` to the \
                             selector. Do NOT reach for a `type=!…` roster instead: it grows \
                             with the engine, and on a moving verb it would also strip an NPC's \
                             dialogue hitbox off its body. Command: `{}`.",
                            line.trim()
                        ),
                    });
                }
            }
        }
    }
    Ok(gate)
}

/// What class-declaring thing this line summons, if any — the membership rule,
/// stated once.
///
/// `minecraft:interaction` and `minecraft:marker` are engine machinery wherever
/// they appear: the compiler is the only thing that summons either. An
/// `item_display` is judged by `dw_marker`, the tag the engine already puts on
/// every piece of visible affordance hardware — which deliberately leaves the
/// cutscene camera (an `item_display` with no such tag) out, because its own
/// driver re-asserts its position every tick.
fn summoned_class_subject(line: &str) -> Option<&'static str> {
    let rest = line.split_once("summon ")?.1;
    match rest.split_whitespace().next()? {
        "minecraft:interaction" => Some("an interaction hitbox"),
        "minecraft:marker" => Some("an engine mark"),
        "minecraft:item_display" if mentions_tag_in_nbt(line, "dw_marker") => {
            Some("affordance hardware")
        }
        _ => None,
    }
}

/// Every `@e[…]` selector in `line` whose arguments carry a positional box — i.e.
/// every region selector, whatever verb wrote it.
///
/// `@a`/`@p`/`@r` are player-only and no player is a fixture, so a box on one of
/// those is not this rule's business. `@s` is already bound by an enclosing
/// `execute as`, whose own selector is the one judged.
fn box_narrowed_entity_selectors(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut from = 0usize;
    while let Some(i) = line[from..].find("@e[") {
        let open = from + i + "@e[".len();
        let Some(close) = matching_bracket(line, open) else {
            break;
        };
        let args = &line[open..close];
        if selector_terms(args).any(|t| t.split_once('=').is_some_and(|(k, _)| k == "x")) {
            out.push(args.to_string());
        }
        from = close + 1;
    }
    out
}

/// The index of the `]` closing the selector that opens at `open`, tracking
/// nesting so an `nbt={Tags:["x"]}` argument cannot end the scan early.
///
/// `open` is a BYTE index and the scan is over `line[open..]`, not over a
/// character count from the start: a `tellraw` component elsewhere on the line
/// may hold a translated string, and slicing a byte offset produced by counting
/// characters would land mid-codepoint and panic.
fn matching_bracket(line: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (off, c) in line.get(open..)?.char_indices() {
        let i = open + off;
        match c {
            '[' | '{' => depth += 1,
            '}' => depth = depth.checked_sub(1)?,
            ']' if depth == 0 => return Some(i),
            ']' => depth -= 1,
            _ => {}
        }
    }
    None
}

/// A selector's top-level `key=value` terms, splitting on commas that are not
/// inside a nested `[…]` or `{…}`.
fn selector_terms(args: &str) -> impl Iterator<Item = &str> {
    let mut depth = 0usize;
    let mut start = 0usize;
    let mut out: Vec<&str> = Vec::new();
    for (i, c) in args.char_indices() {
        match c {
            '[' | '{' => depth += 1,
            ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                out.push(&args[start..i]);
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    if start < args.len() {
        out.push(&args[start..]);
    }
    out.into_iter()
}

/// Whether a selector's argument list carries `term` as a whole top-level term.
fn selector_has_term(args: &str, term: &str) -> bool {
    selector_terms(args).any(|t| t == term)
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

    // -----------------------------------------------------------------------
    // DW0544 — the fixture class
    // -----------------------------------------------------------------------

    /// A stake marker summoned with no class declaration. This is the clause that
    /// keeps the exclusion from being decorative: an undeclared fixture is one a
    /// `tag=!dw_fixture` cannot possibly keep out.
    #[test]
    fn dw0543_a_summon_that_declares_no_class() {
        let out = out_with(&[(
            "stk_fill_embers",
            "execute unless entity @e[tag=dw_stk_embers,distance=..1] run summon \
             minecraft:interaction ~ ~ ~ {Tags:[\"dw_stk_embers\"]}",
        )]);
        let err = check_fixtures(&out).unwrap_err();
        let BuildFailure::Diagnostic { code, message } = err else {
            panic!("expected a coded diagnostic");
        };
        assert_eq!(code, DW_FIXTURE_REACHABLE);
        assert_eq!(code.id(), "DW0544");
        assert!(message.contains("stk_fill_embers"), "{message}");
        assert!(message.contains("dw_fixture"), "{message}");
    }

    /// Both classes at once is as broken as neither: the entity would be a place
    /// and a passenger, and the two verbs reading it would disagree.
    #[test]
    fn dw0543_a_summon_that_declares_both_classes() {
        let out = out_with(&[(
            "setup_finish",
            "summon minecraft:interaction 1 2 3 {Tags:[\"dw_fixture\",\"dw_borne\",\"dw_npc_x\"]}",
        )]);
        let err = check_fixtures(&out).unwrap_err();
        let BuildFailure::Diagnostic { code, message } = err else {
            panic!("expected a coded diagnostic");
        };
        assert_eq!(code, DW_FIXTURE_REACHABLE);
        assert!(message.contains("BOTH"), "{message}");
    }

    /// The motivating defect in its emitted form: a `teleport` whose selector is
    /// a bare box. It reaches the marker, carries it out from under its own
    /// ledger, and `stk_gc_<s>` deletes it on the next tick.
    #[test]
    fn dw0543_a_box_selector_with_no_exclusion() {
        let out = out_with(&[(
            "teleport_ab",
            "tp @e[x=4,dx=2,y=64,dy=2,z=7,dz=2] 5.5 65.0 4.5",
        )]);
        let err = check_fixtures(&out).unwrap_err();
        let BuildFailure::Diagnostic { code, message } = err else {
            panic!("expected a coded diagnostic");
        };
        assert_eq!(code, DW_FIXTURE_REACHABLE);
        assert!(message.contains("teleport_ab"), "{message}");
        assert!(
            message.contains("tag=!dw_fixture"),
            "the message must carry the fix: {message}"
        );
    }

    /// A `type=!…` roster is NOT an acceptable substitute, however complete it
    /// looks. It is keyed to what one verb does to an entity, and inherited by a
    /// verb that moves rather than deletes it would strip an NPC's dialogue
    /// hitbox off its body.
    #[test]
    fn dw0543_a_type_roster_is_not_the_class() {
        let out = out_with(&[(
            "lethal_pit",
            "execute as @e[x=1,dx=0,y=2,dy=0,z=3,dz=0,type=!minecraft:player,\
             type=!minecraft:interaction,type=!minecraft:item_display] run damage @s 1000 \
             minecraft:fall",
        )]);
        let err = check_fixtures(&out).unwrap_err();
        let BuildFailure::Diagnostic { code, .. } = err else {
            panic!("expected a coded diagnostic");
        };
        assert_eq!(code, DW_FIXTURE_REACHABLE);
    }

    /// Both halves declared and excluded: clean, and the ledger counts what it
    /// looked at.
    #[test]
    fn a_declared_class_and_an_excluded_box_pass_and_bind() {
        let out = out_with(&[
            (
                "stk_fill_embers",
                "summon minecraft:interaction ~ ~ ~ {Tags:[\"dw_fixture\",\"dw_stk_embers\"]}\n\
                 summon minecraft:item_display ~ ~ ~ \
                 {Glowing:1b,Tags:[\"dw_fixture\",\"dw_marker\",\"dw_hw_dw_stk_embers\"]}",
            ),
            (
                "npc_keeper",
                "summon minecraft:interaction 1 2 3 {Tags:[\"dw_borne\",\"dw_npc_keeper\"]}",
            ),
            (
                "teleport_ab",
                "tp @e[x=4,dx=2,y=64,dy=2,z=7,dz=2,tag=!dw_fixture] 5.5 65.0 4.5",
            ),
        ]);
        let gate = check_fixtures(&out).unwrap();
        assert_eq!(gate.fixtures, 2);
        assert_eq!(gate.borne, 1);
        assert_eq!(gate.box_selectors, 1);
        assert_eq!(gate.unbound_reason(), None);
        assert!(!gate.to_json()["unbound"].as_bool().unwrap());
    }

    /// A campaign that declares no region verb binds the class and not the
    /// clause, and the ledger must say WHICH — `nobodys-cave-island` is exactly
    /// this shape (47 fixtures, 0 box selectors), and a bare `unbound: true` over
    /// 47 examined objects is how a field stops being read.
    #[test]
    fn the_ledger_names_which_arm_found_nothing() {
        let out = out_with(&[(
            "setup_finish",
            "summon minecraft:interaction 1 2 3 {Tags:[\"dw_fixture\",\"dw_bonfire_0\"]}",
        )]);
        let gate = check_fixtures(&out).unwrap();
        assert_eq!(gate.fixtures, 1);
        assert_eq!(gate.box_selectors, 0);
        let why = gate.unbound_reason().expect("one arm bound nothing");
        assert!(why.contains("no region verb"), "{why}");
        assert!(
            why.contains("class itself is bound"),
            "it must not read as though the class failed: {why}"
        );
        // …and the opposite shape says the opposite thing.
        let out = out_with(&[(
            "teleport_ab",
            "tp @e[x=4,dx=2,y=64,dy=2,z=7,dz=2,tag=!dw_fixture] 5.5 65.0 4.5",
        )]);
        let gate = check_fixtures(&out).unwrap();
        let why = gate.unbound_reason().expect("one arm bound nothing");
        assert!(why.contains("inert"), "{why}");
    }

    /// A tag-narrowed selector is `DW0421`'s business, not this rule's — the
    /// obligation is on the verbs that quantify over a REGION, and widening it to
    /// every selector in the datapack would be an obligation with no defect
    /// behind it.
    #[test]
    fn a_tag_narrowed_selector_is_not_a_region_verb() {
        let out = out_with(&[("move_walker", "tp @e[tag=dw_pup_walker] 5.5 65.0 4.5")]);
        let gate = check_fixtures(&out).unwrap();
        assert_eq!(gate.box_selectors, 0);
    }

    /// A player selector may carry a box freely: no player is a fixture, and
    /// `damage-players`, `give-effect` and a lethal volume's `@a` half all
    /// legitimately quantify over one.
    #[test]
    fn a_player_box_selector_is_not_this_rules_business() {
        let out = out_with(&[(
            "lethal_pit",
            "execute as @a[x=5,dx=0,y=65,dy=0,z=8,dz=0,tag=!dw_cutscene] run function ns:kill",
        )]);
        let gate = check_fixtures(&out).unwrap();
        assert_eq!(gate.box_selectors, 0);
    }

    /// A cutscene camera is deliberately outside the class: its own driver
    /// re-asserts its position every tick, so it is a body the engine flies
    /// rather than a place it recorded.
    #[test]
    fn a_cutscene_camera_declares_nothing_and_that_is_fine() {
        let out = out_with(&[(
            "cutscene_arm",
            "summon minecraft:item_display 1 2 3 {Tags:[\"dw_cam_shot\",\"dw_cam0_shot\"]}",
        )]);
        let gate = check_fixtures(&out).unwrap();
        assert_eq!(gate.fixtures, 0);
        assert_eq!(gate.borne, 0);
    }

    /// PackTest fixtures set themselves up destructively, summon witnesses of
    /// every type and re-select them by box. They are tooling, not the delve.
    #[test]
    fn packtest_templates_are_not_judged_by_the_class_rule() {
        let mut out = out_with(&[(
            "teleport_ab",
            "tp @e[x=4,dx=2,y=64,dy=2,z=7,dz=2,tag=!dw_fixture] 5.5 65.0 4.5",
        )]);
        out.insert(
            "packtest-datapack/data/ns/test/t.mcfunction".to_string(),
            b"summon minecraft:interaction 1 2 3 {Tags:[\"dw_tptest\"]}\n\
              execute store result score #n dw.sys if entity @e[tag=dw_tptest,x=1,dx=2,y=2,dy=2,z=3,dz=2]"
                .to_vec(),
        );
        let gate = check_fixtures(&out).unwrap();
        assert_eq!(gate.fixtures, 0);
        assert_eq!(gate.box_selectors, 1);
    }

    /// The selector scanner must not end a selector early on a `]` nested inside
    /// an `nbt=` argument, or a box selector would read as unnarrowed and the
    /// rule would fire on a correct build.
    #[test]
    fn nested_brackets_do_not_end_a_selector() {
        let sels = box_narrowed_entity_selectors(
            "tp @e[nbt={Tags:[\"a\"]},x=1,dx=2,y=2,dy=2,z=3,dz=2,tag=!dw_fixture] 1 2 3",
        );
        assert_eq!(sels.len(), 1);
        assert!(selector_has_term(&sels[0], FIXTURE_EXCLUDE), "{sels:?}");
    }

    /// **The scan indexes BYTES, and mixing the two units drops the selector
    /// silently rather than loudly.**
    ///
    /// Walking `char_indices().skip(<byte offset>)` starts too far right by
    /// exactly the excess bytes of the text to the left. Where that excess
    /// overshoots the selector's own length the closing bracket is never found,
    /// the scan gives up, and the rule reports green having examined nothing —
    /// the vacuity this project names most often, arriving as a units bug rather
    /// than as a missing check.
    ///
    /// **Stated honestly: no line the engine emits today puts wide text left of a
    /// box selector**, so this is the scanner being correct in its own terms
    /// rather than a live defect fixed. The test pins the threshold instead of
    /// claiming more than that — measured on the old arithmetic, a selector
    /// survives 20 CJK characters to its left and is lost at 30, which is one
    /// ordinary line of authored Chinese.
    #[test]
    fn a_selector_is_examined_however_wide_the_text_left_of_it() {
        for n in [2usize, 20, 30, 60] {
            let line = format!(
                "execute if data storage dw:x {{fallback:\"{}\"}} run \
                 tp @e[x=1,dx=2,y=2,dy=2,z=3,dz=2,tag=!dw_fixture] 1 2 3",
                "沉".repeat(n)
            );
            let sels = box_narrowed_entity_selectors(&line);
            assert_eq!(
                sels.len(),
                1,
                "{n} wide chars left of the selector: {sels:?}"
            );
            assert!(selector_has_term(&sels[0], FIXTURE_EXCLUDE), "{sels:?}");
        }
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
