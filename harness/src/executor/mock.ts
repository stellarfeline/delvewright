// MineflayerExecutor: a mock step: an act no step kind drives, run as its engine function.

import type { MockStep } from "../critical-path.ts";
import { mockCommand, mockVerdict } from "../mock.ts";
import { delay } from "./connection.ts";
import { EFFECT_SETTLE_MS } from "./objective.ts";
import type { MineflayerExecutor } from "../executor.ts";

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  /**
   * spec-0108 §6.5: ask every `before` term, run the act's own bundle function
   * inside a reply bracket on the opped command channel, then ask every `after`
   * term. The step passes only when all of it holds; it is printed and recorded
   * as `mocked` either way, so a mocked act never reads as a driven one.
   */
  async mock(this: MineflayerExecutor, step: MockStep): Promise<void> {
    const command = mockCommand(step);
    process.stderr.write(
      `[mock] ${step.act} (${step.kind}): no step kind drives it — running ${command}\n`,
    );
    const before: (boolean | undefined)[] = [];
    for (const t of step.before) before.push(await this.askTerm(t));
    const reply = await this.bracketed([command]);
    await delay(EFFECT_SETTLE_MS);
    const after: (boolean | undefined)[] = [];
    for (const t of step.after) after.push(await this.askTerm(t));
    const result = mockVerdict(step, before, reply, after);
    this.mockResults.push(result);
    process.stderr.write(
      `[mock] ${step.act}: ${result.passed ? "performed" : "FAILED"} — ${result.detail}\n`,
    );
    if (!result.passed) {
      throw new Error(`mock ${step.act} (${step.function}): ${result.detail}`);
    }
  },
};
