// The bot's client physics, written down tick by tick (spec-0113 §8.1).
//
// prismarine-physics 1.11.1 — the physics a mineflayer bot runs, at the harness
// lockfile's pin — is driven over a fixed set of rigs: the jump rigs of
// `docs/reference/body-movement.md` §9 and one rig per movement class of
// spec-0113 §4.3. Every tick's position, velocity and ground flag is written to
// `crates/dsl/tests/fixtures/movement/cases-1.21.11.json`, and the engine's own
// simulator (`delvewright_dsl::metrics::body`, `Rules::Bot`) replays each case
// and is held to it within 1e-9 a tick (`the_simulator_matches_the_bot_physics`).
// The two share nothing but that file: each block is written with the
// collision boxes the library reads for it, the controls held each tick, and
// the state the library left.
//
// This test refuses when the committed file is not what it writes, so the
// replay can never be held to a stale or hand-edited record. Rewrite it with
// `DELVEWRIGHT_WRITE_MOVEMENT_CASES=1 npm test -- test/physics-cases.test.ts`.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { cellKey, SimBot, SIM_VERSION, type Cells } from "./sim-bot.ts";

const CASE_FILE = fileURLToPath(
  new URL("../../crates/dsl/tests/fixtures/movement/cases-1.21.11.json", import.meta.url),
);

/** The keys held for a run of ticks; `yaw` is the game's degrees (0 south, −90 east). */
interface Hold {
  readonly ticks: number;
  readonly yaw: number;
  readonly forward?: number;
  readonly strafe?: number;
  readonly jump?: boolean;
  readonly sprint?: boolean;
  readonly sneak?: boolean;
}

interface Rig {
  readonly name: string;
  readonly cells: Cells;
  readonly start: readonly [number, number, number];
  readonly vel?: readonly [number, number, number];
  readonly onGround?: boolean;
  readonly holds: readonly Hold[];
}

const STONE = "minecraft:stone";

function slab(cells: Cells, x0: number, x1: number, z0: number, z1: number, y: number, state = STONE): void {
  for (let x = x0; x <= x1; x++) for (let z = z0; z <= z1; z++) cells.set(cellKey(x, y, z), state);
}

/** The §9 jump rig: an approach platform ending at x = 1 (its top at y = 1), `gap` air columns, a landing block at y = `rise`. */
function jumpRig(gap: number, rise: number, lid?: number): Cells {
  const c: Cells = new Map();
  slab(c, -12, 0, -2, 2, 0);
  slab(c, 1 + gap, 6 + gap, -2, 2, rise);
  slab(c, -12, 12, -2, 2, -9);
  if (lid !== undefined) slab(c, -12, 6 + gap, -2, 2, lid);
  return c;
}

const EAST = -90;

/**
 * A run-up east from x = −8.5 holding forward (and sprint), with the jump
 * pressed on the tick that starts from the last position whose box still
 * overlaps the approach platform — found by running the same physics without
 * the jump.
 */
function edgeJump(name: string, cells: Cells, sprint: boolean, after = 24): Rig {
  const probe = new SimBot(cells, [-8.5, 1, 0.5]);
  probe.entity.yaw = Math.PI - (EAST * Math.PI) / 180;
  probe.setControlState("forward", true);
  probe.setControlState("sprint", sprint);
  let t = 0;
  for (; t < 200; t++) {
    probe.tick();
    // The first tick that carries the box clear of the platform's edge: the
    // tick before it starts from the last supported position.
    if (probe.entity.position.x - 0.3 >= 1) break;
  }
  return {
    name,
    cells,
    start: [-8.5, 1, 0.5],
    holds: [
      { ticks: t, yaw: EAST, forward: 1, sprint },
      { ticks: after, yaw: EAST, forward: 1, sprint, jump: true },
    ],
  };
}

function rigs(): Rig[] {
  const out: Rig[] = [];
  const flat = (): Cells => {
    const c: Cells = new Map();
    slab(c, -12, 12, -4, 4, 0);
    return c;
  };
  out.push({ name: "flat walk", cells: flat(), start: [0.5, 1, 0.5], holds: [{ ticks: 40, yaw: EAST, forward: 1 }] });
  out.push({
    name: "flat sprint, then let go",
    cells: flat(),
    start: [-8.5, 1, 0.5],
    holds: [
      { ticks: 30, yaw: EAST, forward: 1, sprint: true },
      { ticks: 20, yaw: EAST },
    ],
  });
  out.push({
    name: "flat sprint, diagonal",
    cells: flat(),
    start: [-8.5, 1, -2.5],
    holds: [{ ticks: 30, yaw: -60, forward: 1, strafe: 1, sprint: true }],
  });
  out.push({
    name: "standing jump in place",
    cells: flat(),
    start: [0.5, 1, 0.5],
    holds: [{ ticks: 16, yaw: EAST, jump: true }],
  });
  out.push(edgeJump("sprint jump, three columns flat", jumpRig(3, 0), true));
  out.push(edgeJump("sprint jump, four columns flat", jumpRig(4, 0), true));
  out.push(edgeJump("walk jump, one column flat", jumpRig(1, 0), false));
  out.push(edgeJump("sprint jump, two columns up one", jumpRig(2, 1), true));
  out.push(edgeJump("sprint jump, four columns down one", jumpRig(4, -1), true));
  out.push(edgeJump("head-hitter, a lid two cells over the launch floor", jumpRig(3, 0, 3), true));
  out.push(edgeJump("head-hitter, a lid three cells over the launch floor", jumpRig(3, 0, 4), true));
  out.push({
    name: "sprint off a ledge, no jump",
    cells: jumpRig(8, 0),
    start: [-8.5, 1, 0.5],
    holds: [{ ticks: 50, yaw: EAST, forward: 1, sprint: true }],
  });
  out.push({
    name: "sprint across one column without a jump",
    cells: jumpRig(1, 0),
    start: [-8.5, 1, 0.5],
    holds: [{ ticks: 40, yaw: EAST, forward: 1, sprint: true }],
  });
  out.push({
    name: "walk into a one-column gap",
    cells: jumpRig(1, 0),
    start: [-8.5, 1, 0.5],
    holds: [{ ticks: 60, yaw: EAST, forward: 1 }],
  });
  {
    const c: Cells = new Map();
    slab(c, -12, 0, -2, 2, 0, "minecraft:smooth_stone_slab[type=bottom,waterlogged=false]");
    slab(c, 3, 8, -2, 2, 0);
    slab(c, -12, 12, -2, 2, -9);
    out.push(edgeJump("slab launch, two columns onto a full block", c, true));
  }
  {
    const c = flat();
    slab(c, 2, 4, -1, 1, 1, "minecraft:smooth_stone_slab[type=bottom,waterlogged=false]");
    out.push({ name: "walk up onto a slab", cells: c, start: [-2.5, 1, 0.5], holds: [{ ticks: 30, yaw: EAST, forward: 1 }] });
  }
  {
    const c = flat();
    c.set(cellKey(1, 1, 0), STONE);
    c.set(cellKey(2, 1, 0), "minecraft:cobblestone_wall[east=none,north=none,south=none,up=true,waterlogged=false,west=none]");
    out.push({
      name: "a block, a wall top, and down",
      cells: c,
      start: [1.5, 2, 0.5],
      holds: [{ ticks: 40, yaw: EAST, forward: 1 }],
    });
  }
  {
    const c = flat();
    c.set(cellKey(1, 1, 0), STONE);
    c.set(cellKey(3, 1, 0), "minecraft:oak_fence[east=false,north=false,south=false,waterlogged=false,west=false]");
    out.push({
      name: "a sprint jump onto a fence top",
      cells: c,
      start: [1.5, 2, 0.5],
      holds: [
        { ticks: 2, yaw: EAST, forward: 1, sprint: true },
        { ticks: 30, yaw: EAST, forward: 1, sprint: true, jump: true },
      ],
    });
  }
  {
    const c = flat();
    slab(c, -12, 12, -1, 1, 0, "minecraft:honey_block");
    out.push({
      name: "honey: sprint and jump",
      cells: c,
      start: [-6.5, 0.9375, 0.5],
      holds: [
        { ticks: 20, yaw: EAST, forward: 1, sprint: true },
        { ticks: 20, yaw: EAST, forward: 1, sprint: true, jump: true },
      ],
    });
  }
  {
    const c = flat();
    slab(c, -12, 12, -1, 1, 0, "minecraft:soul_sand");
    out.push({
      name: "soul sand: sprint and jump",
      cells: c,
      start: [-6.5, 0.875, 0.5],
      holds: [
        { ticks: 20, yaw: EAST, forward: 1, sprint: true },
        { ticks: 20, yaw: EAST, forward: 1, sprint: true, jump: true },
      ],
    });
  }
  {
    const c = flat();
    slab(c, -12, 12, -1, 1, 0, "minecraft:ice");
    out.push({
      name: "ice: sprint, then let go",
      cells: c,
      start: [-10.5, 1, 0.5],
      holds: [
        { ticks: 20, yaw: EAST, forward: 1, sprint: true },
        { ticks: 30, yaw: EAST },
      ],
    });
  }
  {
    const c = flat();
    slab(c, -2, 2, -2, 2, 0, "minecraft:slime_block");
    out.push({ name: "slime: a fall of six and the bounce", cells: c, start: [0.5, 7, 0.5], onGround: false, holds: [{ ticks: 60, yaw: EAST }] });
  }
  {
    const c: Cells = new Map();
    slab(c, -4, 4, -4, 4, 0);
    for (let y = 1; y <= 5; y++) {
      slab(c, -4, 4, -4, 4, y, "minecraft:water[level=0]");
      for (let i = -4; i <= 4; i++) {
        c.set(cellKey(-4, y, i), STONE);
        c.set(cellKey(4, y, i), STONE);
        c.set(cellKey(i, y, -4), STONE);
        c.set(cellKey(i, y, 4), STONE);
      }
    }
    out.push({ name: "water: sink", cells: c, start: [0.5, 5.2, 0.5], onGround: false, holds: [{ ticks: 60, yaw: EAST }] });
    out.push({
      name: "water: rise, jump held",
      cells: new Map(c),
      start: [0.5, 1, 0.5],
      onGround: true,
      holds: [{ ticks: 60, yaw: EAST, jump: true }],
    });
    out.push({
      name: "water: swim across, forward",
      cells: new Map(c),
      start: [-2.5, 4.6, 0.5],
      onGround: false,
      holds: [{ ticks: 40, yaw: EAST, forward: 1 }],
    });
  }
  {
    const c: Cells = new Map();
    slab(c, -4, 0, -2, 2, 0);
    for (let y = 1; y <= 3; y++) slab(c, -4, 0, -2, 2, y, "minecraft:water[level=0]");
    slab(c, 1, 4, -2, 2, 1);
    slab(c, 1, 4, -2, 2, 2);
    slab(c, 1, 4, -2, 2, 3);
    out.push({
      name: "water: climb out onto a ledge flush with the water's top",
      cells: c,
      start: [-1.5, 3.4, 0.5],
      onGround: false,
      holds: [{ ticks: 40, yaw: EAST, forward: 1, jump: true }],
    });
  }
  {
    const c: Cells = new Map();
    slab(c, -6, 6, -2, 2, 0);
    for (let x = -5; x <= 5; x++) {
      const level = Math.min(7, Math.abs(x));
      c.set(cellKey(x, 1, 0), `minecraft:water[level=${level}]`);
    }
    for (let x = -6; x <= 6; x++) {
      c.set(cellKey(x, 1, -1), STONE);
      c.set(cellKey(x, 1, 1), STONE);
    }
    out.push({ name: "water: the current of a flowing stream", cells: c, start: [2.5, 1, 0.5], holds: [{ ticks: 40, yaw: EAST }] });
  }
  {
    const c = flat();
    for (let y = 1; y <= 8; y++) {
      c.set(cellKey(2, y, 0), "minecraft:oak_planks");
      c.set(cellKey(1, y, 0), "minecraft:ladder[facing=west,waterlogged=false]");
    }
    out.push({
      name: "ladder: forward into it, jump held",
      cells: c,
      start: [0.5, 1, 0.5],
      holds: [{ ticks: 60, yaw: EAST, forward: 1, jump: true }],
    });
    out.push({
      name: "ladder: let go at the top of a fall",
      cells: new Map(c),
      start: [1.5, 9, 0.5],
      onGround: false,
      holds: [{ ticks: 40, yaw: EAST }],
    });
  }
  {
    const c = flat();
    c.set(cellKey(2, 1, 0), "minecraft:cobweb");
    out.push({ name: "cobweb: walk through", cells: c, start: [-0.5, 1, 0.5], holds: [{ ticks: 60, yaw: EAST, forward: 1 }] });
  }
  {
    const c: Cells = new Map();
    slab(c, -8, 0, -2, 2, 4);
    slab(c, -8, 8, -2, 2, 0);
    out.push({
      name: "sneak to the edge",
      cells: c,
      start: [-3.5, 5, 0.5],
      holds: [{ ticks: 40, yaw: EAST, forward: 1, sneak: true }],
    });
  }
  return out;
}

/** The block the bot's world holds for a state, with the boxes and the fields the replay reads. */
function paletteRow(bot: SimBot, key: string, state: string): Record<string, unknown> {
  const [x, y, z] = key.split(",").map(Number) as [number, number, number];
  const b = bot.blockAt({ x, y, z } as never) as { name: string; shapes: number[][]; metadata: number };
  return { metadata: b.metadata, name: b.name, shapes: b.shapes, state };
}

function runRig(r: Rig): Record<string, unknown> {
  const bot = new SimBot(r.cells, r.start);
  if (r.vel) {
    const v = bot.entity["velocity"] as { x: number; y: number; z: number };
    [v.x, v.y, v.z] = r.vel;
  }
  bot.entity["onGround"] = r.onGround ?? true;
  const ticks: Record<string, unknown>[] = [];
  for (const h of r.holds) {
    for (let i = 0; i < h.ticks; i++) {
      bot.clearControlStates();
      bot.setControlState("forward", (h.forward ?? 0) > 0);
      bot.setControlState("back", (h.forward ?? 0) < 0);
      bot.setControlState("right", (h.strafe ?? 0) > 0);
      bot.setControlState("left", (h.strafe ?? 0) < 0);
      bot.setControlState("jump", h.jump ?? false);
      bot.setControlState("sprint", h.sprint ?? false);
      bot.setControlState("sneak", h.sneak ?? false);
      bot.entity.yaw = Math.PI - (h.yaw * Math.PI) / 180;
      bot.tick();
      const p = bot.entity.position;
      const v = bot.entity["velocity"] as { x: number; y: number; z: number };
      ticks.push({
        inWater: bot.entity["isInWater"],
        onGround: bot.entity["onGround"],
        pos: [p.x, p.y, p.z],
        vel: [v.x, v.y, v.z],
      });
    }
  }
  // Every cell as `x,y,z=i`, `i` indexing the case's palette of block states.
  const palette: string[] = [];
  const rows: Record<string, unknown>[] = [];
  const cells = [...r.cells.entries()]
    .sort(([a], [b]) => {
      const pa = a.split(",").map(Number);
      const pb = b.split(",").map(Number);
      return pa[0]! - pb[0]! || pa[1]! - pb[1]! || pa[2]! - pb[2]!;
    })
    .map(([k, s]) => {
      let i = palette.indexOf(s);
      if (i < 0) {
        i = palette.length;
        palette.push(s);
        rows.push(paletteRow(bot, k, s));
      }
      return `${k}=${i}`;
    });
  return {
    cells,
    holds: r.holds.map((h) => ({
      forward: h.forward ?? 0,
      jump: h.jump ?? false,
      sneak: h.sneak ?? false,
      sprint: h.sprint ?? false,
      strafe: h.strafe ?? 0,
      ticks: h.ticks,
      yaw: h.yaw,
    })),
    name: r.name,
    palette: rows,
    start: { onGround: r.onGround ?? true, pos: r.start, vel: r.vel ?? [0, 0, 0] },
    ticks,
  };
}

/** Canonical JSON (`delvec fmt`): keys sorted, two-space indent, one value per line. */
function canonical(v: unknown): unknown {
  if (Array.isArray(v)) return v.map(canonical);
  if (v && typeof v === "object") {
    const o = v as Record<string, unknown>;
    return Object.fromEntries(
      Object.keys(o)
        .sort()
        .map((k) => [k, canonical(o[k])]),
    );
  }
  return v;
}

function document(): string {
  const doc = {
    cases: rigs().map(runRig),
    instrument: "prismarine-physics 1.11.1 (the harness lockfile's pin), driven by harness/test/physics-cases.test.ts",
    version: SIM_VERSION,
  };
  return `${JSON.stringify(canonical(doc), null, 2)}\n`;
}

test("the committed movement cases are what the bot's physics writes", () => {
  const text = document();
  if (process.env["DELVEWRIGHT_WRITE_MOVEMENT_CASES"] === "1") writeFileSync(CASE_FILE, text);
  const committed = readFileSync(CASE_FILE, "utf8");
  const cases = (JSON.parse(text) as { cases: unknown[] }).cases.length;
  assert.ok(cases >= 20, `only ${cases} case(s)`);
  assert.equal(committed, text, "the case file is not what the bot's physics writes; rewrite it (see the header)");
});
