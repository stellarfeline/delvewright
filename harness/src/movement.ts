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
 * pillar); `canOpenDoors` is turned ON (opening a door/fence-gate is a use
 * interaction vanilla permits in adventure mode — see {@link configureLeg});
 * `allowSprinting` is turned off only for a sneak leg.
 */
export interface LegMovements {
  canDig: boolean;
  allow1by1towers: boolean;
  canOpenDoors: boolean;
  allowSprinting: boolean;
}

/**
 * Vanilla 1.21.11 entity types whose bounding box does NOT collide with a player —
 * display/marker/interaction entities and the area-effect cloud. Physically they
 * obstruct nothing: a player walks straight through an `interaction` hitbox, a
 * floating `item_display`/`text_display` marker, or a zero-size `marker`. The
 * pathfinder must therefore route THROUGH them, not around them. This is
 * model-alignment with game physics, NOT a check being weakened: a solid obstacle
 * would still block the bot. Genuinely solid or pushing entities (mobs, mannequin
 * NPCs, armor stands, boats, minecarts) are deliberately absent from this list and
 * keep the pathfinder's default avoidance.
 *
 * Names are mineflayer `entity.name` values (no `minecraft:` prefix), matching the
 * keys mineflayer-pathfinder's `Movements.passableEntities` set is checked against.
 */
export const NON_COLLIDING_ENTITY_TYPES: readonly string[] = [
  "interaction",
  "item_display",
  "text_display",
  "block_display",
  "marker",
  "area_effect_cloud",
];

/** The subset of a mineflayer-pathfinder `Movements` that lists entity types the
 * pathfinder is allowed to path through without avoidance cost. */
export interface PassableMovements {
  passableEntities: Set<string>;
}

/**
 * Mark every {@link NON_COLLIDING_ENTITY_TYPES} entity passable on a pathfinder
 * `Movements`, so the bot no longer wedges against a non-colliding `interaction`
 * hitbox or a floating `item_display` marker that leaked from a completed objective
 * (or from any co-located NPC hitbox). Additive — it never removes an entity the
 * pathfinder already avoids, so real threats stay avoided.
 */
export function allowNonCollidingEntities(movements: PassableMovements): void {
  for (const name of NON_COLLIDING_ENTITY_TYPES) {
    movements.passableEntities.add(name);
  }
}

/**
 * Apply the standard adventure-mode movement locks, and — for a sneak leg — disable
 * sprinting in the Movements and turn the bot's sneak control state ON. Returns a
 * restore function that clears the sneak control state again; the caller MUST invoke
 * it (in a `finally`) so a later non-sneak leg is not left crouched. A non-sneak leg
 * returns a no-op restore.
 *
 * `canOpenDoors` is enabled: some delve areas are entered only through a closed
 * door or fence gate (the ram pen's only opening is its oak_fence_gate). Opening one
 * is a right-click USE interaction — not a block break or place — which vanilla
 * permits in adventure mode, so it is the same action a human player must perform,
 * not a world mutation. mineflayer-pathfinder defaults it off (flaky on some
 * servers); the delve genuinely requires it, so the leg turns it on and the run
 * proves it works on the pinned server.
 */
export function configureLeg(
  bot: ControlBot,
  movements: LegMovements,
  sneak: boolean,
): () => void {
  movements.canDig = false; // adventure mode: never break blocks
  movements.allow1by1towers = false;
  movements.canOpenDoors = true; // open doors/fence-gates (adventure-legal use, not a mutation)
  if (!sneak) {
    return () => {};
  }
  movements.allowSprinting = false; // a sprinting bot is not sneaking
  bot.setControlState("sneak", true);
  return () => bot.setControlState("sneak", false);
}
