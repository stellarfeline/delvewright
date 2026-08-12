#!/usr/bin/env bash
# SPIKE TOOLING (ADR-PENDING-map-design-pipeline, spec-PENDING §4.1) — NOT part
# of the shipped pipeline.
#
# The whole falsifier in one run, against the obligations as amended twice
# (§0 — an opt-out must be secured by a property the defect cannot supply):
# the three fixtures, AC8's two adversary scripts, AC9's first-party anchor
# discrimination, AC10's one-floor teeth, AC11's exterior/via teeth, the
# per-obligation red->green demonstrations, the cost table, and the third round
# of attacks.
#
# Deterministic and order-independent; no output carries an absolute path.
#
#   ./run-evidence.sh <artifact-root>
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

line "ONE-FLOOR RULE — the highest-risk new constraint, measured first"
python3 - "$ND" contracts/notre-dame-run1.json "$Z7" "$WORK/z7.json" <<'PY'
import json, sys
import contract_check as cc
from voxels import Grid
for man, con in ((sys.argv[1], sys.argv[2]), (sys.argv[3], sys.argv[4])):
    grid, _ = Grid.from_manifest(man)
    c = json.load(open(con))
    S = grid.standable_cells()
    nb = [r["boxes"] for r in c["no_body"]]
    bad = 0
    print(f"  {c['zone'][:60]}")
    for s in c["spaces"]:
        cells = [p for p in S if cc.in_any(s["boxes"], p)
                 and not any(cc.in_any(b, p) for b in nb)]
        ys = sorted({p[1] for p in cells})
        span = (max(ys) - min(ys) + 1) if ys else 0
        if span > 2:
            bad += 1
            print(f"    VIOLATES  {s['name']:18s} y {min(ys)}..{max(ys)} ({span} levels)")
    print(f"    -> {bad} of {len(c['spaces'])} spaces violate it in the FINAL contracts")
PY

line "FIXTURE 1+2 — trial-0001 run 1 under the honest contract, final rules"
chk "$ND" contracts/notre-dame-run1.json --list=6

line "FIXTURE 1 cross-check (AC2): unreached count against an independent probe"
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
print(f"    {len(owned)} owned cell(s), {len(owned & phys)} reached by a physical walk, "
      f"{len(owned - phys)} stranded")
print(f"    the contract's graph-confined walk reports 465 unreached")
print(f"    two implementations agree: {len(owned - phys) == 465}")
PY

line "FIXTURE 3 — bell Z7 at the fixture region and seed"
chk "$Z7" "$WORK/z7.json" --list=6

for drift in flight-tread-1 flight-landing_run-4; do
  line "FIXTURE 3 — drift ${drift//-/ } (guard removed so the drift reaches geometry)"
  chk "$ROOT/z7/$drift/z7-bell-tower.json" "$WORK/z7.json" --list=0 | gates
done
line "FIXTURE 3 — drift ring_run 22 (contract recomputed for the new runs)"
python3 z7-contract.py ring_run=22 > "$WORK/z7-r22.json"
chk "$ROOT/z7/ring_run-22/z7-bell-tower.json" "$WORK/z7-r22.json" --list=0 | gates
echo "  (the fourth drift, shaft/sill = 10, is refused by lift_shaft's own validity guard"
echo "   and produces no bytes; per AC5 that refusal is the channel, not this checker)"

line "AC8 — both adversary scripts, verbatim, with and without the acknowledgement"
echo "-- adversary 1: the 26-line all-no_body contract"
chk "$ND" contracts/notre-dame-run1-vacuous.json --list=0 | gates
echo "-- adversary 1, with no_body_majority_ack:"
chk "$ND" contracts/notre-dame-run1-vacuous-acked.json --list=0 | gates
echo "-- adversary 2: cheat.py, verbatim (seal the unreached, open the breached, acknowledge)"
python3 cheat.py "$ND" contracts/notre-dame-run1.json "$WORK/cheat.json"
chk "$ND" "$WORK/cheat.json" --list=0 | gates

line "AC9 — first-party anchor resolution and the stranded-belfry discrimination"
echo "-- rafter_hall's perches WITH their real anchors, declared \`posted\`:"
chk "$Z7" "$WORK/z7.json" --list=0 | grep -E '^  no-body|^  anchors|VERDICT'
echo "-- the same shelves stripped of every anchor, tried under all three kinds:"
python3 - "$Z7" "$WORK/z7.json" "$WORK" <<'PY'
import json, subprocess, sys
man = json.load(open(sys.argv[1]))
man["anchors"] = {k: v for k, v in man["anchors"].items() if not k.startswith("anchor/perch")}
json.dump(man, open(sys.argv[3] + "/z7-noanchor.json", "w"))
for kind in ("sealed", "open", "posted"):
    c = json.load(open(sys.argv[2]))
    c["no_body"][0]["kind"] = kind
    json.dump(c, open(sys.argv[3] + f"/z7-{kind}.json", "w"))
PY
cp "$ROOT"/z7/base/*.nbt "$WORK"/ 2>/dev/null || true
for kind in sealed open posted; do
  printf '   %-8s ' "$kind"
  chk "$WORK/z7-noanchor.json" "$WORK/z7-$kind.json" --list=0 | grep -E '^  no-body' || true
done

line "AC10 — the one-floor teeth: Z7's stair-foot and stair-head merged into one space"
chk "$Z7" contracts/bell-z7-merged.json --list=0 | grep -E 'well-formed|VERDICT'

line "AC11 — exterior carries no connectivity, and a via off its shared boundary is refused"
mut "$WORK/z7.json" "$WORK/m.json" <<'PY'
import json, sys
c = json.load(open(sys.argv[1]))
c["edges"] = [e for e in c["edges"] if e["class"] != "stair"]
json.dump(c, open(sys.argv[2], "w"), indent=1)
PY
chk "$Z7" "$WORK/m.json" --list=0 | grep -E 'reachability|VERDICT'
echo "-- and the round-2 five-vias-over-the-breaches cheat:"
chk "$ND" contracts/notre-dame-run1-via.json --list=0 | grep -E 'well-formed|VERDICT'

line "EDGE CLASS barred — as built (stratified), and its teeth (--param unbarred=1)"
chk "$ROOT/bar/far-side-bar.json" contracts/far-side-bar.json --list=0 | gates
chk "$ROOT/bar-open/far-side-bar.json" contracts/far-side-bar.json --list=0 | gates

line "RED->GREEN per obligation, from Z7's baseline"

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

echo "-- well-formed: a stair with no via, and an entry with no exterior edge"
mut "$WORK/z7.json" "$WORK/m.json" <<'PY'
import json, sys
c = json.load(open(sys.argv[1]))
for e in c["edges"]:
    if e["class"] == "stair":
        del e["via"]
c["edges"] = [e for e in c["edges"] if not (e["b"] == "stair-foot" and e["a"] == "exterior")]
json.dump(c, open(sys.argv[2], "w"), indent=1)
PY
chk "$Z7" "$WORK/m.json" --list=0 | grep -E 'well-formed|VERDICT'

echo "-- well-formed: a transit volume overlapping a space"
mut "$WORK/z7.json" "$WORK/m.json" <<'PY'
import json, sys
c = json.load(open(sys.argv[1]))
for e in c["edges"]:
    if e["class"] == "stair":
        e["via"] = [[22, 0, 100, 40, 13, 119]]
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

echo "-- closure/envelope-sky: a roofed space downgraded to \`open\` (AC4's teeth)"
mut "$WORK/z7.json" "$WORK/m.json" <<'PY'
import json, sys
c = json.load(open(sys.argv[1]))
for s in c["spaces"]:
    if s["name"] == "ring":
        s["envelope"] = "open"
json.dump(c, open(sys.argv[2], "w"), indent=1)
PY
chk "$Z7" "$WORK/m.json" --list=0 | grep -E 'closure|VERDICT'

echo "-- edge-proof/walk: the boss doorband filled solid"
chk "$Z7" "$WORK/z7.json" --list=0 --fill=22,8,19,40,13,20 | grep -E 'edge-proof|VERDICT'

echo "-- edge-proof/drop: the shaft mouth filled, so the drop is walkable both ways"
chk "$Z7" "$WORK/z7.json" --list=0 --fill=18,9,49,20,9,51 | grep -E 'edge-proof|VERDICT'

echo "-- edge-proof/drop: the landing doorway plugged, so nothing enters the shaft"
chk "$Z7" "$WORK/z7.json" --list=0 --fill=21,9,49,21,13,51 | grep -E 'edge-proof|VERDICT'

echo "-- edge-proof/rise: geometry one course off the declared relation (AC3's rise teeth)"
chk "$ROOT/z7/flight-landing_run-4/z7-bell-tower.json" "$WORK/z7.json" --list=0 \
  | grep -E 'edge-proof|VERDICT'

echo "-- reachability: the loft's floor severed from the hearth"
chk "$Z7" "$WORK/z7.json" --list=0 --fill=22,8,80,40,13,81 | grep -E 'reachability|VERDICT'

echo "-- no-body/sealed: a region a body reaches, declared sealed"
mut "$WORK/z7.json" "$WORK/m.json" <<'PY'
import json, sys
c = json.load(open(sys.argv[1]))
c["no_body"][0]["kind"] = "sealed"
json.dump(c, open(sys.argv[2], "w"), indent=1)
PY
chk "$Z7" "$WORK/m.json" --list=0 | grep -E 'no-body|VERDICT'

echo "-- no-body/open: a roofed region declared open"
mut "$WORK/z7.json" "$WORK/m.json" <<'PY'
import json, sys
c = json.load(open(sys.argv[1]))
c["no_body"].append({"name": "ring-floor", "kind": "open",
                     "reason": "claiming a roofed strip of the arena is exterior decoration",
                     "boxes": [[24, 9, 2, 30, 10, 8]]})
json.dump(c, open(sys.argv[2], "w"), indent=1)
PY
chk "$Z7" "$WORK/m.json" --list=0 | grep -E 'no-body|VERDICT'

echo "-- no-body/posted: the anchors moved out of the region"
mut "$WORK/z7.json" "$WORK/m.json" <<'PY'
import json, sys
c = json.load(open(sys.argv[1]))
c["no_body"][0]["boxes"] = [[26, 12, 61, 27, 13, 80]]
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

echo "-- vacuity: an unacknowledged no_body majority"
mut contracts/notre-dame-run1.json "$WORK/m.json" <<'PY'
import json, sys
c = json.load(open(sys.argv[1]))
c.pop("no_body_majority_ack", None)
json.dump(c, open(sys.argv[2], "w"), indent=1)
PY
chk "$ND" "$WORK/m.json" --list=0 | grep -E 'vacuity|VERDICT'

line "ROUND-3 ATTACKS — the third mechanical-defeat attempt (§0's own revisit trigger)"
echo "-- F: pad the transit volume. §2.5 asks for cells of a declared SPACE, and a"
echo "      transit volume is not one; \`via\` takes a list; nothing bounds its size."
python3 cheat2.py "$ND" contracts/notre-dame-run1.json "$WORK/f.json" --attack=F
chk "$ND" "$WORK/f.json" --list=0 --literal-targets | gates
echo "   the same contract under this prototype's stricter reading (transit cells ARE"
echo "   reachability targets), which is the one-sentence fix:"
chk "$ND" "$WORK/f.json" --list=0 | grep -E 'reachability|VERDICT'

echo
echo "-- G: shop for a kind. §2.6 lets a region satisfying several declare any one,"
echo "      so the effective obligation is their disjunction."
python3 cheat2.py "$Z7" "$WORK/z7.json" "$WORK/g.json" --attack=G
chk "$Z7" "$WORK/g.json" --list=0 | gates

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
kinds = {}
for r in c["no_body"]:
    kinds[r["kind"]] = kinds.get(r["kind"], 0) + 1
print(f"  {p.split('/')[-1]:32s} {len(c['spaces']):3d} space(s)/{boxes:3d} box(es)  "
      f"{len(c['no_body']):3d} no_body/{nbox:3d} box(es)  {len(c['edges']):3d} edge(s)  "
      f"{lines:3d} lines   {' '.join(f'{k}:{v}' for k, v in sorted(cls.items()))}"
      f"   [{' '.join(f'{k}:{v}' for k, v in sorted(kinds.items()))}]")
PY
done
