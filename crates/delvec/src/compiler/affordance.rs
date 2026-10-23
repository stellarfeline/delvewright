//! The engine's fixtures and the region verbs that must not move them
//! (`DW0545`).
//!
//! # Only the owner may disturb a place (`DW0545`)
//!
//! A fixture's position is engine state, and a `kill` that selects it by **tag**
//! is a choice its owner makes. A region verb does
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
//!   a pressable's click box, a stake marker left at a death, the mark a
//!   cutscene records to put a player back. Moving it does not move a thing, it *rewrites a fact*.
//! * [`BORNE_TAG`] (`dw_borne`) — **carried by a body.** An NPC is a body plus a
//!   co-located dialogue hitbox; the hitbox's position is the NPC's, and the two
//!   travel together or the delve silently loses its speaker: everyone
//!   inside a moved volume travels.
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

use crate::compiler::emit::{BuildFailure, BuildOutput};
use delvewright_dsl::{DwCode, ExitTier};

delvewright_dsl::dw_code! {
    /// `DW0545`: an engine **fixture** — an entity whose position is engine state —
    /// is reachable by a selector that quantifies over a **box**.
    ///
    /// Only a fixture's owner may disturb it, and *moving* it is disturbing it. The rule is
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
    pub const DW_FIXTURE_REACHABLE: DwCode = DwCode::new("DW0545", ExitTier::Build);
}

/// The class tag on every engine-summoned entity whose position **is** engine
/// state — a click box, a stake marker, a cutscene's return mark. Region verbs that move entities exclude it.
pub const FIXTURE_TAG: &str = "dw_fixture";

/// The class tag on every engine-summoned entity **carried by a body**: today,
/// exactly an NPC's co-located dialogue hitbox. Region verbs move it, because
/// leaving it behind is how a delve keeps its speaker and loses its speech.
pub const BORNE_TAG: &str = "dw_borne";

/// The selector term every box-narrowed selector carries so it cannot reach a
/// fixture. One negated tag, not a roster of types: the roster grows with the
/// engine, the class does not.
pub const FIXTURE_EXCLUDE: &str = "tag=!dw_fixture";

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

/// Prove that no selector narrowed by a **positional box** can reach an engine
/// fixture, and return the binding ledger (`DW0545`).
///
/// Reads the shipped datapack, so the verdict is about commands
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
                             (`{FIXTURE_TAG}` — a click box, a stake marker, a cutscene \
                             return mark) or belongs to a body that carries \
                             it (`{BORNE_TAG}` — an NPC's dialogue hitbox, which travels with \
                             its speaker). Region verbs \
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
                             with it); a verb that HARMS them erases a press only its owner \
                             may retire. Add `{FIXTURE_EXCLUDE}` to the \
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
/// `item_display` is judged by the stake marker's own tag (`dw_stk`), the one
/// display the engine leaves at a place — which deliberately leaves the
/// cutscene camera (an `item_display` with no such tag) out, because its own
/// driver re-asserts its position every tick.
fn summoned_class_subject(line: &str) -> Option<&'static str> {
    let rest = line.split_once("summon ")?.1;
    match rest.split_whitespace().next()? {
        "minecraft:interaction" => Some("an interaction hitbox"),
        "minecraft:marker" => Some("an engine mark"),
        "minecraft:item_display" if line.contains("\"dw_stk") => Some("a stake marker's display"),
        // A cutscene's stand-in (spec-0095) stands where its player stood for
        // the cutscene's length: its position is engine state, not a body a
        // region verb moves or a loop counts. An NPC's skinned body is a
        // mannequin too, and is not one, so the class is read off the tag.
        "minecraft:mannequin"
            if mentions_tag_in_nbt(line, crate::compiler::standin::STANDIN_TAG) =>
        {
            Some("a cutscene stand-in")
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
pub(crate) fn matching_bracket(line: &str, open: usize) -> Option<usize> {
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
pub(crate) fn selector_terms(args: &str) -> impl Iterator<Item = &str> {
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
pub(crate) fn selector_has_term(args: &str, term: &str) -> bool {
    selector_terms(args).any(|t| t == term)
}

/// The shipped delve's `.mcfunction` bodies as `(unqualified name, body)`, in
/// deterministic path order. Excludes `packtest-datapack/` (tooling, ADR-0003).
pub(crate) fn shipped_functions(out: &BuildOutput) -> Vec<(String, String)> {
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

/// `Tags:["…","<tag>"]` — the summon NBT form.
fn mentions_tag_in_nbt(line: &str, tag: &str) -> bool {
    line.contains(&format!("\"{tag}\""))
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

    // -----------------------------------------------------------------------
    // DW0545 — the fixture class
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
        assert_eq!(code.id(), "DW0545");
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
                 {Tags:[\"dw_fixture\",\"dw_stk_display\"]}",
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
}
