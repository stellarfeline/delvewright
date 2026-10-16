//! **The six sides of a piece, as words and as vectors.**
//!
//! One vocabulary for a piece's sides, shared rather than copied: the face
//! contract (`spatial_contract.faces`, spec-0036 §2.8) spells a direction with
//! these words, `shown_faces` spells one with the same words, and
//! `compiler::burial`, `compiler::seating`, the view's sightlines and the
//! grammar IR read them through here.
//!
//! Whether two pieces agree about the side they share is asked where their ways
//! are cut: a site plan allocates its seams on faces two boxes already share
//! and proves them over the built bytes (`DW0836`), and a detailed place
//! answers the faces its row declares (`DW0844`).

use crate::compiler::rotation::Rotation;

/// The word for an outward unit direction. Anything that is not one of the
/// other five is `north`.
pub(crate) fn dir_name(dir: [i32; 3]) -> &'static str {
    match dir {
        [1, 0, 0] => "east",
        [-1, 0, 0] => "west",
        [0, 1, 0] => "up",
        [0, -1, 0] => "down",
        [0, 0, 1] => "south",
        _ => "north",
    }
}

/// The outward unit direction a side's word names, or `None` for a word that
/// is not one of the six.
pub(crate) fn dir_vector(name: &str) -> Option<[i32; 3]> {
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

/// A local outward direction, rotated. The pivot does not matter for a
/// direction, so this is the placement's transform without the translation.
pub(crate) fn rotate_dir(rotation: Rotation, dir: [i32; 3]) -> [i32; 3] {
    rotation.transform(dir)
}
