#!/usr/bin/env bash
# Spike: what the engine's piece instruments say about a STRANDED organic body
# whose ground is part of its own piece (spec-0087 §2). Spike code, not an
# engine surface; the compiler consumes nothing from it.
#
#   fitter      the organic-voxel research spike, tools/spike-organic-voxel/,
#               copied into <scratch>/fitter and run unchanged
#   form        form.py — the rig's own body, over the research fitter
#   admission   delvec schem convert (tiled at 48) → make_manifest.py → prefab audit
#   readings    delvec prefab planes --write, delvec prefab lighting,
#               delvec build on the stub campaign (ocean horizon, one entry on the
#               apron, two reach objectives: inside the cavity and on the back)
#   record      <scratch>/observations.json, copied beside this script
#
# usage: run.sh <delvec-binary> <scratch-dir>
#   <delvec-binary>  a release build of the engine (the instrument; named in the record)
#   <scratch-dir>    working directory outside the repository (never /tmp)
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
delvec="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"; scratch="$2"
fitter_src="$repo/tools/spike-organic-voxel"
mkdir -p "$scratch"; scratch="$(cd "$scratch" && pwd)"
base=stranded-rig
pre="$scratch/prefab"; out="$scratch/out"; camp="$scratch/campaign"; build="$scratch/build"
rm -rf "$pre" "$out" "$camp" "$build" "$scratch/fitter"
mkdir -p "$pre" "$out" "$camp" "$scratch/fitter"

"$delvec" --version
for f in voxelize.py schem.py sdf_whale.py make_manifest.py; do
  cp "$fitter_src/$f" "$scratch/fitter/$f"
done
for f in world.json classes.json npcs.json quest-plan.json dialogue.json; do
  cp "$fitter_src/campaign/$f" "$camp/$f"
done
py=(uv run -q --with numpy==2.3.3 --with scipy==1.16.2 python)

echo "== form"
"${py[@]}" "$here/form.py" --fitter-dir "$scratch/fitter" \
    --out "$out/$base.schem" --report "$out/$base.report.json" --grid-out "$out/$base.npy" | tee "$out/form.log"

echo "== convert"
"$delvec" schem convert "$out/$base.schem" -o "$pre/$base.nbt"
mv "$pre/$base.split.json" "$out/"

echo "== anchors"
"${py[@]}" - "$out/$base.npy" "$out/anchors.json" <<'PY'
import json, sys
import numpy as np
sys.stdout.reconfigure(newline="\n")
k = np.load(sys.argv[1]) != 0  # block kind grid, x,y,z; 0 = air — the fitted body only
APRON_TOP = 3
k[:, : APRON_TOP + 1, :] = True  # the apron is laid under it at write time (form.py)
def feet_over(x, z, lo=0):
    col = np.nonzero(k[x, lo:, z])[0]
    return int(col.max()) + lo + 1
back_z = 112
back = [24, feet_over(24, back_z, lo=20), back_z]
inside = [24, APRON_TOP + 1, 72]
entry = [1, APRON_TOP + 1, 64]
for name, (x, y, z) in (("entry", entry), ("inside", inside), ("back", back)):
    assert k[x, y - 1, z] and not k[x, y, z] and not k[x, y + 1, z], (name, x, y, z)
doc = {"anchor/entry": {"pos": entry, "facing": "east", "role": "entry"},
       "anchor/inside": {"pos": inside, "facing": "south"},
       "anchor/back": {"pos": back, "facing": "south"}}
json.dump(doc, open(sys.argv[2], "w"), indent=2)
print(json.dumps(doc))
PY

echo "== manifest"
python3 "$scratch/fitter/make_manifest.py" "$out/$base.split.json" "$out/$base.report.json" "$out/anchors.json" "$pre/$base.json"
python3 - "$pre/$base.json" "$out/$base.report.json" <<'PY'
import json, sys
d = json.load(open(sys.argv[1])); rep = json.load(open(sys.argv[2]))
d["license"] = {
    "source": "original", "spdx": "GPL-3.0-or-later",
    "note": "Original Delvewright spike asset: implicit primitives authored in tools/spike-organic-stranded/form.py over the organic-voxel research fitter; no third-party geometry.",
    "provenance": f"tools/spike-organic-stranded/form.py sha256={rep['program_sha256']} --sub {rep['sub']} --seed {rep['seed']}; fitter voxelize.py sha256={rep['fitter_sha256']['voxelize.py']}; schem sha256={rep['schem_sha256']}; converted by delvec schem convert",
}
d["structure_set"]["generator"] = "tools/spike-organic-stranded (spike, not an engine surface)"
json.dump(d, open(sys.argv[1], "w"), indent=2); open(sys.argv[1], "a").write("\n")
PY

echo "== audit"
"$delvec" prefab audit "$pre/$base.json" -o "$out/$base.audit.json" > "$out/audit.log" 2>&1 || true
cat "$out/audit.log" | cut -c1-300

echo "== planes"
"$delvec" prefab planes "$pre/$base.json" --write > "$out/planes.log" 2>&1 || true
cut -c1-400 "$out/planes.log"

echo "== lighting"
"$delvec" prefab lighting "$pre/$base.json" > "$out/lighting.log" 2>&1 || true
cut -c1-600 "$out/lighting.log"

echo "== build"
python3 - "$camp" "$base" <<'PY'
import json, sys
camp, base = sys.argv[1:3]
w = json.load(open(f"{camp}/world.json"))
w["content"]["horizon"] = "ocean"
w["content"]["boundary"] = {}  # DW0320: an ocean horizon needs a return rule
w["content"]["areas"][0]["prefab"] = f"prefab/{base}"
w["content"]["premise"] = "A stranded body on a mud flat, entered through a wound and climbed by a ramp."
w["content"]["title"] = "Stranded Body Rig"
json.dump(w, open(f"{camp}/world.json", "w"), indent=2)
ver = w["dsl_version"]
def reach(i, anchor):
    return {"anchor": anchor, "happening": {"text": f"the party completes {i}", "verb": "arrives"},
            "id": i, "radius": 2, "type": "reach-anchor"}
quest = {"happening": {"text": "the party takes on quest/look", "verb": "learns"}, "id": "quest/look",
         "objectives": [reach("obj/inside", "anchor/inside"), reach("obj/back", "anchor/back")],
         "on_complete": [{"happening": {"text": "the delve is complete", "verb": "survives"}, "type": "campaign-complete"}],
         "trigger": {"type": "campaign-start"}}
doc = {"campaign_id": "organic-whale", "content": {"quests": [quest]}, "dsl_version": ver, "stage": "quests"}
json.dump(doc, open(f"{camp}/quests.json", "w"), indent=2)
PY
set +e
"$delvec" --prefabs "$pre" build "$camp" -o "$build" > "$out/build.log" 2>&1
build_exit=$?
set -e
echo "build exit $build_exit (as authored: the body carries no light)"
grep -E 'DW[0-9]{4} \[' "$out/build.log" | cut -c1-400 || true

# The same build with the stub's one area declaring `mitigation: night-vision`,
# so the light gate (stage 9) lets the walk proofs (stage 10) run and be read.
# A rig's device, never a remedy: the ruling is that light is placed in design.
echo "== build, light gate stood down"
python3 - "$camp" <<'PY'
import json, sys
camp = sys.argv[1]
w = json.load(open(f"{camp}/world.json"))
w["content"]["areas"][0]["mitigation"] = "night-vision"
json.dump(w, open(f"{camp}/world.json", "w"), indent=2)
PY
set +e
"$delvec" --prefabs "$pre" build "$camp" -o "$build-mitigated" > "$out/build-mitigated.log" 2>&1
build_exit_mitigated=$?
set -e
echo "build exit $build_exit_mitigated"
grep -E 'DW[0-9]{4} \[|DW0921 binding|DW0885|boundary|critical' "$out/build-mitigated.log" | cut -c1-500 || true

echo "== record"
python3 - "$scratch" "$out" "$pre" "$base" "$build_exit" "$("$delvec" --version)" "$(shasum -a 256 "$fitter_src/voxelize.py" | cut -d" " -f1)" "$build_exit_mitigated" > "$scratch/observations.json" <<'PY'
import json, re, sys, hashlib
sys.stdout.reconfigure(newline="\n")
scratch, out, pre, base, build_exit, engine, fitter_sha, build_exit_mitigated = sys.argv[1:9]
rep = json.load(open(f"{out}/{base}.report.json"))
audit = json.load(open(f"{out}/{base}.audit.json")) if __import__("os").path.exists(f"{out}/{base}.audit.json") else None
meta = json.load(open(f"{pre}/{base}.json"))
def lines(path, pat):
    try:
        return [l.rstrip("\n") for l in open(path, errors="replace") if re.search(pat, l)]
    except FileNotFoundError:
        return []
def json_head(path):
    """The JSON object a `delvec prefab` command prints before its sentence."""
    text = open(path, errors="replace").read()
    start = text.find("{")
    if start < 0:
        return None
    try:
        obj, _ = json.JSONDecoder().raw_decode(text[start:])
    except json.JSONDecodeError:
        return None
    obj.pop("asset", None)
    return obj
def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()
rec = {
    "question": "do the piece instruments bind a stranded body's inside and back when its ground is part of the piece",
    "engine": engine,
    "fitter_voxelize_sha256": fitter_sha,
    "form": {"program": rep["program"], "program_sha256": rep["program_sha256"], "size_xyz": rep["size_xyz"],
             "cells": rep["size_xyz"][0] * rep["size_xyz"][1] * rep["size_xyz"][2],
             "filled": rep["filled"], "kinds": rep["kinds"], "components_kept": rep["components_kept"],
             "island_blocks_dropped": rep["island_blocks_dropped"], "sub": rep["sub"], "seed": rep["seed"],
             "palette": rep["palette"], "schem_sha256": rep["schem_sha256"]},
    "nbt_sha256": {p["file"]: sha(f"{pre}/{p['file']}") for p in meta["structure_set"]["parts"]},
    "parts": len(meta["structure_set"]["parts"]),
    "anchors": meta["anchors"],
    "walk_y_written": meta.get("walk_y"),
    "audit": None if audit is None else {k: audit.get(k) for k in ("verdict", "stairs_examined", "underspecified")},
    "planes": {"reading": json_head(f"{out}/planes.log"), "said": lines(f"{out}/planes.log", r"^planes binding|^DW0")},
    "lighting": {"reading": json_head(f"{out}/lighting.log"), "said": lines(f"{out}/lighting.log", r"^DW07")},
    "build_as_authored": {"exit": int(build_exit), "diagnostics": lines(f"{out}/build.log", r"DW[0-9]{4} \[")},
    "build_light_gate_stood_down": {"exit": int(build_exit_mitigated),
                                    "diagnostics": lines(f"{out}/build-mitigated.log", r"DW[0-9]{4} \["),
                                    "bindings": lines(f"{out}/build-mitigated.log", r"DW0921 binding|DW0885|boundary|critical")},
}
json.dump(rec, sys.stdout, indent=2); print()
PY
cp "$scratch/observations.json" "$here/observations.json"
"$delvec" fmt "$here/observations.json"
echo "wrote $here/observations.json"
