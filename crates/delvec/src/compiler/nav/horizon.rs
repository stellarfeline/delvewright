//! Boundary safety against the world's horizon: no step off the proven ground
//! leaves the world or strands a swimmer (`DW0322`).

use super::*;
use crate::compiler::failure::Failure;
use std::collections::{BTreeMap, BTreeSet};

/// How many individual violations a `DW0322` report names before summarising the
/// remainder as a count. A boundary failure is systemic by nature — one stripped
/// berm is hundreds of exposed columns — and hundreds of identical lines are
/// noise, not information (the `DW0354` aggregation precedent, `edit::check_support`).
/// Aborting at the first one instead hid the *scale*, which is the single most
/// useful fact about the failure: "one cell" and "the whole coastline" call for
/// completely different fixes.
pub(in crate::compiler::nav) const BOUNDARY_LIST_LIMIT: usize = 6;

/// How far past the placed geometry the ocean-stranding search window extends
/// before the sea counts as **open sea** (see [`verify_boundary_safety`]). Any
/// margin ≥ 1 works: the ring beyond the window is untouched ambient water in
/// every direction, so every body that reaches it is one and the same sea.
const OPEN_SEA_MARGIN: i32 = 2;

/// Assert the reachable walk region's boundary is safe (spec-0017 boundary
/// safety; [`DW_EDIT_BORDERS_VOID`]). `starts` are the reachability roots (the
/// plan's resolved anchors, each carrying the piece that declares it — see
/// [`AnchorRoot`], and the same roots the relight pass floods from). Run after
/// every edit batch, and once over the finished world for every campaign that
/// assembles one.
///
/// The premise is the world's [`Ambient`] — what a column the compiler modelled
/// nothing into actually contains — and the rule follows from it:
///
/// **`Ambient::Void`** (unchanged, byte-identical semantics). A neighbour column
/// is a void drop when the player could enter it — its feet and head cells are
/// clear (a closed fence gate counts as enterable: opening it is an
/// adventure-legal right-click) — and **nothing anywhere below** would arrest
/// the fall: no solid, no 1.5-tall barrier top, no gate top, no water. A deep
/// drop onto real geometry is legal (that is falling, not leaving the world);
/// only a bottomless column is an error.
///
/// **`Ambient::Ocean`** — the *stranding* invariant. The superflat's bedrock
/// floor makes every column fall-arresting, so the question is never "can the
/// player fall out" but "can the player get back". The model:
///
/// 1. **Entering.** A reachable walkable cell `c` puts the player in the sea if
///    some horizontally adjacent column is enterable at `c`'s level (feet + head
///    clear of solids and 1.5-tall barriers — water does *not* block walking in)
///    and that column is open, between `c`'s level and the sea surface, all the
///    way to ambient water. Whether the player walks in, wades in or falls from a
///    cliff, they end up afloat: vanilla buoyancy puts a swimmer at the surface
///    plane, `sea.level`.
/// 2. **The sea.** A surface cell (`y == sea.level`) is swimmable when it is not
///    solid/tall and is either ambient water or authored water (a lagoon at sea
///    level is physically the same plane). Surface cells are 4-connected into
///    **bodies**; a body that reaches the edge of the search window is the open
///    sea, and all such bodies are one (the ring beyond the window is untouched
///    ambient water). Connectivity is taken on the surface plane only — a diver
///    might swim under a land bridge into another body, which this model
///    deliberately does not count on.
/// 3. **Climbing out.** A body is escapable when some surface cell of it is
///    horizontally adjacent to a **proven reachable walkable** cell whose feet
///    are at `sea.level` (wade out of the shallows onto a rim one block below the
///    waterline) or at `sea.level + 1` (the canonical beach: land flush with the
///    sea surface). A ledge higher than that is a wall to a swimmer, and a
///    boat/blocks are not available in adventure mode.
///
/// A body the player can enter and cannot climb out of is the violation.
///
/// ## Why the climb-out band stays **cell-level** under partial floor heights
///
/// The step rule reasons in sixteenths ([`World::feet_16_fp`]), but this
/// band deliberately does not. A partial floor can only ever *lower* the standing
/// surface inside its own cell (`feet_16(c) ≤ c.y · 16`), so:
///
/// - inside the band, refining `level` / `level + 1` to a true feet height never
///   flips a verdict — a swimmer climbing onto a slab at `level + 0.5` has an
///   easier exit than onto full ground at `level + 1`, and the body is escapable
///   either way;
/// - the only refinement that *could* flip one is admitting a cell at
///   `level + 2` whose partial support drops its feet back into jump range. That
///   would mark **more** bodies escapable, i.e. weaken the stranding proof.
///
/// So the cell-level band is already the conservative reading of the sixteenth
/// model, and tightening it here could only ever lose a `DW0322` that should
/// fire. The two models compose without interacting: partial heights change
/// *which cells are reachable* (via [`World::neighbors_fp`], feeding `reachable`
/// above), never *what counts as a climb-out*.
/// **The fluid proof runs first, and it runs inside here.**
///
/// `boundary_void`'s per-column fall-arrest scan counts a flooded cell as
/// arrest, so a bottomless column with a waterfall running down it reads as
/// *supported* and this proof goes quiet on exactly the columns the water
/// escaped through. Escaping fluid is therefore a false premise of this proof,
/// the way an unsettled gravity block is of everything downstream of `DW0313`.
///
/// That constraint used to be held by the ORDER OF TWO STATEMENTS at each of the
/// two call sites, plus a comment saying why. Source order is not a mechanism:
/// it is a checklist item that survives only until somebody inserts a third gate
/// into the same function — which is precisely what happened when tiled-zone
/// placement landed, and nothing would have said so. So the sequence is a fact
/// about this proof rather than a fact about its callers, and a caller can no
/// longer get it wrong: there is no order to get wrong.
///
/// The masking is not a corner case that a cleverer fixture could dodge. Under a
/// void horizon the flood model spreads without a floor to stop it, so escaped
/// water reaches essentially every column and silences essentially every hit —
/// which is why no black-box test can tell the two orders apart, and why this is
/// structural instead of a test. [`boundary_only`] is the unsequenced proof,
/// kept so the masking can still be DEMONSTRATED rather than asserted.
pub fn verify_boundary_safety(world: &World, starts: &[AnchorRoot]) -> Result<(), Failure> {
    if let Some(e) = measure_fluid_escape(world).finding() {
        return Err(e);
    }
    // The sea-seepage proof (`DW0851`) sits here for the same reason the fluid
    // one does, and it is the same kind of reason: a walk region the sea is about
    // to fill is a FALSE PREMISE of the stranding proof below, which reasons about
    // where a body can stand and climb out. Under it, an interior room proved
    // standable and dry is a room the stranding proof politely finds no fault
    // with. Sequencing lives here, in the proof, so no caller has an order to get
    // wrong — and the walk region is computed once and handed to both, so the two
    // cannot end up judging different sets of cells.
    let reachable = world.reachable_walkable_rooted(starts);
    if let Some(e) = measure_sea_seepage(world, &reachable).finding() {
        return Err(e);
    }
    boundary_from(world, &reachable)
}

/// Boundary safety alone, on a world whose fluid has already been accounted for.
///
/// Split out of [`verify_boundary_safety`] for one reason: the masking that
/// makes the sequence load-bearing has to be showable. A test that wants to see
/// this proof go quiet on a flooded world calls this; nothing else should — and
/// `#[cfg(test)]` is what makes "nothing else" a fact rather than a request.
#[cfg(test)]
fn boundary_only(world: &World, starts: &[AnchorRoot]) -> Result<(), Failure> {
    let reachable = world.reachable_walkable_rooted(starts);
    boundary_from(world, &reachable)
}

/// Boundary safety over an already-computed walk region.
fn boundary_from(world: &World, reachable: &BTreeSet<[i32; 3]>) -> Result<(), Failure> {
    match &world.ambient {
        Ambient::Void => boundary_void(world, reachable),
        Ambient::Ocean(sea) => boundary_ocean(world, reachable, sea),
    }
}

/// Boundary safety under [`Ambient::Void`]: no reachable walkable cell may
/// border a bottomless column. Every violation is collected (see
/// [`BOUNDARY_LIST_LIMIT`]) so one report shows the scale of the breach.
fn boundary_void(world: &World, reachable: &BTreeSet<[i32; 3]>) -> Result<(), Failure> {
    // Per-column lowest fall-arresting cell: solid, tall barrier, use-gate, or
    // flooded — anything vanilla stops a falling player on (or in).
    let mut col_min: BTreeMap<(i32, i32), i32> = BTreeMap::new();
    for set in [&world.solid, &world.tall, &world.use_gates, &world.flooded] {
        for c in set.iter() {
            col_min
                .entry((c[0], c[2]))
                .and_modify(|m| *m = (*m).min(c[1]))
                .or_insert(c[1]);
        }
    }
    // (edge cell, void column entered) pairs, in deterministic BTreeSet order.
    let mut hits: Vec<([i32; 3], [i32; 3])> = Vec::new();
    let mut columns: BTreeSet<(i32, i32)> = BTreeSet::new();
    for &cell in reachable {
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = [cell[0] + dx, cell[1], cell[2] + dz];
            let head = [n[0], n[1] + 1, n[2]];
            // Enterable: feet + head clear of solids/talls/water. A use-gate
            // cell is deliberately enterable (the player can open it and walk
            // through — a gate onto a bottomless drop is exactly the hazard).
            let blocked = |c: [i32; 3]| {
                world.solid.contains(&c) || world.tall.contains(&c) || world.flooded.contains(&c)
            };
            if blocked(n) || blocked(head) {
                continue;
            }
            let has_support = col_min
                .get(&(n[0], n[2]))
                .is_some_and(|&lowest| lowest < n[1]);
            if !has_support {
                hits.push((cell, n));
                columns.insert((n[0], n[2]));
            }
        }
    }
    if hits.is_empty() {
        return Ok(());
    }
    let mut listing = String::new();
    for (cell, n) in hits.iter().take(BOUNDARY_LIST_LIMIT) {
        listing.push_str(&format!("\n  - {cell:?} → void drop at {n:?}"));
    }
    if hits.len() > BOUNDARY_LIST_LIMIT {
        listing.push_str(&format!(
            "\n  - … and {} more",
            hits.len() - BOUNDARY_LIST_LIMIT
        ));
    }
    let first = hits[0].1;
    Err(Failure {
        code: DW_EDIT_BORDERS_VOID,
        message: format!(
            "boundary safety (spec-0017): {} reachable walkable cell(s) border a void drop over \
             {} distinct column(s) — one step off the proven ground falls out of the world:{}\n\
             There is no physical boundary here — either an edit stripped one or the scene ground \
             never had an edge: extend the terrain under the exposed edge (fill/morph a slope or \
             outcrop below {first:?}) or reinstate a barrier shape; do NOT weaken this check or \
             reroute the path to sidestep it",
            hits.len(),
            columns.len(),
            listing,
        ),
    })
}

/// One 4-connected body of sea-surface cells, plus what the walk region does
/// with it (see [`verify_boundary_safety`]'s ocean model).
struct SeaBody {
    /// Reaches the search-window edge ⇒ it is the open sea, and every other
    /// open body is the same water.
    open: bool,
    /// Some surface cell of the body is adjacent to a reachable walkable cell at
    /// `sea.level` or `sea.level + 1`.
    escapable: bool,
    /// Reachable walkable cells from which the player enters this body, in
    /// deterministic order.
    entries: BTreeSet<[i32; 3]>,
    /// A representative surface cell (the smallest, for a stable message).
    sample: [i32; 3],
    /// Surface cells in the body.
    size: usize,
}

/// The sea-surface bodies of an ocean world, labelled, with where the walk region
/// `reachable` enters each and climbs out of it — the one labelling
/// [`boundary_ocean`] judges stranding over and [`open_sea_entry`] reads.
fn sea_bodies(world: &World, reachable: &BTreeSet<[i32; 3]>, sea: &Sea) -> Vec<SeaBody> {
    let level = sea.level;
    let Some(([min_x, min_z], [max_x, max_z])) = ocean_window(world) else {
        return Vec::new(); // nothing placed: open sea everywhere, nothing to strand
    };
    let w = (max_x - min_x + 1) as usize;
    let d = (max_z - min_z + 1) as usize;
    let idx = |x: i32, z: i32| (x - min_x) as usize * d + (z - min_z) as usize;
    let inside = |x: i32, z: i32| (min_x..=max_x).contains(&x) && (min_z..=max_z).contains(&z);
    let blocked = |c: [i32; 3]| world.solid.contains(&c) || world.tall.contains(&c);
    let swimmable = |x: i32, z: i32| {
        let c = [x, level, z];
        !blocked(c) && (world.flooded.contains(&c) || world.ambient_water(c))
    };

    // --- label the sea-surface bodies (deterministic scan + BFS) -------------
    const NONE: u32 = u32::MAX;
    let mut label = vec![NONE; w * d];
    let mut bodies: Vec<SeaBody> = Vec::new();
    for x in min_x..=max_x {
        for z in min_z..=max_z {
            if label[idx(x, z)] != NONE || !swimmable(x, z) {
                continue;
            }
            let id = bodies.len() as u32;
            let mut body = SeaBody {
                open: false,
                escapable: false,
                entries: BTreeSet::new(),
                sample: [x, level, z],
                size: 0,
            };
            let mut queue = std::collections::VecDeque::from([(x, z)]);
            label[idx(x, z)] = id;
            while let Some((cx, cz)) = queue.pop_front() {
                body.size += 1;
                if cx == min_x || cx == max_x || cz == min_z || cz == max_z {
                    body.open = true;
                }
                for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let (nx, nz) = (cx + dx, cz + dz);
                    if !inside(nx, nz) || label[idx(nx, nz)] != NONE || !swimmable(nx, nz) {
                        continue;
                    }
                    label[idx(nx, nz)] = id;
                    queue.push_back((nx, nz));
                }
            }
            bodies.push(body);
        }
    }
    if bodies.is_empty() {
        return bodies;
    }

    // --- where the walk region touches the water ----------------------------
    for &cell in reachable {
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = [cell[0] + dx, cell[1], cell[2] + dz];
            // Climb-out: standing at (or one above) the waterline beside water.
            if (cell[1] == level || cell[1] == level + 1) && inside(n[0], n[2]) {
                let id = label[idx(n[0], n[2])];
                if id != NONE {
                    bodies[id as usize].escapable = true;
                }
            }
            // Entry: an enterable neighbour column that is open all the way to
            // the sea surface. Water does not block walking in.
            if blocked(n) || blocked([n[0], n[1] + 1, n[2]]) || !inside(n[0], n[2]) {
                continue;
            }
            let id = label[idx(n[0], n[2])];
            if id == NONE {
                continue;
            }
            let (lo, hi) = if n[1] > level {
                (level + 1, n[1])
            } else {
                (n[1], level)
            };
            if (lo..=hi).any(|y| blocked([n[0], y, n[2]])) {
                continue;
            }
            bodies[id as usize].entries.insert(cell);
        }
    }

    bodies
}

/// **Where a body can walk into the open sea** (spec-0092 §10): the first
/// reachable walkable cell, in cell order, from which a body enters a sea body
/// that reaches the search window's edge — `None` under a void horizon, or when
/// every water body the walk region enters is enclosed. Read by `DW0960`: a
/// boundary that does not return is legal only where this is `None`.
pub fn open_sea_entry(world: &World, starts: &[AnchorRoot]) -> Option<[i32; 3]> {
    let Ambient::Ocean(sea) = &world.ambient else {
        return None;
    };
    let reachable = world.reachable_walkable_rooted(starts);
    sea_bodies(world, &reachable, sea)
        .iter()
        .filter(|b| b.open)
        .flat_map(|b| b.entries.iter().copied())
        .min()
}

/// Boundary safety under [`Ambient::Ocean`]: the stranding invariant. See
/// [`verify_boundary_safety`] for the model this implements.
fn boundary_ocean(world: &World, reachable: &BTreeSet<[i32; 3]>, sea: &Sea) -> Result<(), Failure> {
    let level = sea.level;
    let bodies = sea_bodies(world, reachable, sea);
    if bodies.is_empty() {
        return Ok(());
    }

    // Every body that reaches the window edge is the same open sea: one climb-out
    // anywhere on the coast serves all of them.
    let open_escapable = bodies.iter().any(|b| b.open && b.escapable);
    let stranding: Vec<&SeaBody> = bodies
        .iter()
        .filter(|b| !b.entries.is_empty() && !b.escapable && !(b.open && open_escapable))
        .collect();
    if stranding.is_empty() {
        return Ok(());
    }

    let shores: usize = stranding.iter().map(|b| b.entries.len()).sum();
    let mut listing = String::new();
    let mut listed = 0usize;
    for b in &stranding {
        for cell in b.entries.iter() {
            if listed == BOUNDARY_LIST_LIMIT {
                break;
            }
            listing.push_str(&format!(
                "\n  - {cell:?} → the sea at {:?} ({})",
                b.sample,
                if b.open { "open sea" } else { "enclosed water" }
            ));
            listed += 1;
        }
    }
    if shores > listed {
        listing.push_str(&format!("\n  - … and {} more", shores - listed));
    }
    let first = stranding[0]
        .entries
        .iter()
        .next()
        .copied()
        .unwrap_or_default();
    Err(Failure {
        code: DW_EDIT_BORDERS_VOID,
        message: format!(
            "boundary safety (spec-0017, `horizon: ocean`): {shores} reachable walkable cell(s) \
             let the player into {} body/bodies of water ({} surface cell(s)) with NO way back \
             ashore — nothing in an ocean world falls out of the world, but a swimmer who cannot \
             climb out is stranded there for the rest of the delve:{}\n\
             A climb-out is a proven-walkable cell at y={level} (a rim one block under the \
             waterline: wade out) or y={} (land flush with the sea surface) beside the water. \
             Give the shoreline near {first:?} such a step — a beach, a bank, a ladder-free \
             landing — or wall the edge off so the player cannot enter the water there; do NOT \
             weaken this check",
            stranding.len(),
            stranding.iter().map(|b| b.size).sum::<usize>(),
            listing,
            level + 1,
        ),
    })
}

/// The x/z window the ocean stranding search runs over: every placed piece and
/// every modelled cell, inflated by [`OPEN_SEA_MARGIN`]. `None` when the world is
/// completely empty. Beyond the window the ambient sea is uniform in every
/// direction, so a body that reaches the edge is the open sea.
fn ocean_window(world: &World) -> Option<([i32; 2], [i32; 2])> {
    let mut lo = [i32::MAX; 2];
    let mut hi = [i32::MIN; 2];
    let mut note = |x: i32, z: i32| {
        lo[0] = lo[0].min(x);
        lo[1] = lo[1].min(z);
        hi[0] = hi[0].max(x);
        hi[1] = hi[1].max(z);
    };
    for (_, (bmin, bmax)) in &world.built {
        note(bmin[0], bmin[2]);
        note(bmax[0], bmax[2]);
    }
    for set in [&world.solid, &world.tall, &world.use_gates, &world.flooded] {
        for c in set.iter() {
            note(c[0], c[2]);
        }
    }
    if lo[0] > hi[0] {
        return None;
    }
    Some((
        [lo[0] - OPEN_SEA_MARGIN, lo[1] - OPEN_SEA_MARGIN],
        [hi[0] + OPEN_SEA_MARGIN, hi[1] + OPEN_SEA_MARGIN],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::nav::testkit::*;
    use std::collections::{BTreeMap, BTreeSet};

    /// **Why `DW0318` runs before `DW0322`, demonstrated rather than asserted.**
    ///
    /// `boundary_void` counts a flooded cell as fall-arrest, so a bottomless
    /// column with a waterfall running down it reads as *supported*. The same
    /// plate is a `DW0322` void drop when it is dry and passes `DW0322` when its
    /// edge is leaking — the escaping water hides the hole it made. Only
    /// `DW0318` sees it, which is why it is asked first.
    #[test]
    fn escaping_water_masks_the_boundary_proof_which_is_why_it_runs_first() {
        // Dry: every rim cell borders a bottomless column → DW0322.
        let mut dry: BTreeMap<[i32; 3], String> = BTreeMap::new();
        for x in 0..3 {
            for z in 0..3 {
                dry.insert([x, 63, z], "minecraft:stone".to_string());
            }
        }
        let dry_world = World::from_occupancy(
            crate::compiler::assembled::occupancy_of(dry.clone(), &BTreeSet::new()),
            Premises::geometry_only(),
        )
        .with_ambient(Ambient::Void, plate_built());
        let err = verify_boundary_safety(&dry_world, &roots([1, 64, 1]))
            .expect_err("a dry plate edge is a void drop");
        assert_eq!(err.code, DW_EDIT_BORDERS_VOID);

        // Wet: one source on the plate floods every surrounding column, and
        // `col_min` now finds arrest in each of them.
        let mut wet = dry;
        wet.insert([1, 64, 1], "minecraft:water".to_string());
        let wet_world = World::from_occupancy(
            crate::compiler::assembled::occupancy_of(wet, &BTreeSet::new()),
            Premises::geometry_only(),
        )
        .with_ambient(Ambient::Void, plate_built());
        assert!(
            boundary_only(&wet_world, &roots([1, 64, 1])).is_ok(),
            "the leak silences the boundary proof — this is the masking, not a pass"
        );
        assert!(
            measure_fluid_escape(&wet_world).finding().is_some(),
            "and DW0318 is the only proof left that sees it"
        );
        // …which is why the sequence is inside `verify_boundary_safety` rather
        // than at its call sites: the same world, through the entry point every
        // caller uses, reports the leak instead of the silence. A caller cannot
        // put these two in the wrong order because there is no order left to
        // put them in.
        let err = verify_boundary_safety(&wet_world, &roots([1, 64, 1]))
            .expect_err("the sequenced entry point reports the leak");
        assert_eq!(err.code, DW_FLUID_LEAVES_WORLD);
    }

    /// Boundary safety (spec-0017 invariant 4): a walkable platform edge whose
    /// neighbour column has NOTHING below is a void drop → `DW0322`; ringing the
    /// platform with a 2-high (unjumpable) rim, or giving the neighbour column
    /// real geometry anywhere below (a deep drop onto land is falling, not
    /// leaving the world), passes.
    #[test]
    fn boundary_safety_flags_a_walkable_edge_over_void_dw0322() {
        // A bare 3×3 platform: every rim cell borders bottomless columns.
        let mut solid = BTreeSet::new();
        for x in 0..3 {
            for z in 0..3 {
                solid.insert([x, 63, z]);
            }
        }
        let world = World::from_solid_cells(solid);
        let err =
            verify_boundary_safety(&world, &roots([1, 64, 1])).expect_err("edge borders void");
        assert_eq!(err.code, DW_EDIT_BORDERS_VOID);
        assert!(err.message.contains("void drop"), "names the hazard");
    }

    #[test]
    fn boundary_safety_accepts_rimmed_platforms_and_deep_drops() {
        // (a) The same platform ringed by a 2-high rim (feet + head blocked, and
        // the rim top is +2 — unclimbable, so it never joins the walkable set).
        let mut solid = BTreeSet::new();
        for x in 0..3 {
            for z in 0..3 {
                solid.insert([x, 63, z]);
            }
        }
        for x in -1..4 {
            for z in -1..4 {
                if (0..3).contains(&x) && (0..3).contains(&z) {
                    continue;
                }
                solid.insert([x, 64, z]);
                solid.insert([x, 65, z]);
            }
        }
        let world = World::from_solid_cells(solid);
        verify_boundary_safety(&world, &roots([1, 64, 1])).expect("a 2-high rim holds the line");

        // (b) A single floor cell whose four neighbour columns all have geometry
        // far below: a deep drop is legal (falling, not leaving the world).
        let mut solid = BTreeSet::new();
        solid.insert([0, 63, 0]);
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            solid.insert([dx, 10, dz]);
        }
        let world = World::from_solid_cells(solid);
        verify_boundary_safety(&world, &roots([0, 64, 0])).expect("deep drops onto land are legal");
    }

    // -----------------------------------------------------------------------
    // DW0322 aggregation + the ocean horizon's stranding invariant
    // -----------------------------------------------------------------------

    /// `DW0322` reports **every** violation of a run, not the first: a stripped
    /// boundary is systemic, and the scale of the breach is the most useful fact
    /// about it. The bare 3×3 platform exposes 12 edge/void pairs over 12 columns
    /// (4 corners × 2 + 4 edges × 1); the message counts all of them and lists
    /// [`BOUNDARY_LIST_LIMIT`] before summarising the rest.
    #[test]
    fn boundary_safety_aggregates_every_void_drop_dw0322() {
        let mut solid = BTreeSet::new();
        for x in 0..3 {
            for z in 0..3 {
                solid.insert([x, 63, z]);
            }
        }
        let world = World::from_solid_cells(solid);
        let err =
            verify_boundary_safety(&world, &roots([1, 64, 1])).expect_err("edge borders void");
        assert_eq!(err.code, DW_EDIT_BORDERS_VOID);
        assert!(
            err.message.contains("12 reachable walkable cell(s)"),
            "counts every violation, not just the first:\n{}",
            err.message
        );
        assert!(
            err.message.contains("12 distinct column(s)"),
            "counts the exposed columns:\n{}",
            err.message
        );
        assert_eq!(
            err.message.matches("void drop at").count(),
            BOUNDARY_LIST_LIMIT,
            "listing is bounded:\n{}",
            err.message
        );
        assert!(
            err.message.contains("and 6 more"),
            "summarises the tail:\n{}",
            err.message
        );
    }

    /// A `size`×`size` island of one solid plate whose top block is at `top`,
    /// inside a piece AABB spanning y 60..=`top`.
    fn island(size: i32, top: i32) -> (BTreeSet<[i32; 3]>, Vec<Bbox>) {
        let mut solid = BTreeSet::new();
        for x in 0..size {
            for z in 0..size {
                for y in 60..=top {
                    solid.insert([x, y, z]);
                }
            }
        }
        (solid, vec![([0, 60, 0], [size - 1, top, size - 1])])
    }

    /// Ocean horizon (spec-0013), the false-premise fix: the pinned superflat
    /// puts bedrock under *every* column, so a coastline is not a void drop —
    /// the identical geometry is `DW0322` under `horizon: void` and clean under
    /// `horizon: ocean`, because its shore is a canonical beach (land top flush
    /// with the sea surface, walk plane at `sea_level + 1`).
    #[test]
    fn boundary_safety_ocean_beach_is_not_a_void_drop_dw0322() {
        let (solid, covered) = island(8, 62);
        let voidish = World::from_solid_cells(solid.clone());
        let err = verify_boundary_safety(&voidish, &roots([3, 63, 3]))
            .expect_err("under `void` the same coast IS a void drop");
        assert_eq!(err.code, DW_EDIT_BORDERS_VOID);
        assert!(err.message.contains("void drop"));

        let sea = ocean(solid, BTreeSet::new(), covered);
        verify_boundary_safety(&sea, &roots([3, 63, 3]))
            .expect("an ocean beach is swimming, not falling out of the world");
    }

    /// The ocean horizon's replacement invariant: a sheer-cliff coast with no
    /// climb-out anywhere strands the player who steps off it, and that is
    /// `DW0322` — with every shore cell aggregated, not the first one.
    #[test]
    fn boundary_safety_ocean_sheer_cliff_strands_the_player_dw0322() {
        let (solid, covered) = island(8, 70);
        let world = ocean(solid, BTreeSet::new(), covered);
        let err = verify_boundary_safety(&world, &roots([3, 71, 3]))
            .expect_err("a sheer coast cannot be re-climbed");
        assert_eq!(err.code, DW_EDIT_BORDERS_VOID);
        assert!(
            err.message.contains("NO way back ashore"),
            "names the stranding hazard:\n{}",
            err.message
        );
        assert!(
            err.message.contains("open sea"),
            "names the water body:\n{}",
            err.message
        );
        // 8×8 plateau: 28 distinct rim cells touch the water (a corner counts
        // once — the report is per shore *cell*, not per cell/direction pair).
        assert!(
            err.message.contains("28 reachable walkable cell(s)"),
            "aggregates every shore cell:\n{}",
            err.message
        );
        assert!(
            err.message.contains("and 22 more"),
            "bounded listing + tail count:\n{}",
            err.message
        );
    }

    /// The other admitted shoreline profile: a rim one block **under** the
    /// waterline (walk plane at `sea_level`, wade out of the shallows). Both it
    /// and the flush beach pass; a lip two blocks above the surface does not.
    #[test]
    fn boundary_safety_ocean_admits_a_rim_below_the_waterline() {
        let (solid, covered) = island(8, 61);
        let world = ocean(solid, BTreeSet::new(), covered);
        verify_boundary_safety(&world, &roots([3, 62, 3])).expect("wade out of the shallows");

        // One block higher than the flush beach: the swimmer faces a wall.
        let (solid, covered) = island(8, 64);
        let world = ocean(solid, BTreeSet::new(), covered);
        let err = verify_boundary_safety(&world, &roots([3, 65, 3]))
            .expect_err("a lip 2 above the surface is not a climb-out");
        assert_eq!(err.code, DW_EDIT_BORDERS_VOID);
    }

    /// Stranding is proven **per body of water**, not globally: an island whose
    /// outer coast is a perfect beach still fails if it contains an inner pool
    /// the player can walk into and not climb out of. A global "is there a
    /// climb-out anywhere" test would pass this world.
    #[test]
    fn boundary_safety_ocean_enclosed_pool_is_checked_separately_dw0322() {
        // Outer plate: top 62 (flush beach, walk plane 63) over 0..=12.
        let mut solid = BTreeSet::new();
        for x in 0..=12 {
            for z in 0..=12 {
                for y in 60..=62 {
                    solid.insert([x, y, z]);
                }
            }
        }
        // Inner plateau one step up (top 63, walk plane 64) over 3..=9, with a
        // 3×3 shaft at 5..=7 down to a pool at sea level.
        let mut flooded = BTreeSet::new();
        for x in 3..=9 {
            for z in 3..=9 {
                if (5..=7).contains(&x) && (5..=7).contains(&z) {
                    solid.remove(&[x, 62, z]);
                    flooded.insert([x, 62, z]);
                } else {
                    solid.insert([x, 63, z]);
                }
            }
        }
        let world = ocean(solid, flooded, vec![([0, 60, 0], [12, 63, 12])]);
        let err = verify_boundary_safety(&world, &roots([1, 63, 1]))
            .expect_err("the inner pool has 2-high walls all round");
        assert_eq!(err.code, DW_EDIT_BORDERS_VOID);
        assert!(
            err.message.contains("enclosed water"),
            "names the enclosed body, not the (escapable) open sea:\n{}",
            err.message
        );
        assert!(
            !err.message.contains("open sea"),
            "the outer beach is proven fine and must not be reported:\n{}",
            err.message
        );
        assert!(
            err.message.contains("1 body/bodies of water"),
            "exactly one failing body:\n{}",
            err.message
        );

        // Lower the inner plateau to the outer datum and the pool is flush with
        // its bank: the player steps straight back out.
        let mut solid = BTreeSet::new();
        for x in 0..=12 {
            for z in 0..=12 {
                for y in 60..=62 {
                    solid.insert([x, y, z]);
                }
            }
        }
        let mut flooded = BTreeSet::new();
        for x in 5..=7 {
            for z in 5..=7 {
                solid.remove(&[x, 62, z]);
                flooded.insert([x, 62, z]);
            }
        }
        let world = ocean(solid, flooded, vec![([0, 60, 0], [12, 62, 12])]);
        verify_boundary_safety(&world, &roots([1, 63, 1]))
            .expect("a flush pool is a puddle, not a trap");
    }

    // --- partial heights x ocean horizon compose ----------------

    /// The two world models are independent axes and must both stay live in one
    /// `World`: `partial` decides *which cells are reachable*, and
    /// `ambient` (spec-0013) decides *what the unmodelled columns contain*.
    ///
    /// Geometry: an ocean island whose top plate is a course of BOTTOM SLABS, so
    /// the walk plane sits at `sea_level + 0.5` rather than `sea_level + 1`. The
    /// slab cells are still `solid` (feet cell `sea_level + 1`), so the climb-out
    /// band sees the canonical beach and the coast is not a stranding — while the
    /// step rule is simultaneously reasoning in sixteenths over the same cells.
    #[test]
    fn partial_floor_heights_and_the_ocean_horizon_compose() {
        let mut cells: Vec<([i32; 3], &str)> = Vec::new();
        for x in 0..8 {
            for z in 0..8 {
                for y in 60..=61 {
                    cells.push(([x, y, z], "minecraft:stone"));
                }
                // The shore course is a bottom slab: top face at 62.5.
                cells.push(([x, 62, z], "minecraft:oak_slab[type=bottom]"));
            }
        }
        let occ = crate::compiler::assembled::occupancy_of(
            cells.iter().map(|(c, n)| (*c, (*n).to_string())).collect(),
            &BTreeSet::new(),
        );
        // The partial map is populated…
        assert_eq!(
            occ.partial.get(&[3, 62, 3]),
            Some(&8),
            "the slab course must be modelled as a half-height floor"
        );
        let world = World::from_occupancy(occ, Premises::geometry_only()).with_ambient(
            Ambient::Ocean(Sea {
                level: 62,
                floor_top: 54,
            }),
            vec![("island".to_string(), ([0, 60, 0], [7, 62, 7]))],
        );

        // …and BOTH axes are live on the same World: the step rule sees the true
        // feet height (62 + 0.5 → 62·16 + 8 sixteenths) …
        let fp = Footprint::player();
        assert_eq!(
            world.feet_16_fp([3, 63, 3], &fp),
            62 * 16 + 8,
            "feet rest on the slab face, not the cell floor"
        );
        // … while the ocean premise still governs the boundary verdict.
        verify_boundary_safety(&world, &roots([3, 63, 3]))
            .expect("a slab-course beach is a climb-out, not a stranding");

        // The control: the identical geometry under the void premise is still a
        // void-drop error — partial heights do not disturb that verdict either.
        let voidish = World::from_occupancy(
            crate::compiler::assembled::occupancy_of(
                cells.iter().map(|(c, n)| (*c, (*n).to_string())).collect(),
                &BTreeSet::new(),
            ),
            Premises::geometry_only(),
        );
        let err = verify_boundary_safety(&voidish, &roots([3, 63, 3]))
            .expect_err("under `void` the same slab coast is a void drop");
        assert_eq!(err.code, DW_EDIT_BORDERS_VOID);
    }
}
