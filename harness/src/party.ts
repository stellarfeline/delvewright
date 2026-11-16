// The PARTY run (spec-0141 §6, "a run with several bots"): the critical-path
// bot walks the delve while three more members stand in for the friends it is
// built for — one who idles at the entry and never moves, one who joins late,
// one who drops out mid-path and comes back.
//
// What it asserts is the multiplayer semantics table
// (`docs/reference/multiplayer-semantics.md`), read on the server:
//
// * **Who is moved.** A member is where their own body is. The idler, at every
//   step boundary, stands where it stood — back in adventure, out of every
//   cutscene's observation state — unless it died, when it stands on the seat
//   in force. The member who left comes back where it left.
// * **Who is given.** A stack for the party is a grant: every member's issue
//   count (`dw.gr`) reaches the party's (`#all`), the late joiner and the one
//   who was away included.
// * **Who arrives where.** A member who takes a class after a checkpoint is
//   armed arrives on that checkpoint's seat.
// * **Who is told.** A telling is for the members present: every completion
//   marker the walking bot heard while a member was online reached that
//   member, and a member never hears one it was away for (a beat is not
//   replayed).
//
// Pure: the schedule, the reading's wire format, and the verdicts. The live
// side is `party-live.ts`. No campaign knowledge: everything comes from the
// critical path and the build's death plan.

import type { Seat } from "./death-loop.ts";
import type { Step, Vec3Tuple } from "./critical-path.ts";

/** The environment switch that turns a run into a party run. */
export const PARTY_ENV = "DELVEWRIGHT_PARTY";

/** Whether this run walks with a party. */
export function partyFromEnv(env: NodeJS.ProcessEnv = process.env): boolean {
  const raw = env[PARTY_ENV];
  return raw === "1" || raw === "true";
}

/** The three members beside the walking bot. Player names: `[A-Za-z0-9_]`, ≤ 16. */
export const IDLER = "party_idle";
export const LATE = "party_late";
export const AWAY = "party_away";
export const MEMBERS = [IDLER, LATE, AWAY] as const;

/** Where a body may stand off its expected cell: vanilla's seat offset, a push. */
export const STAND_TOLERANCE = 1.5;

/** When each member's act falls: the step boundary (after step `i`) it happens at. */
export interface PartySchedule {
  /** The boundary the late member joins at. */
  readonly lateJoinAfter: number;
  /** The boundary the away member disconnects at. */
  readonly dropAfter: number;
  /** The boundary it reconnects at. */
  readonly rejoinAfter: number;
  /** The class command the walking bot takes, which every member takes too; absent with no class. */
  readonly classCommand: string | undefined;
  /** Boundaries the run has: `0 .. completionDue - 1`. */
  readonly boundaries: number;
}

/**
 * Lay the members' acts on the path's step boundaries.
 *
 * The late member joins once the first checkpoint seat is in force, and no
 * earlier than halfway — so it is owed what the party was handed before it
 * came, and has a seat to arrive on. The away member leaves a quarter of the
 * way in and returns three quarters in, so beats happen while it is gone.
 * `undefined` when the path has too few boundaries to separate the acts: a
 * party run over it would bind nothing, and the caller says so.
 */
export function scheduleParty(
  steps: readonly Step[],
  completionDue: number,
  seats: readonly Seat[],
): PartySchedule | undefined {
  const boundaries = completionDue;
  if (boundaries < 3) return undefined;
  const last = boundaries - 1;
  const armed = seats
    .filter((s) => s.cp >= 0 && s.inForceAfter !== null && s.inForceAfter <= last)
    .map((s) => s.inForceAfter as number);
  const firstArmed = armed.length > 0 ? Math.min(...armed) : undefined;
  const lateJoinAfter = Math.min(last, Math.max(firstArmed ?? 0, Math.floor(boundaries / 2)));
  const dropAfter = Math.floor(boundaries / 4);
  const rejoinAfter = Math.max(dropAfter + 1, Math.min(last, Math.floor((boundaries * 3) / 4)));
  const select = steps.find((s) => s.action === "select-class");
  return {
    lateJoinAfter,
    dropAfter,
    rejoinAfter,
    classCommand: select?.action === "select-class" ? select.command : undefined,
    boundaries,
  };
}

// ---------------------------------------------------------------------------
// The reading: one line per member, asked of the server by the opped walker.
// ---------------------------------------------------------------------------

/** What the server says of one member at one moment. */
export interface MemberReading {
  readonly name: string;
  /** Whether the server has a body by this name. */
  readonly online: boolean;
  readonly pos: Vec3Tuple | undefined;
  /** `playerGameType`: 2 adventure, 3 spectator. */
  readonly mode: number | undefined;
  /** Whether the body carries the observation tag a cutscene and a wait put on it. */
  readonly observing: boolean;
  /** This member's grant issue count (`dw.gr`); absent when it has none. */
  readonly granted: number | undefined;
  /** The party's (`#all dw.gr`); absent in a build that issues no grant. */
  readonly partyGranted: number | undefined;
  readonly deaths: number | undefined;
  /** The active checkpoint (`#cp dw.sys`); absent before any. */
  readonly cp: number | undefined;
}

const OBSERVING_TAG = "dw_cutscene";

/** The commands that make the server state `names`' readings to the asking bot. */
export function readingCommands(serial: number, names: readonly string[]): string[] {
  const out: string[] = [];
  for (const name of names) {
    const body = `@a[name=${name},limit=1]`;
    const sep = { text: "|" };
    const score = (holder: string, objective: string) => ({ score: { name: holder, objective } });
    out.push(
      `/tellraw @s ${JSON.stringify([
        { text: `[dw:party ${serial} ${name}|` },
        { nbt: "Pos", entity: body },
        sep,
        { nbt: "playerGameType", entity: body },
        sep,
        score(name, "dw.gr"),
        sep,
        score("#all", "dw.gr"),
        sep,
        score(name, "dw.deaths"),
        sep,
        score("#cp", "dw.sys"),
        { text: "]" },
      ])}`,
    );
    out.push(
      `/execute if entity @a[name=${name},tag=${OBSERVING_TAG}] run tellraw @s ` +
        JSON.stringify({ text: `[dw:party-obs ${serial} ${name}]` }),
    );
  }
  return out;
}

const READING_RE = /^\[dw:party (\d+) ([A-Za-z0-9_]+)\|([^|]*)\|([^|]*)\|([^|]*)\|([^|]*)\|([^|]*)\|([^|]*)\]$/;
const OBS_RE = /^\[dw:party-obs (\d+) ([A-Za-z0-9_]+)\]$/;
// The pinned server prints a list's SNBT without a space after the comma
// (`[15.5d,67.0d,2.5d]`); either spelling is read.
const POS_RE = /^\[(-?[\d.E-]+)d,\s*(-?[\d.E-]+)d,\s*(-?[\d.E-]+)d\]$/;

function int(text: string): number | undefined {
  return /^-?\d+$/.test(text) ? Number(text) : undefined;
}

/**
 * Parse the lines a reading's commands were answered with. A member with no
 * line at all is refused: the reading did not happen, which is never the same
 * as a member who is offline (whose line arrives with its body fields empty).
 */
export function parseReadings(
  serial: number,
  lines: readonly string[],
  names: readonly string[],
): Map<string, MemberReading> {
  const observing = new Set<string>();
  const raw = new Map<string, RegExpExecArray>();
  for (const line of lines) {
    const o = OBS_RE.exec(line);
    if (o && Number(o[1]) === serial) observing.add(o[2]!);
    const m = READING_RE.exec(line);
    if (m && Number(m[1]) === serial) raw.set(m[2]!, m);
  }
  const out = new Map<string, MemberReading>();
  for (const name of names) {
    const m = raw.get(name);
    if (!m) throw new Error(`party reading ${serial}: the server stated nothing for ${name}`);
    const p = POS_RE.exec(m[3]!);
    const pos: Vec3Tuple | undefined = p ? [Number(p[1]), Number(p[2]), Number(p[3])] : undefined;
    out.set(name, {
      name,
      online: pos !== undefined,
      pos,
      mode: int(m[4]!),
      observing: observing.has(name),
      granted: int(m[5]!),
      partyGranted: int(m[6]!),
      deaths: int(m[7]!),
      cp: int(m[8]!),
    });
  }
  return out;
}

// ---------------------------------------------------------------------------
// Verdicts
// ---------------------------------------------------------------------------

/** Whether a reading is settled: in play, out of every observation state. */
export function inPlay(r: MemberReading): boolean {
  return r.online && r.mode === 2 && !r.observing;
}

export function distance(a: Vec3Tuple, b: Vec3Tuple): number {
  return Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);
}

/** A seat's standing point: vanilla lands a body at the cell's centre. */
export function seatPoint(seat: Seat): Vec3Tuple {
  return [seat.cell[0] + 0.5, seat.cell[1], seat.cell[2] + 0.5];
}

/** The seat a respawn or a late class lands on with `cp` in force: the entry spawn below 0. */
export function seatInForce(seats: readonly Seat[], cp: number | undefined): Seat | undefined {
  const key = cp === undefined || cp < 0 ? -1 : cp;
  return seats.find((s) => s.cp === key);
}

const fmt = (p: Vec3Tuple | undefined) => (p ? p.map((v) => v.toFixed(2)).join(" ") : "nowhere");

/**
 * The idler at one boundary: in play, and where it stood — or, after a death,
 * on the seat in force. `undefined` when the reading holds; the failure
 * otherwise, naming the step.
 */
export function judgeIdler(
  stand: Vec3Tuple,
  baselineDeaths: number,
  reading: MemberReading,
  seats: readonly Seat[],
  at: string,
): string | undefined {
  if (!reading.online) return `${at}: the idler is not on the server`;
  if (!inPlay(reading)) {
    return (
      `${at}: the idler is stranded out of play — game mode ${reading.mode ?? "unread"}` +
      `${reading.observing ? ", still under the observation tag" : ""}`
    );
  }
  const died = (reading.deaths ?? 0) > baselineDeaths;
  if (died) {
    const seat = seatInForce(seats, reading.cp);
    if (seat === undefined) return undefined;
    const d = distance(reading.pos!, seatPoint(seat));
    return d <= STAND_TOLERANCE
      ? undefined
      : `${at}: the idler died and stands at ${fmt(reading.pos)}, ${d.toFixed(2)} from the seat in force (${seat.label})`;
  }
  const d = distance(reading.pos!, stand);
  return d <= STAND_TOLERANCE
    ? undefined
    : `${at}: the idler was moved — it stood at ${fmt(stand)} and stands at ${fmt(reading.pos)} ` +
        `(${d.toFixed(2)} blocks), with no death of its own`;
}

/** A member's grants against the party's. `bound` is false in a build that issues none. */
export interface OwedVerdict {
  readonly bound: boolean;
  readonly ok: boolean;
  readonly detail: string;
}

export function judgeOwed(reading: MemberReading, who: string): OwedVerdict {
  if (reading.partyGranted === undefined || reading.partyGranted === 0) {
    return { bound: false, ok: true, detail: `${who}: the party was handed no grant` };
  }
  const ok = reading.granted === reading.partyGranted;
  return {
    bound: true,
    ok,
    detail:
      `${who}: ${reading.granted ?? "no"} grant issue(s) held of the party's ${reading.partyGranted}` +
      (ok ? "" : " — a stack for the party never reached this member"),
  };
}

/** A late member's arrival after its class: on the seat in force. */
export function judgeArrival(
  reading: MemberReading,
  seats: readonly Seat[],
): { bound: boolean; ok: boolean; detail: string } {
  const seat = seatInForce(seats, reading.cp);
  if (seat === undefined) {
    return {
      bound: false,
      ok: true,
      detail: `the build names no seat for checkpoint ${reading.cp ?? -1}: arrival unjudged`,
    };
  }
  if (!reading.pos) return { bound: true, ok: false, detail: "the late member has no body to read" };
  const d = distance(reading.pos, seatPoint(seat));
  return {
    bound: true,
    ok: d <= STAND_TOLERANCE,
    detail:
      `the late member arrived at ${fmt(reading.pos)}, ${d.toFixed(2)} from ${seat.label} ` +
      `(checkpoint ${reading.cp ?? -1} in force)`,
  };
}

/** One completion marker as one body heard it. */
export interface Heard {
  readonly token: string;
  readonly at: number;
}

/** A span a member was on the server: `to` absent while it still is. */
export interface Presence {
  readonly from: number;
  readonly to?: number;
}

/** The tellings a member was owed, and what it was told. */
export interface TellingVerdict {
  /** Markers the walker heard while the member was on the server. */
  readonly owed: number;
  /** Of those, the ones the member heard too. */
  readonly heard: number;
  /** Markers too close to a join or a leave to say either way. */
  readonly edge: number;
  readonly missing: readonly string[];
  /** Markers the member heard that the walker did not hear at that moment: a replay. */
  readonly replayed: readonly string[];
}

/** How far apart two bodies may hear one broadcast. */
export const TELLING_SLACK_MS = 3_000;

/**
 * Who was told: every marker the walking bot heard while `member` was present
 * reached it, and it heard nothing the walker did not hear at the same moment.
 */
export function judgeTellings(
  walker: readonly Heard[],
  member: readonly Heard[],
  presence: readonly Presence[],
  slack = TELLING_SLACK_MS,
): TellingVerdict {
  const inside = (t: number) =>
    presence.some((p) => t >= p.from + slack && (p.to === undefined || t <= p.to - slack));
  const near = (t: number) =>
    presence.some(
      (p) => Math.abs(t - p.from) < slack || (p.to !== undefined && Math.abs(t - p.to) < slack),
    );
  const unused = [...member];
  let owed = 0;
  let heard = 0;
  let edge = 0;
  const missing: string[] = [];
  for (const w of walker) {
    if (near(w.at)) {
      edge += 1;
      const k = unused.findIndex((m) => m.token === w.token && Math.abs(m.at - w.at) <= slack);
      if (k >= 0) unused.splice(k, 1);
      continue;
    }
    if (!inside(w.at)) continue;
    owed += 1;
    const k = unused.findIndex((m) => m.token === w.token && Math.abs(m.at - w.at) <= slack);
    if (k >= 0) {
      heard += 1;
      unused.splice(k, 1);
    } else {
      missing.push(w.token);
    }
  }
  return { owed, heard, edge, missing, replayed: unused.map((m) => m.token) };
}
