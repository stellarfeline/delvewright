// The harness's one block-state parse rule, judged on the committed fixture the
// Python reader (`check-written-world.py`'s `split`) is judged on too.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  blockStateSatisfies,
  blockStateText,
  parseBlockState,
  readBlockState,
} from "../src/block-state.ts";

const FIXTURE = JSON.parse(
  readFileSync(new URL("./fixtures/block-states.json", import.meta.url), "utf8"),
) as {
  parse: { text: string; name: string; properties: Record<string, string> }[];
  satisfies: { stated: string; read: string; holds: boolean }[];
};

test("every fixture row parses to its name and properties", () => {
  assert.ok(FIXTURE.parse.length > 0);
  for (const row of FIXTURE.parse) {
    assert.deepEqual(parseBlockState(row.text), { name: row.name, properties: row.properties }, row.text);
  }
});

test("a stated block is satisfied by exactly the readings the fixture says", () => {
  assert.ok(FIXTURE.satisfies.length > 0);
  for (const row of FIXTURE.satisfies) {
    assert.equal(blockStateSatisfies(row.stated, row.read), row.holds, `${row.stated} vs ${row.read}`);
  }
});

test("a mineflayer reading becomes the canonical text a stated state is judged against", () => {
  const read = readBlockState("white_candle", { lit: true, candles: 3, waterlogged: false });
  assert.equal(read, "minecraft:white_candle[candles=3,lit=true,waterlogged=false]");
  assert.ok(blockStateSatisfies("minecraft:white_candle[candles=3,lit=true,waterlogged=false]", read));
  assert.equal(blockStateText(parseBlockState("minecraft:stone")), "minecraft:stone");
});
