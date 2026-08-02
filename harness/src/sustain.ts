// Eating: the other half of playing the delve the way a player does.
//
// Every class kit in a souls campaign grants food (the-drowned-bell hands each class
// rabbit stew, and its blurbs are written around it — "the stew in your pack is the
// whole of your mercy toward yourself"). The critical-path bot carried that stew
// through every fight and never once ate it, so damage taken early in a route was
// still missing when the next fight started. Content tuned around a player who eats
// was being validated by a bot that starves.
//
// The rules live here, pure and clock-free, so they are unit-testable: the executor
// only supplies the numbers and performs the resulting action.

/**
 * Eat below this fraction of max health. 60% is the point where one more hit from an
 * ordinary delve mob is a real risk but the bot is not yet in the emergency where
 * standing still for the 1.6s eat animation is worse than the damage.
 */
export const EAT_HEALTH_FRACTION = 0.6;

/**
 * How near (blocks) a hostile makes eating a bad idea. Eating locks the bot in place
 * for the food's use duration; doing that with a mob in reach donates free hits. This
 * is the "no hostile within melee range" precondition, in blocks.
 */
export const EAT_SAFE_RANGE = 4;

/** Minimum gap (ms) between eat ATTEMPTS, so a blocked reason cannot spam the log. */
export const EAT_COOLDOWN_MS = 3_000;

/** Why the bot did (or did not) eat — logged verbatim, so a run's log explains itself. */
export type EatDecision =
  | "eat"
  | "healthy"
  | "hostile-near"
  | "hunger-full"
  | "no-food";

/**
 * Decide whether to eat right now.
 *
 * `hunger-full` is not a failure and not a tolerance: vanilla forbids eating ordinary
 * food at 20/20 hunger (mineflayer's `bot.consume` throws "Food is full"), and at that
 * hunger natural regeneration is already running. Reporting it distinctly is what tells
 * a reader of the log "the bot is low but healing", not "the bot ignored its stew".
 */
export function eatDecision(opts: {
  readonly health: number;
  readonly maxHealth: number;
  readonly food: number;
  readonly maxFood: number;
  readonly nearestHostileDistance: number | undefined;
  readonly hasFood: boolean;
}): EatDecision {
  if (opts.health > opts.maxHealth * EAT_HEALTH_FRACTION) return "healthy";
  if (!opts.hasFood) return "no-food";
  if (
    opts.nearestHostileDistance !== undefined &&
    opts.nearestHostileDistance <= EAT_SAFE_RANGE
  ) {
    return "hostile-near";
  }
  if (opts.food >= opts.maxFood) return "hunger-full";
  return "eat";
}

/** An inventory item reduced to what the food rule needs. */
export interface FoodItem {
  readonly name: string;
  /** Hunger points restored, from the pinned minecraft-data food registry. */
  readonly foodPoints: number;
}

/**
 * Pick which food to eat: the most nourishing one, ties broken by name so the choice
 * is deterministic for a given inventory (ADR-0006 spirit — the harness is not a
 * source of run-to-run variance).
 */
export function pickFood<T extends FoodItem>(items: readonly T[]): T | undefined {
  let best: T | undefined;
  for (const item of items) {
    if (
      !best ||
      item.foodPoints > best.foodPoints ||
      (item.foodPoints === best.foodPoints && item.name < best.name)
    ) {
      best = item;
    }
  }
  return best;
}
