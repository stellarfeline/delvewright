"""Place the study's four cameras on a voxelised whale and draw them with
`delvec snapshot` (the draft rasteriser; the piece sits at world (0, 64, 0)).

usage: views.py <delvec> <prefabs-dir> <campaign-dir> <grid.npy> <out-dir> <tag>
       views.py --chunky <world-dir> <grid.npy> <scene-dir> <tag> <spp>
         writes one Chunky scene per view (the field set and camera basis of
         crates/delvec/src/compiler/view/camera.rs::world_scene and
         scene.rs::chunky_orientation over camera.rs::plan_yaw; sun at Chunky's default) for
         validation/chunky.sh to render

Cameras are computed from the block grid, so they scale with the piece:
  far      the reference's establishing view: west of and ahead of the skull, above
  spine    a body's eye standing on the neural spines mid-back, looking to the tail
  ribcage  a body's eye inside the widest rib section, looking up and toward the skull
  skull    an oblique close view of the skull from the front-left, above
"""

import json
import math
import os
import subprocess
import sys

import numpy as np

CHUNKY = sys.argv[1] == "--chunky"
if CHUNKY:
    world, grid_p, out, tag, spp = sys.argv[2:7]
else:
    delvec, prefabs, campaign, grid_p, out, tag = sys.argv[1:7]
k = np.load(grid_p) != 0
X, Y, Z = k.shape
OY = 64
cx = X / 2.0


def cam(eye, target, fov):
    dx, dy, dz = (t - e for t, e in zip(target, eye))
    yaw = math.degrees(math.atan2(-dx, dz))
    pitch = math.degrees(-math.atan2(dy, math.hypot(dx, dz)))
    return f"{eye[0]:.1f},{eye[1]:.1f},{eye[2]:.1f},{yaw:.1f},{pitch:.1f},{fov}"


SUN_ALT = math.radians(38.0)
SUN_AZ = 2.4  # radians; picked from four trial azimuths by eye


def chunky_scene(name, c, world_dir, spp, size):
    x, y, z, yaw, pitch, fov = (float(v) for v in c.split(","))
    X_, Y_, Z_ = size
    return {
        "sdfVersion": 9, "name": name, "width": 1200, "height": 669,
        "yClipMin": 56, "yClipMax": min(320, 64 + Y_ + 16),
        # authored, as a camera record's exposure is: 1.0 blows the sunlit bone out
        "exposure": 0.75,
        "postprocess": "GAMMA", "outputMode": "PNG", "renderTime": 0, "spp": 0,
        "sppTarget": int(spp), "rayDepth": 5, "pathTrace": True, "dumpFrequency": 500,
        "saveSnapshots": False, "emittersEnabled": True, "emitterIntensity": 13.0,
        "sunEnabled": True, "stillWater": False,
        # authored lighting for the spike: a mid-morning sun from the camera's
        # left, as the reference is lit (no delvec scene states this)
        "sun": {"altitude": SUN_ALT, "azimuth": SUN_AZ},
        "world": {"path": os.path.abspath(world_dir), "dimension": 0},
        "camera": {"name": "camera 1", "position": {"x": x, "y": y, "z": z},
                   "orientation": {"roll": 0.0, "pitch": math.radians(pitch) - math.pi / 2,
                                   "yaw": math.radians(-yaw - 90.0) + math.pi},
                   "projectionMode": "PINHOLE", "fov": fov},
        "chunkList": [[cx_, cz_] for cx_ in range(-1, X_ // 16 + 1) for cz_ in range(-1, Z_ // 16 + 1)],
    }


def spine_top(z):
    cols = k[int(cx) - 2:int(cx) + 3, :, z]
    ys = np.nonzero(cols.any(0))[0]
    return int(ys.max())


# the widest rib section between 30% and 60% of the length
zs = range(int(0.30 * Z), int(0.60 * Z))
width = [np.ptp(np.nonzero(k[:, :, z].any(1))[0]) if k[:, :, z].any() else 0 for z in zs]
zr = list(zs)[int(np.argmax(width))]
ring_y = np.nonzero(k[:, :, zr].any(0))[0]
# skull: the first 28% of the length
sk = k[:, :, : int(0.28 * Z)]
sk_y = np.nonzero(sk.any((0, 2)))[0]

zsp = int(0.47 * Z)
ty = spine_top(zsp)
views = {
    "far": cam((cx - 0.62 * Z, OY + 0.5 * Y + 0.12 * Z, 0.0 * Z), (cx, OY + 0.40 * Y, 0.37 * Z), 50),
    "spine": cam((cx, OY + ty + 1 + 1.62, zsp), (cx, OY + ty - 6, zsp + 40), 70),
    "ribcage": cam((cx, OY + (ring_y.min() + ring_y.max()) / 2 - 2, zr + 4),
                   (cx, OY + ring_y.max() + 10, zr - 18), 80),
    "skull": cam((cx - 0.17 * Z, OY + sk_y.max() + 0.07 * Z, 0.02 * Z),
                 (cx, OY + (sk_y.min() + sk_y.max()) / 2, 0.16 * Z), 60),
}
if CHUNKY:
    for name, c in views.items():
        stem = f"{tag}-{name}"
        with open(f"{out}/{stem}.json", "w") as f:
            json.dump(chunky_scene(stem, c, world, spp, (X, Y, Z)), f, indent=2)
        print(f"{stem}: camera {c}")
    sys.exit(0)
for name, c in views.items():
    path = f"{out}/{tag}-{name}.png"
    r = subprocess.run([delvec, "--prefabs", prefabs, "snapshot", campaign, f"--camera={c}",
                        "--width", "1200", "--height", "669", "-o", path],
                       capture_output=True, text=True)
    last = (r.stdout + r.stderr).strip().splitlines()[-1]
    print(f"{name}: camera {c} exit {r.returncode} :: {last}")
    if r.returncode != 0:
        sys.exit(r.returncode)
