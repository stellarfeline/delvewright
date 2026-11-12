// MineflayerExecutor: a census performed by command at the stance of the act
// gated on it.

import type { CensusRow, Step } from "../critical-path.ts";
import { censusOf } from "../critical-path.ts";
import { censusCommands, censusReleaseCommands, censusTerm, censusVerdict } from "../census.ts";
import { isRejection } from "../rejection.ts";
import { delay } from "./connection.ts";
import type { MineflayerExecutor } from "../executor.ts";

/** One census period (10 ticks) and its margin, in ms: what the bot waits
 * after placing the bodies before it reads the count. */
export const CENSUS_SETTLE_MS = 1500;

/** The refusal in a bracketed reply, or the reply's absence, as a sentence. */
function refusalIn(reply: { answered: boolean; lines: readonly string[] }): string | undefined {
  if (!reply.answered) return "the closing marker never arrived";
  return reply.lines.find((line) => isRejection(line));
}

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  /**
   * spec-0134 §1: perform `act` with every census of `step` placed at the
   * act's stance. The act walks to where it acts and calls {@link atStance}
   * there; only then are the bodies put in their boxes — all of them at once,
   * held still — and the count read back after one census period, so the
   * datum the act reads is the one just placed, not one a body has walked out
   * of while the bot was on its way. The held bodies are let go when the act
   * ends, however it ends. An act that ends without reaching its stance placed
   * nothing and is refused by name.
   */
  async performCensus(this: MineflayerExecutor, step: Step, act: () => Promise<void>): Promise<void> {
    // spec-0135 §5: a `riders` census is read by the ride that boards the
    // vehicle, at its end (`executor/ride.ts`).
    const rows = censusOf(step).filter((row) => row.riders === undefined);
    if (rows.length === 0) return act();
    const states = rows.map((r) => r.state).join(", ");
    let placed = false;
    this.stanceDue = async () => {
      placed = true;
      await this.placeCensus(rows);
    };
    let failed = false;
    try {
      await act();
    } catch (err) {
      failed = true;
      if (!placed) {
        process.stderr.write(`[census] ${states}: the act ended before its stance — no body was placed\n`);
      }
      throw err;
    } finally {
      this.stanceDue = undefined;
      if (placed) {
        const refused = refusalIn(await this.bracketed(censusReleaseCommands()));
        if (refused !== undefined) {
          process.stderr.write(`[census] ${states}: letting the held bodies go was refused — ${refused}\n`);
          if (!failed) throw new Error(`census ${states}: letting the held bodies go was refused — ${refused}`);
        }
      }
    }
    if (!placed) {
      throw new Error(
        `census ${states}: never placed — the step's act ended without reaching its stance, so ` +
          `what it read was not the count of bodies the bot put in the box`,
      );
    }
  },

  /**
   * The act has walked to where it acts and acts next: perform what is due at
   * the stance (the step's census), once. A no-op for a step that owes none.
   */
  async atStance(this: MineflayerExecutor): Promise<void> {
    const due = this.stanceDue;
    this.stanceDue = undefined;
    if (due) await due();
  },

  /**
   * Put every row's bodies in its box and hold them there, in one bracketed
   * turn; wait one census period, once; read every row's count back. The first
   * row the game does not count as placed fails the step by name.
   */
  async placeCensus(this: MineflayerExecutor, rows: readonly CensusRow[]): Promise<void> {
    for (const row of rows) {
      process.stderr.write(`[census] ${row.state}: putting ${row.count} body(ies) of ${row.select} in the box\n`);
    }
    const commands = rows.flatMap((row) => censusCommands(row));
    if (commands.length > 0) {
      const refused = refusalIn(await this.bracketed(commands));
      if (refused !== undefined) {
        throw new Error(`census ${rows.map((r) => r.state).join(", ")}: placing the bodies was refused — ${refused}`);
      }
    }
    await delay(CENSUS_SETTLE_MS);
    for (const row of rows) {
      const result = censusVerdict(row, await this.askTerm(censusTerm(row)));
      this.censusResults.push(result);
      process.stderr.write(`[census] ${row.state}: ${result.passed ? "counted" : "FAILED"} — ${result.detail}\n`);
      if (!result.passed) throw new Error(`census ${row.state}: ${result.detail}`);
    }
  },
};
