// Every stretch the bot spent unable to be hurt because the server did not yet
// count it as loaded — measured off the wire, not off the code that is meant to
// end it.
//
// The rule (pinned 1.21.11; `client-loaded.ts` has the reading): from every
// `login` and every `respawn` the server refuses ALL damage to the player until
// the client sends `player_loaded`, or until 60 server ticks have passed. So a
// window opens on each `login`/`respawn` packet the bot receives and closes on
// the first OUTGOING `player_loaded` this tracer sees on the bot's socket — or,
// if none is sent, at the server's own 60-tick fallback, which is then the
// window's whole length.
//
// The tracer observes `_client.write` itself rather than asking the tracker in
// `client-loaded.ts` whether it sent anything, so a bot built without that
// tracker, or a tracker that never fires, is caught by an observer that shares
// none of its logic.
//
// While a window is open it records everything that would have hurt a body
// that could be hurt — every one of them a damage event the window absorbed:
//
//   * `swing`  — a non-player mob within melee reach swung its arm (the server
//                sends the swing with every melee attack, hit or not);
//   * `hurt`   — a `damage_event` naming the bot (impossible inside a window;
//                recorded so the rule itself is re-checked on every run);
//   * `hazard` — the bot's feet or head in a block that damages on contact;
//   * `lethal` — the bot's body inside a declared lethal volume;
//   * `fall`   — a landing more than 3 blocks below the highest point since the
//                window opened (vanilla's fall-damage threshold);
//   * `blast`  — an explosion within 8 blocks.

import { bodyInVolume, type Box } from "./death-loop.ts";

import { SERVER_LOAD_TIMEOUT_TICKS } from "./client-loaded.ts";
const TICK_MS = 50;
const MELEE_REACH = 4.5;
const BLAST_RADIUS = 8;
const FALL_THRESHOLD = 3;
const HAZARD_BLOCKS = new Set([
  "lava",
  "fire",
  "soul_fire",
  "magma_block",
  "campfire",
  "soul_campfire",
  "cactus",
  "sweet_berry_bush",
  "wither_rose",
  "powder_snow",
  "pointed_dripstone",
]);

export type AbsorbedKind = "swing" | "hurt" | "hazard" | "lethal" | "fall" | "blast";

export interface AbsorbedEvent {
  readonly kind: AbsorbedKind;
  /** ms after the window opened. */
  readonly atMs: number;
  readonly detail: string;
}

export interface LoadWindowRecord {
  readonly cause: "login" | "respawn";
  /** The ladder stage the bot was in when the window opened. */
  readonly stage: string;
  /** The critical-path step index at that moment (-1 before the first step). */
  readonly step: number;
  readonly openedAt: number;
  /** ms from open to close; `undefined` while open. */
  lengthMs: number | undefined;
  /** What ended it: the bot's own `player_loaded`, or the server's 60-tick fallback. */
  closedBy: "player_loaded" | "server-timeout" | undefined;
  /**
   * ms after the open at which the bot sent `player_loaded` for this window —
   * also when that was after the server's fallback had already closed it (a
   * chunk that took longer than 60 ticks to arrive; a real client does the
   * same). `undefined` is a bot that never reported loaded at all.
   */
  reportedAtMs: number | undefined;
  readonly absorbed: AbsorbedEvent[];
}

interface Vec {
  x: number;
  y: number;
  z: number;
}
/** mineflayer's own position object: `blockAt` needs its `floored`, so offsets come from it. */
interface Vec3Like extends Vec {
  offset(dx: number, dy: number, dz: number): Vec3Like;
}

/** The slice of a mineflayer bot the tracer reads. */
export interface TracedBot {
  readonly _client: {
    on(event: string, listener: (packet: Record<string, unknown>) => void): unknown;
    write(name: string, params: object): void;
  };
  on(event: "physicsTick", listener: () => void): unknown;
  readonly entity?: { id?: number; position?: Vec3Like; onGround?: boolean } | undefined;
  readonly entities?: Record<number, { type?: string; name?: string; position?: Vec } | undefined>;
  blockAt(pos: Vec3Like): { name?: string } | null;
}

export interface LoadWindowContext {
  stage(): string;
  step(): number;
  /** Declared lethal volumes, judged by the server's own selector rule ({@link bodyInVolume}). */
  lethal(): readonly Box[];
}

export interface LoadWindowTracer {
  windows(): readonly LoadWindowRecord[];
}

const dist = (a: Vec, b: Vec): number => Math.hypot(a.x - b.x, a.y - b.y, a.z - b.z);


export function traceLoadWindows(
  bot: TracedBot,
  ctx: LoadWindowContext,
  now: () => number = Date.now,
): LoadWindowTracer {
  const windows: LoadWindowRecord[] = [];
  let open: LoadWindowRecord | undefined;
  let peakY: number | undefined;
  let wasOnGround = true;
  let inHazard = false;
  let inLethal = false;

  const absorb = (kind: AbsorbedKind, detail: string): void => {
    if (!open) return;
    open.absorbed.push({ kind, atMs: now() - open.openedAt, detail });
  };
  const close = (by: "player_loaded" | "server-timeout"): void => {
    if (!open) return;
    if (by === "player_loaded") open.reportedAtMs = now() - open.openedAt;
    open.lengthMs = by === "server-timeout" ? SERVER_LOAD_TIMEOUT_TICKS * TICK_MS : now() - open.openedAt;
    open.closedBy = by;
    open = undefined;
  };
  const begin = (cause: "login" | "respawn"): void => {
    close("server-timeout");
    open = {
      cause,
      stage: ctx.stage(),
      step: ctx.step(),
      openedAt: now(),
      lengthMs: undefined,
      closedBy: undefined,
      reportedAtMs: undefined,
      absorbed: [],
    };
    windows.push(open);
    peakY = undefined;
    wasOnGround = true;
    inHazard = false;
    inLethal = false;
  };

  // The observer on the socket: every packet the bot sends passes here.
  const write = bot._client.write.bind(bot._client);
  bot._client.write = (name: string, params: object): void => {
    if (name === "player_loaded") {
      const last = windows[windows.length - 1];
      if (open) close("player_loaded");
      else if (last && last.reportedAtMs === undefined) last.reportedAtMs = now() - last.openedAt;
    }
    write(name, params);
  };

  bot._client.on("login", () => begin("login"));
  bot._client.on("respawn", () => begin("respawn"));
  bot._client.on("damage_event", (p) => {
    if (open && p["entityId"] === bot.entity?.id) absorb("hurt", `damage type ${String(p["sourceTypeId"])}`);
  });
  bot._client.on("animation", (p) => {
    if (!open || p["animation"] !== 0) return;
    const me = bot.entity?.position;
    const e = bot.entities?.[p["entityId"] as number];
    if (!me || !e?.position || e.type === "player") return;
    const d = dist(me, e.position);
    if (d <= MELEE_REACH) absorb("swing", `${e.name ?? "mob"} swung at ${d.toFixed(1)} blocks`);
  });
  bot._client.on("explosion", (p) => {
    const me = bot.entity?.position;
    const c = p["center"] as Vec | undefined;
    const at = c ?? (typeof p["x"] === "number" ? (p as unknown as Vec) : undefined);
    if (open && me && at && dist(me, at) <= BLAST_RADIUS) absorb("blast", `explosion at ${dist(me, at).toFixed(1)} blocks`);
  });
  bot.on("physicsTick", () => {
    if (!open) return;
    if (now() - open.openedAt >= SERVER_LOAD_TIMEOUT_TICKS * TICK_MS) {
      close("server-timeout");
      return;
    }
    const me = bot.entity?.position;
    if (!me) return;
    // Hazard contact: feet and head, counted once per entry.
    const touched = [0, 1]
      .map((dy) => bot.blockAt(me.offset(0, dy, 0))?.name)
      .concat(bot.blockAt(me.offset(0, -1, 0))?.name === "magma_block" ? ["magma_block"] : [])
      .filter((n): n is string => n !== undefined && HAZARD_BLOCKS.has(n));
    if (touched.length > 0 && !inHazard) absorb("hazard", `in ${touched.join("+")}`);
    inHazard = touched.length > 0;
    const lethal = ctx.lethal().some((b) => bodyInVolume([me.x, me.y, me.z], b));
    if (lethal && !inLethal) absorb("lethal", `inside a declared lethal volume at ${me.x.toFixed(1)},${me.y.toFixed(1)},${me.z.toFixed(1)}`);
    inLethal = lethal;
    // Falls: the drop from the highest point since the window opened to a landing.
    const onGround = bot.entity?.onGround ?? true;
    peakY = peakY === undefined ? me.y : Math.max(peakY, me.y);
    if (onGround && !wasOnGround && peakY - me.y > FALL_THRESHOLD) {
      absorb("fall", `landed ${(peakY - me.y).toFixed(1)} blocks down`);
    }
    if (onGround) peakY = me.y;
    wasOnGround = onGround;
  });

  return { windows: () => windows };
}

/** The run-report section: one row per window, and the totals a reader needs first. */
export function loadWindowSummary(windows: readonly LoadWindowRecord[]): Record<string, unknown> {
  const closed = windows.filter((w) => w.closedBy !== undefined);
  return {
    opened: windows.length,
    closed_by_player_loaded: closed.filter((w) => w.closedBy === "player_loaded").length,
    closed_by_server_timeout: closed.filter((w) => w.closedBy === "server-timeout").length,
    invulnerable_ms_total: closed.reduce((s, w) => s + (w.lengthMs ?? 0), 0),
    absorbed_total: windows.reduce((s, w) => s + w.absorbed.length, 0),
    windows: windows.map((w) => ({
      cause: w.cause,
      stage: w.stage,
      step: w.step,
      length_ms: w.lengthMs ?? null,
      closed_by: w.closedBy ?? null,
      reported_loaded_at_ms: w.reportedAtMs ?? null,
      absorbed: w.absorbed.map((a) => ({ kind: a.kind, at_ms: a.atMs, detail: a.detail })),
    })),
  };
}

/**
 * The windows in which the bot never told the server it had loaded, although
 * the vanilla client's own 30-second wait had long run out — a bot built
 * without `createHarnessBot`, or a tracker that never fired. `endedAt` is when
 * the run stopped watching, so a respawn in the run's last seconds is not
 * counted against it.
 */
export function unreportedWindows(
  windows: readonly LoadWindowRecord[],
  endedAt: number,
  clientWaitMs: number,
): LoadWindowRecord[] {
  return windows.filter((w, i) => {
    if (w.reportedAtMs !== undefined) return false;
    const next = windows[i + 1];
    const watchedUntil = next ? next.openedAt : endedAt;
    return watchedUntil - w.openedAt > clientWaitMs;
  });
}
