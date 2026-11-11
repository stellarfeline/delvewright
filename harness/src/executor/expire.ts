// MineflayerExecutor: an expire step: a limited objective let lapse (spec-0133 §8).

import type { ExpireStep } from "../critical-path.ts";
import { expiredMarkerLine, markerLine } from "../markers.ts";
import { nextLegWaypoints } from "../waypoints.ts";
import { fmt } from "./connection.ts";
import { EFFECT_SETTLE_MS, OBJECTIVE_TIMEOUT_MS } from "./objective.ts";
import { SCORE_POLL_MS } from "./score.ts";
import type { MineflayerExecutor } from "../executor.ts";

/** Minecraft server ticks per second. */
const TICKS_PER_SECOND = 20;

/**
 * How long (ms) an expire step may wait for its marker: the compiler's
 * `wait_ticks` — the window, a ceiling on what is left of it — converted at the
 * server's rate, plus the ordinary objective grace. Pure arithmetic.
 */
export function expireWindowMs(waitTicks: number): number {
  return (waitTicks * 1000) / TICKS_PER_SECOND + OBJECTIVE_TIMEOUT_MS;
}

/**
 * The verdict once the wait ends: `undefined` when the objective expired and
 * never completed; otherwise the failure, named. Pure, for its unit test.
 */
export function expireVerdict(
  step: ExpireStep,
  campaignId: string,
  expiredAt: number | undefined,
  completedAt: number | undefined,
): string | undefined {
  if (completedAt !== undefined && (expiredAt === undefined || completedAt <= expiredAt)) {
    return (
      `expire ${step.objective}: \`${markerLine(campaignId, step.objective)}\` arrived — the bot ` +
      `completed the objective it was to let lapse. The stance was wrong, and the plan missed it`
    );
  }
  if (expiredAt === undefined) {
    return (
      `expire ${step.objective}: no \`${expiredMarkerLine(campaignId, step.objective)}\` arrived ` +
      `within ${step.waitTicks} tick(s) and the grace`
    );
  }
  return undefined;
}

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  /**
   * Walk to the step's `stand` and do nothing — no chat, no use, no swing —
   * until the objective's expiry marker arrives. A completion marker for the
   * same objective first fails the step by name.
   */
  async expire(this: MineflayerExecutor, step: ExpireStep): Promise<void> {
    // The compiler exports a leg into this step, ending at its critical
    // position: where the path left the party. A lapse walks nowhere, so the
    // lockstep cursor consumes that leg unwalked, exactly as the step that
    // walks it would have; left in place, the next walk takes it and every
    // walk after runs one leg behind, with no proven route under it.
    if (this.waypoints) {
      const own = nextLegWaypoints(this.waypoints.legs, this.legCursor, step.pos);
      this.legCursor = own.cursor;
      if (own.matched) {
        process.stderr.write(
          `[leg] the wait for ${step.objective}: its leg to [${step.pos.join(", ")}] consumed ` +
            `unwalked — the party waits where the path left it\n`,
        );
      }
    }
    // The walk to the stand is no leg of the path's: it names none, so it
    // can never take the next step's.
    await this.walkTo(step.stand, 0, `the wait for ${step.objective}`, step.sneak, undefined, undefined, null);
    const campaign = this.campaignId ?? "?";
    const windowMs = expireWindowMs(step.waitTicks);
    process.stderr.write(
      `[expire] ${step.objective}: waiting up to ${step.waitTicks} tick(s) at ` +
        `${fmt(this.requireBot().entity.position)}\n`,
    );
    const started = Date.now();
    await this.waitFor(
      () =>
        this.expiredObjectives.has(step.objective) ||
        this.completedObjectives.has(step.objective),
      windowMs,
      SCORE_POLL_MS,
    );
    const why = expireVerdict(
      step,
      campaign,
      this.expiredObjectives.get(step.objective),
      this.completedObjectives.get(step.objective),
    );
    const waited = Date.now() - started;
    this.expireWaits.push({ objective: step.objective, waitedMs: waited, passed: why === undefined });
    if (why !== undefined) throw new Error(why);
    process.stderr.write(`[expire] ${step.objective}: expired after ${waited}ms\n`);
    await new Promise((r) => setTimeout(r, EFFECT_SETTLE_MS));
  },
};
