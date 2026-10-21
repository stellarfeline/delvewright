//! **The flag producer table** — every `set-flag` a campaign can fire, with
//! the gate on the chain that fires it and the site that dates or places it.
//!
//! One table, read by both proofs that ask "who sets this flag": the
//! branch-coherent flow model (`delvec`'s `compiler::flow`) and the layout
//! graph's closure ([`crate::layout::Grants`]). Each reads the rows under its
//! own model of time — the flow model by quest-DAG position and path order, the
//! graph by place — but neither keeps a private list of which effects are
//! producers, so a producer one proof counts is a producer the other counts.
//!
//! The policy, root by root (the roots come from
//! [`crate::effects::for_each_effect_root`], the one enumeration of what
//! emission can lower; the match below is exhaustive, so a new root cannot be
//! added without this file deciding what it is):
//!
//! | Producer | Row |
//! |----------|-----|
//! | `set-flag` in `on_objective_complete[o]` | [`ProducerSite::ObjectiveComplete`], gated by the effect chain |
//! | `set-flag` in a quest's `on_complete` | [`ProducerSite::QuestComplete`], same gate rule |
//! | `set-flag` in an environment trigger's effects | [`ProducerSite::Ambient`] at the trigger's `at`, gated by its `requires_flags` |
//! | `set-flag` in a `traps[].payload` | ambient at the trap's `at`, gated by the trap's `requires_flags` |
//! | a trap's `disarm.sets_flag` | ambient at the disarm's `via`, gated by the trap's `requires_flags` |
//! | a timed gate's `disarm.sets_flag` | ambient at the disarm's `via`, ungated |
//! | `set-flag` in a `shortcuts[].on_unlock` | ambient at the shortcut's `unlock`, ungated |
//! | `set-flag` in a `shops[].offers[].effects` | ambient at the shop's `anchor`, ungated by flags |
//! | `set-flag` in a fight's `on_kill` | ambient, **story-only**: credited only by a story world that performs the kill (spec-0108 §4.1); no kill is forced, so every other reader skips it |
//! | `set-flag` in an `on_respawn` / `on_caught` / `on_death` / `on_land` / `on_cross` bundle | **none** — fired at times no static model can name |
//!
//! Each row names the ambient **act** that fires it ([`act_of_owner`],
//! [`disarm_act`]): the one spelling the flow model, the ending ledger and the
//! world rows share (spec-0108 §4.3). A quest bundle's row names none.
//!
//! A dialogue option's `set-flag` is not a row: it belongs to the speaker's
//! dialogue tree, which each proof reads with its own branch model (the flow
//! model's choice groups; the graph's talk-to beats).
//!
//! Determinism (ADR-0006): the rows come out in the root enumeration's fixed
//! order, then trap disarms, then timed-gate disarms, each in document order.

use crate::effects::{EffectRootOwner, for_each_effect_root};
use crate::envelope::Campaign;
use crate::ids::{AnchorId, FlagId};
use crate::{QuestEffect, Verb};

/// Where a producer's firing is dated or placed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProducerSite<'a> {
    /// Fires when this objective completes.
    ObjectiveComplete {
        /// The quest whose `on_objective_complete` map holds the bundle.
        quest: &'a str,
        /// The objective whose completion fires it.
        objective: &'a str,
    },
    /// Fires when this quest completes.
    QuestComplete {
        /// The quest.
        quest: &'a str,
    },
    /// Player-initiated, with no quest-DAG position: somebody strikes, uses,
    /// approaches, springs, disarms, unlocks or buys it, at `at`.
    Ambient {
        /// The anchor the act happens at, when it has one (a `strike-npc`
        /// trigger watches a character, not a cell).
        at: Option<&'a AnchorId>,
        /// The environment trigger whose effects produce it, when it is one.
        trigger: Option<&'a str>,
    },
}

/// One row of the table: a flag, the flags the chain that sets it requires,
/// and the site that fires it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlagProducer<'a> {
    /// The flag set.
    pub flag: &'a str,
    /// The conjunction of flag gates on the enclosing chain, sorted, deduplicated.
    pub requires: Vec<String>,
    /// Where the firing is dated or placed.
    pub site: ProducerSite<'a>,
    /// The ambient act that fires it, by id (spec-0108 §4.3) — a trigger, a
    /// trap's payload or disarm, a timed gate's disarm, a shortcut's unlock, a
    /// shop offer, an `on_kill`. `None` for a quest bundle's row.
    pub act: Option<String>,
    /// Credited only by a story world that performs its act (spec-0108 §4.1):
    /// an `on_kill` row. No kill is forced, so a reader that is not a story
    /// world's skips it.
    pub story_only: bool,
}

/// The act an ambient root is, by id — the one spelling the flow model, the
/// ending ledger and the world rows share (spec-0108 §4.3). `None` for a root
/// a story world neither performs nor skips (a quest bundle, `on_death`, an
/// assembly's `on_land`, a loop's `on_cross`, a dialogue respawn).
#[must_use]
pub fn act_of_owner(owner: &EffectRootOwner<'_>, path: &str) -> Option<String> {
    match owner {
        EffectRootOwner::Trigger(t) => Some(t.id.as_str().to_string()),
        EffectRootOwner::TrapPayload(t) => Some(t.id.as_str().to_string()),
        EffectRootOwner::ShortcutUnlock(s) => Some(s.id.as_str().to_string()),
        EffectRootOwner::ShopOffer(h) => {
            // `/content/shops/<h>/offers/<i>/effects`: segment 5 is the offer.
            let offer = path.split('/').nth(5).unwrap_or("0");
            Some(format!("{}/offer-{offer}", h.id.as_str()))
        }
        EffectRootOwner::OnKill(f) => Some(f.id().to_string()),
        EffectRootOwner::ObjectiveComplete { .. }
        | EffectRootOwner::QuestComplete { .. }
        | EffectRootOwner::DialogueRespawn
        | EffectRootOwner::OnDeath
        | EffectRootOwner::AssemblyLand(_)
        | EffectRootOwner::LoopCross(_) => None,
    }
}

/// The act a trap's or timed gate's disarm is.
#[must_use]
pub fn disarm_act(id: &str) -> String {
    format!("{id}/disarm")
}

/// Every flag producer of `c` — the table this module's note describes.
#[must_use]
pub fn flag_producers(c: &Campaign) -> Vec<FlagProducer<'_>> {
    let mut out = Vec::new();
    let gate_of = |fs: &[FlagId]| -> Vec<String> { fs.iter().map(|f| f.as_str().into()).collect() };
    for_each_effect_root(c, &mut |site, effs| {
        let act = act_of_owner(&site.owner, &site.path);
        let mut story_only = false;
        let (gate, at): (Vec<String>, ProducerSite<'_>) = match site.owner {
            EffectRootOwner::ObjectiveComplete { quest, objective } => (
                Vec::new(),
                ProducerSite::ObjectiveComplete {
                    quest: quest.id.as_str(),
                    objective,
                },
            ),
            EffectRootOwner::QuestComplete { quest } => (
                Vec::new(),
                ProducerSite::QuestComplete {
                    quest: quest.id.as_str(),
                },
            ),
            EffectRootOwner::Trigger(t) => (
                gate_of(&t.requires_flags),
                ProducerSite::Ambient {
                    at: t.at.as_ref(),
                    trigger: Some(t.id.as_str()),
                },
            ),
            EffectRootOwner::TrapPayload(trap) => (
                gate_of(&trap.requires_flags),
                ProducerSite::Ambient {
                    at: Some(&trap.at),
                    trigger: None,
                },
            ),
            // `DW0373` proves the far-side `unlock` walkable while the gate is
            // still sealed, so the party can always go and pull it; it declares
            // no flag gate.
            EffectRootOwner::ShortcutUnlock(s) => (
                Vec::new(),
                ProducerSite::Ambient {
                    at: Some(&s.unlock),
                    trigger: None,
                },
            ),
            // The offer's numeric half is a runtime balance no static flag model
            // can date; crediting it behind its flags only is the conservative
            // direction, because the buyer must still reach the shop.
            EffectRootOwner::ShopOffer(shop) => (
                Vec::new(),
                ProducerSite::Ambient {
                    at: Some(&shop.anchor),
                    trigger: None,
                },
            ),
            // A fight's `on_kill` fires only when a player is CREDITED with a
            // body, and a body may fall, burn or be cut down by another mob with
            // nobody credited, so no kill is forced. A story world that performs
            // the kill credits it (spec-0108 §4.1): the rows are story-only.
            EffectRootOwner::OnKill(_) => {
                story_only = true;
                (
                    Vec::new(),
                    ProducerSite::Ambient {
                        at: None,
                        trigger: None,
                    },
                )
            }
            // Reaction bundles fire when somebody dies, stands where a blow
            // lands, or crosses a slab a mob crosses as readily: no ordered walk
            // dates them, so nothing inside is a producer the mainline may rely
            // on.
            EffectRootOwner::DialogueRespawn
            | EffectRootOwner::OnDeath
            | EffectRootOwner::AssemblyLand(_)
            | EffectRootOwner::LoopCross(_) => return,
        };
        let row = Row {
            site: at,
            act: act.as_deref(),
            story_only,
        };
        collect(effs, &gate, row, &mut out);
    });
    // `disarm.sets_flag` is a field, not an effect list, so it has no root.
    for trap in &c.quests.content.traps {
        if let Some(d) = &trap.disarm {
            out.push(FlagProducer {
                flag: d.sets_flag.as_str(),
                requires: gate_of(&trap.requires_flags),
                site: ProducerSite::Ambient {
                    at: Some(&d.via),
                    trigger: None,
                },
                act: Some(disarm_act(trap.id.as_str())),
                story_only: false,
            });
        }
    }
    for g in &c.quests.content.timed_gates {
        if let Some(d) = &g.disarm {
            out.push(FlagProducer {
                flag: d.sets_flag.as_str(),
                requires: Vec::new(),
                site: ProducerSite::Ambient {
                    at: Some(&d.via),
                    trigger: None,
                },
                act: Some(disarm_act(g.id.as_str())),
                story_only: false,
            });
        }
    }
    out
}

/// What every row one root's bundle produces shares: its site, its act and
/// whether only a story world credits it.
#[derive(Clone, Copy)]
struct Row<'a, 'b> {
    site: ProducerSite<'a>,
    act: Option<&'b str>,
    story_only: bool,
}

/// Every `set-flag` in `effs`, carrying the conjunction of `gate` and the flag
/// gates of the enclosing chain. Reaction bundles nested inside an effect
/// (`set-checkpoint` / `bonfire` `on_respawn`, `begin-stealth` `on_caught`) are
/// not descended.
fn collect<'a>(
    effs: &'a [QuestEffect],
    gate: &[String],
    row: Row<'a, '_>,
    out: &mut Vec<FlagProducer<'a>>,
) {
    for e in effs {
        let mut here: Vec<String> = gate.to_vec();
        here.extend(e.requires_flags().iter().map(|f| f.as_str().to_string()));
        here.sort();
        here.dedup();
        if let Verb::SetFlag { flag, .. } = &e.verb {
            out.push(FlagProducer {
                flag: flag.as_str(),
                requires: here.clone(),
                site: row.site,
                act: row.act.map(str::to_string),
                story_only: row.story_only,
            });
        }
        match &e.verb {
            Verb::SetCheckpoint { .. } | Verb::Bonfire { .. } | Verb::BeginStealth { .. } => {}
            _ => {
                for list in e.nested_effect_lists() {
                    collect(list, &here, row, out);
                }
            }
        }
    }
}
