#!/usr/bin/env bash
# SPIKE TOOLING (ADR-PENDING-map-design-pipeline, spec-PENDING §4.1) — NOT part
# of the shipped pipeline.
#
# The whole falsifier in one run, against the obligations amended on 2026-08-12:
# the three fixtures the design must red, AC8's escape-hatch fixture, the
# per-obligation red->green demonstrations, the cost table, and the attacks that
# still get a broken artifact to green.
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

ND=$ROOT/trial1/out/notre-dame.json
Z7=$ROOT/z7/base/z7-bell-tower.json

chk() { python3 contract_check.py "$@" || true; }
gates() { grep -E '^  [a-z-]+ +(pass|RED)|^VERDICT'; }
line() { printf '\n============================================================\n%s\n============================================================\n' "$1"; }
mut() { python3 - "$@"; }

python3 z7-contract.py > "$WORK/z7.json"

line "FIXTURE 1+2 — trial-0001 run 1 under the honest contract, amended obligations"
chk "$ND" contracts/notre-dame-run1.json --list=6

line "FIXTURE 1 cross-check (AC2): the unreached count against an independent probe"
python3 - "$ND" contracts/notre-dame-run1.json <<'PY'
import json, sys
import contract_check as cc
from voxels import Grid
grid, _ = Grid.from_manifest(sys.argv[1])
c = json.load(open(sys.argv[2]))
sb = {s["name"]: s["boxes"] for s in c["spaces"]}
nb = [n["boxes"] for n in c["no_body"]]
owned = set(p for p in grid.standable_cells()
            if any(cc.in_any(b, p) for b in sb.values())
            and not any(cc.in_any(b, p) for b in nb))
seeds = set(p for p in owned if cc.in_any(sb[c["entry"]], p))
phys = cc.physical_reach(grid, seeds)
print(f"  independent probe (physical walk, contract ignored except for which cells count):")
print(f"    {len(owned)} owned cell(s), {len(owned & phys)} reached, {len(owned - phys)} stranded")
print(f"  the contract's graph-confined walk reports 169 unreached")
print(f"  two implementations agree: {len(owned - phys) == 169}")
PY

line "FIXTURE 3 — bell Z7 at the fixture region and seed"
chk "$Z7" "$WORK/z7.json" --list=0

for drift in flight-tread-1 flight-landing_run-4; do
  line "FIXTURE 3 — drift ${drift//-/ } (guard removed so the drift reaches geometry)"
  chk "$ROOT/z7/$drift/z7-bell-tower.json" "$WORK/z7.json" --list=0 | gates
done
line "FIXTURE 3 — drift ring_run 22 (guard removed; contract recomputed for the new runs)"
python3 z7-contract.py ring_run=22 > "$WORK/z7-r22.json"
chk "$ROOT/z7/ring_run-22/z7-bell-tower.json" "$WORK/z7-r22.json" --list=0 | gates
echo "  (the fourth drift, shaft/sill = 10, is refused by lift_shaft's own validity guard and"
echo "   produces no bytes; per AC5 that refusal is the channel, not this checker)"

line "AC8 — the prototype's own escape-hatch contract, ported and re-run"
chk "$ND" contracts/notre-dame-run1-vacuous.json --list=0 | gates
echo "  ... and the same contract with no_body_majority_ack added:"
chk "$ND" contracts/notre-dame-run1-vacuous-acked.json --list=0 | gates

line "EDGE CLASS barred — as built (stratified), and its teeth (--param unbarred=1)"
chk "$ROOT/bar/far-side-bar.json" contracts/far-side-bar.json --list=0 | gates
chk "$ROOT/bar-open/far-side-bar.json" contracts/far-side-bar.json --list=0 | gates

line "RED->GREEN per obligation, from Z7's all-green baseline"

echo "-- well-formed: two spaces made to overlap"
mut "$WORK/z7.json" "$WORK/m.json" <<'PY'
import json, sys
c = json.load(open(sys.argv[1]))
for s in c["spaces"]:
    if s["name"] == "ring":
        s["boxes"][0][5] += 4
json.dump(c, open(sys.argv[2], "w"), indent=1)
PY
chk "$Z7" "$WORK/m.json" --list=0 | grep -E 'well-formed|VERDICT'

echo "-- well-formed: a no_body region straddling two spaces instead of nesting in one"
mut "$WORK/z7.json" "$WORK/m.json" <<'PY'
import json, sys
c = json.load(open(sys.argv[1]))
c["no_body"][0]["boxes"] = [[22, 12, 55, 40, 13, 85]]
json.dump(c, open(sys.argv[2], "w"), indent=1)
PY
chk "$Z7" "$WORK/m.json" --list=0 | grep -E 'well-formed|VERDICT'

echo "-- well-formed: a stair with no declared rise, and a drop declared rising"
mut "$WORK/z7.json" "$WORK/m.json" <<'PY'
import json, sys
c = json.load(open(sys.argv[1]))
for e in c["edges"]:
    if e["class"] == "stair":
        del e["rise"]
    if e["class"] == "drop":
        e["rise"] = 3
json.dump(c, open(sys.argv[2], "w"), indent=1)
PY
chk "$Z7" "$WORK/m.json" --list=0 | grep -E 'well-formed|VERDICT'

echo "-- well-formed: a no_body region with an empty reason"
mut "$WORK/z7.json" "$WORK/m.json" <<'PY'
import json, sys
c = json.load(open(sys.argv[1]))
c["no_body"][0]["reason"] = "  "
json.dump(c, open(sys.argv[2], "w"), indent=1)
PY
chk "$Z7" "$WORK/m.json" --list=0 | grep -E 'well-formed|VERDICT'

echo "-- coverage: one space deleted from the contract"
mut "$WORK/z7.json" "$WORK/m.json" <<'PY'
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

echo "-- edge-proof/drop: the shaft mouth filled, so the drop is walkable both ways"
chk "$Z7" "$WORK/z7.json" --list=0 --fill=18,9,49,20,9,51 | grep -E 'edge-proof|VERDICT'

echo "-- edge-proof/drop: the landing doorway plugged, so nothing enters the shaft"
chk "$Z7" "$WORK/z7.json" --list=0 --fill=21,9,49,21,13,51 | grep -E 'edge-proof|VERDICT'

echo "-- edge-proof/rise: geometry one course off the declared relation (AC3's rise teeth)"
chk "$ROOT/z7/flight-landing_run-4/z7-bell-tower.json" "$WORK/z7.json" --list=0 \
  | grep -E 'edge-proof|VERDICT'

echo "-- reachability: the stair edge deleted (AC11 — exterior must not carry connectivity)"
mut "$WORK/z7.json" "$WORK/m.json" <<'PY'
import json, sys
c = json.load(open(sys.argv[1]))
c["edges"] = [e for e in c["edges"] if e["class"] != "stair"]
json.dump(c, open(sys.argv[2], "w"), indent=1)
PY
chk "$Z7" "$WORK/m.json" --list=0 | grep -E 'reachability|VERDICT'
echo "   the same contract under the PHYSICAL reading of §2.5, which the spec's wording"
echo "   also admits — AC11 does not hold there:"
chk "$Z7" "$WORK/m.json" --list=0 --physical-reach | grep -E 'reachability|VERDICT'

echo "-- no-body/sealed: a region a body reaches, declared sealed"
mut "$WORK/z7.json" "$WORK/m.json" <<'PY'
import json, sys
c = json.load(open(sys.argv[1]))
c["no_body"][0]["boxes"].append([23, 9, 62, 30, 9, 70])   # loft floor, plainly walked
json.dump(c, open(sys.argv[2], "w"), indent=1)
PY
chk "$Z7" "$WORK/m.json" --list=0 | grep -E 'no-body|VERDICT'

echo "-- no-body/open: a roofed region declared open (a strip of the ring's own floor,"
echo "   which carries the boss ring's ceiling above it)"
mut "$WORK/z7.json" "$WORK/m.json" <<'PY'
import json, sys
c = json.load(open(sys.argv[1]))
c["no_body"].append({"name": "ring-floor", "kind": "open",
                     "reason": "claiming a roofed strip of the arena is exterior decoration",
                     "boxes": [[24, 9, 2, 30, 10, 8]]})
json.dump(c, open(sys.argv[2], "w"), indent=1)
PY
chk "$Z7" "$WORK/m.json" --list=0 | grep -E 'no-body|VERDICT'
echo "   NOTE: the loft rafters pass as EITHER kind, and so does the lift pit at the foot"
echo "   of a 13-deep shaft. 'no solid above within the artifact' is free for anything in"
echo "   the topmost slab and for anything under a hole, so an interior beam and the floor"
echo "   of a pit both read as exterior decoration:"
mut "$WORK/z7.json" "$WORK/m.json" <<'PY'
import json, sys
c = json.load(open(sys.argv[1]))
c["no_body"][0]["kind"] = "open"
json.dump(c, open(sys.argv[2], "w"), indent=1)
PY
chk "$Z7" "$WORK/m.json" --list=0 | grep -E 'no-body|VERDICT'

echo "-- anchors: the loft shrunk off its own perches"
mut "$WORK/z7.json" "$WORK/m.json" <<'PY'
import json, sys
c = json.load(open(sys.argv[1]))
c["no_body"] = []
for s in c["spaces"]:
    if s["name"] == "loft":
        s["boxes"][0][4] = 10
json.dump(c, open(sys.argv[2], "w"), indent=1)
PY
chk "$Z7" "$WORK/m.json" --list=0 | grep -E 'anchors|VERDICT'

echo "-- exterior-faces: the exit face contract removed"
mut "$WORK/z7.json" "$WORK/m.json" <<'PY'
import json, sys
c = json.load(open(sys.argv[1]))
c["edges"] = [e for e in c["edges"] if e["b"] != "exterior"]
json.dump(c, open(sys.argv[2], "w"), indent=1)
PY
chk "$Z7" "$WORK/m.json" --list=0 | grep -E 'exterior-faces|VERDICT'

echo "-- vacuity: every envelope opened, so closure examines nothing"
mut "$WORK/z7.json" "$WORK/m.json" <<'PY'
import json, sys
c = json.load(open(sys.argv[1]))
for s in c["spaces"]:
    s["envelope"] = "open"
json.dump(c, open(sys.argv[2], "w"), indent=1)
PY
chk "$Z7" "$WORK/m.json" --list=0 | grep -E 'vacuity|VERDICT'

echo "-- vacuity: an unacknowledged no_body majority"
chk "$ND" contracts/notre-dame-run1.json --list=0 | grep -E 'vacuity|VERDICT'

line "ATTACKS — three ways to a green verdict on the same broken artifact"

echo "-- A: cheat.py, mechanical. Seal every unreached cell, open every broken envelope."
python3 cheat.py "$ND" contracts/notre-dame-run1.json "$WORK/cheat.json"
chk "$ND" "$WORK/cheat.json" --list=0 | gates

echo
echo "-- B: five \`via\` boxes on edges that already existed, sized over the closure breaches."
chk "$ND" contracts/notre-dame-run1-via.json --list=0 | grep -E 'closure|VERDICT'

echo
echo "-- C: on Z7, merge the flight and its head landing into one union space. No edge"
echo "      crosses the seam any more, so no level relation is declared, so the drifts pass."
for d in base flight-tread-1 flight-landing_run-4; do
  printf '     %-22s ' "$d"
  chk "$ROOT/z7/$d/z7-bell-tower.json" contracts/bell-z7-merged.json --list=0 | tail -1
done

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
boxes = sum(len(s["boxes"]) for s in c["spaces"])
nbox = sum(len(r["boxes"]) for r in c["no_body"])
print(f"  {p.split('/')[-1]:32s} {len(c['spaces']):3d} space(s) / {boxes:3d} box(es)   "
      f"{len(c['no_body']):3d} no_body / {nbox:3d} box(es)   {len(c['edges']):3d} edge(s)  "
      f"{lines:3d} lines   {' '.join(f'{k}:{v}' for k, v in sorted(cls.items()))}")
PY
done
