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

/** One body near a stuck bot, as the stuck report needs to describe it. */
export interface Neighbour {
  /** The mineflayer `entity.name` (no `minecraft:` prefix), or `?` if unknown. */
  readonly name: string;
  /** Distance from the bot, in blocks. */
  readonly distance: number;
}

/**
 * Split the bodies around a stuck bot into the ones the pathfinder routes AROUND
 * and the ones it routes THROUGH, by asking the pathfinder's OWN authority on the
 * question — the live `Movements.passableEntities` set, which is what
 * `updateCollisionIndex` checks each entity's name against before it indexes that
 * entity at all. Anything in the set is invisible to the search; anything outside
 * it contributes {@link https://github.com/PrismarineJS/mineflayer-pathfinder | `entityCost`}
 * to the cells its bounding box covers.
 *
 * Both halves keep the input order, which is the tracker's, so a reader comparing a
 * report against an older one is comparing the same list re-labelled.
 */
export function partitionByPassability(
  neighbours: readonly Neighbour[],
  passableEntities: ReadonlySet<string>,
): { obstructing: Neighbour[]; passable: Neighbour[] } {
  const obstructing: Neighbour[] = [];
  const passable: Neighbour[] = [];
  for (const n of neighbours) {
    (passableEntities.has(n.name) ? passable : obstructing).push(n);
  }
  return { obstructing, passable };
}

/** `name@distance`, one decimal — the form every stuck report has always used. */
function fmtNeighbour(n: Neighbour): string {
  return `${n.name}@${n.distance.toFixed(1)}`;
}

/**
 * The body of the `[stuck]` line: what stands around a bot whose walk failed, said
 * in terms of what the pathfinder does about each of them.
 *
 * ## Why the raw list was worse than nothing
 *
 * The report used to be an undifferentiated dump of every tracked entity within 12
 * blocks. On the gallery that reads `mannequin@10.0, interaction@10.0,
 * interaction@9.5, …` — thirty-odd bodies, almost all of them `interaction` and
 * `item_display` hitboxes that {@link allowNonCollidingEntities} has already marked
 * passable, plus the `item`/`arrow`/`experience_orb` drops mineflayer-pathfinder
 * ships as passable by default. Every one of them is a body the search never sees.
 *
 * A list like that does not merely fail to diagnose; it accuses. A round was
 * commissioned against "the muster hall strands the bot among 34 entities" on the
 * strength of this line, and the 34 could not have stranded anything.
 *
 * ## What the pathfinder actually does with a body
 *
 * Measured against the pinned `mineflayer-pathfinder` 2.4.5, which is the only
 * authority that matters here: `Movements.updateCollisionIndex` indexes a
 * non-passable entity into `entityIntersections`, and every consumer of that index
 * on a walking route adds `entityCost` (1) per intersected cell. The only places an
 * entity makes a move IMPOSSIBLE rather than dearer are inside branches that place
 * a scaffolding block — and {@link configureLeg} turns `canDig` and
 * `allow1by1towers` off for every leg this harness walks, so those branches are
 * unreachable. **A crowd therefore raises a route's price and can never delete
 * one**: `No path to the goal!` is always a statement about blocks, and the line
 * says so rather than leaving the next reader to re-derive it.
 *
 * `passableEntities` is read from the live `Movements` rather than from
 * {@link NON_COLLIDING_ENTITY_TYPES}, so the report cannot drift from what the
 * search was actually configured with — including the pathfinder's own defaults,
 * which this module never enumerates.
 */
export function describeStuckNeighbours(
  neighbours: readonly Neighbour[],
  passableEntities: ReadonlySet<string> | undefined,
): string {
  if (neighbours.length === 0) {
    return "nothing within 12 blocks — the refusal is about blocks, not bodies";
  }
  if (passableEntities === undefined) {
    // No Movements in force (nothing set one yet). Say that, rather than implying a
    // partition that was never computed.
    return (
      `${neighbours.length} entit${neighbours.length === 1 ? "y" : "ies"} within 12 ` +
      `blocks, unclassified (no pathfinder Movements were in force): ` +
      neighbours.map(fmtNeighbour).join(", ")
    );
  }
  const { obstructing, passable } = partitionByPassability(neighbours, passableEntities);
  const around =
    obstructing.length === 0
      ? "none"
      : `${obstructing.length} (${obstructing.map(fmtNeighbour).join(", ")})`;
  const through =
    passable.length === 0
      ? "none"
      : `${passable.length} (${passable.map(fmtNeighbour).join(", ")})`;
  return (
    `${neighbours.length} entit${neighbours.length === 1 ? "y" : "ies"} within 12 blocks. ` +
    `Routed AROUND: ${around}. Routed THROUGH: ${through}. ` +
    `A body is a COST to this pathfinder, never a wall (canDig and 1x1 towers are off ` +
    `on every leg), so no number of them can produce \`No path to the goal!\` — that ` +
    `answer is always about blocks`
  );
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
