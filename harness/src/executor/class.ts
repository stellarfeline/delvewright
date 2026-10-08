// MineflayerExecutor: class selection and the kit: select, equip, re-arm after a respawn.

import type { SelectClassStep } from "../critical-path.ts";
import { delay } from "./connection.ts";
import { SPAWN_POLL_MS } from "./death.ts";
import type { MineflayerExecutor } from "../executor.ts";

/** Settle time (ms) after class selection (teleport + kit give) before moving on. */
const CLASS_SETTLE_MS = 3_000;

/**
 * How long the post-respawn re-arm waits for the kept kit to arrive on the wire.
 *
 * A respawn keeps the kit — the compiler seals `gamerule keep_inventory true` in
 * every build — but the inventory is re-sent to the client a few ticks after the
 * spawn packet, so an immediate read sees an empty bag that is only empty yet.
 * The wait returns the instant an item shows up; only a bag still empty at the
 * deadline is a bag that lost its kit.
 */
const KIT_SETTLE_MS = 3_000;

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  async selectClass(this: MineflayerExecutor, step: SelectClassStep): Promise<void> {
    const bot = this.requireBot();
    // Deliberately NOT remembered for replay. `class_apply_<class>` ends in
    // `teleport @s <campaign entry point>`, so re-running this after a death moves
    // the bot off the very respawn point the die-retry stage exists to measure.
    // The post-death re-arm is `rearmAfterRespawn`, which only puts
    // the kept kit back on.
    // The class-selection dialog button runs `step.command` (a `/trigger`); the
    // bot fires the same command directly. The per-tick handler then applies the
    // kit and teleports the player to the campaign spawn.
    bot.chat(step.command);
    // Give the datapack a tick to reset the trigger, give the kit and teleport.
    await delay(CLASS_SETTLE_MS);
    // Equip the kit (sword + armor) so the bot can fight v0.3 combat waves. A
    // no-op for kits without those items.
    await this.equipLoadout();
    // The baseline every later `keep_inventory` judgement is made against: what a
    // living, kitted bot carries.
    this.itemsBeforeDeath = bot.inventory.items().length;
  },

  /**
   * Equip the best weapon and each armour piece from the current inventory. Item
   * names are matched by substring (`sword`, `helmet`, …). Best-effort per slot.
   */
  async equipLoadout(this: MineflayerExecutor): Promise<void> {
    const bot = this.requireBot();
    const slots: ReadonlyArray<[string, "hand" | "head" | "torso" | "legs" | "feet"]> = [
      ["sword", "hand"],
      ["helmet", "head"],
      ["chestplate", "torso"],
      ["leggings", "legs"],
      ["boots", "feet"],
    ];
    for (const [key, dest] of slots) {
      const item = bot.inventory.items().find((i) => i.name.includes(key));
      if (item) {
        try {
          await bot.equip(item, dest);
        } catch {
          // best effort — a missing slot is not a failure
        }
      }
    }
  },

  /**
   * Ready the bot to fight again after a respawn, without teleporting it.
   *
   * `keep_inventory` means the kit is still in the bag; what a respawn does clear
   * is the *equipped* state a client tracks, so the loadout is put back on. Returns
   * whether the kit survived: a bag that carried items into the death and is still
   * empty {@link KIT_SETTLE_MS} after the respawn lost them, which breaks the retry
   * loop for a human player exactly as it breaks it for the bot.
   */
  async rearmAfterRespawn(this: MineflayerExecutor): Promise<boolean> {
    const kept = await this.awaitKeptKit();
    if (!kept) {
      process.stderr.write(
        `[death] the bot came back EMPTY-HANDED: it carried items into the death and the ` +
          `bag is still empty ${KIT_SETTLE_MS}ms after the respawn. The delve seals ` +
          `\`gamerule keep_inventory true\`, so a lost kit means that seal is not in force\n`,
      );
    }
    await this.equipLoadout();
    return kept;
  },

  /**
   * Wait for the kept inventory to arrive, bounded. `true` the moment an item is
   * visible (and immediately for a bot that carried nothing into the death — there
   * is nothing to keep, so nothing was lost).
   */
  async awaitKeptKit(this: MineflayerExecutor): Promise<boolean> {
    const bot = this.requireBot();
    if (this.itemsBeforeDeath === 0) return true;
    const deadline = Date.now() + KIT_SETTLE_MS;
    for (;;) {
      if (bot.inventory.items().length > 0) return true;
      if (Date.now() >= deadline) return false;
      await delay(SPAWN_POLL_MS);
    }
  },
};
