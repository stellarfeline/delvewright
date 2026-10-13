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
 * The wall-time budget (ms) for the server to advance `ticks` game ticks: the
 * span at one tick a second, plus ten seconds. Running out of it is a named
 * failure ({@link GameTimeStalledError}), never a pass.
 */
export function gameTicksBudgetMs(ticks: number): number {
  return Math.max(0, ticks) * 1_000 + 10_000;
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

/** The named failure: the server's game time did not advance the span inside the budget. */
export class GameTimeStalledError extends Error {
  constructor(purpose: string, ticks: number, advanced: number, budgetMs: number) {
    super(
      `game time: ${purpose}: the server advanced ${advanced} of ${ticks} game tick(s) in ` +
        `${budgetMs}ms (the budget is the span at one tick a second, plus 10s)`,
    );
    this.name = "GameTimeStalledError";
  }
}
