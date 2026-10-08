// MineflayerExecutor: a witness-strike step.

import type { WitnessStrikeStep } from "../critical-path.ts";
import { delay } from "./connection.ts";
import { GATE_DASH_TICK_MS } from "./timed-gate.ts";
import type { MineflayerExecutor } from "../executor.ts";

/** One server tick, in ms. */
const TICK_MS = 50;

/** How long a strike witness drives onto its cell's centre before standing. */
const WITNESS_CENTRE_MS = 4000;

/** How near the cell's centre a strike witness stands, in blocks. */
const WITNESS_CENTRE_TOLERANCE = 0.2;

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  /**
   * **Witness an assembly's blow** (spec-0082 §5.4, §5.5): walk to the step's
   * cell, step onto its centre the way a player presses W, and stand there for
   * the window, reading the bot's own health off the server's packets.
   *
   * `struck` passes on the first drop — the blow of the facing a body on that
   * cell draws, landing on the limb's area — and fails when the window runs
   * out with nothing taken. `spared` passes when the window runs out with
   * nothing taken and fails on any drop: no body there can be selected by the
   * arming region, so no blow is wound up for it.
   *
   * The walk there is a walk leg, held at full health like every other; the
   * standing is not, so a drop while standing is the delve's, not the
   * harness's. Nothing is staged: the blow is the thing under test.
   */
  async witnessStrike(this: MineflayerExecutor, step: WitnessStrikeStep): Promise<void> {
    const bot = this.requireBot();
    const label = `${step.assembly}'s blow (${step.expect})`;
    await this.walkTo(step.pos, 1, label);
    // Onto the cell's centre: the pathfinder's nearest goal is a block away,
    // and the facing a body draws is read from where it stands.
    const centre = (): number => {
      const p = bot.entity.position;
      return Math.hypot(p.x - (step.pos[0] + 0.5), p.z - (step.pos[2] + 0.5));
    };
    const driveUntil = Date.now() + WITNESS_CENTRE_MS;
    try {
      while (Date.now() < driveUntil && centre() > WITNESS_CENTRE_TOLERANCE) {
        if (this.death) throw this.death;
        const p = bot.entity.position;
        await bot.lookAt(p.offset(step.pos[0] + 0.5 - p.x, 0, step.pos[2] + 0.5 - p.z), true);
        bot.setControlState("forward", true);
        await delay(GATE_DASH_TICK_MS);
      }
    } finally {
      bot.clearControlStates();
    }
    const p0 = bot.entity.position;
    const at = `[${p0.x.toFixed(2)}, ${p0.y.toFixed(2)}, ${p0.z.toFixed(2)}]`;
    const start = bot.health;
    const windowMs = step.windowTicks * TICK_MS;
    const until = Date.now() + windowMs;
    let taken = 0;
    while (Date.now() < until) {
      if (this.death) throw this.death;
      if (bot.health < start - 1e-6) {
        taken = start - bot.health;
        break;
      }
      await delay(TICK_MS);
    }
    const facing =
      step.expect === "struck"
        ? ` facing ${step.facing} of ${step.facingCount} (root yaw ${step.yaw})`
        : "";
    if (step.expect === "struck" && taken === 0) {
      throw new Error(
        `${label}: the bot stood at ${at} on [${step.pos.join(", ")}] — a landing cell under ` +
          `the limb of${facing} — for ${step.windowTicks} ticks and no blow took any health. ` +
          `The thing was wound up for a body there and never hit it`,
      );
    }
    if (step.expect === "spared" && taken > 0) {
      throw new Error(
        `${label}: the bot stood at ${at} on [${step.pos.join(", ")}], where no body can be ` +
          `selected by the arming region, and lost ${taken.toFixed(1)} health — a blow nobody ` +
          `was shown`,
      );
    }
    process.stderr.write(
      `[witness] ${label}: at ${at}${facing}, ` +
        (step.expect === "struck"
          ? `struck for ${taken.toFixed(1)} (declared ${step.amount}) within ${step.windowTicks} ticks\n`
          : `nothing taken in ${step.windowTicks} ticks\n`),
    );
  },
};
