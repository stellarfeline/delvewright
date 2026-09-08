//! **A horizon and a piece set are a pair** (spec-0060) — the one place that
//! decides whether a library can stand on a base, and the one implementation
//! two entry points share.
//!
//! # The defect this exists to close
//!
//! Every gate around a horizon was right on its own and the three of them
//! together formed a cycle: `ocean` refused a piece and told the author to
//! choose another horizon, `void` refused the same piece and told them to bury
//! it under `valley`, `valley` refused the campaign and told them to write a
//! site plan, and adding one to an `areas[]` campaign is two placement
//! authorities. Each remedy was the next gate's refusal. `CLAUDE.md` names what
//! the three of them are together: *a gate that names a remedy owes a check
//! that the remedy is reachable*, and nothing held that check.
//!
//! The pairing is a fact about **documents and a library** — the declared base,
//! the pools the world names, the members those pools hold, and each member's
//! metadata and its bytes. Nothing has to be placed to know it, which is why
//! this refuses at validation on `DW0855`'s own precedent, before a build has
//! moved a block.
//!
//! # Two codes, and what each one asks
//!
//! - [`DW_UNSEATABLE`] (`DW0886`) — *this pool cannot stand on this horizon*.
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
//! A verdict computed from declarations alone would report the shipped island
//! pool seatable on an ocean, which is exactly the green that was there before
//! this existed: three of that pool's five waterline declarations stand over no
//! water at all. So every reader here goes to the `.nbt`, and every count it
//! prints carries the denominator it was drawn from.

use std::collections::BTreeMap;
use std::path::Path;

use delvewright_dsl::prefab::PrefabMeta;
use delvewright_dsl::{Campaign, Diagnostic, DwCode, ExitTier, HorizonBase};

use crate::compiler::registry::PrefabRegistry;

/// `DW0886` — **this pool cannot stand on this horizon.**
///
/// Validation tier, exit 1, on `DW0855`'s precedent: the whole verdict is a
/// fact about the documents and the library, so nothing has to be placed to
/// know it and a creator should not spend a build finding out. The `ExitTier`
/// says what happens if the same rule ever refuses with a build under way,
/// which it does — [`crate::compiler::plan`] derives an ocean area's origin
/// from the same number and cannot proceed without it.
pub const DW_UNSEATABLE: DwCode = DwCode::new("DW0886", ExitTier::Build);

/// `DW0887` — **a declared waterline is not in the piece's bytes.**
///
/// Validation tier, exit 1 when a campaign names the piece; also raised by
/// `delvec prefab audit` over a whole library, from this same implementation.
/// A declaration is a claim about the bytes, and until this existed nothing
/// read the bytes.
pub const DW_WATERLINE_FICTION: DwCode = DwCode::new("DW0887", ExitTier::Build);

/// The block a waterline is a claim about. Spelled once: the generator that
/// cuts a tide pool, the auditor that sweeps a library and the check that
/// refuses a fiction must all mean the same block, and a `waterlogged=false`
/// blockstate whose name merely contains the word is not water.
const WATER: &str = "minecraft:water";

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
}

impl PieceFacts {
    /// Read one piece: its declarations from `meta`, its measurements from the
    /// `.nbt` files beside it in `dir`.
    pub fn read(meta: &PrefabMeta, dir: &Path) -> Result<PieceFacts, String> {
        let (grid, nbt_opened) = crate::admit::settling::piece_grid(meta, dir)?;
        Ok(PieceFacts::measure(meta, &grid, nbt_opened))
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
        nbt_opened: usize,
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
            nbt_opened,
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

/// **A shore stands its walk plane one course above its own waterline**
/// (`DW0344`, first arm, asked of the documents).
///
/// An ocean area's origin is `walk_ref - walk_y`, so a piece's declared
/// waterline lands at `walk_ref - walk_y + waterline_y`, and that equals the sea
/// plane exactly when `waterline_y == walk_y - 1`. Both numbers are in the
/// document: nothing has to be placed to know it, and a creator who learns it at
/// the build has already paid for an assembly.
///
/// It carries **`DW0344`**, not `DW0886`, and that is the point of asking it
/// here at all. This is the same rule the placement check states, moved one
/// stage earlier; giving the earlier statement a different number would leave a
/// creator with two codes for one fact and no way to tell they were the same
/// answer.
///
/// Measured on the shipped content library before the generators were repaired:
/// two pieces, `island-beach-camp` (`walk_y: 2`, `waterline_y: 2`) and
/// `cave-shore` (`walk_y: 1`, `waterline_y: 1`), each standing its floor level
/// with its own sea rather than one course over it.
fn walk_plane_over_waterline(base: HorizonBase, f: &PieceFacts) -> Option<Reason> {
    if base != HorizonBase::Ocean {
        return None;
    }
    let (w, wl) = (f.walk_y?, f.declared_waterline?);
    if wl == w - 1 {
        return None;
    }
    let sea = crate::compiler::horizon::SEA_LEVEL;
    let origin = crate::compiler::horizon::OCEAN_WALK_REF_Y - w;
    let lands = origin + wl;
    let delta = lands - sea;
    Some(Reason {
        shape: Shape::WalkPlaneOffItsWaterline,
        code: crate::compiler::plan::DW_OCEAN_WATERLINE,
        member: f.id.clone(),
        short: format!(
            "declares `walk_y: {w}` and `waterline_y: {wl}`; a shore stands its walk plane one \
             course above its own waterline, so this piece's water would land {n} block(s) \
             {dir} the sea",
            n = delta.abs(),
            dir = if delta > 0 { "above" } else { "below" },
        ),
        full: format!(
            "prefab `{id}` declares `walk_y: {w}` and `waterline_y: {wl}`. An ocean area's \
             origin is DERIVED from the walk plane — `{walk_ref} - {w}` = y={origin} — so this \
             piece's own top water block would stand at world y={lands}, {n} block(s) {dir} \
             this world's sea plane (y={sea}). A shore stands its walk plane exactly ONE course \
             above the water it can be climbed out of, which is `waterline_y == walk_y - 1`; \
             these two declarations are {wl} and {w}. Both numbers are measurements of the \
             piece, so the move is to the piece, not to the campaign: (1) RE-MEASURE, if either \
             number was typed rather than read off the blocks — the generator that laid them is \
             where both belong; (2) REBUILD the piece so its floor stands a course over its own \
             water, which is what the count above moves. Changing the horizon does not answer \
             it: the two declarations disagree with each other on every base, and it is only an \
             ocean that has a sea to notice",
            id = f.id,
            walk_ref = crate::compiler::horizon::OCEAN_WALK_REF_Y,
            n = delta.abs(),
            dir = if delta > 0 { "above" } else { "below" },
        ),
    })
}

/// **One area's piece set cannot be seated against two walk planes** (`DW0886`).
///
/// A property of the SET rather than of any member, and the reason it needs its
/// own function: every piece in the pool may be individually perfect and the
/// pool still unseatable, because a base whose datum is a walk plane derives ONE
/// origin per area and the solver's draw decides which member stands there.
///
/// This is the shape that made the whole round necessary. It lived in
/// [`crate::compiler::plan::area_base_y`] alone, at build tier, computed from
/// exactly the documents `delvec prefab seating` had already read and called
/// seatable — so the shipped island and cave pools passed the command and were
/// refused by the build, which is the pairing defect stated inside the one
/// mechanism built to end it. Both callers read this function now.
///
/// `members` is `(prefab id, declared walk_y)` because that is all the rule
/// needs: no bytes, no placement. `label` names what the reason is about — an
/// area at validation, a pool at the command line.
pub fn set_walk_plane(
    base: HorizonBase,
    label: &str,
    members: &[(String, Option<i32>)],
) -> SetPlane {
    if crate::compiler::horizon::walk_ref_y(base).is_none() {
        return SetPlane::NotDerived;
    }
    let mut planes: std::collections::BTreeSet<i32> = std::collections::BTreeSet::new();
    let mut silent: Vec<&str> = Vec::new();
    for (id, walk) in members {
        match walk {
            Some(w) => {
                planes.insert(*w);
            }
            None => silent.push(id.as_str()),
        }
    }
    let walk_ref = crate::compiler::horizon::OCEAN_WALK_REF_Y;
    if !silent.is_empty() {
        return SetPlane::Refused(vec![Reason {
            shape: Shape::NoWalkPlane,
            code: DW_UNSEATABLE,
            member: label.to_string(),
            short: format!(
                "{n} of {total} member(s) declare no `walk_y`, so no origin can be derived: {list}",
                n = silent.len(),
                total = members.len(),
                list = silent.join(", "),
            ),
            full: format!(
                "`{label}` is seated on a `{base}` horizon, whose datum is a WALK PLANE at \
                 y={walk_ref} — one block above the sea — so the origin is derived from the \
                 piece set's own `walk_y`. {n} of its {total} member(s) declare none: {list}. \
                 There is no default to fall back on and there deliberately is not one: a \
                 default is one tileset's authoring convention promoted to a world constant, and \
                 it is why every piece of every other library used to land with its floor under \
                 the sea. DECLARE `walk_y` on each piece named above — it is a measurement of \
                 the piece, written by the generator that built it",
                base = base.token(),
                n = silent.len(),
                total = members.len(),
                list = silent.join(", "),
            ),
        }]);
    }
    match planes.len() {
        0 => SetPlane::NotDerived,
        1 => SetPlane::Agreed(planes.iter().next().copied().expect("one plane")),
        _ => SetPlane::Refused(vec![Reason {
            shape: Shape::WalkPlanesDisagree,
            code: DW_UNSEATABLE,
            member: label.to_string(),
            short: format!(
                "its members do not agree about their own walk plane — `walk_y` values {values:?} \
                 across {total} member(s) — and one origin cannot be derived from two",
                values = planes.iter().copied().collect::<Vec<_>>(),
                total = members.len(),
            ),
            full: format!(
                "`{label}` draws from a piece set whose members do not agree about their own \
                 walk plane — `walk_y` values {values:?} across {total} member(s) — and a \
                 `{base}` horizon derives ONE origin per area from that number. Whichever the \
                 solver drew, the others would stand their walk planes at the wrong height \
                 above the sea. This is a fact about the DOCUMENTS: every member may be \
                 individually correct and the set still unseatable. The moves: (1) SPLIT the \
                 pool so each one seats pieces built to one walk plane; (2) REBUILD the odd \
                 members against the plane the rest share — a walk plane is measured off the \
                 blocks, so what moves it is the piece's own floor",
                base = base.token(),
                values = planes.iter().copied().collect::<Vec<_>>(),
                total = members.len(),
            ),
        }]),
    }
}

/// What [`set_walk_plane`] found: the number an origin derives from, a base that
/// derives none, or the reasons it cannot be derived at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetPlane {
    /// Every member agrees, and this is the plane the origin derives from.
    Agreed(i32),
    /// This base states its datum for the ORIGIN, so no piece is consulted.
    NotDerived,
    /// One origin cannot be derived from this set, and why.
    Refused(Vec<Reason>),
}

/// **Can this member be seated on this base** — every reason it cannot, in the
/// order a creator meets them (`DW0886`).
///
/// The missing walk plane comes first because it is the one that is true on
/// every base and the one everything else is derived through.
///
/// Every shape here is about ONE piece. What the set is judged on — one origin
/// per area, derived from one walk plane — is [`set_walk_plane`], and a caller
/// that judges a pool owes both.
pub fn seating_reasons(base: HorizonBase, f: &PieceFacts) -> Vec<Reason> {
    let mut out = Vec::new();
    let token = base.token();
    if f.walk_y.is_none() {
        out.push(Reason {
            shape: Shape::NoWalkPlane,
            code: DW_UNSEATABLE,
            member: f.id.clone(),
            short: "declares no `walk_y`; the piece's own walk plane is unstated".to_string(),
            full: format!(
                "prefab `{id}` declares no `walk_y`, so nothing states the piece's own walk \
                 plane. It has no default and cannot be given one: a default is one tileset's \
                 authoring convention promoted to a world constant, right for the pieces it was \
                 copied from and silently wrong for every other — the piece that lands under a \
                 sea because of it floods on boot and nothing looks. The move: DECLARE `walk_y` \
                 in `{base_file}.json`, the local y of the cell a body's feet occupy on this \
                 piece's principal floor. It is a MEASUREMENT of the piece, so it is written by \
                 the generator that built it rather than typed — this piece's own bytes stand a \
                 body at local y={measured}",
                id = f.id,
                base_file = f.base,
                measured = f
                    .lowest_standable
                    .map_or_else(|| "no cell at all".to_string(), |y| y.to_string()),
            ),
        });
    }
    if base == HorizonBase::Ocean {
        if let Some(w) = f.walk_y
            && f.standable_below_walk > 0
        {
            let lowest = f.lowest_standable.unwrap_or(w);
            let sea = crate::compiler::horizon::SEA_LEVEL;
            let origin = crate::compiler::horizon::OCEAN_WALK_REF_Y - w;
            out.push(Reason {
                shape: Shape::WadingUnderTheSea,
                code: DW_UNSEATABLE,
                member: f.id.clone(),
                short: format!(
                    "declares `walk_y: {w}` but stands a body {n} cell(s) below it, the \
                         lowest at local y={lowest} — the sea plane cuts through the piece",
                    n = f.standable_below_walk,
                ),
                full: format!(
                    "prefab `{id}` declares `walk_y: {w}`, so an ocean area seating it is \
                         placed at y={origin} and its walk plane stands dry at \
                         y={walk_ref}. But the piece's own bytes stand a body in \
                         {n} cell(s) BELOW that plane, the lowest at local y={lowest}, which \
                         lands at world y={world} — at or below this world's sea plane \
                         (y={sea}). A party would wade there. The moves: (1) RAISE the piece's \
                         low floor to its own walk plane, so no cell of it stands a body under \
                         the sea — this is a change to the piece, and it is observable: the \
                         count above is what moves; (2) DECLARE the walk plane the piece really \
                         has, `walk_y: {lowest}`, if that lower floor is the floor the party \
                         walks on — the area is then seated lower and the whole piece rises \
                         clear; (3) CHOOSE another horizon — a piece with cellars under its \
                         hall wants `void`, which puts no water anywhere, or `valley`, which \
                         buries it in terrain",
                    id = f.id,
                    walk_ref = crate::compiler::horizon::OCEAN_WALK_REF_Y,
                    n = f.standable_below_walk,
                    world = origin + lowest,
                ),
            });
        }
        // **Water in the sea's own plane**, and not merely water. A piece may
        // hold a sealed well or a cistern well above the waterline, and that
        // water never meets the sea however the piece is seated: nothing is
        // unstated about it. The plane is derivable because the origin is —
        // `origin = walk_ref - walk_y`, so a local cell lands at or below the
        // sea exactly when it is at or below `walk_y - 1`.
        let in_the_seas_plane = match (f.walk_y, f.bottom_water_y) {
            (Some(w), Some(b)) => b < w,
            _ => false,
        };
        if f.water_cells > 0 && in_the_seas_plane && f.declared_waterline.is_none() {
            out.push(Reason {
                shape: Shape::UnstatedShore,
                code: DW_UNSEATABLE,
                member: f.id.clone(),
                short: format!(
                    "authors {n} water cell(s) and declares no `waterline_y`; nothing states \
                     where it meets the sea",
                    n = f.water_cells,
                ),
                full: format!(
                    "prefab `{id}` authors {n} `{WATER}` cell(s), the topmost at local \
                     y={top}, and declares no `waterline_y`. On a horizon that has a sea, \
                     nothing then states where this piece's water meets the world's — and every \
                     downstream proof (nav, boundary, POV, PackTest) derives from the placement \
                     none of them checked. The move: DECLARE `waterline_y: {top}` in \
                     `{base_file}.json`, the local y this piece's top water block actually \
                     stands at. That number is read out of the bytes, and `DW0887` holds it to \
                     them, so a declaration copied from a tileset convention rather than \
                     measured is refused rather than believed",
                    id = f.id,
                    n = f.water_cells,
                    top = f.top_water_y.unwrap_or(0),
                    base_file = f.base,
                ),
            });
        }
        // And the converse of the shape above: a piece that states BOTH numbers
        // has stated where it meets the sea, and the two have to hold the
        // relationship a shore has. `DW0344`, asked of the documents.
        if let Some(r) = walk_plane_over_waterline(base, f) {
            out.push(r);
        }
    }
    if base == HorizonBase::Void && f.fluid_at_edge > 0 {
        out.push(Reason {
            shape: Shape::FluidOffTheWorld,
            code: DW_UNSEATABLE,
            member: f.id.clone(),
            short: format!(
                "authors fluid that runs out of {n} of its own face(s), and `{token}` puts \
                 nothing beyond them",
                n = f.fluid_at_edge,
            ),
            full: format!(
                "prefab `{id}` authors a body of fluid that reaches its own outer face in {n} \
                 run direction(s). `void` is the horizon that declares there is nothing outside \
                 the placed geometry, so a column this piece did not build is bottomless and \
                 that water runs off the world forever — which is what `DW0318` refuses once the \
                 world is assembled. The moves: (1) CHOOSE the horizon the piece was built for — \
                 a piece that authors a shore belongs to an `ocean`, which puts a sea against \
                 that face, and one authored to be buried belongs to a `valley`, which builds \
                 terrain against it; (2) PLACE something against the face, so the water meets \
                 another piece rather than the edge of the world; (3) SEAL the body inside the \
                 piece's own bytes, which is a change to the piece and is what \
                 `delvec prefab audit` reports as a run direction that leaves it",
                id = f.id,
                n = f.fluid_at_edge,
            ),
        });
    }
    out
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
}

impl Library {
    /// Read every prefab the registry holds.
    pub fn read(prefabs: &PrefabRegistry, dir: &Path) -> Library {
        let mut lib = Library {
            pieces: BTreeMap::new(),
            documents: 0,
            nbt_opened: 0,
            unreadable: Vec::new(),
        };
        for (id, meta) in prefabs.all() {
            lib.documents += 1;
            match PieceFacts::read(meta, dir) {
                Ok(f) => {
                    lib.nbt_opened += f.nbt_opened;
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
    /// Areas the campaign states.
    pub areas: usize,
    /// Areas whose piece set this examined — every one that names a prefab or a
    /// pool.
    pub areas_examined: usize,
    /// Members examined across those areas.
    pub members: usize,
    /// `.nbt` files opened.
    pub nbt_opened: usize,
    /// Waterline declarations examined among those members.
    pub waterlines_declared: usize,
    /// Of those, the ones the bytes bear out.
    pub waterlines_borne_out: usize,
}

impl SeatingBinding {
    /// **The one line this check owes its reader**, printed whether it found
    /// anything or not.
    pub fn line(&self) -> String {
        format!(
            "seating binding: horizon base `{base}`; {ex} of {areas} area(s) name a piece set, \
             examined over {members} member(s); {opened} `.nbt` opened; {wl} waterline \
             declaration(s) examined, {ok} borne out by the bytes.",
            base = self.base,
            ex = self.areas_examined,
            areas = self.areas,
            members = self.members,
            opened = self.nbt_opened,
            wl = self.waterlines_declared,
            ok = self.waterlines_borne_out,
        )
    }
}

/// **The area's own piece set**: every prefab it could seat, in a deterministic
/// order.
///
/// One place, because the origin derivation, the seating verdict and the
/// seating tool must all mean the same set — an area that draws from a pool
/// cannot know which member the solver will pick, so the whole pool is the
/// answer to "what could stand here".
pub fn area_members(area: &delvewright_dsl::Area, prefabs: &PrefabRegistry) -> Vec<String> {
    if let Some(p) = &area.prefab {
        return vec![p.as_str().to_string()];
    }
    let Some(pool) = &area.prefab_pool else {
        return Vec::new();
    };
    prefabs
        .pool(pool.as_str())
        .map(|members| {
            let mut ids: Vec<String> = members.iter().map(|m| m.prefab.clone()).collect();
            ids.sort();
            ids.dedup();
            ids
        })
        .unwrap_or_default()
}

/// **The campaign-tier refusal** — `DW0886` and `DW0887` over the pieces this
/// campaign actually seats, before anything is placed.
///
/// Returns the binding beside the diagnostics: a check that refuses nothing
/// still owes the numbers it refused nothing over.
///
/// # The one shape that is not yet raised here, and why
///
/// `walk_y` is owed on **every** base (spec-0060 §4.1), and
/// [`seating_reasons`] computes that shape on every base — `delvec prefab
/// seating` reports it for `void` and `valley` exactly as it does for `ocean`.
/// What this campaign-tier refusal raises today is the ocean's half, where the
/// area origin is DERIVED from the number and a build genuinely cannot proceed
/// without it.
///
/// The reason is named rather than hidden: the shipped content library declares
/// `walk_y` on none of its 36 documents, and that work is spec-0060 §8's, in
/// the content repository. Raising the `void` and `valley` halves before that
/// lands would refuse every campaign in the tree for a fact about a library
/// this round does not own. It is a recorded debt with one move: once the
/// content round declares the field and the content pin moves,
/// [`WALK_Y_BINDS_ON_EVERY_BASE`] becomes `true` and this filter goes.
pub fn check(
    campaign: &Campaign,
    prefabs: &PrefabRegistry,
    prefabs_dir: &Path,
) -> (SeatingBinding, Vec<Diagnostic>) {
    let base = crate::compiler::horizon::base_of(campaign);
    let mut binding = SeatingBinding {
        base: base.token().to_string(),
        areas: campaign.world.content.areas.len(),
        ..SeatingBinding::default()
    };
    let mut diags = Vec::new();
    let mut read: BTreeMap<String, Option<PieceFacts>> = BTreeMap::new();

    for area in &campaign.world.content.areas {
        let members = area_members(area, prefabs);
        if members.is_empty() {
            continue;
        }
        binding.areas_examined += 1;
        for id in &members {
            binding.members += 1;
            let facts = read.entry(id.clone()).or_insert_with(|| {
                let meta = prefabs.get(id)?;
                PieceFacts::read(meta, prefabs_dir).ok()
            });
            // A piece with no metadata is `DW0300`'s finding and a piece whose
            // bytes will not open is `DW0346`'s; both are already reported, and
            // restating them here would be a second authority for one fact.
            let Some(f) = facts.as_ref() else { continue };
            binding.nbt_opened += f.nbt_opened;
            if f.declared_waterline.is_some() {
                binding.waterlines_declared += 1;
            }
            let mut reasons = seating_reasons(base, f);
            if !WALK_Y_BINDS_ON_EVERY_BASE && base != HorizonBase::Ocean {
                reasons.retain(|r| r.shape != Shape::NoWalkPlane);
            }
            if let Some(w) = waterline_reason(f) {
                reasons.push(w);
            } else if f.declared_waterline.is_some() {
                binding.waterlines_borne_out += 1;
            }
            for r in reasons {
                diags.push(Diagnostic::error(
                    r.code,
                    "world",
                    format!("/content/areas/{}", area.id.as_str()),
                    format!(
                        "area `{area}` seats `{member}` on a `{base}` horizon, and it cannot \
                         stand there: {full}. {tail}",
                        area = area.id.as_str(),
                        member = r.member,
                        base = base.token(),
                        full = r.full,
                        tail = TAIL,
                    ),
                ));
            }
        }
        // **And the question about the SET**, which no member can answer.
        //
        // Asked here rather than only where the origin is derived, because the
        // derivation happens at build tier: this is the same rule, computed from
        // the same documents, at the stage `DW0855`'s precedent puts it — before
        // anything is placed. Until it was, `delvec prefab seating` called the
        // shipped island and cave pools seatable and the build refused them.
        let declared: Vec<(String, Option<i32>)> = members
            .iter()
            .filter_map(|id| prefabs.get(id).map(|m| (id.clone(), m.walk_y)))
            .collect();
        if let SetPlane::Refused(reasons) = set_walk_plane(base, area.id.as_str(), &declared) {
            for r in reasons {
                diags.push(Diagnostic::error(
                    r.code,
                    "world",
                    format!("/content/areas/{}", area.id.as_str()),
                    format!(
                        "area `{area}` cannot be seated on a `{base}` horizon: {full}. {tail}",
                        area = area.id.as_str(),
                        base = base.token(),
                        full = r.full,
                        tail = TAIL,
                    ),
                ));
            }
        }
    }
    (binding, diags)
}

/// Whether the missing-`walk_y` shape is raised as a campaign refusal on a base
/// that does not derive its origin from the number.
///
/// `false` is a **recorded debt**, not a decision: see [`check`]. The rule is
/// written once in [`seating_reasons`] and reported on every base by
/// `delvec prefab seating`; this constant governs only whether the campaign is
/// refused for it on `void` and `valley`.
pub const WALK_Y_BINDS_ON_EVERY_BASE: bool = false;

/// What every `DW0886` message ends with: the base a campaign cannot reach from
/// here, so that this refusal and `DW0855` read as one answer rather than two.
const TAIL: &str = "The base that builds terrain, `valley`, is not on this list of moves for a \
                    reason: it needs a statement of the whole map's extent, and a campaign that \
                    places `areas[]` by hand states none — that is `DW0855`, and its move is a \
                    site plan, which an `areas[]` campaign cannot also have (`DW0839`). So a \
                    piece authored to be buried is seated in a site-plan campaign, never in this \
                    one";

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

    /// The missing walk plane is a reason on every base — the rule is written
    /// once, whatever the campaign-tier filter does with it.
    #[test]
    fn a_missing_walk_plane_is_a_reason_on_every_base() {
        let mut f = facts();
        f.walk_y = None;
        for base in [HorizonBase::Void, HorizonBase::Ocean, HorizonBase::Valley] {
            let r = seating_reasons(base, &f);
            assert!(
                r.iter().any(|r| r.shape == Shape::NoWalkPlane),
                "{base:?} reported {r:?}"
            );
        }
    }

    /// A body standing under its own declared walk plane is refused on an
    /// ocean, and the message states where that cell lands in the world.
    #[test]
    fn a_cellar_under_the_walk_plane_is_refused_on_an_ocean() {
        let mut f = facts();
        f.walk_y = Some(3);
        f.lowest_standable = Some(1);
        f.standable_below_walk = 12;
        let r = seating_reasons(HorizonBase::Ocean, &f);
        let hit = r
            .iter()
            .find(|r| r.shape == Shape::WadingUnderTheSea)
            .expect("refused");
        assert_eq!(hit.code.id(), "DW0886");
        // origin = 63 - 3 = 60; the lowest standable cell lands at 60 + 1 = 61.
        assert!(hit.full.contains("world y=61"), "{}", hit.full);
        // The same piece on a horizon with no sea is not this finding.
        assert!(
            !seating_reasons(HorizonBase::Void, &f)
                .iter()
                .any(|r| r.shape == Shape::WadingUnderTheSea)
        );
    }

    /// Water with no declaration is refused on an ocean and the move names the
    /// number the bytes hold, so the remedy is one an author can take.
    #[test]
    fn authored_water_with_no_declaration_is_refused_on_an_ocean() {
        let mut f = facts();
        f.walk_y = Some(2);
        f.water_cells = 16;
        f.top_water_y = Some(1);
        f.bottom_water_y = Some(1);
        let r = seating_reasons(HorizonBase::Ocean, &f);
        let hit = r
            .iter()
            .find(|r| r.shape == Shape::UnstatedShore)
            .expect("refused");
        assert!(
            hit.full.contains("DECLARE `waterline_y: 1`"),
            "{}",
            hit.full
        );
    }

    /// **Water is not a shore.** A sealed well standing above the sea's own
    /// plane meets no sea however the piece is seated, so there is nothing
    /// unstated about it and nothing to declare — and a rule that refused it
    /// would be demanding a waterline that could only ever be a fiction.
    #[test]
    fn water_above_the_seas_plane_is_not_an_unstated_shore() {
        let mut f = facts();
        f.walk_y = Some(1);
        f.water_cells = 4;
        f.top_water_y = Some(5);
        f.bottom_water_y = Some(4);
        assert!(
            !seating_reasons(HorizonBase::Ocean, &f)
                .iter()
                .any(|r| r.shape == Shape::UnstatedShore),
            "a well four blocks over the waterline is not a shore"
        );
        // The same piece with its water down in the sea's plane is.
        f.bottom_water_y = Some(0);
        assert!(
            seating_reasons(HorizonBase::Ocean, &f)
                .iter()
                .any(|r| r.shape == Shape::UnstatedShore)
        );
    }

    /// Fluid that leaves the piece's own face is refused on `void` alone: it is
    /// what is beyond the face that decides, and only `void` puts nothing there.
    #[test]
    fn fluid_at_the_edge_is_refused_on_void_alone() {
        let mut f = facts();
        f.fluid_at_edge = 7;
        assert!(
            seating_reasons(HorizonBase::Void, &f)
                .iter()
                .any(|r| r.code == DW_UNSEATABLE)
        );
        assert!(seating_reasons(HorizonBase::Ocean, &f).is_empty());
    }

    /// The binding line states every denominator, on a run that found nothing.
    #[test]
    fn the_binding_line_prints_its_denominators_when_it_found_nothing() {
        let line = SeatingBinding {
            base: "void".into(),
            ..SeatingBinding::default()
        }
        .line();
        assert!(line.contains("0 of 0 area(s)"), "{line}");
        assert!(line.contains("0 `.nbt` opened"), "{line}");
        assert!(line.contains("0 borne out"), "{line}");
    }

    /// **The question no member can answer.** Every piece of a set may be
    /// individually perfect and the set still unseatable, because a base whose
    /// datum is a walk plane derives ONE origin per area.
    ///
    /// The values are the ones measured in the shipped library before the
    /// generators were repaired: `pool/island` held `[2, 3]` and
    /// `pool/cave-shore` `[1, 2]`, and both printed `SEATABLE`.
    #[test]
    fn a_set_of_two_walk_planes_cannot_be_seated_on_an_ocean() {
        let set = [
            ("prefab/beach".to_string(), Some(2)),
            ("prefab/greenfield".to_string(), Some(3)),
        ];
        let SetPlane::Refused(rs) = set_walk_plane(HorizonBase::Ocean, "pool/island", &set) else {
            panic!("two walk planes cannot derive one origin");
        };
        assert_eq!(rs.len(), 1, "one question, one answer: {rs:?}");
        assert_eq!(rs[0].shape, Shape::WalkPlanesDisagree);
        assert_eq!(rs[0].code.id(), "DW0886");
        assert!(rs[0].short.contains("[2, 3]"), "{}", rs[0].short);
        assert!(rs[0].short.contains("across 2 member(s)"), "{}", rs[0].short);
        // The same set on a base that states its datum for the ORIGIN consults
        // no piece at all, so there is nothing here to disagree about.
        for base in [HorizonBase::Void, HorizonBase::Valley] {
            assert_eq!(
                set_walk_plane(base, "pool/island", &set),
                SetPlane::NotDerived,
                "{base:?} derives no origin from a walk plane"
            );
        }
    }

    /// One plane is the number the origin derives from, and a set that states
    /// none is the missing-`walk_y` shape at set level.
    #[test]
    fn one_plane_derives_and_a_silent_set_refuses() {
        let agreed = [
            ("prefab/a".to_string(), Some(3)),
            ("prefab/b".to_string(), Some(3)),
        ];
        assert_eq!(
            set_walk_plane(HorizonBase::Ocean, "pool/x", &agreed),
            SetPlane::Agreed(3)
        );
        let silent = [
            ("prefab/a".to_string(), Some(3)),
            ("prefab/b".to_string(), None),
        ];
        let SetPlane::Refused(rs) = set_walk_plane(HorizonBase::Ocean, "pool/x", &silent) else {
            panic!("a member with no walk plane is refused");
        };
        assert_eq!(rs[0].shape, Shape::NoWalkPlane);
        assert_eq!(rs[0].code.id(), "DW0886");
        assert!(rs[0].short.contains("prefab/b"), "{}", rs[0].short);
        assert!(rs[0].short.contains("1 of 2"), "{}", rs[0].short);
        // An empty set derives nothing rather than refusing: an area that names
        // no piece set is not this rule's subject.
        assert_eq!(
            set_walk_plane(HorizonBase::Ocean, "pool/x", &[]),
            SetPlane::NotDerived
        );
    }

    /// **A shore stands its walk plane one course above its own waterline**, and
    /// the two declarations that say otherwise are refused from the DOCUMENTS,
    /// under `DW0344` — the same number the placement check uses, because it is
    /// the same fact asked one stage earlier.
    ///
    /// The values are the two pieces measured in the shipped library:
    /// `island-beach-camp` (`walk_y: 2`, `waterline_y: 2`) and `cave-shore`
    /// (`walk_y: 1`, `waterline_y: 1`).
    #[test]
    fn a_walk_plane_level_with_its_own_waterline_is_refused_from_the_documents() {
        for (walk, water) in [(2, 2), (1, 1)] {
            let mut f = facts();
            f.walk_y = Some(walk);
            f.declared_waterline = Some(water);
            f.water_cells = 9;
            f.top_water_y = Some(water);
            f.bottom_water_y = Some(water);
            let hit = seating_reasons(HorizonBase::Ocean, &f)
                .into_iter()
                .find(|r| r.shape == Shape::WalkPlaneOffItsWaterline)
                .unwrap_or_else(|| panic!("walk {walk} / waterline {water} is refused"));
            assert_eq!(hit.code.id(), "DW0344");
            assert!(hit.full.contains("1 block(s) above"), "{}", hit.full);
        }
        // The relationship a shore has is not refused, and neither is a piece
        // that declares only one of the two numbers.
        let mut ok = facts();
        ok.walk_y = Some(3);
        ok.declared_waterline = Some(2);
        ok.water_cells = 9;
        ok.top_water_y = Some(2);
        ok.bottom_water_y = Some(2);
        assert!(
            !seating_reasons(HorizonBase::Ocean, &ok)
                .iter()
                .any(|r| r.shape == Shape::WalkPlaneOffItsWaterline)
        );
        // And it is an ocean's question: a base with no sea has nothing for a
        // waterline to be off.
        let mut off = facts();
        off.walk_y = Some(2);
        off.declared_waterline = Some(2);
        off.water_cells = 9;
        off.top_water_y = Some(2);
        off.bottom_water_y = Some(2);
        for base in [HorizonBase::Void, HorizonBase::Valley] {
            assert!(
                !seating_reasons(base, &off)
                    .iter()
                    .any(|r| r.shape == Shape::WalkPlaneOffItsWaterline),
                "{base:?} has no sea plane for this to be measured against"
            );
        }
    }
}
