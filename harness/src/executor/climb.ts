// MineflayerExecutor: a climb the compiler proved (spec-0099), driven raw.
//
// A leg whose route climbs carries the climb in its waypoint artifact
// (`legs[].climbs[]`): where the body takes hold, where it lets go, the column it
// holds in and a ladder's facing. The hop between the two ends is driven here,
// with the bot's own controls, and never handed to the pathfinder — which plans a
// ladder (`Movements.climbables` holds `ladder` alone in mineflayer-pathfinder
// 2.4.5) but steers a straight-up hop at an undefined yaw and takes `|dy| < 1` as
// arrival, and has no vine at all.
//
// Navigation, not game logic: the route is the compiler's, and every control
// below is one a player presses on that route. What the controls do is vanilla's
// (spec-0099 §2): a body on a climbable that pushes into something or holds jump
// is given 0.2 blocks/tick upward, one that does neither slides down at most 0.15,
// and the bot's client physics (prismarine-physics) treats `ladder` and `vine` as
// climbable — and nothing else, which is why a weeping, twisting or cave vine is
// refused here rather than attempted.

import type { MineflayerExecutor } from "../executor.ts";
import { climbDirection, supportCell, type Climb, type GoalSpec } from "../waypoints.ts";
import { delay, fmt } from "./connection.ts";

/** Poll interval while a climb is driven: one game tick. */
const CLIMB_TICK_MS = 50;

/** How close (blocks, horizontally) the bot must be to a column's centre before
 * it takes hold — inside the climbable cell, clear of its panel. */
const COLUMN_ALIGN = 0.25;

/** How close (blocks, horizontally) the bot must be to a let-go cell's centre to
 * have stepped onto it. */
const LET_GO_REACH = 0.35;

/**
 * The time a climb may take: a fixed allowance to line up and to step off, plus
 * a second per block of height — the climb speed is 2.35 blocks a second up and 3
 * down (`delvewright_dsl::metrics::climb_blocks_per_tick`, `CLIMB_SLIDE`), so a
 * second a block is more than twice what either needs.
 */
export function climbBudgetMs(climb: Climb): number {
  return 6_000 + 1_000 * Math.abs(climb.top[1] - climb.bottom[1] + 1);
}

/** The blocks of `#minecraft:climbable` the bot's client physics climbs. */
export const CLIENT_CLIMBABLE = ["minecraft:ladder", "minecraft:vine"] as const;

/** Whether the bot's client physics climbs `block` (a block state). */
export function clientClimbs(block: string): boolean {
  const id = block.split("[")[0]!;
  return (CLIENT_CLIMBABLE as readonly string[]).includes(id);
}

export const methods = {
  /**
   * Drive the climb `spec.climb`, from wherever the previous hop left the bot
   * (within a block of the climb's `from`) to `spec` (the climb's `to`):
   *
   * - **up**: walk into the column's centre, then push against the ladder (or,
   *   for a vine, toward the column) holding jump until the feet are clear of the
   *   top rung by the height of the let-go floor, then steer onto the let-go
   *   cell;
   * - **down**: walk into the column — off the brink when it is entered from
   *   above, which the climbable catches — then hold nothing, so the body slides,
   *   until the feet are at the let-go cell's height, then step onto it.
   *
   * Fails loudly, naming where the bot is, when the budget runs out.
   */
  async climbHop(this: MineflayerExecutor, spec: GoalSpec, label: string): Promise<void> {
    const climb = spec.climb;
    if (!climb) throw new Error(`${label}: climbHop called on a hop with no climb`);
    if (!clientClimbs(climb.block)) {
      throw new Error(
        `${label}: the compiler proved a climb on \`${climb.block}\`, and the bot's client ` +
          `physics (prismarine-physics) climbs only ${CLIENT_CLIMBABLE.join(" and ")} — this ` +
          `climb is beyond the harness, not beyond the route (spec-0099 §6)`,
      );
    }
    const bot = this.requireBot();
    const dir = climbDirection(climb);
    const deadline = Date.now() + climbBudgetMs(climb);
    const centre = (c: readonly number[]): [number, number] => [c[0]! + 0.5, c[2]! + 0.5];
    const [cx, cz] = centre(climb.bottom);
    const [tx, tz] = centre(climb.to);
    const horiz = (x: number, z: number): number => {
      const p = bot.entity.position;
      return Math.hypot(p.x - x, p.z - z);
    };
    const face = async (x: number, y: number, z: number): Promise<void> => {
      const p = bot.entity.position;
      try {
        await bot.lookAt(p.offset(x - p.x, y - p.y, z - p.z), true);
      } catch {
        // best effort — a look failure must not abort the climb
      }
    };
    const fail = (what: string): never => {
      throw new Error(
        `${label}: climb ${dir} ${climb.block} [${climb.bottom.join(", ")}]..[${climb.top.join(", ")}] ` +
          `→ [${climb.to.join(", ")}] ${what}; the bot is at ${fmt(bot.entity.position)}`,
      );
    };
    process.stderr.write(
      `[climb] ${label}: ${dir} ${climb.block} from [${climb.from.join(", ")}] to ` +
        `[${climb.to.join(", ")}], column [${climb.bottom.join(", ")}]..[${climb.top.join(", ")}]\n`,
    );
    try {
      bot.clearControlStates();
      // 1. Into the column. Going down from a brink this walks off it; the
      //    climbable catches the body within a block.
      while (horiz(cx, cz) > COLUMN_ALIGN) {
        if (Date.now() > deadline) fail("did not reach the column");
        await face(cx, bot.entity.position.y + 1.6, cz);
        bot.setControlState("forward", true);
        await delay(CLIMB_TICK_MS);
      }
      bot.setControlState("forward", false);
      if (dir === "up") {
        // 2. Push and hold jump until the feet clear the top rung up to the let-go
        //    floor (`to.y` is the feet cell of the floor it steps onto).
        while (bot.entity.position.y < climb.to[1] + 0.05) {
          if (Date.now() > deadline) fail("did not climb clear of the top rung");
          const push = supportCell(climb, Math.floor(bot.entity.position.y));
          if (push) {
            await face(push[0] + 0.5, bot.entity.position.y + 1.6, push[2] + 0.5);
            bot.setControlState("forward", true);
          }
          bot.setControlState("jump", true);
          await delay(CLIMB_TICK_MS);
        }
      } else {
        // 2. Hold nothing and slide down to the let-go height.
        bot.clearControlStates();
        while (bot.entity.position.y > climb.to[1] + 0.05 && !bot.entity.onGround) {
          if (Date.now() > deadline) fail("did not slide down to the let-go height");
          await delay(CLIMB_TICK_MS);
        }
      }
      // 3. Step onto the let-go cell.
      while (horiz(tx, tz) > LET_GO_REACH || !bot.entity.onGround) {
        if (Date.now() > deadline) fail("did not step onto the let-go cell");
        await face(tx, bot.entity.position.y + 1.6, tz);
        bot.setControlState("forward", horiz(tx, tz) > LET_GO_REACH);
        bot.setControlState("jump", dir === "up" && bot.entity.position.y < climb.to[1] + 0.2);
        await delay(CLIMB_TICK_MS);
      }
      process.stderr.write(`[climb] ${label}: let go at ${fmt(bot.entity.position)}\n`);
    } finally {
      bot.clearControlStates();
    }
  },
};
