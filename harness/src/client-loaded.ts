// The one place a harness bot tells the server it has loaded the world.
//
// ## The vanilla rule (pinned 1.21.11, read from the server and client jars)
//
// The server holds a player "not loaded" from the moment its connection is made
// and again from every respawn (`ServerGamePacketListenerImpl
// .restartClientLoadTimerAfterRespawn`: unloaded, timer = 60). While not loaded,
// `ServerPlayer.isInvulnerableTo` answers TRUE for EVERY damage source — the
// clause is not gated on `bypasses_invulnerability`, so `/kill` and `/damage` are
// refused like a mob's hit. The state ends when the client sends
// `player_loaded` (`handleAcceptPlayerLoad` → `markClientLoaded`), or when the
// server's own 60-tick timer, decremented once per `ServerPlayer.tick`, runs out.
//
// The vanilla client sends `player_loaded` from its `LevelLoadTracker`: a fresh
// tracker on every `login` and `respawn` packet; it advances when the server's
// `level_chunks_load_start` game event arrives; it is ready — and the packet goes —
// on the first client tick on which the chunk section under the player is loaded
// (or the player is spectating, dead, or outside the build height), or 30 s after
// the tracker began, whichever is first.
//
// mineflayer 4.37.1 never sends it. This module is that tracker, for a mineflayer
// bot: {@link installClientLoaded} on every bot this harness creates, through
// {@link createHarnessBot}, which is the only `createBot` the harness calls
// (`test/client-loaded.test.ts` refuses any other).

import { createBot, type Bot, type BotOptions } from "mineflayer";
import { installResourcePack, type PackBot, type ResourcePackState } from "./resource-pack.ts";

/** The vanilla client's `LevelLoadTracker.CLIENT_WAIT_TIMEOUT_MS`: 30 seconds. */
export const CLIENT_WAIT_TIMEOUT_MS = 30_000;

/**
 * The view distance every harness bot asks for, in chunks: vanilla's ceiling. The
 * server serves the lesser of this and its own `view-distance`, which is the
 * campaign's declared `world.view_distance` (floor 10), so the bot holds exactly
 * what the delve serves a player at the game's largest render distance — not
 * mineflayer's default `far` (12), under which a campaign declaring more was
 * never seen past 192 blocks. The host meets the cost the build states for the
 * declared distance (`delvec` `served`).
 */
export const CLIENT_VIEW_DISTANCE = 32;

/** The server's own fallback (`clientLoadedTimeoutTimer`), in server ticks. */
export const SERVER_LOAD_TIMEOUT_TICKS = 60;

/** Where the tracker is, named after the vanilla client's states. */
export type LoadPhase = "waiting-for-server" | "waiting-for-player-chunk" | "loaded";

/** One stretch from a `login`/`respawn` to the `player_loaded` that ended it. */
export interface LoadWindow {
  /** What opened it: the join, or the Nth respawn (1-based). */
  readonly cause: "login" | "respawn";
  /** Wall-clock ms the `login`/`respawn` packet arrived. */
  readonly openedAt: number;
  /** Wall-clock ms the packet was sent, or `undefined` while still open. */
  sentAt: number | undefined;
  /** Why it was sent: the player's chunk loaded, or the client's own timeout. */
  reason: "chunk" | "timeout" | "not-in-world" | undefined;
}

/** The narrow slice of a mineflayer bot the tracker reads and writes. */
export interface LoadedBot {
  readonly _client: {
    on(event: string, listener: (packet: Record<string, unknown>) => void): unknown;
    write(name: string, params: object): void;
  };
  on(event: "physicsTick" | "chunkColumnLoad", listener: () => void): unknown;
  readonly entity?: { position?: { x: number; y: number; z: number } } | undefined;
  readonly game?: { gameMode?: string; minY?: number; height?: number } | undefined;
  readonly health?: number;
  /** Takes the entity's own position object (a Vec3), exactly as mineflayer does. */
  blockAt(pos: { x: number; y: number; z: number }): unknown;
}

/** The live state, read by the run report and by the waits that need a hurtable body. */
export interface ClientLoadedState {
  phase(): LoadPhase;
  /** Every window this bot has opened, in order. */
  windows(): readonly LoadWindow[];
  /** How many `player_loaded` packets this bot has sent. */
  sent(): number;
}

/**
 * Make `bot` send `player_loaded` exactly when the vanilla client would. Install
 * it BEFORE the `login` packet can arrive — that is, in the same turn the bot is
 * created — or the first window is never opened and the join stretch is the
 * server's 60-tick timeout again.
 */
export function installClientLoaded(bot: LoadedBot, now: () => number = Date.now): ClientLoadedState {
  let phase: LoadPhase = "loaded";
  let deadline = 0;
  const windows: LoadWindow[] = [];
  let sent = 0;

  const open = (cause: "login" | "respawn"): void => {
    phase = "waiting-for-server";
    deadline = now() + CLIENT_WAIT_TIMEOUT_MS;
    windows.push({ cause, openedAt: now(), sentAt: undefined, reason: undefined });
  };
  const send = (reason: "chunk" | "timeout" | "not-in-world"): void => {
    bot._client.write("player_loaded", {});
    sent += 1;
    phase = "loaded";
    const w = windows[windows.length - 1];
    if (w && w.sentAt === undefined) {
      w.sentAt = now();
      w.reason = reason;
    }
  };
  /** `LevelLoadTracker.WaitingForPlayerChunk.isReady`, one client tick. */
  const tick = (): void => {
    if (phase !== "waiting-for-player-chunk") return;
    if (now() > deadline) {
      send("timeout");
      return;
    }
    const pos = bot.entity?.position;
    if (!pos) return;
    const minY = bot.game?.minY;
    const height = bot.game?.height;
    const outside =
      typeof minY === "number" && typeof height === "number" && (pos.y < minY || pos.y >= minY + height);
    if (outside || bot.game?.gameMode === "spectator" || (bot.health ?? 20) <= 0) {
      send("not-in-world");
      return;
    }
    // mineflayer's `blockAt` floors the entity's own Vec3; `null` is "no column there".
    if (bot.blockAt(pos) != null) {
      send("chunk");
    }
  };

  bot._client.on("login", () => open("login"));
  bot._client.on("respawn", () => open("respawn"));
  bot._client.on("game_state_change", (packet) => {
    if (packet["reason"] === "level_chunks_load_start" && phase === "waiting-for-server") {
      phase = "waiting-for-player-chunk";
    }
  });
  bot.on("physicsTick", tick);
  bot.on("chunkColumnLoad", tick);

  return {
    phase: () => phase,
    windows: () => windows,
    sent: () => sent,
  };
}

/** A bot this harness drives, with the tracker that keeps it hurtable. */
export interface HarnessBot {
  readonly bot: Bot;
  readonly loaded: ClientLoadedState;
  /** Every resource pack the server pushed, and what the bot made of it. */
  readonly pack: ResourcePackState;
}

/**
 * The harness's only `createBot`. Every bot it returns reports `player_loaded`
 * the way a real client does; a bot made any other way is invulnerable for the
 * server's 60-tick fallback after its join and after every respawn.
 */
export function createHarnessBot(options: BotOptions): HarnessBot {
  const bot = createBot({ viewDistance: CLIENT_VIEW_DISTANCE, ...options });
  const loaded = installClientLoaded(bot as unknown as LoadedBot);
  // A served pack (spec-0084 §11) is pushed in configuration and the server
  // holds the player there until the client answers, so every bot answers.
  const pack = installResourcePack(bot as unknown as PackBot);
  return { bot, loaded, pack };
}
