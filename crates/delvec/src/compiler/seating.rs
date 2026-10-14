//! **A horizon and a piece set are a pair** (spec-0060) — the one place that
//! decides whether a library can stand on a base, and the one implementation
//! two entry points share.
//!
//! The pairing is a fact about **documents and a library** — the declared base,
//! the pieces placed on it, and each piece's metadata and its bytes. Nothing has
//! to be placed to know it, which is why this refuses at validation on
//! `DW0855`'s own precedent, before a build has moved a block.
//!
//! # Two codes, and what each one asks
//!
//! - [`DW_UNSEATABLE`] (`DW0886`) — *this piece set cannot stand on this horizon*.
//!   One question with several answers, so one code: the missing walk plane,
//!   the piece that would be seated wading, the shore that says nothing about
//!   where it meets the sea, the water that would run off the edge of a void
//!   world.
//! - [`DW_WATERLINE_FICTION`] (`DW0887`) — *a declared waterline is not in the
//!   bytes*. A property of the prefab document and its `.nbt`, so it binds
//!   wherever those two are read together and in no other way, on whatever base
//!   the campaign declares: the fiction is in the library, not in the campaign.
//!
//! [`DW0855`](crate::compiler::plan::DW_SURROUND_NO_REGION) **stays and is not
//! restated here** — one rule, one code. What this module does is name its case
//! in [`DW_UNSEATABLE`]'s own message, so a creator reading either one reads a
//! single answer about which base seats what.
//!
//! # It opens the bytes
//!
//! A verdict computed from declarations alone cannot tell a shore from a meadow
//! that says it is one: a `waterline_y` is a claim about cells, and a piece that
//! authors no water at any plane satisfies every question a reader of documents
//! can ask. The pinned library has demonstrated it in both directions — three
//! island pieces declaring a waterline over zero water cells, and the shore
//! piece that really has one declaring nothing — and a library is free to
//! declare either again. So every reader here goes to the `.nbt`, and every
//! count it prints carries the denominator it was drawn from.
//!
//! # What a site plan answers
//!
//! A campaign's map is its site plan (ADR-0035). The pairing rule belongs to
//! *whatever places pieces*, so [`check`] reads the plan's boxes, and
//! [`SeatingBinding::line`] states the numerator and the denominator.
//!
//! The question is one number: **the world y a body's feet will occupy**, asked of [`feet_under_the_sea`]. A prefab derives
//! that y (the base's walk-plane datum, minus the piece's own `walk_y`, plus the
//! cell); a plan box states it, because [`PlacedBox::floor`] IS a walk plane in
//! world coordinates. Nothing else about the pairing survives the crossing, and
//! the reason is that the objects genuinely differ — a derived box is not a
//! library asset:
//!
//! | [`Shape`] | on a site plan | why |
//! |---|---|---|
//! | [`Shape::NoWalkPlane`] | cannot arise | `floor` is required by the schema and resolves to a datum or a `y`; a box that names an undeclared datum is already `DW0112` and does not reach here |
//! | [`Shape::WadingUnderTheSea`] | **applies** | a box's floor at or below the sea plane stands a party in the water, which is the same delve defect and is known from the plan alone |
//! | [`Shape::UnstatedShore`] | cannot arise | a derived box has no authored bytes: [`crate::compiler::blockout::palette`] is a fixed set of opaque cubes and holds no water for a waterline to be about |
//! | [`Shape::FluidOffTheWorld`] | cannot arise | same palette, same reason — there is no fluid to run off a face |
//! | [`Shape::WalkPlanesDisagree`] | cannot arise | it is a property of a piece SET read together; a plan states every box's plane individually, so there is no one origin two members could disagree about |
//! | [`Shape::ShownSideEmpty`] | cannot arise | a derived box has no document to declare a side in |
//! | [`Shape::OutsideUnanswered`] | cannot arise for a box | a derived box is undocumented, and `DW0885` passes over it for that reason (`burial::Piece::documented`). A library piece a detail plan places INTO a box is documented and is judged by `DW0885` at build; this check reads the plan's boxes, not the detail plan's pieces, and that is its stated residual on this model |
//!
//! Those six are reported as not applying rather than dropped: each is a fact
//! about the site plan's own shape, and if one ever stops being true — a plan
//! that seats a library piece, a palette that gains water — this table is the
//! line that was wrong.

use std::collections::BTreeMap;
use std::path::Path;

use delvewright_dsl::metrics::Reads;
use delvewright_dsl::prefab::PrefabMeta;
use delvewright_dsl::siteplan::PlacedBox;
use delvewright_dsl::{Campaign, Diagnostic, DwCode, ExitTier, HorizonBase};

use crate::compiler::registry::PrefabRegistry;

delvewright_dsl::dw_code! {
    /// `DW0886` — **this pool cannot stand on this horizon.**
    ///
    /// Validation tier, exit 1, on `DW0855`'s precedent: the whole verdict is a
    /// fact about the documents and the library, so nothing has to be placed to
    /// know it and a creator should not spend a build finding out. The `ExitTier`
    /// says what happens if the same rule ever refuses with a build under way,
    /// which it does — [`crate::compiler::plan`] derives an ocean area's origin
    /// from the same number and cannot proceed without it.
    pub const DW_UNSEATABLE: DwCode = DwCode::new("DW0886", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0887` — **a declared waterline is not in the piece's bytes.**
    ///
    /// Validation tier, exit 1 when a campaign names the piece; also raised by
    /// `delvec prefab audit` over a whole library, from this same implementation.
    /// A declaration is a claim about the bytes, and until this existed nothing
    /// read the bytes.
    pub const DW_WATERLINE_FICTION: DwCode = DwCode::new("DW0887", ExitTier::Build);
}

/// The block a waterline is a claim about. Spelled once: the generator that
/// cuts a tide pool, the auditor that sweeps a library and the check that
/// refuses a fiction must all mean the same block, and a `waterlogged=false`
/// blockstate whose name merely contains the word is not water.
const WATER: &str = "minecraft:water";

/// The one block that is not a block. `DW0885` reads every other name as
/// something in a cell (`burial::is_solid`), and so does this.
const AIR: &str = "minecraft:air";

/// **The first cell of the piece's own air that stands on its box boundary with
/// no socket's opening to account for it** — see [`PieceFacts::opening_to_outside`].
fn opening_to_outside(
    meta: &PrefabMeta,
    standable: &std::collections::BTreeSet<[i32; 3]>,
    max: [i32; 3],
    solid: &dyn Fn([i32; 3]) -> bool,
) -> Option<[i32; 3]> {
    use std::collections::{BTreeSet, VecDeque};
    let inside = |c: [i32; 3]| (0..3).all(|a| c[a] >= 0 && c[a] <= max[a]);
    let sockets: BTreeSet<([i32; 3], &str)> = meta
        .connectors
        .iter()
        .flat_map(|c| {
            crate::admit::socket::opening_cells(c.local_pos, &c.facing, c.opening)
                .into_iter()
                .map(move |cell| (cell, c.facing.as_str()))
        })
        .collect();
    let mut seen: BTreeSet<[i32; 3]> = BTreeSet::new();
    let mut queue: VecDeque<[i32; 3]> = VecDeque::new();
    for cell in standable {
        for dy in 0..=1 {
            let c = [cell[0], cell[1] + dy, cell[2]];
            if inside(c) && !solid(c) && seen.insert(c) {
                queue.push_back(c);
            }
        }
    }
    while let Some(c) = queue.pop_front() {
        for axis in 0..3 {
            for (bound, sign) in [(0, -1), (max[axis], 1)] {
                if c[axis] != bound {
                    continue;
                }
                let mut d = [0i32; 3];
                d[axis] = sign;
                let side = crate::compiler::faces::dir_name(d);
                if !sockets.contains(&(c, side)) {
                    return Some(c);
                }
            }
        }
        for axis in 0..3 {
            for sign in [-1, 1] {
                let mut n = c;
                n[axis] += sign;
                if inside(n) && !solid(n) && seen.insert(n) {
                    queue.push_back(n);
                }
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// What one piece's own bytes say
// ---------------------------------------------------------------------------

/// **Everything a seating verdict needs about one piece, read once** — its
/// declarations and the measurements its `.nbt` answers them with.
///
/// Read once because the reads are the expensive half and because two readers
/// of one piece is two answers waiting to disagree: the campaign-tier check,
/// the library sweep and `delvec prefab seating` all consume this.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PieceFacts {
    /// The DSL prefab id, `prefab/<id>`.
    pub id: String,
    /// The metadata file's stem — what an author edits to answer a refusal.
    pub base: String,
    /// The piece's declared walk plane, local y (`walk_y`).
    pub walk_y: Option<i32>,
    /// The piece's declared waterline, local y (`waterline_y`).
    pub declared_waterline: Option<i32>,
    /// `minecraft:water` cells in the piece's own bytes.
    pub water_cells: usize,
    /// The local y of the piece's topmost `minecraft:water` block.
    pub top_water_y: Option<i32>,
    /// The local y of the piece's lowest `minecraft:water` block — the one that
    /// decides whether this piece's water is in a sea's plane at all, or a
    /// sealed well standing well above it.
    pub bottom_water_y: Option<i32>,
    /// The lowest local y holding a standable cell — where a body's feet
    /// actually land, measured by the engine's own standable rule.
    pub lowest_standable: Option<i32>,
    /// Standable cells strictly below the declared walk plane: the piece's own
    /// cellars, the cells an ocean would put under the sea.
    pub standable_below_walk: usize,
    /// Run directions in which a body of fluid leaves the piece's own outer
    /// face. What is beyond that face is not in these bytes — under a horizon
    /// that puts something there it is nothing, and under one that puts a sea
    /// there it is the sea.
    pub fluid_at_edge: usize,
    /// `.nbt` files opened for this piece. A denominator, so a piece whose
    /// tiles are missing cannot be reported as examined.
    pub nbt_opened: usize,
    /// **Each of the piece's six local sides its own bytes put a block on**,
    /// with the highest local y of the cells in front of those blocks
    /// ([`crate::compiler::burial::solid_sides`] — the reading `DW0885` makes of
    /// a placed piece, made of the unplaced one).
    pub solid_sides: BTreeMap<[i32; 3], i32>,
    /// The sides the document declares finished exterior surface, as written.
    pub shown_faces: Vec<String>,
    /// **Where the piece's own air reaches its box boundary with no socket to
    /// account for it** — the first such air cell, local, or `None` when the
    /// piece's air is sealed inside its walls and its sockets.
    ///
    /// Flooded from the piece's standable cells, feet and head, through every
    /// cell holding no block. A socket's opening is excluded because the layout
    /// answers for it: a mated one opens into the neighbour, an unmated one is
    /// sealed with wall material (`solver::seal_layout`). Anything else — an open
    /// top, a window, a shore — is where a party gets outside, and outside is
    /// where `DW0885` judges every side of every piece its air can reach.
    pub opening_to_outside: Option<[i32; 3]>,
    /// For each socket, its local y less the piece's `walk_y`: how high above
    /// the walk plane a neighbour is joined. A set whose sockets all agree
    /// stands every member on one walk plane, so an origin derived for one is
    /// the origin of all; any disagreement stands a member higher or lower.
    pub socket_rises: Vec<i32>,
    /// **Everything else this document claims about these same bytes**, held to
    /// them at the same read (`DW0888`, [`crate::compiler::claims`]).
    ///
    /// It rides here because it is answered from exactly the two things a
    /// `PieceFacts` already has — the document and the grid its templates
    /// assemble into — and because every door that asks the seating question
    /// (the library sweep, `delvec prefab seating`, the campaign's own check)
    /// owes the same answer about the same piece. A second read for a second
    /// rule is a second chance for the two to disagree.
    pub claims: crate::compiler::claims::ClaimVerdict,
}

impl PieceFacts {
    /// Read one piece: its declarations from `meta`, its measurements from the
    /// `.nbt` files beside it in `dir`.
    pub fn read(meta: &PrefabMeta, dir: &Path) -> Result<PieceFacts, String> {
        // A template whose own extent is not the one its document declares is
        // two exports of one piece, and `DW0803` is the one answer to that: the
        // move is to re-export. Measuring the blocks anyway would judge a shape
        // the piece does not have and send an author to the wrong document, so
        // such a piece is unreadable here and is left to the rule that owns it.
        let templates = meta.templates();
        let sizes = crate::par::map(&templates, |t| {
            std::fs::read(dir.join(t.file))
                .ok()
                .map(|raw| crate::compiler::assembled::structure_size(&raw))
        });
        for (t, size) in templates.iter().zip(sizes) {
            let Some(read) = size else {
                continue;
            };
            if let Some(actual) = read
                && actual != t.size
            {
                return Err(format!(
                    "`{}` is {actual:?} in its own `.nbt` and {:?} in the document — two exports \
                     of one piece (DW0803)",
                    t.file, t.size
                ));
            }
        }
        let (grid, bytes) = crate::admit::settling::piece_bytes(meta, dir)?;
        Ok(PieceFacts::measure(meta, &grid, &bytes))
    }

    /// The same measurements, over a grid the caller already has.
    ///
    /// `delvec prefab audit` assembles one — a single template's, or a whole
    /// zone's from its manifest — before anything here is asked, and the
    /// admission event is where `DW0887` has to bind: it is what CI runs over a
    /// library and what the prefab procedure runs on every piece, one file at a
    /// time. Splitting the read from the measurement is what lets that door use
    /// this rule rather than grow a second copy of it.
    pub fn measure(
        meta: &PrefabMeta,
        grid: &crate::grammar::model::VoxelModel,
        bytes: &crate::admit::settling::ByteFacts,
    ) -> PieceFacts {
        use crate::schem::nav::standable_cells;

        let mut water_cells = 0usize;
        let mut top_water_y = None::<i32>;
        let mut bottom_water_y = None::<i32>;
        for pos in grid.region().positions() {
            let Some(state) = grid.get(pos) else { continue };
            if state.name == WATER {
                water_cells += 1;
                top_water_y = Some(top_water_y.map_or(pos[1], |t: i32| t.max(pos[1])));
                bottom_water_y = Some(bottom_water_y.map_or(pos[1], |b: i32| b.min(pos[1])));
            }
        }
        let standable = standable_cells(grid);
        let lowest_standable = standable.iter().map(|c| c[1]).min();
        let standable_below_walk = match meta.walk_y {
            Some(w) => standable.iter().filter(|c| c[1] < w).count(),
            None => 0,
        };
        let fluid_at_edge = crate::grammar::settle::fluid_bodies(grid).at_edge.len();
        let region = grid.region();
        let max = [
            region.size[0] as i32 - 1,
            region.size[1] as i32 - 1,
            region.size[2] as i32 - 1,
        ];
        let solid = |pos: [i32; 3]| grid.get(pos).is_some_and(|b| b.name != AIR);
        let solid_sides = crate::compiler::burial::solid_sides(
            region.positions().filter(|p| solid(*p)),
            [0, 0, 0],
            max,
        );
        let opening_to_outside = opening_to_outside(meta, &standable, max, &solid);
        let socket_rises = match meta.walk_y {
            Some(w) => meta.connectors.iter().map(|c| c.local_pos[1] - w).collect(),
            None => Vec::new(),
        };
        PieceFacts {
            id: meta.prefab_id.clone(),
            base: meta.base().to_string(),
            walk_y: meta.walk_y,
            declared_waterline: meta.waterline_y,
            water_cells,
            top_water_y,
            bottom_water_y,
            lowest_standable,
            standable_below_walk,
            fluid_at_edge,
            nbt_opened: bytes.opened,
            solid_sides,
            shown_faces: meta.shown_faces.clone(),
            opening_to_outside,
            socket_rises,
            claims: crate::compiler::claims::check_piece(meta, grid, bytes),
        }
    }
}

// ---------------------------------------------------------------------------
// The verdicts
// ---------------------------------------------------------------------------

/// **Which shape of the pairing a reason is** — the enumerated answers to the
/// one question `DW0886` asks, plus the waterline's own.
///
/// A kind rather than a substring, because a caller that has to narrow this set
/// must say which member of it it means: a filter keyed to the wording of a
/// message is a filter that silently widens the day the message is reworded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// The piece states no walk plane at all.
    NoWalkPlane,
    /// The piece would be seated with a body under the sea.
    WadingUnderTheSea,
    /// The piece authors water and says nothing about where it meets the sea.
    UnstatedShore,
    /// The piece's fluid would run off the edge of a world with no outside.
    FluidOffTheWorld,
    /// The piece's declared waterline is not in its bytes.
    WaterlineFiction,
    /// The piece's declared walk plane does not stand one course above its own
    /// declared waterline, so the placement the one derives puts the other off
    /// the sea plane.
    WalkPlaneOffItsWaterline,
    /// **A property of the SET, not of any member**: the pieces this area could
    /// draw do not agree about their own walk plane, and one origin cannot be
    /// derived from two numbers.
    WalkPlanesDisagree,
    /// The set lets a party outside, and this member puts a block on a side
    /// that neither the horizon buries nor the document declares shown — the
    /// face `DW0885` refuses once the world is assembled, known from the
    /// documents and the bytes before anything is placed.
    OutsideUnanswered,
    /// The member declares a side shown that its bytes put no block on.
    ShownSideEmpty,
}

/// One reason a member cannot be seated, with the code that owns it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reason {
    /// Which of the pairing's shapes this is.
    pub shape: Shape,
    /// The code a campaign is refused with.
    pub code: DwCode,
    /// The member this is about.
    pub member: String,
    /// The sides of the member this reason is about, by name — empty for a
    /// reason that is not about a side.
    pub sides: Vec<String>,
    /// The one line `delvec prefab seating` prints beside the member.
    pub short: String,
    /// The full refusal, with its moves.
    pub full: String,
}

/// **The waterline claim, checked against the bytes** (`DW0887`).
///
/// Base-independent by construction: a declaration that is a fiction is a
/// defect of the library, and the campaign that happens to name the piece has
/// nothing to do with whether the water is there.
pub fn waterline_reason(f: &PieceFacts) -> Option<Reason> {
    let declared = f.declared_waterline?;
    let (short, detail) = match f.top_water_y {
        None => (
            format!("`waterline_y: {declared}` declared, no water block anywhere in the piece"),
            format!(
                "declares `waterline_y: {declared}` and authors no `{WATER}` block anywhere in \
                 its {opened} template(s). A waterline is a claim about the bytes — the local y \
                 of the piece's top authored water block — and there is no water here for it to \
                 be about",
                opened = f.nbt_opened,
            ),
        ),
        Some(top) if top != declared => (
            format!(
                "`waterline_y: {declared}` declared, top authored water block is at local y={top}"
            ),
            format!(
                "declares `waterline_y: {declared}` while its topmost `{WATER}` block stands at \
                 local y={top} ({cells} water cell(s) in {opened} template(s)). The declaration \
                 and the bytes disagree about where this piece meets a sea",
                cells = f.water_cells,
                opened = f.nbt_opened,
            ),
        ),
        Some(_) => return None,
    };
    Some(Reason {
        shape: Shape::WaterlineFiction,
        code: DW_WATERLINE_FICTION,
        member: f.id.clone(),
        sides: Vec::new(),
        short,
        full: format!(
            "prefab `{id}` {detail}. The moves, in the order to try them: (1) DELETE the \
             declaration from `{base}.json` — a piece that authors no shore has no waterline to \
             state, and deleting it does not make the piece unseatable on an ocean, which seats \
             it by `walk_y`; (2) CORRECT the number to the local y the bytes actually hold, which \
             is what the generator that laid them would have written; (3) AUTHOR the shore the \
             piece claims, so the declaration becomes true. Do not answer this by changing the \
             horizon: the fiction is in the library, not in the campaign, and it is a fiction on \
             every base",
            id = f.id,
            base = f.base,
        ),
    })
}

/// **The one question a horizon asks of anything that will stand a body**: at
/// the world y this thing puts a party's feet, is there sea?
///
/// Spelled once because both placement models ask it and neither may drift from
/// the other. An `areas[]` piece reaches this number by derivation — the base's
/// walk-plane datum less the piece's own `walk_y`, plus the local cell — and a
/// site plan's box states it outright, since [`PlacedBox::floor`] is already a
/// walk plane in world coordinates. What makes a delve wrong is the same fact
/// either way: a body's feet at or below the sea plane is a party wading its
/// critical path.
///
/// A base with no sea answers `false` for every y, which is why this is asked of
/// the base rather than under an `if ocean` at each call site: a base that grew
/// a sea would gain both models at once.
#[must_use]
pub fn feet_under_the_sea(base: HorizonBase, world_y: i32) -> bool {
    base == HorizonBase::Ocean && world_y <= crate::compiler::horizon::SEA_LEVEL
}

/// **Can this place stand on this base** — the site plan's half of the pairing
/// (`DW0886`).
///
/// The module table says which of [`Shape`]'s answers can be about a derived box
/// at all, and exactly one can: a plan states its walk planes, so the wading
/// question is asked with the derivation already done and the other four are
/// facts about a library this campaign does not have.
///
/// The moves are the site plan's own. Telling the author of a derived box to
/// declare `walk_y` in a prefab document would name a file that cannot exist —
/// the blockout is authored by no one (spec-0049 §5) — which is precisely the
/// remedy-unreachability this whole module was written to end.
#[must_use]
pub fn box_reasons(base: HorizonBase, b: &PlacedBox) -> Vec<Reason> {
    let floor = b.floor.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
    if !feet_under_the_sea(base, floor) {
        return Vec::new();
    }
    let sea = crate::compiler::horizon::SEA_LEVEL;
    let walk_ref = crate::compiler::horizon::OCEAN_WALK_REF_Y;
    vec![Reason {
        shape: Shape::WadingUnderTheSea,
        code: DW_UNSEATABLE,
        member: b.node.as_str().to_string(),
        sides: Vec::new(),
        short: format!(
            "stands its walk plane at y={floor}, {n} block(s) at or below this world's sea \
             plane (y={sea}) — a body stands in this place with the water over its feet",
            n = sea - floor + 1,
        ),
        full: format!(
            "place `{node}` stands its walk plane at world y={floor}, and this world's sea \
             plane is y={sea}: a body standing in this place has the sea at or over its feet, \
             and vanilla floods the room on boot. A `{base}` horizon's walk plane is y={walk_ref}, \
             one block above the water — the beach relationship a body can climb out of — and a \
             site plan states each place's plane itself rather than deriving it, so this is a \
             fact about the plan and nothing has to be placed to know it. The moves: (1) RAISE \
             this place's `floor` to y={walk_ref} or above — if it names a `datum`, moving the \
             datum lifts every place standing on it, and the plan's `region` has to contain \
             where it lands; (2) CHOOSE another horizon — `void` puts no water anywhere, and \
             `valley` builds terrain the place is buried in, which is what an undercroft cut \
             below grade is usually for",
            node = b.node.as_str(),
            base = base.token(),
        ),
    }]
}

// ---------------------------------------------------------------------------
// The library
// ---------------------------------------------------------------------------

/// **A prefab library, read once**: every document's facts, and the counts that
/// say what was actually opened.
pub struct Library {
    /// Facts per prefab id, in id order.
    pub pieces: BTreeMap<String, PieceFacts>,
    /// Prefab documents the registry loaded.
    pub documents: usize,
    /// `.nbt` files opened across every document.
    pub nbt_opened: usize,
    /// One line per document this could not read, with why. A library that
    /// cannot be opened is a red, never a small number.
    pub unreadable: Vec<String>,
    /// **Every byte-asserting declaration in the library**, held to the bytes at
    /// the same read (`DW0888`).
    pub claims: crate::compiler::claims::ClaimBinding,
    /// The declarations the bytes deny, in prefab-id order.
    pub denied_claims: Vec<crate::compiler::claims::Claim>,
}

impl Library {
    /// Read every prefab the registry holds.
    pub fn read(prefabs: &PrefabRegistry, dir: &Path) -> Library {
        let mut lib = Library {
            pieces: BTreeMap::new(),
            documents: 0,
            nbt_opened: 0,
            unreadable: Vec::new(),
            claims: crate::compiler::claims::ClaimBinding::default(),
            denied_claims: Vec::new(),
        };
        for (id, meta) in prefabs.all() {
            lib.documents += 1;
            match PieceFacts::read(meta, dir) {
                Ok(f) => {
                    lib.nbt_opened += f.nbt_opened;
                    lib.claims.add(&f.claims.binding);
                    lib.denied_claims.extend(f.claims.denied.iter().cloned());
                    lib.pieces.insert(id.clone(), f);
                }
                Err(e) => lib.unreadable.push(format!("{id}: {e}")),
            }
        }
        lib
    }

    /// Waterline declarations found, and how many of them the bytes bear out.
    ///
    /// Printed on every run including the one that finds nothing, because a
    /// count only says something when the run that found nothing prints it too.
    pub fn waterline_census(&self) -> (usize, usize) {
        let declared = self
            .pieces
            .values()
            .filter(|f| f.declared_waterline.is_some())
            .count();
        let borne_out = self
            .pieces
            .values()
            .filter(|f| f.declared_waterline.is_some() && waterline_reason(f).is_none())
            .count();
        (declared, borne_out)
    }
}

// ---------------------------------------------------------------------------
// The campaign's own verdict
// ---------------------------------------------------------------------------

/// What the seating check examined, in the campaign it was run on.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SeatingBinding {
    /// The base the campaign declares.
    pub base: String,
    /// **Boxes the campaign's site plan states**, printed on every run so a
    /// campaign that places no boxes reads as a measured zero rather than as a
    /// check that was never asked.
    pub boxes: usize,
    /// Boxes this examined: the ones the plan's own resolver placed. A box whose
    /// floor names an undeclared datum is not among them — `DW0112` has already
    /// refused it, and a place with no plane has no walk plane to judge — so the
    /// two numbers differing is itself the finding.
    pub boxes_examined: usize,
    /// Bound places whose piece declares a `waterline_y`, on a base with a sea.
    pub waterlines_declared: usize,
    /// Of those, the ones whose frame stands that waterline on the sea plane.
    pub waterlines_on_the_sea: usize,
    /// **The rest of what these same documents claim about these same bytes**
    /// (`DW0888`), examined at the same read.
    pub claims: crate::compiler::claims::ClaimBinding,
}

impl SeatingBinding {
    /// **The one line this check owes its reader**, printed whether it found
    /// anything or not.
    pub fn line(&self) -> String {
        format!(
            "seating binding: horizon base `{base}`; site plan: {bex} of {boxes} box(es) judged \
             against this base; {wl} bound waterline declaration(s) examined, {ok} standing on \
             the sea plane.",
            base = self.base,
            bex = self.boxes_examined,
            boxes = self.boxes,
            wl = self.waterlines_declared,
            ok = self.waterlines_on_the_sea,
        ) + " "
            + &self.claims.line()
    }
}

/// **The campaign-tier refusal** — `DW0886` and `DW0887` over the pieces this
/// campaign actually seats, before anything is placed.
///
/// Returns the binding beside the diagnostics: a check that refuses nothing
/// still owes the numbers it refused nothing over.
///
/// # The walk plane is owed on every base
///
/// `walk_y` is owed on **every** base (spec-0060 §4.1) and refused on every
/// base here, from the one rule [`seating_reasons`] states — so a campaign is
/// refused for a silent walk plane wherever `delvec prefab seating` reports one,
/// and the command and the campaign give a creator one answer rather than two.
/// The ocean is where the number is also CONSUMED, because the area origin is
/// derived from it (spec-0060 §3.2); `void` and `valley` state their own datum
/// and still owe the declaration, because a piece with no walk plane is a piece
/// nothing can say a body stands on.
pub fn check(
    campaign: &Campaign,
    prefabs: &PrefabRegistry,
    prefabs_dir: &Path,
) -> (SeatingBinding, Vec<Diagnostic>) {
    let base = crate::compiler::horizon::base_of(campaign);
    let mut binding = SeatingBinding {
        base: base.token().to_string(),
        ..SeatingBinding::default()
    };
    let mut diags = Vec::new();

    // **The source of placed pieces.**
    //
    // A site plan embeds boxes rather than seating library prefabs, and until
    // this ran the pairing rule had nothing to say about a campaign that placed
    // that way — which is every campaign on the one base that builds terrain,
    // since `valley` needs a declared extent (`DW0855`) and a plan is the only
    // statement of one. The rule belongs to whatever places pieces, so it is
    // asked here, of the same base, under the same code, at the same tier: a
    // box's floor IS a walk plane in world coordinates, so nothing has to be
    // placed to know where a body's feet will land.
    //
    // Read through the plan's own resolver, never a second reading of the
    // document: a box's corner is derived from the packing (spec-0059), and a
    // private copy of that would answer for a place the build does not build.
    if let Some(plan) = &campaign.site_plan {
        binding.boxes = plan.content.boxes.len();
        let mut reads = Reads::new();
        let boxes = delvewright_dsl::siteplan::placed_boxes(campaign, &mut reads);
        binding.boxes_examined = boxes.len();
        for b in &boxes {
            for r in box_reasons(base, b) {
                diags.push(Diagnostic::error(
                    r.code,
                    "site-plan",
                    format!("/content/boxes/{}", r.member),
                    format!(
                        "this site plan puts `{member}` on a `{base}` horizon, and it cannot \
                         stand there: {full}. {tail}",
                        member = r.member,
                        base = base.token(),
                        full = r.full,
                        tail = SITE_PLAN_TAIL,
                    ),
                ));
            }
        }
    }

    // **And everything the bound pieces' documents claim about their bytes**
    // (`DW0888`), reported once per PIECE: a piece two places bind is one
    // library asset with one set of declarations, so the path names the piece.
    let mut read: BTreeMap<String, PieceFacts> = BTreeMap::new();
    for piece in prefabs.campaign_pieces(campaign) {
        if read.contains_key(&piece) {
            continue;
        }
        let Some(meta) = prefabs.get(&piece) else {
            continue; // `DW0842` refuses a piece the library does not hold.
        };
        if let Ok(facts) = PieceFacts::read(meta, prefabs_dir) {
            read.insert(piece, facts);
        }
    }
    diags.extend(bound_walk_planes(campaign, base, &read));
    // A waterline that is a fiction is a defect of the library on every base
    // (`DW0887`), asked of each piece this campaign binds.
    for facts in read.values() {
        if let Some(r) = waterline_reason(facts) {
            diags.push(Diagnostic::error(
                r.code,
                "detail-plan",
                format!("/prefabs/{}", facts.base),
                format!("this campaign binds `{}`, and {}.", r.member, r.full),
            ));
        }
    }
    if base == HorizonBase::Ocean {
        diags.extend(bound_shores(campaign, &read, &mut binding));
    }
    if base == HorizonBase::Void {
        diags.extend(fluid_off_the_world(campaign, &read));
    }
    for facts in read.values() {
        binding.claims.add(&facts.claims.binding);
        for c in &facts.claims.denied {
            diags.push(Diagnostic::error(
                crate::compiler::claims::DW_CLAIM_DENIED,
                "detail-plan",
                format!("/prefabs/{}", facts.base),
                format!(
                    "this campaign binds `{member}`, and its prefab document says something its \
                     own bytes deny: {full}.",
                    member = c.member,
                    full = c.full,
                ),
            ));
        }
    }
    (binding, diags)
}

/// **Where a bound piece meets the sea** — asked of each place a piece is bound
/// to, on a base with a sea, before anything is placed.
///
/// A bound piece is placed at its frame's lowest corner, so the world y of each
/// of its local courses is known from the plan alone: a declared waterline
/// stands at `frame.lo.y + waterline_y` and is held to the sea plane
/// (`DW0344`); water authored at or below the sea plane with no waterline
/// declared is an unstated shore (`DW0886`).
fn bound_shores(
    campaign: &Campaign,
    read: &BTreeMap<String, PieceFacts>,
    binding: &mut SeatingBinding,
) -> Vec<Diagnostic> {
    let sea = crate::compiler::horizon::SEA_LEVEL;
    let mut out = Vec::new();
    let Some(dp) = &campaign.detail_plan else {
        return out;
    };
    let frames = crate::compiler::detail::frames(campaign);
    for (i, row) in dp.content.details.iter().enumerate() {
        let Some(f) = read.get(row.piece.as_str()) else {
            continue;
        };
        let Some((frame, _)) = frames.iter().find(|(fr, _)| fr.node == row.place) else {
            continue; // the binding gates refuse a place the plan does not have
        };
        let lo = i32::try_from(frame.lo[1]).unwrap_or(i32::MIN);
        let at = format!("/content/details/{i}/piece");
        if let Some(wl) = f.declared_waterline {
            binding.waterlines_declared += 1;
            let world = lo + wl;
            if world == sea {
                binding.waterlines_on_the_sea += 1;
                continue;
            }
            let (n, way) = if world > sea {
                (world - sea, "above")
            } else {
                (sea - world, "below")
            };
            out.push(Diagnostic::error(
                crate::compiler::plan::DW_OCEAN_WATERLINE,
                "detail-plan",
                at,
                format!(
                    "`{place}` places `{piece}` with its frame's lowest course at y={lo}, so the \
                     waterline the piece declares (local y={wl}) stands at y={world}, {n} \
                     block(s) {way} this world's sea plane (y={sea}). The shore meets no sea. \
                     The move: seat the place so its frame's lowest course is at y={want} — the \
                     site plan's `region` and this box's `floor` together — or correct \
                     `waterline_y` in `{base}.json` if the bytes hold the water elsewhere",
                    place = row.place,
                    piece = row.piece,
                    want = sea - wl,
                    base = f.base,
                ),
            ));
        } else if f.water_cells > 0 && f.bottom_water_y.is_some_and(|b| lo + b <= sea) {
            out.push(Diagnostic::error(
                DW_UNSEATABLE,
                "detail-plan",
                at,
                format!(
                    "`{place}` places `{piece}` on an `ocean` horizon, and prefab `{piece}` \
                     authors {n} `{WATER}` cell(s) at or below the sea plane, the topmost at \
                     local y={top}, and declares no `waterline_y`. Nothing then states where \
                     this piece's water meets the world's. The move: DECLARE `waterline_y: \
                     {top}` in `{base}.json`, the local y its top water block stands at; \
                     `DW0887` holds that number to the bytes",
                    place = row.place,
                    piece = row.piece,
                    n = f.water_cells,
                    top = f.top_water_y.unwrap_or(0),
                    base = f.base,
                ),
            ));
        }
    }
    out
}

/// **A bound piece states its walk plane** (`DW0886`), on every base: the local
/// y a body's feet occupy on its principal floor is a measurement of the piece,
/// and a piece that states none is a piece nothing can say a body stands on.
fn bound_walk_planes(
    campaign: &Campaign,
    base: HorizonBase,
    read: &BTreeMap<String, PieceFacts>,
) -> Vec<Diagnostic> {
    let Some(dp) = &campaign.detail_plan else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (i, row) in dp.content.details.iter().enumerate() {
        let Some(f) = read.get(row.piece.as_str()) else {
            continue;
        };
        if f.walk_y.is_some() {
            continue;
        }
        out.push(Diagnostic::error(
            DW_UNSEATABLE,
            "detail-plan",
            format!("/content/details/{i}/piece"),
            format!(
                "`{place}` places `{piece}` on a `{token}` horizon, and prefab `{piece}` \
                 declares no `walk_y`, so nothing states the piece's own walk plane. It has no \
                 default. The move: DECLARE `walk_y` in `{base_file}.json`, the local y of the \
                 cell a body's feet occupy on this piece's principal floor — a measurement the \
                 generator that built it writes; this piece's own bytes stand a body at local \
                 y={measured}",
                place = row.place,
                piece = row.piece,
                token = base.token(),
                base_file = f.base,
                measured = f
                    .lowest_standable
                    .map_or_else(|| "no cell at all".to_string(), |y| y.to_string()),
            ),
        ));
    }
    out
}

/// **A bound piece whose water runs out of its own faces, on a base that puts
/// nothing beyond them** (`DW0886`): `void` declares there is nothing outside
/// the placed geometry, so that water runs off the world — what `DW0318`
/// refuses once the world is assembled, said here from the piece's own bytes
/// before a block is placed.
fn fluid_off_the_world(
    campaign: &Campaign,
    read: &BTreeMap<String, PieceFacts>,
) -> Vec<Diagnostic> {
    let Some(dp) = &campaign.detail_plan else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (i, row) in dp.content.details.iter().enumerate() {
        let Some(f) = read.get(row.piece.as_str()) else {
            continue;
        };
        if f.fluid_at_edge == 0 {
            continue;
        }
        out.push(Diagnostic::error(
            DW_UNSEATABLE,
            "detail-plan",
            format!("/content/details/{i}/piece"),
            format!(
                "`{place}` places `{piece}` on a `void` horizon, and prefab `{piece}` authors a \
                 body of fluid that reaches its own outer face in {n} run direction(s). `void` \
                 puts nothing outside the placed geometry, so that water runs off the world \
                 forever, which `DW0318` refuses of the assembled world. The moves: (1) CHOOSE the horizon the piece was built for — a shore \
                 belongs to an `ocean`, which puts a sea against that face; (2) SEAL the body \
                 inside the piece's own bytes, which `delvec prefab audit` reports as a run \
                 direction that leaves it",
                place = row.place,
                piece = row.piece,
                n = f.fluid_at_edge,
            ),
        ));
    }
    out
}

/// What a `DW0886` about a plan's own box ends with. The tail above names the
/// road out of an `areas[]` campaign, which a site-plan campaign has already
/// taken; what a reader here needs instead is that the derived massing is
/// authored by no one, so the edit is to the plan and never to a piece.
const SITE_PLAN_TAIL: &str = "There is no prefab document to edit here and no `walk_y` to declare: \
                              a site plan's mass is DERIVED, authored by no one, so the only \
                              statement of this place's walk plane is the plan's own `floor`";

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> PieceFacts {
        PieceFacts {
            id: "prefab/x".into(),
            base: "x".into(),
            walk_y: Some(1),
            declared_waterline: None,
            water_cells: 0,
            top_water_y: None,
            bottom_water_y: None,
            lowest_standable: Some(1),
            standable_below_walk: 0,
            fluid_at_edge: 0,
            nbt_opened: 1,
            solid_sides: BTreeMap::new(),
            shown_faces: Vec::new(),
            opening_to_outside: None,
            socket_rises: Vec::new(),
            claims: crate::compiler::claims::ClaimVerdict::default(),
        }
    }

    /// A waterline over no water is a fiction whatever the base, because the
    /// fiction is in the library rather than in the campaign.
    #[test]
    fn a_waterline_over_no_water_is_refused() {
        let mut f = facts();
        f.declared_waterline = Some(2);
        let r = waterline_reason(&f).expect("a fiction is refused");
        assert_eq!(r.code.id(), "DW0887");
        assert!(r.short.contains("no water block anywhere"), "{}", r.short);
    }

    /// A waterline over water at a different plane is the second shape, and it
    /// names both numbers.
    #[test]
    fn a_waterline_at_the_wrong_plane_is_refused() {
        let mut f = facts();
        f.declared_waterline = Some(2);
        f.water_cells = 9;
        f.top_water_y = Some(1);
        let r = waterline_reason(&f).expect("a wrong plane is refused");
        assert!(r.short.contains("local y=1"), "{}", r.short);
        assert!(r.full.contains("`waterline_y: 2`"), "{}", r.full);
    }

    /// A waterline the bytes bear out is not a finding, and a piece that
    /// declares none is not one either.
    #[test]
    fn a_true_waterline_and_no_waterline_are_both_clean() {
        let mut f = facts();
        assert!(waterline_reason(&f).is_none());
        f.declared_waterline = Some(2);
        f.water_cells = 4;
        f.top_water_y = Some(2);
        assert!(waterline_reason(&f).is_none());
    }

    /// The binding line states its denominator on a run that found nothing.
    #[test]
    fn the_binding_line_prints_its_denominator_when_it_found_nothing() {
        let line = SeatingBinding {
            base: "void".into(),
            ..SeatingBinding::default()
        }
        .line();
        assert!(line.contains("site plan: 0 of 0 box(es)"), "{line}");
        let plan = SeatingBinding {
            base: "valley".into(),
            boxes: 7,
            boxes_examined: 7,
            ..SeatingBinding::default()
        }
        .line();
        assert!(plan.contains("site plan: 7 of 7 box(es)"), "{plan}");
    }

    /// A box's floor, in world coordinates, and a piece's derived cell reach the
    /// **same** predicate: one rule, whatever placed the thing.
    #[test]
    fn one_rule_answers_for_both_placement_models() {
        let sea = crate::compiler::horizon::SEA_LEVEL;
        for base in [HorizonBase::Void, HorizonBase::Valley] {
            assert!(
                !feet_under_the_sea(base, sea - 10),
                "{base:?} has no sea for anything to stand in"
            );
        }
        assert!(feet_under_the_sea(HorizonBase::Ocean, sea));
        assert!(feet_under_the_sea(HorizonBase::Ocean, sea - 1));
        assert!(!feet_under_the_sea(HorizonBase::Ocean, sea + 1));
        // The walk plane an ocean seats a dry body on is the first y that passes.
        assert_eq!(crate::compiler::horizon::OCEAN_WALK_REF_Y, sea + 1);
    }

    /// A place whose floor stands at or under the sea is refused, and the moves
    /// are the PLAN's — there is no prefab document behind a derived box.
    #[test]
    fn a_place_cut_below_the_sea_is_refused_on_an_ocean() {
        let sunk = plan_box("node/undercroft", 60);
        let rs = box_reasons(HorizonBase::Ocean, &sunk);
        assert_eq!(rs.len(), 1, "one question, one answer: {rs:?}");
        assert_eq!(rs[0].shape, Shape::WadingUnderTheSea);
        assert_eq!(rs[0].code.id(), "DW0886");
        assert!(rs[0].short.contains("y=60"), "{}", rs[0].short);
        assert!(rs[0].full.contains("RAISE"), "{}", rs[0].full);
        assert!(
            !rs[0].full.contains("walk_y"),
            "a derived box has no prefab document to declare one in:\n{}",
            rs[0].full
        );
    }

    /// The sea plane itself is under water, and one course above it is the dry
    /// walk plane an ocean is for — the boundary asserted rather than described.
    #[test]
    fn a_place_one_course_over_the_sea_is_seated() {
        let sea = crate::compiler::horizon::SEA_LEVEL;
        assert!(
            box_reasons(HorizonBase::Ocean, &plan_box("node/quay", sea + 1)).is_empty(),
            "a quay one course over the water is exactly what an ocean seats"
        );
        assert!(!box_reasons(HorizonBase::Ocean, &plan_box("node/quay", sea)).is_empty());
        // And a base with no sea judges the same sunken place clean, because
        // what is outside the placed geometry is what decides.
        for base in [HorizonBase::Void, HorizonBase::Valley] {
            assert!(
                box_reasons(base, &plan_box("node/undercroft", 40)).is_empty(),
                "{base:?} puts no water against a low place"
            );
        }
    }

    /// One placed box, at a stated walk plane. The footprint and headroom are
    /// not this rule's subject — the floor is the whole of what a horizon asks.
    fn plan_box(node: &str, floor: i32) -> PlacedBox {
        PlacedBox {
            node: delvewright_dsl::NodeId(node.to_string()),
            foot: [0, 7, 0, 7],
            floor: i64::from(floor),
            clearance: 4,
            open: false,
            base: delvewright_dsl::siteplan::Base::Ground,
            roof: None,
        }
    }
}
