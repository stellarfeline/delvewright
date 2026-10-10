// A body watches (spec-0101 §5.4): the bot's side of the claim, pure.
//
// The compiler records every watching body in `critical-path.json`'s
// `watchers[]` (the rows of `validation/watchers.json`): the selector that
// addresses the body, where it may stand (its summon point and every walk's
// end), whom it watches, and how far it sees. At every waypoint the bot reaches
// within that reach of a body it can draw, it stands, lets a tick pass, and
// asks the server whether the body faces it: `@e[<body>,y_rotation=<yaw±2>]`,
// the yaw the game's own look-at computes from the body's feet to the bot's.
// No step action is added: the assertion rides the walk legs the path has.
//
// This file holds the arithmetic and the ledger; `executor/watch.ts` speaks to
// the server.

import type { Vec3Tuple } from "./critical-path.ts";
import { distSq, nearestInnerCell } from "./route.ts";

/** The tolerance, in degrees, the bot reads a body's yaw within. */
export const WATCH_YAW_TOLERANCE = 2;

/**
 * How far inside `within` the bot must stand before it judges a body. The
 * server measures `distance` from its own positions; the margin keeps the
 * judgement off the boundary, where the two could disagree about one tick.
 */
export const WATCH_REACH_MARGIN = 0.25;

/**
 * The least horizontal distance at which a bearing is judged. Over the body's
 * own column the game's `atan2` has no direction a test could agree with.
 */
export const WATCH_MIN_HORIZONTAL = 1;

/** One watching body, as the compiler records it. */
export interface Watcher {
  /** The body's declared id (`npc/…`, `actor/…`). */
  readonly id: string;
  /** `npc` or `actor`. */
  readonly class: string;
  /** The body's own marker tag. */
  readonly tag: string;
  /** The selector terms that address the body and nothing else. */
  readonly selector: string;
  /** The body's summon feet point. */
  readonly feet: Vec3Tuple;
  /** Every feet point the body can stand at: its summon point and each walk's end. */
  readonly stands: readonly Vec3Tuple[];
  /** The yaw its summon writes. */
  readonly homeYaw: number;
  /** `nearest`, or the class id. */
  readonly who: string;
  /** The tag a player wears for a class watch to draw them; absent for `nearest`. */
  readonly classTag?: string;
  /** How far the body sees, in blocks. */
  readonly within: number;
}

/** `Mth.wrapDegrees`: an angle in `[-180, 180)`. */
export function wrapDegrees(a: number): number {
  let w = a % 360;
  if (w >= 180) w -= 360;
  if (w < -180) w += 360;
  return w + 0;
}

/**
 * The game's yaw for a body at `from` facing a point at `to` (`Entity.lookAt`:
 * `wrapDegrees(atan2(dz, dx) * 57.2957763671875 - 90)`).
 */
export function bearing(from: Vec3Tuple, to: Vec3Tuple): number {
  const dx = to[0] - from[0];
  const dz = to[2] - from[2];
  return wrapDegrees(Math.atan2(dz, dx) * 57.2957763671875 - 90);
}

/** A `y_rotation` range of `±tol` about `yaw`, wrapped as the selector reads it. */
export function yRotationRange(yaw: number, tol: number): string {
  const lo = wrapDegrees(yaw - tol);
  const hi = wrapDegrees(yaw + tol);
  return `${lo.toFixed(2)}..${hi.toFixed(2)}`;
}

/** Feet-to-feet distance, as the selector's `distance` measures it. */
export function feetDistance(a: Vec3Tuple, b: Vec3Tuple): number {
  return Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);
}

/** Horizontal distance. */
export function horizontalDistance(a: Vec3Tuple, b: Vec3Tuple): number {
  return Math.hypot(a[0] - b[0], a[2] - b[2]);
}

/**
 * Whether the bot at `pos` could be within reach of `w` wherever the body
 * stands: the cheap filter that decides whether a waypoint asks the server
 * anything at all.
 */
export function mayReach(w: Watcher, pos: Vec3Tuple): boolean {
  return w.stands.some((s) => feetDistance(s, pos) <= w.within);
}

/** What {@link watchStops} did: the route with its stops, and the binding. */
export interface WatchStops {
  readonly cells: readonly Vec3Tuple[];
  /** Proven cells inserted as stops, each the route's nearest to a watcher's stand. */
  readonly inserted: number;
  /** The watchers the route passes within reach of, at a vertex or a stop. */
  readonly reachable: readonly string[];
}

/**
 * **Stop where the route passes a watcher, not only at its corners.** The
 * compiler draws a watcher from the dense cells it proved the walk over; the
 * exported route keeps only their corners, and the bot judges where it stops.
 * For every stand of every watcher, the route's nearest proven cell
 * ({@link nearestInnerCell}, the shared route rule) is inserted as a stop when
 * no vertex stands in reach of it and that cell does — so a body beside the
 * middle of a straight leg is judged from the middle. `keepWhole(a, b)` names
 * hops a stop must not split (a climb the executor drives as one hop, a hop
 * through a timed gate staged at its mouth). Pure.
 */
export function watchStops(
  route: readonly Vec3Tuple[],
  watchers: readonly Watcher[],
  keepWhole: (a: Vec3Tuple, b: Vec3Tuple) => boolean = () => false,
): WatchStops {
  const reach = (w: Watcher): number => (w.within - WATCH_REACH_MARGIN) ** 2;
  const add = new Map<number, Vec3Tuple[]>();
  const reachable = new Set<string>();
  for (const w of watchers) {
    for (const feet of w.stands) {
      // A route cell is a block; the body stands at its centre.
      const s: Vec3Tuple = [feet[0] - 0.5, feet[1], feet[2] - 0.5];
      if (route.some((v) => distSq(v, s) <= reach(w))) {
        reachable.add(w.id); // a corner already stands in reach: judged there
        continue;
      }
      const inner = nearestInnerCell(s, route, keepWhole);
      if (!inner || inner.distSq > reach(w)) continue;
      reachable.add(w.id);
      const at = add.get(inner.index) ?? [];
      if (!at.some((c) => c[0] === inner.cell[0] && c[1] === inner.cell[1] && c[2] === inner.cell[2])) {
        at.push(inner.cell);
      }
      add.set(inner.index, at);
    }
  }
  const cells: Vec3Tuple[] = [];
  let inserted = 0;
  route.forEach((v, i) => {
    const stops = add.get(i);
    if (stops) {
      // In walking order: along the segment from its start vertex.
      const a = route[i - 1]!;
      stops.sort((p, q) => distSq(p, a) - distSq(q, a));
      cells.push(...stops);
      inserted += stops.length;
    }
    cells.push(v);
  });
  return { cells, inserted, reachable: watchers.filter((w) => reachable.has(w.id)).map((w) => w.id) };
}

/**
 * The position an NBT component rendering `Pos` states, or `undefined` — the
 * LAST bracketed triple of doubles in `line`, since a marker or an entity name
 * may carry brackets of its own. The separator is read with optional
 * whitespace: the pinned server prints a list `[20.5d,67.0d,5.5d]`, and the
 * spaced form is accepted too (the shape `readServerPos` reads).
 */
export function parsePosReply(line: string): Vec3Tuple | undefined {
  const num = String.raw`(-?[\d.]+(?:[eE]-?\d+)?)d`;
  const triple = new RegExp(String.raw`\[\s*${num}\s*,\s*${num}\s*,\s*${num}\s*\]`, "g");
  const m = [...line.matchAll(triple)].at(-1);
  if (!m) return undefined;
  return [Number(m[1]), Number(m[2]), Number(m[3])];
}

/** The first float tag (`-90.0f`) in `text`, as an NBT component renders `Rotation[0]`. */
export function parseYawReply(text: string): number | undefined {
  const m = /(-?[\d.]+(?:E-?\d+)?)f/.exec(text);
  return m ? Number(m[1]) : undefined;
}

/** The tellraw marker the server echoes when a facing check passes. */
export function facingMarker(id: string): string {
  return `[dw:watch ${id} faces]`;
}

/** The tellraw marker the server echoes when the bot wears a class tag. */
export function classMarker(tag: string): string {
  return `[dw:watch wears ${tag}]`;
}

/**
 * What the run found about the record's watchers: which the path came within
 * reach of, which were asserted, and every contradiction.
 */
export class WatchLedger {
  private readonly reached = new Set<string>();
  private readonly asserted = new Set<string>();
  private readonly failed: string[] = [];
  private judgements = 0;
  /**
   * Whether the walk judges at its waypoints: true for the critical path, and
   * closed when it ends — the stages after it walk into deaths and back, and
   * their walks are not the proven path the binding counts.
   */
  armed = true;

  readonly watchers: readonly Watcher[];

  constructor(watchers: readonly Watcher[]) {
    this.watchers = watchers;
  }

  /** The bot stood within reach of `id`, which it can draw. */
  reach(id: string): void {
    this.reached.add(id);
  }

  /** The server confirmed `id` faced the bot. */
  pass(id: string): void {
    this.asserted.add(id);
    this.judgements += 1;
  }

  /** The server denied it. */
  fail(message: string): void {
    this.failed.push(message);
  }

  /** Every contradiction, in the order observed. */
  failures(): readonly string[] {
    return this.failed;
  }

  /** The record's own count, and the two the path earned. */
  binding(): { inRecord: number; withinReach: number; asserted: number; judgements: number } {
    return {
      inRecord: this.watchers.length,
      withinReach: this.reached.size,
      asserted: this.asserted.size,
      judgements: this.judgements,
    };
  }

  /** The binding line (spec-0101 §5.4), with the watchers never reached named. */
  line(): string {
    const b = this.binding();
    const never = this.watchers.filter((w) => !this.reached.has(w.id)).map((w) => w.id);
    return (
      `[watch] ${b.inRecord} watcher(s) in the record, ${b.withinReach} within reach on the ` +
      `proven path, ${b.asserted} asserted (${b.judgements} judgement(s))` +
      (never.length > 0 ? `; never within reach: ${never.join(", ")}` : "")
    );
  }
}
