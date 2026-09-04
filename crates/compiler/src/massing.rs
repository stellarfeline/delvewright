//! L2 massing verbs (spec-0017): declarative control of a pool area's
//! solved jigsaw layout — `swap-piece`, `insert-piece`, `remove-piece`,
//! `rewire-socket`, `reseed-piece` — applied by `Plan::build` immediately
//! after `solve_area`, so **every** downstream pass (anchor resolution, gate
//! reachability, waterline datum, assembly, relight, nav, the L3 detailing
//! replay, emission) sees the massaged layout: the full assembly validation
//! re-runs over it by construction.
//!
//! ## The thin-surface discipline
//!
//! Every verb is expressed through the solver's own primitives — the socket
//! mating rule (`socket_world` + the attach pose derivation), the inclusive
//! AABB overlap test, and `seal_layout` (seals are always **regenerated** from
//! the pieces' mated flags, then rewire overrides patch individual openings).
//! Nothing here invents new geometry rules; a mutation the solver's rules
//! cannot express (a swap that cannot re-mate, an insert that overlaps, a
//! removal that would orphan children) is a loud [`DW_MASSING`] error naming
//! its batch, never a silently deformed layout.
//!
//! `resize-piece` from the spec's initial verb list is **deliberately absent**:
//! the prefab library has no size-parameterized piece primitive to express it
//! through (a `.nbt` structure has one fixed size), so per the no-hack
//! doctrine the verb is excluded until such a primitive exists — `swap-piece`
//! covers the change-to-a-different-sized-variant case first-class.
//!
//! ## Determinism (ADR-0006)
//!
//! `reseed-piece` draws from a `Splitmix64` stream named by the verb's script
//! position (`stream_seed(seed, "edits/<batch-id>/<edit-index>")`), so the
//! same script + seed always re-picks the same variant, and moving the verb
//! deliberately re-rolls it. Everything else is order-determined.

use crate::failure::Failure;
use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::{Campaign, SocketState, WorldEdit};

use crate::registry::PrefabRegistry;
use crate::solver::{
    AreaLayout, Facing, PlacedPiece, Rotation, Splitmix64, aabb_overlap, opening_region,
    seal_layout, socket_world, stream_seed,
};
use delvewright_dsl::DwCode;

/// An L2 massing verb cannot apply to the solved layout (spec-0017): the
/// target area is single-prefab (no jigsaw layout to mass), a piece
/// index/prefab guard mismatches (layout drift), a swap/reseed candidate
/// cannot re-mate every mated socket without overlap, an insert's socket is
/// mated or its piece cannot attach, a removal targets the entry piece or a
/// non-leaf, or a rewire names an out-of-range connector. Build-tier (exit 3).
pub const DW_MASSING: DwCode = DwCode::every_version("DW0324");

/// Whether a stage-7 edit verb is an L2 massing verb (applied at plan time)
/// as opposed to an L3 detailing verb (applied at replay time).
pub fn is_massing_verb(edit: &WorldEdit) -> bool {
    matches!(
        edit,
        WorldEdit::SwapPiece { .. }
            | WorldEdit::InsertPiece { .. }
            | WorldEdit::RemovePiece { .. }
            | WorldEdit::RewireSocket { .. }
            | WorldEdit::ReseedPiece { .. }
    )
}

/// Whether the campaign's edit script has any massing verb for `area_id` —
/// `Plan::build` uses this to reject massing on single-prefab areas.
pub fn targets_area(campaign: &Campaign, area_id: &str) -> bool {
    has_massing_for(campaign, area_id)
}

/// Whether the campaign's edit script has any massing verb for `area_id`.
fn has_massing_for(campaign: &Campaign, area_id: &str) -> bool {
    campaign.world_edits.as_ref().is_some_and(|env| {
        env.content
            .batches
            .iter()
            .any(|b| b.area.as_str() == area_id && b.edits.iter().any(is_massing_verb))
    })
}

/// A massing application's outcome for one area.
#[derive(Default)]
pub struct MassingOutcome {
    /// Each applied batch's affected world AABB (per-batch snapshot framing).
    pub bounds: BTreeMap<String, ([i32; 3], [i32; 3])>,
    /// Socket world cells whose doorways a `rewire-socket sealed` severed —
    /// the DW0306 piece-connectivity graph must not count those edges.
    pub severed: BTreeSet<[i32; 3]>,
}

/// Apply every massing batch targeting `area_id` to its solved layout, in
/// script order, then regenerate the seals (with rewire-open overrides).
/// No-op (and no re-seal) for an area no massing verb targets — its layout
/// and seals stay byte-identical.
pub fn apply(
    campaign: &Campaign,
    area_id: &str,
    layout: &mut AreaLayout,
    prefabs: &PrefabRegistry,
    seed: u64,
) -> Result<MassingOutcome, Failure> {
    let mut out = MassingOutcome::default();
    if !has_massing_for(campaign, area_id) {
        return Ok(out);
    }
    let env = campaign.world_edits.as_ref().expect("has_massing checked");
    // Rewire-OPEN overrides applied after the re-seal: (piece, connector)
    // whose sealed opening is cleared to air.
    let mut rewires: Vec<(usize, usize)> = Vec::new();

    for batch in &env.content.batches {
        if batch.area.as_str() != area_id || !batch.edits.iter().any(is_massing_verb) {
            continue;
        }
        let bid = batch.id.as_str();
        for (ei, edit) in batch.edits.iter().enumerate() {
            let err = |message: String| Failure {
                code: DW_MASSING,
                message,
            };
            match edit {
                WorldEdit::SwapPiece {
                    piece,
                    prefab,
                    with,
                } => {
                    let idx = guard(layout, bid, *piece, prefab.as_str()).map_err(err)?;
                    let b = swap(layout, prefabs, bid, idx, with.as_str(), None).map_err(err)?;
                    grow_bounds(&mut out.bounds, bid, b);
                }
                WorldEdit::ReseedPiece { piece, prefab } => {
                    let idx = guard(layout, bid, *piece, prefab.as_str()).map_err(err)?;
                    let mut stream =
                        Splitmix64::new(stream_seed(seed, &format!("edits/{bid}/{ei}")));
                    let b = reseed(layout, prefabs, campaign, area_id, bid, idx, &mut stream)
                        .map_err(err)?;
                    grow_bounds(&mut out.bounds, bid, b);
                }
                WorldEdit::InsertPiece {
                    at_piece,
                    prefab,
                    socket,
                    insert,
                } => {
                    let idx = guard(layout, bid, *at_piece, prefab.as_str()).map_err(err)?;
                    let b = insert_at(layout, prefabs, bid, idx, *socket as usize, insert.as_str())
                        .map_err(err)?;
                    grow_bounds(&mut out.bounds, bid, b);
                }
                WorldEdit::RemovePiece { piece, prefab } => {
                    let idx = guard(layout, bid, *piece, prefab.as_str()).map_err(err)?;
                    let b = remove(layout, prefabs, bid, idx).map_err(err)?;
                    grow_bounds(&mut out.bounds, bid, b);
                    // Removal shifts indices; pending rewires that referenced
                    // later pieces would silently retarget — reject the mix.
                    if !rewires.is_empty() {
                        return Err(Failure {
                            code: DW_MASSING,
                            message: format!(
                                "world-edits batch `{bid}`: `remove-piece` after a \
                                 `rewire-socket` in the same area's script — removal shifts \
                                 piece indices under the pending rewire. Order the removals \
                                 before the rewires (re-check indices with `delvec snapshot`)"
                            ),
                        });
                    }
                }
                WorldEdit::RewireSocket {
                    piece,
                    prefab,
                    socket,
                    state,
                } => {
                    let idx = guard(layout, bid, *piece, prefab.as_str()).map_err(err)?;
                    let meta =
                        prefabs
                            .get(&layout.pieces[idx].prefab_id)
                            .ok_or_else(|| Failure {
                                code: DW_MASSING,
                                message: format!(
                                    "world-edits batch `{bid}`: prefab `{}` lost its metadata \
                                 mid-plan — compiler bug, escalate",
                                    layout.pieces[idx].prefab_id
                                ),
                            })?;
                    let ci = *socket as usize;
                    if ci >= meta.connectors.len() {
                        return Err(Failure {
                            code: DW_MASSING,
                            message: format!(
                                "world-edits batch `{bid}`: `rewire-socket` names connector \
                                 {ci} but `{}` declares {} connector(s) (0..={}). Check the \
                                 prefab metadata's `connectors` order",
                                layout.pieces[idx].prefab_id,
                                meta.connectors.len(),
                                meta.connectors.len().saturating_sub(1),
                            ),
                        });
                    }
                    let (wp, f) = socket_world(
                        layout.pieces[idx].pos,
                        layout.pieces[idx].rotation,
                        &meta.connectors[ci],
                    )
                    .map_err(|e| Failure {
                        code: DW_MASSING,
                        message: format!("world-edits batch `{bid}`: {}", e.failure.message),
                    })?;
                    let is_mated = layout.pieces[idx].mated[ci];
                    match state {
                        SocketState::Sealed => {
                            // Sealing a doorway is a GRAPH operation, not a
                            // cosmetic fill: unmate both paired sockets so the
                            // re-seal walls both planes AND the DW0306
                            // piece-connectivity graph loses the edge — a
                            // sealed spine is a compile error, not a shipped
                            // dead end.
                            if !is_mated {
                                return Err(Failure {
                                    code: DW_MASSING,
                                    message: format!(
                                        "world-edits batch `{bid}`: `rewire-socket` seals \
                                         connector {ci} of piece {idx}, which is already \
                                         unmated (sealed) — drop the no-op edit"
                                    ),
                                });
                            }
                            layout.pieces[idx].mated[ci] = false;
                            unmate_partner(layout, prefabs, wp, f);
                            let u = f.unit();
                            out.severed.insert(wp);
                            out.severed
                                .insert([wp[0] + u[0], wp[1] + u[1], wp[2] + u[2]]);
                        }
                        SocketState::Open => {
                            // Opening clears an UNMATED socket's fill. A mated
                            // socket is already an open passage; the
                            // completability model deliberately does NOT gain
                            // an edge from a rewired-open socket (conservative
                            // — it may over-block a proof, never over-prove).
                            if is_mated {
                                return Err(Failure {
                                    code: DW_MASSING,
                                    message: format!(
                                        "world-edits batch `{bid}`: `rewire-socket` opens \
                                         connector {ci} of piece {idx}, which is mated (an \
                                         open passage already) — drop the no-op edit"
                                    ),
                                });
                            }
                            rewires.push((idx, ci));
                        }
                    }
                    let region = opening_region(wp, f, meta.connectors[ci].opening);
                    grow_bounds(&mut out.bounds, bid, region);
                }
                _ => {} // L3 detailing verbs are applied at replay time.
            }
        }
    }

    // Regenerate every seal from the massaged pieces' mated flags, then patch
    // the rewired openings.
    layout.seals = seal_layout(prefabs, &layout.pieces);
    for (idx, ci) in rewires {
        let piece = &layout.pieces[idx];
        let Some(meta) = prefabs.get(&piece.prefab_id) else {
            continue;
        };
        let Ok((wp, f)) = socket_world(piece.pos, piece.rotation, &meta.connectors[ci]) else {
            continue;
        };
        let (from, to) = opening_region(wp, f, meta.connectors[ci].opening);
        for seal in &mut layout.seals {
            if seal.from == from && seal.to == to {
                seal.block = "minecraft:air".to_string();
            }
        }
    }
    Ok(out)
}

/// Resolve + guard a piece reference (the same drift discipline as the L3
/// piece-local frame): the index must be in range and the piece must be the
/// declared prefab. A single-prefab area never gets here — `Plan::build`
/// routes only pool areas through massing; the caller rejects the rest.
fn guard(layout: &AreaLayout, bid: &str, piece: u32, prefab: &str) -> Result<usize, String> {
    let idx = piece as usize;
    let placed = layout.pieces.get(idx).ok_or_else(|| {
        format!(
            "world-edits batch `{bid}`: piece index {idx} is out of range — the area placed \
             {} piece(s) (0..={}). The layout has drifted; re-inspect with `delvec snapshot` \
             and update the verb, do NOT guess an index",
            layout.pieces.len(),
            layout.pieces.len().saturating_sub(1),
        )
    })?;
    if placed.prefab_id != prefab {
        return Err(format!(
            "world-edits batch `{bid}`: piece {idx} is `{}`, not the verb's declared \
             `{prefab}` — the solved layout has drifted since this edit was authored. \
             Re-inspect (`delvec snapshot`) and re-target; do NOT just delete the prefab guard",
            placed.prefab_id,
        ));
    }
    Ok(idx)
}

/// The world poses (cell + facing) of a piece's **mated** connectors — the
/// contract any replacement must re-present exactly.
fn mated_poses(
    prefabs: &PrefabRegistry,
    piece: &PlacedPiece,
) -> Result<Vec<([i32; 3], Facing)>, String> {
    let meta = prefabs.get(&piece.prefab_id).ok_or_else(|| {
        format!(
            "prefab `{}` has no metadata (compiler bug)",
            piece.prefab_id
        )
    })?;
    let mut poses = Vec::new();
    for (ci, conn) in meta.connectors.iter().enumerate() {
        if piece.mated.get(ci).copied().unwrap_or(false) {
            let (wp, f) =
                socket_world(piece.pos, piece.rotation, conn).map_err(|e| e.failure.message)?;
            poses.push((wp, f));
        }
    }
    Ok(poses)
}

/// Try to place `candidate` so it presents a connector at every pose in
/// `required` (the swapped-out piece's mated sockets), overlapping no piece
/// except the one at `skip`. On success returns the placement + its mated
/// connector indices. Deterministic: rotations and connectors are tried in
/// fixed order.
fn mate_replacement(
    layout: &AreaLayout,
    prefabs: &PrefabRegistry,
    candidate: &str,
    required: &[([i32; 3], Facing)],
    skip: usize,
) -> Option<PlacedPiece> {
    let meta = prefabs.get(candidate)?;
    let (first_pos, first_facing) = *required.first()?;
    for rot in Rotation::ALL {
        for conn in &meta.connectors {
            let cf = Facing::parse(&conn.facing)?;
            if cf.rotate(rot) != first_facing {
                continue;
            }
            let tl = rot.transform(conn.local_pos);
            let pos = [
                first_pos[0] - tl[0],
                first_pos[1] - tl[1],
                first_pos[2] - tl[2],
            ];
            let (bmin, bmax) = rot.bbox(pos, meta.size());
            if layout
                .pieces
                .iter()
                .enumerate()
                .any(|(i, p)| i != skip && aabb_overlap((&bmin, &bmax), (&p.bbox_min, &p.bbox_max)))
            {
                continue;
            }
            // Every required pose must be presented by some connector.
            let mut mated = vec![false; meta.connectors.len()];
            let mut ok = true;
            for &(rp, rf) in required {
                let hit = meta.connectors.iter().enumerate().find(|(_, c2)| {
                    let Ok((wp, f)) = socket_world(pos, rot, c2) else {
                        return false;
                    };
                    wp == rp && f == rf
                });
                match hit {
                    Some((ci, _)) => mated[ci] = true,
                    None => {
                        ok = false;
                        break;
                    }
                }
            }
            if !ok {
                continue;
            }
            return Some(PlacedPiece {
                prefab_id: candidate.to_string(),
                pos,
                rotation: rot,
                bbox_min: bmin,
                bbox_max: bmax,
                mated,
            });
        }
    }
    None
}

/// `swap-piece`: replace piece `idx` with `with`, re-mating every mated
/// socket at its exact world pose. `exclude_current` (reseed) rejects a
/// no-op re-pick of the same prefab.
fn swap(
    layout: &mut AreaLayout,
    prefabs: &PrefabRegistry,
    bid: &str,
    idx: usize,
    with: &str,
    label: Option<&str>,
) -> Result<([i32; 3], [i32; 3]), String> {
    let verb = label.unwrap_or("swap-piece");
    if prefabs.get(with).is_none() {
        return Err(format!(
            "world-edits batch `{bid}`: `{verb}` target `{with}` is not in the prefab \
             library — only admitted library prefabs can be placed (ADR-0013)"
        ));
    }
    let required = mated_poses(prefabs, &layout.pieces[idx])
        .map_err(|m| format!("world-edits batch `{bid}`: {m}"))?;
    if required.is_empty() {
        return Err(format!(
            "world-edits batch `{bid}`: `{verb}` target piece {idx} has no mated socket — an \
             unmated piece is not part of the jigsaw graph (compiler bug or drifted layout); \
             re-inspect with `delvec snapshot`"
        ));
    }
    let old_bbox = (layout.pieces[idx].bbox_min, layout.pieces[idx].bbox_max);
    let replacement = mate_replacement(layout, prefabs, with, &required, idx).ok_or_else(|| {
        format!(
            "world-edits batch `{bid}`: `{verb}` cannot place `{with}` — no rotation \
             presents a connector at every mated socket of piece {idx} without overlapping \
             another piece. Pick a variant with compatible sockets (see the prefab \
             metadata), or restructure with insert/remove; do NOT force it by deleting \
             sockets from the metadata"
        )
    })?;
    let new_bbox = (replacement.bbox_min, replacement.bbox_max);
    layout.pieces[idx] = replacement;
    Ok(union_bbox(old_bbox, new_bbox))
}

/// `reseed-piece`: re-pick the piece from its area pool's compatible members
/// by seeded weighted draw (excluding the current prefab — a reseed that can
/// only re-pick itself is an error, not a silent no-op).
fn reseed(
    layout: &mut AreaLayout,
    prefabs: &PrefabRegistry,
    campaign: &Campaign,
    area_id: &str,
    bid: &str,
    idx: usize,
    stream: &mut Splitmix64,
) -> Result<([i32; 3], [i32; 3]), String> {
    let pool_id = campaign
        .world
        .content
        .areas
        .iter()
        .find(|a| a.id.as_str() == area_id)
        .and_then(|a| a.prefab_pool.as_ref())
        .ok_or_else(|| {
            format!(
                "world-edits batch `{bid}`: `reseed-piece` targets area `{area_id}` which \
                 binds no `prefab_pool` — there is no pool to re-pick from"
            )
        })?;
    let members = prefabs.pool(pool_id.as_str()).ok_or_else(|| {
        format!(
            "world-edits batch `{bid}`: pool `{pool_id}` vanished from the prefab library \
             mid-plan — compiler bug, escalate"
        )
    })?;
    let current = layout.pieces[idx].prefab_id.clone();
    let required = mated_poses(prefabs, &layout.pieces[idx])
        .map_err(|m| format!("world-edits batch `{bid}`: {m}"))?;
    // Compatible members (≠ current) in pool order, with their weights.
    let mut compatible: Vec<(&str, u32)> = Vec::new();
    for m in members {
        if m.prefab == current {
            continue;
        }
        if mate_replacement(layout, prefabs, &m.prefab, &required, idx).is_some() {
            compatible.push((&m.prefab, m.weight));
        }
    }
    if compatible.is_empty() {
        return Err(format!(
            "world-edits batch `{bid}`: `reseed-piece` finds no OTHER pool member that can \
             re-mate piece {idx}'s sockets — the pool has no compatible variant. Add one to \
             the pool, or use `swap-piece` with an explicit library prefab"
        ));
    }
    let weights: Vec<u32> = compatible.iter().map(|(_, w)| (*w).max(1)).collect();
    let pick = stream.weighted(&weights).expect("non-empty weights");
    let with = compatible[pick].0.to_string();
    swap(layout, prefabs, bid, idx, &with, Some("reseed-piece"))
}

/// `insert-piece`: attach `insert` at a specific **unmated** socket of an
/// existing piece (the targeted form of the solver's frontier attach).
fn insert_at(
    layout: &mut AreaLayout,
    prefabs: &PrefabRegistry,
    bid: &str,
    idx: usize,
    ci: usize,
    insert: &str,
) -> Result<([i32; 3], [i32; 3]), String> {
    let host = &layout.pieces[idx];
    let host_meta = prefabs
        .get(&host.prefab_id)
        .ok_or_else(|| format!("prefab `{}` has no metadata (compiler bug)", host.prefab_id))?;
    let conn = host_meta.connectors.get(ci).ok_or_else(|| {
        format!(
            "world-edits batch `{bid}`: `insert-piece` names connector {ci} but `{}` \
             declares {} connector(s) (0..={})",
            host.prefab_id,
            host_meta.connectors.len(),
            host_meta.connectors.len().saturating_sub(1),
        )
    })?;
    if host.mated.get(ci).copied().unwrap_or(false) {
        return Err(format!(
            "world-edits batch `{bid}`: `insert-piece` targets connector {ci} of piece {idx} \
             which is already mated — pick an open (sealed) socket; `delvec snapshot` shows \
             the layout"
        ));
    }
    let meta = prefabs.get(insert).ok_or_else(|| {
        format!(
            "world-edits batch `{bid}`: `insert-piece` prefab `{insert}` is not in the \
             prefab library — only admitted library prefabs can be placed (ADR-0013)"
        )
    })?;
    let (ws, wf) = socket_world(host.pos, host.rotation, conn).map_err(|e| e.failure.message)?;
    let want = wf.opposite();
    for rot in Rotation::ALL {
        for (cj, c2) in meta.connectors.iter().enumerate() {
            let Some(cf) = Facing::parse(&c2.facing) else {
                continue;
            };
            if cf.rotate(rot) != want {
                continue;
            }
            let ds = wf.unit();
            let tl = rot.transform(c2.local_pos);
            let pos = [
                ws[0] + ds[0] - tl[0],
                ws[1] + ds[1] - tl[1],
                ws[2] + ds[2] - tl[2],
            ];
            let (bmin, bmax) = rot.bbox(pos, meta.size());
            if layout
                .pieces
                .iter()
                .any(|p| aabb_overlap((&bmin, &bmax), (&p.bbox_min, &p.bbox_max)))
            {
                continue;
            }
            let mut mated = vec![false; meta.connectors.len()];
            mated[cj] = true;
            layout.pieces.push(PlacedPiece {
                prefab_id: insert.to_string(),
                pos,
                rotation: rot,
                bbox_min: bmin,
                bbox_max: bmax,
                mated,
            });
            layout.pieces[idx].mated[ci] = true;
            return Ok((bmin, bmax));
        }
    }
    Err(format!(
        "world-edits batch `{bid}`: `insert-piece` cannot attach `{insert}` at connector \
         {ci} of piece {idx} — no rotation mates the socket without overlapping another \
         piece. Pick a smaller/compatible piece, or free the space first (remove-piece)"
    ))
}

/// `remove-piece`: detach a **leaf** (exactly one mated socket, never the
/// entry piece at index 0), then unmate every neighbour connector whose
/// opening steps into the removed footprint — the re-seal walls those
/// doorways back up.
fn remove(
    layout: &mut AreaLayout,
    prefabs: &PrefabRegistry,
    bid: &str,
    idx: usize,
) -> Result<([i32; 3], [i32; 3]), String> {
    if idx == 0 {
        return Err(format!(
            "world-edits batch `{bid}`: `remove-piece` targets the entry piece — the entry \
             carries the campaign's spawn and roots the layout; it cannot be removed \
             (swap it instead)"
        ));
    }
    let mated_count = layout.pieces[idx].mated.iter().filter(|m| **m).count();
    if mated_count != 1 {
        return Err(format!(
            "world-edits batch `{bid}`: `remove-piece` targets piece {idx} with \
             {mated_count} mated socket(s) — only a leaf (exactly 1) can be removed, or its \
             children would be orphaned mid-air. Remove the leaves first, working inward"
        ));
    }
    let removed = layout.pieces.remove(idx);
    let bbox = (removed.bbox_min, removed.bbox_max);
    unmate_into_bbox(layout, prefabs, bbox);
    Ok(bbox)
}

/// Unmate the partner of a socket at world pose `(wp, f)`: the connector (on
/// any other piece) sitting at `wp + unit(f)` facing `opposite(f)` — the
/// solver's own mating geometry, inverted.
fn unmate_partner(layout: &mut AreaLayout, prefabs: &PrefabRegistry, wp: [i32; 3], f: Facing) {
    let u = f.unit();
    let want_pos = [wp[0] + u[0], wp[1] + u[1], wp[2] + u[2]];
    let want_facing = f.opposite();
    for p in &mut layout.pieces {
        let Some(meta) = prefabs.get(&p.prefab_id) else {
            continue;
        };
        for (ci, conn) in meta.connectors.iter().enumerate() {
            if !p.mated.get(ci).copied().unwrap_or(false) {
                continue;
            }
            let Ok((cwp, cf)) = socket_world(p.pos, p.rotation, conn) else {
                continue;
            };
            if cwp == want_pos && cf == want_facing {
                p.mated[ci] = false;
                return;
            }
        }
    }
}

/// Unmate every connector (of every remaining piece) whose opening steps into
/// `bbox` — the removed piece's footprint. Pose math is the solver's own
/// (`socket_world`), so the neighbour search never guesses positions.
fn unmate_into_bbox(layout: &mut AreaLayout, prefabs: &PrefabRegistry, bbox: ([i32; 3], [i32; 3])) {
    let (bmin, bmax) = bbox;
    let inside = |c: [i32; 3]| {
        (bmin[0]..=bmax[0]).contains(&c[0])
            && (bmin[1]..=bmax[1]).contains(&c[1])
            && (bmin[2]..=bmax[2]).contains(&c[2])
    };
    for p in &mut layout.pieces {
        let Some(meta) = prefabs.get(&p.prefab_id) else {
            continue;
        };
        for (ci, conn) in meta.connectors.iter().enumerate() {
            if !p.mated.get(ci).copied().unwrap_or(false) {
                continue;
            }
            let Ok((wp, f)) = socket_world(p.pos, p.rotation, conn) else {
                continue;
            };
            let u = f.unit();
            if inside([wp[0] + u[0], wp[1] + u[1], wp[2] + u[2]]) {
                p.mated[ci] = false;
            }
        }
    }
}

/// The union of two AABBs.
fn union_bbox(a: ([i32; 3], [i32; 3]), b: ([i32; 3], [i32; 3])) -> ([i32; 3], [i32; 3]) {
    (
        [a.0[0].min(b.0[0]), a.0[1].min(b.0[1]), a.0[2].min(b.0[2])],
        [a.1[0].max(b.1[0]), a.1[1].max(b.1[1]), a.1[2].max(b.1[2])],
    )
}

/// Grow a batch's recorded AABB.
fn grow_bounds(
    bounds: &mut BTreeMap<String, ([i32; 3], [i32; 3])>,
    bid: &str,
    b: ([i32; 3], [i32; 3]),
) {
    bounds
        .entry(bid.to_string())
        .and_modify(|e| *e = union_bbox(*e, b))
        .or_insert(b);
}
