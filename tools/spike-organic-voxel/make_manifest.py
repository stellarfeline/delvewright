"""Turn `delvec schem convert`'s <base>.split.json into a prefab manifest
(`structure_set` form, the shape `delvec grammar expand` writes for a tiled zone)
so `delvec prefab lighting|audit`, `delvec snapshot` and `delvec render piece`
can read the voxelised whale. Spike code.

usage: make_manifest.py <split.json>[,<split.json>...] <report.json> <anchors.json|-> <out.json>
  several split files are z-slabs of one piece (`<base>.z<z0>.split.json`, see
  voxelize.py SCHEM_MAX_CELLS); their parts are offset by z0 and merged
"""

import json
import sys

split_p, report_p, anchors_p, out_p = sys.argv[1:5]
import re

rep = json.load(open(report_p))
anchors = {} if anchors_p == "-" else json.load(open(anchors_p))
parts = []
size = None
for sp in split_p.split(","):
    split = json.load(open(sp))
    m = re.match(r"^(.*)\.z(\d+)$", split["base"])
    base, z0 = (m.group(1), int(m.group(2))) if m else (split["base"], 0)
    zg0 = z0 // split["part_max"]
    w, h, l = split["source_size"]
    size = [w, h, z0 + l] if size is None else [w, h, max(size[2], z0 + l)]
    for p in split["parts"]:
        gi = [p["grid_index"][0], p["grid_index"][1], p["grid_index"][2] + zg0]
        parts.append({"file": p["file"], "id": p["file"][: -len(".nbt")],
                      "grid_index": gi, "offset": [p["offset"][0], p["offset"][1], p["offset"][2] + z0],
                      "size": p["size"]})
parts.sort(key=lambda p: (p["grid_index"][0], p["grid_index"][1], p["grid_index"][2]))
grid = [max(p["grid_index"][i] for p in parts) + 1 for i in range(3)]
if rep.get("source") == "sdf":
    LICENSE = {
        "source": "original",
        "spdx": "GPL-3.0-or-later",
        "note": "Original Delvewright spike asset: implicit primitives authored in tools/spike-organic-voxel/sdf_whale.py; no third-party geometry.",
        "provenance": (f"tools/spike-organic-voxel/sdf_whale.py sha256={rep['program_sha256']} --length "
                       f"{rep['length_blocks']} --sub {rep['sub']} --seed {rep['seed']}; schem sha256="
                       f"{rep['schem_sha256']}; converted by delvec schem convert"),
    }
else:
    LICENSE = {
        "source": "figshare",
        "spdx": "CC0-1.0",
        "note": ("Voxelised from the CC0 digital reconstruction of the Cetotherium riabinini "
                 "holotype skeleton (NMNHU-P OF 668/1), Davydenko, Kovalchuk, Otriazhyi & "
                 "Gol'din, figshare doi:10.6084/m9.figshare.29644028 (CC0)."),
        "provenance": (f"mesh sha256={rep['mesh_sha256']} ({rep['mesh_triangles']} triangles); "
                       f"tools/spike-organic-voxel/voxelize.py --length {rep['length_blocks']} "
                       f"--sub {rep['sub']} --seed {rep['seed']}; schem sha256={rep['schem_sha256']}; "
                       f"converted by delvec schem convert"),
    }
doc = {
    "prefab_id": f"prefab/{base}",
    "structure_set": {
        "base": base, "size": size, "part_max": split["part_max"],
        "grid": grid, "data_version": split["data_version"],
        "generator": "tools/spike-organic-voxel (spike, not an engine surface)", "parts": parts,
    },
    "anchors": anchors,
    "connectors": [],
    "lighting": {"profile": "unmeasured"},
    "license": LICENSE,
}
with open(out_p, "w") as f:
    json.dump(doc, f, indent=2)
    f.write("\n")
