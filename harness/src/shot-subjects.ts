// A shot's body subject is there to be seen (spec-0110 §7, the live half).
//
// The build proves, from the campaign, that every body a cutscene shot frames
// is placed before the cutscene and within its type's tracking range of the
// lens (`DW1034`). The bot is a client the server tracks at the camera, so it
// is the one seat that can watch those two claims happen: during each
// cutscene whose shots frame a body, it records whether that body was spawned
// to it by the shot's first tick and stayed spawned through the shot, read
// from its own entity table. What a client DRAWS cannot be read from a bot.
//
// The clock is the server's game time, never the client's physics ticks
// (which run behind the server and miss the last driver tick: the end
// function returns the gamemode in the same server tick as the last driver
// tick, so no client sees that tick in spectator). While the bot spectates,
// the executor reads the game time over and over (`executor/clock.ts`); a
// reading answered at game time `g` is an exact place in the packet stream:
// everything before it was sent at `g` or earlier. The end is the server's
// own record of the span: the end function runs at the last driver tick
// (`driverTicks - 1`), so the reading after the end places it, and with it
// every reading before. Driver tick `d` is judged on the entity table as the
// server left it at the end of `d` — the reading at `d`, or for the last tick
// the table at the end. A tick no reading landed on takes its neighbours'
// table when no body-typed entity was spawned or removed between them;
// otherwise it is unplaced, and a body is not counted there.
//
// The compiler computes everything (`validation/shot-subjects.json`): where
// the lens stands at each cutscene's first tick (the point a watching client
// is snapped to), each shot's first driver tick and length, and each body's
// entity type and position at the shot's first tick. The harness derives no
// position and no timing itself.
//
// Assertions and bookkeeping only — no game logic (CLAUDE.md).

import { readFile } from "node:fs/promises";
import path from "node:path";

/** Where the export sits, relative to the critical path's directory. */
export const SHOT_SUBJECTS_SUBPATH = ["validation", "shot-subjects.json"] as const;

/** How close (blocks) the bot must be to a cutscene's first lens to be in it. */
export const LENS_TOLERANCE = 1.0;

/** How far (blocks) a seen entity may stand from where the body is planned. */
export const BODY_TOLERANCE = 2.0;

type Vec3 = readonly [number, number, number];

/** One body a shot frames. */
export interface PlannedBody {
  readonly field: string;
  readonly subject: string;
  /** Namespaced entity type, e.g. `minecraft:item_display`. */
  readonly entity: string;
  readonly at: Vec3;
}

/** One shot with at least one body subject. */
export interface PlannedShot {
  readonly index: number;
  readonly startTick: number;
  readonly ticks: number;
  readonly bodies: readonly PlannedBody[];
}

/** One cutscene with at least one body subject. */
export interface PlannedCutscene {
  readonly path: string;
  readonly digest: string;
  readonly startLens: Vec3;
  /** Driver ticks the cutscene plays: its last shot's last tick, plus one. */
  readonly driverTicks: number;
  readonly shots: readonly PlannedShot[];
}

/** The parsed export: only the cutscenes that frame a body. */
export interface ShotSubjectPlan {
  readonly cutscenes: readonly PlannedCutscene[];
}

/** Raised when the export is malformed — a hard failure, never a skip. */
export class ShotSubjectPlanParseError extends Error {
  override readonly name = "ShotSubjectPlanParseError";
}

function bad(detail: string): never {
  throw new ShotSubjectPlanParseError(`shot-subjects.json ${detail}`);
}

function vec3(v: unknown, at: string): Vec3 {
  if (!Array.isArray(v) || v.length !== 3 || !v.every((n) => typeof n === "number")) {
    bad(`${at} is not [x, y, z]`);
  }
  return [v[0] as number, v[1] as number, v[2] as number];
}

function obj(v: unknown, at: string): Record<string, unknown> {
  if (typeof v !== "object" || v === null || Array.isArray(v)) bad(`${at} is not an object`);
  return v as Record<string, unknown>;
}

function arr(v: unknown, at: string): unknown[] {
  if (!Array.isArray(v)) bad(`${at} is not an array`);
  return v;
}

/** Parse the export strictly, keeping the cutscenes that frame a body. */
export function parseShotSubjectPlan(raw: unknown): ShotSubjectPlan {
  const root = obj(raw, "");
  const cutscenes: PlannedCutscene[] = [];
  arr(root["cutscenes"], "/cutscenes").forEach((c, i) => {
    const cs = obj(c, `/cutscenes/${i}`);
    const shots: PlannedShot[] = [];
    let driverTicks = 0;
    arr(cs["shots"], `/cutscenes/${i}/shots`).forEach((s, j) => {
      const at = `/cutscenes/${i}/shots/${j}`;
      const shot = obj(s, at);
      if (typeof shot["index"] !== "number") bad(`${at}/index is not a number`);
      if (typeof shot["start_tick"] !== "number") bad(`${at}/start_tick is not a number`);
      if (typeof shot["ticks"] !== "number") bad(`${at}/ticks is not a number`);
      driverTicks = Math.max(driverTicks, (shot["start_tick"] as number) + (shot["ticks"] as number) + 1);
      const bodies: PlannedBody[] = [];
      arr(shot["subjects"], `${at}/subjects`).forEach((r, k) => {
        const sub = obj(r, `${at}/subjects/${k}`);
        if (sub["entity"] === null || sub["entity"] === undefined) return;
        if (typeof sub["entity"] !== "string") bad(`${at}/subjects/${k}/entity is not a string`);
        bodies.push({
          field: String(sub["field"]),
          subject: String(sub["subject"]),
          entity: sub["entity"] as string,
          at: vec3(sub["body_at"], `${at}/subjects/${k}/body_at`),
        });
      });
      if (bodies.length > 0) {
        shots.push({
          index: shot["index"] as number,
          startTick: shot["start_tick"] as number,
          ticks: shot["ticks"] as number,
          bodies,
        });
      }
    });
    if (shots.length > 0) {
      if (typeof cs["path"] !== "string") bad(`/cutscenes/${i}/path is not a string`);
      cutscenes.push({
        path: cs["path"] as string,
        digest: String(cs["cutscene_digest"]),
        startLens: vec3(cs["start_lens"], `/cutscenes/${i}/start_lens`),
        driverTicks,
        shots,
      });
    }
  });
  return { cutscenes };
}

/** Load the export beside a critical path; absent means the build has no cutscene. */
export async function loadShotSubjectPlanForCriticalPath(
  criticalPathFile: string,
): Promise<ShotSubjectPlan | undefined> {
  const file = path.join(path.dirname(criticalPathFile), ...SHOT_SUBJECTS_SUBPATH);
  let text: string;
  try {
    text = await readFile(file, "utf8");
  } catch {
    return undefined;
  }
  return parseShotSubjectPlan(JSON.parse(text) as unknown);
}

/** An entity the client tracks, as the observer reads it. */
export interface SeenEntity {
  readonly id: number;
  /** Bare type name as mineflayer gives it, e.g. `item_display`. */
  readonly name: string;
  readonly pos: Vec3;
}

/** What one body did during one shot, for the run report. */
export interface BodyRecord {
  readonly cutscene: string;
  readonly shot: number;
  readonly subject: string;
  readonly entity: string;
  /** Spawned to the bot by the shot's first tick. */
  readonly spawnedBefore: boolean;
  /** Still spawned at every tick of the shot (false when never spawned). */
  readonly spawnedThrough: boolean;
  /** Ticks of the shot whose table no reading decides (counted as absent). */
  readonly unplacedTicks: number;
}

const bare = (s: string): string => s.replace(/^minecraft:/, "");

function dist(a: Vec3, b: Vec3): number {
  return Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);
}

/** The entity standing for `body`, if the client tracks one. */
export function findBody(body: PlannedBody, seen: readonly SeenEntity[]): SeenEntity | undefined {
  let best: SeenEntity | undefined;
  for (const e of seen) {
    if (bare(e.name) !== bare(body.entity)) continue;
    const d = dist(e.pos, body.at);
    if (d > BODY_TOLERANCE) continue;
    if (!best || d < dist(best.pos, body.at)) best = e;
  }
  return best;
}

/** The cutscene a lens at `pos` is the first tick of, if any. */
export function cutsceneAt(plan: ShotSubjectPlan, pos: Vec3): PlannedCutscene | undefined {
  return plan.cutscenes.find((c) => dist(c.startLens, pos) <= LENS_TOLERANCE);
}

/** One watched cutscene run, on the server's clock. */
export interface CutsceneSpan {
  readonly cutscene: string;
  /** Driver ticks the build plans. */
  readonly planned: number;
  /** Whether the reading after the end placed it (the readings around the
   * end are one game tick apart). Nothing else is judged when it did not. */
  readonly placed: boolean;
  /** Driver ticks a reading (or the end) landed on. */
  readonly read: number;
  /** Driver ticks whose table no reading decides. */
  readonly unplaced: number;
  /** A reading earlier than the tick before driver tick 0: the server took
   * longer from the start to the end than the plan. */
  readonly overrun: boolean;
}

type Entry =
  | { readonly kind: "clock"; readonly g: number; readonly seen: readonly SeenEntity[] }
  | { readonly kind: "change"; readonly name: string }
  | { readonly kind: "end"; readonly seen: readonly SeenEntity[] };

interface Run {
  /** `null` until the first lens packet names the cutscene; `undefined`
   * when it names none this plan watches. */
  cs: PlannedCutscene | undefined | null;
  readonly log: Entry[];
  ended: boolean;
}

/**
 * Watches the client during cutscenes, on the server's clock. Fed, in the
 * order the packets arrive: every gamemode change, the lens packets (the
 * first names the cutscene by where the bot stands), every game-time reading
 * answered, and every entity spawned to or removed from the client.
 */
export class ShotSubjectObserver {
  readonly records: BodyRecord[] = [];
  readonly spans: CutsceneSpan[] = [];
  private run: Run | undefined;

  private readonly plan: ShotSubjectPlan;

  constructor(plan: ShotSubjectPlan) {
    this.plan = plan;
  }

  /** Whether a cutscene is playing or awaits the reading that places its end. */
  get watching(): boolean {
    return this.run !== undefined;
  }

  /** The gamemode changed: spectator begins a run, leaving it is the end. */
  gameModeChanged(gameMode: string | undefined, seen: readonly SeenEntity[]): void {
    if (gameMode === "spectator") {
      if (!this.run) this.run = { cs: null, log: [], ended: false };
      return;
    }
    const run = this.run;
    if (!run || run.ended) return;
    if (!run.cs) {
      this.run = undefined;
      return;
    }
    run.log.push({ kind: "end", seen });
    run.ended = true;
  }

  /** The server aimed the client at a lens entity (not at itself). */
  lensTick(gameMode: string | undefined, pos: Vec3): void {
    if (gameMode !== "spectator" || !this.run || this.run.cs !== null) return;
    this.run.cs = cutsceneAt(this.plan, pos);
  }

  /** A game-time reading answered at `gameTime`, with the table it found. */
  clock(gameTime: number, seen: readonly SeenEntity[]): void {
    const run = this.run;
    if (!run) return;
    if (!run.ended) {
      run.log.push({ kind: "clock", g: gameTime, seen });
      return;
    }
    this.run = undefined;
    if (run.cs) this.settle(run.cs, run.log, gameTime);
  }

  /** An entity named `name` was spawned to or removed from the client. */
  entityChanged(name: string): void {
    if (this.run && !this.run.ended) this.run.log.push({ kind: "change", name });
  }

  /** The run ended and the reading after it answered `after`: judge it. */
  private settle(cs: PlannedCutscene, log: readonly Entry[], after: number): void {
    const last = cs.driverTicks - 1;
    let before: number | undefined;
    for (const e of log) if (e.kind === "clock") before = Math.max(before ?? e.g, e.g);
    if (before === undefined || after !== before + 1) {
      this.spans.push({ cutscene: cs.path, planned: cs.driverTicks, placed: false, read: 0, unplaced: 0, overrun: false });
      return;
    }
    // The end ran in game tick `after`, at driver tick `last`.
    const first = after - last;
    const types = new Set(cs.shots.flatMap((s) => s.bodies.map((b) => bare(b.entity))));
    const anchors: { d: number; at: number; seen: readonly SeenEntity[] }[] = [];
    let overrun = false;
    log.forEach((e, at) => {
      if (e.kind === "clock") {
        const d = e.g - first;
        if (d < -1) overrun = true;
        anchors.push({ d, at, seen: e.seen });
      } else if (e.kind === "end") {
        anchors.push({ d: last, at, seen: e.seen });
      }
    });
    const changedBetween = (from: number, to: number): boolean => {
      for (let i = from + 1; i < to; i++) {
        const e = log[i]!;
        if (e.kind === "change" && types.has(bare(e.name))) return true;
      }
      return false;
    };
    const table: (readonly SeenEntity[] | undefined)[] = [];
    let read = 0;
    for (let d = 0; d <= last; d++) {
      let exact: (typeof anchors)[number] | undefined;
      let prev: (typeof anchors)[number] | undefined;
      let next: (typeof anchors)[number] | undefined;
      for (const a of anchors) {
        if (a.d === d) exact = a;
        else if (a.d < d && (!prev || a.at > prev.at)) prev = a;
        else if (a.d > d && (!next || a.at < next.at)) next = a;
      }
      if (exact) {
        read += 1;
        table.push(exact.seen);
      } else if (prev && next) {
        table.push(changedBetween(prev.at, next.at) ? undefined : prev.seen);
      } else if (next) {
        table.push(changedBetween(-1, next.at) ? undefined : next.seen);
      } else {
        table.push(undefined);
      }
    }
    this.spans.push({
      cutscene: cs.path,
      planned: cs.driverTicks,
      placed: true,
      read,
      unplaced: table.filter((t) => t === undefined).length,
      overrun,
    });
    for (const shot of cs.shots) {
      for (const body of shot.bodies) this.judge(cs, shot, body, table);
    }
  }

  /** One body over its shot, on the per-tick tables. */
  private judge(
    cs: PlannedCutscene,
    shot: PlannedShot,
    body: PlannedBody,
    table: readonly (readonly SeenEntity[] | undefined)[],
  ): void {
    const end = Math.min(shot.startTick + shot.ticks, table.length - 1);
    const at = table[shot.startTick];
    const e = at ? findBody(body, at) : undefined;
    let through = e !== undefined && shot.startTick + shot.ticks <= table.length - 1;
    let unplaced = at === undefined ? 1 : 0;
    for (let t = shot.startTick + 1; t <= end; t++) {
      const seen = table[t];
      if (seen === undefined) unplaced += 1;
      if (!e || !seen || !seen.some((x) => x.id === e.id)) through = false;
    }
    this.records.push({
      cutscene: cs.path,
      shot: shot.index,
      subject: body.subject,
      entity: body.entity,
      spawnedBefore: e !== undefined,
      spawnedThrough: through,
      unplacedTicks: unplaced,
    });
  }
}

/**
 * The failures among the records: every row must read true, and every
 * watched cutscene must be placed on the server's clock and fit its plan.
 */
export function shotSubjectFailures(
  records: readonly BodyRecord[],
  spans: readonly CutsceneSpan[] = [],
): string[] {
  const rows = records
    .filter((r) => !r.spawnedBefore || !r.spawnedThrough)
    .map(
      (r) =>
        `cutscene ${r.cutscene} shot ${r.shot}: ${r.subject} (${r.entity}) ` +
        (r.spawnedBefore
          ? "was spawned to the bot at the shot's first tick and left before the shot ended"
          : "was not spawned to the bot by the shot's first tick") +
        (r.unplacedTicks > 0 ? ` (${r.unplacedTicks} tick(s) of the shot no reading placed)` : ""),
    );
  const spanFailures = spans.flatMap((s) => {
    if (!s.placed) {
      return [
        `cutscene ${s.cutscene}: the readings around its end were not one game tick apart, ` +
          `so the server's clock did not place it and no shot was judged`,
      ];
    }
    return s.overrun
      ? [`cutscene ${s.cutscene}: the server took longer from its start to its end than its ${s.planned} planned driver tick(s)`]
      : [];
  });
  return [...rows, ...spanFailures];
}

/** The one line the run prints, with its denominator. */
export function shotSubjectBindingLine(
  plan: ShotSubjectPlan,
  records: readonly BodyRecord[],
  spans: readonly CutsceneSpan[] = [],
): string {
  const owed = plan.cutscenes.reduce(
    (n, c) => n + c.shots.reduce((m, s) => m + s.bodies.length, 0),
    0,
  );
  const good = records.filter((r) => r.spawnedBefore && r.spawnedThrough).length;
  const placed = spans.filter((s) => s.placed);
  const planned = placed.reduce((n, s) => n + s.planned, 0);
  const read = placed.reduce((n, s) => n + s.read, 0);
  const unplaced = placed.reduce((n, s) => n + s.unplaced, 0);
  return (
    `shot subjects (live): ${records.length} of ${owed} framed body row(s) observed in ` +
    `${plan.cutscenes.length} cutscene(s), ${good} spawned before and through their shot; ` +
    `${placed.length} of ${spans.length} cutscene run(s) placed on the server's clock, ` +
    `${read} of ${planned} driver tick(s) read, ${unplaced} unplaced`
  );
}
