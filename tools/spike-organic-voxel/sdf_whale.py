"""Spike, second front end: a STYLISED whale skeleton stated as implicit
primitives (tapered capsules, ellipsoids, discs), sampled into the same solid
sub-voxel grid the mesh route builds, then fitted by the same back half
(voxelize.blocks_from_solid).

Why it exists: the CC0 scan is a real 3 m dwarf whale. Its anatomy is not the
reference's: broad, nearly touching ribs (no gaps to read as a cage), a narrow
vertebral ridge (nothing to walk on), a closed jaw and a small braincase (no
temple). The reference is anatomy exaggerated for play. This states that
anatomy directly, with play-scale rules as numbers (README "Stylised rules").
Every number below is AUTHORED for the spike, written at a 320-block body and
scaled by --length/320; anatomy it imitates is cited in README.

usage: sdf_whale.py --length 320 [fit args, see voxelize.py]
"""

import argparse
import math
import os
import sys

import numpy as np
from scipy import ndimage

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import voxelize as vx  # noqa: E402

REF_L = 320.0


class Grid:
    def __init__(self, size_b, sub, scale, noise_amp, seed):
        self.sub = sub
        self.k = scale  # authored unit -> blocks
        self.solid = np.zeros(tuple(int(math.ceil(v * scale)) * sub for v in size_b), dtype=bool)
        self.shape_b = np.array(self.solid.shape) // sub
        # weathering: low-frequency noise on the surface, ~2-block cells
        rng = np.random.Generator(np.random.PCG64(seed + 1000))
        nshape = tuple(int(s // 2) + 3 for s in self.shape_b)
        n = ndimage.gaussian_filter(rng.standard_normal(nshape).astype(np.float32), 1.0)
        self.noise = n / (n.std() + 1e-6) * noise_amp
        self.count = {"add": 0, "sub": 0}

    def _pts(self, lo, hi):
        lo = np.maximum(np.floor(np.array(lo) * self.k * self.sub).astype(int) - 2, 0)
        hi = np.minimum(np.ceil(np.array(hi) * self.k * self.sub).astype(int) + 2, self.solid.shape)
        if np.any(hi <= lo):
            return None
        ax = [(np.arange(lo[i], hi[i]) + 0.5) / self.sub / self.k for i in range(3)]  # authored units
        X, Y, Z = np.meshgrid(*ax, indexing="ij")
        return (slice(lo[0], hi[0]), slice(lo[1], hi[1]), slice(lo[2], hi[2])), X, Y, Z

    def _noise_at(self, X, Y, Z):
        c = np.stack([X * self.k / 2, Y * self.k / 2, Z * self.k / 2])
        return ndimage.map_coordinates(self.noise, c, order=1, mode="nearest") / self.k

    def stamp(self, f, lo, hi, op="add", noisy=True):
        r = self._pts(lo, hi)
        if r is None:
            return
        sl, X, Y, Z = r
        d = f(X, Y, Z)
        if noisy:
            d = d + self._noise_at(X, Y, Z)
        if op == "add":
            self.solid[sl] |= d < 0
        else:
            self.solid[sl] &= ~(d < 0)
        self.count[op] += 1


# ---- primitives (authored units) -------------------------------------------

def capsule(g, a, b, ra, rb, op="add"):
    a, b = np.array(a, float), np.array(b, float)
    ab = b - a
    L2 = float(ab @ ab) + 1e-9

    def f(X, Y, Z):
        px, py, pz = X - a[0], Y - a[1], Z - a[2]
        t = np.clip((px * ab[0] + py * ab[1] + pz * ab[2]) / L2, 0, 1)
        dx, dy, dz = px - t * ab[0], py - t * ab[1], pz - t * ab[2]
        return np.sqrt(dx * dx + dy * dy + dz * dz) - (ra + (rb - ra) * t)

    r = max(ra, rb)
    g.stamp(f, np.minimum(a, b) - r, np.maximum(a, b) + r, op)


def ellipsoid(g, c, rad, op="add", noisy=True):
    c, rad = np.array(c, float), np.array(rad, float)

    def f(X, Y, Z):
        q = np.sqrt(((X - c[0]) / rad[0]) ** 2 + ((Y - c[1]) / rad[1]) ** 2 + ((Z - c[2]) / rad[2]) ** 2)
        return (q - 1.0) * rad.min()

    g.stamp(f, c - rad, c + rad, op, noisy)


def disc(g, c, u, R, h, flat=None):
    """A centrum: a cylinder along the spine; `flat` cuts its top to a plateau
    at c_y + flat*R so a body can walk on it."""
    c, u = np.array(c, float), np.array(u, float) / np.linalg.norm(u)

    def f(X, Y, Z):
        qx, qy, qz = X - c[0], Y - c[1], Z - c[2]
        along = qx * u[0] + qy * u[1] + qz * u[2]
        rx, ry, rz = qx - along * u[0], qy - along * u[1], qz - along * u[2]
        radial = np.sqrt(rx * rx + ry * ry + rz * rz)
        dx, dy = radial - R, np.abs(along) - h
        d = np.minimum(np.maximum(dx, dy), 0) + np.sqrt(np.maximum(dx, 0) ** 2 + np.maximum(dy, 0) ** 2) - 0.6
        return d if flat is None else np.maximum(d, Y - (c[1] + flat * R))

    r = R + h
    g.stamp(f, c - r, c + r)


def chain(g, pts, r0, r1, op="add"):
    n = len(pts) - 1
    for i in range(n):
        capsule(g, pts[i], pts[i + 1], r0 + (r1 - r0) * i / n, r0 + (r1 - r0) * (i + 1) / n, op)


# ---- the skeleton (authored at a 320-block body) ----------------------------

W, H, L = 150.0, 120.0, 320.0
CX = W / 2
Z0 = 104.0  # occiput: the skull is a third of the body, as the reference draws it
ZT = L - 6.0


def spine(s):
    z = Z0 + s * (ZT - Z0)
    y = 72 + 16 * math.sin(math.pi * s * 1.1) - 26 * s ** 3
    return np.array([CX, y, z])


def tangent(s):
    e = 1e-3
    d = spine(min(s + e, 1)) - spine(max(s - e, 0))
    return d / np.linalg.norm(d)


def rv(s):  # vertebral disc radius: 11 at the neck -> ~8-block walkable lanes either side of the spine
    return 1.6 + 9.4 * (1 - s) ** 0.9


def build(g):
    up = np.array([0, 1, 0.0])
    # vertebral column: discs on a continuous core
    n = 38
    for i in range(n):
        s = (i + 0.5) / n
        p, t = spine(s), tangent(s)
        spacing = np.linalg.norm(spine(min(s + 0.5 / n, 1)) - spine(max(s - 0.5 / n, 0)))
        disc(g, p, t, rv(s), 0.36 * spacing, flat=0.55)  # flat-topped: the walk
        if s < 0.85:  # neural spine, raked tailward
            base = p + up * rv(s) * 0.5
            h = 4 + 13 * (1 - s / 0.85)
            tip = base + h * np.array([0, math.cos(math.radians(18)), math.sin(math.radians(18))])
            capsule(g, base, tip, 1.5 - 0.5 * s, 0.9)
        if 0.30 < s < 0.66:  # lumbar transverse processes
            for side in (-1, 1):
                capsule(g, p, p + np.array([side * rv(s) * 1.6, -1, 1]), 1.3, 0.8)
        if s > 0.55:  # chevrons
            capsule(g, p - up * rv(s) * 0.7, p - up * rv(s) * 1.7 + np.array([0, 0, 1.5]), 1.0, 0.7)
    chain(g, [spine(i / 60) for i in range(61)], rv(0) * 0.45, 0.9)
    # ribs: 12 pairs, ~2.6 blocks thick at a ~5.6-block pitch -> ~3-block gaps
    for j in range(12):
        s = 0.02 + j * (0.32 / 11)
        p = spine(s)
        bell = math.sin(math.pi * (s + 0.06) / 0.44) ** 0.6
        a, b = 27 * bell + 4, 31 * bell + 4
        o = p - np.array([0, b * 0.95, 0])
        for side in (-1, 1):
            pts = []
            for k in range(14):
                th = math.radians(25 + k * (140 / 13))
                pts.append(o + np.array([side * a * math.sin(th), b * math.cos(th), 7 * (1 - math.cos(th)) / 2]))
            pts[0] = p + np.array([side * rv(s) * 0.7, 0, 0])
            chain(g, pts, 1.45, 1.0)
    # skull: a flat wedge (rorqual skulls are broad triangles from above, low in
    # profile) with a raised occipital shield holding a hollow hall
    ellipsoid(g, (CX, 82, 90), (32, 17, 18))  # occipital shield / braincase
    ellipsoid(g, (CX, 82, 72), (28, 13, 26))  # skull roof sloping into the rostrum
    for i in range(18):
        u = i / 17
        ellipsoid(g, (CX, 76 - 20 * u - 6 * u * u, 86 - 80 * u), (27 - 21 * u, 11 - 7.5 * u, 10))
    # the hall: flat floor at y=78, 18 high, entered through the foramen magnum
    g.stamp(lambda X, Y, Z: np.maximum(((X - CX) / 19) ** 2 + ((Y - 86) / 10) ** 2 + ((Z - 86) / 15) ** 2 - 1, 78 - Y) * 10,
            (CX - 20, 78, 70), (CX + 20, 97, 102), op="sub", noisy=False)
    for side in (-1, 1):
        ellipsoid(g, (CX + side * 27, 87, 86), (9, 8, 12), op="sub", noisy=False)  # temporal windows into the hall
        ellipsoid(g, (CX + side * 24, 72, 56), (5, 4, 5), op="sub", noisy=False)  # orbits
    capsule(g, (CX, 84, 96), (CX, 84, 112), 6, 6, op="sub")  # foramen magnum: spine walk -> hall
    # mandibles: bowed, the jaw dropped open
    for side in (-1, 1):
        pts = []
        for k in range(15):
            t = k / 14
            pts.append(np.array([CX + side * (24 - 14 * t + 9 * math.sin(math.pi * t)),
                                 60 - 40 * t + 6 * math.sin(math.pi * t), 98 - 92 * t]))
        chain(g, pts, 4.6, 2.6)
    # forelimbs: scapula blade, humerus, forearm, four digits
    for side in (-1, 1):
        ellipsoid(g, (CX + side * 16, 70, 116), (2.6, 12, 9))
        sh = np.array([CX + side * 17, 58, 118.0])
        el = np.array([CX + side * 27, 44, 126.0])
        wr = np.array([CX + side * 33, 28, 140.0])
        capsule(g, sh, el, 4.2, 3.2)
        capsule(g, el, wr, 3.0, 2.6)
        for d in range(4):
            tip = wr + np.array([side * (-4 + 3.5 * d), -16 + 2 * d, 10 + 3 * d])
            mid = (wr + tip) / 2 + np.array([0, -1, 0])
            chain(g, [wr, mid, tip], 1.5, 0.9)
    # vestigial pelvis, floating below the lumbar column
    for side in (-1, 1):
        p = spine(0.6)
        capsule(g, p + np.array([side * 5, -16, -4]), p + np.array([side * 7, -18, 6]), 1.4, 1.0)
    # flukes: an authored departure (a fluke has no bone), drawn because the reference draws one
    e = spine(1.0)
    for side in (-1, 1):
        ellipsoid(g, (CX + side * 13, e[1] + 1, e[2] - 8), (14, 1.8, 6))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--length", type=int, required=True, help="body length in blocks (z)")
    ap.add_argument("--noise", type=float, default=0.45, help="surface weathering amplitude, blocks")
    vx.add_fit_args(ap)
    a = ap.parse_args()
    scale = a.length / REF_L
    g = Grid((W, H, L), a.sub, scale, a.noise, a.seed)
    print(f"blocks {g.shape_b.tolist()} sub-grid {g.solid.shape} ({g.solid.size / 1e6:.0f} M)", flush=True)
    build(g)
    print(f"solid sub-voxels {int(g.solid.sum())}; primitives {g.count}", flush=True)
    rep = {"source": "sdf", "program": "tools/spike-organic-voxel/sdf_whale.py",
           "program_sha256": vx.sha256_file(os.path.abspath(__file__)),
           "length_blocks": a.length, "noise_blocks": a.noise, "primitives": g.count}
    vx.blocks_from_solid(g.solid, a, rep)


if __name__ == "__main__":
    main()
