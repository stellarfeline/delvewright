import { test } from "node:test";
import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import type { Bot } from "mineflayer";
import { MineflayerExecutor, type BotConfig } from "../src/executor.ts";
import { BotDeathError } from "../src/death.ts";
import type { AssertCompleteStep } from "../src/critical-path.ts";

// Minimal Vec3 stand-in with the methods the executor reads off bot.entity.position.
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
  distanceTo(o: { x: number; y: number; z: number }): number {
    return Math.hypot(this.x - o.x, this.y - o.y, this.z - o.z);
  }
}

// A fake mineflayer Bot: an EventEmitter with just the surface the executor touches
// (entity.position, game.gameMode, username, pathfinder.stop). Cast to Bot at the
// attach seam — tests may use structural fakes the full type can't express.
class FakeBot extends EventEmitter {
  username = "delve-bot";
  entity = { position: new FakeVec3(0, 64, 0) };
  game = { gameMode: "adventure" as "adventure" | "spectator" };
  pathfinderStops = 0;
  pathfinder = { stop: (): void => void (this.pathfinderStops += 1) };
  loadPlugin(): void {}
}

const CONFIG: BotConfig = {
  host: "127.0.0.1",
  port: 25565,
  username: "delve-bot",
  version: "1.21.11",
  auth: "offline",
};

function attach(bot: FakeBot, env: Record<string, string | undefined> = {}): MineflayerExecutor {
  const executor = new MineflayerExecutor(CONFIG, env);
  executor.attachBot(bot as unknown as Bot);
  return executor;
}

test("a death event records position + likely cause and stops the pathfinder", () => {
  const bot = new FakeBot();
  bot.entity.position = new FakeVec3(12.4, 65, -3.6);
  const executor = attach(bot);
  // The death message arrives in chat, then the death event fires.
  bot.emit("messagestr", "delve-bot was slain by Zombie");
  bot.emit("death");

  const diag = executor.deathDiagnostic();
  assert.ok(diag instanceof BotDeathError);
  assert.deepEqual(diag.position, [12, 65, -4]); // rounded to whole blocks
  assert.equal(diag.likelyCause, "delve-bot was slain by Zombie");
  assert.equal(bot.pathfinderStops, 1); // in-flight pathfinding aborted
});

test("a death fails an in-flight assert-complete fast with the death diagnostic", async () => {
  const bot = new FakeBot();
  const executor = attach(bot);
  const step: AssertCompleteStep = {
    action: "assert-complete",
    objective: "dw.campaign",
    value: 1,
  };
  // The objective never completes; without the death check this would poll for the
  // full settle window. The death makes it reject promptly instead.
  bot.emit("messagestr", "delve-bot fell from a high place");
  bot.emit("death");
  await assert.rejects(
    () => executor.assertComplete(step),
    (err: unknown) => err instanceof BotDeathError && /high place/.test(err.message),
  );
});

test("awaitCutscene waits out the cutscene and returns once control is restored", async () => {
  const bot = new FakeBot();
  bot.game.gameMode = "spectator";
  bot.entity.position = new FakeVec3(0, 100, 0); // flying camera
  const executor = attach(bot);
  // Restore adventure control shortly after the wait begins.
  setTimeout(() => {
    bot.game.gameMode = "adventure";
    bot.entity.position = new FakeVec3(8, 65, 8); // teleported back, then still
  }, 150);
  const startedInSpectator = bot.game.gameMode;
  await executor.awaitCutscene(0);
  assert.equal(startedInSpectator, "spectator");
  assert.equal(bot.game.gameMode, "adventure"); // control confirmed returned
});

test("awaitCutscene is bounded: it continues (does not hang) if control never returns", async () => {
  const bot = new FakeBot();
  bot.game.gameMode = "spectator"; // never restored
  // Small grace so the bounded give-up path resolves quickly.
  const executor = attach(bot, { DELVEWRIGHT_CUTSCENE_GRACE_MS: "150" });
  await executor.awaitCutscene(0); // resolves (logs + continues), never throws/hangs
  assert.equal(bot.game.gameMode, "spectator");
});

test("awaitCutscene aborts fast if the bot dies during the cutscene", async () => {
  const bot = new FakeBot();
  const executor = attach(bot);
  setTimeout(() => {
    bot.emit("messagestr", "delve-bot was slain by Warden");
    bot.emit("death");
  }, 50);
  await assert.rejects(
    () => executor.awaitCutscene(5), // would otherwise sleep ~5s
    (err: unknown) => err instanceof BotDeathError && /Warden/.test(err.message),
  );
});
