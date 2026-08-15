import { test } from "node:test";
import assert from "node:assert/strict";
import { presentAndTrigger, type HandItem, type InteractBot } from "../src/held-item.ts";

/**
 * A bot that records WHAT it was told to do and IN WHICH ORDER — the order is the
 * whole assertion: the datapack reads the mainhand on the tick it consumes the
 * `/trigger`, so an equip that lands after the chat is worth nothing.
 */
function fakeBot(carried: string[], opts: { equipFails?: boolean } = {}): InteractBot & {
  calls: string[];
} {
  const calls: string[] = [];
  const items: HandItem[] = carried.map((name) => ({ name }));
  return {
    calls,
    inventory: { items: () => items },
    equip: async (item: HandItem, destination: "hand"): Promise<void> => {
      calls.push(`equip(${item.name},${destination})`);
      if (opts.equipFails) throw new Error("server refused the window click");
    },
    chat: (message: string): void => {
      calls.push(`chat(${message})`);
    },
  };
}

test("a requires_item step equips the item BEFORE chatting the trigger", async () => {
  // `requires_item` is mainhand-held: a bot that only carries the item has every
  // trigger swallowed by the guard.
  const bot = fakeBot(["stone_sword", "tripwire_hook"]);
  await presentAndTrigger(
    bot,
    { requiresItem: "minecraft:tripwire_hook", command: "/trigger dw.i.unbar" },
    "anchor/gate",
  );
  assert.deepEqual(bot.calls, ["equip(tripwire_hook,hand)", "chat(/trigger dw.i.unbar)"]);
});

test("a step with no requires_item never touches the hand", async () => {
  // The loadout put a sword there; a step that asked for nothing must not disarm the
  // bot on its way to the next fight.
  const bot = fakeBot(["stone_sword", "tripwire_hook"]);
  await presentAndTrigger(bot, { requiresItem: null, command: "/trigger dw.i.lever" }, "anchor/lever");
  assert.deepEqual(bot.calls, ["chat(/trigger dw.i.lever)"]);
});

test("the required item is matched exactly, with the namespace stripped", async () => {
  // mineflayer item names are unnamespaced; a substring match would equip an
  // `iron_sword` for a required `sword`, which the mainhand guard would then reject.
  const bot = fakeBot(["iron_trapdoor", "trial_key"]);
  await presentAndTrigger(
    bot,
    { requiresItem: "minecraft:trial_key", command: "/trigger dw.i.unbar" },
    "anchor/vault",
  );
  assert.deepEqual(bot.calls, ["equip(trial_key,hand)", "chat(/trigger dw.i.unbar)"]);
});

test("a required item the bot does not carry still sends the trigger and fails on the guard", async () => {
  // Never a check: the harness does not decide that the step is doomed, it reports
  // what it saw and lets the objective marker (which will not arrive) fail the step.
  const bot = fakeBot(["stone_sword"]);
  await presentAndTrigger(
    bot,
    { requiresItem: "minecraft:trial_key", command: "/trigger dw.i.unbar" },
    "anchor/vault",
  );
  assert.deepEqual(bot.calls, ["chat(/trigger dw.i.unbar)"]);
});

test("a failed equip is reported, not thrown — the objective marker stays the arbiter", async () => {
  const bot = fakeBot(["trial_key"], { equipFails: true });
  await presentAndTrigger(
    bot,
    { requiresItem: "minecraft:trial_key", command: "/trigger dw.i.unbar" },
    "anchor/vault",
  );
  assert.deepEqual(bot.calls, ["equip(trial_key,hand)", "chat(/trigger dw.i.unbar)"]);
});
