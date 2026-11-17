// A proven jump starts where the proof's run starts (`jumpHop`).
//
// The real mineflayer-pathfinder over the real client physics (`sim-bot.ts`),
// over the gallery's own cells round the broken plank: the at-load world of
// the gallery built at engine revision bfdc6a824, cut to x 4..26, y 66..72,
// z 12..28 (`fixtures/gallery-jetty-cells.json`). The jump is the gallery's
// `jump/the-broken-plank`: [16, 69, 20] → [12, 69, 20], sprinted, run-up 1,
// from [17, 69, 20] — the east cell of a plank one wide, two courses over the
// hall's floor.
//
// The defect this guards: the drive crept, crouched, in a straight line from
// wherever the bot stood to the run-up's start. On the gallery bot a story
// spider's blows put the bot on the hall's floor beside the plank, and the
// creep pressed it against the plank's face until its budget ran out ("did not
// land within 180 ticks (stuck in its creep)", at [17.5, 68.0, 21.3]). A bot
// off the run-up is walked to its start by the pathfinder first; the drive
// refuses a body off the run-up at once.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { MineflayerExecutor } from "../src/executor.ts";
import type { Bot } from "mineflayer";
import { onRunUp, driveJump, type JumpBot } from "../src/executor/jump.ts";
import type { GoalSpec, Jump } from "../src/waypoints.ts";
import { fmtPos, simBot, simJump, simWalk, type Cells, type SimBot } from "./sim-bot.ts";

const raw = JSON.parse(
  readFileSync(new URL("./fixtures/gallery-jetty-cells.json", import.meta.url), "utf8"),
) as Record<string, string>;
const jetty = (): Cells => new Map(Object.entries(raw));

const PLANK: Jump = { from: [16, 69, 20], to: [12, 69, 20], sprint: true, runUp: 1 };

/** The hall's floor beside the plank, where the blows left the bot. */
const OFF: [number, number, number] = [17.5, 67, 21.5];

/** An executor over `sim`, its staged walk the pathfinder's own, ticked. */
function executorOver(sim: SimBot): MineflayerExecutor {
  const executor = new MineflayerExecutor({
    host: "127.0.0.1",
    port: 25565,
    username: "delve-bot",
    version: "1.21.11",
    auth: "offline",
  });
  (executor as unknown as { bot: Bot }).bot = sim as unknown as Bot;
  (executor as unknown as { gotoStaged: (s: GoalSpec, l: string) => Promise<void> }).gotoStaged = (
    spec: GoalSpec,
    label: string,
  ) => {
    const r = simWalk(sim, [spec.x, spec.y, spec.z], spec.range, 3000);
    return r.reached ? Promise.resolve() : Promise.reject(new Error(`${label}: walk stopped at ${fmtPos(r.at)}`));
  };
  return executor;
}

test("on the plank, the gallery's sprint jump lands on the ferry deck", async () => {
  const r = await simJump(simBot(jetty(), [16.5, 69, 20.5]), PLANK);
  assert.ok(r.landed, r.why);
});

test("knocked off the plank, the bot is walked back to the run-up's start and makes the jump", async () => {
  const sim = simBot(jetty(), OFF);
  for (let i = 0; i < 5; i++) sim.tick();
  assert.equal(onRunUp(sim.entity.position, PLANK), false);
  assert.equal(onRunUp({ x: 16.5, y: 69, z: 20.5 }, PLANK), true, "the launch is on its run-up");
  const executor = executorOver(sim);
  const clock = setInterval(() => sim.tick(), 1);
  try {
    const spec: GoalSpec = { x: 12, y: 69, z: 20, range: 1, jump: PLANK };
    await executor.jumpHop(spec, "the broken plank");
  } finally {
    clearInterval(clock);
  }
  const p = sim.entity.position;
  assert.ok(p.x < 13 && Math.abs(p.y - 69) < 0.01, `came down at ${fmtPos([p.x, p.y, p.z])}`);
});

test("the drive refuses a start off the run-up at once, naming it", async () => {
  const sim = simBot(jetty(), OFF);
  let ticks = 0;
  sim.on("physicsTick", () => {
    ticks += 1;
  });
  const r = await simJump(sim, PLANK);
  assert.equal(r.landed, false);
  assert.match(r.why, /is not on its run-up \[17, 69, 20\]\.\.\[16, 69, 20\]/);
  assert.ok(ticks <= 2, `the drive ran ${ticks} tick(s) before refusing`);
  // The same refusal through the drive's own entry point.
  await assert.rejects(
    () => {
      const p = driveJump(sim as unknown as JumpBot, PLANK);
      sim.tick();
      return p;
    },
    /is not on its run-up/,
  );
});
