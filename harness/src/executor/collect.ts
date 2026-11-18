// MineflayerExecutor: a collect step: picking up a drop.

import type { Bot } from "mineflayer";
import type { Entity } from "prismarine-entity";
import type { CollectHolder, CollectStep, GatheredUse, Vec3Tuple } from "../critical-path.ts";
import { INTERACTION_REACH, acquireFromStances, occlusionFailure } from "../crosshair.ts";
import { BotDeathError } from "../death.ts";
import { WAVE_ENGAGE_NEAR } from "../wave.ts";
import { delay, fmt } from "./connection.ts";
import { OBJECTIVE_TIMEOUT_MS } from "./objective.ts";
import { REACH_POLL_MS, UNSTICK_BURST_MS } from "./walk.ts";
import type { MineflayerExecutor } from "../executor.ts";

/**
 * How far from a search centre ({@link nearestDropOf}) a drop is still this
 * fight's: `WAVE_ENGAGE_NEAR`'s 32, because a wave body chases the party
 * across a room before it falls. The centres carry where the body was last
 * read standing, so a body that fled farther than this is still found.
 */
const DROP_SEARCH_NEAR = WAVE_ENGAGE_NEAR;

/**
 * How long after hitting a frame the bot looks for it again: three ticks, past
 * the freeze `tick` applies the tick after the pop (spec-0111 §4.3) and short
 * of the popped item's ten-tick pickup delay.
 */
const FRAME_CHECK_MS = 150;

/**
 * How long a collect waits for an opened container's contents to reach the
 * client before it reads the window as empty: the open resolves on the
 * window's opening, and the stack can arrive after it.
 */
const WINDOW_CONTENTS_MS = 3000;

/** One item entity as the drop search reads it: its id, where it lies, and
 * the unnamespaced id of the stack it carries. */
export interface SeenDrop {
  readonly id: number;
  readonly position: { readonly x: number; readonly y: number; readonly z: number };
  readonly item: string | undefined;
}

/** A dropped stack the search chose, and its distance from the nearest centre. */
export interface FoundDrop {
  readonly id: number;
  readonly position: SeenDrop["position"];
  /** Blocks from the nearest search centre. */
  readonly from: number;
  /** Index into the centres it was measured from: 0 is the step's anchor. */
  readonly centre: number;
}

/**
 * **Where a drop is looked for** (pure): the dropped `item` nearest any of
 * `centres` within `radius` blocks of it, or `undefined`. The centres are the
 * step's anchor cell (its centre) first, then every position the server last
 * read a body of the dropping wave standing at — a drop lies where the body
 * FELL, and a body that fled the anchor (a struck hen, a wave left alone
 * through a die-retry trial) falls where it fled to.
 */
export function nearestDropOf(
  drops: Iterable<SeenDrop>,
  item: string,
  anchor: readonly [number, number, number],
  stood: readonly (readonly [number, number, number])[],
  radius: number,
): FoundDrop | undefined {
  const centres: (readonly [number, number, number])[] = [
    [anchor[0] + 0.5, anchor[1], anchor[2] + 0.5],
    ...stood,
  ];
  let best: FoundDrop | undefined;
  for (const d of drops) {
    if (d.item !== item) continue;
    for (const [i, c] of centres.entries()) {
      const from = Math.hypot(d.position.x - c[0], d.position.y - c[1], d.position.z - c[2]);
      if (from > radius) continue;
      if (!best || from < best.from) best = { id: d.id, position: d.position, from, centre: i };
    }
  }
  return best;
}

/** The item entities the client tracks, as {@link nearestDropOf} reads them. */
function seenDrops(bot: Bot): SeenDrop[] {
  const out: SeenDrop[] = [];
  for (const e of Object.values(bot.entities)) {
    if (!e?.position || e.name !== "item") continue;
    out.push({ id: e.id, position: e.position, item: e.getDroppedItem()?.name });
  }
  return out;
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
    const seated = this.aboard;
    if (
      seated !== undefined &&
      (step.holder !== undefined || step.droppedBy !== undefined || step.gathered === true)
    ) {
      throw new Error(
        `collect ${step.item}: the bot sits in ${seated.vehicle}, and a take off a holder ` +
          "or the floor, or an item the world supplies, is done on foot (spec-0135 §5)",
      );
    }
    if (step.holder !== undefined) {
      await this.takeFromHolder(step, step.holder);
      return;
    }
    if (step.gathered === true) {
      await this.gather(step);
      return;
    }
    if (step.droppedBy !== undefined) {
      await this.walkTo(step.pos, 1, `drop of ${step.item}`, step.sneak, {
        objective: step.objective,
        transport: step.transport,
      });
      await this.atStance();
      await this.pickUpDrop(step);
      await this.requireObjective(step.objective, `collect ${step.item}`);
      return;
    }
    if (seated !== undefined) {
      // spec-0135 §5: opened from the seat, at the eye the compiler proved
      // has the container in reach and in sight.
      process.stderr.write(
        `[seat] collect ${step.item}: opening [${step.pos.join(", ")}] from ${seated.vehicle}'s seat at ` +
          `${fmt(bot.entity.position)}\n`,
      );
    } else {
      await this.walkTo(step.pos, 2, `chest ${step.item}`, step.sneak, {
        objective: step.objective,
        transport: step.transport,
      });
      await this.atStance();
    }
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
      let held = chest.containerItems();
      if (held.length === 0) {
        // The server's own reading of the block beside the client's: a window
        // that shows nothing is told apart from a container that holds nothing.
        const from = this.chatMark();
        bot.chat(
          `/tellraw @s ${JSON.stringify([{ text: "[dw:holds " }, { nbt: "Items", block: step.pos.join(" ") }, { text: "]" }])}`,
        );
        // mineflayer resolves the open on the window's opening, and its
        // contents can come after: the gallery's crate opened from a boat's
        // seat read empty, the server holding the stack, and full 600 ms on.
        const by = Date.now() + WINDOW_CONTENTS_MS;
        while (held.length === 0 && Date.now() < by) {
          await delay(REACH_POLL_MS);
          held = chest.containerItems();
        }
        process.stderr.write(
          `[collect] ${step.item}: the window showed nothing at first; the server says ` +
            `${this.chatSince(from).lines.join(" | ").slice(0, 400) || "nothing"}\n`,
        );
      }
      process.stderr.write(
        `[collect] ${step.item}: the ${block.name} at [${step.pos.join(", ")}] opened as window ` +
          `${chest.id} holding ${held.map((i) => `${i.count} ${i.name}`).join(", ") || "nothing"}\n`,
      );
      for (const item of held) {
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
   * A **gathered** collect (DSL v0.81): the world supplies the item — a fish
   * off a line, a creature in a bucket — and the engine places nothing. The
   * bot walks to where the act is done
   * and is handed the item by command there, as a census puts its bodies in:
   * the vanilla act is the playtest's, and what the build proves is that
   * possession completes the objective. The proof is the objective's marker.
   */
  async gather(this: MineflayerExecutor, step: CollectStep): Promise<void> {
    await this.walkTo(step.pos, 2, `the place ${step.item} is got`, step.sneak, {
      objective: step.objective,
      transport: step.transport,
    });
    await this.atStance();
    if (step.use !== undefined) {
      await this.useOnBody(step, step.use);
      return;
    }
    const command = `/give @s ${step.item} ${step.count}`;
    process.stderr.write(
      `[collect] ${step.item}: gathered — handed over by \`${command}\`; handed, not performed\n`,
    );
    const refused = await this.refusalOf(command);
    if (refused !== undefined) {
      throw new Error(`collect ${step.item}: handing the gathered item over was refused — ${refused}`);
    }
    await this.requireObjective(step.objective, `collect ${step.item}`);
  },

  /**
   * Perform a gathered item's act where it is a use on a body (format 17): hold
   * the use's item, put the crosshair on a body of its kind near the step's
   * place the way a player does — the first box the ray meets, from a stance
   * the step allows (`crosshair.ts`) — and right-click it. A box standing over
   * the body takes the ray first, and the step fails naming both. The proof is
   * the objective's own marker, as for every collect.
   */
  async useOnBody(this: MineflayerExecutor, step: CollectStep, use: GatheredUse): Promise<void> {
    const bot = this.requireBot();
    const bare = (id: string) => id.replace(/^minecraft:/, "");
    const what = `${step.objective}: ${use.item} on a ${use.on}`;
    for (let n = 0; n < step.count; n += 1) {
      const tool = bot.inventory.items().find((i) => i.name === bare(use.item));
      if (!tool) {
        throw new Error(`${what}: the party holds no ${use.item}, so the act cannot be performed`);
      }
      await bot.equip(tool, "hand");
      const bodies = this.hitboxesNear(step.pos, INTERACTION_REACH + 5);
      const kind = bodies
        .filter((b) => b.name === bare(use.on))
        .sort(
          (a, b) =>
            Math.hypot(a.position.x - step.pos[0] - 0.5, a.position.y - step.pos[1], a.position.z - step.pos[2] - 0.5) -
            Math.hypot(b.position.x - step.pos[0] - 0.5, b.position.y - step.pos[1], b.position.z - step.pos[2] - 0.5),
        );
      if (kind.length === 0) {
        throw new Error(`${what}: no ${use.on} tracked within ${INTERACTION_REACH + 5} blocks of [${step.pos.join(", ")}]`);
      }
      const world = this.blockShapes();
      const stances = this.stancesAround(step.pos, 2, bodies);
      let done = false;
      let last: string | undefined;
      for (const target of kind) {
        const others = bodies.filter((b) => b.id !== target.id);
        const verdict = acquireFromStances(stances, target, others, INTERACTION_REACH, world.shapes, world.names);
        if (!verdict.ok) {
          last = occlusionFailure(what, target, verdict.blockers, verdict.triedStances, verdict.blocks);
          continue;
        }
        const here = bot.entity.position;
        const eye = verdict.eye;
        if (Math.hypot(eye.x - here.x, eye.y - 1.62 - here.y, eye.z - here.z) > 0.5) {
          await this.walkTo(
            [Math.floor(eye.x), Math.round(eye.y - 1.62), Math.floor(eye.z)],
            0,
            `the stance ${what} is acquired from`,
            step.sneak,
          );
        }
        const entity = bot.entities[target.id];
        if (!entity) {
          last = `${what}: the ${use.on} #${target.id} left the client's tracking before the use`;
          continue;
        }
        await bot.lookAt(entity.position.offset(verdict.aim.x - entity.position.x, verdict.aim.y - entity.position.y, verdict.aim.z - entity.position.z), true);
        process.stderr.write(
          `[collect] ${step.item}: performed — ${use.item} used on the ${use.on} #${target.id} ` +
            `at ${fmt(entity.position)}\n`,
        );
        await bot.activateEntity(entity);
        done = true;
        break;
      }
      if (!done) throw new Error(last ?? `${what}: no ${use.on} could be acquired`);
    }
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
    // The proven leg ends at the stance (the compiler snaps a holder's leg
    // there) and is named by the holder's cell, the step's `pos`.
    await this.walkTo(
      holder.stance ?? step.pos,
      holder.stance ? 0 : 2,
      `${holder.kind} ${step.item}`,
      step.sneak,
      { objective: step.objective, transport: step.transport },
      undefined,
      step.pos,
    );
    await this.atStance();
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
    // The frame is judged while the bot still stands beside it: three ticks
    // after the hit, past the freeze (the next tick) and before the popped
    // item's ten-tick pickup delay lets the objective complete and a carry
    // take the bot out of the frame's tracking range.
    await delay(FRAME_CHECK_MS);
    const still = bot.entities[body.id];
    if (!still || !still.isValid) {
      throw new Error(
        `the ${holder.kind} at [${step.pos.join(", ")}] is gone after ${step.objective}'s hit — ` +
          "an emptied frame must freeze (spec-0111 §4.3), not break",
      );
    }
    await this.pickUpDrop(step);
    await this.requireObjective(step.objective, `collect ${step.item}`);
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
   * Walk onto the dropped `step.item` nearest the fight's anchor, or nearest
   * where the server last read a body of the dropping wave standing
   * ({@link nearestDropOf}), until the objective completes, the drop is gone,
   * or the objective's own budget runs out.
   *
   * What a player does: look at the floor where the fight ended, see the item,
   * walk over it. The search is bounded by {@link WAVE_ENGAGE_NEAR} around each
   * centre. Seeing none is not a failure here: the objective's marker decides,
   * and its timeout names the step.
   */
  async pickUpDrop(this: MineflayerExecutor, step: CollectStep): Promise<void> {
    const bot = this.requireBot();
    const want = step.item.replace(/^minecraft:/, "");
    const deadline = Date.now() + OBJECTIVE_TIMEOUT_MS;
    let reported = false;
    while (Date.now() < deadline && !this.completedObjectives.has(step.objective)) {
      if (this.death) throw this.death;
      const stood =
        step.droppedBy === undefined ? [] : (this.waveLastStood.get(step.droppedBy) ?? []);
      const drop = nearestDropOf(seenDrops(bot), want, step.pos, stood, DROP_SEARCH_NEAR);
      if (drop === undefined) {
        await delay(REACH_POLL_MS);
        continue;
      }
      if (!reported) {
        reported = true;
        const where =
          drop.centre === 0
            ? "the anchor"
            : `where the server last read ${step.droppedBy}'s body ${drop.centre} of ${stood.length} standing`;
        process.stderr.write(
          `[collect ${step.objective}] the ${want} lies at ${fmt(drop.position)}, ` +
            `${drop.from.toFixed(1)} blocks from ${where} — walking onto it\n`,
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
