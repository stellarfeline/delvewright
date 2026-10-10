//! Classes: kit items, potions, the flask, and the restore function.

use super::*;

/// The player-local restore a **rest** performs (spec-0016 §1): health,
/// hunger/saturation, negative status effects, flask.
///
/// **Audience (reported ambiguity).** spec-0018 makes the checkpoint party state
/// and the flask/inventory per-player state; spec-0016 §1 says "player fully
/// restored" in the singular. This restores the player who chose to rest, and
/// only them — a party member elsewhere in the map keeps their wounds. The
/// checkpoint half of the same rest is still party-wide.
///
/// **Effect clearing is enumerated, never `effect clear @s`.** A bare clear would
/// also strip the per-area night-vision mitigation clock (`DW0322`'s emission)
/// and any beneficial effect the story granted, turning a rest into a debuff.
/// The list is the pinned 1.21.11 harmful set, sorted, so the emission is
/// deterministic.
pub(super) const HARMFUL_EFFECTS: &[&str] = &[
    "minecraft:bad_omen",
    "minecraft:blindness",
    "minecraft:darkness",
    "minecraft:hunger",
    "minecraft:infested",
    "minecraft:levitation",
    "minecraft:mining_fatigue",
    "minecraft:nausea",
    "minecraft:oozing",
    "minecraft:poison",
    "minecraft:raid_omen",
    "minecraft:slowness",
    "minecraft:trial_omen",
    "minecraft:unluck",
    "minecraft:weakness",
    "minecraft:weaving",
    "minecraft:wind_charged",
    "minecraft:wither",
];

/// The `minecraft:potion_contents` component value of a kit item that declares
/// potion `contents` (DSL v0.8, spec-0016 §1) — compact SNBT, field order fixed
/// (`potion`, `custom_effects`, `custom_color`) so emission is deterministic.
///
/// Written straight from the DSL's fields with nothing invented: a declared
/// `duration`/`amplifier` is emitted, an absent one is left out and takes
/// vanilla's own default. That matters beyond tidiness — the replenish path
/// matches the flask by these exact components ([`kit_item_predicate`]), so any
/// value the emitter made up here would have to be re-derived identically there.
pub(super) fn potion_contents_snbt(pc: &delvewright_dsl::PotionContents) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(p) = &pc.potion {
        parts.push(format!("potion:\"{p}\""));
    }
    if !pc.effects.is_empty() {
        let effects: Vec<String> = pc
            .effects
            .iter()
            .map(|e| {
                let mut f = vec![format!("id:\"{}\"", e.effect)];
                if let Some(dur) = e.duration {
                    f.push(format!("duration:{dur}"));
                }
                if let Some(amp) = e.amplifier {
                    f.push(format!("amplifier:{amp}"));
                }
                format!("{{{}}}", f.join(","))
            })
            .collect();
        parts.push(format!("custom_effects:[{}]", effects.join(",")));
    }
    if let Some(col) = &pc.color {
        // `#rrggbb` → the packed int vanilla stores, through the one colour rule
        // (`dsl::color`) the validator and the firework emitter also read.
        // Validation (`DW0486`) already proved the literal well-formed.
        if let Some(v) = delvewright_dsl::color::packed(col) {
            parts.push(format!("custom_color:{v}"));
        }
    }
    format!("{{{}}}", parts.join(","))
}

/// The component suffix a kit item's `give` carries: the display name and, for a
/// potion-bearing item, its `potion_contents`. `""` for a plain unnamed item, so
/// every campaign that declares neither is byte-identical.
///
/// One function for every place a kit item is handed out — the class kit and the
/// bonfire replenish — because those two must produce the *same item*. When they
/// disagree the rest does not refill the flask, it hands the player a second,
/// subtly different one (the `clear` misses it) and the "per-rest budget"
/// contract silently becomes a stockpile.
pub(super) fn kit_item_components(item: &delvewright_dsl::KitItem) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(n) = &item.name {
        parts.push(format!(
            "custom_name={}",
            tr_with(n, &[("italic", json!(false))])
        ));
    }
    if let Some(pc) = &item.contents {
        parts.push(format!("potion_contents={}", potion_contents_snbt(pc)));
    }
    if let Some((id, count)) = flask_remainder(item) {
        parts.push(format!("use_remainder={}", flask_remainder_snbt(id, count)));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("[{}]", parts.join(","))
    }
}

/// The **item predicate** that identifies this kit item for `clear` — the item id
/// plus, when it carries potion contents, an exact `potion_contents` match.
///
/// Why the components belong in the predicate: a bare `clear @s minecraft:potion`
/// takes every potion in the bag, so on a campaign whose kit holds a healing
/// flask *and* any other brew, one rest would delete the other bottle and re-give
/// only the flask. Matching the contents makes the clear name exactly the stack
/// the `give` on the next line puts back.
pub(super) fn kit_item_predicate(item: &delvewright_dsl::KitItem) -> String {
    match &item.contents {
        Some(pc) => format!(
            "{}[potion_contents={}]",
            item.item,
            potion_contents_snbt(pc)
        ),
        None => item.item.clone(),
    }
}

/// The items whose pinned 1.21.11 definition carries a default
/// `minecraft:use_remainder` — what vanilla leaves in the hand once the item is
/// consumed — with that remainder's id and count.
///
/// Read off the pinned 1.21.11 `item_components` summary (SHA-256
/// `51b191e13f86813ca02f1498942e5bc235947edb71eb8105a78401670b3665c4`, the
/// misode/mcmeta ref `crates/delvec/data/PROVENANCE.md` pins): exactly these
/// seven of its 1505 items declare the component.
pub(super) const USE_REMAINDERS_1_21_11: &[(&str, &str, u32)] = &[
    ("minecraft:beetroot_soup", "minecraft:bowl", 1),
    ("minecraft:honey_bottle", "minecraft:glass_bottle", 1),
    ("minecraft:milk_bucket", "minecraft:bucket", 1),
    ("minecraft:mushroom_stew", "minecraft:bowl", 1),
    ("minecraft:potion", "minecraft:glass_bottle", 1),
    ("minecraft:rabbit_stew", "minecraft:bowl", 1),
    ("minecraft:suspicious_stew", "minecraft:bowl", 1),
];

/// The `custom_data` key that marks a remainder as **the flask's own**: a flask
/// is given with its vanilla `use_remainder` restated plus this mark, so the
/// empty it leaves is told apart from the same item obtained any other way, and
/// `bonfire_flask` takes back exactly those.
pub(super) const FLASK_EMPTY_MARK: &str = "dw_flask_empty";

/// The flask's marked remainder as the `use_remainder` component value: vanilla's
/// own remainder item and count, carrying the [`FLASK_EMPTY_MARK`].
pub(super) fn flask_remainder_snbt(id: &str, count: u32) -> String {
    format!(
        "{{id:\"{id}\",count:{count},components:{{\"minecraft:custom_data\":{{{FLASK_EMPTY_MARK}:1b}}}}}}"
    )
}

/// The flask's empty as an item stack / predicate: the remainder item with the
/// [`FLASK_EMPTY_MARK`] — what drinking the flask leaves, and all that the refill
/// takes back.
pub(super) fn flask_empty_stack(id: &str) -> String {
    format!("{id}[custom_data={{{FLASK_EMPTY_MARK}:1b}}]")
}

/// The flask's vanilla remainder (`(id, count)`), when this kit item is a flask
/// whose item leaves one; `None` for every other kit item and for a flask that is
/// thrown or eaten whole.
pub(super) fn flask_remainder(item: &delvewright_dsl::KitItem) -> Option<(&'static str, u32)> {
    if !item.flask {
        return None;
    }
    let norm = if item.item.contains(':') {
        item.item.clone()
    } else {
        format!("minecraft:{}", item.item)
    };
    USE_REMAINDERS_1_21_11
        .iter()
        .find(|(it, _, _)| *it == norm)
        .map(|&(_, id, count)| (id, count))
}

/// The `bonfire_flask` function: refill every declared flask to its declared
/// count, for the player it runs as, and take back the empties the flask left.
///
/// The take-back clears the flask's MARKED remainder (see [`FLASK_EMPTY_MARK`])
/// for every flask of every class, unguarded by class: an empty a party member
/// handed over is still a flask's empty, and a bottle the player found or was
/// given is never marked, so it is never taken.
///
/// `clear` + `give` rather than `item replace`: a kit item has no fixed inventory
/// slot (the player carries it wherever they moved it), and `item replace` needs
/// one. Clearing the flask's own item predicate and re-giving the kit's exact
/// stack is slot-free, idempotent and byte-stable — and it means "replenish to
/// the declared count" is literally what the commands say, in both directions (a
/// player hoarding extra flasks is brought back DOWN to the declared count, which
/// is the souls contract: the flask is a per-rest budget, not a stockpile).
/// Cleared and re-given through the SAME pair of helpers the class kit uses, so
/// the refilled bottle is the poured-identical item, not a lookalike.
///
/// A player's class is read off the `dw_class_<safe>` tag `class_apply_<safe>`
/// adds — emitted only when the campaign declares a flask at all, so a campaign
/// without one is byte-identical down to the class-apply function.
pub(super) fn emit_flask_function(plan: &Plan) -> Option<(String, String)> {
    let flasks = plan.flasks();
    if flasks.is_empty() {
        return None;
    }
    let classes = &plan.campaign.classes.content.classes;
    let mut body: Vec<String> = Vec::new();
    let empties: std::collections::BTreeSet<&str> = flasks
        .iter()
        .filter_map(|&(ci, ki)| flask_remainder(&classes[ci].kit[ki]))
        .map(|(id, _)| id)
        .collect();
    for id in empties {
        body.push(format!("clear @s {}", flask_empty_stack(id)));
    }
    for (ci, ki) in flasks {
        let item = &classes[ci].kit[ki];
        let tag = class_tag(&plan.classes[ci].safe);
        body.push(format!(
            "execute if entity @s[tag={tag}] run clear @s {}",
            kit_item_predicate(item)
        ));
        body.push(format!(
            "execute if entity @s[tag={tag}] run give @s {}{} {}",
            item.item,
            kit_item_components(item),
            item.count
        ));
    }
    Some(("bonfire_flask".to_string(), lines(&body)))
}

/// Whether any body watches a class (spec-0101): the class apply then tags its
/// player with [`class_tag`], which the watch line filters on.
pub(super) fn campaign_watches_a_class(plan: &Plan) -> bool {
    delvewright_dsl::body_watch_sites(plan.campaign)
        .iter()
        .any(|s| s.watch.who.class().is_some())
}

/// The per-player tag marking which class a player took — the only thing that
/// tells a bonfire rest which flask to refill (`dw.class` is a trigger the class
/// apply resets, and `dw.classed` records only *that* a class was taken), and
/// the filter a class watch reads (spec-0101).
pub(super) fn class_tag(class_safe: &str) -> String {
    format!("dw_class_{class_safe}")
}

/// The `bonfire_restore` function: the full player restore of a *rest*.
/// Emitted only for a campaign with a bonfire → byte-identical otherwise.
///
/// Healing is `instant_health`, feeding is `saturation`: vanilla exposes no
/// `/health` or `/food` command and `/data merge entity` refuses players, so
/// these two effects ARE the primitive (CLAUDE.md no-hacks: use the intended one,
/// do not invent a workaround). Both are instant/1-second and leave nothing
/// behind.
///
/// **Mending** is vanilla's `set_damage` item modifier (`BONFIRE_MEND`, damage
/// fraction 1.0 = full durability), applied with `item modify` to every slot the
/// player carries ([`CARRIED_SLOTS`]) — in place, so the rest repairs what the
/// player holds now, gear picked up in play included, and never re-kits them.
/// `item modify` takes one slot, so it is one line per slot, each guarded on the
/// slot holding a damaged item: an empty slot, a stack and an item with no
/// durability are never handed to the modifier (unguarded, `set_damage` leaves
/// them unchanged but logs a warning per slot per rest).
pub(super) fn emit_restore_function(plan: &Plan) -> Option<(String, String)> {
    plan.bonfires().next()?;
    let ns = &plan.namespace;
    let mut body: Vec<String> = vec![
        // Amplifier 9 heals 2 × 2^10 half-hearts — past any `max_health` a kit
        // can reach, so "full" needs no health arithmetic.
        "effect give @s minecraft:instant_health 1 9 true".to_string(),
        // Saturation adds food + saturation every tick it runs; one second at
        // amplifier 9 pins both bars at full.
        "effect give @s minecraft:saturation 1 9 true".to_string(),
    ];
    body.extend(
        HARMFUL_EFFECTS
            .iter()
            .map(|e| format!("effect clear @s {e}")),
    );
    body.extend(CARRIED_SLOTS.iter().map(|slot| {
        format!(
            "execute if items entity @s {slot} *[damage~{{damage:{{min:1}}}}] \
             run item modify entity @s {slot} {ns}:{BONFIRE_MEND}"
        )
    }));
    if !plan.flasks().is_empty() {
        body.push(format!("function {ns}:bonfire_flask"));
    }
    Some(("bonfire_restore".to_string(), lines(&body)))
}

/// Each class's `class_apply_<c>`: reset the trigger, hand the kit, start the campaign's opening quests, teleport to the entry point.
pub(super) fn class_apply_fns(plan: &Plan) -> Vec<(String, String)> {
    let c = plan.campaign;
    let mut fns: Vec<(String, String)> = Vec::new();
    let campaign_start = campaign_start_quests(c);
    for (i, class) in c.classes.content.classes.iter().enumerate() {
        let plan_class = &plan.classes[i];
        let mut body: Vec<String> = Vec::new();
        body.push("scoreboard players reset @s dw.class".to_string());
        for (k, item) in class.kit.iter().enumerate() {
            let give = format!(
                "give @s {}{} {}",
                item.item,
                kit_item_components(item),
                item.count
            );
            // A class kit is per-player gear by construction. `carrier: "one"`
            // (v0.6, spec-0018) marks a **party-unique** kit item — exactly one
            // copy enters the party, to the first player who takes this class —
            // latched on its own `dw.sys` sentinel so a second taker gets the rest
            // of the kit but not the singleton. Absent `carrier` → unchanged.
            if matches!(item.carrier, Some(delvewright_dsl::Carrier::One)) {
                let latch = format!("#kit_{}_{k}", plan_class.safe);
                body.push(format!(
                    "execute unless score {latch} dw.sys matches 1 run {give}"
                ));
                body.push(format!("scoreboard players set {latch} dw.sys 1"));
            } else {
                body.push(give);
            }
        }
        // spec-0016 §1: a bonfire rest refills the resting player's OWN flask, so
        // the pack has to remember which class they took — `dw.class` is a trigger
        // this function resets and `dw.classed` records only that a class was
        // taken. Emitted only when the campaign declares a flask, so every other
        // campaign's class apply is byte-identical.
        // spec-0101: a watch filtered to a class reads the same tag, so a
        // campaign whose bodies watch a class carries it too.
        if !plan.flasks().is_empty() || campaign_watches_a_class(plan) {
            body.push(format!("tag @s add {}", class_tag(&plan_class.safe)));
        }
        body.push("scoreboard players set @s dw.classed 1".to_string());
        // Party state (spec-0018): the campaign-start quests activate for the
        // PARTY the moment any player takes a class, so a second player who is
        // still on the class screen is not behind on the quest state.
        for qid in &campaign_start {
            body.push(format!(
                "scoreboard players set {} {} 1",
                plan::PARTY,
                quest_active_score(qid)
            ));
        }
        // teleport to the first area's spawn anchor
        if let Some(pos) = campaign_spawn(plan) {
            body.push(format!("teleport @s {} {} {}", pos[0], pos[1], pos[2]));
        }
        fns.push((format!("class_apply_{}", plan_class.safe), lines(&body)));
    }
    fns
}
