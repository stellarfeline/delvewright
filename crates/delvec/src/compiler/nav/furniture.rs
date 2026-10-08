/// **What the furniture exclusion bound on one build** (spec-0065 §4.3).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FurnitureBinding {
    /// Furniture regions placed in the world.
    pub regions: usize,
    /// Solid cells those regions hold.
    pub solid: usize,
    /// Cells a player could stand in on the bare geometry that the exclusion
    /// withholds from walking — the count that says the declarations bit.
    pub withheld: usize,
    /// Walked legs proved: exported critical-path legs, `move-npc` legs and
    /// `move-actor` legs.
    pub legs: usize,
    /// Cells of those legs a body stands on furniture in, computed over every
    /// leg's own cells for the footprint it was routed under.
    pub on_furniture: usize,
    /// The furniture anchors placed, in the plan's order, one per placement.
    pub anchors: Vec<String>,
}

impl FurnitureBinding {
    /// The line every build prints, zeroes included, so a campaign whose pieces
    /// declare no furniture reads as checked rather than unbound.
    pub fn line(&self) -> String {
        format!(
            "furniture binding: {} region(s) over {} solid cell(s), {} standable cell(s) \
             withheld from walking; {} leg(s) proved, {} standing on furniture.",
            self.regions, self.solid, self.withheld, self.legs, self.on_furniture
        )
    }

    /// `validation/furniture-gate.json`: the same counts, for a gate that reads
    /// the build rather than its stderr. `examined` is the region count, so a
    /// reader that reds a zero binding reds a build whose campaign declares
    /// furniture and whose pieces placed none.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "anchors": self.anchors,
            "examined": self.regions,
            "legs": self.legs,
            "on_furniture": self.on_furniture,
            "solid_cells": self.solid,
            "spec": "spec-0065",
            "withheld": self.withheld,
        })
    }
}

/// Measure [`FurnitureBinding`] over the world a build proved its walks in and
/// the legs it proved there.
pub fn furniture_binding(
    plan: &Plan,
    world: &World,
    routes: &[LegRoute],
    moves: &[MovePlan],
    actor_moves: &[ActorMovePlan],
) -> FurnitureBinding {
    let (regions, solid, withheld) = world.furniture_census();
    let player = Footprint::player();
    let mut on_furniture = 0;
    for r in routes {
        on_furniture += world.cells_on_furniture(&r.cells, &player);
    }
    for m in moves {
        on_furniture += world.cells_on_furniture(&m.cells, &player);
    }
    for m in actor_moves {
        let fp = actor_of(plan, &m.actor)
            .map(|a| entity_footprint(&a.entity))
            .unwrap_or_else(Footprint::player);
        on_furniture += world.cells_on_furniture(&m.cells, &fp);
    }
    FurnitureBinding {
        regions,
        solid,
        withheld,
        legs: routes.len() + moves.len() + actor_moves.len(),
        on_furniture,
        anchors: plan
            .furniture
            .iter()
            .map(|(name, _)| name.clone())
            .collect(),
    }
}
