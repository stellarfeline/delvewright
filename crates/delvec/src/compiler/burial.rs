//! **Is the outside of this piece something the player was meant to see?**
//! (`DW0885`.)
//!
//! A prefab is authored for a context. `cave-cavern` is a block of rock with a
//! hole in it: sixty-nine per cent of its box boundary is solid, and every one
//! of those cells is the cut edge of a hill it expects to be standing inside.
//! `hero-galleon-oak` is six per cent, and every one of those cells is hull the
//! player is meant to walk up to and look at. Nothing in the pair of files a
//! prefab ships as told the difference, and nothing at placement asked.
//!
//! So a `horizon: ocean` world — a superflat floor at y=54, water to y=62, and
//! sky above — laid a cave piece on the shore and buried none of it. What
//! shipped was a fifty-two cell stone-and-grass slab standing in open air over
//! the beach, with the party walking underneath it. Every other check was
//! green and correct: the sockets mated (`DW0780`), the piece's own envelope and
//! closure held (`DW0782`), the waterline met the sea (`DW0344`). Each of them
//! judges a piece against itself or against its neighbour. None of them asks the
//! question the render answered two hours later, which is whether the world the
//! piece was placed in is the world it was built for.
//!
//! # The rule
//!
//! **Every outward-facing solid boundary cell of a placed piece is buried — by
//! another piece, or by what the horizon puts there — or the piece declares that
//! side as one the player is meant to see** ([`PrefabMeta::shown_faces`]).
//!
//! Burial is read off the world the build actually writes, never off what a
//! horizon is supposed to provide. A `valley`'s ground is real blocks in real
//! templates and arrives in the assembled block map like any other piece's; an
//! `ocean`'s sea is analytic and arrives as [`nav::Ambient`]; a neighbour is a
//! box. All three are asked the same question about the same cell — *is there
//! anything there* — so a horizon that grows a surround, or a layout that moves
//! a piece, changes this verdict without changing this code.
//!
//! # Where it binds, and the one thing that bounds it
//!
//! A face nobody can be in front of is not a finding, and this is not a
//! softening — it is what stops the rule from being a rule about the inside of
//! sealed boxes. A single-room delve is a closed shell whose every boundary cell
//! is solid and exposed on all six sides, and no player will ever be outside it,
//! because there is nowhere out there to be: `DW0322` refuses a reachable
//! walkable cell over a bottomless column, and the sea is not standing room.
//!
//! So the obligation is quantified over the air **the party can be in**, flooded
//! from the cells the reachability proof already computed, through the same
//! blocks and the same ambient every other proof reads. A piece the party's own
//! air touches is judged, on every side; a piece it never reaches is examined,
//! counted, and reported as unjudged with the count. That is a positive fact
//! about the built world, which is what §0 of spec-0036 asks of any discharge —
//! a defect cannot produce it, because producing it means walling the party in.
//!
//! **A piece the party can get outside of answers for its whole outside.** Air
//! wraps: once the flood is out of a room it runs along that piece's walls, over
//! its roof and under its floor, and no line-of-sight is computed anywhere here.
//! That is deliberate rather than unfinished. A sightline model would let a
//! piece answer only for the sides the party happens to be facing, which is a
//! claim about one walk through the delve and not about the building — and the
//! creator moving a waypoint would move what the piece has to be. Being outside
//! a thing at all is the fact this reads, and it is a fact about the world.
//!
//! **The named residual, with its number in the line.** The flood is bounded by
//! [`SKIN`], so a piece standing well clear of every other piece can be exposed
//! and go unjudged: the party's air runs out of domain before it gets there.
//! That direction is under-marking, and it is stated rather than hidden — the
//! binding line and `validation/piece-exposure.json` carry `exposed` beside
//! `judged`, so the gap between them is the size of what this did not ask.
//!
//! # The three moves, and which ones a horizon leaves open
//!
//! The refusal owes the author a move, as `DW0344` does. There are three, and
//! which of them exist is a property of the horizon:
//!
//! * **Bury it with terrain.** Only `valley` builds any (spec-0026), so this
//!   move exists there and nowhere else — under `ocean` the sea buries what
//!   stands below y=62 and nothing above it, and `void` is by definition a world
//!   with nothing outside the placed geometry.
//! * **Place something against it.** Available under every horizon: a face whose
//!   outward cells fall inside another placed piece's box is buried by that
//!   piece, which is also the arrangement `DW0780` then holds to a face contract.
//! * **Declare the side shown.** Available under every horizon, and the only
//!   move for a piece that really is meant to be looked at. It is a claim about
//!   the PIECE, made in the prefab document, so it changes what that piece is in
//!   every world — which is the cost that keeps it from being the cheap answer.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use delvewright_dsl::{Diagnostic, DwCode, ExitTier, PrefabMeta};

use crate::compiler::faces::{dir_name, dir_vector, rotate_dir};
use crate::compiler::nav::{Ambient, World};
use crate::compiler::plan::Plan;
use crate::compiler::registry::PrefabRegistry;

/// `DW0885`: a placed piece's outward solid boundary stands in the air the party
/// can be in, and neither the world nor the piece answers for it.
pub const DW_PIECE_EXPOSED: DwCode = DwCode::new("DW0885", ExitTier::Build);

/// How far outward of the content the party's air is followed before the flood
/// is called an escape.
///
/// Two, not one, and the reason is that one is the very cell this check asks
/// about: a face's outward neighbour is at distance 1 from its own box, so a
/// domain that stopped there would make every exposed cell a rim cell and every
/// world outdoors. Two is the first distance that is about the air rather than
/// about the face — the same one-cell-skin device `assembled::flood` uses to
/// keep a fluid inside the content, taken one cell further out so the skin
/// itself can be crossed.
const SKIN: i32 = 2;

/// One placed piece, as this check sees it: a box, a document, and the sides
/// that document says are finished surface.
struct Piece {
    area: String,
    prefab: String,
    min: [i32; 3],
    max: [i32; 3],
    /// The declared shown sides, already turned by the placement — world
    /// directions, so nothing downstream has to remember the rotation.
    shown: Vec<[i32; 3]>,
    /// What the document actually wrote, in its own words, for the message.
    shown_local: Vec<String>,
    /// **Is there a document behind this box at all?**
    ///
    /// A site plan's massing shells and its derived blockout volumes are
    /// `PiecePlacement`s the prefab registry has never heard of — the compiler
    /// derived them from the plan, and there is no file in any library for an
    /// author to write a `shown_faces` in. A piece that cannot declare cannot be
    /// asked to, so those boxes are counted, they bury what stands against them,
    /// and the obligation passes over them; `DW0836`/`DW0837`/`DW0838` are what
    /// judge a derivation against the plan it came from (spec-0049 §5.3).
    ///
    /// This is not an opt-out a defect can reach for. A missing document on a
    /// piece the campaign BOUND is `DW0300`, refused upstream before this check
    /// runs, so the only way to be here undocumented is to be a volume the
    /// compiler itself produced.
    documented: bool,
    /// **Which of the piece's six sides its own bytes put a block on**, world
    /// directions, from the templates the placement carries rather than from the
    /// assembled map.
    ///
    /// The distinction is the whole of arm 2. A campaign may carve a doorway
    /// through a wall after the piece lands, and the assembled world then shows
    /// a side of pure air where the ASSET has a wall — so a demand read off the
    /// assembled map would make a prefab's own declaration depend on what every
    /// campaign does to it. This is read off the piece.
    own_sides: BTreeSet<[i32; 3]>,
}

impl Piece {
    /// Is `c` inside this piece's box?
    fn holds(&self, c: [i32; 3]) -> bool {
        (0..3).all(|a| c[a] >= self.min[a] && c[a] <= self.max[a])
    }

    /// Chebyshev distance from `c` to this box: 0 inside it, 1 for a cell that
    /// touches a face, and so on.
    fn distance(&self, c: [i32; 3]) -> i32 {
        (0..3)
            .map(|a| (self.min[a] - c[a]).max(c[a] - self.max[a]).max(0))
            .max()
            .unwrap_or(0)
    }
}

/// What this check examined, stated on every build whether it found anything or
/// not — the vacuity rule: a count only means something when the run that found
/// nothing prints it too.
///
/// The denominator is taken from the **placement and the bytes**, never from the
/// declarations. `boundary` is how many solid cells the placed pieces put on
/// their own box boundaries, which is a fact about geometry no missing
/// `shown_faces` can shrink; `judged` is how many of those the party's air
/// actually reaches. A build whose `judged` is zero has not passed this check,
/// it has not been asked it, and the line says so in those words.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExposureBinding {
    /// Placed pieces in the world.
    pub placed: usize,
    /// Placed pieces whose bytes reached the assembled map at all.
    pub examined: usize,
    /// Solid cells standing on a placed piece's own box boundary.
    pub boundary: usize,
    /// Of those, the ones whose outward neighbour is neither block, box nor
    /// ambient — the cells with nothing in front of them.
    pub exposed: usize,
    /// Of the exposed, the ones the party's air reaches — the binding count.
    pub judged: usize,
    /// `(piece, side)` pairs carrying at least one judged cell.
    pub sides_seen: usize,
    /// `shown_faces` entries over the placed pieces.
    pub declared: usize,
    /// Declared sides that carry a judged cell — a declaration bound to
    /// something the party can see.
    pub bound: usize,
    /// Cells of air the party's flood filled — the size of the thing that
    /// decided which faces were judged, so a `judged: 0` can be read against it.
    pub air: usize,
    /// Whether that flood reached the outer rim of the examined skin, which is
    /// where it was cut off rather than where it ended.
    pub cut_off: bool,
    /// The **ambient**'s own name, so the line can say which moves exist.
    pub horizon: &'static str,
    /// **The base the campaign declared**, beside the ambient it resolves to.
    ///
    /// `valley` resolves to a `void` ambient — its ground is placed blocks
    /// rather than a generator fact — so a line keyed to the ambient alone said
    /// ``horizon `void``` about a world whose author wrote `valley`, and a
    /// reader cannot tell an unbound check from a mislabelled one
    /// (spec-0060 §10.7).
    pub base: &'static str,
}

impl ExposureBinding {
    /// **The one line this check owes its reader on every build.**
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "piece-exposure binding: horizon base `{}` (ambient `{}`); {} of {} placed piece(s) \
             examined, putting {} \
             solid cell(s) on their own box boundaries, of which {} have nothing in front of them \
             and {} stand in air the party can be in ({}); {} side(s) of a piece are seen that \
             way, against {} `shown_faces` declaration(s) of which {} are bound.",
            self.base,
            self.horizon,
            self.examined,
            self.placed,
            self.boundary,
            self.exposed,
            self.judged,
            if self.cut_off {
                format!(
                    "{} cell(s) of party air, cut off at the {SKIN}-cell skin",
                    self.air
                )
            } else {
                format!("{} cell(s) of party air, wholly enclosed", self.air)
            },
            self.sides_seen,
            self.declared,
            self.bound,
        )
    }

    /// The same verdict as a machine-readable ledger
    /// (`validation/piece-exposure.json`).
    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "horizon_base": self.base,
            "horizon": self.horizon,
            "placed": self.placed,
            "examined": self.examined,
            "boundary_cells": self.boundary,
            "exposed_cells": self.exposed,
            "judged_cells": self.judged,
            "sides_seen": self.sides_seen,
            "faces_declared": self.declared,
            "faces_bound": self.bound,
            "party_air_cells": self.air,
            "party_air_cut_off": self.cut_off,
        })
    }
}

/// The moves this horizon leaves an author, named in the refusal.
fn moves(horizon: &str) -> String {
    let bury = match horizon {
        "valley" => {
            "(1) BURY it — this world's `valley` horizon builds terrain (spec-0026), so \
                     moving the piece down into the landform, or widening the region the surround \
                     rings, puts ground against this face"
        }
        "ocean" => {
            "(1) BURY it — this world's `ocean` horizon puts sea to y=62 and a floor at \
                    y=54, so a face BELOW the sea plane is buried by the water itself; above it \
                    the sea has nothing to offer and this move does not exist. `valley` is the \
                    one base that builds terrain"
        }
        _ => {
            "(1) BURY it — not under this horizon. `void` is the horizon that declares there is \
              nothing outside the placed geometry, so it can bury nothing; `valley` is the one \
              base that builds terrain (spec-0026)"
        }
    };
    format!(
        "{bury}; (2) PLACE something against it — a face whose outward cells fall inside another \
         placed piece's box is buried by that piece, and the two then answer to each other's face \
         contract (`DW0780`); (3) DECLARE the side shown — add it to `shown_faces` in the prefab \
         document, which is the piece saying this side is finished exterior surface. Do not \
         declare a side the piece was built to have covered: the slab stays in the sky and the \
         only thing that changes is that nothing says so"
    )
}

/// Refuse a world that stands a piece's unfinished outside where the party can
/// see it.
///
/// `blocks` is the assembled, gravity-settled cell→block map — the world the
/// build writes — and `reachable` the standing cells the reachability proof
/// already computed. Both are arguments rather than things this derives, so this
/// check and every proof around it are reading one world.
pub fn check(
    plan: &Plan,
    prefabs: &PrefabRegistry,
    blocks: &BTreeMap<[i32; 3], String>,
    structures: &BTreeMap<String, Vec<u8>>,
    world: &World,
    reachable: &BTreeSet<[i32; 3]>,
) -> (ExposureBinding, Vec<Diagnostic>) {
    let ambient = Ambient::of_plan(plan);
    let mut binding = ExposureBinding {
        horizon: ambient.name(),
        base: world.base(),
        ..ExposureBinding::default()
    };
    let _ = world;

    // ---- the pieces, and what each of them says about its own sides ---------
    let mut pieces: Vec<Piece> = Vec::new();
    for area in &plan.areas {
        for placement in &area.pieces {
            binding.placed += 1;
            let (min, max) = placement.bbox();
            let meta: Option<&PrefabMeta> = prefabs.get(&placement.prefab_id);
            let mut shown = Vec::new();
            let mut shown_local = Vec::new();
            let documented = meta.is_some();
            if let Some(meta) = meta {
                for side in &meta.shown_faces {
                    let Some(local) = dir_vector(side) else {
                        return (
                            binding,
                            vec![Diagnostic::error(
                                DW_PIECE_EXPOSED,
                                "world",
                                "/areas",
                                format!(
                                    "prefab `{}` declares `shown_faces: [\"{side}\"]`, and `{side}` is \
                                 not a side of a piece. A side is one of `east`, `west`, `up`, \
                                 `down`, `north`, `south` — the same six words the face contract \
                                 spells a direction with (spec-0036 §2.8). Nothing can be judged \
                                 against a side that does not exist, so this is refused where it \
                                 is written rather than counted as a declaration that binds to \
                                 nothing",
                                    placement.prefab_id,
                                ),
                            )],
                        );
                    };
                    shown.push(rotate_dir(placement.rotation, local));
                    shown_local.push(side.clone());
                    binding.declared += 1;
                }
            }
            let own_sides = own_solid_sides(placement, structures, min, max);
            pieces.push(Piece {
                area: area.area_id.clone(),
                prefab: placement.prefab_id.clone(),
                min,
                max,
                shown,
                shown_local,
                documented,
                own_sides,
            });
        }
    }
    if pieces.is_empty() {
        return (binding, Vec::new());
    }

    // ---- the air the party can be in ---------------------------------------
    let (air, cut_off) = party_air(&pieces, blocks, &ambient, reachable);
    binding.air = air.len();
    binding.cut_off = cut_off;

    // Every piece is walked, and every finding collected. Refusing at the first
    // one would tell an author to repair a face and hand them the next on the
    // following run: one placement defect is routinely several pieces, and a
    // report naming one sends a creator round the loop as many times as the
    // world has walls (the shape `blockout::check` already fixed for its own
    // battery). The caller raises the first and prints the rest.
    let mut findings: Vec<Diagnostic> = Vec::new();

    // ---- every outward-facing solid boundary cell, one piece at a time ------
    //
    // The walk is over the six SIDES of each box rather than over the box's
    // whole shell, because the verdict is per side: an author repairs a face,
    // not a cell, and the declaration is spelled in sides.
    for piece in &pieces {
        if !piece.documented {
            continue;
        }
        let mut examined_this = false;
        // Judged cells per world side, in the six-direction order `dir_vector`
        // spells, so the report is deterministic.
        let mut judged_by_side: BTreeMap<[i32; 3], usize> = BTreeMap::new();
        let mut witness: BTreeMap<[i32; 3], [i32; 3]> = BTreeMap::new();
        for axis in 0..3 {
            for sign in [-1i32, 1] {
                let mut dir = [0i32; 3];
                dir[axis] = sign;
                let plane = if sign > 0 {
                    piece.max[axis]
                } else {
                    piece.min[axis]
                };
                let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
                for cu in piece.min[u]..=piece.max[u] {
                    for cv in piece.min[v]..=piece.max[v] {
                        let mut cell = [0i32; 3];
                        cell[axis] = plane;
                        cell[u] = cu;
                        cell[v] = cv;
                        if !is_solid(blocks, cell) {
                            continue;
                        }
                        examined_this = true;
                        binding.boundary += 1;
                        let out = [cell[0] + dir[0], cell[1] + dir[1], cell[2] + dir[2]];
                        if covered(&pieces, blocks, &ambient, out) {
                            continue;
                        }
                        binding.exposed += 1;
                        if !air.contains(&out) {
                            continue;
                        }
                        binding.judged += 1;
                        *judged_by_side.entry(dir).or_default() += 1;
                        witness.entry(dir).or_insert(cell);
                    }
                }
            }
        }
        if examined_this {
            binding.examined += 1;
        }
        binding.sides_seen += judged_by_side.len();

        // (1) A side the party can see that the piece does not answer for.
        //
        // Quantified over the sides the party's air actually reaches. A piece
        // that air never gets outside of contributes nothing here and is
        // reported in the line as examined-not-judged: a side nobody can be in
        // front of is neither shown nor hidden.
        for (dir, cells) in &judged_by_side {
            if piece.shown.contains(dir) {
                binding.bound += 1;
                continue;
            }
            let at = witness[dir];
            let horizon = binding.horizon;
            findings.push(Diagnostic::error(
                DW_PIECE_EXPOSED,
                "world",
                "/areas",
                format!(
                    "area `{area}` places `{prefab}` with its {side} side standing in open air \
                     the party can be in, and nothing answers for it. {cells} solid cell(s) of \
                     that side have neither a piece, a block nor this horizon's own ground in \
                     front of them — the first is at [{x}, {y}, {z}] — and `{prefab}` declares \
                     {declared}. An outward face is either something the world covers or \
                     something the player was meant to look at, and a piece authored to stand \
                     inside a hill says neither, which is how a stone cap ends up hanging in the \
                     sky over a beach. The moves: {moves}",
                    area = piece.area,
                    prefab = piece.prefab,
                    side = dir_name(*dir),
                    cells = cells,
                    x = at[0],
                    y = at[1],
                    z = at[2],
                    declared = declared_phrase(piece),
                    moves = moves(horizon),
                ),
            ));
        }

        // (2) A side declared finished that has nothing on it to finish.
        //
        // This is the arm that keeps `shown_faces` from being free typing, and
        // it is deliberately NOT "you declared a side something is standing
        // against". A finished wall stays finished when a shed is put in front
        // of it: the declaration is a claim about the PIECE, and burial is a
        // fact about one placement, so refusing their coincidence would make a
        // piece's own document depend on every world it is ever seated in.
        //
        // What a piece cannot do is finish a side that is not there. A face of
        // pure air carries no surface to be exterior, so a declaration over it
        // is a claim about nothing — and unlike the placement, this is a fact
        // about the bytes, which is the kind of demand §0 of spec-0036 asks an
        // opt-out to satisfy.
        for (local, dir) in piece.shown_local.iter().zip(&piece.shown) {
            if piece.own_sides.contains(dir) {
                continue;
            }
            findings.push(Diagnostic::error(
                DW_PIECE_EXPOSED,
                "world",
                "/areas",
                format!(
                    "area `{area}` places `{prefab}`, whose prefab document declares \
                     `shown_faces: [\"{local}\"]`, and that side of the piece is empty: not one \
                     cell of its own {side} face holds a block. A side is finished exterior \
                     surface or it is nothing, and there is nothing here to be finished — so the \
                     declaration says something about a face this piece does not have. Drop it \
                     from `shown_faces`, or build the side it claims",
                    area = piece.area,
                    prefab = piece.prefab,
                    side = dir_name(*dir),
                ),
            ));
        }
    }

    (binding, findings)
}

/// `declares no side shown` / ``declares `up`, `north` shown`` — what the piece
/// document says, for a reader who has only the message.
fn declared_phrase(piece: &Piece) -> String {
    if piece.shown_local.is_empty() {
        return "no `shown_faces` at all, so no side of it is meant to be seen".to_string();
    }
    format!(
        "`shown_faces: [{}]`",
        piece
            .shown_local
            .iter()
            .map(|s| format!("\"{s}\""))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// Is there a block in this cell?
fn is_solid(blocks: &BTreeMap<[i32; 3], String>, c: [i32; 3]) -> bool {
    blocks.get(&c).is_some_and(|b| b != "minecraft:air")
}

/// **Is there anything in front of this face?** — asked of the one world the
/// build writes, with the three things that can answer given equal standing.
fn covered(
    pieces: &[Piece],
    blocks: &BTreeMap<[i32; 3], String>,
    ambient: &Ambient,
    c: [i32; 3],
) -> bool {
    if is_solid(blocks, c) {
        return true; // a neighbour's block, a surround's terrain, an edit's fill
    }
    if pieces.iter().any(|p| p.holds(c)) {
        // Inside another piece's box: whatever is in there is that piece's
        // business, and the seam between the two is `DW0780`'s.
        return true;
    }
    match ambient {
        // The generator's own columns. Water and sea floor alike bury a face —
        // a wall under the sea is a wall in the sea, not a wall in the sky.
        Ambient::Ocean(sea) => c[1] <= sea.level,
        Ambient::Void => false,
    }
}

/// **Which of a placed piece's six sides its own templates put a block on.**
///
/// Read from the same bytes, through the same rotation and the same per-template
/// offset `assembled::placed_blocks` uses, so a tiled piece is composed back into
/// the one box it was cut out of and a rotated piece's sides turn with it.
fn own_solid_sides(
    placement: &crate::compiler::plan::PiecePlacement,
    structures: &BTreeMap<String, Vec<u8>>,
    min: [i32; 3],
    max: [i32; 3],
) -> BTreeSet<[i32; 3]> {
    let mut sides = BTreeSet::new();
    for template in &placement.templates {
        let Some(bytes) = structures.get(&template.structure_file) else {
            continue;
        };
        for (local, name) in crate::compiler::assembled::structure_named_cells(bytes) {
            if name == "minecraft:air" {
                continue;
            }
            let t = placement.rotation.transform(local);
            let cell = [
                template.pos[0] + t[0],
                template.pos[1] + t[1],
                template.pos[2] + t[2],
            ];
            for axis in 0..3 {
                if cell[axis] == min[axis] {
                    let mut d = [0i32; 3];
                    d[axis] = -1;
                    sides.insert(d);
                }
                if cell[axis] == max[axis] {
                    let mut d = [0i32; 3];
                    d[axis] = 1;
                    sides.insert(d);
                }
            }
        }
    }
    sides
}

/// The air the party can be in, and whether the flood was cut off by [`SKIN`].
///
/// Flooded from the cells the reachability proof standing on, through cells that
/// hold no block and that the horizon does not fill, confined to the content and
/// its skin ([`SKIN`]). The confinement is what makes the flood finite in an
/// open-sky world; the escape is what makes it honest, because a flood that
/// reaches the rim has not been contained, it has been cut off.
fn party_air(
    pieces: &[Piece],
    blocks: &BTreeMap<[i32; 3], String>,
    ambient: &Ambient,
    reachable: &BTreeSet<[i32; 3]>,
) -> (BTreeSet<[i32; 3]>, bool) {
    let distance = |c: [i32; 3]| {
        pieces
            .iter()
            .map(|p| p.distance(c))
            .min()
            .unwrap_or(i32::MAX)
    };
    let passable = |c: [i32; 3]| {
        if is_solid(blocks, c) {
            return false;
        }
        match ambient {
            Ambient::Ocean(sea) => pieces.iter().any(|p| p.holds(c)) || c[1] > sea.level,
            Ambient::Void => true,
        }
    };
    let mut seen: BTreeSet<[i32; 3]> = BTreeSet::new();
    let mut queue: VecDeque<[i32; 3]> = VecDeque::new();
    let mut cut_off = false;
    // The body, not the foot: a standing cell is where the feet are, and what a
    // player is IN is that cell and the one over their head. Seeding both is what
    // lets the flood leave a room through a doorway whose sill is a step up.
    for cell in reachable {
        for dy in 0..=1 {
            let c = [cell[0], cell[1] + dy, cell[2]];
            if distance(c) <= SKIN && passable(c) && seen.insert(c) {
                queue.push_back(c);
            }
        }
    }
    while let Some(c) = queue.pop_front() {
        if distance(c) >= SKIN {
            // The flood reached the outer rim of the examined skin: it did not
            // end here, it was stopped here, and the line says so.
            cut_off = true;
        }
        for axis in 0..3 {
            for sign in [-1i32, 1] {
                let mut n = c;
                n[axis] += sign;
                if distance(n) <= SKIN && passable(n) && seen.insert(n) {
                    queue.push_back(n);
                }
            }
        }
    }
    (seen, cut_off)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn piece(min: [i32; 3], max: [i32; 3], shown: &[&str]) -> Piece {
        Piece {
            area: "area/a".into(),
            prefab: "prefab/p".into(),
            min,
            max,
            shown: shown
                .iter()
                .map(|s| dir_vector(s).expect("a side"))
                .collect(),
            shown_local: shown.iter().map(|s| (*s).to_string()).collect(),
            documented: true,
            own_sides: [
                [-1, 0, 0],
                [1, 0, 0],
                [0, -1, 0],
                [0, 1, 0],
                [0, 0, -1],
                [0, 0, 1],
            ]
            .into_iter()
            .collect(),
        }
    }

    /// The distance a flood is confined by is the box's own, and a face's
    /// outward neighbour is inside it — which is the whole reason [`SKIN`] is
    /// two and not one.
    #[test]
    fn a_faces_outward_neighbour_is_never_the_rim() {
        let p = piece([0, 0, 0], [4, 4, 4], &[]);
        assert_eq!(p.distance([2, 2, 2]), 0, "inside");
        assert_eq!(p.distance([5, 2, 2]), 1, "the cell a face looks at");
        assert!(
            p.distance([5, 2, 2]) < SKIN,
            "so it is never the rim itself"
        );
        assert_eq!(p.distance([6, 2, 2]), SKIN, "the rim");
    }

    /// A neighbour's box buries a face whatever is inside it, and the sea buries
    /// what stands under it. Both are the same question asked of one cell.
    #[test]
    fn a_box_and_a_sea_both_bury() {
        let pieces = vec![
            piece([0, 0, 0], [4, 4, 4], &[]),
            piece([5, 0, 0], [9, 4, 4], &[]),
        ];
        let blocks = BTreeMap::new();
        assert!(
            covered(&pieces, &blocks, &Ambient::Void, [5, 2, 2]),
            "the neighbour's box"
        );
        assert!(
            !covered(&pieces, &blocks, &Ambient::Void, [-1, 2, 2]),
            "the void covers nothing"
        );
        let sea = Ambient::Ocean(crate::compiler::nav::Sea {
            level: 62,
            floor_top: 54,
        });
        assert!(
            covered(&pieces, &blocks, &sea, [-1, 60, 2]),
            "under the sea"
        );
        assert!(
            !covered(&pieces, &blocks, &sea, [-1, 63, 2]),
            "over the sea is sky"
        );
    }

    /// The line states every number, and says which way the zero reads.
    #[test]
    fn the_line_states_what_was_examined() {
        let b = ExposureBinding {
            placed: 4,
            examined: 4,
            boundary: 1200,
            exposed: 300,
            judged: 0,
            air: 90,
            horizon: "void",
            base: "void",
            ..ExposureBinding::default()
        };
        let line = b.line();
        assert!(line.contains("4 of 4 placed piece(s) examined"), "{line}");
        assert!(line.contains("1200 solid cell(s)"), "{line}");
        assert!(line.contains("90 cell(s) of party air"), "{line}");
        assert!(line.contains("wholly enclosed"), "{line}");
    }

    /// Every horizon names a bury move or says in as many words that it has
    /// none — the diagnostic owes the author a move, and "not under this
    /// horizon" is a move's absence stated rather than a gap.
    #[test]
    fn every_horizon_says_what_it_can_bury_with() {
        for h in ["void", "ocean", "valley"] {
            let m = moves(h);
            assert!(m.contains("PLACE something against it"), "{h}: {m}");
            assert!(m.contains("DECLARE the side shown"), "{h}: {m}");
            assert!(m.contains("BURY it"), "{h}: {m}");
        }
        assert!(moves("void").contains("nothing outside the placed geometry"));
        assert!(moves("ocean").contains("y=62"));
        assert!(moves("valley").contains("builds terrain"));
    }
}
