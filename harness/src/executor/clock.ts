// MineflayerExecutor: reading the server's game time, and waiting game ticks.

import { isRejection } from "../rejection.ts";
import { GameClockWatch, GameTimeUnreadError, gameTimeCommands, parseGameTime } from "../game-time.ts";
import { delay } from "./connection.ts";
import type { MineflayerExecutor } from "../executor.ts";

/** One reading of the server's game time, and what else its bracket asked. */
export interface GameTimeReading {
  /** The game tick the reading was answered in. */
  readonly tick: number;
  /** Every line of the reply, for a caller that appended commands. */
  readonly lines: readonly string[];
  /** The serial of the reading, which numbers its lines. */
  readonly serial: number;
}

/** How many times an unanswered reading is asked again before it fails. */
const READ_ATTEMPTS = 3;

/** How long (ms) the wait pauses between one answered reading and the next. */
export const POLL_MS = 50;

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  /**
   * **The server's game time now** (`game-time.ts`), with `extra(serial)`'s
   * commands run in the same tick after the reading. Every line of the reply
   * is judged by the shared rejection rule: a refusal fails at once
   * ({@link GameTimeUnreadError}). A reply never observed is asked again, up to
   * {@link READ_ATTEMPTS} times, and then fails; silence is never a reading.
   */
  async readGameTime(
    this: MineflayerExecutor,
    purpose: string,
    extra: (serial: number) => readonly string[] = () => [],
  ): Promise<GameTimeReading> {
    for (let attempt = 1; ; attempt += 1) {
      const serial = ++this.clockReads;
      const reply = await this.bracketed([...gameTimeCommands(serial), ...extra(serial)]);
      if (!reply.answered) {
        if (attempt < READ_ATTEMPTS) continue;
        throw new GameTimeUnreadError(purpose, `${READ_ATTEMPTS} reading(s) went unanswered`);
      }
      const refused = reply.lines.find((line) => isRejection(line));
      if (refused !== undefined) {
        throw new GameTimeUnreadError(purpose, `the server refused the reading (${JSON.stringify(refused)})`);
      }
      const tick = parseGameTime(serial, reply.lines);
      if (tick === undefined) {
        throw new GameTimeUnreadError(purpose, "the reply carried no game time");
      }
      return { tick, lines: reply.lines, serial };
    }
  },

  /**
   * **Return once the server's game time has advanced `ticks` past `from`**
   * (a reading taken now when `from` is not given), and return the reading that
   * showed it. Everything the bot received before that reading's reply was
   * sent at that tick or earlier. It fails only when the game clock has
   * stopped ({@link GameClockWatch}): the server keeps answering and its game
   * time does not move. However long the host stalls, the wait never fails on
   * wall time.
   */
  async waitForGameTicks(
    this: MineflayerExecutor,
    ticks: number,
    purpose: string,
    from?: number,
  ): Promise<GameTimeReading> {
    const start = from ?? (await this.readGameTime(purpose)).tick;
    const watch = new GameClockWatch(purpose, start, ticks);
    for (;;) {
      const now = await this.readGameTime(purpose);
      if (now.tick >= start + ticks) return now;
      watch.observe(now.tick, Date.now());
      await delay(POLL_MS);
    }
  },
};
