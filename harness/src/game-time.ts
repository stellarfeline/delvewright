// The server's game time, read where the harness needs a span of game ticks.
//
// A span the game counts in ticks — a sculk sensor's busy phases, a shrieker's
// `shrieking`, a pulse's beat interval, the tick in which a watch line runs — is
// a span of the SERVER's ticks. Wall time stands for it only while the server
// keeps 20 ticks a second; a server that falls behind stretches every such span,
// and a wall-clock window then closes before the game's span has. So the harness
// asks the server for its game time, in the form `placement.ts` reads it: the
// time is stored, then printed, inside one reply bracket (`command-reply.ts`),
// and every line of the reply is judged by the shared rejection rule
// (`rejection.ts`).
//
// Packets on the connection arrive in the order the server sent them. A reading
// that answers `t` was answered in tick `t`: everything the bot received before
// the reply was sent at tick `t` or earlier, and everything after it at `t` or
// later. That ordering, not wall time, places an event against a reading.
//
// This file is pure. The reading itself is `executor/clock.ts`.

/** Where a reading parks the game time it was taken at (harness-owned storage). */
export const GAME_TIME_STORAGE = "dw_harness:clock";

/** The prefix of the line a reading makes the server speak. */
function token(serial: number): string {
  return `[dw:clock ${serial} `;
}

/**
 * The commands of reading `serial`: store the game time, then speak it. Any
 * further commands a caller appends run in the same tick, after these.
 */
export function gameTimeCommands(serial: number): string[] {
  return [
    `/execute store result storage ${GAME_TIME_STORAGE} t int 1 run time query gametime`,
    `/tellraw @s ${JSON.stringify([token(serial), { storage: GAME_TIME_STORAGE, nbt: "t" }, "]"])}`,
  ];
}

/** The game time reading `serial` answered, or `undefined` when its line is absent. */
export function parseGameTime(serial: number, lines: readonly string[]): number | undefined {
  const t = token(serial);
  for (const line of lines) {
    const at = line.indexOf(t);
    if (at < 0) continue;
    const m = /^(-?\d+)\]/.exec(line.slice(at + t.length));
    if (m) return Number(m[1]);
  }
  return undefined;
}

/**
 * The longest gap (ms) between two answered readings for the second to count as
 * the server seen answering: the bot pauses 50ms between readings, and a reply
 * bracket on a local server comes back in a few. A longer gap (the host stalled
 * the server, the bot, or both) says nothing about whether the game clock runs.
 */
export const LIVE_REPLY_GAP_MS = 500;

/**
 * How long (ms) of answers seen at one game tick make the game clock stopped.
 * A live game ticks at least once a second (vanilla refuses a `/tick rate`
 * below 1), so answers that keep coming within {@link LIVE_REPLY_GAP_MS} of each
 * other sit on one tick for about a second at most. Ten seconds of them is a
 * game that is not ticking: frozen, or stepped by hand.
 */
export const STOPPED_CLOCK_MS = 10_000;

/**
 * **The judge of a game-tick wait: has the server's game clock stopped?**
 *
 * Fed every answered reading with the wall time its reply was taken in, it sums,
 * since the last reading that showed another tick, only the gaps of at most
 * {@link LIVE_REPLY_GAP_MS} between consecutive replies: the stretches in which
 * the server was seen answering. When the sum reaches {@link STOPPED_CLOCK_MS}
 * at one tick, it throws {@link GameTimeStoppedError}. A host stall, of the
 * server or of the bot, is one long gap and adds nothing; a slow server answers
 * late (a long gap) or at a later tick (the sum restarts). Wall time alone never
 * fails a wait.
 */
export class GameClockWatch {
  private readonly purpose: string;
  private readonly from: number;
  private readonly ticks: number;
  private tick: number | undefined;
  private lastAt = 0;
  private liveMs = 0;
  private liveReplies = 0;

  /** A wait, for `purpose`, until the game time is `ticks` past `from`. */
  constructor(purpose: string, from: number, ticks: number) {
    this.purpose = purpose;
    this.from = from;
    this.ticks = ticks;
  }

  /** One answered reading: the game tick it showed, and the wall time (ms) its reply was taken in. */
  observe(tick: number, answeredAt: number): void {
    if (tick !== this.tick) {
      this.tick = tick;
      this.liveMs = 0;
      this.liveReplies = 0;
    } else {
      const gap = answeredAt - this.lastAt;
      if (gap >= 0 && gap <= LIVE_REPLY_GAP_MS) {
        this.liveMs += gap;
        this.liveReplies += 1;
      }
    }
    this.lastAt = answeredAt;
    if (this.liveMs >= STOPPED_CLOCK_MS) {
      throw new GameTimeStoppedError(this.purpose, this.ticks, tick - this.from, tick, this.liveReplies, this.liveMs);
    }
  }

  /** The answers seen at the current tick so far: how many, and over how many ms. */
  get seen(): { readonly replies: number; readonly ms: number } {
    return { replies: this.liveReplies, ms: this.liveMs };
  }
}

/** The named failure: the server refused a game-time reading, or never answered it. */
export class GameTimeUnreadError extends Error {
  readonly purpose: string;
  constructor(purpose: string, why: string) {
    super(`game time: ${purpose}: ${why}, so the game ticks this judgement counts cannot be read`);
    this.name = "GameTimeUnreadError";
    this.purpose = purpose;
  }
}

/**
 * The named failure: the server kept answering, and its game clock did not move
 * ({@link GameClockWatch}).
 */
export class GameTimeStoppedError extends Error {
  constructor(purpose: string, ticks: number, advanced: number, tick: number, replies: number, liveMs: number) {
    super(
      `game time: ${purpose}: the game clock stopped at tick ${tick}, ${advanced} of ${ticks} game ` +
        `tick(s) in: the server answered ${replies} reading(s) over ${liveMs}ms, each within ` +
        `${LIVE_REPLY_GAP_MS}ms of the one before, and none showed a later tick (stopped is ` +
        `${STOPPED_CLOCK_MS}ms of such answers at one tick)`,
    );
    this.name = "GameTimeStoppedError";
  }
}
