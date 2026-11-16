// MineflayerExecutor: a body watches (spec-0101 §5.4) — the per-waypoint check.

import type { Vec3Tuple } from "../critical-path.ts";
import { echoCommand, type BracketedReply } from "../command-reply.ts";
import {
  WATCH_REACH_MARGIN,
  WATCH_YAW_TOLERANCE,
  WatchLedger,
  PLAYER_MARKER,
  bearing,
  facingMarker,
  feetDistance,
  mayReach,
  parsePlayerReply,
  parsePosReply,
  parseYawReply,
  watchTargets,
  yRotationRange,
  type PresentPlayer,
  type Watcher,
} from "../watch.ts";
import { delay } from "./connection.ts";
import type { MineflayerExecutor } from "../executor.ts";

/** How often (ms) the settle loop looks at the bot's own velocity. */
const SETTLE_POLL_MS = 50;

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
   * **At a waypoint, every watching body in reach faces the nearest present
   * player it can draw** (spec-0101 §5.4; spec-0141: the party is every member
   * present, so in a party run that may be a member and not the bot). Asks
   * nothing unless some body could be in reach from here ({@link mayReach}
   * over where it can stand). Otherwise the bot stops, lets the game run its
   * watch line, and reads, in one bracketed turn, every player in the world —
   * name, tags and feet — and each body's live position (a body walking,
   * removed or gone reads empty, and is not judged). A body is judged where the
   * bot stands within its reach; whom it should face is {@link watchTargets}
   * over that roster, and it passes when its yaw is within ±2° of the game's
   * bearing from its feet to one of them. A denial is recorded as a failure of
   * the critical-path stage — never thrown into the walk, which would read it
   * as a stalled hop.
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
      await delay(SETTLE_POLL_MS);
    }
    // At least one tick of the server's: it runs the watch line with the
    // position it now holds for the bot. Counted in its game time, which a
    // server that falls behind does not keep in step with wall time.
    await this.waitForGameTicks(2, `${label}: the watch line's tick`);
    const reads: string[] = [
      `/execute as @a run tellraw ${bot.username} ${JSON.stringify([
        { text: PLAYER_MARKER },
        { selector: "@s" },
        { text: " " },
        { nbt: "Tags", entity: "@s" },
        { text: " " },
        { nbt: "Pos", entity: "@s" },
        { text: "]" },
      ])}`,
    ];
    for (const w of near) {
      reads.push(
        `/tellraw @s ${JSON.stringify([
          { text: posMarker(w.id) },
          { nbt: "Pos", entity: liveSelector(w) },
          { text: "]" },
        ])}`,
      );
    }
    const replies = await this.bracketedReplies(reads);
    if (replies.some((r) => !r.answered)) {
      ledger.fail(`${label}: a watch read was never answered — nothing about the watchers here was observed`);
      return;
    }
    const all = replies.flatMap((r) => r.lines);
    const roster: PresentPlayer[] = replies[0]!.lines
      .map((l) => parsePlayerReply(l))
      .filter((p): p is PresentPlayer => p !== undefined);
    const me: Vec3Tuple | undefined = roster.find((p) => p.name === bot.username)?.feet;
    if (!me) {
      ledger.fail(
        `${label}: the server did not state the bot's own position among the players present ` +
          `(its answer: ${JSON.stringify(replies[0]!.lines)})`,
      );
      return;
    }
    const judged: { w: Watcher; body: Vec3Tuple; targets: readonly PresentPlayer[] }[] = [];
    for (const w of near) {
      const line = all.find((l) => l.includes(posMarker(w.id)));
      const body = line ? parsePosReply(line) : undefined;
      if (!body) continue; // walking, removed, or not yet summoned: no live watch
      if (feetDistance(body, me) > w.within - WATCH_REACH_MARGIN) continue; // the bot is not in reach
      const t = watchTargets(w, body, roster);
      if (!t.judged) continue;
      judged.push({ w, body, targets: t.targets });
    }
    if (judged.length === 0) return;
    // A body is live (`dw_watch`) from the tick a walk's arrival, a summon or a
    // respawn restores the tag, and the watch line turns it in that tick or the
    // next: the read above can fall between the two. Its yaw is judged only
    // after the server has run the line with the tag live at the read's start —
    // a whole game tick later at the least — and only for a body that has not
    // moved since, so a walk that took it in between is not judged.
    await this.waitForGameTicks(2, `${label}: the watch line's tick after the bodies were seen live`);
    for (const { w, body, targets } of judged) {
      ledger.reach(w.id);
      const asked = targets.map((p) => {
        const yaw = bearing(body, p.feet);
        return { p, yaw, range: yRotationRange(yaw, WATCH_YAW_TOLERANCE) };
      });
      const replies = await this.bracketedReplies([
        ...asked.map(
          (a) =>
            `/execute if entity @e[${w.selector},tag=dw_watch,y_rotation=${a.range}] run tellraw @s ${JSON.stringify({ text: facingMarker(w.id) })}`,
        ),
        `/tellraw @s ${JSON.stringify([{ text: posMarker(w.id) }, { nbt: "Pos", entity: liveSelector(w) }, { text: "]" }])}`,
        `/tellraw @s ${JSON.stringify([{ text: yawMarker(w.id) }, { nbt: "Rotation[0]", entity: `@e[${w.selector},limit=1]` }, { text: "]" }])}`,
      ]);
      const facing = replies.slice(0, asked.length);
      const [live, actual] = replies.slice(asked.length);
      if (facing.some((r) => lineWith(r, facingMarker(w.id)))) {
        ledger.pass(w.id);
        continue;
      }
      const stillLive = live ? parsePosReply(lineWith(live, posMarker(w.id)) ?? "") : undefined;
      // A walk took the body between the reads: it was not standing, so it was not judged.
      if (!stillLive || feetDistance(stillLive, body) > 0.001) continue;
      const yawLine = actual ? lineWith(actual, yawMarker(w.id)) : undefined;
      const stated = yawLine ? parseYawReply(yawLine.slice(yawLine.indexOf(yawMarker(w.id)) + yawMarker(w.id).length)) : undefined;
      const whom = asked
        .map(
          (a) =>
            `${a.p.name === bot.username ? "the bot" : `\`${a.p.name}\``} at ` +
            `[${a.p.feet.map((c) => c.toFixed(2)).join(", ")}], ${feetDistance(body, a.p.feet).toFixed(2)} ` +
            `block(s) away: the game's bearing is ${a.yaw.toFixed(2)}° (asked ${a.range})`,
        )
        .join("; or ");
      const others = roster.length - 1;
      ledger.fail(
        `${label}: \`${w.id}\` (${w.who}, within ${w.within}) at ` +
          `[${body.map((c) => c.toFixed(2)).join(", ")}] does not face the nearest player it can ` +
          `draw — ${whom} — the body's yaw is ` +
          `${stated === undefined ? "unread" : `${stated.toFixed(2)}°`} ` +
          `(the bot at [${me.map((c) => c.toFixed(2)).join(", ")}], ${others} other player(s) present)`,
      );
    }
  },
};
