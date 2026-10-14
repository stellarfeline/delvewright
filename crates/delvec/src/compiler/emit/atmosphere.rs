//! Atmosphere: the bootstrap and the biomes the server holds.

use super::*;

/// The function a campaign's carried places are painted by at world setup.
pub const ATMOSPHERE_BOOTSTRAP_FN: &str = "atmosphere_bootstrap";

/// The bootstrap paint of every carried place: the biome map's place paints,
/// in its order, through the one `fillbiome` writer.
pub(super) fn atmosphere_bootstrap_lines(plan: &Plan) -> Vec<String> {
    crate::compiler::horizon::biome_map(plan)
        .places()
        .flat_map(|p| crate::compiler::atmosphere::fillbiome_lines(p.fill.0, p.fill.1, &p.biome))
        .collect()
}

/// What the atmosphere surface bound to in this build (spec-0080 §5.1).
pub fn atmosphere_binding(
    plan: &Plan,
    map: &crate::compiler::horizon::BiomeMap,
) -> crate::compiler::atmosphere::Binding {
    let c = plan.campaign;
    let places = crate::compiler::horizon::carried_places(c);
    let repaints = crate::compiler::atmosphere::set_atmospheres(c);
    let volumes: BTreeSet<([i32; 3], [i32; 3])> = repaints
        .iter()
        .filter_map(|(_, _, e)| crate::compiler::horizon::repaint_volume(plan, e))
        .collect();
    let bootstrap_cells = map
        .places()
        .map(|p| {
            (0..3)
                .map(|i| i64::from((p.cells.1[i] - p.cells.0[i] + 1) / 4))
                .product::<i64>()
        })
        .sum();
    crate::compiler::atmosphere::Binding {
        declared: c.world.content.atmospheres.len(),
        carried: places.iter().filter(|(_, a)| a.is_some()).count(),
        places: places.len(),
        repaints: repaints.len(),
        volumes: volumes.len(),
        bootstrap_cells,
        paints: map.paints.len(),
        biome_files: c.world.content.atmospheres.len(),
    }
}

/// The ground biome's datapack definition
/// ([`crate::compiler::horizon::ground_biome`]: the delve's own void or ocean
/// biome, carrying the horizon's declared audio — spec-0109 §3.2), and the void
/// biome's vanilla tag memberships ([`crate::compiler::horizon::VOID_BIOME_TAGS`]).
///
/// Every declared atmosphere ships beside it (spec-0080 §4.3), whether a place
/// carries it or only a beat paints it, because the biome registry closes when
/// the world opens; each joins the void biome's tags, as `minecraft:the_void`'s
/// own field-for-field copy.
///
/// A cutscene that declares its own music (spec-0109 §3.6) ships one variant
/// per base its camera cells can wear, under
/// `worldgen/biome/music/<digest>/<base path>.json`, each once however many
/// cutscenes share its declaration; a variant joins the void biome's tags
/// exactly when its base does, so it differs from its base in music alone.
pub(super) fn emit_ground_biome(
    plan: &Plan,
    out: &mut BuildOutput,
    variants: &[crate::compiler::music::Variant],
) {
    let ground = crate::compiler::horizon::biome_map(plan).ground;
    let ns = &plan.namespace;
    let mut tagged: Vec<String> = Vec::new();
    put_json(
        out,
        &format!("datapack/data/{ns}/worldgen/biome/{}.json", ground.path),
        &ground.definition,
    );
    if ground.void_tags {
        tagged.push(ground.id.clone());
    }
    for a in &plan.campaign.world.content.atmospheres {
        put_json(
            out,
            &crate::compiler::atmosphere::biome_path(ns, a.id.as_str()),
            &crate::compiler::atmosphere::biome_definition(a),
        );
        tagged.push(crate::compiler::atmosphere::biome_id(ns, a.id.as_str()));
    }
    let mut written: BTreeSet<&str> = BTreeSet::new();
    for v in variants {
        if !written.insert(v.id.as_str()) {
            continue;
        }
        put_json(out, &v.file, &v.definition);
        if v.void_tags {
            tagged.push(v.id.clone());
        }
    }
    if tagged.is_empty() {
        return;
    }
    for tag in crate::compiler::horizon::VOID_BIOME_TAGS {
        put_json(
            out,
            &format!("datapack/data/minecraft/tags/worldgen/biome/{tag}.json"),
            &json!({ "values": tagged }),
        );
    }
}
