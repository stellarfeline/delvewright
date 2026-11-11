//! **The item stack, written** (spec-0134 §5): the components a
//! [`delvewright_dsl::Stack`] carries, rendered in each of the three syntaxes
//! the game reads a stack in — a command's item argument (`give`, `item
//! replace`), an entity's NBT (a summoned body's `equipment`) and a loot
//! table's functions (a body's declared drop). Which components a stack carries
//! and what each holds is decided once, in [`components`]; the three writers
//! only spell them, so a stack written at one site and the same stack written
//! at another are the same item.

use super::*;

/// One component a stack carries: its id (with `minecraft:`), its value in
/// SNBT and its value in JSON.
pub(super) struct Component {
    id: String,
    snbt: String,
    json: Value,
}

impl Component {
    /// A component written only by a site, not by the stack (the flask's
    /// `use_remainder`): its id and SNBT value.
    pub(super) fn site(id: &str, snbt: String) -> Self {
        Component {
            id: id.to_string(),
            snbt,
            json: Value::Null,
        }
    }
}

/// **Every component the stack carries**, in the one fixed order — the display
/// name, the enchantments (stored, on a book), the potion's contents, the
/// rocket's stars. A stack that declares none of them carries none, so a plain
/// stack is written bare.
pub(super) fn components(s: &delvewright_dsl::Stack) -> Vec<Component> {
    let mut out = Vec::new();
    if let Some(n) = &s.name {
        let v = tr_with(n, &[("italic", json!(false))]);
        out.push(Component {
            id: "minecraft:custom_name".to_string(),
            snbt: v.to_string(),
            json: v,
        });
    }
    if !s.enchantments.is_empty() {
        let body = s
            .enchantments
            .iter()
            .map(|(id, lvl)| format!("\"{id}\":{lvl}"))
            .collect::<Vec<_>>()
            .join(",");
        let json: serde_json::Map<String, Value> = s
            .enchantments
            .iter()
            .map(|(id, lvl)| (id.clone(), json!(lvl)))
            .collect();
        out.push(Component {
            id: delvewright_dsl::enchantment_component(&s.item).to_string(),
            snbt: format!("{{{body}}}"),
            json: Value::Object(json),
        });
    }
    if let Some(pc) = &s.contents {
        let mut j = serde_json::Map::new();
        if let Some(p) = &pc.potion {
            j.insert("potion".into(), json!(p));
        }
        if !pc.effects.is_empty() {
            let effs: Vec<Value> = pc
                .effects
                .iter()
                .map(|e| {
                    let mut o = serde_json::Map::new();
                    o.insert("id".into(), json!(e.effect));
                    if let Some(d) = e.duration {
                        o.insert("duration".into(), json!(d));
                    }
                    if let Some(a) = e.amplifier {
                        o.insert("amplifier".into(), json!(a));
                    }
                    Value::Object(o)
                })
                .collect();
            j.insert("custom_effects".into(), Value::Array(effs));
        }
        if let Some(v) = pc.color.as_deref().and_then(delvewright_dsl::color::packed) {
            j.insert("custom_color".into(), json!(v));
        }
        out.push(Component {
            id: "minecraft:potion_contents".to_string(),
            snbt: super::class::potion_contents_snbt(pc),
            json: Value::Object(j),
        });
    }
    if let Some(fw) = &s.fireworks {
        let flight = fw.flight.unwrap_or(delvewright_dsl::firework::MIN_FLIGHT);
        let packed = |list: &[String]| -> Vec<Value> {
            list.iter()
                .filter_map(|c| delvewright_dsl::color::packed(c))
                .map(|n| json!(n))
                .collect()
        };
        let bursts: Vec<Value> = fw
            .explosions
            .iter()
            .map(|e| {
                let mut o = serde_json::Map::new();
                o.insert("shape".into(), json!(e.shape.token()));
                o.insert("colors".into(), Value::Array(packed(&e.colors)));
                if !e.fade_colors.is_empty() {
                    o.insert("fade_colors".into(), Value::Array(packed(&e.fade_colors)));
                }
                if e.trail {
                    o.insert("has_trail".into(), json!(true));
                }
                if e.twinkle {
                    o.insert("has_twinkle".into(), json!(true));
                }
                Value::Object(o)
            })
            .collect();
        out.push(Component {
            id: delvewright_dsl::firework::FIREWORKS_COMPONENT.to_string(),
            snbt: super::firework::fireworks_value(flight, &fw.explosions),
            json: json!({"flight_duration": flight, "explosions": bursts}),
        });
    }
    out
}

/// The stack's item argument tail, `[name=…,…]`, as `give` and `item replace`
/// read it, with the site's own components after the stack's; `""` when there
/// are none.
pub(super) fn command_suffix(s: &delvewright_dsl::Stack, site: Vec<Component>) -> String {
    let all: Vec<String> = components(s)
        .into_iter()
        .chain(site)
        .map(|c| {
            format!(
                "{}={}",
                c.id.strip_prefix("minecraft:").unwrap_or(&c.id),
                c.snbt
            )
        })
        .collect();
    if all.is_empty() {
        String::new()
    } else {
        format!("[{}]", all.join(","))
    }
}

/// The stack as an entity's item NBT, `{id:"…",count:N[,components:{…}]}` —
/// a summoned body's worn piece.
pub(super) fn nbt(s: &delvewright_dsl::Stack) -> String {
    let comps = components(s);
    let tail = if comps.is_empty() {
        String::new()
    } else {
        format!(
            ",components:{{{}}}",
            comps
                .iter()
                .map(|c| format!("\"{}\":{}", c.id, c.snbt))
                .collect::<Vec<_>>()
                .join(",")
        )
    };
    format!("{{id:\"{}\",count:{}{tail}}}", s.item, s.count)
}

/// The stack as a loot-table entry: `minecraft:item`, a `set_count` when there
/// is more than one, and a `set_components` carrying every component.
pub(super) fn loot_entry(s: &delvewright_dsl::Stack) -> Value {
    let mut entry = json!({ "type": "minecraft:item", "name": s.item });
    let mut functions: Vec<Value> = Vec::new();
    if s.count != 1 {
        functions.push(json!({"function": "minecraft:set_count", "count": s.count}));
    }
    let comps = components(s);
    if !comps.is_empty() {
        let map: serde_json::Map<String, Value> =
            comps.into_iter().map(|c| (c.id, c.json)).collect();
        functions.push(json!({"function": "minecraft:set_components", "components": map}));
    }
    if !functions.is_empty() {
        entry["functions"] = Value::Array(functions);
    }
    entry
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rocket() -> delvewright_dsl::Stack {
        serde_json::from_value(json!({
            "item": "minecraft:firework_rocket", "count": 2, "name": "Signal",
            "enchantments": {"minecraft:unbreaking": 1},
            "fireworks": {"explosions": [{"shape": "burst", "colors": ["#ff0000"]}]}
        }))
        .unwrap()
    }

    /// The three writers spell the same components in the same order.
    #[test]
    fn the_three_writers_agree() {
        let s = rocket();
        let cmd = command_suffix(&s, vec![]);
        assert!(cmd.starts_with("[custom_name="), "{cmd}");
        assert!(
            cmd.contains(
                ",enchantments={\"minecraft:unbreaking\":1},fireworks={flight_duration:1b"
            ),
            "{cmd}"
        );
        let n = nbt(&s);
        assert!(
            n.starts_with(
                "{id:\"minecraft:firework_rocket\",count:2,components:{\"minecraft:custom_name\":"
            ),
            "{n}"
        );
        assert!(
            n.contains("\"minecraft:fireworks\":{flight_duration:1b"),
            "{n}"
        );
        let e = loot_entry(&s);
        let comps = &e["functions"][1]["components"];
        assert_eq!(comps["minecraft:fireworks"]["flight_duration"], 1);
        assert_eq!(
            comps["minecraft:fireworks"]["explosions"][0]["colors"][0],
            16711680
        );
        assert_eq!(comps["minecraft:enchantments"]["minecraft:unbreaking"], 1);
        assert_eq!(e["functions"][0]["count"], 2);
        // A bare stack is written bare everywhere.
        let bare = delvewright_dsl::Stack::of("minecraft:stick");
        assert_eq!(command_suffix(&bare, vec![]), "");
        assert_eq!(nbt(&bare), "{id:\"minecraft:stick\",count:1}");
        assert_eq!(
            loot_entry(&bare),
            json!({"type": "minecraft:item", "name": "minecraft:stick"})
        );
    }
}
