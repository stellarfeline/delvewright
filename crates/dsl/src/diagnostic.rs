//! Diagnostics: the `--json` shape from spec-0002 and the stable `DW01xx` codes.
//!
//! # Every code declares when it starts binding a campaign
//!
//! A per-stage `dsl_version` fence guards **new surface** — "you may not write
//! this field below version X" (`DW0141`). Until [`Binds`] existed nothing
//! guarded **new obligations** — "you are now required to have X" — so whether a
//! check respected a campaign's declared version depended on whether its author
//! remembered. That is a convention, and its measured cost is this:
//! `dsl::l10n::each_string` widened onto an actor's own `name` with no
//! version gate, `DW0180` compares key SETS and had no version gate either, and
//! the obligation therefore reached every campaign at every declared version at
//! once — a 0.6.0/0.8.0 campaign went red mid-staging with nothing
//! in its own documents changed.
//!
//! The rule: **the compiler processes a campaign
//! according to its DECLARED `dsl_version`; a campaign that compiled before
//! keeps its behaviour unchanged.** [`DwCode`] is how that stops being a
//! convention: a diagnostic cannot be built from a bare string, only from a
//! `DwCode`, and a `DwCode` cannot be built without saying which of the two
//! things it is ([`DwCode::every_version`] / [`DwCode::since`]). "Forgot to
//! fence" is then not a category of mistake — there is no constructor for it.
//!
//! The fence itself — the pass that drops a `Since(n)` diagnostic raised against
//! a campaign below `n` — is [`crate::fence`].
//!
//! # One cause, one line
//!
//! **A secondary whose premise is an already-reported primary is folded into
//! that primary or suppressed, and the line that survives says how many
//! dependants it stands for.** A refusal is the whole product at the moment an
//! author meets it, and N copies of one sentence is a count the reader has to
//! discount rather than information — worse, the copies come first and bury the
//! one line that is theirs to act on.
//!
//! Measured on a 24-place campaign: deleting `layout-graph.json` printed
//! `DW0824` (correct, one line) and then **`DW0842` twenty-four times**, once
//! per `details[]` row, each saying the plan resolves 0 boxes; shortening the
//! region by five courses printed **`DW0826` twenty-four times**, once per box,
//! for one number in one document.
//!
//! The rule has two shapes, and which one applies is decided by whether the
//! secondary still has anything of its own to say:
//!
//! 1. **Fold.** Every finding shares one cause and one repair, so they are one
//!    diagnostic naming all of them. The code is unchanged and still fires per
//!    item the moment the items differ — the folded arm is reachable only in the
//!    state that makes them identical. Instances: [`codes::QUEST_NOT_EXPANDED`]
//!    when stage 5 is empty (`crate::validate`), `DW0842` at a zero box count
//!    (`compiler::detail`), `DW0826` when more than one thing leaves the region
//!    (`crate::siteplan`).
//! 2. **Defer.** The secondary is a real, separate finding whose NUMBER was
//!    measured against something already refused, so it keeps its own line and
//!    gains a clause naming what it is downstream of. Instances: `DW0818`'s
//!    clause when stage 5 declares no quests (`crate::layout`), and
//!    `crate::siteplan::off_grid_note` on every verdict computed from a box
//!    `DW0825` has refused.
//!
//! What the rule never does is drop a code's ability to refuse. Folding changes
//! how many lines say a thing, never whether the run stops: every fold above is
//! an error tier that still exits non-zero, and each has a test on both sides —
//! primary present, one line; primary absent, the secondary fires per item as
//! before.

use serde::Serialize;

/// **When a rule starts binding a campaign**, in `dsl_version` terms.
///
/// The question this answers, and the only question it answers: *could this
/// check go from green to red on a campaign whose own documents did not change,
/// because the engine changed?*
///
/// * **No** → [`Binds::EveryVersion`]. The rule judges what the document SAYS —
///   a malformed id, an unknown item, a surface used below the version that
///   introduced it, a contradiction between two authored fields. Its verdict is
///   a function of the campaign alone, so grandfathering has nothing to
///   grandfather: a campaign that was green stays green until somebody edits it.
/// * **Yes** → [`Binds::Since`]. The rule requires the campaign to HAVE
///   something it may not have had before. It binds only at or above the
///   `dsl_version` that created the requirement, and every campaign below that
///   version is grandfathered — which is what makes the version bump the
///   campaign's own explicit, proof-carrying adoption round (CLAUDE.md
///   §version-adoption discipline) rather than an engine-side surprise.
///
/// The distinction is not cosmetic and the safe-looking answer is not the safe
/// one. Fencing a wellformedness rule would *stop rejecting* bad documents in
/// old campaigns — `DW0141` fenced at 0.10 would let a 0.6 campaign use the 0.10
/// surface. Failing to fence an obligation is the opposite error, and reaches
/// every campaign at once. Neither direction is a
/// default; both are a decision, which is why there is no `Default` impl and no
/// way to leave it unsaid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum Binds {
    /// Applies at every declared `dsl_version`, and always has.
    EveryVersion,
    /// Applies at or above this **minor-version ordinal** (`0.8.0` → `8`, the
    /// same ordinal [`crate::envelope`]'s `is_v0*` predicates compare). A
    /// campaign whose relevant stage declares less than this is grandfathered:
    /// the diagnostic is dropped by [`crate::fence`] before it can reach a
    /// verdict.
    Since(u32),
}

/// A stable DW diagnostic code **together with the version at which it starts
/// binding** ([`Binds`]).
///
/// The pairing is the point. A code is not a string that a check happens to
/// quote; it is a rule with a scope, and the scope travels with it to every site
/// that raises it, so the fence never has to look the rule up in a registry
/// somebody has to remember to update.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DwCode {
    id: &'static str,
    binds: Binds,
    subject: Subject,
}

impl DwCode {
    /// A rule that judges what the document SAYS — see [`Binds::EveryVersion`]
    /// for the test to apply before choosing this.
    pub const fn every_version(id: &'static str) -> DwCode {
        DwCode {
            id,
            binds: Binds::EveryVersion,
            subject: Subject::Campaign,
        }
    }

    /// A rule that REQUIRES the campaign to have something, from the given
    /// minor-version ordinal onward (`0.8.0` → `8`) — see [`Binds::Since`].
    pub const fn since(id: &'static str, minor: u32) -> DwCode {
        DwCode {
            id,
            binds: Binds::Since(minor),
            subject: Subject::Campaign,
        }
    }

    /// Mark this code an **engine-property notice** — see [`Subject::Engine`]
    /// for the test to apply before choosing it. Chained onto whichever
    /// constructor states when the rule binds, because the two questions are
    /// independent: *when does this start applying to a campaign* and *whose
    /// state is it about*.
    pub const fn about_the_engine(self) -> DwCode {
        DwCode {
            id: self.id,
            binds: self.binds,
            subject: Subject::Engine,
        }
    }

    /// The stable code string (`DW0180`).
    pub const fn id(self) -> &'static str {
        self.id
    }

    /// When this rule starts binding a campaign.
    pub const fn binds(self) -> Binds {
        self.binds
    }

    /// Whose state this code's verdict is about.
    pub const fn subject(self) -> Subject {
        self.subject
    }
}

impl Serialize for DwCode {
    /// Serializes as the bare code string: a `DwCode` in a JSON payload is the
    /// `&'static str` it replaced, and [`Binds`] is compiler-internal.
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.id)
    }
}

impl std::fmt::Display for DwCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.id)
    }
}

impl AsRef<str> for DwCode {
    fn as_ref(&self) -> &str {
        self.id
    }
}

impl PartialEq<DwCode> for String {
    fn eq(&self, other: &DwCode) -> bool {
        self == other.id
    }
}

impl PartialEq<String> for DwCode {
    fn eq(&self, other: &String) -> bool {
        self.id == other
    }
}

impl PartialEq<DwCode> for str {
    fn eq(&self, other: &DwCode) -> bool {
        self == other.id
    }
}

impl PartialEq<DwCode> for &str {
    fn eq(&self, other: &DwCode) -> bool {
        *self == other.id
    }
}

impl PartialEq<&str> for DwCode {
    fn eq(&self, other: &&str) -> bool {
        self.id == *other
    }
}

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
    /// When this diagnostic's rule starts binding a campaign, carried over from
    /// the [`DwCode`] that raised it so [`crate::fence`] can grandfather it
    /// without a lookup table.
    ///
    /// Not part of the `--json` wire shape (spec-0002 fixes that at `code`,
    /// `severity`, `stage`, `path`, `message`): it is how the compiler decides
    /// whether to report a diagnostic at all, never something a consumer reads
    /// off one that was reported.
    #[serde(skip)]
    pub binds: Binds,
    /// Whose state this verdict is about, carried over from the [`DwCode`] that
    /// raised it — the key `delvec` groups its output by.
    ///
    /// Not part of the `--json` wire shape either, for the same reason `binds`
    /// is not: it decides how the run PRESENTS a diagnostic, never something a
    /// consumer reads off one.
    #[serde(skip)]
    pub subject: Subject,
}

impl Diagnostic {
    /// Build an error diagnostic.
    pub fn error(
        code: DwCode,
        stage: impl Into<String>,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Diagnostic {
            code: code.id().to_string(),
            severity: Severity::Error,
            stage: stage.into(),
            path: path.into(),
            message: message.into(),
            binds: code.binds(),
            subject: code.subject(),
        }
    }

    /// Build a warning (advisory) diagnostic. Reported, but does not fail the run.
    pub fn warning(
        code: DwCode,
        stage: impl Into<String>,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Diagnostic {
            code: code.id().to_string(),
            severity: Severity::Warning,
            stage: stage.into(),
            path: path.into(),
            message: message.into(),
            binds: code.binds(),
            subject: code.subject(),
        }
    }

    /// **Which of a run's three groups this line belongs in**, lowest first.
    ///
    /// The one authority on the order `delvec` prints in. See [`Subject`] for
    /// what the split is and why.
    #[must_use]
    pub fn group(&self) -> Group {
        match (self.severity, self.subject) {
            (Severity::Error, _) => Group::Refusal,
            (Severity::Warning, Subject::Campaign) => Group::AboutTheCampaign,
            (Severity::Warning, Subject::Engine) => Group::AboutTheEngine,
        }
    }
}

/// **Whose state a code's verdict is about.**
///
/// The question this answers, and the only question it answers: *if the author
/// changed nothing about their campaign and the engine's own tables were
/// finished, would this line go away?*
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Subject {
    /// The campaign. Every refusal, and every advisory whose verdict is a fact
    /// about the documents in front of the author — the default, because a
    /// diagnostic is addressed to an author unless it says otherwise.
    #[default]
    Campaign,
    /// **The ENGINE**, regardless of the campaign: an engine table that is still
    /// seeded, a standard that has not been calibrated. Nothing the author can
    /// write moves it, and it is identical on every campaign the engine
    /// compiles, so it prints after the lines that ARE theirs — see [`Group`].
    ///
    /// This is not a licence to make a campaign's problem quiet. The test is
    /// whether the line would read the same on a different campaign; where it
    /// names something the author wrote, it is a [`Subject::Campaign`] verdict
    /// however advisory its tier.
    Engine,
}

/// **The order a run's diagnostics are printed in**, and the labels they are
/// printed under.
///
/// Author-actionable first, then advisories about the campaign, then notices
/// about the engine. Measured on every site-plan run before this existed: four
/// to six paragraphs saying "this is fine" or "the engine's own table is
/// provisional", ahead of the one line the author was there to act on.
///
/// Ordering only — nothing is dropped, and every code that reported before
/// reports now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    /// A hard rejection. Yours to act on.
    Refusal,
    /// An advisory about the campaign: a measurement, or a verdict that depends
    /// on something outside the documents.
    AboutTheCampaign,
    /// A notice about this engine, true regardless of the campaign.
    AboutTheEngine,
}

impl Group {
    /// The heading this group is printed under, with `n` lines in it.
    #[must_use]
    pub fn heading(self, n: usize) -> String {
        match self {
            Group::Refusal => format!("-- {n} refusal(s): these are yours to act on"),
            Group::AboutTheCampaign => format!("-- {n} advisory(ies) about this campaign"),
            Group::AboutTheEngine => {
                format!("-- {n} notice(s) about this engine, true of any campaign")
            }
        }
    }
}

/// The stable validation diagnostic codes (catalogued in
/// `docs/reference/compiler.md` §5).
///
/// Every entry is a [`DwCode`], so every entry states when it starts binding a
/// campaign ([`Binds`]) — there is no way to add one that does not.
pub mod codes {
    use super::DwCode;

    /// Document does not conform to its stage schema (unknown field / wrong type).
    pub const SCHEMA: DwCode = DwCode::every_version("DW0100");
    /// Envelope `stage` does not match the document's slot.
    pub const STAGE_MISMATCH: DwCode = DwCode::every_version("DW0101");
    /// Unsupported `dsl_version`.
    pub const DSL_VERSION: DwCode = DwCode::every_version("DW0102");
    /// Inconsistent `campaign_id` across stages.
    pub const CAMPAIGN_ID_MISMATCH: DwCode = DwCode::every_version("DW0103");
    /// Malformed id syntax (kebab-case / prefix).
    pub const ID_SYNTAX: DwCode = DwCode::every_version("DW0110");
    /// Duplicate id within its namespace.
    pub const ID_DUPLICATE: DwCode = DwCode::every_version("DW0111");
    /// Dangling reference: an id ref does not resolve.
    pub const DANGLING_REF: DwCode = DwCode::every_version("DW0112");
    /// Stage-6 dialogue node unreachable from `root`.
    pub const DIALOGUE_UNREACHABLE: DwCode = DwCode::every_version("DW0120");
    /// Stage-6 dialogue `root`/`next` references an unknown node.
    pub const DIALOGUE_BAD_REF: DwCode = DwCode::every_version("DW0121");
    /// Stage-6 dialogue effect references an objective that is unknown, not a
    /// `talk-to`, or a `talk-to` on a different NPC (foreign effect).
    pub const DIALOGUE_BAD_OBJECTIVE: DwCode = DwCode::every_version("DW0122");
    /// A stage-5 `talk-to` objective has no reachable completing dialogue option
    /// (the static half of the compiler's `DW0203` deadlock guarantee).
    pub const DIALOGUE_UNCOVERED: DwCode = DwCode::every_version("DW0123");
    /// Quest dependency cycle.
    pub const PLAN_CYCLE: DwCode = DwCode::every_version("DW0130");
    /// `finale` is not a declared quest.
    pub const FINALE_UNKNOWN: DwCode = DwCode::every_version("DW0131");
    /// `finale` is not the convergent sink of the plan: some declared quest is
    /// not a transitive dependency of it.
    ///
    /// **The name deliberately does not contain `FINALE_UNREACHABLE`, which
    /// belongs to `DW0201`.** That code says the finale can never complete; this
    /// one says nothing at all about the finale being reachable — in the fixture
    /// that raises it the finale completes perfectly well and a side trip hangs
    /// off the plan. Both are `DwCode`, so nothing but the name distinguishes
    /// them at a call site, and `tools/check-dw-codes.py` credits a bare
    /// constant name mentioned in a crate's tests to **that crate's** code — so
    /// one shared name would buy coverage for whichever rule the file happens to
    /// sit next to.
    pub const PLAN_NOT_CONVERGENT: DwCode = DwCode::every_version("DW0132");
    /// Non-mandatory quest below [`crate::OPTIONAL_QUESTS_SINCE`], where the
    /// surface is reserved.
    ///
    /// **`every_version` deliberately**, and it is the case the [`Binds`]
    /// doctrine names first: this judges what the document SAYS against the
    /// version the document itself declares. Fencing it as `Since(17)` would
    /// *stop rejecting* `mandatory: false` in a 0.12 campaign — the exact
    /// inversion the doctrine warns about.
    pub const NON_MANDATORY: DwCode = DwCode::every_version("DW0133");
    /// An optional quest inside the finale's dependency closure (spec-0051
    /// §8.1) — including a finale that declares itself optional.
    ///
    /// `every_version`: only a document at [`crate::OPTIONAL_QUESTS_SINCE`] can
    /// carry `mandatory: false` at all, so no campaign below it can reach this
    /// rule and there is nothing to grandfather. Its verdict is a function of
    /// the campaign alone.
    pub const OPTIONAL_ON_SPINE: DwCode = DwCode::every_version("DW0866");
    /// A mandatory quest whose `depends_on` edge or stage-5 `quest-complete`
    /// trigger names an optional quest (spec-0051 §8.2).
    ///
    /// `every_version`, for the same reason as [`OPTIONAL_ON_SPINE`].
    pub const MANDATORY_ON_OPTIONAL: DwCode = DwCode::every_version("DW0867");
    /// A mandatory objective gated on a flag only an optional quest produces
    /// (spec-0051 §8.3) — the mainline key behind participation.
    ///
    /// `every_version`, for the same reason as [`OPTIONAL_ON_SPINE`]. The
    /// participation-minimal replay (`DW0204`) is the compensating stronger
    /// check behind it; this one refuses at the edge so the message can name
    /// the strand.
    pub const MAINLINE_KEY_OPTIONAL: DwCode = DwCode::every_version("DW0868");
    /// Objective `after` cycle.
    pub const AFTER_CYCLE: DwCode = DwCode::every_version("DW0140");
    /// Reserved feature used (reserved enum value or reserved field).
    pub const RESERVED: DwCode = DwCode::every_version("DW0141");
    /// Anchor not provided by the area's bound prefab.
    pub const ANCHOR_UNRESOLVED: DwCode = DwCode::every_version("DW0142");
    /// Item id not in the pinned 1.21.11 registry.
    pub const ITEM_UNKNOWN: DwCode = DwCode::every_version("DW0143");
    /// (spec-0021) An `equipment` or `loot` enchantment id is not in the pinned
    /// 1.21.11 enchantment registry.
    pub const ENCHANTMENT_UNKNOWN: DwCode = DwCode::every_version("DW0433");
    /// (spec-0021) An enchantment level is outside the 1..=255 range vanilla's
    /// `minecraft:enchantments` component can carry.
    pub const ENCHANTMENT_LEVEL: DwCode = DwCode::every_version("DW0434");
    /// (spec-0021) Two `loot` entries target the same anchor, so one would
    /// silently overwrite the other's contents.
    pub const LOOT_DUPLICATE_ANCHOR: DwCode = DwCode::every_version("DW0435");
    /// (spec-0021) A `loot` declaration carries more stacks than the container
    /// it fills has slots.
    pub const LOOT_TOO_MANY_ITEMS: DwCode = DwCode::every_version("DW0432");
    /// A single-slot fill's `count` exceeds the item's `minecraft:max_stack_size`
    /// in the pinned 1.21.11 registry. `item replace … container.<n> with <item>
    /// <count>` fails **silently** above the cap, shipping an empty slot.
    pub const ITEM_COUNT_OVER_STACK: DwCode = DwCode::every_version("DW0436");
    /// An `interact` declares `missing_item_hint` without a `requires_item`: the
    /// hint answers a gate that does not exist, so it could never narrate.
    pub const MISSING_ITEM_HINT_WITHOUT_ITEM: DwCode = DwCode::every_version("DW0437");
    /// Planned quest (stage 4) has no expansion in stage 5.
    pub const QUEST_NOT_EXPANDED: DwCode = DwCode::every_version("DW0150");
    /// Stage-5 quest is not planned in stage 4.
    pub const QUEST_NOT_PLANNED: DwCode = DwCode::every_version("DW0151");
    /// Stage-2 NPC has no stage-6 dialogue tree.
    pub const NPC_WITHOUT_TREE: DwCode = DwCode::every_version("DW0152");
    /// Stage-6 dialogue tree references an NPC not declared in stage 2.
    pub const TREE_WITHOUT_NPC: DwCode = DwCode::every_version("DW0153");
    /// Area binds neither or both of `prefab` / `prefab_pool` (exactly one
    /// required).
    pub const PREFAB_BINDING: DwCode = DwCode::every_version("DW0160");
    /// Area `prefab_pool` references a pool absent from `prefabs/` metadata.
    pub const POOL_UNKNOWN: DwCode = DwCode::every_version("DW0161");
    /// Area `prefab` names a piece absent from `prefabs/` metadata — the same
    /// obligation [`POOL_UNKNOWN`] carries on the other arm of the binding. It
    /// is an error rather than a deferral because an area whose piece is absent
    /// contributes no anchor set at all, so every per-area anchor proof over it
    /// is SKIPPED rather than failed: a misspelling here is strictly less
    /// checked than a correct name.
    pub const PREFAB_UNKNOWN: DwCode = DwCode::every_version("DW0856");
    /// (v0.6, spec-0017) A stage-7 edit script is structurally invalid: an edit
    /// names a region no earlier `select` in its batch defined, a composition
    /// (`union`/`intersect`/`subtract`) lists too few regions, a box `min`
    /// exceeds `max` on an axis, a surface band's `from` exceeds `to`, a palette
    /// recipe is empty / carries a non-positive or non-finite weight or `scale`,
    /// a `matching` list is empty, or a morph `by`/`passes` is 0. (Unknown block
    /// ids in recipes reuse [`BLOCK_UNKNOWN`] / `DW0193`; id-syntax and
    /// duplicate-name violations reuse `DW0110`/`DW0111`.)
    pub const EDIT_INVALID: DwCode = DwCode::every_version("DW0162");
    /// (v0.3) A `kill` objective or `spawn-wave` effect references a `wave/<id>`
    /// not declared in the stage-5 `waves` section (dangling wave reference).
    pub const WAVE_UNKNOWN: DwCode = DwCode::every_version("DW0170");
    /// (v0.3) A declared wave is referenced by a `kill` objective but is never
    /// spawned by any `spawn-wave` effect (referenced-but-never-spawned). A wave
    /// must be spawned by some effect before its kill objective is reachable.
    pub const WAVE_NEVER_SPAWNED: DwCode = DwCode::every_version("DW0171");
    /// (v0.3) A `requires_flags` entry references a `flag/<id>` that no `set-flag`
    /// effect ever produces (dangling flag reference).
    pub const FLAG_UNKNOWN: DwCode = DwCode::every_version("DW0172");
    /// (spec-0016 §1) A wave declares `respawns_on_rest: true` but the campaign
    /// declares no `bonfire` — nothing can ever re-seat it, so the field is a
    /// silent no-op. Either add the bonfire the re-seat is meant to hang off, or
    /// drop the field.
    pub const REST_RESEAT_NO_BONFIRE: DwCode = DwCode::every_version("DW0370");
    /// (spec-0016 §1) The campaign places a `bonfire`
    /// but no class kit declares a `flask`. Resting replenishes the flask to its
    /// declared count; with no flask the rest interaction's whole recovery half
    /// is a no-op and the souls loop has no consumable to spend, so this is a
    /// build error rather than a design choice.
    pub const BONFIRE_NO_FLASK: DwCode = DwCode::every_version("DW0476");
    /// **An item gate a class cannot bring.** An objective completes only for a
    /// player holding a named item, and some class's player has no way to be
    /// holding it: the item's only source in the whole campaign is *another*
    /// class's kit, or it has no source at all.
    ///
    /// A delve is played by one to four players who each pick one class, so an
    /// objective reachable only by one class's pick is an objective a party can
    /// be assembled unable to finish — and the party finds out at the thing they
    /// cannot press. Quantified over EVERY class for the same reason
    /// [`BONFIRE_NO_FLASK`] is: one class that cannot bring it is as broken as
    /// none, because a solo player of that class is a supported party.
    pub const ITEM_GATE_UNBRINGABLE: DwCode = DwCode::every_version("DW0849");
    /// (spec-0016 §1) A kit item's potion `contents`
    /// is not something 1.21.11 can pour: declared on an item that carries no
    /// `minecraft:potion_contents` component, empty (neither a named potion nor
    /// an effect), an unknown potion or status-effect id, an amplifier or
    /// duration outside the field vanilla stores it in, a lasting effect with no
    /// `duration`, an instantaneous one *with* a duration, or a malformed
    /// `color`.
    pub const KIT_POTION_INVALID: DwCode = DwCode::every_version("DW0486");
    /// (spec-0016 §1) A potion-bearing kit item
    /// declares no `contents` at `dsl_version` 0.8.0 — the Uncraftable Potion, a
    /// bottle that pours nothing. The placeholder flask, as a build error.
    pub const KIT_POTION_MISSING: DwCode = DwCode::every_version("DW0487");
    /// A `drops[]` `slot` entry does not
    /// name a distinct slot the same entity's `equipment` actually fills — the
    /// slot is empty, or the same slot is declared twice. A mob can only leave
    /// behind a piece it wears, and it can only leave it behind once.
    pub const DROP_SLOT_UNFILLED: DwCode = DwCode::every_version("DW0490");
    /// `drops[]` on an encounter that is
    /// not billed `elite` or `boss`. Only a named fight leaves anything behind;
    /// an ordinary mob's kit is never farmable (no-grind constitution), so the
    /// declaration is refused rather than silently making rank-and-file gear
    /// lootable.
    pub const DROP_NOT_TIERED: DwCode = DwCode::every_version("DW0491");
    /// A `collect` `dropped_by` is not backed by the wave it names:
    /// the wave declares no `{item}` drop of this objective's item, the count
    /// asks for more copies than the wave's mobs can yield, or the objective
    /// also declares a `container` (the item cannot come out of a box *and* off
    /// a body).
    pub const DROP_COLLECT_UNSOURCED: DwCode = DwCode::every_version("DW0492");
    /// A `collect` `dropped_by` is not ordered after the fight that
    /// produces it: no `kill` objective for that wave precedes this collect in
    /// the objective graph. Without that edge "kill the boss, take its key" is
    /// an authoring intention the quest graph cannot prove, and the collect
    /// reads as reachable from the campaign's first tick.
    pub const DROP_COLLECT_UNORDERED: DwCode = DwCode::every_version("DW0493");
    /// (spec-0031, DSL v0.10) A `lethal_volumes[]` entry's `message` is blank.
    ///
    /// The volume would still kill — and would kill in silence, which is the one
    /// thing the declaration exists to prevent. There is no compiler default that
    /// could be right for a cliff, a lava pit and an acid pool at once, so a blank
    /// wording is refused rather than papered over: a gate that reports green
    /// while the player learns nothing is exactly the vacuous pass CLAUDE.md names.
    pub const LETHAL_MESSAGE_BLANK: DwCode = DwCode::every_version("DW0512");
    /// (spec-0031, DSL v0.10) **A grant whose removal is a later effect, not its
    /// own duration.** A `give-effect` is still live at the moment a
    /// `clear-effect` for the same effect fires in the same bundle, so the clear
    /// — not the duration — is what ends it.
    ///
    /// A bundle that does not reach its end leaves the effect on the player: a
    /// logout, a crash, a death mid-chain, a `sequence` whose remaining
    /// `schedule` never runs. A duration expires with no cooperation from
    /// anything, which is why `seconds` is mandatory and why vanilla's `infinite`
    /// is absent from this surface — this diagnostic is what stops the same
    /// hazard being rebuilt out of two effects that are individually fine.
    pub const EFFECT_CLEARED_LIVE: DwCode = DwCode::every_version("DW0540");
    /// (spec-0031, DSL v0.10) A `give-effect`'s `seconds` is zero or beyond
    /// [`crate::MAX_EFFECT_SECONDS`], or its `amplifier` is beyond
    /// [`crate::MAX_POTION_AMPLIFIER`].
    ///
    /// Zero is the grant that never happens — the unbound-vacuity class as a
    /// number. The ceilings are vanilla's own field widths, so a duration typed
    /// in ticks or milliseconds is caught instead of silently overflowing.
    pub const EFFECT_GRANT_BOUNDS: DwCode = DwCode::every_version("DW0541");
    /// (v0.3) A wave mob `entity` is not a known vanilla entity id. (Item-id
    /// checks for `collect.item`, `interact.requires_item` and `give-item.item`
    /// reuse [`ITEM_UNKNOWN`] / `DW0143`.)
    pub const ENTITY_UNKNOWN: DwCode = DwCode::every_version("DW0173");
    /// (i18n) An l10n sidecar does not correctly cover a declared language: the
    /// `l10n/<code>.json` file is absent, its envelope (`campaign_id` / `lang` /
    /// `dsl_version`) is inconsistent, or it is **missing** a key from the
    /// authoritative inventory (under-coverage). English (`en`) is implicit and
    /// never declared, so it is never checked.
    pub const L10N_MISSING: DwCode = DwCode::every_version("DW0180");
    /// (i18n) An l10n sidecar carries an **orphan** key that is not in the
    /// authoritative string inventory derived from the stage docs (over-coverage).
    pub const L10N_ORPHAN: DwCode = DwCode::every_version("DW0181");
    /// (i18n / harness oracle) A player-visible string — authored English or any
    /// sidecar translation — contains the reserved completion-marker sigil
    /// `[dw:complete`. That chat sequence is the validation bot's per-objective
    /// completion oracle; content carrying it could forge a passing critical-path
    /// step. The channel is reserved, not merely conventional.
    pub const MARKER_RESERVED: DwCode = DwCode::every_version("DW0182");
    /// (i18n v2) A player-visible string — authored English or any sidecar
    /// translation — contains a character from the reserved private-use block the
    /// compiler uses to carry an l10n key from the stage docs to the text
    /// component it is emitted into ([`crate::l10n::TR_SIGIL`]). Content carrying
    /// it could impersonate a translation tag, or survive into the datapack and
    /// render as a tofu box. The block is reserved, not merely conventional.
    pub const TR_SIGIL_RESERVED: DwCode = DwCode::every_version("DW0183");
    /// (i18n v2) A declared language has no entry in the Minecraft language-code
    /// mapping table ([`crate::l10n::mc_lang_code`]), so the resource pack has no
    /// filename to write its `assets/delvewright/lang/<code>.json` under. A
    /// language is never silently dropped: either the code is corrected to a
    /// mapped one, or the table gains the entry.
    pub const LANG_CODE_UNMAPPED: DwCode = DwCode::every_version("DW0184");
    /// (i18n v2) A campaign l10n sidecar defines a key in the reserved
    /// `delvewright.` **chrome** namespace ([`crate::chrome`]). Those are the
    /// engine's own on-screen strings — `New objective: `, `Choose your class`,
    /// the default a bonfire shows — owned by the compiler, translated with it,
    /// and authored by no campaign; a sidecar row under that prefix would be
    /// written into the language file and silently replace product chrome for that
    /// language. The namespace is reserved, not merely conventional.
    pub const CHROME_RESERVED: DwCode = DwCode::every_version("DW0186");
    /// (i18n v2) An l10n sidecar row was translated from English the campaign no
    /// longer holds: its `source` entry differs from the key's canonical English.
    /// The translation is present, applied and **wrong**, and no key-set check can
    /// see it — `DW0180`/`DW0181` compare key SETS, and a rewritten line moves no
    /// key. Load-bearing for entity display names, whose key belongs to the first
    /// site declaring a given text, so renaming one body can migrate a key to
    /// another body and the row that goes stale is not the one the author edited.
    pub const L10N_STALE: DwCode = DwCode::every_version("DW0187");
    /// (i18n v2) An l10n sidecar records provenance for only some of its rows (or
    /// none), so `DW0187` cannot see the rest. A warning, not an error: the
    /// `source` map is additive, and this is the one-version deprecation window
    /// before it is required. It states the unguarded row count, so an
    /// unadopted sidecar is a reported number on every run rather than silence
    /// that reads like a pass.
    pub const L10N_PROVENANCE_MISSING: DwCode = DwCode::every_version("DW0188");
    /// (v0.4) A mannequin NPC `skin.texture_id` is malformed (not a bare kebab
    /// token) or duplicated across NPCs (spec-0009). A missing `model` is a
    /// schema error (`DW0100`); a missing PNG is a build error (`DW0309`).
    pub const SKIN_INVALID: DwCode = DwCode::every_version("DW0190");
    /// (v0.4) A `talk-to` objective has no **ungated** reachable completing
    /// dialogue option — every completing option is `requires_flags`-gated, so
    /// the objective can deadlock the moment it activates (spec-0008 §1). Keep at
    /// least one ungated completing path.
    pub const DIALOGUE_FLAG_DEADLOCK: DwCode = DwCode::every_version("DW0191");
    /// (v0.4) A wave mob `effects[].effect` is not a known 1.21.11 effect id.
    pub const EFFECT_UNKNOWN: DwCode = DwCode::every_version("DW0192");
    /// (v0.4) A `set-block` / `interact.prop` block id is not a known 1.21.11
    /// block id.
    pub const BLOCK_UNKNOWN: DwCode = DwCode::every_version("DW0193");
    /// (v0.4) An environment trigger id is malformed (`DW0110`-style) or
    /// duplicated within the stage-5 `triggers` namespace.
    pub const TRIGGER_INVALID: DwCode = DwCode::every_version("DW0194");
    /// (v0.4) A dialogue `talk-to` or `interact` objective targets an NPC after a
    /// `despawn-npc` removes it on a reachable path (spec-0008 §5).
    pub const NPC_DESPAWNED_REF: DwCode = DwCode::every_version("DW0195");
    /// (v0.5) An area `lighting.min_light` is out of the 1..=14 range (spec-0010).
    pub const LIGHTING_RANGE: DwCode = DwCode::every_version("DW0196");
    /// (v0.6) A stage-2 NPC declares `deferred: true` but **no** `spawn-npc` effect
    /// anywhere in the campaign ever summons it — the NPC never enters the world,
    /// so its dialogue tree and any `talk-to` on it are unreachable content. The
    /// NPC-lifecycle dual of [`NPC_DESPAWNED_REF`] / `DW0195`.
    ///
    /// (0197/0198 were *reserved* by spec-0011's draft and released when that spec
    /// renumbered to `DW0340`/`DW0341`; they were never emitted by any code.)
    pub const NPC_NEVER_SPAWNED: DwCode = DwCode::every_version("DW0197");
    /// (v0.6) A `talk-to` on a `deferred` NPC activates before the NPC can exist:
    /// every `spawn-npc` for it sits in a quest that is a strict *descendant* of the
    /// objective's quest on the stage-4 DAG (and none fires from a trigger or
    /// dialogue), so the objective provably activates on an empty anchor.
    pub const NPC_SPAWNED_LATE: DwCode = DwCode::every_version("DW0198");
    /// (v0.6) A `cutscene` effect's shape is invalid: it mixes the multi-shot
    /// `shots` list with the single-shot `path`/`seconds` fields, gives neither,
    /// or declares a shot with an empty camera `path`. A cutscene must resolve to
    /// at least one shot, and every shot to at least one camera position.
    pub const CUTSCENE_SHAPE: DwCode = DwCode::every_version("DW0199");

    /// (v0.6) `horizon: "ocean"` declared without a `boundary` (spec-0013):
    /// validation-tier (exit 1). An infinite swimmable sea with no return rule is
    /// an authoring error. Grouped in the DW032x world/region family by domain;
    /// unlike the compiler-tier DW030x geometry codes it is raised at DSL
    /// validation, so it exits 1.
    pub const OCEAN_NO_BOUNDARY: DwCode = DwCode::every_version("DW0320");
    /// (v0.6) `boundary.margin` outside the `0..=64` range (spec-0013):
    /// validation-tier (exit 1).
    pub const BOUNDARY_MARGIN: DwCode = DwCode::every_version("DW0321");
    /// A stage-1 `horizon` param is out of range, or is a param of a base other
    /// than the one declared (spec-0026): validation-tier (exit 1).
    ///
    /// `Since(16)` because it can only fire on a declaration the surface below
    /// 0.16.0 has no spelling for — the two names that predate the horizon
    /// library carry no params at all, so a campaign that never opted in binds
    /// zero of this.
    pub const HORIZON_PARAM: DwCode = DwCode::since("DW0853", 16);
    /// A `horizon` whose base BUILDS terrain, on a campaign that states no
    /// extent for that terrain to stand around (spec-0026): validation-tier
    /// (exit 1).
    ///
    /// A surround rings a declared extent — a site plan's `region`. A campaign
    /// that seats its pieces with `areas[]` declares none, and the union of
    /// whatever gets placed is not a substitute: it is an artifact of the
    /// compiler's fixed area stride, mostly the void between areas, so ringing
    /// it builds a mountain range around empty space.
    ///
    /// `Since(16)` for the same reason as `DW0853`: only a base introduced with
    /// the horizon library can build terrain, so a campaign that never opted in
    /// binds zero of this.
    pub const SURROUND_NO_REGION: DwCode = DwCode::since("DW0855", 16);
    /// (v0.6) A `sequence` effect is nested inside another `sequence` (directly, or
    /// reachable via a nested `move-actor` `on_arrive`) — timelines do not recurse
    /// (spec-0014). Flatten the inner steps into the outer timeline.
    pub const NESTED_SEQUENCE: DwCode = DwCode::every_version("DW0329");

    /// (v0.6) Trap declaration structurally invalid (spec-0011): a malformed or
    /// duplicated `trap/<id>`, an `at`/`disarm.via` that no area's prefab provides,
    /// or a trap whose `disarm.via` collides with its own trigger anchor.
    /// Validation-tier (exit 1). Renumbered off the spec's stale reserved number
    /// (0197 — since taken).
    pub const TRAP_INVALID: DwCode = DwCode::every_version("DW0340");
    /// (spec-0016 §2) A `shortcut` declaration is structurally invalid: a
    /// malformed or duplicate `shortcut/<id>`, a `gate`/`unlock` anchor no area's
    /// prefab provides, or a `gate` that IS the `unlock` (the mechanism must sit
    /// on the far side, not in the doorway).
    pub const SHORTCUT_INVALID: DwCode = DwCode::every_version("DW0371");
    /// (spec-0016 §2) A `close-gate` effect targets a gate a `shortcut` owns.
    /// A shortcut opens **permanently** — that is the whole pattern — so its
    /// permanence is structural: there is no verb that can put it back. Use a
    /// different gate for the point-of-no-return beat.
    pub const SHORTCUT_RESEALED: DwCode = DwCode::every_version("DW0372");
    /// (spec-0016 §3) An `ambush` declaration is structurally invalid: a
    /// malformed or duplicate `ambush/<id>`, an empty `actors` list (an ambush
    /// that ambushes nobody), or the same actor listed twice (the second
    /// `spawn-actor` is a guarded no-op, so the author's intent silently halves).
    /// The telegraph is deliberately NOT required — an un-telegraphed ambush is
    /// core souls vocabulary.
    pub const AMBUSH_INVALID: DwCode = DwCode::every_version("DW0375");
    /// (spec-0016 §4) A `timed-gate` declaration is structurally invalid: a
    /// malformed or duplicate `timed-gate/<id>`, an `open_ticks` or
    /// `closed_ticks` of 0 (a gate that never opens, or never closes — neither is
    /// a timing gate), a `phase` at or beyond the full cycle, or a gate another
    /// `timed-gate` or a `shortcut` already owns (two clocks fighting over one
    /// region, or a clock fighting a permanent open), or a `disarm.via` anchor no
    /// area's prefab provides / one that IS the gate anchor (the jam lever cannot
    /// live inside the span it stops).
    pub const TIMED_GATE_INVALID: DwCode = DwCode::every_version("DW0377");
    /// A `close-gate` effect targets the gate of a `timed-gate` that
    /// declares a `disarm`. A disarm suppresses the clock **permanently with the
    /// gate resting open** — a jammed portcullis stays up — so, exactly like a
    /// `shortcut` (`DW0372`), its permanence is structural: there is no verb that
    /// can re-arm it. Use a different gate for the beat that must re-seal, or drop
    /// the `disarm`.
    pub const TIMED_GATE_REARMED: DwCode = DwCode::every_version("DW0389");
    /// (spec-0016 §6) A wave's TD `lane` / `summon` declaration is structurally
    /// invalid or internally contradictory: an empty `waypoints` list, a
    /// waypoint anchor no area's prefab provides, a repeated consecutive
    /// waypoint, an `aggro_radius` outside `4..=64`, a mob whose
    /// `attributes.follow_range` disagrees with `aggro_radius` (they MUST be
    /// equal — a patrolling raider holds ground against a target it cannot
    /// engage), or `lane` together with `summon: aggro-edge` (a lane IS the
    /// routing; aggro-edge is its opposite).
    pub const LANE_INVALID: DwCode = DwCode::every_version("DW0381");
    /// (spec-0016 §6) A lane wave contains a non-raider species. `Patrolling` /
    /// `patrol_target` are Raider NBT: on anything else they are dropped and the
    /// mob simply stands where it spawned. The admitted set is vanilla's own
    /// `#minecraft:raiders` tag, read from the vendored tag table — never a
    /// species list this engine keeps. Non-raiders use `summon: aggro-edge`
    /// instead.
    pub const LANE_NOT_RAIDER: DwCode = DwCode::every_version("DW0382");
    /// (spec-0016 §6) A lane wave fields fewer than 2 mobs. A lone patroller
    /// sets `Patrolling:0b` on itself when it finds no companion within its
    /// follow range (vanilla), so a one-mob lane cancels itself.
    pub const LANE_SQUAD_TOO_SMALL: DwCode = DwCode::every_version("DW0383");
    /// (spec-0016 §6) A lane `pillager` is not holding a crossbow. Its only
    /// attack goal is the crossbow goal, so a pillager that acquires a target it
    /// has no runnable attack for freezes in place indefinitely — patrol blocked
    /// by the target, nothing to run instead (live-verified deadlock).
    pub const LANE_UNARMED: DwCode = DwCode::every_version("DW0384");
    /// (spec-0016 §6) A `summon: aggro-edge` wave mob declares no
    /// `attributes.follow_range`. That radius IS the summon ring — the distance
    /// at which the mob perceives the party — so it is authored, never guessed
    /// from a vanilla defaults table the compiler cannot verify.
    pub const AGGRO_EDGE_NO_RANGE: DwCode = DwCode::every_version("DW0385");
    /// (v0.6) A trap dispense-payload item id is not in the pinned 1.21.11 registry
    /// (spec-0011; mirrors `DW0143`). Validation-tier (exit 1). Renumbered off the
    /// spec's stale reserved number (0198 — since taken).
    pub const TRAP_PAYLOAD_UNKNOWN: DwCode = DwCode::every_version("DW0341");

    /// (spec-0022) A trap declares **no consequence at all**: neither the legacy
    /// redstone `effect` nor a command `payload`. A trap that does nothing is
    /// mute hardware the completability proofs would nonetheless reason about,
    /// so it is a content mistake, not a no-op. Validation-tier (exit 1).
    pub const TRAP_NO_CONSEQUENCE: DwCode = DwCode::every_version("DW0440");
    /// (spec-0022) A `volley` `projectile` / `collapse` `falling_block` /
    /// `then_floor` id is not in the pinned 1.21.11 registry (a `projectile`
    /// must be an ENTITY id, the collapse blocks BLOCK ids).
    /// Validation-tier (exit 1).
    pub const TRAP_VERB_ID_UNKNOWN: DwCode = DwCode::every_version("DW0441");
    /// (spec-0022) A `volley`'s `salvos` / `interval` is out of range (`salvos`
    /// in `1..=16`, `interval` in `1..=200`). A volley fires its whole kill zone
    /// every salvo, so the entity count is `salvos x cells`; and salvos spread
    /// wider than the interval cap stop reading as one trap event.
    /// Validation-tier (exit 1).
    pub const VOLLEY_CADENCE: DwCode = DwCode::every_version("DW0443");

    /// (v0.6) A `shot_style` declaration is semantically invalid (spec-0015 shot
    /// grammar): a styled shot with no `subject`; style-only fields (`subject`,
    /// `subject_b`, `dist`, `degrees`, `bearing`) on an unstyled shot; a
    /// `subject_b` on a style other than `two-shot` (or a `two-shot` without
    /// one); `degrees` off `orbit-arc` or outside `45..=120`; `dist` outside
    /// `1..=48`; or `bearing` outside `-360..=360`. Validation-tier (exit 1).
    pub const SHOT_STYLE_INVALID: DwCode = DwCode::every_version("DW0348");
    /// (v0.6) A `side-track` / `low-follow` shot whose subject has no
    /// compiler-known motion: those styles dolly *with* a moving subject, so the
    /// subject must be an NPC/actor with a matching `move-npc`/`move-actor` in
    /// the same effect group or the same `sequence` timeline (an `anchor`
    /// subject can never move). Validation-tier (exit 1). Use `locked-off` /
    /// `push-in` for a static subject instead.
    pub const SHOT_SUBJECT_UNMOVED: DwCode = DwCode::every_version("DW0349");

    /// (v0.4, added round-6) A `use` trigger anchored where an NPC stands.
    /// Right-click on an NPC already belongs to its dialogue advancement; a
    /// second interaction hitbox in the same cell makes the client's entity
    /// ray-pick ambiguous, and whichever entity loses the tie is silently dead
    /// — the round-6 island soft-lock class (an exactly co-located hitbox
    /// starved the giant's dialogue of every right-click). `strike` triggers
    /// are exempt: a left-click has no dialogue meaning, so the compiler rides
    /// the trigger's tag on the NPC's own hitbox instead of summoning a second
    /// one. Validation-tier (exit 1).
    pub const USE_TRIGGER_ON_NPC: DwCode = DwCode::every_version("DW0350");

    /// (v0.6, spec-0018) `world.min_players` outside the `1..=4` range. A delve is
    /// played by ONE party of 1–4 (ADR/CLAUDE.md product definition), so a declared
    /// mandatory party size can never sit outside it. Validation-tier (exit 1).
    pub const PARTY_SIZE: DwCode = DwCode::every_version("DW0356");
    /// (v0.6, spec-0018) A `carrier: "one"` `give-item` sits in a bundle that is
    /// only ever reached from the **scheduler** (`move-npc`/`move-actor`
    /// `on_arrive`, a `sequence` step). `carrier: "one"` means "hand this single
    /// quest prop to the player whose action earned it"; a scheduled bundle runs
    /// with the server command source and has no acting player, so there is no
    /// defensible recipient. Give it to the whole party (drop `carrier`), or move
    /// the hand-off onto the beat that a player completes. Validation-tier (exit 1).
    pub const PARTY_CARRIER_SCHEDULED: DwCode = DwCode::every_version("DW0357");

    /// (v0.6) `world.difficulty` is `peaceful`. On
    /// peaceful the server discards every hostile-category mob as it is ticked —
    /// `/summon`ed, `NoAI`, `PersistenceRequired`, all of it — so a peaceful delve
    /// is one in which every wave, every hostile actor and every ambush silently
    /// ceases to exist. There is no delve that wants that, so the keyword is
    /// refused rather than honoured. Validation-tier (exit 1).
    pub const DIFFICULTY_INVALID: DwCode = DwCode::every_version("DW0468");
    /// (v0.6) A campaign fields scripted `actors[]` (an
    /// ambush desugars into these too) but **no** `waves[]` and no declared
    /// `world.difficulty`, so the compiler's historical derivation ships
    /// `difficulty=peaceful` — under which every one of those actors that is a
    /// hostile species is discarded on the tick it spawns. The compiler cannot
    /// decide the question for the author: the pinned entity registry is a
    /// membership set with no mob-category data, so "is this actor a monster" is
    /// not something it can verify rather than guess. Advisory (warning,
    /// exit 0) — declaring `world.difficulty` settles it either way.
    pub const DIFFICULTY_UNDECLARED_ACTORS: DwCode = DwCode::every_version("DW0469");
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
    pub const BOSS_RESPAWNS_ON_REST: DwCode = DwCode::every_version("DW0499");

    // -- DSL v0.10 runtime state (spec-0031) ---------------------------------

    /// (v0.10, spec-0031) A `state/<kebab>` reference — in a `requires_state`
    /// comparison or in a `set-state`/`add-state`/`clear-state` verb — names a
    /// datum the campaign never declares in the stage-5 `state` list. Unlike a
    /// flag, a datum IS declared: its scope and its initial value are facts no
    /// use site can supply, so an undeclared reference is not "a datum that
    /// happens to start at zero", it is a datum with no defined multiplayer
    /// semantics at all. Validation-tier (exit 1). Prescription: declare it, or
    /// fix the id.
    pub const STATE_UNDECLARED: DwCode = DwCode::every_version("DW0500");
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
    pub const STATE_NEVER_WRITTEN: DwCode = DwCode::every_version("DW0501");
    /// (v0.10, spec-0031) A declared datum that **no gate anywhere in the
    /// campaign ever reads**. Either some verb writes it and nothing ever asks
    /// (the write is inert — a counter nobody consults), or nothing touches it at
    /// all (a dead declaration). Runtime state exists to be compared against; a
    /// datum with no reader is bookkeeping no player can ever observe.
    /// Validation-tier (exit 1). Prescription: gate something on it with
    /// `requires_state`, or delete the declaration and its writes.
    pub const STATE_NEVER_READ: DwCode = DwCode::every_version("DW0502");
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
    pub const STATE_SCOPE_UNREACHABLE: DwCode = DwCode::every_version("DW0503");
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
    pub const STAKE_STATE_SCOPE: DwCode = DwCode::every_version("DW0520");
    /// (v0.10, spec-0032) A `drop-stake` effect names a stake the campaign never
    /// declares in the stage-5 `stakes` list. Validation-tier (exit 1).
    /// Prescription: declare it, or fix the id.
    pub const STAKE_UNDECLARED: DwCode = DwCode::every_version("DW0521");
    /// (v0.10, spec-0032) A declared stake that **no `drop-stake` effect anywhere
    /// in the campaign ever leaves**. The retention policy, the forfeit rule and
    /// the whole placement table are computed for a mechanism no beat can fire —
    /// a declaration wearing a feature's clothes.
    ///
    /// The same vacuity rule `DW0502` states for a datum with no reader
    /// (CLAUDE.md: *a green gate that binds to nothing is vacuous, not a pass*).
    /// Validation-tier (exit 1). Prescription: drop it from a beat — `on_death`
    /// is the usual one — or delete the declaration.
    pub const STAKE_NEVER_DROPPED: DwCode = DwCode::every_version("DW0522");
    /// (v0.10, spec-0032) A `shops[].offers[]` entry that cannot deliver
    /// anything: it declares no `effects`, so its button is drawn, is pressable,
    /// and does nothing.
    ///
    /// The shop analogue of the invisible-affordance rule: a control the player
    /// can operate must have an observable answer. A refusal counts — an offer
    /// whose only effect is a gated `narrate` saying "you cannot afford that" is
    /// exactly the authored shape spec-0032 asks for. Validation-tier (exit 1).
    /// Prescription: give the offer effects, or delete it.
    pub const SHOP_OFFER_INERT: DwCode = DwCode::every_version("DW0523");
    /// (v0.10, spec-0032) A `forfeit` of kind `proportion` whose `percent` is
    /// above 100 — a death that takes more than the whole purse. Validation-tier
    /// (exit 1). Prescription: 0–100, or use `all`.
    pub const STAKE_FORFEIT_RANGE: DwCode = DwCode::every_version("DW0524");
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
    pub const STATE_READ_AFTER_WRITE: DwCode = DwCode::every_version("DW0527");

    /// (v0.11) **A press answer addressed to a click vanilla cannot attribute.**
    /// A trigger declares `audience: presser` on something other than an
    /// `on: use`.
    ///
    /// `minecraft:player_interacted_with_entity` is the only vanilla criterion
    /// that runs a function as the player who clicked, and it fires on
    /// right-clicks alone. A left-click is recorded in the interaction entity's
    /// `attack` NBT — a UUID no command can become — and an `approach` involves no
    /// click at all. Approximating it (polling the record and assuming the nearest
    /// player) is the downstream folklore CLAUDE.md's no-hack rule excludes, so the
    /// capability is refused rather than faked.
    ///
    /// [`Binds::EveryVersion`]: its verdict is a function of the document alone
    /// — a contradiction between two authored fields on one trigger — and
    /// `audience` itself is unwritable below 0.11.0 (`DW0141`), so no campaign
    /// can go green-to-red on it without being edited.
    pub const TRIGGER_AUDIENCE_UNATTRIBUTABLE: DwCode = DwCode::every_version("DW0427");

    /// (v0.11) **A trigger id in the compiler's reserved `dw-` namespace.** The
    /// compiler synthesizes triggers of its own — today the press answer every
    /// sealed gate and shortcut door gives (`trigger/dw-press-…`) — and two
    /// triggers sharing an id would share one `dw_trig_…` tag and one emitted
    /// function, so one of them would silently disappear. Reserving the prefix
    /// makes the collision impossible by construction instead of improbable.
    ///
    /// [`Binds::EveryVersion`], and deliberately: the collision it prevents is
    /// real at *every* version, because a `close-gate` below 0.11.0 is still
    /// given a synthesized `dw-press-…` answer. It requires the campaign to HAVE
    /// nothing — it forbids one shape of id — so fencing it would be fencing a
    /// wellformedness rule, which [`Binds`] names as the wrong direction.
    pub const TRIGGER_ID_RESERVED: DwCode = DwCode::every_version("DW0428");

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
    ///
    /// [`Binds::Since`] 0.11.0, because it is a tightening — it requires the
    /// campaign to HAVE something it need not have had before. A campaign
    /// authored under the older rule keeps its verdicts and its behaviour: a
    /// door still says nothing, a seal still takes the compiler's canonical
    /// English. The fence is [`crate::fence`]'s, not a private version test:
    /// the check raises the diagnostic and the general fence decides whether it
    /// reaches a verdict.
    pub const SEALED_BODY_UNANSWERED: DwCode = DwCode::since("DW0429", 11);

    /// (v0.11, spec-0034) **A declared locomotion the engine cannot hold the
    /// body to** — today exactly one value, `aquatic`.
    ///
    /// The declaration surface exists so an author can claim a capability and
    /// have the claim PROVEN. `aquatic` is the one
    /// class that carries no exemption and governs no rule: it is a ledger
    /// label the compiler derives from vanilla's own `#minecraft:aquatic` tag.
    /// Declaring it could therefore never change a verdict, so it would always
    /// land in `DW0454` — and a value whose only possible outcome is another
    /// diagnostic is a trap, not a surface.
    ///
    /// The gap it names, stated rather than left to folklore (CLAUDE.md's
    /// no-hack rule): the compiler routes **every** body on standable ground,
    /// and `flooded` cells are impassable and never floor for every body. There
    /// is no water-traversal model for a declaration to feed, so there is
    /// nothing to hold an aquatic claim to. When routing grows one, this
    /// refusal is what has to be deleted to enable the value.
    ///
    /// Error tier, raised in `validate_campaign_with`, so the run ends at the
    /// validation tier (exit 1). Prescription: remove the declaration — a body whose
    /// route crosses water is governed by the flooded-cell rules already, and
    /// the derived aquatic class still reaches the binding ledger.
    ///
    /// [`Binds::EveryVersion`]: it judges what the document SAYS — an authored
    /// value the engine refuses — so its verdict is a function of the campaign
    /// alone and there is nothing to grandfather. The surface that carries the
    /// value is itself fenced per stage at 0.11 by [`RESERVED`], which is where
    /// the version gate belongs; fencing this code as well would only stop
    /// rejecting a bad document, which is the direction [`Binds`] warns about.
    pub const TRAVERSAL_UNPROVABLE: DwCode = DwCode::every_version("DW0455");

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
    /// Error tier, validation (exit 1). [`Binds::EveryVersion`]: it judges an
    /// authored contradiction, a function of the campaign alone.
    pub const GATE_NEVER_OPENS: DwCode = DwCode::every_version("DW0847");
}
