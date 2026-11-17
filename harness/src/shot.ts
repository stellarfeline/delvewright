// spec-0134 §2/§3: the bot's shot and its raised item — the pure half.

/**
 * An arrow's flight in the pinned version, read from the jar
 * (`docs/reference/vehicles-animals-and-targets.md` §14): a full draw launches it at
 * `ARROW_LAUNCH_SPEED` blocks a tick along the look, from the shooter's eye
 * less `ARROW_LAUNCH_DROP`; each tick it moves by its velocity, then the
 * velocity is scaled by `ARROW_DRAG` and loses `ARROW_GRAVITY` downward.
 */
export const ARROW_LAUNCH_SPEED = 3.0;
/** The scale applied to the velocity after each move, in air. */
export const ARROW_DRAG = 0.99;
/** Blocks a tick taken from the vertical velocity after the drag. */
export const ARROW_GRAVITY = 0.05;
/** The arrow spawns this far under the shooter's eye (`0.1F`, widened). */
export const ARROW_LAUNCH_DROP = 0.10000000149011612;
/** The ticks a flight is followed before it is given up as out of reach. */
const FLIGHT_TICKS = 200;

/**
 * How many arrows the bot looses for one hit before the step fails. The
 * server scatters every arrow (each component of its unit direction moved by
 * a triangular draw of half-width 0.0172275, the record §14), so an arc
 * through the centre is a chance per arrow, not a certainty. On the town's
 * belfry — a target in a wall, a top slab under it and a stone column beside
 * it — the model gives 44 to 47 % per arrow from anywhere within 0.3 of the
 * stance's centre (the record §14); twenty leave at most one hit in a
 * hundred thousand unmade (0.56^20).
 */
export const SHOT_TRIES = 20;

/**
 * The game ticks a bow is drawn for a full charge (`BowItem.getPowerForTime`:
 * power `(f² + 2f) / 3` of `f = ticks / 20`, capped at 1).
 */
export const BOW_FULL_DRAW_TICKS = 20;

/**
 * The game ticks the bot holds the bow drawn, read from the server's game time
 * between the draw and the release: one past a full charge. A draw timed by the
 * wall clock is short whenever the server falls behind — on a server running
 * fifteen ticks a second a 1.2 s draw is 18 ticks, power 0.87, an arrow at 2.6
 * blocks a tick that falls short of the arc it was aimed on.
 */
export const SHOT_DRAW_TICKS = BOW_FULL_DRAW_TICKS + 1;

/** How long the bot holds a raised item in use (spec-0134 §3). */
export const USE_ITEM_HOLD_MS = 1000;

/** How long the bot waits for a loosed arrow's hit to be recorded. */
export const SHOT_FLIGHT_MS = 2500;

/**
 * The height over its launch point at which an arrow loosed at `pitch`
 * radians above level crosses `distance` blocks of horizontal travel;
 * `undefined` when the flight window ends first.
 */
export function arrowHeightAt(pitch: number, distance: number): number | undefined {
  let vx = ARROW_LAUNCH_SPEED * Math.cos(pitch);
  let vy = ARROW_LAUNCH_SPEED * Math.sin(pitch);
  let x = 0;
  let y = 0;
  for (let n = 0; n < FLIGHT_TICKS; n++) {
    if (x + vx >= distance) {
      const f = vx === 0 ? 0 : (distance - x) / vx;
      return y + f * vy;
    }
    x += vx;
    y += vy;
    vx *= ARROW_DRAG;
    vy = vy * ARROW_DRAG - ARROW_GRAVITY;
  }
  return undefined;
}

function heightOr(pitch: number, distance: number): number {
  return arrowHeightAt(pitch, distance) ?? Number.NEGATIVE_INFINITY;
}

const PITCH_LO = -Math.PI / 2 + 1e-3;
const PITCH_HI = Math.PI / 2 - 1e-3;

/**
 * The pitch (radians above level) of the low arc that carries a full-drawn
 * arrow `distance` blocks out and `rise` blocks up from its launch point;
 * `undefined` when no arc reaches that point.
 */
export function shotPitch(distance: number, rise: number): number | undefined {
  // The crossing height climbs with the pitch up to the arc of farthest
  // reach, then falls: find that peak, then bisect the climbing side.
  let lo = PITCH_LO;
  let hi = PITCH_HI;
  for (let i = 0; i < 100; i++) {
    const a = lo + (hi - lo) / 3;
    const b = hi - (hi - lo) / 3;
    if (heightOr(a, distance) < heightOr(b, distance)) lo = a;
    else hi = b;
  }
  const peak = (lo + hi) / 2;
  if (heightOr(peak, distance) < rise) return undefined;
  lo = PITCH_LO;
  hi = peak;
  if (heightOr(lo, distance) >= rise) return lo;
  for (let i = 0; i < 100; i++) {
    const mid = (lo + hi) / 2;
    if (heightOr(mid, distance) < rise) lo = mid;
    else hi = mid;
  }
  return (lo + hi) / 2;
}

/**
 * The flight of a full-drawn arrow loosed from `eye` on the low arc through
 * `point`, with no spread: its position at launch and after each tick, up to
 * the first tick at or past `point`'s column. `undefined` when no arc reaches
 * `point`, or it stands straight over or under the eye. The compiler's shot
 * proof follows the same flight (`delvewright_dsl::metrics::arrow::flight`),
 * held to this one by the case file `test/shot.test.ts` writes.
 */
export function arrowFlight(
  eye: readonly [number, number, number],
  point: readonly [number, number, number],
): [number, number, number][] | undefined {
  const launch: [number, number, number] = [eye[0], eye[1] - ARROW_LAUNCH_DROP, eye[2]];
  const dx = point[0] - launch[0];
  const dz = point[2] - launch[2];
  const d = Math.hypot(dx, dz);
  if (d === 0) return undefined;
  const pitch = shotPitch(d, point[1] - launch[1]);
  if (pitch === undefined) return undefined;
  const [ux, uz] = [dx / d, dz / d];
  let vh = ARROW_LAUNCH_SPEED * Math.cos(pitch);
  let vy = ARROW_LAUNCH_SPEED * Math.sin(pitch);
  let p = launch;
  const out: [number, number, number][] = [p];
  let run = 0;
  for (let n = 0; n < FLIGHT_TICKS; n++) {
    p = [p[0] + vh * ux, p[1] + vy, p[2] + vh * uz];
    run += vh;
    out.push(p);
    if (run >= d) return out;
    vh *= ARROW_DRAG;
    vy = vy * ARROW_DRAG - ARROW_GRAVITY;
  }
  return undefined;
}

/**
 * The point the bot looks at from `eye` so that a full-drawn arrow's flight
 * passes through the centre of the target block `target`: one block out along
 * the low arc's launch direction. `undefined` when no arc reaches the centre.
 * Every arrow is aimed the same way; the spread is the server's, drawn afresh
 * for each.
 */
export function shotAim(
  eye: readonly [number, number, number],
  target: readonly [number, number, number],
): [number, number, number] | undefined {
  const c: [number, number, number] = [target[0] + 0.5, target[1] + 0.5, target[2] + 0.5];
  const dx = c[0] - eye[0];
  const dz = c[2] - eye[2];
  const d = Math.hypot(dx, dz);
  if (d === 0) return undefined;
  const pitch = shotPitch(d, c[1] - (eye[1] - ARROW_LAUNCH_DROP));
  if (pitch === undefined) return undefined;
  return [eye[0] + dx / d, eye[1] + Math.tan(pitch), eye[2] + dz / d];
}

/** The failure a shot step ends with when no full-drawn arc reaches the target. */
export function shotOutOfReach(trigger: string): string {
  return `${trigger}: no full-drawn arrow's arc reaches the target's centre from where the bot stands`;
}

/** The failure a shot step ends with once its arrows are spent. */
export function shotSpent(trigger: string, arrows: number): string {
  return `${trigger}: no arrow of ${arrows} landed on the target, each aimed on the arc through its centre`;
}
