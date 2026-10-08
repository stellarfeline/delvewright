// MineflayerExecutor: a trigger step: the `/trigger` echo, firing an env trigger.

import type { TriggerStep } from "../critical-path.ts";
import { markerLine } from "../markers.ts";
import { delay, fmt } from "./connection.ts";
import { AFFORDANCE_RADIUS } from "./crosshair.ts";
import { INTERACT_RANGE } from "./interact.ts";
import { OBJECTIVE_TIMEOUT_MS, EFFECT_SETTLE_MS } from "./objective.ts";
import { SCORE_POLL_MS } from "./score.ts";
import type { MineflayerExecutor } from "../executor.ts";

/**
 * A `/trigger …` a step sent, and every chat line the server answered it with.
 * The bot is opped, so vanilla's command feedback — the success
 * `Triggered [obj] (set value to n)` and every refusal — arrives on the same chat
 * stream the completion markers do; capturing it is what lets a timed-out step say
 * whether its trigger reached the delve at all.
 */
export interface TriggerEcho {
  readonly command: string;
  /** The scoreboard objective the command names, e.g. `dw.dlg_eurylochus`. */
  readonly objective: string;
  /** Server answers observed since the command was sent, in order. */
  readonly lines: string[];
}

/**
 * What the server said about a step's `/trigger`, as a clause appended to that
 * step's objective-timeout message. Diagnostics only — it decides nothing and
 * relaxes nothing; a step that times out still fails.
 *
 * Why it exists (and round 13's `requires_item` defect before it): a
 * swallowed trigger and an undelivered one produce the identical bare 30s timeout,
 * and telling them apart cost a full round of misattributed red runs each time. The
 * server answers every `/trigger` — `Triggered [obj] (set value to n)` on success, a
 * refusal otherwise — and the bot, opped, receives that answer on the same chat
 * stream the completion markers arrive on. So the harness repeats it back:
 *   - answered ⇒ the delve's own guard consumed the trigger without completing the
 *     objective. The classic cause is a REUSED world: a scoreboard that already
 *     carries the objective makes its `unless score … matches 1` guard a no-op, so
 *     nothing completes and nothing is broadcast (island round 13, and again here);
 *   - unanswered ⇒ the command never reached the delve at all, which is the
 *     harness's own problem, not the campaign's.
 *
 * The unanswered reading assumes the delve leaves `sendCommandFeedback` alone, which
 * every compiled delve does — `setup.mcfunction`'s gamerule block never touches it.
 * A campaign that suppressed feedback would silence the answer and read as
 * unanswered; if that ever becomes possible, this must be told, not left to infer.
 */
export function swallowedTriggerVerdict(echo: TriggerEcho | undefined): string {
  if (!echo) return "";
  if (echo.lines.length === 0) {
    return (
      `; the server never answered \`${echo.command}\` — the trigger did not reach the ` +
      `delve (refused or undelivered), so this is a harness/infrastructure failure, ` +
      `not a content one`
    );
  }
  return (
    `; the server ANSWERED \`${echo.command}\` with ${echo.lines.map((l) => `"${l}"`).join(", ")} ` +
    `— the trigger reached the delve and its own guard consumed it without completing ` +
    `the objective. The usual cause is a re-used world whose scoreboard already ` +
    `carries this objective (its \`unless score … matches 1\` guard then completes ` +
    `nothing): tear the stack down with \`validation/fresh-volumes.sh --project ` +
    `<compose-project>\` and re-run on a proven-clean world`
  );
}

/**
 * The scoreboard objective a `/trigger <objective> …` command names, or `undefined`
 * for anything that is not a trigger command.
 */
export function triggerObjective(command: string): string | undefined {
  return /^\/trigger\s+(\S+)/.exec(command.trim())?.[1];
}

/**
 * Whether a chat line is the server ANSWERING a `/trigger <objective>`. Two shapes,
 * because vanilla's success and failure messages are worded independently:
 *   - success names the objective (its display name defaults to its id):
 *     `Triggered [dw.dlg_eurylochus] (set value to 4)`;
 *   - the refusals do not name it, but all of them say "trigger":
 *     `You can't trigger this objective yet`, `This objective is not a trigger`.
 * Only ever consulted inside the window between the harness sending a trigger and
 * that step finishing, where the only such traffic is the answer to our own command.
 */
export function answersTrigger(message: string, objective: string): boolean {
  return message.includes(objective) || /trigger/i.test(message);
}

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  /**
   * Send a step's `/trigger …` command and start listening for the server's own
   * answer to it (see {@link TriggerEcho}). Every trigger-driven step goes through
   * here, so a step that times out can always say whether its trigger REACHED the
   * delve — the one fact that separates a content/state failure from a harness one.
   */
  chatTrigger(this: MineflayerExecutor, command: string): void {
    this.armTrigger(command);
    this.requireBot().chat(command);
  },

  /**
   * Start listening for the server's answer to `command` without sending it — for
   * the steps whose chat is issued by a helper (`interact` goes through
   * `presentAndTrigger`, which must equip first and chat last).
   */
  armTrigger(this: MineflayerExecutor, command: string): void {
    const objective = triggerObjective(command);
    this.trigger = objective === undefined ? undefined : { command, objective, lines: [] };
  },

  /**
   * Perform an environment trigger the way a player does, then wait for the
   * trigger's own fired marker — the only evidence the step accepts.
   *
   * A `strike` is a real attack (`bot.attack`, the client's left-click packet) on
   * the `interaction` hitbox the compiler summoned at the anchor; a `use` is a
   * real right-click on it; a `strike-assembly` attacks the assembly's own
   * hitbox at its cell; a `strike-npc` attacks the NPC's own hitbox at its
   * beat's station; an `approach` is a walk into the trigger's range; a `step`
   * is a walk onto the plate's cell. Never a
   * server-side command: a trigger fired by one would prove the command, not
   * that a player can reach and hit the thing.
   *
   * The target is acquired by the same crosshair rule every click step uses —
   * proximity proposes, the ray decides — so a hitbox buried in a wall or behind
   * another body fails here, naming both, rather than as a silent miss.
   */
  async fireTrigger(this: MineflayerExecutor, step: TriggerStep): Promise<void> {
    const bot = this.requireBot();
    const label = `trigger ${step.trigger} (${step.on})`;
    // A repeated performance (a hit count) owes its own marker: count the
    // arrivals before acting, and wait for one more.
    const performed = this.triggerPerformances.get(step.trigger) ?? 0;
    this.triggerPerformances.set(step.trigger, performed + 1);
    const arrivalsBefore = this.markerArrivals.get(step.trigger) ?? 0;
    // spec-0083 §4: a trigger that carries the party is performed from INSIDE
    // its volume — the compiler names the cell. The bot walks there as a block
    // goal, and then acts without walking again; the sequencer awaits the
    // landing (`transport`) after the fired marker, as for every carried step.
    if (step.stand) {
      await this.walkTo(step.stand, 0, `${label} — to its stand cell`);
      process.stderr.write(
        `[trigger] ${step.trigger}: standing at [${step.stand.join(", ")}] to be carried to ` +
          `[${(step.transport ?? []).join(", ")}]\n`,
      );
    }
    if (step.on === "approach") {
      // The tick fires on `distance=..range` from the anchor cell; aim a block
      // inside it so the goal's own tolerance cannot leave the bot on the rim.
      if (!step.stand) {
        await this.walkTo(step.pos, Math.max(1, (step.range ?? 1) - 1), label);
      }
    } else if (step.on === "step") {
      // The tick fires on a body in the plate's own cell: a block goal on it.
      if (!step.stand) {
        await this.walkTo(step.pos, 0, label);
      }
    } else if (step.block !== undefined) {
      // spec-0093 §6.5: a `use` trigger whose prop a hand presses has no
      // hitbox; the bot right-clicks the block itself.
      if (!step.stand) {
        await this.walkTo(step.pos, INTERACT_RANGE, label);
      }
      await this.pressBlock(step.pos, step.block, label);
    } else {
      if (!step.stand) {
        await this.walkTo(step.pos, INTERACT_RANGE, label);
      }
      const acquired = this.requireCrosshair(step.pos, label, INTERACT_RANGE);
      const target = acquired ? bot.entities[acquired.target.id] : undefined;
      if (!acquired || !target) {
        throw new Error(
          `${label}: no \`interaction\` hitbox within ${AFFORDANCE_RADIUS} blocks of ` +
            `[${step.pos.join(", ")}] — the bot is standing at the target and there is ` +
            `nothing to ${step.on === "use" ? "right-click" : "hit"}, so the trigger can ` +
            `never be fired (bot at ${fmt(bot.entity.position)})`,
        );
      }
      const here = bot.entity.position;
      await bot.lookAt(
        here.offset(acquired.aim.x - here.x, acquired.aim.y - here.y, acquired.aim.z - here.z),
        true,
      );
      process.stderr.write(
        `[trigger] ${step.trigger}: ${step.on === "use" ? "right-clicking" : "striking"} ` +
          `the hitbox at ${fmt(target.position)}\n`,
      );
      if (step.on === "use") {
        await bot.activateEntity(target);
      } else {
        bot.attack(target);
      }
    }
    if (performed > 0) {
      const arrived = await this.waitFor(
        () => (this.markerArrivals.get(step.trigger) ?? 0) > arrivalsBefore,
        OBJECTIVE_TIMEOUT_MS,
        SCORE_POLL_MS,
      );
      if (!arrived) {
        throw new Error(
          `${label}: performance ${performed + 1} broadcast no fresh ` +
            `\`${markerLine(this.campaignId ?? "?", step.trigger)}\` marker within ` +
            `${OBJECTIVE_TIMEOUT_MS}ms (${arrivalsBefore} seen before it); bot at ` +
            `${fmt(bot.entity.position)}`,
        );
      }
    } else {
      await this.awaitObjectiveMarker(step.trigger, label);
    }
    await delay(EFFECT_SETTLE_MS);
  },
};
