//! **What a piece may carry in its `entities` list** (spec-0111 §3.1): an item
//! holder — `minecraft:item_frame`, `minecraft:glow_item_frame`,
//! `minecraft:armor_stand` — saying what it looks like, and nothing else.
//!
//! Two refusals, one per question. An entity whose kind the allowlist's
//! entity list does not name is `DW0730`, the same refusal a block outside
//! the block list gets. A holder carrying a key outside its kind's scene
//! properties is `DW1036`: the item, the lock flags, the drop chance, the slot
//! locks, the tags and the identity are the engine's, which summons every
//! holder itself with the NBT it writes in full (spec-0111 §4.1), so a piece
//! cannot smuggle a stocked holder in any more than a stocked chest.
//!
//! The one judge every door asks — `delvec prefab audit` (per template, per
//! tile), `delvec detail` and the grammar export (over the entities a mark's
//! `holder` writes), and the generators' own invariant — so the doors cannot
//! disagree about what a piece may carry.

use serde::Serialize;

use crate::admit::allowlist::Allowlist;
use crate::admit::diag::{DW_ALLOWLIST, DW_HOLDER_OWNED, Diagnostic};
use crate::schem::nbt::Nbt;

/// What the entity judge examined, for the audit report.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct EntityBinding {
    /// Template entities examined — the denominator.
    pub examined: usize,
    /// Of those, holders of an admitted kind.
    pub holders: usize,
    /// Entities of a kind the entity list does not name (`DW0730`).
    pub not_allowlisted: usize,
    /// Owned keys found on holders (`DW1036`).
    pub owned_keys: usize,
}

/// The entity type id of one template entity: its `nbt.id`.
#[must_use]
pub fn entity_id(entity: &Nbt) -> Option<&str> {
    entity
        .as_compound()?
        .get("nbt")?
        .as_compound()?
        .get("id")?
        .as_str()
}

/// The cell a template entity occupies: its `blockPos`, else its `pos` floored.
#[must_use]
pub fn entity_cell(entity: &Nbt) -> Option<[i32; 3]> {
    let c = entity.as_compound()?;
    if let Some(l) = c.get("blockPos").and_then(Nbt::as_list)
        && l.len() == 3
    {
        return Some([l[0].as_i32()?, l[1].as_i32()?, l[2].as_i32()?]);
    }
    let l = c.get("pos").and_then(Nbt::as_list)?;
    if l.len() != 3 {
        return None;
    }
    let f = |v: &Nbt| match v {
        Nbt::Double(d) => Some(d.floor() as i32),
        Nbt::Float(d) => Some(d.floor() as i32),
        _ => None,
    };
    Some([f(&l[0])?, f(&l[1])?, f(&l[2])?])
}

/// **Judge a template's `entities` list** against `allow` and the holder
/// table: `DW0730` for a kind the list does not name, `DW1036` for every key a
/// holder carries outside its kind's scene properties, each at the entity's
/// cell.
#[must_use]
pub fn judge(entities: &[Nbt], allow: &Allowlist) -> (EntityBinding, Vec<Diagnostic>) {
    let mut b = EntityBinding::default();
    let mut diags = Vec::new();
    for e in entities {
        b.examined += 1;
        let cell = entity_cell(e);
        let at = |d: Diagnostic| match cell {
            Some(c) => d.at(c),
            None => d,
        };
        let Some(id) = entity_id(e) else {
            b.not_allowlisted += 1;
            diags.push(at(Diagnostic::error(
                DW_ALLOWLIST,
                "a template entity names no entity type (`nbt.id`), so nothing can say what the \
                 game would load there. A piece carries item holders and nothing else: an item \
                 frame, a glow item frame or an armour stand, each saying what it looks like",
            )));
            continue;
        };
        if !allow.permits_entity(id) {
            b.not_allowlisted += 1;
            diags.push(at(Diagnostic::error(
                DW_ALLOWLIST,
                format!(
                    "entity `{id}` is not in the allowlist's entity list ({}). A piece carries the \
                     furniture a fill stocks — an item holder — and every other body in a delve \
                     is summoned by the datapack from a declaration (an `actor`, an NPC, a \
                     wave), never shipped in a template. Remove it from the piece, or declare it \
                     in the campaign",
                    allow.entity_list()
                ),
            )));
            continue;
        }
        let Some(kind) = delvewright_dsl::holders::entity_holder(id) else {
            // A kind the owner's override admits that is not a holder: the
            // list is the owner's, and judging its keys is the holder table's
            // business only.
            continue;
        };
        b.holders += 1;
        let keys = e
            .as_compound()
            .and_then(|c| c.get("nbt"))
            .and_then(Nbt::as_compound);
        for key in keys.into_iter().flat_map(|m| m.keys()) {
            if key == delvewright_dsl::holders::ENTITY_ID_KEY || kind.scene_keys.contains(&key.as_str())
            {
                continue;
            }
            b.owned_keys += 1;
            let what = delvewright_dsl::holders::owned_key_rule(kind.family, key).map_or_else(
                || {
                    "not a scene property, and the engine neither reads nor keeps it".to_string()
                },
                str::to_string,
            );
            diags.push(at(Diagnostic::error(
                DW_HOLDER_OWNED,
                format!(
                    "the piece's `{id}` carries `{key}`, which the engine owns — {what}. A piece \
                     says what a holder LOOKS like ({}) and nothing else: the engine strips every \
                     holder from the template it ships and summons it with the NBT it writes in \
                     full. Remove `{key}` from the entity; an item the holder should hold is \
                     declared by a fill — a `loot` entry or a `collect` with `holder` — on the \
                     anchor at its cell",
                    kind.scene_keys
                        .iter()
                        .map(|k| format!("`{k}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            )));
        }
    }
    (b, diags)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn entity(id: &str, extra: &[(&str, Nbt)]) -> Nbt {
        let mut nbt = BTreeMap::new();
        nbt.insert("id".to_string(), Nbt::String(id.to_string()));
        for (k, v) in extra {
            nbt.insert((*k).to_string(), v.clone());
        }
        let mut c = BTreeMap::new();
        c.insert(
            "blockPos".to_string(),
            Nbt::List(vec![Nbt::Int(1), Nbt::Int(2), Nbt::Int(3)]),
        );
        c.insert(
            "pos".to_string(),
            Nbt::List(vec![Nbt::Double(1.5), Nbt::Double(2.5), Nbt::Double(3.5)]),
        );
        c.insert("nbt".to_string(), Nbt::Compound(nbt));
        Nbt::Compound(c)
    }

    #[test]
    fn a_holder_saying_what_it_looks_like_passes() {
        let (b, d) = judge(
            &[
                entity("minecraft:item_frame", &[("Facing", Nbt::Byte(2))]),
                entity("minecraft:armor_stand", &[("ShowArms", Nbt::Byte(1))]),
            ],
            &Allowlist::default(),
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!((b.examined, b.holders), (2, 2));
    }

    #[test]
    fn a_holder_carrying_an_item_is_dw1036_naming_the_key() {
        let item = Nbt::Compound(BTreeMap::new());
        let (b, d) = judge(
            &[entity("minecraft:glow_item_frame", &[("Item", item)])],
            &Allowlist::default(),
        );
        assert_eq!(b.owned_keys, 1);
        assert_eq!(d[0].code, "DW1036");
        assert!(d[0].message.contains("`Item`"), "{}", d[0].message);
        assert!(d[0].message.contains("fill"), "{}", d[0].message);
        assert_eq!(d[0].pos, Some([1, 2, 3]));
    }

    #[test]
    fn any_other_kind_is_dw0730() {
        let (b, d) = judge(&[entity("minecraft:pig", &[])], &Allowlist::default());
        assert_eq!(b.not_allowlisted, 1);
        assert_eq!(d[0].code, "DW0730");
        assert!(d[0].message.contains("minecraft:pig"), "{}", d[0].message);
    }
}
