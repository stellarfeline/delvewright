// ---------------------------------------------------------------------------
// Fluid that leaves the built world (DW0318)
// ---------------------------------------------------------------------------


/// **What the fluid-escape proof looked at**, so the verdict is readable as a
/// measurement rather than as a silence (CLAUDE.md: every validation artifact
/// states its binding count).
///
/// None of these three numbers is the length of the finding list:
/// `pieces` is counted off the **plan** (how much world there is), `fluid_cells`
/// off the **assembled occupancy model** (how much water there is), and only
/// `outside` is this check's own conclusion. A build in which `fluid_cells` is
/// zero examined nothing, and says so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FluidEscape {
    /// The **ambient** the verdict is stated against — `"void"` or `"ocean"`.
    pub horizon: &'static str,
    /// **The base the campaign declared**, beside the ambient it resolves to.
    ///
    /// `valley` resolves to a `void` ambient — its ground is placed blocks
    /// rather than a generator fact — so a line keyed to the ambient alone said
    /// ``horizon `void``` about a world whose author wrote `valley`, and a
    /// reader cannot tell an unbound check from a mislabelled one
    /// (spec-0060 §10.7).
    pub base: &'static str,
    /// Placed pieces forming the built volume ([`World::built`]).
    pub pieces: usize,
    /// Every fluid cell in the assembled world: prefab-authored sources plus the
    /// reach the shared flood model gives them
    /// ([`crate::compiler::assembled::Occupancy::flooded`]). This is the binding count.
    pub fluid_cells: usize,
    /// The subset of those cells lying outside **every** placed piece's AABB,
    /// sorted (ADR-0006). Under `void` this is the violation set; under `ocean`
    /// it is water that reached the ambient sea, and is reported and not judged.
    pub outside: Vec<[i32; 3]>,
    /// The placed pieces whose AABB touches an escaped cell — where to fix it.
    /// Fluid only ever moves cell to cell, so every escaped cell set is
    /// 6-connected back into the volume it came from; sorted, deduplicated.
    pub from_pieces: Vec<String>,
}

/// Measure where the assembled world's fluid ended up relative to the built
/// volume. Pure: it re-derives nothing, reading `world.flooded` — the one
/// placement-time fluid model this repo has ([`crate::compiler::assembled::flood`]'s
/// product, via [`crate::compiler::assembled::occupancy_of`]) — and `world.built`.
pub fn measure_fluid_escape(world: &World) -> FluidEscape {
    let outside: Vec<[i32; 3]> = world
        .flooded
        .iter()
        .filter(|&c| !world.is_built(c))
        .collect();
    // Attribution: the piece an escaped cell is 6-adjacent to. Deterministic —
    // `outside` is sorted (it comes from a `BTreeSet`) and the neighbour offsets
    // are a fixed list, collected through a `BTreeSet`.
    const NEIGHBOURS: [[i32; 3]; 6] = [
        [-1, 0, 0],
        [1, 0, 0],
        [0, -1, 0],
        [0, 1, 0],
        [0, 0, -1],
        [0, 0, 1],
    ];
    let mut from: BTreeSet<String> = BTreeSet::new();
    for &c in &outside {
        for d in NEIGHBOURS {
            let n = [c[0] + d[0], c[1] + d[1], c[2] + d[2]];
            for (id, (lo, hi)) in &world.built {
                if (0..3).all(|a| lo[a] <= n[a] && n[a] <= hi[a]) {
                    from.insert(id.clone());
                }
            }
        }
    }
    FluidEscape {
        horizon: world.ambient.name(),
        base: world.base,
        pieces: world.built.len(),
        fluid_cells: world.flooded.len(),
        outside,
        from_pieces: from.into_iter().collect(),
    }
}

impl FluidEscape {
    /// The `DW0318` violation, or `None`. A finding **only** under
    /// [`Ambient::Void`]: under `ocean` the water met the sea, which is what a
    /// shoreline piece's water is for.
    pub fn finding(&self) -> Option<Failure> {
        if self.horizon != "void" || self.outside.is_empty() {
            return None;
        }
        let columns: BTreeSet<(i32, i32)> = self.outside.iter().map(|c| (c[0], c[2])).collect();
        let lowest = self.outside.iter().map(|c| c[1]).min().unwrap_or(0);
        let sample: Vec<String> = self
            .outside
            .iter()
            .take(BOUNDARY_LIST_LIMIT)
            .map(|c| format!("{c:?}"))
            .collect();
        let more = self.outside.len().saturating_sub(sample.len());
        let blame = if self.from_pieces.is_empty() {
            "no placed piece adjoins them".to_string()
        } else {
            format!("from placed piece(s) {}", self.from_pieces.join(", "))
        };
        Some(Failure {
            code: DW_FLUID_LEAVES_WORLD,
            message: format!(
                "fluid leaves the built world: {n} fluid cell(s) in {cols} column(s) lie outside \
                 every placed piece, {blame}. Under `horizon: void` a column the content did not \
                 build is bottomless, so this water is not a pond that overhangs an edge — it is \
                 a waterfall that runs down forever on the server's own clock, before any player \
                 arrives, and nothing that draws the delve draws it. Cells: {sample}{extra}; the \
                 model stops marking at y={lowest} (the lowest cell it holds), the game does not \
                 stop at all. Examined {cells} fluid cell(s) across {pieces} placed piece(s). \
                 WHERE to fix: the prefab or tileset generator that authored this water, or the \
                 placement that put an open face against nothing — the piece-level rule counts a \
                 run leaving a face and deliberately does not judge it, because only the \
                 placement knows what is on the other side. HOW: wall the face the water runs \
                 out of, pull the body back a cell, place a piece against that face, or declare \
                 `horizon: ocean` if this water is meant to be a sea. Do NOT delete the water to \
                 silence this: an authored pond is first-class content.",
                n = self.outside.len(),
                cols = columns.len(),
                sample = sample.join(", "),
                extra = if more > 0 {
                    format!(" (+{more} more)")
                } else {
                    String::new()
                },
                cells = self.fluid_cells,
                pieces = self.pieces,
            ),
        })
    }

    /// The binding ledger (`validation/fluid-escape.json`): what was examined,
    /// not only what was found. Emitted for every campaign that assembles a
    /// world, so a zero binding is a number a reader can act on rather than a
    /// check nobody notices did nothing.
    pub fn ledger(&self) -> serde_json::Value {
        serde_json::json!({
            "horizon_base": self.base,
            "horizon": self.horizon,
            "pieces_examined": self.pieces,
            "fluid_cells_examined": self.fluid_cells,
            "cells_outside_built_volume": self.outside.len(),
            "from_pieces": self.from_pieces,
            "verdict": if self.finding().is_some() { "fail" } else { "pass" },
        })
    }
}

// ---------------------------------------------------------------------------
// The ambient sea inside the built volume (DW0851)
// ---------------------------------------------------------------------------


/// **What the sea-seepage proof looked at**, so its verdict reads as a
/// measurement rather than a silence (CLAUDE.md: every validation artifact states
/// its binding count, with its denominator).
///
/// Only `submerged` is a conclusion; every other number is a denominator or a
/// measurement. `walk_cells` is the population the verdict is drawn from — the
/// walk region the build already proved, which is where the party stands — and
/// `objective_cells` is how many of the critical path's own cells are in it.
/// `pieces` is how much built volume there is; `contact_cells` is how much of it
/// the sea touches through an open face; `sea_waterlogged` is how many of its
/// blocks the sea waterlogs at placement; `wet_cells` is how far the water then
/// got. A run under `horizon: void` reports zeroes and says so in `horizon`:
/// there is no ambient sea to come in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeaSeepage {
    /// The **ambient** the verdict is stated against — `"void"` or `"ocean"`.
    pub horizon: &'static str,
    /// **The base the campaign declared**, beside the ambient it resolves to.
    ///
    /// `valley` resolves to a `void` ambient — its ground is placed blocks
    /// rather than a generator fact — so a line keyed to the ambient alone said
    /// ``horizon `void``` about a world whose author wrote `valley`, and a
    /// reader cannot tell an unbound check from a mislabelled one
    /// (spec-0060 §10.7).
    pub base: &'static str,
    /// Placed pieces forming the built volume ([`World::built`]).
    pub pieces: usize,
    /// Cells inside the built volume that the ambient sea is directly touching
    /// through an open face — one of the two seed sets.
    pub contact_cells: usize,
    /// Blocks inside the built volume, in the sea's own band, that the placement
    /// hands to the sea: waterloggable, therefore `waterlogged=true` the moment
    /// `/place template` writes them into an ocean column
    /// ([`crate::compiler::assembled::is_waterloggable`]). The other seed set,
    /// and the one that put the field case's whole walk plane under water.
    pub sea_waterlogged: usize,
    /// Every cell inside the built volume the sea reaches from those seeds.
    pub wet_cells: usize,
    /// **The denominator**: reachable standable cells this build has. Every
    /// verdict below is drawn from this population and from nothing else.
    pub walk_cells: usize,
    /// How many cells the critical path names are in that population — the
    /// binding count that matters, because these are the cells the party is
    /// REQUIRED to stand on.
    pub objective_cells: usize,
    /// **Measured, not judged**: walk cells that are dry underfoot and touch
    /// water — head-height or at the foot's own level. That is a shoreline, and a
    /// body walks it; a shoreline 26 cells wide and one 2000 cells wide are
    /// different maps, and a check that says nothing about either is a silence.
    pub wading: Vec<[i32; 3]>,
    /// The violation: walk cells whose **own** cell — where the body's feet go —
    /// holds fluid once the world loads, sorted (ADR-0006).
    pub submerged: Vec<[i32; 3]>,
    /// The placed pieces those cells lie in — where to fix it. Sorted, deduped.
    pub in_pieces: Vec<String>,
    /// The objectives standing in submerged cells, `(objective id, cell)`, in
    /// critical-path order. What a reader acts on.
    pub drowned_objectives: Vec<(String, [i32; 3])>,
}

/// The 6 face-adjacent offsets, in a fixed order (determinism, ADR-0006).
const FACE6: [[i32; 3]; 6] = [
    [-1, 0, 0],
    [1, 0, 0],
    [0, -1, 0],
    [0, 1, 0],
    [0, 0, -1],
    [0, 0, 1],
];

/// The 4 cardinal horizontal steps, in a fixed order (determinism, ADR-0006) —
/// what "beside" means for a body standing in a cell.
const HORIZ4: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

/// Measure the delivered fluid over **the walk**, and what of it the party
/// stands in. See [`DW_SEA_ENTERS_WALK`] for the model.
///
/// `reachable` is the caller's already-computed reachable walkable set — passed
/// in rather than re-derived so this proof and the stranding proof are answering
/// about the **same** walk region, and so a build pays for that flood once. It is
/// also the denominator: this proof asks its question of every cell a body was
/// proved to stand on, and of nothing else.
pub fn measure_sea_seepage(world: &World, reachable: &BTreeSet<[i32; 3]>) -> SeaSeepage {
    let objective_cells = world
        .objective_cells
        .iter()
        .filter(|(_, c)| reachable.contains(c))
        .count();
    let empty = |horizon| SeaSeepage {
        horizon,
        base: world.base,
        pieces: world.built.len(),
        contact_cells: 0,
        sea_waterlogged: 0,
        wet_cells: 0,
        walk_cells: reachable.len(),
        objective_cells,
        wading: Vec::new(),
        submerged: Vec::new(),
        in_pieces: Vec::new(),
        drowned_objectives: Vec::new(),
    };
    let Ambient::Ocean(sea) = &world.ambient else {
        return empty("void");
    };
    if world.built.is_empty() {
        return empty("ocean"); // nothing placed: the sea has nothing to come into
    }

    // Every block cell dams water, exactly as `occupancy_of` has it. `flooded` is
    // deliberately absent: authored water is water, and the sea flows through it.
    let mut barriers: BTreeSet<[i32; 3]> = BTreeSet::new();
    for set in [&world.solid, &world.tall, &world.use_gates] {
        barriers.extend(set.iter());
    }
    // Confinement (model step 3): the one-cell skin of NON-built cells around the
    // built volume becomes barrier, so the flow cannot leave the content. Without
    // it the flood would spread across the open sea — which is already water — and
    // come back in somewhere else, and the answer would be about the ocean rather
    // than about this build.
    for (_, (lo, hi)) in &world.built {
        for x in lo[0] - 1..=hi[0] + 1 {
            for y in lo[1] - 1..=hi[1] + 1 {
                for z in lo[2] - 1..=hi[2] + 1 {
                    let on_skin = x == lo[0] - 1
                        || x == hi[0] + 1
                        || y == lo[1] - 1
                        || y == hi[1] + 1
                        || z == lo[2] - 1
                        || z == hi[2] + 1;
                    let c = [x, y, z];
                    if on_skin && !world.is_built(c) {
                        barriers.insert(c);
                    }
                }
            }
        }
    }

    // Seeds, set 1 (model step 1a): the contact face — inside the built volume,
    // inside the sea's own band, open, and touching ambient sea water.
    let mut contact: BTreeSet<[i32; 3]> = BTreeSet::new();
    for (_, (lo, hi)) in &world.built {
        let y_lo = (sea.floor_top + 1).max(lo[1]);
        let y_hi = sea.level.min(hi[1]);
        for y in y_lo..=y_hi {
            for x in lo[0]..=hi[0] {
                for z in lo[2]..=hi[2] {
                    let c = [x, y, z];
                    if barriers.contains(&c) {
                        continue;
                    }
                    if FACE6
                        .iter()
                        .any(|d| world.ambient_water([c[0] + d[0], c[1] + d[1], c[2] + d[2]]))
                    {
                        contact.insert(c);
                    }
                }
            }
        }
    }
    // Seeds, set 2 (model step 1b): every waterloggable block the placement puts
    // in the sea's band. The cell stays occupied by its host block — it is a
    // barrier, exactly as an authored `waterlogged=true` cell is in
    // `occupancy_of` — and it is a source that wets its neighbours.
    let waterlogged: BTreeSet<[i32; 3]> = world
        .waterloggable
        .iter()
        .filter(|c| c[1] > sea.floor_top && c[1] <= sea.level && world.is_built(*c))
        .collect();
    let mut seeds = contact.clone();
    seeds.extend(waterlogged.iter().copied());
    if seeds.is_empty() {
        return SeaSeepage {
            horizon: "ocean",
            base: world.base,
            pieces: world.built.len(),
            contact_cells: 0,
            sea_waterlogged: 0,
            wet_cells: 0,
            walk_cells: reachable.len(),
            objective_cells,
            wading: Vec::new(),
            submerged: Vec::new(),
            in_pieces: Vec::new(),
            drowned_objectives: Vec::new(),
        };
    }

    // Flow (model step 2), through the block map's own flood — never a second
    // physics.
    let mut wet = crate::compiler::assembled::flood(&barriers, &seeds);
    wet.retain(|c| !barriers.contains(c) && world.is_built(*c));

    // Verdict (model step 4): the FOOT cell decides, and the population is the
    // walk. A cell whose own block is fluid is a cell the delve proved a body
    // stands on and the game fills with water before that body arrives.
    let submerged: Vec<[i32; 3]> = reachable
        .iter()
        .copied()
        .filter(|c| wet.contains(c))
        .collect();
    let wading: Vec<[i32; 3]> = reachable
        .iter()
        .copied()
        .filter(|&c| {
            !wet.contains(&c)
                && (wet.contains(&[c[0], c[1] + 1, c[2]])
                    || HORIZ4
                        .iter()
                        .any(|(dx, dz)| wet.contains(&[c[0] + dx, c[1], c[2] + dz])))
        })
        .collect();
    let mut in_pieces: BTreeSet<String> = BTreeSet::new();
    for &c in &submerged {
        for (id, (lo, hi)) in &world.built {
            if (0..3).all(|a| lo[a] <= c[a] && c[a] <= hi[a]) {
                in_pieces.insert(id.clone());
            }
        }
    }
    let drowned: BTreeSet<[i32; 3]> = submerged.iter().copied().collect();
    let drowned_objectives: Vec<(String, [i32; 3])> = world
        .objective_cells
        .iter()
        .filter(|(_, c)| drowned.contains(c))
        .cloned()
        .collect();
    SeaSeepage {
        horizon: "ocean",
        base: world.base,
        pieces: world.built.len(),
        contact_cells: contact.len(),
        sea_waterlogged: waterlogged.len(),
        wet_cells: wet.len(),
        walk_cells: reachable.len(),
        objective_cells,
        wading,
        submerged,
        in_pieces: in_pieces.into_iter().collect(),
        drowned_objectives,
    }
}

impl SeaSeepage {
    /// The `DW0851` violation, or `None`.
    pub fn finding(&self) -> Option<Failure> {
        if self.submerged.is_empty() {
            return None;
        }
        let columns: BTreeSet<(i32, i32)> = self.submerged.iter().map(|c| (c[0], c[2])).collect();
        let highest = self.submerged.iter().map(|c| c[1]).max().unwrap_or(0);
        let sample: Vec<String> = self
            .submerged
            .iter()
            .take(BOUNDARY_LIST_LIMIT)
            .map(|c| format!("{c:?}"))
            .collect();
        let more = self.submerged.len().saturating_sub(sample.len());
        let blame = if self.in_pieces.is_empty() {
            "no placed piece holds them".to_string()
        } else {
            format!("in placed piece(s) {}", self.in_pieces.join(", "))
        };
        let drowned = if self.drowned_objectives.is_empty() {
            "no objective stands in one".to_string()
        } else {
            format!(
                "the objective(s) {} stand in them",
                self.drowned_objectives
                    .iter()
                    .map(|(id, c)| format!("`{id}` at {c:?}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        Some(Failure {
            code: DW_SEA_ENTERS_WALK,
            message: format!(
                "the sea is in the walk region: {n} of the {walk} cell(s) a body was proved to \
                 stand on, across {cols} column(s), {blame}, hold WATER once the world loads — \
                 and {drowned}. This build proves the party walks there and the game gives them \
                 water to stand in. Cells (feet): {sample}{extra}; the highest is y={highest} \
                 against sea level y={level}. Examined the whole walk region ({walk} cell(s), of \
                 which {obj} are named by the critical path) against {contact} cell(s) of open \
                 contact face and {logged} block(s) the sea waterlogs at placement, over {wet} \
                 cell(s) the water reaches inside {pieces} placed piece(s); a further {wading} \
                 walk cell(s) are dry underfoot and touch water (wading — measured, not judged). \
                 WHERE to fix: the piece whose open face or waterloggable block stands below \
                 y={level}, or the placement that put it there. HOW: raise the floor so the walk \
                 plane stands clear of y={level}, close the face the sea comes through, or \
                 author the water yourself so the room IS flooded and every proof downstream \
                 knows it. Do NOT weaken this check or move the path around it: the water \
                 arrives whether or not anything proved it, and a route over these cells is a \
                 route through the sea.",
                n = self.submerged.len(),
                cols = columns.len(),
                sample = sample.join(", "),
                extra = if more > 0 {
                    format!(" (+{more} more)")
                } else {
                    String::new()
                },
                level = crate::compiler::plan::SEA_LEVEL,
                contact = self.contact_cells,
                logged = self.sea_waterlogged,
                wet = self.wet_cells,
                pieces = self.pieces,
                walk = self.walk_cells,
                obj = self.objective_cells,
                wading = self.wading.len(),
            ),
        })
    }

    /// The one line this proof owes its reader, printed whether it found
    /// anything or not: a count only says something when the run that found
    /// nothing prints it too.
    pub fn line(&self) -> String {
        format!(
            "sea-seepage binding: horizon base `{b}` (ambient `{h}`); {walk} walk cell(s) \
             examined, of which {obj} are named by the critical path; {sub} submerged and {wade} \
             wading, over {wet} cell(s) the sea reaches inside {pieces} placed piece(s) from \
             {contact} cell(s) of open contact face and {logged} block(s) the placement \
             waterlogs.",
            b = self.base,
            h = self.horizon,
            walk = self.walk_cells,
            obj = self.objective_cells,
            sub = self.submerged.len(),
            wade = self.wading.len(),
            wet = self.wet_cells,
            pieces = self.pieces,
            contact = self.contact_cells,
            logged = self.sea_waterlogged,
        )
    }

    /// The binding ledger (`validation/sea-seepage.json`): what was examined, not
    /// only what was found — so a build that looked at a walk region of zero says
    /// so, and one that wades says how far and where.
    pub fn ledger(&self) -> serde_json::Value {
        serde_json::json!({
            "horizon_base": self.base,
            "horizon": self.horizon,
            "pieces_examined": self.pieces,
            "contact_face_cells": self.contact_cells,
            "blocks_the_sea_waterlogs": self.sea_waterlogged,
            "cells_the_sea_reaches": self.wet_cells,
            "walk_cells_examined": self.walk_cells,
            "critical_path_cells_examined": self.objective_cells,
            "walk_cells_submerged": self.submerged.len(),
            "walk_cells_wading": self.wading.len(),
            "submerged_cells": self.submerged,
            "wading_cells": self.wading,
            "drowned_objectives": self.drowned_objectives
                .iter()
                .map(|(id, c)| serde_json::json!({"objective": id, "cell": c}))
                .collect::<Vec<_>>(),
            "in_pieces": self.in_pieces,
            "verdict": if self.finding().is_some() { "fail" } else { "pass" },
        })
    }
}

#[cfg(test)]
mod tests {

    fn plate_with_a_source_at_the_edge() -> World {
        let mut blocks: BTreeMap<[i32; 3], String> = BTreeMap::new();
        for x in 0..3 {
            for z in 0..3 {
                blocks.insert([x, 63, z], "minecraft:stone".to_string());
            }
        }
        blocks.insert([2, 64, 1], "minecraft:water".to_string());
        let occ = crate::compiler::assembled::occupancy_of(blocks, &BTreeSet::new());
        World::from_occupancy(occ, Premises::geometry_only())
    }

    /// **The whole rule, both horizons, one geometry.** The piece is identical;
    /// only the world-generator premise changes, exactly as `DW0322` is stated.
    #[test]
    fn fluid_off_the_built_world_is_a_finding_under_void_and_not_under_ocean_dw0318() {
        // void: the water runs off the plate into columns nothing built.
        let void = plate_with_a_source_at_the_edge().with_ambient(Ambient::Void, plate_built());
        let m = measure_fluid_escape(&void);
        assert_eq!(m.horizon, "void");
        assert_eq!(m.pieces, 1, "the built volume is the plate");
        assert!(
            m.fluid_cells > m.outside.len(),
            "the binding count is the world's water ({}), not the finding list ({})",
            m.fluid_cells,
            m.outside.len()
        );
        assert!(!m.outside.is_empty(), "water left the plate");
        assert_eq!(
            m.from_pieces,
            vec!["prefab/plate".to_string()],
            "the escape is attributed to the piece it came from"
        );
        let err = m.finding().expect("a void world does not hold this water");
        assert_eq!(err.code, DW_FLUID_LEAVES_WORLD);
        assert!(
            err.message.contains("DW0318") || err.code.id() == "DW0318",
            "the code is DW0318"
        );
        assert!(
            err.message.contains("fluid leaves the built world"),
            "names the hazard:\n{}",
            err.message
        );

        // ocean: the same water meets the sea it depicts. Same cells, no finding.
        let ocean = plate_with_a_source_at_the_edge().with_ambient(
            Ambient::Ocean(Sea {
                level: 62,
                floor_top: 54,
            }),
            plate_built(),
        );
        let o = measure_fluid_escape(&ocean);
        assert_eq!(o.horizon, "ocean");
        assert_eq!(
            o.outside, m.outside,
            "the geometry is identical — only the premise moved"
        );
        assert!(
            o.finding().is_none(),
            "an ocean horizon puts sea under every column the content did not build"
        );
    }

    /// Water that stays inside the pieces is not a finding, under either
    /// horizon: a walled pond is first-class content, and the piece's own
    /// `DW0800` already governs it.
    #[test]
    fn fluid_contained_within_the_built_volume_is_not_a_finding() {
        let mut blocks: BTreeMap<[i32; 3], String> = BTreeMap::new();
        // A 5x5 stone dish with a 3x3 rim: the source cannot get out.
        for x in 0..5 {
            for z in 0..5 {
                blocks.insert([x, 63, z], "minecraft:stone".to_string());
            }
        }
        for x in 0..5 {
            for z in 0..5 {
                if (1..4).contains(&x) && (1..4).contains(&z) {
                    continue;
                }
                blocks.insert([x, 64, z], "minecraft:stone".to_string());
            }
        }
        blocks.insert([2, 64, 2], "minecraft:water".to_string());
        let occ = crate::compiler::assembled::occupancy_of(blocks, &BTreeSet::new());
        let world = World::from_occupancy(occ, Premises::geometry_only()).with_ambient(
            Ambient::Void,
            vec![("prefab/dish".to_string(), ([0, 63, 0], [4, 64, 4]))],
        );
        let m = measure_fluid_escape(&world);
        assert!(m.fluid_cells > 0, "the check BOUND to this world's water");
        assert!(m.outside.is_empty(), "the dish holds it: {:?}", m.outside);
        assert!(m.finding().is_none());
        assert_eq!(m.ledger()["verdict"], "pass");
        assert_eq!(m.ledger()["fluid_cells_examined"], m.fluid_cells);
    }

    /// Water that runs from one piece into the piece placed against it is the
    /// deferral's other answer, and it must be a pass: the neighbour's own bytes
    /// decide those cells, and the neighbour's own `DW0800` governs them.
    #[test]
    fn fluid_running_into_the_piece_next_door_is_that_pieces_business() {
        let mut blocks: BTreeMap<[i32; 3], String> = BTreeMap::new();
        for x in 0..6 {
            for z in 0..3 {
                blocks.insert([x, 63, z], "minecraft:stone".to_string());
            }
        }
        // A rim around the whole 6x3 slab so nothing leaves the pair.
        for x in -1..7 {
            for z in -1..4 {
                if (0..6).contains(&x) && (0..3).contains(&z) {
                    continue;
                }
                blocks.insert([x, 63, z], "minecraft:stone".to_string());
                blocks.insert([x, 64, z], "minecraft:stone".to_string());
                blocks.insert([x, 65, z], "minecraft:stone".to_string());
            }
        }
        blocks.insert([2, 64, 1], "minecraft:water".to_string());
        let occ = crate::compiler::assembled::occupancy_of(blocks, &BTreeSet::new());
        let world = World::from_occupancy(occ, Premises::geometry_only()).with_ambient(
            Ambient::Void,
            vec![
                ("prefab/west".to_string(), ([-1, 63, -1], [2, 65, 3])),
                ("prefab/east".to_string(), ([3, 63, -1], [6, 65, 3])),
            ],
        );
        let m = measure_fluid_escape(&world);
        assert_eq!(m.pieces, 2);
        assert!(m.fluid_cells > 1, "the water spread across the seam");
        assert!(
            m.outside.is_empty(),
            "water in the piece next door is that piece's business: {:?}",
            m.outside
        );
    }

    /// **The escape hatch a synthetic world could have opened, closed by
    /// direction.** A `World` with no built volume knows of no content at all,
    /// so it cannot prove any water contained. It must therefore report EVERY
    /// fluid cell as outside — fail closed — rather than report none and pass.
    /// The opposite default is the one that would have shipped: an empty list
    /// read as "everything is inside".
    #[test]
    fn a_world_with_no_built_volume_proves_nothing_contained() {
        let world = plate_with_a_source_at_the_edge();
        let m = measure_fluid_escape(&world);
        assert_eq!(m.pieces, 0, "nothing declared where the content is");
        assert_eq!(
            m.outside.len(),
            m.fluid_cells,
            "with no built volume, no cell can be shown contained"
        );
        assert!(
            m.from_pieces.is_empty(),
            "and nothing can be blamed for it either"
        );
        assert!(m.finding().is_some(), "fails closed under void");
    }

    // -----------------------------------------------------------------------
    // DW0851 — the ambient sea inside the built volume
    // -----------------------------------------------------------------------

    /// A room sunk under the waterline, as a piece box the sea presses against on
    /// every face: floor top at `y=60` (walk plane 61), walls up to `y=63`, a
    /// ceiling at 64. `opening` cells are punched out of the west wall.
    fn sunken_room(opening: &[[i32; 3]]) -> (BTreeSet<[i32; 3]>, Vec<Bbox>) {
        let mut solid = BTreeSet::new();
        for x in 0..8 {
            for z in 0..8 {
                solid.insert([x, 60, z]); // floor
                solid.insert([x, 64, z]); // ceiling
                for y in 61..=63 {
                    if x == 0 || x == 7 || z == 0 || z == 7 {
                        solid.insert([x, y, z]); // walls
                    }
                }
            }
        }
        for c in opening {
            solid.remove(c);
        }
        (solid, vec![([0, 60, 0], [7, 64, 7])])
    }

    /// **A walk plane at exactly the waterline is a finding.** This is the
    /// fixture that decides the verdict's predicate, and it changed.
    ///
    /// A plate whose top is at `sea_level - 1` inside a piece box that carries on
    /// upward: the walk plane sits at exactly the waterline, INSIDE the built
    /// volume, with the open sea against its western face. The sea runs onto that
    /// plate — every cell of it — so a body proved to stand there stands in
    /// water.
    ///
    /// The verdict used to be the HEAD cell, on the argument that a body which
    /// can breathe where it was proved to stand is not a contradiction. The field
    /// case is what settled it the other way: a two-scene delve whose two
    /// objectives stood at `[260,61,4]` and `[260,61,8]`, both `minecraft:water`
    /// in the delivered world, both dry to the head, and `walk_cells_wading: 0`
    /// printed over them. The island convention already says where a shore's land
    /// plane goes — one block ABOVE the waterline — and this is that rule read
    /// forwards.
    #[test]
    fn sea_seepage_refuses_a_walk_plane_at_the_waterline_dw0851() {
        let mut solid = BTreeSet::new();
        for x in 0..8 {
            for z in 0..8 {
                for y in 60..=61 {
                    solid.insert([x, y, z]);
                }
            }
        }
        let world = ocean(solid, BTreeSet::new(), vec![([0, 60, 0], [7, 66, 7])]);
        let reachable = world.reachable_walkable_rooted(&roots([3, 62, 3]));
        let m = measure_sea_seepage(&world, &reachable);
        assert_eq!(m.horizon, "ocean");
        assert!(
            m.contact_cells > 0,
            "the sea IS touching this piece — a zero here would be the unbound \
             vacuity mode wearing a pass: {m:?}"
        );
        assert!(
            !m.submerged.is_empty(),
            "the walk plane is at the waterline, so the party stands in it: {m:?}"
        );
        assert!(m.finding().is_some(), "and that is refused");
        verify_boundary_safety(&world, &roots([3, 62, 3]))
            .expect_err("a walk plane at sea level is not a floor");
    }

    /// **The shore the convention describes**: the same plate one block higher,
    /// so the walk plane stands at `sea_level + 1`. Dry underfoot, and the cells
    /// beside the water are counted as wading rather than judged.
    ///
    /// The wading count is what makes the pass a measurement: a shore 8 cells
    /// wide and one 2000 cells wide are different maps.
    #[test]
    fn sea_seepage_wades_a_shore_one_block_above_the_sea() {
        // A long plate at the waterline, walled off from the sea except at one
        // end, so the flow's 7-cell decay dies before the far end: the wet cells
        // and the dry-but-touching cells are both in one walk region.
        let mut solid = BTreeSet::new();
        for x in 0..20 {
            for z in 0..4 {
                solid.insert([x, 61, z]); // the plate: walk plane at y=62
                if z == 0 || z == 3 {
                    for y in 62..=64 {
                        solid.insert([x, y, z]); // side walls, so only the end is open
                    }
                }
            }
        }
        for y in 62..=64 {
            for z in 0..4 {
                solid.insert([19, y, z]); // the far end is closed
            }
        }
        let world = ocean(solid, BTreeSet::new(), vec![([0, 60, 0], [19, 66, 3])]);
        let reachable = world.reachable_walkable_rooted(&roots([15, 62, 2]));
        let m = measure_sea_seepage(&world, &reachable);
        assert!(m.contact_cells > 0, "the open end is contact face: {m:?}");
        assert!(m.wet_cells > 0, "the sea runs down the plate: {m:?}");
        assert!(
            !m.wading.is_empty(),
            "the first dry cell beyond the flow's reach touches it: {m:?}"
        );
        assert!(
            m.wading.iter().all(|c| !m.submerged.contains(c)),
            "wading and submerged are disjoint populations: {m:?}"
        );
        assert!(
            m.walk_cells >= m.wading.len() + m.submerged.len(),
            "both are drawn from the walk region: {m:?}"
        );
    }

    /// The finding itself: a room whose walk plane is under the sea, with its west
    /// wall open **up to the waterline**, so the sea comes in over the walkers'
    /// heads. Every cell in it was proved standable and dry before `DW0851`.
    #[test]
    fn sea_seepage_refuses_a_room_the_sea_walks_into_dw0851() {
        let opening: Vec<[i32; 3]> = (61..=62).map(|y| [0, y, 3]).collect();
        let (solid, covered) = sunken_room(&opening);
        let world = ocean(solid, BTreeSet::new(), covered);
        let reachable = world.reachable_walkable_rooted(&roots([3, 61, 3]));
        let m = measure_sea_seepage(&world, &reachable);
        assert!(m.contact_cells > 0, "the opening is contact face: {m:?}");
        assert!(
            !m.submerged.is_empty(),
            "the sea is over the walk plane: {m:?}"
        );
        assert!(
            m.walk_cells >= m.submerged.len(),
            "the finding is a subset of its own denominator: {m:?}"
        );
        assert_eq!(m.pieces, 1);

        let err = verify_boundary_safety(&world, &roots([3, 61, 3]))
            .expect_err("a flooded walk plane is refused");
        assert_eq!(err.code, DW_SEA_ENTERS_WALK);
        assert!(
            err.message.contains("hold WATER once the world loads"),
            "names what is wrong:\n{}",
            err.message
        );
        assert!(
            err.message.contains("piece-0"),
            "names the piece to fix:\n{}",
            err.message
        );
        // The verdict carries its own denominators.
        assert!(
            err.message
                .contains("cell(s) a body was proved to stand on")
                && err.message.contains("cell(s) of open contact face")
                && err
                    .message
                    .contains("block(s) the sea waterlogs at placement"),
            "states its binding counts:\n{}",
            err.message
        );
    }

    /// **The sea gets in through a block, not only through a hole.**
    ///
    /// The room's hull is intact — zero contact face — and a single waterloggable
    /// block stands in the sea's band inside it. `/place template` hands that
    /// block the water already in the cell, so it lands `waterlogged=true` and
    /// spreads. This is the field case's own mechanism: the tidewatch's staircase
    /// came out waterlogged four cells under the surface and put both objectives
    /// in the water through a hull that presented no open face at all, and the
    /// gallery's own `ocean-horizon` point does the same thing with 10 blocks and
    /// 367 water cells.
    #[test]
    fn sea_seepage_refuses_a_sealed_room_the_placement_waterlogs_dw0851() {
        let (solid, covered) = sunken_room(&[]);
        let bars = [3, 61, 5];
        let mut solid = solid;
        solid.insert(bars); // the host block occupies its cell, as iron bars do
        let world = ocean(solid, BTreeSet::new(), covered).with_waterloggable([bars].into());
        let reachable = world.reachable_walkable_rooted(&roots([3, 61, 3]));
        let m = measure_sea_seepage(&world, &reachable);
        assert_eq!(
            m.contact_cells, 0,
            "the hull is watertight — a contact-face model sees nothing here: {m:?}"
        );
        assert_eq!(m.sea_waterlogged, 1, "the placement wets one block: {m:?}");
        assert!(
            !m.submerged.is_empty(),
            "and the water it spreads is on the walk plane: {m:?}"
        );
        let err = verify_boundary_safety(&world, &roots([3, 61, 3]))
            .expect_err("a room the placement floods is refused");
        assert_eq!(err.code, DW_SEA_ENTERS_WALK);
        assert!(
            err.message
                .contains("1 block(s) the sea waterlogs at placement"),
            "names the seed that did it:\n{}",
            err.message
        );
    }

    /// The same room with its wall intact and nothing waterloggable in it:
    /// **zero contact face and zero waterlogged blocks**, and the proof says so.
    /// This is the only honest way this check passes without looking at anything,
    /// and it is two numbers rather than a silence.
    #[test]
    fn sea_seepage_passes_a_watertight_hull_with_a_stated_zero() {
        let (solid, covered) = sunken_room(&[]);
        let world = ocean(solid, BTreeSet::new(), covered);
        let reachable = world.reachable_walkable_rooted(&roots([3, 61, 3]));
        let m = measure_sea_seepage(&world, &reachable);
        assert_eq!(m.contact_cells, 0, "no face is open to the sea: {m:?}");
        assert_eq!(m.sea_waterlogged, 0, "nothing in it can hold water: {m:?}");
        assert_eq!(m.wet_cells, 0);
        assert!(m.wading.is_empty());
        assert!(m.submerged.is_empty());
        assert!(
            m.walk_cells > 0,
            "there IS a walk region — the pass is about it, not about an empty world: {m:?}"
        );
        assert_eq!(m.ledger()["verdict"], "pass");
        assert_eq!(m.ledger()["contact_face_cells"], 0);
        assert!(
            m.line().contains("walk cell(s) examined"),
            "the line states the denominator: {}",
            m.line()
        );
    }

    /// A hole below the waterline that never reaches it wets the walk plane and
    /// no heads — vanilla water does not climb, and neither does this model. The
    /// feet are in it, which is the finding; the head cells at `y=62` stay dry,
    /// which is why the old head-cell verdict said nothing.
    #[test]
    fn sea_seepage_water_entering_below_the_surface_does_not_rise() {
        let (solid, covered) = sunken_room(&[[0, 61, 3]]);
        let world = ocean(solid, BTreeSet::new(), covered);
        let reachable = world.reachable_walkable_rooted(&roots([3, 61, 3]));
        let m = measure_sea_seepage(&world, &reachable);
        assert!(m.contact_cells > 0, "the hole is contact face: {m:?}");
        assert!(m.wet_cells > 0, "water came in: {m:?}");
        assert!(
            m.submerged.iter().all(|c| c[1] == 61),
            "the water cannot rise to y=62 from a y=61 hole — every wet walk cell \
             is on the plane the hole is on: {m:?}"
        );
        assert!(
            !m.submerged.is_empty(),
            "the floor it reached is the floor the party stands on: {m:?}"
        );
    }

    /// Under `horizon: void` there is no ambient sea, so the proof reports zeroes
    /// **and names the horizon it reported them for** — a reader can tell "no sea"
    /// from "not run".
    #[test]
    fn sea_seepage_under_void_reports_zeroes_and_says_which_horizon() {
        let (solid, covered) = sunken_room(&[[0, 61, 3]]);
        let world = World::from_solid_cells(solid).with_ambient(Ambient::Void, built(covered));
        let reachable = world.reachable_walkable_rooted(&roots([3, 61, 3]));
        let m = measure_sea_seepage(&world, &reachable);
        assert_eq!(m.horizon, "void");
        assert_eq!(m.contact_cells, 0);
        assert_eq!(m.wet_cells, 0);
        assert!(m.submerged.is_empty());
        assert_eq!(m.pieces, 1, "it still says how much world it looked at");
        assert_eq!(m.ledger()["horizon"], "void");
    }
}
