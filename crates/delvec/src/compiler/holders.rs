//! **The holder census** (spec-0105, spec-0111): every item holder of the
//! built world — a container block or a holder entity a piece carries — is
//! loot, a collect, a device or unclaimed, and the build proves which.
//!
//! The engine fills the holders a `loot` entry or a `collect` names and knew
//! nothing about the rest, so a campaign whose every barrel was empty shipped
//! without a word. This module reads the **edited assembled model** — the same
//! cells every later proof reads — collects every cell whose block is one of the
//! twelve container kinds ([`delvewright_dsl::blocks::CONTAINER_KINDS_1_21_11`])
//! and every holder entity the placed pieces carry
//! ([`delvewright_dsl::holders::ENTITY_HOLDERS_1_21_11`], [`placed_holders`]),
//! and resolves the claims the campaign's declarations make on those cells:
//!
//! | claim | from | cell |
//! |---|---|---|
//! | `loot` | a `loot[]` entry | its anchor's cell |
//! | `collect` | a `collect` adopting `holder`, or one with neither `holder` nor `dropped_by` (the compiler's own chest) | the adopted cell; the `anchor` cell, counted from the declaration because the model holds air there until runtime |
//! | `device` | a `trapped-chest` trap, or any trap's dispenser; a `use` / `strike` trigger or an `interact` objective whose cell holds a container or whose `prop` is one | the `at` / `anchor` cell |
//! | `decoration` | nothing, on a container | the remainder |
//! | `empty` (frozen) | nothing, on a frame or a stand | the remainder |
//!
//! Five verdicts are judged on the result: `DW1018` (the containers built and
//! the containers declared disagree — scenery with no notice, or a notice over
//! a world with no scenery), `DW1024` (scenery arranged as a storeroom, or a
//! holder that matters standing among scenery, spec-0105 §9), `DW1037` (a
//! holder the party cannot take from: unsupported or unreachable, spec-0111
//! §6.3) and `DW1038` (a runtime edit reaches a holder, spec-0111 §6.4). An
//! empty frame or stand is frozen scenery that costs a player nothing to
//! learn, so it is neither a decoration cell for `DW1018` nor one for §9.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use crate::compiler::assembled::base_id;
use crate::compiler::blockstate::BlockMap;
use crate::compiler::failure::Failure;
use crate::compiler::plan::{Plan, ResolvedAnchor};
use crate::schem::nbt::Nbt;
use delvewright_dsl::holders::{EntityHolderKind, HolderFamily};
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

delvewright_dsl::dw_code! {
    /// (spec-0111 §6.3) A holder the party cannot take from: **unsupported** —
    /// a frame whose backing block is not solid, a stand whose foot cell is not
    /// standable — or **unreachable** — a filled holder no standable,
    /// walk-reachable cell sees within `INTERACTION_REACH`.
    pub const DW_HOLDER_UNTAKEABLE: DwCode = DwCode::new("DW1037", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// (spec-0111 §6.4) A runtime `set-block`, `fill-region` or `clear-region`
    /// reaches a holder's cell or the cell it stands on or hangs from: the edit
    /// would pop the frame or drop the stand mid-game, silently.
    pub const DW_HOLDER_EDITED: DwCode = DwCode::new("DW1038", ExitTier::Build);
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
    /// claim on a holder the world already has.
    pub placed: Option<String>,
    /// The slots the claim fills, in the holder family's vocabulary (empty for
    /// a device, which fills nothing).
    pub slots: Vec<String>,
}

/// The brewing stand's block id: the one container whose contents partly show.
pub const BREWING_STAND: &str = "minecraft:brewing_stand";

/// The brewing stand's bottle slots, the ones its block state draws
/// (`has_bottle_0`–`has_bottle_2`, `BrewingStandBlock`): read from the pinned
/// game, the same jar and mappings as the container table.
pub const BREWING_BOTTLE_SLOTS: [&str; 3] = ["container.0", "container.1", "container.2"];

/// The census kind token of a container row.
pub const CONTAINER_KIND: &str = "container";

/// One holder of the built world: a container cell or a holder entity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HolderRow {
    /// The cell (a frame's own cell, a stand's foot cell).
    pub cell: [i32; 3],
    /// The census kind: `container`, `item-frame`, `glow-item-frame` or
    /// `armor-stand`.
    pub kind: &'static str,
    /// The container's base block id, or the holder entity's type id.
    pub block: String,
    /// The holder's slot vocabulary, in its family's order.
    pub slots: Vec<String>,
    /// The slots a claim fills, sorted.
    pub filled: Vec<String>,
    /// The place whose piece holds it ([`OUTSIDE_EVERY_PLACE`] when none).
    pub place: String,
    /// What claims it, in kind then declaration order. Empty = decoration (a
    /// container) or empty and frozen (a frame or a stand).
    pub claims: Vec<(ClaimKind, String)>,
}

impl HolderRow {
    /// Whether this row is a container cell.
    #[must_use]
    pub fn is_container(&self) -> bool {
        self.kind == CONTAINER_KIND
    }

    /// Whether no declaration claims this container — scenery `DW1018` and §9
    /// count. An unclaimed frame or stand is [`HolderRow::is_frozen`], never
    /// decoration, and no holder whose contents show is
    /// ([`HolderRow::is_exempt`]).
    #[must_use]
    pub fn is_decoration(&self) -> bool {
        self.is_container() && self.claims.is_empty() && !self.is_exempt()
    }

    /// **Whether the holder's contents show from outside** (spec-0105 /
    /// spec-0111 amendment): a frame, a stand, or a brewing stand whose bottle
    /// slots are all that is in question — unclaimed, or filled by `loot` /
    /// `collect` in its bottle slots (`container.0`–`container.2`, drawn as
    /// `has_bottle_N`) and nowhere else. A player sees what it holds without
    /// opening it, so it is exempt from the empty and scenery rules: no
    /// decoration cell, no place in the cap, free to stand among holders that
    /// matter, and owed no notice. A brewing stand with a device claim, or any
    /// fill in its ingredient or fuel slot, is an ordinary container.
    #[must_use]
    pub fn is_exempt(&self) -> bool {
        if !self.is_container() {
            return true;
        }
        if self.block != BREWING_STAND {
            return false;
        }
        if self.claims.is_empty() {
            return true;
        }
        self.claims
            .iter()
            .all(|(k, _)| matches!(k, ClaimKind::Loot | ClaimKind::Collect))
            && !self.filled.is_empty()
            && self
                .filled
                .iter()
                .all(|s| BREWING_BOTTLE_SLOTS.contains(&s.as_str()))
    }

    /// Whether this is an entity holder no fill claims: the engine summons it
    /// frozen (spec-0111 §4.4), so it does nothing and costs nothing to learn.
    #[must_use]
    pub fn is_frozen(&self) -> bool {
        !self.is_container() && self.claims.is_empty()
    }

    fn has(&self, kind: ClaimKind) -> bool {
        self.claims.iter().any(|(k, _)| *k == kind)
    }
}

/// The census of one build.
#[derive(Debug, Clone, PartialEq)]
pub struct Census {
    /// Every holder, in cell order (a container before an entity at one cell).
    pub rows: Vec<HolderRow>,
    /// The declared notice, when there is one.
    pub notice: Option<Narration>,
    /// SHA-256 of the census input — every cell and state of the model it
    /// read, then every holder entity it read.
    pub input_digest: String,
    /// The places still standing in their stand-in, in plan order: massing
    /// nobody furnished, whose containers this build cannot see yet. While any
    /// remains, a notice over a census with no scenery is owed to the build
    /// that details them — the same deferral the container fills take
    /// ([`crate::compiler::loot::split_owed_by_detail`]).
    pub stand_ins: Vec<String>,
    /// The stance pass's verdict counts (spec-0111 §5.2), once it has run:
    /// `(unsupported, unreachable)` over the holders it judged.
    pub stance: Option<(usize, usize)>,
}

/// What one place holds that breaks §9.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceFinding {
    /// The place.
    pub place: String,
    /// Its decoration cells.
    pub decoration: Vec<HolderRow>,
    /// Its claimed holders (the mixed arm).
    pub matter: Vec<HolderRow>,
}

fn cell_str(c: [i32; 3]) -> String {
    format!("[{}, {}, {}]", c[0], c[1], c[2])
}

/// The census input's digest: every `(cell, state)` of the model, in its order,
/// then every holder entity (`kind cell`), in theirs.
#[must_use]
pub fn model_digest(blocks: &BlockMap, entities: &[PlacedHolder]) -> String {
    let mut h = Sha256::new();
    for (c, s) in blocks {
        h.update(format!("{} {} {} {}\n", c[0], c[1], c[2], s.as_str()).as_bytes());
    }
    for e in entities {
        h.update(
            format!(
                "entity {} {} {} {}\n",
                e.kind.id, e.cell[0], e.cell[1], e.cell[2]
            )
            .as_bytes(),
        );
    }
    format!("{:x}", h.finalize())
}

/// The slot vocabulary of a container of `slots` slots.
fn container_vocabulary(slots: usize) -> Vec<String> {
    (0..slots).map(|n| format!("container.{n}")).collect()
}

/// The slot vocabulary of an entity holder kind.
#[must_use]
pub fn entity_vocabulary(kind: &delvewright_dsl::holders::EntityHolderKind) -> Vec<String> {
    match kind.family {
        HolderFamily::Frame => vec![delvewright_dsl::holders::FRAME_SLOT.to_string()],
        HolderFamily::Stand => STAND_VOCABULARY.iter().map(|s| (*s).to_string()).collect(),
        HolderFamily::Container => Vec::new(),
    }
}

/// The stand's vocabulary in spec-0111 §3.3's order — the order a reader
/// meets it in.
pub const STAND_VOCABULARY: [&str; 6] = [
    "armor.head",
    "armor.chest",
    "armor.legs",
    "armor.feet",
    "weapon.mainhand",
    "weapon.offhand",
];

/// **Take the census**: every container cell of `blocks`, every holder entity
/// the pieces carry, every runtime container a claim places, each attributed
/// to its place and its claims.
pub fn census(
    blocks: &BlockMap,
    entities: &[PlacedHolder],
    claims: &[Claim],
    place_of: &dyn Fn([i32; 3]) -> Option<String>,
    notice: Option<&Narration>,
) -> Census {
    // Keyed by (cell, 0 for a container / 1+i for the i-th entity), so a
    // container and an entity at one cell stay two rows in a fixed order.
    let mut rows: BTreeMap<([i32; 3], usize), HolderRow> = BTreeMap::new();
    let place = |cell: [i32; 3]| place_of(cell).unwrap_or_else(|| OUTSIDE_EVERY_PLACE.to_string());
    let container_row = |cell: [i32; 3], block: &str, slots: usize| HolderRow {
        cell,
        kind: CONTAINER_KIND,
        block: base_id(block).to_string(),
        slots: container_vocabulary(slots),
        filled: Vec::new(),
        place: place(cell),
        claims: Vec::new(),
    };
    for (cell, state) in blocks {
        if let Some(slots) = delvewright_dsl::blocks::container_slots(state.as_str()) {
            rows.insert((*cell, 0), container_row(*cell, state.as_str(), slots));
        }
    }
    for (i, e) in entities.iter().enumerate() {
        rows.insert(
            (e.cell, 1 + i),
            HolderRow {
                cell: e.cell,
                kind: e.kind.token,
                block: e.kind.id.to_string(),
                slots: entity_vocabulary(e.kind),
                filled: Vec::new(),
                place: place(e.cell),
                claims: Vec::new(),
            },
        );
    }
    for c in claims {
        // The container at the cell first, else the cell's one entity holder.
        let key = if rows.contains_key(&(c.cell, 0)) {
            Some((c.cell, 0))
        } else {
            let at: Vec<([i32; 3], usize)> = rows
                .range((c.cell, 1)..=(c.cell, usize::MAX))
                .map(|(k, _)| *k)
                .collect();
            (at.len() == 1).then(|| at[0])
        };
        let r = match key {
            Some(k) => rows.get_mut(&k).expect("key read from the map"),
            None => {
                let Some(block) = &c.placed else { continue };
                let Some(slots) = delvewright_dsl::blocks::container_slots(block) else {
                    continue;
                };
                rows.entry((c.cell, 0))
                    .or_insert_with(|| container_row(c.cell, block, slots))
            }
        };
        r.claims.push((c.kind, c.id.clone()));
        r.filled.extend(c.slots.iter().cloned());
    }
    let mut rows: Vec<HolderRow> = rows.into_values().collect();
    for r in &mut rows {
        r.claims.sort();
        r.claims.dedup();
        r.filled.sort();
        r.filled.dedup();
    }
    Census {
        rows,
        notice: notice.cloned(),
        input_digest: model_digest(blocks, entities),
        stand_ins: Vec::new(),
        stance: None,
    }
}

impl Census {
    /// Holders carrying a claim of `kind`.
    #[must_use]
    pub fn count(&self, kind: ClaimKind) -> usize {
        self.rows.iter().filter(|r| r.has(kind)).count()
    }

    /// Decoration cells: containers nothing claims.
    #[must_use]
    pub fn decoration(&self) -> usize {
        self.rows.iter().filter(|r| r.is_decoration()).count()
    }

    /// Empty holders: frames and stands nothing claims, frozen.
    #[must_use]
    pub fn empty(&self) -> usize {
        self.rows.iter().filter(|r| r.is_frozen()).count()
    }

    /// Holders whose contents show from outside ([`HolderRow::is_exempt`]).
    #[must_use]
    pub fn exempt(&self) -> usize {
        self.rows.iter().filter(|r| r.is_exempt()).count()
    }

    /// Container cells (what `DW1018` and §9 range over).
    #[must_use]
    pub fn containers(&self) -> usize {
        self.rows.iter().filter(|r| r.is_container()).count()
    }

    fn by<F: Fn(&HolderRow) -> String>(&self, f: F) -> BTreeMap<String, usize> {
        let mut m = BTreeMap::new();
        for r in &self.rows {
            *m.entry(f(r)).or_insert(0) += 1;
        }
        m
    }

    /// Record the stance pass's verdict counts for the binding line.
    #[must_use]
    pub fn with_stance(mut self, unsupported: usize, unreachable: usize) -> Self {
        self.stance = Some((unsupported, unreachable));
        self
    }

    /// The places breaking §9, mixed arm then storeroom arm, each in place order.
    #[must_use]
    pub fn arrangement(&self) -> (Vec<PlaceFinding>, Vec<PlaceFinding>) {
        let mut places: BTreeMap<&str, (Vec<HolderRow>, Vec<HolderRow>)> = BTreeMap::new();
        for r in &self.rows {
            // A holder whose contents show is neither scenery to search nor a
            // holder hidden among it: a player sees what it holds.
            if r.is_exempt() {
                continue;
            }
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

    /// **Owe the places still standing in their stand-in to their detail**:
    /// `places` is [`crate::compiler::blockout::Blockout::massed`].
    #[must_use]
    pub fn deferring(mut self, places: &[String]) -> Self {
        self.stand_ins = places.to_vec();
        self
    }

    /// Whether the notice's second arm — a notice with no scenery — is owed to
    /// a later detail rather than judged now.
    #[must_use]
    pub fn notice_deferred(&self) -> bool {
        self.notice.is_some() && self.decoration() == 0 && !self.stand_ins.is_empty()
    }

    /// The binding line (spec-0105 §5.1, spec-0111 §5.1), for stderr on every
    /// build.
    #[must_use]
    pub fn line(&self) -> String {
        let join = |m: BTreeMap<String, usize>| {
            m.iter()
                .map(|(k, n)| format!("{k}: {n}"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let kinds = join(self.by(|r| r.kind.to_string()));
        let places = join(self.by(|r| r.place.clone()));
        let (mixed, over) = self.arrangement();
        let deferred = if self.stand_ins.is_empty() {
            String::new()
        } else {
            format!(
                " {k} place(s) still stand in their stand-in, whose containers this build cannot \
                 see ({names}); {verdict}.",
                k = self.stand_ins.len(),
                names = self.stand_ins.join(", "),
                verdict = if self.notice_deferred() {
                    "the notice speaks for no scenery here, and is judged (DW1018) by the build \
                     that details them"
                } else {
                    "a notice with no scenery would be judged by the build that details them"
                },
            )
        };
        let stance = match self.stance {
            Some((u, r)) => format!(
                " stance: {} filled holder(s) and {} holder(s) judged, {u} unsupported, {r} \
                 unreachable.",
                self.rows
                    .iter()
                    .filter(|r| !r.is_container() && !r.filled.is_empty())
                    .count(),
                self.rows.iter().filter(|r| !r.is_container()).count(),
            ),
            None => String::new(),
        };
        format!(
            "holder census: {n} holder(s) in the assembled world{kinds}; {l} loot, {c} collect, \
             {v} device, {d} decoration, {e} empty (frozen), {x} exempt (contents show from \
             outside); notice {notice}; by place: \
             {places}; arrangement: {m} mixed place(s), {o} over the cap of {cap} decoration \
             cell(s) per place.{stance}{deferred}",
            n = self.rows.len(),
            kinds = if kinds.is_empty() {
                String::new()
            } else {
                format!(" ({kinds})")
            },
            l = self.count(ClaimKind::Loot),
            c = self.count(ClaimKind::Collect),
            v = self.count(ClaimKind::Device),
            d = self.decoration(),
            e = self.empty(),
            x = self.exempt(),
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
                        r.slots.len()
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
                    n = self.containers(),
                ),
            });
        }
        if d == 0 && self.notice.is_some() && !self.notice_deferred() {
            return Err(Failure {
                code: DW_CONTAINERS_UNDECLARED,
                message: format!(
                    "`containers.decoration_notice` is declared, but every one of the {n} \
                     container cell(s) in the assembled world is claimed — loot, a collect or \
                     a device — so the notice would tell the player something false about the \
                     world: there is no scenery container for it to speak for. Delete \
                     `containers.decoration_notice`.",
                    n = self.containers()
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
        let cells = |rs: &[HolderRow]| {
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

    /// The ledger (spec-0105 §4.3, spec-0111 §4.4): `validation/holders.json`.
    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        use serde_json::json;
        let (mixed, over) = self.arrangement();
        let names = |fs: &[PlaceFinding]| fs.iter().map(|f| f.place.clone()).collect::<Vec<_>>();
        json!({
            "census_input_sha256": self.input_digest,
            "holders": self.rows.len(),
            "cells": self.containers(),
            "loot": self.count(ClaimKind::Loot),
            "collect": self.count(ClaimKind::Collect),
            "device": self.count(ClaimKind::Device),
            "decoration": self.decoration(),
            "empty": self.empty(),
            "exempt": self.exempt(),
            "by_kind": self.by(|r| r.kind.to_string()),
            "by_block": self.by(|r| r.block.clone()),
            "by_place": self.by(|r| r.place.clone()),
            "max_scenery_per_place": MAX_SCENERY_PER_PLACE,
            "mixed_places": names(&mixed),
            "over_cap_places": names(&over),
            "stand_ins": self.stand_ins,
            "stance": self.stance.map(|(u, r)| json!({"unsupported": u, "unreachable": r})),
            "notice": self.notice.as_ref().map(|n| json!({
                "text": delvewright_dsl::l10n::plain(&n.text),
                "style": n.style.unwrap_or(delvewright_dsl::NarrateStyle::Chat).token(),
                "sound": n.sound.as_deref().map(|s| if s.contains(':') {
                    s.to_string()
                } else {
                    format!("minecraft:{s}")
                }),
            })),
            "rows": self.rows.iter().map(|r| json!({
                "cell": r.cell,
                "kind": r.kind,
                "block": r.block,
                "slots": r.slots.iter().map(|s| json!({"slot": s, "filled": r.filled.contains(s)})).collect::<Vec<_>>(),
                "frozen": r.is_frozen(),
                "exempt": r.is_exempt(),
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
pub fn claims_of(plan: &Plan, blocks: &BlockMap, entities: &[PlacedHolder]) -> Vec<Claim> {
    let holds = |cell: [i32; 3]| {
        blocks
            .get(&cell)
            .is_some_and(|s| delvewright_dsl::blocks::container_kind(s.as_str()).is_some())
    };
    let mut out = Vec::new();
    for l in &plan.loot {
        let family = family_at(blocks, entities, l.cell);
        out.push(Claim {
            kind: ClaimKind::Loot,
            id: l.id.clone(),
            cell: l.cell,
            placed: None,
            slots: family.map_or_else(Vec::new, |(f, _)| loot_slots(f, &l.items)),
        });
    }
    for f in &plan.collect_fills {
        let family = family_at(blocks, entities, f.cell);
        out.push(Claim {
            kind: ClaimKind::Collect,
            id: f.objective_id.clone(),
            cell: f.cell,
            placed: None,
            slots: family.map_or_else(Vec::new, |(fam, _)| collect_slots(fam, f)),
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
                    holder: None,
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
                            slots: vec!["container.0".to_string()],
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
                            slots: Vec::new(),
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
                slots: Vec::new(),
            });
        }
        // A trap's dispenser is a container (spec-0105 §2.1) and the trap's
        // own hardware: what it holds is the trap's payload.
        if let Some(cell) = t.dispenser {
            out.push(Claim {
                kind: ClaimKind::Device,
                id: t.id.clone(),
                cell,
                placed: None,
                slots: Vec::new(),
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
                slots: Vec::new(),
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

// ---------------------------------------------------------------------------
// The holder entities a piece carries (spec-0111 §3.1)
// ---------------------------------------------------------------------------

/// A frame's facing, in the game's `Facing` byte (`Direction.get3DDataValue`):
/// down 0, up 1, north 2, south 3, west 4, east 5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FrameFacing {
    /// Hung under a ceiling, facing down.
    Down,
    /// Laid on a floor, facing up.
    Up,
    /// Facing −z.
    North,
    /// Facing +z.
    South,
    /// Facing −x.
    West,
    /// Facing +x.
    East,
}

impl FrameFacing {
    /// From the `Facing` byte.
    #[must_use]
    pub fn from_byte(b: i64) -> Option<FrameFacing> {
        Some(match b {
            0 => FrameFacing::Down,
            1 => FrameFacing::Up,
            2 => FrameFacing::North,
            3 => FrameFacing::South,
            4 => FrameFacing::West,
            5 => FrameFacing::East,
            _ => return None,
        })
    }

    /// The `Facing` byte.
    #[must_use]
    pub fn byte(self) -> i8 {
        match self {
            FrameFacing::Down => 0,
            FrameFacing::Up => 1,
            FrameFacing::North => 2,
            FrameFacing::South => 3,
            FrameFacing::West => 4,
            FrameFacing::East => 5,
        }
    }

    /// The unit direction the frame faces.
    #[must_use]
    pub fn unit(self) -> [i32; 3] {
        match self {
            FrameFacing::Down => [0, -1, 0],
            FrameFacing::Up => [0, 1, 0],
            FrameFacing::North => [0, 0, -1],
            FrameFacing::South => [0, 0, 1],
            FrameFacing::West => [-1, 0, 0],
            FrameFacing::East => [1, 0, 0],
        }
    }

    /// The facing a cardinal keyword names (`north`, …, `up`, `down`).
    #[must_use]
    pub fn parse(s: &str) -> Option<FrameFacing> {
        Some(match s {
            "down" => FrameFacing::Down,
            "up" => FrameFacing::Up,
            "north" => FrameFacing::North,
            "south" => FrameFacing::South,
            "west" => FrameFacing::West,
            "east" => FrameFacing::East,
            _ => return None,
        })
    }

    /// This facing under a piece's yaw rotation; up and down are unmoved.
    #[must_use]
    pub fn rotate(self, r: crate::compiler::solver::Rotation) -> FrameFacing {
        use crate::compiler::solver::Facing;
        let h = match self {
            FrameFacing::North => Facing::North,
            FrameFacing::South => Facing::South,
            FrameFacing::West => Facing::West,
            FrameFacing::East => Facing::East,
            other => return other,
        };
        match h.rotate(r) {
            Facing::North => FrameFacing::North,
            Facing::South => FrameFacing::South,
            Facing::West => FrameFacing::West,
            Facing::East => FrameFacing::East,
        }
    }
}

/// **One holder entity a placed piece carries**, in world space.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedHolder {
    /// The holder kind.
    pub kind: &'static EntityHolderKind,
    /// The cell it occupies: a frame's own cell (its `blockPos`), a stand's
    /// foot cell.
    pub cell: [i32; 3],
    /// A frame's facing, rotated with the piece (`None` on a stand).
    pub facing: Option<FrameFacing>,
    /// A stand's yaw in degrees, rotated with the piece (`0` on a frame).
    pub yaw: f32,
    /// The scene properties the piece wrote (spec-0111 §3.1), the frame's
    /// `Facing` and the stand's `Rotation` excepted — those two are carried
    /// rotated above. In key order.
    pub scene: BTreeMap<String, Nbt>,
    /// The prefab that carries it.
    pub prefab: String,
}

impl PlacedHolder {
    /// Whether the stand is `Small` (its bands and body at half scale).
    #[must_use]
    pub fn small(&self) -> bool {
        matches!(self.scene.get("Small"), Some(Nbt::Byte(b)) if *b != 0)
    }

    /// The block a frame hangs on, or the block a stand stands on — the cell a
    /// runtime edit may not reach (`DW1038`) and the stance pass proves solid.
    #[must_use]
    pub fn support(&self) -> [i32; 3] {
        let c = self.cell;
        match self.facing {
            Some(f) => {
                let u = f.unit();
                [c[0] - u[0], c[1] - u[1], c[2] - u[2]]
            }
            None => [c[0], c[1] - 1, c[2]],
        }
    }

    /// The body's box in world coordinates (spec-0111 §5.2): a frame's
    /// 12/16 × 12/16 × 1/16 plate against its support, a stand's 0.5 × 1.975
    /// column (half that when `Small`).
    #[must_use]
    pub fn body_box(&self) -> crate::compiler::nav::BodyBox {
        let [x, y, z] = self.cell.map(f64::from);
        match self.facing {
            Some(f) => {
                let (lo2, hi2, t) = (2.0 / 16.0, 14.0 / 16.0, 1.0 / 16.0);
                let (lo, hi) = match f {
                    FrameFacing::North => {
                        ([x + lo2, y + lo2, z + 1.0 - t], [x + hi2, y + hi2, z + 1.0])
                    }
                    FrameFacing::South => ([x + lo2, y + lo2, z], [x + hi2, y + hi2, z + t]),
                    FrameFacing::West => {
                        ([x + 1.0 - t, y + lo2, z + lo2], [x + 1.0, y + hi2, z + hi2])
                    }
                    FrameFacing::East => ([x, y + lo2, z + lo2], [x + t, y + hi2, z + hi2]),
                    FrameFacing::Up => ([x + lo2, y, z + lo2], [x + hi2, y + t, z + hi2]),
                    FrameFacing::Down => {
                        ([x + lo2, y + 1.0 - t, z + lo2], [x + hi2, y + 1.0, z + hi2])
                    }
                };
                crate::compiler::nav::BodyBox { lo, hi }
            }
            None => {
                let s = if self.small() { 0.5 } else { 1.0 };
                let half = self.kind.size[0] * s / 2.0;
                crate::compiler::nav::BodyBox {
                    lo: [x + 0.5 - half, y, z + 0.5 - half],
                    hi: [x + 0.5 + half, y + self.kind.size[1] * s, z + 0.5 + half],
                }
            }
        }
    }
}

/// The raw `entities` list of one template's bytes, each as [`Nbt`].
#[must_use]
pub fn template_entities(bytes: &[u8]) -> Vec<Nbt> {
    let mut raw = Vec::new();
    if std::io::Read::read_to_end(&mut flate2::read::GzDecoder::new(bytes), &mut raw).is_err() {
        return Vec::new();
    }
    crate::compiler::nbtread::entities(&raw)
        .into_iter()
        .map(Nbt::from)
        .collect()
}

fn byte_of(v: &Nbt) -> Option<i64> {
    match v {
        Nbt::Byte(b) => Some(i64::from(*b)),
        Nbt::Short(b) => Some(i64::from(*b)),
        Nbt::Int(b) => Some(i64::from(*b)),
        Nbt::Long(b) => Some(*b),
        _ => None,
    }
}

fn float_of(v: &Nbt) -> Option<f32> {
    match v {
        Nbt::Float(f) => Some(*f),
        Nbt::Double(f) => Some(*f as f32),
        _ => None,
    }
}

/// **Every holder entity the placed pieces carry**, in world space, in area,
/// piece, template, then file order (ADR-0006). An entity of a kind that is
/// not a holder is not one: admission refuses it (`DW0730`), and the build
/// ships no template entity at all.
#[must_use]
pub fn placed_holders(plan: &Plan, structures: &BTreeMap<String, Vec<u8>>) -> Vec<PlacedHolder> {
    let mut out = Vec::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for template in &piece.templates {
                let Some(bytes) = structures.get(&template.structure_file) else {
                    continue;
                };
                for e in template_entities(bytes) {
                    let Some(id) = crate::admit::entities::entity_id(&e) else {
                        continue;
                    };
                    let Some(kind) = delvewright_dsl::holders::entity_holder(id) else {
                        continue;
                    };
                    let Some(local) = crate::admit::entities::entity_cell(&e) else {
                        continue;
                    };
                    let t = piece.rotation.transform(local);
                    let cell = [
                        template.pos[0] + t[0],
                        template.pos[1] + t[1],
                        template.pos[2] + t[2],
                    ];
                    let nbt = e
                        .as_compound()
                        .and_then(|c| c.get("nbt"))
                        .and_then(Nbt::as_compound)
                        .cloned()
                        .unwrap_or_default();
                    let mut scene: BTreeMap<String, Nbt> = nbt
                        .iter()
                        .filter(|(k, _)| kind.scene_keys.contains(&k.as_str()))
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect();
                    let (facing, yaw) = match kind.family {
                        HolderFamily::Frame => {
                            let f = scene
                                .remove("Facing")
                                .and_then(|v| byte_of(&v))
                                .and_then(FrameFacing::from_byte)
                                .unwrap_or(FrameFacing::South);
                            (Some(f.rotate(piece.rotation)), 0.0)
                        }
                        _ => {
                            let yaw = scene
                                .remove("Rotation")
                                .and_then(|v| {
                                    v.as_list().and_then(|l| l.first().and_then(float_of))
                                })
                                .unwrap_or(0.0);
                            let turn = match piece.rotation {
                                crate::compiler::solver::Rotation::None => 0.0,
                                crate::compiler::solver::Rotation::Cw90 => 90.0,
                                crate::compiler::solver::Rotation::Cw180 => 180.0,
                                crate::compiler::solver::Rotation::Ccw90 => 270.0,
                            };
                            (None, (yaw + turn).rem_euclid(360.0))
                        }
                    };
                    out.push(PlacedHolder {
                        kind,
                        cell,
                        facing,
                        yaw,
                        scene,
                        prefab: piece.prefab_id.clone(),
                    });
                }
            }
        }
    }
    out
}

/// The family of the holder at `cell`, with its container slot count (0 for
/// an entity): the container block standing there, else the cell's one holder
/// entity. `None` for a cell holding neither, or two holder entities.
#[must_use]
pub fn family_at(
    blocks: &BlockMap,
    entities: &[PlacedHolder],
    cell: [i32; 3],
) -> Option<(HolderFamily, usize)> {
    if let Some(slots) = blocks
        .get(&cell)
        .and_then(|s| delvewright_dsl::blocks::container_slots(s.as_str()))
    {
        return Some((HolderFamily::Container, slots));
    }
    let mut at = entities.iter().filter(|e| e.cell == cell);
    let one = at.next()?;
    if at.next().is_some() {
        return None;
    }
    Some((one.kind.family, 0))
}

/// The entity holders standing at `cell`.
#[must_use]
pub fn entities_at(entities: &[PlacedHolder], cell: [i32; 3]) -> Vec<&PlacedHolder> {
    entities.iter().filter(|e| e.cell == cell).collect()
}

/// **The slot each stack of a fill lands in** on a holder of `family` — the one
/// landing rule of the holder class (spec-0111 §3.3, §3.4), for a `loot` fill
/// and an adopting `collect` alike: a stack's named slot, else positional —
/// `container.<n>` on a container, `contents` on a frame. A stand's unnamed
/// stack lands nowhere (`DW1035`), so it maps to no slot here.
#[must_use]
pub fn landing_slots(family: HolderFamily, named: &[Option<&str>]) -> Vec<String> {
    named
        .iter()
        .enumerate()
        .filter_map(|(n, s)| match (s, family) {
            (Some(s), _) => Some((*s).to_string()),
            (None, HolderFamily::Container) => Some(format!("container.{n}")),
            (None, HolderFamily::Frame) => Some(delvewright_dsl::holders::FRAME_SLOT.to_string()),
            (None, HolderFamily::Stand) => None,
        })
        .collect()
}

/// [`landing_slots`] over a `loot` fill's stacks.
#[must_use]
pub fn loot_slots(
    family: HolderFamily,
    items: &[crate::compiler::plan::LootItemPlan],
) -> Vec<String> {
    let named: Vec<Option<&str>> = items.iter().map(|it| it.slot.as_deref()).collect();
    landing_slots(family, &named)
}

/// [`landing_slots`] over an adopting `collect`'s stacks (spec-0111 §3.4): the
/// objective's stack in the slot it names — positional when it names none —
/// and on a container its padding in the `container.<n>` slots after it.
#[must_use]
pub fn collect_slots(
    family: HolderFamily,
    fill: &crate::compiler::plan::CollectFillPlan,
) -> Vec<String> {
    let named = fill.stack_slots();
    let named: Vec<Option<&str>> = named.iter().map(Option::as_deref).collect();
    landing_slots(family, &named)
}

/// One filled slot of an entity holder: what lands there and who put it.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityFill {
    /// The slot.
    pub slot: String,
    /// The stack, for a `loot` fill (filled at `setup_finish`); `None` for a
    /// `collect`'s stack, which its activation fills.
    pub stack: Option<crate::compiler::plan::LootItemPlan>,
}

/// **An entity holder with what the engine owns about it**: its tag id, its
/// fills and who claims it — everything emission, the PackTest and the
/// critical path read, derived once.
#[derive(Debug, Clone, PartialEq)]
pub struct OwnedHolder {
    /// The placed holder.
    pub holder: PlacedHolder,
    /// The tag stem: `dw_holder_<id>` (the claiming anchor's stem, or the cell
    /// for a holder no fill claims).
    pub id: String,
    /// The anchor that names it, when a fill does.
    pub anchor: Option<String>,
    /// Its filled slots, in fill order: `loot` stacks, then a `collect`'s.
    pub fills: Vec<EntityFill>,
    /// The `loot` id that claims it.
    pub loot: Option<String>,
    /// The `collect` objective that claims it.
    pub collect: Option<String>,
}

impl OwnedHolder {
    /// The holder's own tag.
    #[must_use]
    pub fn tag(&self) -> String {
        format!("dw_holder_{}", self.id)
    }

    /// The selector that names the one body.
    #[must_use]
    pub fn selector(&self) -> String {
        format!("@e[tag={},limit=1]", self.tag())
    }

    /// Whether any fill claims it.
    #[must_use]
    pub fn is_filled(&self) -> bool {
        !self.fills.is_empty()
    }

    /// The slots a fill claims.
    #[must_use]
    pub fn claimed_slots(&self) -> Vec<&str> {
        self.fills.iter().map(|f| f.slot.as_str()).collect()
    }

    /// The stand's computed `DisabledSlots` (spec-0111 §4.1).
    #[must_use]
    pub fn disabled_slots(&self) -> i32 {
        delvewright_dsl::holders::disabled_slots(&self.claimed_slots())
    }

    /// Whether the stand must show its arms: the piece's choice, forced on
    /// when a hand slot is filled.
    #[must_use]
    pub fn show_arms(&self) -> bool {
        let piece = matches!(self.holder.scene.get("ShowArms"), Some(Nbt::Byte(b)) if *b != 0);
        piece
            || self
                .claimed_slots()
                .iter()
                .any(|s| s.starts_with("weapon."))
    }
}

/// **Own every holder entity**: its id, its fills and its claims, in the
/// order of [`placed_holders`]. A fill whose cell holds no single entity
/// holder claims none here (the fill proofs refuse it first).
#[must_use]
pub fn own_holders(plan: &Plan, entities: &[PlacedHolder]) -> Vec<OwnedHolder> {
    let mut out: Vec<OwnedHolder> = entities
        .iter()
        .map(|h| OwnedHolder {
            holder: h.clone(),
            id: format!("at_{}_{}_{}", h.cell[0], h.cell[1], h.cell[2]).replace('-', "m"),
            anchor: None,
            fills: Vec::new(),
            loot: None,
            collect: None,
        })
        .collect();
    let one_at = |cell: [i32; 3]| {
        let idx: Vec<usize> = entities
            .iter()
            .enumerate()
            .filter(|(_, e)| e.cell == cell)
            .map(|(i, _)| i)
            .collect();
        (idx.len() == 1).then(|| idx[0])
    };
    for l in &plan.loot {
        let Some(i) = one_at(l.cell) else { continue };
        let fam = out[i].holder.kind.family;
        let slots = loot_slots(fam, &l.items);
        let o = &mut out[i];
        o.id = crate::compiler::plan::safe_local(&l.anchor);
        o.anchor = Some(l.anchor.clone());
        o.loot = Some(l.id.clone());
        for (slot, it) in slots.into_iter().zip(
            l.items
                .iter()
                .filter(|it| it.slot.is_some() || fam != HolderFamily::Stand),
        ) {
            o.fills.push(EntityFill {
                slot,
                stack: Some(it.clone()),
            });
        }
    }
    for f in &plan.collect_fills {
        let Some(i) = one_at(f.cell) else { continue };
        let fam = out[i].holder.kind.family;
        let o = &mut out[i];
        o.id = crate::compiler::plan::safe_local(&f.anchor);
        o.anchor = Some(f.anchor.clone());
        o.collect = Some(f.objective_id.clone());
        // The objective's own stack: its slot, never its padding (a holder
        // entity has none to pad, `DW1035`).
        if let Some(slot) = collect_slots(fam, f).into_iter().next() {
            o.fills.push(EntityFill { slot, stack: None });
        }
    }
    out
}

// ---------------------------------------------------------------------------
// The stance (spec-0111 §5.2) and the pair (§6.4)
// ---------------------------------------------------------------------------

/// What the stance pass found for one holder.
#[derive(Debug, Clone, PartialEq)]
pub struct Stance {
    /// The holder's tag id.
    pub id: String,
    /// Standable, walk-reachable cells whose eye sees the holder's box within
    /// [`crate::compiler::crosshair::INTERACTION_REACH`], in cell order. Empty
    /// for a holder no fill claims (not searched).
    pub cells: Vec<[i32; 3]>,
}

/// What [`walk_world`] found: the world a route is walked in, and the stance
/// pass's own answer the build judges (`DW1037`).
pub struct Walked {
    /// `world` with each holder `collect`'s stance
    /// ([`crate::compiler::nav::World::with_holder_stances`]).
    pub world: crate::compiler::nav::World,
    /// Each holder's stance cells, in [`own_holders`] order.
    pub stances: Vec<Stance>,
    /// `(unsupported, unreachable)`.
    pub counts: (usize, usize),
    /// `DW1037`, or nothing.
    pub verdict: Result<(), Failure>,
}

/// **The world every critical-path route is walked in, derived once**
/// (spec-0111 §5.2): the build's final world
/// ([`crate::compiler::light::final_world`]: the assembled cells under every
/// premise the campaign states, relight fixtures standing in them), the stance
/// pass over it for the holders `owned`, and that world ending each holder
/// `collect`'s leg at the stance the pass chose. `delvec build` and every tool
/// that reads the critical path (`delvec snapshot --shot …`,
/// `delvec blocking-chart`) take their world here and nowhere else, so a leg
/// ends where the build's does. The caller reads `Relight::diagnostics` and
/// [`Walked::verdict`] — a refused pass is the build's failure to raise, not
/// this derivation's.
pub fn final_walk(
    plan: &Plan,
    assembled: &crate::compiler::assembled::Assembled,
    owned: &[OwnedHolder],
) -> (Walked, crate::compiler::light::Relight) {
    let (world, relight) = crate::compiler::light::final_world(plan, assembled);
    (walk_world(plan, world, owned), relight)
}

/// The stance pass over `world` for the holders `owned`, from the cells the
/// party can walk to, and `world` ending each holder `collect`'s leg at the
/// stance that pass chose. Private: every reader takes it through
/// [`final_walk`].
fn walk_world(plan: &Plan, world: crate::compiler::nav::World, owned: &[OwnedHolder]) -> Walked {
    let reachable = if owned.iter().any(|o| o.is_filled()) {
        world.reachable_walkable_rooted(&crate::compiler::edit::anchor_starts(plan))
    } else {
        std::collections::BTreeSet::new()
    };
    let (stances, counts, verdict) = check_stance(&world, owned, &reachable);
    let world = world.with_holder_stances(collect_stances(owned, &stances));
    Walked {
        world,
        stances,
        counts,
        verdict,
    }
}

/// **The stance pass** (spec-0111 §5.2): every holder is kept by the game —
/// a frame's support solid and its own cell clear, a stand's foot cell
/// standable — and every filled holder has a standable, walk-reachable cell
/// from whose eye its box is within `INTERACTION_REACH` along an unobstructed
/// segment, and in which a standing player's body shares no cell with any
/// holder entity's body ([`holder_free`]). Returns each holder's stance cells (in [`own_holders`] order) and
/// the `(unsupported, unreachable)` counts, or `DW1037` naming every holder
/// that fails and the remedies at the piece.
pub fn check_stance(
    world: &crate::compiler::nav::World,
    owned: &[OwnedHolder],
    reachable: &std::collections::BTreeSet<[i32; 3]>,
) -> (Vec<Stance>, (usize, usize), Result<(), Failure>) {
    let reach = crate::compiler::crosshair::INTERACTION_REACH;
    let occupied: std::collections::BTreeSet<[i32; 3]> = owned
        .iter()
        .flat_map(|o| o.holder.body_box().cells())
        .collect();
    let mut unsupported = Vec::new();
    let mut unreachable = Vec::new();
    let mut stances = Vec::new();
    for o in owned {
        let h = &o.holder;
        let support = h.support();
        let kept = match h.facing {
            Some(_) => world.solid_at(support) && !world.solid_at(h.cell),
            None => world.is_standable(h.cell),
        };
        if !kept {
            let what = match h.facing {
                Some(_) if world.solid_at(h.cell) => format!(
                    "its own cell {} is solid, so the frame's plate collides with the level and \
                     the game drops it within five seconds",
                    cell_str(h.cell)
                ),
                Some(_) => format!(
                    "the block behind it at {} is not solid, so the game drops the frame within \
                     five seconds (`ItemFrame.survives` runs every 100 ticks)",
                    cell_str(support)
                ),
                None => format!(
                    "its foot cell {} is not standable (solid ground under it at {}, two clear \
                     cells over it), so the stand drops or stands inside a block",
                    cell_str(h.cell),
                    cell_str(support)
                ),
            };
            unsupported.push(format!(
                "\n  UNSUPPORTED — the `{}` `{}` at {} in `{}`: {what}.",
                h.kind.id,
                o.id,
                cell_str(h.cell),
                h.prefab
            ));
        }
        let cells = if o.is_filled() {
            crate::compiler::nav::stance_cells(world, &h.body_box(), reach, reachable)
                .into_iter()
                .filter(|c| holder_free(world, *c, &occupied))
                .collect()
        } else {
            Vec::new()
        };
        if o.is_filled() && cells.is_empty() {
            let nearest = reachable
                .iter()
                .min_by_key(|c| {
                    (0..3)
                        .map(|i| i64::from(c[i] - h.cell[i]).pow(2))
                        .sum::<i64>()
                })
                .map_or_else(|| "none".to_string(), |c| cell_str(*c));
            unreachable.push(format!(
                "\n  UNREACHABLE — the `{}` `{}` at {} in `{}` holds {} and no standable cell the \
                 party can walk to sees its body within {reach} blocks (the entity reach) along \
                 a clear line; the nearest walkable cell is {nearest}.",
                h.kind.id,
                o.id,
                cell_str(h.cell),
                h.prefab,
                o.claimed_slots().join(", ")
            ));
        }
        stances.push(Stance {
            id: o.id.clone(),
            cells,
        });
    }
    let counts = (unsupported.len(), unreachable.len());
    if unsupported.is_empty() && unreachable.is_empty() {
        return (stances, counts, Ok(()));
    }
    let failure = Failure {
        code: DW_HOLDER_UNTAKEABLE,
        message: format!(
            "{} holder(s) unsupported and {} unreachable of the {} holder entit(y/ies) the pieces \
             carry: a player could not take what the scene holds.{}{}\n  Remedies, all at the \
             piece: move the holder; open the sightline to it; or stand it on, or hang it from, \
             the solid block it needs. A frame hangs from the block behind its face; a stand \
             stands on a solid block with two clear cells over it.",
            counts.0,
            counts.1,
            owned.len(),
            unsupported.concat(),
            unreachable.concat()
        ),
    };
    (stances, counts, Err(failure))
}

impl Stance {
    /// The stance cell nearest `cell` (squared distance, then cell order) —
    /// the one a holder `collect`'s leg ends at and its critical-path step
    /// names.
    #[must_use]
    pub fn nearest(&self, cell: [i32; 3]) -> Option<[i32; 3]> {
        self.cells
            .iter()
            .min_by_key(|s| {
                let d: i64 = (0..3).map(|i| i64::from(s[i] - cell[i]).pow(2)).sum();
                (d, **s)
            })
            .copied()
    }
}

/// **Where each holder `collect` is taken from** (spec-0111 §5.2): the holder's
/// cell paired with its [`Stance::nearest`] cell, for every holder a `collect`
/// claims and the stance pass found a stance for — what
/// [`crate::compiler::nav::World::with_holder_stances`] ends the leg at, and
/// what the critical-path step carries as `holder.stance`.
#[must_use]
pub fn collect_stances(owned: &[OwnedHolder], stances: &[Stance]) -> BTreeMap<[i32; 3], [i32; 3]> {
    owned
        .iter()
        .zip(stances)
        .filter(|(o, _)| o.collect.is_some())
        .filter_map(|(o, st)| st.nearest(o.holder.cell).map(|s| (o.holder.cell, s)))
        .collect()
}

/// Whether a player standing in `c` keeps its body out of every holder
/// entity's body cells `occupied` (spec-0111 §5.2). The block model reads a
/// holder as air, so a stance search over blocks alone finds the cell a stand
/// stands in as the stance nearest it: a stance inside the holder proves no
/// separate line of sight, and the route to it walks through the stand beside
/// it. The holder's own cells, and a neighbouring holder's, are no stance.
fn holder_free(
    world: &crate::compiler::nav::World,
    c: [i32; 3],
    occupied: &std::collections::BTreeSet<[i32; 3]>,
) -> bool {
    use delvewright_dsl::metrics::PLAYER_HEIGHT;
    let feet = world.feet_y(c);
    let lo = feet.floor() as i32;
    let hi = (feet + PLAYER_HEIGHT - 1e-9).floor() as i32;
    (lo..=hi).all(|y| !occupied.contains(&[c[0], y, c[2]]))
}

/// **`DW1038`** (spec-0111 §6.4): no runtime `set-block`, `fill-region` or
/// `clear-region` reaches a holder's cell or its support cell. Returns how
/// many edits were examined.
pub fn check_runtime_edits(plan: &Plan, owned: &[OwnedHolder]) -> Result<usize, Failure> {
    let mut examined = 0usize;
    let mut bad = Vec::new();
    crate::compiler::plan::for_each_gate_effect(plan.campaign, &mut |site, e| {
        let region = match &e.verb {
            delvewright_dsl::Verb::SetBlock { anchor, .. } => {
                plan.point_any(anchor.as_str()).map(|p| (p, p))
            }
            delvewright_dsl::Verb::FillRegion { region, .. }
            | delvewright_dsl::Verb::ClearRegion { region, .. } => plan.zone_box(region),
            _ => return,
        };
        let Some((a, b)) = region else { return };
        examined += 1;
        let lo = [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])];
        let hi = [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])];
        let inside = |c: [i32; 3]| (0..3).all(|i| lo[i] <= c[i] && c[i] <= hi[i]);
        for o in owned {
            let h = &o.holder;
            for (what, c) in [
                ("its own cell", h.cell),
                ("the block it needs", h.support()),
            ] {
                if inside(c) {
                    bad.push(format!(
                        "\n  `{}` at `{}` reaches the `{}` `{}` at {}: {what}, {}.",
                        e.verb.tag(),
                        site.path,
                        h.kind.id,
                        o.id,
                        cell_str(h.cell),
                        cell_str(c)
                    ));
                }
            }
        }
    });
    if bad.is_empty() {
        return Ok(examined);
    }
    Err(Failure {
        code: DW_HOLDER_EDITED,
        message: format!(
            "{} runtime edit(s) reach a holder the piece carries — the edit would pop the frame or \
             drop the stand mid-game, silently, with whatever it holds:{}\n  Remedy: move the \
             holder in its piece, or shrink the edit's region so it leaves the holder's cell and \
             the block it hangs from or stands on alone.",
            bad.len(),
            bad.concat()
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// spec-0111 §3.4: a `collect` lands its stack in the slot it names, by
    /// the one landing rule a `loot` fill uses — on a container its padding
    /// follows the named slot, on a stand the named slot alone, and a stand
    /// collect naming none lands nowhere (`DW1035` refuses it first).
    #[test]
    fn a_collect_lands_in_the_slot_it_names() {
        let fill = |slot: Option<&str>, slots: usize| crate::compiler::plan::CollectFillPlan {
            objective_id: "obj/take".to_string(),
            anchor: "anchor/case".to_string(),
            cell: [0, 0, 0],
            slots,
            slot: slot.map(str::to_string),
        };
        assert_eq!(
            collect_slots(HolderFamily::Container, &fill(Some("container.11"), 3)),
            ["container.11", "container.12", "container.13"]
        );
        assert_eq!(
            collect_slots(HolderFamily::Container, &fill(None, 2)),
            ["container.0", "container.1"]
        );
        assert_eq!(
            collect_slots(HolderFamily::Stand, &fill(Some("weapon.mainhand"), 1)),
            ["weapon.mainhand"]
        );
        assert_eq!(
            collect_slots(HolderFamily::Frame, &fill(None, 1)),
            [delvewright_dsl::holders::FRAME_SLOT]
        );
        assert!(collect_slots(HolderFamily::Stand, &fill(None, 1)).is_empty());
    }

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
            slots: Vec::new(),
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
        let c = census(&blocks, &[], &claims, &one_place, None);
        assert_eq!(c.rows.len(), 4);
        assert_eq!(c.count(ClaimKind::Loot), 2);
        assert_eq!(c.count(ClaimKind::Collect), 1);
        assert_eq!(c.count(ClaimKind::Device), 1);
        assert_eq!(c.decoration(), 1);
        assert_eq!(c.rows[2].block, "minecraft:barrel");
        assert!(c.rows[2].is_decoration());
        assert_eq!(c.rows[3].block, "minecraft:chest");
        assert!(c.line().contains("4 holder(s)"), "{}", c.line());
        assert!(
            c.line()
                .contains("2 loot, 1 collect, 1 device, 1 decoration, 0 empty (frozen)"),
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
            &[],
            &[claim(ClaimKind::Device, "trigger/x", [0, 0, 0], None)],
            &one_place,
            None,
        );
        assert!(c.rows.is_empty());
        assert!(c.line().contains("0 holder(s)"), "{}", c.line());
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
        let a = census(&blocks, &[], &claims, &one_place, Some(&notice())).to_json();
        let b = census(&blocks, &[], &claims, &one_place, Some(&notice())).to_json();
        assert_eq!(
            serde_json::to_vec(&a).unwrap(),
            serde_json::to_vec(&b).unwrap()
        );
        assert_eq!(a["census_input_sha256"].as_str().unwrap().len(), 64);
        let other = model(&[([0, 0, 0], "minecraft:chest")]);
        assert_ne!(
            a["census_input_sha256"],
            census(&other, &[], &claims, &one_place, None).to_json()["census_input_sha256"]
        );
    }

    #[test]
    fn scenery_without_a_notice_is_dw1018_listing_the_cell() {
        let blocks = model(&[([1, 2, 3], "minecraft:barrel")]);
        let c = census(&blocks, &[], &[], &one_place, None);
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
        let c = census(&blocks, &[], &[], &one_place, Some(&notice()));
        assert!(c.check_declared(&BTreeMap::new()).is_ok());
    }

    /// A place still standing in its stand-in cannot show its containers, so a
    /// notice over a census with no scenery is owed to its detail, stated on
    /// the line; scenery that IS seen is judged as ever.
    #[test]
    fn a_notice_over_no_scenery_waits_for_the_places_still_in_their_stand_in() {
        let empty = model(&[]);
        let massed = vec!["node/house".to_string()];
        let c = census(&empty, &[], &[], &one_place, Some(&notice())).deferring(&massed);
        assert!(c.notice_deferred());
        assert!(c.check_declared(&BTreeMap::new()).is_ok());
        let line = c.line();
        assert!(
            line.contains("1 place(s) still stand in their stand-in"),
            "{line}"
        );
        assert!(line.contains("(node/house)"), "{line}");
        assert!(line.contains("judged (DW1018) by the build"), "{line}");
        // Detailed — nothing left in a stand-in — the arm judges and refuses.
        let c = census(&empty, &[], &[], &one_place, Some(&notice())).deferring(&[]);
        assert!(!c.notice_deferred());
        assert_eq!(
            c.check_declared(&BTreeMap::new()).unwrap_err().code,
            "DW1018"
        );
        assert!(!c.line().contains("stand-in"), "{}", c.line());
        // A seen scenery container with no notice is refused at the stand-in too.
        let bare = model(&[([1, 2, 3], "minecraft:barrel")]);
        let c = census(&bare, &[], &[], &one_place, None).deferring(&massed);
        assert_eq!(
            c.check_declared(&BTreeMap::new()).unwrap_err().code,
            "DW1018"
        );
    }

    #[test]
    fn a_notice_over_a_stocked_world_is_dw1018() {
        let blocks = model(&[([0, 0, 0], "minecraft:chest")]);
        let c = census(
            &blocks,
            &[],
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
        let c = census(&passing, &[], &claims, &two_places, Some(&notice()));
        assert!(c.check_arrangement().is_ok(), "{:?}", c.check_arrangement());
        assert!(c.line().contains("0 mixed place(s)"), "{}", c.line());
        let perturbed = model(&[
            ([0, 0, 0], "minecraft:chest"),
            ([2, 0, 0], "minecraft:barrel"),
        ]);
        let c = census(&perturbed, &[], &claims, &two_places, Some(&notice()));
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
        let c = census(&blocks, &[], &claims, &two_places, None);
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
        let c = census(&passing, &[], &[], &two_places, Some(&notice()));
        assert!(c.check_arrangement().is_ok());
        let perturbed = model(&[
            ([200, 0, 0], "minecraft:barrel"),
            ([202, 0, 0], "minecraft:barrel"),
            ([204, 0, 0], "minecraft:chest"),
        ]);
        let c = census(&perturbed, &[], &[], &two_places, Some(&notice()));
        let e = c.check_arrangement().unwrap_err();
        assert_eq!(e.code, "DW1024");
        assert!(
            e.message.contains("STOREROOM — `area/crypt`"),
            "{}",
            e.message
        );
        assert!(c.line().contains("1 over the cap of 2"), "{}", c.line());
    }

    fn entity(token: &str, cell: [i32; 3]) -> PlacedHolder {
        let kind = delvewright_dsl::holders::entity_holder_by_token(token).unwrap();
        PlacedHolder {
            kind,
            cell,
            facing: (kind.family == HolderFamily::Frame).then_some(FrameFacing::North),
            yaw: 0.0,
            scene: BTreeMap::new(),
            prefab: "prefab/room".to_string(),
        }
    }

    /// spec-0111 criterion 5: one chest claimed by `loot`, one frame claimed by
    /// `collect`, one stand with two `loot` slots and one empty frame — four
    /// rows, `N 4, L 2, C 1, D 0, E 1`, each row with its kind, slots and the
    /// `frozen` mark; and two censuses of one model are one ledger.
    #[test]
    fn the_census_counts_containers_and_entity_holders_as_one_class() {
        let blocks = model(&[([0, 0, 0], "minecraft:chest[facing=north,type=single]")]);
        let entities = [
            entity("item-frame", [2, 1, 0]),
            entity("armor-stand", [4, 0, 0]),
            entity("glow-item-frame", [6, 1, 0]),
        ];
        let mut claims = vec![
            claim(ClaimKind::Loot, "loot/stores", [0, 0, 0], None),
            claim(ClaimKind::Collect, "obj/take-the-map", [2, 1, 0], None),
            claim(ClaimKind::Loot, "loot/rack", [4, 0, 0], None),
        ];
        claims[0].slots = vec!["container.0".to_string()];
        claims[1].slots = vec!["contents".to_string()];
        claims[2].slots = vec!["armor.head".to_string(), "weapon.mainhand".to_string()];
        let c = census(&blocks, &entities, &claims, &one_place, None);
        assert_eq!(c.rows.len(), 4);
        assert_eq!(c.count(ClaimKind::Loot), 2);
        assert_eq!(c.count(ClaimKind::Collect), 1);
        assert_eq!(c.decoration(), 0);
        assert_eq!(c.empty(), 1);
        let kinds: Vec<&str> = c.rows.iter().map(|r| r.kind).collect();
        assert_eq!(
            kinds,
            vec!["container", "item-frame", "armor-stand", "glow-item-frame"]
        );
        assert_eq!(c.rows[2].slots.len(), 6);
        assert_eq!(c.rows[2].filled, vec!["armor.head", "weapon.mainhand"]);
        assert!(c.rows[3].is_frozen());
        assert!(!c.rows[3].is_decoration());
        let line = c.line();
        assert!(
            line.contains("4 holder(s) in the assembled world"),
            "{line}"
        );
        assert!(
            line.contains("2 loot, 1 collect, 0 device, 0 decoration, 1 empty (frozen)"),
            "{line}"
        );
        // An empty frame is not scenery: no notice is owed for it.
        assert!(c.check_declared(&BTreeMap::new()).is_ok());
        assert!(c.check_arrangement().is_ok());
        let a = c.to_json();
        let b = census(&blocks, &entities, &claims, &one_place, None).to_json();
        assert_eq!(
            serde_json::to_vec(&a).unwrap(),
            serde_json::to_vec(&b).unwrap()
        );
        assert_eq!(a["holders"], 4);
        assert_eq!(a["empty"], 1);
        assert_eq!(a["rows"][3]["frozen"], true);
        assert_eq!(a["rows"][1]["slots"][0]["filled"], true);
        // The entities are census input: moving one moves the digest.
        let moved = [
            entity("item-frame", [2, 1, 1]),
            entity("armor-stand", [4, 0, 0]),
            entity("glow-item-frame", [6, 1, 0]),
        ];
        assert_ne!(
            a["census_input_sha256"],
            census(&blocks, &moved, &claims, &one_place, None).to_json()["census_input_sha256"]
        );
    }

    /// The amendment: a row of brewing stands, some holding potions and some
    /// empty, is legitimate — an empty stand shows it holds no bottle, so it is
    /// no decoration cell, owes no notice and takes no place in the cap.
    #[test]
    fn an_empty_brewing_stand_beside_a_filled_one_is_exempt() {
        let blocks = model(&[
            (
                [0, 0, 0],
                "minecraft:brewing_stand[has_bottle_0=false,has_bottle_1=false,has_bottle_2=false]",
            ),
            (
                [2, 0, 0],
                "minecraft:brewing_stand[has_bottle_0=false,has_bottle_1=false,has_bottle_2=false]",
            ),
            (
                [4, 0, 0],
                "minecraft:brewing_stand[has_bottle_0=false,has_bottle_1=false,has_bottle_2=false]",
            ),
            (
                [6, 0, 0],
                "minecraft:brewing_stand[has_bottle_0=false,has_bottle_1=false,has_bottle_2=false]",
            ),
        ]);
        let mut filled = claim(ClaimKind::Loot, "loot/potions", [0, 0, 0], None);
        filled.slots = vec!["container.0".to_string(), "container.1".to_string()];
        let c = census(&blocks, &[], &[filled], &one_place, None);
        assert_eq!(c.decoration(), 0);
        assert_eq!(c.exempt(), 4);
        assert!(c.check_declared(&BTreeMap::new()).is_ok());
        assert!(c.check_arrangement().is_ok());
        assert!(
            c.line().contains("4 exempt (contents show from outside)"),
            "{}",
            c.line()
        );
    }

    /// A brewing stand filled only in its hidden slots (ingredient, fuel) is an
    /// ordinary container, and is still judged: among scenery it is the mixed
    /// arm of `DW1024`.
    #[test]
    fn a_hidden_slot_brewing_stand_fill_is_still_judged() {
        let blocks = model(&[
            ([0, 0, 0], "minecraft:brewing_stand"),
            ([2, 0, 0], "minecraft:barrel"),
        ]);
        let mut hidden = claim(ClaimKind::Loot, "loot/powder", [0, 0, 0], None);
        hidden.slots = vec!["container.4".to_string()];
        let c = census(&blocks, &[], &[hidden.clone()], &one_place, Some(&notice()));
        assert_eq!(c.exempt(), 0);
        let e = c.check_arrangement().unwrap_err();
        assert_eq!(e.code, "DW1024");
        assert!(e.message.contains("loot `loot/powder`"), "{}", e.message);
        // The same stand filled in a bottle slot shows it: no longer judged.
        let mut bottle = hidden;
        bottle.slots = vec!["container.0".to_string()];
        let c = census(&blocks, &[], &[bottle], &one_place, Some(&notice()));
        assert_eq!(c.exempt(), 1);
        assert!(c.check_arrangement().is_ok());
    }
}
