// MineflayerExecutor: a transport: awaiting the forced move to its destination.

import { TRANSPORT_NEAR } from "../critical-path.ts";
import { delay, fmt } from "./connection.ts";
import { REACH_POLL_MS } from "./walk.ts";
import type { MineflayerExecutor } from "../executor.ts";

/**
 * gap 8: how long (ms) to wait for a cross-area teleport to land after a step whose
 * completion relocates the player, how close (blocks, horizontal) counts as
 * "arrived at the destination", and how long to settle once it has.
 */
const TRANSPORT_TIMEOUT_MS = 15_000;

// `TRANSPORT_NEAR` lives in `critical-path.ts`, where the parser refuses a link
// hop it could not observe (spec-0083 §4).
const TRANSPORT_SETTLE_MS = 1_500;

/**
 * gap 8: after the jump lands, how long (ms) to wait for the destination
 * chunk to load and the bot to come to rest on solid ground before the next step
 * starts pathfinding, and the poll cadence. The pathfinder's A* fails immediately
 * ("No path to the goal!") if it starts while the block under the bot is still
 * unloaded (`blockAt` → null) — the race this closes. Bounded: on timeout the wait
 * settles and lets the next step surface its own diagnostic.
 */
const FOOTING_TIMEOUT_MS = 10_000;

const FOOTING_POLL_MS = 100;

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  /**
   * Whether the bot currently stands at/near a compiler-exported transport
   * destination — the same arrival predicate {@link awaitTransport} uses, so the
   * mid-walk check and the post-step settle can never disagree about "arrived".
   */
  atTransportDest(this: MineflayerExecutor, dest: readonly [number, number, number]): boolean {
    const bot = this.bot;
    if (!bot?.entity) return false;
    const p = bot.entity.position;
    return (
      Math.abs(p.x - (dest[0] + 0.5)) < TRANSPORT_NEAR &&
      Math.abs(p.z - (dest[2] + 0.5)) < TRANSPORT_NEAR &&
      Math.abs(p.y - dest[1]) < 4
    );
  },

  /**
   * gap 8: after a step whose completion teleports the player to another area, hold
   * the next step's pathfinding until the relocation has fully landed. Areas sit
   * ~256 blocks apart across void, so the destination is far from the pre-teleport
   * position and the arrival is unambiguous. Navigation plumbing only — no game logic.
   *
   * Three deterministic phases, each bounded and death-aware so nothing
   * can hang the run and a mid-transport death still fails fast:
   *   1. Wait for the position to jump to near `dest` — the teleport landing. The
   *      `forcedMove` handler resets the pathfinder as the jump arrives, so a path
   *      computed in the old area cannot survive it.
   *   2. Reset the pathfinder again here (belt-and-braces): the next `walkTo` must
   *      start from a clean state at the new position.
   *   3. Wait for the destination chunk to load and the bot to rest on solid footing.
   *      A `walkTo` that starts while the block under the bot is still unloaded
   *      (`blockAt` → null) makes the pathfinder's A* fail instantly with "No path to
   *      the goal!"; this closes that race at the boundary rather than racing ahead.
   * If the jump is not observed within the budget, settle briefly and let the next
   * step surface its own diagnostic.
   */
  async awaitTransport(this: MineflayerExecutor, dest: readonly [number, number, number]): Promise<void> {
    const bot = this.requireBot();
    const [x, y, z] = dest;
    const arrived = await this.waitFor(
      () => this.atTransportDest(dest),
      TRANSPORT_TIMEOUT_MS,
      REACH_POLL_MS,
    );
    // Drop any path/goal still referencing the old area (the forcedMove handler has
    // usually done this already; idempotent).
    this.stopPathfinding();
    if (!arrived) {
      process.stderr.write(
        `[transport] did not observe the jump to [${x}, ${y}, ${z}] within ` +
          `${TRANSPORT_TIMEOUT_MS}ms; bot at ${fmt(bot.entity.position)} — continuing\n`,
      );
      await delay(TRANSPORT_SETTLE_MS);
      return;
    }
    // The jump landed: wait for the destination chunk to load and the bot to settle
    // onto solid ground before the next step pathfinds from here.
    const footed = await this.waitFor(
      () => bot.entity.onGround === true && bot.blockAt(bot.entity.position) != null,
      FOOTING_TIMEOUT_MS,
      FOOTING_POLL_MS,
    );
    if (!footed) {
      process.stderr.write(
        `[transport] landed near [${x}, ${y}, ${z}] but footing/chunk not confirmed ` +
          `within ${FOOTING_TIMEOUT_MS}ms; bot at ${fmt(bot.entity.position)} — continuing\n`,
      );
    }
    await delay(TRANSPORT_SETTLE_MS);
  },
};
