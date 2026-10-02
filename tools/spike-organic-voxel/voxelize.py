"""Spike: a CC0 whale-skeleton scan -> block-shape-aware Minecraft voxels -> .schem.

Question: can a scanned organic form, voxelised with sub-block (octant) shape
fitting and a measured bone palette, reach the reference image's quality at
2-3 scales? See README.md. Spike code: not an engine surface.

Pipeline (each step deterministic: fixed inputs, no wall clock, no hash order,
the one RNG is numpy PCG64 seeded from --seed):
  1. Read the binary STL by memmap; map model axes to Minecraft
     (model X lateral, Y length with the skull at -Y, Z up) -> (x=-X, y=Z, z=Y):
     a proper rotation, skull to the north.
  2. Scale so the body is --length blocks along z; mark every sub-voxel
     (--sub per block per axis) a surface sample falls in (vertices, edge
     midpoints, centroids, plus barycentric grids on triangles larger than
     half a sub-voxel).
  3. Solidify: the complement's components touching the grid border are
     outside; everything else is bone (fills each bone's interior).
  4. Octants: each block is 2x2x2 octants; an octant's fraction is the share of
     its sub-voxels that are bone.
  5. Shape fit: per block choose air / full / bottom or top slab / straight
     stair (4 facings x 2 halves) minimising sum |fraction - octant bit|.
  6. Stair `shape` is never chosen: it is derived by vanilla's rule (a port of
     crates/delvec/src/schem/stairs.rs::derive_shape), so the DW0801 gate holds.
  7. Shade: tone from smoothed surface normal (up = bleached) and local
     openness (crevices = dark), plus low-frequency seeded noise (weathering
     patches, not salt-and-pepper); the tone picks a palette family.
  8. bone_block axis = the axis of the longest local run of bone.
"""

import argparse
import hashlib
import json
import math
import os
import sys

import numpy as np
from scipy import ndimage

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from schem import write_schem  # noqa: E402

STL_DTYPE = np.dtype([("n", "<f4", 3), ("v", "<f4", (3, 3)), ("a", "<u2")])

# Facings: index 0..3 = north, east, south, west; vectors in (x, z).
FACINGS = ["north", "east", "south", "west"]
FVEC = [(0, -1), (1, 0), (0, 1), (-1, 0)]

# Palette tones, bleached (0) to weathered (3). Every family offers block,
# slab and stair, so a shape never changes material. `full` may mix extra
# full-cube members (weights). Chosen by block-appearance.py measurement
# (README "Palette"); edit here, re-measure there.
PALETTES = {
    # weathered: bleached bone-block tops, porous warm-grey dead-coral flanks,
    # grey and mossy undersides; shapes from the nearest-lightness stair family
    "weathered": [
        {"full": [("bone_block", 3), ("dead_horn_coral_block", 3), ("calcite", 2), ("diorite", 2)], "family": "diorite"},
        {"full": [("dead_horn_coral_block", 3), ("dead_bubble_coral_block", 3), ("andesite", 2), ("diorite", 2)], "family": "andesite"},
        {"full": [("andesite", 4), ("dead_brain_coral_block", 3), ("tuff", 3)], "family": "andesite"},
        {"full": [("tuff", 5), ("mossy_cobblestone", 3), ("andesite", 2)], "family": "mossy_cobblestone"},
    ],
    # warm: bone-block tops with smooth-sandstone sub-block shapes
    "warm": [
        {"full": [("bone_block", 6), ("calcite", 2), ("smooth_sandstone", 2)], "family": "smooth_sandstone"},
        {"full": [("bone_block", 4), ("diorite", 3), ("smooth_sandstone", 3)], "family": "diorite"},
        {"full": [("andesite", 6), ("diorite", 2), ("tuff", 2)], "family": "andesite"},
        {"full": [("andesite", 5), ("tuff", 3), ("mossy_cobblestone", 2)], "family": "mossy_cobblestone"},
    ],
    # neutral: bone-block tops with diorite sub-block shapes, grey below
    "neutral": [
        {"full": [("bone_block", 6), ("calcite", 2), ("diorite", 2)], "family": "diorite"},
        {"full": [("diorite", 6), ("bone_block", 2), ("andesite", 2)], "family": "diorite"},
        {"full": [("andesite", 6), ("diorite", 2), ("tuff", 2)], "family": "andesite"},
        {"full": [("andesite", 5), ("tuff", 3), ("mossy_cobblestone", 2)], "family": "mossy_cobblestone"},
    ],
}
SLAB = {f: f"{f}_slab" for f in ("smooth_sandstone", "diorite", "andesite", "mossy_cobblestone")}
STAIR = {f: f"{f}_stairs" for f in ("smooth_sandstone", "diorite", "andesite", "mossy_cobblestone")}
PALETTE = PALETTES["weathered"]

# kinds
AIR, FULL, SLAB_B, SLAB_T, STAIR_K = 0, 1, 2, 3, 4


def octant_masks():
    """Candidate shapes as 8-vectors over octants o = ox + 2*oy + 4*oz
    (ox: 0 west/1 east, oy: 0 bottom/1 top, oz: 0 north/1 south)."""
    def m(pred):
        return np.array([1.0 if pred(o & 1, (o >> 1) & 1, (o >> 2) & 1) else 0.0 for o in range(8)])

    side = [lambda x, z: z == 0, lambda x, z: x == 1, lambda x, z: z == 1, lambda x, z: x == 0]
    cands = [(FULL, -1, -1, m(lambda x, y, z: True)),
             (AIR, -1, -1, m(lambda x, y, z: False)),
             (SLAB_B, -1, -1, m(lambda x, y, z: y == 0)),
             (SLAB_T, -1, -1, m(lambda x, y, z: y == 1))]
    for f in range(4):
        s = side[f]
        cands.append((STAIR_K, f, 0, m(lambda x, y, z, s=s: y == 0 or s(x, z))))
        cands.append((STAIR_K, f, 1, m(lambda x, y, z, s=s: y == 1 or s(x, z))))
    return cands


def load_mesh(path):
    m = np.memmap(path, dtype=STL_DTYPE, mode="r", offset=84)
    return m


def to_mc(v):
    # (X, Y, Z) model -> (x, y, z) Minecraft = (-X, Z, Y)
    return np.stack([-v[..., 0], v[..., 2], v[..., 1]], axis=-1)


def mesh_bounds(m, chunk):
    lo = np.full(3, np.inf)
    hi = np.full(3, -np.inf)
    for i in range(0, len(m), chunk):
        p = to_mc(np.asarray(m["v"][i:i + chunk], dtype=np.float64)).reshape(-1, 3)
        lo = np.minimum(lo, p.min(0))
        hi = np.maximum(hi, p.max(0))
    return lo, hi


def mark_surface(m, occ, origin, k, pad, chunk):
    """Set occ[i,j,l] for every sub-voxel a surface sample falls in."""
    shape = np.array(occ.shape)

    def put(p):
        idx = np.floor((p - origin) * k).astype(np.int64) + pad
        idx = idx[np.all((idx >= 0) & (idx < shape), axis=1)]
        occ[idx[:, 0], idx[:, 1], idx[:, 2]] = True

    for i in range(0, len(m), chunk):
        t = to_mc(np.asarray(m["v"][i:i + chunk], dtype=np.float64))  # (n,3,3)
        a, b, c = t[:, 0], t[:, 1], t[:, 2]
        put(np.concatenate([a, b, c, (a + b) / 2, (b + c) / 2, (c + a) / 2, (a + b + c) / 3]))
        e = np.maximum.reduce([np.linalg.norm(a - b, axis=1), np.linalg.norm(b - c, axis=1),
                               np.linalg.norm(c - a, axis=1)]) * k  # in sub-voxels
        need = np.ceil(e / 0.5).astype(int)
        for n in np.unique(need[need > 2]):
            sel = need == n
            us, vs = np.meshgrid(np.arange(n + 1), np.arange(n + 1), indexing="ij")
            keep = us + vs <= n
            u = (us[keep] / n)[None, :, None]
            v = (vs[keep] / n)[None, :, None]
            p = a[sel][:, None] + u * (b[sel] - a[sel])[:, None] + v * (c[sel] - a[sel])[:, None]
            put(p.reshape(-1, 3))


def solidify(occ):
    outside, n = ndimage.label(~occ)
    border = np.unique(np.concatenate([
        outside[0].ravel(), outside[-1].ravel(), outside[:, 0].ravel(), outside[:, -1].ravel(),
        outside[:, :, 0].ravel(), outside[:, :, -1].ravel()]))
    border = border[border != 0]
    lut = np.zeros(n + 1, dtype=bool)
    lut[border] = True
    ext = lut[outside]
    del outside
    return ~ext


def octant_fractions(solid, sub):
    h = sub // 2
    bx, by, bz = (s // sub for s in solid.shape)
    s = solid[:bx * sub, :by * sub, :bz * sub].reshape(bx, 2, h, by, 2, h, bz, 2, h)
    cnt = s.sum(axis=(2, 5, 8), dtype=np.int32)  # (bx,2,by,2,bz,2)
    frac = cnt.transpose(0, 2, 4, 1, 3, 5).astype(np.float32) / float(h ** 3)  # (bx,by,bz,ox,oy,oz)
    # flatten octants o = ox + 2*oy + 4*oz
    return frac.transpose(0, 1, 2, 5, 4, 3).reshape(bx, by, bz, 8)


def fit_shapes(frac, bias):
    """bias: extra cost (octant units) per kind; a sub-block shape must beat a
    full block or air by that margin, so curved surfaces do not pit."""
    cands = octant_masks()
    M = np.stack([c[3] for c in cands])  # (C,8)
    best_cost = None
    best = None
    for ci, mask in enumerate(M):
        cost = np.abs(frac - mask).sum(-1) + bias.get(cands[ci][0], 0.0)
        if best is None:
            best_cost, best = cost, np.zeros(cost.shape, np.int16)
        else:
            better = cost < best_cost - 1e-6
            best[better] = ci
            best_cost = np.where(better, cost, best_cost)
    kind = np.zeros(best.shape, np.int8)
    facing = np.full(best.shape, -1, np.int8)
    half = np.full(best.shape, -1, np.int8)
    for ci, (k, f, h, _) in enumerate(cands):
        sel = best == ci
        kind[sel], facing[sel], half[sel] = k, f, h
    return kind, facing, half


def derive_shapes(kind, facing, half):
    """Port of delvec::schem::stairs::derive_shape (vanilla StairBlock rule)."""
    X, Y, Z = kind.shape
    shape = {}
    ccw = lambda f: (f + 3) % 4  # noqa: E731
    opp = lambda f: (f + 2) % 4  # noqa: E731

    def nb(x, y, z, f):
        dx, dz = FVEC[f]
        nx, nz = x + dx, z + dz
        if 0 <= nx < X and 0 <= nz < Z and kind[nx, y, nz] == STAIR_K:
            return int(facing[nx, y, nz]), int(half[nx, y, nz])
        return None

    for x, y, z in zip(*np.nonzero(kind == STAIR_K)):
        f, h = int(facing[x, y, z]), int(half[x, y, z])

        def can_take(d):
            n = nb(x, y, z, d)
            return n is None or n[0] != f or n[1] != h

        s = "straight"
        front = nb(x, y, z, f)
        if front and front[1] == h and front[0] % 2 != f % 2 and can_take(opp(front[0])):
            s = "outer_left" if front[0] == ccw(f) else "outer_right"
        else:
            back = nb(x, y, z, opp(f))
            if back and back[1] == h and back[0] % 2 != f % 2 and can_take(back[0]):
                s = "inner_left" if back[0] == ccw(f) else "inner_right"
        shape[(int(x), int(y), int(z))] = s
    return shape


def run_axis(solid_blocks):
    """Per block, the axis (0 x, 1 y, 2 z) with the longest bone run within +-3."""
    runs = []
    s = solid_blocks.astype(np.float32)
    for ax in range(3):
        w = np.zeros(7, np.float32) + 1
        runs.append(ndimage.convolve1d(s, w, axis=ax, mode="constant"))
    return np.argmax(np.stack(runs), axis=0)


def shade(kind, frac, seed):
    fill = frac.mean(-1).astype(np.float32)
    g = ndimage.gaussian_filter(fill, 1.2)
    gx, gy, gz = np.gradient(g)
    norm = np.sqrt(gx * gx + gy * gy + gz * gz) + 1e-6
    up = -gy / norm  # +1 = surface faces the sky
    crevice = 1.0 - ndimage.gaussian_filter(fill, 1.5)  # ~0.5 on an open face, low in a crack
    body = 1.0 - ndimage.gaussian_filter(fill, 5.0)  # low deep inside a dense mass
    rng = np.random.Generator(np.random.PCG64(seed))

    def field(sigma):
        f = ndimage.gaussian_filter(rng.standard_normal(fill.shape).astype(np.float32), sigma)
        return f / (f.std() + 1e-6)

    noise = field(2.5)
    t = 0.6 * up + 1.2 * (crevice - 0.55) + 0.6 * (body - 0.85) + 0.30 * noise
    tone = np.digitize(-t, [-0.35, 0.15, 0.55])  # high t -> tone 0 (bleached)
    # member choice inside a tone from a low-frequency field: patches, not speckle
    from scipy.special import ndtr
    pick = ndtr(field(1.5))
    return tone.astype(np.int8), pick


def state_strings(kind, facing, half, shapes, tone, pick, axis):
    X, Y, Z = kind.shape
    out = np.empty(kind.shape, dtype=object)
    out[...] = "minecraft:air"
    for x, y, z in zip(*np.nonzero(kind != AIR)):
        k = kind[x, y, z]
        p = PALETTE[tone[x, y, z]]
        fam = p["family"]
        if k == FULL:
            tot = sum(w for _, w in p["full"])
            r = pick[x, y, z] * tot
            for name, w in p["full"]:
                if r < w:
                    break
                r -= w
            st = f"minecraft:{name}"
            if name == "bone_block":
                st += f"[axis={'xyz'[axis[x, y, z]]}]"
        elif k in (SLAB_B, SLAB_T):
            st = f"minecraft:{SLAB[fam]}[type={'bottom' if k == SLAB_B else 'top'},waterlogged=false]"
        else:
            st = (f"minecraft:{STAIR[fam]}[facing={FACINGS[facing[x, y, z]]},"
                  f"half={'bottom' if half[x, y, z] == 0 else 'top'},"
                  f"shape={shapes[(int(x), int(y), int(z))]},waterlogged=false]")
        out[x, y, z] = st
    return out


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for b in iter(lambda: f.read(1 << 22), b""):
            h.update(b)
    return h.hexdigest()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--stl", required=True)
    ap.add_argument("--length", type=int, required=True, help="body length in blocks (z)")
    ap.add_argument("--sub", type=int, default=4, help="sub-voxels per block per axis (even)")
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--palette", choices=sorted(PALETTES), default="weathered")
    ap.add_argument("--stair-bias", type=float, default=0.6)
    ap.add_argument("--slab-bias", type=float, default=0.4)
    ap.add_argument("--min-component", type=int, default=4,
                    help="drop disconnected block islands smaller than this")
    ap.add_argument("--out", required=True, help="output .schem")
    ap.add_argument("--report", required=True)
    ap.add_argument("--grid-out", help="write the block-kind grid (.npy) for views.py")
    a = ap.parse_args()
    assert a.sub % 2 == 0
    global PALETTE
    PALETTE = PALETTES[a.palette]
    chunk = 1_000_000

    m = load_mesh(a.stl)
    lo, hi = mesh_bounds(m, chunk)
    ext = hi - lo
    s = a.length / ext[2]  # blocks per metre
    k = s * a.sub  # sub-voxels per metre
    pad_b = 2
    pad = pad_b * a.sub
    dims_b = np.ceil(ext * s).astype(int) + 2 * pad_b
    occ = np.zeros(dims_b * a.sub, dtype=bool)
    print(f"blocks {dims_b.tolist()} sub-grid {occ.shape} ({occ.size / 1e6:.0f} M)", flush=True)
    mark_surface(m, occ, lo, k, pad, chunk)
    print(f"surface sub-voxels {int(occ.sum())}", flush=True)
    solid = solidify(occ)
    del occ
    print(f"solid sub-voxels {int(solid.sum())}", flush=True)
    bias = {SLAB_B: a.slab_bias, SLAB_T: a.slab_bias, STAIR_K: a.stair_bias}
    frac = octant_fractions(solid, a.sub)
    kind, facing, half = fit_shapes(frac, bias)
    # Thin plates (a rostrum, a scapula blade) are thinner than half an octant
    # and fit to air, leaving holes. Where the fit is air, refit from the solid
    # thickened by one sub-voxel; thick bone is never re-fitted, so never bloats.
    thick = ndimage.binary_dilation(solid, structure=ndimage.generate_binary_structure(3, 1))
    del solid
    frac_t = octant_fractions(thick, a.sub)
    del thick
    k2, f2, h2 = fit_shapes(frac_t, bias)
    fill_in = (kind == AIR) & (k2 != AIR)
    kind[fill_in], facing[fill_in], half[fill_in] = k2[fill_in], f2[fill_in], h2[fill_in]
    frac = np.where(fill_in[..., None], frac_t, frac)
    thin_filled = int(fill_in.sum())

    # drop tiny disconnected islands (scan noise); a body cannot use them
    lab, n = ndimage.label(kind != AIR, structure=np.ones((3, 3, 3)))
    sizes = np.bincount(lab.ravel())
    small = sizes < a.min_component
    small[0] = False
    dropped = int(small[lab].sum())
    kind[small[lab]] = AIR
    comps = int((sizes[1:] >= a.min_component).sum())

    shapes = derive_shapes(kind, facing, half)
    tone, pick = shade(kind, frac, a.seed)
    axis = run_axis(kind != AIR)
    states = state_strings(kind, facing, half, shapes, tone, pick, axis)

    if a.grid_out:
        np.save(a.grid_out, kind)
    X, Y, Z = kind.shape
    # schem index order: x fastest, then z, then y
    flat = states.transpose(1, 2, 0).ravel()
    palette, ids = [], np.empty(flat.size, np.int64)
    seen = {}
    for i, st in enumerate(flat):
        j = seen.get(st)
        if j is None:
            j = seen[st] = len(palette)
            palette.append(st)
        ids[i] = j
    write_schem(a.out, (X, Y, Z), palette, ids.tolist())

    counts = {}
    for st in flat:
        if st != "minecraft:air":
            base = st.split("[")[0]
            counts[base] = counts.get(base, 0) + 1
    kinds = {n: int((kind == v).sum()) for n, v in
             [("full", FULL), ("slab", SLAB_B), ("slab_top", SLAB_T), ("stair", STAIR_K)]}
    stair_shapes = {}
    for v in shapes.values():
        stair_shapes[v] = stair_shapes.get(v, 0) + 1
    rep = {
        "mesh_sha256": sha256_file(a.stl),
        "mesh_triangles": int(len(m)),
        "mesh_extent_m_mc_xyz": [round(float(v), 4) for v in ext],
        "length_blocks": a.length, "blocks_per_metre": round(float(s), 3), "sub": a.sub,
        "seed": a.seed, "palette": a.palette, "size_xyz": [int(X), int(Y), int(Z)],
        "filled": int((kind != AIR).sum()), "kinds": kinds, "stair_shapes": dict(sorted(stair_shapes.items())),
        "components_kept": comps, "thin_plate_blocks_filled": thin_filled,
        "shape_bias": {"stair": a.stair_bias, "slab": a.slab_bias}, "island_blocks_dropped": dropped,
        "tone_share": {str(t): int(((tone == t) & (kind != AIR)).sum()) for t in range(4)},
        "block_counts": dict(sorted(counts.items(), key=lambda kv: -kv[1])),
        "schem_sha256": sha256_file(a.out),
    }
    with open(a.report, "w") as f:
        json.dump(rep, f, indent=2, sort_keys=True)
        f.write("\n")
    print(json.dumps({k: rep[k] for k in ("size_xyz", "filled", "kinds", "schem_sha256")}))


if __name__ == "__main__":
    main()
