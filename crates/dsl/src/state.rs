//! Stage 5 — runtime state (DSL v0.10, spec-0031): declared data, how it is
//! displayed, written and compared.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::StateId;

#[cfg(doc)]
use crate::FlagId;

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

// ---------------------------------------------------------------------------
// Validation — the checks `dsl::validate` runs over this object (ADR-0031)
// ---------------------------------------------------------------------------

use crate::QuestEffect;
use crate::diagnostic::{Diagnostic, DwCode, ExitTier, codes};
use crate::envelope::Campaign;
use std::collections::{BTreeMap, BTreeSet};

crate::dw_code! {
    /// (spec-0076 §7) **A standing display the sidebar cannot draw as declared.**
    /// A `state[]` datum declares `display: sidebar` and the slot cannot show
    /// what it is handed: a second datum already asks for the one slot (the
    /// sidebar holds one objective, and "first wins" would hide a decision the
    /// creator has to make); the datum has no `name` (the slot's heading is the
    /// display name, and without one the objective's id would stand on screen);
    /// or the datum is `party`-scoped (its value lives on `#party`, and the
    /// sidebar hides every `#`-prefixed holder, so the display would be an empty
    /// heading). One rule about what the slot can draw, three ways to ask for
    /// what it cannot — the `DW0520` shape. Validation-tier (exit 1).
    /// Prescription: keep one `display`, give the datum a `name`, or declare it
    /// `player`-scoped; a `party` purse keeps its announcement and stands nowhere.
    pub const STATE_DISPLAY_UNDRAWABLE: DwCode = DwCode::new("DW0919", ExitTier::Build);
}

crate::dw_code! {
    /// (v0.10, spec-0031) A gate's `requires_state` reads a declared datum that
    /// **no verb anywhere in the campaign ever writes**. The datum can only ever
    /// hold its declared `initial`, so the comparison's answer was decided at
    /// authoring time and the gate is a constant wearing a condition's clothes.
    ///
    /// This is the vacuity rule at the level of one datum (CLAUDE.md: *a green
    /// gate that binds to nothing is vacuous, not a pass*) — the numeric
    /// equivalent of the bot's combat floor examining zero enemies for nineteen
    /// rounds. Validation-tier (exit 1). Prescription: write the datum somewhere
    /// (`set-state`/`add-state`/`clear-state`), or drop the comparison and say
    /// what you meant unconditionally.
    pub const STATE_NEVER_WRITTEN: DwCode = DwCode::new("DW0501", ExitTier::Build);
}

crate::dw_code! {
    /// (v0.10, spec-0031) A declared datum that **no gate anywhere in the
    /// campaign ever reads**. Either some verb writes it and nothing ever asks
    /// (the write is inert — a counter nobody consults), or nothing touches it at
    /// all (a dead declaration). Runtime state exists to be compared against; a
    /// datum with no reader is bookkeeping no player can ever observe.
    /// Validation-tier (exit 1). Prescription: gate something on it with
    /// `requires_state`, or delete the declaration and its writes.
    pub const STATE_NEVER_READ: DwCode = DwCode::new("DW0502", ExitTier::Build);
}

crate::dw_code! {
    /// (v0.10, spec-0031) A `player`-scoped datum is referenced where emission
    /// has no acting player to read or write it against.
    ///
    /// Two such places exist, and both are properties of the SITE, not of the
    /// verb: a scheduler-only bundle (a `sequence` step, a `move-npc` /
    /// `move-actor` `on_arrive`) runs with the server command source — the same
    /// seam `DW0357` polices for `carrier: "one"` — and the gates emission
    /// evaluates against the party holder rather than against a player (an
    /// objective's activation guard, a trigger's arming gate, a trap's arming
    /// gate) have no `@s` either. Validation-tier (exit 1). Prescription: declare
    /// the datum `party`-scoped if the whole party shares it, or move the
    /// read/write onto a site a player drives (a dialogue option, a cast
    /// placement, an effect on a beat a player completes).
    pub const STATE_SCOPE_UNREACHABLE: DwCode = DwCode::new("DW0503", ExitTier::Build);
}

crate::dw_code! {
    /// (v0.10, spec-0032) **A comparison read after the bundle has already changed
    /// what it compares.** An effect's `requires_state` names a datum that an
    /// EARLIER effect in the same bundle writes, so the gate is evaluated against
    /// the post-write value, not the value the beat started with.
    ///
    /// Found in the emitted output of spec-0032's own first shop. The authored
    /// shape a shop wants is "the purchase behind `at-least 1`, the apology behind
    /// `at-most 0`" — and written in that order, buying your LAST ember prints both:
    /// the debit runs, the balance falls to 0, and the apology's gate — evaluated
    /// after it — now holds. Vanilla evaluates each `execute` when it reaches it,
    /// which is the whole reason a per-effect gate is useful, so this is not a bug
    /// to fix in emission: it is an ordering hazard that only reading the generated
    /// function reveals. The fix is always the same and always local — **put the
    /// reading effect before the writing one** — which is why this is a warning
    /// naming the earlier write rather than a refusal.
    ///
    /// Warning-tier (exit 0). Prescription: move the gated effect ahead of the
    /// write, or gate it on something the bundle does not itself change.
    pub const STATE_READ_AFTER_WRITE: DwCode = DwCode::new("DW0527", ExitTier::Build);
}

crate::dw_code! {
    /// A gate contradicts itself, so it can NEVER open: a flag on both its
    /// `requires_flags` and `forbids_flags`, or `requires_state` terms on one
    /// datum that no integer satisfies (`at-least 5` with `at-most 3`, two
    /// different `equals`). The thing carrying it — objective, effect, trigger,
    /// trap, dialogue option, cast placement, shop offer — is authored content
    /// that provably never happens, which is a defect in what the document
    /// SAYS, not a stylistic lint. One rule over the whole closed consumer set
    /// ([`crate::gate::for_each_gate`]), because satisfiability is a property
    /// of the gate, never of the verb that first needed the question answered.
    ///
    /// Error tier, validation (exit 1).
    pub const GATE_NEVER_OPENS: DwCode = DwCode::new("DW0847", ExitTier::Build);
}

/// DSL v0.10 runtime-state checks (spec-0031): every reference resolves, every
/// read has a writer, every datum has a reader, and a `player`-scoped datum is
/// only touched where emission has a player to touch it against.
///
/// Both halves of the read/write obligation are here on purpose. A datum a gate
/// reads and no verb writes is a comparison whose answer was fixed when the
/// campaign was written; a datum a verb writes and no gate reads is bookkeeping
/// no player can observe. Each is a **vacuous binding** in the CLAUDE.md sense,
/// and each is silent — the campaign compiles, the datapack loads, and the delve
/// plays as though the mechanism were live.
///
/// The reads come from [`for_each_gate`](crate::gate::for_each_gate) and the
/// writes from
/// [`for_each_campaign_effect`](crate::for_each_campaign_effect) — the
/// two closed enumerations — so neither side of the ledger can drift narrower
/// than the surface it polices.
pub(crate) fn state_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let decls = &c.quests.content.state;
    // --- the declarations themselves ------------------------------------------
    let mut declared: BTreeMap<&str, &crate::StateDecl> = BTreeMap::new();
    for (i, s) in decls.iter().enumerate() {
        if !s.id.is_valid_syntax() {
            d.push(Diagnostic::error(
                codes::ID_SYNTAX,
                "quests",
                format!("/content/state/{i}/id"),
                format!(
                    "runtime-state id `{}` is malformed — ids are `state/<kebab-case>`",
                    s.id.as_str()
                ),
            ));
            continue;
        }
        if declared.insert(s.id.as_str(), s).is_some() {
            d.push(Diagnostic::error(
                codes::ID_DUPLICATE,
                "quests",
                format!("/content/state/{i}/id"),
                format!(
                    "runtime-state id `{}` is declared more than once — one datum, one \
                     declaration (its scope and its initial value have to be a single fact)",
                    s.id.as_str()
                ),
            ));
        }
    }

    // --- the standing display (spec-0076): what the sidebar can draw -----------
    //
    // One slot, one objective, drawn holder by holder under the objective's
    // display name. A declaration the slot cannot draw as written is refused
    // where it is written (`DW0919`), never resolved by order: a silent "first
    // wins" would hide the one decision this field exists to make explicit.
    let mut standing: Option<&crate::StateDecl> = None;
    for (i, s) in decls.iter().enumerate() {
        let Some(display) = s.display else {
            continue;
        };
        let slot = display.slot();
        let path = format!("/content/state/{i}/display");
        if s.name.is_none() {
            d.push(Diagnostic::error(
                STATE_DISPLAY_UNDRAWABLE,
                "quests",
                path.clone(),
                format!(
                    "`{}` asks to stand on the {slot} but has no `name`, and the {slot}'s \
                     heading is the display name — without one the objective's internal id \
                     would stand on every player's screen. Give the datum a `name`, or take \
                     `display` off it",
                    s.id.as_str()
                ),
            ));
        }
        if s.scope == crate::StateScope::Party {
            d.push(Diagnostic::error(
                STATE_DISPLAY_UNDRAWABLE,
                "quests",
                path.clone(),
                format!(
                    "`{}` is `party`-scoped and asks to stand on the {slot}, but a party \
                     datum's value lives on the `#party` holder and the {slot} hides every \
                     holder whose name starts with `#` — the display would be an empty \
                     heading. Declare it `player`-scoped if each player holds their own, or \
                     take `display` off it and keep the announcement",
                    s.id.as_str()
                ),
            ));
        }
        match standing {
            None => standing = Some(s),
            Some(first) => d.push(Diagnostic::error(
                STATE_DISPLAY_UNDRAWABLE,
                "quests",
                path,
                format!(
                    "`{}` and `{}` both ask to stand on the {slot}, which holds one \
                     objective. Keep `display` on the one datum the party reads between \
                     changes and take it off the other — the engine does not pick for you",
                    first.id.as_str(),
                    s.id.as_str()
                ),
            )),
        }
    }

    // --- the reads: every gate's `requires_state`, from the closed set --------
    let mut read: BTreeSet<String> = BTreeSet::new();
    crate::gate::for_each_gate(c, &mut |site, gate| {
        for (k, cmp) in gate.requires_state.iter().enumerate() {
            let path = format!("{}/requires_state/{k}", site.path);
            let stage = site.consumer.stage();
            match declared.get(cmp.state.as_str()) {
                None => d.push(Diagnostic::error(
                    codes::STATE_UNDECLARED,
                    stage,
                    path,
                    format!(
                        "`requires_state` on this {} compares `{}`, which the campaign never \
                         declares. Add it to the stage-5 `state` list (a datum's scope and its \
                         initial value are facts no use site can supply), or fix the id",
                        site.consumer.label(),
                        cmp.state.as_str()
                    ),
                )),
                Some(decl) => {
                    read.insert(cmp.state.as_str().to_string());
                    // An EFFECT's gate is evaluated wherever its bundle runs, and
                    // that is the root's fact, not the effect's — so
                    // `evaluates_per_player` answers `None` here and the scope
                    // check for effects happens in the root walk below, which
                    // knows both the root's audience and the seams inside it.
                    // A loop's gate is refused for a `player` datum by `DW0949`,
                    // which names the release rather than the audience; one
                    // fault, one code.
                    if decl.scope == crate::StateScope::Player
                        && site.consumer == crate::gate::GateConsumer::LethalVolume
                    {
                        // The same fault as `DW0503` on any other party-read
                        // gate, with the volume's own code and remedy
                        // (spec-0088 §3.2): one check site, the code chosen by
                        // the consumer.
                        d.push(Diagnostic::error(
                            codes::LETHAL_STAGE_GATE,
                            stage,
                            path,
                            format!(
                                "lethal volume `{}` is staged on `{}`, which is `player`-scoped — a \
                                 volume's liveness is a fact about the place, so a term one player \
                                 satisfies and another does not would be a pit that kills one body \
                                 and spares the one beside it, and the sweep's entity half has no \
                                 player to read a per-player score from. Name a flag or a \
                                 `party`-scoped datum in `when`, or leave `when` out to make the \
                                 volume live from world-load",
                                volume_id_at(c, &site.path),
                                cmp.state.as_str()
                            ),
                        ));
                    } else if decl.scope == crate::StateScope::Player
                        && site.consumer.evaluates_per_player() == Some(false)
                        && site.consumer != crate::gate::GateConsumer::Loop
                    {
                        d.push(Diagnostic::error(
                            STATE_SCOPE_UNREACHABLE,
                            stage,
                            path,
                            format!(
                                "`{}` is `player`-scoped, but emission evaluates a {}'s gate \
                                 against the party holder — there is no acting player to read it \
                                 from. Declare the datum `party`-scoped, or move the comparison \
                                 onto a site a player drives (a dialogue option, a cast \
                                 placement, or an effect on a beat a player completes)",
                                cmp.state.as_str(),
                                site.consumer.label()
                            ),
                        ));
                    }
                }
            }
        }
    });

    // --- the writes: every state verb, at every effect root, nesting included -
    let mut written: BTreeSet<String> = BTreeSet::new();
    crate::for_each_campaign_effect(c, &mut |path, _site, eff| {
        let Some((id, _)) = eff.writes_state() else {
            return;
        };
        match declared.get(id.as_str()) {
            None => d.push(Diagnostic::error(
                codes::STATE_UNDECLARED,
                "quests",
                format!("{path}/state"),
                format!(
                    "`{}` writes `{}`, which the campaign never declares. Add it to the stage-5 \
                     `state` list, or fix the id",
                    eff.verb.tag(),
                    id.as_str()
                ),
            )),
            Some(_) => {
                written.insert(id.as_str().to_string());
            }
        }
    });
    // A loop's `counts` is a write: every move raises it by one (spec-0086 §3.4).
    for l in &c.quests.content.loops {
        if let Some(counts) = &l.counts
            && declared.contains_key(counts.as_str())
        {
            written.insert(counts.as_str().to_string());
        }
    }
    // A `player`-scoped datum read or written where there is no acting player
    // has no subject, exactly as a `carrier: "one"` give does (`DW0357`).
    //
    // **The latch starts from the ROOT**, not from `false`. Four of the seven
    // roots run with an acting player and three do not
    // (`EffectRootKind::runs_with_acting_player` — a trigger's effects, a trap's
    // payload and a shortcut's `on_unlock` are all polled on the tick with no
    // executor), and inside a bundle the `sequence` / `on_arrive` seams drop the
    // actor the same way. Seeding it `false` — as this walk first did — read
    // every root as player-bearing and let three of the seven through.
    crate::effects::for_each_effect_root(c, &mut |site, effs| {
        // Per SITE, not per kind (DSL v0.11): a trigger declaring
        // `audience: presser` is dispatched by the interaction advancement and so
        // DOES have an acting player, while every other trigger is polled with
        // none. Asking the kind would refuse a `player`-scoped read that the
        // emitter can serve — the mirror of the bug this seed was added to fix.
        let scheduled = !site.runs_with_acting_player();
        check_player_state_not_scheduled(effs, &declared, site.stage, &site.path, scheduled, d);
    });

    // --- the two halves of the vacuity ledger ---------------------------------
    for (i, s) in decls.iter().enumerate() {
        let id = s.id.as_str();
        // A malformed or duplicate id has already been reported; reporting it a
        // third time as "unread" would be noise about a datum that does not exist.
        if declared.get(id).is_none_or(|kept| !std::ptr::eq(*kept, s)) {
            continue;
        }
        if read.contains(id) && !written.contains(id) {
            d.push(Diagnostic::error(
                STATE_NEVER_WRITTEN,
                "quests",
                format!("/content/state/{i}"),
                format!(
                    "`{id}` is read by a gate but no `set-state`/`add-state`/`clear-state` \
                     anywhere in the campaign ever writes it — it can only ever hold its declared \
                     initial ({}), so every comparison against it was decided when the campaign \
                     was written. Write it somewhere, or drop the comparison and say what you \
                     meant unconditionally",
                    s.initial
                ),
            ));
        }
        if !read.contains(id) {
            let tail = if written.contains(id) {
                "some verb writes it and nothing ever asks"
            } else {
                "nothing touches it at all"
            };
            d.push(Diagnostic::error(
                STATE_NEVER_READ,
                "quests",
                format!("/content/state/{i}"),
                format!(
                    "`{id}` is declared but no gate's `requires_state` anywhere in the campaign \
                     ever reads it — {tail}. Runtime state exists to be compared against; gate \
                     something on it, or delete the declaration and its writes"
                ),
            ));
        }
    }
}

/// Reject a `player`-scoped datum **read or written** where emission has no
/// acting player (`DW0503`).
///
/// `scheduled` arrives already seeded from the ROOT
/// ([`EffectRootKind::runs_with_acting_player`](crate::EffectRootKind::runs_with_acting_player)),
/// and from there the seams and the latch semantics are
/// [`check_carrier_one_not_scheduled`]'s, deliberately: a `sequence` step and a
/// `move-npc`/`move-actor` `on_arrive` are re-invoked with the server command
/// source, while a `set-checkpoint`'s `on_respawn` and a `begin-stealth`'s
/// `on_caught` are dispatched per player and so reset the latch.
///
/// Reads and writes are checked together because they fail the same way: a
/// per-player score named from a sourceless function is `@s` with nothing to
/// resolve it to, whether the command is a `scoreboard players set` or an
/// `execute if score`.
fn check_player_state_not_scheduled(
    effs: &[QuestEffect],
    declared: &BTreeMap<&str, &crate::StateDecl>,
    stage: &str,
    path: &str,
    scheduled: bool,
    d: &mut Vec<Diagnostic>,
) {
    let is_player = |id: &str| {
        declared
            .get(id)
            .is_some_and(|s| s.scope == crate::StateScope::Player)
    };
    for e in effs {
        if scheduled {
            for cmp in e.requires_state() {
                if is_player(cmp.state.as_str()) {
                    d.push(Diagnostic::error(
                        STATE_SCOPE_UNREACHABLE,
                        stage,
                        path.to_string(),
                        format!(
                            "a `{}` effect's `requires_state` compares `{}`, which is \
                             `player`-scoped, in a bundle that runs with no acting player (a \
                             trigger's effects, a trap's payload and a shortcut's `on_unlock` \
                             are polled on the tick from the server command source; so are a \
                             `sequence` step and a `move-npc`/`move-actor` `on_arrive`). There \
                             is no player to read the datum from. Declare it `party`-scoped, or \
                             move the comparison onto a beat a player completes",
                            e.verb.tag(),
                            cmp.state.as_str()
                        ),
                    ));
                }
            }
        }
        if scheduled
            && let Some((id, _)) = e.writes_state()
            && is_player(id.as_str())
        {
            d.push(Diagnostic::error(
                STATE_SCOPE_UNREACHABLE,
                stage,
                path.to_string(),
                format!(
                    "`{}` writes `{}`, which is `player`-scoped, from a bundle that runs with no \
                     acting player (a trigger's effects, a trap's payload and a shortcut's \
                     `on_unlock` are polled on the tick from the server command source; so are a \
                     `sequence` step and a `move-npc`/`move-actor` `on_arrive`). There is no \
                     acting player whose datum this would be, so the write would silently reach \
                     nobody. Declare the datum `party`-scoped, or move the write onto a beat a \
                     player completes",
                    e.verb.tag(),
                    id.as_str()
                ),
            ));
        }
        // spec-0085 §3.3: the fourth shape — an actor-addressed effect where
        // emission has no acting player. One rule, *no `@s` where emission has
        // none*, and one remedy.
        if scheduled && e.audience == Some(crate::EffectAudience::Actor) {
            d.push(Diagnostic::error(
                STATE_SCOPE_UNREACHABLE,
                stage,
                path.to_string(),
                format!(
                    "a `{}` effect declares `audience: actor` in a bundle that runs with no \
                     acting player (a polled trigger's effects, a trap's payload and a shortcut's \
                     `on_unlock` run from the server command source; so do a `move-npc`/\
                     `move-actor` `on_arrive`, a `bonfire`'s `on_rest`, and every step of a \
                     timeline started there). There is no actor to address. Move the beat onto a \
                     site a player drives (an objective's completion, a `presser` trigger, a \
                     respawn), or drop `audience` to address the party",
                    e.verb.tag()
                ),
            ));
        }
        // The seams are the DSL's one statement of them
        // (`QuestEffect::nested_effect_dispatch`): a `sequence` step keeps the
        // actor its timeline was started with (spec-0085 §3.2).
        for (list, how) in e.nested_effect_dispatch() {
            check_player_state_not_scheduled(
                list,
                declared,
                stage,
                path,
                !how.has_actor(!scheduled),
                d,
            );
        }
    }
}

/// `DW0527` — a gate that reads a datum an earlier effect in the same bundle has
/// already written.
///
/// Sibling effects in one bundle are consecutive commands in one generated
/// function, and vanilla evaluates each `execute` condition when it reaches it. So
/// a comparison placed after a write is a comparison against the post-write value —
/// which is exactly what the shop pattern spec-0032 recommends walks into if the
/// refusal is written after the purchase instead of before it.
///
/// Scope is deliberately one flat sibling list: a `sequence` step runs on a later
/// tick and a nested lifecycle bundle runs at a different moment entirely, so
/// "earlier in the same breath" is precisely a list index. Warning tier — an author
/// who means to write then compare is doing something legitimate, and the
/// diagnostic's job is to make sure they meant it.
pub(crate) fn read_after_write_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    read_after_write_walk(c, d);
}

/// What the read-after-write rule (`DW0527`) examined, zeroes included.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReadAfterWriteBinding {
    /// Effect bundles walked — the denominator.
    pub bundles: usize,
    /// Effects walked across those bundles.
    pub effects: usize,
    /// Conditional writes (gated on the datum they write) the walk recorded.
    pub gated_writes: usize,
    /// Diagnostics raised (`DW0527`).
    pub refused: usize,
}

impl ReadAfterWriteBinding {
    /// Count what [`read_after_write_checks`] examines on `c`.
    pub fn of(c: &Campaign) -> Self {
        read_after_write_walk(c, &mut Vec::new())
    }

    /// The one line this rule owes its reader.
    pub fn line(&self) -> String {
        format!(
            "read-after-write binding: {} effect(s) over {} bundle(s) walked, {} gated write(s) \
             recorded, {} refused (DW0527).",
            self.effects, self.bundles, self.gated_writes, self.refused
        )
    }
}

fn read_after_write_walk(c: &Campaign, d: &mut Vec<Diagnostic>) -> ReadAfterWriteBinding {
    let before = d.len();
    let mut b = ReadAfterWriteBinding::default();
    crate::effects::for_each_effect_root(c, &mut |site, list| {
        b.bundles += 1;
        b.effects += list.len();
        // Only a **conditional** write counts, and that narrowing is the whole
        // precision of this rule. An UNCONDITIONAL write followed by a comparison
        // is the ordinary sequenced idiom — *pay the toll, then the door opens
        // because the toll is now zero* — where the author plainly means the value
        // the bundle just produced. A write that is itself gated on the SAME datum
        // is the other thing: the bundle asks about the datum, changes it across
        // the boundary it just asked about, and then asks again — which is the
        // shape that pays for something and apologises for it in the same breath.
        let mut written: BTreeMap<&str, usize> = BTreeMap::new();
        for (i, eff) in list.iter().enumerate() {
            for cmp in eff.requires_state() {
                let Some(at) = written.get(cmp.state.as_str()) else {
                    continue;
                };
                d.push(Diagnostic::warning(
                    STATE_READ_AFTER_WRITE,
                    site.stage,
                    format!("{}/{i}/when/requires_state", site.path),
                    format!(
                        "this `{}` compares `{}`, and effect {at} of the same bundle already \
                         changes `{}` behind a gate on `{}` itself — so this comparison is made \
                         against the value THIS bundle just produced, on the far side of the \
                         boundary it just tested. The shape that bites is a purchase followed by \
                         its own refusal: buying the LAST coin debits it, and the `at-most` \
                         apology written after the debit then holds as well, so the player is \
                         charged AND told they cannot afford it. Move every reading effect ahead \
                         of the write. (An UNCONDITIONAL write followed by a comparison is not \
                         this: `set-state toll 0` and then a door gated on `toll at-most 0` \
                         plainly means the value the bundle just produced, and is not \
                         diagnosed.)",
                        eff.verb.tag(),
                        cmp.state.as_str(),
                        cmp.state.as_str(),
                        cmp.state.as_str()
                    ),
                ));
            }
            if let Some((id, _)) = eff.writes_state()
                && eff
                    .requires_state()
                    .iter()
                    .any(|c| c.state.as_str() == id.as_str())
            {
                b.gated_writes += 1;
                written.entry(id.as_str()).or_insert(i);
            }
        }
    });
    b.refused = d.len() - before;
    b
}

/// `DW0847`: a gate whose own terms contradict each other can never open, so
/// the thing carrying it is authored content that provably never happens — an
/// objective that never activates, an effect that never fires, a dialogue
/// option that never shows, a cast clause that never governs.
///
/// One rule over [`for_each_gate`](crate::gate::for_each_gate)'s closed
/// consumer set, because satisfiability is a property of the **gate** and a
/// check written beside the first verb that needed it would leave the other
/// six classes with no surface (CLAUDE.md: a capability belongs to the object
/// class it acts on). The arithmetic is [`crate::gate::Gate::contradiction`],
/// the same [`crate::gate::DatumSet`] the compiler's cast-ladder solver picks
/// drive values from — one authority, so "can this open" and "at what value"
/// can never disagree.
pub(crate) fn gate_contradiction_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    crate::gate::for_each_gate(c, &mut |site, gate| {
        let Some(contra) = gate.contradiction() else {
            return;
        };
        let what = match contra {
            crate::gate::GateContradiction::Flag(f) => {
                format!("flag `{f}` is both required and forbidden, so the gate is never satisfied")
            }
            crate::gate::GateContradiction::Datum(s) => format!(
                "no value of `{s}` satisfies every `requires_state` term that reads it, so the \
                 gate is never satisfied"
            ),
        };
        d.push(Diagnostic::error(
            GATE_NEVER_OPENS,
            site.consumer.stage(),
            site.path.clone(),
            format!(
                "this {}'s gate contradicts itself: {what}. Whatever it guards can never happen — \
                 fix the gate, or delete the thing it makes unreachable",
                site.consumer.label()
            ),
        ));
    });
}

/// The id of the lethal volume a `/content/lethal_volumes/<i>/…` pointer names,
/// for a diagnostic's wording; the pointer itself when it names none.
fn volume_id_at(c: &Campaign, path: &str) -> String {
    path.strip_prefix("/content/lethal_volumes/")
        .and_then(|rest| rest.split('/').next())
        .and_then(|i| i.parse::<usize>().ok())
        .and_then(|i| c.quests.content.lethal_volumes.get(i))
        .map_or_else(|| path.to_string(), |v| v.id.as_str().to_string())
}
