#!/usr/bin/env bash
# SPIKE TOOLING (ADR-PENDING-map-design-pipeline, spec-PENDING §4.1) — NOT part
# of the shipped pipeline.
#
# The whole falsifier in one run: the three fixtures the design must red, the
# per-obligation red->green demonstrations that keep a green from being
# accidental, and the cost/cheat measurements.
#
# Deterministic and order-independent: every step is a pure function of blocks
# already on disk plus a contract, and no output carries an absolute path.
#
#   ./run-evidence.sh <artifact-root>
#
# <artifact-root> holds:
#   trial1/out/notre-dame.json            trial-0001 run 1's saved tile set
#   z7/base/z7-bell-tower.json            bell Z7 at 41x14x125 seed 1
#   z7/<drift>/z7-bell-tower.json         the same, unguarded, one knob drifted
#   bar/far-side-bar.json  bar-open/…     the barred-edge fixture and its teeth
set -euo pipefail

ROOT=${1:?usage: run-evidence.sh <artifact-root>}
HERE=$(cd "$(dirname "$0")" && pwd)
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
cd "$HERE"

chk() { python3 contract_check.py "$@" || true; }
line() { printf '\n============================================================\n%s\n============================================================\n' "$1"; }

# The gate lines only, so the table stays readable; `--list` is raised where the
# cells themselves are the evidence.
gates() { grep -E '^  [a-z-]+ +(pass|RED)|^VERDICT'; }

line "FIXTURE 1+2 — trial-0001 run 1 under an honest hand-written contract"
chk "$ROOT/trial1/out/notre-dame.json" contracts/notre-dame-run1.json --list=6

line "THE CHEAPEST GREEN CONTRACT for the same artifact (the escape hatch, measured)"
chk "$ROOT/trial1/out/notre-dame.json" contracts/notre-dame-run1-vacuous.json --list=0

line "FIXTURE 3 — bell Z7 at the fixture region and seed"
python3 z7-contract.py > "$WORK/z7.json"
chk "$ROOT/z7/base/z7-bell-tower.json" "$WORK/z7.json" --list=0

for drift in flight-tread-1 flight-landing_run-4; do
  line "FIXTURE 3 — drift ${drift//-/ } (guard removed so the drift reaches geometry)"
  chk "$ROOT/z7/$drift/z7-bell-tower.json" "$WORK/z7.json" --list=0 | gates
done
line "FIXTURE 3 — drift ring_run 22 (guard removed; contract recomputed for the new runs)"
python3 z7-contract.py ring_run=22 > "$WORK/z7-r22.json"
chk "$ROOT/z7/ring_run-22/z7-bell-tower.json" "$WORK/z7-r22.json" --list=0 | gates

line "EDGE CLASS barred — as built, and its teeth (far_side_bar --param unbarred=1)"
chk "$ROOT/bar/far-side-bar.json" contracts/far-side-bar.json --list=0 | gates
chk "$ROOT/bar-open/far-side-bar.json" contracts/far-side-bar.json --list=0 | gates

line "EXTERIOR IS ONE NODE: give the far side its own face contract and the bar stops gating"
python3 - "$WORK/bar-2faces.json" <<'PY'
import json, sys
c = json.load(open("contracts/far-side-bar.json"))
c["edges"].append({"a": "far", "b": "exterior", "class": "walk"})
json.dump(c, open(sys.argv[1], "w"), indent=1)
PY
chk "$ROOT/bar/far-side-bar.json" "$WORK/bar-2faces.json" --list=0 | grep -E 'reachability|VERDICT'

line "RED->GREEN per obligation, from Z7's all-green baseline"
Z7=$ROOT/z7/base/z7-bell-tower.json

echo "-- well-formed: two spaces made to overlap"
python3 - "$WORK/z7.json" "$WORK/m.json" <<'PY'
import json, sys
c = json.load(open(sys.argv[1]))
for s in c["spaces"]:
    if s["name"] == "ring":
        s["box"][5] += 4
json.dump(c, open(sys.argv[2], "w"), indent=1)
PY
chk "$Z7" "$WORK/m.json" --list=0 | grep -E 'well-formed|VERDICT'

echo "-- coverage: one space deleted from the contract"
python3 - "$WORK/z7.json" "$WORK/m.json" <<'PY'
import json, sys
c = json.load(open(sys.argv[1]))
c["spaces"] = [s for s in c["spaces"] if s["name"] != "ring"]
c["edges"] = [e for e in c["edges"] if "ring" not in (e["a"], e["b"])]
json.dump(c, open(sys.argv[2], "w"), indent=1)
PY
chk "$Z7" "$WORK/m.json" --list=3 | grep -E 'coverage|VERDICT|^      [0-9]'

echo "-- closure: one cell of the ring's own floor voided in the delivered blocks"
chk "$Z7" "$WORK/z7.json" --list=3 --void=30,7,10,30,7,10 | grep -E 'closure|VERDICT|^      [0-9]'

echo "-- edge-proof/walk: the boss doorband filled solid"
chk "$Z7" "$WORK/z7.json" --list=0 --fill=22,8,19,40,13,20 | grep -E 'edge-proof|VERDICT'

echo "-- edge-proof/drop: the shaft mouth filled, so nothing falls into it"
chk "$Z7" "$WORK/z7.json" --list=0 --fill=18,9,49,20,9,51 | grep -E 'edge-proof|VERDICT'

echo "-- edge-proof/drop: the landing doorway plugged, so nothing enters the shaft"
chk "$Z7" "$WORK/z7.json" --list=0 --fill=21,9,49,21,13,51 | grep -E 'edge-proof|VERDICT'

echo "-- reachability: the lift shaft's only edge removed"
python3 - "$WORK/z7.json" "$WORK/m.json" <<'PY'
import json, sys
c = json.load(open(sys.argv[1]))
c["edges"] = [e for e in c["edges"] if "lift-shaft" not in (e["a"], e["b"])]
json.dump(c, open(sys.argv[2], "w"), indent=1)
PY
chk "$Z7" "$WORK/m.json" --list=0 | grep -E 'reachability|VERDICT'

echo "-- reachability does NOT red when the stair edge alone is removed: exterior is one"
echo "   node, so the entry face and the exit face connect the zone behind the checker's back"
python3 - "$WORK/z7.json" "$WORK/m.json" <<'PY'
import json, sys
c = json.load(open(sys.argv[1]))
c["edges"] = [e for e in c["edges"] if e["class"] != "stair"]
json.dump(c, open(sys.argv[2], "w"), indent=1)
PY
chk "$Z7" "$WORK/m.json" --list=0 | grep -E 'reachability|VERDICT'

echo "-- anchors: the loft shrunk off its own perches"
python3 - "$WORK/z7.json" "$WORK/m.json" <<'PY'
import json, sys
c = json.load(open(sys.argv[1]))
for s in c["spaces"]:
    if s["name"] == "loft":
        s["box"][4] = 11
json.dump(c, open(sys.argv[2], "w"), indent=1)
PY
chk "$Z7" "$WORK/m.json" --list=0 | grep -E 'anchors|VERDICT'

echo "-- exterior-faces: the exit face contract removed"
python3 - "$WORK/z7.json" "$WORK/m.json" <<'PY'
import json, sys
c = json.load(open(sys.argv[1]))
c["edges"] = [e for e in c["edges"] if e["b"] != "exterior"]
json.dump(c, open(sys.argv[2], "w"), indent=1)
PY
chk "$Z7" "$WORK/m.json" --list=0 | grep -E 'exterior-faces|VERDICT'

line "CONTRACT COST"
for f in contracts/notre-dame-run1.json contracts/bell-z7.json contracts/far-side-bar.json \
         contracts/notre-dame-run1-vacuous.json; do
  python3 - "$f" <<'PY'
import json, sys
p = sys.argv[1]
c = json.load(open(p))
lines = sum(1 for _ in open(p))
cls = {}
for e in c["edges"]:
    cls[e["class"]] = cls.get(e["class"], 0) + 1
print(f"  {p.split('/')[-1]:34s} {len(c['spaces']):3d} space(s)  "
      f"{len(c['no_body']):3d} no_body  {len(c['edges']):3d} edge(s)  "
      f"{lines:3d} lines   {' '.join(f'{k}:{v}' for k, v in sorted(cls.items()))}")
PY
done
