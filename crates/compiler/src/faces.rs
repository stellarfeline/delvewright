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
//! the same way inferring spaces would (ADR-0020 §4, "no inference").
//!
//! A face that opens onto no placed piece at all is not a finding. A delve is a
//! box garden with an outside, and a piece's front door is meant to face it.
//!
//! # Legacy pieces
//!
//! A piece that declares no contract contributes no faces, so a pair where
//! neither declares one is examined zero times — and the check reports that
//! binding count rather than a pass. It is an advisory rather than a red for
//! exactly one reason: the version-adoption rule keeps old documents compiling,
//! and every prefab in the library predates the contract. The adoption round
//! that gives them contracts is what turns this binding from zero into a number.

use delvewright_dsl::{Diagnostic, DwCode};

use crate::plan::{AreaPlacement, PlanError};
use crate::registry::PrefabRegistry;
use crate::solver::Rotation;

/// `DW0780`: two placed pieces whose declared exterior faces do not mate — a way
/// out that the piece on the other side of it does not answer.
pub const DW_FACE_MISMATCH: DwCode = DwCode::every_version("DW0780");

/// `DW0781` (advisory): no placed pair declared a face contract, so the mating
/// check examined nothing.
pub const DW_FACE_UNBOUND: DwCode = DwCode::every_version("DW0781");

/// One declared way in or out, resolved to where it actually is in the world.
#[derive(Debug, Clone)]
struct PlacedFace {
    space: String,
    class: String,
    /// Outward direction, world space.
    dir: [i32; 3],
    /// The opening's world AABB, inclusive.
    min: [i32; 3],
    max: [i32; 3],
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
        format!(
            "{} {} out of space `{}`, at x {}..{} y {}..{} z {}..{}",
            dir_name(self.dir),
            self.class,
            self.space,
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

/// One placed piece's world AABB and the faces it declares.
struct Piece {
    area: String,
    prefab: String,
    min: [i32; 3],
    max: [i32; 3],
    faces: Vec<PlacedFace>,
}

/// The verdict of the mating check: how many declared faces met another placed
/// piece, and the advisory to raise when that is zero.
#[derive(Debug)]
pub struct FaceBinding {
    /// Declared faces that abut another placed piece — the binding count.
    pub bound: usize,
    /// Declared faces in all, over every placed piece.
    pub declared: usize,
    /// Placed pieces that declare a face contract at all.
    pub contracted: usize,
}

impl FaceBinding {
    /// The advisory a zero binding owes its reader, or `None`.
    pub fn finding(&self, pieces: usize) -> Option<Diagnostic> {
        if self.bound > 0 {
            return None;
        }
        Some(Diagnostic::warning(
            DW_FACE_UNBOUND,
            "world",
            "/areas",
            format!(
                "the piece-mating check examined ZERO abutting faces: of {pieces} placed piece(s), \
                 {} declare a spatial contract and {} face(s) are declared in all, none of which \
                 touches another placed piece. Nothing here proves that the pieces of this world \
                 fit together — a piece without a contract makes no claim about its own sides, so \
                 there is nothing for a neighbour to disagree with",
                self.contracted, self.declared
            ),
        ))
    }
}

/// Refuse an assembly in which one piece's declared way out is not answered by
/// the piece on the other side of it.
pub fn check(areas: &[AreaPlacement], prefabs: &PrefabRegistry) -> Result<FaceBinding, PlanError> {
    let mut pieces: Vec<Piece> = Vec::new();
    for area in areas {
        for placement in &area.pieces {
            let (min, max) = placement.bbox();
            let mut faces = Vec::new();
            if let Some(meta) = prefabs.get(&placement.prefab_id)
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
                        class: face.class.clone(),
                        dir,
                        min: [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])],
                        max: [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])],
                    });
                }
            }
            pieces.push(Piece {
                area: area.area_id.clone(),
                prefab: placement.prefab_id.clone(),
                min,
                max,
                faces,
            });
        }
    }

    let declared: usize = pieces.iter().map(|p| p.faces.len()).sum();
    let contracted = pieces.iter().filter(|p| !p.faces.is_empty()).count();
    let mut bound = 0usize;

    for (i, piece) in pieces.iter().enumerate() {
        for face in &piece.faces {
            let axis = face.axis();
            let plane = face.beyond();
            // Which other placed piece owns the cells just beyond this opening?
            let Some((j, neighbour)) = pieces.iter().enumerate().find(|(j, other)| {
                *j != i
                    && plane >= other.min[axis]
                    && plane <= other.max[axis]
                    && (0..3).all(|a| {
                        a == axis || (face.max[a] >= other.min[a] && face.min[a] <= other.max[a])
                    })
            }) else {
                continue; // it opens onto the outside, which is what a front door does
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
                Some(g) if g.class == face.class => {}
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
                            face.class,
                            g.class,
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

    Ok(FaceBinding {
        bound,
        declared,
        contracted,
    })
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
