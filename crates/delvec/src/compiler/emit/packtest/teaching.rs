use super::*;

/// spec-0104 §5.4 PackTests: every teaching site shows its line once.
///
/// **A teaching trigger** — the poll a player standing within the click's
/// reach of the trigger's anchor runs — is driven with the template's own
/// dummy on the anchor's cell, twice: the dummy is marked taught, and the
/// party's line count (`#taught_<class>`) moves by exactly one, so the line is
/// shown once and not again.
///
/// **A teaching objective** has its activation driven here twice, whether the
/// activation places nothing (a `talk-to`, a `kill`: the function exists only
/// because it teaches) or also places the objective's affordance (that
/// placement is asserted by the activation template; the teaching line is
/// asserted here and nowhere else). Its activation teaches every untaught
/// player, so the count is read as at least one, and a second activation adds
/// none. A placing activation is followed by the cleanup of what the two calls
/// summoned, so the template leaves no residue for its siblings.
pub(super) fn emit_teaching_packtests(plan: &Plan, out: &mut BuildOutput) {
    use super::super::teaching::{taught_score, teach_at_fn};
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let sites = crate::compiler::acts::teach_sites(plan);
    for (tid, (classes, p)) in &sites.triggers {
        let id = plan::safe_local(tid);
        let (pin, sel) = pin_dummy(&format!("dw_t_tch_{id}"));
        let tag = format!("dw_t_tch_{id}");
        let mut b = packtest_header(&format!(
            "{title}: standing within reach of trigger `{tid}` teaches its input once"
        ));
        b.push(format!("function {ns}:setup"));
        b.push(pin);
        for cl in classes {
            b.push(format!(
                "scoreboard players reset {sel} {}",
                taught_score(*cl)
            ));
            b.push(format!(
                "scoreboard players set #taught_{} dw.sys 0",
                cl.as_str()
            ));
        }
        b.push(format!("tp {sel} {}.5 {} {}.5", p[0], p[1], p[2]));
        b.push(format!("function {ns}:{}", teach_at_fn(tid)));
        b.push(format!("function {ns}:{}", teach_at_fn(tid)));
        for cl in classes {
            let c = cl.as_str();
            b.push(format!("assert score #taught_{c} dw.sys matches 1"));
            b.push(format!(
                "execute store success score #tch_{id} dw.sys if entity @a[tag={tag},scores={{{}=1}}]",
                taught_score(*cl)
            ));
            b.push(format!("assert score #tch_{id} dw.sys matches 1"));
        }
        b.push(format!("tag {sel} remove {tag}"));
        out.insert(
            format!("packtest-datapack/data/{ns}/test/teach_trigger_{id}.mcfunction"),
            lines(&b).into_bytes(),
        );
    }
    for q in &plan.campaign.quests.content.quests {
        let area = plan.quest_area(q.id.as_str()).unwrap_or("");
        for o in &q.objectives {
            let oid = o.id().as_str();
            let Some(classes) = sites.objectives.get(oid) else {
                continue;
            };
            let placed = activation_commands(plan, area, o);
            // A placement no fixture reads still has its teaching asserted: a
            // site with no assertion is the defect, residue is not.
            let cleanup =
                super::objective::activation_residue_cleanup(&placed, 2).unwrap_or_default();
            let safe = plan::safe_local(oid);
            let tag = format!("dw_t_tco_{safe}");
            let (pin, sel) = pin_dummy(&tag);
            let mut b = packtest_header(&format!(
                "{title}: activating objective `{oid}` teaches its input once"
            ));
            b.push(format!("function {ns}:setup"));
            b.push(pin);
            for cl in classes {
                b.push(format!(
                    "scoreboard players reset {sel} {}",
                    taught_score(*cl)
                ));
                b.push(format!(
                    "scoreboard players set #taught_{} dw.sys 0",
                    cl.as_str()
                ));
            }
            b.push(format!("function {ns}:activate_{}", safe_obj_fn(oid)));
            for cl in classes {
                let c = cl.as_str();
                b.push(format!("assert score #taught_{c} dw.sys matches 1.."));
                b.push(format!(
                    "execute store success score #tco_{safe} dw.sys if entity @a[tag={tag},scores={{{}=1}}]",
                    taught_score(*cl)
                ));
                b.push(format!("assert score #tco_{safe} dw.sys matches 1"));
                b.push(format!(
                    "scoreboard players operation #tc0_{safe} dw.sys = #taught_{c} dw.sys"
                ));
            }
            b.push(format!("function {ns}:activate_{}", safe_obj_fn(oid)));
            for cl in classes {
                let c = cl.as_str();
                b.push(format!(
                    "execute store success score #tc1_{safe} dw.sys if score #taught_{c} dw.sys = #tc0_{safe} dw.sys"
                ));
                b.push(format!("assert score #tc1_{safe} dw.sys matches 1"));
            }
            b.extend(cleanup);
            b.push(format!("tag {sel} remove {tag}"));
            out.insert(
                format!("packtest-datapack/data/{ns}/test/teach_objective_{safe}.mcfunction"),
                lines(&b).into_bytes(),
            );
        }
    }
}
