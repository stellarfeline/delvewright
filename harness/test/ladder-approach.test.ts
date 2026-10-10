// The pathfinder walks along a ladder's face (spec-0099).
//
// The real mineflayer-pathfinder over the real client physics (`sim-bot.ts`).
// The defect this guards: the library aims a node in a ladder cell one block
// above the feet (the top of the ladder's collision shape) unless the next node
// descends. A body entering the cell head-on pushes into the panel and is lifted
// there; a body walking along the panel's face is not, so the steering holds
// still at the cell's edge and the walk times out. The Treehouse Camp's walk to
// its Watch House climb stopped so, one cell short of a two-wide ladder, for 60 s.

import { test } from "node:test";
import assert from "node:assert/strict";
import { holdClimbableNodesAtFeet, holdWalkedClimbableNodes, type PathNode } from "../src/movement.ts";
import { cellKey, fmtPos, simBot, simWalk, type Cells, type SimBot } from "./sim-bot.ts";

const LADDER_WEST = "minecraft:ladder[facing=west,waterlogged=false]";

/**
 * A floor of stone at y=63 over x,z ∈ 0..10, a trunk of logs at x=6 over
 * z ∈ 3..7, and a ladder TWO columns wide hung on the trunk's west face at x=5,
 * z ∈ {4, 5}, y ∈ 64..70 — The Treehouse Camp's Watch House ladder in
 * miniature. `facing=west`: the panel sits on the cell's east face.
 */
function twoWideLadder(): Cells {
  const cells: Cells = new Map();
  for (let x = 0; x <= 10; x++) for (let z = 0; z <= 10; z++) cells.set(cellKey(x, 63, z), "minecraft:stone");
  for (let z = 3; z <= 7; z++) {
    for (let y = 64; y <= 72; y++) cells.set(cellKey(6, y, z), "minecraft:oak_log[axis=y]");
  }
  for (const z of [4, 5]) for (let y = 64; y <= 70; y++) cells.set(cellKey(5, y, z), LADDER_WEST);
  return cells;
}

const held = (bot: SimBot): void => holdWalkedClimbableNodes(bot as never);

/**
 * Two cells short of the near column, standing: the walk has no sprint run-up to
 * carry the body into the cell past the library's aim (a run-up from five cells
 * out does, which is why the defect read as the Watch House's alone).
 */
const ALONG_THE_FACE: readonly [number, number, number] = [5.5, 64, 2.5];

test("without the hold, a walk along a ladder's face stops at its edge (the defect, reproduced)", () => {
  // Leg 6 of The Treehouse Camp in miniature: come south along the panel's face
  // to the far column's foot (range 1). The cell one short of it is the near
  // column — a ladder cell — so the pathfinder ends its path there, aims that
  // node a block up, and never enters it.
  const r = simWalk(simBot(twoWideLadder(), ALONG_THE_FACE), [5, 64, 5], 1, 200);
  assert.equal(r.reached, false, `the library's own aim reached the goal at ${fmtPos(r.at)}`);
  assert.ok(r.at[2] < 4, `the body entered the ladder at ${fmtPos(r.at)}`);
});

test("a walk along a ladder's face reaches the far column's foot, on the floor", () => {
  const r = simWalk(simBot(twoWideLadder(), ALONG_THE_FACE, held), [5, 64, 5], 1);
  assert.ok(r.reached, `stopped at ${fmtPos(r.at)}`);
  assert.ok(Math.abs(r.at[1] - 64) < 0.01, `the body left the floor: ${fmtPos(r.at)}`);
});

test("a walk along a ladder's face passes through both columns to the far side", () => {
  // Range 0 past the ladder: every node of both columns is walked through.
  const r = simWalk(simBot(twoWideLadder(), ALONG_THE_FACE, held), [5, 64, 8], 0);
  assert.ok(r.reached, `stopped at ${fmtPos(r.at)}`);
});

test("a walk into a ladder head-on reaches it, with the hold and without", () => {
  // The approach The Treehouse Camp's glade climb takes: toward the panel.
  for (const prepare of [undefined, held]) {
    const r = simWalk(simBot(twoWideLadder(), [1.5, 64, 4.5], prepare), [5, 64, 4], 0);
    assert.ok(r.reached, `hold=${prepare !== undefined}: stopped at ${fmtPos(r.at)}`);
  }
});

// --- the rule itself ---------------------------------------------------------

/** A node as `postProcessPath` leaves it: a ladder cell aimed at its shape's top. */
function node(x: number, y: number, z: number, raised: boolean): PathNode {
  return { x: x + 0.7, y: raised ? y + 1 : y, z: z + 0.5, hash: `${x},${y},${z}`, toBreak: [], toPlace: [] };
}

test("a ladder node entered level and left level or lower is aimed at the feet", () => {
  const ladder = (x: number, y: number, z: number): boolean => x === 5 && (z === 4 || z === 5);
  const path = [node(5, 64, 3, false), node(5, 64, 4, true), node(5, 64, 5, true)];
  assert.deepEqual(holdClimbableNodesAtFeet(path, 64, ladder), ["5,64,4", "5,64,5"]);
  assert.deepEqual([path[1]!.x, path[1]!.y, path[1]!.z], [5.5, 64, 4.5]);
  // A node in an ordinary cell is the library's, untouched.
  assert.deepEqual([path[0]!.x, path[0]!.y], [5.7, 64]);
});

test("a ladder node the body climbs into, or climbs on from, keeps the library's aim", () => {
  const ladder = (): boolean => true;
  // Climbing into it: the node before is a cell lower.
  const into = [node(5, 65, 4, true), node(5, 65, 5, true)];
  assert.deepEqual(holdClimbableNodesAtFeet(into, 64, ladder), ["5,65,5"]);
  assert.equal(into[0]!.y, 66);
  // Climbing on from it: the node after is higher.
  const onFrom = [node(5, 64, 4, true), node(5, 65, 4, true)];
  assert.deepEqual(holdClimbableNodesAtFeet(onFrom, 64, ladder), []);
  assert.equal(onFrom[0]!.y, 65);
});

test("the hold stops where the library's own processing stops", () => {
  const ladder = (): boolean => true;
  const breaking: PathNode = { ...node(5, 64, 4, true), toBreak: [{}] };
  const path = [breaking, node(5, 64, 5, true)];
  assert.deepEqual(holdClimbableNodesAtFeet(path, 64, ladder), []);
});
