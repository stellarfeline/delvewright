use super::*;

/// What [`emit_variant_packtests`] read back, of what the campaign declares.
pub(in crate::compiler::emit) struct VariantPacktestBinding {
    /// Bodies declaring a non-empty variant.
    pub declared: usize,
    /// Of them, the bodies a template stands and reads back.
    pub read: usize,
}

impl VariantPacktestBinding {
    pub(in crate::compiler::emit) fn line(&self) -> String {
        format!(
            "variant binding (DW1131–DW1132 PackTest): {} of {} declared variant(s) read back on \
             the server — an NPC standing from load and every actor; a wave mob, a mount, a \
             vehicle and a deferred NPC are summoned by the same rule and not stood here",
            self.read, self.declared
        )
    }
}

/// **A body's declared variant reads back as declared** (DSL v0.83): one
/// template per NPC standing from load and per actor that declares a variant.
/// The body is summoned the way the campaign summons it — `setup` for the NPC,
/// its own `spawn_actor_<id>` for the actor — and the template asks the server
/// whether that body, of its declared entity type, holds every field the
/// declaration writes (`nbt={…}`, a subset match over the saved data). A cat
/// declared `{"variant": "minecraft:tabby", "collar": "blue"}` is found only
/// with `variant:"minecraft:tabby"` and `CollarColor:11b` in its save: a key the
/// game did not read, a form it did not take or a body summoned without the
/// fragment finds nothing, and the score stays 0.
pub(super) fn emit_variant_packtests(plan: &Plan, out: &mut BuildOutput) -> VariantPacktestBinding {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let c = plan.campaign;
    let mut binding = VariantPacktestBinding {
        declared: 0,
        read: 0,
    };
    for site in delvewright_dsl::body_variant_sites(c) {
        let fields = delvewright_dsl::variant_fields(site.entity, site.variant);
        if fields.is_empty() {
            continue;
        }
        binding.declared += 1;
        let safe = plan::safe_local(site.id);
        let (stand, selector): (Vec<String>, String) = match site.what {
            "npc" => {
                let Some(npc) = plan.npcs.iter().find(|n| n.npc_id == site.id) else {
                    continue;
                };
                let deferred = c
                    .npcs
                    .content
                    .npcs
                    .iter()
                    .any(|n| n.id.as_str() == site.id && n.deferred);
                if deferred {
                    continue;
                }
                (Vec::new(), format!("tag={}", npc.tag))
            }
            "actor" => (
                vec![
                    format!("kill @e[tag=dw_actor_{safe}]"),
                    format!("function {ns}:spawn_actor_{safe}"),
                ],
                format!("tag=dw_pup_{safe}"),
            ),
            _ => continue,
        };
        binding.read += 1;
        let entity = delvewright_dsl::namespaced_entity(site.entity);
        let nbt = delvewright_dsl::render_fields(&fields);
        let mut b = packtest_header(&format!(
            "{title}: {} `{}`, a {entity}, stands with the variant it declares ({} field(s))",
            site.what,
            site.id,
            fields.len()
        ));
        b.push(format!("function {ns}:setup"));
        b.extend(stand);
        b.push(format!(
            "execute store result score #var_{safe} dw.sys if entity @e[type={entity},{selector},nbt={{{nbt}}}]"
        ));
        b.push(format!("assert score #var_{safe} dw.sys matches 1"));
        out.insert(
            format!(
                "packtest-datapack/data/{ns}/test/variant_{}_{safe}.mcfunction",
                site.what
            ),
            lines(&b).into_bytes(),
        );
    }
    binding
}
