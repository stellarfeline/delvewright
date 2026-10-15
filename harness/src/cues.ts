// The engine's quest cues reach each player where they stand.
//
// The compiler plays three sounds no campaign writes: the "new objective" cue an
// announced objective plays when it activates, the cue it plays when it
// completes, and the campaign-complete fanfare. Each is addressed to the whole
// party. A `playsound` with no position plays at its command source, and the
// functions these run in carry the server's source, which stands at the world
// spawn: a cue played that way reaches a player standing within the sound's
// range of the spawn at the spawn, and a player beyond it not at all.
//
// PackTest cannot see a sound packet. The bot can: it records every sound
// packet naming a cue at the ledger's volume and pitch, and every completion
// marker, in the order they arrive, with its own position at each. Then:
//
// - an announced objective's marker owes its completion cue, the first one
//   after the marker and before the next marker (the cue follows its marker in
//   the completion function);
// - the campaign marker owes the campaign cue, anywhere between the markers on
//   either side of it;
// - an owed cue must arrive, at the bot's own position;
// - every cue packet heard, owed or not, must arrive at the bot's own position.
//
// The compiler states the cues, the objectives that owe them, the world spawn
// and the range (`validation/cues.json`); the harness derives nothing.
//
// Assertions and bookkeeping only — no game logic (CLAUDE.md).

import { readFile } from "node:fs/promises";
import path from "node:path";

import { CAMPAIGN_TOKEN } from "./markers.ts";

type Vec3 = readonly [number, number, number];

/** Where the ledger sits relative to `critical-path.json`. */
export const CUE_LEDGER_SUBPATH = ["validation", "cues.json"] as const;

/** The three cue kinds, by the ledger's names. */
export type CueKind = "announce" | "complete" | "campaign";

/** The ledger as the bot reads it. */
export interface CueLedger {
  /** The cell a positionless sound would play at, or `null` when the build has none. */
  readonly worldSpawn: Vec3 | null;
  readonly volume: number;
  readonly pitch: number;
  /** How far, in blocks, a sound at `volume` carries. */
  readonly range: number;
  readonly cues: Readonly<Record<CueKind, string>>;
  /** The announced objectives: each owes a completion cue at its marker. */
  readonly objectives: readonly string[];
}

/** Raised when the ledger is malformed — a hard failure, never a skip. */
export class CueLedgerParseError extends Error {
  override readonly name = "CueLedgerParseError";
}

function bad(detail: string): never {
  throw new CueLedgerParseError(`cues.json ${detail}`);
}

const KINDS: readonly CueKind[] = ["announce", "complete", "campaign"];

/** Parse the ledger strictly. */
export function parseCueLedger(raw: unknown): CueLedger {
  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) bad("is not an object");
  const root = raw as Record<string, unknown>;
  const spawn = root["world_spawn"];
  let worldSpawn: Vec3 | null = null;
  if (spawn !== null) {
    if (!Array.isArray(spawn) || spawn.length !== 3 || !spawn.every((n) => Number.isInteger(n))) {
      bad("/world_spawn is not a block cell or null");
    }
    worldSpawn = [spawn[0] as number, spawn[1] as number, spawn[2] as number];
  }
  for (const k of ["volume", "pitch", "range"]) {
    if (typeof root[k] !== "number") bad(`/${k} is not a number`);
  }
  const c = root["cues"];
  if (typeof c !== "object" || c === null || Array.isArray(c)) bad("/cues is not an object");
  const cr = c as Record<string, unknown>;
  const cues = {} as Record<CueKind, string>;
  for (const k of KINDS) {
    const v = cr[k];
    if (typeof v !== "string" || v.length === 0) bad(`/cues/${k} is not a non-empty string`);
    cues[k] = v;
  }
  const objectives = root["objectives"];
  if (!Array.isArray(objectives) || !objectives.every((o) => typeof o === "string")) {
    bad("/objectives is not an array of objective ids");
  }
  return {
    worldSpawn,
    volume: root["volume"] as number,
    pitch: root["pitch"] as number,
    range: root["range"] as number,
    cues,
    objectives: objectives as string[],
  };
}

/** Load the ledger beside a critical path; absent is a build that predates it. */
export async function loadCueLedgerForCriticalPath(
  criticalPathFile: string,
): Promise<CueLedger | undefined> {
  const file = path.join(path.dirname(criticalPathFile), ...CUE_LEDGER_SUBPATH);
  let text: string;
  try {
    text = await readFile(file, "utf8");
  } catch (e) {
    if ((e as NodeJS.ErrnoException).code === "ENOENT") return undefined;
    throw e;
  }
  return parseCueLedger(JSON.parse(text) as unknown);
}

/** One thing the bot was told, in arrival order, with where the bot stood. */
export type CueEvent =
  | { readonly kind: "marker"; readonly token: string; readonly bot: Vec3 | null }
  | {
      readonly kind: "sound";
      readonly name: string;
      readonly pos: Vec3;
      readonly volume: number;
      readonly pitch: number;
      readonly bot: Vec3 | null;
    };

/**
 * How far (blocks, per axis) a cue may land from the bot. The packet carries
 * its position in eighths of a block and the bot may move a step between the
 * server playing the cue and the client reading it; a cue at the command source
 * misses by the distance from the spawn.
 */
export const CUE_POS_TOLERANCE = 1.5;

/** How close a packet's volume and pitch must be to the ledger's to be a cue. */
const CUE_FLOAT_TOLERANCE = 0.01;

const bare = (s: string): string => s.replace(/^minecraft:/, "");

/** The verdict on one cue. */
export interface CueVerdict {
  readonly kind: CueKind;
  /** The marker that owed it, or `null` for an announce cue (owed by no marker). */
  readonly token: string | null;
  readonly sound: string;
  /** Where the cue arrived, or `null` when an owed cue did not. */
  readonly heard: Vec3 | null;
  /** The bot's position when the cue (or, unheard, its marker) arrived. */
  readonly bot: Vec3 | null;
  /** The bot's distance from the world spawn there, or `null` when unknown. */
  readonly fromSpawn: number | null;
  readonly ok: boolean;
  readonly detail: string;
}

/** What the run concluded about the cues. */
export interface CueJudgement {
  /** The cues a marker owed, in arrival order. */
  readonly owed: readonly CueVerdict[];
  /** Every cue packet no marker claimed (announce cues, and any other). */
  readonly unclaimed: readonly CueVerdict[];
  readonly failures: readonly string[];
}

function distance(a: Vec3, b: Vec3): number {
  return Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);
}

const round8 = (v: Vec3): number[] => v.map((x) => Math.round(x * 8) / 8);

function atBot(heard: Vec3, bot: Vec3 | null): boolean {
  return bot !== null && heard.every((v, i) => Math.abs(v - bot[i]!) <= CUE_POS_TOLERANCE);
}

function fromSpawn(ledger: CueLedger, bot: Vec3 | null): number | null {
  if (!ledger.worldSpawn || !bot) return null;
  const s = ledger.worldSpawn;
  return distance([s[0] + 0.5, s[1], s[2] + 0.5], bot);
}

/** Which cue kind a packet is, or `undefined` when it is not one. */
function cueKindOf(ledger: CueLedger, e: CueEvent): CueKind | undefined {
  if (e.kind !== "sound") return undefined;
  if (Math.abs(e.volume - ledger.volume) > CUE_FLOAT_TOLERANCE) return undefined;
  if (Math.abs(e.pitch - ledger.pitch) > CUE_FLOAT_TOLERANCE) return undefined;
  return KINDS.find((k) => bare(ledger.cues[k]) === bare(e.name));
}

/** **Judge the event log against the ledger.** Pure: no bot state. */
export function judgeCues(ledger: CueLedger, events: readonly CueEvent[]): CueJudgement {
  const owing = new Set(ledger.objectives);
  const seen = new Set<string>();
  const claimed = new Set<number>();
  const owed: CueVerdict[] = [];
  const isMarker = (i: number): boolean => events[i]!.kind === "marker";
  events.forEach((e, i) => {
    if (e.kind !== "marker" || seen.has(e.token)) return;
    seen.add(e.token);
    let kind: CueKind;
    let from: number;
    if (e.token === CAMPAIGN_TOKEN) {
      kind = "campaign";
      from = i;
      while (from > 0 && !isMarker(from - 1)) from -= 1;
    } else if (owing.has(e.token)) {
      kind = "complete";
      from = i + 1;
    } else {
      return;
    }
    let to = i + 1;
    while (to < events.length && !isMarker(to)) to += 1;
    let hit: number | undefined;
    for (let j = from; j < to; j += 1) {
      if (j !== i && !claimed.has(j) && cueKindOf(ledger, events[j]!) === kind) {
        hit = j;
        break;
      }
    }
    const sound = ledger.cues[kind];
    if (hit === undefined) {
      owed.push({
        kind,
        token: e.token,
        sound,
        heard: null,
        bot: e.bot,
        fromSpawn: fromSpawn(ledger, e.bot),
        ok: false,
        detail:
          `${e.token}'s ${kind} cue ${sound} did NOT arrive: no packet naming it at volume ` +
          `${ledger.volume} pitch ${ledger.pitch} between the markers around it (the bot at ` +
          `${JSON.stringify(e.bot && round8(e.bot))}, ${fmtSpawn(fromSpawn(ledger, e.bot))})`,
      });
      return;
    }
    claimed.add(hit);
    owed.push(verdictOf(ledger, kind, e.token, events[hit]!));
  });
  const unclaimed: CueVerdict[] = [];
  events.forEach((e, j) => {
    const kind = cueKindOf(ledger, e);
    if (kind === undefined || claimed.has(j)) return;
    unclaimed.push(verdictOf(ledger, kind, null, e));
  });
  const failures = [...owed, ...unclaimed].filter((v) => !v.ok).map((v) => `cue: ${v.detail}`);
  return { owed, unclaimed, failures };
}

function fmtSpawn(d: number | null): string {
  return d === null ? "world spawn unknown" : `${d.toFixed(1)} blocks from the world spawn`;
}

function verdictOf(ledger: CueLedger, kind: CueKind, token: string | null, e: CueEvent): CueVerdict {
  if (e.kind !== "sound") throw new Error("a cue verdict is of a sound");
  const ok = atBot(e.pos, e.bot);
  const who = token === null ? `a ${kind} cue` : `${token}'s ${kind} cue`;
  const bot = JSON.stringify(e.bot && round8(e.bot));
  const d = fromSpawn(ledger, e.bot);
  return {
    kind,
    token,
    sound: ledger.cues[kind],
    heard: e.pos,
    bot: e.bot,
    fromSpawn: d,
    ok,
    detail: ok
      ? `${who} ${e.name} arrived at ${JSON.stringify(round8(e.pos))}, the bot at ${bot} (${fmtSpawn(d)})`
      : `${who} ${e.name} arrived at ${JSON.stringify(round8(e.pos))}, NOT at the bot (${bot}, ` +
        `tolerance ${CUE_POS_TOLERANCE}; ${fmtSpawn(d)})`,
  };
}

/** The counts the run report carries and CI reads. */
export interface CueBinding {
  /** Announced objectives the build declares. */
  readonly objectives: number;
  /** Cues a marker the run observed owed. */
  readonly owed: number;
  /** Of those, the ones that arrived at the bot. */
  readonly arrived: number;
  /** Owed cues judged with the bot farther from the world spawn than the cue's range. */
  readonly beyondRange: number;
  /** Cue packets no marker claimed, and how many of them arrived at the bot. */
  readonly unclaimed: number;
  readonly unclaimedAtBot: number;
}

export function cueBinding(ledger: CueLedger, j: CueJudgement): CueBinding {
  return {
    objectives: ledger.objectives.length,
    owed: j.owed.length,
    arrived: j.owed.filter((v) => v.ok).length,
    beyondRange: j.owed.filter((v) => v.fromSpawn !== null && v.fromSpawn > ledger.range).length,
    unclaimed: j.unclaimed.length,
    unclaimedAtBot: j.unclaimed.filter((v) => v.ok).length,
  };
}

/** The one line the run prints, with its denominators. */
export function cueBindingLine(ledger: CueLedger, b: CueBinding): string {
  return (
    `cue binding: ${b.arrived} of ${b.owed} owed cue(s) arrived at the bot ` +
    `(${ledger.objectives.length} announced objective(s) in the build), ` +
    `${b.beyondRange} of the ${b.owed} judged farther than ${ledger.range} blocks from the world spawn; ` +
    `${b.unclaimedAtBot} of ${b.unclaimed} other cue packet(s) at the bot` +
    (b.owed === 0 ? " — UNBOUND: no marker the run observed owed a cue" : "")
  );
}
