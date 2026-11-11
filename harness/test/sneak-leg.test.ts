// A `sneak` leg is walked crouched on every tick (`configureLeg`).
//
// The real mineflayer-pathfinder over the real client physics (`sim-bot.ts`).
// The defect this guards: the library clears every control state on
// `setMovements`, on every `setGoal` and on arrival, and a leg calls the first
// two on every hop, so a crouch set once at the leg's start was gone before the
// body took a step. A declared stealth leg was walked upright on every tick —
// against a warden, the walk its hearing is built to catch.

import { test } from "node:test";
import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { configureLeg, hopStandsUp, type LegMovements } from "../src/movement.ts";
import { cellKey, fmtPos, simBot, simWalk, type Cells, type SimBot } from "./sim-bot.ts";

const requireCjs = createRequire(import.meta.url);
const { Movements } = requireCjs("mineflayer-pathfinder") as {
  Movements: new (bot: unknown) => LegMovements;
};

/** A stone floor at y=63 over x ∈ 0..20, z ∈ 0..4; from x=11 on, one block lower when `step`. */
function lane(step: boolean): Cells {
  const cells: Cells = new Map();
  for (let x = 0; x <= 20; x++) {
    for (let z = 0; z <= 4; z++) cells.set(cellKey(x, step && x > 10 ? 62 : 63, z), "minecraft:stone");
  }
  return cells;
}

/**
 * Walk one leg the way `walkLeg` drives it — `configureLeg`, then for each hop
 * `setMovements` and a goal — counting the ticks the body is not crouched.
 * `standing` is `yieldCrouch`'s answer, decided per hop as the leg decides it.
 */
function walkSneakLeg(
  bot: SimBot,
  hops: ReadonlyArray<readonly [number, number, number]>,
  standing: (hop: readonly [number, number, number]) => boolean = () => false,
): { upright: number; ticks: number; reached: boolean; at: readonly number[] } {
  const movements = new Movements(bot);
  let yielded = false;
  let restore = (): void => {};
  bot.run(() => {
    restore = configureLeg(bot as never, movements, true, () => yielded);
  });
  let upright = 0;
  let ticks = 0;
  const count = (): void => {
    ticks += 1;
    if (!bot.controlState["sneak"] && !yielded) upright += 1;
  };
  let reached = true;
  try {
    for (const hop of hops) {
      bot.run(() => bot.pathfinder.setMovements(movements));
      yielded = standing(hop);
      bot.on("physicsTick", count);
      const r = simWalk(bot, hop, 0, 900);
      bot.off("physicsTick", count);
      yielded = false;
      reached &&= r.reached;
    }
  } finally {
    restore();
  }
  const p = bot.entity.position;
  return { upright, ticks, reached, at: [p.x, p.y, p.z] };
}

test("a sneak leg is crouched on every tick of every hop", () => {
  const bot = simBot(lane(false), [1.5, 64, 2.5]);
  const r = walkSneakLeg(bot, [
    [9, 64, 2],
    [18, 64, 2],
  ]);
  assert.ok(r.reached, `stopped at ${fmtPos(r.at)}`);
  assert.ok(r.ticks > 0);
  assert.equal(r.upright, 0, `upright on ${r.upright} of ${r.ticks} ticks`);
});

test("the leg's end releases the crouch and stops holding it", () => {
  const bot = simBot(lane(false), [1.5, 64, 2.5]);
  walkSneakLeg(bot, [[6, 64, 2]]);
  assert.equal(bot.controlState["sneak"], false, "the crouch outlived its leg");
  bot.tick();
  assert.equal(bot.controlState["sneak"], false, "the hold still runs after its leg ended");
});

test("a crouched body stops at a step down; the leg stands for that hop and arrives", () => {
  // Vanilla's edge back-off, reproduced by the client physics: a crouched body
  // will not walk off a one-block step. Held crouched through it, the hop stalls.
  const held = walkSneakLeg(simBot(lane(true), [1.5, 64, 2.5]), [[18, 63, 2]]);
  assert.equal(held.reached, false, `a crouched body stepped down to ${fmtPos(held.at)}`);
  // The leg's rule stands the body up for the hop down, and only that hop.
  const feet = 64;
  const ruled = walkSneakLeg(
    simBot(lane(true), [1.5, 64, 2.5]),
    [
      [9, 64, 2],
      [12, 63, 2],
      [18, 63, 2],
    ],
    (hop) => hopStandsUp({ y: hop[1] }, hop[0] === 12 ? feet : 63),
  );
  assert.ok(ruled.reached, `stopped at ${fmtPos(ruled.at)}`);
  assert.equal(ruled.upright, 0, `upright on ${ruled.upright} of ${ruled.ticks} crouched ticks`);
});

test("a hop stands up exactly for a step down or a jump, never for a climb", () => {
  assert.equal(hopStandsUp({ y: 64 }, 64), false, "level");
  assert.equal(hopStandsUp({ y: 65 }, 64), false, "up a step");
  assert.equal(hopStandsUp({ y: 63 }, 64), true, "down a step");
  assert.equal(hopStandsUp({ y: 63 }, 64.5), true, "down from a slab");
  assert.equal(hopStandsUp({ y: 64 }, 64.5), false, "off a slab onto the floor below it");
  assert.equal(hopStandsUp({ y: 64, jump: {} }, 64), true, "a declared jump");
  assert.equal(hopStandsUp({ y: 50, climb: {} }, 64), false, "a climb is the climb's to decide");
});
