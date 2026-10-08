//! Stage 3 — classes: the kits a party chooses from, and the item grants and
//! potion contents a kit or a `give-item` carries.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ClassId;
use crate::serde_fields::is_false;

#[cfg(doc)]
use crate::Verb;

/// Who a granted item goes to (DSL v0.6, spec-0018).
///
/// Progression is a fact about the party, so the default for every `give-item` and
/// every class-kit entry is **all**: a quest beat that arms the party arms all of
/// it. `one` is the deliberate exception for a single quest prop (the wine-skin,
/// the stake): exactly one copy enters the party, handed to the player whose action
/// earned it, and the party passes it around physically.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Carrier {
    /// Every party member receives the item (the default).
    #[default]
    All,
    /// Exactly one copy, to the player whose action fired the effect.
    One,
}

/// Stage 3 payload: 1..4 selectable classes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClassesContent {
    /// The selectable classes.
    pub classes: Vec<Class>,
}

/// A player class with a starting kit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Class {
    /// Unique class id.
    pub id: ClassId,
    /// Player-facing name.
    pub name: String,
    /// Selection-screen blurb.
    pub blurb: String,
    /// Granted items.
    pub kit: Vec<KitItem>,
}

/// One item in a class kit.
///
/// Note: `lore`, `enchantments` and `attributes` are reserved for M2/M3
/// (spec-0001). They are intentionally *not* defined as fields in v0, so a
/// document using them is rejected as an unknown field (`DW0100`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KitItem {
    /// Vanilla item id, validated against the pinned 1.21.11 registry.
    pub item: String,
    /// Stack count.
    pub count: u32,
    /// Optional display name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Who gets it (DSL v0.6, spec-0018). Absent = [`Carrier::All`]. A class kit is
    /// per-player gear by construction — every player who picks the class gets the
    /// kit — so `carrier` here marks a **party-unique** kit item: exactly one copy
    /// enters the party, given to the first player to pick this class.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub carrier: Option<Carrier>,
    /// **The flask** (DSL v0.8, spec-0016 §1): this kit
    /// entry is the class's recovery item, and resting at a bonfire replenishes
    /// it to exactly `count`. A campaign that places a `bonfire` and declares no
    /// flask anywhere in its kits is `DW0476` — the estus loop is what makes
    /// dying an investment, so a souls campaign without one is a build error, not
    /// a design choice. Absent on every pre-0.8 kit → emission byte-identical.
    #[serde(default, skip_serializing_if = "is_false")]
    pub flask: bool,
    /// **What is in the bottle** (DSL v0.8, spec-0016 §1): the vanilla
    /// `minecraft:potion_contents` component of a
    /// potion-bearing item ([`POTION_BEARING_ITEMS`]). Without it a
    /// `minecraft:potion` is the *Uncraftable Potion* — a bottle that heals
    /// nothing — which is exactly the placeholder flask this field exists to
    /// abolish, so at `dsl_version` 0.8.0 a potion-bearing kit item that declares
    /// no `contents` is `DW0487`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contents: Option<PotionContents>,
}

/// The items whose vanilla item definition carries a `minecraft:potion_contents`
/// component — the only items a kit `contents` may be declared on (`DW0486`).
///
/// Read off the pinned 1.21.11 `item_components` summary (SHA-256
/// `51b191e13f86813ca02f1498942e5bc235947edb71eb8105a78401670b3665c4`, the same
/// misode/mcmeta ref `crates/delvec/data/PROVENANCE.md` pins): exactly these
/// four items declare the component, and on any other item the game drops the
/// data on the floor.
pub const POTION_BEARING_ITEMS: &[&str] = &[
    "minecraft:lingering_potion",
    "minecraft:potion",
    "minecraft:splash_potion",
    "minecraft:tipped_arrow",
];

/// True if `item_id` (optionally un-namespaced) is one of the four
/// [`POTION_BEARING_ITEMS`].
pub fn is_potion_bearing_item(item_id: &str) -> bool {
    let norm = if item_id.contains(':') {
        item_id.to_string()
    } else {
        format!("minecraft:{item_id}")
    };
    POTION_BEARING_ITEMS.contains(&norm.as_str())
}

/// The two **instantaneous** status effects: they are applied once, on the tick
/// the potion is drunk (`PotionContents.applyToLivingEntity` branches on
/// `isInstantenous` before any effect instance is ever added), so a `duration` on
/// one is a statement the game never reads — `DW0486` says so rather than letting
/// an author believe they wrote a thirty-second heal.
pub const INSTANT_EFFECTS: &[&str] = &["minecraft:instant_health", "minecraft:instant_damage"];

/// A potion-bearing kit item's `minecraft:potion_contents` component (DSL v0.8),
/// modelled field for field on vanilla rather than invented: a **named** potion,
/// a list of **custom effects**, or both, plus the bottle-colour override.
///
/// Vanilla resolves a drink as the named potion's effects followed by the custom
/// ones, and derives the bottle colour from those effects unless `color`
/// overrides it — so `{"potion": "minecraft:strong_healing"}` is literally the
/// Potion of Healing II a player would brew, not an approximation of one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PotionContents {
    /// A named vanilla potion id (`minecraft:strong_healing`,
    /// `minecraft:long_night_vision`, …), validated against the pinned 1.21.11
    /// `potion` registry (`DW0486`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub potion: Option<String>,
    /// Custom effects applied on top of (or instead of) the named potion — the
    /// escape hatch for a recovery item vanilla has no brew for.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<PotionEffect>,
    /// Bottle-colour override, `#rrggbb` (vanilla `custom_color`). Absent → the
    /// colour vanilla derives from the effects themselves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

/// One entry of a potion's `custom_effects` list.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PotionEffect {
    /// Vanilla status-effect id (e.g. `minecraft:regeneration`), validated
    /// against the pinned registry (`DW0486`).
    pub effect: String,
    /// How long it lasts, in **ticks** (20 = one second). Required for every
    /// effect except the two [`INSTANT_EFFECTS`], which must NOT declare one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<u32>,
    /// Amplifier, 0 = level I (vanilla's unsigned byte, so 0–255). Absent = 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amplifier: Option<u32>,
}

impl PotionEffect {
    /// True if this effect is applied once on drinking rather than over time.
    pub fn is_instant(&self) -> bool {
        let norm = crate::registry::namespaced_effect_id(&self.effect);
        INSTANT_EFFECTS.contains(&norm.as_str())
    }
}

/// The largest `duration` a potion effect may declare, in ticks: 1 000 000 ticks
/// ≈ 13.9 hours, past the 10-hour delve ceiling, so nothing a delve can legally
/// need is refused — while a duration typed in *milliseconds*, or one that would
/// overflow vanilla's int, is caught (`DW0486`).
pub const MAX_POTION_DURATION_TICKS: u32 = 1_000_000;

/// The largest `amplifier` a potion effect may declare: vanilla stores it in an
/// unsigned byte, so 255 is not a policy but the end of the field.
pub const MAX_POTION_AMPLIFIER: u32 = 255;

/// The largest `seconds` a [`Verb::GiveEffect`] may declare, derived from
/// [`MAX_POTION_DURATION_TICKS`] rather than chosen again: the two are the same
/// quantity in different units, and a second independently-picked ceiling is how
/// two limits for one fact drift apart. ≈13.9 hours, past the 10-hour delve
/// ceiling, so nothing a delve can legally need is refused — while a duration
/// typed in *ticks* or in milliseconds is caught (`DW0541`).
pub const MAX_EFFECT_SECONDS: u32 = MAX_POTION_DURATION_TICKS / 20;

// ---------------------------------------------------------------------------
// Validation — the checks `dsl::validate` runs over this object (ADR-0031)
// ---------------------------------------------------------------------------

use std::collections::BTreeSet;

use crate::Objective;
use crate::diagnostic::{Diagnostic, DwCode, ExitTier, codes};
use crate::envelope::Campaign;
use crate::registry::EffectRegistry;

crate::dw_code! {
    /// (spec-0016 §1) The campaign places a `bonfire`
    /// but no class kit declares a `flask`. Resting replenishes the flask to its
    /// declared count; with no flask the rest interaction's whole recovery half
    /// is a no-op and the souls loop has no consumable to spend, so this is a
    /// build error rather than a design choice.
    pub const BONFIRE_NO_FLASK: DwCode = DwCode::new("DW0476", ExitTier::Build);
}

crate::dw_code! {
    /// **An item gate a class cannot bring.** An objective completes only for a
    /// player holding a named item, and some class's player has no way to be
    /// holding it: the item's only source in the whole campaign is *another*
    /// class's kit, or it has no source at all.
    ///
    /// A delve is played by one to four players who each pick one class, so an
    /// objective reachable only by one class's pick is an objective a party can
    /// be assembled unable to finish — and the party finds out at the thing they
    /// cannot press. Quantified over EVERY class for the same reason
    /// [`BONFIRE_NO_FLASK`] is: one class that cannot bring it is as broken as
    /// none, because a solo player of that class is a supported party.
    pub const ITEM_GATE_UNBRINGABLE: DwCode = DwCode::new("DW0849", ExitTier::Build);
}

crate::dw_code! {
    /// (spec-0016 §1) A kit item's potion `contents`
    /// is not something 1.21.11 can pour: declared on an item that carries no
    /// `minecraft:potion_contents` component, empty (neither a named potion nor
    /// an effect), an unknown potion or status-effect id, an amplifier or
    /// duration outside the field vanilla stores it in, a lasting effect with no
    /// `duration`, an instantaneous one *with* a duration, or a malformed
    /// `color`.
    pub const KIT_POTION_INVALID: DwCode = DwCode::new("DW0486", ExitTier::Build);
}

crate::dw_code! {
    /// (spec-0016 §1) A potion-bearing kit item
    /// declares no `contents` at `dsl_version` 0.8.0 — the Uncraftable Potion, a
    /// bottle that pours nothing. The placeholder flask, as a build error.
    pub const KIT_POTION_MISSING: DwCode = DwCode::new("DW0487", ExitTier::Build);
}

/// Normalise an authored item id to its namespaced form, so `stripped_oak_log`
/// and `minecraft:stripped_oak_log` are the same item to every comparison here.
/// Same rule [`crate::is_potion_bearing_item`] applies to its own list.
fn ns_item(id: &str) -> String {
    if id.contains(':') {
        id.to_string()
    } else {
        format!("minecraft:{id}")
    }
}

/// Every item this campaign can put into the hands of a player **whatever class
/// they picked** — the class-blind half of the provenance question `DW0849`
/// asks.
///
/// The five ways an item enters a player's inventory are the class kit
/// ([`crate::Class::kit`], which is class-BOUND and therefore
/// deliberately absent here) and these four. They are gathered from the closed
/// enumerations rather than from a walk of the sites this function's author
/// happened to remember: effects come through
/// [`crate::for_each_campaign_effect`], which is
/// [`crate::effects::for_each_effect_root`] underneath — the same eight roots
/// emission lowers from, and the one `tools/ci/check-effect-roots.py` holds closed.
///
/// A trap's `dispense` payload is **not** a source, and the exclusion is about
/// the object rather than about effort: a dispenser fires its stack at the party
/// as a hazard. Being shot with a thing is not being handed it, and a campaign
/// whose only supply of a required item is a trap firing it has a defect this
/// check should name rather than excuse.
fn class_blind_item_sources(c: &Campaign) -> BTreeSet<String> {
    let mut src: BTreeSet<String> = BTreeSet::new();
    let quests = &c.quests.content;

    // A `give-item` anywhere. Deliberately unconditional on its flag gate and on
    // its position in the quest DAG: a gated grant is still a way the item can
    // be had, and treating one as no source at all would red campaigns that are
    // fine. The direction of the approximation is chosen — this check refuses
    // only where NOTHING class-blind supplies the item.
    crate::for_each_campaign_effect(c, &mut |_path, _site, eff| {
        if let Some(item) = eff.give_item() {
            src.insert(ns_item(item));
        }
    });

    for q in &quests.quests {
        for o in &q.objectives {
            // A `collect` is provisioned into a container the compiler fills or
            // adopts, or dropped by a wave — every one of those is open to
            // whoever walks up to it.
            if let Objective::Collect { item, .. } = o {
                src.insert(ns_item(item));
            }
        }
    }

    for l in &quests.loot {
        for it in &l.items {
            src.insert(ns_item(&it.item));
        }
    }

    for w in &quests.waves {
        for m in &w.mobs {
            for drop in &m.drops {
                match (drop.item(), drop.slot()) {
                    (Some(item), _) => {
                        src.insert(ns_item(item));
                    }
                    // A worn piece drops the item the same mob's `equipment`
                    // declares in that slot (`DW0490` already refuses a slot the
                    // equipment leaves empty, so this lookup is total on a
                    // campaign that got that far).
                    (None, Some(slot)) => {
                        if let Some(eq) = m.equipment.as_ref().and_then(|e| e.filled(slot)) {
                            src.insert(ns_item(eq.item()));
                        }
                    }
                    (None, None) => {}
                }
            }
        }
    }

    src
}

/// `DW0849`: **an item gate a class cannot bring.**
///
/// ## The finding this is the general form of
///
/// A required item was issued through one class's kit rather than to the party,
/// so a player who picked any other class arrived at the objective that consumed
/// it and could do nothing. The instance was repaired by moving the item; the
/// class of defect — *completability that depends on which class was picked* —
/// had no check, and a campaign is free to reintroduce it at every new item
/// gate.
///
/// ## Why this is a property of the object class, not of `interact`
///
/// The object is an **item gate**: a place where an objective completes only for
/// a player who holds a named thing. Today the DSL has exactly one such site
/// ([`Objective::Interact::requires_item`]) — a shop's price is a
/// [`crate::StateCompare`] over a datum and not an item at all, and no
/// verb removes an item from an inventory. So the enumeration is one arm wide
/// today and is written as an enumeration anyway, because the second site is
/// where a rule keyed to the first verb leaves the next author with no surface.
///
/// ## The quantifier, and why it is `for all` rather than `there exists`
///
/// A delve is played by one to four players who each pick one class, so **a solo
/// player of any class is a supported party**. An item only one class can bring
/// is therefore an objective some real party is assembled unable to finish, and
/// it finds out at the thing it cannot press. This is exactly the reasoning
/// [`BONFIRE_NO_FLASK`] already states for the flask: one class without it
/// is as broken as none.
///
/// ## The direction the approximation runs
///
/// [`class_blind_item_sources`] is deliberately generous — a flag-gated
/// `give-item` late in the DAG counts as a source. The refusal therefore fires
/// only where the item has **no** class-blind supply anywhere in the campaign,
/// which is the shape the finding had and the shape a typo has. Making it
/// stricter would need the reachability model, which does not model items at
/// all; making it stricter *without* that model would red correct campaigns,
/// and a check that reds correct work is how a check gets weakened.
pub(crate) fn item_gate_class_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let classes = &c.classes.content.classes;
    if classes.is_empty() {
        // The schema requires 1..4, so this is unreachable on a parsed campaign;
        // returning rather than dividing by an empty quantifier keeps the "for
        // all classes" reading honest instead of vacuously true.
        return;
    }
    let blind = class_blind_item_sources(c);

    for (i, q) in c.quests.content.quests.iter().enumerate() {
        for (j, o) in q.objectives.iter().enumerate() {
            let Objective::Interact {
                id, requires_item, ..
            } = o
            else {
                continue;
            };
            let Some(raw) = requires_item.as_deref() else {
                continue;
            };
            let item = ns_item(raw);
            if blind.contains(&item) {
                continue;
            }
            let cannot: Vec<&str> = classes
                .iter()
                .filter(|cl| !cl.kit.iter().any(|k| ns_item(&k.item) == item))
                .map(|cl| cl.id.as_str())
                .collect();
            if cannot.is_empty() {
                continue;
            }
            let supply = if cannot.len() == classes.len() {
                "nothing in this campaign supplies it at all".to_string()
            } else {
                format!(
                    "its only supply is another class's kit, so {} cannot bring it: {}",
                    if cannot.len() == 1 {
                        "one class"
                    } else {
                        "those classes"
                    },
                    cannot.join(", ")
                )
            };
            d.push(Diagnostic::error(
                ITEM_GATE_UNBRINGABLE,
                "quests",
                format!("/content/quests/{i}/objectives/{j}/requires_item"),
                format!(
                    "objective `{}` completes only for a player HOLDING `{raw}`, and {supply}. A \
                     delve is played by one to four players who each pick one class, so a solo \
                     player of any class is a party this campaign must be finishable by — and \
                     this one is assembled unable to finish, which it learns standing at the \
                     thing it cannot press. Three ways to supply it, and any one is enough: put \
                     the item in a `collect` objective or a `loot` container on the way to this \
                     gate; hand it out with a `give-item` effect (its default `carrier` is `all` \
                     — every party member); or add it to EVERY class kit rather than one. Do not \
                     drop `requires_item` to silence this — presenting the item is the beat.",
                    id.as_str()
                ),
            ));
        }
    }
}

/// spec-0016 §1: **what is actually in the flask.**
///
/// The kit `flask` marker landed with no way to say what the bottle pours, so
/// every flask shipped as `minecraft:potion` with no `minecraft:potion_contents`
/// component — the Uncraftable Potion, which a player can drink all day for
/// nothing. `contents` closes that, and these are the two halves of keeping it
/// honest: `DW0487` refuses the placeholder (a potion-bearing kit item that
/// declares no contents), `DW0486` refuses contents 1.21.11 cannot pour.
///
pub(crate) fn kit_potion_checks(
    c: &Campaign,
    effects: &dyn EffectRegistry,
    d: &mut Vec<Diagnostic>,
) {
    for (i, cl) in c.classes.content.classes.iter().enumerate() {
        for (k, item) in cl.kit.iter().enumerate() {
            let bearing = crate::is_potion_bearing_item(&item.item);
            let path = format!("/content/classes/{i}/kit/{k}");
            let Some(contents) = &item.contents else {
                // The placeholder flask, as a build error.
                if bearing {
                    d.push(Diagnostic::error(
                        KIT_POTION_MISSING,
                        "classes",
                        format!("{path}/contents"),
                        format!(
                            "kit item `{}` declares no `contents`, so it compiles to the \
                             *Uncraftable Potion* — a bottle with no `minecraft:potion_contents` \
                             component, which grants nothing when drunk however it is named. \
                             Declare what is in it: `\"contents\": {{\"potion\": \
                             \"minecraft:strong_healing\"}}`, or an `\"effects\"` list of \
                             `{{\"effect\", \"duration\", \"amplifier\"}}`. Do NOT rename the \
                             bottle instead — semantics never key on player-facing text \
                             (spec-0016 §1).",
                            item.item
                        ),
                    ));
                }
                continue;
            };
            // `contents` on an item with no such component: the data would be
            // dropped on the floor, silently.
            if !bearing {
                d.push(Diagnostic::error(
                    KIT_POTION_INVALID,
                    "classes",
                    format!("{path}/contents"),
                    format!(
                        "kit item `{}` cannot carry potion `contents` — in 1.21.11 only \
                         `minecraft:potion`, `minecraft:splash_potion`, \
                         `minecraft:lingering_potion` and `minecraft:tipped_arrow` carry a \
                         `minecraft:potion_contents` component, and on anything else the game \
                         discards it. Put the contents on a potion item, or drop the field.",
                        item.item
                    ),
                ));
                continue;
            }
            if contents.potion.is_none() && contents.effects.is_empty() {
                d.push(Diagnostic::error(
                    KIT_POTION_INVALID,
                    "classes",
                    format!("{path}/contents"),
                    "empty potion `contents` — it names no `potion` and lists no `effects`, so \
                     the bottle still pours nothing. Name a vanilla potion (e.g. \
                     `\"potion\": \"minecraft:strong_healing\"`) or list at least one effect."
                        .to_string(),
                ));
            }
            if let Some(p) = &contents.potion
                && !crate::registry::is_potion_id(p)
            {
                d.push(Diagnostic::error(
                    KIT_POTION_INVALID,
                    "classes",
                    format!("{path}/contents/potion"),
                    format!(
                        "`{p}` is not in the pinned 1.21.11 `potion` registry — use a real potion \
                         id (`minecraft:healing`, `minecraft:strong_healing`, \
                         `minecraft:long_night_vision`, …). Note the 1.20.5+ spelling: strength \
                         and duration are part of the id (`strong_`/`long_` prefixes), not \
                         separate fields."
                    ),
                ));
            }
            if let Some(col) = &contents.color
                && !is_hex_color(col)
            {
                d.push(Diagnostic::error(
                    KIT_POTION_INVALID,
                    "classes",
                    format!("{path}/contents/color"),
                    format!(
                        "potion `color` `{col}` is malformed — write the bottle colour as \
                         `#rrggbb` (e.g. `#ff9c30`), or omit it and take the colour vanilla \
                         derives from the effects."
                    ),
                ));
            }
            for (e, eff) in contents.effects.iter().enumerate() {
                let epath = format!("{path}/contents/effects/{e}");
                if !effects.contains(&eff.effect) {
                    d.push(Diagnostic::error(
                        KIT_POTION_INVALID,
                        "classes",
                        format!("{epath}/effect"),
                        format!(
                            "potion effect `{}` is not a known 1.21.11 status-effect id — use a \
                             valid namespaced effect id (e.g. `minecraft:instant_health`).",
                            eff.effect
                        ),
                    ));
                }
                if let Some(amp) = eff.amplifier
                    && amp > crate::MAX_POTION_AMPLIFIER
                {
                    d.push(Diagnostic::error(
                        KIT_POTION_INVALID,
                        "classes",
                        format!("{epath}/amplifier"),
                        format!(
                            "potion effect `amplifier` {amp} is out of range — vanilla stores it \
                             in an unsigned byte, so it must be 0–{max} (0 = level I).",
                            max = crate::MAX_POTION_AMPLIFIER
                        ),
                    ));
                }
                match (eff.is_instant(), eff.duration) {
                    // An instantaneous effect is applied once on drinking; a
                    // duration on it is a sentence the game never reads.
                    (true, Some(dur)) => d.push(Diagnostic::error(
                        KIT_POTION_INVALID,
                        "classes",
                        format!("{epath}/duration"),
                        format!(
                            "`{}` is instantaneous — it lands once, on the tick the potion is \
                             drunk, so the `duration` of {dur} tick(s) here is never read. Drop \
                             the field; for healing that ticks over time use \
                             `minecraft:regeneration`, which does take a duration.",
                            eff.effect
                        ),
                    )),
                    (false, None) => d.push(Diagnostic::error(
                        KIT_POTION_INVALID,
                        "classes",
                        format!("{epath}/duration"),
                        format!(
                            "potion effect `{}` lasts over time and declares no `duration` — \
                             vanilla would default it to zero ticks, i.e. nothing. Declare the \
                             duration in ticks (20 = one second).",
                            eff.effect
                        ),
                    )),
                    (false, Some(dur)) if dur == 0 || dur > crate::MAX_POTION_DURATION_TICKS => {
                        d.push(Diagnostic::error(
                            KIT_POTION_INVALID,
                            "classes",
                            format!("{epath}/duration"),
                            format!(
                                "potion effect `duration` {dur} is out of range — it is in \
                                 **ticks** (20 = one second) and must be 1–{max} \
                                 (≈13.9 hours, past the delve ceiling).",
                                max = crate::MAX_POTION_DURATION_TICKS
                            ),
                        ));
                    }
                    _ => {}
                }
            }
        }
    }
}

/// True if `s` is a `#rrggbb` colour literal — [`crate::color::is_hex`], the one
/// rule every hex-colour surface reads.
fn is_hex_color(s: &str) -> bool {
    crate::color::is_hex(s)
}

/// Every kit item is in the pinned item registry (`DW0143`).
pub(crate) fn kit_item_checks(
    c: &Campaign,
    items: &dyn crate::registry::ItemRegistry,
    d: &mut Vec<Diagnostic>,
) {
    // Kit items.
    for (i, cl) in c.classes.content.classes.iter().enumerate() {
        for (j, it) in cl.kit.iter().enumerate() {
            if !items.contains(&it.item) {
                d.push(Diagnostic::error(
                    codes::ITEM_UNKNOWN,
                    "classes",
                    format!("/content/classes/{i}/kit/{j}/item"),
                    format!(
                        "kit item `{}` is not in the pinned 1.21.11 item registry — use a valid \
                         namespaced item id (e.g. `minecraft:iron_sword`)",
                        it.item
                    ),
                ));
            }
        }
    }
}

/// spec-0016 §1: a campaign that places a bonfire owes every class a flask
/// (`DW0476`). `has_bonfire` is [`crate::declares_bonfire`], read once by the
/// caller.
pub(crate) fn bonfire_flask_checks(c: &Campaign, has_bonfire: bool, d: &mut Vec<Diagnostic>) {
    // spec-0016 §1: a campaign that places a bonfire is
    // a souls campaign, and a souls campaign owes the party a flask. Resting
    // replenishes every `flask` kit entry to its declared count — with none
    // declared, "rest and save" and "save only" collapse into the same button and
    // the recovery economy the bonfire exists to serve does not exist (`DW0476`).
    // Campaign-global on purpose: the flask is per-class gear, and one class
    // without a flask is as broken as none, so the requirement is on EVERY class.
    if has_bonfire {
        let flaskless: Vec<&str> = c
            .classes
            .content
            .classes
            .iter()
            .filter(|cl| !cl.kit.iter().any(|k| k.flask))
            .map(|cl| cl.id.as_str())
            .collect();
        if !flaskless.is_empty() {
            d.push(Diagnostic::error(
                BONFIRE_NO_FLASK,
                "classes",
                "/content/classes".to_string(),
                format!(
                    "this campaign places a `bonfire` but {} no `flask` kit item: {}. \
                     Resting at a bonfire replenishes every kit entry marked `\"flask\": true` to \
                     its declared `count` — with none, the rest option recovers nothing and the \
                     souls loop has no consumable to spend (spec-0016 §1). \
                     Add a recovery item to each class kit and mark it \
                     `\"flask\": true` (this needs `dsl_version` 0.8.0 on the classes stage). Do \
                     NOT drop the bonfire to silence this — the rest point is the design.",
                    if flaskless.len() == 1 {
                        "one class declares".to_string()
                    } else {
                        format!("{} classes declare", flaskless.len())
                    },
                    flaskless.join(", ")
                ),
            ));
        }
    }
}

/// `DW0110` over the class ids.
pub(crate) fn class_id_syntax(c: &Campaign, d: &mut Vec<Diagnostic>) {
    for (i, cl) in c.classes.content.classes.iter().enumerate() {
        crate::ids::id_syntax!(d, cl.id, "classes", format!("/content/classes/{i}/id"));
    }
}

/// `DW0111` over the class ids.
pub(crate) fn class_id_uniqueness(c: &Campaign, d: &mut Vec<Diagnostic>) {
    crate::ids::dup_check(
        c.classes
            .content
            .classes
            .iter()
            .enumerate()
            .map(|(i, cl)| (cl.id.as_str(), format!("/content/classes/{i}/id"))),
        "classes",
        "class",
        d,
    );
}
