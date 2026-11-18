import { test } from "node:test";
import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import type { Bot } from "mineflayer";
import { within } from "./bounded.ts";
import { MineflayerExecutor, type BotConfig } from "../src/executor.ts";
import { SCORE_TRACK_TIMEOUT_MS } from "../src/executor/score.ts";
import { MS_PER_TICK, SERVER_STALL_CEILING_MS, ServerDeadline } from "../src/server-deadline.ts";

// The judge, pure: a deadline is over only once its span has passed on the wall
// AND on the server's clock.

test("a deadline on a ticking server is over once its span has passed on both clocks", () => {
  const d = new ServerDeadline(5_000, 0, 1_000);
  assert.equal(d.over(4_999, 1_100), false, "the wall span has not passed");
  assert.equal(d.over(5_000, 1_080), false, "80 ticks are 4 s of the server's time");
  assert.equal(d.over(5_000, 1_100), true);
  assert.match(d.spent(5_000, 1_100), /^5000ms \(100 server tick\(s\) over 5000ms\)$/);
});

test("a deadline on a server whose clock stands still stays open until the watchdog's span has passed too", () => {
  const d = new ServerDeadline(5_000, 0, 1_000);
  assert.equal(d.over(21_000, 1_000), false, "a 21 s stall holds the wait open");
  assert.equal(d.over(5_000 + SERVER_STALL_CEILING_MS - 1, 1_020), false);
  assert.equal(d.over(5_000 + SERVER_STALL_CEILING_MS, 1_020), true);
  assert.match(d.spent(65_000, 1_020), /advanced only 20 tick\(s\) of the 100 the wait counts/);
});

test("with no world age in hand the deadline is the wall clock alone", () => {
  const d = new ServerDeadline(5_000, 0, undefined);
  assert.equal(d.over(4_999, undefined), false);
  assert.equal(d.over(5_000, undefined), true);
  assert.equal(d.spent(5_000, undefined), "5000ms");
  assert.equal(MS_PER_TICK, 50);
});

// The read that failed on the town's branch world: the ride read sent at the
// start of a server stall longer than its 5 s span, answered in the first tick
// after it.

class StallingBot extends EventEmitter {
  username = "delve-bot";
  entity = { position: { x: 0, y: 64, z: 0 }, onGround: true };
  game = { gameMode: "adventure" };
  pathfinder = { stop: (): void => {}, setGoal: (): void => {} };
  time = { age: 1_000 as number | null };
  /** Wall ms the server stands still for once a ride read's `tellraw` arrives. */
  stallMs = 0;
  loadPlugin(): void {}
  blockAt(): null {
    return null;
  }
  chat(message: string): void {
    if (!message.startsWith("/tellraw @s ")) return;
    const parts = JSON.parse(message.slice("/tellraw @s ".length)) as { text?: string }[];
    const marker = parts[0]!.text!;
    // The clock stands still (no `update_time`) for the stall, then the
    // server runs its next tick: the answer, then the clock packet.
    setTimeout(() => {
      this.emit("messagestr", `${marker}1b [10.5d, 62.0d, 20.5d]]`);
      this.time.age = (this.time.age ?? 0) + 20;
      this.emit("time");
    }, this.stallMs);
  }
}

const CONFIG: BotConfig = { host: "127.0.0.1", port: 25565, username: "delve-bot", version: "1.21.11", auth: "offline" };

function attach(bot: StallingBot): MineflayerExecutor {
  const executor = new MineflayerExecutor(CONFIG, {});
  executor.useNonCombatants(new Set());
  executor.useStoryBodies([]);
  executor.attachBot(bot as unknown as Bot);
  return executor;
}

test("a ride read sent into a server stall longer than its span is answered after it, not failed", async () => {
  const bot = new StallingBot();
  bot.stallMs = SCORE_TRACK_TIMEOUT_MS + 1_500;
  const executor = attach(bot);
  const reading = await within("readRide(stalled)", executor.readRide("dw_v_boat"), SCORE_TRACK_TIMEOUT_MS + 20_000);
  assert.deepEqual(reading, { riding: true, pos: [10.5, 62, 20.5] });
});

test("a ride read a ticking server never answers fails, naming the server ticks it waited", async () => {
  const bot = new StallingBot();
  bot.chat = (): void => {};
  // The server ticks: a clock packet every second, 20 ticks each.
  const clock = setInterval(() => {
    bot.time.age = (bot.time.age ?? 0) + 20;
    bot.emit("time");
  }, 1_000);
  try {
    const executor = attach(bot);
    await within(
      "readRide(unanswered)",
      assert.rejects(
        () => executor.readRide("dw_v_boat"),
        /^Error: ride read: no answer in 5000ms \(\d+ server tick\(s\) over \d+ms\)$/,
      ),
      SCORE_TRACK_TIMEOUT_MS + 10_000,
    );
  } finally {
    clearInterval(clock);
  }
});
