// A downward climb takes hold before it slides (`climbHop`).
//
// The real client physics (`sim-bot.ts`), one game tick per climb tick. The
// defect this guards: the descent aligned the body within 0.25 of the column's
// centre and slid "while not on ground" — and a body entered from above whose
// 0.6-wide hitbox still overlapped the floor beside the column stood on that
// edge, so the slide ended at once and the climb "let go" at the top. The warden
// maze's tower: "let go at [47.5, 87.0, 48.5]".

import { test } from "node:test";
import assert from "node:assert/strict";
import { MineflayerExecutor } from "../src/executor.ts";
import type { Bot } from "mineflayer";
import type { Climb, GoalSpec } from "../src/waypoints.ts";
import { cellKey, fmtPos, simBot, type Cells } from "./sim-bot.ts";

/**
 * A deck of stone at y=86 over x,z ∈ 2..9 with a ladder column at (5, 64..86, 5)
 * hung on a log column to its south, a floor at y=63 below — the tower in
 * miniature. The body stands on the deck east of the column's top.
 */
function tower(): Cells {
  const cells: Cells = new Map();
  for (let x = 0; x <= 10; x++) for (let z = 0; z <= 10; z++) cells.set(cellKey(x, 63, z), "minecraft:stone");
  for (let x = 2; x <= 9; x++) for (let z = 2; z <= 9; z++) cells.set(cellKey(x, 86, z), "minecraft:stone");
  for (let y = 64; y <= 86; y++) {
    cells.set(cellKey(5, y, 6), "minecraft:oak_log[axis=y]");
    cells.set(cellKey(5, y, 5), "minecraft:ladder[facing=north,waterlogged=false]");
  }
  return cells;
}

const CLIMB: Climb = {
  from: [6, 87, 5],
  to: [5, 64, 4],
  bottom: [5, 64, 5],
  top: [5, 86, 5],
  block: "minecraft:ladder[facing=north,waterlogged=false]",
  facing: "north",
};

test("a downward climb entered from the deck beside the column lets go at its foot", async () => {
  const sim = simBot(tower(), [6.5, 87, 5.5]);
  const executor = new MineflayerExecutor({
    host: "127.0.0.1",
    port: 25565,
    username: "delve-bot",
    version: "1.21.11",
    auth: "offline",
  });
  (executor as unknown as { bot: Bot }).bot = sim as unknown as Bot;
  const clock = setInterval(() => sim.tick(), 50);
  try {
    const spec: GoalSpec = { x: 5, y: 64, z: 4, range: 1, climb: CLIMB };
    await executor.climbHop(spec, "the tower");
  } finally {
    clearInterval(clock);
  }
  const p = sim.entity.position;
  assert.ok(p.y < 64.5, `the climb let go at ${fmtPos([p.x, p.y, p.z])}, not at the foot`);
});
