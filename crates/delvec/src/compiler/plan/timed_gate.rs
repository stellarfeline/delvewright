//! Timed gates: the resolved stage-5 `timed-gate` record and its disarm.

/// A resolved stage-5 `timed-gate` (spec-0016 §4), in declared order.
#[derive(Clone, Debug)]
pub struct TimedGatePlan {
    /// The full timed-gate id.
    pub id: String,
    /// The function/tag-safe local id.
    pub safe: String,
    /// The gate anchor name.
    pub gate_anchor: String,
    /// The gate region's inclusive corners (absolute world coords).
    pub gate_region: ([i32; 3], [i32; 3]),
    /// The block the region is filled with while closed.
    pub gate_block: String,
    /// Ticks open per cycle.
    pub open_ticks: u32,
    /// Ticks closed per cycle.
    pub closed_ticks: u32,
    /// Ticks after world init before the first open window.
    pub phase: u32,
    /// Whether the closing edge kills players caught inside the region
    /// (spec-0016 §4 addendum).
    pub crush: bool,
    /// The resolved disarm affordance, if declared. A gate whose
    /// `disarm.via` anchor does not resolve carries `None` — the DSL tier's
    /// `DW0377` reports that, and no half-built affordance reaches emission.
    pub disarm: Option<TimedGateDisarmPlan>,
}

/// A resolved `timed-gate` disarm affordance — the same shape a
/// trap's [`TrapDisarmPlan`] takes.
#[derive(Clone, Debug)]
pub struct TimedGateDisarmPlan {
    /// The anchor name the player interacts with.
    pub via_anchor: String,
    /// Its resolved absolute cell.
    pub via_cell: [i32; 3],
    /// The flag jamming the gate sets, party-wide.
    pub sets_flag: String,
}
