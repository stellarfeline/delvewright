"""Spike: a STRANDED organic body, stated as implicit primitives, fitted by the
organic-voxel fitter, and handed to the engine's piece instruments.

Question (spec-0087 §2): when a hill-sized organic body lies ON ground that is
part of its own piece, do the walk and light instruments — which seed from
grade on the box's vertical faces — bind the body's inside and its back, or does
the floating-piece gap the organic-structures research measured still hold?

The body is authored for the rig, not for any campaign: a torso resting on a
mud apron, a head, a tail and fluke, one interior cavity entered through a
ground-level wound on the west flank, one ramp cut into the east flank rising
half a block per block onto the back, and one blowhole through the back into
the cavity. Every number is authored; nothing here is anatomy.

The fitter is the research spike's (`voxelize.py`, `sdf_whale.py`'s `Grid`),
fetched by `run.sh` from the revision it names; `--fitter-dir` points at it.

usage: form.py --fitter-dir <dir> [fit args, see voxelize.add_fit_args]
"""

import argparse
import hashlib
import os
import sys

import numpy as np

# Box, in blocks: x (width) × y (height) × z (length).
W, H, L = 49, 62, 146
# The ground the body lies on: full blocks at y ∈ [0, APRON_TOP], feet at APRON_TOP + 1.
APRON_TOP = 3
APRON_BLOCK = "minecraft:packed_mud"
CX = 24.5

# The ramp: feet surface rises RAMP_SLOPE per block of z, from RAMP_Z0 to RAMP_Z1.
RAMP_Z0, RAMP_Z1, RAMP_Y0, RAMP_SLOPE = 24.0, 112.0, 4.0, 0.5
RAMP_HALF_WIDTH = 1.5
RAMP_CLEARANCE = 3.2

# The torso as a y-stretched capsule: centre line and radius at each end.
TORSO_A, TORSO_B = (CX, 30.0, 30.0), (CX, 26.0, 110.0)
TORSO_RA, TORSO_RB, TORSO_YS = 20.0, 16.0, 1.4


def box(g, lo, hi, op, noisy=False):
    lo, hi = np.array(lo, float), np.array(hi, float)
    c, h = (lo + hi) / 2, (hi - lo) / 2

    def f(X, Y, Z):
        return np.maximum(np.maximum(np.abs(X - c[0]) - h[0], np.abs(Y - c[1]) - h[1]), np.abs(Z - c[2]) - h[2])

    g.stamp(f, lo - 1, hi + 1, op, noisy)


def tcapsule(g, a, b, ra, rb, ys, op="add", noisy=True):
    """A capsule whose cross-section is stretched `ys` times in y."""
    a, b = np.array(a, float), np.array(b, float)
    ab = b - a
    L2 = float(ab @ ab) + 1e-9

    def f(X, Y, Z):
        px, py, pz = X - a[0], Y - a[1], Z - a[2]
        t = np.clip((px * ab[0] + py * ab[1] + pz * ab[2]) / L2, 0, 1)
        dx, dy, dz = px - t * ab[0], (py - t * ab[1]) / ys, pz - t * ab[2]
        return np.sqrt(dx * dx + dy * dy + dz * dz) - (ra + (rb - ra) * t)

    r = max(ra, rb) * ys
    g.stamp(f, np.minimum(a, b) - r, np.maximum(a, b) + r, op, noisy)


def torso_half_width(z, y):
    """Half-width of the torso's ellipse at length z and height y (0 outside)."""
    t = np.clip((z - TORSO_A[2]) / (TORSO_B[2] - TORSO_A[2]), 0, 1)
    cy = TORSO_A[1] + (TORSO_B[1] - TORSO_A[1]) * t
    r = TORSO_RA + (TORSO_RB - TORSO_RA) * t
    s = 1 - ((y - cy) / (r * TORSO_YS)) ** 2
    return r * np.sqrt(np.clip(s, 0, None))


def ramp_feet(z):
    return RAMP_Y0 + RAMP_SLOPE * (z - RAMP_Z0)


def ramp(g):
    """A shelf cut into the east flank: solid under a surface rising half a block
    per block, three blocks of headroom cleared over it."""

    def path_x(Z, Yfeet):
        return CX + torso_half_width(Z, Yfeet) - 2.0

    def shelf(X, Y, Z):
        yf = ramp_feet(Z)
        d_path = np.abs(X - path_x(Z, yf)) - RAMP_HALF_WIDTH
        d_z = np.maximum(RAMP_Z0 - Z, Z - RAMP_Z1)
        return np.maximum(np.maximum(d_path, d_z), np.maximum(Y - yf, (yf - 3.0) - Y))

    def clearance(X, Y, Z):
        yf = ramp_feet(Z)
        d_path = np.abs(X - path_x(Z, yf)) - RAMP_HALF_WIDTH - 0.5
        d_z = np.maximum(RAMP_Z0 - Z, Z - RAMP_Z1 - 3.0)
        return np.maximum(np.maximum(d_path, d_z), np.maximum(yf - Y, Y - (yf + RAMP_CLEARANCE)))

    lo = (0.0, 0.0, RAMP_Z0 - 2)
    hi = (float(W), float(H), RAMP_Z1 + 5)
    g.stamp(shelf, lo, hi, "add", noisy=False)
    g.stamp(clearance, lo, hi, "sub", noisy=False)


def build(g, sdf):
    # body
    tcapsule(g, TORSO_A, TORSO_B, TORSO_RA, TORSO_RB, TORSO_YS)
    sdf.ellipsoid(g, (CX, 18.0, 20.0), (16.0, 15.0, 17.0))  # head
    tcapsule(g, (CX, 20.0, 110.0), (CX, 8.0, 132.0), 14.0, 5.0, 1.0)  # tail
    sdf.ellipsoid(g, (CX, 6.0, 136.0), (20.0, 3.0, 8.0))  # fluke, lying on the ground
    # inside: one cavity cut down into the apron, which is re-laid under it, so
    # the floor inside is the flat ground at the apron's feet level
    sdf.ellipsoid(g, (CX, 17.0, 72.0), (14.0, 17.0, 36.0), op="sub", noisy=False)
    # the way in: a wound on the west flank at ground level, through to the cavity
    box(g, (-1.0, APRON_TOP + 1.0, 61.5), (20.0, APRON_TOP + 5.0, 66.5), "sub")
    # onto the back
    ramp(g)
    # the blowhole: a shaft from the back into the cavity
    box(g, (CX - 1.6, 26.0, 78.4), (CX + 1.6, float(H) + 1, 81.6), "sub")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--fitter-dir", required=True, help="directory holding the research spike's voxelize.py and sdf_whale.py")
    ap.add_argument("--noise", type=float, default=0.45, help="surface weathering amplitude, blocks")
    pre, _ = ap.parse_known_args()
    sys.path.insert(0, os.path.abspath(pre.fitter_dir))
    import sdf_whale as sdf  # noqa: E402
    import voxelize as vx  # noqa: E402

    vx.add_fit_args(ap)
    a = ap.parse_args()
    g = sdf.Grid((W, H, L), a.sub, 1.0, a.noise, a.seed)
    build(g, sdf)
    print(f"blocks {g.shape_b.tolist()} solid sub-voxels {int(g.solid.sum())} primitives {g.count}", flush=True)

    # The apron is laid under the fitted body, never fitted: every air cell at
    # y <= APRON_TOP becomes ground. Wrapped around the fitter's writer so the
    # fitter itself is the research revision's, unchanged.
    real_write = vx.write_schem

    def write_with_apron(path, size, states, ids):
        w, h, l = size
        ids = np.asarray(ids, dtype=np.int64).reshape(h, l, w)  # index = x + z*W + y*W*L
        states = list(states)
        air = states.index("minecraft:air") if "minecraft:air" in states else -1
        if APRON_BLOCK not in states:
            states.append(APRON_BLOCK)
        mud = states.index(APRON_BLOCK)
        band = ids[: APRON_TOP + 1]
        laid = int((band == air).sum())
        band[band == air] = mud
        print(f"apron laid {laid} cells at y<={APRON_TOP}", flush=True)
        real_write(path, size, states, ids.ravel().tolist())

    vx.write_schem = write_with_apron
    rep = {
        "source": "sdf",
        "program": "tools/spike-organic-stranded/form.py",
        "program_sha256": hashlib.sha256(open(os.path.abspath(__file__), "rb").read()).hexdigest(),
        "fitter": os.path.abspath(pre.fitter_dir),
        "fitter_sha256": {f: vx.sha256_file(os.path.join(pre.fitter_dir, f)) for f in ("voxelize.py", "sdf_whale.py", "schem.py")},
        "length_blocks": L,
        "noise_blocks": a.noise,
        "apron_top": APRON_TOP,
        "apron_block": APRON_BLOCK,
        "primitives": g.count,
    }
    vx.blocks_from_solid(g.solid, a, rep)


if __name__ == "__main__":
    main()
