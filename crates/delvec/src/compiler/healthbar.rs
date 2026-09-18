//! **A fight shows its health** (spec-0073): the compiler half of `health_bar`.
//!
//! Two things live here.
//!
//! * **`DW0911`** — a `color` or `style` the pinned game does not draw. The
//!   vocabulary is whatever the pinned command tree lists under
//!   `bossbar set <id> color` and `… style`, read through
//!   [`CommandTree::literals_under`]. The tree is this crate's data, so the rule
//!   is compiler-side, raised from the one validation funnel every subcommand
//!   goes through (validation tier, exit 1) — the `DW0343` precedent for a
//!   document rule that needs data the DSL crate does not carry. There is no
//!   Rust enum of colours: that would be a second copy of the tree.
//! * **Emission** — the lines a declared bar lowers to (spec-0073 §6). One
//!   vanilla custom boss bar per declaring fight, id `<ns>:hb_<key>`, where the
//!   key is `wave_<safe id>` or `actor_<safe id>` so a wave and an actor that
//!   share a local name never share a bar.
//!
//! ## What a bar reads, and when
//!
//! The **value** is the sum of `Health` over every live body of the fight
//! (`nbt=!{Health:0.0f}`, the live-census rule: a dying body is not a standing
//! one), recomputed every tick the fight has a live body. The **max** is the sum
//! of `max_health` over the same bodies, captured from the server's own
//! `attribute … max_health get` at every summon of them — `spawn_<wave>`,
//! `spawn_actor_<id>`, `unleash_<id>`, `actor_restand_<id>` — never from a
//! species table the compiler cannot verify (`DW0475`'s rule). Captured rather
//! than re-summed per tick, so a fight of three with one dead reads two-thirds,
//! not full.
//!
//! **Who sees it** is re-derived every tick: every player within `range` of a
//! live body, who is not watching a cutscene (`tag=!dw_cutscene`) and is not
//! dead. With no live body the bar is hidden for everyone.
//!
//! A bar over an actor reads the bodies whose health can move: the unleashed
//! twin (`tag=!dw_pup_<id>` excludes the invulnerable puppet), or the puppet
//! itself when the actor is `vulnerable`.

use delvewright_dsl::{
    Campaign, Diagnostic, DwCode, ExitTier, Fight, FightKind, HealthBar, fights,
};

use crate::compiler::commands::CommandTree;
use crate::compiler::plan;

/// `DW0911`: a `health_bar` `color` or `style` is not among the literals the
/// pinned command tree lists for `bossbar set <id> color|style`.
pub const DW_HEALTH_BAR_VOCABULARY: DwCode = DwCode::new("DW0911", ExitTier::Build);

/// Where the pinned tree lists the colours a custom boss bar can take.
pub const COLOR_PATH: [&str; 4] = ["bossbar", "set", "id", "color"];
/// Where the pinned tree lists the styles a custom boss bar can take.
pub const STYLE_PATH: [&str; 4] = ["bossbar", "set", "id", "style"];

/// The scoreboard objective every holder here lives on.
const SYS: &str = "dw.sys";

/// The live-body filter every read of a fight uses.
const LIVE: &str = "nbt=!{Health:0.0f}";

/// Refuse a `color` or `style` the pinned game does not draw (`DW0911`). The
/// message lists the literals, read from the tree — never a copy.
pub fn check_vocabulary(c: &Campaign, tree: &CommandTree) -> Vec<Diagnostic> {
    let mut d = Vec::new();
    for f in fights(c) {
        let Some(bar) = f.bar else { continue };
        for (field, value, path) in [
            ("color", bar.color.as_deref(), &COLOR_PATH),
            ("style", bar.style.as_deref(), &STYLE_PATH),
        ] {
            let Some(value) = value else { continue };
            let known = tree.literals_under(path).unwrap_or_default();
            if known.contains(&value) {
                continue;
            }
            d.push(Diagnostic::error(
                DW_HEALTH_BAR_VOCABULARY,
                "quests",
                format!("{}/health_bar/{field}", f.path),
                format!(
                    "`health_bar` `{field}` `{value}` on {} `{}` is not a {field} the pinned game \
                     draws a boss bar in. `bossbar set <id> {field}` accepts exactly: {}. Write one \
                     of those, or remove `{field}` to keep the game's default.",
                    f.kind.word(),
                    f.id,
                    known.join(", ")
                ),
            ));
        }
    }
    d
}

/// One declared bar, with everything its emission needs.
#[derive(Clone, Debug)]
pub struct Bar<'a> {
    /// The fight the bar is over.
    pub fight: Fight<'a>,
    /// The declaration.
    pub bar: &'a HealthBar,
    /// The fight's `safe_local` id — the spelling of its body tag.
    pub safe: String,
    /// `wave_<safe>` or `actor_<safe>`: every name below is built from it.
    pub key: String,
}

impl<'a> Bar<'a> {
    /// The custom boss bar's resource id.
    pub fn id(&self, ns: &str) -> String {
        format!("{ns}:hb_{}", self.key)
    }

    /// The selector arguments naming the bodies the bar reads — the ones whose
    /// health can move.
    pub fn bodies(&self) -> String {
        match self.fight.kind {
            FightKind::Wave => format!("tag={}", plan::wave_tag(self.fight.id)),
            FightKind::Actor if self.fight.vulnerable => format!("tag=dw_actor_{}", self.safe),
            FightKind::Actor => format!("tag=dw_actor_{s},tag=!dw_pup_{s}", s = self.safe),
        }
    }

    /// [`Self::bodies`], live ones only.
    pub fn live(&self) -> String {
        format!("{},{LIVE}", self.bodies())
    }

    /// The player tag marking this bar's audience for the current tick.
    pub fn audience_tag(&self) -> String {
        format!("dw_hb_{}", self.key)
    }

    /// The per-tick function that refreshes the bar.
    pub fn refresh_fn(&self) -> String {
        format!("hb_{}", self.key)
    }

    /// The per-body function adding one body's health into the value holder.
    pub fn acc_fn(&self) -> String {
        format!("hb_acc_{}", self.key)
    }

    /// The capture: re-sum the fight's `max_health` into the max holder.
    pub fn capture_fn(&self) -> String {
        format!("hb_max_{}", self.key)
    }

    /// The per-body function adding one body's `max_health` into the max holder.
    pub fn max_acc_fn(&self) -> String {
        format!("hb_macc_{}", self.key)
    }

    /// The holder carrying the bar's summed health this tick.
    pub fn value_holder(&self) -> String {
        format!("#hbv_{}", self.key)
    }

    /// The holder carrying the fight's captured summed `max_health`.
    pub fn max_holder(&self) -> String {
        format!("#hbm_{}", self.key)
    }

    /// One body's reading, before it is added in.
    fn scratch_holder(&self) -> String {
        format!("#hbt_{}", self.key)
    }

    /// The line a summoning function appends so the max follows the bodies it
    /// just put in the world.
    pub fn capture_call(&self, ns: &str) -> String {
        format!("function {ns}:{}", self.capture_fn())
    }
}

/// Every declared bar, waves first then actors, each in declaration order.
/// Empty for a campaign that declares none, so every emitter that reads it is
/// byte-identical there.
pub fn bars(c: &Campaign) -> Vec<Bar<'_>> {
    fights(c)
        .into_iter()
        .filter_map(|f| {
            let bar = f.bar?;
            let safe = plan::safe_local(f.id);
            let key = format!("{}_{safe}", f.kind.word());
            Some(Bar {
                fight: f,
                bar,
                safe,
                key,
            })
        })
        .collect()
}

/// The bar over the wave `wave_id`, if it declares one.
pub fn wave_bar<'a>(c: &'a Campaign, wave_id: &str) -> Option<Bar<'a>> {
    bars(c)
        .into_iter()
        .find(|b| b.fight.kind == FightKind::Wave && b.fight.id == wave_id)
}

/// The bar over the actor `actor_id`, if it declares one.
pub fn actor_bar<'a>(c: &'a Campaign, actor_id: &str) -> Option<Bar<'a>> {
    bars(c)
        .into_iter()
        .find(|b| b.fight.kind == FightKind::Actor && b.fight.id == actor_id)
}

/// World-init lines (`setup`): every declared bar is removed and re-added from
/// nothing — an `add` on an id that already exists fails, and a bar left by an
/// earlier build of the same world must not survive — then given its declared
/// colour and style (none declared ⇒ no line, the game's default stands), an
/// empty audience, and zeroed holders. `title` lowers a player-visible string to
/// its JSON text component.
pub fn setup_lines(ns: &str, bars: &[Bar<'_>], title: &dyn Fn(&str) -> String) -> Vec<String> {
    let mut out = Vec::new();
    for b in bars {
        let id = b.id(ns);
        out.push(format!("bossbar remove {id}"));
        out.push(format!(
            "bossbar add {id} {}",
            title(b.fight.title().unwrap_or_default())
        ));
        if let Some(color) = &b.bar.color {
            out.push(format!("bossbar set {id} color {color}"));
        }
        if let Some(style) = &b.bar.style {
            out.push(format!("bossbar set {id} style {style}"));
        }
        out.push(format!("bossbar set {id} players"));
        out.push(format!(
            "scoreboard players set {} {SYS} 0",
            b.value_holder()
        ));
        out.push(format!("scoreboard players set {} {SYS} 0", b.max_holder()));
    }
    out
}

/// Per-tick lines: refresh a bar whose fight has a live body, hide one whose
/// fight has none — on the tick of the last death.
pub fn tick_lines(ns: &str, bars: &[Bar<'_>]) -> Vec<String> {
    let mut out = Vec::new();
    for b in bars {
        let live = b.live();
        out.push(format!(
            "execute if entity @e[{live},limit=1] run function {ns}:{}",
            b.refresh_fn()
        ));
        out.push(format!(
            "execute unless entity @e[{live},limit=1] run bossbar set {} visible false",
            b.id(ns)
        ));
    }
    out
}

/// The functions every declared bar owns: the refresh, its per-body
/// accumulation, the max capture and its per-body accumulation.
pub fn functions(ns: &str, bars: &[Bar<'_>]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for b in bars {
        let id = b.id(ns);
        let live = b.live();
        let tag = b.audience_tag();
        let v = b.value_holder();
        let m = b.max_holder();
        let t = b.scratch_holder();
        let range = b.bar.range;
        out.push((
            b.refresh_fn(),
            [
                format!("scoreboard players set {v} {SYS} 0"),
                format!("execute as @e[{live}] run function {ns}:{}", b.acc_fn()),
                format!(
                    "execute store result bossbar {id} value run scoreboard players get {v} {SYS}"
                ),
                format!(
                    "execute if score {m} {SYS} matches 1.. store result bossbar {id} max run \
                     scoreboard players get {m} {SYS}"
                ),
                format!("tag @a remove {tag}"),
                format!(
                    "execute as @e[{live}] at @s run tag @a[distance=..{range},tag=!dw_cutscene,\
                     {LIVE}] add {tag}"
                ),
                format!("bossbar set {id} players @a[tag={tag}]"),
                format!("bossbar set {id} visible true"),
            ]
            .join("\n")
                + "\n",
        ));
        out.push((
            b.acc_fn(),
            [
                format!("execute store result score {t} {SYS} run data get entity @s Health 1"),
                format!("scoreboard players operation {v} {SYS} += {t} {SYS}"),
            ]
            .join("\n")
                + "\n",
        ));
        out.push((
            b.capture_fn(),
            [
                format!("scoreboard players set {m} {SYS} 0"),
                format!("execute as @e[{live}] run function {ns}:{}", b.max_acc_fn()),
            ]
            .join("\n")
                + "\n",
        ));
        out.push((
            b.max_acc_fn(),
            [
                format!(
                    "execute store result score {t} {SYS} run attribute @s minecraft:max_health \
                     get 1"
                ),
                format!("scoreboard players operation {m} {SYS} += {t} {SYS}"),
            ]
            .join("\n")
                + "\n",
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_vocabulary_is_read_from_the_pinned_tree() {
        let tree = CommandTree::v1_21_11();
        assert_eq!(
            tree.literals_under(&COLOR_PATH).unwrap(),
            vec!["blue", "green", "pink", "purple", "red", "white", "yellow"]
        );
        assert_eq!(
            tree.literals_under(&STYLE_PATH).unwrap(),
            vec![
                "notched_10",
                "notched_12",
                "notched_20",
                "notched_6",
                "progress"
            ]
        );
        assert_eq!(tree.literals_under(&["bossbar", "nope"]), None);
    }

    #[test]
    fn dw0911_is_the_code() {
        assert_eq!(DW_HEALTH_BAR_VOCABULARY.id(), "DW0911");
    }
}
