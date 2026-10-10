// A bot with no server: the REAL mineflayer-pathfinder 2.4.5 (its A*, its
// `postProcessPath`, its per-tick steering) over the REAL client physics
// (prismarine-physics, which a mineflayer bot runs), in a world of real pinned
// block states. Nothing here models the library: the bot supplies only what a
// mineflayer bot supplies (a block lookup, an entity, controls, a physics tick),
// so what the pathfinder does here is what it does on a server — minus the
// server's own corrections, which a walk on flat ground does not draw.

import { createRequire } from "node:module";
import { EventEmitter } from "node:events";
import registryFor from "prismarine-registry";

const requireCjs = createRequire(import.meta.url);

export interface V3 {
  x: number;
  y: number;
  z: number;
  floored(): V3;
  offset(x: number, y: number, z: number): V3;
  clone(): V3;
}

const { Vec3 } = requireCjs("vec3") as { Vec3: new (x: number, y: number, z: number) => V3 };
const { pathfinder, Movements, goals } = requireCjs("mineflayer-pathfinder") as {
  pathfinder: (bot: unknown) => void;
  Movements: new (bot: unknown) => Record<string, unknown>;
  goals: { GoalNear: new (x: number, y: number, z: number, range: number) => unknown };
};
const physicsLib = requireCjs("prismarine-physics") as {
  Physics: (registry: unknown, world: unknown) => { simulatePlayer(state: unknown, world: unknown): unknown };
  PlayerState: new (bot: unknown, control: unknown) => { apply(bot: unknown): void };
};

export const SIM_VERSION = "1.21.11";
const registry = registryFor(SIM_VERSION);
const Block = requireCjs("prismarine-block")(registry) as {
  fromString(state: string, biome: number): { position: V3 };
  fromStateId(id: number, biome: number): { position: V3 };
};

/** A world: `"x,y,z"` → block state text (`minecraft:ladder[facing=west]`); absent is air. */
export type Cells = Map<string, string>;

export const cellKey = (x: number, y: number, z: number): string => `${x},${y},${z}`;

/** The part of a mineflayer bot the pathfinder and the client physics read. */
export class SimBot extends EventEmitter {
  readonly version = SIM_VERSION;
  readonly registry = registry;
  readonly game = { minY: -64, height: 384 };
  readonly inventory = { slots: [] as unknown[], items: (): unknown[] => [] };
  readonly entities: Record<number, unknown> = {};
  readonly controlState: Record<string, boolean> = {
    forward: false,
    back: false,
    left: false,
    right: false,
    jump: false,
    sprint: false,
    sneak: false,
  };
  jumpTicks = 0;
  jumpQueued = false;
  fireworkRocketDuration = 0;
  readonly entity: Record<string, unknown> & { position: V3; yaw: number; pitch: number };
  readonly physics: ReturnType<typeof physicsLib.Physics>;
  readonly world: { getBlock(p: V3): unknown };
  readonly cells: Cells;
  pathfinder!: {
    setMovements(m: unknown): void;
    setGoal(g: unknown): void;
  };

  constructor(cells: Cells, start: readonly [number, number, number]) {
    super();
    this.cells = cells;
    this.entity = {
      position: new Vec3(start[0], start[1], start[2]),
      velocity: new Vec3(0, 0, 0),
      onGround: true,
      isInWater: false,
      isInLava: false,
      isInWeb: false,
      isCollidedHorizontally: false,
      isCollidedVertically: false,
      elytraFlying: false,
      yaw: 0,
      pitch: 0,
      height: 1.8,
      width: 0.6,
      effects: {},
      attributes: {},
    };
    this.world = { getBlock: (p: V3) => this.blockAt(p) };
    this.physics = physicsLib.Physics(registry, this.world);
  }

  blockAt(p: V3): unknown {
    const [x, y, z] = [Math.floor(p.x), Math.floor(p.y), Math.floor(p.z)];
    const state = this.cells.get(cellKey(x, y, z));
    const b = state ? Block.fromString(state, 0) : Block.fromStateId(0, 0);
    b.position = new Vec3(x, y, z);
    return b;
  }

  setControlState(control: string, state: boolean): void {
    this.controlState[control] = state;
  }

  clearControlStates(): void {
    for (const c of Object.keys(this.controlState)) this.controlState[c] = false;
  }

  look(yaw: number, pitch: number): Promise<void> {
    this.entity.yaw = yaw;
    this.entity.pitch = pitch;
    return Promise.resolve();
  }

  lookAt(point: V3): Promise<void> {
    const p = this.entity.position;
    const dx = point.x - p.x;
    const dy = point.y - (p.y + 1.62);
    const dz = point.z - p.z;
    return this.look(Math.atan2(-dx, -dz), Math.atan2(dy, Math.hypot(dx, dz)));
  }

  /** One game tick: the pathfinder steers, then the client physics moves the body. */
  tick(): void {
    this.emit("physicsTick");
    const state = new physicsLib.PlayerState(this, { ...this.controlState });
    this.physics.simulatePlayer(state, this.world);
    state.apply(this);
  }
}

/**
 * A bot in `cells` at `start`, its pathfinder configured as the harness's
 * `configureLeg` configures it for an adventure-mode leg; `prepare` is handed the
 * bot after the plugin is loaded (where the harness installs its own listeners).
 */
export function simBot(
  cells: Cells,
  start: readonly [number, number, number],
  prepare: (bot: SimBot) => void = () => {},
): SimBot {
  const bot = new SimBot(cells, start);
  pathfinder(bot);
  prepare(bot);
  const movements = new Movements(bot);
  movements["canDig"] = false;
  movements["allow1by1towers"] = false;
  movements["canOpenDoors"] = true;
  bot.pathfinder.setMovements(movements);
  return bot;
}

/** Where a simulated walk ended, and whether the pathfinder said it arrived. */
export interface SimWalk {
  readonly reached: boolean;
  readonly at: readonly [number, number, number];
  readonly ticks: number;
}

/** Walk to `GoalNear(goal, range)` for at most `ticks` game ticks. */
export function simWalk(
  bot: SimBot,
  goal: readonly [number, number, number],
  range: number,
  ticks = 600,
): SimWalk {
  let reached = false;
  const onReached = (): void => {
    reached = true;
  };
  bot.once("goal_reached", onReached);
  bot.pathfinder.setGoal(new goals.GoalNear(goal[0], goal[1], goal[2], range));
  let t = 0;
  for (; t < ticks && !reached; t++) bot.tick();
  bot.off("goal_reached", onReached);
  bot.pathfinder.setGoal(null);
  const p = bot.entity.position;
  return { reached, at: [p.x, p.y, p.z], ticks: t };
}

export const fmtPos = (p: readonly number[]): string => `[${p.map((v) => v.toFixed(2)).join(", ")}]`;
