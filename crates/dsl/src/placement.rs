//! **Where a campaign's places and anchor names come from — asked once.**
//!
//! A campaign has exactly one placement authority. The usual one is stage-1
//! `areas[]`, which seats prefab pieces on the compiler's fixed stride; a
//! campaign planned as a whole map hands the space to its `site-plan.json`
//! instead and the blockout is derived. `DW0839` refuses a campaign holding
//! both.
//!
//! Every rule that resolves an area id or an anchor name already asks which of
//! the two it is — [`crate::validate::AnchorProviders`] does it to build the
//! resolvable set, and three area sets in `crate::validate` do it to admit
//! [`crate::siteplan::SITE_AREA`]. What none of them asked is the second half of
//! the same question: **when the reference does not resolve, what is this author
//! allowed to write instead?**
//!
//! That half was hand-written beside each refusal at twenty sites, in the
//! vocabulary of the only kind of campaign that existed when the rules were
//! written. So a site-plan campaign was refused by `DW0112` with *"declare it in
//! stage-1 `world.areas`"* — which is precisely what `DW0839` refuses in a
//! campaign carrying a site plan, and which `DW0160` refuses again for having no
//! prefab to bind — and by `DW0142` with *"anchor names come from prefab
//! metadata; do NOT invent one"* against names that are synthesized by design and
//! have no metadata anywhere.
//!
//! CLAUDE.md names the shape: **when one gate's prescription is another gate's
//! refusal, the defect belongs to the PAIR**, and a gate that names a remedy owes
//! a check that the remedy is reachable. The repair is the constitution's own —
//! a capability belongs to the object class it acts on, so *"what may this author
//! write"* is a property of the campaign's placement authority and is answered in
//! one place, not re-decided at each verb.
//!
//! Nothing here decides whether a reference is wrong. Every predicate is exactly
//! what it was; only the sentence the author reads afterwards is chosen by the
//! campaign rather than by the site.

use crate::envelope::Campaign;

/// The one thing a campaign hands its space to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// Stage-1 `areas[]` seats prefab pieces. Anchor names come from prefab
    /// metadata; area ids are the ones the world document declares.
    Prefabs,
    /// A `site-plan.json` owns the space. There is exactly one area
    /// ([`crate::siteplan::SITE_AREA`]) and the anchor names are the ones the
    /// derivation places ([`crate::siteplan::synthesized_anchors`]).
    SitePlan,
    /// **Neither.** `areas[]` is empty and there is no site plan, so the campaign
    /// declares no place and nothing places an anchor: every area reference and
    /// every anchor reference in it is unresolvable, whatever it says.
    ///
    /// This is a real authoring state and it is the one the old messages served
    /// worst — a story layer written before its map. Refusing it is correct; the
    /// only question is what the author is told to do about it, and the answer
    /// is not "declare it in `world.areas`", because that names one of the two
    /// halves of a choice the author has not made yet and hides the other.
    NoMap,
}

impl Placement {
    /// Ask the campaign, once.
    ///
    /// A site plan wins when both are present, because that is what the
    /// resolution does: [`crate::validate::AnchorProviders`] and all three area
    /// sets admit the derived vocabulary whenever a plan is on disk, whatever
    /// `areas[]` says. Such a campaign is refused by `DW0839`, whose message is
    /// the one that matters there — this one is describing the set the resolver
    /// actually used.
    #[must_use]
    pub fn of(c: &Campaign) -> Self {
        if c.site_plan.is_some() {
            Self::SitePlan
        } else if c.world.content.areas.is_empty() {
            Self::NoMap
        } else {
            Self::Prefabs
        }
    }

    /// **What to write instead of an area id that does not resolve.**
    ///
    /// The `Prefabs` arm is the sentence every area refusal has always carried,
    /// unchanged, so a prefab campaign reads exactly what it read before.
    #[must_use]
    pub fn area_remedy(self) -> &'static str {
        match self {
            Self::Prefabs => "declare it in stage-1 `world.areas` or correct the reference",
            Self::SitePlan => {
                "this campaign's map is its site plan, so it has exactly one area, \
                 `area/site`. Point the reference at that, and do NOT declare the id in \
                 `world.areas`: a campaign carrying a site plan declares an empty `areas` list \
                 (`DW0839`), and an entry with no prefab bound to it is refused again \
                 (`DW0160`)"
            }
            Self::NoMap => {
                "this campaign declares no area at all: `world.areas` is empty and there is \
                 no `site-plan.json`, so it has no map for a reference to land in. Give it one \
                 placement authority, and only one (`DW0839`): either declare the area in \
                 stage-1 `world.areas` with a `prefab` or `prefab_pool` bound to it, or write \
                 the map pipeline (`geometry-brief.json`, then `layout-graph.json`, then \
                 `site-plan.json`) and name the campaign's one area `area/site`"
            }
        }
    }

    /// **What to write instead of an anchor name that does not resolve.**
    ///
    /// `prefab` is the sentence this call site has always printed for a prefab
    /// campaign, passed in rather than centralised because it is genuinely
    /// per-verb — a trap is told about `anchor/trap` markers, a trigger about
    /// its `at`. It is returned verbatim, so a prefab campaign's refusal is
    /// byte-identical to the one it printed before this module existed.
    ///
    /// It is *dropped* on the other two arms rather than appended, and that is
    /// the whole point: every one of those sentences prescribes a prefab
    /// operation, and a derived map has no prefab for the author to reach.
    #[must_use]
    pub fn anchor_remedy<'a>(self, prefab: &'a str) -> &'a str {
        match self {
            Self::Prefabs => prefab,
            Self::SitePlan => {
                "this campaign's map is its site plan, so there is no prefab metadata to \
                 read. Its anchor names are the ones the derivation places: `anchor/node-<place>` \
                 for each place the layout graph declares, `anchor/seam-<edge>` over each barred \
                 connection, `anchor/unlock-<edge>` on the far side of a one-sided one, and \
                 `spawn` for the entry. Write one of those, and do NOT add an `areas[]` entry to \
                 get a prefab: a campaign carrying a site plan declares an empty `areas` list \
                 (`DW0839`)"
            }
            Self::NoMap => {
                "this campaign has no map yet: `world.areas` is empty and there is no \
                 `site-plan.json`, so nothing places an anchor and no name can resolve. Give it \
                 one placement authority, and only one (`DW0839`): either declare an area in \
                 stage-1 `world.areas` with a `prefab` bound to it and write an anchor that \
                 prefab's metadata exposes, or write the map pipeline (`geometry-brief.json`, \
                 then `layout-graph.json`, then `site-plan.json`) and write one of the names its \
                 derivation places (`anchor/node-<place>`, `anchor/seam-<edge>`, \
                 `anchor/unlock-<edge>`, `spawn`)"
            }
        }
    }
}
