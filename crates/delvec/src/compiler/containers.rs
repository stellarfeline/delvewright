//! **The container census** (spec-0105): every container of the built world is
//! loot, a device or scenery, and the build proves which.
//!
//! The engine fills the containers a `loot` entry or a `collect` names and knew
//! nothing about the rest, so a campaign whose every barrel was empty shipped
//! without a word. This module reads the **edited assembled model** — the same
//! cells every later proof reads — collects every cell whose block is one of the
//! twelve container kinds ([`delvewright_dsl::blocks::CONTAINER_KINDS_1_21_11`]),
//! and resolves the claims the campaign's declarations make on those cells:
//!
//! | claim | from | cell |
//! |---|---|---|
//! | `loot` | a `loot[]` entry | its anchor's cell |
//! | `collect` | a `collect` adopting `container`, or one with neither `container` nor `dropped_by` (the compiler's own chest) | the adopted cell; the `anchor` cell, counted from the declaration because the model holds air there until runtime |
//! | `device` | a `trapped-chest` trap; a `use` / `strike` trigger or an `interact` objective whose cell holds a container or whose `prop` is one | the `at` / `anchor` cell |
//! | `decoration` | nothing | the remainder |
//!
//! Three verdicts are judged on the result: `DW1018` (the containers built and
//! the containers declared disagree — scenery with no notice, or a notice over
//! a world with no scenery) and `DW1024` (scenery arranged as a storeroom, or a
//! container that matters standing among scenery, spec-0105 §9).

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use crate::compiler::assembled::base_id;
use crate::compiler::blockstate::BlockMap;
use crate::compiler::failure::Failure;
use crate::compiler::plan::{Plan, ResolvedAnchor};
use delvewright_dsl::{DwCode, ExitTier, Narration, Objective, TrapTrigger, TriggerOn};

delvewright_dsl::dw_code! {
    /// (spec-0105 §6.1) The containers built and the containers declared
    /// disagree: a container nothing claims with no `decoration_notice`, or a
    /// notice over a world where every container is claimed.
    pub const DW_CONTAINERS_UNDECLARED: DwCode = DwCode::new("DW1018", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// (spec-0105 §9) Scenery containers arranged as a storeroom (more than
    /// [`MAX_SCENERY_PER_PLACE`] in one place), or a container that matters —
    /// loot, a collect or a device — standing in a place with scenery.
    pub const DW_CONTAINERS_ARRANGED: DwCode = DwCode::new("DW1024", ExitTier::Build);
}

/// **The most scenery container cells one place may hold** (spec-0105 §9.2).
///
/// Authored, from the *Oblivion* Construction Set wiki's dungeon practice: an
/// ordinary room holds one to three containers, one or two of them junk. Junk
/// there still holds a stack; scenery here holds nothing, so the cap is the
/// practice's upper bound of non-paying containers. A double chest is two cells
/// and counts two. The signal that would show it wrong: a playtest of a player
/// opening every container of a place that passed (too high), or a place refused
/// at three where a reader at playable scale sees furniture (too low).
pub const MAX_SCENERY_PER_PLACE: usize = 2;

/// The neighbourhood name of a cell that lies in no place.
pub const OUTSIDE_EVERY_PLACE: &str = "(outside every place)";

/// What a declaration claims a container cell as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ClaimKind {
    /// A `loot[]` fill.
    Loot,
    /// A `collect` objective's container.
    Collect,
    /// A trap, trigger or objective that acts on the container.
    Device,
}

impl ClaimKind {
    /// The ledger token.
    #[must_use]
    pub fn token(self) -> &'static str {
        match self {
            ClaimKind::Loot => "loot",
            ClaimKind::Collect => "collect",
            ClaimKind::Device => "device",
        }
    }
}

/// One claim a declaration makes on a cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    /// What kind of claim.
    pub kind: ClaimKind,
    /// The claiming declaration's id.
    pub id: String,
    /// The cell claimed.
    pub cell: [i32; 3],
    /// The container the compiler itself places at this cell at runtime (a
    /// `collect`'s own chest, a container `prop`): the census counts it from
    /// the declaration, since the model holds no container there. `None` for a
    /// claim on a container the world already has.
    pub placed: Option<String>,
}

/// One container cell of the built world.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerRow {
    /// The cell.
    pub cell: [i32; 3],
    /// The container's base block id.
    pub block: String,
    /// Its slot count.
    pub slots: usize,
    /// The place whose piece holds it ([`OUTSIDE_EVERY_PLACE`] when none).
    pub place: String,
    /// What claims it, in kind then declaration order. Empty = decoration.
    pub claims: Vec<(ClaimKind, String)>,
}

impl ContainerRow {
    /// Whether no declaration claims this container.
    #[must_use]
    pub fn is_decoration(&self) -> bool {
        self.claims.is_empty()
    }

    fn has(&self, kind: ClaimKind) -> bool {
        self.claims.iter().any(|(k, _)| *k == kind)
    }
}

/// The census of one build.
#[derive(Debug, Clone, PartialEq)]
pub struct Census {
    /// Every container cell, in cell order.
    pub rows: Vec<ContainerRow>,
    /// The declared notice, when there is one.
    pub notice: Option<Narration>,
    /// SHA-256 of the census input — every cell and state of the model it read.
    pub input_digest: String,
}

/// What one place holds that breaks §9.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceFinding {
    /// The place.
    pub place: String,
    /// Its decoration cells.
    pub decoration: Vec<ContainerRow>,
    /// Its claimed cells (the mixed arm).
    pub matter: Vec<ContainerRow>,
}

fn cell_str(c: [i32; 3]) -> String {
    format!("[{}, {}, {}]", c[0], c[1], c[2])
}

/// The census input's digest: every `(cell, state)` of the model, in its order.
#[must_use]
pub fn model_digest(blocks: &BlockMap) -> String {
    let mut h = Sha256::new();
    for (c, s) in blocks {
        h.update(format!("{} {} {} {}\n", c[0], c[1], c[2], s.as_str()).as_bytes());
    }
    format!("{:x}", h.finalize())
}

/// **Take the census**: every container cell of `blocks`, every runtime
/// container a claim places, each attributed to its place and its claims.
pub fn census(
    blocks: &BlockMap,
    claims: &[Claim],
    place_of: &dyn Fn([i32; 3]) -> Option<String>,
    notice: Option<&Narration>,
) -> Census {
    let mut rows: BTreeMap<[i32; 3], ContainerRow> = BTreeMap::new();
    let row = |cell: [i32; 3], block: &str, slots: usize| ContainerRow {
        cell,
        block: base_id(block).to_string(),
        slots,
        place: place_of(cell).unwrap_or_else(|| OUTSIDE_EVERY_PLACE.to_string()),
        claims: Vec::new(),
    };
    for (cell, state) in blocks {
        if let Some(slots) = delvewright_dsl::blocks::container_slots(state.as_str()) {
            rows.insert(*cell, row(*cell, state.as_str(), slots));
        }
    }
    for c in claims {
        if !rows.contains_key(&c.cell) {
            let Some(block) = &c.placed else { continue };
            let Some(slots) = delvewright_dsl::blocks::container_slots(block) else {
                continue;
            };
            rows.insert(c.cell, row(c.cell, block, slots));
        }
        if let Some(r) = rows.get_mut(&c.cell) {
            r.claims.push((c.kind, c.id.clone()));
        }
    }
    let mut rows: Vec<ContainerRow> = rows.into_values().collect();
    for r in &mut rows {
        r.claims.sort();
        r.claims.dedup();
    }
    Census {
        rows,
        notice: notice.cloned(),
        input_digest: model_digest(blocks),
    }
}

impl Census {
    /// Cells carrying a claim of `kind`.
    #[must_use]
    pub fn count(&self, kind: ClaimKind) -> usize {
        self.rows.iter().filter(|r| r.has(kind)).count()
    }

    /// Decoration cells.
    #[must_use]
    pub fn decoration(&self) -> usize {
        self.rows.iter().filter(|r| r.is_decoration()).count()
    }

    fn by<F: Fn(&ContainerRow) -> String>(&self, f: F) -> BTreeMap<String, usize> {
        let mut m = BTreeMap::new();
        for r in &self.rows {
            *m.entry(f(r)).or_insert(0) += 1;
        }
        m
    }

    /// The places breaking §9, mixed arm then storeroom arm, each in place order.
    #[must_use]
    pub fn arrangement(&self) -> (Vec<PlaceFinding>, Vec<PlaceFinding>) {
        let mut places: BTreeMap<&str, (Vec<ContainerRow>, Vec<ContainerRow>)> = BTreeMap::new();
        for r in &self.rows {
            let e = places.entry(r.place.as_str()).or_default();
            if r.is_decoration() {
                e.0.push(r.clone());
            } else {
                e.1.push(r.clone());
            }
        }
        let mut mixed = Vec::new();
        let mut over = Vec::new();
        for (place, (decoration, matter)) in places {
            if !decoration.is_empty() && !matter.is_empty() {
                mixed.push(PlaceFinding {
                    place: place.to_string(),
                    decoration: decoration.clone(),
                    matter,
                });
            }
            if decoration.len() > MAX_SCENERY_PER_PLACE {
                over.push(PlaceFinding {
                    place: place.to_string(),
                    decoration,
                    matter: Vec::new(),
                });
            }
        }
        (mixed, over)
    }

    /// The binding line (spec-0105 §5.1), for stderr on every build.
    #[must_use]
    pub fn line(&self) -> String {
        let join = |m: BTreeMap<String, usize>| {
            m.iter()
                .map(|(k, n)| format!("{k}: {n}"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let blocks = join(self.by(|r| r.block.clone()));
        let places = join(self.by(|r| r.place.clone()));
        let (mixed, over) = self.arrangement();
        format!(
            "container census: {n} container cell(s) in the assembled world{blocks}; {l} loot, \
             {c} collect, {v} device, {d} decoration; notice {notice}; by place: {places}; \
             arrangement: {m} mixed place(s), {o} over the cap of {cap} decoration cell(s) per \
             place.",
            n = self.rows.len(),
            blocks = if blocks.is_empty() {
                String::new()
            } else {
                format!(" ({blocks})")
            },
            l = self.count(ClaimKind::Loot),
            c = self.count(ClaimKind::Collect),
            v = self.count(ClaimKind::Device),
            d = self.decoration(),
            notice = if self.notice.is_some() {
                "declared"
            } else {
                "none"
            },
            places = if places.is_empty() {
                "none".to_string()
            } else {
                places
            },
            m = mixed.len(),
            o = over.len(),
            cap = MAX_SCENERY_PER_PLACE,
        )
    }

    /// `DW1018` (spec-0105 §6.1): scenery with no notice, or a notice with no
    /// scenery. `anchor_at` names the anchor standing on a cell, so a remedy can
    /// offer the fill an author can write.
    pub fn check_declared(
        &self,
        anchor_at: &BTreeMap<[i32; 3], (String, String)>,
    ) -> Result<(), Failure> {
        let d = self.decoration();
        if d > 0 && self.notice.is_none() {
            let list = self
                .rows
                .iter()
                .filter(|r| r.is_decoration())
                .map(|r| {
                    let fill = match anchor_at.get(&r.cell) {
                        Some((area, name)) => format!(
                            "; anchor `{name}` in `{area}` stands on it — a `loot` entry on \
                             that anchor fills it"
                        ),
                        None => "; no anchor stands on it — the piece or the program of its \
                                 place lands it, so fill it by giving that place an anchor there, \
                                 or replace it"
                            .to_string(),
                    };
                    format!(
                        "\n  `{}` at {} in `{}` ({} slots){fill}",
                        r.block,
                        cell_str(r.cell),
                        r.place,
                        r.slots
                    )
                })
                .collect::<String>();
            return Err(Failure {
                code: DW_CONTAINERS_UNDECLARED,
                message: format!(
                    "{d} of the {n} container cell(s) in the assembled world are claimed by \
                     nothing — no `loot` fill, no `collect`, no trap, trigger or objective acts \
                     on them — and the campaign declares no `containers.decoration_notice`, so \
                     a player would open them to an empty grid with nothing having told them \
                     the place holds nothing:{list}\n  Two remedies, and the choice is yours: \
                     FILL each one (a `loot` entry on its anchor), or declare \
                     `containers.decoration_notice` in `quests.json` — one line, in the \
                     campaign's words, told to each player at arrival, saying what the \
                     containers of this place hold. A campaign whose every container is \
                     claimed passes with 0 decoration.",
                    n = self.rows.len(),
                ),
            });
        }
        if d == 0 && self.notice.is_some() {
            return Err(Failure {
                code: DW_CONTAINERS_UNDECLARED,
                message: format!(
                    "`containers.decoration_notice` is declared, but every one of the {n} \
                     container cell(s) in the assembled world is claimed — loot, a collect or \
                     a device — so the notice would tell the player something false about the \
                     world: there is no scenery container for it to speak for. Delete \
                     `containers.decoration_notice`.",
                    n = self.rows.len()
                ),
            });
        }
        Ok(())
    }

    /// `DW1024` (spec-0105 §9.3): a container that matters among scenery, or a
    /// place holding more scenery containers than [`MAX_SCENERY_PER_PLACE`].
    pub fn check_arrangement(&self) -> Result<(), Failure> {
        let (mixed, over) = self.arrangement();
        if mixed.is_empty() && over.is_empty() {
            return Ok(());
        }
        let cells = |rs: &[ContainerRow]| {
            rs.iter()
                .map(|r| {
                    let claims = if r.claims.is_empty() {
                        "scenery".to_string()
                    } else {
                        r.claims
                            .iter()
                            .map(|(k, id)| format!("{} `{id}`", k.token()))
                            .collect::<Vec<_>>()
                            .join(" + ")
                    };
                    format!("`{}` at {} ({claims})", r.block, cell_str(r.cell))
                })
                .collect::<Vec<_>>()
                .join(", ")
        };
        let mut parts = Vec::new();
        for f in &mixed {
            parts.push(format!(
                "\n  MIXED — `{}`: the container(s) that matter, {}, stand among the scenery \
                 container(s) {}. A player searching for what matters opens every one of them. \
                 Fill every scenery container in this place so each pays off, replace it in the \
                 place's piece or program with furniture that is not a container, or move the \
                 one that matters to a place where every container pays off. A trap among loot \
                 is allowed; a needed item among empty containers is not.",
                f.place,
                cells(&f.matter),
                cells(&f.decoration)
            ));
        }
        for f in &over {
            parts.push(format!(
                "\n  STOREROOM — `{}` holds {} scenery container cell(s), more than the {} a \
                 place may hold: {}. Many empty containers together read as a storeroom, and a \
                 player opens every one. Fill some of them, or replace the surplus in the \
                 place's piece or program with furniture that is not a container (a double \
                 chest is two cells).",
                f.place,
                f.decoration.len(),
                MAX_SCENERY_PER_PLACE,
                cells(&f.decoration)
            ));
        }
        Err(Failure {
            code: DW_CONTAINERS_ARRANGED,
            message: format!(
                "{} mixed place(s) and {} place(s) over the cap of {} scenery container cell(s): \
                 scenery containers stay few, and a container that matters never stands among \
                 them.{}",
                mixed.len(),
                over.len(),
                MAX_SCENERY_PER_PLACE,
                parts.concat()
            ),
        })
    }

    /// The ledger (spec-0105 §4.3): `validation/containers.json`.
    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        use serde_json::json;
        let (mixed, over) = self.arrangement();
        let names = |fs: &[PlaceFinding]| fs.iter().map(|f| f.place.clone()).collect::<Vec<_>>();
        json!({
            "census_input_sha256": self.input_digest,
            "cells": self.rows.len(),
            "loot": self.count(ClaimKind::Loot),
            "collect": self.count(ClaimKind::Collect),
            "device": self.count(ClaimKind::Device),
            "decoration": self.decoration(),
            "by_block": self.by(|r| r.block.clone()),
            "by_place": self.by(|r| r.place.clone()),
            "max_scenery_per_place": MAX_SCENERY_PER_PLACE,
            "mixed_places": names(&mixed),
            "over_cap_places": names(&over),
            "notice": self.notice.as_ref().map(|n| json!({
                "text": delvewright_dsl::l10n::plain(&n.text),
                "style": n.style.unwrap_or(delvewright_dsl::NarrateStyle::Chat).token(),
            })),
            "rows": self.rows.iter().map(|r| json!({
                "cell": r.cell,
                "block": r.block,
                "slots": r.slots,
                "place": r.place,
                "claims": r.claims.iter().map(|(k, id)| json!({"kind": k.token(), "id": id})).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        })
    }
}

/// **The place a cell belongs to**: on a site-plan campaign, the place whose
/// play space holds it (or whose footprint column holds it, the highest floor
/// at or under it); otherwise the area whose placed piece holds it.
#[must_use]
pub fn place_of(plan: &Plan, cell: [i32; 3]) -> Option<String> {
    if let Some(b) = plan.blockout.as_ref() {
        let c = [i64::from(cell[0]), i64::from(cell[1]), i64::from(cell[2])];
        if let Some(bx) = b.boxes.iter().find(|bx| {
            let (lo, hi) = bx.space();
            (0..3).all(|i| c[i] >= lo[i] && c[i] <= hi[i])
        }) {
            return Some(bx.node.0.clone());
        }
        if let Some(bx) = b
            .boxes
            .iter()
            .filter(|bx| {
                c[0] >= bx.foot[0]
                    && c[0] <= bx.foot[1]
                    && c[2] >= bx.foot[2]
                    && c[2] <= bx.foot[3]
                    && bx.floor <= c[1]
            })
            .max_by_key(|bx| bx.floor)
        {
            return Some(bx.node.0.clone());
        }
        return None;
    }
    plan.areas
        .iter()
        .find(|a| {
            a.pieces.iter().any(|p| {
                let (lo, hi) = p.bbox();
                (0..3).all(|i| cell[i] >= lo[i] && cell[i] <= hi[i])
            })
        })
        .map(|a| a.area_id.clone())
}

/// Every claim the campaign's declarations make on a container cell, read off
/// the plan (the same resolutions the fills and the proofs use).
#[must_use]
pub fn claims_of(plan: &Plan, blocks: &BlockMap) -> Vec<Claim> {
    let holds = |cell: [i32; 3]| {
        blocks
            .get(&cell)
            .is_some_and(|s| delvewright_dsl::blocks::container_kind(s.as_str()).is_some())
    };
    let mut out = Vec::new();
    for l in &plan.loot {
        out.push(Claim {
            kind: ClaimKind::Loot,
            id: l.id.clone(),
            cell: l.cell,
            placed: None,
        });
    }
    for f in &plan.collect_fills {
        out.push(Claim {
            kind: ClaimKind::Collect,
            id: f.objective_id.clone(),
            cell: f.cell,
            placed: None,
        });
    }
    let c = plan.campaign;
    for q in &c.quests.content.quests {
        let area = plan.quest_area(q.id.as_str());
        for o in &q.objectives {
            match o {
                Objective::Collect {
                    id,
                    anchor,
                    container: None,
                    dropped_by: None,
                    ..
                } => {
                    let cell = area
                        .and_then(|a| plan.point(a, anchor.as_str()))
                        .or_else(|| plan.point_any(anchor.as_str()));
                    if let Some(cell) = cell {
                        out.push(Claim {
                            kind: ClaimKind::Collect,
                            id: id.as_str().to_string(),
                            cell,
                            placed: Some("minecraft:chest".to_string()),
                        });
                    }
                }
                Objective::Interact {
                    id, anchor, prop, ..
                } => {
                    let cell = area
                        .and_then(|a| plan.point(a, anchor.as_str()))
                        .or_else(|| plan.point_any(anchor.as_str()));
                    let Some(cell) = cell else { continue };
                    let placed = prop
                        .as_ref()
                        .filter(|p| delvewright_dsl::blocks::container_kind(&p.block).is_some())
                        .map(|p| p.block.clone());
                    if placed.is_some() || holds(cell) {
                        out.push(Claim {
                            kind: ClaimKind::Device,
                            id: id.as_str().to_string(),
                            cell,
                            placed,
                        });
                    }
                }
                _ => {}
            }
        }
    }
    for t in &plan.traps {
        if matches!(t.trigger, TrapTrigger::TrappedChest) {
            out.push(Claim {
                kind: ClaimKind::Device,
                id: t.id.clone(),
                cell: t.trigger_cell,
                placed: None,
            });
        }
    }
    for t in &c.quests.content.triggers {
        if !matches!(t.on, TriggerOn::Use | TriggerOn::Strike) {
            continue;
        }
        let Some(cell) = t.at_anchor().and_then(|a| plan.point_any(a)) else {
            continue;
        };
        let placed = t
            .prop
            .as_ref()
            .filter(|p| delvewright_dsl::blocks::container_kind(&p.block).is_some())
            .map(|p| p.block.clone());
        if placed.is_some() || holds(cell) {
            out.push(Claim {
                kind: ClaimKind::Device,
                id: t.id.as_str().to_string(),
                cell,
                placed,
            });
        }
    }
    out
}

/// The anchor standing on each point cell, for a `DW1018` remedy.
#[must_use]
pub fn anchors_by_cell(
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
) -> BTreeMap<[i32; 3], (String, String)> {
    let mut out = BTreeMap::new();
    for ((area, name), r) in anchors {
        if let ResolvedAnchor::Point { pos, .. } = r {
            out.entry(*pos)
                .or_insert_with(|| (area.clone(), name.clone()));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(cells: &[([i32; 3], &str)]) -> BlockMap {
        let mut b = BTreeMap::new();
        for (c, s) in cells {
            b.insert(*c, (*s).to_string());
        }
        crate::compiler::blockstate::interned(b)
    }

    fn claim(kind: ClaimKind, id: &str, cell: [i32; 3], placed: Option<&str>) -> Claim {
        Claim {
            kind,
            id: id.to_string(),
            cell,
            placed: placed.map(str::to_string),
        }
    }

    fn notice() -> Narration {
        Narration {
            text: "The town was stripped.".to_string(),
            style: None,
            sound: None,
        }
    }

    fn one_place(_: [i32; 3]) -> Option<String> {
        Some("area/house".to_string())
    }

    /// spec-0105 criterion 4: one chest claimed by `loot`, one trapped chest
    /// claimed by a trap and `loot` together, one barrel unclaimed and a
    /// `collect` with no container — four rows, `N 4, L 2, C 1, V 1, D 1`.
    #[test]
    fn the_census_counts_the_four_rows() {
        let blocks = model(&[
            ([0, 0, 0], "minecraft:chest[facing=north,type=single]"),
            (
                [2, 0, 0],
                "minecraft:trapped_chest[facing=north,type=single]",
            ),
            ([4, 0, 0], "minecraft:barrel[facing=up,open=false]"),
            ([6, 0, 0], "minecraft:stone"),
        ]);
        let claims = [
            claim(ClaimKind::Loot, "loot/stores", [0, 0, 0], None),
            claim(ClaimKind::Device, "trap/false-chest", [2, 0, 0], None),
            claim(ClaimKind::Loot, "loot/real-chest", [2, 0, 0], None),
            claim(
                ClaimKind::Collect,
                "obj/take",
                [8, 0, 0],
                Some("minecraft:chest"),
            ),
        ];
        let c = census(&blocks, &claims, &one_place, None);
        assert_eq!(c.rows.len(), 4);
        assert_eq!(c.count(ClaimKind::Loot), 2);
        assert_eq!(c.count(ClaimKind::Collect), 1);
        assert_eq!(c.count(ClaimKind::Device), 1);
        assert_eq!(c.decoration(), 1);
        assert_eq!(c.rows[2].block, "minecraft:barrel");
        assert!(c.rows[2].is_decoration());
        assert_eq!(c.rows[3].block, "minecraft:chest");
        assert!(c.line().contains("4 container cell(s)"), "{}", c.line());
        assert!(
            c.line()
                .contains("2 loot, 1 collect, 1 device, 1 decoration"),
            "{}",
            c.line()
        );
    }

    /// A claim on a cell that holds no container and places none binds nothing.
    #[test]
    fn a_claim_on_no_container_makes_no_row() {
        let blocks = model(&[([0, 0, 0], "minecraft:stone")]);
        let c = census(
            &blocks,
            &[claim(ClaimKind::Device, "trigger/x", [0, 0, 0], None)],
            &one_place,
            None,
        );
        assert!(c.rows.is_empty());
        assert!(c.line().contains("0 container cell(s)"), "{}", c.line());
        assert!(c.line().contains("notice none"), "{}", c.line());
        assert!(c.check_declared(&BTreeMap::new()).is_ok());
        assert!(c.check_arrangement().is_ok());
    }

    /// spec-0105 criterion 5: two censuses of one model are one ledger, byte
    /// for byte, and the ledger names the input's digest.
    #[test]
    fn two_censuses_of_one_model_are_one_ledger() {
        let blocks = model(&[
            ([0, 0, 0], "minecraft:chest"),
            ([1, 0, 0], "minecraft:barrel"),
        ]);
        let claims = [claim(ClaimKind::Loot, "loot/a", [0, 0, 0], None)];
        let a = census(&blocks, &claims, &one_place, Some(&notice())).to_json();
        let b = census(&blocks, &claims, &one_place, Some(&notice())).to_json();
        assert_eq!(
            serde_json::to_vec(&a).unwrap(),
            serde_json::to_vec(&b).unwrap()
        );
        assert_eq!(a["census_input_sha256"].as_str().unwrap().len(), 64);
        let other = model(&[([0, 0, 0], "minecraft:chest")]);
        assert_ne!(
            a["census_input_sha256"],
            census(&other, &claims, &one_place, None).to_json()["census_input_sha256"]
        );
    }

    #[test]
    fn scenery_without_a_notice_is_dw1018_listing_the_cell() {
        let blocks = model(&[([1, 2, 3], "minecraft:barrel")]);
        let c = census(&blocks, &[], &one_place, None);
        let e = c.check_declared(&BTreeMap::new()).unwrap_err();
        assert_eq!(e.code, "DW1018");
        assert!(e.message.contains("[1, 2, 3]"), "{}", e.message);
        assert!(e.message.contains("area/house"), "{}", e.message);
        assert!(e.message.contains("decoration_notice"), "{}", e.message);
        // The remedy names the anchor standing on the cell when there is one.
        let mut at = BTreeMap::new();
        at.insert(
            [1, 2, 3],
            ("area/house".to_string(), "anchor/cupboard".to_string()),
        );
        let e = c.check_declared(&at).unwrap_err();
        assert!(e.message.contains("anchor/cupboard"), "{}", e.message);
        // With the notice it passes.
        let c = census(&blocks, &[], &one_place, Some(&notice()));
        assert!(c.check_declared(&BTreeMap::new()).is_ok());
    }

    #[test]
    fn a_notice_over_a_stocked_world_is_dw1018() {
        let blocks = model(&[([0, 0, 0], "minecraft:chest")]);
        let c = census(
            &blocks,
            &[claim(ClaimKind::Loot, "loot/a", [0, 0, 0], None)],
            &one_place,
            Some(&notice()),
        );
        let e = c.check_declared(&BTreeMap::new()).unwrap_err();
        assert_eq!(e.code, "DW1018");
        assert!(e.message.contains("Delete"), "{}", e.message);
    }

    fn two_places(c: [i32; 3]) -> Option<String> {
        Some(
            if c[0] < 100 {
                "area/shop"
            } else {
                "area/crypt"
            }
            .to_string(),
        )
    }

    /// The mixed arm, by perturbation from a passing model: a loot chest alone
    /// in one place and a barrel of scenery in another pass; moving the barrel
    /// into the loot chest's place reds `DW1024`.
    #[test]
    fn a_container_that_matters_among_scenery_is_dw1024_mixed() {
        let claims = [claim(ClaimKind::Loot, "loot/key", [0, 0, 0], None)];
        let passing = model(&[
            ([0, 0, 0], "minecraft:chest"),
            ([200, 0, 0], "minecraft:barrel"),
        ]);
        let c = census(&passing, &claims, &two_places, Some(&notice()));
        assert!(c.check_arrangement().is_ok(), "{:?}", c.check_arrangement());
        assert!(c.line().contains("0 mixed place(s)"), "{}", c.line());
        let perturbed = model(&[
            ([0, 0, 0], "minecraft:chest"),
            ([2, 0, 0], "minecraft:barrel"),
        ]);
        let c = census(&perturbed, &claims, &two_places, Some(&notice()));
        let e = c.check_arrangement().unwrap_err();
        assert_eq!(e.code, "DW1024");
        assert!(e.message.contains("MIXED — `area/shop`"), "{}", e.message);
        assert!(e.message.contains("loot `loot/key`"), "{}", e.message);
        assert!(c.line().contains("1 mixed place(s)"), "{}", c.line());
    }

    /// A trapped chest among loot chests is allowed: every container pays off.
    #[test]
    fn a_trap_among_loot_is_allowed() {
        let blocks = model(&[
            ([0, 0, 0], "minecraft:chest"),
            ([2, 0, 0], "minecraft:trapped_chest"),
        ]);
        let claims = [
            claim(ClaimKind::Loot, "loot/real", [0, 0, 0], None),
            claim(ClaimKind::Device, "trap/false", [2, 0, 0], None),
        ];
        let c = census(&blocks, &claims, &two_places, None);
        assert!(c.check_arrangement().is_ok());
        assert!(c.check_declared(&BTreeMap::new()).is_ok());
    }

    /// The storeroom arm, by perturbation: two scenery cells in a place pass;
    /// a third reds `DW1024`.
    #[test]
    fn a_third_scenery_container_in_a_place_is_dw1024_storeroom() {
        let passing = model(&[
            ([200, 0, 0], "minecraft:barrel"),
            ([202, 0, 0], "minecraft:barrel"),
        ]);
        let c = census(&passing, &[], &two_places, Some(&notice()));
        assert!(c.check_arrangement().is_ok());
        let perturbed = model(&[
            ([200, 0, 0], "minecraft:barrel"),
            ([202, 0, 0], "minecraft:barrel"),
            ([204, 0, 0], "minecraft:chest"),
        ]);
        let c = census(&perturbed, &[], &two_places, Some(&notice()));
        let e = c.check_arrangement().unwrap_err();
        assert_eq!(e.code, "DW1024");
        assert!(
            e.message.contains("STOREROOM — `area/crypt`"),
            "{}",
            e.message
        );
        assert!(c.line().contains("1 over the cap of 2"), "{}", c.line());
    }
}
