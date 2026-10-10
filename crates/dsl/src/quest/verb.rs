//! The verb of an effect: what one effect does when it fires.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::serde_fields::is_zero3;
use crate::{
    ActorId, AnchorId, AssemblyId, AtmosphereId, CameraShot, Carrier, CutsceneParty, DamageKind,
    DespawnStyle, EndingId, FireworkExplosion, FlagId, Mark, NpcId, PlaceRef, PrefabId, QuestEffect,
    SequenceStep, StakeId, StateId, StealthZone, WaveId, WorldTime, WorldWeather,
};

#[cfg(doc)]
use crate::{KitItem, MAX_EFFECT_SECONDS, MAX_POTION_AMPLIFIER, Stake, enchantment_component};

/// What an effect does, without the guard: the closed set of verbs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Verb {
    /// Opens a prefab-declared gate (one-way).
    OpenGate {
        /// The gate anchor to open.
        anchor: AnchorId,
    },
    /// Seals a prefab-declared gate — the physical dual of `open-gate` (DSL v0.6):
    /// fills the gate anchor's region with the block the anchor declares (e.g. the
    /// boulder's `minecraft:basalt`), turning an opened threshold back into a wall.
    /// The declared fill block is prefab metadata; a gate anchor with no `block` is
    /// rejected (`DW0343`). The completability model treats the region as **solid**
    /// from the point in the quest DAG where this fires (mirroring how `open-gate`'s
    /// clearing is modelled) — a critical path that must cross a gate after it seals
    /// fails the DW0311 reachability proof.
    CloseGate {
        /// The gate anchor to seal.
        anchor: AnchorId,
        /// What the seal *says* when a player right-clicks it (DSL v0.8). A sealed gate is a wall the party will walk back to
        /// and press: the compiler answers that press on the actionbar. Absent, the
        /// compiler's canonical English is baked in (`The way is sealed.`) exactly
        /// as `world.boundary.message` does; authored, the line is l10n-inventoried
        /// under `<effect-key>.sealed_hint` and translates like every other
        /// player-visible string.
        ///
        /// Unlike `happening`, this **does** print in the hand-written `Debug`
        /// below when present — it changes emission, so two otherwise-identical
        /// sequences that differ only in their seal's answer are different content.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sealed_hint: Option<String>,
    },
    /// Marks the campaign complete (final advancement + credits). Terminal — not
    /// flag-gatable (gating the campaign's own completion is a deadlock footgun),
    /// so this variant carries no `requires_flags`.
    CampaignComplete {
        /// Which ENDING this is (DSL v0.8, spec-0025).
        /// A campaign with more than one `campaign-complete` has more than one
        /// ending, and a branch that runs to an ending names it here — so the
        /// terminality proof (`DW0482`) can state *which* ending a branch reached
        /// instead of merely that something ended. There is no separate `endings`
        /// section: the set of endings is exactly the set named here, the same
        /// rule flags follow.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ending: Option<EndingId>,
    },
    /// Gives the party an item (v0.3; party-wide since v0.6/spec-0018).
    GiveItem {
        /// Vanilla item id to give (validated against the registry).
        item: String,
        /// How many to give.
        count: u32,
        /// Optional display name (DSL v0.4), matching [`KitItem::name`].
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        /// Who receives it (DSL v0.6, spec-0018). Absent = [`Carrier::All`] — every
        /// party member. `one` hands a single copy to the player whose action fired
        /// the effect; it is rejected in a scheduler-only bundle (`DW0371`), which
        /// has no acting player.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        carrier: Option<Carrier>,
        /// Enchantments on the given stack (`{"minecraft:sharpness": 2}`) —
        /// the field a `loot` stack and an equipped piece carry, under the same
        /// checks (`DW0433`/`DW0434`) and written by the same rule
        /// ([`enchantment_component`]): on a `minecraft:enchanted_book` they are
        /// the book's stored enchantments, the ones an anvil applies.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        enchantments: BTreeMap<String, u32>,
    },
    /// Sets a campaign flag, enabling flag-gated objectives (v0.3).
    SetFlag {
        /// The flag to set.
        flag: FlagId,
    },
    /// Writes a declared datum to an absolute value (DSL v0.10, spec-0031).
    SetState {
        /// The datum to write (stage-5 `state` ref).
        state: StateId,
        /// The value to write.
        value: i32,
    },
    /// Moves a declared datum by a signed amount (DSL v0.10, spec-0031).
    ///
    /// **Signed on purpose.** A purse that a shop debits and a stake that a death
    /// forfeits are the same operation with the sign flipped; a separate
    /// `subtract-state` would be a second verb for one mechanism, and the first
    /// campaign to need "add a negative" would have to choose between them.
    AddState {
        /// The datum to move (stage-5 `state` ref).
        state: StateId,
        /// How far to move it. Negative counts down.
        amount: i32,
    },
    /// Leaves a declared [`Stake`] behind for the acting player (DSL v0.10,
    /// spec-0032): forfeit the declared share of its datum, and place a
    /// collectable marker at the compile-time anchor for where they are.
    ///
    /// **Nothing about this verb says "death".** It is written in `on_death`
    /// because that is where a souls-shaped delve wants it, but the mechanism is
    /// "leave a recoverable cache where the acting player stands", and the effect
    /// carries the ordinary gate so any root may run it. What *is* death-specific
    /// — that the corpse stands on the death position, so the placement lookup has
    /// a position to key on — is a property of the `on_death` root, not of this
    /// verb.
    DropStake {
        /// The stake (stage-5 `stakes` ref) to leave.
        stake: StakeId,
    },
    /// Returns a declared datum to its declared `initial` (DSL v0.10,
    /// spec-0031) — the verb `FlagId` has never had.
    ClearState {
        /// The datum to clear (stage-5 `state` ref).
        state: StateId,
    },
    /// Spawns a stage-5 wave's mobs at its anchor (v0.3).
    SpawnWave {
        /// The wave (stage-5 `waves` ref) to spawn.
        wave: WaveId,
    },
    /// Narrates a player-visible line (DSL v0.4, spec-0008 §3). `text` enters the
    /// l10n key inventory like any player-visible string.
    Narrate {
        /// The line shown to the player.
        text: String,
        /// Presentation channel (default `chat`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        style: Option<NarrateStyle>,
        /// Optional sound id played alongside the line.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sound: Option<String>,
    },
    /// Sets a block at an anchor (DSL v0.4, spec-0008 §2). General form of a prop
    /// placement. Block id validated against the pinned 1.21.11 block registry;
    /// a vanilla blockstate suffix (`minecraft:grindstone[face=floor]`) is
    /// accepted and passed through verbatim (DSL v0.6).
    SetBlock {
        /// The anchor to place the block at.
        anchor: AnchorId,
        /// Vanilla block id to place.
        block: String,
    },
    /// **Fill a declared region with a block** at runtime (DSL v0.10, spec-0031).
    ///
    /// The general spelling of the capability `open-gate` / `close-gate` carried
    /// privately: a region, filled or cleared, from a point in the quest DAG.
    /// `close-gate` is this verb with the region and the block read off a prefab
    /// gate anchor instead of authored; `set-block` is the one-cell case at a
    /// point anchor. All three lower through one emission
    /// (`emit::fill_region_command`) and are modelled by one completability rule
    /// (`plan::RegionEvent`), so a third consumer inherits the proof instead of
    /// re-deriving it.
    ///
    /// From the DAG point at which this fires, the completability model treats the
    /// filled cells as whatever the **block** makes them. A full-cube block leaves
    /// them **solid**, exactly as a `close-gate` seal is: a critical path that must
    /// cross the region afterwards fails `DW0311`. `minecraft:water` /
    /// `minecraft:lava` leave them **flooded** — impassable and never floor, because
    /// nothing stands on a fluid — and because a fill carries no `replace` filter it
    /// takes away whatever floor was in the box, so a forced leg that needed that
    /// footing fails `DW0544`.
    FillRegion {
        /// The volume to fill, as an anchor-centred box (`anchor ± extent`).
        ///
        /// Deliberately the existing [`StealthZone`] — the engine's one
        /// anchor-centred box object class, already shared by `damage-players`'s
        /// `in` filter, `collapse`'s `region_anchor`, a `volley` kill zone and a
        /// `lethal_volumes[]` region, and resolved through the single
        /// `Plan::zone_box`. A private twin with the same two fields would be
        /// `tools/ci/check-capability-ownership.py` check C by construction.
        ///
        /// An anchor-centred box rather than a prefab `region` anchor for the
        /// reason `collapse` states: the assembled model deletes every gate-region
        /// anchor's cells, so a slab declared that way would already be gone.
        region: StealthZone,
        /// The block the region is filled with (validated against the pinned
        /// 1.21.11 block registry, `DW0193`).
        block: String,
    },
    /// **Clear a declared region to air** at runtime (DSL v0.10, spec-0031) — the
    /// physical dual of [`Verb::FillRegion`], and the general spelling of
    /// what `open-gate` does to a gate anchor's region.
    ///
    /// The completability model treats the cleared cells as **passable** from the
    /// DAG point at which this fires, with one exception it states out loud: a
    /// cleared cell the model already floods stays impassable, because clearing a
    /// block does not remove water (`nav::World::with_cleared`).
    ClearRegion {
        /// The volume to clear, as an anchor-centred box (`anchor ± extent`) —
        /// the same object class [`Verb::FillRegion`] fills.
        region: StealthZone,
    },
    /// **Repaint a volume's sky** (spec-0080 §3.3): `/fillbiome` over the
    /// volume with the named atmosphere's biome, while the party stands in it.
    ///
    /// A runtime edit of the world keyed to a volume, so it is a verb of the
    /// physical-edit family [`Verb::FillRegion`] / [`Verb::ClearRegion`] form.
    /// Exactly one of `region` / `place` (`DW0929`): a volume inside a place is
    /// the creator's judgement, a whole place's bounds are a derivation the
    /// creator never types. Painting back is this verb naming the place's own
    /// atmosphere, or `atmosphere: null` for the horizon's biome.
    ///
    /// A hard cut: the client blends fog over its biome-blend radius and grass
    /// not at all, and biome cells are 4×4×4, so the painted volume is the
    /// enclosing 4-aligned box, up to three blocks past each face.
    SetAtmosphere {
        /// One of `world.atmospheres[]`, or `null` for the horizon's biome.
        #[serde(default)]
        atmosphere: Option<AtmosphereId>,
        /// The volume: exactly one of `region` (an anchor-centred box, the
        /// same object class [`Verb::FillRegion`] fills) or `place` (an
        /// `area/…` or a site-plan box's `node/…`, whose volume is the cells
        /// the place's own `atmosphere` paints at setup: its bounds, grown as
        /// far as the client's biome blend reads).
        #[serde(flatten)]
        at: PlaceRef,
    },
    /// **Opens a placed piece's contingent way** (DSL v0.12, spec-0042 §2.4): the
    /// broken flight a beat repairs, the bridge a beat lowers, the rubble a beat
    /// clears.
    ///
    /// A piece's spatial contract may declare a traversal edge whose crossability
    /// depends on a named region — `laid` (empty as built, opening fills it) or
    /// `cleared` (built solid, opening voids it). The prefab checker proves the
    /// piece is severed as shipped and joined once that region is opened; this is
    /// the verb that opens it, and it is the only one, because a way is the object
    /// and opening it is the operation.
    ///
    /// **There is no region, no block and no sign on this effect, and that is the
    /// design rather than an omission.** All three are read from the piece's own
    /// exported metadata (`spatial_contract.edges[].way`), so the effect and the
    /// building cannot disagree about what a way is — two authorities plus an
    /// equality check is the defect this shape avoids, not a variant of the fix
    /// (spec-0042 AC8). What the campaign decides is *when*.
    ///
    /// Completability: the way is **shut until this fires**, and from the DAG
    /// point at which it fires the region is solid-and-footing (`laid`) or
    /// passable (`cleared`) — the same [`Verb::FillRegion`] /
    /// [`Verb::ClearRegion`] model, fed from metadata instead of from an
    /// authored box, so this verb inherits the forced-footing rule (`DW0546`)
    /// rather than restating it. Required content standing beyond a way that no
    /// forced opening precedes is `DW0548`, which names the way, the effect and
    /// the element.
    OpenWay {
        /// The placed piece whose way this opens (`prefab/<name>`).
        ///
        /// A piece, not an anchor: the way's cells are the contract's, not a gate
        /// anchor's, and a piece placed twice has two ways. The reference must
        /// name exactly one placement; naming none or several is `DW0547`.
        piece: PrefabId,
        /// The way's region name, as the piece's contract exports it
        /// (`spatial_contract.edges[].way.region`).
        way: String,
    },
    /// Despawns an NPC and its interaction hitbox (DSL v0.4, spec-0008 §5). The
    /// NPC leaves unseen: no death animation, red flash or death particles.
    DespawnNpc {
        /// The NPC (stage-2 ref) to remove.
        npc: NpcId,
    },
    /// Moves an NPC (and its interaction hitbox in lockstep) to an anchor (DSL
    /// v0.4, spec-0008 §5 + addendum). The compiler plans a **collision-safe walked
    /// path** by A* over the solved voxel grid and emits per-tick teleport
    /// waypoints along it, so the NPC never clips a wall and walks up to (not into)
    /// a solid affordance. An unroutable move is a compile error (`DW0307`).
    MoveNpc {
        /// The NPC (stage-2 ref) to move.
        npc: NpcId,
        /// The destination mark: an anchor and an optional offset (spec-0066).
        to: Mark,
        /// Optional travel speed in blocks/tick (defaults to ~0.15).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        speed: Option<f64>,
        /// Effects fired once the NPC arrives at the destination cell — exact
        /// parity with [`Verb::MoveActor`]
        /// `on_arrive`: same arrival detection (the walk driver's final tick), same
        /// execution context, and every deep effect walker recurses into it via
        /// [`QuestEffect::nested_effect_lists`]. This is what lets content gate a
        /// beat on walk *completion* instead of fire-and-forgetting the walk (e.g.
        /// `on_arrive: [set-flag]` so a cutscene waits for the NPC to reach its
        /// mark).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        on_arrive: Vec<QuestEffect>,
    },
    /// Plays a scripted camera cutscene (DSL v0.4 addendum). Per player: save
    /// gamemode+position, spectator, then dolly two co-located cameras along a
    /// straight-line lerp between waypoints and alternate `spectate` between them
    /// each tick (the two-camera bounce; the same-entity re-`spectate` is a server
    /// no-op and is never emitted), and restore on completion. The compiler
    /// validates the dolly path passes only through non-solid blocks — cameras
    /// fly but must not clip a solid (`DW0308`).
    ///
    /// Camera **aim** (DSL v0.6): with `look_at`, every dolly camera is rotated at
    /// emission to face that world point from its own position, so the shot keeps
    /// its subject framed for the whole move; without it, the camera faces along
    /// the direction of travel (the segment it is currently traversing).
    ///
    /// **Shape** — a cutscene is a list of [`CameraShot`]s played back-to-back
    /// inside one save/restore bracket (hard cut between shots). Two accepted,
    /// mutually exclusive spellings, both normalized by
    /// [`QuestEffect::cutscene_shots`]:
    /// - multi-shot (DSL v0.6): `{"shots": [{path, seconds, look_at?}, …]}`;
    /// - single-shot (DSL v0.4): `{"path": […], "seconds": n, "look_at"?: …}` —
    ///   exactly equivalent to a one-entry `shots` list.
    ///
    /// Mixing or omitting both is `DW0199`.
    Cutscene {
        /// Multi-shot form (DSL v0.6): the ordered shot list. Mutually exclusive
        /// with the single-shot `path`/`seconds` fields (`DW0199`).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        shots: Vec<CameraShot>,
        /// Single-shot form (DSL v0.4): ordered camera waypoints (straight-line
        /// lerp between them).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        path: Vec<Mark>,
        /// Single-shot form (DSL v0.4): shot duration in seconds.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        seconds: Option<u32>,
        /// Single-shot form (DSL v0.6): the subject the camera keeps framed.
        /// Absent = face along the direction of travel.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        look_at: Option<Mark>,
        /// Whether the party's bodies stay in the scene while the camera flies
        /// (spec-0095). Absent = `present`: every player in play is shown by a
        /// stand-in wearing their own skin and equipment, standing where they
        /// stood, for the cutscene's whole length. `absent` takes the bodies out
        /// of the scene: a vision, a memory, a scene somewhere else.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        party: Option<CutsceneParty>,
    },
    /// Cuts the dimension-global world time to a new state (DSL v0.5, spec-0010).
    /// Instantaneous (vanilla has no gradual transition); the state persists
    /// because the daylight cycle is frozen by sealing.
    SetTime {
        /// The time state to cut to.
        time: WorldTime,
    },
    /// Cuts the dimension-global weather to a new state (DSL v0.5, spec-0010).
    /// Instantaneous; persists because the weather cycle is frozen by sealing.
    SetWeather {
        /// The weather state to cut to.
        weather: WorldWeather,
    },
    /// Plays a vanilla sound event, positionally or per-player (DSL v0.6,
    /// spec-0014). `sound` is validated against the vendored pinned-1.21.11
    /// sound-event registry (`DW0326` unknown). `at` selects where the sound
    /// originates (default: each player's own position); `volume`/`pitch` map to
    /// the `playsound` command's trailing args (pitch clamps to 0.0..=2.0 in
    /// vanilla).
    PlaySound {
        /// The vanilla sound-event id (`minecraft:` prefix optional).
        sound: String,
        /// Where the sound plays from (default: `players`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        at: Option<SoundAt>,
        /// Playback volume (vanilla default 1.0; > 1.0 only extends audible range).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        volume: Option<f64>,
        /// Playback pitch (vanilla 0.0..=2.0; default 1.0).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pitch: Option<f64>,
    },
    /// Deals damage to the acting player(s) (DSL v0.6): the real consequence a
    /// stealth `on_caught` or a souls-style beat needs — vanilla's `/damage`
    /// primitive. Runs in the effect's `as @a` / `as @s` context, so `@s` is each
    /// acting player: at top level it damages every player once; inside a stealth
    /// `on_caught` it damages the caught player (the "caught → death → respawn at
    /// checkpoint" beat). `amount` is in **half-hearts** (1 HP each); an amount ≥ 40
    /// is lethal through golden apples / absorption. The envelope's `in`
    /// ([`QuestEffect::within`]) narrows to acting players standing inside an
    /// anchor-centred box, keeping the per-`@s` semantics. `damage_type` is the damage
    /// type — a curated set of vanilla types that all respect `keepInventory` and do
    /// **not** bypass totems (no `out_of_world`/`generic_kill`); default `generic`.
    /// (The field is `damage_type`, not `type`, because the effect enum is
    /// internally tagged on `type`.)
    DamagePlayers {
        /// Damage dealt, in half-hearts (1 = 1 HP; ≥ 40 is effectively lethal).
        amount: u32,
        /// The damage type (default [`DamageKind::Generic`]).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        damage_type: Option<DamageKind>,
    },
    /// Sets the party-wide respawn checkpoint (DSL v0.6, spec-0012). Emits
    /// `spawnpoint @a` at the anchor cell and mirrors the coords into
    /// `storage dw:cp pos`. Party-wide and monotonic by quest order (a later
    /// `set-checkpoint` always replaces an earlier one). The compiler proves the
    /// cell is standable (`DW0316`) and that the remaining critical path stays
    /// reachable from it (`DW0315`).
    SetCheckpoint {
        /// The prefab checkpoint anchor the party respawns at.
        anchor: AnchorId,
        /// Per-player effects re-run each time a player respawns while this
        /// checkpoint is the active one — scene reset (e.g. re-caging an
        /// unleashed actor). Emitted idempotently in declared order; empty = no
        /// hook. Respawn is detected via the vanilla `deathCount` criterion.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        on_respawn: Vec<QuestEffect>,
    },
    /// Places a **bonfire** rest point (DSL v0.6, spec-0016 §1) — the sibling of
    /// [`Verb::SetCheckpoint`] for souls-mode pacing. The effect *arms*
    /// the rest affordance (a `minecraft:interaction` the player right-clicks at
    /// the anchor, the campfire prop being prefab dressing); the checkpoint moves
    /// only **when the party actually rests**. Resting fires `on_rest` — the
    /// scene reset that makes retry cheap: re-arming traps, re-seating waves
    /// declared `respawns_on_rest`, restoring actor postures. Death respawns the
    /// party at the last-rested bonfire and runs the **same** `on_rest` bundle,
    /// so the world's answer to a death and to a rest is identical (spec-0016:
    /// death is an investment, never a tax).
    ///
    /// Proofs are inherited from the checkpoint machinery: the anchor must be
    /// standable (`DW0316`) and must not strand the party (`DW0315`), rooted at
    /// the beat that arms the bonfire (the earliest moment a rest can happen).
    Bonfire {
        /// The prefab anchor the rest affordance stands at, and the cell the
        /// party respawns at once rested.
        anchor: AnchorId,
        /// Effects re-run on every rest **and** on every respawn at this bonfire
        /// — the scene reset. Emitted in declared order and expected to be
        /// idempotent (the same contract as `set-checkpoint`'s `on_respawn`).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        on_rest: Vec<QuestEffect>,
        /// Title of the two-option rest dialog (DSL v0.8).
        /// Absent = the compiler's canonical English `Bonfire`,
        /// baked at emit time (the `world.boundary.message` precedent): an
        /// authored line is inventoried and translates like every other
        /// player-visible string.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prompt: Option<String>,
        /// Label of the **rest and save** button. Absent = `Rest and save`.
        /// A dialog button is a fixed-width caption, so keep it to ~20 Latin /
        /// ~12 Han characters (skill *Writing craft* §C) — a wider label scrolls.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rest_label: Option<String>,
        /// Label of the **save only** button. Absent = `Save only`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        save_label: Option<String>,
        /// Hover tooltip of the **rest and save** button (spec-0078) — the same
        /// optional `tooltip` every dialog button carries, beside the label it
        /// explains. Absent = no tooltip. Not subject to `DW0331`: a tooltip
        /// wraps in its own hover box. Inventoried as `fx.….rest_tooltip`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rest_tooltip: Option<String>,
        /// Hover tooltip of the **save only** button (spec-0078). Absent = no
        /// tooltip. Inventoried as `fx.….save_tooltip`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        save_tooltip: Option<String>,
    },
    /// Begins a stealth beat (DSL v0.6, spec-0014):
    /// zone presence alone = hidden — no sneak requirement, which collides with
    /// the spectator cutscene camera. While active, every player must be
    /// inside some `zone` each tick; a player outside every zone for
    /// `grace_ticks` fires `on_caught` (typically a kill → checkpoint respawn).
    /// Zone membership is read from the player's position. The compiler proves
    /// each zone is standable and reachable from the activating beat (`DW0327`).
    BeginStealth {
        /// The "shadow" regions, each an anchor-centred box (see [`StealthZone`]).
        zones: Vec<StealthZone>,
        /// Per-player effects fired when a player is caught (out of every zone
        /// for `grace_ticks`). Empty = no consequence.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        on_caught: Vec<QuestEffect>,
        /// Ticks a player may be exposed before `on_caught` fires (default 20).
        #[serde(default = "default_grace_ticks")]
        grace_ticks: u32,
    },
    /// Ends the active stealth beat (DSL v0.6, spec-0014). No-op if none active.
    EndStealth,
    /// Summons a `deferred` stage-2 NPC (body + interaction hitbox + name display)
    /// at its declared anchor (DSL v0.6) — the dual of `despawn-npc`, and the
    /// scripted entrance a staged character needs. Idempotent: spawning an NPC
    /// already in the world is a no-op. Only meaningful for an NPC declared
    /// `deferred: true`; a non-deferred NPC is already in the world from init.
    SpawnNpc {
        /// The NPC (stage-2 ref) to summon.
        npc: NpcId,
    },
    // --- DSL v0.6 actor staging effects (spec-0014) ---
    /// Summons a stage-5 actor's puppet at its anchor (DSL v0.6). Idempotent: a
    /// spawn of an already-present actor is a no-op (re-caging after `unleash`).
    SpawnActor {
        /// The actor (stage-5 `actors` ref) to summon.
        actor: ActorId,
    },
    /// Removes an actor's puppet (DSL v0.6). `kill` plays the vanilla death
    /// animation (cutscene deaths); `vanish` is silent removal.
    DespawnActor {
        /// The actor (stage-5 `actors` ref) to remove.
        actor: ActorId,
        /// How the puppet is removed.
        style: DespawnStyle,
    },
    /// Walks an actor's puppet to an anchor by A*-planned per-tick teleport over
    /// the assembled model, using the actor's hitbox footprint, yawed along the
    /// path tangent (DSL v0.6). Concurrent movers are allowed (a herded
    /// flock is N synchronized `move-actor`s). Unroutable → `DW0325`. `on_arrive`
    /// effects fire once the puppet reaches the destination cell.
    MoveActor {
        /// The actor (stage-5 `actors` ref) to move.
        actor: ActorId,
        /// The destination mark: an anchor and an optional offset (spec-0066).
        to: Mark,
        /// Optional travel speed in blocks/tick (defaults to ~0.15).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        speed: Option<f64>,
        /// Effects fired once the puppet arrives at the destination cell.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        on_arrive: Vec<QuestEffect>,
    },
    /// Replaces an actor's puppet with a real-AI twin of the same type / position /
    /// name / attributes / tag (DSL v0.6) — the "attack the idle giant → real
    /// fight" beat. Re-caging is `despawn-actor` + `spawn-actor` (idempotent), not a
    /// special verb.
    UnleashActor {
        /// The actor (stage-5 `actors` ref) to unleash.
        actor: ActorId,
    },
    // --- spec-0082 assembly verbs ---
    /// Summons an assembly (spec-0082): its root, one display per rig part
    /// riding the root, and its hitbox when declared, at its mark, playing its
    /// `initial` clip. Idempotent: a spawn of a present assembly is a no-op.
    SpawnAssembly {
        /// The assembly (stage-5 `assemblies` ref) to summon.
        assembly: AssemblyId,
    },
    /// Removes an assembly's root, parts and hitbox (spec-0082). They leave
    /// unseen: a display entity has no death, so there is no `style`.
    DespawnAssembly {
        /// The assembly (stage-5 `assemblies` ref) to remove.
        assembly: AssemblyId,
    },
    /// Switches the clip an assembly plays (spec-0082); its first frame is
    /// applied on the next tick. While a strike step is in flight the switch
    /// waits for the step to end, so a story beat never cuts a strike at the
    /// frame before it lands.
    PlayClip {
        /// The assembly (stage-5 `assemblies` ref).
        assembly: AssemblyId,
        /// A clip the assembly's rig declares.
        clip: String,
    },
    /// Re-arms an assembly's strike pattern (spec-0094 §3.3). A `play-clip`
    /// stands the pattern down: the clip it plays completes and holds (or
    /// loops) whoever stands in `while_in`, and no wind-up begins again until
    /// this verb runs. The pattern resumes from its first step on the next tick
    /// some player is in its arming region. Naming an assembly that declares
    /// no `strikes` is `DW0970`.
    ArmStrikes {
        /// The assembly (stage-5 `assemblies` ref) whose pattern is re-armed.
        assembly: AssemblyId,
    },
    /// A deterministic timeline (DSL v0.6): one schedule chain firing effect groups
    /// at exact tick offsets. Effects are any in the stage-5 set except a nested
    /// `sequence` (rejected with `DW0329`).
    Sequence {
        /// Timeline steps; each fires its `effects` at `at_ticks` from the start.
        steps: Vec<SequenceStep>,
    },
    /// **Saturating projectile volley** (DSL v0.6, spec-0022): command-summoned
    /// projectiles with real velocity vectors, fired from a gallery slot into a
    /// declared kill zone.
    ///
    /// The contract is **saturation, not sniping**.
    /// Every salvo puts one projectile on the trajectory to **every standable
    /// cell of `kill_zone`**, plus one aimed at the triggering player's
    /// fire-time position (which punishes standing still). A player therefore
    /// cannot dodge a volley by strafing — escaping means *leaving the zone*, a
    /// decision rather than a lucky step. Coverage is proven at compile time:
    /// `from_anchor` must have clear line of fire to every standable kill-zone
    /// cell (`DW0442`), so "the gallery slot can actually hit a player anywhere
    /// on the stairs" is a build-time fact, not a hope.
    ///
    /// Projectiles are summoned `NoGravity` so the flown path is exactly the
    /// straight segment the coverage proof checks — proof and runtime share one
    /// geometry. Drag scales speed but not direction, so the line is preserved.
    Volley {
        /// Projectile entity id (default `minecraft:arrow`; validated against
        /// the pinned 1.21.11 registry, `DW0441`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        projectile: Option<String>,
        /// The gallery slot the volley is fired FROM — a point anchor. Its cell
        /// must be clear (a projectile spawned inside a wall never leaves it).
        from_anchor: AnchorId,
        /// The zone the volley must blanket, as an anchor-centred box
        /// (`anchor ± extent`) — the same shape `damage-players`'s `in` and
        /// `begin-stealth`'s `zones` use. Every standable cell in it receives
        /// fire each salvo.
        ///
        /// Deliberately NOT a bare prefab `region` anchor: the assembled model
        /// clears every gate-region anchor's cells unconditionally, so
        /// describing a zone that way would delete the geometry it names.
        kill_zone: StealthZone,
        /// How many rounds the pattern repeats (default 3, `DW0443` bounds it).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        salvos: Option<u32>,
        /// Ticks between salvos (default 10, `DW0443` bounds it).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        interval: Option<u32>,
    },
    /// **Ceiling collapse** (DSL v0.6, spec-0022): delete a region's blocks and
    /// drop them as `falling_block` entities — the buried-alive trap redstone
    /// cannot express at all.
    ///
    /// The post-collapse world is **modeled, not guessed**: the compiler clears
    /// the region, settles every dropped column through the existing gravity
    /// model (spec-0010), optionally paves the landing surface with
    /// `then_floor`, and re-runs the critical-path completability proof against
    /// that mutated world (`DW0445`). A collapse that buries the only route is a
    /// build error, exactly as a `shortcut` seal that strands the party is.
    Collapse {
        /// The volume whose blocks fall, as an anchor-centred box
        /// (`anchor ± extent`). Must hold blocks and sit above standable footing
        /// (`DW0444`) — a collapse over nothing is scenery, not a trap.
        ///
        /// An anchor-centred box rather than a prefab `region` anchor for a
        /// load-bearing reason: the assembled model deletes every gate-region
        /// anchor's cells, so a ceiling slab declared that way would already be
        /// gone before the trap ever fired.
        region_anchor: StealthZone,
        /// The block the falling entities are made of (default
        /// `minecraft:gravel`; validated against the pinned registry, `DW0441`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        falling_block: Option<String>,
        /// Optional block the landing surface is paved with once the debris
        /// settles — the authored post-collapse floor the completability proof
        /// reasons over.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        then_floor: Option<String>,
    },
    /// Grants a vanilla **status effect** for a stated duration (DSL v0.10,
    /// spec-0031). The engine has emitted status effects since v0.6 — the
    /// night-vision area mitigation is a self-rescheduling, region-scoped
    /// `effect give` — and exposed none, so an author who wanted blindness for a
    /// lift ride, slowness in deep water or regeneration at a shrine had no
    /// surface at all. This is that surface, over the whole pinned 1.21.11
    /// `mob_effect` registry (`DW0192` rejects an id outside it).
    ///
    /// **A grant carries a duration, and there is no way to spell one that does
    /// not.** Vanilla's `infinite` keyword is deliberately absent from this
    /// surface: an effect whose only removal is a later command is an effect the
    /// player keeps forever whenever that command does not run — a logout, a
    /// crash, a chain interrupted by a death. A duration expires on its own, with
    /// no cooperation from anything. `seconds` is therefore required and bounded
    /// (1..=[`MAX_EFFECT_SECONDS`], `DW0541`), and the *pattern* that reintroduces
    /// the same hazard — pairing a grant with a `clear-effect` that removes it
    /// while it is still live — is `DW0540`.
    ///
    /// The envelope's `in` ([`QuestEffect::within`]) narrows to players inside an
    /// anchor-centred box. It is what makes "blind whoever is riding the car"
    /// expressible without blinding the whole party. A blinding grant owes the
    /// blind-reach proof wherever it is written (`DW0943`, spec-0085 §6).
    GiveEffect {
        /// Vanilla status-effect id (e.g. `minecraft:blindness`), validated
        /// against the pinned 1.21.11 registry (`DW0192`).
        effect: String,
        /// Duration in seconds. Required, `1..=`[`MAX_EFFECT_SECONDS`]
        /// (`DW0541`) — see the variant docs for why there is no infinite form.
        seconds: u32,
        /// Amplifier (0 = level I), `0..=`[`MAX_POTION_AMPLIFIER`] (`DW0541`).
        /// Absent = 0.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        amplifier: Option<u32>,
        /// Suppress the swirling particles (vanilla's `hideParticles`). Absent =
        /// `false`, vanilla's own default.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hide_particles: Option<bool>,
    },
    /// Removes a status effect (DSL v0.10, spec-0031) — vanilla's `effect clear`.
    ///
    /// This is **not** how a `give-effect` is supposed to end: a duration is. It
    /// exists for the effects the engine did not grant — a potion the player
    /// drank, a `wither` a mob applied, the whole set at a bonfire — which is why
    /// `effect` is optional (absent = clear everything, exactly as vanilla's
    /// `effect clear <targets>` does). Pairing it with a live grant of the same
    /// effect in the same bundle is `DW0540`.
    ClearEffect {
        /// The status-effect id to remove (`DW0192`). **Absent clears every
        /// effect**, matching `effect clear <targets>` with no id.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        effect: Option<String>,
    },
    /// Teleports **everything inside a declared volume** to an anchor (DSL v0.10,
    /// spec-0031).
    ///
    /// **The selector is a region, never a block.** "Whoever is standing on this
    /// block" has three different answers for a player half a foot over the edge,
    /// a player mid-jump and a player sneaking on the lip; a volume has one, and
    /// it is the same one every tick. `from` is the anchor-centred
    /// [`StealthZone`] box the rest of the engine already uses.
    ///
    /// **The selection is total over bodies.** Emission is a single `tp
    /// @e[<box>,tag=!dw_fixture] <cell>` with no `type=`, no `limit=` and no
    /// `sort=` — every body in the volume moves, and
    /// `crates/delvec/tests/v10_teleport.rs` asserts that from the emitted
    /// selector rather than from anyone's memory. A machinery-**type** exemption
    /// of the kind a `lethal_volumes[]` entry must carry was considered and
    /// **rejected**: an NPC is a body plus a co-located `minecraft:interaction`
    /// hitbox, so exempting `minecraft:interaction` — as the lethal volume does —
    /// would teleport the speaker and leave its dialogue box behind. Everyone on
    /// the car travels, players and entities
    /// alike; totality over bodies is how that is true.
    ///
    /// The one narrowing is a **class the engine's own furniture declares about
    /// itself**: `dw_fixture` means *my position IS engine state*. A bonfire's
    /// hitbox, a shortcut lever and a recovery stake's marker are places, not
    /// passengers, and carrying one does not move a thing — it rewrites a fact
    /// (for a stake, the ledger holds the marker's coordinates, and the next tick
    /// retires a marker nobody has a wager at). Places whose cell is known at
    /// compile time are refused outright instead, because the author can move
    /// them (`DW0542`); places the runtime puts down are excluded by the selector,
    /// because nobody can (`DW0545`). Nothing an author writes carries either tag,
    /// and no campaign JSON can turn either off.
    ///
    /// **A teleport is not a rescue.** Accumulated fall distance carries across
    /// one unchanged and is charged in full at the destination — measured Δ
    /// exactly `0.0000` in 46/46 trials on the pinned 1.21.11, including
    /// teleports 143 and 157 blocks straight *up*, with landing damage
    /// `floor(fall_distance) − 3` (`docs/notes/death-and-teleport-spike.md` §3).
    /// A platform that arrives under a falling player past ~20 blocks of fall is
    /// the surface they die on. The compiler does not try to reset the counter:
    /// what *does* reset it was explicitly not measured, and inventing a
    /// mechanism from recall is the folklore this project forbids.
    Teleport {
        /// The volume whose contents are moved, as an anchor-centred box
        /// (`anchor ± extent`) — the same shape a `begin-stealth` zone, a
        /// `damage-players` `in` filter and a `lethal_volumes[]` region take.
        ///
        /// Deliberately NOT a bare prefab `region` anchor, for the reason
        /// [`Verb::Collapse`] records: the assembled model clears every
        /// gate-region anchor's cells, so a volume described that way would
        /// delete the geometry it names.
        from: StealthZone,
        /// The destination mark (spec-0066). Resolved to a literal cell at build
        /// time, so the emitted `tp` carries absolute coordinates and no runtime
        /// search.
        to: Mark,
    },
    /// Fires a **firework rocket** from a mark (DSL v0.29, spec-0068).
    ///
    /// One effect at a point, the member of the same class as [`Verb::PlaySound`]
    /// — a one-shot thing that happens where the campaign says, beside the sound
    /// that goes with it. A display of many rockets is a [`Verb::Sequence`] of
    /// these, not a verb with timing of its own.
    ///
    /// # The burst height is a stated number, not a roll
    ///
    /// The emitter writes the entity's `LifeTime`
    /// ([`crate::firework::lifetime_ticks`]) rather than leaving it to the game,
    /// which randomises it at launch: two runs of one datapack would otherwise
    /// burst at two heights and nothing could be proven about where the burst is.
    /// Fixed at the floor of the game's range, the burst stands
    /// [`crate::firework::burst_height`] blocks over the mark.
    ///
    /// # A burst hurts, so the compiler asks where it is
    ///
    /// A build refuses a rocket whose column to that height is roofed, and one
    /// whose burst lies within [`crate::firework::BLAST_RADIUS`] blocks of a
    /// place the campaign posts a body (`DW0899`). Players are **not** posted:
    /// a player standing level with a burst takes up to
    /// [`crate::firework::worst_damage_hp`] HP, under a full body's twenty, and
    /// that is a hazard a player can see coming.
    Firework {
        /// The mark the rocket is launched from — the cell's centre, at the
        /// mark's own plane.
        at: Mark,
        /// Flight duration, 1–3 (the three the game crafts). Absent =
        /// [`crate::firework::MIN_FLIGHT`].
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(range(min = 1, max = 3))]
        flight: Option<u8>,
        /// One to seven bursts, in the order the component carries them.
        #[schemars(length(min = 1, max = 7))]
        explosions: Vec<FireworkExplosion>,
    },
    /// Spawns **particles** (spec-0085 §4.3) — the next one-shot point effect
    /// after [`Verb::Firework`], at a mark or at each addressed player.
    ///
    /// `particle` is a vanilla particle type id validated against the pinned
    /// registry `crates/dsl/data/particles-1.21.11.json`; an unknown id, or one
    /// whose type **takes options** (`dust`, `block`, `item`, …), is `DW0941`.
    ///
    /// Emitted as one vanilla `particle` command, always in **`force`** mode,
    /// whose viewers are the effect's audience: a particle a creator writes is
    /// meant to be seen, and `force` is the mode the game sends 512 blocks out
    /// and draws even at the client's Minimal particle setting. A
    /// `minecraft:elder_guardian` at `players` is the full-screen face.
    Particle {
        /// The particle type id (`minecraft:` prefix optional).
        particle: String,
        /// Where the particles spawn: a [`Mark`], or `players` — at each
        /// addressed player's own position.
        at: ParticleAt,
        /// How many (vanilla `<count>`, default 1). Zero is vanilla's spelling of
        /// a different thing — one particle with a velocity — and is refused.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(range(min = 1))]
        count: Option<u32>,
        /// Standard deviations `[x, y, z]` of the spawn spread, in blocks
        /// (vanilla `<delta>`, default `[0, 0, 0]`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        spread: Option<[f64; 3]>,
        /// Vanilla `<speed>` (default 0).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        speed: Option<f64>,
    },
    /// Strikes a **lightning bolt** at a mark (DSL v0.36, spec-0092) — the
    /// one-shot point effect beside [`Verb::Firework`] and [`Verb::Particle`].
    ///
    /// A real `minecraft:lightning_bolt`: every client in range draws the bolt
    /// and the sky flash and hears the thunder. It stands at the mark's cell
    /// centre on the mark's plane, so it strikes the block under the mark. A
    /// storm of strikes is a [`Verb::Sequence`] of these.
    ///
    /// # A bolt hurts, so the compiler asks where it lands
    ///
    /// The bolt hits every living body within the reach
    /// [`crate::lightning::REACH_HORIZONTAL`] / [`crate::lightning::REACH_BELOW`]
    /// / [`crate::lightning::REACH_ABOVE`] states, and turns a villager into a
    /// witch; a build refuses a strike in reach of a place the campaign posts a
    /// body (`DW0958`), and one whose struck block the game would rewrite — a
    /// lightning rod or weathering copper (`DW0959`). Players are **not**
    /// posted: a player in reach takes at most
    /// [`crate::lightning::worst_damage_hp`] HP. It lights no fire: every delve
    /// seals `fire_spread_radius_around_player` at 0 (spec-0092 §2.3).
    Lightning {
        /// The mark the bolt strikes — the cell's centre, at the mark's plane.
        at: Mark,
    },
}

/// Where a [`Verb::Particle`] spawns (spec-0085 §4.3): a mark, or the literal
/// `players`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum ParticleAt {
    /// `"players"` — at each addressed player's own position.
    Players(PlayersKeyword),
    /// A mark — the cell's centre at the mark's plane.
    Mark(Mark),
}

/// The literal `players`, the one keyword [`ParticleAt`] admits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum PlayersKeyword {
    /// At each addressed player.
    Players,
}

/// Default `grace_ticks` for [`Verb::BeginStealth`] (spec-0014).
fn default_grace_ticks() -> u32 {
    20
}

/// Default projectile for [`Verb::Volley`] (spec-0022).
pub const DEFAULT_VOLLEY_PROJECTILE: &str = "minecraft:arrow";
/// Default salvo count for [`Verb::Volley`] (spec-0022).
pub const DEFAULT_VOLLEY_SALVOS: u32 = 3;
/// Default ticks between salvos for [`Verb::Volley`] (spec-0022).
pub const DEFAULT_VOLLEY_INTERVAL: u32 = 10;
/// Largest admissible `salvos` — beyond this a volley is an entity-count
/// hazard rather than a trap (`DW0443`).
pub const MAX_VOLLEY_SALVOS: u32 = 16;
/// Largest admissible `interval` in ticks (`DW0443`): 10 seconds. A volley
/// slower than this is no longer one event the player reads as a trap.
pub const MAX_VOLLEY_INTERVAL: u32 = 200;
/// Default falling block for [`Verb::Collapse`] (spec-0022).
pub const DEFAULT_COLLAPSE_FALLING_BLOCK: &str = "minecraft:gravel";

/// Where a [`Verb::PlaySound`] originates (DSL v0.6, spec-0014). A sound
/// plays at fixed coordinates or at each listener's own position; the compiler
/// resolves no position for a live actor, so the `actor` variant is accepted by
/// the schema and rejected with `DW0335`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "at", rename_all = "kebab-case", deny_unknown_fields)]
pub enum SoundAt {
    /// Play the sound positioned at a resolved anchor, audible to all players.
    Anchor {
        /// The anchor the sound plays from.
        anchor: AnchorId,
        /// Integer `[x, y, z]` block offset from `anchor` (spec-0066, default
        /// `[0, 0, 0]`): the sound plays at the [`Mark`] the two fields spell.
        #[serde(default, skip_serializing_if = "is_zero3")]
        offset: [i32; 3],
    },
    /// Play the sound at each player's own position (the default), or at
    /// `offset` in **the listener's own frame** (spec-0085 §4.4): `+x` to the
    /// listener's left, `+y` up, `+z` the way the listener faces, with the pitch
    /// flattened so *behind* stays at ear height. `[0, 0, -3]` is three blocks
    /// behind. Integer blocks, as a [`Mark`]'s offset is.
    Players {
        /// Integer `[x, y, z]` offset in the listener's local frame (default
        /// `[0, 0, 0]`).
        #[serde(default, skip_serializing_if = "is_zero3")]
        offset: [i32; 3],
    },
    /// Play the sound at a scripted actor's position (rejected — `DW0335`; no
    /// actor position resolves at emission).
    Actor {
        /// The actor id (stage-5 `actors[]`).
        actor: String,
    },
}

/// The presentation channel for a [`Verb::Narrate`] (DSL v0.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum NarrateStyle {
    /// A chat line (default).
    Chat,
    /// A large on-screen title.
    Title,
    /// An on-screen subtitle.
    Subtitle,
    /// An "art" title rendered through the delve's custom resource-pack pixel-banner
    /// font (`delve:art`) so endings can flash blocky all-caps text (DSL v0.6,
    /// spec-0014). Text is checked at compile time against the font's glyph inventory
    /// (`DW0328`); characters outside it (e.g. non-Latin script) are rejected. It
    /// renders in the vanilla title slot, so it is width-checked like any title
    /// (`DW0330`) — roughly 15 glyphs fit on screen.
    Art,
    /// The **actionbar** — the one-line strip above the hotbar (DSL v0.11).
    ///
    /// This is the channel a *reply* uses: it does not interrupt, it does not
    /// stack, and it is overwritten by the next one. Every reply the compiler
    /// itself writes has always used it — a sealed gate's answer, a checkpoint
    /// return, the lobby's party count — but `narrate` could not reach it, which
    /// is the mechanical reason `close-gate.sealed_hint` could not have been an
    /// ordinary `narrate` even had someone tried (capability-ownership audit
    /// finding 3b). A channel is a property of the message, not of the verb that
    /// first wanted it.
    ///
    /// Unlike a title it is never width-checked: vanilla truncates nothing and
    /// draws it at GUI width, and a reply is a fragment rather than a banner.
    Actionbar,
}

impl NarrateStyle {
    /// The kebab tag (`chat` / `title` / `subtitle` / `art` / `actionbar`).
    pub fn token(self) -> &'static str {
        match self {
            NarrateStyle::Chat => "chat",
            NarrateStyle::Title => "title",
            NarrateStyle::Subtitle => "subtitle",
            NarrateStyle::Art => "art",
            NarrateStyle::Actionbar => "actionbar",
        }
    }
}

impl Verb {
    /// The kebab-case `type` tag this verb serializes as — the verb a
    /// diagnostic should name so the author can find it in the JSON.
    pub fn tag(&self) -> &'static str {
        match self {
            Verb::OpenGate { .. } => "open-gate",
            Verb::CloseGate { .. } => "close-gate",
            Verb::CampaignComplete { .. } => "campaign-complete",
            Verb::GiveItem { .. } => "give-item",
            Verb::SetFlag { .. } => "set-flag",
            Verb::SetState { .. } => "set-state",
            Verb::AddState { .. } => "add-state",
            Verb::ClearState { .. } => "clear-state",
            Verb::DropStake { .. } => "drop-stake",
            Verb::SpawnWave { .. } => "spawn-wave",
            Verb::Narrate { .. } => "narrate",
            Verb::SetBlock { .. } => "set-block",
            Verb::FillRegion { .. } => "fill-region",
            Verb::ClearRegion { .. } => "clear-region",
            Verb::SetAtmosphere { .. } => "set-atmosphere",
            Verb::OpenWay { .. } => "open-way",
            Verb::DespawnNpc { .. } => "despawn-npc",
            Verb::MoveNpc { .. } => "move-npc",
            Verb::Cutscene { .. } => "cutscene",
            Verb::SetTime { .. } => "set-time",
            Verb::SetWeather { .. } => "set-weather",
            Verb::PlaySound { .. } => "play-sound",
            Verb::DamagePlayers { .. } => "damage-players",
            Verb::SetCheckpoint { .. } => "set-checkpoint",
            Verb::Bonfire { .. } => "bonfire",
            Verb::BeginStealth { .. } => "begin-stealth",
            Verb::EndStealth => "end-stealth",
            Verb::SpawnNpc { .. } => "spawn-npc",
            Verb::SpawnActor { .. } => "spawn-actor",
            Verb::DespawnActor { .. } => "despawn-actor",
            Verb::MoveActor { .. } => "move-actor",
            Verb::UnleashActor { .. } => "unleash-actor",
            Verb::Sequence { .. } => "sequence",
            Verb::Volley { .. } => "volley",
            Verb::Collapse { .. } => "collapse",
            Verb::GiveEffect { .. } => "give-effect",
            Verb::ClearEffect { .. } => "clear-effect",
            Verb::Teleport { .. } => "teleport",
            Verb::Firework { .. } => "firework",
            Verb::SpawnAssembly { .. } => "spawn-assembly",
            Verb::DespawnAssembly { .. } => "despawn-assembly",
            Verb::PlayClip { .. } => "play-clip",
            Verb::ArmStrikes { .. } => "arm-strikes",
            Verb::Particle { .. } => "particle",
            Verb::Lightning { .. } => "lightning",
        }
    }

    /// **Whether the emitter addresses this verb to players** (spec-0085 §3.3) —
    /// whether its emitted commands name the effect's audience selector at all.
    ///
    /// A verb that answers `false` is a **party fact**: it fires once for the
    /// world (a flag, a gate, a block, a region, a wave, an actor, an NPC, a
    /// camera, the time, a checkpoint, a stealth beat, a timeline, a teleported
    /// volume, a rocket), so the envelope's `audience` and `in` have nothing to
    /// narrow and are refused on it (`DW0942`). A `player`-scoped state write
    /// answers `false` too: its holder is the acting player by declaration, never
    /// the audience.
    ///
    /// Exhaustive, so a new verb cannot be added without answering it; and
    /// `emit`'s own test binds this answer to the emitted bytes in both
    /// directions — every verb is emitted under two audiences, and its commands
    /// differ exactly when this says `true`.
    pub fn addresses_players(&self) -> bool {
        match self {
            Verb::GiveItem { .. }
            | Verb::Narrate { .. }
            | Verb::PlaySound { .. }
            | Verb::DamagePlayers { .. }
            | Verb::GiveEffect { .. }
            | Verb::ClearEffect { .. }
            | Verb::Particle { .. } => true,
            Verb::OpenGate { .. }
            | Verb::CloseGate { .. }
            | Verb::CampaignComplete { .. }
            | Verb::SetFlag { .. }
            | Verb::SetState { .. }
            | Verb::AddState { .. }
            | Verb::ClearState { .. }
            | Verb::DropStake { .. }
            | Verb::SpawnWave { .. }
            | Verb::SetBlock { .. }
            | Verb::FillRegion { .. }
            | Verb::ClearRegion { .. }
            | Verb::OpenWay { .. }
            | Verb::DespawnNpc { .. }
            | Verb::MoveNpc { .. }
            | Verb::Cutscene { .. }
            | Verb::SetTime { .. }
            | Verb::SetWeather { .. }
            | Verb::SetCheckpoint { .. }
            | Verb::Bonfire { .. }
            | Verb::BeginStealth { .. }
            | Verb::EndStealth
            | Verb::SpawnActor { .. }
            | Verb::DespawnActor { .. }
            | Verb::MoveActor { .. }
            | Verb::UnleashActor { .. }
            | Verb::SpawnNpc { .. }
            | Verb::Sequence { .. }
            | Verb::Volley { .. }
            | Verb::Collapse { .. }
            | Verb::Teleport { .. }
            | Verb::Firework { .. }
            // spec-0092: the thunder is the game's to send; the bolt is a world fact.
            | Verb::Lightning { .. }
            // spec-0080: a biome repaint is a world fact (`fillbiome`).
            | Verb::SetAtmosphere { .. }
            // spec-0082: an assembly is a world object.
            | Verb::SpawnAssembly { .. }
            | Verb::DespawnAssembly { .. }
            | Verb::PlayClip { .. }
            | Verb::ArmStrikes { .. } => false,
        }
    }
}
