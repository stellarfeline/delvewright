//! **What a body may carry that changes how it moves** (spec-0113 §5.4,
//! `DW1043`).
//!
//! The reach graph flies the pinned game's body with the attributes a player
//! has and no item in hand. Every item the engine puts in a player's hands — a
//! kit item, a `give-item`, a container's loot, a shop offer's effects — and
//! every effect it grants (`give-effect`) is read here for what it does to that
//! body (a wave's or an actor's `drops` are read as loot):
//!
//! * **an unmodelled movement**: an effect the graph does not fly (`speed`,
//!   `slowness`, `jump_boost`, `slow_falling`, `levitation`,
//!   `dolphins_grace`, by a potion's name or its custom effects, or granted),
//!   an item that moves a body (`elytra`, `wind_charge`, `ender_pearl`,
//!   `chorus_fruit`, a `trident` with `riptide`), or an enchantment that does
//!   (`depth_strider`, `swift_sneak`, `soul_speed`). From the first step a
//!   body can hold one, the graph is **unbounded**: the proofs that read it
//!   (`DW0921`, `DW0322`) report those steps as unjudged, naming the item, and
//!   the build goes on. `DW1043` is that warning.
//! * **a fall it survives further**: `resistance`, `health_boost` or
//!   `absorption`, granted or in a potion, and `feather_falling` or
//!   `protection` on a stack. These are admitted: the graph takes every fall as
//!   survivable from the start (`Premises::unbounded_fall`), which only widens
//!   where a body can be.
//!
//! The stage an item becomes obtainable at is the step after the objective
//! whose completion gives it; a kit's is the first step; any other source —
//! a trigger, a dialogue option, a shop, a container — is taken as obtainable
//! from the first step, which is said in the warning.

use delvewright_dsl::Diagnostic;
use serde_json::Value;

use crate::compiler::plan::Step;

/// What an item or an effect does to the body the reach graph flies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// A movement the graph does not fly.
    Unmodelled,
    /// A fall survived further.
    Reducer,
}

/// One item or effect a body may hold that changes how it moves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MovementItem {
    /// The item or effect, as written.
    pub item: String,
    /// The rule it changes, in words.
    pub rule: String,
    /// Where the campaign gives it.
    pub source: String,
    /// The first critical-path step a body can hold it at.
    pub stage: usize,
    /// Whether that step is a measurement (`true`: the step after the
    /// objective that gives it, or a kit's first step) or the conservative
    /// reading for a source off the path's order.
    pub staged: bool,
}

/// Every item and effect the campaign gives, read for how it moves a body.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MovementItems {
    /// Items and effects read.
    pub examined: usize,
    /// The unmodelled movements, in reading order.
    pub unmodelled: Vec<MovementItem>,
    /// The fall reducers, in reading order.
    pub reducers: Vec<MovementItem>,
}

impl MovementItems {
    /// The first step from which a body may hold an unmodelled movement, and
    /// the item that makes it so.
    #[must_use]
    pub fn unbounded_from(&self) -> Option<&MovementItem> {
        self.unmodelled.iter().min_by_key(|m| m.stage)
    }

    /// Whether every fall is survivable to the graph.
    #[must_use]
    pub fn unbounded_fall(&self) -> bool {
        !self.reducers.is_empty()
    }

    /// The binding line: what was read, and how many quest configurations the
    /// leave proof judged and left unjudged.
    #[must_use]
    pub fn line(&self, judged: usize, unbounded: usize) -> String {
        let from = match self.unbounded_from() {
            Some(m) => format!(
                "; a body may hold `{}` ({}) from critical step {}",
                m.item, m.source, m.stage
            ),
            None => String::new(),
        };
        format!(
            "movement items: {} examined, {} a movement the reach graph does not fly, {} reducing \
             a fall (every fall survivable: {}); {judged} quest configuration(s) judged, \
             {unbounded} unbounded{from}",
            self.examined,
            self.unmodelled.len(),
            self.reducers.len(),
            if self.unbounded_fall() { "yes" } else { "no" },
        )
    }

    /// One `DW1043` warning per unmodelled movement.
    #[must_use]
    pub fn findings(&self, unbounded: usize) -> Vec<Diagnostic> {
        self.unmodelled
            .iter()
            .map(|m| {
                let when = if m.staged {
                    format!("from critical step {}", m.stage)
                } else {
                    format!(
                        "from critical step {} (its source is off the critical path's order, so \
                         it is taken as obtainable from the start)",
                        m.stage
                    )
                };
                Diagnostic::warning(
                    super::DW_MOVEMENT_UNMODELLED,
                    "build",
                    "",
                    format!(
                        "`{}`, given by {}, {}: the reach graph flies a body without it, so where \
                         a body that holds it can be is not judged {when}. {unbounded} quest \
                         configuration(s) of `DW0921` are unjudged, and `DW0322` is unjudged when \
                         the item is held from the start. Nothing is refused: the engine cannot \
                         yet fly this movement, and the places it reaches are the owner's hour",
                        m.item, m.source, m.rule
                    ),
                )
            })
            .collect()
    }
}

/// The rule an effect changes, by its id, and how.
fn effect_rule(effect: &str) -> Option<(Kind, &'static str)> {
    let id = effect.strip_prefix("minecraft:").unwrap_or(effect);
    Some(match id {
        "speed" => (
            Kind::Unmodelled,
            "changes `movement_speed` (the effect `speed`)",
        ),
        "slowness" => (
            Kind::Unmodelled,
            "changes `movement_speed` (the effect `slowness`)",
        ),
        "jump_boost" => (
            Kind::Unmodelled,
            "changes `jump_strength` and the fall it survives (the effect `jump_boost`)",
        ),
        "slow_falling" => (
            Kind::Unmodelled,
            "changes `gravity` (the effect `slow_falling`)",
        ),
        "levitation" => (Kind::Unmodelled, "lifts the body (the effect `levitation`)"),
        "dolphins_grace" => (
            Kind::Unmodelled,
            "changes the water's drag (the effect `dolphins_grace`)",
        ),
        "resistance" | "health_boost" | "absorption" => {
            (Kind::Reducer, "lets a body survive a longer fall")
        }
        _ => return None,
    })
}

/// The effects a named potion carries that bear on movement.
fn potion_effects(potion: &str) -> &'static [&'static str] {
    let id = potion.strip_prefix("minecraft:").unwrap_or(potion);
    let base = id
        .strip_prefix("long_")
        .or_else(|| id.strip_prefix("strong_"))
        .unwrap_or(id);
    match base {
        "swiftness" => &["speed"],
        "slowness" => &["slowness"],
        "leaping" => &["jump_boost"],
        "slow_falling" => &["slow_falling"],
        "turtle_master" => &["slowness", "resistance"],
        _ => &[],
    }
}

/// The rule an item changes by what it is.
fn item_rule(item: &str) -> Option<&'static str> {
    let id = item.strip_prefix("minecraft:").unwrap_or(item);
    Some(match id {
        "elytra" => "glides (`elytra`)",
        "wind_charge" => "launches a body by its burst (`wind_charge`)",
        "ender_pearl" => "teleports a body where the pearl lands (`ender_pearl`)",
        "chorus_fruit" => "teleports a body at random (`chorus_fruit`)",
        _ => return None,
    })
}

/// The rule an enchantment changes, and how.
fn enchantment_rule(item: &str, enchantment: &str) -> Option<(Kind, &'static str)> {
    let id = enchantment
        .strip_prefix("minecraft:")
        .unwrap_or(enchantment);
    let it = item.strip_prefix("minecraft:").unwrap_or(item);
    Some(match id {
        "riptide" if it == "trident" => (
            Kind::Unmodelled,
            "launches a body through water or rain (`riptide`)",
        ),
        "depth_strider" => (
            Kind::Unmodelled,
            "changes `water_movement_efficiency` (`depth_strider`)",
        ),
        "swift_sneak" => (Kind::Unmodelled, "changes `sneaking_speed` (`swift_sneak`)"),
        "soul_speed" => (
            Kind::Unmodelled,
            "changes `movement_speed` on soul blocks (`soul_speed`)",
        ),
        "feather_falling" | "protection" => (Kind::Reducer, "lets a body survive a longer fall"),
        _ => return None,
    })
}

struct Reader<'a> {
    out: MovementItems,
    path: &'a [Step],
}

impl Reader<'_> {
    fn push(&mut self, kind: Kind, item: &str, rule: &str, source: &str, at: (usize, bool)) {
        let m = MovementItem {
            item: item.to_string(),
            rule: rule.to_string(),
            source: source.to_string(),
            stage: at.0,
            staged: at.1,
        };
        match kind {
            Kind::Unmodelled => self.out.unmodelled.push(m),
            Kind::Reducer => self.out.reducers.push(m),
        }
    }

    /// One stack: its id, its enchantments, its potion contents.
    fn stack(
        &mut self,
        item: &str,
        enchantments: Option<&Value>,
        contents: Option<&Value>,
        source: &str,
        at: (usize, bool),
    ) {
        self.out.examined += 1;
        if let Some(rule) = item_rule(item) {
            self.push(Kind::Unmodelled, item, rule, source, at);
        }
        if let Some(Value::Object(e)) = enchantments {
            for name in e.keys() {
                if let Some((kind, rule)) = enchantment_rule(item, name) {
                    self.push(kind, &format!("{item} with {name}"), rule, source, at);
                }
            }
        }
        if let Some(c) = contents {
            let mut effects: Vec<String> = Vec::new();
            if let Some(p) = c.get("potion").and_then(Value::as_str) {
                effects.extend(potion_effects(p).iter().map(|e| (*e).to_string()));
            }
            if let Some(Value::Array(list)) = c.get("effects") {
                effects.extend(
                    list.iter()
                        .filter_map(|e| e.get("effect").and_then(Value::as_str))
                        .map(str::to_string),
                );
            }
            let name = c.get("potion").and_then(Value::as_str).unwrap_or(item);
            for e in effects {
                if let Some((kind, rule)) = effect_rule(&e) {
                    self.push(kind, name, rule, source, at);
                }
            }
        }
    }

    /// The first step a body holds what an effect at `pointer` gives: the
    /// step after the objective whose completion runs it, or the first step.
    fn stage_of(&self, pointer: &str) -> (usize, bool) {
        let mut segs = pointer.split('/');
        while let Some(s) = segs.next() {
            if s == "on_objective_complete"
                && let Some(obj) = segs.next()
            {
                let obj = obj.replace("~1", "/").replace("~0", "~");
                if let Some(i) = self
                    .path
                    .iter()
                    .position(|st| st.objective() == Some(obj.as_str()))
                {
                    return (i + 1, true);
                }
            }
        }
        (0, false)
    }

    /// Every `give-item`, `give-effect` and loot stack in one document.
    fn walk(&mut self, doc: &str, v: &Value, pointer: &mut String) {
        match v {
            Value::Object(m) => {
                match m.get("type").and_then(Value::as_str) {
                    Some("give-item") => {
                        if let Some(item) = m.get("item").and_then(Value::as_str) {
                            let source = format!("a `give-item` at {doc}{pointer}");
                            let at = self.stage_of(pointer);
                            self.stack(item, m.get("enchantments"), None, &source, at);
                        }
                    }
                    Some("give-effect") => {
                        if let Some(effect) = m.get("effect").and_then(Value::as_str) {
                            self.out.examined += 1;
                            if let Some((kind, rule)) = effect_rule(effect) {
                                let source = format!("a `give-effect` at {doc}{pointer}");
                                let at = self.stage_of(pointer);
                                self.push(kind, effect, rule, &source, at);
                            }
                        }
                    }
                    _ => {}
                }
                if m.get("id")
                    .and_then(Value::as_str)
                    .is_some_and(|i| i.starts_with("loot/"))
                    && let Some(Value::Array(items)) = m.get("items")
                {
                    let id = m.get("id").and_then(Value::as_str).unwrap_or_default();
                    for it in items {
                        if let Some(item) = it.get("item").and_then(Value::as_str) {
                            let source = format!("loot `{id}`");
                            self.stack(item, it.get("enchantments"), None, &source, (0, false));
                        }
                    }
                }
                // What a wave's or an actor's body drops is in a player's
                // hands as soon as it is picked up.
                if let Some(Value::Array(drops)) = m.get("drops") {
                    for it in drops {
                        if let Some(item) = it.get("item").and_then(Value::as_str) {
                            let source = format!("a drop at {doc}{pointer}/drops");
                            self.stack(item, it.get("enchantments"), None, &source, (0, false));
                        }
                    }
                }
                for (k, child) in m {
                    let len = pointer.len();
                    pointer.push('/');
                    pointer.push_str(&k.replace('~', "~0").replace('/', "~1"));
                    self.walk(doc, child, pointer);
                    pointer.truncate(len);
                }
            }
            Value::Array(a) => {
                for (i, child) in a.iter().enumerate() {
                    let len = pointer.len();
                    pointer.push_str(&format!("/{i}"));
                    self.walk(doc, child, pointer);
                    pointer.truncate(len);
                }
            }
            _ => {}
        }
    }
}

/// Read every item and effect `campaign` gives (spec-0113 §5.4), staged
/// against the critical path `path`.
#[must_use]
pub fn movement_items(campaign: &delvewright_dsl::Campaign, path: &[Step]) -> MovementItems {
    let mut r = Reader {
        out: MovementItems::default(),
        path,
    };
    for class in &campaign.classes.content.classes {
        for k in &class.kit {
            let source = format!("class `{}`'s kit", class.id.as_str());
            let contents = k
                .contents
                .as_ref()
                .and_then(|c| serde_json::to_value(c).ok());
            r.stack(&k.item, None, contents.as_ref(), &source, (0, true));
        }
    }
    let docs: [(&str, Option<Value>); 4] = [
        (
            "world.json",
            serde_json::to_value(&campaign.world.content).ok(),
        ),
        (
            "npcs.json",
            serde_json::to_value(&campaign.npcs.content).ok(),
        ),
        (
            "quests.json",
            serde_json::to_value(&campaign.quests.content).ok(),
        ),
        (
            "dialogue.json",
            serde_json::to_value(&campaign.dialogue.content).ok(),
        ),
    ];
    for (doc, v) in docs {
        if let Some(v) = v {
            let mut pointer = String::from("/content");
            r.walk(doc, &v, &mut pointer);
        }
    }
    r.out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hello_world() -> delvewright_dsl::Campaign {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../dsl/fixtures/valid/hello-world");
        let loaded = crate::compiler::load::load_campaign_dir(&dir).expect("the fixture loads");
        crate::compiler::load::parse_loaded(&loaded).expect("the fixture parses")
    }

    /// spec-0113 A14: every source an item reaches a player by is read. Each
    /// of the four — a kit, a `give-item`, a container's loot, a shop offer —
    /// perturbed alone with a movement the graph does not fly is one
    /// unmodelled item named by that source, and the fixture alone is none.
    #[test]
    fn the_movement_items_are_enumerated() {
        let base = movement_items(&hello_world(), &[]);
        assert!(base.examined > 0, "the fixture's kit is read");
        assert!(base.unmodelled.is_empty(), "{:?}", base.unmodelled);

        let mut kit = hello_world();
        kit.classes.content.classes[0].kit.push(
            serde_json::from_value(serde_json::json!({
                "item": "minecraft:potion", "count": 1,
                "contents": {"potion": "minecraft:leaping"}
            }))
            .unwrap(),
        );
        let give: delvewright_dsl::QuestEffect = serde_json::from_value(serde_json::json!({
            "type": "give-item", "item": "minecraft:ender_pearl", "count": 1
        }))
        .unwrap();
        let mut given = hello_world();
        given.quests.content.quests[0]
            .on_complete
            .push(give.clone());
        let mut looted = hello_world();
        looted.quests.content.loot.push(
            serde_json::from_value(serde_json::json!({
                "id": "loot/a-glider", "anchor": "anchor/keeper",
                "items": [{"item": "minecraft:elytra"}]
            }))
            .unwrap(),
        );
        let mut shop = hello_world();
        shop.quests.content.shops.push(
            serde_json::from_value(serde_json::json!({
                "id": "shop/a-pearl", "anchor": "anchor/keeper", "title": "Pearls",
                "offers": [{"label": "A wind charge", "effects": [
                    {"type": "give-item", "item": "minecraft:wind_charge", "count": 1}
                ]}]
            }))
            .unwrap(),
        );
        for (name, c, item, source) in [
            (
                "kit",
                kit,
                "minecraft:leaping",
                "class `class/wanderer`'s kit",
            ),
            (
                "give-item",
                given,
                "minecraft:ender_pearl",
                "a `give-item` at quests.json",
            ),
            ("loot", looted, "minecraft:elytra", "loot `loot/a-glider`"),
            (
                "shop",
                shop,
                "minecraft:wind_charge",
                "a `give-item` at quests.json/content/shops/0",
            ),
        ] {
            let m = movement_items(&c, &[]);
            assert_eq!(m.unmodelled.len(), 1, "{name}: {:?}", m.unmodelled);
            assert_eq!(m.unmodelled[0].item, item, "{name}");
            assert!(
                m.unmodelled[0].source.starts_with(source),
                "{name}: {}",
                m.unmodelled[0].source
            );
            let f = m.findings(1);
            assert_eq!(f.len(), 1);
            assert_eq!(f[0].code, "DW1043");
            assert_eq!(f[0].severity, delvewright_dsl::Severity::Warning);
            assert!(f[0].message.contains(item), "{name}: {}", f[0].message);
        }
    }

    /// A fall reducer is admitted, and makes every fall survivable to the graph.
    #[test]
    fn a_fall_reducer_is_admitted() {
        let mut c = hello_world();
        c.quests.content.quests[0].on_complete.push(
            serde_json::from_value(serde_json::json!({
                "type": "give-item", "item": "minecraft:leather_boots", "count": 1,
                "enchantments": {"minecraft:feather_falling": 4}
            }))
            .unwrap(),
        );
        let m = movement_items(&c, &[]);
        assert!(m.unmodelled.is_empty());
        assert_eq!(m.reducers.len(), 1);
        assert!(m.unbounded_fall());
    }
}
