// MineflayerExecutor: waiting, once per connection, for the delve to finish placing its world.

import { isRejection } from "../rejection.ts";
import {
  PLACEMENT_POLL_MS,
  PlacementLatch,
  PlacementRefusedError,
  PlacementTimeoutError,
  parsePlacementReply,
  placementCommands,
  placementLine,
  type PlacementReading,
  type PlacementRecord,
} from "../placement.ts";
import { delay } from "./connection.ts";
import type { MineflayerExecutor } from "../executor.ts";

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  /**
   * **Return once the server says the world is placed and the first join is
   * behind the bot** (see `placement.ts` for the signal and why each half of it
   * is needed). Called by {@link connect} between the spawn and anything else, so
   * every stage that starts a leg (the critical path and the die-retry trials
   * inside it, a driven branch's path, the death loop after it) starts after it.
   *
   * Readings are taken one at a time, each in its own reply bracket, and every
   * line of each reply is judged by the shared rejection rule: a refused reading
   * fails the run at once ({@link PlacementRefusedError}). The budget is
   * {@link placementTimeoutMs}, counted from the spawn; running out of it fails
   * the run ({@link PlacementTimeoutError}). Neither is ever read as a pass.
   */
  async awaitPlacement(this: MineflayerExecutor, spawnedAt: number = Date.now()): Promise<PlacementRecord> {
    const deadline = spawnedAt + this.placementTimeoutMs;
    const latch = new PlacementLatch();
    let readings = 0;
    let unanswered = 0;
    let last: PlacementReading | undefined;
    for (;;) {
      if (Date.now() >= deadline) {
        throw new PlacementTimeoutError(this.placementTimeoutMs, readings, unanswered, last);
      }
      const serial = ++this.placementReads;
      readings += 1;
      const reply = await this.bracketed(placementCommands(serial));
      if (!reply.answered) {
        // Not observed is not an answer either way; the next reading asks again.
        unanswered += 1;
        continue;
      }
      const refused = reply.lines.find((line) => isRejection(line));
      if (refused !== undefined) throw new PlacementRefusedError(refused);
      last = parsePlacementReply(serial, reply.lines);
      const verdict = latch.observe(last);
      if (verdict.ready) {
        const record: PlacementRecord = {
          placedTick: verdict.placedTick,
          firstJoin: verdict.firstJoin,
          waitedMs: Date.now() - spawnedAt,
          readings,
          unanswered,
        };
        this.placement = record;
        process.stderr.write(placementLine(record));
        return record;
      }
      await delay(Math.min(PLACEMENT_POLL_MS, Math.max(0, deadline - Date.now())));
    }
  },
};
