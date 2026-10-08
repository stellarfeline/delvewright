use super::*;

/// The hand camera's own templates (spec-0069), placed over the first area's
/// column.
pub(super) fn emit_creator_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let c = plan.campaign;
    // The hand camera (spec-0069): the creator overlay stamps an eye and a
    // rotation, and takes a body out of itself and back. Every campaign emits the
    // overlay, so every suite proves it; the PackTest server loads
    // `creator-datapack/` beside this suite for exactly these templates.
    let column = plan
        .areas
        .first()
        .map(|a| {
            let (min, _) = a.bounds();
            [min[0], min[2]]
        })
        .unwrap_or([0, 0]);
    for (path, body) in crate::compiler::creator::packtests(ns, artifact_title(c), column) {
        out.insert(path, body.into_bytes());
    }
}
