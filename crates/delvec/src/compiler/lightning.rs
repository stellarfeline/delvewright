//! **A lightning bolt strikes where the campaign meant it to, hurts nobody the
//! campaign posted, and rewrites no block** (spec-0092 §5) — `DW0958`,
//! `DW0959`, and the binding ledger that says what they looked at.
//!
//! # Why a strike owes a proof at all
//!
//! A bolt is not decoration. While it lives it calls `thunderHit` on every
//! living body in a box three blocks out on each side, three below and nine
//! above it ([`delvewright_dsl::lightning::REACH_HORIZONTAL`],
//! [`delvewright_dsl::lightning::REACH_BELOW`],
//! [`delvewright_dsl::lightning::REACH_ABOVE`]): five HP and eight seconds
//! alight, a villager turned into a witch, a pig into a zombified piglin, a
//! creeper charged. And it rewrites the block it strikes when that block is a
//! lightning rod or weathering copper, along a random walk the game rolls. A
//! verb that can do either beside what the campaign placed is a verb that owes
//! an answer to *where does it land*.
//!
//! # The two rules
//!
//! * **A posted body in reach** (`DW0958`). Any place the campaign requires a
//!   body to be — the enumeration is **`DW0511`'s own**
//!   ([`crate::compiler::lethal::posted_places`]), handed over rather than
//!   re-derived, so a post class added for one rule is seen by the other —
//!   whose body — the hitbox that enumeration states, standing on its cell —
//!   meets the bolt's box, by the strict overlap vanilla's own box test makes.
//! * **A struck block the game rewrites** (`DW0959`): the block under the mark,
//!   read from the assembled world, is one
//!   [`delvewright_dsl::lightning::rewrites`] names.
//!
//! # What it deliberately does not catch
//!
//! **Players are not posted**, as for a firework: a player standing within
//! three blocks of a strike takes at most
//! [`delvewright_dsl::lightning::worst_damage_hp`] HP, and the strike is a
//! hazard the creator places in plain sight of the beat. A cutscene puts every
//! player in spectator, where the bolt cannot hurt them. **Fire is not judged**
//! because a delve has none to judge: the bolt lights fire only where a player
//! stands strictly closer than `fire_spread_radius_around_player`, which every
//! delve seals at `0` (spec-0092 §2.3).

use std::collections::BTreeMap;

use delvewright_dsl::{DwCode, ExitTier, Verb};

use crate::compiler::failure::Failure;
use crate::compiler::plan::Plan;

delvewright_dsl::dw_code! {
    /// `DW0958`: a lightning strike's reach holds a place the campaign posts a
    /// body (spec-0092 §5).
    pub const DW_LIGHTNING_POST_IN_REACH: DwCode = DwCode::new("DW0958", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0959`: a lightning strike's struck block is one the game rewrites — a
    /// lightning rod, or weathering copper (spec-0092 §5).
    pub const DW_LIGHTNING_REWRITES_BLOCK: DwCode = DwCode::new("DW0959", ExitTier::Build);
}

/// What one strike's proof examined.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StrikeRow {
    /// The JSON pointer the effect was found at.
    pub path: String,
    /// The mark, as a diagnostic spells it.
    pub mark: String,
    /// The cell the bolt stands in.
    pub cell: [i32; 3],
    /// The block it strikes, as the assembled world holds it (`None` = air or
    /// the horizon's own water, which the world model does not store).
    pub struck: Option<String>,
}

/// **The binding ledger for the strike proofs** (spec-0092 §5). A campaign
/// that declares no strike reports zeroes and says so.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LightningGate {
    /// Strikes declared, at every effect root and any nesting depth.
    pub declared: usize,
    /// One row per strike whose mark resolved to a cell.
    pub rows: Vec<StrikeRow>,
    /// `(post, strike)` pairs examined for reach.
    pub posts: usize,
    /// Findings raised. The build carries the first; the rest print beside it.
    pub refused: usize,
}

impl LightningGate {
    /// The ledger as the `validation/lightning-gate.json` artifact.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "declared": self.declared,
            "struck_blocks_read": self.rows.len(),
            "posts_examined": self.posts,
            "refused": self.refused,
            "reach": {
                "horizontal": delvewright_dsl::lightning::REACH_HORIZONTAL,
                "below": delvewright_dsl::lightning::REACH_BELOW,
                "above": delvewright_dsl::lightning::REACH_ABOVE,
            },
            "strikes": self.rows.iter().map(|r| serde_json::json!({
                "path": r.path,
                "mark": r.mark,
                "cell": r.cell,
                "struck": r.struck,
            })).collect::<Vec<_>>(),
        })
    }

    /// The one line this proof owes its reader, printed on every build that
    /// assembles a world — zeroes included.
    pub fn line(&self) -> String {
        format!(
            "lightning binding: {} strike(s) declared, {} struck block(s) read, {} post(s) within \
             reach examined, {} refused",
            self.declared,
            self.rows.len(),
            self.posts,
            self.refused,
        )
    }
}

/// Every strike the campaign declares, deep, in deterministic content order:
/// `(json pointer, mark)`. Roots from the single enumeration, nesting from the
/// single descent authority, as `compiler::firework` reads them.
fn declared_strikes<'a>(plan: &Plan<'a>) -> Vec<(String, &'a delvewright_dsl::Mark)> {
    let mut out: Vec<(String, &'a delvewright_dsl::Mark)> = Vec::new();
    fn descend<'a>(
        path: String,
        eff: &'a delvewright_dsl::QuestEffect,
        out: &mut Vec<(String, &'a delvewright_dsl::Mark)>,
    ) {
        if let Verb::Lightning { at } = &eff.verb {
            out.push((path.clone(), at));
        }
        for (pseg, _kseg, list) in eff.nested_effect_lists_labeled() {
            for (j, inner) in list.iter().enumerate() {
                descend(format!("{path}/{pseg}/{j}"), inner, out);
            }
        }
    }
    crate::compiler::plan::for_each_effect_root(plan.campaign, &mut |site, effs| {
        for (i, eff) in effs.iter().enumerate() {
            descend(format!("{}/{i}", site.path), eff, &mut out);
        }
    });
    out
}

/// Every strike the campaign declares, deep, in content order: `(json pointer,
/// mark)` — the one enumeration the gate, the setup's chunk hold and the
/// PackTest templates read.
pub fn declared<'a>(plan: &Plan<'a>) -> Vec<(String, &'a delvewright_dsl::Mark)> {
    declared_strikes(plan)
}

/// The cell each declared strike lands in, where its mark resolves.
pub fn strike_cells(plan: &Plan) -> Vec<[i32; 3]> {
    declared_strikes(plan)
        .into_iter()
        .filter_map(|(_, m)| plan.point_any(m.anchor.as_str()).map(|a| m.cell(a)))
        .collect()
}

/// True if this campaign declares a strike at all — the predicate that decides
/// whether the build owes the proof (and therefore the assembled world it reads).
pub fn declares_one(plan: &Plan) -> bool {
    !declared_strikes(plan).is_empty()
}

/// **One resolved strike**: what [`judge`] is asked about, once the plan has
/// said where the mark is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Strike {
    /// The JSON pointer the effect was found at.
    pub path: String,
    /// The mark, as a diagnostic spells it.
    pub mark: String,
    /// The cell the bolt stands in.
    pub cell: [i32; 3],
}

/// **Prove every strike clear of every posted body and every block it would
/// rewrite** (`DW0958`, `DW0959`). Resolves the strikes against the solved
/// layout and the posts against `DW0511`'s own enumeration, then hands both to
/// [`judge`], which is where the rules live.
pub fn check(
    plan: &Plan,
    blocks: &crate::compiler::blockstate::BlockMap,
    entry: Option<[i32; 3]>,
    wave_seats: &BTreeMap<String, Vec<[i32; 3]>>,
) -> (LightningGate, Vec<Failure>) {
    let declared = declared_strikes(plan);
    if declared.is_empty() {
        return (LightningGate::default(), Vec::new());
    }
    let mut strikes: Vec<Strike> = Vec::new();
    for (path, mark) in &declared {
        // `DW0360` owns a mark whose anchor resolves to nothing.
        if let Some(anchor) = plan.point_any(mark.anchor.as_str()) {
            strikes.push(Strike {
                path: path.clone(),
                mark: mark.display(),
                cell: mark.cell(anchor),
            });
        }
    }
    let posts = crate::compiler::lethal::posted_places(plan, entry, wave_seats);
    let (mut gate, findings) = judge(&strikes, &posts, blocks);
    gate.declared = declared.len();
    (gate, findings)
}

/// Whether a body standing on `post` meets the box of a bolt standing in
/// `cell` (spec-0092 §5). The bolt stands at the cell's centre on its plane; its
/// box runs [`delvewright_dsl::lightning::REACH_HORIZONTAL`] out on each
/// horizontal axis, [`delvewright_dsl::lightning::REACH_BELOW`] down and
/// [`delvewright_dsl::lightning::REACH_ABOVE`] up. The body stands at its
/// cell's centre with its hitbox's width and height. Two boxes meet when they
/// overlap on every axis, strictly — the test `AABB.intersects` makes.
pub fn in_reach(cell: [i32; 3], post: [i32; 3], body: delvewright_dsl::metrics::Body) -> bool {
    use delvewright_dsl::lightning::{REACH_ABOVE, REACH_BELOW, REACH_HORIZONTAL};
    let (dx, dy, dz) = (
        f64::from(post[0] - cell[0]),
        f64::from(post[1] - cell[1]),
        f64::from(post[2] - cell[2]),
    );
    let side = f64::from(REACH_HORIZONTAL) + body.width / 2.0;
    dx.abs() < side
        && dz.abs() < side
        && dy + body.height > -f64::from(REACH_BELOW)
        && dy < f64::from(REACH_ABOVE)
}

/// **The rules themselves** (spec-0092 §5), over resolved strikes and resolved
/// posts. Separated from [`check`] so each geometry can be asked directly.
/// Returns the binding beside the findings, in content order; `declared` is
/// the caller's to fill.
pub fn judge(
    strikes: &[Strike],
    posts: &[crate::compiler::lethal::PostedPlace],
    blocks: &crate::compiler::blockstate::BlockMap,
) -> (LightningGate, Vec<Failure>) {
    use delvewright_dsl::lightning;
    let mut gate = LightningGate {
        declared: strikes.len(),
        ..Default::default()
    };
    let mut findings: Vec<Failure> = Vec::new();
    for Strike { path, mark, cell } in strikes {
        let cell = *cell;
        let below = [cell[0], cell[1] - 1, cell[2]];
        let struck = blocks.get(&below).map(|b| b.as_str().to_string());
        gate.rows.push(StrikeRow {
            path: path.clone(),
            mark: mark.clone(),
            cell,
            struck: struck.clone(),
        });
        gate.posts += posts.len();

        if let Some(block) = struck.as_deref().filter(|b| lightning::rewrites(b)) {
            findings.push(Failure {
                code: DW_LIGHTNING_REWRITES_BLOCK,
                message: format!(
                    "the `lightning` at `{path}` strikes `{mark}` at {cell:?}, and the block it \
                     strikes — the one under the mark, at {below:?} — is `{block}`. The game \
                     rewrites that block when a bolt strikes it: a lightning rod is powered, and \
                     weathering copper is scraped back along a random walk through the copper \
                     beside it, a world write the game rolls and no proof of this engine can \
                     state. Move the mark, or put a block the bolt leaves alone under it."
                ),
            });
        }

        let caught: Vec<&crate::compiler::lethal::PostedPlace> = posts
            .iter()
            .filter(|p| in_reach(cell, p.cell, p.body))
            .collect();
        if !caught.is_empty() {
            let named = caught
                .iter()
                .map(|p| format!("{} at {:?}", p.label, p.cell))
                .collect::<Vec<_>>()
                .join("; ");
            findings.push(Failure {
                code: DW_LIGHTNING_POST_IN_REACH,
                message: format!(
                    "the `lightning` at `{path}` strikes `{mark}` at {cell:?}, and {n} place(s) \
                     the campaign posts a body lie in its reach — a body there meets the box \
                     {h} blocks out on each side, {b} below and {a} above the bolt: {named}. The \
                     bolt deals {hp} HP and sets a body alight for {burn} seconds, turns a \
                     villager into a witch and a pig into a zombified piglin, and charges a \
                     creeper, so whatever the campaign puts there is struck every time the beat \
                     fires. Move the mark or move the post.",
                    n = caught.len(),
                    h = lightning::REACH_HORIZONTAL,
                    b = lightning::REACH_BELOW,
                    a = lightning::REACH_ABOVE,
                    hp = lightning::DAMAGE_HP,
                    burn = lightning::BURN_SECONDS,
                ),
            });
        }
    }
    gate.refused = findings.len();
    (gate, findings)
}
