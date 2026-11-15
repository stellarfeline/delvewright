use super::*;

/// The lowest course an engagement pad stands on: above every course a delve
/// builds on, below the pinned overworld's build top (320).
const PAD_FLOOR_Y: i32 = 300;
/// The highest course an engagement pad may stand on, leaving its two
/// courses of headroom under the build top.
const PAD_CEILING_Y: i32 = 316;
/// How long the template waits after the blows for the target goals to run.
const ENGAGE_WAIT_TICKS: u32 = 20;

/// One kind of hostile body a wave seats, as the engagement template drives it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct EngagedKind {
    /// The entity id.
    pub entity: String,
    /// The bodies of the kind a one-player seating puts in the world.
    pub count: u32,
    /// The reach the proofs read for it (`nav::follow::mob_reach`, the least
    /// over the wave's stacks of the kind).
    pub reach: f64,
    /// Where the template stands the kind from the party, blocks along +x.
    pub distance: f64,
    /// Whether `distance` lies beyond the lane's `aggro_radius` — the only
    /// distance at which a mob held at the radius would let the party go.
    pub beyond_lane: bool,
}

/// **The distance the template stands a kind from the party**: inside its
/// reach by a margin a step or two of the mob's walk cannot close, and — for
/// a lane mob whose reach runs past the radius — past the lane's radius, so a
/// mob still held at the radius after its release lets the party go.
pub(crate) fn engage_distance(reach: f64, lane_radius: Option<u32>) -> (f64, bool) {
    let d = if reach >= 4.0 {
        reach - 2.0
    } else {
        (reach - 0.5).max(1.0)
    };
    let beyond = lane_radius.is_some_and(|r| d > f64::from(r));
    (d, beyond)
}

/// The hostile kinds of `w`, in declaration order: every stack whose species
/// the game files under `monster` (`nav::follow::is_monster`).
pub(crate) fn engaged_kinds(w: &delvewright_dsl::Wave) -> Vec<EngagedKind> {
    let mut out: Vec<EngagedKind> = Vec::new();
    for m in &w.mobs {
        if !crate::compiler::nav::follow::is_monster(&m.entity) {
            continue;
        }
        let reach = crate::compiler::nav::follow::mob_reach(w, m).0;
        match out.iter_mut().find(|k| k.entity == m.entity) {
            Some(k) => {
                k.count += m.count.base();
                k.reach = k.reach.min(reach);
            }
            None => out.push(EngagedKind {
                entity: m.entity.clone(),
                count: m.count.base(),
                reach,
                distance: 0.0,
                beyond_lane: false,
            }),
        }
    }
    let lane = w.lane.as_ref().map(|l| l.aggro_radius);
    for k in &mut out {
        (k.distance, k.beyond_lane) = engage_distance(k.reach, lane);
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
    /// Of those, kinds stood past their lane's radius.
    pub beyond_lane: usize,
    /// Hostile waves with no seat, given no template.
    pub unseated: Vec<String>,
}

impl EngageBinding {
    /// The binding line.
    pub fn line(&self) -> String {
        format!(
            "engagement binding (PackTest): {} hostile wave(s) of {} declared, {} hostile kind(s) \
             asserted to keep the party as their target within their reach, {} of them stood past \
             their lane's aggro radius; {} hostile wave(s) unseated and unasserted{}",
            self.hostile,
            self.waves,
            self.kinds,
            self.beyond_lane,
            self.unseated.len(),
            if self.unseated.is_empty() {
                String::new()
            } else {
                format!(" ({})", self.unseated.join(", "))
            }
        )
    }
}

/// **A hostile wave's bodies keep the party as their target within the reach
/// the compiler's proofs read** (`nav::follow::mob_reach`): one template per
/// wave with a hostile stack. The wave is seated, every hostile body is stood
/// on a barrier pad above the world at its kind's distance from the dummy
/// ([`engage_distance`]), struck once by the dummy, its lane clock run, and
/// after [`ENGAGE_WAIT_TICKS`] every body of the kind must hold the dummy as
/// its target. Whether the party survives, or how hard the fight is, is not
/// asked (ADR-0039): only that the bodies engage at the reach the proofs
/// assume. A lane mob whose follow range is held at the lane's radius drops a
/// target past it, so the template reds on that emission.
pub(super) fn emit_engagement_packtests(
    plan: &Plan,
    out: &mut BuildOutput,
    placements: &WavePlacements,
) -> EngageBinding {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let top = plan
        .areas
        .iter()
        .flat_map(|a| a.pieces.iter().map(|p| p.bbox().1[1]))
        .max()
        .unwrap_or(0);
    let y = (top + 4).clamp(PAD_FLOOR_Y, PAD_CEILING_Y);
    let mut b = EngageBinding {
        waves: plan.campaign.quests.content.waves.len(),
        ..EngageBinding::default()
    };
    for w in &plan.campaign.quests.content.waves {
        let kinds = engaged_kinds(w);
        if kinds.is_empty() {
            continue;
        }
        let Some(seat) = placements.get(w.id.as_str()).and_then(|c| c.first()) else {
            b.unseated.push(w.id.as_str().to_string());
            continue;
        };
        b.hostile += 1;
        b.kinds += kinds.len();
        b.beyond_lane += kinds.iter().filter(|k| k.beyond_lane).count();
        let safe = plan::safe_local(w.id.as_str());
        let tag = plan::wave_tag(w.id.as_str());
        let name = format!("engage_{safe}");
        let (pin, sel) = pin_dummy(&format!("dw_pt_eng_{safe}"));
        let far = kinds
            .iter()
            .map(|k| k.distance.ceil() as i32)
            .max()
            .unwrap_or(1);
        let (x0, z0) = (seat[0], seat[2]);
        let (x1, z1) = (x0 + far + 1, z0 + 1);
        let pad = format!("{} {} {} {} {} {}", x0 - 1, y - 1, z0 - 1, x1, y - 1, z1);
        let mut t = vec![
            format!(
                "#> {title}: every hostile of `{}` keeps the party as its target within the reach \
                 the proofs read (spec-0016 §6)",
                w.id
            ),
            super::limit::alone(ns, &name, out),
            "# @dummy".to_string(),
            "# @timeout 200".to_string(),
            String::new(),
            format!("function {ns}:setup"),
            pin,
            format!("kill @e[tag={tag}]"),
            format!("forceload add {} {} {x1} {z1}", x0 - 1, z0 - 1),
            format!("fill {pad} minecraft:barrier replace minecraft:air"),
            format!("tp {sel} {}.5 {y} {}.5", x0, z0),
            format!("effect give {sel} minecraft:resistance 60 4 true"),
        ];
        t.extend(super::super::wave::packtest_spawn_lines(plan, &safe));
        for k in &kinds {
            t.push(format!(
                "execute as @e[tag={tag},type={}] run tp @s {} {y} {}.5",
                k.entity,
                fmt_f64(f64::from(x0) + 0.5 + k.distance),
                z0
            ));
        }
        t.push(format!(
            "execute as @e[tag={tag}] run damage @s 1 minecraft:player_attack by {sel}"
        ));
        if w.lane.is_some() {
            t.push(format!("function {ns}:lane_tick_{safe}"));
        }
        t.push(format!("await delay {ENGAGE_WAIT_TICKS}t"));
        for (i, k) in kinds.iter().enumerate() {
            let held = format!("#e{i}_{safe}");
            t.push(format!("scoreboard players set {held} dw.sys 0"));
            t.push(format!(
                "execute as @e[tag={tag},type={}] on target if entity @s[tag=dw_pt_eng_{safe}] \
                 run scoreboard players add {held} dw.sys 1",
                k.entity
            ));
            t.push(format!("assert score {held} dw.sys matches {}", k.count));
        }
        t.push(format!("kill @e[tag={tag}]"));
        t.push(format!(
            "fill {pad} minecraft:air replace minecraft:barrier"
        ));
        t.push(format!("effect clear {sel}"));
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

    /// A lane pillager (reach 32) is stood past its 8-block radius; a
    /// vindicator on a 14-block lane reaches the radius only and is not.
    #[test]
    fn a_kind_is_stood_inside_its_reach_and_past_its_lane() {
        assert_eq!(engage_distance(32.0, Some(8)), (30.0, true));
        assert_eq!(engage_distance(14.0, Some(14)), (12.0, false));
        assert_eq!(engage_distance(1.0, None), (1.0, false));
    }
}
