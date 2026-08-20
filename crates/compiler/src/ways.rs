//! **What a campaign does with a piece's contingent ways** (spec-0042 §2.4/§2.5).
//!
//! A prefab's spatial contract may declare a traversal edge whose crossability
//! depends on a named region: `laid` — empty as built, opening fills it — or
//! `cleared` — built solid, opening voids it. The prefab checker proves, on the
//! bytes as shipped, that the edge really is severed and that applying the delta
//! really joins it. What it cannot prove is that anything ever *opens* it:
//! "happens" exists only where effects exist. That half is this module's.
//!
//! # Three questions, one pass
//!
//! 1. **Staging** — which ways the placed pieces actually put in the world, with
//!    their world cells, their block and their sign. A way is a fact about a
//!    PLACEMENT, not about a prefab: a piece placed twice puts two ways in the
//!    world, at different coordinates, and an `open-way` must name one of them.
//! 2. **Disposition** — for each staged way, which effect opens it, at which
//!    quest-DAG point, and whether the party is forced to cause that firing;
//!    or that nothing opens it at all. **A door that never opens is content**,
//!    so a never-opened way is reported and is not a finding by itself.
//! 3. **The one red that follows from the other two** — required content
//!    standing beyond a way no forced opening precedes (`DW0548`). That is the
//!    unwinnable delve this surface could otherwise ship: the objective is in the
//!    room, the room is past the break, and the treads are laid by a beat that
//!    happens later, or that nobody has to play, or that nobody wrote.
//!
//! # Whose reachability this is
//!
//! The piece's own — the contract's declared graph, rooted at the `entry` space
//! it declares, with `vision` edges excluded (a sightline is not a traversal
//! claim) and a `drop` traversed forward only. That is deliberately the same
//! reading `crates/grammar`'s reachability walk takes, because the claim being
//! consumed here is the piece's claim; a second, differently-rooted notion of
//! "reachable" would be an instrument disagreeing with the one that proved the
//! piece.
//!
//! Two consequences, both stated rather than discovered:
//!
//! * a space a neighbouring piece could reach through a mated exterior face is
//!   NOT counted as reached, because seams are the face contract's business
//!   (`crate::faces`, `DW0780`) and spec-0042 keeps ways off them entirely;
//! * a `barred` edge is traversed. Its bar is a contract region a `shortcut` or
//!   an `open-gate` opens through the anchor surface, which this module does not
//!   model — and modelling it here would red every campaign that has ever placed
//!   a barred piece, which is a different rule than the one this spec settles.
//!
//! Both readings are conservative in the same direction: they make this red
//! rarer, never commoner. And the whole module binds only to pieces that declare
//! a `way`, so a campaign that stages none is examined and reported as such —
//! never silently skipped.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use delvewright_dsl::prefab::SpatialContract;
use delvewright_dsl::{Diagnostic, DwCode};
use serde_json::json;

use crate::plan::{AreaPlacement, PlanError};
use crate::registry::PrefabRegistry;
use crate::solver::Rotation;

/// `DW0547`: an `open-way` reference does not name exactly one placed way.
pub const DW_WAY_REFERENCE: DwCode = DwCode::every_version("DW0547");

/// `DW0548`: required content stands beyond a way no forced opening precedes.
pub const DW_WAY_UNOPENED: DwCode = DwCode::every_version("DW0548");

/// `DW0549`: a placed piece declares a way the staging could not put in the
/// world — the disposition enumeration binds to fewer ways than exist.
pub const DW_WAY_UNSTAGED: DwCode = DwCode::every_version("DW0549");

/// `DW0555` (advisory): ways are staged and no required element stands behind
/// any of them, so the reachability half of this gate examined nothing.
pub const DW_WAY_UNBOUND: DwCode = DwCode::every_version("DW0555");

/// The space name a contract edge uses for "outside the piece".
const EXTERIOR: &str = "exterior";

/// Which direction opening a way moves in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sign {
    /// The region is empty as built; opening FILLS it with the way's block.
    Laid,
    /// The region stands in the way's block as built; opening VOIDS it.
    Cleared,
}

impl Sign {
    /// The metadata spelling (`laid` / `cleared`), which is also the word a
    /// verdict uses.
    pub fn word(self) -> &'static str {
        match self {
            Sign::Laid => "laid",
            Sign::Cleared => "cleared",
        }
    }

    fn parse(s: &str) -> Option<Sign> {
        match s {
            "laid" => Some(Sign::Laid),
            "cleared" => Some(Sign::Cleared),
            _ => None,
        }
    }
}

/// One way, as a placed piece puts it in the world.
#[derive(Debug, Clone)]
pub struct StagedWay {
    /// The area the carrying piece was placed in.
    pub area_id: String,
    /// The carrying prefab id (`prefab/<name>`).
    pub prefab_id: String,
    /// Which placement within that area, in placement order — what makes two
    /// copies of one prefab two different ways.
    pub placement: usize,
    /// The way's region name, as the contract exports it. What an `open-way`
    /// names.
    pub name: String,
    /// Which direction opening moves in.
    pub sign: Sign,
    /// The block state a `laid` way is filled with, and a `cleared` way stands
    /// in. Read from the metadata and never from the campaign.
    pub block: String,
    /// The way's world boxes (inclusive corners), in metadata order.
    pub boxes: Vec<([i32; 3], [i32; 3])>,
    /// How many world cells the boxes cover.
    pub cells: usize,
}

impl StagedWay {
    /// `prefab/x in area/y (placement 2) way \`deck\`` — how a diagnostic points
    /// at one way of one placement.
    pub fn describe(&self) -> String {
        format!(
            "way `{}` ({}) of `{}` placed in area `{}` (placement {})",
            self.name,
            self.sign.word(),
            self.prefab_id,
            self.area_id,
            self.placement
        )
    }
}

/// Every way the placed world stages, with the counts that say what this
/// examined.
#[derive(Debug, Clone, Default)]
pub struct WayStaging {
    /// The staged ways, in area → placement → declaration order.
    pub ways: Vec<StagedWay>,
    /// Distinct (placement, way name) pairs the placed pieces DECLARE — the
    /// number `ways` is measured against.
    pub declared: usize,
    /// Placed pieces whose contract declares at least one way.
    pub pieces_with_ways: usize,
    /// Placed pieces in all.
    pub pieces: usize,
}

impl WayStaging {
    /// Every staged way a `(piece, name)` reference matches. Zero or several is
    /// `DW0547`'s finding, and it is the CALLER's, because "the reference names
    /// a placement" is a rule about the reference rather than about the world.
    pub fn matching(&self, prefab_id: &str, name: &str) -> Vec<&StagedWay> {
        self.ways
            .iter()
            .filter(|w| w.prefab_id == prefab_id && w.name == name)
            .collect()
    }

    /// The one way a reference names, or the refusal that says why not.
    pub fn resolve(&self, prefab_id: &str, name: &str) -> Result<&StagedWay, PlanError> {
        let hits = self.matching(prefab_id, name);
        match hits.len() {
            1 => Ok(hits[0]),
            0 => Err(PlanError::new(
                DW_WAY_REFERENCE,
                format!(
                    "an `open-way` names way `{name}` of `{prefab_id}`, and the placed world \
                     stages no such way. {}. Geometry, block and sign all come from the piece's \
                     own metadata, so a way this campaign can open is one some placed piece \
                     exports: check the piece is bound to an area (or drawn by its pool), and \
                     that its `spatial_contract` declares an edge whose `way.region` is `{name}`",
                    self.inventory()
                ),
            )),
            n => Err(PlanError::new(
                DW_WAY_REFERENCE,
                format!(
                    "an `open-way` names way `{name}` of `{prefab_id}`, and {n} placed pieces \
                     stage it ({}). A way is a fact about a PLACEMENT — two copies of one piece \
                     put two breaks in the world, at different coordinates — so this reference \
                     opens no one of them rather than all of them. Place the way-carrying piece \
                     once, or give the second placement its own piece",
                    hits.iter()
                        .map(|w| w.describe())
                        .collect::<Vec<_>>()
                        .join("; ")
                ),
            )),
        }
    }

    /// What the world does stage, for a refusal to print instead of nothing.
    fn inventory(&self) -> String {
        if self.ways.is_empty() {
            format!(
                "no placed piece stages any way at all ({} placed piece(s), {} of them declaring \
                 one)",
                self.pieces, self.pieces_with_ways
            )
        } else {
            format!(
                "the placed world stages: {}",
                self.ways
                    .iter()
                    .map(|w| w.describe())
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        }
    }

    /// **The binding gate** (spec-0042 AC11): every way a placed piece declares
    /// must reach the enumeration, or the enumeration is judging a smaller world
    /// than the one being shipped.
    ///
    /// A way with no cells is the shape this catches: it is declared, it is
    /// named in the metadata, an `open-way` on it would emit nothing and the
    /// disposition line would say nothing — and every count downstream would
    /// still read as a pass. A green that examined fewer objects than exist is
    /// not a pass (CLAUDE.md), so it is a refusal.
    pub fn seal(&self) -> Result<(), PlanError> {
        if self.declared <= self.ways.len() {
            return Ok(());
        }
        Err(PlanError::new(
            DW_WAY_UNSTAGED,
            format!(
                "the placed world declares {} contingent way(s) across {} piece(s) and only {} of \
                 them reached the way enumeration, so {} way(s) are in the world with no cells to \
                 open. A way's whole content is the cells its opening writes: one that resolves \
                 to none is a break nothing can repair, and every disposition, ledger count and \
                 reachability verdict past this point would be stated over a world smaller than \
                 the one being shipped. Fix the piece's metadata — its `way.boxes` are empty",
                self.declared,
                self.pieces_with_ways,
                self.ways.len(),
                self.declared - self.ways.len()
            ),
        ))
    }
}

/// **Every way the placed world stages.**
///
/// Reads the same `spatial_contract` block `crate::faces` reads, through the
/// same placement transform, so a way's cells land where the piece's blocks
/// land — rotation included. Ways of one name on several edges of one piece
/// UNION, exactly as the grammar's own reachability walk unions them: one name
/// is one region however many edges are contingent on it.
pub fn stage(areas: &[AreaPlacement], prefabs: &PrefabRegistry) -> WayStaging {
    let mut out = WayStaging::default();
    for area in areas {
        for (index, placement) in area.pieces.iter().enumerate() {
            out.pieces += 1;
            let Some(contract) = prefabs
                .get(&placement.prefab_id)
                .and_then(|m| m.spatial_contract.as_ref())
            else {
                continue;
            };
            // Declaration order, unioned by name — a `BTreeMap` would sort the
            // ways alphabetically and make the ledger's order an accident of
            // naming rather than of the document.
            let mut order: Vec<String> = Vec::new();
            let mut by_name: BTreeMap<String, StagedWay> = BTreeMap::new();
            for edge in &contract.edges {
                let Some(way) = &edge.way else { continue };
                let Some(sign) = Sign::parse(&way.opens) else {
                    // A sign this engine does not model. The metadata reader
                    // reports the unknown value (`DW0543`); staging a way whose
                    // direction is unknown would be inventing one.
                    continue;
                };
                let entry = by_name.entry(way.region.clone()).or_insert_with(|| {
                    order.push(way.region.clone());
                    StagedWay {
                        area_id: area.area_id.clone(),
                        prefab_id: placement.prefab_id.clone(),
                        placement: index,
                        name: way.region.clone(),
                        sign,
                        block: way.block.clone(),
                        boxes: Vec::new(),
                        cells: 0,
                    }
                });
                for b in &way.boxes {
                    let a = world_cell(placement.rotation, placement.pos, b.from);
                    let c = world_cell(placement.rotation, placement.pos, b.to);
                    let lo = [a[0].min(c[0]), a[1].min(c[1]), a[2].min(c[2])];
                    let hi = [a[0].max(c[0]), a[1].max(c[1]), a[2].max(c[2])];
                    entry.cells += crate::assembled::region_cells(lo, hi).count();
                    entry.boxes.push((lo, hi));
                }
            }
            if by_name.is_empty() {
                continue;
            }
            out.pieces_with_ways += 1;
            out.declared += by_name.len();
            for name in order {
                let staged = by_name.remove(&name).expect("named in declaration order");
                if staged.boxes.is_empty() {
                    // Declared and unstageable — `WayStaging::seal` is what says
                    // so, once, with the count.
                    continue;
                }
                out.ways.push(staged);
            }
        }
    }
    out
}

/// One `open-way` firing, as the quest DAG sees it.
///
/// Built from the SAME root classification the region-write model uses
/// (`plan::firing_of`), so the forcedness this reports and the forcedness the
/// completability model credits cannot disagree — they are one reading of one
/// site, not two readings that happen to agree today.
#[derive(Debug, Clone)]
pub struct WayOpening {
    /// The prefab id the effect named.
    pub prefab_id: String,
    /// The way name the effect named.
    pub way: String,
    /// The effect's JSON path, for a diagnostic to point at.
    pub path: String,
    /// The stage the effect was written in.
    pub stage: &'static str,
    /// The critical-path step this firing happens at.
    pub fire_step: usize,
    /// Whether the party is guaranteed to cause it.
    pub forced: bool,
}

impl WayOpening {
    /// `the effect at \`…\`` — how a verdict names the beat.
    pub fn describe(&self) -> String {
        format!("the `open-way` at `{}` ({})", self.path, self.stage)
    }
}

/// What became of one staged way in this campaign.
#[derive(Debug, Clone)]
pub struct Disposition<'a> {
    /// The way.
    pub way: &'a StagedWay,
    /// Every `open-way` that resolves to it, in campaign order. Empty is the
    /// never-opened case — reported, and not a finding by itself.
    pub opened_by: Vec<&'a WayOpening>,
}

impl Disposition<'_> {
    /// Whether any firing that opens this way is one the party cannot avoid.
    pub fn has_forced(&self) -> bool {
        self.opened_by.iter().any(|o| o.forced)
    }

    /// The disposition as the ledger states it.
    fn to_json(&self) -> serde_json::Value {
        json!({
            "area": self.way.area_id,
            "piece": self.way.prefab_id,
            "placement": self.way.placement,
            "way": self.way.name,
            "opens": self.way.sign.word(),
            "block": self.way.block,
            "cells": self.way.cells,
            "opened_by": self.opened_by.iter().map(|o| json!({
                "path": o.path,
                "stage": o.stage,
                "fire_step": o.fire_step,
                "forced": o.forced,
            })).collect::<Vec<_>>(),
        })
    }
}

/// The way gate's binding ledger — what the enumeration examined, stated with
/// its verdict (playtest-methodology rule 1).
#[derive(Debug, Clone)]
pub struct WayGate {
    /// Placed pieces in all.
    pub pieces: usize,
    /// Placed pieces declaring at least one way.
    pub pieces_with_ways: usize,
    /// Ways staged in the world.
    pub staged: usize,
    /// Staged ways some `open-way` opens.
    pub opened: usize,
    /// Staged ways only an unforced firing opens.
    pub unforced_only: usize,
    /// Staged ways nothing opens.
    pub never_opened: usize,
    /// `open-way` effects the campaign writes.
    pub openings: usize,
    /// Required elements this pass resolved into a way-carrying piece's declared
    /// space — the number that makes the reachability half non-vacuous. Zero
    /// means no required content stands inside a way-carrying piece at all, and
    /// the ledger says so rather than reading as a pass.
    pub elements_examined: usize,
    /// Per-way disposition rows.
    pub rows: Vec<serde_json::Value>,
}

impl WayGate {
    /// The artifact written to `validation/ways.json`.
    pub fn to_json(&self) -> serde_json::Value {
        json!({
            "pieces": self.pieces,
            "pieces_with_ways": self.pieces_with_ways,
            "staged": self.staged,
            "opened": self.opened,
            "unforced_only": self.unforced_only,
            "never_opened": self.never_opened,
            "open_way_effects": self.openings,
            "elements_examined": self.elements_examined,
            "ways": self.rows,
        })
    }
}

/// A required element: a place the campaign says the party has to reach.
#[derive(Debug, Clone)]
pub struct RequiredElement {
    /// What it is, in the author's vocabulary (`objective \`x\``, `npc \`y\``).
    pub what: String,
    /// The area it was resolved in.
    pub area_id: String,
    /// Its world cell.
    pub pos: [i32; 3],
    /// The critical-path step by which it must be reachable, when the campaign
    /// orders it. `None` for an element with no step of its own — a body placed
    /// at world load, an anchor a trigger names — which is then judged on the
    /// "is it ever forced open" half alone.
    pub by_step: Option<usize>,
}

/// **The disposition enumeration and its one red** (spec-0042 §2.5).
///
/// `precedes(fire_step, by_step)` is the caller's DAG-ancestry predicate — the
/// same `Plan::gate_fired_before` the region-write model uses, passed in rather
/// than re-derived, so this verdict and the route proof order the world the same
/// way.
pub fn judge(
    staging: &WayStaging,
    openings: &[WayOpening],
    elements: &[RequiredElement],
    areas: &[AreaPlacement],
    prefabs: &PrefabRegistry,
    precedes: &dyn Fn(usize, usize) -> bool,
) -> Result<WayGate, PlanError> {
    // **Every reference names exactly one staged way, first.** A reference that
    // names none is not a way that fails to open in time — it is a way that is
    // not there, and reporting the consequence before the cause sends an author
    // to look at the quest DAG for a typo in a name.
    for opening in openings {
        staging
            .resolve(&opening.prefab_id, &opening.way)
            .map_err(|e| {
                PlanError::new(
                    e.code,
                    format!("{} — written as {}", e.message, opening.describe()),
                )
            })?;
    }

    let dispositions: Vec<Disposition> = staging
        .ways
        .iter()
        .map(|way| Disposition {
            way,
            opened_by: openings
                .iter()
                .filter(|o| o.prefab_id == way.prefab_id && o.way == way.name)
                .collect(),
        })
        .collect();

    let mut examined = 0usize;
    for element in elements {
        let Some((placement_index, prefab_id, contract, local)) =
            piece_holding(areas, prefabs, &element.area_id, element.pos)
        else {
            continue;
        };
        let staged_here: Vec<&Disposition> = dispositions
            .iter()
            .filter(|d| {
                d.way.area_id == element.area_id
                    && d.way.prefab_id == prefab_id
                    && d.way.placement == placement_index
            })
            .collect();
        if staged_here.is_empty() {
            continue; // a piece with no way gates nothing
        }
        let Some(space) = space_holding(contract, local) else {
            continue; // not in a declared space: no claim to check
        };
        examined += 1;

        // What is credited for THIS element: a forced opening that the quest DAG
        // guarantees has already fired by the time the party has to be here.
        let credited: BTreeSet<&str> = staged_here
            .iter()
            .filter(|d| {
                d.opened_by.iter().any(|o| {
                    o.forced
                        && match element.by_step {
                            Some(step) => precedes(o.fire_step, step),
                            None => true,
                        }
                })
            })
            .map(|d| d.way.name.as_str())
            .collect();
        if reaches(contract, &credited).contains(space.as_str()) {
            continue;
        }
        let every: BTreeSet<&str> = staged_here.iter().map(|d| d.way.name.as_str()).collect();
        if !reaches(contract, &every).contains(space.as_str()) {
            // Not this rule's finding: the piece's own contract does not reach
            // this space under ANY opening, which is what the prefab checker's
            // reachability gate refuses before the piece ships.
            continue;
        }
        // Which ways would make the difference — the ones a verdict must name.
        let mut blamed: Vec<&Disposition> = staged_here
            .iter()
            .filter(|d| {
                if credited.contains(d.way.name.as_str()) {
                    return false;
                }
                let mut with = credited.clone();
                with.insert(d.way.name.as_str());
                reaches(contract, &with).contains(space.as_str())
            })
            .copied()
            .collect();
        if blamed.is_empty() {
            // No single way opens it: name every way that is not credited, since
            // the element is behind their conjunction.
            blamed = staged_here
                .iter()
                .filter(|d| !credited.contains(d.way.name.as_str()))
                .copied()
                .collect();
        }
        let why: Vec<String> = blamed
            .iter()
            .map(|d| why_not(d, element, precedes))
            .collect();
        return Err(PlanError::new(
            DW_WAY_UNOPENED,
            format!(
                "{} stands in space `{space}` of `{prefab_id}`, which that piece's contract \
                 reaches only through a way this campaign does not open in time: {}. A way is \
                 shut until its `open-way` fires, and a beat the party is not forced to play — or \
                 one they play afterwards — leaves the room on the far side of a break they \
                 cannot cross. Give the way a forced `open-way` on an objective the quest DAG \
                 puts before this one",
                element.what,
                why.join("; and ")
            ),
        ));
    }

    let opened = dispositions.iter().filter(|d| d.has_forced()).count();
    let unforced_only = dispositions
        .iter()
        .filter(|d| !d.opened_by.is_empty() && !d.has_forced())
        .count();
    Ok(WayGate {
        pieces: staging.pieces,
        pieces_with_ways: staging.pieces_with_ways,
        staged: staging.ways.len(),
        opened,
        unforced_only,
        never_opened: dispositions
            .iter()
            .filter(|d| d.opened_by.is_empty())
            .count(),
        openings: openings.len(),
        elements_examined: examined,
        rows: dispositions.iter().map(Disposition::to_json).collect(),
    })
}

/// Why one way does not count for one element — the sentence that names the way,
/// the effect and what is wrong with it.
fn why_not(
    d: &Disposition,
    element: &RequiredElement,
    precedes: &dyn Fn(usize, usize) -> bool,
) -> String {
    if d.opened_by.is_empty() {
        return format!(
            "{} is never opened — no `open-way` in this campaign names it, and {} cell(s) of \
             building stand behind it",
            d.way.describe(),
            d.way.cells
        );
    }
    let unforced: Vec<&&WayOpening> = d.opened_by.iter().filter(|o| !o.forced).collect();
    let late: Vec<&&WayOpening> = d
        .opened_by
        .iter()
        .filter(|o| {
            o.forced
                && element
                    .by_step
                    .is_some_and(|step| !precedes(o.fire_step, step))
        })
        .collect();
    let mut parts: Vec<String> = Vec::new();
    for o in late {
        parts.push(format!(
            "{} is opened by {} at critical-path step {}, which the quest DAG does not put before \
             this one",
            d.way.describe(),
            o.describe(),
            o.fire_step
        ));
    }
    for o in unforced {
        parts.push(format!(
            "{} is opened by {}, a beat the party is not forced to play — an opening nobody has \
             to cause proves nothing about a route that needs it",
            d.way.describe(),
            o.describe()
        ));
    }
    if parts.is_empty() {
        parts.push(format!("{} is not open here", d.way.describe()));
    }
    parts.join("; ")
}

/// The spaces the contract reaches from its declared entry, with `open` open.
///
/// A plain BFS over the declared graph: `vision` edges are not traversal, a
/// `drop` goes forward only, an `exterior` endpoint is not a node (a piece's
/// outside is the face contract's business), and an edge carrying a `way` exists
/// only while that way is open.
fn reaches(contract: &SpatialContract, open: &BTreeSet<&str>) -> BTreeSet<String> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut queue: VecDeque<String> = VecDeque::new();
    seen.insert(contract.entry.clone());
    queue.push_back(contract.entry.clone());
    while let Some(here) = queue.pop_front() {
        for edge in &contract.edges {
            if edge.class == "vision" {
                continue;
            }
            if let Some(way) = &edge.way
                && !open.contains(way.region.as_str())
            {
                continue;
            }
            let step = if edge.a == here {
                Some(&edge.b)
            } else if edge.b == here && edge.class != "drop" {
                Some(&edge.a)
            } else {
                None
            };
            let Some(next) = step else { continue };
            if next == EXTERIOR || seen.contains(next) {
                continue;
            }
            seen.insert(next.clone());
            queue.push_back(next.clone());
        }
    }
    seen
}

/// The placed piece whose world AABB holds `pos`, with the piece-local cell —
/// `None` for a point outside every placed piece of that area, or in a piece
/// that declares no contract.
fn piece_holding<'a>(
    areas: &[AreaPlacement],
    prefabs: &'a PrefabRegistry,
    area_id: &str,
    pos: [i32; 3],
) -> Option<(usize, String, &'a SpatialContract, [i32; 3])> {
    let area = areas.iter().find(|a| a.area_id == area_id)?;
    for (index, placement) in area.pieces.iter().enumerate() {
        let (min, max) = placement.bbox();
        if (0..3).any(|a| pos[a] < min[a] || pos[a] > max[a]) {
            continue;
        }
        let contract = prefabs
            .get(&placement.prefab_id)
            .and_then(|m| m.spatial_contract.as_ref())?;
        let local = local_cell(placement.rotation, placement.pos, pos);
        return Some((index, placement.prefab_id.clone(), contract, local));
    }
    None
}

/// The declared space holding a piece-local cell, if any.
fn space_holding(contract: &SpatialContract, local: [i32; 3]) -> Option<String> {
    contract
        .spaces
        .iter()
        .find(|(_, space)| {
            space.boxes.iter().any(|b| {
                (0..3).all(|a| {
                    let lo = b.from[a].min(b.to[a]);
                    let hi = b.from[a].max(b.to[a]);
                    lo <= local[a] && local[a] <= hi
                })
            })
        })
        .map(|(name, _)| name.clone())
}

/// A local cell, placed and rotated — the transform `crate::faces` and
/// `crate::solver` both use, so a way's cells land on the piece's blocks.
fn world_cell(rotation: Rotation, pos: [i32; 3], local: [i32; 3]) -> [i32; 3] {
    let t = rotation.transform(local);
    [pos[0] + t[0], pos[1] + t[1], pos[2] + t[2]]
}

/// The inverse of [`world_cell`]: a world cell back into the piece's own frame.
fn local_cell(rotation: Rotation, pos: [i32; 3], world: [i32; 3]) -> [i32; 3] {
    let d = [world[0] - pos[0], world[1] - pos[1], world[2] - pos[2]];
    let back = match rotation {
        Rotation::None => Rotation::None,
        Rotation::Cw90 => Rotation::Ccw90,
        Rotation::Cw180 => Rotation::Cw180,
        Rotation::Ccw90 => Rotation::Cw90,
    };
    back.transform(d)
}

/// The advisory a campaign that stages ways but examines no required element
/// owes its reader: the reachability half of this gate matched nothing.
///
/// Never a refusal — content behind no way at all is ordinary, and a delve whose
/// ways are pure scenery is a delve. What is not acceptable is for that to be
/// indistinguishable from a proof.
pub fn unbound_finding(gate: &WayGate) -> Option<Diagnostic> {
    if gate.staged == 0 || gate.elements_examined > 0 {
        return None;
    }
    Some(Diagnostic::warning(
        DW_WAY_UNBOUND,
        "quests",
        "/content/quests",
        format!(
            "the way-reachability check examined ZERO required elements: {} way(s) are staged \
             across {} piece(s), and no objective anchor, body or campaign reference resolves \
             into a declared space of any of them. The dispositions below are still reported, \
             but nothing here proves an opening is needed for anything",
            gate.staged, gate.pieces_with_ways
        ),
    ))
}
