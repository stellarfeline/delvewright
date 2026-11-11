//! The `firework` verb.

use super::*;

/// Emit a `firework` effect (DSL v0.29, spec-0068): one `summon` of a
/// `minecraft:firework_rocket` at the mark's cell centre, carrying the bursts as
/// a `minecraft:fireworks` item component.
///
/// **`LifeTime` is written, never left to the game.** Unset, vanilla randomises
/// it at launch, so two runs of one datapack would burst at two heights and
/// `DW0899`'s proof would be about a number nobody chose. The emitter writes the
/// floor of that range ([`delvewright_dsl::firework::lifetime_ticks`]), which is
/// both deterministic (ADR-0006) and the conservative side of the height proof.
///
/// Every spelling in the line — the entity, the item field, the component and
/// the five shape tokens — comes from [`delvewright_dsl::firework`], the one
/// file the game facts are pinned in, so a re-pin moves this command without
/// touching this function.
///
/// The audience selector is not consulted: a rocket is a body in the world, not
/// something played at a listener, so every player present sees the same burst.
/// An unresolved anchor emits nothing and is `DW0360` long before here.
pub(super) fn emit_firework(
    plan: &Plan,
    at: &delvewright_dsl::Mark,
    flight: Option<u8>,
    explosions: &[delvewright_dsl::FireworkExplosion],
    body: &mut Vec<String>,
) {
    use delvewright_dsl::firework;
    let Some(anchor) = anchor_point_any(plan, at.anchor.as_str()) else {
        return; // unresolved anchor (`DW0360` owns it)
    };
    let cell = at.cell(anchor);
    let v = ent_xyz(cell);
    let flight = flight.unwrap_or(firework::MIN_FLIGHT);
    let value = fireworks_value(flight, explosions);
    body.push(format!(
        "summon {entity} {x} {y} {z} {{LifeTime:{life},{item}:{{id:\"{item_id}\",count:1,\
         components:{{\"{component}\":{value}}}}}}}",
        entity = firework::ROCKET_ENTITY,
        x = v[0],
        y = v[1],
        z = v[2],
        life = firework::lifetime_ticks(flight),
        item = firework::ITEM_FIELD,
        item_id = firework::ROCKET_ITEM,
        component = firework::FIREWORKS_COMPONENT,
    ));
}

/// **The `minecraft:fireworks` component's value** — `{flight_duration:Nb,
/// explosions:[…]}` — one renderer for every rocket the engine writes: the
/// `firework` effect's summoned rocket and a stack's `fireworks` (spec-0134
/// §5), so the two cannot disagree about a star. Colours are the packed
/// integers the component reads; validation proved every literal well-formed
/// (`DW0100`), so a colour that will not pack never reaches here.
pub(super) fn fireworks_value(
    flight: u8,
    explosions: &[delvewright_dsl::FireworkExplosion],
) -> String {
    let packed = |list: &[String]| -> String {
        list.iter()
            .filter_map(|c| delvewright_dsl::color::packed(c))
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(",")
    };
    let bursts: Vec<String> = explosions
        .iter()
        .map(|e| {
            let mut f = vec![
                format!("shape:\"{}\"", e.shape.token()),
                format!("colors:[I;{}]", packed(&e.colors)),
            ];
            if !e.fade_colors.is_empty() {
                f.push(format!("fade_colors:[I;{}]", packed(&e.fade_colors)));
            }
            if e.trail {
                f.push("has_trail:1b".to_string());
            }
            if e.twinkle {
                f.push("has_twinkle:1b".to_string());
            }
            format!("{{{}}}", f.join(","))
        })
        .collect();
    format!(
        "{{flight_duration:{flight}b,explosions:[{}]}}",
        bursts.join(",")
    )
}

/// The `fireworks=…` component a rocket STACK carries (spec-0134 §5): the
/// stack's flight (default [`delvewright_dsl::firework::MIN_FLIGHT`]) and its
/// stars, in the item-stack syntax `give` and `item replace` read.
pub(super) fn stack_fireworks_component(fw: &delvewright_dsl::StackFireworks) -> String {
    format!(
        "fireworks={}",
        fireworks_value(
            fw.flight.unwrap_or(delvewright_dsl::firework::MIN_FLIGHT),
            &fw.explosions
        )
    )
}
