// MineflayerExecutor: a collect step: picking up a drop.

import type { Bot } from "mineflayer";
import type { Entity } from "prismarine-entity";
import type { CollectHolder, CollectStep, Vec3Tuple } from "../critical-path.ts";
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

/**
 * The holder entity standing at a `collect` step's cell (spec-0111): the
 * nearest body of the step's kind within two blocks of the cell's centre. The
 * client cannot read entity tags on 1.21.11, so the body is found by kind and
 * place — the compiler put exactly one holder at that cell (DW0431 refuses two).
 */
export function holderAt(bot: Bot, holder: CollectHolder, pos: Vec3Tuple): Entity | undefined {
  const name = holder.kind.replace(/-/g, "_");
  let best: { e: Entity; d: number } | undefined;
  for (const e of Object.values(bot.entities)) {
    if (!e?.position || e.name !== name) continue;
    const d = Math.hypot(
      e.position.x - (pos[0] + 0.5),
      e.position.y - (pos[1] + 0.5),
      e.position.z - (pos[2] + 0.5),
    );
    if (d > 2) continue;
    if (!best || d < best.d) best = { e, d };
  }
  return best?.e;
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
    if (step.holder !== undefined) {
      await this.takeFromHolder(step, step.holder);
      return;
    }
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
   * Take a `collect`'s item off the frame or stand standing at its cell
   * (spec-0111 §5.4), the way a player does: walk to the cell the compiler
   * proved in reach and in sight, then HIT a frame — its item pops out and is
   * picked up as any drop — or CLICK a stand with an empty hand at the height
   * whose band holds the slot (`activateEntityAt`, mineflayer's `use_entity`
   * with `mouse: 2`, the packet the server reads as `interact_at`; the swap
   * that puts the stack in the hand happens only on that packet, so the
   * objective completing is the server having received it). After a frame
   * take the bot asserts the frame still hangs there and no frame item came
   * into its inventory — the §4.3 freeze, observed once from outside.
   */
  async takeFromHolder(
    this: MineflayerExecutor,
    step: CollectStep,
    holder: CollectHolder,
  ): Promise<void> {
    const bot = this.requireBot();
    await this.walkTo(holder.stance ?? step.pos, holder.stance ? 0 : 2, `${holder.kind} ${step.item}`, step.sneak, {
      objective: step.objective,
      transport: step.transport,
    });
    const body = holderAt(bot, holder, step.pos);
    if (!body) {
      throw new Error(
        `no ${holder.kind} within 2 blocks of the collect cell [${step.pos.join(", ")}] for ${step.objective}`,
      );
    }
    if (holder.kind === "armor-stand") {
      if (bot.heldItem) {
        await bot.unequip("hand");
      }
      const at = body.position.offset(0, holder.clickHeight ?? 1, 0);
      await bot.activateEntityAt(body, at);
      await this.requireObjective(step.objective, `collect ${step.item}`);
      return;
    }
    await bot.lookAt(body.position, true);
    bot.attack(body);
    await this.pickUpDrop(step);
    await this.requireObjective(step.objective, `collect ${step.item}`);
    const still = bot.entities[body.id];
    if (!still || !still.isValid) {
      throw new Error(
        `the ${holder.kind} at [${step.pos.join(", ")}] is gone after ${step.objective}'s take — ` +
          "an emptied frame must freeze (spec-0111 §4.3), not break",
      );
    }
    const frameItem = bot.inventory
      .items()
      .find((i) => i.name === "item_frame" || i.name === "glow_item_frame");
    if (frameItem) {
      throw new Error(
        `a ${frameItem.name} came into the bot's inventory taking ${step.objective}: ` +
          "the frame broke instead of freezing (spec-0111 §4.3)",
      );
    }
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
