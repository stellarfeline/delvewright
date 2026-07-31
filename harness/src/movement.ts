// Sneak-aware leg configuration for the pathfinder (spec-0008 gap-7 subset).
// A critical-path step may be marked `sneak: true` — the stealth finale walks that
// leg crouched so the bot stays inside the safe (quiet) envelope a live warden's
// vibration senses gate on. This helper centralises the movement/control-state
// setup so it is pure enough to unit-test the flag plumbing without a server.

/** The subset of a mineflayer Bot this helper drives. */
export interface ControlBot {
  setControlState(control: "sneak", state: boolean): void;
}

/**
 * The subset of a mineflayer-pathfinder `Movements` this helper tunes. `canDig`
 * and `allow1by1towers` are always locked off (adventure mode: never break or
 * pillar); `allowSprinting` is turned off only for a sneak leg.
 */
export interface LegMovements {
  canDig: boolean;
  allow1by1towers: boolean;
  allowSprinting: boolean;
}

/** The subset of `Movements` {@link tuneCaveMovements} touches for the cave block mix. */
export interface CaveMovements {
  allowEntityDetection: boolean;
}

/**
 * Belt-and-braces tuning for the cave block mix (task #38). The bot follows a
 * compiler-proven, ground-hugging waypoint polyline (the DW0311 route treats water
 * and fences as impassable, so the proven path already hugs dry standable ground
 * and never routes through a pen), so the remaining risk is the pathfinder reacting
 * to entities near the corridor — penned mobs beside the route. Disabling entity
 * detection keeps each leg's A* deterministic and undistracted; the delve world is
 * sealed, so nothing living blocks the proven path. Liquids, stairs, fences and
 * gravity blocks are already classified from mcdata by the pathfinder, so their
 * default handling needs no override. The adventure-mode block locks (`canDig`,
 * `allow1by1towers`) are applied separately by {@link configureLeg}.
 */
export function tuneCaveMovements(movements: CaveMovements): void {
  movements.allowEntityDetection = false;
}

/**
 * Apply the standard adventure-mode movement locks, and — for a sneak leg — disable
 * sprinting in the Movements and turn the bot's sneak control state ON. Returns a
 * restore function that clears the sneak control state again; the caller MUST invoke
 * it (in a `finally`) so a later non-sneak leg is not left crouched. A non-sneak leg
 * returns a no-op restore.
 */
export function configureLeg(
  bot: ControlBot,
  movements: LegMovements,
  sneak: boolean,
): () => void {
  movements.canDig = false; // adventure mode: never break blocks
  movements.allow1by1towers = false;
  if (!sneak) {
    return () => {};
  }
  movements.allowSprinting = false; // a sprinting bot is not sneaking
  bot.setControlState("sneak", true);
  return () => bot.setControlState("sneak", false);
}
