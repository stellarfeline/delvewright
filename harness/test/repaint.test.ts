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

/** Where the bot stands in every test unless it says otherwise: chunk (0, 0). */
const HERE: [number, number, number] = [8, 64, 8];

/** A watch whose client asks for 32 chunks and whose server serves `served`. */
function watch(served = 10): RepaintWatch {
  const w = new RepaintWatch(plan, 32);
  w.packet("login", { viewDistance: served }, -1);
  return w;
}

function loaded(w: RepaintWatch, at: number): void {
  w.packet("map_chunk", { x: 0, z: 0 }, at);
  w.packet("map_chunk", { x: 0, z: 1 }, at);
}

test("a repaint whose every held chunk is told by chunk_biomes passes", () => {
  const w = watch();
  loaded(w, 0);
  w.marker("obj/ring-the-bell", 1_000, HERE);
  w.packet("chunk_biomes", { biomes: [{ position: { x: 0, z: 0 } }, { position: { x: 0, z: 1 } }] }, 1_005);
  const [v, unmarked] = w.verdicts();
  assert.equal(v!.performed, true);
  assert.equal(v!.held, 2);
  assert.equal(v!.told, 2);
  assert.equal(v!.failure, undefined);
  assert.equal(unmarked!.performed, false);
  assert.match(
    repaintBindingLine(plan, w.verdicts()),
    /2 in the build, 1 with a marker, 1 performed; 2 bound chunk\(s\) \(held inside the served view distance\) of 2 owed of 2 painted; 2 of 2 held chunk\(s\) told by chunk_biomes, 0 resent/,
  );
});

test("a held chunk the packet never named reds", () => {
  const w = watch();
  loaded(w, 0);
  w.marker("obj/ring-the-bell", 1_000, HERE);
  w.packet("chunk_biomes", { biomes: [{ position: { x: 0, z: 0 } }] }, 1_005);
  assert.match(w.verdicts()[0]!.failure ?? "", /no chunk_biomes named chunk\(s\) 0,1/);
});

test("a chunk resent by map_chunk after the repaint reds", () => {
  const w = watch();
  loaded(w, 0);
  w.marker("obj/ring-the-bell", 1_000, HERE);
  w.packet("map_chunk", { x: 0, z: 1 }, 1_002);
  w.packet("chunk_biomes", { biomes: [{ position: { x: 0, z: 0 } }, { position: { x: 0, z: 1 } }] }, 1_005);
  assert.match(w.verdicts()[0]!.failure ?? "", /map_chunk resent chunk\(s\) 0,1/);
});

test("a repaint over chunks the client never held is a zero binding, and reds", () => {
  const w = watch();
  w.marker("obj/ring-the-bell", 1_000, HERE);
  const v = w.verdicts()[0]!;
  assert.equal(v.held, 0);
  assert.match(v.failure ?? "", /held none/);
});

test("a chunk unloaded before the repaint is not counted as held", () => {
  const w = watch();
  loaded(w, 0);
  w.packet("unload_chunk", { chunkX: 0, chunkZ: 1 }, 500);
  w.marker("obj/ring-the-bell", 1_000, HERE);
  w.packet("chunk_biomes", { biomes: [{ position: { x: 0, z: 0 } }] }, 1_005);
  const v = w.verdicts()[0]!;
  assert.equal(v.held, 1);
  assert.equal(v.failure, undefined);
});

test("chunks held only beyond the served view distance bind nothing, held or not", () => {
  // the-stranding r6: a sea-fog repaint ~490 blocks from the bot at a 30-chunk
  // served distance read 0 of 76 chunks held on one run and 24 of 76 on another,
  // and the second PASSED on the chunks the server happened to have streamed at
  // its margin. Scaled down: served 2, the volume 2 and 3 chunks away.
  const far = parseRepaintPlan({
    repaints: [{ effect: "/e", biome: "demo:fog", after: "obj/notice", chunks: [[2, 0], [3, 0]] }],
  });
  for (const fringeHeld of [false, true]) {
    const w = new RepaintWatch(far, 32);
    w.packet("login", { viewDistance: 2 }, -1);
    if (fringeHeld) w.packet("map_chunk", { x: 2, z: 0 }, 0);
    w.marker("obj/notice", 1_000, HERE);
    if (fringeHeld) w.packet("chunk_biomes", { biomes: [{ position: { x: 2, z: 0 } }] }, 1_005);
    const v = w.verdicts()[0]!;
    assert.equal(v.owed, 0, `nothing is owed (fringe held: ${fringeHeld})`);
    assert.equal(v.bound, 0);
    assert.match(v.failure ?? "", /beyond the 2-chunk view distance .* nothing binds/, `refused either way: ${v.failure}`);
  }
});

test("the client is served the lesser of what it asks for and what the server serves", () => {
  const w = new RepaintWatch(plan, 12);
  assert.equal(w.served(), undefined, "unknown until the server says");
  w.packet("login", { viewDistance: 30 }, 0);
  assert.equal(w.served(), 12);
  w.packet("update_view_distance", { viewDistance: 8 }, 1);
  assert.equal(w.served(), 8);
});

test("a respawn drops every chunk the client held; only what the server resends is held", () => {
  // The client discards its level on `respawn` and the server tracks the new body
  // from nothing, sending no `unload_chunk` for what was dropped. A ledger that
  // kept the old chunks expected a chunk_biomes for one the server no longer
  // tracks for this body.
  const w = watch();
  loaded(w, 0);
  w.packet("respawn", {}, 100);
  w.packet("map_chunk", { x: 0, z: 0 }, 200);
  w.marker("obj/ring-the-bell", 1_000, HERE);
  w.packet("chunk_biomes", { biomes: [{ position: { x: 0, z: 0 } }] }, 1_005);
  const v = w.verdicts()[0]!;
  assert.equal(v.held, 1, "only the resent chunk is held");
  assert.equal(v.bound, 1);
  assert.equal(v.failure, undefined);
});

test("a marker whose standpoint is unknown binds nothing", () => {
  const w = watch();
  loaded(w, 0);
  w.marker("obj/ring-the-bell", 1_000);
  w.packet("chunk_biomes", { biomes: [{ position: { x: 0, z: 0 } }, { position: { x: 0, z: 1 } }] }, 1_005);
  assert.match(w.verdicts()[0]!.failure ?? "", /where the bot stood .* unknown/);
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
