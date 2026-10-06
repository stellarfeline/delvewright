#!/usr/bin/env bash
# Spike: read what the engine's walk instruments say about a floating piece.
# Works on a COPY of the prefab directory (planes --write and the entry anchor
# are written there, never into the run.sh output).
#
#   light probe   delvec prefab lighting
#   walk plane    delvec prefab planes --write
#   build         delvec build on the stub campaign, with an `entry` anchor on
#                 the spine top at 47% of the length and one reach-anchor quest
#
# usage: gaps.sh <delvec-binary> <scratch-dir> <base>
set -uo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
delvec="$1"; scratch="$(cd "$2" && pwd)"; base="$3"
work="$scratch/gaps-$base"
rm -rf "$work"; mkdir -p "$work"
cp -R "$scratch/prefab-$base" "$work/prefab"
cp -R "$here/campaign" "$work/campaign"
sed -i.bak "s#prefab/whale-LEN#prefab/$base#" "$work/campaign/world.json" && rm "$work/campaign/world.json.bak"
uv run -q --with numpy==2.3.3 python - "$scratch/out/$base.npy" "$work/prefab/$base.json" <<'EOF'
import json, sys
import numpy as np
sys.stdout.reconfigure(newline="\n")
k = np.load(sys.argv[1]) != 0
X, Y, Z = k.shape
cx, z = X // 2, int(0.47 * Z)
xl = int(round(cx + max(2, 0.035 * X)))
y = int(np.nonzero(k[xl, :, z])[0].max()) + 1
d = json.load(open(sys.argv[2]))
d["anchors"] = {"anchor/spine": {"pos": [xl, y, z], "facing": "south", "role": "entry"}}
json.dump(d, open(sys.argv[2], "w"), indent=2)
print(f"entry anchor: [{xl}, {y}, {z}]")
EOF
# the quest document takes its dsl_version from the stub campaign's own world.json
python3 - "$work/campaign" <<'PY'
import json, sys
camp = sys.argv[1]
ver = json.load(open(f"{camp}/world.json"))["dsl_version"]
obj = {"anchor": "anchor/spine", "happening": {"text": "the party completes obj/spine", "verb": "arrives"},
       "id": "obj/spine", "radius": 2, "type": "reach-anchor"}
quest = {"happening": {"text": "the party takes on quest/look", "verb": "learns"}, "id": "quest/look",
         "objectives": [obj],
         "on_complete": [{"happening": {"text": "the delve is complete", "verb": "survives"}, "type": "campaign-complete"}],
         "trigger": {"type": "campaign-start"}}
doc = {"campaign_id": "organic-whale", "content": {"quests": [quest]}, "dsl_version": ver, "stage": "quests"}
json.dump(doc, open(f"{camp}/quests.json", "w"), indent=2)
PY
echo "== light probe"
"$delvec" prefab lighting "$work/prefab/$base.json" 2>&1 | grep -E "^DW0752|^DW0751|^light|^enclosure" | cut -c1-400
echo "== walk plane"
"$delvec" prefab planes "$work/prefab/$base.json" --write 2>&1 | grep -E "binding" | cut -c1-300
echo "== build"
"$delvec" --prefabs "$work/prefab" build "$work/campaign" -o "$work/build" 2>&1 | grep -E "\[error\]" | cut -c1-500
echo "build exit ${PIPESTATUS[0]}"
