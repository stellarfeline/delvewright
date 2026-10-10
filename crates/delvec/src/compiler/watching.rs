//! A body watches (spec-0101): a still body turns to face the nearest player
//! in reach, by the game's own look-at re-issued from the delve's clock, and
//! yields the turn to any walk the story gives it.
//!
//! The one authority for everything the engine writes and proves about a
//! watching body: which bodies watch ([`watchers`], a filter over
//! `dsl::body_watch_sites`), the line `watch_tick` holds for each
//! ([`watch_line`]), the drawability proof (`DW0997`, [`prove`]) with its binding
//! line, and the record the PackTests and the bot read (`validation/watchers.json`,
//! [`WatchBinding::to_json`]).
//!
//! # The mechanism
//!
//! `rotate <body> facing entity <player> eyes` [cited — the pinned command
//! tree `rotate → target → facing → entity → facingEntity → facingAnchor`]
//! hands a non-player body to `Entity.lookAt`, which writes yaw and pitch from
//! that tick's two positions with no interpolation, and `LivingEntity.lookAt`
//! sets the body's yaw to the head's: body and head turn together, at once.
//! The compiler computes no bearing and eases nothing; every tick's facing is
//! the game's own arithmetic over that tick's state (`compiler.md` §4, "A
//! walked body faces where it is walking", restated quantifier).
//!
//! # The yield
//!
//! Every watching body is summoned carrying [`WATCH_TAG`]; the start function of
//! each of its walk drivers removes it, and the driver's arrival tick restores
//! it, so a walk and a watch never both write one body's yaw in one tick
//! ("One body, one live walk driver"). A superseded driver never arrives, so the
//! tag comes back with the walk that does.

use std::collections::BTreeSet;

use delvewright_dsl::{BodyRef, DwCode, ExitTier};
use serde_json::{Value, json};

use crate::compiler::failure::Failure;
use crate::compiler::plan::{self, Plan, ResolvedAnchor};

delvewright_dsl::dw_code! {
    /// `DW0997` (spec-0101 §5.2): **a watch nobody can draw.** No cell of the
    /// walked population `P` ([`crate::compiler::lethal::walked_population`],
    /// the population `DW0938` reads) has its standing point within `within`
    /// blocks of the body's feet, so no player can ever stand where the body
    /// would turn to them — the declaration is inert, and an inert declaration
    /// is refused, not ignored. Names the body, `within`, and the nearest walked
    /// cell with its distance; raising `within` to that distance's ceiling
    /// passes.
    pub const WATCH_UNDRAWABLE: DwCode = DwCode::new("DW0997", ExitTier::Build);
}

/// The tag a watching body carries while its watch is live. Removed by each of
/// its walk drivers' start functions and restored on the driver's arrival tick.
pub const WATCH_TAG: &str = "dw_watch";

/// The generated function holding one line per watching body.
pub const WATCH_FN: &str = "watch_tick";

/// The least bearing difference, in degrees, between a body's home facing and
/// the player its generated PackTest places, so the turn is observable.
pub const OBSERVABLE_DEGREES: f64 = 10.0;

/// One watching body, resolved.
#[derive(Clone, Debug)]
pub struct Watcher {
    /// The body's declared id.
    pub id: String,
    /// `npc` or `actor`.
    pub class: &'static str,
    /// The body's own marker tag: `dw_npc_<n>` (with `dw_npc`, the body and
    /// never the hitbox) or `dw_pup_<id>` (the puppet, never the unleashed twin).
    pub tag: String,
    /// The selector terms that address the body and nothing else.
    pub selector: String,
    /// The cell the body is summoned onto.
    pub cell: [i32; 3],
    /// The body's feet point: where the summon puts it.
    pub feet: [f64; 3],
    /// The yaw the summon writes (`Rotation[0]`), wrapped to `[-180, 180)`.
    pub home_yaw: f64,
    /// `nearest`, or the class id.
    pub who: String,
    /// `within`, in whole blocks.
    pub within: u32,
    /// The class tag the watch line filters on, for a class watch.
    pub class_tag: Option<String>,
    /// The JSON pointer at the declaration.
    pub path: String,
}

impl Watcher {
    /// The `@e` selector the watch line and every test address the body with,
    /// while its watch is live.
    pub fn live_selector(&self) -> String {
        format!(
            "@e[{},tag={WATCH_TAG},tag=!dw_unseen,limit=1]",
            self.selector
        )
    }
}

/// The game's yaw for a body at `from` facing a point at `to` (`Entity.lookAt`:
/// `wrapDegrees(atan2(dz, dx) * 57.2957763671875 - 90)`), in `[-180, 180)`.
pub fn bearing(from: [f64; 3], to: [f64; 3]) -> f64 {
    let dx = to[0] - from[0];
    let dz = to[2] - from[2];
    wrap_degrees(dz.atan2(dx) * 57.295_776_367_187_5 - 90.0)
}

/// `Mth.wrapDegrees`: an angle in `[-180, 180)`.
pub fn wrap_degrees(a: f64) -> f64 {
    let mut w = a % 360.0;
    if w >= 180.0 {
        w -= 360.0;
    }
    if w < -180.0 {
        w += 360.0;
    }
    // `-0.0` and `0.0` are one yaw; write the one the game prints.
    w + 0.0
}

/// The absolute difference between two yaws, in `[0, 180]`.
pub fn yaw_difference(a: f64, b: f64) -> f64 {
    wrap_degrees(a - b).abs()
}

/// A `y_rotation` range of `±tol` about `yaw`, in the selector's wrapped form:
/// the game wraps both bounds and reads `min > max` as the arc through ±180.
pub fn y_rotation_range(yaw: f64, tol: f64) -> String {
    let lo = wrap_degrees(yaw - tol);
    let hi = wrap_degrees(yaw + tol);
    format!("{lo:.2}..{hi:.2}")
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0 + 0.0
}

/// The feet point of a body summoned onto `cell` — the centre of the cell, on
/// its floor, exactly as the summon writes it.
pub fn feet_point(cell: [i32; 3]) -> [f64; 3] {
    crate::compiler::nav::cell_center(cell)
}

/// The distance the game's `distance` selector measures, feet to feet.
pub fn feet_distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// Where a stage-2 NPC's body is summoned and the yaw it is summoned with —
/// the one resolution the world-init summon, `spawn_npc_<id>` and the watch
/// all read: the NPC's declared area and anchor (`BodyScope::Declared`), plus
/// its offset, facing the anchor's own facing.
pub fn npc_home(plan: &Plan, npc: &delvewright_dsl::Npc) -> ([i32; 3], i32) {
    let area = npc.area.as_str();
    let station = plan::body_station(
        &plan.anchors,
        plan::BodyScope::Declared { area },
        npc.anchor.as_str(),
    );
    match &station {
        plan::BodyStation::At {
            anchor: ResolvedAnchor::Point { pos, facing },
            ..
        } => (
            delvewright_dsl::offset_cell(*pos, npc.offset),
            crate::compiler::emit::facing_yaw(facing.as_deref()),
        ),
        _ => ([0, plan::BASE_Y, 0], 0),
    }
}

/// Every watching body, in stage order, resolved. A body whose anchor does not
/// resolve is skipped (`DW0325`/`DW0345` own dangling references); a class
/// watch naming an undeclared class is skipped (`DW0996` refused it).
pub fn watchers(plan: &Plan) -> Vec<Watcher> {
    let mut out = Vec::new();
    for site in delvewright_dsl::body_watch_sites(plan.campaign) {
        let w = site.watch;
        let class_tag = match w.who.class() {
            None => None,
            Some(c) => match plan.classes.iter().find(|k| k.class_id == c.as_str()) {
                Some(k) => Some(format!("dw_class_{}", k.safe)),
                None => continue,
            },
        };
        let (class, tag, selector, cell, yaw) = match site.body {
            BodyRef::Npc(n) => {
                let safe = plan::safe_local(n.id.as_str());
                let tag = format!("dw_npc_{safe}");
                let (cell, yaw) = npc_home(plan, n);
                if plan.body_point(site.body).is_none() {
                    continue;
                }
                let selector = format!("tag={tag},tag=dw_npc");
                ("npc", tag, selector, cell, yaw)
            }
            BodyRef::Actor(a) => {
                let safe = plan::safe_local(a.id.as_str());
                let tag = format!("dw_pup_{safe}");
                let Some(cell) = plan.body_point(site.body) else {
                    continue;
                };
                let yaw = a
                    .facing
                    .map(|f| crate::compiler::emit::facing_yaw(Some(f.token())))
                    .unwrap_or(0);
                let selector = format!("tag={tag}");
                ("actor", tag, selector, cell, yaw)
            }
        };
        out.push(Watcher {
            id: site.body.id().to_string(),
            class,
            tag,
            selector,
            cell,
            feet: feet_point(cell),
            home_yaw: wrap_degrees(f64::from(yaw)),
            who: w.who.token(),
            within: w.within.get(),
            class_tag,
            path: site.path.clone(),
        });
    }
    out
}

/// Whether the body with this declared id watches.
pub fn body_watches(plan: &Plan, id: &str) -> bool {
    delvewright_dsl::body_watch_sites(plan.campaign)
        .iter()
        .any(|s| s.body.id() == id)
}

/// The one line `watch_tick` holds for a watching body (spec-0101 §4.4).
pub fn watch_line(w: &Watcher) -> String {
    let class = w
        .class_tag
        .as_ref()
        .map(|t| format!(",tag={t}"))
        .unwrap_or_default();
    format!(
        "execute as {} at @s run rotate @s facing entity @p[distance=..{},tag=!dw_cutscene{class}] eyes",
        w.live_selector(),
        w.within
    )
}

/// The root `tick`'s line that runs `watch_tick`; empty for a campaign that
/// declares no watcher, so its `tick` is byte-identical.
pub fn tick_lines(plan: &Plan) -> Vec<String> {
    if watchers(plan).is_empty() {
        return Vec::new();
    }
    vec![format!("function {}:{WATCH_FN}", plan.namespace)]
}

/// `watch_tick`: one line per watching body, in stage order. Empty for a
/// campaign that declares no watcher.
pub fn functions(plan: &Plan) -> Vec<(String, String)> {
    let ws = watchers(plan);
    if ws.is_empty() {
        return Vec::new();
    }
    let body: String = ws.iter().map(|w| format!("{}\n", watch_line(w))).collect();
    vec![(WATCH_FN.to_string(), body)]
}

/// The tag-removal line a watching body's walk start function carries, after
/// its re-entry guards (spec-0101 §4.4). `selector` addresses the body only.
pub fn yield_line(selector: &str) -> String {
    format!("tag @e[{selector}] remove {WATCH_TAG}")
}

/// The arrival-tick line that restores the watch: `when` is the driver's own
/// `if score … matches <total>` condition.
pub fn resume_line(when: &str, selector: &str) -> String {
    format!("execute if {when} run tag @e[{selector}] add {WATCH_TAG}")
}

/// What the drawability proof found for one watcher.
#[derive(Clone, Debug)]
pub struct WatchRecord {
    /// The watcher.
    pub watcher: Watcher,
    /// How many cells of `P` stand within `within` of its feet.
    pub drawable_cells: usize,
    /// The nearest walked cell and its distance, when `P` is not empty.
    pub nearest: Option<([i32; 3], f64)>,
    /// The drawable cell whose bearing differs most from the home facing, with
    /// that bearing — where the generated PackTest stands its player. `None`
    /// when no drawable cell turns the body by [`OBSERVABLE_DEGREES`] or more.
    pub test: Option<([i32; 3], f64)>,
    /// Every feet point the body can stand at: its summon point, then each of
    /// its walks' ends in plan order ([`WatchBinding::attach_walks`]).
    pub stands: Vec<[f64; 3]>,
}

impl WatchRecord {
    /// Drawable: some walked cell is within reach.
    pub fn drawable(&self) -> bool {
        self.drawable_cells > 0
    }
}

/// The binding the drawability proof states on every build.
#[derive(Clone, Debug, Default)]
pub struct WatchBinding {
    /// Cells in the walked population `P`.
    pub population: usize,
    /// Per watcher, in stage order.
    pub records: Vec<WatchRecord>,
}

impl WatchBinding {
    /// Watchers declared.
    pub fn declared(&self) -> usize {
        self.records.len()
    }

    /// Watchers some walked cell can draw.
    pub fn drawable(&self) -> usize {
        self.records.iter().filter(|r| r.drawable()).count()
    }

    /// Watchers refused (`DW0997`).
    pub fn refused(&self) -> usize {
        self.declared() - self.drawable()
    }

    /// Drawable watchers no drawable cell turns by the observable margin.
    pub fn unobservable(&self) -> usize {
        self.records
            .iter()
            .filter(|r| r.drawable() && r.test.is_none())
            .count()
    }

    /// The binding line every build prints, zeroes included.
    pub fn line(&self) -> String {
        format!(
            "watch binding: {} watcher(s) declared, over {} walked cell(s), {} drawable, {} refused, {} unobservable",
            self.declared(),
            self.population,
            self.drawable(),
            self.refused(),
            self.unobservable()
        )
    }

    /// Record where each watcher's walks end, so the bot knows where a body
    /// may stand when it asks whether one faces it.
    pub fn attach_walks(
        &mut self,
        moves: &[crate::compiler::nav::MovePlan],
        actor_moves: &[crate::compiler::nav::ActorMovePlan],
    ) {
        for r in &mut self.records {
            let ends: Vec<[i32; 3]> = match r.watcher.class {
                "npc" => moves
                    .iter()
                    .filter(|m| m.npc == r.watcher.id)
                    .map(|m| m.target)
                    .collect(),
                _ => actor_moves
                    .iter()
                    .filter(|m| m.actor == r.watcher.id)
                    .map(|m| m.target)
                    .collect(),
            };
            for e in ends {
                let p = feet_point(e);
                if !r.stands.contains(&p) {
                    r.stands.push(p);
                }
            }
        }
    }

    /// One record per watcher, the shape `validation/watchers.json` and the
    /// critical path's `watchers[]` carry.
    pub fn watcher_rows(&self) -> Vec<Value> {
        self.records
            .iter()
            .map(|r| {
                let w = &r.watcher;
                json!({
                    "id": w.id,
                    "class": w.class,
                    "tag": w.tag,
                    "selector": w.selector,
                    "feet": w.feet,
                    "stands": r.stands,
                    "home_yaw": w.home_yaw,
                    "who": w.who,
                    "class_tag": w.class_tag,
                    "within": w.within,
                    "drawable_cells": r.drawable_cells,
                    "test_cell": r.test.map(|(c, _)| c),
                    "test_yaw": r.test.map(|(_, y)| round2(y)),
                })
            })
            .collect()
    }

    /// `validation/watchers.json`.
    pub fn to_json(&self) -> Value {
        json!({
            "binding": {
                "declared": self.declared(),
                "population": self.population,
                "drawable": self.drawable(),
                "refused": self.refused(),
                "unobservable": self.unobservable(),
            },
            "watchers": self.watcher_rows(),
        })
    }
}

/// `DW0997`: judge every watcher against the walked population, and pick each
/// one's PackTest cell. Returns the binding beside the verdict, so the line a
/// run prints counts every watcher, not the ones before the first refusal.
pub fn prove(plan: &Plan, population: &BTreeSet<[i32; 3]>) -> (WatchBinding, Vec<Failure>) {
    let mut binding = WatchBinding {
        population: population.len(),
        records: Vec::new(),
    };
    let mut findings = Vec::new();
    for w in watchers(plan) {
        let reach = f64::from(w.within);
        let mut drawable_cells = 0usize;
        let mut nearest: Option<([i32; 3], f64)> = None;
        let mut test: Option<([i32; 3], f64, f64)> = None;
        for &cell in population {
            let stand = feet_point(cell);
            let d = feet_distance(w.feet, stand);
            if nearest.is_none_or(|(_, n)| d < n) {
                nearest = Some((cell, d));
            }
            if d > reach {
                continue;
            }
            drawable_cells += 1;
            // A player standing over the body's own column has no bearing the
            // game would agree on; such a cell never carries the test.
            let horizontal =
                ((stand[0] - w.feet[0]).powi(2) + (stand[2] - w.feet[2]).powi(2)).sqrt();
            if horizontal < 1.0 {
                continue;
            }
            // `P` is the lethality-free population; a player placed inside a
            // killing volume dies, and a dead player draws no look.
            if in_lethal_volume(plan, cell) {
                continue;
            }
            let yaw = bearing(w.feet, stand);
            let turn = yaw_difference(yaw, w.home_yaw);
            if turn >= OBSERVABLE_DEGREES && test.is_none_or(|(_, _, best)| turn > best) {
                test = Some((cell, yaw, turn));
            }
        }
        if drawable_cells == 0 {
            let near = match nearest {
                Some((c, d)) => format!(
                    "the nearest walked cell is [{}, {}, {}], {d:.2} blocks from its feet — \
                     `within: {}` would reach it",
                    c[0],
                    c[1],
                    c[2],
                    d.ceil() as i64
                ),
                None => "the walked population is empty, so no player stands anywhere".to_string(),
            };
            findings.push(Failure {
                code: WATCH_UNDRAWABLE,
                message: format!(
                    "`{}` ({}) watches within {} block(s) of its feet at [{}, {}, {}], and no \
                     cell a player can walk to stands that close: nothing could ever turn the \
                     body, so the declaration is inert. {near}. Raise `within`, or move the \
                     body to where the party walks.",
                    w.id, w.path, w.within, w.cell[0], w.cell[1], w.cell[2]
                ),
            });
        }
        let stands = vec![w.feet];
        binding.records.push(WatchRecord {
            stands,
            watcher: w,
            drawable_cells,
            nearest,
            test: test.map(|(c, y, _)| (c, y)),
        });
    }
    (binding, findings)
}

/// Whether `cell` lies inside any declared killing volume.
fn in_lethal_volume(plan: &Plan, cell: [i32; 3]) -> bool {
    plan.lethal_volumes.iter().any(|v| {
        let (lo, hi) = v.region;
        (0..3).all(|i| cell[i] >= lo[i].min(hi[i]) && cell[i] <= lo[i].max(hi[i]))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bearing_is_the_games_yaw() {
        let o = [0.5, 64.0, 0.5];
        // South (+z) is yaw 0, west (-x) 90, north (-z) -180, east (+x) -90 —
        // a few millionths off, because the game multiplies by the float
        // constant 57.2957763671875 rather than 180/π.
        assert!(yaw_difference(bearing(o, [0.5, 64.0, 5.5]), 0.0) < 1e-4);
        assert!(yaw_difference(bearing(o, [-4.5, 64.0, 0.5]), 90.0) < 1e-4);
        assert!(yaw_difference(bearing(o, [0.5, 64.0, -4.5]), 180.0) < 1e-4);
        assert!(yaw_difference(bearing(o, [5.5, 64.0, 0.5]), -90.0) < 1e-4);
    }

    #[test]
    fn a_range_across_the_seam_is_written_wrapped() {
        assert_eq!(y_rotation_range(179.5, 1.0), "178.50..-179.50");
        assert_eq!(y_rotation_range(0.0, 1.0), "-1.00..1.00");
    }
}
