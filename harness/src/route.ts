// A proven leg's route, read as the compiler proved it (pure).
//
// The compiler proves a leg over dense route cells and exports them thinned to
// their corners (`waypoints::thin`): between two consecutive exported cells the
// route is ONE straight run of equal steps, so every route cell lies on the
// segment between them. A judgement the compiler made from the dense cells — a
// sensor in earshot, a body in reach, a station on the leg — is read against
// that polyline, never against its vertices alone: a straight leg keeps two
// vertices however long it is, and a point beside its middle is far from both.
// This file is the one rule every such reading calls.

import type { Vec3Tuple } from "./critical-path.ts";

/** Integer-cell squared distance. */
export function distSq(a: Vec3Tuple, b: Vec3Tuple): number {
  return (a[0] - b[0]) ** 2 + (a[1] - b[1]) ** 2 + (a[2] - b[2]) ** 2;
}

/** Where on a route the point nearest some `p` lies. */
export interface RoutePosition {
  /** The segment `route[segment] → route[segment + 1]` it lies on (`0` for a one-point route). */
  readonly segment: number;
  /** How far along that segment, in `[0, 1]`. */
  readonly t: number;
  /** The point itself. */
  readonly point: Vec3Tuple;
  /** Its squared distance to `p`. */
  readonly distSq: number;
}

/**
 * The point of the polyline through `route` nearest `p`, ties to the earlier
 * segment; `undefined` for an empty route.
 */
export function nearestOnRoute(p: Vec3Tuple, route: readonly Vec3Tuple[]): RoutePosition | undefined {
  if (route.length === 0) return undefined;
  let best: RoutePosition = { segment: 0, t: 0, point: route[0]!, distSq: distSq(p, route[0]!) };
  for (let i = 1; i < route.length; i += 1) {
    const a = route[i - 1]!;
    const b = route[i]!;
    const ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]] as const;
    const len = ab[0] ** 2 + ab[1] ** 2 + ab[2] ** 2;
    const t =
      len === 0
        ? 0
        : Math.min(
            1,
            Math.max(0, ((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1] + (p[2] - a[2]) * ab[2]) / len),
          );
    const point: Vec3Tuple = [a[0] + t * ab[0], a[1] + t * ab[1], a[2] + t * ab[2]];
    const d = (point[0] - p[0]) ** 2 + (point[1] - p[1]) ** 2 + (point[2] - p[2]) ** 2;
    if (d < best.distSq) best = { segment: i - 1, t, point, distSq: d };
  }
  return best;
}

/** Squared distance from `p` to the polyline through `route`; `Infinity` for an empty route. */
export function distSqToRoute(p: Vec3Tuple, route: readonly Vec3Tuple[]): number {
  return nearestOnRoute(p, route)?.distSq ?? Infinity;
}

/**
 * The index of the first vertex of `route` the walk reaches at or after the
 * point nearest `p`: a walk that stops there and resumes from this index
 * neither skips the rest of `p`'s segment nor walks back over it. `0` for an
 * empty route.
 */
export function routeIndexAfter(p: Vec3Tuple, route: readonly Vec3Tuple[]): number {
  const at = nearestOnRoute(p, route);
  if (!at) return 0;
  return at.t > 0 ? at.segment + 1 : at.segment;
}

/**
 * The unit step of the straight run `a → b`, or `undefined` when the pair is
 * not one: a run of `n` equal steps has each component of `b − a` either 0 or
 * `±n`, and only then is every `a + k·step` a cell the compiler proved.
 */
export function straightRunStep(a: Vec3Tuple, b: Vec3Tuple): Vec3Tuple | undefined {
  const d: Vec3Tuple = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
  const n = Math.max(Math.abs(d[0]), Math.abs(d[1]), Math.abs(d[2]));
  if (n === 0 || !d.every((c) => c === 0 || Math.abs(c) === n)) return undefined;
  return [d[0] / n, d[1] / n, d[2] / n];
}

/**
 * The proven cell of the route nearest `p`, strictly inside a straight-run
 * segment, with the index it would be inserted at; `undefined` when the
 * nearest such cell is a vertex already, or its segment is not one straight
 * run (its inner cells are then unknown). `keepWhole(a, b)` names segments
 * that must not be split. Ties go to the earlier segment and the earlier cell.
 */
export function nearestInnerCell(
  p: Vec3Tuple,
  route: readonly Vec3Tuple[],
  keepWhole: (a: Vec3Tuple, b: Vec3Tuple) => boolean = () => false,
): { readonly cell: Vec3Tuple; readonly index: number; readonly distSq: number } | undefined {
  let best: { cell: Vec3Tuple; index: number; distSq: number } | undefined;
  let bestVertex = Infinity;
  for (const v of route) bestVertex = Math.min(bestVertex, distSq(p, v));
  for (let i = 1; i < route.length; i += 1) {
    const a = route[i - 1]!;
    const b = route[i]!;
    const step = straightRunStep(a, b);
    if (!step || keepWhole(a, b)) continue;
    const n = Math.max(Math.abs(b[0] - a[0]), Math.abs(b[1] - a[1]), Math.abs(b[2] - a[2]));
    for (let k = 1; k < n; k += 1) {
      const cell: Vec3Tuple = [a[0] + step[0] * k, a[1] + step[1] * k, a[2] + step[2] * k];
      const d = distSq(p, cell);
      if (d < bestVertex && (!best || d < best.distSq)) best = { cell, index: i, distSq: d };
    }
  }
  return best;
}
