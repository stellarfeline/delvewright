// MineflayerExecutor: a body watches (spec-0101 §5.4) — the per-waypoint check.

import type { Vec3Tuple } from "../critical-path.ts";
import { echoCommand, type BracketedReply } from "../command-reply.ts";
import {
  WATCH_MIN_HORIZONTAL,
  WATCH_REACH_MARGIN,
  WATCH_YAW_TOLERANCE,
  WatchLedger,
  bearing,
  classMarker,
  facingMarker,
  feetDistance,
  horizontalDistance,
  mayReach,
  parsePosReply,
  parseYawReply,
  yRotationRange,
  type Watcher,
} from "../watch.ts";
import { delay } from "./connection.ts";
import type { MineflayerExecutor } from "../executor.ts";

/** One server tick, in ms. */
const TICK_MS = 50;

/** How long the bot may take to come to rest before a body is judged. */
const SETTLE_MS = 600;

/** How long a bracketed read may take before it is reported unobserved. */
const BRACKET_TIMEOUT_MS = 5_000;

/** The marker a body's live position is echoed under. */
function posMarker(id: string): string {
  return `[dw:watch-pos ${id} `;
}

/** The marker a body's live yaw is echoed under. */
function yawMarker(id: string): string {
  return `[dw:watch-yaw ${id} `;
}

/** The body's selector while its watch is live. */
function liveSelector(w: Watcher): string {
  return `@e[${w.selector},tag=dw_watch,tag=!dw_unseen,limit=1]`;
}

/** The line in `reply` carrying `marker`, if any. */
function lineWith(reply: BracketedReply, marker: string): string | undefined {
  return reply.lines.find((l) => l.includes(marker));
}

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  /** Hand over the record's watchers; the ledger states its binding from them. */
  useWatchers(this: MineflayerExecutor, watchers: readonly Watcher[]): void {
    this.watch = new WatchLedger(watchers);
  },

  /** The run's watch ledger, empty when the record carries none. */
  watchLedger(this: MineflayerExecutor): WatchLedger {
    return this.watch ?? new WatchLedger([]);
  },

  /**
   * Send every command bracketed, back to back in one synchronous turn, and
   * return each one's reply in order. See `command-reply.ts`.
   */
  async bracketedReplies(this: MineflayerExecutor, commands: readonly string[]): Promise<BracketedReply[]> {
    const bot = this.requireBot();
    const brackets = commands.map((command) => {
      const b = this.brackets.begin();
      bot.chat(echoCommand(b.open));
      bot.chat(command);
      bot.chat(echoCommand(b.close));
      return b;
    });
    return Promise.all(brackets.map((b) => this.brackets.reply(b, BRACKET_TIMEOUT_MS)));
  },

  /**
   * **At a waypoint, every watching body in reach faces the bot** (spec-0101
   * §5.4). Asks nothing unless some body could be in reach from here
   * ({@link mayReach} over where it can stand). Otherwise the bot stops, lets
   * the game run its watch line, and reads, in one bracketed turn, its own
   * position and each body's live position (a body walking, removed or gone
   * reads empty, and is not judged), and whether it wears each class a class
   * watch names. For each body within reach that it can draw, it asks whether
   * the body's yaw is within ±2° of the game's bearing from the body's feet to
   * the bot's. A denial is recorded as a failure of the critical-path stage —
   * never thrown into the walk, which would read it as a stalled hop.
   */
  async judgeWatchers(this: MineflayerExecutor, label: string): Promise<void> {
    const ledger = this.watch;
    if (!ledger || !ledger.armed || ledger.watchers.length === 0) return;
    const bot = this.requireBot();
    if (bot.game?.gameMode === "spectator") return;
    const p0 = bot.entity.position;
    const near = ledger.watchers.filter((w) => mayReach(w, [p0.x, p0.y, p0.z]));
    if (near.length === 0) return;
    bot.clearControlStates();
    const settleUntil = Date.now() + SETTLE_MS;
    while (
      Date.now() < settleUntil &&
      Math.hypot(bot.entity.velocity.x, bot.entity.velocity.z) > 0.005
    ) {
      await delay(TICK_MS);
    }
    // At least one tick: the server runs the watch line with the position it
    // now holds for the bot.
    await delay(2 * TICK_MS);
    const ownMarker = "[dw:watch-pos @s ";
    const reads: string[] = [
      `/tellraw @s ${JSON.stringify([{ text: ownMarker }, { nbt: "Pos", entity: "@s" }, { text: "]" }])}`,
    ];
    for (const w of near) {
      reads.push(
        `/tellraw @s ${JSON.stringify([
          { text: posMarker(w.id) },
          { nbt: "Pos", entity: liveSelector(w) },
          { text: "]" },
        ])}`,
      );
      if (w.classTag !== undefined) {
        reads.push(
          `/execute if entity @s[tag=${w.classTag}] run tellraw @s ${JSON.stringify({ text: classMarker(w.classTag) })}`,
        );
      }
    }
    const replies = await this.bracketedReplies(reads);
    if (replies.some((r) => !r.answered)) {
      ledger.fail(`${label}: a watch read was never answered — nothing about the watchers here was observed`);
      return;
    }
    const all = replies.flatMap((r) => r.lines);
    const ownLine = all.find((l) => l.includes(ownMarker));
    const me: Vec3Tuple | undefined = ownLine ? parsePosReply(ownLine) : undefined;
    if (!me) {
      ledger.fail(`${label}: the server did not state the bot's own position`);
      return;
    }
    for (const w of near) {
      const line = all.find((l) => l.includes(posMarker(w.id)));
      const body = line ? parsePosReply(line) : undefined;
      if (!body) continue; // walking, removed, or not yet summoned: no live watch
      if (w.classTag !== undefined && !all.some((l) => l.includes(classMarker(w.classTag!)))) {
        continue; // the bot does not wear the class this body watches
      }
      if (feetDistance(body, me) > w.within - WATCH_REACH_MARGIN) continue;
      if (horizontalDistance(body, me) < WATCH_MIN_HORIZONTAL) continue;
      ledger.reach(w.id);
      const yaw = bearing(body, me);
      const range = yRotationRange(yaw, WATCH_YAW_TOLERANCE);
      const [facing, live, actual] = await this.bracketedReplies([
        `/execute if entity @e[${w.selector},tag=dw_watch,y_rotation=${range}] run tellraw @s ${JSON.stringify({ text: facingMarker(w.id) })}`,
        `/tellraw @s ${JSON.stringify([{ text: posMarker(w.id) }, { nbt: "Pos", entity: liveSelector(w) }, { text: "]" }])}`,
        `/tellraw @s ${JSON.stringify([{ text: yawMarker(w.id) }, { nbt: "Rotation[0]", entity: `@e[${w.selector},limit=1]` }, { text: "]" }])}`,
      ]);
      if (facing && lineWith(facing, facingMarker(w.id))) {
        ledger.pass(w.id);
        continue;
      }
      const stillLive = live ? parsePosReply(lineWith(live, posMarker(w.id)) ?? "") : undefined;
      if (!stillLive) continue; // a walk took the body between the two reads
      const yawLine = actual ? lineWith(actual, yawMarker(w.id)) : undefined;
      const stated = yawLine ? parseYawReply(yawLine.slice(yawLine.indexOf(yawMarker(w.id)) + yawMarker(w.id).length)) : undefined;
      ledger.fail(
        `${label}: \`${w.id}\` (${w.who}, within ${w.within}) at ` +
          `[${body.map((c) => c.toFixed(2)).join(", ")}] does not face the bot at ` +
          `[${me.map((c) => c.toFixed(2)).join(", ")}], ${feetDistance(body, me).toFixed(2)} block(s) ` +
          `away: the game's bearing is ${yaw.toFixed(2)}° (asked ${range}), the body's yaw is ` +
          `${stated === undefined ? "unread" : `${stated.toFixed(2)}°`}`,
      );
    }
  },
};
