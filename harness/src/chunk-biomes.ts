// A repaint, as the client's world receives it.
//
// `fillbiome` reaches a client that already holds the chunk as a clientbound
// `chunk_biomes` packet: per chunk, the biome paletted container of every
// section, written as the chunk data writes them. mineflayer 4.37.1
// (`lib/plugins/blocks.js`) handles `map_chunk` and nothing else, so without
// this its world keeps the biome a chunk carried when it loaded, and every
// read of `bot.world.getBiome` after a repaint of a loaded chunk names the
// biome from before it.
//
// This decodes the packet into the bot's own world, the way the vanilla client
// replaces each section's biomes (`ClientPacketListener.handleChunksBiomes`).
// The container layout is the 1.21.5+ one the pinned protocol writes: a bits
// byte; a single value (0 bits), a local palette (1–3 bits) or the global
// registry (more); then the packed longs with no length prefix, their count
// computed from the bits, values never spanning a long.
//
// Navigation state only — no game logic (CLAUDE.md).

/** Biome cells per section: 4 × 4 × 4. */
export const BIOMES_PER_SECTION = 64;

/** The widest bits a biome container stores in a local palette. */
const MAX_LOCAL_BITS = 3;

/** Raised when a `chunk_biomes` payload does not read as the pinned layout. */
export class ChunkBiomesDecodeError extends Error {
  override readonly name = "ChunkBiomesDecodeError";
}

class Reader {
  private at = 0;
  private readonly buf: Uint8Array;
  constructor(buf: Uint8Array) {
    this.buf = buf;
  }

  get done(): boolean {
    return this.at >= this.buf.length;
  }

  u8(): number {
    if (this.at >= this.buf.length) throw new ChunkBiomesDecodeError("ran out of bytes");
    return this.buf[this.at++]!;
  }

  varint(): number {
    let value = 0;
    for (let shift = 0; shift < 35; shift += 7) {
      const b = this.u8();
      value |= (b & 0x7f) << shift;
      if ((b & 0x80) === 0) return value >>> 0;
    }
    throw new ChunkBiomesDecodeError("varint longer than five bytes");
  }

  long(): bigint {
    let v = 0n;
    for (let i = 0; i < 8; i++) v = (v << 8n) | BigInt(this.u8());
    return v;
  }
}

/**
 * Decode one chunk's `data`: the raw biome ids of every section, bottom up,
 * each 64 entries in vanilla's index order `(y << 2 | z) << 2 | x`.
 */
export function decodeChunkBiomes(data: Uint8Array, sections: number): number[][] {
  const r = new Reader(data);
  const out: number[][] = [];
  for (let s = 0; s < sections; s++) {
    const bits = r.u8();
    if (bits === 0) {
      out.push(new Array<number>(BIOMES_PER_SECTION).fill(r.varint()));
      continue;
    }
    let palette: number[] | undefined;
    if (bits <= MAX_LOCAL_BITS) {
      const n = r.varint();
      palette = [];
      for (let i = 0; i < n; i++) palette.push(r.varint());
    }
    // ceil(64 / perLong) longs: one is read wherever an entry starts a new one.
    const perLong = Math.floor(64 / bits);
    const mask = (1n << BigInt(bits)) - 1n;
    const ids: number[] = [];
    let word = 0n;
    for (let i = 0; i < BIOMES_PER_SECTION; i++) {
      if (i % perLong === 0) word = r.long();
      const raw = Number((word >> BigInt((i % perLong) * bits)) & mask);
      if (palette) {
        const id = palette[raw];
        if (id === undefined) {
          throw new ChunkBiomesDecodeError(`section ${s}: palette index ${raw} of ${palette.length}`);
        }
        ids.push(id);
      } else {
        ids.push(raw);
      }
    }
    out.push(ids);
  }
  if (!r.done) {
    throw new ChunkBiomesDecodeError(`${sections} section(s) read and bytes remain`);
  }
  return out;
}

/** The slice of a chunk column this module writes. */
export interface BiomeColumn {
  setBiome(pos: { x: number; y: number; z: number }, biome: number): void;
}

/** The slice of a mineflayer bot this module reads and writes. */
export interface BiomeBot {
  readonly _client: {
    on(event: "chunk_biomes", listener: (packet: Record<string, unknown>) => void): unknown;
  };
  readonly game: { minY: number; height: number };
  readonly world: { getColumn(chunkX: number, chunkZ: number): unknown };
}

/** What the handler did, for a run to state. */
export interface ChunkBiomesState {
  /** Chunks a `chunk_biomes` named that the bot's world held, and were rewritten. */
  applied(): number;
  /** Chunks a `chunk_biomes` named that the bot's world did not hold. */
  unheld(): number;
}

/**
 * Apply one chunk's biomes to its column. The packet's data is every section
 * the dimension has, in order from `minY`.
 */
export function applyChunkBiomes(column: BiomeColumn, data: Uint8Array, minY: number, height: number): void {
  const sections = height >> 4;
  const ids = decodeChunkBiomes(data, sections);
  for (let s = 0; s < sections; s++) {
    const section = ids[s]!;
    for (let i = 0; i < BIOMES_PER_SECTION; i++) {
      const x = (i & 3) << 2;
      const z = ((i >> 2) & 3) << 2;
      const y = minY + (s << 4) + (((i >> 4) & 3) << 2);
      column.setBiome({ x, y, z }, section[i]!);
    }
  }
}

/**
 * Keep the bot's world's biomes current with every `chunk_biomes` the server
 * sends. Install it in the turn the bot is created.
 */
export function installChunkBiomes(bot: BiomeBot): ChunkBiomesState {
  let applied = 0;
  let unheld = 0;
  bot._client.on("chunk_biomes", (packet) => {
    const entries = packet["biomes"];
    if (!Array.isArray(entries)) return;
    for (const entry of entries) {
      const e = entry as { position?: { x?: unknown; z?: unknown }; data?: unknown };
      const x = e.position?.x;
      const z = e.position?.z;
      if (typeof x !== "number" || typeof z !== "number" || !(e.data instanceof Uint8Array)) {
        throw new ChunkBiomesDecodeError("a chunk_biomes entry carries no position or data");
      }
      const column = bot.world.getColumn(x, z) as BiomeColumn | undefined | null;
      if (!column) {
        unheld += 1;
        continue;
      }
      applyChunkBiomes(column, e.data, bot.game.minY, bot.game.height);
      applied += 1;
    }
  });
  return { applied: () => applied, unheld: () => unheld };
}
