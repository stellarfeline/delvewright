// mineflayer-backed StepExecutor. Connects a headless bot to a pinned 1.21.11
// server and drives the critical path against the amended bot-interaction
// contract (spec-0002, 2026-07-30).
//
// Interaction channel (settled): Minecraft 1.21.6+ routes NPC dialogue / class
// selection through the server-driven dialog system. mineflayer 4.37.x exposes no
// high-level dialog API and cannot reliably emit a dialog button click, so the
// compiler emits every dialog button as a `run_command` firing a `/trigger`, and
// the bot drives the same outcome by chatting that exact command (`bot.chat`).
// select-class / talk-to therefore just send `step.command`; talk-to walks to the
// NPC first (realism + reach mechanics that some dialogs gate on).

import { createBot, type Bot } from "mineflayer";
import type { Entity } from "prismarine-entity";
// mineflayer-pathfinder is CommonJS; import the default and destructure (the only
// harness dependency added for v0.3 — replaces the naive "face + hold forward"
// walk so turns/branches in jigsaw layouts are walkable).
import pathfinderPkg from "mineflayer-pathfinder";
const { pathfinder, Movements, goals } = pathfinderPkg;
import type {
  AssertCompleteStep,
  CollectStep,
  InteractStep,
  KillStep,
  ReachStep,
  RestStep,
  SelectClassStep,
  TalkToStep,
  Vec3Tuple,
} from "./critical-path.ts";
import type { StepExecutor } from "./sequencer.ts";
import { BotDeathError, likelyDeathCause } from "./death.ts";
import {
  AssistLedger,
  assistClearCommand,
  assistCommand,
  assistPolicy,
  deathPhases,
  floorFinding,
  checkpointPreconditionFinding,
  observationOf,
  openTrial,
  respawnedAtCheckpoint,
  retryOutcome,
  scriptedDeathCommand,
  type AssistWindow,
  type CombatPlan,
  type DeathTrial,
  type DeathTrialRecord,
  type Encounter,
  type EncounterPhase,
  type PerformedRest,
  type ReengageObservation,
  type WaveSighting,
} from "./combat.ts";
import { presentAndTrigger } from "./held-item.ts";
import { CAMPAIGN_TOKEN, markerLine, parseCompletionMarker } from "./markers.ts";
import { allowNonCollidingEntities, configureLeg } from "./movement.ts";
import {
  nextLegWaypoints,
  retainStandableWaypoints,
  walkGoals,
  type GoalSpec,
  type TimedGate,
  type Waypoints,
} from "./waypoints.ts";
import {
  GATE_MIN_ATTEMPTS,
  GATE_POLL_MS,
  describeGates,
  gateRegionCells,
  gateRetryBudgetMs,
  gateWindowWaitMs,
  needsStandoff,
} from "./timed-gate.ts";
import type { Item } from "prismarine-item";
import {
  ATTRIBUTION_RANGE,
  RETALIATION_RANGE,
  STALKER_RANGE,
  THREAT_WINDOW_MS,
  ThreatTracker,
  attributeBotDamage,
  pickRetaliationTarget,
  pickStalker,
  type ThreatCandidate,
} from "./threat.ts";
import {
  EAT_COOLDOWN_MS,
  EAT_SAFE_RANGE,
  eatDecision,
  isSafeFood,
  pickFood,
} from "./sustain.ts";
import {
  WAVE_CLEAR_STREAK,
  WAVE_ENGAGE_NEAR,
  beginWave,
  creditsWaveKill,
  waveEngagementCleared,
  type WaveEngagement,
} from "./wave.ts";

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
 * Replay a leg's ordered goals with **stall-recovery** (task #45). Each `goto`
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
): Promise<void> {
  const gates = gate?.gates ?? [];
  let lastProven: GoalSpec | undefined;
  for (let g = 0; g < goalsList.length; g++) {
    const spec = goalsList[g]!;
    const last = g === goalsList.length - 1;
    const glabel = last ? label : `${label} waypoint ${g + 1}/${goalsList.length}`;
    try {
      await goto(spec, glabel);
    } catch (err) {
      if (err instanceof BotDeathError) throw err;
      // A leg the compiler proved walks THROUGH a timed gate (spec-0016 §4) gets the
      // window wait; every other leg keeps the old behaviour exactly, so a real
      // navigation regression still fails on the first stall.
      if (gate && gates.length > 0) {
        await crossTimedGate(spec, glabel, lastProven, gate, goto, unstick, err);
      } else {
        if (!lastProven) throw err; // nothing proven yet — not the pocket-wedge class
        await recoverAndRetry(spec, glabel, lastProven, goto, unstick);
      }
    }
    lastProven = spec;
  }
}

/**
 * The bot-facing half of a timed-gate crossing, injected by the executor so the
 * control flow above stays unit-testable without a live server.
 */
export interface GateAssist {
  /** The gates the compiler proved this leg's route crosses (empty ⇒ no assist). */
  readonly gates: readonly TimedGate[];
  /**
   * Block (bounded) until `gates` are observed to go from closed to OPEN, so the
   * crossing begins at the top of a window rather than its tail. Resolves early if
   * the edge cannot be observed — the caller then simply tries the hop.
   */
  readonly waitForWindow: (gates: readonly TimedGate[]) => Promise<void>;
  /** The bot's current feet cell, or `undefined` when it cannot be read. */
  readonly feetCell: () => Vec3Tuple | undefined;
  /** Injectable clock (tests). */
  readonly now?: () => number;
}

/**
 * Retry a hop that a `timed-gate` clock can interrupt (spec-0016 §4).
 *
 * mineflayer-pathfinder has no concept of a window: when the gate region fills
 * mid-approach it aborts the path ("Path was stopped before it could be completed!")
 * and, before this, the leg failed as though the geometry were broken. The compiler
 * already proves the crossing is READABLE (DW0378: ≥20% of every cycle admits it);
 * this teaches the runtime rung the same verb.
 *
 * Each attempt is: **stand off** — only when the bot is standing IN the fill (see
 * {@link needsStandoff}); every retreated block has to be re-walked inside the open
 * window, and DW0378's proof covers the gate SPAN, not an arbitrary run-up to it, so
 * a bot already clear waits exactly where it stands — then **wait** for the
 * closed→open edge, then re-run the hop, escalating to the ordinary task-#45
 * stall recovery inside the same window (the pathfinder loses a path whose blocks
 * are rewritten under it; a walk does not). The loop is bounded by
 * {@link gateRetryBudgetMs} (two full cycles + margin) and {@link GATE_MIN_ATTEMPTS};
 * once the budget is spent it makes one final full physical recovery (in case the
 * hop was never about the clock) and then fails loudly, naming the gate and its
 * cycle.
 *
 * This is bounded patience for legs the compiler MARKED, never a blanket retry: an
 * unmarked leg is untouched, and a marked leg that is genuinely unwalkable still
 * fails — the check is not weakened, only told what a gate is.
 */
async function crossTimedGate(
  spec: GoalSpec,
  glabel: string,
  proven: GoalSpec | undefined,
  gate: GateAssist,
  goto: (spec: GoalSpec, label: string) => Promise<void>,
  unstick: Unstick | undefined,
  firstErr: unknown,
): Promise<void> {
  const gates = gate.gates;
  const now = gate.now ?? (() => Date.now());
  const budget = gateRetryBudgetMs(gates);
  const start = now();
  let lastErr = firstErr;
  let attempt = 0;
  process.stderr.write(
    `[timed-gate] ${glabel} was interrupted by ${describeGates(gates)}; waiting for a ` +
      `window (budget ${(budget / 1_000).toFixed(1)}s, min ${GATE_MIN_ATTEMPTS} attempts)\n`,
  );
  while (attempt < GATE_MIN_ATTEMPTS || now() - start < budget) {
    attempt++;
    if (proven && needsStandoff(gate.feetCell(), gates)) {
      process.stderr.write(
        `[timed-gate] standing off to [${proven.x}, ${proven.y}, ${proven.z}] — the bot ` +
          `is inside the gate's fill\n`,
      );
      await reached(() => goto({ ...proven, range: 1 }, `${glabel} gate standoff`));
    }
    await gate.waitForWindow(gates);
    const alabel = `${glabel} gate attempt ${attempt}`;
    try {
      await goto(spec, alabel);
      return;
    } catch (err) {
      if (err instanceof BotDeathError) throw err;
      lastErr = err;
      // Every attempt's own reason is logged, not just the last one: a run where the
      // bot never moved and a run where it crossed and was cut off look identical in
      // a single terminal message, and telling them apart is the whole diagnosis.
      process.stderr.write(
        `[timed-gate] attempt ${attempt} pathfind failed: ` +
          `${err instanceof Error ? err.message : String(err)}\n`,
      );
    }
    // Escalate INSIDE the window (observed on the-drowned-bell's portcullis): the
    // pathfinder walks the bot to the cell at the gate's mouth and then aborts with
    // "Path was stopped before it could be completed!", every window, while the raw
    // physics burst walks the very same span in a fraction of a second. A path whose
    // blocks are rewritten under it twice per cycle is not something A* can hold on
    // to; the last blocks of a clocked span have to be crossed by walking. That is
    // the ordinary task-#45 stall escalation (pathfind → look-and-walk burst →
    // re-path), reused verbatim — plain movement a human player makes, still
    // bounded, and it still has to physically get through an open gate.
    if (proven) {
      try {
        await recoverAndRetry(spec, alabel, proven, goto, unstick);
        return;
      } catch (err) {
        if (err instanceof BotDeathError) throw err;
        lastErr = err;
        process.stderr.write(
          `[timed-gate] attempt ${attempt} physical crossing failed: ` +
            `${err instanceof Error ? err.message : String(err)}\n`,
        );
      }
    }
  }
  // Budget spent. Before calling it a failure, give the hop the ordinary physical
  // recovery — the gate mark says a clock CAN interrupt this leg, not that every
  // failure on it is the clock's doing.
  try {
    if (proven) {
      await recoverAndRetry(spec, glabel, proven, goto, unstick);
    } else {
      await goto(spec, glabel);
    }
    return;
  } catch (err) {
    if (err instanceof BotDeathError) throw err;
    lastErr = err;
  }
  const detail = lastErr instanceof Error ? lastErr.message : String(lastErr);
  throw new Error(
    `${glabel}: still blocked after ${attempt} timed-gate crossing attempt(s) over ` +
      `${((now() - start) / 1_000).toFixed(1)}s — more than two full cycles of ` +
      `${describeGates(gates)}. The window is not the problem; this is a real ` +
      `navigation failure: ${detail}`,
  );
}

/** Try a `goto`, returning whether it arrived; a bot death still propagates. */
async function reached(fn: () => Promise<void>): Promise<boolean> {
  try {
    await fn();
    return true;
  } catch (err) {
    if (err instanceof BotDeathError) throw err;
    return false;
  }
}

/**
 * Recover from a stalled hop and retry it (task #45). Level 1: re-path to the last
 * proven cell (range 0) to re-centre on the polyline, then retry the hop. Level 2
 * (the wedge defeats the pathfinder too): a bounded physics {@link Unstick} to break
 * the bot free, retrying the ACTUAL hop at its own range after each burst. A hop
 * still unwalkable after the budget fails loudly.
 *
 * Level-2 aim is **adaptive** (trace-derived, task #45): drive toward the GOAL for
 * forward progress, but if a burst measured no progress the bot is wall-blocked (the
 * goal lies through the concave-corner wall) — the next burst aims at the PROVEN cell
 * instead, the open away-from-wall direction that escapes the pocket. Neither fixed
 * direction alone works: goal-only can't escape the initial pocket (drives into the
 * wall), proven-only shoves an already-advanced bot backward and oscillates.
 */
async function recoverAndRetry(
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

/** Connection + identity for the bot. Sourced from the environment (see below). */
export interface BotConfig {
  readonly host: string;
  readonly port: number;
  readonly username: string;
  /** Pinned per ADR-0009; mineflayer's max supported version is 1.21.11. */
  readonly version: string;
  /** `offline` for local/CI (offline-mode server); `microsoft` for real accounts. */
  readonly auth: "offline" | "microsoft";
}

/** The pinned Minecraft version (ADR-0009). Single source of truth for the harness. */
export const PINNED_MC_VERSION = "1.21.11";

/**
 * Build a {@link BotConfig} from environment variables, with local-testing
 * defaults:
 *   DELVEWRIGHT_MC_HOST      (default `127.0.0.1`)
 *   DELVEWRIGHT_MC_PORT      (default `25565`)
 *   DELVEWRIGHT_BOT_USERNAME (default `delve-bot`)
 *   DELVEWRIGHT_MC_VERSION   (default `1.21.11`, the ADR-0009 pin)
 *   DELVEWRIGHT_MC_AUTH      (`offline` | `microsoft`, default `offline`)
 */
export function botConfigFromEnv(
  env: Record<string, string | undefined> = process.env,
): BotConfig {
  const portRaw = env["DELVEWRIGHT_MC_PORT"] ?? "25565";
  const port = Number.parseInt(portRaw, 10);
  if (!Number.isInteger(port) || port <= 0 || port > 65535) {
    throw new Error(
      `DELVEWRIGHT_MC_PORT must be a valid TCP port, got ${JSON.stringify(portRaw)}`,
    );
  }
  const authRaw = env["DELVEWRIGHT_MC_AUTH"] ?? "offline";
  if (authRaw !== "offline" && authRaw !== "microsoft") {
    throw new Error(
      `DELVEWRIGHT_MC_AUTH must be 'offline' or 'microsoft', got ${JSON.stringify(authRaw)}`,
    );
  }
  return {
    host: env["DELVEWRIGHT_MC_HOST"] ?? "127.0.0.1",
    port,
    username: env["DELVEWRIGHT_BOT_USERNAME"] ?? "delve-bot",
    version: env["DELVEWRIGHT_MC_VERSION"] ?? PINNED_MC_VERSION,
    auth: authRaw,
  };
}

/** How long (ms) a movement step may run before it is declared failed. */
const REACH_TIMEOUT_MS = 60_000;
/** Polling interval (ms) while walking toward a target. */
const REACH_POLL_MS = 250;
/** How long (ms) a `kill` step may run before it is declared failed. */
const KILL_TIMEOUT_MS = 90_000;
/** Attack cadence (ms) — roughly the vanilla sword cooldown. */
const ATTACK_INTERVAL_MS = 400;
/**
 * How long (ms) the die-retry stage trades blows before taking its `mid-fight`
 * scripted death (spec-0023 §1). Short on purpose: the point is that the wave has
 * been ENGAGED — some mobs hurt, the fight's state dirty — when the death lands,
 * because that is the state a respawn has to restore. Winning the fight here would
 * defeat the trial.
 */
const MID_FIGHT_MS = 6_000;
/**
 * How long (ms) the bot may melee a single target, in range, without it dying before
 * that target is deemed unkillable and blacklisted (task: bot-kill-hunt). A living
 * wave mob dies in a few sword swings; an Invulnerable story actor summoned into the
 * combat area — a `minecraft:warden` posing as Polyphemus, a `minecraft:mannequin`
 * class-post puppet — never dies, so without this the `nearestEntity` target selection
 * fixates on it and the wave never "clears" (a 90s timeout). The window is generous
 * (far longer than any wave mob survives) so a legitimately tanky mob is never
 * mis-blacklisted; it only ever catches a truly unkillable entity. NOT a threshold on
 * the kill objective — the wave still requires every real mob dead.
 */
const WAVE_UNKILLABLE_MS = 6_000;
// (the wave-kill proximity rule and its constant now live in wave.ts, shared with the
// self-defense path — see the import above.)
/**
 * How long (ms) to wait for a scoreboard value to reach its target after a chat
 * command. The datapack acts on the trigger on the next server tick(s); give it a
 * generous window so slow CI servers don't flake.
 */
const SCORE_SETTLE_MS = 15_000;
const SCORE_POLL_MS = 250;
/**
 * How long (ms) to wait for a step's OWN objective-completion marker after the bot
 * has done the thing the step asks for (AUDIT-P0). The datapack completes an
 * objective on the tick its condition holds and broadcasts the marker in the same
 * function, so the honest wait is a tick or two; the window is wide enough that a
 * loaded CI server, a lagging advancement or a wave countdown settling can never
 * flake, and short enough that a genuinely uncompletable objective fails the run
 * well inside the wall-clock budget. NOT a tolerance: on expiry the step FAILS.
 */
const OBJECTIVE_TIMEOUT_MS = 30_000;
/**
 * Settle (ms) after an objective's marker before the next step runs, so effects the
 * objective fires (open a gate, give an item, move an NPC) have landed. The marker
 * is broadcast as the score flips — deliberately, so completion timing is exact —
 * which means the effects that follow it in the same function may not have applied
 * yet.
 */
const EFFECT_SETTLE_MS = 1_000;
/** Settle time (ms) after class selection (teleport + kit give) before moving on. */
const CLASS_SETTLE_MS = 3_000;
/**
 * Physics-unstick (task #45): a SHORT forward tap per burst (~a cell, not a launch —
 * a long burst overshoots a tight 2-wide corridor and oscillates wall-to-wall), a
 * jump only when a gentle burst moved less than `UNSTICK_MIN_PROGRESS` blocks (truly
 * wedged against a lip), and a brief settle before re-pathing.
 */
const UNSTICK_BURST_MS = 250;
const UNSTICK_MIN_PROGRESS = 0.5;
const UNSTICK_SETTLE_MS = 300;
/**
 * gap 8: how long (ms) to wait for a cross-area teleport to land after a step whose
 * completion relocates the player, how close (blocks, horizontal) counts as
 * "arrived at the destination", and how long to settle once it has.
 */
const TRANSPORT_TIMEOUT_MS = 15_000;
const TRANSPORT_NEAR = 4;
const TRANSPORT_SETTLE_MS = 1_500;
/**
 * gap 8 (task #32): a server-forced position jump of at least this many blocks
 * (horizontal) is treated as a cross-area teleport rather than knockback or a
 * within-area nudge. Areas sit ~256 blocks apart across void (an unambiguous jump);
 * in-area relocations (spawn, class teleport) stay well under this, so the threshold
 * cleanly separates a transport from ordinary forced moves. When one is observed the
 * pathfinder is reset, so a path computed in the OLD area cannot survive the jump and
 * strand the next step with a spurious "No path to the goal!".
 */
const TRANSPORT_JUMP_BLOCKS = 64;
/**
 * gap 8 (task #32): after the jump lands, how long (ms) to wait for the destination
 * chunk to load and the bot to come to rest on solid ground before the next step
 * starts pathfinding, and the poll cadence. The pathfinder's A* fails immediately
 * ("No path to the goal!") if it starts while the block under the bot is still
 * unloaded (`blockAt` → null) — the race this closes. Bounded: on timeout the wait
 * settles and lets the next step surface its own diagnostic.
 */
const FOOTING_TIMEOUT_MS = 10_000;
const FOOTING_POLL_MS = 100;
/**
 * gap 7 (cutscene): how long (ms) the bot's position must hold steady, once it is
 * back in adventure mode, before control counts as restored; how far (blocks) a
 * position may drift and still count as "steady"; and the grace added on top of the
 * declared cutscene length before the wait gives up and continues (bounded so a
 * cutscene glitch cannot hang the run). Grace is env-tunable for tests.
 */
const CUTSCENE_SETTLE_MS = 500;
const CUTSCENE_STEADY_EPS = 0.05;
const CUTSCENE_POLL_MS = 250;
/** gap 7 (retry): how long (ms) to wait for the bot to respawn before resuming. */
const RESPAWN_TIMEOUT_MS = 15_000;

/** How often the respawn wait re-reads the spawn counter. */
const SPAWN_POLL_MS = 50;

/**
 * How long the die-retry re-engage probe waits for the encounter to show itself
 * before concluding nothing is there.
 *
 * The probe used to be ONE instantaneous sample taken the moment the walk back
 * resolved, and that is a sampling bug, not an observation: a client learns about
 * an entity when the server sends it, which takes ticks after arrival —
 * `fightWave` has always slept a second on arrival for exactly this reason. On
 * nobodys-cave-island r14 three demonstrably-alive drowned (feral, follow_range
 * 48, wandered off the anchor after killing the bot) read as "no hostile was
 * there to fight" and reddened both trials of a healthy encounter.
 *
 * Generous on purpose, and it costs nothing on a healthy run: the probe returns
 * the instant the declared wave is standing. Soundness of looking client-side at
 * all rests on vanilla's own numbers — a monster is tracked out to 8 chunks (128
 * blocks) while `follow_range` tops out far below that, so every mob that could
 * still come for the party is a mob the client can see.
 */
const REENGAGE_SETTLE_MS = 6_000;

/** How far from a rest step's anchor cell its `interaction` affordance may sit. */
const AFFORDANCE_RADIUS = 3;

/** Grace between the bonfire click and the trigger command: the opener runs as an
 * advancement reward, so `dw.rest` is enabled a tick or two after the click. */
const REST_OPEN_SETTLE_MS = 500;
/** Recent chat lines retained for death-cause diagnosis. */
const CHAT_BUFFER = 16;
/**
 * Self-defense (souls ladder, the-drowned-bell): how long (ms) the bot may spend
 * killing a stalker that interrupted a NAVIGATION leg before it gives up, reports, and
 * resumes walking. A delve mob dies in a handful of swings; this window is many times
 * that, so it only ever expires on something the bot genuinely cannot kill (an
 * Invulnerable actor, a mob it cannot reach) — and even then the leg continues, so the
 * budget can never turn a content problem into a navigation failure.
 */
const DEFEND_BUDGET_MS = 12_000;
/**
 * How many times a single hop may be interrupted for self-defense before it is walked
 * regardless. Bounded so a pack of mobs cannot livelock a leg; the wave-fight path
 * (which has its own 90s budget) is where a real fight belongs.
 */
const DEFENSE_ROUNDS_PER_HOP = 3;
/**
 * How often (ms) a walking leg re-checks whether a stalker has latched onto the bot.
 * A backstop only: the check also runs on the damage event itself, so a mob that hits
 * the bot is reacted to on the packet rather than up to a poll later. It matters — a
 * Hollow Gate-Warder swinging an iron axe takes ~7 of the bot's 20 hit points per hit
 * on `easy`, so three hits is the whole margin.
 */
const THREAT_POLL_MS = 200;
/**
 * How long (ms) to let an interrupted `goto` settle before swinging. The pathfinder
 * halts at the next path node, so a moment's grace stops it dragging the bot out of
 * melee mid-fight — bounded tightly, because the bot is being hit while it waits.
 */
const WALK_SETTLE_MS = 300;
/**
 * How long (ms) after a damage packet with a named source the health-drop fallback
 * stays quiet. mineflayer emits `entityHurt` (from `damage_event`) and the health
 * update from the same server tick batch; this grace stops one hit being counted twice
 * — once attributed, once guessed.
 */
const HEALTH_ATTRIBUTION_GRACE_MS = 500;
/**
 * Vanilla player maxima on 1.21.11. A delve's class kit changes gear, never these
 * attributes, so they are constants rather than a per-run read.
 */
const PLAYER_MAX_HEALTH = 20;
const PLAYER_MAX_FOOD = 20;

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/** Reject with a labelled error if `promise` does not settle within `ms`. */
function withTimeout<T>(promise: Promise<T>, ms: number, what: string): Promise<T> {
  let timer: ReturnType<typeof setTimeout>;
  const guard = new Promise<never>((_, reject) => {
    timer = setTimeout(() => reject(new Error(`${what}: timed out after ${ms}ms`)), ms);
  });
  return Promise.race([promise, guard]).finally(() => clearTimeout(timer));
}

/**
 * A mineflayer-backed executor. Construct, `await connect()`, then hand it to
 * `runSequence`. `close()` disconnects the bot. Not reusable across servers.
 */
export class MineflayerExecutor implements StepExecutor {
  private readonly config: BotConfig;
  private bot: Bot | undefined;
  /**
   * The campaign whose markers this run accepts (from `critical-path.json`).
   * Markers naming any other campaign are ignored — a completion belonging to other
   * content can never satisfy this run's steps.
   */
  private campaignId: string | undefined;
  /**
   * Objective ids whose anchored completion marker has arrived, and the 0-based
   * step index that was executing when it did. Buffered from connect, because an
   * objective often completes DURING its step's walk (before the executor gets to
   * wait for it) and campaign completion lands during the last objective step.
   */
  private readonly completedObjectives = new Map<string, number>();
  /**
   * The step index at which the campaign-completion marker arrived, if it has.
   * Endgame discipline: campaign completion belongs to the LAST objective step; its
   * arrival any earlier means the path is incoherent (a branch completed the
   * campaign while steps remained) and the run is failed on the spot rather than
   * marching through hollow remaining steps.
   */
  private campaignCompleteAtStep: number | undefined;
  /** The step index currently executing, for marker attribution. */
  private currentStep = -1;
  /**
   * gap 7 (death): set once when the bot dies; long waits race against it so a death
   * fails FAST with a diagnostic instead of respawning and pathfinding across the void.
   */
  private death: BotDeathError | undefined;
  /**
   * How many deaths this run has observed. The die-retry stage waits for a FRESH
   * death rather than for `this.death` to be set, so a leftover latch (the bot
   * died on the way back from the last one) can never be mistaken for the next
   * scripted death and credited as a trial that never happened.
   */
  private deathSeq = 0;
  /** How many `spawn` events this run has seen (login, then every respawn). */
  private spawnSeq = 0;
  /** {@link spawnSeq} at the moment of the last death — the respawn wait watches
   * for a spawn NEWER than this, so a respawn that beats the wait is never lost. */
  private spawnSeqAtDeath = 0;
  /** One-shot callbacks armed by {@link raceDeath}, fired on death. */
  private readonly deathWaiters = new Set<(err: BotDeathError) => void>();
  /** Ring buffer of recent chat lines, mined for the death-cause message. */
  private readonly recentChat: string[] = [];
  /**
   * gap 8 (task #32): the bot position captured at the previous server-forced move
   * (`forcedMove`). A forced move whose horizontal delta from this reaches
   * {@link TRANSPORT_JUMP_BLOCKS} is a cross-area teleport; used to reset the
   * pathfinder so a path computed in the old area cannot survive the jump.
   */
  private lastForcedPos: { x: number; y: number; z: number } | undefined;
  /** Grace (ms) added onto a cutscene's declared length before giving up. */
  private readonly cutsceneGraceMs: number;
  /**
   * task #38: the compiler's proven per-leg critical-path waypoints (keyed by
   * destination anchor). When a walked step's target has a leg here, `walkTo`
   * replays it as successive nearby goals so each mineflayer A* solve is trivial —
   * instead of one distant goal that strands the bot on a large open winding cave.
   * Absent → the original single distant-goal behavior (fallback). Compiler-proven
   * navigation data, not a route the harness computes.
   */
  private waypoints: Waypoints | undefined;
  /**
   * spec-0023: the compiler's combat plan — which encounters are mandatory, what
   * the content bills each as, and which checkpoint governs a death at it.
   * Absent (a delve with no mandatory combat, or an older build) → `kill` behaves
   * exactly as it did before spec-0023, assists and die-retry included out.
   */
  private combatPlan: CombatPlan | undefined;
  /** Whether the die-retry ladder stage runs. */
  private dieRetry = false;
  /** Every combat-assist window this run opened (spec-0023 §3 run artifact). */
  private readonly assists = new AssistLedger();
  /** Every scripted death and what it proved about the retry loop. Entries are
   * appended when the death is TAKEN and mutated as the loop yields facts, so an
   * aborted run still carries the death it took (task #102). */
  private readonly trials: DeathTrialRecord[] = [];
  /** Waves the die-retry stage entered, whether or not it finished with them.
   * Engagement without records is the silence the run report must not keep. */
  private readonly dieRetryEngaged = new Set<string>();
  /** Every `rest` step the PATH declares, and which of them the bot performed —
   * the die-retry precondition reads both (compiler #220). Declared up front
   * rather than accumulated as they run, so "the route passed this fire without
   * resting" is a statement the check can actually make. */
  private restSteps: readonly PerformedRest[] = [];
  private readonly restedBonfires = new Set<number>();
  /** Encounters whose scripted deaths were SKIPPED because no checkpoint was armed. */
  private readonly preconditionFindings: string[] = [];
  private readonly preconditionWaves = new Set<string>();
  /** Highest health ever observed per `<wave>/<mob name>` — the stand-in for a
   * max-health attribute vanilla never puts on the wire (see fullHealthOf). */
  private readonly waveFullHealth = new Map<string, number>();
  /** How far `kill()` got with each encounter — the reading key for an empty
   * `assist_windows` array (spec-0023 takes no assist while deliberately dying,
   * nor on a billed encounter's honest first attempt). */
  private readonly encounterPhases = new Map<string, EncounterPhase>();
  /** Inverted floor gate findings: billed fights the unassisted bot beat cold. */
  private readonly floorFindings: string[] = [];
  /** The last `select-class` step, replayed to re-arm after a scripted death. */
  private lastSelectClass: SelectClassStep | undefined;
  /**
   * task #38: how many walked legs have been consumed. Legs are matched in lockstep
   * path order (not by destination coordinate), so an anchor visited more than once
   * — e.g. the cave entry the player returns to — never grabs the wrong leg's route.
   */
  private legCursor = 0;
  /**
   * Who has been hitting the bot lately (see threat.ts). Feeds two behaviours a player
   * has and the bot did not: hitting back at whatever is drawing blood during a wave
   * fight, and stopping a navigation leg for a mob that has latched on.
   */
  private readonly threats = new ThreatTracker();
  /** Timestamp (ms) of the last damage attributed from a packet-named source. */
  private lastAttributionAt = 0;
  /** Last observed bot health, for the health-drop attribution fallback. */
  private lastHealth: number | undefined;
  /**
   * Entities a full self-defense budget failed to kill. They stay threats (the bot
   * still knows they hit it) but never interrupt another navigation leg — otherwise an
   * unkillable stalker would stop every hop forever. Reported when first written off.
   */
  private readonly defenseExempt = new Set<number>();
  /** Timestamp (ms) of the last eat attempt, throttling both the action and its log. */
  private lastEatAt = 0;
  /**
   * Armed fight-or-flight watchers (see {@link armStalkerTrip}). Fired from the damage
   * handler so the bot reacts on the hit that qualifies a stalker, not up to a poll
   * later; the poll remains as a backstop for a mob that closes without hitting again.
   */
  private readonly stalkerWaiters = new Set<(id: number) => void>();
  /**
   * Kill accounting for the kill step in progress, armed for the WHOLE step — the walk
   * to the anchor included — so a wave mob killed in self-defense on the way in is
   * credited exactly as one killed at the anchor. `undefined` outside a kill step, which
   * is what keeps ordinary navigation defense kills uncounted.
   */
  private activeWave: WaveEngagement | undefined;

  constructor(config: BotConfig, env: Record<string, string | undefined> = process.env) {
    this.config = config;
    const raw = env["DELVEWRIGHT_CUTSCENE_GRACE_MS"];
    const parsed = raw === undefined ? NaN : Number.parseInt(raw, 10);
    this.cutsceneGraceMs = Number.isInteger(parsed) && parsed >= 0 ? parsed : 10_000;
  }

  /**
   * task #38: supply the compiler's proven critical-path waypoints. Optional —
   * without it, `walkTo` uses the original single distant-goal behavior. Called by
   * the entrypoint when the `validation/critical-path-waypoints.json` artifact
   * accompanies the critical path.
   */
  useWaypoints(waypoints: Waypoints): void {
    this.waypoints = waypoints;
  }

  /** Connect and resolve once the bot has spawned into the world. */
  async connect(): Promise<void> {
    const bot = createBot({
      host: this.config.host,
      port: this.config.port,
      username: this.config.username,
      version: this.config.version,
      auth: this.config.auth,
    });
    this.bot = bot;
    bot.loadPlugin(pathfinder);
    this.installHandlers(bot);

    await new Promise<void>((resolve, reject) => {
      const onSpawn = (): void => {
        cleanup();
        resolve();
      };
      const onError = (err: Error): void => {
        cleanup();
        reject(err);
      };
      const onEnd = (reason: string): void => {
        cleanup();
        reject(new Error(`bot disconnected before spawn: ${reason}`));
      };
      const onKicked = (reason: string): void => {
        cleanup();
        reject(new Error(`bot kicked before spawn: ${reason}`));
      };
      const cleanup = (): void => {
        bot.removeListener("spawn", onSpawn);
        bot.removeListener("error", onError);
        bot.removeListener("end", onEnd);
        bot.removeListener("kicked", onKicked);
      };
      bot.once("spawn", onSpawn);
      bot.once("error", onError);
      bot.once("end", onEnd);
      bot.once("kicked", onKicked);
    });
  }

  private requireBot(): Bot {
    if (!this.bot) {
      throw new Error("executor is not connected; call connect() first");
    }
    return this.bot;
  }

  /**
   * Wire the always-on listeners: completion-marker + chat-ring capture, and the
   * death handler. Called from {@link connect} and from {@link attachBot} (tests).
   */
  private installHandlers(bot: Bot): void {
    // Capture completion markers from the moment we connect: an objective's marker
    // is broadcast the instant its score flips — usually DURING the step's walk,
    // before the executor gets to wait for it — and the campaign marker lands during
    // the last objective step, so both must be buffered as they arrive. The same
    // stream feeds the recent-chat ring the death diagnostic mines for a cause.
    bot.on("messagestr", (message: string) => {
      this.observeMarker(message);
      this.recentChat.push(message);
      if (this.recentChat.length > CHAT_BUFFER) {
        this.recentChat.shift();
      }
    });
    bot.on("death", () => this.onDeath());
    // Counted from connect, so a respawn is never missed by a listener armed too
    // late (see recoverFromDeath).
    bot.on("spawn", () => {
      this.spawnSeq += 1;
    });
    // Self-defense attribution (souls ladder). PRIMARY channel: mineflayer 4.37 turns
    // the 1.20+ `damage_event` packet into `entityHurt(entity, source)`, where `source`
    // is the entity the server names as responsible (`sourceCauseId`). When the hurt
    // entity is the bot, that source IS the attacker — no guessing needed.
    bot.on("entityHurt", (entity: Entity, source?: Entity) => {
      if (!entity || entity.id !== bot.entity?.id) return;
      this.onBotDamaged(source?.id);
    });
    // FALLBACK: `sourceCauseId` is 0 when the server names no responsible entity (and
    // the lookup misses if that entity is not tracked client-side), so a hit can arrive
    // with no source. A health DROP with no fresh attribution is then blamed on the
    // nearest hostile in melee reach — and on nothing at all if none is close, so a
    // trap or a fall never makes the bot swing at a bystander.
    bot.on("health", () => this.onHealthUpdate());
    // gap 8 (task #32): mineflayer applies a server position packet to
    // `bot.entity.position` and THEN emits `forcedMove` (lib/plugins/physics.js).
    // A large horizontal jump is the compiler's cross-area `teleport` landing; stop
    // the pathfinder so any path/goal computed in the old area is dropped rather than
    // fought or resumed across the void (the "No path to the goal!" / "Path was
    // stopped" race documented in the nobodys-cave gap-8 field notes).
    bot.on("forcedMove", () => this.onForcedMove());
  }

  /**
   * Handle a server-forced position update. Records the new position and, when the
   * jump is large enough to be a cross-area teleport, resets the pathfinder. Reading
   * the position after the event is correct: mineflayer sets it before emitting.
   */
  private onForcedMove(): void {
    const bot = this.bot;
    const p = bot?.entity?.position;
    if (!p) return;
    const now = { x: p.x, y: p.y, z: p.z };
    const prev = this.lastForcedPos;
    this.lastForcedPos = now;
    if (prev && Math.hypot(now.x - prev.x, now.z - prev.z) >= TRANSPORT_JUMP_BLOCKS) {
      this.stopPathfinding();
    }
  }

  /**
   * Abandon whatever the pathfinder is doing, leaving it in a state the NEXT `goto`
   * can actually use.
   *
   * mineflayer-pathfinder's `stop()` only raises an internal `stopPathing` flag; the
   * flag is cleared when the walking bot next reaches a path node, or by a
   * `setGoal`/`setMovements` reset — so calling `stop()` on a bot that is NOT mid-path
   * leaves it raised. The next `goto` then sets its goal, the reset sees the raised
   * flag, fires `path_stop` synchronously, and the brand-new goto rejects instantly
   * with "Path was stopped before it could be completed!" — while `runGoto`'s own
   * failure handler stops the pathfinder again, re-arming the flag. The result is a
   * self-sustaining loop where every later hop fails without the bot ever attempting to
   * walk (observed on the-drowned-bell: after the bot stopped to fight an ambusher,
   * every subsequent hop and every recovery re-path failed that way, and the run died
   * on a leg it had walked fine the run before).
   *
   * `setGoal(null)` immediately after performs that reset ourselves: the flag is
   * consumed here, once, instead of poisoning the next caller. Used everywhere the
   * executor stops the pathfinder.
   */
  private stopPathfinding(): void {
    const bot = this.bot;
    if (!bot) return;
    try {
      bot.pathfinder.stop();
      bot.pathfinder.setGoal(null);
    } catch {
      // best effort — clearing the pathfinder must never mask the reason we stopped
    }
  }

  /**
   * Every hostile the bot can currently see, as {@link ThreatCandidate}s plus a lookup
   * back to the live entities. "Hostile" is exactly {@link isWaveMob} — the same
   * classifier the kill loop uses, so NPC mannequins, displays and dropped items can
   * never be recorded as attackers or become defense targets.
   */
  private visibleHostiles(): {
    candidates: ThreatCandidate[];
    byId: Map<number, Entity>;
  } {
    const bot = this.bot;
    const candidates: ThreatCandidate[] = [];
    const byId = new Map<number, Entity>();
    if (!bot?.entity) return { candidates, byId };
    const here = bot.entity.position;
    for (const entity of Object.values(bot.entities)) {
      if (!entity?.position || !isWaveMob(entity, bot.entity)) continue;
      candidates.push({ id: entity.id, distance: here.distanceTo(entity.position) });
      byId.set(entity.id, entity);
    }
    return { candidates, byId };
  }

  /**
   * The bot took a hit; remember who dealt it. `sourceId` is the entity the server
   * named as responsible (`damage_event.sourceCauseId`, resolved by mineflayer), or
   * `undefined` when it named none — see {@link attributeBotDamage} for what happens
   * then. Recording only; the decision to swing back belongs to the kill loop and the
   * navigation trip.
   */
  private onBotDamaged(sourceId: number | undefined): void {
    const { candidates, byId } = this.visibleHostiles();
    const attacker = attributeBotDamage(sourceId, candidates);
    if (attacker === undefined) return;
    if (sourceId !== undefined && attacker === sourceId) {
      this.lastAttributionAt = Date.now();
    }
    this.threats.record(attacker);
    const name = byId.get(attacker)?.name ?? "?";
    const how = attacker === sourceId ? "server-named source" : "nearest hostile in reach";
    process.stderr.write(
      `[threat] hit by ${name}#${attacker} (${how}); ` +
        `${this.threats.hitsWithin(attacker)} hit(s) in the last ` +
        `${(THREAT_WINDOW_MS / 1_000).toFixed(0)}s\n`,
    );
    this.notifyStalker();
  }

  /**
   * Wake any armed fight-or-flight watcher if a stalker now qualifies. Called from the
   * damage handlers so the reaction happens on the hit, not on the next poll.
   */
  private notifyStalker(): void {
    if (this.stalkerWaiters.size === 0) return;
    const id = this.currentStalker();
    if (id === undefined) return;
    for (const waiter of [...this.stalkerWaiters]) {
      waiter(id);
    }
  }

  /**
   * Health-drop fallback attribution: a drop with no packet-named source in the last
   * {@link HEALTH_ATTRIBUTION_GRACE_MS} is blamed on the nearest hostile within
   * {@link ATTRIBUTION_RANGE}. If nothing is that close, nothing is blamed — a fall, a
   * trap or drowning must never make the bot attack a bystander.
   */
  private onHealthUpdate(): void {
    const bot = this.bot;
    if (!bot) return;
    const previous = this.lastHealth;
    this.lastHealth = bot.health;
    if (previous === undefined || bot.health >= previous) return;
    if (Date.now() - this.lastAttributionAt < HEALTH_ATTRIBUTION_GRACE_MS) return;
    const { candidates, byId } = this.visibleHostiles();
    const attacker = attributeBotDamage(undefined, candidates, ATTRIBUTION_RANGE);
    if (attacker === undefined) return;
    this.threats.record(attacker);
    process.stderr.write(
      `[threat] lost ${(previous - bot.health).toFixed(1)} health with no named source; ` +
        `attributing to the nearest hostile in reach: ` +
        `${byId.get(attacker)?.name ?? "?"}#${attacker} ` +
        `(${this.threats.hitsWithin(attacker)} hit(s) in the last ` +
        `${(THREAT_WINDOW_MS / 1_000).toFixed(0)}s)\n`,
    );
    this.notifyStalker();
  }

  /**
   * Who has hit the bot inside the threat window, most recent first. Diagnostic
   * accessor (as {@link deathDiagnostic}); also what lets tests assert the damage
   * attribution without a live server.
   */
  recentAttackers(): ReturnType<ThreatTracker["active"]> {
    return this.threats.active();
  }

  /** Distance (blocks) to the nearest hostile, or `undefined` if none is visible. */
  private nearestHostileDistance(): number | undefined {
    const { candidates } = this.visibleHostiles();
    let best: number | undefined;
    for (const c of candidates) {
      if (best === undefined || c.distance < best) best = c.distance;
    }
    return best;
  }

  /**
   * Every edible item currently in the inventory that is safe to eat, with its hunger
   * value. The registry says what is edible; {@link isSafeFood} says what a player
   * would actually swallow — rotten flesh is food to minecraft-data and poison to the
   * run (round 2: 7.3 → 3.4 health from the bot's own "eat when hurt" behavior).
   */
  private foodInInventory(): Array<{ item: Item; name: string; foodPoints: number }> {
    const bot = this.requireBot();
    // The pinned minecraft-data registry is the single source of truth for what counts
    // as food — no hardcoded item list to drift from the class kits.
    const foods = (bot.registry as unknown as { foods?: Record<number, { foodPoints?: number }> })
      .foods;
    if (!foods) return [];
    const out: Array<{ item: Item; name: string; foodPoints: number }> = [];
    for (const item of bot.inventory.items()) {
      const food = foods[item.type];
      if (!food) continue;
      if (!isSafeFood(item.name)) continue;
      out.push({ item, name: item.name, foodPoints: food.foodPoints ?? 0 });
    }
    return out;
  }

  /**
   * Eat, the way a player does, when the bot is hurt and nothing is in its face.
   *
   * The class kits hand every class food (the-drowned-bell gives each one rabbit stew);
   * before this the bot carried it through the whole delve untouched, so damage taken
   * in one fight was still missing at the start of the next. Bounded and throttled by
   * {@link EAT_COOLDOWN_MS}; every outcome — including the reasons NOT to eat — is
   * logged, so a run's log says whether the bot was healing or just hurt.
   *
   * Asserts nothing and hides nothing: a bot that dies still fails the run.
   */
  async maybeEat(label: string): Promise<void> {
    const bot = this.bot;
    if (!bot?.entity) return;
    if (Date.now() - this.lastEatAt < EAT_COOLDOWN_MS) return;
    const foods = this.foodInInventory();
    const decision = eatDecision({
      health: bot.health,
      maxHealth: PLAYER_MAX_HEALTH,
      food: bot.food,
      maxFood: PLAYER_MAX_FOOD,
      nearestHostileDistance: this.nearestHostileDistance(),
      hasFood: foods.length > 0,
    });
    if (decision === "healthy") return;
    this.lastEatAt = Date.now();
    const state =
      `health ${bot.health.toFixed(1)}/${PLAYER_MAX_HEALTH}, hunger ${bot.food}/${PLAYER_MAX_FOOD}`;
    if (decision !== "eat") {
      const why = {
        "no-food": "no safe edible item in the kit (harmful food is never eaten)",
        "hostile-near": `a hostile is within ${EAT_SAFE_RANGE} blocks — eating would donate free hits`,
        "hunger-full": "hunger is full, so vanilla forbids eating; natural regeneration is running",
      }[decision];
      process.stderr.write(`[eat] ${label}: not eating (${state}) — ${why}\n`);
      return;
    }
    const choice = pickFood(foods);
    if (!choice) return; // unreachable (hasFood was true) — defensive
    const before = bot.health;
    try {
      await bot.equip(choice.item, "hand");
      await bot.consume();
      process.stderr.write(
        `[eat] ${label}: ate ${choice.name} at ${state} → health ` +
          `${bot.health.toFixed(1)}/${PLAYER_MAX_HEALTH}, hunger ${bot.food}/${PLAYER_MAX_FOOD} ` +
          `(was ${before.toFixed(1)})\n`,
      );
    } catch (err) {
      // Eating is opportunistic: a failed bite is reported and the run carries on.
      process.stderr.write(
        `[eat] ${label}: could not eat ${choice.name} (${state}): ` +
          `${err instanceof Error ? err.message : String(err)}\n`,
      );
    } finally {
      // Back to the sword — never leave the bot walking into a fight holding a bowl.
      await this.equipLoadout();
    }
  }

  /**
   * The hostile currently worth interrupting a walking leg for, if any: a mob that has
   * hit the bot {@link STALKER_HITS}+ times inside the threat window and is still
   * within {@link STALKER_RANGE}. Entities a defense budget already failed to kill are
   * excluded, so an unkillable stalker cannot stop every hop of a leg.
   */
  private currentStalker(): number | undefined {
    const { candidates } = this.visibleHostiles();
    return pickStalker(
      candidates.filter((c) => !this.defenseExempt.has(c.id)),
      this.threats,
    );
  }

  /**
   * Arm a watch that resolves the moment a stalker qualifies (see
   * {@link currentStalker}). Two channels: the damage handler wakes it on the hit that
   * qualifies (near-zero latency, which is the difference between fighting back and
   * dying with a full stew in the pack), and a slow poll backstops the case where a mob
   * that already hit the bot merely closes the distance. `cancel()` disarms both; the
   * promise then simply never settles, which is what `Promise.race` wants.
   */
  private armStalkerTrip(): {
    promise: Promise<{ kind: "stalker"; id: number }>;
    cancel: () => void;
  } {
    let cancelled = false;
    let waiter: ((id: number) => void) | undefined;
    const promise = new Promise<{ kind: "stalker"; id: number }>((resolve) => {
      const fire = (id: number): void => {
        if (cancelled) return;
        cancelled = true;
        if (waiter) this.stalkerWaiters.delete(waiter);
        resolve({ kind: "stalker", id });
      };
      waiter = fire;
      this.stalkerWaiters.add(fire);
      const poll = async (): Promise<void> => {
        while (!cancelled) {
          await delay(THREAT_POLL_MS);
          if (cancelled) return;
          const id = this.currentStalker();
          if (id !== undefined) {
            fire(id);
            return;
          }
        }
      };
      void poll();
    });
    return {
      promise,
      cancel: () => {
        cancelled = true;
        if (waiter) this.stalkerWaiters.delete(waiter);
      },
    };
  }

  /**
   * Stand and fight a mob that latched onto the bot mid-leg, then resume walking.
   *
   * Bounded by {@link DEFEND_BUDGET_MS}: a delve mob dies in a few swings, and anything
   * that outlasts the budget is written off (added to `defenseExempt`, reported) and the
   * leg continues — so this can never convert a content problem into a navigation
   * failure. The bot does NOT chase: a mob that breaks off is let go, because the job is
   * the route, not the kill.
   */
  private async defendAgainst(id: number, label: string): Promise<void> {
    const bot = this.requireBot();
    const name = bot.entities[id]?.name ?? "?";
    process.stderr.write(
      `[defend] ${label}: ${name}#${id} has hit the bot ${this.threats.hitsWithin(id)}× in the ` +
        `last ${(THREAT_WINDOW_MS / 1_000).toFixed(0)}s and is still within ${STALKER_RANGE} ` +
        `blocks — stopping to fight it (budget ${(DEFEND_BUDGET_MS / 1_000).toFixed(0)}s)\n`,
    );
    // If a kill objective is in progress, this mob is being fought AS PART OF IT — the
    // approach leg is inside the step. Record the engagement so that, if it dies near
    // the wave anchor, it is credited exactly as a kill-loop kill would be. Without
    // this, a wave mob that ambushes the bot on the way in dies uncounted and the step
    // can never reach `step.count` (ladder run 13).
    this.activeWave?.engaged.add(id);
    // Stop walking, but NOT sneaking: `bot.clearControlStates()` would drop the crouch
    // a sneak leg turned on, standing the bot up inside whatever the crouch was hiding
    // it from.
    for (const control of ["forward", "back", "left", "right", "jump", "sprint"] as const) {
      bot.setControlState(control, false);
    }
    const deadline = Date.now() + DEFEND_BUDGET_MS;
    while (Date.now() < deadline) {
      if (this.death) throw this.death;
      const mob = bot.entities[id];
      if (!mob?.position) {
        process.stderr.write(`[defend] ${name}#${id} is down; resuming ${label}\n`);
        this.threats.forget(id);
        return;
      }
      const dist = bot.entity.position.distanceTo(mob.position);
      if (dist > RETALIATION_RANGE + 2) {
        process.stderr.write(
          `[defend] ${name}#${id} broke off (${dist.toFixed(1)} blocks away); resuming ${label}\n`,
        );
        return;
      }
      if (dist > RETALIATION_RANGE) {
        // Out of the bot's own reach but still on it — let it close rather than chase.
        await delay(REACH_POLL_MS);
        continue;
      }
      try {
        await bot.lookAt(mob.position.offset(0, (mob.height ?? 1) * 0.5, 0), true);
      } catch {
        // best effort — a failed look must not abort the defense
      }
      bot.attack(mob);
      await delay(ATTACK_INTERVAL_MS);
    }
    this.defenseExempt.add(id);
    process.stderr.write(
      `[defend] could not put ${name}#${id} down within ` +
        `${(DEFEND_BUDGET_MS / 1_000).toFixed(0)}s — resuming ${label} and ignoring it for the ` +
        `rest of the run (unreachable, or an Invulnerable actor)\n`,
    );
  }

  /**
   * One hop, with fight-or-flight. Runs {@link runGoto}, but races it against a
   * stalker watch: if a mob latches onto the bot mid-walk the path is stopped, the mob
   * is fought (bounded), and the hop is retried. Bounded by
   * {@link DEFENSE_ROUNDS_PER_HOP}, after which the hop is walked with no further
   * interruption and fails exactly as loudly as it did before this existed.
   *
   * A hop that FAILS while a stalker is on the bot gets the same treatment once (a mob
   * body is a pathfinder obstacle), then rethrows.
   *
   * **A `sneak` leg is exempt from all of it** — no fighting, no eating. `sneak: true`
   * is the delve declaring that stealth, not combat, is the mechanic on this leg, and
   * a stealth section runs on a clock: nobodys-cave-island's `begin-stealth` gives
   * 90 ticks of grace outside a safe zone and answers a miss with `damage-players 40`
   * — an instant kill. Stopping to swing at the (Invulnerable) warden it wants the
   * player to creep past spent that grace and killed the bot on a leg that had always
   * been green. Worse, the stealth damage itself carries NO source entity, so it is
   * attributed to the nearest hostile — the warden — and the bot "retaliates" against
   * the very thing punishing it. On a sneak leg, fight-or-flight is flight.
   */
  private async gotoDefended(spec: GoalSpec, label: string, sneak = false): Promise<void> {
    if (sneak) {
      // Walk it, crouched, and do not stop for anything. Unchanged pre-self-defense
      // behaviour, which is exactly what a stealth leg wants.
      await this.runGoto(spec, label);
      return;
    }
    for (let round = 0; round < DEFENSE_ROUNDS_PER_HOP; round++) {
      await this.maybeEat(label);
      const trip = this.armStalkerTrip();
      // Observe the walk's outcome exactly once: the trip can win the race while the
      // walk is still in flight, and an unobserved rejection would crash the process.
      const walk = this.runGoto(spec, label).then(
        () => ({ kind: "walk" as const, err: undefined as unknown }),
        (err: unknown) => ({ kind: "walk" as const, err: err as unknown }),
      );
      const winner = await Promise.race([walk, trip.promise]);
      trip.cancel();
      if (winner.kind === "walk") {
        if (winner.err === undefined) return;
        if (winner.err instanceof BotDeathError) throw winner.err;
        const stalker = this.currentStalker();
        if (stalker === undefined) throw winner.err;
        process.stderr.write(
          `[defend] ${label} failed with a mob on the bot; dealing with it and retrying the hop\n`,
        );
        await this.defendAgainst(stalker, label);
        continue;
      }
      // A stalker latched on mid-walk: stop, fight it, then resume the leg. The settle
      // is capped (the pathfinder only halts at its next node, and every millisecond
      // spent waiting is another swing taken), and the walk wrapper never rejects, so
      // it can be collected after the fight.
      this.stopPathfinding();
      await Promise.race([walk, delay(WALK_SETTLE_MS)]);
      await this.defendAgainst(winner.id, label);
      const walked = await walk;
      if (walked.err instanceof BotDeathError) throw walked.err;
      // Clear the stop flag the settled goto may have re-raised, so the retry below
      // actually walks (see stopPathfinding).
      this.stopPathfinding();
    }
    // Defense rounds spent — walk it out. Still the original, unweakened hop.
    await this.runGoto(spec, label);
  }

  /**
   * Record an anchored completion marker (AUDIT-P0). Exact whole-line parse, scoped
   * to this run's campaign; everything else on the chat stream is ignored, including
   * lines that merely mention completion. First arrival wins — a re-broadcast must
   * not relabel when an objective actually completed.
   */
  private observeMarker(message: string): void {
    const marker = parseCompletionMarker(message);
    if (!marker || marker.campaignId !== this.campaignId) return;
    if (marker.token === CAMPAIGN_TOKEN) {
      this.campaignCompleteAtStep ??= this.currentStep;
      return;
    }
    if (!this.completedObjectives.has(marker.token)) {
      this.completedObjectives.set(marker.token, this.currentStep);
    }
  }

  /**
   * Wait until `objectiveId`'s own anchored completion marker has arrived — the
   * ONLY evidence that a step's objective completed. Arriving somewhere, opening a
   * dialogue or emptying a chest are means, never proof; a step whose marker never
   * comes fails loudly with the bot's position and what the delve did broadcast.
   * Death-aware and bounded.
   *
   * Public so it is directly unit-testable: it is the executor's whole success
   * criterion for a step, and testing it through `reach`/`collect` would need a live
   * pathfinder and a real chest.
   */
  async requireObjective(objectiveId: string, label: string): Promise<void> {
    const alreadyDone = this.completedObjectives.get(objectiveId);
    if (alreadyDone !== undefined && alreadyDone < this.currentStep) {
      // Not a failure — the objective did complete — but the path claims THIS step
      // proves it, so the ordering is worth surfacing in the run log.
      process.stderr.write(
        `[oracle] ${objectiveId} completed during step ${alreadyDone}, before its own ` +
          `step ${this.currentStep} (${label})\n`,
      );
      return;
    }
    const arrived = await this.waitFor(
      () => this.completedObjectives.has(objectiveId),
      OBJECTIVE_TIMEOUT_MS,
      SCORE_POLL_MS,
    );
    if (arrived) return;
    const seen = [...this.completedObjectives.keys()];
    throw new Error(
      `${label}: objective ${objectiveId} did not complete within ` +
        `${OBJECTIVE_TIMEOUT_MS}ms — no \`${markerLine(this.campaignId ?? "?", objectiveId)}\` ` +
        `marker arrived; bot at ${fmt(this.requireBot().entity.position)}; objectives ` +
        `completed so far: ${seen.join(", ") || "none"}`,
    );
  }

  /**
   * Adopt the critical path's campaign id and step count. Markers are scoped to this
   * campaign, so a marker from other content can never satisfy a step. Called by the
   * entrypoint before the run starts.
   */
  useCampaign(campaignId: string): void {
    this.campaignId = campaignId;
  }

  /** Sequencer hook: the run has moved on to step `index`. Attribution only. */
  beginStep(index: number): void {
    this.currentStep = index;
  }

  /**
   * Endgame discipline (AUDIT-P0). Called by the sequencer after every step that
   * still has an objective step ahead of it: campaign completion belongs to the LAST
   * objective step, so its marker arriving any earlier proves the path is incoherent
   * — the remaining steps cannot be doing anything the campaign still needs. Fail
   * here, at the step that revealed it, rather than reporting a green run whose tail
   * was hollow.
   */
  assertEndgameNotReached(stepIndex: number, finalObjectiveIndex: number): void {
    if (this.campaignCompleteAtStep === undefined) return;
    throw new Error(
      `campaign completed at step ${this.campaignCompleteAtStep}, but the critical path ` +
        `runs objective steps through step ${finalObjectiveIndex} (detected after step ` +
        `${stepIndex}) — every later step is hollow. The path and the delve's completion ` +
        `condition disagree; fix the campaign or the path, never the check`,
    );
  }

  /**
   * Test/advanced seam: adopt an already-created (or fake) bot and install the same
   * handlers `connect()` wires, without the network path. Unit tests use this to
   * drive death/cutscene behaviour against a mocked bot.
   */
  attachBot(bot: Bot): void {
    this.bot = bot;
    this.installHandlers(bot);
  }

  /**
   * Death handler: record where and (best-effort) why the bot died, stop any
   * in-flight pathfinding, and fire the death waiters so the current long-running
   * step rejects promptly with a {@link BotDeathError} instead of hanging.
   */
  private onDeath(): void {
    if (this.death) return; // already recorded this death
    const bot = this.bot;
    let position: readonly [number, number, number] | undefined;
    const p = bot?.entity?.position;
    if (p) {
      position = [Math.round(p.x), Math.round(p.y), Math.round(p.z)];
    }
    const cause = likelyDeathCause(this.recentChat, bot?.username ?? "");
    const err = new BotDeathError(position, cause);
    this.death = err;
    this.deathSeq += 1;
    this.spawnSeqAtDeath = this.spawnSeq;
    this.stopPathfinding();
    for (const waiter of this.deathWaiters) {
      waiter(err);
    }
    this.deathWaiters.clear();
  }

  /**
   * Race `op` against the bot dying: resolves/rejects with `op`, but rejects with the
   * {@link BotDeathError} the instant a death is observed (the underlying op keeps
   * running but the pathfinder is already stopped in {@link onDeath}). Used to abort
   * the ~60s `pathfinder.goto` wait the moment the bot dies.
   */
  private raceDeath<T>(op: Promise<T>): Promise<T> {
    if (this.death) return Promise.reject(this.death);
    return new Promise<T>((resolve, reject) => {
      let settled = false;
      const onDeath = (err: BotDeathError): void => {
        if (settled) return;
        settled = true;
        reject(err);
      };
      this.deathWaiters.add(onDeath);
      op.then(
        (value) => {
          if (settled) return;
          settled = true;
          this.deathWaiters.delete(onDeath);
          resolve(value);
        },
        (err: unknown) => {
          if (settled) return;
          settled = true;
          this.deathWaiters.delete(onDeath);
          reject(err);
        },
      );
    });
  }

  /**
   * The death recorded so far, if any. Diagnostic accessor (also lets tests assert the
   * captured position/cause after a simulated death).
   */
  deathDiagnostic(): BotDeathError | undefined {
    return this.death;
  }

  /**
   * gap 7 (retry path): ready the bot to resume after a death — wait for it to respawn,
   * then clear the death latch so subsequent steps run against the live bot again. The
   * sequencer re-runs `select-class` afterwards (respawn drops class state).
   */
  async recoverFromDeath(): Promise<void> {
    this.requireBot();
    // COUNTED, not listened for. mineflayer auto-respawns within a few dozen ms of
    // the death, which is sooner than a caller polling the death latch can arm a
    // `once("spawn")` — so the old listener routinely missed the respawn it was
    // waiting for and burned the whole 15s timeout before "resuming anyway". Free
    // before spec-0023; on the die-retry stage it is 15s per scripted death, two
    // per encounter, straight out of the run budget (task #102, observed live on
    // the keep-trial fixture). A counter cannot miss an event that already fired.
    const deadline = Date.now() + RESPAWN_TIMEOUT_MS;
    let respawned = false;
    while (Date.now() < deadline) {
      if (this.spawnSeq > this.spawnSeqAtDeath) {
        respawned = true;
        break;
      }
      await delay(SPAWN_POLL_MS);
    }
    if (!respawned) {
      // Best effort: proceed anyway — the re-select-class teleport re-establishes a
      // known position regardless.
      process.stderr.write(
        `[death] no respawn observed within ${RESPAWN_TIMEOUT_MS}ms; resuming anyway\n`,
      );
    }
    this.death = undefined;
    // A respawn starts a new life: old grudges (and old write-offs) do not carry into
    // it, and the entities they name are usually gone anyway.
    this.threats.clear();
    this.defenseExempt.clear();
    this.lastHealth = undefined;
  }

  /** Disconnect the bot, if connected. Safe to call more than once. */
  close(): void {
    if (this.bot) {
      this.bot.end();
      this.bot = undefined;
    }
  }

  async selectClass(step: SelectClassStep): Promise<void> {
    const bot = this.requireBot();
    // Remembered so the die-retry stage can re-arm after a scripted death: a
    // respawn drops the whole kit, and a bare-handed bot proves nothing about
    // the fight it is about to re-engage (spec-0023 §1).
    this.lastSelectClass = step;
    // The class-selection dialog button runs `step.command` (a `/trigger`); the
    // bot fires the same command directly. The per-tick handler then applies the
    // kit and teleports the player to the campaign spawn.
    bot.chat(step.command);
    // Give the datapack a tick to reset the trigger, give the kit and teleport.
    await delay(CLASS_SETTLE_MS);
    // Equip the kit (sword + armor) so the bot can fight v0.3 combat waves. A
    // no-op for kits without those items.
    await this.equipLoadout();
  }

  /**
   * Equip the best weapon and each armour piece from the current inventory. Item
   * names are matched by substring (`sword`, `helmet`, …). Best-effort per slot.
   */
  private async equipLoadout(): Promise<void> {
    const bot = this.requireBot();
    const slots: ReadonlyArray<[string, "hand" | "head" | "torso" | "legs" | "feet"]> = [
      ["sword", "hand"],
      ["helmet", "head"],
      ["chestplate", "torso"],
      ["leggings", "legs"],
      ["boots", "feet"],
    ];
    for (const [key, dest] of slots) {
      const item = bot.inventory.items().find((i) => i.name.includes(key));
      if (item) {
        try {
          await bot.equip(item, dest);
        } catch {
          // best effort — a missing slot is not a failure
        }
      }
    }
  }

  async talkTo(step: TalkToStep): Promise<void> {
    const bot = this.requireBot();
    // Walk to the NPC first (realism; some dialog effects are reach-gated), then
    // chat the dialog-option `/trigger` command the button would have run.
    await this.walkTo(step.pos, 3, `npc ${step.npc}`, step.sneak);
    bot.chat(step.command);
    // A dialogue that OPENED proves nothing: the option must actually complete the
    // objective this step stands for. Wait for that objective's own marker.
    await this.requireObjective(step.objective, `talk-to ${step.npc}`);
    await delay(EFFECT_SETTLE_MS);
  }

  /**
   * Walk to the anchor using the pathfinder. The datapack completes the objective
   * with a `distance=..radius` check on the exact anchor point, so the bot targets
   * a goal one block *tighter* than `radius` — landing well inside the check rather
   * than on its boundary (where the pathfinder's block-granular goal and the
   * server's precise-position check can disagree).
   */
  async reach(step: ReachStep): Promise<void> {
    await this.walkTo(step.pos, Math.max(1, step.radius - 1), `anchor ${step.anchor}`, step.sneak);
    // Standing at the anchor is NOT success (AUDIT-P0): the objective's own
    // completion marker is. A reach step whose zone check never fires — wrong cell,
    // an inactive objective, a gate the path assumed open — now fails here instead
    // of silently marching the run forward.
    await this.requireObjective(step.objective, `reach ${step.anchor}`);
  }

  /**
   * Pathfind to within `range` blocks of the absolute target (mineflayer-pathfinder
   * `GoalNear`). Replaces the pre-v0.3 "face + hold forward" walk, so turns and
   * branches in jigsaw layouts are walkable. Digging is disabled (adventure mode).
   * A `sneak` leg (gap 7) walks crouched with sprinting disabled; the crouch is
   * restored to off afterwards so a later plain leg is not left sneaking. The long
   * `goto` wait races the death latch so a death aborts it fast, not after ~60s.
   */
  private async walkTo(
    pos: readonly [number, number, number],
    range: number,
    label: string,
    sneak = false,
  ): Promise<void> {
    const bot = this.requireBot();
    const r = Math.max(1, Math.floor(range));
    const movements = new Movements(bot);
    const restoreControls = configureLeg(bot, movements, sneak);
    // task #38: no cave-specific Movements override is needed — the compiler-proven
    // waypoints keep the bot on clear standable ground (the DW0311 A* treats water
    // and fences as impassable, so the route never crosses them, and gravity blocks
    // and stairs are ordinary floor). Entity detection is left ON (the pathfinder
    // default) so the bot routes AROUND a transient mob on a hop rather than ramming
    // it — disabling it made the bot wedge against a leaked mob and time out.
    // task #45: but the pathfinder's default treats EVERY non-passable entity as an
    // obstacle, including non-colliding display/interaction/marker entities that
    // block nothing in-world. Those (a completed interact objective's leaked
    // `interaction` hitbox, an NPC's co-located hitbox, floating item/text displays)
    // congested the terminal approach to an NPC and timed the leg out. Mark them
    // passable so the bot paths through them — physics-honest, and solid entities
    // (mobs, the mannequin NPC itself) are still avoided.
    allowNonCollidingEntities(movements);
    bot.pathfinder.setMovements(movements);
    // Long multi-level layouts (e.g. a 5-storey keep, ~90 blocks + 4 staircases)
    // sit at the edge of the default A* budget and fail nondeterministically
    // with "No path to the goal!" — give the search real headroom. With leg-by-leg
    // waypoints each solve is tiny, so this budget is only a safety margin.
    bot.pathfinder.thinkTimeout = 30_000;
    try {
      // task #38: when the compiler proved a waypoint polyline for this leg, replay
      // it as short hops so each A* solve is trivial (avoids the single giant solve
      // that strands the bot on a large open winding cave); the final goal is always
      // the true destination. Legs are matched in lockstep path order and consumed
      // as walked; a non-matching walk (a sub-walk, or a post-transport step) does
      // not consume and falls back to the single destination goal.
      let legWaypoints: readonly Vec3Tuple[] | undefined;
      // spec-0016 §4 (task #81): the timed gates this leg's proven route walks
      // THROUGH. Only a marked leg is allowed the window wait below.
      let legGates: readonly TimedGate[] = [];
      if (this.waypoints) {
        const match = nextLegWaypoints(this.waypoints.legs, this.legCursor, [
          pos[0],
          pos[1],
          pos[2],
        ]);
        legWaypoints = match.waypoints;
        legGates = match.timedGates;
        this.legCursor = match.cursor;
      }
      if (legGates.length > 0) {
        process.stderr.write(
          `[timed-gate] ${label}: proven route crosses ${describeGates(legGates)}\n`,
        );
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
        // Fight-or-flight: every hop of a walked leg is defended (see gotoDefended) —
        // a mob that has latched onto the bot is dealt with and the leg resumes,
        // instead of the bot walking on while a stalker from an earlier ambush chews
        // through the health it needs for the next fight.
        (spec, glabel) => this.gotoDefended(spec, glabel, sneak),
        (target) => this.unstickToward(target),
        legGates.length > 0
          ? {
              gates: legGates,
              waitForWindow: (gates) => this.waitForGateWindow(gates),
              feetCell: () => this.feetCell(),
            }
          : undefined,
      );
    } finally {
      restoreControls();
    }
  }

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
  private waypointSupportStandable(cell: Vec3Tuple, fences: Set<number>): boolean {
    const bot = this.requireBot();
    const p = bot.entity.position;
    const support = bot.blockAt(p.offset(cell[0] - p.x, cell[1] - 1 - p.y, cell[2] - p.z));
    if (!support) return true; // support unknown (chunk not loaded) → keep the waypoint
    return !fences.has(support.type);
  }

  /** The bot's current feet cell (floored block position), or `undefined` if the bot
   * is not connected. Read-only observation, used only to decide whether a timed-gate
   * retry must retreat to a standoff first. */
  private feetCell(): Vec3Tuple | undefined {
    const bot = this.bot;
    if (!bot?.entity) return undefined;
    const p = bot.entity.position;
    return [Math.floor(p.x), Math.floor(p.y), Math.floor(p.z)];
  }

  /**
   * Whether every cell of `gate`'s compiler-declared region currently reads as empty
   * space — i.e. the clock has the gate OPEN. `undefined` when the region's blocks
   * cannot be read (chunk not loaded), so the caller can distinguish "shut" from
   * "cannot see". The emptiness test is the block's own collision shape
   * (`boundingBox === "empty"`), not a block-name comparison, so it stays correct for
   * whatever block a campaign fills its gate with.
   */
  private gateOpen(gate: TimedGate): boolean | undefined {
    const bot = this.requireBot();
    const p = bot.entity.position;
    for (const [x, y, z] of gateRegionCells(gate)) {
      const block = bot.blockAt(p.offset(x - p.x, y - p.y, z - p.z));
      if (!block) return undefined; // not loaded — state unknown
      if (block.boundingBox !== "empty") return false;
    }
    return true;
  }

  /**
   * Wait for `gates` to swing from closed to OPEN, so a crossing starts at the top of
   * the window rather than its tail (spec-0016 §4, task #81).
   *
   * Two bounded phases: first watch until the gates read CLOSED (so an already-open
   * window whose remaining ticks are unknown is not mistaken for a fresh one), then
   * watch until they read OPEN. Each phase is capped at {@link gateWindowWaitMs} —
   * one full cycle plus margin, within which the clock is guaranteed to produce the
   * edge — so an unreadable region (chunk not loaded) can never hang the run; the
   * wait simply gives up and the caller tries the hop anyway.
   *
   * This is navigation, not game logic: the harness reads the world only to TIME a
   * movement the compiler already proved possible. It asserts nothing about the gate.
   */
  private async waitForGateWindow(gates: readonly TimedGate[]): Promise<void> {
    const cap = gateWindowWaitMs(gates);
    const allOpen = (): boolean | undefined => {
      let known = true;
      for (const g of gates) {
        const open = this.gateOpen(g);
        if (open === undefined) known = false;
        else if (!open) return false;
      }
      return known ? true : undefined;
    };
    const watch = async (want: boolean, phase: string): Promise<boolean> => {
      const deadline = Date.now() + cap;
      while (Date.now() < deadline) {
        if (allOpen() === want) return true;
        await delay(GATE_POLL_MS);
      }
      process.stderr.write(
        `[timed-gate] gave up waiting for the gate to read ${phase} after ` +
          `${(cap / 1_000).toFixed(1)}s — crossing on the next attempt regardless\n`,
      );
      return false;
    };
    if (!(await watch(false, "closed"))) return;
    process.stderr.write(`[timed-gate] gate is shut; waiting for it to open\n`);
    if (await watch(true, "open")) {
      process.stderr.write(`[timed-gate] window open — crossing now\n`);
    }
  }

  /**
   * Raw, pathfinder-free nudge toward `target` to dislodge a physically wedged bot
   * (task #45). When the stall-recovery pathfind itself can't escape a concave corner
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
  private async unstickToward(target: GoalSpec): Promise<number> {
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
  }

  /**
   * Pathfind to a single {@link GoalSpec} (get within `spec.range` blocks of it),
   * with the death-aware, timed, one-retry behavior the critical path depends on: a
   * bot death rethrows the {@link BotDeathError} immediately (never retried across
   * the void); a transient failure is retried once after a settle (an `open-gate`
   * fill may land after the first path computation started); a persistent failure
   * throws a diagnostic naming the goal and the bot's position. The caller sets the
   * Movements and think budget once for the whole leg.
   *
   * Arrival is VERIFIED, not trusted: mineflayer's `goto` can resolve on a
   * best-effort partial path without the bot actually reaching the goal (observed on
   * an unwalkable waypoint hop — it "succeeds" while the bot sits blocks away). A
   * resolve that leaves the bot outside the goal range is treated as a failure, so a
   * stuck hop fails the step loudly instead of silently marching the walk forward.
   */
  private async runGoto(spec: GoalSpec, label: string): Promise<void> {
    const bot = this.requireBot();
    const { x, y, z, range } = spec;
    // Already within the goal? Return without pathfinding. mineflayer-pathfinder
    // rejects a `goto` issued when the bot already sits at the target with "Path was
    // stopped before it could be completed" (task #45: after a physics-unstick lands
    // the bot inside a hop's range, the retry `goto` would otherwise fail spuriously
    // on a goal that is in fact already satisfied).
    if (this.withinGoal(spec)) {
      return;
    }
    let lastErr: unknown;
    for (let attempt = 0; attempt < 2; attempt++) {
      if (attempt > 0) {
        await delay(1_500);
      }
      try {
        await this.raceDeath(
          withTimeout(
            bot.pathfinder.goto(new goals.GoalNear(x, y, z, range)),
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
      .map((e) => `${e.name ?? "?"}@${e.position.distanceTo(bot.entity.position).toFixed(1)}`);
    process.stderr.write(`[stuck] near ${fmt(bot.entity.position)}: ${near.join(", ") || "none"}\n`);
    throw new Error(
      `failed ${label} at [${x}, ${y}, ${z}] (range ${range}); bot at ` +
        `${fmt(bot.entity.position)}: ${detail}`,
    );
  }

  /**
   * Whether the bot's block position is within `spec.range` blocks of the goal cell
   * — the same block-distance metric mineflayer-pathfinder's `GoalNear` uses to
   * decide it arrived. The `y` axis is given one extra block of slack so standing on
   * a stair/slab (a fractional-height floor) still counts as arrived.
   */
  private withinGoal(spec: GoalSpec): boolean {
    const p = this.requireBot().entity.position;
    const dx = Math.floor(p.x) - spec.x;
    const dz = Math.floor(p.z) - spec.z;
    const dy = Math.floor(p.y) - spec.y;
    const yTol = spec.range + 1;
    return dx * dx + dz * dz <= spec.range * spec.range && Math.abs(dy) <= yTol;
  }

  /**
   * Slay a wave: go to the wave anchor, then hunt and kill the wave's mobs until the
   * required `step.count` are confirmed dead (the primary, objective-semantic signal)
   * or no eligible wave mob remains, or the budget runs out.
   *
   * The delve world is sealed (`spawn_mobs false`), but it is NOT empty of mob-shaped
   * entities: story actors are summoned as ordinary living mobs — an Invulnerable
   * `minecraft:warden` posing as the cyclops Polyphemus, `minecraft:mannequin` NPC
   * puppets at the class posts — and they sit right where combat happens (the surf
   * wave spawns beside the eurylochus mannequin; the warden waits ~64 blocks off in a
   * later cave area). `nearestEntity` cannot tell them from a wave mob by shape, and
   * mineflayer on 1.21.11 cannot read the entity `Tags`/scoreboard that would (the
   * `KillStep.tag` is informational only). So the bot proves the wave down without
   * ever attacking — or walking to — a story actor:
   *   * mannequin NPCs are excluded from targeting outright (see NON_WAVE_ENTITIES);
   *   * a confirmed KILL is a targeted mob that winks out in melee near the anchor;
   *     reaching `step.count` clears the wave and returns immediately, so the bot
   *     never treks off to the distant warden (which would trip later-area triggers);
   *   * any candidate the bot cannot kill (meleed past {@link WAVE_UNKILLABLE_MS} and
   *     still alive → Invulnerable) or cannot path to is blacklisted, so the loop
   *     hunts the next real wave mob instead of fixating on an unkillable one; when no
   *     eligible wave mob is left, the wave is likewise cleared.
   * Navigation + assertion only: the datapack's kill advancement + countdown are what
   * actually complete the objective when the last tagged mob dies.
   */
  private async fightWave(step: KillStep): Promise<void> {
    const bot = this.requireBot();
    // Confirmed kills: a mob the bot has attacked that then vanishes near the wave
    // anchor (see wave.ts). Counting these (rather than "no mob-shaped entity remains")
    // is what lets the step end at `step.count` without walking to a far Invulnerable
    // actor. Armed BEFORE the approach walk: self-defense (#173) can kill a wave mob
    // that ambushes the bot on the way in, and that kill is wave progress like any other
    // — crediting it only from the kill loop deadlocked the objective (ladder run 13).
    const engagement = beginWave(step.wave, step.pos);
    const onGone = (e: Entity): void => {
      if (!creditsWaveKill(engagement, e.id, e.position)) return;
      engagement.credited.add(e.id);
      engagement.killed += 1;
      process.stderr.write(
        `[kill ${step.wave}] confirmed kill: ${e.name ?? "?"}#${e.id} ` +
          `(${engagement.killed}/${step.count})\n`,
      );
    };
    bot.on("entityGone", onGone);
    this.activeWave = engagement;
    // Entities the bot has proven it can neither kill nor reach — never re-targeted.
    const blacklist = new Set<number>();
    try {
      await this.equipLoadout();
      await this.walkTo(step.pos, 3, `wave ${step.wave}`, step.sneak);
      // Give AI-enabled mobs a moment to path toward the bot after we arrive.
      await delay(1_000);
      // Diagnostic: what does the bot see near the wave anchor?
      const near = Object.values(bot.entities)
        .filter((e) => e && e !== bot.entity && bot.entity.position.distanceTo(e.position) < 48)
        .map(
          (e) =>
            `${e.name ?? "?"}(t=${e.type},k=${(e as { kind?: string }).kind ?? "?"},h=${e.height ?? "?"})`,
        );
      process.stderr.write(
        `[kill ${step.wave}] nearby(${near.length}): ${near.join(", ") || "none"}` +
          `${engagement.killed > 0 ? ` — ${engagement.killed} already down on the approach` : ""}\n`,
      );

      const deadline = Date.now() + KILL_TIMEOUT_MS;
      let emptyStreak = 0;
      let clearedStreak = 0;
      let engagedId: number | undefined;
      let engagedSince = 0;
      while (Date.now() < deadline) {
        // Fail fast if a mob killed the bot mid-fight (gap 7) rather than looping.
        if (this.death) throw this.death;
        // The whole wave is confirmed down — done, wherever the bot happens to stand.
        if (engagement.killed >= step.count) return;
        // Eat between exchanges when hurt and nothing is in reach (no-op otherwise).
        await this.maybeEat(`wave ${step.wave}`);
        const wave = bot.nearestEntity((e) => isWaveMob(e, bot.entity) && !blacklist.has(e.id));
        // RETALIATION (souls ladder): the wave is the objective, but anything currently
        // drawing the bot's blood in melee outranks it — a souls `ambush` desugars to
        // spawn + unleash with no kill objective, so a bypassed ambusher belongs to no
        // wave, follows the bot across the map and free-hits it through the next fight.
        // A player would turn around. The bot now does too.
        const { candidates, byId } = this.visibleHostiles();
        const retaliateId = pickRetaliationTarget(
          candidates.filter((c) => !blacklist.has(c.id)),
          this.threats,
        );
        const retaliation = retaliateId !== undefined && retaliateId !== wave?.id;
        const mob = (retaliation ? byId.get(retaliateId!) : undefined) ?? wave;
        // Second terminal condition, judged by the LIVE mobs rather than the wave's
        // declared size: every mob this fight engaged is down and nothing hostile is
        // near enough to still be part of it. `killed >= step.count` cannot see this
        // case, because `count` is the wave's ORIGINAL size — if a member died in a way
        // the proximity rule could not attribute (killed well off the anchor, or by a
        // trap), the counter can never get there and the step would burn its whole
        // budget on a wave the bot has already beaten (ladder run 13).
        const nearestEligible = candidates
          .filter((c) => !blacklist.has(c.id))
          .reduce<number | undefined>(
            (best, c) => (best === undefined || c.distance < best ? c.distance : best),
            undefined,
          );
        if (
          waveEngagementCleared({
            engagedIds: [...engagement.engaged],
            isDown: (id) => !bot.entities[id] || blacklist.has(id),
            nearestEligibleDistance: nearestEligible,
          })
        ) {
          if (++clearedStreak >= WAVE_CLEAR_STREAK) {
            process.stderr.write(
              `[kill ${step.wave}] every mob this fight engaged is down ` +
                `(${engagement.killed} confirmed near the anchor) and no hostile is within ` +
                `${WAVE_ENGAGE_NEAR} blocks — wave cleared\n`,
            );
            return;
          }
          await delay(REACH_POLL_MS);
          continue;
        }
        clearedStreak = 0;
        if (!mob) {
          // No eligible wave mob remains (every real mob dead; any unkillable actor
          // blacklisted) → wave cleared.
          if (++emptyStreak >= WAVE_CLEAR_STREAK) return;
          await delay(REACH_POLL_MS);
          continue;
        }
        emptyStreak = 0;
        const dist = bot.entity.position.distanceTo(mob.position);
        if (retaliation) {
          if (engagedId !== mob.id) {
            process.stderr.write(
              `[defend] wave ${step.wave}: hitting back at ${mob.name ?? "?"}#${mob.id} ` +
                `(${this.threats.hitsWithin(mob.id)} hit(s) in the last ` +
                `${(THREAT_WINDOW_MS / 1_000).toFixed(0)}s, ${dist.toFixed(1)} blocks) before ` +
                `resuming the wave\n`,
            );
          }
          if (dist > 3) {
            // Never chase a retaliation target away from the wave anchor: it is on the
            // bot, so it closes by itself. The wave stays the job.
            engagedId = undefined;
            await delay(REACH_POLL_MS);
            continue;
          }
        }
        if (dist > 3) {
          engagedId = undefined; // moving; re-establish the melee timer on arrival
          try {
            await this.walkTo(
              [Math.floor(mob.position.x), Math.floor(mob.position.y), Math.floor(mob.position.z)],
              2,
              `mob ${mob.name ?? "?"}`,
              step.sneak,
            );
          } catch (err) {
            if (err instanceof BotDeathError) throw err;
            // Cannot path to this candidate (wedged in geometry, across a void gap, or
            // a far Invulnerable actor) — drop it and hunt the next real wave mob
            // rather than failing the whole step on an unreachable non-target.
            blacklist.add(mob.id);
          }
        } else {
          if (engagedId !== mob.id) {
            engagedId = mob.id;
            engagedSince = Date.now();
          }
          // Every mob the bot melees during this step is recorded, retaliation target or
          // not. #173 excluded retaliation targets to stop a non-wave stalker inflating
          // the count; that was over-broad — a WAVE mob that attacks the bot is picked by
          // the retaliation rule too, and refusing to credit it makes the objective
          // impossible to finish (ladder run 13). The proximity rule in
          // {@link creditsWaveKill} is the arbiter, exactly as it was before self-defense
          // existed.
          engagement.engaged.add(mob.id);
          await bot.lookAt(mob.position.offset(0, (mob.height ?? 1) * 0.5, 0), true);
          bot.attack(mob);
          await delay(ATTACK_INTERVAL_MS);
          // Meleed in range this long and still alive → Invulnerable story actor;
          // blacklist it so the loop stops fixating and looks for a real wave mob.
          if (Date.now() - engagedSince >= WAVE_UNKILLABLE_MS) {
            blacklist.add(mob.id);
            engagedId = undefined;
          }
        }
      }
    } finally {
      bot.removeListener("entityGone", onGone);
      this.activeWave = undefined;
    }
    throw new Error(
      `kill timed out after ${KILL_TIMEOUT_MS}ms: wave ${step.wave} ` +
        `(${engagement.killed}/${step.count} confirmed dead; ` +
        `${engagement.engaged.size} mob(s) engaged) not cleared`,
    );
  }


  /**
   * Adopt the compiler's combat plan (spec-0023). With it, a `kill` step becomes a
   * verified ENCOUNTER rather than a fight to be won: the die-retry stage proves
   * dying is safe, the assist windows keep bot fencing skill from capping how hard
   * a delve may be, and a billed `elite`/`boss` gets one honest unassisted attempt
   * so the inverted floor gate has something to measure.
   */
  useCombatPlan(plan: CombatPlan, dieRetry: boolean): void {
    this.combatPlan = plan;
    this.dieRetry = dieRetry;
  }

  /** Every assist window this run opened, for the run report. */
  assistWindows(): readonly AssistWindow[] {
    return this.assists.windows();
  }

  /** Assist windows the harness opened and failed to close — a harness bug, and
   * one the report shows rather than swallows. */
  leakedAssists(): readonly AssistWindow[] {
    return this.assists.leaked();
  }

  /** Every scripted death of the die-retry stage — including the ones whose loop
   * the run abandoned half-way, which is the whole point of recording on death. */
  deathTrials(): readonly DeathTrial[] {
    return this.trials;
  }

  /** Waves the die-retry stage entered. A wave here with no completed trial is an
   * unproven retry loop, not a silent pass. */
  dieRetryEngagements(): ReadonlySet<string> {
    return this.dieRetryEngaged;
  }

  /** How far `kill()` got with `wave`. `not-reached` when the run ended first. */
  encounterPhase(wave: string): EncounterPhase {
    return this.encounterPhases.get(wave) ?? "not-reached";
  }

  /** Inverted floor-gate findings (advisory, spec-0023). */
  floorGateFindings(): readonly string[] {
    return this.floorFindings;
  }

  /** The plan's entry for `wave`, if the campaign declares one. */
  private encounterFor(wave: string): Encounter | undefined {
    return this.combatPlan?.encounters.find((e) => e.wave === wave);
  }

  /**
   * The critical path's `kill` step (spec-0023 §1/§3/§4).
   *
   * Order is the whole design. The die-retry stage runs FIRST, while the
   * encounter is still live — dying to a fight already won proves nothing — and
   * only then is the fight taken to completion, unassisted first when the content
   * billed it hard.
   */
  async kill(step: KillStep): Promise<void> {
    const enc = this.encounterFor(step.wave);
    if (!enc) {
      // No combat plan (or a wave outside it): pre-spec-0023 behaviour, untouched.
      await this.fightWave(step);
      return;
    }
    if (this.dieRetry) {
      this.encounterPhases.set(enc.wave, "die-retry");
      await this.dieRetryAt(step, enc);
    }
    if (assistPolicy(enc) === "unassisted-first") {
      this.encounterPhases.set(enc.wave, "unassisted");
      const won = await this.attemptUnassisted(step, enc);
      const finding = floorFinding(enc, { attempted: true, won });
      if (finding) {
        this.floorFindings.push(finding);
        process.stderr.write(`[floor] ${finding}\n`);
      }
      if (won) {
        this.encounterPhases.set(enc.wave, "cleared");
        return;
      }
      process.stderr.write(
        `[assist] ${step.wave}: the unassisted attempt did not clear the fight — ` +
          `taking a labelled assist window\n`,
      );
      this.encounterPhases.set(enc.wave, "assisted");
      await this.withAssist(enc, "after an unassisted attempt failed", () =>
        this.fightWave(step),
      );
      this.encounterPhases.set(enc.wave, "cleared");
      return;
    }
    this.encounterPhases.set(enc.wave, "assisted");
    await this.withAssist(enc, "policy: ordinary encounter", () => this.fightWave(step));
    this.encounterPhases.set(enc.wave, "cleared");
  }

  /**
   * One honest, unassisted attempt at a billed encounter. Returns whether the bot
   * cleared it; a death or a timeout is a normal `false`, not a failed run — the
   * bot losing a souls fight is the DESIGN, and spec-0023 downgraded bot melee
   * competence from gate-critical to telemetry precisely so it could be.
   */
  private async attemptUnassisted(step: KillStep, enc: Encounter): Promise<boolean> {
    process.stderr.write(
      `[floor] ${step.wave} is billed \`${enc.tier}\` — one unassisted attempt first\n`,
    );
    try {
      await this.fightWave(step);
      return true;
    } catch (err) {
      const detail = err instanceof Error ? err.message : String(err);
      process.stderr.write(`[floor] ${step.wave}: unassisted attempt ended — ${detail}\n`);
      if (this.death) await this.respawnAndRearm();
      return false;
    }
  }

  /** Run `body` inside a bounded, logged Resistance window. */
  private async withAssist<T>(
    enc: Encounter,
    reason: string,
    body: () => Promise<T>,
  ): Promise<T> {
    const bot = this.requireBot();
    const window = this.assists.open(enc, reason, Date.now());
    process.stderr.write(
      `[assist] OPEN ${enc.wave} (${enc.objective}, tier ${enc.tier}): resistance ` +
        `amplifier ${window.amplifier} for ${window.ticks} ticks — ${reason}\n`,
    );
    bot.chat(assistCommand());
    try {
      return await body();
    } finally {
      bot.chat(assistClearCommand());
      this.assists.close(window, Date.now());
      process.stderr.write(`[assist] CLOSE ${enc.wave}\n`);
    }
  }

  /**
   * The die-retry ladder stage for one encounter (spec-0023 §1): the load-bearing
   * combat proof. In a souls delve the sacred property is not winning — it is that
   * dying is always SAFE. So the harness deliberately dies to each mandatory
   * encounter and proves the whole loop: death → respawn at the governing
   * checkpoint → the route back is walkable → the encounter re-engages → and no
   * completed objective was lost on the way.
   */
  private async dieRetryAt(step: KillStep, enc: Encounter): Promise<void> {
    const bot = this.requireBot();
    // Recorded BEFORE the approach walk: from here on, silence about this
    // encounter is a finding. `dieRetryCoverageFailures` turns an engagement with
    // no completed trial into a red stage, so a run that dies on the way in can
    // never report a passed die-retry (task #102).
    // PRECONDITION (compiler #220): the loop this stage proves is "death → respawn
    // at the governing checkpoint → walk back". If that checkpoint was never armed
    // — a bonfire the route walked past without resting — the respawn lands at
    // world spawn and every measurement below describes the harness's own gap, not
    // the delve. Report it as the gap it is and take NO death: a scripted death
    // here would blame the campaign for a proof that skipped the player loop.
    const precondition = checkpointPreconditionFinding(
      enc,
      this.restSteps,
      this.restedBonfires,
      this.currentStep,
    );
    if (precondition !== undefined) {
      this.preconditionFindings.push(precondition);
      this.preconditionWaves.add(enc.wave);
      process.stderr.write(`[die-retry] ${precondition}\n`);
      return;
    }
    this.dieRetryEngaged.add(enc.wave);
    await this.walkTo(step.pos, 3, `die-retry approach ${step.wave}`, step.sneak);
    const phases = deathPhases();
    for (const [i, phase] of phases.entries()) {
      const attempt = i + 1;
      // A death still latched from the last loop (the bot was killed for real on
      // the way back) would make the next scripted death resolve instantly and
      // credit a trial that never happened. Clear it first, honestly.
      if (this.death) {
        process.stderr.write(
          `[die-retry] an unscripted death is still pending — recovering from it before ` +
            `taking the next scripted one\n`,
        );
        await this.respawnAndRearm();
      }
      // "mid-fight" means the bot has traded blows first; "first-contact" is the
      // moment of arrival. Both are the same command, taken at different times —
      // what differs is the wave state the respawn has to restore.
      if (phase === "mid-fight") {
        await this.tradeBlows(step);
      }
      const before = new Set(this.completedObjectives.keys());
      // Which wave mobs this life fought. A re-seat must replace every one of
      // them; an id that survives into the next life IS the chipped survivor the
      // owner ruling forbids (2026-08-03).
      const seenBefore = new Set(this.waveSightings(enc).map((sight) => sight.id));
      process.stderr.write(
        `[die-retry] ${step.wave} death ${attempt}/${phases.length} (${phase})\n`,
      );
      // The record exists from the moment the harness commits to dying. Everything
      // below MUTATES it, so however the run ends the artifact still says a death
      // was taken here and what was (and was not) learned from it.
      const trial = openTrial(enc, attempt, phase);
      this.trials.push(trial);
      try {
        const seq = this.deathSeq;
        bot.chat(scriptedDeathCommand());
        if (!(await this.awaitDeathAfter(seq, RESPAWN_TIMEOUT_MS))) {
          // The bot is opped for exactly this command; if no death followed, the
          // command was refused (or the op seed drifted) and the stage proves
          // nothing. Fail the trial rather than walk a loop nobody opened.
          trial.abortedWith =
            `the scripted death never landed within ${RESPAWN_TIMEOUT_MS}ms — ` +
            `\`${scriptedDeathCommand()}\` was refused (is the bot opped?)`;
          throw new Error(`die-retry: ${trial.abortedWith}`);
        }
        trial.cause = this.death?.likelyCause;
        trial.respawnPos = await this.respawnAndRearm();
        trial.atCheckpoint = respawnedAtCheckpoint(trial.respawnPos ?? [0, 0, 0], enc.checkpoint);
        try {
          await this.walkTo(step.pos, 3, `die-retry return ${step.wave}`, step.sneak);
          trial.returned = true;
        } catch (err) {
          const detail = err instanceof Error ? err.message : String(err);
          process.stderr.write(`[die-retry] return leg failed: ${detail}\n`);
        }
        const after = new Set(this.completedObjectives.keys());
        trial.lostObjectives = [...before].filter((o) => !after.has(o));
        trial.objectivesIntact = trial.lostObjectives.length === 0;
        // Two observations, one verdict (see RetryOutcome). A wave mob standing
        // here again means the fight is retriable. Nothing left to fight is only
        // a failure if the encounter's objective is ALSO unfinished — then the
        // party can neither complete it nor re-fight it, which is a soft lock.
        // A wave already beaten before the death is a won fight staying won.
        const obs = await this.awaitReengage(enc, seenBefore);
        trial.reengage = obs;
        trial.reEngaged = obs.present > 0;
        trial.objectiveComplete = this.completedObjectives.has(enc.objective);
        trial.outcome = retryOutcome(trial.reEngaged, trial.objectiveComplete);
        process.stderr.write(
          `[die-retry] ${step.wave} death ${attempt}: ${obs.present}/${obs.declared} wave mob(s) ` +
            `after ${obs.settleMs}ms` +
            `${obs.nearest !== undefined ? `, ${obs.nearest.toFixed(1)}–${obs.farthest!.toFixed(1)} blocks from the anchor` : ""}` +
            `${obs.carriedOver > 0 ? `, ${obs.carriedOver} carried over from a previous life` : ""}` +
            `${obs.healthReadable > 0 ? `, ${obs.damaged}/${obs.healthReadable} damaged` : ""}\n`,
        );
        process.stderr.write(
          `[die-retry] ${step.wave} death ${attempt}: ${trial.outcome}` +
            `${trial.outcome === "cleared-before-retry" ? ` (\`${enc.objective}\` was already complete — the death cost no progress)` : ""}\n`,
        );
        trial.completed = true;
      } catch (err) {
        trial.abortedWith ??= err instanceof Error ? err.message : String(err);
        process.stderr.write(
          `[die-retry] ${step.wave} death ${attempt} loop abandoned: ${trial.abortedWith}\n`,
        );
        throw err;
      }
    }
  }

  /**
   * Wait for a death NEWER than `seq` — the harness's own scripted one.
   *
   * Deliberately not {@link waitFor}, which THROWS the recorded
   * {@link BotDeathError} the instant one exists. That is right everywhere else
   * in the harness (a death mid-step is a failure to surface fast) and fatal
   * here, where the death IS the step: `waitFor(() => this.death !== undefined)`
   * threw on the very condition it was asked to wait for, so no die-retry trial
   * could ever complete and the harness's own scripted death was reported as the
   * content killing the bot (task #102, the-drowned-bell round 3).
   */
  private async awaitDeathAfter(seq: number, timeoutMs: number): Promise<boolean> {
    const deadline = Date.now() + timeoutMs;
    for (;;) {
      if (this.deathSeq > seq) return true;
      if (Date.now() >= deadline) return false;
      await delay(SCORE_POLL_MS);
    }
  }

  /**
   * Every wave mob the client currently tracks, with its distance from the
   * encounter anchor and — where the server surfaced them — its health and max
   * health.
   *
   * A SET, not a nearest hit: the re-seat fidelity check needs the whole wave,
   * and a feral mob that wandered off the anchor after killing the party is still
   * part of the fight. There is deliberately no distance filter — `follow_range`
   * (48 at the top end) sits well inside vanilla's 128-block monster tracking
   * range, so anything the client can see is something that can still come.
   *
   * Health is read through the PINNED REGISTRY's named metadata layout
   * (`metadataKeys`, where `health` is resolved by name for 1.21.11), never a
   * hardcoded packet index — a protocol-version constant baked into the harness
   * would be exactly the downstream folklore CLAUDE.md forbids. Both fields are
   * optional: a server that never sent this entity's attributes leaves max health
   * unknown, and an unknown is reported as unknown, never as full.
   */
  private waveSightings(enc: Encounter): WaveSighting[] {
    const bot = this.requireBot();
    const anchor = enc.pos;
    const out: WaveSighting[] = [];
    for (const e of Object.values(bot.entities)) {
      if (!isWaveMob(e, bot.entity)) continue;
      const p = e.position;
      if (!p) continue;
      const dx = p.x - anchor[0];
      const dy = p.y - anchor[1];
      const dz = p.z - anchor[2];
      const health = this.entityHealth(e);
      out.push({
        id: e.id,
        distance: Math.sqrt(dx * dx + dy * dy + dz * dz),
        health,
        maxHealth: this.fullHealthOf(enc.wave, e, health),
      });

    }
    return out;
  }

  /** Current health off the entity's metadata, addressed by NAME through the
   * pinned registry's layout for this mob. `undefined` when the layout does not
   * publish one or no metadata has arrived yet. */
  private entityHealth(e: Entity): number | undefined {
    const bot = this.requireBot();
    const keys = (
      bot.registry as { entitiesByName?: Record<string, { metadataKeys?: string[] }> }
    ).entitiesByName?.[e.name ?? ""]?.metadataKeys;
    const idx = keys?.indexOf("health") ?? -1;
    if (idx < 0) return undefined;
    const raw = (e as unknown as { metadata?: Record<number, unknown> }).metadata?.[idx];
    return typeof raw === "number" && Number.isFinite(raw) ? raw : undefined;
  }

  /**
   * What "full health" means for this mob type in this wave.
   *
   * Preferred source is the server's own `entity_update_attributes`, but vanilla
   * only transmits attributes that DIFFER from the entity's defaults — a live
   * 1.21.11 server sends a wave zombie nothing but `generic.scale`, so max health
   * is simply not on the wire (verified against the keep-trial fixture, task
   * #108). Falling back to a vanilla health table would be inventing data the
   * compiler itself refuses to invent (DW0475).
   *
   * So the baseline is the wave AS THIS RUN FIRST MET IT: the highest health ever
   * observed for this mob type in this wave. The bot always sees the wave fresh —
   * the die-retry approach happens before a blow is struck — so the baseline is
   * established whole, and "identical to what the first life faced" is exactly
   * the property the owner ruling asks for. It errs conservative by construction:
   * a baseline can only ever be too LOW, which can hide damage, never invent it.
   */
  private fullHealthOf(wave: string, e: Entity, health: number | undefined): number | undefined {
    const attrs = (
      e as unknown as { attributes?: Record<string, { value?: unknown } | undefined> }
    ).attributes;
    for (const key of ["minecraft:max_health", "generic.max_health", "max_health"]) {
      const raw = attrs?.[key]?.value;
      if (typeof raw === "number" && Number.isFinite(raw)) return raw;
    }
    const slot = `${wave}/${e.name ?? "?"}`;
    const seen = this.waveFullHealth.get(slot);
    if (health !== undefined && (seen === undefined || health > seen)) {
      this.waveFullHealth.set(slot, health);
      return health;
    }
    return seen;
  }

  /**
   * Wait for the encounter to show itself, then describe what came back.
   *
   * Returns the moment the declared wave is standing, so a healthy run pays
   * nothing; otherwise it settles for {@link REENGAGE_SETTLE_MS} before
   * concluding. A single instantaneous sample was the island-r14 false negative:
   * entity tracking lags arrival by ticks, and three living drowned read as an
   * empty room.
   */
  private async awaitReengage(
    enc: Encounter,
    seenBefore: ReadonlySet<number>,
  ): Promise<ReengageObservation> {
    const started = Date.now();
    const deadline = started + REENGAGE_SETTLE_MS;
    let sightings = this.waveSightings(enc);
    for (;;) {
      // Enough is standing to answer every question this observation feeds.
      if (sightings.length >= enc.count) break;
      if (Date.now() >= deadline) break;
      await delay(REACH_POLL_MS);
      sightings = this.waveSightings(enc);
    }
    return observationOf(sightings, enc.count, seenBefore, Date.now() - started);
  }

  /** Melee whatever wave mob is closest for a moment, so the next scripted death
   * lands mid-fight rather than at first contact. Best effort by design — if
   * nothing is in reach there is nothing to trade with, and the trial still runs. */
  private async tradeBlows(step: KillStep): Promise<void> {
    const bot = this.requireBot();
    const deadline = Date.now() + MID_FIGHT_MS;
    while (Date.now() < deadline && !this.death) {
      const mob = bot.nearestEntity((e) => isWaveMob(e, bot.entity));
      if (!mob) break;
      if (bot.entity.position.distanceTo(mob.position) > 3) {
        try {
          await this.walkTo(
            [Math.floor(mob.position.x), Math.floor(mob.position.y), Math.floor(mob.position.z)],
            2,
            `die-retry close ${step.wave}`,
            step.sneak,
          );
        } catch {
          break;
        }
        continue;
      }
      bot.attack(mob);
      await delay(ATTACK_INTERVAL_MS);
    }
  }

  /** Wait out a death, note WHERE the bot came back, then replay `select-class` so
   * it is armed again. The position is read before the re-selection, because the
   * class trigger teleports — the respawn point is what the checkpoint contract is
   * about, and it must be measured while it is still observable. */
  private async respawnAndRearm(): Promise<Vec3Tuple | undefined> {
    const bot = this.requireBot();
    await this.recoverFromDeath();
    const p = bot.entity?.position;
    const respawnPos: Vec3Tuple | undefined =
      p === undefined ? undefined : [Math.floor(p.x), Math.floor(p.y), Math.floor(p.z)];
    if (this.lastSelectClass) {
      await this.selectClass(this.lastSelectClass);
    }
    return respawnPos;
  }

  /**
   * Rest at a bonfire (compiler #220) — the player loop every later proof depends on.
   *
   * Two acts, in this order, and the order is the whole thing:
   *
   *   1. **right-click the `dw_bonfire_<i>` interaction**. This is not flavour. The
   *      click is what fires the `player_interacted_with_entity` advancement whose
   *      reward opens the dialog AND `enable`s the `dw.rest` trigger. Until then the
   *      trigger is DISABLED and the chat line below is a silent no-op.
   *   2. **chat the step's command** — the exact line the "rest and save" button runs.
   *
   * Why not click the dialog button: a `dialog show` is rendered client-side and
   * mineflayer models no dialog at all, so there is no button to press. The button's
   * command is the primitive the compiler exports precisely so a headless client can
   * perform the same loop; `/trigger` is also the only command form a non-operator
   * player may run, so this is the player's own path, not an op shortcut.
   *
   * The affordance is found by POSITION, not by tag: entity `Tags` are server-side
   * and never reach a client. The compiler puts the interaction on the step's own
   * anchor cell, so the nearest `interaction` entity to it is the fire.
   *
   * Actuation only. Nothing here asserts the checkpoint moved — the next die-retry
   * trial's respawn position is what proves that, and it proves it the way a player
   * would find out.
   */
  async rest(step: RestStep): Promise<void> {
    const bot = this.requireBot();
    await this.walkTo(step.pos, 2, `bonfire ${step.anchor}`, step.sneak);
    const fire = this.affordanceAt(step.pos);
    if (!fire) {
      throw new Error(
        `no \`interaction\` affordance within ${AFFORDANCE_RADIUS} blocks of bonfire ` +
          `${step.bonfire} at [${step.pos.join(", ")}] — the bot is standing at the fire ` +
          `and there is nothing to right-click, so the rest can never be performed ` +
          `(bot at ${fmt(bot.entity.position)})`,
      );
    }
    process.stderr.write(
      `[rest] bonfire ${step.bonfire} (${step.anchor}): right-clicking the affordance, ` +
        `then \`${step.command}\`\n`,
    );
    await bot.activateEntity(fire);
    // The opener runs through an advancement reward, so the trigger is enabled a
    // tick or two after the click lands — chatting inside the same tick would be
    // refused exactly as chatting without clicking is.
    await delay(REST_OPEN_SETTLE_MS);
    bot.chat(step.command);
    await delay(EFFECT_SETTLE_MS);
    this.restedBonfires.add(step.bonfire);
  }

  /** The nearest `minecraft:interaction` affordance to `pos`, if one is tracked. */
  private affordanceAt(pos: Vec3Tuple): Entity | undefined {
    const bot = this.requireBot();
    let best: Entity | undefined;
    let bestDist = AFFORDANCE_RADIUS;
    for (const e of Object.values(bot.entities)) {
      if (e?.name !== "interaction" || !e.position) continue;
      const d = Math.sqrt(
        (e.position.x - (pos[0] + 0.5)) ** 2 +
          (e.position.y - pos[1]) ** 2 +
          (e.position.z - (pos[2] + 0.5)) ** 2,
      );
      if (d <= bestDist) {
        best = e;
        bestDist = d;
      }
    }
    return best;
  }

  /** Adopt the rest steps the exported critical path carries (compiler #220), with
   * their EXPORTED indices — the coordinate system the precondition compares in. */
  useRestSteps(rests: readonly PerformedRest[]): void {
    this.restSteps = rests;
  }

  /** Rests this run performed, and the bonfires among them. For the run report. */
  performedRests(): readonly PerformedRest[] {
    return this.restSteps.filter((r) => this.restedBonfires.has(r.bonfire));
  }

  /** Encounters whose scripted deaths were skipped for want of an armed checkpoint. */
  dieRetryPreconditionFindings(): readonly string[] {
    return this.preconditionFindings;
  }

  /** The waves those findings name. Coverage stays silent about them: the
   * precondition already says why they are unproven, and "never reached this
   * encounter" would be plainly untrue — the bot stood in the room and declined. */
  dieRetryPreconditionWaves(): ReadonlySet<string> {
    return this.preconditionWaves;
  }

  /** Collect items from the chest at the anchor: go there, open it, withdraw all. */
  async collect(step: CollectStep): Promise<void> {
    const bot = this.requireBot();
    await this.walkTo(step.pos, 2, `chest ${step.item}`, step.sneak);
    const here = bot.entity.position;
    const target = here.offset(
      step.pos[0] + 0.5 - here.x,
      step.pos[1] + 0.5 - here.y,
      step.pos[2] + 0.5 - here.z,
    );
    const block = bot.blockAt(target);
    if (!block) {
      throw new Error(`no block at collect anchor [${step.pos.join(", ")}]`);
    }
    const chest = await bot.openContainer(block);
    try {
      for (const item of chest.containerItems()) {
        await chest.withdraw(item.type, null, item.count);
      }
    } finally {
      chest.close();
    }
    // Holding the items is not the objective; the inventory_changed advancement
    // completing it is. Wait for that objective's own marker.
    await this.requireObjective(step.objective, `collect ${step.item}`);
  }

  /**
   * Interact at the anchor: go there, take the required item in hand, then chat
   * the emitted `/trigger` command.
   *
   * The interaction advancement and that chat command both feed the same per-tick
   * handler, and the datapack applies the `requires_item` + flag guards there —
   * `requires_item` against the MAINHAND (compiler PR #205), which is why the hand
   * is loaded first. See {@link presentAndTrigger}.
   */
  async interact(step: InteractStep): Promise<void> {
    const bot = this.requireBot();
    await this.walkTo(step.pos, 3, `interact ${step.anchor}`, step.sneak);
    await presentAndTrigger<Item>(bot, step, step.anchor);
    await this.requireObjective(step.objective, `interact ${step.anchor}`);
    await delay(EFFECT_SETTLE_MS);
  }

  /**
   * gap 8: after a step whose completion teleports the player to another area, hold
   * the next step's pathfinding until the relocation has fully landed. Areas sit
   * ~256 blocks apart across void, so the destination is far from the pre-teleport
   * position and the arrival is unambiguous. Navigation plumbing only — no game logic.
   *
   * Three deterministic phases (task #32), each bounded and death-aware so nothing
   * can hang the run and a mid-transport death still fails fast:
   *   1. Wait for the position to jump to near `dest` — the teleport landing. The
   *      `forcedMove` handler resets the pathfinder as the jump arrives, so a path
   *      computed in the old area cannot survive it.
   *   2. Reset the pathfinder again here (belt-and-braces): the next `walkTo` must
   *      start from a clean state at the new position.
   *   3. Wait for the destination chunk to load and the bot to rest on solid footing.
   *      A `walkTo` that starts while the block under the bot is still unloaded
   *      (`blockAt` → null) makes the pathfinder's A* fail instantly with "No path to
   *      the goal!"; this closes that race at the boundary rather than racing ahead.
   * If the jump is not observed within the budget, settle briefly and let the next
   * step surface its own diagnostic.
   */
  async awaitTransport(dest: readonly [number, number, number]): Promise<void> {
    const bot = this.requireBot();
    const [x, y, z] = dest;
    const arrived = await this.waitFor(
      () => {
        const p = bot.entity.position;
        return (
          Math.abs(p.x - (x + 0.5)) < TRANSPORT_NEAR &&
          Math.abs(p.z - (z + 0.5)) < TRANSPORT_NEAR &&
          Math.abs(p.y - y) < 4
        );
      },
      TRANSPORT_TIMEOUT_MS,
      REACH_POLL_MS,
    );
    // Drop any path/goal still referencing the old area (the forcedMove handler has
    // usually done this already; idempotent).
    this.stopPathfinding();
    if (!arrived) {
      process.stderr.write(
        `[transport] did not observe the jump to [${x}, ${y}, ${z}] within ` +
          `${TRANSPORT_TIMEOUT_MS}ms; bot at ${fmt(bot.entity.position)} — continuing\n`,
      );
      await delay(TRANSPORT_SETTLE_MS);
      return;
    }
    // The jump landed: wait for the destination chunk to load and the bot to settle
    // onto solid ground before the next step pathfinds from here.
    const footed = await this.waitFor(
      () => bot.entity.onGround === true && bot.blockAt(bot.entity.position) != null,
      FOOTING_TIMEOUT_MS,
      FOOTING_POLL_MS,
    );
    if (!footed) {
      process.stderr.write(
        `[transport] landed near [${x}, ${y}, ${z}] but footing/chunk not confirmed ` +
          `within ${FOOTING_TIMEOUT_MS}ms; bot at ${fmt(bot.entity.position)} — continuing\n`,
      );
    }
    await delay(TRANSPORT_SETTLE_MS);
  }

  /**
   * Poll `predicate` every `pollMs` until it holds (→ true) or `timeoutMs` elapses
   * (→ false). Death-aware: throws the recorded {@link BotDeathError} the moment the
   * bot dies, so a transport/footing wait never outlives a death. A pure timing helper
   * — no game logic.
   */
  private async waitFor(
    predicate: () => boolean,
    timeoutMs: number,
    pollMs: number,
  ): Promise<boolean> {
    const deadline = Date.now() + timeoutMs;
    for (;;) {
      if (this.death) throw this.death;
      if (predicate()) return true;
      if (Date.now() >= deadline) return false;
      await delay(pollMs);
    }
  }

  /**
   * gap 7 (cutscene): after a step marked `cutscene_seconds`, the compiler may force
   * the bot into spectator and dolly a camera for ~n seconds, then restore gamemode
   * and position. The harness makes no assertions about the cutscene — it only waits
   * for control to return so the next step does not start pathfinding mid-spectator.
   *
   * Two phases, both death-aware and bounded (deadline = n + grace, so a cutscene
   * glitch cannot hang the run):
   *   1. Sleep through the declared duration — the bot is out of our control anyway
   *      (this also covers a cutscene brief enough that we would otherwise miss the
   *      spectator window entirely).
   *   2. Extend the awaitTransport discontinuity pattern: wait for the gamemode to be
   *      back to adventure AND the position to hold steady for a short settle window.
   */
  async awaitCutscene(seconds: number): Promise<void> {
    const bot = this.requireBot();
    const start = Date.now();
    const minEnd = start + seconds * 1000;
    const deadline = minEnd + this.cutsceneGraceMs;

    // Phase 1: wait out the declared cutscene length (control is not ours meanwhile).
    while (Date.now() < minEnd) {
      if (this.death) throw this.death;
      await delay(CUTSCENE_POLL_MS);
    }

    // Phase 2: confirm control returned — adventure mode AND a settled position.
    let steadySince: number | undefined;
    let last = bot.entity.position.clone();
    while (Date.now() < deadline) {
      if (this.death) throw this.death;
      const here = bot.entity.position;
      const moved = here.distanceTo(last) > CUTSCENE_STEADY_EPS;
      last = here.clone();
      if (bot.game.gameMode === "adventure" && !moved) {
        steadySince ??= Date.now();
        if (Date.now() - steadySince >= CUTSCENE_SETTLE_MS) return;
      } else {
        steadySince = undefined;
      }
      await delay(CUTSCENE_POLL_MS);
    }
    process.stderr.write(
      `[cutscene] control not confirmed restored within ${seconds}s + grace; ` +
        `gamemode ${bot.game.gameMode}, bot at ${fmt(bot.entity.position)} — continuing\n`,
    );
  }

  async assertComplete(step: AssertCompleteStep): Promise<void> {
    const bot = this.requireBot();
    // Completion is observed two ways, whichever surfaces first:
    //   1. The anchored campaign-completion marker (the working path on 1.21.11 —
    //      see markers.ts), buffered since connect.
    //   2. The sidebar score via mineflayer (future-proof: works if/when mineflayer
    //      gains 1.21.11 score-packet support; currently always unset).
    // The campaign completes during the LAST objective step; the sequencer has
    // already failed the run if the marker arrived any earlier than that
    // (assertEndgameNotReached), so reaching here means it is either due now or due
    // within a tick or two of the last objective.
    const deadline = Date.now() + SCORE_SETTLE_MS;
    while (Date.now() < deadline) {
      if (this.death) throw this.death;
      if (this.campaignCompleteAtStep !== undefined) {
        return;
      }
      const board = bot.scoreboards[step.objective];
      if (board?.itemsMap[bot.username]?.value === step.value) {
        return;
      }
      await delay(SCORE_POLL_MS);
    }
    const board = bot.scoreboards[step.objective];
    const sidebar = board?.itemsMap[bot.username]?.value ?? "unset";
    const done = [...this.completedObjectives.keys()];
    throw new Error(
      `campaign not complete after ${SCORE_SETTLE_MS}ms: no ` +
        `\`${markerLine(this.campaignId ?? "?", CAMPAIGN_TOKEN)}\` marker arrived ` +
        `(objective ${step.objective} expected ${step.value}; sidebar: ${sidebar}); ` +
        `objectives completed: ${done.join(", ") || "none"}`,
    );
  }
}

function fmt(p: { x: number; y: number; z: number }): string {
  return `[${p.x.toFixed(1)}, ${p.y.toFixed(1)}, ${p.z.toFixed(1)}]`;
}

/**
 * Entity names that are never a combat target. The delve world is sealed
 * (`spawn_mobs false`), so the only living mobs are compiler-summoned wave mobs;
 * everything else near the bot is an NPC, a display, or a dropped object.
 */
const NON_WAVE_ENTITIES = new Set<string>([
  "player",
  "villager",
  // Every Delvewright NPC (class-post puppet, quest-giver) is summoned as an
  // Invulnerable `minecraft:mannequin` (compiler emit.rs — `immovable:1b`,
  // `Invulnerable:1b`). mineflayer's minecraft-data DOES resolve its name
  // ("mannequin", type "living", height 1.8), so without this exclusion the kill
  // loop's nearestEntity(isWaveMob) classifies a mannequin as a slayable wave mob.
  // When a wave anchor sits next to a class post (the nobodys-cave surf wave spawns
  // a block from the eurylochus mannequin), the nearest "wave mob" becomes an
  // Invulnerable mannequin at d<3: the bot attacks it in place forever, never
  // clearing the wave and never hunting the real remaining drowned that wandered
  // off — a 90s kill timeout. NPCs are never combat targets, so exclude the whole
  // entity type; this generalizes to every future mannequin-beside-combat layout
  // rather than depending on anchor placement.
  "mannequin",
  "interaction",
  "item",
  "experience_orb",
  "arrow",
  "spectral_arrow",
  "armor_stand",
  "marker",
  "text_display",
  "block_display",
  "item_display",
  "area_effect_cloud",
  "item_frame",
  "glow_item_frame",
  "painting",
  "leash_knot",
  "fishing_bobber",
]);

/**
 * True if `e` is a slayable wave mob: not the bot, not a player/NPC/display/
 * dropped object, and tall enough to be a living mob (excludes small dropped
 * entities). Classified by name (reliable across mineflayer versions) rather than
 * `type`/`kind`, which vary.
 */
export function isWaveMob(e: unknown, self: unknown): boolean {
  if (!e || e === self) return false;
  const ent = e as { name?: string; height?: number };
  const name = ent.name ?? "";
  if (name === "" || NON_WAVE_ENTITIES.has(name)) return false;
  return (ent.height ?? 0) >= 0.5;
}
