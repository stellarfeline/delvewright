//! **The item stack** — the one shape every site that puts an item somewhere
//! writes (spec-0134 §5): a class kit entry, a `loot` fill, a `give-item`, a
//! worn piece of `equipment` and a body's declared `drops`. A capability of a
//! stack (a display name, enchantments, a potion's contents, a rocket's stars)
//! belongs to the stack, so it is declared here once and every site carries it;
//! a site adds only what is its own (a kit's `carrier` and `flask`, a fill's
//! `slot`, a give's `carrier`).
//!
//! The sites embed [`Stack`] with `#[serde(flatten)]`, so the JSON of every
//! site is the stack's keys beside the site's own. [`for_each_stack`] is the one
//! enumeration of the sites, and [`stack_checks`] asks every stack the same
//! questions: the item is in the pinned registry (`DW0143`), its enchantments
//! are real and representable (`DW0433`/`DW0434`), a potion pours something
//! (`DW0486`/`DW0487`), and stars sit on a rocket (`DW1095`).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::diagnostic::{Diagnostic, codes};
use crate::envelope::Campaign;
use crate::registry::{EffectRegistry, ItemRegistry};
use crate::{ActorId, EquipSlot, PotionContents, QuestEffect, StackFireworks, Verb};

/// One item stack: the item, how many, and the components the game writes on
/// it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Stack {
    /// Vanilla item id, validated against the pinned 1.21.11 registry
    /// (`DW0143`).
    pub item: String,
    /// Stack size. Defaults to 1.
    #[serde(default = "one_u32")]
    pub count: u32,
    /// Display name, written as the `minecraft:custom_name` component.
    /// Player-visible, so it enters the l10n string inventory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Enchantment id → level (`{"minecraft:sharpness": 3}`), emitted as the
    /// `minecraft:enchantments` component — or, on a `minecraft:enchanted_book`,
    /// `minecraft:stored_enchantments` ([`crate::enchantment_component`]).
    /// Checked by `DW0433`/`DW0434`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub enchantments: BTreeMap<String, u32>,
    /// **What is in the bottle** (spec-0016 §1): the `minecraft:potion_contents`
    /// component of a potion-bearing item ([`crate::POTION_BEARING_ITEMS`]).
    /// A potion-bearing stack that declares none is the Uncraftable Potion
    /// (`DW0487`); contents on another item, or contents the game cannot pour,
    /// are `DW0486`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contents: Option<PotionContents>,
    /// **A rocket's stars** (DSL v0.73, spec-0134 §5): the `minecraft:fireworks`
    /// component of a `minecraft:firework_rocket` stack. On any other item it is
    /// `DW1095`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fireworks: Option<StackFireworks>,
}

fn one_u32() -> u32 {
    1
}

/// The keys [`Stack`] reads, in declaration order — what a site that must tell
/// a stack's keys from its own asks ([`GiveSource`]'s reader). Held equal to the
/// exported schema's property set by a unit test.
pub const STACK_FIELDS: &[&str] = &[
    "item",
    "count",
    "name",
    "enchantments",
    "contents",
    "fireworks",
];

impl Stack {
    /// A bare stack of one `item`, with no components.
    pub fn of(item: impl Into<String>) -> Self {
        Stack {
            item: item.into(),
            count: 1,
            name: None,
            enchantments: BTreeMap::new(),
            contents: None,
            fireworks: None,
        }
    }

    /// A bare stack of `count` × `item`.
    pub fn counted(item: impl Into<String>, count: u32) -> Self {
        Stack {
            count,
            ..Stack::of(item)
        }
    }
}

/// Where a `give-item` stack comes from (spec-0134 §8): written out, or taken
/// off an actor — `{"from": {"actor": id, "slot": slot}}`, the stack the
/// actor's `equipment` declares in that slot, emptied from the actor in the
/// same bundle. A taken stack is never spelled a second time: `from` beside any
/// stack key is refused (`DW0100`).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum GiveSource {
    /// A stack written out.
    Stack(Stack),
    /// The stack an actor holds in a slot.
    Taken {
        /// The actor and its slot.
        from: TakenFrom,
    },
}

/// An actor's equipment slot, the source of a taken stack (spec-0134 §8).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TakenFrom {
    /// The actor (`actor/<kebab>`) whose equipment holds the stack (`DW0112`).
    pub actor: ActorId,
    /// The slot it is taken from; the actor's `equipment` must fill it
    /// (`DW0490`).
    pub slot: EquipSlot,
}

impl<'de> Deserialize<'de> for GiveSource {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        use serde::de::{Error, MapAccess, Visitor};
        struct Keys;
        impl<'de> Visitor<'de> for Keys {
            type Value = serde_json::Map<String, serde_json::Value>;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an item stack, or `from` an actor's slot")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut m: A) -> Result<Self::Value, A::Error> {
                let mut out = serde_json::Map::new();
                while let Some((k, v)) = m.next_entry::<String, serde_json::Value>()? {
                    out.insert(k, v);
                }
                Ok(out)
            }
        }
        // Named, so a flattening reader hands over exactly these keys and
        // leaves every other one to the enclosing object's own unknown-field
        // rule.
        static KEYS: std::sync::LazyLock<Vec<&'static str>> = std::sync::LazyLock::new(|| {
            let mut k = STACK_FIELDS.to_vec();
            k.push("from");
            k
        });
        let map = de.deserialize_struct("GiveSource", &KEYS, Keys)?;
        if let Some(from) = map.get("from") {
            if let Some(k) = map.keys().find(|k| *k != "from") {
                return Err(D::Error::custom(format!(
                    "`from` gives the stack the actor holds, so `{k}` would spell it a second \
                     time — remove `{k}`, or remove `from` and write the stack out"
                )));
            }
            return serde_json::from_value(from.clone())
                .map(|from| GiveSource::Taken { from })
                .map_err(D::Error::custom);
        }
        Stack::deserialize(serde_json::Value::Object(map))
            .map(GiveSource::Stack)
            .map_err(D::Error::custom)
    }
}

impl JsonSchema for GiveSource {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "GiveSource".into()
    }

    fn json_schema(g: &mut schemars::SchemaGenerator) -> schemars::Schema {
        // The stack's own object, open: the enclosing verb's keys sit beside it.
        let mut stack = <Stack as JsonSchema>::json_schema(g);
        if let Some(o) = stack.as_object_mut() {
            o.remove("additionalProperties");
            o.remove("title");
            o.remove("description");
        }
        let from = g.subschema_for::<TakenFrom>();
        schemars::json_schema!({
            "description": "An item stack written out, or `from` an actor's equipment slot \
                            (the stack it holds there, emptied from it in the same bundle).",
            "anyOf": [
                stack,
                {
                    "type": "object",
                    "properties": { "from": from },
                    "required": ["from"]
                }
            ]
        })
    }
}

impl GiveSource {
    /// The written stack, or `None` for a taken one.
    pub fn stack(&self) -> Option<&Stack> {
        match self {
            GiveSource::Stack(s) => Some(s),
            GiveSource::Taken { .. } => None,
        }
    }

    /// The actor slot a taken stack comes from.
    pub fn taken(&self) -> Option<&TakenFrom> {
        match self {
            GiveSource::Taken { from } => Some(from),
            GiveSource::Stack(_) => None,
        }
    }

    /// The stack this give hands over: the written one, or the one the named
    /// actor's `equipment` declares in the slot (`None` when the actor or the
    /// slot does not resolve — `DW0112`/`DW0490`).
    pub fn resolve<'a>(&'a self, c: &'a Campaign) -> Option<std::borrow::Cow<'a, Stack>> {
        match self {
            GiveSource::Stack(s) => Some(std::borrow::Cow::Borrowed(s)),
            GiveSource::Taken { from } => {
                let a = c
                    .quests
                    .content
                    .actors
                    .iter()
                    .find(|a| a.id == from.actor)?;
                a.equipment.as_ref()?.filled(from.slot).map(|p| p.stack())
            }
        }
    }
}

/// Which kind of site a stack stands at.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum StackSiteKind {
    /// A class kit entry.
    Kit,
    /// A `loot` fill's stack.
    Loot,
    /// A `give-item` at any effect root.
    Give,
    /// A worn piece of a wave mob's or an actor's `equipment`.
    Equipment,
    /// A body's declared quest-item drop.
    Drop,
}

impl StackSiteKind {
    /// Every site kind, in the order [`for_each_stack`] visits them.
    pub const ALL: [StackSiteKind; 5] = [
        StackSiteKind::Kit,
        StackSiteKind::Loot,
        StackSiteKind::Give,
        StackSiteKind::Equipment,
        StackSiteKind::Drop,
    ];

    /// The words a diagnostic names the site by.
    pub fn word(self) -> &'static str {
        match self {
            StackSiteKind::Kit => "kit item",
            StackSiteKind::Loot => "loot item",
            StackSiteKind::Give => "`give-item` item",
            StackSiteKind::Equipment => "equipment",
            StackSiteKind::Drop => "drop",
        }
    }
}

/// One stack where it stands.
#[derive(Clone, Debug)]
pub struct StackAt<'a> {
    /// The site kind.
    pub kind: StackSiteKind,
    /// The stage document it lives in.
    pub stage: &'static str,
    /// JSON pointer to the stack's object; for a worn piece written as a bare
    /// id, the pointer to that string.
    pub path: String,
    /// Whether the stack is written as a bare item id (a worn piece's plain
    /// form): its item is at `path` itself, not at `path/item`.
    pub bare: bool,
    /// The stack.
    pub stack: std::borrow::Cow<'a, Stack>,
}

impl StackAt<'_> {
    /// The pointer to the stack's `item`.
    pub fn item_path(&self) -> String {
        if self.bare {
            self.path.clone()
        } else {
            format!("{}/item", self.path)
        }
    }
}

/// **Every item stack the campaign writes**, at every site: class kits, `loot`
/// fills, `give-item` at every effect root and nesting depth, the worn pieces of
/// wave mobs and actors, and their declared quest-item drops. A taken
/// `give-item` (`from`) is not a stack of its own — it is the actor's worn
/// piece, visited there.
pub fn for_each_stack<'a>(c: &'a Campaign, f: &mut dyn FnMut(StackAt<'a>)) {
    use std::borrow::Cow;
    for (i, cl) in c.classes.content.classes.iter().enumerate() {
        for (k, it) in cl.kit.iter().enumerate() {
            f(StackAt {
                kind: StackSiteKind::Kit,
                stage: "classes",
                path: format!("/content/classes/{i}/kit/{k}"),
                bare: false,
                stack: Cow::Borrowed(&it.stack),
            });
        }
    }
    let q = &c.quests.content;
    for (i, l) in q.loot.iter().enumerate() {
        for (k, it) in l.items.iter().enumerate() {
            f(StackAt {
                kind: StackSiteKind::Loot,
                stage: "quests",
                path: format!("/content/loot/{i}/items/{k}"),
                bare: false,
                stack: Cow::Borrowed(&it.stack),
            });
        }
    }
    fn descend<'a>(
        stage: &'static str,
        path: String,
        eff: &'a QuestEffect,
        f: &mut dyn FnMut(StackAt<'a>),
    ) {
        if let Verb::GiveItem {
            source: GiveSource::Stack(s),
            ..
        } = &eff.verb
        {
            f(StackAt {
                kind: StackSiteKind::Give,
                stage,
                path: path.clone(),
                bare: false,
                stack: std::borrow::Cow::Borrowed(s),
            });
        }
        for (pseg, _kseg, list) in eff.nested_effect_lists_labeled() {
            for (j, inner) in list.iter().enumerate() {
                descend(stage, format!("{path}/{pseg}/{j}"), inner, f);
            }
        }
    }
    crate::effects::for_each_effect_root(c, &mut |site, effs| {
        for (i, eff) in effs.iter().enumerate() {
            descend(site.stage, format!("{}/{i}", site.path), eff, f);
        }
    });
    let worn = |base: String, eq: &'a crate::MobEquipment, f: &mut dyn FnMut(StackAt<'a>)| {
        for (slot, piece) in eq.slots() {
            let Some(p) = piece else { continue };
            f(StackAt {
                kind: StackSiteKind::Equipment,
                stage: "quests",
                path: format!("{base}/{slot}"),
                bare: matches!(p, crate::EquipItem::Plain(_)),
                stack: p.stack(),
            });
        }
    };
    for (i, w) in q.waves.iter().enumerate() {
        for (k, m) in w.mobs.iter().enumerate() {
            if let Some(eq) = &m.equipment {
                worn(format!("/content/waves/{i}/mobs/{k}/equipment"), eq, f);
            }
        }
    }
    for (i, a) in q.actors.iter().enumerate() {
        if let Some(eq) = &a.equipment {
            worn(format!("/content/actors/{i}/equipment"), eq, f);
        }
    }
    let dropped = |base: String, drops: &'a [crate::MobDrop], f: &mut dyn FnMut(StackAt<'a>)| {
        for (n, dr) in drops.iter().enumerate() {
            if let crate::MobDrop::Item(s) = dr {
                f(StackAt {
                    kind: StackSiteKind::Drop,
                    stage: "quests",
                    path: format!("{base}/{n}"),
                    bare: false,
                    stack: Cow::Borrowed(s),
                });
            }
        }
    };
    for (i, w) in q.waves.iter().enumerate() {
        for (k, m) in w.mobs.iter().enumerate() {
            dropped(format!("/content/waves/{i}/mobs/{k}/drops"), &m.drops, f);
        }
    }
    for (i, a) in q.actors.iter().enumerate() {
        dropped(format!("/content/actors/{i}/drops"), &a.drops, f);
    }
}

/// **Every stack, asked the same questions** (spec-0134 §5): the item is in
/// the pinned registry (`DW0143`); its enchantments are real and of a level the
/// component stores (`DW0433`/`DW0434`); a potion-bearing stack pours something
/// and nothing else carries contents (`DW0486`/`DW0487`); stars sit on a rocket
/// (`DW1095`) within the bounds a `firework` effect's are (`DW0100`).
pub(crate) fn stack_checks(
    c: &Campaign,
    items: &dyn ItemRegistry,
    effects: &dyn EffectRegistry,
    d: &mut Vec<Diagnostic>,
) {
    let ench_reg = crate::registry::VendoredEnchantmentRegistry::v1_21_11();
    let mut bounds: Vec<(String, &'static str, String)> = Vec::new();
    for_each_stack(c, &mut |at| {
        let s = &*at.stack;
        let what = format!("{} `{}`", at.kind.word(), s.item);
        if !items.contains(&s.item) {
            d.push(Diagnostic::error(
                codes::ITEM_UNKNOWN,
                at.stage,
                at.item_path(),
                format!(
                    "{} `{}` is not in the pinned 1.21.11 item registry — use a valid \
                     namespaced item id (e.g. `minecraft:iron_sword`)",
                    at.kind.word(),
                    s.item
                ),
            ));
        }
        crate::wave::check_enchantments(
            &s.enchantments,
            &what,
            at.stage,
            &format!("{}/enchantments", at.path),
            &ench_reg,
            d,
        );
        crate::class::check_potion_contents(s, &what, at.stage, &at.path, effects, d);
        if let Some(fw) = &s.fireworks {
            if !crate::firework::is_rocket(&s.item) {
                d.push(Diagnostic::error(
                    crate::loot::STACK_FIREWORKS_NOT_ROCKET,
                    at.stage,
                    format!("{}/fireworks", at.path),
                    format!(
                        "a stack of `{item}` carries `fireworks`, and the game keeps a rocket's \
                         stars on `{rocket}` alone, so these stars would be written and never \
                         seen. Make the stack `{rocket}`, or remove `fireworks`.",
                        item = s.item,
                        rocket = crate::firework::ROCKET_ITEM,
                    ),
                ));
            }
            crate::firework::firework_bounds(
                at.stage,
                &at.path,
                fw.flight,
                &fw.explosions,
                &mut bounds,
            );
        }
    });
    for (path, stage, message) in bounds {
        d.push(Diagnostic::error(codes::SCHEMA, stage, path, message));
    }
}

/// **A stack taken off an actor** (spec-0134 §8): at every effect root and
/// nesting depth, a `give-item` `from` names a declared actor (`DW0112`) whose
/// `equipment` fills the slot — the question a `drops[]` slot entry asks, from
/// the same function ([`crate::wave::slot_unfilled`], `DW0490`).
pub(crate) fn give_from_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    fn descend<'a>(
        stage: &'static str,
        path: String,
        eff: &'a QuestEffect,
        out: &mut Vec<(&'static str, String, &'a TakenFrom)>,
    ) {
        if let Verb::GiveItem {
            source: GiveSource::Taken { from },
            ..
        } = &eff.verb
        {
            out.push((stage, path.clone(), from));
        }
        for (pseg, _kseg, list) in eff.nested_effect_lists_labeled() {
            for (j, inner) in list.iter().enumerate() {
                descend(stage, format!("{path}/{pseg}/{j}"), inner, out);
            }
        }
    }
    let mut taken = Vec::new();
    crate::effects::for_each_effect_root(c, &mut |site, effs| {
        for (i, eff) in effs.iter().enumerate() {
            descend(site.stage, format!("{}/{i}", site.path), eff, &mut taken);
        }
    });
    for (stage, path, from) in taken {
        let Some(a) = c.quests.content.actors.iter().find(|a| a.id == from.actor) else {
            d.push(Diagnostic::error(
                codes::DANGLING_REF,
                stage,
                format!("{path}/from/actor"),
                format!(
                    "`give-item` takes from `{}`, which `actors[]` does not declare — name a \
                     declared actor",
                    from.actor
                ),
            ));
            continue;
        };
        if let Some(fills) = crate::wave::slot_unfilled(a.equipment.as_ref(), from.slot) {
            let field = from.slot.field();
            d.push(Diagnostic::error(
                crate::wave::DROP_SLOT_UNFILLED,
                stage,
                format!("{path}/from/slot"),
                format!(
                    "`give-item` takes the `{field}` piece off actor `{}`, but its `equipment` \
                     puts nothing in `{field}` — {fills}. A thing can only be taken off a body \
                     that holds it: equip the slot, or take from a slot it fills",
                    from.actor
                ),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_field_list_is_the_schemas() {
        let schema = schemars::schema_for!(Stack);
        let props: Vec<&str> = schema
            .as_object()
            .and_then(|o| o.get("properties"))
            .and_then(|p| p.as_object())
            .expect("the stack has properties")
            .keys()
            .map(String::as_str)
            .collect();
        let mut want: Vec<&str> = STACK_FIELDS.to_vec();
        want.sort_unstable();
        let mut got = props.clone();
        got.sort_unstable();
        assert_eq!(got, want);
    }

    #[test]
    fn a_taken_stack_is_never_spelled_twice() {
        let ok: GiveSource =
            serde_json::from_str(r#"{"from": {"actor": "actor/fox", "slot": "main_hand"}}"#)
                .unwrap();
        assert!(ok.taken().is_some());
        let err = serde_json::from_str::<GiveSource>(
            r#"{"from": {"actor": "actor/fox", "slot": "main_hand"}, "item": "minecraft:stick"}"#,
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("second time"), "{err}");
        let s: GiveSource = serde_json::from_str(r#"{"item": "minecraft:stick"}"#).unwrap();
        assert_eq!(s.stack().unwrap().count, 1);
    }
}
