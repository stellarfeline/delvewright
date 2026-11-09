// spec-0101 §5.4: when the bot reads a watching body's yaw. The judgement is
// made against a fake server that keeps the game's tick order, so the window
// between a body going live and the watch line turning it can be hit on purpose.

import { test } from "node:test";
import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import type { Bot } from "mineflayer";
import { within } from "./bounded.ts";
import { MineflayerExecutor, type BotConfig } from "../src/executor.ts";
import { bearing, type Watcher } from "../src/watch.ts";

const CONFIG: BotConfig = { host: "127.0.0.1", port: 25565, username: "delve-bot", version: "1.21.11", auth: "offline" };

const BODY_FEET: [number, number, number] = [15.5, 67, 9.5];
const BOT_FEET: [number, number, number] = [22.5, 67, 8.38];
const HOME_YAW = -180;

const WATCHER: Watcher = {
  id: "npc/curator",
  class: "npc",
  tag: "dw_npc_curator",
  selector: "tag=dw_npc_curator,tag=dw_npc",
  feet: BODY_FEET,
  stands: [BODY_FEET],
  homeYaw: HOME_YAW,
  who: "nearest",
  within: 8,
};

/**
 * A server that runs ticks in the game's order. Each answered game-time reading
 * is one tick's packet phase; the tick's function phase follows it: the watch
 * line first (it turns a body whose tag is live), then the story's functions,
 * among them a walk's arrival, which restores the tag after the watch line has
 * already passed — the order that leaves a live body unturned for one tick.
 */
class WatchServer {
  tick = 1_000;
  /** The tick whose function phase restores the body's tag; never when undefined. */
  arrivesAt: number | undefined;
  live = false;
  yaw = HOME_YAW;
  /** The yaw reads the harness made, with the tick each was answered in. */
  readonly yawReads: { tick: number; yaw: number }[] = [];
  private stored = 0;
  private readonly bot: EventEmitter;
  constructor(bot: EventEmitter) {
    this.bot = bot;
  }

  private functionPhase(): void {
    if (this.live) this.yaw = bearing(BODY_FEET, BOT_FEET); // the watch line
    if (this.arrivesAt === this.tick) this.live = true; // the walk's arrival, after it
    this.tick += 1;
  }

  answer(message: string): void {
    const say = (text: string): void => void this.bot.emit("messagestr", text);
    if (message.startsWith("/execute store result storage dw_harness:clock t int 1 run time query gametime")) {
      this.stored = this.tick;
      return;
    }
    if (!message.startsWith("/tellraw @s ") && !message.startsWith("/execute if entity")) return;
    if (message.startsWith("/execute if entity @e[")) {
      const m = /y_rotation=(-?[\d.]+)\.\.(-?[\d.]+)\]/.exec(message);
      const text = (JSON.parse(message.slice(message.indexOf(" run tellraw @s ") + 16)) as { text: string }).text;
      if (this.live && m && this.yaw >= Number(m[1]) && this.yaw <= Number(m[2])) say(text);
      return;
    }
    const body = JSON.parse(message.slice("/tellraw @s ".length)) as unknown;
    if (!Array.isArray(body)) {
      say((body as { text: string }).text);
      return;
    }
    const parts = body as ({ text?: string; storage?: string; nbt?: string; entity?: string } | string)[];
    const head = parts[0] as { text?: string } | string;
    const marker = typeof head === "string" ? head : (head.text ?? "");
    const nbt = (parts[1] as { nbt?: string }).nbt;
    if (nbt === "t") {
      say(`${marker}${this.stored}]`);
      // The reading was answered in this tick's packet phase; its function phase follows,
      // and the reads the bot sends on hearing the reply land in the next tick's packet phase.
      this.functionPhase();
      return;
    }
    if (marker.startsWith("[dw:watch-pos @s ")) return say(`${marker}[${BOT_FEET.map((c) => `${c}d`).join(", ")}]`);
    if (marker.startsWith("[dw:watch-pos ")) {
      if (this.live) say(`${marker}[${BODY_FEET.map((c) => `${c}d`).join(", ")}]`);
      return;
    }
    if (marker.startsWith("[dw:watch-yaw ")) {
      this.yawReads.push({ tick: this.tick, yaw: this.yaw });
      say(`${marker}${this.yaw}f]`);
    }
  }
}

function setup(): { executor: MineflayerExecutor; server: WatchServer; bot: EventEmitter } {
  const bot = new EventEmitter() as EventEmitter & Record<string, unknown>;
  bot.username = "delve-bot";
  bot.entity = { position: { x: BOT_FEET[0], y: BOT_FEET[1], z: BOT_FEET[2] }, velocity: { x: 0, y: 0, z: 0 } };
  bot.game = { gameMode: "adventure" };
  bot.clearControlStates = () => {};
  const server = new WatchServer(bot);
  bot.chat = (m: string) => server.answer(m);
  const executor = new MineflayerExecutor(CONFIG, {});
  executor.attachBot(bot as unknown as Bot);
  executor.useWatchers([WATCHER]);
  return { executor, server, bot };
}

test("a body that went live in the tick before the bot's read is judged after the watch line has run on it", async () => {
  const { executor, server } = setup();
  // The body's walk arrives in the function phase of the tick the third
  // game-time reading answers: after that tick's watch line, so the reads the
  // bot sends next find the tag live and the yaw still the summon's.
  server.arrivesAt = server.tick + 2;
  await within("judgeWatchers(arrival)", executor.judgeWatchers("arrival waypoint"));
  const ledger = executor.watchLedger();
  assert.deepEqual(ledger.failures(), [], `no judgement of an unturned body: ${ledger.failures().join(" | ")}`);
  assert.equal(ledger.binding().asserted, 1, "the body was judged, not skipped");
  assert.ok(
    server.yawReads.every((r) => r.yaw !== HOME_YAW),
    "no yaw was read before the watch line had run",
  );
});

test("a body live for many ticks is judged, and a body that never faces is still refused", async () => {
  const { executor, server } = setup();
  server.live = true;
  await within("judgeWatchers(standing)", executor.judgeWatchers("standing waypoint"));
  assert.deepEqual(executor.watchLedger().failures(), []);
  assert.equal(executor.watchLedger().binding().asserted, 1);

  // A watch line that never turns it: the denial is recorded, not hidden.
  const b = setup();
  b.server.live = true;
  (b.server as unknown as { functionPhase: () => void }).functionPhase = function (this: WatchServer) {
    this.tick += 1; // time passes; nothing turns the body
  };
  await within("judgeWatchers(deaf)", b.executor.judgeWatchers("deaf waypoint"));
  assert.equal(b.executor.watchLedger().failures().length, 1);
  assert.match(b.executor.watchLedger().failures()[0]!, /the body's yaw is -180\.00°/);
});
