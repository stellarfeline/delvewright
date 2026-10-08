//! A quest objective: what the party does to advance a quest.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::serde_fields::{is_false, is_zero};
use crate::{
    AnchorId, FlagId, Guidance, Happening, NpcId, ObjectiveId, Prop, StateCompare, Visibility,
    WaveId,
};

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
        /// Whether this objective is **announced** — the `New objective` line, the
        /// hint's line, the cue sound and the `Objective complete` line (spec-0093).
        /// Absent = the campaign's [`Guidance::announcements`]. An objective with
        /// no `title` is never announced whatever this says; `shown` on one is
        /// `DW0961`. A hint on an unannounced objective is `DW0862`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        announcement: Option<Visibility>,
        /// The NPC to talk to.
        npc: NpcId,
        /// Prerequisite objectives (intra-quest ordering).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        after: Vec<ObjectiveId>,
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
        /// it. Default empty, so a pre-0.10 campaign is byte-identical.
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
        /// Whether this objective is **announced** — the `New objective` line, the
        /// hint's line, the cue sound and the `Objective complete` line (spec-0093).
        /// Absent = the campaign's [`Guidance::announcements`]. An objective with
        /// no `title` is never announced whatever this says; `shown` on one is
        /// `DW0961`. A hint on an unannounced objective is `DW0862`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        announcement: Option<Visibility>,
        /// The anchor to reach.
        anchor: AnchorId,
        /// Whether the glowing end-rod marker is summoned at the anchor when this
        /// objective activates (spec-0093). Absent = the campaign's
        /// [`Guidance::markers`]. The completion volume is adjudicated either way.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        marker: Option<Visibility>,
        /// Completion radius (blocks).
        radius: u32,
        /// Prerequisite objectives (intra-quest ordering).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        after: Vec<ObjectiveId>,
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
        /// it. Default empty, so a pre-0.10 campaign is byte-identical.
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
        /// Whether this objective is **announced** — the `New objective` line, the
        /// hint's line, the cue sound and the `Objective complete` line (spec-0093).
        /// Absent = the campaign's [`Guidance::announcements`]. An objective with
        /// no `title` is never announced whatever this says; `shown` on one is
        /// `DW0961`. A hint on an unannounced objective is `DW0862`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        announcement: Option<Visibility>,
        /// The wave (stage-5 `waves` ref) whose mobs must be slain.
        wave: WaveId,
        /// Prerequisite objectives.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        after: Vec<ObjectiveId>,
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
        /// it. Default empty, so a pre-0.10 campaign is byte-identical.
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
    },
    /// Completed when `count` of `item` have been collected (v0.3).
    ///
    /// The items are provided in a container: the compiler's own chest at
    /// `anchor` by default, or — since DSL v0.8 — the prefab's existing
    /// chest/barrel at [`Objective::Collect::container`], optionally carrying an
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
        /// Whether this objective is **announced** — the `New objective` line, the
        /// hint's line, the cue sound and the `Objective complete` line (spec-0093).
        /// Absent = the campaign's [`Guidance::announcements`]. An objective with
        /// no `title` is never announced whatever this says; `shown` on one is
        /// `DW0961`. A hint on an unannounced objective is `DW0862`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        announcement: Option<Visibility>,
        /// Vanilla item id to collect (validated against the registry).
        item: String,
        /// How many are required.
        count: u32,
        /// The anchor items are provided at (chest / pickup).
        anchor: AnchorId,
        /// **Adopt the container the prefab already placed**: the anchor whose
        /// assembled-world cell holds
        /// a `chest` / `trapped_chest` / `barrel` this collect fills instead of
        /// conjuring its own chest at [`Objective::Collect::anchor`].
        ///
        /// Same division of labour a `loot` entry and a trap's dispenser already
        /// keep with the prefab: furniture belongs in the piece. A beach camp's
        /// barrel is scenery the player has been walking past since minute one —
        /// having the compiler `setblock` a *second*, floating chest beside it to
        /// hold the quest item is exactly the downstream workaround the no-hack
        /// rule forbids. A `container` whose cell holds no container is a build
        /// error (`DW0438`), never a silent fill into a wall.
        ///
        /// The critical-path step's position follows the container (the bot opens
        /// *this* block), and no chest is placed at `anchor` when it is set.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        container: Option<AnchorId>,
        /// **The item comes off a body, not out of a box**: the wave whose
        /// declared `drops[]` yield
        /// this objective's item. No container is placed — not the compiler's own
        /// chest at `anchor`, not a prefab one — and `container` is therefore
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
        /// this counts SLOTS: the objective's own stack lands in `container.0` and
        /// each padding stack repeats it in `container.1`, `container.2`, … Slot
        /// assignment is positional and total, the same determinism story `loot`
        /// tells (ADR-0006): no RNG, no loot tables, nothing to reseed.
        ///
        /// The padding is the same item, so taking the whole barrel still
        /// completes the objective and never over- or under-counts it.
        #[serde(default, skip_serializing_if = "is_zero")]
        fill_count: u32,
        /// Prerequisite objectives.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        after: Vec<ObjectiveId>,
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
        /// it. Default empty, so a pre-0.10 campaign is byte-identical.
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
        /// Whether this objective is **announced** — the `New objective` line, the
        /// hint's line, the cue sound and the `Objective complete` line (spec-0093).
        /// Absent = the campaign's [`Guidance::announcements`]. An objective with
        /// no `title` is never announced whatever this says; `shown` on one is
        /// `DW0961`. A hint on an unannounced objective is `DW0862`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        announcement: Option<Visibility>,
        /// The anchor the interaction entity stands at.
        anchor: AnchorId,
        /// Whether the glowing lantern marker is summoned beside the hitbox when
        /// this objective activates (spec-0093). Absent = the campaign's
        /// [`Guidance::markers`]. The `minecraft:interaction` hitbox is summoned
        /// either way — it is what the player presses. Meaningless beside a
        /// `prop`, which never had a marker: declaring both is `DW0962`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        marker: Option<Visibility>,
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
        /// Diegetic feedback for a click that arrives without the required item in
        /// hand (DSL v0.7): narrated to that player in
        /// chat instead of the silence the gate used to answer with. Requires
        /// `requires_item` (`DW0437`).
        ///
        /// Only fires while the objective is genuinely open — same activation gate
        /// as the affordance itself — so a finished or not-yet-active interaction
        /// stays quiet.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        missing_item_hint: Option<String>,
        /// Prop block that IS the interaction affordance (DSL v0.4, spec-0008
        /// §2): the compiler `setblock`s it at the anchor on activation (exactly
        /// as `collect` uses a real chest). Omitted = the glowing-lantern
        /// hologram marker (the v0.3 fallback).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prop: Option<Prop>,
        /// Prerequisite objectives.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        after: Vec<ObjectiveId>,
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
        /// it. Default empty, so a pre-0.10 campaign is byte-identical.
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
    },
}

impl Objective {
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

    /// This objective's prerequisites.
    pub fn after(&self) -> &[ObjectiveId] {
        match self {
            Objective::TalkTo { after, .. }
            | Objective::ReachAnchor { after, .. }
            | Objective::Kill { after, .. }
            | Objective::Collect { after, .. }
            | Objective::Interact { after, .. } => after,
        }
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

    /// The objective's own `marker`, when it is a kind that has one and states
    /// it (spec-0093). `None` for a `talk-to`, `kill` or `collect`, which carry no
    /// such field, and for an `interact` or `reach-anchor` that leaves it absent.
    pub fn marker(&self) -> Option<Visibility> {
        match self {
            Objective::ReachAnchor { marker, .. } | Objective::Interact { marker, .. } => *marker,
            Objective::TalkTo { .. } | Objective::Kill { .. } | Objective::Collect { .. } => None,
        }
    }

    /// Whether this objective's kind summons a wayfinding marker at all: a
    /// `reach-anchor` (its end rod) or an `interact` with no `prop` (its lantern).
    /// A `collect` places its chest, a `talk-to` has a body, a `kill` has bodies,
    /// and an `interact` with a `prop` has the prop.
    pub fn summons_marker(&self) -> bool {
        match self {
            Objective::ReachAnchor { .. } => true,
            Objective::Interact { prop, .. } => prop.is_none(),
            Objective::TalkTo { .. } | Objective::Kill { .. } | Objective::Collect { .. } => false,
        }
    }

    /// **Is this objective marked** (spec-0093): its kind summons a marker and
    /// its resolved visibility — its own `marker`, else the campaign's
    /// [`Guidance::markers`] — is `shown`.
    pub fn marker_shown(&self, guidance: &Guidance) -> bool {
        self.summons_marker() && self.marker().unwrap_or(guidance.markers).is_shown()
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

    /// The container this objective ADOPTS (DSL v0.8), if it is a `collect` that
    /// declares one: the anchor whose prefab-placed chest/barrel it fills instead
    /// of conjuring its own chest. `None` on every other objective and on a
    /// `collect` that keeps the compiler-placed chest.
    pub fn collect_container(&self) -> Option<&AnchorId> {
        match self {
            Objective::Collect { container, .. } => container.as_ref(),
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
