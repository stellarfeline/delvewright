//! Native i18n: author-declared languages, l10n sidecar documents, the
//! authoritative key inventory, and the localization pass (spec-0001 i18n
//! addendum).
//!
//! **English is canonical.** Stage docs stay pure English; the strings the
//! compiler emits to players come from the stage docs. A campaign that declares
//! `world.languages = ["<code>", …]` must ship one `l10n/<code>.json` sidecar per
//! declared language, each a flat map of **stable key → translated string**.
//!
//! The **key inventory** ([`inventory`]) is derived deterministically from the
//! stage docs; it is the single source of truth for both coverage validation
//! ([`validate_l10n`]) and the build-time swap ([`localize`]). Both walk the exact
//! same traversal ([`each_string`]), so a key can never be checked but not applied
//! (or vice-versa).
//!
//! ## Key scheme (stable, path-derived, collision-free)
//!
//! Keys are dotted paths built from the local part of each DSL id (the segment
//! after `<prefix>/`, kebab preserved). Ids are unique within their namespace, so
//! every key is unique.
//!
//! | Key | Source string |
//! |-----|---------------|
//! | `world.title` | stage-1 `content.title` |
//! | `area.<area>.name` | each stage-1 area `name` |
//! | `class.<class>.name` / `.blurb` | each stage-3 class |
//! | `class.<class>.kit.<i>.name` | a kit item's display `name` (only if set) |
//! | `npc.<npc>.name` | each stage-2 NPC `name` (see *entity display names* below) |
//! | `actor.<actor>.name` | each stage-5 actor `name` (v0.6, only if set; see below) |
//! | `quest.<quest>.goal` | each stage-4 planned-quest `goal` |
//! | `obj.<quest>.<obj>.title` / `.hint` | a stage-5 objective's `title`/`hint` (only if set) |
//! | `obj.<quest>.<obj>.missing_item_hint` | a stage-5 `interact`'s `missing_item_hint` (v0.7, only if set) |
//! | `obj.<quest>.<obj>.item_name` | a stage-5 `collect`'s `item_name` (v0.8, only if set) |
//! | `dlg.<npc>.<node>.text` | each stage-6 dialogue node `text` |
//! | `dlg.<npc>.<node>.opt.<i>.label` | each dialogue option `label` |
//! | `dlg.<npc>.<node>.opt.<i>.tooltip` | that option's hover `tooltip` (v0.8, only if set) |
//! | `wave.<wave>.mob.<i>.name` | a wave mob's custom `name` (only if set) |
//! | `wave.<wave>.mob.<i>.drop.<n>.name` | a declared quest-item drop's display `name` (v0.9, only if set) |
//! | `actor.<actor>.drop.<n>.name` | an actor's declared quest-item drop `name` (v0.9, only if set) |
//! | `fx.…​.narrate` / `fx.…​.give` | a `narrate` line / named `give-item` in an effect list |
//! | `fx.…​.rest_prompt` / `.rest_label` / `.save_label` | a `bonfire`'s authored rest-dialog strings (v0.8, only if set) |
//! | `fx.…​.sealed_hint` | a `close-gate`'s authored answer to a right-click on the seal (v0.8, only if set) |
//! | `lethal.<volume>.message` | a stage-5 lethal volume's death wording (v0.10) |
//! | `state.<datum>.name` | a runtime datum's player-visible name — a currency (v0.10) |
//! | `shop.<shop>.title` | a stage-5 shop dialog's title (v0.10) |
//! | `shop.<shop>.offer.<i>.label` | a shop button's caption (v0.10) |
//! | `shop.<shop>.offer.<i>.tooltip` | a shop button's hover tooltip (v0.10) |
//! | `stake.<stake>.collected` | what collecting a recovery stake says (v0.10) |
//!
//! ## Nested effects (DSL v0.6)
//!
//! Effect strings nested inside a `sequence` step or a lifecycle bundle
//! (`on_respawn`/`on_caught`/`on_arrive`) are player-visible too, so they are
//! inventoried under **position-derived** child keys: the parent effect's `fx.…`
//! key, then a stable segment ([`crate::stages::QuestEffect::nested_effect_lists_keyed_mut`])
//! — `seq.<step>` for a sequence step, `respawn`/`caught`/`arrive` for the bundles —
//! then the effect's index in that list, then the leaf (`.narrate`/`.give`).
//! Example: a narrate in sequence step 1, effect 0 of `on_objective_complete`
//! effect 0 → `fx.<quest>.oc.<obj>.0.seq.1.0.narrate`. Nesting is arbitrary-depth
//! (a `move-actor.on_arrive` inside a `sequence` step nests both segments). Keys are
//! purely position-derived → deterministic and stable across builds (ADR-0006).
//!
//! ## Entity display names are keyed by their TEXT, not by their site
//!
//! An NPC (`npc.<npc>.name`) and a scripted actor (`actor.<actor>.name`) are two
//! DSL surfaces for the same thing a player reads: a nameplate over a body. One
//! character routinely occupies both — a stage-2 NPC that stands and talks, plus
//! one actor puppet per cutscene pose it is staged in. If each site owned its own
//! key, a translator would be asked for `Polyphemus` five times and could answer
//! differently each time, and the giant's name would **change as he walked into a
//! cutscene** — a worse defect than the untranslated one, and an authored one.
//!
//! So the key of an entity display name is decided by its **canonical English
//! text**: the first site (in this traversal's fixed order — NPCs before actors)
//! declaring a given name owns the key, and every later site carrying the
//! byte-identical name emits that same key. The inventory therefore asks for each
//! distinct name exactly once, and two bodies a player reads as one character
//! cannot render as two.
//!
//! Scope is deliberately the **entity display-name class only** (`npc.*.name`,
//! `actor.*.name`). Prose — a narrate line, a dialogue label, an objective title —
//! is context-bound and keeps one key per site: two English strings that happen to
//! coincide may legitimately need different renderings. Wave-mob names
//! (`wave.*.mob.*.name`) are the same shape and are **not** merged here: that is a
//! generalization beyond the finding this rule closes, and it is an owner call
//! because it retires keys live campaigns already translate.
//!
//! Player-visible strings only. Deliberately **excluded** (authoring context the
//! player never sees, so translating them is pointless and out of scope): world
//! `theme`/`premise`, NPC `persona` fields, persona `relationships`.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::diagnostic::{Binds, Diagnostic, codes};
use crate::envelope::{Campaign, Stage, is_supported_version, minor_ordinal};
use crate::ids::CampaignId;
use crate::stages::{NarrateStyle, QuestEffect};

/// Walk the player-visible strings of a single quest effect (DSL v0.4): a
/// `narrate` line and a named `give-item`'s display name. `keybase` is the
/// effect's stable position-derived key prefix.
fn effect_strings(
    eff: &mut QuestEffect,
    keybase: &str,
    entry: KeyEntry,
    f: &mut dyn FnMut(&str, &mut String, KeyEntry),
) {
    match eff {
        QuestEffect::Narrate { text, .. } => f(&format!("{keybase}.narrate"), text, entry),
        QuestEffect::GiveItem { name: Some(n), .. } => f(&format!("{keybase}.give"), n, entry),
        // spec-0016 §1: the bonfire's rest dialog is
        // read by the player like any other on-screen line, so its authored
        // strings translate like any other. Unauthored fields are absent from the
        // inventory — the compiler bakes its canonical English, exactly as
        // `world.boundary.message` does.
        QuestEffect::Bonfire {
            prompt,
            rest_label,
            save_label,
            ..
        } => {
            if let Some(p) = prompt.as_mut() {
                f(&format!("{keybase}.rest_prompt"), p, entry);
            }
            if let Some(r) = rest_label.as_mut() {
                f(&format!("{keybase}.rest_label"), r, entry);
            }
            if let Some(s) = save_label.as_mut() {
                f(&format!("{keybase}.save_label"), s, entry);
            }
        }
        // DSL v0.8: what a sealed gate answers when the party right-clicks it. Read
        // off the actionbar exactly like a `narrate`, so it translates like one. An
        // unauthored hint is absent from the inventory — the compiler bakes its
        // canonical English, exactly as `world.boundary.message` does.
        QuestEffect::CloseGate {
            sealed_hint: Some(h),
            ..
        } => f(&format!("{keybase}.sealed_hint"), h, entry),
        _ => {}
    }
}

/// Walk the player-visible strings of `eff` **and every effect nested inside it**
/// (DSL v0.6): a `narrate`/`give-item` inside a `sequence` step or an
/// `on_respawn`/`on_caught`/`on_arrive` bundle is player-visible and must enter the
/// inventory (and be localized on the emission path), else it ships English-only in
/// a translated build. Child keys extend `keybase` with the effect's stable key
/// segment ([`QuestEffect::nested_effect_lists_keyed_mut`]) and the effect's index
/// within that list, e.g. `<keybase>.seq.<step>.<j>.narrate` for a narrate in
/// sequence step `<step>`, effect `<j>`. Deterministic and stable across builds.
fn effect_strings_deep(
    eff: &mut QuestEffect,
    keybase: &str,
    entry: KeyEntry,
    f: &mut dyn FnMut(&str, &mut String, KeyEntry),
) {
    effect_strings(eff, keybase, entry, f);
    for (seg, list) in eff.nested_effect_lists_keyed_mut() {
        for (j, inner) in list.iter_mut().enumerate() {
            effect_strings_deep(inner, &format!("{keybase}.{seg}.{j}"), entry, f);
        }
    }
}

/// The implicit, always-canonical language. Never appears in `world.languages`
/// and never has a sidecar; `delvec build --lang en` emits the pure-English delve.
pub const CANONICAL_LANG: &str = "en";

/// The reserved sigil opening the compiler's machine-readable **completion-marker**
/// channel — `[dw:complete <campaign_id> <token>]`, the only evidence the
/// validation bot accepts that an objective (or the campaign) actually completed.
/// The channel rides chat, so any authored or translated player-visible string
/// carrying this sigil could forge a completion and make a critical-path step pass
/// hollow. [`validate_marker_channel`] (`DW0182`) reserves it, making the collision
/// structurally impossible instead of merely implausible.
pub const MARKER_SIGIL: &str = "[dw:complete";

/// Reserve the machine completion-marker channel (`DW0182`): no player-visible
/// string — authored English (the whole [`inventory`]) or any declared language's
/// sidecar rendition — may contain [`MARKER_SIGIL`]. Language-independent; runs on
/// every `validate` / `analyze` / `build`. Checks translations too: a translator
/// (LLM or human) copying the sigil through is exactly the forgery this closes.
pub fn validate_marker_channel(
    c: &Campaign,
    sidecars: &BTreeMap<String, L10nDoc>,
) -> Vec<Diagnostic> {
    let mut d = Vec::new();
    let mut flag = |where_: String, key: &str, text: &str| {
        if text.contains(MARKER_SIGIL) {
            d.push(Diagnostic::error(
                codes::MARKER_RESERVED,
                "l10n",
                where_,
                format!(
                    "player-visible string `{key}` contains the reserved completion-marker \
                     sigil `{MARKER_SIGIL}` — that chat sequence is the validation bot's \
                     completion oracle, and authored text carrying it could forge a passing \
                     critical-path step. Reword the line to drop `{MARKER_SIGIL}`"
                ),
            ));
        }
    };
    for (key, text) in inventory(c) {
        flag(format!("#/{key}"), &key, &text);
    }
    for (lang, doc) in sidecars {
        for (key, text) in &doc.content {
            flag(format!("l10n/{lang}.json#/content/{key}"), key, text);
        }
    }
    d
}

/// The kind marker on an l10n sidecar envelope (`"l10n"`), analogous to the stage
/// marker on a stage document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum L10nKind {
    /// A localization sidecar (`l10n/<code>.json`).
    L10n,
}

/// An l10n sidecar document: `{ dsl_version, campaign_id, kind: "l10n", lang,
/// content, source }`, mirroring the stage-doc envelope style. `content` is a flat
/// map of [inventory](crate::l10n::inventory) key → translated string; `source`
/// records the canonical English each of those translations was made **from**.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct L10nDoc {
    /// DSL version string (same versioning as the stage docs).
    pub dsl_version: String,
    /// Owning campaign id (must match the stage docs).
    pub campaign_id: CampaignId,
    /// Document kind marker (`l10n`).
    pub kind: L10nKind,
    /// The BCP-47-style language code this sidecar translates into (must match the
    /// `l10n/<code>.json` filename and appear in `world.languages`).
    pub lang: String,
    /// Flat map of inventory key → translated string.
    pub content: BTreeMap<String, String>,
    /// **Translation provenance**: inventory key → the canonical English that key
    /// held when its [`Self::content`] row was written.
    ///
    /// Coverage validation proves the sidecar has a row for every key
    /// (`DW0180`/`DW0181`), which is a statement about key SETS and says nothing
    /// about whether a row still corresponds to the English it renders. Edit an
    /// authored line and its translation is stale, present, applied, and wrong —
    /// and nothing in the key sets moved. `source` is what makes that
    /// **detectable** ([`validate_l10n_provenance`], `DW0187`) instead of audited.
    ///
    /// This is load-bearing for entity display names in particular, because their
    /// key is owned by the first site declaring a given text (see the module
    /// header): renaming ONE body can migrate a key's ownership to ANOTHER body,
    /// so the row that goes wrong is not the row the author touched.
    ///
    /// Optional in the format — an older sidecar parses unchanged and simply
    /// carries no provenance, which `DW0188` reports as an unguarded row count on
    /// every run rather than letting it pass in silence.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub source: BTreeMap<String, String>,
}

/// The local part of a type-prefixed id: the segment after the first `/` (kebab
/// preserved). `npc/keeper` → `keeper`. Ids without a `/` pass through unchanged.
fn local(id: &str) -> &str {
    id.split_once('/').map(|(_, r)| r).unwrap_or(id)
}

/// The local part of a type-prefixed DSL id, exactly as the key scheme derives it
/// (`npc/keeper` → `keeper`). Public so a consumer that pairs inventory keys back
/// with their source objects (`delvec l10n-inventory`, matching `dlg.<npc>.…` keys
/// to the NPC that speaks them) derives the same segment the keys are built from.
pub fn local_id(id: &str) -> &str {
    local(id)
}

/// **When an inventory key started being required**, and which stage document's
/// `dsl_version` decides whether a given campaign has reached that point.
///
/// This is [`Binds`] one level down, and it exists because an obligation reaches
/// an old campaign in two ways, only one of which is a per-diagnostic question. A
/// NEW check is fenced by its code (`DwCode::since`). But [`each_string`] widening
/// raises no new code at all: `DW0180` was always allowed to fire, and simply
/// started demanding more keys when the walk widened onto an actor's own `name`,
/// a v0.6 surface that older campaigns already emitted — which sends a shipped
/// campaign red mid-staging with nothing in its own documents changed.
///
/// So the binding is versioned at the granularity the binding has: **per key**.
/// A key whose entry version the campaign has not reached is still inventoried,
/// still tagged and still translated if a sidecar happens to carry it — nothing
/// about emission moves — but it is not something coverage may *demand*
/// ([`required_inventory`]).
///
/// The rule for choosing, same as [`Binds`]: could adding this site turn a
/// campaign red whose own documents did not change?
///
/// * **No** — the field is surface introduced at version N, so a campaign below
///   N cannot carry it at all (`DW0141`) and the key never appears. Declare
///   [`KeyEntry::always`]; the entry version is irrelevant because the key's
///   existence is already gated by the surface.
/// * **Yes** — the field existed in older campaigns and the walk is only NOW
///   inventorying it. Declare [`KeyEntry::since`] with the version current when
///   the widening lands, so existing campaigns adopt it in their own explicit
///   version round rather than on the next engine build.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEntry {
    /// The stage document whose `dsl_version` governs this key.
    pub stage: Stage,
    /// When the key started being required of a campaign.
    pub binds: Binds,
}

impl KeyEntry {
    /// This key has been inventoried for as long as its surface has existed, so
    /// its coverage obligation is exactly as old as the field itself.
    pub const fn always(stage: Stage) -> KeyEntry {
        KeyEntry {
            stage,
            binds: Binds::EveryVersion,
        }
    }

    /// This key was **added to the walk** at the given minor version over surface
    /// that already existed — so campaigns below it are grandfathered.
    pub const fn since(stage: Stage, minor: u32) -> KeyEntry {
        KeyEntry {
            stage,
            binds: Binds::Since(minor),
        }
    }

    /// Is `c` obliged to translate a key with this entry?
    fn required_by(self, versions: &StageVersions) -> bool {
        match self.binds {
            Binds::EveryVersion => true,
            Binds::Since(n) => versions.of(self.stage) >= n,
        }
    }
}

/// The six (or seven) declared `dsl_version` ordinals of one campaign, snapshotted
/// before the mutable walk borrows the campaign.
#[derive(Clone, Copy, Debug)]
struct StageVersions {
    world: u32,
    npcs: u32,
    classes: u32,
    quest_plan: u32,
    quests: u32,
    dialogue: u32,
    world_edits: u32,
    /// The spec-0049 map-pipeline documents. Neither carries a player-visible
    /// string — a node's `intent` and a fact's `note` are authoring prose a
    /// player never sees — so no inventory key is ever fenced at either of them.
    /// They are carried anyway rather than defaulted, because `of` is a total
    /// function over [`Stage`] and a stage with no answer would have to be a
    /// `_` arm, which is how the next stage's strings get silently unfenced.
    geometry_brief: u32,
    layout_graph: u32,
    site_plan: u32,
    detail_plan: u32,
}

impl StageVersions {
    fn of(self, stage: Stage) -> u32 {
        match stage {
            Stage::World => self.world,
            Stage::Npcs => self.npcs,
            Stage::Classes => self.classes,
            Stage::QuestPlan => self.quest_plan,
            Stage::Quests => self.quests,
            Stage::Dialogue => self.dialogue,
            Stage::WorldEdits => self.world_edits,
            Stage::GeometryBrief => self.geometry_brief,
            Stage::LayoutGraph => self.layout_graph,
            Stage::SitePlan => self.site_plan,
            Stage::DetailPlan => self.detail_plan,
        }
    }

    fn of_campaign(c: &Campaign) -> StageVersions {
        StageVersions {
            world: minor_ordinal(&c.world.dsl_version),
            npcs: minor_ordinal(&c.npcs.dsl_version),
            classes: minor_ordinal(&c.classes.dsl_version),
            quest_plan: minor_ordinal(&c.quest_plan.dsl_version),
            quests: minor_ordinal(&c.quests.dsl_version),
            dialogue: minor_ordinal(&c.dialogue.dsl_version),
            world_edits: c
                .world_edits
                .as_ref()
                .map(|w| minor_ordinal(&w.dsl_version))
                .unwrap_or(0),
            geometry_brief: c
                .geometry_brief
                .as_ref()
                .map(|g| minor_ordinal(&g.dsl_version))
                .unwrap_or(0),
            layout_graph: c
                .layout_graph
                .as_ref()
                .map(|g| minor_ordinal(&g.dsl_version))
                .unwrap_or(0),
            site_plan: c
                .site_plan
                .as_ref()
                .map(|g| minor_ordinal(&g.dsl_version))
                .unwrap_or(0),
            detail_plan: c
                .detail_plan
                .as_ref()
                .map(|g| minor_ordinal(&g.dsl_version))
                .unwrap_or(0),
        }
    }
}

/// **The one key in the walk whose obligation is younger than its surface.**
///
/// `actors[].name` is DSL v0.6 surface, and [`each_string`] reaches it —
/// correctly, a puppet's nameplate is as player-visible as an NPC's. Widened
/// with no version gate, that makes every campaign at every declared version owe
/// a translation for a string its own documents had not touched, on the next
/// engine build.
///
/// 0.10.0 is the version current at the widening, so that is the version
/// that owes it: campaigns at 0.10.0 and above translate actor nameplates,
/// campaigns below are grandfathered and adopt it with their own version bump,
/// which is what CLAUDE.md's version-adoption discipline asks for.
const ACTOR_NAME_ENTRY: KeyEntry = KeyEntry::since(Stage::Quests, 10);

/// The stage an effect root was authored in, as a [`Stage`].
fn effect_stage(stage: &str) -> Stage {
    match stage {
        "dialogue" => Stage::Dialogue,
        // Every other effect root — quest bundles, triggers, traps, `on_death` —
        // is authored in the stage-5 quests document.
        _ => Stage::Quests,
    }
}

/// Walk every player-visible string in `c` in a fixed, deterministic order,
/// invoking `f(key, &mut value, entry)` for each. The single traversal shared by
/// [`inventory`] and [`localize`] — they cannot drift.
///
/// The third argument is not decoration. Every site must state its [`KeyEntry`],
/// which is what stops a widening from silently creating a coverage obligation
/// on campaigns that predate it; there is no way to add a site without
/// answering the question, because there is no `f` that takes two arguments.
pub fn each_string(c: &mut Campaign, f: &mut dyn FnMut(&str, &mut String, KeyEntry)) {
    // Entity display names (a nameplate over a body) are keyed by their canonical
    // English TEXT, not by their declaration site: canonical English → owning key.
    // See the module header — one character routinely has one NPC identity and
    // several actor puppets, and a per-site key would let its name be translated
    // several ways. Filled in traversal order, so the NPC identity always owns the
    // key and the puppets follow it.
    let mut entity_names: BTreeMap<String, String> = BTreeMap::new();
    // Stage 1 — world title + area names.
    f(
        "world.title",
        &mut c.world.content.title,
        KeyEntry::always(Stage::World),
    );
    for area in &mut c.world.content.areas {
        let key = format!("area.{}.name", local(area.id.as_str()));
        f(&key, &mut area.name, KeyEntry::always(Stage::World));
    }
    // Stage 1 — boundary return message (v0.6, only when authored). The compiler's
    // default English is baked at emit time, so it is not inventoried; an authored
    // message is translated like every other player-facing string.
    if let Some(b) = c.world.content.boundary.as_mut()
        && let Some(msg) = b.message.as_mut()
    {
        f(
            "world.boundary.message",
            msg,
            KeyEntry::always(Stage::World),
        );
    }
    // Stage 1 — campaign outro (v0.6, only when authored): the closing line on the
    // completion advancement. Unauthored, the emitter falls back to the finale
    // quest's `goal`, which is inventoried in its own right — so the last sentence
    // of a delve is campaign-derived and translated either way.
    if let Some(outro) = c.world.content.outro.as_mut() {
        f("world.outro", outro, KeyEntry::always(Stage::World));
    }
    // Stage 3 — class names/blurbs + optional kit item display names.
    for class in &mut c.classes.content.classes {
        let cl = local(class.id.as_str()).to_string();
        f(
            &format!("class.{cl}.name"),
            &mut class.name,
            KeyEntry::always(Stage::Classes),
        );
        f(
            &format!("class.{cl}.blurb"),
            &mut class.blurb,
            KeyEntry::always(Stage::Classes),
        );
        for (i, item) in class.kit.iter_mut().enumerate() {
            if let Some(name) = item.name.as_mut() {
                f(
                    &format!("class.{cl}.kit.{i}.name"),
                    name,
                    KeyEntry::always(Stage::Classes),
                );
            }
        }
    }
    // Stage 2 — NPC names. First in the entity display-name class, so an NPC
    // identity owns the key every actor puppet portraying it shares.
    for npc in &mut c.npcs.content.npcs {
        let key = entity_name_key(
            &mut entity_names,
            &npc.name,
            format!("npc.{}.name", local(npc.id.as_str())),
        );
        f(&key, &mut npc.name, KeyEntry::always(Stage::Npcs));
    }
    // Stage 4 — quest goals.
    for q in &mut c.quest_plan.content.quests {
        let key = format!("quest.{}.goal", local(q.id.as_str()));
        f(&key, &mut q.goal, KeyEntry::always(Stage::QuestPlan));
    }
    // Stage 5 — objective titles/hints (when set).
    for q in &mut c.quests.content.quests {
        let ql = local(q.id.as_str()).to_string();
        for o in &mut q.objectives {
            let ol = local(o.id().as_str()).to_string();
            if let Some(title) = o.title_mut().as_mut() {
                f(
                    &format!("obj.{ql}.{ol}.title"),
                    title,
                    KeyEntry::always(Stage::Quests),
                );
            }
            if let Some(hint) = o.hint_mut().as_mut() {
                f(
                    &format!("obj.{ql}.{ol}.hint"),
                    hint,
                    KeyEntry::always(Stage::Quests),
                );
            }
            // Stage 5 — v0.7 `interact.missing_item_hint`: narrated in chat to the
            // player who clicks without the required item in hand, so it is as
            // player-visible as `hint` and translates like it. Absent on every
            // pre-0.7 objective → inventory unchanged.
            if let crate::stages::Objective::Interact {
                missing_item_hint: Some(m),
                ..
            } = o
            {
                f(
                    &format!("obj.{ql}.{ol}.missing_item_hint"),
                    m,
                    KeyEntry::always(Stage::Quests),
                );
            }
            // Stage 5 — v0.8 `collect.item_name`: the display name the
            // collected item carries as a `custom_name` component. A player reads
            // it off the stack in the barrel and off their own hotbar, so it is as
            // player-visible as a `title` and translates like one. Absent on every
            // pre-0.8 objective → inventory unchanged.
            if let crate::stages::Objective::Collect {
                item_name: Some(n), ..
            } = o
            {
                f(
                    &format!("obj.{ql}.{ol}.item_name"),
                    n,
                    KeyEntry::always(Stage::Quests),
                );
            }
        }
        // Stage 5 — v0.7 cast-ledger bark lines (spec-0020). Barks are spoken
        // in-game exactly like narrate text, so they translate like it too.
        // `doing` is deliberately NOT inventoried: it is authoring context for
        // the dialogue stage, never shown to a player. (`cast` is a BTreeMap and
        // the placement list is ordered, so the traversal stays deterministic.)
        for (npc, entry) in &mut q.cast {
            let np = local(npc.as_str()).to_string();
            for (b, p) in entry.placements_mut().into_iter().enumerate() {
                let Some(crate::stages::CastDialogue::Barks(pool)) = p.dialogue.as_mut() else {
                    continue;
                };
                for (i, line) in pool.barks.iter_mut().enumerate() {
                    f(
                        &format!("cast.{ql}.{np}.{b}.bark.{i}"),
                        line,
                        KeyEntry::always(Stage::Quests),
                    );
                }
            }
        }
    }
    // Stage 6 — dialogue node text + option labels.
    for tree in &mut c.dialogue.content.dialogues {
        let np = local(tree.npc.as_str()).to_string();
        for node in &mut tree.nodes {
            let nd = local(node.id.as_str()).to_string();
            f(
                &format!("dlg.{np}.{nd}.text"),
                &mut node.text,
                KeyEntry::always(Stage::Dialogue),
            );
            for (i, opt) in node.options.iter_mut().enumerate() {
                f(
                    &format!("dlg.{np}.{nd}.opt.{i}.label"),
                    &mut opt.label,
                    KeyEntry::always(Stage::Dialogue),
                );
                // v0.8: the button's hover tooltip. A player reads it exactly as
                // they read the caption, so it translates exactly like one; an
                // unauthored tooltip is absent from the inventory (no key, no
                // coverage obligation), like every other `only if set` string.
                if let Some(tip) = opt.tooltip.as_mut() {
                    f(
                        &format!("dlg.{np}.{nd}.opt.{i}.tooltip"),
                        tip,
                        KeyEntry::always(Stage::Dialogue),
                    );
                }
            }
        }
    }
    // Stage 5 — wave mob custom names (when set).
    for w in &mut c.quests.content.waves {
        let wl = local(w.id.as_str()).to_string();
        for (i, mob) in w.mobs.iter_mut().enumerate() {
            if let Some(name) = mob.name.as_mut() {
                f(
                    &format!("wave.{wl}.mob.{i}.name"),
                    name,
                    KeyEntry::always(Stage::Quests),
                );
            }
            // Stage 5 — v0.9 declared quest-item drops. The name
            // rides the dropped stack's `custom_name`, so the player reads it off
            // the ground and off their own hotbar: as player-visible as a wave
            // mob's own name, and translated like one.
            for (n, dr) in mob.drops.iter_mut().enumerate() {
                if let Some(name) = dr.name_mut() {
                    f(
                        &format!("wave.{wl}.mob.{i}.drop.{n}.name"),
                        name,
                        KeyEntry::always(Stage::Quests),
                    );
                }
            }
        }
    }
    // Stage 5 — actors: the nameplate over the puppet, then its v0.9 drops,
    // keyed off the actor id exactly as a wave mob's drop is keyed off its
    // wave.
    for a in &mut c.quests.content.actors {
        let al = local(a.id.as_str()).to_string();
        // The puppet's own name (v0.6 `actors[].name`). Player-visible in every
        // frame it stands in — a nameplate and, for a cutscene mannequin, the
        // label the party reads while the scene plays — so it is as translatable
        // as the stage-2 NPC name it usually duplicates, and shares that NPC's key
        // when the two texts are identical (module header).
        //
        // `ACTOR_NAME_ENTRY` carries the whole fence: the field is v0.6 but the
        // walk only reached it in the 0.10 era, so the coverage obligation is
        // 0.10's and not v0.6's.
        if let Some(name) = a.name.as_mut() {
            let key = entity_name_key(&mut entity_names, name, format!("actor.{al}.name"));
            f(&key, name, ACTOR_NAME_ENTRY);
        }
        for (n, dr) in a.drops.iter_mut().enumerate() {
            if let Some(name) = dr.name_mut() {
                f(
                    &format!("actor.{al}.drop.{n}.name"),
                    name,
                    KeyEntry::always(Stage::Quests),
                );
            }
        }
    }
    // Stage 5 — loot item custom names (spec-0021), keyed like a class kit
    // item's name so a named prop in a chest translates like any other.
    for l in &mut c.quests.content.loot {
        let ll = local(l.id.as_str()).to_string();
        for (i, item) in l.items.iter_mut().enumerate() {
            if let Some(name) = item.name.as_mut() {
                f(
                    &format!("loot.{ll}.item.{i}.name"),
                    name,
                    KeyEntry::always(Stage::Quests),
                );
            }
        }
    }
    // Stage 5 — lethal volumes (v0.10, spec-0031): the line the volume says as it
    // kills. As player-visible as a narrate, and read at the worst possible moment
    // to be reading a raw key, so it is inventoried like any other authored line.
    // Widening this reaches NO older campaign: only a 0.10.0 quests stage may
    // declare a lethal volume at all (`DW0141`), so the error-tier obligation this
    // module's `inventory` doc warns about cannot be created retroactively here.
    for v in &mut c.quests.content.lethal_volumes {
        let vl = local(v.id.as_str()).to_string();
        f(
            &format!("lethal.{vl}.message"),
            &mut v.message,
            KeyEntry::always(Stage::Quests),
        );
    }
    // Stage 5 — a runtime datum's player-visible name (v0.10, spec-0032). A named
    // datum is a currency: the engine states `<name>: <value>` on the holder's
    // action bar on every write, so the name is read as often as any narrate.
    // Widening this reaches no older campaign — only a 0.10.0 quests stage may
    // carry the field at all (`DW0141`).
    for st in &mut c.quests.content.state {
        let sl = local(st.id.as_str()).to_string();
        if let Some(name) = st.name.as_mut() {
            f(
                &format!("state.{sl}.name"),
                name,
                KeyEntry::always(Stage::Quests),
            );
        }
    }
    // Stage 5 — shops (v0.10, spec-0032): the dialog's title, and each button's
    // caption and tooltip. Keyed exactly as a dialogue node's label/tooltip are,
    // because they are the same two components of the same vanilla button codec.
    for sh in &mut c.quests.content.shops {
        let hl = local(sh.id.as_str()).to_string();
        f(
            &format!("shop.{hl}.title"),
            &mut sh.title,
            KeyEntry::always(Stage::Quests),
        );
        for (i, off) in sh.offers.iter_mut().enumerate() {
            f(
                &format!("shop.{hl}.offer.{i}.label"),
                &mut off.label,
                KeyEntry::always(Stage::Quests),
            );
            if let Some(t) = off.tooltip.as_mut() {
                f(
                    &format!("shop.{hl}.offer.{i}.tooltip"),
                    t,
                    KeyEntry::always(Stage::Quests),
                );
            }
        }
    }
    // Stage 5 — recovery stakes (v0.10, spec-0032): the line a collection says.
    for st in &mut c.quests.content.stakes {
        let sl = local(st.id.as_str()).to_string();
        f(
            &format!("stake.{sl}.collected"),
            &mut st.collected_message,
            KeyEntry::always(Stage::Quests),
        );
    }
    // v0.4 effect strings — `narrate` text, a named `give-item`, a bonfire's rest
    // dialog, a seal's answer — over **every** root emission can lower an effect
    // from, not just the quests stage's three ([`effect_roots_mut`]).
    // Nesting inside each root is descended by `effect_strings_deep`, so a narrate
    // in a `sequence` step of a trap payload is inventoried like any other.
    for (stage, _path, keybase, eff) in effect_roots_mut(c) {
        effect_strings_deep(eff, &keybase, KeyEntry::always(effect_stage(stage)), f);
    }
}

/// The key an entity display name is inventoried under: the key already claimed by
/// an identical name earlier in the traversal, or `own` if this site is the first
/// to carry that text (in which case it claims it for every later site).
///
/// The lookup is on the string as authored, captured **before** `f` may rewrite it
/// ([`localize`] swaps the NPC's name to the target language, and the actor puppets
/// that follow are still English at the moment they are looked up).
fn entity_name_key(claimed: &mut BTreeMap<String, String>, text: &str, own: String) -> String {
    claimed.entry(text.to_string()).or_insert(own).clone()
}

/// The authoritative key → canonical-English inventory derived from the stage
/// docs. Deterministic (keys are unique and the traversal order is fixed).
///
/// # Widening this is an ERROR-tier obligation on every existing campaign
///
/// This walk consults the campaign document and **never its `dsl_version`**.
/// For the surface itself that is right — a campaign at 0.6.0 cannot use a 0.9
/// surface, so those keys simply never appear — but it has a consequence worth
/// stating, and one already paid for twice — by the widenings of
/// [`each_string`] onto `traps[].payload` and onto `on_respawn` bundles:
///
/// **when a widening reaches strings that OLDER surfaces already emitted**, the
/// inventory grows for campaigns of every declared version at once, and
/// [`L10N_MISSING`](crate::codes::L10N_MISSING) (`DW0180`) is
/// `Diagnostic::error`. A campaign that was complete and green stops building
/// on the next engine, with no deprecation window and nothing in its own
/// document changed.
///
/// Measured 2026-08-08 on `nobodys-cave-island` (sidecar `dsl_version` 0.6.0):
/// removing one key — the shape a widening creates — exits 1 immediately with
/// `DW0180 [error]`.
///
/// **The asymmetry is the finding, and it is with this module's own siblings.**
/// Two comparable obligations on existing content take a warn-first window and
/// say so in their own text: `DW0188` (translation provenance, in
/// [`validate_l10n_provenance`] right here) and `DW0465` (the cast ledger, in
/// `compiler::cast`). Coverage does not. Whether it should is an owner call —
/// changing a check's tier is never a mechanical change (CLAUDE.md) — so this
/// records the measurement rather than acting on it.
///
/// Whichever way it is decided, the widening PR is where it has to be decided:
/// adoption rounds for every active campaign belong in the same milestone as
/// the `dsl_version` that creates the obligation, and a widening that skips the
/// version bump creates the obligation with no milestone at all.
pub fn inventory(c: &Campaign) -> BTreeMap<String, String> {
    let mut c2 = c.clone();
    let mut out = BTreeMap::new();
    each_string(&mut c2, &mut |key, value, _entry| {
        out.insert(key.to_string(), value.clone());
    });
    out
}

/// The subset of [`inventory`] a campaign's sidecars are **obliged** to cover:
/// every key whose [`KeyEntry`] this campaign's own declared `dsl_version`s have
/// reached.
///
/// The two differ only by keys added to the walk over surface older campaigns
/// already had — the widening shape. A key outside this set is still
/// inventoried, still tagged into its text component and still translated when a
/// sidecar carries one, so nothing about emission or about an already-adopted
/// sidecar moves; it simply may not be *demanded* of a campaign that never opted
/// into it. `DW0180` reads this; `DW0181` (orphans) reads the full [`inventory`],
/// so a campaign that translated an un-demanded key early is never punished for
/// being ahead.
pub fn required_inventory(c: &Campaign) -> BTreeMap<String, String> {
    let versions = StageVersions::of_campaign(c);
    let mut c2 = c.clone();
    let mut out = BTreeMap::new();
    each_string(&mut c2, &mut |key, value, entry| {
        if entry.required_by(&versions) {
            out.insert(key.to_string(), value.clone());
        }
    });
    out
}

/// The NPC an inventory key belongs to (its **local** id), when the key scheme
/// encodes one: `dlg.<npc>.…` (that NPC's dialogue tree — `.text` is the NPC's own
/// line, `.opt.<i>.label` the player's reply *within* it), `npc.<npc>.name`, and
/// `cast.<quest>.<npc>.…` (a v0.7 bark line — the NPC's own murmured speech, so a
/// translator gets the same persona context a dialogue line gets).
/// Returns `None` for every other key kind.
///
/// Lives beside [`each_string`] — the traversal that *defines* the key scheme — so
/// the two cannot drift silently (a CLI test asserts every speaker derived from a
/// real campaign's inventory resolves to a declared NPC). Consumed by
/// `delvec l10n-inventory`, which hands a translator the speaking character's
/// persona (`speech_style` above all) alongside the English line.
pub fn key_speaker(key: &str) -> Option<&str> {
    let (kind, rest) = key.split_once('.')?;
    match kind {
        "dlg" | "npc" => rest.split('.').next(),
        // `cast.<quest>.<npc>.<branch>.bark.<i>` — the npc is the second segment.
        "cast" => rest.split('.').nth(1),
        _ => None,
    }
}

/// One `narrate` `art` occurrence (DSL v0.6, spec-0014): its stage-doc path (for
/// diagnostics), its l10n inventory key, and the canonical English text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtNarrate {
    /// The stage document the string was authored in (`quests` / `dialogue`).
    pub stage: &'static str,
    /// JSON-pointer-ish path within that stage doc.
    pub path: String,
    /// The l10n inventory key (`fx.…​.narrate`) — always present, since every
    /// `narrate` lives in an inventoried effect position.
    pub key: String,
    /// The canonical English text.
    pub text: String,
}

/// Every `narrate` effect using the v0.6 `art` style, in a fixed deterministic
/// order. Its l10n `key` is derived by the **same** traversal/keying as
/// [`inventory`]/[`each_string`], so the compiler's art-font glyph check
/// (`DW0328`) can look each art string up in every declared-language sidecar.
/// Every art narrate lives in a quest `on_objective_complete`/`on_complete` or an
/// environment trigger — all inventoried — so each `key` is guaranteed present in
/// a fully-covered sidecar.
pub fn art_narrates(c: &Campaign) -> Vec<ArtNarrate> {
    let mut out = Vec::new();
    each_effect_ref(c, &mut |stage, path, keybase, eff| {
        if let Some(text) = eff.narrate_art_text() {
            out.push(ArtNarrate {
                stage,
                path: format!("{path}/text"),
                key: format!("{keybase}.narrate"),
                text: text.to_string(),
            });
        }
    });
    out
}

/// One on-screen `narrate` occurrence — `title`, `subtitle` or `art` — its stage-doc
/// path, its l10n inventory key, its style, and the canonical English text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScreenNarrate {
    /// The stage document the string was authored in (`quests` / `dialogue`).
    pub stage: &'static str,
    /// JSON-pointer-ish path within that stage doc.
    pub path: String,
    /// The l10n inventory key (`fx.…​.narrate`).
    pub key: String,
    /// Which on-screen channel vanilla draws it in — this selects the width budget.
    pub style: NarrateStyle,
    /// The canonical English text.
    pub text: String,
}

/// Every `narrate` effect vanilla draws **on screen** (`title` / `subtitle` / `art`),
/// in a fixed deterministic order. Like [`art_narrates`], each `key` is derived by the
/// **same** traversal/keying as [`inventory`]/[`each_string`], so the compiler's
/// text-fit check (`DW0330`) can look every string up in each declared-language
/// sidecar and report it under the offending locale and key. `chat` narrates are
/// excluded: chat wraps and scrolls, so it has no width budget.
pub fn on_screen_narrates(c: &Campaign) -> Vec<ScreenNarrate> {
    let mut out = Vec::new();
    each_effect_ref(c, &mut |stage, path, keybase, eff| {
        if let Some((style, text)) = eff.narrate_on_screen() {
            out.push(ScreenNarrate {
                stage,
                path: format!("{path}/text"),
                key: format!("{keybase}.narrate"),
                style,
                text: text.to_string(),
            });
        }
    });
    out
}

/// One dialogue option label — the caption vanilla draws on a fixed-width dialog
/// button — with its stage-doc path, its l10n inventory key and the canonical
/// English text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OptionLabel {
    /// The stage document the string was authored in (`dialogue` for a real
    /// dialogue option; a bonfire's labels carry the stage they were authored in).
    pub stage: &'static str,
    /// JSON-pointer-ish path within that stage doc.
    pub path: String,
    /// The l10n inventory key (`dlg.<npc>.<node>.opt.<i>.label`).
    pub key: String,
    /// The canonical English text.
    pub text: String,
}

/// Every dialogue option label, in a fixed deterministic order (declaration order:
/// tree, then node, then option). Each `key` is derived by the **same** keying as
/// [`inventory`]/[`each_string`], so the compiler's button-width check (`DW0331`)
/// can look every label up in each declared-language sidecar and report an
/// overflowing translation under its own locale and key.
///
/// Every option label is emitted as a button caption exactly once per node variant
/// (`emit::build_node_dialog`); display gating (`requires_flags`/`forbids_flags`)
/// only decides *whether* a variant shows it, never how wide it renders, so gated
/// and ungated options carry the same budget and are all visited here.
pub fn dialogue_option_labels(c: &Campaign) -> Vec<OptionLabel> {
    let mut out = Vec::new();
    for (ti, tree) in c.dialogue.content.dialogues.iter().enumerate() {
        let np = local(tree.npc.as_str());
        for (ni, node) in tree.nodes.iter().enumerate() {
            let nd = local(node.id.as_str());
            for (oi, opt) in node.options.iter().enumerate() {
                out.push(OptionLabel {
                    stage: "dialogue",
                    path: format!("/content/dialogues/{ti}/nodes/{ni}/options/{oi}/label"),
                    key: format!("dlg.{np}.{nd}.opt.{oi}.label"),
                    text: opt.label.clone(),
                });
            }
        }
    }
    out
}

/// Every **authored** bonfire rest-dialog label (spec-0016 §1), in the same
/// fixed effect order the inventory uses. A bonfire's
/// two options are drawn on exactly the same 150-GUI-px `multi_action` button a
/// dialogue option is, so they carry exactly the same width budget (`DW0331`) —
/// the check follows the widget, not the stage the string was authored in.
///
/// Unauthored labels are absent by construction: the compiler's canonical English
/// (`Rest and save` / `Save only` / `Bonfire`) is measured once by a compiler unit
/// test rather than re-measured per campaign, since it cannot vary.
pub fn bonfire_option_labels(c: &Campaign) -> Vec<OptionLabel> {
    let mut out = Vec::new();
    each_effect_ref(c, &mut |stage, path, keybase, eff| {
        let Some(l) = eff.bonfire_labels() else {
            return;
        };
        for (text, field, key) in [
            (l.rest_label, "rest_label", "rest_label"),
            (l.save_label, "save_label", "save_label"),
        ] {
            if let Some(text) = text {
                out.push(OptionLabel {
                    stage,
                    path: format!("{path}/{field}"),
                    key: format!("{keybase}.{key}"),
                    text: text.to_string(),
                });
            }
        }
    });
    out
}

/// The **five effect roots** the compiler can lower a quest effect from, as
/// `(stage, json_path, l10n keybase)` for each root's `i`-th top-level effect.
///
/// The roots themselves are **not enumerated here**. They come from
/// [`crate::effects::for_each_effect_root`], the one enumeration in the workspace,
/// which this walk simply indexes into per top-level effect (`path` + `/{i}`,
/// `key` + `.{i}`). Before that module existed this function held its own copy of
/// the root list and [`effect_roots_mut`] held a second one, which is the
/// arrangement that let a walk go blind to a root — twice, independently, in this
/// file alone. The paths and keys are unchanged in both directions.
fn effect_roots(c: &Campaign) -> Vec<EffectRoot<'_>> {
    let mut out = Vec::new();
    crate::effects::for_each_effect_root(c, &mut |site, list| {
        for (i, eff) in list.iter().enumerate() {
            out.push(EffectRoot {
                stage: site.stage,
                path: format!("{}/{i}", site.path),
                key: format!("{}.{i}", site.key),
                eff,
            });
        }
    });
    out
}

/// One top-level effect root: where it lives, what its l10n keys hang off, and the
/// effect itself.
struct EffectRoot<'a> {
    /// The stage document (`quests` / `dialogue`) this effect was authored in.
    stage: &'static str,
    /// JSON pointer to the effect within that document.
    path: String,
    /// The effect's l10n key prefix.
    key: String,
    /// The effect.
    eff: &'a QuestEffect,
}

/// The **mutable mirror** of [`effect_roots`]: the identical roots, in the
/// identical order, with the same `(stage, path, key)` descriptors, exposed
/// mutably so [`each_string`] (and through it [`localize`]) can rewrite the
/// player-visible strings in place.
///
/// Like [`effect_roots`] it enumerates nothing itself — it indexes
/// [`crate::effects::for_each_effect_root_mut`], which is generated from the
/// **same macro body** as the immutable walk. The two mirrors are therefore
/// lockstep by construction rather than by a test that has to be remembered; the
/// descriptor-equality test below now pins that property instead of establishing
/// it.
fn effect_roots_mut(c: &mut Campaign) -> Vec<(&'static str, String, String, &mut QuestEffect)> {
    let mut out: Vec<(&'static str, String, String, &mut QuestEffect)> = Vec::new();
    crate::effects::for_each_effect_root_mut(c, &mut |kind, path, key, list| {
        for (i, eff) in list.iter_mut().enumerate() {
            out.push((
                kind.stage(),
                format!("{path}/{i}"),
                format!("{key}.{i}"),
                eff,
            ));
        }
    });
    out
}

/// Visit every effect emission can lower — **top-level and every
/// transitively-nested** one (a `sequence` step, an
/// `on_respawn`/`on_caught`/`on_arrive` bundle) — over all five
/// [`effect_roots`], in the fixed inventory order, invoking
/// `f(stage, path, keybase, effect)`. `stage` names the document the effect lives
/// in (`quests` or `dialogue`) and `path` is its JSON pointer within it, so a
/// diagnostic can point at the real site; `keybase` is its l10n key prefix, derived
/// by the **same** position-keying as [`each_string`]/[`effect_strings_deep`] (so an
/// art narrate's key matches its inventory key). Shared by [`art_narrates`],
/// [`on_screen_narrates`], [`bonfire_option_labels`], [`sound_refs`] and
/// [`play_sound_actor_refs`], so the consumer checks
/// (`DW0326`/`DW0328`/`DW0330`/`DW0331`/`DW0335`) see exactly the strings the
/// inventory demands a translation for.
fn each_effect_ref<'a>(
    c: &'a Campaign,
    f: &mut dyn FnMut(&'static str, &str, &str, &'a QuestEffect),
) {
    for r in effect_roots(c) {
        effect_deep(r.eff, r.stage, &r.path, &r.key, f);
    }
}

/// Visit `eff` and every transitively-nested effect (depth-first, pre-order),
/// threading the JSON-pointer `path` and l10n `keybase` through each nested list via
/// [`QuestEffect::nested_effect_lists_labeled`] (path segment + key segment + the
/// per-effect index). The key segments match [`effect_strings_deep`] exactly.
fn effect_deep<'a>(
    eff: &'a QuestEffect,
    stage: &'static str,
    path: &str,
    keybase: &str,
    f: &mut dyn FnMut(&'static str, &str, &str, &'a QuestEffect),
) {
    f(stage, path, keybase, eff);
    for (pseg, kseg, list) in eff.nested_effect_lists_labeled() {
        for (j, inner) in list.iter().enumerate() {
            effect_deep(
                inner,
                stage,
                &format!("{path}/{pseg}/{j}"),
                &format!("{keybase}.{kseg}.{j}"),
                f,
            );
        }
    }
}

/// One vanilla sound-event reference (DSL v0.6/v0.4): its stage-doc path and the
/// referenced id, for registry validation (`DW0326`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoundRef {
    /// The stage document the reference was authored in (`quests` / `dialogue`).
    pub stage: &'static str,
    /// JSON-pointer-ish path within that stage doc.
    pub path: String,
    /// The referenced sound-event id (`minecraft:` prefix optional).
    pub sound: String,
}

/// Every vanilla sound-event id referenced by a quest/trigger effect — a
/// `play-sound`'s `sound` (v0.6) and a `narrate`'s optional `sound` (v0.4) — in a
/// fixed deterministic order, for `DW0326` validation.
pub fn sound_refs(c: &Campaign) -> Vec<SoundRef> {
    let mut out = Vec::new();
    each_effect_ref(c, &mut |stage, path, _key, eff| {
        for (sub, sound) in eff.sound_refs() {
            out.push(SoundRef {
                stage,
                path: format!("{path}/{sub}"),
                sound: sound.to_string(),
            });
        }
    });
    out
}

/// Every `play-sound` effect using the deferred `at: actor` target, in a fixed
/// order, as `(path, actor-id)` pairs. The actor variant is accepted by the
/// schema but rejected (`DW0335`) until the actors surface (spec-0014 `actors[]`)
/// lands; the compiler applies that check. `SoundRef::sound` carries the actor id.
pub fn play_sound_actor_refs(c: &Campaign) -> Vec<SoundRef> {
    let mut out = Vec::new();
    each_effect_ref(c, &mut |stage, path, _key, eff| {
        if let Some(actor) = eff.play_sound_actor() {
            out.push(SoundRef {
                stage,
                path: format!("{path}/at/actor"),
                sound: actor.to_string(),
            });
        }
    });
    out
}

/// Replace every inventoried string in `c` with its translation from
/// `translations` (an l10n sidecar's `content`). Keys absent from the map are left
/// as canonical English — but a fully-validated sidecar ([`validate_l10n`]) covers
/// the inventory exactly, so a build only reaches here with complete coverage.
pub fn localize(c: &mut Campaign, translations: &BTreeMap<String, String>) {
    each_string(c, &mut |key, value, _entry| {
        if let Some(t) = translations.get(key) {
            *value = t.clone();
        }
    });
}

// ---------------------------------------------------------------------------
// i18n v2 — translation tags and the Minecraft language-code table (spec-0029)
// ---------------------------------------------------------------------------

/// The reserved Unicode **private-use** character that delimits a *translation
/// tag* — the in-band form that carries an inventory key alongside its canonical
/// English from the stage docs to the text component the compiler emits it into.
///
/// A tagged string is `<SIGIL><key><SIGIL><english>` ([`tag`]). It exists only
/// between [`tag_translatables`] and emission: every emitter that lowers an
/// authored string into a **text component** splits it back apart and emits
/// `{"translate": key, "fallback": english}` (spec-0029 §1), and every consumer
/// that wants the human string calls [`plain`].
///
/// The point of an in-band tag is that a site which *fails* to do either leaks the
/// sigil into the built tree, where the compiler's own output scan sees it and
/// fails the build (`DW0185`). That turns "prove every authored string lands in a
/// component" from an audit that rots into an invariant the compiler re-proves on
/// every build, including for emitters not yet written.
///
/// U+E000 is the first code point of the Basic Multilingual Plane's Private Use
/// Area: it has no character assignment, so no authored or translated content can
/// legitimately contain it. [`validate_tr_sigil`] (`DW0183`) reserves the whole
/// block anyway, so the tag can never be forged or shadowed by content.
pub const TR_SIGIL: char = '\u{E000}';

/// The reserved private-use range [`TR_SIGIL`] is drawn from (`U+E000..=U+F8FF`).
/// Reserved wholesale so a near-miss cannot be authored either.
const PUA: std::ops::RangeInclusive<char> = '\u{E000}'..='\u{F8FF}';

/// Build the translation tag for `key` over its canonical English `english`.
pub fn tag(key: &str, english: &str) -> String {
    format!("{TR_SIGIL}{key}{TR_SIGIL}{english}")
}

/// Split a translation tag into `(key, english)`. `None` for an untagged string —
/// a compiler-baked literal such as the default boundary message, which has no
/// inventory key and is translated by neither v1 nor v2.
pub fn untag(s: &str) -> Option<(&str, &str)> {
    let rest = s.strip_prefix(TR_SIGIL)?;
    let (key, english) = rest.split_once(TR_SIGIL)?;
    Some((key, english))
}

/// The human string behind `s`: its English source if `s` is a translation tag,
/// otherwise `s` unchanged. The accessor every **non-component** consumer of an
/// authored string uses — the build manifest, the reviewer chronicles, the bot's
/// `critical-path.json`, the generated PackTest sources. Each such site is a named
/// exclusion in `docs/reference/compiler.md`: it is not a text component, so it
/// cannot carry a translate key, and it is not read by a player.
pub fn plain(s: &str) -> &str {
    untag(s).map(|(_, e)| e).unwrap_or(s)
}

/// Whether `s` contains any reserved private-use character — i.e. whether it is,
/// or embeds, a translation tag. The predicate the compiler's build-output scan
/// (`DW0185`) runs over every emitted byte.
pub fn has_tr_sigil(s: &str) -> bool {
    s.chars().any(|c| PUA.contains(&c))
}

/// Rewrite every inventoried player-visible string in `c` into its translation tag
/// ([`tag`]), returning the canonical-English inventory it was derived from.
///
/// Runs on the exact same traversal as [`inventory`] and [`localize`]
/// ([`each_string`]), so the tagged set and the translated set are the same set by
/// construction — the property spec-0029 keeps.
///
/// The campaign handed to the compiler is tagged **once**, before the plan is
/// built; from there the tag is the compiler's only evidence that a string it is
/// about to emit is player-visible and translatable.
pub fn tag_translatables(c: &mut Campaign) -> BTreeMap<String, String> {
    let mut inv = BTreeMap::new();
    each_string(c, &mut |key, value, _entry| {
        inv.insert(key.to_string(), value.clone());
        *value = tag(key, value);
    });
    inv
}

/// Reserve the private-use block the translation tag is built from (`DW0183`): no
/// player-visible string — authored English (the whole [`inventory`]) or any
/// declared language's sidecar rendition — may contain a `U+E000..=U+F8FF`
/// character. Language-independent; runs beside [`validate_marker_channel`] on
/// every `validate` / `analyze` / `build`.
pub fn validate_tr_sigil(c: &Campaign, sidecars: &BTreeMap<String, L10nDoc>) -> Vec<Diagnostic> {
    let mut d = Vec::new();
    let mut flag = |where_: String, key: &str, text: &str| {
        let Some(bad) = text.chars().find(|ch| PUA.contains(ch)) else {
            return;
        };
        d.push(Diagnostic::error(
            codes::TR_SIGIL_RESERVED,
            "l10n",
            where_,
            format!(
                "player-visible string `{key}` contains the reserved private-use character \
                 U+{:04X} — that block is how the compiler carries an l10n key into the text \
                 component this string is emitted as, and it has no rendering in any \
                 Minecraft font. Remove U+{:04X} from the line",
                bad as u32, bad as u32
            ),
        ));
    };
    for (key, text) in inventory(c) {
        flag(format!("#/{key}"), &key, &text);
    }
    for (lang, doc) in sidecars {
        for (key, text) in &doc.content {
            flag(format!("l10n/{lang}.json#/content/{key}"), key, text);
        }
    }
    d
}

/// Every declared-language code this build knows how to write a lang file for, in
/// declaration order, as `(declared code, minecraft code)`. `Err` names the first
/// unmapped code (`DW0184`) — a language is never silently dropped.
pub fn declared_mc_codes(c: &Campaign) -> Result<Vec<(String, &'static str)>, Diagnostic> {
    let mut out = Vec::new();
    for lang in &c.world.content.languages {
        match crate::mclang::mc_lang_code(lang) {
            Some(mc) => out.push((lang.clone(), mc)),
            None => {
                return Err(Diagnostic::error(
                    codes::LANG_CODE_UNMAPPED,
                    "world",
                    format!("/content/languages/{lang}"),
                    format!(
                        "declared language `{lang}` has no Minecraft language-file code — the \
                         resource pack has nowhere to write its \
                         `assets/delvewright/lang/<code>.json`, and the language would ship \
                         invisible. Use a code the pinned 1.21.11 client really loads \
                         (`dsl::mclang::CLIENT_LANGS`, derived from Mojang's own asset index) \
                         — e.g. `zh-cn`, `ja-jp`, `de-de`"
                    ),
                ));
            }
        }
    }
    Ok(out)
}

/// **Translation provenance** (`DW0187` / `DW0188`): is each sidecar row still a
/// translation of the English it renders?
///
/// [`validate_l10n`] proves the sidecar's key SET equals the inventory's. That is
/// silent about whether a row still *corresponds* to its key: rewrite an authored
/// line and its translation is present, applied and wrong, with no key moved. The
/// sidecar's [`L10nDoc::source`] map closes it by recording the English each row
/// was translated from, so the compiler can compare instead of a human auditing.
///
/// Two findings:
///
/// * `DW0187` — a recorded source differs from the key's canonical English (the
///   row is stale), or names a key the sidecar does not translate at all (the
///   provenance itself is stale).
/// * `DW0188` — rows with no recorded provenance, **counted**. Those rows are
///   unguarded, and saying so on every run is what keeps an unadopted sidecar
///   from reading like a checked one. Warning tier: `source` is additive, and
///   this is the one-version deprecation window before it is required.
///
/// The entity display-name rule makes this more than hygiene. A name key belongs
/// to the first site declaring a given text, so renaming ONE body can migrate a
/// key to ANOTHER — the row that goes stale is not the row the author edited, and
/// the missing-key half of the move (`DW0180`) points somewhere else entirely.
pub fn validate_l10n_provenance(
    c: &Campaign,
    sidecars: &BTreeMap<String, L10nDoc>,
) -> Vec<Diagnostic> {
    let mut d = Vec::new();
    if c.world.content.languages.is_empty() {
        return d;
    }
    let inv = inventory(c);
    for lang in &c.world.content.languages {
        let Some(doc) = sidecars.get(lang) else {
            continue; // absent sidecar is DW0180's finding, not this one's.
        };
        for (key, was) in &doc.source {
            match inv.get(key) {
                Some(now) if now == was => {}
                Some(now) => d.push(Diagnostic::error(
                    codes::L10N_STALE,
                    "l10n",
                    format!("l10n/{lang}.json#/source/{key}"),
                    format!(
                        "`{key}` was translated from {was:?} but now reads {now:?} — the \
                         translation in `content` still renders the old line and would ship \
                         attached to the new one. Re-translate `{key}` and update its `source` \
                         (`tools/i18n-translate.py <campaign> --lang {lang}` does both). If a \
                         RENAME surprised you here: an entity display name's key belongs to the \
                         first body declaring that text, so renaming one body can hand its key \
                         to another"
                    ),
                )),
                None => d.push(Diagnostic::error(
                    codes::L10N_STALE,
                    "l10n",
                    format!("l10n/{lang}.json#/source/{key}"),
                    format!(
                        "`source` records `{key}`, which is not in the string inventory — the \
                         provenance is stale even if the translation is gone. Remove `{key}` \
                         from `source` in `l10n/{lang}.json`"
                    ),
                )),
            }
        }
        // Every row `source` does not cover is a row DW0187 cannot see. Report the
        // count: an unadopted sidecar must never look like a checked one.
        let unguarded = doc
            .content
            .keys()
            .filter(|k| !doc.source.contains_key(*k))
            .count();
        if unguarded > 0 {
            let total = doc.content.len();
            d.push(Diagnostic::warning(
                codes::L10N_PROVENANCE_MISSING,
                "l10n",
                format!("l10n/{lang}.json"),
                format!(
                    "{unguarded} of {total} translated rows record no `source`, so nothing can \
                     tell whether they still translate the English they render — an edited line \
                     leaves its translation present, applied and wrong, and no key moves. Run \
                     `tools/i18n-translate.py <campaign> --lang {lang}` to record provenance for \
                     the rows it already has. This warning is the one-version deprecation \
                     window; `source` becomes required after it"
                ),
            ));
        }
    }
    d
}

/// Coverage + envelope validation for every declared language's l10n sidecar
/// (`DW0180` / `DW0181`). Language-independent: it runs on every `validate` /
/// `analyze` / `build`, regardless of `--lang`. Returns no diagnostics for a
/// campaign that declares no languages. `sidecars` is keyed by language code
/// (the `l10n/<code>.json` filename stem).
pub fn validate_l10n(c: &Campaign, sidecars: &BTreeMap<String, L10nDoc>) -> Vec<Diagnostic> {
    let mut d = Vec::new();
    let declared = &c.world.content.languages;
    if declared.is_empty() {
        return d;
    }
    // Coverage is asymmetric on purpose, and the asymmetry IS the obligation
    // fence at key granularity (see [`KeyEntry`]):
    //
    // * `required` — what a sidecar must have. Every key whose entry version this
    //   campaign has reached. A widening onto older surface adds to `inventory`
    //   but not to `required`, so it cannot turn an unchanged campaign red.
    // * `inv` — what a sidecar MAY have. The full inventory, so a campaign that
    //   translated a not-yet-demanded key is not then told it is an orphan.
    let required = required_inventory(c);
    let required_keys: BTreeSet<&str> = required.keys().map(String::as_str).collect();
    let inv = inventory(c);
    let inv_keys: BTreeSet<&str> = inv.keys().map(String::as_str).collect();
    let campaign_id = c.world.campaign_id.as_str();

    for lang in declared {
        if lang == CANONICAL_LANG {
            d.push(Diagnostic::error(
                codes::L10N_MISSING,
                "l10n",
                format!("/world/languages/{lang}"),
                format!(
                    "`{lang}` is the canonical language and must not be declared in \
                     `world.languages` — English is implicit; remove `{lang}` from the list"
                ),
            ));
            continue;
        }
        let Some(doc) = sidecars.get(lang) else {
            d.push(Diagnostic::error(
                codes::L10N_MISSING,
                "l10n",
                format!("l10n/{lang}.json"),
                format!(
                    "declared language `{lang}` has no `l10n/{lang}.json` sidecar — add the \
                     sidecar (a full key→translation map), or remove `{lang}` from \
                     `world.languages`"
                ),
            ));
            continue;
        };
        // Envelope consistency (folded into DW0180 — the sidecar does not correctly
        // cover the declared language).
        if doc.campaign_id.as_str() != campaign_id {
            d.push(Diagnostic::error(
                codes::L10N_MISSING,
                "l10n",
                format!("l10n/{lang}.json"),
                format!(
                    "sidecar `campaign_id` `{}` differs from the campaign's `{campaign_id}` — set \
                     the sidecar's `campaign_id` to `{campaign_id}`",
                    doc.campaign_id
                ),
            ));
        }
        if doc.lang != *lang {
            d.push(Diagnostic::error(
                codes::L10N_MISSING,
                "l10n",
                format!("l10n/{lang}.json"),
                format!(
                    "sidecar `lang` field `{}` differs from filename code `{lang}` — set the \
                     sidecar's `lang` to `{lang}` (it must match the `l10n/{lang}.json` filename)",
                    doc.lang
                ),
            ));
        }
        if !is_supported_version(&doc.dsl_version) {
            d.push(Diagnostic::error(
                codes::L10N_MISSING,
                "l10n",
                format!("l10n/{lang}.json"),
                format!(
                    "sidecar has unsupported dsl_version `{}` — set it to a supported version \
                     matching the stage docs (one of {:?})",
                    doc.dsl_version,
                    crate::accepted_versions().collect::<Vec<_>>()
                ),
            ));
        }
        // Coverage: missing (DW0180) against what this campaign's version owes,
        // orphan (DW0181) against the whole inventory.
        let side_keys: BTreeSet<&str> = doc.content.keys().map(String::as_str).collect();
        for missing in required_keys.difference(&side_keys) {
            d.push(Diagnostic::error(
                codes::L10N_MISSING,
                "l10n",
                format!("l10n/{lang}.json"),
                format!(
                    "sidecar is missing a translation for inventory key `{missing}` — add \
                     `{missing}` to `l10n/{lang}.json` (coverage must be exact)"
                ),
            ));
        }
        for orphan in side_keys.difference(&inv_keys) {
            d.push(Diagnostic::error(
                codes::L10N_ORPHAN,
                "l10n",
                format!("l10n/{lang}.json#/content/{orphan}"),
                format!(
                    "orphan key `{orphan}` is not in the string inventory — remove it from \
                     `l10n/{lang}.json` (the sidecar must cover exactly the inventory, no extras)"
                ),
            ));
        }
    }
    d
}
