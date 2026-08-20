//! Runtime-watch coverage of per-object bodies: when the generated PackTest
//! suite drives ONE declared object's own emitted body, it must drive **every**
//! sibling in that family (`DW0810`).
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

/// Judge the finished tree. Returns what was examined and every unwatched
/// sibling, both in deterministic order.
pub fn check_tree(
    ns: &str,
    out: &BTreeMap<String, Vec<u8>>,
    ids: &BTreeSet<String>,
) -> (WatchBinding, Vec<Unwatched>) {
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
