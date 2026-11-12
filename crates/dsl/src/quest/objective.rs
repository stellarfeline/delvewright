//! A quest objective: what the party does to advance a quest.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::serde_fields::{is_false, is_zero};
use crate::{
    AnchorId, FlagId, Guidance, Happening, Highlight, NpcId, ObjectiveId, Prop, QuestEffect,
    StateCompare, VehicleId, Visibility, WaveId,
};

/// The fewest ticks a `limit` may run (one second).
pub const LIMIT_MIN_TICKS: u32 = 20;
/// The most ticks a `limit` may run (the ten-hour ceiling of a delve).
pub const LIMIT_MAX_TICKS: u32 = 720_000;

/// **An objective's window** (spec-0133 §2): how long it stays open, the event
/// its clock starts at, and what its expiry does.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Limit {
    /// The window in game ticks, `20..=720000` (one second to the ten-hour
    /// ceiling of a delve). The player sees minutes and seconds.
    #[schemars(range(min = 20, max = 720000))]
    pub ticks: u32,
    /// The event the clock starts at. Absent = [`LimitFrom::Activation`].
    #[serde(default, skip_serializing_if = "LimitFrom::is_activation")]
    pub from: LimitFrom,
    /// What expiry fires, as the party (`Audience::Party`): an effect root of
    /// its own. Optional; when present, at least one effect (`DW0100`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1))]
    pub on_expire: Option<Vec<QuestEffect>>,
}

impl Limit {
    /// The `on_expire` bundle, empty when none is declared.
    pub fn on_expire(&self) -> &[QuestEffect] {
        self.on_expire.as_deref().unwrap_or(&[])
    }
}

/// **The event a limit's clock starts at** (spec-0133 §2).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum LimitFrom {
    /// The tick the objective's told guard first holds — it activates.
    #[default]
    Activation,
    /// The tick the objective's quest activates.
    QuestStart,
    /// The activation of an earlier objective of the same quest, one of the
    /// limited objective's `after`-ancestors (`DW1086`).
    Objective(ObjectiveId),
}

impl LimitFrom {
    /// Whether this is the default start.
    pub fn is_activation(&self) -> bool {
        matches!(self, LimitFrom::Activation)
    }

    /// The token a reader names the start by: `activation`, `quest-start`, or
    /// the named objective's id.
    pub fn token(&self) -> String {
        match self {
            LimitFrom::Activation => "activation".to_string(),
            LimitFrom::QuestStart => "quest-start".to_string(),
            LimitFrom::Objective(o) => o.as_str().to_string(),
        }
    }
}

/// A quest objective.
///
/// Every variant may carry `requires_flags`:
/// flag-gated activation, satisfied only once each referenced flag has been set
/// by a `set-flag` effect.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Objective {
    /// Completed by a dialogue option's `complete-objective` effect.
    TalkTo {
        /// Objective id.
        id: ObjectiveId,
        /// Short player-facing objective name (v0.3, optional).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        /// One-line location/direction hint (v0.3, optional).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hint: Option<String>,
        /// **What the task was** (spec-0137 §2.1): one line, read on the
        /// journal's finished page once the objective is over — beside its
        /// title and the way it ended (`record-outcome`). Needs no `title` and
        /// no announcement: an objective that carries it, or that a
        /// `record-outcome` names, is recorded, and a recorded objective has a
        /// finished page whether or not it was announced.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        record: Option<String>,
        /// Whether this objective is **announced** — the `New objective` line, the
        /// hint's line, the cue sound and the `Objective complete` line (spec-0093).
        /// Absent = the campaign's [`Guidance::announcements`]. An objective with
        /// no `title` is never announced whatever this says; `shown` on one is
        /// `DW0961`. A hint on an unannounced objective is `DW0862`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        announcement: Option<Visibility>,
        /// Whether this objective is one the compass can follow (spec-0104 §6).
        /// Absent = the campaign's [`Guidance::compass`]. Only an announced
        /// objective is followed; `shown` on one that is not is `DW1017`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        compass: Option<Visibility>,
        /// The proximity highlight (spec-0104 §7): this objective's block or
        /// body glows while a player is within `within` blocks and it is
        /// pending. Absent = none.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        highlight: Option<Highlight>,
        /// The NPC to talk to.
        npc: NpcId,
        /// Prerequisite objectives (intra-quest ordering): ids, all of which
        /// must settle, and quorums (spec-0134 §7), each of which holds when
        /// `at_least` of its members are done.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        after: Vec<AfterTerm>,
        /// Flags that must be set before this objective activates (v0.3).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_flags: Vec<FlagId>,
        /// Negative flag gate (DSL v0.6): the
        /// objective is suppressed (cannot activate or complete) while ANY listed
        /// flag is set for the player — the dual of `requires_flags`.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        forbids_flags: Vec<FlagId>,
        /// Numeric gate terms (DSL v0.10, spec-0031): every listed comparison
        /// must hold for this gate to be open. The third field of the one gate,
        /// carried by every gate consumer — never by the verb that first wanted
        /// it. Default empty: no numeric term.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_state: Vec<StateCompare>,
        /// Bot stealth hint (DSL v0.4): mark this leg as one the critical-path
        /// bot should traverse sneaking (sprint disabled). Emitted into
        /// `critical-path.json` as `sneak: true` on the step. Purely a harness
        /// hint; no datapack effect.
        #[serde(default, skip_serializing_if = "is_false")]
        stealth: bool,
        /// What this objective does to the story (DSL v0.8, spec-0025; required
        /// at 0.8.0, `DW0481`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        happening: Option<Happening>,
        /// **The window this objective is open for** (spec-0133): absent, it
        /// stays pending until its act; present, it settles **expired** when
        /// the clock runs out first — `on_expire` fires instead of the
        /// completion bundle, and its `after`-dependents and its quest go on.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<Limit>,
        /// **The leg to this objective is ridden** (spec-0135 §4): the party
        /// walks to the named vehicle, boards it at its mark, rides it through
        /// its medium as near this objective as the medium reaches, leaves and
        /// walks on. Absent, the leg is walked, and a vehicle carries it only
        /// where no walk crosses. A leg the vehicle cannot carry is `DW0311`;
        /// a vehicle nobody declared is `DW0112`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ride: Option<VehicleId>,
    },
    /// Completed by reaching an anchor once prerequisites are met.
    ReachAnchor {
        /// Objective id.
        id: ObjectiveId,
        /// Short player-facing objective name (v0.3, optional).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        /// One-line location/direction hint (v0.3, optional).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hint: Option<String>,
        /// **What the task was** (spec-0137 §2.1): one line, read on the
        /// journal's finished page once the objective is over — beside its
        /// title and the way it ended (`record-outcome`). Needs no `title` and
        /// no announcement: an objective that carries it, or that a
        /// `record-outcome` names, is recorded, and a recorded objective has a
        /// finished page whether or not it was announced.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        record: Option<String>,
        /// Whether this objective is **announced** — the `New objective` line, the
        /// hint's line, the cue sound and the `Objective complete` line (spec-0093).
        /// Absent = the campaign's [`Guidance::announcements`]. An objective with
        /// no `title` is never announced whatever this says; `shown` on one is
        /// `DW0961`. A hint on an unannounced objective is `DW0862`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        announcement: Option<Visibility>,
        /// Whether this objective is one the compass can follow (spec-0104 §6).
        /// Absent = the campaign's [`Guidance::compass`]. Only an announced
        /// objective is followed; `shown` on one that is not is `DW1017`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        compass: Option<Visibility>,
        /// The anchor to reach.
        anchor: AnchorId,
        /// Completion radius (blocks).
        radius: u32,
        /// Prerequisite objectives (intra-quest ordering): ids, all of which
        /// must settle, and quorums (spec-0134 §7), each of which holds when
        /// `at_least` of its members are done.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        after: Vec<AfterTerm>,
        /// Flags that must be set before this objective activates (v0.3).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_flags: Vec<FlagId>,
        /// Negative flag gate (DSL v0.6): the
        /// objective is suppressed (cannot activate or complete) while ANY listed
        /// flag is set for the player — the dual of `requires_flags`.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        forbids_flags: Vec<FlagId>,
        /// Numeric gate terms (DSL v0.10, spec-0031): every listed comparison
        /// must hold for this gate to be open. The third field of the one gate,
        /// carried by every gate consumer — never by the verb that first wanted
        /// it. Default empty: no numeric term.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_state: Vec<StateCompare>,
        /// Bot stealth hint (DSL v0.4): mark this leg as one the critical-path
        /// bot should traverse sneaking (sprint disabled). Emitted into
        /// `critical-path.json` as `sneak: true` on the step. Purely a harness
        /// hint; no datapack effect.
        #[serde(default, skip_serializing_if = "is_false")]
        stealth: bool,
        /// What this objective does to the story (DSL v0.8, spec-0025; required
        /// at 0.8.0, `DW0481`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        happening: Option<Happening>,
        /// **The window this objective is open for** (spec-0133): absent, it
        /// stays pending until its act; present, it settles **expired** when
        /// the clock runs out first — `on_expire` fires instead of the
        /// completion bundle, and its `after`-dependents and its quest go on.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<Limit>,
        /// **The leg to this objective is ridden** (spec-0135 §4): the party
        /// walks to the named vehicle, boards it at its mark, rides it through
        /// its medium as near this objective as the medium reaches, leaves and
        /// walks on. Absent, the leg is walked, and a vehicle carries it only
        /// where no walk crosses. A leg the vehicle cannot carry is `DW0311`;
        /// a vehicle nobody declared is `DW0112`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ride: Option<VehicleId>,
    },
    /// Completed when the referenced wave is fully slain (v0.3).
    Kill {
        /// Objective id.
        id: ObjectiveId,
        /// Short player-facing objective name (v0.3, optional).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        /// One-line location/direction hint (v0.3, optional).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hint: Option<String>,
        /// **What the task was** (spec-0137 §2.1): one line, read on the
        /// journal's finished page once the objective is over — beside its
        /// title and the way it ended (`record-outcome`). Needs no `title` and
        /// no announcement: an objective that carries it, or that a
        /// `record-outcome` names, is recorded, and a recorded objective has a
        /// finished page whether or not it was announced.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        record: Option<String>,
        /// Whether this objective is **announced** — the `New objective` line, the
        /// hint's line, the cue sound and the `Objective complete` line (spec-0093).
        /// Absent = the campaign's [`Guidance::announcements`]. An objective with
        /// no `title` is never announced whatever this says; `shown` on one is
        /// `DW0961`. A hint on an unannounced objective is `DW0862`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        announcement: Option<Visibility>,
        /// Whether this objective is one the compass can follow (spec-0104 §6).
        /// Absent = the campaign's [`Guidance::compass`]. Only an announced
        /// objective is followed; `shown` on one that is not is `DW1017`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        compass: Option<Visibility>,
        /// The proximity highlight (spec-0104 §7): this objective's block or
        /// body glows while a player is within `within` blocks and it is
        /// pending. Absent = none.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        highlight: Option<Highlight>,
        /// The wave (stage-5 `waves` ref) whose mobs must be slain.
        wave: WaveId,
        /// Prerequisite objectives.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        after: Vec<AfterTerm>,
        /// Flags that must be set before this objective activates (v0.3).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_flags: Vec<FlagId>,
        /// Negative flag gate (DSL v0.6): the
        /// objective is suppressed (cannot activate or complete) while ANY listed
        /// flag is set for the player — the dual of `requires_flags`.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        forbids_flags: Vec<FlagId>,
        /// Numeric gate terms (DSL v0.10, spec-0031): every listed comparison
        /// must hold for this gate to be open. The third field of the one gate,
        /// carried by every gate consumer — never by the verb that first wanted
        /// it. Default empty: no numeric term.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_state: Vec<StateCompare>,
        /// Bot stealth hint (DSL v0.4): mark this leg as one the critical-path
        /// bot should traverse sneaking (sprint disabled). Emitted into
        /// `critical-path.json` as `sneak: true` on the step. Purely a harness
        /// hint; no datapack effect.
        #[serde(default, skip_serializing_if = "is_false")]
        stealth: bool,
        /// What this objective does to the story (DSL v0.8, spec-0025; required
        /// at 0.8.0, `DW0481`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        happening: Option<Happening>,
        /// **The window this objective is open for** (spec-0133): absent, it
        /// stays pending until its act; present, it settles **expired** when
        /// the clock runs out first — `on_expire` fires instead of the
        /// completion bundle, and its `after`-dependents and its quest go on.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<Limit>,
        /// **The leg to this objective is ridden** (spec-0135 §4): the party
        /// walks to the named vehicle, boards it at its mark, rides it through
        /// its medium as near this objective as the medium reaches, leaves and
        /// walks on. Absent, the leg is walked, and a vehicle carries it only
        /// where no walk crosses. A leg the vehicle cannot carry is `DW0311`;
        /// a vehicle nobody declared is `DW0112`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ride: Option<VehicleId>,
    },
    /// Completed when `count` of `item` have been collected (v0.3).
    ///
    /// The items are provided in a holder: the compiler's own chest at
    /// `anchor` by default, or the prefab's existing holder — a container of
    /// any of the twelve kinds (spec-0105), an item frame or an armour stand
    /// (spec-0111) — at [`Objective::Collect::holder`], optionally carrying an
    /// [`Objective::Collect::item_name`] and padded to read full with
    /// [`Objective::Collect::fill_count`].
    Collect {
        /// Objective id.
        id: ObjectiveId,
        /// Short player-facing objective name (v0.3, optional).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        /// One-line location/direction hint (v0.3, optional).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hint: Option<String>,
        /// **What the task was** (spec-0137 §2.1): one line, read on the
        /// journal's finished page once the objective is over — beside its
        /// title and the way it ended (`record-outcome`). Needs no `title` and
        /// no announcement: an objective that carries it, or that a
        /// `record-outcome` names, is recorded, and a recorded objective has a
        /// finished page whether or not it was announced.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        record: Option<String>,
        /// Whether this objective is **announced** — the `New objective` line, the
        /// hint's line, the cue sound and the `Objective complete` line (spec-0093).
        /// Absent = the campaign's [`Guidance::announcements`]. An objective with
        /// no `title` is never announced whatever this says; `shown` on one is
        /// `DW0961`. A hint on an unannounced objective is `DW0862`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        announcement: Option<Visibility>,
        /// Whether this objective is one the compass can follow (spec-0104 §6).
        /// Absent = the campaign's [`Guidance::compass`]. Only an announced
        /// objective is followed; `shown` on one that is not is `DW1017`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        compass: Option<Visibility>,
        /// The proximity highlight (spec-0104 §7): this objective's block or
        /// body glows while a player is within `within` blocks and it is
        /// pending. Absent = none.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        highlight: Option<Highlight>,
        /// Vanilla item id to collect (validated against the registry).
        item: String,
        /// How many are required.
        count: u32,
        /// The anchor items are provided at (chest / pickup).
        anchor: AnchorId,
        /// **Adopt the holder the prefab already placed** (DSL v0.49, spec-0111):
        /// the anchor whose assembled-world cell holds a container (any of the
        /// twelve kinds, spec-0105), an item frame or an armour stand — the
        /// holder this collect fills instead of conjuring its own chest at
        /// [`Objective::Collect::anchor`].
        ///
        /// Same division of labour a `loot` entry and a trap's dispenser already
        /// keep with the prefab: furniture belongs in the piece. The holder's
        /// kind, its flags and its tag are the engine's derivations from the
        /// piece; nothing about them is typed here. Where in the holder the
        /// objective's item lands is [`Objective::Collect::slot`]. A `holder`
        /// whose cell holds no holder is a build error (`DW0438`), never a
        /// silent fill into a wall; padding (`fill_count`) on a frame or a
        /// stand is `DW1035`, because neither has a `container.<n>` slot to pad.
        ///
        /// The critical-path step's position follows the holder, and no chest
        /// is placed at `anchor` when it is set.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        holder: Option<AnchorId>,
        /// **The slot of the adopted holder the objective's item lands in**
        /// (DSL v0.49, spec-0111 §3.4), in the holder class's one vocabulary —
        /// the same words, and the same validation, as a `loot` stack's
        /// [`crate::LootItem::slot`]: `container.<n>` on a container (padding
        /// follows in `container.<n+1>`, …), `contents` on an item frame,
        /// `armor.head`, `armor.chest`, `armor.legs`, `armor.feet`,
        /// `weapon.mainhand` or `weapon.offhand` on an armour stand. Omitted, the
        /// item lands positionally — `container.0`, a frame's `contents` — and
        /// on a stand it is `DW1035`, because which hand or head a thing sits in
        /// is the author's judgement. Only a collect with a `holder` has a slot
        /// to name (`DW1035` otherwise); a slot the holder at the anchor lacks
        /// is `DW0438`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        slot: Option<String>,
        /// **The item comes off a body, not out of a box**: the wave whose
        /// declared `drops[]` yield
        /// this objective's item. No container is placed — not the compiler's own
        /// chest at `anchor`, not a prefab one — and `holder` is therefore
        /// mutually exclusive with it (`DW0100`-adjacent; `DW0492`).
        ///
        /// This is what makes "kill the boss → pick up its key → open the door"
        /// a *proved* chain rather than an authoring intention. The compiler
        /// requires (a) that the named wave really declares an `{item}` drop of
        /// this item (`DW0492`), and (b) that a `kill` objective for that wave
        /// precedes this collect in the objective graph (`DW0493`). The existing
        /// flow machinery then carries the ordering the rest of the way: the
        /// door's `requires_flags` hangs off this collect exactly as it would off
        /// a chest one.
        ///
        /// **Waves only.** An actor's death is not observable by any objective —
        /// there is no vanilla-side signal the flow machinery could consume — so
        /// an actor-gated collect would be an unprovable claim, and per the
        /// no-hack doctrine it is excluded rather than approximated. An actor may
        /// still declare `drops[]`; those drops just cannot gate a quest.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dropped_by: Option<WaveId>,
        /// Display name for the collected item (DSL v0.8), emitted as the vanilla `custom_name` item component.
        ///
        /// A quest item is a *named thing* in the story ("Cheese", "Tide
        /// Ledger"), and a player who opens the barrel must read that name — an
        /// unnamed `minecraft:pumpkin_pie` says nothing about what the quest asked
        /// for. Player-visible, so it enters the l10n string inventory
        /// (`obj.<quest>.<obj>.item_name`) and translates like any other line.
        ///
        /// Naming changes nothing about adjudication: the completion advancement
        /// and the per-tick held check both match on the ITEM ID, which a named
        /// stack still carries.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        item_name: Option<String>,
        /// Padding stacks that make the container **read full**. Default `0` =
        /// the single required
        /// stack and nothing else.
        ///
        /// A barrel of cheese that opens on one lonely wheel reads as a bug, and
        /// vanilla's notion of "full" is *occupied slots*, not stack size — so
        /// this counts SLOTS: the objective's own stack lands in `container.0` (or
        /// the `container.<n>` [`Objective::Collect::slot`] names) and each
        /// padding stack repeats it in the slots after it. Slot
        /// assignment is positional and total, the same determinism story `loot`
        /// tells (ADR-0006): no RNG, no loot tables, nothing to reseed.
        ///
        /// The padding is the same item, so taking the whole barrel still
        /// completes the objective and never over- or under-counts it.
        #[serde(default, skip_serializing_if = "is_zero")]
        fill_count: u32,
        /// Prerequisite objectives.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        after: Vec<AfterTerm>,
        /// Flags that must be set before this objective activates (v0.3).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_flags: Vec<FlagId>,
        /// Negative flag gate (DSL v0.6): the
        /// objective is suppressed (cannot activate or complete) while ANY listed
        /// flag is set for the player — the dual of `requires_flags`.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        forbids_flags: Vec<FlagId>,
        /// Numeric gate terms (DSL v0.10, spec-0031): every listed comparison
        /// must hold for this gate to be open. The third field of the one gate,
        /// carried by every gate consumer — never by the verb that first wanted
        /// it. Default empty: no numeric term.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_state: Vec<StateCompare>,
        /// Bot stealth hint (DSL v0.4): mark this leg as one the critical-path
        /// bot should traverse sneaking (sprint disabled). Emitted into
        /// `critical-path.json` as `sneak: true` on the step. Purely a harness
        /// hint; no datapack effect.
        #[serde(default, skip_serializing_if = "is_false")]
        stealth: bool,
        /// What this objective does to the story (DSL v0.8, spec-0025; required
        /// at 0.8.0, `DW0481`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        happening: Option<Happening>,
        /// **The window this objective is open for** (spec-0133): absent, it
        /// stays pending until its act; present, it settles **expired** when
        /// the clock runs out first — `on_expire` fires instead of the
        /// completion bundle, and its `after`-dependents and its quest go on.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<Limit>,
        /// **The leg to this objective is ridden** (spec-0135 §4): the party
        /// walks to the named vehicle, boards it at its mark, rides it through
        /// its medium as near this objective as the medium reaches, leaves and
        /// walks on. Absent, the leg is walked, and a vehicle carries it only
        /// where no walk crosses. A leg the vehicle cannot carry is `DW0311`;
        /// a vehicle nobody declared is `DW0112`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ride: Option<VehicleId>,
    },
    /// Completed by interacting with an entity at `anchor`; if `requires_item` is
    /// set, the item must be **held in the main hand** (v0.3; held semantics since
    /// DSL v0.7 — see [`Objective::Interact::requires_item`]).
    Interact {
        /// Objective id.
        id: ObjectiveId,
        /// Short player-facing objective name (v0.3, optional).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        /// One-line location/direction hint (v0.3, optional).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hint: Option<String>,
        /// **What the task was** (spec-0137 §2.1): one line, read on the
        /// journal's finished page once the objective is over — beside its
        /// title and the way it ended (`record-outcome`). Needs no `title` and
        /// no announcement: an objective that carries it, or that a
        /// `record-outcome` names, is recorded, and a recorded objective has a
        /// finished page whether or not it was announced.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        record: Option<String>,
        /// Whether this objective is **announced** — the `New objective` line, the
        /// hint's line, the cue sound and the `Objective complete` line (spec-0093).
        /// Absent = the campaign's [`Guidance::announcements`]. An objective with
        /// no `title` is never announced whatever this says; `shown` on one is
        /// `DW0961`. A hint on an unannounced objective is `DW0862`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        announcement: Option<Visibility>,
        /// Whether this objective is one the compass can follow (spec-0104 §6).
        /// Absent = the campaign's [`Guidance::compass`]. Only an announced
        /// objective is followed; `shown` on one that is not is `DW1017`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        compass: Option<Visibility>,
        /// The proximity highlight (spec-0104 §7): this objective's block or
        /// body glows while a player is within `within` blocks and it is
        /// pending. Absent = none.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        highlight: Option<Highlight>,
        /// The anchor the interaction entity stands at.
        anchor: AnchorId,
        /// Item the player must be **holding in the main hand** for the
        /// interaction to complete (optional).
        ///
        /// Held, not merely possessed: presenting the
        /// item IS the action — a player who right-clicks a sleeping giant with a
        /// sharpened stake buried in their backpack has not stabbed anything.
        /// Before this ruling the gate read the whole inventory, which made every
        /// `requires_item` interaction fire the moment the item was picked up
        /// anywhere, whatever the player was actually doing with their hands.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        requires_item: Option<String>,
        /// **The presented item leaves the hand** (spec-0114 §6.3): one of
        /// [`Objective::Interact::requires_item`] is taken from the player who
        /// presented it, at the moment the objective completes. Default `false`,
        /// absent when false. Requires `requires_item` (`DW1046`).
        ///
        /// What makes *lay*, *hang* and *give* true of a click: the thing is laid
        /// at the place and is no longer carried. The world showing it there
        /// afterwards is the author's, composed from what exists (a `set-block` in
        /// `on_objective_complete`), never a second mechanism here.
        #[serde(default, skip_serializing_if = "is_false")]
        takes_item: bool,
        /// **The real block the player presses** (spec-0117 §3a): placed at the
        /// anchor from `setup_finish` to the end of the delve. A bell or a button
        /// is pressed on its own; any other block with an outline — a door, a
        /// lever, a chest, a campfire — is pressed through a click box fitted to
        /// it, so it does not act before its step. Required (`DW1059`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(required)]
        prop: Option<Prop>,
        /// **What the press answers outside its window** (spec-0117 §4):
        /// `not_yet` before the objective is pending, `done` after it completes
        /// (unless `release`), and with `requires_item` `missing_item` for a
        /// press inside the window without the item in hand. Each owed line is
        /// `DW1051`, each inert one `DW1052`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        answers: Option<crate::Answers>,
        /// **The block is vanilla's once the objective completes** (spec-0117
        /// §3a): a boxed block that acts on its own — a door the party unbarred
        /// — is not boxed again and answers nothing thereafter. Default
        /// `false`; inert on a bell or a button (`DW1052`), silent on a block the
        /// game reports no press of (`DW1051`).
        #[serde(default, skip_serializing_if = "is_false")]
        release: bool,
        /// Prerequisite objectives.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        after: Vec<AfterTerm>,
        /// Flags that must be set before this objective activates (v0.3).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_flags: Vec<FlagId>,
        /// Negative flag gate (DSL v0.6): the
        /// objective is suppressed (cannot activate or complete) while ANY listed
        /// flag is set for the player — the dual of `requires_flags`.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        forbids_flags: Vec<FlagId>,
        /// Numeric gate terms (DSL v0.10, spec-0031): every listed comparison
        /// must hold for this gate to be open. The third field of the one gate,
        /// carried by every gate consumer — never by the verb that first wanted
        /// it. Default empty: no numeric term.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        requires_state: Vec<StateCompare>,
        /// Bot stealth hint (DSL v0.4): mark this leg as one the critical-path
        /// bot should traverse sneaking (sprint disabled). Emitted into
        /// `critical-path.json` as `sneak: true` on the step. Purely a harness
        /// hint; no datapack effect.
        #[serde(default, skip_serializing_if = "is_false")]
        stealth: bool,
        /// What this objective does to the story (DSL v0.8, spec-0025; required
        /// at 0.8.0, `DW0481`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        happening: Option<Happening>,
        /// **The window this objective is open for** (spec-0133): absent, it
        /// stays pending until its act; present, it settles **expired** when
        /// the clock runs out first — `on_expire` fires instead of the
        /// completion bundle, and its `after`-dependents and its quest go on.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<Limit>,
        /// **The leg to this objective is ridden** (spec-0135 §4): the party
        /// walks to the named vehicle, boards it at its mark, rides it through
        /// its medium as near this objective as the medium reaches, leaves and
        /// walks on. Absent, the leg is walked, and a vehicle carries it only
        /// where no walk crosses. A leg the vehicle cannot carry is `DW0311`;
        /// a vehicle nobody declared is `DW0112`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ride: Option<VehicleId>,
    },
}

/// **An act a player performs**, as the pinned game adjudicates it (spec-0114
/// §4): the closed vocabulary an objective's completion is read in, and the one
/// its title's head word is read against (`DW1045`).
///
/// Declared in the order of the spec's table; the order is the one every
/// printed act set is written in. A planned quest declares its line's
/// activity as a set of these (`PlannedQuest::activity`, spec-0131), spelled
/// in kebab-case as [`Act::as_str`] spells them.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Act {
    /// A body inside a volume at a place — the one **place** act.
    Reach,
    /// A dialogue option chosen on a body.
    Talk,
    /// A wave's counter at zero.
    Kill,
    /// The item in the player's inventory.
    Take,
    /// A thing at a place used: the hitbox, or the hand-pressed block.
    Use,
    /// The item held in the main hand at the thing when it is used.
    Present,
    /// One of the presented item leaves the player at the thing.
    Give,
}

impl Act {
    /// Every act, in declaration order.
    pub const ALL: [Act; 7] = [
        Act::Reach,
        Act::Talk,
        Act::Kill,
        Act::Take,
        Act::Use,
        Act::Present,
        Act::Give,
    ];

    /// The act's name as the lexicon and every message spell it.
    pub const fn as_str(self) -> &'static str {
        match self {
            Act::Reach => "reach",
            Act::Talk => "talk",
            Act::Kill => "kill",
            Act::Take => "take",
            Act::Use => "use",
            Act::Present => "present",
            Act::Give => "give",
        }
    }

    /// The act a lexicon key names, if it names one.
    pub fn parse(s: &str) -> Option<Act> {
        Act::ALL.into_iter().find(|a| a.as_str() == s)
    }
}

/// An act set written as the messages and the translator's situation write it:
/// `use, present, give`.
pub fn acts_line(acts: &std::collections::BTreeSet<Act>) -> String {
    acts.iter()
        .map(|a| a.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// One term of an objective's `after` (spec-0134 §7): an objective that must
/// settle, or a quorum of several.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum AfterTerm {
    /// An objective (same quest) that must settle first.
    Objective(ObjectiveId),
    /// Any `at_least` of several objectives, done.
    Quorum(Quorum),
}

/// **A quorum** (spec-0134 §7): it holds when `at_least` of its members are
/// done. When it first holds, every member not yet done is **lapsed** —
/// settled, its completion bundle never fired, its act refused after. The
/// schema bounds `at_least` to `1..=|any| − 1` and refuses a repeated member
/// (`DW0100`): a quorum of every member is the plain list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Quorum {
    /// The members (same quest), at least two.
    #[schemars(length(min = 2))]
    pub any: Vec<ObjectiveId>,
    /// How many of them must be done, at least one and fewer than all.
    #[schemars(range(min = 1))]
    pub at_least: u32,
}

impl Quorum {
    /// Whether at least `at_least` members are `done`.
    pub fn holds(&self, done: &dyn Fn(&str) -> bool) -> bool {
        self.any.iter().filter(|m| done(m.as_str())).count() >= self.at_least as usize
    }

    /// The members a world that completes `chosen` lapses: every member not
    /// in it.
    pub fn lapsing<'a>(&'a self, chosen: &'a [&'a str]) -> impl Iterator<Item = &'a str> + 'a {
        self.any
            .iter()
            .map(ObjectiveId::as_str)
            .filter(move |m| !chosen.contains(m))
    }

    /// The members the default world completes: the first `at_least`, by id
    /// order. The rest lapse.
    pub fn default_completers(&self) -> Vec<&str> {
        let mut ids: Vec<&str> = self.any.iter().map(ObjectiveId::as_str).collect();
        ids.sort_unstable();
        ids.truncate(self.at_least as usize);
        ids
    }
}

impl Objective {
    /// **The acts this objective's completion performs** (spec-0114 §4),
    /// derived from its kind and the fields that shape what it completes on,
    /// with `completion_effects` — the effects its quest fires when it
    /// completes (`on_objective_complete[<id>]`) — read for a `give-item`,
    /// which adds [`Act::Take`] to any kind. Nested effect lists (a `sequence`'s
    /// steps) are read too: an item handed later in the same bundle is still
    /// handed by this completion.
    ///
    /// `reach-anchor` → `{reach}`; `talk-to` → `{talk}`; `kill` → `{kill}`;
    /// `collect` → `{take}`; `interact` → `{use}`, `+ present` with
    /// `requires_item`, `+ give` with `takes_item` as well.
    pub fn completion_acts(
        &self,
        completion_effects: &[crate::QuestEffect],
    ) -> std::collections::BTreeSet<Act> {
        let mut acts = std::collections::BTreeSet::new();
        match self {
            Objective::ReachAnchor { .. } => {
                acts.insert(Act::Reach);
            }
            Objective::TalkTo { .. } => {
                acts.insert(Act::Talk);
            }
            Objective::Kill { .. } => {
                acts.insert(Act::Kill);
            }
            Objective::Collect { .. } => {
                acts.insert(Act::Take);
            }
            Objective::Interact {
                requires_item,
                takes_item,
                ..
            } => {
                acts.insert(Act::Use);
                if requires_item.is_some() {
                    acts.insert(Act::Present);
                    if *takes_item {
                        acts.insert(Act::Give);
                    }
                }
            }
        }
        fn hands(effs: &[crate::QuestEffect]) -> bool {
            effs.iter().any(|e| {
                e.give_item().is_some()
                    || e.nested_effect_lists_labeled()
                        .into_iter()
                        .any(|(_, _, list)| hands(list))
            })
        }
        if hands(completion_effects) {
            acts.insert(Act::Take);
        }
        acts
    }

    /// The kind and the fields the completion's acts are derived from, as
    /// `DW1045` names them: `reach-anchor`, `interact + requires_item`,
    /// `talk-to + give-item`, ….
    pub fn completion_shape(&self, completion_effects: &[crate::QuestEffect]) -> String {
        let mut s = self.kind().to_string();
        if let Objective::Interact {
            requires_item,
            takes_item,
            ..
        } = self
            && requires_item.is_some()
        {
            s.push_str(" + requires_item");
            if *takes_item {
                s.push_str(" + takes_item");
            }
        }
        if self
            .completion_acts(completion_effects)
            .contains(&Act::Take)
            && !matches!(self, Objective::Collect { .. })
        {
            s.push_str(" + give-item");
        }
        s
    }

    /// Whether this is an `interact` whose presented item leaves the hand
    /// (spec-0114 §6.3), with the item it takes.
    pub fn taken_item(&self) -> Option<&str> {
        match self {
            Objective::Interact {
                requires_item: Some(it),
                takes_item: true,
                ..
            } => Some(it),
            _ => None,
        }
    }

    /// This objective's id.
    pub fn id(&self) -> &ObjectiveId {
        match self {
            Objective::TalkTo { id, .. }
            | Objective::ReachAnchor { id, .. }
            | Objective::Kill { id, .. }
            | Objective::Collect { id, .. }
            | Objective::Interact { id, .. } => id,
        }
    }

    /// This objective's prerequisite terms, as declared.
    pub fn after_terms(&self) -> &[AfterTerm] {
        match self {
            Objective::TalkTo { after, .. }
            | Objective::ReachAnchor { after, .. }
            | Objective::Kill { after, .. }
            | Objective::Collect { after, .. }
            | Objective::Interact { after, .. } => after,
        }
    }

    /// **Every objective this one's `after` names** — its plain prerequisites
    /// and every member of its quorums — in declaration order: the edges of the
    /// intra-quest order. Ordering, cycles (`DW0140`) and "may come before"
    /// read these; whether the objective may open reads [`Self::after_holds`].
    pub fn after_edges(&self) -> Vec<&ObjectiveId> {
        let mut out = Vec::new();
        for t in self.after_terms() {
            match t {
                AfterTerm::Objective(id) => out.push(id),
                AfterTerm::Quorum(q) => out.extend(q.any.iter()),
            }
        }
        out
    }

    /// The prerequisites that must settle before this objective opens, every
    /// one of them: the plain ids, never a quorum's members. "Is certainly
    /// done before" reads these.
    pub fn after_required(&self) -> Vec<&ObjectiveId> {
        self.after_terms()
            .iter()
            .filter_map(|t| match t {
                AfterTerm::Objective(id) => Some(id),
                AfterTerm::Quorum(_) => None,
            })
            .collect()
    }

    /// This objective's quorums (spec-0134 §7), in declaration order.
    pub fn after_quorums(&self) -> Vec<&Quorum> {
        self.after_terms()
            .iter()
            .filter_map(|t| match t {
                AfterTerm::Quorum(q) => Some(q),
                AfterTerm::Objective(_) => None,
            })
            .collect()
    }

    /// **Whether this objective's `after` holds** — the one definition every
    /// reader of the gate answers through (spec-0134 §7): every plain
    /// prerequisite `settled` (done, expired or lapsed), and every quorum with
    /// at least `at_least` of its members `done` (completed by their act).
    pub fn after_holds(&self, settled: &dyn Fn(&str) -> bool, done: &dyn Fn(&str) -> bool) -> bool {
        self.after_terms().iter().all(|t| match t {
            AfterTerm::Objective(id) => settled(id.as_str()),
            AfterTerm::Quorum(q) => q.holds(done),
        })
    }

    /// The short player-facing objective title (v0.3, optional).
    pub fn title(&self) -> Option<&str> {
        match self {
            Objective::TalkTo { title, .. }
            | Objective::ReachAnchor { title, .. }
            | Objective::Kill { title, .. }
            | Objective::Collect { title, .. }
            | Objective::Interact { title, .. } => title.as_deref(),
        }
    }

    /// The one-line location/direction hint (v0.3, optional).
    pub fn hint(&self) -> Option<&str> {
        match self {
            Objective::TalkTo { hint, .. }
            | Objective::ReachAnchor { hint, .. }
            | Objective::Kill { hint, .. }
            | Objective::Collect { hint, .. }
            | Objective::Interact { hint, .. } => hint.as_deref(),
        }
    }

    /// **What the task was** (spec-0137 §2.1), when the objective states it.
    pub fn record(&self) -> Option<&str> {
        match self {
            Objective::TalkTo { record, .. }
            | Objective::ReachAnchor { record, .. }
            | Objective::Kill { record, .. }
            | Objective::Collect { record, .. }
            | Objective::Interact { record, .. } => record.as_deref(),
        }
    }

    /// Mutable access to the optional `record` (i18n localization).
    pub fn record_mut(&mut self) -> &mut Option<String> {
        match self {
            Objective::TalkTo { record, .. }
            | Objective::ReachAnchor { record, .. }
            | Objective::Kill { record, .. }
            | Objective::Collect { record, .. }
            | Objective::Interact { record, .. } => record,
        }
    }

    /// Mutable access to the optional player-facing title (i18n localization).
    pub fn title_mut(&mut self) -> &mut Option<String> {
        match self {
            Objective::TalkTo { title, .. }
            | Objective::ReachAnchor { title, .. }
            | Objective::Kill { title, .. }
            | Objective::Collect { title, .. }
            | Objective::Interact { title, .. } => title,
        }
    }

    /// Mutable access to the optional one-line hint (i18n localization).
    pub fn hint_mut(&mut self) -> &mut Option<String> {
        match self {
            Objective::TalkTo { hint, .. }
            | Objective::ReachAnchor { hint, .. }
            | Objective::Kill { hint, .. }
            | Objective::Collect { hint, .. }
            | Objective::Interact { hint, .. } => hint,
        }
    }

    /// The objective's own `announcement`, when it states one (spec-0093).
    pub fn announcement(&self) -> Option<Visibility> {
        match self {
            Objective::TalkTo { announcement, .. }
            | Objective::ReachAnchor { announcement, .. }
            | Objective::Kill { announcement, .. }
            | Objective::Collect { announcement, .. }
            | Objective::Interact { announcement, .. } => *announcement,
        }
    }

    /// The objective's own `compass`, when it states one (spec-0104 §6).
    pub fn compass(&self) -> Option<Visibility> {
        match self {
            Objective::TalkTo { compass, .. }
            | Objective::ReachAnchor { compass, .. }
            | Objective::Kill { compass, .. }
            | Objective::Collect { compass, .. }
            | Objective::Interact { compass, .. } => *compass,
        }
    }

    /// The objective's `highlight`, when its kind admits one and it states it
    /// (spec-0104 §7). A `reach-anchor` has nothing to light.
    pub fn highlight(&self) -> Option<Highlight> {
        match self {
            Objective::TalkTo { highlight, .. }
            | Objective::Kill { highlight, .. }
            | Objective::Collect { highlight, .. }
            | Objective::Interact { highlight, .. } => *highlight,
            Objective::ReachAnchor { .. } => None,
        }
    }

    /// **Is this objective tracked** (spec-0104 §6.1): announced, and its
    /// resolved `compass` — its own, else the campaign's
    /// [`Guidance::compass`] — is `shown`. The compass follows exactly these.
    pub fn tracked(&self, guidance: &Guidance) -> bool {
        self.announced(guidance) && self.compass().unwrap_or(guidance.compass).is_shown()
    }

    /// **Is this objective announced** (spec-0093): it has a `title` and its
    /// resolved visibility — its own `announcement`, else the campaign's
    /// [`Guidance::announcements`] — is `shown`. The emitter prints the
    /// activation and completion lines for exactly these objectives, so every
    /// rule about what the party is told reads this and nothing else.
    pub fn announced(&self, guidance: &Guidance) -> bool {
        self.title().is_some_and(|t| !t.trim().is_empty())
            && self
                .announcement()
                .unwrap_or(guidance.announcements)
                .is_shown()
    }

    /// The flags that must be set before this objective activates (v0.3).
    pub fn requires_flags(&self) -> &[FlagId] {
        match self {
            Objective::TalkTo { requires_flags, .. }
            | Objective::ReachAnchor { requires_flags, .. }
            | Objective::Kill { requires_flags, .. }
            | Objective::Collect { requires_flags, .. }
            | Objective::Interact { requires_flags, .. } => requires_flags,
        }
    }

    /// The negative flag gate (DSL v0.6): flags whose being set **suppresses**
    /// this objective. The dual of [`Objective::requires_flags`].
    pub fn forbids_flags(&self) -> &[FlagId] {
        match self {
            Objective::TalkTo { forbids_flags, .. }
            | Objective::ReachAnchor { forbids_flags, .. }
            | Objective::Kill { forbids_flags, .. }
            | Objective::Collect { forbids_flags, .. }
            | Objective::Interact { forbids_flags, .. } => forbids_flags,
        }
    }

    /// The numeric gate terms (DSL v0.10, spec-0031): comparisons that must hold
    /// before this objective activates. See [`StateCompare`].
    pub fn requires_state(&self) -> &[StateCompare] {
        match self {
            Objective::TalkTo { requires_state, .. }
            | Objective::ReachAnchor { requires_state, .. }
            | Objective::Kill { requires_state, .. }
            | Objective::Collect { requires_state, .. }
            | Objective::Interact { requires_state, .. } => requires_state,
        }
    }

    /// What this objective does to the story (DSL v0.8, spec-0025).
    pub fn happening(&self) -> Option<&Happening> {
        match self {
            Objective::TalkTo { happening, .. }
            | Objective::ReachAnchor { happening, .. }
            | Objective::Kill { happening, .. }
            | Objective::Collect { happening, .. }
            | Objective::Interact { happening, .. } => happening.as_ref(),
        }
    }

    /// The objective's window (spec-0133), when it declares one.
    pub fn limit(&self) -> Option<&Limit> {
        match self {
            Objective::TalkTo { limit, .. }
            | Objective::ReachAnchor { limit, .. }
            | Objective::Kill { limit, .. }
            | Objective::Collect { limit, .. }
            | Objective::Interact { limit, .. } => limit.as_ref(),
        }
    }

    /// The vehicle the leg to this objective is ridden on (spec-0135 §4),
    /// when it declares one.
    pub fn ride(&self) -> Option<&VehicleId> {
        match self {
            Objective::TalkTo { ride, .. }
            | Objective::ReachAnchor { ride, .. }
            | Objective::Kill { ride, .. }
            | Objective::Collect { ride, .. }
            | Objective::Interact { ride, .. } => ride.as_ref(),
        }
    }

    /// Mutable access to the objective's window (i18n localization of its
    /// `on_expire`).
    pub fn limit_mut(&mut self) -> Option<&mut Limit> {
        match self {
            Objective::TalkTo { limit, .. }
            | Objective::ReachAnchor { limit, .. }
            | Objective::Kill { limit, .. }
            | Objective::Collect { limit, .. }
            | Objective::Interact { limit, .. } => limit.as_mut(),
        }
    }

    /// The bot stealth hint (DSL v0.4): traverse this leg sneaking.
    pub fn stealth(&self) -> bool {
        match self {
            Objective::TalkTo { stealth, .. }
            | Objective::ReachAnchor { stealth, .. }
            | Objective::Kill { stealth, .. }
            | Objective::Collect { stealth, .. }
            | Objective::Interact { stealth, .. } => *stealth,
        }
    }

    /// The holder this objective ADOPTS (DSL v0.49, spec-0111), if it is a
    /// `collect` that declares one: the anchor whose prefab-placed holder — a
    /// container, a frame or a stand — it fills instead of conjuring its own
    /// chest. `None` on every other objective and on a `collect` that keeps the
    /// compiler-placed chest.
    pub fn collect_holder(&self) -> Option<&AnchorId> {
        match self {
            Objective::Collect { holder, .. } => holder.as_ref(),
            _ => None,
        }
    }

    /// The slot a `collect`'s item lands in in its adopted holder (DSL v0.49,
    /// spec-0111 §3.4), if it names one.
    pub fn collect_slot(&self) -> Option<&str> {
        match self {
            Objective::Collect { slot, .. } => slot.as_deref(),
            _ => None,
        }
    }

    /// The wave whose declared drops provide this objective's item (DSL v0.9),
    /// if it is a `collect` that declares one. `None` on every other objective
    /// and on a `collect` fed by a container.
    pub fn collect_dropped_by(&self) -> Option<&WaveId> {
        match self {
            Objective::Collect { dropped_by, .. } => dropped_by.as_ref(),
            _ => None,
        }
    }

    /// The padding-stack count of a `collect` (DSL v0.8); `0` for every other
    /// objective and for a `collect` that fills the single required stack only.
    pub fn collect_fill_count(&self) -> u32 {
        match self {
            Objective::Collect { fill_count, .. } => *fill_count,
            _ => 0,
        }
    }

    /// The `interact` prop block (DSL v0.4), if this is an `interact` with a prop.
    pub fn prop(&self) -> Option<&Prop> {
        match self {
            Objective::Interact { prop, .. } => prop.as_ref(),
            _ => None,
        }
    }

    /// The `interact`'s declared answers (spec-0117 §4), if any.
    pub fn answers(&self) -> Option<&crate::Answers> {
        match self {
            Objective::Interact { answers, .. } => answers.as_ref(),
            _ => None,
        }
    }

    /// Whether the `interact` releases its block to vanilla at completion
    /// (spec-0117 §3a); `false` for every other objective.
    pub fn release(&self) -> bool {
        matches!(self, Objective::Interact { release: true, .. })
    }

    /// The kebab type tag.
    pub fn kind(&self) -> &'static str {
        match self {
            Objective::TalkTo { .. } => "talk-to",
            Objective::ReachAnchor { .. } => "reach-anchor",
            Objective::Kill { .. } => "kill",
            Objective::Collect { .. } => "collect",
            Objective::Interact { .. } => "interact",
        }
    }

    /// The v0.3 verb name if this objective is one of the verbs introduced in
    /// DSL v0.3 (`kill`/`collect`/`interact`). These validate in v0.3 campaigns
    ///.
    pub fn v03_verb(&self) -> Option<&'static str> {
        match self {
            Objective::Kill { .. } => Some("kill"),
            Objective::Collect { .. } => Some("collect"),
            Objective::Interact { .. } => Some("interact"),
            Objective::TalkTo { .. } | Objective::ReachAnchor { .. } => None,
        }
    }
}
