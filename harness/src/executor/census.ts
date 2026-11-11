// MineflayerExecutor: a census performed by command before a step gated on it.

import type { Step } from "../critical-path.ts";
import { censusOf } from "../critical-path.ts";
import { censusCommands, censusTerm, censusVerdict } from "../census.ts";
import { delay } from "./connection.ts";
import type { MineflayerExecutor } from "../executor.ts";

/** One census period (10 ticks) and its margin, in ms: what the bot waits
 * after placing the bodies before it reads the count. */
export const CENSUS_SETTLE_MS = 1500;

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  /**
   * spec-0134 §1: put every census's bodies in its box on the opped command
   * channel, wait one census period, and read the datum back. The step is
   * refused by name when the game's count disagrees with the bodies placed.
   */
  async performCensus(this: MineflayerExecutor, step: Step): Promise<void> {
    for (const row of censusOf(step)) {
      const commands = censusCommands(row);
      process.stderr.write(
        `[census] ${row.state}: putting ${row.count} body(ies) of ${row.select} in the box\n`,
      );
      if (commands.length > 0) await this.bracketed(commands);
      await delay(CENSUS_SETTLE_MS);
      const result = censusVerdict(row, await this.askTerm(censusTerm(row)));
      this.censusResults.push(result);
      process.stderr.write(`[census] ${row.state}: ${result.passed ? "counted" : "FAILED"} — ${result.detail}\n`);
      if (!result.passed) throw new Error(`census ${row.state}: ${result.detail}`);
    }
  },
};
