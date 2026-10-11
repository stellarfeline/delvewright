// A pulse is heard where the compiler says it is (spec-0102 §5.3).
//
// A pulse is a `playsound` the server schedules on a fixed interval to every
// player standing in a place. PackTest proves the chain re-arms and the latch
// moves; what it cannot see is the packet, which is client-bound. This hears
// it: at each pulse's listening station the bot stands `2 · every + 10` ticks
// and must hear at least two beats naming the sound, each at the source point
// (±0.5), at the derived volume and the declared pitch; at its silent station
// it stands as long and must hear none.
//
// The compiler computes everything (`validation/pulses.json`): the stations
// from the forced route's proven legs, the volume from the declared `floor`.
// The harness derives no position, no volume and no station itself.
//
// Assertions and bookkeeping only — no game logic (CLAUDE.md).

import { readFile } from "node:fs/promises";
import path from "node:path";

import type { Vec3Tuple } from "./critical-path.ts";

/** Where the export sits, relative to the critical path's directory. */
export const PULSE_PLAN_SUBPATH = ["validation", "pulses.json"] as const;

/** Position tolerance, in blocks: the packet carries eighths of a block. */
export const PULSE_POS_TOLERANCE = 0.5;

/** Volume and pitch tolerance: the packet carries 32-bit floats. */
export const PULSE_FLOAT_TOLERANCE = 1e-3;

/** How many beats a listening station must hear. */
export const PULSE_MIN_BEATS = 2;

/** A cell of the forced route the bot stands at. */
export interface PlannedStation {
  readonly cell: Vec3Tuple;
  /** The compiler's critical-step index of the leg holding the cell. */
  readonly step: number;
  /** The token (objective or trigger id) of that step: "before this step". */
  readonly before: string;
}

/** One pulse the build declares. */
export interface PlannedPulse {
  readonly id: string;
  /** The sound event, namespaced. */
  readonly sound: string;
  /** The point the sound stands at. */
  readonly source: readonly [number, number, number];
  readonly every: number;
  readonly volume: number;
  readonly pitch: number;
  readonly listening: PlannedStation | undefined;
  readonly silent: PlannedStation | undefined;
}

/** The parsed export. */
export interface PulsePlan {
  readonly declared: number;
  readonly pulses: readonly PlannedPulse[];
}

/** Raised when the export is malformed — a hard failure, never a skip. */
export class PulsePlanParseError extends Error {
  override readonly name = "PulsePlanParseError";
}

function bad(detail: string): never {
  throw new PulsePlanParseError(`pulses.json ${detail}`);
}

function vec3(v: unknown, at: string): [number, number, number] {
  if (!Array.isArray(v) || v.length !== 3 || !v.every((n) => typeof n === "number")) {
    bad(`${at} is not [x, y, z]`);
  }
  return [v[0] as number, v[1] as number, v[2] as number];
}

function station(v: unknown, at: string): PlannedStation | undefined {
  if (v === null) return undefined;
  if (typeof v !== "object" || v === undefined) bad(`${at} is not an object or null`);
  const s = v as Record<string, unknown>;
  const cell = vec3(s["cell"], `${at}/cell`);
  if (!cell.every((n) => Number.isInteger(n))) bad(`${at}/cell is not a block cell`);
  if (typeof s["step"] !== "number") bad(`${at}/step is not a number`);
  if (typeof s["before"] !== "string") bad(`${at}/before is not a string`);
  return { cell, step: s["step"] as number, before: s["before"] as string };
}

/** Parse the export strictly. */
export function parsePulsePlan(raw: unknown): PulsePlan {
  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) bad("is not an object");
  const root = raw as Record<string, unknown>;
  const declared = root["declared"];
  if (typeof declared !== "number") bad("/declared is not a number");
  const list = root["pulses"];
  if (!Array.isArray(list)) bad("/pulses is not an array");
  const pulses = list.map((entry, i): PlannedPulse => {
    if (typeof entry !== "object" || entry === null) bad(`/pulses/${i} is not an object`);
    const e = entry as Record<string, unknown>;
    for (const k of ["id", "sound"]) {
      if (typeof e[k] !== "string") bad(`/pulses/${i}/${k} is not a string`);
    }
    for (const k of ["every", "volume", "pitch"]) {
      if (typeof e[k] !== "number") bad(`/pulses/${i}/${k} is not a number`);
    }
    const st = e["stations"];
    if (typeof st !== "object" || st === null) bad(`/pulses/${i}/stations is not an object`);
    const stations = st as Record<string, unknown>;
    return {
      id: e["id"] as string,
      sound: e["sound"] as string,
      source: vec3(e["source"], `/pulses/${i}/source`),
      every: e["every"] as number,
      volume: e["volume"] as number,
      pitch: e["pitch"] as number,
      listening: station(stations["listening"], `/pulses/${i}/stations/listening`),
      silent: station(stations["silent"], `/pulses/${i}/stations/silent`),
    };
  });
  if (pulses.length !== declared) {
    bad(`declares ${declared} pulse(s) and carries ${pulses.length} row(s)`);
  }
  return { declared, pulses };
}

/** Load the export beside a critical path; absent means the build declares no pulse. */
export async function loadPulsePlanForCriticalPath(
  criticalPathFile: string,
): Promise<PulsePlan | undefined> {
  const file = path.join(path.dirname(criticalPathFile), ...PULSE_PLAN_SUBPATH);
  let text: string;
  try {
    text = await readFile(file, "utf8");
  } catch {
    return undefined;
  }
  return parsePulsePlan(JSON.parse(text) as unknown);
}

/** How long the bot stands at a station, in ticks. */
export function standTicks(p: PlannedPulse): number {
  return 2 * p.every + 10;
}

/** One sound the client was told about. */
export interface HeardSound {
  readonly name: string;
  readonly pos: readonly [number, number, number];
  readonly volume: number;
  readonly pitch: number;
}

const bare = (s: string): string => s.replace(/^minecraft:/, "");

/** Whether `h` names the pulse's sound. */
export function namesSound(p: PlannedPulse, h: HeardSound): boolean {
  return bare(h.name) === bare(p.sound);
}

/** Whether `h` is one of the pulse's beats as the compiler wrote it. */
export function isBeat(p: PlannedPulse, h: HeardSound): boolean {
  return (
    namesSound(p, h) &&
    h.pos.every((v, i) => Math.abs(v - p.source[i]!) <= PULSE_POS_TOLERANCE) &&
    Math.abs(h.volume - p.volume) <= PULSE_FLOAT_TOLERANCE &&
    Math.abs(h.pitch - p.pitch) <= PULSE_FLOAT_TOLERANCE
  );
}

/** The listening verdict: `undefined` when heard, else the failure. */
export function judgeListening(p: PlannedPulse, heard: readonly HeardSound[]): string | undefined {
  const named = heard.filter((h) => namesSound(p, h));
  const beats = named.filter((h) => isBeat(p, h));
  if (beats.length >= PULSE_MIN_BEATS) return undefined;
  const odd = named.find((h) => !isBeat(p, h));
  return (
    `pulse ${p.id}: heard ${beats.length} beat(s) of ${p.sound} at its listening station ` +
    `${JSON.stringify(p.listening?.cell)} over ${standTicks(p)} ticks, owed at least ` +
    `${PULSE_MIN_BEATS} at ${JSON.stringify(p.source)} volume ${p.volume} pitch ${p.pitch}` +
    (odd
      ? `; a ${p.sound} arrived at ${JSON.stringify(odd.pos)} volume ${odd.volume} pitch ` +
        `${odd.pitch} instead`
      : "")
  );
}

/** The silent verdict: `undefined` when nothing was heard, else the failure. */
export function judgeSilent(p: PlannedPulse, heard: readonly HeardSound[]): string | undefined {
  const named = heard.filter((h) => namesSound(p, h));
  if (named.length === 0) return undefined;
  return (
    `pulse ${p.id}: heard ${named.length} ${p.sound} at its silent station ` +
    `${JSON.stringify(p.silent?.cell)}, outside the place it is heard in`
  );
}

/** A station due before the step whose token is `before`. */
export interface DueStation {
  readonly pulse: PlannedPulse;
  readonly kind: "listening" | "silent";
  readonly station: PlannedStation;
  /** A key the run marks done, so a token the path repeats is listened at once. */
  readonly key: string;
}

/** Every station due before the step with token `before`, not yet done. */
export function stationsBefore(
  plan: PulsePlan,
  before: string,
  done: ReadonlySet<string>,
): DueStation[] {
  const out: DueStation[] = [];
  for (const pulse of plan.pulses) {
    for (const kind of ["listening", "silent"] as const) {
      const station = pulse[kind];
      if (!station || station.before !== before) continue;
      const key = `${pulse.id}#${kind}`;
      if (done.has(key)) continue;
      out.push({ pulse, kind, station, key });
    }
  }
  return out;
}

/** What one station heard, for the run report. */
export interface StationRecord {
  readonly pulse: string;
  readonly kind: "listening" | "silent";
  readonly cell: Vec3Tuple;
  readonly ticks: number;
  /** Sounds naming the pulse's sound. */
  readonly named: number;
  /** Of those, the beats as the compiler wrote them. */
  readonly beats: number;
  readonly failure: string | null;
}

/** What one pulse's stations concluded. */
export interface PulseVerdict {
  readonly id: string;
  readonly listening: "heard" | "failed" | "no station" | "not reached";
  readonly silent: "silent" | "failed" | "no station" | "not reached";
  /** What a station that was reached heard wrong. */
  readonly failures: readonly string[];
  /** Stations the walk never reached — a failure of a run that walked the whole path. */
  readonly unreached: readonly string[];
}

/** Every pulse's verdict from what the stations recorded. */
export function pulseVerdicts(
  plan: PulsePlan,
  recorded: ReadonlyMap<string, string | undefined>,
): PulseVerdict[] {
  return plan.pulses.map((p) => {
    const failures: string[] = [];
    const unreached: string[] = [];
    const l = `${p.id}#listening`;
    const s = `${p.id}#silent`;
    const listening: PulseVerdict["listening"] = !p.listening
      ? "no station"
      : !recorded.has(l)
        ? "not reached"
        : recorded.get(l) === undefined
          ? "heard"
          : "failed";
    const silent: PulseVerdict["silent"] = !p.silent
      ? "no station"
      : !recorded.has(s)
        ? "not reached"
        : recorded.get(s) === undefined
          ? "silent"
          : "failed";
    for (const k of [l, s]) {
      const f = recorded.get(k);
      if (f !== undefined) failures.push(f);
    }
    if (p.listening && !recorded.has(l)) {
      unreached.push(
        `pulse ${p.id}: its listening station before ${p.listening.before} was never reached`,
      );
    }
    if (p.silent && !recorded.has(s)) {
      unreached.push(`pulse ${p.id}: its silent station before ${p.silent.before} was never reached`);
    }
    return { id: p.id, listening, silent, failures, unreached };
  });
}

/** The one line the run prints, with its denominator. */
export function pulseBindingLine(plan: PulsePlan, verdicts: readonly PulseVerdict[]): string {
  const heard = verdicts.filter((v) => v.listening === "heard").length;
  const silent = verdicts.filter((v) => v.silent === "silent").length;
  const unheard = verdicts.filter((v) => v.listening === "no station").map((v) => v.id);
  return (
    `pulse binding: ${heard} of ${plan.pulses.length} pulse(s) heard at a listening station, ` +
    `${silent} silent at a silent station` +
    (unheard.length > 0 ? `; not_heard: no station — ${unheard.join(", ")}` : "")
  );
}
