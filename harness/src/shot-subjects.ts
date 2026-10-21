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
// the table at the end. The start (the gamemode change) is a view too: the
// start function runs in the tick before driver tick 0 and schedules the
// driver one tick on, so it lies before that tick's end. A fact at a tick no
// view landed on is decided by the views on either side when they agree and
// nothing that could change it arrived between them: for a body seen on both
// sides, a spawn or removal of that same entity (by id — the lens the start
// summons is an `item_display` too, and is not the body); for a body seen on
// neither, any entity of its type. Otherwise it is unplaced, and a body is
// not counted there.
//
// What the pinned server guarantees (`docs/reference/entity-tracking.md`):
// a body is sent to a client in the tick its tracker first finds it within
// range of the player and in a chunk the client already holds, and a
// spectating player is moved to the camera and re-checked against every
// tracker each tick. So a body absent at its shot's first tick is a finding
// either way, and the record says which: its chunk had been sent by the tick
// before (the server owed the body), or not (the shot opens before the client
// holds the place it frames).
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

/**
 * What the client holds at one place in the packet stream: every entity it
 * tracks, and which of the framed bodies' chunks it has been sent.
 */
export interface ClientView {
  readonly seen: readonly SeenEntity[];
  /** Keys ({@link chunkKey}) of the observer's `chunks` the client holds. */
  readonly chunks: ReadonlySet<string>;
}

/** The key of the chunk column holding block x/z: `"<cx>,<cz>"`. */
export function chunkKey(x: number, z: number): string {
  return `${Math.floor(x / 16)},${Math.floor(z / 16)}`;
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
  /** Whether the body's chunk had been sent to the bot by the end of the
   * tick before the shot's first tick — the server's precondition for
   * sending the body in that first tick; `null` when no reading decides. */
  readonly chunkSent: boolean | null;
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
  /** Driver ticks at which some framed body's presence no reading decides. */
  readonly unplaced: number;
  /** A reading earlier than the tick before driver tick 0: the server took
   * longer from the start to the end than the plan. */
  readonly overrun: boolean;
}

type Entry =
  | { readonly kind: "start"; readonly view: ClientView }
  | { readonly kind: "clock"; readonly g: number; readonly view: ClientView }
  | { readonly kind: "entity"; readonly id: number; readonly name: string }
  | { readonly kind: "chunk"; readonly key: string }
  | { readonly kind: "end"; readonly view: ClientView };

interface Run {
  /** `null` until the first lens packet names the cutscene; `undefined`
   * when it names none this plan watches. */
  cs: PlannedCutscene | undefined | null;
  readonly log: Entry[];
  ended: boolean;
}

/** A place in the log whose view is known, at driver tick `d`. */
interface Anchor {
  readonly d: number;
  readonly at: number;
  readonly view: ClientView;
}

type Presence =
  | { readonly kind: "present"; readonly id: number }
  | { readonly kind: "absent" }
  | { readonly kind: "undecided" };

/**
 * The start's place on the driver clock: inside the game tick before driver
 * tick 0, before that tick ends. The start function switches the gamemode
 * and schedules the driver one tick on, so the gamemode packet precedes the
 * end of that tick; a view there decides the end of the tick only together
 * with a later view and nothing changed between.
 */
const START_D = -1.5;

/**
 * Watches the client during cutscenes, on the server's clock. Fed, in the
 * order the packets arrive: every gamemode change, the lens packets (the
 * first names the cutscene by where the bot stands), every game-time reading
 * answered, every entity spawned to or removed from the client, and every
 * framed body's chunk column the client is sent or told to forget.
 */
export class ShotSubjectObserver {
  readonly records: BodyRecord[] = [];
  readonly spans: CutsceneSpan[] = [];
  /** Every chunk column a framed body stands in, by {@link chunkKey}. */
  readonly chunks: readonly string[];
  private readonly chunkSet: ReadonlySet<string>;
  private run: Run | undefined;

  private readonly plan: ShotSubjectPlan;

  constructor(plan: ShotSubjectPlan) {
    this.plan = plan;
    const keys = new Set<string>();
    for (const c of plan.cutscenes) {
      for (const s of c.shots) for (const b of s.bodies) keys.add(chunkKey(b.at[0], b.at[2]));
    }
    this.chunks = [...keys].sort();
    this.chunkSet = keys;
  }

  /** Whether a cutscene is playing or awaits the reading that places its end. */
  get watching(): boolean {
    return this.run !== undefined;
  }

  /** The gamemode changed: spectator begins a run, leaving it is the end. */
  gameModeChanged(gameMode: string | undefined, view: ClientView): void {
    if (gameMode === "spectator") {
      if (!this.run) this.run = { cs: null, log: [{ kind: "start", view }], ended: false };
      return;
    }
    const run = this.run;
    if (!run || run.ended) return;
    if (!run.cs) {
      this.run = undefined;
      return;
    }
    run.log.push({ kind: "end", view });
    run.ended = true;
  }

  /** The server aimed the client at a lens entity (not at itself). */
  lensTick(gameMode: string | undefined, pos: Vec3): void {
    if (gameMode !== "spectator" || !this.run || this.run.cs !== null) return;
    this.run.cs = cutsceneAt(this.plan, pos);
  }

  /** A game-time reading answered at `gameTime`, with the view it found. */
  clock(gameTime: number, view: ClientView): void {
    const run = this.run;
    if (!run) return;
    if (!run.ended) {
      run.log.push({ kind: "clock", g: gameTime, view });
      return;
    }
    this.run = undefined;
    if (run.cs) this.settle(run.cs, run.log, gameTime);
  }

  /** The entity `id`, of type `name`, was spawned to or removed from the client. */
  entityChanged(id: number, name: string): void {
    if (this.run && !this.run.ended) this.run.log.push({ kind: "entity", id, name });
  }

  /** The chunk column `key` ({@link chunkKey}) was sent to or dropped by the client. */
  chunkChanged(key: string): void {
    if (!this.chunkSet.has(key)) return;
    if (this.run && !this.run.ended) this.run.log.push({ kind: "chunk", key });
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
    const anchors: Anchor[] = [];
    let overrun = false;
    log.forEach((e, at) => {
      if (e.kind === "start") {
        anchors.push({ d: START_D, at, view: e.view });
      } else if (e.kind === "clock") {
        const d = e.g - first;
        if (d < -1) overrun = true;
        anchors.push({ d, at, view: e.view });
      } else if (e.kind === "end") {
        anchors.push({ d: last, at, view: e.view });
      }
    });
    const judge = new TickJudge(log, anchors);
    let read = 0;
    for (let d = 0; d <= last; d++) if (judge.exact(d)) read += 1;
    const undecided = new Set<number>();
    for (const shot of cs.shots) {
      for (const body of shot.bodies) this.judge(cs, shot, body, last, judge, undecided);
    }
    this.spans.push({
      cutscene: cs.path,
      planned: cs.driverTicks,
      placed: true,
      read,
      unplaced: undecided.size,
      overrun,
    });
  }

  /** One body over its shot, tick by tick on the server's clock. */
  private judge(
    cs: PlannedCutscene,
    shot: PlannedShot,
    body: PlannedBody,
    last: number,
    judge: TickJudge,
    undecided: Set<number>,
  ): void {
    const end = Math.min(shot.startTick + shot.ticks, last);
    const at = judge.presence(body, shot.startTick);
    const id = at.kind === "present" ? at.id : undefined;
    let through = id !== undefined && shot.startTick + shot.ticks <= last;
    let unplaced = 0;
    for (let t = shot.startTick; t <= end; t++) {
      const p = t === shot.startTick ? at : judge.presence(body, t);
      if (p.kind === "undecided") {
        unplaced += 1;
        undecided.add(t);
      }
      if (p.kind !== "present" || p.id !== id) through = false;
    }
    this.records.push({
      cutscene: cs.path,
      shot: shot.index,
      subject: body.subject,
      entity: body.entity,
      spawnedBefore: id !== undefined,
      spawnedThrough: through,
      unplacedTicks: unplaced,
      chunkSent: judge.chunkHeld(chunkKey(body.at[0], body.at[2]), shot.startTick - 1),
    });
  }
}

/**
 * Decides one fact at the end of one driver tick from the views around it.
 * A view taken at that tick decides it. Otherwise the views on either side
 * decide it when they agree and nothing that could change the fact arrived
 * between them: for a body seen on both sides, a spawn or removal of that
 * same entity; for a body seen on neither, a spawn or removal of any entity
 * of its type; for a chunk, a load or unload of that chunk. Anything else is
 * undecided.
 */
class TickJudge {
  private readonly log: readonly Entry[];
  private readonly anchors: readonly Anchor[];

  constructor(log: readonly Entry[], anchors: readonly Anchor[]) {
    this.log = log;
    this.anchors = anchors;
  }

  exact(d: number): Anchor | undefined {
    let exact: Anchor | undefined;
    for (const a of this.anchors) if (a.d === d) exact = a;
    return exact;
  }

  private around(d: number): { prev: Anchor | undefined; next: Anchor | undefined } {
    let prev: Anchor | undefined;
    let next: Anchor | undefined;
    for (const a of this.anchors) {
      if (a.d < d && (!prev || a.at > prev.at)) prev = a;
      else if (a.d > d && (!next || a.at < next.at)) next = a;
    }
    return { prev, next };
  }

  private arrivedBetween(from: number, to: number, hit: (e: Entry) => boolean): boolean {
    for (let i = from + 1; i < to; i++) if (hit(this.log[i]!)) return true;
    return false;
  }

  presence(body: PlannedBody, d: number): Presence {
    const exact = this.exact(d);
    if (exact) {
      const e = findBody(body, exact.view.seen);
      return e ? { kind: "present", id: e.id } : { kind: "absent" };
    }
    const { prev, next } = this.around(d);
    if (!prev || !next) return { kind: "undecided" };
    const a = findBody(body, prev.view.seen);
    const b = findBody(body, next.view.seen);
    if (a && b && a.id === b.id) {
      const moved = this.arrivedBetween(prev.at, next.at, (e) => e.kind === "entity" && e.id === a.id);
      return moved ? { kind: "undecided" } : { kind: "present", id: a.id };
    }
    if (!a && !b) {
      const type = bare(body.entity);
      const moved = this.arrivedBetween(prev.at, next.at, (e) => e.kind === "entity" && bare(e.name) === type);
      return moved ? { kind: "undecided" } : { kind: "absent" };
    }
    return { kind: "undecided" };
  }

  chunkHeld(key: string, d: number): boolean | null {
    const exact = this.exact(d);
    if (exact) return exact.view.chunks.has(key);
    const { prev, next } = this.around(d);
    if (!prev || !next) return null;
    const a = prev.view.chunks.has(key);
    if (a !== next.view.chunks.has(key)) return null;
    return this.arrivedBetween(prev.at, next.at, (e) => e.kind === "chunk" && e.key === key) ? null : a;
  }
}

/** Why a body absent at its shot's first tick is a finding, from its chunk. */
function notSpawnedWhy(chunkSent: boolean | null): string {
  if (chunkSent === true) return ", though its chunk had been sent to the bot by the tick before";
  if (chunkSent === false) {
    return (
      ", and its chunk had not been sent to the bot by the tick before: the server sends no entity " +
      "in a chunk the client does not hold, so the shot opens before the client has the place it frames"
    );
  }
  return ", and no reading decides whether its chunk had been sent to the bot by the tick before";
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
          : "was not spawned to the bot by the shot's first tick" + notSpawnedWhy(r.chunkSent)) +
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
