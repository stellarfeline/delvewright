#!/usr/bin/env node
// SPIKE TOOLING (block settling) — NOT part of the shipped pipeline, not wired
// into CI. Run by `tools/spike-block-settling/run.sh`.
//
// Every mutation's response is checked (`ok()`); a census that legitimately
// counts zero goes through `raw()` and parses the reply instead: a command
// whose response nobody reads cannot fail.
//
// The world is a DRY superflat — bedrock 1 + stone 3 — so everything above
// y=5 is air and no ambient water can reach a rig.

import { spawn } from "node:child_process";
import { writeFileSync } from "node:fs";
import readline from "node:readline";

import { assertAccepted } from "../lib/rcon.mjs";

const CONTAINER = process.env.SPIKE_CONTAINER ?? "dw-spike-block-settling";
const OUT = process.env.SPIKE_OUT ?? new URL("./observations.json", import.meta.url).pathname;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// ---------------------------------------------------------------- rcon channel
// One long-lived `rcon-cli` on stdin, commands PIPELINED and each batch fenced
// by a #sync scoreboard write/read. Lifted from `tools/spike-fluid-plane`,
// including its two hardenings: a dead channel rejects the pending read (an
// empty event loop exits 0, which once produced a "successful" run with no
// observations at all), and channel start is a retried handshake.
let proc = null;
let inbox = [];
let notify = null;
let channelDown = null;
function startChannel() {
  inbox = [];
  channelDown = null;
  proc = spawn("docker", ["exec", "-i", CONTAINER, "rcon-cli"], {
    stdio: ["pipe", "pipe", "inherit"],
  });
  const rl = readline.createInterface({ input: proc.stdout });
  rl.on("line", (line) => {
    inbox.push(line.replace(/^>\s?/, ""));
    if (notify) {
      const n = notify;
      notify = null;
      n.resolve();
    }
  });
  proc.on("close", (code) => {
    channelDown = new Error(`rcon channel closed (exit ${code})`);
    if (notify) {
      const n = notify;
      notify = null;
      n.reject(channelDown);
    }
  });
  proc.stdin.on("error", () => {});
}
async function readLines(n) {
  while (inbox.length < n) {
    if (channelDown) throw channelDown;
    await new Promise((resolve, reject) => (notify = { resolve, reject }));
  }
  return inbox.splice(0, n);
}
async function connectChannel() {
  for (let attempt = 1; attempt <= 24; attempt++) {
    startChannel();
    try {
      proc.stdin.write("list\n");
      await Promise.race([
        readLines(1),
        new Promise((_, rej) => setTimeout(() => rej(new Error("handshake timeout")), 10000)),
      ]);
      // The fence every batch below is bracketed by. It has to exist before the
      // first batch, so it is written and read here, outside the fence.
      proc.stdin.write("scoreboard objectives add dw.sync dummy\nscoreboard players set #sync dw.sync 0\n");
      await readLines(2);
      inbox = [];
      return;
    } catch (e) {
      console.error(`[measure] rcon handshake attempt ${attempt}: ${e.message ?? e}`);
      try {
        proc.kill();
      } catch {}
      await sleep(5000);
    }
  }
  throw new Error("rcon channel never came up");
}
let syncN = 0;
async function batch(cmds) {
  const n = ++syncN;
  const all = [
    `scoreboard players set #sync dw.sync ${n}`,
    ...cmds,
    "scoreboard players get #sync dw.sync",
  ];
  proc.stdin.write(all.join("\n") + "\n");
  const out = await readLines(all.length);
  if (!out[out.length - 1].includes(`#sync has ${n} `)) {
    throw new Error(`rcon desync at ${n}; responses were ${JSON.stringify(out)}`);
  }
  return out.slice(1, -1);
}
const raw = async (cmd) => (await batch([cmd]))[0];
async function ok(cmd) {
  return assertAccepted(cmd, await raw(cmd));
}
/** Run many commands in chunks, checking every reply. */
async function okAll(cmds, chunk = 200) {
  for (let i = 0; i < cmds.length; i += chunk) {
    const slice = cmds.slice(i, i + chunk);
    const replies = await batch(slice);
    slice.forEach((c, j) => assertAccepted(c, replies[j]));
  }
}

// A conditional-execute used as a boolean: the payload answers on a match and
// says nothing at all when the condition fails.
const PROBE = "run time query gametime";
function flag(line) {
  if (line.startsWith("The time is")) return true;
  if (line === "") return false;
  throw new Error(`boolean probe did not evaluate: ${JSON.stringify(line)}`);
}
/** Run many boolean probes, in order. */
async function flags(cmds, chunk = 200) {
  const out = [];
  for (let i = 0; i < cmds.length; i += chunk) {
    const replies = await batch(cmds.slice(i, i + chunk));
    for (const r of replies) out.push(flag(r));
  }
  return out;
}
/** "Successfully filled N block(s)" -> N; "No blocks were filled" -> 0. */
function filled(line) {
  const m = line.match(/Successfully filled (\d+) block/);
  if (m) return Number(m[1]);
  if (line.startsWith("No blocks were filled")) return 0;
  throw new Error(`not a fill reply: ${JSON.stringify(line)}`);
}

// ------------------------------------------------------------- the stair field
//
// WHY A RANDOM FIELD. The derivation reads five cells (the stair, its front,
// its back, and one cell to each side) and cares only about their `facing` and
// `half` — so a field of random stairs walks a large sample of the whole input
// space at once, and every cell is one measured case. Deterministic PRNG, so
// the field is a fact this file states rather than one a run invents.
const SEED = 20260814;
function mulberry32(a) {
  return function () {
    a |= 0;
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
const FIELD_ORIGIN = [0, 100, 0];
const FIELD_N = 31;
// Two DIFFERENT stair blocks on purpose: vanilla's neighbour test is "is this a
// stair", not "is this the same block", and a field of one block could not tell
// the two readings apart.
const STAIR_BLOCKS = ["minecraft:oak_stairs", "minecraft:stone_brick_stairs"];
const FACINGS = ["north", "south", "east", "west"];
const HALVES = ["bottom", "top"];
const SHAPES = ["straight", "inner_left", "inner_right", "outer_left", "outer_right"];

function buildField() {
  const rnd = mulberry32(SEED);
  const cells = [];
  for (let ix = 0; ix < FIELD_N; ix++) {
    for (let iz = 0; iz < FIELD_N; iz++) {
      const x = FIELD_ORIGIN[0] + ix;
      const z = FIELD_ORIGIN[2] + iz;
      if (rnd() < 0.2) {
        cells.push({ x, z, block: "minecraft:air" });
        continue;
      }
      cells.push({
        x,
        z,
        block: STAIR_BLOCKS[Math.floor(rnd() * STAIR_BLOCKS.length)],
        facing: FACINGS[Math.floor(rnd() * FACINGS.length)],
        half: HALVES[Math.floor(rnd() * HALVES.length)],
        // Authored as `straight` everywhere: whatever comes back that is not
        // `straight` was derived by the game and by nothing else.
        authored_shape: "straight",
      });
    }
  }
  return cells;
}
const stateOf = (c) =>
  c.block === "minecraft:air"
    ? "minecraft:air"
    : `${c.block}[facing=${c.facing},half=${c.half},shape=${c.authored_shape},waterlogged=false]`;

async function measureStairField() {
  const y = FIELD_ORIGIN[1];
  const cells = buildField();
  await ok(
    `forceload add ${FIELD_ORIGIN[0] - 16} ${FIELD_ORIGIN[2] - 16} ${FIELD_ORIGIN[0] + FIELD_N + 16} ${FIELD_ORIGIN[2] + FIELD_N + 16}`,
  );
  // A pre-clear, and NOT through `ok`: on a dry superflat this box is already
  // air, and "No blocks were filled" is the honest answer to it rather than a
  // refusal. The reply is still read — it is parsed instead of discarded.
  filled(
    await raw(
      `fill ${FIELD_ORIGIN[0] - 2} ${y - 1} ${FIELD_ORIGIN[2] - 2} ${FIELD_ORIGIN[0] + FIELD_N + 1} ${y + 1} ${FIELD_ORIGIN[2] + FIELD_N + 1} minecraft:air`,
    ),
  );
  // Only the stairs are placed: the pre-clear already left every other cell
  // air, and `/setblock` answers "Could not set the block" when the state it
  // is given is the state that is already there.
  await okAll(
    cells
      .filter((c) => c.block !== "minecraft:air")
      .map((c) => `setblock ${c.x} ${y} ${c.z} ${stateOf(c)}`),
  );

  // SETTLING, and the rig's own trap.
  //
  // `/setblock` writes the state literally — the block it sets is never itself
  // re-derived — and `StairBlock.updateShape` only recomputes the shape for a
  // HORIZONTAL update direction. So a cell is re-derived exactly when a
  // horizontal NEIGHBOUR changes.
  //
  // The obvious rig (set each stair to air and back, so its neighbours
  // re-derive) settles neighbours and RESETS the poked cell to what it was
  // written as. The first run of this file did that in both directions and
  // left 10 of 758 cells carrying their authored `straight` — a number small
  // enough to read as "the implementation is wrong about ten corner cases"
  // rather than "the rig lied". So the poke never touches a stair: a temporary
  // stone is placed in a NON-stair cell beside it and removed again, which
  // re-derives every stair around that cell twice and leaves the field exactly
  // as it was placed. Every stair with at least one non-stair neighbour is
  // covered; a stair boxed in by four stairs is covered by the air/restore
  // pass, whose reset is harmless there because a later neighbour poke always
  // follows it.
  const stairs = cells.filter((c) => c.block !== "minecraft:air");
  const isStair = new Set(stairs.map((c) => `${c.x},${c.z}`));
  const pokeSites = new Set();
  for (const c of stairs) {
    for (const [dx, dz] of [
      [1, 0],
      [-1, 0],
      [0, 1],
      [0, -1],
    ]) {
      const key = `${c.x + dx},${c.z + dz}`;
      if (!isStair.has(key)) pokeSites.add(key);
    }
  }
  const stonePokes = [...pokeSites].flatMap((key) => {
    const [px, pz] = key.split(",");
    return [
      `setblock ${px} ${y} ${pz} minecraft:stone`,
      `setblock ${px} ${y} ${pz} minecraft:air`,
    ];
  });
  const airPokes = (list) =>
    list.flatMap((c) => [
      `setblock ${c.x} ${y} ${c.z} minecraft:air`,
      `setblock ${c.x} ${y} ${c.z} ${stateOf(c)}`,
    ]);
  await okAll(airPokes(stairs));
  await okAll(airPokes([...stairs].reverse()));
  await okAll(stonePokes);

  // Read back. Four probes per cell; nothing matching means `straight`.
  const probed = SHAPES.slice(1);
  const readBack = async () => {
    const cmds = stairs.flatMap((c) =>
      probed.map((s) => `execute if block ${c.x} ${y} ${c.z} ${c.block}[shape=${s}] ${PROBE}`),
    );
    const answers = await flags(cmds);
    return stairs.map((c, i) => {
      const hits = probed.filter((_, j) => answers[i * probed.length + j]);
      if (hits.length > 1) throw new Error(`${c.x},${c.z}: matched ${hits.join(" and ")}`);
      return hits[0] ?? "straight";
    });
  };
  const first = await readBack();
  // STABILITY, and it is the guard the first run of this file did not have: a
  // settled field is a fixpoint, so another round of updates must change
  // nothing. An unsettled field is exactly what moves here.
  await okAll(stonePokes);
  const second = await readBack();
  const moved = first
    .map((s, i) => (s === second[i] ? null : `${stairs[i].x},${stairs[i].z}: ${s} -> ${second[i]}`))
    .filter(Boolean);
  if (moved.length) {
    throw new Error(
      `${moved.length} cell(s) changed shape under a second round of block updates, so the ` +
        `field was not settled when it was read: ${moved.slice(0, 10).join("; ")}`,
    );
  }
  stairs.forEach((c, i) => {
    c.observed_shape = first[i];
  });

  // The rig's other falsifier. If settling had silently not happened at all,
  // every cell would read back `straight` — the authored value — and this file
  // would report "vanilla derives nothing", which is the wrong answer arrived
  // at green.
  const seen = new Set(stairs.map((c) => c.observed_shape));
  if (seen.size !== SHAPES.length) {
    throw new Error(
      `the field settled into only ${[...seen].join(", ")} — the rig did not settle, so the ` +
        `readings are the authored states, not the game's`,
    );
  }
  return { origin: FIELD_ORIGIN, size: [FIELD_N, 1, FIELD_N], seed: SEED, cells };
}

// -------------------------------------------------------------- water rigs
//
// Each rig is a stone shell with one interior cell, built well away from the
// stair field, given one water source, and read after the fluid has had time
// to move (vanilla water spreads on a 5-tick clock).
async function box(x, y, z, wall = "minecraft:stone") {
  filled(await raw(`fill ${x - 2} ${y - 2} ${z - 2} ${x + 2} ${y + 2} ${z + 2} minecraft:air`));
  await ok(`fill ${x - 1} ${y - 1} ${z - 1} ${x + 1} ${y + 1} ${z + 1} ${wall}`);
  await ok(`setblock ${x} ${y} ${z} minecraft:air`);
}
async function flowingAround(x, y, z, r = 6) {
  let n = 0;
  for (let lvl = 1; lvl <= 15; lvl++) {
    const reply = await raw(
      `fill ${x - r} ${y - r} ${z - r} ${x + r} ${y + r} ${z + r} minecraft:glass replace minecraft:water[level=${lvl}]`,
    );
    n += filled(reply);
  }
  return n;
}
async function sourcesAround(x, y, z, r = 6) {
  const reply = await raw(
    `fill ${x - r} ${y - r} ${z - r} ${x + r} ${y + r} ${z + r} minecraft:blue_stained_glass replace minecraft:water[level=0]`,
  );
  return filled(reply);
}

async function measureWater() {
  const y = 120;
  const out = {};
  // The rigs sit east of the stair field, past its forceload ring.
  await ok("forceload add 30 -16 240 16");

  // 1. Sealed. Every neighbour of the source is a full block.
  await box(40, y, 0);
  await ok(`setblock 40 ${y} 0 minecraft:water[level=0]`);
  await sleep(4000);
  out.sealed = { flowing: await flowingAround(40, y, 0), sources: await sourcesAround(40, y, 0) };

  // 2. One open neighbour: the wall cell east of the source is air.
  await box(60, y, 0);
  await ok(`setblock 61 ${y} 0 minecraft:air`);
  await ok(`setblock 60 ${y} 0 minecraft:water[level=0]`);
  await sleep(4000);
  out.open_neighbour = {
    flowing: await flowingAround(60, y, 0),
    sources: await sourcesAround(60, y, 0),
  };

  // 3. A waterloggable block, written `waterlogged=false`, IN the wall. The
  //    shell is otherwise closed, so anything that gets past this cell got
  //    past it through the block.
  await box(80, y, 0);
  await ok(`setblock 81 ${y} 0 minecraft:oak_stairs[facing=east,half=bottom,shape=straight,waterlogged=false]`);
  filled(await raw(`fill 82 ${y - 1} -1 84 ${y + 1} 1 minecraft:air`));
  await ok(`setblock 80 ${y} 0 minecraft:water[level=0]`);
  await sleep(6000);
  out.waterloggable_wall = {
    stair_waterlogged: flag(
      await raw(`execute if block 81 ${y} 0 minecraft:oak_stairs[waterlogged=true] ${PROBE}`),
    ),
    water_beyond: (await flowingAround(83, y, 0, 2)) + (await sourcesAround(83, y, 0, 2)),
  };

  // 4. The same, with iron bars — the drain-grate case: a grate at the
  //    waterline is a hole in the structure's own face.
  await box(100, y, 0);
  await ok(
    `setblock 101 ${y} 0 minecraft:iron_bars[east=false,north=true,south=true,waterlogged=false,west=false]`,
  );
  filled(await raw(`fill 102 ${y - 1} -1 104 ${y + 1} 1 minecraft:air`));
  await ok(`setblock 100 ${y} 0 minecraft:water[level=0]`);
  await sleep(6000);
  out.grate_wall = {
    bars_waterlogged: flag(
      await raw(`execute if block 101 ${y} 0 minecraft:iron_bars[waterlogged=true] ${PROBE}`),
    ),
    water_beyond: (await flowingAround(103, y, 0, 2)) + (await sourcesAround(103, y, 0, 2)),
  };

  // 5. TWO sources, one on each side of a dry waterloggable block, with solid
  //    below it. This is the case vanilla converts to a SOURCE rather than a
  //    flow — and a source is the only fluid a waterloggable block accepts.
  filled(await raw(`fill 118 ${y - 2} -2 126 ${y + 2} 2 minecraft:air`));
  await ok(`fill 119 ${y - 1} -1 125 ${y + 1} 1 minecraft:stone`);
  await ok(`fill 120 ${y} 0 124 ${y} 0 minecraft:air`);
  await ok(
    `setblock 122 ${y} 0 minecraft:oak_stairs[facing=east,half=bottom,shape=straight,waterlogged=false]`,
  );
  await ok(`setblock 121 ${y} 0 minecraft:water[level=0]`);
  await ok(`setblock 123 ${y} 0 minecraft:water[level=0]`);
  await sleep(6000);
  out.two_sources_across_a_waterloggable = {
    stair_waterlogged: flag(
      await raw(`execute if block 122 ${y} 0 minecraft:oak_stairs[waterlogged=true] ${PROBE}`),
    ),
  };

  // 6. A source directly ABOVE a dry waterloggable block, solid under it.
  //    Falling water is a flow, so the rule predicts the block stays dry.
  filled(await raw(`fill 138 ${y - 2} -2 144 ${y + 2} 2 minecraft:air`));
  await ok(`fill 139 ${y - 2} -1 143 ${y + 2} 1 minecraft:stone`);
  await ok(`fill 141 ${y} 0 141 ${y + 1} 0 minecraft:air`);
  await ok(
    `setblock 141 ${y} 0 minecraft:oak_stairs[facing=east,half=bottom,shape=straight,waterlogged=false]`,
  );
  await ok(`setblock 141 ${y + 1} 0 minecraft:water[level=0]`);
  await sleep(6000);
  out.source_above_a_waterloggable = {
    stair_waterlogged: flag(
      await raw(`execute if block 141 ${y} 0 minecraft:oak_stairs[waterlogged=true] ${PROBE}`),
    ),
  };

  // 7. One source beside a dry waterloggable block turned so that its OPEN
  //    face is the one the water meets. Rig 3 leaves two readings alive — the
  //    block refused the flow, or the block's own face occluded it — and this
  //    is the one that separates them.
  await box(160, y, 0);
  await ok(
    `setblock 161 ${y} 0 minecraft:oak_stairs[facing=west,half=bottom,shape=straight,waterlogged=false]`,
  );
  filled(await raw(`fill 162 ${y - 1} -1 164 ${y + 1} 1 minecraft:air`));
  await ok(`setblock 160 ${y} 0 minecraft:water[level=0]`);
  await sleep(6000);
  out.open_face_waterloggable = {
    stair_waterlogged: flag(
      await raw(`execute if block 161 ${y} 0 minecraft:oak_stairs[waterlogged=true] ${PROBE}`),
    ),
    water_beyond: (await flowingAround(163, y, 0, 2)) + (await sourcesAround(163, y, 0, 2)),
  };

  // 8. A block written `waterlogged=true` with an OPEN cell beside it, and no
  //    other water anywhere. The gate treats such a block as part of a body
  //    rather than as a wall, and this is the rig that decides whether that is
  //    true: if the water in it spreads, it is a body.
  await box(180, y, 0);
  await ok(`setblock 181 ${y} 0 minecraft:air`);
  await ok(
    `setblock 180 ${y} 0 minecraft:oak_stairs[facing=east,half=bottom,shape=straight,waterlogged=true]`,
  );
  await sleep(6000);
  out.waterlogged_block_beside_air = { flowing: await flowingAround(180, y, 0, 4) };

  // 8b. The same, after ANY block update near it. A waterloggable block's own
  //     `updateShape` schedules a fluid tick, so "it has not moved yet" and
  //     "it will not move" are different claims, and only one of them is what
  //     a gate may rest on. This rig is the difference between them.
  await box(220, y, 0);
  await ok(`setblock 221 ${y} 0 minecraft:air`);
  await ok(
    `setblock 220 ${y} 0 minecraft:oak_stairs[facing=east,half=bottom,shape=straight,waterlogged=true]`,
  );
  await sleep(2000);
  const beforePoke = await flowingAround(220, y, 0, 4);
  await ok(`setblock 222 ${y} 0 minecraft:stone`);
  await ok(`setblock 222 ${y} 0 minecraft:air`);
  await sleep(6000);
  out.waterlogged_block_after_a_block_update = {
    flowing_before_poke: beforePoke,
    flowing_after_poke: await flowingAround(220, y, 0, 4),
    still_waterlogged: flag(
      await raw(`execute if block 220 ${y} 0 minecraft:oak_stairs[waterlogged=true] ${PROBE}`),
    ),
  };

  // 9. The same block, sealed. A grate written wet inside a wall is not a leak.
  await box(200, y, 0);
  await ok(
    `setblock 200 ${y} 0 minecraft:oak_stairs[facing=east,half=bottom,shape=straight,waterlogged=true]`,
  );
  await sleep(6000);
  out.waterlogged_block_sealed = {
    flowing: await flowingAround(200, y, 0, 4),
    still_waterlogged: flag(
      await raw(`execute if block 200 ${y} 0 minecraft:oak_stairs[waterlogged=true] ${PROBE}`),
    ),
  };

  return out;
}

// ---------------------------------------------------------------------- main
(async () => {
  await connectChannel();
  await ok("gamerule random_tick_speed 0");
  const stair_field = await measureStairField();
  const water = await measureWater();

  const shapes = {};
  for (const c of stair_field.cells) {
    if (c.observed_shape) shapes[c.observed_shape] = (shapes[c.observed_shape] ?? 0) + 1;
  }
  const observations = {
    minecraft_version: "1.21.11",
    measured_by: "tools/spike-block-settling/measure.mjs",
    stair_field,
    stair_shape_histogram: shapes,
    water,
  };
  writeFileSync(OUT, JSON.stringify(observations, null, 2) + "\n");
  console.log(`[measure] ${stair_field.cells.length} field cells, shapes ${JSON.stringify(shapes)}`);
  console.log(`[measure] water ${JSON.stringify(water)}`);
  proc.stdin.end();
})().catch((e) => {
  console.error(`[measure] FAILED: ${e.stack ?? e}`);
  process.exit(1);
});
