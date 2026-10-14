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
    arr(cs["shots"], `/cutscenes/${i}/shots`).forEach((s, j) => {
      const at = `/cutscenes/${i}/shots/${j}`;
      const shot = obj(s, at);
      if (typeof shot["index"] !== "number") bad(`${at}/index is not a number`);
      if (typeof shot["start_tick"] !== "number") bad(`${at}/start_tick is not a number`);
      if (typeof shot["ticks"] !== "number") bad(`${at}/ticks is not a number`);
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

/**
 * Watches the client during cutscenes: fed once per client tick with the
 * gamemode, the bot's position and the entities it tracks.
 */
export class ShotSubjectObserver {
  readonly records: BodyRecord[] = [];
  private active: { cs: PlannedCutscene; tick: number } | undefined;
  private seenIds = new Map<string, number | undefined>();
  private through = new Map<string, boolean>();
  private done = new Set<string>();

  private readonly plan: ShotSubjectPlan;

  constructor(plan: ShotSubjectPlan) {
    this.plan = plan;
  }

  /** One client tick. */
  tick(gameMode: string | undefined, pos: Vec3, seen: readonly SeenEntity[]): void {
    if (gameMode !== "spectator") {
      if (this.active) this.finish();
      return;
    }
    if (!this.active) {
      const cs = cutsceneAt(this.plan, pos);
      if (!cs) return;
      this.active = { cs, tick: 0 };
    } else {
      this.active.tick += 1;
    }
    const { cs, tick } = this.active;
    for (const shot of cs.shots) {
      if (tick < shot.startTick || tick > shot.startTick + shot.ticks) continue;
      for (const body of shot.bodies) {
        const key = `${cs.path}#${shot.index}#${body.field}`;
        if (tick === shot.startTick) {
          const e = findBody(body, seen);
          this.seenIds.set(key, e?.id);
          this.through.set(key, e !== undefined);
        } else if (this.seenIds.has(key)) {
          const id = this.seenIds.get(key);
          if (id === undefined || !seen.some((e) => e.id === id)) this.through.set(key, false);
        }
        if (tick === shot.startTick + shot.ticks) this.record(cs, shot, body, key);
      }
    }
  }

  private record(cs: PlannedCutscene, shot: PlannedShot, body: PlannedBody, key: string): void {
    if (this.done.has(key) || !this.seenIds.has(key)) return;
    this.done.add(key);
    this.records.push({
      cutscene: cs.path,
      shot: shot.index,
      subject: body.subject,
      entity: body.entity,
      spawnedBefore: this.seenIds.get(key) !== undefined,
      spawnedThrough: this.through.get(key) === true,
    });
  }

  /** The cutscene ended: record every shot it started and did not finish. */
  private finish(): void {
    const a = this.active;
    this.active = undefined;
    if (!a) return;
    for (const shot of a.cs.shots) {
      for (const body of shot.bodies) {
        const key = `${a.cs.path}#${shot.index}#${body.field}`;
        if (this.seenIds.has(key) && !this.done.has(key)) {
          this.through.set(key, false);
          this.record(a.cs, shot, body, key);
        }
      }
    }
  }
}

/** The failures among the records: every row must read true. */
export function shotSubjectFailures(records: readonly BodyRecord[]): string[] {
  return records
    .filter((r) => !r.spawnedBefore || !r.spawnedThrough)
    .map(
      (r) =>
        `cutscene ${r.cutscene} shot ${r.shot}: ${r.subject} (${r.entity}) ` +
        (r.spawnedBefore
          ? "was spawned to the bot at the shot's first tick and left before the shot ended"
          : "was not spawned to the bot by the shot's first tick"),
    );
}

/** The one line the run prints, with its denominator. */
export function shotSubjectBindingLine(plan: ShotSubjectPlan, records: readonly BodyRecord[]): string {
  const owed = plan.cutscenes.reduce(
    (n, c) => n + c.shots.reduce((m, s) => m + s.bodies.length, 0),
    0,
  );
  const good = records.filter((r) => r.spawnedBefore && r.spawnedThrough).length;
  return (
    `shot subjects (live): ${records.length} of ${owed} framed body row(s) observed in ` +
    `${plan.cutscenes.length} cutscene(s), ${good} spawned before and through their shot`
  );
}
