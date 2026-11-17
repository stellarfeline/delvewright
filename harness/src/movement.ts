// Sneak-aware leg configuration for the pathfinder (spec-0008 gap-7 subset).
// A critical-path step may be marked `sneak: true` — the stealth finale walks that
// leg crouched so the bot stays inside the safe (quiet) envelope a live warden's
// vibration senses gate on. This helper centralises the movement/control-state
// setup so it is pure enough to unit-test the flag plumbing without a server.

import { createRequire } from "node:module";

const requireCjs = createRequire(import.meta.url);

/** The subset of a mineflayer Bot this helper drives. */
export interface ControlBot {
  setControlState(control: "sneak", state: boolean): void;
  /** The control states as the client holds them (mineflayer's `bot.controlState`). */
  readonly controlState: { readonly sneak: boolean };
  on(event: "physicsTick", listener: () => void): unknown;
  removeListener(event: "physicsTick", listener: () => void): unknown;
}

/**
 * The subset of a mineflayer-pathfinder `Movements` this helper tunes. `canDig`
 * and `allow1by1towers` are always locked off (adventure mode: never break or
 * pillar); `canOpenDoors` is turned ON (opening a door/fence-gate is a use
 * interaction vanilla permits in adventure mode — see {@link configureLeg});
 * `allowSprinting` is turned off for a sneak leg; `allowParkour` is turned off
 * for every leg and every hop — a jump the compiler proved is driven by the
 * executor as the compiler flew it (spec-0113 §6, `executor/jump.ts`), never
 * by the pathfinder.
 */
export interface LegMovements {
  canDig: boolean;
  allow1by1towers: boolean;
  canOpenDoors: boolean;
  allowSprinting: boolean;
  allowParkour: boolean;
  /** mineflayer-pathfinder's `Movements.liquidCost` (present at runtime,
   * absent from its typings). */
  liquidCost?: number;
  allowFreeMotion?: boolean;
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
 * Whether a block is a door a hand opens: every `*_door` but the iron one
 * (spec-0123 §3.1, `docs/reference/doors-by-hand.md`). The bot's pathfinder
 * (mineflayer-pathfinder 2.4.5) opens fence gates itself and has no notion of
 * a door, so the walk opens each door its proven route crosses and steps
 * through it on foot.
 */
export function isDoorByHand(name: string): boolean {
  return name.endsWith("_door") && name !== "iron_door";
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
 * sprinting in the Movements and HOLD the bot's sneak control state on for the whole
 * leg. Returns a restore function that releases the hold and clears the crouch; the
 * caller MUST invoke it (in a `finally`) so a later non-sneak leg is not left
 * crouched. A non-sneak leg returns a no-op restore.
 *
 * **The crouch is held every tick, not set once.** mineflayer-pathfinder clears
 * every control state on `setMovements`, on every `setGoal` and on arrival
 * (`resetPath`, `fullStop` → `bot.clearControlStates()`), and the leg calls the
 * first two on every hop, so a crouch set once at the leg's start was gone before
 * the body took a step: a sneak leg was walked upright on every tick
 * (`test/sneak-leg.test.ts` reproduces it on the real library). The hold re-asserts
 * the crouch on each `physicsTick` — after the library's steering ran and before
 * the client sends that tick's position — so the server reads the body crouched
 * at every move it is sent.
 *
 * `yieldCrouch` names the moments a crouched body cannot do what the leg asks
 * and a player stands up for it: the leg's `stand_ups`, the compiler's one
 * answer (a climb down, a step down, a jump — `waypoints::stand_ups`). While it
 * answers `true` the hold stands the body up instead.
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
  yieldCrouch: () => boolean = () => false,
): () => void {
  movements.canDig = false; // adventure mode: never break blocks
  movements.allow1by1towers = false;
  movements.canOpenDoors = true; // open doors/fence-gates (adventure-legal use, not a mutation)
  // The library leaps gaps on its own (`allowParkour` defaults on). The bot
  // walks what the compiler proved: a leap is a proven jump's hop, which the
  // executor drives itself (spec-0113 §6), so the library never leaps.
  movements.allowParkour = false;
  // spec-0134 §10: the route proof crosses water only on a leg it exported a
  // `water` stretch for; every other leg keeps out of it, as the proof did.
  swimFor(movements, false);
  if (!sneak) {
    return () => {};
  }
  movements.allowSprinting = false; // a sprinting bot is not sneaking
  const hold = (): void => {
    const want = !yieldCrouch();
    if (bot.controlState.sneak !== want) bot.setControlState("sneak", want);
  };
  hold();
  bot.on("physicsTick", hold);
  return () => {
    bot.removeListener("physicsTick", hold);
    bot.setControlState("sneak", false);
  };
}

/**
 * One node of a mineflayer-pathfinder path after the library's
 * `postProcessPath`: `x`/`y`/`z` are where its steering aims (no longer the
 * cell), `hash` is still the cell, `"x,y,z"`.
 */
export interface PathNode {
  x: number;
  y: number;
  z: number;
  readonly hash: string;
  readonly toBreak: readonly unknown[];
  readonly toPlace: readonly unknown[];
}

/**
 * Re-aim every ladder node of `path` that the body walks through on its floor at
 * the body's feet, and return the cells re-aimed.
 *
 * mineflayer-pathfinder 2.4.5's `postProcessPath` aims each node at the top of
 * the collision shape in its cell, and a ladder's shape is a full-height panel, so
 * a node in a ladder cell is aimed a block above the feet — unless the next node
 * descends, where the library aims at the feet itself. Its steering takes a node
 * as reached only within `|dy| < 1`. A body entering the cell head-on pushes into
 * the panel and is lifted to that height; a body walking along the panel's face
 * pushes into nothing, so the steering holds still at the cell's edge and the
 * walk never arrives. In vanilla a ladder cell is open to a body standing on the
 * floor beneath it, and the compiler's route model walks through it so.
 *
 * The library's own descent rule, widened by one case: a node is aimed at the
 * feet when the body enters it level (`prevY`, the cell before it, at the same
 * height) and the next node does not rise. A node the body climbs into, or climbs
 * on from, keeps the library's aim. `climbable` is asked of the node's CELL, so
 * only what the library raised is ever moved; the walk stops at the first node
 * that breaks or places, as the library's own processing does.
 */
export function holdClimbableNodesAtFeet(
  path: PathNode[],
  startY: number,
  climbable: (x: number, y: number, z: number) => boolean,
): string[] {
  const cell = (n: PathNode): [number, number, number] => {
    const [x, y, z] = n.hash.split(",").map(Number) as [number, number, number];
    return [x, y, z];
  };
  const held: string[] = [];
  let prevY = startY;
  for (let i = 0; i < path.length; i++) {
    const node = path[i]!;
    if (node.toBreak.length > 0 || node.toPlace.length > 0) break;
    const [x, y, z] = cell(node);
    const next = path[i + 1];
    const level = prevY === y && (next === undefined || cell(next)[1] <= y);
    if (level && node.y > y && climbable(x, y, z)) {
      node.x = x + 0.5;
      node.y = y;
      node.z = z + 0.5;
      held.push(node.hash);
    }
    prevY = y;
  }
  return held;
}

/** The subset of a mineflayer bot {@link holdWalkedClimbableNodes} reads. */
export interface ClimbHoldBot {
  on(event: "path_update", listener: (results: { path: PathNode[] }) => void): unknown;
  readonly entity: {
    readonly position: {
      readonly x: number;
      readonly y: number;
      readonly z: number;
      offset(dx: number, dy: number, dz: number): unknown;
    };
  };
  blockAt(pos: unknown): { readonly type: number } | null;
  readonly pathfinder: { readonly movements?: { readonly climbables?: Set<number> } };
}

/**
 * Install {@link holdClimbableNodesAtFeet} on every path the pathfinder adopts.
 * `path_update` hands the listener the very path the library's steering is about
 * to follow, after its own post-processing, so the re-aim is what the steering
 * reads. What counts as climbable is the library's own set
 * (`Movements.climbables`). Once per bot, after the plugin is loaded.
 */
export function holdWalkedClimbableNodes(bot: ClimbHoldBot, log: (line: string) => void = () => {}): void {
  bot.on("path_update", (results) => {
    const climbables = bot.pathfinder.movements?.climbables;
    if (!climbables || climbables.size === 0) return;
    const p = bot.entity.position;
    const held = holdClimbableNodesAtFeet(results.path, Math.floor(p.y), (x, y, z) => {
      const block = bot.blockAt(p.offset(x + 0.5 - p.x, y + 0.5 - p.y, z + 0.5 - p.z));
      return block !== null && climbables.has(block.type);
    });
    if (held.length > 0) {
      log(`[ladder] ${held.length} ladder node(s) walked through on the floor, aimed at the feet: ${held.join(" ")}\n`);
    }
  });
}

/** The pathfinder's per-block cost of a liquid cell on a leg the compiler
 * proved swims (spec-0134 §10): finite, so the leg's water is crossed. */
export const SWIM_LIQUID_COST = 1;

/**
 * Admit liquid on a leg the compiler proved swims (its `water` stretch), and
 * on no other (spec-0134 §10): `liquidCost` finite on a water leg and
 * unreachable otherwise; free motion off either way, so the bot swims the
 * cells the proof did. Returns whether the leg swims.
 */
export function swimFor(
  movements: Pick<LegMovements, "liquidCost" | "allowFreeMotion">,
  water: boolean,
): boolean {
  movements.liquidCost = water ? SWIM_LIQUID_COST : Number.POSITIVE_INFINITY;
  movements.allowFreeMotion = false;
  return water;
}

/** A cell of mineflayer-pathfinder 2.4.5's search: `Movements.getBlock`'s answer. */
export interface SwimCell {
  readonly name: string;
  readonly boundingBox: string;
  readonly physical: boolean;
  readonly safe: boolean;
  /** The block's collision boxes, each `[x0, y0, z0, x1, y1, z1]` in blocks
   * of its own cell, as the library's `Block.shapes` gives them. */
  readonly shapes?: readonly (readonly number[])[];
  readonly position?: { readonly x: number; readonly y: number; readonly z: number } | null;
}

/** A node of the library's search (its `Move`). */
export interface SwimNode {
  readonly x: number;
  readonly y: number;
  readonly z: number;
  readonly remainingBlocks: number;
}

/** The part of a mineflayer-pathfinder `Movements` {@link installSwimMoves} extends. */
export interface SwimMovements {
  liquidCost?: number;
  getNeighbors(node: SwimNode): SwimNode[];
  getBlock(node: SwimNode, dx: number, dy: number, dz: number): SwimCell;
  exclusionStep(block: SwimCell): number;
}

/** How high a body in water climbs out, in cells over the cell it swims in:
 * the route proof's `AFLOAT_CLIMB_OUT_CELLS` (`compiler/nav/world/body.rs`). */
export const SWIM_CLIMB_OUT_CELLS = 1;

/** The vanilla auto-step in blocks, `metrics::MAX_AUTO_STEP_16` (9/16): a
 * floor in the cell one over the band whose collision top is no higher than
 * this over that cell's bottom is one the route proof's flown climb out may
 * land a body on (`World::flown_climb_outs`, a lily pad on the water). */
export const SWIM_CLIMB_OUT_THIN_TOP = 9 / 16;

/** A floor whose every collision box tops out within {@link SWIM_CLIMB_OUT_THIN_TOP}. */
const thinFloor = (b: SwimCell): boolean =>
  b.physical &&
  b.shapes !== undefined &&
  b.shapes.length > 0 &&
  b.shapes.every((s) => s[4] <= SWIM_CLIMB_OUT_THIN_TOP + 1e-9);

const { Move } = { Move: requireCjs("mineflayer-pathfinder/lib/move") } as {
  Move: new (x: number, y: number, z: number, remainingBlocks: number, cost: number) => SwimNode;
};

const installed = new WeakSet<object>();

/**
 * **The water moves the route proof takes and mineflayer-pathfinder 2.4.5 does
 * not** (spec-0134 §10). The library's search keeps a body in water at the
 * height it is in: its `getMoveUp` refuses a node in liquid, and its
 * `getMoveJumpUp` measures the rise from the floor block under the node, which
 * in water is water whose height is its own cell's bottom, so a ledge one over
 * the swim cell reads two blocks high and is refused. A body in water then has
 * no way up and no way out onto a ledge flush with the surface, where a player
 * swims up and steps out. The route proof (`World::swim_moves`,
 * `compiler/nav/route/mod.rs`) crosses both: from a cell a body swims in, up
 * to the swim cell over it, and out onto a standable cell beside it within
 * {@link SWIM_CLIMB_OUT_CELLS}, with open or water headroom in its own column
 * for the lift. This adds exactly those moves to the library's own, on a leg
 * whose liquid is admitted ({@link swimFor}) and on no other; the library's
 * steering already holds the jump in water, which rises the body and makes
 * the fluid jump over the rim. Lava is never swum. Installed once per
 * `Movements`.
 */
export function installSwimMoves(movements: SwimMovements): void {
  if (installed.has(movements)) return;
  installed.add(movements);
  const base = movements.getNeighbors.bind(movements);
  movements.getNeighbors = (node: SwimNode): SwimNode[] => {
    const out = base(node);
    const liquid = movements.liquidCost;
    if (liquid === undefined || !Number.isFinite(liquid)) return out;
    out.push(...swimMoves(movements, node, liquid));
    return out;
  };
}

/** Water, never lava: a cell a body floats in. */
const isWater = (b: SwimCell): boolean => b.name === "water";
/** A cell a body's box passes through. */
const open = (b: SwimCell): boolean => b.boundingBox === "empty";

/** The moves {@link installSwimMoves} adds out of `node`, each priced as the
 * library prices a step with its feet in liquid. */
export function swimMoves(movements: SwimMovements, node: SwimNode, liquidCost: number): SwimNode[] {
  const at = (dx: number, dy: number, dz: number): SwimCell => movements.getBlock(node, dx, dy, dz);
  const swimsIn = (dx: number, dy: number, dz: number): boolean =>
    isWater(at(dx, dy, dz)) && (isWater(at(dx, dy + 1, dz)) || open(at(dx, dy + 1, dz)));
  if (!swimsIn(0, 0, 0)) return [];
  const out: SwimNode[] = [];
  const push = (dx: number, dy: number, dz: number, cost: number): void => {
    const total = cost + liquidCost + movements.exclusionStep(at(dx, dy, dz));
    if (total > 100) return;
    out.push(new Move(node.x + dx, node.y + dy, node.z + dz, node.remainingBlocks, total));
  };
  // A swimmer rises.
  if (swimsIn(0, 1, 0)) push(0, 1, 0, 1);
  // And climbs out: the first standable cell beside it, lowest first.
  for (const [dx, dz] of [
    [-1, 0],
    [1, 0],
    [0, -1],
    [0, 1],
  ] as const) {
    for (let rise = 0; rise <= SWIM_CLIMB_OUT_CELLS; rise++) {
      let headroom = true;
      for (let y = 1; y <= rise + 1; y++) {
        const b = at(0, y, 0);
        if (!open(b) && !isWater(b)) headroom = false;
      }
      const floor = at(dx, rise - 1, dz);
      const feet = at(dx, rise, dz);
      const head = at(dx, rise + 1, dz);
      if (headroom && floor.physical && feet.safe && !isWater(feet) && head.safe) {
        push(dx, rise, dz, 1 + rise);
        break;
      }
    }
    // Or onto a thin floor in the cell one over the band's floor, a lily pad
    // on the water, where the route proof's flown climb out lands a body
    // (the route proof's standing cell is the one over the pad; the library's
    // node is the cell the body's feet are in, the pad's own).
    const rise = SWIM_CLIMB_OUT_CELLS;
    let headroom = true;
    for (let y = 1; y <= rise + 2; y++) {
      const b = at(0, y, 0);
      if (!open(b) && !isWater(b)) headroom = false;
    }
    const pad = at(dx, rise, dz);
    const head = at(dx, rise + 1, dz);
    const over = at(dx, rise + 2, dz);
    if (headroom && thinFloor(pad) && head.safe && !isWater(head) && over.safe) {
      push(dx, rise, dz, 2 + rise);
    }
  }
  return out;
}
