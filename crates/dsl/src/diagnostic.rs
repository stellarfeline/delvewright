//! Diagnostics: the `--json` shape from spec-0002 and the stable `DW01xx` codes.

use serde::Serialize;

/// Diagnostic severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// A hard rejection.
    Error,
    /// Advisory. Reported and rendered like an error, but does **not** fail the
    /// run — `delvec` exits non-zero only on [`Severity::Error`]. Reserved for
    /// rules whose verdict depends on something the compiler cannot fully know
    /// (e.g. `DW0330`: how much text fits depends on the player's window size and
    /// GUI scale), where a hard rejection would be a guess dressed as a fact.
    Warning,
}

/// One diagnostic, serialized as one JSON object per line by `delvec --json`.
///
/// Field order matches spec-0002: `code`, `severity`, `stage`, `path`, `message`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    /// Stable machine code, e.g. `DW0101`.
    pub code: String,
    /// Severity.
    pub severity: Severity,
    /// The stage this diagnostic concerns (`world`, `npcs`, …), or empty.
    pub stage: String,
    /// JSON-pointer-ish location within the stage document.
    pub path: String,
    /// Human-readable explanation.
    pub message: String,
}

impl Diagnostic {
    /// Build an error diagnostic.
    pub fn error(
        code: &str,
        stage: impl Into<String>,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Diagnostic {
            code: code.to_string(),
            severity: Severity::Error,
            stage: stage.into(),
            path: path.into(),
            message: message.into(),
        }
    }

    /// Build a warning (advisory) diagnostic. Reported, but does not fail the run.
    pub fn warning(
        code: &str,
        stage: impl Into<String>,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Diagnostic {
            code: code.to_string(),
            severity: Severity::Warning,
            stage: stage.into(),
            path: path.into(),
            message: message.into(),
        }
    }
}

/// The stable validation diagnostic codes (see `crates/dsl/README.md`).
pub mod codes {
    /// Document does not conform to its stage schema (unknown field / wrong type).
    pub const SCHEMA: &str = "DW0100";
    /// Envelope `stage` does not match the document's slot.
    pub const STAGE_MISMATCH: &str = "DW0101";
    /// Unsupported `dsl_version`.
    pub const DSL_VERSION: &str = "DW0102";
    /// Inconsistent `campaign_id` across stages.
    pub const CAMPAIGN_ID_MISMATCH: &str = "DW0103";
    /// Malformed id syntax (kebab-case / prefix).
    pub const ID_SYNTAX: &str = "DW0110";
    /// Duplicate id within its namespace.
    pub const ID_DUPLICATE: &str = "DW0111";
    /// Dangling reference: an id ref does not resolve.
    pub const DANGLING_REF: &str = "DW0112";
    /// Stage-6 dialogue node unreachable from `root`.
    pub const DIALOGUE_UNREACHABLE: &str = "DW0120";
    /// Stage-6 dialogue `root`/`next` references an unknown node.
    pub const DIALOGUE_BAD_REF: &str = "DW0121";
    /// Stage-6 dialogue effect references an objective that is unknown, not a
    /// `talk-to`, or a `talk-to` on a different NPC (foreign effect).
    pub const DIALOGUE_BAD_OBJECTIVE: &str = "DW0122";
    /// A stage-5 `talk-to` objective has no reachable completing dialogue option
    /// (the static half of the compiler's `DW0203` deadlock guarantee).
    pub const DIALOGUE_UNCOVERED: &str = "DW0123";
    /// Quest dependency cycle.
    pub const PLAN_CYCLE: &str = "DW0130";
    /// `finale` is not a declared quest.
    pub const FINALE_UNKNOWN: &str = "DW0131";
    /// `finale` is not the convergent sink of the plan.
    pub const FINALE_UNREACHABLE: &str = "DW0132";
    /// Non-mandatory quest (reserved in v0).
    pub const NON_MANDATORY: &str = "DW0133";
    /// Objective `after` cycle.
    pub const AFTER_CYCLE: &str = "DW0140";
    /// Reserved feature used (reserved enum value or reserved field).
    pub const RESERVED: &str = "DW0141";
    /// Anchor not provided by the area's bound prefab.
    pub const ANCHOR_UNRESOLVED: &str = "DW0142";
    /// Item id not in the pinned 1.21.11 registry.
    pub const ITEM_UNKNOWN: &str = "DW0143";
    /// (spec-0021) An `equipment` or `loot` enchantment id is not in the pinned
    /// 1.21.11 enchantment registry.
    pub const ENCHANTMENT_UNKNOWN: &str = "DW0433";
    /// (spec-0021) An enchantment level is outside the 1..=255 range vanilla's
    /// `minecraft:enchantments` component can carry.
    pub const ENCHANTMENT_LEVEL: &str = "DW0434";
    /// (spec-0021) Two `loot` entries target the same anchor, so one would
    /// silently overwrite the other's contents.
    pub const LOOT_DUPLICATE_ANCHOR: &str = "DW0435";
    /// (spec-0021) A `loot` declaration carries more stacks than the container
    /// it fills has slots.
    pub const LOOT_TOO_MANY_ITEMS: &str = "DW0432";
    /// A single-slot fill's `count` exceeds the item's `minecraft:max_stack_size`
    /// in the pinned 1.21.11 registry. `item replace … container.<n> with <item>
    /// <count>` fails **silently** above the cap, shipping an empty slot.
    pub const ITEM_COUNT_OVER_STACK: &str = "DW0436";
    /// An `interact` declares `missing_item_hint` without a `requires_item`: the
    /// hint answers a gate that does not exist, so it could never narrate.
    pub const MISSING_ITEM_HINT_WITHOUT_ITEM: &str = "DW0437";
    /// Planned quest (stage 4) has no expansion in stage 5.
    pub const QUEST_NOT_EXPANDED: &str = "DW0150";
    /// Stage-5 quest is not planned in stage 4.
    pub const QUEST_NOT_PLANNED: &str = "DW0151";
    /// Stage-2 NPC has no stage-6 dialogue tree.
    pub const NPC_WITHOUT_TREE: &str = "DW0152";
    /// Stage-6 dialogue tree references an NPC not declared in stage 2.
    pub const TREE_WITHOUT_NPC: &str = "DW0153";
    /// Area binds neither or both of `prefab` / `prefab_pool` (exactly one
    /// required).
    pub const PREFAB_BINDING: &str = "DW0160";
    /// Area `prefab_pool` references a pool absent from `prefabs/` metadata.
    pub const POOL_UNKNOWN: &str = "DW0161";
    /// (v0.6, spec-0017) A stage-7 edit script is structurally invalid: an edit
    /// names a region no earlier `select` in its batch defined, a composition
    /// (`union`/`intersect`/`subtract`) lists too few regions, a box `min`
    /// exceeds `max` on an axis, a surface band's `from` exceeds `to`, a palette
    /// recipe is empty / carries a non-positive or non-finite weight or `scale`,
    /// a `matching` list is empty, or a morph `by`/`passes` is 0. (Unknown block
    /// ids in recipes reuse [`BLOCK_UNKNOWN`] / `DW0193`; id-syntax and
    /// duplicate-name violations reuse `DW0110`/`DW0111`.)
    pub const EDIT_INVALID: &str = "DW0162";
    /// (v0.3) A `kill` objective or `spawn-wave` effect references a `wave/<id>`
    /// not declared in the stage-5 `waves` section (dangling wave reference).
    pub const WAVE_UNKNOWN: &str = "DW0170";
    /// (v0.3) A declared wave is referenced by a `kill` objective but is never
    /// spawned by any `spawn-wave` effect (referenced-but-never-spawned). A wave
    /// must be spawned by some effect before its kill objective is reachable.
    pub const WAVE_NEVER_SPAWNED: &str = "DW0171";
    /// (v0.3) A `requires_flags` entry references a `flag/<id>` that no `set-flag`
    /// effect ever produces (dangling flag reference).
    pub const FLAG_UNKNOWN: &str = "DW0172";
    /// (spec-0016 §1) A wave declares `respawns_on_rest: true` but the campaign
    /// declares no `bonfire` — nothing can ever re-seat it, so the field is a
    /// silent no-op. Either add the bonfire the re-seat is meant to hang off, or
    /// drop the field.
    pub const REST_RESEAT_NO_BONFIRE: &str = "DW0370";
    /// (spec-0016 §1, owner ruling 2026-08-03) The campaign places a `bonfire`
    /// but no class kit declares a `flask`. Resting replenishes the flask to its
    /// declared count; with no flask the rest interaction's whole recovery half
    /// is a no-op and the souls loop has no consumable to spend, so this is a
    /// build error rather than a design choice.
    pub const BONFIRE_NO_FLASK: &str = "DW0476";
    /// (spec-0016 §1, owner directive 2026-08-03) A kit item's potion `contents`
    /// is not something 1.21.11 can pour: declared on an item that carries no
    /// `minecraft:potion_contents` component, empty (neither a named potion nor
    /// an effect), an unknown potion or status-effect id, an amplifier or
    /// duration outside the field vanilla stores it in, a lasting effect with no
    /// `duration`, an instantaneous one *with* a duration, or a malformed
    /// `color`.
    pub const KIT_POTION_INVALID: &str = "DW0486";
    /// (spec-0016 §1, owner directive 2026-08-03) A potion-bearing kit item
    /// declares no `contents` at `dsl_version` 0.8.0 — the Uncraftable Potion, a
    /// bottle that pours nothing. The placeholder flask, as a build error.
    pub const KIT_POTION_MISSING: &str = "DW0487";
    /// (task #179, owner ruling 2026-08-04) A `drops[]` `slot` entry does not
    /// name a distinct slot the same entity's `equipment` actually fills — the
    /// slot is empty, or the same slot is declared twice. A mob can only leave
    /// behind a piece it wears, and it can only leave it behind once.
    pub const DROP_SLOT_UNFILLED: &str = "DW0490";
    /// (task #179, owner ruling 2026-08-04) `drops[]` on an encounter that is
    /// not billed `elite` or `boss`. Only a named fight leaves anything behind;
    /// an ordinary mob's kit is never farmable (no-grind constitution), so the
    /// declaration is refused rather than silently making rank-and-file gear
    /// lootable.
    pub const DROP_NOT_TIERED: &str = "DW0491";
    /// (task #179) A `collect` `dropped_by` is not backed by the wave it names:
    /// the wave declares no `{item}` drop of this objective's item, the count
    /// asks for more copies than the wave's mobs can yield, or the objective
    /// also declares a `container` (the item cannot come out of a box *and* off
    /// a body).
    pub const DROP_COLLECT_UNSOURCED: &str = "DW0492";
    /// (task #179) A `collect` `dropped_by` is not ordered after the fight that
    /// produces it: no `kill` objective for that wave precedes this collect in
    /// the objective graph. Without that edge "kill the boss, take its key" is
    /// an authoring intention the quest graph cannot prove, and the collect
    /// reads as reachable from the campaign's first tick.
    pub const DROP_COLLECT_UNORDERED: &str = "DW0493";
    /// (spec-0031, DSL v0.10) A `lethal_volumes[]` entry's `message` is blank.
    ///
    /// The volume would still kill — and would kill in silence, which is the one
    /// thing the declaration exists to prevent. There is no compiler default that
    /// could be right for a cliff, a lava pit and an acid pool at once, so a blank
    /// wording is refused rather than papered over: a gate that reports green
    /// while the player learns nothing is exactly the vacuous pass CLAUDE.md names.
    pub const LETHAL_MESSAGE_BLANK: &str = "DW0512";
    /// (v0.3) A wave mob `entity` is not a known vanilla entity id. (Item-id
    /// checks for `collect.item`, `interact.requires_item` and `give-item.item`
    /// reuse [`ITEM_UNKNOWN`] / `DW0143`.)
    pub const ENTITY_UNKNOWN: &str = "DW0173";
    /// (i18n) An l10n sidecar does not correctly cover a declared language: the
    /// `l10n/<code>.json` file is absent, its envelope (`campaign_id` / `lang` /
    /// `dsl_version`) is inconsistent, or it is **missing** a key from the
    /// authoritative inventory (under-coverage). English (`en`) is implicit and
    /// never declared, so it is never checked.
    pub const L10N_MISSING: &str = "DW0180";
    /// (i18n) An l10n sidecar carries an **orphan** key that is not in the
    /// authoritative string inventory derived from the stage docs (over-coverage).
    pub const L10N_ORPHAN: &str = "DW0181";
    /// (i18n / harness oracle) A player-visible string — authored English or any
    /// sidecar translation — contains the reserved completion-marker sigil
    /// `[dw:complete`. That chat sequence is the validation bot's per-objective
    /// completion oracle; content carrying it could forge a passing critical-path
    /// step. The channel is reserved, not merely conventional.
    pub const MARKER_RESERVED: &str = "DW0182";
    /// (i18n v2) A player-visible string — authored English or any sidecar
    /// translation — contains a character from the reserved private-use block the
    /// compiler uses to carry an l10n key from the stage docs to the text
    /// component it is emitted into ([`crate::l10n::TR_SIGIL`]). Content carrying
    /// it could impersonate a translation tag, or survive into the datapack and
    /// render as a tofu box. The block is reserved, not merely conventional.
    pub const TR_SIGIL_RESERVED: &str = "DW0183";
    /// (i18n v2) A declared language has no entry in the Minecraft language-code
    /// mapping table ([`crate::l10n::mc_lang_code`]), so the resource pack has no
    /// filename to write its `assets/delvewright/lang/<code>.json` under. A
    /// language is never silently dropped: either the code is corrected to a
    /// mapped one, or the table gains the entry.
    pub const LANG_CODE_UNMAPPED: &str = "DW0184";
    /// (i18n v2) A campaign l10n sidecar defines a key in the reserved
    /// `delvewright.` **chrome** namespace ([`crate::chrome`]). Those are the
    /// engine's own on-screen strings — `New objective: `, `Choose your class`,
    /// the default a bonfire shows — owned by the compiler, translated with it,
    /// and authored by no campaign; a sidecar row under that prefix would be
    /// written into the language file and silently replace product chrome for that
    /// language. The namespace is reserved, not merely conventional.
    pub const CHROME_RESERVED: &str = "DW0186";
    /// (i18n v2) An l10n sidecar row was translated from English the campaign no
    /// longer holds: its `source` entry differs from the key's canonical English.
    /// The translation is present, applied and **wrong**, and no key-set check can
    /// see it — `DW0180`/`DW0181` compare key SETS, and a rewritten line moves no
    /// key. Load-bearing for entity display names, whose key belongs to the first
    /// site declaring a given text, so renaming one body can migrate a key to
    /// another body and the row that goes stale is not the one the author edited.
    pub const L10N_STALE: &str = "DW0187";
    /// (i18n v2) An l10n sidecar records provenance for only some of its rows (or
    /// none), so `DW0187` cannot see the rest. A warning, not an error: the
    /// `source` map is additive, and this is the one-version deprecation window
    /// before it is required. It states the unguarded row count, so an
    /// unadopted sidecar is a reported number on every run rather than silence
    /// that reads like a pass.
    pub const L10N_PROVENANCE_MISSING: &str = "DW0188";
    /// (v0.4) A mannequin NPC `skin.texture_id` is malformed (not a bare kebab
    /// token) or duplicated across NPCs (spec-0009). A missing `model` is a
    /// schema error (`DW0100`); a missing PNG is a build error (`DW0309`).
    pub const SKIN_INVALID: &str = "DW0190";
    /// (v0.4) A `talk-to` objective has no **ungated** reachable completing
    /// dialogue option — every completing option is `requires_flags`-gated, so
    /// the objective can deadlock the moment it activates (spec-0008 §1). Keep at
    /// least one ungated completing path.
    pub const DIALOGUE_FLAG_DEADLOCK: &str = "DW0191";
    /// (v0.4) A wave mob `effects[].effect` is not a known 1.21.11 effect id.
    pub const EFFECT_UNKNOWN: &str = "DW0192";
    /// (v0.4) A `set-block` / `interact.prop` block id is not a known 1.21.11
    /// block id.
    pub const BLOCK_UNKNOWN: &str = "DW0193";
    /// (v0.4) An environment trigger id is malformed (`DW0110`-style) or
    /// duplicated within the stage-5 `triggers` namespace.
    pub const TRIGGER_INVALID: &str = "DW0194";
    /// (v0.4) A dialogue `talk-to` or `interact` objective targets an NPC after a
    /// `despawn-npc` removes it on a reachable path (spec-0008 §5).
    pub const NPC_DESPAWNED_REF: &str = "DW0195";
    /// (v0.5) An area `lighting.min_light` is out of the 1..=14 range (spec-0010).
    pub const LIGHTING_RANGE: &str = "DW0196";
    /// (v0.6) A stage-2 NPC declares `deferred: true` but **no** `spawn-npc` effect
    /// anywhere in the campaign ever summons it — the NPC never enters the world,
    /// so its dialogue tree and any `talk-to` on it are unreachable content. The
    /// NPC-lifecycle dual of [`NPC_DESPAWNED_REF`] / `DW0195`.
    ///
    /// (0197/0198 were *reserved* by spec-0011's draft and released when that spec
    /// renumbered to `DW0340`/`DW0341`; they were never emitted by any code.)
    pub const NPC_NEVER_SPAWNED: &str = "DW0197";
    /// (v0.6) A `talk-to` on a `deferred` NPC activates before the NPC can exist:
    /// every `spawn-npc` for it sits in a quest that is a strict *descendant* of the
    /// objective's quest on the stage-4 DAG (and none fires from a trigger or
    /// dialogue), so the objective provably activates on an empty anchor.
    pub const NPC_SPAWNED_LATE: &str = "DW0198";
    /// (v0.6) A `cutscene` effect's shape is invalid: it mixes the multi-shot
    /// `shots` list with the single-shot `path`/`seconds` fields, gives neither,
    /// or declares a shot with an empty camera `path`. A cutscene must resolve to
    /// at least one shot, and every shot to at least one camera position.
    pub const CUTSCENE_SHAPE: &str = "DW0199";

    /// (v0.6) `horizon: "ocean"` declared without a `boundary` (spec-0013):
    /// validation-tier (exit 1). An infinite swimmable sea with no return rule is
    /// an authoring error. Grouped in the DW032x world/region family by domain;
    /// unlike the compiler-tier DW030x geometry codes it is raised at DSL
    /// validation, so it exits 1.
    pub const OCEAN_NO_BOUNDARY: &str = "DW0320";
    /// (v0.6) `boundary.margin` outside the `0..=64` range (spec-0013):
    /// validation-tier (exit 1).
    pub const BOUNDARY_MARGIN: &str = "DW0321";
    /// (v0.6) A `sequence` effect is nested inside another `sequence` (directly, or
    /// reachable via a nested `move-actor` `on_arrive`) — timelines do not recurse
    /// (spec-0014). Flatten the inner steps into the outer timeline.
    pub const NESTED_SEQUENCE: &str = "DW0329";

    /// (v0.6) Trap declaration structurally invalid (spec-0011): a malformed or
    /// duplicated `trap/<id>`, an `at`/`disarm.via` that no area's prefab provides,
    /// or a trap whose `disarm.via` collides with its own trigger anchor.
    /// Validation-tier (exit 1). Renumbered off the spec's stale reserved number
    /// (0197 — since taken).
    pub const TRAP_INVALID: &str = "DW0340";
    /// (spec-0016 §2) A `shortcut` declaration is structurally invalid: a
    /// malformed or duplicate `shortcut/<id>`, a `gate`/`unlock` anchor no area's
    /// prefab provides, or a `gate` that IS the `unlock` (the mechanism must sit
    /// on the far side, not in the doorway).
    pub const SHORTCUT_INVALID: &str = "DW0371";
    /// (spec-0016 §2) A `close-gate` effect targets a gate a `shortcut` owns.
    /// A shortcut opens **permanently** — that is the whole pattern — so its
    /// permanence is structural: there is no verb that can put it back. Use a
    /// different gate for the point-of-no-return beat.
    pub const SHORTCUT_RESEALED: &str = "DW0372";
    /// (spec-0016 §3) An `ambush` declaration is structurally invalid: a
    /// malformed or duplicate `ambush/<id>`, an empty `actors` list (an ambush
    /// that ambushes nobody), or the same actor listed twice (the second
    /// `spawn-actor` is a guarded no-op, so the author's intent silently halves).
    /// The telegraph is deliberately NOT required — an un-telegraphed ambush is
    /// core souls vocabulary (owner ruling 2026-08-02).
    pub const AMBUSH_INVALID: &str = "DW0375";
    /// (spec-0016 §4) A `timed-gate` declaration is structurally invalid: a
    /// malformed or duplicate `timed-gate/<id>`, an `open_ticks` or
    /// `closed_ticks` of 0 (a gate that never opens, or never closes — neither is
    /// a timing gate), a `phase` at or beyond the full cycle, or a gate another
    /// `timed-gate` or a `shortcut` already owns (two clocks fighting over one
    /// region, or a clock fighting a permanent open), or a `disarm.via` anchor no
    /// area's prefab provides / one that IS the gate anchor (the jam lever cannot
    /// live inside the span it stops).
    pub const TIMED_GATE_INVALID: &str = "DW0377";
    /// (task #184) A `close-gate` effect targets the gate of a `timed-gate` that
    /// declares a `disarm`. A disarm suppresses the clock **permanently with the
    /// gate resting open** — a jammed portcullis stays up — so, exactly like a
    /// `shortcut` (`DW0372`), its permanence is structural: there is no verb that
    /// can re-arm it. Use a different gate for the beat that must re-seal, or drop
    /// the `disarm`.
    pub const TIMED_GATE_REARMED: &str = "DW0389";
    /// (spec-0016 §6) A wave's TD `lane` / `summon` declaration is structurally
    /// invalid or internally contradictory: an empty `waypoints` list, a
    /// waypoint anchor no area's prefab provides, a repeated consecutive
    /// waypoint, an `aggro_radius` outside `4..=64`, a mob whose
    /// `attributes.follow_range` disagrees with `aggro_radius` (they MUST be
    /// equal — a patrolling raider holds ground against a target it cannot
    /// engage), or `lane` together with `summon: aggro-edge` (a lane IS the
    /// routing; aggro-edge is its opposite).
    pub const LANE_INVALID: &str = "DW0381";
    /// (spec-0016 §6) A lane wave contains a non-raider species. `Patrolling` /
    /// `patrol_target` are Raider NBT: on anything else they are dropped and the
    /// mob simply stands where it spawned. Live-verified marching: pillager,
    /// vindicator, evoker, ravager, witch. Non-raiders use
    /// `summon: aggro-edge` instead.
    pub const LANE_NOT_RAIDER: &str = "DW0382";
    /// (spec-0016 §6) A lane wave fields fewer than 2 mobs. A lone patroller
    /// sets `Patrolling:0b` on itself when it finds no companion within its
    /// follow range (vanilla), so a one-mob lane cancels itself.
    pub const LANE_SQUAD_TOO_SMALL: &str = "DW0383";
    /// (spec-0016 §6) A lane `pillager` is not holding a crossbow. Its only
    /// attack goal is the crossbow goal, so a pillager that acquires a target it
    /// has no runnable attack for freezes in place indefinitely — patrol blocked
    /// by the target, nothing to run instead (live-verified deadlock).
    pub const LANE_UNARMED: &str = "DW0384";
    /// (spec-0016 §6) A `summon: aggro-edge` wave mob declares no
    /// `attributes.follow_range`. That radius IS the summon ring — the distance
    /// at which the mob perceives the party — so it is authored, never guessed
    /// from a vanilla defaults table the compiler cannot verify.
    pub const AGGRO_EDGE_NO_RANGE: &str = "DW0385";
    /// (v0.6) A trap dispense-payload item id is not in the pinned 1.21.11 registry
    /// (spec-0011; mirrors `DW0143`). Validation-tier (exit 1). Renumbered off the
    /// spec's stale reserved number (0198 — since taken).
    pub const TRAP_PAYLOAD_UNKNOWN: &str = "DW0341";

    /// (spec-0022) A trap declares **no consequence at all**: neither the legacy
    /// redstone `effect` nor a command `payload`. A trap that does nothing is
    /// mute hardware the completability proofs would nonetheless reason about,
    /// so it is a content mistake, not a no-op. Validation-tier (exit 1).
    pub const TRAP_NO_CONSEQUENCE: &str = "DW0440";
    /// (spec-0022) A `volley` `projectile` / `collapse` `falling_block` /
    /// `then_floor` id is not in the pinned 1.21.11 registry (a `projectile`
    /// must be an ENTITY id, the collapse blocks BLOCK ids).
    /// Validation-tier (exit 1).
    pub const TRAP_VERB_ID_UNKNOWN: &str = "DW0441";
    /// (spec-0022) A `volley`'s `salvos` / `interval` is out of range (`salvos`
    /// in `1..=16`, `interval` in `1..=200`). A volley fires its whole kill zone
    /// every salvo, so the entity count is `salvos x cells`; and salvos spread
    /// wider than the interval cap stop reading as one trap event.
    /// Validation-tier (exit 1).
    pub const VOLLEY_CADENCE: &str = "DW0443";

    /// (v0.6) A `shot_style` declaration is semantically invalid (spec-0015 shot
    /// grammar): a styled shot with no `subject`; style-only fields (`subject`,
    /// `subject_b`, `dist`, `degrees`, `bearing`) on an unstyled shot; a
    /// `subject_b` on a style other than `two-shot` (or a `two-shot` without
    /// one); `degrees` off `orbit-arc` or outside `45..=120`; `dist` outside
    /// `1..=48`; or `bearing` outside `-360..=360`. Validation-tier (exit 1).
    pub const SHOT_STYLE_INVALID: &str = "DW0348";
    /// (v0.6) A `side-track` / `low-follow` shot whose subject has no
    /// compiler-known motion: those styles dolly *with* a moving subject, so the
    /// subject must be an NPC/actor with a matching `move-npc`/`move-actor` in
    /// the same effect group or the same `sequence` timeline (an `anchor`
    /// subject can never move). Validation-tier (exit 1). Use `locked-off` /
    /// `push-in` for a static subject instead.
    pub const SHOT_SUBJECT_UNMOVED: &str = "DW0349";

    /// (v0.4, added round-6) A `use` trigger anchored where an NPC stands.
    /// Right-click on an NPC already belongs to its dialogue advancement; a
    /// second interaction hitbox in the same cell makes the client's entity
    /// ray-pick ambiguous, and whichever entity loses the tie is silently dead
    /// — the round-6 island soft-lock class (an exactly co-located hitbox
    /// starved the giant's dialogue of every right-click). `strike` triggers
    /// are exempt: a left-click has no dialogue meaning, so the compiler rides
    /// the trigger's tag on the NPC's own hitbox instead of summoning a second
    /// one. Validation-tier (exit 1).
    pub const USE_TRIGGER_ON_NPC: &str = "DW0350";

    /// (v0.6, spec-0018) `world.min_players` outside the `1..=4` range. A delve is
    /// played by ONE party of 1–4 (ADR/CLAUDE.md product definition), so a declared
    /// mandatory party size can never sit outside it. Validation-tier (exit 1).
    pub const PARTY_SIZE: &str = "DW0356";
    /// (v0.6, spec-0018) A `carrier: "one"` `give-item` sits in a bundle that is
    /// only ever reached from the **scheduler** (`move-npc`/`move-actor`
    /// `on_arrive`, a `sequence` step). `carrier: "one"` means "hand this single
    /// quest prop to the player whose action earned it"; a scheduled bundle runs
    /// with the server command source and has no acting player, so there is no
    /// defensible recipient. Give it to the whole party (drop `carrier`), or move
    /// the hand-off onto the beat that a player completes. Validation-tier (exit 1).
    pub const PARTY_CARRIER_SCHEDULED: &str = "DW0357";

    /// (v0.6, owner ruling 2026-08-03) `world.difficulty` is `peaceful`. On
    /// peaceful the server discards every hostile-category mob as it is ticked —
    /// `/summon`ed, `NoAI`, `PersistenceRequired`, all of it — so a peaceful delve
    /// is one in which every wave, every hostile actor and every ambush silently
    /// ceases to exist. There is no delve that wants that, so the keyword is
    /// refused rather than honoured. Validation-tier (exit 1).
    pub const DIFFICULTY_INVALID: &str = "DW0468";
    /// (v0.6, owner ruling 2026-08-03) A campaign fields scripted `actors[]` (an
    /// ambush desugars into these too) but **no** `waves[]` and no declared
    /// `world.difficulty`, so the compiler's historical derivation ships
    /// `difficulty=peaceful` — under which every one of those actors that is a
    /// hostile species is discarded on the tick it spawns. The compiler cannot
    /// decide the question for the author: the pinned entity registry is a
    /// membership set with no mob-category data, so "is this actor a monster" is
    /// not something it can verify rather than guess. Advisory (warning,
    /// exit 0) — declaring `world.difficulty` settles it either way.
    pub const DIFFICULTY_UNDECLARED_ACTORS: &str = "DW0469";
    /// (spec-0016 §1, spec-0023, souls ruling 5/7: "stage bosses never respawn
    /// on rest") A wave declares BOTH `tier: boss` and `respawns_on_rest: true`.
    /// `tier` and `respawns_on_rest` are two fields on the same [`Wave`]
    /// declaration — the only place a "boss" billing and a "re-seat on rest"
    /// contract can land on one another; an [`Actor`] carries `tier` too but has
    /// no `respawns_on_rest` field at all (it is killed by hand, never re-seated
    /// by a bonfire), so this is the sole structurally expressible violation of
    /// the ruling. A rest-respawning boss re-fight breaks the retry economy the
    /// ruling exists to protect: a boss is the campaign's named fight, not
    /// trash pressure the party grinds back down every rest. Validation-tier
    /// (exit 1), `dsl::validate`. Prescription: drop `tier: boss` if the
    /// encounter really is meant to re-seat (bill it `elite` instead), or drop
    /// `respawns_on_rest` if it really is the boss.
    ///
    /// [`Wave`]: crate::stages::Wave
    /// [`Actor`]: crate::stages::Actor
    pub const BOSS_RESPAWNS_ON_REST: &str = "DW0499";

    // -- DSL v0.10 runtime state (spec-0031) ---------------------------------

    /// (v0.10, spec-0031) A `state/<kebab>` reference — in a `requires_state`
    /// comparison or in a `set-state`/`add-state`/`clear-state` verb — names a
    /// datum the campaign never declares in the stage-5 `state` list. Unlike a
    /// flag, a datum IS declared: its scope and its initial value are facts no
    /// use site can supply, so an undeclared reference is not "a datum that
    /// happens to start at zero", it is a datum with no defined multiplayer
    /// semantics at all. Validation-tier (exit 1). Prescription: declare it, or
    /// fix the id.
    pub const STATE_UNDECLARED: &str = "DW0500";
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
    pub const STATE_NEVER_WRITTEN: &str = "DW0501";
    /// (v0.10, spec-0031) A declared datum that **no gate anywhere in the
    /// campaign ever reads**. Either some verb writes it and nothing ever asks
    /// (the write is inert — a counter nobody consults), or nothing touches it at
    /// all (a dead declaration). Runtime state exists to be compared against; a
    /// datum with no reader is bookkeeping no player can ever observe.
    /// Validation-tier (exit 1). Prescription: gate something on it with
    /// `requires_state`, or delete the declaration and its writes.
    pub const STATE_NEVER_READ: &str = "DW0502";
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
    pub const STATE_SCOPE_UNREACHABLE: &str = "DW0503";
    /// (v0.10, spec-0032) A `stakes[]` declaration is unusable as a personal
    /// wager: its `state` is a datum the campaign never declares, or one declared
    /// `party`-scoped.
    ///
    /// **The scope half is the multiplayer decision most likely to be made by
    /// accident** (spec-0032, stated for correction rather than left to emerge).
    /// A stake is one player's loss and one player's chance to get it back; a
    /// party-shared purse would turn a teammate's death into a penalty on
    /// everyone, and nothing in the JSON would say so. Validation-tier (exit 1).
    /// Prescription: declare the datum `player`-scoped, or point the stake at a
    /// datum that is.
    pub const STAKE_STATE_SCOPE: &str = "DW0520";
    /// (v0.10, spec-0032) A `drop-stake` effect names a stake the campaign never
    /// declares in the stage-5 `stakes` list. Validation-tier (exit 1).
    /// Prescription: declare it, or fix the id.
    pub const STAKE_UNDECLARED: &str = "DW0521";
    /// (v0.10, spec-0032) A declared stake that **no `drop-stake` effect anywhere
    /// in the campaign ever leaves**. The retention policy, the forfeit rule and
    /// the whole placement table are computed for a mechanism no beat can fire —
    /// a declaration wearing a feature's clothes.
    ///
    /// The same vacuity rule `DW0502` states for a datum with no reader
    /// (CLAUDE.md: *a green gate that binds to nothing is vacuous, not a pass*).
    /// Validation-tier (exit 1). Prescription: drop it from a beat — `on_death`
    /// is the usual one — or delete the declaration.
    pub const STAKE_NEVER_DROPPED: &str = "DW0522";
    /// (v0.10, spec-0032) A `shops[].offers[]` entry that cannot deliver
    /// anything: it declares no `effects`, so its button is drawn, is pressable,
    /// and does nothing.
    ///
    /// The shop analogue of the invisible-affordance rule: a control the player
    /// can operate must have an observable answer. A refusal counts — an offer
    /// whose only effect is a gated `narrate` saying "you cannot afford that" is
    /// exactly the authored shape spec-0032 asks for. Validation-tier (exit 1).
    /// Prescription: give the offer effects, or delete it.
    pub const SHOP_OFFER_INERT: &str = "DW0523";
    /// (v0.10, spec-0032) A `forfeit` of kind `proportion` whose `percent` is
    /// above 100 — a death that takes more than the whole purse. Validation-tier
    /// (exit 1). Prescription: 0–100, or use `all`.
    pub const STAKE_FORFEIT_RANGE: &str = "DW0524";
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
    pub const STATE_READ_AFTER_WRITE: &str = "DW0527";
}
