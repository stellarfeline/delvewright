//! Atmosphere: the bootstrap, the repaint plan and the ground biome the server holds.

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

/// The bot tier's repaint contract (spec-0080 §5.2), or `None` when the
/// campaign repaints nothing.
///
/// `after` is the completion-marker token the bundle that fires the repaint
/// broadcasts — an objective's `obj/<id>`, or a trigger's own `trigger/<id>`
/// when it carries one ([`plan::trigger_may_be_performed`]) — and `null` for a
/// repaint nested in a later `sequence` step, gated by `when`, or fired from a
/// root no marker announces: the bot cannot tell when those fire, and the file
/// says so rather than guessing.
pub(super) fn atmosphere_repaint_plan(plan: &Plan) -> Option<serde_json::Value> {
    let c = plan.campaign;
    let map = crate::compiler::horizon::biome_map(plan);
    let mut rows = Vec::new();
    let marked_triggers: BTreeSet<&str> = c
        .quests
        .content
        .triggers
        .iter()
        .filter(|t| plan::trigger_may_be_performed(t))
        .map(|t| t.id.as_str())
        .collect();
    delvewright_dsl::for_each_campaign_effect(c, &mut |path, site, e| {
        if !matches!(e.verb, Verb::SetAtmosphere { .. }) {
            return;
        }
        let Some((min, max)) = crate::compiler::horizon::repaint_volume(plan, e) else {
            return;
        };
        let Verb::SetAtmosphere { atmosphere, .. } = &e.verb else {
            return;
        };
        // A nested list fires at a moment its root's marker does not mark: a
        // `sequence` step at its offset, an arrival, a rest, a respawn, a catch.
        let top_level = ![
            "/steps/",
            "/on_respawn/",
            "/on_rest/",
            "/on_caught/",
            "/on_arrive/",
        ]
        .iter()
        .any(|seg| path.contains(seg));
        let after = if e.when.is_some() || !top_level {
            None
        } else {
            match site {
                delvewright_dsl::EffectSite::Objective { objective, .. } => Some(objective.clone()),
                delvewright_dsl::EffectSite::Trigger { trigger }
                    if marked_triggers.contains(trigger.as_str()) =>
                {
                    Some(trigger.clone())
                }
                _ => None,
            }
        };
        let (lo, hi) = crate::compiler::atmosphere::painted_box(min, max);
        let mut chunks = Vec::new();
        for cx in lo[0].div_euclid(16)..=hi[0].div_euclid(16) {
            for cz in lo[2].div_euclid(16)..=hi[2].div_euclid(16) {
                chunks.push(json!([cx, cz]));
            }
        }
        rows.push(json!({
            "effect": path,
            "biome": map.biome_of(atmosphere.as_ref().map(|a| a.as_str())),
            "after": after,
            "chunks": chunks,
        }));
    });
    (!rows.is_empty()).then(|| json!({ "repaints": rows }))
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

/// The ground biome's datapack definition, when the delve ships its own
/// ([`crate::compiler::horizon::ground_biome`]), and its vanilla tag
/// memberships ([`crate::compiler::horizon::VOID_BIOME_TAGS`]).
///
/// Every declared atmosphere ships beside it (spec-0080 §4.3), whether a place
/// carries it or only a beat paints it, because the biome registry closes when
/// the world opens; each joins the void biome's tags, as `minecraft:the_void`'s
/// own field-for-field copy.
pub(super) fn emit_ground_biome(plan: &Plan, out: &mut BuildOutput) {
    let ground = crate::compiler::horizon::biome_map(plan).ground;
    let ns = &plan.namespace;
    let mut tagged: Vec<String> = Vec::new();
    if let Some(definition) = &ground.definition {
        let path = crate::compiler::horizon::VOID_BIOME_PATH;
        put_json(
            out,
            &format!("datapack/data/{ns}/worldgen/biome/{path}.json"),
            definition,
        );
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
