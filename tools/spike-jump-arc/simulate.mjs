#!/usr/bin/env node
// SPIKE TOOLING (jump kinematics, phase 2) — NOT part of the shipped pipeline.
//
// The offline twin of measure.mjs: the same rig (approach platform, an air gap
// of `gap` whole columns, a landing platform `rise` blocks up or down), driven
// through prismarine-physics — the movement code the harness bot and the live
// spike both run — against an in-memory world, with no server.
//
// What it adds over the live rig is the one variable the live rig held fixed:
// the RUNWAY, the number of standable cells the body has behind and including
// the launch cell. The live spike always started 9.5 blocks back; a body on a
// garden boulder or a plinth top starts on one to three. And, because it is
// offline and deterministic, it searches the whole policy space the body has —
// sprint or not, and the tick the jump is pressed on — and reports a config as
// reachable when ANY policy lands it. That is the question a trap proof asks
// ("can a body get there"), which is not the question a route proof asks ("can
// the bot do it on cue"); the live spike's 3-of-3 rule answers the second.
//
// Cross-check: `--check` re-runs the live spike's own configurations (runway
// 10, the spike's jump-at-the-edge policy) and prints landX and apex beside the
// live figures recorded in docs/notes/jump-arc-model.md §2.
//
//   node tools/spike-jump-arc/simulate.mjs           # the envelope table
//   node tools/spike-jump-arc/simulate.mjs --check   # the cross-check
//
// Resolves its dependencies from harness/node_modules (run `npm ci` there).

import { createRequire } from "node:module";

const require = createRequire(new URL("../../harness/package.json", import.meta.url));
const VERSION = "1.21.11";
const mcData = require("minecraft-data")(VERSION);
const Block = require("prismarine-block")(VERSION);
const { Physics, PlayerState } = require("prismarine-physics");
const { Vec3 } = require("vec3");

const Y = 100; // standing surface of the approach platform
const STONE = mcData.blocksByName.stone.id;
const AIR = mcData.blocksByName.air.id;

function rig({ runway, gap, rise }) {
  const solid = (x, y, z) => {
    if (z < -1 || z > 1) return false;
    if (y === Y - 1 && x <= 0 && x >= -(runway - 1)) return true;
    if (y === Y - 1 + rise && x >= gap + 1 && x <= gap + 12) return true;
    return false;
  };
  return {
    getBlock(pos) {
      const b = Block.fromStateId(
        solid(Math.floor(pos.x), Math.floor(pos.y), Math.floor(pos.z))
          ? mcData.blocks[STONE].defaultState
          : mcData.blocks[AIR].defaultState,
        0,
      );
      b.position = pos.floored();
      return b;
    },
  };
}

function body(x) {
  return {
    version: VERSION,
    entity: {
      position: new Vec3(x, Y, 0.5),
      velocity: new Vec3(0, 0, 0),
      onGround: true,
      isInWater: false,
      isInLava: false,
      isInWeb: false,
      isCollidedHorizontally: false,
      isCollidedVertically: true,
      elytraFlying: false,
      // mineflayer yaw: -PI/2 faces +x (east), the rig's jump axis.
      yaw: -Math.PI / 2,
      pitch: 0,
      effects: {},
      attributes: {},
    },
    jumpTicks: 0,
    jumpQueued: false,
    fireworkRocketDuration: 0,
    inventory: { slots: [] },
  };
}

/**
 * One run. `jumpTick` = the tick the jump is pressed on (null = the spike's
 * edge policy: press on the first supported tick past `edgeAt`). Returns
 * {landed, launchX, apex, landX}.
 */
function run(cfg, { sprint, jumpTick = null, edgeAt = null, startX = null }) {
  const world = rig(cfg);
  const physics = Physics(mcData, world);
  const bot = body(startX ?? -(cfg.runway - 1) + 0.5);
  const controls = { forward: true, back: false, left: false, right: false, jump: false, sprint, sneak: false };
  let jumped = false;
  let launchX = null;
  let apex = -Infinity;
  const land = Y + cfg.rise;
  for (let t = 0; t < 200; t++) {
    const p = bot.entity.position;
    apex = Math.max(apex, p.y);
    if (!jumped) {
      const now = jumpTick === null ? bot.entity.onGround && p.x >= edgeAt : t === jumpTick;
      if (now && bot.entity.onGround) {
        controls.jump = true;
        jumped = true;
        launchX = p.x;
      }
    } else {
      controls.jump = false;
    }
    const st = new PlayerState(bot, controls);
    physics.simulatePlayer(st, world).apply(bot);
    const q = bot.entity.position;
    if (q.y < Math.min(Y, land) - 4) return { landed: false, launchX, apex: apex - Y, landX: q.x };
    if (jumped && bot.entity.onGround && t > 0) {
      if (Math.abs(q.y - land) < 0.01 && q.x >= cfg.gap + 1 - 0.299) {
        return { landed: true, launchX, apex: apex - Y, landX: q.x };
      }
      if (Math.abs(q.y - Y) < 0.01 && q.x < 1 && bot.entity.velocity.y === 0 && t > 3) {
        // back on the approach platform after a jump: this policy failed
        return { landed: false, launchX, apex: apex - Y, landX: q.x };
      }
    }
  }
  return { landed: false, launchX, apex: apex - Y, landX: bot.entity.position.x };
}

/** Whether ANY policy (walk/sprint x every jump tick) lands the config. */
function reachable(cfg) {
  for (const sprint of [false, true]) {
    for (let jt = 0; jt < 40; jt++) {
      const r = run(cfg, { sprint, jumpTick: jt });
      if (r.landed) return { sprint, jumpTick: jt, ...r };
    }
  }
  return null;
}

const why = process.argv.indexOf("--why");
if (why > 0) {
  // `--why runway,gap,rise`: the first policy that lands the config, tick by tick.
  const [runway, gap, rise] = process.argv[why + 1].split(",").map(Number);
  const r = reachable({ runway, gap, rise });
  console.log(JSON.stringify({ runway, gap, rise, policy: r }));
  process.exit(0);
}

if (process.argv.includes("--check")) {
  // The live spike's configurations, its runway (start 9.5 back of the launch
  // plane at x=1, i.e. x=-8.5) and its policy (press at x >= 0.95; 0.5 for gap 0).
  const live = [
    ["walk", 1, 0], ["walk", 2, 0], ["walk", 3, 0], ["sprint", 1, 0], ["sprint", 2, 0],
    ["sprint", 3, 0], ["sprint", 4, 0], ["walk", 0, 1], ["walk", 1, 1], ["walk", 2, 1],
    ["sprint", 0, 1], ["sprint", 1, 1], ["sprint", 2, 1], ["sprint", 3, 1],
  ];
  for (const [mode, gap, rise] of live) {
    const r = run({ runway: 10, gap, rise }, {
      sprint: mode === "sprint",
      edgeAt: gap === 0 ? 0.5 : 0.95,
      startX: -8.5,
    });
    console.log(
      `${mode.padEnd(6)} gap=${gap} rise=${rise} : ${r.landed ? "landed" : "missed"}` +
        `  launchX=${r.launchX?.toFixed(3)} apex=+${r.apex.toFixed(3)} landX=${r.landX.toFixed(3)}`,
    );
  }
  process.exit(0);
}

// The envelope: for each rise (landing surface minus launch surface, whole
// blocks), the largest gap of air columns ANY policy lands from EVERY runway in
// RUNWAYS — the minimum over runways, so the figure holds for a body that has
// no more than the launch cell to stand on. A longer runway never lands less in
// this table's range, and where the tick grid makes one runway land a gap
// further than its neighbour the smaller figure is the one kept.
//
// Rises run from +1 (the highest a jump climbs: vanilla's 1.2522 apex) down to
// the deepest fall an unarmoured body survives (22 blocks, dsl::metrics).
const RUNWAYS = [1, 2, 3, 4, 6, 10];
const table = [];
for (let rise = 1; rise >= -22; rise--) {
  let floor = Infinity;
  const per = [];
  for (const runway of RUNWAYS) {
    let best = rise <= 0 ? 0 : -1; // a plain walk-off reaches gap 0 at any drop
    for (let gap = 1; gap <= 16; gap++) {
      if (reachable({ runway, gap, rise })) best = gap;
      else break;
    }
    per.push(best);
    floor = Math.min(floor, best);
  }
  table.push({ rise, max_gap: floor, per_runway: per });
  console.log(`rise ${String(rise).padStart(3)} : max gap ${floor}   (runways ${RUNWAYS.join("/")}: ${per.join("/")})`);
}
if (process.argv.includes("--json")) console.log(JSON.stringify(table.map((r) => [r.rise, r.max_gap])));
