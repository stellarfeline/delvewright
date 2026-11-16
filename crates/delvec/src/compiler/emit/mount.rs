//! A body that rides another (spec-0135 §13): the mount a wave mob is
//! summoned on, through the pinned game's `Passengers` entity data —
//! the shape vanilla's own mounted spawn takes
//! (`docs/reference/vehicles-animals-and-targets.md` §16).

use super::*;
use delvewright_dsl::{EquipSlot, Mount};

/// The tag every mount the engine summons carries.
pub(super) const MOUNT_TAG: &str = "dw_mount";

/// The tag the mounts of wave `wave`'s bodies carry. Never the wave's own tag:
/// the wave's kill count, census and muster read riders, and a mount killed is
/// no member of the wave felled.
pub(crate) fn wave_mount_tag(wave: &str) -> String {
    format!("dw_wmount_{}", plan::safe_local(wave))
}

/// The mount's own compound, inside the braces: its tags, then `extra`, then
/// what it wears (every drop chance zero: a mount is never loot) and its
/// attribute overrides.
fn mount_compound(m: &Mount, tags: &[String], extra: &[String]) -> String {
    let mut parts: Vec<String> = vec![format!(
        "Tags:[{}]",
        std::iter::once(MOUNT_TAG.to_string())
            .chain(tags.iter().cloned())
            .map(|t| format!("\"{t}\""))
            .collect::<Vec<_>>()
            .join(",")
    )];
    parts.extend(extra.iter().cloned());
    if let Some(eq) = &m.equipment {
        let mut items: Vec<String> = Vec::new();
        let mut chances: Vec<String> = Vec::new();
        for slot in EquipSlot::ALL {
            if let Some(p) = eq.filled(slot) {
                items.push(format!("{}:{}", slot.nbt(), piece_nbt(p.item(), Some(p))));
                chances.push(format!("{}:0.0f", slot.nbt()));
            }
        }
        if !items.is_empty() {
            parts.push(format!(
                "equipment:{{{}}},drop_chances:{{{}}}",
                items.join(","),
                chances.join(",")
            ));
        }
    }
    let attrs = attributes_snbt(m.attributes.as_ref());
    if let Some(a) = attrs.strip_prefix(',') {
        parts.push(a.to_string());
    }
    // DSL v0.83: the mount's variant, by the one emission rule.
    if let Some(v) = delvewright_dsl::variant_nbt(&m.entity, m.variant.as_ref()).strip_prefix(',') {
        parts.push(v.to_string());
    }
    parts.join(",")
}

/// **A summon put on a mount**: `summon <rider> <pos> {<rider data>}` becomes
/// `summon <mount> <pos> {<mount data>,Passengers:[{id:"<rider>",<rider
/// data>}]}` — the rider arrives aboard, at the mount's seat, in the one
/// command. `None` for the mount leaves the summon as written.
pub(super) fn ride_on(
    summon: &str,
    m: Option<&Mount>,
    tags: &[String],
    extra: &[String],
) -> String {
    let Some(m) = m else {
        return summon.to_string();
    };
    let rest = summon
        .strip_prefix("summon ")
        .expect("a rider's summon starts with `summon`");
    let (rider, rest) = rest
        .split_once(' ')
        .expect("a rider's summon names the entity and a position");
    let (pos, data) = match rest.find('{') {
        Some(i) => (rest[..i].trim_end(), &rest[i..]),
        None => (rest.trim_end(), "{}"),
    };
    let inner = &data[1..data.len() - 1];
    let passenger = if inner.is_empty() {
        format!("{{id:\"{rider}\"}}")
    } else {
        format!("{{id:\"{rider}\",{inner}}}")
    };
    format!(
        "summon {} {pos} {{{},Passengers:[{passenger}]}}",
        delvewright_dsl::namespaced_entity(&m.entity),
        mount_compound(m, tags, extra)
    )
}

/// The mount a wave body is summoned on, or `None`.
pub(super) fn wave_mob_ride_on(
    w: &delvewright_dsl::Wave,
    mob: &delvewright_dsl::WaveMob,
    summon: String,
) -> String {
    match &mob.mount {
        None => summon,
        Some(m) => ride_on(
            &summon,
            Some(m),
            &[wave_mount_tag(w.id.as_str())],
            &["PersistenceRequired:1b".to_string()],
        ),
    }
}

/// Whether any stack of wave `w` rides.
pub(super) fn wave_rides(w: &delvewright_dsl::Wave) -> bool {
    w.mobs.iter().any(|m| m.mount.is_some())
}

/// The removal of a wave's mounts, after its riders' (spec-0135 §13): the
/// mounts leave unseen with the wave. Empty for a wave nobody rides.
pub(super) fn wave_mount_removal(ns: &str, w: &delvewright_dsl::Wave) -> Vec<String> {
    if !wave_rides(w) {
        return Vec::new();
    }
    removal_lines(ns, &wave_mount_tag(w.id.as_str()), false, Exit::Unseen)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn horse() -> Mount {
        Mount {
            entity: "minecraft:zombie_horse".to_string(),
            variant: None,
            equipment: None,
            attributes: None,
        }
    }

    #[test]
    fn a_summon_is_put_on_its_mount_as_its_passenger() {
        let s = "summon minecraft:zombie 1.5 64 2.5 {Tags:[\"dw_wave_x\"],PersistenceRequired:1b}";
        assert_eq!(
            ride_on(s, Some(&horse()), &["dw_wmount_x".to_string()], &[]),
            "summon minecraft:zombie_horse 1.5 64 2.5 {Tags:[\"dw_mount\",\"dw_wmount_x\"],\
             Passengers:[{id:\"minecraft:zombie\",Tags:[\"dw_wave_x\"],PersistenceRequired:1b}]}"
        );
        assert_eq!(ride_on(s, None, &[], &[]), s);
        // DSL v0.83: the mount's variant rides in its own compound, never the rider's.
        let mut pig = horse();
        pig.entity = "minecraft:pig".to_string();
        pig.variant = Some(delvewright_dsl::BodyVariant(
            [("variant".to_string(), "minecraft:warm".to_string())].into(),
        ));
        assert_eq!(
            ride_on(s, Some(&pig), &[], &[]),
            "summon minecraft:pig 1.5 64 2.5 {Tags:[\"dw_mount\"],variant:\"minecraft:warm\",\
             Passengers:[{id:\"minecraft:zombie\",Tags:[\"dw_wave_x\"],PersistenceRequired:1b}]}"
        );
        assert_eq!(
            ride_on("summon minecraft:pig ~ ~ ~", Some(&horse()), &[], &[]),
            "summon minecraft:zombie_horse ~ ~ ~ {Tags:[\"dw_mount\"],Passengers:[{id:\"minecraft:pig\"}]}"
        );
    }
}
