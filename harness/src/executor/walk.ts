// MineflayerExecutor: movement: a reach step, the leg walk, the goto, stall recovery.

// mineflayer-pathfinder is CommonJS; import the default and destructure (the only
// harness dependency added for v0.3 — replaces the naive "face + hold forward"
// walk so turns/branches in jigsaw layouts are walkable).
import pathfinderPkg from "mineflayer-pathfinder";
import type { ReachStep, Transport, Vec3Tuple } from "../critical-path.ts";
import { insideCompletion, reachGoal } from "../critical-path.ts";
import { BotDeathError } from "../death.ts";
import { lethalStepCost } from "../death-loop.ts";
import { allowNonCollidingEntities, configureLeg, describeStuckNeighbours } from "../movement.ts";
import {
  nextLegWaypoints,
  LEG_START_REACH,
  retainStandableWaypoints,
  walkGoals,
  type GoalSpec,
  type TimedGate,
  type Waypoints,
} from "../waypoints.ts";
import {
  describeGates,
  gatesBindingWalk,
  gatesCrossedByHop,
  unstagedCrushBoxes,
} from "../timed-gate.ts";
import { delay, withTimeout, fmt } from "./connection.ts";
import { ControlTakenError } from "./cutscene.ts";
import { type GateAssist, crossTimedGate } from "./timed-gate.ts";
import type { MineflayerExecutor } from "../executor.ts";

export const { pathfinder, Movements, goals } = pathfinderPkg;

/** Bounded number of physics-unstick bursts before a wedged hop fails loudly. */
const UNSTICK_ATTEMPTS = 3;

/**
 * A raw, pathfinder-free nudge toward `target` to dislodge a physically wedged bot
 * (a concave corner beside a wall the A* pathfinder cannot escape). Returns how far
 * (blocks) the bot actually moved, so the caller can adapt the aim when a burst is
 * wall-blocked. Navigation robustness, NOT game logic. Provided by the executor;
 * injected so the recovery control flow stays unit-testable.
 */
export type Unstick = (target: GoalSpec) => Promise<number>;

/**
 * Authoritative "this leg's purpose is already fulfilled" oracle,
 * consulted ONLY on a walk's failure path — never to shortcut a healthy hop.
 * Returns a human-readable reason when the step the walk serves is already
 * settled (its objective's anchored completion marker arrived, or its exported
 * completion transport has carried the bot to the next area), `undefined`
 * otherwise.
 *
 * Why it exists: an objective can complete MID-WALK — the tide-mill `reach` fires
 * its distance check as the bot crosses a timed-gate leg, and the objective's
 * emission teleports it to the next area (a physically one-way transport). The
 * remaining hops of the old area's leg then fail, and without this oracle the
 * harness read that position discontinuity as the gate blocking a leg the bot had
 * ALREADY walked — looping gate retries and "re-centering" toward cells the
 * one-way transport makes unreachable. Completion signals outrank position: an
 * objective that is complete makes its leg a success, wherever the bot stands.
 */
export type LegSettled = () => string | undefined;

/** Log-and-report helper for a {@link LegSettled} hit on a failure path. */
export function legSettledReason(settled: LegSettled | undefined, glabel: string): string | undefined {
  const reason = settled?.();
  if (reason !== undefined) {
    process.stderr.write(
      `[settled] ${glabel}: ending the leg as SUCCEEDED — ${reason}; ` +
        `resuming from the bot's current area\n`,
    );
  }
  return reason;
}

/**
 * Replay a leg's ordered goals with **stall-recovery**. Each `goto`
 * performs one verified hop (rejecting on stall / death). Extracted from `walkTo`
 * as a pure control-flow function — injecting `goto` (and an optional physics
 * `unstick`) — so the recovery logic is unit-testable without a live pathfinder.
 *
 * A leg replays compiler-proven cells at `WAYPOINT_RANGE = 1`. That range-1
 * tolerance lets the bot satisfy the PREVIOUS hop at an off-route cell — a corner
 * pocket beside a wall — from which the next hop wedges: the bot oscillates and
 * times out (the nobodys-cave perimedes approach). Recovery escalates:
 *   1. re-path to the exact last proven cell (`range 0`, back onto the proven
 *      polyline); if that succeeds, retry the hop;
 *   2. if the recovery pathfind ITSELF stalls (the wedge defeats the pathfinder
 *      too), fall back to a bounded {@link Unstick} — a raw look+forward(+jump)
 *      burst toward the proven cell that bypasses the pathfinder — and after each
 *      burst retry the **actual next hop at its own range** (never the proven cell
 *      at range 0: a freed bot overshoots the strict target and oscillates; the
 *      hop's normal range 1 is forgiving enough to land).
 * Range 0 is used only for the level-1 re-centre, so a legitimate slab/stair
 * fractional-height floor on the happy path is unaffected, and the per-hop `goto`
 * timeout is untouched. A first-hop stall (nothing proven yet) is not this class and
 * is rethrown; a hop still unwalkable after recovery + unstick fails loudly.
 */
export async function replayLegWithRecovery(
  goalsList: readonly GoalSpec[],
  label: string,
  goto: (spec: GoalSpec, label: string) => Promise<void>,
  unstick?: Unstick,
  gate?: GateAssist,
  settled?: LegSettled,
): Promise<void> {
  const gates = gate?.gates ?? [];
  let lastProven: GoalSpec | undefined;
  for (let g = 0; g < goalsList.length; g++) {
    const spec = goalsList[g]!;
    const last = g === goalsList.length - 1;
    const glabel = last ? label : `${label} waypoint ${g + 1}/${goalsList.length}`;
    // A `crush: true` gate must never be entered blind. The reactive flow
    // below waits for a window only AFTER a hop fails — and on a crush gate the
    // first failure is the closing edge killing the bot inside the fill (an instant,
    // gear-independent kill the compiler emits at close). So a hop whose straight
    // mouth-to-mouth segment crosses a crush gate is STAGED proactively: the bot
    // holds at the gate edge (the compiler-pinned mouth cell it is already standing
    // on), observes a fresh closed→open edge, checks the crossing fits the window
    // with margin, and only then enters. Non-crush gates keep the proven reactive
    // flow — their worst case is a path abort, which is information, not damage.
    if (gate && gates.some((tg) => tg.crush)) {
      const origin: Vec3Tuple | undefined = lastProven
        ? [lastProven.x, lastProven.y, lastProven.z]
        : gate.feetCell();
      const crushCrossed = gatesCrossedByHop(origin, [spec.x, spec.y, spec.z], gates).filter(
        (tg) => tg.crush,
      );
      if (crushCrossed.length > 0) {
        process.stderr.write(
          `[timed-gate] ${glabel}: crush gate ahead — staging at the edge of ` +
            `${describeGates(crushCrossed)} for a fresh window\n`,
        );
        if (
          (await crossTimedGate(spec, glabel, lastProven, gate, goto, unstick, undefined, settled, crushCrossed)) ===
          "settled"
        ) {
          return;
        }
        lastProven = spec;
        continue;
      }
    }
    try {
      await goto(spec, glabel);
    } catch (err) {
      if (err instanceof BotDeathError) throw err;
      // Before judging the hop failed, consult the completion oracle.
      // A step whose objective is already complete (or whose completion transport
      // has landed) has nothing left for this leg to prove — the "failure" is the
      // position discontinuity of a teleport the leg itself triggered. Failure
      // path only: a healthy hop is never shortcut.
      if (legSettledReason(settled, glabel) !== undefined) return;
      // A leg the compiler proved walks THROUGH a timed gate (spec-0016 §4) gets the
      // window wait; every other leg keeps the old behaviour exactly, so a real
      // navigation regression still fails on the first stall.
      if (gate && gates.length > 0) {
        // A `settled` outcome ends the WHOLE leg, not just this hop: the step is
        // already complete and the remaining hops belong to the area the (one-way)
        // transport carried the bot out of.
        if ((await crossTimedGate(spec, glabel, lastProven, gate, goto, unstick, err, settled)) === "settled") {
          return;
        }
      } else {
        if (!lastProven) throw err; // nothing proven yet — not the pocket-wedge class
        try {
          await recoverAndRetry(spec, glabel, lastProven, goto, unstick);
        } catch (recoverErr) {
          if (recoverErr instanceof BotDeathError) throw recoverErr;
          // The settle signal can land while the recovery is in flight (a marker is
          // a chat packet racing the position jump) — re-check before failing.
          if (legSettledReason(settled, glabel) !== undefined) return;
          throw recoverErr;
        }
      }
    }
    lastProven = spec;
  }
}

/**
 * The completion facts of the step a walk serves: the `obj/<id>` the
 * step proves and, when the compiler exported one, the absolute destination its
 * completion teleports the player to (gap 8). `walkTo` turns these into the
 * {@link LegSettled} oracle its failure paths consult — pure step-contract data
 * from `critical-path.json`, never a harness inference.
 */
interface StepCompletion {
  readonly objective: string;
  readonly transport?: Transport;
}

/** Try a `goto`, returning whether it arrived; a bot death still propagates. */
export async function reached(fn: () => Promise<void>): Promise<boolean> {
  try {
    await fn();
    return true;
  } catch (err) {
    if (err instanceof BotDeathError) throw err;
    return false;
  }
}

/**
 * Recover from a stalled hop and retry it. Level 1: re-path to the last
 * proven cell (range 0) to re-centre on the polyline, then retry the hop. Level 2
 * (the wedge defeats the pathfinder too): a bounded physics {@link Unstick} to break
 * the bot free, retrying the ACTUAL hop at its own range after each burst. A hop
 * still unwalkable after the budget fails loudly.
 *
 * Level-2 aim is **adaptive** (trace-derived): drive toward the GOAL for
 * forward progress, but if a burst measured no progress the bot is wall-blocked (the
 * goal lies through the concave-corner wall) — the next burst aims at the PROVEN cell
 * instead, the open away-from-wall direction that escapes the pocket. Neither fixed
 * direction alone works: goal-only can't escape the initial pocket (drives into the
 * wall), proven-only shoves an already-advanced bot backward and oscillates.
 */
export async function recoverAndRetry(
  spec: GoalSpec,
  glabel: string,
  proven: GoalSpec,
  goto: (spec: GoalSpec, label: string) => Promise<void>,
  unstick?: Unstick,
): Promise<void> {
  const provenGoal: GoalSpec = { x: proven.x, y: proven.y, z: proven.z, range: 0 };
  process.stderr.write(
    `[recover] re-centering on proven cell [${proven.x}, ${proven.y}, ${proven.z}] ` +
      `(range 0), then retrying ${glabel}\n`,
  );
  // Level 1: pathfinder re-centre, then retry the hop.
  if (await reached(() => goto(provenGoal, `${glabel} recovery to last proven cell`))) {
    await goto(spec, glabel); // retry; rethrows if still stuck
    return;
  }
  // Level 2: bounded adaptive physics-unstick, retrying the hop after each burst.
  if (unstick) {
    let lastMoved = Number.POSITIVE_INFINITY; // first burst aims at the goal
    for (let a = 0; a < UNSTICK_ATTEMPTS; a++) {
      const towardGoal = lastMoved >= UNSTICK_MIN_PROGRESS;
      const target = towardGoal ? spec : provenGoal;
      process.stderr.write(
        `[recover] physics-unstick burst ${a + 1}/${UNSTICK_ATTEMPTS} toward ` +
          `${towardGoal ? "goal" : "proven cell"}\n`,
      );
      lastMoved = await unstick(target);
      if (await reached(() => goto(spec, `${glabel} retry after unstick ${a + 1}`))) {
        return;
      }
    }
  }
  await goto(spec, glabel); // budget exhausted — surface the failure loudly
}

/** How long (ms) a movement step may run before it is declared failed. */
const REACH_TIMEOUT_MS = 60_000;

/** Polling interval (ms) while walking toward a target. */
export const REACH_POLL_MS = 250;

/** How far ahead (blocks) a step is probed for safe footing — a little more than
 * one tick of walking, so the ledge is seen before the body reaches it. */
const STEP_PROBE_BLOCKS = 0.7;

/**
 * Physics-unstick: a SHORT forward tap per burst (~a cell, not a launch —
 * a long burst overshoots a tight 2-wide corridor and oscillates wall-to-wall), a
 * jump only when a gentle burst moved less than `UNSTICK_MIN_PROGRESS` blocks (truly
 * wedged against a lip), and a brief settle before re-pathing.
 */
export const UNSTICK_BURST_MS = 250;

const UNSTICK_MIN_PROGRESS = 0.5;

const UNSTICK_SETTLE_MS = 300;

/**
 * How many times one walk may lose control to a cutscene and resume — the
 * bound that keeps a trigger re-firing on every return from hanging the run.
 */
const MAX_CONTROL_RESUMES = 8;

/**
 * How many times a single hop may be interrupted for self-defense before it is walked
 * regardless. Bounded so a pack of mobs cannot livelock a leg; the wave-fight path
 * (which has its own 90s budget) is where a real fight belongs.
 */
const DEFENSE_ROUNDS_PER_HOP = 3;

/**
 * How long (ms) to let an interrupted `goto` settle before swinging. The pathfinder
 * halts at the next path node, so a moment's grace stops it dragging the bot out of
 * melee mid-fight — bounded tightly, because the bot is being hit while it waits.
 */
const WALK_SETTLE_MS = 300;

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  /**
   * Supply the compiler's proven critical-path waypoints. Optional —
   * without it, `walkTo` uses the original single distant-goal behavior. Called by
   * the entrypoint when the `validation/critical-path-waypoints.json` artifact
   * accompanies the critical path.
   */
  useWaypoints(this: MineflayerExecutor, waypoints: Waypoints): void {
    this.waypoints = waypoints;
  },

  /**
   * One hop, with staging.
   *
   * Runs {@link runGoto}, but races it against the damage handlers: a body that
   * latches onto the bot mid-walk is taken out of the run by {@link stageAway} the
   * moment it lands a hit, and the hop is retried. Bounded by
   * {@link DEFENSE_ROUNDS_PER_HOP}, after which the hop is walked with no further
   * interruption and fails exactly as loudly as it did before this existed.
   *
   * Nothing here is a fight and nothing here is a measurement: whether a player
   * could walk this leg through that body is the owner's playtest hour. What the
   * hop proves is that the ROUTE is walkable, and a body standing on it is a thing
   * to be removed so the route can be read.
   *
   * **A `sneak` leg is exempt from all of it.** `sneak: true` is the delve
   * declaring that stealth, not combat, is the mechanic on this leg, and a stealth
   * section runs on a clock: nobodys-cave-island's `begin-stealth` gives 90 ticks
   * of grace outside a safe zone and answers a miss with `damage-players 40` — an
   * instant kill. The stealth damage itself carries NO source entity, so it is
   * attributed to the nearest hostile — the (Invulnerable) warden the delve wants
   * the player to creep past — and staging it away would be the harness answering
   * a stealth mechanic by deleting its subject. On a sneak leg, the bot creeps.
   */
  async gotoStaged(this: MineflayerExecutor, spec: GoalSpec, label: string, sneak = false): Promise<void> {
    if (sneak) {
      this.sneaking = true;
      try {
        await this.runGoto(spec, label);
      } finally {
        this.sneaking = false;
      }
      return;
    }
    for (let round = 0; round < DEFENSE_ROUNDS_PER_HOP; round++) {
      await this.maybeEat(label);
      const before = this.bodiesStaged();
      try {
        await this.runGoto(spec, label);
        return;
      } catch (err) {
        if (err instanceof BotDeathError) throw err;
        // A hop can fail because a body is standing in the path; the damage
        // handlers stage away anything that hits the bot, so a retry is only
        // worth taking when one of them actually went.
        if (this.bodiesStaged() === before) throw err;
        process.stderr.write(
          `[staged] ${label} failed with a body on the bot; ` +
            `${this.bodiesStaged() - before} removed since the hop opened — retrying\n`,
        );
      }
    }
    // Rounds spent — walk it out. Still the original, unweakened hop.
    await this.runGoto(spec, label);
  },

  /**
   * Walk into the objective's completion volume, then prove the bot is in it.
   *
   * The goal comes from `step.completion` — what the SERVER adjudicates — and
   * never from the authored `radius`. Those were the same number until DSL v0.3
   * and have not been since: the datapack moved to a fixed ±1 cube and this bot
   * kept aiming at `radius - 1`, so a `radius: 3` reach let it stop three blocks
   * out, outside the region, and hang on the wait below. It failed intermittently,
   * because a `GoalNear` usually overshoots inward — which is the worst way for it
   * to be wrong.
   *
   * Standing in the volume is not success — the objective's own marker is
   * (AUDIT-P0) — so the position is read on the FAILURE path and nowhere else.
   * That placement is not squeamishness, it is the only correct one: this
   * objective's completion may TELEPORT the player (an exported `transport`), so
   * a bot that did everything right is legitimately somewhere else by the time
   * anyone could look, and a positive precondition here would fail exactly the
   * runs that worked. On the failure path there is no race and nothing to
   * false-fail: the marker did not arrive, so nothing moved the bot, and where it
   * is standing is where its walk left it. That turns the timeout this defect
   * used to produce — sixty seconds of silence blamed on the datapack — into a
   * sentence naming the volume, the position, and which of the two is wrong.
   */
  async reach(this: MineflayerExecutor, step: ReachStep): Promise<void> {
    if (step.completedOnLanding) {
      // The compiler says the previous step's carry put the party down inside
      // this volume, so the objective completed on that landing. Nothing is
      // walked; the marker is asserted, and its absence is the compiler's claim
      // failing, never a reason to go looking for the volume.
      const done = this.completedObjectives.get(step.objective);
      if (done !== undefined) {
        process.stderr.write(
          `[reach] ${step.objective} completed on the landing (step ${done}), as the path says\n`,
        );
        return;
      }
      try {
        await this.requireObjective(step.objective, `reach ${step.anchor} (on the landing)`);
      } catch (err) {
        if (err instanceof BotDeathError) throw err;
        throw new Error(
          `reach ${step.anchor}: the path says the previous carry's landing completes ` +
            `${step.objective} (completed_on_landing), and no marker arrived — the bot is at ` +
            `${fmt(this.requireBot().entity.position)}, volume ${JSON.stringify(step.completion)}. ` +
            `The landing and the volume disagree with the compiler's reading of them. ` +
            `Original: ${(err as Error).message}`,
        );
      }
      return;
    }
    const goal = reachGoal(step.completion);
    await this.walkTo(goal.pos, goal.range, `anchor ${step.anchor}`, step.sneak, {
      objective: step.objective,
      transport: step.transport,
    });
    try {
      await this.requireObjective(step.objective, `reach ${step.anchor}`);
    } catch (err) {
      const p = this.bot?.entity?.position;
      if (p) {
        const here: Vec3Tuple = [p.x, p.y, p.z];
        if (!insideCompletion(here, step.completion)) {
          throw new Error(
            `reach ${step.anchor}: the objective never completed, and the bot is at ` +
              `${here.map((n) => n.toFixed(2)).join(", ")} — OUTSIDE the volume the server ` +
              `adjudicates this objective in (${JSON.stringify(step.completion)}; authored ` +
              `radius ${step.radius}). The fault is the walk, not the datapack: no marker ` +
              `can arrive from here. Fix the navigation or the exported volume — do not ` +
              `widen either to make this pass. Original: ${(err as Error).message}`,
          );
        }
      }
      throw err;
    }
  },

  /**
   * Pathfind to within `range` blocks of the absolute target (mineflayer-pathfinder
   * `GoalNear`). Replaces the pre-v0.3 "face + hold forward" walk, so turns and
   * branches in jigsaw layouts are walkable. Digging is disabled (adventure mode).
   * A `sneak` leg (gap 7) walks crouched with sprinting disabled; the crouch is
   * restored to off afterwards so a later plain leg is not left sneaking. The long
   * `goto` wait races the death latch so a death aborts it fast, not after ~60s.
   *
   * `completion` names the step this walk serves: its objective id and
   * exported transport destination, consulted on the walk's FAILURE paths only. A
   * step can complete mid-walk — a `reach` distance check fires as the bot crosses
   * a timed gate, and the completion emission teleports it to the next area — and
   * the leg's remaining hops then fail on a position discontinuity that is
   * SUCCESS, not blockage. Passed only by step handlers whose walk targets the
   * step's own anchor; internal walks (die-retry, mob chases) carry none.
   */
  async walkTo(
    this: MineflayerExecutor,
    pos: readonly [number, number, number],
    range: number,
    label: string,
    sneak = false,
    completion?: StepCompletion,
    explicitWaypoints?: readonly Vec3Tuple[],
  ): Promise<void> {
    this.requireBot();
    // A range of exactly 0 is a block goal: a link's stand cell, which the bot
    // must be IN to be carried (spec-0083 §4). Every other range keeps its floor.
    const r = range === 0 ? 0 : Math.max(1, Math.floor(range));
    // Every walk leg starts at full health and is held there (see
    // `holdFullHealth`): whether the bot survives the walk is not what a walk
    // leg is for.
    this.walkLegs += 1;
    const outerLabel = this.walkLabel;
    this.walkLabel = label;
    try {
      await this.holdFullHealth("its start");
      await this.refreshStagedExclusion();
      const watch = !this.controlTaken();
      try {
        await this.walkLeg(pos, r, label, sneak, completion, explicitWaypoints);
        // A cutscene that took the body as the walk ended (the goal read as
        // reached from wherever the camera held it) is waited out, and the walk
        // is made again from where it returns the bot, before anything is done
        // at the goal.
        for (let n = 0; watch && this.controlTaken(); n++) {
          if (n >= MAX_CONTROL_RESUMES) {
            throw new Error(`${label}: control was taken by a cutscene ${n} times at the goal`);
          }
          await this.awaitControlEnRoute(label);
          await this.walkLeg(pos, r, label, sneak, completion, explicitWaypoints);
        }
      } catch (err) {
        const fault = this.takeLoopFault();
        if (fault) throw fault;
        throw err;
      }
      const fault = this.takeLoopFault();
      if (fault) throw fault;
    } finally {
      this.walkLegs -= 1;
      this.walkLabel = outerLabel;
    }
  },

  /** The body of {@link walkTo}, run with the bot held at full health. */
  async walkLeg(
    this: MineflayerExecutor,
    pos: readonly [number, number, number],
    r: number,
    label: string,
    sneak: boolean,
    completion: StepCompletion | undefined,
    explicitWaypoints: readonly Vec3Tuple[] | undefined,
  ): Promise<void> {
    const bot = this.requireBot();
    const movements = new Movements(bot);
    const restoreControls = configureLeg(bot, movements, sneak);
    // No cave-specific Movements override is needed — the compiler-proven
    // waypoints keep the bot on clear standable ground (the DW0311 A* treats water
    // and fences as impassable, so the route never crosses them, and gravity blocks
    // and stairs are ordinary floor). Entity detection is left ON (the pathfinder
    // default) so the bot routes AROUND a transient mob on a hop rather than ramming
    // it — disabling it made the bot wedge against a leaked mob and time out.
    // But the pathfinder's default treats EVERY non-passable entity as an
    // obstacle, including non-colliding display/interaction/marker entities that
    // block nothing in-world. Those (a completed interact objective's leaked
    // `interaction` hitbox, an NPC's co-located hitbox, floating item/text displays)
    // congested the terminal approach to an NPC and timed the leg out. Mark them
    // passable so the bot paths through them — physics-honest, and solid entities
    // (mobs, the mannequin NPC itself) are still avoided.
    allowNonCollidingEntities(movements);
    // A declared lethal volume is impassable in every route proof the
    // compiler runs, and it has to be impassable here too. Without this the walk
    // BACK from a death routes through the hazard that caused it — the bot dies a
    // second time on a leg that has nothing to do with the delve's content, and the
    // run reads as flaky rather than as a navigator that disagreed with the build.
    this.applyLethalExclusion(movements);
    bot.pathfinder.setMovements(movements);
    // Long multi-level layouts (e.g. a 5-storey keep, ~90 blocks + 4 staircases)
    // sit at the edge of the default A* budget and fail nondeterministically
    // with "No path to the goal!" — give the search real headroom. With leg-by-leg
    // waypoints each solve is tiny, so this budget is only a safety margin.
    bot.pathfinder.thinkTimeout = 30_000;
    try {
      // When the compiler proved a waypoint polyline for this leg, replay
      // it as short hops so each A* solve is trivial (avoids the single giant solve
      // that strands the bot on a large open winding cave); the final goal is always
      // the true destination. Legs are matched in lockstep path order and consumed
      // as walked; a non-matching walk (a sub-walk, or a post-transport step) does
      // not consume and falls back to the single destination goal.
      let legWaypoints: readonly Vec3Tuple[] | undefined;
      // spec-0016 §4: the timed gates that bind THIS walk. A gate is a world fact
      // the compiler exports for the whole campaign; a proven leg's `timed_gates`
      // narrows that table to the subset its route crosses. A walk with no proven
      // leg — a death-loop approach or walk back, a die-retry return, a mob or actor
      // chase — cannot narrow it and takes the declared table, so every walk crosses
      // a gate by the same rule instead of the unproven ones reading a shut gate as
      // broken geometry. See `gatesBindingWalk` for the crush withholding.
      const declaredGates = this.waypoints?.timedGates ?? [];
      let walkGates: readonly TimedGate[] = [];
      if (explicitWaypoints) {
        // A caller walking PART of a proven leg (a run-back's approach) hands
        // the proven cells itself; it consumes no leg.
        legWaypoints = explicitWaypoints;
      } else if (this.waypoints) {
        const match = nextLegWaypoints(
          this.waypoints.legs,
          this.legCursor,
          [pos[0], pos[1], pos[2]],
          this.feetCell(),
        );
        legWaypoints = match.waypoints;
        if (match.matched && legWaypoints && this.legResume?.leg === this.legCursor) {
          legWaypoints = legWaypoints.slice(this.legResume.from);
          this.legResume = undefined;
        }
        this.legCursor = match.cursor;
        // A leg's gate subset is a proof about the route from where the leg starts;
        // a walk that starts elsewhere (a die-retry return from the respawn seat) is
        // not that route, and takes the declared table.
        const binding = gatesBindingWalk(
          match.matched,
          match.timedGates,
          declaredGates,
          match.startsOnLeg,
        );
        if (match.matched && match.startOffset !== undefined) {
          process.stderr.write(
            `[leg] ${label}: starts ${match.startOffset.toFixed(1)} block(s) from where its ` +
              `leg was proven` +
              (match.startsOnLeg ? "" : ` — beyond ${LEG_START_REACH}, so not that leg's route`) +
              `\n`,
          );
        }
        walkGates = binding.gates;
        // A crush gate this walk does not bind has no staging on it, so its region
        // costs what a lethal volume costs: a pathfinder that cuts through it
        // between two proven cells meets the closing edge blind, which is how a
        // gallery walk to the east bay died at the inner door. A walk that binds
        // the gate stages it at its proven mouth instead.
        const unstaged = unstagedCrushBoxes(declaredGates, walkGates);
        if (unstaged.length > 0) {
          movements.exclusionAreasStep.push((block): number => lethalStepCost(block, unstaged));
        }
        // Stated binding count, once per walk, for every campaign that declares a
        // gate at all: how many of the declared gates bind this walk, which they
        // are, what said so, and what was withheld. A zero here is a reader's
        // finding, not something to be inferred from the absence of a line.
        if (declaredGates.length > 0) {
          process.stderr.write(
            `[timed-gate] ${label}: ${binding.gates.length} of ${declaredGates.length} ` +
              `declared gate(s) bind this walk, from ${binding.source}` +
              (binding.gates.length > 0 ? ` — ${describeGates(binding.gates)}` : "") +
              (binding.withheld.length > 0
                ? `; withheld (crush — staging needs a compiler-pinned mouth this walk ` +
                  `does not have): ${binding.withheld.map((g) => g.id).join(", ")}`
                : "") +
              `\n`,
          );
        }
      }
      // Drop proven waypoints the bot cannot physically stand on. The compiler models
      // every non-air block as a full 1×1×1 solid, so a leg may be proven by standing
      // the player on a fence-top (a legal +1 step there); vanilla physics makes a
      // fence 1.5 tall and the pathfinder marks any such block non-physical
      // (`movements.fences`), never solving a subgoal atop it — so that hop wedges.
      // Filtering it lets the pathfinder bridge the neighbouring proven cells with a
      // real-shape route (through the adjacent gate, which canOpenDoors lets it open).
      // The leg's true destination is still appended below, so connectivity — the
      // compiler's actual proof — is unchanged.
      if (legWaypoints) {
        const kept = retainStandableWaypoints(legWaypoints, (cell) =>
          this.waypointSupportStandable(cell, movements.fences),
        );
        if (kept.length !== legWaypoints.length) {
          const dropped = legWaypoints.filter((w) => !kept.includes(w));
          process.stderr.write(
            `[waypoint] ${label}: skipping ${dropped.length} proven cell(s) atop a ` +
              `non-physical block (fence/wall/closed gate) the bot cannot stand on: ` +
              `${dropped.map((d) => `[${d.join(", ")}]`).join(" ")}\n`,
          );
        }
        legWaypoints = kept;
      }
      const goalsList = walkGoals(legWaypoints, [pos[0], pos[1], pos[2]], r);
      await replayLegWithRecovery(
        goalsList,
        label,
        // Every hop of a walked leg is staged (see gotoStaged) — a body that has
        // latched onto the bot is removed and the leg resumes, so what the leg
        // reports on is the route rather than on whatever was standing in it.
        (spec, glabel) => this.gotoStaged(spec, glabel, sneak),
        (target) => this.unstickToward(target),
        walkGates.length > 0
          ? {
              gates: walkGates,
              // Both raw phases race the death signal: a bot
              // that dies mid-wait or mid-dash respawns at world spawn, and an
              // un-raced loop reads that as "clear of the fill" and marches the
              // machinery on from the wrong end of the map. Death is terminal for
              // the run — surface it, never walk it off.
              waitForWindow: (gates, hold, press) =>
                this.raceDeath(() => this.waitForGateWindow(gates, hold, press)),
              feetCell: () => this.feetCell(),
              dash: (through, from, to, budgetMs) =>
                this.raceDeath(() => this.dashThroughGate(through, from, to, budgetMs)),
            }
          : undefined,
        completion ? () => this.stepSettled(completion) : undefined,
      );
    } finally {
      restoreControls();
    }
  },

  /**
   * The {@link LegSettled} oracle for the step a walk serves: the
   * authoritative completion signals the harness already consumes, read without
   * asserting anything new.
   *   - The objective's own anchored `[dw:complete …]` marker has arrived
   *     (buffered since connect — see {@link observeMarker}); or
   *   - the step's compiler-exported completion transport has landed: the bot
   *     stands at/near the exported destination, a place only that teleport can
   *     put it mid-step (areas sit ~256 blocks apart across void, and the
   *     transport is one-way).
   * Either ⇒ the walk's purpose is fulfilled regardless of where the leg's
   * remaining hops point. Consulted on walk FAILURE paths only.
   */
  stepSettled(this: MineflayerExecutor, completion: StepCompletion): string | undefined {
    if (this.completedObjectives.has(completion.objective)) {
      return `objective ${completion.objective} is complete (its marker arrived)`;
    }
    const dest = completion.transport;
    if (dest && this.atTransportDest(dest)) {
      return (
        `the step's completion transport landed the bot at its exported ` +
        `destination [${dest[0]}, ${dest[1]}, ${dest[2]}]`
      );
    }
    return undefined;
  },

  /**
   * Whether the bot's own physical model can stand at feet cell `cell`: the block
   * directly below it must NOT be one mineflayer-pathfinder classifies non-physical
   * — a fence, wall, or closed fence-gate, whose collision shape is taller than 1
   * and which lives in `movements.fences`. This is the pathfinder's own standability
   * criterion, reused verbatim, so the waypoint replay never issues a subgoal the
   * pathfinder itself cannot stand at (the compiler's full-solid model proved the
   * cell standable; the bot's real-shape physics disagrees only for these blocks).
   * A cell whose support chunk is not loaded reads as standable (we only ever DROP a
   * waypoint we can positively prove un-standable; the pathfinder resolves the rest).
   * Uses the `position.offset` idiom (as {@link collect}) to build the absolute
   * support cell without importing Vec3.
   */
  waypointSupportStandable(this: MineflayerExecutor, cell: Vec3Tuple, fences: Set<number>): boolean {
    const bot = this.requireBot();
    const p = bot.entity.position;
    const support = bot.blockAt(p.offset(cell[0] - p.x, cell[1] - 1 - p.y, cell[2] - p.z));
    if (!support) return true; // support unknown (chunk not loaded) → keep the waypoint
    return !fences.has(support.type);
  },

  /** The bot's current feet cell (floored block position), or `undefined` if the bot
   * is not connected. Read-only observation, used only to decide whether a timed-gate
   * retry must retreat to a standoff first. */
  feetCell(this: MineflayerExecutor): Vec3Tuple | undefined {
    const bot = this.bot;
    if (!bot?.entity) return undefined;
    const p = bot.entity.position;
    return [Math.floor(p.x), Math.floor(p.y), Math.floor(p.z)];
  },

  /**
   * Raw, pathfinder-free nudge toward `target` to dislodge a physically wedged
   * bot. When the stall-recovery pathfind itself can't escape a concave corner
   * beside a wall, this bypasses the A* pathfinder: clear controls, face the target
   * cell, and drive forward for a SHORT burst — a gentle tap, not a launch, so on a
   * tight 2-wide corridor the bot edges toward the corridor axis instead of
   * overshooting to the far wall. Only if that gentle burst makes no progress (the
   * bot is truly stuck against a lip) does it add a jump. It deliberately does NOT
   * call `pathfinder.stop()`: the previous hop already returned, and stopping here
   * churns pathfinder state and interrupts the caller's very next `goto` ("Path was
   * stopped"). Navigation robustness, NOT game logic; the caller re-paths afterwards
   * and still fails loudly if the hop stays unwalkable. Returns the blocks moved so
   * the caller can adapt aim: a near-zero move means the target lies through a wall.
   */
  async unstickToward(this: MineflayerExecutor, target: GoalSpec): Promise<number> {
    const bot = this.requireBot();
    bot.clearControlStates();
    // Face the block-centre of the target cell so the forward drive heads toward it.
    const p0 = bot.entity.position;
    try {
      await bot.lookAt(p0.offset(target.x + 0.5 - p0.x, 0, target.z + 0.5 - p0.z), true);
    } catch {
      // best effort — an unforced look failure must not abort the unstick
    }
    const before = bot.entity.position.clone();
    bot.setControlState("forward", true);
    await delay(UNSTICK_BURST_MS);
    // Jump only when the gentle forward burst got nowhere (wedged against a lip).
    if (bot.entity.position.distanceTo(before) < UNSTICK_MIN_PROGRESS) {
      bot.setControlState("jump", true);
      await delay(UNSTICK_BURST_MS);
      bot.setControlState("jump", false);
    }
    bot.setControlState("forward", false);
    bot.clearControlStates();
    await delay(UNSTICK_SETTLE_MS);
    return bot.entity.position.distanceTo(before);
  },

  async runGoto(this: MineflayerExecutor, spec: GoalSpec, label: string): Promise<void> {
    const bot = this.requireBot();
    const { x, y, z, range } = spec;
    // Already within the goal? Return without pathfinding. mineflayer-pathfinder
    // rejects a `goto` issued when the bot already sits at the target with "Path was
    // stopped before it could be completed" (after a physics-unstick lands
    // the bot inside a hop's range, the retry `goto` would otherwise fail spuriously
    // on a goal that is in fact already satisfied).
    if (this.withinGoal(spec)) {
      return;
    }
    let lastErr: unknown;
    let resumes = 0;
    for (let attempt = 0; attempt < 2; attempt++) {
      if (attempt > 0) {
        await delay(1_500);
      }
      try {
        // A body already held as the walk starts (a cutscene that began while
        // the bot stood still — a gate stager's wait, the sequencer's) is waited
        // out against the step's declaration before the walk is made; the walk
        // is then watched only if it starts with the body in hand.
        if (this.controlTaken()) await this.awaitHeldAtStart(label);
        const watch = !this.controlTaken();
        // Through the navigation owner, never `bot.pathfinder.goto` directly: this
        // wait is abandoned on a death and on the timeout, and an abandoned trip's
        // later rejection has to land on a handler that already exists.
        await this.raceDeath(() =>
          withTimeout(
            watch
              ? this.raceControl(this.nav.goto(new goals.GoalNear(x, y, z, range)))
              : this.nav.goto(new goals.GoalNear(x, y, z, range)),
            REACH_TIMEOUT_MS,
            `reaching ${label}`,
          ),
        );
        if (this.withinGoal(spec)) {
          return;
        }
        throw new Error(
          `pathfinder resolved but the bot is at ${fmt(bot.entity.position)}, ` +
            `not within ${range} of the goal`,
        );
      } catch (err) {
        // A death is terminal for this run — never retry a path across the void.
        if (err instanceof BotDeathError) throw err;
        // A cutscene took the body: wait it out against the step's declared
        // allowance, then walk again from wherever it put the bot back. Not an
        // attempt — the walk did not fail, it was interrupted.
        if (err instanceof ControlTakenError) {
          this.stopPathfinding();
          resumes += 1;
          if (resumes > MAX_CONTROL_RESUMES) {
            throw new Error(
              `${label}: control was taken by a cutscene ${resumes} times in one walk — ` +
                `something on the way re-fires every time control returns`,
            );
          }
          await this.awaitControlEnRoute(label);
          attempt -= 1;
          continue;
        }
        lastErr = err;
        // Clear the pathfinder for the retry — including the internal stop flag, which
        // would otherwise make the retry (and every later hop) reject instantly without
        // walking a step. See {@link stopPathfinding}.
        this.stopPathfinding();
      }
    }
    const detail = lastErr instanceof Error ? lastErr.message : String(lastErr);
    const near = Object.values(bot.entities)
      .filter((e) => e && e !== bot.entity && bot.entity.position.distanceTo(e.position) < 12)
      .map((e) => ({
        name: e.name ?? "?",
        distance: e.position.distanceTo(bot.entity.position),
      }));
    // Classified by the pathfinder's OWN passable set, not by a list kept here —
    // a raw dump of the neighbourhood names bodies the search never even indexed,
    // and reads as an accusation. See `describeStuckNeighbours`.
    const passable = (bot.pathfinder.movements as { passableEntities?: Set<string> } | undefined)
      ?.passableEntities;
    process.stderr.write(
      `[stuck] near ${fmt(bot.entity.position)}: ${describeStuckNeighbours(near, passable)}\n`,
    );
    throw new Error(
      `failed ${label} at [${x}, ${y}, ${z}] (range ${range}); bot at ` +
        `${fmt(bot.entity.position)}: ${detail}`,
    );
  },

  /**
   * Whether the bot's block position is within `spec.range` blocks of the goal cell
   * — the same block-distance metric mineflayer-pathfinder's `GoalNear` uses to
   * decide it arrived. The `y` axis is given one extra block of slack so standing on
   * a stair/slab (a fractional-height floor) still counts as arrived.
   */
  withinGoal(this: MineflayerExecutor, spec: GoalSpec): boolean {
    const p = this.requireBot().entity.position;
    const dx = Math.floor(p.x) - spec.x;
    const dz = Math.floor(p.z) - spec.z;
    const dy = Math.floor(p.y) - spec.y;
    const yTol = spec.range + 1;
    return dx * dx + dz * dz <= spec.range * spec.range && Math.abs(dy) <= yTol;
  },
};
