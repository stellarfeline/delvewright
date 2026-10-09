//! Diagnostics: the `--json` shape from spec-0002 and the stable `DW01xx` codes.
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
//!    state that makes them identical. Instances: [`crate::quest::check::QUEST_NOT_EXPANDED`]
//!    when stage 5 is empty (`crate::validate`), `DW0842` at a zero box count
//!    (`compiler::detail`), `DW0826` when more than one thing leaves the region
//!    (`crate::siteplan`).
//! 2. **Defer.** The secondary is a real, separate finding whose NUMBER was
//!    measured against something already refused, so it keeps its own line and
//!    gains a clause naming what it is downstream of. Instances: `DW0818`'s
//!    clause when stage 5 declares no quests (`crate::layout`), and
//!    `crate::siteplan::refused_upstream` on every stage-6 verdict measured
//!    against a seam the site plan wrote and did not resolve.
//!
//! What the rule never does is drop a code's ability to refuse. Folding changes
//! how many lines say a thing, never whether the run stops: every fold above is
//! an error tier that still exits non-zero, and each has a test on both sides —
//! primary present, one line; primary absent, the secondary fires per item as
//! before.

use serde::Serialize;

/// **Which exit status a hard failure carrying this code ends the run with.**
///
/// The question this answers, and the only question it answers: *when this code
/// is what stopped the run, does the process exit 2 or 3?*
///
/// * [`ExitTier::Analysis`] — **exit 2.** The compiler did its job and the
///   CONTENT is the defect: a quest nothing can reach, a room too dark to read,
///   a wave larger than the room it spawns in. The author fixes a campaign
///   document or a prefab; nothing about the engine is wrong.
/// * [`ExitTier::Build`] — **exit 3.** The compiler could not produce a tree it
///   is willing to stand behind: geometry, navigation, the solver, the emitted
///   call graph.
///
/// # Why it lives on the code and not at the call site
///
/// The tier is a property of the RULE — `DW0210` is an analysis-tier refusal
/// wherever it is raised — and it was nonetheless re-derived from the code's
/// SPELLING at three separate places in `delvec`'s `main`, each a copy of
/// `code.id().starts_with("DW02") || code == …` with three named exceptions
/// appended. Three copies of a rule is three chances to update two of them, and
/// the spelling is not the rule: `DW0312`, `DW0313` and `DW0342` are
/// analysis-tier codes whose numbers say otherwise, which is exactly why the
/// exceptions had to be written out by hand in the first place.
///
/// # What a code that never stops a build declares
///
/// Most codes are reported as a [`Diagnostic`] among their phase's findings, and
/// the PHASE decides the exit (`validate` exits 1, `analyze` exits 2). Such a
/// code declares [`ExitTier::Build`], and that is a statement rather than a
/// placeholder: it says that IF this rule ever refuses with a build under way,
/// it stops the build. That is precisely what the string-prefix predicate did
/// for every code it did not recognise, so the declaration is the behaviour,
/// written down where the rule is.
///
/// There is no `Default` and no constructor that leaves it unsaid — a new
/// code cannot be added without answering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum ExitTier {
    /// Exit 2: the content is the defect, not the build.
    Analysis,
    /// Exit 3: the compiler could not produce a tree.
    Build,
}

impl ExitTier {
    /// The process exit status this tier ends the run with.
    ///
    /// The numbers are the CLI's stable contract (`docs/reference/compiler.md`
    /// §1): `0` ok, `1` validation, `2` analysis, `3` build.
    pub const fn exit_status(self) -> u8 {
        match self {
            ExitTier::Analysis => 2,
            ExitTier::Build => 3,
        }
    }
}

/// A stable DW diagnostic code together with the exit tier it stops a run at
/// ([`ExitTier`]) and whose state its verdict is about ([`Subject`]).
///
/// A code is not a string that a check happens to quote; it is a rule with its
/// properties, and they travel with it to every site that raises it. Every
/// rule applies to every document the engine accepts (ADR-0024): there is one
/// `dsl_version`, so a code carries nothing about when it starts binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DwCode {
    id: &'static str,
    tier: ExitTier,
    subject: Subject,
}

impl DwCode {
    /// A rule, with the tier it exits at — see [`ExitTier`].
    pub const fn new(id: &'static str, tier: ExitTier) -> DwCode {
        DwCode {
            id,
            tier,
            subject: Subject::Campaign,
        }
    }

    /// Mark this code an **engine-property notice** — see [`Subject::Engine`]
    /// for the test to apply before choosing it. Chained onto the constructor,
    /// because the two questions are independent: *whose state is it about*,
    /// and *what does it exit with*.
    pub const fn about_the_engine(self) -> DwCode {
        DwCode {
            id: self.id,
            tier: self.tier,
            subject: Subject::Engine,
        }
    }

    /// The stable code string (`DW0180`).
    pub const fn id(self) -> &'static str {
        self.id
    }

    /// Which exit status a hard failure carrying this code ends the run with.
    pub const fn exit_tier(self) -> ExitTier {
        self.tier
    }

    /// Whose state this code's verdict is about.
    pub const fn subject(self) -> Subject {
        self.subject
    }
}

impl Serialize for DwCode {
    /// Serializes as the bare code string: a `DwCode` in a JSON payload is the
    /// `&'static str` it replaced; the tier and the subject are compiler-internal.
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
    /// Whose state this verdict is about, carried over from the [`DwCode`] that
    /// raised it — the key `delvec` groups its output by.
    ///
    /// Not part of the `--json` wire shape (spec-0002 fixes that at `code`,
    /// `severity`, `stage`, `path`, `message`): it decides how the run PRESENTS
    /// a diagnostic, never something a consumer reads off one.
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize)]
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

#[doc(hidden)]
pub use linkme as __linkme;

/// **Every DW code this binary declares**, one entry per [`dw_code!`]
/// declaration, gathered by the linker from wherever the declaration sits.
///
/// The declarations are the one authority: a code is declared by writing it
/// inside `dw_code!`, and that same act registers it, so there is no list to
/// keep beside them. `delvec codes` prints this registry, and
/// `tools/ci/check-dw-codes.py --delvec` holds it equal to the declarations the
/// source spells, in both directions.
#[linkme::distributed_slice]
pub static DECLARED: [Declared];

/// One registered declaration: the code, the constant that names it, and the
/// properties the code carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Declared {
    /// The code string (`DW0944`).
    pub code: &'static str,
    /// The exit tier, or `None` for a bare `&str` code — one raised by a verb
    /// outside the campaign pipeline (`prefab`, `schem`, `render`, the view
    /// arms), whose own exit table decides.
    pub tier: Option<ExitTier>,
    /// Whose state the verdict is about; `None` exactly when `tier` is.
    pub subject: Option<Subject>,
    /// The constant's name.
    pub name: &'static str,
    /// The module that declares it.
    pub module: &'static str,
}

impl Declared {
    #[doc(hidden)]
    pub const fn coded(name: &'static str, module: &'static str, code: DwCode) -> Declared {
        Declared {
            code: code.id,
            tier: Some(code.tier),
            subject: Some(code.subject),
            name,
            module,
        }
    }

    #[doc(hidden)]
    pub const fn bare(name: &'static str, module: &'static str, code: &'static str) -> Declared {
        Declared {
            code,
            tier: None,
            subject: None,
            name,
            module,
        }
    }
}

/// The registry in a stable order — by code, then module, then name — because
/// the linker's order is the link order and promises nothing.
#[must_use]
pub fn declared() -> Vec<Declared> {
    let mut all: Vec<Declared> = DECLARED.iter().copied().collect();
    all.sort_by(|a, b| (a.code, a.module, a.name).cmp(&(b.code, b.module, b.name)));
    all
}

/// **Declare a DW code** — the only way to. Expands to the constant as written
/// and registers it in [`DECLARED`]:
///
/// ```text
/// dw_code! {
///     /// What the rule refuses.
///     pub const <NAME>: DwCode = DwCode::new("<code>", ExitTier::<tier>);
/// }
/// dw_code! { pub const <NAME>: &str = "<code>"; }
/// ```
///
/// The declaration inside the braces is spelled the way
/// `tools/ci/check-dw-codes.py`'s `CONST_RE` reads it, so the source reading
/// and the binary's registry are two readings of one text.
#[macro_export]
macro_rules! dw_code {
    (
        $(#[$meta:meta])*
        $vis:vis const $name:ident : DwCode = DwCode::new($id:literal, ExitTier::$tier:ident)
            $(.$chain:ident())? ;
    ) => {
        $(#[$meta])*
        // `DwCode` and `ExitTier` resolve where the declaration is written, as
        // they did before it was wrapped: the declaring module imports them.
        $vis const $name: DwCode = DwCode::new($id, ExitTier::$tier)$(.$chain())?;
        const _: () = {
            #[$crate::diagnostic::__linkme::distributed_slice($crate::diagnostic::DECLARED)]
            #[linkme(crate = $crate::diagnostic::__linkme)]
            static DECLARED_CODE: $crate::diagnostic::Declared =
                $crate::diagnostic::Declared::coded(stringify!($name), module_path!(), $name);
        };
    };
    (
        $(#[$meta:meta])*
        $vis:vis const $name:ident : &str = $id:literal ;
    ) => {
        $(#[$meta])*
        $vis const $name: &str = $id;
        const _: () = {
            #[$crate::diagnostic::__linkme::distributed_slice($crate::diagnostic::DECLARED)]
            #[linkme(crate = $crate::diagnostic::__linkme)]
            static DECLARED_CODE: $crate::diagnostic::Declared =
                $crate::diagnostic::Declared::bare(stringify!($name), module_path!(), $name);
        };
    };
}

/// The stable validation diagnostic codes (catalogued in
/// `docs/reference/compiler.md` §5).
///
/// Every entry is a [`DwCode`], so every entry states its exit tier — there is
/// no way to add one that does not.
pub mod codes {
    use super::{DwCode, ExitTier};

    crate::dw_code! {
        /// Document does not conform to its stage schema (unknown field / wrong type).
        pub const SCHEMA: DwCode = DwCode::new("DW0100", ExitTier::Build);
    }
    crate::dw_code! {
        /// Unsupported `dsl_version`.
        pub const DSL_VERSION: DwCode = DwCode::new("DW0102", ExitTier::Build);
    }
    crate::dw_code! {
        /// Malformed id syntax (kebab-case / prefix).
        pub const ID_SYNTAX: DwCode = DwCode::new("DW0110", ExitTier::Build);
    }
    crate::dw_code! {
        /// Duplicate id within its namespace.
        pub const ID_DUPLICATE: DwCode = DwCode::new("DW0111", ExitTier::Build);
    }
    crate::dw_code! {
        /// Dangling reference: an id ref does not resolve.
        pub const DANGLING_REF: DwCode = DwCode::new("DW0112", ExitTier::Build);
    }
    crate::dw_code! {
        /// Anchor not provided by the area's bound prefab.
        pub const ANCHOR_UNRESOLVED: DwCode = DwCode::new("DW0142", ExitTier::Build);
    }
    crate::dw_code! {
        /// Item id not in the pinned 1.21.11 registry.
        pub const ITEM_UNKNOWN: DwCode = DwCode::new("DW0143", ExitTier::Build);
    }
    crate::dw_code! {
        /// (spec-0021) A `loot` declaration carries more stacks than the container
        /// it fills has slots.
        pub const LOOT_TOO_MANY_ITEMS: DwCode = DwCode::new("DW0432", ExitTier::Build);
    }
    crate::dw_code! {
        /// (v0.3) A `kill` objective or `spawn-wave` effect references a `wave/<id>`
        /// not declared in the stage-5 `waves` section (dangling wave reference).
        pub const WAVE_UNKNOWN: DwCode = DwCode::new("DW0170", ExitTier::Build);
    }
    crate::dw_code! {
        /// (v0.3) A `requires_flags` entry references a `flag/<id>` that no `set-flag`
        /// effect ever produces (dangling flag reference).
        pub const FLAG_UNKNOWN: DwCode = DwCode::new("DW0172", ExitTier::Build);
    }
    crate::dw_code! {
        /// (spec-0067) An `equipment` piece is declared where the pinned game will
        /// not show it on that body: the body's entity type draws no such piece in
        /// that slot, the item declares a different slot, or the item's allowed
        /// entities exclude the body. One code, three shapes in one message.
        /// Validation-tier (exit 1).
        pub const EQUIPMENT_UNSHOWN: DwCode = DwCode::new("DW0898", ExitTier::Build);
    }
    crate::dw_code! {
        /// (spec-0073 §8.1) **A health bar over a body whose health cannot move**:
        /// an actor declaring `health_bar` that is not `vulnerable` and that no
        /// `unleash-actor` names, so the only body the bar could ever read is an
        /// invulnerable puppet. Validation-tier (exit 1).
        pub const HEALTH_BAR_STILL: DwCode = DwCode::new("DW0909", ExitTier::Build);
    }
    crate::dw_code! {
        /// (spec-0073 §8.2) **A health bar with nothing to title it**: no `title`
        /// (or a blank one), and the fight has no single name of its own — a wave of
        /// two entries or more, or a body with no `name`. Validation-tier (exit 1).
        pub const HEALTH_BAR_UNTITLED: DwCode = DwCode::new("DW0910", ExitTier::Build);
    }
    crate::dw_code! {
        /// (spec-0073 §8.4) **Advisory: a fight billed `boss` declares no
        /// `health_bar`.** Warning tier, never blocking — the build proceeds. Fires
        /// for `boss` only, on a wave or an actor alike; `elite` and `ordinary` are
        /// never named by it.
        pub const HEALTH_BAR_ADVISED: DwCode = DwCode::new("DW0912", ExitTier::Build);
    }
    crate::dw_code! {
        /// (spec-0074 §8.1) **An `on_kill` bundle on a body no player can be credited
        /// with killing**: on a wave no beat spawns (it resolves no area, so it has
        /// no bodies and no kill machinery), or on an actor no `unleash-actor` names
        /// that is not `vulnerable` (its body is `Invulnerable` for the whole delve).
        /// A declaration nothing can exercise is refused. Validation-tier (exit 1).
        pub const ON_KILL_UNREACHABLE: DwCode = DwCode::new("DW0913", ExitTier::Build);
    }
    crate::dw_code! {
        /// (spec-0081 §6) **A celestial time whose shape states nothing a sky can
        /// show.** One rule about one value's shape, four ways to break it: the
        /// object names neither or both of `sun` / `moon`; it states a `phase` where
        /// the moon is below the horizon; `world.time` states no `phase` where the
        /// moon is up; a `set-time`, a design row or a camera states the phase the
        /// world already declares. Validation-tier (exit 1). Prescription: name one
        /// body, remove the phase nobody can see, state the phase the party sees, or
        /// remove the restated phase.
        pub const CELESTIAL_TIME: DwCode = DwCode::new("DW0931", ExitTier::Build);
    }
    crate::dw_code! {
        /// (spec-0088) **A gate that cannot stage a lethal volume.** Two shapes,
        /// refused at the document where they are entered:
        ///
        /// * **`when: {}`** — a stage with no term. An always-live volume is
        ///   spelled by leaving `when` out; an empty gate says nothing and is not
        ///   a stage.
        /// * **A `requires_state` term on a `player`-scoped datum.** A volume's
        ///   liveness is a fact about the place, so its gate is a fact about the
        ///   party: a term one player satisfies and another does not would be a
        ///   pit that kills one body and spares the one beside it, and the sweep's
        ///   entity half has no player to read a per-player score from.
        ///
        /// The remedy is to leave `when` out, or to name a flag or a `party`
        /// datum. Raised by [`crate::validate`] with no world built.
        pub const LETHAL_STAGE_GATE: DwCode = DwCode::new("DW0953", ExitTier::Build);
    }
    crate::dw_code! {
        /// (v0.3) A wave mob `entity` is not a known vanilla entity id. (Item-id
        /// checks for `collect.item`, `interact.requires_item` and `give-item.item`
        /// reuse [`ITEM_UNKNOWN`] / `DW0143`.)
        pub const ENTITY_UNKNOWN: DwCode = DwCode::new("DW0173", ExitTier::Build);
    }
    crate::dw_code! {
        /// (i18n) An l10n sidecar does not correctly cover a declared language: the
        /// `l10n/<code>.json` file is absent, its envelope (`campaign_id` / `lang` /
        /// `dsl_version`) is inconsistent, or it is **missing** a key from the
        /// authoritative inventory (under-coverage). English (`en`) is implicit and
        /// never declared, so it is never checked.
        pub const L10N_MISSING: DwCode = DwCode::new("DW0180", ExitTier::Build);
    }
    crate::dw_code! {
        /// (i18n) An l10n sidecar carries an **orphan** key that is not in the
        /// authoritative string inventory derived from the stage docs (over-coverage).
        pub const L10N_ORPHAN: DwCode = DwCode::new("DW0181", ExitTier::Build);
    }
    crate::dw_code! {
        /// (i18n / harness oracle) A player-visible string — authored English or any
        /// sidecar translation — contains the reserved completion-marker sigil
        /// `[dw:complete`. That chat sequence is the validation bot's per-objective
        /// completion oracle; content carrying it could forge a passing critical-path
        /// step. The channel is reserved, not merely conventional.
        pub const MARKER_RESERVED: DwCode = DwCode::new("DW0182", ExitTier::Build);
    }
    crate::dw_code! {
        /// (i18n v2) A player-visible string — authored English or any sidecar
        /// translation — contains a character from the reserved private-use block the
        /// compiler uses to carry an l10n key from the stage docs to the text
        /// component it is emitted into ([`crate::l10n::TR_SIGIL`]). Content carrying
        /// it could impersonate a translation tag, or survive into the datapack and
        /// render as a tofu box. The block is reserved, not merely conventional.
        pub const TR_SIGIL_RESERVED: DwCode = DwCode::new("DW0183", ExitTier::Build);
    }
    crate::dw_code! {
        /// (i18n v2) A declared language has no entry in the Minecraft language-code
        /// mapping table ([`crate::l10n::mc_lang_code`]), so the resource pack has no
        /// filename to write its `assets/delvewright/lang/<code>.json` under. A
        /// language is never silently dropped: either the code is corrected to a
        /// mapped one, or the table gains the entry.
        pub const LANG_CODE_UNMAPPED: DwCode = DwCode::new("DW0184", ExitTier::Build);
    }
    crate::dw_code! {
        /// (i18n v2) A campaign l10n sidecar defines a key in the reserved
        /// `delvewright.` **chrome** namespace ([`crate::chrome`]). Those are the
        /// engine's own on-screen strings — `New objective: `, `Choose your class`,
        /// the default a bonfire shows — owned by the compiler, translated with it,
        /// and authored by no campaign; a sidecar row under that prefix would be
        /// written into the language file and silently replace product chrome for that
        /// language. The namespace is reserved, not merely conventional.
        pub const CHROME_RESERVED: DwCode = DwCode::new("DW0186", ExitTier::Build);
    }
    crate::dw_code! {
        /// (i18n v2) An l10n sidecar row was translated from English the campaign no
        /// longer holds: its `source` entry differs from the key's canonical English.
        /// The translation is present, applied and **wrong**, and no key-set check can
        /// see it — `DW0180`/`DW0181` compare key SETS, and a rewritten line moves no
        /// key. Load-bearing for entity display names, whose key belongs to the first
        /// site declaring a given text, so renaming one body can migrate a key to
        /// another body and the row that goes stale is not the one the author edited.
        pub const L10N_STALE: DwCode = DwCode::new("DW0187", ExitTier::Build);
    }
    crate::dw_code! {
        /// (i18n v2) An l10n sidecar records provenance for only some of its rows (or
        /// none), so `DW0187` cannot see the rest. A warning, not an error: the
        /// `source` map is additive, and this is the one-version deprecation window
        /// before it is required. It states the unguarded row count, so an
        /// unadopted sidecar is a reported number on every run rather than silence
        /// that reads like a pass.
        pub const L10N_PROVENANCE_MISSING: DwCode = DwCode::new("DW0188", ExitTier::Build);
    }
    crate::dw_code! {
        /// (v0.4, widened by spec-0084, narrowed by spec-0097) An image id a campaign
        /// declares is malformed (not a bare kebab token) — a body's `skin.texture_id`
        /// or a `world.textures[]` row's `id` — or a `world.textures[]` row's `id` is
        /// duplicated. A skin's `texture_id` names a file two bodies may both wear. A missing `model` is a
        /// schema error (`DW0100`); a missing PNG is a build error (`DW0309`).
        pub const SKIN_INVALID: DwCode = DwCode::new("DW0190", ExitTier::Build);
    }
    crate::dw_code! {
        /// (v0.4) A wave mob `effects[].effect` is not a known 1.21.11 effect id.
        pub const EFFECT_UNKNOWN: DwCode = DwCode::new("DW0192", ExitTier::Build);
    }
    crate::dw_code! {
        /// (v0.4) A `set-block` / `interact.prop` block id is not a known 1.21.11
        /// block id.
        pub const BLOCK_UNKNOWN: DwCode = DwCode::new("DW0193", ExitTier::Build);
    }
    crate::dw_code! {
        /// (v0.5) An area `lighting.min_light` is out of the 1..=14 range (spec-0010).
        pub const LIGHTING_RANGE: DwCode = DwCode::new("DW0196", ExitTier::Build);
    }

    crate::dw_code! {
        /// (spec-0091) **A view aimed past the served view distance.** What a
        /// body is farther from than the served radius is never sent to its
        /// client, so a far view is a declaration (`world.view_distance`), and a
        /// thing aimed past it cannot render. Five shapes of one rule: a declared
        /// `view_distance` outside `FLOOR..=CEILING` (validation tier, exit 1,
        /// on `/content/view_distance`); a site-plan sightline or view longer
        /// than the served radius (validation tier, `crate::viewdistance`); a
        /// showcase camera whose subject — the first solid cell on its central
        /// ray, else where that ray enters the loaded scene — is beyond it, and
        /// a cutscene keyframe farther from its aim than it (both build tier,
        /// exit 3, against the assembled world). Prescription, in every shape:
        /// the fewest chunks that serve the distance, or the two ends nearer.
        pub const VIEW_BEYOND_SERVED: DwCode = DwCode::new("DW0956", ExitTier::Build);
    }

    // -- DSL v0.10 runtime state (spec-0031) ---------------------------------

    crate::dw_code! {
        /// (v0.10, spec-0031) A `state/<kebab>` reference — in a `requires_state`
        /// comparison or in a `set-state`/`add-state`/`clear-state` verb — names a
        /// datum the campaign never declares in the stage-5 `state` list. Unlike a
        /// flag, a datum IS declared: its scope and its initial value are facts no
        /// use site can supply, so an undeclared reference is not "a datum that
        /// happens to start at zero", it is a datum with no defined multiplayer
        /// semantics at all. Validation-tier (exit 1). Prescription: declare it, or
        /// fix the id.
        pub const STATE_UNDECLARED: DwCode = DwCode::new("DW0500", ExitTier::Build);
    }

    crate::dw_code! {
        /// An asset's licence is outside the ADR-0013 allowlist, or its record lacks
        /// a field the allowlist's rule for that asset requires. One code for every
        /// asset that records a licence: a prefab catalog card (`delvec prefab`,
        /// `delvec::admit::diag::DW_LICENSE`) and an image a campaign declares in
        /// `world.textures[]` (spec-0084 §6.3, `crate::license`).
        pub const LICENSE_REFUSED: DwCode = DwCode::new("DW0741", ExitTier::Build);
    }
    crate::dw_code! {
        /// (spec-0084 §6.2) A `world.textures[]` row's file is not an image the
        /// named texture can be replaced by: not a PNG, not `k·w₀ × k·h₀` of the
        /// vanilla frame (or `k·w₀ × n·k·h₀` with a sidecar), a sidecar that does not
        /// parse as vanilla's animation metadata, or bytes identical to vanilla's.
        pub const TEXTURE_IMAGE: DwCode = DwCode::new("DW0940", ExitTier::Build);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tier's arithmetic is the CLI's published contract
    /// (`docs/reference/compiler.md` §1), so it is asserted rather than left to
    /// whoever next reads the `match`.
    #[test]
    fn a_tier_maps_to_its_published_exit_status() {
        assert_eq!(ExitTier::Analysis.exit_status(), 2);
        assert_eq!(ExitTier::Build.exit_status(), 3);
    }

    /// A code carries the tier it was declared with, independently of what its
    /// number happens to spell — which is the
    /// whole point of moving the tier off the code's spelling. `DW0312` is the
    /// live instance: a `DW03xx` number that exits 2.
    #[test]
    fn a_code_carries_the_tier_it_declares_not_the_one_its_number_spells() {
        // `let`, not `const`: a `const NAME: DwCode = …` here would be a SECOND
        // diagnostic constant declaring a live code, and `tools/ci/check-dw-codes.py`
        // reads every `crates/**/*.rs` — it refuses one code declared twice, and
        // it is right to. Measured: this test written with `const` reds that gate
        // on all three codes.
        let analysis_spelt_dw03 = DwCode::new("DW0312", ExitTier::Analysis);
        let build_spelt_dw03 = DwCode::new("DW0311", ExitTier::Build);

        assert_eq!(analysis_spelt_dw03.exit_tier(), ExitTier::Analysis);
        assert_eq!(analysis_spelt_dw03.exit_tier().exit_status(), 2);
        assert_eq!(build_spelt_dw03.exit_tier().exit_status(), 3);
    }

    /// A code's properties are independent, and `about_the_engine`
    /// rebuilds the struct field by field — the one place where setting one
    /// could silently reset another. Today it cannot (there is no `Default` and
    /// no struct-update syntax, so an omitted field is a compile error), but
    /// "the compiler would catch it" is a claim about the current shape, and
    /// this is the assertion that survives the shape changing.
    #[test]
    fn marking_a_code_an_engine_notice_keeps_its_tier() {
        let engine_notice = DwCode::new("DW0813", ExitTier::Analysis).about_the_engine();
        assert_eq!(engine_notice.subject(), Subject::Engine);
        assert_eq!(engine_notice.exit_tier(), ExitTier::Analysis);
        assert_eq!(engine_notice.id(), "DW0813");
    }
}
