// Executor-level wiring for self-defense and eating (souls ladder, the-drowned-bell).
// The selection RULES are unit-tested in threat.test.ts / sustain.test.ts; these tests
// pin the parts that only exist against a bot: which mineflayer event feeds the damage
// attribution, which entities may ever be blamed, and that a hurt bot actually eats
// (and goes back to holding its sword).

import { test } from "node:test";
import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import type { Bot } from "mineflayer";
import { MineflayerExecutor, type BotConfig } from "../src/executor.ts";

/**
 * The attack speed the fake server sends: 20 swings a second, so the bot's
 * full-charge wait (`fullChargeMs`) is two ticks and a test of the fight's LOGIC
 * does not sit through a sword's real cooldown. The cadence itself is
 * `melee.test.ts`'s subject.
 */
const FAKE_WEAPON = { "generic.attack_speed": { value: 20, modifiers: [] } };

class FakeVec3 {
  readonly x: number;
  readonly y: number;
  readonly z: number;
  constructor(x: number, y: number, z: number) {
    this.x = x;
    this.y = y;
    this.z = z;
  }
  clone(): FakeVec3 {
    return new FakeVec3(this.x, this.y, this.z);
  }
  offset(dx: number, dy: number, dz: number): FakeVec3 {
    return new FakeVec3(this.x + dx, this.y + dy, this.z + dz);
  }
  distanceTo(o: { x: number; y: number; z: number }): number {
    return Math.hypot(this.x - o.x, this.y - o.y, this.z - o.z);
  }
}

interface FakeEntity {
  id: number;
  name: string;
  /** The registry category mineflayer copies onto a tracked entity. */
  type: string;
  height: number;
  position: FakeVec3;
}

function mob(id: number, name: string, distance: number): FakeEntity {
  return { id, name, type: "hostile", height: 1.95, position: new FakeVec3(distance, 64, 0) };
}

const RABBIT_STEW = { type: 900, name: "rabbit_stew", count: 1 };
const SWORD = { type: 700, name: "iron_sword", count: 1 };

class FakeBot extends EventEmitter {
  username = "delve-bot";
  entity = { id: 1, position: new FakeVec3(0, 64, 0), onGround: true, attributes: FAKE_WEAPON };
  game = { gameMode: "adventure" as const };
  health = 20;
  food = 14;
  entities: Record<number, FakeEntity> = {};
  /** Pinned minecraft-data shape: food items keyed by item type id. */
  registry = { foods: { 900: { foodPoints: 10 } } as Record<number, { foodPoints: number }> };
  inventoryItems: Array<{ type: number; name: string; count: number; components?: unknown[] }> = [
    SWORD,
    RABBIT_STEW,
  ];
  inventory = {
    items: (): Array<{ type: number; name: string; count: number }> => this.inventoryItems,
    slots: [] as Array<{ name: string } | undefined>,
  };
  equips: Array<[string, string]> = [];
  consumed = 0;
  pathfinder = { stop: (): void => {} };
  loadPlugin(): void {}
  clearControlStates(): void {}
  async equip(item: { name: string }, dest: string): Promise<void> {
    this.equips.push([item.name, dest]);
  }
  async consume(): Promise<void> {
    this.consumed += 1;
    this.health = Math.min(20, this.health + 6);
    this.food = Math.min(20, this.food + 10);
    this.inventoryItems = this.inventoryItems.filter((i) => i !== RABBIT_STEW);
  }
}

const CONFIG: BotConfig = {
  host: "127.0.0.1",
  port: 25565,
  username: "delve-bot",
  version: "1.21.11",
  auth: "offline",
};

function attach(bot: FakeBot): MineflayerExecutor {
  const executor = new MineflayerExecutor(CONFIG, {});
  // What `run.ts` does before anything can fight, off the path's `non_combatants`.
  // The executor refuses to classify a body without it — see the refusal case at
  // the bottom of this file.
  executor.useNonCombatants(new Set(["mannequin", "villager"]));
  executor.attachBot(bot as unknown as Bot);
  return executor;
}

test("a damage packet naming its source records that entity as an attacker", () => {
  const bot = new FakeBot();
  const husk = mob(42, "husk", 2);
  bot.entities[husk.id] = husk;
  const executor = attach(bot);
  // mineflayer re-emits the 1.20+ `damage_event` packet as entityHurt(entity, source).
  bot.emit("entityHurt", bot.entity, husk);
  assert.deepEqual(
    executor.recentAttackers().map((a) => [a.id, a.hits]),
    [[42, 1]],
  );
});

test("damage to some OTHER entity is not the bot's business", () => {
  const bot = new FakeBot();
  const husk = mob(42, "husk", 2);
  const vindicator = mob(43, "vindicator", 3);
  bot.entities[husk.id] = husk;
  bot.entities[vindicator.id] = vindicator;
  const executor = attach(bot);
  // The bot hitting a mob must never make that mob "an attacker".
  bot.emit("entityHurt", husk, bot.entity);
  assert.deepEqual(executor.recentAttackers(), []);
});

test("a health drop with no named source is blamed on the nearest hostile in reach", () => {
  const bot = new FakeBot();
  const husk = mob(42, "husk", 2);
  bot.entities[husk.id] = husk;
  const executor = attach(bot);
  bot.emit("health"); // establish the baseline (20)
  bot.health = 14;
  bot.emit("health");
  assert.deepEqual(
    executor.recentAttackers().map((a) => a.id),
    [42],
  );
});

test("a hazard with nothing in reach blames nobody (no phantom retaliation)", () => {
  const bot = new FakeBot();
  bot.entities[42] = mob(42, "husk", 20); // across the room
  const executor = attach(bot);
  bot.emit("health");
  bot.health = 8; // a fall, a trap, drowning
  bot.emit("health");
  assert.deepEqual(executor.recentAttackers(), []);
});

test("an NPC mannequin is never recorded as an attacker", () => {
  const bot = new FakeBot();
  // Every Delvewright NPC is an Invulnerable mannequin standing right where the
  // player fights; it must never become a defense target.
  bot.entities[7] = mob(7, "mannequin", 1);
  const executor = attach(bot);
  bot.emit("health");
  bot.health = 15;
  bot.emit("health");
  assert.deepEqual(executor.recentAttackers(), []);
  // Nor via a (nonsensical) named source.
  bot.emit("entityHurt", bot.entity, bot.entities[7]);
  assert.deepEqual(executor.recentAttackers(), []);
});

test("a hurt bot eats its kit stew and goes back to holding the sword", async () => {
  const bot = new FakeBot();
  bot.health = 7; // below 60% of 20
  const executor = attach(bot);
  await executor.maybeEat("test leg");
  assert.equal(bot.consumed, 1);
  assert.ok(bot.health > 7, "eating healed the bot");
  assert.deepEqual(bot.equips[0], ["rabbit_stew", "hand"]);
  assert.ok(
    bot.equips.some(([name, dest]) => name === "iron_sword" && dest === "hand"),
    "the sword is re-equipped after the meal",
  );
});

test("a hurt bot with a mob in its face fights instead of eating", async () => {
  const bot = new FakeBot();
  bot.health = 7;
  bot.entities[42] = mob(42, "husk", 2);
  const executor = attach(bot);
  await executor.maybeEat("test leg");
  assert.equal(bot.consumed, 0);
});

test("a healthy bot never stops to eat", async () => {
  const bot = new FakeBot();
  bot.health = 20;
  const executor = attach(bot);
  await executor.maybeEat("test leg");
  assert.equal(bot.consumed, 0);
  assert.deepEqual(bot.equips, []);
});

// --- the delve's cast is required, never assumed --------------------------------

test("an executor never told the delve's cast refuses to classify a body", () => {
  // The refusal IS the repair. Before this, the harness carried its own list of
  // entity names, so a run against a campaign whose author bodied an NPC
  // differently classified a quest-giver as a fight and nothing said so.
  const bot = new FakeBot();
  bot.entities[42] = mob(42, "husk", 2);
  const executor = new MineflayerExecutor(CONFIG, {});
  executor.attachBot(bot as unknown as Bot);
  assert.throws(
    () => bot.emit("entityHurt", bot.entity, bot.entities[42]),
    /never told which entity kinds are never a combat target/,
  );
});

test("a delve that stages no NPC states an EMPTY cast, which is not the same as none", () => {
  // An empty set is a legitimate answer and the executor takes it: nothing here
  // is exempt by cast, and the vanilla exclusions still apply.
  const bot = new FakeBot();
  const husk = mob(42, "husk", 2);
  bot.entities[husk.id] = husk;
  const executor = new MineflayerExecutor(CONFIG, {});
  executor.useNonCombatants(new Set<string>());
  executor.attachBot(bot as unknown as Bot);
  bot.emit("entityHurt", bot.entity, husk);
  assert.deepEqual(
    executor.recentAttackers().map((a) => a.id),
    [42],
  );
});

// --- drinking a healing draught, the way a player drinks it --------------------

/** A Vigil Draught as the server describes it: Potion of Healing II (registry id 25). */
function draught(): { type: number; name: string; count: number; components: unknown[] } {
  return {
    type: 1000,
    name: "potion",
    count: 1,
    components: [{ type: "potion_contents", data: { potionId: 25, customEffects: [] } }],
  };
}

/**
 * A bot whose server finishes a drink the way vanilla does: the use key is held,
 * 32 ticks later the entity event 9 arrives on the bot, the bottle is gone and the
 * heal has landed. `consume()` — mineflayer's shortcut, which a blow mid-drink
 * resolves early — must never be the path.
 */
class DrinkBot extends FakeBot {
  _client = new EventEmitter();
  heldName: string | undefined;
  used: string[] = [];
  setControlState(): void {}
  override async equip(item: { name: string }, dest: string): Promise<void> {
    await super.equip(item, dest);
    if (dest === "hand") this.heldName = item.name;
  }
  activateItem(offHand = false): void {
    this.used.push(offHand ? "off-hand" : `hand:${this.heldName}`);
    if (offHand || this.heldName !== "potion") return;
    setTimeout(() => {
      const i = this.inventoryItems.findIndex((it) => it.name === "potion");
      if (i >= 0) this.inventoryItems.splice(i, 1);
      this.health = Math.min(20, this.health + 8);
      this._client.emit("entity_status", { entityId: 1, entityStatus: 9 });
    }, 60);
  }
  deactivateItem(): void {
    this.used.push("release");
  }
}

test("a hurt bot drinks a healing draught with the use key, and it lands", async () => {
  const bot = new DrinkBot();
  bot.health = 9;
  bot.inventoryItems = [SWORD, draught(), draught()];
  const executor = attach(bot);
  await executor.maybeDrink("test fight");
  assert.equal(bot.health, 17);
  assert.equal(bot.inventoryItems.filter((i) => i.name === "potion").length, 1);
  assert.equal(bot.consumed, 0, "never through mineflayer's consume()");
  assert.deepEqual(bot.used, ["hand:potion"]);
  assert.ok(
    bot.equips.some(([name, dest]) => name === "iron_sword" && dest === "hand"),
    "the sword is back in hand after the drink",
  );
});

test("a draught is not wasted on a scratch", async () => {
  const bot = new DrinkBot();
  bot.health = 13; // 7 missing, the draught heals 8
  bot.inventoryItems = [SWORD, draught()];
  const executor = attach(bot);
  await executor.maybeDrink("test fight");
  assert.deepEqual(bot.used, []);
  assert.equal(bot.health, 13);
});

test("a draught waits while a melee attacker is on the bot", async () => {
  const bot = new DrinkBot();
  bot.health = 9;
  bot.inventoryItems = [SWORD, draught()];
  bot.entities[43] = mob(43, "vindicator", 2);
  const executor = attach(bot);
  await executor.maybeDrink("test fight");
  assert.deepEqual(bot.used, []);
});

test("a bottle of harming is never drunk", async () => {
  const bot = new DrinkBot();
  bot.health = 6;
  bot.inventoryItems = [
    SWORD,
    { ...draught(), components: [{ type: "potion_contents", data: { potionId: 26 } }] },
  ];
  const executor = attach(bot);
  await executor.maybeDrink("test fight");
  assert.deepEqual(bot.used, []);
});

test("at a third of max health the draught is drunk with the attacker on the bot", async () => {
  const bot = new DrinkBot();
  bot.health = 5.6;
  bot.inventoryItems = [SWORD, draught()];
  bot.entities[43] = mob(43, "zombie", 2);
  const executor = attach(bot);
  await executor.maybeDrink("test fight");
  assert.deepEqual(bot.used, ["hand:potion"]);
  assert.equal(bot.health, 13.6);
});
