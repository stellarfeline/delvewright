// MineflayerExecutor: a loop: the forced move it makes, and crossing one.

import type { LoopStep, Step, Vec3Tuple } from "../critical-path.ts";
import { delay, fmt } from "./connection.ts";
import { POLL_MS } from "./clock.ts";
import { goals } from "./walk.ts";
import { GameClockWatch } from "../game-time.ts";
import { LOOP_MOVE_TICKS, LoopCrossWatch, serverMove, type ServerMove } from "../loop-cross.ts";
import type { MineflayerExecutor } from "../executor.ts";

/**
 * gap 8: a server-forced position jump of at least this many blocks
 * (horizontal) is treated as a cross-area teleport rather than knockback or a
 * within-area nudge. Areas sit ~256 blocks apart across void (an unambiguous jump);
 * in-area relocations (spawn, class teleport) stay well under this, so the threshold
 * cleanly separates a transport from ordinary forced moves. When one is observed the
 * pathfinder is reset, so a path computed in the OLD area cannot survive the jump and
 * strand the next step with a spurious "No path to the goal!".
 */
/** spec-0086 §6: whether a forced move's delta is a loop's offset. */
function sameDelta(delta: Vec3Tuple, offset: Vec3Tuple): boolean {
  return delta.every((c, i) => Math.abs(c - offset[i]!) <= LOOP_DELTA_TOLERANCE);
}

/** A server move as a failure names it: its delta, or where it set the body and on which axes. */
function fmtMove(move: ServerMove | undefined): string {
  const f = (v: Vec3Tuple): string => `[${v.map((c) => c.toFixed(3)).join(", ")}]`;
  if (!move) return "with no position packet read";
  return move.relative
    ? `by ${f(move.delta)}`
    : `to ${f(move.to)} (absolute on ${move.absolute.join(", ")}, so a move to a place, not by a delta)`;
}

const TRANSPORT_JUMP_BLOCKS = 64;

/**
 * spec-0086 §6: how close a forced move's delta must come to a loop's offset, on
 * every axis, to be that loop's move. The spike measured the delta exact on 10
 * of 10 moves (`tools/spike-seamless-loop/ at 2278c55877e2`), so this is a numeric tolerance,
 * not a judgement.
 */
const LOOP_DELTA_TOLERANCE = 1e-3;

/**
 * The crossing walk aims at the slab's own cell, `cross`, never past it: the
 * server reads a body's position once per tick, after every movement packet
 * queued since the last, so a server that falls behind while the bot walks on
 * sees the body jump from before the slab to beyond it, and the loop never
 * holds it. A body that stops in the slab is moved whenever the server next
 * ticks.
 */

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  /**
   * Handle a server-forced position update. Records the new position and, when the
   * jump is large enough to be a cross-area teleport, resets the pathfinder. Reading
   * the position after the event is correct: mineflayer sets it before emitting.
   */
  onForcedMove(this: MineflayerExecutor): void {
    const bot = this.bot;
    const p = bot?.entity?.position;
    // spec-0086 §6: the move as the server stated it, from the packet that
    // caused this event; taken once, so a later event never reads it again.
    const move = serverMove(this.lastPositionPacket);
    this.lastPositionPacket = undefined;
    if (!p) return;
    const now = { x: p.x, y: p.y, z: p.z };
    const prev = this.lastForcedPos;
    this.lastForcedPos = now;
    if (this.loopWatch) {
      this.loopWatch.moves.push(move);
    } else if (move?.relative) {
      for (const [id, offset] of this.loopOffsets) {
        if (sameDelta(move.delta, offset)) {
          this.loopFault = new Error(
            `a forced move by ${fmtMove(move)} during a plain ` +
              `walk is loop ${id}'s offset — the loop held where the compiler proved it ` +
              `released, so the proof's configuration and the game disagree about its gate`,
          );
          this.stopPathfinding();
          break;
        }
      }
    }
    if (prev && Math.hypot(now.x - prev.x, now.z - prev.z) >= TRANSPORT_JUMP_BLOCKS) {
      this.stopPathfinding();
    }
  },

  /** The latched {@link loopFault}, cleared as it is taken. */
  takeLoopFault(this: MineflayerExecutor): Error | undefined {
    const f = this.loopFault;
    this.loopFault = undefined;
    return f;
  },

  /**
   * spec-0086 §6: the loops the path exercises, so a forced move during a plain
   * walk that equals one of their offsets fails that walk naming the loop.
   */
  useLoops(this: MineflayerExecutor, steps: readonly Step[]): void {
    this.loopOffsets = new Map(
      steps.flatMap((s): Array<[string, Vec3Tuple]> =>
        s.action === "loop" ? [[s.loop, s.offset]] : [],
      ),
    );
  },

  /**
   * **Exercise a loop** (spec-0086 §6): walk to the step's approach cell, set a
   * walk goal in the slab, and wait for the server's forced move. The move
   * must be exactly the loop's offset — within {@link LOOP_DELTA_TOLERANCE} on
   * each axis, as the server's position packet states it ({@link serverMove}),
   * never measured against a position the client sampled — and on it the pathfinder
   * is stopped, so the stale goal does not walk the body into the slab again.
   * Repeated until `times` moves are seen. A forced move with any other delta
   * fails the step naming the loop, the delta seen and the count reached, and so
   * does a crossing {@link crossSlab} judges failed. The move is identified by
   * its delta, never by its size: {@link TRANSPORT_JUMP_BLOCKS} is not consulted.
   */
  async exerciseLoop(this: MineflayerExecutor, step: LoopStep): Promise<void> {
    const bot = this.requireBot();
    const label = `loop ${step.loop}`;
    const axis = step.offset.findIndex((c) => c !== 0);
    const dir = -Math.sign(step.offset[axis]!);
    const goal: Vec3Tuple = [step.cross[0], step.cross[1], step.cross[2]];
    await this.walkTo(step.pos, 1, `${label} — to its approach`);
    for (let seen = 0; seen < step.times; seen++) {
      if (seen > 0) {
        await this.walkTo(step.pos, 1, `${label} — back to its approach`, false, undefined, [
          step.pos,
        ]);
      }
      this.loopWatch = { moves: [] };
      let moves: Array<ServerMove | undefined>;
      try {
        moves = await this.crossSlab(step, label, goal, axis, dir, seen);
      } finally {
        this.loopWatch = undefined;
      }
      const move = moves[0];
      if (!move?.relative || !sameDelta(move.delta, step.offset)) {
        throw new Error(
          `${label}: a forced move ${fmtMove(move)} is not the loop's relative move by ` +
            `[${step.offset.join(", ")}]; ${seen} of ${step.times} move(s) seen before it`,
        );
      }
      const delta = move.delta;
      process.stderr.write(
        `[loop] ${step.loop}: move ${seen + 1} of ${step.times} by ` +
          `[${delta.map((c) => c.toFixed(3)).join(", ")}], bot now at ${fmt(bot.entity.position)}\n`,
      );
    }
  },

  /**
   * **One crossing of the slab** (spec-0086 §6): walk toward `goal`, the slab's
   * cell, until the server's forced move arrives, and return the deltas seen.
   * Judged by {@link LoopCrossWatch}, in the server's game ticks
   * (`executor/clock.ts`): the step fails when the body has stood at or past the
   * slab's near face through {@link LOOP_MOVE_TICKS} game ticks with no move, when
   * the walk keeps ending short of the slab without advancing or holds its trip,
   * live, without advancing, or when the game clock stops ({@link GameClockWatch}).
   * A trip that ends short is walked again. No wall-clock budget: a slow or
   * stalled host stretches or stops the
   * ticks, and never fails the crossing.
   */
  async crossSlab(
    this: MineflayerExecutor,
    step: LoopStep,
    label: string,
    goal: Vec3Tuple,
    axis: number,
    dir: number,
    seen: number,
  ): Promise<Array<ServerMove | undefined>> {
    const bot = this.requireBot();
    const here = (): Vec3Tuple => {
      const p = bot.entity.position;
      return [p.x, p.y, p.z];
    };
    const moved = (): boolean => (this.loopWatch?.moves.length ?? 0) > 0;
    const watch = new LoopCrossWatch(label, step.cross, axis, dir, seen, step.times);
    const purpose = `${label}, crossing ${seen + 1} of ${step.times}`;
    let clock: GameClockWatch | undefined;
    let ended: { why: string } | undefined;
    let trip: Promise<void> | undefined;
    const walk = (): void => {
      ended = undefined;
      watch.tripBegins(here());
      const t = this.nav.goto(new goals.GoalNear(goal[0], goal[1], goal[2], 0)).then(
        () => {
          if (trip === t) ended = { why: "the pathfinder resolved" };
        },
        (err: unknown) => {
          if (trip === t) ended = { why: err instanceof Error ? err.message : String(err) };
        },
      );
      trip = t;
    };
    try {
      walk();
      for (;;) {
        if (this.death) throw this.death;
        if (moved()) break;
        if (ended !== undefined) {
          if (watch.tripEnded(here(), ended.why)) walk();
          else ended = undefined;
        }
        // The position first, then the reading: the server held the body there
        // when it answered, and a move it sent by then arrived before the answer.
        const at = here();
        const reading = await this.readGameTime(purpose);
        if (this.death) throw this.death;
        if (moved()) break;
        const answeredAt = performance.now();
        clock ??= new GameClockWatch(purpose, reading.tick, LOOP_MOVE_TICKS);
        clock.observe(reading.tick, answeredAt);
        watch.reading(reading.tick, at, answeredAt);
        await delay(POLL_MS);
      }
    } finally {
      this.stopPathfinding();
      const t = trip;
      trip = undefined;
      await t;
    }
    return this.loopWatch?.moves ?? [];
  },
};
