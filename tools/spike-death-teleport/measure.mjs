#!/usr/bin/env node
// SPIKE TOOLING (death edge + teleport fall settlement) — NOT part of the shipped
// pipeline, not wired into CI. Same shape as `tools/spike-jump-arc/`.
//
// Answers two questions about vanilla Minecraft Java 1.21.11 BY MEASUREMENT:
//
//   Q1  For which death causes does a pre-respawn death signal exist, and is the
//       DEATH POSITION readable at that moment? Three candidate mechanisms are
//       probed side by side for each of void / fall / drown / lava / mob:
//         (a) the engine's own edge — the vanilla `deathCount` scoreboard
//             criterion (`dw.deaths` vs `dw.death_ack`, see
//             crates/compiler/src/emit.rs `emit_checkpoint_functions`);
//         (b) an advancement on `minecraft:entity_killed_player`
//             (`dwspike:killed_by_entity`);
//         (c) an advancement on `minecraft:entity_hurt_player`
//             (`dwspike:hurt_any`) — fires on ANY damage, probed to learn
//             whether environmental damage reaches advancement triggers at all;
//       plus the corpse's own `Pos` and the player's `LastDeathLocation`.
//
//   Q2  How does accumulated fall distance settle when a player is teleported
//       mid-fall? (Does an arriving lift car catch a falling player?)
//
// The instrument is a mineflayer bot (the harness's pinned dependency) plus a
// PIPELINED rcon channel. Every value recorded below is read from the SERVER's
// own NBT/scoreboard state, never from the bot's client-side belief — fall
// damage for players is applied server-side from the movement packets, so the
// server's `fall_distance` is the quantity that decides whether the player dies.
//
// Every command's response is checked (`ok()`): the reason this rig exists in
// this shape is that `tools/spike-jump-arc/measure.mjs` line 186 issues
// `gamerule fallDamage false`, which 1.21.11 REJECTS (the rules were renamed to
// snake_case), and nothing noticed because no rcon response was ever read.
//
// Raw observations are written to --out as JSON and committed alongside the
// findings note, so a future session can check the reasoning against the data.

import { createRequire } from "node:module";
import { spawn } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import readline from "node:readline";

import { REJECTION, assertAccepted } from "../lib/rcon.mjs";

const require = createRequire(new URL("../../harness/package.json", import.meta.url));
const mineflayer = require("mineflayer");

const CONTAINER = process.env.SPIKE_CONTAINER ?? "dw-spike-death-tp";
const PORT = Number(process.env.SPIKE_PORT ?? 25599);
const OUT = process.env.SPIKE_OUT ?? new URL("./observations.json", import.meta.url).pathname;
const BOT = "dw_spike";

const Q1_REPS = Number(process.env.SPIKE_Q1_REPS ?? 3);
const Q2_REPS = Number(process.env.SPIKE_Q2_REPS ?? 2);

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// ---------------------------------------------------------------- rcon channel
// One long-lived `rcon-cli` reading stdin. Commands are PIPELINED: a whole
// sample batch is written at once and the responses read back in order, so a
// sample is one round trip and (measured, see `channel` in the output) lands
// inside a single server tick. Alignment is not assumed: every batch is fenced
// by a `#sync` scoreboard write/read whose value must come back, and a mismatch
// aborts the run rather than silently shifting every later reading by one.
const proc = spawn("docker", ["exec", "-i", CONTAINER, "rcon-cli"], {
  stdio: ["pipe", "pipe", "inherit"],
});
const rl = readline.createInterface({ input: proc.stdout });
const inbox = [];
let notify = null;
rl.on("line", (line) => {
  inbox.push(line.replace(/^>\s?/, ""));
  if (notify) {
    const n = notify;
    notify = null;
    n();
  }
});
async function readLines(n) {
  while (inbox.length < n) await new Promise((r) => (notify = r));
  return inbox.splice(0, n);
}
let syncN = 0;
async function batch(cmds) {
  const n = ++syncN;
  const all = [`scoreboard players set #sync dw.sync ${n}`, ...cmds, "scoreboard players get #sync dw.sync"];
  proc.stdin.write(all.join("\n") + "\n");
  const out = await readLines(all.length);
  if (!out[out.length - 1].includes(`#sync has ${n} `)) {
    throw new Error(`rcon desync at ${n}; responses were ${JSON.stringify(out)}`);
  }
  return out.slice(1, -1);
}
const rcon = async (cmd) => (await batch([cmd]))[0];

// The rejection shapes now live in `tools/lib/rcon.mjs`, keyed to the object
// class they are about — "a command issued to a live server" — rather than to
// this one spike (task #70). They were written here first and correctly, and
// that is precisely why the jump-arc rig and the gallery had nothing to reuse:
// a general mechanism re-implemented privately inside one verb leaves the next
// caller writing the unchecked version. `ERR` is kept as a local alias because
// the two probes below are ASKING whether the server rejects something.
const ERR = REJECTION;
/** Run a setup/mutation command and FAIL LOUDLY if the server rejected it. */
async function ok(cmd) {
  return assertAccepted(cmd, await rcon(cmd));
}

// ------------------------------------------------------------------- parsing
const DATA = / has the following entity data: /;
/** `data get` payload, or null when the tag is absent / the read failed. */
function nbt(line) {
  const i = line.search(DATA);
  return i < 0 ? null : line.slice(i).replace(DATA, "");
}
const num = (line) => {
  const v = nbt(line);
  return v === null ? null : Number.parseFloat(v);
};
const vec3 = (line) => {
  const v = nbt(line);
  if (v === null) return null;
  const m = v.match(/-?\d+(?:\.\d+)?(?:[eE]-?\d+)?/g);
  return m ? m.slice(0, 3).map(Number) : null;
};
const score = (line) => {
  const m = line.match(/ has (-?\d+) \[/);
  return m ? Number(m[1]) : null;
};
// A conditional `execute` with no `run` returns an EMPTY rcon response on
// 1.21.11 (no "Test passed" text), which is indistinguishable from a rejected
// command. So every boolean probe below is spelled
// `execute <conditions> run time query gametime`: the condition holding prints
// "The time is <tick>", not holding prints nothing, and anything else is a
// malformed probe and aborts the run rather than reading as `false`.
const PROBE = "run time query gametime";
function flag(line) {
  if (line.startsWith("The time is")) return true;
  if (line === "") return false;
  throw new Error(`boolean probe did not evaluate: ${JSON.stringify(line)}`);
}

// ------------------------------------------------------------------ sampling
const SAMPLE_CMDS = [
  "time query gametime",
  `data get entity ${BOT} Health`,
  `data get entity ${BOT} Pos`,
  `data get entity ${BOT} fall_distance`,
  `data get entity ${BOT} OnGround`,
  `data get entity ${BOT} DeathTime`,
  `data get entity ${BOT} LastDeathLocation`,
  `scoreboard players get ${BOT} dw.deaths`,
  `scoreboard players get ${BOT} dw.death_ack`,
  // The engine's own guard, verbatim from emit_checkpoint_functions().
  `execute as ${BOT} unless data entity @s {Health:0.0f} if score @s dw.deaths > @s dw.death_ack ${PROBE}`,
  // The same edge WITHOUT the aliveness guard: shows whether the unspent edge is
  // armed while the player is still a corpse, i.e. whether the guard is what
  // holds `cp_respawn_check` back rather than the score not having moved yet.
  `execute as ${BOT} if score @s dw.deaths > @s dw.death_ack ${PROBE}`,
  `execute as ${BOT} if entity @s[advancements={dwspike:killed_by_entity=true}] ${PROBE}`,
  `execute as ${BOT} if entity @s[advancements={dwspike:hurt_any=true}] ${PROBE}`,
];
const T0 = Date.now();
async function sample(tag) {
  const t = Date.now();
  const r = await batch(SAMPLE_CMDS);
  return {
    tag,
    ms: t - T0,
    rtt: Date.now() - t,
    tick: Number((r[0].match(/(-?\d+)/) ?? [])[1] ?? NaN),
    hp: num(r[1]),
    pos: vec3(r[2]),
    fall: num(r[3]),
    ground: nbt(r[4]) === "1b",
    deathTime: num(r[5]),
    ldl: nbt(r[6]),
    deaths: score(r[7]),
    ack: score(r[8]),
    guard: flag(r[9]),
    edgeArmed: flag(r[10]),
    advKilled: flag(r[11]),
    advHurt: flag(r[12]),
  };
}
/** Signature used to drop consecutive samples in which nothing changed. */
const sig = (s) =>
  JSON.stringify([
    s.hp,
    s.pos?.map((v) => v.toFixed(2)),
    s.fall?.toFixed(3),
    s.ground,
    s.deathTime,
    s.ldl,
    s.deaths,
    s.ack,
    s.guard,
    s.edgeArmed,
    s.advKilled,
    s.advHurt,
  ]);

/**
 * Poll until `stop(sample, all)` is true or `ms` elapses. Every sample is taken,
 * but only state CHANGES are kept (plus the first and last), so an idle second
 * costs one row instead of thirty and no transition is ever lost.
 */
async function poll({ ms, stop, tag }) {
  const kept = [];
  const deadline = Date.now() + ms;
  let last = null;
  let lastSig = null;
  let stopped = false;
  while (Date.now() < deadline) {
    const s = await sample(tag);
    const g = sig(s);
    if (g !== lastSig) {
      kept.push(s);
      lastSig = g;
    }
    last = s;
    if (stop && stop(s, kept)) {
      stopped = true;
      break;
    }
  }
  if (last && kept[kept.length - 1] !== last) kept.push(last);
  return { kept, stopped, last };
}

// ------------------------------------------------------------------ geometry
const FLOOR_Y = 99; // block layer; standing surface is FLOOR_Y + 1 = 100
const SURF = FLOOR_Y + 1;
const PADS = {
  // Q1 — one pad per cause, at a distinct XZ so LastDeathLocation is unambiguous.
  void: [0, SURF, 0],
  fall: [20, SURF, 20],
  drown: [40, SURF, 40],
  lava: [60, SURF, 60],
  mob: [80, SURF, 80],
};
const Q2 = { A: [200, SURF, 0], B: [300, SURF, 0], C: [400, SURF, 0] };
// A throwaway death site used before every Q1 trial. `LastDeathLocation` cannot
// be cleared (players reject `/data modify`), and three repeats of one cause die
// on the same block — so "did this death write it?" is only answerable if the
// PREVIOUS value is known to be somewhere else. Every trial is therefore
// preceded by a scrub death here, which is never any cause's death site.
const SCRUB = [-5, SURF, -5];
const at = ([x, y, z]) => `${x + 0.5} ${y} ${z + 0.5}`;

async function buildWorld() {
  // Sealed, still, hostile-free box. The gamerule names are the 1.21.11 ones
  // (snake_case, renamed this version) and every one is `ok()`-checked.
  for (const g of [
    "gamerule spawn_mobs false",
    "gamerule advance_time false",
    "gamerule advance_weather false",
    "gamerule keep_inventory true",
    "gamerule respawn_radius 0",
    "gamerule fall_damage true",
    "gamerule immediate_respawn false",
    // `natural_health_regeneration`, not `naturalRegeneration`: 1.21.11 renamed
    // the whole gamerule registry to snake_case and several rules changed word
    // for word. Regen off matters twice — it would race drowning damage (2 HP/s
    // vs saturation regen 2 HP/s) and it would silently repair the fall damage
    // Q2 is trying to measure.
    "gamerule natural_health_regeneration false",
  ]) {
    await ok(g);
  }
  await ok("time set midnight");
  await ok("weather clear");
  await ok("difficulty hard");
  await ok(`setworldspawn ${PADS.void[0]} ${PADS.void[1]} ${PADS.void[2]}`);

  // Every rig region is FORCELOADED before it is built. `fill` into an unloaded
  // chunk is a no-op with a polite message, and the Q2 pads at x=200/300/400 are
  // far outside the spawn chunks.
  for (const [x0, z0, x1, z1] of [
    [-16, -16, 96, 96],
    [Q2.A[0] - 16, -16, Q2.A[0] + 16, 16],
    [Q2.B[0] - 16, -16, Q2.B[0] + 16, 16],
    [Q2.C[0] - 16, -16, Q2.C[0] + 16, 16],
  ]) {
    await ok(`forceload add ${x0} ${z0} ${x1} ${z1}`);
  }

  // Q1 floor: one slab of stone under every Q1 pad.
  await ok(`fill -6 ${FLOOR_Y} -6 90 ${FLOOR_Y} 90 minecraft:stone`);
  // Drown chamber: a sealed stone box filled with water.
  await ok(`fill 37 ${SURF - 1} 37 43 ${SURF + 6} 43 minecraft:stone`);
  await ok(`fill 38 ${SURF} 38 42 ${SURF + 5} 42 minecraft:water`);
  // Lava pit: a sealed stone box filled with lava.
  await ok(`fill 57 ${SURF - 1} 57 63 ${SURF + 4} 63 minecraft:stone`);
  await ok(`fill 58 ${SURF} 58 62 ${SURF + 3} 62 minecraft:lava`);

  // Q2 floors: A = the fall's own floor, B = the "lift car", C = a deep drop.
  for (const [x, , z] of Object.values(Q2)) {
    await ok(`fill ${x - 5} ${FLOOR_Y} ${z - 5} ${x + 5} ${FLOOR_Y} ${z + 5} minecraft:stone`);
  }

  await ok("scoreboard objectives add dw.deaths deathCount");
  await ok("scoreboard objectives add dw.death_ack dummy");
}

// ------------------------------------------------------------------- bot ops
/**
 * Restore full health. `instant_health` is applied on the target's next tick, so
 * `effect clear` must NOT follow it in the same breath (it removes the pending
 * effect and the player stays hurt); nothing needs clearing anyway, the effect
 * is instantaneous. `natural_health_regeneration` is off for the whole run, so
 * this is the ONLY thing that heals and the caller must confirm it landed.
 */
async function heal() {
  await ok(`effect give ${BOT} minecraft:instant_health 1 10 true`);
}
async function resetBot(bot, pad) {
  if (!bot.entity || bot.health <= 0) await respawn(bot);
  await batch([
    `gamemode survival ${BOT}`,
    `effect clear ${BOT}`,
    `clear ${BOT}`,
    `kill @e[type=minecraft:zombie]`,
    `tp ${BOT} ${at(pad)}`,
    `advancement revoke ${BOT} only dwspike:killed_by_entity`,
    `advancement revoke ${BOT} only dwspike:hurt_any`,
    `scoreboard players set ${BOT} dw.deaths 0`,
    `scoreboard players set ${BOT} dw.death_ack 0`,
  ]);
  await heal();
  bot.clearControlStates();
  let last = null;
  for (let i = 0; i < 300; i++) {
    const s = await sample("reset");
    last = s;
    const here =
      s.pos && Math.abs(s.pos[0] - (pad[0] + 0.5)) < 1.5 && Math.abs(s.pos[2] - (pad[2] + 0.5)) < 1.5;
    if (s.hp === 20 && s.ground && here && s.deaths === 0 && s.advHurt === false) return s;
    if (s.hp !== null && s.hp > 0 && s.hp < 20 && i % 10 === 9) await heal();
    // A corpse from the previous trial: dismiss the death screen and re-reset.
    // `bot.health` is the client's belief and can still read alive at this
    // point, so the retry keys off the SERVER's Health, not the bot's.
    if (s.hp === 0) {
      bot.respawn();
      await sleep(200);
      await batch([
        `tp ${BOT} ${at(pad)}`,
        `scoreboard players set ${BOT} dw.deaths 0`,
        `scoreboard players set ${BOT} dw.death_ack 0`,
        `advancement revoke ${BOT} only dwspike:hurt_any`,
        `advancement revoke ${BOT} only dwspike:killed_by_entity`,
      ]);
      await heal();
      continue;
    }
    await sleep(50);
  }
  throw new Error(`bot did not settle at the reset pad; last sample ${JSON.stringify(last)}`);
}
async function respawn(bot) {
  for (let i = 0; i < 100; i++) {
    if (bot.health > 0 && bot.entity) return;
    bot.respawn();
    await sleep(200);
  }
  throw new Error("bot did not respawn");
}

// ------------------------------------------------------------------------ Q1
const CAUSES = {
  void: async () => {
    // Out-of-world damage: below (min build height - 64). Flat world floor is -64.
    await ok(`tp ${BOT} ${PADS.void[0] + 0.5} -300 ${PADS.void[2] + 0.5}`);
  },
  fall: async () => {
    await ok(`tp ${BOT} ${PADS.fall[0] + 0.5} ${SURF + 100} ${PADS.fall[2] + 0.5}`);
  },
  drown: async () => {
    await ok(`tp ${BOT} 40.5 ${SURF + 1} 40.5`);
  },
  lava: async () => {
    await ok(`tp ${BOT} 60.5 ${SURF + 1} 60.5`);
  },
  mob: async () => {
    await ok(
      `summon minecraft:zombie ${PADS.mob[0] + 2.5} ${SURF} ${PADS.mob[2] + 0.5} ` +
        `{PersistenceRequired:1b,attributes:[{id:"minecraft:attack_damage",base:200.0}],` +
        `Tags:["dw_spike_mob"]}`,
    );
  },
};
const CAUSE_TIMEOUT = { void: 15000, fall: 20000, drown: 90000, lava: 20000, mob: 40000 };

async function q1Trial(bot, cause, rep) {
  // Scrub: die somewhere that is not this cause's death site, so the trial's own
  // LastDeathLocation write is detectable as a CHANGE rather than assumed. The
  // scrub is CONFIRMED by reading the value back, not by having issued `/kill`:
  // a scrub that quietly did not happen would turn every repeat after the first
  // into "LastDeathLocation absent", which is the exact false negative this
  // whole trial exists to rule out.
  //
  // The scrub death is dealt with `/damage <player> 1000 minecraft:generic`, NOT
  // `/kill`. `/kill <player>` answers "Killed <player>" and then, intermittently,
  // does not kill: measured 2 of 3 on this build (see the findings note,
  // "unresolved"). `/damage` killed on the issuing tick in every observation.
  // The scrub is still confirmed by reading the value back and re-issued if it
  // did not land, and the number of attempts is recorded — a scrub that quietly
  // did not happen would turn every repeat after the first into "no
  // LastDeathLocation", which is the exact false negative this trial exists to
  // rule out.
  const scrubTag = `${SCRUB[0]}, ${SCRUB[1]}, ${SCRUB[2]}`;
  await resetBot(bot, SCRUB);
  let scrubAttempts = 0;
  let scrubbed = false;
  for (let i = 0; i < 200; i++) {
    if (i % 40 === 0) {
      await ok(`damage ${BOT} 1000 minecraft:generic`);
      scrubAttempts++;
    }
    const s = await sample(`${cause}/scrub`);
    if (s.ldl && s.ldl.includes(scrubTag)) {
      scrubbed = true;
      break;
    }
    if (s.hp === 0) bot.respawn();
    await sleep(50);
  }
  if (!scrubbed) throw new Error(`scrub death never wrote LastDeathLocation ${scrubTag}`);
  await respawn(bot);
  const before = await resetBot(bot, PADS[cause]);
  if (!before.ldl || !before.ldl.includes(scrubTag)) {
    throw new Error(`LastDeathLocation moved off the scrub before the trial: ${before.ldl}`);
  }
  const preLdl = before.ldl;
  await CAUSES[cause]();
  const induced = Date.now() - T0;

  // Phase 1 — run to the death edge (deathCount ticks up) WITHOUT respawning.
  const dying = await poll({
    tag: `${cause}/dying`,
    ms: CAUSE_TIMEOUT[cause],
    stop: (s) => s.deaths > 0,
  });
  // Phase 2 — hold the corpse on the death screen for 3s and keep sampling.
  const holding = dying.stopped ? await poll({ tag: `${cause}/dead-held`, ms: 3000 }) : { kept: [] };
  // Phase 3 — dismiss the death screen, then sample the respawned player.
  await respawn(bot);
  const after = await poll({ tag: `${cause}/respawned`, ms: 2500 });

  const samples = [before, ...dying.kept, ...holding.kept, ...after.kept];

  // A sample is ONE pipelined rcon batch, and a batch is not guaranteed to be
  // executed inside a single server tick: a batch that straddles the killing
  // tick reports e.g. `Health: 4` next to `deaths: 1`. So nothing below is read
  // off one sample. The death-screen window lasts as long as we decline to
  // respawn, so every "at the moment of death" claim is taken from the HOLD set
  // — the samples in which the player is unambiguously a corpse (Health 0,
  // death counted, respawn screen not yet dismissed) — and the per-field first
  // tick is reported separately for the timing question.
  const hold = samples.filter((s) => s.hp === 0 && s.deaths > 0);
  const firstDeath = samples.find((s) => s.deaths > 0) ?? null;
  const firstHp0 = samples.find((s) => s.hp === 0) ?? null;
  const lastAlive = [...samples].reverse().find((s) => s.hp > 0 && s.ms < (firstHp0?.ms ?? Infinity));
  const firstLdl = samples.find((s) => s.ldl && s.ldl !== preLdl) ?? null;
  const firstAdvKilled = samples.find((s) => s.advKilled === true) ?? null;
  const firstAdvHurt = samples.find((s) => s.advHurt === true) ?? null;
  const afterRespawn = samples.filter((s) => s.tag.endsWith("respawned"));
  const firstGuard = afterRespawn.find((s) => s.guard === true) ?? null;
  const holdLdl = hold.filter((s) => s.ldl && s.ldl !== preLdl);

  const summary = {
    cause,
    rep,
    induced_ms: induced,
    scrub_attempts: scrubAttempts,
    died: !!firstDeath,

    // --- the engine's edge: the vanilla `deathCount` criterion ---------------
    deathcount_edge_tick: firstDeath?.tick ?? null,
    hp_zero_tick: firstHp0?.tick ?? null,
    edge_minus_hp0_ticks: firstDeath && firstHp0 ? firstDeath.tick - firstHp0.tick : null,
    // Was the edge observable while the player was still on the death screen?
    edge_before_respawn: hold.length > 0,
    hold_samples: hold.length,
    hold_span_ticks: hold.length ? hold[hold.length - 1].tick - hold[0].tick : 0,

    // --- position at that moment --------------------------------------------
    last_alive_pos: lastAlive?.pos ?? null,
    corpse_pos_first: hold[0]?.pos ?? null,
    corpse_pos_last: hold[hold.length - 1]?.pos ?? null,
    // A corpse is still an entity: if it was falling it keeps falling, so the
    // readable position drifts away from the death position.
    corpse_pos_drift: hold.length
      ? Number((hold[0].pos[1] - hold[hold.length - 1].pos[1]).toFixed(3))
      : null,

    // --- LastDeathLocation ---------------------------------------------------
    ldl_before: preLdl,
    ldl_first_tick: firstLdl?.tick ?? null,
    ldl_value: firstLdl?.ldl ?? null,
    ldl_lag_ticks: firstLdl && firstDeath ? firstLdl.tick - firstDeath.tick : null,
    ldl_readable_while_dead: holdLdl.length > 0,

    // --- advancement probes ---------------------------------------------------
    adv_killed_by_entity: !!firstAdvKilled,
    adv_killed_tick: firstAdvKilled?.tick ?? null,
    adv_killed_while_dead: hold.some((s) => s.advKilled === true),
    adv_hurt_any: !!firstAdvHurt,
    adv_hurt_tick: firstAdvHurt?.tick ?? null,
    adv_hurt_hp_at_fire: firstAdvHurt?.hp ?? null,

    // --- when does cp_respawn_check actually fire? ----------------------------
    edge_armed_while_dead: hold.some((s) => s.edgeArmed === true),
    guard_true_while_dead: hold.some((s) => s.guard === true),
    guard_first_tick_after_respawn: firstGuard?.tick ?? null,
    guard_lag_from_edge_ticks: firstGuard && firstDeath ? firstGuard.tick - firstDeath.tick : null,
    guard_pos: firstGuard?.pos ?? null,
  };
  console.log(
    `[q1] ${cause} #${rep}: died=${summary.died} edge@${summary.deathcount_edge_tick} ` +
      `(hp0@${summary.hp_zero_tick}) pre-respawn=${summary.edge_before_respawn}(${summary.hold_samples} samples) ` +
      `corpse=${JSON.stringify(summary.corpse_pos_first)} drift=${summary.corpse_pos_drift} ` +
      `LDL=${summary.ldl_value ? `${summary.ldl_value} (+${summary.ldl_lag_ticks}t, whileDead=${summary.ldl_readable_while_dead})` : "ABSENT"} ` +
      `advKilled=${summary.adv_killed_by_entity} advHurt=${summary.adv_hurt_any} ` +
      `edgeArmedWhileDead=${summary.edge_armed_while_dead} guardWhileDead=${summary.guard_true_while_dead} ` +
      `guardAfterRespawn@${summary.guard_first_tick_after_respawn}`,
  );
  return { summary, samples };
}

// ------------------------------------------------------------------------ Q2
/**
 * One teleport-during-fall trial.
 *
 *   h      launch height above floor A, in blocks
 *   when   "none"  — no teleport; the control that fixes what the fall costs
 *          "early" — teleport a quarter of the way down
 *          "late"  — teleport with ~15% of the drop left (never below floor+1.2)
 *   dest   "car"   — standing ON floor B: the lift car arrives under the player
 *          "car+1" — 1 block above floor B: the car arrives just below them
 *          "deep"  — 160 blocks above floor C, so the player KEEPS falling after
 *                    the teleport and the fall_distance trajectory itself is
 *                    observable rather than only its consequence
 *
 * `when` is only a way to pick a trigger height; the independent variable that
 * is REPORTED is `fall_pre_tp`, the accumulated distance the server actually had
 * on the player at the instant of the teleport. The teleport is issued from
 * inside the sampling loop on the server's own Y reading, and the sample
 * immediately before and immediately after the `tp` are both kept, so the
 * transition is measured rather than inferred.
 *
 * A very fast fall can reach the floor between the trigger sample and the `tp`
 * landing on a tick. That is recorded as `tp_missed`, never silently averaged in.
 */
async function q2Run(bot, cfg, rep) {
  const before = await resetBot(bot, Q2.A);
  const startY = SURF + cfg.h;
  // Two ways to pick the teleport instant. `fallTrigger` is the one that
  // answers the design question directly — teleport the player at a chosen
  // ACCUMULATED FALL DISTANCE, sweep it, and read off the value at which the
  // arriving car stops being a rescue and becomes the thing that kills them.
  const triggerY =
    cfg.fallTrigger != null ? -Infinity
    : cfg.when === "early" ? startY - Math.max(1.5, 0.25 * cfg.h)
    : Math.max(SURF + 1.2, startY - 0.85 * cfg.h);
  const destPos =
    cfg.dest === "car" ? `${Q2.B[0] + 0.5} ${SURF} ${Q2.B[2] + 0.5}`
    : cfg.dest === "car+1" ? `${Q2.B[0] + 0.5} ${SURF + 1} ${Q2.B[2] + 0.5}`
    : cfg.dest === "deep" ? `${Q2.C[0] + 0.5} ${SURF + 160} ${Q2.C[2] + 0.5}`
    : null;

  const samples = [];
  let lastSig = null;
  const keep = (s) => {
    const g = sig(s);
    if (g !== lastSig) {
      samples.push(s);
      lastSig = g;
    }
  };

  // Phase 0 — launch. Nothing may be judged until the SERVER agrees the player
  // is at the launch height; the reset pad is itself on the ground at floor A,
  // so a trigger test run before the launch teleport lands is trivially true.
  await ok(`tp ${BOT} ${Q2.A[0] + 0.5} ${startY} ${Q2.A[2] + 0.5}`);
  let launch = null;
  for (let i = 0; i < 400; i++) {
    const s = await sample(`q2/${cfg.h}/${cfg.when}/${cfg.dest ?? "none"}/launch`);
    keep(s);
    if (s.pos && Math.abs(s.pos[1] - startY) < 0.9 && !s.ground) {
      launch = s;
      break;
    }
  }
  if (!launch) throw new Error(`q2: player never reached the launch height ${startY}`);

  // Phase 1 — the fall, with the teleport trigger inline.
  let tpDone = destPos === null;
  let tpMissed = false;
  let preTp = null;
  let postTp = null;
  let lastAirborne = null;
  let landed = null;
  const tag = `q2/${cfg.h}/${cfg.when}/${cfg.dest ?? "none"}`;
  const deadline = Date.now() + 40000;
  while (Date.now() < deadline) {
    const s = await sample(tag);
    keep(s);
    if (s.hp !== null && s.hp > 0 && !s.ground) lastAirborne = s;
    if (!tpDone) {
      if (s.ground) {
        tpMissed = true;
        tpDone = true; // the fall finished before the trigger height was seen
      } else if (
        cfg.fallTrigger != null ? s.fall >= cfg.fallTrigger : s.pos && s.pos[1] <= triggerY
      ) {
        preTp = s;
        await ok(`tp ${BOT} ${destPos}`);
        postTp = await sample(`${tag}/post-tp`);
        keep(postTp);
        tpDone = true;
        continue;
      }
    }
    if (s.deaths > 0) {
      landed = s;
      break;
    }
    if (tpDone && s.ground && s.fall === 0 && s.ms - (postTp?.ms ?? launch.ms) > 400) {
      landed = s;
      break;
    }
  }
  const settle = await poll({ tag: `${tag}/settle`, ms: 1200 });
  settle.kept.forEach(keep);
  const died = samples.some((s) => s.deaths > 0);
  if (died) await respawn(bot);

  const hps = samples.filter((s) => s.hp !== null && s.ms >= launch.ms).map((s) => s.hp);
  const hpMin = hps.length ? Math.min(...hps) : null;
  const after = postTp ? samples.filter((s) => s.ms >= postTp.ms) : [];
  const end = settle.last ?? landed;
  const summary = {
    ...cfg,
    rep,
    start_y: startY,
    trigger_y: destPos && cfg.fallTrigger == null ? Number(triggerY.toFixed(2)) : null,
    tp_missed: tpMissed,
    hp_before: before.hp,
    // The state at the instant of the teleport.
    fall_pre_tp: preTp?.fall ?? null,
    y_pre_tp: preTp?.pos?.[1] ?? null,
    tick_pre_tp: preTp?.tick ?? null,
    fall_post_tp: postTp?.fall ?? null,
    y_post_tp: postTp?.pos?.[1] ?? null,
    tick_post_tp: postTp?.tick ?? null,
    // Does the accumulated distance survive the teleport?
    fall_carried: preTp && postTp ? Number((postTp.fall - preTp.fall).toFixed(4)) : null,
    peak_fall_after_tp: after.length ? Math.max(...after.map((s) => s.fall ?? 0)) : null,
    // The last airborne reading is the distance the landing was charged for.
    fall_at_last_airborne: lastAirborne?.fall ?? null,
    peak_fall_overall: Math.max(...samples.filter((s) => s.ms >= launch.ms).map((s) => s.fall ?? 0)),
    died,
    hp_min: hpMin,
    damage: died ? "lethal" : Number(((before.hp ?? 20) - (hpMin ?? 0)).toFixed(2)),
    end_pos: end?.pos ?? null,
  };
  console.log(
    `[q2] h=${cfg.h} ${cfg.fallTrigger != null ? `fall>=${cfg.fallTrigger}` : cfg.when}->${cfg.dest ?? "none"} #${rep}: ` +
      (tpMissed ? "TP MISSED (landed first) " : "") +
      `fall pre=${summary.fall_pre_tp?.toFixed?.(2) ?? "-"} post=${summary.fall_post_tp?.toFixed?.(2) ?? "-"} ` +
      `carried=${summary.fall_carried ?? "-"} lastAir=${summary.fall_at_last_airborne?.toFixed?.(2) ?? "-"} ` +
      `-> ${died ? "DIED" : `hp ${summary.hp_min} (dmg ${summary.damage})`}`,
  );
  return { summary, samples };
}

// ------------------------------------------------------------------ incidental
/**
 * The legacy camelCase gamerule identifiers, probed live. `crates/admit/src/
 * gallery.rs` and `tools/spike-jump-arc/measure.mjs` both still emit some of
 * these; recording the server's verdict here makes the claim checkable.
 */
async function legacyGameruleProbe() {
  const rows = [];
  for (const g of [
    "fallDamage",
    "doDaylightCycle",
    "doWeatherCycle",
    "doMobSpawning",
    "doImmediateRespawn",
    "keepInventory",
    "spawnRadius",
    "naturalRegeneration",
  ]) {
    const r = await rcon(`gamerule ${g}`);
    rows.push({ legacy: g, response: r, accepted: !ERR.test(r) });
  }
  return rows;
}

/**
 * The 1.21.11 gamerule registry, MEASURED: run.sh dumps candidate identifier
 * strings out of the pinned server jar's constant pool, and every candidate is
 * offered to `/gamerule` here. Accepted ones are the real registry (with their
 * default value on this server); the rest are noise from the same class.
 */
async function gameruleRegistry() {
  const path = process.env.SPIKE_GAMERULE_CANDIDATES;
  if (!path) return null;
  const cands = readFileSync(path, "utf8").split("\n").map((s) => s.trim()).filter(Boolean);
  const res = await batch(cands.map((c) => `gamerule ${c}`));
  return cands
    .map((name, i) => ({ name, accepted: !ERR.test(res[i]), response: res[i] }))
    .filter((r) => r.accepted)
    .map((r) => ({ name: r.name, default: (r.response.match(/currently set to: (.*)$/) ?? [])[1] ?? null }));
}

/**
 * How long is a player invulnerable after respawning, and what do `/damage` and
 * `/kill` do inside that window?
 *
 * This started as an instrument defect: the Q1 scrub death needed exactly two
 * `/damage` issues in all 15 trials, and `/kill <player>` had already been seen
 * to answer "Killed <player>" and not kill. Both are the same window. `/damage`
 * at least says so ("Target is invulnerable to the given damage type");
 * `/kill` reports success either way, which is the dangerous half.
 *
 * Measured per repeat: the tick the player's Health returns non-zero (respawn),
 * the tick a 1-point `/damage` first lands, and whether a `/kill` issued at the
 * top of the window did anything.
 */
async function respawnInvulnerabilityProbe(bot) {
  const rows = [];
  for (let rep = 1; rep <= 2; rep++) {
    await resetBot(bot, SCRUB);
    // Die (retrying, since this issue may itself land in a previous window).
    let died = false;
    for (let i = 0; i < 200 && !died; i++) {
      if (i % 40 === 0) await ok(`damage ${BOT} 1000 minecraft:generic`);
      died = (await sample("inv/dying")).hp === 0;
      if (!died) await sleep(50);
    }
    if (!died) throw new Error("respawn-invulnerability probe: could not kill the player");
    // Respawn, and note the tick the server first reports us alive.
    bot.respawn();
    let respawnTick = null;
    for (let i = 0; i < 200; i++) {
      const s = await sample("inv/respawning");
      if (s.hp > 0) {
        respawnTick = s.tick;
        break;
      }
      bot.respawn();
      await sleep(25);
    }
    if (respawnTick === null) throw new Error("respawn-invulnerability probe: never respawned");
    // A `/kill` at the top of the window, then hammer a 1-point `/damage` until
    // it is accepted.
    const killResponse = await rcon(`kill ${BOT}`);
    let landedTick = null;
    let refusal = null;
    let refusals = 0;
    for (let i = 0; i < 400; i++) {
      const r = await rcon(`damage ${BOT} 1 minecraft:generic`);
      if (r.startsWith("Applied")) {
        landedTick = (await sample("inv/landed")).tick;
        break;
      }
      refusal = r;
      refusals++;
      await sleep(25);
    }
    const after = await sample("inv/after");
    rows.push({
      rep,
      respawn_tick: respawnTick,
      damage_landed_tick: landedTick,
      window_ticks: landedTick === null ? null : landedTick - respawnTick,
      refusals,
      refusal_text: refusal,
      kill_response: killResponse,
      kill_actually_killed: after.hp === 0,
      hp_after: after.hp,
    });
    console.log(
      `[inv] rep ${rep}: respawn@${respawnTick} damage accepted@${landedTick} ` +
        `(window ${rows[rows.length - 1].window_ticks} ticks, ${refusals} refusals); ` +
        `\`kill\` said ${JSON.stringify(killResponse)} and killed=${after.hp === 0}`,
    );
  }
  return rows;
}

/** Which player-NBT spellings exist on 1.21.11 (the names this rig reads). */
async function nbtFieldProbe() {
  const fields = [
    "Pos", "Health", "fall_distance", "FallDistance", "LastDeathLocation",
    "OnGround", "on_ground", "Motion", "motion", "Air", "DeathTime",
  ];
  const res = await batch(fields.map((f) => `data get entity ${BOT} ${f}`));
  return fields.map((f, i) => ({ field: f, present: nbt(res[i]) !== null, response: res[i] }));
}

// ----------------------------------------------------------------------- main
async function main() {
  // `dw.sync` is the batch fence, so it has to exist BEFORE the first fenced
  // batch — created here on the raw channel, one command in, one line out.
  proc.stdin.write("scoreboard objectives add dw.sync dummy\n");
  await readLines(1);

  const version = await rcon("version");
  const datapacks = await rcon("datapack list");
  console.log(`[spike] ${version}`);
  console.log(`[spike] ${datapacks}`);
  if (!/dwspike|dw-spike|spikepack/i.test(datapacks) && !/file\/dw-spike/.test(datapacks)) {
    console.log("[spike] WARNING: spike datapack not visible in `datapack list`");
  }

  const bot = mineflayer.createBot({
    host: "127.0.0.1",
    port: PORT,
    username: BOT,
    auth: "offline",
    respawn: false, // the corpse must stay on the death screen until we dismiss it
  });
  await new Promise((res, rej) => {
    bot.once("spawn", res);
    bot.once("kicked", (r) => rej(new Error(`kicked: ${JSON.stringify(r)}`)));
    bot.once("error", rej);
  });
  console.log(`[spike] bot spawned (protocol version ${bot.version})`);
  await sleep(1500);
  // Self-check the boolean-probe spelling before anything depends on it: one
  // condition that must hold (the bot is online) and one that must not.
  {
    const [t, f] = await batch([
      `execute if entity ${BOT} ${PROBE}`,
      `execute if entity @e[type=minecraft:creeper] ${PROBE}`,
    ]);
    if (flag(t) !== true) throw new Error("probe self-check: a true condition printed nothing");
    if (flag(f) !== false) throw new Error("probe self-check: a false condition printed something");
  }
  await buildWorld();
  await ok(`tp ${BOT} ${at(PADS.void)}`);
  await sleep(500);

  const out = {
    meta: {
      version_response: version,
      datapack_list: datapacks,
      bot_version: bot.version,
      mineflayer: require("mineflayer/package.json").version,
      container: CONTAINER,
      q1_reps: Q1_REPS,
      q2_reps: Q2_REPS,
    },
    incidental: {},
    q1: [],
    q2: [],
  };

  out.incidental.nbt_fields = await nbtFieldProbe();
  out.incidental.legacy_gamerules = await legacyGameruleProbe();
  out.incidental.gamerule_registry = await gameruleRegistry();
  out.incidental.respawn_invulnerability = await respawnInvulnerabilityProbe(bot);
  console.log("[spike] legacy gamerule identifiers accepted on 1.21.11: " +
    out.incidental.legacy_gamerules.filter((r) => r.accepted).map((r) => r.legacy).join(", ") || "(none)");

  // --- Q1
  for (const cause of Object.keys(CAUSES)) {
    for (let rep = 1; rep <= Q1_REPS; rep++) {
      out.q1.push(await q1Trial(bot, cause, rep));
    }
  }

  // --- Q2
  const q2cfgs = [];
  for (const h of [6, 30, 120]) {
    q2cfgs.push({ h, when: "none", dest: null });
    for (const when of ["early", "late"]) {
      for (const dest of ["car", "car+1"]) q2cfgs.push({ h, when, dest });
    }
  }
  for (const h of [30, 120]) q2cfgs.push({ h, when: "late", dest: "deep" });
  // The catch-threshold sweep: one long shaft, the teleport fired at a chosen
  // accumulated fall distance, straight onto the car's surface.
  for (const fallTrigger of [1, 5, 10, 18, 20, 22, 23, 24, 30]) {
    q2cfgs.push({ h: 120, when: "fall", dest: "car", fallTrigger });
  }
  for (const cfg of q2cfgs) {
    for (let rep = 1; rep <= Q2_REPS; rep++) {
      out.q2.push(await q2Run(bot, cfg, rep));
    }
  }

  // Channel resolution, so the sampling rate is part of the record.
  const rtts = [...out.q1, ...out.q2].flatMap((t) => t.samples.map((s) => s.rtt)).sort((a, b) => a - b);
  out.meta.channel = {
    samples: rtts.length,
    rtt_ms_p50: rtts[Math.floor(rtts.length * 0.5)],
    rtt_ms_p95: rtts[Math.floor(rtts.length * 0.95)],
    rtt_ms_max: rtts[rtts.length - 1],
  };
  console.log(`[spike] rcon batch rtt p50=${out.meta.channel.rtt_ms_p50}ms p95=${out.meta.channel.rtt_ms_p95}ms`);

  writeFileSync(OUT, JSON.stringify(out, null, 1));
  console.log(`[spike] raw observations -> ${OUT}`);
  bot.end();
  proc.stdin.end();
  process.exit(0);
}

main().catch((e) => {
  console.error("[spike] FAILED:", e);
  process.exit(1);
});
