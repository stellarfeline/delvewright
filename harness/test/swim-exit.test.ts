// A body in water swims up and climbs out where the route proof does
// (spec-0134 §10).
//
// The real mineflayer-pathfinder over the real client physics (`sim-bot.ts`),
// configured as the walk configures a leg whose liquid is admitted. The defect
// this guards: the library's search has no move up out of a node in liquid,
// and measures a jump-up from the block under the node, which in water is
// water, so a ledge flush with the surface reads two blocks high. The cheerful
// town's festival day stopped so: a boat's rider, left in the cove one cell
// short of a sand shoal whose top is level with the water, read "No path to
// the goal!" where the proof's leg steps out.

import { test } from "node:test";
import assert from "node:assert/strict";
import {
  configureLeg,
  installSwimMoves,
  SWIM_CLIMB_OUT_CELLS,
  swimFor,
  swimMoves,
  type SwimCell,
  type SwimMovements,
} from "../src/movement.ts";
import { cellKey, fmtPos, simBot, simWalk, type Cells, type SimBot } from "./sim-bot.ts";

const WATER = "minecraft:water[level=0]";

/**
 * The town's cove at [113, *, 478..479] in miniature: a stone bed at y 57,
 * water from y 58 to y 62 over z ≤ `shore - 1`, and from z = `shore` a bank of
 * sand from y 58 up to `top` — a shoal flush with the surface at `top` = 62.
 * `fluid` fills the water columns (lava for the lava case).
 */
function cove(top = 62, shore = 3, fluid = WATER): Cells {
  const c: Cells = new Map();
  for (let x = -4; x <= 4; x++) {
    for (let z = -4; z <= 7; z++) {
      c.set(cellKey(x, 57, z), "minecraft:stone");
      if (z >= shore) {
        for (let y = 58; y <= top; y++) c.set(cellKey(x, y, z), "minecraft:sand");
      } else {
        for (let y = 58; y <= 62; y++) c.set(cellKey(x, y, z), fluid);
      }
    }
  }
  return c;
}

/** A bot whose leg is configured as the walk configures it (`walkLeg`). */
function legBot(cells: Cells, start: readonly [number, number, number], water: boolean, fixed = true): SimBot {
  const bot = simBot(cells, start);
  const pf = bot.pathfinder as unknown as { movements: SwimMovements & Record<string, unknown> };
  const m = pf.movements;
  configureLeg(bot as never, m as never, false);
  if (fixed) installSwimMoves(m);
  swimFor(m as never, water);
  bot.pathfinder.setMovements(m);
  return bot;
}

/** Where the town's rider was left: two below the surface, one cell short of the shoal. */
const SUBMERGED: readonly [number, number, number] = [0.5, 61.0, 2.5];
const ON_THE_SHOAL: readonly [number, number, number] = [0, 63, 3];

test("without the swim moves, a body in water reads no path onto a shoal flush with the surface (the defect, reproduced)", () => {
  for (const start of [SUBMERGED, [0.5, 62.2, 2.5] as const]) {
    const r = simWalk(legBot(cove(), start, true, false), ON_THE_SHOAL, 0, 300);
    assert.equal(r.reached, false, `the library alone climbed out from ${fmtPos(start)} to ${fmtPos(r.at)}`);
  }
});

test("a body in water swims up and steps out onto a shoal flush with the surface", () => {
  // From under the surface, at it, and from the bed.
  for (const start of [SUBMERGED, [0.5, 62.2, 2.5] as const, [0.5, 58, 0.5] as const]) {
    const r = simWalk(legBot(cove(), start, true), ON_THE_SHOAL, 0, 400);
    assert.ok(r.reached, `from ${fmtPos(start)}: stopped at ${fmtPos(r.at)}`);
    assert.ok(Math.abs(r.at[1] - 63) < 0.1, `from ${fmtPos(start)}: not on the shoal: ${fmtPos(r.at)}`);
  }
});

test("a body under the surface rises and walks out onto a shore one lower", () => {
  const r = simWalk(legBot(cove(61), SUBMERGED, true), [0, 62, 3], 0, 400);
  assert.ok(r.reached, `stopped at ${fmtPos(r.at)}`);
});

test("a bank two over the swim cell is past the climb-out band, as the proof holds it", () => {
  assert.equal(SWIM_CLIMB_OUT_CELLS, 1);
  const r = simWalk(legBot(cove(63), SUBMERGED, true), [0, 64, 3], 0, 300);
  assert.equal(r.reached, false, `climbed a two-high bank to ${fmtPos(r.at)}`);
});

test("a leg whose liquid is not admitted takes no swim move", () => {
  const r = simWalk(legBot(cove(), SUBMERGED, false), ON_THE_SHOAL, 0, 300);
  assert.equal(r.reached, false, `a dry leg swam out to ${fmtPos(r.at)}`);
});

// --- the moves themselves -------------------------------------------------

/** `Movements.getBlock` over a cell map, as the library answers it. */
function fakeMovements(cells: Map<string, string>): SwimMovements {
  const cell = (name: string): SwimCell => {
    const solid = name === "stone" || name === "sand";
    return { name, boundingBox: solid ? "block" : "empty", physical: solid, safe: !solid && name !== "lava" };
  };
  return {
    liquidCost: 1,
    getNeighbors: () => [],
    getBlock: (n, dx, dy, dz) => {
      const b = cell(cells.get(cellKey(n.x + dx, n.y + dy, n.z + dz)) ?? "air");
      return { ...b, position: { x: n.x + dx, y: n.y + dy, z: n.z + dz } };
    },
    exclusionStep: () => 0,
  };
}

function column(cells: Map<string, string>, x: number, z: number, from: number, to: number, name: string): void {
  for (let y = from; y <= to; y++) cells.set(cellKey(x, y, z), name);
}

test("the swim moves: up to the swim cell over it, and out onto a flush ledge; never out of lava", () => {
  const cells = new Map<string, string>();
  column(cells, 0, 0, 58, 62, "water");
  column(cells, 0, 1, 58, 62, "sand");
  const node = { x: 0, y: 61, z: 0, remainingBlocks: 0 };
  const m = fakeMovements(cells);
  const hashes = (ns: readonly { x: number; y: number; z: number }[]): string[] =>
    ns.map((n) => `${n.x},${n.y},${n.z}`).sort();
  assert.deepEqual(hashes(swimMoves(m, node, 1)), ["0,62,0"]);
  assert.deepEqual(hashes(swimMoves(m, { ...node, y: 62 }, 1)), ["0,63,1"]);
  column(cells, 0, 0, 58, 62, "lava");
  assert.deepEqual(swimMoves(m, { ...node, y: 62 }, 1), []);
});

test("installed swim moves join the library's own only while liquid is admitted", () => {
  const cells = new Map<string, string>();
  column(cells, 0, 0, 58, 62, "water");
  column(cells, 0, 1, 58, 62, "sand");
  const m = fakeMovements(cells);
  installSwimMoves(m);
  installSwimMoves(m);
  const node = { x: 0, y: 62, z: 0, remainingBlocks: 0 };
  assert.equal(m.getNeighbors(node).length, 1, "installed once, the climb out added");
  m.liquidCost = Number.POSITIVE_INFINITY;
  assert.equal(m.getNeighbors(node).length, 0);
});
