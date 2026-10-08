// The harness bot reports `player_loaded` when a vanilla client would, and a bot
// that does not is caught — by the factory rule and by an observer on the wire.
import { test } from "node:test";
import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import { readdir, readFile } from "node:fs/promises";
import nodePath from "node:path";
import { fileURLToPath } from "node:url";
import {
  CLIENT_WAIT_TIMEOUT_MS,
  installClientLoaded,
  type LoadedBot,
} from "../src/client-loaded.ts";
import { traceLoadWindows, unreportedWindows, type TracedBot } from "../src/load-window.ts";

class FakeVec {
  x: number;
  y: number;
  z: number;
  constructor(x: number, y: number, z: number) {
    this.x = x;
    this.y = y;
    this.z = z;
  }
  offset(dx: number, dy: number, dz: number): FakeVec {
    return new FakeVec(this.x + dx, this.y + dy, this.z + dz);
  }
}

/** A mineflayer-shaped bot: a client that receives packets and records what it sends. */
class FakeBot extends EventEmitter {
  readonly client = new EventEmitter() as EventEmitter & { write(name: string, params: object): void };
  readonly sentPackets: string[] = [];
  readonly _client = this.client;
  entity: { id: number; position: FakeVec | undefined; onGround: boolean } = {
    id: 7,
    position: undefined,
    onGround: true,
  };
  entities: Record<number, undefined> = {};
  game = { gameMode: "adventure", minY: -64, height: 384 };
  health = 20;
  chunkLoaded = false;
  constructor() {
    super();
    this.client.write = (name: string): void => {
      this.sentPackets.push(name);
    };
  }
  blockAt(_pos: FakeVec): { name: string } | null {
    return this.chunkLoaded ? { name: "stone" } : null;
  }
  /** The server's side of a join or respawn, up to the chunk the player stands in. */
  arrive(cause: "login" | "respawn"): void {
    this.chunkLoaded = false;
    this.client.emit(cause, {});
    this.client.emit("game_state_change", { reason: "level_chunks_load_start", gameMode: 0 });
    this.entity.position = new FakeVec(0.5, 64, 0.5);
  }
  tick(): void {
    this.emit("physicsTick");
  }
  loaded(): number {
    return this.sentPackets.filter((p) => p === "player_loaded").length;
  }
}

test("player_loaded goes once the player's chunk is loaded, not before", () => {
  const bot = new FakeBot();
  const state = installClientLoaded(bot as unknown as LoadedBot);
  bot.arrive("login");
  bot.tick();
  bot.tick();
  assert.equal(bot.loaded(), 0, "sent before the chunk under the player arrived");
  assert.equal(state.phase(), "waiting-for-player-chunk");
  bot.chunkLoaded = true;
  bot.tick();
  assert.equal(bot.loaded(), 1);
  bot.tick();
  assert.equal(bot.loaded(), 1, "sent twice for one join");
  assert.equal(state.phase(), "loaded");
  assert.equal(state.windows()[0]?.reason, "chunk");
});

test("every respawn opens a fresh window and is answered again", () => {
  const bot = new FakeBot();
  installClientLoaded(bot as unknown as LoadedBot);
  for (const cause of ["login", "respawn", "respawn"] as const) {
    bot.arrive(cause);
    bot.chunkLoaded = true;
    bot.tick();
  }
  assert.equal(bot.loaded(), 3);
});

test("nothing goes before the server says the level is loading", () => {
  const bot = new FakeBot();
  installClientLoaded(bot as unknown as LoadedBot);
  bot.client.emit("login", {});
  bot.entity.position = new FakeVec(0.5, 64, 0.5);
  bot.chunkLoaded = true;
  bot.tick();
  assert.equal(bot.loaded(), 0, "the vanilla client waits for level_chunks_load_start");
  bot.client.emit("game_state_change", { reason: "level_chunks_load_start", gameMode: 0 });
  bot.tick();
  assert.equal(bot.loaded(), 1);
});

test("a chunk that never arrives is answered at the client's own 30 s timeout", () => {
  let clock = 1_000;
  const bot = new FakeBot();
  const state = installClientLoaded(bot as unknown as LoadedBot, () => clock);
  bot.arrive("login");
  clock += CLIENT_WAIT_TIMEOUT_MS;
  bot.tick();
  assert.equal(bot.loaded(), 0, "sent at, not after, the deadline");
  clock += 1;
  bot.tick();
  assert.equal(bot.loaded(), 1);
  assert.equal(state.windows()[0]?.reason, "timeout");
});

const ctx = { stage: () => "critical-path", step: () => 3, lethal: () => [] };

test("the wire observer: a bot WITHOUT the tracker is detected, one with it is not", () => {
  let clock = 0;
  const now = (): number => clock;

  const bare = new FakeBot();
  const bareTrace = traceLoadWindows(bare as unknown as TracedBot, ctx, now);
  bare.arrive("login");
  bare.chunkLoaded = true;
  for (let i = 0; i < 80; i++) {
    clock += 50;
    bare.tick();
  }
  const [w] = bareTrace.windows();
  assert.equal(w?.closedBy, "server-timeout");
  assert.equal(w?.lengthMs, 3_000);
  clock += CLIENT_WAIT_TIMEOUT_MS;
  assert.equal(unreportedWindows(bareTrace.windows(), clock, CLIENT_WAIT_TIMEOUT_MS).length, 1);

  clock = 0;
  const harness = new FakeBot();
  // Observer first, as the executor installs it after the factory: either order
  // must see the send, because both wrap or call the same `write`.
  installClientLoaded(harness as unknown as LoadedBot, now);
  const trace = traceLoadWindows(harness as unknown as TracedBot, ctx, now);
  harness.arrive("login");
  harness.chunkLoaded = true;
  clock += 50;
  harness.tick();
  const [h] = trace.windows();
  assert.equal(h?.closedBy, "player_loaded");
  assert.equal(h?.lengthMs, 50);
  clock += CLIENT_WAIT_TIMEOUT_MS * 2;
  assert.equal(unreportedWindows(trace.windows(), clock, CLIENT_WAIT_TIMEOUT_MS).length, 0);
});

test("a hit inside an open window is recorded as absorbed", () => {
  const clock = 0;
  const bot = new FakeBot();
  const trace = traceLoadWindows(bot as unknown as TracedBot, ctx, () => clock);
  bot.arrive("respawn");
  (bot.entities as Record<number, unknown>)[9] = { type: "hostile", name: "zombie", position: new FakeVec(1.5, 64, 0.5) };
  bot.client.emit("animation", { entityId: 9, animation: 0 });
  assert.deepEqual(
    trace.windows()[0]?.absorbed.map((a) => a.kind),
    ["swing"],
  );
});

// The factory rule: every bot the harness makes goes through createHarnessBot.
const HARNESS = nodePath.resolve(nodePath.dirname(fileURLToPath(import.meta.url)), "..");
/** The factory itself, and the probe whose subject is a bot that does NOT send. */
const MAY_CALL_CREATEBOT = new Set(["src/client-loaded.ts", "probe/client-loaded.ts"]);

test("no harness bot is made except by createHarnessBot", async () => {
  const offenders: string[] = [];
  let scanned = 0;
  for (const dir of ["src", "probe"]) {
    // Recursive: `src/executor/` holds one file per object the executor acts on.
    for (const name of await readdir(nodePath.join(HARNESS, dir), { recursive: true })) {
      if (!/\.(ts|mts|js|mjs)$/.test(name)) continue;
      const rel = `${dir}/${name.split(nodePath.sep).join("/")}`;
      scanned += 1;
      const text = await readFile(nodePath.join(HARNESS, rel), "utf8");
      if (/\bcreateBot\s*\(/.test(text) && !MAY_CALL_CREATEBOT.has(rel)) offenders.push(rel);
    }
  }
  assert.ok(scanned >= 25, `scanned only ${scanned} file(s) — the walk bound to nothing`);
  assert.deepEqual(offenders, [], "a bot made outside createHarnessBot never reports player_loaded");
  // The exemption is held to its purpose: the probe makes BOTH kinds of bot.
  const probe = await readFile(nodePath.join(HARNESS, "probe/client-loaded.ts"), "utf8");
  assert.match(probe, /mineflayer\.createBot\(/);
  assert.match(probe, /createHarnessBot\(/);
});
