//! Stage 5 — runtime state (DSL v0.10, spec-0031): declared data, how it is
//! displayed, written and compared.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::StateId;

#[cfg(doc)]
use crate::{FlagId, QuestEffect};

/// Who holds a datum's value (DSL v0.10, spec-0031).
///
/// **Declared, never inferred.** A datum's multiplayer semantics is the one
/// thing about it that cannot be recovered from its uses: a purse read on a
/// `talk-to` looks identical whether every player has their own or the party
/// shares one, and the difference decides the whole design. spec-0031 states it
/// as a rule, and the type makes it un-omittable — there is no default.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum StateScope {
    /// Each player holds their own value. Read and written against the acting
    /// player, so a bundle with no acting player (the scheduler) cannot touch one
    /// (`DW0503`).
    Player,
    /// One value the whole party shares — the same holder story flags use
    /// (spec-0018). Readable and writable from every audience, including the
    /// scheduler.
    Party,
}

impl StateScope {
    /// The wire token (`player` / `party`).
    pub fn token(self) -> &'static str {
        match self {
            StateScope::Player => "player",
            StateScope::Party => "party",
        }
    }
}

/// One declared runtime datum (DSL v0.10, spec-0031): a named, scoped,
/// integer-valued counter.
///
/// This is what [`FlagId`] is not. A flag is boolean, party-wide and
/// **monotonic** — no verb clears one — which is exactly right for "this has
/// happened" and useless for a balance, a floor number, or "a ride is in
/// progress". A datum clears, counts down as well as up, and states its scope.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StateDecl {
    /// Unique datum id (`state/<kebab>`).
    pub id: StateId,
    /// Who holds the value. Required — see [`StateScope`].
    pub scope: StateScope,
    /// The value the datum starts at, and the value `clear-state` returns it to.
    /// Defaults to `0`.
    ///
    /// One field rather than a separate `initial` and `cleared`: "the value this
    /// datum has when nothing has happened to it yet" is one fact, and two fields
    /// would let a campaign declare a datum that can never be returned to its own
    /// starting state.
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub initial: i32,
    /// Free prose: what this datum means. Never machine-checked, never shown to a
    /// player — the forcing function that makes an author say what the number is,
    /// the same role `cast[].doing` plays.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// The player-visible name of this datum (DSL v0.10, spec-0032).
    ///
    /// **A named datum is a currency.** There is no separate `currencies` section
    /// and there deliberately is not: a purse is a runtime datum that the player
    /// can see, and "the player can see it" is a property of the datum, not a
    /// different object class. A second struct carrying `id` + `scope` +
    /// `initial` + `name` would be a private copy of this one, which is the defect
    /// CLAUDE.md names second.
    ///
    /// Present ⇒ every `set-state` / `add-state` / `clear-state` on this datum
    /// also states the new balance to whoever holds it, on the action bar, as
    /// `<name>: <value>` with the value carried by vanilla's own `score`
    /// component. Absent ⇒ the datum is silent bookkeeping and emission is exactly
    /// what it was.
    ///
    /// Player-visible, so it is inventoried under `state.<id>.name` and
    /// translated like any other authored line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Where this datum **stands** on screen between changes (spec-0076).
    ///
    /// The announcement `name` buys fades with the action bar; a datum that
    /// declares `display` also occupies a vanilla display slot, so its balance is
    /// on screen at every moment for every player. Declared, never automatic: a
    /// creator may want a named tally that is spoken only when it moves, and the
    /// engine does not decide which of two named datums is the purse.
    ///
    /// Present ⇒ `setup` heads the datum's objective with its translated `name`,
    /// paints the value gold, and puts the objective in the slot. Requires `name`
    /// (the slot's heading is the display name, and without one it would show the
    /// objective's id) and a `player` scope (the sidebar hides `#`-prefixed
    /// holders, which is what a `party` datum's value lives on); one datum per
    /// campaign may stand, because the slot holds one objective — all three are
    /// `DW0919`. Absent ⇒ the datum announces itself and stands nowhere.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<StateDisplay>,
}

/// The vanilla display slot a named datum stands in (spec-0076).
///
/// One value, because vanilla has one slot that stands for the viewer: `list`
/// shows only while the tab key is held and `below_name` draws under *other*
/// players' name tags, never over the viewer's own body. A second variant is
/// added when the pinned game offers a second standing surface, not before.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum StateDisplay {
    /// The right-hand sidebar: a heading (the datum's `name`) and one line per
    /// player, each showing that player's own balance.
    Sidebar,
}

impl StateDisplay {
    /// The `minecraft:scoreboard_slot` token the slot is addressed by.
    pub fn slot(self) -> &'static str {
        match self {
            StateDisplay::Sidebar => "sidebar",
        }
    }
}

/// serde `skip_serializing_if` helper: skip a zero `i32` (`StateDecl.initial`).
fn is_zero_i32(v: &i32) -> bool {
    *v == 0
}

/// How a [`StateCompare`] relates a datum to its operand (DSL v0.10).
///
/// Four operators, not six: over integers `less-than n` is `at-most n-1` and
/// `greater-than n` is `at-least n+1`, so the extra spellings would add a second
/// way to say one thing and a second emission path to keep honest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum CompareOp {
    /// The datum is exactly `value`.
    Equals,
    /// The datum is anything but `value`.
    NotEquals,
    /// The datum is `value` or more.
    AtLeast,
    /// The datum is `value` or less.
    AtMost,
}

impl CompareOp {
    /// The wire token (`equals` / `not-equals` / `at-least` / `at-most`).
    pub fn token(self) -> &'static str {
        match self {
            CompareOp::Equals => "equals",
            CompareOp::NotEquals => "not-equals",
            CompareOp::AtLeast => "at-least",
            CompareOp::AtMost => "at-most",
        }
    }
}

/// What one of the DSL v0.10 state verbs does to a datum — the value half of
/// [`QuestEffect::writes_state`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateWrite {
    /// `set-state`: the datum becomes this value.
    Set(i32),
    /// `add-state`: the datum moves by this signed amount.
    Add(i32),
    /// `clear-state`: the datum returns to its declared `initial`.
    Clear,
}

/// One numeric term of a gate (DSL v0.10, spec-0031): *this datum compares thus
/// to this value*.
///
/// It rides [`Gate`](crate::gate::Gate) — the shared gate, carried by every
/// consumer of `requires_flags`/`forbids_flags` — and not any one verb. The
/// comparison's consumers are exactly the gate's consumers ("this door opens at
/// 500", "this line is withheld below 200", "this lever does nothing while the
/// car is moving"), so hanging it off the first verb that asked would leave the
/// second with no surface and make a second bespoke field look like the fix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StateCompare {
    /// The datum to read. Must be declared in the stage-5 `state` list
    /// (`DW0502`).
    pub state: StateId,
    /// How to compare it.
    pub op: CompareOp,
    /// What to compare it against.
    pub value: i32,
}

impl StateCompare {
    /// Whether a datum holding `v` satisfies this comparison.
    pub fn holds(&self, v: i32) -> bool {
        match self.op {
            CompareOp::Equals => v == self.value,
            CompareOp::NotEquals => v != self.value,
            CompareOp::AtLeast => v >= self.value,
            CompareOp::AtMost => v <= self.value,
        }
    }
}
