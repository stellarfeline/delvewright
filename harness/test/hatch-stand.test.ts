import { test } from "node:test";
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  HATCH_POS_COMMAND,
  hatchBindingLine,
  judgeStand,
  parseHatchPlan,
  parseHatchPos,
  tpCommand,
  type StandReading,
} from "../src/hatch-stand.ts";

const ledger = {
  gates_examined: 3,
  gates: [
    // A shut 2x2 hatch: a floor gate.
    {
      anchor: "anchor/seam-shaft",
      from: [20, 63, 21],
      to: [21, 63, 22],
      sealed: true,
      stand: { feet_y: 64, points: [[20.5, 21.5], [21, 22]] },
    },
    // A wall gate: no stand.
    { anchor: "anchor/door", from: [0, 64, 0], to: [2, 66, 0], sealed: true, stand: null },
    // A floor gate the world builds open: nothing shut to stand on.
    { anchor: "anchor/open", from: [5, 63, 5], to: [5, 63, 5], sealed: false, stand: { feet_y: 64, points: [[5.5, 5.5]] } },
  ],
};

test("only a sealed gate with a stand is a floor gate", () => {
  const plan = parseHatchPlan(ledger);
  assert.equal(plan.examined, 3);
  assert.deepEqual(
    plan.gates.map((g) => [g.anchor, g.courseY, g.feetY, g.points.length]),
    [["anchor/seam-shaft", 63, 64, 2]],
  );
});

test("a stand with no point is refused, never read as nothing to stand on", () => {
  const bad = structuredClone(ledger);
  bad.gates[0]!.stand = { feet_y: 64, points: [] };
  assert.throws(() => parseHatchPlan(bad), /no point/);
});

const held: StandReading = {
  anchor: "anchor/seam-shaft",
  at: [21, 22],
  feetY: 64,
  clientY: 64,
  serverY: 64,
  died: false,
};

test("a body on the gate is held; one below it fell, on either reading", () => {
  assert.equal(judgeStand(held), undefined);
  assert.match(judgeStand({ ...held, clientY: 60, serverY: 60 })!, /fell through/);
  assert.match(judgeStand({ ...held, serverY: 60 })!, /server holds the body's feet at y=60\.000/);
  assert.match(judgeStand({ ...held, serverY: undefined })!, /never answered/);
  assert.match(judgeStand({ ...held, died: true })!, /died/);
  // A cover lying on a floor holds the feet 3/16 up.
  assert.equal(judgeStand({ ...held, feetY: 64.1875, clientY: 64.1875, serverY: 64.1875 }), undefined);
});

test("the binding line counts what it stood on out of what was examined", () => {
  assert.equal(
    hatchBindingLine(parseHatchPlan(ledger), 1, 1),
    "hatch-stand binding: 1 shut floor gate(s) of 3 gate(s) examined, 2 stand point(s), 1 stood, 1 fell or failed",
  );
});

test("a stand point between two cells is not moved onto a cell centre", () => {
  // `/tp @s 21 64 22` puts the feet at (21.5, 22.5): the game centres a bare integer.
  assert.equal(tpCommand(21, 64, 22), "/tp @s 21.0000 64.0000 22.0000");
  assert.equal(tpCommand(18.5, 64.1875, 24.5), "/tp @s 18.5000 64.1875 24.5000");
});

test("the server's feet height is read from the reading's own reply", () => {
  assert.equal(parseHatchPos(["[dw-time 3 1200]", "HATCHPOS 64.0d"]), 64);
  assert.equal(parseHatchPos(["HATCHPOS -12.5d"]), -12.5);
  assert.equal(parseHatchPos(["[dw-time 3 1200]"]), undefined);
  assert.match(HATCH_POS_COMMAND, /^\/tellraw @s \["HATCHPOS ",\{"nbt":"Pos\[1\]","entity":"@s"\}\]$/);
});

// Every wait for the server's game ticks is judged by the game clock
// (`GameClockWatch`); mineflayer's `bot.waitForTicks` counts the client's own
// physics ticks, which the host's wall clock drives.
test("no harness source waits on the client's ticks", () => {
  const src = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "src");
  const files: string[] = [];
  const walk = (d: string): void => {
    for (const e of readdirSync(d, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
      const p = path.join(d, e.name);
      if (e.isDirectory()) walk(p);
      else if (e.name.endsWith(".ts")) files.push(p);
    }
  };
  walk(src);
  assert.ok(files.length > 0, "the scan bound no source file");
  const offenders = files.filter((f) => /\.waitForTicks\(/.test(readFileSync(f, "utf8")));
  assert.deepEqual(offenders, [], `${files.length} file(s) scanned`);
});
