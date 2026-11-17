// A proven jump is driven as the compiler flew it (`driveJump`).
//
// The real client physics (`sim-bot.ts`), one game tick per drive tick. The
// defect this guards: a walking jump the compiler proved under the bot's own
// physics — three cells forward and one down, the cat's first dish in the
// festival town — was refused before the walk began, because the only leap the
// harness made was mineflayer-pathfinder 2.4.5's parkour move, which lands two
// cells forward at most with sprinting off. The body makes the jump; the
// harness lacked the move.

import { test } from "node:test";
import assert from "node:assert/strict";
import { boxOverColumn, pressNow, runUpStart } from "../src/executor/jump.ts";
import type { Jump } from "../src/waypoints.ts";
import { cellKey, fmtPos, simBot, simJump, type Cells } from "./sim-bot.ts";

/**
 * A stone run-up at y=63 over x ∈ 0..3, z ∈ 0..4; a landing at `landY` from
 * x = `landX` on; the columns between open down to a floor at y=40.
 */
function ledge(landX: number, landY: number): Cells {
  const cells: Cells = new Map();
  for (let z = 0; z <= 4; z++) {
    for (let x = 0; x <= 3; x++) cells.set(cellKey(x, 63, z), "minecraft:stone");
    for (let x = landX; x <= landX + 5; x++) cells.set(cellKey(x, landY, z), "minecraft:stone");
    for (let x = 0; x <= landX + 5; x++) cells.set(cellKey(x, 40, z), "minecraft:stone");
  }
  return cells;
}

test("a walking jump three forward and one down is driven and lands", async () => {
  // The festival town's: [177,75,370] → [177,74,373], walking.
  const jump: Jump = { from: [3, 64, 2], to: [6, 63, 2], sprint: false, runUp: 2 };
  const bot = simBot(ledge(6, 62), [3.5, 64, 2.5]);
  let sprinted = 0;
  bot.on("physicsTick", () => {
    if (bot.controlState["sprint"]) sprinted += 1;
  });
  const r = await simJump(bot, jump);
  const p = bot.entity.position;
  assert.ok(r.landed, `${r.why} at ${fmtPos([p.x, p.y, p.z])}`);
  assert.ok(Math.abs(p.y - 63) < 0.01, `came down at ${fmtPos([p.x, p.y, p.z])}`);
  assert.equal(sprinted, 0, "a walking jump was sprinted");
});

test("a sprinted jump four forward on the flat is driven and lands", async () => {
  const jump: Jump = { from: [3, 64, 2], to: [7, 64, 2], sprint: true, runUp: 3 };
  const r = await simJump(simBot(ledge(7, 63), [3.5, 64, 2.5]), jump);
  assert.ok(r.landed, r.why);
});

test("a jump no press makes is refused, naming where the body came down", async () => {
  // Five forward, walking: beyond any walking jump.
  const jump: Jump = { from: [3, 64, 2], to: [8, 64, 2], sprint: false, runUp: 3 };
  const bot = simBot(ledge(8, 63), [3.5, 64, 2.5]);
  const r = await simJump(bot, jump);
  assert.equal(r.landed, false);
  assert.match(r.why, /came down off the landing's floor/);
  assert.ok(bot.entity.position.y < 50, "the body did not fall into the gap");
});

test("the run starts its run-up behind the launch and presses on its last ticks over it", () => {
  assert.deepEqual(runUpStart({ from: [3, 64, 2], to: [6, 63, 2], sprint: false, runUp: 2 }), [1, 64, 2]);
  assert.deepEqual(runUpStart({ from: [0, 64, 0], to: [0, 64, -4], sprint: true, runUp: 3 }), [0, 64, 3]);
  // The box is over the column while its edge is: a body centred 0.29 past the cell.
  assert.ok(boxOverColumn(4.29, 2.5, [3, 64, 2]));
  assert.ok(!boxOverColumn(4.31, 2.5, [3, 64, 2]));
  // At walking speed (0.216 a tick) the press comes within two ticks of the edge.
  const v = { x: 0.216, z: 0 };
  assert.equal(pressNow({ x: 3.8, z: 2.5 }, v, true, [3, 64, 2]), false, "three ticks out");
  assert.equal(pressNow({ x: 3.9, z: 2.5 }, v, true, [3, 64, 2]), true, "two ticks out");
  assert.equal(pressNow({ x: 3.9, z: 2.5 }, v, false, [3, 64, 2]), false, "in the air");
});
