// The delve has finished placing its world: the signal, the question, the verdict.
//
// A delve builds its world AFTER the server reports ready. The datapack's
// `setup` marks `#placed dw.sys` 0; the tick re-places every template until each
// sentinel verifies (`place_all` / `place_verify`); the world build latches it
// at 2 while its block writes run, one step a tick; and `setup_finish` latches it
// at 1 as its last line (`crates/delvec/src/compiler/emit/functions.rs`). Only
// then does the tick's first-join placement run: `execute if score #placed
// dw.sys matches 1 as @a[tag=!dw_joined] run function <ns>:join_place`, which
// teleports a player to the campaign's entry point and tags it `dw_joined`.
//
// A bot that starts walking before that walks a world that is not there yet,
// from wherever the server dropped it at login, and the first-join teleport then
// lands under it mid-leg. Both were measured on one build of a large map, three
// times in all: `No path to the goal!` with the bot at the world origin, and
// `The goal was changed before it could be completed!` when the teleport arrived
// during the leg; other runs of the same build passed. So no leg starts until the
// server itself says the world is placed and the first join is behind the bot.
//
// The question is put to the server in the form it adjudicates: `execute if
// score …` / `execute unless score …` each speaking a `tellraw`, which reaches
// the bot whatever `send_command_feedback` says. The game time is stored first
// and printed in the same line, so every reading carries the tick it was taken
// at. That tick is what decides the first join: `join_place` runs from the tick
// function, which a tick runs before it advances the game time, so a reading
// taken at a game time past the one placement was first read at comes after the
// tick in which `join_place` had its chance. A bot still untagged then belongs to
// a build that places no first join (one with no `spawn` anchor). Nothing here
// waits on wall time except the budget, and the budget running out is a named
// failure, never a pass.
//
// This file is pure. The polling loop lives in `executor/placement.ts`.

/** The score holder the datapack latches when placement is done. */
export const PLACED_HOLDER = "#placed";
/** The objective that holder lives in. */
export const PLACED_OBJECTIVE = "dw.sys";
/** The value `setup_finish` latches as its last line: the world is placed. */
export const PLACED_DONE = 1;
/** The tag `join_place` gives a player it has placed. */
export const JOINED_TAG = "dw_joined";
/** Where the reading parks the game time it was taken at (harness-owned storage). */
export const PLACEMENT_STORAGE = "dw_harness:placement";

/**
 * The default budget (ms) from the bot's spawn to a placed world.
 *
 * Overridable with `DELVEWRIGHT_PLACEMENT_TIMEOUT_MS`. Running out of it fails
 * the run; it is never a reason to proceed.
 */
export const PLACEMENT_TIMEOUT_MS = 300_000;

/** How long (ms) the loop pauses between one answered reading and the next. */
export const PLACEMENT_POLL_MS = 500;

/** The budget a run uses: the environment's, or {@link PLACEMENT_TIMEOUT_MS}. */
export function placementTimeoutFromEnv(env: Record<string, string | undefined>): number {
  const raw = env["DELVEWRIGHT_PLACEMENT_TIMEOUT_MS"];
  if (raw === undefined || raw === "") return PLACEMENT_TIMEOUT_MS;
  const parsed = Number.parseInt(raw, 10);
  if (!Number.isInteger(parsed) || parsed <= 0 || String(parsed) !== raw.trim()) {
    throw new Error(
      `DELVEWRIGHT_PLACEMENT_TIMEOUT_MS must be a positive whole number of milliseconds, ` +
        `got ${JSON.stringify(raw)}`,
    );
  }
  return parsed;
}

/** The lines one reading's commands make the server speak. */
function tokens(serial: number): { done: string; pending: string; joined: string } {
  return {
    done: `[dw:placed ${serial} done `,
    pending: `[dw:placed ${serial} pending `,
    joined: `[dw:placed ${serial} joined]`,
  };
}

/**
 * The commands of reading `serial`, in the order they must run: store the game
 * time, then speak whether the world is placed (with that time), then whether
 * the first-join placement has tagged the bot. Sent inside one reply bracket
 * (`command-reply.ts`), so the lines between its markers are this reading's.
 */
export function placementCommands(serial: number): string[] {
  const t = tokens(serial);
  const at = { storage: PLACEMENT_STORAGE, nbt: "t" };
  const score = `score ${PLACED_HOLDER} ${PLACED_OBJECTIVE} matches ${PLACED_DONE}`;
  return [
    `/execute store result storage ${PLACEMENT_STORAGE} t int 1 run time query gametime`,
    `/execute if ${score} run tellraw @s ${JSON.stringify([t.done, at, "]"])}`,
    `/execute unless ${score} run tellraw @s ${JSON.stringify([t.pending, at, "]"])}`,
    `/execute if entity @s[tag=${JOINED_TAG}] run tellraw @s ${JSON.stringify({ text: t.joined })}`,
  ];
}

/** What one reading established. */
export interface PlacementReading {
  /** True placed, false not yet, undefined when the server said both or neither. */
  readonly placed: boolean | undefined;
  /** The game time the reading was taken at, when the server printed one. */
  readonly tick: number | undefined;
  /** Whether the first-join placement has tagged the bot. */
  readonly joined: boolean;
}

/** Read reading `serial`'s answer out of the lines between its bracket's markers. */
export function parsePlacementReply(serial: number, lines: readonly string[]): PlacementReading {
  const t = tokens(serial);
  let done: number | undefined | null = null;
  let pending: number | undefined | null = null;
  const tickAfter = (line: string, prefix: string): number | undefined => {
    const rest = line.slice(line.indexOf(prefix) + prefix.length);
    const m = /^(-?\d+)\]/.exec(rest);
    return m ? Number(m[1]) : undefined;
  };
  let joined = false;
  for (const line of lines) {
    if (line.includes(t.done)) done = tickAfter(line, t.done);
    else if (line.includes(t.pending)) pending = tickAfter(line, t.pending);
    else if (line.includes(t.joined)) joined = true;
  }
  const saidDone = done !== null;
  const saidPending = pending !== null;
  const placed = saidDone === saidPending ? undefined : saidDone;
  const tick = placed === true ? (done ?? undefined) : placed === false ? (pending ?? undefined) : undefined;
  return { placed, tick, joined };
}

/** How the first join was settled, once the world is placed. */
export type FirstJoin =
  | { readonly kind: "joined"; readonly tick: number | undefined }
  | { readonly kind: "none"; readonly tick: number };

/** The verdict the loop acts on after each reading. */
export type PlacementVerdict =
  | { readonly ready: false }
  | { readonly ready: true; readonly placedTick: number | undefined; readonly firstJoin: FirstJoin };

/**
 * Folds readings into a verdict. Ready when the world reads placed AND either
 * the bot wears {@link JOINED_TAG}, or a reading taken at a later game time than
 * the one placement was first read at still finds it untagged — the tick in
 * which `join_place` would have run is then behind the bot, and the build places
 * no first join.
 */
export class PlacementLatch {
  /** The game time placement was first read at. */
  placedTick: number | undefined;
  /** Whether placement has been read as done at all. */
  placedSeen = false;

  observe(reading: PlacementReading): PlacementVerdict {
    if (reading.placed !== true) return { ready: false };
    if (!this.placedSeen) {
      this.placedSeen = true;
      this.placedTick = reading.tick;
    }
    if (reading.joined) {
      return { ready: true, placedTick: this.placedTick, firstJoin: { kind: "joined", tick: reading.tick } };
    }
    if (this.placedTick !== undefined && reading.tick !== undefined && reading.tick > this.placedTick) {
      return { ready: true, placedTick: this.placedTick, firstJoin: { kind: "none", tick: reading.tick } };
    }
    return { ready: false };
  }
}

/** What a placed world's wait measured, for the run's log. */
export interface PlacementRecord {
  readonly placedTick: number | undefined;
  readonly firstJoin: FirstJoin;
  /** Wall time (ms) from the bot's spawn to the verdict. */
  readonly waitedMs: number;
  /** Readings taken, answered or not. */
  readonly readings: number;
  /** Readings whose closing marker never arrived inside the bracket timeout. */
  readonly unanswered: number;
}

/** The run's log line for a placed world. */
export function placementLine(r: PlacementRecord): string {
  const join =
    r.firstJoin.kind === "joined"
      ? `the build's first-join placement had tagged the bot ${JOINED_TAG}` +
        (r.firstJoin.tick !== undefined ? ` (read at game tick ${r.firstJoin.tick})` : "")
      : `the build places no first join (the bot was still untagged at game tick ${r.firstJoin.tick}, ` +
        `after the tick in which ${JOINED_TAG} would have been given)`;
  return (
    `[placement] the world is placed (${PLACED_HOLDER} ${PLACED_OBJECTIVE} = ${PLACED_DONE}` +
    (r.placedTick !== undefined ? `, first read at game tick ${r.placedTick}` : "") +
    `) ${r.waitedMs}ms after the bot spawned, over ${r.readings} reading(s) ` +
    `(${r.unanswered} unanswered); ${join}. No leg started before this line.\n`
  );
}

/** The named failure: the world was not placed inside the budget. */
export class PlacementTimeoutError extends Error {
  readonly budgetMs: number;
  readonly readings: number;
  readonly unanswered: number;
  readonly last: PlacementReading | undefined;
  constructor(budgetMs: number, readings: number, unanswered: number, last: PlacementReading | undefined) {
    const lastSaid =
      last === undefined
        ? "no reading was answered"
        : last.placed === true
          ? `the last reading found the world placed${last.tick !== undefined ? ` at game tick ${last.tick}` : ""} ` +
            `but the bot not yet tagged ${JOINED_TAG}`
          : last.placed === false
            ? `the last reading found ${PLACED_HOLDER} ${PLACED_OBJECTIVE} not yet ${PLACED_DONE}` +
              (last.tick !== undefined ? ` at game tick ${last.tick}` : "")
            : "the last reading answered neither placed nor pending";
    super(
      `placement: the delve had not finished placing its world ${budgetMs}ms after the bot ` +
        `spawned (${readings} reading(s), ${unanswered} unanswered; ${lastSaid}). No leg was ` +
        `started. The budget is DELVEWRIGHT_PLACEMENT_TIMEOUT_MS`,
    );
    this.name = "PlacementTimeoutError";
    this.budgetMs = budgetMs;
    this.readings = readings;
    this.unanswered = unanswered;
    this.last = last;
  }
}

/** The named failure: the server refused the reading itself. */
export class PlacementRefusedError extends Error {
  readonly refusal: string;
  constructor(refusal: string) {
    super(
      `placement: the server refused the placement reading (${JSON.stringify(refusal)}), so ` +
        `whether the world is placed cannot be read. No leg was started`,
    );
    this.name = "PlacementRefusedError";
    this.refusal = refusal;
  }
}
