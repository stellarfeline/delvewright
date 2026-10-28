// The bot's pick ray meets a block by its outline (spec-0117 §2b).
//
// `src/outline.ts` reads the engine's outline table; these tests hold it to
// the engine's own answers, and hold the crosshair to the click box the
// compiler fits inside a block with no collision.

import { test } from "node:test";
import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import type { Bot } from "mineflayer";
import registryFor from "prismarine-registry";

import { blockStateText } from "../src/block-state.ts";
import { MineflayerExecutor, type BotConfig } from "../src/executor.ts";
import { outlineOf, outlineTableCounts, type OutlineBox } from "../src/outline.ts";

const requireCjs = createRequire(import.meta.url);
const registry = registryFor("1.21.11");

interface LiveBlock {
  name: string;
  boundingBox: string;
  shapes: number[][];
  getProperties(): Record<string, unknown>;
}

const Block = requireCjs("prismarine-block")(registry) as {
  fromString(state: string, biome: number): LiveBlock;
  fromStateId(id: number, biome: number): LiveBlock;
};

/** The engine's answers: `crates/delvec/tests/outline_ledger.rs` writes them. */
interface Agreement {
  readonly blocks: readonly string[];
  readonly states: number;
  readonly cases: readonly { readonly boxes: readonly OutlineBox[]; readonly states: readonly string[] }[];
}

const AGREEMENT = JSON.parse(
  readFileSync(new URL("./fixtures/outline-agreement-1.21.11.json", import.meta.url), "utf8"),
) as Agreement;

const agreementBlock = (id: string): boolean =>
  id === "minecraft:lever" || id.endsWith("_button") || id.endsWith("_sign");

test("the harness's outline of every lever, button and sign state is the engine's", () => {
  const engine = new Map<string, readonly OutlineBox[]>();
  for (const c of AGREEMENT.cases) for (const s of c.states) engine.set(s, c.boxes);
  assert.equal(engine.size, AGREEMENT.states);

  // The states as the client knows them: every state id of every such block.
  const ids: string[] = [];
  const differences: string[] = [];
  let asked = 0;
  for (const b of registry.blocksArray as { name: string; minStateId: number; maxStateId: number }[]) {
    const id = `minecraft:${b.name}`;
    if (!agreementBlock(id)) continue;
    ids.push(id);
    for (let sid = b.minStateId; sid <= b.maxStateId; sid += 1) {
      const live = Block.fromStateId(sid, 0);
      const props = Object.fromEntries(
        Object.entries(live.getProperties()).map(([k, v]) => [k, String(v)]),
      );
      const text = blockStateText({ name: id, properties: props });
      asked += 1;
      const ours = outlineOf(live.name, live.getProperties());
      const theirs = engine.get(text);
      if (theirs === undefined) differences.push(`${text}: the engine was not asked`);
      else if (JSON.stringify(ours) !== JSON.stringify(theirs)) {
        differences.push(`${text}: harness ${JSON.stringify(ours)}, engine ${JSON.stringify(theirs)}`);
      }
    }
  }
  assert.deepEqual([...ids].sort(), [...AGREEMENT.blocks].sort(), "the same blocks on both sides");
  assert.equal(asked, AGREEMENT.states, "the client and the engine count the same states");
  assert.deepEqual(differences, [], `${differences.length} of ${asked} states differ`);
  process.stderr.write(`[outline] agreement: ${asked} states of ${ids.length} blocks, 0 differences\n`);
});

test("the outline table stands for every state of the client's registry", () => {
  const { blocks, states } = outlineTableCounts();
  const reg = registry.blocksArray as { name: string; minStateId: number; maxStateId: number }[];
  assert.equal(blocks, reg.length);
  assert.equal(states, reg.reduce((n, b) => n + b.maxStateId - b.minStateId + 1, 0));
  assert.deepEqual(outlineOf("stone", {}), [[0, 0, 0, 1, 1, 1]]);
  assert.deepEqual(outlineOf("air", {}), []);
  assert.equal(outlineOf("modded:thing", {}), undefined);
  assert.equal(outlineOf("lever", { face: "wall" }), undefined, "a missing property is not guessed");
});

// --- the pick ray meets a wall lever's fitted click box ---------------------

class Vec {
  readonly x: number;
  readonly y: number;
  readonly z: number;
  constructor(x: number, y: number, z: number) {
    this.x = x;
    this.y = y;
    this.z = z;
  }
  offset(dx: number, dy: number, dz: number): Vec {
    return new Vec(this.x + dx, this.y + dy, this.z + dz);
  }
  clone(): Vec {
    return new Vec(this.x, this.y, this.z);
  }
  distanceTo(o: { x: number; y: number; z: number }): number {
    return Math.hypot(this.x - o.x, this.y - o.y, this.z - o.z);
  }
}

/** A room: stone floor below y = 65, a stone wall at x = 28, a wall lever on it. */
class LeverRoomBot extends EventEmitter {
  username = "delve-bot";
  registry = registry;
  entity = { id: 1, position: new Vec(25.5, 65, 7.5), onGround: true, attributes: {} };
  game = { gameMode: "adventure" };
  entities: Record<number, unknown> = {};
  pathfinder = { stop: (): void => {}, setGoal: (): void => {} };
  loadPlugin(): void {}
  blockAt(pos: { x: number; y: number; z: number }): LiveBlock {
    const [x, y, z] = [Math.floor(pos.x), Math.floor(pos.y), Math.floor(pos.z)];
    if (x === 27 && y === 65 && z === 7) {
      return Block.fromString("minecraft:lever[face=wall,facing=west,powered=false]", 0);
    }
    if (y < 65 || x >= 28) return Block.fromString("minecraft:stone", 0);
    return Block.fromString("minecraft:air", 0);
  }
}

const CONFIG: BotConfig = {
  host: "127.0.0.1",
  port: 25565,
  username: "delve-bot",
  version: "1.21.11",
  auth: "offline",
};

test("a wall lever's fitted click box is picked from a stance in front of it", () => {
  const bot = new LeverRoomBot();
  // The box `ClickBox::fitted_to` summons for a west-facing wall lever at
  // [27, 65, 7]: its outline x 10..16, y 4..12, z 5..11 sixteenths, one margin
  // proud of it — the box a live demo run met (x 27.6125..28.0075).
  bot.entities[2] = {
    id: 2,
    name: "interaction",
    position: { x: 27.81, y: 65.24, z: 7.5 },
    width: 0.395,
    height: 0.52,
  };
  assert.deepEqual(outlineOf("lever", { face: "wall", facing: "west", powered: false }), [
    [0.625, 0.25, 0.3125, 1, 0.75, 0.6875],
  ]);
  const executor = new MineflayerExecutor(CONFIG, {});
  executor.attachBot(bot as unknown as Bot);
  const got = executor.requireCrosshair([27, 65, 7], "interact anchor/lever", 2);
  assert.ok(got !== undefined, "the box is tracked");
  assert.equal(got.target.id, 2);
});

test("the pick ray still stops at a lever's outline when no box stands proud of it", () => {
  const bot = new LeverRoomBot();
  // A box flush inside the outline: the lever's own face is nearer on every ray.
  bot.entities[2] = {
    id: 2,
    name: "interaction",
    position: { x: 27.85, y: 65.3, z: 7.5 },
    width: 0.2,
    height: 0.3,
  };
  const executor = new MineflayerExecutor(CONFIG, {});
  executor.attachBot(bot as unknown as Bot);
  assert.throws(
    () => executor.requireCrosshair([27, 65, 7], "interact anchor/lever", 2),
    /block lever at \[27, 65, 7\]/,
  );
});
