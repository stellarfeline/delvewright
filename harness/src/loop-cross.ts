// The judge of one crossing of a loop's slab (spec-0086 §6).
//
// What a loop owes is a move: its tick line runs every game tick, and moves
// every body whose box meets its slab in that tick. So the crossing is judged in
// the server's game ticks while the body stands in the slab, never in wall time
// while the bot walks toward it. The walk to the slab is the harness's own: the
// pathfinder can end a trip short of its goal (mineflayer-pathfinder resolves a
// `goto` on an empty partial path, which its 40ms-per-tick A* budget returns
// whenever the host stalls the bot mid-search), and a trip ended short says
// nothing about the loop; it is walked again.
//
// Ordering, not time, places the move against a reading (`game-time.ts`): the
// body's position is taken BEFORE a reading is sent, and a client sends its
// position in the physics tick that moved it, so the server held the body at
// that position when it answered. A move the loop sent in any tick up to the
// answer's arrives before the answer does.
//
// This file is pure. The walk and the readings are `executor/loop.ts`.

import type { Vec3Tuple } from "./critical-path.ts";
import { LIVE_REPLY_GAP_MS, type GameClockWatch } from "./game-time.ts";

/**
 * How many game ticks the body may stand at or past the slab's near face with no
 * forced move before the loop is judged not to have moved it. The loop's tick
 * line moves a body in the tick the server first holds it in the slab, and the
 * move reaches the bot before the reply of any reading answered a tick later; a
 * second's worth of ticks is the margin for the server's own movement
 * correction of a body it holds at the slab's edge.
 */
export const LOOP_MOVE_TICKS = 20;

/**
 * How many trips toward the slab, one after another, may each end with the
 * body short of the slab and no further along the loop's axis than before it,
 * before the walk is judged unable to reach the slab. A trip ends short on a
 * pathfinder event, and a trip that moved the body forward (by at least
 * {@link LOOP_ADVANCE_BLOCKS}) resets the count, so
 * this counts events, never time.
 */
export const LOOP_BARREN_TRIPS = 8;

/** How far (blocks) along the axis a trip must take the body past its best to count as advancing. */
export const LOOP_ADVANCE_BLOCKS = 0.1;

/**
 * How long (ms) of a live server and a live bot, short of the slab, with the
 * body not advancing, make the walk toward the slab stalled. Live is the
 * judgement {@link GameClockWatch} makes: only the gaps of at most
 * {@link LIVE_REPLY_GAP_MS} between consecutive answered readings are summed,
 * so a host that stalls the server, the bot or both adds one long gap and
 * nothing to the sum. A live bot and server walk a body the length of any slab's
 * approach in a few seconds of such answers.
 */
export const LOOP_WALK_STALL_MS = 20_000;

/**
 * How far the body is past the slab's near face along the loop's axis, in
 * blocks: negative short of it, zero or more at it or beyond. `dir` is the
 * direction of the walk across the slab (`-sign(offset)` on the axis); the near
 * face of `cross` is its low face when `dir` is positive, its high face when not.
 */
export function pastFace(at: Vec3Tuple, cross: Vec3Tuple, axis: number, dir: number): number {
  return dir > 0 ? at[axis]! - cross[axis]! : cross[axis]! + 1 - at[axis]!;
}

/** The named failure: the body stood in the slab and the loop did not move it. */
export class LoopNotMovedError extends Error {
  constructor(label: string, since: number, tick: number, at: Vec3Tuple, seen: number, times: number) {
    super(
      `${label}: no forced move: the body stood at or past the slab's near face from game tick ` +
        `${since} to ${tick} (${tick - since} of ${LOOP_MOVE_TICKS} game ticks allowed), last at ` +
        `[${at.map((c) => c.toFixed(1)).join(", ")}]; ${seen} of ${times} move(s) seen`,
    );
    this.name = "LoopNotMovedError";
  }
}

/** The named failure: the walk toward the slab held its trip, live, and the body did not advance. */
export class LoopWalkStalledError extends Error {
  constructor(label: string, liveMs: number, replies: number, at: Vec3Tuple, seen: number, times: number) {
    super(
      `${label}: the walk toward the slab stalled short of its near face: the server answered ` +
        `${replies} reading(s) over ${liveMs}ms, each within ${LIVE_REPLY_GAP_MS}ms of the one ` +
        `before, and the body did not advance ${LOOP_ADVANCE_BLOCKS} block along the loop's axis ` +
        `(stalled is ${LOOP_WALK_STALL_MS}ms of such answers), body at ` +
        `[${at.map((c) => c.toFixed(1)).join(", ")}]; ${seen} of ${times} move(s) seen`,
    );
    this.name = "LoopWalkStalledError";
  }
}

/** The named failure: the walk toward the slab kept ending short of it without advancing. */
export class LoopWalkBarrenError extends Error {
  constructor(label: string, trips: number, at: Vec3Tuple, why: string, seen: number, times: number) {
    super(
      `${label}: the walk toward the slab ended ${trips} time(s) in a row short of its near face ` +
        `without advancing along the loop's axis (the last: ${why}), body at ` +
        `[${at.map((c) => c.toFixed(1)).join(", ")}]; ${seen} of ${times} move(s) seen`,
    );
    this.name = "LoopWalkBarrenError";
  }
}

/**
 * **One crossing, judged.** Fed every reading (the game tick it answered and the
 * body's position taken before it was sent) and every end of a trip toward the
 * slab. It throws {@link LoopNotMovedError} when the body has stood at or past
 * the near face through {@link LOOP_MOVE_TICKS} game ticks, and
 * {@link LoopWalkBarrenError} when {@link LOOP_BARREN_TRIPS} trips in a row
 * ended short without advancing, and {@link LoopWalkStalledError} when, short
 * of the face, {@link LOOP_WALK_STALL_MS} of live answers pass with the body
 * not advancing. A slow server stretches the game ticks and a stalled host stops
 * them and adds nothing live; none of the three counts is reached by either.
 */
export class LoopCrossWatch {
  private readonly label: string;
  private readonly cross: Vec3Tuple;
  private readonly axis: number;
  private readonly dir: number;
  private readonly seen: number;
  private readonly times: number;
  /** The game tick of the first reading taken with the body at or past the face, in the current stand. */
  private since: number | undefined;
  /** The furthest the body has been along the axis (blocks past the face) since the last trip began. */
  private best = -Infinity;
  /** The furthest it had been when the last trip began. */
  private bestAtIssue = -Infinity;
  private barren = 0;
  /** The furthest the body had been when the current stall began. */
  private bestAtStall = -Infinity;
  private lastAnswerAt: number | undefined;
  private stallMs = 0;
  private stallReplies = 0;

  constructor(label: string, cross: Vec3Tuple, axis: number, dir: number, seen: number, times: number) {
    this.label = label;
    this.cross = cross;
    this.axis = axis;
    this.dir = dir;
    this.seen = seen;
    this.times = times;
  }

  /** Whether the body at `at` is at or past the slab's near face. */
  atFace(at: Vec3Tuple): boolean {
    return pastFace(at, this.cross, this.axis, this.dir) >= 0;
  }

  /** A trip toward the slab begins with the body at `at`. */
  tripBegins(at: Vec3Tuple): void {
    this.note(at);
    this.bestAtIssue = this.best;
  }

  /**
   * A reading answered at game tick `tick`, its reply taken at `answeredAt` on a
   * monotonic clock (`performance.now()`), with no forced move received before
   * it; `at` is the body's position taken before the reading was sent.
   */
  reading(tick: number, at: Vec3Tuple, answeredAt: number): void {
    this.note(at);
    const gap = this.lastAnswerAt === undefined ? undefined : answeredAt - this.lastAnswerAt;
    this.lastAnswerAt = answeredAt;
    if (!this.atFace(at)) {
      this.since = undefined;
      if (this.best >= this.bestAtStall + LOOP_ADVANCE_BLOCKS) {
        this.bestAtStall = this.best;
        this.stallMs = 0;
        this.stallReplies = 0;
      } else if (gap !== undefined && gap >= 0 && gap <= LIVE_REPLY_GAP_MS) {
        this.stallMs += gap;
        this.stallReplies += 1;
        if (this.stallMs >= LOOP_WALK_STALL_MS) {
          throw new LoopWalkStalledError(this.label, this.stallMs, this.stallReplies, at, this.seen, this.times);
        }
      }
      return;
    }
    this.since ??= tick;
    if (tick - this.since >= LOOP_MOVE_TICKS) {
      throw new LoopNotMovedError(this.label, this.since, tick, at, this.seen, this.times);
    }
  }

  /**
   * The trip toward the slab ended (`why`: arrival or the pathfinder's reason)
   * with the body at `at` and no forced move received. Returns whether the walk
   * is made again: not when the body is at or past the face, where the loop owes
   * the move and the readings judge it.
   */
  tripEnded(at: Vec3Tuple, why: string): boolean {
    this.note(at);
    if (this.atFace(at)) return false;
    this.barren = this.best >= this.bestAtIssue + LOOP_ADVANCE_BLOCKS ? 0 : this.barren + 1;
    if (this.barren >= LOOP_BARREN_TRIPS) {
      throw new LoopWalkBarrenError(this.label, this.barren, at, why, this.seen, this.times);
    }
    return true;
  }

  private note(at: Vec3Tuple): void {
    this.best = Math.max(this.best, pastFace(at, this.cross, this.axis, this.dir));
  }
}
