import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, mkdir, writeFile, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import {
  RepaintPlanParseError,
  RepaintWatch,
  loadRepaintPlanForCriticalPath,
  parseRepaintPlan,
  repaintBindingLine,
} from "../src/repaint.ts";
import { within } from "./bounded.ts";

// spec-0080 §5.2: a repaint reaches the client as a `chunk_biomes` naming every
// held chunk of its volume, and never as a `map_chunk` resend. Each failure mode
// is planted here and must red: a chunk the packet missed, a reload, a repaint
// whose volume the client held none of (a zero binding is not a pass).

const plan = parseRepaintPlan({
  repaints: [
    {
      effect: "/content/quests/0/objectives/1/on_complete/0",
      biome: "demo:atmosphere/wrong-place",
      after: "obj/ring-the-bell",
      chunks: [
        [0, 0],
        [0, 1],
      ],
    },
    {
      effect: "/content/triggers/0/effects/0",
      biome: "demo:atmosphere/wrong-place",
      after: null,
      chunks: [[5, 5]],
    },
  ],
});

function loaded(w: RepaintWatch, at: number): void {
  w.packet("map_chunk", { x: 0, z: 0 }, at);
  w.packet("map_chunk", { x: 0, z: 1 }, at);
}

test("a repaint whose every held chunk is told by chunk_biomes passes", () => {
  const w = new RepaintWatch(plan);
  loaded(w, 0);
  w.marker("obj/ring-the-bell", 1_000);
  w.packet("chunk_biomes", { biomes: [{ position: { x: 0, z: 0 } }, { position: { x: 0, z: 1 } }] }, 1_005);
  const [v, unmarked] = w.verdicts();
  assert.equal(v!.performed, true);
  assert.equal(v!.held, 2);
  assert.equal(v!.told, 2);
  assert.equal(v!.failure, undefined);
  assert.equal(unmarked!.performed, false);
  assert.match(
    repaintBindingLine(plan, w.verdicts()),
    /2 in the build, 1 with a marker, 1 performed; 2 of 2 held chunk\(s\) told by chunk_biomes, 0 resent/,
  );
});

test("a held chunk the packet never named reds", () => {
  const w = new RepaintWatch(plan);
  loaded(w, 0);
  w.marker("obj/ring-the-bell", 1_000);
  w.packet("chunk_biomes", { biomes: [{ position: { x: 0, z: 0 } }] }, 1_005);
  assert.match(w.verdicts()[0]!.failure ?? "", /no chunk_biomes named chunk\(s\) 0,1/);
});

test("a chunk resent by map_chunk after the repaint reds", () => {
  const w = new RepaintWatch(plan);
  loaded(w, 0);
  w.marker("obj/ring-the-bell", 1_000);
  w.packet("map_chunk", { x: 0, z: 1 }, 1_002);
  w.packet("chunk_biomes", { biomes: [{ position: { x: 0, z: 0 } }, { position: { x: 0, z: 1 } }] }, 1_005);
  assert.match(w.verdicts()[0]!.failure ?? "", /map_chunk resent chunk\(s\) 0,1/);
});

test("a repaint over chunks the client never held is a zero binding, and reds", () => {
  const w = new RepaintWatch(plan);
  w.marker("obj/ring-the-bell", 1_000);
  const v = w.verdicts()[0]!;
  assert.equal(v.held, 0);
  assert.match(v.failure ?? "", /held none/);
});

test("a chunk unloaded before the repaint is not counted as held", () => {
  const w = new RepaintWatch(plan);
  loaded(w, 0);
  w.packet("unload_chunk", { chunkX: 0, chunkZ: 1 }, 500);
  w.marker("obj/ring-the-bell", 1_000);
  w.packet("chunk_biomes", { biomes: [{ position: { x: 0, z: 0 } }] }, 1_005);
  const v = w.verdicts()[0]!;
  assert.equal(v.held, 1);
  assert.equal(v.failure, undefined);
});

test("a malformed export is refused", () => {
  assert.throws(() => parseRepaintPlan({ repaints: [{ effect: "x", biome: "y", after: 3, chunks: [[0, 0]] }] }), RepaintPlanParseError);
  assert.throws(() => parseRepaintPlan({ repaints: [{ effect: "x", biome: "y", after: null, chunks: [] }] }), RepaintPlanParseError);
});

test("the export is read from the build tree's validation/ beside the critical path", async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), "repaint-plan-"));
  try {
    const cp = path.join(dir, "critical-path.json");
    await writeFile(cp, "{}");
    assert.equal(
      await within("loadRepaintPlanForCriticalPath(absent)", loadRepaintPlanForCriticalPath(cp)),
      undefined,
      "absent means no repaint",
    );
    await mkdir(path.join(dir, "validation"));
    await writeFile(
      path.join(dir, "validation", "atmosphere-repaints.json"),
      JSON.stringify({ repaints: [{ effect: "e", biome: "b", after: "obj/x", chunks: [[1, 2]] }] }),
    );
    const plan = await within("loadRepaintPlanForCriticalPath(present)", loadRepaintPlanForCriticalPath(cp));
    assert.equal(plan?.repaints.length, 1, "the build's own validation/ export is found");
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});
