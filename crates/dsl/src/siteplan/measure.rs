//! Identities: the plan held to the brief's own numbers (`DW0833`, `DW0834`).

use super::*;

// ---------------------------------------------------------------------------
// Identities: the plan held to the brief's own numbers
// ---------------------------------------------------------------------------

/// What one measure came out at, or why it could not be taken.
enum Measured {
    Value(f64),
    Unresolved(Diagnostic),
}

/// `DW0833` and `DW0834`: the brief's numbers still hold once the boxes are
/// drawn, and the binding that holds them is not empty.
///
/// `DW0834` is a **warning**, not an error, and the difference is deliberate: a
/// deliberately minimal plan — a fixture, a first sketch — is a legitimate
/// thing to hold, and refusing it would make the smallest useful document
/// uncompilable. What it may not be is *silent*, so the empty side is named on
/// every run and is a finding for the round summary.
///
/// `DW0833` runs here over the plan. **Its second call site is the built
/// world**, where the same rule recomputes the same measures from assembled
/// bytes so that a derivation defect which moved a datum cannot hide behind a
/// plan-time green. That site belongs to the round that builds the blockout;
/// nothing here approximates it.
pub(super) fn identities(
    c: &Campaign,
    plan: &SitePlanContent,
    placed: &[Placed<'_>],
    d: &mut Vec<Diagnostic>,
) {
    let facts: BTreeMap<&str, &crate::layout::BriefFact> = c
        .geometry_brief
        .as_ref()
        .map(|b| {
            b.content
                .facts
                .iter()
                .map(|f| (f.id.0.as_str(), f))
                .collect()
        })
        .unwrap_or_default();

    if facts.is_empty() || plan.identities.is_empty() {
        let empty = match (facts.is_empty(), plan.identities.is_empty()) {
            (true, true) => "the brief states no fact and the plan declares no identity",
            (true, false) => "the brief states no fact",
            _ => "the plan declares no identity",
        };
        d.push(Diagnostic::warning(
            DW_IDENTITY_EMPTY,
            "site-plan",
            "/content/identities",
            format!(
                "the identity gate binds nothing: {empty}. This is what holds the whole map to \
                 the design somebody wrote down — with either side empty, the plan may say \
                 anything at all and every check above will still pass, because none of them \
                 has an opinion about how big the map was meant to be. It is a warning rather \
                 than a refusal so that a deliberately minimal plan stays compilable; it is \
                 printed every run so that the emptiness is never quietly a pass."
            ),
        ));
    }

    let by_node: BTreeMap<&str, &Placed<'_>> =
        placed.iter().map(|p| (p.plan.node.0.as_str(), p)).collect();
    let datums: BTreeMap<&str, i64> = plan.datums.iter().map(|x| (x.id.0.as_str(), x.y)).collect();

    for (i, id) in plan.identities.iter().enumerate() {
        let Some(fact) = facts.get(id.fact.0.as_str()) else {
            d.push(Diagnostic::error(
                DW_PLAN_AGREEMENT,
                "site-plan",
                format!("/content/identities/{i}/fact"),
                format!(
                    "this identity holds the map to `{f}`, which the geometry brief states no \
                     fact for. An identity binds to a number the brief WROTE DOWN — that is what \
                     makes it a design being kept rather than an assertion the plan makes about \
                     itself.",
                    f = id.fact,
                ),
            ));
            continue;
        };
        let measured = measure(&id.measure, plan, &by_node, &datums, i);
        let value = match measured {
            Measured::Value(v) => v,
            Measured::Unresolved(diag) => {
                d.push(diag);
                continue;
            }
        };
        if id.cmp.holds(value, fact.value) {
            continue;
        }
        d.push(Diagnostic::error(
            DW_IDENTITY_FALSE,
            "site-plan",
            format!("/content/identities/{i}"),
            format!(
                "the plan does not keep `{f}`: {what} measures {value}, and the brief asks for \
                 {cmp} {want}{unit}. The brief's sentence was: \"{note}\". Either move the \
                 geometry until the number is true, or change the brief's fact — in the brief, \
                 where the design is written down, so that the change is a decision somebody \
                 took rather than a plan that drifted.",
                f = id.fact,
                what = describe(&id.measure),
                cmp = id.cmp.as_str(),
                want = fact.value,
                unit = fact
                    .unit
                    .as_ref()
                    .map(|u| format!(" {u}"))
                    .unwrap_or_default(),
                note = fact.note,
            ),
        ));
    }
}

/// Take one measure off the plan.
fn measure(
    m: &Measure,
    plan: &SitePlanContent,
    by_node: &BTreeMap<&str, &Placed<'_>>,
    datums: &BTreeMap<&str, i64>,
    i: usize,
) -> Measured {
    let missing_node = |node: &NodeId| {
        Measured::Unresolved(Diagnostic::error(
            DW_PLAN_AGREEMENT,
            "site-plan",
            format!("/content/identities/{i}/measure"),
            format!(
                "this identity measures `{node}`, which this plan embeds no box for. A measure \
                 is taken off the geometry, so it can only name a place the plan actually put \
                 somewhere."
            ),
        ))
    };
    match m {
        Measure::RegionExtent { axis } => {
            Measured::Value(f64::from(plan.region.extent[axis.index()].get()))
        }
        Measure::BoxExtent { node, axis } => match by_node.get(node.0.as_str()) {
            Some(p) => Measured::Value(f64::from(p.plan.extent[axis.index()].get())),
            None => missing_node(node),
        },
        Measure::BoxHeight { node } => match by_node.get(node.0.as_str()) {
            Some(p) => match p.clearance {
                Some(c) => Measured::Value(f64::from(c)),
                None => Measured::Unresolved(Diagnostic::error(
                    DW_PLAN_AGREEMENT,
                    "site-plan",
                    format!("/content/identities/{i}/measure"),
                    format!(
                        "this identity measures the height of `{node}`, which is sky-open and \
                         whose size class did not resolve — so the plan states no headroom for \
                         it at all. Fix the class name the layout graph declares (`DW0812` names \
                         it) and the height becomes the class's own minimum."
                    ),
                )),
            },
            None => missing_node(node),
        },
        Measure::DistanceXz { from, to } => {
            let (Some(a), Some(b)) = (by_node.get(from.0.as_str()), by_node.get(to.0.as_str()))
            else {
                return missing_node(if by_node.contains_key(from.0.as_str()) {
                    to
                } else {
                    from
                });
            };
            let (ax, az) = a.centre_xz();
            let (bx, bz) = b.centre_xz();
            Measured::Value(((bx - ax).powi(2) + (bz - az).powi(2)).sqrt())
        }
        Measure::DatumY { datum } => match datums.get(datum.0.as_str()) {
            Some(y) => Measured::Value(*y as f64),
            None => Measured::Unresolved(Diagnostic::error(
                crate::codes::DANGLING_REF,
                "site-plan",
                format!("/content/identities/{i}/measure"),
                format!(
                    "this identity measures `{datum}`, which this plan declares no `datums[]` \
                     entry for."
                ),
            )),
        },
    }
}

/// What a measure is, in a refusal's own words.
fn describe(m: &Measure) -> String {
    match m {
        Measure::RegionExtent { axis } => {
            format!("the region's extent on {}", axis.as_str())
        }
        Measure::BoxExtent { node, axis } => {
            format!("`{node}`'s footprint on {}", axis.as_str())
        }
        Measure::BoxHeight { node } => format!("`{node}`'s headroom"),
        Measure::DistanceXz { from, to } => {
            format!("the horizontal distance from `{from}` to `{to}`")
        }
        Measure::DatumY { datum } => format!("the plane `{datum}`"),
    }
}

impl Axis {
    fn index(self) -> usize {
        match self {
            Axis::X => 0,
            Axis::Y => 1,
            Axis::Z => 2,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Axis::X => "x",
            Axis::Y => "y",
            Axis::Z => "z",
        }
    }
}

impl PlanAxis {
    fn index(self) -> usize {
        match self {
            PlanAxis::X => 0,
            PlanAxis::Z => 1,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            PlanAxis::X => "x",
            PlanAxis::Z => "z",
        }
    }
}

impl VolumeRole {
    /// The keyword a refusal prints.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            VolumeRole::Massif => "massif",
            VolumeRole::Ground => "ground",
            VolumeRole::Clearance => "clearance",
        }
    }
}
