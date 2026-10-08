//! Environment triggers and the props a player interacts with.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::serde_fields::default_true;
use crate::{AnchorId, AssemblyId, FlagId, NpcId, QuestEffect, StateCompare, TriggerId};

#[cfg(doc)]
use crate::stepped_blocks;

/// A stage-5 environment trigger (DSL v0.4). Emission uses vanilla-intended
/// primitives only (spec-0008 §7): `strike`/`use` read a `minecraft:interaction`
/// entity's attack/interaction records; `approach` is a `distance` selector on
/// the tick. Look-at / break-attempt detection is excluded on principle (no
/// vanilla primitive).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EnvTrigger {
    /// Unique trigger id (`trigger/<kebab>`).
    pub id: TriggerId,
    /// The anchor this trigger watches. Required for `strike` / `use` /
    /// `approach`, which watch a *place*; **absent** for `strike-npc` (DSL
    /// v0.6), which watches a *character* and names it in `on.npc` instead —
    /// there is no cell for the author to supply and no cell the compiler
    /// would use. Either mismatch is `DW0194`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<AnchorId>,
    /// **The visible object the click acts on** (spec-0093 §6.5): for a `use`
    /// or a `strike` trigger, the block the compiler places at `at` — a lever,
    /// a bell, a lamp, a stone. A `use` on a block vanilla reports the use of
    /// (a lever, a button, a bell) fires through vanilla's `default_block_use`
    /// criterion and summons no hitbox; any other prop, and every `strike`, is
    /// placed with the `minecraft:interaction` hitbox fitted over it as its hit
    /// area. A click trigger with no `prop` on open air is refused (`DW0963`):
    /// the hitbox is invisible and is never the object. A `prop` on an event
    /// with no cell of its own — an approach, a `strike-npc`, a
    /// `strike-assembly` — is `DW0964`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prop: Option<Prop>,
    /// The event that fires it.
    pub on: TriggerOn,
    /// Flags that must be set before the trigger can fire (DSL v0.4).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_flags: Vec<FlagId>,
    /// Negative flag gate (DSL v0.6): the trigger is
    /// **suppressed** while ANY listed flag is set (by any player — flags are
    /// campaign state). The dual of `requires_flags`, so an "armed between two
    /// story beats" trigger needs no re-arm plumbing: e.g. a strike-the-giant
    /// retaliation trigger with `requires_flags: [flag/sealed]` and
    /// `forbids_flags: [flag/asleep]` arms when the cave seals and stands down
    /// the moment the wake beat takes over.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forbids_flags: Vec<FlagId>,
    /// Numeric gate terms (DSL v0.10, spec-0031): every listed comparison must
    /// hold for this gate to be open. The third field of the one gate, carried by
    /// every gate consumer — never by the verb that first wanted it. Default
    /// empty, so a pre-0.10 campaign is byte-identical.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_state: Vec<StateCompare>,
    /// Fire at most once (default `true`, mirroring objective completion). Set
    /// `false` to allow re-firing every time the condition is met.
    #[serde(default = "default_true")]
    pub once: bool,
    /// **Who the trigger's effects address** (DSL v0.11). Default
    /// [`TriggerAudience::Party`], so every campaign written before this field
    /// existed is byte-identical.
    ///
    /// A trigger is two different things depending on what the author means by
    /// it. A pressure plate that opens a gate and narrates the room is a **party
    /// beat**: everyone should see it, and it does not matter who stepped on the
    /// plate. A barred door that answers *"this cannot be opened from this
    /// side"* is a **reply to one person**: broadcasting it tells four players
    /// about a door three of them are nowhere near.
    ///
    /// Until this field the second was inexpressible, which is why the two verbs
    /// that needed it (`close-gate`'s seal answer, and nothing at all for a
    /// shortcut door) grew their own private reply machinery instead. The
    /// capability belongs to the press, not to the verb.
    #[serde(default, skip_serializing_if = "TriggerAudience::is_party")]
    pub audience: TriggerAudience,
    /// Effects fired when the trigger matches.
    pub effects: Vec<QuestEffect>,
}

impl EnvTrigger {
    /// The anchor this trigger watches, if it watches a place at all. `None`
    /// for `strike-npc`, whose target is a character.
    pub fn at_anchor(&self) -> Option<&str> {
        self.at.as_ref().map(|a| a.as_str())
    }

    /// Whether this trigger's bundle is addressed to the player who pressed it.
    pub fn addresses_presser(&self) -> bool {
        self.audience == TriggerAudience::Presser
    }

    /// Whether vanilla can name the player whose act fired this trigger, which
    /// is what `audience: presser` needs: a right-click (`use`, through
    /// `minecraft:player_interacted_with_entity`) and a step (`step`, a player
    /// in the cell). A left-click is recorded as a UUID no command can become;
    /// everything else is refused by `DW0427`.
    pub fn attributes_its_actor(&self) -> bool {
        matches!(self.on, TriggerOn::Use | TriggerOn::Step)
    }
}

/// Who an [`EnvTrigger`]'s effects address (DSL v0.11).
///
/// **This is a dispatch decision, not a cosmetic one.** A `party` trigger is
/// polled on the tick with no executor, so `@s` does not exist and every
/// player-facing command addresses `@a`. A `presser` trigger is dispatched by a
/// `minecraft:player_interacted_with_entity` advancement — the one vanilla
/// primitive that runs a function *as the player who clicked* — so `@s` is the
/// presser and the bundle addresses them alone. A `presser` trigger `on: step`
/// is polled as `execute as @a[<the cell>]`, so `@s` is each player who stepped
/// on, on their own step.
///
/// The click primitive exists for **right-clicks only**. Vanilla records a
/// left-click on an interaction entity in NBT (which names a UUID no command can
/// become) and offers no criterion for it, so `presser` on a `strike` is refused
/// (`DW0427`) rather than approximated: per CLAUDE.md's no-hack rule, a
/// capability with no vanilla primitive under it is excluded, never faked
/// downstream.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TriggerAudience {
    /// The whole party (the default, and what every trigger did before v0.11).
    #[default]
    Party,
    /// The one player whose press fired it: the right-click of a `use`, the
    /// step of a `step`.
    Presser,
}

impl TriggerAudience {
    /// Serde skip predicate: the default needs no field on the wire, so a
    /// canonical round-trip of a pre-0.11 campaign is byte-identical.
    fn is_party(&self) -> bool {
        *self == TriggerAudience::Party
    }
}

/// The event an [`EnvTrigger`] watches (DSL v0.4).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "on", rename_all = "kebab-case", deny_unknown_fields)]
pub enum TriggerOn {
    /// The player attacks (left-clicks) the interaction entity at the anchor.
    Strike,
    /// The player uses (right-clicks) the interaction entity at the anchor.
    Use,
    /// The player comes within `range` blocks of the anchor.
    Approach {
        /// Approach radius (blocks).
        range: u32,
    },
    /// A player steps onto the anchor's cell, which holds a block a step fires
    /// — a pressure plate or the tripwire string ([`stepped_blocks`]); the
    /// piece places it, and a cell that holds anything else is `DW0917`.
    ///
    /// Detected as a player whose hitbox is in the cell (the selector a plate
    /// or tripwire trap fires on), edge-latched so standing on the plate fires
    /// once. With `audience: presser` each player who steps on is dispatched
    /// as `@s` on their own step: the act and the actor are the same fact, a
    /// body in the cell, so no player is inferred after the event.
    Step,
    /// The player attacks (left-clicks) an **NPC's body** (DSL v0.6).
    ///
    /// The place-based [`TriggerOn::Strike`] cannot express "hit the giant": it
    /// summons its own `minecraft:interaction` at a *cell*, and a large NPC's
    /// body eclipses that cell (`DW0359`), so the click never reaches the
    /// trigger — the owner's island round-7 finding. This form has no cell. It
    /// rides the interaction entity the NPC already owns, which is the entity a
    /// click on that NPC reaches by construction.
    ///
    /// Right-click and left-click stay separate all the way down: a
    /// `minecraft:interaction` records them in two distinct NBT fields
    /// (`interaction` and `attack`), so the NPC's dialogue keeps the right-click
    /// and this trigger takes the left-click, on one shared hitbox.
    StrikeNpc {
        /// The NPC (stage-2 ref) whose body is the target.
        npc: NpcId,
    },
    /// The player attacks (left-clicks) an **assembly's hitbox** (spec-0082).
    ///
    /// The exact shape of [`TriggerOn::StrikeNpc`]: no `at`, because the target
    /// is an object with a hitbox of its own, and the trigger rides it. Melee
    /// only — the hitbox is a `minecraft:interaction`, and an arrow passes
    /// through one without writing its `attack` record (spec-0082 §8 row 5).
    /// `once: false` with an `add-state` is how a hit count is built.
    StrikeAssembly {
        /// The assembly (stage-5 `assemblies` ref) whose hitbox is the target.
        assembly: AssemblyId,
    },
}

impl TriggerOn {
    /// The kebab tag (`strike` / `use` / `approach` / `step` / `strike-npc` /
    /// `strike-assembly`).
    pub fn kind(&self) -> &'static str {
        match self {
            TriggerOn::Strike => "strike",
            TriggerOn::Use => "use",
            TriggerOn::Approach { .. } => "approach",
            TriggerOn::Step => "step",
            TriggerOn::StrikeNpc { .. } => "strike-npc",
            TriggerOn::StrikeAssembly { .. } => "strike-assembly",
        }
    }

    /// Whether this event is a click on a `minecraft:interaction` hitbox — a
    /// `strike`, a `use`, a `strike-npc`, a `strike-assembly`. An `approach`
    /// and a `step` are a body's position, read on the tick, and have no
    /// hitbox.
    pub fn is_click(&self) -> bool {
        matches!(
            self,
            TriggerOn::Strike
                | TriggerOn::Use
                | TriggerOn::StrikeNpc { .. }
                | TriggerOn::StrikeAssembly { .. }
        )
    }

    /// Whether this event needs an `at` anchor — true for everything that
    /// watches a place, false for `strike-npc` and `strike-assembly`, which
    /// watch an object that carries its own hitbox.
    pub fn needs_anchor(&self) -> bool {
        !matches!(
            self,
            TriggerOn::StrikeNpc { .. } | TriggerOn::StrikeAssembly { .. }
        )
    }

    /// The assembly whose hitbox this event watches (`strike-assembly` only).
    pub fn assembly_target(&self) -> Option<&AssemblyId> {
        match self {
            TriggerOn::StrikeAssembly { assembly } => Some(assembly),
            _ => None,
        }
    }

    /// The NPC whose body this event watches (`strike-npc` only).
    pub fn npc_target(&self) -> Option<&NpcId> {
        match self {
            TriggerOn::StrikeNpc { npc } => Some(npc),
            _ => None,
        }
    }
}

/// A prop block for an `interact` objective or a `use` trigger (DSL v0.4;
/// spec-0093 §6.5). The block is the affordance the player interacts with; its
/// id is validated against the pinned 1.21.11 block registry (`DW0193`).
///
/// **When the block is one a hand presses** — a lever or a button
/// ([`crate::blockshape::is_hand_pressed`]) — the block IS the detector: the
/// compiler summons no `minecraft:interaction` hitbox and the act is vanilla's
/// own, reported by the `default_block_use` advancement criterion at the
/// block's cell. Any other block is placed and the invisible hitbox stands in
/// its cell, because vanilla reports no use of it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Prop {
    /// Vanilla block id, with an optional blockstate suffix (e.g.
    /// `minecraft:lever[face=floor,facing=north]`).
    pub block: String,
}

impl Prop {
    /// Whether this prop's block is one a hand presses — the block is then the
    /// act's own detector (spec-0093 §6.5).
    pub fn is_hand_pressed(&self) -> bool {
        crate::blockshape::is_hand_pressed(&self.block)
    }
}

// ---------------------------------------------------------------------------
// Validation — the checks `dsl::validate` runs over this object (ADR-0031)
// ---------------------------------------------------------------------------

use crate::diagnostic::{Diagnostic, DwCode, ExitTier, codes};
use crate::envelope::Campaign;
use crate::quest::check::check_effect_v04;
use crate::registry::{AnchorRegistry, BlockRegistry};
use crate::validate::{
    AnchorProviders, check_block_field, for_each_trigger_effect_deep, station_kind_diag,
};
use std::collections::{BTreeMap, BTreeSet};

crate::dw_code! {
    /// (v0.4) An environment trigger id is malformed (`DW0110`-style) or
    /// duplicated within the stage-5 `triggers` namespace.
    pub const TRIGGER_INVALID: DwCode = DwCode::new("DW0194", ExitTier::Build);
}

crate::dw_code! {
    /// (v0.4, added round-6) A `use` trigger anchored where an NPC stands.
    /// Right-click on an NPC already belongs to its dialogue advancement; a
    /// second interaction hitbox in the same cell makes the client's entity
    /// ray-pick ambiguous, and whichever entity loses the tie is silently dead
    /// — the round-6 island soft-lock class (an exactly co-located hitbox
    /// starved the giant's dialogue of every right-click). `strike` triggers
    /// are exempt: a left-click has no dialogue meaning, so the compiler rides
    /// the trigger's tag on the NPC's own hitbox instead of summoning a second
    /// one. Validation-tier (exit 1).
    pub const USE_TRIGGER_ON_NPC: DwCode = DwCode::new("DW0350", ExitTier::Build);
}

crate::dw_code! {
    /// (v0.11) **A press answer addressed to a click vanilla cannot attribute.**
    /// A trigger declares `audience: presser` on something other than an
    /// `on: use` or an `on: step`.
    ///
    /// `minecraft:player_interacted_with_entity` is the only vanilla criterion
    /// that runs a function as the player who clicked, and it fires on
    /// right-clicks alone; a step is a player standing in the cell, which a
    /// positional selector names. A left-click is recorded in the interaction entity's
    /// `attack` NBT — a UUID no command can become — and an `approach` involves no
    /// click at all. Approximating it (polling the record and assuming the nearest
    /// player) is the downstream folklore CLAUDE.md's no-hack rule excludes, so the
    /// capability is refused rather than faked.
    pub const TRIGGER_AUDIENCE_UNATTRIBUTABLE: DwCode = DwCode::new("DW0427", ExitTier::Build);
}

crate::dw_code! {
    /// (v0.11) **A trigger id in the compiler's reserved `dw-` namespace.** The
    /// compiler synthesizes triggers of its own — today the press answer every
    /// sealed gate and shortcut door gives (`trigger/dw-press-…`) — and two
    /// triggers sharing an id would share one `dw_trig_…` tag and one emitted
    /// function, so one of them would silently disappear. Reserving the prefix
    /// makes the collision impossible by construction instead of improbable.
    pub const TRIGGER_ID_RESERVED: DwCode = DwCode::new("DW0428", ExitTier::Build);
}

crate::dw_code! {
    /// (v0.11) **A sealed body with no press answer**, uniformly over the
    /// pressable class. A `shortcuts[]` door or
    /// a `close-gate`'s wall is sealed, and nothing says what it answers when the
    /// party presses it — no `use` trigger anchored on it, and (for a
    /// `close-gate`) no authored `sealed_hint`.
    ///
    /// The compiler deliberately does **not** fill that silence. A baked default
    /// is the compiler making a design statement — about tone, about what this
    /// specific door is — on the author's behalf, and then never telling them it
    /// did; an error makes the author say it. Same rule as "no hacks at any
    /// layer": if content needs a thing, the DSL exposes it and the author
    /// declares it, rather than a lower layer inventing it.
    ///
    /// One rule for the whole pressable class: two objects of the same class do
    /// not get two defaulting policies, which would be the "capability keyed to
    /// the verb" defect this very surface is CLAUDE.md's worked example of.
    pub const SEALED_BODY_UNANSWERED: DwCode = DwCode::new("DW0429", ExitTier::Build);
}

/// `DW0427`/`DW0428`: the two ways a trigger's **press answer** surface can be
/// declared wrong (DSL v0.11).
///
/// Both are about the trigger as an *object*, not about any effect inside it, so
/// they sit together and are checked over the one trigger authority.
pub(crate) fn press_answer_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    for (i, t) in c.quests.content.triggers.iter().enumerate() {
        if t.addresses_presser() && !t.attributes_its_actor() {
            d.push(Diagnostic::error(
                TRIGGER_AUDIENCE_UNATTRIBUTABLE,
                "quests",
                format!("/content/triggers/{i}/audience"),
                format!(
                    "trigger `{}` watches a `{}` and asks for `audience: presser`, but vanilla \
                     names the player only for a RIGHT-click and a step. \
                     `minecraft:player_interacted_with_entity` is the one criterion that runs a \
                     function as the clicker, and a step is a player standing in the cell; a \
                     left-click is recorded in the interaction entity's `attack` NBT, which names \
                     a UUID no command can become, and an `approach` is not attributed. Guessing \
                     — polling the record and hoping the nearest player is the striker — is the \
                     kind of downstream folklore this engine refuses (CLAUDE.md: a capability \
                     with no vanilla primitive under it is excluded, not faked). Prescription: \
                     make it an `on: use` or `on: step` trigger, or drop `audience` and let the \
                     beat address the party",
                    t.id,
                    t.on.kind()
                ),
            ));
        }
        let local = crate::l10n::local_id(t.id.as_str());
        if local.starts_with(RESERVED_TRIGGER_PREFIX) {
            d.push(Diagnostic::error(
                TRIGGER_ID_RESERVED,
                "quests",
                format!("/content/triggers/{i}/id"),
                format!(
                    "trigger id `{}` opens with `{RESERVED_TRIGGER_PREFIX}`, which the compiler \
                     reserves for the triggers it synthesizes itself — today the press answer \
                     every sealed gate and shortcut door gives (`trigger/dw-press-…`). Two \
                     triggers with one id would share one `dw_trig_…` tag and one emitted \
                     function, so one of them would silently vanish. Prescription: rename it; any \
                     kebab id not starting with `{RESERVED_TRIGGER_PREFIX}` is yours",
                    t.id
                ),
            ));
        }
    }
}

/// `DW0429`: **a sealed body the campaign never answers** (DSL v0.11),
/// uniformly over the pressable class.
///
/// A sealed thing is something the party walks up to and pushes on — a `shortcut`
/// door on the wrong side of the loop, a `close-gate`'s wall — and the press has
/// to say something. The compiler will not say it for them: a baked default is a
/// design statement (about tone, about what this thing is) made on the author's
/// behalf and never disclosed, so the obligation is stated instead of filled.
///
/// **One rule for the whole pressable class, not one per verb.** A shortcut door
/// and a sealed gate are two objects of the same class, and giving them two
/// defaulting policies would be exactly the "capability keyed to the verb" defect
/// CLAUDE.md's worked example is about — which this surface *is*. So above the
/// fence both are held to the same obligation, and `plan::press_answer_sites`
/// carries the single shared list they are read from.
///
/// **Two ways to discharge it**, and they are the same thing said at two layers:
///
/// * a `use` trigger anchored on the body — the general verb, available to every
///   pressable object (`QuestsContent::answers_press_at`);
/// * for a `close-gate`, an authored `sealed_hint` — the sugar, which *is* the
///   author defining the wording. The compiler lowering that onto the general
///   path is not the compiler putting words in a player's mouth.
///
/// A `strike` discharges neither: pressing a thing is a right-click, and a
/// left-click reply is a gesture the player may never make.
pub(crate) fn press_obligation_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let quests = &c.quests.content;

    // Every gate anchor some `close-gate` seals, and whether any firing on it
    // authored a wording. Keyed by ANCHOR because the seal is a place, not an
    // event — the same reason `plan::collect_seal_hints` dedups by anchor and
    // `DW0423` refuses two firings that disagree.
    let mut sealed: BTreeMap<&str, (bool, String)> = BTreeMap::new();
    crate::for_each_campaign_effect(c, &mut |path, _site, eff| {
        let Some(anchor) = eff.close_gate_anchor() else {
            return;
        };
        let entry = sealed
            .entry(anchor.as_str())
            .or_insert_with(|| (false, path.to_string()));
        entry.0 |= eff.close_gate_sealed_hint().is_some();
    });
    for (anchor, (authored, path)) in sealed {
        if authored || quests.answers_press_at(anchor) {
            continue;
        }
        d.push(Diagnostic::error(
            SEALED_BODY_UNANSWERED,
            "quests",
            path,
            format!(
                "this `close-gate` seals `{anchor}`, and nothing says what the wall answers when \
                 the party presses it. A seal is a thing the party walks back to and pushes on, \
                 so the press has to say something — and the compiler will not word it for you: a \
                 baked default decides this wall's tone on your behalf and never tells you it \
                 did. Two ways to say it, and either is enough: add `\"sealed_hint\": \"<what the \
                 wall says>\"` to this effect, or anchor a trigger on the gate — \
                 `{{\"id\": \"trigger/<name>\", \"at\": \"{anchor}\", \"on\": {{\"on\": \"use\"}}, \
                 \"once\": false, \"audience\": \"presser\", \"effects\": [{{\"type\": \"narrate\", \
                 \"style\": \"actionbar\", \"text\": \"<what the wall says>\"}}]}}`. The trigger form \
                 is the general one and can carry a sound, a flag gate or any other effect"
            ),
        ));
    }

    for (i, sc) in quests.shortcuts.iter().enumerate() {
        let gate = sc.gate.as_str();
        if quests.answers_press_at(gate) {
            continue;
        }
        d.push(Diagnostic::error(
            SEALED_BODY_UNANSWERED,
            "quests",
            format!("/content/shortcuts/{i}"),
            format!(
                "shortcut `{}` bars the gate `{gate}` from world-load, and nothing in the \
                 campaign answers a right-click on it — so a player who walks the long way \
                 round, arrives at the wrong side of the door and pushes on it is told nothing. \
                 That is the press a shortcut loop most invites. The compiler will not word it \
                 for you: a baked default would be the engine deciding this door's tone and \
                 never saying that it had. A `shortcut` carries no wording field, deliberately — \
                 the line is a trigger. Prescription: add a trigger anchored on the gate — \
                 `{{\"id\": \"trigger/<name>\", \"at\": \"{gate}\", \"on\": {{\"on\": \"use\"}}, \
                 \"once\": false, \"audience\": \"presser\", \"effects\": [{{\"type\": \"narrate\", \
                 \"style\": \"actionbar\", \"text\": \"<what the door says>\"}}]}}` — which rides \
                 the door's own hitboxes, fires only from the sealed side, and retires when the \
                 door opens. Any `use` trigger on `{gate}` discharges this, whatever it does",
                sc.id
            ),
        ));
    }
}

/// The id prefix the compiler reserves for triggers it synthesizes
/// (`plan::press_answer_trigger_id`). Stated here because the *reservation* is a
/// DSL-level fact even though today's only user is in the compiler.
const RESERVED_TRIGGER_PREFIX: &str = "dw-";

/// Every effect anchor of an environment trigger (`DW0142`/`DW0871`), at any
/// nesting depth, resolved against the union of every known area's anchors.
pub(crate) fn trigger_anchor_checks(
    c: &Campaign,
    providers: &AnchorProviders,
    d: &mut Vec<Diagnostic>,
) {
    // Environment triggers are global (no owning area), so their effect anchors
    // resolve against the union of every known area's anchors — the same
    // resolved-or-diagnostic rule as quest effects, applied at the only scope a
    // trigger has. Skipped entirely when some area binds a pool / an unknown
    // prefab, because then the union is not the whole truth.
    if providers.all_areas_known() {
        for (ti, t) in c.quests.content.triggers.iter().enumerate() {
            for_each_trigger_effect_deep(t, |path, eff| {
                for (suffix, anchor, demands) in eff.anchor_refs() {
                    if let Some(f) = station_kind_diag(
                        providers,
                        anchor.as_str(),
                        demands,
                        &format!("`{}`", eff.verb.tag()),
                        "quests",
                        format!("/content/triggers/{ti}/{path}/{suffix}"),
                    ) {
                        d.push(f);
                        continue;
                    }
                    if providers.union().contains(anchor.as_str()) {
                        continue;
                    }
                    d.push(Diagnostic::error(
                        codes::ANCHOR_UNRESOLVED,
                        "quests",
                        format!("/content/triggers/{ti}/{path}/{suffix}"),
                        format!(
                            "`{verb}` anchor `{anchor}` in an environment trigger is not \
                             provided by any area's prefab — {}",
                            providers.anchor_remedy(
                                "use an anchor a prefab exposes (anchor names come from prefab \
                                 metadata; do NOT invent one)"
                            ),
                            verb = eff.verb.tag(),
                        ),
                    ));
                }
            });
        }
    }
}

/// An environment trigger's effect `requires_flags` / `forbids_flags`, at any
/// nesting depth, name a produced flag (`DW0172`).
pub(crate) fn trigger_effect_flag_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let quests = &c.quests.content;

    let declared_flags: BTreeSet<String> = crate::validate::produced_flags(c);

    // v0.6: environment-trigger effect `requires_flags` / `forbids_flags`
    // resolution (DW0172).
    for (i, t) in quests.triggers.iter().enumerate() {
        for_each_trigger_effect_deep(t, |path, eff| {
            for (n, f) in eff.requires_flags().iter().enumerate() {
                if !declared_flags.contains(f.as_str()) {
                    d.push(Diagnostic::error(
                        codes::FLAG_UNKNOWN,
                        "quests",
                        format!("/content/triggers/{i}/{path}/when/requires_flags/{n}"),
                        format!(
                            "effect `requires_flags` references flag `{f}`, which no `set-flag` \
                             effect ever produces — add a `set-flag {{ flag: \"{f}\" }}` effect \
                             earlier, or correct the flag name"
                        ),
                    ));
                }
            }
            for (n, f) in eff.forbids_flags().iter().enumerate() {
                if !declared_flags.contains(f.as_str()) {
                    d.push(Diagnostic::error(
                        codes::FLAG_UNKNOWN,
                        "quests",
                        format!("/content/triggers/{i}/{path}/when/forbids_flags/{n}"),
                        format!(
                            "effect `forbids_flags` references flag `{f}`, which no `set-flag` \
                             effect ever produces — the gate can never suppress anything; add the \
                             producing `set-flag {{ flag: \"{f}\" }}` effect, or correct the flag \
                             name"
                        ),
                    ));
                }
            }
        });
    }
}

/// A trigger's `prop` block id is in the block registry (`DW0193`).
pub(crate) fn trigger_prop_checks(
    c: &Campaign,
    blocks: &dyn BlockRegistry,
    d: &mut Vec<Diagnostic>,
) {
    let quests = &c.quests.content;

    // --- block ids: interact props + set-block effects (quest + trigger) ---
    // A trigger's `prop` (spec-0093 §6.5) is the same object class as an
    // interact's and is held to the same registry.
    for (i, t) in quests.triggers.iter().enumerate() {
        if let Some(prop) = &t.prop {
            check_block_field(
                blocks,
                &prop.block,
                format!("/content/triggers/{i}/prop/block"),
                "triggers[].prop",
                "minecraft:lever[face=floor,facing=north]",
                d,
            );
        }
    }
}

/// Environment trigger declarations (spec-0008 §7): id syntax and uniqueness
/// (`DW0194`), the `at` / `strike-npc` target (`DW0194`, `DW0142`, `DW0112`), an
/// `approach` range, a `use` trigger on an NPC's cell (`DW0350`), the trigger's
/// own flags (`DW0172`) and every effect's references ([`check_effect_v04`]).
pub(crate) fn trigger_decl_checks(
    c: &Campaign,
    anchors: &dyn AnchorRegistry,
    blocks: &dyn BlockRegistry,
    flags: &BTreeSet<&str>,
    d: &mut Vec<Diagnostic>,
) {
    let quests = &c.quests.content;
    let npc_ids: BTreeSet<&str> = c.npcs.content.npcs.iter().map(|n| n.id.as_str()).collect();
    let declared_waves: BTreeSet<&str> = quests.waves.iter().map(|w| w.id.as_str()).collect();

    // area anchor sets (single-prefab areas only) + whether any pool area exists.
    let providers = AnchorProviders::build(c, anchors);

    // --- environment triggers ---
    let mut seen_triggers: BTreeSet<&str> = BTreeSet::new();
    for (i, t) in quests.triggers.iter().enumerate() {
        if !t.id.is_valid_syntax() {
            d.push(Diagnostic::error(
                TRIGGER_INVALID,
                "quests",
                format!("/content/triggers/{i}/id"),
                format!(
                    "malformed trigger id `{}` — trigger ids must be lowercase kebab-case with \
                     the `trigger/` prefix (e.g. `trigger/pressure-plate`)",
                    t.id
                ),
            ));
        }
        if !seen_triggers.insert(t.id.as_str()) {
            d.push(Diagnostic::error(
                TRIGGER_INVALID,
                "quests",
                format!("/content/triggers/{i}/id"),
                format!(
                    "duplicate trigger id `{}` — rename one so every trigger id is unique",
                    t.id
                ),
            ));
        }
        // `at` names a place; `strike-npc` names a character. Exactly one of the
        // two must be supplied, so neither form can be authored half-way (an
        // ignored anchor would read as meaningful and silently do nothing).
        match (t.on.needs_anchor(), t.at_anchor()) {
            (true, None) => d.push(Diagnostic::error(
                TRIGGER_INVALID,
                "quests",
                format!("/content/triggers/{i}/at"),
                format!(
                    "trigger `{}` fires on `{}`, which watches a place, but declares no `at` \
                     anchor — add one ({}), or switch to `strike-npc` if the target is an NPC's \
                     body",
                    t.id,
                    t.on.kind(),
                    providers
                        .anchor_remedy("anchor names come from prefab metadata; do NOT invent one"),
                ),
            )),
            (false, Some(at)) => d.push(Diagnostic::error(
                TRIGGER_INVALID,
                "quests",
                format!("/content/triggers/{i}/at"),
                format!(
                    "trigger `{}` fires on `{}`, whose target is {} — it watches no cell, so \
                     the `at` anchor `{at}` names nothing and would be silently ignored. \
                     Remove `at`.",
                    t.id,
                    t.on.kind(),
                    match (t.on.npc_target(), t.on.assembly_target()) {
                        (Some(n), _) => format!("NPC `{n}`'s body"),
                        (_, Some(m)) => format!("assembly `{m}`'s hitbox"),
                        _ => "an object".to_string(),
                    }
                ),
            )),
            (true, Some(at)) if !providers.resolvable(at) => d.push(Diagnostic::error(
                codes::ANCHOR_UNRESOLVED,
                "quests",
                format!("/content/triggers/{i}/at"),
                format!(
                    "trigger `at` anchor `{at}` is not provided by any area's prefab — {}",
                    providers.anchor_remedy(
                        "set `at` to an anchor some area's prefab exposes (anchor names come from \
                         prefab metadata; do NOT invent one)"
                    ),
                ),
            )),
            _ => {}
        }
        // A `strike-npc` target must be a real stage-2 NPC: the trigger's tag
        // rides that NPC's hitbox, so an unknown id would emit a tag on nothing
        // and the trigger could never fire.
        if let Some(npc) = t.on.npc_target()
            && !c.npcs.content.npcs.iter().any(|n| n.id == *npc)
        {
            d.push(Diagnostic::error(
                codes::DANGLING_REF,
                "quests",
                format!("/content/triggers/{i}/on/npc"),
                format!(
                    "`strike-npc` trigger `{}` targets NPC `{npc}`, which stage 2 does not \
                     declare — use a declared npc id",
                    t.id
                ),
            ));
        }
        if let TriggerOn::Approach { range } = &t.on
            && *range == 0
        {
            d.push(Diagnostic::error(
                TRIGGER_INVALID,
                "quests",
                format!("/content/triggers/{i}/on/range"),
                "`approach` trigger `range` must be > 0 — set a positive block radius (e.g. 3)"
                    .to_string(),
            ));
        }
        if matches!(t.on, TriggerOn::Use)
            && let Some(at) = t.at_anchor()
            && let Some(npc) = c
                .npcs
                .content
                .npcs
                .iter()
                .find(|n| n.anchor.as_str() == at && n.offset == [0, 0, 0])
        {
            d.push(Diagnostic::error(
                USE_TRIGGER_ON_NPC,
                "quests",
                format!("/content/triggers/{i}/at"),
                format!(
                    "`use` trigger `{}` is anchored at `{}`, where NPC `{}` stands — a \
                     right-click there already belongs to the NPC's dialogue, and two \
                     interaction hitboxes in one cell race for the same click (the loser is \
                     silently dead, which can soft-lock the delve). Move the trigger to its \
                     own anchor, or express the interaction as a dialogue option on the NPC. \
                     (To make an NPC's body itself the target, use `strike-npc`.)",
                    t.id, at, npc.id
                ),
            ));
        }
        for (m, f) in t.requires_flags.iter().enumerate() {
            if !flags.contains(f.as_str()) {
                d.push(Diagnostic::error(
                    codes::FLAG_UNKNOWN,
                    "quests",
                    format!("/content/triggers/{i}/requires_flags/{m}"),
                    format!(
                        "trigger `requires_flags` references flag `{f}`, which no `set-flag` \
                         effect ever produces — add a `set-flag {{ flag: \"{f}\" }}` effect \
                         somewhere, or correct the flag name"
                    ),
                ));
            }
        }
        // v0.6: trigger-level `forbids_flags` — same unknown-flag treatment as
        // `requires_flags` (DW0172).
        for (m, f) in t.forbids_flags.iter().enumerate() {
            if !flags.contains(f.as_str()) {
                d.push(Diagnostic::error(
                    codes::FLAG_UNKNOWN,
                    "quests",
                    format!("/content/triggers/{i}/forbids_flags/{m}"),
                    format!(
                        "trigger `forbids_flags` references flag `{f}`, which no `set-flag` \
                         effect ever produces — the gate can never suppress anything; add the \
                         producing `set-flag {{ flag: \"{f}\" }}` effect, or correct the flag name"
                    ),
                ));
            }
        }
        for_each_trigger_effect_deep(t, |path, eff| {
            check_effect_v04(
                eff,
                blocks,
                &declared_waves,
                &format!("/content/triggers/{i}/{path}"),
                &npc_ids,
                d,
            );
        });
    }
}
