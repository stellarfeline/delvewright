//! Loot containers and what they hold.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{AnchorId, LootId};

#[cfg(doc)]
use crate::enchantment_component;

/// A stage-5 holder fill (DSL v0.6, spec-0021; holders spec-0111): contents
/// for an item holder the prefab already placed — any of the twelve container
/// kinds of the pinned game (spec-0105): a chest, trapped chest, barrel or
/// shulker box (27 slots), a hopper or brewing stand (5), a dispenser, dropper
/// or crafter (9), a furnace, blast furnace or smoker (3: input, fuel, result)
/// — or an item frame (one slot, `contents`) or an armour stand (`armor.head`,
/// `armor.chest`, `armor.legs`, `armor.feet`, `weapon.mainhand`,
/// `weapon.offhand`, each named by the stack's `slot`).
///
/// The holder is **hardware the prefab authored**, exactly like a trap's
/// dispenser: this declaration gives an already-placed, already-lit, already
/// composed piece of furniture its contents. The compiler never places the
/// holder itself — if the anchor's cell does not already hold one, that is a
/// content defect and a build error (`DW0431`), not something to paper over by
/// setblock-ing a chest into a wall.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Loot {
    /// Unique loot id (`loot/<kebab>`).
    pub id: LootId,
    /// The anchor whose cell holds the holder to fill.
    pub anchor: AnchorId,
    /// Contents, in declaration order. A stack with no `slot` lands
    /// positionally — the first entry in `container.0` (or a frame's
    /// `contents`), the second in `container.1`, and so on (ADR-0006: no RNG,
    /// no loot tables); a stack that names its `slot` lands there.
    pub items: Vec<LootItem>,
}

/// One stack inside a [`Loot`] container.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LootItem {
    /// Item id (e.g. `minecraft:cooked_cod`). Validated against the pinned
    /// 1.21.11 item registry (`DW0143`).
    pub item: String,
    /// Stack size. Defaults to 1.
    #[serde(default = "one_u32")]
    pub count: u32,
    /// Optional custom item name. Enters the l10n string inventory exactly like
    /// a class kit item's name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Enchantments on this stack (`{"minecraft:sharpness": 3}`), emitted as the
    /// 1.21 `minecraft:enchantments` item component — or, on a
    /// `minecraft:enchanted_book`, `minecraft:stored_enchantments`
    /// ([`enchantment_component`]).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub enchantments: BTreeMap<String, u32>,
    /// **The slot this stack lands in** (DSL v0.49, spec-0111 §3.3), in the
    /// holder family's vocabulary — which is also vanilla's `item replace`
    /// spelling: `container.<n>` on a container, `contents` on an item frame,
    /// `armor.head`, `armor.chest`, `armor.legs`, `armor.feet`,
    /// `weapon.mainhand` or `weapon.offhand` on an armour stand. Omitted, the
    /// stack lands positionally (a container, a frame); a stand's stacks each
    /// name their slot, because which hand or head a thing sits in is the
    /// author's judgement. A fill names every slot or none (`DW1035`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<String>,
}

/// **The container class's home in stage 5** (DSL v0.43, spec-0105).
///
/// Every container cell of the built world is loot (a fill stands in it), a
/// device (a trap, trigger or objective acts on it) or scenery. Declaring
/// [`Containers::decoration_notice`] is the declaration that every container
/// no fill and no device claims is scenery, and that this line tells the
/// player so — once, at arrival, before any container can be reached. What it
/// covers is derived by the build's census and printed; it is never typed.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Containers {
    /// The line that tells each player, at arrival, what the containers no
    /// fill claims hold — the one `narrate` payload ([`crate::Narration`]),
    /// emitted in every class's apply function after the warp in, so its sound
    /// plays where the player arrives. Inventoried for translation as
    /// `containers.decoration_notice`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decoration_notice: Option<crate::Narration>,
}

impl Containers {
    /// Whether the object declares nothing (`DW1019`).
    pub fn is_empty(&self) -> bool {
        self.decoration_notice.is_none()
    }
}

fn one_u32() -> u32 {
    1
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

use std::collections::BTreeSet;

use crate::diagnostic::{Diagnostic, DwCode, ExitTier, codes};
use crate::envelope::Campaign;
use crate::registry::{AnchorRegistry, ItemRegistry};
use crate::validate::{AnchorProviders, station_kind_diag};
use crate::wave::check_enchantments;

crate::dw_code! {
    /// (spec-0021) Two `loot` entries target the same anchor, so one would
    /// silently overwrite the other's contents.
    pub const LOOT_DUPLICATE_ANCHOR: DwCode = DwCode::new("DW0435", ExitTier::Build);
}

crate::dw_code! {
    /// A single-slot fill's `count` exceeds the item's `minecraft:max_stack_size`
    /// in the pinned 1.21.11 registry. `item replace … container.<n> with <item>
    /// <count>` fails **silently** above the cap, shipping an empty slot.
    pub const ITEM_COUNT_OVER_STACK: DwCode = DwCode::new("DW0436", ExitTier::Build);
}

crate::dw_code! {
    /// (spec-0111 §6.1) A fill names a slot the holder class does not have, or
    /// a stack a slot cannot wear: a `slot` outside every family's vocabulary,
    /// a slot named twice in one fill, a fill mixing named and unnamed stacks,
    /// an armour slot given a stack whose `equippable` slot is another; at the
    /// build tier, where the family is known, a stand fill with an unnamed
    /// stack and a `collect` padding a frame or a stand.
    pub const DW_HOLDER_SLOT: DwCode = DwCode::new("DW1035", ExitTier::Build);
}

/// The vocabulary of every holder family and which slots take any stack — the
/// shared tail of every `DW1035` message (spec-0111 §6.1).
pub fn holder_vocabulary_text() -> String {
    use crate::holders::HolderFamily;
    format!(
        "The holder class's slot vocabulary, per family: a container — {}; an item frame — {};          an armour stand — {}. Any stack goes in `container.<n>`, `contents`, `armor.head`, \
         `weapon.mainhand` and `weapon.offhand`; `armor.chest`, `armor.legs` and `armor.feet` \
         take only a stack whose `equippable` slot is that slot (the game refuses anything \
         else there).",
        HolderFamily::Container.vocabulary(),
        HolderFamily::Frame.vocabulary(),
        HolderFamily::Stand.vocabulary(),
    )
}

/// `DW1035`'s document arm over one fill's stacks (spec-0111 §3.3): every slot
/// is in some family's vocabulary, none is named twice, the stacks name every
/// slot or none, and an armour slot holds only a stack the game puts there.
fn check_fill_slots(
    what: &str,
    path: &str,
    stacks: &[LootItem],
    items: &dyn ItemRegistry,
    d: &mut Vec<Diagnostic>,
) {
    let named = stacks.iter().filter(|it| it.slot.is_some()).count();
    if named > 0 && named < stacks.len() {
        d.push(Diagnostic::error(
            DW_HOLDER_SLOT,
            "quests",
            format!("{path}/items"),
            format!(
                "{what} names the slot of {named} of its {} stacks and leaves the rest \
                 positional, so where an unnamed stack lands depends on which slots the named \
                 ones took. Name the `slot` of every stack, or of none. {}",
                stacks.len(),
                holder_vocabulary_text()
            ),
        ));
    }
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    for (k, it) in stacks.iter().enumerate() {
        let Some(slot) = it.slot.as_deref() else {
            continue;
        };
        let at = format!("{path}/items/{k}/slot");
        if !check_stack_slot(what, &at, &it.item, slot, items, d) {
            continue;
        }
        if let Some(prev) = seen.insert(slot, k) {
            d.push(Diagnostic::error(
                DW_HOLDER_SLOT,
                "quests",
                at,
                format!(
                    "{what} names slot `{slot}` twice (stacks {prev} and {k}), so the second \
                     stack overwrites the first and `{}` never reaches the holder. Give each \
                     stack its own slot. {}",
                    stacks[prev].item,
                    holder_vocabulary_text()
                ),
            ));
        }
    }
}

/// `DW1035`'s document arm over ONE stack and the slot it names — the one
/// validation every fill of the holder class runs, a `loot` stack's and a
/// `collect`'s alike (spec-0111 §3.3, §3.4): the slot is in some family's
/// vocabulary, and an armour slot holds only a stack the game puts there.
/// Returns whether the slot is in the vocabulary (a slot outside it is judged
/// no further).
fn check_stack_slot(
    what: &str,
    at: &str,
    item: &str,
    slot: &str,
    items: &dyn ItemRegistry,
    d: &mut Vec<Diagnostic>,
) -> bool {
    if !crate::holders::in_some_vocabulary(slot) {
        d.push(Diagnostic::error(
            DW_HOLDER_SLOT,
            "quests",
            at.to_string(),
            format!(
                "{what} puts `{item}` in slot `{slot}`, which no holder has. {}",
                holder_vocabulary_text()
            ),
        ));
        return false;
    }
    if let Some(stand) = crate::holders::stand_slot(slot)
        && let crate::holders::SlotAccepts::Equippable(want) = stand.accepts
    {
        let fits = match items.equippable(item) {
            crate::equipment::EquippableFact::Unknown => true,
            crate::equipment::EquippableFact::Plain => false,
            crate::equipment::EquippableFact::Equippable(e) => {
                e.slot == want
                    && (e.allowed_entities.is_empty()
                        || crate::equipment::admitted("minecraft:armor_stand", &e.allowed_entities))
            }
        };
        if !fits {
            d.push(Diagnostic::error(
                DW_HOLDER_SLOT,
                "quests",
                at.to_string(),
                format!(
                    "{what} puts `{item}` in `{slot}`, but the pinned game puts a stack in \
                     `{slot}` only when its `equippable` slot is `{want}`, and `{item}`'s is not \
                     — `item replace entity … {slot}` would be refused and the stand would \
                     stand empty there. Put it in a slot that takes any stack, or choose an \
                     item made for that slot. {}",
                    holder_vocabulary_text()
                ),
            ));
        }
    }
    true
}

/// `DW1035` over every `collect`'s `slot` (spec-0111 §3.4): the slot passes the
/// one stack validation [`check_stack_slot`] a `loot` stack passes, and only a
/// collect that adopts a `holder` names one — the compiler's own chest and an
/// item that drops from a wave have no slot the author chooses.
pub(crate) fn collect_slot_checks(c: &Campaign, items: &dyn ItemRegistry, d: &mut Vec<Diagnostic>) {
    for (i, q) in c.quests.content.quests.iter().enumerate() {
        for (j, o) in q.objectives.iter().enumerate() {
            let crate::Objective::Collect {
                id,
                item,
                slot: Some(slot),
                holder,
                dropped_by,
                ..
            } = o
            else {
                continue;
            };
            let what = format!("collect objective `{id}`");
            let at = format!("/content/quests/{i}/objectives/{j}/slot");
            if holder.is_none() {
                let source = if dropped_by.is_some() {
                    "its item drops from a wave (`dropped_by`), so it lands on the ground"
                } else {
                    "it conjures the compiler's own chest at `anchor`, whose one slot the \
                     compiler fills"
                };
                d.push(Diagnostic::error(
                    DW_HOLDER_SLOT,
                    "quests",
                    at,
                    format!(
                        "{what} names slot `{slot}` but adopts no `holder`: {source}. A `slot` \
                         names where the item lands in the holder a collect adopts. Name the \
                         holder the piece places with `holder`, or delete `slot`. {}",
                        holder_vocabulary_text()
                    ),
                ));
                continue;
            }
            check_stack_slot(&what, &at, item, slot, items, d);
        }
    }
}

crate::dw_code! {
    /// (spec-0105 §6.2) A container declaration that holds nothing: a `loot`
    /// fill with no stack, a `containers` object that declares nothing, or a
    /// runtime `set-block` / `fill-region` that places a container — which
    /// stands empty for ever, because every fill runs at `setup_finish`.
    pub const CONTAINER_HOLDS_NOTHING: DwCode = DwCode::new("DW1019", ExitTier::Build);
}

/// `DW1019`: every declaration that would leave a container holding nothing
/// (spec-0105 §6.2). A container meant to hold nothing is scenery, and scenery
/// is declared once, by `containers.decoration_notice` — never by an empty
/// fill, and never by a container conjured at runtime.
pub(crate) fn container_declaration_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let quests = &c.quests.content;
    for (i, l) in quests.loot.iter().enumerate() {
        if l.items.is_empty() {
            d.push(Diagnostic::error(
                CONTAINER_HOLDS_NOTHING,
                "quests",
                format!("/content/loot/{i}/items"),
                format!(
                    "loot `{}` declares no stack, so the container at anchor `{}` would open \
                     to an empty grid. A loot fill is never empty: give it at least one stack. \
                     A container meant to hold nothing is scenery, and scenery is declared \
                     once for the whole campaign by `containers.decoration_notice` — delete \
                     this entry and let the notice speak for the container.",
                    l.id, l.anchor
                ),
            ));
        }
    }
    if quests.containers.as_ref().is_some_and(Containers::is_empty) {
        d.push(Diagnostic::error(
            CONTAINER_HOLDS_NOTHING,
            "quests",
            "/content/containers".to_string(),
            "`containers` is present and declares nothing. Declare \
             `containers.decoration_notice` — the line that tells the party, at arrival, \
             what the containers no fill claims hold — or delete `containers`."
                .to_string(),
        ));
    }
    fn descend(
        path: String,
        eff: &crate::QuestEffect,
        f: &mut dyn FnMut(String, &crate::QuestEffect),
    ) {
        f(path.clone(), eff);
        for (pseg, _kseg, list) in eff.nested_effect_lists_labeled() {
            for (j, inner) in list.iter().enumerate() {
                descend(format!("{path}/{pseg}/{j}"), inner, f);
            }
        }
    }
    crate::effects::for_each_effect_root(c, &mut |site, list| {
        for (m, eff) in list.iter().enumerate() {
            descend(format!("{}/{m}", site.path), eff, &mut |path, e| {
                let block = match &e.verb {
                    crate::Verb::SetBlock { block, .. } | crate::Verb::FillRegion { block, .. } => {
                        block
                    }
                    _ => return,
                };
                if crate::blocks::container_kind(block).is_none() {
                    return;
                }
                d.push(Diagnostic::error(
                    CONTAINER_HOLDS_NOTHING,
                    site.stage,
                    format!("{path}/block"),
                    format!(
                        "this `{}` places `{block}` at runtime. A container placed at runtime \
                         stands empty for ever: every fill runs at `setup_finish`, before it \
                         exists. A container in a delve is one of two things — a `collect`'s \
                         own chest, which the compiler places and fills (a `collect` with \
                         neither `container` nor `dropped_by`), or a container the piece \
                         places, filled by a `loot` entry. Place a block that is not a \
                         container here.",
                        e.verb.tag()
                    ),
                ));
            });
        }
    });
}

/// Exclusive ownership of an adopted container (DSL v0.8, `DW0435`).
///
/// Both container-fill surfaces write **positionally** from `container.0`: a
/// `loot` entry and a `collect`'s adopted container filling one cell overwrite
/// each other slot-for-slot, and the loser vanishes without a word — the same
/// silent-overwrite defect `DW0435` already names for two `loot` entries, reached
/// through a second door. Two `collect` objectives sharing a container is the
/// same collision (and worse: whichever activates second replaces the first
/// objective's items with its own).
///
/// Only claims involving at least one `collect` are reported here; `loot`-vs-
/// `loot` stays in [`loot_checks`], so nothing is diagnosed twice.
pub(crate) fn collect_container_claim_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    // (anchor -> what claims it), in declaration order. `loot` first: a loot
    // entry is the surface that exists to fill a container, so it reads as the
    // incumbent in the message.
    let mut claimed: BTreeMap<&str, String> = BTreeMap::new();
    for l in &c.quests.content.loot {
        claimed
            .entry(l.anchor.as_str())
            .or_insert_with(|| format!("loot `{}`", l.id));
    }
    for (i, q) in c.quests.content.quests.iter().enumerate() {
        for (j, o) in q.objectives.iter().enumerate() {
            let Some(cont) = o.collect_holder() else {
                continue;
            };
            let mine = format!("collect objective `{}`", o.id());
            if let Some(prev) = claimed.get(cont.as_str()) {
                d.push(Diagnostic::error(
                    LOOT_DUPLICATE_ANCHOR,
                    "quests",
                    format!("/content/quests/{i}/objectives/{j}/holder"),
                    format!(
                        "{prev} and {mine} both fill the container at anchor `{cont}`. Slots \
                         are assigned positionally from `container.0`, so one fill overwrites \
                         the other slot-for-slot and its items never reach the player. Give the \
                         collect its own container anchor (prefabs may expose several), or \
                         fold the other fill's items into it — do NOT rely on declaration \
                         order to combine them."
                    ),
                ));
            } else {
                claimed.insert(cont.as_str(), mine);
            }
        }
    }
}

use crate::blocks::MAX_CONTAINER_SLOTS;

/// `DW0436`: a **single-slot fill** whose `count` exceeds the item's
/// `minecraft:max_stack_size` in the pinned 1.21.11 registry.
///
/// Every one of these compiles to `item replace … container.<n> with <item>
/// <count>`, and that command fails **silently** above the cap: the slot simply
/// stays empty and the server logs nothing. A `count: 2` of `minecraft:rabbit_stew`
/// (cap 1) shipped an empty chest slot in the-drowned-bell round 2 — exactly the
/// silent-failure class `DW0431` exists for, one tier too late. The cap comes from
/// Mojang's own item-components data, vendored per MC pin
/// (`crates/delvec/data/item-stack-sizes-1.21.11.json`), never a hand table.
///
/// Skipped when the registry does not carry stack sizes (the small vendored DSL-side
/// subset) or the item id is unknown — the latter is already `DW0143`, and stacking
/// a second diagnostic on one typo is noise.
pub(crate) fn check_stack_count(
    item: &str,
    count: u32,
    what: &str,
    path: String,
    items: &dyn ItemRegistry,
    d: &mut Vec<Diagnostic>,
) {
    let Some(cap) = items.max_stack_size(item) else {
        return;
    };
    if count <= cap {
        return;
    }
    d.push(Diagnostic::error(
        ITEM_COUNT_OVER_STACK,
        "quests",
        path,
        format!(
            "{what} declares `{item}` × {count}, but `{item}` stacks to at most {cap} in \
             1.21.11. This is filled with `item replace … container.<n>`, which fails \
             SILENTLY above the cap — the slot ships empty and nothing is logged. Lower \
             the count to {cap} or fewer, or declare additional entries/containers."
        ),
    ));
}

/// Stage-5 `loot` declarations (spec-0021): id syntax/uniqueness (`DW0110`/
/// `DW0111`), anchor resolution (`DW0142`), item ids (`DW0143`), enchantments
/// (`DW0433`/`DW0434`), duplicate anchors (`DW0435`) and slot overflow
/// (`DW0432`).
///
/// The *container-ness* of the anchor's cell is deliberately NOT checked here:
/// it needs the assembled world, so it is a build-tier proof (`DW0431`) in the
/// compiler. This tier checks everything decidable from the DSL alone.
pub(crate) fn loot_checks(
    c: &Campaign,
    items: &dyn ItemRegistry,
    anchors: &dyn AnchorRegistry,
    d: &mut Vec<Diagnostic>,
) {
    let quests = &c.quests.content;
    if quests.loot.is_empty() {
        return;
    }
    let ench_reg = crate::registry::VendoredEnchantmentRegistry::v1_21_11();

    let providers = AnchorProviders::build(c, anchors);

    let mut seen_id: BTreeSet<&str> = BTreeSet::new();
    let mut seen_anchor: BTreeMap<&str, usize> = BTreeMap::new();
    for (i, l) in quests.loot.iter().enumerate() {
        if !l.id.is_valid_syntax() {
            d.push(Diagnostic::error(
                codes::ID_SYNTAX,
                "quests",
                format!("/content/loot/{i}/id"),
                format!(
                    "malformed loot id `{}` — loot ids must be lowercase kebab-case with the \
                     `loot/` prefix (e.g. `loot/galley-stores`)",
                    l.id
                ),
            ));
        }
        if !seen_id.insert(l.id.as_str()) {
            d.push(Diagnostic::error(
                codes::ID_DUPLICATE,
                "quests",
                format!("/content/loot/{i}/id"),
                format!("duplicate loot id `{}`", l.id),
            ));
        }
        if let Some(f) = station_kind_diag(
            &providers,
            l.anchor.as_str(),
            crate::layout::StationKind::Point,
            "a loot chest",
            "quests",
            format!("/content/loot/{i}/anchor"),
        ) {
            d.push(f);
        }
        if !providers.resolvable(l.anchor.as_str()) {
            d.push(Diagnostic::error(
                codes::ANCHOR_UNRESOLVED,
                "quests",
                format!("/content/loot/{i}/anchor"),
                format!(
                    "loot anchor `{}` is not provided by any prefab bound in this campaign — {}",
                    l.anchor,
                    providers.anchor_remedy(
                        "use an anchor the prefab exposes (anchor names come from prefab \
                         metadata; do NOT invent one)"
                    ),
                ),
            ));
        }
        // Two fills on one container: the second `item replace block` overwrites
        // the first slot-for-slot, so one declaration silently loses.
        if let Some(prev) = seen_anchor.insert(l.anchor.as_str(), i) {
            d.push(Diagnostic::error(
                LOOT_DUPLICATE_ANCHOR,
                "quests",
                format!("/content/loot/{i}/anchor"),
                format!(
                    "loot `{}` and loot `{}` both fill anchor `{}`. Slots are assigned \
                     positionally from `container.0`, so the later declaration overwrites the \
                     earlier one and its items never appear. Merge the two `items` lists into \
                     ONE `loot` entry — do NOT rely on declaration order to combine them.",
                    quests.loot[prev].id, l.id, l.anchor
                ),
            ));
        }
        if l.items.len() > MAX_CONTAINER_SLOTS {
            d.push(Diagnostic::error(
                codes::LOOT_TOO_MANY_ITEMS,
                "quests",
                format!("/content/loot/{i}/items"),
                format!(
                    "loot `{}` declares {} stacks, more than the {MAX_CONTAINER_SLOTS} slots of \
                     the largest vanilla container. Slots are assigned positionally, so every \
                     stack past the {MAX_CONTAINER_SLOTS}th would be dropped silently. Split \
                     the contents across more than one container.",
                    l.id,
                    l.items.len()
                ),
            ));
        }
        for (k, it) in l.items.iter().enumerate() {
            if !items.contains(&it.item) {
                d.push(Diagnostic::error(
                    codes::ITEM_UNKNOWN,
                    "quests",
                    format!("/content/loot/{i}/items/{k}/item"),
                    format!(
                        "loot item `{}` is not in the pinned 1.21.11 item registry — use a \
                         valid namespaced item id (e.g. `minecraft:cooked_cod`)",
                        it.item
                    ),
                ));
            }
            check_stack_count(
                &it.item,
                it.count,
                &format!("loot `{}`", l.id),
                format!("/content/loot/{i}/items/{k}/count"),
                items,
                d,
            );
            check_enchantments(
                &it.enchantments,
                &format!("loot `{}` item `{}`", l.id, it.item),
                "quests",
                &format!("/content/loot/{i}/items/{k}/enchantments"),
                &ench_reg,
                d,
            );
        }
        check_fill_slots(
            &format!("loot `{}`", l.id),
            &format!("/content/loot/{i}"),
            &l.items,
            items,
            d,
        );
    }
}
