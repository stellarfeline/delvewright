// MineflayerExecutor: a jump the compiler proved (spec-0113 §6), driven raw.
//
// A leg whose route leaps carries the leap in its waypoint artifact
// (`legs[].jumps[]`): the launch cell, the landing cell, whether it is sprinted
// and the straight run-up behind the launch the compiler measured. The compiler
// proves the jump by flying it (`World::declared_jump`): a body at rest in the
// cell `run_up` cells behind the launch, facing along the jump's axis, holds
// forward (sprinting when declared) and presses jump on either of the last
// `DECLARED_JUMP_PRESS_TICKS` ticks it stands over the launch cell's column —
// under the game's rules and under the bot's client physics alike. The hop
// between the two ends is driven here exactly so, with the bot's own controls,
// one game tick at a time.
//
// It is never handed to mineflayer-pathfinder 2.4.5's parkour move
// (`getMoveParkourForward`): that move lands two cells forward at most with
// sprinting off, so a walking jump a body makes — three forward and one down,
// say — is no neighbour of its launch; and with sprinting on, it sprints a jump
// the proof flew walking, which can carry the body past a landing the walk
// stops on.
//
// Navigation, not game logic: the route is the compiler's, and every control
// below is one a player presses on that route.

import type { MineflayerExecutor } from "../executor.ts";
import type { Vec3Tuple } from "../critical-path.ts";
import { jumpAxis, type GoalSpec, type Jump } from "../waypoints.ts";
import { fmt } from "./connection.ts";

/** How close (blocks, horizontally) the body stops to the run-up's start before
 * it is taken as standing there. */
const START_REACH = 0.12;

/** How far (blocks, horizontally) off the run-up a body may stand for the
 * drive to take it from there — across the line, and past either end along
 * it: inside the run-up's own cells, so the crouched creep only ever walks
 * the straight run of standable cells the compiler measured. Farther, the
 * executor walks it there first ({@link methods.jumpHop}). */
export const RUN_UP_REACH = 0.75;

/** The body's half width (a player's box is 0.6 wide). */
const HALF_WIDTH = 0.3;

/** Below this, two feet heights are one height: the client physics moves a
 * body by sums of floats, never by less. */
const FEET_EPS = 1e-6;

/**
 * Whether feet at height `y` are those of a body standing in the cell row
 * `cellY`, by the compiler's rule (`feet_of`): a body standing in cell `c`
 * rests on the collision top of the block under it, `c - 1`, and that top lies
 * above the block's floor and at most half a block over `c`'s own floor (a
 * fence's or a wall's 1.5). So the feet lie in `(c - 1, c + 0.5]`: on a lily
 * pad or a carpet the feet are almost a whole block under the cell's floor,
 * on a fence half a block over it. A body one cell lower stands with its feet
 * at `c - 1` or under, outside the band.
 */
export function feetInCell(y: number, cellY: number): boolean {
  return y > cellY - 1 + FEET_EPS && y <= cellY + 0.5 + FEET_EPS;
}

/** How many ticks the body stands still at the run-up's start before it runs
 * (the compiler flies the run from rest), and on the landing before the jump
 * is judged. */
const SETTLE_TICKS = 4;

/** The ticks a jump may take: a fixed allowance to line up and to land, plus
 * twenty per cell of run-up walked back crouched and run again. */
export function jumpBudgetTicks(jump: Jump): number {
  return 160 + 20 * jump.runUp;
}

/** The cell the compiler's run starts at: `runUp` cells behind the launch. */
export function runUpStart(jump: Jump): Vec3Tuple {
  const [ax, az] = jumpAxis(jump);
  return [jump.from[0] - ax * jump.runUp, jump.from[1], jump.from[2] - az * jump.runUp];
}

/** Whether a body centred at `(x, z)` lies over `cell`'s column at all — the
 * compiler's `box_over_column`. */
export function boxOverColumn(x: number, z: number, cell: Vec3Tuple): boolean {
  return (
    x + HALF_WIDTH > cell[0] &&
    x - HALF_WIDTH < cell[0] + 1 &&
    z + HALF_WIDTH > cell[2] &&
    z - HALF_WIDTH < cell[2] + 1
  );
}

/** Whether a body at `pos` stands on the run-up: on the straight run of
 * cells from its start to the launch, within {@link RUN_UP_REACH}, its feet
 * those of a body standing in their row ({@link feetInCell}). */
export function onRunUp(pos: { x: number; y: number; z: number }, jump: Jump): boolean {
  const [ax, az] = jumpAxis(jump);
  const [fx, fz] = [jump.from[0] + 0.5, jump.from[2] + 0.5];
  // Along the axis from the launch's centre (negative behind it), and across.
  const along = (pos.x - fx) * ax + (pos.z - fz) * az;
  const across = Math.abs((pos.x - fx) * az - (pos.z - fz) * ax);
  return (
    along <= RUN_UP_REACH &&
    along >= -jump.runUp - RUN_UP_REACH &&
    across <= RUN_UP_REACH &&
    feetInCell(pos.y, jump.from[1])
  );
}

/**
 * Whether the jump is pressed this tick: the body stands over the launch cell's
 * column, and two more ticks at its present speed would carry it off — so this
 * is one of the last two ticks it stands there, the window the compiler proved
 * (`DECLARED_JUMP_PRESS_TICKS`).
 */
export function pressNow(
  pos: { x: number; z: number },
  vel: { x: number; z: number },
  onGround: boolean,
  launch: Vec3Tuple,
): boolean {
  return (
    onGround &&
    boxOverColumn(pos.x, pos.z, launch) &&
    !boxOverColumn(pos.x + 2 * vel.x, pos.z + 2 * vel.z, launch)
  );
}

/** The part of a mineflayer bot a jump drives. */
export interface JumpBot {
  readonly entity: {
    readonly position: { x: number; y: number; z: number };
    readonly velocity: { x: number; y: number; z: number };
    readonly onGround: boolean;
    readonly isCollidedHorizontally?: boolean;
  };
  setControlState(control: "forward" | "jump" | "sprint" | "sneak", state: boolean): void;
  clearControlStates(): void;
  look(yaw: number, pitch: number, force?: boolean): Promise<void>;
  on(event: "physicsTick", listener: () => void): unknown;
  removeListener(event: "physicsTick", listener: () => void): unknown;
}

/** Where a driven jump came down. */
export interface JumpLanding {
  readonly at: Vec3Tuple;
  /** The tick of the run the jump was pressed on. */
  readonly pressedAt: number;
  /** How many runs were set back to the start because the server moved the
   * body before the press ({@link driveJump}, step 2). */
  readonly reruns: number;
}

/** How many times one drive sets a disturbed run back to its start. */
export const MAX_RERUNS = 3;

/**
 * **Drive `jump` as the compiler flew it**, one physics tick at a time:
 *
 * 1. once the body is on the floor (a walk that brought it here may hand it
 *    over still falling into the cell), from a body standing on the run-up
 *    ({@link onRunUp}; any other is refused at once, never walked across the
 *    geometry), creep, crouched, along it to the start's centre — a crouched
 *    body does not walk off an edge, and it stops where it is let go — and
 *    stand there, on the floor, for {@link SETTLE_TICKS} ticks;
 * 2. face along the jump's axis, hold forward (and sprint when the jump is
 *    sprinted), and press jump once, on one of the last two ticks the body
 *    stands over the launch cell's column ({@link pressNow}). A body the client
 *    reports off the floor before the press with its feet where they stood is
 *    one the server moved — a hurt sets the body's motion, and the client
 *    then sees no floor under a body that has not fallen: that run is not the
 *    one the compiler flew, so it is set back to step 1, at most
 *    {@link MAX_RERUNS} times. A body whose feet moved off the floor before
 *    the press fails;
 * 3. release on landing, stand still, and require the feet in the landing's
 *    row, past the launch.
 *
 * `standing(true)` is called as the body stands up to run and `standing(false)`
 * as it crouches to creep, so a sneak leg's crouch hold yields to the run.
 * Rejects, naming what went wrong, when the body misses or the budget runs out.
 */
export function driveJump(
  bot: JumpBot,
  jump: Jump,
  standing: (up: boolean) => void = () => {},
): Promise<JumpLanding> {
  const start = runUpStart(jump);
  const [ax, az] = jumpAxis(jump);
  const [sx, sz] = [start[0] + 0.5, start[2] + 0.5];
  const budget = jumpBudgetTicks(jump);
  return new Promise<JumpLanding>((resolve, reject) => {
    type Phase = "alight" | "creep" | "settle" | "run" | "air" | "land";
    let phase: Phase = "alight";
    let tick = 0;
    let still = 0;
    let pressedAt = -1;
    let restFeet = Number.NaN;
    let reruns = 0;
    const finish = (err?: string): void => {
      bot.removeListener("physicsTick", onTick);
      bot.clearControlStates();
      const p = bot.entity.position;
      if (err !== undefined) reject(new Error(err));
      else resolve({ at: [p.x, p.y, p.z], pressedAt, reruns });
    };
    const onTick = (): void => {
      tick += 1;
      const e = bot.entity;
      const p = e.position;
      if (tick > budget) {
        finish(`did not land within ${budget} ticks (stuck in its ${phase})`);
        return;
      }
      if (phase === "alight") {
        if (!e.onGround) return;
        if (!onRunUp(p, jump)) {
          finish(`is not on its run-up [${start.join(", ")}]..[${jump.from.join(", ")}]`);
          return;
        }
        phase = "creep";
      }
      switch (phase) {
        case "creep": {
          const dx = sx - p.x;
          const dz = sz - p.z;
          if (Math.hypot(dx, dz) <= START_REACH) {
            bot.setControlState("forward", false);
            bot.setControlState("jump", false);
            phase = "settle";
            still = 0;
            return;
          }
          void bot.look(Math.atan2(-dx, -dz), 0, true);
          bot.setControlState("sneak", true);
          bot.setControlState("forward", true);
          return;
        }
        case "settle": {
          // From rest on the floor, as the compiler's run starts.
          still = e.onGround ? still + 1 : 0;
          if (still < SETTLE_TICKS) return;
          restFeet = p.y;
          standing(true);
          bot.setControlState("sneak", false);
          void bot.look(Math.atan2(-ax, -az), 0, true);
          bot.setControlState("sprint", jump.sprint);
          bot.setControlState("forward", true);
          phase = "run";
          return;
        }
        case "run": {
          if (!e.onGround) {
            if (Math.abs(p.y - restFeet) <= FEET_EPS && reruns < MAX_RERUNS) {
              reruns += 1;
              tick = 0;
              bot.clearControlStates();
              standing(false);
              phase = "alight";
              return;
            }
            finish(
              Math.abs(p.y - restFeet) <= FEET_EPS
                ? `was moved off its run by the server ${reruns + 1} times before a tick to press jump on`
                : "left the floor before a tick to press jump on",
            );
            return;
          }
          if (pressNow(p, e.velocity, e.onGround, jump.from)) {
            bot.setControlState("jump", true);
            pressedAt = tick;
            phase = "air";
          }
          return;
        }
        case "air": {
          // One press, never held: a held jump leaps again on landing.
          bot.setControlState("jump", false);
          if (e.onGround && tick > pressedAt + 1) {
            bot.setControlState("forward", false);
            bot.setControlState("sprint", false);
            phase = "land";
            still = 0;
          }
          return;
        }
        case "land": {
          still += 1;
          if (still < SETTLE_TICKS) return;
          const along = (p.x - (jump.from[0] + 0.5)) * ax + (p.z - (jump.from[2] + 0.5)) * az;
          if (!feetInCell(p.y, jump.to[1]) || along < 1) finish("came down off the landing's floor");
          else finish();
          return;
        }
      }
    };
    standing(false);
    bot.clearControlStates();
    bot.on("physicsTick", onTick);
  });
}

export const methods = {
  /**
   * Drive the jump `spec.jump` ({@link driveJump}) to `spec` (the jump's `to`).
   * A bot off the run-up — a blow knocked it off a one-wide plank, or the
   * previous hop left it beside the line — is walked to the run-up's start first, by the
   * staged pathfinder walk every other hop takes (`sneak` as its leg), so the
   * drive starts where the proof's run starts. Fails loudly, naming where the
   * bot is.
   */
  async jumpHop(
    this: MineflayerExecutor,
    spec: GoalSpec,
    label: string,
    sneak = false,
  ): Promise<void> {
    const jump = spec.jump;
    if (!jump) throw new Error(`${label}: jumpHop called on a hop with no jump`);
    const bot = this.requireBot();
    const how = `${jump.sprint ? "sprinted" : "walking"}, run-up ${jump.runUp}`;
    process.stderr.write(
      `[jump] ${label}: [${jump.from.join(", ")}] → [${jump.to.join(", ")}] (${how}) from ` +
        `[${runUpStart(jump).join(", ")}]\n`,
    );
    const start = runUpStart(jump);
    if (!onRunUp(bot.entity.position, jump)) {
      process.stderr.write(
        `[jump] ${label}: the bot is at ${fmt(bot.entity.position)}, off the run-up; ` +
          `walking it to [${start.join(", ")}]\n`,
      );
      await this.gotoStaged({ x: start[0], y: start[1], z: start[2], range: 0 }, `${label} (run-up)`, sneak);
    }
    const yielded = this.crouchYielded;
    try {
      const landing = await driveJump(bot as unknown as JumpBot, jump, (up) => {
        this.crouchYielded = up;
      });
      const reran =
        landing.reruns > 0
          ? ` (after ${landing.reruns} run(s) the server moved the body on were set back to the start)`
          : "";
      process.stderr.write(
        `[jump] ${label}: pressed on tick ${landing.pressedAt}, landed at ${fmt(bot.entity.position)}${reran}\n`,
      );
    } catch (e) {
      throw new Error(
        `${label}: jump [${jump.from.join(", ")}] → [${jump.to.join(", ")}] (${how}) ` +
          `${(e as Error).message}; the bot is at ${fmt(bot.entity.position)}`,
      );
    } finally {
      this.crouchYielded = yielded;
      bot.clearControlStates();
    }
  },
};
