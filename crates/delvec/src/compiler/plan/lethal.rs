//! Lethal volumes: the resolved record, its staged gates, and its collection.

use super::*;

/// A resolved stage-5 **lethal volume** (DSL v0.10, spec-0031): the box, the
/// wording, and the damage type the kill is dealt with.
///
/// Resolution is the shared [`Plan::zone_box`] — the same anchor-centred box a
/// `begin-stealth` zone and a `damage-players` `in` filter resolve through — so a
/// volume cannot drift into its own geometry rule. A volume whose anchor no placed
/// piece provides is simply absent from this list (validation reports it as
/// `DW0142`), never a blank box at the world origin.
pub struct LethalVolumePlan {
    /// The authored id (`lethal/<kebab>`).
    pub id: String,
    /// `safe_local(id)` — the segment that names the emitted function.
    pub safe: String,
    /// Inclusive world-space corners of the box.
    pub region: ([i32; 3], [i32; 3]),
    /// The (l10n-tagged) line the volume says as it kills.
    pub message: String,
    /// The damage type the kill is dealt with.
    pub damage_type: delvewright_dsl::DamageKind,
    /// The blocks the volume declares as showing it (spec-0062 §3), as
    /// declared. Read by `DW0891` against the assembled bytes, per caught cell.
    pub shown_by: Vec<String>,
    /// The story stage this volume is live from (spec-0088), or `None` for a
    /// volume live from world-load to the end.
    pub staged: Option<StagedGate>,
}

/// A lethal volume's gate (spec-0088), resolved: the three axes as declared,
/// and [`Plan::gate_terms`]'s reduction of them — the one reading the emitted
/// tick guard, the PackTest templates and `death-plan.json` all take.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StagedGate {
    /// Flags that must all be set for the volume to kill.
    pub requires_flags: Vec<String>,
    /// Flags whose being set withholds the volume.
    pub forbids_flags: Vec<String>,
    /// Numeric comparisons (every one on a `party` datum: `DW0953`).
    pub requires_state: Vec<delvewright_dsl::StateCompare>,
    /// [`Plan::gate_terms`] over the three, in its order.
    pub terms: Vec<GateTerm>,
}

impl StagedGate {
    /// The gate's terms in words, for a diagnostic: `requires flag/x`,
    /// `forbids flag/y`, `state/z at-least 3`.
    pub fn words(&self) -> String {
        let mut out: Vec<String> = Vec::new();
        out.extend(
            self.requires_flags
                .iter()
                .map(|f| format!("requires `{f}`")),
        );
        out.extend(self.forbids_flags.iter().map(|f| format!("forbids `{f}`")));
        out.extend(self.requires_state.iter().map(|c| {
            format!(
                "`{}` {} {}",
                c.state.as_str(),
                serde_json::to_value(c.op)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_string))
                    .unwrap_or_default(),
                c.value
            )
        }));
        out.join(", ")
    }
}

impl LethalVolumePlan {
    /// Whether `cell` lies inside this volume.
    pub fn contains(&self, cell: [i32; 3]) -> bool {
        let (lo, hi) = self.region;
        (0..3).all(|i| lo[i] <= cell[i] && cell[i] <= hi[i])
    }
}

/// Resolve every declared lethal volume (DSL v0.10, spec-0031) against the solved
/// layout, in declaration order.
///
/// The box is `anchor ± extent`, resolved exactly as [`Plan::zone_box`] resolves a
/// stealth zone and a `damage-players` `in` filter — one geometry rule for every
/// anchor-centred box in the engine. A volume whose anchor no placed piece
/// provides is dropped (validation already reported `DW0142`) rather than
/// silently becoming a box at the origin.
pub(super) fn collect_lethal_volumes(
    campaign: &Campaign,
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
) -> Vec<LethalVolumePlan> {
    campaign
        .quests
        .content
        .lethal_volumes
        .iter()
        .filter_map(|v| {
            let c = point_any(anchors, v.region.anchor.as_str())?;
            let e = v.region.extent;
            Some(LethalVolumePlan {
                id: v.id.as_str().to_string(),
                safe: safe_local(v.id.as_str()),
                region: (
                    [c[0] - e[0] as i32, c[1] - e[1] as i32, c[2] - e[2] as i32],
                    [c[0] + e[0] as i32, c[1] + e[1] as i32, c[2] + e[2] as i32],
                ),
                message: v.message.clone(),
                damage_type: v
                    .damage_type
                    .unwrap_or(delvewright_dsl::DamageKind::Generic),
                shown_by: v.shown_by.clone(),
                staged: v.when.as_ref().map(|g| StagedGate {
                    requires_flags: g
                        .requires_flags
                        .iter()
                        .map(|f| f.as_str().to_string())
                        .collect(),
                    forbids_flags: g
                        .forbids_flags
                        .iter()
                        .map(|f| f.as_str().to_string())
                        .collect(),
                    requires_state: g.requires_state.clone(),
                    terms: gate_terms_of(campaign, v.gate()),
                }),
            })
        })
        .collect()
}
