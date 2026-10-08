// MineflayerExecutor: awaiting entity settle after a join.

import { hasSettled } from "../entity-settle.ts";
import type { MineflayerExecutor } from "../executor.ts";

/**
 * How often (ms) {@link MineflayerExecutor.awaitEntitySettle} polls the non-player
 * entity count while waiting for the tracker to stop changing shape after spawn.
 */
const ENTITY_SETTLE_POLL_MS = 200;

/**
 * Hard ceiling (ms) on the post-spawn entity-settle wait (2026-08-06 island
 * triage). World-persisted entities were observed populating by t+4s; this
 * leaves comfortable margin without letting a build that never spawns anything
 * near the bot hang the run — the wait always gives up and proceeds, and the
 * existing "not tracked" crosshair warning stays honest about whatever state the
 * tracker is actually in when a step reads it.
 */
export const ENTITY_SETTLE_TIMEOUT_MS = 8_000;

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  /**
   * Wait once, at spawn, for `bot.entities` to stop changing shape before anything
   * reads it (2026-08-06 island triage — see entity-settle.ts). Polls the
   * non-player entity count until it has held steady for
   * {@link hasSettled}'s stability window, or gives up after
   * {@link entitySettleTimeoutMs} and proceeds regardless — a build that
   * legitimately spawns nothing near the bot must not hang the run behind this
   * wait, and every caller downstream (`requireCrosshair` foremost) already
   * reports an empty tracker honestly rather than inventing a verdict from it.
   */
  async awaitEntitySettle(this: MineflayerExecutor): Promise<void> {
    const bot = this.requireBot();
    const history: number[] = [];
    const settled = await this.waitFor(
      () => {
        const count = Object.values(bot.entities).filter(
          (e) => e && e.id !== bot.entity?.id && e.type !== "player",
        ).length;
        history.push(count);
        return hasSettled(history);
      },
      this.entitySettleTimeoutMs,
      ENTITY_SETTLE_POLL_MS,
    );
    const last = history[history.length - 1] ?? 0;
    if (settled) {
      process.stderr.write(
        `[entity-settle] tracker settled at ${last} non-player entit${last === 1 ? "y" : "ies"} ` +
          `after ${history.length} poll(s)\n`,
      );
    } else {
      process.stderr.write(
        `[entity-settle] gave up after ${this.entitySettleTimeoutMs}ms waiting for the entity ` +
          `tracker to stop changing shape (last count: ${last}) — proceeding; a step that finds ` +
          `nothing tracked still reports that honestly rather than failing on it\n`,
      );
    }
  },
};
