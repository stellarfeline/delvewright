import { test } from "node:test";
import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import type { Bot } from "mineflayer";
import { within } from "./bounded.ts";
import { MineflayerExecutor, type BotConfig } from "../src/executor.ts";
import type { HarnessBot } from "../src/client-loaded.ts";
import type { ClientLoadedState } from "../src/client-loaded.ts";
import type { ResourcePackState } from "../src/resource-pack.ts";
import {
  PLACEMENT_TIMEOUT_MS,
  PlacementLatch,
  PlacementRefusedError,
  PlacementTimeoutError,
  parsePlacementReply,
  placementCommands,
  placementTimeoutFromEnv,
} from "../src/placement.ts";

const CONFIG: BotConfig = {
  host: "127.0.0.1",
  port: 25565,
  username: "delve-bot",
  version: "1.21.11",
  auth: "offline",
};

class FakeVec3 {
  x: number;
  y: number;
  z: number;
  constructor(x: number, y: number, z: number) {
    this.x = x;
    this.y = y;
    this.z = z;
  }
  offset(dx: number, dy: number, dz: number): FakeVec3 {
    return new FakeVec3(this.x + dx, this.y + dy, this.z + dz);
  }
}

/**
 * A server that lets the bot log in at once and builds its world LATER: the
 * datapack's `#placed dw.sys` reads 1 from game tick `placedAt`, and (when the
 * build has a `spawn` anchor) the tick after that is the one in which
 * `join_place` teleports the bot to the entry point and tags it `dw_joined`.
 * The game time moves on `ticksPerCommand` with every command it processes.
 *
 * It answers the commands the harness sends the way the pinned server does:
 * `time query gametime` stored in harness storage, `execute if|unless score
 * #placed dw.sys matches 1 run tellraw @s […]`, `execute if entity
 * @s[tag=dw_joined] run tellraw …`, and a bracket's `tellraw @s {"text": …}`.
 */
class LateWorldServer extends EventEmitter {
  username = "delve-bot";
  entity = { id: 1, position: new FakeVec3(8.5, 63, 8.5), onGround: true };
  entities: Record<number, unknown> = {};
  game = { gameMode: "adventure", minY: -64, height: 384 };
  health = 20;
  pathfinder = { stop: (): void => {}, setGoal: (): void => {}, movements: undefined };
  readonly _client = Object.assign(new EventEmitter(), { write: (): void => {} });
  /** Every command the bot sent, with the game time and world state it arrived at. */
  readonly log: { command: string; tick: number; placed: boolean; joined: boolean }[] = [];
  tick = 0;
  stored: number | undefined;
  readonly placedAt: number;
  readonly firstJoin: boolean;
  readonly ticksPerCommand: number;
  readonly refuse: string | undefined;
  constructor(placedAt: number, firstJoin: boolean, ticksPerCommand = 5, refuse: string | undefined = undefined) {
    super();
    this.placedAt = placedAt;
    this.firstJoin = firstJoin;
    this.ticksPerCommand = ticksPerCommand;
    this.refuse = refuse;
  }
  loadPlugin(): void {}
  blockAt(): { name: string; type: number } | null {
    return { name: "stone", type: 1 };
  }
  placed(): boolean {
    return this.tick >= this.placedAt;
  }
  joined(): boolean {
    return this.firstJoin && this.tick > this.placedAt;
  }
  end(): void {}
  chat(command: string): void {
    this.tick += this.ticksPerCommand;
    if (this.joined()) this.entity.position = new FakeVec3(265.5, 96, 10.5);
    this.log.push({ command, tick: this.tick, placed: this.placed(), joined: this.joined() });
    const say = (text: string): void => {
      setImmediate(() => this.emit("messagestr", text));
    };
    const tellraw = (json: string): void => {
      const parsed = JSON.parse(json) as unknown;
      const parts = Array.isArray(parsed) ? parsed : [parsed];
      say(
        parts
          .map((p) =>
            typeof p === "string"
              ? p
              : (p as { text?: string; nbt?: string }).text ??
                ((p as { nbt?: string }).nbt !== undefined && this.stored !== undefined
                  ? String(this.stored)
                  : ""),
          )
          .join(""),
      );
    };
    let m: RegExpExecArray | null;
    if ((m = /^\/tellraw @s (.*)$/.exec(command))) {
      tellraw(m[1]!);
    } else if (/^\/execute store result storage \S+ t int 1 run time query gametime$/.test(command)) {
      this.stored = this.tick;
    } else if ((m = /^\/execute (if|unless) score #placed dw\.sys matches 1 run tellraw @s (.*)$/.exec(command))) {
      if (this.refuse !== undefined) {
        say(this.refuse);
        return;
      }
      if ((m[1] === "if") === this.placed()) tellraw(m[2]!);
    } else if ((m = /^\/execute if entity @s\[tag=dw_joined\] run tellraw @s (.*)$/.exec(command))) {
      if (this.joined()) tellraw(m[1]!);
    }
  }
}

function make(server: LateWorldServer): () => HarnessBot {
  return () => {
    // The login: the server spawns the bot as soon as it connects, whatever the
    // datapack is still building.
    setImmediate(() => server.emit("spawn"));
    return {
      bot: server as unknown as Bot,
      loaded: { phase: () => "loaded", windows: () => [], sent: () => [] } as unknown as ClientLoadedState,
      pack: { pushes: () => [] } as unknown as ResourcePackState,
    };
  };
}

const ENV = { DELVEWRIGHT_ENTITY_SETTLE_TIMEOUT_MS: "0" };

test("a world placed after the login: no step's command reaches the server before placement and the first join", async () => {
  // The race, made deterministic: the bot spawns at once, and the world reads
  // placed 60 game ticks later. Before the wait, the first step's `/trigger`
  // went out at the spawn, into a world still at `#placed 0`.
  const server = new LateWorldServer(60, true);
  const executor = new MineflayerExecutor(CONFIG, ENV);
  await within("executor.connect()", executor.connect(make(server)));
  executor.bot!.chat("/trigger dw.class set 1"); // the first step's command
  const first = server.log.find((l) => !l.command.startsWith("/tellraw") && !l.command.startsWith("/execute"));
  assert.ok(first, "the step's command reached the server");
  assert.equal(first.command, "/trigger dw.class set 1");
  assert.ok(first.placed, `the first step's command arrived at tick ${first.tick}, before placement at tick 60`);
  assert.ok(first.joined, `the first step's command arrived at tick ${first.tick}, before the first-join teleport`);
  assert.equal(executor.placement?.firstJoin.kind, "joined");
  assert.ok((executor.placement?.placedTick ?? 0) >= 60);
  assert.ok((executor.placement?.readings ?? 0) > 1, "placement was read more than once: it was waited for");
});

test("a build with no first-join placement is ready at the first reading past the placement tick", async () => {
  const server = new LateWorldServer(30, false);
  const executor = new MineflayerExecutor(CONFIG, ENV);
  await within("executor.connect()", executor.connect(make(server)));
  const record = executor.placement!;
  assert.equal(record.firstJoin.kind, "none");
  assert.ok(record.placedTick !== undefined && record.placedTick >= 30);
  assert.ok(record.firstJoin.tick > record.placedTick!, "decided at a LATER game tick than placement was read at");
});

test("a world never placed inside the budget fails connect by name, and nothing else is sent", async () => {
  const server = new LateWorldServer(Number.MAX_SAFE_INTEGER, true);
  const executor = new MineflayerExecutor(CONFIG, { ...ENV, DELVEWRIGHT_PLACEMENT_TIMEOUT_MS: "1200" });
  await assert.rejects(
    within("executor.connect()", executor.connect(make(server))),
    (err: unknown) =>
      err instanceof PlacementTimeoutError &&
      /had not finished placing its world 1200ms after the bot spawned/.test(err.message) &&
      /not yet 1/.test(err.message),
  );
  assert.equal(executor.placement, undefined);
  assert.ok(
    server.log.every((l) => l.command.startsWith("/tellraw") || l.command.startsWith("/execute")),
    "only placement readings were sent",
  );
});

test("a refused placement reading fails connect at once, by the shared rejection rule", async () => {
  const server = new LateWorldServer(0, true, 5, "Unknown scoreboard objective 'dw.sys'");
  const executor = new MineflayerExecutor(CONFIG, ENV);
  await assert.rejects(
    within("executor.connect()", executor.connect(make(server))),
    (err: unknown) => err instanceof PlacementRefusedError && /Unknown scoreboard objective/.test(err.message),
  );
});

test("the reading's commands ask the datapack's own latch, its own tag, and the game time", () => {
  const cmds = placementCommands(7);
  assert.equal(cmds.length, 4);
  assert.equal(cmds[0], "/execute store result storage dw_harness:placement t int 1 run time query gametime");
  assert.match(cmds[1]!, /^\/execute if score #placed dw\.sys matches 1 run tellraw @s /);
  assert.match(cmds[2]!, /^\/execute unless score #placed dw\.sys matches 1 run tellraw @s /);
  assert.match(cmds[3]!, /^\/execute if entity @s\[tag=dw_joined\] run tellraw @s /);
  for (const c of cmds) assert.ok(c.length <= 256, `a chat command fits one packet: ${c.length}`);
});

test("a reply that says both, or neither, establishes nothing", () => {
  assert.equal(parsePlacementReply(1, []).placed, undefined);
  assert.equal(
    parsePlacementReply(1, ["[dw:placed 1 done 5]", "[dw:placed 1 pending 5]"]).placed,
    undefined,
  );
  assert.deepEqual(parsePlacementReply(1, ["[dw:placed 1 done 5]"]), { placed: true, tick: 5, joined: false });
  assert.deepEqual(parsePlacementReply(2, ["[dw:placed 1 done 5]"]).placed, undefined, "another reading's line");
});

test("the latch never decides 'no first join' without a later game tick", () => {
  const latch = new PlacementLatch();
  assert.equal(latch.observe({ placed: false, tick: 1, joined: false }).ready, false);
  assert.equal(latch.observe({ placed: true, tick: 9, joined: false }).ready, false);
  assert.equal(latch.observe({ placed: true, tick: 9, joined: false }).ready, false, "same tick: join_place may not have run");
  assert.equal(latch.observe({ placed: true, tick: undefined, joined: false }).ready, false, "no tick: undecidable");
  const v = latch.observe({ placed: true, tick: 10, joined: false });
  assert.ok(v.ready && v.firstJoin.kind === "none" && v.placedTick === 9);
  const untimed = new PlacementLatch();
  assert.equal(untimed.observe({ placed: true, tick: undefined, joined: false }).ready, false);
  assert.equal(untimed.observe({ placed: true, tick: 50, joined: false }).ready, false);
  assert.ok(untimed.observe({ placed: true, tick: undefined, joined: true }).ready);
});

test("the placement budget comes from the environment, and a malformed one is refused", () => {
  assert.equal(placementTimeoutFromEnv({}), PLACEMENT_TIMEOUT_MS);
  assert.equal(placementTimeoutFromEnv({ DELVEWRIGHT_PLACEMENT_TIMEOUT_MS: "900000" }), 900_000);
  for (const bad of ["0", "-5", "12s", "1.5"]) {
    assert.throws(() => placementTimeoutFromEnv({ DELVEWRIGHT_PLACEMENT_TIMEOUT_MS: bad }));
  }
});
