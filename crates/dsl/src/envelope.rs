//! The stage envelope, the assembled [`Campaign`], and parsing from raw JSON.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::detailplan::DetailPlanContent;
use crate::diagnostic::{Diagnostic, codes};
use crate::ids::CampaignId;
use crate::layout::{GeometryBriefContent, LayoutGraphContent};
use crate::siteplan::SitePlanContent;
use crate::stages::{
    ClassesContent, DialogueContent, NpcsContent, QuestPlanContent, QuestsContent, WorldContent,
    WorldEditsContent,
};

/// The latest `dsl_version` this crate implements (identity / tooling default).
pub const SUPPORTED_DSL_VERSION: &str = "0.16.0";

/// The `dsl_version` that introduces the **`open-way`** effect (spec-0042 §2.4):
/// a campaign opening a placed piece's contingent way, with the geometry, the
/// block and the sign read from the piece's own exported metadata.
///
/// The hand-written name the ledger's uniqueness rules read
/// (`tools/check-version-ledger-uniqueness.py`, rule 6). It replaces the
/// [`RESERVED_DSL_VERSIONS`] row that held this number while the surface was in
/// flight: the reservation and the constant are the same claim at two stages of
/// its life, never two claims, so the row is deleted by the change that defines
/// this constant.
pub const OPEN_WAY_SINCE: &str = "0.12.0";

/// The `dsl_version` at which a campaign may carry the spec-0049 map-pipeline
/// documents: `geometry-brief.json` and `layout-graph.json`.
///
/// The **hand-written name** for `0.13.0`, and the reason it is written rather
/// than derived is the reason [`RESERVED_DSL_VERSIONS`] gives: `is_v13` follows
/// from the number, so two branches claiming `0.13.0` would produce the same
/// anchor and the uniqueness gate would read one claim where there are two. A
/// name an author chose cannot agree by accident.
///
/// This number was previously **reserved** for a surface with no scheduled work,
/// and the row was cancelled rather than renumbered: a campaign built on a site
/// plan cannot reach that surface at all, because spec-0049 §6 makes a campaign
/// carry `areas[]` or a site plan and never both. The number was therefore being
/// held in front of the live line on behalf of a road that is not being taken.
pub const LAYOUT_GRAPH_SINCE: &str = "0.13.0";

/// The `dsl_version` at which a campaign may carry the spec-0049 **site plan**:
/// `site-plan.json`, the geometric embedding of the layout graph and the whole
/// map's design of record.
///
/// The **hand-written name** for `0.14.0`, written rather than derived for the
/// reason [`RESERVED_DSL_VERSIONS`] gives: `is_v14` follows from the number, so
/// two branches claiming `0.14.0` would produce the same anchor and the
/// uniqueness gate would read one claim where there are two. A name an author
/// chose cannot agree by accident.
///
/// It is a version of its own rather than a field added at
/// [`LAYOUT_GRAPH_SINCE`], because a version names one surface: a campaign
/// declaring 0.13.0 states its space as a graph and has no way to embed it,
/// which is exactly the state the ordering wants reachable.
pub const SITE_PLAN_SINCE: &str = "0.14.0";

/// The `dsl_version` at which a campaign may carry the spec-0050 **detail
/// plan**: `detail-plan.json`, the statement of which piece stands in which of
/// the site plan's places.
///
/// The **hand-written name** for `0.15.0`, written rather than derived for the
/// reason [`RESERVED_DSL_VERSIONS`] gives: `is_v15` follows from the number, so
/// two branches claiming `0.15.0` would produce the same anchor and the
/// uniqueness gate would read one claim where there are two. A name an author
/// chose cannot agree by accident.
///
/// A version of its own rather than a field added at [`SITE_PLAN_SINCE`],
/// because a version names one surface: a campaign declaring 0.14.0 states the
/// whole map and has no way to detail a part of it, which is exactly the state
/// the ordering wants reachable.
pub const DETAIL_PLAN_SINCE: &str = "0.15.0";

/// The `dsl_version` at which the stage-1 `horizon` is the spec-0026 **horizon
/// library**: the object form `{base, …params}`, and every shorthand beyond the
/// `"void"`/`"ocean"` pair that predates it.
///
/// The **hand-written name** for `0.16.0`, written rather than derived for the
/// reason [`RESERVED_DSL_VERSIONS`] gives: `is_v16` follows from the number, so
/// two branches claiming `0.16.0` would produce the same anchor and the
/// uniqueness gate would read one claim where there are two. A name an author
/// chose cannot agree by accident.
///
/// It is a version of its own rather than a field added at
/// [`DETAIL_PLAN_SINCE`], because a version names one surface: this one is the
/// ground and the sky a map stands in, which is orthogonal to how the map
/// itself is stated. A campaign declaring 0.15.0 details its places and still
/// has no way to say what surrounds them.
pub const HORIZON_LIBRARY_SINCE: &str = "0.16.0";

/// Every `dsl_version` this crate accepts. Each version is an **additive
/// superset** of the previous: v0.3 added the stage-5 verbs/waves/flags; v0.4
/// (spec-0008) adds dialogue state, props, narration, live-threat tuning, NPC
/// lifecycle + skins, environment triggers, cutscenes and named given items;
/// v0.5 (spec-0010) adds declared world `time`/`weather`, per-area `lighting`
/// (deterministic relight), and the `set-time`/`set-weather` effect verbs; v0.6
/// (spec-0012/spec-0013/spec-0014) adds checkpoints (`set-checkpoint` +
/// `on_respawn`), the stealth-zone verbs (`begin-stealth`/`end-stealth`), the
/// stage-1 `horizon` (`ocean` backdrop) + `boundary` (playable region +
/// return-to-checkpoint enforcement), and the staging surface — the `play-sound`
/// effect and the `narrate` `art` style — alongside the actors/sequence surface
/// from sibling PRs; v0.7 (spec-0020) adds the per-quest `cast` ledger; v0.8
/// (spec-0025) adds declared stage-4 `branch_points`, the per-node `happening`
/// declaration and the named `campaign-complete` `ending`, and (spec-0016 §1
/// owner rulings) the bonfire rest interaction — the `bonfire` effect's
/// authorable option strings and the class-kit `flask`; v0.9 adds
/// declared elite/boss `drops[]` and the `collect` `dropped_by`; v0.10
/// (spec-0031) adds **runtime state** — the stage-5 `state[]` declaration, the
/// `set-state`/`add-state`/`clear-state` verbs and the `requires_state` numeric
/// comparison on every gate consumer — the campaign-wide `on_death` effect
/// root, the bundle that runs at the moment a player dies, and the stage-5
/// `lethal_volumes` declaration; v0.11 adds two surfaces and one obligation —
/// (spec-0034) the per-body `traversal` declaration, what a body can do when it
/// moves, on the stage-2 NPC and the stage-5 actor; the **press-answer lift**, a
/// `narrate` `actionbar` style and a trigger `audience: presser`; and with the
/// lift the one obligation of the version, `DW0429`; v0.12 (spec-0042) adds the
/// **`open-way`** effect — a campaign opening a placed piece's contingent way,
/// whose geometry, block and sign are read from that piece's exported metadata
/// and are unauthorable on the effect; v0.13 (spec-0049) adds the two
/// map-pipeline documents — a `geometry-brief` of named numbers and a
/// `layout-graph` stating the campaign's space as places and connections before
/// any coordinate exists; v0.14 (spec-0049) adds the `site-plan`, the geometric
/// embedding of that graph and the whole map's design of record; v0.15
/// (spec-0050) adds the `detail-plan`, which piece stands in which of those
/// places — a document with no coordinate, no extent and no seam in it, so that
/// a part cannot move the box the whole gave it; v0.16 (spec-0026) generalizes
/// the stage-1 `horizon` into the horizon library — a base and its params
/// rather than one of two names — and lands `valley`, the first base that
/// builds terrain instead of picking a world generator.
/// Older campaigns remain valid and compile byte-identically. A construct
/// introduced in a later version is rejected with `DW0141` in an earlier one.
///
/// # One number, one surface — and a number a spec has taken is not free
///
/// This is the **ledger of the campaign document format**, not an inventory of
/// what this crate happens to have built. A number in it names exactly one
/// surface, and names the same surface in every engine that knows the number.
/// Two changes that each take *the next free number* for a different surface
/// produce two engines that both answer to that number and disagree about what
/// a document declaring it may contain — and then an engine accepts a document
/// declaring a version it "knows" and silently drops the half it does not
/// implement. That is the exact failure the per-stage fence exists to prevent,
/// reintroduced by the fence's own numbering.
///
/// So a number whose surface a **sibling** change introduces is
/// [`RESERVED_DSL_VERSIONS`] here rather than skipped: a skipped number is a
/// free number, and the append-only rule means a skipped number can never be
/// filled afterwards. A reservation is **in the ledger and not accepted** —
/// [`is_supported_version`] refuses it, which is the only loud answer this crate
/// has for a surface it does not implement — and it is deleted by the change
/// that defines the constant it names, in the same edit.
///
/// The shape is `crates/grammar/src/version.rs`'s, transplanted rather than
/// invented, and `tools/check-version-ledger-uniqueness.py` reads both ledgers
/// through one set of rules.
pub const SUPPORTED_DSL_VERSIONS: &[&str] = &[
    "0.2.0", "0.3.0", "0.4.0", "0.5.0", "0.6.0", "0.7.0", "0.8.0", "0.9.0", "0.10.0", "0.11.0",
    "0.12.0", "0.13.0", "0.14.0", "0.15.0", "0.16.0",
];

/// Ledger entries whose surface a **sibling** change introduces: the version,
/// and the name of the fence constant that change defines for it.
///
/// A reservation is what keeps a number from being taken twice while the change
/// that owns it is still in flight, and it is the ledger's only way to say so —
/// prose in a spec is not a claim a machine can read, which is how one number
/// came to be allocated twice by two authors who each checked and each found it
/// free.
///
/// The name is load-bearing in a second way: a campaign `dsl_version`'s
/// implemented anchors are the self-naming `is_vNN` predicates, so two branches
/// adding the same number produce the *same* anchor and the uniqueness gate
/// reads one claim where there are two. A hand-written name is not derivable
/// from the number, so two branches naming one number disagree visibly. That is
/// why `tools/check-version-ledger-uniqueness.py` requires every version a
/// branch **adds** to carry a hand-written name — a reservation row here, or a
/// `*_SINCE` constant when the surface lands.
///
/// # The list is empty, and what that does and does not mean
///
/// It does not mean the mechanism is retired. A reservation still holds a number
/// the moment one is genuinely owed, and every rule above still applies to it.
///
/// What it means is narrower and worth stating, because the alternative reading
/// is that nothing is protecting the next free number. A row belongs here for a
/// surface whose change is **in flight** — that is what "sibling" means, and it
/// is the whole of what a row can do, since a row is a claim in THIS tree and a
/// competing claim lives in a tree this crate cannot see. The last row was held
/// for a surface with no scheduled work, which is a different thing: it stood in
/// front of the live line on behalf of a road that may not be taken, and it made
/// the ledger's own ordinal invariant unsatisfiable for the change that came
/// next. Removing it is not a relaxation — the invariant it broke is asserted
/// again below, unchanged.
///
/// The protection a standing row was approximating lives where it can actually
/// see the other claim: the **allocation scan over every remote ref**, run
/// before a round is dispatched, which is the only instrument that reads a
/// number claimed on a branch that has not merged. `tools/check-version-ledger-
/// uniqueness.py` is the same instrument's automatic half, and it diffs against
/// `origin/main` alone — which is precisely why the scan exists and why the
/// number a round will consume is handed to it rather than chosen by it.
///
/// A surface that lost its number to an adjudication takes a **fresh** one when
/// it lands. Renumbering a standing reservation upward instead works exactly
/// once and becomes a treadmill the moment two rows are pending.
pub const RESERVED_DSL_VERSIONS: &[(&str, &str)] = &[];

/// The fence constant that introduces `version`'s surface, when `version` is a
/// ledger entry this crate does not implement; `None` otherwise.
pub fn reserved_for(version: &str) -> Option<&'static str> {
    RESERVED_DSL_VERSIONS
        .iter()
        .find(|(v, _)| *v == version)
        .map(|(_, anchor)| *anchor)
}

/// Every `dsl_version` this crate will build — the ledger minus its
/// reservations. What a refusal names, because naming the whole ledger would
/// list versions the refusal was issued *for*.
pub fn accepted_versions() -> impl Iterator<Item = &'static str> {
    SUPPORTED_DSL_VERSIONS
        .iter()
        .copied()
        .filter(|v| reserved_for(v).is_none())
}

/// True if `version` is a `dsl_version` this crate accepts: in the ledger, and
/// not reserved for a surface this crate does not implement.
pub fn is_supported_version(version: &str) -> bool {
    SUPPORTED_DSL_VERSIONS.contains(&version) && reserved_for(version).is_none()
}

/// The minor-version ordinal of a supported `dsl_version` (`0.4.0` → 4); `0` for
/// anything this crate does not accept.
///
/// Every version predicate below is `ordinal(version) >= n`, so adding a version
/// to [`SUPPORTED_DSL_VERSIONS`] is the *only* edit a version bump needs. The
/// hand-written `version == "0.4.0" || version == "0.5.0" || …` chains this
/// replaced had to be extended in lockstep in five places; forgetting one made
/// the newest campaigns silently lose an older version's surface.
///
/// Public as [`minor_ordinal`]: the obligation fence ([`crate::fence`]) compares
/// a rule's [`Binds::Since`](crate::Binds::Since) against exactly this number, so
/// "version 0.8.0" means the same thing to a fence as it does to `is_v08`.
///
/// A **reserved** version keeps its own ordinal, so the ledger stays a
/// contiguous sequence — the surface it names sits at that ordinal whether or
/// not this crate implements it. The predicates below are therefore open at a
/// reserved version, and that is not load-bearing: [`is_supported_version`]
/// refuses it, `DW0102` is an error, and an error means no datapack is written
/// at all. There is no path on which a reserved version's document reaches
/// emission with a surface honoured or dropped.
fn ordinal(version: &str) -> u32 {
    match version {
        "0.2.0" => 2,
        "0.3.0" => 3,
        "0.4.0" => 4,
        "0.5.0" => 5,
        "0.6.0" => 6,
        "0.7.0" => 7,
        "0.8.0" => 8,
        "0.9.0" => 9,
        "0.10.0" => 10,
        "0.11.0" => 11,
        "0.12.0" => 12,
        "0.13.0" => 13,
        "0.14.0" => 14,
        "0.15.0" => 15,
        "0.16.0" => 16,
        _ => 0,
    }
}

/// The minor-version ordinal of a supported `dsl_version` (`0.8.0` → `8`); `0`
/// for anything this crate does not accept. The number every `is_v0*` predicate
/// below compares, and the number [`crate::fence`] grandfathers against.
pub fn minor_ordinal(version: &str) -> u32 {
    ordinal(version)
}

/// True if `version` enables the DSL v0.3 verbs (`kill`/`collect`/`interact`,
/// `give-item`/`set-flag`/`spawn-wave`, waves, flags). v0.4 is an additive
/// superset, so it enables the whole v0.3 surface too.
pub fn is_v03(version: &str) -> bool {
    ordinal(version) >= 3
}

/// True if `version` enables the DSL v0.4 surface (spec-0008): dialogue
/// `set-flag` + `requires_flags`, props (`interact.prop`, `set-block`),
/// `narrate`, wave `attributes`/`effects`, `despawn-npc`/`move-npc`, `cutscene`,
/// NPC `skin`, stage-5 `triggers`, named `give-item`, objective `stealth`.
pub fn is_v04(version: &str) -> bool {
    ordinal(version) >= 4
}

/// True if `version` enables the DSL v0.5 surface (spec-0010): declared world
/// `time`/`weather`, per-area `lighting` (deterministic relight fixtures), and
/// the `set-time`/`set-weather` effect verbs. v0.6 is an additive superset.
pub fn is_v05(version: &str) -> bool {
    ordinal(version) >= 5
}

/// True if `version` enables the DSL v0.6 surface: the `set-checkpoint` effect
/// (with its optional `on_respawn` hook) and the `begin-stealth`/`end-stealth`
/// verbs (spec-0012 checkpoints + spec-0014 stealth zones), the stage-1 `horizon`
/// (`ocean`) and `boundary` (playable region) world fields (spec-0013), the
/// `play-sound` effect + `narrate` `art` style (spec-0014), and the stage-5
/// scripted `actors` + staging effects (`spawn`/`despawn`/`move`/`unleash-actor`,
/// `sequence`, spec-0014). Additive over v0.5; a campaign that uses none is
/// byte-identical, and a use of the v0.6 surface in an earlier campaign is
/// rejected with `DW0141`.
pub fn is_v06(version: &str) -> bool {
    ordinal(version) >= 6
}

/// True if `version` enables the DSL v0.7 surface (spec-0020): the per-quest
/// `cast` ledger — for every live stage-2 NPC, where it stands, what it is doing,
/// and what its right-click offers during that quest. Additive over v0.6: a
/// campaign that declares no `cast` compiles byte-identically, and a pre-0.7
/// campaign that declares none keeps building with the `DW0465` deprecation
/// warning for one version window.
pub fn is_v07(version: &str) -> bool {
    ordinal(version) >= 7
}

/// True if `version` enables the DSL v0.8 surface. Two specs land in it:
/// spec-0025's stage-4 `branch_points` declaration, per-node `happening` and
/// named `ending` on `campaign-complete`; and spec-0016 §1's bonfire **rest
/// interaction** — the `bonfire` effect's authorable
/// `prompt` / `rest_label` / `save_label` strings and the stage-3 kit item
/// `flask` marker a rest replenishes. Additive over v0.7 — a campaign that
/// declares none of it compiles byte-identically, and any use of the surface in
/// an earlier campaign is rejected with `DW0141`. The **requirement** side (a
/// story node without a `happening` is `DW0481`; an undeclared fork is `DW0480`)
/// fires only at 0.8.0 and above, which is what keeps every 0.6/0.7 campaign's
/// datapack byte-for-byte unchanged.
pub fn is_v08(version: &str) -> bool {
    ordinal(version) >= 8
}

/// True if `version` enables the DSL v0.9 surface: declared **drops** on an
/// elite/boss — the `drops[]` list on a
/// wave mob and on an actor, and the `collect` `dropped_by` that turns a boss's
/// quest token into a proved link in the quest graph. Additive over v0.8: a
/// campaign that declares none of it compiles byte-identically (every
/// undeclared slot keeps drop chance `0.0`, which is exactly what pre-0.9
/// emission wrote), and any use of the surface in an earlier campaign is
/// rejected with `DW0141`.
pub fn is_v09(version: &str) -> bool {
    ordinal(version) >= 9
}

/// True if `version` enables the DSL v0.10 surface. **Two spec-0031 surfaces
/// land in it**, and they are additive over v0.9 and over each other:
///
/// * §"the missing primitive" — **runtime state**: the stage-5 `state[]`
///   declaration of named, scoped, integer-valued data; the `set-state` /
///   `add-state` / `clear-state` verbs that write one; and `requires_state`, the
///   numeric comparison carried by **every** gate consumer beside
///   `requires_flags` / `forbids_flags`. The datum is what `FlagId` is not: it
///   clears, it counts, and its multiplayer scope is declared rather than
///   assumed. The comparison lives in the gate and not in any one verb, because
///   its consumers are exactly the gate's consumers — a door that opens at 500,
///   a line withheld below 200, a lever inert while a ride is in progress.
///   Generality is decided at the first site (CLAUDE.md); a second bespoke field
///   would be the defect, not the fix.
/// * §`on_death` — the stage-5 campaign-wide `on_death` bundle: the seventh
///   effect root, and the only one that runs while the player who fired it is
///   still a corpse.
/// * §"Lethal volume" — the stage-5 `lethal_volumes[]` declaration: a box that
///   kills whatever enters it, worded by the campaign's own strings. Geometry,
///   so the completability model owns it (`DW0510`/`DW0511`); an ordinary
///   `/damage`, so the death edge above needs no second detector.
///
/// A campaign that declares none of it compiles byte-identically (no new scoreboard
/// objective, no new guard clause, no new function, and the whole
/// `dw.death_seen` half of the death edge absent; no lethal tick call and no
/// navigation cell), and any use of any of it in an earlier campaign is rejected
/// with `DW0141`.
///
/// **The version is `0.10.0`, not `0.9.1`.** `ordinal` matches the literal
/// string, so the ledger is a sequence of minors and a patch would sort nowhere.
pub fn is_v10(version: &str) -> bool {
    ordinal(version) >= 10
}

/// True if `version` enables the DSL v0.11 surface. **Two surfaces land in this
/// version and one obligation rides with them**, and one predicate carries all
/// three.
///
/// # The per-body `traversal` declaration (spec-0034)
///
/// What a body can do when it moves, carried by the stage-2 NPC and the stage-5
/// actor through one shared [`crate::stages::BodyTraversal`] type.
///
/// Spiders really do climb, so the traversal proof's rules cannot be absolute;
/// what was missing was the author's side of that. A declaration is not an
/// exemption: the compiler compares the verdicts the body earns under the
/// declared class against the ones it earns under its species' derived class,
/// and a declaration that changes none of them is `DW0454`. It can never reach
/// the error tier (`DW0452`), because that rule is a collision-and-interaction
/// question with no authorable exemption.
///
/// Purely additive: nothing obliges a body to declare traversal, a campaign that
/// declares none routes exactly as it did before (the derived class is what
/// every pre-0.11 build used), and declaring it in an earlier campaign is
/// rejected with `DW0141`.
///
/// # The press-answer lift
///
/// Two additions, and they are one lift — each alone leaves the general
/// mechanism unable to say what `close-gate`'s private copy said:
///
/// * `narrate` gains the **`actionbar`** style — the reply strip every string the
///   compiler writes itself already used, and the one channel the general effect
///   could not reach;
/// * an environment trigger gains **`audience: presser`** — dispatch by the
///   `player_interacted_with_entity` advancement, so the bundle runs as the
///   player who right-clicked instead of addressing the whole party.
///
/// With both, "a pressable thing answers the player who pressed it" is an
/// ordinary trigger with an ordinary effect, and `close-gate.sealed_hint` stops
/// being a mechanism and becomes what it always was — a wording. Additive: a
/// campaign that declares neither keeps every verdict and every line it showed,
/// and any use of either below 0.11.0 is `DW0141`.
///
/// The surface is additive; the version also carries **one requirement**, and it
/// is fenced rather than reserved. At 0.11.0 and above a sealed body nothing
/// answers is `DW0429` — a `shortcut` door and a `close-gate` wall alike, one
/// rule over the class. That obligation declares itself on its own code
/// ([`crate::Binds::Since`] 11) and is carried by [`crate::fence`], so a campaign
/// below 0.11.0 is grandfathered: its sealed gate answers exactly as it did
/// before, and its silent door stays silent.
///
/// # What "additive" does and does not promise
///
/// A campaign that declares none of the new surface keeps every verdict and
/// every player-facing string it had. It does **not** follow that its datapack
/// is byte-identical: a `close-gate` seal's answer is now emitted through the
/// general trigger path rather than through a private one, so the set of emitted
/// files and identifiers moves even where the line the player reads does not.
/// The fence grandfathers the verdict and the wording; it does not grandfather
/// emitted identifiers. Reproduction of a released delve is the pinned engine's
/// job (`versions.toml` + OCI), not eternal byte-stable emission.
pub fn is_v11(version: &str) -> bool {
    ordinal(version) >= 11
}

/// True if `version` enables the DSL v0.12 surface (spec-0042,
/// [`OPEN_WAY_SINCE`]): the **`open-way`** effect.
///
/// One verb, and the whole of the version. It names a placed piece and one of
/// the ways that piece's spatial contract exports, and it carries **no region,
/// no block and no sign** — all three are read from the piece's own metadata,
/// because two authorities that can disagree is the defect the shape avoids
/// rather than a variant of the fix (spec-0042 AC8). What the effect fills or
/// clears is therefore decided by the building, and the campaign decides only
/// *when*.
///
/// Purely additive: nothing obliges a campaign to open a way, a campaign that
/// declares no `open-way` emits exactly what it emitted before (no fill, no
/// region event, no `validation/ways.json`), and declaring one below 0.12.0 is
/// `DW0141`. The **requirement** half of the surface belongs to the piece, not
/// to the version: a required element standing beyond a way no forced opening
/// precedes is `DW0548` at every version, because a campaign below 0.12.0 has no
/// way to reach that state — it cannot stage an `open-way` at all, and a piece
/// carrying a way it never opens is content (spec-0042 §2.5).
pub fn is_v12(version: &str) -> bool {
    ordinal(version) >= 12
}

/// True if `version` enables the DSL v0.13 surface (spec-0049,
/// [`LAYOUT_GRAPH_SINCE`]): the two **map-pipeline stage documents**, and
/// nothing else.
///
/// * `geometry-brief.json` — the whole map's written brief reduced to numbers, a
///   `facts[]` list a later site plan's identities bind to.
/// * `layout-graph.json` — the campaign's space as a graph: places, the
///   connections between them, the authored critical path, and where each quest
///   beat happens. **No coordinate appears in either.**
///
/// Purely additive, and additive in the strongest sense available: both
/// documents are optional files in a campaign directory, so a campaign that
/// ships neither parses, validates and emits exactly as it did — there is no new
/// field on any existing type for an older document to be judged against. Every
/// check the two documents owe is reached only through the documents themselves,
/// so a campaign without them binds zero of them and says so.
pub fn is_v13(version: &str) -> bool {
    ordinal(version) >= 13
}

/// True if `version` enables the DSL v0.14 surface (spec-0049 §4,
/// [`SITE_PLAN_SINCE`]): the **site plan**, and nothing else.
///
/// `site-plan.json` is the geometric embedding of the layout graph — the
/// region, the datums, a box per place, a seam per connection, the mass the
/// whole itself owns, and the guarded comparisons that hold all of it to the
/// geometry brief's written numbers. It is the whole map's design of record.
///
/// Purely additive, and additive in the same strong sense v0.13 is: the document
/// is an optional file in a campaign directory, so a campaign that ships none
/// parses, validates and emits exactly as it did — there is no new field on any
/// existing type for an older document to be judged against. Every obligation
/// the plan owes is reached only through the plan itself, so a campaign without
/// one binds zero of them and says so.
///
/// The one thing a plan is not free to do is arrive alone: it validates only
/// against a layout graph and a geometry brief (`DW0824`), which is the ordering
/// made uncompilable rather than advised.
pub fn is_v14(version: &str) -> bool {
    ordinal(version) >= 14
}

/// True if `version` enables the DSL v0.15 surface (spec-0050,
/// [`DETAIL_PLAN_SINCE`]): the **detail plan**, and nothing else.
///
/// `detail-plan.json` states which piece fills which of the site plan's places,
/// and which of that piece's anchors answers each name the campaign had already
/// bound to the place. It carries no coordinate, no region, no extent, no datum,
/// no seam and no offset — absent fields, not optional ones — so a part is
/// structurally unable to move the box the whole gave it.
///
/// Purely additive, and additive in the same strong sense v0.13 and v0.14 are:
/// the document is an optional file in a campaign directory, so a campaign that
/// ships none parses, validates and emits exactly as it did. Every obligation
/// the detail plan owes is reached only through the document itself, so a
/// campaign without one binds zero of them and says so.
pub fn is_v15(version: &str) -> bool {
    ordinal(version) >= 15
}

/// True if `version` enables the **horizon library** (spec-0026,
/// [`HORIZON_LIBRARY_SINCE`]): the stage-1 `horizon` as a base plus that base's
/// params — the object form `{base, …params}`, and every string shorthand
/// beyond the `"void"`/`"ocean"` pair that predates it.
///
/// Additive on the wire and additive in emission: a campaign below this version
/// may still write `"void"` or `"ocean"`, those two parse to exactly the bases
/// they always named, and nothing about such a build moves. What the version
/// buys is the ability to say something the old surface had no spelling for —
/// `valley`, whose surround is generated terrain rather than a world-generator
/// setting, and the params that shape it.
pub fn is_v16(version: &str) -> bool {
    ordinal(version) >= 16
}

/// Which stage a document belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Stage {
    /// Stage 1.
    World,
    /// Stage 2.
    Npcs,
    /// Stage 3.
    Classes,
    /// Stage 4.
    QuestPlan,
    /// Stage 5.
    Quests,
    /// Stage 6.
    Dialogue,
    /// Stage 7 (optional; DSL v0.6, spec-0017): the map-editor edit script.
    WorldEdits,
    /// The whole map's written brief, reduced to numbers (optional; DSL v0.13,
    /// spec-0049 §4.2). Named, never renumbered into the 1..7 sequence: it is a
    /// different pipeline's document and the two orderings are unrelated.
    GeometryBrief,
    /// The campaign's space as a graph, before any coordinate exists (optional;
    /// DSL v0.13, spec-0049 §3).
    LayoutGraph,
    /// The geometric embedding of that graph — the whole map's design of record
    /// (optional; DSL v0.14, spec-0049 §4).
    SitePlan,
    /// Which piece stands in which of the plan's places (optional; DSL v0.15,
    /// spec-0050). Named, never renumbered, for the reason `GeometryBrief`
    /// gives.
    DetailPlan,
}

impl Stage {
    /// The wire/filename name (`world`, `npcs`, `classes`, `quest-plan`,
    /// `quests`, `dialogue`).
    pub fn name(self) -> &'static str {
        match self {
            Stage::World => "world",
            Stage::Npcs => "npcs",
            Stage::Classes => "classes",
            Stage::QuestPlan => "quest-plan",
            Stage::Quests => "quests",
            Stage::Dialogue => "dialogue",
            Stage::WorldEdits => "world-edits",
            Stage::GeometryBrief => "geometry-brief",
            Stage::LayoutGraph => "layout-graph",
            Stage::SitePlan => "site-plan",
            Stage::DetailPlan => "detail-plan",
        }
    }

    /// **Every stage, in document order.** The one enumeration.
    ///
    /// Hand-written stage lists are how a new document escapes a gate that was
    /// written before it existed: `crates/dsl/tests/gate_consumers.rs` walked
    /// seven stages by name, so a schema object declaring part of the gate in an
    /// eighth would have been invisible to the check whose whole subject is that
    /// no such object exists. Anything that means "over the stages" reads this.
    pub const ALL: [Stage; 11] = [
        Stage::World,
        Stage::Npcs,
        Stage::Classes,
        Stage::QuestPlan,
        Stage::Quests,
        Stage::Dialogue,
        Stage::WorldEdits,
        Stage::GeometryBrief,
        Stage::LayoutGraph,
        Stage::SitePlan,
        Stage::DetailPlan,
    ];
}

/// A stage document: `{ dsl_version, campaign_id, stage, content }`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Envelope<T> {
    /// DSL version string.
    pub dsl_version: String,
    /// Owning campaign id.
    pub campaign_id: CampaignId,
    /// The stage this document is for.
    pub stage: Stage,
    /// The stage payload.
    pub content: T,
}

/// The parsed stage documents that make up one campaign: the six required
/// stages plus the optional stage-7 edit script (spec-0017).
#[derive(Clone, Debug, PartialEq)]
pub struct Campaign {
    /// Stage 1.
    pub world: Envelope<WorldContent>,
    /// Stage 2.
    pub npcs: Envelope<NpcsContent>,
    /// Stage 3.
    pub classes: Envelope<ClassesContent>,
    /// Stage 4.
    pub quest_plan: Envelope<QuestPlanContent>,
    /// Stage 5.
    pub quests: Envelope<QuestsContent>,
    /// Stage 6.
    pub dialogue: Envelope<DialogueContent>,
    /// Stage 7 (optional; DSL v0.6, spec-0017): the map-editor edit script.
    /// `None` = no `world-edits.json` in the campaign directory — byte-identical
    /// to a campaign from before the stage existed.
    pub world_edits: Option<Envelope<WorldEditsContent>>,
    /// The whole map's brief as numbers (optional; DSL v0.13, spec-0049 §4.2).
    pub geometry_brief: Option<Envelope<GeometryBriefContent>>,
    /// The campaign's space as a graph (optional; DSL v0.13, spec-0049 §3).
    pub layout_graph: Option<Envelope<LayoutGraphContent>>,
    /// The geometric embedding of that graph (optional; DSL v0.14, spec-0049 §4).
    pub site_plan: Option<Envelope<SitePlanContent>>,
    /// Which piece fills which of the plan's places (optional; DSL v0.15,
    /// spec-0050 §1).
    pub detail_plan: Option<Envelope<DetailPlanContent>>,
}

/// The stage documents as raw JSON strings (compiler input): six required, the
/// stage-7 edit script optional.
#[derive(Clone, Debug, PartialEq)]
pub struct RawCampaign {
    /// `world.json`.
    pub world: String,
    /// `npcs.json`.
    pub npcs: String,
    /// `classes.json`.
    pub classes: String,
    /// `quest-plan.json`.
    pub quest_plan: String,
    /// `quests.json`.
    pub quests: String,
    /// `dialogue.json`.
    pub dialogue: String,
    /// `world-edits.json` (optional stage 7, spec-0017); `None` when the
    /// campaign directory ships none.
    pub world_edits: Option<String>,
    /// `geometry-brief.json` (optional; spec-0049 §4.2).
    pub geometry_brief: Option<String>,
    /// `layout-graph.json` (optional; spec-0049 §3).
    pub layout_graph: Option<String>,
    /// `site-plan.json` (optional; spec-0049 §4).
    pub site_plan: Option<String>,
    /// `detail-plan.json` (optional; spec-0050 §1).
    pub detail_plan: Option<String>,
}

fn parse_stage<T: for<'de> Deserialize<'de>>(
    src: &str,
    stage: Stage,
    out: &mut Result<Envelope<T>, ()>,
    diags: &mut Vec<Diagnostic>,
) {
    match serde_json::from_str::<Envelope<T>>(src) {
        Ok(env) => *out = Ok(env),
        Err(e) => {
            *out = Err(());
            diags.push(Diagnostic::error(
                codes::SCHEMA,
                stage.name(),
                "",
                format!(
                    "`{}` stage document does not conform to its schema: {e}. Fix the offending \
                     field (unknown field, wrong type, or missing required one) in the campaign \
                     JSON to match the schema — run `delvec schema --stage <1..7>` to see the \
                     exact shape.",
                    stage.name()
                ),
            ));
        }
    }
}

/// Parse all six stage documents.
///
/// On any schema/parse failure returns every `DW0100` diagnostic collected
/// (validation cannot run on unparseable input).
pub fn parse_campaign(raw: &RawCampaign) -> Result<Campaign, Vec<Diagnostic>> {
    let mut diags = Vec::new();
    let mut world = Err(());
    let mut npcs = Err(());
    let mut classes = Err(());
    let mut quest_plan = Err(());
    let mut quests = Err(());
    let mut dialogue = Err(());

    parse_stage(&raw.world, Stage::World, &mut world, &mut diags);
    parse_stage(&raw.npcs, Stage::Npcs, &mut npcs, &mut diags);
    parse_stage(&raw.classes, Stage::Classes, &mut classes, &mut diags);
    parse_stage(
        &raw.quest_plan,
        Stage::QuestPlan,
        &mut quest_plan,
        &mut diags,
    );
    parse_stage(&raw.quests, Stage::Quests, &mut quests, &mut diags);
    parse_stage(&raw.dialogue, Stage::Dialogue, &mut dialogue, &mut diags);
    // The optional stage-7 edit script (spec-0017): absent = `None`; present but
    // unparseable = a `DW0100` like any other stage.
    let mut world_edits: Result<Option<Envelope<WorldEditsContent>>, ()> = Ok(None);
    if let Some(src) = &raw.world_edits {
        let mut parsed = Err(());
        parse_stage(src, Stage::WorldEdits, &mut parsed, &mut diags);
        world_edits = parsed.map(Some);
    }
    // The spec-0049 map-pipeline documents, on the same terms: absent = `None`
    // and a campaign that ships neither is byte-identical to one from before
    // they existed; present = parsed, validated and hashed like any other stage.
    let mut geometry_brief: Result<Option<Envelope<GeometryBriefContent>>, ()> = Ok(None);
    if let Some(src) = &raw.geometry_brief {
        let mut parsed = Err(());
        parse_stage(src, Stage::GeometryBrief, &mut parsed, &mut diags);
        geometry_brief = parsed.map(Some);
    }
    let mut layout_graph: Result<Option<Envelope<LayoutGraphContent>>, ()> = Ok(None);
    if let Some(src) = &raw.layout_graph {
        let mut parsed = Err(());
        parse_stage(src, Stage::LayoutGraph, &mut parsed, &mut diags);
        layout_graph = parsed.map(Some);
    }
    let mut site_plan: Result<Option<Envelope<SitePlanContent>>, ()> = Ok(None);
    if let Some(src) = &raw.site_plan {
        let mut parsed = Err(());
        parse_stage(src, Stage::SitePlan, &mut parsed, &mut diags);
        site_plan = parsed.map(Some);
    }
    let mut detail_plan: Result<Option<Envelope<DetailPlanContent>>, ()> = Ok(None);
    if let Some(src) = &raw.detail_plan {
        let mut parsed = Err(());
        parse_stage(src, Stage::DetailPlan, &mut parsed, &mut diags);
        detail_plan = parsed.map(Some);
    }

    match (
        world,
        npcs,
        classes,
        quest_plan,
        quests,
        dialogue,
        world_edits,
        geometry_brief,
        layout_graph,
        site_plan,
        detail_plan,
    ) {
        (
            Ok(world),
            Ok(npcs),
            Ok(classes),
            Ok(quest_plan),
            Ok(quests),
            Ok(dialogue),
            Ok(world_edits),
            Ok(geometry_brief),
            Ok(layout_graph),
            Ok(site_plan),
            Ok(detail_plan),
        ) => {
            let mut campaign = Campaign {
                world,
                npcs,
                classes,
                quest_plan,
                quests,
                dialogue,
                world_edits,
                geometry_brief,
                layout_graph,
                site_plan,
                detail_plan,
            };
            // spec-0016 §3: expand the `ambush` sugar into real environment
            // triggers, ONCE, at the DSL boundary. Every downstream consumer —
            // validation, l10n, the flow producer scans, nav, emission — then
            // sees the same `triggers` list it always has, so the sugar has no
            // second code path to drift down and an ambush is exactly as
            // debuggable as the trigger an author would otherwise hand-write.
            campaign.quests.content.expand_ambushes();
            Ok(campaign)
        }
        _ => Err(diags),
    }
}

/// Parse then validate, **through the obligation fence**.
///
/// Convenience over [`parse_campaign`], [`crate::validate::validate_campaign`]
/// and [`crate::fence::Fenced`], and the fence is not optional here: a caller with
/// a raw document in hand has no campaign to fence against afterwards, so an
/// unfenced list handed out from this function is one nothing downstream could
/// correct. Every diagnostic returned is one the campaign's own declared
/// `dsl_version` makes it answerable for; a [`crate::Binds::Since`] rule raised
/// against a stage below its version is grandfathered and never appears.
///
/// A document that does not parse cannot be fenced — there is no declared
/// version to read — so that path takes [`crate::fence::Fenced::structural`],
/// which refuses to carry anything version-scoped.
pub fn check_campaign(raw: &RawCampaign) -> Vec<Diagnostic> {
    match parse_campaign(raw) {
        Ok(campaign) => {
            let diags = crate::validate::validate_campaign(&campaign);
            crate::fence::Fenced::apply(&campaign, diags)
                .reported()
                .to_vec()
        }
        Err(diags) => crate::fence::Fenced::structural(diags).reported().to_vec(),
    }
}

#[cfg(test)]
mod version_ledger_tests {
    use super::*;

    /// A reservation is **in the ledger and not accepted**, and it sits above
    /// everything implemented. Below the latest it would shadow a landed
    /// surface; outside the ledger it would not be held at all, because a
    /// number outside the ledger is a number the next author finds free.
    #[test]
    fn a_reservation_is_in_the_ledger_held_and_refused() {
        assert!(is_supported_version(SUPPORTED_DSL_VERSION));
        assert_eq!(accepted_versions().last(), Some(SUPPORTED_DSL_VERSION));
        assert!(reserved_for(SUPPORTED_DSL_VERSION).is_none());

        // Binding count: this is the property's whole population. An empty
        // reservation list is the ordinary state of a ledger whose every number
        // has landed — and it is exactly the state in which the loops below
        // examine nothing, so it is asserted rather than skipped. What holds
        // instead, over the WHOLE ledger, is the complement of what a
        // reservation asserts: every number in the list is one this crate
        // builds, and the newest of them is the latest implemented version.
        // There is no number in flight, which is a claim with a population of
        // `SUPPORTED_DSL_VERSIONS.len()` rather than of zero.
        if RESERVED_DSL_VERSIONS.is_empty() {
            for v in SUPPORTED_DSL_VERSIONS {
                assert!(
                    is_supported_version(v),
                    "{v} is in the ledger, nothing reserves it, and this crate refuses it — a \
                     number that names no surface is a number a second change can take"
                );
                assert!(
                    minor_ordinal(v) <= minor_ordinal(SUPPORTED_DSL_VERSION),
                    "{v} is newer than the latest implemented version {SUPPORTED_DSL_VERSION} \
                     and is not reserved"
                );
            }
            assert!(
                !SUPPORTED_DSL_VERSIONS.is_empty(),
                "binding count 0: the ledger itself is empty"
            );
        }
        for (version, anchor) in RESERVED_DSL_VERSIONS {
            assert!(
                SUPPORTED_DSL_VERSIONS.contains(version),
                "{version} is reserved for {anchor} but is not in the ledger — a \
                 number outside the ledger is a free number"
            );
            assert!(
                !is_supported_version(version),
                "{version} is reserved for {anchor} yet this crate accepts it; a \
                 campaign declaring it would be built with that surface dropped"
            );
            assert!(
                minor_ordinal(version) > minor_ordinal(SUPPORTED_DSL_VERSION),
                "reserved {version} ({anchor}) is not newer than {SUPPORTED_DSL_VERSION}"
            );
        }

        // Every ledger entry above the latest implemented one is a reservation
        // and nothing else — a version in the list that neither this crate
        // builds nor holds for a sibling names nothing.
        for v in SUPPORTED_DSL_VERSIONS {
            if minor_ordinal(v) > minor_ordinal(SUPPORTED_DSL_VERSION) {
                assert!(
                    reserved_for(v).is_some(),
                    "{v} is newer than the latest implemented version and not reserved"
                );
            }
        }
    }

    #[test]
    fn accepted_versions_is_the_ledger_minus_its_reservations() {
        let accepted: Vec<&str> = accepted_versions().collect();
        assert!(accepted.iter().all(|v| is_supported_version(v)));
        assert!(
            accepted.iter().all(|v| reserved_for(v).is_none()),
            "a reserved version reached the accepted list"
        );
        assert_eq!(
            accepted.len() + RESERVED_DSL_VERSIONS.len(),
            SUPPORTED_DSL_VERSIONS.len()
        );
    }

    /// A version added to the list but not to `ordinal` would silently share
    /// `0.2.0`'s fence and open every surface below it.
    #[test]
    fn every_ledger_version_has_its_own_ascending_ordinal() {
        let mut seen: Vec<u32> = Vec::new();
        for v in SUPPORTED_DSL_VERSIONS {
            let n = minor_ordinal(v);
            assert!(n > 0, "{v} is in the ledger with no `ordinal` arm");
            assert!(
                !seen.contains(&n),
                "{v} shares an ordinal with an earlier version"
            );
            seen.push(n);
        }
        assert!(
            seen.windows(2).all(|w| w[0] < w[1]),
            "the ledger is read in order, oldest first"
        );
    }
}
