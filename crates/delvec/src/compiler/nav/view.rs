//! The derived cameras of the render plan: no eye stands inside geometry
//! (`DW0724`).

use super::*;
use crate::compiler::failure::Failure;

/// One derived camera's eye, as [`verify_camera_eyes`] needs it.
///
/// Built by the derivation ([`crate::compiler::render_plan`]) from the same eye position it
/// writes into the shot's `camera`, so a shot cannot carry one camera and offer
/// the proof another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CameraEye {
    /// The shot id the camera belongs to (`seam/keep/0`, `pov/leg0/wp3`, …).
    pub shot_id: String,
    /// The shot's `kind`, so the message can say what to repair.
    pub kind: &'static str,
    /// The integer block the eye sits in (`floor` of the eye position).
    pub cell: [i32; 3],
}

/// Assert every derived camera's eye cell is clear (unoccupied) in `world` — the
/// final assembled model the shots will be rendered from. Returns
/// [`DW_CAMERA_EYE_OCCLUDED`] (`DW0724`) naming the first offending shot on
/// violation. The structural guard behind the visual tier: it is impossible to
/// ship a render plan holding a camera that looks out from inside geometry.
pub fn verify_camera_eyes(world: &World, cameras: &[CameraEye]) -> Result<(), Failure> {
    for cam in cameras {
        if world.is_clear(cam.cell) {
            continue;
        }
        let CameraEye {
            shot_id,
            kind,
            cell,
        } = cam;
        // A `pov` eye is clear by construction (1.62 over a DW0314-proven
        // standable waypoint), so the two verdicts point at different repairs and
        // must not be blurred into one sentence.
        let repair = if *kind == "pov" {
            "The eye sits at 1.62 above a proven standable waypoint, so fix the POV camera \
             derivation (eye height / standing cell) — do NOT move the waypoint or the geometry."
        } else {
            "This camera is placed at a fixed offset from authored geometry, so the finding is \
             that geometry: move what occupies the cell (a hung lantern on the centre column is \
             the recorded case), or move the anchor/seal the camera is derived from. Never nudge \
             the camera to make the picture come out."
        };
        return Err(Failure {
            code: DW_CAMERA_EYE_OCCLUDED,
            message: format!(
                "{kind} shot `{shot_id}`: the camera eye cell {cell:?} is occupied (a solid block \
                 or water) in the assembled world — the frame would render the inside of a block, \
                 not the scene. {repair}"
            ),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::nav::testkit::*;

    fn eye(shot_id: &str, kind: &'static str, cell: [i32; 3]) -> CameraEye {
        CameraEye {
            shot_id: shot_id.to_string(),
            kind,
            cell,
        }
    }

    #[test]
    fn pov_camera_in_open_air_passes_but_inside_a_block_is_dw0724() {
        // A flat floor at y=64 with headroom; the eye of a standing player is at
        // y=65..66 (clear). A camera eye in a clear cell passes; one placed inside
        // the floor block is DW0724.
        let world = floored(5, 5, 65, &[]);
        assert!(world.is_clear([2, 65, 2]), "standing eye cell is clear");
        // Clear eye → ok.
        verify_camera_eyes(&world, &[eye("pov/leg0/wp0", "pov", [2, 65, 2])])
            .expect("clear eye ok");
        // Eye buried in the solid floor → DW0724.
        let err = verify_camera_eyes(&world, &[eye("pov/leg0/wp1", "pov", [2, 64, 2])])
            .expect_err("occupied eye must fail");
        assert_eq!(err.code, DW_CAMERA_EYE_OCCLUDED);
        assert!(err.message.contains("pov/leg0/wp1"), "names the shot");
    }

    /// The widening this code exists for: the identical fact about a camera that
    /// is NOT the player's own eye. A seam camera stands one cell under the
    /// ceiling on the tile's centre column — where a hanging lantern is — and
    /// before this binding reached it the frame was one flat colour and no build
    /// in the repository said anything.
    #[test]
    fn a_seam_camera_inside_a_ceiling_block_is_dw0724_too() {
        let world = floored(5, 5, 65, &[[2, 67, 2]]);
        verify_camera_eyes(&world, &[eye("seam/keep/0", "seam", [2, 66, 2])])
            .expect("a clear seam eye passes");
        let err = verify_camera_eyes(&world, &[eye("seam/keep/0", "seam", [2, 67, 2])])
            .expect_err("a seam eye inside the hung block must fail");
        assert_eq!(err.code, DW_CAMERA_EYE_OCCLUDED);
        assert!(
            err.message.contains("seam shot `seam/keep/0`"),
            "{}",
            err.message
        );
        // The two kinds prescribe different repairs, and the message must not
        // send a seam author looking at a waypoint they do not have.
        assert!(
            !err.message.contains("waypoint"),
            "a non-pov verdict must not blame the waypoint derivation: {}",
            err.message
        );
    }
}
