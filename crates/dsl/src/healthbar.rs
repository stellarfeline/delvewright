//! **A fight shows its health** (spec-0073): the `health_bar` a wave or an
//! actor may declare, and the document-tier rules that judge it.
//!
//! A bar acts on a *fight*, and the engine has exactly two fight classes — the
//! wave (`dw_wave_<id>`) and the actor (`dw_actor_<id>`) — the two that carry
//! [`EncounterTier`] and the two the undefeated re-seat is defined over. One
//! type serves both, on the `equipment` precedent: one shape on two object
//! classes, so the two surfaces cannot drift.
//!
//! The bar is **declared, never derived from `tier`**: `tier` is a declaration
//! the compiler may not scale content from, and drawing a bar because a fight is
//! billed `boss` would make the billing a knob. A `boss`-billed fight with no bar
//! is legal and builds; it is advised ([`codes::HEALTH_BAR_ADVISED`], warning
//! tier), and no other tier ever is.
//!
//! What lives here, and what does not:
//!
//! * [`health_bar_checks`] — the rules that need only the documents: the
//!   schema's `range` bound restated at the document tier (`DW0100`), a bar over
//!   a body whose health cannot move (`DW0909`), a bar with nothing to title it
//!   (`DW0910`), and the advisory (`DW0912`).
//! * The colour and style vocabulary is **not** here. It is whatever the pinned
//!   command tree lists under `bossbar set <id> color|style`, and that tree is
//!   `delvec`'s data, so the rule that reads it (`DW0911`) is compiler-side —
//!   the one authority the emitter already holds every line to, never a second
//!   copy of it in this crate.
//! * [`HealthBarBinding`] — what the rules examined on a campaign, with the
//!   denominator, so a campaign that declares no bar reads as zero bound rather
//!   than as a pass.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::diagnostic::{Diagnostic, codes};
use crate::envelope::Campaign;
use crate::fight::{Fight, FightKind, fights, unleashed_actors};
use crate::stages::EncounterTier;

/// The smallest `range` a bar may declare, in blocks — the same bound
/// `lane.aggro_radius` carries.
pub const MIN_RANGE: u32 = 4;
/// The largest `range` a bar may declare, in blocks.
pub const MAX_RANGE: u32 = 64;

/// A health bar over one fight (spec-0073 §2): a named bar over the fight's
/// total health, drawn for every player within `range` blocks of a live body of
/// the fight, from the moment they enter that range until the last body falls.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HealthBar {
    /// The bar's player-visible title. Absent = the fight's own name: an actor's
    /// `name`, or the `name` of a wave's one mob entry. A wave of two entries or
    /// more, or a body with no name, must state it (`DW0910`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// How near a live body of the fight a player must stand to see the bar, in
    /// blocks (4..=64): the creator's statement of the arena. Required — the bar
    /// is meant to be seen on crossing the threshold, before the body has
    /// perceived anyone, so it is not the body's `follow_range`.
    #[schemars(range(min = 4, max = 64))]
    pub range: u32,
    /// The bar's colour: one of the literals the pinned game's
    /// `bossbar set <id> color` accepts (`DW0911`). Absent = the game's default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// The bar's style: one of the literals the pinned game's
    /// `bossbar set <id> style` accepts (`DW0911`). Absent = the game's default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
}

impl<'a> Fight<'a> {
    /// The title the fight's bar draws: a stated `title` always wins, otherwise
    /// the fight's own name (spec-0073 §5). `None` when the fight declares no
    /// bar, or when there is nothing to draw, which is `DW0910`.
    pub fn bar_title(&self) -> Option<&'a str> {
        let bar = self.health_bar()?;
        match bar.title.as_deref() {
            Some(t) => Some(t),
            None => self.own_name(),
        }
    }
}

/// The document-tier health-bar rules (spec-0073 §8): `DW0100` for a `range`
/// outside the schema's bound, `DW0909`, `DW0910`, and the `DW0912` advisory.
/// Every walk is over [`fights`]; a campaign that declares no bar and bills no
/// fight `boss` gets nothing.
pub fn health_bar_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let unleashed = unleashed_actors(c);
    for (path, f) in fights(c) {
        let Some(bar) = f.health_bar() else {
            if f.tier() == Some(EncounterTier::Boss) {
                d.push(Diagnostic::warning(
                    codes::HEALTH_BAR_ADVISED,
                    "quests",
                    format!("{path}/tier"),
                    format!(
                        "{kind} `{id}` is billed `tier: boss` and declares no `health_bar`, so a \
                         player fighting it sees nothing of how the fight is going. This is \
                         advice, not a refusal: the build goes on. To show it, declare \
                         `health_bar: {{ range: <blocks> }}` on the {kind} — a bar titled with \
                         its name, over its bodies' total health, drawn for every player within \
                         `range` blocks of it.",
                        kind = f.word(),
                        id = f.id(),
                    ),
                ));
            }
            continue;
        };
        let bar_path = format!("{path}/health_bar");
        if !(MIN_RANGE..=MAX_RANGE).contains(&bar.range) {
            d.push(Diagnostic::error(
                codes::SCHEMA,
                "quests",
                format!("{bar_path}/range"),
                format!(
                    "`health_bar` `range` {} on {} `{}` is outside {MIN_RANGE}..={MAX_RANGE}. It \
                     is the distance, in blocks, at which a player standing near a live body of \
                     the fight sees its bar: below {MIN_RANGE} the bar appears only once the \
                     player is already in contact, and past {MAX_RANGE} it is drawn across \
                     rooms that are not the fight's. State the arena's size in \
                     {MIN_RANGE}..={MAX_RANGE}.",
                    bar.range,
                    f.word(),
                    f.id()
                ),
            ));
        }
        match bar.title.as_deref() {
            Some(t) if t.trim().is_empty() => d.push(Diagnostic::error(
                codes::HEALTH_BAR_UNTITLED,
                "quests",
                format!("{bar_path}/title"),
                format!(
                    "`health_bar` `title` on {} `{}` is blank, so the bar would be drawn with \
                     nothing on it. Write the title the player reads over the bar, or remove \
                     `title` to draw the fight's own name.",
                    f.word(),
                    f.id()
                ),
            )),
            Some(_) => {}
            None if f.own_name().is_none() => {
                let why = match f.kind() {
                    FightKind::Wave if f.entries() != 1 => format!(
                        "the wave declares {} mob entries, and no one entry's name is the \
                         name of the whole fight",
                        f.entries()
                    ),
                    FightKind::Wave => "its one mob entry declares no `name`".to_string(),
                    FightKind::Actor => "the actor declares no `name`".to_string(),
                };
                d.push(Diagnostic::error(
                    codes::HEALTH_BAR_UNTITLED,
                    "quests",
                    bar_path.clone(),
                    format!(
                        "`health_bar` on {} `{}` has nothing to title it: it states no `title`, \
                         and {why}. The engine owns no wording that is right for every fight, \
                         so it does not invent one. State `title`, or name the body.",
                        f.word(),
                        f.id()
                    ),
                ));
            }
            None => {}
        }
        if f.kind() == FightKind::Actor && !f.vulnerable() && !unleashed.contains(f.id()) {
            d.push(Diagnostic::error(
                codes::HEALTH_BAR_STILL,
                "quests",
                bar_path.clone(),
                format!(
                    "`health_bar` on actor `{}` is over a body whose health can never move: the \
                     actor is not `vulnerable`, so its puppet is invulnerable, and no \
                     `unleash-actor` names it, so no body that can be hurt ever stands in its \
                     place. The bar would sit full for as long as the puppet stands. Unleash it \
                     somewhere, mark it `vulnerable: true`, or remove the bar.",
                    f.id()
                ),
            ));
        }
    }
}

/// What the health-bar rules examined on one campaign, with the denominator
/// (CLAUDE.md, vacuity): how many fights were declared, how many carry a bar,
/// how many are billed `boss` and how many of those carry one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HealthBarBinding {
    /// Fights declared — waves plus actors. The denominator.
    pub fights: usize,
    /// Waves declared.
    pub waves: usize,
    /// Actors declared.
    pub actors: usize,
    /// Fights carrying a bar — the objects the rules bind.
    pub with_bar: usize,
    /// Fights billed `boss`.
    pub boss: usize,
    /// Boss-billed fights carrying a bar.
    pub boss_with_bar: usize,
    /// Refusals raised (`DW0100` on a bar's `range`, `DW0909`, `DW0910`).
    pub refused: usize,
    /// Advisories raised (`DW0912`).
    pub advised: usize,
}

impl HealthBarBinding {
    /// Count what [`health_bar_checks`] examines on `c`.
    pub fn of(c: &Campaign) -> Self {
        let mut b = HealthBarBinding::default();
        for (_, f) in fights(c) {
            b.fights += 1;
            match f.kind() {
                FightKind::Wave => b.waves += 1,
                FightKind::Actor => b.actors += 1,
            }
            let boss = f.tier() == Some(EncounterTier::Boss);
            if boss {
                b.boss += 1;
            }
            if f.health_bar().is_some() {
                b.with_bar += 1;
                if boss {
                    b.boss_with_bar += 1;
                }
            }
        }
        let mut d = Vec::new();
        health_bar_checks(c, &mut d);
        b.advised = d
            .iter()
            .filter(|x| x.code == codes::HEALTH_BAR_ADVISED.id())
            .count();
        b.refused = d.len() - b.advised;
        b
    }

    /// The one line this rule owes its reader.
    pub fn line(&self) -> String {
        format!(
            "health-bar binding: {} of {} fight(s) carry a bar ({} wave(s), {} actor(s) \
             declared); {} of {} boss-billed fight(s) carry one; {} refused (DW0100/DW0909/\
             DW0910), {} advised (DW0912).",
            self.with_bar,
            self.fights,
            self.waves,
            self.actors,
            self.boss_with_bar,
            self.boss,
            self.refused,
            self.advised
        )
    }
}
