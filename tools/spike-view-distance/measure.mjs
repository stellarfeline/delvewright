#!/usr/bin/env node
// SPIKE TOOLING (view distance) — NOT part of the shipped pipeline, not wired
// into CI. Driven by `run.sh` against a throwaway pinned 1.21.11 server, once
// per cell of its matrix. Appends one cell record to `SPIKE_OUT`.
//
// The questions, each answered by the server's own state and the bots' packets:
//   A  which chunks a client is SENT at this `view-distance`: how many, the shape
//      of the set around the player's chunk, and the radius a body is guaranteed
//      to see in every direction from any standing position in its chunk;
//   B  what four clients standing in four separate discs cost the server: tick
//      time (`tick query`), the container's CPU and RSS (`docker stats`), and the
//      JVM's heap after each collection (`/data/gc.log`);
//   C  (cells marked `track`) the distance at which the server stops tracking a
//      `block_display` and an `armor_stand` for a client, under this view
//      distance.
//
// Every rcon reply is read: setup goes through `run` (throws on refusal).

import { createRequire } from "node:module";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { execFile } from "node:child_process";
import { promisify } from "node:util";

import { rconChannel } from "../lib/rcon.mjs";

const require = createRequire(new URL("../../harness/package.json", import.meta.url));
const mineflayer = require("mineflayer");
const execFileP = promisify(execFile);

const CONTAINER = process.env.SPIKE_CONTAINER ?? "dw-spike-view-distance";
const PORT = Number(process.env.SPIKE_PORT);
const OUT = process.env.SPIKE_OUT ?? new URL("./observations.json", import.meta.url).pathname;
const HORIZON = process.env.SPIKE_HORIZON ?? "ocean";
const VIEW = Number(process.env.SPIKE_VIEW);
const SIM = Number(process.env.SPIKE_SIM);
const TRACK = (process.env.SPIKE_TRACK ?? "") === "track";
const HEAP = process.env.SPIKE_HEAP ?? "";
const PROPS = process.env.SPIKE_PROPS ?? "";
// The delve's player cap (CLAUDE.md: a fixed group of 1–4 players), each in its
// own disc: 2048 blocks apart is four times the widest disc vanilla can serve
// (32 chunks = 512 blocks), so no chunk is shared and the cost is the sum.
const PLAYERS = 4;
const CLIENT_VIEW_DISTANCE = Number(process.env.SPIKE_CLIENT_VIEW ?? 32);
const STATION_STRIDE = 2048;
// Above the ocean flat's surface (y 127), so a spectator hovers over open water.
const STATION_Y = 130;
const SETTLE_STABLE_MS = Number(process.env.SPIKE_SETTLE_STABLE_MS ?? 15_000);
const SETTLE_MAX_MS = 900_000;
const WINDOW_MS = 60_000;
const SAMPLE_EVERY_MS = 10_000;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const r = rconChannel(CONTAINER);
const replies = {};

async function run(cmd, key) {
  const reply = await r.run(cmd);
  if (key) replies[key] = { cmd, reply };
  return reply;
}

function parseTick(reply) {
  const avg = reply.match(/Average time per tick: ([\d.]+)ms/);
  const p50 = reply.match(/P50: ([\d.]+)ms/);
  const p95 = reply.match(/P95: ([\d.]+)ms/);
  const p99 = reply.match(/P99: ([\d.]+)ms/);
  const n = reply.match(/Sample: (\d+)/);
  return { avg_ms: Number(avg?.[1]), p50_ms: Number(p50?.[1]), p95_ms: Number(p95?.[1]), p99_ms: Number(p99?.[1]), sample: Number(n?.[1]), raw: reply };
}

async function dockerStats() {
  const { stdout } = await execFileP("docker", ["stats", "--no-stream", "--format", "{{.CPUPerc}}\t{{.MemUsage}}", CONTAINER]);
  const [cpu, mem] = stdout.trim().split("\t");
  const used = mem.split("/")[0].trim();
  return { cpu_percent: Number(cpu.replace("%", "")), rss: used, rss_mib: toMiB(used), raw: stdout.trim() };
}

function toMiB(s) {
  const m = String(s).trim().match(/^([\d.]+)\s*([KMG])i?B$/);
  if (!m) return null;
  const v = Number(m[1]);
  return m[2] === "G" ? v * 1024 : m[2] === "K" ? v / 1024 : v;
}

async function gcLog() {
  const { stdout } = await execFileP("docker", ["exec", CONTAINER, "sh", "-c", "cat /data/gc.log 2>/dev/null || true"]);
  return stdout;
}

/** Every collection line, as `{uptime_s, kind, before_m, after_m, committed_m}`. */
function parsePauses(log) {
  const out = [];
  const re = /\[(\d+\.\d+)s\]\s+GC\(\d+\)\s+(Pause [^0-9]*?)\s+(\d+)M->(\d+)M\((\d+)M\)/g;
  let m;
  while ((m = re.exec(log)) !== null) {
    out.push({ uptime_s: Number(m[1]), kind: m[2].trim(), before_m: Number(m[3]), after_m: Number(m[4]), committed_m: Number(m[5]) });
  }
  return out;
}

/**
 * The shape of a sent chunk set, as offsets from the player's chunk.
 *
 * `guaranteed_radius_blocks` is the radius a body is sure to see in every
 * direction: the least distance from any standing position in the player's chunk
 * (a half-block grid over [0,16)²) to the nearest point of any chunk that was NOT
 * sent, over the bounding square of the sent set widened by two chunks.
 */
function shape(offsets) {
  const set = new Set(offsets.map(([a, b]) => `${a},${b}`));
  let cheb = 0, axis = 0, diag = 0;
  for (const [a, b] of offsets) {
    cheb = Math.max(cheb, Math.abs(a), Math.abs(b));
    if (b === 0) axis = Math.max(axis, Math.abs(a));
    if (a === 0) axis = Math.max(axis, Math.abs(b));
    if (Math.abs(a) === Math.abs(b)) diag = Math.max(diag, Math.abs(a));
  }
  const reach = cheb + 2;
  let guaranteed = Infinity;
  for (let a = -reach; a <= reach; a++) {
    for (let b = -reach; b <= reach; b++) {
      if (set.has(`${a},${b}`)) continue;
      for (let u = 0; u < 16; u += 0.5) {
        for (let w = 0; w < 16; w += 0.5) {
          const dx = Math.max(0, 16 * a - u, u - (16 * a + 16));
          const dz = Math.max(0, 16 * b - w, w - (16 * b + 16));
          guaranteed = Math.min(guaranteed, Math.hypot(dx, dz));
        }
      }
    }
  }
  // Does the set match vanilla's inset circle, (|dx|-1)² + (|dz|-1)² < view²?
  // Read from `ChunkTrackingView.isWithinDistance` of the pinned version; a
  // mismatch is printed, never assumed away.
  let insetCircleMatches = true;
  const mismatches = [];
  for (let a = -reach; a <= reach; a++) {
    for (let b = -reach; b <= reach; b++) {
      const i = Math.max(0, Math.abs(a) - 1), j = Math.max(0, Math.abs(b) - 1);
      const predicted = i * i + j * j < VIEW * VIEW;
      const got = set.has(`${a},${b}`);
      if (predicted !== got) { insetCircleMatches = false; if (mismatches.length < 12) mismatches.push([a, b, predicted, got]); }
    }
  }
  // The boundary, for fitting the rule: per dx, the largest |dz| sent.
  const profile = {};
  for (const [a, b] of offsets) profile[a] = Math.max(profile[a] ?? 0, Math.abs(b));
  return {
    chunks: offsets.length,
    profile: Object.keys(profile).map(Number).sort((p, q) => p - q).map((a) => [a, profile[a]]),
    chebyshev_max: cheb,
    axis_max: axis,
    diagonal_max: diag,
    guaranteed_radius_blocks: Number(guaranteed.toFixed(2)),
    inset_circle_matches: insetCircleMatches,
    inset_circle_mismatches: mismatches,
  };
}

async function joinBot(i) {
  const name = `dw_vd${i}`;
  // The client's own render-distance setting travels to the server in its
  // client-information packet, and the server sends min(server view-distance,
  // client setting) plus one ring: measured on this rig with mineflayer's
  // default of 12, a server at 16 sent a Chebyshev-13 set. The bots declare the
  // vanilla maximum so the SERVER's number is what this rig measures.
  const bot = mineflayer.createBot({ host: "127.0.0.1", port: PORT, username: name, version: "1.21.11", auth: "offline", viewDistance: CLIENT_VIEW_DISTANCE });
  await new Promise((res, rej) => { bot.once("spawn", res); bot.once("error", rej); bot.once("kicked", rej); });
  await run(`gamemode spectator ${name}`);
  // The client simulates no physics of its own: a spectator over a void world
  // otherwise falls, client side, and a client that moves is sent a moving set
  // (measured: the void cell's clients read 653, 473 and 5406 columns).
  bot.physicsEnabled = false;
  // Let the spawn batch finish before the far teleport: a bot teleported while
  // its first batch was still streaming reported ZERO columns at its station for
  // the whole settle window (measured on the first run of this rig), which the
  // settle check below refuses rather than records.
  await settle([{ bot }], 5_000);
  const x = i * STATION_STRIDE + 8;
  await run(`tp ${name} ${x} ${STATION_Y} 8`);
  return { name, bot, station: [x, STATION_Y, 8] };
}

function columnsOf(bot) {
  return bot.world.getColumns().map((c) => [Number(c.chunkX), Number(c.chunkZ)]);
}

async function settle(bots, stableMs = SETTLE_STABLE_MS) {
  const t0 = Date.now();
  let last = bots.map(() => -1), lastChange = Date.now();
  while (Date.now() - t0 < SETTLE_MAX_MS) {
    const now = bots.map((b) => columnsOf(b.bot).length);
    if (now.some((n, i) => n !== last[i])) { last = now; lastChange = Date.now(); }
    if (Date.now() - lastChange >= stableMs && now.every((n) => n > 0)) return { seconds: (Date.now() - t0) / 1000, counts: now, stable: true };
    await sleep(1000);
  }
  return { seconds: (Date.now() - t0) / 1000, counts: last, stable: false };
}

async function trackingProbe(bot0, station) {
  // Two tracked kinds at increasing distances east of the first bot, each in a
  // chunk the bot's own view distance keeps loaded; the server either sends the
  // entity or does not.
  const distances = [128, 144, 160, 176, 192, 256, 320, 368];
  const rows = [];
  for (const d of distances) {
    const x = station[0] + d;
    await run(`summon minecraft:block_display ${x} ${station[1]} ${station[2]} {Tags:["dwvd_probe"],block_state:{Name:"minecraft:stone"}}`);
    await run(`summon minecraft:armor_stand ${x} ${station[1] - 1} ${station[2]} {Tags:["dwvd_probe"],NoGravity:1b}`);
  }
  await sleep(4000);
  const seen = Object.values(bot0.entities).filter((e) => e.name === "block_display" || e.name === "armor_stand");
  for (const d of distances) {
    const x = station[0] + d;
    const near = (kind) => seen.some((e) => e.name === kind && Math.abs(e.position.x - x) < 1.5);
    rows.push({ distance_blocks: d, chunks: d / 16, block_display_seen: near("block_display"), armor_stand_seen: near("armor_stand") });
  }
  const serverCount = await run("execute if entity @e[tag=dwvd_probe]", "probe_count");
  await run("kill @e[tag=dwvd_probe]");
  return { rows, server_reply: serverCount };
}

async function main() {
  const t0 = Date.now();
  await run("list", "list");
  const bots = [];
  for (let i = 0; i < PLAYERS; i++) bots.push(await joinBot(i));
  const settled = await settle(bots);
  if (!settled.stable) throw new Error(`chunk sets never settled: ${JSON.stringify(settled)}`);
  const perBot = bots.map((b) => {
    const cx = Math.floor(b.bot.entity.position.x / 16), cz = Math.floor(b.bot.entity.position.z / 16);
    const offsets = columnsOf(b.bot).map(([a, c]) => [a - cx, c - cz]);
    return { name: b.name, station: b.station, chunk: [cx, cz], ...shape(offsets) };
  });
  // The window: every 10 s a tick reading and a stats reading.
  const gcStartLen = parsePauses(await gcLog()).length;
  const windowStart = Date.now();
  const ticks = [], stats = [];
  while (Date.now() - windowStart < WINDOW_MS) {
    await sleep(SAMPLE_EVERY_MS);
    ticks.push(parseTick(await run("tick query")));
    stats.push(await dockerStats());
  }
  const pauses = parsePauses(await gcLog());
  const inWindow = pauses.slice(gcStartLen);
  const heap = {
    collections_in_window: inWindow.length,
    live_set_min_after_mib: inWindow.length ? Math.min(...inWindow.map((p) => p.after_m)) : null,
    max_after_mib: inWindow.length ? Math.max(...inWindow.map((p) => p.after_m)) : null,
    max_before_mib: inWindow.length ? Math.max(...inWindow.map((p) => p.before_m)) : null,
    committed_mib: inWindow.length ? inWindow[inWindow.length - 1].committed_m : null,
    all_time_max_before_mib: pauses.length ? Math.max(...pauses.map((p) => p.before_m)) : null,
    window_pauses: inWindow.slice(-12),
  };
  let tracking = null;
  if (TRACK) tracking = await trackingProbe(bots[0].bot, bots[0].station);
  const cell = {
    horizon: HORIZON,
    view_distance: VIEW,
    simulation_distance: SIM,
    heap_ceiling: HEAP,
    server_properties: PROPS.trim().split(/\s+/).filter(Boolean),
    players: PLAYERS,
    client_view_distance: CLIENT_VIEW_DISTANCE,
    settle: settled,
    sent_chunks: perBot,
    sent_chunks_total: perBot.reduce((s, b) => s + b.chunks, 0),
    tick: {
      samples: ticks.map(({ raw, ...t }) => t),
      avg_ms_mean: Number((ticks.reduce((s, t) => s + t.avg_ms, 0) / ticks.length).toFixed(2)),
      p95_ms_max: Math.max(...ticks.map((t) => t.p95_ms)),
    },
    container: {
      samples: stats,
      cpu_percent_mean: Number((stats.reduce((s, x) => s + x.cpu_percent, 0) / stats.length).toFixed(1)),
      rss_mib_max: Math.max(...stats.map((x) => x.rss_mib ?? 0)),
    },
    heap,
    tracking,
    replies,
    seconds: (Date.now() - t0) / 1000,
  };
  for (const b of bots) b.bot.quit();
  const doc = existsSync(OUT)
    ? JSON.parse(readFileSync(OUT, "utf8"))
    : {
        instrument: {
          server: "itzg/minecraft-server@sha256:3e7db256… (versions.toml [images.base]), vanilla 1.21.11, flat world, adventure; JVM_OPTS -Xlog:gc + G1 periodic cycle every 10 s",
          bot: "mineflayer 4.37.1 (harness pin), offline, protocol 1.21.11; four bots in spectator, 2048 blocks apart",
          rig: "tools/spike-view-distance/run.sh + measure.mjs",
          host: `${process.platform} ${process.arch}, node ${process.version}`,
          heap_live_set: "the smallest heap-after-collection in the 60 s window, read from /data/gc.log (method 1); the container's RSS from `docker stats` (method 2, no shared configuration)",
        },
        cells: [],
      };
  doc.cells.push(cell);
  writeFileSync(OUT, JSON.stringify(doc, null, 2) + "\n");
  const g = perBot[0];
  console.log(
    `[spike] ${HORIZON} view ${VIEW} sim ${SIM}: ${g.chunks} chunks/client (cheb ${g.chebyshev_max}, axis ${g.axis_max}, diag ${g.diagonal_max}, ` +
      `guaranteed ${g.guaranteed_radius_blocks} blocks, inset-circle ${g.inset_circle_matches}); settle ${settled.seconds}s stable=${settled.stable}; ` +
      `tick avg ${cell.tick.avg_ms_mean} ms p95 ${cell.tick.p95_ms_max}; cpu ${cell.container.cpu_percent_mean}%; rss max ${cell.container.rss_mib_max} MiB; ` +
      `heap live ${heap.live_set_min_after_mib} MiB (max before ${heap.max_before_mib}, committed ${heap.committed_mib}, ${heap.collections_in_window} collections)`,
  );
  if (tracking) console.log("[spike] tracking:", JSON.stringify(tracking.rows));
  process.exit(0);
}

main().catch((e) => {
  console.error("[spike] FAILED:", e);
  process.exit(1);
});
