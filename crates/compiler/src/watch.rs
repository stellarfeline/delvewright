//! Runtime-watch coverage of per-object bodies, in two tiers.
//!
//! * `DW0810`, a **warning**, read off the shipped bytes with no mechanic named
//!   anywhere: when the generated PackTest suite drives ONE declared object's
//!   own emitted body, it must drive **every** sibling in that family.
//! * `DW0811`, a **refusal**, judged against a claim the emitter registers over
//!   the plan's own authored list. See the `claims` section at the foot of this
//!   file for why the refusal cannot be drawn on the byte read alone.
//!
//! ## The defect class this closes
//!
//! A mechanic whose runtime body is emitted *per object* — a timed gate's
//! `tgate_open_<id>`/`tgate_close_<id>`, an actor's `unleash_<id>`, a wave's
//! `wave_census_<id>` — has one body per declared object, over its own region,
//! with its own blocks and its own judgement. A template that drives one of
//! them proves nothing whatever about the next.
//!
//! The worked instance. The timed-gate emitter bound `plan.timed_gates.first()`
//! and wrote one template per campaign. A level declaring three gates, with the
//! lethal `crush` flag on the **third**, therefore shipped its only
//! player-killing mechanic with a compile-time proof and no runtime proof at
//! all — and the suite was green for as long as the level existed. Its own
//! generation record showed six critical-path bot attempts, every one exit 1,
//! the lethal gate never reached in any run; nobody had connected the two facts.
//!
//! This is the unbound-gate vacuity mode wearing a subtler surface. The gate was
//! not unbound: it bound one object and reported honestly about that one, while
//! the set it was supposed to cover had N members. `CLAUDE.md` names the same
//! shape one layer down as *a hand-rolled walk enumerating 3 of 5 effect roots —
//! a defect of expressibility, not of care.*
//!
//! ## Why it is decided here, over the bytes
//!
//! The operator running `delvec` does not run `cargo test`, so a unit test
//! asserting that one emitter loops would not have caught the next emitter to
//! make the same choice — and the enumeration found the same shape at eight
//! other families in the engine's own gallery. Judged from the finished tree,
//! this guards emitters that do not exist yet, in the same family as
//! [`crate::integrity`], [`crate::seeding`] and [`crate::batchstate`].
//!
//! ## How a family is discovered, without a table of mechanics
//!
//! A table of "watchable classes" would be the very defect it is checking — a
//! hand-rolled walk over the mechanics someone remembered. So nothing here names
//! a mechanic:
//!
//! 1. The **declared ids** come from a generic walk over the authored stage
//!    documents, collecting every `id` string at any depth. That is literally
//!    what the author wrote, so a stage or mechanic added later is covered with
//!    no change to this file.
//! 2. An emitted campaign function `f` is **object `i`'s body in family
//!    `prefix`** when `f == prefix + i` and `prefix` ends in `_`. Where several
//!    ids match, the longest wins, so `tgate_open_side_door` belongs to
//!    `side_door` and not to `door`.
//! 3. A body is **watched** when a generated template invokes it directly —
//!    `function <ns>:<body>`. Nothing weaker counts: a template that never calls
//!    the body has not proven the body, and merely *mentioning* it would let a
//!    comment stand in for a proof.
//!
//! A family with two or more members, at least one watched and at least one not,
//! is the finding.
//!
//! ## Scope, and why it is drawn there
//!
//! * **At least one sibling must already be watched.** The rule this enforces is
//!   *the suite claims to watch this mechanic, so it must watch all of it*. A
//!   family where nothing is watched is a different and much broader question —
//!   most emitted functions have no template and never will — and folding it in
//!   here would bury the finding this exists to surface. The limit is stated
//!   rather than silent: [`WatchBinding`] reports unwatched families too, so a
//!   mechanic with no runtime proof at all is visible in the ledger instead of
//!   being passed over.
//! * **Sub-bodies are not siblings.** `lethal_east_pit` and
//!   `lethal_east_pit_kill` share a prefix but only the first ends in a declared
//!   id, so the second is never mistaken for an object. This is what keeps the
//!   rule quiet enough to read: over the gallery it drops every one of the
//!   sixteen families a naive prefix match reports, and keeps the eight real ones.
//!
//! Determinism (ADR-0006): every set is a `BTreeSet` and every map a `BTreeMap`,
//! so the message text is a function of the tree alone.

use delvewright_dsl::DwCode;
use std::collections::{BTreeMap, BTreeSet};

/// `DW0810`: the generated PackTest suite drives one declared object's own
/// emitted body but not a sibling's, so the sibling ships with no runtime proof.
///
/// Warning tier. The suite still loads and every template in it still passes —
/// that is the failure mode, not a mitigation.
pub const DW_UNWATCHED_SIBLING: DwCode = DwCode::every_version("DW0810");

/// One declared object whose own body no template drives.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Unwatched {
    /// The emitted-function family, e.g. `tgate_open_`.
    pub family: String,
    /// The object's function/tag-safe id, e.g. `crush_door`.
    pub id: String,
    /// The emitted body nothing drives, e.g. `tgate_open_crush_door`.
    pub function: String,
    /// The siblings in the same family that ARE driven — the proof that this
    /// family is one the suite claims to watch.
    pub watched_siblings: Vec<String>,
}

/// What the check actually examined. A proof over "every object" that bound to
/// nothing is vacuous, not a pass (CLAUDE.md), so the numbers are reported and
/// travel with the build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchBinding {
    /// Distinct `id` values found in the authored stage documents.
    pub declared_ids: usize,
    /// Emitted campaign functions in the datapack.
    pub campaign_functions: usize,
    /// Campaign functions the suite invokes directly.
    pub invoked: usize,
    /// Per-object families discovered (any size).
    pub families: usize,
    /// Of those, families with two or more declared objects — the ones this
    /// check can judge at all.
    pub multi_object_families: usize,
    /// Of those, families where NO member is watched. Not a `DW0810` finding
    /// (see the module doc), but reported so the limit is never silent.
    pub unwatched_families: usize,
    /// Objects in multi-object families whose body the suite drives.
    pub watched_objects: usize,
    /// Objects in multi-object families whose body nothing drives, in a family
    /// where a sibling IS driven. The `DW0810` count.
    pub unwatched_objects: usize,
}

impl WatchBinding {
    /// Objects this check actually judged: the members of every multi-object
    /// family, watched and unwatched alike. Zero means the check bound nothing,
    /// which on a campaign that declares several of anything is a finding rather
    /// than a pass.
    pub fn examined(&self) -> usize {
        self.watched_objects + self.unwatched_objects
    }

    /// Render as JSON for `validation/watch-ledger.json`, so the binding count
    /// travels with the build instead of living only in a stderr string.
    ///
    /// The `examined` key is deliberately spelled the way the gallery coverage
    /// gate already looks for (`tools/check-gallery-coverage.py` reds on a zero
    /// `examined` in any `validation/*.json`). That binds this ledger to an
    /// invocation that already exists rather than adding a gate that would have
    /// to be remembered — a doc line is not an invocation.
    pub fn to_json(&self, findings: &[Unwatched]) -> serde_json::Value {
        serde_json::json!({
            "examined": self.examined(),
            "declared_ids": self.declared_ids,
            "campaign_functions": self.campaign_functions,
            "invoked": self.invoked,
            "families": self.families,
            "multi_object_families": self.multi_object_families,
            "unwatched_families": self.unwatched_families,
            "watched_objects": self.watched_objects,
            "unwatched_objects": self.unwatched_objects,
            "unwatched": findings
                .iter()
                .map(|u| {
                    serde_json::json!({
                        "family": u.family,
                        "id": u.id,
                        "function": u.function,
                        "watched_siblings": u.watched_siblings,
                    })
                })
                .collect::<Vec<_>>(),
        })
    }
}

/// Every `id` string, at any depth, in the authored stage documents — the
/// generic id authority (see the module doc). Values are reduced to the same
/// function/tag-safe form the emitters use, so they compare against emitted
/// function names directly.
pub fn declared_ids(input_bytes: &BTreeMap<String, Vec<u8>>) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for raw in input_bytes.values() {
        let Ok(v) = serde_json::from_slice::<serde_json::Value>(raw) else {
            continue;
        };
        collect_ids(&v, &mut ids);
    }
    ids
}

fn collect_ids(v: &serde_json::Value, ids: &mut BTreeSet<String>) {
    match v {
        serde_json::Value::Object(map) => {
            for (k, child) in map {
                if k == "id"
                    && let Some(s) = child.as_str()
                {
                    let safe = crate::plan::safe_local(s);
                    if !safe.is_empty() {
                        ids.insert(safe);
                    }
                }
                collect_ids(child, ids);
            }
        }
        serde_json::Value::Array(items) => {
            for child in items {
                collect_ids(child, ids);
            }
        }
        _ => {}
    }
}

/// The two facts every judgement here rests on, read once off the finished
/// tree: the campaign functions that exist, and the ones the generated suite
/// calls. Both readings are shared by [`check_tree`] and [`check_claims`] on
/// purpose — a second reading would be a second calibration, and two gates that
/// disagree about what the tree contains is worse than either.
fn emitted_and_invoked(
    ns: &str,
    out: &BTreeMap<String, Vec<u8>>,
) -> (BTreeSet<String>, BTreeSet<String>) {
    let fn_prefix = format!("datapack/data/{ns}/function/");
    let suite_prefix = format!("packtest-datapack/data/{ns}/");

    // Emitted campaign functions, by bare name.
    let functions: BTreeSet<String> = out
        .keys()
        .filter_map(|p| {
            p.strip_prefix(&fn_prefix)
                .and_then(|r| r.strip_suffix(".mcfunction"))
                .map(str::to_string)
        })
        .collect();

    // Every campaign function the generated suite invokes directly.
    let call = format!("function {ns}:");
    let mut invoked: BTreeSet<String> = BTreeSet::new();
    for (path, body) in out {
        if !path.starts_with(&suite_prefix) {
            continue;
        }
        let Ok(text) = std::str::from_utf8(body) else {
            continue;
        };
        for line in text.lines() {
            let mut rest = line;
            while let Some(at) = rest.find(&call) {
                rest = &rest[at + call.len()..];
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '/')
                    .collect();
                if functions.contains(&name) {
                    invoked.insert(name);
                }
            }
        }
    }
    (functions, invoked)
}

/// Judge the finished tree. Returns what was examined and every unwatched
/// sibling, both in deterministic order.
pub fn check_tree(
    ns: &str,
    out: &BTreeMap<String, Vec<u8>>,
    ids: &BTreeSet<String>,
) -> (WatchBinding, Vec<Unwatched>) {
    let (functions, invoked) = emitted_and_invoked(ns, out);

    // Decompose each function into (family, object id), longest id wins.
    let mut families: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for f in &functions {
        let mut best: Option<&String> = None;
        for i in ids {
            if f.len() > i.len() + 1
                && f.ends_with(i.as_str())
                && f.as_bytes()[f.len() - i.len() - 1] == b'_'
                && best.is_none_or(|b| i.len() > b.len())
            {
                best = Some(i);
            }
        }
        if let Some(i) = best {
            let family = f[..f.len() - i.len()].to_string();
            families
                .entry(family)
                .or_default()
                .insert(i.clone(), f.clone());
        }
    }

    let mut binding = WatchBinding {
        declared_ids: ids.len(),
        campaign_functions: functions.len(),
        invoked: invoked.len(),
        families: families.len(),
        multi_object_families: 0,
        unwatched_families: 0,
        watched_objects: 0,
        unwatched_objects: 0,
    };
    let mut findings = Vec::new();

    for (family, members) in &families {
        if members.len() < 2 {
            continue;
        }
        binding.multi_object_families += 1;
        let watched: Vec<&String> = members
            .iter()
            .filter(|(_, f)| invoked.contains(*f))
            .map(|(id, _)| id)
            .collect();
        if watched.is_empty() {
            binding.unwatched_families += 1;
            continue;
        }
        binding.watched_objects += watched.len();
        let watched_siblings: Vec<String> = watched.iter().map(|s| (*s).clone()).collect();
        for (id, f) in members {
            if invoked.contains(f) {
                continue;
            }
            binding.unwatched_objects += 1;
            findings.push(Unwatched {
                family: family.clone(),
                id: id.clone(),
                function: f.clone(),
                watched_siblings: watched_siblings.clone(),
            });
        }
    }

    (binding, findings)
}

/// The `DW0810` warning naming every unwatched sibling, or `None` when the suite
/// watches every family it touches.
pub fn finding(
    binding: &WatchBinding,
    findings: &[Unwatched],
) -> Option<delvewright_dsl::Diagnostic> {
    if findings.is_empty() {
        return None;
    }
    let mut rows: Vec<String> = findings
        .iter()
        .map(|u| {
            format!(
                "`{}` (declared `{}`; the suite drives its sibling(s) {})",
                u.function,
                u.id,
                u.watched_siblings
                    .iter()
                    .map(|s| format!("`{}{s}`", u.family))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
        .collect();
    rows.sort();
    Some(delvewright_dsl::Diagnostic::warning(
        DW_UNWATCHED_SIBLING,
        "build",
        "packtest watch coverage",
        format!(
            "the generated PackTest suite drives one declared object's own emitted body but not \
             {} sibling(s) of it, across {} of {} multi-object family/families: {}. A per-object \
             body is the object's own code — its own region, blocks and judgement — so watching \
             one sibling proves nothing about the next; the level that exposed this shipped a \
             LETHAL timed gate, third of three, with no runtime proof at all and a green suite. \
             Drive every member of the family, not the first. (Examined {} declared id(s) against \
             {} emitted campaign function(s), {} of which the suite invokes; {} multi-object \
             family/families carry no runtime proof at all and are reported in \
             `validation/watch-ledger.json` rather than here.)",
            binding.unwatched_objects,
            findings
                .iter()
                .map(|u| &u.family)
                .collect::<BTreeSet<_>>()
                .len(),
            binding.multi_object_families,
            rows.join("; "),
            binding.declared_ids,
            binding.campaign_functions,
            binding.invoked,
            binding.unwatched_families,
        ),
    ))
}

// ---------------------------------------------------------------- claims --
//
// `DW0810` above is read off the bytes and names no mechanic, which is what
// makes it general — and is also exactly why it can only be a warning. Nothing
// in the finished tree distinguishes *the emitter meant to prove every member
// and skipped some* from *the suite deliberately drives one exemplar*: both are
// a family with a watched member and an unwatched one, and eight families on
// the engine's own gallery are honestly the second. A refusal drawn on that
// reading would either red eight standing families or need a per-family
// allowlist, and an allowlist is an opt-out the defect itself can supply.
//
// So the refusal is drawn where the distinction actually lives: in the emitter.
// A suite emitter that walks a declared list to write per-object templates
// **registers a [`Claim`] over that list**, and the claim is then judged against
// the shipped bytes. The two halves cannot both be faked by the defect:
//
// * `declared` comes from the plan's own authored list, so skipping members
//   does not shrink it — `plan.timed_gates.first()` still registers three;
// * `invoked` is read off the emitted suite, never from the emitter's own
//   bookkeeping, so an emitter cannot report coverage it did not write.
//
// The stated limit, because a silent one is worse than a narrow one: an emitter
// that registers no claim at all is outside this refusal. That escape is not
// free — deleting a registration is a visible deletion, `DW0810` still names
// every sibling it leaves unwatched, and the gallery's warning ledger is a
// set-equality that reds on the new row. What the claim buys is that the
// mechanic which HAS been proven per object can never quietly stop being.

/// `DW0811`: a suite emitter claimed per-object runtime proof over a declared
/// list, and the shipped suite drives only some of the bodies it wrote for that
/// list. Refusal tier — the emitter's own claim is the proof obligation, and a
/// strict subset does not discharge it.
pub const DW_CLAIM_NOT_DISCHARGED: DwCode = DwCode::every_version("DW0811");

/// One suite emitter's claim that it proves a declared list **per object**.
///
/// Registered beside the loop that satisfies it, over the same authored list
/// the loop walks. Emitting for a subset of `declared` is the breach.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    /// The mechanic as an author names it, for the message only.
    pub mechanic: &'static str,
    /// The emitted body families this claim covers, e.g. `tgate_open_`.
    pub families: &'static [&'static str],
    /// Every declared object's function-safe id, from the plan's authored list.
    pub declared: BTreeSet<String>,
}

/// One declared object whose emitted body the claiming suite does not drive.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ClaimBreach {
    /// The claiming mechanic.
    pub mechanic: String,
    /// The body family, e.g. `tgate_open_`.
    pub family: String,
    /// The declared object's safe id.
    pub id: String,
    /// The emitted body nothing in the suite calls.
    pub function: String,
}

/// What the claim check examined. Stated for the same reason every other
/// binding here is: a refusal that judged nothing is vacuous, not a pass.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClaimBinding {
    /// Claims registered by the emitters.
    pub claims: usize,
    /// Declared objects across every claim.
    pub declared_objects: usize,
    /// Emitted bodies those objects own, across every claimed family — the
    /// number this refusal actually judged.
    pub bodies_judged: usize,
    /// Of those, the ones the suite drives.
    pub bodies_watched: usize,
}

impl ClaimBinding {
    /// Bodies this refusal actually judged. Zero on a campaign that declares
    /// none of the claimed mechanic is honest; zero on one that declares it
    /// means the refusal stopped reaching what the document plainly writes.
    pub fn examined(&self) -> usize {
        self.bodies_judged
    }

    /// Render for `validation/watch-claims.json`.
    ///
    /// Its own file rather than a section of the watch ledger, and the reason is
    /// the whole point: `tools/check-gallery-coverage.py` reds on a top-level
    /// `examined: 0` in any `validation/*.json`, and it reads TOP-LEVEL keys
    /// only. Nested beside the `DW0810` numbers this count would have been
    /// written, committed, diffed — and never once judged, which is the UNRUN
    /// shape wearing a ledger's clothes. Hung off an invocation that already
    /// exists, a claim that quietly stops binding on the gallery is a red rather
    /// than a number nobody reads.
    pub fn to_json(&self, breaches: &[ClaimBreach]) -> serde_json::Value {
        serde_json::json!({
            "examined": self.examined(),
            "claims": self.claims,
            "declared_objects": self.declared_objects,
            "bodies_judged": self.bodies_judged,
            "bodies_watched": self.bodies_watched,
            "breaches": breaches
                .iter()
                .map(|b| {
                    serde_json::json!({
                        "mechanic": b.mechanic,
                        "family": b.family,
                        "id": b.id,
                        "function": b.function,
                    })
                })
                .collect::<Vec<_>>(),
        })
    }
}

/// Judge every registered claim against the shipped bytes.
///
/// A body is judged only when it EXISTS: a family that is emitted for a subset
/// of the declared list by design — a gate's `tgate_disarm_<id>` exists only for
/// the gates that declare a disarm — must not be read as a breach for the ones
/// that were never meant to have one. What is judged is the body that was
/// written, and the rule over it is total: **written for a declared object,
/// therefore driven.**
pub fn check_claims(
    ns: &str,
    out: &BTreeMap<String, Vec<u8>>,
    claims: &[Claim],
) -> (ClaimBinding, Vec<ClaimBreach>) {
    let (functions, invoked) = emitted_and_invoked(ns, out);
    let mut binding = ClaimBinding {
        claims: claims.len(),
        ..ClaimBinding::default()
    };
    let mut breaches = Vec::new();
    for c in claims {
        binding.declared_objects += c.declared.len();
        for family in c.families {
            for id in &c.declared {
                let f = format!("{family}{id}");
                if !functions.contains(&f) {
                    continue;
                }
                binding.bodies_judged += 1;
                if invoked.contains(&f) {
                    binding.bodies_watched += 1;
                } else {
                    breaches.push(ClaimBreach {
                        mechanic: c.mechanic.to_string(),
                        family: (*family).to_string(),
                        id: id.clone(),
                        function: f,
                    });
                }
            }
        }
    }
    breaches.sort();
    (binding, breaches)
}

/// The `DW0811` refusal naming every undischarged claim, or `None`.
pub fn claim_finding(
    binding: &ClaimBinding,
    breaches: &[ClaimBreach],
) -> Option<delvewright_dsl::Diagnostic> {
    if breaches.is_empty() {
        return None;
    }
    let mechanics: BTreeSet<&str> = breaches.iter().map(|b| b.mechanic.as_str()).collect();
    let rows: Vec<String> = breaches
        .iter()
        .map(|b| format!("`{}` (declared `{}`, {})", b.function, b.id, b.mechanic))
        .collect();
    Some(delvewright_dsl::Diagnostic::error(
        DW_CLAIM_NOT_DISCHARGED,
        "build",
        "packtest watch claim",
        format!(
            "the suite emitter for {} claims per-object runtime proof over the campaign's \
             declared list, and the shipped suite drives only {} of the {} body/bodies it wrote \
             for that list. {} declared object(s) therefore ship with a compile-time proof and \
             no runtime proof at all: {}. A per-object body is the object's own code — its own \
             region, blocks and judgement — so driving one member proves nothing about the next; \
             the level that exposed this shipped a LETHAL timed gate, third of three, with a \
             green suite throughout. Drive every member of the declared list, not the first. \
             (Judged {} claim(s) over {} declared object(s).)",
            mechanics
                .iter()
                .map(|m| format!("`{m}`"))
                .collect::<Vec<_>>()
                .join(", "),
            binding.bodies_watched,
            binding.bodies_judged,
            breaches.len(),
            rows.join("; "),
            binding.claims,
            binding.declared_objects,
        ),
    ))
}
