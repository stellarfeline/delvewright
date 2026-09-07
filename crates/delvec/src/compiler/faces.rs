//! **Does the piece next to this one answer the way out it declares?**
//! (ADR-0020 §3, spec-0036 §2.8.)
//!
//! A prefab's `exterior` edges are its **face contract**: the sides it claims a
//! body can enter or leave by, and the opening it leaves on each. The prefab
//! checker proves that contract against the piece's own blocks. What no
//! single-piece check can see is the pair — the failure the owner names as the
//! one that costs her a review round: *pieces are approved one at a time and
//! then they do not assemble.*
//!
//! So this runs where the pieces are placed. Two placed pieces abut; one
//! declares a door on the face they share; the other declares nothing there, or
//! declares an opening somewhere else along the same wall, or declares a window
//! where the first declares a door. Each piece is individually correct. The
//! assembly is a door into a wall.
//!
//! # What is compared, and what is deliberately not
//!
//! The **declared** faces, in world coordinates, against each other. Not the
//! blocks: the blocks are the prefab checker's business, and re-deriving faces
//! from the assembled voxels here would make the mating claim unfalsifiable in
//! the same way inferring spaces would (ADR-0020 §4, "no inference"; ADR-0022 §4
//! keeps the checker on exactly that ground — declared intent against built
//! bytes).
//!
//! A face that opens onto no placed piece at all is not a finding. A delve is a
//! box garden with an outside, and a piece's front door is meant to face it.
//!
//! # The two declarations a piece makes about its own sides
//!
//! A prefab says what its sides are in **two** places, and both are declarations
//! in the prefab document — neither is inferred from a block pattern:
//!
//! 1. `spatial_contract.faces` (spec-0036 §2.8) — an `exterior` edge resolved to
//!    the side it is on, the opening it leaves there, and the **class** of way it
//!    is (`walk` / `stair` / `drop` / `barred` / `vision`).
//! 2. `connectors` (ADR-0004) — the jigsaw socket: a wall cell, an outward
//!    facing, and a `[w, h]` opening. It says *there is a way through here, this
//!    size, on this side*, and says nothing about what kind of way it is.
//!
//! Reading only the first is what this check used to do, and it is why it bound
//! to nothing on every world the jigsaw assembles. The contract block is written
//! by the grammar exporter and by `delvec prefab` admission; every hand-built
//! piece in the library predates it. The **socket**, meanwhile, is carried by
//! every piece a pool can seat — a piece with no socket can never be drawn — and
//! it is the declaration that *placed* these pieces in the first place. So the
//! check judged an assembly using a declaration it never read, reported
//! `0 with a spatial contract`, and passed. That is the constitution's *general
//! mechanism whose binding is too narrow to reach the objects it should*.
//!
//! **One authority per piece**: a piece that declares a spatial contract is
//! judged by its contract, and a piece that does not is judged by its sockets.
//! The contract is the stronger, byte-checked statement, so where a piece makes
//! both it is the one that speaks; nothing here merges them into a third thing
//! that neither document says.
//!
//! A socket declares no class, so where a seam has a socket on either side the
//! **class comparison is skipped and the geometry comparison is not**: two
//! openings must still be in the same place, the same size, at the same plane.
//! Claiming a socket is a `walk` in order to have something to compare would be
//! this module inventing a declaration, which is the same defect one level down.
//!
//! # A pair that touches and says nothing is refused
//!
//! The binding count is computed from the **placement**, which always exists,
//! and never from the declarations, which may not: the denominator is the number
//! of placed pieces whose boxes abut. A pair that abuts and across whose shared
//! plane neither piece declares anything cannot be judged at all — and a check
//! that cannot judge the thing in front of it says so as a refusal (`DW0780`)
//! naming both pieces and the plane, rather than passing quietly with a zero.
//! `DW0781` is then what it says on the tin: a world in which nothing touches
//! anything, so there was no pair to examine.
//!
//! # The layout's own claim, made falsifiable
//!
//! The solver records, per socket, whether it **mated** it — and `seal_layout`
//! clears exactly those doorways to air and walls the rest. That flag is a claim
//! about the placement the placement can contradict: a mated socket with no
//! placed piece beyond it is an open hole in a wall, shipped, with nothing
//! behind it. It is also the one contradiction a world can reach without any
//! author writing anything, because it is what *moving a mated piece* looks like
//! — the pair simply stops touching, and a check quantified over pairs that
//! touch has nothing to say about two pieces that no longer do. So the flag is
//! carried out of the solver and checked here.

use delvewright_dsl::{Diagnostic, DwCode, ExitTier};

use crate::compiler::plan::{AreaPlacement, PlanError};
use crate::compiler::registry::PrefabRegistry;
use crate::compiler::solver::{Rotation, opening_region, socket_world};

/// `DW0780`: two placed pieces whose declared exterior faces do not mate — a way
/// out that the piece on the other side of it does not answer, or a pair that
/// touches and declares nothing at all across the plane it shares.
pub const DW_FACE_MISMATCH: DwCode = DwCode::new("DW0780", ExitTier::Build);

/// `DW0781` (advisory): no two placed pieces touch, so the mating check had no
/// pair to examine.
pub const DW_FACE_UNBOUND: DwCode = DwCode::new("DW0781", ExitTier::Build);

/// Which of the piece's two declarations a [`PlacedFace`] was read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FaceSource {
    /// `spatial_contract.faces` — carries a class.
    Contract,
    /// `connectors` — a jigsaw socket, which declares an opening and a side and
    /// no class.
    Socket,
}

/// One declared way in or out, resolved to where it actually is in the world.
#[derive(Debug, Clone)]
struct PlacedFace {
    /// The space the way belongs to, for a contract face; the socket's jigsaw
    /// `name`, for a socket.
    space: String,
    /// The class of way, for a contract face. `None` for a socket, which does
    /// not declare one.
    class: Option<String>,
    source: FaceSource,
    /// Outward direction, world space.
    dir: [i32; 3],
    /// The opening's world AABB, inclusive.
    min: [i32; 3],
    max: [i32; 3],
    /// **The layout's own claim that this way joins another piece.** True only
    /// for a jigsaw socket the solver mated, which is also the socket whose
    /// doorway it clears to air rather than sealing with wall material.
    ///
    /// A face contract makes no such claim — it says a body may leave this way,
    /// not that anything is out there — so it is never `mated`, and a front door
    /// onto the outside stays what it has always been: not a finding.
    mated: bool,
}

impl PlacedFace {
    /// Which axis the face is flat in.
    fn axis(&self) -> usize {
        (0..3).find(|&a| self.dir[a] != 0).unwrap_or(0)
    }

    /// The plane one cell beyond the opening, where a neighbour's answering face
    /// would have to be.
    fn beyond(&self) -> i32 {
        let a = self.axis();
        if self.dir[a] > 0 {
            self.max[a] + 1
        } else {
            self.min[a] - 1
        }
    }

    /// `east walk, x 4..4 y 1..2 z 9..11` — where to go and look.
    fn describe(&self) -> String {
        let what = match self.source {
            FaceSource::Contract => format!(
                "{} out of space `{}`",
                self.class.as_deref().unwrap_or("way"),
                self.space
            ),
            FaceSource::Socket => format!("jigsaw socket `{}`", self.space),
        };
        format!(
            "{} {}, at x {}..{} y {}..{} z {}..{}",
            dir_name(self.dir),
            what,
            self.min[0],
            self.max[0],
            self.min[1],
            self.max[1],
            self.min[2],
            self.max[2]
        )
    }

    /// The opening's extent on the two axes it is NOT flat in — what two mating
    /// faces have to agree about.
    fn transverse(&self) -> [(i32, i32); 2] {
        let a = self.axis();
        let mut out = [(0, 0); 2];
        let mut i = 0;
        for axis in 0..3 {
            if axis != a {
                out[i] = (self.min[axis], self.max[axis]);
                i += 1;
            }
        }
        out
    }
}

fn dir_name(dir: [i32; 3]) -> &'static str {
    match dir {
        [1, 0, 0] => "east",
        [-1, 0, 0] => "west",
        [0, 1, 0] => "up",
        [0, -1, 0] => "down",
        [0, 0, 1] => "south",
        _ => "north",
    }
}

fn dir_vector(name: &str) -> Option<[i32; 3]> {
    Some(match name {
        "east" => [1, 0, 0],
        "west" => [-1, 0, 0],
        "up" => [0, 1, 0],
        "down" => [0, -1, 0],
        "south" => [0, 0, 1],
        "north" => [0, 0, -1],
        _ => return None,
    })
}

/// Where two placed boxes meet — the shared plane, derived from the placement.
struct Abutment {
    /// The axis the two boxes meet across.
    axis: usize,
    /// The FIRST piece's own boundary coordinate on that axis.
    plane: i32,
    /// Which way the first piece looks to see the second: `+1` or `-1`.
    sign: i32,
    /// The shared region's inclusive world AABB, flat in `axis`.
    min: [i32; 3],
    max: [i32; 3],
}

/// One placed piece's world AABB and the faces it declares.
struct Piece {
    area: String,
    prefab: String,
    min: [i32; 3],
    max: [i32; 3],
    faces: Vec<PlacedFace>,
    /// Whether the prefab declares a `spatial_contract` at all — which is a
    /// different question from whether it declares a FACE. A piece whose every
    /// edge joins two of its own spaces makes a complete claim about its
    /// inside and none about its sides, so it contracts and faces nothing.
    contracted: bool,
    /// **Whether this piece was placed by an ALLOCATION rather than by a
    /// mating** — that is, whether it stands in the one area a site plan has.
    ///
    /// Such a piece is never a neighbour here, and the reason is the one
    /// `FaceBinding::finding` already gives its reader in prose: a site plan
    /// cuts its ways at stage 4, on faces two boxes already share, and proves
    /// them over the built bytes with `DW0836`. Nothing in that world was ever
    /// asked to mate with anything.
    ///
    /// The distinction only started to matter at stage 6, and it matters in two
    /// ways that a narrower predicate catches only one of. A derived blockout box
    /// is a `PiecePlacement` the registry has never heard of, and the party plane
    /// a detail piece's face opens onto lies inside that box's SHELL bbox — so
    /// without this, every detailed place is refused `DW0780` for failing to mate
    /// with a shell that is not a building. And two DETAIL pieces stacked
    /// vertically mate through the horizontal party plane directly, where this
    /// check demands equal classes and spec-0050 §3's table requires `drop`
    /// leaving against `walk` landing, and `stair` in the hosting box against
    /// `walk` in the other. A `drop` between two bound places would have
    /// satisfied `DW0844` and then hard-failed here.
    ///
    /// Both are the same fact — the ways of that world are allocated — so both
    /// answer to one predicate rather than to a list.
    allocated: bool,
}

impl Piece {
    /// Where this piece's box meets `other`'s, when the two abut face to face.
    ///
    /// Computed from the placement and nothing else: no metadata, no
    /// declaration, no blocks. That is what makes it usable as a denominator —
    /// it counts objects that exist whether or not anyone declared anything
    /// about them.
    fn abutment(&self, other: &Piece) -> Option<Abutment> {
        for axis in 0..3 {
            let (plane, sign) = if self.max[axis] + 1 == other.min[axis] {
                (self.max[axis], 1)
            } else if other.max[axis] + 1 == self.min[axis] {
                (self.min[axis], -1)
            } else {
                continue;
            };
            let mut min = [0i32; 3];
            let mut max = [0i32; 3];
            let mut overlaps = true;
            for a in 0..3 {
                if a == axis {
                    min[a] = plane;
                    max[a] = plane;
                } else {
                    min[a] = self.min[a].max(other.min[a]);
                    max[a] = self.max[a].min(other.max[a]);
                    overlaps &= min[a] <= max[a];
                }
            }
            if overlaps {
                return Some(Abutment {
                    axis,
                    plane,
                    sign,
                    min,
                    max,
                });
            }
        }
        None
    }

    /// Does this piece declare a face standing on `plane` of `axis`, pointing
    /// the way `sign` points, and overlapping the shared region — the question
    /// *does this piece say anything about the side it shares with that one?*
    fn speaks_across(&self, axis: usize, plane: i32, sign: i32, at: &Abutment) -> bool {
        self.faces.iter().any(|f| {
            f.axis() == axis
                && f.dir[axis].signum() == sign
                && plane_of(f, axis) == plane
                && (0..3).all(|a| a == axis || (f.max[a] >= at.min[a] && f.min[a] <= at.max[a]))
        })
    }
}

/// The verdict of the mating check, over the denominator the placement itself
/// supplies: how many pairs of placed pieces touch, how many of those any piece
/// declared a face across, and how many declared faces met another piece.
#[derive(Debug)]
pub struct FaceBinding {
    /// Declared faces that abut another placed piece.
    pub bound: usize,
    /// Declared faces in all, over every placed piece.
    pub declared: usize,
    /// Placed pieces that declare at least one FACE, from either declaration.
    pub faced: usize,
    /// Placed pieces that declare a `spatial_contract` at all.
    ///
    /// Reported beside [`FaceBinding::faced`] rather than folded into it,
    /// because the two are different findings with different repairs: a
    /// library that predates the contract needs an adoption round, while a
    /// contracted piece with no face has made its claim and simply has no
    /// side to offer a neighbour. Collapsing them told the reader of a
    /// contracted piece that nothing declares a contract.
    pub contracted: usize,
    /// Placed pieces judged by their jigsaw sockets, because they declare no
    /// spatial contract.
    pub socketed: usize,
    /// **The denominator**: pairs of placed pieces whose boxes abut, computed
    /// from the placement alone. A world that places pieces which never touch
    /// asks this check nothing, and that is the one honest zero.
    pub pairs: usize,
    /// Placed pieces that touch at least one other placed piece — the same
    /// denominator counted per piece rather than per pair, so the line can say
    /// how much of the world was in the question at all.
    pub touching: usize,
    /// Abutting pairs across whose shared plane at least one of the two pieces
    /// declares a face. Any shortfall is refused by `DW0780` before the verdict
    /// is returned, so this equals [`FaceBinding::pairs`] whenever `check`
    /// succeeds — it is carried so the line can print both sides of the
    /// fraction rather than assert one of them.
    pub judged: usize,
}

impl FaceBinding {
    /// The same verdict as a machine-readable ledger
    /// (`validation/piece-mating.json`).
    ///
    /// `examined` is the key the gallery's coverage gate reds on when it is
    /// zero, and it is deliberately the count over the **placement** — abutting
    /// pairs — rather than over the declarations. A ledger keyed on declarations
    /// would have read `0` as an honest measurement of a library that declares
    /// nothing, which is exactly the sentence this check used to print while
    /// passing.
    #[must_use]
    pub fn to_json(&self, pieces: usize, allocated: bool) -> serde_json::Value {
        serde_json::json!({
            "placed": pieces,
            "touching": self.touching,
            "examined": self.pairs,
            "judged": self.judged,
            "faces_declared": self.declared,
            "faces_bound": self.bound,
            "by_contract": self.contracted,
            "by_socket": self.socketed,
            // Why a zero is a zero, for a reader who has only this file: a
            // site-plan world's ways are allocated at stage 4 and proved by
            // `DW0836`, so nothing in it was ever asked to mate.
            "allocated": allocated,
        })
    }

    /// **The binding line this check owes its reader on every run**, found
    /// anything or not (the vacuity rule: a count only means something when the
    /// run that found nothing prints it too).
    ///
    /// Stated as a fraction of the placement, never of the declarations: the
    /// reader's question is *did anything examine the world I just built*, and a
    /// count of declarations answers a different one.
    #[must_use]
    pub fn line(&self, pieces: usize) -> String {
        format!(
            "piece-mating binding: {} of {pieces} placed piece(s) touch in {} pair(s), {} of \
             which a declared face crosses; {} face(s) declared ({} piece(s) by spatial \
             contract, {} by jigsaw socket), {} of them abutting another piece.",
            self.touching,
            self.pairs,
            self.judged,
            self.declared,
            self.contracted,
            self.socketed,
            self.bound,
        )
    }

    /// The advisory a world with nothing to examine owes its reader, or `None`.
    ///
    /// `allocated` says the world's ways were **allocated rather than mated** —
    /// a site-plan campaign (spec-0049), whose seams are cut by the plan on
    /// faces two boxes already share and proved over the built bytes by
    /// `DW0836`. The zero is still stated, because a count only means something
    /// when the run that found nothing prints it too; what changes is that the
    /// reader is told where the question moved to instead of being told nothing
    /// proves the world fits together, which in that world is false.
    pub fn finding(&self, pieces: usize, allocated: bool) -> Option<Diagnostic> {
        if self.bound > 0 {
            return None;
        }
        let tail = if allocated {
            "This world allocates its ways in a SITE PLAN and proves them by `DW0836`, so that \
             zero is a question it does not ask."
        } else {
            "Nothing here proves that the pieces of this world fit together."
        };
        Some(Diagnostic::warning(
            DW_FACE_UNBOUND,
            "world",
            "/areas",
            format!(
                "the piece-mating check examined ZERO abutting faces: of {pieces} placed \
                 piece(s), {} with a spatial contract, {} judged by jigsaw socket, {} with a \
                 face on either, and {} face(s) declared in all — none of which touches another \
                 placed piece, because no two of these pieces touch at all ({} abutting pair(s)). \
                 {tail}",
                self.contracted, self.socketed, self.faced, self.declared, self.pairs,
            ),
        ))
    }
}

/// Refuse an assembly in which one piece's declared way out is not answered by
/// the piece on the other side of it.
pub fn check(areas: &[AreaPlacement], prefabs: &PrefabRegistry) -> Result<FaceBinding, PlanError> {
    let mut pieces: Vec<Piece> = Vec::new();
    let mut socketed = 0usize;
    for area in areas {
        for placement in &area.pieces {
            let (min, max) = placement.bbox();
            let meta = prefabs.get(&placement.prefab_id);
            let mut faces = Vec::new();
            // One authority per piece. The contract is the resolved,
            // byte-checked statement, so where a piece makes both it speaks;
            // where it makes only the older one, the socket speaks. Merging them
            // would produce a set of sides neither document declares.
            let contracted = meta.is_some_and(|m| m.spatial_contract.is_some());
            if let Some(meta) = meta
                && let Some(contract) = &meta.spatial_contract
            {
                for face in &contract.faces {
                    let Some(local_dir) = dir_vector(&face.dir) else {
                        continue;
                    };
                    // The declared side and the declared opening both turn with
                    // the placement. A face contract read without the rotation
                    // would mate a west door to a west door.
                    let dir = rotate_dir(placement.rotation, local_dir);
                    let a = world_cell(placement.rotation, placement.pos, face.opening.from);
                    let b = world_cell(placement.rotation, placement.pos, face.opening.to);
                    faces.push(PlacedFace {
                        space: face.space.clone(),
                        class: Some(face.class.clone()),
                        source: FaceSource::Contract,
                        dir,
                        min: [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])],
                        max: [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])],
                        mated: false,
                    });
                }
            } else if let Some(meta) = meta {
                // The jigsaw socket, resolved through the very functions the
                // solver mated and sealed it with — one derivation, so a face
                // this check compares cannot be in a different place from the
                // opening the world actually gets.
                for (ci, conn) in meta.connectors.iter().enumerate() {
                    let Ok((wp, facing)) = socket_world(placement.pos, placement.rotation, conn)
                    else {
                        continue;
                    };
                    let (from, to) = opening_region(wp, facing, conn.opening);
                    let u = facing.unit();
                    faces.push(PlacedFace {
                        space: conn.name.clone(),
                        class: None,
                        source: FaceSource::Socket,
                        dir: [u[0], u[1], u[2]],
                        min: from,
                        max: to,
                        mated: placement.mated.get(ci).copied().unwrap_or(false),
                    });
                }
                if !faces.is_empty() {
                    socketed += 1;
                }
            }
            pieces.push(Piece {
                area: area.area_id.clone(),
                prefab: placement.prefab_id.clone(),
                min,
                max,
                faces,
                contracted,
                allocated: area.area_id == delvewright_dsl::SITE_AREA,
            });
        }
    }

    let declared: usize = pieces.iter().map(|p| p.faces.len()).sum();
    let faced = pieces.iter().filter(|p| !p.faces.is_empty()).count();
    let contracted = pieces.iter().filter(|p| p.contracted).count();
    let mut bound = 0usize;

    for (i, piece) in pieces.iter().enumerate() {
        for face in &piece.faces {
            let axis = face.axis();
            let plane = face.beyond();
            // Which other placed piece owns the cells just beyond this opening?
            let Some((j, neighbour)) = pieces.iter().enumerate().find(|(j, other)| {
                *j != i
                    && !other.allocated
                    && plane >= other.min[axis]
                    && plane <= other.max[axis]
                    && (0..3).all(|a| {
                        a == axis || (face.max[a] >= other.min[a] && face.min[a] <= other.max[a])
                    })
            }) else {
                // Nothing is out there. For a face contract that is a front door,
                // and a box garden has an outside — not a finding.
                //
                // For a socket the LAYOUT MATED it is a contradiction, and the
                // only one in this module that a placement can produce all by
                // itself: the solver said this socket joins another piece, and
                // `seal_layout` cleared its doorway to air on the strength of
                // that, so the world ships an open hole looking at whatever
                // happens to be at that plane. Detaching a mated piece by one
                // block is exactly this shape, and until the flag reached here
                // nothing could see it — the pair simply stopped touching, and a
                // check quantified over pairs that touch has nothing to say
                // about two pieces that no longer do.
                if face.mated {
                    return Err(PlanError::new(
                        DW_FACE_MISMATCH,
                        format!(
                            "area `{}` places `{}` with a jigsaw socket the layout says is \
                             MATED, and there is no placed piece on the other side of it. `{}` \
                             declares {}; the cell(s) just beyond it at {} {} belong to no piece \
                             in this world. A mated socket is not a claim about intent — it is \
                             what `seal_layout` clears the doorway to air for, so this world \
                             ships an open hole in a wall with nothing behind it. The layout and \
                             the placement disagree about where this piece stands; do not seal \
                             the socket to hide it",
                            piece.area,
                            piece.prefab,
                            piece.prefab,
                            face.describe(),
                            ["x", "y", "z"][axis],
                            plane,
                        ),
                    ));
                }
                continue;
            };
            let _ = j;
            bound += 1;

            let opposite: Vec<&PlacedFace> = neighbour
                .faces
                .iter()
                .filter(|g| g.dir == [-face.dir[0], -face.dir[1], -face.dir[2]])
                .collect();
            let mated = opposite.iter().find(|g| {
                g.transverse() == face.transverse() && g.beyond() == plane_of(face, axis)
            });
            match mated {
                // A socket declares an opening and a side and no class, so a
                // seam with a socket on either end is compared on its geometry
                // and not on a claim neither document makes.
                Some(g) if classes_agree(face, g) => {}
                Some(g) => {
                    return Err(PlanError::new(
                        DW_FACE_MISMATCH,
                        format!(
                            "area `{}` places `{}` against area `{}`'s `{}`, and the two faces \
                             where they meet claim different things. `{}` declares {}; `{}` \
                             declares {}. A `{}` face and a `{}` face are not the same way \
                             through: one says a body crosses here and the other does not. Both \
                             pieces are individually correct; the assembly is not",
                            piece.area,
                            piece.prefab,
                            neighbour.area,
                            neighbour.prefab,
                            piece.prefab,
                            face.describe(),
                            neighbour.prefab,
                            g.describe(),
                            face.class.as_deref().unwrap_or("socket"),
                            g.class.as_deref().unwrap_or("socket"),
                        ),
                    ));
                }
                None => {
                    let offered = if opposite.is_empty() {
                        format!(
                            "area `{}`'s `{}` declares NO face on its {} side at all — the \
                             neighbour is a solid wall as far as its own contract says",
                            neighbour.area,
                            neighbour.prefab,
                            dir_name([-face.dir[0], -face.dir[1], -face.dir[2]])
                        )
                    } else {
                        format!(
                            "area `{}`'s `{}` declares {} there instead",
                            neighbour.area,
                            neighbour.prefab,
                            opposite
                                .iter()
                                .map(|g| g.describe())
                                .collect::<Vec<_>>()
                                .join("; and ")
                        )
                    };
                    return Err(PlanError::new(
                        DW_FACE_MISMATCH,
                        format!(
                            "area `{}` places `{}` with a declared way out that the piece on the \
                             other side of it does not answer. `{}` declares {}; {offered}. Two \
                             pieces reviewed one at a time are both right and still do not \
                             assemble: either give the neighbour a matching face, or place \
                             something on that side that has one",
                            piece.area,
                            piece.prefab,
                            piece.prefab,
                            face.describe(),
                        ),
                    ));
                }
            }
        }
    }

    // ---- the denominator, and the pair that cannot be judged ----------------
    //
    // Everything above quantifies over DECLARED faces, so a world whose pieces
    // declare nothing gives it nothing to do and it says so with a zero. The
    // count below quantifies over the PLACEMENT: two boxes either share a plane
    // or they do not, and no metadata is consulted to find out. That is what
    // makes the zero readable — a zero here means nothing in this world touches
    // anything, and a non-zero that nothing declared a face across is a refusal
    // rather than a quiet pass over an unexamined seam.
    let mut pairs = 0usize;
    let mut judged = 0usize;
    let mut touching = vec![false; pieces.len()];
    for i in 0..pieces.len() {
        for j in (i + 1)..pieces.len() {
            let (a, b) = (&pieces[i], &pieces[j]);
            if a.allocated || b.allocated {
                continue; // this world's ways are allocated — `DW0836` owns them
            }
            let Some(at) = a.abutment(b) else {
                continue;
            };
            pairs += 1;
            touching[i] = true;
            touching[j] = true;
            if a.speaks_across(at.axis, at.plane, at.sign, &at)
                || b.speaks_across(at.axis, at.plane + at.sign, -at.sign, &at)
            {
                judged += 1;
                continue;
            }
            return Err(PlanError::new(
                DW_FACE_MISMATCH,
                format!(
                    "area `{}` places `{}` hard against area `{}`'s `{}`, and NEITHER piece \
                     declares anything on the face they share. The two boxes meet across x \
                     {}..{} y {}..{} z {}..{}, and nothing in either prefab document says \
                     whether that is a wall or a way — so this pair is the one thing the \
                     piece-mating check exists for and the one thing it cannot judge. A piece \
                     says what its sides are in one of two places: a `spatial_contract` face \
                     (spec-0036) or a jigsaw `connectors` socket (ADR-0004). `{}` declares {} \
                     face(s) in all and none there; `{}` declares {} and none there. Give the \
                     shared side a face or a socket in one of the two prefab documents, or \
                     place the pieces apart",
                    a.area,
                    a.prefab,
                    b.area,
                    b.prefab,
                    at.min[0],
                    at.max[0],
                    at.min[1],
                    at.max[1],
                    at.min[2],
                    at.max[2],
                    a.prefab,
                    a.faces.len(),
                    b.prefab,
                    b.faces.len(),
                ),
            ));
        }
    }

    Ok(FaceBinding {
        bound,
        declared,
        faced,
        contracted,
        socketed,
        pairs,
        touching: touching.iter().filter(|t| **t).count(),
        judged,
    })
}

/// Do two mating faces agree about the KIND of way they are?
///
/// A jigsaw socket declares an opening and a side and no class. Where either end
/// of a seam is one, there is no class claim to contradict, and demanding one
/// would mean this module inventing a declaration the prefab document does not
/// make — the same defect as inferring a contract from the blocks, one level up.
/// The geometry comparison is not relaxed by this and is what still binds.
fn classes_agree(a: &PlacedFace, b: &PlacedFace) -> bool {
    match (&a.class, &b.class) {
        (Some(x), Some(y)) => x == y,
        _ => true,
    }
}

/// The plane the face itself sits in, on its own axis.
fn plane_of(face: &PlacedFace, axis: usize) -> i32 {
    if face.dir[axis] > 0 {
        face.max[axis]
    } else {
        face.min[axis]
    }
}

/// A local cell, placed and rotated.
fn world_cell(rotation: Rotation, pos: [i32; 3], local: [i32; 3]) -> [i32; 3] {
    let t = rotation.transform(local);
    [pos[0] + t[0], pos[1] + t[1], pos[2] + t[2]]
}

/// A local outward direction, rotated. The pivot does not matter for a
/// direction, so this is the same transform without the translation.
fn rotate_dir(rotation: Rotation, dir: [i32; 3]) -> [i32; 3] {
    rotation.transform(dir)
}

/// How many placed pieces there were, for the advisory's own sentence.
pub fn placed_pieces(areas: &[AreaPlacement]) -> usize {
    areas.iter().map(|a| a.pieces.len()).sum()
}
