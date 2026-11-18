// A proven jump is driven from where the compiler's edge says it starts.
//
// The real mineflayer-pathfinder over the real client physics (`sim-bot.ts`),
// over two shapes the festival town's branch worlds failed on:
//
// * **A launch on a lily pad.** The compiler names the cell a body stands in by
//   the block under it (`feet_of`): a body on a lily pad at y=64 stands in cell
//   65 with its feet at 64.09375, almost a block under the cell's floor. The
//   drive required the feet within 0.6 of the cell's floor, so it refused a body
//   standing exactly where the edge starts ("is not on its run-up", the bot at
//   [177.7, 74.1, 370.5] for the launch [177, 75, 370]).
// * **A launch at the foot of a stair.** A walk ends when the pathfinder counts
//   the body inside the goal's cell, which it does while the body is still
//   dropping off the stair into it; and a hurt during the run (the server sets
//   the body's motion, and the client then sees no floor under feet that have
//   not moved) read as the body leaving the floor before the press ("left the
//   floor before a tick to press jump on", the bot at [199.8, 92.0, 483.5]).
//   The compiler flies the run from rest on the floor: the drive starts it
//   there, and runs it again from there when the server moved the body.

import { test } from "node:test";
import assert from "node:assert/strict";
import { driveJump, feetInCell, MAX_RERUNS, onRunUp, type JumpBot } from "../src/executor/jump.ts";
import type { Jump } from "../src/waypoints.ts";
import { cellKey, fmtPos, simBot, simJump, simWalk, type Cells, type SimBot } from "./sim-bot.ts";

/**
 * A pond: water at y=63 over x ∈ 0..4, z ∈ 0..2, on dirt at y=62; lily pads at
 * y=64 along z=0; sand at y=63 from z=3 to z=6, the bank. The festival town's
 * cat-dish shore, cut to its shape.
 */
function pond(): Cells {
  const cells: Cells = new Map();
  for (let x = 0; x <= 4; x++) {
    for (let z = 0; z <= 6; z++) {
      cells.set(cellKey(x, 62, z), "minecraft:dirt");
      cells.set(cellKey(x, 63, z), z <= 2 ? "minecraft:water[level=0]" : "minecraft:sand");
    }
    cells.set(cellKey(x, 64, 0), "minecraft:lily_pad");
  }
  return cells;
}

/** From the lily pad at [2, 64, 0] to the bank's first cell, sprinted, run-up 0. */
const LILY: Jump = { from: [2, 65, 0], to: [2, 64, 3], sprint: true, runUp: 0 };

/**
 * A stair down into a one-cell launch: a stone-brick stair at [0, 64, 0]
 * (facing west, its top at 65), the launch's polished andesite at [1, 63, 0],
 * a gap at x=2 open down to a floor at y=40, the landing's andesite at
 * [3, 63, 0]; walls at z=-1 and z=1 up to y=67. The festival town's lamp-tower
 * stair, cut to its shape.
 */
function stairFoot(): Cells {
  const cells: Cells = new Map();
  cells.set(cellKey(-1, 64, 0), "minecraft:stone_bricks");
  cells.set(cellKey(-1, 63, 0), "minecraft:stone_bricks");
  cells.set(cellKey(0, 64, 0), "minecraft:stone_brick_stairs[facing=west,half=bottom,shape=straight,waterlogged=false]");
  cells.set(cellKey(0, 63, 0), "minecraft:stone_bricks");
  cells.set(cellKey(1, 63, 0), "minecraft:polished_andesite");
  cells.set(cellKey(3, 63, 0), "minecraft:polished_andesite");
  cells.set(cellKey(4, 63, 0), "minecraft:polished_andesite");
  for (let x = -1; x <= 4; x++) {
    cells.set(cellKey(x, 40, 0), "minecraft:stone");
    for (let y = 63; y <= 67; y++) {
      cells.set(cellKey(x, y, -1), "minecraft:white_concrete");
      cells.set(cellKey(x, y, 1), "minecraft:white_concrete");
    }
  }
  return cells;
}

/** From the stair's foot across the one-cell gap, walking, run-up 0. */
const STAIR: Jump = { from: [1, 64, 0], to: [3, 64, 0], sprint: false, runUp: 0 };

const at = (bot: SimBot): string => {
  const p = bot.entity.position;
  return fmtPos([p.x, p.y, p.z]);
};

test("feet in a cell's row lie above the floor under it and at most half a block over its own", () => {
  assert.equal(feetInCell(64.09375, 65), true, "a lily pad's");
  assert.equal(feetInCell(64.0625, 65), true, "a carpet's");
  assert.equal(feetInCell(64.5, 65), true, "a bottom slab's");
  assert.equal(feetInCell(65, 65), true, "a full block's");
  assert.equal(feetInCell(65.5, 65), true, "a fence's");
  assert.equal(feetInCell(64, 65), false, "a full block one row lower");
  assert.equal(feetInCell(65.6, 65), false, "above a fence's top");
  assert.equal(feetInCell(66, 65), false, "a row higher");
});

test("a body on the lily pad stands on the run-up of the jump that starts there", () => {
  const bot = simBot(pond(), [2.5, 64.09375, 0.5]);
  for (let i = 0; i < 3; i++) bot.tick();
  assert.ok(bot.entity.onGround, `the body is not on the lily pad: ${at(bot)}`);
  assert.equal(onRunUp(bot.entity.position, LILY), true, `on the lily pad at ${at(bot)}`);
});

test("a sprinted jump off a lily pad is driven from the pad and lands on the bank", async () => {
  const bot = simBot(pond(), [2.5, 64.09375, 0.5]);
  for (let i = 0; i < 3; i++) bot.tick();
  const r = await simJump(bot, LILY);
  assert.ok(r.landed, `${r.why} at ${at(bot)}`);
  assert.ok(Math.abs(bot.entity.position.y - 64) < 0.01, `came down at ${at(bot)}`);
});

test("walked to the foot of the stair, the body is driven once it is down, and makes the jump", async () => {
  const bot = simBot(stairFoot(), [0.5, 65, 0.5]);
  for (let i = 0; i < 3; i++) bot.tick();
  const w = simWalk(bot, STAIR.from, 0, 400);
  assert.ok(w.reached, `the walk did not arrive: ${fmtPos(w.at)}`);
  const r = await simJump(bot, STAIR);
  assert.ok(r.landed, `${r.why} at ${at(bot)}`);
  assert.ok(Math.abs(bot.entity.position.y - 64) < 0.01 && bot.entity.position.x >= 3, `came down at ${at(bot)}`);
});

test("a body still dropping into the launch is driven once it is down", async () => {
  // Over the launch's centre, 0.8 above its floor, falling: where the walk off
  // the town's stair handed the body over.
  const bot = simBot(stairFoot(), [1.5, 64.8, 0.5]);
  bot.entity.onGround = false;
  const r = await simJump(bot, STAIR);
  assert.ok(r.landed, `${r.why} at ${at(bot)}`);
});

/**
 * Set the body's motion to nothing on the `n`th tick of the run, as a hurt's
 * motion packet does: the client then moves the body by nothing that tick, finds
 * no floor under it, and reports it off the ground with its feet unmoved.
 */
function hurtOnRunTick(bot: SimBot, n: number, times = 1): () => number {
  let runTicks = 0;
  let hurts = 0;
  bot.on("physicsTick", () => {
    const running = bot.controlState["forward"] && !bot.controlState["sneak"] && !bot.controlState["jump"];
    runTicks = running ? runTicks + 1 : 0;
    if (runTicks === n && hurts < times) {
      hurts += 1;
      const v = bot.entity.velocity as { x: number; y: number; z: number };
      v.x = 0;
      v.y = 0;
      v.z = 0;
    }
  });
  return () => hurts;
}

test("a run the server moved the body on before the press is run again from the start", async () => {
  const bot = simBot(stairFoot(), [1.5, 64, 0.5]);
  for (let i = 0; i < 3; i++) bot.tick();
  const hurts = hurtOnRunTick(bot, 2);
  let landing: Awaited<ReturnType<typeof driveJump>> | undefined;
  let why = "";
  let settled = false;
  driveJump(bot as unknown as JumpBot, STAIR).then(
    (l) => {
      landing = l;
      settled = true;
    },
    (e: unknown) => {
      why = (e as Error).message;
      settled = true;
    },
  );
  for (let t = 0; t < 600 && !settled; t++) {
    bot.tick();
    await new Promise((r) => setImmediate(r));
  }
  assert.equal(hurts(), 1, "the hurt never landed on the run");
  assert.ok(landing, `${why} at ${at(bot)}`);
  assert.equal(landing.reruns, 1);
  assert.ok(Math.abs(bot.entity.position.y - 64) < 0.01 && bot.entity.position.x >= 3, `came down at ${at(bot)}`);
});

test("a run the server keeps moving the body on is refused, naming it", async () => {
  const bot = simBot(stairFoot(), [1.5, 64, 0.5]);
  for (let i = 0; i < 3; i++) bot.tick();
  hurtOnRunTick(bot, 2, MAX_RERUNS + 1);
  const r = await simJump(bot, STAIR);
  assert.equal(r.landed, false);
  assert.match(r.why, new RegExp(`was moved off its run by the server ${MAX_RERUNS + 1} times`));
});
