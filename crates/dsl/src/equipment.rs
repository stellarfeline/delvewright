//! **What a body shows** (spec-0067 §4): the equipment slots each living entity
//! type of pinned Minecraft Java 1.21.11 draws, the kind of piece each slot
//! draws, and the fit rule `DW0898` holds every declared piece to.
//!
//! The server stores all eight slots on every living entity; the client draws
//! only the ones the renderer registered for that entity type carries an
//! equipment layer for. A piece in a slot the body does not draw is structurally
//! perfect NBT nobody sees, and it is refused where it is declared.
//!
//! Two pinned tables answer the two halves:
//!
//! - **The body table** (`crates/dsl/data/entity-slots-1.21.11.json`): one row
//!   per living entity type of the pinned registry, slot → the piece kinds its
//!   layers draw, the body/saddle layer type, and the state a conditional hand
//!   is drawn in; each row names the client renderer it was read from. Authored
//!   from the pinned client's renderers under Mojang's published mappings, and
//!   held to the entity-type tags and the equipment assets by the cross-checks
//!   in `crates/delvec/tests/equipment_tables.rs`.
//! - **The item facts** ([`Equippable`]), injected through
//!   [`crate::registry::ItemRegistry::equippable`]: the compiler vendors them as
//!   `crates/delvec/data/item-equippable-1.21.11.json`
//!   (`tools/extract-item-equippable.py`).

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

use crate::diagnostic::{Diagnostic, codes};
use crate::envelope::Campaign;
use crate::registry::{ItemRegistry, entity_in_tag, entity_tags, namespaced_entity};
use crate::stages::{BodyRef, EquipSlot, MobEquipment};

/// The kind of piece an item is, derived from its `equippable` component and
/// the equipment asset it draws through (spec-0067 §4.1). The kind decides
/// which of a body's shown slots can draw it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PieceKind {
    /// An asset and a `head` / `chest` / `legs` / `feet` slot: drawn by
    /// `HumanoidArmorLayer`.
    Armour,
    /// An asset with a `wings` layer (the elytra): drawn by `WingsLayer`.
    Wings,
    /// An asset with a `*_body` or `*_saddle` layer: drawn by a body's own
    /// equipment layer.
    Animal,
    /// No asset — a head, the carved pumpkin, the shield, and every item with
    /// no `equippable` component at all.
    Item,
}

impl PieceKind {
    /// The word a diagnostic prints.
    pub fn name(self) -> &'static str {
        match self {
            PieceKind::Armour => "armour",
            PieceKind::Wings => "wings",
            PieceKind::Animal => "animal",
            PieceKind::Item => "item",
        }
    }
}

/// An item's `minecraft:equippable` facts in the pinned item data.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Equippable {
    /// The slot the item declares, in the game's spelling (`body`, `offhand`).
    pub slot: String,
    /// The equipment asset the item draws through, if any.
    #[serde(default)]
    pub asset_id: Option<String>,
    /// The entity ids and `#tag`s the item admits; empty when it names none.
    #[serde(default)]
    pub allowed_entities: Vec<String>,
    /// The kind of piece this is.
    pub kind: PieceKind,
}

impl Equippable {
    /// The declared slot as the DSL's slot type.
    pub fn declared_slot(&self) -> Option<EquipSlot> {
        EquipSlot::from_nbt(&self.slot)
    }
}

/// What an item registry knows about an item's `equippable` component.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EquippableFact<'a> {
    /// The registry carries no equippable data; nothing is judged.
    Unknown,
    /// The item carries no `equippable` component: a plain item.
    Plain,
    /// The item's component.
    Equippable(&'a Equippable),
}

impl EquippableFact<'_> {
    /// The kind of piece, or `None` when the registry does not know.
    pub fn kind(self) -> Option<PieceKind> {
        match self {
            EquippableFact::Unknown => None,
            EquippableFact::Plain => Some(PieceKind::Item),
            EquippableFact::Equippable(e) => Some(e.kind),
        }
    }
}

/// One slot a body draws.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShownSlot {
    /// The piece kinds this slot's layers draw.
    pub kinds: Vec<PieceKind>,
    /// The equipment layer type a `body` / `saddle` slot draws through.
    #[serde(default)]
    pub layer: Option<String>,
    /// The state a conditional hand is drawn in (an illager's, the panda's);
    /// absent when the slot is drawn in every state.
    #[serde(default)]
    pub when: Option<String>,
}

/// One living entity type's row in the body table.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyRow {
    /// The client renderer class the row was read from.
    pub renderer: String,
    /// Slot (the game's spelling) → what it draws. Empty for a body that
    /// draws no equipment.
    pub slots: BTreeMap<String, ShownSlot>,
}

impl BodyRow {
    /// What this body draws in `slot`, if it draws it at all.
    pub fn shown(&self, slot: EquipSlot) -> Option<&ShownSlot> {
        self.slots.get(slot.nbt())
    }
}

/// The body table: namespaced entity id → its row, one per living entity type
/// of the pinned registry.
pub fn body_table() -> &'static BTreeMap<String, BodyRow> {
    static TABLE: std::sync::LazyLock<BTreeMap<String, BodyRow>> = std::sync::LazyLock::new(|| {
        serde_json::from_str(include_str!("../data/entity-slots-1.21.11.json"))
            .expect("the vendored body table is valid JSON of its own shape")
    });
    &TABLE
}

/// The row for `entity`, or `None` for an entity type that is not a living
/// entity of the pinned registry (a boat, a marker, a display).
pub fn body_row(entity: &str) -> Option<&'static BodyRow> {
    body_table().get(&namespaced_entity(entity))
}

/// Whether `entity` draws `slot` for a piece of `kind`.
pub fn shows(entity: &str, slot: EquipSlot, kind: PieceKind) -> bool {
    body_row(entity)
        .and_then(|r| r.shown(slot))
        .is_some_and(|s| s.kinds.contains(&kind))
}

/// Whether `entity` draws anything at all in `slot`.
pub fn shows_slot(entity: &str, slot: EquipSlot) -> bool {
    body_row(entity).and_then(|r| r.shown(slot)).is_some()
}

/// The slots `entity` shows, spelled for a creator: `` `body` (animal) ``,
/// `` `main_hand` (any piece) `` — or `None` when it shows none.
pub fn shown_slots_phrase(entity: &str) -> Option<String> {
    let row = body_row(entity)?;
    let all = [
        PieceKind::Armour,
        PieceKind::Wings,
        PieceKind::Animal,
        PieceKind::Item,
    ];
    let parts: Vec<String> = EquipSlot::ALL
        .into_iter()
        .filter_map(|s| {
            let shown = row.shown(s)?;
            let kinds = if all.iter().all(|k| shown.kinds.contains(k)) {
                "any piece".to_string()
            } else {
                let mut ks: Vec<PieceKind> = shown.kinds.clone();
                ks.sort();
                ks.iter().map(|k| k.name()).collect::<Vec<_>>().join(" or ")
            };
            let when = shown
                .when
                .as_ref()
                .map(|w| format!(", drawn only while {w}"))
                .unwrap_or_default();
            Some(format!("`{}` ({kinds}{when})", s.field()))
        })
        .collect();
    (!parts.is_empty()).then(|| parts.join(", "))
}

/// Whether `entity` is admitted by an `allowed_entities` list: an entry is an
/// entity id or a `#tag` of the vendored entity-type tag table.
pub fn admitted(entity: &str, allowed: &[String]) -> bool {
    let id = namespaced_entity(entity);
    allowed.iter().any(|a| match a.strip_prefix('#') {
        Some(tag) => entity_in_tag(&id, tag),
        None => namespaced_entity(a) == id,
    })
}

/// An `allowed_entities` list spelled out: every tag expanded to its members,
/// sorted and de-duplicated, so a creator never opens the tag file.
pub fn admitted_entities(allowed: &[String]) -> Vec<String> {
    let mut out: BTreeSet<String> = BTreeSet::new();
    for a in allowed {
        match a.strip_prefix('#') {
            Some(tag) => {
                out.extend(entity_tags().get(tag).into_iter().flatten().cloned());
            }
            None => {
                out.insert(namespaced_entity(a));
            }
        }
    }
    out.into_iter().collect()
}

/// The shapes of `DW0898` one piece meets (spec-0067 §4.2).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Misfit {
    /// Shape 1: the body does not show this piece in this slot.
    pub unshown: bool,
    /// Shape 2: the item declares this other slot.
    pub declared_elsewhere: Option<EquipSlot>,
    /// Shape 3: the item's allowed-entities list excludes the body; the list,
    /// expanded.
    pub excluded_by: Option<Vec<String>>,
}

impl Misfit {
    /// Whether any shape holds.
    pub fn any(&self) -> bool {
        self.unshown || self.declared_elsewhere.is_some() || self.excluded_by.is_some()
    }
}

/// Judge one piece `item` in `slot` on a body of type `entity`. `None` when the
/// registry does not know the item's equippable facts (nothing is judged).
pub fn judge(
    entity: &str,
    slot: EquipSlot,
    item: &str,
    items: &dyn ItemRegistry,
) -> Option<Misfit> {
    let fact = items.equippable(item);
    let kind = fact.kind()?;
    let mut m = Misfit {
        unshown: !shows(entity, slot, kind),
        ..Misfit::default()
    };
    if let EquippableFact::Equippable(e) = fact {
        if let Some(declared) = e.declared_slot()
            && !slot.is_hand()
            && declared != slot
        {
            m.declared_elsewhere = Some(declared);
        }
        if !e.allowed_entities.is_empty() && !admitted(entity, &e.allowed_entities) {
            m.excluded_by = Some(admitted_entities(&e.allowed_entities));
        }
    }
    Some(m)
}

/// The `DW0898` message for one misfit piece: every shape that holds, each with
/// its remedy.
pub fn misfit_message(
    what: &str,
    entity: &str,
    slot: EquipSlot,
    item: &str,
    kind: PieceKind,
    m: &Misfit,
) -> String {
    let body = namespaced_entity(entity);
    let mut shapes: Vec<String> = Vec::new();
    if m.unshown {
        let shown = match (body_row(&body), shown_slots_phrase(&body)) {
            (None, _) => format!(
                "`{body}` is not a living entity of the pinned 1.21.11 registry, so no slot of it \
                 is ever drawn"
            ),
            (Some(_), None) => format!("`{body}` draws no equipment slot at all"),
            (Some(_), Some(p)) => format!("`{body}` draws {p}"),
        };
        let weapon = if slot.is_hand() {
            " A held weapon meant as a number is written as `attributes`, which applies whether \
             or not anything is drawn."
        } else {
            ""
        };
        shapes.push(format!(
            "the body does not show it: `{item}` is a piece of kind {} in `{}`, and {shown}. \
             Dress another body, or choose a piece this body draws.{weapon}",
            kind.name(),
            slot.field()
        ));
    }
    if let Some(declared) = m.declared_elsewhere {
        shapes.push(format!(
            "the wrong slot: `{item}` declares the `{}` slot and is written in `{}`. Move it to \
             `{}`.",
            declared.field(),
            slot.field(),
            declared.field()
        ));
    }
    if let Some(admits) = &m.excluded_by {
        shapes.push(format!(
            "the wrong body: `{item}` is worn only by {}, and this body is `{body}`. Change the \
             entity, or choose a piece `{body}` wears.",
            admits
                .iter()
                .map(|e| format!("`{e}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    format!(
        "{what} equipment `{}` puts `{item}` where the pinned game will not show it on this body \
         ({} shape(s)): {}",
        slot.field(),
        shapes.len(),
        shapes.join(" Also, ")
    )
}

/// One body that wears an `equipment` declaration: where it is, what it is
/// called in a diagnostic, the entity type the puppet actually wears, and the
/// declaration.
pub struct DressedBody<'a> {
    /// JSON pointer of the `equipment` object.
    pub path: String,
    /// `actor` / `wave-mob`, for a diagnostic.
    pub what: &'static str,
    /// The entity type the body ships as.
    pub entity: String,
    /// The declaration.
    pub equipment: &'a MobEquipment,
}

/// Every equipment declaration the campaign makes, in declaration order: wave
/// mobs, then actors (judged as the body the puppet wears — a skinned actor is a
/// mannequin).
pub fn dressed_bodies(c: &Campaign) -> Vec<DressedBody<'_>> {
    let q = &c.quests.content;
    let mut out = Vec::new();
    for (i, w) in q.waves.iter().enumerate() {
        for (k, m) in w.mobs.iter().enumerate() {
            if let Some(eq) = &m.equipment {
                out.push(DressedBody {
                    path: format!("/content/waves/{i}/mobs/{k}/equipment"),
                    what: "wave-mob",
                    entity: m.entity.clone(),
                    equipment: eq,
                });
            }
        }
    }
    for (i, a) in q.actors.iter().enumerate() {
        if let Some(eq) = &a.equipment {
            out.push(DressedBody {
                path: format!("/content/actors/{i}/equipment"),
                what: "actor",
                entity: BodyRef::Actor(a).worn_entity().to_string(),
                equipment: eq,
            });
        }
    }
    out
}

/// `DW0898`: every declared piece is put where the pinned game shows it on
/// that body.
pub fn fit_checks(c: &Campaign, items: &dyn ItemRegistry, d: &mut Vec<Diagnostic>) {
    for b in dressed_bodies(c) {
        for (slot, piece) in b.equipment.pieces() {
            let Some(piece) = piece else { continue };
            let item = piece.item();
            if !items.contains(item) {
                continue; // `DW0143` names it.
            }
            let Some(m) = judge(&b.entity, slot, item, items) else {
                continue;
            };
            if !m.any() {
                continue;
            }
            let kind = items
                .equippable(item)
                .kind()
                .expect("judged, so the kind is known");
            d.push(Diagnostic::error(
                codes::EQUIPMENT_UNSHOWN,
                "quests",
                format!("{}/{}", b.path, slot.field()),
                misfit_message(b.what, &b.entity, slot, item, kind, &m),
            ));
        }
    }
}

/// What the fit rule examined, zeroes included (spec-0067 §5).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EquipmentBinding {
    /// Equipment declarations (a wave-mob stack or an actor).
    pub bodies: usize,
    /// Distinct entity types among them.
    pub entity_types: usize,
    /// Pieces declared.
    pub pieces: usize,
    /// Distinct slots in use.
    pub slots: usize,
    /// Pieces whose item declares a slot in the pinned item data.
    pub declared_slot: usize,
    /// Pieces whose item carries an allowed-entities list.
    pub allowed_list: usize,
    /// Pieces refused (`DW0898`).
    pub refused: usize,
}

impl EquipmentBinding {
    /// Count what [`fit_checks`] examines on `c`.
    pub fn of(c: &Campaign, items: &dyn ItemRegistry) -> Self {
        let mut b = EquipmentBinding::default();
        let mut types: BTreeSet<String> = BTreeSet::new();
        let mut slots: BTreeSet<EquipSlot> = BTreeSet::new();
        for body in dressed_bodies(c) {
            b.bodies += 1;
            types.insert(namespaced_entity(&body.entity));
            for (slot, piece) in body.equipment.pieces() {
                let Some(piece) = piece else { continue };
                b.pieces += 1;
                slots.insert(slot);
                let item = piece.item();
                if let EquippableFact::Equippable(e) = items.equippable(item) {
                    b.declared_slot += 1;
                    if !e.allowed_entities.is_empty() {
                        b.allowed_list += 1;
                    }
                }
                if items.contains(item)
                    && judge(&body.entity, slot, item, items).is_some_and(|m| m.any())
                {
                    b.refused += 1;
                }
            }
        }
        b.entity_types = types.len();
        b.slots = slots.len();
        b
    }

    /// The one line this rule owes its reader.
    pub fn line(&self) -> String {
        format!(
            "equipment binding: {} body(ies) dressed over {} entity type(s), {} piece(s) declared \
             over {} slot(s) in use, {} piece(s) with a registry-declared slot, {} with an \
             allowed-entity list, {} refused (DW0898).",
            self.bodies,
            self.entity_types,
            self.pieces,
            self.slots,
            self.declared_slot,
            self.allowed_list,
            self.refused
        )
    }
}
