//! **No signal opens a shut gate** (`DW1047`): a shut gate whose block reads a
//! redstone signal — the closed iron trapdoor a floor is shut with
//! (`siteplan::SEAM_HATCH`, `SEAM_COVER`) among them — has no signal source
//! anywhere that can power its cell, in the world as placed or after any
//! runtime write or prop. A wall's iron bars read no signal: examined, counted,
//! never judged.
//!
//! The pinned jar's rule, read once while this was built (`javap -c -p` with
//! the server mappings, `docs/reference/compiler.md`, *No signal opens a shut
//! gate*): `TrapDoorBlock.neighborChanged` sets `open` to
//! `Level.hasNeighborSignal(pos)` whenever it differs from `powered`, and
//! `SignalGetter.hasNeighborSignal` asks `getSignal(pos.relative(d), d) > 0`
//! for each of the six faces. `getSignal` is the neighbour's own (weak) signal
//! toward the cell, and — when the neighbour `isRedstoneConductor` — the
//! greatest strong signal (`getDirectSignal`) any of ITS six neighbours sends
//! into it (`getDirectSignalTo`). So power reaches a trapdoor from a source in
//! a face neighbour, or from a source beside a conductor in a face neighbour,
//! and from nowhere further: a conductor sends no strong signal of its own.
//!
//! **The model**: a source powers its six face neighbours and, for each that
//! may hold a conductor, that conductor's six neighbours. It ignores which way
//! a source points and whether it is on — a lever's strong signal reaches only
//! its attached face, a torch's none upward into its own block — so it reads
//! every source as the strongest it could be: it can only refuse more. A
//! signal source is `blockshape::redstone_source` (the pinned jar's
//! `isSignalSource` on any state), a conductor `blockshape::redstone_conductor`,
//! a block that reads a signal `blockshape::redstone_reader` — the measured
//! table `crates/dsl/data/redstone-1.21.11.tsv`. Every block a cell may hold is
//! `sculk::possible`, over `sculk::runtime_writes`: the as-placed block and
//! every block a `fill-region`, `set-block`, `close-gate`, laid way, shortcut
//! seal, timed gate or prop may lay there, all at once, and the relight
//! fixtures.

use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::blockshape::{redstone_conductor, redstone_reader, redstone_source};
use delvewright_dsl::siteplan::{SitePlan, seam_anchor};
use delvewright_dsl::{DwCode, ExitTier};
use serde_json::{Value, json};

use crate::compiler::blockstate::BlockMap;
use crate::compiler::failure::Failure;
use crate::compiler::plan::Plan;
use crate::compiler::plan::ResolvedAnchor;
use crate::compiler::sculk::{Write, neighbours, possible, runtime_writes};

delvewright_dsl::dw_code! {
    /// `DW1047`: **a redstone signal can open a shut gate**. A cell of a shut
    /// gate holds a block that reads a signal (a closed iron trapdoor through
    /// a floor), and a signal source — as placed, or laid by a runtime
    /// write — stands in one of its six face neighbours, or beside a conductor
    /// in one of them. The trapdoor would open on that signal without its story
    /// beat while the seal model holds it shut. Build tier.
    pub const DW_GATE_SIGNAL: DwCode = DwCode::new("DW1047", ExitTier::Build);
}

/// One shut gate judged: its anchor, and the cells its block stands in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShutGate {
    /// The gate's anchor, `anchor/seam-<edge>`.
    pub anchor: String,
    /// The block the seam is shut with.
    pub block: String,
    /// Whether the seam is through a floor.
    pub floor: bool,
    /// The gate's cells, in world coordinates.
    pub cells: Vec<[i32; 3]>,
}

/// What [`prove`] examined.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GateSignalBinding {
    /// Shut gates examined: every gate region of the world.
    pub examined: usize,
    /// Of which their block reads a signal, judged.
    pub gates: usize,
    /// Of which through a floor.
    pub floor_gates: usize,
    /// Their cells.
    pub cells: usize,
    /// Of which their block reads no signal (iron bars), not judged.
    pub inert_gates: usize,
    /// Cells that may hold a signal source, examined.
    pub sources: usize,
    /// Of which a runtime write may lay the source.
    pub sources_written: usize,
    /// The least Manhattan distance from a source to a judged cell.
    pub nearest: Option<i64>,
}

impl GateSignalBinding {
    /// The binding line.
    pub fn line(&self) -> String {
        let nearest = self.nearest.map_or_else(
            || "no source to measure against".to_string(),
            |d| format!("the nearest {d} block(s) from a judged cell"),
        );
        format!(
            "DW1047 binding: {} shut gate(s) examined; {} whose block reads a signal judged ({} \
             through a floor, {} cell(s)), {} whose block reads none not judged; {} signal \
             source(s) examined ({} laid by a runtime write), {nearest}",
            self.examined,
            self.gates,
            self.floor_gates,
            self.cells,
            self.inert_gates,
            self.sources,
            self.sources_written
        )
    }

    /// The ledger `validation/gate-signal.json`; the gates examined are
    /// stated as `gates_examined`, which the gallery's coverage gate refuses
    /// at 0.
    pub fn to_json(&self) -> Value {
        json!({
            "code": "DW1047",
            "gates_examined": self.examined,
            "gates": self.gates,
            "floor_gates": self.floor_gates,
            "cells": self.cells,
            "inert_gates": self.inert_gates,
            "sources": self.sources,
            "sources_written": self.sources_written,
            "nearest": self.nearest,
        })
    }
}

fn narrow(c: [i64; 3]) -> [i32; 3] {
    c.map(|v| i32::try_from(v).expect("a site cell fits a world coordinate"))
}

fn cells_between(a: [i32; 3], b: [i32; 3]) -> Vec<[i32; 3]> {
    let lo = [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])];
    let hi = [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])];
    crate::compiler::assembled::region_cells(lo, hi).collect()
}

/// **Every shut gate of the built world**: each gate region the plan resolves
/// (a prefab's declared gate, a derived seam's `anchor/seam-<edge>`, the
/// region a `close-gate` or a timed gate fills), with its fill block, and
/// every shut seam of the site plan the anchors do not already name. Sorted by
/// anchor (ADR-0006).
pub fn shut_gates(plan: &Plan) -> Vec<ShutGate> {
    let site = SitePlan::of(plan.campaign);
    let floor_of = |anchor: &str| {
        site.seams
            .iter()
            .any(|s| s.normal_axis == 1 && s.bar().is_some() && seam_anchor(&s.edge) == anchor)
    };
    let mut out: BTreeMap<String, ShutGate> = BTreeMap::new();
    for ((_, name), resolved) in plan.anchors.iter() {
        if let ResolvedAnchor::Gate { from, to, block } = resolved {
            out.entry(name.clone()).or_insert_with(|| ShutGate {
                anchor: name.clone(),
                block: block.clone(),
                floor: floor_of(name),
                cells: cells_between(*from, *to),
            });
        }
    }
    for s in &site.seams {
        let Some((lo, hi)) = s.bar() else { continue };
        let name = seam_anchor(&s.edge);
        out.entry(name.clone()).or_insert_with(|| ShutGate {
            anchor: name,
            block: s.bar_block().to_string(),
            floor: s.normal_axis == 1,
            cells: cells_between(narrow(lo), narrow(hi)),
        });
    }
    out.into_values().collect()
}

fn manhattan(a: [i32; 3], b: [i32; 3]) -> i64 {
    (0..3)
        .map(|i| (i64::from(a[i]) - i64::from(b[i])).abs())
        .sum()
}

/// **`DW1047`: no signal source can power a shut gate's cell.** A gate is
/// judged when its block, or any block a cell of it may hold, reads a signal.
/// The binding is returned beside the first refusal.
pub fn prove(
    gates: &[ShutGate],
    blocks: &BlockMap,
    writes: &[Write],
) -> (GateSignalBinding, Option<Failure>) {
    let mut b = GateSignalBinding::default();
    // Judged cell -> (gate anchor, the reading block there).
    let mut judged: BTreeMap<[i32; 3], (&str, String)> = BTreeMap::new();
    b.examined = gates.len();
    for g in gates {
        let reads = redstone_reader(&g.block)
            || g.cells.iter().any(|c| {
                possible(blocks, writes, *c)
                    .iter()
                    .any(|(n, _)| redstone_reader(n))
            });
        if !reads {
            b.inert_gates += 1;
            continue;
        }
        b.gates += 1;
        b.floor_gates += usize::from(g.floor);
        for c in &g.cells {
            let block = possible(blocks, writes, *c)
                .into_iter()
                .map(|(n, _)| n.to_string())
                .find(|n| redstone_reader(n))
                .unwrap_or_else(|| g.block.clone());
            judged.insert(*c, (g.anchor.as_str(), block));
        }
    }
    b.cells = judged.len();

    // Every cell that may hold a signal source: as placed, then every cell a
    // write laying one covers.
    let mut sources: BTreeSet<[i32; 3]> = blocks
        .iter()
        .filter(|(_, s)| redstone_source(s.as_str()))
        .map(|(c, _)| *c)
        .collect();
    let mut written: BTreeSet<[i32; 3]> = BTreeSet::new();
    for w in writes.iter().filter(|w| redstone_source(&w.block)) {
        written.extend(w.cells());
    }
    sources.extend(written.iter().copied());
    b.sources = sources.len();
    b.sources_written = written.len();
    if judged.is_empty() {
        return (b, None);
    }

    let mut first: Option<Failure> = None;
    for s in &sources {
        for c in judged.keys() {
            let d = manhattan(*s, *c);
            if b.nearest.is_none_or(|best| d < best) {
                b.nearest = Some(d);
            }
        }
        if first.is_some() {
            continue;
        }
        let Some((src, how)) = possible(blocks, writes, *s)
            .into_iter()
            .find(|(n, _)| redstone_source(n))
        else {
            continue;
        };
        // What the source powers: its face neighbours, and the neighbours of
        // any of them that may hold a conductor.
        let mut reach: Vec<([i32; 3], Option<([i32; 3], String)>)> =
            neighbours(*s).into_iter().map(|n| (n, None)).collect();
        for n in neighbours(*s) {
            if let Some((cond, chow)) = possible(blocks, writes, n)
                .into_iter()
                .find(|(m, _)| redstone_conductor(m))
            {
                for m in neighbours(n) {
                    if m != *s {
                        reach.push((m, Some((n, format!("`{cond}` {chow}")))));
                    }
                }
            }
        }
        for (r, via) in reach {
            let Some((anchor, block)) = judged.get(&r) else {
                continue;
            };
            let via = match via {
                None => "it stands in one of the trapdoor's six face neighbours".to_string(),
                Some((n, cond)) => format!(
                    "it stands beside [{}, {}, {}], a face neighbour of the trapdoor holding the \
                     conductor {cond}, which hands its strong signal on",
                    n[0], n[1], n[2]
                ),
            };
            first = Some(Failure {
                code: DW_GATE_SIGNAL,
                message: format!(
                    "the shut gate `{anchor}` stands `{block}` at [{}, {}, {}], and the signal \
                     source `{src}` at [{}, {}, {}] ({how}) can power that cell: {via}. An iron \
                     trapdoor opens whenever a neighbour signal reaches it \
                     (`TrapDoorBlock.neighborChanged` reads `Level.hasNeighborSignal`), so the \
                     gate would open without its story beat while the seal model holds it shut. \
                     Move the source or the gate: no signal source in the trapdoor's six face \
                     neighbours, nor beside a conductor in one of them",
                    r[0], r[1], r[2], s[0], s[1], s[2]
                ),
            });
            break;
        }
    }
    (b, first)
}

/// [`prove`] over every shut gate of the plan, the assembled world, every
/// runtime write and prop (`sculk::runtime_writes`), and the relight fixtures
/// the datapack sets at setup.
pub fn check(
    plan: &Plan,
    blocks: &BlockMap,
    fixtures: &[crate::compiler::light::Placement],
) -> (GateSignalBinding, Option<Failure>) {
    let mut writes = runtime_writes(plan);
    writes.extend(fixtures.iter().map(|f| Write {
        region: (f.pos, f.pos),
        block: f.block.clone(),
        label: "a relight fixture set at setup".to_string(),
    }));
    prove(&shut_gates(plan), blocks, &writes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::blockstate::BlockState;

    const HATCH: &str = delvewright_dsl::siteplan::SEAM_HATCH;
    const BAR: &str = delvewright_dsl::siteplan::SEAM_BAR;
    const LEVER: &str = "minecraft:lever[face=floor,facing=north,powered=false]";

    /// A stone floor at y = 10 over x, z in 0..=8, with a hatch at [4, 10, 4].
    fn floor_with_hatch() -> (Vec<ShutGate>, BlockMap) {
        let mut m = BlockMap::new();
        for x in 0..=8 {
            for z in 0..=8 {
                m.insert([x, 10, z], BlockState::new("minecraft:stone"));
            }
        }
        m.insert([4, 10, 4], BlockState::new(HATCH));
        let gates = vec![ShutGate {
            anchor: "anchor/seam-shaft".into(),
            block: HATCH.into(),
            floor: true,
            cells: vec![[4, 10, 4]],
        }];
        (gates, m)
    }

    #[test]
    fn a_lever_beside_a_hatch_is_refused_and_moved_away_passes() {
        let (gates, mut m) = floor_with_hatch();
        // Beside it: on the floor, over the stone next to the hatch — the
        // stone conducts, so the lever's power reaches the trapdoor.
        m.insert([5, 11, 4], BlockState::new(LEVER));
        let (b, f) = prove(&gates, &m, &[]);
        let f = f.expect("a lever beside a hatch is refused");
        assert_eq!(f.code.id(), "DW1047");
        assert!(f.message.contains("[4, 10, 4]"), "{}", f.message);
        assert!(f.message.contains("[5, 11, 4]"), "{}", f.message);
        assert!(f.message.contains("Move the source or the gate"));
        assert_eq!((b.gates, b.cells, b.sources), (1, 1, 1));
        assert_eq!(b.nearest, Some(2));

        // Moved three blocks away: nothing reaches.
        m.remove(&[5, 11, 4]);
        m.insert([7, 11, 4], BlockState::new(LEVER));
        let (b, f) = prove(&gates, &m, &[]);
        assert!(f.is_none(), "{f:?}");
        assert_eq!((b.gates, b.sources, b.nearest), (1, 1, Some(4)));
    }

    #[test]
    fn a_source_in_a_face_neighbour_is_refused() {
        let (gates, mut m) = floor_with_hatch();
        m.insert([4, 9, 4], BlockState::new("minecraft:redstone_block"));
        let (_, f) = prove(&gates, &m, &[]);
        let f = f.expect("refused");
        assert!(f.message.contains("six face neighbours"), "{}", f.message);
    }

    #[test]
    fn a_source_beside_a_non_conductor_does_not_reach() {
        let (gates, mut m) = floor_with_hatch();
        m.insert([5, 10, 4], BlockState::new("minecraft:glass"));
        m.insert([5, 11, 4], BlockState::new(LEVER));
        let (_, f) = prove(&gates, &m, &[]);
        assert!(f.is_none(), "{f:?}");
    }

    #[test]
    fn a_source_a_runtime_write_lays_is_judged() {
        let (gates, m) = floor_with_hatch();
        let w = Write {
            region: ([4, 11, 5], [4, 11, 5]),
            block: "minecraft:redstone_torch[lit=true]".into(),
            label: "`set-block` at `quests[0]`".into(),
        };
        let (b, f) = prove(&gates, &m, std::slice::from_ref(&w));
        let f = f.expect("refused");
        assert!(f.message.contains("laid by `set-block`"), "{}", f.message);
        assert_eq!(b.sources_written, 1);
    }

    #[test]
    fn bars_read_no_signal_and_are_not_judged() {
        let (_, mut m) = floor_with_hatch();
        m.insert([4, 10, 4], BlockState::new(BAR));
        m.insert([5, 11, 4], BlockState::new(LEVER));
        let gates = vec![ShutGate {
            anchor: "anchor/seam-door".into(),
            block: BAR.into(),
            floor: false,
            cells: vec![[4, 10, 4]],
        }];
        let (b, f) = prove(&gates, &m, &[]);
        assert!(f.is_none());
        assert_eq!((b.gates, b.inert_gates, b.cells), (0, 1, 0));
    }
}
