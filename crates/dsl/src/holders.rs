//! **The item-holder class** (spec-0111): something with named slots the engine
//! fills at build time and a player empties by one act.
//!
//! Three families. The **container** family is the twelve block kinds of
//! [`crate::blocks::CONTAINER_KINDS_1_21_11`] (spec-0105), addressed by
//! `container.<n>`. The two **entity** families are the item frame (and its
//! glow variant) and the armour stand, whose rows are [`ENTITY_HOLDERS_1_21_11`]:
//! a piece carries the body in its template's `entities` list, the engine
//! strips it and summons it with the NBT it owns, and `loot[]` or `collect`
//! fills a named slot.
//!
//! Every fact in this module was read from the pinned 1.21.11 dedicated server
//! jar (`versions.toml` pin, bundled `server-1.21.11.jar` sha256
//! `ec47239a8de246335e1d54f6ac319bd35641778eb4b6a6da06372840d02fcebc`),
//! disassembled with `javap -c -p` and named through Mojang's official 1.21.11
//! server mappings (`server.txt` sha1
//! `5621e9253f05fd57872bbe7f8ddf5f9a7d525955`); the methods are named on each
//! item and recorded in `crates/delvec/data/PROVENANCE.md`.

/// The family a holder belongs to — what decides its slot vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HolderFamily {
    /// A container block (`container.<n>`).
    Container,
    /// An item frame or a glow item frame (`contents`).
    Frame,
    /// An armour stand (`armor.*`, `weapon.*`).
    Stand,
}

impl HolderFamily {
    /// The family's name in a diagnostic.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            HolderFamily::Container => "container",
            HolderFamily::Frame => "frame",
            HolderFamily::Stand => "stand",
        }
    }

    /// The family's slot vocabulary, as a diagnostic prints it.
    #[must_use]
    pub fn vocabulary(self) -> String {
        match self {
            HolderFamily::Container => {
                "`container.<n>`, `n` below the kind's slot count (positional when `slot` is \
                 omitted)"
                    .to_string()
            }
            HolderFamily::Frame => "`contents` (the default when `slot` is omitted)".to_string(),
            HolderFamily::Stand => format!(
                "{} (named on every stack: a stand's slots have no order)",
                STAND_SLOTS
                    .iter()
                    .map(|s| format!("`{}`", s.name))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }

    /// Whether `slot` is in this family's vocabulary, for a holder of
    /// `container_slots` slots when it is a container.
    #[must_use]
    pub fn has_slot(self, slot: &str, container_slots: usize) -> bool {
        match self {
            HolderFamily::Container => container_index(slot).is_some_and(|n| n < container_slots),
            HolderFamily::Frame => slot == FRAME_SLOT,
            HolderFamily::Stand => stand_slot(slot).is_some(),
        }
    }
}

/// What a stand slot accepts (`LivingEntity.createEquipmentSlotAccess`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotAccepts {
    /// Any stack (`HEAD`, `MAINHAND`, `OFFHAND`).
    Any,
    /// A stack whose `equippable` slot is this one (the game's spelling).
    Equippable(&'static str),
}

/// One armour-stand slot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StandSlot {
    /// The slot's name — the DSL's `slot` and `item replace entity`'s word.
    pub name: &'static str,
    /// The `EquipmentSlot` filter bit (`EquipmentSlot`'s static initialiser).
    pub filter_bit: u32,
    /// The click band (`ArmorStand.getClickedSlot`), feet-relative y over the
    /// stand's scale: `[from, to)`; `None` for a hand, which takes the click
    /// that falls in no filled band.
    pub band: Option<(f64, f64)>,
    /// What the slot accepts.
    pub accepts: SlotAccepts,
}

/// The item frame's one slot (`ItemFrame.getSlot(0)`, `SlotRanges` `contents`).
pub const FRAME_SLOT: &str = "contents";

/// **The armour stand's six slots**, in the order `getClickedSlot` tests the
/// armour bands (feet, chest, legs, head) and then the hands.
///
/// Bands from `ArmorStand.getClickedSlot`: `FEET` at `0.1 ≤ y < 0.55`, `CHEST`
/// at `0.9 ≤ y < 1.6`, `LEGS` at `0.4 ≤ y < 1.2`, `HEAD` at `y ≥ 1.6`, each
/// only when that slot holds an item. Filter bits from `EquipmentSlot`:
/// mainhand 0, feet 1, legs 2, chest 3, head 4, offhand 5.
pub const STAND_SLOTS: [StandSlot; 6] = [
    StandSlot {
        name: "armor.feet",
        filter_bit: 1,
        band: Some((0.1, 0.55)),
        accepts: SlotAccepts::Equippable("feet"),
    },
    StandSlot {
        name: "armor.chest",
        filter_bit: 3,
        band: Some((0.9, 1.6)),
        accepts: SlotAccepts::Equippable("chest"),
    },
    StandSlot {
        name: "armor.legs",
        filter_bit: 2,
        band: Some((0.4, 1.2)),
        accepts: SlotAccepts::Equippable("legs"),
    },
    StandSlot {
        name: "armor.head",
        filter_bit: 4,
        band: Some((1.6, f64::INFINITY)),
        accepts: SlotAccepts::Any,
    },
    StandSlot {
        name: "weapon.mainhand",
        filter_bit: 0,
        band: None,
        accepts: SlotAccepts::Any,
    },
    StandSlot {
        name: "weapon.offhand",
        filter_bit: 5,
        band: None,
        accepts: SlotAccepts::Any,
    },
];

/// The armour stand's height at scale 1 (`EntityType` registers 0.5 × 1.975).
pub const STAND_HEIGHT: f64 = 1.975;

/// The stand row of `name`, when it is one.
#[must_use]
pub fn stand_slot(name: &str) -> Option<&'static StandSlot> {
    STAND_SLOTS.iter().find(|s| s.name == name)
}

/// `n` of a `container.<n>` slot name.
#[must_use]
pub fn container_index(slot: &str) -> Option<usize> {
    let n = slot.strip_prefix("container.")?;
    if n.is_empty() || (n.len() > 1 && n.starts_with('0')) {
        return None;
    }
    n.parse().ok()
}

/// Whether `slot` belongs to SOME family's vocabulary (`DW1035`'s document
/// arm): a `container.<n>` under the largest container's 27, `contents`, or a
/// stand slot.
#[must_use]
pub fn in_some_vocabulary(slot: &str) -> bool {
    container_index(slot).is_some_and(|n| n < crate::blocks::MAX_CONTAINER_SLOTS)
        || slot == FRAME_SLOT
        || stand_slot(slot).is_some()
}

/// One entity holder kind.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntityHolderKind {
    /// The entity type id.
    pub id: &'static str,
    /// The census and grammar token (`item-frame`, `glow-item-frame`,
    /// `armor-stand`).
    pub token: &'static str,
    /// The family.
    pub family: HolderFamily,
    /// The NBT keys a piece may carry — what the holder LOOKS like
    /// (spec-0111 §3.1). Every other key is the engine's (`DW1036`).
    pub scene_keys: &'static [&'static str],
    /// The hitbox, `[width, height]` (`EntityType`'s registration).
    pub size: [f64; 2],
}

/// **The two entity holder families of the pinned game, three entity kinds**
/// (spec-0111 §2.1).
///
/// The frame's scene keys are what `ItemFrame.readAdditionalSaveData` reads
/// that is not the item, its drop or its lock (`Facing`, `ItemRotation`,
/// `Invisible`); the stand's are what `ArmorStand.readAdditionalSaveData` reads
/// that is not a slot lock or the marker (`Pose`, `Small`, `ShowArms`,
/// `NoBasePlate`, `Invisible`), plus the entity's `Rotation`. `GlowItemFrame`
/// overrides only sounds and its frame item, so every fact of the frame row
/// holds for it.
pub const ENTITY_HOLDERS_1_21_11: [EntityHolderKind; 3] = [
    EntityHolderKind {
        id: "minecraft:item_frame",
        token: "item-frame",
        family: HolderFamily::Frame,
        scene_keys: &["Facing", "ItemRotation", "Invisible"],
        size: [0.5, 0.5],
    },
    EntityHolderKind {
        id: "minecraft:glow_item_frame",
        token: "glow-item-frame",
        family: HolderFamily::Frame,
        scene_keys: &["Facing", "ItemRotation", "Invisible"],
        size: [0.5, 0.5],
    },
    EntityHolderKind {
        id: "minecraft:armor_stand",
        token: "armor-stand",
        family: HolderFamily::Stand,
        scene_keys: &[
            "Rotation",
            "Pose",
            "Small",
            "ShowArms",
            "NoBasePlate",
            "Invisible",
        ],
        size: [0.5, STAND_HEIGHT],
    },
];

/// The keys every template entity carries that the engine reads but does not
/// judge as owned: the entity type `id` (the kind itself).
pub const ENTITY_ID_KEY: &str = "id";

/// The holder kind of entity type `id`, when it is one.
#[must_use]
pub fn entity_holder(id: &str) -> Option<&'static EntityHolderKind> {
    let id = if id.contains(':') {
        std::borrow::Cow::Borrowed(id)
    } else {
        std::borrow::Cow::Owned(format!("minecraft:{id}"))
    };
    ENTITY_HOLDERS_1_21_11.iter().find(|k| k.id == id.as_ref())
}

/// The holder kind of a census / grammar token.
#[must_use]
pub fn entity_holder_by_token(token: &str) -> Option<&'static EntityHolderKind> {
    ENTITY_HOLDERS_1_21_11.iter().find(|k| k.token == token)
}

/// **What the engine sets on a key a piece may not carry** (spec-0111 §4.1),
/// for `DW1036`'s message: `Some(what)` for a key the engine owns by name,
/// `None` for any other key (which the engine neither reads nor keeps).
#[must_use]
pub fn owned_key_rule(family: HolderFamily, key: &str) -> Option<&'static str> {
    match (family, key) {
        (_, "Tags") => {
            Some("set by the engine: `dw_fixture`, `dw_holder` and the holder's own tag")
        }
        (_, "UUID") => Some("derived by the game when the engine summons the body"),
        (_, "Pos") | (_, "block_pos") | (_, "TileX") | (_, "TileY") | (_, "TileZ") => {
            Some("derived from the entity's place in the template, rotated with the piece")
        }
        (_, "Invulnerable") => Some(
            "set by the engine: `0b` on a frame that holds an item, `1b` on an empty frame and on \
             every stand",
        ),
        (HolderFamily::Frame, "Item") => {
            Some("set by the engine from the fill that claims the frame (`loot[]` or `collect`)")
        }
        (HolderFamily::Frame, "Fixed") => {
            Some("set by the engine: `0b` while the frame holds an item, `1b` once it is empty")
        }
        (HolderFamily::Frame, "ItemDropChance") => {
            Some("set by the engine to `1.0f`, the one random on the take path, pinned")
        }
        (HolderFamily::Stand, "equipment")
        | (HolderFamily::Stand, "HandItems")
        | (HolderFamily::Stand, "ArmorItems") => {
            Some("set by the engine from the fill that claims each slot (`loot[]` or `collect`)")
        }
        (HolderFamily::Stand, "DisabledSlots") => Some(
            "computed by the engine: putting disabled on all six slots, taking disabled on every \
             slot no fill claims",
        ),
        (HolderFamily::Stand, "Marker") => {
            Some("set by the engine to `0b`: a marker stand ignores the click")
        }
        _ => None,
    }
}

/// **The click height that takes `slot` from a stand** whose filled slots are
/// `filled` (spec-0111 §5.4): the middle of the longest run of heights, at
/// one-hundredth-of-a-block steps over the stand's body, that
/// `ArmorStand.getClickedSlot` maps to `slot`, in world units (the band times
/// the stand's scale, `0.5` for a `Small` stand). `None` when no height maps to
/// it (an off hand behind a filled main hand).
#[must_use]
pub fn click_height(slot: &str, filled: &[&str], small: bool) -> Option<f64> {
    let scale = if small { 0.5 } else { 1.0 };
    let holds = |name: &str| filled.contains(&name);
    let clicked = |y: f64| -> &'static str {
        for s in STAND_SLOTS.iter().filter(|s| s.band.is_some()) {
            let (lo, hi) = s.band.expect("filtered");
            if y >= lo && y < hi && holds(s.name) {
                return s.name;
            }
        }
        if !holds("weapon.mainhand") && holds("weapon.offhand") {
            "weapon.offhand"
        } else {
            "weapon.mainhand"
        }
    };
    let steps = (STAND_HEIGHT * 100.0) as i32;
    let mut best: Option<(i32, i32)> = None;
    let mut run: Option<i32> = None;
    for i in 0..=steps {
        let y = f64::from(i) / 100.0;
        let hit = clicked(y) == slot && i < steps;
        match (hit, run) {
            (true, None) => run = Some(i),
            (false, Some(start)) => {
                let len = i - start;
                if best.is_none_or(|(s, e)| len > e - s) {
                    best = Some((start, i));
                }
                run = None;
            }
            _ => {}
        }
    }
    let (s, e) = best?;
    let mid = (f64::from(s) + f64::from(e - 1)) / 2.0 / 100.0;
    Some(((mid * scale) * 100.0).round() / 100.0)
}

/// The stand's `DisabledSlots` (spec-0111 §4.1): bit `16 + f` for all six
/// slots (nothing can be put on), and bit `8 + f` for every slot `claimed`
/// does not name (only what the fill placed can be taken).
#[must_use]
pub fn disabled_slots(claimed: &[&str]) -> i32 {
    let mut bits = 0i32;
    for s in &STAND_SLOTS {
        bits |= 1 << (16 + s.filter_bit);
        if !claimed.contains(&s.name) {
            bits |= 1 << (8 + s.filter_bit);
        }
    }
    bits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_vocabulary_is_closed() {
        for s in [
            "container.0",
            "container.26",
            "contents",
            "armor.head",
            "weapon.offhand",
        ] {
            assert!(in_some_vocabulary(s), "{s}");
        }
        for s in [
            "container.27",
            "container.01",
            "container.",
            "armor.body",
            "weapon.main",
            "slot.0",
        ] {
            assert!(!in_some_vocabulary(s), "{s}");
        }
        assert!(HolderFamily::Container.has_slot("container.2", 3));
        assert!(!HolderFamily::Container.has_slot("container.3", 3));
        assert!(!HolderFamily::Frame.has_slot("armor.head", 0));
        assert!(HolderFamily::Stand.has_slot("armor.head", 0));
    }

    /// A stand holding a helmet and a sword: the helmet is taken by a click on
    /// the head, the sword by a click anywhere outside it.
    #[test]
    fn the_click_bands_answer_which_slot_a_click_takes() {
        let filled = ["armor.head", "weapon.mainhand"];
        let head = click_height("armor.head", &filled, false).unwrap();
        assert!((1.6..STAND_HEIGHT).contains(&head), "{head}");
        let hand = click_height("weapon.mainhand", &filled, false).unwrap();
        assert!(hand < 1.6, "{hand}");
        // An off hand behind a filled main hand takes no click.
        assert_eq!(
            click_height(
                "weapon.offhand",
                &["weapon.mainhand", "weapon.offhand"],
                false
            ),
            None
        );
        // A small stand halves the bands.
        let small = click_height("armor.head", &filled, true).unwrap();
        assert!((0.8..1.0).contains(&small), "{small}");
    }

    #[test]
    fn disabled_slots_lock_putting_everywhere_and_taking_where_nothing_was_placed() {
        let all_put = (0..6).fold(0, |b, i| b | 1 << (16 + i));
        let all_take = (0..6).fold(0, |b, i| b | 1 << (8 + i));
        assert_eq!(disabled_slots(&[]), all_put | all_take);
        let two = disabled_slots(&["armor.head", "weapon.mainhand"]);
        assert_eq!(two & (1 << (8 + 4)), 0, "the head can be taken");
        assert_eq!(two & (1 << 8), 0, "the main hand can be taken");
        assert_ne!(two & (1 << (8 + 5)), 0, "the off hand cannot");
    }

    #[test]
    fn every_kind_is_found_by_id_and_token() {
        for k in &ENTITY_HOLDERS_1_21_11 {
            assert_eq!(entity_holder(k.id), Some(k));
            assert_eq!(entity_holder_by_token(k.token), Some(k));
        }
        assert_eq!(
            entity_holder("item_frame").map(|k| k.token),
            Some("item-frame")
        );
        assert!(entity_holder("minecraft:pig").is_none());
    }
}
