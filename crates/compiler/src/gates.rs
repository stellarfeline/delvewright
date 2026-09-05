//! `close-gate` gate-block validation (DSL v0.6): the physical dual of `open-gate`
//! fills a gate anchor's region with the block the anchor declares (a basalt
//! boulder sealing a cave mouth, iron bars dropping across a doorway). A
//! `close-gate` on an anchor that declares **no** fill `block` — or that is not a
//! gate region at all — cannot be sealed, so it is rejected at validate-time
//! (`DW0343`). This lives in the compiler (not `dsl::validate`) because the fill
//! `block` is prefab metadata, which the DSL's anchor-registry surface does not
//! carry; it runs in the compiler's validate stage alongside the atmos sound/art
//! checks, so `validate`/`analyze`/`build` all catch it (validation tier, exit 1).

use delvewright_dsl::{Campaign, Diagnostic};

use crate::registry::PrefabRegistry;
use delvewright_dsl::{DwCode, ExitTier};
use std::collections::{BTreeMap, BTreeSet};

/// `DW0343`: a `close-gate` targets a gate anchor that declares no fill `block` in
/// its prefab metadata (or is not a gate region), so the compiler cannot seal it.
pub const DW_GATE_NO_BLOCK: DwCode = DwCode::every_version("DW0343", ExitTier::Build);

/// `DW0423`: two `close-gate` effects seal the **same** gate anchor with
/// different `sealed_hint` wordings.
///
/// The seal's answer belongs to the place, not to the firing: one anchor gets one
/// set of `dw_seal_<anchor>` hitboxes and one reward function, so a second
/// wording has nowhere to live and would be silently dropped. Rejected instead —
/// a line an author wrote and a player can never read is the same silence class
/// as the finding this verb exists to close.
pub const DW_SEAL_HINT_CONFLICT: DwCode = DwCode::every_version("DW0423", ExitTier::Build);

/// `DW0857`: a gate verb names an anchor **more than one of the campaign's areas
/// provides as a gate**, so which building it addresses is decided by nothing an
/// author can see.
///
/// The scope of uniqueness for a gate-anchor name is the **area**, and that is
/// not a new policy — it is the policy the DSL tier already states. `DW0142`
/// resolves every anchor reference against the anchors of the quest's own area
/// and makes exactly one exception, a cutscene camera, which is allowed to fly
/// anywhere. The compiler's by-name lookup honoured none of it: it walks a map
/// keyed by `(area, name)` and returns the first entry whose NAME matches, across
/// every area, first match wins.
///
/// While one placed area provides the name those two readings agree, which is why
/// nothing ever noticed. When two do, the compiler's answer is whichever area id
/// sorts first, and at the call site a green meaning *some other building
/// satisfies this* is indistinguishable from a green meaning *this one does*.
/// Measured on a campaign of eight zones: five names collided, two of them on the
/// critical path — a portcullis shadowed by a chapel door, and an escort beat
/// whose destination resolved back to the cell the NPC already stood on.
///
/// So this is not the unbound vacuity mode: the check examined something and
/// reported truthfully about it. It is the computed-key family — the lookup asked
/// the right question about the wrong object, and the answer came back honest and
/// affirmative.
///
/// **What is refused is the ambiguity, not the crossing.** A gate anchor exactly
/// one area provides resolves exactly as it always did, from anywhere, so no
/// campaign that was unambiguous moves a byte and a beat that legitimately
/// reaches into another area still does. Two pieces inside ONE area sharing a
/// name is not this finding either: that is what a `prefab_pool` is for — its
/// members share anchor names so that whichever member the solver seats provides
/// the anchor — and it has always been resolved within the area.
///
/// The repair for a campaign that hits it is to rename the gate in one of the two
/// areas. There is no second way, deliberately: an escape hatch here would have
/// to be the author naming which area they meant, and that is the area-scoped
/// resolution this diagnostic exists because the compiler does not have.
pub const DW_GATE_ANCHOR_AMBIGUOUS: DwCode = DwCode::every_version("DW0857", ExitTier::Build);

/// `DW0859`: an anchor reference **no scope settles** names a place more than one
/// of the campaign's areas provides.
///
/// [`DW_GATE_ANCHOR_AMBIGUOUS`] is the same finding keyed to four gate verbs.
/// This is the same rule keyed to the **object class the property belongs to** —
/// an anchor reference — because a name that two buildings answer to is a fact
/// about the name, not about the verb that happened to say it.
///
/// **Why this is a refusal and not simply area-scoped resolution.** The scope of
/// uniqueness is the AREA, so the right answer is always to resolve in the
/// referring area — and for the two object classes the DSL gives an area to
/// (`Npc`, `PlannedQuest`) that is exactly what the compiler now does. The other
/// eleven anchor-bearing classes — traps, shortcuts, timed gates, loot, lethal
/// volumes, ambushes, actors, waves, stealth zones, shops, environment triggers
/// — are flat campaign-wide vectors that **never record an area at all**, so
/// there is no scope to resolve them in and none can be invented here without a
/// DSL surface that says which area they belong to.
///
/// For those, a by-name match is a **candidate**, not an identity. One candidate
/// is an answer and resolves from anywhere — what is refused is the ambiguity,
/// not the crossing, so no unambiguous campaign moves a byte. Two candidates is
/// a question the compiler cannot answer, and answering it by whichever area id
/// sorts first is how a green meaning *another building satisfies this* became
/// indistinguishable from a green meaning *this one does*.
///
/// The hatch question: there is none, for the same reason `DW0857` has none. An
/// opt-out would have to be the author naming which area they meant, and that is
/// the area-scoped resolution the DSL cannot express for these classes — the
/// escape hatch and the missing capability are the same thing, so an opt-out
/// here would be secured by nothing the defect could not also supply.
pub const DW_ANCHOR_AMBIGUOUS: DwCode = DwCode::every_version("DW0859", ExitTier::Build);

/// Every area of `c` that provides `anchor` as a gate the compiler can fill —
/// **and which of that area's pieces provides it** — paired with the refusals
/// the authority raised while asking.
///
/// The denominator is **this campaign's own pieces** — each area's bare `prefab`,
/// or every member of its `prefab_pool` — and never the loaded library. A piece
/// no area binds cannot be placed, so it has no standing to answer a question
/// about what this campaign's gates are made of. Asking the library is what let
/// two pieces belonging to no area of the campaign satisfy `DW0343` on a shortcut
/// they had nothing to do with.
///
/// The **piece** is kept, not just the area, because it is the difference between
/// a refusal an author can act on and one they cannot. The remedy for an
/// ambiguous gate is to change which piece one of the areas binds, and an author
/// cannot choose which area to change without knowing what each one is bound to.
/// The walk already had the answer in hand and threw it away.
fn gate_providers(
    c: &Campaign,
    prefabs: &PrefabRegistry,
    anchor: &str,
) -> (BTreeMap<String, BTreeSet<String>>, Vec<String>) {
    let mut areas: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut refusals = Vec::new();
    for area in &c.world.content.areas {
        for piece in prefabs.area_pieces(area) {
            match prefabs.gate_anchor_in(&piece, anchor) {
                Ok(Some(_)) => {
                    areas
                        .entry(area.id.as_str().to_string())
                        .or_default()
                        .insert(piece.clone());
                }
                Ok(None) => {}
                Err(why) => refusals.push(format!("`{piece}`: {why}")),
            }
        }
    }
    (areas, refusals)
}

/// **What an author may do about an ambiguous anchor name, and what they may
/// not.**
///
/// One writer, because [`DW_GATE_ANCHOR_AMBIGUOUS`] and
/// [`DW_ANCHOR_AMBIGUOUS`] are one finding — a name two of this campaign's
/// buildings answer to — reached through two doors, and both used to lead with
/// *"rename the anchor in one of these areas"*.
///
/// **That prescription is not available to the author it is printed for.** The
/// anchor name is not the campaign's: it is a key in the piece's exported
/// metadata, shared by every campaign that binds that piece, and changing it goes
/// through the prefab library's own admission. A campaign author reading their
/// own refusal is told to edit a file they do not own, in a repository the
/// campaign does not include, for a change that would move every other campaign
/// using the piece.
///
/// The remedy that IS theirs is one line of `world.areas[]`: bind a different
/// piece to one of the named areas. That is why [`gate_providers`] keeps the
/// piece — an author cannot choose which area to change without being told what
/// each is bound to.
///
/// And where that is not what they want, the honest thing is to say the change is
/// in the piece and not here. "You cannot fix this from these documents" is a
/// remedy. Naming a repair the reader is not allowed to perform is not.
///
/// Reached only by a campaign whose space is stage-1 `areas[]`: this walks
/// `world.areas`, and a campaign carrying a `site-plan.json` is required to
/// declare that list empty (`DW0839`), so `areas.len() >= 2` cannot hold for a
/// derived map. A campaign carrying both is refused by `DW0839`, whose own
/// prescription — declare the empty list — also dissolves this one, so the two
/// do not pull against each other.
/// `areas` maps each provider area to the pieces of it that provide the name.
/// The piece set may be empty at a call site that does not know it, and the
/// clause naming pieces is then simply not written — never rendered as an empty
/// parenthetical.
pub(crate) fn anchor_ambiguity_remedy(areas: &BTreeMap<String, BTreeSet<String>>) -> String {
    format!(
        "An anchor name is unique per AREA, which is the scope `DW0142` already resolves every \
         reference in. What to do about it: the name itself belongs to the PIECE, not to this \
         campaign — it is a key in that piece's exported metadata, shared by every campaign that \
         binds the piece — so renaming it is a prefab-library change and is not something these \
         campaign documents can make. What they can make is the binding: in `world.areas[]`, give \
         one of these areas a `prefab` (or a `prefab_pool`) that does not declare this name, and \
         the remaining one provider resolves from anywhere exactly as before. If both areas must \
         keep the pieces they have, then the two names have to stop colliding in the pieces \
         themselves, and that change lives in the prefab library, through the piece's own \
         admission — you cannot reach it from here. The areas that provide it: {list}.",
        list = areas
            .iter()
            .map(|(a, pieces)| {
                if pieces.is_empty() {
                    return format!("`{a}`");
                }
                format!(
                    "`{a}` (through {})",
                    pieces
                        .iter()
                        .map(|p| format!("`{p}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
            .collect::<Vec<_>>()
            .join(", "),
    )
}

/// The `DW0857` a gate verb earns, or `None` when its anchor names one area.
///
/// One writer for the wording, because `open-gate` and the three fill-block verbs
/// raise the identical finding and a second copy of the sentence is how two rules
/// for one thing start.
fn ambiguous(
    areas: &BTreeMap<String, BTreeSet<String>>,
    anchor: &str,
    verb: &str,
    path: String,
) -> Option<Diagnostic> {
    if areas.len() < 2 {
        return None;
    }
    Some(Diagnostic::error(
        DW_GATE_ANCHOR_AMBIGUOUS,
        "quests",
        path,
        format!(
            "gate anchor `{anchor}` is provided by {n} of this campaign's areas ({list}), so \
             nothing says which one `{verb}` addresses. The compiler resolves a gate anchor by \
             name across every area and takes the first match, which is whichever area id sorts \
             first — a different building from the one the beat was written about, and a green \
             indistinguishable from the right answer. {remedy}",
            n = areas.len(),
            list = areas
                .keys()
                .map(|a| format!("`{a}`"))
                .collect::<Vec<_>>()
                .join(", "),
            remedy = anchor_ambiguity_remedy(areas),
        ),
    ))
}

/// Validate every verb that needs a gate anchor's **fill block** references an
/// anchor that declares one (`DW0343`): `close-gate` (which fills the region back
/// in) and a stage-5 `shortcut` (spec-0016 §2, whose unlock clears the region
/// `replace <block>` and whose gate is sealed from world-load by that very
/// block). Descends every nested effect list (`sequence` steps / lifecycle
/// bundles) so a `close-gate` buried in a timeline is checked too, and a
/// `timed-gate` (spec-0016 §4), whose clock fills and clears the region with that
/// block twice a cycle.
///
/// The same walk asks *which building* each verb addresses and raises `DW0857`
/// where the answer is more than one — see [`DW_GATE_ANCHOR_AMBIGUOUS`]. It
/// covers `open-gate` as well, which carries no fill-block obligation (it CLEARS
/// the region) but addresses a building exactly as the other three do: left out,
/// the one verb that opens a wall would be the one verb allowed to open somebody
/// else's.
pub fn check_close_gates(c: &Campaign, prefabs: &PrefabRegistry) -> Vec<Diagnostic> {
    let mut d = Vec::new();
    // `fills` distinguishes the three verbs that need a block to write from
    // `open-gate`, which only needs to know whose wall it is.
    let diag = |anchor: &str, verb: &str, fills: bool, path: String| -> Vec<Diagnostic> {
        // A DERIVED seam gate declares its block in the derivation, not in prefab
        // metadata — a site-plan campaign has no prefab to ask. Asking only the
        // prefab registry made this check measure a smaller world than the
        // campaign has: it refused a `shortcut` naming the very
        // `anchor/seam-<edge>` the derivation seals, and nothing was red, because
        // a check resolving against a truncated input refuses CONTENT. The
        // question is "can the compiler fill and clear this", and here it
        // demonstrably can — the block is the one the derivation writes.
        if delvewright_dsl::synthesized_gate_block(c, anchor).is_some() {
            return Vec::new();
        }
        // And a derivation with no `layout-graph.json` to read names no gate at
        // all, so the question above cannot be answered rather than answered no.
        // The same silence the DSL tier keeps over every other anchor reference
        // in that state, kept here for the same reason and from the same
        // authority: `DW0824` is the finding, and this gate is re-judged the
        // moment the graph exists.
        if delvewright_dsl::anchor_vocabulary_unknowable(c) {
            return Vec::new();
        }
        let (areas, refusals) = gate_providers(c, prefabs, anchor);
        if let Some(diagnostic) = ambiguous(&areas, anchor, verb, path.clone()) {
            return vec![diagnostic];
        }
        if !fills || (areas.len() == 1 && refusals.is_empty()) {
            return Vec::new();
        }
        let why = if refusals.is_empty() {
            String::new()
        } else {
            format!(" — {}", refusals.join("; "))
        };
        vec![Diagnostic::error(
            DW_GATE_NO_BLOCK,
            "quests",
            path,
            format!(
                "gate anchor `{anchor}` declares no fill `block` in the prefab metadata of any \
                 piece this campaign's areas can place (or is not a gate region){why}. \
                 `close-gate` fills the region with the anchor's declared block (the dual of \
                 `open-gate`), and a `shortcut` clears exactly that block on unlock. \
                 Prescription: {} — or remove the verb.",
                delvewright_dsl::Placement::of(c).anchor_remedy(
                    "declare the gate on an anchor of a piece an area binds — either as a \
                     `region` plus a `block`, or as a `resolves_to` of `bar:<region>` whose bar \
                     the piece's spatial contract carries"
                )
            ),
        )]
    };
    // Every root, every depth. Its own file's `check_seal_hints` (`DW0423`, twenty
    // lines below) already carried the corrected reasoning; this half was never
    // back-ported and still hand-listed three of the five roots. The walk also
    // reports the *nested* effect's own pointer now rather than its top-level
    // ancestor's, because `for_each_campaign_effect` threads the path down —
    // a `close-gate` inside a `sequence` step used to be blamed on the sequence.
    delvewright_dsl::for_each_campaign_effect(c, &mut |path, _site, e| {
        if let Some(a) = e.close_gate_anchor() {
            d.extend(diag(
                a.as_str(),
                "close-gate",
                true,
                format!("{path}/anchor"),
            ));
        }
        if let Some(a) = e.open_gate_anchor() {
            d.extend(diag(
                a.as_str(),
                "open-gate",
                false,
                format!("{path}/anchor"),
            ));
        }
    });
    // spec-0016 §2: a shortcut's gate is cleared `replace <block>` on unlock and is
    // sealed by that same block at world-load, so it carries the identical
    // fill-block obligation as `close-gate`.
    for (si, sc) in c.quests.content.shortcuts.iter().enumerate() {
        d.extend(diag(
            sc.gate.as_str(),
            "shortcut",
            true,
            format!("/content/shortcuts/{si}/gate"),
        ));
    }
    // spec-0016 §4: a timed gate's clock fills and clears the region with exactly
    // that block, twice a cycle — the same obligation, third verb.
    for (gi, g) in c.quests.content.timed_gates.iter().enumerate() {
        d.extend(diag(
            g.gate.as_str(),
            "timed-gate",
            true,
            format!("/content/timed_gates/{gi}/gate"),
        ));
    }
    d
}

/// Prove every gate anchor has ONE answer (`DW0423`, DSL v0.8).
///
/// A `close-gate`'s `sealed_hint` is the line the sealed region answers a
/// right-click with. The hitboxes that carry it are named after the **anchor**,
/// so all the `close-gate`s that seal one anchor share them — and therefore share
/// one wording. Two firings that disagree are rejected here rather than resolved
/// by declaration order.
///
/// A firing that authors no hint is compatible with anything: it asks for the
/// compiler's canonical English, which any authored wording refines. Only two
/// *authored* and *different* lines conflict.
///
/// Walks [`crate::plan::for_each_gate_effect`] — the **same** traversal the seal
/// planner uses, so the check and the emission can never disagree about which
/// firings exist. `dsl::for_each_campaign_effect` is deliberately not used: it
/// stops at the quests stage and would miss a `close-gate` inside a dialogue
/// option's `set-checkpoint` `on_respawn` bundle, which really does emit a fill.
pub fn check_seal_hints(c: &Campaign) -> Vec<Diagnostic> {
    let mut d = Vec::new();
    // anchor → (first authored wording, its path)
    let mut seen: std::collections::BTreeMap<String, (String, String)> = Default::default();
    crate::plan::for_each_gate_effect(c, &mut |site, e| {
        let (Some(anchor), Some(hint)) = (e.close_gate_anchor(), e.close_gate_sealed_hint()) else {
            return;
        };
        let path = &site.path;
        let key = anchor.as_str().to_string();
        match seen.get(&key) {
            None => {
                seen.insert(key, (hint.to_string(), format!("{path}/sealed_hint")));
            }
            Some((first, first_path)) if first != hint => {
                d.push(Diagnostic::error(
                    DW_SEAL_HINT_CONFLICT,
                    site.stage,
                    format!("{path}/sealed_hint"),
                    format!(
                        "gate anchor `{}` is sealed with two different `sealed_hint` lines — \
                         `{first}` at `{first_path}`, and `{hint}` here. A seal's answer belongs \
                         to the PLACE: one anchor carries one set of `dw_seal_…` hitboxes and one \
                         reward function, so the second wording would never reach a player. Give \
                         both firings the same line, or seal two different gate anchors.",
                        anchor.as_str()
                    ),
                ));
            }
            Some(_) => {}
        }
    });
    d
}
