// spec-0134 §2/§3: the bot's shot and its raised item — the pure half.

/**
 * The elevations the bot aims a full-drawn bow over the target's centre, in
 * blocks per 27 blocks of distance, tried in order. The vanilla record
 * (`docs/reference/vehicles-animals-and-targets.md` §1) measured both shots
 * striking at 1.5, 2.0 and 2.5 blocks over the centre at 27 blocks, and none at
 * 0, 1, 3 or 4; the middle is tried first, then either side.
 */
export const SHOT_ELEVATIONS_AT_27 = [2.0, 1.5, 2.5] as const;

/** The distance the record's elevations were measured at. */
export const SHOT_RECORD_DISTANCE = 27;

/** How long the bot holds the bow drawn: past a full charge (20 ticks). */
export const SHOT_DRAW_MS = 1200;

/** How long the bot holds a raised item in use (spec-0134 §3). */
export const USE_ITEM_HOLD_MS = 1000;

/** How long the bot waits for a loosed arrow's hit to be recorded. */
export const SHOT_FLIGHT_MS = 2500;

/**
 * The point the bot aims at for attempt `k` (0-based) from `eye` at the
 * target block `target`: its centre raised by the table's elevation, scaled
 * linearly with horizontal distance. `undefined` once the table is spent.
 */
export function shotAim(
  eye: readonly [number, number, number],
  target: readonly [number, number, number],
  k: number,
): [number, number, number] | undefined {
  const e = SHOT_ELEVATIONS_AT_27[k];
  if (e === undefined) return undefined;
  const c: [number, number, number] = [target[0] + 0.5, target[1] + 0.5, target[2] + 0.5];
  const d = Math.hypot(c[0] - eye[0], c[2] - eye[2]);
  return [c[0], c[1] + (e * d) / SHOT_RECORD_DISTANCE, c[2]];
}

/** The failure a shot step ends with once the table is spent. */
export function shotSpent(trigger: string, arrows: number): string {
  return (
    `${trigger}: no arrow of ${arrows} landed on the target — the aim table ` +
    `(${SHOT_ELEVATIONS_AT_27.join(", ")} over the centre at ${SHOT_RECORD_DISTANCE} blocks) is spent`
  );
}
