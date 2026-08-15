#!/usr/bin/env node
// SPIKE TOOLING (jump kinematics, phase 1) — NOT part of the shipped pipeline.
//
// Measures 1.21.11 jump kinematics with a mineflayer bot against the throwaway
// vanilla server started by run.sh. The bot IS the measurement instrument on
// purpose: it is the same client stack (mineflayer + prismarine-physics vanilla
// movement) the validation harness drives, so every distance recorded here is a
// distance the actual critical-path bot can reproduce on the pinned server — the
// set the compiler's jump model must never exceed (never over-prove). Vanilla
// wiki kinematics (jump apex ~1.25 blocks, sprint ~5.6 m/s) serve only as a
// sanity cross-check in docs/notes/jump-arc-model.md.
//
// Rig (rebuilt per trial config, all via RCON — the bot never edits the world):
//
//        approach (stone)      gap g        landing (stone)
//   x:  -10 ............ 0 | 1 .. g | g+1 ............ g+12      z = -1..1
//   y:  platform tops at Y_SURF-1, standing surface Y_SURF; catch floor below
//
// The bot sprints/walks east (+x), jumps on the last supported tick at the
// launch edge (plane x = 1), and the outcome is read from its own physics:
// landing onGround at the landing surface height past the gap = success;
// dropping to the catch floor = failure. Trials are tick-quantised and repeated
// (ATTEMPTS×) — a config counts as achievable if ANY attempt lands it, matching
// "can the bot make this jump on cue at all".
//
// Reuses the harness's pinned mineflayer via createRequire — no new deps.

import { createRequire } from "node:module";

import { rconChannel } from "../lib/rcon.mjs";

const require = createRequire(new URL("../../harness/package.json", import.meta.url));
const mineflayer = require("mineflayer");

const CONTAINER = process.env.SPIKE_CONTAINER ?? "dw-spike-jump-arc";
const PORT = Number(process.env.SPIKE_PORT ?? 25599);
const BOT = "dw_spike";
const Y_SURF = 120; // standing surface; platform blocks at Y_SURF-1
const LAUNCH_PLANE = 1; // start platform is x<=0, so its east face is x=1
const ATTEMPTS = 3;

// Every rig command is CHECKED (tools/lib/rcon.mjs): a `fill` into an unloaded
// chunk or a rejected gamerule used to answer, be discarded, and leave the rig
// measuring a bot falling through a floor that was never placed.
const { run: rcon } = rconChannel(CONTAINER);

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** Clear the rig volume, then build platforms + catch floor for one config. */
async function buildRig(gap, rise, ceil) {
  // Park the bot on the permanent pad OUTSIDE the wipe volume first — a bot left
  // standing on the catch floor from a failed attempt would fall when the wipe
  // removes it mid-rebuild (the first clean-run crash).
  await rcon(`tp ${BOT} -20.5 ${Y_SURF} 0.5 -90 0`);
  await sleep(300);
  // Wipe a generous volume around the rig (also removes previous ceilings).
  await rcon(`fill -14 ${Y_SURF - 9} -4 ${gap + 16} ${Y_SURF + 8} 4 minecraft:air`);
  // Catch floor (failure detector) well below the surface.
  await rcon(`fill -14 ${Y_SURF - 9} -3 ${gap + 16} ${Y_SURF - 9} 3 minecraft:stone`);
  // Approach platform: x -10..0.
  await rcon(`fill -10 ${Y_SURF - 1} -1 0 ${Y_SURF - 1} 1 minecraft:stone`);
  // Landing platform: x gap+1 .. gap+12, `rise` blocks higher.
  await rcon(
    `fill ${gap + 1} ${Y_SURF - 1 + rise} -1 ${gap + 12} ${Y_SURF - 1 + rise} 1 minecraft:stone`,
  );
  // Optional ceiling: a stone lid leaving `headroom` air cells above the surface
  // of the phase it covers (launch = the last approach cells, gap = the air gap
  // columns, landing = the first landing cells, full = the whole rig).
  if (ceil) {
    const y = Y_SURF + ceil.headroom; // air cells Y_SURF .. y-1 => `headroom` of air
    const spans = {
      launch: [-3, 0],
      gap: [1, Math.max(1, gap)],
      landing: [gap + 1, gap + 4],
      full: [-10, gap + 12],
    };
    const [x0, x1] = spans[ceil.phase];
    await rcon(`fill ${x0} ${y} -1 ${x1} ${y} 1 minecraft:stone`);
  }
  await rcon(`effect give ${BOT} minecraft:saturation infinite 10 true`);
}

/** Teleport the bot to the approach start and wait until it stands still. */
async function resetBot(bot) {
  bot.clearControlStates();
  await rcon(`tp ${BOT} -8.5 ${Y_SURF} 0.5 -90 0`);
  await sleep(400);
  for (let i = 0; i < 100; i++) {
    const p = bot.entity.position;
    if (bot.entity.onGround && Math.abs(p.x - -8.5) < 1 && Math.abs(p.y - Y_SURF) < 0.01) return;
    await sleep(50);
  }
  throw new Error("bot did not settle at the start pad");
}

/**
 * One jump attempt. Returns {landed, launchX, apexY, landX} — `landed` true when
 * the bot ends onGround on the landing surface past the gap.
 */
function attempt(bot, { gap, rise, sprint }) {
  return new Promise((resolve) => {
    let jumped = false;
    let launchX = null;
    let apexY = -Infinity;
    const landSurface = Y_SURF + rise;
    // gap=0 (a pure step-up jump): the landing face is AT the launch plane, so the
    // bot's AABB (half-width 0.3) collides at x≈0.7 — jump earlier or it never can.
    const jumpAt = gap === 0 ? LAUNCH_PLANE - 0.5 : LAUNCH_PLANE - 0.05;
    const done = (landed) => {
      bot.removeListener("physicsTick", onTick);
      bot.clearControlStates();
      resolve({ landed, launchX, apexY, landX: bot.entity.position.x });
    };
    const onTick = () => {
      const p = bot.entity.position;
      apexY = Math.max(apexY, p.y);
      if (!jumped) {
        // Sprint-jump straight east; jump on the last supported tick at the edge.
        bot.setControlState("forward", true);
        bot.setControlState("sprint", sprint);
        if (bot.entity.onGround && p.x >= jumpAt) {
          bot.setControlState("jump", true);
          jumped = true;
          launchX = p.x;
        }
        // Walked off the edge before the jump fired (tick quantisation): failure.
        if (!bot.entity.onGround && p.x >= LAUNCH_PLANE + 0.1) jumped = true;
      } else {
        bot.setControlState("jump", false);
        if (p.y < Y_SURF - 3) return done(false); // fell to the catch floor
        if (bot.entity.onGround && Math.abs(p.y - landSurface) < 0.01 && p.x >= LAUNCH_PLANE) {
          return done(p.x >= gap + 1 - 0.299); // supported by the landing platform?
        }
        if (bot.entity.onGround && Math.abs(p.y - Y_SURF) < 0.01 && p.x < LAUNCH_PLANE) {
          return done(false); // ended back on the approach platform
        }
      }
    };
    bot.on("physicsTick", onTick);
    setTimeout(() => done(false), 6000);
  });
}

async function trial(bot, cfg) {
  await buildRig(cfg.gap, cfg.rise, cfg.ceil ?? null);
  let best = null;
  let successes = 0;
  for (let i = 0; i < ATTEMPTS; i++) {
    await resetBot(bot);
    const r = await attempt(bot, cfg);
    if (r.landed) {
      successes++;
      if (!best || r.launchX > best.launchX) best = r;
    }
  }
  const tag = `${cfg.sprint ? "sprint" : "walk"} gap=${cfg.gap} rise=${cfg.rise}` +
    (cfg.ceil ? ` ceil=${cfg.ceil.phase}/${cfg.ceil.headroom}` : "");
  const apex = best ? (best.apexY - Y_SURF).toFixed(3) : "-";
  console.log(
    `[trial] ${tag}: ${successes}/${ATTEMPTS} landed` +
      (best ? ` (launchX=${best.launchX.toFixed(3)}, apex=+${apex}, landX=${best.landX.toFixed(3)})` : ""),
  );
  return { ...cfg, successes, attempts: ATTEMPTS, apex: best ? best.apexY - Y_SURF : null };
}

async function main() {
  const bot = mineflayer.createBot({
    host: "127.0.0.1",
    port: PORT,
    username: BOT,
    auth: "offline",
  });
  await new Promise((resolve, reject) => {
    bot.once("spawn", resolve);
    bot.once("kicked", (r) => reject(new Error(`kicked: ${JSON.stringify(r)}`)));
    bot.once("error", reject);
  });
  console.log(`[spike] bot spawned (server version ${bot.version})`);
  bot.on("death", () => bot.respawn?.()); // sturdiness only; should not trigger
  await sleep(1000);
  // Permanent parking pad outside the rig-wipe volume, and no fall damage — the
  // catch-floor drop after a FAILED attempt must not grind the bot down (fall
  // damage alters no jump kinematics, only the instrument's durability).
  //
  // 1.21.11 spells this rule `fall_damage`; the legacy `fallDamage` this line
  // carried is rejected outright ("Incorrect argument for command"), so the
  // comment above was simply untrue for the whole spike and no reply was read to
  // say so. Re-measured on the pinned server 2026-08-11.
  await rcon(`fill -22 ${Y_SURF - 1} -1 -19 ${Y_SURF - 1} 1 minecraft:stone`);
  await rcon("gamerule fall_damage false");

  const results = [];
  // (1) flat gaps, walk then sprint.
  for (const sprint of [false, true]) {
    for (let gap = 1; gap <= (sprint ? 6 : 4); gap++) {
      results.push(await trial(bot, { gap, rise: 0, sprint }));
    }
  }
  // (2) +1 rise, walk then sprint (gap 0 = jump up the step across no air gap).
  for (const sprint of [false, true]) {
    for (let gap = 0; gap <= (sprint ? 4 : 2); gap++) {
      results.push(await trial(bot, { gap, rise: 1, sprint }));
    }
  }
  // (3) ceiling clearance: at the max flat sprint gap that landed, lid one phase
  // at a time with 2..4 air cells of headroom, plus a full lid.
  const maxSprintFlat = Math.max(
    ...results.filter((r) => r.sprint && r.rise === 0 && r.successes > 0).map((r) => r.gap),
  );
  console.log(`[spike] ceiling matrix at sprint flat gap=${maxSprintFlat}`);
  for (const phase of ["launch", "gap", "landing", "full"]) {
    for (const headroom of [2, 3, 4]) {
      results.push(
        await trial(bot, { gap: maxSprintFlat, rise: 0, sprint: true, ceil: { phase, headroom } }),
      );
    }
  }
  // (4) ceiling over a +1 rise jump (the tighter case: apex is higher).
  const maxSprintRise = Math.max(
    ...results.filter((r) => r.sprint && r.rise === 1 && !r.ceil && r.successes > 0).map((r) => r.gap),
  );
  for (const headroom of [2, 3, 4]) {
    results.push(
      await trial(bot, { gap: maxSprintRise, rise: 1, sprint: true, ceil: { phase: "full", headroom } }),
    );
  }

  console.log("\n[spike] raw results JSON:");
  console.log(JSON.stringify(results, null, 1));
  bot.end();
  process.exit(0);
}

main().catch((e) => {
  console.error("[spike] FAILED:", e);
  process.exit(1);
});
