// MineflayerExecutor: a rest step at a bonfire.

import type { Entity } from "prismarine-entity";
import type { RestStep, Vec3Tuple } from "../critical-path.ts";
import { type PerformedRest } from "../combat.ts";
import { delay, fmt } from "./connection.ts";
import { AFFORDANCE_RADIUS } from "./crosshair.ts";
import { EFFECT_SETTLE_MS } from "./objective.ts";
import type { MineflayerExecutor } from "../executor.ts";

const REST_RANGE = 2;

/** Grace between the bonfire click and the trigger command: the opener runs as an
 * advancement reward, so `dw.rest` is enabled a tick or two after the click. */
const REST_OPEN_SETTLE_MS = 500;

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  /**
   * Rest at a bonfire — the player loop every later proof depends on.
   *
   * Two acts, in this order, and the order is the whole thing:
   *
   *   1. **right-click the `dw_bonfire_<i>` interaction**. This is not flavour. The
   *      click is what fires the `player_interacted_with_entity` advancement whose
   *      reward opens the dialog AND `enable`s the `dw.rest` trigger. Until then the
   *      trigger is DISABLED and the chat line below is a silent no-op.
   *   2. **chat the step's command** — the exact line the "rest and save" button runs.
   *
   * Why not click the dialog button: a `dialog show` is rendered client-side and
   * mineflayer models no dialog at all, so there is no button to press. The button's
   * command is the primitive the compiler exports precisely so a headless client can
   * perform the same loop; `/trigger` is also the only command form a non-operator
   * player may run, so this is the player's own path, not an op shortcut.
   *
   * The affordance is found by POSITION, not by tag: entity `Tags` are server-side
   * and never reach a client. The compiler puts the interaction on the step's own
   * anchor cell, so the nearest `interaction` entity to it is the fire.
   *
   * Actuation only. Nothing here asserts the checkpoint moved — the next die-retry
   * trial's respawn position is what proves that, and it proves it the way a player
   * would find out.
   */
  async rest(this: MineflayerExecutor, step: RestStep): Promise<void> {
    const bot = this.requireBot();
    await this.walkTo(step.pos, REST_RANGE, `bonfire ${step.anchor}`, step.sneak);
    // The one step in the tree with a REAL right-click, so acquisition and
    // actuation are the same act here: the ray picks the fire, the bot looks
    // where the ray went, and only then does it click. Selecting the nearest
    // affordance to a coordinate — what this used to do — cannot tell a fire from
    // whatever is standing in front of it.
    const acquired = this.requireCrosshair(step.pos, `bonfire ${step.anchor}`, REST_RANGE);
    const fire = acquired ? bot.entities[acquired.target.id] : undefined;
    if (!acquired || !fire) {
      throw new Error(
        `no \`interaction\` affordance within ${AFFORDANCE_RADIUS} blocks of bonfire ` +
          `${step.bonfire} at [${step.pos.join(", ")}] — the bot is standing at the fire ` +
          `and there is nothing to right-click, so the rest can never be performed ` +
          `(bot at ${fmt(bot.entity.position)})`,
      );
    }
    process.stderr.write(
      `[rest] bonfire ${step.bonfire} (${step.anchor}): right-clicking the affordance, ` +
        `then \`${step.command}\`\n`,
    );
    const here = bot.entity.position;
    await bot.lookAt(
      here.offset(acquired.aim.x - here.x, acquired.aim.y - here.y, acquired.aim.z - here.z),
      true,
    );
    await bot.activateEntity(fire);
    // The opener runs through an advancement reward, so the trigger is enabled a
    // tick or two after the click lands — chatting inside the same tick would be
    // refused exactly as chatting without clicking is.
    await delay(REST_OPEN_SETTLE_MS);
    bot.chat(step.command);
    await delay(EFFECT_SETTLE_MS);
    this.restedBonfires.add(step.bonfire);
    this.restedAt.set(step.bonfire, this.currentStep);
    this.seatEpoch += 1;
  },

  /** The nearest `minecraft:interaction` affordance to `pos`, if one is tracked. */
  affordanceAt(this: MineflayerExecutor, pos: Vec3Tuple): Entity | undefined {
    const bot = this.requireBot();
    let best: Entity | undefined;
    let bestDist = AFFORDANCE_RADIUS;
    for (const e of Object.values(bot.entities)) {
      if (e?.name !== "interaction" || !e.position) continue;
      const d = Math.sqrt(
        (e.position.x - (pos[0] + 0.5)) ** 2 +
          (e.position.y - pos[1]) ** 2 +
          (e.position.z - (pos[2] + 0.5)) ** 2,
      );
      if (d <= bestDist) {
        best = e;
        bestDist = d;
      }
    }
    return best;
  },

  /** Adopt the rest steps the exported critical path carries, with
   * their EXPORTED indices — the coordinate system the precondition compares in. */
  useRestSteps(this: MineflayerExecutor, rests: readonly PerformedRest[]): void {
    this.restSteps = rests;
  },

  /** Rests this run performed, and the bonfires among them. For the run report. */
  performedRests(this: MineflayerExecutor): readonly PerformedRest[] {
    return this.restSteps.filter((r) => this.restedBonfires.has(r.bonfire));
  },
};
