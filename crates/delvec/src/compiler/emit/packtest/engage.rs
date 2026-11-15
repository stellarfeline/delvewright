use super::*;

/// One kind of hostile body a wave seats, as the engagement template reads it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct EngagedKind {
    /// The entity id.
    pub entity: String,
    /// The bodies of the kind a one-player seating puts in the world.
    pub count: u32,
    /// The reach the proofs read for it (`nav::follow::mob_reach`, the most
    /// over the wave's stacks of the kind).
    pub reach: f64,
    /// Whether the reach runs past the lane's `aggro_radius` — the kinds a
    /// mob held at the radius after its release would fail.
    pub beyond_lane: bool,
}

/// The hostile kinds of `w`, in declaration order: every stack whose species
/// the game files under `monster` (`nav::follow::is_monster`).
pub(crate) fn engaged_kinds(w: &delvewright_dsl::Wave) -> Vec<EngagedKind> {
    let lane = w.lane.as_ref().map(|l| f64::from(l.aggro_radius));
    let mut out: Vec<EngagedKind> = Vec::new();
    for m in &w.mobs {
        if !crate::compiler::nav::follow::is_monster(&m.entity) {
            continue;
        }
        let reach = crate::compiler::nav::follow::mob_reach(w, m).0;
        match out.iter_mut().find(|k| k.entity == m.entity) {
            Some(k) => {
                k.count += m.count.base();
                k.reach = k.reach.max(reach);
            }
            None => out.push(EngagedKind {
                entity: m.entity.clone(),
                count: m.count.base(),
                reach,
                beyond_lane: false,
            }),
        }
    }
    for k in &mut out {
        k.beyond_lane = lane.is_some_and(|r| k.reach > r);
    }
    out
}

/// What the engagement templates bound, zeroes included.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EngageBinding {
    /// Waves declared.
    pub waves: usize,
    /// Waves with a hostile stack, each given a template.
    pub hostile: usize,
    /// Hostile kinds asserted across them.
    pub kinds: usize,
    /// Of those, lane kinds whose reach runs past the lane's radius.
    pub beyond_lane: usize,
}

impl EngageBinding {
    /// The binding line.
    pub fn line(&self) -> String {
        format!(
            "engagement binding (PackTest): {} hostile wave(s) of {} declared, {} hostile kind(s) \
             asserted to carry, once in the fight, the follow range the proofs read as their \
             reach; {} of them lane kinds whose reach runs past the lane's aggro radius",
            self.hostile, self.waves, self.kinds, self.beyond_lane
        )
    }
}

/// **A hostile wave's bodies, once in the fight, keep a target as far as the
/// reach the compiler's proofs read** (`nav::follow::mob_reach`): one
/// template per wave with a hostile stack. The wave is seated, every body is
/// struck once by the dummy and the lane clock run — the dummy is the body
/// that hurt it, so a lane mob is released — and every body of every hostile
/// kind must carry a `minecraft:follow_range` of at least its kind's reach.
/// A mob keeps a target only within its follow range
/// (`docs/reference/lane-engagement.md` §1, established once against the
/// pinned jar), so this is the compiler's half of "the raiders fight": the
/// emitted bodies reach as far as every proof over the fight assumes. It
/// asks nothing of difficulty or survival (ADR-0039). A lane mob held at the
/// radius after its release fails it.
pub(super) fn emit_engagement_packtests(plan: &Plan, out: &mut BuildOutput) -> EngageBinding {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let mut b = EngageBinding {
        waves: plan.campaign.quests.content.waves.len(),
        ..EngageBinding::default()
    };
    for w in &plan.campaign.quests.content.waves {
        let kinds = engaged_kinds(w);
        if kinds.is_empty() {
            continue;
        }
        b.hostile += 1;
        b.kinds += kinds.len();
        b.beyond_lane += kinds.iter().filter(|k| k.beyond_lane).count();
        let safe = plan::safe_local(w.id.as_str());
        let tag = plan::wave_tag(w.id.as_str());
        let name = format!("engage_{safe}");
        let (pin, sel) = pin_dummy(&format!("dw_pt_eng_{safe}"));
        let mut t = packtest_header(&format!(
            "{title}: every hostile of `{}`, once in the fight, keeps a target as far as the \
             reach the proofs read (spec-0016 §6)",
            w.id
        ));
        t.push(format!("function {ns}:setup"));
        t.push(pin);
        t.push(format!("kill @e[tag={tag}]"));
        t.extend(super::super::wave::packtest_spawn_lines(plan, &safe));
        t.push(format!(
            "execute as @e[tag={tag}] run damage @s 1 minecraft:player_attack by {sel}"
        ));
        if w.lane.is_some() {
            t.push(format!("function {ns}:lane_tick_{safe}"));
        }
        for (i, k) in kinds.iter().enumerate() {
            let held = format!("#e{i}_{safe}");
            let bodies = format!("@e[tag={tag},type={}]", k.entity);
            t.push(format!("scoreboard players set {held} dw.sys 0"));
            t.push(format!(
                "execute as {bodies} store result score @s dw.sys run attribute @s \
                 minecraft:follow_range get"
            ));
            t.push(format!(
                "execute as {bodies} if score @s dw.sys matches {}.. run scoreboard players add \
                 {held} dw.sys 1",
                k.reach.ceil() as i64
            ));
            t.push(format!("scoreboard players reset @e[tag={tag}] dw.sys"));
            t.push(format!("assert score {held} dw.sys matches {}", k.count));
        }
        t.push(format!("kill @e[tag={tag}]"));
        t.push(format!("tag {sel} remove dw_pt_eng_{safe}"));
        out.insert(
            format!("packtest-datapack/data/{ns}/test/{name}.mcfunction"),
            lines(&t).into_bytes(),
        );
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A lane pillager's reach (32) runs past its 8-block radius, and the
    /// template asks the body to carry it; a non-monster stack is no kind.
    #[test]
    fn a_lane_kind_owes_its_species_reach_and_a_creature_owes_nothing() {
        let w: delvewright_dsl::Wave = serde_json::from_value(serde_json::json!({
            "id": "wave/w", "anchor": "anchor/a",
            "lane": { "aggro_radius": 8, "waypoints": ["anchor/b"] },
            "mobs": [{ "entity": "minecraft:pillager", "count": 2 },
                     { "entity": "minecraft:goat", "count": 1 }]
        }))
        .expect("a wave");
        let k = engaged_kinds(&w);
        assert_eq!(k.len(), 1);
        assert_eq!((k[0].reach, k[0].count, k[0].beyond_lane), (32.0, 2, true));
    }
}
