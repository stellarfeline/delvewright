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
import { configureLeg, hopMovements, type LegMovements } from "../src/movement.ts";
import { type GoalSpec, walkGoals } from "../src/waypoints.ts";
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
 * `hopMovements`, `setMovements`, the crouch yielded for a stand-up, and a goal —
 * counting the ticks the body is not crouched outside a stand-up.
 */
function walkSneakLeg(
  bot: SimBot,
  hops: ReadonlyArray<readonly [number, number, number] | GoalSpec>,
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
    for (const h of hops) {
      const hop: GoalSpec = Array.isArray(h) ? { x: h[0], y: h[1], z: h[2], range: 0 } : (h as GoalSpec);
      hopMovements(movements, true, hop);
      bot.run(() => bot.pathfinder.setMovements(movements));
      yielded = hop.standsUp === true;
      bot.on("physicsTick", count);
      const r = simWalk(bot, [hop.x, hop.y, hop.z], 0, 900);
      bot.off("physicsTick", count);
      hopMovements(movements, true, {});
      yielded = false;
      reached &&= r.reached;
      if (!r.reached) break;
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
  // The compiler's `stand_ups` names the hop down; the leg stands for it, and
  // only for it.
  const goals = walkGoals(
    [
      [9, 64, 2],
      [10, 64, 2],
      [11, 63, 2],
    ],
    [18, 63, 2],
    0,
    [],
    [],
    [{ from: [10, 64, 2], to: [11, 63, 2], why: "step-down" }],
  );
  assert.deepEqual(
    goals.map((g) => g.standsUp === true),
    [false, false, true, false],
  );
  const ruled = walkSneakLeg(
    simBot(lane(true), [1.5, 64, 2.5]),
    goals.map((g) => ({ ...g, range: 0 })),
  );
  assert.ok(ruled.reached, `stopped at ${fmtPos(ruled.at)}`);
  assert.equal(ruled.upright, 0, `upright on ${ruled.upright} of ${ruled.ticks} crouched ticks`);
});

/** A stone floor at y=63 over x ∈ 0..20, z ∈ 0..4, open over x ∈ 10..12: a three-wide gap. */
function gapLane(): Cells {
  const cells: Cells = new Map();
  for (let x = 0; x <= 20; x++) {
    if (x >= 10 && x <= 12) continue;
    for (let z = 0; z <= 4; z++) cells.set(cellKey(x, 63, z), "minecraft:stone");
  }
  return cells;
}

test("a sneak leg makes its proven sprint jump standing, at a sprint, and lands", () => {
  // The gallery's broken plank on a walk declared stealth: a three-wide gap,
  // declared `sprint`, the compiler's `stand_ups` naming the jump.
  const jump = { from: [9, 64, 2] as const, to: [13, 64, 2] as const, sprint: true, runUp: 1 };
  const goals = walkGoals(
    [
      [5, 64, 2],
      [9, 64, 2],
      [13, 64, 2],
    ],
    [18, 64, 2],
    0,
    [],
    [jump],
    [{ from: jump.from, to: jump.to, why: "jump" }],
  );
  const r = walkSneakLeg(
    simBot(gapLane(), [1.5, 64, 2.5]),
    goals.map((g) => ({ ...g, range: 0 })),
  );
  assert.ok(r.reached, `stopped at ${fmtPos(r.at)}`);
  assert.equal(r.upright, 0, `upright on ${r.upright} of ${r.ticks} crouched ticks`);
  const flags = goals.map((g) => hopMovements({ allowParkour: false, allowSprinting: false }, true, g));
  assert.deepEqual(
    flags.map((f) => f.sprint),
    [false, false, true, false],
    "a sneak leg sprints for its sprint jump's hop and no other",
  );
});
