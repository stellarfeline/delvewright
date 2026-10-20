//! Item holders the pieces carry (spec-0111 §4): the engine owns the body.
//!
//! Every holder entity is stripped from the template the build ships and
//! summoned in `setup_finish` with NBT the engine writes in full — the look
//! the piece gave it, the tags, and the flags that make the take the only act
//! that works — then filled by `item replace entity`, the one command shape
//! the whole holder class is filled by. A frame a player empties freezes on
//! the next tick (§4.3).

use super::*;
use crate::compiler::holders::OwnedHolder;
use crate::schem::nbt::Nbt;
use delvewright_dsl::holders::HolderFamily;

/// The entity-type tag naming the two frame kinds (`#<ns>:frames`), which the
/// freeze line selects.
pub(super) const FRAMES_TAG: &str = "frames";

/// One NBT value as SNBT, deterministically (compounds in key order).
pub(crate) fn snbt(v: &Nbt) -> String {
    match v {
        Nbt::Byte(b) => format!("{b}b"),
        Nbt::Short(b) => format!("{b}s"),
        Nbt::Int(b) => format!("{b}"),
        Nbt::Long(b) => format!("{b}L"),
        Nbt::Float(f) => format!("{}f", fmt_f64(f64::from(*f))),
        Nbt::Double(f) => format!("{}d", fmt_f64(*f)),
        Nbt::String(s) => snbt_string(s),
        Nbt::ByteArray(a) => format!(
            "[B;{}]",
            a.iter()
                .map(|b| format!("{b}b"))
                .collect::<Vec<_>>()
                .join(",")
        ),
        Nbt::IntArray(a) => format!(
            "[I;{}]",
            a.iter().map(i32::to_string).collect::<Vec<_>>().join(",")
        ),
        Nbt::LongArray(a) => format!(
            "[L;{}]",
            a.iter()
                .map(|b| format!("{b}L"))
                .collect::<Vec<_>>()
                .join(",")
        ),
        Nbt::List(l) => format!("[{}]", l.iter().map(snbt).collect::<Vec<_>>().join(",")),
        Nbt::Compound(m) => format!(
            "{{{}}}",
            m.iter()
                .map(|(k, v)| format!("{k}:{}", snbt(v)))
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}

/// The body's position as a summon writes it: a frame at its cell's centre
/// (the game sets its `block_pos` from the position), a stand on its foot
/// cell's floor centre.
fn summon_pos(o: &OwnedHolder) -> [String; 3] {
    let c = o.holder.cell;
    let f = |v: i32, half: bool| fmt_f64(f64::from(v) + if half { 0.5 } else { 0.0 });
    let frame = o.holder.facing.is_some();
    [f(c[0], true), f(c[1], frame), f(c[2], true)]
}

/// Whether the holder starts holding an item: a `loot` fill fills it at
/// `setup_finish`; a `collect`'s stack waits for its activation.
fn starts_filled(o: &OwnedHolder) -> bool {
    o.fills.iter().any(|f| f.stack.is_some())
}

/// The full NBT the engine summons one holder with (spec-0111 §4.1).
pub(crate) fn summon_nbt(o: &OwnedHolder) -> String {
    let h = &o.holder;
    let mut parts: Vec<String> = Vec::new();
    match h.kind.family {
        HolderFamily::Frame => {
            let open = starts_filled(o);
            parts.push(format!(
                "Facing:{}b",
                h.facing.expect("a frame has a facing").byte()
            ));
            // `BlockAttachedEntity.readAdditionalSaveData` reads `block_pos`
            // and refuses one further than 16 blocks from where the body
            // stands as it loads, logging an invalid position; the summon
            // loads the NBT before it moves the body, so `Pos` is written too,
            // and the body loads where it hangs. Both are the engine's
            // derivations from the piece.
            let p = summon_pos(o);
            parts.push(format!("Pos:[{}d,{}d,{}d]", p[0], p[1], p[2]));
            parts.push(format!(
                "block_pos:[I;{},{},{}]",
                h.cell[0], h.cell[1], h.cell[2]
            ));
            for (k, v) in &h.scene {
                parts.push(format!("{k}:{}", snbt(v)));
            }
            parts.push(format!("Fixed:{}b", u8::from(!open)));
            parts.push(format!("Invulnerable:{}b", u8::from(!open)));
            parts.push("ItemDropChance:1.0f".to_string());
        }
        HolderFamily::Stand => {
            parts.push(format!("Rotation:[{}f,0.0f]", fmt_f64(f64::from(h.yaw))));
            for (k, v) in &h.scene {
                if k == "ShowArms" {
                    continue;
                }
                parts.push(format!("{k}:{}", snbt(v)));
            }
            parts.push(format!("ShowArms:{}b", u8::from(o.show_arms())));
            parts.push("Marker:0b".to_string());
            parts.push("Invulnerable:1b".to_string());
            parts.push(format!("DisabledSlots:{}", o.disabled_slots()));
        }
        HolderFamily::Container => unreachable!("a container is a block, never summoned"),
    }
    parts.push(format!("Tags:[{FIXTURE_NBT}\"dw_holder\",\"{}\"]", o.tag()));
    format!("{{{}}}", parts.join(","))
}

/// The name of a holder's own body function: `holder_<id>`.
pub(crate) fn holder_fn(o: &OwnedHolder) -> String {
    format!("holder_{}", o.id)
}

/// **One holder's own body** (spec-0111 §4.1): summon it, unless it already
/// stands, with the NBT the engine owns, then fill every slot a `loot` entry
/// claims. Idempotent, so the PackTest that drives it leaves one body.
pub(crate) fn holder_body(o: &OwnedHolder) -> Vec<String> {
    let p = summon_pos(o);
    let mut out = vec![format!(
        "execute unless entity @e[tag={}] run summon {} {} {} {} {}",
        o.tag(),
        o.holder.kind.id,
        p[0],
        p[1],
        p[2],
        summon_nbt(o)
    )];
    for f in &o.fills {
        let Some(it) = &f.stack else { continue };
        out.push(fill_line(o, &f.slot, it));
    }
    out
}

/// `setup_finish`: run every holder's own body, in
/// [`crate::compiler::holders::own_holders`] order. Empty for a campaign whose
/// pieces carry no holder, so its `setup_finish` is byte-identical.
pub(super) fn holder_setup(ns: &str, holders: &[OwnedHolder]) -> Vec<String> {
    holders
        .iter()
        .map(|o| format!("function {ns}:{}", holder_fn(o)))
        .collect()
}

/// Every holder's body function, `(name, body)`.
pub(super) fn holder_fns(holders: &[OwnedHolder]) -> Vec<(String, String)> {
    holders
        .iter()
        .map(|o| (holder_fn(o), lines(&holder_body(o))))
        .collect()
}

/// One `item replace entity` line filling `slot` of a holder with `it`.
fn fill_line(o: &OwnedHolder, slot: &str, it: &crate::compiler::plan::LootItemPlan) -> String {
    format!(
        "item replace entity {} {slot} with {}{} {}",
        o.selector(),
        it.item,
        container_stack_components(&it.item, it.name.as_deref(), &it.enchantments),
        it.count
    )
}

/// The `tick` line that freezes a frame its player has emptied (spec-0111
/// §4.3), or `None` when no holder is a frame.
pub(super) fn frame_freeze_line(ns: &str, holders: &[OwnedHolder]) -> Option<String> {
    holders
        .iter()
        .any(|o| o.holder.kind.family == HolderFamily::Frame)
        .then(|| {
            format!(
                "execute as @e[type=#{ns}:{FRAMES_TAG},tag=dw_holder] unless items entity @s \
                 contents * run data merge entity @s {{Fixed:1b,Invulnerable:1b}}"
            )
        })
}

/// The `#<ns>:frames` entity-type tag's values: the two frame kinds.
pub(super) fn frames_tag_values() -> Vec<&'static str> {
    delvewright_dsl::holders::ENTITY_HOLDERS_1_21_11
        .iter()
        .filter(|k| k.family == HolderFamily::Frame)
        .map(|k| k.id)
        .collect()
}

/// The activation lines of a `collect` that adopts a holder entity: the stack
/// in the holder's first slot, and a frame unfrozen to give it up. `None` when
/// the objective adopts no holder entity.
pub(super) fn collect_holder_activation(
    holders: &[OwnedHolder],
    objective: &str,
    stack: &str,
) -> Option<Vec<String>> {
    let o = holders
        .iter()
        .find(|o| o.collect.as_deref() == Some(objective))?;
    let slot = o.fills.iter().find(|f| f.stack.is_none())?.slot.clone();
    let mut out = vec![format!(
        "item replace entity {} {slot} with {stack}",
        o.selector()
    )];
    if o.holder.kind.family == HolderFamily::Frame {
        out.push(format!(
            "data merge entity {} {{Fixed:0b,Invulnerable:0b}}",
            o.selector()
        ));
    }
    Some(out)
}

/// **What the bot is told about a `collect` that takes from a holder entity**
/// (spec-0111 §5.4), by objective id: the `holder` object a `critical-path.json`
/// format-5 collect step carries — the holder's census `kind`, the `slot` its
/// stack is in, the `click_height` that takes it from a stand (absent on a
/// frame, which is hit), and a `stance`: the walk-reachable cell nearest the
/// holder from which the stance pass proved it within reach and in sight.
pub(super) fn holder_steps(
    holders: &[OwnedHolder],
    stances: &[crate::compiler::holders::Stance],
) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    for (o, st) in holders.iter().zip(stances) {
        let Some(obj) = &o.collect else { continue };
        let Some(slot) = o
            .fills
            .iter()
            .find(|f| f.stack.is_none())
            .map(|f| f.slot.clone())
        else {
            continue;
        };
        let stance = st.nearest(o.holder.cell);
        let mut v = json!({
            "kind": o.holder.kind.token,
            "slot": slot,
        });
        if o.holder.kind.family == HolderFamily::Stand
            && let Some(h) =
                delvewright_dsl::holders::click_height(&slot, &o.claimed_slots(), o.holder.small())
        {
            v["click_height"] = json!(h);
        }
        if let Some(s) = stance {
            v["stance"] = json!(s);
        }
        out.insert(obj.clone(), v);
    }
    out
}

/// Write each holder collect's `holder` object onto its `critical-path.json`
/// step (spec-0111 §5.4).
pub(super) fn annotate_holder_steps(cp: &mut Value, steps: &BTreeMap<String, Value>) {
    if steps.is_empty() {
        return;
    }
    let Some(list) = cp.get_mut("steps").and_then(Value::as_array_mut) else {
        return;
    };
    for step in list {
        if step.get("action").and_then(Value::as_str) != Some("collect") {
            continue;
        }
        let Some(obj) = step.get("objective").and_then(Value::as_str) else {
            continue;
        };
        if let Some(h) = steps.get(obj).cloned()
            && let Some(m) = step.as_object_mut()
        {
            m.insert("holder".to_string(), h);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::holders::{EntityFill, FrameFacing, PlacedHolder};
    use std::collections::BTreeMap;

    fn holder(id: &str, family: HolderFamily) -> PlacedHolder {
        let kind = delvewright_dsl::holders::ENTITY_HOLDERS_1_21_11
            .iter()
            .find(|k| k.family == family && k.id == id)
            .unwrap();
        PlacedHolder {
            kind,
            cell: [3, 65, -2],
            facing: (family == HolderFamily::Frame).then_some(FrameFacing::North),
            yaw: 180.0,
            scene: BTreeMap::new(),
            prefab: "prefab/x".to_string(),
        }
    }

    fn stack(item: &str) -> crate::compiler::plan::LootItemPlan {
        crate::compiler::plan::LootItemPlan {
            item: item.to_string(),
            count: 1,
            name: None,
            enchantments: BTreeMap::new(),
            slot: None,
        }
    }

    #[test]
    fn a_filled_frame_is_summoned_open_and_filled_by_item_replace_entity() {
        let o = OwnedHolder {
            holder: holder("minecraft:glow_item_frame", HolderFamily::Frame),
            id: "map_frame".to_string(),
            anchor: Some("anchor/map-frame".to_string()),
            fills: vec![EntityFill {
                slot: "contents".to_string(),
                stack: Some(stack("minecraft:map")),
            }],
            loot: Some("loot/map".to_string()),
            collect: None,
        };
        assert_eq!(
            holder_setup("ns", std::slice::from_ref(&o)),
            vec!["function ns:holder_map_frame".to_string()]
        );
        let lines = holder_body(&o);
        assert_eq!(
            lines[0],
            "execute unless entity @e[tag=dw_holder_map_frame] run summon \
             minecraft:glow_item_frame 3.5 65.5 -1.5 {Facing:2b,Pos:[3.5d,65.5d,-1.5d],block_pos:[I;3,65,-2],Fixed:0b,\
             Invulnerable:0b,ItemDropChance:1.0f,Tags:[\"dw_fixture\",\"dw_holder\",\"dw_holder_map_frame\"]}"
        );
        assert_eq!(
            lines[1],
            "item replace entity @e[tag=dw_holder_map_frame,limit=1] contents with minecraft:map 1"
        );
        assert!(frame_freeze_line("ns", &[o]).is_some());
    }

    #[test]
    fn a_stand_is_invulnerable_and_locks_every_slot_but_its_fills() {
        let o = OwnedHolder {
            holder: holder("minecraft:armor_stand", HolderFamily::Stand),
            id: "rack".to_string(),
            anchor: Some("anchor/rack".to_string()),
            fills: vec![EntityFill {
                slot: "weapon.mainhand".to_string(),
                stack: Some(stack("minecraft:iron_sword")),
            }],
            loot: Some("loot/rack".to_string()),
            collect: None,
        };
        let nbt = summon_nbt(&o);
        assert!(nbt.contains("ShowArms:1b"), "{nbt}");
        assert!(nbt.contains("Marker:0b,Invulnerable:1b"), "{nbt}");
        let bits = delvewright_dsl::holders::disabled_slots(&["weapon.mainhand"]);
        assert!(nbt.contains(&format!("DisabledSlots:{bits}")), "{nbt}");
        assert!(frame_freeze_line("ns", &[o]).is_none());
    }
}
