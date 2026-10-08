// MineflayerExecutor: a collect step: picking up a drop.

import type { Bot } from "mineflayer";
import type { Entity } from "prismarine-entity";
import type { CollectStep, Vec3Tuple } from "../critical-path.ts";
import { BotDeathError } from "../death.ts";
import { WAVE_ENGAGE_NEAR } from "../wave.ts";
import { delay, fmt } from "./connection.ts";
import { OBJECTIVE_TIMEOUT_MS } from "./objective.ts";
import { REACH_POLL_MS, UNSTICK_BURST_MS } from "./walk.ts";
import type { MineflayerExecutor } from "../executor.ts";

/**
 * How far from a `collect` step's anchor a drop is still this fight's.
 *
 * `WAVE_ENGAGE_NEAR`'s 32, because a drop lies where the BODY fell and a wave
 * body chases the party across a room before it does. Narrowed to 12 by
 * inspection, the bell `wave/drowned-choir`'s Precentor leaves was outside the
 * search and `obj/take-the-tongue` timed out on vesperhold.
 */
const DROP_SEARCH_NEAR = WAVE_ENGAGE_NEAR;

/**
 * The dropped `item` (unnamespaced id) nearest `anchor` within `radius` blocks,
 * off the item entities the client tracks, or `undefined` when none is in sight.
 */
function nearestDrop(
  bot: Bot,
  item: string,
  anchor: readonly [number, number, number],
  radius: number,
): { id: number; position: Entity["position"]; fromAnchor: number } | undefined {
  let best: { id: number; position: Entity["position"]; fromAnchor: number } | undefined;
  for (const e of Object.values(bot.entities)) {
    if (!e?.position || e.name !== "item") continue;
    if (e.getDroppedItem()?.name !== item) continue;
    const fromAnchor = Math.hypot(
      e.position.x - (anchor[0] + 0.5),
      e.position.y - anchor[1],
      e.position.z - (anchor[2] + 0.5),
    );
    if (fromAnchor > radius) continue;
    if (!best || fromAnchor < best.fromAnchor) best = { id: e.id, position: e.position, fromAnchor };
  }
  return best;
}

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  /**
   * Collect items from the chest at the anchor: go there, open it, withdraw all.
   *
   * A **drop-gated** collect (v0.9 `dropped_by`) has no chest to open — the
   * compiler places none, because the item exists only after the fight. The drop
   * lies where the body FELL, which is wherever the fight took it, not the anchor
   * the wave was seated on: the vesperhold Porter died seven blocks from
   * `anchor/porter`, and a bot that walked to the anchor and waited there timed
   * out beside a key a player would simply have picked up. So the bot walks to
   * the fight's ground, then goes to the dropped item it can SEE and lets vanilla
   * pickup do the rest (`pickUpDrop`); the proof is the same one every collect
   * uses, the objective's own marker.
   */
  async collect(this: MineflayerExecutor, step: CollectStep): Promise<void> {
    const bot = this.requireBot();
    if (step.droppedBy !== undefined) {
      await this.walkTo(step.pos, 1, `drop of ${step.item}`, step.sneak, {
        objective: step.objective,
        transport: step.transport,
      });
      await this.pickUpDrop(step);
      await this.requireObjective(step.objective, `collect ${step.item}`);
      return;
    }
    await this.walkTo(step.pos, 2, `chest ${step.item}`, step.sneak, {
      objective: step.objective,
      transport: step.transport,
    });
    const here = bot.entity.position;
    const target = here.offset(
      step.pos[0] + 0.5 - here.x,
      step.pos[1] + 0.5 - here.y,
      step.pos[2] + 0.5 - here.z,
    );
    const block = bot.blockAt(target);
    if (!block) {
      throw new Error(`no block at collect anchor [${step.pos.join(", ")}]`);
    }
    const chest = await bot.openContainer(block);
    try {
      for (const item of chest.containerItems()) {
        await chest.withdraw(item.type, null, item.count);
      }
    } finally {
      chest.close();
    }
    // Holding the items is not the objective; the inventory_changed advancement
    // completing it is. Wait for that objective's own marker.
    await this.requireObjective(step.objective, `collect ${step.item}`);
  },

  /**
   * Walk onto the dropped `step.item` nearest the fight's anchor until the
   * objective completes, the drop is gone, or the objective's own budget runs out.
   *
   * What a player does: look at the floor of the room the fight was in, see the
   * item, walk over it. The search is bounded by {@link WAVE_ENGAGE_NEAR}, the
   * radius inside which this harness already counts a body as still part of a
   * fight — a drop outside it is not one this fight left. Seeing none is not a failure here:
   * the objective's marker decides, and its timeout names the step.
   */
  async pickUpDrop(this: MineflayerExecutor, step: CollectStep): Promise<void> {
    const bot = this.requireBot();
    const want = step.item.replace(/^minecraft:/, "");
    const deadline = Date.now() + OBJECTIVE_TIMEOUT_MS;
    let reported = false;
    while (Date.now() < deadline && !this.completedObjectives.has(step.objective)) {
      if (this.death) throw this.death;
      const drop = nearestDrop(bot, want, step.pos, DROP_SEARCH_NEAR);
      if (drop === undefined) {
        await delay(REACH_POLL_MS);
        continue;
      }
      if (!reported) {
        reported = true;
        process.stderr.write(
          `[collect ${step.objective}] the ${want} lies at ${fmt(drop.position)}, ` +
            `${drop.fromAnchor.toFixed(1)} blocks from the anchor — walking onto it\n`,
        );
      }
      const cell: Vec3Tuple = [
        Math.floor(drop.position.x),
        Math.floor(drop.position.y),
        Math.floor(drop.position.z),
      ];
      try {
        await this.walkTo(cell, 1, `drop of ${step.item}`, step.sneak);
      } catch (err) {
        if (err instanceof BotDeathError) throw err;
        process.stderr.write(
          `[collect ${step.objective}] could not walk to the ${want}: ` +
            `${err instanceof Error ? err.message : String(err)}\n`,
        );
        return;
      }
      // The pathfinder stops within a block; vanilla picks up an item the player's
      // box, grown by one block sideways, touches. Close the last step by hand.
      const live = bot.entities[drop.id];
      if (live?.position && !this.completedObjectives.has(step.objective)) {
        await bot.lookAt(live.position, true).catch(() => {});
        bot.setControlState("forward", true);
        await delay(UNSTICK_BURST_MS);
        bot.setControlState("forward", false);
      }
      await delay(REACH_POLL_MS);
    }
  },
};
