// A repaint reaches the bot's world (spec-0080 §5.2, spec-0109 §5.3).
//
// The pinned mineflayer drops `chunk_biomes`, so a station read after a
// repaint of a chunk the bot already held named the biome from before it.
// These tests encode the packet with prismarine-chunk's own biome-container
// writer — a second implementation of the format, sharing nothing with the
// decoder — and read the result back through the world the bot navigates by.

import { test } from "node:test";
import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import { createRequire } from "node:module";

import { createHarnessBot } from "../src/client-loaded.ts";
import { BIOMES_PER_SECTION, ChunkBiomesDecodeError, decodeChunkBiomes } from "../src/chunk-biomes.ts";

const VERSION = "1.21.11";
const MIN_Y = -64;
const HEIGHT = 384;
const SECTIONS = HEIGHT >> 4;

const require = createRequire(import.meta.url);
const chunkRequire = createRequire(require.resolve("prismarine-chunk"));

interface Column {
  getBiome(pos: { x: number; y: number; z: number }): number;
  setBiome(pos: { x: number; y: number; z: number }, id: number): void;
  biomes: { write(buf: unknown): void }[];
}

function newColumn(): Column {
  const Chunk = require("prismarine-chunk")(VERSION) as new (o: object) => Column;
  return new Chunk({ minY: MIN_Y, worldHeight: HEIGHT });
}

/** Fill every biome cell of a column with `id`. */
function fill(c: Column, id: number): void {
  for (let y = MIN_Y; y < MIN_Y + HEIGHT; y += 4) {
    for (let x = 0; x < 16; x += 4) for (let z = 0; z < 16; z += 4) c.setBiome({ x, y, z }, id);
  }
}

/** The `chunk_biomes` entry data for a column: every section's biome container, in order. */
function encode(c: Column): Buffer {
  const { SmartBuffer } = chunkRequire("smart-buffer") as { SmartBuffer: new () => { toBuffer(): Buffer } };
  const buf = new SmartBuffer();
  for (const section of c.biomes) section.write(buf);
  return buf.toBuffer();
}

const PLAINS = 40;
const DESERT = 14;

test("the decoder reads each container shape the reference writer emits", () => {
  const c = newColumn(); // every section untouched: a single value
  // Section 4: one biome written over the empty one (a one-bit local palette).
  for (let i = 0; i < BIOMES_PER_SECTION; i++) {
    c.setBiome({ x: (i & 3) << 2, y: MIN_Y + 4 * 16 + (((i >> 4) & 3) << 2), z: ((i >> 2) & 3) << 2 }, PLAINS);
  }
  // Section 5: three biomes (a two-bit local palette).
  c.setBiome({ x: 0, y: MIN_Y + 5 * 16, z: 0 }, PLAINS);
  c.setBiome({ x: 4, y: MIN_Y + 5 * 16 + 8, z: 12 }, DESERT);
  // Section 6: twelve biomes (past the local palette's three bits: the global registry).
  for (let i = 0; i < 12; i++) c.setBiome({ x: (i & 3) << 2, y: MIN_Y + 6 * 16 + ((i >> 2) << 2), z: 0 }, i);
  const data = encode(c);
  // The shapes the writer chose, read off its own bytes: the test binds all three.
  const bits = (section: number): number => {
    const one = newColumn();
    one.biomes[0] = c.biomes[section]!;
    return encode(one)[0]!;
  };
  assert.deepEqual([bits(0), bits(4), bits(5), bits(6)], [0, 1, 2, 8]);
  const ids = decodeChunkBiomes(data, SECTIONS);
  assert.equal(ids.length, SECTIONS);
  let compared = 0;
  for (let s = 0; s < SECTIONS; s++) {
    assert.equal(ids[s]!.length, BIOMES_PER_SECTION);
    for (let i = 0; i < BIOMES_PER_SECTION; i++) {
      const pos = { x: (i & 3) << 2, y: MIN_Y + (s << 4) + (((i >> 4) & 3) << 2), z: ((i >> 2) & 3) << 2 };
      assert.equal(ids[s]![i], c.getBiome(pos), `section ${s} cell ${i}`);
      compared++;
    }
  }
  assert.equal(compared, SECTIONS * BIOMES_PER_SECTION);
});

test("a truncated or overlong payload is refused, never half-read", () => {
  const c = newColumn();
  fill(c, PLAINS);
  const data = encode(c);
  assert.throws(() => decodeChunkBiomes(data.subarray(0, data.length - 1), SECTIONS), ChunkBiomesDecodeError);
  assert.throws(() => decodeChunkBiomes(Buffer.concat([data, Buffer.from([0])]), SECTIONS), ChunkBiomesDecodeError);
});

test("a harness bot's world takes a repaint of a chunk it already holds", async () => {
  // The real pinned mineflayer, on a client that is never connected.
  const client = Object.assign(new EventEmitter(), {
    version: VERSION,
    write() {},
    end() {},
    writeChannel() {},
    registerChannel() {},
  });
  const { bot, biomes } = createHarnessBot({ client: client as never, username: "probe", auth: "offline" });
  await new Promise((r) => setImmediate(r));
  // The world as a loaded chunk leaves it: (0, 0) all plains.
  const World = require("prismarine-world")(VERSION) as new () => { sync: unknown };
  const world = new World().sync as {
    setColumn(x: number, z: number, c: Column): void;
    getBiome(p: { x: number; y: number; z: number }): number;
  };
  const held = newColumn();
  fill(held, PLAINS);
  world.setColumn(0, 0, held);
  const b = bot as unknown as { world: unknown; game: { minY: number; height: number } };
  b.world = world;
  b.game.minY = MIN_Y;
  b.game.height = HEIGHT;
  const eye = { x: 6.5, y: 65.5, z: 9.5 };
  assert.equal(world.getBiome(eye), PLAINS);

  // `fillbiome` paints the column desert from y 64 up; the server sends the
  // whole column's containers for the held chunk, and one for a chunk not held.
  const painted = newColumn();
  fill(painted, PLAINS);
  for (let y = 64; y < MIN_Y + HEIGHT; y += 4) {
    for (let x = 0; x < 16; x += 4) for (let z = 0; z < 16; z += 4) painted.setBiome({ x, y, z }, DESERT);
  }
  const data = encode(painted);
  client.emit("chunk_biomes", {
    biomes: [
      { position: { x: 0, z: 0 }, data },
      { position: { x: 5, z: 5 }, data },
    ],
  });

  assert.equal(world.getBiome(eye), DESERT, "the station reads the repaint, not the biome the chunk loaded with");
  assert.equal(world.getBiome({ x: 6.5, y: 60, z: 9.5 }), PLAINS, "below the painted volume is untouched");
  assert.equal(biomes.applied(), 1);
  assert.equal(biomes.unheld(), 1);
});
