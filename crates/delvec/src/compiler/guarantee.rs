//! **Which anchors does this area guarantee** — asked and answered before
//! anything is placed.
//!
//! # The defect this exists to close
//!
//! An area that binds a `prefab_pool` seats a **subset** of the pool's members.
//! [`crate::compiler::solver::solve_area`] places exactly three things: the
//! `entry`-role member, at the area origin, unconditionally; one carrier for
//! each anchor the campaign makes the solver guarantee
//! ([`crate::compiler::solver::required_carriers`]); and `filler_count`
//! `connector`-role pieces drawn **with replacement** by weight from the pool's
//! own PRNG stream. Membership in the pool buys a piece nothing. A `room` or
//! `terminal` member that nothing requires is never seated, and an anchor it
//! declares does not exist in the built world.
//!
//! Nothing said so before the build. The shipped library's cave and keep pools
//! guarantee **two** anchor names per area — `spawn` and `anchor/exit`, both on
//! the entry piece — out of the ten their members declare between them, and the
//! only diagnostics that spoke were the build's: `DW0302` when the pool cannot
//! supply a required anchor at all, `DW0360` when an effect's anchor resolves
//! nowhere in the assembled world, `DW0498` when the settled draw made an anchor
//! ambiguous. Every one of them needs a build. The anchors are chosen at the
//! third authoring step and bound at the fifth, so the information arrived
//! several steps after the decision it was for, and a creator who designed
//! against the anchor set the library appears to offer paid a build to find out.
//!
//! # What is answerable here, and what is not
//!
//! Everything this module computes is a fact about **declarations**: the pool's
//! members and their roles, each member's `anchors` map, and the anchors this
//! campaign's own documents name. No `.nbt` is opened, no piece is placed, no
//! PRNG is drawn. That is why the whole verdict is available at validation, on
//! [`crate::compiler::seating`]'s precedent — a creator should not spend a build
//! to learn something the documents already decided.
//!
//! What is **not** knowable here is the filler draw. Which connectors the seed
//! seats is settled by [`crate::compiler::solver`] and by nothing else, so an
//! anchor carried only by connectors is reported as *outside the guarantee*,
//! never as absent. That asymmetry is the honest one and it decides the
//! severity: see [`DW_UNGUARANTEED_ANCHOR`].
//!
//! # Two readers, one implementation
//!
//! - `delvec prefab anchors` reads a **library** and answers per pool, with no
//!   campaign at all — which is what the third authoring step can ask, because
//!   at that step `quests.json` and `dialogue.json` do not exist yet and every
//!   campaign verb refuses a directory that is not all six documents (`DW0874`).
//! - `validate` (and so `analyze` and `build`) reads a **campaign** and answers
//!   per area, because the campaign's own required references promote members
//!   into the guaranteed set.
//!
//! Both go through [`pool_guarantee`], so the tool and the compiler cannot
//! disagree about what a pool guarantees.

use std::collections::BTreeSet;

use delvewright_dsl::{Campaign, Diagnostic, DwCode, ExitTier};

use crate::compiler::registry::PrefabRegistry;

/// `DW0889` — **an anchor this campaign names is outside what its area's layout
/// guarantees.**
///
/// **Advisory (warning, exit 0), and the tier is the finding rather than a
/// concession.** The guaranteed set is a *sufficient* condition, not a necessary
/// one: an anchor carried only by `connector` members is seated whenever the
/// filler draw happens to pick that connector, and a campaign that builds green
/// today may be resting on exactly that. Refusing here would refuse campaigns a
/// creator legitimately wants and whose builds succeed — the over-refusal the
/// brief this round answers names — and the engine already owns the sharp end:
/// `DW0302`, `DW0142`, `DW0360`, `DW0431` and `DW0447` all still refuse a
/// reference the assembled world does not answer. What was missing was never a
/// refusal. It was the answer arriving before the build.
///
/// **Its quantifier**: every anchor name this campaign's documents carry that
/// some member of a bound pool declares and no member the layout is required to
/// seat declares. It says nothing about a name no member declares — that is
/// `DW0302`'s, at the use site, and restating it here would be two codes for one
/// fact — and nothing about a name two placed pieces answer to, which is
/// `DW0498`/`DW0305`'s and cannot be known without the settled draw.
///
/// `ExitTier::Build` states what happens if this rule ever refuses with a build
/// under way; as a warning it never does.
pub const DW_UNGUARANTEED_ANCHOR: DwCode = DwCode::new("DW0889", ExitTier::Build);

// ---------------------------------------------------------------------------
// What a POOL guarantees, from the library alone
// ---------------------------------------------------------------------------

/// One member of a pool, as this answer needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    /// The member prefab id (`prefab/<name>`).
    pub prefab: String,
    /// The layout role the pool declares: `entry` | `connector` | anything else.
    pub role: String,
    /// The anchor names this member's metadata declares, or an empty set when
    /// the library holds no document for it (already `DW0300`/`DW0346`).
    pub anchors: BTreeSet<String>,
}

/// **What one pool guarantees**, read from the library and nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolGuarantee {
    /// The pool id.
    pub pool: String,
    /// Every member, in declaration order — the denominator every count below
    /// is drawn from.
    pub members: Vec<Member>,
    /// The member the solver seats at the area origin on every draw, or `None`
    /// when the pool declares no `entry`-role member (`DW0301` at the build).
    pub entry: Option<String>,
    /// The anchor names the entry member declares: what an area guarantees
    /// before its campaign requires anything.
    pub unconditional: BTreeSet<String>,
    /// Every anchor name any member declares — the pool's whole vocabulary.
    pub vocabulary: BTreeSet<String>,
}

impl PoolGuarantee {
    /// Every member that declares `anchor`, entry included.
    #[must_use]
    pub fn carriers(&self, anchor: &str) -> Vec<&Member> {
        self.members
            .iter()
            .filter(|m| m.anchors.contains(anchor))
            .collect()
    }

    /// How an anchor of this pool's vocabulary comes to exist in a built world,
    /// as a phrase a diagnostic can say.
    #[must_use]
    fn how(&self, anchor: &str) -> String {
        let carriers = self.carriers(anchor);
        let names: Vec<String> = carriers
            .iter()
            .map(|m| format!("`{}` (role `{}`)", m.prefab, m.role))
            .collect();
        let all_filler = !carriers.is_empty() && carriers.iter().all(|m| m.role == "connector");
        // Said without naming "this campaign", because the same sentence is
        // printed by `delvec prefab anchors`, where there is no campaign at all.
        let tail = if all_filler {
            "which the layout seats only as a filler draw, so whether it is in the built world is \
             settled by the area's seed"
        } else {
            "which the layout seats only when a campaign requires an anchor that piece carries"
        };
        format!("declared by {}, {tail}", names.join(", "))
    }
}

/// **Read one pool's guarantee.** `None` when the library declares no such pool
/// (already `DW0161` at validation).
#[must_use]
pub fn pool_guarantee(prefabs: &PrefabRegistry, pool: &str) -> Option<PoolGuarantee> {
    let raw = prefabs.pool(pool)?;
    let members: Vec<Member> = raw
        .iter()
        .map(|m| Member {
            prefab: m.prefab.clone(),
            role: m.role.clone(),
            anchors: prefabs
                .get(&m.prefab)
                .map(|meta| meta.anchors.keys().cloned().collect())
                .unwrap_or_default(),
        })
        .collect();
    let entry = crate::compiler::solver::entry_member(raw).map(|m| m.prefab.clone());
    let unconditional = entry
        .as_ref()
        .and_then(|e| members.iter().find(|m| &m.prefab == e))
        .map(|m| m.anchors.clone())
        .unwrap_or_default();
    let vocabulary: BTreeSet<String> = members
        .iter()
        .flat_map(|m| m.anchors.iter().cloned())
        .collect();
    Some(PoolGuarantee {
        pool: pool.to_string(),
        members,
        entry,
        unconditional,
        vocabulary,
    })
}

// ---------------------------------------------------------------------------
// What an AREA guarantees, given the campaign that binds it
// ---------------------------------------------------------------------------

/// **What one area of a campaign guarantees**, and which of the names the
/// campaign uses fall outside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaGuarantee {
    /// The area id.
    pub area: String,
    /// The pool it binds.
    pub pool: PoolGuarantee,
    /// The area's `pieces` budget: how many members a draw may seat.
    pub pieces: Option<(u32, u32)>,
    /// The pieces this campaign's required references force the solver to seat,
    /// beside the entry — [`crate::compiler::solver::required_carriers`]'s own
    /// answer, not a second one.
    pub forced: Vec<String>,
    /// Every anchor an area with this campaign is guaranteed to resolve: the
    /// entry member's, plus everything the forced carriers declare.
    pub guaranteed: BTreeSet<String>,
    /// The names this campaign carries that this pool declares somewhere and
    /// this area does not guarantee.
    pub outside: BTreeSet<String>,
}

/// What this check examined, stated whether it found anything or not.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GuaranteeBinding {
    /// Areas the campaign states.
    pub areas: usize,
    /// Of those, the ones that bind a pool — the only ones with a subset to
    /// report. A single-prefab area guarantees everything its piece declares.
    pub pool_areas: usize,
    /// Pool members examined across those areas.
    pub members: usize,
    /// Anchor names those pools declare between them.
    pub vocabulary: usize,
    /// Of those, the ones the areas guarantee.
    pub guaranteed: usize,
    /// Anchor names this campaign's documents carry, of any area.
    pub named: usize,
    /// Of those, the ones reported as outside a guarantee (`DW0889`).
    pub outside: usize,
}

impl GuaranteeBinding {
    /// **The one line this check owes its reader**, printed whether it found
    /// anything or not.
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "anchor-guarantee binding: {pool} of {areas} area(s) bind a pool, examined over \
             {members} member(s) declaring {vocab} anchor name(s), of which {guar} are \
             guaranteed; {named} name(s) carried by this campaign's documents, {outside} \
             reported outside a guarantee.",
            pool = self.pool_areas,
            areas = self.areas,
            members = self.members,
            vocab = self.vocabulary,
            guar = self.guaranteed,
            named = self.named,
            outside = self.outside,
        )
    }
}

/// **Every anchor name this campaign's documents carry**, walked as documents.
///
/// A typed walk over the verbs that may name an anchor is what this deliberately
/// is not. Eleven of those already exist and the twelfth is the defect
/// `CLAUDE.md` names by shape — *a hand-rolled walk enumerating three of five
/// effect roots* — because it is right on the day it is written and silently
/// narrower than the DSL on every day after. Serialising the stage documents
/// cannot forget a field: a verb added tomorrow is walked without this function
/// being touched.
///
/// It over-reports rather than under-reports, and that direction is chosen: a
/// name is collected wherever it appears, so on a multi-area campaign a name one
/// area answers to is offered to every pool that declares it. The consumer
/// intersects with the pool's own vocabulary, so the surplus is bounded by the
/// library, and the report says *this campaign names it*, never *this area
/// resolves it*.
#[must_use]
pub fn names_used(c: &Campaign) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let docs = [
        serde_json::to_value(&c.world).ok(),
        serde_json::to_value(&c.npcs).ok(),
        serde_json::to_value(&c.classes).ok(),
        serde_json::to_value(&c.quest_plan).ok(),
        serde_json::to_value(&c.quests).ok(),
        serde_json::to_value(&c.dialogue).ok(),
        c.world_edits
            .as_ref()
            .and_then(|e| serde_json::to_value(e).ok()),
        c.geometry_brief
            .as_ref()
            .and_then(|e| serde_json::to_value(e).ok()),
        c.layout_graph
            .as_ref()
            .and_then(|e| serde_json::to_value(e).ok()),
        c.site_plan
            .as_ref()
            .and_then(|e| serde_json::to_value(e).ok()),
        c.detail_plan
            .as_ref()
            .and_then(|e| serde_json::to_value(e).ok()),
    ];
    for doc in docs.into_iter().flatten() {
        collect_strings(&doc, &mut out);
    }
    out
}

/// Every string value in a JSON document, into `out`.
fn collect_strings(v: &serde_json::Value, out: &mut BTreeSet<String>) {
    match v {
        serde_json::Value::String(s) => {
            out.insert(s.clone());
        }
        serde_json::Value::Array(a) => {
            for x in a {
                collect_strings(x, out);
            }
        }
        serde_json::Value::Object(o) => {
            for (k, x) in o {
                out.insert(k.clone());
                collect_strings(x, out);
            }
        }
        _ => {}
    }
}

/// **The campaign-tier answer**: what every pool area guarantees, and the
/// `DW0889` advisories for the names that fall outside.
///
/// Returns the binding beside the diagnostics — a check that reported nothing
/// still owes the numbers it reported nothing over.
#[must_use]
pub fn check(c: &Campaign, prefabs: &PrefabRegistry) -> (GuaranteeBinding, Vec<Diagnostic>) {
    let mut binding = GuaranteeBinding {
        areas: c.world.content.areas.len(),
        ..GuaranteeBinding::default()
    };
    let mut diags = Vec::new();
    let used = names_used(c);
    binding.named = used.len();

    for area in areas(c, prefabs, &used) {
        binding.pool_areas += 1;
        binding.members += area.pool.members.len();
        binding.vocabulary += area.pool.vocabulary.len();
        binding.guaranteed += area.guaranteed.len();
        binding.outside += area.outside.len();
        for anchor in &area.outside {
            diags.push(Diagnostic::warning(
                DW_UNGUARANTEED_ANCHOR,
                "world",
                format!("/content/areas/{}", area.area),
                area.message(anchor),
            ));
        }
    }
    (binding, diags)
}

/// Every pool area of a campaign, with its guarantee computed. `used` is
/// [`names_used`]'s answer, passed in so one walk serves every area.
#[must_use]
pub fn areas(
    c: &Campaign,
    prefabs: &PrefabRegistry,
    used: &BTreeSet<String>,
) -> Vec<AreaGuarantee> {
    let mut out = Vec::new();
    for area in &c.world.content.areas {
        let Some(pool_id) = &area.prefab_pool else {
            continue;
        };
        let Some(pool) = pool_guarantee(prefabs, pool_id.as_str()) else {
            continue;
        };
        let area_id = area.id.as_str().to_string();
        // The solver's own answer to *which pieces must this layout seat*,
        // asked of the solver rather than re-derived. An unsatisfiable required
        // anchor is `DW0302`'s finding at the build, and reporting nothing here
        // rather than a second version of it is deliberate.
        let required = crate::compiler::plan::required_anchors_for_area(c, &area_id);
        let forced = match (&pool.entry, prefabs.pool(pool_id.as_str())) {
            (Some(entry), Some(members)) => crate::compiler::solver::required_carriers(
                prefabs,
                pool_id.as_str(),
                members,
                entry,
                &required,
            )
            .unwrap_or_default(),
            _ => Vec::new(),
        };
        let mut guaranteed = pool.unconditional.clone();
        for p in &forced {
            if let Some(m) = pool.members.iter().find(|m| &m.prefab == p) {
                guaranteed.extend(m.anchors.iter().cloned());
            }
        }
        let outside: BTreeSet<String> = pool
            .vocabulary
            .iter()
            .filter(|a| used.contains(*a) && !guaranteed.contains(*a))
            .cloned()
            .collect();
        out.push(AreaGuarantee {
            area: area_id,
            pool,
            pieces: area.pieces.map(|p| (p.min, p.max)),
            forced,
            guaranteed,
            outside,
        });
    }
    out
}

impl AreaGuarantee {
    /// The `DW0889` sentence for one name outside the guarantee — the answer,
    /// its denominator, and what the creator does about it.
    #[must_use]
    pub fn message(&self, anchor: &str) -> String {
        let (min, max) = self.pieces.unwrap_or((0, 0));
        format!(
            "this campaign names anchor `{anchor}`, and area `{area}` does not guarantee it. \
             `{area}` binds `{pool}` ({members} member(s), `pieces` {min}..{max}); a draw seats \
             the `entry` member{entry} plus {forced} piece(s) this campaign's own required \
             anchors force, plus `connector` fillers drawn from the seed. That is {guar} of the \
             {vocab} anchor name(s) the pool declares. `{anchor}` is {how}. Either bind a name the \
             area guarantees, require this one where the solver must seat its carrier (an \
             objective, an NPC stand, a wave, a lane waypoint or an anchor-bearing effect in this \
             area), or bind the area to that piece with `prefab` instead of `prefab_pool` — and \
             then read this line again, because it is the answer, not a guess: \
             `delvec prefab anchors --pool {pool}` states the same set with no campaign at all.",
            area = self.area,
            pool = self.pool.pool,
            members = self.pool.members.len(),
            entry = self
                .pool
                .entry
                .as_ref()
                .map(|e| format!(" `{e}`"))
                .unwrap_or_default(),
            forced = self.forced.len(),
            guar = self.guaranteed.len(),
            vocab = self.pool.vocabulary.len(),
            how = self.pool.how(anchor),
        )
    }
}

// ---------------------------------------------------------------------------
// The library-only report (`delvec prefab anchors`)
// ---------------------------------------------------------------------------

/// One pool's answer as a report line set, for `delvec prefab anchors`.
///
/// Human lines and the JSON object are built from the same [`PoolGuarantee`],
/// so the two renderings cannot describe different libraries.
#[must_use]
pub fn report_lines(g: &PoolGuarantee) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!(
        "{pool}: {n} member(s); entry {entry}; guarantees {guar} of {vocab} anchor name(s)",
        pool = g.pool,
        n = g.members.len(),
        entry = g
            .entry
            .as_ref()
            .map(|e| format!("`{e}`"))
            .unwrap_or_else(|| "NONE — a draw of this pool is DW0301 at the build".to_string()),
        guar = g.unconditional.len(),
        vocab = g.vocabulary.len(),
    ));
    if g.unconditional.is_empty() {
        lines.push("  guaranteed: (none)".to_string());
    } else {
        for a in &g.unconditional {
            lines.push(format!("  guaranteed  {a}"));
        }
    }
    let mut rest: Vec<&String> = g
        .vocabulary
        .iter()
        .filter(|a| !g.unconditional.contains(*a))
        .collect();
    rest.sort();
    for a in rest {
        lines.push(format!("  conditional {a:<26} {}", g.how(a)));
    }
    lines
}

/// One pool's answer as JSON, for `delvec prefab anchors --json`.
#[must_use]
pub fn report_json(g: &PoolGuarantee) -> serde_json::Value {
    serde_json::json!({
        "pool": g.pool,
        "members": g.members.iter().map(|m| serde_json::json!({
            "prefab": m.prefab,
            "role": m.role,
            "anchors": m.anchors.iter().cloned().collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        // Named `entry_member`, not `entry`: it holds the POOL MEMBER the solver
        // seats first, which is a prefab id and a `PoolMember` role. The anchor
        // role of the same word is a different vocabulary on a different object
        // (`dsl::prefab::AnchorRole`), and one key spelling both is how the two
        // get confused.
        "entry_member": g.entry,
        "guaranteed": g.unconditional.iter().cloned().collect::<Vec<_>>(),
        "vocabulary": g.vocabulary.iter().cloned().collect::<Vec<_>>(),
        "conditional": g.vocabulary.iter().filter(|a| !g.unconditional.contains(*a))
            .map(|a| serde_json::json!({
                "anchor": a,
                "carriers": g.carriers(a).iter().map(|m| serde_json::json!({
                    "prefab": m.prefab, "role": m.role,
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
    })
}
