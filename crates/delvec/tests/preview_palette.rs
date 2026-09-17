//! The CPU draft rasteriser paints every block the pinned version has.
//!
//! `delvec cameras --preview`, `delvec snapshot` and `delvec edit preview` draw
//! the assembled world with [`delvec::compiler::snapshot::block_color`]; the GPU
//! path textures the same world from the pinned client jar. A block the CPU
//! surface cannot colour comes out as the missing-texture magenta, so a creator
//! placing a camera sees a frame that judges composition and lies about
//! material — and the two surfaces disagree about a question that has one
//! answer.
//!
//! The container is the pinned block registry, enumerated here rather than
//! restated: every id 1.21.11 has, minus the states that are absence and the
//! two authoring markers whose magenta is a deliberate alarm.

use delvec::compiler::snapshot::{FALLBACK_COLOR, block_color};
use delvec::compiler::view::blockcolor::is_air;
use delvec::schem::blocks::BlockRegistry;

/// Authoring markers the solver strips before anything is drawn. Vanilla
/// deletes `jigsaw` at placement and `structure_block` is a tool, not scenery;
/// neither may reach the voxel model, and painting them the fallback is how a
/// leak announces itself. They are the only ids excluded by name.
const AUTHORING_MARKERS: &[&str] = &["minecraft:jigsaw", "minecraft:structure_block"];

#[test]
fn every_pinned_block_has_a_preview_colour() {
    let registry = BlockRegistry::v1_21_11();
    let total = registry.ids().len();
    assert!(total > 1000, "the pinned registry is the container: {total}");

    let mut air = 0usize;
    let mut markers = 0usize;
    let mut painted = 0usize;
    let mut unpainted: Vec<&str> = Vec::new();
    for id in registry.ids() {
        if is_air(id) {
            air += 1;
            continue;
        }
        if AUTHORING_MARKERS.contains(&id) {
            markers += 1;
            assert_eq!(
                block_color(id).0,
                FALLBACK_COLOR,
                "`{id}` is excluded because its magenta is the alarm; if it now has a \
                 colour the alarm is gone and the exclusion is wrong"
            );
            continue;
        }
        if block_color(id).0 == FALLBACK_COLOR {
            unpainted.push(id);
        } else {
            painted += 1;
        }
    }

    eprintln!(
        "preview palette binding: {painted} of {} paintable block(s) coloured, out of {total} in \
         the pinned registry ({air} air-like, {} authoring marker(s))",
        painted + unpainted.len(),
        markers
    );
    assert_eq!(air, 5, "the air-like states of 1.21.11");
    assert_eq!(markers, AUTHORING_MARKERS.len());
    assert!(
        unpainted.is_empty(),
        "{} of {} pinned block(s) fall to the missing-texture magenta in the CPU preview while \
         the GPU render paints them: {:?}",
        unpainted.len(),
        painted + unpainted.len(),
        unpainted
    );
}
