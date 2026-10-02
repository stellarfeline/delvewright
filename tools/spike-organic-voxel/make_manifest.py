"""Turn `delvec schem convert`'s <base>.split.json into a prefab manifest
(`structure_set` form, the shape `delvec grammar expand` writes for a tiled zone)
so `delvec prefab lighting|audit`, `delvec snapshot` and `delvec render piece`
can read the voxelised whale. Spike code.

usage: make_manifest.py <dir>/<base>.split.json <report.json> <anchors.json|-> <out.json>
"""

import json
import sys

split_p, report_p, anchors_p, out_p = sys.argv[1:5]
split = json.load(open(split_p))
rep = json.load(open(report_p))
base = split["base"]
anchors = {} if anchors_p == "-" else json.load(open(anchors_p))
parts = []
for p in split["parts"]:
    gi = p["grid_index"]
    parts.append({"file": p["file"], "id": f"{base}.x{gi[0]}y{gi[1]}z{gi[2]}",
                  "grid_index": gi, "offset": p["offset"], "size": p["size"]})
doc = {
    "prefab_id": f"prefab/{base}",
    "structure_set": {
        "base": base, "size": split["source_size"], "part_max": split["part_max"],
        "grid": split["grid"], "data_version": split["data_version"],
        "generator": "tools/spike-organic-voxel (spike, not an engine surface)", "parts": parts,
    },
    "anchors": anchors,
    "connectors": [],
    "lighting": {"profile": "unmeasured"},
    "license": {
        "source": "figshare",
        "spdx": "CC0-1.0",
        "note": ("Voxelised from the CC0 digital reconstruction of the Cetotherium riabinini "
                 "holotype skeleton (NMNHU-P OF 668/1), Davydenko, Kovalchuk, Otriazhyi & "
                 "Gol'din, figshare doi:10.6084/m9.figshare.29644028 (CC0)."),
        "provenance": (f"mesh sha256={rep['mesh_sha256']} ({rep['mesh_triangles']} triangles); "
                       f"tools/spike-organic-voxel/voxelize.py --length {rep['length_blocks']} "
                       f"--sub {rep['sub']} --seed {rep['seed']}; schem sha256={rep['schem_sha256']}; "
                       f"converted by delvec schem convert"),
    },
}
with open(out_p, "w") as f:
    json.dump(doc, f, indent=2)
    f.write("\n")
