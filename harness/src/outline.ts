// A block state's outline: the box the client's crosshair pick meets.
//
// The pinned jar's `BlockState.getShape` is not a block's collision shape: a
// lever, a button and a sign have no collision and a real outline, and the
// outline is what the pick ray hits (spec-0117 §2b). mineflayer carries the
// collision boxes only, so the harness reads the outline from the engine's own
// table, `delvewright_dsl::blockshape::outline`, written for it as
// `src/data/outline-shapes-1.21.11.json` by `crates/delvec/tests/outline_ledger.rs`
// — which refuses when the committed file is not what the engine writes. The
// harness never states a shape of its own.
//
// The table is dense per block over the properties that move the outline,
// each with its values; a live block is indexed by its own properties, the
// first named property most significant.

import { readFileSync } from "node:fs";

/** One box, in blocks from the cell's minimum corner: `[x0, y0, z0, x1, y1, z1]`. */
export type OutlineBox = readonly [number, number, number, number, number, number];

interface BlockRow {
  readonly properties: readonly (readonly [string, readonly string[]])[];
  readonly shapes: readonly number[];
}

interface OutlineTable {
  readonly version: string;
  readonly states: number;
  readonly shapes: readonly (readonly OutlineBox[])[];
  readonly blocks: Readonly<Record<string, BlockRow>>;
}

/** Where the engine writes the table, beside this module. */
export const OUTLINE_TABLE_URL = new URL("./data/outline-shapes-1.21.11.json", import.meta.url);

let table: OutlineTable | undefined;

function load(): OutlineTable {
  table ??= JSON.parse(readFileSync(OUTLINE_TABLE_URL, "utf8")) as OutlineTable;
  return table;
}

/** How many blocks and states the table stands for — its binding count. */
export function outlineTableCounts(): { blocks: number; states: number } {
  const t = load();
  return { blocks: Object.keys(t.blocks).length, states: t.states };
}

/**
 * The pinned outline of a block state, or `undefined` when the table does not
 * hold the block or the state names a property value it does not list. An
 * empty list is a state with no outline (air, a fluid): the ray passes through.
 *
 * `name` may be bare (`lever`, as mineflayer names a block) or namespaced;
 * `properties` are the block's live values, any scalar type.
 */
export function outlineOf(
  name: string,
  properties: Readonly<Record<string, unknown>>,
): readonly OutlineBox[] | undefined {
  const t = load();
  const row = t.blocks[name.includes(":") ? name : `minecraft:${name}`];
  if (!row) return undefined;
  let index = 0;
  for (const [prop, values] of row.properties) {
    const v = properties[prop];
    const at = v === undefined ? -1 : values.indexOf(String(v));
    if (at < 0) return undefined;
    index = index * values.length + at;
  }
  const shape = row.shapes[index];
  return shape === undefined ? undefined : t.shapes[shape];
}
